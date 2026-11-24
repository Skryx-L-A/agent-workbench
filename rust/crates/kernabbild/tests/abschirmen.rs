//! The tests of the limit of core images under Linux: the test program of the crate (`kernabbild-probe`) calls `abschirmen`, tells its result and
//! waits; the tests read `/proc/<pid>` (the owner of the files is root while the process cannot be dumped; `limits` shows the limit 1/1). A child of the
//! program inherits the limit 1/1 and can be dumped again after `exec`. A hard limit 0 that the program inherits through `sh -c 'ulimit ...'` leads to
//! the note or to the abort, chosen by the core pattern (a file of the test, and the real pattern of the system).
#![cfg(target_os = "linux")]

use std::io::{BufRead, BufReader};
use std::os::unix::fs::MetadataExt;
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use kernabbild::{Abbruch, Hinweis};
use rustix::process::{Resource, getrlimit};

const PROBE: &str = env!("CARGO_BIN_EXE_kernabbild-probe");

/// The start time of the process `pid` (field 22 of `/proc/<pid>/stat`, in clock ticks since boot), or `None` when there is no such process.
/// The name of the process stands in parentheses and may hold spaces and parentheses: the fields count from behind the last `)`.
fn startzeit(pid: u32) -> Option<u64> {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    let nach_name = &stat[stat.rfind(')')? + 1..];
    // fields from 3 on: state (3), ... starttime (22) is the 20th of them
    nach_name.split_whitespace().nth(19)?.parse().ok()
}

/// The watchdog may signal `pid` only while it is still the process that was started: the start time is the same. A process that was reaped
/// and whose pid went to a foreign process has another start time (or none).
fn derselbe_prozess(pid: u32, startzeit_beim_start: Option<u64>) -> bool {
    startzeit_beim_start.is_some() && startzeit(pid) == startzeit_beim_start
}

/// Starts `befehl` with standard input and output piped and reads its lines until `BEREIT` or the end of its output. A watchdog kills the
/// process after 30 seconds, so that a program that waits where it should not turns the test red and does not hang it. It signals only
/// while the pid is still the started process (`derselbe_prozess`): after the test has reaped the child, the pid may belong to a foreign process.
fn starten(mut befehl: Command) -> (Child, Vec<String>) {
    let mut kind = befehl
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("the probe starts");
    let pid = kind.id();
    let startzeit_beim_start = startzeit(pid);
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(30));
        if derselbe_prozess(pid, startzeit_beim_start) {
            let _ = Command::new("kill").arg("-9").arg(pid.to_string()).status();
        }
    });
    let aus = kind.stdout.take().expect("stdout");
    let mut zeilen = Vec::new();
    let mut leser = BufReader::new(aus);
    loop {
        let mut zeile = String::new();
        if leser.read_line(&mut zeile).expect("read") == 0 {
            break;
        }
        let zeile = zeile.trim_end().to_owned();
        let ende = zeile == "BEREIT";
        zeilen.push(zeile);
        if ende {
            break;
        }
    }
    (kind, zeilen)
}

/// Ends the probe: closing standard input lets it end.
fn beenden(mut kind: Child) {
    drop(kind.stdin.take());
    let _ = kind.wait();
}

/// The soft and the hard value of the line `Max core file size` of `/proc/<pid>/limits`.
fn core_grenze(pid: u32) -> (String, String) {
    let text = std::fs::read_to_string(format!("/proc/{pid}/limits")).expect("limits");
    let zeile = text
        .lines()
        .find(|z| z.starts_with("Max core file size"))
        .expect("the line of the core limit");
    let felder: Vec<&str> = zeile.split_whitespace().collect();
    // Max core file size  <soft>  <hard>  bytes
    (felder[4].to_owned(), felder[5].to_owned())
}

/// The owner of `/proc/<pid>/stat`: the user of the process, or root while the process cannot be dumped (the folder itself stays the user's).
fn eigentuemer(pid: u32) -> u32 {
    std::fs::metadata(format!("/proc/{pid}/stat"))
        .expect("proc")
        .uid()
}

fn eigener_nutzer() -> u32 {
    rustix::process::geteuid().as_raw()
}

/// The test cannot say anything when the test process itself has a hard limit 0 or runs as root: then the probe starts under the other branch.
fn uebergehen() -> bool {
    getrlimit(Resource::Core).maximum == Some(0) || eigener_nutzer() == 0
}

#[test]
fn ein_abgeschirmtes_programm_ist_nicht_abzugsfaehig_und_hat_die_grenze_1_1() {
    if uebergehen() {
        return;
    }
    let (kind, zeilen) = starten(Command::new(PROBE));
    assert_eq!(zeilen, ["OK", "BEREIT"]);
    let pid = kind.id();
    assert_ne!(
        eigentuemer(pid),
        eigener_nutzer(),
        "the files of /proc/<pid> belong to root while the process cannot be dumped"
    );
    assert_eq!(core_grenze(pid), ("1".to_owned(), "1".to_owned()));
    beenden(kind);
}

#[test]
fn ein_kind_erbt_die_grenze_1_1_und_ist_nach_exec_wieder_abzugsfaehig() {
    if uebergehen() {
        return;
    }
    let (kind, zeilen) = starten({
        let mut befehl = Command::new(PROBE);
        befehl.arg("kind");
        befehl
    });
    assert_eq!(zeilen.len(), 3, "{zeilen:?}");
    assert_eq!(zeilen[0], "OK");
    let kind_pid: u32 = zeilen[1]
        .strip_prefix("KIND ")
        .expect("the line KIND")
        .parse()
        .expect("a pid");
    assert_eq!(zeilen[2], "BEREIT");
    // the child inherits 1/1 and, after `exec` of `sleep`, can be dumped again: its files belong to the user
    assert_eq!(core_grenze(kind_pid), ("1".to_owned(), "1".to_owned()));
    assert_eq!(eigentuemer(kind_pid), eigener_nutzer());
    // the parent stays shielded
    assert_ne!(eigentuemer(kind.id()), eigener_nutzer());
    beenden(kind);
}

/// The probe started under an inherited hard limit 0: `sh -c 'ulimit -c 0; ulimit -H -c 0; exec "$0" "$@"' <probe> <arguments>`.
fn mit_harter_null(argumente: &[&str]) -> Command {
    let mut befehl = Command::new("sh");
    befehl
        .arg("-c")
        .arg("ulimit -c 0; ulimit -H -c 0; exec \"$0\" \"$@\"")
        .arg(PROBE)
        .args(argumente);
    befehl
}

#[test]
fn eine_geerbte_harte_null_gibt_je_nach_muster_ein_protokoll_oder_den_abbruch() {
    let ordner = std::env::temp_dir().join(format!("kernabbild-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&ordner);
    std::fs::create_dir_all(&ordner).expect("folder");
    let datei = ordner.join("core_pattern");
    let pfad = datei.to_string_lossy().into_owned();
    // a file pattern: the program goes on with the note, the limit stays 0/0 (it cannot be raised), the process still cannot be dumped
    std::fs::write(&datei, "core.%p\n").expect("write");
    let (kind, zeilen) = starten(mit_harter_null(&["muster", &pfad]));
    assert_eq!(
        zeilen,
        [
            format!("PROTOKOLL {}", Hinweis::MusterDatei.text()),
            "BEREIT".to_owned()
        ]
    );
    assert_eq!(core_grenze(kind.id()), ("0".to_owned(), "0".to_owned()));
    if eigener_nutzer() != 0 {
        assert_ne!(eigentuemer(kind.id()), eigener_nutzer());
    }
    beenden(kind);
    // a pipe pattern: abort with the fixed reason
    std::fs::write(&datei, "|/usr/share/apport/apport %p\n").expect("write");
    let (kind, zeilen) = starten(mit_harter_null(&["muster", &pfad]));
    assert_eq!(zeilen, [format!("ABBRUCH {}", Abbruch::MusterPipe.grund())]);
    beenden(kind);
    // a pattern that cannot be read: abort with the fixed reason
    std::fs::remove_file(&datei).expect("rm");
    let (kind, zeilen) = starten(mit_harter_null(&["muster", &pfad]));
    assert_eq!(
        zeilen,
        [format!("ABBRUCH {}", Abbruch::MusterNichtLesbar.grund())]
    );
    beenden(kind);
    let _ = std::fs::remove_dir_all(&ordner);
}

#[test]
fn mit_harter_null_entscheidet_das_echte_muster_des_systems() {
    let roh = std::fs::read("/proc/sys/kernel/core_pattern");
    let (kind, zeilen) = starten(mit_harter_null(&[]));
    let erwartet = match roh {
        Ok(inhalt) if inhalt.first() == Some(&b'|') => {
            vec![format!("ABBRUCH {}", Abbruch::MusterPipe.grund())]
        }
        Ok(_) => vec![
            format!("PROTOKOLL {}", Hinweis::MusterDatei.text()),
            "BEREIT".to_owned(),
        ],
        Err(_) => vec![format!("ABBRUCH {}", Abbruch::MusterNichtLesbar.grund())],
    };
    assert_eq!(zeilen, erwartet);
    beenden(kind);
}

#[test]
fn der_waechter_erkennt_den_eigenen_prozess_an_der_startzeit() {
    let eigene = std::process::id();
    let zeit = startzeit(eigene);
    assert!(zeit.is_some(), "the start time of this process is readable");
    assert!(derselbe_prozess(eigene, zeit));
    assert!(
        !derselbe_prozess(eigene, zeit.map(|z| z + 1)),
        "another start time is another process"
    );
    assert!(
        !derselbe_prozess(eigene, None),
        "no start time at the start: no signal"
    );
    // a pid that does not exist has no start time (the highest pid of Linux is below 2^22)
    assert_eq!(startzeit(4_194_305), None);
    assert!(!derselbe_prozess(4_194_305, zeit));
}

/// The probe started with `ulimit` settings: `sh -c '<ulimit commands>; exec "$0" "$@"' <probe> <arguments>`.
fn mit_grenzen(ulimit: &str, argumente: &[&str]) -> Command {
    let mut befehl = Command::new("sh");
    befehl
        .arg("-c")
        .arg(format!("{ulimit}; exec \"$0\" \"$@\""))
        .arg(PROBE)
        .args(argumente);
    befehl
}

/// The values of the line `GRENZE <soft> <hard>` of the probe (`-` for unlimited is not used here).
fn grenze_zeile(zeile: &str) -> (u64, u64) {
    let felder: Vec<&str> = zeile.split_whitespace().collect();
    assert_eq!(felder[0], "GRENZE", "{zeile}");
    (
        felder[1].parse().expect("soft"),
        felder[2].parse().expect("hard"),
    )
}

#[test]
fn lage_gibt_die_entscheidung_ohne_die_grenze_zu_aendern() {
    if uebergehen() {
        return;
    }
    // hard limit not 0: Weiter, and the limit stays as it was: soft 0 and a hard limit above 1 (the unit of `ulimit` depends on the shell, so the
    // hard value is not named); `lage` sets nothing, `abschirmen` would make it 1/1
    let (kind, zeilen) = starten(mit_grenzen("ulimit -S -c 0; ulimit -H -c 2000", &["lage"]));
    assert_eq!(zeilen.len(), 2, "{zeilen:?}");
    assert_eq!(zeilen[0], "LAGE Weiter");
    let (weich, hart) = grenze_zeile(&zeilen[1]);
    assert_eq!(weich, 0, "{zeilen:?}");
    assert!(hart > 1, "{zeilen:?}");
    beenden(kind);
}

#[test]
fn lage_mit_harter_null_entscheidet_nach_dem_muster_und_aendert_nichts() {
    let ordner = std::env::temp_dir().join(format!("kernabbild-lage-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&ordner);
    std::fs::create_dir_all(&ordner).expect("folder");
    let datei = ordner.join("core_pattern");
    let pfad = datei.to_string_lossy().into_owned();
    let null = "ulimit -c 0; ulimit -H -c 0";
    std::fs::write(&datei, "core.%p\n").expect("write");
    let (kind, zeilen) = starten(mit_grenzen(null, &["lage", "muster", &pfad]));
    assert_eq!(zeilen, ["LAGE Protokoll(MusterDatei)", "GRENZE 0 0"]);
    beenden(kind);
    std::fs::write(&datei, "|/usr/share/apport/apport %p\n").expect("write");
    let (kind, zeilen) = starten(mit_grenzen(null, &["lage", "muster", &pfad]));
    assert_eq!(zeilen, ["LAGE Abbruch(MusterPipe)", "GRENZE 0 0"]);
    beenden(kind);
    std::fs::remove_file(&datei).expect("rm");
    let (kind, zeilen) = starten(mit_grenzen(null, &["lage", "muster", &pfad]));
    assert_eq!(zeilen, ["LAGE Abbruch(MusterNichtLesbar)", "GRENZE 0 0"]);
    beenden(kind);
    let _ = std::fs::remove_dir_all(&ordner);
}
