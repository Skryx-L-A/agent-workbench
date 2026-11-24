//! The program `ra` on bad and large input: exit codes, empty stdout, one line on stderr, never a panic.
//!
//! These tests start the built program (`CARGO_BIN_EXE_ra`); no other test of the crate starts a process.

use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

const GRENZE: usize = 4_194_304;

/// A small valid case.
const KLEIN: &str = r#"{"schritte":[{"art":"Zeile","sgr":"a"}]}"#;

struct Lauf {
    code: Option<i32>,
    stdout: Vec<u8>,
    stderr: String,
}

fn starten(args: &[&str], stdin: &[u8]) -> Lauf {
    let mut kind = Command::new(env!("CARGO_BIN_EXE_ra"))
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("ra starts");
    let mut ein = kind.stdin.take().expect("stdin is piped");
    let daten = stdin.to_vec();
    let schreiber = std::thread::spawn(move || {
        let _ = ein.write_all(&daten);
    });
    let aus = kind.wait_with_output().expect("ra ends");
    schreiber.join().expect("writer ends");
    Lauf {
        code: aus.status.code(),
        stdout: aus.stdout,
        stderr: String::from_utf8_lossy(&aus.stderr).into_owned(),
    }
}

/// The case `KLEIN` followed by spaces up to `laenge` bytes.
fn aufgefuellt(laenge: usize) -> Vec<u8> {
    let mut bytes = KLEIN.as_bytes().to_vec();
    bytes.resize(laenge, b' ');
    bytes
}

fn ungueltig(lauf: &Lauf) {
    assert_eq!(lauf.code, Some(2), "stderr: {}", lauf.stderr);
    assert!(lauf.stdout.is_empty(), "stdout is not empty");
    assert_eq!(lauf.stderr.lines().count(), 1, "stderr: {}", lauf.stderr);
}

fn gueltig(lauf: &Lauf) {
    assert_eq!(lauf.code, Some(0), "stderr: {}", lauf.stderr);
    assert!(lauf.stderr.is_empty(), "stderr: {}", lauf.stderr);
    assert!(lauf.stdout.ends_with(b"\n"));
    assert_eq!(lauf.stdout.split(|b| *b == b'\n').count(), 2, "one line");
    let wert: serde_json::Value = serde_json::from_slice(&lauf.stdout).expect("stdout is JSON");
    assert!(wert["ergebnisse"].is_array());
}

#[test]
fn a_small_case_on_stdin_runs() {
    gueltig(&starten(&[], KLEIN.as_bytes()));
    gueltig(&starten(&["-"], KLEIN.as_bytes()));
}

#[test]
fn the_result_is_the_one_of_the_library() {
    let lauf = starten(&[], KLEIN.as_bytes());
    let soll = rasterauszug::laufen(KLEIN.as_bytes()).expect("valid case");
    let ist: serde_json::Value = serde_json::from_slice(&lauf.stdout).expect("stdout is JSON");
    assert_eq!(ist, soll);
    assert_eq!(ist["ergebnisse"][0]["ok"], true);
}

#[test]
fn input_of_exactly_the_limit_runs() {
    gueltig(&starten(&[], &aufgefuellt(GRENZE)));
}

#[test]
fn input_one_byte_over_the_limit_is_invalid() {
    ungueltig(&starten(&[], &aufgefuellt(GRENZE + 1)));
}

#[test]
fn input_one_byte_over_the_limit_that_is_no_json_is_invalid() {
    ungueltig(&starten(&[], &vec![b'x'; GRENZE + 1]));
}

#[test]
fn a_very_long_input_is_invalid_without_reading_all() {
    ungueltig(&starten(&[], &aufgefuellt(GRENZE * 2)));
}

#[test]
fn deep_nesting_is_invalid() {
    ungueltig(&starten(&[], &vec![b'['; 100_000]));
}

#[test]
fn nesting_beyond_128_levels_inside_a_case_is_invalid() {
    let tief = format!(
        r#"{{"schritte":[{{"art":"Zeile","sgr":"a","x":{}{}}}]}}"#,
        "[".repeat(200),
        "]".repeat(200)
    );
    ungueltig(&starten(&[], tief.as_bytes()));
}

#[test]
fn invalid_utf8_is_invalid() {
    let mut bytes = br#"{"schritte":[{"art":"Zeile","sgr":""#.to_vec();
    bytes.extend_from_slice(&[0xff, 0xfe]);
    bytes.extend_from_slice(br#""}]}"#);
    ungueltig(&starten(&[], &bytes));
}

#[test]
fn truncated_json_is_invalid() {
    ungueltig(&starten(&[], &KLEIN.as_bytes()[..KLEIN.len() - 3]));
}

#[test]
fn empty_input_is_invalid() {
    ungueltig(&starten(&[], b""));
}

#[test]
fn a_number_instead_of_an_object_is_invalid() {
    ungueltig(&starten(&[], b"42"));
    ungueltig(&starten(&[], b"[]"));
    ungueltig(&starten(&[], b"null"));
}

#[test]
fn missing_steps_or_a_malformed_step_are_invalid() {
    ungueltig(&starten(&[], b"{}"));
    ungueltig(&starten(&[], br#"{"schritte":{}}"#));
    ungueltig(&starten(&[], br#"{"schritte":[5]}"#));
    ungueltig(&starten(&[], br#"{"schritte":[{"art":5}]}"#));
    ungueltig(&starten(&[], br#"{"schritte":[{"art":"Unbekannt"}]}"#));
    ungueltig(&starten(&[], br#"{"schritte":[{"art":"Zeile"}]}"#));
}

#[test]
fn a_case_file_as_argument_runs() {
    let pfad: PathBuf =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/faelle/ra5_abzug_voll.json");
    gueltig(&starten(&[pfad.to_str().expect("path is text")], b""));
}

#[test]
fn a_missing_file_or_too_many_arguments_are_invalid() {
    ungueltig(&starten(&["/nonexistent/ra-case.json"], b""));
    ungueltig(&starten(&["-", "-"], KLEIN.as_bytes()));
    ungueltig(&starten(&["a", "b", "c"], KLEIN.as_bytes()));
}

#[test]
fn a_file_over_the_limit_is_invalid() {
    let pfad = std::env::temp_dir().join(format!("ra-grenze-{}.json", std::process::id()));
    std::fs::write(&pfad, aufgefuellt(GRENZE + 1)).expect("file is written");
    let lauf = starten(&[pfad.to_str().expect("path is text")], b"");
    std::fs::remove_file(&pfad).expect("file is removed");
    ungueltig(&lauf);
}

#[test]
fn a_folder_as_argument_is_invalid() {
    ungueltig(&starten(&[env!("CARGO_MANIFEST_DIR")], b""));
}
