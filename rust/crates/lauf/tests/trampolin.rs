//! The programs of the delivered build without the test build (no feature): the trampoline `werkbank-lauf-kind` with
//! its three forms and the watcher, the children of [`lauf::kind::starten`], and the steps of `werkbank-lauf` and
//! `werkbank-ordnermarke` before the configuration. These tests run in the build the package delivers, so a change of
//! these parts fails there too, not only in the test build.

#![cfg(target_os = "linux")]
#![allow(clippy::zombie_processes)]

use std::io::Read;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::time::{Duration, Instant};

use lauf::fehler::Fehler;
use lauf::kind::{
    self, Ausfall, BEREIT_FRIST, HALTEN, HALTEN_MELDEN, KIND_AUFRUF_FEHLER, KIND_START_FEHLER,
    WACHE,
};

const KIND: &str = env!("CARGO_BIN_EXE_werkbank-lauf-kind");
const LAUF: &str = env!("CARGO_BIN_EXE_werkbank-lauf");
const ORDNERMARKE: &str = env!("CARGO_BIN_EXE_werkbank-ordnermarke");

/// The ready line as a shell command.
const BEREIT: &str = r#"printf '{"bereit":true}\n'"#;

/// A counter for the names of the folders of [`laufen`].
static ZAEHLER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// Accepts the first connection within 10 s (a program that never connects fails the test instead of hanging it).
fn annehmen(hoerer: &std::os::unix::net::UnixListener) -> UnixStream {
    hoerer.set_nonblocking(true).expect("non-blocking");
    let ende = Instant::now() + Duration::from_secs(10);
    loop {
        match hoerer.accept() {
            Ok((v, _)) => {
                v.set_nonblocking(false).expect("blocking");
                return v;
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                assert!(
                    !ende.saturating_duration_since(Instant::now()).is_zero(),
                    "no connection within 10 s"
                );
                std::thread::sleep(Duration::from_millis(10));
            }
            Err(e) => panic!("accept: {e}"),
        }
    }
}

/// A folder of its own, removed again at the end.
struct Ordner(PathBuf);

impl Ordner {
    fn neu(name: &str) -> Ordner {
        let o = std::env::temp_dir().join(format!("lauf-tr-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&o);
        std::fs::create_dir_all(&o).expect("folder");
        Ordner(o)
    }
}

impl Drop for Ordner {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Whether the process `pid` runs (exists and is no zombie).
fn lebt(pid: u32) -> bool {
    std::fs::read_to_string(format!("/proc/{pid}/stat")).is_ok_and(|s| {
        s.rsplit_once(") ")
            .is_some_and(|(_, rest)| !rest.starts_with('Z'))
    })
}

/// Waits at most `frist` until `bedingung` holds.
fn bis(frist: Duration, mut bedingung: impl FnMut() -> bool) -> bool {
    let ende = Instant::now() + frist;
    loop {
        if bedingung() {
            return true;
        }
        if ende.saturating_duration_since(Instant::now()).is_zero() {
            return false;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// Runs `programm` with `args` from bash, which closes every descriptor above 2 and then, with `vier`, lays that socket on
/// descriptor 4. The program is a direct child of this process (bash replaces itself by it). Waits at most 15 s; standard
/// error goes into a file, so a grandchild that keeps a copy never holds the test.
fn laufen(
    programm: &str,
    args: &[String],
    vier: Option<UnixStream>,
    umgebung: &[(&str, &str)],
) -> (ExitStatus, String) {
    let vier_da = vier.is_some();
    let mut c = Command::new("bash");
    c.args([
        "-c",
        &format!(
            "for f in /proc/$$/fd/*; do n=${{f##*/}}; if [ \"$n\" -gt 2 ]; then eval \"exec $n>&-\"; fi; done; {} exec \"$0\" \"$@\"",
            if vier_da { "exec 4<&0 0</dev/null;" } else { "" }
        ),
    ])
    .arg(programm)
    .args(args)
    .env_clear()
    .envs(umgebung.iter().copied())
    .stdout(Stdio::null());
    let o = Ordner::neu(&format!(
        "laufen-{}",
        ZAEHLER.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
    ));
    let pfad = o.0.join("fehler");
    c.stderr(std::fs::File::create(&pfad).expect("error file"));
    match vier {
        Some(v) => c.stdin(Stdio::from(std::os::fd::OwnedFd::from(v))),
        None => c.stdin(Stdio::null()),
    };
    let mut k = c.spawn().expect("bash");
    drop(c);
    let ende = Instant::now() + Duration::from_secs(15);
    let status = loop {
        if let Some(s) = k.try_wait().expect("wait") {
            break s;
        }
        if ende.saturating_duration_since(Instant::now()).is_zero() {
            let _ = k.kill();
            let _ = k.wait();
            panic!("{programm} {args:?} did not end");
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    let fehler = std::fs::read_to_string(&pfad).unwrap_or_default();
    (status, fehler)
}

/// What the service reads on its end of the report until the connection ends (at most 10 s).
fn gelesen(dienst: &UnixStream) -> Vec<u8> {
    dienst
        .set_read_timeout(Some(Duration::from_secs(10)))
        .expect("limit");
    let mut alles = Vec::new();
    let mut leser = dienst;
    leser.read_to_end(&mut alles).expect("read to the end");
    alles
}

fn s(w: &str) -> String {
    w.to_owned()
}

fn eltern() -> String {
    std::process::id().to_string()
}

/// A shell script that ends with 7 when it is the leader of its session and with 8 otherwise (`leiter`), or the other
/// way round, and with 9 when it holds descriptor 4.
fn sitzung_skript(leiter: bool) -> String {
    let (ja, nein) = if leiter { (7, 8) } else { (8, 7) };
    format!(
        "[ -e /proc/$$/fd/4 ] && exit 9; read -r z < /proc/$$/stat; set -- $z; if [ \"$6\" = \"$$\" ]; then exit {ja}; fi; exit {nein}"
    )
}

#[test]
fn a_child_becomes_the_program_in_a_session_of_its_own() {
    let (st, _) = laufen(
        KIND,
        &[
            eltern(),
            s("--"),
            s("/bin/sh"),
            s("-c"),
            sitzung_skript(true),
        ],
        None,
        &[],
    );
    assert_eq!(st.code(), Some(7));
}

#[test]
fn the_starter_becomes_the_program_and_keeps_the_session() {
    let (st, _) = laufen(
        KIND,
        &[
            eltern(),
            s(HALTEN),
            s("/bin/sh"),
            s("-c"),
            sitzung_skript(false),
        ],
        None,
        &[],
    );
    assert_eq!(st.code(), Some(7));
}

#[test]
fn the_starter_with_report_closes_descriptor_4_before_its_exec() {
    let (dienst, vier) = UnixStream::pair().expect("pair");
    let (st, _) = laufen(
        KIND,
        &[
            eltern(),
            s(HALTEN_MELDEN),
            s("/bin/sh"),
            s("-c"),
            sitzung_skript(false),
        ],
        Some(vier),
        &[],
    );
    assert_eq!(st.code(), Some(7));
    assert_eq!(
        gelesen(&dienst),
        Vec::<u8>::new(),
        "the start ends the connection without a line"
    );
}

#[test]
fn the_starter_with_report_but_without_descriptor_4_never_runs_the_program() {
    let o = Ordner::neu("ohne-vier");
    let lief = o.0.join("lief");
    let (st, _) = laufen(
        KIND,
        &[
            eltern(),
            s(HALTEN_MELDEN),
            s("/bin/sh"),
            s("-c"),
            format!("echo x > '{}'; exit 7", lief.display()),
        ],
        None,
        &[],
    );
    assert_eq!(st.code(), Some(i32::from(Fehler::Intern.exitcode())));
    assert!(!lief.exists());
}

#[test]
fn a_parent_that_is_not_the_parent_is_refused_in_every_form() {
    let o = Ordner::neu("fremd");
    let lief = o.0.join("lief");
    let skript = format!("echo x > '{}'; exit 7", lief.display());
    for art in ["--", HALTEN] {
        let (st, _) = laufen(
            KIND,
            &[s("1"), s(art), s("/bin/sh"), s("-c"), skript.clone()],
            None,
            &[],
        );
        assert_eq!(st.code(), Some(KIND_AUFRUF_FEHLER), "{art}");
    }
    let (dienst, vier) = UnixStream::pair().expect("pair");
    let (st, _) = laufen(
        KIND,
        &[s("1"), s(HALTEN_MELDEN), s("/bin/sh"), s("-c"), skript],
        Some(vier),
        &[],
    );
    assert_eq!(st.code(), Some(i32::from(Fehler::Intern.exitcode())));
    assert_eq!(
        gelesen(&dienst),
        lauf::meldung::zeile(&Fehler::Intern).as_bytes()
    );
    assert!(!lief.exists());
}

#[test]
fn a_program_that_cannot_run_has_the_code_of_its_form() {
    let fehlt = s("/nicht/da/programm");
    let (st, _) = laufen(KIND, &[eltern(), s("--"), fehlt.clone()], None, &[]);
    assert_eq!(st.code(), Some(KIND_START_FEHLER));
    let harness = Fehler::HarnessFehlt;
    let (st, fehler) = laufen(KIND, &[eltern(), s(HALTEN), fehlt.clone()], None, &[]);
    assert_eq!(st.code(), Some(i32::from(harness.exitcode())));
    assert_eq!(fehler, harness.zeile());
    let (dienst, vier) = UnixStream::pair().expect("pair");
    let (st, fehler) = laufen(KIND, &[eltern(), s(HALTEN_MELDEN), fehlt], Some(vier), &[]);
    assert_eq!(st.code(), Some(i32::from(harness.exitcode())));
    assert_eq!(fehler, harness.zeile());
    assert_eq!(gelesen(&dienst), lauf::meldung::zeile(&harness).as_bytes());
}

#[test]
fn the_trampoline_and_the_watcher_refuse_a_wrong_call() {
    for args in [
        vec![],
        vec![s(WACHE)],
        vec![s(WACHE), s("x"), s("/a"), s("/b")],
        vec![eltern(), s("-"), s("/bin/true")],
    ] {
        let (st, _) = laufen(KIND, &args, None, &[]);
        assert_eq!(st.code(), Some(KIND_AUFRUF_FEHLER), "{args:?}");
    }
}

fn starten(skript: &str, marke: &Path) -> Result<kind::Kind, Ausfall> {
    kind::starten(
        Path::new(KIND),
        Path::new("/bin/sh"),
        &[s("-c"), s(skript), marke.display().to_string()],
        BEREIT_FRIST,
    )
}

#[test]
fn a_ready_child_is_the_program_with_its_own_pid() {
    let o = Ordner::neu("pid");
    let marke = o.0.join("pid");
    let k = starten(
        &format!("echo $$ > \"$0\"; {BEREIT}; exec /bin/sleep 30"),
        &marke,
    )
    .expect("ready");
    let pid = k.pid();
    assert!(lebt(pid));
    assert_eq!(
        std::fs::read_to_string(&marke).expect("pid file").trim(),
        pid.to_string()
    );
    k.beenden();
    assert!(!lebt(pid));
}

#[test]
fn closing_the_output_frees_a_child_that_writes() {
    let o = Ordner::neu("aus");
    let marke = o.0.join("zu");
    // the child writes until the pipe is closed (SIGPIPE ignored: the write fails), then leaves a mark
    let mut k = starten(
        &format!("trap '' PIPE; {BEREIT}; while printf xxxxxxxxxxxxxxxx 2>/dev/null; do :; done; echo zu > \"$0\"; exec /bin/sleep 30"),
        &marke,
    )
    .expect("ready");
    std::thread::sleep(Duration::from_millis(300));
    assert!(!marke.exists(), "the pipe is full and open");
    k.ausgabe_schliessen();
    assert!(
        bis(Duration::from_secs(10), || marke.exists()),
        "the write failed after the close"
    );
    k.beenden();
}

#[test]
fn the_end_of_a_child_waits_for_its_handler_and_ends_one_that_ignores_sigterm() {
    let o = Ordner::neu("ende");
    let marke = o.0.join("term");
    let k = starten(
        &format!("trap '/bin/sleep 0.3; echo t > \"$0\"; exit 0' TERM; {BEREIT}; while :; do /bin/sleep 0.05; done"),
        &marke,
    )
    .expect("ready");
    let pid = k.pid();
    k.beenden();
    assert!(!lebt(pid));
    assert!(
        marke.exists(),
        "SIGTERM came first and the handler had its time"
    );
    let stur = starten(
        &format!("trap '' TERM; {BEREIT}; exec /bin/sleep 300"),
        &marke,
    )
    .expect("ready");
    let pid = stur.pid();
    let beginn = Instant::now();
    stur.beenden();
    assert!(!lebt(pid), "SIGKILL after the grace time");
    assert!(beginn.elapsed() < Duration::from_secs(20));
}

#[test]
fn a_child_that_does_not_start_is_missing_and_one_that_ends_is_not_ready() {
    let o = Ordner::neu("ausfall");
    let marke = o.0.join("x");
    let fehlt = kind::starten(
        Path::new(KIND),
        Path::new("/nicht/da/programm"),
        &[],
        BEREIT_FRIST,
    );
    assert_eq!(
        fehlt.map(|k| k.pid()).err(),
        Some(Ausfall::Fehlt),
        "code 127 of the trampoline"
    );
    // the code of a wrong call counts as missing too
    assert_eq!(
        starten("exit 125", &marke).map(|k| k.pid()).err(),
        Some(Ausfall::Fehlt)
    );
    for anders in ["exit 0", "exit 1", "exit 126"] {
        assert_eq!(
            starten(anders, &marke).map(|k| k.pid()).err(),
            Some(Ausfall::NichtBereit),
            "{anders}"
        );
    }
}

/// The variable of the helper process of the watcher test (see below); without it the helper test does nothing.
const WACHE_FALL: &str = "WB_LAUF_WACHE_FALL";

/// The helper of the watcher: starts two children (one that ends on SIGTERM after its handler, one that ignores it) and
/// the watcher, writes their PIDs and waits for the end of its standard input. Its end is the end of "Lauf starten".
#[test]
fn wache_helfer() {
    let Ok(ordner) = std::env::var(WACHE_FALL) else {
        return;
    };
    let ordner = PathBuf::from(ordner);
    let a = starten(
        &format!("trap '/bin/sleep 0.3; echo t > \"$0\"; exit 0' TERM; {BEREIT}; while :; do /bin/sleep 0.05; done"),
        &ordner.join("a-term"),
    )
    .expect("child a");
    let b = starten(
        &format!("trap '' TERM; {BEREIT}; exec /bin/sleep 300"),
        &ordner.join("b"),
    )
    .expect("child b");
    let w = kind::wache_starten(
        Path::new(KIND),
        [&a, &b],
        &ordner.join("sockel"),
        &ordner.join("sitzung"),
    )
    .expect("watcher");
    // through a file, made whole by a rename: the output of the test runner may stand before a line on stdout
    let teil = ordner.join("pids.teil");
    std::fs::write(&teil, format!("{} {} {w}\n", a.pid(), b.pid())).expect("pids");
    std::fs::rename(&teil, ordner.join("pids")).expect("pids");
    let mut rest = String::new();
    let _ = std::io::stdin().read_to_string(&mut rest);
    // the children stay: the watcher ends them
    std::mem::forget(a);
    std::mem::forget(b);
}

#[test]
fn the_watcher_waits_for_the_parent_then_ends_both_children_and_removes_the_folders() {
    let o = Ordner::neu("wache");
    let sockel = o.0.join("sockel");
    let sitzung = o.0.join("sitzung");
    std::fs::create_dir_all(sockel.join("innen")).expect("socket folder");
    std::fs::create_dir_all(sitzung.join("zustand")).expect("session folder");
    let mut helfer = Command::new(std::env::current_exe().expect("test program"))
        .args(["--exact", "wache_helfer", "--nocapture", "--test-threads=1"])
        .env(WACHE_FALL, &o.0)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("helper");
    let datei = o.0.join("pids");
    let mut geendet = false;
    if !bis(Duration::from_secs(15), || {
        geendet = geendet || matches!(helfer.try_wait(), Ok(Some(_)));
        geendet || datei.exists()
    }) || !datei.exists()
    {
        let _ = helfer.kill();
        let _ = helfer.wait();
        panic!("the helper wrote no PIDs");
    }
    let zeile = std::fs::read_to_string(&datei).expect("pids");
    let pids: Vec<u32> = zeile
        .split_whitespace()
        .map(|w| w.parse().expect("pid"))
        .collect();
    let [a, b, w] = pids[..] else {
        panic!("{zeile}");
    };
    // while the parent lives the watcher waits and touches nothing
    std::thread::sleep(Duration::from_millis(500));
    assert!(lebt(a) && lebt(b) && lebt(w), "{a} {b} {w}");
    assert!(sockel.exists() && sitzung.exists());
    drop(helfer.stdin.take());
    let _ = helfer.wait();
    assert!(
        bis(Duration::from_secs(15), || !lebt(b) && !sockel.exists()),
        "the child that ignores SIGTERM is killed and the socket folder removed"
    );
    assert!(!lebt(a));
    assert!(!lebt(w) || bis(Duration::from_secs(5), || !lebt(w)));
    assert!(!sitzung.exists());
    assert!(
        o.0.join("a-term").exists(),
        "the first child had its grace time"
    );
}

#[test]
fn the_program_refuses_a_wrong_call_and_a_tls_variable_before_any_file() {
    let aufruf = Fehler::Aufruf;
    let (st, fehler) = laufen(LAUF, &[], None, &[]);
    assert_eq!(st.code(), Some(i32::from(aufruf.exitcode())));
    assert_eq!(fehler, aufruf.zeile());
    let tls = Fehler::TlsUmgebung;
    let (st, fehler) = laufen(
        LAUF,
        &[s("/srv/beispiel/start.json")],
        None,
        &[("SSL_CERT_FILE", "/srv/beispiel/x")],
    );
    assert_eq!(st.code(), Some(i32::from(tls.exitcode())));
    assert_eq!(fehler, tls.zeile());
}

#[test]
fn the_report_needs_descriptor_4_and_carries_the_line_of_the_first_fault() {
    let o = Ordner::neu("meldung");
    let sock = o.0.join("m.sock");
    let start = s("/nicht/da/start.json");
    let unlesbar = Fehler::DateiUnlesbar;
    // without the option: the start file cannot be read
    let (st, fehler) = laufen(LAUF, std::slice::from_ref(&start), None, &[]);
    assert_eq!(st.code(), Some(i32::from(unlesbar.exitcode())));
    assert_eq!(fehler, unlesbar.zeile());
    // with the option and descriptors 3 and 4 free: the line of that fault goes to the service as well
    let hoerer = std::os::unix::net::UnixListener::bind(&sock).expect("socket");
    let mit = [s("--meldung"), sock.display().to_string(), start.clone()];
    let (st, fehler) = laufen(LAUF, &mit, None, &[]);
    assert_eq!(st.code(), Some(i32::from(unlesbar.exitcode())));
    assert_eq!(fehler, unlesbar.zeile());
    let v = annehmen(&hoerer);
    assert_eq!(gelesen(&v), lauf::meldung::zeile(&unlesbar).as_bytes());
    // with descriptor 4 taken (here by a socket of the test): no place for the hand-over, the report says so
    let (_rest, vier) = UnixStream::pair().expect("pair");
    let fehlt = Fehler::MeldungFehlt;
    let (st, fehler) = laufen(LAUF, &mit, Some(vier), &[]);
    assert_eq!(st.code(), Some(i32::from(fehlt.exitcode())));
    assert_eq!(fehler, fehlt.zeile());
    let v = annehmen(&hoerer);
    assert_eq!(gelesen(&v), lauf::meldung::zeile(&fehlt).as_bytes());
    // descriptor 4 taken but no option: nothing to report, the fault of the start file
    let (_rest, vier) = UnixStream::pair().expect("pair");
    let (st, _) = laufen(LAUF, &[start], Some(vier), &[]);
    assert_eq!(st.code(), Some(i32::from(unlesbar.exitcode())));
}

#[test]
fn the_command_of_the_mark_refuses_a_wrong_call() {
    let (st, fehler) = laufen(ORDNERMARKE, &[], None, &[]);
    assert_ne!(st.code(), Some(0));
    assert_ne!(fehler, "", "the line of the fault");
}
