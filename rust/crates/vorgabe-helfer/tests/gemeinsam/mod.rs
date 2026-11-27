//! Test help: the file calls of the system with injected errors and reported owners, and a folder per test.
//!
//! Without root no test can give a file to root or to another account. [`Gestoert`] therefore reports, for an entry
//! that belongs to the test user, the owner its inode is mapped to (default: the uid of the helper). Every other call
//! goes to the system unchanged, unless an injected error stops it.

#![allow(dead_code)]

use std::cell::Cell;
use std::collections::{BTreeSet, HashMap};
use std::io;
use std::os::fd::{BorrowedFd, OwnedFd};
use std::path::{Path, PathBuf};

use vorgabe_helfer::sys::{Dateien, Echt, Meta};

/// The uid the tests give to `werkbank-vorgabe`.
pub const HELFER_UID: u32 = 990;

/// The calls of the system with injected errors.
///
/// - `temp`: the first `anlegen` fails; `rueckweg`: every `anlegen` after the first fails (the way back);
/// - `schreiben`, `chmod`, `fsync`, `rename`: the first such call fails;
/// - `ordner_fsync`: the first `fsync` of the folder fails; `ordner_fsync_zwei`: the second one fails;
/// - `exist:<n>`: the first n `anlegen` fail with `EEXIST`;
/// - `lesen`: reading the file fails; `fstat_ordner`/`fstat_datei`: `fstat` of the folder or the file fails.
pub struct Gestoert {
    pub fehler: BTreeSet<String>,
    pub eigner: HashMap<u64, u32>,
    pub eigene_uid: u32,
    pub angelegt: Cell<u32>,
    pub geschrieben: Cell<u32>,
    pub chmod: Cell<u32>,
    pub syncs: Cell<u32>,
    pub renames: Cell<u32>,
    pub ordner_syncs: Cell<u32>,
    pub fstats: Cell<u32>,
    pub entfernt: std::cell::RefCell<Vec<String>>,
}

impl Gestoert {
    pub fn neu(fehler: &[&str]) -> Gestoert {
        Gestoert {
            fehler: fehler.iter().map(|s| (*s).to_owned()).collect(),
            eigner: HashMap::new(),
            eigene_uid: rustix::process::getuid().as_raw(),
            angelegt: Cell::new(0),
            geschrieben: Cell::new(0),
            chmod: Cell::new(0),
            syncs: Cell::new(0),
            renames: Cell::new(0),
            ordner_syncs: Cell::new(0),
            fstats: Cell::new(0),
            entfernt: std::cell::RefCell::new(Vec::new()),
        }
    }

    /// Reports `uid` as the owner of the entry at `pfad` (by its inode, without following a link).
    pub fn eigner_von(&mut self, pfad: &Path, uid: u32) {
        let m = Echt.lstat(pfad).expect("lstat");
        self.eigner.insert(m.ino, uid);
    }

    fn hat(&self, f: &str) -> bool {
        self.fehler.contains(f)
    }

    fn melden(&self, mut m: Meta) -> Meta {
        if m.uid == self.eigene_uid {
            m.uid = *self.eigner.get(&m.ino).unwrap_or(&HELFER_UID);
        }
        m
    }

    fn exist(&self) -> u32 {
        self.fehler
            .iter()
            .find_map(|f| f.strip_prefix("exist:"))
            .and_then(|n| n.parse().ok())
            .unwrap_or(0)
    }
}

fn kaputt() -> io::Error {
    io::Error::other("injected")
}

fn zaehle(c: &Cell<u32>) -> u32 {
    let n = c.get() + 1;
    c.set(n);
    n
}

impl Dateien for Gestoert {
    fn lstat(&self, pfad: &Path) -> io::Result<Meta> {
        Echt.lstat(pfad).map(|m| self.melden(m))
    }

    fn ordner_oeffnen(&self, pfad: &Path) -> io::Result<OwnedFd> {
        Echt.ordner_oeffnen(pfad)
    }

    fn fstat(&self, fd: BorrowedFd<'_>) -> io::Result<Meta> {
        let n = zaehle(&self.fstats);
        if (n == 1 && self.hat("fstat_ordner")) || (n == 2 && self.hat("fstat_datei")) {
            return Err(kaputt());
        }
        Echt.fstat(fd).map(|m| self.melden(m))
    }

    fn oeffnen(&self, ordner: BorrowedFd<'_>, name: &str) -> io::Result<OwnedFd> {
        Echt.oeffnen(ordner, name)
    }

    fn lesen(&self, fd: BorrowedFd<'_>, max: usize) -> io::Result<Vec<u8>> {
        if self.hat("lesen") {
            return Err(kaputt());
        }
        Echt.lesen(fd, max)
    }

    fn anlegen(&self, ordner: BorrowedFd<'_>, name: &str) -> io::Result<OwnedFd> {
        let n = zaehle(&self.angelegt);
        if n <= self.exist() {
            return Err(io::Error::from(io::ErrorKind::AlreadyExists));
        }
        let n = n - self.exist();
        if (n == 1 && self.hat("temp")) || (n > 1 && self.hat("rueckweg")) {
            return Err(kaputt());
        }
        Echt.anlegen(ordner, name)
    }

    fn schreiben(&self, fd: BorrowedFd<'_>, bytes: &[u8]) -> io::Result<()> {
        if zaehle(&self.geschrieben) == 1 && self.hat("schreiben") {
            // half the bytes, then the error: a part must never become the file
            let _ = Echt.schreiben(fd, &bytes[..bytes.len() / 2]);
            return Err(kaputt());
        }
        Echt.schreiben(fd, bytes)
    }

    fn modus_setzen(&self, fd: BorrowedFd<'_>, modus: u32) -> io::Result<()> {
        if zaehle(&self.chmod) == 1 && self.hat("chmod") {
            return Err(kaputt());
        }
        Echt.modus_setzen(fd, modus)
    }

    fn datei_sync(&self, fd: BorrowedFd<'_>) -> io::Result<()> {
        if zaehle(&self.syncs) == 1 && self.hat("fsync") {
            return Err(kaputt());
        }
        Echt.datei_sync(fd)
    }

    fn umbenennen(&self, ordner: BorrowedFd<'_>, von: &str, nach: &str) -> io::Result<()> {
        if zaehle(&self.renames) == 1 && self.hat("rename") {
            return Err(kaputt());
        }
        Echt.umbenennen(ordner, von, nach)
    }

    fn ordner_sync(&self, ordner: BorrowedFd<'_>) -> io::Result<()> {
        let n = zaehle(&self.ordner_syncs);
        if (n == 1 && self.hat("ordner_fsync")) || (n == 2 && self.hat("ordner_fsync_zwei")) {
            return Err(kaputt());
        }
        Echt.ordner_sync(ordner)
    }

    fn entfernen(&self, ordner: BorrowedFd<'_>, name: &str) -> io::Result<()> {
        self.entfernt.borrow_mut().push(name.to_owned());
        Echt.entfernen(ordner, name)
    }
}

/// A new empty folder of this test below the temporary folder, removed by [`Ort::drop`].
pub struct Ort(pub PathBuf);

impl Ort {
    pub fn neu(name: &str) -> Ort {
        let p = std::env::temp_dir().join(format!("vh-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).expect("folder");
        Ort(p)
    }
}

impl Drop for Ort {
    fn drop(&mut self) {
        // modes like 0555 on a folder of the test would stop the removal
        for e in walk(&self.0) {
            let _ =
                std::fs::set_permissions(&e, std::os::unix::fs::PermissionsExt::from_mode(0o755));
        }
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn walk(p: &Path) -> Vec<PathBuf> {
    let mut aus = vec![p.to_path_buf()];
    if let Ok(rd) = std::fs::read_dir(p) {
        for e in rd.flatten() {
            if e.file_type().is_ok_and(|t| t.is_dir()) {
                aus.extend(walk(&e.path()));
            }
        }
    }
    aus
}

/// The names in `ordner`, sorted.
pub fn namen(ordner: &Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(ordner)
        .expect("read_dir")
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    v.sort();
    v
}

/// Sets the mode of `pfad`.
pub fn modus(pfad: &Path, m: u32) {
    std::fs::set_permissions(pfad, std::os::unix::fs::PermissionsExt::from_mode(m)).expect("chmod");
}
