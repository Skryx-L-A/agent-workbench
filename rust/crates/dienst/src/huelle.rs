//! The cover of the child (`wb-dienst --exec <mit-shell|ohne-shell> <program> [arguments]`), SECURITY point of DI8.
//!
//! The carrier (`traeger::Kind`, built on `portable-pty`) starts a child with the variables of the start file and one more:
//! `portable-pty` always sets `SHELL`. The contract says the environment of the child is exactly `kind.env`, nothing else. So the
//! service starts the cover instead of the program; the cover clears the environment, puts back everything but `SHELL` (and
//! `SHELL` too, if the start file names it) and replaces itself by the program (`exec`, so the process id, the session and the
//! terminal stay the same). The cover has no `unsafe`: `CommandExt::exec` is a safe call.

use std::io::Write;
use std::os::unix::process::CommandExt;
use std::process::Command;

/// `args` are the flag, the program and its arguments. Returns only if the program cannot be started (exit code 127).
#[must_use]
pub fn ausfuehren(args: &[String]) -> i32 {
    let [flag, programm, rest @ ..] = args else {
        return 64;
    };
    let mit_shell = flag == "mit-shell";
    let mut befehl = Command::new(programm);
    befehl.args(rest).env_clear();
    for (name, wert) in std::env::vars_os() {
        if name == "SHELL" && !mit_shell {
            continue;
        }
        befehl.env(name, wert);
    }
    let fehler = befehl.exec();
    let _ = writeln!(std::io::stderr(), "wb-dienst: exec: {}", fehler.kind());
    127
}

#[cfg(test)]
mod tests {
    use super::ausfuehren;

    #[test]
    fn zu_wenig_argumente() {
        assert_eq!(ausfuehren(&[]), 64);
        assert_eq!(ausfuehren(&["ohne-shell".to_owned()]), 64);
    }

    #[test]
    fn programm_fehlt() {
        assert_eq!(
            ausfuehren(&["ohne-shell".to_owned(), "/nirgends/da".to_owned()]),
            127
        );
    }
}
