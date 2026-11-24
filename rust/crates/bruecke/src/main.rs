//! The program `wb-bruecke <startdatei.json>`; see `programm.rs` in the library.

#[cfg(unix)]
use std::process::ExitCode;
#[cfg(unix)]
use std::sync::Arc;

#[cfg(unix)]
use bruecke::Halt;

#[cfg(unix)]
fn main() -> ExitCode {
    let args: Vec<String> = std::env::args_os()
        .skip(1)
        .map(|a| a.to_string_lossy().into_owned())
        .collect();
    // the first step: the memory of the process is shielded from core images (exit code 5 when it cannot be)
    if let Some(code) = bruecke::abschirmen(kernabbild::abschirmen(), &mut std::io::stderr()) {
        return ExitCode::from(code);
    }
    let halt = Arc::new(Halt::neu());
    if bruecke::signale_anmelden(&halt).is_err() {
        let _ = std::io::Write::write_all(
            &mut std::io::stderr(),
            b"wb-bruecke: Signale lassen sich nicht anmelden\n",
        );
        return ExitCode::from(1);
    }
    let code = bruecke::lauf(
        &args,
        &mut std::io::stdout(),
        &mut std::io::stderr(),
        &halt,
        bruecke::protokoll_stderr(),
    );
    ExitCode::from(code)
}

/// Not available: the program runs on Unix systems only (the crate is empty elsewhere).
#[cfg(not(unix))]
fn main() -> std::process::ExitCode {
    use std::io::Write;
    let _ = std::io::stderr().write_all(b"wb-bruecke: nicht verfuegbar: only on Unix systems\n");
    std::process::ExitCode::from(2)
}
