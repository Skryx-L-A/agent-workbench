//! The program `wb-bruecke` as a process: call errors, the port line, the files, SIGTERM and SIGINT (exit code 0, files
//! gone), and that the token never reaches stdout or stderr. Own folders under `/tmp`, own processes, all ended at the end.
#![cfg(unix)] // the crate is Unix only (see `src/lib.rs`)
#![allow(clippy::assert_is_empty)]

use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

const PROGRAMM: &str = env!("CARGO_BIN_EXE_wb-bruecke");

struct Ordner(PathBuf);

impl Ordner {
    fn neu(name: &str) -> Self {
        let p = PathBuf::from("/tmp").join(format!("wbbr-prog-{}-{name}", std::process::id()));
        fs::create_dir(&p).expect("folder");
        fs::set_permissions(&p, fs::Permissions::from_mode(0o700)).expect("chmod");
        Self(p)
    }

    fn pfad(&self, n: &str) -> String {
        self.0.join(n).to_str().expect("utf8").to_string()
    }
}

impl Drop for Ordner {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn start_datei(o: &Ordner, extra: &str) -> String {
    let p = o.pfad("start.json");
    let text = format!(
        r#"{{"dienst": {{"socket": "{}"}}, "port": 0, "token_datei": "{}", "deflate": false{extra}}}"#,
        o.pfad("d.sock"),
        o.pfad("t")
    );
    fs::write(&p, text).expect("write");
    p
}

fn starten(datei: &str) -> Child {
    Command::new(PROGRAMM)
        .arg(datei)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("program starts")
}

fn port_lesen(kind: &mut Child) -> (u16, BufReader<std::process::ChildStdout>) {
    let mut aus = BufReader::new(kind.stdout.take().expect("stdout piped"));
    let mut zeile = String::new();
    aus.read_line(&mut zeile).expect("a line");
    let port = zeile
        .strip_prefix("PORT ")
        .and_then(|r| r.trim_end().parse().ok())
        .expect("PORT <n>");
    (port, aus)
}

fn beenden(kind: &mut Child, signal: &str) -> Option<i32> {
    Command::new("kill")
        .args([signal, &kind.id().to_string()])
        .status()
        .expect("kill runs");
    let ende = Instant::now() + Duration::from_secs(10);
    while Instant::now() < ende {
        if let Some(s) = kind.try_wait().expect("wait") {
            return s.code();
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    let _ = kind.kill();
    let _ = kind.wait();
    None
}

fn rest(r: &mut impl Read) -> String {
    let mut s = String::new();
    let _ = r.read_to_string(&mut s);
    s
}

#[test]
fn call_errors_are_exit_code_2_with_one_line_and_an_empty_stdout() {
    for args in [vec![], vec!["a", "b"], vec!["/nicht/da.json"]] {
        let aus = Command::new(PROGRAMM).args(&args).output().expect("runs");
        assert_eq!(aus.status.code(), Some(2), "{args:?}");
        assert!(aus.stdout.is_empty());
        let fehler = String::from_utf8_lossy(&aus.stderr).into_owned();
        assert_eq!(fehler.lines().count(), 1, "{fehler:?}");
        assert!(fehler.starts_with("wb-bruecke: "));
    }
}

#[test]
fn an_existing_token_file_is_exit_code_3_and_stays() {
    let o = Ordner::neu("besteht");
    let datei = start_datei(&o, "");
    fs::write(o.pfad("t"), b"alt").expect("write");
    let aus = Command::new(PROGRAMM).arg(&datei).output().expect("runs");
    assert_eq!(aus.status.code(), Some(3));
    assert!(aus.stdout.is_empty());
    assert_eq!(String::from_utf8_lossy(&aus.stderr).lines().count(), 1);
    assert_eq!(fs::read(o.pfad("t")).expect("read"), b"alt");
}

#[test]
fn a_busy_port_is_exit_code_1_and_leaves_no_file() {
    let besetzt = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = besetzt.local_addr().expect("addr").port();
    let o = Ordner::neu("belegt");
    let datei = start_datei(&o, "");
    let text = fs::read_to_string(&datei)
        .expect("read")
        .replace("\"port\": 0", &format!("\"port\": {port}"));
    fs::write(&datei, text).expect("write");
    let aus = Command::new(PROGRAMM).arg(&datei).output().expect("runs");
    assert_eq!(aus.status.code(), Some(1));
    assert!(aus.stdout.is_empty());
    assert_eq!(String::from_utf8_lossy(&aus.stderr).lines().count(), 1);
    assert!(fs::metadata(o.pfad("t")).is_err());
}

fn lauf_und_ende(signal: &str) {
    let o = Ordner::neu(&format!("lauf{}", signal.trim_start_matches('-')));
    let datei = start_datei(&o, &format!(r#", "port_datei": "{}""#, o.pfad("p")));
    let mut kind = starten(&datei);
    let (port, mut aus) = port_lesen(&mut kind);
    let token =
        fs::read_to_string(o.pfad("t")).expect("token file exists when the port line is out");
    assert_eq!(token.len(), 65);
    assert_eq!(
        fs::metadata(o.pfad("t")).expect("stat").mode() & 0o7777,
        0o600
    );
    assert_eq!(
        fs::read_to_string(o.pfad("p")).expect("port file"),
        format!("{port}\n")
    );
    // one request that is refused, one that is served: the token is in no output
    let mut s = TcpStream::connect(("127.0.0.1", port)).expect("connect");
    s.write_all(
        format!(
            "GET /strom HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nAuthorization: Bearer {}x\r\n\r\n",
            token.trim_end()
        )
        .as_bytes(),
    )
    .expect("write");
    let mut antwort = String::new();
    let _ = s.read_to_string(&mut antwort);
    assert!(antwort.starts_with("HTTP/1.1 401 "), "{antwort}");
    let code = beenden(&mut kind, signal);
    assert_eq!(code, Some(0), "exit code after {signal}");
    let stdout = rest(&mut aus);
    let stderr = rest(kind.stderr.as_mut().expect("stderr piped"));
    assert_eq!(stdout, "");
    assert_eq!(stderr, "wb-bruecke: abgelehnt token\n");
    assert!(!stdout.contains(token.trim_end()) && !stderr.contains(token.trim_end()));
    assert!(fs::metadata(o.pfad("t")).is_err() && fs::metadata(o.pfad("p")).is_err());
    assert!(TcpStream::connect(("127.0.0.1", port)).is_err());
}

#[test]
fn sigterm_ends_the_program_with_0_and_removes_the_files() {
    lauf_und_ende("-TERM");
}

#[test]
fn sigint_ends_the_program_with_0_and_removes_the_files() {
    lauf_und_ende("-INT");
}

/// The owner of `/proc/<pid>/stat`: the user of the process, or root while the process cannot be dumped.
#[cfg(target_os = "linux")]
fn eigentuemer(pid: u32) -> u32 {
    fs::metadata(format!("/proc/{pid}/stat"))
        .expect("proc")
        .uid()
}

/// Kernabbild (step 2a), Linux: the running bridge shields its memory from core images: its `/proc/<pid>/stat` no longer belongs to the user,
/// and `/proc/<pid>/limits` shows `Max core file size 1 1`.
#[cfg(target_os = "linux")]
#[test]
fn the_running_bridge_is_shielded_from_core_images() {
    let eigener = fs::metadata("/proc/self/stat").expect("proc").uid();
    if eigener == 0 {
        // as root the files of a process that cannot be dumped belong to root as before: the test cannot say anything
        return;
    }
    let o = Ordner::neu("abschirmung");
    let datei = start_datei(&o, "");
    let mut kind = starten(&datei);
    let (_port, _aus) = port_lesen(&mut kind);
    let pid = kind.id();
    assert_ne!(
        eigentuemer(pid),
        eigener,
        "the files of /proc/<pid> belong to root while the bridge cannot be dumped"
    );
    let limits = fs::read_to_string(format!("/proc/{pid}/limits")).expect("limits");
    let zeile = limits
        .lines()
        .find(|z| z.starts_with("Max core file size"))
        .expect("the line of the core limit");
    let felder: Vec<&str> = zeile.split_whitespace().collect();
    assert_eq!((felder[4], felder[5]), ("1", "1"), "{zeile}");
    assert_eq!(beenden(&mut kind, "-TERM"), Some(0));
}
