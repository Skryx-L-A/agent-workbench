//! The state core of the new interface: events in, effects out.
//!
//! The library holds the logic only. It reads no file, starts no process, opens no socket and asks for
//! neither the time nor a random number: the only clock is the field `t` of each event. The program `zk`
//! (`src/main.rs`) reads one case and writes the effects; the contract is `rust/spec/VERTRAEGE.md` of the contract
//! package, rules R1 to R14. A service that runs for a whole session uses [`Sitzungskern`]: one event per
//! [`Sitzungskern::schritt`], and [`Sitzungskern::naechste_frist`] says when the core wants the next `Takt`;
//! [`laufen`] is built on it.

mod anfang;
mod base64;
mod eingabe;
mod ereignis;
mod kern;
mod regel;
mod schritt;
mod wert;

#[cfg(test)]
mod tests;

use std::fmt;

use serde_json::{Map, Value};

use crate::schritt::Abbruch;

pub use crate::schritt::{Sitzungskern, Wirkungen};

/// The most bytes of a case the program reads.
pub const EINGABE_MAX: usize = 4_194_304;

/// The most bytes (serialised) all messages to the viewers may hold. A case beyond it is invalid: a small
/// case can make a message to many viewers per event, so the output is not bounded by the input.
pub const NACHRICHTEN_MAX: usize = 33_554_432;

/// The case is invalid: the program ends with exit code 2 and writes nothing to stdout.
#[derive(Debug, PartialEq, Eq)]
pub struct Ungueltig(pub String);

impl fmt::Display for Ungueltig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Fall ungueltig: {}", self.0)
    }
}

impl From<String> for Ungueltig {
    fn from(grund: String) -> Self {
        Self(grund)
    }
}

impl From<&str> for Ungueltig {
    fn from(grund: &str) -> Self {
        Self(grund.to_owned())
    }
}

/// Runs a case given as JSON bytes and returns the effects.
///
/// # Errors
///
/// [`Ungueltig`] when the input is longer than [`EINGABE_MAX`], is no JSON object, lacks `ereignisse`, has a
/// malformed start, an event without a whole number `t` or with a falling `t`, or when the messages pass
/// [`NACHRICHTEN_MAX`].
pub fn laufen(eingabe: &[u8]) -> Result<Value, Ungueltig> {
    laufen_mit(eingabe, NACHRICHTEN_MAX)
}

/// Like [`laufen`] with another limit for the bytes of all messages.
///
/// # Errors
///
/// See [`laufen`].
pub fn laufen_mit(eingabe: &[u8], nachrichten_max: usize) -> Result<Value, Ungueltig> {
    if eingabe.len() > EINGABE_MAX {
        return Err(Ungueltig(format!("mehr als {EINGABE_MAX} Byte")));
    }
    let fall: Value =
        serde_json::from_slice(eingabe).map_err(|e| Ungueltig(format!("kein JSON: {e}")))?;
    let Value::Object(fall) = fall else {
        return Err("Fall ist kein Objekt".into());
    };
    let leer = Map::new();
    let anfang = match fall.get("anfang") {
        None => &leer,
        Some(Value::Object(anfang)) => anfang,
        Some(_) => return Err("anfang ist kein Objekt".into()),
    };
    let Some(Value::Array(ereignisse)) = fall.get("ereignisse") else {
        return Err("ereignisse fehlt oder ist keine Liste".into());
    };
    let ereignisse = ereignisse_pruefen(ereignisse)?;
    let mut kern = Sitzungskern::aus_objekt(anfang, nachrichten_max)?;
    let mut wirkungen = Wirkungen::default();
    let mut bytes = 0_usize;
    for (e, t) in ereignisse {
        // The limit counts the messages of the whole case here; a step gets what is left of it.
        let (schritt, schritt_bytes) = kern
            .schritt_geprueft(e, t, nachrichten_max - bytes)
            .map_err(|abbruch| match abbruch {
                Abbruch::Ungueltig(grund) => grund,
                Abbruch::Ueberlauf => {
                    Ungueltig(format!("Nachrichten uebersteigen {nachrichten_max} Byte"))
                }
            })?;
        bytes += schritt_bytes;
        wirkungen.anhaengen(schritt);
    }
    let mut aus = wirkungen.als_wert();
    if let Value::Object(map) = &mut aus {
        map.insert("zustand".to_owned(), kern.zustand());
    }
    Ok(aus)
}

/// An event with its time.
type Zeitpunkt<'a> = (&'a Map<String, Value>, i64);

/// The event at position `i` with its time: an object with a whole number `t` that does not fall below `vorher`.
pub(crate) fn ereignis_pruefen(
    i: usize,
    ereignis: &Value,
    vorher: i64,
) -> Result<Zeitpunkt<'_>, Ungueltig> {
    ereignis
        .as_object()
        .and_then(|o| o.get("t").and_then(Value::as_i64).map(|t| (o, t)))
        .filter(|(_, t)| *t >= vorher)
        .ok_or_else(|| Ungueltig(format!("Ereignis {i}: kein Objekt, t fehlt oder faellt")))
}

/// Every event with its time; each must be an object with a whole number `t` that does not fall below the one
/// before (the first below 0).
fn ereignisse_pruefen(ereignisse: &[Value]) -> Result<Vec<Zeitpunkt<'_>>, Ungueltig> {
    let mut vorher = 0;
    let mut aus = Vec::with_capacity(ereignisse.len());
    for (i, e) in ereignisse.iter().enumerate() {
        let eintrag = ereignis_pruefen(i, e, vorher)?;
        vorher = eintrag.1;
        aus.push(eintrag);
    }
    Ok(aus)
}
