//! The leak lock (contract 8, section 7): the bytes of a key never go to the harness, also not across the border of two
//! parts. The lock holds back the last (length of the longest key - 1) bytes until the next part or the end comes.

use crate::http::finde;

/// The lock for one answer.
pub(crate) struct Sperre<'a> {
    schluessel: Vec<&'a [u8]>,
    halten: usize,
    gehalten: Vec<u8>,
}

impl<'a> Sperre<'a> {
    /// A lock for keys of at least one byte each.
    pub(crate) fn neu(schluessel: &[&'a [u8]]) -> Self {
        let laengste = schluessel.iter().map(|s| s.len()).max().unwrap_or(1);
        Self {
            schluessel: schluessel.to_vec(),
            halten: laengste.saturating_sub(1),
            gehalten: Vec::new(),
        }
    }

    /// Takes the next part. Returns the bytes that may go to the harness now, or `None` if the key is found in the held
    /// bytes together with this part.
    pub(crate) fn teil(&mut self, teil: &[u8]) -> Option<Vec<u8>> {
        let mut puffer = std::mem::take(&mut self.gehalten);
        puffer.extend_from_slice(teil);
        if self.schluessel.iter().any(|s| enthaelt(&puffer, s)) {
            return None;
        }
        let frei = puffer.len().saturating_sub(self.halten);
        self.gehalten = puffer.split_off(frei);
        Some(puffer)
    }

    /// The end of the answer: the held bytes may go now (they hold no whole key).
    pub(crate) fn ende(self) -> Vec<u8> {
        self.gehalten
    }
}

/// Whether `heu` holds the bytes of `schluessel`.
pub(crate) fn enthaelt(heu: &[u8], schluessel: &[u8]) -> bool {
    finde(heu, schluessel).is_some()
}

#[cfg(test)]
mod tests {
    use super::Sperre;

    #[test]
    fn the_last_bytes_wait_for_the_next_part() {
        let mut s = Sperre::neu(&[b"KEY"]);
        assert_eq!(s.teil(b"abcd"), Some(b"ab".to_vec()));
        assert_eq!(s.teil(b"e"), Some(b"c".to_vec()));
        assert_eq!(s.teil(b""), Some(Vec::new()));
        assert_eq!(s.ende(), b"de".to_vec());
    }

    #[test]
    fn a_key_across_parts_is_found() {
        let mut s = Sperre::neu(&[b"KEY"]);
        assert_eq!(s.teil(b"xK"), Some(Vec::new()));
        assert_eq!(s.teil(b"E"), Some(b"x".to_vec()));
        assert_eq!(s.teil(b"Yz"), None);
        let mut s = Sperre::neu(&[b"KEY"]);
        assert_eq!(s.teil(b"aKEYb"), None);
    }

    #[test]
    fn the_longest_of_several_keys_sets_what_is_held() {
        let mut s = Sperre::neu(&[b"AB", b"KEYS"]);
        assert_eq!(s.teil(b"xyzw"), Some(b"x".to_vec()));
        assert_eq!(s.teil(b"A"), Some(b"y".to_vec()));
        assert_eq!(s.teil(b"B"), None);
        let mut s = Sperre::neu(&[b"KEYS", b"AB"]);
        assert_eq!(s.teil(b"xyzw"), Some(b"x".to_vec()));
        assert_eq!(s.teil(b"xKEYS"), None);
    }

    #[test]
    fn a_key_of_one_byte_holds_nothing_back() {
        let mut s = Sperre::neu(&[b"K"]);
        assert_eq!(s.teil(b"ab"), Some(b"ab".to_vec()));
        assert_eq!(s.teil(b"K"), None);
    }
}
