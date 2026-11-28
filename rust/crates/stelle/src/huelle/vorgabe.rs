//! The default of the admins for workers (contract 6, section 14.5), read on descriptors only.
//!
//! From `/` down, each folder is opened with `O_NOFOLLOW` and `O_DIRECTORY` relative to the one before and checked
//! with `fstat`; the file itself is opened with `O_NOFOLLOW` and checked with `fstat` before a byte is read. No step
//! checks a path text and opens it afterwards. Any doubt gives `None`: then there are no rules (fail closed).

use std::ffi::OsStr;
use std::fs::File;
use std::io::Read;
use std::os::fd::{AsFd, OwnedFd};
use std::path::{Component, Path};

use rustix::fs::{CWD, FileType, Mode, OFlags, fstat, openat};

use crate::worker::VORGABE_MAX;

const ORDNER: OFlags = OFlags::RDONLY
    .union(OFlags::DIRECTORY)
    .union(OFlags::NOFOLLOW)
    .union(OFlags::CLOEXEC);
const DATEI: OFlags = OFlags::RDONLY
    .union(OFlags::NOFOLLOW)
    .union(OFlags::NONBLOCK)
    .union(OFlags::CLOEXEC);

fn eigner_gut(eigner: u32, zweiter: Option<u32>) -> bool {
    eigner == 0 || zweiter == Some(eigner)
}

/// A folder above the default: a folder of an allowed owner that neither the group nor others may write; one that
/// others may write is allowed only with the sticky bit, and never as the last folder.
pub(crate) fn ordner_gut(
    art: FileType,
    eigner: u32,
    modus: u32,
    zweiter: Option<u32>,
    letzter: bool,
) -> bool {
    let schreibbar = modus & 0o022 != 0;
    let klebend = modus & 0o1000 != 0;
    art == FileType::Directory
        && eigner_gut(eigner, zweiter)
        && (!schreibbar || (klebend && !letzter))
}

/// The default itself: a regular file of an allowed owner that neither the group nor others may write.
pub(crate) fn datei_gut(art: FileType, eigner: u32, modus: u32, zweiter: Option<u32>) -> bool {
    art == FileType::RegularFile && eigner_gut(eigner, zweiter) && modus & 0o022 == 0
}

/// The mode bits as `u32` (`mode_t` has 32 bits under Linux, 16 under macOS).
#[cfg_attr(
    target_os = "linux",
    allow(
        clippy::useless_conversion,
        reason = "mode_t is u32 under Linux, u16 under macOS"
    )
)]
fn modus(st: &rustix::fs::Stat) -> u32 {
    u32::from(st.st_mode)
}

fn pruefe_ordner(fd: &OwnedFd, zweiter: Option<u32>, letzter: bool) -> Option<()> {
    let st = fstat(fd).ok()?;
    let art = FileType::from_raw_mode(st.st_mode);
    ordner_gut(art, st.st_uid, modus(&st), zweiter, letzter).then_some(())
}

/// The bytes of the default at `pfad` (absolute, without `.`, `..` or an empty part), when every folder on the way
/// and the file itself pass. `zweiter` is the one owner besides root a test build allows (the own user); the
/// delivered build passes `None`.
#[must_use]
pub fn lesen(pfad: &Path, zweiter: Option<u32>) -> Option<Vec<u8>> {
    let wurzel = openat(CWD, "/", ORDNER, Mode::empty()).ok()?;
    lesen_ab(wurzel, &teile(pfad)?, zweiter)
}

/// The parts of an absolute path below `/`; `None` for a relative path and for `..` (`Path::components` drops `.` in the
/// middle and empty parts, so they name the same file).
fn teile(pfad: &Path) -> Option<Vec<&OsStr>> {
    let mut k = pfad.components();
    (k.next()? == Component::RootDir).then_some(())?;
    k.map(|t| match t {
        Component::Normal(name) => Some(name),
        _ => None,
    })
    .collect()
}

/// The walk below the open folder `wurzel` (`/` for [`lesen`]): each folder relative to the one before, then the file.
fn lesen_ab(wurzel: OwnedFd, teile: &[&OsStr], zweiter: Option<u32>) -> Option<Vec<u8>> {
    let (datei, ordner) = teile.split_last()?;
    let mut fd = wurzel;
    pruefe_ordner(&fd, zweiter, ordner.is_empty())?;
    for (stelle, name) in ordner.iter().enumerate() {
        fd = openat(fd.as_fd(), *name, ORDNER, Mode::empty()).ok()?;
        pruefe_ordner(&fd, zweiter, stelle + 1 == ordner.len())?;
    }
    let datei = openat(fd.as_fd(), *datei, DATEI, Mode::empty()).ok()?;
    let st = fstat(&datei).ok()?;
    if !datei_gut(
        FileType::from_raw_mode(st.st_mode),
        st.st_uid,
        modus(&st),
        zweiter,
    ) {
        return None;
    }
    let mut bytes = Vec::new();
    let grenze = u64::try_from(VORGABE_MAX).ok()? + 1;
    File::from(datei)
        .take(grenze)
        .read_to_end(&mut bytes)
        .ok()?;
    (bytes.len() <= VORGABE_MAX).then_some(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::{PermissionsExt, symlink};
    use std::path::PathBuf;

    struct Probe(PathBuf);

    impl Probe {
        fn neu(name: &str) -> Self {
            let basis = std::env::temp_dir().canonicalize().expect("tmp");
            let p = basis.join(format!("sv-{}-{name}", std::process::id()));
            let _ = fs::remove_dir_all(&p);
            fs::create_dir_all(p.join("etc")).expect("dir");
            fs::set_permissions(&p, fs::Permissions::from_mode(0o755)).expect("chmod");
            fs::set_permissions(p.join("etc"), fs::Permissions::from_mode(0o755)).expect("chmod");
            Self(p)
        }
    }

    impl Drop for Probe {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[allow(
        clippy::unnecessary_wraps,
        reason = "the checks take the second owner as an option"
    )]
    fn ich() -> Option<u32> {
        Some(rustix::process::geteuid().as_raw())
    }

    /// Reads `rel` below the folder of the probe as if it were `/`: the probe belongs to the own user, so the walk does not
    /// depend on the owners of the folders above it (in a user namespace `/` belongs to the overflow account).
    fn lies(p: &Probe, rel: &str, zweiter: Option<u32>) -> Option<Vec<u8>> {
        let wurzel = openat(CWD, p.0.as_path(), ORDNER, Mode::empty()).expect("probe");
        let pfad = Path::new("/").join(rel);
        lesen_ab(wurzel, &teile(&pfad)?, zweiter)
    }

    #[test]
    fn eine_gute_vorgabe_wird_gelesen_und_jede_unsichere_nicht() {
        let p = Probe::neu("gut");
        let f = p.0.join("etc/vorgabe.json");
        fs::write(&f, "abc").expect("file");
        fs::set_permissions(&f, fs::Permissions::from_mode(0o644)).expect("chmod");
        assert_eq!(lies(&p, "etc/vorgabe.json", ich()), Some(b"abc".to_vec()));
        // the delivered build allows root only
        assert_eq!(lies(&p, "etc/vorgabe.json", None), None);
        // writable by the group or others
        for modus in [0o664, 0o646, 0o666] {
            fs::set_permissions(&f, fs::Permissions::from_mode(modus)).expect("chmod");
            assert_eq!(lies(&p, "etc/vorgabe.json", ich()), None, "{modus:o}");
        }
        fs::set_permissions(&f, fs::Permissions::from_mode(0o600)).expect("chmod");
        assert!(lies(&p, "etc/vorgabe.json", ich()).is_some());
        // a folder on the way writable by others, even with the sticky bit as the last folder; with the sticky bit above
        // the last folder it is allowed
        fs::set_permissions(p.0.join("etc"), fs::Permissions::from_mode(0o1777)).expect("chmod");
        assert_eq!(lies(&p, "etc/vorgabe.json", ich()), None);
        fs::set_permissions(p.0.join("etc"), fs::Permissions::from_mode(0o775)).expect("chmod");
        assert_eq!(lies(&p, "etc/vorgabe.json", ich()), None);
        fs::create_dir(p.0.join("etc/unter")).expect("dir");
        fs::set_permissions(p.0.join("etc/unter"), fs::Permissions::from_mode(0o755))
            .expect("chmod");
        fs::write(p.0.join("etc/unter/v.json"), "x").expect("file");
        fs::set_permissions(p.0.join("etc"), fs::Permissions::from_mode(0o1777)).expect("chmod");
        assert_eq!(lies(&p, "etc/unter/v.json", ich()), Some(b"x".to_vec()));
        fs::set_permissions(p.0.join("etc/unter"), fs::Permissions::from_mode(0o1777))
            .expect("chmod");
        assert_eq!(lies(&p, "etc/unter/v.json", ich()), None);
        fs::set_permissions(p.0.join("etc"), fs::Permissions::from_mode(0o755)).expect("chmod");
        // a file right below the root: the root is the last folder
        fs::write(p.0.join("oben.json"), "o").expect("file");
        assert_eq!(lies(&p, "oben.json", ich()), Some(b"o".to_vec()));
        fs::set_permissions(&p.0, fs::Permissions::from_mode(0o1777)).expect("chmod");
        assert_eq!(lies(&p, "oben.json", ich()), None);
        assert_eq!(
            lies(&p, "etc/vorgabe.json", ich()).map(|b| b.len()),
            Some(3)
        );
        fs::set_permissions(&p.0, fs::Permissions::from_mode(0o755)).expect("chmod");
        // too large
        fs::write(&f, vec![b' '; VORGABE_MAX]).expect("file");
        assert_eq!(
            lies(&p, "etc/vorgabe.json", ich()).map(|b| b.len()),
            Some(VORGABE_MAX)
        );
        fs::write(&f, vec![b' '; VORGABE_MAX + 1]).expect("file");
        assert_eq!(lies(&p, "etc/vorgabe.json", ich()), None);
    }

    #[test]
    fn ein_link_oder_ein_krummer_pfad_wird_nie_gelesen() {
        let p = Probe::neu("link");
        let echt = p.0.join("etc/echt.json");
        fs::write(&echt, "abc").expect("file");
        fs::set_permissions(&echt, fs::Permissions::from_mode(0o644)).expect("chmod");
        symlink(&echt, p.0.join("etc/link.json")).expect("link");
        assert_eq!(lies(&p, "etc/link.json", ich()), None);
        symlink(p.0.join("etc"), p.0.join("ordnerlink")).expect("link");
        assert_eq!(lies(&p, "ordnerlink/echt.json", ich()), None);
        // a folder in place of the file, a file in place of a folder, nothing
        assert_eq!(lies(&p, "etc", ich()), None);
        assert_eq!(lies(&p, "etc/echt.json/x", ich()), None);
        assert_eq!(lies(&p, "etc/fehlt.json", ich()), None);
        assert!(lies(&p, "etc/echt.json", ich()).is_some());
    }

    #[test]
    fn nur_ein_absoluter_pfad_ohne_rueckweg_hat_teile() {
        let t = |p: &str| {
            teile(Path::new(p)).map(|v| {
                v.iter()
                    .map(|o| o.to_string_lossy().into_owned())
                    .collect::<Vec<_>>()
            })
        };
        assert_eq!(
            t("/etc/werkbank/vorgabe.json"),
            Some(vec!["etc".into(), "werkbank".into(), "vorgabe.json".into()])
        );
        // `.` and empty parts name the same file
        assert_eq!(
            t("/etc/./w//v.json"),
            Some(vec!["etc".into(), "w".into(), "v.json".into()])
        );
        assert_eq!(t("/"), Some(Vec::new()));
        for falsch in ["etc/v.json", "./v.json", "/etc/../v.json", "", "../v.json"] {
            assert_eq!(t(falsch), None, "{falsch}");
        }
        // the root alone is no file, a relative path never reaches the walk
        assert_eq!(lesen(Path::new("/"), ich()), None);
        assert_eq!(lesen(Path::new("etc/v.json"), ich()), None);
    }

    /// The walk from the real `/`: the delivered build allows root only, so a file below a folder of the own user is never
    /// read. That `lesen` reads a good default from `/` needs folders of root down to the file; the cases above check the
    /// walk itself below a folder of the own user.
    #[test]
    fn der_weg_beginnt_an_der_echten_wurzel() {
        let p = Probe::neu("wurzel");
        let f = p.0.join("etc/v.json");
        fs::write(&f, "w").expect("file");
        fs::set_permissions(&f, fs::Permissions::from_mode(0o644)).expect("chmod");
        assert_eq!(lesen(&f, None), None);
    }

    #[test]
    fn die_regeln_fuer_ordner_und_datei() {
        let d = FileType::Directory;
        let r = FileType::RegularFile;
        assert!(ordner_gut(d, 0, 0o755, None, false));
        assert!(ordner_gut(d, 0, 0o1777, None, false));
        assert!(!ordner_gut(d, 0, 0o1777, None, true));
        assert!(!ordner_gut(d, 0, 0o775, None, false));
        assert!(!ordner_gut(d, 0, 0o757, None, false));
        assert!(!ordner_gut(d, 5, 0o755, None, false));
        assert!(ordner_gut(d, 5, 0o755, Some(5), true));
        assert!(!ordner_gut(r, 0, 0o755, None, false));
        assert!(datei_gut(r, 0, 0o644, None));
        assert!(!datei_gut(r, 0, 0o664, None));
        assert!(!datei_gut(r, 0, 0o646, None));
        assert!(!datei_gut(r, 7, 0o644, Some(5)));
        assert!(!datei_gut(d, 0, 0o644, None));
    }
}
