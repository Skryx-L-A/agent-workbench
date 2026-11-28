//! The fixed forms of the contracts: identifier, tool name, pseudonym, request identifier, model name, path,
//! integers and the strict JSON reading (no duplicate key on any level).

use std::collections::HashSet;

use serde_json::{Map, Value};

/// A key or identifier: `^[a-z0-9][a-z0-9._-]{0,63}$`.
pub(crate) fn kennung(text: &str) -> bool {
    zeichenfolge(
        text,
        64,
        |c| matches!(c, b'a'..=b'z' | b'0'..=b'9'),
        |c| matches!(c, b'a'..=b'z' | b'0'..=b'9' | b'.' | b'_' | b'-'),
    )
}

/// A tool name: `^[A-Za-z0-9][A-Za-z0-9._-]{0,127}$`.
pub(crate) fn werkzeug(text: &str) -> bool {
    zeichenfolge(
        text,
        128,
        |c| c.is_ascii_alphanumeric(),
        |c| c.is_ascii_alphanumeric() || matches!(c, b'.' | b'_' | b'-'),
    )
}

/// First byte by `erstes`, every further byte by `weiter`, 1 up to `laenge` bytes.
fn zeichenfolge(text: &str, laenge: usize, erstes: fn(u8) -> bool, weiter: fn(u8) -> bool) -> bool {
    match text.as_bytes().split_first() {
        Some((kopf, rest)) => {
            erstes(*kopf) && rest.len() < laenge && rest.iter().all(|c| weiter(*c))
        }
        None => false,
    }
}

/// Lower-case hex digits only, exactly `laenge` of them.
pub(crate) fn hex(text: &str, laenge: usize) -> bool {
    text.len() == laenge && text.bytes().all(|c| matches!(c, b'0'..=b'9' | b'a'..=b'f'))
}

/// A pseudonym: `^ps_[0-9a-f]{32}$`.
pub(crate) fn pseudonym(text: &str) -> bool {
    text.strip_prefix("ps_").is_some_and(|rest| hex(rest, 32))
}

/// A request identifier: `^mf-[0-9]{1,15}-[1-9][0-9]{0,9}$`.
pub(crate) fn anfrage(text: &str) -> bool {
    zaehlkennung(text, "mf-")
}

/// A promise identifier (section 15.2): `^zs-[0-9]{1,15}-[1-9][0-9]{0,9}$`.
pub(crate) fn zusage(text: &str) -> bool {
    zaehlkennung(text, "zs-")
}

/// `<praefix><laden_ms>-<n>`: 1 up to 15 digits, then 1 up to 10 digits without a leading zero.
fn zaehlkennung(text: &str, praefix: &str) -> bool {
    let Some(rest) = text.strip_prefix(praefix) else {
        return false;
    };
    let Some((laden, n)) = rest.split_once('-') else {
        return false;
    };
    ziffern(laden, 15) && ziffern(n, 10) && !n.starts_with('0')
}

/// A model name of contract 1 (section 15.3): 1 up to 128 bytes of `[A-Za-z0-9._:/@+-]`.
pub(crate) fn modell(text: &str) -> bool {
    (1..=128).contains(&text.len())
        && text.bytes().all(|c| {
            c.is_ascii_alphanumeric() || matches!(c, b'.' | b'_' | b':' | b'/' | b'@' | b'+' | b'-')
        })
}

/// 1 up to `hoechstens` decimal digits.
fn ziffern(text: &str, hoechstens: usize) -> bool {
    (1..=hoechstens).contains(&text.len()) && text.bytes().all(|c| c.is_ascii_digit())
}

/// An integer from `von` up to `bis`.
pub(crate) fn ganzzahl_in(wert: &Value, von: i64, bis: i64) -> bool {
    wert.as_i64().is_some_and(|n| (von..=bis).contains(&n))
}

/// Reads bytes as one JSON object: UTF-8 without BOM, no text after the object, no duplicate key in any object.
pub(crate) fn objekt_lesen(bytes: &[u8]) -> Option<Map<String, Value>> {
    let wert: Value = serde_json::from_slice(bytes).ok()?;
    match wert {
        Value::Object(objekt) if !schluessel_doppelt(bytes) => Some(objekt),
        _ => None,
    }
}

/// True when an object of the (valid) JSON text holds a key twice; keys compare after their escapes.
/// One pass over the bytes, so that no change of an index can make it loop.
pub(crate) fn schluessel_doppelt(bytes: &[u8]) -> bool {
    let mut stapel: Vec<Option<HashSet<String>>> = Vec::new();
    let mut schluessel_naechst = false;
    let mut text_anfang: Option<usize> = None;
    let mut maskiert = false;
    for (stelle, zeichen) in bytes.iter().enumerate() {
        if let Some(anfang) = text_anfang {
            if maskiert {
                maskiert = false;
            } else if *zeichen == b'\\' {
                maskiert = true;
            } else if *zeichen == b'"' {
                text_anfang = None;
                if schluessel_naechst {
                    schluessel_naechst = false;
                    let roh = bytes.get(anfang..=stelle).unwrap_or_default();
                    let schluessel: String = serde_json::from_slice(roh).unwrap_or_default();
                    if let Some(Some(gesehen)) = stapel.last_mut()
                        && !gesehen.insert(schluessel)
                    {
                        return true;
                    }
                }
            }
            continue;
        }
        match zeichen {
            b'"' => text_anfang = Some(stelle),
            b'{' => {
                stapel.push(Some(HashSet::new()));
                schluessel_naechst = true;
            }
            b'[' => stapel.push(None),
            b'}' | b']' => {
                stapel.pop();
                schluessel_naechst = false;
            }
            b',' => schluessel_naechst = matches!(stapel.last(), Some(Some(_))),
            _ => {}
        }
    }
    false
}

/// Decodes standard base64 (`[A-Za-z0-9+/]*` with at most two `=` at the end); `None` for any other text.
/// Bits that do not fill a byte at the end are dropped.
pub(crate) fn base64(text: &str) -> Option<Vec<u8>> {
    let daten = text.trim_end_matches('=');
    if text.len() - daten.len() > 2 {
        return None;
    }
    let mut aus = Vec::new();
    let mut puffer: u32 = 0;
    let mut bits = 0;
    for c in daten.bytes() {
        let wert = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => return None,
        };
        puffer = (puffer << 6) + u32::from(wert);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            aus.push((puffer >> bits).to_le_bytes()[0]);
        }
    }
    Some(aus)
}

#[cfg(test)]
mod tests;
