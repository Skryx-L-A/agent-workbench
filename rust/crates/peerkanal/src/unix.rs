//! The Unix side (Linux and macOS): a stream socket `<ordner>/<name>.sock` whose server measures the uid and gid of each
//! client from the kernel (Linux `SO_PEERCRED`, macOS `LOCAL_PEERCRED` and `LOCAL_PEERPID`), and whose client checks the uid
//! of the server the same way before it sends a byte. No `unsafe`: `rustix` and, under macOS, `nix` wrap the calls.
//!
//! SECURITY, fail closed: when the kernel gives no uid (the call fails, the system has no way, a value is missing), there is
//! no connection: the server refuses with `pk.peer_unmessbar` (`pk.messung_gescheitert`), the client with
//! `pk.server_unmessbar`. There is no path that hands over a connection whose other side was not measured.
//! Not in this stage (open): the pidfd of the peer (Linux `SO_PEERPIDFD`) and the check `pk.prozess_gewechselt`, the audit
//! token, the code signature and the sandbox flag under macOS.

use std::io::{self, ErrorKind, Read, Write};
use std::os::unix::fs::{FileTypeExt, MetadataExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use rustix::fs::{AtFlags, CWD, Mode, chmodat};
use rustix::net::{
    AddressFamily, SocketAddrUnix, SocketFlags, SocketType, bind, listen, socket_with,
};

use crate::ablauf::Prozessbindung;
use crate::fehler::{Code, Ereignis, Fehler, Grund, Peer, System};
use crate::kennung::{Kanalname, Kennung};
use crate::vorgabe::{ClientVorgabe, Erlaubnis, Erwartet, ServerVorgabe};

/// The hello: `pk1` and LF.
const HALLO: &[u8; 4] = b"pk1\n";

/// The bytes `sun_path` holds, the NUL included (macOS 104, Linux 108).
#[cfg(target_os = "macos")]
const SUN_PATH: usize = 104;
#[cfg(not(target_os = "macos"))]
const SUN_PATH: usize = 108;

/// The hook for events, as on the pipe side.
pub(crate) type Haken = Arc<dyn Fn(&Ereignis) + Send + Sync>;

/// What the kernel says about the other end of a connected socket.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Gemessen {
    pub(crate) uid: u32,
    pub(crate) gid: Option<u32>,
    pub(crate) pid: Option<u32>,
}

/// The uid, gid and pid of the other end, or `None` when the kernel does not give the uid (fail closed).
#[cfg(target_os = "linux")]
pub(crate) fn messen(strom: &UnixStream) -> Option<Gemessen> {
    let cred = rustix::net::sockopt::socket_peercred(strom).ok()?;
    Some(Gemessen {
        uid: cred.uid.as_raw(),
        gid: Some(cred.gid.as_raw()),
        pid: u32::try_from(cred.pid.as_raw_nonzero().get()).ok(),
    })
}

/// The uid, gid (the first group of `LOCAL_PEERCRED`) and pid (`LOCAL_PEERPID`) of the other end, or `None` when the kernel
/// does not give the uid (fail closed). The pid may be missing: macOS answers `ENOTCONN` when the peer closed already.
#[cfg(target_os = "macos")]
pub(crate) fn messen(strom: &UnixStream) -> Option<Gemessen> {
    use nix::sys::socket::{getsockopt, sockopt};
    let cred = getsockopt(strom, sockopt::LocalPeerCred).ok()?;
    let pid = getsockopt(strom, sockopt::LocalPeerPid)
        .ok()
        .and_then(|p| u32::try_from(p).ok());
    Some(Gemessen {
        uid: cred.uid(),
        gid: cred.groups().first().copied(),
        pid,
    })
}

/// Any other Unix system has no measured way here: no uid, so no connection (fail closed).
#[cfg(not(any(target_os = "linux", target_os = "macos")))]
pub(crate) fn messen(_strom: &UnixStream) -> Option<Gemessen> {
    None
}

fn system() -> System {
    if cfg!(target_os = "macos") {
        System::Macos
    } else {
        System::Linux
    }
}

/// Start time and program of a pid where the system shows them without rights (Linux `/proc`), else `None`.
fn prozessdaten(pid: Option<u32>) -> (Option<u64>, Option<String>) {
    let Some(pid) = pid else {
        return (None, None);
    };
    if !cfg!(target_os = "linux") {
        return (None, None);
    }
    let startzeit = std::fs::read_to_string(format!("/proc/{pid}/stat"))
        .ok()
        .and_then(|s| startzeit_aus_stat(&s));
    let programm = std::fs::read_link(format!("/proc/{pid}/exe"))
        .ok()
        .and_then(|p| p.to_str().map(str::to_owned));
    (startzeit, programm)
}

/// Field 22 of `/proc/<pid>/stat`: counted after the last `)`, which ends the name of the program.
pub(crate) fn startzeit_aus_stat(stat: &str) -> Option<u64> {
    let rest = &stat[stat.rfind(')')? + 1..];
    // after `)` come field 3 (state) and onwards: field 22 is the 20th of them
    rest.split_ascii_whitespace().nth(19)?.parse().ok()
}

fn peer_von(g: Option<Gemessen>) -> Peer {
    let mut peer = Peer::leer(system());
    if let Some(g) = g {
        let (startzeit, programm) = prozessdaten(g.pid);
        peer.uid = Some(g.uid);
        peer.gid = g.gid;
        peer.pid = g.pid;
        peer.startzeit = startzeit;
        peer.programm = programm;
    }
    peer
}

fn pfad_von(ordner: &Path, name: &Kanalname) -> PathBuf {
    ordner.join(format!("{}.sock", name.als_str()))
}

/// The folder passes "Server anlegen" step 2: a folder, no link (`lstat`), of the own effective uid, mode exactly 0700.
pub(crate) fn ordner_gut(ordner: &Path, euid: u32) -> bool {
    std::fs::symlink_metadata(ordner).is_ok_and(|m| {
        m.file_type().is_dir() && m.uid() == euid && m.permissions().mode() & 0o7777 == 0o700
    })
}

/// The path has fewer bytes than `sun_path` holds (the NUL counts).
pub(crate) fn pfad_kurz_genug(pfad: &Path) -> bool {
    pfad.as_os_str().len() < SUN_PATH
}

/// Step 6: the first fitting permission. `pk.uid_nicht_erlaubt` when no permission has the uid, `pk.gid_falsch` when every
/// one with the uid fixes another gid.
pub(crate) fn erlaubt(liste: &[Erlaubnis], uid: u32, gid: Option<u32>) -> Result<(), Grund> {
    let mut mit_uid = false;
    for e in liste {
        if let Erlaubnis::Unix { uid: u, gid: g } = e
            && *u == uid
        {
            mit_uid = true;
            if g.is_none() || *g == gid {
                return Ok(());
            }
        }
    }
    Err(if mit_uid {
        Grund::GidFalsch
    } else {
        Grund::UidNichtErlaubt
    })
}

/// Reads the hello within the deadline: `Ok` for exactly `pk1` LF, `pk.hallo_falsch` as soon as the bytes cannot begin it,
/// `pk.hallo_fehlt` at the end of the connection or the deadline before four fitting bytes.
pub(crate) fn hallo_lesen(strom: &mut UnixStream, frist: Duration) -> Result<(), Grund> {
    let ende = Instant::now() + frist;
    let mut da = 0usize;
    let mut puffer = [0u8; 4];
    while da < HALLO.len() {
        let rest = ende.saturating_duration_since(Instant::now());
        if rest.is_zero() || strom.set_read_timeout(Some(rest)).is_err() {
            return Err(Grund::HalloFehlt);
        }
        // the end of the connection and any error, an interrupted call too, end the wait (fail closed)
        let n = strom.read(&mut puffer[da..]).unwrap_or(0);
        if n == 0 {
            return Err(Grund::HalloFehlt);
        }
        if puffer[da..da + n] != HALLO[da..da + n] {
            return Err(Grund::HalloFalsch);
        }
        da += n;
    }
    Ok(())
}

/// A stream socket that is closed on `exec`: under Linux in one call, under macOS (no `SOCK_CLOEXEC`) right after it.
fn socket(fehler: &dyn Fn(rustix::io::Errno) -> Fehler) -> Result<std::os::fd::OwnedFd, Fehler> {
    #[cfg(target_os = "linux")]
    let fd = socket_with(
        AddressFamily::UNIX,
        SocketType::STREAM,
        SocketFlags::CLOEXEC,
        None,
    )
    .map_err(fehler)?;
    #[cfg(not(target_os = "linux"))]
    let fd = {
        let fd = socket_with(
            AddressFamily::UNIX,
            SocketType::STREAM,
            SocketFlags::empty(),
            None,
        )
        .map_err(fehler)?;
        rustix::io::fcntl_setfd(&fd, rustix::io::FdFlags::CLOEXEC).map_err(fehler)?;
        fd
    };
    Ok(fd)
}

/// The mode bits as the type of the system (`mode_t` has 32 bits under Linux, 16 under macOS).
#[cfg_attr(
    target_os = "linux",
    allow(
        clippy::useless_conversion,
        reason = "mode_t is u32 under Linux, u16 under macOS"
    )
)]
fn modus(bits: u32) -> Option<Mode> {
    Some(Mode::from_raw_mode(bits.try_into().ok()?))
}

/// The server of the Unix side.
pub(crate) struct Server {
    hoerer: UnixListener,
    kanal: Kanalname,
    pfad: PathBuf,
    geraet: u64,
    inode: u64,
    erlaubt: Vec<Erlaubnis>,
    hallo_frist: Duration,
    prozess_pflicht: bool,
    pub(crate) haken: Option<Haken>,
}

impl Server {
    /// "Server anlegen" steps 2 to 5 under Unix (step 1, the check of the settings, is done by the caller).
    pub(crate) fn anlegen(v: ServerVorgabe) -> Result<Self, Fehler> {
        let ort = v.unix.ok_or(Fehler::neu(Code::VorgabeUngueltig))?;
        let euid = rustix::process::geteuid().as_raw();
        if !ordner_gut(&ort.ordner, euid) {
            return Err(Fehler::neu(Code::OrdnerRechte));
        }
        let pfad = pfad_von(&ort.ordner, &v.name);
        if !pfad_kurz_genug(&pfad) {
            return Err(Fehler::neu(Code::PfadZuLang));
        }
        // anything at the path, even an orphaned socket, a file or a (dangling) link: no fallback, nothing is removed. The
        // check uses `lstat`, because under macOS `bind` follows a dangling link instead of failing with `EADDRINUSE`.
        let belegt = std::fs::symlink_metadata(&pfad)
            .map_or_else(|e| e.kind() != ErrorKind::NotFound, |_| true);
        if belegt {
            return Err(Fehler::neu(Code::NameBelegt));
        }
        let fehler =
            |e: rustix::io::Errno| Fehler::os(Code::AnlegenFehlgeschlagen, e.raw_os_error());
        let fd = socket(&fehler)?;
        let adresse = SocketAddrUnix::new(&pfad).map_err(|_| Fehler::neu(Code::PfadZuLang))?;
        // a name taken between the check and `bind` (only the own user can do that, the folder is 0700) fails closed
        bind(&fd, &adresse).map_err(fehler)?;
        // the mode before `listen`: until then no one can connect (the folder is 0700)
        let gebunden = (|| {
            let m = modus(ort.modus.bits()).ok_or(Fehler::neu(Code::AnlegenFehlgeschlagen))?;
            chmodat(CWD, &pfad, m, AtFlags::empty()).map_err(fehler)?;
            listen(&fd, 64).map_err(fehler)?;
            let m = std::fs::symlink_metadata(&pfad)
                .map_err(|_| Fehler::neu(Code::AnlegenFehlgeschlagen))?;
            if !m.file_type().is_socket() {
                return Err(Fehler::neu(Code::AnlegenFehlgeschlagen));
            }
            Ok((m.dev(), m.ino()))
        })();
        let (geraet, inode) = match gebunden {
            Ok(w) => w,
            Err(e) => {
                let _ = std::fs::remove_file(&pfad);
                return Err(e);
            }
        };
        Ok(Self {
            hoerer: UnixListener::from(fd),
            kanal: v.name,
            pfad,
            geraet,
            inode,
            erlaubt: v.erlaubt,
            hallo_frist: v.hallo_frist,
            prozess_pflicht: v.prozess_pflicht,
            haken: None,
        })
    }

    fn abweisen(&self, strom: UnixStream, grund: Grund, peer: Option<Peer>) -> Ereignis {
        let ereignis = Ereignis {
            kanal: self.kanal.clone(),
            grund,
            peer,
        };
        if let Some(haken) = &self.haken {
            haken(&ereignis);
        }
        let _ = strom.shutdown(std::net::Shutdown::Both);
        drop(strom);
        ereignis
    }

    /// "Annehmen und Messen" under Unix: the kernel measures at once after `accept`, then the hello, then the checks.
    pub(crate) fn annehmen(&self) -> Result<Annahme, Fehler> {
        let (strom, _) = self
            .hoerer
            .accept()
            .map_err(|e| Fehler::os(Code::Io, e.raw_os_error().unwrap_or(0)))?;
        let gemessen = messen(&strom);
        Ok(self.pruefen(strom, gemessen))
    }

    /// Steps 3, 4 and 6 for a connection the kernel measured as `gemessen` (`None`: no uid).
    fn pruefen(&self, mut strom: UnixStream, gemessen: Option<Gemessen>) -> Annahme {
        let peer = peer_von(gemessen);
        if let Err(grund) = hallo_lesen(&mut strom, self.hallo_frist) {
            return Annahme::Abgewiesen(self.abweisen(strom, grund, Some(peer)));
        }
        let Some(g) = gemessen else {
            return Annahme::Abgewiesen(self.abweisen(
                strom,
                Grund::MessungGescheitert,
                Some(peer),
            ));
        };
        if self.prozess_pflicht && g.pid.is_none() {
            return Annahme::Abgewiesen(self.abweisen(strom, Grund::ProzessNichtOffen, Some(peer)));
        }
        if let Err(grund) = erlaubt(&self.erlaubt, g.uid, g.gid) {
            return Annahme::Abgewiesen(self.abweisen(strom, grund, Some(peer)));
        }
        let _ = strom.set_read_timeout(None);
        Annahme::Vorlaeufig(Vorlaeufig {
            strom: Some(strom),
            prozess: bindung(&peer),
            peer,
            kanal: self.kanal.clone(),
            haken: self.haken.clone(),
        })
    }
}

impl Drop for Server {
    /// Removes the socket only when device and inode are still the own (a socket someone else put there stays).
    fn drop(&mut self) {
        if std::fs::symlink_metadata(&self.pfad)
            .is_ok_and(|m| m.dev() == self.geraet && m.ino() == self.inode)
        {
            let _ = std::fs::remove_file(&self.pfad);
        }
    }
}

/// The outcome of `annehmen` under Unix.
pub(crate) enum Annahme {
    Vorlaeufig(Vorlaeufig),
    Abgewiesen(Ereignis),
}

/// A client the list allows; no byte went back yet.
pub(crate) struct Vorlaeufig {
    strom: Option<UnixStream>,
    prozess: Option<Prozessbindung>,
    pub(crate) peer: Peer,
    kanal: Kanalname,
    haken: Option<Haken>,
}

impl Vorlaeufig {
    pub(crate) fn prozess(&self) -> Option<&Prozessbindung> {
        self.prozess.as_ref()
    }

    /// Sends the hello back and hands the connection over.
    pub(crate) fn bestaetigen(mut self) -> Result<Verbindung, Fehler> {
        let mut strom = self.strom.take().ok_or(Fehler::neu(Code::Io))?;
        strom
            .write_all(HALLO)
            .map_err(|e| Fehler::os(Code::Io, e.raw_os_error().unwrap_or(0)))?;
        Ok(Verbindung {
            strom,
            prozess: self.prozess.take(),
            peer: self.peer.clone(),
        })
    }

    /// The event `pk.aufrufer_abgewiesen` to the hook, then the end without an answer.
    pub(crate) fn ablehnen(mut self, grund: Kennung) -> Ereignis {
        let ereignis = Ereignis {
            kanal: self.kanal.clone(),
            grund: Grund::Aufrufer(grund),
            peer: Some(self.peer.clone()),
        };
        if let Some(haken) = &self.haken {
            haken(&ereignis);
        }
        if let Some(strom) = self.strom.take() {
            let _ = strom.shutdown(std::net::Shutdown::Both);
        }
        ereignis
    }
}

fn bindung(peer: &Peer) -> Option<Prozessbindung> {
    peer.pid.map(|pid| Prozessbindung {
        pid,
        startzeit: peer.startzeit,
    })
}

/// A connection after the handshake: a byte stream.
pub(crate) struct Verbindung {
    strom: UnixStream,
    prozess: Option<Prozessbindung>,
    pub(crate) peer: Peer,
}

impl Verbindung {
    pub(crate) fn prozess(&self) -> Option<&Prozessbindung> {
        self.prozess.as_ref()
    }

    /// Limits every following read (contract, "Lesefrist"). The system takes no zero: a zero is the shortest deadline it
    /// takes, so it never means "no deadline".
    pub(crate) fn lesefrist(&mut self, frist: Option<Duration>) {
        let _ = self
            .strom
            .set_read_timeout(frist.map(|f| f.max(Duration::from_micros(1))));
    }

    /// A deadline that passes is `TimedOut` (Unix gives `EAGAIN`), as on the pipe side; any other error is `pk.io` with
    /// `os_fehler`. The end of the other side stays a read of 0 bytes.
    pub(crate) fn lesen(&mut self, puffer: &mut [u8]) -> io::Result<usize> {
        self.strom.read(puffer).map_err(|e| {
            // a blocking socket gives `EAGAIN` only when a deadline is set
            if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) {
                io::Error::new(ErrorKind::TimedOut, Fehler::neu(Code::Io))
            } else {
                io::Error::other(Fehler::os(Code::Io, e.raw_os_error().unwrap_or(0)))
            }
        })
    }

    pub(crate) fn schreiben(&mut self, daten: &[u8]) -> io::Result<usize> {
        self.strom
            .write(daten)
            .map_err(|e| io::Error::other(Fehler::os(Code::Io, e.raw_os_error().unwrap_or(0))))
    }
}

/// The client's check of the server, before any byte: `pk.server_unmessbar` without a uid, `pk.falscher_server` with
/// another one.
fn server_pruefen(gemessen: Option<Gemessen>, erwartet: u32) -> Result<Gemessen, Fehler> {
    let g = gemessen.ok_or(Fehler::neu(Code::ServerUnmessbar))?;
    if g.uid == erwartet {
        Ok(g)
    } else {
        Err(Fehler::neu(Code::FalscherServer))
    }
}

/// "Client verbinden" under Unix: connect within `frist`, measure the uid of the server and compare it with the expected one
/// before any byte, then (with `hallo`) the hello and its answer.
pub(crate) fn verbinden(v: &ClientVorgabe, hallo: bool) -> Result<Verbindung, Fehler> {
    let Erwartet::Unix { uid: erwartet } = v.erwartet else {
        return Err(Fehler::neu(Code::VorgabeUngueltig));
    };
    let ordner = v
        .unix_ordner
        .as_ref()
        .ok_or(Fehler::neu(Code::VorgabeUngueltig))?;
    let pfad = pfad_von(ordner, &v.name);
    let ende = Instant::now() + v.frist;
    // a blocking connect: `EAGAIN` does not come from it (open: a full backlog may hold it beyond `frist` under Linux)
    let mut strom = UnixStream::connect(&pfad).map_err(|e| {
        let code = if e.kind() == ErrorKind::PermissionDenied {
            Code::ZugriffVerweigert
        } else {
            Code::NichtErreichbar
        };
        Fehler::os(code, e.raw_os_error().unwrap_or(0))
    })?;
    let g = server_pruefen(messen(&strom), erwartet)?;
    let peer = peer_von(Some(g));
    let prozess = bindung(&peer);
    if hallo {
        let abgewiesen = || Fehler::neu(Code::Abgewiesen);
        let rest = ende.saturating_duration_since(Instant::now());
        if rest.is_zero() {
            return Err(abgewiesen());
        }
        strom
            .set_write_timeout(Some(rest))
            .map_err(|_| abgewiesen())?;
        strom.write_all(HALLO).map_err(|_| abgewiesen())?;
        hallo_lesen(&mut strom, ende.saturating_duration_since(Instant::now()))
            .map_err(|_| abgewiesen())?;
        let _ = strom.set_read_timeout(None);
        let _ = strom.set_write_timeout(None);
    }
    Ok(Verbindung {
        strom,
        prozess,
        peer,
    })
}

#[cfg(test)]
#[allow(
    clippy::many_single_char_names,
    reason = "server, client, folder and stream ends are short names in the tests"
)]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use crate::vorgabe::{SocketModus, UnixOrt};

    /// A folder 0700 of the test, short enough for `sun_path`; removed at the end.
    struct Ordner(PathBuf);

    impl Ordner {
        fn neu(name: &str) -> Self {
            let basis = std::env::temp_dir().canonicalize().expect("tmp");
            let p = basis.join(format!("pk{}{name}", std::process::id()));
            let _ = std::fs::remove_dir_all(&p);
            std::fs::create_dir(&p).expect("dir");
            std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o700)).expect("chmod");
            Self(p)
        }
    }

    impl Drop for Ordner {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn ich() -> (u32, u32) {
        (
            rustix::process::geteuid().as_raw(),
            rustix::process::getegid().as_raw(),
        )
    }

    fn kanal() -> Kanalname {
        Kennung::neu("k-beispiel").expect("form")
    }

    fn vorgabe(o: &Ordner, erlaubt: Vec<Erlaubnis>) -> ServerVorgabe {
        let mut v = ServerVorgabe::neu(kanal(), erlaubt, 4);
        v.unix = Some(UnixOrt {
            ordner: o.0.clone(),
            modus: SocketModus::M0600,
        });
        v.hallo_frist = Duration::from_millis(300);
        v
    }

    fn eigene(o: &Ordner) -> ServerVorgabe {
        vorgabe(
            o,
            vec![Erlaubnis::Unix {
                uid: ich().0,
                gid: None,
            }],
        )
    }

    fn client(o: &Ordner, uid: u32) -> ClientVorgabe {
        ClientVorgabe {
            name: kanal(),
            erwartet: Erwartet::Unix { uid },
            frist: Duration::from_millis(500),
            unix_ordner: Some(o.0.clone()),
        }
    }

    fn grund(a: Annahme) -> Grund {
        match a {
            Annahme::Abgewiesen(e) => e.grund,
            Annahme::Vorlaeufig(_) => panic!("accepted"),
        }
    }

    #[test]
    fn ein_eigener_client_kommt_durch_und_beide_seiten_kennen_die_gegenseite() {
        let o = Ordner::neu("gut");
        let s = Server::anlegen(eigene(&o)).expect("server");
        let pfad = o.0.join("k-beispiel.sock");
        let m = std::fs::symlink_metadata(&pfad).expect("socket");
        assert!(m.file_type().is_socket());
        assert_eq!(m.permissions().mode() & 0o777, 0o600);
        std::thread::scope(|t| {
            let c = t.spawn(|| {
                let mut v = verbinden(&client(&o, ich().0), true).expect("connect");
                assert_eq!(v.peer.uid, Some(ich().0));
                assert_eq!(v.prozess().map(|p| p.pid), Some(std::process::id()));
                v.schreiben(b"ping").expect("write");
                let mut b = [0u8; 4];
                let mut da = 0;
                while da < 4 {
                    da += v.lesen(&mut b[da..]).expect("read");
                }
                assert_eq!(&b, b"pong");
            });
            let Annahme::Vorlaeufig(vl) = s.annehmen().expect("accept") else {
                panic!("refused");
            };
            assert_eq!(vl.peer.uid, Some(ich().0));
            assert!(vl.peer.gid.is_some());
            assert_eq!(vl.prozess().map(|p| p.pid), Some(std::process::id()));
            if cfg!(target_os = "linux") {
                let stat = std::fs::read_to_string("/proc/self/stat").expect("stat");
                assert_eq!(vl.peer.startzeit, startzeit_aus_stat(&stat));
                assert!(vl.peer.startzeit.is_some());
                assert_eq!(vl.prozess().and_then(|p| p.startzeit), vl.peer.startzeit);
                let exe = std::fs::read_link("/proc/self/exe").expect("exe");
                assert_eq!(vl.peer.programm.as_deref(), exe.to_str());
            } else {
                assert_eq!(
                    (vl.peer.startzeit, vl.peer.programm.as_deref()),
                    (None, None)
                );
            }
            let mut v = vl.bestaetigen().expect("hello");
            assert_eq!(v.prozess().map(|p| p.pid), Some(std::process::id()));
            let mut b = [0u8; 4];
            let mut da = 0;
            while da < 4 {
                da += v.lesen(&mut b[da..]).expect("read");
            }
            assert_eq!(&b, b"ping");
            v.schreiben(b"pong").expect("write");
            c.join().expect("client");
        });
        drop(s);
        assert!(
            std::fs::symlink_metadata(&pfad).is_err(),
            "the own socket goes with the server"
        );
    }

    #[test]
    fn eine_fremde_uid_und_eine_falsche_gid_kommen_nie_durch() {
        let (uid, gid) = ich();
        for (erlaubt, erwartet) in [
            (
                vec![Erlaubnis::Unix {
                    uid: uid + 1,
                    gid: None,
                }],
                Grund::UidNichtErlaubt,
            ),
            (
                vec![Erlaubnis::Unix {
                    uid,
                    gid: Some(gid + 1),
                }],
                Grund::GidFalsch,
            ),
        ] {
            let o = Ordner::neu("fremd");
            let s = Server::anlegen(vorgabe(&o, erlaubt)).expect("server");
            std::thread::scope(|t| {
                let c = t.spawn(|| verbinden(&client(&o, uid), true).map(|_| ()));
                assert_eq!(grund(s.annehmen().expect("accept")), erwartet);
                assert_eq!(
                    c.join().expect("client").err().map(|f| f.code),
                    Some(Code::Abgewiesen)
                );
            });
        }
    }

    #[test]
    fn ohne_messung_gibt_es_keine_verbindung() {
        let o = Ordner::neu("ohne");
        let s = Server::anlegen(eigene(&o)).expect("server");
        let (mut a, b) = UnixStream::pair().expect("pair");
        a.write_all(HALLO).expect("hello");
        assert_eq!(grund(s.pruefen(b, None)), Grund::MessungGescheitert);
        assert_eq!(
            server_pruefen(None, ich().0).err().map(|f| f.code),
            Some(Code::ServerUnmessbar)
        );
        let g = Gemessen {
            uid: 7,
            gid: None,
            pid: None,
        };
        assert_eq!(
            server_pruefen(Some(g), 8).err().map(|f| f.code),
            Some(Code::FalscherServer)
        );
        assert_eq!(server_pruefen(Some(g), 7).ok(), Some(g));
        // with the process mandatory, a measured uid without pid does not pass either
        let mut v = eigene(&o);
        v.prozess_pflicht = true;
        drop(s);
        let s = Server::anlegen(v).expect("server");
        let (mut a, b) = UnixStream::pair().expect("pair");
        a.write_all(HALLO).expect("hello");
        let ohne_pid = Gemessen {
            uid: ich().0,
            gid: None,
            pid: None,
        };
        assert_eq!(
            grund(s.pruefen(b, Some(ohne_pid))),
            Grund::ProzessNichtOffen
        );
        // without the duty a uid alone passes
        drop(s);
        let s = Server::anlegen(eigene(&o)).expect("server");
        let (mut a, b) = UnixStream::pair().expect("pair");
        a.write_all(HALLO).expect("hello");
        assert!(matches!(
            s.pruefen(b, Some(ohne_pid)),
            Annahme::Vorlaeufig(_)
        ));
    }

    #[test]
    fn der_client_sendet_einem_falschen_server_kein_byte() {
        let o = Ordner::neu("falsch");
        let s = Server::anlegen(eigene(&o)).expect("server");
        std::thread::scope(|t| {
            let c = t.spawn(|| verbinden(&client(&o, ich().0 + 1), true).map(|_| ()));
            assert_eq!(
                c.join().expect("client").err().map(|f| f.code),
                Some(Code::FalscherServer)
            );
            // the server sees the end without a single byte of the hello
            assert_eq!(grund(s.annehmen().expect("accept")), Grund::HalloFehlt);
        });
        let ohne = verbinden(&client(&o, ich().0 + 1), false)
            .err()
            .map(|f| f.code);
        assert_eq!(ohne, Some(Code::FalscherServer));
    }

    #[test]
    fn das_hallo_ist_genau_pk1_lf() {
        let o = Ordner::neu("hallo");
        let s = Server::anlegen(eigene(&o)).expect("server");
        let pfad = o.0.join("k-beispiel.sock");
        for (bytes, erwartet) in [
            (&b"GET /"[..], Grund::HalloFalsch),
            (b"pk1\r", Grund::HalloFalsch),
            (b"x", Grund::HalloFalsch),
            (b"pk", Grund::HalloFehlt),
            (b"", Grund::HalloFehlt),
        ] {
            let mut roh = UnixStream::connect(&pfad).expect("connect");
            roh.write_all(bytes).expect("write");
            if bytes == b"pk" || bytes.is_empty() {
                roh.shutdown(std::net::Shutdown::Write).expect("shutdown");
            }
            assert_eq!(grund(s.annehmen().expect("accept")), erwartet, "{bytes:?}");
        }
        // nothing within the deadline
        let _still = UnixStream::connect(&pfad).expect("connect");
        assert_eq!(grund(s.annehmen().expect("accept")), Grund::HalloFehlt);
    }

    #[test]
    fn der_haken_hoert_jede_ablehnung_vor_dem_ende() {
        let o = Ordner::neu("haken");
        let mut s = Server::anlegen(eigene(&o)).expect("server");
        let zahl = Arc::new(AtomicUsize::new(0));
        let z = Arc::clone(&zahl);
        s.haken = Some(Arc::new(move |_: &Ereignis| {
            z.fetch_add(1, Ordering::SeqCst);
        }));
        std::thread::scope(|t| {
            let c = t.spawn(|| verbinden(&client(&o, ich().0), true).map(|_| ()));
            let Annahme::Vorlaeufig(vl) = s.annehmen().expect("accept") else {
                panic!("refused");
            };
            let e = vl.ablehnen(Kennung::neu("sitzung-fremd").expect("form"));
            assert_eq!(e.grund.code(), Code::AufruferAbgewiesen);
            assert_eq!(
                c.join().expect("client").err().map(|f| f.code),
                Some(Code::Abgewiesen)
            );
        });
        let pfad = o.0.join("k-beispiel.sock");
        let mut roh = UnixStream::connect(&pfad).expect("connect");
        roh.write_all(b"x").expect("write");
        let _ = s.annehmen().expect("accept");
        assert_eq!(zahl.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn anlegen_prueft_ordner_pfad_und_vorbeleger() {
        let o = Ordner::neu("anlegen");
        let code = |v: ServerVorgabe| Server::anlegen(v).err().map(|f| f.code);
        std::fs::set_permissions(&o.0, std::fs::Permissions::from_mode(0o750)).expect("chmod");
        assert_eq!(code(eigene(&o)), Some(Code::OrdnerRechte));
        std::fs::set_permissions(&o.0, std::fs::Permissions::from_mode(0o700)).expect("chmod");
        let link = Ordner::neu("link");
        std::fs::remove_dir(&link.0).expect("rm");
        symlink(&o.0, &link.0).expect("link");
        assert_eq!(code(eigene(&link)), Some(Code::OrdnerRechte));
        let fehlt = Ordner::neu("fehlt");
        std::fs::remove_dir(&fehlt.0).expect("rm");
        assert_eq!(code(eigene(&fehlt)), Some(Code::OrdnerRechte));
        // a file, a link and an orphaned socket at the path: no fallback, nothing removed
        let pfad = o.0.join("k-beispiel.sock");
        std::fs::write(&pfad, "x").expect("file");
        assert_eq!(code(eigene(&o)), Some(Code::NameBelegt));
        assert_eq!(std::fs::read(&pfad).expect("still there"), b"x");
        std::fs::remove_file(&pfad).expect("rm");
        symlink("/srv/beispiel/ziel", &pfad).expect("link");
        assert_eq!(code(eigene(&o)), Some(Code::NameBelegt));
        std::fs::remove_file(&pfad).expect("rm");
        drop(UnixListener::bind(&pfad).expect("bind"));
        assert_eq!(code(eigene(&o)), Some(Code::NameBelegt));
        std::fs::remove_file(&pfad).expect("rm");
        // the path with exactly as many bytes as `sun_path` holds is one too long; one byte less fits
        for (laenge, gut) in [(SUN_PATH, false), (SUN_PATH - 1, true)] {
            let n = laenge - o.0.as_os_str().len() - "/".len() - "/k-beispiel.sock".len();
            let lang = o.0.join("x".repeat(n));
            std::fs::create_dir(&lang).expect("dir");
            std::fs::set_permissions(&lang, std::fs::Permissions::from_mode(0o700)).expect("chmod");
            assert_eq!(lang.join("k-beispiel.sock").as_os_str().len(), laenge);
            let mut v = eigene(&o);
            v.unix = Some(UnixOrt {
                ordner: lang.clone(),
                modus: SocketModus::M0600,
            });
            let r = Server::anlegen(v);
            assert_eq!(r.is_ok(), gut, "{laenge}");
            if !gut {
                assert_eq!(r.err().map(|f| f.code), Some(Code::PfadZuLang));
            }
        }
    }

    #[test]
    fn ein_socket_ohne_rechte_ist_zugriff_verweigert_und_keiner_ist_nicht_erreichbar() {
        let o = Ordner::neu("rechte");
        assert_eq!(
            verbinden(&client(&o, ich().0), true).err().map(|f| f.code),
            Some(Code::NichtErreichbar)
        );
        let s = Server::anlegen(eigene(&o)).expect("server");
        if ich().0 == 0 {
            return; // root passes every mode; the case cannot be made then
        }
        let pfad = o.0.join("k-beispiel.sock");
        std::fs::set_permissions(&pfad, std::fs::Permissions::from_mode(0o000)).expect("chmod");
        assert_eq!(
            verbinden(&client(&o, ich().0), true).err().map(|f| f.code),
            Some(Code::ZugriffVerweigert)
        );
        drop(s);
    }

    #[test]
    fn der_server_raeumt_nur_den_eigenen_socket() {
        let o = Ordner::neu("raeumt");
        let s = Server::anlegen(eigene(&o)).expect("server");
        let pfad = o.0.join("k-beispiel.sock");
        std::fs::remove_file(&pfad).expect("rm");
        std::fs::write(&pfad, "fremd").expect("file");
        drop(s);
        assert_eq!(std::fs::read(&pfad).expect("stays"), b"fremd");
    }

    #[test]
    fn grenzen_von_pfad_und_erlaubnis() {
        assert!(pfad_kurz_genug(Path::new(&"a".repeat(SUN_PATH - 1))));
        assert!(!pfad_kurz_genug(Path::new(&"a".repeat(SUN_PATH))));
        let liste = [
            Erlaubnis::Unix {
                uid: 5,
                gid: Some(9),
            },
            Erlaubnis::Unix {
                uid: 5,
                gid: Some(10),
            },
            Erlaubnis::Unix { uid: 6, gid: None },
        ];
        assert_eq!(erlaubt(&liste, 5, Some(10)), Ok(()));
        assert_eq!(erlaubt(&liste, 5, Some(11)), Err(Grund::GidFalsch));
        assert_eq!(erlaubt(&liste, 5, None), Err(Grund::GidFalsch));
        assert_eq!(erlaubt(&liste, 6, None), Ok(()));
        assert_eq!(erlaubt(&liste, 7, Some(9)), Err(Grund::UidNichtErlaubt));
        assert_eq!(erlaubt(&[], 5, None), Err(Grund::UidNichtErlaubt));
    }

    #[test]
    fn die_startzeit_steht_nach_dem_letzten_klammerzu() {
        let mut felder: Vec<String> = (3..=30).map(|n| n.to_string()).collect();
        felder[19] = "424242".to_owned();
        let stat = format!("17 (a) b) c) {}", felder.join(" "));
        assert_eq!(startzeit_aus_stat(&stat), Some(424_242));
        assert_eq!(startzeit_aus_stat("17 kein ende"), None);
        assert_eq!(startzeit_aus_stat("17 (a) 1 2"), None);
    }

    #[test]
    fn die_lesefrist_endet_das_warten_mit_timed_out() {
        let (a, mut b) = UnixStream::pair().expect("pair");
        let mut v = Verbindung {
            strom: a,
            prozess: None,
            peer: Peer::leer(system()),
        };
        let mut puffer = [0u8; 1];
        for frist in [Duration::from_millis(20), Duration::ZERO] {
            v.lesefrist(Some(frist));
            let e = v.lesen(&mut puffer).expect_err("deadline");
            assert_eq!(e.kind(), ErrorKind::TimedOut, "{frist:?}");
        }
        // the connection stays open: a later read goes on
        b.write_all(b"x").expect("write");
        assert_eq!(v.lesen(&mut puffer).expect("byte"), 1);
        drop(b);
        v.lesefrist(None);
        assert_eq!(v.lesen(&mut puffer).expect("end"), 0);
        // writing to a closed end is `pk.io`
        let e = v
            .schreiben(b"y")
            .and_then(|_| v.schreiben(b"y"))
            .expect_err("closed");
        let f = e
            .get_ref()
            .and_then(|f| f.downcast_ref::<Fehler>())
            .copied();
        assert_eq!(f.map(|f| f.code), Some(Code::Io));
    }
}
