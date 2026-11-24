//! The program `tr`: reads one case (a file, `-` or standard input) and writes the result as one JSON line; or, as
//! `tr --kind <name> [arguments]`, runs one fixed child program (the cases of the child part start it in a PTY).
//!
//! Exit code 0: one JSON object on stdout. Exit code 2: the case is invalid (not JSON, not an object, more than
//! 4 194 304 bytes, nested deeper than 128 levels, a wrong kind, size, step or hex); stdout stays empty and one line goes
//! to stderr.

use std::fs::File;
use std::io::{self, Read, Write};
use std::process::ExitCode;

use traeger::{EINGABE_MAX, Ungueltig};

/// Reads at most one byte more than the limit, so that a longer input is seen without reading all of it.
fn lesen(quelle: impl Read) -> io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    quelle
        .take(EINGABE_MAX as u64 + 1)
        .read_to_end(&mut bytes)?;
    Ok(bytes)
}

/// The input as the arguments name it: a file, `-` or nothing (standard input).
fn eingabe(args: &[String]) -> Result<Vec<u8>, Ungueltig> {
    let gelesen = match args {
        [] => lesen(io::stdin().lock()),
        [pfad] if pfad == "-" => lesen(io::stdin().lock()),
        [pfad] => File::open(pfad).and_then(lesen),
        _ => {
            return Err(Ungueltig(
                "Aufruf: tr [fall.json | - | --kind <name> ...]".to_owned(),
            ));
        }
    };
    gelesen.map_err(|e| Ungueltig(format!("Eingabe nicht lesbar: {e}")))
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().is_some_and(|erstes| erstes == "--kind") {
        let code = traeger::kinderlauf(&args[1..]);
        return ExitCode::from(u8::try_from(code & 0xff).unwrap_or(1));
    }
    let ergebnis = std::env::current_exe()
        .map_err(|e| Ungueltig(format!("eigener Pfad unbekannt: {e}")))
        .and_then(|exe| eingabe(&args).and_then(|bytes| traeger::laufen(&bytes, &exe)));
    match ergebnis {
        Ok(aus) => {
            let mut stdout = io::stdout().lock();
            if writeln!(stdout, "{aus}")
                .and_then(|()| stdout.flush())
                .is_err()
            {
                let _ = writeln!(io::stderr(), "tr: Ausgabe nicht geschrieben");
                return ExitCode::from(1);
            }
            ExitCode::SUCCESS
        }
        Err(fehler) => {
            let _ = writeln!(io::stderr(), "tr: ungueltiger Fall: {fehler}");
            ExitCode::from(2)
        }
    }
}
