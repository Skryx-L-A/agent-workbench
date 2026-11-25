//! The seam of the test build (feature `pruefbau`): the program reads its configuration `lauf.json` next to itself
//! instead of `/etc/werkbank/lauf.json`. This file runs in both builds: the delivered build (without the feature) must
//! ignore a configuration next to the program, also with `WB_TESTUMGEBUNG=1`, and the test build must read it. A broken
//! file shows which way the program went: read, it gives `lauf.konfig_kaputt`.
//!
//! The program runs as a copy in a folder of the test, so the file next to it is the test's own; the steps before the
//! configuration (0 to 2) need only a start file of the own user with mode 0600.
#![cfg(unix)]

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use lauf::fehler::Fehler;

const BIN: &str = env!("CARGO_BIN_EXE_werkbank-lauf");

/// The fixed place of the delivered build.
const FEST: &str = "/etc/werkbank/lauf.json";

fn ordner(name: &str) -> PathBuf {
    let o = std::env::temp_dir().join(format!("lauf-naht-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&o);
    std::fs::create_dir_all(&o).expect("work folder");
    o
}

fn datei(p: &Path, inhalt: &[u8], modus: u32) {
    std::fs::write(p, inhalt).expect("write");
    std::fs::set_permissions(p, std::fs::Permissions::from_mode(modus)).expect("mode");
}

/// Runs the copy of the program next to the broken configuration with the start file; returns code and standard error.
fn lauf(o: &Path) -> (Option<i32>, String) {
    let programm = o.join("werkbank-lauf");
    std::fs::copy(BIN, &programm).expect("copy of the program");
    datei(&o.join("lauf.json"), b"{", 0o600);
    let start = o.join("start.json");
    datei(&start, b"{}", 0o600);
    let a = Command::new(&programm)
        .arg(&start)
        .env_clear()
        .env("WB_TESTUMGEBUNG", "1")
        .stdin(Stdio::null())
        .output()
        .expect("program starts");
    (
        a.status.code(),
        String::from_utf8(a.stderr).expect("UTF-8 stderr"),
    )
}

#[test]
fn the_configuration_next_to_the_program_counts_only_in_the_test_build() {
    let o = ordner("konfig");
    let (code, fehler) = lauf(&o);
    let kaputt = Fehler::KonfigKaputt;
    if cfg!(feature = "pruefbau") {
        // Counter-check: the test build reads the broken file next to it.
        assert_eq!(code, Some(i32::from(kaputt.exitcode())), "{fehler}");
        assert_eq!(fehler, kaputt.zeile());
    } else if !Path::new(FEST).exists() {
        // The delivered build reads the fixed place only; the file next to it is never read.
        let fehlt = Fehler::KonfigFehlt;
        assert_eq!(code, Some(i32::from(fehlt.exitcode())), "{fehler}");
        assert_eq!(fehler, fehlt.zeile());
    } else {
        // a machine with an installed configuration: whatever it says, it is not the broken file of the test
        assert_ne!(fehler, kaputt.zeile());
    }
    let _ = std::fs::remove_dir_all(&o);
}

#[test]
fn the_source_names_the_fixed_place_and_takes_the_seam_only_behind_the_feature() {
    let konfig = include_str!("../src/konfig.rs");
    assert!(konfig.contains(&format!("pub const KONFIG_PFAD: &str = \"{FEST}\";")));
    let pfad = &konfig[konfig.find("pub fn pfad()").expect("pfad")..];
    let pfad = &pfad[..pfad.find("\n}\n").expect("end of pfad")];
    assert!(pfad.contains("if cfg!(feature = \"pruefbau\") {"), "{pfad}");
    assert!(pfad.contains("Some(PathBuf::from(KONFIG_PFAD))"), "{pfad}");
    // the program reads no variable of the environment of the test build: the only names it reads are those of the
    // service and of the harness
    for quelle in [
        include_str!("../src/main.rs"),
        include_str!("../src/ablauf.rs"),
        include_str!("../src/konfig.rs"),
    ] {
        assert!(!quelle.contains("WB_TESTUMGEBUNG"));
    }
}
