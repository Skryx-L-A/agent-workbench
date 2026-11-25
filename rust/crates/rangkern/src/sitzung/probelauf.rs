//! The counting of the markers in the probe-run mode (contract 4, section "Zählung der Marker (Probelauf)").
//!
//! Only counts are kept, no byte of an answer is written. For every answer to `search` the process counts how often each
//! marker stands byte for byte in the buffer, shortly before the buffer goes to the socket, and writes the counts to a file
//! of mode 0600, atomically through a file beside it, in the form of RFC 8785.
//!
//! Under Windows the file goes through its path (there is no held folder without `unsafe`): a file beside it, then
//! `rename`. Its rights follow the folder (an ACL of its own is open in the addendum for Windows, section 7), and the removal
//! after a failed start removes the file at the path without a check of its identity.

#[cfg(unix)]
use std::ffi::OsString;
use std::io::{self, Write};
use std::path::Path;
use std::sync::{Mutex, PoisonError};

use formate::kette::kanonisch;
#[cfg(unix)]
use rustix::fs::{Access, AtFlags, FileType, Mode};
use serde_json::{Map, Value, json};

#[cfg(unix)]
use super::loeschen::{Kennung, Ordner};

/// Under Windows the file this process wrote is known by its path only: the mark that it wrote one.
#[cfg(windows)]
#[derive(Debug)]
struct Kennung;

/// The fixed line on the error output before the readiness line.
pub const STDERR_ZEILE: &str = "such.probelauf: Marker werden gezählt";

/// The check of the count file before the socket (contract 4, table of the start: 78 comes before 74; decision of 23.11.2026,
/// W3): its folder exists and is no link, the process may write into it, and at the name stands nothing or a plain file, no
/// link. The file itself is written only after the socket.
///
/// # Errors
///
/// The folder is missing, a link or not writable, or the name shows a link, a folder or anything else but a plain file.
#[cfg(unix)]
pub fn zaehldatei_pruefen(datei: &Path) -> io::Result<()> {
    let (ordner, name) = Ordner::von(datei)?;
    if ordner
        .kennung(&name)
        .is_some_and(|da| !da.ist(FileType::RegularFile))
    {
        return Err(io::Error::other(
            "the count file is no plain file (a link does not count)",
        ));
    }
    rustix::fs::accessat(
        &ordner,
        ".",
        Access::WRITE_OK.union(Access::EXEC_OK),
        AtFlags::EACCESS,
    )?;
    Ok(())
}

/// The check of the count file under Windows: its folder exists and is no link, and at the name stands nothing or a plain
/// file. Whether the process may write there shows only the first write, after the pipe (a failure is exit 78 then, too).
///
/// # Errors
///
/// The folder is missing or a link, or the name shows a link, a folder or anything else but a plain file.
#[cfg(windows)]
pub fn zaehldatei_pruefen(datei: &Path) -> io::Result<()> {
    let eltern = match datei.parent() {
        Some(eltern) if !eltern.as_os_str().is_empty() => eltern,
        _ => Path::new("."),
    };
    let ordner = std::fs::symlink_metadata(eltern)?;
    if !ordner.is_dir() {
        return Err(io::Error::other(
            "the folder of the count file is no folder (a link does not count)",
        ));
    }
    match std::fs::symlink_metadata(datei) {
        Ok(da) if !da.file_type().is_file() => Err(io::Error::other(
            "the count file is no plain file (a link does not count)",
        )),
        Ok(_) => Ok(()),
        Err(fehler) if fehler.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(fehler) => Err(fehler),
    }
}

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
    /// Kind, device and inode of the file this process wrote last: only that file is removed again (review B8).
    datei: Option<Kennung>,
}

/// The counting of one process.
#[derive(Debug)]
pub struct Zaehlung {
    marker: Vec<String>,
    /// The folder of the file, opened once; the file and the file beside it are only ever reached by a name in it.
    #[cfg(unix)]
    ordner: Ordner,
    #[cfg(unix)]
    name: OsString,
    /// Windows: the path of the file.
    #[cfg(windows)]
    pfad: std::path::PathBuf,
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
            datei: None,
        };
        #[cfg(unix)]
        let (ordner, name) = Ordner::von(datei.as_ref())?;
        let zaehlung = Self {
            marker,
            #[cfg(unix)]
            ordner,
            #[cfg(unix)]
            name,
            #[cfg(windows)]
            pfad: datei.as_ref().to_path_buf(),
            sitzung,
            stand: Mutex::new(stand),
        };
        {
            let mut stand = zaehlung.stand();
            stand.datei = Some(zaehlung.schreiben(&stand)?);
        }
        Ok(zaehlung)
    }

    /// Removes the count file if its name still shows the file this process wrote last; `true` if it was removed. For a start
    /// that fails after the file was written (review B8): nothing of the failed start stays. Anything else under the name stays.
    #[must_use]
    pub fn entfernen(&self) -> bool {
        let stand = self.stand();
        #[cfg(unix)]
        let entfernt = stand
            .datei
            .as_ref()
            .is_some_and(|soll| self.ordner.entfernen_wenn(&self.name, soll));
        #[cfg(windows)]
        let entfernt = stand.datei.is_some() && std::fs::remove_file(&self.pfad).is_ok();
        entfernt
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
        let mut neu = Stand {
            antworten: stand.antworten + 1,
            anzahl: self
                .marker
                .iter()
                .zip(&stand.anzahl)
                .map(|(marker, bisher)| bisher + zaehle(puffer, marker.as_bytes()))
                .collect(),
            datei: None,
        };
        neu.datei = Some(self.schreiben(&neu)?);
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
    /// folder; on a failure the file beside it is deleted only if it is still the one this call made. Returns the kind, device
    /// and inode of the file now under the name.
    #[cfg(unix)]
    fn schreiben(&self, stand: &Stand) -> io::Result<Kennung> {
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
        geschrieben.map(|()| soll)
    }
}

#[cfg(windows)]
impl Zaehlung {
    /// Writes the file atomically under Windows: a new file beside it, then `rename` over the name (which replaces a file
    /// there). On a failure the file beside it is deleted again.
    fn schreiben(&self, stand: &Stand) -> io::Result<Kennung> {
        let text = self.text(stand)?;
        let mut name = self.pfad.as_os_str().to_owned();
        name.push(format!(".{}.neu", std::process::id()));
        let daneben = std::path::PathBuf::from(name);
        let mut datei = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&daneben)?;
        let geschrieben = datei
            .write_all(text.as_bytes())
            .and_then(|()| datei.sync_data());
        drop(datei);
        let geschrieben = geschrieben.and_then(|()| std::fs::rename(&daneben, &self.pfad));
        if geschrieben.is_err() {
            let _ = std::fs::remove_file(&daneben);
        }
        geschrieben.map(|()| Kennung)
    }
}

/// The name of the file beside `name` that is written first.
#[cfg(unix)]
fn daneben(name: &OsString) -> OsString {
    let mut daneben = name.clone();
    daneben.push(format!(".{}.neu", std::process::id()));
    daneben
}
