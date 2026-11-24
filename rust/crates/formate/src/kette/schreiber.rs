//! Appending a line to a chain file: one exclusive lock on the handle, the last line read under the lock,
//! line and line feed in one write, then `sync_data`.

use std::fs::{File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::Path;

#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;

use super::hash::sha256_hex;
use super::{Fehler, Gesperrt, NULL_HASH, zeile_gesperrt};

pub(super) fn ein_ausgabe(fehler: &io::Error) -> Fehler {
    Fehler::EinAusgabe(fehler.to_string())
}

/// The last line of the file (the bytes before its line feed), or `None` for an empty file.
/// The file must end with a line feed; reads from the end, in chunks that double, so that a long chain costs nothing.
pub(super) fn letzte_zeile(datei: &mut File, laenge: u64) -> Result<Option<Vec<u8>>, Fehler> {
    if laenge == 0 {
        return Ok(None);
    }
    let mut ende = laenge;
    let mut groesse: u64 = 4096;
    let mut gelesen: Vec<u8> = Vec::new();
    // The chunk doubles each time, so 64 rounds reach any file length; the bound only keeps the loop finite.
    for _ in 0..64 {
        let anfang = ende.saturating_sub(groesse);
        let mut stueck = vec![0u8; usize::try_from(ende - anfang).unwrap_or(0)];
        datei
            .seek(SeekFrom::Start(anfang))
            .and_then(|_| datei.read_exact(&mut stueck))
            .map_err(|e| ein_ausgabe(&e))?;
        stueck.extend_from_slice(&gelesen);
        gelesen = stueck;
        // The last byte of the file must be the line feed; it is not part of the line.
        let Some((&letztes, zeilen)) = gelesen.split_last() else {
            return Ok(None);
        };
        if ende == laenge && letztes != b'\n' {
            return Err(Fehler::LetzteZeileUnvollstaendig);
        }
        if let Some(stelle) = zeilen.iter().rposition(|b| *b == b'\n') {
            return Ok(Some(zeilen[stelle + 1..].to_vec()));
        }
        if anfang == 0 {
            return Ok(Some(zeilen.to_vec()));
        }
        ende = anfang;
        groesse = groesse.saturating_mul(2);
    }
    Err(Fehler::EinAusgabe("letzte Zeile nicht gefunden".to_owned()))
}

/// The sha256 of the last line of the file, or 64 zeros for an empty file.
fn vorher_lesen(datei: &mut File, laenge: u64) -> Result<String, Fehler> {
    Ok(letzte_zeile(datei, laenge)?
        .map_or_else(|| NULL_HASH.to_owned(), |zeile| sha256_hex(&zeile)))
}

/// Reads the last line, builds the line and appends it. The lock is held by the caller.
fn unter_sperre(datei: &mut File, g: &Gesperrt) -> Result<String, Fehler> {
    let laenge = datei.metadata().map_err(|e| ein_ausgabe(&e))?.len();
    let vorher = vorher_lesen(datei, laenge)?;
    let zeile = zeile_gesperrt(g, &vorher)?;
    let mut bytes = zeile.clone().into_bytes();
    bytes.push(b'\n');
    if let Err(fehler) = datei.write_all(&bytes).and_then(|()| datei.sync_data()) {
        // A half written line must not stay: back to the length before the write (best effort).
        let _ = datei.set_len(laenge);
        return Err(ein_ausgabe(&fehler));
    }
    Ok(sha256_hex(zeile.as_bytes()))
}

/// Appends the line `ausgabe.gesperrt` and a line feed to the chain file and returns the sha256 of the line.
///
/// The fields are checked first. The file is opened for reading and appending (a new file gets the mode 0600
/// on Unix) and locked exclusively with `File::lock` (`flock` on Unix, `LockFileEx` on Windows). Under the
/// lock the last line gives `vorher`; an empty file gives 64 zeros. After an error the file is unchanged.
///
/// # Errors
/// As [`zeile_gesperrt`]; `letzte_zeile_unvollstaendig` when the file is not empty and does not end with a
/// line feed; `ein_ausgabe` when the file cannot be opened, locked, read or written.
pub fn anhaengen_gesperrt(datei: &Path, g: &Gesperrt) -> Result<String, Fehler> {
    zeile_gesperrt(g, NULL_HASH)?;
    let mut optionen = OpenOptions::new();
    optionen.read(true).append(true).create(true);
    #[cfg(unix)]
    optionen.mode(0o600);
    let mut handle = optionen.open(datei).map_err(|e| ein_ausgabe(&e))?;
    handle.lock().map_err(|e| ein_ausgabe(&e))?;
    let ergebnis = unter_sperre(&mut handle, g);
    // Closing the handle also releases the lock; the explicit call keeps the order of the contract.
    let _ = handle.unlock();
    ergebnis
}
