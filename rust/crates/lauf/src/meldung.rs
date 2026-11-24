//! The report to the core service (contract 1, "Meldung an den Dienst"): with `--meldung <pfad>` the program connects to
//! a Unix stream socket the service made in its session folder. A start that fails before the starter runs writes
//! exactly one line `<code> <grund>\n` there; a start that reaches the starter writes nothing, and the end of the
//! connection without data tells the service that the starter runs.
//!
//! The connection carries close-on-exec while the program starts the watcher, the search process and the model path, so
//! none of them inherits it. Right before the trampoline starts, the program lays it with `dup2` over descriptor 4,
//! reserved at the start ([`Meldung::reservieren`]); `dup2` clears close-on-exec on 4 only. The trampoline sets
//! close-on-exec on 4 before its `exec` into the starter, so the starter never holds it, and writes
//! `69 lauf.harness_fehlt` there when that `exec` fails. The program closes its own copies right after the start of the
//! trampoline. Between the `dup2` and that close it starts no other child (no other thread runs at that time).

use std::io::Write;
use std::os::fd::{AsRawFd, OwnedFd};
use std::os::unix::net::UnixStream;
use std::path::Path;

use crate::fehler::Fehler;

/// The descriptor of the report in the trampoline.
pub const MELDUNG_FD: i32 = 4;

/// The longest line of the report in bytes, with its line end (form agreed with the service).
pub const ZEILE_MAX: usize = 128;

/// The longest key of a line.
pub const GRUND_MAX: usize = 64;

/// The connection to the service and the reserved descriptor 4; both empty without `--meldung`.
#[derive(Debug, Default)]
pub struct Meldung {
    verbindung: Option<UnixStream>,
    vier: Option<OwnedFd>,
}

impl Meldung {
    /// No report (`--meldung` is missing): the program behaves as before.
    #[must_use]
    pub fn keine() -> Meldung {
        Meldung::default()
    }

    /// Opens `/dev/null` (close-on-exec) and keeps it only when it got the number 4: descriptor 4 stays free for the
    /// hand-over. Called after the reservation of descriptor 3 and before the first other file.
    #[must_use]
    pub fn reservieren() -> Option<OwnedFd> {
        rustix::fs::open(
            "/dev/null",
            rustix::fs::OFlags::RDONLY | rustix::fs::OFlags::CLOEXEC,
            rustix::fs::Mode::empty(),
        )
        .ok()
        .filter(|f| f.as_raw_fd() == MELDUNG_FD)
    }

    /// Connects to the socket of the service (close-on-exec, as every socket of `std`).
    ///
    /// # Errors
    /// `lauf.meldung_fehlt` when the connection fails, also for a path too long for `sun_path`. The caller then writes
    /// only the line on standard error.
    pub fn verbinden(pfad: &Path, vier: Option<OwnedFd>) -> Result<Meldung, Fehler> {
        let verbindung = UnixStream::connect(pfad).map_err(|_| Fehler::MeldungFehlt)?;
        Ok(Meldung {
            verbindung: Some(verbindung),
            vier,
        })
    }

    /// Whether the program reports (it holds a connection).
    #[must_use]
    pub fn aktiv(&self) -> bool {
        self.verbindung.is_some()
    }

    /// Whether descriptor 4 is held for the hand-over.
    #[must_use]
    pub fn vier_da(&self) -> bool {
        self.vier.is_some()
    }

    /// The descriptors the report holds in this process (the connection, the reserved 4): step 6 does not count them
    /// as inherited.
    #[must_use]
    pub fn eigene(&self) -> Vec<i32> {
        self.verbindung
            .iter()
            .map(AsRawFd::as_raw_fd)
            .chain(self.vier.iter().map(AsRawFd::as_raw_fd))
            .collect()
    }

    /// Lays the connection over descriptor 4 without close-on-exec, right before the start of the trampoline.
    /// Returns `false` without a report.
    ///
    /// # Errors
    /// `lauf.meldung_fehlt` when descriptor 4 is not held or still carries close-on-exec afterwards.
    pub fn auf_vier_legen(&mut self) -> Result<bool, Fehler> {
        let Some(v) = &self.verbindung else {
            return Ok(false);
        };
        let ziel = self.vier.as_mut().ok_or(Fehler::MeldungFehlt)?;
        rustix::io::dup2(v, ziel).map_err(|_| Fehler::MeldungFehlt)?;
        if ziel.as_raw_fd() != MELDUNG_FD {
            return Err(Fehler::MeldungFehlt);
        }
        let flags = rustix::io::fcntl_getfd(&*ziel).map_err(|_| Fehler::MeldungFehlt)?;
        if flags.contains(rustix::io::FdFlags::CLOEXEC) {
            return Err(Fehler::MeldungFehlt);
        }
        Ok(true)
    }

    /// Closes descriptor 4 (the copy of the hand-over); the connection stays for a line.
    pub fn vier_schliessen(&mut self) {
        drop(self.vier.take());
    }

    /// Closes both copies: after the start of the trampoline only it holds the connection.
    pub fn schliessen(&mut self) {
        drop(self.vier.take());
        drop(self.verbindung.take());
    }

    /// Writes the line of `f` once and closes the connection. Best effort: a service that is gone changes nothing
    /// about the code of the program.
    pub fn melden(&mut self, f: &Fehler) {
        drop(self.vier.take());
        if let Some(mut v) = self.verbindung.take() {
            let _ = v.write_all(zeile(f).as_bytes());
        }
    }
}

/// The line of the report: `<code> <grund>\n`, the exit code and the key of the fault (never the reason in words).
#[must_use]
pub fn zeile(f: &Fehler) -> String {
    format!("{} {}\n", f.exitcode(), f.code())
}

/// Whether a key has the form of the report: 1 to [`GRUND_MAX`] characters of `[a-z0-9_.]`.
#[must_use]
pub fn grund_gut(grund: &str) -> bool {
    (1..=GRUND_MAX).contains(&grund.len())
        && grund
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'.')
}

/// Whether `daten` (everything the service read) is exactly one line of the agreed form: at most [`ZEILE_MAX`] bytes,
/// `<code> <grund>\n` with a code from 1 to 255 (decimal, no leading zero) and a key of [`grund_gut`], nothing after it.
/// The service checks the same; everything else is `lauf.meldung_ungueltig` there.
#[must_use]
pub fn zeile_gut(daten: &[u8]) -> bool {
    if daten.len() > ZEILE_MAX {
        return false;
    }
    let Some(text) = daten.strip_suffix(b"\n") else {
        return false;
    };
    let Ok(text) = std::str::from_utf8(text) else {
        return false;
    };
    let Some((code, grund)) = text.split_once(' ') else {
        return false;
    };
    let code_gut = !code.is_empty()
        && code.bytes().all(|b| b.is_ascii_digit())
        && !code.starts_with('0')
        && code.parse::<u16>().is_ok_and(|n| (1..=255).contains(&n));
    code_gut && grund_gut(grund)
}

#[cfg(test)]
mod tests;
