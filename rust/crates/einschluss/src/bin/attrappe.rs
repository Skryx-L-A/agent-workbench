//! The program `einschluss-attrappe`: stand-in harness and stand-in search process of the outside tests.
//! Only test builds have it (feature `attrappe`).
//!
//! `einschluss-attrappe harness <probe> [argument ...]` makes one probe and writes one line.
//! `einschluss-attrappe such <socket> <route>` is the stand-in search process; it runs until it is killed.
//! `einschluss-attrappe weiter <socket> <address>` is the stand-in relay from a Unix socket to a TCP address.
//! `einschluss-attrappe cloud <socket> <placeholder> <mark>` is the stand-in relay of the cloud exit: it swaps the
//! placeholder of the token for the mark and answers how many it found.
//! Exit code 2: wrong call or unknown probe; 1: the stand-in search process could not start.

use std::io::{self, Write};
use std::path::Path;
use std::process::ExitCode;

use einschluss::attrappe;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let meldung = |text: &str| {
        let _ = writeln!(io::stderr(), "einschluss-attrappe: {text}");
    };
    match args.first().map(String::as_str) {
        Some("harness") => {
            let mut aus = io::stdout().lock();
            match attrappe::lauf(&args[1..], &mut aus)
                .and_then(|()| aus.flush().map_err(|e| e.to_string()))
            {
                Ok(()) => ExitCode::SUCCESS,
                Err(text) => {
                    meldung(&text);
                    ExitCode::from(2)
                }
            }
        }
        Some("such") if args.len() == 3 => {
            match attrappe::such_server(Path::new(&args[1]), &args[2]) {
                Ok(()) => ExitCode::SUCCESS,
                Err(e) => {
                    meldung(&format!("search: {e}"));
                    ExitCode::from(1)
                }
            }
        }
        Some("cloud") if args.len() == 4 => {
            match attrappe::cloud_server(Path::new(&args[1]), &args[2], &args[3]) {
                Ok(()) => ExitCode::SUCCESS,
                Err(e) => {
                    meldung(&format!("cloud relay: {e}"));
                    ExitCode::from(1)
                }
            }
        }
        Some("weiter") if args.len() == 3 => {
            match attrappe::weiter_server(Path::new(&args[1]), &args[2]) {
                Ok(()) => ExitCode::SUCCESS,
                Err(e) => {
                    meldung(&format!("relay: {e}"));
                    ExitCode::from(1)
                }
            }
        }
        _ => {
            meldung(
                "usage: harness <probe> [argument ...] | such <socket> <route> | weiter <socket> <address> | cloud <socket> <placeholder> <mark>",
            );
            ExitCode::from(2)
        }
    }
}
