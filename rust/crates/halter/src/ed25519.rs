//! Check of an Ed25519 signature after RFC 8032 (section 5.1.7), only the check: the holder never signs, holds no
//! private key of the point and so needs no constant time. Field elements mod 2^255 - 19 in five limbs of 51 bits,
//! points in extended coordinates with the complete addition of section 5.1.4.
//!
//! The check is the one of section 5.1.7 without the factor 8: `[S]B = R + [k]A` compared as encodings. `S` must be
//! below the group order L, `A` and `R` must decode (y below p, a square root exists, no "-0").

#![allow(
    clippy::many_single_char_names,
    clippy::needless_range_loop,
    reason = "the names of RFC 8032 (x, y, z, t, a to h, d, k) keep the formulas checkable against the standard"
)]

use crate::sha512::sha512;

const MASKE: u64 = (1 << 51) - 1;

#[derive(Clone, Copy)]
struct Fe([u64; 5]);

impl Fe {
    const NULL: Fe = Fe([0; 5]);
    const EINS: Fe = Fe([1, 0, 0, 0, 0]);

    fn klein(n: u64) -> Fe {
        Fe([n, 0, 0, 0, 0])
    }

    /// The low 255 bits of 32 bytes (little endian).
    fn aus_bytes(b: &[u8; 32]) -> Fe {
        let mut w = [0u64; 4];
        for (i, teil) in b.as_chunks::<8>().0.iter().enumerate() {
            w[i] = u64::from_le_bytes(*teil);
        }
        Fe([
            w[0] & MASKE,
            ((w[0] >> 51) | (w[1] << 13)) & MASKE,
            ((w[1] >> 38) | (w[2] << 26)) & MASKE,
            ((w[2] >> 25) | (w[3] << 39)) & MASKE,
            (w[3] >> 12) & MASKE,
        ])
    }

    fn tragen(mut self) -> Fe {
        for _ in 0..2 {
            for i in 0..4 {
                self.0[i + 1] += self.0[i] >> 51;
                self.0[i] &= MASKE;
            }
            self.0[0] += 19 * (self.0[4] >> 51);
            self.0[4] &= MASKE;
        }
        self
    }

    fn plus(self, o: Fe) -> Fe {
        let mut r = [0u64; 5];
        for i in 0..5 {
            r[i] = self.0[i] + o.0[i];
        }
        Fe(r).tragen()
    }

    fn minus(self, o: Fe) -> Fe {
        // + 4p, so that no limb goes below zero (every limb of `o` is below 2^52 after `tragen`)
        const VIER_P: [u64; 5] = [
            4 * ((1 << 51) - 19),
            4 * MASKE,
            4 * MASKE,
            4 * MASKE,
            4 * MASKE,
        ];
        let mut r = [0u64; 5];
        for i in 0..5 {
            r[i] = self.0[i] + VIER_P[i] - o.0[i];
        }
        Fe(r).tragen()
    }

    fn mal(self, o: Fe) -> Fe {
        let a = self.0.map(u128::from);
        let b = o.0.map(u128::from);
        let b19 = b.map(|x| x * 19);
        let r0 = a[0] * b[0] + a[1] * b19[4] + a[2] * b19[3] + a[3] * b19[2] + a[4] * b19[1];
        let r1 = a[0] * b[1] + a[1] * b[0] + a[2] * b19[4] + a[3] * b19[3] + a[4] * b19[2];
        let r2 = a[0] * b[2] + a[1] * b[1] + a[2] * b[0] + a[3] * b19[4] + a[4] * b19[3];
        let r3 = a[0] * b[3] + a[1] * b[2] + a[2] * b[1] + a[3] * b[0] + a[4] * b19[4];
        let r4 = a[0] * b[4] + a[1] * b[3] + a[2] * b[2] + a[3] * b[1] + a[4] * b[0];
        let mut r = [r0, r1, r2, r3, r4];
        let m = u128::from(MASKE);
        for i in 0..4 {
            r[i + 1] += r[i] >> 51;
            r[i] &= m;
        }
        r[0] += 19 * (r[4] >> 51);
        r[4] &= m;
        // every limb now fits into 64 bits; the cast keeps the value
        Fe(r.map(|x| u64::try_from(x).unwrap_or(u64::MAX))).tragen()
    }

    /// `self` to the power of the 256-bit exponent `e` (little endian).
    fn hoch(self, e: &[u8; 32]) -> Fe {
        let mut r = Fe::EINS;
        for byte in e.iter().rev() {
            for bit in (0..8).rev() {
                r = r.mal(r);
                if (byte >> bit) & 1 == 1 {
                    r = r.mal(self);
                }
            }
        }
        r
    }

    fn kehrwert(self) -> Fe {
        // p - 2 = 2^255 - 21
        let mut e = [0xff; 32];
        e[0] = 0xeb;
        e[31] = 0x7f;
        self.hoch(&e)
    }

    fn zu_bytes(self) -> [u8; 32] {
        let mut h = self.tragen().0;
        while h.iter().any(|x| x >> 51 != 0) {
            h = Fe(h).tragen().0;
        }
        // now h < 2^255; subtract p once if h >= p, i.e. if h + 19 reaches 2^255
        let mut t = h;
        t[0] += 19;
        for i in 0..4 {
            t[i + 1] += t[i] >> 51;
            t[i] &= MASKE;
        }
        if t[4] >> 51 != 0 {
            t[4] &= MASKE;
            h = t;
        }
        let w0 = h[0] | (h[1] << 51);
        let w1 = (h[1] >> 13) | (h[2] << 38);
        let w2 = (h[2] >> 26) | (h[3] << 25);
        let w3 = (h[3] >> 39) | (h[4] << 12);
        let mut aus = [0u8; 32];
        for (i, w) in [w0, w1, w2, w3].iter().enumerate() {
            aus[i * 8..i * 8 + 8].copy_from_slice(&w.to_le_bytes());
        }
        aus
    }

    fn gleich(self, o: Fe) -> bool {
        self.zu_bytes() == o.zu_bytes()
    }

    fn ist_null(self) -> bool {
        self.gleich(Fe::NULL)
    }

    fn ungerade(self) -> bool {
        self.zu_bytes()[0] & 1 == 1
    }

    fn neg(self) -> Fe {
        Fe::NULL.minus(self)
    }
}

struct Konstanten {
    d: Fe,
    zwei_d: Fe,
    wurzel_minus_eins: Fe,
}

fn konstanten() -> Konstanten {
    let d = Fe::klein(121_665).neg().mal(Fe::klein(121_666).kehrwert());
    // sqrt(-1) = 2^((p-1)/4), (p-1)/4 = 2^253 - 5
    let mut e = [0xff; 32];
    e[0] = 0xfb;
    e[31] = 0x1f;
    Konstanten {
        d,
        zwei_d: d.plus(d),
        wurzel_minus_eins: Fe::klein(2).hoch(&e),
    }
}

#[derive(Clone, Copy)]
struct Punkt {
    x: Fe,
    y: Fe,
    z: Fe,
    t: Fe,
}

impl Punkt {
    const NEUTRAL: Punkt = Punkt {
        x: Fe::NULL,
        y: Fe::EINS,
        z: Fe::EINS,
        t: Fe::NULL,
    };

    fn plus(self, q: Punkt, k: &Konstanten) -> Punkt {
        let a = self.y.minus(self.x).mal(q.y.minus(q.x));
        let b = self.y.plus(self.x).mal(q.y.plus(q.x));
        let c = self.t.mal(k.zwei_d).mal(q.t);
        let d = self.z.plus(self.z).mal(q.z);
        let e = b.minus(a);
        let f = d.minus(c);
        let g = d.plus(c);
        let h = b.plus(a);
        Punkt {
            x: e.mal(f),
            y: g.mal(h),
            t: e.mal(h),
            z: f.mal(g),
        }
    }

    fn mal(self, skalar: &[u8; 32], k: &Konstanten) -> Punkt {
        let mut r = Punkt::NEUTRAL;
        for byte in skalar.iter().rev() {
            for bit in (0..8).rev() {
                r = r.plus(r, k);
                if (byte >> bit) & 1 == 1 {
                    r = r.plus(self, k);
                }
            }
        }
        r
    }

    fn neg(self) -> Punkt {
        Punkt {
            x: self.x.neg(),
            y: self.y,
            z: self.z,
            t: self.t.neg(),
        }
    }

    fn kodieren(self) -> [u8; 32] {
        let zi = self.z.kehrwert();
        let x = self.x.mal(zi);
        let mut aus = self.y.mal(zi).zu_bytes();
        if x.ungerade() {
            aus[31] |= 0x80;
        }
        aus
    }

    /// Section 5.1.3.
    fn dekodieren(b: &[u8; 32], k: &Konstanten) -> Option<Punkt> {
        let vorzeichen = b[31] >> 7 == 1;
        let mut yb = *b;
        yb[31] &= 0x7f;
        let y = Fe::aus_bytes(&yb);
        if y.zu_bytes() != yb {
            return None; // y >= p
        }
        let yy = y.mal(y);
        let u = yy.minus(Fe::EINS);
        let v = k.d.mal(yy).plus(Fe::EINS);
        // x = u v^3 (u v^7)^((p-5)/8), (p-5)/8 = 2^252 - 3
        let mut e = [0xff; 32];
        e[0] = 0xfd;
        e[31] = 0x0f;
        let v3 = v.mal(v).mal(v);
        let v7 = v3.mal(v3).mal(v);
        let mut x = u.mal(v3).mal(u.mal(v7).hoch(&e));
        let vxx = v.mal(x).mal(x);
        if vxx.gleich(u) {
        } else if vxx.gleich(u.neg()) {
            x = x.mal(k.wurzel_minus_eins);
        } else {
            return None;
        }
        if x.ist_null() && vorzeichen {
            return None;
        }
        if x.ungerade() != vorzeichen {
            x = x.neg();
        }
        Some(Punkt {
            x,
            y,
            z: Fe::EINS,
            t: x.mal(y),
        })
    }
}

/// The group order L in four words, little endian.
const L: [u64; 4] = [
    0x5812_631a_5cf5_d3ed,
    0x14de_f9de_a2f7_9cd6,
    0,
    0x1000_0000_0000_0000,
];

fn ist_kleiner_l(w: &[u64; 4]) -> bool {
    for i in (0..4).rev() {
        if w[i] != L[i] {
            return w[i] < L[i];
        }
    }
    false
}

fn minus_l(w: &mut [u64; 4]) {
    let mut borgen = 0u64;
    for i in 0..4 {
        let (a, b1) = w[i].overflowing_sub(L[i]);
        let (b, b2) = a.overflowing_sub(borgen);
        w[i] = b;
        borgen = u64::from(b1 || b2);
    }
}

fn woerter(b: &[u8; 32]) -> [u64; 4] {
    let mut w = [0u64; 4];
    for (i, teil) in b.as_chunks::<8>().0.iter().enumerate() {
        w[i] = u64::from_le_bytes(*teil);
    }
    w
}

/// A 512-bit number (little endian) mod L, as 32 bytes little endian.
fn mod_l(zahl: &[u8; 64]) -> [u8; 32] {
    // shift in bit by bit from the top; the rest stays below L < 2^253, so 2 r + 1 fits in 256 bits
    let mut r = [0u64; 4];
    for byte in zahl.iter().rev() {
        for bit in (0..8).rev() {
            let mut uebertrag = u64::from((byte >> bit) & 1);
            for wort in &mut r {
                let neu = (*wort << 1) | uebertrag;
                uebertrag = *wort >> 63;
                *wort = neu;
            }
            if !ist_kleiner_l(&r) {
                minus_l(&mut r);
            }
        }
    }
    let mut aus = [0u8; 32];
    for (i, w) in r.iter().enumerate() {
        aus[i * 8..i * 8 + 8].copy_from_slice(&w.to_le_bytes());
    }
    aus
}

/// `true` if `signatur` (64 bytes) is a valid Ed25519 signature of `nachricht` under the public key `oeffentlich`.
#[must_use]
pub fn pruefen(oeffentlich: &[u8; 32], nachricht: &[u8], signatur: &[u8; 64]) -> bool {
    let k = konstanten();
    let Some(a) = Punkt::dekodieren(oeffentlich, &k) else {
        return false;
    };
    let mut r_bytes = [0u8; 32];
    r_bytes.copy_from_slice(&signatur[..32]);
    let mut s = [0u8; 32];
    s.copy_from_slice(&signatur[32..]);
    if !ist_kleiner_l(&woerter(&s)) {
        return false;
    }
    if Punkt::dekodieren(&r_bytes, &k).is_none() {
        return false;
    }
    let h = mod_l(&sha512(&[&r_bytes, oeffentlich, nachricht]));
    let mut basis_bytes = [0x66u8; 32];
    basis_bytes[0] = 0x58;
    let Some(basis) = Punkt::dekodieren(&basis_bytes, &k) else {
        return false;
    };
    let links = basis.mal(&s, &k);
    let rechts_ohne_r = a.neg().mal(&h, &k);
    // [S]B - [k]A must be R
    links.plus(rechts_ohne_r, &k).kodieren() == r_bytes
}

#[cfg(test)]
mod tests {
    use super::pruefen;

    fn bytes<const N: usize>(hex: &str) -> [u8; N] {
        let mut aus = [0u8; N];
        for (i, o) in aus.iter_mut().enumerate() {
            *o = u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).expect("hex");
        }
        aus
    }

    #[test]
    fn rfc8032_test_1_und_2() {
        let a1 = bytes::<32>("d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a");
        let s1 = bytes::<64>(
            "e5564300c360ac729086e2cc806e828a84877f1eb8e5d974d873e065224901555fb8821590a33bacc61e39701cf9b46bd25bf5f0595bbe24655141438e7a100b",
        );
        assert!(pruefen(&a1, b"", &s1));
        assert!(!pruefen(&a1, b"x", &s1));
        let a2 = bytes::<32>("3d4017c3e843895a92b70aa74d1b7ebc9c982ccf2ec4968cc0cd55f12af4660c");
        let s2 = bytes::<64>(
            "92a009a9f0d4cab8720e820b5f642540a2b27b5416503f8fb3762223ebdb69da085ac1e43e15996e458f3613d0f11d8c387b2eaeb4302aeeb00d291612bb0c00",
        );
        assert!(pruefen(&a2, &[0x72], &s2));
        let mut falsch = s2;
        falsch[10] ^= 1;
        assert!(!pruefen(&a2, &[0x72], &falsch));
        assert!(!pruefen(&a1, &[0x72], &s2));
    }

    #[test]
    fn s_nicht_unter_l_ist_ungueltig() {
        let a1 = bytes::<32>("d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a");
        let mut s1 = bytes::<64>(
            "e5564300c360ac729086e2cc806e828a84877f1eb8e5d974d873e065224901555fb8821590a33bacc61e39701cf9b46bd25bf5f0595bbe24655141438e7a100b",
        );
        // S + L is the same scalar but not canonical
        let l = bytes::<32>("edd3f55c1a631258d69cf7a2def9de1400000000000000000000000000000010");
        let mut uebertrag = 0u16;
        for i in 0..32 {
            let summe = u16::from(s1[32 + i]) + u16::from(l[i]) + uebertrag;
            s1[32 + i] = (summe & 0xff) as u8;
            uebertrag = summe >> 8;
        }
        assert!(!pruefen(&a1, b"", &s1));
    }
}
