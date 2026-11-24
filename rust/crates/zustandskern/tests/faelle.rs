//! Case tests: every file under `tests/faelle/` runs through the library and is compared with
//! `tests/erwartet/<name>.json` (`exit`, and `ausgabe` for exit 0).
//!
//! `r*` and `x_*` are the visible outside cases of the contract package (compacted copies); `g_*` are cases made by a
//! random generator whose expected output comes from the Python reference of the contract (Python 3.15.2, run with
//! `zk-ref`); `h_*` are written by hand for the notes in the contract text. All data are invented.

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

fn ordner(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join(name)
}

fn lesen(pfad: &PathBuf) -> Value {
    let text = fs::read_to_string(pfad).expect("file is readable");
    serde_json::from_str(&text).expect("file is JSON")
}

/// The first difference of `ist` against `soll`, as a path.
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
                    _ => return Some(format!("{pfad}.{k}: fehlt oder zu viel")),
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
            (a.len() != b.len()).then(|| format!("{pfad}: Laenge {} statt {}", a.len(), b.len()))
        }
        _ => (ist != soll).then(|| format!("{pfad}: {ist} statt {soll}")),
    }
}

#[test]
fn every_case_gives_the_expected_effects() {
    let mut namen: Vec<String> = fs::read_dir(ordner("faelle"))
        .expect("case folder exists")
        .map(|e| e.expect("entry").file_name().to_string_lossy().into_owned())
        .filter(|n| Path::new(n).extension().is_some_and(|e| e == "json"))
        .collect();
    namen.sort();
    assert!(namen.len() >= 250, "only {} cases found", namen.len());
    let mut rot = Vec::new();
    for name in &namen {
        let fall = fs::read(ordner("faelle").join(name)).expect("case is readable");
        let erwartet = lesen(&ordner("erwartet").join(name));
        let ergebnis = zustandskern::laufen(&fall);
        let grund = match (erwartet["exit"].as_i64(), ergebnis) {
            (Some(0), Ok(ist)) => unterschied(&ist, &erwartet["ausgabe"], "$"),
            (Some(0), Err(e)) => Some(format!("Fehler statt Ausgabe: {e}")),
            (Some(2), Err(_)) => None,
            (Some(2), Ok(_)) => Some("Ausgabe statt Fehler".to_owned()),
            (andere, _) => Some(format!("unerwarteter Exitcode {andere:?} in der Erwartung")),
        };
        if let Some(g) = grund {
            rot.push(format!("{name}: {g}"));
        }
    }
    assert!(
        rot.is_empty(),
        "{} von {} rot:\n{}",
        rot.len(),
        namen.len(),
        rot.join("\n")
    );
}
