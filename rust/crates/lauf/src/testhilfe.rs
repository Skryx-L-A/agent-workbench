//! Helpers of the unit tests (test build of the crate only).
//!
//! A helper process: the test program starts itself again with one test (`--exact <test>`, one thread), with a case in
//! [`FALL`] and a set of descriptors the shell makes: every descriptor above 3 is closed, then `umlenkung` applies. Some
//! parts of the program need a process of their own: the reserved descriptors 3 and 4 exist only when they are the lowest
//! free numbers, the count of threads is one only without the threads of the test runner, and a process that ends
//! itself (`uebergabe::enden`) ends the test program.
//!
//! The helpers that start processes are used by the tests of Linux only.
#![cfg_attr(not(target_os = "linux"), allow(dead_code))]

use std::process::{Command, ExitStatus, Stdio};
use std::time::{Duration, Instant};

/// The variable that names the case of a helper process; the helper tests do nothing without it.
pub const FALL: &str = "WB_LAUF_TESTFALL";

/// The line a helper writes when its case passed.
pub const OK: &str = "TESTFALL-OK";

/// Runs the test `test` (its full path, `modul::tests::name`) as a helper process with the case `fall` and waits at
/// most `frist`. Returns the end of the helper (`None`: still running at the limit, then killed) and its standard output
/// and standard error.
#[must_use]
pub fn helfer(
    test: &str,
    fall: &str,
    umlenkung: &str,
    frist: Duration,
) -> (Option<ExitStatus>, String) {
    let exe = std::env::current_exe().expect("test program");
    // bash closes every descriptor above 3 it holds, whatever its number (a runner may hand down numbers far above 9)
    let mut c = Command::new("bash");
    c.args([
        "-c",
        &format!(
            "for f in /proc/$$/fd/*; do n=${{f##*/}}; if [ \"$n\" -gt 3 ]; then eval \"exec $n>&-\"; fi; done; exec \"$0\" \"$@\" {umlenkung}"
        ),
    ])
    .arg(&exe)
    .args(["--exact", test, "--nocapture", "--test-threads=1"])
    .env(FALL, fall)
    .stdin(Stdio::null());
    // the output goes into a file, not a pipe: a grandchild that keeps a copy of a pipe would hold the read of the test
    // until it ends
    let ordner = Ordner::neu("helfer");
    let pfad = ordner.0.join("ausgabe");
    let datei = std::fs::File::create(&pfad).expect("output file");
    c.stdout(datei.try_clone().expect("output file"))
        .stderr(datei);
    let mut kind = c.spawn().expect("helper process");
    drop(c);
    let ende = Instant::now() + frist;
    let status = loop {
        if let Some(s) = kind.try_wait().expect("wait") {
            break Some(s);
        }
        if ende.saturating_duration_since(Instant::now()).is_zero() {
            let _ = kind.kill();
            let _ = kind.wait();
            break None;
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    let ausgabe = std::fs::read_to_string(&pfad).unwrap_or_default();
    (status, ausgabe)
}

/// Like [`helfer`]: the case must end with code 0 and [`OK`] within 15 s (a case takes well under a second; a hang fails
/// within the limit of a mutant run).
pub fn helfer_gut(test: &str, fall: &str, umlenkung: &str) {
    let (status, ausgabe) = helfer(test, fall, umlenkung, Duration::from_secs(15));
    assert_eq!(status.and_then(|s| s.code()), Some(0), "{fall}: {ausgabe}");
    assert!(ausgabe.contains(OK), "{fall}: {ausgabe}");
}

/// The case of this process, if it is a helper started for the test `test` (the variable alone is not enough: the
/// helper of another module runs with `--exact` and never reaches this test).
#[must_use]
pub fn fall() -> Option<String> {
    std::env::var(FALL).ok()
}

/// Writes [`OK`] at the end of a case.
pub fn fertig() {
    #[allow(clippy::print_stdout)]
    {
        println!("{OK}");
    }
}

/// Runs `f` in a thread and waits at most `frist` for its result: a call that hangs fails the test within the limit
/// instead of hanging it (the thread stays behind and ends with the test program).
pub fn mit_frist<T: Send + 'static>(frist: Duration, f: impl FnOnce() -> T + Send + 'static) -> T {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(f());
    });
    rx.recv_timeout(frist)
        .unwrap_or_else(|e| panic!("no result within {frist:?}: {e}"))
}

/// Accepts the first connection on `hoerer` within `frist` (a program that never connects fails the test instead of
/// hanging it).
#[cfg(unix)]
pub fn annehmen(
    hoerer: &std::os::unix::net::UnixListener,
    frist: Duration,
) -> std::os::unix::net::UnixStream {
    hoerer.set_nonblocking(true).expect("non-blocking");
    let ende = Instant::now() + frist;
    loop {
        match hoerer.accept() {
            Ok((v, _)) => {
                v.set_nonblocking(false).expect("blocking");
                return v;
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                assert!(
                    !ende.saturating_duration_since(Instant::now()).is_zero(),
                    "no connection within {frist:?}"
                );
                std::thread::sleep(Duration::from_millis(10));
            }
            Err(e) => panic!("accept: {e}"),
        }
    }
}

/// A folder of its own below the temporary folder, empty, removed again when the value is dropped.
pub struct Ordner(pub std::path::PathBuf);

impl Ordner {
    /// A new empty folder whose name carries `name`, the PID and a counter.
    #[must_use]
    pub fn neu(name: &str) -> Ordner {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static ZAEHLER: AtomicUsize = AtomicUsize::new(0);
        let n = ZAEHLER.fetch_add(1, Ordering::SeqCst);
        let o = std::env::temp_dir().join(format!("lauf-{name}-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&o);
        std::fs::create_dir_all(&o).expect("test folder");
        // the canonical form: the checks of the program compare canonical paths
        Ordner(std::fs::canonicalize(&o).expect("canonical"))
    }
}

impl Drop for Ordner {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
