//! Reading JSON values the way the reference reads Python values.

use serde_json::Value;

/// Python truthiness of a JSON value: `null`, `false`, zero, `""`, `[]` and `{}` are false.
pub(crate) fn wahr(wert: &Value) -> bool {
    match wert {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().is_none_or(|f| f.abs() > 0.0),
        Value::String(s) => !s.is_empty(),
        Value::Array(a) => !a.is_empty(),
        Value::Object(o) => !o.is_empty(),
    }
}
