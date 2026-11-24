//! Deleting a file this call made, relative to an open folder, without following a link (Unix).
//!
//! The folder of the path is opened once (`O_DIRECTORY`, `O_NOFOLLOW`, `O_CLOEXEC`); the look is `fstatat`
//! (`AT_SYMLINK_NOFOLLOW`) and the delete is `unlinkat`, both on a single name. The file is deleted only if the name
//! still shows the regular file that the caller holds open (same device and inode). Anything else stays where it is.

use std::ffi::OsStr;
use std::fs::File;
use std::os::fd::{AsFd, OwnedFd};
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

use rustix::fs::{AtFlags, FileType, Mode, OFlags, Stat};

/// Is `name` a single entry of a folder: not empty, no `/`, not `.` and not `..`?
fn einfacher_name(name: &OsStr) -> bool {
    let bytes = name.as_bytes();
    !bytes.is_empty() && !bytes.contains(&b'/') && bytes != b"." && bytes != b".."
}

/// Kind, device and inode of an entry; `None` if it is not a regular file.
fn datei_kennung(st: &Stat) -> Option<(i128, i128)> {
    (FileType::from_raw_mode(st.st_mode) == FileType::RegularFile)
        .then(|| (i128::from(st.st_dev), i128::from(st.st_ino)))
}

/// Deletes `name` in `ordner` if it is the regular file `datei` (same device and inode), not a link, not another file.
/// `true` if it was deleted.
fn entfernen_in(ordner: &impl AsFd, name: &OsStr, datei: &impl AsFd) -> bool {
    if !einfacher_name(name) {
        return false;
    }
    let Ok(eigen) = rustix::fs::fstat(datei) else {
        return false;
    };
    let Ok(da) = rustix::fs::statat(ordner, name, AtFlags::SYMLINK_NOFOLLOW) else {
        return false;
    };
    let Some(soll) = datei_kennung(&eigen) else {
        return false;
    };
    if datei_kennung(&da) != Some(soll) {
        return false;
    }
    rustix::fs::unlinkat(ordner, name, AtFlags::empty()).is_ok()
}

/// The folder a path lies in; for a bare name (`m.jsonl`) the current folder.
fn eltern(pfad: &Path) -> &Path {
    match pfad.parent() {
        Some(p) if !p.as_os_str().is_empty() => p,
        _ => Path::new("."),
    }
}

/// Deletes the file at `pfad` if it is still the regular file that `datei` holds open (the one this call made).
/// The folder of `pfad` is opened without following a link at its last part. If the folder cannot be opened, the
/// name is not a single name or the name shows something else, nothing is deleted. `true` if it was deleted.
pub(super) fn eigene_datei_entfernen(datei: &File, pfad: &Path) -> bool {
    let Some(name) = pfad.file_name() else {
        return false;
    };
    if !einfacher_name(name) {
        return false;
    }
    let ordner: Result<OwnedFd, _> = rustix::fs::open(
        eltern(pfad),
        OFlags::RDONLY
            .union(OFlags::DIRECTORY)
            .union(OFlags::NOFOLLOW)
            .union(OFlags::CLOEXEC),
        Mode::empty(),
    );
    let Ok(ordner) = ordner else {
        return false;
    };
    entfernen_in(&ordner, name, datei)
}

#[cfg(test)]
mod tests {
    use std::ffi::OsStr;
    use std::fs::{self, File, OpenOptions};
    use std::os::unix::fs::symlink;
    use std::path::{Path, PathBuf};

    use rustix::fs::{Mode, OFlags};

    use super::{eigene_datei_entfernen, einfacher_name, eltern, entfernen_in};

    fn wegwerf(name: &str) -> PathBuf {
        let pfad =
            std::env::temp_dir().join(format!("formate-loeschen-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&pfad);
        fs::create_dir_all(&pfad).expect("folder");
        pfad
    }

    fn neu(pfad: &PathBuf) -> File {
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(pfad)
            .expect("make")
    }

    #[test]
    fn a_single_name_is_not_empty_has_no_slash_and_is_no_dot() {
        assert!(einfacher_name(OsStr::new("a")));
        assert!(einfacher_name(OsStr::new("a.jsonl")));
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
    fn the_own_file_is_deleted() {
        let ordner = wegwerf("eigen");
        let pfad = ordner.join("m.jsonl");
        let datei = neu(&pfad);
        assert!(eigene_datei_entfernen(&datei, &pfad));
        assert!(!pfad.exists());
        assert!(
            !eigene_datei_entfernen(&datei, &pfad),
            "nothing is there any more"
        );
        let _ = fs::remove_dir_all(&ordner);
    }

    #[test]
    fn another_file_a_link_or_a_folder_under_the_name_stays() {
        let ordner = wegwerf("fremd");
        let aussen = ordner.join("aussen");
        fs::write(&aussen, "x").expect("write");
        let pfad = ordner.join("m.jsonl");
        let datei = neu(&pfad);
        // Another file with another inode (the first stays open, so its inode is not used again).
        fs::remove_file(&pfad).expect("remove");
        fs::write(&pfad, "fremd").expect("write");
        assert!(!eigene_datei_entfernen(&datei, &pfad));
        assert_eq!(fs::read_to_string(&pfad).expect("read"), "fremd");
        // A link to a file outside: the link stays and so does its target.
        fs::remove_file(&pfad).expect("remove");
        symlink(&aussen, &pfad).expect("link");
        assert!(!eigene_datei_entfernen(&datei, &pfad));
        assert!(fs::symlink_metadata(&pfad).expect("link").is_symlink());
        assert_eq!(fs::read_to_string(&aussen).expect("read"), "x");
        // A link to the very file the caller holds: still a link, so it stays.
        fs::remove_file(&pfad).expect("remove");
        let echt = ordner.join("echt");
        let gehalten = neu(&echt);
        symlink(&echt, &pfad).expect("link");
        assert!(!eigene_datei_entfernen(&gehalten, &pfad));
        assert!(fs::symlink_metadata(&pfad).expect("link").is_symlink());
        assert!(echt.exists());
        // A folder.
        fs::remove_file(&pfad).expect("remove");
        fs::create_dir(&pfad).expect("folder");
        assert!(!eigene_datei_entfernen(&datei, &pfad));
        assert!(pfad.is_dir());
        let _ = fs::remove_dir_all(&ordner);
    }

    #[test]
    fn a_name_with_a_slash_a_dot_or_nothing_deletes_nothing() {
        let ordner = wegwerf("name");
        fs::create_dir(ordner.join("unter")).expect("folder");
        let tief = ordner.join("unter").join("t");
        let datei = neu(&tief);
        let fd = rustix::fs::open(&ordner, OFlags::RDONLY | OFlags::DIRECTORY, Mode::empty())
            .expect("open folder");
        for schlecht in ["unter/t", "./unter/t", "", ".", ".."] {
            assert!(
                !entfernen_in(&fd, OsStr::new(schlecht), &datei),
                "{schlecht:?}"
            );
        }
        assert!(tief.exists());
        // The same file by its single name in its own folder is deleted.
        let fd2 = rustix::fs::open(
            ordner.join("unter"),
            OFlags::RDONLY | OFlags::DIRECTORY,
            Mode::empty(),
        )
        .expect("open folder");
        assert!(entfernen_in(&fd2, OsStr::new("t"), &datei));
        assert!(!tief.exists());
        // The path itself must end in a single name.
        for pfad in [ordner.join(".."), PathBuf::from("/"), PathBuf::new()] {
            assert!(!eigene_datei_entfernen(&datei, &pfad), "{pfad:?}");
        }
        let _ = fs::remove_dir_all(&ordner);
    }

    #[test]
    fn a_folder_that_is_a_link_is_not_opened() {
        let ordner = wegwerf("verweis");
        let echt = ordner.join("echt");
        fs::create_dir(&echt).expect("folder");
        symlink(&echt, ordner.join("lnk")).expect("link");
        let pfad = echt.join("f");
        let datei = neu(&pfad);
        assert!(!eigene_datei_entfernen(
            &datei,
            &ordner.join("lnk").join("f")
        ));
        assert!(pfad.exists());
        assert!(eigene_datei_entfernen(&datei, &pfad));
        let _ = fs::remove_dir_all(&ordner);
    }

    #[test]
    fn the_folder_of_a_bare_name_is_the_current_folder() {
        assert_eq!(eltern(Path::new("m.jsonl")), Path::new("."));
        assert_eq!(eltern(Path::new("a/m.jsonl")), Path::new("a"));
        assert_eq!(eltern(Path::new("/m.jsonl")), Path::new("/"));
    }
}
