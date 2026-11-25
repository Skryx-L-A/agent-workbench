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

// Windows (decision of 25.11.2026): every line of Windows code of the crate stands in `windows.rs`, which the mutation gate leaves
// out; the other modules only call what is named here (checked by `tests/sys_regel.rs`).
#[cfg(windows)]
pub(crate) use windows::{
    AsFd, BorrowedFd, JobStand, Ordner, OwnedFd, UnixListener, UnixStream, alte_zielliste_raeumen,
    eintrag_durchsuchbar, euid, gegenseite_gut, gleiche_datei, gruppe_beenden, job_schliessen,
    kind_argv, kind_beenden, kind_mit_job, kind_nochmal_beenden, kinder_beenden, laufen,
    marker_lesen, meldung_binden, mit_job, nanosekunden, ordner_anlegen, ordner_oeffnen,
    programm_urteil, sitzung_mit_job, socket_binden,
};
#[cfg(windows)]
pub use windows::{Signale, signale_anmelden};
#[cfg(all(windows, feature = "pruefbau"))]
pub(crate) use windows::{
    laufkind, mitschrift_eingabe, mitschrift_ereignisse, mitschrift_groesse, modus_von, ohne_exe,
    pfad_text, signal_an_sich, stty, symlink,
};
