//! Keyboard of the terminal (contract `seite`, SE1 and SE2 point 4): one `keydown` becomes an event of the view, a
//! focus move or nothing. The view never turns a key into bytes; the core does that (`VT` R1).

/// The parts of a `keydown` the view reads; the flags are those of `KeyboardEvent`.
#[allow(clippy::struct_excessive_bools)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Taste<'a> {
    /// `KeyboardEvent.key`.
    pub key: &'a str,
    /// Shift.
    pub umschalt: bool,
    /// Control.
    pub strg: bool,
    /// Alt or Option.
    pub alt: bool,
    /// Meta (Command).
    pub meta: bool,
    /// `KeyboardEvent.isComposing`.
    pub komponiert: bool,
    /// `KeyboardEvent.keyCode` (229 during a composition).
    pub key_code: u32,
}

/// What a key does.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Deutung {
    /// Send nothing and prevent nothing (the browser keeps it, a dead key, a composition).
    Nichts,
    /// F6 (`1`) or Shift+F6 (`-1`): leave the terminal.
    Fokus(i8),
    /// A named key or Control/Alt with a letter.
    Taste {
        /// Name after `VT` R1.
        taste: String,
        /// Modifiers in the order umschalt, strg, alt.
        umschalter: Vec<&'static str>,
    },
    /// A printable character (also with `AltGr` and the Option key).
    Text(String),
}

/// The named keys with the names of `VT` R1.
const TASTEN: [(&str, &str); 13] = [
    ("Enter", "Enter"),
    ("Tab", "Tab"),
    ("Escape", "Escape"),
    ("Backspace", "Ruecktaste"),
    ("Delete", "Entf"),
    ("ArrowUp", "Hoch"),
    ("ArrowDown", "Runter"),
    ("ArrowLeft", "Links"),
    ("ArrowRight", "Rechts"),
    ("Home", "Pos1"),
    ("End", "Ende"),
    ("PageUp", "BildAuf"),
    ("PageDown", "BildAb"),
];

/// The modifiers in the order umschalt, strg, alt (SE1 point 7).
#[must_use]
pub fn umschalter(t: &Taste<'_>) -> Vec<&'static str> {
    let mut u = Vec::new();
    if t.umschalt {
        u.push("umschalt");
    }
    if t.strg {
        u.push("strg");
    }
    if t.alt {
        u.push("alt");
    }
    u
}

/// Reads one `keydown` (SE1 points 1 to 8, SE2 point 4).
#[must_use]
pub fn deute(t: &Taste<'_>) -> Deutung {
    if t.komponiert || t.key == "Dead" || t.key == "Process" || t.key_code == 229 {
        return Deutung::Nichts;
    }
    if t.key == "F6" {
        return Deutung::Fokus(if t.umschalt { -1 } else { 1 });
    }
    if t.meta {
        return Deutung::Nichts;
    }
    let u = umschalter(t);
    if let Some((_, name)) = TASTEN.iter().find(|(k, _)| *k == t.key) {
        return Deutung::Taste {
            taste: (*name).to_owned(),
            umschalter: u,
        };
    }
    let mut zeichen = t.key.chars();
    let (Some(c), None) = (zeichen.next(), zeichen.next()) else {
        return Deutung::Nichts;
    };
    let buchstabe = c.is_ascii_alphabetic();
    if t.strg && !t.alt {
        // Control+V and Control+Shift+letter belong to the browser (SE1 point 8), Control with other characters too.
        if !buchstabe || t.umschalt || c.eq_ignore_ascii_case(&'v') {
            return Deutung::Nichts;
        }
        return Deutung::Taste {
            taste: c.to_ascii_lowercase().to_string(),
            umschalter: vec!["strg"],
        };
    }
    // Alt with an unchanged letter is a key; what the layout gives with Alt or AltGr is text (SE1 points 2 and 6).
    if t.alt && c.is_ascii_lowercase() {
        return Deutung::Taste {
            taste: c.to_string(),
            umschalter: u.into_iter().filter(|x| *x != "umschalt").collect(),
        };
    }
    Deutung::Text(t.key.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn k(key: &str) -> Taste<'_> {
        Taste {
            key,
            ..Taste::default()
        }
    }

    #[test]
    fn printable_characters_and_umlauts_are_text() {
        for z in ["a", "Z", "7", " ", "ä", "Ö", "ü", "ß", "€"] {
            assert_eq!(deute(&k(z)), Deutung::Text(z.to_owned()));
        }
    }

    #[test]
    fn named_keys_carry_the_modifiers_in_order() {
        let t = Taste {
            key: "Enter",
            umschalt: true,
            ..Taste::default()
        };
        assert_eq!(
            deute(&t),
            Deutung::Taste {
                taste: "Enter".into(),
                umschalter: vec!["umschalt"]
            }
        );
        let t = Taste {
            key: "ArrowLeft",
            umschalt: true,
            strg: true,
            alt: true,
            ..Taste::default()
        };
        assert_eq!(
            deute(&t),
            Deutung::Taste {
                taste: "Links".into(),
                umschalter: vec!["umschalt", "strg", "alt"]
            }
        );
        assert_eq!(
            deute(&k("Backspace")),
            Deutung::Taste {
                taste: "Ruecktaste".into(),
                umschalter: vec![]
            }
        );
    }

    #[test]
    fn control_letters_are_keys_and_the_browser_keeps_its_own() {
        let strg = |key| Taste {
            key,
            strg: true,
            ..Taste::default()
        };
        assert_eq!(
            deute(&strg("C")),
            Deutung::Taste {
                taste: "c".into(),
                umschalter: vec!["strg"]
            }
        );
        assert_eq!(deute(&strg("v")), Deutung::Nichts);
        assert_eq!(deute(&strg("1")), Deutung::Nichts);
        let t = Taste {
            key: "C",
            strg: true,
            umschalt: true,
            ..Taste::default()
        };
        assert_eq!(deute(&t), Deutung::Nichts);
        let t = Taste {
            key: "c",
            meta: true,
            ..Taste::default()
        };
        assert_eq!(deute(&t), Deutung::Nichts);
    }

    #[test]
    fn altgr_and_option_characters_are_text_alt_letters_are_keys() {
        let t = Taste {
            key: "@",
            strg: true,
            alt: true,
            ..Taste::default()
        };
        assert_eq!(deute(&t), Deutung::Text("@".into()));
        let t = Taste {
            key: "{",
            alt: true,
            ..Taste::default()
        };
        assert_eq!(deute(&t), Deutung::Text("{".into()));
        let t = Taste {
            key: "b",
            alt: true,
            ..Taste::default()
        };
        assert_eq!(
            deute(&t),
            Deutung::Taste {
                taste: "b".into(),
                umschalter: vec!["alt"]
            }
        );
        let t = Taste {
            key: "x",
            strg: true,
            alt: true,
            ..Taste::default()
        };
        assert_eq!(
            deute(&t),
            Deutung::Taste {
                taste: "x".into(),
                umschalter: vec!["strg", "alt"]
            }
        );
    }

    #[test]
    fn dead_keys_compositions_and_f6() {
        assert_eq!(deute(&k("Dead")), Deutung::Nichts);
        assert_eq!(deute(&k("Process")), Deutung::Nichts);
        let t = Taste {
            key: "a",
            key_code: 229,
            ..Taste::default()
        };
        assert_eq!(deute(&t), Deutung::Nichts);
        let t = Taste {
            key: "a",
            komponiert: true,
            ..Taste::default()
        };
        assert_eq!(deute(&t), Deutung::Nichts);
        assert_eq!(deute(&k("F6")), Deutung::Fokus(1));
        let t = Taste {
            key: "F6",
            umschalt: true,
            ..Taste::default()
        };
        assert_eq!(deute(&t), Deutung::Fokus(-1));
        assert_eq!(deute(&k("F5")), Deutung::Nichts);
        assert_eq!(deute(&k("Shift")), Deutung::Nichts);
    }
}
