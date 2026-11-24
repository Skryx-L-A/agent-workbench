//! Measurement of the answer time and the number of threads with 64 connections (contract `bruecke`, section 11). It is not
//! a gate: the test is ignored and runs only when asked, alone on an otherwise idle host:
//!
//! ```text
//! cargo test -p bruecke --release --test messung -- --ignored --nocapture
//! ```
//!
//! It starts the built program with a stand-in for the service, opens 63 streams (a stream is a connection that stays), and
//! measures: the threads of the process without a connection, with 64 streams and after they end; the time of 300 requests
//! of the page while 63 streams are open (the 64th connection is the request); the time of the answer 503 when 64 are open;
//! the resident memory. The numbers are printed; the test checks only that the limits hold (no more than 64 connections,
//! thread count not above twice the connections plus a constant).
#![cfg(unix)] // the crate is Unix only (see `src/lib.rs`)
#![allow(clippy::print_stderr, clippy::too_many_lines)]

use std::fs;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixListener;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

const PROGRAMM: &str = env!("CARGO_BIN_EXE_wb-bruecke");

fn threads(pid: u32) -> usize {
    if let Ok(s) = fs::read_to_string(format!("/proc/{pid}/status")) {
        return s
            .lines()
            .find_map(|z| z.strip_prefix("Threads:"))
            .and_then(|r| r.trim().parse().ok())
            .expect("Threads: line");
    }
    let aus = Command::new("ps")
        .args(["-M", "-p", &pid.to_string()])
        .output()
        .expect("ps runs");
    String::from_utf8_lossy(&aus.stdout)
        .lines()
        .count()
        .saturating_sub(1)
}

fn rss_kib(pid: u32) -> u64 {
    let aus = Command::new("ps")
        .args(["-o", "rss=", "-p", &pid.to_string()])
        .output()
        .expect("ps runs");
    String::from_utf8_lossy(&aus.stdout)
        .trim()
        .parse()
        .unwrap_or(0)
}

fn lesen_bis_kopf(s: &mut TcpStream) -> String {
    let mut alles = Vec::new();
    let mut b = [0u8; 1024];
    while !alles.windows(4).any(|w| w == b"\r\n\r\n") {
        let n = s.read(&mut b).expect("read");
        assert!(n > 0, "closed before the head");
        alles.extend_from_slice(&b[..n]);
    }
    String::from_utf8_lossy(&alles).into_owned()
}

/// The value at the percentile `p` (0 to 100) of a sorted list, nearest rank, in whole numbers.
fn prozent(v: &[Duration], p: usize) -> Duration {
    let i = (v.len() * p).div_ceil(100);
    v[i.clamp(1, v.len()) - 1]
}

fn beenden(kind: &mut Child) {
    let _ = Command::new("kill")
        .args(["-TERM", &kind.id().to_string()])
        .status();
    let ende = Instant::now() + Duration::from_secs(10);
    while Instant::now() < ende {
        if kind.try_wait().expect("wait").is_some() {
            return;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    let _ = kind.kill();
    let _ = kind.wait();
}

#[test]
#[ignore = "measurement: runs alone on request, not in the gates"]
fn measurement_with_64_connections() {
    let ordner = PathBuf::from("/tmp").join(format!("wbbr-messung-{}", std::process::id()));
    fs::create_dir(&ordner).expect("folder");
    fs::set_permissions(&ordner, fs::Permissions::from_mode(0o700)).expect("chmod");
    let sock = ordner.join("d.sock");
    let hoerer = UnixListener::bind(&sock).expect("stand-in");
    std::thread::spawn(move || {
        let mut gehalten = Vec::new();
        while let Ok((mut s, _)) = hoerer.accept() {
            let _ = s.write_all(&[0, 0, 0, 2, 6, b'x']);
            gehalten.push(s);
        }
    });
    let start = ordner.join("start.json");
    fs::write(
        &start,
        format!(
            r#"{{"dienst": {{"socket": "{}"}}, "port": 0, "token_datei": "{}", "deflate": false}}"#,
            sock.display(),
            ordner.join("t").display()
        ),
    )
    .expect("start file");
    let mut kind = Command::new(PROGRAMM)
        .arg(&start)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("starts");
    let mut zeile = String::new();
    let mut aus = std::io::BufReader::new(kind.stdout.take().expect("piped"));
    std::io::BufRead::read_line(&mut aus, &mut zeile).expect("line");
    let port: u16 = zeile
        .strip_prefix("PORT ")
        .and_then(|r| r.trim_end().parse().ok())
        .expect("PORT");
    let token = fs::read_to_string(ordner.join("t"))
        .expect("token")
        .trim_end()
        .to_string();
    let pid = kind.id();
    let strom = format!(
        "GET /strom HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nAuthorization: Bearer {token}\r\n\r\n"
    );
    let seite = format!("GET / HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n\r\n");
    std::thread::sleep(Duration::from_millis(200));
    let faeden_leer = threads(pid);
    let rss_leer = rss_kib(pid);
    // 63 streams
    let mut strome = Vec::new();
    for _ in 0..63 {
        let mut s = TcpStream::connect(("127.0.0.1", port)).expect("connect");
        s.write_all(strom.as_bytes()).expect("write");
        let kopf = lesen_bis_kopf(&mut s);
        assert!(kopf.starts_with("HTTP/1.1 200 "), "{kopf}");
        strome.push(s);
    }
    std::thread::sleep(Duration::from_millis(200));
    let faeden_63 = threads(pid);
    // 300 requests of the page as the 64th connection
    let mut zeiten = Vec::new();
    for _ in 0..300 {
        let t = Instant::now();
        let mut s = TcpStream::connect(("127.0.0.1", port)).expect("connect");
        s.write_all(seite.as_bytes()).expect("write");
        let mut alles = Vec::new();
        s.read_to_end(&mut alles).expect("read");
        zeiten.push(t.elapsed());
        assert!(alles.starts_with(b"HTTP/1.1 200 "));
    }
    zeiten.sort();
    // the 64th stream, and then the 65th connection: 503
    let mut s64 = TcpStream::connect(("127.0.0.1", port)).expect("connect");
    s64.write_all(strom.as_bytes()).expect("write");
    assert!(lesen_bis_kopf(&mut s64).starts_with("HTTP/1.1 200 "));
    strome.push(s64);
    std::thread::sleep(Duration::from_millis(200));
    let faeden_64 = threads(pid);
    let rss_64 = rss_kib(pid);
    let t = Instant::now();
    let mut s65 = TcpStream::connect(("127.0.0.1", port)).expect("connect");
    s65.write_all(seite.as_bytes()).expect("write");
    let mut antwort = Vec::new();
    s65.read_to_end(&mut antwort).expect("read");
    let zeit_503 = t.elapsed();
    assert!(
        antwort.starts_with(b"HTTP/1.1 503 "),
        "{}",
        String::from_utf8_lossy(&antwort)
    );
    drop(strome);
    std::thread::sleep(Duration::from_millis(500));
    let faeden_nach = threads(pid);
    eprintln!("MESSUNG bruecke, 64 Verbindungen (Prozess {pid})");
    eprintln!("  Faeden ohne Verbindung:      {faeden_leer}");
    eprintln!("  Faeden mit 63 Stroemen:      {faeden_63}");
    eprintln!("  Faeden mit 64 Stroemen:      {faeden_64}");
    eprintln!("  Faeden nach dem Schliessen:  {faeden_nach}");
    eprintln!(
        "  Antwortzeit GET / bei 63 Stroemen (300 Anfragen): min {:?}, Median {:?}, p95 {:?}, p99 {:?}, max {:?}",
        zeiten[0],
        prozent(&zeiten, 50),
        prozent(&zeiten, 95),
        prozent(&zeiten, 99),
        zeiten[zeiten.len() - 1]
    );
    eprintln!("  Antwort 503 bei 64 Verbindungen: {zeit_503:?}");
    eprintln!("  Speicher (RSS): leer {rss_leer} KiB, mit 64 Stroemen {rss_64} KiB");
    beenden(&mut kind);
    let _ = fs::remove_dir_all(&ordner);
    assert!(faeden_64 <= 2 * 64 + 8, "{faeden_64} threads");
    assert!(
        faeden_nach <= faeden_leer + 2,
        "threads do not go back: {faeden_nach} after, {faeden_leer} before"
    );
}
