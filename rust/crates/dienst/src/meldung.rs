//! The report of `lauf` to the service (contract `dienst`, section 18): a Unix stream socket `meldung.sock` in the folder of the session,
//! one connection, one line `<code> <grund>\n`, or the line `laeuft\n` and after it requests of a worker, each answered on the same
//! connection (Nachtrag 28.11.2026, stage 1: every valid request is refused with `stelle_nicht_erreichbar`; [`Meldeweg`]).
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
use crate::stelle::Anforderung;

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

    /// Accepts what waits, without blocking: the first connection that passes the check is the one; the listener is closed then, so
    /// no second connection comes in. Every other connection is closed at once, unread.
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
}

/// The first line of a start that reached the starter (Nachtrag 28.11.2026): exactly these 7 bytes, once, and only as the first line.
pub(crate) const LAEUFT: &[u8] = b"laeuft\n";
/// The most bytes of one line after [`LAEUFT`], with its line end.
pub(crate) const ANFORDERUNG_MAX: usize = 256;
/// The answer to every valid request in stage 1: the channel to the authority is not there yet (fail closed, never `frei`).
pub(crate) const ANTWORT_STELLE: &[u8] = b"abgelehnt stelle_nicht_erreichbar\n";
/// The answer to a valid request while another one is open.
pub(crate) const ANTWORT_OFFEN: &[u8] = b"abgelehnt anfordern_offen\n";
/// The most bytes read after [`LAEUFT`] in one turn of the loop, so that one session cannot hold the loop.
const RUNDE_MAX: usize = 1024;
/// No more is read while this many bytes of answers wait to be written (the peer does not read them).
const AUSGANG_MAX: usize = 512;
/// The most writes of the answers in one turn of the loop.
const SCHREIB_VERSUCHE: usize = 8;
/// The longest field `modell` (the form of contract 1).
const MODELL_MAX: usize = 128;
/// The longest field in the form of an identifier (`rolle`, `harness`, `effort`).
const KENNUNG_MAX: usize = 64;

/// `^[a-z0-9][a-z0-9._-]{0,63}$`.
fn kennung_gut(text: &[u8]) -> bool {
    text.first()
        .is_some_and(|b| matches!(b, b'a'..=b'z' | b'0'..=b'9'))
        && text.len() <= KENNUNG_MAX
        && text
            .iter()
            .all(|b| matches!(b, b'a'..=b'z' | b'0'..=b'9' | b'.' | b'_' | b'-'))
}

/// 1 to 128 characters of `[A-Za-z0-9._:/@+-]`.
fn modell_gut(text: &[u8]) -> bool {
    (1..=MODELL_MAX).contains(&text.len())
        && text.iter().all(|b| {
            b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b':' | b'/' | b'@' | b'+' | b'-')
        })
}

/// `anfordern <rolle> <harness> <modell> <effort>` without its line end: exactly five fields, each separated by exactly one space.
pub(crate) fn anforderung_gut(zeile: &[u8]) -> bool {
    anforderung_lesen(zeile).is_some()
}

/// The fields of a valid request (see [`anforderung_gut`]), else `None`.
pub(crate) fn anforderung_lesen(zeile: &[u8]) -> Option<Anforderung> {
    let felder: Vec<&[u8]> = zeile.split(|b| *b == b' ').collect();
    let [wort, rolle, harness, modell, effort] = felder[..] else {
        return None;
    };
    let gut = wort == b"anfordern"
        && kennung_gut(rolle)
        && kennung_gut(harness)
        && modell_gut(modell)
        && kennung_gut(effort);
    // every field is ASCII after the checks above
    let text = |t: &[u8]| String::from_utf8_lossy(t).into_owned();
    gut.then(|| Anforderung {
        rolle: text(rolle),
        harness: text(harness),
        modell: text(modell),
        effort: text(effort),
    })
}

/// What the result of one write of the answers means.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Geschrieben {
    /// This many bytes (at least one) went out.
    Daten(usize),
    /// Interrupted: write again.
    Nochmal,
    /// The buffer of the peer is full: the rest waits for the next turn.
    Spaeter,
    /// The connection is broken (an error, or no byte taken).
    Ende,
}

pub(crate) fn geschrieben_als(ergebnis: std::io::Result<usize>) -> Geschrieben {
    match ergebnis {
        Ok(0) => Geschrieben::Ende,
        Ok(n) => Geschrieben::Daten(n),
        Err(e) if e.kind() == std::io::ErrorKind::Interrupted => Geschrieben::Nochmal,
        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => Geschrieben::Spaeter,
        Err(_) => Geschrieben::Ende,
    }
}

/// The report channel of one session with what comes after [`LAEUFT`] (Nachtrag 28.11.2026, stage 1): requests of a worker, each
/// answered on the same connection. Before [`LAEUFT`] the first line is judged as before (section 18 point 4).
pub(crate) struct Meldeweg {
    /// The socket and the first line.
    pub(crate) kanal: Meldung,
    /// The first line was exactly [`LAEUFT`]: the starter runs, what follows are requests.
    laeuft: bool,
    /// The line after [`LAEUFT`] read so far, without its line end, fewer than [`ANFORDERUNG_MAX`] bytes.
    zeile: Vec<u8>,
    /// The line read so far passed [`ANFORDERUNG_MAX`]: its rest up to the next line end is dropped (it was counted already).
    ueberlang: bool,
    /// Bytes of answers not yet written.
    ausgang: Vec<u8>,
    /// The bytes of [`Meldeweg::ausgang`] up to the end of the answer to the open request; 0: no request is open.
    offen: usize,
    /// The lines after [`LAEUFT`] that were dropped without an answer; only this number goes to the log, never a line.
    verworfen: u64,
    /// The service has a channel to the authority (`lauf.stelle`, stage 2): a valid request waits for the service instead of the
    /// answer of stage 1.
    ueber_stelle: bool,
    /// The request that waits for the service (stage 2).
    angefordert: Angefordert,
}

/// A valid request of stage 2 between the line and the answer of the service.
#[derive(Debug, PartialEq, Eq)]
enum Angefordert {
    /// None is open, or its answer is written already.
    Nichts,
    /// It came and the service has not taken it yet ([`Meldeweg::anforderung_nehmen`]).
    Neu(Anforderung),
    /// The service took it; its answer has not come yet ([`Meldeweg::antworten`]).
    Wartet,
}

impl Meldeweg {
    /// Binds the socket (see [`Meldung::binden`]).
    ///
    /// # Errors
    /// As [`Meldung::binden`].
    pub(crate) fn binden(pfad: &Path) -> Result<Self, ()> {
        Ok(Self {
            kanal: Meldung::binden(pfad)?,
            laeuft: false,
            zeile: Vec::new(),
            ueberlang: false,
            ausgang: Vec::new(),
            offen: 0,
            verworfen: 0,
            ueber_stelle: false,
            angefordert: Angefordert::Nichts,
        })
    }

    /// Stage 2 (Nachtrag 28.11.2026, Worker-Start durch den Orchestrator): a valid request goes to the service, which asks the
    /// authority and answers with [`Meldeweg::antworten`]. Without it every valid request is refused at once (stage 1).
    pub(crate) fn ueber_stelle_setzen(&mut self) {
        self.ueber_stelle = true;
    }

    /// The valid request that came and was not taken yet; it stays open until [`Meldeweg::antworten`].
    pub(crate) fn anforderung_nehmen(&mut self) -> Option<Anforderung> {
        match std::mem::replace(&mut self.angefordert, Angefordert::Wartet) {
            Angefordert::Neu(a) => Some(a),
            alt => {
                self.angefordert = alt;
                None
            }
        }
    }

    /// The answer of the service to the open request (`frei <sitzung>\n` or `abgelehnt <code>\n`). It is written without blocking; when
    /// the connection is gone it is dropped. Without an open request nothing happens.
    pub(crate) fn antworten(&mut self, zeile: &[u8]) {
        if self.angefordert == Angefordert::Nichts {
            return;
        }
        self.angefordert = Angefordert::Nichts;
        if self.kanal.strom.is_none() {
            return;
        }
        self.ausgang.extend_from_slice(zeile);
        self.offen = self.ausgang.len();
        self.schreiben();
    }

    /// A request is open: its answer is not written yet.
    pub(crate) fn offen(&self) -> bool {
        self.offen > 0 || self.angefordert != Angefordert::Nichts
    }

    /// Names the child whose connection is the right one (0: none).
    pub(crate) fn kind_setzen(&mut self, pid: u32) {
        self.kanal.kind_setzen(pid);
    }

    /// The number of lines after [`LAEUFT`] that were dropped.
    pub(crate) fn verworfen(&self) -> u64 {
        self.verworfen
    }

    /// Accepts what waits, reads what came and writes the answers, without blocking: before [`LAEUFT`] at most [`ZEILE_MAX`] bytes in
    /// all, after it at most [`RUNDE_MAX`] bytes in this turn and nothing while [`AUSGANG_MAX`] bytes of answers wait.
    pub(crate) fn abfragen(&mut self) {
        self.kanal.annehmen();
        if !self.laeuft {
            self.kanal.lesen();
            self.laeuft_pruefen();
        }
        if self.laeuft {
            self.weiterlesen();
        }
        self.schreiben();
    }

    /// The first line is exactly [`LAEUFT`]: from now on requests follow; what came after it in the same read is the start of them.
    fn laeuft_pruefen(&mut self) {
        if self.kanal.puffer.starts_with(LAEUFT) {
            self.laeuft = true;
            let rest = self.kanal.puffer.split_off(LAEUFT.len());
            self.eingang(&rest);
        }
    }

    /// Reads after [`LAEUFT`]: at most [`RUNDE_MAX`] bytes in this turn, in pieces of at most [`ANFORDERUNG_MAX`].
    fn weiterlesen(&mut self) {
        let mut gelesen_runde = 0;
        while gelesen_runde < RUNDE_MAX && self.ausgang.len() < AUSGANG_MAX {
            let Some(strom) = self.kanal.strom.as_mut() else {
                return;
            };
            let mut stueck = [0_u8; ANFORDERUNG_MAX];
            match gelesen(strom.read(&mut stueck)) {
                Gelesen::Daten(n) => {
                    gelesen_runde += n;
                    self.eingang(&stueck[..n]);
                }
                Gelesen::Ende => self.kanal.strom = None,
                Gelesen::Nochmal => return,
            }
        }
    }

    /// Splits what came after [`LAEUFT`] into lines of at most [`ANFORDERUNG_MAX`] bytes with their line end.
    fn eingang(&mut self, bytes: &[u8]) {
        for &b in bytes {
            if self.ueberlang {
                self.ueberlang = b != b'\n';
            } else if b == b'\n' {
                let zeile = std::mem::take(&mut self.zeile);
                self.zeile_behandeln(&zeile);
            } else {
                self.zeile.push(b);
                // the line end would be byte 257: too long
                if self.zeile.len() >= ANFORDERUNG_MAX {
                    self.zeile.clear();
                    self.ueberlang = true;
                    self.verworfen += 1;
                }
            }
        }
    }

    /// One whole line after [`LAEUFT`] (without its line end): a valid request gets its answer, anything else is dropped and counted.
    fn zeile_behandeln(&mut self, zeile: &[u8]) {
        if !anforderung_gut(zeile) {
            self.verworfen += 1;
            return;
        }
        if self.offen() {
            self.ausgang.extend_from_slice(ANTWORT_OFFEN);
            return;
        }
        if self.ueber_stelle {
            // stage 2: the service asks the authority; the request stays open until its answer is written
            // `anforderung_gut` held above, so the fields are there
            self.angefordert =
                anforderung_lesen(zeile).map_or(Angefordert::Wartet, Angefordert::Neu);
            return;
        }
        // stage 1: the answer is known at once; the request stays open until its answer is written at the end of the turn, so a
        // second request that came before that answer is refused
        self.ausgang.extend_from_slice(ANTWORT_STELLE);
        self.offen = self.ausgang.len();
    }

    /// Writes waiting answers without blocking, at most [`SCHREIB_VERSUCHE`] times in this turn. A broken connection ends it; what
    /// waited is dropped.
    fn schreiben(&mut self) {
        // at most this many writes in one turn, so that a write that is interrupted again and again cannot hold the loop
        for _ in 0..SCHREIB_VERSUCHE {
            if self.ausgang.is_empty() {
                return;
            }
            let Some(strom) = self.kanal.strom.as_mut() else {
                self.ausgang.clear();
                self.offen = 0;
                return;
            };
            match geschrieben_als(std::io::Write::write(strom, &self.ausgang)) {
                Geschrieben::Daten(n) => self.geschrieben(n),
                Geschrieben::Nochmal => {}
                Geschrieben::Spaeter => return,
                Geschrieben::Ende => {
                    self.kanal.strom = None;
                    self.ausgang.clear();
                    self.offen = 0;
                    return;
                }
            }
        }
    }

    /// `n` bytes of the answers are written; the open request ends with the last byte of its answer.
    fn geschrieben(&mut self, n: usize) {
        self.ausgang.drain(..n);
        self.offen = self.offen.saturating_sub(n);
    }

    /// The end of the child has come: the socket is read empty first, then the end is judged. A line that `lauf` wrote just before its
    /// end is not lost this way. After [`LAEUFT`] the code of the child counts, without a reason; a line still unfinished is dropped.
    pub(crate) fn auswerten_am_ende(&mut self) -> Ende {
        self.abfragen();
        // the end of the run: a request not taken and an answer not come yet are dropped (stage 2, point 4)
        self.angefordert = Angefordert::Nichts;
        if !self.kanal.verbunden {
            return Ende::Keine;
        }
        if self.laeuft {
            if !self.zeile.is_empty() {
                self.zeile.clear();
                self.verworfen += 1;
            }
            return Ende::Starter;
        }
        match auswerten(&self.kanal.puffer) {
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
        let mut m = Meldeweg::binden(&pfad).expect("binds");
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
        let mut m = Meldeweg::binden(&w.0.join("a.sock")).expect("binds");
        m.kind_setzen(std::process::id());
        assert_eq!(m.auswerten_am_ende(), Ende::Keine);
        // a connection without data, closed, and one that stays open and silent: the starter
        for halten in [false, true] {
            let pfad = w.0.join(format!("b{halten}.sock"));
            let mut m = Meldeweg::binden(&pfad).expect("binds");
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
                while m.kanal.strom.is_some() && std::time::Instant::now() < frist {
                    std::thread::sleep(std::time::Duration::from_millis(10));
                    m.abfragen();
                }
                assert!(m.kanal.strom.is_none(), "the end of the connection is seen");
                assert!(m.kanal.verbunden);
                assert_eq!(m.auswerten_am_ende(), Ende::Starter);
            }
        }
        // a partial line, and a line with data after it
        for (bytes, soll) in [
            (&b"69 lauf"[..], Ende::Ungueltig),
            (b"69 a\nx", Ende::Ungueltig),
        ] {
            let pfad = w.0.join("c.sock");
            let mut m = Meldeweg::binden(&pfad).expect("binds");
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
        let mut m = Meldeweg::binden(&pfad).expect("binds");
        m.kind_setzen(std::process::id());
        let mut c = UnixStream::connect(&pfad).expect("connect");
        std::io::Write::write_all(&mut c, &[b'a'; 300]).expect("write");
        m.abfragen();
        assert_eq!(m.kanal.puffer.len(), ZEILE_MAX);
        m.abfragen();
        assert_eq!(m.kanal.puffer.len(), ZEILE_MAX);
        assert!(
            m.kanal.strom.is_some(),
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
        let mut m = Meldeweg::binden(&pfad).expect("binds");
        // another pid than ours: this process is a stranger
        m.kind_setzen(std::process::id().wrapping_add(1));
        let mut fremd = UnixStream::connect(&pfad).expect("connect");
        std::io::Write::write_all(&mut fremd, b"99 fremd\n").expect("write");
        m.abfragen();
        assert!(
            !m.kanal.verbunden && m.kanal.hoerer.is_some(),
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
        let mut keine = Meldeweg::binden(&w.0.join("k.sock")).expect("binds");
        keine.kind_setzen(0);
        let _c = UnixStream::connect(w.0.join("k.sock")).expect("connect");
        assert_eq!(keine.auswerten_am_ende(), Ende::Keine);
        // the right pid: accepted, the listener is closed, a later connection finds no listener
        let pfad2 = w.0.join("r.sock");
        let mut m = Meldeweg::binden(&pfad2).expect("binds");
        m.kind_setzen(std::process::id());
        let _c = UnixStream::connect(&pfad2).expect("connect");
        m.abfragen();
        assert!(m.kanal.verbunden && m.kanal.strom.is_some() && m.kanal.hoerer.is_none());
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
        let mut m = Meldeweg::binden(&pfad).expect("binds");
        m.kind_setzen(std::process::id());
        m.kanal.euid = m.kanal.euid.wrapping_add(1);
        let _c = UnixStream::connect(&pfad).expect("connect");
        m.abfragen();
        assert!(
            !m.kanal.verbunden && m.kanal.hoerer.is_some(),
            "another uid is closed"
        );
        drop(m);
        let _ = fs::remove_file(&pfad);
        // the child: connects, writes its line, holds its end until its stdin ends
        let mut m = Meldeweg::binden(&pfad).expect("binds");
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
            !m.kanal.verbunden && m.kanal.hoerer.is_some(),
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
        while m.kanal.puffer.len() < 13 && std::time::Instant::now() < ende {
            m.abfragen();
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert!(
            m.kanal.verbunden && m.kanal.hoerer.is_none(),
            "the child is accepted"
        );
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

    /// A report channel without a socket whose connection has come: the first bytes are `puffer`. No answer is written (there is no
    /// connection), so the test sees them in `ausgang`.
    fn weg(puffer: &[u8]) -> Meldeweg {
        let mut m = Meldeweg {
            kanal: Meldung {
                hoerer: None,
                strom: None,
                verbunden: true,
                puffer: puffer.to_vec(),
                pid: 0,
                euid: 0,
            },
            laeuft: false,
            zeile: Vec::new(),
            ueberlang: false,
            ausgang: Vec::new(),
            offen: 0,
            verworfen: 0,
            ueber_stelle: false,
            angefordert: Angefordert::Nichts,
        };
        m.laeuft_pruefen();
        m
    }

    /// A valid request of exactly `laenge` bytes with its line end (from 18 to 334).
    fn anforderung(laenge: usize) -> Vec<u8> {
        // "anfordern " and four spaces and the line end are 14 bytes besides the four fields
        let mut rest = laenge - 14 - 4;
        let mut feld = |max: usize| {
            let n = rest.min(max - 1);
            rest -= n;
            n + 1
        };
        let (m, r, h, e) = (
            feld(MODELL_MAX),
            feld(KENNUNG_MAX),
            feld(KENNUNG_MAX),
            feld(KENNUNG_MAX),
        );
        assert_eq!(rest, 0, "{laenge}");
        let zeile = format!(
            "anfordern {} {} {} {}\n",
            "r".repeat(r),
            "h".repeat(h),
            "m".repeat(m),
            "e".repeat(e)
        );
        assert_eq!(zeile.len(), laenge);
        assert!(anforderung_gut(&zeile.as_bytes()[..laenge - 1]));
        zeile.into_bytes()
    }

    #[test]
    fn die_form_der_anforderung() {
        for gut in [
            &b"anfordern worker pi qwen3:8b medium"[..],
            b"anfordern a b c d",
            b"anfordern 0 9.a_b-c Org/Mo_del-1.5@v2+x:latest z",
        ] {
            assert!(anforderung_gut(gut), "{gut:?}");
        }
        let k64 = format!("anfordern {} h m e", "r".repeat(64));
        assert!(anforderung_gut(k64.as_bytes()));
        let m128 = format!("anfordern r h {} e", "m".repeat(128));
        assert!(anforderung_gut(m128.as_bytes()));
        let k65 = format!("anfordern {} h m e", "r".repeat(65));
        let m129 = format!("anfordern r h {} e", "m".repeat(129));
        let e65 = format!("anfordern r h m {}", "e".repeat(65));
        for schlecht in [
            &b""[..],
            b"anfordern",
            b"anfordern a b c",
            b"anfordern a b c d e",
            b"anfordern  a b c d",
            b"anfordern a b c d ",
            b" anfordern a b c d",
            b"anfordern\ta b c d",
            b"Anfordern a b c d",
            b"anfordernx a b c d",
            b"anfordern A b c d",
            b"anfordern -a b c d",
            b"anfordern .a b c d",
            b"anfordern _a b c d",
            b"anfordern a B c d",
            b"anfordern a b c D",
            b"anfordern a b c d/e",
            b"anfordern a b c! d",
            b"anfordern a b m\xc3\xa4 d",
            b"anfordern a b c d\r",
            b"anfordern a b c\x00 d",
            k65.as_bytes(),
            m129.as_bytes(),
            e65.as_bytes(),
        ] {
            assert!(!anforderung_gut(schlecht), "{schlecht:?}");
        }
    }

    /// `laeuft\n` is exactly 7 bytes; it means the starter runs, and the code of the child counts without a reason.
    #[test]
    fn laeuft_sind_genau_sieben_byte_als_erste_zeile() {
        assert_eq!(LAEUFT.len(), 7);
        let mut m = weg(b"laeuft\n");
        assert!(m.laeuft);
        assert_eq!(m.auswerten_am_ende(), Ende::Starter);
        assert_eq!(m.verworfen(), 0);
        // anything that is not exactly these 7 bytes is the line before it: judged as a code line, so invalid
        for schlecht in [
            &b"laeuft"[..],
            b"laeuf\n",
            b"laeuft \n",
            b" laeuft\n",
            b"Laeuft\n",
            b"laeuft\r\n",
            b"laeuftt\n",
            b"\nlaeuft\n",
        ] {
            let mut m = weg(schlecht);
            assert!(!m.laeuft, "{schlecht:?}");
            assert_eq!(m.auswerten_am_ende(), Ende::Ungueltig, "{schlecht:?}");
        }
    }

    /// Before `laeuft` the code line counts as before: a valid one gives its code, at most 128 bytes, nothing after it; a code line
    /// followed by `laeuft` is a line with data after it.
    #[test]
    fn vor_laeuft_gilt_die_code_zeile() {
        let mut m = weg(b"69 lauf.harness_fehlt\n");
        assert_eq!(
            m.auswerten_am_ende(),
            Ende::Gemeldet {
                code: 69,
                grund: "lauf.harness_fehlt".to_owned()
            }
        );
        for schlecht in [
            &b"69 lauf.harness_fehlt\nlaeuft\n"[..],
            &[b'1'; ZEILE_MAX][..],
            &[b'1'; ZEILE_MAX - 1][..],
        ] {
            let mut m = weg(schlecht);
            assert!(!m.laeuft);
            assert_eq!(m.auswerten_am_ende(), Ende::Ungueltig, "{schlecht:?}");
        }
        // 128 bytes that begin with `laeuft\n`: the rest is the start of a request, read further from the connection
        let mut voll = LAEUFT.to_vec();
        voll.extend_from_slice(
            &anforderung(ZEILE_MAX - LAEUFT.len() + 1)[..ZEILE_MAX - LAEUFT.len()],
        );
        assert_eq!(voll.len(), ZEILE_MAX);
        let mut m = weg(&voll);
        assert!(m.laeuft);
        m.eingang(b"\n");
        assert_eq!(m.ausgang, ANTWORT_STELLE);
    }

    /// A valid request gets `abgelehnt stelle_nicht_erreichbar` at once and never `frei`; 256 bytes with the line end are valid, 257 are
    /// dropped without an answer and counted, and the rest of such a line up to its line end does not count as a line of its own.
    #[test]
    fn eine_anforderung_hat_hoechstens_256_byte() {
        let mut m = weg(b"laeuft\n");
        m.eingang(&anforderung(ANFORDERUNG_MAX));
        assert_eq!(m.ausgang, ANTWORT_STELLE);
        assert!(!m.ausgang.starts_with(b"frei"));
        m.geschrieben(m.ausgang.len());
        m.eingang(&anforderung(ANFORDERUNG_MAX + 1));
        assert_eq!(m.ausgang, [] as [u8; 0], "no answer to 257 bytes");
        assert_eq!(m.verworfen(), 1);
        assert!(!m.ueberlang, "the line end of the long line ends it");
        // a much longer line is one dropped line too, and the next one counts again
        let mut lang = vec![b'a'; 3 * ANFORDERUNG_MAX];
        lang.push(b'\n');
        m.eingang(&lang);
        m.eingang(&anforderung(22));
        assert_eq!(m.verworfen(), 2);
        assert_eq!(m.ausgang, ANTWORT_STELLE);
    }

    /// After `laeuft` a code line, a second `laeuft` and any other line are dropped without an answer and counted; the session is
    /// not ended by them and its code stays the one of the child.
    #[test]
    fn nach_laeuft_wird_jede_andere_zeile_verworfen() {
        let mut m = weg(b"laeuft\n69 lauf.harness_fehlt\nlaeuft\nanfordern a b c\n\n");
        assert_eq!(m.ausgang, [] as [u8; 0]);
        assert_eq!(m.verworfen(), 4);
        assert_eq!(m.auswerten_am_ende(), Ende::Starter);
        assert!(m.kanal.verbunden);
    }

    /// One request is open per run: a second valid one before the answer to the first is written gets `abgelehnt anfordern_offen`;
    /// once the whole answer is written, the next one is answered again.
    #[test]
    fn zwei_anforderungen_vor_der_antwort() {
        let a = anforderung(30);
        let mut m = weg(b"laeuft\n");
        m.eingang(&a);
        m.eingang(&a);
        assert_eq!(m.ausgang, [ANTWORT_STELLE, ANTWORT_OFFEN].concat());
        // the answer to the first is not written completely: still open
        m.geschrieben(ANTWORT_STELLE.len() - 1);
        m.eingang(&a);
        assert_eq!(
            m.ausgang,
            [
                &ANTWORT_STELLE[ANTWORT_STELLE.len() - 1..],
                ANTWORT_OFFEN,
                ANTWORT_OFFEN
            ]
            .concat()
        );
        m.geschrieben(1);
        m.eingang(&a);
        assert!(m.ausgang.ends_with(ANTWORT_STELLE));
        assert_eq!(m.verworfen(), 0);
    }

    /// Stage 2: a valid request waits for the service; until its answer every further valid one is `anfordern_offen` at once. The
    /// answer of the service ends it; an answer without an open request does nothing; the end drops what was not taken.
    #[test]
    fn mit_stelle_wartet_die_anforderung_auf_den_dienst() {
        let mut m = weg(b"laeuft\n");
        m.ueber_stelle_setzen();
        m.eingang(b"anfordern pruefer pi qwen3:8b low\n");
        assert!(m.ausgang.is_empty(), "no answer of stage 1");
        assert!(m.offen());
        m.eingang(b"anfordern zweiter pi m low\n");
        assert_eq!(m.ausgang, ANTWORT_OFFEN);
        let a = m.anforderung_nehmen().expect("the first");
        assert_eq!(
            (
                a.rolle.as_str(),
                a.harness.as_str(),
                a.modell.as_str(),
                a.effort.as_str()
            ),
            ("pruefer", "pi", "qwen3:8b", "low")
        );
        assert_eq!(m.anforderung_nehmen(), None, "taken once");
        m.eingang(b"anfordern dritter pi m low\n");
        assert_eq!(m.ausgang, [ANTWORT_OFFEN, ANTWORT_OFFEN].concat());
        // without a connection the answer is dropped, and the request is no longer open
        m.antworten(b"frei s-2\n");
        assert!(!m.offen());
        assert_eq!(m.ausgang, [ANTWORT_OFFEN, ANTWORT_OFFEN].concat());
        m.antworten(b"frei s-3\n");
        assert_eq!(
            m.ausgang,
            [ANTWORT_OFFEN, ANTWORT_OFFEN].concat(),
            "nothing open"
        );
        // a new request after the answer; the end drops it
        m.eingang(b"anfordern vierter pi m low\n");
        assert!(m.offen());
        assert_eq!(m.auswerten_am_ende(), Ende::Starter);
        assert!(!m.offen());
        assert_eq!(m.anforderung_nehmen(), None);
        assert_eq!(m.verworfen(), 0);
    }

    /// Stage 2 on a real connection: the answer of the service comes on the same connection and ends the open request.
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn mit_stelle_kommt_die_antwort_des_dienstes_auf_derselben_verbindung() {
        use std::io::{BufRead, Write};
        let w = Wegwerf::neu("antwort2");
        let pfad = w.0.join(SOCKET_NAME);
        let mut m = Meldeweg::binden(&pfad).expect("binds");
        m.ueber_stelle_setzen();
        m.kind_setzen(std::process::id());
        let mut c = UnixStream::connect(&pfad).expect("connect");
        c.set_read_timeout(Some(std::time::Duration::from_millis(20)))
            .expect("timeout");
        let mut leser = std::io::BufReader::new(c.try_clone().expect("clone"));
        c.write_all(b"laeuft\nanfordern worker pi qwen3:8b medium\nanfordern zweiter pi m low\n")
            .expect("write");
        let frist = std::time::Instant::now() + std::time::Duration::from_secs(10);
        let mut a = None;
        while a.is_none() && std::time::Instant::now() < frist {
            m.abfragen();
            a = m.anforderung_nehmen();
        }
        assert_eq!(a.map(|a| a.rolle), Some("worker".to_owned()));
        let mut antworten = Vec::new();
        let mut zeile = String::new();
        while antworten.is_empty() && std::time::Instant::now() < frist {
            m.abfragen();
            if leser.read_line(&mut zeile).is_ok_and(|n| n > 0) && zeile.ends_with('\n') {
                antworten.push(std::mem::take(&mut zeile));
            }
        }
        m.antworten(b"frei s-7\n");
        while antworten.len() < 2 && std::time::Instant::now() < frist {
            m.abfragen();
            if leser.read_line(&mut zeile).is_ok_and(|n| n > 0) && zeile.ends_with('\n') {
                antworten.push(std::mem::take(&mut zeile));
            }
        }
        assert_eq!(antworten, ["abgelehnt anfordern_offen\n", "frei s-7\n"]);
        assert!(!m.offen());
    }

    /// EOF after `laeuft` means only that no requests come any more: the code of the child counts without a reason; a line that has
    /// not ended is dropped.
    #[test]
    fn eof_nach_laeuft() {
        let mut m = weg(b"laeuft\nanfordern a b");
        assert_eq!(m.auswerten_am_ende(), Ende::Starter);
        assert_eq!(m.verworfen(), 1);
        let mut m = weg(b"laeuft\n");
        assert_eq!(m.auswerten_am_ende(), Ende::Starter);
        assert_eq!(m.verworfen(), 0);
    }

    /// The service reads at most 1024 bytes after `laeuft` in one turn, in pieces of 256, and nothing while answers of
    /// [`AUSGANG_MAX`] bytes wait.
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn nach_laeuft_wird_je_runde_begrenzt_gelesen() {
        let w = Wegwerf::neu("runde");
        let pfad = w.0.join(SOCKET_NAME);
        let mut m = Meldeweg::binden(&pfad).expect("binds");
        m.kind_setzen(std::process::id());
        let mut c = UnixStream::connect(&pfad).expect("connect");
        std::io::Write::write_all(&mut c, LAEUFT).expect("write");
        let frist = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while !m.laeuft && std::time::Instant::now() < frist {
            m.abfragen();
        }
        assert!(m.laeuft);
        // 256 lines of 16 bytes
        let zeilen = b"xxxxxxxxxxxxxxx\n".repeat(256);
        std::io::Write::write_all(&mut c, &zeilen).expect("write");
        m.ausgang = vec![b'a'; 512];
        m.weiterlesen();
        assert_eq!(m.verworfen(), 0, "nothing is read while answers wait");
        m.ausgang.clear();
        m.weiterlesen();
        // 1024 bytes in one turn: 64 lines of 16 bytes
        assert_eq!(m.verworfen(), 64);
        while m.verworfen() < 256 && std::time::Instant::now() < frist {
            m.abfragen();
        }
        assert_eq!(m.verworfen(), 256);
    }

    /// A peer that does not read its answers: once its buffer is full, the answers wait (`WouldBlock`), the connection stays, and no
    /// more than [`AUSGANG_MAX`] and one turn of answers wait.
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn ein_voller_puffer_der_gegenseite_beendet_die_verbindung_nicht() {
        let w = Wegwerf::neu("voll");
        let pfad = w.0.join(SOCKET_NAME);
        let mut m = Meldeweg::binden(&pfad).expect("binds");
        m.kind_setzen(std::process::id());
        let mut c = UnixStream::connect(&pfad).expect("connect");
        std::io::Write::write_all(&mut c, LAEUFT).expect("write");
        c.set_nonblocking(true).expect("nonblocking");
        let zeilen = b"anfordern a b c d\n".repeat(50);
        let frist = std::time::Instant::now() + std::time::Duration::from_secs(30);
        loop {
            let _ = std::io::Write::write(&mut c, &zeilen);
            m.abfragen();
            assert!(m.kanal.strom.is_some(), "the connection stays");
            if m.laeuft && !m.ausgang.is_empty() {
                break;
            }
            assert!(
                std::time::Instant::now() < frist,
                "the buffer of the peer did not fill"
            );
        }
        for _ in 0..100 {
            let _ = std::io::Write::write(&mut c, &zeilen);
            m.abfragen();
        }
        assert!(m.kanal.strom.is_some(), "the connection stays");
        assert!(m.ausgang.len() < AUSGANG_MAX + RUNDE_MAX * ANTWORT_OFFEN.len());
    }

    /// Writing fails (the peer shut its reading side): the connection ends, the waiting answers are dropped, and the end is the one of
    /// the starter.
    #[cfg(target_os = "linux")]
    #[test]
    fn ein_fehler_beim_schreiben_beendet_die_verbindung() {
        let w = Wegwerf::neu("epipe");
        let pfad = w.0.join(SOCKET_NAME);
        let mut m = Meldeweg::binden(&pfad).expect("binds");
        m.kind_setzen(std::process::id());
        let mut c = UnixStream::connect(&pfad).expect("connect");
        c.shutdown(std::net::Shutdown::Read).expect("shutdown");
        std::io::Write::write_all(&mut c, b"laeuft\nanfordern a b c d\n").expect("write");
        let frist = std::time::Instant::now() + std::time::Duration::from_secs(10);
        // the connection is accepted first, then the write of the answer fails
        while !(m.kanal.verbunden && m.kanal.strom.is_none()) && std::time::Instant::now() < frist {
            m.abfragen();
        }
        assert!(
            m.kanal.verbunden && m.kanal.strom.is_none(),
            "the failed write ends the connection"
        );
        assert_eq!(m.ausgang, [] as [u8; 0]);
        assert_eq!(m.offen, 0);
        assert!(m.laeuft);
        assert_eq!(m.auswerten_am_ende(), Ende::Starter);
        drop(c);
    }

    #[test]
    fn die_bedeutung_eines_schreibens() {
        use std::io::{Error, ErrorKind};
        assert_eq!(geschrieben_als(Ok(3)), Geschrieben::Daten(3));
        assert_eq!(geschrieben_als(Ok(1)), Geschrieben::Daten(1));
        assert_eq!(geschrieben_als(Ok(0)), Geschrieben::Ende);
        assert_eq!(
            geschrieben_als(Err(Error::from(ErrorKind::Interrupted))),
            Geschrieben::Nochmal
        );
        assert_eq!(
            geschrieben_als(Err(Error::from(ErrorKind::WouldBlock))),
            Geschrieben::Spaeter
        );
        assert_eq!(
            geschrieben_als(Err(Error::from(ErrorKind::BrokenPipe))),
            Geschrieben::Ende
        );
    }

    /// Over a real connection: the answer comes on the same connection, the end of the connection ends nothing but the requests.
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn die_antwort_kommt_auf_derselben_verbindung() {
        use std::io::{BufRead, Write};
        let w = Wegwerf::neu("antwort");
        let pfad = w.0.join(SOCKET_NAME);
        let mut m = Meldeweg::binden(&pfad).expect("binds");
        m.kind_setzen(std::process::id());
        let mut c = UnixStream::connect(&pfad).expect("connect");
        c.set_read_timeout(Some(std::time::Duration::from_millis(20)))
            .expect("timeout");
        let mut leser = std::io::BufReader::new(c.try_clone().expect("clone"));
        c.write_all(b"laeuft\nanfordern worker pi qwen3:8b medium\n")
            .expect("write");
        let mut antworten = Vec::new();
        let frist = std::time::Instant::now() + std::time::Duration::from_secs(10);
        let mut zeile = String::new();
        while antworten.is_empty() && std::time::Instant::now() < frist {
            m.abfragen();
            if leser.read_line(&mut zeile).is_ok_and(|n| n > 0) && zeile.ends_with('\n') {
                antworten.push(std::mem::take(&mut zeile));
            }
        }
        assert_eq!(antworten, ["abgelehnt stelle_nicht_erreichbar\n"]);
        // an invalid line gets no answer, the valid one after it does
        c.write_all(b"anfordern Worker pi m e\nanfordern a b c d\n")
            .expect("write");
        while antworten.len() < 2 && std::time::Instant::now() < frist {
            m.abfragen();
            if leser.read_line(&mut zeile).is_ok_and(|n| n > 0) && zeile.ends_with('\n') {
                antworten.push(std::mem::take(&mut zeile));
            }
        }
        assert_eq!(antworten[1], "abgelehnt stelle_nicht_erreichbar\n");
        assert_eq!(m.verworfen(), 1);
        drop(leser);
        drop(c);
        while m.kanal.strom.is_some() && std::time::Instant::now() < frist {
            m.abfragen();
        }
        assert!(m.kanal.strom.is_none(), "the end of the connection is seen");
        assert_eq!(m.auswerten_am_ende(), Ende::Starter);
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
