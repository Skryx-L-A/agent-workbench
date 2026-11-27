//! H11 and H12: replace the file atomically relative to the open folder, and the way back (contract 15.12.3).
//!
//! The temporary file is made in the same folder with `O_CREAT | O_EXCL | O_NOFOLLOW` and mode `0600`, written in
//! full, set to `0644` with `fchmod`, synced and renamed onto the name. After the rename the folder is synced. If that
//! fails, the old bytes go back the same way; if the way back fails as well, the state is unclear, but the file holds
//! one whole content, never a part. A temporary file never stays after an error.

use std::os::fd::{AsFd, BorrowedFd};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::code::Code;
use crate::sys::Dateien;

/// Mode of the file after the replacement.
pub const MODUS_DATEI: u32 = 0o644;
/// Attempts to find a free name for the temporary file.
const VERSUCHE: u32 = 8;

/// A name for a temporary file next to `name`: a dot, the name and a number from the clock, the pid and the attempt.
#[must_use]
pub fn temp_name(name: &str, versuch: u32) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.subsec_nanos());
    format!(
        ".{name}.{:08x}{:08x}{versuch:02x}.neu",
        nanos,
        std::process::id()
    )
}

/// Writes `bytes` under `name` in `ordner` through a temporary file and `rename`. Never leaves the temporary file.
///
/// # Errors
/// [`Code::TempFehlgeschlagen`], [`Code::FsyncFehlgeschlagen`] or [`Code::RenameFehlgeschlagen`]; the entry `name`
/// is then unchanged.
pub fn ablegen<D: Dateien>(
    d: &D,
    ordner: BorrowedFd<'_>,
    name: &str,
    bytes: &[u8],
) -> Result<(), Code> {
    let mut angelegt = None;
    for versuch in 0..VERSUCHE {
        let t = temp_name(name, versuch);
        match d.anlegen(ordner, &t) {
            Ok(fd) => {
                angelegt = Some((t, fd));
                break;
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(_) => return Err(Code::TempFehlgeschlagen),
        }
    }
    let Some((temp, fd)) = angelegt else {
        return Err(Code::TempFehlgeschlagen);
    };
    let schritte = d
        .schreiben(fd.as_fd(), bytes)
        .and_then(|()| d.modus_setzen(fd.as_fd(), MODUS_DATEI))
        .map_err(|_| Code::TempFehlgeschlagen)
        .and_then(|()| {
            d.datei_sync(fd.as_fd())
                .map_err(|_| Code::FsyncFehlgeschlagen)
        })
        .and_then(|()| {
            d.umbenennen(ordner, &temp, name)
                .map_err(|_| Code::RenameFehlgeschlagen)
        });
    drop(fd);
    if schritte.is_err() {
        // never the old file: the temporary name only, and only while the rename has not happened
        let _ = d.entfernen(ordner, &temp);
    }
    schritte
}

/// H11 and H12: replaces `name` with `neu`; `alt` are the bytes of the valid file for the way back.
///
/// # Errors
/// The code of the first failed step; [`Code::OrdnerFsyncFehlgeschlagen`] after a successful way back,
/// [`Code::ZustandUnklar`] when the way back failed too.
pub fn ersetzen<D: Dateien>(
    d: &D,
    ordner: BorrowedFd<'_>,
    name: &str,
    alt: &[u8],
    neu: &[u8],
) -> Result<(), Code> {
    ablegen(d, ordner, name, neu)?;
    if d.ordner_sync(ordner).is_ok() {
        return Ok(());
    }
    let zurueck = ablegen(d, ordner, name, alt)
        .and_then(|()| d.ordner_sync(ordner).map_err(|_| Code::ZustandUnklar));
    match zurueck {
        Ok(()) => Err(Code::OrdnerFsyncFehlgeschlagen),
        Err(_) => Err(Code::ZustandUnklar),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[allow(clippy::case_sensitive_file_extension_comparisons)]
    fn temp_name_ohne_weg_und_verschieden() {
        let a = temp_name("firmenvorgabe.json", 0);
        let b = temp_name("firmenvorgabe.json", 1);
        assert!(a.starts_with(".firmenvorgabe.json."), "{a}");
        assert!(a.ends_with(".neu"), "{a}");
        assert!(!a.contains('/'));
        assert_ne!(a, b);
        assert_ne!(a, "firmenvorgabe.json");
        assert_eq!(a.len(), ".firmenvorgabe.json.".len() + 18 + ".neu".len());
    }
}
