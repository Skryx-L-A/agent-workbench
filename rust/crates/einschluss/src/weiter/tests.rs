use std::fs;
use std::io::{Read, Write};
use std::net::{Shutdown, TcpStream};
use std::os::unix::net::UnixListener;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use super::*;

static ZAEHLER: AtomicU32 = AtomicU32::new(0);

/// A short throw-away folder for a socket (the path of a Unix socket is limited to about 104 bytes).
struct Ordner(PathBuf);

impl Ordner {
    fn neu() -> Ordner {
        let n = ZAEHLER.fetch_add(1, Ordering::SeqCst);
        let basis = std::env::var_os("EINSCHLUSS_TESTORT")
            .map_or_else(|| PathBuf::from("/var/tmp"), PathBuf::from);
        let pfad = basis.join(format!("ei-w-{}-{n}", std::process::id()));
        let _ = fs::remove_dir_all(&pfad);
        fs::create_dir_all(&pfad).expect("folder");
        Ordner(pfad)
    }
}

impl Drop for Ordner {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn a(teile: &[&str]) -> Vec<String> {
    teile.iter().map(|t| (*t).to_owned()).collect()
}

/// A stand-in relay: reads until the end of the request, answers with the request in upper case and the text `|ende`.
fn relay(socket: &Path) {
    let hoerer = UnixListener::bind(socket).expect("relay socket");
    thread::spawn(move || {
        for v in hoerer.incoming() {
            let Ok(mut v) = v else { continue };
            thread::spawn(move || {
                let mut frage = Vec::new();
                v.read_to_end(&mut frage).expect("request");
                let mut antwort = frage.to_ascii_uppercase();
                antwort.extend_from_slice(b"|ende");
                v.write_all(&antwort).expect("answer");
            });
        }
    });
}

fn mit_forwarder(socket: &Path) -> u16 {
    let hoerer = hoeren_frei();
    let port = hoerer.local_addr().expect("addr").port();
    let socket = socket.to_path_buf();
    thread::spawn(move || leiten(&hoerer, &socket));
    port
}

fn hoeren_frei() -> TcpListener {
    TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).expect("free port")
}

fn frage(port: u16, daten: &[u8]) -> Vec<u8> {
    let mut c = TcpStream::connect((Ipv4Addr::LOCALHOST, port)).expect("connect");
    c.set_read_timeout(Some(Duration::from_secs(10)))
        .expect("timeout");
    c.write_all(daten).expect("write");
    halb_schliessen(&c);
    let mut antwort = Vec::new();
    let _ = c.read_to_end(&mut antwort);
    antwort
}

/// The half close of the client. A forwarder that closed at once (no socket) may have reset the connection before it:
/// macOS then answers ENOTCONN (measured: 40 of 40 with a pause between write and half close, 0 of 40 without). The
/// answer decides, not that call; every other error of the half close stays an abort.
fn halb_schliessen(c: &TcpStream) {
    match c.shutdown(Shutdown::Write) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::NotConnected => {}
        Err(e) => panic!("half close: {e:?}"),
    }
}

#[test]
fn a_request_goes_to_the_socket_and_the_answer_comes_back_after_a_half_close() {
    let o = Ordner::neu();
    let socket = o.0.join("r.sock");
    relay(&socket);
    let port = mit_forwarder(&socket);
    assert_eq!(frage(port, b"hallo"), b"HALLO|ende");
}

#[test]
fn a_large_request_passes_in_both_directions_unchanged() {
    let o = Ordner::neu();
    let socket = o.0.join("r.sock");
    relay(&socket);
    let port = mit_forwarder(&socket);
    let daten: Vec<u8> = (0..3_000_000u32).map(|i| b'a' + (i % 23) as u8).collect();
    let mut soll = daten.to_ascii_uppercase();
    soll.extend_from_slice(b"|ende");
    let antwort = {
        // the request is written from a thread: the answer cannot start before the request ends
        let mut c = TcpStream::connect((Ipv4Addr::LOCALHOST, port)).expect("connect");
        c.set_read_timeout(Some(Duration::from_secs(20)))
            .expect("timeout");
        let mut schreiber = c.try_clone().expect("clone");
        let senden = daten.clone();
        let faden = thread::spawn(move || {
            schreiber.write_all(&senden).expect("write");
            schreiber.shutdown(Shutdown::Write).expect("half close");
        });
        let mut antwort = Vec::new();
        c.read_to_end(&mut antwort).expect("read");
        faden.join().expect("writer");
        antwort
    };
    assert_eq!(antwort.len(), soll.len());
    assert!(
        antwort == soll,
        "the answer differs from the request in upper case"
    );
}

#[test]
fn two_connections_at_once_do_not_mix() {
    let o = Ordner::neu();
    let socket = o.0.join("r.sock");
    relay(&socket);
    let port = mit_forwarder(&socket);
    let erste = thread::spawn(move || frage(port, b"eins"));
    let zweite = thread::spawn(move || frage(port, b"zwei"));
    assert_eq!(erste.join().expect("first"), b"EINS|ende");
    assert_eq!(zweite.join().expect("second"), b"ZWEI|ende");
}

#[test]
fn a_missing_socket_closes_the_connection_without_an_answer() {
    let o = Ordner::neu();
    let port = mit_forwarder(&o.0.join("fehlt.sock"));
    assert_eq!(frage(port, b"hallo"), b"");
}

/// The order that a loaded machine makes by chance, made on purpose: the request is written, the forwarder (no socket)
/// closes and resets the connection, and only then comes the half close. It must not abort the request, and there is
/// still no answer.
#[test]
fn a_half_close_after_the_forwarder_closed_does_not_abort() {
    let o = Ordner::neu();
    let port = mit_forwarder(&o.0.join("fehlt.sock"));
    let mut c = TcpStream::connect((Ipv4Addr::LOCALHOST, port)).expect("connect");
    c.set_read_timeout(Some(Duration::from_secs(10)))
        .expect("timeout");
    c.write_all(b"hallo").expect("write");
    // the end of the connection from the side of the forwarder: an end of file or a reset, never data
    let mut antwort = Vec::new();
    let _ = c.read_to_end(&mut antwort);
    assert_eq!(antwort, b"");
    halb_schliessen(&c);
}

#[test]
fn the_forwarder_listens_on_the_loopback_only() {
    let hoerer = hoeren(0).expect("bind");
    assert!(hoerer.local_addr().expect("addr").ip().is_loopback());
    assert_eq!(hoerer.local_addr().expect("addr").ip(), Ipv4Addr::LOCALHOST);
}

#[test]
fn the_call_is_read_in_full_or_refused() {
    let gut = lesen(&a(&[
        "8402",
        "/run/brain/modell.sock",
        "--",
        "/bin/sh",
        "-c",
        "x",
    ]));
    assert_eq!(
        gut,
        Ok(Aufruf {
            port: 8402,
            socket: PathBuf::from("/run/brain/modell.sock"),
            programm: a(&["/bin/sh", "-c", "x"]),
        })
    );
    for schlecht in [
        a(&[]),
        a(&["8402", "/s"]),
        a(&["8402", "/s", "--"]),
        a(&["8402", "/s", "x", "prog"]),
        a(&["abc", "/s", "--", "p"]),
        a(&["70000", "/s", "--", "p"]),
        a(&["0", "/s", "--", "p"]),
        a(&["8402", "relativ.sock", "--", "p"]),
    ] {
        assert!(lesen(&schlecht).is_err(), "{schlecht:?}");
    }
}

#[test]
fn the_start_gives_the_exit_code_of_the_program() {
    // the real function, with real programs: the code is compared as text of the ExitCode
    let o = Ordner::neu();
    let socket = o.0.join("s.sock").to_string_lossy().into_owned();
    let lauf = |programm: &[&str]| {
        // a new port for each run: the forwarder of an earlier run keeps its port until the test process ends
        // (another test thread may take the port between the pick and the bind: ask again, a taken port is code 1)
        let mut ergebnis = String::new();
        for _ in 0..20 {
            let frei = hoeren_frei().local_addr().expect("addr").port().to_string();
            let mut args = a(&[&frei, &socket, "--"]);
            args.extend(a(programm));
            ergebnis = format!("{:?}", starten(&args));
            if ergebnis != format!("{:?}", ExitCode::from(PORT_FEHLER)) {
                break;
            }
        }
        ergebnis
    };
    assert_eq!(
        lauf(&["/bin/sh", "-c", "exit 0"]),
        format!("{:?}", ExitCode::from(0))
    );
    assert_eq!(
        lauf(&["/bin/sh", "-c", "exit 3"]),
        format!("{:?}", ExitCode::from(3))
    );
    assert_eq!(
        lauf(&["/bin/sh", "-c", "kill -9 $$"]),
        format!("{:?}", ExitCode::from(137))
    );
    assert_eq!(
        lauf(&["/nicht/da/programm"]),
        format!("{:?}", ExitCode::from(START_FEHLER))
    );
}

#[test]
fn the_start_refuses_a_wrong_call_and_a_taken_port() {
    assert_eq!(
        format!("{:?}", starten(&a(&["1"]))),
        format!("{:?}", ExitCode::from(AUFRUFFEHLER))
    );
    let belegt = hoeren_frei();
    let port = belegt.local_addr().expect("addr").port().to_string();
    assert_eq!(
        format!("{:?}", starten(&a(&[&port, "/s", "--", "/bin/true"]))),
        format!("{:?}", ExitCode::from(PORT_FEHLER))
    );
}

#[test]
fn the_program_runs_while_the_forwarder_already_listens() {
    // a thread asks through the forwarder after the start; a port that another test thread took in between is code 1:
    // then the whole run is made again with a new port
    let o = Ordner::neu();
    let socket = o.0.join("r.sock");
    relay(&socket);
    let socket_text = socket.to_string_lossy().into_owned();
    for _ in 0..20 {
        let frei = hoeren_frei();
        let port = frei.local_addr().expect("addr").port();
        drop(frei);
        let fragesteller = thread::spawn(move || {
            for _ in 0..200 {
                if TcpStream::connect((Ipv4Addr::LOCALHOST, port)).is_ok() {
                    return frage(port, b"abc");
                }
                thread::sleep(Duration::from_millis(25));
            }
            Vec::new()
        });
        let ende = starten(&a(&[
            &port.to_string(),
            &socket_text,
            "--",
            "/bin/sleep",
            "1",
        ]));
        if format!("{ende:?}") == format!("{:?}", ExitCode::from(PORT_FEHLER)) {
            continue;
        }
        assert_eq!(format!("{ende:?}"), format!("{:?}", ExitCode::from(0)));
        assert_eq!(fragesteller.join().expect("asker"), b"ABC|ende");
        return;
    }
    panic!("no free port after 20 tries");
}
