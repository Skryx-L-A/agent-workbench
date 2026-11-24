//! The program `zk`: reads one case (a file, `-` or standard input), writes the effects as one JSON line.
//!
//! Exit code 0: the effects are on stdout. Exit code 2: the case is invalid (not JSON, not an object, more than
//! 4 194 304 bytes, nested deeper than 128 levels, `ereignisse` or a `t` missing or falling, a wrong type in
//! the start, an unreadable file, more than one argument); stdout stays empty and one line goes to stderr.

use std::fs::File;
use std::io::{self, Read, Write};
use std::process::ExitCode;

use zustandskern::{EINGABE_MAX, Ungueltig};

/// Reads at most one byte more than the limit, so that a longer input is seen without reading all of it.
fn lesen(quelle: impl Read) -> io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    quelle
        .take(EINGABE_MAX as u64 + 1)
        .read_to_end(&mut bytes)?;
    Ok(bytes)
}

/// The input as the arguments name it: a file, `-` or nothing (standard input).
fn eingabe() -> Result<Vec<u8>, Ungueltig> {
    let mut args = std::env::args_os().skip(1);
    let (erstes, zweites) = (args.next(), args.next());
    if zweites.is_some() {
        return Err("Aufruf: zk [fall.json | -]".into());
    }
    let gelesen = match erstes {
        Some(pfad) if pfad != "-" => File::open(pfad).and_then(lesen),
        _ => lesen(io::stdin().lock()),
    };
    gelesen.map_err(|e| Ungueltig(format!("Eingabe nicht lesbar: {e}")))
}

fn main() -> ExitCode {
    let ergebnis = eingabe().and_then(|bytes| zustandskern::laufen(&bytes));
    match ergebnis {
        Ok(aus) => {
            let mut stdout = io::stdout().lock();
            let text = aus.to_string();
            if writeln!(stdout, "{text}")
                .and_then(|()| stdout.flush())
                .is_err()
            {
                let _ = writeln!(io::stderr(), "zk: Ausgabe nicht geschrieben");
                return ExitCode::from(1);
            }
            ExitCode::SUCCESS
        }
        Err(fehler) => {
            let _ = writeln!(io::stderr(), "zk: {fehler}");
            ExitCode::from(2)
        }
    }
}
