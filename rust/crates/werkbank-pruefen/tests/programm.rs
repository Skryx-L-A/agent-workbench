//! The program itself, outside the seam where it can: the exit code when its output cannot be written, and
//! `--vor-entfernen` on a machine without the package.
#![cfg(unix)] // the collection and the program read a Unix machine (see `src/main.rs`)

use std::path::Path;
use std::process::Command;
#[cfg(all(target_os = "linux", feature = "pruefbau"))]
use std::process::Stdio;

const BIN: &str = env!("CARGO_BIN_EXE_werkbank-pruefen");

/// A finding keeps its exit code when standard output cannot be written (`/dev/full`): only a green result turns into
/// the internal error 70.
#[cfg(target_os = "linux")]
#[test]
fn ein_befund_behaelt_seinen_code_wenn_die_ausgabe_scheitert() {
    let heim = Path::new("/srv/beispiel");
    let ordner = heim.join(".ssh");
    assert!(werkbank_pruefen::programmordner::verboten(&ordner, heim));
    let a = Command::new(BIN)
        .arg("--programmordner")
        .arg(heim)
        .arg(&ordner)
        .env_remove("WB_TESTUMGEBUNG")
        .stdout(std::fs::File::create("/dev/full").expect("/dev/full"))
        .output()
        .expect("program starts");
    assert_eq!(a.status.code(), Some(1));
}

/// A green result that cannot be written is the internal error 70 (test build: the seam gives the green state).
#[cfg(all(target_os = "linux", feature = "pruefbau"))]
#[test]
fn ein_gruenes_ergebnis_das_nicht_geschrieben_wird_ist_ein_interner_fehler() {
    let gruen = serde_json::json!({
        "plattform": "linux", "konten": 1, "dienste": [], "apparmor": "ok", "einstellungen": {"zustand": "ok"},
        "nutzerordner": "ok", "schluessel": "ok", "copilot": "ok", "vault": "ok", "stelle": "ok",
        "paketstand": {"manifest": "ok", "bau": "auslieferung", "version": "gleich", "dateien": []}
    });
    let lage = werkbank_pruefen::lage::Lage::aus_json(&gruen).expect("lage");
    assert_eq!(
        werkbank_pruefen::bewerte::bewerte(&lage).rc,
        0,
        "the state is green"
    );
    let datei =
        std::env::temp_dir().join(format!("wb-pruefen-programm-{}.json", std::process::id()));
    std::fs::write(&datei, gruen.to_string()).expect("write");
    let geschrieben = Command::new(BIN)
        .env("WB_TESTUMGEBUNG", "1")
        .env("WB_PRUEFEN_LAGE", &datei)
        .output()
        .expect("program starts");
    let voll = Command::new(BIN)
        .env("WB_TESTUMGEBUNG", "1")
        .env("WB_PRUEFEN_LAGE", &datei)
        .stdout(std::fs::File::create("/dev/full").expect("/dev/full"))
        .stderr(Stdio::null())
        .output()
        .expect("program starts");
    std::fs::remove_file(&datei).ok();
    assert_eq!(geschrieben.status.code(), Some(0), "counter-check: green");
    assert_eq!(voll.status.code(), Some(70));
}

/// `--vor-entfernen` reads the real `/var/lib/werkbank`. Without it, no key file stays: no line, exit 0. On a machine
/// with the package this test has nothing to check.
#[test]
fn vor_dem_entfernen_ohne_werkbank_ordner_bleibt_kein_schluessel() {
    if Path::new("/var/lib/werkbank").exists() {
        return;
    }
    let a = Command::new(BIN)
        .arg("--vor-entfernen")
        .env_remove("WB_TESTUMGEBUNG")
        .env_remove("WB_PRUEFEN_LAGE")
        .output()
        .expect("program starts");
    assert_eq!(
        (
            a.status.code(),
            String::from_utf8_lossy(&a.stdout).into_owned(),
            String::from_utf8_lossy(&a.stderr).into_owned()
        ),
        (Some(0), String::new(), String::new())
    );
}
