//! Case tests: the visible outside cases of the contract `dienst` (280, `tests/faelle` and `tests/erwartet`, copies from the
//! contract package, with the cases `di12_*`) run against the program `wb-dienst`.
//!
//! The runner and the check client are the Python files of the contract (`tests/werkzeug/`, standard library only, copies from the
//! contract package; the decoder of the viewer is `tests/werkzeug/referenz/ra_ref.py`). The client starts the program with a start file,
//! talks to it over the socket as one or more viewers, and writes one JSON line; the runner compares it with `tests/erwartet`.
//! The test needs `python3` and `lsof` (the client counts the network sockets of the service with it); without them it is red,
//! not skipped. The cases run in several runners at once, each in its own base folder.
#![cfg(unix)] // the crate is Unix only (see `src/lib.rs`)
#![cfg(feature = "pruefbau")] // the fixed child programs `--kind` exist in the test build only; `naht.rs` runs in both builds

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const DIENST: &str = env!("CARGO_BIN_EXE_wb-dienst");

fn tests() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests")
}

fn fall_namen() -> Vec<String> {
    let mut namen: Vec<String> = std::fs::read_dir(tests().join("faelle"))
        .expect("case folder exists")
        .filter_map(|e| {
            let n = e.expect("entry").file_name().to_string_lossy().into_owned();
            n.strip_suffix(".json").map(str::to_owned)
        })
        .collect();
    namen.sort();
    namen
}

/// A base folder below `/tmp` (the path of the socket must stay below 100 bytes) that is removed at the end.
struct Basis(PathBuf);

impl Basis {
    fn neu() -> Self {
        let pfad = Path::new("/tmp").join(format!("dienst-faelle-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&pfad);
        std::fs::create_dir_all(&pfad).expect("base folder");
        Self(pfad)
    }
}

impl Drop for Basis {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Runs the cases `namen` in one runner; returns the exit code and the output.
fn laeufer(namen: &[String], basis: &Path, erstrot: bool) -> (Option<i32>, String) {
    let werkzeug = tests().join("werkzeug");
    std::fs::create_dir_all(basis).expect("runner folder");
    let mut befehl = Command::new("python3");
    befehl
        .args(["-I", "-B", "-u"])
        .arg(werkzeug.join("test_aussen.py"))
        .args(namen)
        .env("DI_BIN", DIENST)
        .env("DI_FAELLE", tests())
        .env("DI_REF_DIR", werkzeug.join("referenz"))
        .env("DI_TMP", basis)
        .env("DI_ZEIT", "60")
        .stdin(Stdio::null());
    if erstrot {
        // a red case ends the runner: a service that is broken does not make the test wait for every case (also in the second
        // run: a case that is red twice is red, and the rest of the group need not run to say so)
        befehl.env("DI_ERSTROT", "1");
    }
    // The runner of the contract stops at the first red case only when the case differs from its expectation, not when it runs
    // into its time limit. So this function reads the output line by line and ends the runner itself at the first red line.
    let fehler_datei = basis.join("stderr.txt");
    befehl
        .stdout(Stdio::piped())
        .stderr(std::fs::File::create(&fehler_datei).expect("stderr file"));
    let mut kind = befehl
        .spawn()
        .expect("python3 starts (it is needed for this test)");
    let mut stdout = String::new();
    let mut abgebrochen = false;
    if let Some(aus) = kind.stdout.take() {
        for zeile in BufReader::new(aus).lines().map_while(Result::ok) {
            stdout.push_str(&zeile);
            stdout.push('\n');
            if erstrot && zeile.starts_with("ROT") {
                abgebrochen = true;
                let _ = kind.kill();
                break;
            }
        }
    }
    let status = kind.wait().expect("runner ends");
    let text = format!(
        "{stdout}{}",
        std::fs::read_to_string(&fehler_datei).unwrap_or_default()
    );
    let code = if abgebrochen { Some(1) } else { status.code() };
    (code, text)
}

#[test]
fn sichtbare_faelle() {
    let namen = fall_namen();
    assert!(namen.len() >= 280, "only {} cases found", namen.len());
    assert_eq!(
        std::fs::read_dir(tests().join("erwartet"))
            .expect("expected folder")
            .count(),
        namen.len(),
        "every case has its expectation"
    );
    let basis = Basis::neu();
    let zahl = std::thread::available_parallelism()
        .map_or(4, std::num::NonZero::get)
        .clamp(1, 8);
    let mut gruppen: Vec<Vec<String>> = vec![Vec::new(); zahl];
    for (i, n) in namen.iter().enumerate() {
        gruppen[i % zahl].push(n.clone());
    }
    // No repetition: the service reads its connections in one `poll`, so the order of the inputs of two viewers does not depend
    // on which thread wakes first. A red case ends its runner and the test is red.
    let ergebnisse = alle_laufen(&gruppen, &basis.0, "a", true);
    let mut rot = Vec::new();
    let mut gruen = 0_usize;
    for e in &ergebnisse {
        let (r, g) = auswerten(e);
        if r.is_empty() {
            gruen += g;
        } else {
            rot.extend(r);
        }
    }
    assert!(rot.is_empty(), "red cases:\n{}", rot.join("\n"));
    assert_eq!(gruen, namen.len(), "every case ran and is green");
}

/// Runs every group in a runner of its own, at the same time.
fn alle_laufen(
    gruppen: &[Vec<String>],
    basis: &Path,
    lauf: &str,
    erstrot: bool,
) -> Vec<(Option<i32>, String)> {
    std::thread::scope(|s| {
        let faeden: Vec<_> = gruppen
            .iter()
            .enumerate()
            .map(|(i, g)| {
                let ordner = basis.join(format!("{lauf}{i}"));
                s.spawn(move || laeufer(g, &ordner, erstrot))
            })
            .collect();
        faeden
            .into_iter()
            .map(|f| f.join().expect("runner thread"))
            .collect()
    })
}

/// The red lines and the number of green cases of one runner; a runner that failed without a red line gives one.
fn auswerten(ergebnis: &(Option<i32>, String)) -> (Vec<String>, usize) {
    let (code, text) = ergebnis;
    let mut rot = Vec::new();
    let mut gruen = 0;
    for zeile in text.lines() {
        if zeile.starts_with("ROT") {
            rot.push(zeile.to_owned());
        }
        if let Some(n) = zeile
            .split_once(" gruen, ")
            .and_then(|(vorn, _)| vorn.parse::<usize>().ok())
        {
            gruen += n;
        }
    }
    if *code != Some(0) && rot.is_empty() {
        rot.push(format!("runner exit {code:?}: {text}"));
    }
    (rot, gruen)
}
