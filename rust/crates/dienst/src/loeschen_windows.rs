//! Removing below the socket folder under Windows (Nachtrag 25.11.2026, contract `dienst`, section 21). The Unix form is `loeschen.rs`.
//!
//! The service holds the socket folder open without `FILE_SHARE_DELETE` from the check to the end (`sys/windows.rs`, `Ordner`):
//! nobody can rename, move or delete the folder while it runs, so its path keeps naming the checked folder. A name with a slash,
//! a backslash, a colon (no alternate data stream), an empty name, `.` and `..` are refused before any call. A file or a link
//! (also a junction or an `AF_UNIX` entry) is removed itself and never followed (`DeleteFileW` removes the reparse point, not its
//! target). A folder is removed with `std::fs::remove_dir_all`, which does not follow links or junctions below it.
//!
//! Deviation from Unix (section 21): there is no `openat`; the removal goes by the path below the held folder. A folder below it
//! that a foreign process swaps for a junction between the check and the call is not followed either (`remove_dir_all` checks
//! every entry for a reparse point before it descends).

use std::path::Path;

use crate::sys::BorrowedFd;

/// Whether `name` is a single plain name (no separator of either kind, no stream, not `.` or `..`, no NUL).
pub(crate) fn name_gut(name: &str) -> bool {
    !name.is_empty() && !name.contains(['/', '\\', ':', '\0']) && name != "." && name != ".."
}

/// Whether `name` is the folder of a session (`s-<digits>`).
pub(crate) fn sitzungsname_gut(name: &str) -> bool {
    name_gut(name)
        && name
            .strip_prefix("s-")
            .is_some_and(|z| !z.is_empty() && z.bytes().all(|b| b.is_ascii_digit()))
}

/// Removes the folder of a session below the socket folder; a file or link of that name is removed itself.
pub(crate) fn sitzung_entfernen(wurzel: BorrowedFd<'_>, name: &str) -> bool {
    if !sitzungsname_gut(name) {
        return false;
    }
    entfernen(&wurzel.pfad().join(name));
    true
}

/// Removes one entry below the socket folder that is not a folder (a folder of that name stays).
pub(crate) fn datei_entfernen(wurzel: BorrowedFd<'_>, name: &str) -> bool {
    if !name_gut(name) {
        return false;
    }
    let pfad = wurzel.pfad().join(name);
    if let Ok(m) = std::fs::symlink_metadata(&pfad)
        && (!m.is_dir() || m.file_type().is_symlink())
    {
        let _ = std::fs::remove_file(&pfad);
    }
    true
}

fn entfernen(pfad: &Path) {
    let Ok(m) = std::fs::symlink_metadata(pfad) else {
        return;
    };
    if m.is_dir() && !m.file_type().is_symlink() {
        let _ = std::fs::remove_dir_all(pfad);
    } else if m.file_type().is_symlink() && m.is_dir() {
        // a link to a folder or a junction: the link itself goes
        let _ = std::fs::remove_dir(pfad);
    } else {
        let _ = std::fs::remove_file(pfad);
    }
}
