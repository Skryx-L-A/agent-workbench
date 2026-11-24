//! `rangkern serve` as a program: the protocol on the pipes, the exit codes, and the answers
//! against `rangkern search` on the same files after every kind of change the contract lists.
//!
//! Every vault here is invented. Each program runs with a cleared environment and only `PATH`,
//! `LANG`, `TZ` and `HOME`, as the runner of the contract does.

// Permissions and symbolic links need Unix; the program itself builds for Windows too.
#![cfg(unix)]

use std::fs::{self, FileTimes, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::PermissionsExt;
#[cfg(target_os = "linux")]
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

const BIN: &str = env!("CARGO_BIN_EXE_rangkern");

fn command(root: &Path) -> Command {
    command_with(root, &[])
}

/// [`command`] behind a prefix of programs that replace each other by `exec` (the observer prefix below): the pid stays the pid of
/// the program.
fn command_with(root: &Path, prefix: &[String]) -> Command {
    let mut command = match prefix.split_first() {
        Some((first, rest)) => {
            let mut command = Command::new(first);
            command.args(rest).arg(BIN);
            command
        }
        None => Command::new(BIN),
    };
    command
        .current_dir(root)
        .env_clear()
        .env("PATH", std::env::var_os("PATH").unwrap_or_default())
        .env("LANG", "C.UTF-8")
        .env("TZ", "UTC")
        .env("HOME", root.join("home"));
    command
}

/// A running `rangkern serve` with its pipes.
struct Serve {
    child: Arc<Mutex<Child>>,
    stdin: Option<ChildStdin>,
    stdout: BufReader<ChildStdout>,
}

impl Serve {
    fn start(root: &Path, args: &[&str]) -> Self {
        Self::start_with(root, &[], args)
    }

    fn start_with(root: &Path, prefix: &[String], args: &[&str]) -> Self {
        let mut child = command_with(root, prefix)
            .arg("serve")
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("program starts");
        let stdin = child.stdin.take();
        let stdout = BufReader::new(child.stdout.take().expect("stdout is piped"));
        // A program that does not answer would block the test for ever. After a while the watchdog
        // ends it, the pipe closes, and the test fails. A healthy test lives well under a second.
        let child = Arc::new(Mutex::new(child));
        let watched = Arc::clone(&child);
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_secs(8));
            let _ = watched.lock().map(|mut child| child.kill());
        });
        Self {
            child,
            stdin,
            stdout,
        }
    }

    fn send_raw(&mut self, bytes: &[u8]) {
        let stdin = self.stdin.as_mut().expect("stdin is open");
        stdin.write_all(bytes).expect("request is written");
        stdin.flush().expect("request is flushed");
    }

    /// The next answer line, `None` at the end of the output.
    fn read(&mut self) -> Option<Value> {
        let mut line = String::new();
        let count = self.stdout.read_line(&mut line).expect("answer is read");
        if count == 0 {
            return None;
        }
        assert!(line.ends_with('\n'), "{line:?}");
        Some(serde_json::from_str(&line).expect("answer is JSON"))
    }

    fn ask_line(&mut self, line: &str) -> Value {
        self.send_raw(format!("{line}\n").as_bytes());
        self.read().expect("an answer comes")
    }

    fn ask(&mut self, request: &Value) -> Value {
        self.ask_line(&request.to_string())
    }

    fn close(&mut self) {
        self.stdin = None;
    }

    /// Waits for the end of the program; its exit code.
    fn finish(&mut self) -> i32 {
        self.close();
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            if let Some(status) = self
                .child
                .lock()
                .expect("lock")
                .try_wait()
                .expect("status is read")
            {
                return status.code().expect("exit code");
            }
            assert!(Instant::now() < deadline, "the program does not end");
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}

impl Drop for Serve {
    fn drop(&mut self) {
        if let Ok(mut child) = self.child.lock()
            && child.try_wait().ok().flatten().is_none()
        {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("serve-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("home")).expect("home is created");
        fs::create_dir_all(root.join("vault")).expect("vault is created");
        fs::create_dir_all(root.join("aux")).expect("aux is created");
        Self(root)
    }

    fn vault(&self) -> PathBuf {
        self.0.join("vault")
    }

    fn put(&self, rel: &str, text: &str) -> PathBuf {
        let path = self.0.join(rel);
        fs::create_dir_all(path.parent().expect("has a parent")).expect("parent is created");
        fs::write(&path, text).expect("file is written");
        path
    }

    fn serve(&self, vectors: bool) -> Serve {
        let vault = self.vault();
        let file = self.0.join("aux/vectors.json");
        if vectors {
            Serve::start(
                &self.0,
                &[
                    vault.to_str().expect("UTF-8"),
                    "--vectors",
                    file.to_str().expect("UTF-8"),
                ],
            )
        } else {
            Serve::start(&self.0, &[vault.to_str().expect("UTF-8")])
        }
    }

    /// `serve` with vectors behind the observer prefix: the test stands in the user namespace of the program and may read its
    /// descriptor table although the program shields itself.
    #[cfg(target_os = "linux")]
    fn serve_observable(&self) -> Serve {
        let vault = self.vault();
        let file = self.0.join("aux/vectors.json");
        Serve::start_with(
            &self.0,
            observer_prefix(),
            &[
                vault.to_str().expect("UTF-8"),
                "--vectors",
                file.to_str().expect("UTF-8"),
            ],
        )
    }

    /// What `rangkern search` prints for the request on the files as they are now.
    fn cold(&self, request: &Value) -> Value {
        let mut args = vec![
            "search".to_string(),
            self.vault().to_string_lossy().into_owned(),
        ];
        let temp = self.0.join("cold-vectors.json");
        if let Some(query) = request.get("query_vector").filter(|v| !v.is_null()) {
            let notes = fs::read_to_string(self.0.join("aux/vectors.json"))
                .ok()
                .and_then(|text| serde_json::from_str::<Value>(&text).ok())
                .and_then(|value| value.get("notes").cloned());
            let Some(notes) = notes else {
                return json!({"error": "unreadable"});
            };
            fs::write(&temp, json!({"query": query, "notes": notes}).to_string())
                .expect("vectors are written");
            args.push("--vectors".into());
            args.push(temp.to_string_lossy().into_owned());
        }
        if let Some(k) = request.get("k") {
            args.push("-k".into());
            args.push(k.to_string());
        }
        if let Some(space) = request.get("space").and_then(Value::as_str) {
            args.push("--space".into());
            args.push(space.to_string());
        }
        if let Some(today) = request.get("today").and_then(Value::as_str) {
            args.push("--today".into());
            args.push(today.to_string());
        }
        args.push("--".into());
        args.push(request["query"].as_str().expect("query").to_string());
        let output = command(&self.0).args(&args).output().expect("program runs");
        serde_json::from_slice(&output.stdout).expect("one JSON line")
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn search(query: &str) -> Value {
    json!({"op": "search", "query": query, "today": "2026-11-18"})
}

fn with(mut request: Value, key: &str, value: Value) -> Value {
    request[key] = value;
    request
}

/// The answer without the `id`.
fn bare(mut answer: Value) -> Value {
    if let Some(object) = answer.as_object_mut() {
        object.remove("id");
    }
    answer
}

/// The open files of the process `pid` that lie under `root`.
///
/// An empty answer must mean that nothing is open there, never that the table could not be read.
/// So the check first shows that it sees the process: the descriptors 0, 1 and 2 must resolve.
/// A descriptor that is listed but gone when it is resolved (`ENOENT`) was closed in between
/// (the loader of a just started program opens and closes its libraries, for example): it counts
/// as not open. Every other error makes the test fail. `root` is resolved first, because the
/// kernel names the targets without symbolic links.
#[cfg(target_os = "linux")]
fn open_under(pid: u32, root: &Path) -> Vec<PathBuf> {
    open_under_with(pid, root, |_| {})
}

/// The entries of `/proc/<pid>/fd`. An error is a failure of the test; its message names the status of the child and of the test
/// process (`State`, `Uid`), so that a process that is not dumpable can be told from one that is gone.
#[cfg(target_os = "linux")]
fn fd_table(pid: u32) -> fs::ReadDir {
    fs::read_dir(format!("/proc/{pid}/fd")).unwrap_or_else(|e| {
        panic!(
            "fd table is readable: {e}; child {pid}: {}; test process: {}",
            status_lines(pid),
            status_lines(std::process::id())
        )
    })
}

/// The lines `State`, `Uid` and `Pid` of `/proc/<pid>/status`, for the message of a failure.
#[cfg(target_os = "linux")]
fn status_lines(pid: u32) -> String {
    match fs::read_to_string(format!("/proc/{pid}/status")) {
        Ok(text) => text
            .lines()
            .filter(|l| ["State:", "Uid:", "Pid:"].iter().any(|k| l.starts_with(k)))
            .collect::<Vec<_>>()
            .join(" | "),
        Err(e) => format!("status not readable ({e})"),
    }
}

/// [`open_under`] with a seam between listing a descriptor and resolving it: `between` gets the
/// number of each descriptor before it is resolved (a test closes one there to force the race).
#[cfg(target_os = "linux")]
fn open_under_with(pid: u32, root: &Path, mut between: impl FnMut(&str)) -> Vec<PathBuf> {
    let root = fs::canonicalize(root).expect("root resolves");
    let mut numbers = Vec::new();
    let mut targets = Vec::new();
    for entry in fd_table(pid) {
        let entry = entry.expect("fd entry is readable");
        let number = entry.file_name().to_string_lossy().into_owned();
        between(&number);
        let target = match fs::read_link(entry.path()) {
            Ok(target) => target,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => panic!("fd {number} resolves: {e}"),
        };
        numbers.push(number);
        targets.push(target);
    }
    for fd in ["0", "1", "2"] {
        assert!(
            numbers.iter().any(|n| n == fd),
            "fd {fd} is visible: {numbers:?}"
        );
    }
    targets
        .into_iter()
        .filter(|target| target.starts_with(&root))
        .collect()
}

#[test]
fn ping_and_quit_answer_in_order_and_quit_ends_the_program_with_exit_0() {
    let scratch = Scratch::new("quit");
    let mut serve = scratch.serve(false);
    assert_eq!(serve.ask_line(r#"{"op":"ping"}"#), json!({"ok": true}));
    assert_eq!(
        serve.ask_line(r#"{"op":"ping","id":"a"}"#),
        json!({"ok": true, "id": "a"})
    );
    serve.send_raw(b"{\"op\":\"quit\",\"id\":7}\n{\"op\":\"ping\"}\n");
    assert_eq!(serve.read(), Some(json!({"ok": true, "id": 7})));
    assert_eq!(serve.read(), None, "lines after quit are not read");
    assert_eq!(serve.finish(), 0);
}

#[test]
fn the_end_of_the_input_ends_the_program_with_exit_0_and_a_last_line_without_a_newline_counts() {
    let scratch = Scratch::new("eof");
    let mut serve = scratch.serve(false);
    serve.send_raw(b"{\"op\":\"ping\",\"id\":1}\r\n{\"op\":\"ping\",\"id\":2}");
    assert_eq!(serve.read(), Some(json!({"ok": true, "id": 1})));
    serve.close();
    assert_eq!(serve.read(), Some(json!({"ok": true, "id": 2})));
    assert_eq!(serve.read(), None);
    assert_eq!(serve.finish(), 0);
    let mut empty = scratch.serve(false);
    empty.close();
    assert_eq!(empty.read(), None);
    assert_eq!(empty.finish(), 0);
}

#[test]
fn an_invalid_line_is_answered_bad_request_and_the_program_runs_on() {
    let scratch = Scratch::new("bad");
    let mut serve = scratch.serve(false);
    let bad = json!({"error": "bad_request"});
    assert_eq!(serve.ask_line(""), bad);
    assert_eq!(serve.ask_line("not json"), bad);
    assert_eq!(serve.ask_line("[1]"), bad);
    assert_eq!(serve.ask_line(r#"{"op":"ping","op":"ping"}"#), bad);
    assert_eq!(
        serve.ask_line(r#"{"op":"nope","id":3}"#),
        json!({"error": "bad_request", "id": 3})
    );
    assert_eq!(serve.ask_line(r#"{"op":"ping","id":[1]}"#), bad);
    assert_eq!(
        serve.ask_line(r#"{"op":"search","query":"x","k":5.0}"#),
        bad
    );
    assert_eq!(serve.ask_line(r#"{"op":"ping","query":"x"}"#), bad);
    let frame = "{\"op\":\"ping\",\"id\":\"\"}".len();
    let exact = format!(
        "{{\"op\":\"ping\",\"id\":\"{}\"}}",
        "x".repeat((1 << 20) - frame)
    );
    assert_eq!(exact.len(), 1 << 20);
    assert_eq!(serve.ask_line(&exact)["ok"], json!(true));
    let long = format!(
        "{{\"op\":\"ping\",\"id\":\"{}\"}}",
        "x".repeat((1 << 20) - frame + 1)
    );
    assert_eq!(serve.ask_line(&long), bad);
    assert_eq!(
        serve.ask_line(r#"{"op":"ping","id":1}"#),
        json!({"ok": true, "id": 1})
    );
    assert_eq!(serve.finish(), 0);
}

#[test]
fn a_vault_that_is_no_folder_is_answered_unreadable_until_it_is_one() {
    let scratch = Scratch::new("none");
    fs::remove_dir_all(scratch.vault()).expect("vault is removed");
    let mut serve = scratch.serve(false);
    let request = with(search("x"), "id", json!("i"));
    assert_eq!(
        serve.ask(&request),
        json!({"error": "unreadable", "id": "i"})
    );
    assert_eq!(serve.ask_line(r#"{"op":"ping"}"#), json!({"ok": true}));
    scratch.put("vault/a.md", "x\n");
    let answer = serve.ask(&request);
    assert_eq!(answer["hits"][0]["rel"], json!("a.md"));
    assert_eq!(answer["id"], json!("i"));
    assert_eq!(serve.finish(), 0);
}

#[test]
fn wrong_usage_is_exit_2_without_output() {
    let scratch = Scratch::new("usage");
    let vault = scratch.vault();
    let vault = vault.to_str().expect("UTF-8");
    let cases: [&[&str]; 7] = [
        &[],
        &[vault, "extra"],
        &[vault, "--vec", "x"],
        &[vault, "-v", "x"],
        &[vault, "--vectors"],
        &["--licenses"],
        &[vault, "--licenses"],
    ];
    for args in cases {
        let output = command(&scratch.0)
            .arg("serve")
            .args(args)
            .stdin(Stdio::null())
            .output()
            .expect("program runs");
        assert_eq!(output.status.code(), Some(64), "{args:?}");
        assert!(output.stdout.is_empty(), "{args:?}");
    }
}

/// The program that observes a shielded `serve` (rule of the contract `dienst`, section 16, point 6; decision of 21.11.2026): `serve` starts
/// as `unshare -Ur -- setpriv --bounding-set=-all --inh-caps=-all -- rangkern serve ...`. `unshare` and `setpriv` replace each other
/// by `exec`, so the pid stays the pid of `serve`; this test owns the user namespace and may read the descriptor table. A missing
/// `unshare` or `setpriv`, or a user namespace that is not allowed, is a failure of the test with its reason, never a skip.
#[cfg(target_os = "linux")]
fn observer_prefix() -> &'static [String] {
    static PREFIX: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
    PREFIX.get_or_init(|| {
        let find = |name: &str| -> String {
            let path = std::env::var_os("PATH").unwrap_or_default();
            std::env::split_paths(&path)
                .map(|dir| dir.join(name))
                .find(|file| file.is_file())
                .unwrap_or_else(|| panic!("user namespace unusable: `{name}` is missing"))
                .to_string_lossy()
                .into_owned()
        };
        let prefix: Vec<String> = vec![
            find("unshare"),
            "-Ur".to_string(),
            "--".to_string(),
            find("setpriv"),
            "--bounding-set=-all".to_string(),
            "--inh-caps=-all".to_string(),
            "--".to_string(),
        ];
        let probe = Command::new(&prefix[0])
            .args(&prefix[1..])
            .arg("true")
            .output()
            .expect("the observer prefix starts");
        assert!(
            probe.status.success(),
            "user namespace not allowed (`unshare -Ur`): {}",
            String::from_utf8_lossy(&probe.stderr).trim()
        );
        prefix
    })
}

/// A stable vault of 30 notes with a small vector file, and the request of the long runs.
fn stable_vault(name: &str) -> (Scratch, Value) {
    let scratch = Scratch::new(name);
    for i in 0..30 {
        scratch.put(
            &format!("vault/d{}/n{i}.md", i % 3),
            &format!("---\ntitle: Notiz {i}\n---\ngarten beet kaffee {i}\n"),
        );
    }
    scratch.put(
        "aux/vectors.json",
        r#"{"notes": {"d0/n0.md": [1, 0], "d1/n1.md": [0, 1]}}"#,
    );
    let request = with(search("garten kaffee"), "query_vector", json!([1, 0]));
    (scratch, request)
}

/// Fifty searches give the same answer as the first and as a cold `search`, and no file of the folder stays open: measured on the
/// program `serve` itself, from inside its user namespace (see [`observer_prefix`]). The check has its positive control (`open_under`
/// needs the descriptors 0, 1 and 2 to resolve) and is red when the table cannot be read. On other systems the fifty searches run
/// without the check of open files, as before.
#[test]
fn fifty_searches_without_a_change_give_the_same_answer_and_no_file_stays_open() {
    let (scratch, request) = stable_vault("stable");
    #[cfg(target_os = "linux")]
    let mut serve = scratch.serve_observable();
    #[cfg(not(target_os = "linux"))]
    let mut serve = scratch.serve(true);
    let first = serve.ask(&request);
    assert_ne!(first["hits"], json!([]));
    for _ in 0..50 {
        assert_eq!(serve.ask(&request), first);
    }
    assert_eq!(bare(first), scratch.cold(&request));
    #[cfg(target_os = "linux")]
    {
        let pid = serve.child.lock().expect("lock").id();
        let open = open_under(pid, &scratch.0);
        assert!(open.is_empty(), "files stay open: {open:?}");
    }
    assert_eq!(serve.finish(), 0);
}

/// Counter-check of the shield: without the user namespace the descriptor table of `serve` cannot be read by the same user
/// (`PermissionDenied`; the files of the process belong to root).
#[cfg(target_os = "linux")]
#[test]
fn without_the_user_namespace_the_fd_table_of_serve_cannot_be_read() {
    let (scratch, request) = stable_vault("closed");
    let mut serve = scratch.serve(true);
    assert_ne!(serve.ask(&request)["hits"], json!([]));
    let pid = serve.child.lock().expect("lock").id();
    let e = fs::read_dir(format!("/proc/{pid}/fd")).expect_err("the table of fd is closed");
    assert_eq!(e.kind(), std::io::ErrorKind::PermissionDenied);
    assert_eq!(serve.finish(), 0);
}

/// The same check on the same code in this process: no file stays open when the input ends while the server still lives.
#[cfg(target_os = "linux")]
#[test]
fn no_file_stays_open_in_the_loop_of_the_test_process() {
    let (scratch, request) = stable_vault("schleife");
    no_file_stays_open_in_the_loop(&scratch, &request);
}

/// A reader of request lines that checks the open files of this process when its lines are used up, just before it reports the end of
/// the input: at that moment the server of `serve_lines` is still alive.
#[cfg(target_os = "linux")]
struct PruefLeser<'a> {
    inner: std::io::Cursor<Vec<u8>>,
    root: &'a Path,
    offen: Option<Vec<PathBuf>>,
}

#[cfg(target_os = "linux")]
impl std::io::Read for PruefLeser<'_> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        std::io::Read::read(&mut self.inner, buf)
    }
}

#[cfg(target_os = "linux")]
impl BufRead for PruefLeser<'_> {
    fn fill_buf(&mut self) -> std::io::Result<&[u8]> {
        let ende =
            usize::try_from(self.inner.position()).expect("position") >= self.inner.get_ref().len();
        if ende && self.offen.is_none() {
            self.offen = Some(open_under(std::process::id(), self.root));
        }
        self.inner.fill_buf()
    }

    fn consume(&mut self, amt: usize) {
        self.inner.consume(amt);
    }
}

/// 51 requests of `request` through `run_with_input` in this process: no file of the folder is open when the input ends.
#[cfg(target_os = "linux")]
fn no_file_stays_open_in_the_loop(scratch: &Scratch, request: &Value) {
    let zeilen = format!("{request}\n").repeat(51);
    let mut leser = PruefLeser {
        inner: std::io::Cursor::new(zeilen.into_bytes()),
        root: &scratch.0,
        offen: None,
    };
    let args = [
        "serve".to_string(),
        scratch.vault().to_string_lossy().into_owned(),
        "--vectors".to_string(),
        scratch
            .0
            .join("aux/vectors.json")
            .to_string_lossy()
            .into_owned(),
    ];
    let (mut out, mut err) = (Vec::new(), Vec::new());
    assert_eq!(
        rangkern::run_with_input(&args, &mut leser, &mut out, &mut err),
        0
    );
    let antworten = String::from_utf8(out).expect("text").lines().count();
    assert_eq!(antworten, 51, "every request is answered");
    let offen = leser.offen.expect("the check ran at the end of the input");
    assert!(offen.is_empty(), "files stay open: {offen:?}");
}

/// The counter-check of the test above: the same check finds a file that a process holds open,
/// also when the folder is given over a symbolic link.
#[cfg(target_os = "linux")]
#[test]
fn the_check_for_open_files_finds_a_file_that_a_process_holds_open() {
    let scratch = Scratch::new("held");
    let held = scratch.put("vault/held.md", "garten\n");
    let alias = scratch.0.join("alias");
    std::os::unix::fs::symlink(&scratch.0, &alias).expect("link is created");
    // `spawn` returns while the child is still inside its `exec` (the parent runs on when the old memory is released): the table of
    // the child then still holds the close-on-exec copy of the file, and its new memory is not dumpable yet. The child therefore
    // writes a line when it runs (after the `exec` of `sh`), and the test reads it before it looks. `sleep` runs as a child of the
    // shell, in a process group that the test ends as a whole.
    let mut child = Command::new("sh")
        .args(["-c", "echo ready; sleep 30; true"])
        .process_group(0)
        .stdin(fs::File::open(&held).expect("file opens"))
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("the shell starts");
    let mut line = String::new();
    BufReader::new(child.stdout.take().expect("stdout is piped"))
        .read_line(&mut line)
        .expect("the child says it runs");
    assert_eq!(line, "ready\n");
    let direct = open_under(child.id(), &scratch.0);
    let linked = open_under(child.id(), &alias);
    // The group is ended by the system call, not by the program `kill`: its forms for a negative PID differ between the
    // systems (the `kill` of procps, Debian and Ubuntu, did not end `kill -KILL -<pgid>` successfully).
    let group = rustix::process::kill_process_group(
        rustix::process::Pid::from_raw(i32::try_from(child.id()).expect("pid")).expect("pid > 0"),
        rustix::process::Signal::KILL,
    );
    assert!(group.is_ok(), "the process group ends: {group:?}");
    let _ = child.wait();
    let expected = vec![fs::canonicalize(&held).expect("file resolves")];
    assert_eq!(direct, expected);
    assert_eq!(linked, expected);
}

/// The race of the check, forced: a descriptor that is closed between listing and resolving
/// counts as not open and is no error; the descriptors 0, 1 and 2 are still required.
#[cfg(target_os = "linux")]
#[test]
fn a_descriptor_closed_between_listing_and_resolving_counts_as_not_open() {
    use std::os::fd::AsRawFd;
    let scratch = Scratch::new("race");
    let held = scratch.put("vault/held.md", "garten\n");
    let mut file = Some(fs::File::open(&held).expect("file opens"));
    let number = file.as_ref().expect("open").as_raw_fd().to_string();
    let mut closed = false;
    let open = open_under_with(std::process::id(), &scratch.0, |seen| {
        if seen == number {
            drop(file.take());
            closed = true;
        }
    });
    assert!(closed, "the seam saw fd {number}");
    assert!(open.is_empty(), "a closed descriptor is not open: {open:?}");
}

#[test]
fn the_program_writes_no_file() {
    let scratch = Scratch::new("readonly");
    scratch.put("vault/a.md", "garten\n");
    scratch.put("aux/vectors.json", r#"{"notes": {"a.md": [1]}}"#);
    let listing = |root: &Path| {
        let mut all = Vec::new();
        let mut pending = vec![root.to_path_buf()];
        while let Some(dir) = pending.pop() {
            for entry in fs::read_dir(&dir).expect("readable").flatten() {
                let meta = entry.metadata().expect("metadata");
                all.push((entry.path(), meta.len(), meta.modified().ok()));
                if meta.is_dir() {
                    pending.push(entry.path());
                }
            }
        }
        all.sort();
        all
    };
    let before = listing(&scratch.0);
    let mut serve = scratch.serve(true);
    serve.ask(&with(search("garten"), "query_vector", json!([1])));
    serve.ask(&search("garten"));
    assert_eq!(serve.finish(), 0);
    assert_eq!(listing(&scratch.0), before);
}

/// Asks the program and `rangkern search`; the answers must be equal (the `id` apart).
fn assert_same(serve: &mut Serve, scratch: &Scratch, request: &Value, what: &str) {
    let warm = bare(serve.ask(request));
    let cold = scratch.cold(request);
    assert_eq!(warm, cold, "{what}: {request}");
}

fn requests() -> Vec<Value> {
    vec![
        search("garten"),
        search("kaffee beet"),
        with(search("garten"), "query_vector", json!([1, 0])),
        with(
            with(search("kaffee"), "query_vector", json!([0, 1])),
            "k",
            json!(2),
        ),
        with(search("garten"), "space", json!("pp")),
        with(search("zebrawort"), "k", json!(3)),
        with(search("zeb+rawort"), "k", json!(3)),
    ]
}

fn all_same(serve: &mut Serve, scratch: &Scratch, what: &str) {
    for request in requests() {
        assert_same(serve, scratch, &request, what);
    }
}

/// Rewrites a file with text of the same size and sets `mtime` back to what it was.
fn rewrite_keeping_mtime(path: &Path, text: &str) {
    let old = fs::metadata(path)
        .and_then(|meta| meta.modified())
        .expect("mtime");
    fs::write(path, text).expect("rewritten");
    OpenOptions::new()
        .write(true)
        .open(path)
        .expect("opens")
        .set_times(FileTimes::new().set_modified(old))
        .expect("mtime is reset");
}

#[test]
fn every_change_of_the_contract_list_shows_in_the_next_answer() {
    let scratch = Scratch::new("changes");
    scratch.put("vault/a.md", "---\ntitle: Garten\n---\ngarten beet\n");
    scratch.put("vault/b.md", "kaffee beet zebrawort\n");
    scratch.put("vault/p/c.md", "garten kaffee\n");
    scratch.put("vault/q/quellen/orig.md", "original garten\n");
    scratch.put("vault/q/n.md", "notiz garten\n");
    scratch.put(
        "aux/vectors.json",
        r#"{"notes": {"a.md": [1, 0], "b.md": [0, 1], "p/c.md": [1, 1], "90-secrets/s.md": [1, 0]}}"#,
    );
    let mut serve = scratch.serve(true);
    all_same(&mut serve, &scratch, "start");
    // a note's content, also with the same size and the old mtime
    let a = scratch.vault().join("a.md");
    scratch.put(
        "vault/a.md",
        "---\ntitle: Garten\n---\nkaffee kaffee kaffee\n",
    );
    all_same(&mut serve, &scratch, "content");
    rewrite_keeping_mtime(&a, "---\ntitle: Garten\n---\ngarten garten garten\n");
    all_same(&mut serve, &scratch, "same size, old mtime");
    rewrite_keeping_mtime(&a, "---\ntitle: Garten\n---\nbeet beet beet beet\n");
    all_same(&mut serve, &scratch, "same size, old mtime again");
    // new, deleted and renamed notes and folders
    scratch.put("vault/neu/n.md", "garten ganz neu\n");
    all_same(&mut serve, &scratch, "new note");
    fs::remove_file(scratch.vault().join("b.md")).expect("removed");
    all_same(&mut serve, &scratch, "deleted note");
    fs::rename(
        scratch.vault().join("p/c.md"),
        scratch.vault().join("p/d.md"),
    )
    .expect("renamed");
    all_same(&mut serve, &scratch, "renamed note");
    fs::rename(scratch.vault().join("neu"), scratch.vault().join("alt")).expect("renamed");
    all_same(&mut serve, &scratch, "renamed folder");
    fs::create_dir_all(scratch.vault().join("leer")).expect("created");
    all_same(&mut serve, &scratch, "new empty folder");
    fs::remove_dir_all(scratch.vault().join("alt")).expect("removed");
    all_same(&mut serve, &scratch, "deleted folder");
    // spaces, source spaces, contradictions
    scratch.put("vault/.space.yml", "{\"art\": \"normal\"}");
    scratch.put("vault/q/.space.yml", "{\"art\": \"quellen\"}");
    all_same(&mut serve, &scratch, ".space.yml of a source space");
    scratch.put("vault/q/.space.yml", "{\"art\": \"normal\"}");
    all_same(&mut serve, &scratch, ".space.yml back");
    scratch.put(
        "vault/.brain/spaces.lock",
        r#"{"spaces": [{"space": "pp", "mount": "p"}]}"#,
    );
    all_same(&mut serve, &scratch, "spaces.lock");
    fs::remove_file(scratch.vault().join(".brain/spaces.lock")).expect("removed");
    all_same(&mut serve, &scratch, "spaces.lock removed");
    scratch.put(
        "vault/_meta/state/contradictions.json",
        r#"{"findings": {"1": {"status": "open", "note_a": {"rel": "a.md"}, "note_b": {"rel": "p/d.md"}}}}"#,
    );
    all_same(&mut serve, &scratch, "contradictions.json");
    // the vector file: content, deleted, created again
    scratch.put(
        "aux/vectors.json",
        r#"{"notes": {"a.md": [0, 1], "p/d.md": [1, 0], "q/n.md": [1, 1]}}"#,
    );
    all_same(&mut serve, &scratch, "vector file content");
    fs::remove_file(scratch.0.join("aux/vectors.json")).expect("removed");
    all_same(&mut serve, &scratch, "vector file deleted");
    scratch.put("aux/vectors.json", r#"{"notes": {"a.md": [1, 0]}}"#);
    all_same(&mut serve, &scratch, "vector file created again");
    rewrite_keeping_mtime(
        &scratch.0.join("aux/vectors.json"),
        r#"{"notes": {"a.md": [0, 1]}}"#,
    );
    all_same(&mut serve, &scratch, "vector file, same size, old mtime");
    scratch.put("aux/vectors.json", "{\"notes\": [");
    all_same(&mut serve, &scratch, "vector file invalid");
    scratch.put("aux/vectors.json", r#"{"notes": {"a.md": [1, 0]}}"#);
    // a note moves into an excluded folder
    fs::create_dir_all(scratch.vault().join("90-secrets")).expect("created");
    fs::rename(
        scratch.vault().join("a.md"),
        scratch.vault().join("90-secrets/a.md"),
    )
    .expect("moved");
    all_same(&mut serve, &scratch, "note moved to 90-secrets");
    fs::rename(
        scratch.vault().join("90-secrets/a.md"),
        scratch.vault().join("a.md"),
    )
    .expect("moved back");
    all_same(&mut serve, &scratch, "note moved back");
    // the date
    scratch.put(
        "vault/alt.md",
        "---\nvalid-until: 2026-12\n---\ngarten alt\n",
    );
    all_same(&mut serve, &scratch, "valid-until before");
    let mut later = search("garten");
    later["today"] = json!("2027-03-01");
    assert_same(&mut serve, &scratch, &later, "today moves on");
    assert_eq!(serve.finish(), 0);
}

#[test]
fn permissions_of_a_note_and_of_a_folder_take_effect_and_come_back() {
    let scratch = Scratch::new("modes");
    scratch.put("vault/a.md", "garten\n");
    let note = scratch.put("vault/d/b.md", "garten beet\n");
    scratch.put("aux/vectors.json", r#"{"notes": {"a.md": [1]}}"#);
    let probe = scratch.put("probe", "x");
    fs::set_permissions(&probe, fs::Permissions::from_mode(0o000)).expect("mode");
    let effective = fs::read(&probe).is_err();
    fs::set_permissions(&probe, fs::Permissions::from_mode(0o644)).expect("mode");
    if !effective {
        return;
    }
    let mut serve = scratch.serve(true);
    all_same(&mut serve, &scratch, "start");
    let folder = note.parent().expect("folder").to_path_buf();
    for target in [note.clone(), folder] {
        let old = fs::metadata(&target).expect("exists").permissions();
        fs::set_permissions(&target, fs::Permissions::from_mode(0o000)).expect("mode");
        all_same(&mut serve, &scratch, "mode 000");
        fs::set_permissions(&target, old).expect("mode back");
        all_same(&mut serve, &scratch, "mode back");
    }
    let vectors = scratch.0.join("aux/vectors.json");
    fs::set_permissions(&vectors, fs::Permissions::from_mode(0o000)).expect("mode");
    all_same(&mut serve, &scratch, "vector file mode 000");
    fs::set_permissions(&vectors, fs::Permissions::from_mode(0o644)).expect("mode back");
    all_same(&mut serve, &scratch, "vector file mode back");
    assert_eq!(serve.finish(), 0);
}

#[test]
fn a_vector_for_a_note_that_is_gone_and_a_moved_note_are_no_hits() {
    let scratch = Scratch::new("relics");
    scratch.put("vault/a.md", "garten\n");
    scratch.put("vault/b.md", "garten\n");
    scratch.put(
        "aux/vectors.json",
        r#"{"notes": {"a.md": [1, 0], "b.md": [0, 1], "90-secrets/s.md": [1, 0]}}"#,
    );
    let mut serve = scratch.serve(true);
    let request = with(search("garten"), "query_vector", json!([1, 0]));
    assert_same(&mut serve, &scratch, &request, "start");
    fs::remove_file(scratch.vault().join("a.md")).expect("removed");
    let answer = serve.ask(&request);
    let rels: Vec<&str> = answer["hits"]
        .as_array()
        .expect("hits")
        .iter()
        .map(|hit| hit["rel"].as_str().expect("rel"))
        .collect();
    assert_eq!(rels, ["b.md"], "the vector of a deleted note is a relic");
    assert_eq!(bare(answer), scratch.cold(&request));
    assert_eq!(serve.finish(), 0);
}
