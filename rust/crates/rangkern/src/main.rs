//! The program `rangkern`: see the library documentation.

use std::ffi::OsString;
use std::io::{stderr, stdin, stdout};
use std::process::ExitCode;

fn main() -> ExitCode {
    let roh: Vec<OsString> = std::env::args_os().skip(1).collect();
    // The first act of `serve` and `sitzung`: before the other arguments are read (also before an argument that is no UTF-8 is
    // found), before a thread and before a socket.
    let abschirmung = roh
        .first()
        .and_then(|befehl| befehl.to_str())
        .filter(|befehl| rangkern::abschirmung::betrifft(befehl))
        .and_then(|befehl| rangkern::abschirmung::schritt(befehl, &mut stderr()));
    if let Some(code) = abschirmung {
        return ExitCode::from(code);
    }
    let args: Result<Vec<String>, _> = roh.into_iter().map(OsString::into_string).collect();
    let code = match args {
        // The error output goes without a lock (`cli::fehlerausgabe`). The standard output keeps its lock: only this thread
        // writes to it.
        Ok(args) => rangkern::run_with_input(
            &args,
            &mut stdin().lock(),
            &mut stdout().lock(),
            &mut rangkern::cli::fehlerausgabe(),
        ),
        // An argument that is no UTF-8 is a usage error like every other: exit 64.
        Err(_) => rangkern::cli::EXIT_USAGE,
    };
    ExitCode::from(code)
}
