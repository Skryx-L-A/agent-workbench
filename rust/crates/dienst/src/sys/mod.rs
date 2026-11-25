//! The system calls that differ between Unix and Windows (Nachtrag 25.11.2026, contract `dienst`, section 21).
//!
//! Unix: only the names of `std` and `rustix` the service used before; nothing changes there. Windows: the submodule `windows`,
//! the only place of the crate with `unsafe`. It declares the few Win32 calls itself (`ws2_32`, `advapi32`, `kernel32`, which
//! every Windows build of `std` links already), so the crate needs no new package. The socket under Windows is `AF_UNIX`
//! (Windows 10 and later): a file entry in the socket folder like on Unix, refused when the path exists, reachable only through
//! the access list of the folder and the entry.

#[cfg(unix)]
pub(crate) use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
#[cfg(unix)]
pub(crate) use std::os::unix::net::{UnixListener, UnixStream};

/// The open socket folder: every removal below it goes through it (`loeschen.rs`).
#[cfg(unix)]
pub(crate) type Ordner = OwnedFd;

/// The number of the effective user (Unix). Under Windows the owner is a SID, which `windows::eigene_sid` reads; the number is 0.
#[cfg(unix)]
pub(crate) fn euid() -> u32 {
    rustix::process::geteuid().as_raw()
}

#[cfg(windows)]
#[allow(unsafe_code)]
mod windows;

#[cfg(all(windows, feature = "pruefbau"))]
pub(crate) use windows::modus_text;
#[cfg(windows)]
pub(crate) use windows::{
    AsFd, BorrowedFd, Eigen, Job, Ordner, OwnedFd, UnixListener, UnixStream, datei_anlegen_eigen,
    eigen_besitz, eigen_oeffnen, ordner_anlegen_eigen, ordner_durchsuchbar, ordner_oeffnen,
    socket_binden, socket_binden_pfad,
};

#[cfg(all(windows, feature = "pruefbau"))]
pub(crate) use windows::{konsole_groesse, konsole_roh};

/// Windows: no number of the user (the owner is a SID, see `windows::eigene_sid`).
#[cfg(windows)]
pub(crate) fn euid() -> u32 {
    0
}
