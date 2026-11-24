//! The first step of `rangkern serve` and `rangkern sitzung` as a program: the limit for core images 1/1 and `PR_SET_DUMPABLE 0`
//! (crate `kernabbild`, decision of 21.11.2026), the order before the other arguments, the inherited hard limit 0 and the commands that take
//! no step. Every program here runs with a cleared environment in a folder of its own; the vault is invented.

#![cfg(unix)]

use std::fs;
#[cfg(target_os = "linux")]
use std::io::Write;
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use kernabbild::{Abbruch, Hinweis};

const BIN: &str = env!("CARGO_BIN_EXE_rangkern");
const MUSTER: &str = "/proc/sys/kernel/core_pattern";

/// A folder of the test with a vault, settings, a log folder and a socket path.
struct Welt {
    root: PathBuf,
}

impl Welt {
    fn new(name: &str) -> Self {
        // under /tmp: the path of a Unix socket must be short
        let root = PathBuf::from(format!("/tmp/rkabs-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("vault")).expect("vault");
        fs::create_dir_all(root.join("protokoll")).expect("log folder");
        // The folder of the socket is 0700: `rangkern sitzung` checks it under Linux too (review B7, 23.11.2026).
        fs::set_permissions(&root, std::os::unix::fs::PermissionsExt::from_mode(0o700))
            .expect("mode of the folder");
        fs::write(
            root.join("vault/a.md"),
            "---\ntitle: Eine Notiz\n---\nText der Notiz.\n",
        )
        .expect("note");
        fs::write(
            root.join("einst.json"),
            r#"{"fassung":1,"klassen":["oeffentlich","intern"],"vorgabestufe":"intern","routen":[{"schluessel":"cloud","art":"cloud","klasse":"intern"}],"kunden":[],"kundendaten_an_cloud":{"vorgabe":"nie","freigaben":[]}}"#,
        )
        .expect("settings");
        Self { root }
    }

    fn pfad(&self, name: &str) -> String {
        self.root.join(name).to_string_lossy().into_owned()
    }

    fn serve_args(&self) -> Vec<String> {
        vec!["serve".to_string(), self.pfad("vault")]
    }

    fn sitzung_args(&self) -> Vec<String> {
        [
            "sitzung",
            &self.pfad("vault"),
            "--socket",
            &self.pfad("s.sock"),
            "--route",
            "cloud",
            "--sitzung",
            "s-abs-1",
            "--einstellungen",
            &self.pfad("einst.json"),
            "--protokoll",
            &self.pfad("protokoll"),
        ]
        .iter()
        .map(|s| (*s).to_string())
        .collect()
    }

    /// The program with a cleared environment; `harte_null` starts it through a shell that sets the hard limit for core to 0
    /// (`ulimit -c 0`: soft and hard; `ulimit -Hc 0` alone fails with a soft limit above it), so that the program inherits it. No `unsafe` and no `pre_exec`.
    fn befehl(&self, harte_null: bool, args: &[String]) -> Command {
        let mut befehl = if harte_null {
            let mut sh = Command::new("sh");
            sh.args(["-c", "ulimit -c 0 && exec \"$0\" \"$@\"", BIN]);
            sh
        } else {
            Command::new(BIN)
        };
        befehl
            .args(args)
            .current_dir(&self.root)
            .env_clear()
            .env("PATH", std::env::var_os("PATH").unwrap_or_default())
            .env("LANG", "C.UTF-8")
            .env("TZ", "UTC")
            .env("HOME", self.pfad("home"));
        befehl
    }
}

impl Drop for Welt {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

/// What a program that ends on its own did.
struct Ende {
    code: Option<i32>,
    stdout: String,
    stderr: String,
}

/// Runs a program that ends on its own (a usage error, an abort) and waits at most ten seconds; a program that does not end is
/// killed and gives no code.
fn laufen_lassen(mut befehl: Command) -> Ende {
    let mut kind = befehl
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("program starts");
    let ende = Instant::now() + Duration::from_secs(10);
    let status = loop {
        if let Some(status) = kind.try_wait().expect("wait") {
            break status.code();
        }
        if Instant::now() > ende {
            kind.kill().expect("kill");
            let _ = kind.wait();
            break None;
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    let lesen = |quelle: Option<&mut dyn Read>| {
        let mut text = String::new();
        if let Some(quelle) = quelle {
            let _ = quelle.read_to_string(&mut text);
        }
        text
    };
    Ende {
        code: status,
        stdout: lesen(kind.stdout.as_mut().map(|s| s as &mut dyn Read)),
        stderr: lesen(kind.stderr.as_mut().map(|s| s as &mut dyn Read)),
    }
}

/// The core pattern of the host, `None` when it cannot be read (then the host is no Linux).
fn muster_des_wirts() -> Option<String> {
    fs::read_to_string(MUSTER).ok()
}

fn ist_pipe(muster: &str) -> bool {
    muster.starts_with('|')
}

/// The pair soft and hard of the line `Max core file size` of `/proc/<pid>/limits`. A file that cannot be read or has no such line is
/// a failure of the test, never a pass (positive control).
#[cfg(target_os = "linux")]
fn kernabbild_grenze(pid: u32) -> (String, String) {
    let text = fs::read_to_string(format!("/proc/{pid}/limits"))
        .unwrap_or_else(|e| panic!("limits of {pid} cannot be read: {e}"));
    let zeile = text
        .lines()
        .find(|z| z.starts_with("Max core file size"))
        .unwrap_or_else(|| panic!("no line `Max core file size` in:\n{text}"));
    let felder: Vec<&str> = zeile.split_whitespace().collect();
    // "Max core file size <soft> <hard> bytes"
    assert!(felder.len() >= 6, "{zeile}");
    (felder[4].to_string(), felder[5].to_string())
}

/// The owner of the files under `/proc/<pid>`: root for a process that cannot be dumped, the user for one that can. A failure to
/// read is a failure of the test. Inside a user namespace (the enclosure of the hand mutants) root is not mapped and shows as 65534.
#[cfg(target_os = "linux")]
fn eigentuemer(pid: u32) -> u32 {
    use std::os::unix::fs::MetadataExt;
    fs::metadata(format!("/proc/{pid}/stat"))
        .unwrap_or_else(|e| panic!("stat of {pid} cannot be read: {e}"))
        .uid()
}

/// The check "not dumpable": the files belong to root (0, or 65534 where root is unmapped) and not to the user of the test.
#[cfg(target_os = "linux")]
fn gehoert_root(eigentuemer: u32) {
    use std::os::unix::fs::MetadataExt;
    let ich = fs::metadata("/proc/self").expect("own entry").uid();
    assert_ne!(
        eigentuemer, ich,
        "the process is dumpable: its files belong to the user"
    );
    assert!(
        matches!(eigentuemer, 0 | 65534),
        "the files of /proc/<pid> belong neither to root (0) nor to the unmapped root (65534)"
    );
}

/// Sends a signal through the `kill` program (no `unsafe`, no libc).
fn signal(pid: u32, name: &str) {
    let status = Command::new("kill")
        .args([format!("-{name}"), pid.to_string()])
        .status()
        .expect("kill runs");
    assert!(status.success(), "kill -{name} {pid}");
}

fn warten(kind: &mut Child) -> Option<i32> {
    let ende = Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(status) = kind.try_wait().expect("wait") {
            return status.code();
        }
        if Instant::now() > ende {
            let _ = kind.kill();
            let _ = kind.wait();
            return None;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// A started `rangkern sitzung` after its readiness line.
#[cfg(target_os = "linux")]
fn sitzung_starten(welt: &Welt, harte_null: bool) -> Child {
    let mut kind = welt
        .befehl(harte_null, &welt.sitzung_args())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("program starts");
    let mut zeile = String::new();
    BufReader::new(kind.stdout.as_mut().expect("stdout"))
        .read_line(&mut zeile)
        .expect("readiness line");
    assert_eq!(zeile, "{\"bereit\":true}\n");
    kind
}

#[cfg(target_os = "linux")]
#[test]
fn the_process_of_sitzung_is_not_dumpable_and_has_the_limit_1_1() {
    let welt = Welt::new("sitzung-grenze");
    let mut kind = sitzung_starten(&welt, false);
    let pid = kind.id();
    let (eigentuemer, grenze) = (eigentuemer(pid), kernabbild_grenze(pid));
    signal(pid, "TERM");
    assert_eq!(warten(&mut kind), Some(0), "SIGTERM ends the program");
    gehoert_root(eigentuemer);
    assert_eq!(grenze, ("1".to_string(), "1".to_string()));
    assert!(!Path::new(&welt.pfad("s.sock")).exists(), "socket is gone");
    assert!(
        kind.try_wait().expect("wait").is_some(),
        "the process ended"
    );
}

#[cfg(target_os = "linux")]
#[test]
fn the_process_of_serve_is_not_dumpable_and_has_the_limit_1_1() {
    let welt = Welt::new("serve-grenze");
    let mut kind = welt
        .befehl(false, &welt.serve_args())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("program starts");
    let mut stdin = kind.stdin.take().expect("stdin");
    let mut stdout = BufReader::new(kind.stdout.take().expect("stdout"));
    // the answer to `ping` shows that the loop runs
    stdin
        .write_all(b"{\"op\":\"ping\",\"id\":1}\n")
        .expect("ping");
    let mut zeile = String::new();
    stdout.read_line(&mut zeile).expect("answer");
    assert!(zeile.contains("\"id\":1"), "{zeile}");
    let pid = kind.id();
    let (eigentuemer, grenze) = (eigentuemer(pid), kernabbild_grenze(pid));
    stdin
        .write_all(b"{\"op\":\"quit\",\"id\":2}\n")
        .expect("quit");
    assert_eq!(warten(&mut kind), Some(0), "quit ends the program");
    gehoert_root(eigentuemer);
    assert_eq!(grenze, ("1".to_string(), "1".to_string()));
}

/// The inherited hard limit 0 for `serve` and `sitzung`. The result depends on the core pattern of the host: a pipe ends the start with
/// exit 5 and the line of `muster_pipe`; any other pattern gives the note and a normal start. The test reads the pattern and expects
/// the one branch that belongs to it; the branch that ran is printed (`--nocapture`).
#[test]
// The one line of the branch that ran goes to the error output of the test (`--nocapture`), for the report of the build.
#[allow(clippy::print_stderr)]
fn an_inherited_hard_limit_0_gives_exit_5_with_a_pipe_pattern_and_a_note_with_a_file_pattern() {
    let welt = Welt::new("harte-null");
    let Some(muster) = muster_des_wirts() else {
        // macOS has no core pattern: the note `MusterFehlt` and a normal start
        let ende = laufen_lassen(welt.befehl(true, &["serve".to_string()]));
        assert_eq!(
            ende.code,
            Some(64),
            "usage error after the note: {}",
            ende.stderr
        );
        assert!(
            ende.stderr.starts_with(&format!(
                "rangkern serve: {}\n",
                Hinweis::MusterFehlt.text()
            )),
            "{}",
            ende.stderr
        );
        eprintln!("branch: no core pattern (macOS), note MusterFehlt");
        return;
    };
    if ist_pipe(&muster) {
        eprintln!("branch: pipe pattern {muster:?}, exit 5 with muster_pipe");
        for (befehl, args) in [
            ("serve", welt.serve_args()),
            ("sitzung", welt.sitzung_args()),
        ] {
            let ende = laufen_lassen(welt.befehl(true, &args));
            assert_eq!(ende.code, Some(5), "{befehl}: {}", ende.stderr);
            assert_eq!(ende.stdout, "", "{befehl}: nothing on standard output");
            assert_eq!(
                ende.stderr,
                format!(
                    "rangkern {befehl}: Kernabbild nicht abschirmbar: {} ({})\n",
                    Abbruch::MusterPipe.grund(),
                    Abbruch::MusterPipe.abhilfe()
                ),
                "{befehl}: exactly one line"
            );
        }
        assert!(
            !Path::new(&welt.pfad("s.sock")).exists(),
            "no socket exists after the abort"
        );
    } else {
        eprintln!("branch: file pattern {muster:?}, note MusterDatei and a normal start");
        let mut kind = welt
            .befehl(true, &welt.sitzung_args())
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("program starts");
        let mut zeile = String::new();
        BufReader::new(kind.stdout.as_mut().expect("stdout"))
            .read_line(&mut zeile)
            .expect("readiness line");
        assert_eq!(zeile, "{\"bereit\":true}\n");
        signal(kind.id(), "TERM");
        assert_eq!(warten(&mut kind), Some(0));
        let mut fehler = String::new();
        kind.stderr
            .take()
            .expect("stderr")
            .read_to_string(&mut fehler)
            .expect("stderr is read");
        assert_eq!(
            fehler,
            format!("rangkern sitzung: {}\n", Hinweis::MusterDatei.text())
        );
    }
}

/// The step comes before the other arguments: `sitzung` without arguments ends with 64 only when the step passed, and `serve` without a
/// vault ends with 64 only then. With a pipe pattern and the hard limit 0 both end with 5 first.
#[test]
fn the_step_comes_before_the_arguments_are_read() {
    let welt = Welt::new("reihenfolge");
    let ohne = laufen_lassen(welt.befehl(false, &["sitzung".to_string()]));
    assert_eq!(ohne.code, Some(64), "{}", ohne.stderr);
    assert!(
        ohne.stderr.starts_with("usage: rangkern "),
        "{}",
        ohne.stderr
    );
    let ohne = laufen_lassen(welt.befehl(false, &["serve".to_string()]));
    assert_eq!(ohne.code, Some(64), "{}", ohne.stderr);
    if muster_des_wirts().is_some_and(|m| ist_pipe(&m)) {
        for befehl in ["sitzung", "serve"] {
            let ende = laufen_lassen(welt.befehl(true, &[befehl.to_string()]));
            assert_eq!(ende.code, Some(5), "{befehl}: {}", ende.stderr);
            assert_eq!(ende.stderr.lines().count(), 1, "{befehl}: one line");
            assert!(
                ende.stderr.contains(Abbruch::MusterPipe.grund()),
                "{}",
                ende.stderr
            );
        }
    }
}

/// `--licenses` and `search` take no step: they run under the hard limit 0 with a pipe pattern as they do without it.
#[test]
fn the_other_commands_take_no_step_and_run_under_the_hard_limit_0() {
    let welt = Welt::new("andere");
    let lizenz = laufen_lassen(welt.befehl(true, &["--licenses".to_string()]));
    assert_eq!(lizenz.code, Some(0), "{}", lizenz.stderr);
    assert_ne!(lizenz.stdout, "", "the notices are printed");
    assert_eq!(lizenz.stderr, "");
    let suche = laufen_lassen(welt.befehl(
        true,
        &[
            "search".to_string(),
            welt.pfad("vault"),
            "Notiz".to_string(),
        ],
    ));
    assert_eq!(suche.code, Some(0), "{}", suche.stderr);
    assert!(suche.stdout.contains("a.md"), "{}", suche.stdout);
    assert_eq!(suche.stderr, "", "no note, no step");
}
