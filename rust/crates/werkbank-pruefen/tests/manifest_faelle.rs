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

/// A valid Ubuntu manifest with the required files, the given features and one extra file at `extra`.
fn manifest_mit(merkmale: &[&str], extra: &str) -> Vec<u8> {
    manifest_mit_dateien(merkmale, &[extra])
}

/// A valid Ubuntu manifest with the required files, the given features and the extra files.
fn manifest_mit_dateien(merkmale: &[&str], extra: &[&str]) -> Vec<u8> {
    use werkbank_pruefen::manifest::{PFLICHT, PROFIL_A, STARTER};
    let leer = "0".repeat(64);
    let mut dateien: Vec<String> = PFLICHT
        .iter()
        .copied()
        .chain([(PROFIL_A, "daten"), (STARTER, "programm")])
        .chain(extra.iter().map(|p| (*p, "daten")))
        .map(|(pfad, art)| {
            format!(r#"{{"pfad":"{pfad}","groesse":0,"sha256":"{leer}","art":"{art}"}}"#)
        })
        .collect();
    dateien.sort();
    let merkmale: Vec<String> = merkmale.iter().map(|k| format!("\"{k}\"")).collect();
    format!(
        r#"{{"fassung":1,"paket":"werkbank","version":"1.2.3","plattform":"ubuntu-26.04-x64","bau":"test","merkmale":[{}],"dateien":[{}],"bibliotheken":3}}"#,
        merkmale.join(","),
        dateien.join(",")
    )
    .into_bytes()
}

#[test]
fn ein_merkmal_hat_die_form_einer_kennung() {
    assert_eq!(
        pruefen(&manifest_mit(&[], "usr/share/werkbank/x")).err(),
        None
    );
    let lang = "a".repeat(64);
    assert_eq!(
        pruefen(&manifest_mit(&[&lang], "usr/share/werkbank/x")).err(),
        None
    );
    let zu_lang = "a".repeat(65);
    assert_eq!(
        pruefen(&manifest_mit(&[&zu_lang], "usr/share/werkbank/x")).err(),
        Some("kennung_form")
    );
    assert_eq!(
        pruefen(&manifest_mit(&[""], "usr/share/werkbank/x")).err(),
        Some("kennung_form")
    );
}

#[test]
fn ein_manifest_hat_hoechstens_16_merkmale() {
    let namen: Vec<String> = (0..17).map(|i| format!("merkmal{i:02}")).collect();
    let mut merkmale: Vec<&str> = namen.iter().map(String::as_str).collect();
    merkmale.pop();
    assert_eq!(
        pruefen(&manifest_mit(&merkmale, "usr/share/werkbank/x")).err(),
        None
    );
    merkmale.push(&namen[16]);
    assert_eq!(
        pruefen(&manifest_mit(&merkmale, "usr/share/werkbank/x")).err(),
        Some("anzahl")
    );
}

#[test]
fn nur_com_und_lpt_mit_genau_einer_ziffer_von_1_bis_9_sind_reserviert() {
    for name in ["com1", "lpt9", "com1.txt"] {
        assert_eq!(
            pruefen(&manifest_mit(&[], &format!("usr/share/werkbank/{name}"))).err(),
            Some("pfad_form"),
            "{name}"
        );
    }
    for name in ["com12", "comx", "lpt0", "com"] {
        assert_eq!(
            pruefen(&manifest_mit(&[], &format!("usr/share/werkbank/{name}"))).err(),
            None,
            "{name}"
        );
    }
}

#[test]
fn ein_manifest_von_genau_der_grenze_ist_gueltig_ein_byte_mehr_zu_gross() {
    use werkbank_pruefen::manifest::GRENZE;
    let mut roh = manifest_mit(&[], "usr/share/werkbank/x");
    roh.resize(GRENZE, b' ');
    assert_eq!(pruefen(&roh).err(), None);
    roh.push(b' ');
    assert_eq!(pruefen(&roh).err(), Some("zu_gross"));
}

#[test]
fn ein_manifest_hat_hoechstens_2000_dateien() {
    use werkbank_pruefen::manifest::PFLICHT;
    // The required files, the profile and the starter come first; the rest are extra files.
    let pfade: Vec<String> = (0..2000 - PFLICHT.len() - 2)
        .map(|i| format!("usr/share/werkbank/f{i:04}"))
        .collect();
    let mut extra: Vec<&str> = pfade.iter().map(String::as_str).collect();
    assert_eq!(pruefen(&manifest_mit_dateien(&[], &extra)).err(), None);
    extra.push("usr/share/werkbank/eine-mehr");
    assert_eq!(
        pruefen(&manifest_mit_dateien(&[], &extra)).err(),
        Some("anzahl")
    );
}
