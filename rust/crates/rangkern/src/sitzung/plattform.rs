//! The platform whose rules apply to the arguments of `rangkern sitzung`, and the rules that need no system call: the socket
//! path under macOS (addendum `macos/README.md`, section 2) and the pipe name and the package SID under Windows (addendum
//! `windows/README.md`, section 2).
//!
//! Every rule is a pure function with the platform as a parameter, so the tests of every platform run on any machine; the
//! program calls them with [`Plattform::AKTUELL`].

use std::path::Path;

/// The system whose rules apply.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Plattform {
    /// Linux and every other Unix: the rules of contract 4 only.
    Linux,
    /// macOS: the addendum for macOS.
    MacOs,
    /// Windows: the addendum for Windows (a named pipe instead of a Unix socket).
    Windows,
}

impl Plattform {
    /// The platform this program was built for.
    pub const AKTUELL: Self = if cfg!(windows) {
        Self::Windows
    } else if cfg!(target_os = "macos") {
        Self::MacOs
    } else {
        Self::Linux
    };
}

/// The longest socket path under macOS: `sun_path` holds 104 bytes, the NUL counts.
pub const SOCKET_MAX_MACOS: usize = 103;

/// The fixed start of the pipe name under Windows; 32 characters of `0-9a-f` follow.
pub const PIPE_PRAEFIX: &str = r"\\.\pipe\wb-such-";

/// The start of an `AppContainer` SID; exactly seven decimal numbers follow.
pub const PAKET_SID_PRAEFIX: &str = "S-1-15-2-";

/// Whether the arguments are usable for the platform (section 2 of each addendum): under macOS the socket path is absolute and
/// at most 103 bytes long, and the rights are `0600`; under Windows the socket is a pipe name of the form [`pipe_name_ok`]
/// (the rights are checked by the caller: under Windows the option itself is a usage error). Anything else is a usage error
/// (exit 64).
#[must_use]
pub fn aufruf_ok(plattform: Plattform, socket: &Path, rechte: u32) -> bool {
    match plattform {
        Plattform::Linux => true,
        Plattform::MacOs => {
            socket.is_absolute() && socket.as_os_str().len() <= SOCKET_MAX_MACOS && rechte == 0o600
        }
        Plattform::Windows => socket.to_str().is_some_and(pipe_name_ok),
    }
}

/// The pipe name under Windows: exactly `\\.\pipe\wb-such-` and 32 characters of `0-9a-f`. A file path, a remote name
/// (`\\host\pipe\…`), capital letters or another length are refused.
#[must_use]
pub fn pipe_name_ok(name: &str) -> bool {
    name.strip_prefix(PIPE_PRAEFIX).is_some_and(|zufall| {
        zufall.len() == 32
            && zufall
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    })
}

/// The channel name of the pipe for `peerkanal` (the part after `\\.\pipe\`), or `None` if the name has not the form of
/// [`pipe_name_ok`].
#[must_use]
pub fn kanalname(name: &str) -> Option<&str> {
    pipe_name_ok(name)
        .then(|| name.strip_prefix(r"\\.\pipe\"))
        .flatten()
}

/// The package SID of `--paket-sid`: `S-1-15-2-` and exactly seven decimal numbers, each without a leading zero and at most
/// 4294967295 (as the admin helper of the Windows confinement takes it).
#[must_use]
pub fn paket_sid_ok(text: &str) -> bool {
    let Some(rest) = text.strip_prefix(PAKET_SID_PRAEFIX) else {
        return false;
    };
    let teile: Vec<&str> = rest.split('-').collect();
    teile.len() == 7
        && teile.iter().all(|teil| {
            !teil.is_empty()
                && teil.bytes().all(|b| b.is_ascii_digit())
                && (teil.len() == 1 || !teil.starts_with('0'))
                && teil.parse::<u32>().is_ok()
        })
}
