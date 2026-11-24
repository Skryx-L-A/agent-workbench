//! One viewer: a connection with its queue of frames, its extract (`rasterauszug`) and what the service remembers of it.

use std::collections::{BTreeMap, VecDeque};
use std::io::{ErrorKind, Write};
use std::net::Shutdown;
use std::os::unix::net::UnixStream;

use rasterauszug::Verbindung;

use crate::rahmen::Eingang;

/// A connection that the service accepted (DI4). The name is `b<nr>`; `nr` counts from 1 in the order of accepting and is
/// never given again.
pub(crate) struct Betrachter {
    /// The number in the name.
    pub(crate) nr: u64,
    /// The name `b<nr>`, as the core knows the viewer.
    pub(crate) name: String,
    /// The socket of the loop (non-blocking): the loop reads after `poll`, writes ([`Betrachter::schreiben`]) and shuts the
    /// connection down.
    pub(crate) strom: UnixStream,
    /// What was read and is not a whole frame yet.
    pub(crate) eingang: Eingang,
    /// The frames the service took for this viewer and the socket has not taken yet, oldest first (DI5).
    pub(crate) schlange: VecDeque<Vec<u8>>,
    /// How many bytes of the first frame of the queue are written already.
    pub(crate) vorn: usize,
    /// `ausstehend` (DI5): the bytes of the whole frames of the queue, of a partly written frame the rest.
    pub(crate) laenge: usize,
    /// The sender of the extract of this connection: one model picture, one Deflate context (RA).
    pub(crate) raster: Verbindung,
    /// The size of the last accepted `Abo` per session (columns, rows), for the rebuild of the core (DI6).
    pub(crate) abos: BTreeMap<String, (i64, i64)>,
    /// Test mode: the viewer takes nothing, the kernel buffer counts as 0 bytes (DI5).
    pub(crate) stumm: bool,
    /// A protocol error came: the queue is written, then the connection ends, and nothing is read from it any more (DI3).
    pub(crate) schliessen: bool,
    /// The core got `Trennen` for this viewer already (at most once, DI4).
    pub(crate) trennen_gemeldet: bool,
}

impl Betrachter {
    /// A viewer for a socket that was just accepted.
    pub(crate) fn neu(nr: u64, strom: UnixStream, deflate: bool) -> Self {
        Self {
            nr,
            name: format!("b{nr}"),
            strom,
            eingang: Eingang::default(),
            schlange: VecDeque::new(),
            vorn: 0,
            laenge: 0,
            raster: Verbindung::neu(deflate),
            abos: BTreeMap::new(),
            stumm: false,
            schliessen: false,
            trennen_gemeldet: false,
        }
    }

    /// Puts a frame at the end of the queue.
    pub(crate) fn einreihen(&mut self, daten: Vec<u8>) {
        self.laenge += daten.len();
        self.schlange.push_back(daten);
    }

    /// Takes `n` bytes of the first frame of the queue as written.
    pub(crate) fn abgenommen(&mut self, n: usize) {
        self.laenge -= n;
        self.vorn += n;
        if self.schlange.front().is_some_and(|f| f.len() == self.vorn) {
            self.schlange.pop_front();
            self.vorn = 0;
        }
    }

    /// Drops the whole queue.
    pub(crate) fn verwerfen(&mut self) {
        self.schlange.clear();
        self.vorn = 0;
        self.laenge = 0;
    }

    /// Writes what the socket takes now; the rest stays at the front of the queue in order (DI5). A muted viewer gets nothing.
    /// The socket is non-blocking, so the service never blocks here. `false` if the connection is broken.
    pub(crate) fn schreiben(&mut self) -> bool {
        if self.stumm || self.schlange.is_empty() {
            return true;
        }
        loop {
            let Some(vorn) = self.schlange.front() else {
                break true;
            };
            match (&self.strom).write(&vorn[self.vorn..]) {
                Ok(0) => break false,
                Ok(n) => self.abgenommen(n),
                Err(e) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::Interrupted) => {
                    break true;
                }
                Err(_) => break false,
            }
        }
    }

    /// Ends the connection: the peer sees the end.
    pub(crate) fn schliessen_strom(&self) {
        let _ = self.strom.shutdown(Shutdown::Both);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paar() -> Betrachter {
        let (a, _b) = UnixStream::pair().expect("pair");
        Betrachter::neu(7, a, false)
    }

    #[test]
    fn name_aus_nummer() {
        let b = paar();
        assert_eq!((b.nr, b.name.as_str()), (7, "b7"));
        assert!(!b.stumm && !b.schliessen && !b.trennen_gemeldet);
    }

    fn lesen_bis(gegenueber: &mut UnixStream, n: usize) -> Vec<u8> {
        use std::io::Read;
        gegenueber
            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .expect("timeout");
        let mut aus = vec![0; n];
        gegenueber.read_exact(&mut aus).expect("read");
        aus
    }

    #[test]
    fn schreibt_alles_was_der_socket_nimmt() {
        let (a, mut gegen) = UnixStream::pair().expect("pair");
        let mut b = Betrachter::neu(1, a, false);
        b.einreihen(vec![1, 2, 3]);
        b.einreihen(vec![4, 5]);
        assert!(b.schreiben());
        assert_eq!((b.laenge, b.schlange.len()), (0, 0));
        assert_eq!(lesen_bis(&mut gegen, 5), vec![1, 2, 3, 4, 5]);
        // empty queue: nothing to do
        assert!(b.schreiben());
    }

    #[test]
    fn stumm_schreibt_nichts() {
        let (a, mut gegen) = UnixStream::pair().expect("pair");
        let mut b = Betrachter::neu(1, a, false);
        b.stumm = true;
        b.einreihen(vec![9; 7]);
        assert!(b.schreiben());
        assert_eq!((b.laenge, b.schlange.len()), (7, 1));
        b.stumm = false;
        assert!(b.schreiben());
        assert_eq!(b.laenge, 0);
        assert_eq!(lesen_bis(&mut gegen, 7), vec![9; 7]);
    }

    #[test]
    fn blockiert_nie_und_haelt_den_rest_in_ordnung() {
        use std::io::Read;
        let (a, mut gegen) = UnixStream::pair().expect("pair");
        a.set_nonblocking(true)
            .expect("the loop makes the socket non-blocking");
        let mut b = Betrachter::neu(1, a, false);
        // far more than any socket buffer: the first write takes some, the rest waits
        let rahmen: Vec<u8> = (0..4_000_000_u32).map(|i| (i % 251) as u8).collect();
        b.einreihen(rahmen.clone());
        b.einreihen(vec![7; 10]);
        assert!(b.schreiben(), "returns although the peer reads nothing");
        assert!(b.laenge > 0 && b.laenge < rahmen.len() + 10);
        assert!(b.vorn > 0, "a partly written frame keeps its place");
        let geschrieben = rahmen.len() + 10 - b.laenge;
        // the peer reads; more can be written, in the same order
        let mut gelesen = Vec::new();
        let mut runden = 0;
        while gelesen.len() < rahmen.len() + 10 {
            runden += 1;
            assert!(runden < 100_000, "the bytes did not arrive");
            let mut puffer = vec![0; 65536];
            gegen.set_nonblocking(true).expect("nb");
            if let Ok(n) = gegen.read(&mut puffer) {
                gelesen.extend_from_slice(&puffer[..n]);
            }
            assert!(b.schreiben());
        }
        assert!(geschrieben > 0);
        assert_eq!(&gelesen[..rahmen.len()], &rahmen[..]);
        assert_eq!(&gelesen[rahmen.len()..], &[7; 10]);
        assert_eq!(b.laenge, 0);
    }

    #[test]
    fn kaputte_verbindung_meldet_false() {
        let (a, gegen) = UnixStream::pair().expect("pair");
        let mut b = Betrachter::neu(1, a, false);
        drop(gegen);
        // The other end is closed only when the last copy of its descriptor is gone: a test thread that forks a child at this moment
        // (the unit tests of this crate start children) leaves a copy in the child until its `exec`, and a write in that time still
        // goes into the buffer. So this waits for the state "the write fails" and queues one more piece each round (a write that
        // went through leaves the queue empty and says `true`); the deadline is only the bound of that wait.
        let frist = std::time::Instant::now() + std::time::Duration::from_secs(30);
        loop {
            b.einreihen(vec![1; 100]);
            if !b.schreiben() {
                break;
            }
            assert!(
                std::time::Instant::now() < frist,
                "the write to a closed connection never failed"
            );
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }

    #[test]
    fn schliessen_strom_beendet_die_verbindung() {
        use std::io::Read;
        let (a, mut gegen) = UnixStream::pair().expect("pair");
        let b = Betrachter::neu(1, a, false);
        b.schliessen_strom();
        gegen
            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .expect("timeout");
        let mut rest = Vec::new();
        assert_eq!(gegen.read_to_end(&mut rest).expect("eof"), 0);
    }

    #[test]
    fn warteschlange_zaehlt_bytes() {
        let mut b = paar();
        b.einreihen(vec![0; 10]);
        b.einreihen(vec![0; 5]);
        assert_eq!(b.laenge, 15);
        b.abgenommen(4);
        assert_eq!((b.laenge, b.vorn, b.schlange.len()), (11, 4, 2));
        b.abgenommen(6);
        assert_eq!((b.laenge, b.vorn, b.schlange.len()), (5, 0, 1));
        b.abgenommen(5);
        assert_eq!((b.laenge, b.vorn, b.schlange.len()), (0, 0, 0));
        b.einreihen(vec![1; 3]);
        b.abgenommen(1);
        b.verwerfen();
        assert_eq!((b.laenge, b.vorn, b.schlange.len()), (0, 0, 0));
    }
}
