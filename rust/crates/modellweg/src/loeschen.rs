//! Creating, renaming and deleting a file relative to an open folder, without following a link (Unix).
//!
//! The folder is opened once (`O_DIRECTORY`, `O_NOFOLLOW`, `O_CLOEXEC`); after that every call is `openat`, `renameat`,
//! `fstatat` (`AT_SYMLINK_NOFOLLOW`) or `unlinkat` on a single name. A name is a single entry: not empty, no `/`, not `.`
//! and not `..`. The code deletes only what it made itself: it remembers the kind, the device and the inode of what it
//! made, and `unlinkat` runs only if the name still shows exactly that. Anything else stays where it is.
//!
//! [`Ordner::ohne_verweis`] is the stronger way in: it opens the folder so that **no** part of the path is followed
//! through a link, not only the last one. There is no fallback that opens.
//!
//! Two windows stay open, and the caller has to carry them:
//! - Between the `fstatat` of [`Ordner::entfernen_wenn`] and its `unlinkat` lies a moment; whoever may write into the
//!   folder can swap the entry in it, and then `unlinkat` hits the other entry. POSIX has no delete through the
//!   descriptor of an entry, so the window cannot be closed here. It stays closed as long as only this user may write
//!   into the folder ([`Ordner::nur_eigen_schreibbar`]).
//! - [`Eigene`] holds no descriptor on its entry. For a socket the listener holds the inode as long as it is bound, so
//!   the inode cannot be used again; for a regular file it could be, after a foreign delete. `merken` therefore takes
//!   the kind it is used for, and today only `FileType::Socket` is used.

use std::ffi::{OsStr, OsString};
use std::fs::File;
use std::io;
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

use rustix::fs::{AtFlags, FileType, Mode, OFlags, Stat};

/// The flags of every folder this module opens: read only, a folder, not a link, closed on `exec`.
const ORDNER_FLAGGEN: OFlags = OFlags::RDONLY
    .union(OFlags::DIRECTORY)
    .union(OFlags::NOFOLLOW)
    .union(OFlags::CLOEXEC);

/// Is `name` a single entry of a folder: not empty, no `/`, not `.` and not `..`? Every name goes through this check
/// before it reaches a call relative to an open folder.
pub(crate) fn einfacher_name(name: &OsStr) -> bool {
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

/// The folder `ordner`, opened with one `openat2` that refuses every link in the path (`RESOLVE_NO_SYMLINKS`). An old
/// kernel without `openat2` ends with `ENOSYS`, and that ends the start; there is no second way that opens.
#[cfg(target_os = "linux")]
fn ordner_ohne_verweis(ordner: &Path) -> io::Result<OwnedFd> {
    Ok(rustix::fs::openat2(
        rustix::fs::CWD,
        ordner,
        ORDNER_FLAGGEN,
        Mode::empty(),
        rustix::fs::ResolveFlags::NO_SYMLINKS,
    )?)
}

/// The folder `ordner`, opened part by part with `O_NOFOLLOW` (every system without `openat2`, among them macOS): a
/// link anywhere in the path ends with `ELOOP`, a part `..` is refused before any call. The first open is `/` for an
/// absolute path and `.` for a relative one.
#[cfg(not(target_os = "linux"))]
fn ordner_ohne_verweis(ordner: &Path) -> io::Result<OwnedFd> {
    use std::path::Component;

    let mut fd = rustix::fs::open(
        if ordner.is_absolute() {
            Path::new("/")
        } else {
            Path::new(".")
        },
        ORDNER_FLAGGEN,
        Mode::empty(),
    )?;
    for teil in ordner.components() {
        match teil {
            Component::RootDir | Component::CurDir => {}
            Component::Normal(n) if einfacher_name(n) => {
                fd = rustix::fs::openat(&fd, n, ORDNER_FLAGGEN, Mode::empty())?;
            }
            Component::Normal(_) | Component::ParentDir | Component::Prefix(_) => {
                return Err(io::ErrorKind::InvalidInput.into());
            }
        }
    }
    Ok(fd)
}

/// Kind, device and inode of an entry: what the code remembers about a file or socket it made.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Kennung {
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
    pub(crate) fn von_datei(datei: &impl AsFd) -> io::Result<Self> {
        Ok(Self::aus(&rustix::fs::fstat(datei)?))
    }

    /// Is the entry of this kind (a file, a socket, …)?
    pub(crate) fn ist(&self, art: FileType) -> bool {
        self.art == art
    }
}

/// An open folder.
pub(crate) struct Ordner {
    fd: OwnedFd,
}

impl Ordner {
    /// Opens the folder of `pfad` and returns it with the last part of `pfad` as the name. Fails if the name is not a
    /// single name or the folder cannot be opened (also if it is a link).
    pub(crate) fn von(pfad: &Path) -> io::Result<(Self, OsString)> {
        let name = pfad.file_name().ok_or(io::ErrorKind::InvalidInput)?;
        if !einfacher_name(name) {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        let fd = rustix::fs::open(eltern(pfad), ORDNER_FLAGGEN, Mode::empty())?;
        Ok((Self { fd }, name.to_os_string()))
    }

    /// Opens the folder of `pfad` so that **no** part of the path is followed through a link, and returns it with the
    /// last part of `pfad` as the name. Fails if any part is a link, a part is `..`, the name is not a single name or
    /// the folder cannot be opened; there is no fallback that opens.
    ///
    /// Under Linux one `openat2` with `RESOLVE_NO_SYMLINKS` does it: the kernel refuses every link in the path,
    /// including a magic link, and an old kernel without `openat2` ends with `ENOSYS` (fail closed). On every other
    /// system each part is opened on its own with `O_NOFOLLOW`, starting at `/` for an absolute path.
    ///
    /// The part `..` is refused here, for every system and before any call. `RESOLVE_NO_SYMLINKS` refuses links, not
    /// `..`: the kernel resolves it as usual (only `RESOLVE_BENEATH` would refuse it), while the walk part by part has
    /// no way to follow it (measured 24.11.2026 on both systems). One rule for both is the one that fails closed.
    pub(crate) fn ohne_verweis(pfad: &Path) -> io::Result<(Self, OsString)> {
        let name = pfad.file_name().ok_or(io::ErrorKind::InvalidInput)?;
        if !einfacher_name(name) {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        if pfad
            .components()
            .any(|teil| teil == std::path::Component::ParentDir)
        {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        let fd = ordner_ohne_verweis(eltern(pfad))?;
        Ok((Self { fd }, name.to_os_string()))
    }

    /// The current working folder as an open folder; it is only needed to go back to it after a `fchdir`.
    #[cfg(not(target_os = "linux"))]
    pub(crate) fn hier() -> io::Result<Self> {
        Ok(Self {
            fd: rustix::fs::open(Path::new("."), ORDNER_FLAGGEN, Mode::empty())?,
        })
    }

    /// The descriptor of this folder, for a call that has no `…at` form (the `bind` of a socket).
    pub(crate) fn fd(&self) -> BorrowedFd<'_> {
        self.fd.as_fd()
    }

    /// Is this a folder of the user `uid` that no other user may write into (no `w` for group or others)? Only then is
    /// the window between making an entry and looking at it closed: nobody else can put something under the name in
    /// between. `root` is not covered by it.
    pub(crate) fn nur_eigen_schreibbar(&self, uid: u32) -> bool {
        let Ok(st) = rustix::fs::fstat(&self.fd) else {
            return false;
        };
        FileType::from_raw_mode(st.st_mode) == FileType::Directory
            && st.st_uid == uid
            && !Mode::from_raw_mode(st.st_mode).intersects(Mode::WGRP.union(Mode::WOTH))
    }

    /// Makes a new file `name` (`O_CREAT`, `O_EXCL`, `O_NOFOLLOW`) for writing with the rights `modus` (before the umask).
    pub(crate) fn neu_anlegen(&self, name: &OsStr, modus: Mode) -> io::Result<File> {
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

    /// Renames `von` to `nach`, both in this folder.
    pub(crate) fn umbenennen(&self, von: &OsStr, nach: &OsStr) -> io::Result<()> {
        if !einfacher_name(von) || !einfacher_name(nach) {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        Ok(rustix::fs::renameat(&self.fd, von, &self.fd, nach)?)
    }

    /// The `fstatat` of the entry `name`, without following a link; `None` if there is none or the name is not a
    /// single name.
    fn stat(&self, name: &OsStr) -> Option<Stat> {
        if !einfacher_name(name) {
            return None;
        }
        rustix::fs::statat(&self.fd, name, AtFlags::SYMLINK_NOFOLLOW).ok()
    }

    /// Kind, device and inode of the entry `name`, without following a link; `None` if there is none or the name is
    /// not a single name.
    pub(crate) fn kennung(&self, name: &OsStr) -> Option<Kennung> {
        self.stat(name).map(|st| Kennung::aus(&st))
    }

    /// Deletes the entry `name` only if it still has the kind, device and inode `soll`. `true` if it was deleted.
    pub(crate) fn entfernen_wenn(&self, name: &OsStr, soll: &Kennung) -> bool {
        if self.kennung(name).as_ref() != Some(soll) {
            return false;
        }
        rustix::fs::unlinkat(&self.fd, name, AtFlags::empty()).is_ok()
    }
}

/// An entry this process made, with the folder it lies in: it can delete exactly that entry and nothing else.
pub(crate) struct Eigene {
    ordner: Ordner,
    name: OsString,
    kennung: Kennung,
    modus: Mode,
}

impl Eigene {
    /// Remembers the entry `name` of `ordner` as made by this process if it is of the kind `art`. Kind, inode and the
    /// rights come from one `fstatat`, so there is no state in which something is made but nothing is remembered.
    pub(crate) fn merken(ordner: Ordner, name: OsString, art: FileType) -> Option<Self> {
        let st = ordner.stat(&name)?;
        let kennung = Kennung::aus(&st);
        kennung.ist(art).then_some(Self {
            ordner,
            name,
            kennung,
            modus: Mode::from_raw_mode(st.st_mode),
        })
    }

    /// The rights (permission bits) the entry had when it was remembered.
    pub(crate) fn modus(&self) -> Mode {
        self.modus
    }

    /// Deletes the entry if the name still shows what was made; `true` if it was deleted. Anything else stays.
    pub(crate) fn entfernen(&self) -> bool {
        self.ordner.entfernen_wenn(&self.name, &self.kennung)
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::{OsStr, OsString};
    use std::fs;
    use std::os::unix::fs::{PermissionsExt, symlink};
    use std::path::{Path, PathBuf};

    use rustix::fs::{FileType, Mode};

    use super::{Eigene, Kennung, Ordner, einfacher_name, eltern};

    fn wegwerf(name: &str) -> PathBuf {
        let pfad = std::env::temp_dir().join(format!("mw-loeschen-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&pfad);
        fs::create_dir_all(&pfad).expect("folder");
        pfad
    }

    #[test]
    fn a_single_name_is_not_empty_has_no_slash_and_is_no_dot() {
        assert!(einfacher_name(OsStr::new("a")));
        assert!(einfacher_name(OsStr::new("a.b")));
        assert!(einfacher_name(OsStr::new("..a")));
        assert!(einfacher_name(OsStr::new("a..")));
        assert!(!einfacher_name(OsStr::new("")));
        assert!(!einfacher_name(OsStr::new("a/b")));
        assert!(!einfacher_name(OsStr::new("/a")));
        assert!(!einfacher_name(OsStr::new("a/")));
        assert!(!einfacher_name(OsStr::new(".")));
        assert!(!einfacher_name(OsStr::new("..")));
    }

    #[test]
    fn an_own_file_is_deleted_by_name_in_the_folder() {
        let ordner = wegwerf("eigen");
        let pfad = ordner.join("f");
        let (o, name) = Ordner::von(&pfad).expect("folder");
        let datei = o
            .neu_anlegen(&name, Mode::from_raw_mode(0o600))
            .expect("make");
        let soll = Kennung::von_datei(&datei).expect("fstat");
        assert!(soll.ist(FileType::RegularFile));
        assert!(pfad.exists());
        assert!(o.entfernen_wenn(&name, &soll));
        assert!(!pfad.exists());
        assert!(!o.entfernen_wenn(&name, &soll), "nothing is there any more");
        let _ = fs::remove_dir_all(&ordner);
    }

    #[test]
    fn another_file_a_link_or_a_folder_under_the_name_stays() {
        let ordner = wegwerf("fremd");
        let aussen = ordner.join("aussen");
        fs::write(&aussen, "x").expect("write");
        let pfad = ordner.join("f");
        let (o, name) = Ordner::von(&pfad).expect("folder");
        let datei = o
            .neu_anlegen(&name, Mode::from_raw_mode(0o600))
            .expect("make");
        let soll = Kennung::von_datei(&datei).expect("fstat");
        // Another file with another inode (the first stays open, so its inode is not used again).
        fs::remove_file(&pfad).expect("remove");
        fs::write(&pfad, "fremd").expect("write");
        assert!(!o.entfernen_wenn(&name, &soll));
        assert_eq!(fs::read_to_string(&pfad).expect("read"), "fremd");
        // A link to a file outside: the link stays and so does its target.
        fs::remove_file(&pfad).expect("remove");
        symlink(&aussen, &pfad).expect("link");
        assert!(!o.entfernen_wenn(&name, &soll));
        assert!(fs::symlink_metadata(&pfad).expect("link").is_symlink());
        assert_eq!(fs::read_to_string(&aussen).expect("read"), "x");
        // A folder.
        fs::remove_file(&pfad).expect("remove");
        fs::create_dir(&pfad).expect("folder");
        assert!(!o.entfernen_wenn(&name, &soll));
        assert!(pfad.is_dir());
        let _ = fs::remove_dir_all(&ordner);
    }

    #[test]
    fn a_name_with_a_slash_a_dot_or_nothing_deletes_nothing() {
        let ordner = wegwerf("name");
        fs::create_dir(ordner.join("unter")).expect("folder");
        let tief = ordner.join("unter").join("t");
        fs::write(&tief, "x").expect("write");
        let (o, _) = Ordner::von(&ordner.join("f")).expect("folder");
        let soll = Kennung::von_datei(&fs::File::open(&tief).expect("open")).expect("fstat");
        for schlecht in ["unter/t", "./unter/t", "", ".", ".."] {
            assert!(
                !o.entfernen_wenn(OsStr::new(schlecht), &soll),
                "{schlecht:?}"
            );
            assert!(o.kennung(OsStr::new(schlecht)).is_none(), "{schlecht:?}");
        }
        assert!(tief.exists());
        assert!(
            o.neu_anlegen(OsStr::new("unter/n"), Mode::from_raw_mode(0o600))
                .is_err(),
            "a name with a slash makes nothing"
        );
        assert!(!ordner.join("unter").join("n").exists());
        assert!(
            o.umbenennen(OsStr::new("unter/t"), OsStr::new("z"))
                .is_err()
        );
        assert!(
            o.umbenennen(OsStr::new("a"), OsStr::new("unter/z"))
                .is_err()
        );
        assert!(o.umbenennen(OsStr::new("a"), OsStr::new("..")).is_err());
        assert!(tief.exists());
        // The path itself must end in a single name.
        for pfad in [ordner.join(".."), PathBuf::from("/"), PathBuf::new()] {
            assert!(Ordner::von(&pfad).is_err(), "{pfad:?}");
        }
        let _ = fs::remove_dir_all(&ordner);
    }

    #[test]
    fn a_folder_that_is_a_link_is_not_opened() {
        let ordner = wegwerf("verweis");
        fs::create_dir(ordner.join("echt")).expect("folder");
        symlink(ordner.join("echt"), ordner.join("lnk")).expect("link");
        assert!(Ordner::von(&ordner.join("lnk").join("f")).is_err());
        assert!(Ordner::von(&ordner.join("echt").join("f")).is_ok());
        let _ = fs::remove_dir_all(&ordner);
    }

    #[test]
    fn a_link_anywhere_in_the_path_is_not_followed() {
        let ordner = wegwerf("ueber");
        let echt = ordner.join("echt");
        let unten = echt.join("unten");
        fs::create_dir_all(&unten).expect("folders");
        symlink(&echt, ordner.join("lnk")).expect("link");
        let ueber_verweis = ordner.join("lnk").join("unten").join("f");
        // The last part of the folder is a real folder, so `von` opens it: its `O_NOFOLLOW` sees only that one part.
        assert!(Ordner::von(&ueber_verweis).is_ok());
        // `ohne_verweis` refuses the link above it.
        assert!(Ordner::ohne_verweis(&ueber_verweis).is_err());
        // The same folder without the link opens, so the test measures the link and nothing else.
        let aufgeloest = fs::canonicalize(&unten).expect("resolve").join("f");
        assert!(Ordner::ohne_verweis(&aufgeloest).is_ok());
        // A part `..` never reaches a call.
        assert!(Ordner::ohne_verweis(&unten.join("..").join("f")).is_err());
        let _ = fs::remove_dir_all(&ordner);
    }

    #[test]
    fn only_a_folder_of_this_user_that_nobody_else_may_write_into_counts() {
        let ordner = fs::canonicalize(wegwerf("rechte")).expect("resolve");
        let (o, _) = Ordner::ohne_verweis(&ordner.join("f")).expect("folder");
        let uid = crate::os::eigene_uid();
        for modus in [0o700, 0o750, 0o755] {
            fs::set_permissions(&ordner, fs::Permissions::from_mode(modus)).expect("chmod");
            assert!(o.nur_eigen_schreibbar(uid), "{modus:o}");
        }
        for modus in [0o770, 0o707, 0o777, 0o722] {
            fs::set_permissions(&ordner, fs::Permissions::from_mode(modus)).expect("chmod");
            assert!(!o.nur_eigen_schreibbar(uid), "{modus:o}");
        }
        fs::set_permissions(&ordner, fs::Permissions::from_mode(0o700)).expect("chmod");
        assert!(
            !o.nur_eigen_schreibbar(uid.wrapping_add(1)),
            "a folder of another user"
        );
        let _ = fs::remove_dir_all(&ordner);
    }

    #[test]
    fn a_name_is_remembered_only_for_the_kind_that_was_made() {
        let ordner = wegwerf("art");
        fs::write(ordner.join("f"), "x").expect("write");
        let (o, name) = Ordner::von(&ordner.join("f")).expect("folder");
        assert!(Eigene::merken(o, name, FileType::Socket).is_none());
        let (o, name) = Ordner::von(&ordner.join("g")).expect("folder");
        assert!(
            Eigene::merken(o, name, FileType::RegularFile).is_none(),
            "nothing is there"
        );
        let (o, name) = Ordner::von(&ordner.join("f")).expect("folder");
        let eigene = Eigene::merken(o, name, FileType::RegularFile).expect("remember");
        assert!(eigene.entfernen());
        assert!(!ordner.join("f").exists());
        assert!(!eigene.entfernen());
        let _ = fs::remove_dir_all(&ordner);
    }

    #[test]
    fn a_rename_goes_relative_to_the_folder() {
        let ordner = wegwerf("umbenennen");
        let (o, _) = Ordner::von(&ordner.join("x")).expect("folder");
        drop(
            o.neu_anlegen(OsStr::new("a"), Mode::from_raw_mode(0o600))
                .expect("make"),
        );
        assert!(
            o.neu_anlegen(OsStr::new("a"), Mode::from_raw_mode(0o600))
                .is_err(),
            "O_EXCL: an existing name is not made again"
        );
        o.umbenennen(OsStr::new("a"), OsStr::new("b"))
            .expect("rename");
        assert!(!ordner.join("a").exists());
        assert!(ordner.join("b").exists());
        let name: OsString = "b".into();
        assert!(o.kennung(&name).is_some());
        let _ = fs::remove_dir_all(&ordner);
    }

    #[test]
    fn the_folder_of_a_bare_name_is_the_current_folder() {
        assert_eq!(eltern(Path::new("s.sock")), Path::new("."));
        assert_eq!(eltern(Path::new("a/s.sock")), Path::new("a"));
        assert_eq!(eltern(Path::new("/s.sock")), Path::new("/"));
    }
}
