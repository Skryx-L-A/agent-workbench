//! Outside tests: reads every visible case of contract 1 (`rust/spec/start/faelle`) and compares the
//! result with `erwartet`.
//!
//! A case has `form` (`anfrage` or `datei`), the start as `text` (UTF-8) or `b64` (raw bytes), and the
//! settings file as the JSON value `einstellungen`. The cases of form `aufruf` describe the program
//! `Lauf starten` as a whole; that program is a later step, so this runner counts them as skipped.

use std::fs;
use std::path::{Path, PathBuf};

use einstellungen::lesen as lesen_einstellungen;
use serde_json::Value;
use start::{lesen_anfrage, lesen_datei, pruefen, schreiben};

/// Decodes standard base 64; padding ends the input.
fn base64_decode(text: &str) -> Vec<u8> {
    let digit = |c: u8| -> Option<u32> {
        match c {
            b'A'..=b'Z' => Some(u32::from(c - b'A')),
            b'a'..=b'z' => Some(u32::from(c - b'a') + 26),
            b'0'..=b'9' => Some(u32::from(c - b'0') + 52),
            b'+' => Some(62),
            b'/' => Some(63),
            _ => None,
        }
    };
    let mut out = Vec::new();
    let mut bits = 0_u32;
    let mut count = 0;
    for value in text.bytes().map_while(digit) {
        bits = (bits << 6) | value;
        count += 6;
        if count >= 8 {
            count -= 8;
            out.push(u8::try_from((bits >> count) & 0xff).expect("masked to one byte"));
        }
    }
    out
}

fn read_json(path: &Path) -> Value {
    let text = fs::read_to_string(path).expect("case file is readable");
    serde_json::from_str(&text).expect("case file is JSON")
}

fn case_bytes(case: &Value) -> Vec<u8> {
    match case.get("text").and_then(Value::as_str) {
        Some(text) => text.as_bytes().to_vec(),
        None => base64_decode(case["b64"].as_str().expect("case has text or b64")),
    }
}

fn case_files() -> Vec<PathBuf> {
    let base = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../spec/start/faelle");
    let mut names: Vec<PathBuf> = fs::read_dir(base)
        .expect("case directory exists")
        .map(|entry| entry.expect("entry is readable").path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
        .collect();
    names.sort();
    names
}

/// Returns a description of the mismatch, or `None` when the case passes.
fn check_case(case_path: &Path, expected_path: &Path) -> Option<String> {
    let case = read_json(case_path);
    let expected = read_json(expected_path);
    let einstellungen = match lesen_einstellungen(
        serde_json::to_vec(&case["einstellungen"])
            .expect("settings serialize")
            .as_slice(),
    ) {
        Ok(einstellungen) => einstellungen,
        Err(fehler) => return Some(format!("settings file rejected: {fehler}")),
    };
    let bytes = case_bytes(&case);
    let ergebnis = match case["form"].as_str() {
        Some("anfrage") => {
            lesen_anfrage(&bytes).and_then(|anfrage| pruefen(&anfrage, &einstellungen))
        }
        Some("datei") => lesen_datei(&bytes).and_then(|datei| pruefen(&datei, &einstellungen)),
        other => return Some(format!("unknown form {other:?}")),
    };
    match (expected["gueltig"].as_bool(), ergebnis) {
        (Some(true), Ok(art)) => {
            (expected["route_art"].as_str() != Some(art.kennung())).then(|| {
                format!(
                    "route kind {}, expected {}",
                    art.kennung(),
                    expected["route_art"]
                )
            })
        }
        (Some(true), Err(fehler)) => Some(format!("rejected with {fehler}, expected valid")),
        (Some(false), Ok(art)) => Some(format!(
            "accepted as {}, expected {}",
            art.kennung(),
            expected["code"]
        )),
        (Some(false), Err(fehler)) => (expected["code"].as_str() != Some(fehler.code()))
            .then(|| format!("code {}, expected {}", fehler.code(), expected["code"])),
        (None, _) => Some("expectation has no field gueltig".to_owned()),
    }
}

#[test]
#[allow(clippy::print_stderr)]
fn visible_cases_of_contract_one_pass_except_the_program_cases() {
    let base = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../spec/start");
    let names = case_files();
    assert!(!names.is_empty(), "no visible cases");
    let mut failures = Vec::new();
    let mut uebersprungen = 0;
    let mut geprueft = 0;
    for case_path in &names {
        let case = read_json(case_path);
        if case["form"].as_str() == Some("aufruf") {
            uebersprungen += 1;
            continue;
        }
        geprueft += 1;
        let expected_path = base
            .join("erwartet")
            .join(case_path.file_name().expect("case file name"));
        if let Some(problem) = check_case(case_path, &expected_path) {
            failures.push(format!("{}: {problem}", case_path.display()));
        }
    }
    eprintln!(
        "contract 1: {geprueft} visible cases checked, {uebersprungen} skipped (form aufruf, program Lauf starten: later step)"
    );
    assert!(
        failures.is_empty(),
        "{} of {geprueft} cases failed:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

#[test]
fn a_written_start_file_reads_back_unchanged() {
    let base = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../spec/start");
    let mut geprueft = 0;
    for case_path in case_files() {
        let case = read_json(&case_path);
        if case["form"].as_str() != Some("datei") {
            continue;
        }
        let Ok(datei) = lesen_datei(&case_bytes(&case)) else {
            continue;
        };
        let geschrieben = schreiben(&datei);
        let wieder = lesen_datei(&geschrieben)
            .unwrap_or_else(|fehler| panic!("written file rejected: {fehler}"));
        assert_eq!(wieder, datei, "{}", case_path.display());
        assert_eq!(wieder.sitzung(), datei.sitzung());
        geprueft += 1;
    }
    assert!(
        geprueft > 0,
        "no start file to round-trip in {}",
        base.display()
    );
}
