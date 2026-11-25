//! Waiting for the readiness of sockets: which of them can be read now, in which order (contract `dienst`, section 11).
//!
//! The service handles the input of several viewers in the order in which it learns that they are ready. On Linux that is the
//! readiness list of `epoll`: the kernel appends a socket to it when its bytes arrive, so the order is the order of arrival, also
//! when the loop wakes late (a machine under heavy load). Elsewhere (macOS) there is only `poll`, which names the ready sockets in
//! the order of the array; the array is sorted by key, so the order is fixed, but it is the order of the keys, not of the arrival.
//! Both come from `rustix` (feature `event`), safe API only.

use crate::sys::{AsFd, OwnedFd};
use std::collections::BTreeMap;
use std::io;
use std::time::Duration;

use rustix::event::Timespec;

/// How many ready sockets one wait names at most; the rest stays ready and comes with the next one.
#[cfg(target_os = "linux")]
const EREIGNISSE_MAX: usize = 256;

/// A set of sockets with a key each, waited for with `poll`. The registered sockets are copies of the handed ones (`dup`): the
/// original may be closed before [`Abfrage::weg`]. It is built on every platform (the tests run it on Linux too, so that the
/// code that macOS uses is tested where the gates run); `Bereitschaft` is this type where there is no `epoll`.
#[cfg_attr(target_os = "linux", allow(dead_code))]
pub(crate) struct Abfrage {
    quellen: BTreeMap<u64, OwnedFd>,
}

/// The set the loop uses: `epoll` on Linux, else the `poll` set.
#[cfg(not(target_os = "linux"))]
pub(crate) type Bereitschaft = Abfrage;

/// A set of sockets with a key each, waited for with `epoll` (the order of the readiness list is the order of arrival). The
/// registered sockets are copies of the handed ones (`dup`): the original may be closed before [`Bereitschaft::weg`].
#[cfg(target_os = "linux")]
pub(crate) struct Bereitschaft {
    quellen: BTreeMap<u64, OwnedFd>,
    epoll: OwnedFd,
}

/// `frist` as the time of a wait (the longest waits of the loop are a few milliseconds).
fn zeit(frist: Duration) -> Timespec {
    Timespec {
        tv_sec: i64::try_from(frist.as_secs()).unwrap_or(i64::MAX),
        #[cfg(unix)]
        tv_nsec: frist.subsec_nanos().into(),
        // Windows: the nanoseconds are an `i32` in `rustix` (`sys::nanosekunden`)
        #[cfg(windows)]
        tv_nsec: crate::sys::nanosekunden(frist),
    }
}

#[cfg(target_os = "linux")]
impl Bereitschaft {
    /// An empty set.
    pub(crate) fn neu() -> io::Result<Self> {
        let epoll = rustix::event::epoll::create(rustix::event::epoll::CreateFlags::CLOEXEC)?;
        Ok(Self {
            quellen: BTreeMap::new(),
            epoll,
        })
    }

    /// Adds a socket under `schluessel` (an old one with this key is replaced).
    pub(crate) fn hinzu(&mut self, quelle: &impl AsFd, schluessel: u64) -> io::Result<()> {
        use rustix::event::epoll::{EventData, EventFlags, add};
        self.weg(schluessel);
        let kopie = quelle.as_fd().try_clone_to_owned()?;
        add(
            &self.epoll,
            &kopie,
            EventData::new_u64(schluessel),
            EventFlags::IN,
        )?;
        self.quellen.insert(schluessel, kopie);
        Ok(())
    }

    /// Removes the socket of `schluessel`.
    pub(crate) fn weg(&mut self, schluessel: u64) {
        if let Some(kopie) = self.quellen.remove(&schluessel) {
            let _ = rustix::event::epoll::delete(&self.epoll, &kopie);
        }
    }

    /// Waits at most `frist`; gives the keys of the sockets that are ready (data, end or error), in the order of their readiness.
    pub(crate) fn warten(&mut self, frist: Duration) -> Vec<u64> {
        let mut liste = Vec::with_capacity(EREIGNISSE_MAX);
        let t = zeit(frist);
        match rustix::event::epoll::wait(
            &self.epoll,
            rustix::buffer::spare_capacity(&mut liste),
            Some(&t),
        ) {
            Ok(_) => liste.iter().map(|e| e.data.u64()).collect(),
            // a signal or a broken call: nothing is named, the loop turns again (the beat stays)
            Err(_) => Vec::new(),
        }
    }
}

#[cfg_attr(target_os = "linux", allow(dead_code))]
impl Abfrage {
    /// An empty set. It returns a `Result` like the set of `epoll`, so that the loop calls both the same way.
    #[allow(clippy::unnecessary_wraps)]
    pub(crate) fn neu() -> io::Result<Self> {
        Ok(Self {
            quellen: BTreeMap::new(),
        })
    }

    /// Adds a socket under `schluessel` (an old one with this key is replaced).
    pub(crate) fn hinzu(&mut self, quelle: &impl AsFd, schluessel: u64) -> io::Result<()> {
        let kopie = quelle.as_fd().try_clone_to_owned()?;
        self.quellen.insert(schluessel, kopie);
        Ok(())
    }

    /// Removes the socket of `schluessel`.
    pub(crate) fn weg(&mut self, schluessel: u64) {
        self.quellen.remove(&schluessel);
    }

    /// Waits at most `frist`; gives the keys of the sockets that are ready (data, end or error), in the order of the keys.
    pub(crate) fn warten(&mut self, frist: Duration) -> Vec<u64> {
        use rustix::event::{PollFd, PollFlags, poll};
        let mut fds: Vec<PollFd<'_>> = self
            .quellen
            .values()
            .map(|fd| PollFd::new(fd, PollFlags::IN))
            .collect();
        let t = zeit(frist);
        match poll(&mut fds, Some(&t)) {
            Ok(_) => self
                .quellen
                .keys()
                .zip(&fds)
                .filter(|(_, f)| !f.revents().is_empty())
                .map(|(k, _)| *k)
                .collect(),
            // a signal or a broken call: nothing is named, the loop turns again (the beat stays)
            Err(_) => Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sys::UnixStream;
    use std::io::Write;

    fn paar() -> (UnixStream, UnixStream) {
        UnixStream::pair().expect("pair")
    }

    /// The tests every kind of set has to pass.
    macro_rules! gemeinsam {
        ($modul:ident, $typ:ty) => {
            mod $modul {
                use super::*;

                #[test]
                fn ohne_bereite_quelle_laeuft_die_frist_ab() {
                    let mut b = <$typ>::neu().expect("set");
                    let (a, _g) = paar();
                    b.hinzu(&a, 5).expect("add");
                    let beginn = std::time::Instant::now();
                    assert_eq!(b.warten(Duration::from_millis(30)), Vec::<u64>::new());
                    assert!(beginn.elapsed() >= Duration::from_millis(25));
                }

                #[test]
                fn nennt_bereite_quellen_und_weckt_sofort() {
                    let mut b = <$typ>::neu().expect("set");
                    let (a, mut ga) = paar();
                    let (c, _gc) = paar();
                    b.hinzu(&a, 3).expect("add");
                    b.hinzu(&c, 9).expect("add");
                    ga.write_all(&[1]).expect("write");
                    let beginn = std::time::Instant::now();
                    assert_eq!(b.warten(Duration::from_secs(5)), vec![3]);
                    assert!(beginn.elapsed() < Duration::from_secs(2));
                    // level triggered: it stays ready until it is read
                    assert_eq!(b.warten(Duration::ZERO), vec![3]);
                }

                #[test]
                fn ende_der_verbindung_ist_bereit() {
                    let mut b = <$typ>::neu().expect("set");
                    let (a, ga) = paar();
                    b.hinzu(&a, 1).expect("add");
                    drop(ga);
                    assert_eq!(b.warten(Duration::from_secs(5)), vec![1]);
                }

                #[test]
                fn weg_nimmt_die_quelle_heraus() {
                    let mut b = <$typ>::neu().expect("set");
                    let (a, mut ga) = paar();
                    let (c, mut gc) = paar();
                    b.hinzu(&a, 1).expect("add");
                    b.hinzu(&c, 2).expect("add");
                    b.weg(1);
                    ga.write_all(&[1]).expect("write");
                    assert_eq!(b.warten(Duration::from_millis(20)), Vec::<u64>::new());
                    // the other one stays
                    gc.write_all(&[1]).expect("write");
                    assert_eq!(b.warten(Duration::from_secs(5)), vec![2]);
                    // removing a key that is not there is harmless
                    b.weg(1);
                    b.weg(7);
                }

                #[test]
                fn eine_quelle_mit_gleichem_schluessel_ersetzt_die_alte() {
                    let mut b = <$typ>::neu().expect("set");
                    let (a, mut ga) = paar();
                    let (c, mut gc) = paar();
                    b.hinzu(&a, 1).expect("add");
                    b.hinzu(&c, 1).expect("add");
                    ga.write_all(&[1]).expect("write");
                    assert_eq!(b.warten(Duration::from_millis(20)), Vec::<u64>::new());
                    gc.write_all(&[1]).expect("write");
                    assert_eq!(b.warten(Duration::from_secs(5)), vec![1]);
                }

                #[test]
                fn mehrere_bereite_quellen_alle_genannt() {
                    let mut b = <$typ>::neu().expect("set");
                    let (a, mut ga) = paar();
                    let (c, mut gc) = paar();
                    let (d, _gd) = paar();
                    b.hinzu(&a, 1).expect("add");
                    b.hinzu(&c, 2).expect("add");
                    b.hinzu(&d, 3).expect("add");
                    ga.write_all(&[1]).expect("write");
                    gc.write_all(&[1]).expect("write");
                    let mut bereit = b.warten(Duration::from_secs(5));
                    bereit.sort_unstable();
                    assert_eq!(bereit, vec![1, 2]);
                }
            }
        };
    }

    gemeinsam!(abfrage, Abfrage);
    #[cfg(target_os = "linux")]
    gemeinsam!(epoll, Bereitschaft);

    #[test]
    fn abfrage_nennt_die_folge_der_schluessel() {
        let mut b = Abfrage::neu().expect("set");
        let (a, mut ga) = paar();
        let (c, mut gc) = paar();
        b.hinzu(&a, 1).expect("add");
        b.hinzu(&c, 2).expect("add");
        gc.write_all(&[1]).expect("write");
        ga.write_all(&[1]).expect("write");
        assert_eq!(b.warten(Duration::from_secs(5)), vec![1, 2]);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn epoll_nennt_die_folge_der_ankunft_nicht_die_der_schluessel() {
        let mut b = Bereitschaft::neu().expect("set");
        let (a, mut ga) = paar();
        let (c, mut gc) = paar();
        b.hinzu(&a, 1).expect("add");
        b.hinzu(&c, 2).expect("add");
        // the socket with the higher key gets its byte first, and nobody waits in between
        gc.write_all(&[1]).expect("write");
        ga.write_all(&[1]).expect("write");
        assert_eq!(b.warten(Duration::from_secs(5)), vec![2, 1]);
    }

    #[test]
    fn zeit_rechnet_sekunden_und_nanosekunden() {
        let t = zeit(Duration::new(3, 7));
        assert_eq!((t.tv_sec, t.tv_nsec), (3, 7));
        let t = zeit(Duration::from_millis(2));
        assert_eq!((t.tv_sec, t.tv_nsec), (0, 2_000_000));
    }
}
