//! The program `modellweg` (contract 8, section 4). All work is in the library; this file only hands over argv and the two
//! standard streams.

#[cfg(unix)]
use std::process::ExitCode;

#[cfg(unix)]
fn main() -> ExitCode {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    ExitCode::from(modellweg::start::haupt(
        &args,
        &mut std::io::stdout(),
        &mut std::io::stderr(),
    ))
}

/// Not available: the program runs on Unix systems only (the crate is empty elsewhere).
#[cfg(not(unix))]
fn main() -> std::process::ExitCode {
    use std::io::Write;
    let _ = std::io::stderr().write_all(b"modellweg: nicht verfuegbar: only on Unix systems\n");
    std::process::ExitCode::from(2)
}
