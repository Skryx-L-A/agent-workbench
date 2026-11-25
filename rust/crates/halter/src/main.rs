//! `werkbank-halter`: the process shell of the holder (section 2.1, start steps 0 to 4, then the accept loop).
//!
//! Seams for tests (`WB_HALTER_KONFIG`, `WB_HALTER_KONFIG_EIGNER`, `WB_HALTER_PASSWD`, `WB_HALTER_GROUP`,
//! `WB_HALTER_LAUFZEIT`) exist only in the test build (feature `pruefbau`) and count there
//! only with `WB_TESTUMGEBUNG=1`. The delivered build never reads them; its paths are fixed.

use std::io::Write;
use std::process::ExitCode;

fn zeile(text: &str) {
    let mut e = std::io::stderr();
    let _ = writeln!(e, "{text}");
    let _ = e.flush();
}

#[cfg(target_os = "linux")]
fn main() -> ExitCode {
    linux::lauf()
}

#[cfg(not(target_os = "linux"))]
fn main() -> ExitCode {
    // version 1 is built for Linux step I only; macOS (keychain) and Windows (service, pipe) follow (OPEN)
    let f = halter::code::Startfehler::Intern;
    zeile(&f.zeile());
    ExitCode::from(f.teile().0)
}

#[cfg(target_os = "linux")]
mod linux {
    use std::process::ExitCode;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::time::Duration;

    use halter::code::Startfehler;
    use halter::dienst::{self, Dienst};
    use halter::start::{self, Orte};

    use super::zeile;

    /// Most connections at once (at least 32 of the role `bruecke`, section 3, besides hub and surface).
    const VERBINDUNGEN_MAX: usize = 128;

    fn ende(f: Startfehler) -> ExitCode {
        zeile(&f.zeile());
        ExitCode::from(f.teile().0)
    }

    #[cfg(feature = "pruefbau")]
    fn orte() -> Orte {
        use std::path::PathBuf;
        let mut o = Orte::fest();
        if std::env::var_os("WB_TESTUMGEBUNG").as_deref() != Some(std::ffi::OsStr::new("1")) {
            return o;
        }
        let pfad = |n: &str| std::env::var_os(n).map(PathBuf::from);
        let zahl = |n: &str| std::env::var(n).ok().and_then(|v| v.parse::<u32>().ok());
        if let Some(p) = pfad("WB_HALTER_KONFIG") {
            o.konfig = p;
        }
        if let Some(u) = zahl("WB_HALTER_KONFIG_EIGNER") {
            o.konfig_eigner = u;
        }
        if let Some(p) = pfad("WB_HALTER_PASSWD") {
            o.passwd = p;
        }
        if let Some(p) = pfad("WB_HALTER_GROUP") {
            o.gruppen = p;
        }
        o.laufzeit = pfad("WB_HALTER_LAUFZEIT");
        o
    }

    #[cfg(not(feature = "pruefbau"))]
    fn orte() -> Orte {
        Orte::fest()
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
            Err(f) => return ende(f),
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
        let offen = Arc::new(AtomicUsize::new(0));
        while !halt.load(Ordering::Relaxed) {
            match bereit.hoerer.accept() {
                Ok((strom, _)) => {
                    if strom.set_nonblocking(false).is_err()
                        || offen.load(Ordering::Relaxed) >= VERBINDUNGEN_MAX
                    {
                        continue; // closed unanswered
                    }
                    offen.fetch_add(1, Ordering::Relaxed);
                    let d = Arc::clone(&d);
                    let offen2 = Arc::clone(&offen);
                    let gestartet = std::thread::Builder::new().spawn(move || {
                        d.verbindung(strom);
                        offen2.fetch_sub(1, Ordering::Relaxed);
                    });
                    if gestartet.is_err() {
                        offen.fetch_sub(1, Ordering::Relaxed);
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    let mut fds = [rustix::event::PollFd::new(
                        &bereit.hoerer,
                        rustix::event::PollFlags::IN,
                    )];
                    let t = rustix::time::Timespec {
                        tv_sec: 0,
                        tv_nsec: 200_000_000,
                    };
                    let _ = rustix::event::poll(&mut fds, Some(&t));
                }
                Err(_) => std::thread::sleep(Duration::from_millis(50)),
            }
        }
        start::socket_entfernen(&bereit.ordner, bereit.socket_id);
        ExitCode::SUCCESS
    }
}
