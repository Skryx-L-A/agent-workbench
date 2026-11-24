//! The frames on the socket (contract section 5): `[n: u32 big endian][art: u8][payload: n - 1 bytes]`.

/// Into the service: an event of the view (JSON object).
pub(crate) const A_EREIGNIS: u8 = 0x01;
/// Into the service, test mode: set the clock.
pub(crate) const A_UHR: u8 = 0x10;
/// Both ways, test mode: a viewer that takes nothing (in), and the receipt (out).
pub(crate) const A_STUMM: u8 = 0x11;
/// Both ways, test mode: the state (out carries JSON).
pub(crate) const A_ZUSTAND: u8 = 0x12;
/// Into the service, test mode: the core is built anew.
pub(crate) const A_KERNFEHLER: u8 = 0x13;
/// Out: a raster frame of `rasterauszug`.
pub(crate) const A_RASTER: u8 = 0x05;
/// Out: a message of the core.
pub(crate) const A_NACHRICHT: u8 = 0x06;
/// Out: the core rejected an event of this viewer.
pub(crate) const A_ABLEHNUNG: u8 = 0x07;
/// Out: a protocol error, the connection ends.
pub(crate) const A_FEHLER: u8 = 0x08;

/// The largest `n` the service reads (DI3).
pub(crate) const MAX_HINEIN: usize = 1_048_576;

/// A frame as bytes: head of five bytes and the payload.
pub(crate) fn rahmen(art: u8, nutzlast: &[u8]) -> Vec<u8> {
    let n = u32::try_from(nutzlast.len().saturating_add(1)).unwrap_or(u32::MAX);
    let mut aus = Vec::with_capacity(nutzlast.len() + 5);
    aus.extend_from_slice(&n.to_be_bytes());
    aus.push(art);
    aus.extend_from_slice(nutzlast);
    aus
}

/// What can be wrong with the head of a frame (DI3). The text is the code on the wire.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum KopfFehler {
    /// `n` is 0.
    LaengeUngueltig,
    /// `n` is larger than [`MAX_HINEIN`]; known as soon as the four bytes of the head are there.
    ZuGross,
}

impl KopfFehler {
    /// The code in the frame `Fehler`.
    pub(crate) fn code(self) -> &'static str {
        match self {
            Self::LaengeUngueltig => "laenge_ungueltig",
            Self::ZuGross => "rahmen_zu_gross",
        }
    }
}

/// The input side of one connection: puts the pieces together (a frame in several reads, several frames in one read).
#[derive(Default)]
pub(crate) struct Eingang {
    puffer: Vec<u8>,
}

impl Eingang {
    /// Adds bytes that were read.
    pub(crate) fn anhaengen(&mut self, bytes: &[u8]) {
        self.puffer.extend_from_slice(bytes);
    }

    /// `true` if bytes of an unfinished frame are held (a connection that ends then is a plain `Trennen`, DI3).
    #[cfg(test)]
    pub(crate) fn hat_rest(&self) -> bool {
        !self.puffer.is_empty()
    }

    /// Drops what is held (after an error the connection reads nothing more).
    pub(crate) fn leeren(&mut self) {
        self.puffer.clear();
    }

    /// The next whole frame as (art, payload), `None` if it is not complete yet. The head is judged as soon as its four bytes
    /// are there: a length above the limit does not wait for the payload (DI3).
    pub(crate) fn naechster(&mut self) -> Result<Option<(u8, Vec<u8>)>, KopfFehler> {
        let Some(kopf) = self.puffer.first_chunk::<4>() else {
            return Ok(None);
        };
        let n = usize::try_from(u32::from_be_bytes(*kopf)).unwrap_or(usize::MAX);
        if n == 0 {
            return Err(KopfFehler::LaengeUngueltig);
        }
        if n > MAX_HINEIN {
            return Err(KopfFehler::ZuGross);
        }
        if self.puffer.len() < 4 + n {
            return Ok(None);
        }
        let art = self.puffer[4];
        let nutzlast = self.puffer[5..4 + n].to_vec();
        self.puffer.drain(..4 + n);
        Ok(Some((art, nutzlast)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rahmen_form() {
        assert_eq!(rahmen(0x06, b"ab"), vec![0, 0, 0, 3, 6, b'a', b'b']);
        assert_eq!(rahmen(0x11, b""), vec![0, 0, 0, 1, 0x11]);
    }

    #[test]
    fn in_stuecken_und_gebuendelt() {
        let mut e = Eingang::default();
        assert_eq!(e.naechster(), Ok(None));
        let a = rahmen(1, b"xyz");
        let b = rahmen(2, b"");
        let alles: Vec<u8> = a.iter().chain(&b).copied().collect();
        // all bytes but the last: the first frame is whole after its last byte, the second is not
        for (i, byte) in alles[..alles.len() - 1].iter().enumerate() {
            e.anhaengen(&[*byte]);
            if i + 1 < a.len() {
                assert_eq!(e.naechster(), Ok(None), "byte {i}");
            }
        }
        assert_eq!(e.naechster(), Ok(Some((1, b"xyz".to_vec()))));
        assert_eq!(e.naechster(), Ok(None));
        assert!(e.hat_rest());
        e.anhaengen(&alles[alles.len() - 1..]);
        assert_eq!(e.naechster(), Ok(Some((2, Vec::new()))));
        assert!(!e.hat_rest());
        e.anhaengen(&alles);
        assert_eq!(e.naechster(), Ok(Some((1, b"xyz".to_vec()))));
        assert_eq!(e.naechster(), Ok(Some((2, Vec::new()))));
    }

    #[test]
    fn kopf_wird_sofort_beurteilt() {
        let mut e = Eingang::default();
        e.anhaengen(&[0, 0, 0]);
        assert_eq!(e.naechster(), Ok(None));
        e.anhaengen(&[0]);
        assert_eq!(e.naechster(), Err(KopfFehler::LaengeUngueltig));
        e.leeren();
        assert!(!e.hat_rest());
        // exactly the limit waits for the payload, one more is an error at once
        let grenze = u32::try_from(MAX_HINEIN).unwrap_or(0);
        e.anhaengen(&grenze.to_be_bytes());
        assert_eq!(e.naechster(), Ok(None));
        e.leeren();
        e.anhaengen(&(grenze + 1).to_be_bytes());
        assert_eq!(e.naechster(), Err(KopfFehler::ZuGross));
        e.leeren();
        e.anhaengen(&u32::MAX.to_be_bytes());
        assert_eq!(e.naechster(), Err(KopfFehler::ZuGross));
    }

    #[test]
    fn grenze_ganz() {
        let mut e = Eingang::default();
        let nutz = vec![b' '; MAX_HINEIN - 1];
        e.anhaengen(&rahmen(1, &nutz));
        let Ok(Some((art, n))) = e.naechster() else {
            panic!("frame at the limit is read");
        };
        assert_eq!((art, n.len()), (1, MAX_HINEIN - 1));
    }

    #[test]
    fn codes() {
        assert_eq!(KopfFehler::LaengeUngueltig.code(), "laenge_ungueltig");
        assert_eq!(KopfFehler::ZuGross.code(), "rahmen_zu_gross");
    }
}
