//! The row "hard limit 0, file pattern" of the case table of contract 4 as a program test, and the row "pattern cannot be read".
//! `rangkern serve` and `rangkern sitzung` start in a user and mount namespace of their own (`unshare -Urm`), in which a file of the
//! test lies over `/proc/sys/kernel/core_pattern` (`mount --bind`), under the inherited hard limit 0 (`ulimit -c 0`). The bind stays
//! inside the namespace: the pattern of the host is the same before and after every test.
//!
//! - a file pattern (`core`): the note `MusterDatei` and a normal start, for `serve` and `sitzung`;
//! - a pattern that cannot be read (the file has mode 000 and the program runs without capabilities, `setpriv`): exit 5 with
//!   `kernabbild.muster_nicht_lesbar` and nothing else.
//!
//! A folder cannot lie over the file (`mount --bind` of a folder over a file fails with "Not a directory"); the case "cannot be read" is
//! therefore made with the rights of a file. If the kernel refuses the bind in a user namespace, or `unshare`/`setpriv` is missing, the
//! tests are red with the reason, never skipped. Every program runs with a cleared environment in a folder of its own.

#![cfg(target_os = "linux")]

use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::{Child, Command, Output, Stdio};
use std::time::{Duration, Instant};

use kernabbild::{Abbruch, Hinweis};

const BIN: &str = env!("CARGO_BIN_EXE_rangkern");
const MUSTER: &str = "/proc/sys/kernel/core_pattern";

/// Binds `$1` over the core pattern, sets the hard limit 0 and runs the rest. `$0` is the program, `$1` the pattern file.
const BINDEN: &str = "mount --bind \"$1\" /proc/sys/kernel/core_pattern || exit 97; \
                      ulimit -c 0 || exit 98; p=\"$0\"; shift; exec \"$p\" \"$@\"";
/// The same, but the program runs without any capability: root of the namespace cannot read a file of mode 000 then.
const BINDEN_OHNE_RECHTE: &str = "mount --bind \"$1\" /proc/sys/kernel/core_pattern || exit 97; \
                                  ulimit -c 0 || exit 98; p=\"$0\"; shift; \
                                  exec setpriv --inh-caps=-all --bounding-set=-all -- \"$p\" \"$@\"";

/// A folder of the test with a vault, settings, a log folder, a socket path and the pattern file `muster`.
struct Welt {
    root: PathBuf,
}

impl Welt {
    fn new(name: &str, muster: &[u8], modus: u32) -> Self {
        // under /tmp: the path of a Unix socket must be short
        let root = PathBuf::from(format!("/tmp/rkmu-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("vault")).expect("vault");
        fs::create_dir_all(root.join("protokoll")).expect("log folder");
        // The folder of the socket is 0700: `rangkern sitzung` checks it under Linux too (review B7, 23.11.2026).
        fs::set_permissions(&root, PermissionsExt::from_mode(0o700)).expect("mode of the folder");
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
        fs::write(root.join("muster"), muster).expect("pattern file");
        fs::set_permissions(root.join("muster"), fs::Permissions::from_mode(modus))
            .expect("mode of the pattern file");
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
            "s-muster-1",
            "--einstellungen",
            &self.pfad("einst.json"),
            "--protokoll",
            &self.pfad("protokoll"),
        ]
        .iter()
        .map(|s| (*s).to_string())
        .collect()
    }

    /// `programm` with `args` in its own user and mount namespace, the pattern file over the core pattern and the hard limit 0.
    /// `unshare` without `--fork` and `exec` in the shell: the pid of the child is the pid of the program.
    fn im_namensraum(&self, skript: &str, programm: &str, args: &[String]) -> Command {
        let mut befehl = Command::new("unshare");
        befehl
            .args(["-Urm", "sh", "-c", skript, programm])
            .arg(self.pfad("muster"))
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

fn muster_des_wirts() -> Vec<u8> {
    fs::read(MUSTER).expect("the core pattern of the host is read")
}

/// Runs a command that ends on its own and waits at most ten seconds; one that does not end is killed and fails the test.
fn ausfuehren(mut befehl: Command) -> Output {
    let mut kind = befehl
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("unshare starts (util-linux installed?)");
    let ende = Instant::now() + Duration::from_secs(10);
    while kind.try_wait().expect("wait").is_none() {
        if Instant::now() > ende {
            let _ = kind.kill();
            let _ = kind.wait();
            panic!("the command did not end within 10 s");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    kind.wait_with_output().expect("output")
}

/// Positive control: inside the namespace the core pattern shows the content of the pattern file. A refused bind is a failure with
/// the reason (exit 97: `mount` failed), never a skip.
fn bindung_wirkt(welt: &Welt, inhalt: &str) {
    let aus = ausfuehren(welt.im_namensraum(BINDEN, "cat", &[MUSTER.to_string()]));
    assert_eq!(
        aus.status.code(),
        Some(0),
        "the bind over the core pattern in a user namespace failed: {}",
        String::from_utf8_lossy(&aus.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&aus.stdout), inhalt);
}

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

fn fehlerausgabe(kind: &mut Child) -> String {
    let mut text = String::new();
    kind.stderr
        .take()
        .expect("stderr")
        .read_to_string(&mut text)
        .expect("stderr is read");
    text
}

#[test]
fn a_file_pattern_under_the_hard_limit_0_gives_the_note_and_a_normal_start_of_sitzung() {
    let vorher = muster_des_wirts();
    let welt = Welt::new("sitzung", b"core\n", 0o600);
    bindung_wirkt(&welt, "core\n");
    let mut kind = welt
        .im_namensraum(BINDEN, BIN, &welt.sitzung_args())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("program starts");
    let mut zeile = String::new();
    BufReader::new(kind.stdout.as_mut().expect("stdout"))
        .read_line(&mut zeile)
        .expect("readiness line");
    assert_eq!(zeile, "{\"bereit\":true}\n", "{}", fehlerausgabe(&mut kind));
    signal(kind.id(), "TERM");
    assert_eq!(warten(&mut kind), Some(0), "SIGTERM ends the program");
    assert_eq!(
        fehlerausgabe(&mut kind),
        format!("rangkern sitzung: {}\n", Hinweis::MusterDatei.text())
    );
    assert!(
        !std::path::Path::new(&welt.pfad("s.sock")).exists(),
        "socket is gone"
    );
    assert_eq!(muster_des_wirts(), vorher, "the host pattern is unchanged");
}

#[test]
fn a_file_pattern_under_the_hard_limit_0_gives_the_note_and_a_normal_start_of_serve() {
    let vorher = muster_des_wirts();
    let welt = Welt::new("serve", b"core\n", 0o600);
    bindung_wirkt(&welt, "core\n");
    let mut kind = welt
        .im_namensraum(BINDEN, BIN, &welt.serve_args())
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
    stdin
        .write_all(b"{\"op\":\"quit\",\"id\":2}\n")
        .expect("quit");
    assert_eq!(warten(&mut kind), Some(0), "quit ends the program");
    assert_eq!(
        fehlerausgabe(&mut kind),
        format!("rangkern serve: {}\n", Hinweis::MusterDatei.text())
    );
    assert_eq!(muster_des_wirts(), vorher, "the host pattern is unchanged");
}

#[test]
fn a_pattern_that_cannot_be_read_under_the_hard_limit_0_ends_serve_and_sitzung_with_5() {
    let vorher = muster_des_wirts();
    let welt = Welt::new("nicht-lesbar", b"core\n", 0o000);
    // Positive control: without capabilities `cat` cannot read the bound pattern (mode 000), so the program meets the same refusal.
    let kontrolle =
        ausfuehren(welt.im_namensraum(BINDEN_OHNE_RECHTE, "cat", &[MUSTER.to_string()]));
    assert_eq!(
        kontrolle.status.code(),
        Some(1),
        "cat must fail on the pattern of mode 000 (97: mount refused, 127: setpriv missing): {}",
        String::from_utf8_lossy(&kontrolle.stderr)
    );
    assert!(
        String::from_utf8_lossy(&kontrolle.stderr).contains("Permission denied"),
        "{}",
        String::from_utf8_lossy(&kontrolle.stderr)
    );
    for (befehl, args) in [
        ("serve", welt.serve_args()),
        ("sitzung", welt.sitzung_args()),
    ] {
        let aus = ausfuehren(welt.im_namensraum(BINDEN_OHNE_RECHTE, BIN, &args));
        assert_eq!(
            aus.status.code(),
            Some(5),
            "{befehl}: {}",
            String::from_utf8_lossy(&aus.stderr)
        );
        assert_eq!(aus.stdout, b"", "{befehl}: nothing on standard output");
        assert_eq!(
            String::from_utf8_lossy(&aus.stderr),
            format!(
                "rangkern {befehl}: Kernabbild nicht abschirmbar: {} ({})\n",
                Abbruch::MusterNichtLesbar.grund(),
                Abbruch::MusterNichtLesbar.abhilfe()
            ),
            "{befehl}: exactly one line"
        );
    }
    assert!(
        !std::path::Path::new(&welt.pfad("s.sock")).exists(),
        "no socket exists after the abort"
    );
    assert_eq!(muster_des_wirts(), vorher, "the host pattern is unchanged");
}
