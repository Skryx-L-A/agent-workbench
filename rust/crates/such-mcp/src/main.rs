//! The program `such-mcp`: `such-mcp` or `such-mcp --socket <path>` (under macOS only the second form).
//!
//! MCP over stdio, one JSON-RPC line per message. Exit 0 at the end of stdin; 64 (`EX_USAGE` of `sysexits.h`) a wrong call, with
//! nothing written; 74 (`EX_IOERR`) a read error of stdin or a write error of stdout.

use std::ffi::OsString;
use std::io;
use std::process::ExitCode;
use such_mcp::{aufruf, mac::Plattform, suche::UnixSocket, zeilen};

/// A wrong call (`EX_USAGE`).
const EXIT_USAGE: u8 = 64;
/// A read error of stdin or a write error of stdout (`EX_IOERR`).
const EXIT_IOERR: u8 = 74;

fn main() -> ExitCode {
    let args: Vec<OsString> = std::env::args_os().skip(1).collect();
    let Some(pfad) = aufruf::socket_pfad_fuer(Plattform::AKTUELL, &args) else {
        return ExitCode::from(EXIT_USAGE);
    };
    let socket = UnixSocket::neu(pfad);
    let mut eingabe = io::stdin().lock();
    let mut ausgabe = io::stdout().lock();
    match zeilen::lauf(&mut eingabe, &mut ausgabe, &socket, zeilen::MAX_ZEILE) {
        Ok(()) => ExitCode::SUCCESS,
        Err(_) => ExitCode::from(EXIT_IOERR),
    }
}
