//! The fixed child programs `wb-dienst --kind <name>` exist only in the test build (feature `pruefbau`). This file runs in both
//! builds: the delivered build (without the feature) must refuse `--kind` like any unknown argument and must hold no code and
//! no text of the child programs; the test build must run them.
#![cfg(unix)] // the crate is Unix only (see `src/lib.rs`)

use std::fs;
use std::process::{Command, Stdio};

const BIN: &str = env!("CARGO_BIN_EXE_wb-dienst");

/// Texts that stand only in the child programs (`src/kindprogramm.rs`) and the dispatch of `--kind` (`src/lib.rs`); none of them may
/// stand in the delivered program. Short German words (`klammer`, `fuellen`) are left out: a package of the dependencies may hold them.
const NAMEN: [&str; 5] = [
    "--kind",
    "ausgabewarte",
    "osc52viele",
    "zeuge.term",
    "zeuge.bereit",
];

fn enthaelt(heu: &[u8], nadel: &str) -> bool {
    heu.windows(nadel.len()).any(|w| w == nadel.as_bytes())
}

/// Runs the program with the arguments, an empty environment and no input; returns the exit code, standard output and standard error.
fn aufrufen(args: &[&str]) -> (Option<i32>, String, String) {
    let aus = Command::new(BIN)
        .args(args)
        .env_clear()
        .stdin(Stdio::null())
        .output()
        .expect("the program runs");
    (
        aus.status.code(),
        String::from_utf8_lossy(&aus.stdout).into_owned(),
        String::from_utf8_lossy(&aus.stderr).into_owned(),
    )
}

#[test]
fn the_flag_kind_counts_only_in_the_test_build() {
    // `still 0` is the child program that writes nothing, reads nothing and ends at once with 0.
    let (code, aus, fehler) = aufrufen(&["--kind", "still", "0"]);
    assert_eq!(aus, "", "nothing on standard output");
    if cfg!(feature = "pruefbau") {
        // Counter-check: the test build runs the child program.
        assert_eq!(code, Some(0), "{fehler}");
        assert_eq!(fehler, "");
    } else {
        // The delivered build knows no `--kind`: three arguments are the fault of usage, like with any unknown argument.
        assert_eq!(code, Some(2), "{fehler}");
        assert_eq!(fehler, "wb-dienst: Aufruf: wb-dienst <startdatei.json>\n");
        let (code, aus, fehler) = aufrufen(&["--kind", "gibt-es-nicht", "x"]);
        assert_eq!((code, aus.as_str()), (Some(2), ""), "{fehler}");
        assert_eq!(fehler, "wb-dienst: Aufruf: wb-dienst <startdatei.json>\n");
    }
}

#[test]
fn the_flag_kind_alone_is_a_start_file_in_the_delivered_build() {
    // One argument is the start file, whatever it is called: the delivered program looks for a file `--kind` and finds none; it
    // starts no child and prints one line.
    let (code, aus, fehler) = aufrufen(&["--kind"]);
    assert_eq!(aus, "", "nothing on standard output");
    if cfg!(feature = "pruefbau") {
        // Counter-check: the test build runs the child program `` (no name): it ends with 64 (unknown name).
        assert_eq!(code, Some(64), "{fehler}");
    } else {
        assert_eq!(code, Some(2), "{fehler}");
        assert!(fehler.starts_with("wb-dienst: "), "{fehler}");
        assert_eq!(fehler.lines().count(), 1, "{fehler}");
    }
}

#[test]
fn the_delivered_program_holds_no_text_of_the_child_programs() {
    let programm = fs::read(BIN).expect("program readable");
    let gefunden: Vec<&str> = NAMEN
        .iter()
        .copied()
        .filter(|n| enthaelt(&programm, n))
        .collect();
    if cfg!(feature = "pruefbau") {
        // Counter-check: the test build holds every text, so the search finds what is there.
        assert_eq!(gefunden, NAMEN.to_vec());
    } else {
        assert!(gefunden.is_empty(), "texts in the program: {gefunden:?}");
    }
}
