//! The check of a folder of one source (contract 5, section 5.4): the month files as one chain, every closed
//! month against its anchor.

use std::collections::BTreeMap;
use std::fs::File;
use std::io::{self, BufReader};
use std::path::Path;

use serde_json::{Map, Value};

use super::hash::sha256_hex;
use super::monate::{anker_name, anker_passt, monat_aus_name, monatsnamen};
use super::pruefen::{
    Bruch, Gelesen, Pruefung, Zustand, bruch, dateiname, objekt_lesen, zeile_lesen,
};
use super::{Fehler, ist_sha256, quelle_ok};

/// One line of the anchor file.
struct Anker {
    nummer: u64,
    roh: Vec<u8>,
    wert: Map<String, Value>,
}

/// The anchor lines by `abschnitt`. `Err` is the first break of the structure of the file.
fn anker_lesen(
    ordner: &Path,
    quelle: &str,
    monate: &[String],
) -> Result<BTreeMap<String, Anker>, Bruch> {
    let name = anker_name(quelle);
    let datei = match File::open(ordner.join(&name)) {
        Ok(datei) => datei,
        Err(fehler) if fehler.kind() == io::ErrorKind::NotFound => return Ok(BTreeMap::new()),
        Err(_) => return Err(bruch(&name, None, "datei_nicht_lesbar")),
    };
    let mut leser = BufReader::new(datei);
    let mut puffer = Vec::new();
    let mut anker = BTreeMap::new();
    let mut nummer: u64 = 0;
    loop {
        nummer += 1;
        let gelesen = zeile_lesen(&mut leser, &mut puffer)
            .map_err(|_| bruch(&name, None, "datei_nicht_lesbar"))?;
        let wert = match gelesen {
            Gelesen::Ende => return Ok(anker),
            Gelesen::Unvollstaendig => {
                return Err(bruch(&name, Some(nummer), "letzte_zeile_unvollstaendig"));
            }
            Gelesen::ZuLang => None,
            Gelesen::Zeile => objekt_lesen(&puffer),
        };
        let Some(Value::Object(wert)) = wert else {
            return Err(bruch(&name, Some(nummer), "zeile_unlesbar"));
        };
        let Some(abschnitt) = wert
            .get("abschnitt")
            .and_then(Value::as_str)
            .filter(|a| monat_aus_name(quelle, a).is_some())
            .map(str::to_owned)
        else {
            return Err(bruch(&name, Some(nummer), "anker_falsch"));
        };
        if anker.contains_key(&abschnitt) {
            return Err(bruch(&name, Some(nummer), "anker_doppelt"));
        }
        if !monate.contains(&abschnitt) {
            return Err(bruch(&name, Some(nummer), "anker_ohne_datei"));
        }
        anker.insert(
            abschnitt,
            Anker {
                nummer,
                roh: puffer.clone(),
                wert,
            },
        );
    }
}

/// A result that ends at a break.
fn gebrochen(zustand: &Zustand, bruch: Bruch) -> Pruefung {
    Pruefung {
        zeilen: zustand.zeilen,
        letzte: None,
        bruch: Some(bruch),
    }
}

/// Checks the folder of a source whose name is already known to be one of the five.
pub(super) fn pruefe_quelle_geprueft(
    ordner: &Path,
    quelle: &str,
    vorher: &str,
    letzte: Option<&str>,
) -> Pruefung {
    pruefe_quelle_mit(ordner, quelle, vorher, letzte, &mut |_, _| {})
}

/// As [`pruefe_quelle_geprueft`]; `sicht` sees every good line of the month files, in order.
fn pruefe_quelle_mit(
    ordner: &Path,
    quelle: &str,
    vorher: &str,
    letzte: Option<&str>,
    sicht: &mut dyn FnMut(&[u8], &Value),
) -> Pruefung {
    let mut zustand = Zustand::neu(vorher);
    let Ok(monate) = monatsnamen(ordner, quelle) else {
        let b = bruch(&dateiname(ordner), None, "datei_nicht_lesbar");
        return gebrochen(&zustand, b);
    };
    let anker = match anker_lesen(ordner, quelle, &monate) {
        Ok(anker) => anker,
        Err(b) => return gebrochen(&zustand, b),
    };
    let ankername = anker_name(quelle);
    for (stelle, name) in monate.iter().enumerate() {
        let werte = match zustand.datei_mit(&ordner.join(name), name, sicht) {
            Ok(werte) => werte,
            Err(b) => return gebrochen(&zustand, b),
        };
        let abweichung = match (&werte, anker.get(name)) {
            (None, Some(a)) => Some(bruch(&ankername, Some(a.nummer), "anker_falsch")),
            (Some(w), Some(a)) if !anker_passt(&a.roh, &a.wert, name, quelle, w) => {
                Some(bruch(&ankername, Some(a.nummer), "anker_falsch"))
            }
            (Some(_), None) if stelle + 1 < monate.len() => Some(bruch(name, None, "anker_fehlt")),
            _ => None,
        };
        if let Some(b) = abweichung {
            return gebrochen(&zustand, b);
        }
    }
    let letzter_name = monate.last().cloned().unwrap_or_else(|| dateiname(ordner));
    zustand.abschluss(letzter_name, letzte)
}

/// Checks the month files of one source in a folder as one chain, and every closed month against its anchor.
///
/// `vorher` is the expected `vorher` of the first line, `letzte` the expected sha256 of the last line, as for
/// [`super::pruefe`]. The function changes no file and reads no clock.
///
/// # Errors
/// `feld_ungueltig:quelle` when `quelle` is not one of the five sources.
pub fn pruefe_quelle(
    ordner: &Path,
    quelle: &str,
    vorher: &str,
    letzte: Option<&str>,
) -> Result<Pruefung, Fehler> {
    if !quelle_ok(quelle) {
        return Err(Fehler::FeldUngueltig("quelle"));
    }
    Ok(pruefe_quelle_geprueft(ordner, quelle, vorher, letzte))
}

/// A line found by [`letzte_zeile_der_art`].
#[derive(Debug, Clone, PartialEq)]
pub struct Gefunden {
    /// The line as a value.
    pub zeile: Value,
    /// The sha256 of the raw bytes of the line, without the line feed.
    pub sha256: String,
}

/// The last line of a source in which every field of `filter` has the given value, or `None`. The folder is
/// checked as a whole, like [`pruefe_quelle`]; a break is an error and no line is returned. Contract 5, section 5.7.
///
/// # Errors
/// `feld_ungueltig:quelle`, `feld_ungueltig:vorher` (not 64 lower-case hex digits); `kette_bruch:<grund>` when
/// the check of the folder finds a break (the error carries the place).
pub fn letzte_zeile_der_art(
    ordner: &Path,
    quelle: &str,
    vorher: &str,
    filter: &[(&str, Value)],
) -> Result<Option<Gefunden>, Fehler> {
    if !quelle_ok(quelle) {
        return Err(Fehler::FeldUngueltig("quelle"));
    }
    if !ist_sha256(vorher) {
        return Err(Fehler::FeldUngueltig("vorher"));
    }
    let mut gefunden: Option<Gefunden> = None;
    let pruefung = pruefe_quelle_mit(ordner, quelle, vorher, None, &mut |roh, wert| {
        if filter
            .iter()
            .all(|(feld, soll)| wert.get(feld) == Some(soll))
        {
            gefunden = Some(Gefunden {
                zeile: wert.clone(),
                sha256: sha256_hex(roh),
            });
        }
    });
    match pruefung.bruch {
        Some(b) => Err(Fehler::KetteBruch(b)),
        None => Ok(gefunden),
    }
}
