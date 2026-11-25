//! Strict reading of one line (stage 3) and the check of its canonical bytes (stage 4).
//!
//! `serde_json::Value` keeps the last value of a repeated key; this reader refuses a duplicate key on every level, so
//! `{"a":1,"a":2}` never becomes a valid object. A BOM, invalid UTF-8, `NaN` and trailing text are refused as well.

use std::fmt;

use serde::de::{self, Deserialize, Deserializer, MapAccess, SeqAccess, Visitor};
use serde_json::{Map, Number, Value};

struct OhneDoppel(Value);

struct Besucher;

impl<'de> Visitor<'de> for Besucher {
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

    fn visit_string<E: de::Error>(self, s: String) -> Result<OhneDoppel, E> {
        Ok(OhneDoppel(Value::String(s)))
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
                return Err(de::Error::custom("duplicate key"));
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

/// Reads one JSON value from `bytes`: valid UTF-8 without BOM, no duplicate key, nothing after the value.
#[must_use]
pub fn streng_lesen(bytes: &[u8]) -> Option<Value> {
    if bytes.starts_with(&[0xef, 0xbb, 0xbf]) {
        return None;
    }
    let text = std::str::from_utf8(bytes).ok()?;
    let mut leser = serde_json::Deserializer::from_str(text);
    let OhneDoppel(wert) = OhneDoppel::deserialize(&mut leser).ok()?;
    leser.end().ok()?;
    Some(wert)
}

/// The canonical text (RFC 8785) of a value, `None` if it has none (a number out of range).
#[must_use]
pub fn kanonisch(wert: &Value) -> Option<String> {
    formate::kette::kanonisch(wert).ok()
}

/// `true` if `bytes` are exactly the canonical form of `wert`.
#[must_use]
pub fn ist_kanonisch(wert: &Value, bytes: &[u8]) -> bool {
    kanonisch(wert).is_some_and(|k| k.as_bytes() == bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The text of the visitor in an error of serde (`invalid type: ..., expected any JSON value`).
    #[test]
    fn besucher_nennt_was_er_erwartet() {
        let e: &dyn de::Expected = &Besucher;
        assert_eq!(e.to_string(), "any JSON value");
    }

    #[test]
    fn doppelt_bom_nan_und_rest_sind_ungueltig() {
        assert!(streng_lesen(br#"{"a":1,"a":2}"#).is_none());
        assert!(streng_lesen(br#"{"b":{"a":1,"a":1}}"#).is_none());
        assert!(streng_lesen(b"\xef\xbb\xbf{}").is_none());
        assert!(streng_lesen(br#"{"a":NaN}"#).is_none());
        assert!(streng_lesen(b"{} x").is_none());
        assert!(streng_lesen(b"{\"a\":\"\xff\"}").is_none());
        assert!(streng_lesen(br#"{"a":[1,{"b":null}]}"#).is_some());
    }

    #[test]
    fn kanonisch_trennt_form_und_wert() {
        let w = streng_lesen(br#"{"b":1,"a":2}"#).expect("json");
        assert!(!ist_kanonisch(&w, br#"{"b":1,"a":2}"#));
        assert!(ist_kanonisch(&w, br#"{"a":2,"b":1}"#));
        let w = streng_lesen(br#"{"a":1.0}"#).expect("json");
        assert!(!ist_kanonisch(&w, br#"{"a":1.0}"#));
    }
}
