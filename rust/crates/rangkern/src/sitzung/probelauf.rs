//! The counting of the markers in the probe-run mode (contract 4, section "Zählung der Marker (Probelauf)").
//!
//! Only counts are kept, no byte of an answer is written. For every answer to `search` the process counts how often each
//! marker stands byte for byte in the buffer, shortly before the buffer goes to the socket, and writes the counts to a file
//! of mode 0600, atomically through a file beside it, in the form of RFC 8785.

use std::ffi::OsString;
use std::io::{self, Write};
use std::path::Path;
use std::sync::{Mutex, PoisonError};

use formate::kette::kanonisch;
use rustix::fs::Mode;
use serde_json::{Map, Value, json};

use super::loeschen::{Kennung, Ordner};

/// The fixed line on the error output before the readiness line.
pub const STDERR_ZEILE: &str = "such.probelauf: Marker werden gezählt";

/// How often `marker` stands in `puffer`, from the left, with no overlap.
#[must_use]
pub fn zaehle(puffer: &[u8], marker: &[u8]) -> u64 {
    if marker.is_empty() {
        return 0;
    }
    let mut anzahl = 0;
    // The first position where a new hit may start: right after the last one.
    let mut frei_ab = 0;
    for (at, fenster) in puffer.windows(marker.len()).enumerate() {
        if at >= frei_ab && fenster == marker {
            anzahl += 1;
            frei_ab = at + marker.len();
        }
    }
    anzahl
}

/// The counts: the number of answers and the count of every marker.
#[derive(Debug)]
struct Stand {
    antworten: u64,
    anzahl: Vec<u64>,
}

/// The counting of one process.
#[derive(Debug)]
pub struct Zaehlung {
    marker: Vec<String>,
    /// The folder of the file, opened once; the file and the file beside it are only ever reached by a name in it.
    ordner: Ordner,
    name: OsString,
    sitzung: String,
    stand: Mutex<Stand>,
}

impl Zaehlung {
    /// The counting with the markers at zero; it writes the file with the count 0 first.
    ///
    /// # Errors
    ///
    /// The error of the write: the folder of the file is missing, or it cannot be written.
    pub fn anlegen(
        datei: impl AsRef<Path>,
        sitzung: String,
        marker: Vec<String>,
    ) -> io::Result<Self> {
        let stand = Stand {
            antworten: 0,
            anzahl: vec![0; marker.len()],
        };
        let (ordner, name) = Ordner::von(datei.as_ref())?;
        let zaehlung = Self {
            marker,
            ordner,
            name,
            sitzung,
            stand: Mutex::new(stand),
        };
        zaehlung.schreiben(&zaehlung.stand())?;
        Ok(zaehlung)
    }

    /// The number of markers.
    #[must_use]
    pub fn anzahl_marker(&self) -> usize {
        self.marker.len()
    }

    fn stand(&self) -> std::sync::MutexGuard<'_, Stand> {
        self.stand.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Counts the markers in the buffer of one answer to `search` and writes the file. If the file cannot be written, the counts
    /// stay as they were.
    ///
    /// # Errors
    ///
    /// The error of the write.
    pub fn zaehlen(&self, puffer: &[u8]) -> io::Result<()> {
        let mut stand = self.stand();
        let neu = Stand {
            antworten: stand.antworten + 1,
            anzahl: self
                .marker
                .iter()
                .zip(&stand.anzahl)
                .map(|(marker, bisher)| bisher + zaehle(puffer, marker.as_bytes()))
                .collect(),
        };
        self.schreiben(&neu)?;
        *stand = neu;
        Ok(())
    }

    /// The text of the file for a state: `{"antworten":n,"marker":{…},"sitzung":"…"}` in RFC 8785.
    fn text(&self, stand: &Stand) -> io::Result<String> {
        let marker: Map<String, Value> = self
            .marker
            .iter()
            .zip(&stand.anzahl)
            .map(|(marker, anzahl)| (marker.clone(), json!(anzahl)))
            .collect();
        kanonisch(&json!({
            "antworten": stand.antworten,
            "marker": marker,
            "sitzung": self.sitzung,
        }))
        .map_err(|fehler| io::Error::other(fehler.code()))
    }

    /// Writes the file atomically: a file beside it with mode 0600, then rename. Everything goes relative to the open
    /// folder; on a failure the file beside it is deleted only if it is still the one this call made.
    fn schreiben(&self, stand: &Stand) -> io::Result<()> {
        let text = self.text(stand)?;
        let daneben = daneben(&self.name);
        let mut datei = self
            .ordner
            .neu_anlegen(&daneben, Mode::from_raw_mode(0o600))?;
        let soll = Kennung::von_datei(&datei)?;
        let geschrieben = datei
            .write_all(text.as_bytes())
            .and_then(|()| datei.sync_data())
            .and_then(|()| self.ordner.umbenennen_wenn(&daneben, &self.name, &soll));
        if geschrieben.is_err() {
            let _ = self.ordner.entfernen_wenn(&daneben, &soll);
        }
        geschrieben
    }
}

/// The name of the file beside `name` that is written first.
fn daneben(name: &OsString) -> OsString {
    let mut daneben = name.clone();
    daneben.push(format!(".{}.neu", std::process::id()));
    daneben
}
