//! A strict reader and a writer for the JSON text of the description (std only, no dependency).
//!
//! Strict means: one value and nothing after it (white space only), an object has no key twice, a string holds no
//! raw control character, numbers are plain non-negative integers (no sign, no fraction, no exponent, no leading
//! zero), nesting is at most [`TIEFE_MAX`] deep. Anything else is a refusal, never a guess.

use std::collections::BTreeMap;
use std::fmt::Write as _;

/// The deepest nesting of arrays and objects. The description needs 3.
pub(crate) const TIEFE_MAX: usize = 8;

/// A JSON value as far as the description needs it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Wert {
    Null,
    Zahl(u64),
    Text(String),
    Liste(Vec<Wert>),
    Objekt(BTreeMap<String, Wert>),
}

/// Reads exactly one value from `bytes`. `None` for every kind of malformed input.
pub(crate) fn lesen(bytes: &[u8]) -> Option<Wert> {
    let text = std::str::from_utf8(bytes).ok()?;
    let mut leser = Leser {
        z: text.as_bytes(),
        i: 0,
    };
    leser.leer();
    let wert = leser.wert(0)?;
    leser.leer();
    (leser.i == leser.z.len()).then_some(wert)
}

struct Leser<'a> {
    z: &'a [u8],
    i: usize,
}

impl Leser<'_> {
    fn leer(&mut self) {
        while matches!(self.z.get(self.i), Some(b' ' | b'\t' | b'\n' | b'\r')) {
            self.i += 1;
        }
    }

    fn naechstes(&self) -> Option<u8> {
        self.z.get(self.i).copied()
    }

    fn wort(&mut self, wort: &str) -> bool {
        if self.z[self.i..].starts_with(wort.as_bytes()) {
            self.i += wort.len();
            true
        } else {
            false
        }
    }

    fn wert(&mut self, tiefe: usize) -> Option<Wert> {
        match self.naechstes()? {
            b'n' => self.wort("null").then_some(Wert::Null),
            b'"' => self.text().map(Wert::Text),
            b'0'..=b'9' => self.zahl(),
            b'[' if tiefe < TIEFE_MAX => self.liste(tiefe),
            b'{' if tiefe < TIEFE_MAX => self.objekt(tiefe),
            _ => None,
        }
    }

    fn zahl(&mut self) -> Option<Wert> {
        let start = self.i;
        while self.naechstes().is_some_and(|c| c.is_ascii_digit()) {
            self.i += 1;
        }
        let ziffern = std::str::from_utf8(&self.z[start..self.i]).ok()?;
        if ziffern.len() > 1 && ziffern.starts_with('0') {
            return None;
        }
        if matches!(self.naechstes(), Some(b'.' | b'e' | b'E')) {
            return None;
        }
        ziffern.parse().ok().map(Wert::Zahl)
    }

    fn hex4(&mut self) -> Option<u32> {
        let teil = self.z.get(self.i..self.i + 4)?;
        if !teil.iter().all(u8::is_ascii_hexdigit) {
            return None;
        }
        self.i += 4;
        u32::from_str_radix(std::str::from_utf8(teil).ok()?, 16).ok()
    }

    fn text(&mut self) -> Option<String> {
        self.i += 1; // the opening quote
        let mut aus: Vec<u8> = Vec::new();
        loop {
            let c = self.naechstes()?;
            self.i += 1;
            match c {
                b'"' => return String::from_utf8(aus).ok(),
                0..=0x1f => return None,
                b'\\' => {
                    let e = self.naechstes()?;
                    self.i += 1;
                    let zeichen = match e {
                        b'"' => '"',
                        b'\\' => '\\',
                        b'/' => '/',
                        b'b' => '\u{8}',
                        b'f' => '\u{c}',
                        b'n' => '\n',
                        b'r' => '\r',
                        b't' => '\t',
                        b'u' => self.unicode()?,
                        _ => return None,
                    };
                    let mut puffer = [0u8; 4];
                    aus.extend_from_slice(zeichen.encode_utf8(&mut puffer).as_bytes());
                }
                _ => aus.push(c),
            }
        }
    }

    fn unicode(&mut self) -> Option<char> {
        let erst = self.hex4()?;
        if (0xD800..0xDC00).contains(&erst) {
            if !self.wort("\\u") {
                return None;
            }
            let zweit = self.hex4()?;
            if !(0xDC00..0xE000).contains(&zweit) {
                return None;
            }
            char::from_u32(0x10000 + ((erst - 0xD800) << 10) + (zweit - 0xDC00))
        } else {
            char::from_u32(erst)
        }
    }

    fn liste(&mut self, tiefe: usize) -> Option<Wert> {
        self.i += 1;
        let mut werte = Vec::new();
        self.leer();
        if self.naechstes()? == b']' {
            self.i += 1;
            return Some(Wert::Liste(werte));
        }
        loop {
            self.leer();
            werte.push(self.wert(tiefe + 1)?);
            self.leer();
            match self.naechstes()? {
                b',' => self.i += 1,
                b']' => {
                    self.i += 1;
                    return Some(Wert::Liste(werte));
                }
                _ => return None,
            }
        }
    }

    fn objekt(&mut self, tiefe: usize) -> Option<Wert> {
        self.i += 1;
        let mut felder = BTreeMap::new();
        self.leer();
        if self.naechstes()? == b'}' {
            self.i += 1;
            return Some(Wert::Objekt(felder));
        }
        loop {
            self.leer();
            if self.naechstes()? != b'"' {
                return None;
            }
            let name = self.text()?;
            self.leer();
            if self.naechstes()? != b':' {
                return None;
            }
            self.i += 1;
            self.leer();
            let wert = self.wert(tiefe + 1)?;
            if felder.insert(name, wert).is_some() {
                return None;
            }
            self.leer();
            match self.naechstes()? {
                b',' => self.i += 1,
                b'}' => {
                    self.i += 1;
                    return Some(Wert::Objekt(felder));
                }
                _ => return None,
            }
        }
    }
}

/// A string as JSON text, with quotes. Control characters and the two special characters are escaped.
pub(crate) fn text_schreiben(aus: &mut String, text: &str) {
    aus.push('"');
    for c in text.chars() {
        match c {
            '"' => aus.push_str("\\\""),
            '\\' => aus.push_str("\\\\"),
            c if u32::from(c) < 0x20 || c == '\u{7f}' => {
                let _ = write!(aus, "\\u{:04x}", u32::from(c));
            }
            c => aus.push(c),
        }
    }
    aus.push('"');
}

#[cfg(test)]
mod tests;
