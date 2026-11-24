//! The visible manifest cases of the contract (`manifest-*`) against `manifest::pruefen`.
//!
//! A case for another platform than Ubuntu stops at `plattform_fremd` in this program; that is accepted only where the
//! contract says the case is valid or lacks a required file of its own platform (the tables of the other platforms are
//! not this program's business).

use std::path::{Path, PathBuf};

use serde_json::Value;
use werkbank_pruefen::manifest::pruefen;

fn ordner() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../spec/paket")
}

fn base64(text: &str) -> Vec<u8> {
    let mut aus = Vec::new();
    let (mut puffer, mut bits) = (0u32, 0u32);
    for c in text.bytes().filter(|c| *c != b'=') {
        let wert = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => panic!("not base64"),
        };
        puffer = (puffer << 6) | u32::from(wert);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            aus.push(u8::try_from((puffer >> bits) & 0xff).expect("byte"));
        }
    }
    aus
}

fn json(pfad: &Path) -> Value {
    serde_json::from_slice(&std::fs::read(pfad).expect("read")).expect("json")
}

#[test]
fn jeder_sichtbare_manifestfall() {
    let mut namen: Vec<String> = std::fs::read_dir(ordner().join("faelle"))
        .expect("cases")
        .map(|e| e.expect("entry").file_name().to_string_lossy().into_owned())
        .filter(|n| {
            n.starts_with("manifest-") && Path::new(n).extension().is_some_and(|e| e == "json")
        })
        .collect();
    namen.sort();
    assert!(namen.len() >= 80, "found {} manifest cases", namen.len());
    let mut falsch = Vec::new();
    let mut gleich_geprueft = 0;
    for n in &namen {
        let fall = json(&ordner().join("faelle").join(n));
        let erw = json(&ordner().join("erwartet").join(n));
        let mut roh = match fall.get("text").and_then(Value::as_str) {
            Some(t) => t.as_bytes().to_vec(),
            None => base64(fall["b64"].as_str().expect("b64")),
        };
        let auffuellen = fall
            .get("auffuellen")
            .and_then(Value::as_u64)
            .map_or(0, |n| usize::try_from(n).expect("size"));
        roh.extend(std::iter::repeat_n(b' ', auffuellen));
        let soll: Option<&str> = if erw["gueltig"] == true {
            None
        } else {
            Some(
                erw["code"]
                    .as_str()
                    .expect("code")
                    .strip_prefix("paket.")
                    .expect("prefix"),
            )
        };
        let ist = pruefen(&roh).err();
        if ist == soll {
            gleich_geprueft += 1;
        } else if !(ist == Some("plattform_fremd")
            && matches!(soll, None | Some("pflichtdatei_fehlt")))
        {
            falsch.push(format!("{n}: expected {soll:?}, got {ist:?}"));
        }
    }
    assert!(
        falsch.is_empty(),
        "{} cases differ:\n{}",
        falsch.len(),
        falsch.join("\n")
    );
    assert!(
        gleich_geprueft >= 70,
        "only {gleich_geprueft} cases were compared exactly"
    );
}
