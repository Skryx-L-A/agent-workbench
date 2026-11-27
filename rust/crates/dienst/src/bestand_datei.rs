//! The file of the Bestand (contract, DI-L5 points 3 and 4): the content and the checks of `bestand.rs`, the calls of the file system
//! here. Under Windows the Bestand is not built yet: a start file that names one ends the start with exit code 2 (fail closed), and
//! nothing is written.
//!
//! SECURITY: reading opens without following a link and without blocking on a FIFO, then checks the opened file itself (regular,
//! owned by the effective user, mode 0600). Writing makes `<bestand>.neu.<pid>` exclusive with mode 0600 without following a link,
//! `fsync`, renames it onto the Bestand and calls `fsync` on the folder.

use crate::fehler::StartFehler;

/// The content of the Bestand at `pfad`, at most one byte more than [`crate::bestand::BESTAND_MAX`] (so that a larger file is seen);
/// `None` when the file is missing.
///
/// # Errors
/// Exit code 2: a link, no regular file, owned by another user, not mode 0600, or not readable; under Windows always.
pub(crate) fn lesen(pfad: &str) -> Result<Option<Vec<u8>>, StartFehler> {
    #[cfg(unix)]
    {
        unix::lesen(pfad)
    }
    #[cfg(not(unix))]
    {
        let _ = pfad;
        Err(StartFehler::neu(2, "lauf.bestand nur unter Unix"))
    }
}

/// Writes `roh` to `pfad` as DI-L5 point 3 says. `Err` is the code for the line on stderr; under Windows always.
pub(crate) fn schreiben(pfad: &str, roh: &[u8]) -> Result<(), String> {
    #[cfg(unix)]
    {
        unix::schreiben(pfad, roh)
    }
    #[cfg(not(unix))]
    {
        let _ = (pfad, roh);
        Err("plattform".to_owned())
    }
}

#[cfg(unix)]
mod unix {
    use std::io::{ErrorKind, Read, Write};
    use std::os::unix::fs::{MetadataExt, PermissionsExt};

    use rustix::fs::{Mode, OFlags};

    use crate::bestand::{BESTAND_MAX, ungueltig};
    use crate::fehler::StartFehler;

    /// See [`super::lesen`].
    pub(super) fn lesen(pfad: &str) -> Result<Option<Vec<u8>>, StartFehler> {
        // no link is followed and a FIFO does not block; the checks below are made on the opened file itself
        let flaggen = OFlags::from_iter([
            OFlags::RDONLY,
            OFlags::NOFOLLOW,
            OFlags::NONBLOCK,
            OFlags::CLOEXEC,
        ]);
        let fd = match rustix::fs::open(pfad, flaggen, Mode::empty()) {
            Ok(fd) => fd,
            Err(e) if std::io::Error::from(e).kind() == ErrorKind::NotFound => return Ok(None),
            Err(_) => return Err(ungueltig()),
        };
        let datei = std::fs::File::from(fd);
        let meta = datei.metadata().map_err(|_| ungueltig())?;
        if !meta.is_file()
            || meta.uid() != crate::sys::euid()
            || meta.permissions().mode() & 0o7777 != 0o600
        {
            return Err(ungueltig());
        }
        let mut roh = Vec::new();
        datei
            .take(BESTAND_MAX + 1)
            .read_to_end(&mut roh)
            .map_err(|_| ungueltig())?;
        Ok(Some(roh))
    }

    /// See [`super::schreiben`].
    pub(super) fn schreiben(pfad: &str, roh: &[u8]) -> Result<(), String> {
        let code = |e: std::io::Error| {
            e.raw_os_error()
                .map_or_else(|| "io".to_owned(), |n| n.to_string())
        };
        let neu = format!("{pfad}.neu.{}", std::process::id());
        let flaggen = OFlags::from_iter([
            OFlags::WRONLY,
            OFlags::CREATE,
            OFlags::EXCL,
            OFlags::NOFOLLOW,
            OFlags::CLOEXEC,
        ]);
        let fd = rustix::fs::open(neu.as_str(), flaggen, Mode::from_raw_mode(0o600))
            .map_err(|e| code(e.into()))?;
        let mut datei = std::fs::File::from(fd);
        datei
            .set_permissions(std::fs::Permissions::from_mode(0o600))
            .map_err(code)?;
        datei.write_all(roh).map_err(code)?;
        datei.sync_all().map_err(code)?;
        drop(datei);
        std::fs::rename(&neu, pfad).map_err(code)?;
        let ordner = std::path::Path::new(pfad)
            .parent()
            .unwrap_or_else(|| std::path::Path::new("/"));
        std::fs::File::open(ordner)
            .and_then(|o| o.sync_all())
            .map_err(code)
    }
}

#[cfg(all(test, unix))]
mod tests {
    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    /// Only a regular file is a Bestand: a FIFO of the own user with mode 0600 opens without blocking and reads as empty, and it is
    /// refused all the same; a regular file with the same owner and mode is read.
    #[test]
    fn nur_eine_regulaere_datei_ist_ein_bestand() {
        let o = std::env::temp_dir().join(format!("bd-{}", std::process::id()));
        let _ = fs::remove_dir_all(&o);
        fs::create_dir_all(&o).expect("dir");
        let fifo = o.join("fifo");
        let ok = std::process::Command::new("mkfifo")
            .arg(&fifo)
            .status()
            .expect("mkfifo");
        assert!(ok.success());
        fs::set_permissions(&fifo, fs::Permissions::from_mode(0o600)).expect("mode");
        assert!(super::lesen(fifo.to_str().expect("utf8")).is_err());
        let datei = o.join("datei");
        fs::write(&datei, b"{}").expect("file");
        fs::set_permissions(&datei, fs::Permissions::from_mode(0o600)).expect("mode");
        assert_eq!(
            super::lesen(datei.to_str().expect("utf8")).ok().flatten(),
            Some(b"{}".to_vec())
        );
        let _ = fs::remove_dir_all(&o);
    }
}
