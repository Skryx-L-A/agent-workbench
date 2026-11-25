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

fn datei_flags() -> OFlags {
    OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC | OFlags::NONBLOCK | OFlags::NOCTTY
}

fn ordner_flags() -> OFlags {
    OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC
}

impl Lauf<'_> {
    fn datei(&mut self, fd: OwnedFd) {
        let Ok(st) = fstat(&fd) else {
            self.e.nicht_lesbar += 1;
            return;
        };
        if FileType::from_raw_mode(st.st_mode) != FileType::RegularFile {
            self.e.uebersprungen += 1;
            return;
        }
        let schluessel = geraet_inode(&st);
        if !self.gesehen.insert(schluessel) {
            return;
        }
        let mut datei = std::fs::File::from(fd);
        let mut z = Zaehler::neu(self.nadel);
        let mut buf = vec![0u8; self.puffer];
        let mut treffer = 0;
        loop {
            match datei.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => treffer += z.block(&buf[..n]),
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
                Err(_) => {
                    self.e.nicht_lesbar += 1;
                    self.e.treffer += treffer;
                    return;
                }
            }
        }
        self.e.treffer += treffer;
        self.e.dateien += 1;
    }

    fn ordner(&mut self, fd: &OwnedFd, tiefe: usize) {
        let Ok(dir) = Dir::read_from(fd.as_fd()) else {
            self.e.nicht_lesbar += 1;
            return;
        };
        let mut namen = Vec::new();
        for e in dir {
            let Ok(e) = e else {
                self.e.nicht_lesbar += 1;
                return;
            };
            let n = e.file_name().to_bytes();
            if n != b"." && n != b".." {
                namen.push(e.file_name().to_owned());
            }
        }
        for name in namen {
            let Ok(st) = statat(fd, name.as_c_str(), AtFlags::SYMLINK_NOFOLLOW) else {
                self.e.nicht_lesbar += 1;
                continue;
            };
            match FileType::from_raw_mode(st.st_mode) {
                FileType::Directory => {
                    if tiefe + 1 > TIEFE_MAX {
                        self.e.nicht_lesbar += 1;
                        continue;
                    }
                    match openat(fd, name.as_c_str(), ordner_flags(), Mode::empty()) {
                        Ok(unter) => self.ordner(&unter, tiefe + 1),
                        Err(_) => self.e.nicht_lesbar += 1,
                    }
                }
                FileType::RegularFile => {
                    match openat(fd, name.as_c_str(), datei_flags(), Mode::empty()) {
                        Ok(f) => self.datei(f),
                        Err(_) => self.e.nicht_lesbar += 1,
                    }
                }
                _ => self.e.uebersprungen += 1,
            }
        }
    }

    fn wurzel(&mut self, pfad: &Path) {
        let Ok(st) = statat(rustix::fs::CWD, pfad, AtFlags::SYMLINK_NOFOLLOW) else {
            self.e.nicht_lesbar += 1;
            return;
        };
        match FileType::from_raw_mode(st.st_mode) {
            FileType::Directory => {
                match openat(rustix::fs::CWD, pfad, ordner_flags(), Mode::empty()) {
                    Ok(fd) => self.ordner(&fd, 0),
                    Err(_) => self.e.nicht_lesbar += 1,
                }
            }
            FileType::RegularFile => {
                match openat(rustix::fs::CWD, pfad, datei_flags(), Mode::empty()) {
                    Ok(f) => self.datei(f),
                    Err(_) => self.e.nicht_lesbar += 1,
                }
            }
            _ => self.e.nicht_lesbar += 1,
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
    use super::Zaehler;

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
