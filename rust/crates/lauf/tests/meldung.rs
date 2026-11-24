//! The report to the core service (contract 1, "Meldung an den Dienst"): with `--meldung <pfad>` the program writes
//! exactly one line `<code> <grund>\n` to the socket of the service when the start fails before the starter, and
//! nothing when the starter runs; then the end of the connection without data comes while the starter runs. The test
//! plays the service: a listening socket in the folder of the world.
//!
//! Which process holds the connection is read from `/proc/<pid>/fd` (the link `socket:[<inode>]`); the inode of the end
//! of the program is the peer of the end the test accepted, which `ss -x` names (`sock_diag`). Every such check begins
//! with a positive control: the same reading finds a connection the test holds itself.

#![cfg(all(feature = "pruefbau", target_os = "linux"))]
#![allow(clippy::print_stderr, clippy::zombie_processes)]

mod gemeinsam;

use std::io::Read;
use std::os::fd::AsRawFd;
use std::os::unix::net::{UnixListener, UnixStream};
use std::os::unix::process::ExitStatusExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use gemeinsam::{Gesperrt, HAENGESPERRE_MS, Welt, datei, endet_binnen, lebt, nachkommen};

/// How long the test waits for a connection or its end.
const FRIST: Duration = Duration::from_secs(20);

/// The service of the test: a socket `m.sock` in the root of the world.
struct Dienst {
    hoerer: UnixListener,
    pfad: PathBuf,
}

impl Dienst {
    fn neu(w: &Welt) -> Dienst {
        let pfad = w.wurzel.join("m.sock");
        let hoerer = UnixListener::bind(&pfad).expect("bind");
        hoerer.set_nonblocking(true).expect("nonblocking");
        Dienst { hoerer, pfad }
    }

    /// The arguments of the program with the report.
    fn args(&self, start: &[String]) -> Vec<String> {
        let mut a = vec![
            "--meldung".to_owned(),
            self.pfad.to_string_lossy().into_owned(),
        ];
        a.extend_from_slice(start);
        a
    }

    /// The one connection of the program, within [`FRIST`].
    fn annehmen(&self) -> UnixStream {
        let ende = Instant::now() + FRIST;
        loop {
            match self.hoerer.accept() {
                Ok((s, _)) => {
                    s.set_nonblocking(false).expect("blocking");
                    return s;
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    assert!(Instant::now() < ende, "the program did not connect");
                    std::thread::sleep(Duration::from_millis(10));
                }
                Err(e) => panic!("accept: {e}"),
            }
        }
    }

    /// No second connection waits.
    fn keine_weitere(&self) {
        assert!(
            matches!(self.hoerer.accept(), Err(e) if e.kind() == std::io::ErrorKind::WouldBlock),
            "one connection only"
        );
    }
}

/// Everything until the end of the connection; `None` when it does not end within `frist` (someone still holds it).
fn bis_zum_ende(s: &mut UnixStream, frist: Duration) -> Option<Vec<u8>> {
    s.set_read_timeout(Some(frist)).expect("timeout");
    let mut alles = Vec::new();
    let mut puffer = [0u8; 256];
    loop {
        match s.read(&mut puffer) {
            Ok(0) => return Some(alles),
            Ok(n) => alles.extend_from_slice(&puffer[..n]),
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
            Err(_) => return None,
        }
    }
}

/// The inode of a socket of this process (`/proc/self/fd/<n>` reads `socket:[<inode>]`).
fn inode(s: &impl AsRawFd) -> u64 {
    let ziel = std::fs::read_link(format!("/proc/self/fd/{}", s.as_raw_fd())).expect("link");
    let t = ziel.to_string_lossy();
    t.strip_prefix("socket:[")
        .and_then(|r| r.strip_suffix(']'))
        .and_then(|n| n.parse().ok())
        .expect("socket")
}

/// The inode of the peer of the socket with `inode` (`ss -x -a`: local address and port, peer address and port; the
/// port is the inode).
fn gegenstelle(inode: u64) -> Option<u64> {
    let aus = Command::new("ss")
        .args(["-x", "-a", "-n", "-H"])
        .output()
        .expect("ss");
    let text = String::from_utf8_lossy(&aus.stdout);
    for zeile in text.lines() {
        let w: Vec<&str> = zeile.split_whitespace().collect();
        // Netid State Recv-Q Send-Q Local Port Peer Port
        if w.len() >= 8 && w[5] == inode.to_string() {
            return w[7].parse().ok();
        }
    }
    None
}

/// Whether the process `pid` holds the socket with `inode`.
fn haelt(pid: i32, inode: u64) -> bool {
    let soll = format!("socket:[{inode}]");
    std::fs::read_dir(format!("/proc/{pid}/fd")).is_ok_and(|l| {
        l.filter_map(Result::ok)
            .filter_map(|e| std::fs::read_link(e.path()).ok())
            .any(|z| z.to_string_lossy() == soll)
    })
}

/// Positive control of the reading: a connection of the test to its own service is found at the test.
fn kontrolle(d: &Dienst) {
    let eigene = UnixStream::connect(&d.pfad).expect("connect");
    let angenommen = d.annehmen();
    let gegen = gegenstelle(inode(&angenommen)).expect("ss names the peer");
    assert_eq!(gegen, inode(&eigene), "ss pairs the two ends");
    let ich = i32::try_from(std::process::id()).expect("pid");
    assert!(
        haelt(ich, gegen),
        "positive control: /proc finds the end the test holds"
    );
    drop(eigene);
    drop(angenommen);
}

/// A `bwrap` whose final call becomes the harness: `sleep`, a child of the program.
fn schlafender_bwrap(w: &Welt) {
    let skript = "#!/bin/sh\nfor a in \"$@\"; do last=\"$a\"; done\nif [ \"$last\" = /usr/bin/true ]; then exit 0; fi\nexec sleep 60\n";
    datei(&w.bin.join("bwrap"), skript.as_bytes(), 0o755);
}

/// A `bwrap` whose final call ends with 42.
fn bwrap_mit_code(w: &Welt) {
    let skript = "#!/bin/sh\nfor a in \"$@\"; do last=\"$a\"; done\nif [ \"$last\" = /usr/bin/true ]; then exit 0; fi\nexit 42\n";
    datei(&w.bin.join("bwrap"), skript.as_bytes(), 0o755);
}

/// A `bwrap` that passes the test run of the profile and then takes away its own execute bit: the `exec` of the
/// trampoline into it fails.
fn verschwindender_bwrap(w: &Welt) {
    let skript = "#!/bin/sh\nfor a in \"$@\"; do last=\"$a\"; done\nif [ \"$last\" = /usr/bin/true ]; then chmod a-x \"$0\"; exit 0; fi\nexit 42\n";
    datei(&w.bin.join("bwrap"), skript.as_bytes(), 0o755);
}

/// The child of the program `lauf` whose program is `sleep`.
fn harness_von(lauf: u32) -> Option<i32> {
    let lauf = i32::try_from(lauf).ok()?;
    nachkommen(lauf)
        .into_iter()
        .find(|p| p.befehl.starts_with("sleep "))
        .map(|p| p.pid)
}

fn welt_leer(w: &Welt) {
    let ende = Instant::now() + Duration::from_millis(HAENGESPERRE_MS);
    while !gemeinsam::prozesse_unter(&w.wurzel).is_empty() && Instant::now() < ende {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(gemeinsam::prozesse_unter(&w.wurzel), [], "nothing is left");
}

/// (1) A fault before the start of the starter: exactly one line `<code> <grund>\n`, then the end of the connection; the
/// exit is the code of the line, the line on standard error stays as before.
#[test]
fn a_fault_before_the_start_writes_exactly_one_line_and_ends_with_its_code() {
    // (setup, expected exit, key)
    let faelle: [(&str, u8, &str); 3] = [
        ("modus", 66, "lauf.datei_modus"),
        ("modellweg", 69, "lauf.modellweg_nicht_bereit"),
        ("harness", 69, "lauf.harness_fehlt"),
    ];
    for (art, code, grund) in faelle {
        let w = Welt::neu();
        let d = Dienst::neu(&w);
        let start = w.lokal();
        match art {
            "modus" => {
                std::fs::set_permissions(
                    w.startdatei(),
                    std::os::unix::fs::PermissionsExt::from_mode(0o644),
                )
                .expect("mode");
            }
            "modellweg" => w.verhalten("modellweg", "ende:3"),
            // the program of the harness is missing: the program itself refuses at step 8, before the trampoline
            _ => {
                let _ = std::fs::remove_file(w.bin.join("pi"));
            }
        }
        let lauf = w.lauf(&d.args(&start), &[]);
        let mut s = d.annehmen();
        let daten = bis_zum_ende(&mut s, FRIST).expect("the connection ends");
        let zeile = String::from_utf8_lossy(&daten).into_owned();
        assert_eq!(zeile, format!("{code} {grund}\n"), "{art}");
        assert!(lauf::meldung::zeile_gut(&daten), "{art}: form");
        assert_eq!(lauf.exit(), Some(i32::from(code)), "{art}: exit = code");
        assert!(
            lauf.stderr().starts_with(&format!("{grund}: ")),
            "{art}: the line on standard error stays: {}",
            lauf.stderr()
        );
        d.keine_weitere();
        welt_leer(&w);
    }
}

/// (2), (4), (5): a start that reaches the starter writes nothing; the connection ends while the harness runs, so no
/// process holds it: not the watcher, the search process, the model path, the program, nor the starter after its `exec`.
/// The program then ends with the code of the starter.
#[test]
fn a_start_ends_the_connection_without_data_and_no_child_holds_it() {
    let w = Welt::neu();
    schlafender_bwrap(&w);
    let d = Dienst::neu(&w);
    kontrolle(&d);
    let mut lauf = gemeinsam::ohne_fremde_deskriptoren(&w.bin.join("werkbank-lauf"))
        .args(d.args(&w.lokal()))
        .env_clear()
        .env("TERM", "xterm")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .gesperrt_spawn()
        .expect("start");
    let mut s = d.annehmen();
    let gegen = gegenstelle(inode(&s));
    let daten = bis_zum_ende(&mut s, FRIST);
    let lauf_pid = i32::try_from(lauf.id()).expect("pid");
    let alle = nachkommen(lauf_pid);
    // the peer is read while the program still holds it; `ss` drops it once every holder is gone
    eprintln!("peer {gegen:?}, descendants {alle:?}");
    assert_eq!(daten.as_deref(), Some(&b""[..]), "end without data");
    // the starter (the `bwrap` stand-in, later the harness `sleep`) runs at the end of the connection
    let starter = alle
        .iter()
        .find(|p| p.befehl.contains("/bin/bwrap ") || p.befehl.starts_with("sleep "))
        .map(|p| p.pid)
        .expect("the starter runs at the end of the connection");
    assert!(lebt(starter), "the starter still runs");
    let ende = Instant::now() + FRIST;
    let harness = loop {
        if let Some(h) = harness_von(lauf.id()) {
            break h;
        }
        assert!(Instant::now() < ende, "the harness started");
        std::thread::sleep(Duration::from_millis(10));
    };
    let pids: Vec<i32> = w.kinder_pids();
    assert_eq!(pids.len(), 2, "search process and model path run");
    for p in &pids {
        assert!(lebt(*p), "child {p} runs at the end of the connection");
    }
    assert!(
        alle.len() >= 4,
        "watcher, search process, model path and harness: {alle:?}"
    );
    let g = gegen.expect("ss names the peer while the program starts");
    for p in alle.iter().map(|p| p.pid).chain([lauf_pid]) {
        assert!(!haelt(p, g), "process {p} holds the connection");
    }
    d.keine_weitere();
    if let Some(pid) = rustix::process::Pid::from_raw(harness) {
        let _ = rustix::process::kill_process(pid, rustix::process::Signal::KILL);
    }
    let status = lauf.wait().expect("wait");
    assert_eq!(status.signal(), Some(9), "the end of the starter");
    welt_leer(&w);
}

/// (2) The exit after the end without data is the code of the starter.
#[test]
fn after_the_end_without_data_the_exit_is_the_code_of_the_starter() {
    let w = Welt::neu();
    bwrap_mit_code(&w);
    let d = Dienst::neu(&w);
    let lauf = w.lauf(&d.args(&w.lokal()), &[]);
    let mut s = d.annehmen();
    assert_eq!(bis_zum_ende(&mut s, FRIST).as_deref(), Some(&b""[..]));
    assert_eq!(lauf.exit(), Some(42));
    assert_eq!(lauf.stderr(), "");
    welt_leer(&w);
}

/// (3) The `exec` of the trampoline into the starter fails: the trampoline writes `69 lauf.harness_fehlt`.
#[test]
fn a_failed_exec_of_the_starter_reports_lauf_harness_fehlt() {
    let w = Welt::neu();
    verschwindender_bwrap(&w);
    let d = Dienst::neu(&w);
    let lauf = w.lauf(&d.args(&w.lokal()), &[]);
    let mut s = d.annehmen();
    let daten = bis_zum_ende(&mut s, FRIST).expect("the connection ends");
    assert_eq!(String::from_utf8_lossy(&daten), "69 lauf.harness_fehlt\n");
    assert_eq!(lauf.exit(), Some(69));
    assert_eq!(
        lauf.stderr(),
        "lauf.harness_fehlt: Harness nicht startbar\n"
    );
    welt_leer(&w);
}

/// The trampoline alone with `--halten-melden` but without descriptor 4: it cannot set close-on-exec, so it does not
/// run the starter and ends with the code of `lauf.intern`.
#[test]
fn the_trampoline_without_descriptor_4_does_not_run_the_starter() {
    let eltern = std::process::id().to_string();
    let aus = gemeinsam::ohne_fremde_deskriptoren(Path::new(gemeinsam::KIND))
        .args([eltern.as_str(), "--halten-melden", "/bin/true"])
        .env_clear()
        .stdin(Stdio::null())
        .gesperrt_output()
        .expect("trampoline");
    assert_eq!(aus.status.code(), Some(70));
}

/// (6) Without `--meldung` the program behaves as before: no connection, and the start ends as it did.
#[test]
fn without_the_option_nothing_connects() {
    let w = Welt::neu();
    bwrap_mit_code(&w);
    let d = Dienst::neu(&w);
    let lauf = w.lauf(&w.lokal(), &[]);
    assert_eq!(lauf.exit(), Some(42));
    assert_eq!(lauf.stderr(), "");
    d.keine_weitere();
    welt_leer(&w);
}

/// (7) The connection fails (no socket, a file, a path too long for `sun_path`): 69 `lauf.meldung_fehlt`, only the line
/// on standard error, no child.
#[test]
fn a_failed_connection_ends_with_lauf_meldung_fehlt_and_starts_no_child() {
    let w = Welt::neu();
    let datei_pfad = w.wurzel.join("datei");
    datei(&datei_pfad, b"x", 0o600);
    let lang = format!("{}/{}", w.wurzel.display(), "m".repeat(120));
    for pfad in [
        w.wurzel.join("fehlt.sock").to_string_lossy().into_owned(),
        datei_pfad.to_string_lossy().into_owned(),
        lang,
    ] {
        let mut args = vec!["--meldung".to_owned(), pfad.clone()];
        args.extend(w.lokal());
        let lauf = w.lauf(&args, &[]);
        assert_eq!(lauf.exit(), Some(69), "{pfad}");
        assert_eq!(
            lauf.stderr(),
            "lauf.meldung_fehlt: Meldung an den Dienst nicht moeglich\n",
            "{pfad}"
        );
        assert_eq!(w.pid_von("rangkern"), None, "no child");
    }
}

/// Descriptor 4 is not free at the start: the program cannot hand the report over and says so on the connection.
#[test]
fn an_inherited_descriptor_4_with_the_report_ends_with_lauf_meldung_fehlt() {
    let w = Welt::neu();
    let d = Dienst::neu(&w);
    let aus = Command::new(gemeinsam::BASH)
        .args([
            "-c",
            "for f in /proc/$$/fd/*; do n=${f##*/}; if [ \"$n\" -gt 2 ] 2>/dev/null; then eval \"exec $n>&-\" 2>/dev/null; fi; done; exec 4</dev/null; exec \"$0\" \"$@\"",
        ])
        .arg(w.bin.join("werkbank-lauf"))
        .args(d.args(&w.lokal()))
        .env_clear()
        .env("TERM", "xterm")
        .stdin(Stdio::null())
        .gesperrt_output()
        .expect("run");
    let mut s = d.annehmen();
    let daten = bis_zum_ende(&mut s, FRIST).expect("end");
    assert_eq!(String::from_utf8_lossy(&daten), "69 lauf.meldung_fehlt\n");
    assert_eq!(aus.status.code(), Some(69));
    assert_eq!(w.pid_von("rangkern"), None, "no child");
}

/// (4) The search process and the model path never inherit the connection, also while the program still holds it:
/// with the search process slow to report ready, the program holds it (positive control) and the search process not.
#[test]
fn the_search_process_does_not_hold_the_connection_while_the_program_does() {
    let w = Welt::neu();
    schlafender_bwrap(&w);
    w.verhalten("rangkern", "stumm");
    let d = Dienst::neu(&w);
    kontrolle(&d);
    let lauf = gemeinsam::ohne_fremde_deskriptoren(&w.bin.join("werkbank-lauf"))
        .args(d.args(&w.lokal()))
        .env_clear()
        .env("TERM", "xterm")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .gesperrt_spawn()
        .expect("start");
    let s = d.annehmen();
    let gegen = gegenstelle(inode(&s)).expect("ss names the peer of the program");
    let lauf_pid = i32::try_from(lauf.id()).expect("pid");
    let ende = Instant::now() + FRIST;
    let suche = loop {
        if let Some(p) = w.pid_von("rangkern") {
            break p;
        }
        assert!(Instant::now() < ende, "the search process started");
        std::thread::sleep(Duration::from_millis(10));
    };
    assert!(
        haelt(lauf_pid, gegen),
        "positive control: the program holds it"
    );
    assert!(!haelt(suche, gegen), "the search process holds it");
    drop(s);
    let mut lauf = lauf;
    let _ = lauf.wait();
    assert!(
        endet_binnen(suche, HAENGESPERRE_MS),
        "the search process ends"
    );
    welt_leer(&w);
}

/// The form of the call is closed (contract 1, "Meldung an den Dienst"): the option after the start file, a relative path
/// and the option without a path are each 64 `lauf.aufruf`; the program connects to nothing and starts no child.
#[test]
fn every_other_form_of_the_call_is_a_usage_error() {
    let w = Welt::neu();
    let d = Dienst::neu(&w);
    let start = w.lokal();
    let sock = d.pfad.to_string_lossy().into_owned();
    let name = d
        .pfad
        .file_name()
        .expect("name")
        .to_string_lossy()
        .into_owned();
    let faelle: [(&str, Vec<String>); 3] = [
        (
            "option after the start file",
            [start.clone(), vec!["--meldung".to_owned(), sock.clone()]].concat(),
        ),
        (
            "relative path",
            [vec!["--meldung".to_owned(), name], start.clone()].concat(),
        ),
        (
            "option without a path",
            [vec!["--meldung".to_owned()], start.clone()].concat(),
        ),
    ];
    for (art, args) in faelle {
        // the relative name would reach the socket from the root of the world: run the program there
        let aus = gemeinsam::ohne_fremde_deskriptoren(&w.bin.join("werkbank-lauf"))
            .args(&args)
            .current_dir(&w.wurzel)
            .env_clear()
            .env("TERM", "xterm")
            .stdin(Stdio::null())
            .gesperrt_output()
            .expect("run");
        assert_eq!(aus.status.code(), Some(64), "{art}");
        assert_eq!(
            String::from_utf8_lossy(&aus.stderr),
            "lauf.aufruf: genau ein Pfad erwartet\n",
            "{art}"
        );
        d.keine_weitere();
        assert_eq!(w.pid_von("rangkern"), None, "{art}: no child");
    }
}
