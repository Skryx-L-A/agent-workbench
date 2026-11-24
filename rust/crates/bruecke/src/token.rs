//! The token (BR2): randomness from `/dev/urandom`, the hex form and the comparison in constant time.

use std::fmt::Write as _;
use std::fs::File;
use std::io::{self, Read};

/// The token has 32 random bytes, written as 64 lower-case hex characters.
pub(crate) const TOKEN_BYTES: usize = 32;
/// The identifier of a stream has 16 random bytes, written as 32 lower-case hex characters.
pub(crate) const KENNUNG_BYTES: usize = 16;

/// Compares two byte strings in constant time and counts the positions it read. The loop runs over EVERY position up to
/// the longer of the two, never stops early, and folds the difference of the lengths into the result. It uses no `==` on
/// byte sequences.
pub(crate) fn token_gleich_gezaehlt(vorgelegt: &[u8], eigen: &[u8]) -> (bool, usize) {
    let laenge = vorgelegt.len().max(eigen.len());
    let mut unterschied = vorgelegt.len() ^ eigen.len();
    let mut gelesen = 0;
    for i in 0..laenge {
        let links = vorgelegt.get(i).copied().unwrap_or(0);
        let rechts = eigen.get(i).copied().unwrap_or(0);
        unterschied |= usize::from(links ^ rechts);
        gelesen += 1;
    }
    (unterschied == 0, gelesen)
}

/// The truth of [`token_gleich_gezaehlt`].
pub(crate) fn token_gleich(a: &[u8], b: &[u8]) -> bool {
    token_gleich_gezaehlt(a, b).0
}

/// Lower-case hex of the bytes.
pub(crate) fn hex(bytes: &[u8]) -> String {
    bytes.iter().fold(String::new(), |mut s, b| {
        let _ = write!(s, "{b:02x}");
        s
    })
}

/// Reads exactly `n` bytes from `quelle`; a short source is an error.
pub(crate) fn zufall_aus(quelle: &mut impl Read, n: usize) -> io::Result<Vec<u8>> {
    let mut puffer = vec![0u8; n];
    quelle.read_exact(&mut puffer)?;
    Ok(puffer)
}

/// `n` bytes from `/dev/urandom`.
pub(crate) fn zufall(n: usize) -> io::Result<Vec<u8>> {
    zufall_aus(&mut File::open("/dev/urandom")?, n)
}

/// A new token: 64 lower-case hex characters (ASCII bytes).
pub(crate) fn token_erzeugen() -> io::Result<Vec<u8>> {
    Ok(hex(&zufall(TOKEN_BYTES)?).into_bytes())
}

/// A new identifier of a stream: 32 lower-case hex characters.
pub(crate) fn kennung_erzeugen() -> io::Result<String> {
    Ok(hex(&zufall(KENNUNG_BYTES)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn a64() -> Vec<u8> {
        vec![b'a'; 64]
    }

    /// The nine pairs of the self-test of the reference (`bruecke_ref.py --selbsttest`).
    #[test]
    fn nine_pairs_truth_and_every_position_read() {
        let mut erster_anders = vec![b'b'];
        erster_anders.extend(vec![b'a'; 63]);
        let mut letzter_anders = vec![b'a'; 63];
        letzter_anders.push(b'b');
        let paare: Vec<(Vec<u8>, Vec<u8>, bool)> = vec![
            (a64(), a64(), true),
            (a64(), erster_anders, false),
            (a64(), letzter_anders, false),
            (a64(), vec![b'a'; 63], false),
            (a64(), vec![b'a'; 65], false),
            (Vec::new(), Vec::new(), true),
            (Vec::new(), b"x".to_vec(), false),
            (b"abc".to_vec(), Vec::new(), false),
            (a64(), vec![0u8; 64], false),
        ];
        for (a, b, soll) in paare {
            let (wahr, gelesen) = token_gleich_gezaehlt(&a, &b);
            assert_eq!(
                wahr,
                soll,
                "truth of {:?} against {:?}",
                &a[..a.len().min(4)],
                &b[..b.len().min(4)]
            );
            assert_eq!(gelesen, a.len().max(b.len()), "positions read");
            assert_eq!(token_gleich(&a, &b), soll);
        }
    }

    #[test]
    fn differences_do_not_cancel_each_other() {
        // 'a' ^ 'b' == 'b' ^ 'a': an xor-accumulator would call these equal.
        assert!(!token_gleich(b"ab", b"ba"));
        assert!(!token_gleich(&[1, 2, 3], &[2, 1, 3]));
    }

    #[test]
    fn a_length_difference_alone_counts() {
        // the longer value ends in zero bytes, which read as the padding
        assert!(!token_gleich(b"abc", b"abc\0"));
        assert!(!token_gleich(b"abc\0\0", b"abc"));
        assert!(!token_gleich(b"abc\0\0\0", b"abc\0\0"));
        assert!(!token_gleich(b"", b"\0"));
        assert!(token_gleich(b"abc\0", b"abc\0"));
    }

    #[test]
    fn any_single_differing_bit_is_found() {
        let a = vec![0x55u8; 8];
        for i in 0..8 {
            for bit in 0..8 {
                let mut b = a.clone();
                b[i] ^= 1 << bit;
                assert!(!token_gleich(&a, &b), "byte {i} bit {bit}");
            }
        }
    }

    #[test]
    fn hex_is_lower_case_with_two_digits() {
        assert_eq!(hex(&[0, 1, 0xab, 0xff]), "0001abff");
        assert_eq!(hex(&[]), "");
    }

    #[test]
    fn a_short_source_is_an_error() {
        let mut kurz: &[u8] = &[1, 2, 3];
        assert!(zufall_aus(&mut kurz, 4).is_err());
        let mut genug: &[u8] = &[1, 2, 3, 4];
        assert_eq!(zufall_aus(&mut genug, 3).ok(), Some(vec![1, 2, 3]));
    }

    #[test]
    fn the_token_has_the_form_and_is_new_each_time() {
        let t = token_erzeugen().expect("urandom is readable");
        assert_eq!(t.len(), 64);
        assert!(t.iter().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')));
        let mut verschieden: Vec<u8> = t.clone();
        verschieden.sort_unstable();
        verschieden.dedup();
        assert!(verschieden.len() >= 8, "at least 8 different characters");
        assert_ne!(t, token_erzeugen().expect("urandom is readable"));
    }

    #[test]
    fn the_identifier_has_32_hex_characters_and_is_not_a_counter() {
        let a = kennung_erzeugen().expect("urandom is readable");
        let b = kennung_erzeugen().expect("urandom is readable");
        assert_eq!(a.len(), 32);
        assert!(a.bytes().all(|c| matches!(c, b'0'..=b'9' | b'a'..=b'f')));
        assert_ne!(a, b);
        assert_eq!(zufall(5).expect("urandom").len(), 5);
        assert_eq!(zufall(0).expect("urandom").len(), 0);
    }
}
