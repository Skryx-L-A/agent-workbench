//! The proof of the human (contract 6, section 6): the interface [`Nachweispruefer`] and the checker `h1`
//! (HMAC-SHA256 with a shared key of 32 bytes over the pseudonym, LF and the challenge).

use std::fmt;

use formate::kette::{kanonisch, sha256_hex};
use serde_json::json;

use crate::form;

/// Checks the proof of a human for one challenge.
pub trait Nachweispruefer: Send + Sync {
    /// `Ok(pseudonym)` when the proof holds for exactly these challenge bytes, otherwise `Err(())`.
    /// The core calls it once per `mcp.entscheiden` and treats any value that is no pseudonym as `Err`.
    ///
    /// # Errors
    /// `Err(())` when the proof does not hold.
    #[allow(
        clippy::result_unit_err,
        reason = "contract 6, section 1, fixes this signature: a refusal carries no information"
    )]
    fn pruefe(&self, herausforderung: &[u8], nachweis: &str) -> Result<String, ()>;
}

/// Block size of SHA-256 in bytes.
const BLOCK: usize = 64;

/// The value of one lower-case hex digit.
fn nibble(c: u8) -> u8 {
    match c {
        b'a'..=b'f' => c - b'a' + 10,
        _ => c.wrapping_sub(b'0'),
    }
}

/// 64 lower-case hex digits as 32 bytes; the caller checks the form first.
fn hex_bytes(text: &str) -> [u8; 32] {
    let mut aus = [0u8; 32];
    for (ziel, paar) in aus.iter_mut().zip(text.as_bytes().as_chunks::<2>().0) {
        *ziel = nibble(paar[0]) * 16 + nibble(paar[1]);
    }
    aus
}

/// SHA-256 of the bytes, as bytes (built on `formate::kette::sha256_hex`).
fn sha256(daten: &[u8]) -> [u8; 32] {
    hex_bytes(&sha256_hex(daten))
}

/// HMAC-SHA256 (RFC 2104) with a key of 32 bytes.
pub(crate) fn hmac_sha256(schluessel: &[u8; 32], nachricht: &[u8]) -> [u8; 32] {
    let mut innen = [0x36u8; BLOCK];
    let mut aussen = [0x5cu8; BLOCK];
    for (stelle, byte) in schluessel.iter().enumerate() {
        innen[stelle] ^= byte;
        aussen[stelle] ^= byte;
    }
    let mut erster = innen.to_vec();
    erster.extend_from_slice(nachricht);
    let mut zweiter = aussen.to_vec();
    zweiter.extend_from_slice(&sha256(&erster));
    sha256(&zweiter)
}

/// Equality of two MACs in constant time: every byte is looked at, whatever the first difference.
pub(crate) fn gleich_konstant(a: &[u8; 32], b: &[u8; 32]) -> bool {
    a.iter().zip(b).fold(0u8, |summe, (x, y)| summe | (x ^ y)) == 0
}

/// The challenge of the human (contract 6, section 6) as bytes: RFC 8785 of `{"anfrage", "art": "mcp.entscheidung", "digest",
/// "entscheidung", "vorschau_sha256"}`. The point builds it from its own state; the service of the human from `mcp.offene`.
#[must_use]
pub fn herausforderung(
    anfrage: &str,
    digest: &str,
    entscheidung: &str,
    vorschau_sha256: Option<&str>,
) -> Vec<u8> {
    let wert = json!({
        "anfrage": anfrage,
        "art": "mcp.entscheidung",
        "digest": digest,
        "entscheidung": entscheidung,
        "vorschau_sha256": vorschau_sha256,
    });
    kanonisch(&wert).unwrap_or_default().into_bytes()
}

/// The proof `h1:<pseudonym>:<64 hex>` for a challenge: HMAC-SHA256 with the key over the pseudonym, LF and the challenge. The
/// service of the human issues it (addendum of contract `dienst`, channel to the point); [`PrueferH1`] checks it.
#[must_use]
pub fn nachweis_h1(schluessel: &[u8; 32], pseudonym: &str, herausforderung: &[u8]) -> String {
    const ZIFFERN: &[u8; 16] = b"0123456789abcdef";
    let mut nachricht = format!("{pseudonym}\n").into_bytes();
    nachricht.extend_from_slice(herausforderung);
    let mac: String = hmac_sha256(schluessel, &nachricht)
        .iter()
        .flat_map(|b| [ZIFFERN[usize::from(b >> 4)], ZIFFERN[usize::from(b & 15)]])
        .map(char::from)
        .collect();
    format!("h1:{pseudonym}:{mac}")
}

/// The checker `h1`: the proof is the text `h1:<pseudonym>:<64 hex>`.
pub struct PrueferH1 {
    schluessel: [u8; 32],
}

impl PrueferH1 {
    /// A checker with this key. The key comes from the installation configuration, never from the source.
    #[must_use]
    pub fn neu(schluessel: [u8; 32]) -> Self {
        Self { schluessel }
    }
}

impl fmt::Debug for PrueferH1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("PrueferH1 { schluessel: <verdeckt> }")
    }
}

impl Nachweispruefer for PrueferH1 {
    fn pruefe(&self, herausforderung: &[u8], nachweis: &str) -> Result<String, ()> {
        let rest = nachweis.strip_prefix("h1:").ok_or(())?;
        let (person, mac) = rest.split_once(':').ok_or(())?;
        if !form::pseudonym(person) || !form::hex(mac, 64) {
            return Err(());
        }
        let mut nachricht = format!("{person}\n").into_bytes();
        nachricht.extend_from_slice(herausforderung);
        if gleich_konstant(&hmac_sha256(&self.schluessel, &nachricht), &hex_bytes(mac)) {
            Ok(person.to_owned())
        } else {
            Err(())
        }
    }
}

#[cfg(test)]
mod tests;
