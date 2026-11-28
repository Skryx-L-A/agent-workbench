//! `werkbank-halter`: the process shell of the holder (section 2.1, start steps 0 to 4, then the accept loop).
//!
//! Seams for tests (`WB_HALTER_KONFIG`, `WB_HALTER_KONFIG_EIGNER`, `WB_HALTER_PASSWD`, `WB_HALTER_GROUP`,
//! `WB_HALTER_NSSWITCH`, `WB_HALTER_LAUFZEIT`) exist only in the test build (feature `pruefbau`) and count there
//! only with `WB_TESTUMGEBUNG=1`. The delivered build never reads them; its paths are fixed.

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
    // version 1 is built for Linux step I only; macOS (keychain) and Windows (service, pipe) follow (OPEN)
    #[cfg(not(target_os = "linux"))]
    let code = {
        let f = halter::code::Startfehler::Intern;
        zeile(&f.zeile());
        ExitCode::from(f.teile().0)
    };
    code
}

#[cfg(target_os = "linux")]
mod linux {
    use std::process::ExitCode;
    use std::sync::Arc;
    use std::sync::atomic::AtomicBool;

    use halter::code::Startfehler;
    use halter::dienst::{self, Dienst};
    use halter::start::{self, Orte};

    use super::zeile;

    fn ende(f: Startfehler) -> ExitCode {
        zeile(&f.zeile());
        ExitCode::from(f.teile().0)
    }

    // One `orte` with the choice inside, as in `main`: a function behind `cfg` would give mutants that the run without
    // the feature cannot catch. The test build reads the seams through `Orte::aus_naht`; the delivered build never
    // reads them.
    fn orte() -> Orte {
        #[cfg(feature = "pruefbau")]
        let o = Orte::aus_naht(|n: &str| std::env::var_os(n));
        #[cfg(not(feature = "pruefbau"))]
        let o = Orte::fest();
        o
    }

    pub fn lauf() -> ExitCode {
        // step 0, before any read
        match kernabbild::abschirmen() {
            Ok(a) => {
                if let Some(h) = a.hinweis {
                    zeile(h.text());
                }
            }
            Err(_) => return ende(Startfehler::Kernabbild),
        }
        // step 1: no argument besides the name of the program
        if std::env::args_os().len() != 1 {
            return ende(Startfehler::Aufruf);
        }
        let uid = rustix::process::getuid().as_raw();
        let orte = orte();
        // files and folders of the holder are its own only
        rustix::process::umask(rustix::fs::Mode::from_raw_mode(0o077));
        // SIGTERM and SIGINT are taken over before the socket exists: a signal the moment the socket is there ends the
        // holder through the loop below, which removes the socket, never by the signal with the socket left
        let halt = Arc::new(AtomicBool::new(false));
        for sig in [signal_hook::consts::SIGTERM, signal_hook::consts::SIGINT] {
            if signal_hook::flag::register(sig, Arc::clone(&halt)).is_err() {
                return ende(Startfehler::Intern);
            }
        }
        let bereit = match start::starten(&orte, uid) {
            Ok(b) => b,
            Err(f) => {
                // decision of the operator, 28.11.2026: a level writable for a group that is not the private group
                // gets the remedy with its paths in the same line
                if f == Startfehler::AblageRechte
                    && let Some(abhilfe) = start::ablage_abhilfe(&orte, uid)
                {
                    zeile(&format!("{}; Abhilfe: {abhilfe}", f.zeile()));
                    return ExitCode::from(f.teile().0);
                }
                return ende(f);
            }
        };
        if bereit.hoerer.set_nonblocking(true).is_err() {
            start::socket_entfernen(&bereit.ordner, bereit.socket_id);
            return ende(Startfehler::Intern);
        }
        let d = Arc::new(Dienst::neu(
            bereit.konfig,
            bereit.ablage,
            uid,
            dienst::jetzt_ms(),
            bereit.rollen,
            bereit.kette,
            dienst::jetzt_ms,
        ));
        dienst::annehmen(&d, &bereit.hoerer, &halt, dienst::VERBINDUNGEN_MAX);
        start::socket_entfernen(&bereit.ordner, bereit.socket_id);
        ExitCode::SUCCESS
    }
}
