//! Strict JSON reading (contract 8, stage 9): UTF-8 without BOM, a duplicate key on any level is an error. `serde_json`
//! alone keeps the last of two keys; the visitor here refuses the second one.

use std::fmt;

use serde::de::{self, DeserializeSeed, MapAccess, SeqAccess, Visitor};
use serde_json::{Map, Number, Value};

/// The value of `bytes`, or `None` for no UTF-8, a BOM, a syntax error, a duplicate key or a number out of range.
/// `serde_json` itself refuses a BOM (it is no JSON whitespace) and bytes that are not UTF-8 (outside of strings they
/// are syntax errors, inside of strings it checks them); the tests below hold that.
pub(crate) fn lies_streng(bytes: &[u8]) -> Option<Value> {
    let mut leser = serde_json::Deserializer::from_slice(bytes);
    let Wert(wert) = Streng.deserialize(&mut leser).ok()?;
    leser.end().ok()?;
    Some(wert)
}

/// A read value. No `Default`: a changed visitor cannot hand back an empty value instead of the read one.
struct Wert(Value);

struct Streng;

impl<'de> DeserializeSeed<'de> for Streng {
    type Value = Wert;

    fn deserialize<D: de::Deserializer<'de>>(self, d: D) -> Result<Wert, D::Error> {
        d.deserialize_any(self)
    }
}

impl<'de> Visitor<'de> for Streng {
    type Value = Wert;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("JSON")
    }

    fn visit_bool<E>(self, v: bool) -> Result<Wert, E> {
        Ok(Wert(Value::Bool(v)))
    }

    fn visit_i64<E>(self, v: i64) -> Result<Wert, E> {
        Ok(Wert(Value::Number(v.into())))
    }

    fn visit_u64<E>(self, v: u64) -> Result<Wert, E> {
        Ok(Wert(Value::Number(v.into())))
    }

    fn visit_f64<E: de::Error>(self, v: f64) -> Result<Wert, E> {
        Number::from_f64(v)
            .map(|n| Wert(Value::Number(n)))
            .ok_or_else(|| E::custom("number"))
    }

    fn visit_str<E>(self, v: &str) -> Result<Wert, E> {
        Ok(Wert(Value::String(v.to_owned())))
    }

    fn visit_unit<E>(self) -> Result<Wert, E> {
        Ok(Wert(Value::Null))
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut a: A) -> Result<Wert, A::Error> {
        let mut liste = Vec::new();
        while let Some(w) = a.next_element_seed(Streng)? {
            liste.push(w.0);
        }
        Ok(Wert(Value::Array(liste)))
    }

    fn visit_map<A: MapAccess<'de>>(self, mut a: A) -> Result<Wert, A::Error> {
        let mut objekt = Map::new();
        while let Some(schluessel) = a.next_key::<String>()? {
            let wert = a.next_value_seed(Streng)?;
            if objekt.insert(schluessel, wert.0).is_some() {
                return Err(de::Error::custom("duplicate key"));
            }
        }
        Ok(Wert(Value::Object(objekt)))
    }
}

#[cfg(test)]
mod tests {
    use super::{Streng, lies_streng};
    use serde::de::DeserializeSeed;
    use serde::de::value::{BytesDeserializer, Error};
    use serde_json::json;

    #[test]
    fn a_wrong_type_names_what_was_expected() {
        let f = Streng
            .deserialize(BytesDeserializer::<Error>::new(b"x"))
            .err()
            .expect("bytes are no JSON value");
        assert!(f.to_string().ends_with("expected JSON"), "{f}");
    }

    #[test]
    fn values_are_read_as_they_are() {
        assert_eq!(
            lies_streng(br#" {"a":[true,false,null,-1,2,1.5,"x\/"],"b":{}} "#),
            Some(json!({"a":[true,false,null,-1,2,1.5,"x/"],"b":{}}))
        );
        assert_eq!(lies_streng(b"[]"), Some(json!([])));
        assert_eq!(
            lies_streng(b"18446744073709551615").and_then(|v| v.as_u64()),
            Some(u64::MAX)
        );
        assert_eq!(lies_streng(b"3.0").map(|v| v.is_u64()), Some(false));
    }

    #[test]
    fn a_duplicate_key_on_any_level_is_refused() {
        assert_eq!(lies_streng(br#"{"a":1,"a":1}"#), None);
        assert_eq!(lies_streng(br#"{"x":[{"a":1,"b":2,"a":3}]}"#), None);
        assert_eq!(lies_streng(br#"{"x":{"a":{},"a":{}}}"#), None);
        assert!(lies_streng(br#"{"a":{"a":1}}"#).is_some());
    }

    #[test]
    fn broken_input_is_refused() {
        assert_eq!(lies_streng(b"\xef\xbb\xbf{}"), None);
        assert_eq!(lies_streng(b"{\"a\":\"\xff\"}"), None);
        assert_eq!(lies_streng(b"{} x"), None);
        assert_eq!(lies_streng(b"{"), None);
        assert_eq!(lies_streng(b"1e999"), None);
        assert_eq!(lies_streng(b""), None);
    }
}
