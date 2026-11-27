//! The narrow layer of the file calls (contract 15.12.3, H3, H11 and H12). Every call that touches the folder of the
//! helper goes through [`Dateien`]; [`Echt`] makes the calls of the system. A test wraps [`Echt`] and lets one call fail
//! or reports another owner, so that each injected error of the cases (`temp`, `fsync`, `rename`, `ordner_fsync`,
//! `rueckweg`) and each owner can be checked without root.
//!
//! Everything below the folder runs relative to its open descriptor (`openat`, `renameat`, `unlinkat`) and never follows
//! a link (`O_NOFOLLOW`). A name never holds `/`.

use std::io;
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
use std::path::Path;

use rustix::fs::{AtFlags, CWD, FileType, Mode, OFlags};

/// The kind of a directory entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Art {
    /// A regular file.
    Datei,
    /// A folder.
    Ordner,
    /// A symbolic link.
    Verweis,
    /// Anything else (socket, FIFO, device).
    Andere,
}

/// What the helper reads of an entry: kind, owner, mode and inode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Meta {
    /// The kind.
    pub art: Art,
    /// The uid of the owner.
    pub uid: u32,
    /// The permission bits (`& 0o7777`).
    pub modus: u32,
    /// The inode number (a test maps an owner by it).
    pub ino: u64,
}

impl Meta {
    // `mode_t` is `u16` under macOS and `u32` under Linux
    #[allow(clippy::useless_conversion)]
    fn aus(st: &rustix::fs::Stat) -> Meta {
        let roh = u32::from(st.st_mode);
        let art = match FileType::from_raw_mode(st.st_mode) {
            FileType::RegularFile => Art::Datei,
            FileType::Directory => Art::Ordner,
            FileType::Symlink => Art::Verweis,
            _ => Art::Andere,
        };
        Meta {
            art,
            uid: st.st_uid,
            modus: roh & 0o7777,
            ino: st.st_ino,
        }
    }
}

/// The file calls of the helper.
pub trait Dateien {
    /// `lstat` of a path (the parent folders).
    ///
    /// # Errors
    /// The error of the system.
    fn lstat(&self, pfad: &Path) -> io::Result<Meta>;
    /// Opens a folder with `O_DIRECTORY | O_NOFOLLOW`.
    ///
    /// # Errors
    /// The error of the system; a link as the last part fails.
    fn ordner_oeffnen(&self, pfad: &Path) -> io::Result<OwnedFd>;
    /// `fstat` of an open descriptor.
    ///
    /// # Errors
    /// The error of the system.
    fn fstat(&self, fd: BorrowedFd<'_>) -> io::Result<Meta>;
    /// Opens `name` in `ordner` for reading, with `O_NOFOLLOW | O_NONBLOCK`.
    ///
    /// # Errors
    /// The error of the system; a link fails.
    fn oeffnen(&self, ordner: BorrowedFd<'_>, name: &str) -> io::Result<OwnedFd>;
    /// Reads at most `max` bytes from the start of the open file.
    ///
    /// # Errors
    /// The error of the system.
    fn lesen(&self, fd: BorrowedFd<'_>, max: usize) -> io::Result<Vec<u8>>;
    /// Makes `name` in `ordner` with `O_CREAT | O_EXCL | O_NOFOLLOW | O_WRONLY` and mode `0600`.
    ///
    /// # Errors
    /// The error of the system; an existing entry fails.
    fn anlegen(&self, ordner: BorrowedFd<'_>, name: &str) -> io::Result<OwnedFd>;
    /// Writes all `bytes`.
    ///
    /// # Errors
    /// The error of the system.
    fn schreiben(&self, fd: BorrowedFd<'_>, bytes: &[u8]) -> io::Result<()>;
    /// `fchmod` of the open file.
    ///
    /// # Errors
    /// The error of the system.
    fn modus_setzen(&self, fd: BorrowedFd<'_>, modus: u32) -> io::Result<()>;
    /// `fsync` of the open temporary file.
    ///
    /// # Errors
    /// The error of the system.
    fn datei_sync(&self, fd: BorrowedFd<'_>) -> io::Result<()>;
    /// `renameat` of `von` to `nach`, both in `ordner`.
    ///
    /// # Errors
    /// The error of the system.
    fn umbenennen(&self, ordner: BorrowedFd<'_>, von: &str, nach: &str) -> io::Result<()>;
    /// `fsync` of the open folder.
    ///
    /// # Errors
    /// The error of the system.
    fn ordner_sync(&self, ordner: BorrowedFd<'_>) -> io::Result<()>;
    /// `unlinkat` of `name` in `ordner` (a file, never a folder).
    ///
    /// # Errors
    /// The error of the system.
    fn entfernen(&self, ordner: BorrowedFd<'_>, name: &str) -> io::Result<()>;
}

/// Flags of `ordner_oeffnen`: a folder, never through a link at the last part.
const ORDNER_OEFFNEN: OFlags = OFlags::RDONLY
    .union(OFlags::DIRECTORY)
    .union(OFlags::NOFOLLOW)
    .union(OFlags::CLOEXEC);
/// Flags of `oeffnen`: read, never through a link, never blocking on a FIFO.
const DATEI_OEFFNEN: OFlags = OFlags::RDONLY
    .union(OFlags::NOFOLLOW)
    .union(OFlags::NONBLOCK)
    .union(OFlags::CLOEXEC);
/// Flags of `anlegen`: a new file only, never through a link.
const ANLEGEN: OFlags = OFlags::WRONLY
    .union(OFlags::CREATE)
    .union(OFlags::EXCL)
    .union(OFlags::NOFOLLOW)
    .union(OFlags::CLOEXEC);

/// The calls of the system.
#[derive(Debug, Clone, Copy, Default)]
pub struct Echt;

fn name_gut(name: &str) -> io::Result<()> {
    if name.is_empty() || name == "." || name == ".." || name.contains('/') || name.contains('\0') {
        return Err(io::Error::from(io::ErrorKind::InvalidInput));
    }
    Ok(())
}

impl Dateien for Echt {
    fn lstat(&self, pfad: &Path) -> io::Result<Meta> {
        Ok(Meta::aus(&rustix::fs::lstat(pfad)?))
    }

    fn ordner_oeffnen(&self, pfad: &Path) -> io::Result<OwnedFd> {
        Ok(rustix::fs::openat(
            CWD,
            pfad,
            ORDNER_OEFFNEN,
            Mode::empty(),
        )?)
    }

    fn fstat(&self, fd: BorrowedFd<'_>) -> io::Result<Meta> {
        Ok(Meta::aus(&rustix::fs::fstat(fd)?))
    }

    fn oeffnen(&self, ordner: BorrowedFd<'_>, name: &str) -> io::Result<OwnedFd> {
        name_gut(name)?;
        Ok(rustix::fs::openat(
            ordner,
            name,
            DATEI_OEFFNEN,
            Mode::empty(),
        )?)
    }

    fn lesen(&self, fd: BorrowedFd<'_>, max: usize) -> io::Result<Vec<u8>> {
        let mut aus = Vec::new();
        let mut puffer = [0u8; 8192];
        loop {
            let rest = (max - aus.len()).min(puffer.len());
            if rest == 0 {
                break;
            }
            let n = rustix::io::read(fd, &mut puffer[..rest])?;
            if n == 0 {
                break;
            }
            aus.extend_from_slice(&puffer[..n]);
        }
        Ok(aus)
    }

    fn anlegen(&self, ordner: BorrowedFd<'_>, name: &str) -> io::Result<OwnedFd> {
        name_gut(name)?;
        Ok(rustix::fs::openat(
            ordner,
            name,
            ANLEGEN,
            Mode::from_raw_mode(0o600),
        )?)
    }

    fn schreiben(&self, fd: BorrowedFd<'_>, bytes: &[u8]) -> io::Result<()> {
        let mut rest = bytes;
        while !rest.is_empty() {
            let n = rustix::io::write(fd, rest)?;
            if n == 0 {
                return Err(io::Error::from(io::ErrorKind::WriteZero));
            }
            rest = &rest[n..];
        }
        Ok(())
    }

    // a mode is at most 0o7777 and fits into the 16 bits of `mode_t` under macOS
    #[allow(clippy::cast_possible_truncation)]
    fn modus_setzen(&self, fd: BorrowedFd<'_>, modus: u32) -> io::Result<()> {
        Ok(rustix::fs::fchmod(
            fd,
            Mode::from_raw_mode((modus & 0o7777) as _),
        )?)
    }

    fn datei_sync(&self, fd: BorrowedFd<'_>) -> io::Result<()> {
        Ok(rustix::fs::fsync(fd)?)
    }

    fn umbenennen(&self, ordner: BorrowedFd<'_>, von: &str, nach: &str) -> io::Result<()> {
        name_gut(von)?;
        name_gut(nach)?;
        Ok(rustix::fs::renameat(ordner, von, ordner, nach)?)
    }

    fn ordner_sync(&self, ordner: BorrowedFd<'_>) -> io::Result<()> {
        Ok(rustix::fs::fsync(ordner)?)
    }

    fn entfernen(&self, ordner: BorrowedFd<'_>, name: &str) -> io::Result<()> {
        name_gut(name)?;
        Ok(rustix::fs::unlinkat(ordner, name, AtFlags::empty())?)
    }
}

/// Borrows an open descriptor (a short name for the calls above).
#[must_use]
pub fn griff(fd: &OwnedFd) -> BorrowedFd<'_> {
    fd.as_fd()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ort() -> std::path::PathBuf {
        let p = std::env::temp_dir().join(format!("vh-sys-{}-{}", std::process::id(), line!()));
        let _ = std::fs::create_dir(&p);
        p
    }

    #[test]
    fn namen_ohne_weg() {
        for n in ["", ".", "..", "a/b", "/a", "a\0"] {
            assert!(name_gut(n).is_err(), "{n:?}");
        }
        assert!(name_gut("firmenvorgabe.json").is_ok());
        assert!(name_gut("...").is_ok());
    }

    #[test]
    #[allow(clippy::many_single_char_names)]
    fn echt_legt_an_schreibt_und_benennt_um() {
        let p = ort();
        let e = Echt;
        let o = e.ordner_oeffnen(&p).expect("folder");
        assert_eq!(e.fstat(griff(&o)).expect("fstat").art, Art::Ordner);
        let t = e.anlegen(griff(&o), "t").expect("make");
        assert!(e.anlegen(griff(&o), "t").is_err(), "O_EXCL");
        e.schreiben(griff(&t), b"hallo").expect("write");
        let m = e.fstat(griff(&t)).expect("fstat");
        assert_eq!((m.art, m.modus), (Art::Datei, 0o600));
        assert_eq!(m.uid, rustix::process::getuid().as_raw());
        e.modus_setzen(griff(&t), 0o644).expect("chmod");
        assert_eq!(e.fstat(griff(&t)).expect("fstat").modus, 0o644);
        e.datei_sync(griff(&t)).expect("fsync");
        e.umbenennen(griff(&o), "t", "z").expect("rename");
        e.ordner_sync(griff(&o)).expect("fsync folder");
        let z = e.oeffnen(griff(&o), "z").expect("open");
        assert_eq!(e.lesen(griff(&z), 3).expect("read"), b"hal");
        let z = e.oeffnen(griff(&o), "z").expect("open");
        assert_eq!(e.lesen(griff(&z), 100).expect("read"), b"hallo");
        std::os::unix::fs::symlink("z", p.join("v")).expect("link");
        assert!(e.oeffnen(griff(&o), "v").is_err(), "O_NOFOLLOW");
        assert_eq!(e.lstat(&p.join("v")).expect("lstat").art, Art::Verweis);
        assert!(e.ordner_oeffnen(&p.join("v")).is_err());
        std::os::unix::fs::symlink(&p, p.join("w")).expect("link");
        assert!(e.ordner_oeffnen(&p.join("w")).is_err(), "folder link");
        e.entfernen(griff(&o), "v").expect("unlink");
        e.entfernen(griff(&o), "w").expect("unlink");
        e.entfernen(griff(&o), "z").expect("unlink");
        assert!(e.entfernen(griff(&o), "z").is_err());
        let grosse = vec![7u8; 20000];
        let t = e.anlegen(griff(&o), "g").expect("make");
        e.schreiben(griff(&t), &grosse).expect("write");
        let g = e.oeffnen(griff(&o), "g").expect("open");
        assert_eq!(e.lesen(griff(&g), 30000).expect("read"), grosse);
        let g = e.oeffnen(griff(&o), "g").expect("open");
        assert_eq!(
            e.lesen(griff(&g), 10000).expect("read").len(),
            10000,
            "more than one buffer, less than the file"
        );
        let g = e.oeffnen(griff(&o), "g").expect("open");
        assert_eq!(e.lesen(griff(&g), 0).expect("read").len(), 0);
        // fsync of a socket fails: the call reaches the system and its error comes back
        let (a, _b) = std::os::unix::net::UnixStream::pair().expect("pair");
        assert!(e.datei_sync(a.as_fd()).is_err());
        assert!(e.ordner_sync(a.as_fd()).is_err());
        // a FIFO in place of the file opens without waiting for a writer
        let fifo = p.join("fifo");
        assert!(
            std::process::Command::new("mkfifo")
                .arg(&fifo)
                .status()
                .expect("mkfifo")
                .success()
        );
        let f = e.oeffnen(griff(&o), "fifo").expect("open without blocking");
        assert_eq!(e.fstat(griff(&f)).expect("fstat").art, Art::Andere);
        e.entfernen(griff(&o), "fifo").expect("unlink");
        // the descriptors do not leak into a program started later
        for fd in [&o, &f] {
            let flags = rustix::io::fcntl_getfd(fd).expect("getfd");
            assert!(flags.contains(rustix::io::FdFlags::CLOEXEC));
        }
        let t = e.anlegen(griff(&o), "c").expect("make");
        assert!(
            rustix::io::fcntl_getfd(&t)
                .expect("getfd")
                .contains(rustix::io::FdFlags::CLOEXEC)
        );
        e.entfernen(griff(&o), "c").expect("unlink");
        // a file for writing only, never readable through the descriptor of anlegen
        assert!(rustix::io::read(&t, &mut [0u8; 1]).is_err());
        e.entfernen(griff(&o), "g").expect("unlink");
        drop(o);
        std::fs::remove_dir(&p).expect("empty folder");
    }
}
