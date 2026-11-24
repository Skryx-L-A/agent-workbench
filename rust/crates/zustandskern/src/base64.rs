//! Standard Base64 with padding (RFC 4648), as the case files carry the OSC 52 data.

const ZIFFERN: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// The value of one Base64 digit, or `None` for any other byte.
fn wert(byte: u8) -> Option<u32> {
    match byte {
        b'A'..=b'Z' => Some(u32::from(byte - b'A')),
        b'a'..=b'z' => Some(u32::from(byte - b'a') + 26),
        b'0'..=b'9' => Some(u32::from(byte - b'0') + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    }
}

/// The digit for the low six bits of a value.
fn ziffer(wert: u32) -> char {
    usize::try_from(wert & 63)
        .ok()
        .and_then(|i| ZIFFERN.get(i))
        .map_or('A', |b| char::from(*b))
}

/// Decodes strictly: only the 64 digits, then exactly the padding that completes the last group of four.
/// The empty text is valid. Bits left over in the last digit are ignored.
pub(crate) fn dekodieren(text: &str) -> Option<Vec<u8>> {
    let bytes = text.as_bytes();
    let auffuell = bytes.iter().rev().take_while(|b| **b == b'=').count();
    let daten = bytes.get(..bytes.len() - auffuell)?;
    let ziffern: Vec<u32> = daten.iter().map(|b| wert(*b)).collect::<Option<_>>()?;
    let soll = match ziffern.len() % 4 {
        0 => 0,
        2 => 2,
        3 => 1,
        _ => return None,
    };
    if auffuell != soll {
        return None;
    }
    let mut aus = Vec::with_capacity(ziffern.len() / 4 * 3 + 2);
    for gruppe in ziffern.chunks(4) {
        let n = gruppe.len();
        let bits = gruppe.iter().fold(0_u32, |a, z| (a << 6) + z) << (6 * (4 - n));
        for k in 0..n - 1 {
            aus.push(u8::try_from((bits >> (16 - 8 * k)) & 0xff).unwrap_or(0));
        }
    }
    Some(aus)
}

/// Encodes with padding.
pub(crate) fn kodieren(daten: &[u8]) -> String {
    let mut aus = String::with_capacity(daten.len().div_ceil(3) * 4);
    for gruppe in daten.chunks(3) {
        let n = gruppe.len();
        let bits = gruppe.iter().fold(0_u32, |a, b| (a << 8) + u32::from(*b)) << (8 * (3 - n));
        for k in 0..=n {
            aus.push(ziffer(bits >> (18 - 6 * k)));
        }
        for _ in n..3 {
            aus.push('=');
        }
    }
    aus
}
