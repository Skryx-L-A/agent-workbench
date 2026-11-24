//! The rights of the token file and the port file (BR1, a security point).
//!
//! The folder must be a real folder (no link, checked with `lstat`), owned by the effective user, with exactly the mode
//! 0700. The file must not exist (file, link, dangling link, folder). The bridge never creates the folder and never
//! replaces or deletes anything that is already there. A file is created exclusively with the mode 0600.

use std::ffi::CString;
use std::fs::{self, File};
use std::io::{self, Write};
use std::os::fd::OwnedFd;
use std::os::unix::fs::MetadataExt;
use std::path::Path;

use rustix::fs::{AtFlags, CWD, Mode, OFlags, fchmod, fstat, openat, statat, unlinkat};

use crate::fehler::StartFehler;

fn rechte_fehler(text: &'static str) -> StartFehler {
    StartFehler { code: 3, text }
}

/// Checks the folder of `pfad` and that `pfad` does not exist. Exit code 3 otherwise.
pub(crate) fn pruefen(pfad: &str) -> Result<(), StartFehler> {
    pruefen_fuer(pfad, rustix::process::geteuid().as_raw())
}

/// [`pruefen`] with the user id to compare the owner of the folder with.
pub(crate) fn pruefen_fuer(pfad: &str, euid: u32) -> Result<(), StartFehler> {
    let ordner = Path::new(pfad)
        .parent()
        .ok_or_else(|| rechte_fehler("Ordner der Datei fehlt"))?;
    let st = fs::symlink_metadata(ordner).map_err(|_| rechte_fehler("Ordner der Datei fehlt"))?;
    if st.file_type().is_symlink() {
        return Err(rechte_fehler("Ordner der Datei ist ein symbolischer Link"));
    }
    if !st.is_dir() {
        return Err(rechte_fehler("Ordner der Datei ist kein Ordner"));
    }
    if st.uid() != euid {
        return Err(rechte_fehler(
            "Ordner der Datei gehoert einem anderen Benutzer",
        ));
    }
    if st.mode() & 0o7777 != 0o700 {
        return Err(rechte_fehler("Ordner der Datei hat nicht den Modus 0700"));
    }
    match fs::symlink_metadata(pfad) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        _ => Err(rechte_fehler("Datei besteht schon oder ist nicht pruefbar")),
    }
}

/// A file this bridge made: the open folder it lies in, its name there, and the device and inode it was made with.
/// SECURITY: the removal at the end goes only through the folder descriptor (`fstatat` without following a link, `unlinkat`) and only
/// when device and inode are still the ones of the file that was made; a file that somebody put in its place stays.
/// The file stays open until then: an inode that is still open is not freed, so no other file can get its number. Without it
/// a file system such as ext4 gives a file made right after the removal the same inode, and the comparison would take it for ours.
#[derive(Debug)]
pub(crate) struct Angelegt {
    eltern: OwnedFd,
    name: CString,
    dev: u64,
    ino: u64,
    _offen: File,
}

impl Angelegt {
    /// Removes the file if it is still the one that was made. Returns whether it was removed.
    pub(crate) fn entfernen(&self) -> bool {
        let Ok(st) = statat(&self.eltern, &self.name, AtFlags::SYMLINK_NOFOLLOW) else {
            return false;
        };
        if kennung(&st) != (self.dev, self.ino) {
            return false;
        }
        unlinkat(&self.eltern, &self.name, AtFlags::empty()).is_ok()
    }
}

/// Device and inode of a `Stat` as two `u64`: the types of the fields differ between the systems (`i32` and `u32` under macOS, `u64` under Linux).
#[allow(clippy::unnecessary_cast, clippy::cast_sign_loss)]
fn kennung(st: &rustix::fs::Stat) -> (u64, u64) {
    (st.st_dev as u64, st.st_ino as u64)
}

/// The flags to open the folder of a file without following a link, and to make the file: constants, so the combination is made once.
const ORDNER_OFFEN: OFlags = OFlags::RDONLY
    .union(OFlags::DIRECTORY)
    .union(OFlags::NOFOLLOW)
    .union(OFlags::CLOEXEC);
const DATEI_NEU: OFlags = OFlags::WRONLY
    .union(OFlags::CREATE)
    .union(OFlags::EXCL)
    .union(OFlags::NOFOLLOW)
    .union(OFlags::CLOEXEC);

/// A file name the bridge accepts below an open folder: not empty, no slash, no NUL, not `.` and not `..`.
fn name_gut(name: &str) -> bool {
    !name.is_empty() && !name.contains(['/', '\0']) && name != "." && name != ".."
}

/// Creates `pfad` exclusively with the mode 0600 in its folder, opened without following a link, and writes `inhalt`. The creation mode is
/// cut by the umask, never widened, so the file is never more open than 0600 even for an instant; `fchmod` then makes it exactly 0600.
pub(crate) fn anlegen(pfad: &str, inhalt: &[u8]) -> Result<Angelegt, StartFehler> {
    let fehler = || rechte_fehler("Datei laesst sich nicht anlegen");
    let p = Path::new(pfad);
    let name = p
        .file_name()
        .and_then(|n| n.to_str())
        .filter(|n| name_gut(n))
        .ok_or_else(fehler)?;
    let eltern = openat(
        CWD,
        p.parent().ok_or_else(fehler)?,
        ORDNER_OFFEN,
        Mode::empty(),
    )
    .map_err(|_| fehler())?;
    let cname = CString::new(name).map_err(|_| fehler())?;
    let fd =
        openat(&eltern, &cname, DATEI_NEU, Mode::from_raw_mode(0o600)).map_err(|_| fehler())?;
    fchmod(&fd, Mode::from_raw_mode(0o600)).map_err(|_| fehler())?;
    let st = fstat(&fd).map_err(|_| fehler())?;
    let mut f = File::from(fd);
    f.write_all(inhalt).map_err(|_| fehler())?;
    Ok(Angelegt {
        eltern,
        name: cname,
        dev: kennung(&st).0,
        ino: kennung(&st).1,
        _offen: f,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pruefstand::Ordner;
    use std::fs::Permissions;
    use std::os::unix::fs::PermissionsExt;
    use std::os::unix::fs::symlink;

    fn euid() -> u32 {
        rustix::process::geteuid().as_raw()
    }

    #[test]
    fn a_good_folder_and_a_missing_file_pass() {
        let o = Ordner::neu();
        let p = o.pfad("t");
        assert_eq!(pruefen(p.to_str().expect("utf8")), Ok(()));
    }

    #[test]
    fn only_the_mode_0700_passes() {
        let o = Ordner::neu();
        let p = o.pfad("t");
        let pfad = p.to_str().expect("utf8");
        for modus in [0o755, 0o770, 0o750, 0o701, 0o600, 0o500, 0o777, 0o1700] {
            fs::set_permissions(o.wurzel(), Permissions::from_mode(modus)).expect("chmod");
            assert_eq!(
                pruefen(pfad).expect_err(&format!("{modus:o}")).code,
                3,
                "{modus:o}"
            );
        }
        fs::set_permissions(o.wurzel(), Permissions::from_mode(0o700)).expect("chmod");
        assert_eq!(pruefen(pfad), Ok(()));
    }

    #[test]
    fn a_missing_folder_is_refused_and_not_created() {
        let o = Ordner::neu();
        let p = o.pfad("fehlt/t");
        assert_eq!(
            pruefen(p.to_str().expect("utf8"))
                .expect_err("missing")
                .code,
            3
        );
        assert!(!o.pfad("fehlt").exists());
    }

    #[test]
    fn a_link_as_folder_is_refused_even_to_a_good_folder() {
        let o = Ordner::neu();
        let echt = o.pfad("echt");
        fs::create_dir(&echt).expect("mkdir");
        fs::set_permissions(&echt, Permissions::from_mode(0o700)).expect("chmod");
        symlink(&echt, o.pfad("link")).expect("symlink");
        let p = o.pfad("link/t");
        assert_eq!(
            pruefen(p.to_str().expect("utf8")).expect_err("link").code,
            3
        );
        assert_eq!(pruefen(echt.join("t").to_str().expect("utf8")), Ok(()));
    }

    #[test]
    fn a_file_as_folder_is_refused() {
        let o = Ordner::neu();
        fs::write(o.pfad("datei"), b"x").expect("write");
        let p = o.pfad("datei/t");
        assert_eq!(
            pruefen(p.to_str().expect("utf8")).expect_err("file").code,
            3
        );
    }

    #[test]
    fn an_existing_entry_is_refused_and_stays() {
        let o = Ordner::neu();
        let p = o.pfad("t");
        let pfad = p.to_str().expect("utf8");
        fs::write(&p, b"alt").expect("write");
        assert_eq!(pruefen(pfad).expect_err("file").code, 3);
        assert_eq!(fs::read(&p).expect("read"), b"alt");
        fs::remove_file(&p).expect("rm");
        symlink(o.pfad("nirgends"), &p).expect("symlink");
        assert_eq!(pruefen(pfad).expect_err("dangling link").code, 3);
        assert!(fs::symlink_metadata(&p).is_ok());
        fs::remove_file(&p).expect("rm");
        fs::create_dir(&p).expect("mkdir");
        assert_eq!(pruefen(pfad).expect_err("folder").code, 3);
    }

    #[test]
    fn a_folder_of_another_user_is_refused() {
        let o = Ordner::neu();
        let p = o.pfad("t");
        let pfad = p.to_str().expect("utf8");
        assert_eq!(pruefen_fuer(pfad, euid()), Ok(()));
        assert_eq!(
            pruefen_fuer(pfad, euid() + 1).expect_err("other user").code,
            3
        );
    }

    #[test]
    fn an_entry_that_cannot_be_looked_at_is_refused_too() {
        // a name that is too long: `lstat` fails with another error than "not found"
        let o = Ordner::neu();
        let p = o.pfad(&"a".repeat(300));
        assert_eq!(
            pruefen(p.to_str().expect("utf8"))
                .expect_err("cannot be looked at")
                .code,
            3
        );
    }

    #[test]
    fn a_path_without_a_folder_is_refused() {
        assert_eq!(pruefen("/").expect_err("no parent").code, 3);
    }

    #[test]
    fn a_file_is_created_with_exactly_0600_and_never_replaced() {
        let o = Ordner::neu();
        let p = o.pfad("t");
        let pfad = p.to_str().expect("utf8");
        let angelegt = anlegen(pfad, b"abc\n").expect("made");
        assert_eq!(fs::read(&p).expect("read"), b"abc\n");
        assert_eq!(fs::metadata(&p).expect("stat").mode() & 0o7777, 0o600);
        assert_eq!(anlegen(pfad, b"neu").expect_err("exists").code, 3);
        assert_eq!(fs::read(&p).expect("read"), b"abc\n");
        assert_eq!(
            anlegen(o.pfad("fehlt/t").to_str().expect("utf8"), b"x")
                .expect_err("no folder")
                .code,
            3
        );
        assert!(angelegen_ist_noch_da(&angelegt, &p));
    }

    fn angelegen_ist_noch_da(a: &Angelegt, p: &Path) -> bool {
        a.dev != 0 && p.exists()
    }

    #[test]
    fn the_file_that_was_made_is_removed_through_the_folder() {
        let o = Ordner::neu();
        let p = o.pfad("t");
        let a = anlegen(p.to_str().expect("utf8"), b"tok\n").expect("made");
        assert!(a.entfernen());
        assert!(!p.exists());
        assert!(!a.entfernen(), "a second removal finds nothing");
    }

    #[test]
    fn a_file_put_in_its_place_with_another_inode_stays() {
        let o = Ordner::neu();
        let p = o.pfad("t");
        let a = anlegen(p.to_str().expect("utf8"), b"tok\n").expect("made");
        fs::remove_file(&p).expect("rm");
        fs::write(&p, b"fremd").expect("other file");
        assert!(!a.entfernen());
        assert_eq!(fs::read(&p).expect("read"), b"fremd");
    }

    #[test]
    fn a_link_put_in_its_place_is_not_followed_and_stays() {
        let o = Ordner::neu();
        let p = o.pfad("t");
        let ziel = o.pfad("ziel");
        fs::write(&ziel, b"opfer").expect("target");
        let a = anlegen(p.to_str().expect("utf8"), b"tok\n").expect("made");
        fs::remove_file(&p).expect("rm");
        symlink(&ziel, &p).expect("link");
        assert!(!a.entfernen());
        assert!(fs::symlink_metadata(&p).is_ok(), "the link stays");
        assert_eq!(fs::read(&ziel).expect("read"), b"opfer");
    }

    #[test]
    fn the_folder_renamed_after_the_start_and_replaced_by_a_link_spares_the_victim() {
        let o = Ordner::neu();
        let dir = o.pfad("d");
        let opfer = o.pfad("opfer");
        fs::create_dir(&dir).expect("d");
        fs::create_dir(&opfer).expect("opfer");
        fs::write(opfer.join("t"), b"opfer").expect("victim file");
        let a = anlegen(dir.join("t").to_str().expect("utf8"), b"tok\n").expect("made");
        fs::rename(&dir, o.pfad("d2")).expect("rename");
        symlink(&opfer, &dir).expect("link");
        assert!(a.entfernen());
        assert!(
            !o.pfad("d2/t").exists(),
            "the file of the original folder went"
        );
        assert_eq!(fs::read(opfer.join("t")).expect("read"), b"opfer");
    }

    #[test]
    fn names_with_a_slash_dots_and_the_empty_name_are_refused() {
        for n in ["", ".", "..", "a/b", "a\0b"] {
            assert!(!name_gut(n), "{n:?}");
        }
        assert!(name_gut("t"));
    }
}
