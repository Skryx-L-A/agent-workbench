//! The answer lines (section 5) and their closed form (schema `$defs/antwort`). The holder builds every line here and
//! checks it against [`gueltig`] before it leaves; a line outside the form becomes `halter.intern`.

use serde_json::{Map, Value, json};

use crate::b64;
use crate::code::Code;
use crate::form::{self, KOPF_MAX, KOPFWERT_MAX, TEIL_B64_MAX};
use crate::json;

/// Head names of a target that never go out in `beginn` (section 4, "Antwortköpfe").
pub const ANTWORTKOPF_VERBOTEN: [&str; 15] = [
    "connection",
    "keep-alive",
    "proxy-connection",
    "te",
    "trailer",
    "upgrade",
    "transfer-encoding",
    "content-length",
    "set-cookie",
    "proxy-authenticate",
    "authorization",
    "proxy-authorization",
    "x-api-key",
    "api-key",
    "cookie",
];

fn grundform(o: &Map<String, Value>, erlaubt: &[&str], pflicht: &[&str]) -> bool {
    o.keys().all(|k| erlaubt.contains(&k.as_str())) && pflicht.iter().all(|p| o.contains_key(*p))
}

fn ganz(v: Option<&Value>, min: i64, max: i64) -> bool {
    v.and_then(Value::as_i64)
        .is_some_and(|n| (min..=max).contains(&n))
        && v.is_some_and(|v| v.is_i64() || v.is_u64())
}

fn kopf_ok(v: Option<&Value>) -> bool {
    let Some(Value::Object(k)) = v else {
        return false;
    };
    k.len() <= KOPF_MAX
        && k.iter().all(|(name, wert)| {
            (1..=64).contains(&name.len())
                && name
                    .bytes()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-')
                && !ANTWORTKOPF_VERBOTEN.contains(&name.as_str())
                && wert.as_str().is_some_and(|s| {
                    s.chars().count() <= KOPFWERT_MAX
                        && s.bytes().all(|c| (0x20..=0x7e).contains(&c))
                })
        })
}

fn suche_ok(v: Option<&Value>) -> bool {
    const FELDER: [&str; 4] = ["dateien", "nicht_lesbar", "treffer", "uebersprungen"];
    let Some(Value::Object(s)) = v else {
        return false;
    };
    grundform(s, &FELDER, &FELDER)
        && FELDER
            .iter()
            .all(|f| ganz(s.get(*f), 0, 9_007_199_254_740_991))
}

fn code_ok(v: Option<&Value>) -> bool {
    v.and_then(Value::as_str).is_some_and(|c| {
        c.strip_prefix("halter.").is_some_and(|r| {
            (2..=40).contains(&r.len()) && r.bytes().all(|b| b.is_ascii_lowercase() || b == b'_')
        })
    })
}

fn grund_ok(v: Option<&Value>) -> bool {
    v.and_then(Value::as_str).is_some_and(|g| {
        (1..=80).contains(&g.len()) && g.bytes().all(|c| (0x20..=0x7e).contains(&c))
    })
}

/// `true` if `wert` is a line in the closed form of `$defs/antwort`.
#[must_use]
pub fn wert_gueltig(wert: &Value) -> bool {
    let Value::Object(o) = wert else {
        return false;
    };
    if o.get("fassung").and_then(Value::as_u64) != Some(1)
        || !o.get("fassung").is_some_and(Value::is_u64)
    {
        return false;
    }
    let Some(id) = o.get("id") else {
        return false;
    };
    let id_ok = form::id_gueltig(id);
    let ok = o.get("ok");
    match o.get("art").and_then(Value::as_str) {
        Some(art @ ("antwort" | "ende")) => match ok {
            Some(Value::Bool(true)) if art == "antwort" => {
                id_ok
                    && o.len() <= 5
                    && grundform(
                        o,
                        &[
                            "art",
                            "fassung",
                            "ok",
                            "id",
                            "vorhanden",
                            "ergebnis",
                            "suche",
                        ],
                        &["ok"],
                    )
                    && o.get("vorhanden").is_none_or(Value::is_boolean)
                    && o.get("ergebnis").is_none_or(Value::is_object)
                    && (o.get("suche").is_none() || suche_ok(o.get("suche")))
            }
            Some(Value::Bool(true)) => {
                id_ok && grundform(o, &["art", "fassung", "ok", "id"], &["ok"])
            }
            Some(Value::Bool(false)) => {
                (id_ok || id.is_null())
                    && grundform(
                        o,
                        &["art", "fassung", "ok", "id", "code", "grund", "mcp_code"],
                        &["ok", "code", "grund"],
                    )
                    && code_ok(o.get("code"))
                    && grund_ok(o.get("grund"))
                    && (o.get("mcp_code").is_none()
                        || ganz(o.get("mcp_code"), -2_147_483_648, 2_147_483_647))
            }
            _ => false,
        },
        Some("beginn") => {
            id_ok
                && grundform(
                    o,
                    &["art", "fassung", "id", "status", "kopf"],
                    &["status", "kopf"],
                )
                && ganz(o.get("status"), 100, 599)
                && kopf_ok(o.get("kopf"))
        }
        Some("teil") => {
            id_ok
                && grundform(o, &["art", "fassung", "id", "daten_b64"], &["daten_b64"])
                && o.get("daten_b64")
                    .and_then(Value::as_str)
                    .is_some_and(|t| (4..=TEIL_B64_MAX).contains(&t.len()) && b64::form_ok(t))
        }
        _ => false,
    }
}

/// `true` if `bytes` are a line the holder may send: strict JSON, canonical, in the closed form.
#[must_use]
pub fn gueltig(bytes: &[u8]) -> bool {
    json::streng_lesen(bytes).is_some_and(|w| json::ist_kanonisch(&w, bytes) && wert_gueltig(&w))
}

/// A line as canonical text with LF; a value outside the form becomes the line `halter.intern` with the same id.
#[must_use]
pub fn zeile(wert: &Value) -> Vec<u8> {
    if wert_gueltig(wert)
        && let Some(t) = json::kanonisch(wert)
    {
        let mut b = t.into_bytes();
        b.push(b'\n');
        return b;
    }
    let id = wert.get("id").filter(|v| form::id_gueltig(v)).cloned();
    let mut b = json::kanonisch(&fehler_wert("antwort", id.as_ref(), Code::Intern, None))
        .unwrap_or_default()
        .into_bytes();
    b.push(b'\n');
    b
}

fn fehler_wert(art: &str, id: Option<&Value>, code: Code, mcp_code: Option<i64>) -> Value {
    let mut o = json!({
        "art": art,
        "code": code.code(),
        "fassung": 1,
        "grund": code.grund(),
        "id": id.cloned().unwrap_or(Value::Null),
        "ok": false,
    });
    if let (Some(m), Value::Object(map)) = (mcp_code, &mut o) {
        map.insert("mcp_code".to_owned(), json!(m));
    }
    o
}

/// `antwort` with `ok: false`.
#[must_use]
pub fn fehler(id: Option<&Value>, code: Code) -> Value {
    fehler_wert("antwort", id, code, None)
}

/// `antwort` with `halter.mcp_fehler` and the number of the server.
#[must_use]
pub fn mcp_fehler(id: &Value, mcp_code: i64) -> Value {
    fehler_wert("antwort", Some(id), Code::McpFehler, Some(mcp_code))
}

/// `ende` with `ok: false`.
#[must_use]
pub fn ende_fehler(id: &Value, code: Code) -> Value {
    fehler_wert("ende", Some(id), code, None)
}

/// `antwort` with `ok: true` and an optional field.
#[must_use]
pub fn ok(id: &Value, feld: Option<(&str, Value)>) -> Value {
    let mut o = json!({"art": "antwort", "fassung": 1, "id": id, "ok": true});
    if let (Some((k, v)), Value::Object(map)) = (feld, &mut o) {
        map.insert(k.to_owned(), v);
    }
    o
}

/// `beginn`.
#[must_use]
#[allow(clippy::needless_pass_by_value, reason = "the map moves into the line")]
pub fn beginn(id: &Value, status: u16, kopf: Map<String, Value>) -> Value {
    json!({"art": "beginn", "fassung": 1, "id": id, "kopf": kopf, "status": status})
}

/// `teil` with at most 65536 bytes.
#[must_use]
pub fn teil(id: &Value, daten: &[u8]) -> Value {
    json!({"art": "teil", "daten_b64": b64::kodieren(daten), "fassung": 1, "id": id})
}

/// `ende` with `ok: true`.
#[must_use]
pub fn ende(id: &Value) -> Value {
    json!({"art": "ende", "fassung": 1, "id": id, "ok": true})
}

#[cfg(test)]
mod tests {
    use super::*;

    fn g(t: &str) -> bool {
        gueltig(t.as_bytes())
    }

    #[test]
    fn grenzen_der_form() {
        assert!(g(
            r#"{"art":"beginn","fassung":1,"id":"m","kopf":{},"status":100}"#
        ));
        assert!(g(
            r#"{"art":"beginn","fassung":1,"id":"m","kopf":{},"status":599}"#
        ));
        assert!(!g(
            r#"{"art":"beginn","fassung":1,"id":"m","kopf":{},"status":99}"#
        ));
        assert!(!g(
            r#"{"art":"beginn","fassung":1,"id":"m","kopf":{},"status":600}"#
        ));
        assert!(!g(
            r#"{"art":"beginn","fassung":1,"id":"m","kopf":{},"status":"200"}"#
        ));
        assert!(!g(
            r#"{"art":"beginn","fassung":1,"id":"m","kopf":{"Content-Type":"a"},"status":200}"#
        ));
        assert!(!g(
            r#"{"art":"beginn","fassung":1,"id":"m","kopf":{"a":1},"status":200}"#
        ));
        assert!(!g(
            r#"{"art":"beginn","fassung":1,"id":"m","kopf":{"a":"\n"},"status":200}"#
        ));
        assert!(!g(r#"{"art":"beginn","fassung":1,"id":"m","status":200}"#));
        let fehler = |code: &str, extra: &str| {
            format!(
                r#"{{"art":"antwort","code":"{code}","fassung":1,"grund":"x","id":1{extra},"ok":false}}"#
            )
        };
        assert!(g(&fehler("halter.ab", "")));
        assert!(!g(&fehler("halter.a", "")));
        assert!(!g(&fehler("halter.Ab", "")));
        assert!(!g(&fehler("halterab", "")));
        assert!(!g(&fehler(&format!("halter.{}", "a".repeat(41)), "")));
        assert!(g(&fehler(
            "halter.mcp_fehler",
            r#","mcp_code":-2147483648"#
        )));
        assert!(!g(&fehler(
            "halter.mcp_fehler",
            r#","mcp_code":2147483648"#
        )));
        assert!(!g(&fehler("halter.mcp_fehler", r#","mcp_code":1.5"#)));
        assert!(!g(
            r#"{"art":"ende","fassung":1,"id":"m","ok":true,"vorhanden":true}"#
        ));
        assert!(g(
            r#"{"art":"antwort","fassung":1,"id":"m","ok":true,"vorhanden":true}"#
        ));
        assert!(!g(
            r#"{"art":"antwort","fassung":1,"id":"m","ok":true,"vorhanden":1}"#
        ));
        assert!(!g(r#"{"art":"antwort","fassung":1,"id":null,"ok":true}"#));
        assert!(g(
            r#"{"art":"antwort","code":"halter.peer","fassung":1,"grund":"x","id":null,"ok":false}"#
        ));
        assert!(!g(r#"{"art":"antwort","fassung":2,"id":"m","ok":true}"#));
        assert!(!g(r#"{"art":"antwort","fassung":1,"id":"m","ok":"ja"}"#));
        assert!(!g(r#"{"art":"teil","daten_b64":"","fassung":1,"id":"m"}"#));
        assert!(!g(r#"{"art":"teil","daten_b64":"AAAA","fassung":1}"#));
        assert!(!g(r#"{"art":"x","fassung":1,"id":"m"}"#));
        let suche = |inhalt: &str| {
            format!(r#"{{"art":"antwort","fassung":1,"id":"u","ok":true,"suche":{inhalt}}}"#)
        };
        assert!(g(&suche(
            r#"{"dateien":0,"nicht_lesbar":0,"treffer":0,"uebersprungen":9007199254740991}"#
        )));
        assert!(!g(&suche(
            r#"{"dateien":-1,"nicht_lesbar":0,"treffer":0,"uebersprungen":0}"#
        )));
        assert!(!g(&suche(r#"{"dateien":0,"nicht_lesbar":0,"treffer":0}"#)));
    }

    #[test]
    fn gebaute_zeilen_sind_gueltig() {
        let id = json!("m-1");
        for w in [
            fehler(None, Code::JsonUngueltig),
            fehler(Some(&json!(7)), Code::ScheinBenutzt),
            mcp_fehler(&json!(7), -32603),
            ende_fehler(&id, Code::LeckGesperrt),
            ok(&id, Some(("vorhanden", json!(true)))),
            ok(&id, None),
            beginn(&id, 200, Map::new()),
            teil(&id, b"abc"),
            ende(&id),
        ] {
            let z = zeile(&w);
            assert!(gueltig(&z[..z.len() - 1]), "{w}");
            assert!(!z.windows(13).any(|f| f == b"halter.intern"), "{w}");
        }
    }

    #[test]
    fn verbotenes_wird_intern() {
        let mut k = Map::new();
        k.insert("set-cookie".to_owned(), json!("a"));
        let z = zeile(&beginn(&json!("m-1"), 200, k));
        assert!(z.windows(13).any(|f| f == b"halter.intern"));
        let z = zeile(&teil(&json!("m-1"), b""));
        assert!(z.windows(13).any(|f| f == b"halter.intern"));
    }
}
