//! `halter.json` (step 2 of the start): readable, strict JSON, closed form, no local server with a token on Linux and
//! macOS, and the role groups of section 2.3a resolved in `/etc/group` (contract section 2.3a, schema `$defs/konfig`):
//!
//! ```json
//! {"fassung":1,
//!  "konten":{"1000":{"person":null}},
//!  "rollen":{"bruecke":"wb-bruecke"},
//!  "ziele":{"copilot":{"host":"api.example.invalid","port":443,"tls":true,"praefix":"",
//!           "kopf":"authorization","vorsatz":"Bearer ","schluessel":"anmeldung.copilot","pfade":["GET /*"]}},
//!  "server":{},
//!  "kids":{}}
//! ```
//!
//! `server` entries carry `host`, `port`, `tls`, `pfad`, `kopf`, `vorsatz`, `schluessel` (`mcp.*`), `freigabe`,
//! `werkzeuge` (`lesend` or `schreibend`) and `lokal`; a local server (`lokal: true`) holds its token in its environment
//! and is refused under Linux and macOS (section 2.1). `tls: false` is allowed only for a loopback host.

use std::collections::BTreeMap;

use serde_json::{Map, Value};

use crate::b64;
use crate::form::{self, ist_kennung, ist_name};
use crate::json;
use crate::pruefung::{Klasse, Rolle, ServerRegel, ZielRegel};

/// Largest `halter.json` in bytes.
pub const GROESSE_MAX: usize = 1 << 20;

/// Address and key head of a target or server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Adresse {
    /// Host name or loopback address.
    pub host: String,
    /// TCP port.
    pub port: u16,
    /// TLS with the truststore of the system.
    pub tls: bool,
    /// Head name of the key, `^[a-z0-9-]{1,64}$`.
    pub kopf: String,
    /// Text before the value in that head, printable ASCII.
    pub vorsatz: String,
}

/// A model target.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ziel {
    /// Address.
    pub adresse: Adresse,
    /// Path prefix before every path of a request (`""` or a path without `/` at the end).
    pub praefix: String,
    /// Secret and path rules.
    pub regel: ZielRegel,
}

/// A remote MCP server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Server {
    /// Address.
    pub adresse: Adresse,
    /// Path of the endpoint.
    pub pfad: String,
    /// Secret, release option and classes.
    pub regel: ServerRegel,
}

/// The configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Konfig {
    /// Allowed accounts with the pseudonym of their person (`None` if none is known).
    pub konten: BTreeMap<String, Option<String>>,
    /// Role groups: role to the name of its group (section 2.3a); two roles never share a group.
    pub rollen: BTreeMap<Rolle, String>,
    /// Model targets.
    pub ziele: BTreeMap<String, Ziel>,
    /// MCP servers.
    pub server: BTreeMap<String, Server>,
    /// Public keys of the decision point per `kid`.
    pub kids: BTreeMap<String, [u8; 32]>,
}

fn obj<'a>(v: &'a Value, felder: &[&str]) -> Option<&'a Map<String, Value>> {
    let o = v.as_object()?;
    (o.len() == felder.len() && felder.iter().all(|f| o.contains_key(*f))).then_some(o)
}

fn text<'a>(o: &'a Map<String, Value>, k: &str) -> Option<&'a str> {
    o.get(k)?.as_str()
}

fn druckbar(t: &str, max: usize) -> bool {
    t.len() <= max && t.bytes().all(|c| (0x20..=0x7e).contains(&c))
}

fn kopfname(t: &str) -> bool {
    (1..=64).contains(&t.len())
        && t.bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-')
}

fn loopback(host: &str) -> bool {
    matches!(host, "127.0.0.1" | "::1" | "localhost")
}

fn adresse(o: &Map<String, Value>) -> Option<Adresse> {
    let host = text(o, "host")?;
    let host_ok = (1..=253).contains(&host.len())
        && host
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'.' | b'-' | b':'));
    let port = u16::try_from(o.get("port")?.as_u64()?)
        .ok()
        .filter(|p| *p > 0)?;
    let tls = o.get("tls")?.as_bool()?;
    let kopf = text(o, "kopf")?;
    let vorsatz = text(o, "vorsatz")?;
    let kopf_ok = kopfname(kopf)
        && (matches!(kopf, "authorization" | "x-api-key" | "api-key")
            || !form::KOPF_VERBOTEN.contains(&kopf));
    (host_ok && (tls || loopback(host)) && kopf_ok && druckbar(vorsatz, 64)).then(|| Adresse {
        host: host.to_owned(),
        port,
        tls,
        kopf: kopf.to_owned(),
        vorsatz: vorsatz.to_owned(),
    })
}

fn regel_ok(r: &str) -> bool {
    let Some((m, p)) = r.split_once(' ') else {
        return false;
    };
    if !matches!(m, "GET" | "POST") || !p.starts_with('/') || p.len() > 512 {
        return false;
    }
    let segmente: Vec<&str> = p[1..].split('/').collect();
    segmente.iter().enumerate().all(|(i, s)| {
        if *s == "*" {
            return i + 1 == segmente.len();
        }
        let letzter = i + 1 == segmente.len();
        (letzter && s.is_empty())
            || (!s.is_empty()
                && *s != "."
                && *s != ".."
                && s.bytes()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'.' | b'_' | b'~' | b'-')))
    })
}

fn ziel(v: &Value) -> Option<Ziel> {
    let o = obj(
        v,
        &[
            "host",
            "port",
            "tls",
            "praefix",
            "kopf",
            "vorsatz",
            "schluessel",
            "pfade",
        ],
    )?;
    let praefix = text(o, "praefix")?;
    let praefix_ok = praefix.is_empty()
        || (form::pfad_form(praefix)
            && !praefix.ends_with('/')
            && !crate::pruefung::pfad_unsicher(praefix));
    let schluessel = text(o, "schluessel")?;
    let art_ok = schluessel.starts_with("modell.") || schluessel.starts_with("anmeldung.");
    let pfade: Vec<String> = o
        .get("pfade")?
        .as_array()?
        .iter()
        .map(|r| r.as_str().filter(|r| regel_ok(r)).map(str::to_owned))
        .collect::<Option<_>>()?;
    if !(praefix_ok && ist_name(schluessel) && art_ok && (1..=32).contains(&pfade.len())) {
        return None;
    }
    Some(Ziel {
        adresse: adresse(o)?,
        praefix: praefix.to_owned(),
        regel: ZielRegel {
            schluessel: schluessel.to_owned(),
            pfade,
        },
    })
}

/// A failure of `halter.json`: [`Fehler::Lokal`] is the case of a local server with a token (still exit 78).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fehler {
    /// Not valid.
    Form,
    /// A local server with a token, not allowed on Linux and macOS.
    Lokal,
}

fn server(v: &Value) -> Result<Server, Fehler> {
    let o = obj(
        v,
        &[
            "host",
            "port",
            "tls",
            "pfad",
            "kopf",
            "vorsatz",
            "schluessel",
            "freigabe",
            "werkzeuge",
            "lokal",
        ],
    )
    .ok_or(Fehler::Form)?;
    if o.get("lokal")
        .and_then(Value::as_bool)
        .ok_or(Fehler::Form)?
    {
        return Err(Fehler::Lokal);
    }
    let pfad = text(o, "pfad").ok_or(Fehler::Form)?;
    let schluessel = text(o, "schluessel").ok_or(Fehler::Form)?;
    let freigabe = o
        .get("freigabe")
        .and_then(Value::as_bool)
        .ok_or(Fehler::Form)?;
    let mut werkzeuge = BTreeMap::new();
    for (w, k) in o
        .get("werkzeuge")
        .and_then(Value::as_object)
        .ok_or(Fehler::Form)?
    {
        let k = match k.as_str() {
            Some("lesend") => Klasse::Lesend,
            Some("schreibend") => Klasse::Schreibend,
            _ => return Err(Fehler::Form),
        };
        werkzeuge.insert(w.clone(), k);
    }
    if !form::pfad_form(pfad)
        || crate::pruefung::pfad_unsicher(pfad)
        || !ist_name(schluessel)
        || !schluessel.starts_with("mcp.")
    {
        return Err(Fehler::Form);
    }
    Ok(Server {
        adresse: adresse(o).ok_or(Fehler::Form)?,
        pfad: pfad.to_owned(),
        regel: ServerRegel {
            schluessel: schluessel.to_owned(),
            freigabe,
            werkzeuge,
        },
    })
}

/// A group name as section 2.3a allows it: `^[a-z_][a-z0-9_-]{0,31}$`.
#[must_use]
pub fn gruppenname(g: &str) -> bool {
    let b = g.as_bytes();
    (1..=32).contains(&b.len())
        && (b[0].is_ascii_lowercase() || b[0] == b'_')
        && b.iter()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, b'_' | b'-'))
}

/// The password fields of `/etc/group` that let nobody take the group with `newgrp` without membership.
const GRUPPE_PASSWORT_OK: [&str; 4] = ["x", "!", "*", ""];

/// The lines of a file in the form of `/etc/group` or `/etc/passwd` with exactly `n` fields; other lines do not count.
fn felder(text: &str, n: usize) -> impl Iterator<Item = Vec<&str>> {
    text.split('\n')
        .filter(|z| !z.is_empty())
        .map(|z| z.split(':').collect::<Vec<_>>())
        .filter(move |f| f.len() == n)
}

/// Section 2.3a, the role groups at the start: each group of `rollen` stands exactly once in `group`, its gid is a
/// number other than 0, no two roles share a gid, the group has no member and no password, and it is the primary group
/// of no account of `konten` in `passwd`. The result maps the gid to the role.
///
/// # Errors
/// [`Fehler::Form`] for the first broken rule.
pub fn rollengruppen(
    k: &Konfig,
    group: &str,
    passwd: &str,
) -> Result<BTreeMap<u32, Rolle>, Fehler> {
    let mut gids = BTreeMap::new();
    for (rolle, name) in &k.rollen {
        let mut treffer = felder(group, 4).filter(|f| f[0] == name);
        let (Some(f), None) = (treffer.next(), treffer.next()) else {
            return Err(Fehler::Form);
        };
        let gid = Some(f[2])
            .filter(|g| !g.is_empty() && g.bytes().all(|c| c.is_ascii_digit()))
            .and_then(|g| g.parse::<u32>().ok())
            .filter(|g| *g != 0)
            .ok_or(Fehler::Form)?;
        // every line of an account of konten counts, also a second line of the same uid
        let hauptgruppe = felder(passwd, 7)
            .any(|f| k.konten.contains_key(f[2]) && f[3].parse::<u32>().ok() == Some(gid));
        if !f[3].is_empty()
            || !GRUPPE_PASSWORT_OK.contains(&f[1])
            || hauptgruppe
            || gids.insert(gid, *rolle).is_some()
        {
            return Err(Fehler::Form);
        }
    }
    Ok(gids)
}

/// Step 2 of the start without the file checks: the bytes of `halter.json` are valid, the own uid stands in `konten`,
/// and the role groups resolve after section 2.3a against the texts of `/etc/group` and `/etc/passwd`.
///
/// # Errors
/// [`Fehler`] of [`lesen`], else [`Fehler::Form`].
pub fn fuer_start(
    bytes: &[u8],
    eigene_uid: u32,
    group: &str,
    passwd: &str,
) -> Result<(Konfig, BTreeMap<u32, Rolle>), Fehler> {
    let k = lesen(bytes)?;
    if !k.konten.contains_key(&eigene_uid.to_string()) {
        return Err(Fehler::Form);
    }
    let gruppen = rollengruppen(&k, group, passwd)?;
    Ok((k, gruppen))
}

/// Reads the bytes of `halter.json`.
///
/// # Errors
/// [`Fehler`] for any break of the closed form.
pub fn lesen(bytes: &[u8]) -> Result<Konfig, Fehler> {
    if bytes.len() > GROESSE_MAX {
        return Err(Fehler::Form);
    }
    let wert = json::streng_lesen(bytes).ok_or(Fehler::Form)?;
    let o = obj(
        &wert,
        &["fassung", "konten", "rollen", "ziele", "server", "kids"],
    )
    .ok_or(Fehler::Form)?;
    if o.get("fassung").and_then(Value::as_u64) != Some(1) || !o["fassung"].is_u64() {
        return Err(Fehler::Form);
    }
    let mut konten = BTreeMap::new();
    for (k, v) in o["konten"].as_object().ok_or(Fehler::Form)? {
        let e = obj(v, &["person"]).ok_or(Fehler::Form)?;
        let person = match &e["person"] {
            Value::Null => None,
            Value::String(p)
                if p.strip_prefix("ps_").is_some_and(|h| {
                    h.len() == 32
                        && h.bytes()
                            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
                }) =>
            {
                Some(p.clone())
            }
            _ => return Err(Fehler::Form),
        };
        // at most 10 digits follows from the rest: without a leading zero, 11 digits are above u32::MAX (10 digits),
        // so a rule of its own could never refuse a key the others let pass
        if k.is_empty()
            || !k.bytes().all(|c| c.is_ascii_digit())
            || (k.len() > 1 && k.starts_with('0'))
            || k.parse::<u32>().is_err()
        {
            return Err(Fehler::Form);
        }
        konten.insert(k.clone(), person);
    }
    let mut rollen = BTreeMap::new();
    for (r, gruppe) in o["rollen"].as_object().ok_or(Fehler::Form)? {
        let rolle = Rolle::aus_text(r).ok_or(Fehler::Form)?;
        let gruppe = gruppe
            .as_str()
            .filter(|g| gruppenname(g))
            .ok_or(Fehler::Form)?;
        if rollen.values().any(|g| g == gruppe) {
            return Err(Fehler::Form);
        }
        rollen.insert(rolle, gruppe.to_owned());
    }
    let mut ziele = BTreeMap::new();
    for (n, v) in o["ziele"].as_object().ok_or(Fehler::Form)? {
        if !ist_kennung(n) {
            return Err(Fehler::Form);
        }
        ziele.insert(n.clone(), ziel(v).ok_or(Fehler::Form)?);
    }
    let mut alle_server = BTreeMap::new();
    let mut lokal = false;
    for (n, v) in o["server"].as_object().ok_or(Fehler::Form)? {
        if !ist_kennung(n) {
            return Err(Fehler::Form);
        }
        match server(v) {
            Ok(s) => {
                alle_server.insert(n.clone(), s);
            }
            Err(Fehler::Lokal) => lokal = true,
            Err(e) => return Err(e),
        }
    }
    let mut kids = BTreeMap::new();
    for (k, v) in o["kids"].as_object().ok_or(Fehler::Form)? {
        let b = v.as_str().and_then(b64::dekodieren).ok_or(Fehler::Form)?;
        if !ist_kennung(k) {
            return Err(Fehler::Form);
        }
        kids.insert(
            k.clone(),
            <[u8; 32]>::try_from(b).map_err(|_| Fehler::Form)?,
        );
    }
    if lokal {
        return Err(Fehler::Lokal);
    }
    Ok(Konfig {
        konten,
        rollen,
        ziele,
        server: alle_server,
        kids,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const GUT: &str = r#"{"fassung":1,"konten":{"1000":{"person":null}},"rollen":{"bruecke":"wb-bruecke"},"ziele":{"copilot":{"host":"api.example.invalid","port":443,"tls":true,"praefix":"","kopf":"authorization","vorsatz":"Bearer ","schluessel":"anmeldung.copilot","pfade":["GET /*","GET /copilot_internal/*","POST /chat/completions"]}},"server":{},"kids":{}}"#;

    #[test]
    fn gute_konfig() {
        let k = lesen(GUT.as_bytes()).expect("valid");
        assert_eq!(
            k.rollen.get(&Rolle::Bruecke).map(String::as_str),
            Some("wb-bruecke")
        );
        assert_eq!(k.ziele["copilot"].regel.pfade.len(), 3);
    }

    #[test]
    fn falsche_konfig() {
        for (alt, neu) in [
            (r#""fassung":1"#, r#""fassung":2"#),
            (r#""tls":true"#, r#""tls":false"#),
            (r#""GET /*","#, r#""GET /a/../b","#),
            (r#""GET /*","#, r#""GET /*/x","#),
            (r#""kids":{}"#, r#""kids":{},"zusatz":1"#),
            (r#""anmeldung.copilot""#, r#""mcp.copilot""#),
            (r#""1000""#, r#""-1""#),
            (r#""bruecke""#, r#""agent""#),
            (r#""kopf":"authorization""#, r#""kopf":"cookie""#),
        ] {
            let t = GUT.replacen(alt, neu, 1);
            assert_ne!(t, GUT);
            assert_eq!(lesen(t.as_bytes()), Err(Fehler::Form), "{neu}");
        }
        let doppelt = GUT.replacen(r#""server":{}"#, r#""server":{},"server":{}"#, 1);
        assert_eq!(lesen(doppelt.as_bytes()), Err(Fehler::Form));
    }

    fn mit(alt: &str, neu: &str) -> Result<Konfig, Fehler> {
        let t = GUT.replacen(alt, neu, 1);
        assert_ne!(t, GUT, "{alt}");
        lesen(t.as_bytes())
    }

    const SERVER: &str = r#""server":{"x":{"host":"mcp.example.invalid","port":443,"tls":true,"pfad":"/mcp","kopf":"authorization","vorsatz":"Bearer ","schluessel":"mcp.x","freigabe":true,"werkzeuge":{"a":"lesend","b":"schreibend"},"lokal":false}}"#;

    #[test]
    fn jede_grenze_einzeln() {
        let gut = |alt: &str, neu: &str| assert!(mit(alt, neu).is_ok(), "{neu}");
        let schlecht = |alt: &str, neu: &str| assert_eq!(mit(alt, neu), Err(Fehler::Form), "{neu}");
        // the uid of an account: digits only; `+5` would pass `parse::<u32>` alone
        schlecht(r#""konten":{"1000""#, r#""konten":{"+1000""#);
        schlecht(r#""konten":{"1000""#, r#""konten":{"+5""#);
        gut(r#""port":443"#, r#""port":65535"#);
        gut(r#""port":443"#, r#""port":1"#);
        schlecht(r#""port":443"#, r#""port":0"#);
        schlecht(r#""port":443"#, r#""port":65536"#);
        schlecht(r#""port":443"#, r#""port":"443""#);
        gut(r#""host":"api.example.invalid""#, r#""host":"::1""#);
        schlecht(r#""host":"api.example.invalid""#, r#""host":"a b""#);
        schlecht(r#""host":"api.example.invalid""#, r#""host":"a/b""#);
        schlecht(r#""host":"api.example.invalid""#, r#""host":"""#);
        schlecht(
            r#""host":"api.example.invalid""#,
            &format!(r#""host":"{}""#, "a".repeat(254)),
        );
        gut(
            r#""host":"api.example.invalid""#,
            &format!(r#""host":"{}""#, "a".repeat(253)),
        );
        gut(
            r#""host":"api.example.invalid","port":443,"tls":true"#,
            r#""host":"127.0.0.1","port":443,"tls":false"#,
        );
        gut(r#""kopf":"authorization""#, r#""kopf":"x-api-key""#);
        gut(r#""kopf":"authorization""#, r#""kopf":"x-token""#);
        schlecht(r#""kopf":"authorization""#, r#""kopf":"X-Token""#);
        schlecht(r#""kopf":"authorization""#, r#""kopf":"""#);
        schlecht(
            r#""kopf":"authorization""#,
            &format!(r#""kopf":"{}""#, "a".repeat(65)),
        );
        schlecht(r#""kopf":"authorization""#, r#""kopf":"host""#);
        gut(
            r#""vorsatz":"Bearer ""#,
            &format!(r#""vorsatz":"{}""#, "v".repeat(64)),
        );
        schlecht(
            r#""vorsatz":"Bearer ""#,
            &format!(r#""vorsatz":"{}""#, "v".repeat(65)),
        );
        schlecht(r#""vorsatz":"Bearer ""#, r#""vorsatz":"a""#);
        gut(r#""praefix":"""#, r#""praefix":"/api/v1""#);
        schlecht(r#""praefix":"""#, r#""praefix":"/api/""#);
        schlecht(r#""praefix":"""#, r#""praefix":"api""#);
        schlecht(r#""praefix":"""#, r#""praefix":"/a/../b""#);
        schlecht(r#""anmeldung.copilot""#, r#""anmeldung.Copilot""#);
        gut(r#""anmeldung.copilot""#, r#""modell.copilot""#);
        gut(r#""GET /*","#, r#""GET /a/","#);
        gut(r#""GET /*","#, r#""POST /a/b-c.d_e~f","#);
        schlecht(r#""GET /*","#, r#""PUT /a","#);
        schlecht(r#""GET /*","#, r#""GET a","#);
        schlecht(r#""GET /*","#, r#""GET /a b","#);
        schlecht(r#""GET /*","#, r#""GET /a/./b","#);
        schlecht(r#""GET /*","#, r#""GET /a//b","#);
        schlecht(r#""GET /*","#, r#""GET /a%2e","#);
        schlecht(r#""GET /*","#, &format!(r#""GET /{}","#, "a".repeat(512)));
        gut(r#""GET /*","#, &format!(r#""GET /{}","#, "a".repeat(511)));
        let viele: Vec<String> = (0..32).map(|i| format!(r#""GET /p{i}""#)).collect();
        gut(
            r#""pfade":["GET /*","GET /copilot_internal/*","POST /chat/completions"]"#,
            &format!(r#""pfade":[{}]"#, viele.join(",")),
        );
        let zu_viele: Vec<String> = (0..33).map(|i| format!(r#""GET /p{i}""#)).collect();
        schlecht(
            r#""pfade":["GET /*","GET /copilot_internal/*","POST /chat/completions"]"#,
            &format!(r#""pfade":[{}]"#, zu_viele.join(",")),
        );
        schlecht(
            r#""pfade":["GET /*","GET /copilot_internal/*","POST /chat/completions"]"#,
            r#""pfade":[]"#,
        );
        schlecht(r#""copilot":{"#, r#""Copilot":{"#);
        // accounts and persons
        gut(
            r#""1000":{"person":null}"#,
            &format!(r#""4294967295":{{"person":"ps_{}"}}"#, "a1".repeat(16)),
        );
        schlecht(
            r#""1000":{"person":null}"#,
            r#""4294967296":{"person":null}"#,
        );
        schlecht(
            r#""1000":{"person":null}"#,
            r#""12345678901":{"person":null}"#,
        );
        schlecht(r#""1000":{"person":null}"#, r#""":{"person":null}"#);
        schlecht(
            r#""1000":{"person":null}"#,
            &format!(r#""1000":{{"person":"ps_{}"}}"#, "A1".repeat(16)),
        );
        schlecht(
            r#""1000":{"person":null}"#,
            &format!(r#""1000":{{"person":"ps_{}"}}"#, "a1".repeat(15)),
        );
        schlecht(r#""1000":{"person":null}"#, r#""1000":{"person":1}"#);
        // role groups (section 2.3a)
        gut(
            r#""bruecke":"wb-bruecke""#,
            r#""bruecke":"wb-bruecke","hub":"wb-hub","oberflaeche":"_wb_o-1""#,
        );
        gut(r#""wb-bruecke""#, &format!(r#""{}""#, "a".repeat(32)));
        schlecht(r#""wb-bruecke""#, &format!(r#""{}""#, "a".repeat(33)));
        schlecht(r#""wb-bruecke""#, r#""""#);
        schlecht(r#""wb-bruecke""#, r#""Wb-bruecke""#);
        schlecht(r#""wb-bruecke""#, r#""1wb""#);
        schlecht(r#""wb-bruecke""#, r#""-wb""#);
        schlecht(r#""wb-bruecke""#, r#""wb.bruecke""#);
        schlecht(r#""wb-bruecke""#, r#""wb bruecke""#);
        schlecht(
            r#""wb-bruecke""#,
            r#"["/usr/lib/werkbank/werkbank-modellweg"]"#,
        );
        schlecht(r#""wb-bruecke""#, "1");
        schlecht(
            r#""bruecke":"wb-bruecke""#,
            r#""bruecke":"wb-bruecke","hub":"wb-bruecke""#,
        );
        gut(r#""rollen":{"bruecke":"wb-bruecke"}"#, r#""rollen":{}"#);
        // accounts without a leading zero
        gut(r#""1000":{"person":null}"#, r#""0":{"person":null}"#);
        schlecht(r#""1000":{"person":null}"#, r#""01000":{"person":null}"#);
        schlecht(r#""1000":{"person":null}"#, r#""00":{"person":null}"#);
        schlecht(r#""1000":{"person":null}"#, r#""+1000":{"person":null}"#);
        // keys of the decision point
        gut(
            r#""kids":{}"#,
            r#""kids":{"stelle-mcp-1":"qvy5SgFhefK2a9hsceqk+MJ5evYpEJTTdGpqO3O85VY="}"#,
        );
        schlecht(
            r#""kids":{}"#,
            r#""kids":{"Stelle":"qvy5SgFhefK2a9hsceqk+MJ5evYpEJTTdGpqO3O85VY="}"#,
        );
        schlecht(r#""kids":{}"#, r#""kids":{"k":"AAAA"}"#);
        // servers
        gut(r#""server":{}"#, SERVER);
        let s =
            |alt: &str, neu: &str| GUT.replacen(r#""server":{}"#, &SERVER.replacen(alt, neu, 1), 1);
        for (alt, neu) in [
            (r#""pfad":"/mcp""#, r#""pfad":"mcp""#),
            (r#""pfad":"/mcp""#, r#""pfad":"/a/../b""#),
            (r#""schluessel":"mcp.x""#, r#""schluessel":"modell.x""#),
            (r#""schluessel":"mcp.x""#, r#""schluessel":"mcp.X""#),
            (r#""lesend""#, r#""lesen""#),
            (r#""freigabe":true"#, r#""freigabe":1"#),
            (r#""lokal":false"#, r#""lokal":0"#),
            (r#""tls":true"#, r#""tls":false"#),
        ] {
            assert_eq!(lesen(s(alt, neu).as_bytes()), Err(Fehler::Form), "{neu}");
        }
        let k = lesen(s("", "").as_bytes()).expect("server");
        assert_eq!(k.server["x"].regel.werkzeuge.len(), 2);
        assert!(k.server["x"].regel.freigabe);
        let gross = format!("{}{}", GUT, " ".repeat(GROESSE_MAX));
        assert_eq!(lesen(gross.as_bytes()), Err(Fehler::Form));
        let genau = format!("{}{}", GUT, " ".repeat(GROESSE_MAX - GUT.len()));
        assert!(lesen(genau.as_bytes()).is_ok());
    }

    #[test]
    fn lokaler_server_ist_eigener_fehler() {
        let s = r#""server":{"x":{"host":"127.0.0.1","port":9,"tls":false,"pfad":"/mcp","kopf":"authorization","vorsatz":"Bearer ","schluessel":"mcp.x","freigabe":true,"werkzeuge":{},"lokal":true}}"#;
        let t = GUT.replacen(r#""server":{}"#, s, 1);
        assert_eq!(lesen(t.as_bytes()), Err(Fehler::Lokal));
        let t = t.replacen(r#""lokal":true"#, r#""lokal":false"#, 1);
        assert!(lesen(t.as_bytes()).is_ok());
    }

    const GROUP: &str = "root:x:0:\nperson:x:1000:\nwb-hub:x:990:\nwb-bruecke:x:991:\n";
    const PASSWD: &str =
        "root:x:0:0:root:/root:/bin/sh\nperson:x:1000:1000::/srv/beispiel/person:/bin/sh\n";

    fn gruppen(rollen: &str, group: &str, passwd: &str) -> Result<BTreeMap<u32, Rolle>, Fehler> {
        let t = GUT.replacen(r#""rollen":{"bruecke":"wb-bruecke"}"#, rollen, 1);
        rollengruppen(&lesen(t.as_bytes()).expect("valid"), group, passwd)
    }

    #[test]
    fn rollengruppen_nach_den_fuenf_regeln() {
        let br = r#""rollen":{"bruecke":"wb-bruecke"}"#;
        assert_eq!(
            gruppen(br, GROUP, PASSWD),
            Ok(BTreeMap::from([(991, Rolle::Bruecke)]))
        );
        assert_eq!(
            gruppen(
                r#""rollen":{"bruecke":"wb-bruecke","hub":"wb-hub"}"#,
                GROUP,
                PASSWD
            ),
            Ok(BTreeMap::from([(990, Rolle::Hub), (991, Rolle::Bruecke)]))
        );
        assert_eq!(gruppen(r#""rollen":{}"#, "", ""), Ok(BTreeMap::new()));
        // passwords that let nobody in
        for pw in ["!", "*", ""] {
            let g = GROUP.replace("wb-bruecke:x:", &format!("wb-bruecke:{pw}:"));
            assert!(gruppen(br, &g, PASSWD).is_ok(), "{pw}");
        }
        // the primary group of an account outside konten does not count
        let p = format!("{PASSWD}dienst:x:1002:991::/srv/beispiel/d:/bin/sh\n");
        assert!(gruppen(br, GROUP, &p).is_ok());
        // lines of another form do not count
        assert!(gruppen(br, &format!("wb-bruecke:x\n{GROUP}"), PASSWD).is_ok());
        for (group, passwd, grund) in [
            (
                GROUP.replace("wb-bruecke:x:991:\n", ""),
                PASSWD.to_owned(),
                "missing",
            ),
            (
                format!("{GROUP}wb-bruecke:x:993:\n"),
                PASSWD.to_owned(),
                "twice",
            ),
            (GROUP.replace(":991:", ":0:"), PASSWD.to_owned(), "gid 0"),
            (
                GROUP.replace(":991:", ":99a:"),
                PASSWD.to_owned(),
                "gid no number",
            ),
            (GROUP.replace(":991:", "::"), PASSWD.to_owned(), "gid empty"),
            (
                GROUP.replace(":991:", ":4294967296:"),
                PASSWD.to_owned(),
                "gid too big",
            ),
            (
                GROUP.replace(":991:", ":+991:"),
                PASSWD.to_owned(),
                "gid with sign",
            ),
            (
                GROUP.replace(":991:", ":991:person"),
                PASSWD.to_owned(),
                "member",
            ),
            (
                GROUP.replace(":991:", ":991:anders"),
                PASSWD.to_owned(),
                "other member",
            ),
            (
                GROUP.replace("wb-bruecke:x:", "wb-bruecke:$6$a$b:"),
                PASSWD.to_owned(),
                "password",
            ),
            (
                GROUP.to_owned(),
                PASSWD.replace("1000:1000:", "1000:991:"),
                "primary group",
            ),
            (
                GROUP.to_owned(),
                format!("{PASSWD}person2:x:1000:991::/srv/beispiel/p:/bin/sh\n"),
                "primary group in a second line of the uid",
            ),
        ] {
            assert_eq!(gruppen(br, &group, &passwd), Err(Fehler::Form), "{grund}");
        }
        // two role groups with the same gid
        assert_eq!(
            gruppen(
                r#""rollen":{"bruecke":"wb-bruecke","hub":"wb-hub"}"#,
                &GROUP.replace(":990:", ":991:"),
                PASSWD
            ),
            Err(Fehler::Form)
        );
    }

    #[test]
    fn fuer_start_verlangt_die_eigene_uid() {
        assert!(fuer_start(GUT.as_bytes(), 1000, GROUP, PASSWD).is_ok());
        assert_eq!(
            fuer_start(GUT.as_bytes(), 1001, GROUP, PASSWD).map(|_| ()),
            Err(Fehler::Form)
        );
        assert_eq!(
            fuer_start(GUT.as_bytes(), 1000, "", PASSWD).map(|_| ()),
            Err(Fehler::Form)
        );
        assert_eq!(
            fuer_start(b"{}", 1000, GROUP, PASSWD).map(|_| ()),
            Err(Fehler::Form)
        );
    }
}
