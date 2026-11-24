//! The program `einschluss-weiter` as a program: the call, the exit code, and the bytes it passes. It is part of the
//! delivery, so these tests run without the feature `attrappe`.
#![cfg(unix)]
#![allow(clippy::print_stderr)]

use std::io::{Read, Write};
use std::net::{Ipv4Addr, Shutdown, TcpListener, TcpStream};
use std::os::unix::net::UnixListener;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const WEITER: &str = env!("CARGO_BIN_EXE_einschluss-weiter");

fn lauf(args: &[&str]) -> (Option<i32>, String) {
    let aus = Command::new(WEITER)
        .args(args)
        .env_clear()
        .stdin(Stdio::null())
        .output()
        .expect("the program starts");
    (
        aus.status.code(),
        String::from_utf8_lossy(&aus.stderr).into_owned(),
    )
}

fn freier_port() -> u16 {
    TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .expect("port")
        .local_addr()
        .expect("addr")
        .port()
}

#[test]
fn a_wrong_call_ends_with_2_and_a_usage_line() {
    let (code, fehler) = lauf(&[]);
    assert_eq!(code, Some(2));
    assert!(fehler.contains("usage"), "{fehler}");
    assert_eq!(lauf(&["8402", "/s"]).0, Some(2));
    assert_eq!(lauf(&["abc", "/s", "--", "/bin/true"]).0, Some(2));
}

/// How often a test starts again when its port was taken in the gap between `freier_port` and the bind of the
/// forwarder (another process asked the kernel for a free port in that moment; seen as a red test under load).
const PORT_VERSUCHE: usize = 5;

/// Runs the forwarder on a free port with `rest` (socket, `--`, program ...). A start that ends with exit 1 and
/// `cannot listen` lost the race for the port; it is made again with a new port. Any other outcome is returned.
fn lauf_auf_freiem_port(rest: &[&str]) -> (Option<i32>, String) {
    let mut letzte = (None, String::new());
    for _ in 0..PORT_VERSUCHE {
        let port = freier_port().to_string();
        let mut args = vec![port.as_str()];
        args.extend_from_slice(rest);
        letzte = lauf(&args);
        if !(letzte.0 == Some(1) && letzte.1.contains("cannot listen")) {
            return letzte;
        }
        eprintln!("weiter-wiederholung: cannot listen");
    }
    letzte
}

#[test]
fn the_exit_code_of_the_program_is_the_exit_code_of_the_forwarder() {
    assert_eq!(
        lauf_auf_freiem_port(&["/s.sock", "--", "/bin/sh", "-c", "exit 0"]).0,
        Some(0)
    );
    assert_eq!(
        lauf_auf_freiem_port(&["/s.sock", "--", "/bin/sh", "-c", "exit 7"]).0,
        Some(7)
    );
    assert_eq!(
        lauf_auf_freiem_port(&["/s.sock", "--", "/bin/sh", "-c", "kill -9 $$"]).0,
        Some(137)
    );
    let (code, fehler) = lauf_auf_freiem_port(&["/s.sock", "--", "/nicht/da/programm"]);
    assert_eq!(code, Some(127));
    assert!(fehler.contains("cannot start"), "{fehler}");
}

#[test]
fn a_taken_port_ends_with_1() {
    let belegt = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).expect("port");
    let port = belegt.local_addr().expect("addr").port().to_string();
    let (code, fehler) = lauf(&[&port, "/s.sock", "--", "/bin/true"]);
    assert_eq!(code, Some(1));
    assert!(fehler.contains("cannot listen"), "{fehler}");
}

/// A throw-away folder that goes away when the test ends, also when an assertion fails.
struct Weg(PathBuf);

impl Drop for Weg {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// The program of the forwarder in the bytes test: writes the mark file, then waits. `einschluss-weiter` binds its port
/// first and starts the program afterwards (`weiter.rs`, `starten`), so the mark says that the forwarder of this test
/// holds the port.
const MARKEN_PROGRAMM: &str = ": > \"$1\"; exec /bin/sleep 3";

/// The port of this test comes from `freier_port`, which gives the number back to the kernel at once. In the gap
/// another process can take it, and the forwarder then ends with exit 1 and `cannot listen` before the mark is there.
/// Only that is made again, with a new port, at most `PORT_VERSUCHE` times. After the mark there is exactly one
/// exchange: every I/O error and every wrong answer is red, with the standard error of the forwarder in the message.
#[test]
fn the_program_passes_the_bytes_while_the_child_runs() {
    let basis = std::env::var_os("EINSCHLUSS_TESTORT")
        .map_or_else(|| PathBuf::from("/var/tmp"), PathBuf::from);
    let ordner = basis.join(format!("ei-wp-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&ordner);
    std::fs::create_dir_all(&ordner).expect("folder");
    let _weg = Weg(ordner.clone());
    let socket = ordner.join("r.sock");
    let hoerer = UnixListener::bind(&socket).expect("relay");
    std::thread::spawn(move || {
        for v in hoerer.incoming() {
            let Ok(mut v) = v else { continue };
            let mut frage = Vec::new();
            let _ = v.read_to_end(&mut frage);
            let _ = v.write_all(&frage.to_ascii_uppercase());
        }
    });
    for versuch in 0..PORT_VERSUCHE {
        let port = freier_port();
        let marke = ordner.join(format!("bereit-{versuch}"));
        let fehlerdatei = ordner.join(format!("stderr-{versuch}"));
        let mut kind = Command::new(WEITER)
            .args([
                &port.to_string(),
                socket.to_str().expect("utf8"),
                "--",
                "/bin/sh",
                "-c",
                MARKEN_PROGRAMM,
                "sh",
                marke.to_str().expect("utf8"),
            ])
            .env_clear()
            .stdin(Stdio::null())
            .stderr(std::fs::File::create(&fehlerdatei).expect("stderr file"))
            .spawn()
            .expect("the program starts");
        let meldung = || std::fs::read_to_string(&fehlerdatei).unwrap_or_default();
        let frist = Instant::now() + Duration::from_secs(5);
        let bereit = loop {
            if marke.exists() {
                break true;
            }
            if let Some(status) = kind.try_wait().expect("wait") {
                // the program never started: the forwarder ended before the mark
                if status.code() == Some(1) && meldung().contains("cannot listen") {
                    eprintln!("weiter-wiederholung: cannot listen");
                    break false;
                }
                panic!(
                    "the forwarder ended before the mark: {status}; stderr: {}",
                    meldung()
                );
            }
            assert!(
                Instant::now() < frist,
                "no mark within 5 s; stderr: {}",
                meldung()
            );
            std::thread::sleep(Duration::from_millis(10));
        };
        if !bereit {
            continue;
        }
        let ergebnis = austausch_einmal(port);
        let _ = kind.kill();
        let _ = kind.wait();
        match ergebnis {
            Ok(antwort) => assert_eq!(antwort, b"DURCHGANG", "wrong answer; stderr: {}", meldung()),
            Err(e) => panic!("the exchange failed: {e}; stderr: {}", meldung()),
        }
        return;
    }
    panic!("the port was taken in all {PORT_VERSUCHE} attempts");
}

/// The one exchange: connect, send the request with a half close, read the answer to the end.
fn austausch_einmal(port: u16) -> std::io::Result<Vec<u8>> {
    let mut c = TcpStream::connect((Ipv4Addr::LOCALHOST, port))?;
    c.set_read_timeout(Some(Duration::from_secs(5)))?;
    c.write_all(b"durchgang")?;
    c.shutdown(Shutdown::Write)?;
    let mut antwort = Vec::new();
    c.read_to_end(&mut antwort)?;
    Ok(antwort)
}
