//! The wake socket of the loop: one socket for every thread that has something for the loop (contract `dienst`, DI-L11).
//!
//! A thread never writes data into the socket. It puts a [`Botschaft`] into the channel and then writes one byte, which only wakes
//! the loop. The loop reads the bytes away and takes every waiting message from the channel; the kind of a message is its variant,
//! one per source, and the loop hands each message on by its kind (`Dienst::post_abholen`). So the loop never reads data without
//! the check of its kind. The stop of [`crate::Anhalter`] writes the same byte and sets its flag; it carries no data.

use std::io::{Read, Write};
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, channel};

use crate::dienst::{Gelesen, gelesen};
use crate::sys::UnixStream;

/// A message of a thread to the loop: one variant per source.
pub(crate) enum Botschaft {
    /// The local way (`lokal.rs`): a connection after the check of its peer, with its measured account.
    Lokal(crate::lokal::Neu),
}

/// The sending side: any thread may hold a copy.
#[derive(Clone)]
pub(crate) struct Wecker {
    sender: Sender<Botschaft>,
    /// The write end of the wake socket (non-blocking).
    weck: Arc<UnixStream>,
}

impl Wecker {
    /// Puts the message into the channel and wakes the loop. The message comes back when the loop is gone.
    ///
    /// # Errors
    /// The message itself, when the receiving side is dropped.
    pub(crate) fn senden(&self, botschaft: Botschaft) -> Result<(), Botschaft> {
        self.sender.send(botschaft).map_err(|e| e.0)?;
        // a full wake socket already holds a byte that wakes the loop: the error can be ignored
        let _ = (&*self.weck).write(&[1]);
        Ok(())
    }
}

/// The receiving side, owned by the loop.
pub(crate) struct Post {
    empfang: Receiver<Botschaft>,
    /// The read end of the wake socket (non-blocking).
    lese: UnixStream,
}

impl Post {
    /// The read end, for the set of sockets the loop waits for.
    pub(crate) fn strom(&self) -> &UnixStream {
        &self.lese
    }

    /// Reads every waiting byte of the wake socket away (so that the wait does not name it again) and returns every waiting
    /// message, oldest first.
    pub(crate) fn abholen(&mut self) -> Vec<Botschaft> {
        let mut puffer = [0_u8; 64];
        // until nothing is waiting (an interrupted read leaves a byte, which wakes the loop once more)
        while let Gelesen::Daten(_) = gelesen((&self.lese).read(&mut puffer)) {}
        let mut aus = Vec::new();
        // `Empty` and `Disconnected` both end the turn: nothing more is waiting
        while let Ok(b) = self.empfang.try_recv() {
            aus.push(b);
        }
        aus
    }
}

/// The wake socket as a pair: the write end for the [`Wecker`] (and the stop of [`crate::Anhalter`]), the read end in the [`Post`].
///
/// # Errors
/// The error of the system when the pair of sockets cannot be made.
pub(crate) fn paar() -> std::io::Result<(Wecker, Post, Arc<UnixStream>)> {
    let (lese, schreib) = UnixStream::pair()?;
    lese.set_nonblocking(true)?;
    schreib.set_nonblocking(true)?;
    let schreib = Arc::new(schreib);
    let (sender, empfang) = channel();
    Ok((
        Wecker {
            sender,
            weck: Arc::clone(&schreib),
        },
        Post { empfang, lese },
        schreib,
    ))
}

// `cfg(test)` on its own line: cargo-mutants leaves a module with it out, but not one with `cfg(all(test, ..))`
#[cfg(test)]
#[cfg(unix)]
mod tests {
    use super::*;
    use crate::lokal::{Neu, Wer};
    use std::io::ErrorKind;

    fn neu() -> Neu {
        let (strom, _anderes) = UnixStream::pair().expect("pair");
        Neu {
            strom,
            wer: Wer::unix("linux", 7, "anna".to_owned()),
        }
    }

    /// A message comes out in its order, with its kind, and the bytes of the wake socket are read away.
    #[test]
    fn botschaften_in_ihrer_ordnung_und_der_socket_leer() {
        let (wecker, mut post, _schreib) = paar().expect("pair");
        assert!(post.abholen().is_empty());
        for _ in 0..3 {
            assert!(wecker.senden(Botschaft::Lokal(neu())).is_ok());
        }
        let liste = post.abholen();
        assert_eq!(liste.len(), 3);
        for b in &liste {
            let Botschaft::Lokal(n) = b;
            assert_eq!(n.wer, Wer::unix("linux", 7, "anna".to_owned()));
        }
        let mut eins = [0_u8; 1];
        let leer = (post.strom()).read(&mut eins);
        assert!(
            leer.is_err_and(|e| e.kind() == ErrorKind::WouldBlock),
            "the bytes are read away"
        );
        assert!(post.abholen().is_empty());
    }

    /// Each message writes a byte, so a wait on the read end wakes.
    #[test]
    fn senden_weckt() {
        let (wecker, post, _schreib) = paar().expect("pair");
        assert!(wecker.senden(Botschaft::Lokal(neu())).is_ok());
        let mut eins = [0_u8; 1];
        assert_eq!((post.strom()).read(&mut eins).ok(), Some(1));
        assert_eq!(eins, [1]);
    }

    /// Without the loop the message comes back to the sender.
    #[test]
    fn ohne_schleife_zurueck() {
        let (wecker, post, _schreib) = paar().expect("pair");
        drop(post);
        assert!(wecker.senden(Botschaft::Lokal(neu())).is_err());
    }
}
