//! The program `werkbank-einschluss` on the ways that end before the harness: a refused call, a refused input, the
//! missing working folder and a self test that cannot build its enclosure. None of them needs the stand-in harness, so
//! these cases run in the default build too (and in the mutation run, which builds without the feature `attrappe`).
//! The cases with the stand-in are in `tests/starter_programm.rs`.
//!
//! Descriptor 3 is put in place by the shell (`3<&0`, `3<file`): safe Rust cannot map a descriptor onto a number.
#![cfg(target_os = "linux")]

use std::fs;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};

use einschluss::{Beschreibung, System, Umgebung, Zustand, beschreibung_schreiben};

const STARTER: &str = env!("CARGO_BIN_EXE_werkbank-einschluss");
const START: [&str; 3] = ["start", "--beschreibung-fd", "3"];
const FEST: &str = "lauf.einschluss_fehlt: Einschluss nicht verfuegbar\n";
const FRIST: Duration = Duration::from_secs(60);

static ZAEHLER: AtomicU32 = AtomicU32::new(0);

/// An own empty folder of this test.
fn ordner() -> PathBuf {
    let n = ZAEHLER.fetch_add(1, Ordering::SeqCst);
    let basis =
        std::env::var_os("EINSCHLUSS_TESTORT").map_or_else(std::env::temp_dir, PathBuf::from);
    let p = basis.join(format!("ei-sa-{}-{n}", std::process::id()));
    let _ = fs::remove_dir_all(&p);
    fs::create_dir_all(&p).expect("folder");
    p
}

/// A valid description whose working folder does not exist: the starter reads and builds it, and refuses it at the
/// first check of the host, before any `bwrap`.
fn ohne_arbeitsordner() -> String {
    let b = Beschreibung {
        harness: "attrappe".to_owned(),
        arbeitsordner: PathBuf::from("/srv/beispiel/arbeit"),
        programmordner: Vec::new(),
        zustand: Zustand {
            host: PathBuf::from("/srv/beispiel/zustand"),
            name: ".zustand".to_owned(),
            fortsetzen: false,
        },
        heim: PathBuf::from("/srv/beispiel/heim"),
        such_socket: PathBuf::from("/srv/beispiel/such/s"),
        modell_socket: None,
        lokal_weiter: None,
        dateien: Vec::new(),
        cloud: None,
        paket: None,
        umgebung: Umgebung {
            home: "/srv/beispiel/heim".to_owned(),
            path: "/usr/bin".to_owned(),
            term: "xterm-256color".to_owned(),
            lang: "C.UTF-8".to_owned(),
            tmpdir: "/tmp".to_owned(),
        },
        geschuetzt: Vec::new(),
        argv: vec!["/usr/bin/true".to_owned()],
        system: System::vom_host().expect("host system"),
    };
    beschreibung_schreiben(&b)
}

/// What stands on descriptor 3.
enum Fd3<'a> {
    /// Nothing: the descriptor is not open.
    Keine,
    /// A pipe that carries this text and ends.
    Text(&'a str),
    /// A regular file with this content.
    Datei(&'a str),
}

/// Runs the starter with `args`, an empty environment and descriptor 3 as given; waits at most [`FRIST`]. The exit
/// code, standard output and standard error.
fn starter(args: &[&str], fd3: &Fd3) -> (Option<i32>, String, String) {
    let mut c = Command::new("/bin/sh");
    let ort = ordner();
    match fd3 {
        Fd3::Keine => {
            // descriptor 3 is closed explicitly: the test runner may have handed a descriptor of its own down
            c.args(["-c", "exec \"$@\" 3<&-", "sh"]);
        }
        Fd3::Text(_) => {
            c.args(["-c", "exec \"$@\" 3<&0", "sh"]);
        }
        Fd3::Datei(text) => {
            let datei = ort.join("eingabe.json");
            fs::write(&datei, text).expect("input file");
            c.args(["-c", "f=$1; shift; exec \"$@\" 3<\"$f\"", "sh"])
                .arg(&datei);
        }
    }
    c.arg(STARTER)
        .args(args)
        .env_clear()
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut kind = c.spawn().expect("starts");
    let mut eingang = kind.stdin.take().expect("stdin");
    if let Fd3::Text(t) = fd3 {
        // the starter may stop reading early; the write may then fail
        let _ = eingang.write_all(t.as_bytes());
    }
    drop(eingang);
    let ende = Instant::now() + FRIST;
    let status = loop {
        if let Some(s) = kind.try_wait().expect("wait") {
            break s.code();
        }
        if Instant::now() > ende {
            let _ = kind.kill();
            let _ = kind.wait();
            break None;
        }
        std::thread::sleep(Duration::from_millis(5));
    };
    let (mut aus, mut fehler) = (String::new(), String::new());
    if let Some(mut s) = kind.stdout.take() {
        let _ = s.read_to_string(&mut aus);
    }
    if let Some(mut s) = kind.stderr.take() {
        let _ = s.read_to_string(&mut fehler);
    }
    let _ = fs::remove_dir_all(&ort);
    (status, aus, fehler)
}

fn mit_teil(code: i32, wort: &str) -> String {
    format!("{FEST}lauf.einschluss_teil: {code} {wort}\n")
}

#[test]
fn a_foreign_call_ends_with_2_and_the_fixed_line() {
    for args in [
        &[][..],
        &["roh"][..],
        &["start"][..],
        &["start", "--beschreibung-fd", "4"][..],
        &["selbsttest", "extra"][..],
        &["vorentpacken"][..],
    ] {
        assert_eq!(
            starter(args, &Fd3::Text("{}")),
            (
                Some(2),
                String::new(),
                "werkbank-einschluss: Aufruf ungueltig\n".to_owned()
            ),
            "{args:?}"
        );
    }
}

#[test]
fn the_input_is_read_from_a_pipe_only() {
    let gut = ohne_arbeitsordner();
    // a pipe with a valid text: read, built and refused at the missing working folder (exit 4, no second line)
    assert_eq!(
        starter(&START, &Fd3::Text(&gut)),
        (
            Some(4),
            String::new(),
            "lauf.ordner_fehlt: Arbeitsordner fehlt oder ist kein Ordner\n".to_owned()
        )
    );
    // the same text in a regular file: refused before it is read
    assert_eq!(
        starter(&START, &Fd3::Datei(&gut)),
        (Some(3), String::new(), mit_teil(3, "eingabe-kein-rohr"))
    );
    // no descriptor 3
    assert_eq!(
        starter(&START, &Fd3::Keine),
        (Some(3), String::new(), mit_teil(3, "eingabe-nicht-lesbar"))
    );
    // a pipe with a text that is no description
    assert_eq!(
        starter(&START, &Fd3::Text("{}")),
        (Some(3), String::new(), mit_teil(3, "eingabe-ungueltig"))
    );
    // the small enclosure takes its input the same way
    assert_eq!(
        starter(
            &["vorentpacken", "--beschreibung-fd", "3"],
            &Fd3::Datei("{}")
        ),
        (Some(3), String::new(), mit_teil(3, "eingabe-kein-rohr"))
    );
}

#[test]
fn selbsttest_fails_where_the_enclosure_cannot_be_built() {
    // inside an enclosure that forbids further user namespaces the inner `bwrap` cannot start: the self test must say
    // so, with the fixed line, its second line and the exit code of the namespace (7) or of the test run (8)
    let aus = Command::new("/usr/bin/bwrap")
        .args([
            "--unshare-user",
            "--disable-userns",
            "--ro-bind",
            "/usr",
            "/usr",
            "--symlink",
            "usr/lib",
            "/lib",
            "--symlink",
            "usr/lib64",
            "/lib64",
            "--symlink",
            "usr/bin",
            "/bin",
            "--proc",
            "/proc",
            "--dev",
            "/dev",
            "--ro-bind",
            STARTER,
            "/starter",
            "/starter",
            "selbsttest",
        ])
        .env_clear()
        .stdin(Stdio::null())
        .output()
        .expect("bwrap runs");
    let fehler = String::from_utf8_lossy(&aus.stderr).into_owned();
    let code = aus.status.code();
    assert!(matches!(code, Some(7 | 8)), "{:?}: {fehler:?}", aus.status);
    let wort = fehler
        .strip_prefix(FEST)
        .and_then(|r| r.strip_prefix(&format!("lauf.einschluss_teil: {} ", code.unwrap_or(0))))
        .and_then(|w| w.strip_suffix('\n'));
    assert!(
        wort.is_some_and(
            |w| !w.is_empty() && w.bytes().all(|b| b.is_ascii_lowercase() || b == b'-')
        ),
        "{fehler:?}"
    );
    assert_eq!(String::from_utf8_lossy(&aus.stdout), "");
}
