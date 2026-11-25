//! `modell.suche` (section 4; contract 8, section 9a, S3 and S4): the holder looks for the value of the secret of a
//! target below up to 16 roots and answers only with four counts. Links are never followed or read (`O_NOFOLLOW`, every
//! entry looked at with `lstat` relative to its folder); folders are walked to a depth of 256; a file reachable twice is
//! read once (device and inode). Occurrences are counted exactly, left to right without overlap, across read borders.

use std::collections::HashSet;
use std::io::Read;
use std::os::fd::{AsFd, OwnedFd};
use std::path::Path;

use rustix::fs::{AtFlags, Dir, FileType, Mode, OFlags, fstat, openat, statat};

/// Deepest folder below a root (the root has depth 0).
pub const TIEFE_MAX: usize = 256;
/// Size of one read.
pub const PUFFER: usize = 65_536;

/// The four counts.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Ergebnis {
    /// Regular files read to their end.
    pub dateien: u64,
    /// Files, folders and roots that could not be read.
    pub nicht_lesbar: u64,
    /// Occurrences of the value.
    pub treffer: u64,
    /// Links, FIFOs, sockets and devices inside a tree.
    pub uebersprungen: u64,
}

impl Ergebnis {
    // One place per count: the defensive branches that use them (`fstat` on an open descriptor, an error while reading
    // a folder, an entry gone between reading the folder and its `lstat`) cannot be reached by a test without a race,
    // the counts themselves are reached by the tests of unreadable and skipped entries.
    fn nicht_lesbar_zaehlen(&mut self) {
        self.nicht_lesbar += 1;
    }

    fn uebersprungen_zaehlen(&mut self) {
        self.uebersprungen += 1;
    }
}

/// The hits of `nadel` in everything `r` gives, read in parts of `puffer` bytes; `false` if a read failed (the hits
/// before it are counted). An interrupted read is repeated.
fn zaehlen(r: &mut impl Read, nadel: &[u8], puffer: usize) -> (u64, bool) {
    let mut z = Zaehler::neu(nadel);
    let mut buf = vec![0u8; puffer];
    let mut treffer = 0;
    loop {
        match r.read(&mut buf) {
            Ok(0) => return (treffer, true),
            Ok(n) => treffer += z.block(&buf[..n]),
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
            Err(_) => return (treffer, false),
        }
    }
}

/// A streaming counter after Knuth, Morris and Pratt that starts anew after each hit (no overlap).
struct Zaehler<'a> {
    nadel: &'a [u8],
    rand: Vec<usize>,
    stand: usize,
}

impl<'a> Zaehler<'a> {
    fn neu(nadel: &'a [u8]) -> Self {
        let mut rand = vec![0usize; nadel.len()];
        let mut k = 0;
        for i in 1..nadel.len() {
            while k > 0 && nadel[i] != nadel[k] {
                k = rand[k - 1];
            }
            if nadel[i] == nadel[k] {
                k += 1;
            }
            rand[i] = k;
        }
        Zaehler {
            nadel,
            rand,
            stand: 0,
        }
    }

    fn block(&mut self, b: &[u8]) -> u64 {
        if self.nadel.is_empty() {
            return 0;
        }
        let mut n = 0;
        for &c in b {
            while self.stand > 0 && c != self.nadel[self.stand] {
                self.stand = self.rand[self.stand - 1];
            }
            if c == self.nadel[self.stand] {
                self.stand += 1;
            }
            if self.stand == self.nadel.len() {
                n += 1;
                self.stand = 0;
            }
        }
        n
    }
}

/// Device and inode of a status; their types differ by platform (`i32` device on macOS).
#[allow(
    clippy::cast_sign_loss,
    clippy::unnecessary_cast,
    clippy::cast_lossless,
    reason = "the types of st_dev and st_ino differ by platform; the bits are only compared"
)]
#[must_use]
pub fn geraet_inode(st: &rustix::fs::Stat) -> (u64, u64) {
    (st.st_dev as u64, st.st_ino as u64)
}

struct Lauf<'a> {
    nadel: &'a [u8],
    puffer: usize,
    gesehen: HashSet<(u64, u64)>,
    e: Ergebnis,
}

// Flags are joined with `union`: `|` and `^` give the same value for disjoint flags, so that mutant could never fail.
fn datei_flags() -> OFlags {
    OFlags::RDONLY
        .union(OFlags::NOFOLLOW)
        .union(OFlags::CLOEXEC)
        .union(OFlags::NONBLOCK)
        .union(OFlags::NOCTTY)
}

fn ordner_flags() -> OFlags {
    OFlags::RDONLY
        .union(OFlags::DIRECTORY)
        .union(OFlags::NOFOLLOW)
        .union(OFlags::CLOEXEC)
}

impl Lauf<'_> {
    fn datei(&mut self, fd: OwnedFd) {
        let Ok(st) = fstat(&fd) else {
            self.e.nicht_lesbar_zaehlen();
            return;
        };
        if FileType::from_raw_mode(st.st_mode) != FileType::RegularFile {
            self.e.uebersprungen_zaehlen();
            return;
        }
        let schluessel = geraet_inode(&st);
        if !self.gesehen.insert(schluessel) {
            return;
        }
        let (treffer, ganz) = zaehlen(&mut std::fs::File::from(fd), self.nadel, self.puffer);
        self.e.treffer += treffer;
        if ganz {
            self.e.dateien += 1;
        } else {
            self.e.nicht_lesbar_zaehlen();
        }
    }

    fn ordner(&mut self, fd: &OwnedFd, tiefe: usize) {
        let Ok(dir) = Dir::read_from(fd.as_fd()) else {
            self.e.nicht_lesbar_zaehlen();
            return;
        };
        let mut namen = Vec::new();
        for e in dir {
            let Ok(e) = e else {
                self.e.nicht_lesbar_zaehlen();
                return;
            };
            if !matches!(e.file_name().to_bytes(), b"." | b"..") {
                namen.push(e.file_name().to_owned());
            }
        }
        for name in namen {
            let Ok(st) = statat(fd, name.as_c_str(), AtFlags::SYMLINK_NOFOLLOW) else {
                self.e.nicht_lesbar_zaehlen();
                continue;
            };
            match FileType::from_raw_mode(st.st_mode) {
                FileType::Directory => {
                    if tiefe + 1 > TIEFE_MAX {
                        self.e.nicht_lesbar_zaehlen();
                        continue;
                    }
                    match openat(fd, name.as_c_str(), ordner_flags(), Mode::empty()) {
                        Ok(unter) => self.ordner(&unter, tiefe + 1),
                        Err(_) => self.e.nicht_lesbar_zaehlen(),
                    }
                }
                FileType::RegularFile => {
                    match openat(fd, name.as_c_str(), datei_flags(), Mode::empty()) {
                        Ok(f) => self.datei(f),
                        Err(_) => self.e.nicht_lesbar_zaehlen(),
                    }
                }
                _ => self.e.uebersprungen_zaehlen(),
            }
        }
    }

    fn wurzel(&mut self, pfad: &Path) {
        let Ok(st) = statat(rustix::fs::CWD, pfad, AtFlags::SYMLINK_NOFOLLOW) else {
            self.e.nicht_lesbar_zaehlen();
            return;
        };
        match FileType::from_raw_mode(st.st_mode) {
            FileType::Directory => {
                match openat(rustix::fs::CWD, pfad, ordner_flags(), Mode::empty()) {
                    Ok(fd) => self.ordner(&fd, 0),
                    Err(_) => self.e.nicht_lesbar_zaehlen(),
                }
            }
            FileType::RegularFile => {
                match openat(rustix::fs::CWD, pfad, datei_flags(), Mode::empty()) {
                    Ok(f) => self.datei(f),
                    Err(_) => self.e.nicht_lesbar_zaehlen(),
                }
            }
            _ => self.e.nicht_lesbar_zaehlen(),
        }
    }
}

/// Searches `wert` below every root; `puffer` is the size of one read (at least 1).
#[must_use]
pub fn durchsuchen(wert: &[u8], wurzeln: &[&Path], puffer: usize) -> Ergebnis {
    let mut lauf = Lauf {
        nadel: wert,
        puffer: puffer.max(1),
        gesehen: HashSet::new(),
        e: Ergebnis::default(),
    };
    for w in wurzeln {
        lauf.wurzel(w);
    }
    lauf.e
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;
    use std::io::{Error, ErrorKind};
    use std::os::fd::OwnedFd;

    use super::{Ergebnis, Lauf, Zaehler, zaehlen};

    /// The table of the borders after Knuth, Morris and Pratt, also where its making must fall back.
    #[test]
    fn rand_der_nadel() {
        assert_eq!(Zaehler::neu(b"aab").rand, [0, 1, 0]);
        assert_eq!(Zaehler::neu(b"abab").rand, [0, 0, 1, 2]);
        assert_eq!(Zaehler::neu(b"aabaaab").rand, [0, 1, 0, 1, 2, 2, 3]);
        assert_eq!(Zaehler::neu(b"abacabab").rand, [0, 0, 1, 0, 1, 2, 3, 2]);
        // the counter falls back on the border: "abacabab" in "abacabacabab" once, after a fall back
        assert_eq!(Zaehler::neu(b"abacabab").block(b"abacabacabab"), 1);
    }

    struct Quelle(VecDeque<std::io::Result<Vec<u8>>>);

    impl std::io::Read for Quelle {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            match self.0.pop_front() {
                None => Ok(0),
                Some(Err(e)) => Err(e),
                Some(Ok(t)) => {
                    buf[..t.len()].copy_from_slice(&t);
                    Ok(t.len())
                }
            }
        }
    }

    #[test]
    fn zaehlen_ueber_unterbrechung_und_bis_zum_fehler() {
        let mut q = Quelle(
            vec![
                Ok(b"xa".to_vec()),
                Err(Error::from(ErrorKind::Interrupted)),
                Ok(b"bx ab".to_vec()),
            ]
            .into(),
        );
        assert_eq!(zaehlen(&mut q, b"ab", 8), (2, true));
        let mut q = Quelle(
            vec![
                Ok(b"ab ab".to_vec()),
                Err(Error::from(ErrorKind::Other)),
                Ok(b"ab".to_vec()),
            ]
            .into(),
        );
        assert_eq!(zaehlen(&mut q, b"ab", 8), (2, false));
    }

    fn lauf(nadel: &[u8]) -> Lauf<'_> {
        Lauf {
            nadel,
            puffer: 8,
            gesehen: std::collections::HashSet::new(),
            e: Ergebnis::default(),
        }
    }

    /// A descriptor that is no regular file (it may have changed between `lstat` and `open`) is skipped. (A file that
    /// cannot be read is the case `false` of `zaehlen`; a real descriptor with a lasting error would only hang a mutant
    /// that repeats every error.)
    #[test]
    fn datei_mit_anderer_art() {
        let ordner = std::env::temp_dir();
        let fd: OwnedFd = std::fs::File::open(&ordner).expect("folder").into();
        let mut l = lauf(b"ab");
        l.datei(fd);
        assert_eq!(
            l.e,
            Ergebnis {
                uebersprungen: 1,
                ..Ergebnis::default()
            }
        );
    }

    #[test]
    fn ohne_ueberlappung_und_ueber_grenzen() {
        let mut z = Zaehler::neu(b"aa");
        assert_eq!(z.block(b"aaaaa"), 2);
        let mut z = Zaehler::neu(b"abab");
        assert_eq!(z.block(b"ababab"), 1);
        let mut z = Zaehler::neu(b"hgx1-wert");
        let text = b"x hgx1-wert y hgx1-wert hgx1-wer";
        let mut n = 0;
        for c in text.chunks(1) {
            n += z.block(c);
        }
        assert_eq!(n, 2);
        let mut z = Zaehler::neu(b"aab");
        assert_eq!(z.block(b"aaab"), 1);
    }
}
