//! The child part (TR12) with the fixed child programs of `tr --kind`: real children in a real PTY, with fixed bytes. `TERM`,
//! `LC_ALL` and the folder are set; no answer depends on a process id, a path, the time or how long a run takes.

use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration, Instant};

#[cfg(unix)]
use traeger::GroesseUngueltig;
use traeger::{Ereignis, Fehler, Kind, SCHLANGE_MAX, Start};

const TR: &str = env!("CARGO_BIN_EXE_tr");

/// The upper bound of every wait for a state of a child. The tests wait for the state (the loops end at once when it is
/// there); the bound only ends a test that would never get there.
const FRIST: Duration = Duration::from_secs(8);

fn env() -> Vec<(String, String)> {
    [
        ("TERM", "xterm-256color"),
        ("LC_ALL", "C"),
        ("PATH", "/usr/bin:/bin"),
    ]
    .iter()
    .map(|(a, b)| ((*a).to_owned(), (*b).to_owned()))
    .collect()
}

fn start_mit(
    name: &str,
    argumente: &[&str],
    env: Vec<(String, String)>,
    groesse: (usize, usize),
) -> Start {
    let mut argv = vec![TR.to_owned(), "--kind".to_owned(), name.to_owned()];
    argv.extend(argumente.iter().map(|a| (*a).to_owned()));
    Start {
        argv,
        env,
        cwd: PathBuf::from("/"),
        spalten: groesse.0,
        zeilen: groesse.1,
        ruecklauf: 100,
    }
}

fn starten(name: &str, argumente: &[&str]) -> Kind {
    Kind::starten(start_mit(name, argumente, env(), (30, 8))).expect("child starts")
}

/// Reads until a row of the screen holds `gesucht`; the events seen on the way are added to `ereignisse`.
fn bis_text(kind: &mut Kind, gesucht: &str, ereignisse: &mut Vec<Ereignis>) {
    let frist = Instant::now() + FRIST;
    while !kind.raster().klartext().iter().any(|z| z.contains(gesucht)) {
        assert!(
            Instant::now() < frist,
            "{gesucht:?} did not show: {:?}",
            kind.raster().klartext()
        );
        ereignisse.extend(kind.lies(Duration::from_millis(100)));
    }
}

/// Reads until the child has ended and `Kindende` was reported; returns all events.
fn bis_ende(kind: &mut Kind) -> Vec<Ereignis> {
    let frist = Instant::now() + FRIST;
    let mut ereignisse = Vec::new();
    while !kind.ist_zu_ende() {
        assert!(Instant::now() < frist, "the child did not end");
        ereignisse.extend(kind.lies(Duration::from_millis(100)));
    }
    ereignisse
}

/// Whether the process `pid` still exists: `kill -0` on Unix. On Windows `kill` is not the system's (Git for Windows brings
/// one that counts its own process ids), so the list of the system is asked.
#[cfg(unix)]
fn lebt(pid: u32) -> bool {
    Command::new("kill")
        .args(["-0", &pid.to_string()])
        .output()
        .expect("kill runs")
        .status
        .success()
}

#[cfg(windows)]
fn lebt(pid: u32) -> bool {
    let aus = Command::new("tasklist")
        .args(["/FI", &format!("PID eq {pid}"), "/FO", "CSV", "/NH"])
        .output()
        .expect("tasklist runs");
    assert!(aus.status.success(), "tasklist fails");
    String::from_utf8_lossy(&aus.stdout).contains(&format!("\"{pid}\""))
}

fn kindende(ereignisse: &[Ereignis]) -> Vec<u32> {
    ereignisse
        .iter()
        .filter_map(|e| match e {
            Ereignis::Kindende { code } => Some(*code),
            _ => None,
        })
        .collect()
}

#[test]
fn the_output_comes_before_kindende_and_the_code_is_the_exit_code() {
    let mut kind = starten("ausgabe", &["686f6c61"]);
    let ereignisse = bis_ende(&mut kind);
    assert_eq!(kind.raster().klartext()[0], "hola");
    assert!(matches!(
        ereignisse.last(),
        Some(Ereignis::Kindende { code: 0 })
    ));
    assert_eq!(kindende(&ereignisse), [0]);
    assert!(
        ereignisse[..ereignisse.len() - 1]
            .iter()
            .all(Ereignis::ist_ausgabe)
    );
    assert!(kind.raster().abbild_gleich());
    // after the end nothing more is read and nothing more is reported
    assert_eq!(kind.lies(Duration::from_millis(10)).len(), 0);
}

#[test]
fn a_piece_of_output_without_an_event_says_that_it_was_output() {
    // the bytes of an unfinished clipboard sequence (`ESC ] 52 ; c ; Q Q`, no end) give no event; a reader that takes "no event" for
    // "nothing there" stops at the first such piece and loses a turn of its loop for each of them
    let mut kind = starten("ausgabe", &["1b5d35323b633b5151"]);
    let frist = Instant::now() + FRIST;
    loop {
        assert!(Instant::now() < frist, "the piece of output did not come");
        let ereignisse = kind.lies(Duration::from_millis(100));
        if kind.daten_gelesen() {
            assert!(ereignisse.is_empty(), "{ereignisse:?}");
            break;
        }
    }
    bis_ende(&mut kind);
    // a call that finds nothing says so
    assert_eq!(kind.lies(Duration::ZERO).len(), 0);
    assert!(!kind.daten_gelesen());
}

#[test]
fn the_exit_code_of_the_child_is_reported_as_it_is() {
    for code in ["0", "1", "7", "255"] {
        let mut kind = starten("ende", &[code]);
        let ereignisse = bis_ende(&mut kind);
        assert_eq!(
            kindende(&ereignisse),
            [code.parse::<u32>().expect("number")]
        );
        assert!(
            ereignisse.iter().all(|e| !e.ist_ausgabe()),
            "no output was written"
        );
    }
}

// Signals exist on Unix only; on Windows `Kindende` carries the exit code (`src/kind.rs`, `kindcode`).
#[cfg(unix)]
#[test]
fn a_child_ended_by_a_signal_reports_128_plus_the_signal_number() {
    for (signal, code) in [("9", 137), ("15", 143), ("1", 129), ("2", 130), ("6", 134)] {
        // The child runs with `ulimit -c 0`: SIGABRT (6) would dump the core, and where the system pipes cores to a helper
        // (`systemd-coredump` on the Linux build host) the child is not gone until the helper is done; that took the test beyond its bound
        // under load (twice on 20.11.2026). The signal and the code the test checks stay the same.
        let mut start = start_mit("signal", &[signal], env(), (30, 8));
        let tr = std::mem::take(&mut start.argv);
        start.argv = ["sh", "-c", "ulimit -c 0; exec \"$@\"", "sh"]
            .iter()
            .map(|a| (*a).to_owned())
            .chain(tr)
            .collect();
        let mut kind = Kind::starten(start).expect("child starts");
        let ereignisse = bis_ende(&mut kind);
        assert_eq!(kindende(&ereignisse), [code], "signal {signal}");
    }
}

// Signals exist on Unix only; on Windows `Kindende` carries the exit code (`src/kind.rs`, `kindcode`).
#[cfg(unix)]
#[test]
fn a_child_that_writes_and_is_then_ended_by_a_signal_has_its_output_before_kindende() {
    let mut kind = starten("signal", &["15", "686f6c61"]);
    let ereignisse = bis_ende(&mut kind);
    assert_eq!(kind.raster().klartext()[0], "hola");
    assert_eq!(kindende(&ereignisse), [143]);
    assert!(matches!(
        ereignisse.last(),
        Some(Ereignis::Kindende { code: 143 })
    ));
    assert!(kind.raster().abbild_gleich());
}

#[test]
fn an_exit_code_above_128_is_not_taken_for_a_signal() {
    // a child that ends with the code 143 itself is no child ended by a signal; the code stays as it is
    let mut kind = starten("ende", &["143"]);
    let ereignisse = bis_ende(&mut kind);
    assert_eq!(kindende(&ereignisse), [143]);
}

#[test]
fn the_lines_of_a_long_output_all_arrive_and_the_scrollback_is_kept() {
    let mut kind = starten("zeilen", &["500"]);
    let ereignisse = bis_ende(&mut kind);
    assert_eq!(kindende(&ereignisse), [0]);
    let raster = kind.raster();
    assert_eq!(raster.klartext()[6], "Z000500");
    // 500 lines, 8 rows, the cursor in the last row: 500 + 1 - 8 lines are in the scrollback, but only 100 are kept
    assert_eq!(raster.ruecklauf_zahl(), 100);
    let (erste, letzte) = raster.ruecklauf_enden().expect("scrollback is not empty");
    assert_eq!(erste, "Z000394");
    assert_eq!(letzte, "Z000493");
    assert!(raster.abbild_gleich());
}

#[test]
fn the_environment_is_exactly_the_given_one() {
    let mut kind = Kind::starten(start_mit(
        "umgebung",
        &["TERM", "LC_ALL", "HOME", "USER", "TR_ZUSATZ", "A"],
        vec![
            ("TERM".to_owned(), "vt100".to_owned()),
            ("LC_ALL".to_owned(), "C".to_owned()),
            ("A".to_owned(), "b c".to_owned()),
            ("PATH".to_owned(), "/usr/bin:/bin".to_owned()),
        ],
        (30, 8),
    ))
    .expect("child starts");
    bis_ende(&mut kind);
    let zeilen = kind.raster().klartext();
    assert_eq!(
        &zeilen[..6],
        [
            "TERM=vt100",
            "LC_ALL=C",
            "HOME=-",
            "USER=-",
            "TR_ZUSATZ=-",
            "A=b c"
        ]
    );
}

#[test]
fn the_environment_has_no_variable_the_start_did_not_give() {
    // `portable-pty` alone would add `SHELL` (TR15); with `SHELL` in the start it stays as given
    for (env, erwartet) in [
        (
            vec![("PATH", "/usr/bin:/bin"), ("A", "b c")],
            vec!["A=b c", "PATH=/usr/bin:/bin"],
        ),
        (
            vec![("PATH", "/usr/bin:/bin"), ("SHELL", "/bin/zz")],
            vec!["PATH=/usr/bin:/bin", "SHELL=/bin/zz"],
        ),
    ] {
        let env = env
            .iter()
            .map(|(a, b)| ((*a).to_owned(), (*b).to_owned()))
            .collect();
        let mut kind =
            Kind::starten(start_mit("umgebung_alle", &[], env, (40, 8))).expect("child starts");
        bis_ende(&mut kind);
        let zeilen: Vec<String> = kind
            .raster()
            .klartext()
            .into_iter()
            .filter(|z| !z.is_empty())
            .collect();
        assert_eq!(zeilen, erwartet);
    }
}

// The carrier searches the program itself on Unix only (`src/kind.rs`, `befehl_bauen`); on Windows `portable-pty` hands a
// name it does not find in `PATH` to `CreateProcessW`, which also searches the system folders. The case for Windows is below.
#[cfg(unix)]
#[test]
fn a_program_without_a_path_is_found_by_the_path_of_the_start() {
    let mut start = start_mit("ende", &[], env(), (30, 8));
    start.argv = vec!["sh".to_owned(), "-c".to_owned(), "exit 5".to_owned()];
    let mut kind = Kind::starten(start).expect("sh is found in /usr/bin:/bin");
    assert_eq!(kindende(&bis_ende(&mut kind)), vec![5]);
    let mut ohne_pfad = start_mit("ende", &[], Vec::new(), (30, 8));
    ohne_pfad.argv = vec!["sh".to_owned()];
    assert!(Kind::starten(ohne_pfad).is_err(), "no PATH, no search");
}

/// On Windows the program `tr` is found by the folder in the `PATH` of the start (with the extension `.exe`).
#[cfg(windows)]
#[test]
fn a_program_without_a_path_is_found_by_the_path_of_the_start() {
    let ordner = std::path::Path::new(TR)
        .parent()
        .expect("tr is in a folder");
    let mut start = start_mit(
        "ende",
        &[],
        vec![("PATH".to_owned(), ordner.display().to_string())],
        (30, 8),
    );
    start.argv = ["tr", "--kind", "ende", "5"]
        .iter()
        .map(|a| (*a).to_owned())
        .collect();
    let mut kind = Kind::starten(start).expect("tr is found in the PATH of the start");
    assert_eq!(kindende(&bis_ende(&mut kind)), vec![5]);
}

#[test]
fn a_cwd_that_is_not_a_folder_is_an_error_and_never_a_fall_back() {
    for kein_ordner in ["/nonexistent-folder-of-traeger", "/usr/bin/env", ""] {
        let mut start = start_mit("ordner", &[], env(), (30, 8));
        start.cwd = PathBuf::from(kein_ordner);
        let fehler = Kind::starten(start)
            .err()
            .expect("a cwd that is no folder is an error");
        assert!(matches!(fehler, Fehler::Start(_)));
        assert_eq!(
            fehler.to_string(),
            format!("cwd ist kein Ordner: {kein_ordner}")
        );
    }
}

#[test]
fn the_working_folder_is_the_given_one() {
    // the folders exist on the system of the run; the child names its folder as the system writes it
    #[cfg(unix)]
    let ordner_liste = ["/".to_owned(), "/usr".to_owned()];
    #[cfg(not(unix))]
    let ordner_liste = {
        let tr = std::path::Path::new(TR)
            .parent()
            .expect("tr is in a folder");
        let ober = tr.parent().expect("the folder of tr is in a folder");
        [tr.display().to_string(), ober.display().to_string()]
    };
    for ordner in ordner_liste {
        // wide enough for the line of the child: a longer line wraps into the second row of the grid (the folder of
        // the GitHub runner on Windows has more than 30 characters)
        let spalten = (format!("ORDNER {ordner}").chars().count() + 1).max(30);
        let mut start = start_mit("ordner", &[], env(), (spalten, 8));
        start.cwd = PathBuf::from(&ordner);
        let mut kind = Kind::starten(start).expect("child starts");
        bis_ende(&mut kind);
        assert_eq!(kind.raster().klartext()[0], format!("ORDNER {ordner}"));
    }
}

// The child switches its terminal with `stty` (raw mode, `stty size`), which exists on Unix only; the cases of the
// contract with such a child carry `system: ["linux", "macos"]` (TR16) for the same reason.
#[cfg(unix)]
#[test]
fn keys_reach_the_child_unchanged_and_the_size_reaches_the_pty() {
    let mut kind = starten("eingabe", &["4"]);
    let mut ereignisse = Vec::new();
    bis_text(&mut kind, "BEREIT", &mut ereignisse);
    kind.schreibe(&[0x0d, 0x1b, 0x00, 0x7f])
        .expect("keys are written");
    bis_text(&mut kind, "EINGABE 0d1b007f", &mut ereignisse);
    ereignisse.extend(bis_ende(&mut kind));
    assert_eq!(kindende(&ereignisse), [0]);

    let mut kind = starten("groesse", &[]);
    let mut ereignisse = Vec::new();
    bis_text(&mut kind, "BEREIT", &mut ereignisse);
    let aus_groesse = kind.groesse(40, 5).expect("size is valid");
    assert_eq!(kind.raster().groesse_jetzt(), (40, 5));
    ereignisse.extend(aus_groesse);
    kind.schreibe(b"\n").expect("line is written");
    bis_text(&mut kind, "GROESSE 5 40", &mut ereignisse);
    ereignisse.extend(bis_ende(&mut kind));
    assert_eq!(kindende(&ereignisse), [0]);
}

#[test]
fn a_size_outside_the_limits_never_starts_a_child() {
    // the check comes first: the error is the size, not a PTY or a child
    for groesse in [(1, 8), (0, 8), (30, 0), (1001, 8), (30, 1001)] {
        let fehler = Kind::starten(start_mit("ende", &["0"], env(), groesse))
            .err()
            .expect("the size is refused");
        assert_eq!(fehler, Fehler::GroesseUngueltig, "{groesse:?}");
        assert_eq!(fehler.to_string(), "groesse_ungueltig");
    }
    // a refused size beats an empty argv as well
    let mut leer = start_mit("ende", &[], env(), (1, 8));
    leer.argv.clear();
    assert_eq!(
        Kind::starten(leer).err(),
        Some(Fehler::GroesseUngueltig),
        "the size is checked before the argv"
    );
    // the smallest size starts
    let mut kind = Kind::starten(start_mit("ende", &["0"], env(), (2, 1))).expect("2 x 1 starts");
    assert_eq!(kind.raster().groesse_jetzt(), (2, 1));
    let ereignisse = bis_ende(&mut kind);
    assert_eq!(kindende(&ereignisse), [0]);
}

// The child switches its terminal with `stty` (raw mode, `stty size`), which exists on Unix only; the cases of the
// contract with such a child carry `system: ["linux", "macos"]` (TR16) for the same reason.
#[cfg(unix)]
#[test]
fn a_refused_size_leaves_the_pty_and_the_screen_as_they_were() {
    let mut kind = starten("groesse", &[]);
    let mut ereignisse = Vec::new();
    bis_text(&mut kind, "BEREIT", &mut ereignisse);
    let _ = kind.groesse(42, 11).expect("size is valid");
    let zeilen_vorher = kind.raster().zeilen();
    for (spalten, zeilen) in [(1, 11), (0, 11), (42, 0), (1001, 11), (42, 1001)] {
        assert_eq!(kind.groesse(spalten, zeilen), Err(GroesseUngueltig));
    }
    assert_eq!(kind.raster().groesse_jetzt(), (42, 11));
    assert_eq!(kind.raster().zeilen(), zeilen_vorher);
    // the child reads the PTY: still the size that was taken
    kind.schreibe(b"\n").expect("line is written");
    bis_text(&mut kind, "GROESSE 11 42", &mut ereignisse);
    ereignisse.extend(bis_ende(&mut kind));
    assert_eq!(kindende(&ereignisse), [0]);
}

// The child switches its terminal with `stty` (raw mode, `stty size`), which exists on Unix only; the cases of the
// contract with such a child carry `system: ["linux", "macos"]` (TR16) for the same reason.
// Under ConPTY the pseudo console answers the device query itself, before the grid sees it.
#[cfg(unix)]
#[test]
fn the_answer_of_the_grid_to_a_device_query_goes_back_to_the_child() {
    // the child asks DA1 and reports the five bytes it reads back; `ESC [ ? 6 c` is the answer of alacritty_terminal
    let mut kind = starten("abfrage", &[]);
    let mut ereignisse = Vec::new();
    bis_text(&mut kind, "ANTWORT 1b5b3f3663", &mut ereignisse);
    ereignisse.extend(bis_ende(&mut kind));
    assert_eq!(kindende(&ereignisse), [0]);
}

// The child switches its terminal with `stty` (raw mode, `stty size`), which exists on Unix only; the cases of the
// contract with such a child carry `system: ["linux", "macos"]` (TR16) for the same reason.
#[cfg(unix)]
#[test]
fn a_child_that_asks_for_the_cursor_gets_the_place_of_the_cursor_in_the_grid() {
    // A109, second case: the child writes `ESC[6n` and waits 1 s at most. The place the carrier names is the cursor of its
    // grid at the query (row 3, column 7, counted from 1), the second query after `xy` names column 9.
    // the bytes `ab ESC[3;7H ESC[6n xy ESC[6n`
    let hex = "61621b5b333b37481b5b366e78791b5b366e";
    // wide enough that the line `ANTWORT …` does not wrap
    let mut kind =
        Kind::starten(start_mit("cursor", &[hex], env(), (60, 8))).expect("child starts");
    let mut ereignisse = Vec::new();
    bis_text(&mut kind, "ANTWORT", &mut ereignisse);
    ereignisse.extend(bis_ende(&mut kind));
    let zeilen = kind.raster().klartext();
    let antwort = zeilen
        .iter()
        .find_map(|z| z.strip_prefix("ANTWORT "))
        .expect("the child reports an answer");
    assert_eq!(antwort, "1b5b333b37521b5b333b3952", "{zeilen:?}");
    assert_eq!(kindende(&ereignisse), [0]);
}

#[test]
fn a_title_and_the_end_come_as_events_in_order() {
    let mut kind = starten("titel", &["Werkbank_eins"]);
    let ereignisse = bis_ende(&mut kind);
    let rest: Vec<&Ereignis> = ereignisse.iter().filter(|e| !e.ist_ausgabe()).collect();
    assert_eq!(
        rest,
        [
            &Ereignis::Titel {
                text: "Werkbank_eins".to_owned()
            },
            &Ereignis::Kindende { code: 0 }
        ]
    );
}

#[test]
fn a_child_that_still_runs_is_ended_and_collected_when_the_session_is_dropped() {
    let mut kind = starten("eingabe", &["5"]);
    let mut ereignisse = Vec::new();
    bis_text(&mut kind, "BEREIT", &mut ereignisse);
    let pid = kind.pid().expect("the child has a process id");
    assert!(lebt(pid), "the child runs");
    drop(kind);
    assert!(!lebt(pid), "the child is gone and collected");
}

#[test]
fn a_start_that_cannot_work_is_an_error() {
    let mut leer = start_mit("ende", &[], env(), (30, 8));
    leer.argv.clear();
    assert!(Kind::starten(leer).is_err());
    let mut fehlt = start_mit("ende", &[], env(), (30, 8));
    fehlt.argv = vec!["/nonexistent-program-of-traeger".to_owned()];
    assert!(Kind::starten(fehlt).is_err());
}

#[test]
fn the_error_of_a_start_says_what_is_wrong() {
    let mut leer = start_mit("ende", &[], env(), (30, 8));
    leer.argv.clear();
    let fehler = Kind::starten(leer)
        .err()
        .expect("an empty argv is an error");
    assert_eq!(fehler.to_string(), "argv ist leer");
}

/// Starts the child that never reads its input.
fn stummes_kind() -> Kind {
    let mut kind = starten("stumm", &[]);
    bis_text(&mut kind, "BEREIT", &mut Vec::new());
    kind
}

#[test]
fn schreibe_never_blocks_when_the_child_does_not_read() {
    let mut kind = stummes_kind();
    let start = Instant::now();
    // 200 KiB are far above what the PTY takes (about 4 KiB on Linux); the old write would have waited for ever
    assert!(kind.schreibe(&vec![b'a'; 200_000]).is_ok());
    assert!(kind.schreibe(&vec![b'b'; 62_144]).is_ok());
    assert!(start.elapsed() < FRIST, "returned at once");
    let pid = kind.pid().expect("the child has a process id");
    drop(kind);
    assert!(!lebt(pid), "the stuck child is ended and collected");
}

#[test]
fn schreibe_refuses_a_write_that_does_not_fit_the_queue_and_queues_nothing_of_it() {
    let mut kind = stummes_kind();
    let fehler = kind
        .schreibe(&vec![b'c'; SCHLANGE_MAX + 1])
        .expect_err("more than the queue holds");
    assert_eq!(fehler.kind(), std::io::ErrorKind::WouldBlock);
    // the queue is still empty, so a write of exactly the limit fits
    assert!(kind.schreibe(&vec![b'd'; SCHLANGE_MAX]).is_ok());
    assert_eq!(
        kind.schreibe(&vec![b'e'; SCHLANGE_MAX])
            .expect_err("the queue holds the first write")
            .kind(),
        std::io::ErrorKind::WouldBlock
    );
}

// The child switches its terminal with `stty` (raw mode, `stty size`), which exists on Unix only; the cases of the
// contract with such a child carry `system: ["linux", "macos"]` (TR16) for the same reason.
#[cfg(unix)]
#[test]
fn the_child_cursor_gets_the_answer_of_the_carrier_to_its_cursor_query() {
    // "alpha", then the cursor to row 3 column 7, then `ESC [ 6 n`: the answer is `ESC [ 3 ; 7 R` (the position plus 1)
    let anfrage = "616c7068611b5b333b37481b5b366e";
    let mut kind =
        Kind::starten(start_mit("cursor", &[anfrage], env(), (70, 8))).expect("child starts");
    let ereignisse = bis_ende(&mut kind);
    assert_eq!(kindende(&ereignisse), vec![0]);
    let zeilen = kind.raster().klartext();
    assert!(
        zeilen.iter().any(|z| z == "ANTWORT 1b5b333b3752"),
        "{zeilen:?}"
    );
}
