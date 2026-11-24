//! `werkbank-lauf <startdatei>`: the program "Lauf starten" (contract 1). See the crate `lauf`.

#[cfg(unix)]
use std::io::Write;

#[cfg(unix)]
fn main() -> std::process::ExitCode {
    let args: Vec<std::ffi::OsString> = std::env::args_os().skip(1).collect();
    let fehler = lauf::ablauf::lauf(&args, &lauf::ablauf::Umgebung::eigene());
    // a closed standard error must not change the exit code
    let _ = std::io::stderr().write_all(fehler.zeile().as_bytes());
    std::process::ExitCode::from(fehler.exitcode())
}

/// Not available: the program runs on Unix systems only (the crate is empty elsewhere).
#[cfg(not(unix))]
fn main() -> std::process::ExitCode {
    use std::io::Write;
    let _ = std::io::stderr().write_all(b"werkbank-lauf: nicht verfuegbar: only on Unix systems\n");
    std::process::ExitCode::from(2)
}
