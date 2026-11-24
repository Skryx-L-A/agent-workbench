//! The files of "Lauf starten": the start file of step 2, the counting folder of step 5a and the session files and
//! folders it writes (mode 0600 and 0700, never through a link, never over an existing file).

use std::fs::{DirBuilder, File, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::Path;

use rustix::fs::{FileType, Mode, OFlags};

use crate::fehler::Fehler;

/// The largest start file that is read; a longer one is cut there and stage 1 of contract 1 refuses it.
pub const START_LESEN_MAX: u64 = 4097;

/// The mode bits of `st` as `u32` (`u16` on macOS, `u32` on Linux).
#[allow(clippy::useless_conversion)]
pub(crate) fn modus_von(st: &rustix::fs::Stat) -> u32 {
    u32::from(st.st_mode)
}

fn eigen() -> u32 {
    rustix::process::geteuid().as_raw()
}

/// The rule of step 2 on the result of `fstat`: a regular file of the own user with mode exactly 0600.
#[must_use]
pub fn start_modus_gut(art: FileType, eigner: u32, modus: u32, uid: u32) -> bool {
    art == FileType::RegularFile && eigner == uid && modus & 0o7777 == 0o600
}

/// Step 2: opens the start file with `O_NOFOLLOW` and reads at most [`START_LESEN_MAX`] bytes.
///
/// # Errors
/// `lauf.datei_modus` for a link, another kind of file, another owner or another mode; `lauf.datei_unlesbar` when it
/// cannot be opened or read.
pub fn startdatei_lesen(pfad: &Path) -> Result<Vec<u8>, Fehler> {
    let fd = rustix::fs::open(
        pfad,
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC | OFlags::NONBLOCK,
        Mode::empty(),
    )
    .map_err(|e| match e {
        rustix::io::Errno::LOOP => Fehler::DateiModus,
        _ => Fehler::DateiUnlesbar,
    })?;
    let st = rustix::fs::fstat(&fd).map_err(|_| Fehler::DateiUnlesbar)?;
    if !start_modus_gut(
        FileType::from_raw_mode(st.st_mode),
        st.st_uid,
        modus_von(&st),
        eigen(),
    ) {
        return Err(Fehler::DateiModus);
    }
    let mut text = Vec::new();
    File::from(fd)
        .take(START_LESEN_MAX)
        .read_to_end(&mut text)
        .map_err(|_| Fehler::DateiUnlesbar)?;
    Ok(text)
}

/// The rule of step 5a on the result of `fstat`: a folder of the own user with mode exactly 0700.
#[must_use]
pub fn zaehlordner_gut(art: FileType, eigner: u32, modus: u32, uid: u32) -> bool {
    art == FileType::Directory && eigner == uid && modus & 0o7777 == 0o700
}

/// Step 5a: the counting folder opened with `O_NOFOLLOW`: exists, is a folder, own, mode exactly 0700.
///
/// # Errors
/// `lauf.marker_ordner`.
pub fn zaehlordner_pruefen(pfad: &Path) -> Result<(), Fehler> {
    if eigener_ordner_0700(pfad) {
        Ok(())
    } else {
        Err(Fehler::MarkerOrdner)
    }
}

/// A real folder (no link) of the own user with mode exactly 0700, opened with `O_NOFOLLOW`: `false` when it is
/// missing, a link, foreign or has another mode.
#[must_use]
pub fn eigener_ordner_0700(pfad: &Path) -> bool {
    let Ok(fd) = rustix::fs::open(
        pfad,
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC | OFlags::DIRECTORY,
        Mode::empty(),
    ) else {
        return false;
    };
    rustix::fs::fstat(&fd).is_ok_and(|st| {
        zaehlordner_gut(
            FileType::from_raw_mode(st.st_mode),
            st.st_uid,
            modus_von(&st),
            eigen(),
        )
    })
}

/// Makes a new folder with mode 0700; an existing path is a fault.
///
/// # Errors
/// The folder exists or cannot be made.
pub fn ordner_neu(pfad: &Path) -> std::io::Result<()> {
    DirBuilder::new().mode(0o700).create(pfad)?;
    // the umask may have taken bits; the mode must be exactly 0700
    std::fs::set_permissions(pfad, std::os::unix::fs::PermissionsExt::from_mode(0o700))
}

/// Creates a new file with mode 0600: never through a link and never over an existing file (`O_CREAT | O_EXCL`
/// refuses every existing name, a link too). An error means this call made no file.
///
/// # Errors
/// The file exists or cannot be made.
pub fn datei_anlegen(pfad: &Path) -> std::io::Result<File> {
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(pfad)
}

/// Sets mode 0600 through the descriptor (the name may have been swapped since), writes `inhalt` and `fsync`s.
///
/// # Errors
/// The file cannot be written.
pub fn datei_fuellen(datei: &mut File, inhalt: &[u8]) -> std::io::Result<()> {
    datei.set_permissions(std::os::unix::fs::PermissionsExt::from_mode(0o600))?;
    datei.write_all(inhalt)?;
    datei.sync_all()
}

/// Writes a new file with mode 0600 ([`datei_anlegen`], [`datei_fuellen`]).
///
/// # Errors
/// The file exists or cannot be written.
pub fn datei_neu(pfad: &Path, inhalt: &[u8]) -> std::io::Result<()> {
    datei_fuellen(&mut datei_anlegen(pfad)?, inhalt)
}

/// The deepest folder tree [`ordner_entfernen`] descends into (the session folder has three levels at most).
const ENTFERNEN_TIEFE: usize = 32;

/// Removes the folder `pfad` with everything in it, by name and through descriptors: the parent is opened once, the
/// folder is looked at with `AT_SYMLINK_NOFOLLOW` and opened with `O_NOFOLLOW | O_DIRECTORY`, and every entry below is
/// treated the same way. A link in the place of the folder is left alone, a link inside it is removed as a link; none is
/// ever followed, so nothing outside the folder is touched even if a name is swapped for a link while this runs.
/// [`ENTFERNEN_TIEFE`] limits the depth.
///
/// # Errors
/// The folder is missing, is not a real folder (a link or a file stays untouched), or something in it could not be
/// removed (the rest is removed all the same).
pub fn ordner_entfernen(pfad: &Path) -> std::io::Result<()> {
    use std::os::unix::ffi::OsStrExt;
    let (Some(eltern), Some(name)) = (pfad.parent(), pfad.file_name()) else {
        return Err(std::io::ErrorKind::InvalidInput.into());
    };
    let eltern = if eltern.as_os_str().is_empty() {
        Path::new(".")
    } else {
        eltern
    };
    let name = std::ffi::CString::new(name.as_bytes())
        .map_err(|_| std::io::Error::from(std::io::ErrorKind::InvalidInput))?;
    let fd = rustix::fs::open(
        eltern,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
        Mode::empty(),
    )?;
    entfernen_in(&fd, &name, 0)
}

/// Opens the folder `name` below `eltern` and nothing else: never through a link (`O_NOFOLLOW`), only a folder
/// (`O_DIRECTORY`), by name relative to the descriptor.
fn unterordner_oeffnen(
    eltern: &rustix::fd::OwnedFd,
    name: &std::ffi::CStr,
) -> std::io::Result<rustix::fd::OwnedFd> {
    rustix::fs::openat(
        eltern,
        name,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(Into::into)
}

fn entfernen_in(
    eltern: &rustix::fd::OwnedFd,
    name: &std::ffi::CStr,
    tiefe: usize,
) -> std::io::Result<()> {
    use rustix::fs::{AtFlags, statat, unlinkat};
    // second guard (the listing below drops both names as well): `.` and `..` must never be entered, or the removal
    // would climb out of the folder; a name with a slash is no name of an entry either
    if matches!(name.to_bytes(), b"." | b".." | b"") || name.to_bytes().contains(&b'/') {
        return Err(std::io::ErrorKind::InvalidInput.into());
    }
    let st = statat(eltern, name, AtFlags::SYMLINK_NOFOLLOW)?;
    if FileType::from_raw_mode(st.st_mode) != FileType::Directory {
        return Err(std::io::ErrorKind::NotADirectory.into());
    }
    if tiefe >= ENTFERNEN_TIEFE {
        return Err(std::io::ErrorKind::InvalidInput.into());
    }
    let fd = unterordner_oeffnen(eltern, name)?;
    let namen: Vec<std::ffi::CString> = rustix::fs::Dir::read_from(&fd)?
        .filter_map(Result::ok)
        .map(|e| e.file_name().to_owned())
        .filter(|n| n.as_bytes() != b"." && n.as_bytes() != b"..")
        .collect();
    let mut rest = Ok(());
    for n in &namen {
        let ergebnis = match statat(&fd, n.as_c_str(), AtFlags::SYMLINK_NOFOLLOW) {
            Ok(s) if FileType::from_raw_mode(s.st_mode) == FileType::Directory => {
                entfernen_in(&fd, n.as_c_str(), tiefe + 1)
            }
            Ok(_) => unlinkat(&fd, n.as_c_str(), AtFlags::empty()).map_err(Into::into),
            Err(e) => Err(e.into()),
        };
        if ergebnis.is_err() {
            rest = ergebnis;
        }
    }
    drop(fd);
    rest?;
    unlinkat(eltern, name, AtFlags::REMOVEDIR).map_err(Into::into)
}

#[cfg(test)]
mod tests;
