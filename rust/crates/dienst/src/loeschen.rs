//! Removing below the socket folder, and only through its descriptor (Auftrag vom 21.11.2026: "Löschen nur über Deskriptoren").
//!
//! SECURITY: a path put together from strings is never the only lock. The service opens the socket folder once after the check of its
//! rights ([`crate::rechte::ordner_oeffnen`]: `O_DIRECTORY | O_NOFOLLOW | O_CLOEXEC`, owner, mode and `fstat` equal to the folder that
//! was checked) and keeps the descriptor to the end. Every removal below it goes through `statat` with `AT_SYMLINK_NOFOLLOW`,
//! `openat` with `O_DIRECTORY | O_NOFOLLOW` (device and inode must equal the `statat` result), `unlinkat` relative to the open folder.
//! A folder is emptied entry by entry and then removed with `AT_REMOVEDIR`; a file or a link is removed itself and never followed.
//! If the socket folder is renamed or replaced by a link after the start, the descriptor still names the original folder, so nothing
//! outside it is touched. A name with a slash, an empty name, `.` and `..` are refused before any call.
//! The depth is bounded by [`TIEFE_MAX`]: below it the service stops and leaves the rest, it reports nothing to the page.

use std::ffi::CString;
use std::os::fd::{AsFd, BorrowedFd};

use rustix::fs::{AtFlags, Dir, FileType, Mode, OFlags, fstat, openat, statat, unlinkat};

/// The deepest level of folders below the named entry that is entered (the named folder itself is level 0). Deeper folders stay.
pub(crate) const TIEFE_MAX: usize = 64;

/// The flags to open a folder without following a link. A constant: the combination is made once, here.
pub(crate) const ORDNER_OFFEN: OFlags = OFlags::RDONLY
    .union(OFlags::DIRECTORY)
    .union(OFlags::NOFOLLOW)
    .union(OFlags::CLOEXEC);

/// A name the removal accepts: not empty, no slash, no NUL, not `.` and not `..`.
pub(crate) fn name_gut(name: &str) -> bool {
    !name.is_empty() && !name.contains(['/', '\0']) && name != "." && name != ".."
}

/// The form of the folder the service makes for a session: `s-` and digits.
pub(crate) fn sitzungsname_gut(name: &str) -> bool {
    name_gut(name)
        && name
            .strip_prefix("s-")
            .is_some_and(|z| !z.is_empty() && z.bytes().all(|b| b.is_ascii_digit()))
}

/// Removes the session folder `name` (form `s-<digits>`) below `wurzel` with everything in it. A link or file at that place is removed itself.
/// Returns `false` when the name is refused (nothing was done).
pub(crate) fn sitzung_entfernen(wurzel: BorrowedFd<'_>, name: &str) -> bool {
    if !sitzungsname_gut(name) {
        return false;
    }
    let Ok(c) = CString::new(name) else {
        return false;
    };
    rekursiv(wurzel, &c, 0);
    true
}

/// Removes the entry `name` below `wurzel` that is no folder (the socket file). A folder is not removed. `false`: the name is refused.
pub(crate) fn datei_entfernen(wurzel: BorrowedFd<'_>, name: &str) -> bool {
    if !name_gut(name) {
        return false;
    }
    let Ok(c) = CString::new(name) else {
        return false;
    };
    let _ = unlinkat(wurzel, &c, AtFlags::empty());
    true
}

/// Removes `name` below `dir`: no link is followed, a folder is entered only when `openat` finds the folder that `statat` saw.
fn rekursiv(dir: BorrowedFd<'_>, name: &std::ffi::CStr, tiefe: usize) {
    // the listing of a folder holds `.` and `..`: they are never entered or removed
    if matches!(name.to_bytes(), b"." | b"..") {
        return;
    }
    let Ok(st) = statat(dir, name, AtFlags::SYMLINK_NOFOLLOW) else {
        return;
    };
    if FileType::from_raw_mode(st.st_mode) != FileType::Directory {
        let _ = unlinkat(dir, name, AtFlags::empty());
        return;
    }
    if tiefe > TIEFE_MAX {
        return;
    }
    let Ok(sub) = openat(dir, name, ORDNER_OFFEN, Mode::empty()) else {
        return;
    };
    let Ok(st2) = fstat(&sub) else {
        return;
    };
    if (st2.st_dev, st2.st_ino) != (st.st_dev, st.st_ino) {
        return;
    }
    let Ok(eintraege) = Dir::read_from(&sub) else {
        return;
    };
    let namen: Vec<CString> = eintraege
        .filter_map(Result::ok)
        .map(|e| e.file_name().to_owned())
        .collect();
    for n in &namen {
        rekursiv(sub.as_fd(), n, tiefe + 1);
    }
    let _ = unlinkat(dir, name, AtFlags::REMOVEDIR);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::symlink;
    use std::path::{Path, PathBuf};

    /// A folder of the test below `TMPDIR`, removed at the end by the test itself with `std` (test code, not the product).
    struct Probe(PathBuf);

    impl Probe {
        fn neu(name: &str) -> Self {
            let p = std::env::temp_dir().join(format!("dl-{}-{name}", std::process::id()));
            let _ = fs::remove_dir_all(&p);
            fs::create_dir_all(&p).expect("dir");
            Self(p)
        }
    }

    impl Drop for Probe {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn fd(p: &Path) -> rustix::fd::OwnedFd {
        openat(rustix::fs::CWD, p, ORDNER_OFFEN, Mode::empty()).expect("open")
    }

    #[test]
    fn eine_verknuepfung_an_der_stelle_des_ordners_verschwindet_das_ziel_bleibt() {
        let p = Probe::neu("link-stelle");
        let wurzel = p.0.join("w");
        let opfer = p.0.join("opfer");
        fs::create_dir_all(&wurzel).expect("w");
        fs::create_dir_all(&opfer).expect("opfer");
        fs::write(opfer.join("datei"), "x").expect("f");
        symlink(&opfer, wurzel.join("s-1")).expect("link");
        assert!(sitzung_entfernen(fd(&wurzel).as_fd(), "s-1"));
        assert!(fs::symlink_metadata(wurzel.join("s-1")).is_err());
        assert!(opfer.join("datei").is_file());
    }

    #[test]
    fn eine_verknuepfung_im_ordner_verschwindet_das_ziel_bleibt() {
        let p = Probe::neu("link-drin");
        let wurzel = p.0.join("w");
        let opfer = p.0.join("opfer");
        fs::create_dir_all(wurzel.join("s-2/unter")).expect("w");
        fs::create_dir_all(&opfer).expect("opfer");
        fs::write(opfer.join("datei"), "x").expect("f");
        fs::write(wurzel.join("s-2/start.json"), "{}").expect("f");
        symlink(&opfer, wurzel.join("s-2/unter/ziel")).expect("link");
        symlink(opfer.join("datei"), wurzel.join("s-2/dlink")).expect("link");
        assert!(sitzung_entfernen(fd(&wurzel).as_fd(), "s-2"));
        assert!(fs::symlink_metadata(wurzel.join("s-2")).is_err());
        assert!(opfer.join("datei").is_file());
    }

    #[test]
    fn wird_der_wurzelordner_umbenannt_bleibt_ein_opferordner_an_seiner_stelle_unberuehrt() {
        let p = Probe::neu("umbenannt");
        let wurzel = p.0.join("w");
        let opfer = p.0.join("opfer");
        fs::create_dir_all(wurzel.join("s-3")).expect("w");
        fs::create_dir_all(opfer.join("s-3")).expect("opfer");
        fs::write(opfer.join("s-3/datei"), "x").expect("f");
        let offen = fd(&wurzel);
        fs::rename(&wurzel, p.0.join("woanders")).expect("rename");
        symlink(&opfer, &wurzel).expect("link an der alten Stelle");
        assert!(sitzung_entfernen(offen.as_fd(), "s-3"));
        assert!(opfer.join("s-3/datei").is_file(), "the victim is untouched");
        assert!(
            !p.0.join("woanders/s-3").exists(),
            "the original folder was cleaned"
        );
        fs::write(p.0.join("woanders/sock"), "").expect("sock");
        assert!(datei_entfernen(offen.as_fd(), "sock"));
        assert!(!p.0.join("woanders/sock").exists());
        fs::write(opfer.join("sock"), "").expect("sock");
        assert!(datei_entfernen(offen.as_fd(), "sock"));
        assert!(
            opfer.join("sock").exists(),
            "the victim keeps its file of the same name"
        );
    }

    #[test]
    fn die_namen_punkt_punktpunkt_schraegstrich_und_leer_werden_abgewiesen() {
        let p = Probe::neu("namen");
        let wurzel = p.0.join("w");
        fs::create_dir_all(wurzel.join("a")).expect("w");
        fs::write(p.0.join("aussen"), "x").expect("f");
        fs::write(wurzel.join("a/b"), "x").expect("f");
        let offen = fd(&wurzel);
        for n in ["..", ".", "a/b", "", "../aussen", "s-1/x", "s-", "s-x"] {
            assert!(!sitzung_entfernen(offen.as_fd(), n), "{n:?}");
        }
        for n in ["..", ".", "a/b", "", "../aussen", "a\0b"] {
            assert!(!datei_entfernen(offen.as_fd(), n), "{n:?}");
        }
        assert!(p.0.join("aussen").is_file() && wurzel.join("a/b").is_file());
    }

    #[test]
    fn tiefer_als_die_grenze_hoert_der_dienst_auf_und_ausserhalb_aendert_sich_nichts() {
        let p = Probe::neu("tiefe");
        let wurzel = p.0.join("w");
        fs::create_dir_all(&wurzel).expect("w");
        fs::write(p.0.join("aussen"), "x").expect("f");
        // s-4/d/d/... with 70 levels
        let mut pfad = wurzel.join("s-4");
        for _ in 0..70 {
            pfad.push("d");
        }
        fs::create_dir_all(&pfad).expect("deep");
        fs::write(pfad.join("tief"), "x").expect("f");
        fs::write(wurzel.join("s-4/oben"), "x").expect("f");
        assert!(sitzung_entfernen(fd(&wurzel).as_fd(), "s-4"));
        assert!(
            pfad.join("tief").is_file(),
            "below the bound the rest stays"
        );
        assert!(
            !wurzel.join("s-4/oben").exists(),
            "above the bound everything is removed"
        );
        assert!(p.0.join("aussen").is_file());
        // a tree exactly at the bound goes completely
        let mut flach = wurzel.join("s-5");
        for _ in 0..TIEFE_MAX {
            flach.push("d");
        }
        fs::create_dir_all(&flach).expect("flat");
        assert!(sitzung_entfernen(fd(&wurzel).as_fd(), "s-5"));
        assert!(!wurzel.join("s-5").exists());
    }

    #[test]
    fn eine_datei_oder_ein_leerer_ordner_an_der_stelle_geht() {
        let p = Probe::neu("arten");
        let wurzel = p.0.join("w");
        fs::create_dir_all(wurzel.join("s-7")).expect("w");
        fs::write(wurzel.join("s-8"), "x").expect("f");
        let offen = fd(&wurzel);
        assert!(sitzung_entfernen(offen.as_fd(), "s-7"));
        assert!(sitzung_entfernen(offen.as_fd(), "s-8"));
        assert!(
            sitzung_entfernen(offen.as_fd(), "s-9"),
            "a missing entry is no error"
        );
        assert!(fs::read_dir(&wurzel).expect("rd").next().is_none());
    }
}
