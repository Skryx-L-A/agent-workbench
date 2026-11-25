//! Stages 8 to 12: the closed form of each request (schema `$defs/anfrage`). Every level is checked; the code is the one
//! of the lowest violated stage over the whole request, so an unknown key inside the ticket (stage 8) wins over a missing
//! field at the top (stage 9).

use serde_json::{Map, Value};

use crate::b64;
use crate::code::Code;

/// Most entries of `kopf`.
pub const KOPF_MAX: usize = 64;
/// Longest value of one head entry (characters).
pub const KOPFWERT_MAX: usize = 8192;
/// Longest `koerper_b64` (characters): Base64 of 24117248 bytes.
pub const KOERPER_B64_MAX: usize = 32_156_332;
/// Longest `daten_b64` of `strom.teil` (characters): Base64 of 65536 bytes.
pub const TEIL_B64_MAX: usize = 87_384;
/// Largest integer `id` (2^53 - 1).
pub const ID_MAX: u64 = 9_007_199_254_740_991;
/// Largest time of a ticket in ms (9999-12-31T23:59:59.999Z).
pub const MS_MAX: u64 = 253_402_300_799_999;

/// Head names the bridge must never send (section 4); the holder sets them itself or refuses them.
pub const KOPF_VERBOTEN: [&str; 20] = [
    "authorization",
    "proxy-authorization",
    "x-api-key",
    "api-key",
    "cookie",
    "set-cookie",
    "host",
    "content-length",
    "transfer-encoding",
    "connection",
    "upgrade",
    "te",
    "trailer",
    "keep-alive",
    "proxy-connection",
    "forwarded",
    "x-forwarded-for",
    "x-forwarded-host",
    "accept-encoding",
    "sec-websocket-extensions",
];

/// The operations of the table in section 2.1.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op {
    /// `schluessel.vorhanden`
    Vorhanden,
    /// `modell.aufruf`
    ModellAufruf,
    /// `modell.strom`
    ModellStrom,
    /// `strom.teil`
    StromTeil,
    /// `strom.ende`
    StromEnde,
    /// `modell.suche`
    ModellSuche,
    /// `mcp.aufruf`
    McpAufruf,
    /// `schluessel.setzen`
    Setzen,
    /// `schluessel.loeschen`
    Loeschen,
}

impl Op {
    /// The op of a name, `None` for an unknown one.
    #[must_use]
    pub fn aus_text(t: &str) -> Option<Op> {
        Some(match t {
            "schluessel.vorhanden" => Op::Vorhanden,
            "modell.aufruf" => Op::ModellAufruf,
            "modell.strom" => Op::ModellStrom,
            "strom.teil" => Op::StromTeil,
            "strom.ende" => Op::StromEnde,
            "modell.suche" => Op::ModellSuche,
            "mcp.aufruf" => Op::McpAufruf,
            "schluessel.setzen" => Op::Setzen,
            "schluessel.loeschen" => Op::Loeschen,
            _ => return None,
        })
    }
}

/// The check of one field: `Err` with stage 10, 11 or 12, or a nested violation of stage 8 or 9.
type Pruefer = fn(&Value) -> Result<(), Code>;

#[allow(
    clippy::unnecessary_wraps,
    reason = "the result is folded into the next call"
)]
fn kleinste(a: Option<Code>, b: Code) -> Option<Code> {
    Some(a.map_or(b, |a| a.min(b)))
}

/// Stages 8 to 12 of a closed object: unknown keys, missing keys, then the check of each present field.
fn objekt(
    o: &Map<String, Value>,
    felder: &[(&str, Pruefer)],
    pflicht: &[&str],
) -> Result<(), Code> {
    let mut schlecht = None;
    if o.keys().any(|k| !felder.iter().any(|(f, _)| f == k)) {
        schlecht = kleinste(schlecht, Code::SchluesselUnbekannt);
    }
    if pflicht.iter().any(|p| !o.contains_key(*p)) {
        schlecht = kleinste(schlecht, Code::SchluesselFehlt);
    }
    for (name, pruefer) in felder {
        if let Some(wert) = o.get(*name)
            && let Err(c) = pruefer(wert)
        {
            schlecht = kleinste(schlecht, c);
        }
    }
    schlecht.map_or(Ok(()), Err)
}

fn text(v: &Value) -> Result<&str, Code> {
    v.as_str().ok_or(Code::TypFalsch)
}

fn form(ok: bool) -> Result<(), Code> {
    if ok { Ok(()) } else { Err(Code::WertForm) }
}

fn ganzzahl(v: &Value) -> Result<Option<u64>, Code> {
    let Value::Number(n) = v else {
        return Err(Code::TypFalsch);
    };
    if let Some(u) = n.as_u64() {
        return Ok(Some(u));
    }
    if n.as_i64().is_some() {
        return Ok(None); // negative: an integer, but below every range here
    }
    Err(Code::TypFalsch)
}

fn ganz_im_bereich(v: &Value, min: u64, max: u64) -> Result<(), Code> {
    form(ganzzahl(v)?.is_some_and(|u| (min..=max).contains(&u)))
}

/// `^[a-z0-9][a-z0-9._-]{0,63}$`
#[must_use]
pub fn ist_kennung(t: &str) -> bool {
    let b = t.as_bytes();
    !b.is_empty()
        && b.len() <= 64
        && (b[0].is_ascii_lowercase() || b[0].is_ascii_digit())
        && b.iter().all(|c| {
            c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, b'.' | b'_' | b'-')
        })
}

/// A name of a secret: at most 64 characters, `^(modell|anmeldung|mcp|mail)\.[a-z0-9][a-z0-9._-]*$`.
#[must_use]
pub fn ist_name(t: &str) -> bool {
    if t.len() > 64 {
        return false;
    }
    let Some((art, rest)) = t.split_once('.') else {
        return false;
    };
    matches!(art, "modell" | "anmeldung" | "mcp" | "mail")
        && rest
            .as_bytes()
            .first()
            .is_some_and(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        && rest.bytes().all(|c| {
            c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, b'.' | b'_' | b'-')
        })
}

fn hex(t: &str, n: usize) -> bool {
    t.len() == n
        && t.bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
}

fn p_id(v: &Value) -> Result<(), Code> {
    match v {
        Value::String(s) => form(ist_kennung(s)),
        Value::Number(_) => ganz_im_bereich(v, 0, ID_MAX),
        _ => Err(Code::TypFalsch),
    }
}

/// `true` if `v` is a valid `id` (stage 10 and 11 of the field).
#[must_use]
pub fn id_gueltig(v: &Value) -> bool {
    p_id(v).is_ok()
}

#[allow(clippy::unnecessary_wraps, reason = "every checker has the same type")]
fn p_frei(_: &Value) -> Result<(), Code> {
    Ok(())
}

fn p_name(v: &Value) -> Result<(), Code> {
    form(ist_name(text(v)?))
}

fn p_kennung(v: &Value) -> Result<(), Code> {
    form(ist_kennung(text(v)?))
}

fn p_methode(v: &Value) -> Result<(), Code> {
    form(matches!(text(v)?, "GET" | "POST"))
}

/// `^/[A-Za-z0-9._~/-]{0,511}$`
#[must_use]
pub fn pfad_form(t: &str) -> bool {
    t.starts_with('/')
        && t.len() <= 512
        && t.bytes()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'.' | b'_' | b'~' | b'/' | b'-'))
}

fn p_pfad(v: &Value) -> Result<(), Code> {
    form(pfad_form(text(v)?))
}

fn p_abfrage(v: &Value) -> Result<(), Code> {
    match v {
        Value::Null => Ok(()),
        Value::String(s) => form(
            (1..=1024).contains(&s.len())
                && s.bytes()
                    .all(|c| c.is_ascii_alphanumeric() || b"._~!$&'()*+,;=:@/?%-".contains(&c)),
        ),
        _ => Err(Code::TypFalsch),
    }
}

fn p_kopf(v: &Value) -> Result<(), Code> {
    let Value::Object(o) = v else {
        return Err(Code::TypFalsch);
    };
    let mut schlecht = None;
    if o.len() > KOPF_MAX {
        schlecht = kleinste(schlecht, Code::WertForm);
    }
    for (name, wert) in o {
        let name_ok = (1..=64).contains(&name.len())
            && name
                .bytes()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-');
        if !name_ok {
            schlecht = kleinste(schlecht, Code::WertForm);
        } else if KOPF_VERBOTEN.contains(&name.as_str()) {
            schlecht = kleinste(schlecht, Code::KopfVerboten);
        }
        match wert {
            Value::String(s) => {
                if s.chars().count() > KOPFWERT_MAX
                    || !s.bytes().all(|c| (0x20..=0x7e).contains(&c))
                {
                    schlecht = kleinste(schlecht, Code::WertForm);
                }
            }
            _ => schlecht = kleinste(schlecht, Code::TypFalsch),
        }
    }
    schlecht.map_or(Ok(()), Err)
}

fn b64_mit(v: &Value, min: usize, max: usize) -> Result<(), Code> {
    let t = text(v)?;
    form((min..=max).contains(&t.len()) && b64::form_ok(t))
}

fn p_koerper(v: &Value) -> Result<(), Code> {
    b64_mit(v, 0, KOERPER_B64_MAX)
}

fn p_teil(v: &Value) -> Result<(), Code> {
    b64_mit(v, 4, TEIL_B64_MAX)
}

fn p_wert(v: &Value) -> Result<(), Code> {
    b64_mit(v, 4, 16384)
}

/// One search path: `^/[^\x00-\x1f]{0,4095}$` and no segment `.` or `..`.
fn suchpfad_form(t: &str) -> bool {
    t.starts_with('/')
        && t.chars().count() <= 4096
        && !t.chars().any(|c| u32::from(c) < 0x20)
        && !t.split('/').any(|s| s == "." || s == "..")
}

fn p_pfade(v: &Value) -> Result<(), Code> {
    let Value::Array(liste) = v else {
        return Err(Code::TypFalsch);
    };
    let mut schlecht = None;
    if !(1..=16).contains(&liste.len()) {
        schlecht = kleinste(schlecht, Code::WertForm);
    }
    for p in liste {
        match p {
            Value::String(s) if suchpfad_form(s) => {}
            Value::String(_) => schlecht = kleinste(schlecht, Code::WertForm),
            _ => schlecht = kleinste(schlecht, Code::TypFalsch),
        }
    }
    schlecht.map_or(Ok(()), Err)
}

fn p_werkzeug(v: &Value) -> Result<(), Code> {
    let t = text(v)?;
    form(
        (1..=128).contains(&t.len())
            && t.bytes()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'_' | b'.' | b'-')),
    )
}

fn p_objekt(v: &Value) -> Result<(), Code> {
    if v.is_object() {
        Ok(())
    } else {
        Err(Code::TypFalsch)
    }
}

fn p_person(v: &Value) -> Result<(), Code> {
    let t = text(v)?;
    form(t.strip_prefix("ps_").is_some_and(|h| hex(h, 32)))
}

fn p_vorschau(v: &Value) -> Result<(), Code> {
    match v {
        Value::Null => Ok(()),
        Value::String(s) => form(hex(s, 64)),
        _ => Err(Code::TypFalsch),
    }
}

fn p_nachweis(v: &Value) -> Result<(), Code> {
    let t = text(v)?;
    form((1..=8192).contains(&t.len()) && t.bytes().all(|c| (0x21..=0x7e).contains(&c)))
}

fn p_ms(v: &Value) -> Result<(), Code> {
    ganz_im_bereich(v, 0, MS_MAX)
}

fn p_mf(v: &Value) -> Result<(), Code> {
    form(text(v)?.strip_prefix("mf_").is_some_and(|h| hex(h, 32)))
}

fn p_hex64(v: &Value) -> Result<(), Code> {
    form(hex(text(v)?, 64))
}

fn p_hex32(v: &Value) -> Result<(), Code> {
    form(hex(text(v)?, 32))
}

fn p_scheinfassung(v: &Value) -> Result<(), Code> {
    ganz_im_bereich(v, 1, 1000)
}

fn p_daten(v: &Value) -> Result<(), Code> {
    const FELDER: [(&str, Pruefer); 9] = [
        ("ablauf", p_ms),
        ("anfrage", p_mf),
        ("art", p_kennung),
        ("ausgestellt", p_ms),
        ("digest", p_hex64),
        ("einmal", p_hex32),
        ("fassung", p_scheinfassung),
        ("kid", p_kennung),
        ("lauf", p_kennung),
    ];
    let Value::Object(o) = v else {
        return Err(Code::TypFalsch);
    };
    let pflicht: Vec<&str> = FELDER.iter().map(|(n, _)| *n).collect();
    objekt(o, &FELDER, &pflicht)
}

fn p_signatur(v: &Value) -> Result<(), Code> {
    let t = text(v)?;
    form(t.len() == 88 && t.ends_with("==") && b64::form_ok(t))
}

fn p_schein(v: &Value) -> Result<(), Code> {
    let Value::Object(o) = v else {
        return Err(Code::TypFalsch);
    };
    objekt(
        o,
        &[("daten", p_daten), ("signatur", p_signatur)],
        &["daten", "signatur"],
    )
}

const KOPF: [(&str, Pruefer); 3] = [("fassung", p_frei), ("id", p_id), ("op", p_frei)];

fn mit_kopf(weitere: &[(&'static str, Pruefer)]) -> Vec<(&'static str, Pruefer)> {
    let mut v = KOPF.to_vec();
    v.extend_from_slice(weitere);
    v
}

/// Stages 8 to 12 for an object whose `fassung` is 1 and whose `op` is `op`.
///
/// # Errors
/// The code of the lowest violated stage.
pub fn pruefen(o: &Map<String, Value>, op: Op) -> Result<(), Code> {
    let (felder, pflicht): (Vec<(&str, Pruefer)>, &[&str]) = match op {
        Op::Vorhanden => (mit_kopf(&[("name", p_name)]), &["id", "name"]),
        Op::ModellAufruf => (
            mit_kopf(&[
                ("ziel", p_kennung),
                ("methode", p_methode),
                ("pfad", p_pfad),
                ("abfrage", p_abfrage),
                ("kopf", p_kopf),
                ("koerper_b64", p_koerper),
            ]),
            &[
                "id",
                "ziel",
                "methode",
                "pfad",
                "abfrage",
                "kopf",
                "koerper_b64",
            ],
        ),
        Op::ModellStrom => (
            mit_kopf(&[
                ("ziel", p_kennung),
                ("pfad", p_pfad),
                ("abfrage", p_abfrage),
                ("kopf", p_kopf),
            ]),
            &["id", "ziel", "pfad", "abfrage", "kopf"],
        ),
        Op::StromTeil => (mit_kopf(&[("daten_b64", p_teil)]), &["id", "daten_b64"]),
        Op::StromEnde => (mit_kopf(&[]), &["id"]),
        Op::ModellSuche => (
            mit_kopf(&[("ziel", p_kennung), ("pfade", p_pfade)]),
            &["id", "ziel", "pfade"],
        ),
        Op::McpAufruf => (
            mit_kopf(&[
                ("lauf", p_kennung),
                ("server", p_kennung),
                ("werkzeug", p_werkzeug),
                ("argumente", p_objekt),
                ("person", p_person),
                ("vorschau_sha256", p_vorschau),
                ("schein", p_schein),
            ]),
            &[
                "id",
                "lauf",
                "server",
                "werkzeug",
                "argumente",
                "person",
                "vorschau_sha256",
            ],
        ),
        Op::Setzen => (
            mit_kopf(&[
                ("name", p_name),
                ("wert_b64", p_wert),
                ("nachweis", p_nachweis),
            ]),
            &["id", "name", "wert_b64", "nachweis"],
        ),
        Op::Loeschen => (
            mit_kopf(&[("name", p_name), ("nachweis", p_nachweis)]),
            &["id", "name", "nachweis"],
        ),
    };
    objekt(o, &felder, pflicht)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn obj(v: Value) -> Map<String, Value> {
        match v {
            Value::Object(o) => o,
            _ => Map::new(),
        }
    }

    #[test]
    fn kennung_und_name() {
        assert!(ist_kennung("a"));
        assert!(ist_kennung(&"a".repeat(64)));
        assert!(!ist_kennung(&"a".repeat(65)));
        assert!(!ist_kennung("-a"));
        assert!(!ist_kennung("A"));
        assert!(ist_name("anmeldung.copilot"));
        assert!(!ist_name("anmeldung."));
        assert!(!ist_name("schluessel.x"));
        assert!(!ist_name(&format!("mcp.{}", "a".repeat(61))));
        assert!(ist_name(&format!("mcp.{}", "a".repeat(60))));
    }

    #[test]
    fn stufen_in_reihenfolge() {
        // unknown key in the ticket wins over the missing field at the top
        let o = obj(
            json!({"fassung":1,"id":1,"op":"mcp.aufruf","schein":{"daten":{},"signatur":"x","z":1}}),
        );
        assert_eq!(pruefen(&o, Op::McpAufruf), Err(Code::SchluesselUnbekannt));
        let o = obj(json!({"fassung":1,"id":1,"op":"schluessel.vorhanden","name":5}));
        assert_eq!(pruefen(&o, Op::Vorhanden), Err(Code::TypFalsch));
        let o = obj(json!({"fassung":1,"id":-1,"op":"schluessel.vorhanden","name":"mcp.a"}));
        assert_eq!(pruefen(&o, Op::Vorhanden), Err(Code::WertForm));
        let o = obj(json!({"fassung":1,"id":1,"op":"strom.ende"}));
        assert_eq!(pruefen(&o, Op::StromEnde), Ok(()));
        let o = obj(
            json!({"fassung":1,"id":1,"op":"modell.strom","ziel":"a","pfad":"/r","abfrage":null,
            "kopf":{"Host":"x","host":"y"}}),
        );
        assert_eq!(pruefen(&o, Op::ModellStrom), Err(Code::WertForm));
    }

    #[test]
    fn kopf_grenzen() {
        let mut k = Map::new();
        for i in 0..65 {
            k.insert(format!("k{i}"), json!("v"));
        }
        assert_eq!(p_kopf(&Value::Object(k.clone())), Err(Code::WertForm));
        k.remove("k0");
        assert_eq!(p_kopf(&Value::Object(k)), Ok(()));
        assert_eq!(p_kopf(&json!({"a": "x".repeat(8192)})), Ok(()));
        assert_eq!(p_kopf(&json!({"a": "x".repeat(8193)})), Err(Code::WertForm));
        assert_eq!(p_kopf(&json!({"a": 1})), Err(Code::TypFalsch));
        assert_eq!(p_kopf(&json!({"cookie": "a"})), Err(Code::KopfVerboten));
    }

    #[test]
    fn feldpruefer_einzeln() {
        use serde_json::json;
        let ok = |f: Pruefer, v: Value| f(&v).is_ok();
        let h64 = "a".repeat(64);
        assert!(ok(p_hex64, json!(h64)));
        assert!(!ok(p_hex64, json!("A".repeat(64))));
        assert!(!ok(p_hex64, json!("a".repeat(63))));
        assert!(!ok(p_hex64, json!("g".repeat(64))));
        assert!(ok(p_hex32, json!("0".repeat(32))));
        assert!(!ok(p_hex32, json!("0".repeat(33))));
        assert!(ok(p_mf, json!(format!("mf_{}", "1".repeat(32)))));
        assert!(!ok(p_mf, json!("1".repeat(35))));
        assert!(ok(p_ms, json!(MS_MAX)));
        assert!(!ok(p_ms, json!(MS_MAX + 1)));
        assert!(!ok(p_ms, json!(-1)));
        assert_eq!(p_ms(&json!("1")), Err(Code::TypFalsch));
        assert_eq!(p_ms(&json!(1.5)), Err(Code::TypFalsch));
        assert!(ok(p_scheinfassung, json!(1000)));
        assert!(!ok(p_scheinfassung, json!(0)));
        assert!(!ok(p_scheinfassung, json!(1001)));
        assert!(ok(p_kennung, json!("l-1")));
        assert!(!ok(p_kennung, json!("L-1")));
        assert!(ok(p_methode, json!("POST")));
        assert!(!ok(p_methode, json!("PUT")));
        assert!(!ok(p_methode, json!("get")));
        assert!(ok(p_werkzeug, json!("a.B_c-1")));
        assert!(ok(p_werkzeug, json!("a".repeat(128))));
        assert!(!ok(p_werkzeug, json!("a".repeat(129))));
        assert!(!ok(p_werkzeug, json!("")));
        assert!(!ok(p_werkzeug, json!("a b")));
        assert!(ok(p_vorschau, json!(null)));
        assert!(ok(p_vorschau, json!("b".repeat(64))));
        assert!(!ok(p_vorschau, json!("b".repeat(65))));
        assert_eq!(p_vorschau(&json!(1)), Err(Code::TypFalsch));
        assert!(ok(p_nachweis, json!("x")));
        assert!(ok(p_nachweis, json!("x".repeat(8192))));
        assert!(!ok(p_nachweis, json!("x".repeat(8193))));
        assert!(!ok(p_nachweis, json!("")));
        assert!(!ok(p_nachweis, json!("a b")));
        assert!(ok(p_teil, json!("AAAA")));
        assert!(!ok(p_teil, json!("")));
        assert!(!ok(p_teil, json!("A".repeat(TEIL_B64_MAX + 4))));
        assert!(ok(p_teil, json!("A".repeat(TEIL_B64_MAX))));
        assert!(ok(p_wert, json!("AAAA")));
        assert!(!ok(p_wert, json!("")));
        assert!(!ok(p_wert, json!("A".repeat(16388))));
        assert!(ok(p_koerper, json!("")));
        assert!(!ok(p_koerper, json!("A")));
        let sig = format!("{}==", "A".repeat(86));
        assert!(ok(p_signatur, json!(sig)));
        assert!(!ok(p_signatur, json!(format!("{}=", "A".repeat(87)))));
        assert!(!ok(p_signatur, json!("A".repeat(88))));
        assert!(!ok(p_signatur, json!(format!("{}==", "A".repeat(82)))));
        assert!(id_gueltig(&json!(ID_MAX)));
        assert!(!id_gueltig(&json!(ID_MAX + 1)));
        assert!(!id_gueltig(&json!("A")));
        assert!(!id_gueltig(&json!(true)));
        let daten = json!({"ablauf": 2, "anfrage": format!("mf_{}", "1".repeat(32)), "art": "mcp-schein",
            "ausgestellt": 1, "digest": h64, "einmal": "5".repeat(32), "fassung": 1, "kid": "k", "lauf": "l"});
        assert!(ok(p_daten, daten.clone()));
        let mut ohne = daten.clone();
        if let Value::Object(o) = &mut ohne {
            o.remove("einmal");
        }
        assert_eq!(p_daten(&ohne), Err(Code::SchluesselFehlt));
        let mut falsch = daten;
        falsch["kid"] = json!("K");
        assert_eq!(p_daten(&falsch), Err(Code::WertForm));
        assert_eq!(p_daten(&json!([])), Err(Code::TypFalsch));
    }

    #[test]
    fn suchpfade() {
        assert!(suchpfad_form("/srv/beispiel/a b"));
        assert!(!suchpfad_form("/srv/./a"));
        assert!(!suchpfad_form("/srv/.."));
        assert!(suchpfad_form("/srv/...x"));
        assert!(!suchpfad_form("/a\u{1}"));
        assert!(!suchpfad_form("a"));
    }
}
