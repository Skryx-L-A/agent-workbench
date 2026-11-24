//! The program `kette`: `kette pruefe [--vorher <64 hex>] [--letzte <64 hex>] <datei>...` and
//! `kette pruefe-quelle [--vorher <64 hex>] [--letzte <64 hex>] <ordner> <quelle>`.
//!
//! One line on stdout in RFC 8785. Exit 0 the chain is whole; 1 a break (or the output could not be written);
//! 64 a wrong call, with stdout empty.

use std::ffi::OsString;
use std::io::{self, Write};
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<OsString> = std::env::args_os().skip(1).collect();
    let ausgang = formate::kette::befehl(&args);
    if !ausgang.stderr.is_empty() {
        let _ = io::stderr().write_all(ausgang.stderr.as_bytes());
    }
    let mut stdout = io::stdout().lock();
    if stdout
        .write_all(ausgang.stdout.as_bytes())
        .and_then(|()| stdout.flush())
        .is_err()
    {
        let _ = writeln!(io::stderr(), "kette: Ausgabe nicht geschrieben");
        return ExitCode::from(1);
    }
    ExitCode::from(ausgang.exit)
}
