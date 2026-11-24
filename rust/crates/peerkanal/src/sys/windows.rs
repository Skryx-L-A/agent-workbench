//! Win32 over `windows-sys` 0.61.2. Every `unsafe` block of the crate stands here, each with the reason why the call is sound.
//!
//! Rules of the module: every handle goes into an [`OwnedHandle`] at once; every memory from `LocalAlloc` of the system is
//! freed with `LocalFree` in the same function; every `OVERLAPPED` lives on the stack of one function and is waited for
//! before the function returns, also after `CancelIoEx`.

use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle, RawHandle};
use std::ptr;
use std::time::Duration;

use windows_sys::Win32::Foundation::{
    self as F, ERROR_BROKEN_PIPE, ERROR_IO_PENDING, ERROR_NO_DATA, ERROR_PIPE_CONNECTED,
    ERROR_PIPE_NOT_CONNECTED, FILETIME, GetLastError, HANDLE, INVALID_HANDLE_VALUE, LocalFree,
    WAIT_OBJECT_0, WAIT_TIMEOUT,
};
use windows_sys::Win32::Security::Authorization::{
    ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW, GetSecurityInfo,
    SDDL_REVISION_1, SE_KERNEL_OBJECT,
};
use windows_sys::Win32::Security::{
    GetTokenInformation, IsTokenRestricted, OWNER_SECURITY_INFORMATION, PSECURITY_DESCRIPTOR, PSID,
    RevertToSelf, SECURITY_ATTRIBUTES, TOKEN_APPCONTAINER_INFORMATION, TOKEN_INFORMATION_CLASS,
    TOKEN_QUERY, TOKEN_USER, TokenAppContainerSid, TokenUser,
};
use windows_sys::Win32::Storage::FileSystem::{
    self as FS, CreateFileW, OPEN_EXISTING, ReadFile, WriteFile,
};
use windows_sys::Win32::System::IO::{CancelIoEx, GetOverlappedResult, OVERLAPPED};
use windows_sys::Win32::System::JobObjects::{IsProcessInJob, OpenJobObjectW};
use windows_sys::Win32::System::Pipes::{
    self as PI, ConnectNamedPipe, CreateNamedPipeW, DisconnectNamedPipe,
    GetNamedPipeClientProcessId, GetNamedPipeServerProcessId, ImpersonateNamedPipeClient,
    WaitNamedPipeW,
};
use windows_sys::Win32::System::SystemInformation::GetSystemTimeAsFileTime;
use windows_sys::Win32::System::Threading::{
    CreateEventW, GetCurrentProcess, GetCurrentThread, GetExitCodeProcess, GetProcessTimes,
    OpenProcess, OpenProcessToken, OpenThreadToken, PROCESS_QUERY_LIMITED_INFORMATION,
    QueryFullProcessImageNameW, WaitForSingleObject,
};

use crate::ablauf::{Lesen, Plattform};
use crate::acl;
use crate::pruefung::Token;
use crate::vorgabe::JobBezug;

// The numbers of `acl` are the numbers of the system.
const _: () = {
    assert!(acl::PIPE_ACCESS_DUPLEX == FS::PIPE_ACCESS_DUPLEX);
    assert!(acl::FILE_FLAG_FIRST_PIPE_INSTANCE == FS::FILE_FLAG_FIRST_PIPE_INSTANCE);
    assert!(acl::FILE_FLAG_OVERLAPPED == FS::FILE_FLAG_OVERLAPPED);
    assert!(acl::PIPE_TYPE_BYTE == PI::PIPE_TYPE_BYTE);
    assert!(acl::PIPE_READMODE_BYTE == PI::PIPE_READMODE_BYTE);
    assert!(acl::PIPE_REJECT_REMOTE_CLIENTS == PI::PIPE_REJECT_REMOTE_CLIENTS);
    assert!(acl::GENERIC_READ == F::GENERIC_READ);
    assert!(acl::FILE_WRITE_DATA == FS::FILE_WRITE_DATA);
    assert!(acl::FILE_CREATE_PIPE_INSTANCE == FS::FILE_APPEND_DATA);
    assert!(acl::SECURITY_SQOS_PRESENT == FS::SECURITY_SQOS_PRESENT);
    assert!(acl::SECURITY_IDENTIFICATION == FS::SECURITY_IDENTIFICATION);
    assert!(crate::ablauf::OS_NICHT_GEFUNDEN.cast_unsigned() == F::ERROR_FILE_NOT_FOUND);
    assert!(crate::ablauf::OS_ZUGRIFF.cast_unsigned() == F::ERROR_ACCESS_DENIED);
    assert!(crate::ablauf::OS_BELEGT.cast_unsigned() == F::ERROR_PIPE_BUSY);
    assert!(crate::ablauf::OS_KEINE_DATEN.cast_unsigned() == ERROR_NO_DATA);
};

/// `STILL_ACTIVE` as exit code.
const STILL_ACTIVE: u32 = 259;

/// `JOB_OBJECT_QUERY`: enough for `IsProcessInJob`.
const JOB_OBJECT_QUERY: u32 = 0x0004;

/// Opens a named job object with `JOB_OBJECT_QUERY` (for [`JobBezug::oeffnen`]).
pub(crate) fn job_oeffnen(name: &str) -> std::io::Result<OwnedHandle> {
    let name_w = breit(name);
    // SAFETY: a NUL-terminated name; the returned handle goes into an OwnedHandle at once.
    let h = unsafe { OpenJobObjectW(JOB_OBJECT_QUERY, 0, name_w.as_ptr()) };
    besitzen(h).ok_or_else(std::io::Error::last_os_error)
}

/// The Win32 system.
pub(crate) struct Os;

/// A pipe handle of server or client, opened with `FILE_FLAG_OVERLAPPED`.
pub(crate) struct Griff(OwnedHandle);

fn letzter_fehler() -> i32 {
    // SAFETY: GetLastError has no preconditions.
    unsafe { GetLastError() }.cast_signed()
}

fn roh(h: &OwnedHandle) -> HANDLE {
    h.as_raw_handle().cast()
}

/// Takes a handle the system just returned. `None` for the null handle and for `INVALID_HANDLE_VALUE`.
fn besitzen(h: HANDLE) -> Option<OwnedHandle> {
    if h.is_null() || h == INVALID_HANDLE_VALUE {
        return None;
    }
    // SAFETY: the system returned this handle to us just now, it is valid and nobody else owns it.
    Some(unsafe { OwnedHandle::from_raw_handle(h as RawHandle) })
}

fn breit(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

fn pipe_name(name: &str) -> Vec<u16> {
    breit(&format!(r"\\.\pipe\{name}"))
}

fn millis(d: Duration) -> u32 {
    // INFINITE is u32::MAX; a time limit stays below it.
    u32::try_from(d.as_millis())
        .unwrap_or(u32::MAX - 1)
        .min(u32::MAX - 1)
}

/// The text of a SID; `None` for a null pointer or an error.
fn sid_text(sid: PSID) -> Option<String> {
    if sid.is_null() {
        return None;
    }
    let mut text: *mut u16 = ptr::null_mut();
    // SAFETY: `sid` points to a valid SID owned by the caller for this call; `text` receives a LocalAlloc buffer.
    if unsafe { ConvertSidToStringSidW(sid, &raw mut text) } == 0 || text.is_null() {
        return None;
    }
    let mut n = 0;
    // SAFETY: the system wrote a NUL-terminated UTF-16 string at `text`.
    while unsafe { *text.add(n) } != 0 {
        n += 1;
    }
    // SAFETY: `text` holds `n` valid UTF-16 units before the NUL.
    let s = String::from_utf16(unsafe { std::slice::from_raw_parts(text, n) }).ok();
    // SAFETY: `text` came from LocalAlloc of ConvertSidToStringSidW and is freed once.
    unsafe { LocalFree(text.cast()) };
    s
}

/// `GetTokenInformation` into a buffer aligned for the pointers in it.
fn token_info(token: &OwnedHandle, klasse: TOKEN_INFORMATION_CLASS) -> Result<Vec<u64>, i32> {
    let mut laenge = 0u32;
    // SAFETY: a null buffer of length 0 asks only for the length.
    unsafe { GetTokenInformation(roh(token), klasse, ptr::null_mut(), 0, &raw mut laenge) };
    if laenge == 0 {
        return Err(letzter_fehler());
    }
    let mut puffer = vec![0u64; (laenge as usize).div_ceil(8)];
    // SAFETY: the buffer holds at least `laenge` bytes and is aligned to 8.
    let ok = unsafe {
        GetTokenInformation(
            roh(token),
            klasse,
            puffer.as_mut_ptr().cast(),
            laenge,
            &raw mut laenge,
        )
    };
    if ok == 0 {
        Err(letzter_fehler())
    } else {
        Ok(puffer)
    }
}

fn token_nutzer(token: &OwnedHandle) -> Result<String, i32> {
    let puffer = token_info(token, TokenUser)?;
    // SAFETY: the system wrote a TOKEN_USER at the start of the buffer; its SID points into the same buffer, which lives here.
    let sid = unsafe { (*puffer.as_ptr().cast::<TOKEN_USER>()).User.Sid };
    sid_text(sid).ok_or(0)
}

fn token_container(token: &OwnedHandle) -> Result<Option<String>, i32> {
    let puffer = token_info(token, TokenAppContainerSid)?;
    // SAFETY: the system wrote a TOKEN_APPCONTAINER_INFORMATION; a null SID means no container.
    let sid =
        unsafe { (*puffer.as_ptr().cast::<TOKEN_APPCONTAINER_INFORMATION>()).TokenAppContainer };
    Ok(sid_text(sid))
}

enum Io {
    Frist,
    Os(i32),
}

/// One overlapped call and its end. `start` starts the call with the given `OVERLAPPED`; with a time limit the call is
/// cancelled when it passes, and the function still waits for its end, so the `OVERLAPPED` is free when it returns.
fn ueberlappt(
    h: HANDLE,
    frist: Option<Duration>,
    start: impl FnOnce(*mut OVERLAPPED) -> i32,
) -> Result<u32, Io> {
    // SAFETY: an unnamed manual-reset event without security attributes.
    let ereignis = besitzen(unsafe { CreateEventW(ptr::null(), 1, 0, ptr::null()) })
        .ok_or_else(|| Io::Os(letzter_fehler()))?;
    // SAFETY: OVERLAPPED is plain data; all zero is its initial state.
    let mut ov: OVERLAPPED = unsafe { std::mem::zeroed() };
    ov.hEvent = roh(&ereignis);
    let ovp = &raw mut ov;
    if start(ovp) == 0 {
        let e = letzter_fehler();
        if e.cast_unsigned() != ERROR_IO_PENDING {
            return Err(Io::Os(e));
        }
    }
    if let Some(f) = frist {
        // SAFETY: waits on our own event.
        let w = unsafe { WaitForSingleObject(roh(&ereignis), millis(f)) };
        if w == WAIT_TIMEOUT {
            let mut n = 0u32;
            // SAFETY: cancels our own pending call and waits for its end, so `ov` is no longer used afterwards.
            unsafe {
                CancelIoEx(h, ovp);
                GetOverlappedResult(h, ovp, &raw mut n, 1);
            }
            return Err(Io::Frist);
        }
        if w != WAIT_OBJECT_0 {
            let e = letzter_fehler();
            let mut n = 0u32;
            // SAFETY: as above.
            unsafe {
                CancelIoEx(h, ovp);
                GetOverlappedResult(h, ovp, &raw mut n, 1);
            }
            return Err(Io::Os(e));
        }
    }
    let mut n = 0u32;
    // SAFETY: waits for the end of the call started with `ov`.
    if unsafe { GetOverlappedResult(h, ovp, &raw mut n, 1) } == 0 {
        return Err(Io::Os(letzter_fehler()));
    }
    Ok(n)
}

impl Plattform for Os {
    type Griff = Griff;
    type Prozess = OwnedHandle;
    type Job = JobBezug;

    fn eigene_sid(&self) -> Result<String, i32> {
        let mut t: HANDLE = ptr::null_mut();
        // SAFETY: the pseudo handle of the own process; `t` receives a new token handle.
        if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &raw mut t) } == 0 {
            return Err(letzter_fehler());
        }
        let t = besitzen(t).ok_or(0)?;
        token_nutzer(&t)
    }

    fn instanz_anlegen(
        &self,
        name: &str,
        erste: bool,
        instanzen: u8,
        sddl: &str,
    ) -> Result<Griff, i32> {
        let sddl_w = breit(sddl);
        let mut sd: PSECURITY_DESCRIPTOR = ptr::null_mut();
        // SAFETY: a NUL-terminated SDDL text; `sd` receives a LocalAlloc buffer.
        if unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                sddl_w.as_ptr(),
                SDDL_REVISION_1,
                &raw mut sd,
                ptr::null_mut(),
            )
        } == 0
        {
            return Err(letzter_fehler());
        }
        let sa = SECURITY_ATTRIBUTES {
            nLength: u32::try_from(size_of::<SECURITY_ATTRIBUTES>()).unwrap_or(0),
            lpSecurityDescriptor: sd,
            bInheritHandle: 0,
        };
        let name_w = pipe_name(name);
        // SAFETY: valid name and security attributes for the duration of the call.
        let h = unsafe {
            CreateNamedPipeW(
                name_w.as_ptr(),
                acl::oeffnungsmodus(erste),
                acl::PIPE_MODUS,
                u32::from(instanzen),
                4096,
                4096,
                0,
                &raw const sa,
            )
        };
        let fehler = letzter_fehler();
        // SAFETY: `sd` came from LocalAlloc of the conversion and is freed once, after the call.
        unsafe { LocalFree(sd.cast()) };
        besitzen(h).map(Griff).ok_or(fehler)
    }

    fn auf_client_warten(&self, g: &Griff) -> Result<(), i32> {
        let h = roh(&g.0);
        let mut verbunden = false;
        // SAFETY: an overlapped connect on our own instance; `ueberlappt` waits for its end.
        let r = ueberlappt(h, None, |ov| {
            let ok = unsafe { ConnectNamedPipe(h, ov) };
            if ok == 0 && letzter_fehler().cast_unsigned() == ERROR_PIPE_CONNECTED {
                verbunden = true;
            }
            ok
        });
        if verbunden {
            return Ok(());
        }
        r.map(|_| ()).map_err(|e| match e {
            Io::Os(e) => e,
            Io::Frist => 0,
        })
    }

    fn jetzt(&self) -> u64 {
        let mut ft = FILETIME {
            dwLowDateTime: 0,
            dwHighDateTime: 0,
        };
        // SAFETY: writes the current time into `ft`.
        unsafe { GetSystemTimeAsFileTime(&raw mut ft) };
        (u64::from(ft.dwHighDateTime) << 32) | u64::from(ft.dwLowDateTime)
    }

    fn client_pid(&self, g: &Griff) -> Option<u32> {
        let mut pid = 0u32;
        // SAFETY: our own instance; `pid` receives the id.
        (unsafe { GetNamedPipeClientProcessId(roh(&g.0), &raw mut pid) } != 0).then_some(pid)
    }

    fn prozess_oeffnen(&self, pid: u32) -> Option<(OwnedHandle, u64)> {
        // SAFETY: opens a process for limited queries; the handle goes into an OwnedHandle at once.
        let h = besitzen(unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) })?;
        let null = FILETIME {
            dwLowDateTime: 0,
            dwHighDateTime: 0,
        };
        let (mut erzeugt, mut ende, mut kern, mut nutzer) = (null, null, null, null);
        // SAFETY: a valid process handle and four FILETIME outputs.
        if unsafe {
            GetProcessTimes(
                roh(&h),
                &raw mut erzeugt,
                &raw mut ende,
                &raw mut kern,
                &raw mut nutzer,
            )
        } == 0
        {
            return None;
        }
        Some((
            h,
            (u64::from(erzeugt.dwHighDateTime) << 32) | u64::from(erzeugt.dwLowDateTime),
        ))
    }

    fn programm(&self, p: &OwnedHandle) -> Option<String> {
        let mut puffer = vec![0u16; 32_768];
        let mut n = u32::try_from(puffer.len()).unwrap_or(0);
        // SAFETY: a valid process handle and a buffer of `n` units.
        if unsafe { QueryFullProcessImageNameW(roh(p), 0, puffer.as_mut_ptr(), &raw mut n) } == 0 {
            return None;
        }
        String::from_utf16(puffer.get(..n as usize)?).ok()
    }

    fn lebt(&self, p: &OwnedHandle) -> Option<bool> {
        let mut code = 0u32;
        // SAFETY: a valid process handle.
        (unsafe { GetExitCodeProcess(roh(p), &raw mut code) } != 0).then_some(code == STILL_ACTIVE)
    }

    fn im_job(&self, p: &OwnedHandle, job: &JobBezug) -> Option<bool> {
        let mut drin = 0;
        // SAFETY: valid process and job handles.
        (unsafe { IsProcessInJob(roh(p), roh(&job.griff), &raw mut drin) } != 0)
            .then_some(drin != 0)
    }

    fn kennung_messen(&self, g: &Griff) -> Result<Token, i32> {
        // SAFETY: our own instance, after the first read (otherwise error 1368).
        if unsafe { ImpersonateNamedPipeClient(roh(&g.0)) } == 0 {
            return Err(letzter_fehler());
        }
        let mut t: HANDLE = ptr::null_mut();
        // SAFETY: the token of the own thread, opened as self; `t` receives a new handle.
        let ok = unsafe { OpenThreadToken(GetCurrentThread(), TOKEN_QUERY, 1, &raw mut t) };
        let fehler = letzter_fehler();
        // SAFETY: ends the impersonation of this thread. If that fails, the thread would go on with the identity of the
        // client; the contract demands the end of the process then.
        if unsafe { RevertToSelf() } == 0 {
            std::process::abort();
        }
        if ok == 0 {
            return Err(fehler);
        }
        let t = besitzen(t).ok_or(fehler)?;
        let sid = token_nutzer(&t)?;
        let appcontainer_sid = token_container(&t)?;
        // SAFETY: a valid token handle.
        let eingeschraenkt = unsafe { IsTokenRestricted(roh(&t)) } != 0;
        Ok(Token {
            sid,
            appcontainer_sid,
            eingeschraenkt,
        })
    }

    fn trennen(&self, g: &Griff) {
        // SAFETY: our own instance; an error (not connected) changes nothing.
        unsafe { DisconnectNamedPipe(roh(&g.0)) };
    }

    fn lesen(&self, g: &Griff, puffer: &mut [u8], frist: Option<Duration>) -> Lesen {
        let h = roh(&g.0);
        let laenge = u32::try_from(puffer.len()).unwrap_or(u32::MAX);
        let ziel = puffer.as_mut_ptr();
        // SAFETY: the buffer outlives the call, because `ueberlappt` waits for its end.
        match ueberlappt(h, frist, |ov| unsafe {
            ReadFile(h, ziel, laenge, ptr::null_mut(), ov)
        }) {
            Ok(0) => Lesen::Ende,
            Ok(n) => Lesen::Daten(n as usize),
            Err(Io::Frist) => Lesen::Frist,
            Err(Io::Os(e))
                if [ERROR_BROKEN_PIPE, ERROR_PIPE_NOT_CONNECTED, ERROR_NO_DATA]
                    .contains(&e.cast_unsigned()) =>
            {
                Lesen::Ende
            }
            Err(Io::Os(e)) => Lesen::Fehler(e),
        }
    }

    fn schreiben(&self, g: &Griff, daten: &[u8]) -> Result<usize, i32> {
        let h = roh(&g.0);
        let laenge = u32::try_from(daten.len()).unwrap_or(u32::MAX);
        // SAFETY: the data outlive the call, because `ueberlappt` waits for its end.
        match ueberlappt(h, None, |ov| unsafe {
            WriteFile(h, daten.as_ptr(), laenge, ptr::null_mut(), ov)
        }) {
            Ok(n) => Ok(n as usize),
            Err(Io::Os(e)) => Err(e),
            Err(Io::Frist) => Err(0),
        }
    }

    fn oeffnen(&self, name: &str) -> Result<Griff, i32> {
        let name_w = pipe_name(name);
        // SAFETY: a valid name; access and flags of B7.
        let h = unsafe {
            CreateFileW(
                name_w.as_ptr(),
                acl::CLIENT_ZUGRIFF,
                0,
                ptr::null(),
                OPEN_EXISTING,
                acl::CLIENT_FLAGS,
                ptr::null_mut(),
            )
        };
        let fehler = letzter_fehler();
        besitzen(h).map(Griff).ok_or(fehler)
    }

    fn warten(&self, name: &str, frist: Duration) {
        let name_w = pipe_name(name);
        // SAFETY: a valid name.
        unsafe { WaitNamedPipeW(name_w.as_ptr(), millis(frist).max(1)) };
    }

    fn besitzer(&self, g: &Griff) -> Result<String, i32> {
        let mut besitzer: PSID = ptr::null_mut();
        let mut sd: PSECURITY_DESCRIPTOR = ptr::null_mut();
        // SAFETY: our own handle (opened with READ_CONTROL through GENERIC_READ); `besitzer` points into `sd`.
        let r = unsafe {
            GetSecurityInfo(
                roh(&g.0),
                SE_KERNEL_OBJECT,
                OWNER_SECURITY_INFORMATION,
                &raw mut besitzer,
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null_mut(),
                &raw mut sd,
            )
        };
        if r != 0 {
            return Err(r.cast_signed());
        }
        let text = sid_text(besitzer);
        // SAFETY: `sd` came from LocalAlloc of GetSecurityInfo and is freed once; `besitzer` is not used afterwards.
        unsafe { LocalFree(sd.cast()) };
        text.ok_or(0)
    }

    fn server_pid(&self, g: &Griff) -> Option<u32> {
        let mut pid = 0u32;
        // SAFETY: our own handle; `pid` receives the id.
        (unsafe { GetNamedPipeServerProcessId(roh(&g.0), &raw mut pid) } != 0).then_some(pid)
    }
}
