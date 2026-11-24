//! `such-mcp` under macOS (addendum to contract 4, section 6): the call, and the check of the server after `connect`. The
//! rules take the platform as a parameter, so these tests run under Linux with `MacOs`. The measurement of the server
//! itself (`getpeereid`) is tested only on a Mac, at the end of this file.
#![cfg(unix)]

use std::ffi::OsString;
use std::io::{Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use such_mcp::aufruf::socket_pfad_fuer;
use such_mcp::mac::{Plattform, SOCKET_MAX_MACOS, Serverpruefung, pfad_ok, server_ok};
use such_mcp::suche::{Befrager, Unerreichbar, UnixSocket};

const MAC: Plattform = Plattform::MacOs;
const EIGEN: u32 = 501;

fn os(teile: &[&str]) -> Vec<OsString> {
    teile.iter().map(OsString::from).collect()
}

fn pfad_von(laenge: usize) -> String {
    let kopf = "/tmp/";
    format!("{kopf}{}", "a".repeat(laenge - kopf.len()))
}

#[test]
fn under_macos_the_socket_option_is_required() {
    assert_eq!(socket_pfad_fuer(MAC, &os(&[])), None);
    assert_eq!(
        socket_pfad_fuer(MAC, &os(&["--socket", "/tmp/s/such.sock"])),
        Some(PathBuf::from("/tmp/s/such.sock"))
    );
    // Under Linux nothing changes: no argument is the fixed path.
    assert_eq!(
        socket_pfad_fuer(Plattform::Linux, &os(&[])),
        Some(PathBuf::from("/run/brain/such.sock"))
    );
}

#[test]
fn under_macos_a_relative_or_too_long_path_is_a_usage_error() {
    for relativ in ["such.sock", "./such.sock", "s/such.sock"] {
        assert_eq!(
            socket_pfad_fuer(MAC, &os(&["--socket", relativ])),
            None,
            "{relativ}"
        );
        assert!(socket_pfad_fuer(Plattform::Linux, &os(&["--socket", relativ])).is_some());
    }
    assert_eq!(SOCKET_MAX_MACOS, 103);
    assert!(socket_pfad_fuer(MAC, &os(&["--socket", &pfad_von(103)])).is_some());
    assert_eq!(
        socket_pfad_fuer(MAC, &os(&["--socket", &pfad_von(104)])),
        None
    );
    assert!(socket_pfad_fuer(Plattform::Linux, &os(&["--socket", &pfad_von(104)])).is_some());
    assert!(pfad_ok(MAC, Path::new(&pfad_von(103))));
    assert!(!pfad_ok(MAC, Path::new(&pfad_von(104))));
}

#[test]
fn under_macos_every_other_call_is_a_usage_error() {
    for args in [
        os(&["--route", "lokal"]),
        os(&["--socket"]),
        os(&["--socket", ""]),
        os(&["/tmp/s/such.sock"]),
        os(&[
            "--socket",
            "/tmp/s/such.sock",
            "--socket",
            "/tmp/t/such.sock",
        ]),
        os(&["--socket", "/tmp/s/such.sock", "extra"]),
        os(&["--sock", "/tmp/s/such.sock"]),
    ] {
        assert_eq!(socket_pfad_fuer(MAC, &args), None, "{args:?}");
    }
}

#[test]
fn only_the_own_euid_may_be_the_server() {
    assert!(server_ok(MAC, Ok::<u32, ()>(EIGEN), EIGEN));
    assert!(!server_ok(MAC, Ok::<u32, ()>(EIGEN + 1), EIGEN));
    assert!(!server_ok(MAC, Ok::<u32, ()>(0), EIGEN));
    assert!(!server_ok(MAC, Err("EINVAL"), EIGEN));
    assert!(server_ok(Plattform::Linux, Ok::<u32, ()>(EIGEN + 1), EIGEN));
    assert!(server_ok(Plattform::Linux, Err(()), EIGEN));
}

#[allow(clippy::unnecessary_wraps)] // the type of the measurement is fixed by the check
fn messen_eigen(_: &UnixStream) -> std::io::Result<u32> {
    Ok(EIGEN)
}

#[allow(clippy::unnecessary_wraps)] // the type of the measurement is fixed by the check
fn messen_fremd(_: &UnixStream) -> std::io::Result<u32> {
    Ok(EIGEN + 1)
}

fn messen_scheitert(_: &UnixStream) -> std::io::Result<u32> {
    Err(std::io::Error::from(std::io::ErrorKind::InvalidInput))
}

fn pruefung(
    plattform: Plattform,
    messen: fn(&UnixStream) -> std::io::Result<u32>,
) -> Serverpruefung {
    Serverpruefung {
        plattform,
        euid: EIGEN,
        messen,
    }
}

/// A server that answers one line `antwort` to whatever comes, and reports the bytes it received.
fn server_starten(pfad: &Path, antwort: &'static str) -> mpsc::Receiver<Vec<u8>> {
    let listener = UnixListener::bind(pfad).expect("bind");
    let (sender, empfaenger) = mpsc::channel();
    std::thread::spawn(move || {
        let Ok((mut strom, _)) = listener.accept() else {
            return;
        };
        // Under macOS `setsockopt` fails with EINVAL when the peer has closed the connection already. That is the case these
        // tests want: the client refuses without a byte and closes at once. Every byte sent before the close is still read
        // below, and the read then ends with 0, so the deadline is not needed. On any other failure the test ends with the
        // deadline of `recv_timeout`.
        let _ = strom.set_read_timeout(Some(Duration::from_millis(700)));
        let mut gelesen = Vec::new();
        let mut stueck = [0u8; 256];
        loop {
            match strom.read(&mut stueck) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    gelesen.extend_from_slice(&stueck[..n]);
                    if gelesen.contains(&b'\n') {
                        let _ = strom.write_all(antwort.as_bytes());
                        break;
                    }
                }
            }
        }
        let _ = sender.send(gelesen);
    });
    empfaenger
}

fn ordner(name: &str) -> PathBuf {
    let ordner = PathBuf::from(format!("/tmp/smm-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&ordner);
    std::fs::create_dir_all(&ordner).expect("folder");
    ordner
}

#[test]
fn a_foreign_server_gets_no_byte_and_the_answer_is_unreachable() {
    let ordner = ordner("fremd");
    let pfad = ordner.join("s.sock");
    let empfangen = server_starten(&pfad, "{\"ok\":true}\n");
    let socket = UnixSocket::mit_grenzen(&pfad, Duration::from_secs(5), 1000)
        .mit_serverpruefung(pruefung(MAC, messen_fremd));
    let start = Instant::now();
    assert_eq!(socket.frage("{\"op\":\"ping\"}"), Err(Unerreichbar));
    assert!(
        start.elapsed() < Duration::from_secs(3),
        "no waiting for the deadline"
    );
    let gelesen = empfangen
        .recv_timeout(Duration::from_secs(3))
        .expect("server done");
    assert!(
        gelesen.is_empty(),
        "no byte reached the server: {gelesen:?}"
    );
    let _ = std::fs::remove_dir_all(&ordner);
}

#[test]
fn a_server_that_cannot_be_measured_gets_no_byte() {
    let ordner = ordner("unmessbar");
    let pfad = ordner.join("s.sock");
    let empfangen = server_starten(&pfad, "{\"ok\":true}\n");
    let socket = UnixSocket::mit_grenzen(&pfad, Duration::from_secs(5), 1000)
        .mit_serverpruefung(pruefung(MAC, messen_scheitert));
    assert_eq!(socket.frage("{\"op\":\"ping\"}"), Err(Unerreichbar));
    let gelesen = empfangen
        .recv_timeout(Duration::from_secs(3))
        .expect("server done");
    assert!(
        gelesen.is_empty(),
        "no byte reached the server: {gelesen:?}"
    );
    let _ = std::fs::remove_dir_all(&ordner);
}

#[test]
fn a_server_with_the_own_euid_is_asked() {
    let ordner = ordner("eigen");
    let pfad = ordner.join("s.sock");
    let empfangen = server_starten(&pfad, "{\"ok\":true}\n");
    let socket = UnixSocket::mit_grenzen(&pfad, Duration::from_secs(5), 1000)
        .mit_serverpruefung(pruefung(MAC, messen_eigen));
    assert_eq!(
        socket.frage("{\"op\":\"ping\"}"),
        Ok("{\"ok\":true}".to_owned())
    );
    let gelesen = empfangen
        .recv_timeout(Duration::from_secs(3))
        .expect("server done");
    assert_eq!(gelesen, b"{\"op\":\"ping\"}\n");
    let _ = std::fs::remove_dir_all(&ordner);
}

#[test]
fn under_linux_the_server_is_not_checked() {
    let ordner = ordner("linux");
    let pfad = ordner.join("s.sock");
    let _empfangen = server_starten(&pfad, "{\"ok\":true}\n");
    let socket = UnixSocket::mit_grenzen(&pfad, Duration::from_secs(5), 1000)
        .mit_serverpruefung(pruefung(Plattform::Linux, messen_fremd));
    assert_eq!(
        socket.frage("{\"op\":\"ping\"}"),
        Ok("{\"ok\":true}".to_owned())
    );
    let _ = std::fs::remove_dir_all(&ordner);
}

/// The real way, only on a Mac: a server of the same user is asked, with the measurement of `getpeereid`.
#[cfg(target_os = "macos")]
#[test]
fn a_server_of_the_same_user_is_asked_with_the_real_measurement() {
    let ordner = ordner("echt");
    let pfad = ordner.join("s.sock");
    let empfangen = server_starten(&pfad, "{\"ok\":true}\n");
    let socket = UnixSocket::mit_grenzen(&pfad, Duration::from_secs(5), 1000);
    assert_eq!(
        socket.frage("{\"op\":\"ping\"}"),
        Ok("{\"ok\":true}".to_owned())
    );
    let gelesen = empfangen
        .recv_timeout(Duration::from_secs(3))
        .expect("server done");
    assert_eq!(gelesen, b"{\"op\":\"ping\"}\n");
    let _ = std::fs::remove_dir_all(&ordner);
}
