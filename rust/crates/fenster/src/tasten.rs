//! The keyboard of the terminal in the window (OF14 point 4, SE1): egui events become the readings of the view core of
//! the crate `ansicht` ([`ansicht::tasten::deute`]); the window has no key table of its own except the `AltGr` gap of
//! Windows below.

use ansicht::tasten::{Deutung, Taste};
use egui::{Event, Key, Modifiers};

/// What one input event in the terminal does.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Tat {
    /// A reading of the view core (a key, text, F6).
    Deutung(Deutung),
    /// Paste this text (`Einfuegen`, SE1 point 10).
    Einfuegen(String),
    /// Ask the system for the clipboard (Control+V, Control+Shift+V); egui answers with a paste event.
    EinfuegenAnfragen,
    /// Copy the marked text (Control+Shift+C, B38).
    Kopieren,
}

/// The `KeyboardEvent.key` name of an egui key, as far as the view core reads it.
fn name(k: Key) -> Option<&'static str> {
    Some(match k {
        Key::Enter => "Enter",
        Key::Tab => "Tab",
        Key::Escape => "Escape",
        Key::Backspace => "Backspace",
        Key::Delete => "Delete",
        Key::ArrowUp => "ArrowUp",
        Key::ArrowDown => "ArrowDown",
        Key::ArrowLeft => "ArrowLeft",
        Key::ArrowRight => "ArrowRight",
        Key::Home => "Home",
        Key::End => "End",
        Key::PageUp => "PageUp",
        Key::PageDown => "PageDown",
        Key::F6 => "F6",
        _ => return None,
    })
}

/// The letter of a key A to Z.
fn buchstabe(k: Key) -> Option<char> {
    let n = k.name();
    let mut c = n.chars();
    match (c.next(), c.next()) {
        (Some(b), None) if b.is_ascii_alphabetic() => Some(b.to_ascii_lowercase()),
        _ => None,
    }
}

/// `AltGr` under Windows (SE1 point 2): Windows reports `AltGr` as Control and Alt, and `egui-winit` 0.37 then drops the
/// text of the key. The window reads the character from the key instead, for the German layout of the pilot.
fn altgr_windows(k: Key) -> Option<&'static str> {
    Some(match k {
        Key::Q => "@",
        Key::E => "€",
        Key::M => "µ",
        Key::OpenCurlyBracket | Key::Num7 => "{",
        Key::OpenBracket | Key::Num8 => "[",
        Key::CloseBracket | Key::Num9 => "]",
        Key::CloseCurlyBracket | Key::Num0 => "}",
        Key::Backslash | Key::Minus => "\\",
        Key::Plus => "~",
        Key::Pipe | Key::IntlBackslash => "|",
        Key::Num2 => "²",
        Key::Num3 => "³",
        _ => return None,
    })
}

/// Reads the events of one frame while the terminal has the focus. `windows` says whether the `AltGr` rule of Windows
/// applies (the caller passes `cfg!(windows)`; the tests pass both).
#[must_use]
pub fn lesen(events: &[Event], windows: bool) -> Vec<Tat> {
    let mut aus = Vec::new();
    // after a key that went out as `Taste`, egui may bring the same character as text: it must not go out twice
    let mut text_schlucken = false;
    for e in events {
        match e {
            Event::Text(t) => {
                if text_schlucken {
                    text_schlucken = false;
                    continue;
                }
                aus.push(Tat::Deutung(Deutung::Text(t.clone())));
            }
            Event::Ime(egui::ImeEvent::Commit(t)) if !t.is_empty() => {
                aus.push(Tat::Deutung(Deutung::Text(t.clone())));
            }
            Event::Paste(t) => {
                if !t.is_empty() {
                    aus.push(Tat::Einfuegen(t.clone()));
                }
            }
            Event::Copy => aus.push(Tat::Kopieren),
            Event::Key {
                key,
                pressed: true,
                modifiers,
                ..
            } => {
                text_schlucken = false;
                if let Some(t) = taste(*key, *modifiers, windows) {
                    if matches!(t, Tat::Deutung(Deutung::Taste { .. })) {
                        text_schlucken = true;
                    }
                    aus.push(t);
                }
            }
            _ => {}
        }
    }
    aus
}

fn taste(key: Key, m: Modifiers, windows: bool) -> Option<Tat> {
    let strg = m.ctrl || (m.command && !m.mac_cmd);
    if windows
        && strg
        && m.alt
        && let Some(z) = altgr_windows(key)
    {
        return Some(Tat::Deutung(Deutung::Text(z.to_owned())));
    }
    // macOS: Command+V pastes and Command+C copies; every other Command key stays with the system (SE1 point 8)
    if m.mac_cmd && !m.alt {
        match key {
            Key::V => return Some(Tat::EinfuegenAnfragen),
            Key::C => return Some(Tat::Kopieren),
            _ => {}
        }
    }
    if strg && !m.alt {
        match (key, m.shift) {
            (Key::V, _) => return Some(Tat::EinfuegenAnfragen),
            (Key::C, true) => return Some(Tat::Kopieren),
            _ => {}
        }
    }
    let gross;
    let key_text: &str = if let Some(n) = name(key) {
        n
    } else {
        let b = buchstabe(key)?;
        // a plain letter comes as text; only Control or Alt make it a key (SE1 point 6)
        if !strg && !m.alt {
            return None;
        }
        gross = if m.shift && !strg {
            b.to_ascii_uppercase().to_string()
        } else {
            b.to_string()
        };
        &gross
    };
    let t = Taste {
        key: key_text,
        umschalt: m.shift,
        strg,
        alt: m.alt,
        meta: m.mac_cmd,
        komponiert: false,
        key_code: 0,
    };
    match ansicht::tasten::deute(&t) {
        Deutung::Nichts => None,
        d => Some(Tat::Deutung(d)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn k(key: Key, modifiers: Modifiers) -> Event {
        Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        }
    }

    fn taste_tat(t: &str, u: &[&'static str]) -> Tat {
        Tat::Deutung(Deutung::Taste {
            taste: t.to_owned(),
            umschalter: u.to_vec(),
        })
    }

    #[test]
    fn umlauts_and_composed_dead_keys_go_as_one_text() {
        let ev = [
            k(Key::Quote, Modifiers::NONE),
            Event::Text("ä".into()),
            Event::Text("ê".into()),
            Event::Ime(egui::ImeEvent::Preedit {
                text: "^".into(),
                active_range_chars: None,
            }),
            Event::Ime(egui::ImeEvent::Commit("é".into())),
        ];
        assert_eq!(
            lesen(&ev, false),
            vec![
                Tat::Deutung(Deutung::Text("ä".into())),
                Tat::Deutung(Deutung::Text("ê".into())),
                Tat::Deutung(Deutung::Text("é".into()))
            ]
        );
    }

    #[test]
    fn control_letters_and_named_keys_carry_their_modifiers() {
        let ev = [
            k(Key::C, Modifiers::CTRL),
            k(Key::Enter, Modifiers::SHIFT),
            k(Key::Tab, Modifiers::SHIFT),
            k(Key::ArrowLeft, Modifiers::CTRL),
        ];
        assert_eq!(
            lesen(&ev, false),
            vec![
                taste_tat("c", &["strg"]),
                taste_tat("Enter", &["umschalt"]),
                taste_tat("Tab", &["umschalt"]),
                taste_tat("Links", &["strg"])
            ]
        );
    }

    #[test]
    fn alt_with_a_letter_is_a_key_and_its_text_does_not_go_twice() {
        let ev = [k(Key::B, Modifiers::ALT), Event::Text("b".into())];
        assert_eq!(lesen(&ev, false), vec![taste_tat("b", &["alt"])]);
    }

    #[test]
    fn altgr_under_windows_is_text() {
        let altgr = Modifiers {
            alt: true,
            ctrl: true,
            ..Modifiers::NONE
        };
        let ev = [
            k(Key::Q, altgr),
            k(Key::OpenCurlyBracket, altgr),
            k(Key::E, altgr),
        ];
        assert_eq!(
            lesen(&ev, true),
            vec![
                Tat::Deutung(Deutung::Text("@".into())),
                Tat::Deutung(Deutung::Text("{".into())),
                Tat::Deutung(Deutung::Text("€".into()))
            ]
        );
        // outside Windows, Control+Alt+Q is a key (SE1 point 6)
        assert_eq!(
            lesen(&[k(Key::Q, altgr)], false),
            vec![taste_tat("q", &["strg", "alt"])]
        );
    }

    #[test]
    fn command_keys_of_macos_copy_and_paste_and_nothing_else() {
        let cmd = Modifiers::MAC_CMD | Modifiers::COMMAND;
        let ev = [k(Key::V, cmd), k(Key::C, cmd), k(Key::Q, cmd)];
        assert_eq!(
            lesen(&ev, false),
            vec![Tat::EinfuegenAnfragen, Tat::Kopieren]
        );
    }

    #[test]
    fn paste_copy_and_f6() {
        let ev = [
            k(Key::V, Modifiers::CTRL),
            Event::Paste("zwei\nZeilen".into()),
            k(Key::C, Modifiers::CTRL | Modifiers::SHIFT),
            k(Key::F6, Modifiers::NONE),
            k(Key::F6, Modifiers::SHIFT),
        ];
        assert_eq!(
            lesen(&ev, false),
            vec![
                Tat::EinfuegenAnfragen,
                Tat::Einfuegen("zwei\nZeilen".into()),
                Tat::Kopieren,
                Tat::Deutung(Deutung::Fokus(1)),
                Tat::Deutung(Deutung::Fokus(-1))
            ]
        );
    }
}
