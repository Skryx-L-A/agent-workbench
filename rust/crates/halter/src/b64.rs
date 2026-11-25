//! Base64 with padding (RFC 4648, section 4), in the form of the schema: groups of four, the last one with `==` or
//! `=`. Unused bits of the last group are ignored, as the pattern of the schema allows them.

const ZEICHEN: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

fn wert(c: u8) -> Option<u32> {
    match c {
        b'A'..=b'Z' => Some(u32::from(c - b'A')),
        b'a'..=b'z' => Some(u32::from(c - b'a') + 26),
        b'0'..=b'9' => Some(u32::from(c - b'0') + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    }
}

/// `true` if `text` has the form of the schema (`$defs/b64`).
#[must_use]
pub fn form_ok(text: &str) -> bool {
    dekodieren(text).is_some()
}

/// The bytes of `text`, `None` if it is not in the form of the schema.
#[must_use]
pub fn dekodieren(text: &str) -> Option<Vec<u8>> {
    let b = text.as_bytes();
    if !b.len().is_multiple_of(4) {
        return None;
    }
    let mut aus = Vec::with_capacity(b.len() / 4 * 3);
    let gruppen = b.len() / 4;
    for (i, g) in b.as_chunks::<4>().0.iter().enumerate() {
        let letzte = i + 1 == gruppen;
        let fuell = if letzte && g[2] == b'=' && g[3] == b'=' {
            2
        } else {
            usize::from(letzte && g[3] == b'=')
        };
        let mut n = 0u32;
        for (j, &c) in g.iter().enumerate() {
            let v = if j >= 4 - fuell { 0 } else { wert(c)? };
            n = (n << 6) | v;
        }
        let [_, b0, b1, b2] = n.to_be_bytes();
        aus.push(b0);
        if fuell < 2 {
            aus.push(b1);
        }
        if fuell < 1 {
            aus.push(b2);
        }
    }
    Some(aus)
}

/// The text of `bytes`, with padding.
#[must_use]
pub fn kodieren(bytes: &[u8]) -> String {
    let mut aus = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for g in bytes.chunks(3) {
        let n = (u32::from(g[0]) << 16)
            | (u32::from(*g.get(1).unwrap_or(&0)) << 8)
            | u32::from(*g.get(2).unwrap_or(&0));
        for j in 0..4 {
            if j > g.len() {
                aus.push('=');
            } else {
                let index = (n >> (18 - 6 * j)) & 63;
                aus.push(char::from(ZEICHEN[index as usize]));
            }
        }
    }
    aus
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hin_und_zurueck() {
        for n in 0..20u8 {
            let b: Vec<u8> = (0..n).map(|x| x.wrapping_mul(37)).collect();
            let t = kodieren(&b);
            assert_eq!(dekodieren(&t).as_deref(), Some(&b[..]), "{t}");
        }
        assert_eq!(kodieren(b"event: ping\n\n"), "ZXZlbnQ6IHBpbmcKCg==");
    }

    #[test]
    fn falsche_form() {
        for t in [
            "nicht base64",
            "abc",
            "a===",
            "ab=c",
            "ab==ab==",
            "=AAA",
            "AA-A",
        ] {
            assert!(dekodieren(t).is_none(), "{t}");
        }
        assert!(dekodieren("").is_some());
    }
}
