//! TR15: the write queue of a session. `Kind::schreibe` puts the bytes here and returns at once; a writing thread takes
//! them from the front and writes them to the PTY. A child that never reads its input, or reads slowly, fills the PTY
//! buffer (about 4 KiB on Linux); the writing thread then waits in `write`, and the caller never does.

use std::collections::VecDeque;
use std::io::{self, Write};
use std::sync::{Condvar, Mutex, MutexGuard, PoisonError};

/// The most bytes the queue holds, 256 KiB (262,144 bytes). A write that would take the queue above it is refused whole,
/// nothing of it is queued. Why this number: the input of one session is keys and pastes, a few bytes to some KiB; 256 KiB
/// is far above a large paste (it is 64 times the PTY buffer of Linux) and keeps the memory a stuck child can bind at
/// 256 KiB per session, a small part of the 11.4 MiB budget of one session (P7). A larger limit only moves the time at
/// which a stuck child is noticed.
pub const SCHLANGE_MAX: usize = 262_144;

/// The most bytes the writing thread hands to one `write_all`, so that it looks at the queue between two pieces.
const STUECK: usize = 4096;

#[derive(Default)]
struct Inhalt {
    bytes: VecDeque<u8>,
    /// The session is closed; the writing thread ends and drops what is left.
    zu: bool,
    /// A write to the PTY failed (the child is gone); the queue takes nothing more.
    kaputt: bool,
}

/// The queue and the signal that wakes the writing thread.
#[derive(Default)]
pub(crate) struct Schlange {
    inhalt: Mutex<Inhalt>,
    wach: Condvar,
}

impl Schlange {
    fn sperre(&self) -> MutexGuard<'_, Inhalt> {
        self.inhalt.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Queues the bytes and never waits.
    ///
    /// # Errors
    /// `WouldBlock` when the queue would hold more than [`SCHLANGE_MAX`] bytes after it (nothing is queued); `BrokenPipe` when a
    /// write to the PTY failed before or the session is closed.
    pub(crate) fn einreihen(&self, bytes: &[u8]) -> io::Result<()> {
        let mut inhalt = self.sperre();
        if inhalt.kaputt || inhalt.zu {
            return Err(io::Error::from(io::ErrorKind::BrokenPipe));
        }
        if bytes.len() > SCHLANGE_MAX - inhalt.bytes.len() {
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "Schreibschlange voll",
            ));
        }
        if !bytes.is_empty() {
            inhalt.bytes.extend(bytes);
            self.wach.notify_one();
        }
        Ok(())
    }

    /// Closes the queue: the writing thread ends, the bytes still in it are dropped.
    pub(crate) fn schliessen(&self) {
        self.sperre().zu = true;
        self.wach.notify_all();
    }

    /// The body of the writing thread: writes the queue in order until the queue is closed or a write fails.
    pub(crate) fn schreiben(&self, ziel: &mut dyn Write) {
        loop {
            let stueck: Vec<u8> = {
                let mut inhalt = self.sperre();
                while inhalt.bytes.is_empty() && !inhalt.zu {
                    inhalt = self
                        .wach
                        .wait(inhalt)
                        .unwrap_or_else(PoisonError::into_inner);
                }
                if inhalt.zu {
                    return;
                }
                inhalt.bytes.iter().take(STUECK).copied().collect()
            };
            // The bytes stay in the queue while they are written, so the limit counts them too.
            let gelungen = ziel.write_all(&stueck).and_then(|()| ziel.flush()).is_ok();
            let mut inhalt = self.sperre();
            if !gelungen {
                inhalt.kaputt = true;
                inhalt.bytes.clear();
                return;
            }
            inhalt.bytes.drain(..stueck.len());
        }
    }

    /// How many bytes wait in the queue (tests).
    #[cfg(test)]
    fn laenge(&self) -> usize {
        self.sperre().bytes.len()
    }
}

#[cfg(test)]
mod tests {
    use std::io;
    use std::sync::mpsc::{Receiver, Sender, channel};
    use std::sync::{Arc, Mutex};
    use std::thread;
    use std::time::{Duration, Instant};

    use super::{SCHLANGE_MAX, STUECK, Schlange};

    /// A target that hands every write to a test and waits for the test to let it through, like a PTY a child does not read.
    struct Gesperrt {
        gesehen: Sender<Vec<u8>>,
        frei: Receiver<()>,
    }

    impl io::Write for Gesperrt {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            let _ = self.gesehen.send(buf.to_vec());
            let _ = self.frei.recv();
            Ok(buf.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    /// A target that collects what it gets.
    #[derive(Clone, Default)]
    struct Sammler(Arc<Mutex<Vec<u8>>>);

    impl io::Write for Sammler {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.0.lock().expect("lock").extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    /// A target that refuses every write.
    struct Kaputt;

    impl io::Write for Kaputt {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> {
            Err(io::Error::from(io::ErrorKind::BrokenPipe))
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    /// Waits for the thread to end; a thread that never ends is a failure of the test, not a hang of it.
    fn warte_ende(faden: thread::JoinHandle<()>) {
        let frist = Instant::now() + Duration::from_secs(8);
        while !faden.is_finished() {
            assert!(Instant::now() < frist, "the thread did not end");
            thread::sleep(Duration::from_millis(5));
        }
        faden.join().expect("the thread ends without a panic");
    }

    fn warte_bis(bedingung: impl Fn() -> bool) {
        let frist = Instant::now() + Duration::from_secs(8);
        while !bedingung() {
            assert!(Instant::now() < frist, "the condition did not come true");
            thread::sleep(Duration::from_millis(5));
        }
    }

    #[test]
    fn the_limit_is_256_kib() {
        assert_eq!(SCHLANGE_MAX, 262_144);
    }

    #[test]
    fn a_write_up_to_the_limit_is_queued_and_one_byte_more_is_refused_whole() {
        let schlange = Schlange::default();
        assert!(schlange.einreihen(&vec![1; SCHLANGE_MAX]).is_ok());
        assert_eq!(schlange.laenge(), SCHLANGE_MAX);
        let fehler = schlange.einreihen(&[2]).expect_err("the queue is full");
        assert_eq!(fehler.kind(), io::ErrorKind::WouldBlock);
        assert_eq!(schlange.laenge(), SCHLANGE_MAX, "nothing of it was queued");
        let schlange = Schlange::default();
        let fehler = schlange
            .einreihen(&vec![1; SCHLANGE_MAX + 1])
            .expect_err("more than the limit at once");
        assert_eq!(fehler.kind(), io::ErrorKind::WouldBlock);
        assert_eq!(schlange.laenge(), 0);
    }

    #[test]
    fn the_free_space_counts_what_is_queued() {
        let schlange = Schlange::default();
        assert!(schlange.einreihen(&vec![1; SCHLANGE_MAX - 10]).is_ok());
        assert!(schlange.einreihen(&[2; 10]).is_ok());
        assert!(schlange.einreihen(&[3]).is_err());
        let schlange = Schlange::default();
        assert!(schlange.einreihen(&vec![1; SCHLANGE_MAX - 10]).is_ok());
        assert!(schlange.einreihen(&[2; 11]).is_err());
        assert_eq!(schlange.laenge(), SCHLANGE_MAX - 10);
    }

    #[test]
    fn an_empty_write_is_fine_even_when_the_queue_is_full() {
        let schlange = Schlange::default();
        assert!(schlange.einreihen(&vec![1; SCHLANGE_MAX]).is_ok());
        assert!(schlange.einreihen(&[]).is_ok());
    }

    #[test]
    fn the_writing_thread_writes_everything_in_order() {
        let schlange = Arc::new(Schlange::default());
        let sammler = Sammler::default();
        let faden = {
            let (schlange, mut sammler) = (Arc::clone(&schlange), sammler.clone());
            thread::spawn(move || schlange.schreiben(&mut sammler))
        };
        let mut erwartet = Vec::new();
        for i in 0u8..40 {
            let stueck = vec![i; 1000 + usize::from(i)];
            schlange.einreihen(&stueck).expect("queued");
            erwartet.extend_from_slice(&stueck);
        }
        warte_bis(|| sammler.0.lock().expect("lock").len() == erwartet.len());
        assert_eq!(*sammler.0.lock().expect("lock"), erwartet);
        warte_bis(|| schlange.laenge() == 0);
        schlange.schliessen();
        warte_ende(faden);
    }

    #[test]
    fn a_target_that_does_not_take_bytes_never_holds_the_caller() {
        let schlange = Arc::new(Schlange::default());
        let (gesehen, gesehen_empfaenger) = channel();
        let (frei_sender, frei) = channel();
        let faden = {
            let schlange = Arc::clone(&schlange);
            thread::spawn(move || schlange.schreiben(&mut Gesperrt { gesehen, frei }))
        };
        schlange.einreihen(&[7; 100]).expect("queued");
        let erstes = gesehen_empfaenger
            .recv_timeout(Duration::from_secs(8))
            .expect("the thread is in write");
        assert_eq!(erstes, vec![7; 100]);
        // the thread now waits in write; the caller still queues, and the limit counts the bytes in flight
        let start = Instant::now();
        assert!(schlange.einreihen(&vec![8; SCHLANGE_MAX - 100]).is_ok());
        assert!(schlange.einreihen(&[9]).is_err());
        assert!(start.elapsed() < Duration::from_secs(2));
        // let the writes through; the queue empties in order
        drop(frei_sender);
        let mut gesamt = erstes;
        while gesamt.len() < SCHLANGE_MAX {
            gesamt.extend(
                gesehen_empfaenger
                    .recv_timeout(Duration::from_secs(8))
                    .expect("the rest follows"),
            );
        }
        assert_eq!(gesamt.len(), SCHLANGE_MAX);
        assert!(gesamt[..100].iter().all(|b| *b == 7));
        assert!(gesamt[100..].iter().all(|b| *b == 8));
        schlange.schliessen();
        warte_ende(faden);
    }

    #[test]
    fn a_piece_is_at_most_one_buffer() {
        let schlange = Arc::new(Schlange::default());
        let (gesehen, gesehen_empfaenger) = channel();
        let (frei_sender, frei) = channel();
        let faden = {
            let schlange = Arc::clone(&schlange);
            thread::spawn(move || schlange.schreiben(&mut Gesperrt { gesehen, frei }))
        };
        schlange
            .einreihen(&vec![1; STUECK * 2 + 5])
            .expect("queued");
        let erstes = gesehen_empfaenger
            .recv_timeout(Duration::from_secs(8))
            .expect("first piece");
        assert_eq!(erstes.len(), STUECK);
        drop(frei_sender);
        schlange.schliessen();
        warte_ende(faden);
    }

    #[test]
    fn a_failed_write_closes_the_queue_for_good() {
        let schlange = Arc::new(Schlange::default());
        let faden = {
            let schlange = Arc::clone(&schlange);
            thread::spawn(move || schlange.schreiben(&mut Kaputt))
        };
        schlange.einreihen(&[1, 2, 3]).expect("queued");
        warte_ende(faden);
        assert_eq!(schlange.laenge(), 0);
        let fehler = schlange.einreihen(&[1]).expect_err("the PTY is gone");
        assert_eq!(fehler.kind(), io::ErrorKind::BrokenPipe);
    }

    #[test]
    fn a_closed_queue_takes_nothing_and_ends_the_thread() {
        let schlange = Arc::new(Schlange::default());
        let faden = {
            let schlange = Arc::clone(&schlange);
            thread::spawn(move || schlange.schreiben(&mut Sammler::default()))
        };
        schlange.schliessen();
        warte_ende(faden);
        let fehler = schlange.einreihen(&[1]).expect_err("closed");
        assert_eq!(fehler.kind(), io::ErrorKind::BrokenPipe);
    }
}
