use super::*;

/// Lower-case hex of bytes.
fn hex_text(bytes: &[u8]) -> String {
    const ZIFFERN: &[u8; 16] = b"0123456789abcdef";
    bytes
        .iter()
        .flat_map(|b| [ZIFFERN[usize::from(b >> 4)], ZIFFERN[usize::from(b & 15)]])
        .map(char::from)
        .collect()
}

#[test]
fn hmac_vektoren() {
    // Reference values from Python's hmac module (hmac.new(key, msg, hashlib.sha256)).
    assert_eq!(
        hex_text(&hmac_sha256(&[1; 32], b"abc")),
        "73860612aa6aadf68985b6e9c4233357cedd5f24a221eba740b2aa8350276cfc"
    );
    let mut schluessel = [0u8; 32];
    for (n, b) in schluessel.iter_mut().enumerate() {
        *b = u8::try_from(n).expect("small");
    }
    assert_eq!(
        hex_text(&hmac_sha256(&schluessel, b"")),
        "d38b42096d80f45f826b44a9d5607de72496a415d3f4a1a8c88e3bb9da8dc1cb"
    );
    assert_eq!(
        hex_text(&hmac_sha256(&[0xff; 32], &[b'x'; 100])),
        "b8229bddef3b975b775595a0e363172fdcedaf52bb1bd947c40b803e07ae8111"
    );
}

#[test]
fn hex_hin_und_zurueck() {
    let alle: Vec<u8> = (0..32).map(|n| n * 8 + 7).collect();
    let text = hex_text(&alle);
    assert_eq!(hex_bytes(&text).to_vec(), alle);
    assert_eq!(hex_bytes(&"af".repeat(32)), [0xaf; 32]);
    assert_eq!(hex_bytes(&"09".repeat(32)), [0x09; 32]);
    assert_eq!(nibble(b'0'), 0);
    assert_eq!(nibble(b'9'), 9);
    assert_eq!(nibble(b'a'), 10);
    assert_eq!(nibble(b'f'), 15);
}

#[test]
fn vergleich_sieht_jedes_byte() {
    let a = [0x5a; 32];
    assert!(gleich_konstant(&a, &a));
    for stelle in [0, 17, 31] {
        let mut b = a;
        b[stelle] ^= 0x01;
        assert!(!gleich_konstant(&a, &b), "difference at {stelle}");
    }
    // Two differences with the same bits: a sum by xor would cancel them, the sum by or does not.
    let mut b = a;
    b[3] ^= 0x10;
    b[9] ^= 0x10;
    assert!(!gleich_konstant(&a, &b));
}

fn nachweis(schluessel: &[u8; 32], person: &str, herausforderung: &[u8]) -> String {
    let mut nachricht = format!("{person}\n").into_bytes();
    nachricht.extend_from_slice(herausforderung);
    format!(
        "h1:{person}:{}",
        hex_text(&hmac_sha256(schluessel, &nachricht))
    )
}

#[test]
fn h1_nimmt_nur_den_richtigen_nachweis() {
    let schluessel = [7u8; 32];
    let p = PrueferH1::neu(schluessel);
    let person = format!("ps_{}", "0123456789abcdef".repeat(2));
    let gut = nachweis(&schluessel, &person, b"herausforderung");
    assert_eq!(p.pruefe(b"herausforderung", &gut), Ok(person.clone()));
    assert_eq!(p.pruefe(b"herausforderunG", &gut), Err(()));
    assert_eq!(p.pruefe(b"herausforderung", &gut.to_uppercase()), Err(()));
    assert_eq!(
        p.pruefe(b"herausforderung", &gut.replacen("h1:", "h2:", 1)),
        Err(())
    );
    assert_eq!(
        p.pruefe(b"herausforderung", &gut.replacen("h1:", "", 1)),
        Err(())
    );
    assert_eq!(p.pruefe(b"herausforderung", &format!("{gut}0")), Err(()));
    assert_eq!(p.pruefe(b"herausforderung", &gut[..gut.len() - 1]), Err(()));
    assert_eq!(
        p.pruefe(b"herausforderung", &gut.replacen(':', ";", 2)),
        Err(())
    );
    let andere = format!("ps_{}", "f".repeat(32));
    assert_eq!(
        p.pruefe(b"herausforderung", &gut.replace(&person, &andere)),
        Err(())
    );
    assert_eq!(
        p.pruefe(
            b"herausforderung",
            &nachweis(&[8; 32], &person, b"herausforderung")
        ),
        Err(())
    );
    // A proof over the challenge without the pseudonym and LF does not hold.
    let ohne = format!(
        "h1:{person}:{}",
        hex_text(&hmac_sha256(&schluessel, b"herausforderung"))
    );
    assert_eq!(p.pruefe(b"herausforderung", &ohne), Err(()));
    let kein_pseudonym = nachweis(&schluessel, "ps_kurz", b"herausforderung");
    assert_eq!(p.pruefe(b"herausforderung", &kein_pseudonym), Err(()));
}

#[test]
fn debug_zeigt_den_schluessel_nicht() {
    let text = format!("{:?}", PrueferH1::neu([0xab; 32]));
    assert_eq!(text, "PrueferH1 { schluessel: <verdeckt> }");
}
