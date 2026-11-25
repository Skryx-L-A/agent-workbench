//! The frames of the target after a 101 (section 4, `modell.strom`): RFC 6455 section 5.2 and the frame rules of section
//! 5, checked fail closed at the head of each frame, and the leak lock over the payload of all data frames in order
//! (across frame and message borders); a control frame is checked on its own. The bytes go out unchanged, in order,
//! as soon as the lock released the payload before them.

use std::collections::VecDeque;

use crate::code::Code;
use crate::geheim::Geheim;
use crate::leck::{self, Sperre};

/// Longest payload of one frame of the target.
pub const NUTZ_MAX: u64 = 33_554_432;

/// Why the stream breaks off.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Abriss {
    /// RSV bit set (`halter.ziel_komprimiert`).
    Komprimiert,
    /// Mask bit, opcode, length or a frame rule (`halter.ziel_fehler`).
    Fehler,
    /// A secret in the payload (`halter.leck_gesperrt`).
    Leck,
}

impl Abriss {
    /// The code of the line `ende`.
    #[must_use]
    pub fn code(self) -> Code {
        match self {
            Abriss::Komprimiert => Code::ZielKomprimiert,
            Abriss::Fehler => Code::ZielFehler,
            Abriss::Leck => Code::LeckGesperrt,
        }
    }
}

enum Stueck {
    /// Bytes checked already: a head of a data frame or a whole control frame.
    Roh(Vec<u8>),
    /// So many payload bytes of data frames, released by the lock in order.
    Daten(u64),
}

enum Zustand {
    Kopf(Vec<u8>),
    Daten(u64),
    Steuer { kopf_und_nutz: Vec<u8>, rest: u64 },
}

/// The reader of the direction target to bridge.
pub struct Rahmenleser<'a> {
    werte: &'a [Geheim],
    sperre: Sperre<'a>,
    zustand: Zustand,
    offen: bool,
    warte: VecDeque<Stueck>,
    frei: VecDeque<u8>,
}

/// The length of a complete head, or `None` while bytes are missing; checks RSV first, then the mask bit.
fn kopflaenge(k: &[u8]) -> Result<Option<usize>, Abriss> {
    let Some(&b0) = k.first() else {
        return Ok(None);
    };
    if b0 & 0x70 != 0 {
        return Err(Abriss::Komprimiert);
    }
    let Some(&b1) = k.get(1) else {
        return Ok(None);
    };
    if b1 & 0x80 != 0 {
        return Err(Abriss::Fehler);
    }
    let n = match b1 & 0x7f {
        126 => 4,
        127 => 10,
        _ => 2,
    };
    Ok((k.len() >= n).then_some(n))
}

impl<'a> Rahmenleser<'a> {
    /// A reader for the secrets `werte`.
    #[must_use]
    pub fn neu(werte: &'a [Geheim]) -> Self {
        Rahmenleser {
            werte,
            sperre: Sperre::neu(werte),
            zustand: Zustand::Kopf(Vec::new()),
            offen: false,
            warte: VecDeque::new(),
            frei: VecDeque::new(),
        }
    }

    /// The rules at a complete head; returns the next state and the head bytes.
    fn kopf_fertig(&mut self, k: Vec<u8>) -> Result<Zustand, Abriss> {
        let fin = k[0] & 0x80 != 0;
        let opcode = k[0] & 0x0f;
        let laenge = match k[1] & 0x7f {
            126 => u64::from(u16::from_be_bytes([k[2], k[3]])),
            127 => {
                let mut acht = [0u8; 8];
                acht.copy_from_slice(&k[2..10]);
                u64::from_be_bytes(acht)
            }
            n => u64::from(n),
        };
        if !matches!(opcode, 0x0 | 0x1 | 0x2 | 0x8 | 0x9 | 0xA) || laenge > NUTZ_MAX {
            return Err(Abriss::Fehler);
        }
        let steuer = opcode >= 0x8;
        if steuer {
            if !fin || laenge > 125 {
                return Err(Abriss::Fehler);
            }
            if laenge == 0 {
                self.warte.push_back(Stueck::Roh(k));
                return Ok(Zustand::Kopf(Vec::new()));
            }
            return Ok(Zustand::Steuer {
                kopf_und_nutz: k,
                rest: laenge,
            });
        }
        if opcode == 0x0 && !self.offen {
            return Err(Abriss::Fehler);
        }
        if opcode != 0x0 && self.offen {
            return Err(Abriss::Fehler);
        }
        self.offen = !fin;
        self.warte.push_back(Stueck::Roh(k));
        Ok(if laenge == 0 {
            Zustand::Kopf(Vec::new())
        } else {
            Zustand::Daten(laenge)
        })
    }

    fn sammeln(&mut self) -> Vec<u8> {
        let mut aus = Vec::new();
        while let Some(st) = self.warte.front_mut() {
            match st {
                Stueck::Roh(b) => {
                    aus.append(b);
                    self.warte.pop_front();
                }
                Stueck::Daten(n) => {
                    let zahl = usize::try_from(*n)
                        .unwrap_or(usize::MAX)
                        .min(self.frei.len());
                    aus.extend(self.frei.drain(..zahl));
                    *n -= zahl as u64;
                    if *n > 0 {
                        break;
                    }
                    self.warte.pop_front();
                }
            }
        }
        aus
    }

    /// Takes the next bytes of the target and returns the bytes that may go to the bridge now.
    ///
    /// # Errors
    /// [`Abriss`] at the first violation; nothing more may go out then, also not what was kept back.
    pub fn schieben(&mut self, mut daten: &[u8]) -> Result<Vec<u8>, Abriss> {
        while !daten.is_empty() {
            let zustand = std::mem::replace(&mut self.zustand, Zustand::Kopf(Vec::new()));
            self.zustand = match zustand {
                Zustand::Kopf(mut k) => {
                    // one byte at a time until the head is complete, so a violation is found as early as it stands
                    k.push(daten[0]);
                    daten = &daten[1..];
                    match kopflaenge(&k)? {
                        Some(_) => self.kopf_fertig(k)?,
                        None => Zustand::Kopf(k),
                    }
                }
                Zustand::Daten(rest) => {
                    let n = usize::try_from(rest).unwrap_or(usize::MAX).min(daten.len());
                    let frei = self
                        .sperre
                        .schieben(&daten[..n])
                        .map_err(|_| Abriss::Leck)?;
                    self.frei.extend(frei);
                    match self.warte.back_mut() {
                        Some(Stueck::Daten(m)) => *m += n as u64,
                        _ => self.warte.push_back(Stueck::Daten(n as u64)),
                    }
                    daten = &daten[n..];
                    if rest > n as u64 {
                        Zustand::Daten(rest - n as u64)
                    } else {
                        Zustand::Kopf(Vec::new())
                    }
                }
                Zustand::Steuer {
                    mut kopf_und_nutz,
                    rest,
                } => {
                    let n = usize::try_from(rest).unwrap_or(usize::MAX).min(daten.len());
                    kopf_und_nutz.extend_from_slice(&daten[..n]);
                    daten = &daten[n..];
                    if rest > n as u64 {
                        Zustand::Steuer {
                            kopf_und_nutz,
                            rest: rest - n as u64,
                        }
                    } else {
                        let kopf = kopflaenge(&kopf_und_nutz)?.unwrap_or(2);
                        if leck::trifft(self.werte, &kopf_und_nutz[kopf..]) {
                            return Err(Abriss::Leck);
                        }
                        self.warte.push_back(Stueck::Roh(kopf_und_nutz));
                        Zustand::Kopf(Vec::new())
                    }
                }
            };
        }
        Ok(self.sammeln())
    }

    /// The end of the stream (the target closed or the bridge sent `strom.ende`): everything the lock saw goes out,
    /// also an incomplete head and the part of a cut frame.
    ///
    /// # Errors
    /// [`Abriss::Leck`] if a cut control frame holds a secret.
    pub fn ende(&mut self) -> Result<Vec<u8>, Abriss> {
        let rest = self.sperre.ende();
        self.frei.extend(rest);
        match std::mem::replace(&mut self.zustand, Zustand::Kopf(Vec::new())) {
            Zustand::Kopf(k) if !k.is_empty() => self.warte.push_back(Stueck::Roh(k)),
            Zustand::Steuer { kopf_und_nutz, .. } => {
                let kopf = kopflaenge(&kopf_und_nutz)
                    .ok()
                    .flatten()
                    .unwrap_or(2)
                    .min(kopf_und_nutz.len());
                if leck::trifft(self.werte, &kopf_und_nutz[kopf..]) {
                    return Err(Abriss::Leck);
                }
                self.warte.push_back(Stueck::Roh(kopf_und_nutz));
            }
            _ => {}
        }
        Ok(self.sammeln())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rahmen(fin: bool, opcode: u8, nutz: &[u8]) -> Vec<u8> {
        let mut r = vec![if fin { 0x80 } else { 0 } | opcode];
        match nutz.len() {
            n if n < 126 => r.push(u8::try_from(n).unwrap_or(0)),
            n if n < 65536 => {
                r.push(126);
                r.extend_from_slice(&u16::try_from(n).unwrap_or(0).to_be_bytes());
            }
            n => {
                r.push(127);
                r.extend_from_slice(&(n as u64).to_be_bytes());
            }
        }
        r.extend_from_slice(nutz);
        r
    }

    fn alles(werte: &[Geheim], bytes: &[u8], stueck: usize) -> Result<Vec<u8>, Abriss> {
        let mut l = Rahmenleser::neu(werte);
        let mut aus = Vec::new();
        for s in bytes.chunks(stueck.max(1)) {
            aus.extend(l.schieben(s)?);
        }
        aus.extend(l.ende()?);
        Ok(aus)
    }

    fn marke() -> Vec<Geheim> {
        vec![Geheim::neu(b"hgx5-0123456789a".to_vec())]
    }

    #[test]
    fn regeln_l8_a_bis_e() {
        let w = marke();
        let faelle = [
            rahmen(false, 0x9, b"x"),
            rahmen(true, 0xA, &[0u8; 126]),
            rahmen(true, 0x8, &[0u8; 126]),
            rahmen(true, 0x0, b"x"),
            [rahmen(false, 0x1, b"a"), rahmen(true, 0x1, b"b")].concat(),
        ];
        for (i, f) in faelle.iter().enumerate() {
            let mut l = Rahmenleser::neu(&w);
            let mut raus = Vec::new();
            let mut ergebnis = Ok(());
            for s in f.chunks(1) {
                match l.schieben(s) {
                    Ok(b) => raus.extend(b),
                    Err(e) => {
                        ergebnis = Err(e);
                        break;
                    }
                }
            }
            assert_eq!(ergebnis, Err(Abriss::Fehler), "case {i}");
            // nothing of the violating frame went out
            if i == 4 {
                assert!(raus.len() <= 3, "case {i}");
            } else {
                assert!(raus.is_empty(), "case {i}");
            }
        }
    }

    #[test]
    fn regeln_l8_f_und_g_laufen_durch() {
        let w = marke();
        let f = [
            rahmen(false, 0x1, b"abc"),
            rahmen(true, 0x9, b"12345"),
            rahmen(true, 0x0, b"def"),
        ]
        .concat();
        assert_eq!(alles(&w, &f, 1), Ok(f.clone()));
        let g = rahmen(true, 0x9, &[7u8; 125]);
        assert_eq!(alles(&w, &g, 3), Ok(g.clone()));
    }

    #[test]
    fn rsv_maske_opcode_laenge() {
        let w = marke();
        assert_eq!(alles(&w, &[0xC1, 0x00], 1), Err(Abriss::Komprimiert));
        assert_eq!(alles(&w, &[0x81, 0x80], 1), Err(Abriss::Fehler));
        assert_eq!(alles(&w, &[0x83, 0x00], 1), Err(Abriss::Fehler));
        let mut lang = vec![0x82, 127];
        lang.extend_from_slice(&(NUTZ_MAX + 1).to_be_bytes());
        assert_eq!(alles(&w, &lang, 4), Err(Abriss::Fehler));
    }

    #[test]
    fn leck_ueber_zwei_rahmen_mit_ping() {
        let w = marke();
        let f = [
            rahmen(false, 0x1, b"vorne hgx5-0123"),
            rahmen(true, 0x9, b"p"),
            rahmen(true, 0x0, b"456789a hinten"),
        ]
        .concat();
        for stueck in [1, 2, 7, 1000] {
            assert_eq!(alles(&w, &f, stueck), Err(Abriss::Leck), "{stueck}");
        }
        let im_ping = rahmen(true, 0x9, b"hgx5-0123456789a");
        assert_eq!(alles(&w, &im_ping, 5), Err(Abriss::Leck));
    }

    #[test]
    fn laenge_genau_an_der_grenze_und_steuerrahmen_in_stuecken() {
        let w = marke();
        let mut l = Rahmenleser::neu(&w);
        let mut k = vec![0x82, 127];
        k.extend_from_slice(&NUTZ_MAX.to_be_bytes());
        // the head with exactly 32 MiB passes and goes out before the payload
        assert_eq!(l.schieben(&k), Ok(k.clone()));
        // a ping in pieces, then a data frame: the data frame comes out before the end of the stream
        let mut l = Rahmenleser::neu(&w);
        let ping = rahmen(true, 0x9, b"0123456789");
        let mut aus = Vec::new();
        for s in ping.chunks(3) {
            aus.extend(l.schieben(s).expect("ok"));
        }
        assert_eq!(aus, ping, "a complete control frame goes out at once");
        let daten = rahmen(true, 0x2, &[b'q'; 40]);
        aus.extend(l.schieben(&daten).expect("ok"));
        assert_eq!(
            aus.len(),
            ping.len() + daten.len() - 15,
            "only the hold-back of the lock is missing"
        );
        // cut inside a control frame: what came goes out, unless it holds a secret
        let mut l = Rahmenleser::neu(&w);
        let halb = &rahmen(true, 0x9, b"abcdefgh")[..6];
        let mut aus = l.schieben(halb).expect("ok");
        aus.extend(l.ende().expect("ok"));
        assert_eq!(aus, halb);
        let mut l = Rahmenleser::neu(&w);
        let geheim = &rahmen(true, 0xA, b"xhgx5-0123456789ay")[..19];
        assert_eq!(l.schieben(geheim), Ok(Vec::new()));
        assert_eq!(l.ende(), Err(Abriss::Leck));
    }

    #[test]
    fn l9_alles_kommt_am_ende() {
        let w = marke();
        let f = rahmen(true, 0x1, &[b'y'; 40]);
        assert_eq!(alles(&w, &f, 1), Ok(f.clone()));
        // cut in the middle of the next frame: everything sent goes out
        let mut g = f.clone();
        g.extend_from_slice(&rahmen(true, 0x1, &[b'z'; 30])[..10]);
        assert_eq!(alles(&w, &g, 6), Ok(g.clone()));
        // cut in the middle of a head
        let mut h = f.clone();
        h.extend_from_slice(&[0x81, 126, 0]);
        assert_eq!(alles(&w, &h, 2), Ok(h.clone()));
    }
}
