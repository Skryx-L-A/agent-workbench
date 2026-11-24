//! The report of `lauf` to the service (contract `dienst`, section 18): a Unix stream socket `meldung.sock` in the folder of the session,
//! one connection, one line `<code> <grund>\n`.
//!
//! Why a socket and no inherited descriptor: the carrier closes every descriptor above 2 in the child before the `exec`, and `unsafe`
//! is forbidden here, so a pipe cannot reach `lauf`. `lauf` finds the socket through the path in its argument `--meldung <pfad>`.
//!
//! SECURITY: the service accepts exactly one connection, and only one whose peer is this user and, under Linux, the child of the session
//! (`SO_PEERCRED`: user and pid of the process that called `connect`). Every other connection is closed at once and never read. It reads at
//! most [`ZEILE_MAX`] bytes without blocking. Only the exact form of the line counts; anything else is [`GRUND_UNGUELTIG`] (fail closed).
//! Where the system gives no peer credentials without `unsafe` (every system but Linux in `rustix` 1.1.6) only the mode of the socket
//! (0600) and of the folder of the session (0700) keep others out; the contract names this as a limit.

use std::fs;
use std::io::Read;
use std::os::fd::OwnedFd;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;

use crate::dienst::{Annahme, Gelesen, annahme, gelesen};
use crate::startdatei::SOCKET_PFAD_MAX;

/// The name of the socket in the folder of the session.
pub(crate) const SOCKET_NAME: &str = "meldung.sock";
/// The most bytes the service reads from the connection.
pub(crate) const ZEILE_MAX: usize = 128;
/// The reason the list shows when the line is not exactly the form.
pub(crate) const GRUND_UNGUELTIG: &str = "lauf.meldung_ungueltig";
/// The code of a session whose line is not exactly the form (`EX_SOFTWARE`): fixed and never 0, so that a failed start never looks like a
/// success, whatever code the child ended with (decision of 22.11.2026).
pub(crate) const CODE_UNGUELTIG: u32 = 70;
/// The longest reason of a line.
const GRUND_MAX: usize = 64;
/// The backlog of the socket: one connection counts, a second is closed at once.
const WARTESCHLANGE: i32 = 4;

/// What the buffer says so far.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Zeile {
    /// Nothing came.
    Leer,
    /// Bytes came, but no line end yet and fewer than [`ZEILE_MAX`].
    Unvollstaendig,
    /// Exactly `<code> <grund>\n`.
    Gueltig { code: u8, grund: String },
    /// Anything else.
    Ungueltig,
}

/// A decimal whole number from 1 to 255 in its plain form: digits only, no sign, no leading zero.
fn code_lesen(text: &[u8]) -> Option<u8> {
    // `parse` refuses the empty text and every number above 255; a first `0` also refuses the code 0 and every leading zero
    if text.first() == Some(&b'0') || !text.iter().all(u8::is_ascii_digit) {
        return None;
    }
    std::str::from_utf8(text).ok()?.parse::<u8>().ok()
}

/// 1 to 64 characters of `[a-z0-9_.]`.
fn grund_gut(text: &[u8]) -> bool {
    (1..=GRUND_MAX).contains(&text.len())
        && text
            .iter()
            .all(|b| matches!(b, b'a'..=b'z' | b'0'..=b'9' | b'_' | b'.'))
}

/// Judges what was read (at most [`ZEILE_MAX`] bytes): only `<code> <grund>\n` with nothing after it is valid.
pub(crate) fn auswerten(puffer: &[u8]) -> Zeile {
    if puffer.is_empty() {
        return Zeile::Leer;
    }
    let Some(ende) = puffer.iter().position(|b| *b == b'\n') else {
        return if puffer.len() >= ZEILE_MAX {
            Zeile::Ungueltig
        } else {
            Zeile::Unvollstaendig
        };
    };
    if ende + 1 != puffer.len() {
        return Zeile::Ungueltig;
    }
    let zeile = &puffer[..ende];
    let Some(leer) = zeile.iter().position(|b| *b == b' ') else {
        return Zeile::Ungueltig;
    };
    let (code, grund) = (&zeile[..leer], &zeile[leer + 1..]);
    match (code_lesen(code), grund_gut(grund)) {
        (Some(code), true) => Zeile::Gueltig {
            code,
            grund: String::from_utf8_lossy(grund).into_owned(),
        },
        _ => Zeile::Ungueltig,
    }
}

/// What the report means for the end of the session (contract 18, point 4).
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Ende {
    /// No connection until the end of the child: `lauf` failed before the `connect`; the code of the end is its own.
    Keine,
    /// The connection ended or stayed silent: the starter runs; the code of the end belongs to the starter or the harness.
    Starter,
    /// A valid line: the session ends with this code and this reason.
    Gemeldet { code: u8, grund: String },
    /// Anything else: a failed start with the fixed code [`CODE_UNGUELTIG`] and the reason [`GRUND_UNGUELTIG`].
    Ungueltig,
}

/// The socket of one session and what came on it.
pub(crate) struct Meldung {
    /// The listening socket until the one connection is accepted (non-blocking).
    hoerer: Option<UnixListener>,
    /// The accepted connection (non-blocking); `None` before it comes and after its end.
    strom: Option<UnixStream>,
    /// A connection that passed the check was accepted (it may be ended since).
    verbunden: bool,
    /// What was read, at most [`ZEILE_MAX`] bytes.
    puffer: Vec<u8>,
    /// The pid of the child of the session; set after its start. 0 matches no peer.
    pid: u32,
    /// The effective user of the service.
    euid: u32,
}

impl Meldung {
    /// Binds the socket `pfad`, sets its mode 0600 before `listen` and listens (nobody can connect with another mode).
    ///
    /// # Errors
    /// The path is longer than [`SOCKET_PFAD_MAX`] bytes (too long for `sun_path`), or the socket cannot be made, bound, set or listened on. The caller refuses the start and removes
    /// the folder of the session, with the socket in it.
    pub(crate) fn binden(pfad: &Path) -> Result<Self, ()> {
        use rustix::net::{SocketAddrUnix, SocketType, bind, listen};
        // the same limit as the socket of the service (the `sun_path` of the Mac): a path that fits under Linux only is refused too
        if pfad.as_os_str().len() > SOCKET_PFAD_MAX {
            return Err(());
        }
        let fd: OwnedFd = crate::rechte::unix_socket(SocketType::STREAM).map_err(|_| ())?;
        let adresse = SocketAddrUnix::new(pfad).map_err(|_| ())?;
        bind(&fd, &adresse).map_err(|_| ())?;
        fs::set_permissions(pfad, fs::Permissions::from_mode(0o600)).map_err(|_| ())?;
        listen(&fd, WARTESCHLANGE).map_err(|_| ())?;
        let hoerer = UnixListener::from(fd);
        hoerer.set_nonblocking(true).map_err(|_| ())?;
        Ok(Self {
            hoerer: Some(hoerer),
            strom: None,
            verbunden: false,
            puffer: Vec::new(),
            pid: 0,
            euid: rustix::process::geteuid().as_raw(),
        })
    }

    /// Names the child whose connection is the right one (0: none).
    pub(crate) fn kind_setzen(&mut self, pid: u32) {
        self.pid = pid;
    }

    /// `true` if the peer of `strom` may report: this user and, under Linux, the child of the session.
    fn gegenseite_gut(&self, strom: &UnixStream) -> bool {
        #[cfg(target_os = "linux")]
        {
            let Ok(cred) = rustix::net::sockopt::socket_peercred(strom) else {
                return false;
            };
            cred.uid.as_raw() == self.euid
                && u32::try_from(cred.pid.as_raw_nonzero().get()).is_ok_and(|p| p == self.pid)
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = (strom, self.euid, self.pid);
            true
        }
    }

    /// Accepts what waits and reads what came, without blocking: the first connection that passes the check is the one; the
    /// listener is closed then, so no second connection comes in. Every other connection is closed at once, unread.
    pub(crate) fn abfragen(&mut self) {
        self.annehmen();
        self.lesen();
    }

    fn annehmen(&mut self) {
        while !self.verbunden {
            let Some(hoerer) = &self.hoerer else {
                return;
            };
            match annahme(hoerer.accept().map(|(strom, _)| strom)) {
                Annahme::Neu(strom) => {
                    // a connection that fails the check is dropped here: closed, never read
                    let liest_ohne_warten = strom.set_nonblocking(true).is_ok();
                    if self.gegenseite_gut(&strom) && liest_ohne_warten {
                        self.strom = Some(strom);
                        self.verbunden = true;
                        self.hoerer = None;
                    }
                }
                Annahme::Nochmal => {}
                Annahme::Keine => return,
            }
        }
    }

    /// Reads up to [`ZEILE_MAX`] bytes in all. The end of the connection closes it (`strom` is `None` then); what was read stays.
    fn lesen(&mut self) {
        while self.puffer.len() < ZEILE_MAX {
            let Some(strom) = self.strom.as_mut() else {
                return;
            };
            let mut stueck = [0_u8; ZEILE_MAX];
            let frei = ZEILE_MAX - self.puffer.len();
            match gelesen(strom.read(&mut stueck[..frei])) {
                Gelesen::Daten(n) => self.puffer.extend_from_slice(&stueck[..n]),
                Gelesen::Ende => self.strom = None,
                Gelesen::Nochmal => return,
            }
        }
    }

    /// The end of the child has come: the socket is read empty first, then the end is judged. A line that `lauf` wrote just before its
    /// end is not lost this way.
    pub(crate) fn auswerten_am_ende(&mut self) -> Ende {
        self.abfragen();
        if !self.verbunden {
            return Ende::Keine;
        }
        match auswerten(&self.puffer) {
            Zeile::Leer => Ende::Starter,
            Zeile::Gueltig { code, grund } => Ende::Gemeldet { code, grund },
            Zeile::Unvollstaendig | Zeile::Ungueltig => Ende::Ungueltig,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn g(code: u8, grund: &str) -> Zeile {
        Zeile::Gueltig {
            code,
            grund: grund.to_owned(),
        }
    }

    #[test]
    fn die_gueltige_zeile() {
        assert_eq!(
            auswerten(b"69 lauf.harness_fehlt\n"),
            g(69, "lauf.harness_fehlt")
        );
        assert_eq!(auswerten(b"1 a\n"), g(1, "a"));
        assert_eq!(auswerten(b"255 z9_.\n"), g(255, "z9_."));
        let lang = format!("7 {}\n", "a".repeat(64));
        assert_eq!(auswerten(lang.as_bytes()), g(7, &"a".repeat(64)));
    }

    #[test]
    fn code_nur_1_bis_255_in_einfacher_form() {
        for schlecht in [
            &b"0 a\n"[..],
            b"256 a\n",
            b"1000 a\n",
            b"-1 a\n",
            b"+5 a\n",
            b"007 a\n",
            b" 5 a\n",
            b"5a a\n",
            b" a\n",
            b"a a\n",
        ] {
            assert_eq!(auswerten(schlecht), Zeile::Ungueltig, "{schlecht:?}");
        }
    }

    #[test]
    fn grund_nur_aus_dem_zeichensatz_und_bis_64() {
        for schlecht in [
            &b"5 \n"[..],
            b"5 A\n",
            b"5 a-b\n",
            b"5 a b\n",
            b"5 a\x00\n",
            b"5 \xc3\xa4\n",
            b"5  a\n",
        ] {
            assert_eq!(auswerten(schlecht), Zeile::Ungueltig, "{schlecht:?}");
        }
        let zu_lang = format!("7 {}\n", "a".repeat(65));
        assert_eq!(auswerten(zu_lang.as_bytes()), Zeile::Ungueltig);
    }

    #[test]
    fn nichts_vor_dem_ende_und_nichts_danach() {
        assert_eq!(auswerten(b""), Zeile::Leer);
        assert_eq!(auswerten(b"69 lauf"), Zeile::Unvollstaendig);
        assert_eq!(auswerten(b"69 lauf.harness_fehlt\nx"), Zeile::Ungueltig);
        assert_eq!(auswerten(b"69 a\n\n"), Zeile::Ungueltig);
        assert_eq!(auswerten(b"69 a\n69 a\n"), Zeile::Ungueltig);
        assert_eq!(auswerten(b"\n"), Zeile::Ungueltig);
        assert_eq!(auswerten(b"69 a\r\n"), Zeile::Ungueltig);
    }

    #[test]
    fn ohne_zeilenende_gilt_die_grenze_von_128_byte() {
        assert_eq!(auswerten(&[b'a'; 127]), Zeile::Unvollstaendig);
        assert_eq!(auswerten(&[b'a'; 128]), Zeile::Ungueltig);
    }

    /// A throw-away folder of this test only.
    struct Wegwerf(std::path::PathBuf);

    impl Wegwerf {
        fn neu(name: &str) -> Self {
            let pfad =
                std::env::temp_dir().join(format!("dienst-meldung-{}-{name}", std::process::id()));
            let _ = fs::remove_dir_all(&pfad);
            fs::create_dir(&pfad).expect("folder");
            Self(pfad)
        }
    }

    impl Drop for Wegwerf {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn binden_legt_den_socket_mit_0600_an_und_ein_langer_pfad_scheitert() {
        use std::os::unix::fs::FileTypeExt;
        let w = Wegwerf::neu("binden");
        let pfad = w.0.join(SOCKET_NAME);
        let m = Meldung::binden(&pfad).expect("binds");
        let meta = fs::symlink_metadata(&pfad).expect("exists");
        assert!(meta.file_type().is_socket());
        assert_eq!(meta.permissions().mode() & 0o7777, 0o600);
        // the name exists: no second socket, nothing replaced
        assert!(Meldung::binden(&pfad).is_err());
        drop(m);
        // the limit is the one of the socket of the service: 100 bytes bind, 101 do not
        let dir = w.0.as_os_str().len();
        let genau = w.0.join("m".repeat(SOCKET_PFAD_MAX - dir - 1));
        assert_eq!(genau.as_os_str().len(), SOCKET_PFAD_MAX);
        let m = Meldung::binden(&genau).expect("binds at the limit");
        drop(m);
        let zu_lang = w.0.join("m".repeat(SOCKET_PFAD_MAX - dir));
        assert_eq!(zu_lang.as_os_str().len(), SOCKET_PFAD_MAX + 1);
        assert!(Meldung::binden(&zu_lang).is_err());
    }

    /// The child writes its line and ends before the service looks at the socket: the end reads the socket empty first.
    #[cfg(target_os = "linux")]
    #[test]
    fn am_ende_wird_zuerst_gelesen_dann_ausgewertet() {
        let w = Wegwerf::neu("drain");
        let pfad = w.0.join(SOCKET_NAME);
        let mut m = Meldung::binden(&pfad).expect("binds");
        m.kind_setzen(std::process::id());
        let mut c = UnixStream::connect(&pfad).expect("connect");
        std::io::Write::write_all(&mut c, b"69 lauf.harness_fehlt\n").expect("write");
        drop(c);
        assert_eq!(
            m.auswerten_am_ende(),
            Ende::Gemeldet {
                code: 69,
                grund: "lauf.harness_fehlt".to_owned()
            }
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn die_vier_ausgaenge_am_ende() {
        // no connection
        let w = Wegwerf::neu("ausgaenge");
        let mut m = Meldung::binden(&w.0.join("a.sock")).expect("binds");
        m.kind_setzen(std::process::id());
        assert_eq!(m.auswerten_am_ende(), Ende::Keine);
        // a connection without data, closed, and one that stays open and silent: the starter
        for halten in [false, true] {
            let pfad = w.0.join(format!("b{halten}.sock"));
            let mut m = Meldung::binden(&pfad).expect("binds");
            m.kind_setzen(std::process::id());
            let c = UnixStream::connect(&pfad).expect("connect");
            if halten {
                assert_eq!(m.auswerten_am_ende(), Ende::Starter);
                drop(c);
            } else {
                drop(c);
                m.abfragen();
                assert!(m.strom.is_none(), "the end of the connection is seen");
                assert!(m.verbunden);
                assert_eq!(m.auswerten_am_ende(), Ende::Starter);
            }
        }
        // a partial line, and a line with data after it
        for (bytes, soll) in [
            (&b"69 lauf"[..], Ende::Ungueltig),
            (b"69 a\nx", Ende::Ungueltig),
        ] {
            let pfad = w.0.join("c.sock");
            let mut m = Meldung::binden(&pfad).expect("binds");
            m.kind_setzen(std::process::id());
            let mut c = UnixStream::connect(&pfad).expect("connect");
            std::io::Write::write_all(&mut c, bytes).expect("write");
            assert_eq!(m.auswerten_am_ende(), soll, "{bytes:?}");
            let _ = fs::remove_file(&pfad);
        }
    }

    /// The service reads at most [`ZEILE_MAX`] bytes and then no more.
    #[cfg(target_os = "linux")]
    #[test]
    fn nie_mehr_als_128_byte_werden_gelesen() {
        let w = Wegwerf::neu("grenze");
        let pfad = w.0.join(SOCKET_NAME);
        let mut m = Meldung::binden(&pfad).expect("binds");
        m.kind_setzen(std::process::id());
        let mut c = UnixStream::connect(&pfad).expect("connect");
        std::io::Write::write_all(&mut c, &[b'a'; 300]).expect("write");
        m.abfragen();
        assert_eq!(m.puffer.len(), ZEILE_MAX);
        m.abfragen();
        assert_eq!(m.puffer.len(), ZEILE_MAX);
        assert!(
            m.strom.is_some(),
            "the connection stays open after 128 bytes"
        );
        assert_eq!(m.auswerten_am_ende(), Ende::Ungueltig);
    }

    /// The connection of a process that is not the child is closed at once and never read; the right one that comes after it counts.
    #[cfg(target_os = "linux")]
    #[test]
    fn eine_fremde_pid_wird_geschlossen_und_die_richtige_danach_angenommen() {
        let w = Wegwerf::neu("fremd");
        let pfad = w.0.join(SOCKET_NAME);
        let mut m = Meldung::binden(&pfad).expect("binds");
        // another pid than ours: this process is a stranger
        m.kind_setzen(std::process::id().wrapping_add(1));
        let mut fremd = UnixStream::connect(&pfad).expect("connect");
        std::io::Write::write_all(&mut fremd, b"99 fremd\n").expect("write");
        m.abfragen();
        assert!(
            !m.verbunden && m.hoerer.is_some(),
            "still waiting for the child"
        );
        fremd
            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .expect("timeout");
        let mut b = [0_u8; 4];
        assert!(
            matches!(Read::read(&mut fremd, &mut b), Ok(0) | Err(_)),
            "closed by the service"
        );
        assert_eq!(
            m.auswerten_am_ende(),
            Ende::Keine,
            "nothing of the stranger counts"
        );
        // no pid at all matches nobody
        let mut keine = Meldung::binden(&w.0.join("k.sock")).expect("binds");
        keine.kind_setzen(0);
        let _c = UnixStream::connect(w.0.join("k.sock")).expect("connect");
        assert_eq!(keine.auswerten_am_ende(), Ende::Keine);
        // the right pid: accepted, the listener is closed, a later connection finds no listener
        let pfad2 = w.0.join("r.sock");
        let mut m = Meldung::binden(&pfad2).expect("binds");
        m.kind_setzen(std::process::id());
        let _c = UnixStream::connect(&pfad2).expect("connect");
        m.abfragen();
        assert!(m.verbunden && m.strom.is_some() && m.hoerer.is_none());
        assert!(UnixStream::connect(&pfad2).is_err(), "no second connection");
    }
}
