//! The sources of the crate have no side effects: the library imports nothing that reads or writes the outside
//! world, and the program `src/main.rs` does only what the order allows.
//!
//! The test reads the sources with `include_str!` and fails on every forbidden pattern. A second test checks that
//! the list of sources below holds every `.rs` file under `src/`, so a new file cannot slip past the scan.

use std::fs;
use std::path::{Path, PathBuf};

/// Patterns that must not occur in any source under `src/` except `src/main.rs`.
const VERBOTEN_BIBLIOTHEK: &[&str] = &[
    "std::fs",
    "std::io",
    "std::process",
    "std::net",
    "std::env",
    "std::thread",
    "std::time",
    "core::time",
    "use std::{",
    "extern",
];

/// Patterns that must not occur in `src/main.rs`.
const VERBOTEN_PROGRAMM: &[&str] = &[
    "env::var",
    "env::vars",
    "set_var",
    "Command",
    "std::net",
    "File::create",
    "OpenOptions",
    "fs::write",
    "remove_",
    "rename",
    "create_dir",
    "std::thread",
    "SystemTime",
    "Instant",
];

/// Every `.rs` file under `src/`, by path relative to `src/`, with its text.
const QUELLEN: &[(&str, &str)] = &[
    ("bild.rs", include_str!("../src/bild.rs")),
    ("deflate.rs", include_str!("../src/deflate.rs")),
    ("empfang.rs", include_str!("../src/empfang.rs")),
    ("fehler.rs", include_str!("../src/fehler.rs")),
    ("lauf.rs", include_str!("../src/lauf.rs")),
    ("lib.rs", include_str!("../src/lib.rs")),
    ("main.rs", include_str!("../src/main.rs")),
    ("rahmen.rs", include_str!("../src/rahmen.rs")),
    ("schub.rs", include_str!("../src/schub.rs")),
    ("tests/bild.rs", include_str!("../src/tests/bild.rs")),
    ("tests/deflate.rs", include_str!("../src/tests/deflate.rs")),
    ("tests/fehler.rs", include_str!("../src/tests/fehler.rs")),
    ("tests/lauf.rs", include_str!("../src/tests/lauf.rs")),
    ("tests/mod.rs", include_str!("../src/tests/mod.rs")),
    ("tests/rahmen.rs", include_str!("../src/tests/rahmen.rs")),
    ("tests/schub.rs", include_str!("../src/tests/schub.rs")),
    (
        "tests/verbindung.rs",
        include_str!("../src/tests/verbindung.rs"),
    ),
    ("tests/zeile.rs", include_str!("../src/tests/zeile.rs")),
    ("verbindung.rs", include_str!("../src/verbindung.rs")),
    ("zeile.rs", include_str!("../src/zeile.rs")),
];

fn treffer(text: &str, muster: &[&str]) -> Vec<String> {
    let mut gefunden = Vec::new();
    for (nr, zeile) in text.lines().enumerate() {
        for m in muster {
            if zeile.contains(m) {
                gefunden.push(format!("Zeile {}: {m}", nr + 1));
            }
        }
    }
    gefunden
}

#[test]
fn no_source_holds_a_forbidden_pattern() {
    let mut rot = Vec::new();
    for (name, text) in QUELLEN {
        let muster = if *name == "main.rs" {
            VERBOTEN_PROGRAMM
        } else {
            VERBOTEN_BIBLIOTHEK
        };
        for t in treffer(text, muster) {
            rot.push(format!("src/{name}: {t}"));
        }
    }
    assert!(rot.is_empty(), "verbotene Muster:\n{}", rot.join("\n"));
}

#[test]
fn the_scan_names_every_source_file() {
    fn sammeln(ordner: &Path, vor: &Path, aus: &mut Vec<String>) {
        for eintrag in fs::read_dir(ordner).expect("folder is readable") {
            let pfad = eintrag.expect("entry").path();
            if pfad.is_dir() {
                sammeln(&pfad, vor, aus);
            } else if pfad.extension().is_some_and(|e| e == "rs") {
                // the list names a file with `/` between its parts, also on Windows
                let teile: Vec<String> = pfad
                    .strip_prefix(vor)
                    .expect("below src")
                    .components()
                    .map(|t| t.as_os_str().to_string_lossy().into_owned())
                    .collect();
                aus.push(teile.join("/"));
            }
        }
    }
    let src: PathBuf = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut auf_platte = Vec::new();
    sammeln(&src, &src, &mut auf_platte);
    auf_platte.sort();
    let mut gelistet: Vec<String> = QUELLEN.iter().map(|(n, _)| (*n).to_owned()).collect();
    gelistet.sort();
    assert_eq!(auf_platte, gelistet);
}

#[test]
fn the_scan_finds_what_it_looks_for() {
    assert_eq!(
        treffer("a\nuse std::fs;\nb", VERBOTEN_BIBLIOTHEK),
        vec!["Zeile 2: std::fs".to_owned()]
    );
    assert_eq!(
        treffer("let t = Instant::now();", VERBOTEN_PROGRAMM),
        vec!["Zeile 1: Instant".to_owned()]
    );
    assert_eq!(
        treffer("use std::fmt;\nfn f() {}", VERBOTEN_BIBLIOTHEK),
        Vec::<String>::new()
    );
    assert_eq!(
        treffer("std::env::args_os()", VERBOTEN_PROGRAMM),
        Vec::<String>::new()
    );
}
