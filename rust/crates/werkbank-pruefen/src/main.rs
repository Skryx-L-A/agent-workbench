//! The program `werkbank-pruefen`. It takes no argument (exit 64 otherwise) and runs the seven checks. Two exceptions: `--vor-entfernen`,
//! called by `prerm` on `apt remove`, only reports a key file that stays and never fails; `--programmordner <heim> <ordner>`, called by
//! `postinst`, applies the rule of the enclosure for program folders to one folder (exit 0 allowed, 1 refused).
//!
//! Test seam, only in the test build (feature `pruefbau`): with `WB_TESTUMGEBUNG=1` and `WB_PRUEFEN_LAGE=<file>` the program
//! reads a `Lage` from the file (the `lage` of a case of the contract) instead of the machine. Without `WB_TESTUMGEBUNG=1` the
//! variable is ignored. The delivered build has no seam and never reads either variable.
//!
//! The program reads the state of a Linux machine (`sammeln`, Unix only): on another system it says that it is not
//! available. The program itself is in `main/unix.rs`.

#[cfg(unix)]
#[path = "main/unix.rs"]
mod unix;

#[cfg(unix)]
fn main() -> std::process::ExitCode {
    unix::main()
}

/// Not available: the checks read a Linux machine.
#[cfg(not(unix))]
fn main() -> std::process::ExitCode {
    use std::io::Write;
    let _ =
        std::io::stderr().write_all(b"werkbank-pruefen: nicht verfuegbar: only on Unix systems\n");
    std::process::ExitCode::from(2)
}
