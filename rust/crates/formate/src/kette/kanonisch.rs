//! JSON in the canonical form of RFC 8785 (JCS): keys sorted by UTF-16 code units, numbers as ECMAScript
//! `Number.prototype.toString` writes them, strings with the shortest escapes of `JSON.stringify`.

use serde_json::Value;

use super::Fehler;

/// A number as ECMAScript writes it (`Number.prototype.toString`, radix 10): `-0` is `0`, 21 digits before
/// the exponent form, 6 zeros after the point.
pub(crate) fn zahl_text(x: f64) -> Result<String, Fehler> {
    if !x.is_finite() {
        return Err(Fehler::NichtEndlich);
    }
    if x == 0.0 {
        return Ok("0".to_owned());
    }
    let vorzeichen = if x.is_sign_negative() { "-" } else { "" };
    // `{:e}` gives the shortest digits that read back to the same double, as "d.ddde-7" or "de21".
    let kurz = format!("{:e}", x.abs());
    let (mantisse, exponent) = kurz.split_once('e').unwrap_or((&kurz, "0"));
    let ziffern: String = mantisse.chars().filter(|c| *c != '.').collect();
    let k = i32::try_from(ziffern.len()).unwrap_or(i32::MAX);
    // n: the decimal point stands after n digits (ECMAScript 6.1.6.1.20: the value is digits * 10^(n-k)).
    let n = exponent.parse::<i32>().unwrap_or(0) + 1;
    let nullen = |anzahl: i32| "0".repeat(usize::try_from(anzahl).unwrap_or(0));
    let text = if (k..=21).contains(&n) {
        format!("{ziffern}{}", nullen(n - k))
    } else if (1..=21).contains(&n) {
        let (vor, nach) = ziffern.split_at(usize::try_from(n).unwrap_or(0));
        format!("{vor}.{nach}")
    } else if (-5..=0).contains(&n) {
        format!("0.{}{ziffern}", nullen(-n))
    } else {
        let e = n - 1;
        let e_text = format!("{}{}", if e < 0 { '-' } else { '+' }, e.abs());
        let (erste, weitere) = ziffern.split_at(1);
        if weitere.is_empty() {
            format!("{erste}e{e_text}")
        } else {
            format!("{erste}.{weitere}e{e_text}")
        }
    };
    Ok(format!("{vorzeichen}{text}"))
}

fn text_schreiben(text: &str, aus: &mut String) {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    aus.push('"');
    for c in text.chars() {
        match c {
            '"' => aus.push_str("\\\""),
            '\\' => aus.push_str("\\\\"),
            '\u{8}' => aus.push_str("\\b"),
            '\t' => aus.push_str("\\t"),
            '\n' => aus.push_str("\\n"),
            '\u{c}' => aus.push_str("\\f"),
            '\r' => aus.push_str("\\r"),
            c if u32::from(c) < 0x20 => {
                let b = usize::from(u8::try_from(u32::from(c)).unwrap_or(0));
                aus.push_str("\\u00");
                aus.push(char::from(HEX[b >> 4]));
                aus.push(char::from(HEX[b & 15]));
            }
            c => aus.push(c),
        }
    }
    aus.push('"');
}

fn schreiben(wert: &Value, aus: &mut String) -> Result<(), Fehler> {
    match wert {
        Value::Null => aus.push_str("null"),
        Value::Bool(true) => aus.push_str("true"),
        Value::Bool(false) => aus.push_str("false"),
        Value::Number(n) => aus.push_str(&zahl_text(n.as_f64().ok_or(Fehler::NichtEndlich)?)?),
        Value::String(text) => text_schreiben(text, aus),
        Value::Array(liste) => {
            aus.push('[');
            for (i, element) in liste.iter().enumerate() {
                if i > 0 {
                    aus.push(',');
                }
                schreiben(element, aus)?;
            }
            aus.push(']');
        }
        Value::Object(map) => {
            let mut schluessel: Vec<&String> = map.keys().collect();
            schluessel.sort_by(|a, b| a.encode_utf16().cmp(b.encode_utf16()));
            aus.push('{');
            for (i, name) in schluessel.into_iter().enumerate() {
                if i > 0 {
                    aus.push(',');
                }
                text_schreiben(name, aus);
                aus.push(':');
                if let Some(inhalt) = map.get(name) {
                    schreiben(inhalt, aus)?;
                }
            }
            aus.push('}');
        }
    }
    Ok(())
}

/// The bytes of `wert` after RFC 8785. A number that is no finite double gives `nicht_endlich`.
///
/// # Errors
/// `nicht_endlich` for a number that is not a finite double.
pub fn kanonisch(wert: &Value) -> Result<String, Fehler> {
    let mut aus = String::new();
    schreiben(wert, &mut aus)?;
    Ok(aus)
}

#[cfg(test)]
mod tests {
    use super::{Fehler, kanonisch, zahl_text};
    use serde_json::json;

    #[test]
    fn numbers_as_ecmascript() {
        let faelle: [(f64, &str); 28] = [
            (0.0, "0"),
            (-0.0, "0"),
            (1.0, "1"),
            (-1.0, "-1"),
            (5.0, "5"),
            (100.0, "100"),
            (0.1, "0.1"),
            (1.5, "1.5"),
            (123.456, "123.456"),
            (-1.5e-10, "-1.5e-10"),
            (1e-7, "1e-7"),
            (0.000_001, "0.000001"),
            (0.000_001_5, "0.0000015"),
            (1.5e-7, "1.5e-7"),
            (5e-324, "5e-324"),
            (1e20, "100000000000000000000"),
            (1e21, "1e+21"),
            (1.5e21, "1.5e+21"),
            (123_456_789_012_345_680_000.0, "123456789012345680000"),
            (9_007_199_254_740_992.0, "9007199254740992"),
            (4_294_967_295.0, "4294967295"),
            (1.797_693_134_862_315_7e308, "1.7976931348623157e+308"),
            (0.5, "0.5"),
            (10.5, "10.5"),
            (999_999_999_999_999_900_000.0, "999999999999999900000"),
            (-1e21, "-1e+21"),
            (2.5e-6, "0.0000025"),
            (12345.0, "12345"),
        ];
        for (x, text) in faelle {
            assert_eq!(zahl_text(x), Ok(text.to_owned()), "number {x:e}");
        }
    }

    #[test]
    fn not_finite_numbers_are_an_error() {
        assert_eq!(zahl_text(f64::NAN), Err(Fehler::NichtEndlich));
        assert_eq!(zahl_text(f64::INFINITY), Err(Fehler::NichtEndlich));
        assert_eq!(zahl_text(f64::NEG_INFINITY), Err(Fehler::NichtEndlich));
    }

    #[test]
    fn strings_use_the_shortest_escapes() {
        let wert = json!("a\"b\\c/d\n\t\r\u{8}\u{c}\u{1}\u{1f}\u{7f}\u{2028}ä€𝄞");
        assert_eq!(
            kanonisch(&wert).as_deref(),
            Ok("\"a\\\"b\\\\c/d\\n\\t\\r\\b\\f\\u0001\\u001f\u{7f}\u{2028}ä€𝄞\"")
        );
    }

    #[test]
    fn keys_sort_by_utf16_code_units() {
        // U+FB33 (one unit 0xFB33) sorts after U+1F600 (units 0xD83D 0xDE00) in UTF-16, before it in UTF-8.
        let wert: serde_json::Value =
            serde_json::from_str("{\"\u{fb33}\":1,\"\u{1f600}\":2,\"b\":3,\"10\":4,\"2\":5}")
                .expect("valid JSON");
        assert_eq!(
            kanonisch(&wert).as_deref(),
            Ok("{\"10\":4,\"2\":5,\"b\":3,\"\u{1f600}\":2,\"\u{fb33}\":1}")
        );
    }

    #[test]
    fn nested_values() {
        let wert = json!({"z": [1, {"b": null, "a": true}, false, []], "a": {}});
        assert_eq!(
            kanonisch(&wert).as_deref(),
            Ok("{\"a\":{},\"z\":[1,{\"a\":true,\"b\":null},false,[]]}")
        );
    }
}
