//! Case tests: every file under `tests/faelle/` runs through the library and through the program `tr`, and the result is
//! compared with `tests/erwartet/<name>.json` (`exit`, and `ausgabe` for exit 0; `datensatz` is metadata).
//!
//! The cases are the visible outside cases of the contract (compacted copies). The expectations come from the reference
//! of the contract (`alacritty_terminal` 0.26.0 plus the extras). All data are invented.
//!
//! Two optional fields of a case belong to this runner (TR16): `system` names the systems where the case applies (on another
//! system it is not run and counts as "not applicable": never green, never red), and `last` (1 to 64) keeps that many busy
//! threads running while the case runs, only with `TR_LAST=1` (without it the case does not apply: "load not allowed", never
//! green, never red). The busy threads belong to this test program, so none outlives it.

use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::Value;

const TR: &str = env!("CARGO_BIN_EXE_tr");

fn ordner(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join(name)
}

fn lesen(pfad: &Path) -> Value {
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

fn namen() -> Vec<String> {
    let mut namen: Vec<String> = fs::read_dir(ordner("faelle"))
        .expect("case folder exists")
        .map(|e| e.expect("entry").file_name().to_string_lossy().into_owned())
        .filter(|n| Path::new(n).extension().is_some_and(|e| e == "json"))
        .collect();
    namen.sort();
    assert!(namen.len() >= 109, "only {} cases found", namen.len());
    namen
}

/// How long one case may take before it counts as hanging (the limit of a step `Warte` is 20 s).
const GRENZE: Duration = Duration::from_secs(5);

/// Runs the program with the case on standard input: exit code and parsed standard output. A program that is still running
/// after `GRENZE` is killed and gives no exit code.
fn programm(fall: &[u8]) -> (Option<i32>, Option<Value>) {
    let mut kind = Command::new(TR)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("tr starts");
    let mut ein = kind.stdin.take().expect("stdin is piped");
    let mut aus = kind.stdout.take().expect("stdout is piped");
    let daten = fall.to_vec();
    let schreiber = std::thread::spawn(move || {
        let _ = ein.write_all(&daten);
    });
    let leser = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        let _ = aus.read_to_end(&mut bytes);
        bytes
    });
    let anfang = Instant::now();
    let code = loop {
        if let Some(status) = kind.try_wait().expect("tr is polled") {
            break status.code();
        }
        if anfang.elapsed() > GRENZE {
            let _ = kind.kill();
            let _ = kind.wait();
            break None;
        }
        std::thread::sleep(Duration::from_millis(5));
    };
    schreiber.join().expect("writer ends");
    let bytes = leser.join().expect("reader ends");
    (code, serde_json::from_slice(&bytes).ok())
}

fn pruefe(
    rot: &mut Vec<String>,
    name: &str,
    erwartet: &Value,
    code: Option<i32>,
    ist: Option<Value>,
) {
    let grund = match (erwartet["exit"].as_i64(), code, ist) {
        // the first difference, with the lines and the step results of the run (macos-latest, 23.11.2026: a lone
        // `$.ende.cursor[0]: 0 statt 1` did not tell whether the line had come at all)
        (Some(0), Some(0), Some(ist)) => unterschied(&ist, &erwartet["ausgabe"], "$").map(|d| {
            format!(
                "{d} (ist: ende.zeilen {}, ergebnisse {})",
                ist["ende"]["zeilen"], ist["ergebnisse"]
            )
        }),
        (Some(2), Some(2), None) => None,
        (soll, ist_code, _) => Some(format!("Exitcode {ist_code:?} statt {soll:?}")),
    };
    if let Some(g) = grund {
        rot.push(format!("{name}: {g}"));
    }
}

/// The system of this run: `linux`, `macos` or `windows`.
const SYSTEM: &str = std::env::consts::OS;

/// A case without the field `system` applies everywhere; with it, only on the systems it names (TR16).
fn gilt_hier(fall: &Value, system: &str) -> bool {
    fall.get("system")
        .and_then(Value::as_array)
        .is_none_or(|liste| liste.iter().any(|s| s.as_str() == Some(system)))
}

/// Load is allowed only with `TR_LAST=1` (the machine says so: the Linux build host under the board).
fn last_erlaubt(wert: Option<&str>) -> bool {
    wert == Some("1")
}

/// The number of busy threads a case asks for with `last` (1 to 64), or 0 (TR16).
fn last_von(fall: &Value) -> usize {
    fall.get("last")
        .and_then(Value::as_u64)
        .filter(|n| (1..=64).contains(n))
        .map_or(0, |n| usize::try_from(n).unwrap_or(0))
}

/// Busy threads for the time of one case. They end at `drop`, after the longest time (100 s) and with the test program.
struct Last {
    halt: Arc<AtomicBool>,
    faeden: Vec<std::thread::JoinHandle<()>>,
}

impl Last {
    fn starten(n: usize) -> Self {
        let halt = Arc::new(AtomicBool::new(false));
        let ende = Instant::now() + Duration::from_secs(100);
        let faeden = (0..n)
            .map(|_| {
                let halt = Arc::clone(&halt);
                std::thread::spawn(move || {
                    while !halt.load(Ordering::Relaxed) && Instant::now() < ende {
                        for _ in 0..20_000 {
                            std::hint::spin_loop();
                        }
                    }
                })
            })
            .collect();
        Self { halt, faeden }
    }
}

impl Drop for Last {
    fn drop(&mut self) {
        self.halt.store(true, Ordering::Relaxed);
        for faden in self.faeden.drain(..) {
            let _ = faden.join();
        }
    }
}

/// What a run did: the red cases, the cases that ran, and the cases that do not apply here (never run, never green).
struct Lauf {
    rot: Vec<String>,
    gelaufen: Vec<String>,
    na: Vec<String>,
}

/// Runs every case that applies with `lauf`: the cases without `last` on a few threads at once (a case that hangs then costs
/// its own time, not the sum), then the cases with `last` one after the other, each with its busy threads.
fn alle(lauf: impl Fn(&[u8]) -> (Option<i32>, Option<Value>) + Sync) -> Lauf {
    let mut na = Vec::new();
    let mut normal = Vec::new();
    let mut mit_last = Vec::new();
    let last_frei = last_erlaubt(std::env::var("TR_LAST").ok().as_deref());
    for name in namen() {
        let fall: Option<Value> = fs::read(ordner("faelle").join(&name))
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok());
        match fall {
            Some(f) if !gilt_hier(&f, SYSTEM) => na.push(name),
            Some(f) if last_von(&f) > 0 && !last_frei => na.push(name),
            Some(f) if last_von(&f) > 0 => mit_last.push((name, last_von(&f))),
            _ => normal.push(name),
        }
    }
    let rot = Mutex::new(Vec::new());
    let eins = |name: &str| {
        let fall = fs::read(ordner("faelle").join(name)).expect("case is readable");
        let erwartet = lesen(&ordner("erwartet").join(name));
        let (code, ist) = lauf(&fall);
        let mut eigene = Vec::new();
        pruefe(&mut eigene, name, &erwartet, code, ist);
        rot.lock().expect("lock").extend(eigene);
    };
    let naechster = AtomicUsize::new(0);
    std::thread::scope(|bereich| {
        for _ in 0..12 {
            bereich.spawn(|| {
                loop {
                    let i = naechster.fetch_add(1, Ordering::SeqCst);
                    let Some(name) = normal.get(i) else { break };
                    eins(name);
                }
            });
        }
    });
    for (name, n) in &mit_last {
        let _last = Last::starten(*n);
        eins(name);
    }
    let mut rot = rot.into_inner().expect("lock");
    rot.sort();
    let mut gelaufen = normal;
    gelaufen.extend(mit_last.into_iter().map(|(name, _)| name));
    Lauf { rot, gelaufen, na }
}

/// No red case; every case ran or does not apply here, none does both: a case that does not apply is neither green nor run.
fn bewerte(l: &Lauf) {
    assert!(
        l.rot.is_empty(),
        "{} rot:\n{}",
        l.rot.len(),
        l.rot.join("\n")
    );
    assert_eq!(
        l.gelaufen.len() + l.na.len(),
        namen().len(),
        "every case ran or does not apply"
    );
    assert!(
        l.na.iter().all(|n| !l.gelaufen.contains(n)),
        "a case that does not apply was run"
    );
    if SYSTEM != "windows" {
        assert!(
            l.na.iter().any(|n| n == "tr8_kind_vorspann_titel.json"),
            "the cases of W7 apply only under Windows and must count as not applicable here"
        );
        let lastfall = "tr14_cpr_unter_last.json";
        if last_erlaubt(std::env::var("TR_LAST").ok().as_deref()) {
            assert!(
                l.gelaufen.iter().any(|n| n == lastfall),
                "the case under load runs with TR_LAST=1"
            );
        } else {
            assert!(
                l.na.iter().any(|n| n == lastfall) && !l.gelaufen.iter().any(|n| n == lastfall),
                "without TR_LAST=1 the case under load does not apply and does not run"
            );
        }
    }
}

#[test]
fn every_case_gives_the_expected_result_through_the_library() {
    let rot = alle(|fall| {
        // The code of the grid has loops that never end for some input: the case runs on its own thread and counts as
        // hanging after `GRENZE` (the thread is left behind and ends with the test program). A short deadline for the
        // steps `Warte`: a step that waits for something that never comes must end the case quickly.
        let (sender, empfaenger) = std::sync::mpsc::channel();
        let daten = fall.to_vec();
        std::thread::spawn(move || {
            let ergebnis =
                match traeger::laufen_mit_frist(&daten, Path::new(TR), Duration::from_secs(3)) {
                    Ok(ist) => (Some(0), Some(ist)),
                    Err(_) => (Some(2), None),
                };
            let _ = sender.send(ergebnis);
        });
        empfaenger.recv_timeout(GRENZE).unwrap_or((None, None))
    });
    bewerte(&rot);
}

#[test]
fn every_case_gives_the_expected_result_through_the_program() {
    bewerte(&alle(programm));
}

#[test]
fn the_fields_system_and_last_are_read() {
    let fall = |text: &str| serde_json::from_str::<Value>(text).expect("JSON");
    assert!(gilt_hier(&fall("{}"), "windows"), "no field: everywhere");
    assert!(gilt_hier(
        &fall(r#"{"system": ["linux", "macos"]}"#),
        "macos"
    ));
    assert!(!gilt_hier(
        &fall(r#"{"system": ["linux", "macos"]}"#),
        "windows"
    ));
    assert!(!gilt_hier(&fall(r#"{"system": ["windows"]}"#), "linux"));
    assert!(
        !gilt_hier(&fall(r#"{"system": []}"#), "linux"),
        "an empty list applies nowhere"
    );
    assert_eq!(last_von(&fall("{}")), 0);
    assert_eq!(last_von(&fall(r#"{"last": 16}"#)), 16);
    assert_eq!(last_von(&fall(r#"{"last": 64}"#)), 64);
    for schlecht in ["0", "65", "-1", "\"x\"", "true", "1.5"] {
        assert_eq!(
            last_von(&fall(&format!(r#"{{"last": {schlecht}}}"#))),
            0,
            "{schlecht}"
        );
    }
}

#[test]
fn load_is_allowed_only_with_tr_last_1() {
    assert!(last_erlaubt(Some("1")));
    for nein in [None, Some(""), Some("0"), Some("2"), Some("true")] {
        assert!(!last_erlaubt(nein), "{nein:?}");
    }
}

#[test]
fn the_busy_threads_end_with_their_guard() {
    let halt = {
        let last = Last::starten(4);
        assert_eq!(last.faeden.len(), 4);
        assert!(
            last.faeden.iter().all(|f| !f.is_finished()),
            "they run while the guard lives"
        );
        Arc::clone(&last.halt)
    };
    assert!(
        halt.load(Ordering::Relaxed),
        "the guard stopped them at drop"
    );
}
