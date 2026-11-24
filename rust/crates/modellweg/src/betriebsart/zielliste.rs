//! The target list of mode 2 `kopf-einsetzen` (contract 8, section 12): a session file of "Lauf starten", JSON, at most
//! 65536 bytes, closed form on every level, read in 15 stages. The first failing stage names the code (`zl.…`).

use serde_json::{Map, Value};

use crate::aufruf::{PFAD_MAX, kennung_ok, pfad_ok};
use crate::json::lies_streng;

/// Largest target list in bytes.
pub const ZL_MAX: usize = 65_536;
/// The known methods of a rule (besides `*`).
pub const METHODEN: [&str; 7] = ["GET", "HEAD", "POST", "PUT", "PATCH", "DELETE", "OPTIONS"];
const OBEN: [&str; 3] = ["fassung", "ca_zulassung", "ziele"];
const ZIEL: [&str; 8] = [
    "name",
    "host",
    "port",
    "schluessel_datei",
    "kopf",
    "wert_vorsatz",
    "regeln",
    "upgrade",
];
const REGEL: [&str; 2] = ["methode", "pfad"];
/// Header names a bound target must not use for its key.
const KOPF_VERBOTEN: [&str; 10] = [
    "host",
    "connection",
    "content-length",
    "transfer-encoding",
    "te",
    "trailer",
    "upgrade",
    "keep-alive",
    "proxy-connection",
    "accept-encoding",
];
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

/// The key of a bound target: where it is and how it goes out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bindung {
    /// The key file (section 5).
    pub schluessel_datei: String,
    /// The lower-case name of the header with the key.
    pub kopf: String,
    /// The text before the key.
    pub wert_vorsatz: String,
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
    /// The key, for a bound target.
    pub bindung: Option<Bindung>,
    /// The rules (1 to 32).
    pub regeln: Vec<Regel>,
    /// Whether an upgrade (WebSocket) is handed through.
    pub upgrade_durchreichen: bool,
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
        .any(|(o, form)| o.keys().any(|k| !form.contains(&k.as_str())))
    {
        return Err("zl.schluessel_unbekannt");
    }
    if ebenen
        .iter()
        .any(|(o, form)| form.iter().any(|k| !o.contains_key(*k)))
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
        let doppelt = ziele[..i]
            .iter()
            .any(|a| a.name == z.name || (a.host == z.host && a.port == z.port));
        if doppelt {
            return Err("zl.doppelt");
        }
    }
    Ok(Zielliste { zulassung, ziele })
}

/// The objects whose keys are checked, with their form: the top, each target object, each rule object.
fn ebenen(oben: &Map<String, Value>) -> Vec<(&Map<String, Value>, &'static [&'static str])> {
    let mut v: Vec<(&Map<String, Value>, &'static [&'static str])> = vec![(oben, &OBEN)];
    for z in oben
        .get("ziele")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let Value::Object(z) = z else { continue };
        v.push((z, &ZIEL));
        for r in z
            .get("regeln")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            if let Value::Object(r) = r {
                v.push((r, &REGEL));
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
    bindung: [Option<&'a str>; 3],
    regeln: Vec<(&'a str, &'a str)>,
}

impl Roh<'_> {
    fn ziel(self) -> Ziel {
        let [datei, kopf, vorsatz] = self.bindung;
        let bindung = match (datei, kopf, vorsatz) {
            (Some(d), Some(k), Some(v)) => Some(Bindung {
                schluessel_datei: d.to_owned(),
                kopf: k.to_owned(),
                wert_vorsatz: v.to_owned(),
            }),
            _ => None,
        };
        Ziel {
            name: self.name.to_owned(),
            host: self.host.to_owned(),
            port: u16::try_from(self.port).unwrap_or(0),
            bindung,
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
            bindung: [
                text_oder_null("schluessel_datei")?,
                text_oder_null("kopf")?,
                text_oder_null("wert_vorsatz")?,
            ],
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
    let regeln_gut = ziele
        .iter()
        .flat_map(|z| z.regeln.iter())
        .all(|(m, p)| (*m == "*" || METHODEN.contains(m)) && regel_pfad_ok(p));
    if !regeln_gut {
        return Err("zl.regel_form");
    }
    if !ziele.iter().all(|z| bindung_ok(&z.bindung)) {
        return Err("zl.bindung");
    }
    let upgrade_gut = ziele.iter().all(|z| match z.upgrade {
        "ablehnen" => true,
        "durchreichen" => z.bindung[1].is_some(),
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

/// All three set (valid forms) or all three null.
fn bindung_ok(b: &[Option<&str>; 3]) -> bool {
    match *b {
        [None, None, None] => true,
        [Some(datei), Some(kopf), Some(vorsatz)] => {
            let kopf_gut = (1..=64).contains(&kopf.len())
                && kopf
                    .bytes()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-')
                && !KOPF_VERBOTEN.contains(&kopf);
            let vorsatz_gut =
                vorsatz.len() <= 32 && vorsatz.bytes().all(|c| (0x20..=0x7e).contains(&c));
            kopf_gut && vorsatz_gut && pfad_ok(datei, PFAD_MAX)
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::{Regel, bindung_ok, host_ok, lies, regel_pfad_ok};

    #[test]
    fn a_list_of_exactly_65536_bytes_is_read() {
        let mut text = br#"{"fassung":1,"ca_zulassung":"zugelassen","ziele":[{"name":"a","host":"a.b","port":443,"schluessel_datei":null,"kopf":null,"wert_vorsatz":null,"regeln":[{"methode":"GET","pfad":"/*"}],"upgrade":"ablehnen"}]}"#.to_vec();
        text.resize(65_536, b' ');
        assert!(lies(&text).is_ok());
        text.push(b' ');
        assert_eq!(lies(&text).err(), Some("zl.zu_gross"));
    }

    #[test]
    fn the_binding_of_a_target() {
        assert!(bindung_ok(&[None, None, None]));
        assert!(bindung_ok(&[
            Some("/k"),
            Some("x-api-key2"),
            Some("Bearer ")
        ]));
        assert!(bindung_ok(&[Some("/k"), Some("a"), Some("")]));
        assert!(!bindung_ok(&[Some("/k"), Some("X-Api"), Some("")]));
        assert!(!bindung_ok(&[Some("/k"), Some("a_b"), Some("")]));
        assert!(!bindung_ok(&[
            Some("/k"),
            Some("accept-encoding"),
            Some("")
        ]));
        assert!(!bindung_ok(&[Some("/k"), Some(""), Some("")]));
        assert!(bindung_ok(&[Some("/k"), Some(&"a".repeat(64)), Some("")]));
        assert!(!bindung_ok(&[Some("/k"), Some(&"a".repeat(65)), Some("")]));
        assert!(bindung_ok(&[Some("/k"), Some("a"), Some(&" ".repeat(32))]));
        assert!(!bindung_ok(&[Some("/k"), Some("a"), Some(&" ".repeat(33))]));
        assert!(!bindung_ok(&[Some("/k"), Some("a"), Some("\t")]));
        assert!(!bindung_ok(&[Some("k"), Some("a"), Some("")]));
        assert!(!bindung_ok(&[Some("/k"), None, None]));
        assert!(!bindung_ok(&[None, Some("a"), Some("")]));
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
