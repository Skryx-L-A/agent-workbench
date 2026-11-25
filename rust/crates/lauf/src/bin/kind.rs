//! The trampoline `werkbank-lauf-kind <ppid> -- <program> [argument ...]` (see `lauf::kind`).

/// One `main` with a `cfg` on its body: cargo-mutants reads the text and would keep the mutants of a second `main` that
/// the gate machine never builds. Outside Unix the program is not available (the crate is empty there).
fn main() -> std::process::ExitCode {
    #[cfg(not(unix))]
    let code = {
        use std::io::Write;
        let _ = std::io::stderr()
            .write_all(b"werkbank-lauf-kind: nicht verfuegbar: only on Unix systems\n");
        std::process::ExitCode::from(2)
    };
    #[cfg(unix)]
    let code = {
        let args: Vec<String> = std::env::args().skip(1).collect();
        std::process::ExitCode::from(u8::try_from(lauf::kind::trampolin(&args)).unwrap_or(1))
    };
    code
}
