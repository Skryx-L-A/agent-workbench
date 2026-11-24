//! Base64: the vectors of RFC 4648, strict padding, round trip.

use crate::base64::{dekodieren, kodieren};

const VEKTOREN: &[(&[u8], &str)] = &[
    (b"", ""),
    (b"f", "Zg=="),
    (b"fo", "Zm8="),
    (b"foo", "Zm9v"),
    (b"foob", "Zm9vYg=="),
    (b"fooba", "Zm9vYmE="),
    (b"foobar", "Zm9vYmFy"),
    (&[0xfb, 0xff, 0xbf], "+/+/"),
    (&[0x00], "AA=="),
    (&[0xff, 0xff], "//8="),
];

#[test]
fn the_rfc_vectors_encode_and_decode() {
    for (roh, text) in VEKTOREN {
        assert_eq!(kodieren(roh), *text);
        assert_eq!(dekodieren(text).as_deref(), Some(*roh), "{text}");
    }
}

#[test]
fn padding_must_complete_the_group() {
    for falsch in [
        "Zg", "Zg=", "Zg===", "Zm8", "Zm8==", "Zm9v=", "Zm9v==", "Z", "Z=", "Z===", "=", "==",
        "====",
    ] {
        assert_eq!(dekodieren(falsch), None, "{falsch}");
    }
}

#[test]
fn other_characters_are_invalid() {
    for falsch in [
        "Zm9!", "Zm9\n", " Zm9v", "Zm9v ", "Zg==Zg==", "Z=g=", "Zm9é", "-_-_", "Zm9\u{0}", "é",
    ] {
        assert_eq!(dekodieren(falsch), None, "{falsch:?}");
    }
}

#[test]
fn bits_left_over_in_the_last_digit_are_ignored() {
    assert_eq!(dekodieren("Zh==").as_deref(), Some(&b"f"[..]));
    assert_eq!(dekodieren("Zm9=").as_deref(), Some(&b"fo"[..]));
}

#[test]
fn every_length_round_trips() {
    for n in 0..40_u8 {
        let roh: Vec<u8> = (0..n).map(|i| i.wrapping_mul(37).wrapping_add(n)).collect();
        let text = kodieren(&roh);
        assert_eq!(text.len() % 4, 0);
        assert_eq!(dekodieren(&text), Some(roh));
    }
}

#[test]
fn every_digit_decodes_to_its_value() {
    let alphabet = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    for (i, c) in alphabet.chars().enumerate() {
        let text = format!("{c}A==");
        let roh = dekodieren(&text).expect("valid");
        assert_eq!(roh, vec![u8::try_from(i * 4).expect("fits")], "{c}");
    }
}
