//! The visible outside cases of the contract `bruecke` (`tests/aussen/aussentests/`, Datensatz A) against the program
//! `wb-bruecke`, run by the outside runner of the contract (`tests/aussen/test_aussen.py`, unchanged copy) with the check
//! client `pruefclient.py` and the Python reference of the service (`referenz/dienst_ref.py`) as the other side.
//!
//! Every case runs in its own runner (`python3 -I -B test_aussen.py <case>`), a few at once. The runner ends the process
//! group of the case after each case. Needs `python3` and `lsof`. The expectations are the contract's; this test does not
//! change them.
#![cfg(unix)] // the crate is Unix only (see `src/lib.rs`)

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

const PROGRAMM: &str = env!("CARGO_BIN_EXE_wb-bruecke");
/// At most this many cases at once; the time cases need a machine that is not overloaded.
const GLEICHZEITIG: usize = 6;

fn ordner() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/aussen")
}

fn namen() -> Vec<String> {
    let mut namen: Vec<String> = fs::read_dir(ordner().join("aussentests/faelle"))
        .expect("the case folder is readable")
        .map(|e| e.expect("an entry").path())
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .map(|p| {
            p.file_stem()
                .expect("a name")
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    namen.sort();
    namen
}

#[test]
fn every_visible_case_is_green() {
    let namen = namen();
    assert!(namen.len() >= 60, "only {} cases found", namen.len());
    let basis = PathBuf::from("/tmp").join(format!("wbbr-aussen-{}", std::process::id()));
    fs::create_dir_all(&basis).expect("a base folder for the runs");
    let naechster = AtomicUsize::new(0);
    let rot: Mutex<Vec<String>> = Mutex::new(Vec::new());
    std::thread::scope(|s| {
        for _ in 0..GLEICHZEITIG {
            s.spawn(|| {
                loop {
                    let i = naechster.fetch_add(1, Ordering::SeqCst);
                    let Some(name) = namen.get(i) else { return };
                    let aus = Command::new("python3")
                        .args(["-I", "-B", "test_aussen.py", name])
                        .current_dir(ordner())
                        .env("BR_BIN", PROGRAMM)
                        .env("BR_FAELLE", ordner().join("aussentests"))
                        .env("BR_TMP", &basis)
                        .env("BR_ZEIT", "120")
                        .output()
                        .expect("python3 starts");
                    if !aus.status.success() {
                        rot.lock().expect("lock").push(format!(
                            "{name}: exit {:?}\n{}{}",
                            aus.status.code(),
                            String::from_utf8_lossy(&aus.stdout),
                            String::from_utf8_lossy(&aus.stderr)
                        ));
                    }
                }
            });
        }
    });
    let _ = fs::remove_dir_all(&basis);
    let rot = rot.into_inner().expect("lock");
    assert!(
        rot.is_empty(),
        "{} of {} cases are red:\n{}",
        rot.len(),
        namen.len(),
        rot.join("\n")
    );
}
