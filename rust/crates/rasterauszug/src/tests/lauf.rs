//! The case runner: the input limit, the text of a refusal and the small readers.

use serde_json::json;

use crate::lauf::{hex_lesen, hex_schreiben};
use crate::{EINGABE_MAX, Ungueltig, laufen};

const KLEIN: &str = r#"{"schritte":[{"art":"Zeile","sgr":"a"}]}"#;

/// The case `KLEIN` followed by spaces up to `laenge` bytes.
fn aufgefuellt(laenge: usize) -> Vec<u8> {
    let mut bytes = KLEIN.as_bytes().to_vec();
    bytes.resize(laenge, b' ');
    bytes
}

#[test]
fn input_up_to_the_limit_runs_and_beyond_it_is_refused() {
    assert_eq!(EINGABE_MAX, 4_194_304);
    assert!(laufen(&aufgefuellt(EINGABE_MAX - 1)).is_ok());
    assert!(laufen(&aufgefuellt(EINGABE_MAX)).is_ok());
    assert!(laufen(&aufgefuellt(EINGABE_MAX + 1)).is_err());
    assert!(laufen(&aufgefuellt(EINGABE_MAX + 2)).is_err());
    assert!(laufen(&aufgefuellt(EINGABE_MAX + 100)).is_err());
}

#[test]
fn a_refusal_says_why() {
    let Err(e) = laufen(b"[]") else {
        panic!("a list is no case");
    };
    assert_eq!(e.to_string(), format!("Fall ungueltig: {}", e.0));
    assert!(e.0.contains("schritte"), "{e}");
    assert_eq!(Ungueltig::from("x").to_string(), "Fall ungueltig: x");
    assert_eq!(Ungueltig("y".to_owned()), Ungueltig::from("y"));
    let Err(zu_lang) = laufen(&aufgefuellt(EINGABE_MAX + 1)) else {
        panic!("too long");
    };
    assert!(zu_lang.0.contains("4194304"), "{zu_lang}");
}

#[test]
fn hex_is_lower_case_pairs() {
    assert_eq!(hex_lesen(""), Ok(Vec::new()));
    assert_eq!(hex_lesen("00ff"), Ok(vec![0, 255]));
    assert_eq!(hex_lesen("09af"), Ok(vec![0x09, 0xaf]));
    assert_eq!(
        hex_lesen("0a1b2c3d4e5f6789"),
        Ok(vec![0x0a, 0x1b, 0x2c, 0x3d, 0x4e, 0x5f, 0x67, 0x89])
    );
    assert_eq!(hex_lesen("f0"), Ok(vec![0xf0]));
    assert_eq!(hex_lesen("0f"), Ok(vec![0x0f]));
    for schlecht in [
        "0", "abc", "0F", "A0", "g0", "0g", "zz", "0x", " 0", "0 ", "\u{e9}0", "00\u{e9}", "+1",
        "-1",
    ] {
        assert!(hex_lesen(schlecht).is_err(), "{schlecht:?}");
    }
    assert_eq!(hex_schreiben(&[]), "");
    assert_eq!(
        hex_schreiben(&[0x00, 0x0f, 0xf0, 0xab, 0xff, 0x12, 0x9a]),
        "000ff0abff129a"
    );
    for n in 0..=255_u8 {
        assert_eq!(hex_lesen(&hex_schreiben(&[n])), Ok(vec![n]));
    }
}

#[test]
fn a_step_that_fails_is_a_result_and_a_bad_step_is_a_refused_case() {
    let aus =
        laufen(br#"{"schritte":[{"art":"Zeile","sgr":"\u001b[1J"},{"art":"Zeile","sgr":"a"}]}"#)
            .expect("valid case");
    assert_eq!(
        aus["ergebnisse"][0],
        json!({"ok": false, "fehler": "sgr_unbekannt"})
    );
    assert_eq!(
        aus["ergebnisse"][1],
        json!({"ok": true, "laeufe": [[0, 0, 0, "a"]], "sgr": "a", "rundreise": true})
    );
    assert!(laufen(br#"{"schritte":[{"art":"Zeile","sgr":5}]}"#).is_err());
    assert!(laufen(br#"{"schritte":[{"art":"Verbinden","verbindung":"v"}]}"#).is_err());
}

#[test]
fn the_session_number_is_a_whole_number_from_0_to_255() {
    let fall = |nr: &str| {
        format!(
            r#"{{"schritte":[{{"art":"Verbinden","verbindung":"v","deflate":false}},{{"art":"Vergessen","verbindung":"v","nr":{nr}}}]}}"#
        )
    };
    for gut in ["0", "1", "254", "255"] {
        assert!(laufen(fall(gut).as_bytes()).is_ok(), "nr {gut}");
    }
    for schlecht in ["256", "-1", "1.5", "1.0", "\"1\"", "null", "true", "1000"] {
        assert!(laufen(fall(schlecht).as_bytes()).is_err(), "nr {schlecht}");
    }
}
