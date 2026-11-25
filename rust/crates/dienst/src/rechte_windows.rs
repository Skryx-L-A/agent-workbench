//! DI2 under Windows (Nachtrag 25.11.2026, contract `dienst`, section 21): the rights of the folder and of the socket path. The Unix
//! form is `rechte.rs`; the calls of Windows are in `sys/windows.rs`.
//!
//! The folder exists, is a folder and no reparse point, belongs to the own user and has a protected access list that allows only
//! the own user and `SYSTEM` (the counterpart of mode 0700). The socket path must not exist. The entry of the socket gets a
//! protected access list with exactly the own user (the counterpart of mode 0600).

use std::fs;
use std::io::ErrorKind;
use std::path::Path;

use crate::fehler::StartFehler;
use crate::startdatei::Startdatei;
pub(crate) use crate::sys::{ordner_oeffnen, socket_binden};

/// The mode the folder must have on Unix; under Windows its counterpart is the protected list of the own user and `SYSTEM`.
pub const ORDNER_MODUS: u32 = 0o700;
/// The mode of the socket on Unix; under Windows its counterpart is the protected list with the own user only.
pub const SOCKET_MODUS: u32 = 0o600;

/// Checks the folder (exit code 3) and that the socket path does not exist (exit code 3).
///
/// # Errors
/// Exit code 3 with the reason.
pub fn rechte_pruefen(start: &Startdatei) -> Result<(), StartFehler> {
    drop(ordner_oeffnen(
        Path::new(&start.ordner),
        crate::sys::euid(),
    )?);
    socket_pfad_pruefen(Path::new(&start.pfad))
}

/// DI2 point 4: no entry of any kind at the socket path (also a link or a reparse point that points nowhere).
pub(crate) fn socket_pfad_pruefen(pfad: &Path) -> Result<(), StartFehler> {
    match fs::symlink_metadata(pfad) {
        Ok(_) => Err(StartFehler::neu(3, "Socket-Pfad besteht schon")),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(()),
        Err(e) => Err(StartFehler::neu(
            3,
            &format!("Socket-Pfad nicht pruefbar: {}", e.kind()),
        )),
    }
}
