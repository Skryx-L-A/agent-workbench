//! The file system part of reading and searching the working folder under Unix (contract `dienst`, "Nachtrag 26.11.2026: Ordner lesen
//! und durchsuchen (L9)"). The checks of the text and the class are in `arbeitsordner.rs`.
//!
//! SECURITY: only descriptors, never a path put together from text. The working folder is opened once with
//! `O_DIRECTORY | O_NOFOLLOW | O_CLOEXEC`; every member below it is opened relative to the folder opened before, with the same flags,
//! so no member is ever a link and a folder swapped for a link after it was opened is not followed (the descriptor still names the
//! folder that was opened). A file of the search is opened relative to its folder with `O_RDONLY | O_NOFOLLOW | O_NONBLOCK |
//! O_CLOEXEC` (a FIFO does not block the open) and read only when `fstat` says it is a regular file with one link (a hard link to a file
//! outside has the same inode, so a second link is refused) and with the device and inode that `statat` saw. A folder of the search is
//! entered only when `fstat` of the opened folder names the device and inode that `statat` saw. Links, FIFOs, sockets and devices are
//! never opened by the search.

use std::ffi::{CStr, CString};
use std::io::Read;
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
use std::path::Path;

use rustix::fs::{AtFlags, CWD, Dir, FileType, Mode, OFlags, Stat, fstat, openat, statat};

use crate::arbeitsordner::{
    Art, Eintrag, Grenzen, Inhalt, NICHT_LESBAR, PFAD_AUSSERHALB, PFAD_UNGUELTIG, Suche, Treffer,
    auszug, treffer_bytes,
};
use crate::loeschen::ORDNER_OFFEN;

/// The flags to open a file of the search: read only, no link, no wait for a writer of a FIFO.
const DATEI_OFFEN: OFlags = OFlags::RDONLY
    .union(OFlags::NOFOLLOW)
    .union(OFlags::NONBLOCK)
    .union(OFlags::CLOEXEC);

/// Lists the folder `glieder` below `wurzel`.
pub(crate) fn ordner_lesen(
    wurzel: &Path,
    glieder: &[&str],
    g: &Grenzen,
) -> Result<Inhalt, &'static str> {
    ordner_lesen_mit(wurzel, glieder, g, &mut |_| {})
}

/// [`ordner_lesen`] with a step between the members (a case swaps a folder there).
pub(crate) fn ordner_lesen_mit(
    wurzel: &Path,
    glieder: &[&str],
    g: &Grenzen,
    zwischen: &mut dyn FnMut(usize),
) -> Result<Inhalt, &'static str> {
    let dir = absteigen(wurzel, glieder, zwischen)?;
    let (namen, voll) = namen_lesen(dir.as_fd(), g.eintraege).ok_or(NICHT_LESBAR)?;
    let mut inhalt = Inhalt {
        eintraege: Vec::new(),
        abgeschnitten: voll,
    };
    for name in namen {
        // a name that is no UTF-8 cannot be named in a later `pfad`: it is not shown, and the answer says that something is missing
        let Ok(text) = name.to_str() else {
            inhalt.abgeschnitten = true;
            continue;
        };
        let Ok(st) = statat(dir.as_fd(), &name, AtFlags::SYMLINK_NOFOLLOW) else {
            inhalt.abgeschnitten = true;
            continue;
        };
        let art = art_von(&st);
        inhalt.eintraege.push(Eintrag {
            name: text.to_owned(),
            art,
            // a size below 0 does not exist; it would be `null`, never a made-up number
            groesse: (art == Art::Datei)
                .then(|| u64::try_from(st.st_size).ok())
                .flatten(),
        });
    }
    Ok(inhalt)
}

/// The root, opened once, without following a link at its last member.
fn wurzel_oeffnen(wurzel: &Path) -> Result<OwnedFd, &'static str> {
    openat(CWD, wurzel, ORDNER_OFFEN, Mode::empty()).map_err(|_| NICHT_LESBAR)
}

/// Opens the members one after the other, each relative to the folder opened before and never through a link.
fn absteigen(
    wurzel: &Path,
    glieder: &[&str],
    zwischen: &mut dyn FnMut(usize),
) -> Result<OwnedFd, &'static str> {
    let mut dir = wurzel_oeffnen(wurzel)?;
    for (i, glied) in glieder.iter().enumerate() {
        zwischen(i);
        // the members come from the text check; a member that could name more than one step is refused here once more
        if !crate::loeschen::name_gut(glied) {
            return Err(PFAD_UNGUELTIG);
        }
        let Ok(name) = CString::new(*glied) else {
            return Err(PFAD_UNGUELTIG);
        };
        dir = match openat(dir.as_fd(), &name, ORDNER_OFFEN, Mode::empty()) {
            Ok(d) => d,
            Err(_) => return Err(warum(dir.as_fd(), &name)),
        };
    }
    Ok(dir)
}

/// Why a member did not open: a link is [`PFAD_AUSSERHALB`], everything else [`NICHT_LESBAR`]. Only a reason; nothing is opened.
fn warum(dir: BorrowedFd<'_>, name: &CStr) -> &'static str {
    match statat(dir, name, AtFlags::SYMLINK_NOFOLLOW) {
        Ok(st) if FileType::from_raw_mode(st.st_mode) == FileType::Symlink => PFAD_AUSSERHALB,
        _ => NICHT_LESBAR,
    }
}

fn art_von(st: &Stat) -> Art {
    match FileType::from_raw_mode(st.st_mode) {
        FileType::RegularFile => Art::Datei,
        FileType::Directory => Art::Ordner,
        _ => Art::Anderes,
    }
}

/// At most `max` names of the folder in byte order, without `.` and `..`; `true` when there are more or the listing broke off.
/// `None`: the folder cannot be listed at all.
fn namen_lesen(dir: BorrowedFd<'_>, max: usize) -> Option<(Vec<CString>, bool)> {
    let eintraege = Dir::read_from(dir).ok()?;
    let mut namen = Vec::new();
    let mut voll = false;
    for e in eintraege {
        let Ok(e) = e else {
            voll = true;
            break;
        };
        let name = e.file_name();
        if matches!(name.to_bytes(), b"." | b"..") {
            continue;
        }
        if namen.len() >= max {
            voll = true;
            break;
        }
        namen.push(name.to_owned());
    }
    namen.sort();
    Some((namen, voll))
}

/// Searches `muster` in every regular file below `wurzel`, line by line, in byte order of the names, depth first.
pub(crate) fn inhalt_suchen(
    wurzel: &Path,
    muster: &str,
    g: &Grenzen,
    uhr: &mut dyn FnMut() -> u64,
) -> Result<Suche, &'static str> {
    inhalt_suchen_mit(wurzel, muster, g, uhr, &mut |_| {})
}

/// [`inhalt_suchen`] with a step between `statat` of an entry and its opening (a case swaps the entry there).
pub(crate) fn inhalt_suchen_mit(
    wurzel: &Path,
    muster: &str,
    g: &Grenzen,
    uhr: &mut dyn FnMut() -> u64,
    zwischen: &mut dyn FnMut(&str),
) -> Result<Suche, &'static str> {
    let dir = wurzel_oeffnen(wurzel)?;
    let (namen, voll) = namen_lesen(dir.as_fd(), g.eintraege).ok_or(NICHT_LESBAR)?;
    let beginn = uhr();
    let mut lauf = Lauf {
        g,
        muster: muster.as_bytes(),
        uhr,
        zwischen,
        beginn,
        dateien: 0,
        bytes: 0,
        antwort: 0,
        ende: false,
        suche: Suche {
            treffer: Vec::new(),
            abgeschnitten: voll,
        },
    };
    lauf.namen(dir.as_fd(), &namen, "", 0);
    Ok(lauf.suche)
}

/// The state of one search.
struct Lauf<'a> {
    g: &'a Grenzen,
    muster: &'a [u8],
    uhr: &'a mut dyn FnMut() -> u64,
    /// The step between `statat` and the opening of an entry, with its path.
    zwischen: &'a mut dyn FnMut(&str),
    beginn: u64,
    dateien: usize,
    bytes: usize,
    /// The bytes the hits take in the answer so far ([`treffer_bytes`]).
    antwort: usize,
    /// A limit of the whole search was reached: nothing more is read.
    ende: bool,
    suche: Suche,
}

impl Lauf<'_> {
    /// Marks the result as cut off and, with `ganz`, ends the search.
    fn schnitt(&mut self, ganz: bool) {
        self.suche.abgeschnitten = true;
        self.ende |= ganz;
    }

    /// The entries `namen` of the folder `dir` (at depth `tiefe`, path `vorn`).
    fn namen(&mut self, dir: BorrowedFd<'_>, namen: &[CString], vorn: &str, tiefe: usize) {
        for name in namen {
            if self.ende {
                return;
            }
            let Ok(text) = name.to_str() else {
                self.schnitt(false);
                continue;
            };
            let pfad = if vorn.is_empty() {
                text.to_owned()
            } else {
                format!("{vorn}/{text}")
            };
            let Ok(st) = statat(dir, name, AtFlags::SYMLINK_NOFOLLOW) else {
                self.schnitt(false);
                continue;
            };
            (self.zwischen)(&pfad);
            match art_von(&st) {
                Art::Ordner => self.ordner(dir, name, &st, &pfad, tiefe + 1),
                Art::Datei => self.datei(dir, name, &st, &pfad),
                // links, FIFOs, sockets and devices are no content of the folder: never opened, never followed
                Art::Anderes => {}
            }
        }
    }

    fn ordner(&mut self, dir: BorrowedFd<'_>, name: &CStr, st: &Stat, pfad: &str, tiefe: usize) {
        if tiefe > self.g.tiefe {
            return self.schnitt(false);
        }
        let Ok(sub) = openat(dir, name, ORDNER_OFFEN, Mode::empty()) else {
            return self.schnitt(false);
        };
        if !gleich(&sub, st) {
            return self.schnitt(false);
        }
        let Some((namen, voll)) = namen_lesen(sub.as_fd(), self.g.eintraege) else {
            return self.schnitt(false);
        };
        if voll {
            self.schnitt(false);
        }
        self.namen(sub.as_fd(), &namen, pfad, tiefe);
    }

    fn datei(&mut self, dir: BorrowedFd<'_>, name: &CStr, st: &Stat, pfad: &str) {
        if (self.uhr)().saturating_sub(self.beginn) >= self.g.laufzeit_ms
            || self.dateien >= self.g.dateien
            || self.bytes >= self.g.gesamt_bytes
        {
            return self.schnitt(true);
        }
        self.dateien += 1;
        let Ok(fd) = openat(dir, name, DATEI_OFFEN, Mode::empty()) else {
            return self.schnitt(false);
        };
        let Ok(st2) = fstat(&fd) else {
            return self.schnitt(false);
        };
        if FileType::from_raw_mode(st2.st_mode) != FileType::RegularFile
            || st2.st_nlink != 1
            || !gleich_stat(&st2, st)
        {
            return self.schnitt(false);
        }
        let max = self.g.datei_bytes.min(self.g.gesamt_bytes - self.bytes);
        let (daten, ganz) = lesen(fd, max);
        self.bytes += daten.len();
        if !ganz {
            self.schnitt(false);
        }
        self.zeilen(&daten, pfad);
    }

    /// The hits of one file.
    fn zeilen(&mut self, daten: &[u8], pfad: &str) {
        let m = self.muster;
        for (i, zeile) in daten.split(|&b| b == b'\n').enumerate() {
            if !zeile.windows(m.len()).any(|w| w == m) {
                continue;
            }
            let auszug = auszug(zeile, self.g.auszug_bytes);
            let groesse = treffer_bytes(pfad, &auszug);
            if self.suche.treffer.len() >= self.g.treffer
                || self.antwort + groesse > self.g.antwort_bytes
            {
                return self.schnitt(true);
            }
            self.antwort += groesse;
            self.suche.treffer.push(Treffer {
                pfad: pfad.to_owned(),
                zeile: u64::try_from(i).map_or(u64::MAX, |z| z + 1),
                auszug,
            });
        }
    }
}

/// Whether the opened folder is the entry `statat` saw.
fn gleich(fd: &OwnedFd, st: &Stat) -> bool {
    fstat(fd).is_ok_and(|s| gleich_stat(&s, st))
}

fn gleich_stat(a: &Stat, b: &Stat) -> bool {
    (a.st_dev, a.st_ino) == (b.st_dev, b.st_ino)
}

/// At most `max` bytes of the file; `true` when that was the whole file (no error, no byte more).
fn lesen(fd: OwnedFd, max: usize) -> (Vec<u8>, bool) {
    let mut daten = Vec::new();
    let grenze = u64::try_from(max).map_or(u64::MAX, |m| m.saturating_add(1));
    let ok = std::fs::File::from(fd)
        .take(grenze)
        .read_to_end(&mut daten)
        .is_ok();
    let ganz = ok && daten.len() <= max;
    daten.truncate(max);
    (daten, ganz)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::arbeitsordner::{GRENZEN, glieder};
    use std::fs;
    use std::os::unix::fs::{PermissionsExt, symlink};
    use std::path::PathBuf;

    /// A folder of the test below `TMPDIR`, removed at the end by the test itself with `std` (test code, not the product).
    struct Probe(PathBuf);

    impl Probe {
        fn neu(name: &str) -> Self {
            let p = std::env::temp_dir().join(format!("ao-{}-{name}", std::process::id()));
            freigeben(&p);
            let _ = fs::remove_dir_all(&p);
            fs::create_dir_all(p.join("w")).expect("dir");
            fs::create_dir_all(p.join("draussen")).expect("dir");
            fs::write(p.join("draussen/geheim.txt"), "GEHEIM zeile\n").expect("file");
            Self(p)
        }

        fn w(&self) -> PathBuf {
            self.0.join("w")
        }

        fn draussen(&self) -> PathBuf {
            self.0.join("draussen")
        }
    }

    impl Drop for Probe {
        fn drop(&mut self) {
            freigeben(&self.0);
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    /// Gives every folder below `p` back its rights, so that a test which stopped before its own reset leaves nothing behind.
    /// Links are not followed (`symlink_metadata`).
    fn freigeben(p: &Path) {
        let Ok(m) = fs::symlink_metadata(p) else {
            return;
        };
        if !m.is_dir() {
            return;
        }
        let _ = fs::set_permissions(p, fs::Permissions::from_mode(0o700));
        if let Ok(liste) = fs::read_dir(p) {
            for e in liste.flatten() {
                freigeben(&e.path());
            }
        }
    }

    fn lesen_text(w: &Path, pfad: &str) -> Result<Inhalt, &'static str> {
        let g = glieder(pfad, &GRENZEN)?;
        ordner_lesen(w, &g, &GRENZEN)
    }

    fn namen(i: &Inhalt) -> Vec<(&str, Art, Option<u64>)> {
        i.eintraege
            .iter()
            .map(|e| (e.name.as_str(), e.art, e.groesse))
            .collect()
    }

    fn suchen(w: &Path, muster: &str, g: &Grenzen) -> Result<Suche, &'static str> {
        inhalt_suchen(w, muster, g, &mut || 0)
    }

    fn treffer(s: &Suche) -> Vec<(&str, u64, &str)> {
        s.treffer
            .iter()
            .map(|t| (t.pfad.as_str(), t.zeile, t.auszug.as_str()))
            .collect()
    }

    #[test]
    fn gutes_lesen() {
        let p = Probe::neu("gut");
        let w = p.w();
        fs::create_dir_all(w.join("b/c")).expect("dir");
        fs::write(w.join("a.txt"), "abc").expect("file");
        fs::write(w.join("b/d.txt"), "12345").expect("file");
        symlink(p.draussen(), w.join("z-link")).expect("link");
        let i = lesen_text(&w, "").expect("root");
        assert_eq!(
            namen(&i),
            vec![
                ("a.txt", Art::Datei, Some(3)),
                ("b", Art::Ordner, None),
                ("z-link", Art::Anderes, None)
            ]
        );
        assert!(!i.abgeschnitten);
        let i = lesen_text(&w, "b").expect("b");
        assert_eq!(
            namen(&i),
            vec![("c", Art::Ordner, None), ("d.txt", Art::Datei, Some(5))]
        );
        let i = lesen_text(&w, "b/c").expect("b/c");
        assert_eq!(i, Inhalt::default());
    }

    #[test]
    fn ein_link_auf_einen_ordner_draussen_ist_ausserhalb() {
        let p = Probe::neu("link-ordner");
        let w = p.w();
        symlink(p.draussen(), w.join("l")).expect("link");
        assert_eq!(lesen_text(&w, "l"), Err(PFAD_AUSSERHALB));
        // the search does not follow it either
        let s = suchen(&w, "GEHEIM", &GRENZEN).expect("search");
        assert_eq!(s, Suche::default());
    }

    #[test]
    fn ein_link_auf_eine_datei_draussen_wird_nie_gelesen() {
        let p = Probe::neu("link-datei");
        let w = p.w();
        symlink(p.draussen().join("geheim.txt"), w.join("l.txt")).expect("link");
        assert_eq!(lesen_text(&w, "l.txt"), Err(PFAD_AUSSERHALB));
        let s = suchen(&w, "GEHEIM", &GRENZEN).expect("search");
        assert_eq!(s, Suche::default());
        let i = lesen_text(&w, "").expect("root");
        assert_eq!(namen(&i), vec![("l.txt", Art::Anderes, None)]);
    }

    #[test]
    fn ein_link_in_der_mitte_ist_ausserhalb() {
        let p = Probe::neu("link-mitte");
        let w = p.w();
        fs::create_dir_all(w.join("a")).expect("dir");
        fs::create_dir_all(p.draussen().join("tief")).expect("dir");
        symlink(p.draussen(), w.join("a/m")).expect("link");
        assert_eq!(lesen_text(&w, "a/m/tief"), Err(PFAD_AUSSERHALB));
        assert_eq!(lesen_text(&w, "a/m"), Err(PFAD_AUSSERHALB));
    }

    #[test]
    fn die_wurzel_als_link_wird_nicht_geoeffnet() {
        let p = Probe::neu("wurzel-link");
        let l = p.0.join("wl");
        symlink(p.draussen(), &l).expect("link");
        assert_eq!(ordner_lesen(&l, &[], &GRENZEN), Err(NICHT_LESBAR));
        assert_eq!(suchen(&l, "GEHEIM", &GRENZEN), Err(NICHT_LESBAR));
    }

    #[test]
    fn fehlendes_datei_und_nul_sind_nicht_lesbar() {
        let p = Probe::neu("fehlt");
        let w = p.w();
        fs::write(w.join("f"), "x").expect("file");
        assert_eq!(lesen_text(&w, "nicht"), Err(NICHT_LESBAR));
        assert_eq!(lesen_text(&w, "f"), Err(NICHT_LESBAR));
        assert_eq!(lesen_text(&w, "f/x"), Err(NICHT_LESBAR));
        assert_eq!(ordner_lesen(&w, &["a\0b"], &GRENZEN), Err(PFAD_UNGUELTIG));
        assert_eq!(
            ordner_lesen(&p.0.join("nirgends"), &[], &GRENZEN),
            Err(NICHT_LESBAR)
        );
        assert_eq!(
            suchen(&p.0.join("nirgends"), "x", &GRENZEN),
            Err(NICHT_LESBAR)
        );
    }

    /// The file system part refuses a member that is not one plain name, also when the text check is passed by.
    #[test]
    fn der_dateiteil_nimmt_nur_einfache_glieder() {
        let p = Probe::neu("glieder");
        let w = p.w();
        fs::create_dir_all(w.join("a/b")).expect("dir");
        for g in ["..", ".", "", "a/b", "/", "a\0b"] {
            assert_eq!(
                ordner_lesen(&w, &[g], &GRENZEN),
                Err(PFAD_UNGUELTIG),
                "{g:?}"
            );
        }
        assert_eq!(
            ordner_lesen(&w, &["a", ".."], &GRENZEN),
            Err(PFAD_UNGUELTIG)
        );
        assert!(ordner_lesen(&w, &["a", "b"], &GRENZEN).is_ok());
    }

    #[test]
    fn rennen_ein_ordner_wird_zwischen_zwei_schritten_zum_link() {
        let p = Probe::neu("rennen");
        let w = p.w();
        fs::create_dir_all(w.join("a/b")).expect("dir");
        fs::write(w.join("a/b/innen.txt"), "x").expect("file");
        fs::create_dir_all(p.draussen().join("b")).expect("dir");
        fs::write(p.draussen().join("b/draussen.txt"), "x").expect("file");
        let a = w.join("a");
        let alt = p.0.join("a-alt");
        let draussen = p.draussen();
        // after `a` is opened (before member 1, `b`), `a` is moved away and a link to the outside takes its place
        let mut tausch = |i: usize| {
            if i == 1 {
                fs::rename(&a, &alt).expect("move");
                symlink(&draussen, &a).expect("link");
            }
        };
        let i = ordner_lesen_mit(&w, &["a", "b"], &GRENZEN, &mut tausch).expect("read");
        assert_eq!(namen(&i), vec![("innen.txt", Art::Datei, Some(1))]);
        // a new request sees the link
        assert_eq!(lesen_text(&w, "a/b"), Err(PFAD_AUSSERHALB));
    }

    #[test]
    fn gute_suche() {
        let p = Probe::neu("suche");
        let w = p.w();
        fs::create_dir_all(w.join("b")).expect("dir");
        fs::write(w.join("a.txt"), "eins\nzwei ei\r\ndrei\n").expect("file");
        fs::write(w.join("b/c.txt"), "keins").expect("file");
        fs::write(w.join("b/d.txt"), "nichts").expect("file");
        let s = suchen(&w, "ei", &GRENZEN).expect("search");
        assert_eq!(
            treffer(&s),
            vec![
                ("a.txt", 1, "eins"),
                ("a.txt", 2, "zwei ei"),
                ("a.txt", 3, "drei"),
                ("b/c.txt", 1, "keins")
            ]
        );
        assert!(!s.abgeschnitten);
        let s = suchen(&w, "zwei ei", &GRENZEN).expect("search");
        assert_eq!(treffer(&s), vec![("a.txt", 2, "zwei ei")]);
        let s = suchen(&w, "nirgends", &GRENZEN).expect("search");
        assert_eq!(s, Suche::default());
    }

    #[test]
    fn die_suche_oeffnet_keinen_hardlink_und_keine_fifo() {
        let p = Probe::neu("hardlink");
        let w = p.w();
        fs::hard_link(p.draussen().join("geheim.txt"), w.join("h.txt")).expect("hard link");
        let s = suchen(&w, "GEHEIM", &GRENZEN).expect("search");
        assert_eq!(s.treffer, Vec::<Treffer>::new());
        assert!(
            s.abgeschnitten,
            "a file that is not searched marks the answer"
        );
        let p2 = Probe::neu("fifo");
        let w2 = p2.w();
        let status = std::process::Command::new("mkfifo")
            .arg(w2.join("f"))
            .status()
            .expect("mkfifo");
        assert!(status.success());
        fs::write(w2.join("g.txt"), "da").expect("file");
        let s = suchen(&w2, "da", &GRENZEN).expect("search, the FIFO does not block");
        assert_eq!(treffer(&s), vec![("g.txt", 1, "da")]);
        assert!(!s.abgeschnitten);
    }

    #[test]
    fn grenzen_des_ordners() {
        let p = Probe::neu("g-ordner");
        let w = p.w();
        for n in ["a", "b", "c"] {
            fs::write(w.join(n), "x").expect("file");
        }
        let g3 = Grenzen {
            eintraege: 3,
            ..GRENZEN
        };
        let i = ordner_lesen(&w, &[], &g3).expect("read");
        assert_eq!(i.eintraege.len(), 3);
        assert!(!i.abgeschnitten);
        let g2 = Grenzen {
            eintraege: 2,
            ..GRENZEN
        };
        let i = ordner_lesen(&w, &[], &g2).expect("read");
        assert_eq!(i.eintraege.len(), 2);
        assert!(i.abgeschnitten);
        // the search reads at most as many entries of a folder, and says so
        let s = suchen(&w, "x", &g2).expect("search");
        assert_eq!(s.treffer.len(), 2);
        assert!(s.abgeschnitten);
        let s = suchen(&w, "x", &g3).expect("search");
        assert_eq!(s.treffer.len(), 3);
        assert!(!s.abgeschnitten);
        fs::create_dir_all(w.join("d")).expect("dir");
        for n in ["e", "f", "g"] {
            fs::write(w.join("d").join(n), "x").expect("file");
        }
        let g4 = Grenzen {
            eintraege: 4,
            ..GRENZEN
        };
        let s = suchen(&w, "x", &g4).expect("search");
        assert_eq!(s.treffer.len(), 6);
        assert!(!s.abgeschnitten);
        let s = suchen(&w, "x", &Grenzen { eintraege: 2, ..g4 }).expect("search");
        assert!(s.abgeschnitten);
    }

    #[test]
    fn grenzen_der_treffer_und_dateien() {
        let p = Probe::neu("g-treffer");
        let w = p.w();
        fs::write(w.join("a"), "x\nx\nx\n").expect("file");
        fs::write(w.join("b"), "x\n").expect("file");
        let g = |treffer, dateien| Grenzen {
            treffer,
            dateien,
            ..GRENZEN
        };
        let s = suchen(&w, "x", &g(4, 2)).expect("search");
        assert_eq!(s.treffer.len(), 4);
        assert!(!s.abgeschnitten);
        let s = suchen(&w, "x", &g(3, 2)).expect("search");
        assert_eq!(s.treffer.len(), 3);
        assert!(s.abgeschnitten);
        let s = suchen(&w, "x", &g(10, 1)).expect("search");
        assert_eq!(treffer(&s).len(), 3);
        assert!(s.abgeschnitten);
        // the limit of the hits ends the whole search: the next file is not opened (the clock is asked at the start and once
        // before each file it opens)
        let mut blicke = 0;
        let s = inhalt_suchen(&w, "x", &g(1, 2), &mut || {
            blicke += 1;
            0
        })
        .expect("search");
        assert_eq!(treffer(&s), vec![("a", 1, "x")]);
        assert!(s.abgeschnitten);
        assert_eq!(blicke, 2);
        let mut blicke = 0;
        let s = inhalt_suchen(&w, "x", &g(10, 2), &mut || {
            blicke += 1;
            0
        })
        .expect("search");
        assert_eq!(treffer(&s).len(), 4);
        assert_eq!(blicke, 3);
    }

    #[test]
    fn rennen_ein_eintrag_wird_zwischen_statat_und_oeffnen_getauscht() {
        let p = Probe::neu("rennen-suche");
        let w = p.w();
        fs::create_dir_all(w.join("d")).expect("dir");
        fs::write(w.join("d/innen.txt"), "alt").expect("file");
        fs::write(w.join("e.txt"), "alt").expect("file");
        let ablage = p.0.join("ablage");
        fs::create_dir_all(&ablage).expect("dir");
        // after `statat` the folder `d` and the file `e.txt` are moved away and new ones with the pattern take their places: the
        // device and inode differ from what `statat` saw, so neither is read
        let mut tausch = |pfad: &str| match pfad {
            "d" => {
                fs::rename(w.join("d"), ablage.join("d")).expect("move");
                fs::create_dir_all(w.join("d")).expect("dir");
                fs::write(w.join("d/neu.txt"), "NEU").expect("file");
            }
            "e.txt" => {
                fs::rename(w.join("e.txt"), ablage.join("e.txt")).expect("move");
                fs::write(w.join("e.txt"), "NEU").expect("file");
            }
            _ => {}
        };
        let s = inhalt_suchen_mit(&w, "NEU", &GRENZEN, &mut || 0, &mut tausch).expect("search");
        assert_eq!(s.treffer, Vec::<Treffer>::new());
        assert!(s.abgeschnitten);
        // without a swap the same entries are read
        let s = suchen(&w, "NEU", &GRENZEN).expect("search");
        assert_eq!(
            treffer(&s),
            vec![("d/neu.txt", 1, "NEU"), ("e.txt", 1, "NEU")]
        );
        assert!(!s.abgeschnitten);
    }

    #[test]
    fn grenze_der_antwort() {
        let p = Probe::neu("g-antwort");
        let w = p.w();
        fs::write(w.join("a"), "x\nx\n").expect("file");
        // one hit takes (1 + 1) * 6 + 64 = 76 bytes
        let g = |antwort_bytes| Grenzen {
            antwort_bytes,
            ..GRENZEN
        };
        let s = suchen(&w, "x", &g(152)).expect("search");
        assert_eq!(s.treffer.len(), 2);
        assert!(!s.abgeschnitten);
        let s = suchen(&w, "x", &g(151)).expect("search");
        assert_eq!(s.treffer.len(), 1);
        assert!(s.abgeschnitten);
    }

    #[test]
    fn grenzen_der_tiefe() {
        let p = Probe::neu("g-tiefe");
        let w = p.w();
        fs::create_dir_all(w.join("1/2")).expect("dir");
        fs::write(w.join("1/a"), "x").expect("file");
        fs::write(w.join("1/2/b"), "x").expect("file");
        let g = |tiefe| Grenzen { tiefe, ..GRENZEN };
        let s = suchen(&w, "x", &g(2)).expect("search");
        assert_eq!(treffer(&s), vec![("1/2/b", 1, "x"), ("1/a", 1, "x")]);
        assert!(!s.abgeschnitten);
        let s = suchen(&w, "x", &g(1)).expect("search");
        assert_eq!(treffer(&s), vec![("1/a", 1, "x")]);
        assert!(s.abgeschnitten);
    }

    #[test]
    fn grenzen_der_bytes() {
        let p = Probe::neu("g-bytes");
        let w = p.w();
        fs::write(w.join("a"), "abcx").expect("file");
        fs::write(w.join("b"), "x").expect("file");
        let g = |datei_bytes, gesamt_bytes| Grenzen {
            datei_bytes,
            gesamt_bytes,
            ..GRENZEN
        };
        let s = suchen(&w, "x", &g(4, 5)).expect("search");
        assert_eq!(treffer(&s), vec![("a", 1, "abcx"), ("b", 1, "x")]);
        assert!(!s.abgeschnitten);
        // the file is longer than its limit: the part is searched, the answer is marked
        let s = suchen(&w, "x", &g(3, 100)).expect("search");
        assert_eq!(treffer(&s), vec![("b", 1, "x")]);
        assert!(s.abgeschnitten);
        // the whole search reads at most 4 bytes: the second file is not read
        let s = suchen(&w, "x", &g(4, 4)).expect("search");
        assert_eq!(treffer(&s), vec![("a", 1, "abcx")]);
        assert!(s.abgeschnitten);
        // the total ends inside the first file: its first 3 bytes are searched, the second file is not opened
        let s = suchen(&w, "x", &g(4, 3)).expect("search");
        assert_eq!(s.treffer, Vec::<Treffer>::new());
        assert!(s.abgeschnitten);
        let s = suchen(&w, "ab", &g(4, 3)).expect("search");
        assert_eq!(treffer(&s), vec![("a", 1, "abc")]);
        assert!(s.abgeschnitten);
        // the second file gets only the rest of the total
        fs::write(w.join("b"), "xyz").expect("file");
        let s = suchen(&w, "y", &g(4, 6)).expect("search");
        assert_eq!(treffer(&s), vec![("b", 1, "xy")]);
        assert!(s.abgeschnitten);
        let s = suchen(&w, "y", &g(4, 7)).expect("search");
        assert_eq!(treffer(&s), vec![("b", 1, "xyz")]);
        assert!(!s.abgeschnitten);
    }

    #[test]
    fn grenze_der_laufzeit_mit_der_uhr_des_dienstes() {
        let probe = Probe::neu("g-zeit");
        let w = probe.w();
        for n in ["a", "b", "c"] {
            fs::write(w.join(n), "x").expect("file");
        }
        let g = Grenzen {
            laufzeit_ms: 10,
            ..GRENZEN
        };
        // the clock goes forward 4 ms with every look: at the start 0, then 4, 8, 12
        let mut zeit = 0;
        let mut uhr = || {
            let jetzt = zeit;
            zeit += 4;
            jetzt
        };
        let s = inhalt_suchen(&w, "x", &g, &mut uhr).expect("search");
        assert_eq!(treffer(&s), vec![("a", 1, "x"), ("b", 1, "x")]);
        assert!(s.abgeschnitten);
        // a clock that stands still (test mode) never reaches the limit
        let s = inhalt_suchen(&w, "x", &g, &mut || 7).expect("search");
        assert_eq!(s.treffer.len(), 3);
        assert!(!s.abgeschnitten);
        // a clock that would go back does not either
        let mut z = 100;
        let mut zurueck = || {
            z -= 1;
            z
        };
        let s = inhalt_suchen(&w, "x", &g, &mut zurueck).expect("search");
        assert_eq!(s.treffer.len(), 3);
    }

    #[test]
    fn unlesbare_eintraege_markieren_die_antwort() {
        let p = Probe::neu("rechte");
        let w = p.w();
        fs::create_dir_all(w.join("zu")).expect("dir");
        fs::write(w.join("zu/x"), "x").expect("file");
        fs::write(w.join("f"), "x").expect("file");
        fs::write(w.join("g"), "x").expect("file");
        fs::set_permissions(w.join("zu"), fs::Permissions::from_mode(0o000)).expect("chmod");
        fs::set_permissions(w.join("g"), fs::Permissions::from_mode(0o000)).expect("chmod");
        // a run as root reads everything: the case cannot be made then
        if rustix::process::geteuid().is_root() {
            fs::set_permissions(w.join("zu"), fs::Permissions::from_mode(0o700)).expect("chmod");
            return;
        }
        let s = suchen(&w, "x", &GRENZEN).expect("search");
        assert_eq!(treffer(&s), vec![("f", 1, "x")]);
        assert!(s.abgeschnitten);
        assert_eq!(lesen_text(&w, "zu"), Err(NICHT_LESBAR));
        let i = lesen_text(&w, "").expect("root");
        assert_eq!(i.eintraege.len(), 3);
        assert!(!i.abgeschnitten);
        fs::set_permissions(w.join("zu"), fs::Permissions::from_mode(0o700)).expect("chmod");
        // the root itself without rights: a refusal, not an empty list
        fs::set_permissions(&w, fs::Permissions::from_mode(0o300)).expect("chmod");
        assert_eq!(ordner_lesen(&w, &[], &GRENZEN), Err(NICHT_LESBAR));
        assert_eq!(suchen(&w, "x", &GRENZEN), Err(NICHT_LESBAR));
        fs::set_permissions(&w, fs::Permissions::from_mode(0o700)).expect("chmod");
    }

    #[test]
    fn nur_lesen_liest_bis_zur_grenze() {
        let p = Probe::neu("lesen");
        let f = p.w().join("f");
        fs::write(&f, "abcd").expect("file");
        let offen = |pfad: &Path| -> OwnedFd {
            openat(CWD, pfad, DATEI_OFFEN, Mode::empty()).expect("open")
        };
        assert_eq!(lesen(offen(&f), 4), (b"abcd".to_vec(), true));
        assert_eq!(lesen(offen(&f), 5), (b"abcd".to_vec(), true));
        assert_eq!(lesen(offen(&f), 3), (b"abc".to_vec(), false));
        assert_eq!(lesen(offen(&f), 0), (Vec::new(), false));
    }

    #[test]
    fn nicht_utf8_namen_fehlen_mit_merkmal() {
        use std::ffi::OsStr;
        use std::os::unix::ffi::OsStrExt;
        let p = Probe::neu("utf8");
        let w = p.w();
        let name = OsStr::from_bytes(b"a\xff");
        // macOS (APFS) refuses names that are no UTF-8: the case cannot be made there
        if fs::write(w.join(name), "x").is_err() {
            return;
        }
        fs::write(w.join("b"), "x").expect("file");
        let i = lesen_text(&w, "").expect("root");
        assert_eq!(namen(&i), vec![("b", Art::Datei, Some(1))]);
        assert!(i.abgeschnitten);
        let s = suchen(&w, "x", &GRENZEN).expect("search");
        assert_eq!(treffer(&s), vec![("b", 1, "x")]);
        assert!(s.abgeschnitten);
    }
}
