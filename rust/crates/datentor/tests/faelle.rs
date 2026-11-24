//! Case tests: every file under `rust/spec/datentor/sichtbar/faelle/` goes to the program `datentor` on stdin, and exit code
//! and parsed JSON are compared with `sichtbar/erwartet/<name>.json`, as the outside runner `aussen_laeufer.py` does. At
//! exit 2 stdout must be empty and stderr must hold exactly one line. The cases and expectations are the contract's; this
//! test does not change them. All data are invented.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde_json::Value;

const PROGRAMM: &str = env!("CARGO_BIN_EXE_datentor");

fn sichtbar() -> PathBuf {
    satz("sichtbar")
}

/// A set of cases under `rust/spec/datentor/`: `sichtbar`, `klassen/sichtbar` or `zonen/sichtbar`.
fn satz(pfad: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../spec/datentor")
        .join(pfad)
}

struct Lauf {
    code: Option<i32>,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

fn starten(args: &[&str], stdin: &[u8]) -> Lauf {
    let mut kind = Command::new(PROGRAMM)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("program starts");
    let mut eingang = kind.stdin.take().expect("stdin is piped");
    let daten = stdin.to_vec();
    // The input is written from a thread: a program that exits before it has read everything must not block the test.
    let schreiber = std::thread::spawn(move || {
        let _ = eingang.write_all(&daten);
    });
    let aus = kind.wait_with_output().expect("program ends");
    schreiber.join().expect("writer ends");
    Lauf {
        code: aus.status.code(),
        stdout: aus.stdout,
        stderr: aus.stderr,
    }
}

/// True and 1 differ in `Value` already; the first difference as a path, or none.
fn unterschied(ist: &Value, soll: &Value, pfad: &str) -> Option<String> {
    match (ist, soll) {
        (Value::Object(a), Value::Object(b)) => {
            for k in a.keys().chain(b.keys()) {
                match (a.get(k), b.get(k)) {
                    (Some(x), Some(y)) => {
                        if let Some(d) = unterschied(x, y, &format!("{pfad}.{k}")) {
                            return Some(d);
                        }
                    }
                    _ => return Some(format!("{pfad}.{k}: missing or extra")),
                }
            }
            None
        }
        (Value::Array(a), Value::Array(b)) => {
            for (i, (x, y)) in a.iter().zip(b).enumerate() {
                if let Some(d) = unterschied(x, y, &format!("{pfad}[{i}]")) {
                    return Some(d);
                }
            }
            (a.len() != b.len())
                .then(|| format!("{pfad}: length {} instead of {}", a.len(), b.len()))
        }
        _ => (ist != soll).then(|| format!("{pfad}: {ist} instead of {soll}")),
    }
}

/// Why the case is red, or none; `basis` is the set, `args` the subcommand.
fn pruefen(basis: &Path, args: &[&str], name: &str) -> Option<String> {
    let fall =
        fs::read(basis.join("faelle").join(format!("{name}.json"))).expect("case is readable");
    let erwartet: Value = serde_json::from_slice(
        &fs::read(basis.join("erwartet").join(format!("{name}.json")))
            .expect("expectation is readable"),
    )
    .expect("expectation is JSON");
    let soll_exit = erwartet["exit"].as_i64().expect("exit is a number");
    let lauf = starten(args, &fall);
    if lauf.code.map(i64::from) != Some(soll_exit) {
        return Some(format!("exit code {:?} instead of {soll_exit}", lauf.code));
    }
    if soll_exit == 2 {
        if !lauf.stdout.is_empty() {
            return Some("stdout is not empty".to_owned());
        }
        let text = String::from_utf8_lossy(&lauf.stderr);
        let zeilen = text.lines().count();
        if zeilen != 1 || text.trim().is_empty() || !text.ends_with('\n') {
            return Some(format!(
                "stderr has {zeilen} lines instead of exactly one: {text:?}"
            ));
        }
        return None;
    }
    let Ok(ist) = serde_json::from_slice::<Value>(&lauf.stdout) else {
        return Some("stdout is not JSON".to_owned());
    };
    if !lauf.stderr.is_empty() {
        return Some("stderr is not empty at exit 0".to_owned());
    }
    unterschied(&ist, &erwartet["ausgabe"], "$")
}

fn namen(basis: &Path) -> Vec<String> {
    let mut namen: Vec<String> = fs::read_dir(basis.join("faelle"))
        .expect("case folder exists")
        .map(|e| {
            let pfad = e.expect("entry is readable").path();
            pfad.file_stem()
                .expect("file has a name")
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    namen.sort();
    namen
}

/// Runs every case of a set; at least `mindestens` cases must be found.
fn satz_pruefen(pfad: &str, args: &[&str], mindestens: usize) {
    let basis = satz(pfad);
    let namen = namen(&basis);
    assert!(
        namen.len() >= mindestens,
        "only {} cases found",
        namen.len()
    );
    let rot: Vec<String> = namen
        .iter()
        .filter_map(|n| pruefen(&basis, args, n).map(|grund| format!("{n}: {grund}")))
        .collect();
    assert!(
        rot.is_empty(),
        "{} of {} cases red:\n{}",
        rot.len(),
        namen.len(),
        rot.join("\n")
    );
}

#[test]
fn jeder_sichtbare_fall_stimmt() {
    satz_pruefen("sichtbar", &[], 53);
}

#[test]
fn jeder_sichtbare_fall_der_klassen_stimmt() {
    satz_pruefen("klassen/sichtbar", &["klassen"], 101);
    satz_pruefen("klassen/sichtbar", &["klassen", "-"], 101);
}

#[test]
fn jeder_sichtbare_fall_der_zonen_stimmt() {
    satz_pruefen("zonen/sichtbar", &["zonen"], 114);
}

#[test]
fn unterbefehl_falsch_aufgerufen_ist_exit_2() {
    let fall =
        fs::read(satz("zonen/sichtbar/faelle/u-ohne-modellzonen.json")).expect("case is readable");
    for args in [
        &["zonen", "x"][..],
        &["klassen", "-", "-"],
        &["Zonen"],
        &["-", "zonen"],
        &["zonen", "klassen"],
    ] {
        let lauf = starten(args, &fall);
        assert_eq!(lauf.code, Some(2), "args {args:?}");
        assert_eq!(lauf.stdout.len(), 0);
        assert_eq!(String::from_utf8_lossy(&lauf.stderr).lines().count(), 1);
    }
}

#[test]
fn eingabe_mit_dem_argument_minus_ist_dieselbe() {
    let fall = fs::read(sichtbar().join("faelle/d1-vorfuehrung.json")).expect("case is readable");
    let a = starten(&[], &fall);
    let b = starten(&["-"], &fall);
    assert_eq!(a.code, Some(0));
    assert_eq!(b.code, Some(0));
    assert_eq!(a.stdout, b.stdout);
}

#[test]
fn falscher_aufruf_ist_exit_2_mit_einer_zeile() {
    let fall = fs::read(sichtbar().join("faelle/d1-vorfuehrung.json")).expect("case is readable");
    for args in [&["x"][..], &["-", "-"], &["--hilfe"], &[""]] {
        let lauf = starten(args, &fall);
        assert_eq!(lauf.code, Some(2), "args {args:?}");
        assert_eq!(lauf.stdout.len(), 0);
        assert_eq!(String::from_utf8_lossy(&lauf.stderr).lines().count(), 1);
    }
}

#[test]
fn eingabegrenze_genau_ein_mebibyte() {
    let fall = fs::read(sichtbar().join("faelle/d1-keine-anfrage.json")).expect("case is readable");
    let mut text = fall.clone();
    // Valid JSON padded with blanks: exactly 1 048 576 bytes pass, one more does not.
    text.resize(1_048_576, b' ');
    let lauf = starten(&[], &text);
    assert_eq!(lauf.code, Some(0));
    text.push(b' ');
    let lauf = starten(&[], &text);
    assert_eq!(lauf.code, Some(2));
    assert_eq!(lauf.stdout.len(), 0);
    assert_eq!(String::from_utf8_lossy(&lauf.stderr).lines().count(), 1);
    // A much larger input is refused, too, without being read to the end of memory.
    let lauf = starten(&[], &vec![b' '; 5_000_000]);
    assert_eq!(lauf.code, Some(2));
}

#[test]
fn schlechte_eingabe_ist_nie_eine_panik() {
    let eingaben: [&[u8]; 9] = [
        b"",
        b"\xff\xfe",
        b"\xef\xbb\xbf{}",
        b"null",
        b"{}",
        b"[[[[[[",
        b"{\"vorgabe\":",
        b"{\"anfragen\":[],\"vorgabe\":{}}",
        b"\0\0\0",
    ];
    for e in eingaben {
        let lauf = starten(&[], e);
        assert_eq!(lauf.code, Some(2), "input {e:?}");
        assert_eq!(lauf.stdout.len(), 0);
        assert_eq!(String::from_utf8_lossy(&lauf.stderr).lines().count(), 1);
    }
    // Deep nesting of one megabyte does not overflow the stack.
    let lauf = starten(&[], &vec![b'['; 1_048_576]);
    assert_eq!(lauf.code, Some(2));
}
