//! The cases `laufkonfig-*` of the contract against the real rule for program folders, and the call `--programmordner`.
//!
//! The package script asks `werkbank-pruefen --programmordner <heim> <ordner>` and writes no folder that it refuses. The cases hold the answer
//! of that call as a list (`pruefer_verboten`); this test holds the real rule (`programmordner::verboten`, the two functions of the enclosure)
//! to the same list: every listed folder is refused, every folder that the expected file names is allowed.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;
use werkbank_pruefen::programmordner::verboten;

fn spec() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../spec/paket")
}

fn json(pfad: &Path) -> Value {
    serde_json::from_slice(&std::fs::read(pfad).expect("read")).expect("json")
}

fn namen() -> Vec<String> {
    let mut n: Vec<String> = std::fs::read_dir(spec().join("faelle"))
        .expect("cases")
        .map(|e| e.expect("entry").file_name().to_string_lossy().into_owned())
        .filter(|n| {
            n.starts_with("laufkonfig-") && Path::new(n).extension().is_some_and(|e| e == "json")
        })
        .collect();
    n.sort();
    n
}

#[test]
fn die_echte_regel_haelt_die_listen_der_faelle() {
    let (mut erlaubt, mut abgelehnt) = (0, 0);
    for n in namen() {
        let fall = json(&spec().join("faelle").join(&n));
        let erw = json(&spec().join("erwartet").join(&n));
        let Some(heim) = fall["heim"].as_str().filter(|h| h.starts_with('/')) else {
            continue;
        };
        let heim = Path::new(heim);
        if let Some(datei) = erw["datei"].as_object() {
            for o in datei["programmordner"].as_array().expect("list") {
                let o = o.as_str().expect("text");
                assert!(!verboten(Path::new(o), heim), "{n}: {o} must pass");
                erlaubt += 1;
            }
        }
        if fall.get("pruefer_verboten").is_some() {
            let liste = fall["pruefer_verboten"].as_array().expect("list");
            assert!(!liste.is_empty(), "{n}");
            for o in liste {
                let o = o.as_str().expect("text");
                assert!(verboten(Path::new(o), heim), "{n}: {o} must be refused");
                abgelehnt += 1;
            }
        }
    }
    assert!(
        erlaubt >= 10 && abgelehnt >= 3,
        "{erlaubt} allowed, {abgelehnt} refused"
    );
}

fn aufruf(args: &[&str]) -> (i32, String, String) {
    let a = Command::new(env!("CARGO_BIN_EXE_werkbank-pruefen"))
        .args(args)
        .env_remove("WB_TESTUMGEBUNG")
        .output()
        .expect("run");
    (
        a.status.code().expect("code"),
        String::from_utf8(a.stdout).expect("utf8"),
        String::from_utf8(a.stderr).expect("utf8"),
    )
}

// The program reads a Linux machine and exists on Unix only (`src/main.rs`); the rule itself is checked above on every
// system.
#[cfg(unix)]
#[test]
fn der_aufruf_programmordner_gibt_exit_0_1_oder_64() {
    let h = "/srv/beispiel/h";
    assert_eq!(
        aufruf(&[
            "--programmordner",
            h,
            "/srv/beispiel/h/.local/lib/node_modules/p"
        ]),
        (0, String::new(), String::new())
    );
    let (rc, out, err) = aufruf(&["--programmordner", h, "/srv/beispiel/h/.local"]);
    assert_eq!((rc, err.as_str()), (1, ""));
    assert!(
        out.starts_with("pruef.programmordner_verboten: ") && out.ends_with('\n'),
        "{out}"
    );
    assert!(out.is_ascii() && out.len() <= 200, "{out}");
    assert_eq!(
        aufruf(&[
            "--programmordner",
            h,
            "/srv/beispiel/h/.local/share/keyrings"
        ])
        .0,
        1
    );
    assert_eq!(
        aufruf(&["--programmordner", h, "/opt/.secrets-sync/p"]).0,
        1
    );
    for args in [
        vec!["--programmordner"],
        vec!["--programmordner", h],
        vec!["--programmordner", h, "relativ"],
        vec!["--programmordner", "relativ", "/opt/p"],
        vec!["--programmordner", h, "/opt/p", "mehr"],
    ] {
        assert_eq!(aufruf(&args).0, 64, "{args:?}");
    }
}

/// On another system the program says for every call that it is not available, and ends with exit 2.
#[cfg(not(unix))]
#[test]
fn der_aufruf_meldet_ausserhalb_von_unix_nicht_verfuegbar() {
    let h = "/srv/beispiel/h";
    for args in [
        vec![
            "--programmordner",
            h,
            "/srv/beispiel/h/.local/lib/node_modules/p",
        ],
        vec!["--programmordner", h, "/srv/beispiel/h/.local"],
        vec!["--programmordner"],
        vec![],
    ] {
        assert_eq!(
            aufruf(&args),
            (
                2,
                String::new(),
                "werkbank-pruefen: nicht verfuegbar: only on Unix systems\n".to_owned()
            ),
            "{args:?}"
        );
    }
}
