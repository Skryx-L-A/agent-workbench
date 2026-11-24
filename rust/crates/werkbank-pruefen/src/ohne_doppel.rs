//! A JSON reader that refuses a duplicate key in any object, on every level (copy of `datentor`, one reader per crate).
//!
//! `serde_json::Value` keeps the last value of a repeated key and so can hide a stricter entry. This reader builds the same
//! `Value`, but an object with a repeated key is an error (fail closed). Keys are compared after decoding: `{"a":1,"a":2}`
//! has a duplicate.

use std::fmt;

use serde::de::{self, Deserialize, Deserializer, MapAccess, SeqAccess, Visitor};
use serde_json::{Map, Number, Value};

/// A `Value` that was read without a duplicate key.
struct OhneDoppel(Value);

/// Reads the visitor's values; every container recurses through [`OhneDoppel`].
struct Besucher;

impl<'de> Visitor<'de> for Besucher {
    // The visitor's value is the wrapper, which has no `Default`: a mutation cannot swap it for the default (`Null`), so no
    // equivalent mutant of the null branch survives.
    type Value = OhneDoppel;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("any JSON value")
    }

    fn visit_bool<E: de::Error>(self, b: bool) -> Result<OhneDoppel, E> {
        Ok(OhneDoppel(Value::Bool(b)))
    }

    fn visit_i64<E: de::Error>(self, n: i64) -> Result<OhneDoppel, E> {
        Ok(OhneDoppel(Value::Number(n.into())))
    }

    fn visit_u64<E: de::Error>(self, n: u64) -> Result<OhneDoppel, E> {
        Ok(OhneDoppel(Value::Number(n.into())))
    }

    fn visit_f64<E: de::Error>(self, n: f64) -> Result<OhneDoppel, E> {
        Number::from_f64(n)
            .map(|n| OhneDoppel(Value::Number(n)))
            .ok_or_else(|| E::custom("number is not finite"))
    }

    fn visit_str<E: de::Error>(self, s: &str) -> Result<OhneDoppel, E> {
        Ok(OhneDoppel(Value::String(s.to_owned())))
    }

    fn visit_unit<E: de::Error>(self) -> Result<OhneDoppel, E> {
        Ok(OhneDoppel(Value::Null))
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<OhneDoppel, A::Error> {
        let mut liste = Vec::new();
        while let Some(OhneDoppel(x)) = seq.next_element()? {
            liste.push(x);
        }
        Ok(OhneDoppel(Value::Array(liste)))
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<OhneDoppel, A::Error> {
        let mut objekt = Map::new();
        while let Some(schluessel) = map.next_key::<String>()? {
            let OhneDoppel(wert) = map.next_value()?;
            if objekt.contains_key(&schluessel) {
                return Err(de::Error::custom(format!("duplicate key {schluessel:?}")));
            }
            objekt.insert(schluessel, wert);
        }
        Ok(OhneDoppel(Value::Object(objekt)))
    }
}

impl<'de> Deserialize<'de> for OhneDoppel {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        d.deserialize_any(Besucher)
    }
}

/// Reads one JSON value from `eingabe`; trailing text, bad JSON or a duplicate key in any object is an error.
///
/// # Errors
/// The `serde_json` error: bad JSON, trailing text, or a duplicate key.
pub fn lesen(eingabe: &[u8]) -> Result<Value, serde_json::Error> {
    serde_json::from_slice::<OhneDoppel>(eingabe).map(|x| x.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn fehler(text: &str) -> String {
        lesen(text.as_bytes())
            .expect_err("must be refused")
            .to_string()
    }

    #[test]
    fn gleiche_werte_wie_serde_json() {
        for text in [
            "null",
            "true",
            "false",
            "0",
            "-5",
            "18446744073709551615",
            "-9223372036854775808",
            "1.5",
            "-0.0",
            "1e3",
            "\"\"",
            "\"a\\n\\u00e4\\ud83d\\ude00\"",
            "[]",
            "{}",
            "[1,[2,[3]],{\"a\":[{}]}]",
            "{\"a\":1,\"b\":{\"c\":[true,null]},\"d\":\"x\"}",
            " \n[ 1 , 2 ]\t",
        ] {
            let soll: Value = serde_json::from_str(text).expect("valid JSON");
            assert_eq!(lesen(text.as_bytes()).expect("valid JSON"), soll, "{text}");
        }
        assert_eq!(
            lesen(br#"{"a":[1,{"b":null}]}"#).expect("valid"),
            json!({"a": [1, {"b": null}]})
        );
    }

    #[test]
    fn doppelter_schluessel_auf_jeder_ebene() {
        for text in [
            r#"{"a":1,"a":2}"#,
            r#"{"a":1,"a":1}"#,
            r#"{"a":{"b":1},"a":{"b":1}}"#,
            r#"{"x":{"a":1,"a":2}}"#,
            r#"{"x":[{"a":1,"a":2}]}"#,
            r#"[{"a":1,"a":2}]"#,
            r#"[[[{"k":null,"k":null}]]]"#,
            r#"{"a":1,"b":2,"c":3,"a":4}"#,
            r#"{"":1,"":2}"#,
            r#"{"a":1,"a":2}"#,
            r#"{"a":1,"a":2}"#,
            r#"{"ä":1,"ä":2}"#,
            r#"{"a\nb":1,"a\u000ab":2}"#,
            r#"{"😀":1,"😀":2}"#,
            r#"{"a":{"b":{"c":{"d":1,"d":2}}}}"#,
        ] {
            let e = fehler(text);
            assert!(e.contains("duplicate key"), "{text}: {e}");
        }
    }

    #[test]
    fn gleiche_schluessel_in_verschiedenen_objekten_sind_kein_doppel() {
        for text in [
            r#"{"a":1,"b":{"a":2}}"#,
            r#"[{"a":1},{"a":2}]"#,
            r#"{"a":{"a":{"a":1}}}"#,
            r#"{"a":[{"a":1}],"b":[{"a":1}]}"#,
            r#"{"a":1,"A":2,"a ":3}"#,
            r#"{"\u00e4":1,"a\u0308":2}"#,
        ] {
            assert!(lesen(text.as_bytes()).is_ok(), "{text}");
        }
    }

    #[test]
    fn schlechtes_json_bleibt_ein_fehler() {
        for text in [
            "",
            "{",
            "{\"a\":}",
            "[1,]",
            "1 2",
            "{} x",
            "nul",
            "\"a",
            "{\"a\" 1}",
        ] {
            assert!(lesen(text.as_bytes()).is_err(), "{text:?}");
        }
        assert!(lesen(b"\xff").is_err());
        assert!(lesen(b"1e999").is_err());
    }

    #[test]
    fn der_besucher_nennt_was_er_erwartet() {
        let e =
            <serde_json::Error as de::Error>::invalid_type(de::Unexpected::Bytes(b"x"), &Besucher);
        assert!(e.to_string().ends_with("expected any JSON value"), "{e}");
    }

    #[test]
    fn der_fehlertext_ist_eine_zeile() {
        let e = fehler("{\"a\\nb\":1,\"a\\nb\":2}");
        assert!(!e.contains('\n'), "{e:?}");
    }
}
