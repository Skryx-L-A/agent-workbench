//! The entry points of the library: limits, error texts, the limit for all messages.

use serde_json::Value;

use crate::{EINGABE_MAX, NACHRICHTEN_MAX, Ungueltig, laufen, laufen_mit};

const FALL: &str = r#"{"anfang":{"personen":{"p":{}},"sitzungen":[{"id":"s","kundenraum":"r","groesse":[4,2]}]},
"ereignisse":[
{"t":0,"art":"Verbinden","betrachter":"b1","person":"p","raum":"r"},
{"t":0,"art":"Verbinden","betrachter":"b2","person":"p","raum":"r"},
{"t":1,"art":"Abo","betrachter":"b1","sitzung":"s","spalten":4,"zeilen":2},
{"t":2,"art":"Titel","sitzung":"s","text":"neu"}]}"#;

/// The bytes of all messages of the output, serialised, as the core counts them.
fn nachrichten_bytes(aus: &Value) -> usize {
    aus["nachrichten"]
        .as_object()
        .expect("object")
        .values()
        .flat_map(|liste| liste.as_array().expect("array"))
        .map(|n| serde_json::to_vec(n).expect("serialises").len())
        .sum()
}

#[test]
fn the_limits_are_those_of_the_contract() {
    assert_eq!(EINGABE_MAX, 4_194_304);
    assert_eq!(NACHRICHTEN_MAX, 32 * 1024 * 1024);
}

#[test]
fn the_limit_for_all_messages_is_exact() {
    let aus = laufen(FALL.as_bytes()).expect("valid");
    let summe = nachrichten_bytes(&aus);
    assert!(summe > 1000, "{summe}");
    assert_eq!(
        laufen_mit(FALL.as_bytes(), summe).expect("at the limit"),
        aus
    );
    assert!(laufen_mit(FALL.as_bytes(), summe - 1).is_err());
    assert!(laufen_mit(FALL.as_bytes(), 0).is_err());
}

#[test]
fn a_message_to_many_viewers_counts_for_each() {
    let verbinden: Vec<String> = (0..30)
        .map(|i| {
            format!(r#"{{"t":0,"art":"Verbinden","betrachter":"b{i}","person":"p","raum":"r"}}"#)
        })
        .collect();
    let ereignisse = format!(
        r#"{},{{"t":1,"art":"Titel","sitzung":"s","text":"neu"}}"#,
        verbinden.join(",")
    );
    let fall = format!(
        r#"{{"anfang":{{"personen":{{"p":{{}}}},"sitzungen":[{{"id":"s","kundenraum":"r"}}]}},"ereignisse":[{ereignisse}]}}"#
    );
    let aus = laufen(fall.as_bytes()).expect("valid");
    let summe = nachrichten_bytes(&aus);
    assert!(laufen_mit(fall.as_bytes(), summe).is_ok());
    assert!(laufen_mit(fall.as_bytes(), summe - 1).is_err());
}

#[test]
fn input_over_the_limit_is_invalid_and_at_the_limit_is_read() {
    let mut bytes = FALL.as_bytes().to_vec();
    bytes.resize(EINGABE_MAX, b' ');
    assert!(laufen(&bytes).is_ok());
    bytes.push(b' ');
    let fehler = laufen(&bytes).expect_err("too long");
    assert!(fehler.0.contains("4194304"), "{}", fehler.0);
}

#[test]
fn an_invalid_case_names_its_reason() {
    let grund = |text: &str| laufen(text.as_bytes()).expect_err("invalid").0;
    assert!(grund("x").starts_with("kein JSON"));
    assert!(grund("[]").contains("kein Objekt"));
    assert!(grund("{}").contains("ereignisse"));
    assert!(grund(r#"{"ereignisse":3}"#).contains("ereignisse"));
    assert!(grund(r#"{"anfang":3,"ereignisse":[]}"#).contains("anfang"));
    assert!(grund(r#"{"anfang":null,"ereignisse":[]}"#).contains("anfang"));
    assert!(
        grund(r#"{"ereignisse":[{"t":2,"art":"Takt"},{"t":1,"art":"Takt"}]}"#)
            .contains("Ereignis 1")
    );
    assert!(grund(r#"{"ereignisse":[{"t":0,"art":"Takt"},3]}"#).contains("Ereignis 1"));
    assert!(grund(r#"{"anfang":{"routen":[{}]},"ereignisse":[]}"#).contains("route"));
}

#[test]
fn a_case_without_start_and_with_equal_times_runs() {
    let aus = laufen(
        br#"{"ereignisse":[{"t":0,"art":"Takt"},{"t":0,"art":"Takt"},{"t":7,"art":"Nix"}]}"#,
    )
    .expect("valid");
    assert_eq!(aus["ablehnungen"][0]["ereignis"], 2);
    assert_eq!(aus["ablehnungen"][0]["t"], 7);
    assert_eq!(aus["ablehnungen"][0]["grund"], "ereignis_unbekannt");
    assert_eq!(aus["ablehnungen"].as_array().expect("array").len(), 1);
}

#[test]
fn a_start_with_a_size_outside_2_to_1000_columns_and_1_to_1000_rows_is_invalid() {
    let mit = |groesse: &str| {
        let fall = format!(
            r#"{{"anfang":{{"sitzungen":[{{"id":"s","groesse":{groesse}}}]}},"ereignisse":[]}}"#
        );
        laufen(fall.as_bytes())
    };
    for gut in ["[2,1]", "[1000,1000]", "[2,1000]", "[1000,1]", "[80,24]"] {
        assert!(mit(gut).is_ok(), "{gut}");
    }
    for falsch in [
        // one column and zero are refused (the carrier cannot hold one column, `alacritty_terminal` issue 9112)
        "[1,3]",
        "[0,3]",
        "[3,0]",
        "[0,0]",
        "[1,1]",
        "[1000,0]",
        "[0,1000]",
        "[1001,3]",
        "[3,1001]",
        "[1001,1001]",
        "[-1,3]",
        "[3,-1]",
        "[3]",
        "[3,4,5]",
        "3",
        "[\"a\",3]",
        "[3,2.5]",
    ] {
        assert!(mit(falsch).is_err(), "{falsch}");
    }
}

#[test]
fn the_error_reads_as_one_line() {
    let fehler = Ungueltig::from("kaputt");
    assert_eq!(fehler.to_string(), "Fall ungueltig: kaputt");
    assert_eq!(Ungueltig::from("x".to_owned()), Ungueltig("x".to_owned()));
}
