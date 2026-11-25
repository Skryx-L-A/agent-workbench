//! The rights of the token folder and of the files of the bridge under Windows (Nachtrag 25.11.2026, contract `bruecke`, section
//! of the platforms). The Unix form is `rechte.rs`; the calls of Windows are in `sys/windows.rs`.
//!
//! The folder of a file exists, is a folder and no reparse point, belongs to the own user and has a protected access list that
//! allows only the own user and `SYSTEM` (the counterpart of mode 0700). The file must not exist. The bridge makes it new with a
//! protected list of the own user alone (the counterpart of mode 0600) and removes it at the end only while the path still names it.

use std::fs;
use std::io;
use std::path::Path;

use crate::fehler::StartFehler;
use crate::sys::{Datei, OrdnerFehler};

fn rechte_fehler(text: &'static str) -> StartFehler {
    StartFehler { code: 3, text }
}

/// Checks the folder of `pfad` and that `pfad` does not exist (exit code 3).
pub(crate) fn pruefen(pfad: &str) -> Result<(), StartFehler> {
    let ordner = Path::new(pfad)
        .parent()
        .ok_or_else(|| rechte_fehler("Ordner der Datei fehlt"))?;
    crate::sys::ordner_pruefen(ordner).map_err(|f| {
        rechte_fehler(match f {
            OrdnerFehler::Fehlt => "Ordner der Datei fehlt",
            OrdnerFehler::Link => "Ordner der Datei ist ein symbolischer Link",
            OrdnerFehler::KeinOrdner => "Ordner der Datei ist kein Ordner",
            OrdnerFehler::Fremd => "Ordner der Datei gehoert einem anderen Benutzer",
            OrdnerFehler::Offen => "Ordner der Datei hat nicht den Modus 0700",
        })
    })?;
    match fs::symlink_metadata(pfad) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        _ => Err(rechte_fehler("Datei besteht schon oder ist nicht pruefbar")),
    }
}

/// A file the bridge made; [`Angelegt::entfernen`] removes it while the path still names it.
#[derive(Debug)]
pub(crate) struct Angelegt(Datei);

impl Angelegt {
    /// Removes the file if it is still the one the bridge made.
    pub(crate) fn entfernen(&self) -> bool {
        self.0.entfernen()
    }
}

/// Makes the file `pfad` new with the list of the own user and writes `inhalt` (exit code 3 on every error).
pub(crate) fn anlegen(pfad: &str, inhalt: &[u8]) -> Result<Angelegt, StartFehler> {
    let p = Path::new(pfad);
    let name_gut = p.file_name().and_then(|n| n.to_str()).is_some_and(|n| {
        !n.is_empty() && !n.contains(['/', '\\', ':', '\0']) && n != "." && n != ".."
    });
    if !name_gut {
        return Err(rechte_fehler("Datei laesst sich nicht anlegen"));
    }
    crate::sys::datei_anlegen(p, inhalt)
        .map(Angelegt)
        .map_err(|_| rechte_fehler("Datei laesst sich nicht anlegen"))
}
