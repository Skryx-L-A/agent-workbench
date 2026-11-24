//! The reading of contract 1 (start) and contract 2 (settings) through the crates `start` and `einstellungen`,
//! against the visible cases of those contracts (`rust/spec/start/`, `rust/spec/einstellungen/`). The cases of the form `aufruf` of
//! contract 1 concern the program "Lauf starten" and are not the point's.

use std::fs;
use std::path::PathBuf;

use serde_json::Value;
use stelle::einstellungen;
use stelle::start::{self, Form};

fn faelle(vertrag: &str) -> Vec<(String, Value, Value)> {
    let wurzel = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../spec")
        .join(vertrag);
    let mut namen: Vec<PathBuf> = fs::read_dir(wurzel.join("faelle"))
        .expect("the case folder is readable")
        .map(|e| e.expect("an entry").path())
        .collect();
    namen.sort();
    namen
        .into_iter()
        .map(|p| {
            let datei = p.file_name().expect("a name");
            let lies = |pfad: PathBuf| -> Value {
                serde_json::from_str(&fs::read_to_string(pfad).expect("readable")).expect("JSON")
            };
            let fall = lies(p.clone());
            let erwartet = lies(wurzel.join("erwartet").join(datei));
            (datei.to_string_lossy().into_owned(), fall, erwartet)
        })
        .collect()
}

fn base64(text: &str) -> Vec<u8> {
    let wert = |c: u8| -> u32 {
        u32::from(match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' => 62,
            _ => 63,
        })
    };
    let mut aus = Vec::new();
    for gruppe in text.trim_end_matches('=').as_bytes().chunks(4) {
        let n = gruppe.iter().fold(0u32, |s, c| (s << 6) | wert(*c)) << (6 * (4 - gruppe.len()));
        aus.extend_from_slice(&n.to_be_bytes()[1..gruppe.len()]);
    }
    aus
}

fn bytes(fall: &Value) -> Vec<u8> {
    match (fall["text"].as_str(), fall["b64"].as_str()) {
        (Some(text), _) => text.as_bytes().to_vec(),
        (None, Some(b64)) => base64(b64),
        _ => panic!("a case without text and b64"),
    }
}

#[test]
fn vertrag_1_start() {
    let alle = faelle("start");
    let dateien = alle.len();
    let mut zahl = 0;
    let mut aufruf = 0;
    for (name, fall, erwartet) in alle {
        let form = match fall["form"].as_str() {
            Some("anfrage") => Form::Anfrage,
            Some("datei") => Form::Datei,
            Some("aufruf") => {
                aufruf += 1;
                continue;
            }
            andere => panic!("{name}: unknown form {andere:?}"),
        };
        let einst = serde_json::to_vec(&fall["einstellungen"]).expect("settings");
        let einst = einstellungen::lesen(&einst).expect(&name);
        match start::pruefen(&bytes(&fall), form, &einst) {
            Ok(s) => {
                assert_eq!(erwartet["gueltig"], true, "{name}");
                assert_eq!(erwartet["route_art"], s.route_art.kennung(), "{name}");
                assert_eq!(s.sitzung.is_some(), form == Form::Datei, "{name}");
            }
            Err(abgelehnt) => {
                assert_eq!(erwartet["gueltig"], false, "{name}");
                assert_eq!(erwartet["code"], abgelehnt.code.code(), "{name}");
            }
        }
        zahl += 1;
    }
    // Every case file is read: each one is checked here or has the form `aufruf` ("Lauf starten").
    // New cases of contract 1 keep this test green; a silently skipped case does not.
    assert_eq!(
        zahl + aufruf,
        dateien,
        "every case file of contract 1 is read"
    );
    assert!(zahl >= 88, "cases of the forms anfrage and datei: {zahl}");
}

#[test]
fn vertrag_2_einstellungen() {
    let alle = faelle("einstellungen");
    let dateien = alle.len();
    let mut zahl = 0;
    for (name, fall, erwartet) in alle {
        match einstellungen::lesen(&bytes(&fall)) {
            Ok(_) => assert_eq!(erwartet["gueltig"], true, "{name}"),
            Err(code) => {
                assert_eq!(erwartet["gueltig"], false, "{name}");
                assert_eq!(erwartet["code"], code.code(), "{name}");
            }
        }
        zahl += 1;
    }
    assert_eq!(zahl, dateien, "every case file of contract 2 is read");
    assert!(zahl >= 180, "cases of contract 2: {zahl}");
}
