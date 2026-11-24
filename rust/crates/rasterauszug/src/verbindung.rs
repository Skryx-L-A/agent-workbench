//! The sender side of one connection (one viewer): the model pictures per session number, the extract of a
//! message of the core and the Deflate context (contract rules RA5, RA6, RA8, RA12, RA13).

use std::collections::HashMap;

use serde_json::{Map, Value};

use crate::bild::{Auszug, Bild, Cursor, GROESSE_MAX, SPALTEN_MIN, ZEILEN_MIN};
use crate::deflate::Verpacker;
use crate::fehler::Fehler;
use crate::rahmen::{cursor_pruefen, rahmen_kodieren};
use crate::schub::unterschied;
use crate::zeile::{Zeile, zeile_lesen};

/// The sender of one connection. It holds one model picture per session number; the number the caller passes
/// decides, the name of the session is not read (RA12).
pub struct Verbindung {
    vorbild: HashMap<u8, Bild>,
    verpacker: Option<Verpacker>,
    deflate: bool,
}

impl Verbindung {
    /// A new connection without any model picture. With `deflate` the frames on the wire go through one raw
    /// Deflate stream per connection (RA11), created at the first frame.
    #[must_use]
    pub fn neu(deflate: bool) -> Verbindung {
        Verbindung {
            vorbild: HashMap::new(),
            verpacker: None,
            deflate,
        }
    }

    /// The bytes on the wire for a message of the core, or `None` when there is nothing to send (RA6).
    ///
    /// # Errors
    ///
    /// Every error of rule RA12 and RA5; a rejected message changes nothing.
    pub fn nimm(&mut self, nr: u8, nachricht: &Value) -> Result<Option<Vec<u8>>, Fehler> {
        Ok(self
            .nimm_roh(nr, nachricht)?
            .map(|roh| self.verpacken(&roh)))
    }

    /// Like [`Verbindung::nimm`], but returns the frame before Deflate.
    ///
    /// # Errors
    ///
    /// See [`Verbindung::nimm`].
    pub fn nimm_roh(&mut self, nr: u8, nachricht: &Value) -> Result<Option<Vec<u8>>, Fehler> {
        let msg = nachricht.as_object().ok_or(Fehler::NachrichtUngueltig)?;
        match msg.get("art") {
            Some(Value::String(art)) if art == "Abzug" => self.abzug(nr, msg).map(Some),
            Some(Value::String(art)) if art == "Rasteraenderung" => self.aenderung(nr, msg),
            Some(Value::String(_)) => Err(Fehler::ArtUnbekannt),
            _ => Err(Fehler::NachrichtUngueltig),
        }
    }

    /// The bytes on the wire for a frame: the frame itself, or its Deflate form.
    pub fn verpacken(&mut self, roh: &[u8]) -> Vec<u8> {
        if !self.deflate {
            return roh.to_vec();
        }
        self.verpacker
            .get_or_insert_with(Verpacker::neu)
            .verpacken(roh)
    }

    /// The model picture of a session number.
    #[must_use]
    pub fn vorbild(&self, nr: u8) -> Option<&Bild> {
        self.vorbild.get(&nr)
    }

    /// Forgets the model picture of a session number; a number without one is no error (RA13).
    pub fn vergessen(&mut self, nr: u8) {
        self.vorbild.remove(&nr);
    }

    /// A message `Abzug` makes a full picture and replaces the model picture (RA5).
    fn abzug(&mut self, nr: u8, msg: &Map<String, Value>) -> Result<Vec<u8>, Fehler> {
        let stand = stand_feld(msg, "stand")?;
        let (spalten, zeilen) = groesse_feld(msg)?;
        let invers = bool_feld(msg, "invers")?;
        let Some(Value::Array(texte)) = msg.get("zeilen") else {
            return Err(Fehler::NachrichtUngueltig);
        };
        text_feld(msg, "sitzung")?;
        let texte: Vec<&str> = texte
            .iter()
            .map(|t| t.as_str().ok_or(Fehler::NachrichtUngueltig))
            .collect::<Result<_, _>>()?;
        let (spalten, zeilen) = groesse_pruefen(spalten, zeilen)?;
        if texte.len() != zeilen {
            return Err(Fehler::NachrichtUngueltig);
        }
        let cursor = cursor_feld(msg)?;
        if let Some(c) = cursor {
            cursor_pruefen(c.0, c.1, spalten, zeilen)?;
        }
        let lines: Vec<Zeile> = texte
            .iter()
            .map(|t| zeile_lesen(t).map_err(|_| Fehler::ZeileUngueltig))
            .collect::<Result<_, _>>()?;
        let cursor = cursor.map(cursor_aus);
        let auszug = Auszug {
            nr,
            stand,
            groesse: Some((spalten, zeilen)),
            invers,
            schub: None,
            cursor,
            zeilen: lines
                .iter()
                .enumerate()
                .filter(|(_, z)| !z.is_empty())
                .map(|(i, z)| (i, z.clone()))
                .collect(),
        };
        self.vorbild.insert(
            nr,
            Bild {
                spalten,
                zeilen,
                lines,
                invers,
                cursor: cursor.unwrap_or_default(),
                stand,
            },
        );
        Ok(rahmen_kodieren(&auszug))
    }

    /// A message `Rasteraenderung` is computed against the model picture (RA6, RA7, RA8).
    fn aenderung(&mut self, nr: u8, msg: &Map<String, Value>) -> Result<Option<Vec<u8>>, Fehler> {
        text_feld(msg, "sitzung")?;
        let von = stand_feld(msg, "von")?;
        let stand = stand_feld(msg, "stand")?;
        let invers = bool_feld(msg, "invers")?;
        let Some(Value::Object(zeilen)) = msg.get("zeilen") else {
            return Err(Fehler::NachrichtUngueltig);
        };
        let texte: Vec<(&str, &str)> = zeilen
            .iter()
            .map(|(k, t)| {
                t.as_str()
                    .map(|t| (k.as_str(), t))
                    .ok_or(Fehler::NachrichtUngueltig)
            })
            .collect::<Result<_, _>>()?;
        let v = self.vorbild.get_mut(&nr).ok_or(Fehler::KeinAbzug)?;
        if von != v.stand {
            return Err(Fehler::Luecke);
        }
        if stand <= von {
            return Err(Fehler::StandAlt);
        }
        let cursor = cursor_feld(msg)?;
        if let Some(c) = cursor {
            cursor_pruefen(c.0, c.1, v.spalten, v.zeilen)?;
        }
        let cursor = cursor.map(cursor_aus);
        let mut neu_lines = v.lines.clone();
        for (schluessel, text) in texte {
            let i = zeilenindex(schluessel, v.zeilen)?;
            neu_lines[i] = zeile_lesen(text).map_err(|_| Fehler::ZeileUngueltig)?;
        }
        let (schub, geaendert) = unterschied(&v.lines, &neu_lines);
        let cursor_aenderung = cursor.filter(|c| *c != v.cursor);
        let auszug = Auszug {
            nr,
            stand,
            groesse: None,
            invers,
            schub,
            cursor: cursor_aenderung,
            zeilen: geaendert
                .into_iter()
                .map(|i| (i, neu_lines[i].clone()))
                .collect(),
        };
        let nichts = auszug.zeilen.is_empty() && schub.is_none() && cursor_aenderung.is_none();
        let rahmen = (!nichts || invers != v.invers).then(|| rahmen_kodieren(&auszug));
        v.lines = neu_lines;
        v.invers = invers;
        if let Some(c) = cursor {
            v.cursor = c;
        }
        v.stand = stand;
        Ok(rahmen)
    }
}

/// A whole number field of a message, from 0 to `i64::MAX`.
fn stand_feld(msg: &Map<String, Value>, name: &str) -> Result<u64, Fehler> {
    msg.get(name)
        .and_then(Value::as_i64)
        .and_then(|n| u64::try_from(n).ok())
        .ok_or(Fehler::NachrichtUngueltig)
}

fn bool_feld(msg: &Map<String, Value>, name: &str) -> Result<bool, Fehler> {
    msg.get(name)
        .and_then(Value::as_bool)
        .ok_or(Fehler::NachrichtUngueltig)
}

fn text_feld<'a>(msg: &'a Map<String, Value>, name: &str) -> Result<&'a str, Fehler> {
    msg.get(name)
        .and_then(Value::as_str)
        .ok_or(Fehler::NachrichtUngueltig)
}

/// The field `groesse`: a list of two whole numbers (columns, rows), not yet range checked.
fn groesse_feld(msg: &Map<String, Value>) -> Result<(i64, i64), Fehler> {
    match msg.get("groesse") {
        Some(Value::Array(g)) if g.len() == 2 => match (g[0].as_i64(), g[1].as_i64()) {
            (Some(sp), Some(ze)) => Ok((sp, ze)),
            _ => Err(Fehler::NachrichtUngueltig),
        },
        _ => Err(Fehler::NachrichtUngueltig),
    }
}

/// A size of 2 to 1000 columns and 1 to 1000 rows (RA5).
fn groesse_pruefen(spalten: i64, zeilen: i64) -> Result<(usize, usize), Fehler> {
    let gueltig = |n: i64, von: usize| {
        usize::try_from(n)
            .ok()
            .filter(|n| (von..=GROESSE_MAX).contains(n))
    };
    match (gueltig(spalten, SPALTEN_MIN), gueltig(zeilen, ZEILEN_MIN)) {
        (Some(sp), Some(ze)) => Ok((sp, ze)),
        _ => Err(Fehler::GroesseUngueltig),
    }
}

/// The optional field `cursor`: `[row, column, shown]`; the numbers are not yet checked against the size. A wrong
/// shape is [`Fehler::NachrichtUngueltig`]. A negative number is returned as a huge one so that the size check
/// rejects it as [`Fehler::CursorUngueltig`].
fn cursor_feld(msg: &Map<String, Value>) -> Result<Option<(u64, u64, bool)>, Fehler> {
    let Some(c) = msg.get("cursor") else {
        return Ok(None);
    };
    let Value::Array(c) = c else {
        return Err(Fehler::NachrichtUngueltig);
    };
    match c.as_slice() {
        [z, s, Value::Bool(sichtbar)] => match (z.as_i64(), s.as_i64()) {
            (Some(z), Some(s)) => Ok(Some((
                u64::try_from(z).unwrap_or(u64::MAX),
                u64::try_from(s).unwrap_or(u64::MAX),
                *sichtbar,
            ))),
            _ => Err(Fehler::NachrichtUngueltig),
        },
        _ => Err(Fehler::NachrichtUngueltig),
    }
}

/// The cursor of a checked triple.
fn cursor_aus(c: (u64, u64, bool)) -> Cursor {
    Cursor {
        zeile: usize::try_from(c.0).unwrap_or(0),
        spalte: usize::try_from(c.1).unwrap_or(0),
        sichtbar: c.2,
    }
}

/// A row key of a change: decimal digits (ASCII) without a leading zero (except `0`), below the number of rows.
fn zeilenindex(schluessel: &str, zeilen: usize) -> Result<usize, Fehler> {
    let ziffern = !schluessel.is_empty() && schluessel.bytes().all(|b| b.is_ascii_digit());
    let fuehrende_null = schluessel.len() > 1 && schluessel.starts_with('0');
    if !ziffern || fuehrende_null {
        return Err(Fehler::ZeileAusserhalb);
    }
    schluessel
        .parse::<usize>()
        .ok()
        .filter(|i| *i < zeilen)
        .ok_or(Fehler::ZeileAusserhalb)
}
