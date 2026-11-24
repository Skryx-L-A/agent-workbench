//! The trampoline `werkbank-lauf-kind <ppid> -- <program> [argument ...]` (see `lauf::kind`).

#[cfg(unix)]
fn main() -> std::process::ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    std::process::ExitCode::from(u8::try_from(lauf::kind::trampolin(&args)).unwrap_or(1))
}

/// Not available: the program runs on Unix systems only (the crate is empty elsewhere).
#[cfg(not(unix))]
fn main() -> std::process::ExitCode {
    use std::io::Write;
    let _ = std::io::stderr()
        .write_all(b"werkbank-lauf-kind: nicht verfuegbar: only on Unix systems\n");
    std::process::ExitCode::from(2)
}
