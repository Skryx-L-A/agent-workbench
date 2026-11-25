//! The JSON text of the service: keys sorted, `", "` between the members and `": "` after a key, non-ASCII text as it is
//! (the form of `json.dumps(sort_keys=True, ensure_ascii=False)` of the reference). The size of a frame in the queue depends
//! on these bytes, and the outside cases compare the number of bytes in the queue.

use serde_json::Value;

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
    use super::bytes;
    use serde_json::json;

    fn s(v: &serde_json::Value) -> String {
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
}
