//! Windows: the `AF_UNIX` client to the service and the access lists of the token folder and of the files the bridge makes
//! (Nachtrag 25.11.2026, contract `bruecke`, section of the platforms). This file is the only place of the crate with `unsafe`;
//! every block names what it relies on. The calls are the same as in `dienst/src/sys/windows.rs`; the two crates share no code
//! (a shared crate would be a new dependency).
//!
//! - The folder of a file belongs to the own user (owner SID), its access list is protected (no inherited entry) and allows nobody
//!   but the own user and `SYSTEM` (the counterpart of mode 0700).
//! - A file of the bridge (token, port) is made new (`CREATE_NEW`, never an existing one), gets the own user as owner and a protected
//!   list with exactly the own user (the counterpart of mode 0600), and the list is read back.
//! - The bridge holds the file open and removes it at the end only while the path still names that file (volume and file index).

use std::ffi::c_void;
use std::fs::{File, OpenOptions};
use std::io::{self, Read, Write};
use std::net::Shutdown;
use std::os::windows::fs::OpenOptionsExt;
use std::os::windows::io::{AsRawHandle, AsRawSocket, AsSocket, FromRawSocket, OwnedSocket};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::Duration;

type Socket = usize;
const INVALID_SOCKET: Socket = !0;
const AF_UNIX: u16 = 1;
const SOCK_STREAM: i32 = 1;
const WSA_FLAG_OVERLAPPED: u32 = 0x01;
const WSA_FLAG_NO_HANDLE_INHERIT: u32 = 0x80;
const SOL_SOCKET: i32 = 0xFFFF;
const SO_RCVTIMEO: i32 = 0x1006;
const SO_SNDTIMEO: i32 = 0x1005;
const PFAD_MAX: usize = 108;

const READ_CONTROL: u32 = 0x0002_0000;
const WRITE_DAC: u32 = 0x0004_0000;
const WRITE_OWNER: u32 = 0x0008_0000;
const SYNCHRONIZE: u32 = 0x0010_0000;
const FILE_LIST_DIRECTORY: u32 = 0x0001;
const FILE_READ_ATTRIBUTES: u32 = 0x0080;
const FILE_ALL_ACCESS: u32 = 0x001F_01FF;
const FILE_GENERIC_WRITE: u32 = 0x0012_0116;
const FILE_SHARE_READ: u32 = 0x1;
const FILE_SHARE_WRITE: u32 = 0x2;
const FILE_SHARE_DELETE: u32 = 0x4;
const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;
const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
const FILE_ATTRIBUTE_DIRECTORY: u32 = 0x10;
const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
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

/// `BY_HANDLE_FILE_INFORMATION`: attributes, three times, volume, size, links and the file index.
#[repr(C)]
#[derive(Default)]
struct DateiInfo {
    attribute: u32,
    zeiten: [u32; 6],
    volume: u32,
    groesse_hoch: u32,
    groesse_tief: u32,
    links: u32,
    index_hoch: u32,
    index_tief: u32,
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
    fn connect(s: Socket, name: *const c_void, laenge: i32) -> i32;
    fn recv(s: Socket, puffer: *mut u8, laenge: i32, flags: i32) -> i32;
    fn send(s: Socket, puffer: *const u8, laenge: i32, flags: i32) -> i32;
    fn setsockopt(s: Socket, ebene: i32, name: i32, wert: *const u8, laenge: i32) -> i32;
    fn shutdown(s: Socket, wie: i32) -> i32;
    fn WSAGetLastError() -> i32;
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
    fn GetTokenInformation(
        token: *mut c_void,
        klasse: i32,
        info: *mut c_void,
        laenge: u32,
        zurueck: *mut u32,
    ) -> i32;
}

#[link(name = "bcrypt")]
unsafe extern "system" {
    fn BCryptGenRandom(algorithmus: *mut c_void, puffer: *mut u8, laenge: u32, flags: u32) -> i32;
}

/// `BCRYPT_USE_SYSTEM_PREFERRED_RNG`: no algorithm handle, the generator of the system.
const BCRYPT_USE_SYSTEM_PREFERRED_RNG: u32 = 0x2;

/// `n` random bytes from the preferred generator of the system (the source of `std` for its hash keys, too).
pub(crate) fn zufall(n: usize) -> io::Result<Vec<u8>> {
    let mut puffer = vec![0_u8; n];
    let laenge = u32::try_from(n).map_err(|_| io::Error::from(io::ErrorKind::InvalidInput))?;
    // SAFETY: `puffer` holds `laenge` bytes and lives for the call; a null algorithm is allowed with this flag.
    let status = unsafe {
        BCryptGenRandom(
            std::ptr::null_mut(),
            puffer.as_mut_ptr(),
            laenge,
            BCRYPT_USE_SYSTEM_PREFERRED_RNG,
        )
    };
    if status == 0 {
        Ok(puffer)
    } else {
        Err(io::Error::other(format!("BCryptGenRandom: {status:#x}")))
    }
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetCurrentProcess() -> *mut c_void;
    fn CloseHandle(griff: *mut c_void) -> i32;
    fn LocalFree(speicher: *mut c_void) -> *mut c_void;
    fn GetFileInformationByHandleEx(
        griff: *mut c_void,
        klasse: i32,
        info: *mut c_void,
        laenge: u32,
    ) -> i32;
    fn GetFileInformationByHandle(griff: *mut c_void, info: *mut DateiInfo) -> i32;
}

/// `WSAStartup` once per process (version 2.2).
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

fn roh(s: &OwnedSocket) -> Socket {
    usize::try_from(s.as_raw_socket()).unwrap_or(INVALID_SOCKET)
}

/// A connected `AF_UNIX` stream to the service.
#[derive(Debug)]
pub(crate) struct UnixStream(OwnedSocket);

impl UnixStream {
    /// Connects to the socket at `pfad` (UTF-8, shorter than 108 bytes).
    pub(crate) fn connect(pfad: impl AsRef<Path>) -> io::Result<Self> {
        winsock()?;
        let text = pfad
            .as_ref()
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
        let s = unsafe { OwnedSocket::from_raw_socket(s as u64) };
        let laenge = i32::try_from(size_of::<SockaddrUn>()).unwrap_or(i32::MAX);
        // SAFETY: valid socket; `a` is a complete `sockaddr_un` that lives for the call.
        if unsafe { connect(roh(&s), (&raw const a).cast(), laenge) } != 0 {
            return Err(letzter_fehler());
        }
        Ok(Self(s))
    }

    /// A second handle of the same connection (`WSADuplicateSocketW` through `std`).
    pub(crate) fn try_clone(&self) -> io::Result<Self> {
        Ok(Self(self.0.as_socket().try_clone_to_owned()?))
    }

    pub(crate) fn set_read_timeout(&self, frist: Option<Duration>) -> io::Result<()> {
        self.frist(SO_RCVTIMEO, frist)
    }

    pub(crate) fn set_write_timeout(&self, frist: Option<Duration>) -> io::Result<()> {
        self.frist(SO_SNDTIMEO, frist)
    }

    fn frist(&self, name: i32, frist: Option<Duration>) -> io::Result<()> {
        let ms: u32 = match frist {
            Some(d) if d.is_zero() => {
                return Err(io::Error::new(io::ErrorKind::InvalidInput, "zero timeout"));
            }
            Some(d) => u32::try_from(d.as_millis()).unwrap_or(u32::MAX).max(1),
            None => 0,
        };
        let b = ms.to_ne_bytes();
        // SAFETY: valid socket, a DWORD of 4 bytes as the option value.
        if unsafe { setsockopt(roh(&self.0), SOL_SOCKET, name, b.as_ptr(), 4) } == 0 {
            Ok(())
        } else {
            Err(letzter_fehler())
        }
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
}

impl Read for UnixStream {
    fn read(&mut self, puffer: &mut [u8]) -> io::Result<usize> {
        let n = i32::try_from(puffer.len()).unwrap_or(i32::MAX);
        // SAFETY: valid socket; at most `n` bytes are written into `puffer`.
        let r = unsafe { recv(roh(&self.0), puffer.as_mut_ptr(), n, 0) };
        usize::try_from(r).map_err(|_| zeit_als_wouldblock(letzter_fehler()))
    }
}

impl Write for UnixStream {
    fn write(&mut self, puffer: &[u8]) -> io::Result<usize> {
        let n = i32::try_from(puffer.len()).unwrap_or(i32::MAX);
        // SAFETY: valid socket; at most `n` bytes are read from `puffer`.
        let r = unsafe { send(roh(&self.0), puffer.as_ptr(), n, 0) };
        usize::try_from(r).map_err(|_| zeit_als_wouldblock(letzter_fehler()))
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// A timeout of `SO_RCVTIMEO` is `WSAETIMEDOUT` (10060); `std` reports it as `TimedOut`, Unix as `WouldBlock`. The bridge treats
/// both alike; the kind of Unix keeps the code of the bridge the same on both systems.
fn zeit_als_wouldblock(e: io::Error) -> io::Error {
    if e.raw_os_error() == Some(10060) {
        io::Error::from(io::ErrorKind::WouldBlock)
    } else {
        e
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

/// Volume and file index of an open file: the identity of the file (the counterpart of device and inode).
fn kennung(datei: &File) -> io::Result<(u32, u64)> {
    let mut info = DateiInfo::default();
    // SAFETY: valid handle; `info` has the layout of `BY_HANDLE_FILE_INFORMATION` and lives for the call.
    if unsafe { GetFileInformationByHandle(datei.as_raw_handle(), &raw mut info) } == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok((
        info.volume,
        (u64::from(info.index_hoch) << 32) | u64::from(info.index_tief),
    ))
}

/// The SID of the user of this process, in its binary form.
fn eigene_sid() -> io::Result<Vec<u8>> {
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
    // SAFETY: `GetTokenInformation` succeeded, so the first pointer-sized field of `puffer` points at a valid SID in `puffer`.
    let sid = unsafe { *puffer.as_ptr().cast::<*const c_void>() };
    sid_kopie(sid).ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "invalid SID"))
}

fn sid_kopie(sid: *const c_void) -> Option<Vec<u8>> {
    if sid.is_null() {
        return None;
    }
    // SAFETY: `sid` is not null and points at a SID from the system; `IsValidSid` checks its form before `GetLengthSid` reads its
    // length, and the copy reads exactly that many bytes.
    unsafe {
        if IsValidSid(sid) == 0 {
            return None;
        }
        let n = GetLengthSid(sid) as usize;
        Some(std::slice::from_raw_parts(sid.cast::<u8>(), n).to_vec())
    }
}

struct Eintrag {
    typ: u8,
    /// The access mask of an allowed or denied entry (0 for other types).
    maske: u32,
    sid: Option<Vec<u8>>,
}

struct Sicherheit {
    besitzer: Option<Vec<u8>>,
    geschuetzt: bool,
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
    let besitzer = sid_kopie(besitzer);
    // SAFETY: `beschreibung` was allocated by `GetSecurityInfo` and is freed once; nothing points into it after this.
    unsafe { LocalFree(beschreibung) };
    Ok(Sicherheit {
        besitzer,
        geschuetzt,
        eintraege,
    })
}

/// Why a folder fails the check (the texts of the Unix form of `rechte.rs`).
pub(crate) enum OrdnerFehler {
    Fehlt,
    Link,
    KeinOrdner,
    Fremd,
    Offen,
}

/// The folder of a file under Windows: it exists, is a folder and no reparse point, belongs to the own user, and its protected
/// list allows only the own user and `SYSTEM`. The check runs on the open handle.
pub(crate) fn ordner_pruefen(ordner: &Path) -> Result<(), OrdnerFehler> {
    let griff = OpenOptions::new()
        .access_mode(READ_CONTROL | FILE_LIST_DIRECTORY | FILE_READ_ATTRIBUTES | SYNCHRONIZE)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
        .open(ordner)
        .map_err(|_| OrdnerFehler::Fehlt)?;
    let info = attribut_tag(&griff).map_err(|_| OrdnerFehler::Fehlt)?;
    if info.attribute & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
        return Err(OrdnerFehler::Link);
    }
    if info.attribute & FILE_ATTRIBUTE_DIRECTORY == 0 {
        return Err(OrdnerFehler::KeinOrdner);
    }
    let eigen = eigene_sid().map_err(|_| OrdnerFehler::Fremd)?;
    let s = sicherheit_lesen(&griff).map_err(|_| OrdnerFehler::Offen)?;
    if s.besitzer.as_deref() != Some(eigen.as_slice()) {
        return Err(OrdnerFehler::Fremd);
    }
    // the owner bits `rwx` of 0700: an allowed entry of the own user with full access (a folder 0600 under Unix is refused too)
    let nur_eigen = s.geschuetzt
        && s.eintraege.as_ref().is_some_and(|l| {
            l.iter().all(|e| match (e.typ, e.sid.as_deref()) {
                (ACCESS_DENIED_ACE_TYPE, Some(_)) => true,
                (ACCESS_ALLOWED_ACE_TYPE, Some(sid)) => sid == eigen || sid == SID_SYSTEM,
                _ => false,
            }) && l.iter().any(|e| {
                e.typ == ACCESS_ALLOWED_ACE_TYPE
                    && e.sid.as_deref() == Some(eigen.as_slice())
                    && e.maske & FILE_ALL_ACCESS == FILE_ALL_ACCESS
            })
        });
    if nur_eigen {
        Ok(())
    } else {
        Err(OrdnerFehler::Offen)
    }
}

/// A file the bridge made: held open (no `FILE_SHARE_DELETE` for others is needed; the bridge deletes by path while the
/// identity holds).
#[derive(Debug)]
pub(crate) struct Datei {
    pfad: PathBuf,
    kennung: (u32, u64),
    _offen: File,
}

impl Datei {
    /// Removes the file if the path still names it (volume and file index); returns whether it did.
    pub(crate) fn entfernen(&self) -> bool {
        let Ok(jetzt) = OpenOptions::new()
            .access_mode(FILE_READ_ATTRIBUTES)
            .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE)
            .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
            .open(&self.pfad)
        else {
            return false;
        };
        if kennung(&jetzt).ok() != Some(self.kennung) {
            return false;
        }
        drop(jetzt);
        std::fs::remove_file(&self.pfad).is_ok()
    }
}

/// Makes the file `pfad` new (never an existing one, no link followed), sets the own user as owner and a protected list with only
/// the own user, reads the list back and writes `inhalt`.
pub(crate) fn datei_anlegen(pfad: &Path, inhalt: &[u8]) -> io::Result<Datei> {
    let eigen = eigene_sid()?;
    let mut f = OpenOptions::new()
        .access_mode(FILE_GENERIC_WRITE | READ_CONTROL | WRITE_DAC | WRITE_OWNER)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE)
        .write(true)
        .create_new(true)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
        .open(pfad)?;
    let weg = |e: io::Error| {
        let _ = std::fs::remove_file(pfad);
        e
    };
    liste_setzen(&f, &eigen).map_err(weg)?;
    f.write_all(inhalt).map_err(weg)?;
    let kennung = kennung(&f).map_err(weg)?;
    Ok(Datei {
        pfad: pfad.to_path_buf(),
        kennung,
        _offen: f,
    })
}

fn liste_setzen(f: &File, eigen: &[u8]) -> io::Result<()> {
    let falsch = || io::Error::new(io::ErrorKind::PermissionDenied, "access list not set");
    let laenge = 8 + 8 + eigen.len();
    let mut acl = vec![0_u32; laenge.div_ceil(4) + 1];
    let groesse = u32::try_from(acl.len() * 4).map_err(|_| falsch())?;
    // SAFETY: `acl` is DWORD aligned and large enough for the head and one allowed ACE with this SID; `eigen` is a valid SID.
    let gebaut = unsafe {
        InitializeAcl(acl.as_mut_ptr().cast(), groesse, ACL_REVISION) != 0
            && AddAccessAllowedAceEx(
                acl.as_mut_ptr().cast(),
                ACL_REVISION,
                0,
                FILE_ALL_ACCESS,
                eigen.as_ptr().cast(),
            ) != 0
    };
    if !gebaut {
        return Err(falsch());
    }
    // SAFETY: valid handle with WRITE_DAC and WRITE_OWNER; the SID and the ACL live for the call; the system copies them.
    let rc = unsafe {
        SetSecurityInfo(
            f.as_raw_handle(),
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
        return Err(falsch());
    }
    let s = sicherheit_lesen(f)?;
    let genau = s.geschuetzt
        && s.besitzer.as_deref() == Some(eigen)
        && s.eintraege.as_ref().is_some_and(|l| {
            l.len() == 1
                && l[0].typ == ACCESS_ALLOWED_ACE_TYPE
                && l[0].sid.as_deref() == Some(eigen)
        });
    if genau { Ok(()) } else { Err(falsch()) }
}

// test code: short names for folder, path, listener and the two ends read better here
#[cfg(test)]
#[allow(clippy::many_single_char_names)]
mod tests {
    use super::*;

    #[test]
    fn zufall_gibt_die_laenge_und_jedes_mal_andere_bytes() {
        let a = zufall(32).expect("random");
        assert_eq!(a.len(), 32);
        assert_ne!(a, zufall(32).expect("random"));
        assert!(zufall(0).expect("random").is_empty());
    }

    #[test]
    fn eine_datei_ist_neu_nur_fuer_den_eigenen_nutzer_und_geht_nur_als_sie_selbst() {
        let o = std::env::temp_dir().join(format!("wbbr-sys-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&o);
        std::fs::create_dir(&o).expect("folder");
        let p = o.join("token");
        let d = datei_anlegen(&p, b"geheim").expect("file");
        assert!(
            datei_anlegen(&p, b"x").is_err(),
            "an existing file is never replaced"
        );
        let f = File::open(&p).expect("open");
        let s = sicherheit_lesen(&f).expect("list");
        let eigen = eigene_sid().expect("sid");
        assert!(s.geschuetzt);
        assert_eq!(s.besitzer.as_deref(), Some(eigen.as_slice()));
        assert!(
            s.eintraege
                .as_ref()
                .is_some_and(|l| l.len() == 1 && l[0].sid.as_deref() == Some(eigen.as_slice()))
        );
        drop(f);
        assert!(d.entfernen());
        assert!(!p.exists());
        // a new file under the same name is not the file the bridge made: it stays
        let d2 = datei_anlegen(&p, b"a").expect("file");
        std::fs::remove_file(&p).expect("remove");
        std::fs::write(&p, b"fremd").expect("other file");
        assert!(!d2.entfernen());
        assert!(p.exists());
        let _ = std::fs::remove_dir_all(&o);
    }
}
