//! The seam `WB_PRUEFEN_LAGE` exists only in the test build (feature `pruefbau`). This file runs in both builds: the
//! delivered build (without the feature) must ignore it, also with `WB_TESTUMGEBUNG=1`, and check the machine; the test
//! build must read the file. A broken file shows which way the program went: read, it gives the internal error 70.
#![cfg(unix)] // the collection and the program read a Unix machine (see `src/main.rs`)

use std::path::{Path, PathBuf};
use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_werkbank-pruefen");

/// The names of the seam; none of them may stand in the delivered program.
const NAMEN: [&str; 2] = ["WB_TESTUMGEBUNG", "WB_PRUEFEN_LAGE"];

fn arbeitsordner(name: &str) -> PathBuf {
    let o = std::env::temp_dir().join(format!("wb-pruefen-naht-{}-{name}", std::process::id()));
    std::fs::create_dir_all(&o).expect("work folder");
    o
}

fn lauf(lage: &Path, aufruf: &[&str]) -> (i32, String, String) {
    let a = Command::new(BIN)
        .args(aufruf)
        .env_remove("SUDO_USER")
        .env("WB_TESTUMGEBUNG", "1")
        .env("WB_PRUEFEN_LAGE", lage)
        .output()
        .expect("program starts");
    (
        a.status.code().expect("exit code"),
        String::from_utf8(a.stdout).expect("ASCII stdout"),
        String::from_utf8(a.stderr).expect("ASCII stderr"),
    )
}

#[test]
fn the_seam_counts_only_in_the_test_build() {
    let ordner = arbeitsordner("naht");
    let kaputt = ordner.join("kaputt.json");
    std::fs::write(&kaputt, "{").expect("write");
    for aufruf in [&[][..], &["--vor-entfernen"][..]] {
        let (rc, out, err) = lauf(&kaputt, aufruf);
        if cfg!(feature = "pruefbau") {
            // Counter-check: the test build reads the broken file.
            assert_eq!(
                (rc, err.as_str()),
                (70, "pruef.intern: Interner Fehler.\n"),
                "{aufruf:?}"
            );
        } else {
            // The delivered build checks the machine; the file of the seam is never read.
            assert_ne!(rc, 70, "{aufruf:?}: {out}{err}");
            assert!(!err.contains("pruef.intern"), "{aufruf:?}: {err}");
            if aufruf.is_empty() && !Path::new("/usr/share/werkbank/manifest.json").exists() {
                assert_eq!(rc, 1, "the real machine without the package is red: {out}");
                assert!(out.contains("pruef.paketstand_manifest_fehlt"), "{out}");
            }
        }
    }
    std::fs::remove_dir_all(&ordner).ok();
}

#[test]
fn the_delivered_program_holds_no_name_of_the_seam() {
    let programm = std::fs::read(BIN).expect("program readable");
    let gefunden: Vec<&str> = NAMEN
        .iter()
        .copied()
        .filter(|n| programm.windows(n.len()).any(|w| w == n.as_bytes()))
        .collect();
    if cfg!(feature = "pruefbau") {
        // Counter-check: the test build knows both names, so the search finds what is there.
        assert_eq!(gefunden, NAMEN.to_vec());
    } else {
        assert!(gefunden.is_empty(), "names in the program: {gefunden:?}");
    }
}
