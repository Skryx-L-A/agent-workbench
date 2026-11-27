//! Strict reading of the file and of the request, and the form in which the helper writes the file.
//!
//! The strict reader is the one of the library `regelebenen` ([`streng_lesen`]): it refuses a duplicate key on every
//! level, a BOM, invalid UTF-8, `NaN` and trailing text, the same for the helper and the service.
//!
//! The helper writes JSON with sorted keys, an indent of one space and one line end (contract 15.12.2), byte for byte
//! as `json.dumps(inhalt, ensure_ascii=False, indent=1, sort_keys=True) + "\n"` of the reference `bau-faelle.py`.

use std::fmt::Write as _;

pub use regelebenen::json::streng_lesen;
use serde_json::Value;

fn zeichenkette(aus: &mut String, s: &str) {
    aus.push('"');
    for z in s.chars() {
        match z {
            '"' => aus.push_str("\\\""),
            '\\' => aus.push_str("\\\\"),
            '\n' => aus.push_str("\\n"),
            '\r' => aus.push_str("\\r"),
            '\t' => aus.push_str("\\t"),
            '\u{08}' => aus.push_str("\\b"),
            '\u{0c}' => aus.push_str("\\f"),
            z if u32::from(z) < 0x20 => {
                let _ = write!(aus, "\\u{:04x}", u32::from(z));
            }
            z => aus.push(z),
        }
    }
    aus.push('"');
}

fn einzug(aus: &mut String, tiefe: usize) {
    aus.push('\n');
    for _ in 0..tiefe {
        aus.push(' ');
    }
}

fn wert(aus: &mut String, w: &Value, tiefe: usize) {
    match w {
        Value::Null => aus.push_str("null"),
        Value::Bool(true) => aus.push_str("true"),
        Value::Bool(false) => aus.push_str("false"),
        Value::Number(n) => aus.push_str(&n.to_string()),
        Value::String(s) => zeichenkette(aus, s),
        Value::Array(liste) => {
            if liste.is_empty() {
                aus.push_str("[]");
                return;
            }
            aus.push('[');
            for (i, x) in liste.iter().enumerate() {
                if i > 0 {
                    aus.push(',');
                }
                einzug(aus, tiefe + 1);
                wert(aus, x, tiefe + 1);
            }
            einzug(aus, tiefe);
            aus.push(']');
        }
        Value::Object(objekt) => {
            if objekt.is_empty() {
                aus.push_str("{}");
                return;
            }
            let mut schluessel: Vec<&String> = objekt.keys().collect();
            schluessel.sort();
            aus.push('{');
            for (i, k) in schluessel.into_iter().enumerate() {
                if i > 0 {
                    aus.push(',');
                }
                einzug(aus, tiefe + 1);
                zeichenkette(aus, k);
                aus.push_str(": ");
                if let Some(x) = objekt.get(k) {
                    wert(aus, x, tiefe + 1);
                }
            }
            einzug(aus, tiefe);
            aus.push('}');
        }
    }
}

/// The bytes the helper writes for `inhalt`: sorted keys, indent 1, `": "` and `","` as separators, one line end.
#[must_use]
pub fn bytes_der(inhalt: &Value) -> Vec<u8> {
    let mut aus = String::new();
    wert(&mut aus, inhalt, 0);
    aus.push('\n');
    aus.into_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn doppelt_bom_nan_und_rest_sind_ungueltig() {
        assert!(streng_lesen(br#"{"a":1,"a":2}"#).is_none());
        assert!(streng_lesen(br#"{"b":{"a":1,"a":1}}"#).is_none());
        assert!(streng_lesen(b"\xef\xbb\xbf{}").is_none());
        assert!(streng_lesen(br#"{"a":NaN}"#).is_none());
        assert!(streng_lesen(b"{} x").is_none());
        assert!(streng_lesen(b"{\"a\":\"\xff\"}").is_none());
        let w = streng_lesen(br#"{"a":[1,-2,1.5,true,false,null,{"b":"c"}]}"#).expect("json");
        assert_eq!(
            w,
            serde_json::json!({"a": [1, -2, 1.5, true, false, null, {"b": "c"}]})
        );
        assert_eq!(streng_lesen(b" {}\n"), Some(serde_json::json!({})));
    }

    /// The expected texts are the output of `json.dumps(w, ensure_ascii=False, indent=1, sort_keys=True) + "\n"`
    /// of Python 3 (computed by hand from its rules, checked against the sha256 values of the cases in the outside
    /// test).
    #[test]
    fn form_wie_die_referenz() {
        let w = serde_json::json!({"b": [], "a": {}, "c": [1, {"y": null, "x": true}], "d": "ä\"\\\n\u{1}\u{7f}"});
        let soll = "{\n \"a\": {},\n \"b\": [],\n \"c\": [\n  1,\n  {\n   \"x\": true,\n   \"y\": null\n  }\n ],\n \
                    \"d\": \"ä\\\"\\\\\\n\\u0001\u{7f}\"\n}\n";
        assert_eq!(String::from_utf8(bytes_der(&w)).expect("utf8"), soll);
        assert_eq!(
            bytes_der(&serde_json::json!(" \u{1f}")),
            b"\" \\u001f\"\n",
            "a blank is no control character"
        );
        assert_eq!(bytes_der(&serde_json::json!([])), b"[]\n");
        assert_eq!(bytes_der(&serde_json::json!({})), b"{}\n");
        assert_eq!(bytes_der(&serde_json::json!(false)), b"false\n");
        assert_eq!(
            bytes_der(&serde_json::json!("\r\t\u{8}\u{c}\u{1f}")),
            b"\"\\r\\t\\b\\f\\u001f\"\n"
        );
        assert_eq!(
            bytes_der(&serde_json::json!(["a", "b"])),
            b"[\n \"a\",\n \"b\"\n]\n"
        );
    }
}
