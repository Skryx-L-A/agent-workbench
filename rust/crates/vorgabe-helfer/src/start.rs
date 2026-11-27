//! The start of the program under Linux (rust/spec/paket/README.md, part A3, "Die Einheiten"): exactly one socket from
//! systemd, the process runs as `werkbank-vorgabe`, then one connection after the other until SIGTERM or SIGINT.
//! `main` only joins these parts; each part is checked here without systemd and without root.

use std::io;
use std::os::unix::net::{UnixListener, UnixStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use rustix::event::{PollFd, PollFlags, poll};

use crate::helfer::HELFERNUTZER;
use crate::konten::Konten;

/// Wait of one round of the loop for a connection; a signal is seen at the latest after this time.
pub const WARTEN: rustix::time::Timespec = rustix::time::Timespec {
    tv_sec: 0,
    tv_nsec: 500_000_000,
};
/// Pause after a failed `accept`, so that a lasting error does not spin.
pub const PAUSE: Duration = Duration::from_millis(100);

/// The listening socket if systemd handed over exactly one (`anzahl` from `LISTEN_FDS`) and it is a Unix stream socket;
/// `nehmen` takes it and is called only for exactly one.
pub fn socket_waehlen(
    anzahl: usize,
    nehmen: impl FnOnce() -> io::Result<Option<UnixListener>>,
) -> Option<UnixListener> {
    if anzahl != 1 {
        return None;
    }
    nehmen().ok().flatten()
}

/// The uid of `werkbank-vorgabe` if the process runs under it (`eigene` is the effective uid); `None` otherwise.
pub fn helfer_uid(k: &impl Konten, eigene: u32) -> Option<u32> {
    k.uid(HELFERNUTZER).filter(|u| *u == eigene)
}

/// Takes one connection after the other from `hoerer` (non-blocking) and hands each to `eine`, until `halt` is set.
/// Two connections never run at the same time.
pub fn schleife(
    hoerer: &UnixListener,
    halt: &AtomicBool,
    warten: rustix::time::Timespec,
    mut eine: impl FnMut(UnixStream),
) {
    while !halt.load(Ordering::SeqCst) {
        let mut fds = [PollFd::new(hoerer, PollFlags::IN)];
        if let Ok(0) | Err(_) = poll(&mut fds, Some(&warten)) {
            continue;
        }
        match hoerer.accept() {
            Ok((strom, _)) => eine(strom),
            Err(_) => std::thread::sleep(PAUSE),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::konten::Tabelle;
    use std::cell::Cell;
    use std::io::{Read, Write};
    use std::sync::Arc;

    fn ort(name: &str) -> std::path::PathBuf {
        let p = std::env::temp_dir().join(format!("vh-start-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir(&p).expect("folder");
        p
    }

    #[test]
    fn genau_ein_socket() {
        let p = ort("waehlen");
        let gerufen = Cell::new(0);
        for anzahl in [0, 2, 3] {
            assert!(
                socket_waehlen(anzahl, || {
                    gerufen.set(gerufen.get() + 1);
                    Ok(None)
                })
                .is_none()
            );
        }
        assert_eq!(gerufen.get(), 0, "taken only for exactly one");
        assert!(socket_waehlen(1, || Ok(None)).is_none());
        assert!(socket_waehlen(1, || Err(io::ErrorKind::InvalidInput.into())).is_none());
        let l = UnixListener::bind(p.join("s")).expect("bind");
        assert!(socket_waehlen(1, || Ok(Some(l))).is_some());
        std::fs::remove_dir_all(&p).expect("rm");
    }

    #[test]
    fn nur_als_helfernutzer() {
        let t = Tabelle(vec![
            ("werkbank-vorgabe".into(), 990),
            ("anderes".into(), 991),
        ]);
        assert_eq!(helfer_uid(&t, 990), Some(990));
        assert_eq!(helfer_uid(&t, 991), None);
        assert_eq!(helfer_uid(&t, 0), None);
        assert_eq!(
            helfer_uid(&Tabelle(vec![("anderes".into(), 990)]), 990),
            None
        );
    }

    #[test]
    fn eine_verbindung_nach_der_anderen_bis_zum_halt() {
        let p = ort("schleife");
        let sock = p.join("s");
        let hoerer = UnixListener::bind(&sock).expect("bind");
        hoerer.set_nonblocking(true).expect("nonblocking");
        let halt = Arc::new(AtomicBool::new(false));
        let kurz = rustix::time::Timespec {
            tv_sec: 0,
            tv_nsec: 20_000_000,
        };
        let s2 = sock.clone();
        let client = std::thread::spawn(move || {
            let mut antworten = Vec::new();
            for _ in 0..2 {
                let mut c = UnixStream::connect(&s2).expect("connect");
                c.set_read_timeout(Some(Duration::from_secs(5)))
                    .expect("timeout");
                let mut a = String::new();
                let _ = c.read_to_string(&mut a);
                antworten.push(a);
            }
            antworten
        });
        let mut n = 0;
        let h = Arc::clone(&halt);
        schleife(&hoerer, &halt, kurz, |mut s| {
            n += 1;
            let _ = s.write_all(format!("{n}").as_bytes());
            if n == 2 {
                h.store(true, Ordering::SeqCst);
            }
        });
        assert_eq!(n, 2);
        assert_eq!(
            client.join().expect("client"),
            vec!["1".to_owned(), "2".to_owned()]
        );
        // a set halt takes no further connection, even with one waiting
        let _warte = UnixStream::connect(&sock).expect("connect");
        let mut weitere = 0;
        schleife(&hoerer, &halt, kurz, |_| weitere += 1);
        assert_eq!(weitere, 0);
        assert_eq!(
            (WARTEN.tv_sec, WARTEN.tv_nsec, PAUSE),
            (0, 500_000_000, Duration::from_millis(100))
        );
        std::fs::remove_dir_all(&p).expect("rm");
    }
}
