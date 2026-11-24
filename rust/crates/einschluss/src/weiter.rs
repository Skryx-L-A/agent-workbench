//! The forwarder inside the enclosure (Unix systems). The cloud harness reaches the outside through one proxy address
//! on the own loopback, `127.0.0.1:8402`; this forwarder listens there and passes every connection, byte for byte in
//! both directions, to the Unix socket of the relay outside. It knows no host, no certificate and no key: what the
//! harness says goes through, and the relay decides.
//!
//! The binary `einschluss-weiter` calls [`starten`]: `einschluss-weiter <port> <socket> -- <program> [argument ...]`.
//! It binds the port first, then starts the program below it and ends with the exit code of the program.

use std::io::{self, Write};
use std::net::{Ipv4Addr, Shutdown, TcpListener, TcpStream};
use std::os::unix::net::UnixStream;
use std::os::unix::process::ExitStatusExt;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};
use std::thread;

/// Exit code for a wrong call.
const AUFRUFFEHLER: u8 = 2;
/// Exit code when the port cannot be bound.
const PORT_FEHLER: u8 = 1;
/// Exit code when the program cannot be started (the shell convention for "not found").
const START_FEHLER: u8 = 127;

/// Binds the loopback address of the forwarder.
///
/// # Errors
/// The port is taken or cannot be bound.
pub fn hoeren(port: u16) -> io::Result<TcpListener> {
    TcpListener::bind((Ipv4Addr::LOCALHOST, port))
}

/// Accepts connections on `hoerer` for ever and passes each to the Unix socket `socket`.
pub fn leiten(hoerer: &TcpListener, socket: &Path) {
    for verbindung in hoerer.incoming() {
        let Ok(tcp) = verbindung else { continue };
        let socket = socket.to_path_buf();
        thread::spawn(move || {
            let _ = verbinden(tcp, &socket);
        });
    }
}

/// One connection: the bytes of the client go to the socket, the bytes of the socket go to the client. A closed
/// direction is passed on as a closed direction (half close), so a request that ends with the end of the stream still
/// gets its answer.
fn verbinden(tcp: TcpStream, socket: &Path) -> io::Result<()> {
    let unix = UnixStream::connect(socket)?;
    let mut tcp_lesen = tcp.try_clone()?;
    let mut unix_schreiben = unix.try_clone()?;
    let hin = thread::spawn(move || {
        let _ = io::copy(&mut tcp_lesen, &mut unix_schreiben);
        let _ = unix_schreiben.shutdown(Shutdown::Write);
    });
    let (mut unix_lesen, mut tcp_schreiben) = (unix, tcp);
    let _ = io::copy(&mut unix_lesen, &mut tcp_schreiben);
    let _ = tcp_schreiben.shutdown(Shutdown::Write);
    let _ = hin.join();
    Ok(())
}

/// The parts of the call.
#[derive(Debug, PartialEq, Eq)]
struct Aufruf {
    port: u16,
    socket: PathBuf,
    programm: Vec<String>,
}

fn lesen(args: &[String]) -> Result<Aufruf, &'static str> {
    let [port, socket, naht, programm @ ..] = args else {
        return Err("usage: <port> <socket> -- <program> [argument ...]");
    };
    if naht != "--" || programm.is_empty() {
        return Err("usage: <port> <socket> -- <program> [argument ...]");
    }
    let port = port.parse::<u16>().map_err(|_| "port is not a number")?;
    if port == 0 {
        return Err("port is 0");
    }
    if !socket.starts_with('/') {
        return Err("socket path is not absolute");
    }
    Ok(Aufruf {
        port,
        socket: PathBuf::from(socket),
        programm: programm.to_vec(),
    })
}

fn meldung(text: &str) {
    let _ = writeln!(io::stderr(), "einschluss-weiter: {text}");
}

/// Binds the port, forwards in a thread, runs the program and returns its exit code (128 plus the signal number when a
/// signal ended it).
#[must_use]
pub fn starten(args: &[String]) -> ExitCode {
    let aufruf = match lesen(args) {
        Ok(a) => a,
        Err(text) => {
            meldung(text);
            return ExitCode::from(AUFRUFFEHLER);
        }
    };
    let hoerer = match hoeren(aufruf.port) {
        Ok(h) => h,
        Err(e) => {
            meldung(&format!("cannot listen: {e}"));
            return ExitCode::from(PORT_FEHLER);
        }
    };
    let socket = aufruf.socket.clone();
    thread::spawn(move || leiten(&hoerer, &socket));
    let status = Command::new(&aufruf.programm[0])
        .args(&aufruf.programm[1..])
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status();
    match status {
        Ok(s) => match (s.code(), s.signal()) {
            (Some(code), _) => ExitCode::from(u8::try_from(code & 0xff).unwrap_or(1)),
            (None, Some(signal)) => ExitCode::from(u8::try_from(128 + signal).unwrap_or(255)),
            (None, None) => ExitCode::from(1),
        },
        Err(e) => {
            meldung(&format!("cannot start the program: {e}"));
            ExitCode::from(START_FEHLER)
        }
    }
}

#[cfg(test)]
mod tests;
