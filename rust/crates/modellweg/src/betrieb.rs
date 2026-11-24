//! The common running part of every mode: places of connection threads, the 8 places of work, the tear-down at the end,
//! the blocked protocol after a failed append, and the clock of the lines.

use std::io::{self, Read, Write};
use std::os::unix::net::UnixStream;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex, PoisonError};
use std::time::{Duration, Instant};

/// At most this many requests (mode 1) or tunnels (mode 2) in work at once.
pub const PARALLEL_MAX: usize = 8;
/// At most this many connections are served at once; further ones wait in the queue of the socket.
pub const FAEDEN_MAX: usize = 64;

/// A clock in milliseconds since 1970 (in production the system time).
pub type Uhr = Box<dyn Fn() -> i64 + Send + Sync>;

/// A service of one mode.
pub trait Server: Send + Sync {
    /// Serves one connection of the harness up to its end.
    fn bediene(&self, harness: UnixStream);
    /// The common running part.
    fn betrieb(&self) -> &Betrieb;
}

/// Tears one connection down from another thread.
pub(crate) struct Abbruch {
    pub(crate) harness: UnixStream,
    /// Set before the shutdown: from then on the sheath refuses every read and write, whatever the system does with the
    /// shutdown (on macOS it has no effect on a stream whose other side already shut down its writing half).
    gerissen: Arc<AtomicBool>,
    pub(crate) ziel: Mutex<Option<Box<dyn Fn() + Send + Sync>>>,
}

impl Abbruch {
    pub(crate) fn reissen(&self) {
        self.gerissen.store(true, Ordering::SeqCst);
        let _ = self.harness.shutdown(std::net::Shutdown::Both);
        if let Some(f) = self
            .ziel
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .as_ref()
        {
            f();
        }
    }

    /// The stream of the harness in the sheath that this tear-down locks.
    pub(crate) fn huelle(&self, strom: UnixStream) -> Huelle {
        Huelle {
            strom,
            gerissen: Arc::clone(&self.gerissen),
        }
    }

    /// Sets the way to tear the connection to the target down; at the end of the process at once.
    pub(crate) fn setze_ziel(&self, betrieb: &Betrieb, f: Box<dyn Fn() + Send + Sync>) {
        *self.ziel.lock().unwrap_or_else(PoisonError::into_inner) = Some(f);
        if betrieb.ende.load(Ordering::SeqCst) {
            self.reissen();
        }
    }
}

/// The stream of the harness: after the tear-down every read and write is refused with `BrokenPipe`, so the lock does
/// not depend on what `shutdown` does on the system. Every write place of a connection goes through it, also the TLS
/// stream over it in mode 2.
pub(crate) struct Huelle {
    strom: UnixStream,
    gerissen: Arc<AtomicBool>,
}

impl Huelle {
    fn pruefen(&self) -> io::Result<()> {
        if self.gerissen.load(Ordering::SeqCst) {
            Err(io::ErrorKind::BrokenPipe.into())
        } else {
            Ok(())
        }
    }

    /// A second handle on the same stream, locked by the same tear-down.
    #[cfg(any(target_os = "linux", test))]
    pub(crate) fn try_clone(&self) -> io::Result<Self> {
        Ok(Self {
            strom: self.strom.try_clone()?,
            gerissen: Arc::clone(&self.gerissen),
        })
    }

    /// Shuts the stream down itself, past the lock: this is the end of the connection, not a write.
    pub(crate) fn shutdown(&self, wie: std::net::Shutdown) -> io::Result<()> {
        self.strom.shutdown(wie)
    }

    #[cfg(target_os = "linux")]
    pub(crate) fn set_read_timeout(&self, frist: Option<Duration>) -> io::Result<()> {
        self.strom.set_read_timeout(frist)
    }
}

impl Read for Huelle {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.pruefen()?;
        self.strom.read(buf)
    }
}

impl Write for Huelle {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.pruefen()?;
        self.strom.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.pruefen()?;
        self.strom.flush()
    }
}

/// The common running state.
pub struct Betrieb {
    pub(crate) uhr: Uhr,
    laufend: AtomicUsize,
    pub(crate) protokoll_kaputt: AtomicBool,
    ende: AtomicBool,
    abbrueche: Mutex<Vec<(u64, Arc<Abbruch>)>>,
    naechste: AtomicU64,
    faeden: Mutex<usize>,
    faden_frei: Condvar,
}

/// A place among the 8 in work; dropping it frees the place.
pub(crate) struct Platz<'a>(&'a AtomicUsize);

impl Drop for Platz<'_> {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

/// One registered connection; dropping it takes it out of the tear-down list.
pub(crate) struct Anmeldung<'a> {
    betrieb: &'a Betrieb,
    id: u64,
    pub(crate) abbruch: Arc<Abbruch>,
}

impl Drop for Anmeldung<'_> {
    fn drop(&mut self) {
        let id = self.id;
        self.betrieb
            .abbrueche
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .retain(|(n, _)| *n != id);
    }
}

impl Betrieb {
    /// A fresh state with the clock of the lines.
    #[must_use]
    pub fn neu(uhr: Uhr) -> Self {
        Self {
            uhr,
            laufend: AtomicUsize::new(0),
            protokoll_kaputt: AtomicBool::new(false),
            ende: AtomicBool::new(false),
            abbrueche: Mutex::new(Vec::new()),
            naechste: AtomicU64::new(0),
            faeden: Mutex::new(0),
            faden_frei: Condvar::new(),
        }
    }

    /// One of the 8 places, or `None`.
    pub(crate) fn platz(&self) -> Option<Platz<'_>> {
        self.laufend
            .try_update(Ordering::SeqCst, Ordering::SeqCst, |n| {
                (n < PARALLEL_MAX).then_some(n + 1)
            })
            .ok()
            .map(|_| Platz(&self.laufend))
    }

    /// Registers a connection for the tear-down; torn down at once after the end began.
    pub(crate) fn anmelden(&self, harness: &UnixStream) -> Option<Anmeldung<'_>> {
        let abbruch = Arc::new(Abbruch {
            harness: harness.try_clone().ok()?,
            gerissen: Arc::new(AtomicBool::new(false)),
            ziel: Mutex::new(None),
        });
        let id = self.naechste.fetch_add(1, Ordering::SeqCst);
        self.abbrueche
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push((id, Arc::clone(&abbruch)));
        if self.ende.load(Ordering::SeqCst) {
            abbruch.reissen();
        }
        Some(Anmeldung {
            betrieb: self,
            id,
            abbruch,
        })
    }

    /// How many connections are registered for the tear-down (for the tests of the registration).
    #[cfg(test)]
    pub(crate) fn angemeldet(&self) -> usize {
        self.abbrueche
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .len()
    }

    /// End of the process: every running connection is torn down (each still writes its line), new ones too.
    pub fn abreissen_alle(&self) {
        self.ende.store(true, Ordering::SeqCst);
        for (_, a) in self
            .abbrueche
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .iter()
        {
            a.reissen();
        }
    }

    /// Waits at most `frist` until no connection is served any more; true if none is left.
    #[must_use]
    pub fn warte_leer(&self, frist: Duration) -> bool {
        let ende = Instant::now() + frist;
        let mut n = self.faeden.lock().unwrap_or_else(PoisonError::into_inner);
        while *n > 0 {
            let Some(rest) = ende.checked_duration_since(Instant::now()) else {
                return false;
            };
            n = self
                .faden_frei
                .wait_timeout(n, rest)
                .unwrap_or_else(PoisonError::into_inner)
                .0;
        }
        true
    }
}

/// One running connection thread; dropping it frees its place.
pub struct Faden(Arc<dyn Server>);

impl Faden {
    /// Waits until fewer than [`FAEDEN_MAX`] connections are served and takes a place.
    #[must_use]
    pub fn nimm(server: Arc<dyn Server>) -> Self {
        {
            let b = server.betrieb();
            let mut n = b.faeden.lock().unwrap_or_else(PoisonError::into_inner);
            while *n >= FAEDEN_MAX {
                n = b.faden_frei.wait(n).unwrap_or_else(PoisonError::into_inner);
            }
            *n += 1;
        }
        Self(server)
    }

    /// Serves one connection with the place of this thread, then frees the place.
    pub fn bediene(self, harness: UnixStream) {
        self.0.bediene(harness);
    }
}

impl Drop for Faden {
    fn drop(&mut self) {
        let b = self.0.betrieb();
        let mut n = b.faeden.lock().unwrap_or_else(PoisonError::into_inner);
        *n -= 1;
        b.faden_frei.notify_all();
    }
}

#[cfg(test)]
mod tests {
    use super::Betrieb;
    use std::io::{Read, Write};
    use std::os::unix::net::UnixStream;
    use std::sync::atomic::Ordering;
    use std::time::Duration;

    #[test]
    fn a_finished_connection_leaves_the_tear_down_list() {
        let b = Betrieb::neu(Box::new(|| 0));
        let (x, _y) = UnixStream::pair().expect("pair");
        let erste = b.anmelden(&x).expect("registered");
        let zweite = b.anmelden(&x).expect("registered");
        assert_eq!(b.angemeldet(), 2);
        drop(erste);
        assert_eq!(b.angemeldet(), 1);
        drop(zweite);
        assert_eq!(b.angemeldet(), 0);
    }

    /// The lock sits in the sheath, not in `shutdown`: only the state is set here and `shutdown` is never called, so
    /// the test shows the lock on every system (on macOS `shutdown` has no effect after the other side shut down).
    #[test]
    fn a_torn_down_sheath_refuses_reading_and_writing() {
        let betrieb = Betrieb::neu(Box::new(|| 0));
        let (nah, mut fern) = UnixStream::pair().expect("pair");
        fern.set_read_timeout(Some(Duration::from_secs(5)))
            .expect("timeout");
        let anm = betrieb.anmelden(&nah).expect("registered");
        let mut huelle = anm.abbruch.huelle(nah.try_clone().expect("clone"));
        huelle.write_all(b"a").expect("before");
        huelle.flush().expect("flush before");
        anm.abbruch.gerissen.store(true, Ordering::SeqCst);
        let art = |r: std::io::Result<usize>| r.expect_err("refused").kind();
        let kaputt = std::io::ErrorKind::BrokenPipe;
        assert_eq!(art(huelle.write(b"b")), kaputt);
        assert_eq!(art(huelle.read(&mut [0; 4])), kaputt);
        assert_eq!(huelle.flush().expect_err("refused").kind(), kaputt);
        let mut zweite = huelle.try_clone().expect("clone");
        assert_eq!(art(zweite.write(b"c")), kaputt);
        drop((huelle, zweite, anm, nah));
        let mut gelesen = Vec::new();
        fern.read_to_end(&mut gelesen).expect("end");
        assert_eq!(gelesen, b"a");
    }

    #[test]
    fn tearing_down_sets_the_lock_before_the_shutdown() {
        let betrieb = Betrieb::neu(Box::new(|| 0));
        let (nah, mut fern) = UnixStream::pair().expect("pair");
        fern.set_read_timeout(Some(Duration::from_secs(5)))
            .expect("timeout");
        let anm = betrieb.anmelden(&nah).expect("registered");
        let mut huelle = anm.abbruch.huelle(nah.try_clone().expect("clone"));
        // The other side has shut its writing half down, as a harness does after its request.
        fern.shutdown(std::net::Shutdown::Write)
            .expect("half close");
        anm.abbruch.reissen();
        assert!(anm.abbruch.gerissen.load(Ordering::SeqCst));
        assert!(huelle.write_all(b"late").is_err());
        drop((huelle, anm, nah));
        let mut gelesen = Vec::new();
        fern.read_to_end(&mut gelesen).expect("end");
        assert_eq!(gelesen, Vec::<u8>::new());
    }

    #[test]
    fn a_connection_registered_after_the_end_is_locked_at_once() {
        let betrieb = Betrieb::neu(Box::new(|| 0));
        betrieb.abreissen_alle();
        let (nah, _fern) = UnixStream::pair().expect("pair");
        let anm = betrieb.anmelden(&nah).expect("registered");
        let mut huelle = anm.abbruch.huelle(nah.try_clone().expect("clone"));
        assert!(huelle.write_all(b"x").is_err());
    }

    /// The shutdown of the sheath reaches the other side while another handle of the stream is still open.
    #[test]
    fn the_sheath_ends_the_connection_for_the_other_side() {
        let betrieb = Betrieb::neu(Box::new(|| 0));
        let (nah, mut fern) = UnixStream::pair().expect("pair");
        fern.set_read_timeout(Some(Duration::from_secs(5)))
            .expect("timeout");
        let anm = betrieb.anmelden(&nah).expect("registered");
        let mut huelle = anm.abbruch.huelle(nah.try_clone().expect("clone"));
        huelle.write_all(b"a").expect("write");
        huelle.shutdown(std::net::Shutdown::Both).expect("shutdown");
        let mut gelesen = Vec::new();
        fern.read_to_end(&mut gelesen).expect("end");
        assert_eq!(gelesen, b"a");
        drop((huelle, anm, nah));
    }
}
