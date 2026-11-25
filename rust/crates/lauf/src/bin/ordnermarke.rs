//! `werkbank-ordnermarke zeigen|loesen|senken`: shows, releases or lowers a folder mark (contract 1). See `lauf::befehl`.

/// One `main` with a `cfg` on its body: cargo-mutants reads the text and would keep the mutants of a second `main` that
/// the gate machine never builds. Outside Unix the program is not available (the crate is empty there).
fn main() -> std::process::ExitCode {
    use std::io::Write;
    #[cfg(not(unix))]
    let code = {
        let _ = std::io::stderr()
            .write_all(b"werkbank-ordnermarke: nicht verfuegbar: only on Unix systems\n");
        std::process::ExitCode::from(2)
    };
    #[cfg(unix)]
    let code = {
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
    };
    code
}
