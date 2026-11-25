//! The program `einschluss-attrappe`: stand-in harness and stand-in search process of the outside tests.
//! Only test builds have it (feature `attrappe`).
//!
//! `einschluss-attrappe harness <probe> [argument ...]` makes one probe and writes one line.
//! `einschluss-attrappe such <socket> <route>` is the stand-in search process; it runs until it is killed.
//! `einschluss-attrappe weiter <socket> <address>` is the stand-in relay from a Unix socket to a TCP address.
//! `einschluss-attrappe cloud <socket> <placeholder> <mark>` is the stand-in relay of the cloud exit: it swaps the
//! placeholder of the token for the mark and answers how many it found.
//! Exit code 2: wrong call or unknown probe; 1: the stand-in search process could not start.

use std::io;
use std::process::ExitCode;

use einschluss::attrappe;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    ExitCode::from(attrappe::programm(
        &args,
        &mut io::stdout().lock(),
        &mut io::stderr(),
    ))
}
