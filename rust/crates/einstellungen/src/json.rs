//! Strict reader of the JSON text of the contract files.
//!
//! The reader takes the bytes of one file. It accepts one JSON value in UTF-8 without a byte order
//! mark, with whitespace around it. It refuses a duplicate key in one object, text after the value,
//! and nesting deeper than [`TIEFE_MAX`]. An integer keeps its text, so that `1` and `1.0` stay
//! apart.

use std::collections::HashSet;
use std::fmt::Write as _;

/// Deepest nesting of arrays and objects that the reader accepts.
pub const TIEFE_MAX: usize = 127;

const BOM: &[u8] = &[0xEF, 0xBB, 0xBF];

/// One value of a contract file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Wert {
    /// `null`.
    Null,
    /// `true` or `false`.
    Bool(bool),
    /// An integer as written, without fraction or exponent, such as `1` or `-3`.
    Ganzzahl(String),
    /// A number with fraction or exponent, as written, such as `1.0`.
    Kommazahl(String),
    /// A string with its escapes resolved.
    Text(String),
    /// An array, in the order of the text.
    Liste(Vec<Wert>),
    /// An object. Keys are unique and keep the order of the text.
    Objekt(Vec<(String, Wert)>),
}

/// The text is not one JSON value of the contract form.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Lesefehler;

/// Reads the bytes of one file as one JSON value.
///
/// # Errors
/// Returns [`Lesefehler`] for a byte order mark, bytes that are not UTF-8, a syntax error, a duplicate
/// key in one object, text after the value, or nesting deeper than [`TIEFE_MAX`].
pub fn lesen(bytes: &[u8]) -> Result<Wert, Lesefehler> {
    if bytes.starts_with(BOM) {
        return Err(Lesefehler);
    }
    let text = std::str::from_utf8(bytes).map_err(|_| Lesefehler)?;
    let mut leser = Leser { text, pos: 0 };
    let wert = leser.wert(0)?;
    leser.leerraum();
    if leser.pos == text.len() {
        Ok(wert)
    } else {
        Err(Lesefehler)
    }
}

/// Finds the value of a key in the fields of an object.
#[must_use]
pub fn feld<'a>(felder: &'a [(String, Wert)], name: &str) -> Option<&'a Wert> {
    felder
        .iter()
        .find(|(schluessel, _)| schluessel == name)
        .map(|(_, wert)| wert)
}

/// Writes `text` as a JSON string, with quotes and the escapes that JSON needs.
#[must_use]
pub fn text_schreiben(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for zeichen in text.chars() {
        match zeichen {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if u32::from(c) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", u32::from(c));
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

struct Leser<'a> {
    text: &'a str,
    pos: usize,
}

impl Leser<'_> {
    fn bytes(&self) -> &[u8] {
        self.text.as_bytes()
    }

    fn peek(&self) -> Option<u8> {
        self.bytes().get(self.pos).copied()
    }

    fn leerraum(&mut self) {
        let weiter = self.bytes().get(self.pos..).unwrap_or_default();
        let anzahl = weiter
            .iter()
            .take_while(|c| matches!(c, b' ' | b'\t' | b'\n' | b'\r'))
            .count();
        self.pos += anzahl;
    }

    fn erwarte(&mut self, zeichen: u8) -> Result<(), Lesefehler> {
        if self.peek() == Some(zeichen) {
            self.pos += 1;
            Ok(())
        } else {
            Err(Lesefehler)
        }
    }

    fn wert(&mut self, tiefe: usize) -> Result<Wert, Lesefehler> {
        self.leerraum();
        match self.peek().ok_or(Lesefehler)? {
            b'{' => self.objekt(tiefe),
            b'[' => self.liste(tiefe),
            b'"' => self.zeichenkette().map(Wert::Text),
            b't' => self.wort("true").map(|()| Wert::Bool(true)),
            b'f' => self.wort("false").map(|()| Wert::Bool(false)),
            b'n' => self.wort("null").map(|()| Wert::Null),
            b'-' | b'0'..=b'9' => self.zahl(),
            _ => Err(Lesefehler),
        }
    }

    fn wort(&mut self, wort: &str) -> Result<(), Lesefehler> {
        let passt = self
            .bytes()
            .get(self.pos..)
            .is_some_and(|rest| rest.starts_with(wort.as_bytes()));
        if passt {
            self.pos += wort.len();
            Ok(())
        } else {
            Err(Lesefehler)
        }
    }

    fn objekt(&mut self, tiefe: usize) -> Result<Wert, Lesefehler> {
        if tiefe >= TIEFE_MAX {
            return Err(Lesefehler);
        }
        self.erwarte(b'{')?;
        let mut felder = Vec::new();
        let mut namen = HashSet::new();
        self.leerraum();
        if self.peek() == Some(b'}') {
            self.pos += 1;
            return Ok(Wert::Objekt(felder));
        }
        loop {
            self.leerraum();
            if self.peek() != Some(b'"') {
                return Err(Lesefehler);
            }
            let name = self.zeichenkette()?;
            if !namen.insert(name.clone()) {
                return Err(Lesefehler);
            }
            self.leerraum();
            self.erwarte(b':')?;
            let wert = self.wert(tiefe + 1)?;
            felder.push((name, wert));
            self.leerraum();
            match self.peek() {
                Some(b',') => self.pos += 1,
                Some(b'}') => {
                    self.pos += 1;
                    return Ok(Wert::Objekt(felder));
                }
                _ => return Err(Lesefehler),
            }
        }
    }

    fn liste(&mut self, tiefe: usize) -> Result<Wert, Lesefehler> {
        if tiefe >= TIEFE_MAX {
            return Err(Lesefehler);
        }
        self.erwarte(b'[')?;
        let mut elemente = Vec::new();
        self.leerraum();
        if self.peek() == Some(b']') {
            self.pos += 1;
            return Ok(Wert::Liste(elemente));
        }
        // Each round consumes at least one byte, so the bound holds for every valid text.
        for _ in 0..=self.text.len() {
            elemente.push(self.wert(tiefe + 1)?);
            self.leerraum();
            match self.peek() {
                Some(b',') => self.pos += 1,
                Some(b']') => {
                    self.pos += 1;
                    return Ok(Wert::Liste(elemente));
                }
                _ => return Err(Lesefehler),
            }
        }
        Err(Lesefehler)
    }

    fn zeichenkette(&mut self) -> Result<String, Lesefehler> {
        self.erwarte(b'"')?;
        let mut out = String::new();
        // Each round consumes at least one byte, so the bound holds for every valid text.
        for _ in 0..=self.text.len() {
            match self.peek().ok_or(Lesefehler)? {
                b'"' => {
                    self.pos += 1;
                    return Ok(out);
                }
                b'\\' => {
                    self.pos += 1;
                    out.push(self.escape()?);
                }
                0..=0x1f => return Err(Lesefehler),
                _ => {
                    let zeichen = self
                        .text
                        .get(self.pos..)
                        .and_then(|rest| rest.chars().next())
                        .ok_or(Lesefehler)?;
                    out.push(zeichen);
                    self.pos += zeichen.len_utf8();
                }
            }
        }
        Err(Lesefehler)
    }

    fn escape(&mut self) -> Result<char, Lesefehler> {
        let zeichen = self.peek().ok_or(Lesefehler)?;
        self.pos += 1;
        Ok(match zeichen {
            b'"' => '"',
            b'\\' => '\\',
            b'/' => '/',
            b'b' => '\u{8}',
            b'f' => '\u{c}',
            b'n' => '\n',
            b'r' => '\r',
            b't' => '\t',
            b'u' => return self.unicode(),
            _ => return Err(Lesefehler),
        })
    }

    fn unicode(&mut self) -> Result<char, Lesefehler> {
        let hoch = self.hex4()?;
        let wert = if (0xD800..=0xDBFF).contains(&hoch) {
            self.erwarte(b'\\')?;
            self.erwarte(b'u')?;
            let tief = self.hex4()?;
            if !(0xDC00..=0xDFFF).contains(&tief) {
                return Err(Lesefehler);
            }
            0x10000 + ((hoch - 0xD800) << 10) + (tief - 0xDC00)
        } else {
            hoch
        };
        char::from_u32(wert).ok_or(Lesefehler)
    }

    fn hex4(&mut self) -> Result<u32, Lesefehler> {
        let mut wert = 0;
        for _ in 0..4 {
            let ziffer = char::from(self.peek().ok_or(Lesefehler)?)
                .to_digit(16)
                .ok_or(Lesefehler)?;
            wert = wert * 16 + ziffer;
            self.pos += 1;
        }
        Ok(wert)
    }

    fn zahl(&mut self) -> Result<Wert, Lesefehler> {
        let anfang = self.pos;
        if self.peek() == Some(b'-') {
            self.pos += 1;
        }
        match self.peek() {
            Some(b'0') => self.pos += 1,
            Some(b'1'..=b'9') => self.ziffern(),
            _ => return Err(Lesefehler),
        }
        let mut kommazahl = false;
        if self.peek() == Some(b'.') {
            kommazahl = true;
            self.pos += 1;
            self.ziffern_pflicht()?;
        }
        if matches!(self.peek(), Some(b'e' | b'E')) {
            kommazahl = true;
            self.pos += 1;
            if matches!(self.peek(), Some(b'+' | b'-')) {
                self.pos += 1;
            }
            self.ziffern_pflicht()?;
        }
        let text = self
            .text
            .get(anfang..self.pos)
            .ok_or(Lesefehler)?
            .to_owned();
        Ok(if kommazahl {
            Wert::Kommazahl(text)
        } else {
            Wert::Ganzzahl(text)
        })
    }

    fn ziffern(&mut self) {
        let weiter = self.bytes().get(self.pos..).unwrap_or_default();
        let anzahl = weiter.iter().take_while(|c| c.is_ascii_digit()).count();
        self.pos += anzahl;
    }

    fn ziffern_pflicht(&mut self) -> Result<(), Lesefehler> {
        if !matches!(self.peek(), Some(b'0'..=b'9')) {
            return Err(Lesefehler);
        }
        self.ziffern();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{Lesefehler, Wert, lesen, text_schreiben};

    fn gelesen(text: &str) -> Result<Wert, Lesefehler> {
        lesen(text.as_bytes())
    }

    #[test]
    fn literals_and_numbers_keep_their_kind() {
        assert_eq!(gelesen(" null "), Ok(Wert::Null));
        assert_eq!(gelesen("true"), Ok(Wert::Bool(true)));
        assert_eq!(gelesen("false"), Ok(Wert::Bool(false)));
        assert_eq!(gelesen("1"), Ok(Wert::Ganzzahl("1".to_owned())));
        assert_eq!(gelesen("-0"), Ok(Wert::Ganzzahl("-0".to_owned())));
        assert_eq!(gelesen("1.0"), Ok(Wert::Kommazahl("1.0".to_owned())));
        assert_eq!(
            gelesen("-2.5e+3"),
            Ok(Wert::Kommazahl("-2.5e+3".to_owned()))
        );
        assert_eq!(gelesen("1E2"), Ok(Wert::Kommazahl("1E2".to_owned())));
    }

    #[test]
    fn malformed_numbers_and_words_are_refused() {
        for text in [
            "01", "-", "1.", "1.x", "1e", "1e+", "-a", "nul", "tru", "fals", "+1", ".5",
        ] {
            assert_eq!(gelesen(text), Err(Lesefehler), "{text}");
        }
    }

    #[test]
    fn whitespace_is_space_tab_newline_and_carriage_return_only() {
        assert_eq!(gelesen("\t\r\n[ 1 ,\t2\r\n]\n"), gelesen("[1,2]"));
        assert_eq!(gelesen("\u{b}1"), Err(Lesefehler));
        assert_eq!(gelesen("\u{c}1"), Err(Lesefehler));
    }

    #[test]
    fn strings_resolve_every_escape() {
        let text = r#""a\"b\\c\/d\be\ff\ng\rh\ti\u0041\u00e4""#;
        assert_eq!(
            gelesen(text),
            Ok(Wert::Text(
                "a\"b\\c/d\u{8}e\u{c}f\ng\rh\ti\u{41}\u{e4}".to_owned()
            ))
        );
    }

    #[test]
    fn surrogate_pairs_join_to_one_character() {
        assert_eq!(
            gelesen(r#""\ud834\udd1e""#),
            Ok(Wert::Text("\u{1d11e}".to_owned()))
        );
        assert_eq!(gelesen(r#""\ud834""#), Err(Lesefehler));
        assert_eq!(gelesen(r#""\ud834x""#), Err(Lesefehler));
        assert_eq!(gelesen(r#""\ud834\u0041""#), Err(Lesefehler));
        assert_eq!(gelesen(r#""\udd1e""#), Err(Lesefehler));
    }

    #[test]
    fn bad_strings_are_refused() {
        for text in [
            "\"\u{1}\"",
            "\"\\x\"",
            "\"\\u12\"",
            "\"\\u12G4\"",
            "\"abc",
            "\"\\",
        ] {
            assert_eq!(gelesen(text), Err(Lesefehler), "{text:?}");
        }
    }

    #[test]
    fn duplicate_keys_are_refused_and_the_order_is_kept() {
        assert_eq!(gelesen(r#"{"a":1,"a":2}"#), Err(Lesefehler));
        assert_eq!(
            gelesen(r#"{"b":1,"a":{"a":2}}"#),
            Ok(Wert::Objekt(vec![
                ("b".to_owned(), Wert::Ganzzahl("1".to_owned())),
                (
                    "a".to_owned(),
                    Wert::Objekt(vec![("a".to_owned(), Wert::Ganzzahl("2".to_owned()))])
                ),
            ]))
        );
    }

    #[test]
    fn structure_errors_are_refused() {
        for text in [
            "",
            "{",
            "[",
            "[1,]",
            "{\"a\":1,}",
            "{1:2}",
            "{\"a\" 1}",
            "[1 2]",
            "{\"a\":1 \"b\":2}",
            "1 2",
        ] {
            assert_eq!(gelesen(text), Err(Lesefehler), "{text}");
        }
        assert_eq!(gelesen("[]"), Ok(Wert::Liste(Vec::new())));
        assert_eq!(gelesen("{}"), Ok(Wert::Objekt(Vec::new())));
    }

    #[test]
    fn byte_order_mark_and_invalid_utf8_are_refused() {
        assert_eq!(lesen(&[0xEF, 0xBB, 0xBF, b'1']), Err(Lesefehler));
        assert_eq!(lesen(&[b'"', 0xFF, b'"']), Err(Lesefehler));
    }

    #[test]
    fn nesting_is_limited_to_the_depth_maximum() {
        let ok = format!("{}{}", "[".repeat(127), "]".repeat(127));
        assert!(gelesen(&ok).is_ok());
        let zu_tief = format!("{}{}", "[".repeat(128), "]".repeat(128));
        assert_eq!(gelesen(&zu_tief), Err(Lesefehler));
        let objekte = format!("{}1{}", "{\"a\":".repeat(127), "}".repeat(127));
        assert!(gelesen(&objekte).is_ok());
        let objekte_zu_tief = format!("{}1{}", "{\"a\":".repeat(128), "}".repeat(128));
        assert_eq!(gelesen(&objekte_zu_tief), Err(Lesefehler));
    }

    #[test]
    fn written_strings_read_back_unchanged() {
        let text = "Ordner \"x\"\\y\n\t\u{1}\u{7f}ä€";
        let geschrieben = text_schreiben(text);
        assert_eq!(gelesen(&geschrieben), Ok(Wert::Text(text.to_owned())));
        assert_eq!(text_schreiben("a"), "\"a\"");
    }
}
