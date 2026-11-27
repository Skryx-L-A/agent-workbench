//! Reading the request (contract 15.12.2, H6 and H7): until the client shuts its write side, at most 1048577 bytes, and
//! only until the deadline of 2 s after the accept.

use std::io::{self, Read};
use std::os::unix::net::UnixStream;
use std::time::{Duration, Instant};

use crate::helfer::{GRENZE_ANFRAGE, Lesefehler};

/// Deadline of the request after the accept.
pub const FRIST: Duration = Duration::from_secs(2);

/// A source of bytes with a wait limit per read.
pub trait Quelle {
    /// Reads into `puffer`, waiting at most `rest`.
    ///
    /// # Errors
    /// The error of the system; `WouldBlock` or `TimedOut` when the wait ran out.
    fn lesen_hoechstens(&mut self, puffer: &mut [u8], rest: Duration) -> io::Result<usize>;
}

impl Quelle for UnixStream {
    fn lesen_hoechstens(&mut self, puffer: &mut [u8], rest: Duration) -> io::Result<usize> {
        self.set_read_timeout(Some(rest))?;
        self.read(puffer)
    }
}

/// Reads the whole request from `q` before `frist`; `jetzt` is the clock.
///
/// # Errors
/// [`Lesefehler::ZuGross`] after more than [`GRENZE_ANFRAGE`] bytes (it reads no further), [`Lesefehler::Frist`] when
/// the deadline passes first, [`Lesefehler::Abbruch`] on any other error.
pub fn anfrage_lesen<Q: Quelle>(
    q: &mut Q,
    frist: Instant,
    jetzt: impl Fn() -> Instant,
) -> Result<Vec<u8>, Lesefehler> {
    let mut aus = Vec::new();
    let mut puffer = vec![0u8; 65536];
    loop {
        let nun = jetzt();
        if nun >= frist {
            return Err(Lesefehler::Frist);
        }
        let platz = (GRENZE_ANFRAGE + 1 - aus.len()).min(puffer.len());
        match q.lesen_hoechstens(&mut puffer[..platz], frist - nun) {
            Ok(0) => return Ok(aus),
            Ok(n) => {
                aus.extend_from_slice(&puffer[..n]);
                if aus.len() > GRENZE_ANFRAGE {
                    return Err(Lesefehler::ZuGross);
                }
            }
            Err(e)
                if matches!(
                    e.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                ) =>
            {
                return Err(Lesefehler::Frist);
            }
            Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
            Err(_) => return Err(Lesefehler::Abbruch),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::net::Shutdown;

    struct Stuecke(Vec<io::Result<Vec<u8>>>, usize);

    impl Quelle for Stuecke {
        fn lesen_hoechstens(&mut self, puffer: &mut [u8], _rest: Duration) -> io::Result<usize> {
            self.1 = self.1.max(puffer.len());
            if self.0.is_empty() {
                return Ok(0);
            }
            let mut s = self.0.remove(0)?;
            let n = s.len().min(puffer.len());
            puffer[..n].copy_from_slice(&s[..n]);
            if n < s.len() {
                self.0.insert(0, Ok(s.split_off(n)));
            }
            Ok(n)
        }
    }

    #[test]
    fn bis_zum_ende_der_schreibseite() {
        let (mut a, mut b) = UnixStream::pair().expect("pair");
        a.write_all(b"{\"x\":").expect("write");
        a.write_all(b"1}").expect("write");
        a.shutdown(Shutdown::Write).expect("shutdown");
        let frist = Instant::now() + FRIST;
        assert_eq!(
            anfrage_lesen(&mut b, frist, Instant::now),
            Ok(b"{\"x\":1}".to_vec())
        );
    }

    #[test]
    fn frist_ohne_ende() {
        let (mut a, mut b) = UnixStream::pair().expect("pair");
        a.write_all(b"{").expect("write");
        let start = Instant::now();
        let frist = start + Duration::from_millis(150);
        assert_eq!(
            anfrage_lesen(&mut b, frist, Instant::now),
            Err(Lesefehler::Frist)
        );
        let dauer = start.elapsed();
        assert!(dauer >= Duration::from_millis(150), "{dauer:?}");
        assert!(dauer < Duration::from_secs(2), "{dauer:?}");
        // a deadline already passed reads nothing
        assert_eq!(
            anfrage_lesen(&mut b, start, Instant::now),
            Err(Lesefehler::Frist)
        );
        assert_eq!(FRIST, Duration::from_secs(2));
    }

    #[test]
    fn grenze_genau() {
        let mut q = Stuecke(vec![Ok(vec![b' '; GRENZE_ANFRAGE])], 0);
        let frist = Instant::now() + FRIST;
        assert_eq!(
            anfrage_lesen(&mut q, frist, Instant::now).map(|v| v.len()),
            Ok(GRENZE_ANFRAGE)
        );
        let mut q = Stuecke((0..17).map(|_| Ok(vec![b' '; 65536])).collect(), 0);
        assert_eq!(
            anfrage_lesen(&mut q, frist, Instant::now),
            Err(Lesefehler::ZuGross)
        );
        assert_eq!(
            q.0.len(),
            1,
            "the 17th piece gives one byte, the rest stays unread"
        );
        assert_eq!(q.0[0].as_ref().map(Vec::len).ok(), Some(65535));
        assert_eq!(q.1, 65536, "no read larger than the buffer");
        let mut q = Stuecke(
            vec![
                Ok(vec![b' '; GRENZE_ANFRAGE]),
                Ok(vec![b' '; 1]),
                Ok(vec![b' '; 1]),
            ],
            0,
        );
        assert_eq!(
            anfrage_lesen(&mut q, frist, Instant::now),
            Err(Lesefehler::ZuGross)
        );
        assert_eq!(
            q.0.len(),
            1,
            "it reads no further than one byte over the limit"
        );
    }

    #[test]
    fn fehler_der_quelle() {
        let frist = Instant::now() + FRIST;
        let mut q = Stuecke(
            vec![Err(io::ErrorKind::Interrupted.into()), Ok(b"{}".to_vec())],
            0,
        );
        assert_eq!(
            anfrage_lesen(&mut q, frist, Instant::now),
            Ok(b"{}".to_vec())
        );
        let mut q = Stuecke(vec![Err(io::ErrorKind::WouldBlock.into())], 0);
        assert_eq!(
            anfrage_lesen(&mut q, frist, Instant::now),
            Err(Lesefehler::Frist)
        );
        let mut q = Stuecke(vec![Err(io::ErrorKind::TimedOut.into())], 0);
        assert_eq!(
            anfrage_lesen(&mut q, frist, Instant::now),
            Err(Lesefehler::Frist)
        );
        let mut q = Stuecke(vec![Err(io::ErrorKind::ConnectionReset.into())], 0);
        assert_eq!(
            anfrage_lesen(&mut q, frist, Instant::now),
            Err(Lesefehler::Abbruch)
        );
    }
}
