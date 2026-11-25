//! Names and identities in the form of the contract: the channel name, SIDs of accounts and services, package SIDs.
//!
//! Every type checks its form once, at construction; a value of the type always has the form of the schema
//! (`peerkanal.schema.json`, `$defs/kennung`, `sid`, `package_sid`).

use std::fmt;

use crate::fehler::{Code, Fehler};

/// A name in the form `^[a-z0-9][a-z0-9._-]{0,63}$`, as in contracts 1 to 5.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Kennung(String);

/// The name of a channel: a [`Kennung`]. Windows: `\\.\pipe\<name>`.
pub type Kanalname = Kennung;

impl Kennung {
    /// Checks the form.
    ///
    /// # Errors
    /// `pk.vorgabe_ungueltig` when the text is not in the form.
    pub fn neu(text: &str) -> Result<Self, Fehler> {
        if ist_kennung(text) {
            Ok(Self(text.to_owned()))
        } else {
            Err(Fehler::neu(Code::VorgabeUngueltig))
        }
    }

    /// The text.
    #[must_use]
    pub fn als_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Kennung {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

pub(crate) fn ist_kennung(text: &str) -> bool {
    let b = text.as_bytes();
    if b.is_empty() || b.len() > 64 {
        return false;
    }
    if !(b[0].is_ascii_lowercase() || b[0].is_ascii_digit()) {
        return false;
    }
    b[1..].iter().all(|&c| {
        c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'.' || c == b'_' || c == b'-'
    })
}

/// The SID of an account or a service as text: a local or domain account (`S-1-5-21-a-b-c-rid`), Entra ID
/// (`S-1-12-1-a-b-c-d`), a virtual service account (`S-1-5-80-a-b-c-d-e`) or SYSTEM (`S-1-5-18`).
///
/// Group SIDs are no identity: Everyone, Authenticated Users, Users, ALL APPLICATION PACKAGES and the like do not have this form.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Sid(String);

impl Sid {
    /// Checks the form.
    ///
    /// # Errors
    /// `pk.vorgabe_ungueltig` when the text is not the SID of an account or a service.
    pub fn neu(text: &str) -> Result<Self, Fehler> {
        if ist_konto_sid(text) {
            Ok(Self(text.to_owned()))
        } else {
            Err(Fehler::neu(Code::VorgabeUngueltig))
        }
    }

    /// The SID of the virtual service account `NT SERVICE\<name>`: `S-1-5-80-` and the SHA-1 of the name in capitals as
    /// UTF-16LE, read as five 32-bit numbers in little-endian order (the rule of `RtlCreateServiceSid`). The name has the form of
    /// a [`Kennung`]; the value is computed and needs no system call, so it is the same on every system.
    ///
    /// # Errors
    /// `pk.vorgabe_ungueltig` when the name is not in the form of a [`Kennung`].
    pub fn dienst(name: &str) -> Result<Self, Fehler> {
        let name = Kennung::neu(name)?;
        let utf16: Vec<u8> = name
            .als_str()
            .to_ascii_uppercase()
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect();
        let h = sha1(&utf16);
        let teile: Vec<String> = h
            .as_chunks::<4>()
            .0
            .iter()
            .map(|c| u32::from_le_bytes(*c).to_string())
            .collect();
        Ok(Self(format!("S-1-5-80-{}", teile.join("-"))))
    }

    /// The text.
    #[must_use]
    pub fn als_str(&self) -> &str {
        &self.0
    }
}

/// SHA-1 (FIPS 180-4), only for [`Sid::dienst`]: the system derives the SID of a service with it. No secret depends on it.
#[allow(clippy::many_single_char_names)] // the names of FIPS 180-4
fn sha1(daten: &[u8]) -> [u8; 20] {
    let mut h: [u32; 5] = [
        0x6745_2301,
        0xEFCD_AB89,
        0x98BA_DCFE,
        0x1032_5476,
        0xC3D2_E1F0,
    ];
    let mut nachricht = daten.to_vec();
    let bits = (daten.len() as u64).wrapping_mul(8);
    nachricht.push(0x80);
    while nachricht.len() % 64 != 56 {
        nachricht.push(0);
    }
    nachricht.extend_from_slice(&bits.to_be_bytes());
    for block in nachricht.as_chunks::<64>().0 {
        let mut w = [0u32; 80];
        for (i, c) in block.as_chunks::<4>().0.iter().enumerate() {
            w[i] = u32::from_be_bytes(*c);
        }
        for i in 16..80 {
            w[i] = (w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]).rotate_left(1);
        }
        let [mut a, mut b, mut c, mut d, mut e] = h;
        for (i, wi) in w.iter().enumerate() {
            let (f, k) = match i {
                0..=19 => ((b & c) | (!b & d), 0x5A82_7999),
                20..=39 => (b ^ c ^ d, 0x6ED9_EBA1),
                40..=59 => ((b & c) | (b & d) | (c & d), 0x8F1B_BCDC),
                _ => (b ^ c ^ d, 0xCA62_C1D6),
            };
            let t = a
                .rotate_left(5)
                .wrapping_add(f)
                .wrapping_add(e)
                .wrapping_add(k)
                .wrapping_add(*wi);
            e = d;
            d = c;
            c = b.rotate_left(30);
            b = a;
            a = t;
        }
        for (x, y) in h.iter_mut().zip([a, b, c, d, e]) {
            *x = x.wrapping_add(y);
        }
    }
    let mut aus = [0u8; 20];
    for (i, x) in h.iter().enumerate() {
        aus[i * 4..i * 4 + 4].copy_from_slice(&x.to_be_bytes());
    }
    aus
}

impl fmt::Display for Sid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// An `AppContainer` package SID: `S-1-15-2-` with exactly seven sub-authorities. `S-1-15-2-1` (ALL APPLICATION PACKAGES) and
/// `S-1-15-2-2` do not have this form.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PackageSid(String);

impl PackageSid {
    /// Checks the form.
    ///
    /// # Errors
    /// `pk.vorgabe_ungueltig` when the text is not a package SID.
    pub fn neu(text: &str) -> Result<Self, Fehler> {
        if ist_package_sid(text) {
            Ok(Self(text.to_owned()))
        } else {
            Err(Fehler::neu(Code::VorgabeUngueltig))
        }
    }

    /// The text.
    #[must_use]
    pub fn als_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for PackageSid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// `n` groups of 1 to 10 decimal digits, separated by `-`, and nothing else.
fn zahlengruppen(rest: &str, n: usize) -> bool {
    let mut zahl = 0;
    for teil in rest.split('-') {
        if teil.is_empty() || teil.len() > 10 || !teil.bytes().all(|c| c.is_ascii_digit()) {
            return false;
        }
        zahl += 1;
    }
    zahl == n
}

pub(crate) fn ist_konto_sid(text: &str) -> bool {
    if text == "S-1-5-18" {
        return true;
    }
    if let Some(rest) = text.strip_prefix("S-1-5-21-") {
        return zahlengruppen(rest, 4);
    }
    if let Some(rest) = text.strip_prefix("S-1-12-1-") {
        return zahlengruppen(rest, 4);
    }
    if let Some(rest) = text.strip_prefix("S-1-5-80-") {
        return zahlengruppen(rest, 5);
    }
    false
}

pub(crate) fn ist_package_sid(text: &str) -> bool {
    text.strip_prefix("S-1-15-2-")
        .is_some_and(|rest| zahlengruppen(rest, 7))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kennung_form() {
        for gut in [
            "k-beispiel",
            "a",
            "0",
            "wb-such-0123456789abcdef0123456789abcdef",
            "a.b_c-d",
        ] {
            assert!(Kennung::neu(gut).is_ok(), "{gut}");
        }
        let lang = "a".repeat(65);
        for schlecht in [
            "",
            "K-Beispiel",
            "-a",
            ".a",
            "_a",
            "a b",
            "a/b",
            "ä",
            lang.as_str(),
        ] {
            assert!(Kennung::neu(schlecht).is_err(), "{schlecht}");
        }
        assert!(Kennung::neu(&"a".repeat(64)).is_ok());
    }

    #[test]
    fn sid_form() {
        for gut in [
            "S-1-5-21-1000000001-1000000002-1000000003-1001",
            "S-1-12-1-1-2-3-4",
            "S-1-5-80-1000000011-1000000012-1000000013-1000000014-1000000015",
            "S-1-5-18",
        ] {
            assert!(Sid::neu(gut).is_ok(), "{gut}");
        }
        for schlecht in [
            "S-1-1-0",
            "S-1-5-11",
            "S-1-5-32-545",
            "S-1-5-32-544",
            "S-1-5-4",
            "S-1-5-2",
            "S-1-5-7",
            "S-1-15-2-1",
            "S-1-15-2-2",
            "S-1-5-19",
            "S-1-5-21-1-2-3",
            "S-1-5-21-1-2-3-4-5",
            "S-1-5-21-12345678901-2-3-4",
            "S-1-5-21-1-2-3-",
            "S-1-5-21-1-2-3-x",
            "S-1-5-80-1-2-3-4",
            "s-1-5-18",
            "",
        ] {
            assert!(Sid::neu(schlecht).is_err(), "{schlecht}");
        }
    }

    #[test]
    fn sha1_bekannte_werte() {
        let hex = |b: [u8; 20]| {
            b.iter().fold(String::new(), |mut t, x| {
                use std::fmt::Write;
                let _ = write!(t, "{x:02x}");
                t
            })
        };
        assert_eq!(hex(sha1(b"")), "da39a3ee5e6b4b0d3255bfef95601890afd80709");
        assert_eq!(
            hex(sha1(b"abc")),
            "a9993e364706816aba3e25717850c26c9cd0d89d"
        );
        // Two blocks (56 bytes and more).
        assert_eq!(
            hex(sha1(
                b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"
            )),
            "84983e441c3bd26ebaae4aa1f95129e5e54670f1"
        );
    }

    /// The service SID of `TrustedInstaller` is documented and the same on every Windows; the name counts in capitals, so the
    /// form of a `Kennung` gives the same value.
    #[test]
    fn dienst_sid() {
        assert_eq!(
            Sid::dienst("trustedinstaller").expect("form").als_str(),
            "S-1-5-80-956008885-3418522649-1831038044-1853292631-2271478464"
        );
        // Value of Python `hashlib` over "WB-PLATTFORM" as UTF-16LE (and `sc showsid wb-plattform` in the VM).
        let wb = Sid::dienst("wb-plattform").expect("form");
        assert_eq!(
            wb.als_str(),
            "S-1-5-80-3008460381-3285883375-3225195253-229837603-2978416923"
        );
        assert!(ist_konto_sid(wb.als_str()), "{wb}");
        assert_ne!(wb, Sid::dienst("wb-plattform2").expect("form"));
        for schlecht in ["", "TrustedInstaller", "a b", "-a", "ä"] {
            assert_eq!(
                Sid::dienst(schlecht).err(),
                Some(Fehler::neu(Code::VorgabeUngueltig)),
                "{schlecht}"
            );
        }
    }

    #[test]
    fn package_sid_form() {
        assert!(PackageSid::neu("S-1-15-2-1-2-3-4-5-6-7").is_ok());
        for schlecht in [
            "S-1-15-2-1",
            "S-1-15-2-2",
            "S-1-15-2-1-2-3-4-5-6",
            "S-1-15-2-1-2-3-4-5-6-7-8",
            "S-1-15-3-1-2-3-4-5-6-7",
        ] {
            assert!(PackageSid::neu(schlecht).is_err(), "{schlecht}");
        }
    }
}
