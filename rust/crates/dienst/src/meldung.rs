//! The report of `lauf` to the service (contract `dienst`, section 18): a Unix stream socket `meldung.sock` in the folder of the session,
//! one connection, one line `<code> <grund>\n`.
//!
//! Why a socket and no inherited descriptor: the carrier closes every descriptor above 2 in the child before the `exec`, and `unsafe`
//! is forbidden here, so a pipe cannot reach `lauf`. `lauf` finds the socket through the path in its argument `--meldung <pfad>`.
//!
//! SECURITY: the service accepts exactly one connection, and only one whose peer is this user and, under Linux and macOS, the child of
//! the session (Linux `SO_PEERCRED` through `rustix`, macOS `LOCAL_PEERCRED` and `LOCAL_PEERPID` through `nix`: user and pid of the process
//! that called `connect`). Every other connection is closed at once and never read. It reads at most [`ZEILE_MAX`] bytes without blocking.
//! Only the exact form of the line counts; anything else is [`GRUND_UNGUELTIG`] (fail closed). On any other system there is no measured
//! check of the peer, so the crate does not build there (project decision, 27.11.2026; before, the first connection was taken, fail open).

#[cfg(unix)]
use std::fs;
use std::io::Read;
#[cfg(unix)]
use std::os::fd::OwnedFd;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

use crate::sys::{UnixListener, UnixStream};
use std::path::Path;

use crate::dienst::{Annahme, Gelesen, annahme, gelesen};
#[cfg(unix)]
use crate::startdatei::SOCKET_PFAD_MAX;

// The peer of the report channel is checked only under Linux, macOS and Windows (`Meldung::gegenseite_gut`). Any other system would take
// every peer (fail open), so it gets no build at all (contract `dienst` section 18 point 6 (b), project decision, 27.11.2026).
#[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
compile_error!(
    "wb-dienst: the report channel checks its peer only under Linux, macOS and Windows; this system is not supported"
);

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
#[cfg(unix)]
const WARTESCHLANGE: i32 = 4;

/// Sets the mode 0600 on the name `pfad` without following a link, and checks afterwards with `lstat` that the name is a socket
/// with mode 0600 (N3 of the review of 23.11.2026): a process of the same user that puts a link in place of the socket between
/// `bind` and this call changes no other file, and the start fails.
///
/// Linux: the name is opened with `O_PATH | O_NOFOLLOW` (a link fails with `ELOOP`), the descriptor must be a socket, and the mode
/// is set through `/proc/self/fd/<n>`, which names exactly that inode (the way glibc does `fchmodat` with `AT_SYMLINK_NOFOLLOW`;
/// `rustix` 1.1.6 refuses that flag under Linux). Elsewhere `fchmodat` with `AT_SYMLINK_NOFOLLOW` changes a link itself, never
/// its target.
#[cfg(unix)]
fn modus_setzen(pfad: &Path) -> Result<(), ()> {
    use std::os::unix::fs::FileTypeExt;
    #[cfg(target_os = "linux")]
    {
        use rustix::fs::{Mode, OFlags};
        use std::os::fd::AsRawFd;
        let fd = rustix::fs::open(
            pfad,
            OFlags::PATH | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(|_| ())?;
        let datei = fs::File::from(fd);
        if !datei.metadata().map_err(|_| ())?.file_type().is_socket() {
            return Err(());
        }
        fs::set_permissions(
            format!("/proc/self/fd/{}", datei.as_raw_fd()),
            fs::Permissions::from_mode(0o600),
        )
        .map_err(|_| ())?;
    }
    #[cfg(not(target_os = "linux"))]
    {
        use rustix::fs::{AtFlags, CWD, Mode};
        rustix::fs::chmodat(
            CWD,
            pfad,
            Mode::from_bits_truncate(0o600),
            AtFlags::SYMLINK_NOFOLLOW,
        )
        .map_err(|_| ())?;
    }
    let meta = fs::symlink_metadata(pfad).map_err(|_| ())?;
    if meta.file_type().is_socket() && meta.permissions().mode() & 0o7777 == 0o600 {
        Ok(())
    } else {
        Err(())
    }
}

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
    pub(crate) hoerer: Option<UnixListener>,
    /// The accepted connection (non-blocking); `None` before it comes and after its end.
    pub(crate) strom: Option<UnixStream>,
    /// A connection that passed the check was accepted (it may be ended since).
    pub(crate) verbunden: bool,
    /// What was read, at most [`ZEILE_MAX`] bytes.
    pub(crate) puffer: Vec<u8>,
    /// The pid of the child of the session; set after its start. 0 matches no peer.
    pub(crate) pid: u32,
    /// The effective user of the service.
    pub(crate) euid: u32,
}

impl Meldung {
    /// Binds the socket `pfad`, sets its mode 0600 before `listen` and listens (nobody can connect with another mode).
    ///
    /// # Errors
    /// The path is longer than [`SOCKET_PFAD_MAX`] bytes (too long for `sun_path`), or the socket cannot be made, bound, set or listened on. The caller refuses the start and removes
    /// the folder of the session, with the socket in it.
    #[cfg(unix)]
    pub(crate) fn binden(pfad: &Path) -> Result<Self, ()> {
        use rustix::net::{SocketAddrUnix, SocketType, bind, listen};
        // the same limit as the socket of the service (the `sun_path` of the Mac): a path that fits under Linux only is refused too
        if pfad.as_os_str().len() > SOCKET_PFAD_MAX {
            return Err(());
        }
        let fd: OwnedFd = crate::rechte::unix_socket(SocketType::STREAM).map_err(|_| ())?;
        let adresse = SocketAddrUnix::new(pfad).map_err(|_| ())?;
        bind(&fd, &adresse).map_err(|_| ())?;
        modus_setzen(pfad)?;
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

    /// Windows (section 21): the same socket as `AF_UNIX` with the protected list of the own user alone (the counterpart of 0600)
    /// before `listen` (`sys::meldung_binden`).
    ///
    /// # Errors
    /// The path is longer than [`SOCKET_PFAD_MAX`] bytes, or the socket cannot be made, bound, set or listened on.
    #[cfg(windows)]
    pub(crate) fn binden(pfad: &Path) -> Result<Self, ()> {
        crate::sys::meldung_binden(pfad)
    }

    /// Names the child whose connection is the right one (0: none).
    pub(crate) fn kind_setzen(&mut self, pid: u32) {
        self.pid = pid;
    }

    /// `true` if the peer of `strom` may report: this user and, under Linux and macOS, the child of the session.
    fn gegenseite_gut(&self, strom: &UnixStream) -> bool {
        #[cfg(target_os = "linux")]
        {
            let Ok(cred) = rustix::net::sockopt::socket_peercred(strom) else {
                return false;
            };
            cred.uid.as_raw() == self.euid
                && u32::try_from(cred.pid.as_raw_nonzero().get()).is_ok_and(|p| p == self.pid)
        }
        #[cfg(target_os = "macos")]
        {
            // `LOCAL_PEERCRED` (the effective uid, taken at the `connect`) and `LOCAL_PEERPID` (the pid); `nix` wraps both calls, our
            // code has no `unsafe`. A failed uid refuses the connection (fail closed). The pid exists only while the peer holds its end:
            // when the peer closed before the `accept`, macOS answers `ENOTCONN` (measured, macOS 26.5.1), and only the uid decides
            // then, so that a line `lauf` wrote just before its end is not lost (point 5). Any other error refuses the connection.
            use nix::sys::socket::{getsockopt, sockopt};
            let Ok(cred) = getsockopt(strom, sockopt::LocalPeerCred) else {
                return false;
            };
            if cred.uid() != self.euid {
                return false;
            }
            match getsockopt(strom, sockopt::LocalPeerPid) {
                Ok(pid) => u32::try_from(pid).is_ok_and(|p| p == self.pid),
                Err(nix::errno::Errno::ENOTCONN) => true,
                Err(_) => false,
            }
        }
        // Windows (section 21): the process of the peer must be the child of the session; the user is checked by the list of
        // the socket (only the own user may connect). A failed query refuses the connection (fail closed, `sys::gegenseite_gut`).
        #[cfg(windows)]
        {
            crate::sys::gegenseite_gut(strom, self.pid, self.euid)
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

#[cfg(all(test, unix))]
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

    /// N3: the mode is set on the name itself: a link in place of the socket is refused, and the mode of its target stays as it was.
    #[test]
    fn modus_setzen_folgt_keinem_link() {
        let w = Wegwerf::neu("link");
        let ziel = w.0.join("ziel");
        fs::write(&ziel, b"x").expect("target");
        fs::set_permissions(&ziel, fs::Permissions::from_mode(0o644)).expect("chmod");
        let link = w.0.join(SOCKET_NAME);
        std::os::unix::fs::symlink(&ziel, &link).expect("link");
        assert_eq!(modus_setzen(&link), Err(()));
        let modus = fs::metadata(&ziel).expect("target").permissions().mode() & 0o7777;
        assert_eq!(modus, 0o644, "the target keeps its mode");
        // a plain file is no socket either
        assert_eq!(modus_setzen(&ziel), Err(()));
        assert_eq!(modus_setzen(&w.0.join("fehlt")), Err(()));
    }

    /// A link in place of the report socket that points to another socket: the call fails and the other socket keeps its mode.
    /// Under Linux this needs `O_NOFOLLOW` on the open (mutant `PATH | NOFOLLOW & CLOEXEC`, 25.11.2026).
    #[test]
    fn ein_link_auf_einen_anderen_socket_aendert_dessen_modus_nicht() {
        let w = Wegwerf::neu("link-socket");
        let ziel = w.0.join("anderer.sock");
        let _anderer = UnixListener::bind(&ziel).expect("bind");
        fs::set_permissions(&ziel, fs::Permissions::from_mode(0o644)).expect("chmod");
        let link = w.0.join(SOCKET_NAME);
        std::os::unix::fs::symlink(&ziel, &link).expect("link");
        assert_eq!(modus_setzen(&link), Err(()));
        let modus = fs::symlink_metadata(&ziel)
            .expect("target")
            .permissions()
            .mode()
            & 0o7777;
        assert_eq!(modus, 0o644, "the other socket keeps its mode");
    }

    /// The child writes its line and ends before the service looks at the socket: the end reads the socket empty first.
    #[cfg(any(target_os = "linux", target_os = "macos"))]
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

    #[cfg(any(target_os = "linux", target_os = "macos"))]
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
                // A test running at the same time can hold a copy of `c` between its `fork` and `exec` (CLOEXEC); the end then comes
                // when that copy goes. The service must see it within the deadline, never miss it.
                let frist = std::time::Instant::now() + std::time::Duration::from_secs(10);
                m.abfragen();
                while m.strom.is_some() && std::time::Instant::now() < frist {
                    std::thread::sleep(std::time::Duration::from_millis(10));
                    m.abfragen();
                }
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
    #[cfg(any(target_os = "linux", target_os = "macos"))]
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
    #[cfg(any(target_os = "linux", target_os = "macos"))]
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
        // macOS refuses the timeout with EINVAL on a socket whose peer has closed already; the read below then ends at once
        if let Err(e) = fremd.set_read_timeout(Some(std::time::Duration::from_secs(5))) {
            assert!(
                cfg!(target_os = "macos") && e.raw_os_error() == Some(22),
                "timeout: {e}"
            );
        }
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
        // a second connection finds no listener. A test running at the same time can hold a copy of the listener between its `fork` and
        // `exec` (the socket is CLOEXEC, the copy goes at `exec`); a connection that comes in then must end without being served, never wait.
        if let Ok(mut zweite) = UnixStream::connect(&pfad2) {
            match zweite.set_read_timeout(Some(std::time::Duration::from_secs(10))) {
                Ok(()) => {}
                Err(e) => assert!(
                    cfg!(target_os = "macos") && e.raw_os_error() == Some(22),
                    "timeout: {e}"
                ),
            }
            let mut b = [0_u8; 1];
            match Read::read(&mut zweite, &mut b) {
                Ok(0) => {}
                Err(e) => assert!(
                    !matches!(
                        e.kind(),
                        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                    ),
                    "no second connection: it waited ({e})"
                ),
                Ok(_) => panic!("no second connection: it was served"),
            }
        }
    }
    /// A real child (another process than this test) connects and holds its end: its connection counts, the one of this test (the
    /// same user, another pid) is closed first. Another uid than the one of the service is closed too.
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn die_verbindung_des_kindes_zaehlt_eine_andere_pid_und_eine_andere_uid_nicht() {
        use std::process::{Command, Stdio};
        let w = Wegwerf::neu("kind");
        let pfad = w.0.join(SOCKET_NAME);
        // another uid: the effective user of the service is set to one that is not ours
        let mut m = Meldung::binden(&pfad).expect("binds");
        m.kind_setzen(std::process::id());
        m.euid = m.euid.wrapping_add(1);
        let _c = UnixStream::connect(&pfad).expect("connect");
        m.abfragen();
        assert!(!m.verbunden && m.hoerer.is_some(), "another uid is closed");
        drop(m);
        let _ = fs::remove_file(&pfad);
        // the child: connects, writes its line, holds its end until its stdin ends
        let mut m = Meldung::binden(&pfad).expect("binds");
        let skript = "import socket, sys\ns = socket.socket(socket.AF_UNIX)\ns.connect(sys.argv[1])\n\
                      s.sendall(b'69 lauf.kind\\n')\nprint('verbunden', flush=True)\nsys.stdin.read()\n";
        let mut kind = Command::new("/usr/bin/python3")
            .args(["-I", "-c", skript])
            .arg(&pfad)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .expect("python3");
        m.kind_setzen(kind.id());
        // this test connects first: the same user, but not the child
        let _fremd = UnixStream::connect(&pfad).expect("connect");
        m.abfragen();
        assert!(
            !m.verbunden && m.hoerer.is_some(),
            "the test is not the child"
        );
        let mut zeile = String::new();
        std::io::BufRead::read_line(
            &mut std::io::BufReader::new(kind.stdout.take().expect("stdout")),
            &mut zeile,
        )
        .expect("read");
        assert_eq!(zeile, "verbunden\n");
        let ende = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while m.puffer.len() < 13 && std::time::Instant::now() < ende {
            m.abfragen();
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert!(m.verbunden && m.hoerer.is_none(), "the child is accepted");
        drop(kind.stdin.take());
        assert!(kind.wait().expect("wait").success());
        assert_eq!(
            m.auswerten_am_ende(),
            Ende::Gemeldet {
                code: 69,
                grund: "lauf.kind".to_owned()
            }
        );
    }

    /// Contract `dienst` section 18 point 6 (b), project decision, 27.11.2026: no branch for a system outside Linux, macOS and Windows may
    /// let a check pass. Every `cfg(not(any(…)))` in the sources of the crate is read with its next lines; a `true` or an `Ok(())` there
    /// is a check that passes where nothing was measured. The report channel itself refuses to build on such a system.
    #[test]
    fn kein_zweig_fuer_fremde_systeme_laesst_durch() {
        fn dateien(ordner: &Path, liste: &mut Vec<std::path::PathBuf>) {
            for eintrag in fs::read_dir(ordner).expect("src is readable") {
                let pfad = eintrag.expect("entry").path();
                if pfad.is_dir() {
                    dateien(&pfad, liste);
                } else if pfad.extension().is_some_and(|e| e == "rs") {
                    liste.push(pfad);
                }
            }
        }
        let mut liste = Vec::new();
        dateien(
            &Path::new(env!("CARGO_MANIFEST_DIR")).join("src"),
            &mut liste,
        );
        assert!(liste.len() > 10, "the sources were found");
        let mut funde = Vec::new();
        for pfad in &liste {
            let text = fs::read_to_string(pfad).expect("source is readable");
            let zeilen: Vec<&str> = text.lines().collect();
            for (i, zeile) in zeilen.iter().enumerate() {
                if !zeile.contains("cfg(not(any(") || zeile.trim_start().starts_with("//") {
                    continue;
                }
                for folgende in zeilen.iter().skip(i + 1).take(6) {
                    let t = folgende.trim();
                    if t == "true"
                        || t == "Ok(())"
                        || t.ends_with("=> true,")
                        || t == "return true;"
                    {
                        funde.push(format!("{}:{}", pfad.display(), i + 1));
                    }
                }
            }
        }
        assert!(
            funde.is_empty(),
            "a branch for another system lets a check pass: {funde:?}"
        );
        let eigen = include_str!("meldung.rs");
        let sperre = "#[cfg(not(any(target_os = \"linux\", target_os = \"macos\", windows)))]\ncompile_error!(";
        assert!(
            eigen.contains(sperre),
            "the report channel refuses to build outside Linux, macOS and Windows"
        );
    }
}
