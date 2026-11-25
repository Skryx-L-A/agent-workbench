//! The target list of mode 2 `kopf-einsetzen` (contract 8, section 12): a session file of "Lauf starten", JSON, at most
//! 65536 bytes, closed form on every level, read in 15 stages. The first failing stage names the code (`zl.…`).

use serde_json::{Map, Value};

use crate::aufruf::kennung_ok;
use crate::json::lies_streng;

/// Largest target list in bytes.
pub const ZL_MAX: usize = 65_536;
/// The known methods of a rule (besides `*`).
pub const METHODEN: [&str; 7] = ["GET", "HEAD", "POST", "PUT", "PATCH", "DELETE", "OPTIONS"];
const OBEN: [&str; 3] = ["fassung", "ca_zulassung", "ziele"];
/// The form of a target: six keys. The three fields that fell away on the way to the holder (`schluessel_datei`,
/// `kopf` and `wert_vorsatz`) stay known here, so a list that still carries one of them next to `halter_ziel` names
/// stage 12 (`zl.bindung`) and not stage 3 (contract 8, section 12).
const ZIEL: [&str; 6] = ["name", "host", "port", "halter_ziel", "regeln", "upgrade"];
/// The fields that fell away; named at stage 12.
const WEGGEFALLEN: [&str; 3] = ["schluessel_datei", "kopf", "wert_vorsatz"];
const REGEL: [&str; 2] = ["methode", "pfad"];
/// The methods a rule of a **bound** target may name: `halter.json` knows only these two, and `modell.aufruf` takes
/// only them (contract 8, section 12, stage 11).
pub const GEBUNDEN_METHODEN: [&str; 2] = ["GET", "POST"];
/// Largest JSON integer that JavaScript reads exactly (the reference model reads the list with it).
const SICHER: u64 = 9_007_199_254_740_991;

/// The answer of the company on the local CA.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Zulassung {
    /// The mode may start.
    Zugelassen,
    /// No answer yet: no start.
    Offen,
    /// Refused: no start.
    Abgelehnt,
}

/// One rule: a method (or `*`) and a path (exact, a subtree `…/*` or `/*`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Regel {
    /// `GET`, `HEAD`, `POST`, `PUT`, `PATCH`, `DELETE`, `OPTIONS` or `*`.
    pub methode: String,
    /// The path of the rule.
    pub pfad: String,
}

impl Regel {
    /// Whether the rule covers `pfad`: `/*` covers every path, `…/*` the subtree with its `/`, else only the same path.
    #[must_use]
    pub fn passt(&self, pfad: &str) -> bool {
        match self.pfad.strip_suffix('*') {
            Some(vor) => pfad.starts_with(vor),
            None => pfad == self.pfad,
        }
    }

    /// Whether the rule allows `methode` (`*` allows each of the seven known ones).
    #[must_use]
    pub fn erlaubt(&self, methode: &str) -> bool {
        if self.methode == "*" {
            METHODEN.contains(&methode)
        } else {
            self.methode == methode
        }
    }
}

/// One target of the list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ziel {
    /// The name for the line.
    pub name: String,
    /// Exactly one host.
    pub host: String,
    /// Exactly one port.
    pub port: u16,
    /// The name of the target in `halter.json` (exactly `host`), or `None` for an unbound target. Only the holder
    /// knows its address, its header name and the **name** of its secret; the list never carries a key (section 5.1).
    pub halter_ziel: Option<String>,
    /// The rules (1 to 32).
    pub regeln: Vec<Regel>,
    /// Whether an upgrade (WebSocket) is handed through.
    pub upgrade_durchreichen: bool,
}

impl Ziel {
    /// Whether the target is bound: the holder holds its token and keeps its way out (section 5.1).
    #[must_use]
    pub fn gebunden(&self) -> bool {
        self.halter_ziel.is_some()
    }
}

/// A read target list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Zielliste {
    /// The answer on the local CA.
    pub zulassung: Zulassung,
    /// The targets (1 to 16).
    pub ziele: Vec<Ziel>,
}

/// Reads a target list; the error is the code of the first failing stage (`zl.…`).
///
/// # Errors
/// The code of the stage.
pub fn lies(text: &[u8]) -> Result<Zielliste, &'static str> {
    if text.len() > ZL_MAX {
        return Err("zl.zu_gross");
    }
    let Some(Value::Object(oben)) = lies_streng(text) else {
        return Err("zl.json_ungueltig");
    };
    let ebenen = ebenen(&oben);
    if ebenen
        .iter()
        .any(|(o, erlaubt, _)| o.keys().any(|k| !erlaubt.contains(&k.as_str())))
    {
        return Err("zl.schluessel_unbekannt");
    }
    if ebenen
        .iter()
        .any(|(o, _, pflicht)| pflicht.iter().any(|k| !o.contains_key(*k)))
    {
        return Err("zl.schluessel_fehlt");
    }
    let (fassung, zulassung, ziele) = typen(&oben).ok_or("zl.typ_falsch")?;
    if fassung != 1 {
        return Err("zl.fassung_unbekannt");
    }
    let anzahl_gut =
        (1..=16).contains(&ziele.len()) && ziele.iter().all(|z| (1..=32).contains(&z.regeln.len()));
    if !anzahl_gut {
        return Err("zl.anzahl");
    }
    pruefe_ziele(&ziele)?;
    let zulassung = match zulassung {
        "zugelassen" => Zulassung::Zugelassen,
        "offen" => Zulassung::Offen,
        "abgelehnt" => Zulassung::Abgelehnt,
        _ => return Err("zl.ca_zulassung_wert"),
    };
    let ziele: Vec<Ziel> = ziele.into_iter().map(Roh::ziel).collect();
    for (i, z) in ziele.iter().enumerate() {
        let doppelt = ziele[..i].iter().any(|a| {
            a.name == z.name
                || (a.host == z.host && a.port == z.port)
                || (a.halter_ziel.is_some() && a.halter_ziel == z.halter_ziel)
        });
        if doppelt {
            return Err("zl.doppelt");
        }
    }
    Ok(Zielliste { zulassung, ziele })
}

/// The objects whose keys are checked, each with the keys it may carry and the keys it must carry: the top, each
/// target object (a target may carry a field that fell away, so that stage 12 names it), each rule object.
type Ebene<'a> = (
    &'a Map<String, Value>,
    Vec<&'static str>,
    &'static [&'static str],
);

fn ebenen(oben: &Map<String, Value>) -> Vec<Ebene<'_>> {
    let ziel_erlaubt: Vec<&'static str> = ZIEL.iter().chain(WEGGEFALLEN.iter()).copied().collect();
    let mut v: Vec<Ebene<'_>> = vec![(oben, OBEN.to_vec(), &OBEN)];
    for z in oben
        .get("ziele")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let Value::Object(z) = z else { continue };
        v.push((z, ziel_erlaubt.clone(), &ZIEL));
        for r in z
            .get("regeln")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            if let Value::Object(r) = r {
                v.push((r, REGEL.to_vec(), &REGEL));
            }
        }
    }
    v
}

/// A target with its fields in their JSON types (stage 5).
struct Roh<'a> {
    name: &'a str,
    host: &'a str,
    port: i64,
    upgrade: &'a str,
    halter_ziel: Option<&'a str>,
    /// Whether a field that fell away still stands in this target (stage 12).
    weggefallen: bool,
    regeln: Vec<(&'a str, &'a str)>,
}

impl Roh<'_> {
    fn ziel(self) -> Ziel {
        Ziel {
            name: self.name.to_owned(),
            host: self.host.to_owned(),
            port: u16::try_from(self.port).unwrap_or(0),
            halter_ziel: self.halter_ziel.map(str::to_owned),
            regeln: self
                .regeln
                .iter()
                .map(|(m, p)| Regel {
                    methode: (*m).to_owned(),
                    pfad: (*p).to_owned(),
                })
                .collect(),
            upgrade_durchreichen: self.upgrade == "durchreichen",
        }
    }
}

/// An integer written without fraction and exponent, at most 2^53 - 1 in its amount.
fn ganz(v: &Value) -> Option<i64> {
    v.as_i64().filter(|n| n.unsigned_abs() <= SICHER)
}

/// Stage 5: every field in its JSON type.
fn typen(oben: &Map<String, Value>) -> Option<(i64, &str, Vec<Roh<'_>>)> {
    let fassung = ganz(&oben["fassung"])?;
    let zulassung = oben["ca_zulassung"].as_str()?;
    let mut ziele = Vec::new();
    for z in oben["ziele"].as_array()? {
        let z = z.as_object()?;
        let text_oder_null = |k: &str| match &z[k] {
            Value::Null => Some(None),
            Value::String(s) => Some(Some(s.as_str())),
            _ => None,
        };
        let mut regeln = Vec::new();
        for r in z["regeln"].as_array()? {
            let r = r.as_object()?;
            regeln.push((r["methode"].as_str()?, r["pfad"].as_str()?));
        }
        ziele.push(Roh {
            name: z["name"].as_str()?,
            host: z["host"].as_str()?,
            port: ganz(&z["port"])?,
            upgrade: z["upgrade"].as_str()?,
            halter_ziel: text_oder_null("halter_ziel")?,
            weggefallen: WEGGEFALLEN.iter().any(|k| z.contains_key(*k)),
            regeln,
        });
    }
    Some((fassung, zulassung, ziele))
}

/// Stages 8 to 13 over all targets, each stage over the whole list before the next.
fn pruefe_ziele(ziele: &[Roh<'_>]) -> Result<(), &'static str> {
    if !ziele.iter().all(|z| kennung_ok(z.name)) {
        return Err("zl.name_form");
    }
    if !ziele.iter().all(|z| host_ok(z.host)) {
        return Err("zl.host_form");
    }
    if !ziele.iter().all(|z| (1..=65_535).contains(&z.port)) {
        return Err("zl.port_grenze");
    }
    let regeln_gut = ziele.iter().all(|z| {
        z.regeln.iter().all(|(m, p)| {
            let form = (*m == "*" || METHODEN.contains(m)) && regel_pfad_ok(p);
            // A bound target carries only `GET`, `POST` or `*` (which means exactly those two there): `pfade` in
            // `halter.json` knows no other method, and `modell.aufruf` takes none (stage 11).
            let gebunden_gut =
                z.halter_ziel.is_none() || *m == "*" || GEBUNDEN_METHODEN.contains(m);
            form && gebunden_gut
        })
    });
    if !regeln_gut {
        return Err("zl.regel_form");
    }
    // Stage 12: `halter_ziel` is exactly the `host` of this target, so no caller with an own list sends a secret to
    // another host; and no field that fell away stands there any more.
    let bindung_gut = ziele
        .iter()
        .all(|z| !z.weggefallen && z.halter_ziel.is_none_or(|n| n == z.host));
    if !bindung_gut {
        return Err("zl.bindung");
    }
    let upgrade_gut = ziele.iter().all(|z| match z.upgrade {
        "ablehnen" => true,
        "durchreichen" => z.halter_ziel.is_some(),
        _ => false,
    });
    if !upgrade_gut {
        return Err("zl.upgrade_wert");
    }
    Ok(())
}

/// A DNS name of lower-case labels with at least one dot, at most 253 bytes, not only digits and dots.
#[must_use]
pub fn host_ok(host: &str) -> bool {
    let label_gut = |l: &str| {
        let b = l.as_bytes();
        let rand = |c: Option<&u8>| c.is_some_and(|c| c.is_ascii_lowercase() || c.is_ascii_digit());
        b.len() <= 63
            && rand(b.first())
            && rand(b.last())
            && b.iter()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == b'-')
    };
    host.len() <= 253
        && host.contains('.')
        && host.split('.').all(label_gut)
        && !host.bytes().all(|c| c.is_ascii_digit() || c == b'.')
}

/// A rule path: `/*`, or segments `/[A-Za-z0-9._~-]+` (not `.` or `..`) with an optional `/*` at the end.
#[must_use]
pub fn regel_pfad_ok(pfad: &str) -> bool {
    let ohne_stern = pfad.strip_suffix("/*");
    if ohne_stern == Some("") {
        return true;
    }
    let segmente = ohne_stern.unwrap_or(pfad);
    segmente.strip_prefix('/').is_some_and(|rest| {
        rest.split('/').all(|s| {
            !s.is_empty()
                && s != "."
                && s != ".."
                && s.bytes()
                    .all(|c| c.is_ascii_alphanumeric() || b"._~-".contains(&c))
        })
    })
}

#[cfg(test)]
mod tests {
    use super::{Regel, host_ok, lies, regel_pfad_ok};

    /// One target of the list; `halter_ziel` as it is written in JSON (`null` or a quoted name).
    fn ziel(name: &str, host: &str, halter_ziel: &str, methode: &str, upgrade: &str) -> String {
        format!(
            r#"{{"name":"{name}","host":"{host}","port":443,"halter_ziel":{halter_ziel},"regeln":[{{"methode":"{methode}","pfad":"/*"}}],"upgrade":"{upgrade}"}}"#
        )
    }

    fn liste(ziele: &[String]) -> String {
        format!(
            r#"{{"fassung":1,"ca_zulassung":"zugelassen","ziele":[{}]}}"#,
            ziele.join(",")
        )
    }

    fn gebunden() -> String {
        ziele_text(&[ziel("a", "a.b", "\"a.b\"", "GET", "durchreichen")])
    }

    fn ziele_text(ziele: &[String]) -> String {
        liste(ziele)
    }

    #[test]
    fn a_list_of_exactly_65536_bytes_is_read() {
        let mut text = gebunden().into_bytes();
        let fuellen = 65_536 - text.len();
        text.resize(text.len() + fuellen, b' ');
        assert_eq!(text.len(), 65_536);
        assert!(lies(&text).is_ok());
        text.push(b' ');
        assert_eq!(lies(&text).err(), Some("zl.zu_gross"));
    }

    #[test]
    fn a_bound_target_carries_the_host_as_the_name_of_the_holder() {
        let gut = lies(gebunden().as_bytes()).expect("list");
        assert_eq!(gut.ziele[0].halter_ziel.as_deref(), Some("a.b"));
        assert!(gut.ziele[0].gebunden());
        let ungebunden = liste(&[ziel("a", "a.b", "null", "POST", "ablehnen")]);
        let ungebunden = lies(ungebunden.as_bytes()).expect("list");
        assert_eq!(ungebunden.ziele[0].halter_ziel, None);
        assert!(!ungebunden.ziele[0].gebunden());
        // Stage 12: another name than the host, in either direction.
        for falsch in ["\"c.d\"", "\"a.b.\"", "\"A.B\"", "\"\"", "\"a.b:443\""] {
            let t = liste(&[ziel("a", "a.b", falsch, "GET", "ablehnen")]);
            assert_eq!(lies(t.as_bytes()).err(), Some("zl.bindung"), "{falsch}");
        }
    }

    #[test]
    fn a_field_that_fell_away_names_the_binding_and_not_an_unknown_key() {
        for feld in ["schluessel_datei", "kopf", "wert_vorsatz"] {
            let t = gebunden().replace(
                r#""halter_ziel":"a.b""#,
                &format!(r#""halter_ziel":"a.b","{feld}":"x""#),
            );
            assert_eq!(lies(t.as_bytes()).err(), Some("zl.bindung"), "{feld}");
            let null = gebunden().replace(
                r#""halter_ziel":"a.b""#,
                &format!(r#""halter_ziel":"a.b","{feld}":null"#),
            );
            assert_eq!(lies(null.as_bytes()).err(), Some("zl.bindung"), "{feld}");
        }
        // A key that never stood in the form stays stage 3.
        let t = gebunden().replace(r#""halter_ziel":"a.b""#, r#""halter_ziel":"a.b","x":1"#);
        assert_eq!(lies(t.as_bytes()).err(), Some("zl.schluessel_unbekannt"));
        // A list without `halter_ziel` misses a required key (stage 4), whatever else it carries.
        let t = gebunden().replace(r#""halter_ziel":"a.b","#, r#""schluessel_datei":"/k","#);
        assert_eq!(lies(t.as_bytes()).err(), Some("zl.schluessel_fehlt"));
    }

    #[test]
    fn a_rule_of_a_bound_target_names_only_get_and_post() {
        for gut in ["GET", "POST", "*"] {
            let t = liste(&[ziel("a", "a.b", "\"a.b\"", gut, "ablehnen")]);
            assert!(lies(t.as_bytes()).is_ok(), "{gut}");
        }
        for falsch in ["HEAD", "PUT", "PATCH", "DELETE", "OPTIONS"] {
            let t = liste(&[ziel("a", "a.b", "\"a.b\"", falsch, "ablehnen")]);
            assert_eq!(lies(t.as_bytes()).err(), Some("zl.regel_form"), "{falsch}");
            // An unbound target keeps every one of the seven methods.
            let frei = liste(&[ziel("a", "a.b", "null", falsch, "ablehnen")]);
            assert!(lies(frei.as_bytes()).is_ok(), "{falsch}");
        }
    }

    #[test]
    fn a_name_of_the_holder_stands_only_once() {
        let t = liste(&[
            ziel("a", "a.b", "\"a.b\"", "GET", "ablehnen"),
            ziel("b", "a.b", "\"a.b\"", "GET", "ablehnen"),
        ]);
        // Host and port are doubled here as well; both are stage 15.
        assert_eq!(lies(t.as_bytes()).err(), Some("zl.doppelt"));
        let zwei = liste(&[
            ziel("a", "a.b", "null", "GET", "ablehnen"),
            ziel("b", "c.d", "null", "GET", "ablehnen"),
        ]);
        assert!(lies(zwei.as_bytes()).is_ok());
    }

    #[test]
    fn only_a_bound_target_hands_an_upgrade_through() {
        let t = liste(&[ziel("a", "a.b", "null", "GET", "durchreichen")]);
        assert_eq!(lies(t.as_bytes()).err(), Some("zl.upgrade_wert"));
        let t = liste(&[ziel("a", "a.b", "\"a.b\"", "GET", "ja")]);
        assert_eq!(lies(t.as_bytes()).err(), Some("zl.upgrade_wert"));
    }

    #[test]
    fn host_labels_with_hyphens() {
        assert!(host_ok("a-b.c"));
        assert!(host_ok("a.b-c"));
        assert!(!host_ok("-a.b"));
        assert!(!host_ok("a-.b"));
        assert!(!host_ok("a.b-"));
        assert!(!host_ok("a_b.c"));
        assert!(!host_ok("a..b"));
        assert!(!host_ok(&format!("{}.b", "a".repeat(64))));
        assert!(host_ok(&format!("{}.b", "a".repeat(63))));
    }

    #[test]
    fn rules_cover_their_paths() {
        let r = |p: &str| Regel {
            methode: "GET".to_owned(),
            pfad: p.to_owned(),
        };
        assert!(r("/*").passt("/x/y"));
        assert!(r("/a/*").passt("/a/b"));
        assert!(r("/a/*").passt("/a/"));
        assert!(!r("/a/*").passt("/a"));
        assert!(!r("/a/*").passt("/ab"));
        assert!(r("/a").passt("/a"));
        assert!(!r("/a").passt("/a/x"));
        assert!(r("/a").erlaubt("GET"));
        assert!(!r("/a").erlaubt("POST"));
        let alle = Regel {
            methode: "*".to_owned(),
            pfad: "/*".to_owned(),
        };
        assert!(alle.erlaubt("OPTIONS"));
        assert!(!alle.erlaubt("TRACE"));
    }

    #[test]
    fn rule_paths_and_hosts_have_a_form() {
        for gut in ["/*", "/a", "/a/*", "/a.b/~c-_/*"] {
            assert!(regel_pfad_ok(gut), "{gut}");
        }
        for falsch in [
            "", "/", "*", "a", "/a/", "/a/*/b", "/./a", "/a/..", "//a", "/a b", "/*/*",
        ] {
            assert!(!regel_pfad_ok(falsch), "{falsch}");
        }
        assert!(host_ok("api.github.com"));
        assert!(!host_ok("localhost"));
        assert!(!host_ok("140.82.112.6"));
        assert!(host_ok("1.a"));
        assert!(!host_ok("Api.github.com"));
        assert!(!host_ok("*.github.com"));
        let lang = format!(
            "{}.{}",
            "a".repeat(63),
            ["b".repeat(63), "c".repeat(63), "d".repeat(61)].join(".")
        );
        assert_eq!(lang.len(), 253);
        assert!(host_ok(&lang));
        assert!(!host_ok(&format!("{lang}e")));
    }
}
