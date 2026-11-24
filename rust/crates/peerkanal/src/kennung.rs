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

    /// The text.
    #[must_use]
    pub fn als_str(&self) -> &str {
        &self.0
    }
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
