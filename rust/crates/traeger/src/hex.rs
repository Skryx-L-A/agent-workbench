//! Lower-case hexadecimal text, the way the cases write bytes.

use std::fmt::Write as _;

/// The bytes as lower-case hex.
pub(crate) fn hex(bytes: &[u8]) -> String {
    bytes.iter().fold(String::new(), |mut aus, byte| {
        let _ = write!(aus, "{byte:02x}");
        aus
    })
}

/// The bytes of a lower-case hex text of even length; `None` for anything else (upper case, odd length, other characters).
pub(crate) fn unhex(text: &str) -> Option<Vec<u8>> {
    let ziffer = |c: u8| match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        _ => None,
    };
    let bytes = text.as_bytes();
    if !bytes.len().is_multiple_of(2) {
        return None;
    }
    bytes
        .chunks(2)
        .map(|paar| Some(ziffer(paar[0])? * 16 + ziffer(paar[1])?))
        .collect()
}
