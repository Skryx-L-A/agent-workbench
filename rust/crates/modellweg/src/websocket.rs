//! The frames of the service after a WebSocket switch of a bound target (contract 8, section 13.3, W3 to W5 and
//! amendment 2). The frames are read after RFC 6455 section 5.2 and go on unchanged; the leak lock runs on their
//! payload, not on the raw bytes: one lock over the payload of every data frame in order, each control frame for itself.
//! A frame goes out only after the lock has run over its payload. The last (length of the longest key - 1) payload bytes
//! of the data frames are held back, and everything behind the first held byte waits with them, so the order stays.

use std::collections::VecDeque;

use crate::probelauf::{Folge, Zaehlung};
use crate::sperre::enthaelt;

/// The largest payload of one frame (section 13.3, as the largest body).
pub(crate) const LAENGE_MAX: u64 = 33_554_432;

/// Why the switched connection is torn down.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Abriss {
    /// RSV1, RSV2 or RSV3 set: `mw.antwort_komprimiert`.
    Komprimiert,
    /// Mask bit, unknown opcode, too long, or a frame against RFC 6455 section 5: `mw.ziel_nicht_erreichbar`.
    Regelwidrig,
    /// A key in the payload: `mw.leck_gesperrt`.
    Leck,
    /// Trial run: a frame of the harness that cannot be read (section 9, Z3): `mw.anfrage_unlesbar`.
    Unlesbar,
    /// Trial run: the count file cannot be written (section 9, Z3): `mw.zaehlung_fehler`.
    Zaehlung,
}

impl Abriss {
    /// The code of the line.
    pub(crate) fn code(self) -> &'static str {
        match self {
            Self::Komprimiert => "mw.antwort_komprimiert",
            Self::Regelwidrig => "mw.ziel_nicht_erreichbar",
            Self::Leck => "mw.leck_gesperrt",
            Self::Unlesbar => "mw.anfrage_unlesbar",
            Self::Zaehlung => "mw.zaehlung_fehler",
        }
    }
}

/// What a bridge runs on the way from the target to the harness: it takes the next part and returns the bytes that may
/// go now; at the end it returns the rest.
pub(crate) trait Filter {
    /// The next part.
    ///
    /// # Errors
    /// The reason to tear the connection down.
    fn teil(&mut self, teil: &[u8]) -> Result<Vec<u8>, Abriss>;

    /// The end of the connection.
    ///
    /// # Errors
    /// The reason to tear the connection down.
    fn ende(self) -> Result<Vec<u8>, Abriss>;
}

/// The plain lock on raw bytes, for the tests of the polling of the bridge.
#[cfg(test)]
impl Filter for crate::sperre::Sperre<'_> {
    fn teil(&mut self, teil: &[u8]) -> Result<Vec<u8>, Abriss> {
        crate::sperre::Sperre::teil(self, teil).ok_or(Abriss::Leck)
    }

    fn ende(self) -> Result<Vec<u8>, Abriss> {
        Ok(crate::sperre::Sperre::ende(self))
    }
}

/// The head of one frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Kopf {
    /// Length of the head: 2, 4 or 10, and 4 more for the masking key of a frame of the harness.
    kopflaenge: usize,
    /// Length of the payload.
    laenge: usize,
    steuer: bool,
    fin: bool,
}

/// What is wrong with the head of a frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kopffehler {
    /// RSV1, RSV2 or RSV3 set.
    Rsv,
    /// Mask bit not as it must be, unknown opcode, too long, or against RFC 6455 sections 5.4 and 5.5.
    Regel,
}

/// The head at the start of `e`: `None` while it is not complete up to the length. `offen`: a fragmented message is
/// open; `maskiert`: the mask bit must be set (frames of the harness) or must not be (frames of the service). The RSV
/// bits come first, then mask bit and opcode, then the length and the rules of RFC 6455 sections 5.4 and 5.5.
fn kopf_lesen(e: &[u8], offen: bool, maskiert: bool) -> Result<Option<Kopf>, Kopffehler> {
    let (Some(&b0), Some(&b1)) = (e.first(), e.get(1)) else {
        return Ok(None);
    };
    if b0 & 0x70 != 0 {
        return Err(Kopffehler::Rsv);
    }
    let opcode = b0 & 0x0f;
    if (b1 & 0x80 != 0) != maskiert || !matches!(opcode, 0x0 | 0x1 | 0x2 | 0x8 | 0x9 | 0xa) {
        return Err(Kopffehler::Regel);
    }
    let (kopflaenge, laenge) = match b1 & 0x7f {
        126 => match e.get(2..4) {
            Some(l) => (4, u64::from(u16::from_be_bytes([l[0], l[1]]))),
            None => return Ok(None),
        },
        127 => match e.get(2..10).and_then(|l| <[u8; 8]>::try_from(l).ok()) {
            Some(l) => (10, u64::from_be_bytes(l)),
            None => return Ok(None),
        },
        n => (2, u64::from(n)),
    };
    if laenge > LAENGE_MAX {
        return Err(Kopffehler::Regel);
    }
    let steuer = opcode >= 0x8;
    let fin = b0 & 0x80 != 0;
    let regelwidrig = (steuer && (!fin || laenge > 125))
        || (opcode == 0x0 && !offen)
        || (matches!(opcode, 0x1 | 0x2) && offen);
    if regelwidrig {
        return Err(Kopffehler::Regel);
    }
    let laenge = usize::try_from(laenge).map_err(|_| Kopffehler::Regel)?;
    Ok(Some(Kopf {
        kopflaenge: if maskiert { kopflaenge + 4 } else { kopflaenge },
        laenge,
        steuer,
        fin,
    }))
}

/// The lock on the frames of one switched connection.
pub(crate) struct Rahmensperre<'a> {
    schluessel: Vec<&'a [u8]>,
    halten: usize,
    /// Bytes read that are not yet a whole frame.
    eingang: Vec<u8>,
    /// Checked bytes not yet sent: from the first held payload byte on.
    wartend: Vec<u8>,
    /// The places in `wartend` of the last `halten` payload bytes of the data frames.
    stellen: VecDeque<usize>,
    /// The last `halten` payload bytes of the data frames (what the lock carries over).
    daten: Vec<u8>,
    /// A fragmented message is open (FIN=0 came, the closing continuation is missing).
    offen: bool,
}

impl<'a> Rahmensperre<'a> {
    /// A lock for keys of at least one byte each.
    pub(crate) fn neu(schluessel: &[&'a [u8]]) -> Self {
        let laengste = schluessel.iter().map(|s| s.len()).max().unwrap_or(1);
        Self {
            schluessel: schluessel.to_vec(),
            halten: laengste.saturating_sub(1),
            eingang: Vec::new(),
            wartend: Vec::new(),
            stellen: VecDeque::new(),
            daten: Vec::new(),
            offen: false,
        }
    }

    fn fund(&self, heu: &[u8]) -> bool {
        self.schluessel.iter().any(|s| enthaelt(heu, s))
    }

    /// The head at the start of `eingang` (frames of the service carry no mask).
    fn kopf(&self) -> Result<Option<Kopf>, Abriss> {
        kopf_lesen(&self.eingang, self.offen, false).map_err(|f| match f {
            Kopffehler::Rsv => Abriss::Komprimiert,
            Kopffehler::Regel => Abriss::Regelwidrig,
        })
    }

    /// The lock over the payload of a frame (whole or cut off at the end); for a data frame together with what the lock
    /// carries over. Returns the new carry of a data frame.
    fn pruefe(&self, k: Kopf, nutzdaten: &[u8]) -> Result<Option<Vec<u8>>, Abriss> {
        if k.steuer {
            return if self.fund(nutzdaten) {
                Err(Abriss::Leck)
            } else {
                Ok(None)
            };
        }
        let mut suche = self.daten.clone();
        suche.extend_from_slice(nutzdaten);
        if self.fund(&suche) {
            return Err(Abriss::Leck);
        }
        let ab = suche.len().saturating_sub(self.halten);
        Ok(Some(suche.split_off(ab)))
    }

    /// Takes the whole frame `k` (`rahmen` holds its head and its payload, already taken out of `eingang`).
    fn rahmen(&mut self, k: Kopf, rahmen: &[u8]) -> Result<(), Abriss> {
        let neu = self.pruefe(k, &rahmen[k.kopflaenge..])?;
        let von = self.wartend.len() + k.kopflaenge;
        self.wartend.extend_from_slice(rahmen);
        if let Some(daten) = neu {
            self.daten = daten;
            self.stellen.extend(von..von + k.laenge);
            let zuviel = self.stellen.len().saturating_sub(self.halten);
            self.stellen.drain(..zuviel);
            self.offen = !k.fin;
        }
        Ok(())
    }

    /// Everything before the first held payload byte.
    fn freigeben(&mut self) -> Vec<u8> {
        let bis = self.stellen.front().copied().unwrap_or(self.wartend.len());
        for s in &mut self.stellen {
            *s -= bis;
        }
        self.wartend.drain(..bis).collect()
    }
}

impl Filter for Rahmensperre<'_> {
    fn teil(&mut self, teil: &[u8]) -> Result<Vec<u8>, Abriss> {
        self.eingang.extend_from_slice(teil);
        while let Some(k) = self.kopf()? {
            if self.eingang.len() < k.kopflaenge + k.laenge {
                break;
            }
            let rahmen: Vec<u8> = self.eingang.drain(..k.kopflaenge + k.laenge).collect();
            self.rahmen(k, &rahmen)?;
        }
        Ok(self.freigeben())
    }

    /// A frame cut off by the end is no tear-down (amendment 2): what was read goes out, the held bytes and an incomplete
    /// head too; the lock still runs over the payload that came.
    fn ende(mut self) -> Result<Vec<u8>, Abriss> {
        if let Some(k) = self.kopf()? {
            self.pruefe(k, &self.eingang[k.kopflaenge..])?;
        }
        let mut raus = std::mem::take(&mut self.wartend);
        raus.append(&mut self.eingang);
        Ok(raus)
    }
}

/// Trial run (section 9, Z1 to Z3): the frames of the harness to the target after a switch. Each frame is read whole
/// and checked; the payload of a data frame is unmasked only for the count, the markers are counted over the sequence of
/// every data payload, and the count file is written before the frame goes on. The bytes to the target stay as the
/// harness sent them (still masked). A frame cut off by the end never goes out and is not counted.
pub(crate) struct Zaehlrahmen<'a> {
    zaehlung: &'a Zaehlung,
    folge: Folge,
    /// Bytes read that are not yet a whole frame.
    eingang: Vec<u8>,
    /// A fragmented message is open.
    offen: bool,
}

impl<'a> Zaehlrahmen<'a> {
    pub(crate) fn neu(zaehlung: &'a Zaehlung) -> Self {
        Self {
            zaehlung,
            folge: zaehlung.folge(),
            eingang: Vec::new(),
            offen: false,
        }
    }

    /// The next part from the harness: the whole frames that may go to the target now, and the reason to tear down if
    /// a frame cannot be read or counted (the frames before it go, it and everything after it never).
    pub(crate) fn teil(&mut self, teil: &[u8]) -> (Vec<u8>, Option<Abriss>) {
        self.eingang.extend_from_slice(teil);
        let mut raus = Vec::new();
        loop {
            let k = match kopf_lesen(&self.eingang, self.offen, true) {
                Ok(Some(k)) => k,
                Ok(None) => break,
                Err(_) => return (raus, Some(Abriss::Unlesbar)),
            };
            // A method, not `+`: a frame of length 0 must still leave the input (no endless loop under a mutant).
            let ganz = k.kopflaenge.saturating_add(k.laenge);
            if self.eingang.len() < ganz {
                break;
            }
            if !k.steuer {
                let maske = &self.eingang[k.kopflaenge - 4..k.kopflaenge];
                let mut nutz: Vec<u8> = self.eingang[k.kopflaenge..ganz]
                    .iter()
                    .zip(maske.iter().cycle())
                    .map(|(b, m)| b ^ m)
                    .collect();
                let gezaehlt = self.zaehlung.rahmen(&mut self.folge, &nutz);
                nutz.fill(0);
                if gezaehlt.is_err() {
                    return (raus, Some(Abriss::Zaehlung));
                }
                self.offen = !k.fin;
            }
            raus.extend(self.eingang.drain(..ganz));
        }
        (raus, None)
    }
}

#[cfg(test)]
mod tests {
    use super::{Abriss, Filter, LAENGE_MAX, Rahmensperre, Zaehlrahmen};

    const KEY: &[u8] = b"KEY";

    fn rahmen(b0: u8, nutz: &[u8]) -> Vec<u8> {
        let mut v = vec![b0];
        let n = nutz.len();
        if n < 126 {
            v.push(u8::try_from(n).expect("short"));
        } else if let Ok(n) = u16::try_from(n) {
            v.push(126);
            v.extend_from_slice(&n.to_be_bytes());
        } else {
            v.push(127);
            v.extend_from_slice(&u64::try_from(n).expect("length").to_be_bytes());
        }
        v.extend_from_slice(nutz);
        v
    }

    fn sperre() -> Rahmensperre<'static> {
        Rahmensperre::neu(&[KEY])
    }

    #[test]
    fn the_codes() {
        assert_eq!(Abriss::Komprimiert.code(), "mw.antwort_komprimiert");
        assert_eq!(Abriss::Regelwidrig.code(), "mw.ziel_nicht_erreichbar");
        assert_eq!(Abriss::Leck.code(), "mw.leck_gesperrt");
        assert_eq!(Abriss::Unlesbar.code(), "mw.anfrage_unlesbar");
        assert_eq!(Abriss::Zaehlung.code(), "mw.zaehlung_fehler");
    }

    #[test]
    fn a_frame_goes_out_without_its_last_payload_bytes() {
        let mut s = sperre();
        let r = rahmen(0x81, b"abcd");
        assert_eq!(s.teil(&r), Ok(b"\x81\x04ab".to_vec()));
        assert_eq!(s.ende(), Ok(b"cd".to_vec()));
    }

    #[test]
    fn a_frame_waits_until_it_is_whole() {
        let mut s = sperre();
        assert_eq!(s.teil(b"\x81"), Ok(Vec::new()));
        assert_eq!(s.teil(b"\x04ab"), Ok(Vec::new()));
        assert_eq!(s.teil(b"cdX"), Ok(b"\x81\x04ab".to_vec()));
        assert_eq!(s.ende(), Ok(b"cdX".to_vec()));
    }

    #[test]
    fn the_lock_runs_over_data_frames_and_skips_their_heads() {
        let mut s = sperre();
        assert_eq!(s.teil(&rahmen(0x01, b"xK")), Ok(b"\x01\x02".to_vec()));
        assert_eq!(s.teil(&rahmen(0x80, b"EY")), Err(Abriss::Leck));
        let mut s = sperre();
        assert_eq!(s.teil(&rahmen(0x01, b"xK")), Ok(b"\x01\x02".to_vec()));
        assert_eq!(s.teil(&rahmen(0x89, b"p")), Ok(Vec::new()));
        assert_eq!(s.teil(&rahmen(0x80, b"EY")), Err(Abriss::Leck));
        let mut s = sperre();
        let mut r = rahmen(0x01, b"xK");
        r.extend(rahmen(0x80, b"EY"));
        assert_eq!(s.teil(&r), Err(Abriss::Leck));
    }

    #[test]
    fn a_control_frame_is_checked_for_itself_and_waits_behind_held_bytes() {
        let mut s = sperre();
        assert_eq!(s.teil(&rahmen(0x81, b"abK")), Ok(b"\x81\x03a".to_vec()));
        assert_eq!(s.teil(&rahmen(0x89, b"EY")), Ok(Vec::new()));
        assert_eq!(s.ende(), Ok([&b"bK"[..], b"\x89\x02EY"].concat()));
        let mut s = sperre();
        assert_eq!(s.teil(&rahmen(0x89, b"aKEY")), Err(Abriss::Leck));
        let mut s = sperre();
        assert_eq!(
            s.teil(&rahmen(0x88, b"\x03\xe8ok")),
            Ok(b"\x88\x04\x03\xe8ok".to_vec())
        );
        assert_eq!(s.teil(&rahmen(0x8a, b"")), Ok(b"\x8a\x00".to_vec()));
        assert_eq!(s.ende(), Ok(Vec::new()));
    }

    #[test]
    fn data_after_a_control_frame_releases_it() {
        let mut s = sperre();
        assert_eq!(s.teil(&rahmen(0x01, b"ab")), Ok(b"\x01\x02".to_vec()));
        assert_eq!(s.teil(&rahmen(0x89, b"p")), Ok(Vec::new()));
        assert_eq!(
            s.teil(&rahmen(0x80, b"cdef")),
            Ok([&b"ab"[..], b"\x89\x01p", b"\x80\x04cd"].concat())
        );
        assert_eq!(s.ende(), Ok(b"ef".to_vec()));
    }

    #[test]
    fn the_rsv_bits_tear_down_first() {
        for b0 in [0xc1, 0xa1, 0x91, 0xf1] {
            let mut s = sperre();
            assert_eq!(s.teil(&[b0, 0x80]), Err(Abriss::Komprimiert), "{b0:x}");
        }
        let mut s = sperre();
        assert_eq!(s.teil(&[0xc3]), Ok(Vec::new()));
        assert_eq!(s.teil(&[0x01]), Err(Abriss::Komprimiert));
    }

    #[test]
    fn mask_bit_and_unknown_opcodes_tear_down() {
        let mut s = sperre();
        assert_eq!(
            s.teil(b"\x81\x82\x01\x02\x03\x04ab"),
            Err(Abriss::Regelwidrig)
        );
        for opcode in [0x3, 0x4, 0x5, 0x6, 0x7, 0xb, 0xc, 0xd, 0xe, 0xf] {
            let mut s = sperre();
            assert_eq!(
                s.teil(&[0x80 | opcode, 0x01, b'a']),
                Err(Abriss::Regelwidrig),
                "{opcode:x}"
            );
        }
        for opcode in [0x1, 0x2] {
            let mut s = sperre();
            assert_eq!(
                s.teil(&[0x80 | opcode, 0x00]),
                Ok(vec![0x80 | opcode, 0x00])
            );
        }
    }

    #[test]
    fn the_long_lengths() {
        let mut s = sperre();
        let nutz = vec![b'a'; 200];
        let r = rahmen(0x81, &nutz);
        assert_eq!(&r[..4], b"\x81\x7e\x00\xc8");
        assert_eq!(s.teil(&r), Ok(r[..r.len() - 2].to_vec()));
        assert_eq!(s.ende(), Ok(b"aa".to_vec()));
        let mut s = sperre();
        let nutz = vec![b'b'; 70_000];
        let r = rahmen(0x82, &nutz);
        assert_eq!(&r[..2], b"\x82\x7f");
        assert_eq!(s.teil(&r[..9]), Ok(Vec::new()));
        assert_eq!(s.teil(&r[9..]), Ok(r[..r.len() - 2].to_vec()));
        let mut s = sperre();
        let mut r = vec![0x81, 0x7e, 0x00, 0x05];
        r.extend_from_slice(b"abcde");
        assert_eq!(s.teil(&r), Ok(b"\x81\x7e\x00\x05abc".to_vec()));
    }

    #[test]
    fn a_length_over_the_limit_tears_down_at_the_head() {
        let mut s = sperre();
        let mut r = vec![0x82, 0x7f];
        r.extend_from_slice(&(LAENGE_MAX + 1).to_be_bytes());
        assert_eq!(s.teil(&r), Err(Abriss::Regelwidrig));
        let mut s = sperre();
        let mut r = vec![0x82, 0x7f];
        r.extend_from_slice(&u64::MAX.to_be_bytes());
        assert_eq!(s.teil(&r), Err(Abriss::Regelwidrig));
        let mut s = sperre();
        let mut r = vec![0x82, 0x7f];
        r.extend_from_slice(&LAENGE_MAX.to_be_bytes());
        assert_eq!(s.teil(&r), Ok(Vec::new()));
    }

    #[test]
    fn control_frames_must_be_final_and_short() {
        let mut s = sperre();
        assert_eq!(s.teil(b"\x09\x01p"), Err(Abriss::Regelwidrig));
        let mut s = sperre();
        let mut r = vec![0x89, 0x7e, 0x00, 0x7e];
        r.extend(vec![b'p'; 126]);
        assert_eq!(s.teil(&r), Err(Abriss::Regelwidrig));
        let mut s = sperre();
        let r = rahmen(0x89, &[b'p'; 125]);
        assert_eq!(s.teil(&r), Ok(r.clone()));
        let mut s = sperre();
        assert_eq!(
            s.teil(b"\x89\x7e\x00\x05ppppp"),
            Ok(b"\x89\x7e\x00\x05ppppp".to_vec())
        );
        for b0 in [0x08, 0x0a] {
            let mut s = sperre();
            assert_eq!(s.teil(&[b0, 0x00]), Err(Abriss::Regelwidrig), "{b0:x}");
        }
    }

    #[test]
    fn continuations_need_an_open_message() {
        let mut s = sperre();
        assert_eq!(s.teil(&rahmen(0x80, b"ab")), Err(Abriss::Regelwidrig));
        let mut s = sperre();
        assert_eq!(s.teil(&rahmen(0x00, b"ab")), Err(Abriss::Regelwidrig));
        let mut s = sperre();
        assert!(s.teil(&rahmen(0x01, b"ab")).is_ok());
        assert!(s.teil(&rahmen(0x00, b"cd")).is_ok());
        assert!(s.teil(&rahmen(0x80, b"ef")).is_ok());
        assert_eq!(s.teil(&rahmen(0x80, b"gh")), Err(Abriss::Regelwidrig));
        let mut s = sperre();
        assert!(s.teil(&rahmen(0x02, b"ab")).is_ok());
        assert_eq!(s.teil(&rahmen(0x81, b"cd")), Err(Abriss::Regelwidrig));
        let mut s = sperre();
        assert!(s.teil(&rahmen(0x01, b"ab")).is_ok());
        assert_eq!(s.teil(&rahmen(0x02, b"cd")), Err(Abriss::Regelwidrig));
        let mut s = sperre();
        assert!(s.teil(&rahmen(0x01, b"ab")).is_ok());
        assert!(s.teil(&rahmen(0x89, b"p")).is_ok());
        assert!(s.teil(&rahmen(0x80, b"cd")).is_ok());
        assert!(s.teil(&rahmen(0x82, b"ef")).is_ok());
    }

    #[test]
    fn a_frame_cut_off_at_the_end_goes_out() {
        let mut s = sperre();
        assert_eq!(s.teil(b"\x81\x0aabcd"), Ok(Vec::new()));
        assert_eq!(s.ende(), Ok(b"\x81\x0aabcd".to_vec()));
        let mut s = sperre();
        assert_eq!(s.teil(b"\x81"), Ok(Vec::new()));
        assert_eq!(s.ende(), Ok(b"\x81".to_vec()));
        let mut s = sperre();
        assert_eq!(s.teil(b"\x81\x7e\x00"), Ok(Vec::new()));
        assert_eq!(s.ende(), Ok(b"\x81\x7e\x00".to_vec()));
        let mut s = sperre();
        assert_eq!(s.teil(b"\x09\x7e\x00"), Ok(Vec::new()));
        assert_eq!(s.ende(), Ok(b"\x09\x7e\x00".to_vec()));
        let mut s = sperre();
        assert_eq!(s.teil(&rahmen(0x81, b"abK")), Ok(b"\x81\x03a".to_vec()));
        assert_eq!(s.teil(b"\x81\x0aE"), Ok(Vec::new()));
        assert_eq!(s.ende(), Ok(b"bK\x81\x0aE".to_vec()));
    }

    #[test]
    fn the_lock_runs_over_a_cut_off_payload() {
        let mut s = sperre();
        assert!(s.teil(&rahmen(0x81, b"abK")).is_ok());
        assert_eq!(s.teil(b"\x81\x0aEY"), Ok(Vec::new()));
        assert_eq!(s.ende(), Err(Abriss::Leck));
        let mut s = sperre();
        assert!(s.teil(&rahmen(0x81, b"abK")).is_ok());
        assert_eq!(s.teil(b"\x89\x0aEY"), Ok(Vec::new()));
        assert_eq!(s.ende(), Ok(b"bK\x89\x0aEY".to_vec()));
        let mut s = sperre();
        assert_eq!(s.teil(b"\x89\x0axKEY"), Ok(Vec::new()));
        assert_eq!(s.ende(), Err(Abriss::Leck));
    }

    #[test]
    fn several_frames_in_one_part_and_one_frame_over_many_parts() {
        let mut alle = rahmen(0x01, b"hello ");
        alle.extend(rahmen(0x89, b"p"));
        alle.extend(rahmen(0x80, b"world"));
        alle.extend(rahmen(0x88, b"\x03\xe8"));
        let mut s = sperre();
        let mut raus = s.teil(&alle).expect("no key");
        raus.extend(s.ende().expect("no key"));
        assert_eq!(raus, alle);
        let mut s = sperre();
        let mut raus = Vec::new();
        for b in &alle {
            raus.extend(s.teil(&[*b]).expect("no key"));
        }
        raus.extend(s.ende().expect("no key"));
        assert_eq!(raus, alle);
    }

    #[test]
    fn the_longest_key_sets_what_is_held_and_a_short_key_holds_nothing() {
        let mut s = Rahmensperre::neu(&[b"AB", b"KEYS"]);
        assert_eq!(s.teil(&rahmen(0x81, b"wxyz")), Ok(b"\x81\x04w".to_vec()));
        assert_eq!(s.teil(&rahmen(0x89, b"AB")), Err(Abriss::Leck));
        let mut s = Rahmensperre::neu(&[b"KEYS", b"AB"]);
        assert_eq!(s.teil(&rahmen(0x81, b"wxyz")), Ok(b"\x81\x04w".to_vec()));
        let mut s = Rahmensperre::neu(&[b"K"]);
        assert_eq!(s.teil(&rahmen(0x81, b"ab")), Ok(b"\x81\x02ab".to_vec()));
        assert_eq!(s.teil(&rahmen(0x89, b"p")), Ok(b"\x89\x01p".to_vec()));
        assert_eq!(s.teil(&rahmen(0x81, b"K")), Err(Abriss::Leck));
        let mut s = Rahmensperre::neu(&[]);
        assert_eq!(s.teil(&rahmen(0x81, b"ab")), Ok(b"\x81\x02ab".to_vec()));
    }

    #[test]
    fn the_carry_holds_only_the_last_bytes() {
        let mut s = sperre();
        assert!(s.teil(&rahmen(0x01, b"Kxxxxx")).is_ok());
        assert!(s.teil(&rahmen(0x80, b"EY")).is_ok());
        let mut s = sperre();
        assert_eq!(s.teil(&rahmen(0x01, b"a")), Ok(b"\x01\x01".to_vec()));
        assert_eq!(s.teil(&rahmen(0x00, b"b")), Ok(Vec::new()));
        assert_eq!(s.teil(&rahmen(0x80, b"c")), Ok(b"a\x00\x01".to_vec()));
        assert_eq!(s.ende(), Ok(b"b\x80\x01c".to_vec()));
    }

    /// A frame of the harness: masked with `maske`.
    fn vom_harness(b0: u8, maske: [u8; 4], nutz: &[u8]) -> Vec<u8> {
        let v = rahmen(b0, nutz);
        let mut r = v[..v.len() - nutz.len()].to_vec();
        r[1] |= 0x80;
        r.extend_from_slice(&maske);
        r.extend(nutz.iter().zip(maske.iter().cycle()).map(|(b, m)| b ^ m));
        r
    }

    struct Ort(std::path::PathBuf);

    impl Drop for Ort {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn zaehlung(name: &str) -> (Ort, crate::probelauf::Zaehlung) {
        let ordner = std::env::temp_dir().join(format!("mw-zr-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&ordner);
        std::fs::create_dir(&ordner).expect("folder");
        let z = crate::probelauf::Zaehlung::neu(
            vec!["MK-KU".to_owned()],
            ordner.join("z.json"),
            "s-1".to_owned(),
        );
        z.anfang().expect("first state");
        (Ort(ordner), z)
    }

    fn stand(o: &Ort) -> String {
        std::fs::read_to_string(o.0.join("z.json")).expect("read")
    }

    const M: [u8; 4] = [0x37, 0xfa, 0x21, 0x3d];

    #[test]
    fn harness_frames_are_unmasked_for_the_count_and_go_on_unchanged() {
        let (ort, zl) = zaehlung("roh");
        let mut leser = Zaehlrahmen::neu(&zl);
        let a = vom_harness(0x81, M, b"x MK-KU y MK-KU");
        assert!(!a.windows(5).any(|w| w == b"MK-KU"));
        assert_eq!(leser.teil(&a), (a.clone(), None));
        assert!(stand(&ort).contains(r#""MK-KU":2"#), "{}", stand(&ort));
        let b = vom_harness(0x82, [0, 0, 0, 0], b"MK-KU");
        assert_eq!(leser.teil(&b), (b.clone(), None));
        assert!(stand(&ort).contains(r#""MK-KU":3"#), "{}", stand(&ort));
        let lang = vom_harness(0x81, M, &[b"MK-KU".as_slice(), &[b'x'; 200]].concat());
        assert_eq!(&lang[..4], b"\x81\xfe\x00\xcd");
        assert_eq!(leser.teil(&lang), (lang.clone(), None));
        assert!(stand(&ort).contains(r#""MK-KU":4"#), "{}", stand(&ort));
    }

    #[test]
    fn a_marker_over_two_frames_counts_once_and_control_frames_do_not_count() {
        let (ort, zl) = zaehlung("geteilt");
        let mut leser = Zaehlrahmen::neu(&zl);
        let teile = [
            vom_harness(0x01, M, b"a MK-"),
            vom_harness(0x89, M, b"MK-KU"),
            vom_harness(0x80, M, b"KU b"),
            vom_harness(0x88, M, b"\x03\xe8MK-KU"),
            vom_harness(0x8a, M, b""),
            vom_harness(0x81, M, b"MK-K"),
            vom_harness(0x81, M, b"U"),
        ];
        let alle = teile.concat();
        let mut raus = Vec::new();
        for b in &alle {
            let (r, a) = leser.teil(&[*b]);
            assert_eq!(a, None);
            raus.extend(r);
        }
        assert_eq!(raus, alle);
        assert!(stand(&ort).contains(r#""MK-KU":2"#), "{}", stand(&ort));
    }

    #[test]
    fn a_frame_goes_out_only_whole_and_counted() {
        let (ort, zl) = zaehlung("ganz");
        let mut leser = Zaehlrahmen::neu(&zl);
        let a = vom_harness(0x81, M, b"MK-KU");
        assert_eq!(leser.teil(&a[..a.len() - 1]), (Vec::new(), None));
        assert!(stand(&ort).contains(r#""MK-KU":0"#), "{}", stand(&ort));
        let b = vom_harness(0x89, M, b"p");
        let mut rest = a[a.len() - 1..].to_vec();
        rest.extend_from_slice(&b);
        rest.extend_from_slice(&a[..3]);
        assert_eq!(leser.teil(&rest), ([a.clone(), b].concat(), None));
        assert!(stand(&ort).contains(r#""MK-KU":1"#), "{}", stand(&ort));
        assert_eq!(leser.teil(&a[3..]), (a, None));
        assert!(stand(&ort).contains(r#""MK-KU":2"#), "{}", stand(&ort));
    }

    #[test]
    fn an_unreadable_harness_frame_tears_down_after_the_frames_before_it() {
        let gut = vom_harness(0x81, M, b"ok");
        let mut lang = vec![0x82, 0xff];
        lang.extend_from_slice(&(LAENGE_MAX + 1).to_be_bytes());
        let mut genau = vec![0x82, 0xff];
        genau.extend_from_slice(&LAENGE_MAX.to_be_bytes());
        let mut ping_lang = vec![0x89, 0xfe, 0x00, 0x7e];
        ping_lang.extend_from_slice(&M);
        let schlecht: Vec<Vec<u8>> = vec![
            rahmen(0x81, b"ok"),
            vec![0xc1, 0x02],
            vec![0xa1, 0x82],
            vec![0x91, 0x82],
            vec![0x83, 0x82],
            vec![0x8b, 0x82],
            lang,
            vec![0x09, 0x81],
            ping_lang,
            vec![0x08, 0x80],
            vec![0x80, 0x82],
            vec![0x00, 0x82],
        ];
        for (i, s) in schlecht.iter().enumerate() {
            let (ort, zl) = zaehlung(&format!("schlecht{i}"));
            let mut leser = Zaehlrahmen::neu(&zl);
            let mut alles = gut.clone();
            alles.extend_from_slice(s);
            assert_eq!(
                leser.teil(&alles),
                (gut.clone(), Some(Abriss::Unlesbar)),
                "{i}"
            );
            assert!(stand(&ort).contains(r#""MK-KU":0"#), "{i}");
        }
        let (_ort, zl) = zaehlung("genau");
        let mut leser = Zaehlrahmen::neu(&zl);
        assert_eq!(leser.teil(&genau), (Vec::new(), None));
        for (i, zweiter) in [0x81u8, 0x82].into_iter().enumerate() {
            let (_ort, zl) = zaehlung(&format!("offen{i}"));
            let mut leser = Zaehlrahmen::neu(&zl);
            let a = vom_harness(0x01, M, b"ab");
            let ping = vom_harness(0x89, M, b"ping");
            let fort = vom_harness(0x00, M, b"cd");
            let mut alles = [a.clone(), ping.clone(), fort.clone()].concat();
            alles.extend(vom_harness(zweiter, M, b"x"));
            assert_eq!(
                leser.teil(&alles),
                ([a, ping, fort].concat(), Some(Abriss::Unlesbar)),
                "{i}"
            );
        }
        let (_ort, zl) = zaehlung("schluss");
        let mut leser = Zaehlrahmen::neu(&zl);
        let a = vom_harness(0x02, M, b"ab");
        let fort = vom_harness(0x80, M, b"cd");
        let neu = vom_harness(0x82, M, b"ef");
        let alles = [a.clone(), fort.clone(), neu.clone()].concat();
        assert_eq!(leser.teil(&alles), (alles, None));
        assert_eq!(
            leser.teil(&vom_harness(0x80, M, b"g")),
            (Vec::new(), Some(Abriss::Unlesbar))
        );
    }

    #[test]
    fn a_count_file_that_cannot_be_written_stops_the_frame() {
        let (ort, zl) = zaehlung("datei");
        let mut leser = Zaehlrahmen::neu(&zl);
        let a = vom_harness(0x81, M, b"MK-KU");
        let p = vom_harness(0x89, M, b"p");
        assert_eq!(leser.teil(&a), (a.clone(), None));
        std::fs::remove_dir_all(&ort.0).expect("remove");
        assert_eq!(leser.teil(&p), (p.clone(), None));
        let b = vom_harness(0x81, M, b"MK-KU");
        let mut alles = p.clone();
        alles.extend_from_slice(&b);
        alles.extend_from_slice(&p);
        assert_eq!(leser.teil(&alles), (p, Some(Abriss::Zaehlung)));
    }
}
