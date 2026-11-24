//! The case runner behind the program `ra` (contract sections 2 and 3): reads one case as JSON bytes and returns
//! one result per step.

use std::collections::HashMap;
use std::fmt;

use serde_json::{Map, Value, json};

use crate::bild::{Abbild, Auszug, Cursor};
use crate::empfang::Empfang;
use crate::fehler::Fehler;
use crate::verbindung::Verbindung;
use crate::zeile::{Zeile, zeile_lesen, zeile_pruefen, zeile_schreiben};

/// The most bytes of a case the program reads.
pub const EINGABE_MAX: usize = 4_194_304;

/// The case is invalid: the program ends with exit code 2 and writes nothing to stdout.
#[derive(Debug, PartialEq, Eq)]
pub struct Ungueltig(pub String);

impl fmt::Display for Ungueltig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Fall ungueltig: {}", self.0)
    }
}

impl From<&str> for Ungueltig {
    fn from(grund: &str) -> Self {
        Self(grund.to_owned())
    }
}

/// Sender and receiver of one connection of a case.
struct Strecke {
    sender: Verbindung,
    empfang: Empfang,
}

/// Runs a case given as JSON bytes and returns `{"ergebnisse": [...]}`.
///
/// # Errors
///
/// [`Ungueltig`] when the input is longer than [`EINGABE_MAX`], is no JSON object, has no list `schritte`, or a
/// step is malformed (no text `art`, an unknown `art`, a missing field or one of the wrong type, a bad `nr`, an
/// unknown, doubled or ended connection, `hex` that is no lower-case hex of even length, `laeufe` of the wrong
/// shape). A fault in the message of a step is a result with `ok` false, not an invalid case.
pub fn laufen(eingabe: &[u8]) -> Result<Value, Ungueltig> {
    if eingabe.len() > EINGABE_MAX {
        return Err(Ungueltig(format!("mehr als {EINGABE_MAX} Byte")));
    }
    let fall: Value =
        serde_json::from_slice(eingabe).map_err(|e| Ungueltig(format!("kein JSON: {e}")))?;
    let Some(Value::Array(schritte)) = fall.get("schritte") else {
        return Err("schritte fehlt oder ist keine Liste".into());
    };
    let mut verbindungen: HashMap<&str, Strecke> = HashMap::new();
    let mut ergebnisse = Vec::with_capacity(schritte.len());
    for schritt in schritte {
        ergebnisse.push(schritt_ausfuehren(schritt, &mut verbindungen)?);
    }
    Ok(json!({ "ergebnisse": ergebnisse }))
}

/// A result of a failed step.
fn fehler_ergebnis(f: Fehler) -> Value {
    json!({ "ok": false, "fehler": f.code() })
}

/// A text field of a step.
fn text_feld<'a>(schritt: &'a Map<String, Value>, name: &str) -> Result<&'a str, Ungueltig> {
    schritt
        .get(name)
        .and_then(Value::as_str)
        .ok_or_else(|| Ungueltig(format!("Feld {name}")))
}

/// The session number of a step: a whole number from 0 to 255.
fn nr_feld(schritt: &Map<String, Value>) -> Result<u8, Ungueltig> {
    schritt
        .get("nr")
        .and_then(Value::as_i64)
        .and_then(|n| u8::try_from(n).ok())
        .ok_or_else(|| Ungueltig("Feld nr".to_owned()))
}

/// The bytes of a field `hex`: lower-case hex of even length.
pub(crate) fn hex_lesen(text: &str) -> Result<Vec<u8>, Ungueltig> {
    fn ziffer(b: u8) -> Option<u8> {
        match b {
            b'0'..=b'9' => Some(b - b'0'),
            b'a'..=b'f' => Some(b - b'a' + 10),
            _ => None,
        }
    }
    let b = text.as_bytes();
    if !b.len().is_multiple_of(2) {
        return Err("hex hat ungerade Laenge".into());
    }
    b.chunks(2)
        .map(|p| match (ziffer(p[0]), ziffer(p[1])) {
            (Some(h), Some(l)) => Ok((h << 4) + l),
            _ => Err(Ungueltig("hex ist kein Kleinbuchstaben-Hex".to_owned())),
        })
        .collect()
}

/// The lower-case hex of bytes.
pub(crate) fn hex_schreiben(bytes: &[u8]) -> String {
    const ZIFFERN: &[u8; 16] = b"0123456789abcdef";
    let mut aus = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        aus.push(char::from(ZIFFERN[usize::from(b >> 4)]));
        aus.push(char::from(ZIFFERN[usize::from(b & 15)]));
    }
    aus
}

/// The runs of the step `Laeufe`: `[[fg, bg, fl, text], ...]` of whole numbers and text.
fn laeufe_lesen(schritt: &Map<String, Value>) -> Result<Vec<(i64, i64, i64, String)>, Ungueltig> {
    let Some(Value::Array(liste)) = schritt.get("laeufe") else {
        return Err("laeufe ist keine Liste".into());
    };
    liste
        .iter()
        .map(|e| match e.as_array().map(Vec::as_slice) {
            Some([fg, bg, fl, Value::String(text)]) => {
                match (fg.as_i64(), bg.as_i64(), fl.as_i64()) {
                    (Some(fg), Some(bg), Some(fl)) => Ok((fg, bg, fl, text.clone())),
                    _ => Err(Ungueltig("Lauf ohne ganze Zahlen".to_owned())),
                }
            }
            _ => Err(Ungueltig(
                "Lauf hat nicht die Form [ganz, ganz, ganz, text]".to_owned(),
            )),
        })
        .collect()
}

/// The runs as JSON: `[[fg, bg, fl, text], ...]`.
fn laeufe_json(z: &Zeile) -> Value {
    Value::Array(
        z.iter()
            .map(|l| json!([l.stil.fg, l.stil.bg, l.stil.fl, l.text]))
            .collect(),
    )
}

fn cursor_json(c: Cursor) -> Value {
    json!([c.zeile, c.spalte, c.sichtbar])
}

fn auszug_json(a: &Auszug) -> Value {
    json!({
        "nr": a.nr,
        "stand": a.stand,
        "groesse": a.groesse.map(|(sp, ze)| json!([sp, ze])),
        "invers": a.invers,
        "schub": a.schub.map(|s| json!([s.oben, s.unten, s.n])),
        "cursor": a.cursor.map(cursor_json),
        "zeilen": a.zeilen.iter().map(|(i, z)| json!([i, zeile_schreiben(z)])).collect::<Vec<_>>(),
    })
}

fn abbild_json(nr: u8, b: &Abbild) -> Value {
    json!({
        "nr": nr,
        "stand": b.stand,
        "groesse": [b.spalten, b.zeilen],
        "invers": b.invers,
        "cursor": cursor_json(b.cursor),
        "zeilen": b.lines.iter().map(|z| zeile_schreiben(z)).collect::<Vec<_>>(),
    })
}

/// One step of a case.
fn schritt_ausfuehren<'a>(
    schritt: &'a Value,
    verbindungen: &mut HashMap<&'a str, Strecke>,
) -> Result<Value, Ungueltig> {
    let Some(schritt) = schritt.as_object() else {
        return Err("Schritt ist kein Objekt".into());
    };
    let art =
        text_feld(schritt, "art").map_err(|_| Ungueltig("Schritt ohne Text art".to_owned()))?;
    match art {
        "Zeile" => Ok(zeile_schritt(text_feld(schritt, "sgr")?)),
        "Laeufe" => Ok(laeufe_schritt(&laeufe_lesen(schritt)?)),
        "Verbinden" => {
            let name = text_feld(schritt, "verbindung")?;
            if verbindungen.contains_key(name) {
                return Err("Verbindung doppelt eingerichtet".into());
            }
            let Some(deflate) = schritt.get("deflate").and_then(Value::as_bool) else {
                return Err("Feld deflate".into());
            };
            verbindungen.insert(
                name,
                Strecke {
                    sender: Verbindung::neu(deflate),
                    empfang: Empfang::neu(deflate),
                },
            );
            Ok(json!({ "ok": true }))
        }
        "Nachricht" | "Rahmen" | "Vergessen" | "Trennen" => {
            let name = text_feld(schritt, "verbindung")?;
            if art == "Trennen" {
                return verbindungen
                    .remove(name)
                    .map(|_| json!({ "ok": true }))
                    .ok_or_else(|| Ungueltig("Verbindung unbekannt".to_owned()));
            }
            let Some(strecke) = verbindungen.get_mut(name) else {
                return Err("Verbindung unbekannt".into());
            };
            match art {
                "Vergessen" => {
                    let nr = nr_feld(schritt)?;
                    strecke.sender.vergessen(nr);
                    strecke.empfang.vergessen(nr);
                    Ok(json!({ "ok": true }))
                }
                "Nachricht" => {
                    let nr = nr_feld(schritt)?;
                    let Some(nachricht) = schritt.get("nachricht") else {
                        return Err("Feld nachricht".into());
                    };
                    Ok(nachricht_schritt(strecke, nr, nachricht))
                }
                _ => {
                    let wire = hex_lesen(text_feld(schritt, "hex")?)?;
                    Ok(rahmen_schritt(strecke, &wire))
                }
            }
        }
        _ => Err(Ungueltig(format!("art {art} unbekannt"))),
    }
}

/// The step `Zeile`: reads the line form and writes it back in the canonical form.
fn zeile_schritt(sgr: &str) -> Value {
    match zeile_lesen(sgr) {
        Err(f) => fehler_ergebnis(f),
        Ok(z) => {
            let kanon = zeile_schreiben(&z);
            let rundreise = zeile_lesen(&kanon).is_ok_and(|z2| z2 == z);
            json!({ "ok": true, "laeufe": laeufe_json(&z), "sgr": kanon, "rundreise": rundreise })
        }
    }
}

/// The step `Laeufe`: checks the runs, writes them and reads them again.
fn laeufe_schritt(roh: &[(i64, i64, i64, String)]) -> Value {
    match zeile_pruefen(roh) {
        Err(f) => fehler_ergebnis(f),
        Ok(z) => {
            let kanon = zeile_schreiben(&z);
            let rundreise = zeile_lesen(&kanon).is_ok_and(|z2| z2 == z);
            json!({ "ok": true, "sgr": kanon, "rundreise": rundreise })
        }
    }
}

/// The step `Nachricht`: the sender makes the frame, the receiver of the connection reads it.
fn nachricht_schritt(strecke: &mut Strecke, nr: u8, nachricht: &Value) -> Value {
    let roh = match strecke.sender.nimm_roh(nr, nachricht) {
        Err(f) => return fehler_ergebnis(f),
        Ok(roh) => roh,
    };
    let Some(roh) = roh else {
        let abbild = strecke.empfang.abbild(nr).map(|b| abbild_json(nr, b));
        return json!({
            "ok": true, "hex": null, "auszug": null, "abbild": abbild,
            "rundreise": true, "leitung_hex": null, "leitung_laenge": 0,
        });
    };
    let wire = strecke.sender.verpacken(&roh);
    match strecke.empfang.nimm(&wire) {
        Err(f) => fehler_ergebnis(f),
        Ok((auszug, neu)) => {
            let rundreise = strecke.sender.vorbild(nr).is_some_and(|v| v.gleicht(neu));
            json!({
                "ok": true, "hex": hex_schreiben(&roh), "auszug": auszug_json(&auszug),
                "abbild": abbild_json(nr, neu), "rundreise": rundreise,
                "leitung_hex": hex_schreiben(&wire), "leitung_laenge": wire.len(),
            })
        }
    }
}

/// The step `Rahmen`: raw bytes for the receiver of the connection.
fn rahmen_schritt(strecke: &mut Strecke, wire: &[u8]) -> Value {
    match strecke.empfang.nimm_roh(wire) {
        Err(f) => fehler_ergebnis(f),
        Ok((roh, auszug, neu)) => json!({
            "ok": true, "hex": hex_schreiben(&roh), "auszug": auszug_json(&auszug),
            "abbild": abbild_json(auszug.nr, neu),
        }),
    }
}
