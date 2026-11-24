//! The program `rangkern`: see the library documentation.

use std::io::{stderr, stdin, stdout};
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Result<Vec<String>, _> = std::env::args_os()
        .skip(1)
        .map(std::ffi::OsString::into_string)
        .collect();
    let code = match args {
        Ok(args) => {
            // The first act of `serve` and `sitzung`: before the other arguments are read, before a thread and before a socket.
            let abschirmung = args
                .first()
                .filter(|befehl| rangkern::abschirmung::betrifft(befehl))
                .and_then(|befehl| rangkern::abschirmung::schritt(befehl, &mut stderr().lock()));
            match abschirmung {
                Some(code) => code,
                None => rangkern::run_with_input(
                    &args,
                    &mut stdin().lock(),
                    &mut stdout().lock(),
                    &mut stderr().lock(),
                ),
            }
        }
        Err(_) => rangkern::cli::EXIT_USAGE,
    };
    ExitCode::from(code)
}
