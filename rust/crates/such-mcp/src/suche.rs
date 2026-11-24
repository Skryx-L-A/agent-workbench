//! The connection to the search socket: one connection per call, one request line, one answer line, one deadline.

use std::path::{Path, PathBuf};
use std::time::Duration;

#[cfg(unix)]
use crate::mac::Serverpruefung;

/// The deadline for one call: connecting, writing the request and reading the answer line together.
pub const FRIST: Duration = Duration::from_secs(10);

/// The longest answer line accepted, without the LF. A longer answer counts as no answer.
pub const MAX_ANTWORT: usize = 8 * 1024 * 1024;

/// The socket gave no usable answer: not reachable, no answer in time, a closed connection before the LF, an answer
/// that is not UTF-8 or longer than the limit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Unerreichbar;

/// Asks the search of the session one request line and returns its answer line without the LF.
pub trait Befrager {
    /// Sends `zeile` (without LF) and returns the one answer line (without LF).
    ///
    /// # Errors
    /// [`Unerreichbar`] when no usable answer comes.
    fn frage(&self, zeile: &str) -> Result<String, Unerreichbar>;
}

/// The Unix socket of the session. Every question opens its own connection.
#[derive(Debug, Clone)]
pub struct UnixSocket {
    pfad: PathBuf,
    frist: Duration,
    max_antwort: usize,
    #[cfg(unix)]
    server: Serverpruefung,
}

impl UnixSocket {
    /// The socket at `pfad` with the deadline of 10 s and the answer limit of this module.
    #[must_use]
    pub fn neu(pfad: impl Into<PathBuf>) -> Self {
        Self::mit_grenzen(pfad, FRIST, MAX_ANTWORT)
    }

    /// The socket at `pfad` with a chosen deadline and answer limit (for tests with a short deadline).
    #[must_use]
    pub fn mit_grenzen(pfad: impl Into<PathBuf>, frist: Duration, max_antwort: usize) -> Self {
        Self {
            pfad: pfad.into(),
            frist,
            max_antwort,
            #[cfg(unix)]
            server: Serverpruefung::eigene(),
        }
    }

    #[cfg(unix)]
    /// The same socket with another check of the server (for tests that run the rules of macOS under Linux).
    #[must_use]
    pub fn mit_serverpruefung(mut self, server: Serverpruefung) -> Self {
        self.server = server;
        self
    }

    /// The socket path.
    #[must_use]
    pub fn pfad(&self) -> &Path {
        &self.pfad
    }
}

impl Befrager for UnixSocket {
    fn frage(&self, zeile: &str) -> Result<String, Unerreichbar> {
        // The whole exchange runs in its own thread so that even a connect that blocks (a full queue of the
        // socket) ends the call at the deadline. The thread ends by itself at its own deadline or when the
        // connect returns; the program never waits for it.
        let (sender, empfaenger) = std::sync::mpsc::channel();
        let pfad = self.pfad.clone();
        let zeile = zeile.to_owned();
        let (frist, max) = (self.frist, self.max_antwort);
        #[cfg(unix)]
        let server = self.server;
        std::thread::spawn(move || {
            #[cfg(unix)]
            let ergebnis = tausch::austausch(&pfad, &zeile, frist, max, &server);
            // Without Unix sockets (Windows) the search is not reachable until the named pipe of variant B comes.
            #[cfg(not(unix))]
            let ergebnis: Result<String, Unerreichbar> = {
                let _ = (&pfad, &zeile, frist, max);
                Err(Unerreichbar)
            };
            let _ = sender.send(ergebnis);
        });
        empfaenger
            .recv_timeout(self.frist)
            .map_err(|_| Unerreichbar)?
    }
}

#[cfg(unix)]
mod tausch {
    use super::Unerreichbar;
    use crate::mac::Serverpruefung;
    use std::io::{Read, Write};
    use std::os::unix::net::UnixStream;
    use std::path::Path;
    use std::time::{Duration, Instant};

    pub fn austausch(
        pfad: &Path,
        zeile: &str,
        frist: Duration,
        max: usize,
        server: &Serverpruefung,
    ) -> Result<String, Unerreichbar> {
        let ende = Instant::now() + frist;
        let mut strom = UnixStream::connect(pfad).map_err(|_| Unerreichbar)?;
        // After `connect` and before the first byte: a server that is not ours (macOS) gets nothing.
        if !server.erlaubt(&strom) {
            return Err(Unerreichbar);
        }
        // macOS refuses `setsockopt` (EINVAL) once the peer has closed its end, even when its answer still waits in
        // the buffer. So a failed timeout call is ignored: a closed peer cannot hold a read up, and the caller's
        // own deadline (`recv_timeout` in `frage`) covers the whole exchange in any case.
        let _ = strom.set_write_timeout(Some(rest(ende)?));
        let mut anfrage = Vec::with_capacity(zeile.len() + 1);
        anfrage.extend_from_slice(zeile.as_bytes());
        anfrage.push(b'\n');
        strom.write_all(&anfrage).map_err(|_| Unerreichbar)?;
        let mut antwort: Vec<u8> = Vec::new();
        let mut stueck = [0u8; 8192];
        loop {
            let _ = strom.set_read_timeout(Some(rest(ende)?));
            let n = match strom.read(&mut stueck) {
                Ok(0) | Err(_) => return Err(Unerreichbar),
                Ok(n) => n,
            };
            if let Some(lf) = stueck[..n].iter().position(|&b| b == b'\n') {
                antwort.extend_from_slice(&stueck[..lf]);
                break;
            }
            antwort.extend_from_slice(&stueck[..n]);
            if antwort.len() > max {
                return Err(Unerreichbar);
            }
        }
        if antwort.len() > max {
            return Err(Unerreichbar);
        }
        String::from_utf8(antwort).map_err(|_| Unerreichbar)
    }

    /// The time left until `ende`; none left is an error (a zero timeout would mean no timeout).
    fn rest(ende: Instant) -> Result<Duration, Unerreichbar> {
        ende.checked_duration_since(Instant::now())
            .filter(|d| !d.is_zero())
            .ok_or(Unerreichbar)
    }
}
