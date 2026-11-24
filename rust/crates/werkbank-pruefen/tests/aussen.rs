//! Outside test: every visible case `pruefung-linux-*` of the contract runs against the built program.
//!
//! The program reads the `lage` of a case through the test seam (`WB_TESTUMGEBUNG=1`, `WB_PRUEFEN_LAGE=<file>`), gets the
//! `aufruf` of the case as its arguments, and must print exactly the expected lines on the expected stream with the
//! expected exit code. Nothing here touches the real machine.
#![cfg(unix)] // the collection and the program read a Unix machine (see `src/main.rs`)

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

fn faelle() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../spec/paket")
}

fn lesen(pfad: &Path) -> Value {
    serde_json::from_slice(&std::fs::read(pfad).expect("case file readable"))
        .expect("case file is JSON")
}

fn arbeitsordner(name: &str) -> PathBuf {
    let o = std::env::temp_dir().join(format!("wb-pruefen-aussen-{}-{name}", std::process::id()));
    std::fs::create_dir_all(&o).expect("work folder");
    o
}

fn lauf(lage: &Path, aufruf: &[String], seam: bool) -> (i32, String, String) {
    let mut c = Command::new(env!("CARGO_BIN_EXE_werkbank-pruefen"));
    c.args(aufruf)
        .env_remove("SUDO_USER")
        .env("WB_PRUEFEN_LAGE", lage);
    if seam {
        c.env("WB_TESTUMGEBUNG", "1");
    } else {
        c.env_remove("WB_TESTUMGEBUNG");
    }
    let a = c.output().expect("program starts");
    (
        a.status.code().expect("exit code"),
        String::from_utf8(a.stdout).expect("ASCII stdout"),
        String::from_utf8(a.stderr).expect("ASCII stderr"),
    )
}

#[test]
fn jeder_sichtbare_fall_linux() {
    let ordner = arbeitsordner("faelle");
    let mut namen: Vec<String> = std::fs::read_dir(faelle().join("faelle"))
        .expect("cases folder")
        .map(|e| e.expect("entry").file_name().to_string_lossy().into_owned())
        .filter(|n| {
            n.starts_with("pruefung-linux-")
                && Path::new(n).extension().is_some_and(|e| e == "json")
        })
        .collect();
    namen.sort();
    assert!(
        namen.len() >= 33,
        "the contract lists 33 visible cases, found {}",
        namen.len()
    );
    let mut falsch = Vec::new();
    for n in &namen {
        let fall = lesen(&faelle().join("faelle").join(n));
        let erw = lesen(&faelle().join("erwartet").join(n));
        let lage = ordner.join(n);
        std::fs::write(&lage, fall["lage"].to_string()).expect("write lage");
        let aufruf: Vec<String> = fall["aufruf"]
            .as_array()
            .expect("aufruf")
            .iter()
            .map(|a| a.as_str().expect("text").to_owned())
            .collect();
        let (rc, out, err) = lauf(&lage, &aufruf, true);
        let (strom, andere) = if erw["strom"] == "stderr" {
            (&err, &out)
        } else {
            (&out, &err)
        };
        let mut soll = String::new();
        for z in erw["zeilen"].as_array().expect("zeilen") {
            soll.push_str(z.as_str().expect("text"));
            soll.push('\n');
        }
        if i64::from(rc) != erw["rc"].as_i64().expect("rc") || *strom != soll || !andere.is_empty()
        {
            falsch.push(format!("{n}: rc {rc}, stdout {out:?}, stderr {err:?}"));
        }
    }
    std::fs::remove_dir_all(&ordner).ok();
    assert!(
        falsch.is_empty(),
        "{} of {} cases differ:\n{}",
        falsch.len(),
        namen.len(),
        falsch.join("\n")
    );
}

#[test]
fn die_naht_gilt_nur_in_der_testumgebung() {
    // Without WB_TESTUMGEBUNG=1 the program must not read the file: a broken file gives no exit code 70 then.
    let ordner = arbeitsordner("naht");
    let kaputt = ordner.join("kaputt.json");
    std::fs::write(&kaputt, "{").expect("write");
    let (rc_mit, _, err_mit) = lauf(&kaputt, &[], true);
    assert_eq!(
        (rc_mit, err_mit.as_str()),
        (70, "pruef.intern: Interner Fehler.\n")
    );
    if !Path::new("/usr/share/werkbank/manifest.json").exists() {
        let (rc, out, _) = lauf(&kaputt, &[], false);
        assert_eq!(rc, 1, "the real machine without the package is red: {out}");
        assert!(out.contains("pruef.paketstand_manifest_fehlt"), "{out}");
    }
    std::fs::remove_dir_all(&ordner).ok();
}
