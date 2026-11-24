//! Reader of the settings file of contract 2: strict JSON, closed form, the stages `einst.*` and a
//! typed view of the result.
//!
//! [`lesen`] runs the stages 1 to 34 in the order of the contract and returns the code of the first
//! stage that the file violates. The typed view [`Einstellungen`] borrows nothing and copies nothing
//! out of the reader; the rule checker `datentor` reads it through the accessors.
//!
//! The crate reads bytes and never touches the file system. The strict JSON reader is public, because
//! contract 1 (the start structure) uses the same JSON rules.

mod aufbau;
mod fehler;
pub mod json;
mod netz;
mod typen;

pub use aufbau::lesen;
pub use fehler::Fehler;
pub use typen::{
    Einstellungen, Freigabe, Kunde, KundendatenAnCloud, McpServer, Netzziel, PfadRegel, Route,
    Routenart, Schema, Werkzeug, Ziel, Zugriff,
};

/// Tells whether `text` has the form of a key (contract 1 and 2): one to 64 characters of `a-z`,
/// `0-9`, `.`, `_` and `-`, the first character a letter or digit.
#[must_use]
pub fn ist_kennung(text: &str) -> bool {
    let bytes = text.as_bytes();
    bytes.len() <= 64
        && bytes
            .first()
            .is_some_and(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        && bytes.iter().all(|c| {
            c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, b'.' | b'_' | b'-')
        })
}

#[cfg(test)]
mod tests {
    use super::ist_kennung;

    #[test]
    fn kennung_form_follows_the_contract() {
        assert!(ist_kennung("a"));
        assert!(ist_kennung("0.-_z"));
        assert!(ist_kennung(&"a".repeat(64)));
        assert!(!ist_kennung(""));
        assert!(!ist_kennung(&"a".repeat(65)));
        assert!(!ist_kennung(".a"));
        assert!(!ist_kennung("-a"));
        assert!(!ist_kennung("A"));
        assert!(!ist_kennung("a b"));
        assert!(!ist_kennung("ä"));
    }
}
