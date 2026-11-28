//! The report to the core service (contract 1, "Meldung an den Dienst"): with `--meldung <pfad>` the program connects to
//! a Unix stream socket the service made in its session folder. A start that fails before the starter runs writes
//! exactly one line `<code> <grund>\n` there; a start that reaches the starter writes nothing, and the end of the
//! connection without data tells the service that the starter runs.
//!
//! The connection carries close-on-exec while the program starts the watcher, the search process and the model path, so
//! none of them inherits it. Right before the hand-over the program gives it to the holding process
//! ([`Meldung::halter_starten`], [`crate::anfordern`]) and keeps the write end of a pipe to it instead: from then on
//! every line goes through the pipe, and the holding process passes it on. Right before the trampoline starts, the
//! program lays that write end with `dup2` over descriptor 4, reserved at the start ([`Meldung::reservieren`]); `dup2`
//! clears close-on-exec on 4 only. The trampoline sets close-on-exec on 4 before its `exec` into the starter, so the
//! starter never holds it, and writes `69 lauf.harness_fehlt` there when that `exec` fails. The program closes its own
//! copies right after the start of the trampoline. Between the `dup2` and that close it starts no other child (no other
//! thread runs at that time).

use std::io::Write;
use std::os::fd::{AsFd, AsRawFd, OwnedFd};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::process::{Command, Stdio};

use crate::fehler::Fehler;

/// The descriptor of the report in the trampoline.
pub const MELDUNG_FD: i32 = 4;

/// The longest line of the report in bytes, with its line end (form agreed with the service).
pub const ZEILE_MAX: usize = 128;

/// The longest key of a line.
pub const GRUND_MAX: usize = 64;

/// The connection to the service (until the holding process has it), the write end of the pipe to the holding process
/// (after), and the reserved descriptor 4; all empty without `--meldung` (`Meldung::default()`: no report, the program
/// behaves as before).
#[derive(Debug, Default)]
pub struct Meldung {
    verbindung: Option<UnixStream>,
    rohr: Option<std::fs::File>,
    vier: Option<OwnedFd>,
}

/// A new pipe whose two ends carry close-on-exec: atomically on Linux; elsewhere (macOS has no `pipe2`) set right after,
/// which is enough, as the holding process starts on Linux only (step 6 ends every start elsewhere before).
fn rohr_neu() -> rustix::io::Result<(OwnedFd, OwnedFd)> {
    #[cfg(target_os = "linux")]
    let rohr = rustix::pipe::pipe_with(rustix::pipe::PipeFlags::CLOEXEC);
    #[cfg(not(target_os = "linux"))]
    let rohr = rustix::pipe::pipe().and_then(|(lesen, schreiben)| {
        rustix::io::fcntl_setfd(&lesen, rustix::io::FdFlags::CLOEXEC)?;
        rustix::io::fcntl_setfd(&schreiben, rustix::io::FdFlags::CLOEXEC)?;
        Ok((lesen, schreiben))
    });
    rohr
}

impl Meldung {
    /// Opens `/dev/null` (close-on-exec) and keeps it only when it got the number 4: descriptor 4 stays free for the
    /// hand-over. Called after the reservation of descriptor 3 and before the first other file.
    #[must_use]
    pub fn reservieren() -> Option<OwnedFd> {
        rustix::fs::open(
            "/dev/null",
            rustix::fs::OFlags::RDONLY.union(rustix::fs::OFlags::CLOEXEC),
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
            rohr: None,
            vier,
        })
    }

    /// Whether the program reports (it holds a connection, or the pipe to the holding process).
    #[must_use]
    pub fn aktiv(&self) -> bool {
        self.verbindung.is_some() || self.rohr.is_some()
    }

    /// Starts the holding process (`<kind> --halteprozess <pid>`, [`crate::anfordern::halteprozess`]): the read end of a
    /// new pipe on its standard input, the connection to the service on its standard output, the listening socket
    /// `hoerer` of the worker request on its standard error, an empty environment, `/` as its folder; it inherits no
    /// other descriptor (every one of this process carries close-on-exec). This process keeps the write end of the pipe
    /// (close-on-exec) in place of the connection. Returns its PID; without a report `None` and nothing starts.
    ///
    /// # Errors
    /// `lauf.intern` when the pipe cannot be made or the holding process does not start; the connection stays here
    /// then, so the line still reaches the service.
    pub fn halter_starten(&mut self, kind: &Path, hoerer: OwnedFd) -> Result<Option<u32>, Fehler> {
        let Some(v) = &self.verbindung else {
            return Ok(None);
        };
        let (lesen, schreiben) = rohr_neu().map_err(|_| Fehler::Intern)?;
        let kopie = v.as_fd().try_clone_to_owned().map_err(|_| Fehler::Intern)?;
        let kind = Command::new(kind)
            .arg(crate::anfordern::HALTEPROZESS)
            .arg(std::process::id().to_string())
            .env_clear()
            .current_dir("/")
            .stdin(Stdio::from(lesen))
            .stdout(Stdio::from(kopie))
            .stderr(Stdio::from(hoerer))
            .spawn()
            .map_err(|_| Fehler::Intern)?;
        // from here on only the holding process holds the connection; every line goes through the pipe
        self.verbindung = None;
        self.rohr = Some(std::fs::File::from(schreiben));
        Ok(Some(kind.id()))
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
            .chain(self.rohr.iter().map(AsRawFd::as_raw_fd))
            .chain(self.vier.iter().map(AsRawFd::as_raw_fd))
            .collect()
    }

    /// Lays the pipe to the holding process over descriptor 4 without close-on-exec, right before the start of the
    /// trampoline. Returns `false` without a report.
    ///
    /// # Errors
    /// `lauf.meldung_fehlt` when descriptor 4 is not held or still carries close-on-exec afterwards, or when the
    /// holding process was not started (the connection itself never goes to the trampoline).
    pub fn auf_vier_legen(&mut self) -> Result<bool, Fehler> {
        if !self.aktiv() {
            return Ok(false);
        }
        let quelle = self.rohr.as_ref().ok_or(Fehler::MeldungFehlt)?;
        let ziel = self.vier.as_mut().ok_or(Fehler::MeldungFehlt)?;
        rustix::io::dup2(quelle, ziel).map_err(|_| Fehler::MeldungFehlt)?;
        if ziel.as_raw_fd() != MELDUNG_FD {
            return Err(Fehler::MeldungFehlt);
        }
        let flags = rustix::io::fcntl_getfd(&*ziel).map_err(|_| Fehler::MeldungFehlt)?;
        if flags.contains(rustix::io::FdFlags::CLOEXEC) {
            return Err(Fehler::MeldungFehlt);
        }
        Ok(true)
    }

    /// Closes descriptor 4 (the copy of the hand-over); the pipe stays for a line.
    pub fn vier_schliessen(&mut self) {
        drop(self.vier.take());
    }

    /// Closes every copy: after the start of the trampoline only it holds the pipe, until its `exec`.
    pub fn schliessen(&mut self) {
        drop(self.vier.take());
        drop(self.verbindung.take());
        drop(self.rohr.take());
    }

    /// Writes the line of `f` once and closes the connection or the pipe (the holding process passes the line on).
    /// Best effort: a service that is gone changes nothing about the code of the program.
    pub fn melden(&mut self, f: &Fehler) {
        drop(self.vier.take());
        if let Some(mut v) = self.verbindung.take() {
            let _ = v.write_all(zeile(f).as_bytes());
        }
        if let Some(mut r) = self.rohr.take() {
            let _ = r.write_all(zeile(f).as_bytes());
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
    // No check of the length of its own: the form below bounds a line at 3 + 1 + GRUND_MAX + 1 = 69 bytes, below
    // ZEILE_MAX, so a longer `daten` always fails the form (a check `> ZEILE_MAX` could never refuse on its own).
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
