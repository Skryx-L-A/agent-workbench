//! `werkbank-vorgabe`: the process shell of the write helper (rust/spec/paket/README.md, part A3, "Die Einheiten").
//!
//! Exit codes: 0 after SIGTERM or SIGINT, 64 for a call with arguments, 78 without an inherited socket, with more than
//! one, or when the process does not run as the system user `werkbank-vorgabe`. A refused request is an answer with a
//! code, never an end of the process. The program has no seam: every place is fixed.

use std::io::Write;
use std::process::ExitCode;

fn zeile(text: &str) {
    let mut e = std::io::stderr();
    let _ = writeln!(e, "{text}");
    let _ = e.flush();
}

// One `main` with the choice inside: cargo-mutants reads the source as text, so a second `main` for another platform
// would give mutants that no test on the machine of the gate can catch.
fn main() -> ExitCode {
    #[cfg(target_os = "linux")]
    let code = linux::lauf();
    // the helper exists for Linux only (regelebenen OPEN 26)
    #[cfg(not(target_os = "linux"))]
    let code = {
        zeile("werkbank-vorgabe: nur unter Linux, Code 78");
        ExitCode::from(vorgabe_helfer::code::EXIT_KONFIG)
    };
    code
}

#[cfg(target_os = "linux")]
mod linux {
    use std::process::ExitCode;
    use std::sync::Arc;
    use std::sync::atomic::AtomicBool;

    use vorgabe_helfer::bedienen::bedienen;
    use vorgabe_helfer::code::{EXIT_AUFRUF, EXIT_ENDE, EXIT_KONFIG};
    use vorgabe_helfer::helfer::Orte;
    use vorgabe_helfer::konten::System;
    use vorgabe_helfer::start;
    use vorgabe_helfer::sys::Echt;

    use super::zeile;

    fn ende(text: &str, code: u8) -> ExitCode {
        zeile(text);
        ExitCode::from(code)
    }

    pub fn lauf() -> ExitCode {
        if std::env::args_os().len() != 1 {
            return ende(
                "werkbank-vorgabe: Aufruf ohne Argumente, Code 64",
                EXIT_AUFRUF,
            );
        }
        rustix::process::umask(rustix::fs::Mode::from_raw_mode(0o077));
        let mut fds = listenfd::ListenFd::from_env();
        let Some(hoerer) = start::socket_waehlen(fds.len(), || fds.take_unix_listener(0)) else {
            return ende(
                "werkbank-vorgabe: nicht genau ein Socket von systemd, Code 78",
                EXIT_KONFIG,
            );
        };
        let Some(helfer_uid) = start::helfer_uid(&System, rustix::process::geteuid().as_raw())
        else {
            return ende(
                "werkbank-vorgabe: läuft nicht als werkbank-vorgabe, Code 78",
                EXIT_KONFIG,
            );
        };
        let halt = Arc::new(AtomicBool::new(false));
        for sig in [signal_hook::consts::SIGTERM, signal_hook::consts::SIGINT] {
            if signal_hook::flag::register(sig, Arc::clone(&halt)).is_err() {
                return ende(
                    "werkbank-vorgabe: Signale nicht übernommen, Code 78",
                    EXIT_KONFIG,
                );
            }
        }
        if hoerer.set_nonblocking(true).is_err() {
            return ende(
                "werkbank-vorgabe: Socket nicht bereit, Code 78",
                EXIT_KONFIG,
            );
        }
        let orte = Orte::fest(helfer_uid);
        start::schleife(&hoerer, &halt, start::WARTEN, |strom| {
            let _ = strom.set_nonblocking(false);
            zeile(&bedienen(strom, &orte, &Echt, &System).zeile());
        });
        ExitCode::from(EXIT_ENDE)
    }
}
