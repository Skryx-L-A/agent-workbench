//! One connection under Linux (contract 15.12.2): measure the peer, run the stages, answer with one JSON object and one
//! line end, close. The deadline of the request starts at the accept.
//!
//! A refusal at H1 to H5 comes before one byte of the request is read. Closing a Unix socket with unread bytes resets the
//! connection, and the client would lose the answer (measured in the throwaway VM on 27.11.2026: `ECONNRESET` for root).
//! So the helper writes the answer, shuts its write side and then drops whatever the client still sends, without
//! reading it as a request, up to the limit and the deadline of the request. Only then it closes.

use std::io::Write;
use std::net::Shutdown;
use std::os::unix::net::UnixStream;
use std::time::Instant;

use crate::helfer::{self, Ergebnis, Orte};
use crate::konten::Konten;
use crate::lesen::{self, FRIST};
use crate::partner;
use crate::sys::Dateien;

/// Serves the accepted `strom` and returns the outcome for the journal line.
pub fn bedienen<D: Dateien, K: Konten>(
    mut strom: UnixStream,
    orte: &Orte,
    d: &D,
    k: &K,
) -> Ergebnis {
    let frist = Instant::now() + FRIST;
    let p = partner::messen(&strom);
    let erg = helfer::bearbeiten(orte, d, k, p, || {
        lesen::anfrage_lesen(&mut strom, frist, Instant::now)
    });
    let _ = strom.set_write_timeout(Some(FRIST));
    let _ = strom.write_all(&erg.antwort());
    let _ = strom.shutdown(Shutdown::Write);
    // the rest of the request is dropped unread as a request, so that the close does not reset the answer
    let _ = lesen::anfrage_lesen(&mut strom, frist, Instant::now);
    let _ = strom.shutdown(Shutdown::Both);
    erg
}
