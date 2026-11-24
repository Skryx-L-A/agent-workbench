//! The program `zk` on bad and large input: exit codes, empty stdout, one line on stderr, never a panic.
//!
//! These tests start the built program (`CARGO_BIN_EXE_zk`); no other test of the crate starts a process.

use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

const GRENZE: usize = 4_194_304;

/// A small valid case.
const KLEIN: &str = r#"{"anfang":{},"ereignisse":[{"t":0,"art":"Takt"}]}"#;

struct Lauf {
    code: Option<i32>,
    stdout: Vec<u8>,
    stderr: String,
}

fn starten(args: &[&str], stdin: &[u8]) -> Lauf {
    let mut kind = Command::new(env!("CARGO_BIN_EXE_zk"))
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("zk starts");
    let mut ein = kind.stdin.take().expect("stdin is piped");
    let daten = stdin.to_vec();
    let schreiber = std::thread::spawn(move || {
        let _ = ein.write_all(&daten);
    });
    let aus = kind.wait_with_output().expect("zk ends");
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
    let wert: serde_json::Value = serde_json::from_slice(&lauf.stdout).expect("stdout is JSON");
    assert!(wert["zustand"].is_object());
}

#[test]
fn a_small_case_on_stdin_runs() {
    gueltig(&starten(&[], KLEIN.as_bytes()));
    gueltig(&starten(&["-"], KLEIN.as_bytes()));
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
        r#"{{"anfang":{{}},"ereignisse":[{{"t":0,"art":"Takt","x":{}{}}}]}}"#,
        "[".repeat(200),
        "]".repeat(200)
    );
    ungueltig(&starten(&[], tief.as_bytes()));
}

#[test]
fn invalid_utf8_is_invalid() {
    let mut bytes = br#"{"anfang":{},"ereignisse":[{"t":0,"art":"Titel","text":""#.to_vec();
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
fn t_as_text_is_invalid() {
    ungueltig(&starten(
        &[],
        br#"{"anfang":{},"ereignisse":[{"t":"0","art":"Takt"}]}"#,
    ));
}

#[test]
fn a_falling_or_missing_or_fractional_t_is_invalid() {
    ungueltig(&starten(
        &[],
        br#"{"ereignisse":[{"t":5,"art":"Takt"},{"t":4,"art":"Takt"}]}"#,
    ));
    ungueltig(&starten(&[], br#"{"ereignisse":[{"art":"Takt"}]}"#));
    ungueltig(&starten(&[], br#"{"ereignisse":[{"t":-1,"art":"Takt"}]}"#));
    ungueltig(&starten(&[], br#"{"ereignisse":[{"t":1.5,"art":"Takt"}]}"#));
    ungueltig(&starten(
        &[],
        br#"{"ereignisse":[{"t":true,"art":"Takt"}]}"#,
    ));
    ungueltig(&starten(&[], br#"{"ereignisse":[5]}"#));
}

#[test]
fn missing_events_or_a_wrong_start_are_invalid() {
    ungueltig(&starten(&[], b"{}"));
    ungueltig(&starten(&[], br#"{"anfang":{}}"#));
    ungueltig(&starten(&[], br#"{"ereignisse":{}}"#));
    ungueltig(&starten(&[], br#"{"anfang":[],"ereignisse":[]}"#));
    ungueltig(&starten(
        &[],
        br#"{"anfang":{"personen":[]},"ereignisse":[]}"#,
    ));
    ungueltig(&starten(
        &[],
        br#"{"anfang":{"sitzungen":[{"id":5}]},"ereignisse":[]}"#,
    ));
}

#[test]
fn a_case_file_as_argument_runs() {
    let pfad: PathBuf =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/faelle/r8_routen.json");
    gueltig(&starten(&[pfad.to_str().expect("path is text")], b""));
}

#[test]
fn a_missing_file_or_too_many_arguments_are_invalid() {
    ungueltig(&starten(&["/nonexistent/zk-case.json"], b""));
    ungueltig(&starten(&["-", "-"], KLEIN.as_bytes()));
    ungueltig(&starten(&["a", "b", "c"], KLEIN.as_bytes()));
}

#[test]
fn a_file_over_the_limit_is_invalid() {
    let pfad = std::env::temp_dir().join(format!("zk-grenze-{}.json", std::process::id()));
    std::fs::write(&pfad, aufgefuellt(GRENZE + 1)).expect("file is written");
    let lauf = starten(&[pfad.to_str().expect("path is text")], b"");
    std::fs::remove_file(&pfad).expect("file is removed");
    ungueltig(&lauf);
}

#[test]
fn a_folder_as_argument_is_invalid() {
    ungueltig(&starten(&[env!("CARGO_MANIFEST_DIR")], b""));
}
