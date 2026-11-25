//! The store of version 1 under Linux (section 2.3): `<heim>/.local/state/agent-workbench/halter/geheim/`, one file 0600
//! per secret, named after the secret. The home comes from field 6 of the own entry in `/etc/passwd`; `HOME` and
//! `XDG_STATE_HOME` never count.
//!
//! Every level is opened relative to the one before with `O_NOFOLLOW`; each must be a folder of the own account that
//! group and others cannot write, and `geheim` must be exactly 0700. A missing level is made with 0700. A file is read
//! anew at every use (`O_NOFOLLOW`, regular, own account, 0600): the value and exactly one LF, every byte of the value
//! in `\x21` to `\x7e`. Names with a dot at the start (temporary files of `werkbank-anmelden`) are never a secret.

use std::io::Read;
use std::os::fd::{AsFd, OwnedFd};
use std::path::{Path, PathBuf};

use rustix::fs::{Dir, FileType, Mode, OFlags, fstat, mkdirat, openat};
use rustix::io::Errno;

use crate::form::ist_name;
use crate::geheim::Geheim;

/// The levels below the home, in order.
pub const EBENEN: [&str; 5] = [".local", "state", "agent-workbench", "halter", "geheim"];

/// Largest file of the store: Base64 of 16384 characters holds 12288 bytes, plus the LF.
pub const DATEI_MAX: usize = 12_289;

/// A level breaks the rule, or cannot be opened or made (step 3, exit 78).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rechte;

/// The store of one account.
#[derive(Debug, Clone)]
pub struct Ablage {
    heim: PathBuf,
    konto: u32,
}

// Flags are joined with `union`: `|` and `^` give the same value for disjoint flags, so that mutant could never fail.
fn ordner_flags() -> OFlags {
    OFlags::RDONLY
        .union(OFlags::DIRECTORY)
        .union(OFlags::NOFOLLOW)
        .union(OFlags::CLOEXEC)
}

/// Field 6 (the home) of the entry of `uid` in a file of the form of `/etc/passwd`.
#[must_use]
pub fn heim_aus_passwd(inhalt: &str, uid: u32) -> Option<PathBuf> {
    inhalt.lines().find_map(|zeile| {
        let felder: Vec<&str> = zeile.split(':').collect();
        (felder.len() == 7
            && felder[2].parse::<u32>().ok() == Some(uid)
            && felder[5].starts_with('/'))
        .then(|| PathBuf::from(felder[5]))
    })
}

impl Ablage {
    /// The store below `heim` for the account `konto`.
    #[must_use]
    pub fn neu(heim: &Path, konto: u32) -> Self {
        Ablage {
            heim: heim.to_path_buf(),
            konto,
        }
    }

    fn ebene_ok(&self, fd: &OwnedFd, letzte: bool) -> bool {
        let Ok(st) = fstat(fd) else {
            return false;
        };
        let modus = st.st_mode & 0o7777;
        FileType::from_raw_mode(st.st_mode) == FileType::Directory
            && st.st_uid == self.konto
            && modus & 0o022 == 0
            && (!letzte || modus == 0o700)
    }

    /// Opens the folder `geheim` along every level; `anlegen` makes missing levels with 0700 (step 3).
    ///
    /// # Errors
    /// [`Rechte`] if a level is a link, not a folder, not the own one, writable for group or others, `geheim` not 0700,
    /// or if a level is missing and `anlegen` is false.
    pub fn oeffnen(&self, anlegen: bool) -> Result<OwnedFd, Rechte> {
        let mut fd = openat(rustix::fs::CWD, &self.heim, ordner_flags(), Mode::empty())
            .map_err(|_| Rechte)?;
        for (i, name) in EBENEN.iter().enumerate() {
            let naechste = match openat(&fd, *name, ordner_flags(), Mode::empty()) {
                Ok(n) => n,
                Err(Errno::NOENT) if anlegen => {
                    match mkdirat(&fd, *name, Mode::from_raw_mode(0o700)) {
                        Ok(()) | Err(Errno::EXIST) => {}
                        Err(_) => return Err(Rechte),
                    }
                    openat(&fd, *name, ordner_flags(), Mode::empty()).map_err(|_| Rechte)?
                }
                Err(_) => return Err(Rechte),
            };
            if !self.ebene_ok(&naechste, i + 1 == EBENEN.len()) {
                return Err(Rechte);
            }
            fd = naechste;
        }
        Ok(fd)
    }

    /// The names of all files in `geheim` that have the form of a secret (no dot at the start).
    #[must_use]
    pub fn namen(&self) -> Vec<String> {
        let Ok(fd) = self.oeffnen(false) else {
            return Vec::new();
        };
        let Ok(dir) = Dir::read_from(fd.as_fd()) else {
            return Vec::new();
        };
        let mut namen: Vec<String> = dir
            .filter_map(Result::ok)
            .filter_map(|e| e.file_name().to_str().ok().map(str::to_owned))
            .filter(|n| !n.starts_with('.') && ist_name(n))
            .collect();
        namen.sort();
        namen
    }

    /// The value of the secret `name`, read anew; `None` if it is missing or breaks the rules of a file of the store.
    #[must_use]
    pub fn lesen(&self, name: &str) -> Option<Geheim> {
        if !ist_name(name) {
            return None; // no slash, no "." or "..": a name always stays in `geheim`
        }
        let ordner = self.oeffnen(false).ok()?;
        let fd = openat(
            &ordner,
            name,
            OFlags::RDONLY
                .union(OFlags::NOFOLLOW)
                .union(OFlags::CLOEXEC)
                .union(OFlags::NONBLOCK),
            Mode::empty(),
        )
        .ok()?;
        let st = fstat(&fd).ok()?;
        if !datei_regel(st.st_mode, st.st_uid, self.konto) {
            return None;
        }
        let mut inhalt = Vec::new();
        let mut datei = std::fs::File::from(fd);
        let gelesen = (&mut datei)
            .take(DATEI_MAX as u64 + 1)
            .read_to_end(&mut inhalt);
        let inhalt = Geheim::neu(inhalt);
        gelesen.ok()?;
        wert_aus_datei(inhalt.bytes()).map(|w| Geheim::neu(w.to_vec()))
    }

    /// Every secret of the store with its value.
    #[must_use]
    pub fn alle(&self) -> Vec<(String, Geheim)> {
        self.namen()
            .into_iter()
            .filter_map(|n| self.lesen(&n).map(|w| (n, w)))
            .collect()
    }
}

/// The rule of a file of the store after `fstat`: a regular file of the account `konto` with exactly 0600. A pure
/// function, so that a test can give every kind of file (the read after it fails for every other kind anyway).
#[must_use]
pub fn datei_regel(modus: rustix::fs::RawMode, uid: u32, konto: u32) -> bool {
    FileType::from_raw_mode(modus) == FileType::RegularFile
        && uid == konto
        && modus & 0o7777 == 0o600
}

/// The value of the content of a file of the store: exactly one LF at the end, every byte before it in `\x21`..`\x7e`.
#[must_use]
pub fn wert_aus_datei(inhalt: &[u8]) -> Option<&[u8]> {
    if inhalt.len() > DATEI_MAX {
        return None;
    }
    let wert = inhalt.strip_suffix(b"\n")?;
    (!wert.is_empty() && wert.iter().all(|c| (0x21..=0x7e).contains(c))).then_some(wert)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn form_der_datei() {
        assert_eq!(wert_aus_datei(b"abc\n"), Some(&b"abc"[..]));
        assert_eq!(wert_aus_datei(b"abc"), None);
        assert_eq!(wert_aus_datei(b"abc\n\n"), None);
        assert_eq!(wert_aus_datei(b"a c\n"), None);
        assert_eq!(wert_aus_datei(b"\n"), None);
        assert_eq!(wert_aus_datei(b"a\xc3\xa4\n"), None);
    }

    #[test]
    fn regel_der_datei_mit_art_eigner_und_modus() {
        assert!(datei_regel(0o100_600, 1000, 1000));
        // another kind with 0600
        for art in [0o040_000, 0o010_000, 0o140_000, 0o020_000, 0o120_000] {
            assert!(!datei_regel(art | 0o600, 1000, 1000), "{art:o}");
        }
        // another owner
        assert!(!datei_regel(0o100_600, 1001, 1000));
        assert!(!datei_regel(0o100_600, 0, 1000));
        // another mode, also with a bit above 0777
        for m in [0o644, 0o400, 0o700, 0o4600, 0o1600, 0o000] {
            assert!(!datei_regel(0o100_000 | m, 1000, 1000), "{m:o}");
        }
    }

    #[test]
    fn heim_aus_der_passwd() {
        let p = "root:x:0:0:root:/root:/bin/sh\nperson:x:1000:1000::/srv/beispiel/person:/bin/bash\nkaputt:x:1001\n";
        assert_eq!(
            heim_aus_passwd(p, 1000),
            Some(PathBuf::from("/srv/beispiel/person"))
        );
        assert_eq!(heim_aus_passwd(p, 1001), None);
        assert_eq!(heim_aus_passwd(p, 7), None);
    }
}
