//! Outside tests: the start command of `copilot` and the key of a folder mark against the program cases
//! of contract 1 (form `aufruf`, `rust/spec/start/faelle`).
//!
//! The runner of `faelle.rs` skips the program cases. This file reads the part of them that the crate
//! builds: the argument list of `copilot` and the form of the key in `ordnermarken` and `marke_danach`.

use std::fs;
use std::path::PathBuf;

use einstellungen::lesen as lesen_einstellungen;
use serde_json::Value;
use start::{copilot_argv, effort, ist_marken_kennung, lesen_datei};

fn faelle() -> Vec<(String, Value, Value)> {
    let base = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../spec/start");
    let mut namen: Vec<PathBuf> = fs::read_dir(base.join("faelle"))
        .expect("case directory exists")
        .map(|e| e.expect("entry is readable").path())
        .filter(|p| p.extension().is_some_and(|e| e == "json"))
        .collect();
    namen.sort();
    namen
        .into_iter()
        .map(|pfad| {
            let lesen = |p: &PathBuf| -> Value {
                serde_json::from_str(&fs::read_to_string(p).expect("readable")).expect("JSON")
            };
            let name = pfad
                .file_stem()
                .expect("name")
                .to_string_lossy()
                .into_owned();
            let erwartet = base
                .join("erwartet")
                .join(pfad.file_name().expect("file name"));
            (name, lesen(&pfad), lesen(&erwartet))
        })
        .filter(|(_, fall, _)| fall["form"].as_str() == Some("aufruf"))
        .collect()
}

#[test]
fn the_argument_list_of_copilot_matches_every_program_case_that_states_it() {
    let mut geprueft = 0;
    let mut mit_werkzeugen = 0;
    let mut mit_auto = 0;
    for (name, fall, erwartet) in faelle() {
        if erwartet["exec"].as_str() != Some("copilot") || erwartet["harness_argv"].is_null() {
            continue;
        }
        let lage = &fall["lage"];
        // a start of version 2 adds the effort and the session option of "Lauf starten" to this list; the test of
        // `lauf` (tests/faelle.rs) checks those cases with their whole argv
        if lage["start"]["fassung"].as_u64() == Some(2) {
            continue;
        }
        let einst = lesen_einstellungen(
            serde_json::to_vec(&lage["einstellungen"])
                .expect("serializes")
                .as_slice(),
        )
        .unwrap_or_else(|f| panic!("{name}: settings rejected: {f}"));
        let modell = lage["start"]["modell"].as_str().expect("model");
        let argv = copilot_argv(
            modell,
            einst.copilot_ohne_rueckfrage(),
            "<heim>/.copilot/wb-mcp.json",
        );
        let mut erwartet_argv: Vec<&str> = erwartet["harness_argv"]
            .as_array()
            .expect("list")
            .iter()
            .map(|a| a.as_str().expect("text"))
            .collect();
        // "Lauf starten" adds the effort it passes on right after the model (contract 1, "Effort immer ausdruecklich"):
        // for copilot always, here the fallback of a start of version 1
        let datei = lesen_datei(
            serde_json::to_vec(&lage["start"])
                .expect("serializes")
                .as_slice(),
        )
        .unwrap_or_else(|f| panic!("{name}: start rejected: {f}"));
        let nach = erwartet_argv
            .iter()
            .position(|a| *a == "--model")
            .expect("model")
            + 2;
        let paar: Vec<&str> = erwartet_argv.drain(nach..nach + 2).collect();
        assert_eq!(
            paar,
            [
                "--reasoning-effort",
                effort(&datei, &einst).expect("effort")
            ],
            "{name}"
        );
        assert_eq!(argv, erwartet_argv, "{name}");
        geprueft += 1;
        mit_werkzeugen += usize::from(!einst.copilot_ohne_rueckfrage().is_empty());
        mit_auto += usize::from(modell == "auto");
    }
    assert!(geprueft >= 8, "only {geprueft} cases checked");
    assert!(
        mit_werkzeugen >= 4,
        "only {mit_werkzeugen} cases with tools"
    );
    assert!(mit_auto >= 2, "only {mit_auto} cases with auto");
}

#[test]
fn a_case_that_names_forbidden_arguments_gets_none_of_them() {
    let mut geprueft = 0;
    for (name, fall, erwartet) in faelle() {
        let Some(ohne) = erwartet["argv_ohne"].as_array() else {
            continue;
        };
        if erwartet["exec"].as_str() != Some("copilot") {
            continue;
        }
        let einst = lesen_einstellungen(
            serde_json::to_vec(&fall["lage"]["einstellungen"])
                .expect("serializes")
                .as_slice(),
        )
        .expect("settings valid");
        let modell = fall["lage"]["start"]["modell"].as_str().expect("model");
        let argv = copilot_argv(modell, einst.copilot_ohne_rueckfrage(), "/x");
        for verboten in ohne {
            let verboten = verboten.as_str().expect("text");
            assert!(!argv.iter().any(|a| a == verboten), "{name}: {verboten}");
        }
        geprueft += 1;
    }
    assert!(geprueft >= 3, "only {geprueft} cases checked");
}

#[test]
fn the_keys_of_the_marks_in_the_cases_have_the_form_except_in_the_cases_about_the_key() {
    let mut gut = 0;
    let mut schlecht = 0;
    for (name, fall, erwartet) in faelle() {
        let kennungsfall = name.contains("kennung");
        let marken = fall["lage"]["ordnermarken"].as_array();
        let danach = erwartet["marke_danach"].as_array();
        for marke in marken.into_iter().chain(danach).flatten() {
            let Some(kennung) = marke["kennung"].as_str() else {
                assert!(kennungsfall, "{name}: mark without key outside a key case");
                continue;
            };
            if kennung == "<neu>" {
                continue;
            }
            if ist_marken_kennung(kennung) {
                gut += 1;
            } else {
                assert!(kennungsfall, "{name}: bad key {kennung:?}");
                schlecht += 1;
            }
        }
    }
    assert!(gut >= 20, "only {gut} good keys seen");
    assert!(schlecht >= 4, "only {schlecht} bad keys seen");
}
