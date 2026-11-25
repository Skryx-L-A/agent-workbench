//! Windows: `AF_UNIX` sockets and the access lists of the socket folder and of the socket entry (Nachtrag 25.11.2026, contract
//! `dienst`, section 21). This file is the only place of the crate with `unsafe`; every block names what it relies on.
//!
//! - The socket is `AF_UNIX` (Windows 10 and later): `bind` makes a file entry in the folder (a reparse point with the tag
//!   `IO_REPARSE_TAG_AF_UNIX`), refuses a path that exists (`WSAEADDRINUSE`) and the entry stays after `closesocket` until it is
//!   removed. `connect` needs access to the entry: a user without an entry in its access list gets `WSAEACCES` (measured in the
//!   Windows VM on 25.11.2026 with an ordinary second user). `WSAPoll` works on the listener and on connections, so the loop of
//!   the service waits as on Unix (`rustix::event::poll`, `bereit.rs`).
//! - The rights of DI2 become access lists: the folder belongs to the own user (owner SID), its list is protected (no inherited
//!   entry) and allows nobody but the own user and `SYSTEM` (the counterpart of `root`, which passes a mode 0700 too). The socket
//!   entry gets a protected list with exactly one entry, the own user (the counterpart of mode 0600), and the service reads the
//!   list back as on Unix it reads the mode back.
//! - The folder is held open without `FILE_SHARE_DELETE` from the check to the end: nobody can rename, move or delete it while
//!   the service runs (the counterpart of the descriptor of the folder on Unix).

use std::ffi::c_void;
use std::fs::{File, Metadata, OpenOptions};
use std::io::{self, Read, Write};
use std::net::Shutdown;
use std::os::windows::fs::OpenOptionsExt;
use std::os::windows::io::{
    AsRawHandle, AsRawSocket, AsSocket, BorrowedSocket, FromRawSocket, OwnedSocket,
};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use start::Startanfrage;
use traeger::Kind;

use crate::dienst::Dienst;
use crate::fehler::StartFehler;
use crate::markerdatei::{DATEI_MAX, inhalt_lesen, ungueltig};
use crate::meldung::Meldung;
use crate::sitzung::{KindStart, Sitzung};
use crate::startdatei::{Lauf, SOCKET_PFAD_MAX};
use crate::starten::{Abgelehnt, eintraege_der_nummer, zielliste_name};
use crate::waisen::{kinder_verwerfen, start_verwerfen};

/// A socket of the poll set (`bereit.rs`): the owned socket of `std`.
pub(crate) type OwnedFd = OwnedSocket;
/// The folder a removal goes through (`loeschen.rs`).
pub(crate) type BorrowedFd<'a> = &'a Ordner;

/// What `bereit.rs` waits on: a socket.
pub(crate) trait AsFd {
    /// The socket, borrowed.
    fn as_fd(&self) -> BorrowedSocket<'_>;
}

type Socket = usize;
const INVALID_SOCKET: Socket = !0;
const AF_UNIX: u16 = 1;
const SOCK_STREAM: i32 = 1;
const WSA_FLAG_OVERLAPPED: u32 = 0x01;
const WSA_FLAG_NO_HANDLE_INHERIT: u32 = 0x80;
// `FIONBIO` is `_IOW('f', 126, u_long)` = 0x8004_667E; `ioctlsocket` takes it as a signed `long`
#[allow(clippy::cast_possible_wrap)]
const FIONBIO: i32 = 0x8004_667E_u32 as i32;
const SIO_AF_UNIX_GETPEERPID: u32 = 0x5800_0100;
const SOL_SOCKET: i32 = 0xFFFF;
const SO_RCVTIMEO: i32 = 0x1006;
/// The most bytes one write hands to the socket (see `UnixStream::schreiben`).
const SCHREIB_MAX: usize = 65_536;
/// The length of `sun_path` in `sockaddr_un` of Windows (`afunix.h`), with the closing NUL.
pub(crate) const PFAD_MAX: usize = 108;

const READ_CONTROL: u32 = 0x0002_0000;
const WRITE_DAC: u32 = 0x0004_0000;
const WRITE_OWNER: u32 = 0x0008_0000;
const SYNCHRONIZE: u32 = 0x0010_0000;
const FILE_LIST_DIRECTORY: u32 = 0x0001;
const FILE_READ_ATTRIBUTES: u32 = 0x0080;
const FILE_ALL_ACCESS: u32 = 0x001F_01FF;
const FILE_SHARE_READ: u32 = 0x1;
const FILE_SHARE_WRITE: u32 = 0x2;
const FILE_SHARE_DELETE: u32 = 0x4;
const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;
const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
const FILE_ATTRIBUTE_DIRECTORY: u32 = 0x10;
const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
const IO_REPARSE_TAG_AF_UNIX: u32 = 0x8000_0023;
const FILE_ATTRIBUTE_TAG_INFO: i32 = 9;
const SE_FILE_OBJECT: i32 = 1;
const OWNER_SECURITY_INFORMATION: u32 = 0x1;
const DACL_SECURITY_INFORMATION: u32 = 0x4;
const PROTECTED_DACL_SECURITY_INFORMATION: u32 = 0x8000_0000;
const SE_DACL_PROTECTED: u16 = 0x1000;
const ACL_REVISION: u32 = 2;
const ACCESS_ALLOWED_ACE_TYPE: u8 = 0;
const ACCESS_DENIED_ACE_TYPE: u8 = 1;
const TOKEN_QUERY: u32 = 0x0008;
const TOKEN_DUPLICATE: u32 = 0x0002;
const GROUP_SECURITY_INFORMATION: u32 = 0x2;
/// `SecurityImpersonation` of `SECURITY_IMPERSONATION_LEVEL`.
const SECURITY_IMPERSONATION: i32 = 2;

/// `GENERIC_MAPPING` with the values of files and folders (`FILE_GENERIC_READ`, `_WRITE`, `_EXECUTE`, `FILE_ALL_ACCESS`).
#[repr(C)]
struct AbbildungAllgemein {
    lesen: u32,
    schreiben: u32,
    ausfuehren: u32,
    alles: u32,
}
const TOKEN_USER_KLASSE: i32 = 1;
/// `S-1-5-18` (`SYSTEM`), in the binary form of a SID.
const SID_SYSTEM: [u8; 12] = [1, 1, 0, 0, 0, 0, 0, 5, 18, 0, 0, 0];

#[repr(C)]
struct SockaddrUn {
    familie: u16,
    pfad: [u8; PFAD_MAX],
}

#[repr(C)]
struct AttributTag {
    attribute: u32,
    tag: u32,
}

/// `CONSOLE_SCREEN_BUFFER_INFO`: size, cursor, attributes, the window (`SMALL_RECT`) and the largest window.
#[cfg(feature = "pruefbau")]
#[repr(C)]
#[derive(Default)]
struct KonsolenInfo {
    groesse: [i16; 2],
    cursor: [i16; 2],
    attribute: u16,
    fenster: [i16; 4],
    groesstes: [i16; 2],
}

#[repr(C)]
struct AclKopf {
    revision: u8,
    sbz1: u8,
    groesse: u16,
    anzahl: u16,
    sbz2: u16,
}

#[link(name = "ws2_32")]
unsafe extern "system" {
    fn WSAStartup(version: u16, daten: *mut u8) -> i32;
    fn WSASocketW(
        af: i32,
        typ: i32,
        proto: i32,
        info: *mut c_void,
        gruppe: u32,
        flags: u32,
    ) -> Socket;
    fn bind(s: Socket, name: *const c_void, laenge: i32) -> i32;
    fn listen(s: Socket, rueckstau: i32) -> i32;
    fn accept(s: Socket, adresse: *mut c_void, laenge: *mut i32) -> Socket;
    fn connect(s: Socket, name: *const c_void, laenge: i32) -> i32;
    fn recv(s: Socket, puffer: *mut u8, laenge: i32, flags: i32) -> i32;
    fn send(s: Socket, puffer: *const u8, laenge: i32, flags: i32) -> i32;
    fn ioctlsocket(s: Socket, befehl: i32, wert: *mut u32) -> i32;
    #[cfg_attr(not(test), allow(dead_code))]
    fn setsockopt(s: Socket, ebene: i32, name: i32, wert: *const u8, laenge: i32) -> i32;
    fn shutdown(s: Socket, wie: i32) -> i32;
    fn WSAGetLastError() -> i32;
    fn WSAIoctl(
        s: Socket,
        code: u32,
        ein: *const c_void,
        ein_laenge: u32,
        aus: *mut c_void,
        aus_laenge: u32,
        zurueck: *mut u32,
        overlapped: *mut c_void,
        routine: *mut c_void,
    ) -> i32;
}

#[link(name = "advapi32")]
unsafe extern "system" {
    fn GetSecurityInfo(
        griff: *mut c_void,
        art: i32,
        info: u32,
        besitzer: *mut *mut c_void,
        gruppe: *mut *mut c_void,
        dacl: *mut *mut c_void,
        sacl: *mut *mut c_void,
        beschreibung: *mut *mut c_void,
    ) -> u32;
    fn SetSecurityInfo(
        griff: *mut c_void,
        art: i32,
        info: u32,
        besitzer: *mut c_void,
        gruppe: *mut c_void,
        dacl: *const c_void,
        sacl: *const c_void,
    ) -> u32;
    fn GetSecurityDescriptorControl(
        beschreibung: *mut c_void,
        steuerung: *mut u16,
        revision: *mut u32,
    ) -> i32;
    fn GetAce(acl: *const c_void, index: u32, ace: *mut *mut c_void) -> i32;
    fn GetLengthSid(sid: *const c_void) -> u32;
    fn IsValidSid(sid: *const c_void) -> i32;
    fn InitializeAcl(acl: *mut c_void, laenge: u32, revision: u32) -> i32;
    fn AddAccessAllowedAceEx(
        acl: *mut c_void,
        revision: u32,
        flags: u32,
        maske: u32,
        sid: *const c_void,
    ) -> i32;
    fn OpenProcessToken(prozess: *mut c_void, zugriff: u32, token: *mut *mut c_void) -> i32;
    fn DuplicateToken(token: *mut c_void, stufe: i32, kopie: *mut *mut c_void) -> i32;
    fn AccessCheck(
        beschreibung: *mut c_void,
        token: *mut c_void,
        gewuenscht: u32,
        abbildung: *const AbbildungAllgemein,
        rechte: *mut c_void,
        rechte_laenge: *mut u32,
        gewaehrt: *mut u32,
        erlaubt: *mut i32,
    ) -> i32;
    fn GetTokenInformation(
        token: *mut c_void,
        klasse: i32,
        info: *mut c_void,
        laenge: u32,
        zurueck: *mut u32,
    ) -> i32;
}

#[link(name = "kernel32")]
unsafe extern "system" {
    #[cfg(feature = "pruefbau")]
    fn GetStdHandle(nummer: u32) -> *mut c_void;
    #[cfg(feature = "pruefbau")]
    fn GetConsoleMode(griff: *mut c_void, modus: *mut u32) -> i32;
    #[cfg(feature = "pruefbau")]
    fn SetConsoleMode(griff: *mut c_void, modus: u32) -> i32;
    #[cfg(feature = "pruefbau")]
    fn GetConsoleScreenBufferInfo(griff: *mut c_void, info: *mut KonsolenInfo) -> i32;
    fn GetCurrentProcess() -> *mut c_void;
    fn CreateJobObjectW(sicherheit: *const c_void, name: *const u16) -> *mut c_void;
    fn SetInformationJobObject(
        job: *mut c_void,
        klasse: i32,
        info: *const c_void,
        laenge: u32,
    ) -> i32;
    fn AssignProcessToJobObject(job: *mut c_void, prozess: *mut c_void) -> i32;
    fn CloseHandle(griff: *mut c_void) -> i32;
    fn LocalFree(speicher: *mut c_void) -> *mut c_void;
    fn GetFileInformationByHandleEx(
        griff: *mut c_void,
        klasse: i32,
        info: *mut c_void,
        laenge: u32,
    ) -> i32;
}

/// `JobObjectExtendedLimitInformation` of `SetInformationJobObject`.
const JOB_OBJECT_EXTENDED_LIMIT_INFORMATION: i32 = 9;
/// Every process of the job ends when the last handle of the job is closed.
const JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE: u32 = 0x2000;

/// `JOBOBJECT_BASIC_LIMIT_INFORMATION` (`winnt.h`); `repr(C)` gives the same padding (64 bytes on 64 bit).
#[repr(C)]
#[derive(Default)]
struct JobGrenzen {
    zeit_prozess: i64,
    zeit_job: i64,
    flags: u32,
    arbeitsmenge_min: usize,
    arbeitsmenge_max: usize,
    prozesse_max: u32,
    affinitaet: usize,
    prioritaet: u32,
    planung: u32,
}

/// `JOBOBJECT_EXTENDED_LIMIT_INFORMATION`: the basic limits, `IO_COUNTERS` (six `u64`) and four sizes (144 bytes on 64 bit).
#[repr(C)]
#[derive(Default)]
struct JobGrenzenErweitert {
    grundlage: JobGrenzen,
    ein_aus: [u64; 6],
    speicher_prozess: usize,
    speicher_job: usize,
    spitze_prozess: usize,
    spitze_job: usize,
}

#[cfg(feature = "pruefbau")]
thread_local! {
    /// Test build: while `true`, [`Job::neu`] of this thread fails before it makes a job. It reaches the refusal of a start without a
    /// job (contract `dienst`, section 21a point 4) without an error of the system.
    pub(crate) static JOB_VERWEIGERN: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// The job object of one session (Nachtrag 25.11.2026, contract `dienst`, section 21): it holds the child, and every process the
/// child starts after it joined is in it too. With `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` the system ends all of them when the handle
/// closes: on drop (the end of the session or of the service) and when the service ends in any other way, because the system closes
/// the handles of an ended process. The handle is not inheritable (no security attributes), so no child holds the job open. It is
/// kept as a number (a kernel handle is valid in every thread), so the value is `Send` like the rest of the session.
pub(crate) struct Job(usize);

impl Job {
    /// A new job that ends its processes when it closes, still empty.
    ///
    /// # Errors
    /// The job could not be made or set (in the test build also while `JOB_VERWEIGERN` is set in this thread).
    pub(crate) fn neu() -> io::Result<Self> {
        #[cfg(feature = "pruefbau")]
        if JOB_VERWEIGERN.with(std::cell::Cell::get) {
            return Err(io::Error::other("job refused by the test build"));
        }
        // SAFETY: null security attributes (not inheritable) and no name are allowed; the result is checked below.
        let roh = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
        if roh.is_null() {
            return Err(io::Error::last_os_error());
        }
        // from here on, `Drop` closes the handle on every way out
        let job = Self(roh.expose_provenance());
        let mut grenzen = JobGrenzenErweitert::default();
        grenzen.grundlage.flags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        // SAFETY: `job` holds the valid job from above; `grenzen` has the layout of `JOBOBJECT_EXTENDED_LIMIT_INFORMATION`, its size
        // is passed, and it lives for the call.
        let gesetzt = unsafe {
            SetInformationJobObject(
                job.griff(),
                JOB_OBJECT_EXTENDED_LIMIT_INFORMATION,
                (&raw const grenzen).cast(),
                u32::try_from(size_of::<JobGrenzenErweitert>()).unwrap_or(u32::MAX),
            )
        };
        if gesetzt == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(job)
    }

    /// Puts the process of `griff` into the job.
    ///
    /// # Errors
    /// The process could not join (for example because it has ended already).
    pub(crate) fn aufnehmen(&self, griff: std::os::windows::io::RawHandle) -> io::Result<()> {
        // SAFETY: `self` holds the valid job; `griff` is the handle of the child that the carrier holds open while the caller borrows it.
        if unsafe { AssignProcessToJobObject(self.griff(), griff.cast()) } == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }

    /// A new job with the process of `griff` in it ([`Job::neu`], then [`Job::aufnehmen`]).
    ///
    /// # Errors
    /// See both.
    #[cfg(test)]
    pub(crate) fn fuer(griff: std::os::windows::io::RawHandle) -> io::Result<Self> {
        let job = Self::neu()?;
        job.aufnehmen(griff)?;
        Ok(job)
    }

    /// The handle of the job.
    fn griff(&self) -> *mut c_void {
        std::ptr::with_exposed_provenance_mut(self.0)
    }
}

impl Drop for Job {
    /// Closes the job: the system ends every process still in it.
    fn drop(&mut self) {
        // SAFETY: the handle came from `CreateJobObjectW`, belongs to this value alone and is closed once.
        unsafe {
            CloseHandle(self.griff());
        }
    }
}

/// `WSAStartup` once per process (version 2.2). `std` starts Winsock only for its own sockets.
fn winsock() -> io::Result<()> {
    static START: OnceLock<i32> = OnceLock::new();
    let code = *START.get_or_init(|| {
        let mut daten = [0_u8; 512];
        // SAFETY: `daten` is larger than `WSADATA` (408 bytes on 64 bit) and lives for the call.
        unsafe { WSAStartup(0x0202, daten.as_mut_ptr()) }
    });
    if code == 0 {
        Ok(())
    } else {
        Err(io::Error::from_raw_os_error(code))
    }
}

fn letzter_fehler() -> io::Error {
    // SAFETY: no argument; reads the error of the last Winsock call of this thread.
    io::Error::from_raw_os_error(unsafe { WSAGetLastError() })
}

/// A new `AF_UNIX` stream socket that no child inherits (`WSA_FLAG_NO_HANDLE_INHERIT`, the counterpart of `CLOEXEC`).
fn neuer_socket() -> io::Result<OwnedSocket> {
    winsock()?;
    // SAFETY: plain values; a null protocol info is allowed.
    let s = unsafe {
        WSASocketW(
            i32::from(AF_UNIX),
            SOCK_STREAM,
            0,
            std::ptr::null_mut(),
            0,
            WSA_FLAG_OVERLAPPED | WSA_FLAG_NO_HANDLE_INHERIT,
        )
    };
    if s == INVALID_SOCKET {
        return Err(letzter_fehler());
    }
    // SAFETY: `s` is a new, valid socket that nothing else owns.
    Ok(unsafe { OwnedSocket::from_raw_socket(s as u64) })
}

fn roh(s: &OwnedSocket) -> Socket {
    // a SOCKET is pointer-sized on Windows; `as_raw_socket` widens it to u64
    usize::try_from(s.as_raw_socket()).unwrap_or(INVALID_SOCKET)
}

/// The address of a path: UTF-8 bytes and a NUL within 108 bytes.
fn adresse(pfad: &Path) -> io::Result<SockaddrUn> {
    let text = pfad
        .to_str()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "path is not UTF-8"))?;
    let bytes = text.as_bytes();
    if bytes.len() >= PFAD_MAX || bytes.contains(&0) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "path too long for AF_UNIX",
        ));
    }
    let mut a = SockaddrUn {
        familie: AF_UNIX,
        pfad: [0; PFAD_MAX],
    };
    a.pfad[..bytes.len()].copy_from_slice(bytes);
    Ok(a)
}

fn laenge_von<T>() -> i32 {
    i32::try_from(size_of::<T>()).unwrap_or(i32::MAX)
}

fn nicht_blockierend(s: &OwnedSocket, an: bool) -> io::Result<()> {
    let mut wert = u32::from(an);
    // SAFETY: valid socket, `wert` lives for the call.
    if unsafe { ioctlsocket(roh(s), FIONBIO, &raw mut wert) } == 0 {
        Ok(())
    } else {
        Err(letzter_fehler())
    }
}

#[cfg_attr(not(test), allow(dead_code))]
fn frist_setzen(s: &OwnedSocket, name: i32, frist: Option<Duration>) -> io::Result<()> {
    let ms: u32 = match frist {
        Some(d) if d.is_zero() => {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "zero timeout"));
        }
        Some(d) => u32::try_from(d.as_millis()).unwrap_or(u32::MAX).max(1),
        None => 0,
    };
    let b = ms.to_ne_bytes();
    // SAFETY: valid socket, a DWORD of 4 bytes as the option value.
    if unsafe { setsockopt(roh(s), SOL_SOCKET, name, b.as_ptr(), 4) } == 0 {
        Ok(())
    } else {
        Err(letzter_fehler())
    }
}

/// A connected `AF_UNIX` stream.
#[derive(Debug)]
pub(crate) struct UnixStream(OwnedSocket);

impl UnixStream {
    /// Connects to the socket at `pfad`.
    pub(crate) fn connect(pfad: impl AsRef<Path>) -> io::Result<Self> {
        let s = neuer_socket()?;
        let a = adresse(pfad.as_ref())?;
        // SAFETY: valid socket; `a` is a complete `sockaddr_un` that lives for the call.
        if unsafe { connect(roh(&s), (&raw const a).cast(), laenge_von::<SockaddrUn>()) } != 0 {
            return Err(letzter_fehler());
        }
        Ok(Self(s))
    }

    /// Two connected streams of this process (Winsock has no `socketpair`): a listener under a fresh name in the folder of the
    /// temporary files of the user, one connection, and the name removed again. The accepted side must come from this process
    /// (`SIO_AF_UNIX_GETPEERPID`), otherwise the pair is refused.
    pub(crate) fn pair() -> io::Result<(Self, Self)> {
        static ZAEHLER: AtomicU64 = AtomicU64::new(0);
        let n = ZAEHLER.fetch_add(1, Ordering::Relaxed);
        let pfad = std::env::temp_dir().join(format!("wbd-{}-{n}.sock", std::process::id()));
        let hoerer = UnixListener::binden(&pfad)?;
        let ergebnis = (|| {
            hoerer.listen()?;
            let a = Self::connect(&pfad)?;
            let (b, _) = hoerer.accept()?;
            if b.peer_pid()? != std::process::id() {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "foreign peer",
                ));
            }
            Ok((a, b))
        })();
        drop(hoerer);
        let _ = std::fs::remove_file(&pfad);
        ergebnis
    }

    /// The process on the other side (`SIO_AF_UNIX_GETPEERPID`).
    pub(crate) fn peer_pid(&self) -> io::Result<u32> {
        let mut pid: u32 = 0;
        let mut zurueck: u32 = 0;
        // SAFETY: valid socket; `pid` (4 bytes) and `zurueck` live for the call; no overlapped I/O.
        let rc = unsafe {
            WSAIoctl(
                roh(&self.0),
                SIO_AF_UNIX_GETPEERPID,
                std::ptr::null(),
                0,
                (&raw mut pid).cast(),
                4,
                &raw mut zurueck,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        };
        if rc == 0 {
            Ok(pid)
        } else {
            Err(letzter_fehler())
        }
    }

    pub(crate) fn set_nonblocking(&self, an: bool) -> io::Result<()> {
        nicht_blockierend(&self.0, an)
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn set_read_timeout(&self, frist: Option<Duration>) -> io::Result<()> {
        frist_setzen(&self.0, SO_RCVTIMEO, frist)
    }

    pub(crate) fn shutdown(&self, wie: Shutdown) -> io::Result<()> {
        let w = match wie {
            Shutdown::Read => 0,
            Shutdown::Write => 1,
            Shutdown::Both => 2,
        };
        // SAFETY: valid socket.
        if unsafe { shutdown(roh(&self.0), w) } == 0 {
            Ok(())
        } else {
            Err(letzter_fehler())
        }
    }

    fn lesen(&self, puffer: &mut [u8]) -> io::Result<usize> {
        let n = i32::try_from(puffer.len()).unwrap_or(i32::MAX);
        // SAFETY: valid socket; at most `n` bytes are written into `puffer`.
        let r = unsafe { recv(roh(&self.0), puffer.as_mut_ptr(), n, 0) };
        usize::try_from(r).map_err(|_| letzter_fehler())
    }

    /// At most [`SCHREIB_MAX`] bytes per call: a non-blocking `AF_UNIX` socket of Windows takes two calls whole, whatever their
    /// size, before it says `WSAEWOULDBLOCK` (measured 25.11.2026: 2 x 4 000 000 bytes, and 2 x 65 536 bytes). Without the bound two
    /// frames of up to 16 MiB would sit in the system and out of the count `ausstehend` of DI5; with it at most 128 KiB, about the
    /// buffer of a Unix socket.
    fn schreiben(&self, puffer: &[u8]) -> io::Result<usize> {
        let n = i32::try_from(puffer.len().min(SCHREIB_MAX)).unwrap_or(i32::MAX);
        // SAFETY: valid socket; at most `n` bytes are read from `puffer`.
        let r = unsafe { send(roh(&self.0), puffer.as_ptr(), n, 0) };
        usize::try_from(r).map_err(|_| letzter_fehler())
    }
}

impl Read for UnixStream {
    fn read(&mut self, puffer: &mut [u8]) -> io::Result<usize> {
        self.lesen(puffer)
    }
}

impl Read for &UnixStream {
    fn read(&mut self, puffer: &mut [u8]) -> io::Result<usize> {
        self.lesen(puffer)
    }
}

impl Write for UnixStream {
    fn write(&mut self, puffer: &[u8]) -> io::Result<usize> {
        self.schreiben(puffer)
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Write for &UnixStream {
    fn write(&mut self, puffer: &[u8]) -> io::Result<usize> {
        self.schreiben(puffer)
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl AsFd for UnixStream {
    fn as_fd(&self) -> BorrowedSocket<'_> {
        self.0.as_socket()
    }
}

/// The place of the address of the other side that `std` returns from `accept`; the service never reads it.
#[derive(Debug)]
pub(crate) struct Adresse;

/// A listening `AF_UNIX` socket.
#[derive(Debug)]
pub(crate) struct UnixListener(OwnedSocket);

impl UnixListener {
    /// A socket bound to `pfad`, not yet listening.
    fn binden(pfad: &Path) -> io::Result<Self> {
        let s = neuer_socket()?;
        let a = adresse(pfad)?;
        // SAFETY: valid socket; `a` is a complete `sockaddr_un` that lives for the call.
        if unsafe { bind(roh(&s), (&raw const a).cast(), laenge_von::<SockaddrUn>()) } != 0 {
            return Err(letzter_fehler());
        }
        Ok(Self(s))
    }

    fn listen(&self) -> io::Result<()> {
        // SAFETY: valid, bound socket.
        if unsafe { listen(roh(&self.0), 128) } == 0 {
            Ok(())
        } else {
            Err(letzter_fehler())
        }
    }

    /// Takes one connection. The second value stands for the address of `std` (no use here).
    pub(crate) fn accept(&self) -> io::Result<(UnixStream, Adresse)> {
        // SAFETY: valid listening socket; no address is asked for.
        let s = unsafe { accept(roh(&self.0), std::ptr::null_mut(), std::ptr::null_mut()) };
        if s == INVALID_SOCKET {
            return Err(letzter_fehler());
        }
        // SAFETY: `s` is a new, valid socket that nothing else owns.
        Ok((
            UnixStream(unsafe { OwnedSocket::from_raw_socket(s as u64) }),
            Adresse,
        ))
    }

    pub(crate) fn set_nonblocking(&self, an: bool) -> io::Result<()> {
        nicht_blockierend(&self.0, an)
    }
}

impl AsFd for UnixListener {
    fn as_fd(&self) -> BorrowedSocket<'_> {
        self.0.as_socket()
    }
}

/// The socket folder, held open from the check to the end without `FILE_SHARE_DELETE`.
#[derive(Debug)]
pub(crate) struct Ordner {
    _griff: File,
    pfad: PathBuf,
}

impl Ordner {
    /// The folder itself (the counterpart of `as_fd` on Unix).
    pub(crate) fn as_fd(&self) -> &Self {
        self
    }

    /// The path the folder was opened by; it cannot change while the folder is open.
    pub(crate) fn pfad(&self) -> &Path {
        &self.pfad
    }
}

fn attribut_tag(datei: &File) -> io::Result<AttributTag> {
    let mut info = AttributTag {
        attribute: 0,
        tag: 0,
    };
    // SAFETY: valid handle; `info` has the layout of `FILE_ATTRIBUTE_TAG_INFO` and lives for the call.
    let ok = unsafe {
        GetFileInformationByHandleEx(
            datei.as_raw_handle(),
            FILE_ATTRIBUTE_TAG_INFO,
            (&raw mut info).cast(),
            8,
        )
    };
    if ok == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(info)
    }
}

/// The SID of the user of this process, in its binary form.
pub(crate) fn eigene_sid() -> io::Result<Vec<u8>> {
    static SID: OnceLock<Result<Vec<u8>, i32>> = OnceLock::new();
    SID.get_or_init(|| sid_lesen().map_err(|e| e.raw_os_error().unwrap_or(0)))
        .clone()
        .map_err(io::Error::from_raw_os_error)
}

fn sid_lesen() -> io::Result<Vec<u8>> {
    let mut token: *mut c_void = std::ptr::null_mut();
    // SAFETY: the pseudo handle of this process; `token` receives a new handle that is closed below.
    if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &raw mut token) } == 0 {
        return Err(io::Error::last_os_error());
    }
    let mut puffer = vec![0_u64; 128];
    let mut laenge: u32 = 0;
    let groesse = u32::try_from(puffer.len() * 8).unwrap_or(0);
    // SAFETY: valid token; `puffer` (1024 bytes, aligned to 8) takes the `TOKEN_USER` and its SID.
    let ok = unsafe {
        GetTokenInformation(
            token,
            TOKEN_USER_KLASSE,
            puffer.as_mut_ptr().cast(),
            groesse,
            &raw mut laenge,
        )
    };
    let fehler = io::Error::last_os_error();
    // SAFETY: `token` came from `OpenProcessToken` and is closed once.
    unsafe { CloseHandle(token) };
    if ok == 0 {
        return Err(fehler);
    }
    // TOKEN_USER starts with SID_AND_ATTRIBUTES, whose first field is the pointer to the SID (inside `puffer`)
    // SAFETY: `GetTokenInformation` succeeded, so the first pointer-sized field of `puffer` points at a valid SID in `puffer`.
    let sid = unsafe { *puffer.as_ptr().cast::<*const c_void>() };
    sid_kopie(sid).ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "invalid SID"))
}

/// A copy of the SID at `sid`, if it is valid.
fn sid_kopie(sid: *const c_void) -> Option<Vec<u8>> {
    if sid.is_null() {
        return None;
    }
    // SAFETY: `sid` is not null and points at a SID from the system (owner, ACE or token); `IsValidSid` checks its form
    // before `GetLengthSid` reads its length, and the copy reads exactly that many bytes.
    unsafe {
        if IsValidSid(sid) == 0 {
            return None;
        }
        let n = GetLengthSid(sid) as usize;
        Some(std::slice::from_raw_parts(sid.cast::<u8>(), n).to_vec())
    }
}

/// An entry of an access list: type, flags and SID.
struct Eintrag {
    typ: u8,
    /// The access mask of an allowed or denied entry (0 for other types).
    maske: u32,
    sid: Option<Vec<u8>>,
}

/// Owner, protection and entries of the access list of an open file or folder.
struct Sicherheit {
    besitzer: Option<Vec<u8>>,
    geschuetzt: bool,
    /// `None`: no list at all (a null DACL, which allows everyone everything).
    eintraege: Option<Vec<Eintrag>>,
}

fn sicherheit_lesen(datei: &File) -> io::Result<Sicherheit> {
    let mut besitzer: *mut c_void = std::ptr::null_mut();
    let mut dacl: *mut c_void = std::ptr::null_mut();
    let mut beschreibung: *mut c_void = std::ptr::null_mut();
    // SAFETY: valid handle with READ_CONTROL; the out pointers live for the call; `beschreibung` is freed below with LocalFree,
    // and `besitzer` and `dacl` point into it.
    let rc = unsafe {
        GetSecurityInfo(
            datei.as_raw_handle(),
            SE_FILE_OBJECT,
            OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION,
            &raw mut besitzer,
            std::ptr::null_mut(),
            &raw mut dacl,
            std::ptr::null_mut(),
            &raw mut beschreibung,
        )
    };
    if rc != 0 {
        return Err(io::Error::from_raw_os_error(i32::try_from(rc).unwrap_or(0)));
    }
    let ergebnis = sicherheit_aus(besitzer, dacl, beschreibung);
    // SAFETY: `beschreibung` was allocated by `GetSecurityInfo` and is freed once; nothing points into it after this.
    unsafe { LocalFree(beschreibung) };
    Ok(ergebnis)
}

fn sicherheit_aus(
    besitzer: *mut c_void,
    dacl: *mut c_void,
    beschreibung: *mut c_void,
) -> Sicherheit {
    let mut steuerung: u16 = 0;
    let mut revision: u32 = 0;
    // SAFETY: `beschreibung` is the valid descriptor from `GetSecurityInfo`.
    let geschuetzt = unsafe {
        GetSecurityDescriptorControl(beschreibung, &raw mut steuerung, &raw mut revision)
    } != 0
        && steuerung & SE_DACL_PROTECTED != 0;
    let eintraege = if dacl.is_null() {
        None
    } else {
        // SAFETY: `dacl` is a valid ACL inside the descriptor; its head is read, and `GetAce` checks each index against it.
        let anzahl = unsafe { (*dacl.cast::<AclKopf>()).anzahl };
        let mut liste = Vec::new();
        for i in 0..u32::from(anzahl) {
            let mut ace: *mut c_void = std::ptr::null_mut();
            // SAFETY: valid ACL, index below its count.
            if unsafe { GetAce(dacl, i, &raw mut ace) } == 0 || ace.is_null() {
                liste.push(Eintrag {
                    typ: u8::MAX,
                    maske: 0,
                    sid: None,
                });
                continue;
            }
            // SAFETY: `ace` points at an ACE of the list; its first byte is the type. The SID of an allowed or denied ACE starts
            // after the head (4 bytes) and the mask (4 bytes); other types are not read further.
            let typ = unsafe { *ace.cast::<u8>() };
            let (maske, sid) = if typ == ACCESS_ALLOWED_ACE_TYPE || typ == ACCESS_DENIED_ACE_TYPE {
                // SAFETY: as above; the mask is the 4 bytes after the head, read without alignment.
                let maske = unsafe { ace.cast::<u8>().add(4).cast::<u32>().read_unaligned() };
                (maske, sid_kopie(unsafe { ace.cast::<u8>().add(8) }.cast()))
            } else {
                (0, None)
            };
            liste.push(Eintrag { typ, maske, sid });
        }
        Some(liste)
    };
    Sicherheit {
        besitzer: sid_kopie(besitzer),
        geschuetzt,
        eintraege,
    }
}

fn verweigert(text: &str) -> StartFehler {
    StartFehler::neu(3, text)
}

/// DI2 points 1 to 3 under Windows: the folder exists, is a folder and no reparse point (no link, no junction), belongs to the
/// own user and its protected list allows nobody but the own user and `SYSTEM`. The check runs on the open handle, so the
/// folder that is checked is the folder that is held.
pub(crate) fn ordner_oeffnen(ordner: &Path, _euid: u32) -> Result<Ordner, StartFehler> {
    let griff = OpenOptions::new()
        .access_mode(READ_CONTROL | FILE_LIST_DIRECTORY | FILE_READ_ATTRIBUTES | SYNCHRONIZE)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
        .open(ordner)
        .map_err(|e| {
            verweigert(&format!(
                "Ordner des Sockets fehlt oder ist nicht lesbar: {}",
                e.kind()
            ))
        })?;
    let info = attribut_tag(&griff).map_err(|_| verweigert("Ordner des Sockets nicht pruefbar"))?;
    if info.attribute & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
        return Err(verweigert("Ordner des Sockets ist ein symbolischer Link"));
    }
    if info.attribute & FILE_ATTRIBUTE_DIRECTORY == 0 {
        return Err(verweigert("Ordner des Sockets ist kein Ordner"));
    }
    let eigen = eigene_sid().map_err(|_| verweigert("eigener Benutzer nicht lesbar"))?;
    let s =
        sicherheit_lesen(&griff).map_err(|_| verweigert("Ordner des Sockets nicht pruefbar"))?;
    if s.besitzer.as_deref() != Some(eigen.as_slice()) {
        return Err(verweigert(
            "Ordner des Sockets gehoert einem anderen Benutzer",
        ));
    }
    if !liste_nur_eigen(&s, &eigen) {
        return Err(verweigert(
            "Ordner des Sockets ist fuer andere offen (Zugriffsliste nicht geschuetzt oder mit anderen Eintraegen statt 0700)",
        ));
    }
    Ok(Ordner {
        _griff: griff,
        pfad: ordner.to_path_buf(),
    })
}

/// Whether the list is protected and every entry names the own user or `SYSTEM` (a deny entry of anyone only takes away).
fn liste_nur_eigen(s: &Sicherheit, eigen: &[u8]) -> bool {
    let Some(eintraege) = s.eintraege.as_ref() else {
        return false;
    };
    s.geschuetzt
        && eintraege.iter().all(|e| match (e.typ, e.sid.as_deref()) {
            (ACCESS_DENIED_ACE_TYPE, Some(_)) => true,
            (ACCESS_ALLOWED_ACE_TYPE, Some(sid)) => sid == eigen || sid == SID_SYSTEM,
            _ => false,
        })
        && eigen_darf(eintraege, eigen, FILE_ALL_ACCESS)
}

/// Read and write of a file (`FILE_GENERIC_READ | FILE_GENERIC_WRITE`): what the owner holds with mode 0600.
const LESEN_SCHREIBEN: u32 = 0x0012_0089 | 0x0012_0116;

/// Whether an allowed entry of the own user holds at least `noetig` (the owner bits of the mode: `rwx` of 0700 is full access, `rw`
/// of 0600 is read and write). Without it the list is not the counterpart of the exact mode (a folder 0600 under Unix is refused too).
fn eigen_darf(eintraege: &[Eintrag], eigen: &[u8], noetig: u32) -> bool {
    eintraege.iter().any(|e| {
        e.typ == ACCESS_ALLOWED_ACE_TYPE
            && e.sid.as_deref() == Some(eigen)
            && e.maske & noetig == noetig
    })
}

/// The socket entry for setting its list: opened as the entry itself (no reparse point is followed), and it must be an
/// `AF_UNIX` entry.
fn eintrag_oeffnen(pfad: &Path) -> io::Result<File> {
    let griff = OpenOptions::new()
        .access_mode(READ_CONTROL | WRITE_DAC | WRITE_OWNER)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
        .open(pfad)?;
    let info = attribut_tag(&griff)?;
    if info.attribute & FILE_ATTRIBUTE_REPARSE_POINT == 0 || info.tag != IO_REPARSE_TAG_AF_UNIX {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "not an AF_UNIX entry",
        ));
    }
    Ok(griff)
}

/// DI2 point 5 under Windows: the entry gets the own user as owner and a protected list with one entry, full access for the
/// own user (the counterpart of 0600); then the list is read back.
fn socket_liste_setzen(pfad: &Path) -> Result<(), ()> {
    let griff = eintrag_oeffnen(pfad).map_err(|_| ())?;
    liste_eigen_setzen(&griff, false)
}

/// Sets the own user as owner and a protected list with one entry, full access for the own user (inherited by what is made
/// below a folder when `vererben`), and reads it back.
fn liste_eigen_setzen(griff: &File, vererben: bool) -> Result<(), ()> {
    let eigen = eigene_sid().map_err(|_| ())?;
    // OBJECT_INHERIT_ACE | CONTAINER_INHERIT_ACE
    let flags = if vererben { 0x3 } else { 0 };
    let laenge = 8 + 8 + eigen.len();
    let mut acl = vec![0_u32; laenge.div_ceil(4) + 1];
    let groesse = u32::try_from(acl.len() * 4).map_err(|_| ())?;
    // SAFETY: `acl` is DWORD aligned and large enough for the head and one allowed ACE with this SID; `eigen` is a valid SID.
    let gebaut = unsafe {
        InitializeAcl(acl.as_mut_ptr().cast(), groesse, ACL_REVISION) != 0
            && AddAccessAllowedAceEx(
                acl.as_mut_ptr().cast(),
                ACL_REVISION,
                flags,
                FILE_ALL_ACCESS,
                eigen.as_ptr().cast(),
            ) != 0
    };
    if !gebaut {
        return Err(());
    }
    // SAFETY: valid handle with WRITE_DAC and WRITE_OWNER; the SID and the ACL live for the call; the system copies them.
    let rc = unsafe {
        SetSecurityInfo(
            griff.as_raw_handle(),
            SE_FILE_OBJECT,
            OWNER_SECURITY_INFORMATION
                | DACL_SECURITY_INFORMATION
                | PROTECTED_DACL_SECURITY_INFORMATION,
            eigen.as_ptr().cast_mut().cast(),
            std::ptr::null_mut(),
            acl.as_ptr().cast(),
            std::ptr::null(),
        )
    };
    if rc != 0 {
        return Err(());
    }
    let s = sicherheit_lesen(griff).map_err(|_| ())?;
    let genau_eigen = s.geschuetzt
        && s.besitzer.as_deref() == Some(eigen.as_slice())
        && s.eintraege.as_ref().is_some_and(|l| {
            l.len() == 1
                && l[0].typ == ACCESS_ALLOWED_ACE_TYPE
                && l[0].sid.as_deref() == Some(eigen.as_slice())
        });
    if genau_eigen { Ok(()) } else { Err(()) }
}

/// Binds, sets the list of the entry and listens (DI2 point 5, exit code 1 on every error; the entry is removed again).
pub(crate) fn socket_binden(pfad: &Path, wurzel: &Ordner) -> Result<UnixListener, StartFehler> {
    let _ = wurzel;
    let hoerer =
        UnixListener::binden(pfad).map_err(|e| StartFehler::neu(1, &format!("bind: {e}")))?;
    if socket_liste_setzen(pfad).is_err() {
        drop(hoerer);
        let _ = std::fs::remove_file(pfad);
        return Err(StartFehler::neu(
            1,
            "chmod: Zugriffsliste des Sockets nicht gesetzt",
        ));
    }
    if let Err(e) = hoerer.listen() {
        drop(hoerer);
        let _ = std::fs::remove_file(pfad);
        return Err(StartFehler::neu(1, &format!("listen: {e}")));
    }
    Ok(hoerer)
}

/// Binds the socket `pfad` with the list of the own user alone and listens, non-blocking (the socket of the report of `lauf`,
/// contract section 18). The entry is removed again on every error.
pub(crate) fn socket_binden_pfad(pfad: &Path) -> io::Result<UnixListener> {
    let hoerer = UnixListener::binden(pfad)?;
    let fertig = socket_liste_setzen(pfad)
        .map_err(|()| io::Error::from(io::ErrorKind::PermissionDenied))
        .and_then(|()| hoerer.listen())
        .and_then(|()| hoerer.set_nonblocking(true));
    if let Err(e) = fertig {
        drop(hoerer);
        let _ = std::fs::remove_file(pfad);
        return Err(e);
    }
    Ok(hoerer)
}

/// Why an entry is not a folder or file of the own user (the grounds of the Unix forms).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Eigen {
    /// The entry is missing.
    Fehlt,
    /// It cannot be opened for another reason.
    Unlesbar,
    /// A reparse point (link, junction, `AF_UNIX` entry).
    Link,
    /// A folder where a file is wanted, or the other way round.
    Art,
    /// Another owner.
    Fremd,
    /// The list is not the counterpart of 0700 (folder) or 0600 (file).
    Offen,
}

/// Opens `pfad` itself (no reparse point followed) with read access when `lesen`, and checks that it is a folder (`ordner`) or a
/// file of the own user with a protected list: a folder allows only the own user and `SYSTEM` (0700), a file only the own user (0600).
pub(crate) fn eigen_oeffnen(pfad: &Path, ordner: bool, lesen: bool) -> Result<File, Eigen> {
    eigen_pruefen(pfad, ordner, lesen, true)
}

/// Like [`eigen_oeffnen`] without the check of the list: a folder or file of the own user, no reparse point (the rule of the old
/// target list, DI12 point 15a, which under Unix checks the owner and not the mode).
pub(crate) fn eigen_besitz(pfad: &Path, ordner: bool) -> bool {
    eigen_pruefen(pfad, ordner, false, false).is_ok()
}

fn eigen_pruefen(pfad: &Path, ordner: bool, lesen: bool, liste: bool) -> Result<File, Eigen> {
    const FILE_GENERIC_READ: u32 = 0x0012_0089;
    let zugriff = READ_CONTROL
        | FILE_READ_ATTRIBUTES
        | SYNCHRONIZE
        | if lesen { FILE_GENERIC_READ } else { 0 };
    let griff = OpenOptions::new()
        .access_mode(zugriff)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
        .open(pfad)
        .map_err(|e| {
            if e.kind() == io::ErrorKind::NotFound {
                Eigen::Fehlt
            } else {
                Eigen::Unlesbar
            }
        })?;
    let info = attribut_tag(&griff).map_err(|_| Eigen::Unlesbar)?;
    if info.attribute & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
        return Err(Eigen::Link);
    }
    if (info.attribute & FILE_ATTRIBUTE_DIRECTORY != 0) != ordner {
        return Err(Eigen::Art);
    }
    let eigen = eigene_sid().map_err(|_| Eigen::Unlesbar)?;
    let s = sicherheit_lesen(&griff).map_err(|_| Eigen::Unlesbar)?;
    if s.besitzer.as_deref() != Some(eigen.as_slice()) {
        return Err(Eigen::Fremd);
    }
    if !liste {
        return Ok(griff);
    }
    let gut = if ordner {
        liste_nur_eigen(&s, &eigen)
    } else {
        s.geschuetzt
            && s.eintraege.as_ref().is_some_and(|l| {
                l.iter().all(|e| match (e.typ, e.sid.as_deref()) {
                    (ACCESS_DENIED_ACE_TYPE, Some(_)) => true,
                    (ACCESS_ALLOWED_ACE_TYPE, Some(sid)) => sid == eigen.as_slice(),
                    _ => false,
                }) && eigen_darf(l, &eigen, LESEN_SCHREIBEN)
            })
    };
    if gut { Ok(griff) } else { Err(Eigen::Offen) }
}

/// DI12 points 13 and 15 under Windows (addendum of 25.11.2026): whether the access list of the folder `ordner` lets the own user
/// look into it (`FILE_TRAVERSE` and `FILE_LIST_DIRECTORY`, the counterpart of the search bit). A `lstat` alone does not tell:
/// in a folder without an entry for the own user a missing name answers `NotFound` (measured as an ordinary user), and a process
/// with `SeBackupPrivilege` enabled opens everything. So the folder is held open, its descriptor is read on the handle, and
/// `AccessCheck` checks the two rights against the token of the process (groups, deny entries and owner rights included,
/// backup privileges not). A link to the folder is followed, as the path of a `lstat` follows it.
///
/// # Errors
/// `NotFound` when the folder is missing, `NotADirectory` when it is no folder, `PermissionDenied` when its list does not grant
/// both rights, any other error of the open, of the descriptor or of the check.
pub(crate) fn ordner_durchsuchbar(ordner: &Path) -> io::Result<()> {
    const FILE_TRAVERSE: u32 = 0x0020;
    let griff = OpenOptions::new()
        .access_mode(READ_CONTROL | FILE_READ_ATTRIBUTES | SYNCHRONIZE)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS)
        .open(ordner)?;
    if attribut_tag(&griff)?.attribute & FILE_ATTRIBUTE_DIRECTORY == 0 {
        return Err(io::Error::from(io::ErrorKind::NotADirectory));
    }
    if zugriff_erlaubt(&griff, FILE_TRAVERSE | FILE_LIST_DIRECTORY)? {
        Ok(())
    } else {
        Err(io::Error::from(io::ErrorKind::PermissionDenied))
    }
}

/// Whether the descriptor of `griff` grants the token of this process every right of `gewuenscht` (`AccessCheck`).
fn zugriff_erlaubt(griff: &File, gewuenscht: u32) -> io::Result<bool> {
    let mut beschreibung: *mut c_void = std::ptr::null_mut();
    // SAFETY: valid handle with READ_CONTROL; only the descriptor is asked for (owner, group and list lie inside it); the out
    // pointer lives for the call, and the descriptor is freed below with LocalFree.
    let rc = unsafe {
        GetSecurityInfo(
            griff.as_raw_handle(),
            SE_FILE_OBJECT,
            OWNER_SECURITY_INFORMATION | GROUP_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            &raw mut beschreibung,
        )
    };
    if rc != 0 {
        return Err(io::Error::from_raw_os_error(i32::try_from(rc).unwrap_or(0)));
    }
    let ergebnis = token_pruefen(beschreibung, gewuenscht);
    // SAFETY: `beschreibung` was allocated by `GetSecurityInfo` and is freed once; nothing points into it after this.
    unsafe { LocalFree(beschreibung) };
    ergebnis
}

/// `AccessCheck` of the descriptor `beschreibung` (from `GetSecurityInfo`, with owner, group and list) for an impersonation copy
/// of the token of this process.
fn token_pruefen(beschreibung: *mut c_void, gewuenscht: u32) -> io::Result<bool> {
    let mut token: *mut c_void = std::ptr::null_mut();
    // SAFETY: the pseudo handle of this process; `token` receives a new handle that is closed below.
    if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_DUPLICATE, &raw mut token) } == 0 {
        return Err(io::Error::last_os_error());
    }
    let mut kopie: *mut c_void = std::ptr::null_mut();
    // SAFETY: `token` is valid with TOKEN_DUPLICATE; `kopie` receives a new impersonation token that is closed below.
    let kopiert = unsafe { DuplicateToken(token, SECURITY_IMPERSONATION, &raw mut kopie) } != 0;
    let fehler = io::Error::last_os_error();
    // SAFETY: `token` came from `OpenProcessToken` and is closed once.
    unsafe { CloseHandle(token) };
    if !kopiert {
        return Err(fehler);
    }
    let abbildung = AbbildungAllgemein {
        lesen: 0x0012_0089,
        schreiben: 0x0012_0116,
        ausfuehren: 0x0012_00A0,
        alles: FILE_ALL_ACCESS,
    };
    // PRIVILEGE_SET: AccessCheck fills it only when a privilege grants a right; 64 bytes hold its head and several entries
    let mut rechte = [0_u32; 16];
    let mut laenge = u32::try_from(size_of_val(&rechte)).unwrap_or(0);
    let mut gewaehrt: u32 = 0;
    let mut erlaubt: i32 = 0;
    // SAFETY: `beschreibung` is the valid descriptor from `GetSecurityInfo` with owner, group and list; `kopie` is an
    // impersonation token with TOKEN_QUERY; the mapping, the buffer (DWORD aligned, its length passed) and the out values live
    // for the call.
    let geprueft = unsafe {
        AccessCheck(
            beschreibung,
            kopie,
            gewuenscht,
            &raw const abbildung,
            rechte.as_mut_ptr().cast(),
            &raw mut laenge,
            &raw mut gewaehrt,
            &raw mut erlaubt,
        )
    } != 0;
    let fehler = io::Error::last_os_error();
    // SAFETY: `kopie` came from `DuplicateToken` and is closed once.
    unsafe { CloseHandle(kopie) };
    if !geprueft {
        return Err(fehler);
    }
    Ok(erlaubt != 0 && gewaehrt & gewuenscht == gewuenscht)
}

/// The Unix mode the list of `pfad` stands for: `0700` (a folder of the own user that only it and `SYSTEM` reach), `0600` (a
/// file only the own user reaches) or `offen`. For the report of the fixed child `lauf` (test build).
#[cfg_attr(not(feature = "pruefbau"), allow(dead_code))]
pub(crate) fn modus_text(pfad: &Path) -> String {
    let ordner = std::fs::symlink_metadata(pfad).is_ok_and(|m| m.is_dir());
    match eigen_oeffnen(pfad, ordner, false) {
        Ok(_) if ordner => "0700".to_owned(),
        Ok(_) => "0600".to_owned(),
        Err(_) => "offen".to_owned(),
    }
}

/// Makes the folder `pfad` new (never an existing one) with the own user as owner and a protected list of the own user alone,
/// inherited by what is made inside (the counterpart of `mkdir` with 0700). A folder this call made and could not set is removed.
pub(crate) fn ordner_anlegen_eigen(pfad: &Path) -> io::Result<()> {
    std::fs::create_dir(pfad)?;
    let gesetzt = OpenOptions::new()
        .access_mode(READ_CONTROL | WRITE_DAC | WRITE_OWNER | FILE_READ_ATTRIBUTES)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
        .open(pfad)
        .ok()
        .filter(|g| attribut_tag(g).is_ok_and(|i| i.attribute & FILE_ATTRIBUTE_REPARSE_POINT == 0))
        .is_some_and(|g| liste_eigen_setzen(&g, true).is_ok());
    if gesetzt {
        Ok(())
    } else {
        let _ = std::fs::remove_dir(pfad);
        Err(io::Error::from(io::ErrorKind::PermissionDenied))
    }
}

/// Makes the file `pfad` new (`CREATE_NEW`, never an existing one) with the own user as owner and a protected list of the own user
/// alone (the counterpart of `O_CREAT | O_EXCL` with 0600), then writes `inhalt`. A file this call made and could not fill is removed.
pub(crate) fn datei_anlegen_eigen(pfad: &Path, inhalt: &[u8]) -> io::Result<()> {
    const FILE_GENERIC_WRITE: u32 = 0x0012_0116;
    let mut f = OpenOptions::new()
        .access_mode(FILE_GENERIC_WRITE | READ_CONTROL | WRITE_DAC | WRITE_OWNER)
        .share_mode(FILE_SHARE_READ)
        .write(true)
        .create_new(true)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
        .open(pfad)?;
    let fertig = liste_eigen_setzen(&f, false)
        .map_err(|()| io::Error::from(io::ErrorKind::PermissionDenied))
        .and_then(|()| f.write_all(inhalt));
    if let Err(e) = fertig {
        drop(f);
        let _ = std::fs::remove_file(pfad);
        return Err(e);
    }
    Ok(())
}

/// `STD_INPUT_HANDLE` and `STD_OUTPUT_HANDLE` (-10 and -11 as `DWORD`).
#[cfg(feature = "pruefbau")]
const STD_EINGABE: u32 = 0xFFFF_FFF6;
#[cfg(feature = "pruefbau")]
const STD_AUSGABE: u32 = 0xFFFF_FFF5;

/// The fixed child programs of the tests (test build): the counterpart of `stty raw -echo`. The input of the console gives every
/// character at once, without echo and without the processing of Ctrl+C (`ENABLE_LINE_INPUT`, `ENABLE_ECHO_INPUT`,
/// `ENABLE_PROCESSED_INPUT` off, `ENABLE_VIRTUAL_TERMINAL_INPUT` on); the output takes escape sequences and does not add a carriage
/// return to a line feed (`ENABLE_VIRTUAL_TERMINAL_PROCESSING`, `DISABLE_NEWLINE_AUTO_RETURN`).
#[cfg(feature = "pruefbau")]
pub(crate) fn konsole_roh() {
    const ENABLE_PROCESSED_INPUT: u32 = 0x1;
    const ENABLE_LINE_INPUT: u32 = 0x2;
    const ENABLE_ECHO_INPUT: u32 = 0x4;
    const ENABLE_VIRTUAL_TERMINAL_INPUT: u32 = 0x200;
    const ENABLE_VIRTUAL_TERMINAL_PROCESSING: u32 = 0x4;
    const DISABLE_NEWLINE_AUTO_RETURN: u32 = 0x8;
    for (nummer, weg, dazu) in [
        (
            STD_EINGABE,
            ENABLE_PROCESSED_INPUT | ENABLE_LINE_INPUT | ENABLE_ECHO_INPUT,
            ENABLE_VIRTUAL_TERMINAL_INPUT,
        ),
        (
            STD_AUSGABE,
            0,
            ENABLE_VIRTUAL_TERMINAL_PROCESSING | DISABLE_NEWLINE_AUTO_RETURN,
        ),
    ] {
        let mut modus: u32 = 0;
        // SAFETY: a standard handle of this process (or null, which the calls refuse); `modus` lives for the call.
        unsafe {
            let griff = GetStdHandle(nummer);
            if GetConsoleMode(griff, &raw mut modus) != 0 {
                SetConsoleMode(griff, (modus & !weg) | dazu);
            }
        }
    }
}

/// The fixed child programs of the tests (test build): the counterpart of `stty size`, the window of the console as rows and
/// columns.
#[cfg(feature = "pruefbau")]
pub(crate) fn konsole_groesse() -> Option<(u16, u16)> {
    let mut info = KonsolenInfo::default();
    // SAFETY: a standard handle of this process; `info` has the layout of `CONSOLE_SCREEN_BUFFER_INFO` and lives for the call.
    if unsafe { GetConsoleScreenBufferInfo(GetStdHandle(STD_AUSGABE), &raw mut info) } == 0 {
        return None;
    }
    let [links, oben, rechts, unten] = info.fenster;
    let zeilen = u16::try_from(i32::from(unten) - i32::from(oben) + 1).ok()?;
    let spalten = u16::try_from(i32::from(rechts) - i32::from(links) + 1).ok()?;
    Some((zeilen, spalten))
}

// ---- Moved here from the other modules (decision of 25.11.2026) -------------------------------------------------------
//
// The other modules of the crate hold no Windows logic: where Windows needs another body than Unix, the body stands here and the
// module only calls it behind `cfg(windows)` (the rule is checked by `tests/sys_regel.rs`). The mutation gate does not look into this
// file (`rust/.cargo/mutants.toml`); its cases run on the Windows test machine. Nothing below holds `unsafe`.

/// Windows (section 21a point 4): the state of the job of a session. `Ok(None)` for the room core, for a child that had ended before it
/// could join, and once the session has ended. `Err`: the child runs, but its job could not be made or the child could not join it;
/// the caller ends the child and refuses the start (fail closed).
pub(crate) type JobStand = Result<Option<Job>, ()>;

/// Windows: no number of the user (the owner is a SID, see [`eigene_sid`]).
pub(crate) fn euid() -> u32 {
    0
}

/// Windows has no device and inode in `Metadata` of `std` (section 21): two files never count as the same, so no pid file is read
/// and no signal goes out (as there is no signal under Windows anyway).
pub(crate) fn gleiche_datei(_a: &Metadata, _b: &Metadata) -> bool {
    false
}

/// Windows: no process group and no signal (section 21).
pub(crate) fn gruppe_beenden(_pid: u32) -> bool {
    false
}

/// Windows: the nanoseconds of a wait are an `i32` in `rustix` (below 10^9, so they fit).
pub(crate) fn nanosekunden(frist: Duration) -> i32 {
    i32::try_from(frist.subsec_nanos()).unwrap_or(0)
}

/// The check of the program of the child (DI8): Windows has no execute bit, so a file is a program (section 21). Always `Some`; the
/// match behind the call in `kind_vorpruefen` is the Unix check and is not reached.
// the caller in `sitzung.rs` takes `Some` as the verdict and falls through to the Unix match on `None`; Windows always has a verdict
#[allow(clippy::unnecessary_wraps)]
pub(crate) fn programm_urteil(programm: &str) -> Option<Result<(), StartFehler>> {
    let kein_kind =
        |text: &str| StartFehler::neu(4, &format!("Kind laesst sich nicht starten: {text}"));
    Some(match std::fs::metadata(programm) {
        Ok(m) if m.is_file() => Ok(()),
        Ok(_) => Err(kein_kind("Programm nicht ausfuehrbar")),
        Err(_) => Err(kein_kind("Programm fehlt")),
    })
}

/// Windows (section 21): the argv of the start file as it is. `portable-pty` adds `SHELL` only under Unix, and the carrier clears
/// the environment, so the child gets exactly `kind.env` without the cover; the process of the carrier is the program itself, as
/// after `exec` under Unix.
pub(crate) fn kind_argv(start: &KindStart, _huelle: &str) -> Vec<String> {
    start.argv.clone()
}

/// The job of a new child. `Ok(None)`: the child had ended before it could join, so no process of it is left to hold (the session
/// runs without a job and reports the end as usual). `Err`: the job could not be made or set, or a running child could not join.
fn job_fuer(kind: &mut Kind) -> JobStand {
    let job = Job::neu().map_err(drop)?;
    let griff = kind.prozess_griff().ok_or(())?;
    match job.aufnehmen(griff) {
        Ok(()) => Ok(Some(job)),
        Err(_) if kind.prozess_beendet() => Ok(None),
        Err(_) => Err(()),
    }
}

/// The child of a new session joins its job right after its start (section 21a point 4); the room core has no child and no job.
pub(crate) fn kind_mit_job(kind: Option<Kind>) -> (Option<Kind>, JobStand) {
    let mut kind = kind;
    let job = kind.as_mut().map_or(Ok(None), job_fuer);
    (kind, job)
}

/// The end of the session closes the job: it ends what the child left behind (the counterpart of the orphans of section 17).
pub(crate) fn job_schliessen(job: &mut JobStand) {
    *job = Ok(None);
}

/// `Beenden` (section 21): the carrier ends a child that is still there at once (`TerminateProcess`, code 129).
pub(crate) fn kind_beenden(kind: &mut Option<Kind>) {
    if let Some(kind) = kind.as_mut().filter(|k| !k.ist_zu_ende()) {
        let _ = kind.beenden();
    }
}

/// The moment of SIGKILL of the Unix form: once more through the carrier (it does nothing to a child that has ended).
pub(crate) fn kind_nochmal_beenden(kind: &mut Option<Kind>) {
    if let Some(kind) = kind.as_mut() {
        let _ = kind.beenden();
    }
}

/// Ends children (DI8, section 21): the carrier ends every running child at once.
pub(crate) fn kinder_beenden(kinder: &mut [Kind]) {
    for kind in kinder.iter_mut().filter(|k| !k.ist_zu_ende()) {
        let _ = kind.beenden();
    }
}

/// The session of `kind` at the start of the service. A child without a job is a start error (section 21a point 4, fail closed; DI8:
/// exit code 4): the child is ended and nothing stays.
pub(crate) fn mit_job(sitzung: Sitzung) -> Result<Sitzung, StartFehler> {
    if sitzung.job.is_err() {
        start_verwerfen(Dienst::kinder_nehmen(&mut [sitzung]));
        return Err(StartFehler::neu(
            4,
            "Kind laesst sich nicht starten: kein Job-Objekt",
        ));
    }
    Ok(sitzung)
}

/// The new session of `Starten` (section 21a point 4, fail closed). A child without a job is ended, the folder of the session
/// removed and the start refused; then `None`. Otherwise the session itself.
pub(crate) fn sitzung_mit_job(dienst: &mut Dienst, sitzung: Sitzung, nr: u64) -> Option<Sitzung> {
    if sitzung.job.is_ok() {
        return Some(sitzung);
    }
    let mut sitzung = sitzung;
    let kennung = sitzung.sid.clone().unwrap_or_default();
    for sid in kinder_verwerfen(Dienst::kinder_nehmen(std::slice::from_mut(&mut sitzung))) {
        dienst.waisen.jagen(sid, Instant::now());
    }
    drop(sitzung);
    crate::loeschen::sitzung_entfernen(dienst.wurzel.as_fd(), &kennung);
    dienst.start_ablehnen(nr, &Abgelehnt::fest("programm_nicht_startbar"));
    None
}

/// The caught console events: the flag that the handler of `SIGINT`, `SIGTERM` and `SIGBREAK` sets.
pub struct Signale(Arc<AtomicBool>);

/// Catches `SIGINT` (`CTRL_C_EVENT`), `SIGBREAK` (`CTRL_BREAK_EVENT`) and `SIGTERM` (only raised inside the process) with the safe
/// API of `signal-hook` (Nachtrag 25.11.2026, contract section 21).
///
/// # Errors
/// The error of the system when a handler cannot be set.
pub fn signale_anmelden() -> io::Result<Signale> {
    use signal_hook::consts::{SIGBREAK, SIGINT, SIGTERM};
    let flagge = Arc::new(AtomicBool::new(false));
    for s in [SIGINT, SIGTERM, SIGBREAK] {
        signal_hook::flag::register(s, Arc::clone(&flagge))?;
    }
    Ok(Signale(flagge))
}

/// The loop until one of the console events `CTRL_C_EVENT` or `CTRL_BREAK_EVENT` comes (`SIGINT` and `SIGBREAK` of the C runtime,
/// which stand for SIGINT and SIGTERM); then the same end as under Unix. A thread looks at the flag of the handler every 5 ms and
/// calls [`crate::Anhalter::anhalten`].
pub(crate) fn laufen(dienst: &mut Dienst, signale: Signale) -> i32 {
    let anhalter = dienst.anhalter();
    let fertig = Arc::new(AtomicBool::new(false));
    let fertig_faden = Arc::clone(&fertig);
    let faden = std::thread::spawn(move || {
        while !fertig_faden.load(Ordering::SeqCst) {
            if signale.0.load(Ordering::SeqCst) {
                anhalter.anhalten();
                return;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    });
    let code = dienst.schleife();
    fertig.store(true, Ordering::SeqCst);
    let _ = faden.join();
    code
}

/// The same socket of the report as `AF_UNIX` with the protected list of the own user alone (the counterpart of 0600) before `listen`;
/// the peer is checked by its process (`SIO_AF_UNIX_GETPEERPID`), the list keeps other users out.
///
/// # Errors
/// The path is longer than `SOCKET_PFAD_MAX` bytes, or the socket cannot be made, bound, set or listened on.
pub(crate) fn meldung_binden(pfad: &Path) -> Result<Meldung, ()> {
    if pfad.as_os_str().len() > SOCKET_PFAD_MAX {
        return Err(());
    }
    let hoerer = socket_binden_pfad(pfad).map_err(|_| ())?;
    Ok(Meldung {
        hoerer: Some(hoerer),
        strom: None,
        verbunden: false,
        puffer: Vec::new(),
        pid: 0,
        euid: 0,
    })
}

/// The process of the peer must be the child of the session; the user is checked by the list of the socket (only the own user may
/// connect). A failed query refuses the connection (fail closed).
pub(crate) fn gegenseite_gut(strom: &UnixStream, pid: u32, euid: u32) -> bool {
    let _ = euid;
    strom.peer_pid().is_ok_and(|p| p == pid)
}

/// The same steps as under Unix with access lists. The folder must be a folder of the own user whose protected list allows only it and
/// `SYSTEM` (0700), the file a file of the own user whose protected list allows only it (0600); a reparse point is never followed. A
/// missing folder or file gives no markers.
///
/// # Errors
/// Exit code 2 with the same grounds as under Unix.
pub(crate) fn marker_lesen(
    pfad: &str,
    ordner: Option<&str>,
) -> Result<Option<start::Marker>, StartFehler> {
    let p = Path::new(pfad);
    let (Some(eltern), Some(name)) = (p.parent(), p.file_name().and_then(|n| n.to_str())) else {
        return Err(ungueltig("pfad"));
    };
    if !crate::loeschen::name_gut(name) {
        return Err(ungueltig("pfad"));
    }
    let _eltern = match eigen_oeffnen(eltern, true, false) {
        Ok(g) => g,
        Err(Eigen::Fehlt) => return Ok(None),
        Err(Eigen::Fremd) => return Err(ungueltig("ordner_eigentuemer")),
        Err(Eigen::Offen) => return Err(ungueltig("ordner_modus")),
        Err(_) => return Err(ungueltig("ordner")),
    };
    let datei = match eigen_oeffnen(p, false, true) {
        Ok(g) => g,
        Err(Eigen::Fehlt) => return Ok(None),
        Err(Eigen::Link) => return Err(ungueltig("datei_verknuepfung")),
        Err(Eigen::Art) => return Err(ungueltig("datei_art")),
        Err(Eigen::Fremd) => return Err(ungueltig("datei_eigentuemer")),
        Err(Eigen::Offen) => return Err(ungueltig("datei_modus")),
        Err(Eigen::Unlesbar) => return Err(ungueltig("datei_unlesbar")),
    };
    let mut roh = Vec::new();
    datei
        .take(DATEI_MAX as u64 + 1)
        .read_to_end(&mut roh)
        .map_err(|_| ungueltig("datei_unlesbar"))?;
    if roh.len() > DATEI_MAX {
        return Err(ungueltig("zu_gross"));
    }
    inhalt_lesen(&roh, ordner)
}

/// DI12 point 15a (section 21): the same rule as under Unix. The folder `lauf.ziellisten` must be a folder of the own user (no reparse
/// point); only a file of the own user (no reparse point, no folder) is removed. As under Unix the owner counts, not the list. The
/// removal goes by the path (`DeleteFileW` removes a reparse point itself, never its target).
pub(crate) fn alte_zielliste_raeumen(lauf: &Lauf, n: usize) -> bool {
    let Some(ordner) = lauf.ziellisten.as_deref() else {
        return false;
    };
    let liste = Path::new(ordner).join(zielliste_name(n));
    let andere_frei = eintraege_der_nummer(lauf, n)
        .iter()
        .filter(|p| **p != liste)
        .all(|p| matches!(std::fs::symlink_metadata(p), Err(e) if e.kind() == io::ErrorKind::NotFound));
    if !andere_frei || !eigen_besitz(Path::new(ordner), true) {
        return false;
    }
    let eigene_datei = eigen_besitz(&liste, false);
    eigene_datei && std::fs::remove_file(&liste).is_ok()
}

/// The folder `<ordner>/<kennung>` with `start.json` that holds the start structure with `sitzung` set, with access lists: the folder
/// new, with the protected list of the own user (0700), `start.json` new (`CREATE_NEW`) with the protected list of the own user
/// (0600). A folder that exists is never replaced or deleted; a folder this call made and could not fill is removed again.
///
/// # Errors
/// The folder exists, cannot be made, or the file cannot be written.
pub(crate) fn ordner_anlegen(
    ordner: &Path,
    wurzel: BorrowedFd<'_>,
    kennung: &str,
    start: &Startanfrage,
    marker: Option<&start::Marker>,
) -> Result<(PathBuf, PathBuf), ()> {
    let inhalt = start::schreiben(
        &start
            .clone()
            .mit_sitzung(kennung, marker.cloned())
            .map_err(|_| ())?,
    );
    let sitzung = ordner.join(kennung);
    ordner_anlegen_eigen(&sitzung).map_err(|_| ())?;
    let datei = sitzung.join("start.json");
    if datei_anlegen_eigen(&datei, &inhalt).is_ok() {
        Ok((sitzung, datei))
    } else {
        crate::loeschen::sitzung_entfernen(wurzel, kennung);
        Err(())
    }
}

/// DI12 point 15a (section 21b): the folder of the entry `pfad` must grant the own user the right to look into it (the counterpart
/// of the search bit). A folder that is missing leaves the entry free, as under Unix; any other error refuses the start.
///
/// # Errors
/// The folder cannot be looked into for another reason than that it is missing.
pub(crate) fn eintrag_durchsuchbar(pfad: &Path) -> Result<(), ()> {
    if let Some(ordner) = pfad.parent() {
        match ordner_durchsuchbar(ordner) {
            Err(e) if e.kind() != io::ErrorKind::NotFound => return Err(()),
            _ => {}
        }
    }
    Ok(())
}

// ---- The same for the fixed child programs of the tests (test build) ---------------------------------------------------

/// The two uses of `stty` through the console: `raw -echo` and `size` (rows and columns, as `stty size` prints them).
#[cfg(feature = "pruefbau")]
pub(crate) fn stty(argumente: &[&str]) -> String {
    match argumente {
        ["raw", "-echo"] => {
            konsole_roh();
            String::new()
        }
        ["size"] => konsole_groesse()
            .map(|(zeilen, spalten)| format!("{zeilen} {spalten}"))
            .unwrap_or_default(),
        _ => String::new(),
    }
}

/// A path as the report writes it: in the form of the start file, without the drive and with `/` (`C:\\t\\o` is `/t/o`), the way the
/// service reads a path of the start file on the drive of the working folder.
#[cfg(feature = "pruefbau")]
pub(crate) fn pfad_text(p: &Path) -> String {
    let text = p.display().to_string();
    let ohne = text.get(1..2) == Some(":");
    let rest = if ohne { &text[2..] } else { text.as_str() };
    rest.replace('\\', "/")
}

/// A link to a folder and a link to a file are two calls.
#[cfg(feature = "pruefbau")]
pub(crate) fn symlink(ziel: impl AsRef<Path>, name: impl AsRef<Path>) -> io::Result<()> {
    if ziel.as_ref().is_dir() {
        std::os::windows::fs::symlink_dir(ziel, name)
    } else {
        std::os::windows::fs::symlink_file(ziel, name)
    }
}

/// The mode the access list of `p` stands for (see [`modus_text`]), `Err` when `p` cannot be read.
#[cfg(feature = "pruefbau")]
pub(crate) fn modus_von(p: &Path) -> io::Result<String> {
    std::fs::metadata(p).map(|_| modus_text(p))
}

/// A program carries `.exe`; the report names it without, as the name of the program under Unix.
#[cfg(feature = "pruefbau")]
pub(crate) fn ohne_exe(name: String) -> String {
    match name.strip_suffix(".exe") {
        Some(ohne) => ohne.to_owned(),
        None => name,
    }
}

/// `signal <nr>`: no signal to itself; the child ends after 5 s by itself.
#[cfg(feature = "pruefbau")]
pub(crate) fn signal_an_sich(_a: &[String]) {
    std::thread::sleep(Duration::from_secs(5));
}

/// A copy of the program named `laufkind.exe` is the fixed child `lauf` with all its arguments; under Unix a small script does the
/// same with `exec`, which Windows does not have. `Some(code)` when this process is such a copy, with the exit code of the child.
#[cfg(feature = "pruefbau")]
pub(crate) fn laufkind() -> Option<i32> {
    let ist_kopie = std::env::current_exe()
        .ok()
        .and_then(|p| p.file_stem().map(|s| s.eq_ignore_ascii_case("laufkind")))
        .unwrap_or(false);
    if !ist_kopie {
        return None;
    }
    let mut rest = vec!["lauf".to_owned()];
    rest.extend(std::env::args_os().map(|a| a.to_string_lossy().into_owned()));
    Some(crate::kindprogramm::lauf(&rest) & 0xff)
}

// The cases of section 21a point 4 (moved from `dienst.rs` with the rule of section 21, point 17): test code of the Windows build.
/// Section 21a point 4 under Windows (fail closed): a child whose job cannot be made is ended, and its start is refused. The test
/// build reaches the error through `JOB_VERWEIGERN`; each case also runs without it, so the same start goes through there.
#[cfg(all(test, feature = "pruefbau"))]
mod dienst_tests {
    use super::*;
    use serde_json::{Value, json};

    /// A new folder of the own user alone (DI2) under the temporary folder, as a path of the start file (no drive, `/`).
    fn wegwerf(name: &str) -> (PathBuf, String) {
        let p = std::env::temp_dir().join(format!("wbd-job-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        ordner_anlegen_eigen(&p).expect("folder");
        let text = p.to_string_lossy().replace('\\', "/");
        let ohne_laufwerk = text
            .split_once(':')
            .map_or(text.as_str(), |(_, r)| r)
            .to_owned();
        (p, ohne_laufwerk)
    }

    /// The processes whose command line holds `marke` (the asking PowerShell left out).
    fn prozesse_mit(marke: &str) -> usize {
        let abfrage = format!(
            "@(Get-CimInstance Win32_Process | Where-Object {{ $_.Name -ne 'powershell.exe' -and $_.CommandLine -like '*{marke}*' }}).Count"
        );
        let aus = std::process::Command::new("powershell.exe")
            .args(["-NoProfile", "-NonInteractive", "-Command", &abfrage])
            .output()
            .expect("powershell runs");
        String::from_utf8_lossy(&aus.stdout)
            .trim()
            .parse()
            .expect("a number")
    }

    /// Waits up to 10 s until `prozesse_mit(marke)` is `anzahl`.
    fn warte_auf(marke: &str, anzahl: usize) -> bool {
        let frist = Instant::now() + Duration::from_secs(10);
        loop {
            if prozesse_mit(marke) == anzahl {
                return true;
            }
            if Instant::now() > frist {
                return false;
            }
            std::thread::sleep(Duration::from_millis(200));
        }
    }

    fn grundlage(ordner: &str) -> Value {
        json!({
            "socket": {"ordner": ordner},
            "betrachter": {"person": "anna", "raum": "raum-nord", "ansicht": "fenster"},
            "deflate": false,
            "pruefmodus": true,
            "anfang": {"personen": {"anna": {"art": "mensch", "rolle": "nutzer"}}, "sitzungen": []}
        })
    }

    fn verweigern(an: bool) {
        JOB_VERWEIGERN.with(|v| v.set(an));
    }

    /// The session of `kind`: without a job the service does not start (exit code 4, DI8), the child is ended and no socket is
    /// left. Without the error the same start file starts the service with the child running.
    #[test]
    fn ohne_job_startet_der_dienst_nicht_und_das_kind_endet() {
        let (p, ordner) = wegwerf("kind");
        let marke = format!("wbdjob{}kind", std::process::id());
        let mut start = grundlage(&ordner);
        start["kind"] = json!({
            "argv": ["/Windows/System32/cmd.exe", "/k", "rem", marke],
            "env": {"SystemRoot": "C:\\Windows"},
            "cwd": "/Windows"
        });
        start["groesse"] = json!([40, 10]);
        start["anfang"]["sitzungen"] = json!([{"id": "s-alpha", "kundenraum": "raum-nord", "person": "anna", "route": "r1", "titel": "Alpha"}]);
        let cfg = || crate::startdatei::pruefen(&start).expect("start file");
        verweigern(true);
        let fehler = Dienst::neu(cfg()).err().expect("refused without a job");
        verweigern(false);
        assert_eq!(fehler.code, 4, "{}", fehler.text);
        assert!(!p.join("dienst.sock").exists(), "no socket is left");
        assert!(warte_auf(&marke, 0), "the child has ended");
        let dienst = Dienst::neu(cfg()).expect("the service starts with a job");
        assert!(p.join("dienst.sock").exists(), "the socket is there");
        assert!(warte_auf(&marke, 1), "the child runs");
        drop(dienst);
        assert!(
            warte_auf(&marke, 0),
            "the end of the service ends the child"
        );
        let _ = std::fs::remove_dir_all(&p);
    }

    /// A session of `Starten`: without a job the start is refused with `programm_nicht_startbar`, the child is ended and its
    /// folder removed; no session and no number are taken. Without the error the same start makes the session `s-1`.
    #[test]
    fn ohne_job_lehnt_der_dienst_starten_ab_und_das_kind_endet() {
        let (p, ordner) = wegwerf("lauf");
        let marke = format!("wbd-job-{}-lauf", std::process::id());
        let einst = p.join("einst.json");
        std::fs::write(
            &einst,
            include_str!("../../tests/werkzeug/einstellungen/vorfuehrung.json"),
        )
        .expect("settings");
        let mut start = grundlage(&ordner);
        start["lauf"] = json!({
            "programm": "/Windows/System32/cmd.exe",
            "env": {"SystemRoot": "C:\\Windows"},
            "groesse": [40, 10],
            "einstellungen": format!("{ordner}/einst.json"),
        });
        let mut dienst =
            Dienst::neu(crate::startdatei::pruefen(&start).expect("start file")).expect("service");
        let ereignis = json!({"art": "Starten", "start": {"fassung": 1, "harness": "claude", "modell": "m", "route": "cloud-beispiel", "kunde": "k-beispiel", "arbeitsordner": "/Windows"}});
        let ereignis = ereignis.as_object().expect("object");
        verweigern(true);
        dienst.starten(1, ereignis);
        verweigern(false);
        assert_eq!(dienst.sitzungen.len(), 1, "only the room core");
        assert_eq!(dienst.naechste, 0, "no identifier is taken");
        assert!(
            !p.join("s-1").exists(),
            "the folder of the session is removed"
        );
        assert!(warte_auf(&marke, 0), "the child has ended");
        dienst.starten(1, ereignis);
        assert_eq!(dienst.sitzungen.len(), 2, "the session s-1 runs");
        assert!(p.join("s-1").exists(), "its folder is there");
        assert!(warte_auf(&marke, 1), "the child runs");
        drop(dienst);
        assert!(
            warte_auf(&marke, 0),
            "the end of the service ends the child"
        );
        let _ = std::fs::remove_dir_all(&p);
    }
}

// test code: short names for folder, path, listener and the two ends read better here
#[cfg(test)]
#[allow(clippy::many_single_char_names)]
mod tests {
    use super::*;

    /// A job ends its process when the job closes; a process that has ended cannot join.
    #[test]
    fn ein_job_beendet_beim_schliessen_den_prozess() {
        let mut kind = std::process::Command::new("cmd.exe")
            .args(["/c", "ping -n 600 127.0.0.1 >NUL"])
            .spawn()
            .expect("cmd starts");
        let job = Job::fuer(kind.as_raw_handle()).expect("the child joins the job");
        std::thread::sleep(Duration::from_millis(500));
        assert!(kind.try_wait().expect("asked").is_none(), "cmd runs");
        drop(job);
        let frist = Instant::now() + Duration::from_secs(10);
        while kind.try_wait().expect("asked").is_none() {
            assert!(Instant::now() < frist, "the job did not end cmd");
            std::thread::sleep(Duration::from_millis(20));
        }
        let mut fertig = std::process::Command::new("cmd.exe")
            .args(["/c", "exit 0"])
            .spawn()
            .expect("cmd starts");
        fertig.wait().expect("cmd ends");
        assert!(
            Job::fuer(fertig.as_raw_handle()).is_err(),
            "an ended process does not join"
        );
    }

    fn wegwerf(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("wbd-sys-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        p
    }

    #[test]
    fn ein_neuer_ordner_und_eine_neue_datei_gehoeren_nur_dem_eigenen_nutzer() {
        let o = wegwerf("anlegen");
        ordner_anlegen_eigen(&o).expect("folder");
        assert!(
            ordner_anlegen_eigen(&o).is_err(),
            "an existing folder is never made again"
        );
        assert_eq!(eigen_oeffnen(&o, true, false).map(|_| ()), Ok(()));
        assert_eq!(modus_text(&o), "0700");
        let d = o.join("start.json");
        datei_anlegen_eigen(&d, b"{}").expect("file");
        assert!(
            datei_anlegen_eigen(&d, b"{}").is_err(),
            "an existing file is never replaced"
        );
        assert_eq!(eigen_oeffnen(&d, false, true).map(|_| ()), Ok(()));
        assert_eq!(modus_text(&d), "0600");
        assert_eq!(std::fs::read(&d).expect("read"), b"{}");
        assert_eq!(eigen_oeffnen(&d, true, false).map(|_| ()), Err(Eigen::Art));
        let _ = std::fs::remove_dir_all(&o);
    }

    #[test]
    fn ein_ordner_ohne_eintrag_des_eigenen_nutzers_ist_nicht_durchsuchbar() {
        let o = wegwerf("durchsuchen");
        assert_eq!(
            ordner_durchsuchbar(&o).map_err(|e| e.kind()),
            Err(io::ErrorKind::NotFound)
        );
        ordner_anlegen_eigen(&o).expect("folder");
        std::fs::create_dir(o.join("s-1")).expect("entry");
        assert_eq!(ordner_durchsuchbar(&o).map_err(|e| e.kind()), Ok(()));
        // the counterpart of mode 0000: a protected list with `SYSTEM` alone; the owner keeps the right to change the list
        let icacls = |a: &[&str]| {
            std::process::Command::new("icacls")
                .arg(&o)
                .args(a)
                .output()
                .is_ok_and(|x| x.status.success())
        };
        let nutzer = std::env::var("USERNAME").expect("USERNAME");
        assert!(icacls(&[
            "/inheritance:r",
            "/grant:r",
            "*S-1-5-18:(OI)(CI)(F)"
        ]));
        assert!(icacls(&["/remove:g", &nutzer]));
        // also with `SeBackupPrivilege` enabled (an elevated session over ssh): the list decides, not the privilege
        assert_eq!(
            ordner_durchsuchbar(&o).map_err(|e| e.kind()),
            Err(io::ErrorKind::PermissionDenied)
        );
        let datei = o.with_extension("datei");
        std::fs::write(&datei, b"x").expect("file");
        assert_eq!(
            ordner_durchsuchbar(&datei).map_err(|e| e.kind()),
            Err(io::ErrorKind::NotADirectory)
        );
        let _ = std::fs::remove_file(&datei);
        assert!(icacls(&["/grant", &format!("{nutzer}:(OI)(CI)(F)")]));
        assert_eq!(ordner_durchsuchbar(&o).map_err(|e| e.kind()), Ok(()));
        let _ = std::fs::remove_dir_all(&o);
    }

    #[test]
    fn der_bericht_des_sockets_lauf_hoert_mit_der_liste_des_eigenen_nutzers() {
        let o = wegwerf("meldung");
        ordner_anlegen_eigen(&o).expect("folder");
        let p = o.join("m.sock");
        let h = socket_binden_pfad(&p).expect("bind");
        let c = UnixStream::connect(&p).expect("connect");
        let mut angenommen = None;
        for _ in 0..200 {
            if let Ok((a, _)) = h.accept() {
                angenommen = Some(a);
                break;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        let a = angenommen.expect("accepted");
        assert_eq!(a.peer_pid().expect("peer"), std::process::id());
        drop((c, a, h));
        let _ = std::fs::remove_dir_all(&o);
    }
}
