//! The search for the key (contract 8, section 9a, amendment 18): after a run the runner asks whether the real key stands
//! anywhere in the work area or in the visible output. The forwarder does it itself, outside the enclosure, because it
//! reads the key files anyway; no second program reads a secret and the value never leaves the process.
//!
//! The call is `modellweg suche (--schluessel-datei <datei> | --zielliste <datei>) --in <pfad> [--in <pfad> ...]`. Stage 0
//! and the shielding of the start run before it (see [`crate::start::haupt`]). The search writes no file and no protocol
//! line, makes no socket and no connection. Its only output is one line on stdout (RFC 8785) or the fixed line of a
//! failure on stderr; the line holds four counts and never a value, a part, a length or a file name.
//!
//! The walk follows no link: every entry is looked at with `lstat` relative to its folder, a link counts as skipped and is
//! never opened, and a regular file is opened with `O_NOFOLLOW` and checked again on the open descriptor. A FIFO, a socket
//! or a device is never opened. The key stays in the memory of [`Schluessel`] (zeroed on drop) and is dropped before the
//! line is written.

use std::collections::HashSet;
use std::ffi::{CStr, CString, OsString};
use std::fs::File;
use std::io::{BufRead, BufReader, Read, Write};
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

use rustix::fs::{AtFlags, CWD, Dir, FileType, Mode, OFlags, openat, statat};

use crate::aufruf;
use crate::betriebsart::zielliste;
use crate::halter;
use crate::schluessel::{self, Schluessel};

/// Size of one read of a file in bytes. A test sets a smaller one so that the border between two reads is hit often.
pub const PUFFER: usize = 65_536;
/// Most `--in` of one call.
pub const IN_MAX: usize = 16;
/// Longest path of the call in bytes (section 4).
const PFAD_MAX: usize = 1024;
/// Deepest folder the walk enters, counted from the root of a `--in` (the root itself is depth 0). A deeper folder counts
/// as not readable, so a tree made to be deeper than the descriptors of the process never ends in a green result.
pub const TIEFE_MAX: usize = 256;

/// A failure of the call, with its code, fixed reason and exit code (section 9a, S6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Suchfehler {
    /// `mw.suche_aufruf`, exit 64.
    Aufruf,
    /// `mw.ziele_ungueltig`, exit 78 (the target list).
    ZieleUngueltig,
    /// `mw.schluessel_fehlt`, exit 78 (no key to search, or a key file breaks section 5).
    SchluesselFehlt,
    /// `mw.intern`, exit 70 (the line on stdout cannot be written).
    Intern,
}

impl Suchfehler {
    /// The code.
    #[must_use]
    pub fn code(self) -> &'static str {
        match self {
            Self::Aufruf => "mw.suche_aufruf",
            Self::ZieleUngueltig => "mw.ziele_ungueltig",
            Self::SchluesselFehlt => "mw.schluessel_fehlt",
            Self::Intern => "mw.intern",
        }
    }

    /// The fixed reason (ASCII).
    #[must_use]
    pub fn grund(self) -> &'static str {
        match self {
            Self::Aufruf => "Suchaufruf ungueltig",
            Self::ZieleUngueltig => "Zielliste ungueltig",
            Self::SchluesselFehlt => "Schluessel nicht lesbar",
            Self::Intern => "interner Fehler",
        }
    }

    /// The exit code.
    #[must_use]
    pub fn exit(self) -> u8 {
        match self {
            Self::Aufruf => 64,
            Self::ZieleUngueltig | Self::SchluesselFehlt => 78,
            Self::Intern => 70,
        }
    }

    /// The line on stderr: `<code>: <grund>` and a line feed.
    #[must_use]
    pub fn zeile(self) -> String {
        format!("{}: {}\n", self.code(), self.grund())
    }
}

/// The four counts of a search.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Ergebnis {
    /// Regular files read to their end.
    pub dateien: u64,
    /// Files, folders and roots that could not be read.
    pub nicht_lesbar: u64,
    /// Occurrences of all keys together.
    pub treffer: u64,
    /// Links, FIFOs, sockets and devices inside a tree.
    pub uebersprungen: u64,
}

impl Ergebnis {
    /// The exit code: 1 with a hit (also if something was not readable), 3 with nothing found and something not read, else 0.
    #[must_use]
    pub fn exit(&self) -> u8 {
        if self.treffer > 0 {
            1
        } else if self.nicht_lesbar > 0 {
            3
        } else {
            0
        }
    }

    /// The line on stdout in RFC 8785 (keys in order) and a line feed.
    #[must_use]
    pub fn zeile(&self) -> String {
        format!(
            "{{\"dateien\":{},\"nicht_lesbar\":{},\"treffer\":{},\"uebersprungen\":{}}}\n",
            self.dateien, self.nicht_lesbar, self.treffer, self.uebersprungen
        )
    }
}

/// Where the keys come from.
#[derive(Debug, PartialEq, Eq)]
enum Quelle {
    /// One key file (route local).
    Datei(PathBuf),
    /// A target list (route Copilot): from step I every bound target carries `halter_ziel` and no key file, so the
    /// search asks the holder for it (S2, step 3a) and holds no value of its own.
    Liste(PathBuf),
}

/// A call read after section 9a, S1.
#[derive(Debug, PartialEq, Eq)]
struct Aufruf {
    quelle: Quelle,
    eingaenge: Vec<PathBuf>,
}

/// S1: exactly one source (each option at most once), 1 to 16 `--in`, every path absolute without `.` or `..`, no control
/// character, at most 1024 bytes; option and value are two arguments.
fn aufruf_lesen(args: &[OsString]) -> Option<Aufruf> {
    let mut quelle = None;
    let mut eingaenge = Vec::new();
    let mut it = args.iter();
    while let Some(name) = it.next() {
        let wert = it.next()?.to_str()?;
        if !aufruf::pfad_ok(wert, PFAD_MAX) {
            return None;
        }
        let pfad = PathBuf::from(wert);
        match name.to_str()? {
            "--schluessel-datei" => {
                if quelle.replace(Quelle::Datei(pfad)).is_some() {
                    return None;
                }
            }
            "--zielliste" => {
                if quelle.replace(Quelle::Liste(pfad)).is_some() {
                    return None;
                }
            }
            "--in" => eingaenge.push(pfad),
            _ => return None,
        }
    }
    if eingaenge.is_empty() || eingaenge.len() > IN_MAX {
        return None;
    }
    Some(Aufruf {
        quelle: quelle?,
        eingaenge,
    })
}

/// The target list as bytes: own file, 0600, no link, at most `ZL_MAX + 1` bytes (the reader of the list refuses more).
fn liste_lesen(pfad: &Path, eigene_uid: u32) -> Option<Vec<u8>> {
    let flags = OFlags::NOFOLLOW
        .union(OFlags::CLOEXEC)
        .union(OFlags::NONBLOCK);
    let datei = File::from(rustix::fs::open(pfad, flags, Mode::empty()).ok()?);
    let meta = datei.metadata().ok()?;
    if !schluessel::datei_ok(meta.is_file(), meta.uid(), meta.mode(), eigene_uid) {
        return None;
    }
    let mut inhalt = Vec::new();
    let grenze = u64::try_from(zielliste::ZL_MAX.saturating_add(1)).ok()?;
    datei.take(grenze).read_to_end(&mut inhalt).ok()?;
    Some(inhalt)
}

/// What the search asks the holder for one bound target (S2, step 3a): `modell.suche` with the `--in` paths.
/// `None` means the connection, the check of the counterparty or the call failed, or the holder holds no secret for
/// that target; every one of those is `mw.schluessel_fehlt` and never a green result.
pub type Halterfrage<'a> = &'a mut dyn FnMut(&str, &[String]) -> Option<halter::Suche>;

/// S2, step 3a for the whole list: one `modell.suche` per bound target. A valid list without a bound target is
/// `mw.schluessel_fehlt`: where there is nothing to search, there is no green result.
fn halter_ziele(
    pfad: &Path,
    eigene_uid: u32,
    eingaenge: &[PathBuf],
    frage: Halterfrage<'_>,
) -> Result<Ergebnis, Suchfehler> {
    let text = liste_lesen(pfad, eigene_uid).ok_or(Suchfehler::ZieleUngueltig)?;
    let liste = zielliste::lies(&text).map_err(|_| Suchfehler::ZieleUngueltig)?;
    let ziele: Vec<String> = liste
        .ziele
        .iter()
        .filter_map(|z| z.halter_ziel.clone())
        .collect();
    if ziele.is_empty() {
        return Err(Suchfehler::SchluesselFehlt);
    }
    let pfade: Vec<String> = eingaenge
        .iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect();
    let mut summe = Ergebnis::default();
    for z in &ziele {
        let a = frage(z, &pfade).ok_or(Suchfehler::SchluesselFehlt)?;
        summe.dateien = summe.dateien.saturating_add(a.dateien);
        summe.nicht_lesbar = summe.nicht_lesbar.saturating_add(a.nicht_lesbar);
        summe.treffer = summe.treffer.saturating_add(a.treffer);
        summe.uebersprungen = summe.uebersprungen.saturating_add(a.uebersprungen);
    }
    Ok(summe)
}

/// S2, steps 4 and 5 for one key file (route local): own file, 0600, `O_NOFOLLOW`, one line. The value searched is the
/// value the forwarder inserts, without the closing line feed and without any change.
fn schluessel_holen(pfad: &Path, eigene_uid: u32) -> Result<Vec<Schluessel>, Suchfehler> {
    let k = schluessel::lies(pfad, eigene_uid).map_err(|_| Suchfehler::SchluesselFehlt)?;
    Ok(vec![k])
}

/// The first place where `nadel` (at least one byte) starts in `heu`.
fn finde(heu: &[u8], nadel: &[u8]) -> Option<usize> {
    heu.windows(nadel.len()).position(|w| w == nadel)
}

/// Counts one key over the blocks of a file (S4): every occurrence, from left to right, without overlap and also across
/// the border of two blocks. Between two blocks it keeps the bytes after the last occurrence, at most one byte less than the
/// key; a longer rest could never start an occurrence.
struct Zaehler {
    schluessel: Schluessel,
    rest: Vec<u8>,
}

impl Zaehler {
    fn neu(schluessel: Schluessel) -> Self {
        Self {
            schluessel,
            rest: Vec::new(),
        }
    }

    /// A new file starts: nothing of the old one carries over.
    fn datei_beginnt(&mut self) {
        self.rest.fill(0);
        self.rest.clear();
    }

    /// The occurrences that end inside this block (those that began in an earlier block count here).
    fn block(&mut self, block: &[u8]) -> u64 {
        let nadel = self.schluessel.bytes();
        self.rest.extend_from_slice(block);
        let mut treffer = 0_u64;
        let mut ab = 0_usize;
        while let Some(i) = finde(&self.rest[ab..], nadel) {
            treffer += 1;
            ab = ab.saturating_add(i).saturating_add(nadel.len());
        }
        let behalte = self
            .rest
            .len()
            .saturating_sub(ab)
            .min(nadel.len().saturating_sub(1));
        let weg = self.rest.len().saturating_sub(behalte);
        self.rest.drain(..weg);
        treffer
    }
}

/// The state of one walk.
struct Lauf {
    zaehler: Vec<Zaehler>,
    puffer: usize,
    gesehen: HashSet<(u64, u64)>,
    ergebnis: Ergebnis,
}

fn datei_flags() -> OFlags {
    OFlags::NOFOLLOW
        .union(OFlags::CLOEXEC)
        .union(OFlags::NONBLOCK)
}

fn ordner_flags() -> OFlags {
    OFlags::DIRECTORY
        .union(OFlags::NOFOLLOW)
        .union(OFlags::CLOEXEC)
}

impl Lauf {
    /// Counts something not read.
    fn nicht_lesbar(&mut self) {
        self.ergebnis.nicht_lesbar += 1;
    }

    /// Counts something skipped.
    fn uebersprungen(&mut self) {
        self.ergebnis.uebersprungen += 1;
    }

    /// A root of the call (S3): a regular file or a folder. A link, a missing path or another kind is not read and counts as
    /// not readable, never as "nothing found".
    fn wurzel(&mut self, pfad: &Path) {
        match CString::new(pfad.as_os_str().as_bytes()) {
            Ok(name) => self.eintrag(CWD, &name, 0, true),
            Err(_) => self.nicht_lesbar(),
        }
    }

    /// One entry below `dirfd`, looked at without following a link. `wurzel` tells a root of the call from an entry of a
    /// tree: a root of another kind is not readable, an entry of another kind is skipped.
    fn eintrag(&mut self, dirfd: BorrowedFd<'_>, name: &CStr, tiefe: usize, wurzel: bool) {
        let Ok(stat) = statat(dirfd, name, AtFlags::SYMLINK_NOFOLLOW) else {
            self.nicht_lesbar();
            return;
        };
        match FileType::from_raw_mode(stat.st_mode) {
            FileType::RegularFile => match openat(dirfd, name, datei_flags(), Mode::empty()) {
                Ok(fd) => self.datei(fd),
                Err(_) => self.nicht_lesbar(),
            },
            FileType::Directory => self.ordner_oeffnen(dirfd, name, tiefe),
            _ if wurzel => self.nicht_lesbar(),
            _ => self.uebersprungen(),
        }
    }

    fn ordner_oeffnen(&mut self, dirfd: BorrowedFd<'_>, name: &CStr, tiefe: usize) {
        if tiefe > TIEFE_MAX {
            self.nicht_lesbar();
            return;
        }
        match openat(dirfd, name, ordner_flags(), Mode::empty()) {
            Ok(fd) => {
                // A folder entered once is not entered again (a bind mount loop, a nested root).
                let f = File::from(fd);
                let Ok(meta) = f.metadata() else {
                    self.nicht_lesbar();
                    return;
                };
                if self.gesehen.insert((meta.dev(), meta.ino())) {
                    self.ordner(&OwnedFd::from(f), tiefe);
                }
            }
            Err(_) => self.nicht_lesbar(),
        }
    }

    /// Reads the names of an open folder and walks them. The folder stays open for the entries; the list of names is read
    /// first so that only one descriptor per level is held.
    fn ordner(&mut self, fd: &OwnedFd, tiefe: usize) {
        let Ok(dir) = Dir::read_from(fd) else {
            self.nicht_lesbar();
            return;
        };
        let mut namen: Vec<CString> = Vec::new();
        for e in dir {
            let Ok(e) = e else {
                self.nicht_lesbar();
                break;
            };
            let n = e.file_name();
            if !matches!(n.to_bytes(), b"." | b"..") {
                namen.push(n.to_owned());
            }
        }
        for n in &namen {
            self.eintrag(fd.as_fd(), n, tiefe + 1, false);
        }
    }

    /// Reads a file opened without following a link. What turns out not to be a regular file after the open (a swap
    /// between the look and the open) counts as skipped; a file already read through another path is not read again
    /// (same device and inode).
    fn datei(&mut self, fd: OwnedFd) {
        let f = File::from(fd);
        let Ok(meta) = f.metadata() else {
            self.nicht_lesbar();
            return;
        };
        if !meta.is_file() {
            self.uebersprungen();
            return;
        }
        if !self.gesehen.insert((meta.dev(), meta.ino())) {
            return;
        }
        for z in &mut self.zaehler {
            z.datei_beginnt();
        }
        let mut leser = BufReader::with_capacity(self.puffer, f);
        loop {
            let Ok(block) = leser.fill_buf() else {
                self.nicht_lesbar();
                return;
            };
            if block.is_empty() {
                break;
            }
            let n = block.len();
            for z in &mut self.zaehler {
                self.ergebnis.treffer += z.block(block);
            }
            leser.consume(n);
        }
        self.ergebnis.dateien += 1;
    }
}

/// Searches every root for every key and returns the four counts. `puffer` is the size of one read (at least 1).
#[must_use]
pub fn durchsuchen(schluessel: Vec<Schluessel>, eingaenge: &[PathBuf], puffer: usize) -> Ergebnis {
    let mut lauf = Lauf {
        zaehler: schluessel.into_iter().map(Zaehler::neu).collect(),
        puffer: puffer.max(1),
        gesehen: HashSet::new(),
        ergebnis: Ergebnis::default(),
    };
    for e in eingaenge {
        lauf.wurzel(e);
    }
    lauf.ergebnis
}

/// S2 and S3: the call, then the keys of a key file or the search of the holder per bound target. The keys are
/// dropped (zeroed) when this returns.
fn suchen(
    args: &[OsString],
    eigene_uid: u32,
    puffer: usize,
    frage: Halterfrage<'_>,
) -> Result<Ergebnis, Suchfehler> {
    let aufruf = aufruf_lesen(args).ok_or(Suchfehler::Aufruf)?;
    match &aufruf.quelle {
        Quelle::Datei(p) => {
            let schluessel = schluessel_holen(p, eigene_uid)?;
            Ok(durchsuchen(schluessel, &aufruf.eingaenge, puffer))
        }
        Quelle::Liste(p) => halter_ziele(p, eigene_uid, &aufruf.eingaenge, frage),
    }
}

/// Writes the fixed line of a failure on stderr and returns its exit code.
fn fehler_ausgeben(stderr: &mut dyn Write, f: Suchfehler) -> u8 {
    let _ = stderr.write_all(f.zeile().as_bytes());
    let _ = stderr.flush();
    f.exit()
}

/// The search for the arguments after `suche`, with the question to the holder given: writes the line on stdout
/// (exit 0, 1 or 3) or the fixed line on stderr (64, 78 or 70) and returns the exit code. Stage 0 and the shielding
/// are done before (see [`crate::start::haupt`]).
pub fn lauf_mit_halter(
    args: &[OsString],
    eigene_uid: u32,
    puffer: usize,
    frage: Halterfrage<'_>,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> u8 {
    match suchen(args, eigene_uid, puffer, frage) {
        Ok(e) => {
            if stdout
                .write_all(e.zeile().as_bytes())
                .and_then(|()| stdout.flush())
                .is_err()
            {
                return fehler_ausgeben(stderr, Suchfehler::Intern);
            }
            e.exit()
        }
        Err(f) => fehler_ausgeben(stderr, f),
    }
}

/// The search of the program: the question to the holder goes over his fixed endpoint ([`halter::fester_pfad`]), one
/// connection per target, never an option of the caller. S7: this is the only connection the search opens, and it is
/// a local one to a role program of the same machine.
pub fn lauf(
    args: &[OsString],
    eigene_uid: u32,
    puffer: usize,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> u8 {
    let mut frage = |ziel: &str, pfade: &[String]| halter_suche(ziel, pfade, eigene_uid);
    lauf_mit_halter(args, eigene_uid, puffer, &mut frage, stdout, stderr)
}

/// One `modell.suche` at the holder for `ziel`; `None` on every failure and on a target without a secret.
#[cfg(target_os = "linux")]
fn halter_suche(ziel: &str, pfade: &[String], eigene_uid: u32) -> Option<halter::Suche> {
    let mut k = halter::Klient::verbinde(&halter::fester_pfad(eigene_uid), eigene_uid).ok()?;
    let id = k.naechste_id();
    k.frist(Some(halter::FRIST_KOPF)).ok()?;
    k.senden(&halter::modell_suche(id, ziel, pfade).ok()?)
        .ok()?;
    match k.naechste_zeile().ok()? {
        halter::Zeile::Antwort {
            code: None,
            suche: Some(s),
            ..
        } => Some(s),
        _ => None,
    }
}

/// Mode 2 and with it the holder path are built under Linux only; on another system there is no bound target.
#[cfg(not(target_os = "linux"))]
fn halter_suche(_ziel: &str, _pfade: &[String], _eigene_uid: u32) -> Option<halter::Suche> {
    None
}

#[cfg(test)]
mod tests {
    use super::{
        Aufruf, Ergebnis, PFAD_MAX, Quelle, Suchfehler, TIEFE_MAX, Zaehler, aufruf_lesen,
        durchsuchen, finde, lauf,
    };
    use crate::schluessel::Schluessel;
    use std::ffi::OsString;
    use std::os::unix::fs::PermissionsExt;
    use std::path::{Path, PathBuf};

    fn args(a: &[&str]) -> Vec<OsString> {
        a.iter().map(OsString::from).collect()
    }

    fn zaehle(nadel: &[u8], teile: &[&[u8]]) -> u64 {
        let mut z = Zaehler::neu(Schluessel::fuer_test(nadel));
        z.datei_beginnt();
        teile.iter().map(|t| z.block(t)).sum()
    }

    /// A folder below the temp dir, removed when dropped (also folders made unreadable).
    struct Ordner(PathBuf);

    impl Ordner {
        fn neu(name: &str) -> Self {
            let p = std::env::temp_dir().join(format!("mw-unit-su-{}-{name}", std::process::id()));
            let _ = std::fs::remove_dir_all(&p);
            std::fs::create_dir_all(&p).expect("create");
            Self(p)
        }
        fn pfad(&self, name: &str) -> PathBuf {
            self.0.join(name)
        }
        fn datei(&self, name: &str, inhalt: &[u8]) -> PathBuf {
            let p = self.pfad(name);
            if let Some(eltern) = p.parent() {
                std::fs::create_dir_all(eltern).expect("parents");
            }
            std::fs::write(&p, inhalt).expect("write");
            p
        }
    }

    impl Drop for Ordner {
        fn drop(&mut self) {
            fn frei(p: &Path) {
                let _ = std::fs::set_permissions(p, std::fs::Permissions::from_mode(0o700));
                if let Ok(rd) = std::fs::read_dir(p) {
                    for e in rd.flatten() {
                        if e.file_type().is_ok_and(|t| t.is_dir()) {
                            frei(&e.path());
                        }
                    }
                }
            }
            frei(&self.0);
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn suche(nadeln: &[&[u8]], eingaenge: &[&Path], puffer: usize) -> Ergebnis {
        let k = nadeln.iter().map(|n| Schluessel::fuer_test(n)).collect();
        let e: Vec<PathBuf> = eingaenge.iter().map(|p| p.to_path_buf()).collect();
        durchsuchen(k, &e, puffer)
    }

    #[test]
    fn a_key_is_counted_across_every_border_of_two_reads() {
        let nadel = b"KEYVALUE";
        for lage in 0..40_usize {
            let mut inhalt = vec![b'x'; lage];
            inhalt.extend_from_slice(nadel);
            inhalt.extend_from_slice(&[b'y'; 10]);
            for gross in 1..=inhalt.len() {
                let teile: Vec<&[u8]> = inhalt.chunks(gross).collect();
                assert_eq!(zaehle(nadel, &teile), 1, "lage {lage} block {gross}");
            }
        }
    }

    #[test]
    fn occurrences_do_not_overlap_and_a_new_file_starts_empty() {
        assert_eq!(zaehle(b"aa", &[b"aaa"]), 1);
        assert_eq!(zaehle(b"aa", &[b"aaaa"]), 2);
        assert_eq!(zaehle(b"aa", &[b"a", b"a", b"a"]), 1);
        assert_eq!(zaehle(b"aa", &[b"a", b"aa", b"a"]), 2);
        assert_eq!(zaehle(b"aba", &[b"ababa"]), 1);
        assert_eq!(zaehle(b"aba", &[b"ab", b"aba", b"ba"]), 2);
        assert_eq!(zaehle(b"k", &[b"kkk"]), 3);
        assert_eq!(zaehle(b"abc", &[b"ab", b"x", b"c"]), 0);
        let mut z = Zaehler::neu(Schluessel::fuer_test(b"abc"));
        z.datei_beginnt();
        assert_eq!(z.block(b"ab"), 0);
        z.datei_beginnt();
        assert_eq!(z.block(b"c"), 0);
        assert!(z.rest.len() < 3);
    }

    #[test]
    fn a_rest_never_grows_beyond_the_key_minus_one() {
        let mut z = Zaehler::neu(Schluessel::fuer_test(b"abcd"));
        z.datei_beginnt();
        for _ in 0..50 {
            z.block(b"zzzzzzzzabcabcabcabc");
            assert!(z.rest.len() <= 3);
        }
        assert_eq!(finde(b"xxabxx", b"ab"), Some(2));
        assert_eq!(finde(b"xxaxbx", b"ab"), None);
    }

    #[test]
    fn the_call_is_read_after_s1() {
        let gut = aufruf_lesen(&args(&[
            "--schluessel-datei",
            "/a/k",
            "--in",
            "/a/b",
            "--in",
            "/c",
        ]))
        .expect("good");
        assert_eq!(
            gut,
            Aufruf {
                quelle: Quelle::Datei("/a/k".into()),
                eingaenge: vec!["/a/b".into(), "/c".into()],
            }
        );
        let liste = aufruf_lesen(&args(&["--in", "/x", "--zielliste", "/a/z"])).expect("list");
        assert_eq!(liste.quelle, Quelle::Liste("/a/z".into()));
        let viele: Vec<&str> = std::iter::repeat_n(["--in", "/x"], 16)
            .flatten()
            .chain(["--zielliste", "/z"])
            .collect();
        assert_eq!(
            aufruf_lesen(&args(&viele)).map(|a| a.eingaenge.len()),
            Some(16)
        );
        let zu_viele: Vec<&str> = std::iter::repeat_n(["--in", "/x"], 17)
            .flatten()
            .chain(["--zielliste", "/z"])
            .collect();
        assert_eq!(aufruf_lesen(&args(&zu_viele)), None);
        let lang = format!("/{}", "a".repeat(PFAD_MAX - 1));
        assert!(aufruf_lesen(&args(&["--zielliste", "/z", "--in", &lang])).is_some());
        let zu_lang = format!("/{}", "a".repeat(PFAD_MAX));
        assert_eq!(
            aufruf_lesen(&args(&["--zielliste", "/z", "--in", &zu_lang])),
            None
        );
    }

    #[test]
    fn a_wrong_call_is_refused() {
        let falsch: [&[&str]; 14] = [
            &[],
            &["--in", "/x"],
            &["--zielliste", "/z"],
            &["--zielliste", "/z", "--in"],
            &["--zielliste", "/z", "--zielliste", "/z", "--in", "/x"],
            &[
                "--schluessel-datei",
                "/k",
                "--schluessel-datei",
                "/k",
                "--in",
                "/x",
            ],
            &[
                "--schluessel-datei",
                "/k",
                "--zielliste",
                "/z",
                "--in",
                "/x",
            ],
            &["--zielliste", "z", "--in", "/x"],
            &["--zielliste", "/z", "--in", "x"],
            &["--zielliste", "/z", "--in", "/x/../y"],
            &["--zielliste", "/z", "--in=/x"],
            &["--zielliste", "/z", "--wo", "/x"],
            &["--zielliste", "/z", "/x"],
            &["--zielliste", "/z", "--in", "/x\u{1}"],
        ];
        for a in falsch {
            assert_eq!(aufruf_lesen(&args(a)), None, "{a:?}");
        }
        let mut nicht_utf8 = args(&["--zielliste", "/z", "--in"]);
        nicht_utf8.push(nicht_utf8_pfad());
        assert_eq!(aufruf_lesen(&nicht_utf8), None);
    }

    fn nicht_utf8_pfad() -> OsString {
        use std::os::unix::ffi::OsStringExt;
        OsString::from_vec(vec![b'/', 0xff, 0xfe])
    }

    #[test]
    fn the_failures_have_their_fixed_lines() {
        let f = [
            (
                Suchfehler::Aufruf,
                "mw.suche_aufruf: Suchaufruf ungueltig\n",
                64,
            ),
            (
                Suchfehler::ZieleUngueltig,
                "mw.ziele_ungueltig: Zielliste ungueltig\n",
                78,
            ),
            (
                Suchfehler::SchluesselFehlt,
                "mw.schluessel_fehlt: Schluessel nicht lesbar\n",
                78,
            ),
            (Suchfehler::Intern, "mw.intern: interner Fehler\n", 70),
        ];
        for (fehler, zeile, exit) in f {
            assert_eq!(fehler.zeile(), zeile);
            assert_eq!(fehler.exit(), exit);
        }
    }

    #[test]
    fn the_exit_follows_the_counts() {
        let e = |t, n| Ergebnis {
            dateien: 1,
            nicht_lesbar: n,
            treffer: t,
            uebersprungen: 5,
        };
        assert_eq!(e(0, 0).exit(), 0);
        assert_eq!(e(1, 0).exit(), 1);
        assert_eq!(e(1, 1).exit(), 1);
        assert_eq!(e(0, 1).exit(), 3);
        assert_eq!(
            e(2, 3).zeile(),
            "{\"dateien\":1,\"nicht_lesbar\":3,\"treffer\":2,\"uebersprungen\":5}\n"
        );
    }

    #[test]
    fn the_walk_counts_files_links_and_the_unreadable() {
        let o = Ordner::neu("walk");
        o.datei("a/eins.txt", b"xx GEHEIM yy GEHEIM");
        o.datei("a/b/zwei.txt", b"GEHEIM");
        o.datei("a/leer.txt", b"");
        std::os::unix::fs::symlink(o.pfad("a/eins.txt"), o.pfad("a/verweis")).expect("link");
        std::os::unix::fs::symlink(o.pfad("a/b"), o.pfad("a/verweis-ordner")).expect("link");
        for puffer in [1, 5, 6, 7, 4096] {
            let e = suche(&[b"GEHEIM"], &[&o.pfad("a")], puffer);
            assert_eq!(
                e,
                Ergebnis {
                    dateien: 3,
                    nicht_lesbar: 0,
                    treffer: 3,
                    uebersprungen: 2
                },
                "{puffer}"
            );
        }
    }

    #[test]
    fn two_keys_count_together_and_a_key_inside_another_counts_in_both() {
        let o = Ordner::neu("zwei");
        o.datei("d/x.txt", b"ABCDEF ABC");
        let e = suche(&[b"ABCDEF", b"ABC", b"nichts"], &[&o.pfad("d")], 3);
        assert_eq!((e.treffer, e.dateien), (3, 1));
    }

    #[test]
    fn a_root_that_is_not_a_file_or_folder_is_not_readable() {
        let o = Ordner::neu("wurzel");
        let ziel = o.datei("echt/f.txt", b"GEHEIM");
        std::os::unix::fs::symlink(&ziel, o.pfad("verweis")).expect("link");
        std::os::unix::fs::symlink(o.pfad("echt"), o.pfad("verweis-ordner")).expect("link");
        let e = suche(&[b"GEHEIM"], &[&o.pfad("verweis")], 8);
        assert_eq!(
            (e.nicht_lesbar, e.treffer, e.dateien, e.uebersprungen),
            (1, 0, 0, 0)
        );
        let e = suche(&[b"GEHEIM"], &[&o.pfad("verweis-ordner")], 8);
        assert_eq!(
            (e.nicht_lesbar, e.treffer, e.dateien, e.uebersprungen),
            (1, 0, 0, 0)
        );
        let e = suche(&[b"GEHEIM"], &[&o.pfad("fehlt")], 8);
        assert_eq!(
            (e.nicht_lesbar, e.treffer, e.dateien, e.uebersprungen),
            (1, 0, 0, 0)
        );
        let e = suche(&[b"GEHEIM"], &[&ziel], 8);
        assert_eq!(
            (e.nicht_lesbar, e.treffer, e.dateien, e.uebersprungen),
            (0, 1, 1, 0)
        );
        // The same file by three roots is read once.
        let e = suche(&[b"GEHEIM"], &[&ziel, &o.pfad("echt"), &o.0], 8);
        assert_eq!((e.nicht_lesbar, e.treffer, e.dateien), (0, 1, 1));
    }

    #[test]
    fn a_folder_deeper_than_the_limit_is_not_entered() {
        let o = Ordner::neu("tief");
        let mut tief = o.pfad("w");
        for _ in 0..TIEFE_MAX {
            tief.push("d");
        }
        std::fs::create_dir_all(&tief).expect("deep");
        std::fs::write(tief.join("f.txt"), b"GEHEIM").expect("write");
        let e = suche(&[b"GEHEIM"], &[&o.pfad("w")], 64);
        assert_eq!((e.nicht_lesbar, e.treffer, e.dateien), (0, 1, 1));
        let tiefer = tief.join("d");
        std::fs::create_dir_all(&tiefer).expect("deeper");
        std::fs::write(tiefer.join("g.txt"), b"GEHEIM").expect("write");
        let e = suche(&[b"GEHEIM"], &[&o.pfad("w")], 64);
        assert_eq!((e.nicht_lesbar, e.treffer, e.dateien), (1, 1, 1));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn a_read_error_counts_as_not_readable() {
        // Reading /proc/self/mem at offset 0 fails with EIO although the file is a regular file.
        let e = suche(&[b"GEHEIM"], &[Path::new("/proc/self/mem")], 64);
        assert_eq!((e.nicht_lesbar, e.dateien, e.treffer), (1, 0, 0));
    }

    #[test]
    fn the_program_writes_one_line_or_a_fixed_line() {
        let o = Ordner::neu("lauf");
        let k = o.datei("schluessel", b"GEHEIM\n");
        std::fs::set_permissions(&k, std::fs::Permissions::from_mode(0o600)).expect("chmod");
        o.datei("w/f.txt", b"..GEHEIM..");
        let uid = crate::os::eigene_uid();
        let a = args(&[
            "--schluessel-datei",
            k.to_str().expect("utf8"),
            "--in",
            o.pfad("w").to_str().expect("utf8"),
        ]);
        let (mut aus, mut fehler) = (Vec::new(), Vec::new());
        assert_eq!(lauf(&a, uid, 4, &mut aus, &mut fehler), 1);
        assert_eq!(
            String::from_utf8(aus).expect("ascii"),
            "{\"dateien\":1,\"nicht_lesbar\":0,\"treffer\":1,\"uebersprungen\":0}\n"
        );
        assert_eq!(fehler, Vec::<u8>::new());
        let (mut aus, mut fehler) = (Vec::new(), Vec::new());
        assert_eq!(
            lauf(&args(&["--in", "/x"]), uid, 4, &mut aus, &mut fehler),
            64
        );
        assert_eq!(aus, Vec::<u8>::new());
        assert_eq!(fehler, b"mw.suche_aufruf: Suchaufruf ungueltig\n");
        // A key that is not own, not 0600 or has a wrong form stops the call before anything is searched.
        std::fs::set_permissions(&k, std::fs::Permissions::from_mode(0o644)).expect("chmod");
        let (mut aus, mut fehler) = (Vec::new(), Vec::new());
        assert_eq!(lauf(&a, uid, 4, &mut aus, &mut fehler), 78);
        assert_eq!(aus, Vec::<u8>::new());
        assert_eq!(fehler, b"mw.schluessel_fehlt: Schluessel nicht lesbar\n");
    }

    #[test]
    fn a_failing_stdout_is_an_internal_error() {
        struct Kaputt;
        impl std::io::Write for Kaputt {
            fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
                Err(std::io::Error::other("kaputt"))
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        let o = Ordner::neu("kaputt");
        let k = o.datei("schluessel", b"GEHEIM");
        std::fs::set_permissions(&k, std::fs::Permissions::from_mode(0o600)).expect("chmod");
        let a = args(&[
            "--schluessel-datei",
            k.to_str().expect("utf8"),
            "--in",
            o.0.to_str().expect("utf8"),
        ]);
        let mut fehler = Vec::new();
        let exit = lauf(&a, crate::os::eigene_uid(), 8, &mut Kaputt, &mut fehler);
        assert_eq!(exit, 70);
        assert_eq!(fehler, b"mw.intern: interner Fehler\n");
    }
}
