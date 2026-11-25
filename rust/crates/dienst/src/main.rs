//! The program `wb-dienst`, the core service and the check program (see `lib.rs`): `wb-dienst <start.json>` runs the service until SIGTERM or SIGINT;
//! `wb-dienst --kind <name> ...` runs one fixed child program of the tests.

#[cfg(any(unix, windows))]
use std::process::ExitCode;

#[cfg(any(unix, windows))]
fn main() -> ExitCode {
    let args: Vec<std::ffi::OsString> = std::env::args_os().skip(1).collect();
    ExitCode::from(u8::try_from(dienst::programm(&args)).unwrap_or(1))
}

/// Not available: the program runs on Unix systems and Windows only (the crate is empty elsewhere).
#[cfg(not(any(unix, windows)))]
fn main() -> std::process::ExitCode {
    use std::io::Write;
    let _ = std::io::stderr().write_all(b"wb-dienst: nicht verfuegbar: only on Unix systems\n");
    std::process::ExitCode::from(2)
}
