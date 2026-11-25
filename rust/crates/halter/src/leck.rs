//! The leak lock (section 8): before a byte goes out, the holder looks for the bytes of every secret it holds. In a
//! stream it keeps the last (longest secret - 1) bytes back until the next part covers them; at the end of the stream
//! the kept bytes go out, because they cannot hold a whole secret.

use crate::geheim::Geheim;

/// `true` if `nadel` stands in `heu`.
#[must_use]
pub fn enthaelt(heu: &[u8], nadel: &[u8]) -> bool {
    !nadel.is_empty() && nadel.len() <= heu.len() && heu.windows(nadel.len()).any(|f| f == nadel)
}

/// `true` if any of `werte` stands in `daten`.
#[must_use]
pub fn trifft(werte: &[Geheim], daten: &[u8]) -> bool {
    werte.iter().any(|w| enthaelt(daten, w.bytes()))
}

/// A secret was found in the bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Treffer;

/// The lock over one stream of bytes.
pub struct Sperre<'a> {
    werte: &'a [Geheim],
    rueckhalt: usize,
    puffer: Vec<u8>,
    gesehen: u64,
    frei: u64,
}

impl<'a> Sperre<'a> {
    /// A lock for the secrets `werte`.
    #[must_use]
    pub fn neu(werte: &'a [Geheim]) -> Self {
        let laengste = werte.iter().map(|w| w.bytes().len()).max().unwrap_or(0);
        Sperre {
            werte,
            rueckhalt: laengste.saturating_sub(1),
            puffer: Vec::new(),
            gesehen: 0,
            frei: 0,
        }
    }

    /// Takes the next bytes and returns the bytes that may go out now.
    ///
    /// # Errors
    /// [`Treffer`] if the kept bytes and `daten` together hold a secret; nothing more may go out then.
    pub fn schieben(&mut self, daten: &[u8]) -> Result<Vec<u8>, Treffer> {
        self.puffer.extend_from_slice(daten);
        self.gesehen += daten.len() as u64;
        if trifft(self.werte, &self.puffer) {
            self.puffer.clear();
            return Err(Treffer);
        }
        let raus = self.puffer.len().saturating_sub(self.rueckhalt);
        let frei: Vec<u8> = self.puffer.drain(..raus).collect();
        self.frei += frei.len() as u64;
        Ok(frei)
    }

    /// The end of the stream: the kept bytes, which were checked already.
    pub fn ende(&mut self) -> Vec<u8> {
        self.frei += self.puffer.len() as u64;
        std::mem::take(&mut self.puffer)
    }

    /// Number of bytes taken so far.
    #[must_use]
    pub fn gesehen(&self) -> u64 {
        self.gesehen
    }

    /// Number of bytes released so far.
    #[must_use]
    pub fn frei(&self) -> u64 {
        self.frei
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn g(t: &[u8]) -> Geheim {
        Geheim::neu(t.to_vec())
    }

    #[test]
    fn treffer_ueber_teilgrenze() {
        let w = [g(b"hgx1-geheimwert")];
        let mut s = Sperre::neu(&w);
        let a = s.schieben(b"0123456789vorne hgx1-ge").expect("no hit yet");
        // 23 bytes in, 14 (longest secret - 1) kept back
        assert_eq!(a, b"012345678");
        assert_eq!(s.schieben(b"heimwert hinten"), Err(Treffer));
    }

    #[test]
    fn ende_gibt_alles_frei() {
        let w = [g(b"0123456789abcdef")];
        let mut s = Sperre::neu(&w);
        let mut aus = s.schieben(&[b'x'; 40]).expect("no hit");
        aus.extend(s.ende());
        assert_eq!(aus.len(), 40);
        assert_eq!(s.frei(), 40);
        assert_eq!(s.gesehen(), 40);
    }

    #[test]
    fn ohne_geheimnis_geht_alles_sofort() {
        let w: [Geheim; 0] = [];
        let mut s = Sperre::neu(&w);
        assert_eq!(s.schieben(b"abc").expect("no hit"), b"abc");
        assert_eq!(s.ende(), Vec::<u8>::new());
    }
}
