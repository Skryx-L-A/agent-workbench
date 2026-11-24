//! Creating, renaming, changing the rights of and deleting a file or socket relative to an open folder, without following a
//! link (Unix). The pattern is that of `modellweg/src/loeschen.rs`; a shared crate is
//! not part of this step.
//!
//! The folder is opened once (`O_DIRECTORY`, `O_NOFOLLOW`, `O_CLOEXEC`); after that every call is `openat`, `renameat`,
//! `fstatat` (`AT_SYMLINK_NOFOLLOW`) or `unlinkat` on a single name. A name is a single entry: not empty, no `/`, not `.`
//! and not `..`. The code deletes only what it made itself: it remembers the kind, the device and the inode of what it
//! made, and `unlinkat` runs only if the name still shows exactly that. Anything else stays where it is. No composed path is
//! the only guard of a deletion, a rename or a change of rights.
//!
//! Rights come in as `rustix::fs::Mode`, never as a number: `mode_t` is `u32` under Linux and `u16` under macOS. A caller
//! with a fixed value writes `Mode::from_raw_mode(0o600)`; the literal takes the type of the system. A value from outside is
//! turned into a `Mode` by the caller and refused if it does not fit (same rule as in `modellweg`, decision of 21.11.2026).

use std::ffi::{OsStr, OsString};
use std::fs::File;
use std::io;
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

use rustix::fs::{AtFlags, FileType, Mode, OFlags, Stat};

/// Is `name` a single entry of a folder: not empty, no `/`, not `.` and not `..`? Every name goes through this check
/// before it reaches a call relative to an open folder.
#[must_use]
pub fn einfacher_name(name: &OsStr) -> bool {
    let bytes = name.as_bytes();
    !bytes.is_empty() && !bytes.contains(&b'/') && bytes != b"." && bytes != b".."
}

/// The folder a path lies in; for a bare name (`s.sock`) the current folder.
fn eltern(pfad: &Path) -> &Path {
    match pfad.parent() {
        Some(p) if !p.as_os_str().is_empty() => p,
        _ => Path::new("."),
    }
}

/// Kind, device and inode of an entry: what the code remembers about a file or socket it made.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Kennung {
    art: FileType,
    geraet: i128,
    inode: i128,
}

impl Kennung {
    fn aus(st: &Stat) -> Self {
        Self {
            art: FileType::from_raw_mode(st.st_mode),
            geraet: i128::from(st.st_dev),
            inode: i128::from(st.st_ino),
        }
    }

    /// The kind, device and inode of an open file (`fstat`).
    ///
    /// # Errors
    ///
    /// The error of `fstat`.
    pub fn von_datei(datei: &impl AsFd) -> io::Result<Self> {
        Ok(Self::aus(&rustix::fs::fstat(datei)?))
    }

    /// Is the entry of this kind (a file, a socket, …)?
    #[must_use]
    pub fn ist(&self, art: FileType) -> bool {
        self.art == art
    }
}

/// An open folder.
#[derive(Debug)]
pub struct Ordner {
    fd: OwnedFd,
}

/// The descriptor of the folder, for a check by `fstat` or `faccessat` on the folder that is held (review B6, 23.11.2026):
/// read only, nothing in this file deletes through it.
impl AsFd for Ordner {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.fd.as_fd()
    }
}

impl Ordner {
    /// Opens the folder of `pfad` and returns it with the last part of `pfad` as the name.
    ///
    /// # Errors
    ///
    /// The name is not a single name, or the folder cannot be opened (also if it is a link).
    pub fn von(pfad: &Path) -> io::Result<(Self, OsString)> {
        let name = pfad.file_name().ok_or(io::ErrorKind::InvalidInput)?;
        if !einfacher_name(name) {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        let fd = rustix::fs::open(
            eltern(pfad),
            OFlags::RDONLY
                .union(OFlags::DIRECTORY)
                .union(OFlags::NOFOLLOW)
                .union(OFlags::CLOEXEC),
            Mode::empty(),
        )?;
        Ok((Self { fd }, name.to_os_string()))
    }

    /// Makes a new file `name` (`O_CREAT`, `O_EXCL`, `O_NOFOLLOW`) for writing with the rights `modus` (before the umask).
    ///
    /// # Errors
    ///
    /// The name is not a single name, or the file exists or cannot be made.
    pub fn neu_anlegen(&self, name: &OsStr, modus: Mode) -> io::Result<File> {
        if !einfacher_name(name) {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        let fd = rustix::fs::openat(
            &self.fd,
            name,
            OFlags::WRONLY
                .union(OFlags::CREATE)
                .union(OFlags::EXCL)
                .union(OFlags::NOFOLLOW)
                .union(OFlags::CLOEXEC),
            modus,
        )?;
        Ok(File::from(fd))
    }

    /// Renames `von` to `nach`, both in this folder, if `von` still shows `soll`.
    ///
    /// # Errors
    ///
    /// A name is not a single name, `von` is not what `soll` says, or the rename fails.
    pub fn umbenennen_wenn(&self, von: &OsStr, nach: &OsStr, soll: &Kennung) -> io::Result<()> {
        if !einfacher_name(von) || !einfacher_name(nach) {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        if self.kennung(von).as_ref() != Some(soll) {
            return Err(io::Error::other("the name does not show the own entry"));
        }
        Ok(rustix::fs::renameat(&self.fd, von, &self.fd, nach)?)
    }

    /// Kind, device and inode of the entry `name`, without following a link; `None` if there is none or the name is
    /// not a single name.
    #[must_use]
    pub fn kennung(&self, name: &OsStr) -> Option<Kennung> {
        if !einfacher_name(name) {
            return None;
        }
        let st = rustix::fs::statat(&self.fd, name, AtFlags::SYMLINK_NOFOLLOW).ok()?;
        Some(Kennung::aus(&st))
    }

    /// Deletes the entry `name` only if it still has the kind, device and inode `soll`. `true` if it was deleted.
    #[must_use]
    pub fn entfernen_wenn(&self, name: &OsStr, soll: &Kennung) -> bool {
        if self.kennung(name).as_ref() != Some(soll) {
            return false;
        }
        rustix::fs::unlinkat(&self.fd, name, AtFlags::empty()).is_ok()
    }

    /// Sets the rights of the entry `name` only if it still has the kind, device and inode `soll`.
    ///
    /// Under Linux the entry is opened with `O_PATH | O_NOFOLLOW`, compared by `fstat` on that descriptor and changed through
    /// `/proc/self/fd`, so that no swap of the name can come between the comparison and the change. Elsewhere
    /// `fchmodat(AT_SYMLINK_NOFOLLOW)` follows the comparison; the folder of the socket is a private folder there
    /// (addendum to contract 4, section 3).
    ///
    /// # Errors
    ///
    /// The name is not a single name, it does not show `soll`, or the change fails.
    #[cfg(target_os = "linux")]
    pub fn rechte_setzen_wenn(&self, name: &OsStr, soll: &Kennung, modus: Mode) -> io::Result<()> {
        use std::os::fd::AsRawFd;
        if !einfacher_name(name) {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        let fd = rustix::fs::openat(
            &self.fd,
            name,
            OFlags::PATH.union(OFlags::NOFOLLOW).union(OFlags::CLOEXEC),
            Mode::empty(),
        )?;
        if Kennung::von_datei(&fd)? != *soll {
            return Err(io::Error::other("the name does not show the own entry"));
        }
        Ok(rustix::fs::chmod(
            format!("/proc/self/fd/{}", fd.as_raw_fd()),
            modus,
        )?)
    }

    /// See the Linux variant.
    ///
    /// # Errors
    ///
    /// The name is not a single name, it does not show `soll`, or the change fails.
    #[cfg(not(target_os = "linux"))]
    pub fn rechte_setzen_wenn(&self, name: &OsStr, soll: &Kennung, modus: Mode) -> io::Result<()> {
        if !einfacher_name(name) {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        if self.kennung(name).as_ref() != Some(soll) {
            return Err(io::Error::other("the name does not show the own entry"));
        }
        Ok(rustix::fs::chmodat(
            &self.fd,
            name,
            modus,
            AtFlags::SYMLINK_NOFOLLOW,
        )?)
    }
}

/// An entry this process made, with the folder it lies in: it can change the rights of and delete exactly that entry and
/// nothing else.
#[derive(Debug)]
pub struct Eigene {
    ordner: Ordner,
    name: OsString,
    kennung: Kennung,
}

impl Eigene {
    /// Remembers the entry `name` of `ordner` as made by this process if it is of the kind `art`.
    #[must_use]
    pub fn merken(ordner: Ordner, name: OsString, art: FileType) -> Option<Self> {
        let kennung = ordner.kennung(&name)?;
        kennung.ist(art).then_some(Self {
            ordner,
            name,
            kennung,
        })
    }

    /// Sets the rights of the entry if the name still shows what was made.
    ///
    /// # Errors
    ///
    /// The name shows something else, or the change fails.
    pub fn rechte_setzen(&self, modus: Mode) -> io::Result<()> {
        self.ordner
            .rechte_setzen_wenn(&self.name, &self.kennung, modus)
    }

    /// Deletes the entry if the name still shows what was made; `true` if it was deleted. Anything else stays.
    #[must_use]
    pub fn entfernen(&self) -> bool {
        self.ordner.entfernen_wenn(&self.name, &self.kennung)
    }
}
