//! The bytes the core writes to the PTY for text, keys and pasted text (contract, rule R1).

/// Modifier bit: Shift.
pub(crate) const UMSCHALT: u8 = 1;
/// Modifier bit: Ctrl.
pub(crate) const STRG: u8 = 2;
/// Modifier bit: Alt.
pub(crate) const ALT: u8 = 4;
/// Modifier bits: Alt and Ctrl together.
const ALT_STRG: u8 = 6;

const PASTE_AN: &str = "\u{1b}[200~";
const PASTE_AUS: &str = "\u{1b}[201~";

/// The modifier bit of a modifier name, or `None` for an unknown name.
pub(crate) fn umschalter_bit(name: &str) -> Option<u8> {
    match name {
        "umschalt" => Some(UMSCHALT),
        "strg" => Some(STRG),
        "alt" => Some(ALT),
        _ => None,
    }
}

/// UTF-8 of the text, or `None` when it holds a control character (below 0x20, or 0x7f to 0x9f).
pub(crate) fn text_bytes(text: &str) -> Option<Vec<u8>> {
    if text
        .chars()
        .any(|c| u32::from(c) < 0x20 || (0x7f..=0x9f).contains(&u32::from(c)))
    {
        return None;
    }
    Some(text.as_bytes().to_vec())
}

/// The bytes of a key with its modifier bits, or `None` for a key the table does not hold.
pub(crate) fn taste_bytes(taste: &str, umschalter: u8) -> Option<Vec<u8>> {
    let fest: Option<&[u8]> = match (taste, umschalter) {
        ("Enter", 0) => Some(b"\r"),
        ("Enter", UMSCHALT) => Some(b"\x1b\r"),
        ("Tab", 0) => Some(b"\t"),
        ("Tab", UMSCHALT) => Some(b"\x1b[Z"),
        ("Escape", 0) => Some(b"\x1b"),
        ("Ruecktaste", 0) => Some(b"\x7f"),
        ("Ruecktaste", ALT) => Some(b"\x1b\x7f"),
        ("Entf", 0) => Some(b"\x1b[3~"),
        ("Hoch", 0) => Some(b"\x1b[A"),
        ("Runter", 0) => Some(b"\x1b[B"),
        ("Rechts", 0) => Some(b"\x1b[C"),
        ("Links", 0) => Some(b"\x1b[D"),
        ("Links", STRG) => Some(b"\x1b[1;5D"),
        ("Rechts", STRG) => Some(b"\x1b[1;5C"),
        ("Links", ALT) => Some(b"\x1b[1;3D"),
        ("Rechts", ALT) => Some(b"\x1b[1;3C"),
        ("Pos1", 0) => Some(b"\x1b[H"),
        ("Ende", 0) => Some(b"\x1b[F"),
        ("BildAuf", 0) => Some(b"\x1b[5~"),
        ("BildAb", 0) => Some(b"\x1b[6~"),
        _ => None,
    };
    if let Some(bytes) = fest {
        return Some(bytes.to_vec());
    }
    let mut zeichen = taste.chars();
    let (Some(buchstabe @ 'a'..='z'), None) = (zeichen.next(), zeichen.next()) else {
        return None;
    };
    let steuer = u8::try_from(u32::from(buchstabe) - 96).ok()?;
    let klein = u8::try_from(u32::from(buchstabe)).ok()?;
    match umschalter {
        STRG => Some(vec![steuer]),
        ALT => Some(vec![0x1b, klein]),
        ALT_STRG => Some(vec![0x1b, steuer]),
        _ => None,
    }
}

/// The bytes of a paste. Bracket marks inside the text are removed. With bracketed paste on, LF and CRLF
/// become CR and the text is wrapped in the marks; with it off, CR and CRLF become LF and nothing is added.
/// An empty result means nothing is sent.
pub(crate) fn einfuegen_bytes(text: &str, klammern: bool) -> Vec<u8> {
    let umgewandelt = if klammern {
        text.replace("\r\n", "\r").replace('\n', "\r")
    } else {
        text.replace("\r\n", "\n").replace('\r', "\n")
    };
    let bereinigt = umgewandelt.replace(PASTE_AN, "").replace(PASTE_AUS, "");
    if bereinigt.is_empty() {
        return Vec::new();
    }
    if klammern {
        format!("{PASTE_AN}{bereinigt}{PASTE_AUS}").into_bytes()
    } else {
        bereinigt.into_bytes()
    }
}
