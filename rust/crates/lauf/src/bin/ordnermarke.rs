//! `werkbank-ordnermarke zeigen|loesen|senken`: shows, releases or lowers a folder mark (contract 1). See `lauf::befehl`.

#[cfg(unix)]
use std::io::Write;

#[cfg(unix)]
fn main() -> std::process::ExitCode {
    let args: Vec<std::ffi::OsString> = std::env::args_os().skip(1).collect();
    match lauf::befehl::befehl(&args) {
        Ok(text) => {
            // a closed standard output must not change the exit code of a change that is made
            let _ = std::io::stdout().write_all(text.as_bytes());
            std::process::ExitCode::SUCCESS
        }
        Err(fehler) => {
            let _ = std::io::stderr().write_all(fehler.zeile().as_bytes());
            std::process::ExitCode::from(fehler.exitcode())
        }
    }
}

/// Not available: the program runs on Unix systems only (the crate is empty elsewhere).
#[cfg(not(unix))]
fn main() -> std::process::ExitCode {
    use std::io::Write;
    let _ = std::io::stderr()
        .write_all(b"werkbank-ordnermarke: nicht verfuegbar: only on Unix systems\n");
    std::process::ExitCode::from(2)
}
