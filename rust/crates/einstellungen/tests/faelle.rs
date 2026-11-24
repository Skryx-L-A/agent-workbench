//! Outside tests: reads every visible case of contract 2 (`rust/spec/einstellungen/faelle`) and
//! compares the result with `erwartet`.
//!
//! A case holds `text` (UTF-8) or `b64` (raw bytes as base 64). An expectation is `{"gueltig": true}`
//! or `{"gueltig": false, "code": "einst.<code>"}`. The case files are the contract; this runner only
//! reads them.

use std::fs;
use std::path::{Path, PathBuf};

use einstellungen::lesen;
use serde_json::Value;

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

/// Returns a description of the mismatch, or `None` when the case passes.
fn check_case(case_path: &Path, expected_path: &Path) -> Option<String> {
    let case = read_json(case_path);
    let expected = read_json(expected_path);
    let ergebnis = lesen(&case_bytes(&case));
    match (expected["gueltig"].as_bool(), ergebnis) {
        (Some(true), Ok(_)) => None,
        (Some(true), Err(fehler)) => Some(format!("rejected with {fehler}, expected valid")),
        (Some(false), Ok(_)) => Some(format!("accepted, expected {}", expected["code"])),
        (Some(false), Err(fehler)) => (expected["code"].as_str() != Some(fehler.code()))
            .then(|| format!("code {}, expected {}", fehler.code(), expected["code"])),
        (None, _) => Some("expectation has no field gueltig".to_owned()),
    }
}

#[test]
fn visible_cases_of_contract_two_pass() {
    let base = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../spec/einstellungen");
    let mut names: Vec<PathBuf> = fs::read_dir(base.join("faelle"))
        .expect("case directory exists")
        .map(|entry| entry.expect("entry is readable").path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
        .collect();
    names.sort();
    assert!(!names.is_empty(), "no visible cases");
    let mut failures = Vec::new();
    for case_path in &names {
        let expected_path = base
            .join("erwartet")
            .join(case_path.file_name().expect("case file name"));
        if let Some(problem) = check_case(case_path, &expected_path) {
            failures.push(format!("{}: {problem}", case_path.display()));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} cases failed:\n{}",
        failures.len(),
        names.len(),
        failures.join("\n")
    );
}
