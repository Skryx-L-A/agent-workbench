//! The JSON text of the service: keys sorted, `", "` between the members and `": "` after a key, non-ASCII text as it is
//! (the form of `json.dumps(sort_keys=True, ensure_ascii=False)` of the reference). The size of a frame in the queue depends
//! on these bytes, and the outside cases compare the number of bytes in the queue.

use serde_json::{Map, Value};

/// The payload of a frame `Ereignis` as a JSON value; `None` if it is no UTF-8 or no JSON (the error `json_ungueltig`).
///
/// `serde_json` reads at most 127 levels. The bridge passes on a body of up to 127 levels inside the event
/// `{"art": "Starten", "start": ...}`, one level deeper (bridge contract, section 6 and section 14 point 2). For this case a payload that
/// `serde_json` refuses is read member by member once more: each member value on its own again has at most 127 levels, so the event has at
/// most 128. Only an object with `art` `Starten` is taken from there. Every other payload that is too deep stays `json_ungueltig`, as
/// before; the bound stays a bound and the stack depth stays the one of `serde_json`. A payload refused for another reason than the depth
/// fails member by member in the same place, because each member is read by `serde_json` again; so it stays `json_ungueltig` as well.
pub(crate) fn ereignis_lesen(nutzlast: &[u8]) -> Option<Value> {
    let text = std::str::from_utf8(nutzlast).ok()?;
    if let Ok(wert) = serde_json::from_str::<Value>(text) {
        return Some(wert);
    }
    let karte = objekt_je_glied(text)?;
    (karte.get("art").and_then(Value::as_str) == Some("Starten")).then_some(Value::Object(karte))
}

/// The position of the first byte from `p` on that is no JSON whitespace.
fn leer(b: &[u8], mut p: usize) -> usize {
    while matches!(b.get(p), Some(b' ' | b'\n' | b'\t' | b'\r')) {
        p += 1;
    }
    p
}

/// One JSON value from byte `p` on, read by `serde_json` on its own (with its own depth); the value and the position after it.
fn glied(text: &str, p: usize) -> Option<(Value, usize)> {
    let mut strom = serde_json::Deserializer::from_str(text.get(p..)?).into_iter::<Value>();
    let wert = strom.next()?.ok()?;
    Some((wert, p + strom.byte_offset()))
}

/// A JSON object read member by member: whitespace, `{`, members `"key": value` separated by `,`, `}`, whitespace, nothing else
/// (the syntax of `serde_json`; a later duplicate key replaces the earlier one as there). An object without members gives `None`: it
/// has no `art`, so it is never the event `Starten`.
fn objekt_je_glied(text: &str) -> Option<Map<String, Value>> {
    let b = text.as_bytes();
    let mut p = leer(b, 0);
    if b.get(p) != Some(&b'{') {
        return None;
    }
    let mut karte = Map::new();
    p = leer(b, p + 1);
    loop {
        if b.get(p) != Some(&b'"') {
            return None;
        }
        let (Value::String(schluessel), q) = glied(text, p)? else {
            return None;
        };
        p = leer(b, q);
        if b.get(p) != Some(&b':') {
            return None;
        }
        let (wert, q) = glied(text, p + 1)?;
        karte.insert(schluessel, wert);
        p = leer(b, q);
        match b.get(p) {
            Some(b',') => p = leer(b, p + 1),
            Some(b'}') => return (leer(b, p + 1) == b.len()).then_some(karte),
            _ => return None,
        }
    }
}

/// The text of a hex string as bytes; an odd length or a character that is no hex digit gives no bytes.
pub(crate) fn unhex(text: &str) -> Vec<u8> {
    let ziffer = |b: u8| match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    };
    let roh = text.as_bytes();
    if !roh.len().is_multiple_of(2) {
        return Vec::new();
    }
    roh.chunks(2)
        .map(|p| Some((ziffer(p[0])? << 4) + ziffer(p[1])?))
        .collect::<Option<Vec<u8>>>()
        .unwrap_or_default()
}

/// The JSON text of `wert`.
pub(crate) fn bytes(wert: &Value) -> Vec<u8> {
    let mut aus = String::new();
    schreiben(wert, &mut aus);
    aus.into_bytes()
}

fn text(s: &str, aus: &mut String) {
    // `serde_json` escapes a string like Python does: quote, backslash, `\n`, `\r`, `\t`, `\b`, `\f`, other controls as `\u00xx`.
    match serde_json::to_string(s) {
        Ok(t) => aus.push_str(&t),
        Err(_) => aus.push_str("\"\""),
    }
}

fn schreiben(wert: &Value, aus: &mut String) {
    match wert {
        Value::Null => aus.push_str("null"),
        Value::Bool(b) => aus.push_str(if *b { "true" } else { "false" }),
        Value::Number(n) => aus.push_str(&n.to_string()),
        Value::String(s) => text(s, aus),
        Value::Array(liste) => {
            aus.push('[');
            for (i, x) in liste.iter().enumerate() {
                if i > 0 {
                    aus.push_str(", ");
                }
                schreiben(x, aus);
            }
            aus.push(']');
        }
        Value::Object(karte) => {
            // `Map` keeps its keys sorted (no feature `preserve_order` in this workspace); the test `sortiert` checks it.
            aus.push('{');
            for (i, (k, x)) in karte.iter().enumerate() {
                if i > 0 {
                    aus.push_str(", ");
                }
                text(k, aus);
                aus.push_str(": ");
                schreiben(x, aus);
            }
            aus.push('}');
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{bytes, ereignis_lesen};
    use serde_json::{Value, json};

    fn s(v: &Value) -> String {
        String::from_utf8(bytes(v)).unwrap_or_default()
    }

    #[test]
    fn sortiert() {
        assert_eq!(
            s(&json!({"b": 1, "a": [1, 2], "c": {}})),
            r#"{"a": [1, 2], "b": 1, "c": {}}"#
        );
    }

    #[test]
    fn skalare_und_leere() {
        assert_eq!(s(&json!(null)), "null");
        assert_eq!(s(&json!(true)), "true");
        assert_eq!(s(&json!(false)), "false");
        assert_eq!(s(&json!([])), "[]");
        assert_eq!(s(&json!(-12)), "-12");
        assert_eq!(s(&json!([null, false])), "[null, false]");
    }

    #[test]
    fn text_wie_python() {
        assert_eq!(
            s(&json!("a\"b\\c\n\u{1}é\u{1f600}")),
            "\"a\\\"b\\\\c\\n\\u0001é\u{1f600}\""
        );
        assert_eq!(s(&json!({"k\n": "v"})), "{\"k\\n\": \"v\"}");
    }

    /// A JSON array with `n` levels: `[[...]]`.
    fn tief(n: usize) -> String {
        format!("{}{}", "[".repeat(n), "]".repeat(n))
    }

    fn starten_mit(start: &str) -> Vec<u8> {
        format!(r#"{{"art": "Starten", "start": {start}}}"#).into_bytes()
    }

    /// The bridge passes on a body of 127 levels (its bound) as `{"art": "Starten", "start": <body>}`: 128 levels for the service. The
    /// service reads it and refuses the structure as the check of the crate `start` does (stage 2, a structure that is no object), instead of
    /// ending the connection with `json_ungueltig` (outside case `br_starten_tiefe_127` of the bridge).
    #[test]
    fn starten_mit_einem_koerper_von_127_ebenen_wird_gelesen_und_als_start_abgelehnt() {
        let roh = starten_mit(&tief(127));
        assert!(
            serde_json::from_slice::<Value>(&roh).is_err(),
            "serde_json alone stops at 128 levels"
        );
        let wert = ereignis_lesen(&roh).expect("the event Starten with 128 levels is read");
        assert_eq!(wert["art"], json!("Starten"));
        let mut start = &wert["start"];
        let mut ebenen = 0;
        while let Some(innen) = start.as_array() {
            ebenen += 1;
            start = innen.first().unwrap_or(&Value::Null);
        }
        assert_eq!(ebenen, 127, "the structure is kept whole");
        let a = crate::starten::start_form(&wert["start"]).expect_err("no object");
        assert_eq!(
            (a.grund.as_str(), a.code.as_str()),
            ("start_pruefung", "start.json_ungueltig")
        );
        // an object as the deepest level, with whitespace around every member
        let roh = format!(
            " \n{{ \"start\" :\t{{\"a\": {} }} ,\r\"art\":\"Starten\" }} ",
            tief(126)
        );
        let wert = ereignis_lesen(roh.as_bytes()).expect("128 levels with an object");
        assert!(wert["start"]["a"].is_array());
    }

    /// The bound stays a bound: one level more, or a deep event of another kind, stays `json_ungueltig`.
    #[test]
    fn tiefer_oder_ein_anderes_ereignis_bleibt_json_ungueltig() {
        assert!(
            ereignis_lesen(&starten_mit(&tief(128))).is_none(),
            "129 levels"
        );
        assert!(
            ereignis_lesen(&starten_mit(&tief(10_000))).is_none(),
            "very deep"
        );
        let eingabe = format!(
            r#"{{"art": "Eingabe", "sitzung": "s-1", "x": {}}}"#,
            tief(127)
        );
        assert!(ereignis_lesen(eingabe.as_bytes()).is_none(), "another kind");
        let ohne_art = format!(r#"{{"start": {}}}"#, tief(127));
        assert!(ereignis_lesen(ohne_art.as_bytes()).is_none(), "no art");
        let art_kein_text = format!(r#"{{"art": ["Starten"], "start": {}}}"#, tief(127));
        assert!(
            ereignis_lesen(art_kein_text.as_bytes()).is_none(),
            "art no text"
        );
        assert!(ereignis_lesen(tief(129).as_bytes()).is_none(), "no object");
    }

    /// Read member by member, the syntax is the one of `serde_json`: what it refuses stays refused.
    #[test]
    fn je_glied_gilt_die_syntax_von_serde_json() {
        let t = tief(127);
        for falsch in [
            format!(r#"{{"art": "Starten", "start": {t},}}"#),
            format!(r#"{{"art": "Starten" "start": {t}}}"#),
            format!(r#"{{"art": "Starten", "start" {t}}}"#),
            format!(r#"{{"art": "Starten", "start": {t}}} x"#),
            format!(r#"{{"art": "Starten", "start": {t}}}{{}}"#),
            format!(r#"{{"art": "Starten", start: {t}}}"#),
            format!(r#"{{"art": "Starten", 1: {t}}}"#),
            format!(r#"{{"art": "Starten", "start": {t}, "n": NaN}}"#),
            format!(r#"{{"art": "Starten", "start": {t}, "n": 1x}}"#),
            format!(r#"{{"art": "Starten", "start": {t}, "n": "\ud800"}}"#),
            format!(r#"{{"art": "Starten", "start": {t}"#),
            format!(r#"[{{"art": "Starten", "start": {t}}}]"#),
            format!("\u{feff}{{\"art\": \"Starten\", \"start\": {t}}}"),
            format!("{{\"art\": \"Starten\",\u{a0}\"start\": {t}}}"),
        ] {
            assert!(ereignis_lesen(falsch.as_bytes()).is_none(), "{falsch:.60}");
        }
        let mut kaputt = starten_mit(&t);
        kaputt.push(0xff);
        assert!(ereignis_lesen(&kaputt).is_none(), "no UTF-8");
        // a later duplicate key replaces the earlier one, as in `serde_json`
        let doppelt =
            format!(r#"{{"art": "Eingabe", "start": 1, "art": "Starten", "start": {t}}}"#);
        let wert = ereignis_lesen(doppelt.as_bytes()).expect("duplicate key");
        assert_eq!(wert.as_object().map(serde_json::Map::len), Some(2));
        assert!(wert["start"].is_array());
        // shallow payloads are the ones of `serde_json`
        for flach in [
            r#"{"art": "Starten", "start": {"a": [1, 2.5, null]}}"#,
            "[1]",
            "3",
        ] {
            assert_eq!(
                ereignis_lesen(flach.as_bytes()),
                serde_json::from_str::<Value>(flach).ok()
            );
        }
        for kein in ["", "{", "{\"a\": NaN}", "{} {}"] {
            assert!(ereignis_lesen(kein.as_bytes()).is_none(), "{kein}");
        }
    }

    /// A shallow payload that `serde_json` refuses for another reason than the depth stays refused, also with `art` `Starten`, and an
    /// empty object is read as by `serde_json`.
    #[test]
    fn flacher_fehler_mit_art_starten_und_leeres_objekt() {
        for falsch in [
            r#"{"art": "Starten", "start": {},}"#,
            r#"{"art": "Starten", "start": {}} x"#,
            r#"{"art": "Starten", "start": {}, "n": 1x}"#,
            r#"{"art": "Starten", "start": {"a": NaN}}"#,
            "{} x",
            " {\t} ]",
        ] {
            assert!(ereignis_lesen(falsch.as_bytes()).is_none(), "{falsch}");
        }
        for leer in ["{}", " {\n} ", "{ }"] {
            assert_eq!(ereignis_lesen(leer.as_bytes()), Some(json!({})), "{leer:?}");
        }
    }
}
