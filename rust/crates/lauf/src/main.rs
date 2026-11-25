//! `werkbank-lauf <startdatei>`: the program "Lauf starten" (contract 1). See the crate `lauf`.

/// One `main` with a `cfg` on its body: cargo-mutants reads the text and would keep the mutants of a second `main` that
/// the gate machine never builds. Outside Unix the program is not available (the crate is empty there).
fn main() -> std::process::ExitCode {
    use std::io::Write;
    #[cfg(not(unix))]
    let code = {
        let _ =
            std::io::stderr().write_all(b"werkbank-lauf: nicht verfuegbar: only on Unix systems\n");
        std::process::ExitCode::from(2)
    };
    #[cfg(unix)]
    let code = {
        let args: Vec<std::ffi::OsString> = std::env::args_os().skip(1).collect();
        let fehler = lauf::ablauf::lauf(&args, &lauf::ablauf::Umgebung::eigene());
        // a closed standard error must not change the exit code
        let _ = std::io::stderr().write_all(fehler.zeile().as_bytes());
        std::process::ExitCode::from(fehler.exitcode())
    };
    code
}
