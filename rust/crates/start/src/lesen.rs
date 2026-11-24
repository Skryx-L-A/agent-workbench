//! Reads a start: stages 1 to 13 of contract 1, for the request and for the start file.
//!
//! As in the settings reader, every check records its violation and the lowest stage wins.

use einstellungen::json::{self, Wert, feld};

use crate::form::{Felder, harness_art, ist_marker_liste};
use crate::{Fehler, Marker, Startanfrage, Startdatei};

const GROESSE_MAX: usize = 4096;
const ORDNER_MAX: usize = 1024;
const MODELL_MAX: usize = 128;
const ANFRAGE: [&str; 6] = [
    "fassung",
    "harness",
    "modell",
    "route",
    "kunde",
    "arbeitsordner",
];
const DATEI: [&str; 8] = [
    "fassung",
    "sitzung",
    "harness",
    "modell",
    "route",
    "kunde",
    "arbeitsordner",
    "marker",
];
const MARKER: [&str; 2] = ["liste", "ordner"];

/// Reads a request of the view. The bytes must not carry `sitzung`.
///
/// # Errors
/// Returns the [`Fehler`] of the lowest stage of contract 1 that the request violates.
pub fn lesen_anfrage(bytes: &[u8]) -> Result<Startanfrage, Fehler> {
    let roh = lesen(bytes, false)?;
    Ok(Startanfrage { felder: roh.felder })
}

/// Reads a start file of the service. The bytes must carry `sitzung`.
///
/// # Errors
/// Returns the [`Fehler`] of the lowest stage of contract 1 that the file violates.
pub fn lesen_datei(bytes: &[u8]) -> Result<Startdatei, Fehler> {
    let roh = lesen(bytes, true)?;
    Ok(Startdatei {
        sitzung: roh.sitzung,
        felder: roh.felder,
        marker: roh.marker,
    })
}

/// The parts of a start that the reader has checked. Only returned when no violation was recorded.
struct Roh {
    sitzung: String,
    felder: Felder,
    marker: Option<Marker>,
}

fn lesen(bytes: &[u8], datei: bool) -> Result<Roh, Fehler> {
    if bytes.len() > GROESSE_MAX {
        return Err(Fehler::ZuGross);
    }
    let wert = json::lesen(bytes).map_err(|_| Fehler::JsonUngueltig)?;
    let Wert::Objekt(felder) = wert else {
        return Err(Fehler::JsonUngueltig);
    };
    let mut befund = Befund::default();
    let roh = formlesen(&felder, datei, &mut befund);
    befund.ergebnis()?;
    Ok(roh)
}

#[derive(Default)]
struct Befund {
    fehler: Vec<Fehler>,
}

impl Befund {
    fn melde(&mut self, fehler: Fehler) {
        self.fehler.push(fehler);
    }

    fn ergebnis(self) -> Result<(), Fehler> {
        match self.fehler.into_iter().min_by_key(Fehler::stufe) {
            Some(fehler) => Err(fehler),
            None => Ok(()),
        }
    }
}

fn formlesen(felder: &[(String, Wert)], datei: bool, befund: &mut Befund) -> Roh {
    if !datei && feld(felder, "sitzung").is_some() {
        befund.melde(Fehler::SitzungNichtErlaubt);
    }
    if !datei && feld(felder, "marker").is_some() {
        befund.melde(Fehler::MarkerNichtErlaubt);
    }
    let erlaubt: &[&str] = if datei { &DATEI } else { &ANFRAGE };
    if felder
        .iter()
        .any(|(name, _)| !erlaubt.contains(&name.as_str()))
    {
        befund.melde(Fehler::SchluesselUnbekannt);
    }
    fassung(felder, befund);
    let sitzung = if datei {
        kennung(felder, "sitzung", befund)
            .unwrap_or_default()
            .to_owned()
    } else {
        String::new()
    };
    let harness = kennung(felder, "harness", befund).unwrap_or_default();
    if harness_art(harness).is_none() {
        befund.melde(Fehler::HarnessUnbekannt);
    }
    let route = kennung(felder, "route", befund).unwrap_or_default();
    let kunde = kunde(felder, befund);
    let modell = modell(felder, befund).unwrap_or_default();
    let arbeitsordner = ordner(felder, befund).unwrap_or_default();
    let marker = if datei { marker(felder, befund) } else { None };
    Roh {
        marker,
        sitzung,
        felder: Felder {
            harness: harness.to_owned(),
            modell: modell.to_owned(),
            route: route.to_owned(),
            kunde,
            arbeitsordner: arbeitsordner.to_owned(),
        },
    }
}

/// A required key: records stage 6 when it is missing.
fn pflicht<'a>(felder: &'a [(String, Wert)], name: &str, befund: &mut Befund) -> Option<&'a Wert> {
    let wert = feld(felder, name);
    if wert.is_none() {
        befund.melde(Fehler::SchluesselFehlt);
    }
    wert
}

/// A required key with a text value; records stage 7 for another type.
fn text<'a>(felder: &'a [(String, Wert)], name: &str, befund: &mut Befund) -> Option<&'a str> {
    let wert = pflicht(felder, name, befund)?;
    if let Wert::Text(t) = wert {
        Some(t)
    } else {
        befund.melde(Fehler::TypFalsch);
        None
    }
}

/// A required key with a text value of the form of a key; records stage 9.
fn kennung<'a>(felder: &'a [(String, Wert)], name: &str, befund: &mut Befund) -> Option<&'a str> {
    let t = text(felder, name, befund)?;
    if !einstellungen::ist_kennung(t) {
        befund.melde(Fehler::KennungForm);
    }
    Some(t)
}

fn fassung(felder: &[(String, Wert)], befund: &mut Befund) {
    match pflicht(felder, "fassung", befund) {
        None => {}
        Some(Wert::Ganzzahl(zahl)) if zahl == "1" => {}
        Some(Wert::Ganzzahl(_)) => befund.melde(Fehler::FassungUnbekannt),
        Some(_) => befund.melde(Fehler::TypFalsch),
    }
}

/// The customer: `null` for a session without a customer, otherwise a key (stage 9).
fn kunde(felder: &[(String, Wert)], befund: &mut Befund) -> Option<String> {
    match pflicht(felder, "kunde", befund)? {
        Wert::Null => None,
        Wert::Text(t) => {
            if !einstellungen::ist_kennung(t) {
                befund.melde(Fehler::KennungForm);
            }
            Some(t.clone())
        }
        _ => {
            befund.melde(Fehler::TypFalsch);
            None
        }
    }
}

fn modell<'a>(felder: &'a [(String, Wert)], befund: &mut Befund) -> Option<&'a str> {
    let t = text(felder, "modell", befund)?;
    if !ist_modell(t) {
        befund.melde(Fehler::ModellForm);
    }
    Some(t)
}

fn ordner<'a>(felder: &'a [(String, Wert)], befund: &mut Befund) -> Option<&'a str> {
    let t = text(felder, "arbeitsordner", befund)?;
    if !ist_ordner(t) {
        befund.melde(Fehler::OrdnerUngueltig);
    }
    Some(t)
}

/// The optional `marker` object of the start file: stages 5 to 7 and 12.
fn marker(felder: &[(String, Wert)], befund: &mut Befund) -> Option<Marker> {
    let Wert::Objekt(objekt) = feld(felder, "marker")? else {
        befund.melde(Fehler::TypFalsch);
        return None;
    };
    if objekt
        .iter()
        .any(|(name, _)| !MARKER.contains(&name.as_str()))
    {
        befund.melde(Fehler::SchluesselUnbekannt);
    }
    let mut liste = Vec::new();
    match pflicht(objekt, "liste", befund) {
        Some(Wert::Liste(elemente)) => {
            for element in elemente {
                match element {
                    Wert::Text(t) => liste.push(t.clone()),
                    _ => befund.melde(Fehler::TypFalsch),
                }
            }
            let namen: Vec<&str> = liste.iter().map(String::as_str).collect();
            if !ist_marker_liste(&namen) {
                befund.melde(Fehler::MarkerForm);
            }
        }
        Some(_) => befund.melde(Fehler::TypFalsch),
        None => {}
    }
    let mut ordner = String::new();
    match pflicht(objekt, "ordner", befund) {
        Some(Wert::Text(t)) => {
            if !ist_ordner(t) {
                befund.melde(Fehler::MarkerForm);
            }
            t.clone_into(&mut ordner);
        }
        Some(_) => befund.melde(Fehler::TypFalsch),
        None => {}
    }
    Some(Marker { liste, ordner })
}

/// One to 128 characters of `A-Z a-z 0-9 . _ : / @ + -`, no space, no option.
fn ist_modell(text: &str) -> bool {
    (1..=MODELL_MAX).contains(&text.len())
        && text.bytes().all(|c| {
            c.is_ascii_alphanumeric() || matches!(c, b'.' | b'_' | b':' | b'/' | b'@' | b'+' | b'-')
        })
}

/// An absolute, normalized path of at most 1024 bytes, with no `//`, no trailing `/`, no `.` or `..`
/// segment and no control character (U+0000 to U+001F, U+007F to U+009F).
pub(crate) fn ist_ordner(text: &str) -> bool {
    text.len() <= ORDNER_MAX
        && text.starts_with('/')
        && !text.ends_with('/')
        && !text.contains("//")
        && text
            .split('/')
            .skip(1)
            .all(|segment| segment != "." && segment != "..")
        && !text.chars().any(ist_steuerzeichen)
}

fn ist_steuerzeichen(zeichen: char) -> bool {
    let nummer = u32::from(zeichen);
    nummer <= 0x1f || (0x7f..=0x9f).contains(&nummer)
}

#[cfg(test)]
mod tests {
    use super::{ist_modell, ist_ordner, ist_steuerzeichen};

    #[test]
    fn model_names_follow_the_character_set_and_length() {
        assert!(ist_modell("a"));
        assert!(ist_modell("qwen3:8b"));
        assert!(ist_modell("a/b@c+d-e_f.g"));
        assert!(ist_modell(&"a".repeat(128)));
        assert!(!ist_modell(""));
        assert!(!ist_modell(&"a".repeat(129)));
        assert!(!ist_modell("a b"));
        assert!(!ist_modell("a,b"));
        assert!(!ist_modell("ä"));
    }

    #[test]
    fn folders_follow_the_path_rules() {
        assert!(ist_ordner("/a"));
        assert!(ist_ordner("/a b/ä/.x/...y"));
        assert!(ist_ordner(&format!("/{}", "a".repeat(1023))));
        assert!(!ist_ordner(&format!("/{}", "a".repeat(1024))));
        assert!(!ist_ordner(""));
        assert!(!ist_ordner("/"));
        assert!(!ist_ordner("a/b"));
        assert!(!ist_ordner("/a/"));
        assert!(!ist_ordner("/a//b"));
        assert!(!ist_ordner("/a/./b"));
        assert!(!ist_ordner("/a/../b"));
        assert!(!ist_ordner("/a\nb"));
        assert!(!ist_ordner("/a\u{7f}"));
        assert!(!ist_ordner("/a\u{80}"));
        assert!(!ist_ordner("/a\u{9f}"));
        assert!(ist_ordner("/a\u{a0}"));
    }

    #[test]
    fn control_characters_are_c0_del_and_c1() {
        assert!(ist_steuerzeichen('\u{0}'));
        assert!(ist_steuerzeichen('\u{1f}'));
        assert!(!ist_steuerzeichen(' '));
        assert!(ist_steuerzeichen('\u{7f}'));
        assert!(!ist_steuerzeichen('\u{7e}'));
        assert!(ist_steuerzeichen('\u{80}'));
        assert!(ist_steuerzeichen('\u{9f}'));
        assert!(!ist_steuerzeichen('\u{a0}'));
    }
}
