//! The conversations of the harnesses under `lauf.gespraeche` (contract, Nachtrag 27.11.2026, DI-E4): `Loeschen` removes
//! `<lauf.gespraeche>/<kennung>` of the deleted session and of its ended workers. Under Windows there is no such folder yet: a start
//! file that names one makes every `Loeschen` with a conversation end with `gespraech_nicht_geloescht` (fail closed).
//!
//! SECURITY: no path put together from strings reaches a call except `lauf.gespraeche` itself. The folder is opened once with
//! `O_RDONLY | O_DIRECTORY | O_NOFOLLOW | O_CLOEXEC`, `fstat` must show the effective user as owner and mode 0700, and every entry
//! below it is removed relative to that descriptor by [`crate::loeschen::sitzung_entfernen`] (no link is followed, a folder only when
//! `openat` finds what `statat` saw). Afterwards `statat` without following a link must find no entry.

/// Removes `<wurzel>/<name>` for every name, all through one descriptor of `wurzel`. A missing folder or entry is no error.
///
/// # Errors
/// The reason of the line on stderr: `gespraeche_unsicher` (no folder, a link, another owner, not mode 0700), `oeffnen` (another error
/// at opening; under Windows always) or `rest` (an entry is still there afterwards).
pub(crate) fn entfernen(wurzel: &str, namen: &[String]) -> Result<(), &'static str> {
    #[cfg(unix)]
    {
        unix::entfernen(wurzel, namen)
    }
    #[cfg(not(unix))]
    {
        let _ = (wurzel, namen);
        Err("oeffnen")
    }
}

#[cfg(unix)]
mod unix {
    use std::os::fd::AsFd;

    use rustix::fs::{AtFlags, CWD, Mode, fstat, openat, statat};
    use rustix::io::Errno;

    use crate::loeschen::{ORDNER_OFFEN, sitzung_entfernen};

    /// See [`super::entfernen`].
    pub(super) fn entfernen(wurzel: &str, namen: &[String]) -> Result<(), &'static str> {
        let fd = match openat(CWD, wurzel, ORDNER_OFFEN, Mode::empty()) {
            Ok(fd) => fd,
            Err(Errno::NOENT) => return Ok(()),
            Err(Errno::LOOP | Errno::NOTDIR) => return Err("gespraeche_unsicher"),
            Err(_) => return Err("oeffnen"),
        };
        let st = fstat(&fd).map_err(|_| "oeffnen")?;
        if st.st_uid != crate::sys::euid() || st.st_mode & 0o7777 != 0o700 {
            return Err("gespraeche_unsicher");
        }
        for name in namen {
            sitzung_entfernen(fd.as_fd(), name);
        }
        let rest = namen.iter().any(|name| {
            statat(&fd, name.as_str(), AtFlags::SYMLINK_NOFOLLOW).err() != Some(Errno::NOENT)
        });
        if rest { Err("rest") } else { Ok(()) }
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::entfernen;
    use std::fs;
    use std::os::unix::fs::{PermissionsExt, symlink};
    use std::path::PathBuf;

    fn probe(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("gs-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).expect("dir");
        p
    }

    fn ordner(p: &std::path::Path, modus: u32) {
        fs::create_dir_all(p).expect("dir");
        fs::set_permissions(p, fs::Permissions::from_mode(modus)).expect("mode");
    }

    fn namen(n: &[&str]) -> Vec<String> {
        n.iter().map(|x| (*x).to_owned()).collect()
    }

    /// The named conversations go with everything in them; a link inside is removed as a link and its target outside stays; another
    /// entry stays; a missing entry is no error.
    #[test]
    fn entfernt_nur_die_genannten_und_folgt_keinem_verweis() {
        let o = probe("gut");
        let g = o.join("g");
        ordner(&g, 0o700);
        fs::create_dir_all(o.join("aussen")).expect("dir");
        fs::write(o.join("aussen/a.txt"), b"a").expect("file");
        for s in ["s-1", "s-2", "s-9"] {
            fs::create_dir_all(g.join(s).join("tief/er")).expect("dir");
            fs::write(g.join(s).join("tief/er/x"), b"x").expect("file");
            symlink(o.join("aussen"), g.join(s).join("ordner")).expect("link");
            symlink(o.join("aussen/a.txt"), g.join(s).join("datei")).expect("link");
        }
        assert_eq!(
            entfernen(g.to_str().expect("utf8"), &namen(&["s-1", "s-2", "s-5"])),
            Ok(())
        );
        let mut da: Vec<String> = fs::read_dir(&g)
            .expect("dir")
            .map(|e| e.expect("entry").file_name().to_string_lossy().into_owned())
            .collect();
        da.sort();
        assert_eq!(da, ["s-9"]);
        assert_eq!(fs::read(o.join("aussen/a.txt")).expect("target"), b"a");
        let _ = fs::remove_dir_all(&o);
    }

    /// A missing folder is no error; a link to a good folder, a file, another mode and an unknown name that stays give their reasons.
    #[test]
    fn die_gruende() {
        let o = probe("gruende");
        let g = o.join("g");
        let pfad = |p: &PathBuf| p.to_str().expect("utf8").to_owned();
        assert_eq!(entfernen(&pfad(&g), &namen(&["s-1"])), Ok(()));
        ordner(&g, 0o700);
        fs::create_dir(g.join("s-1")).expect("dir");
        symlink(&g, o.join("link")).expect("link");
        assert_eq!(
            entfernen(&pfad(&o.join("link")), &namen(&["s-1"])),
            Err("gespraeche_unsicher")
        );
        fs::write(o.join("datei"), b"x").expect("file");
        assert_eq!(
            entfernen(&pfad(&o.join("datei")), &namen(&["s-1"])),
            Err("gespraeche_unsicher")
        );
        for modus in [0o755, 0o750, 0o701, 0o1700] {
            fs::set_permissions(&g, fs::Permissions::from_mode(modus)).expect("mode");
            assert_eq!(
                entfernen(&pfad(&g), &namen(&["s-1"])),
                Err("gespraeche_unsicher"),
                "{modus:o}"
            );
        }
        fs::set_permissions(&g, fs::Permissions::from_mode(0o700)).expect("mode");
        assert!(g.join("s-1").exists(), "nothing was removed");
        // a name that the removal refuses stays: the check afterwards finds it (`rest`); the good name is gone all the same
        fs::create_dir(g.join("x")).expect("dir");
        assert_eq!(entfernen(&pfad(&g), &namen(&["s-1", "x"])), Err("rest"));
        assert!(!g.join("s-1").exists());
        assert_eq!(entfernen(&pfad(&g), &namen(&[])), Ok(()));
        // another error at opening (a path the system refuses) is `oeffnen`, an empty path is a missing folder
        assert_eq!(entfernen("/x\0y", &namen(&["s-1"])), Err("oeffnen"));
        assert_eq!(entfernen("", &namen(&["s-1"])), Ok(()));
        let _ = fs::remove_dir_all(&o);
    }
}
