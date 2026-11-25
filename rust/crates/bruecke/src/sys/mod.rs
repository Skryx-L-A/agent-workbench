//! The system calls that differ between Unix and Windows (Nachtrag 25.11.2026, contract `bruecke`, section of the platforms).
//!
//! Unix: the stream of `std`; nothing changes there. Windows: the submodule `windows`, the only place of the crate with `unsafe`.
//! It declares the few Win32 calls itself (`ws2_32`, `advapi32`, `kernel32`, which every Windows build of `std` links already),
//! so the crate needs no new package: the `AF_UNIX` client to the service, the access list of the token folder and of the
//! files the bridge makes.

#[cfg(unix)]
pub(crate) use std::os::unix::net::UnixStream;

#[cfg(windows)]
#[allow(unsafe_code)]
mod windows;

#[cfg(windows)]
pub(crate) use windows::{Datei, OrdnerFehler, UnixStream, datei_anlegen, ordner_pruefen, zufall};
