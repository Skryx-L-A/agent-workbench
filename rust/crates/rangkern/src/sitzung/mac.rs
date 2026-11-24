//! The rules of `rangkern sitzung` for macOS (addendum to contract 4, `rust/spec/VERTRAEGE.md`).
//!
//! Every rule is a pure function with the platform as a parameter, so that the tests run under Linux with `MacOs`; the
//! program calls them with [`Plattform::AKTUELL`]. Under Linux nothing changes: every function lets it through.
//! Only the measurement of the peer ([`gemessene_euid`]) exists for macOS alone.

use std::fs;
use std::os::unix::fs::MetadataExt;
use std::os::unix::net::UnixStream;
use std::path::Path;

/// The system whose rules apply.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Plattform {
    /// Linux and every other Unix: the rules of contract 4 only.
    Linux,
    /// macOS: the addendum.
    MacOs,
}

impl Plattform {
    /// The platform this program was built for.
    pub const AKTUELL: Self = if cfg!(target_os = "macos") {
        Self::MacOs
    } else {
        Self::Linux
    };
}

/// The longest socket path under macOS: `sun_path` holds 104 bytes, the NUL counts.
pub const SOCKET_MAX_MACOS: usize = 103;

/// The reason of the log line when `getpeereid` fails.
pub const GRUND_UNMESSBAR: &str = "such.kennung_unmessbar";
/// The reason of the log line when the euid of the peer is not the own one.
pub const GRUND_FREMD: &str = "such.kennung_fremd";

/// Whether the arguments are usable for the platform (section 2): under macOS the socket path is absolute and at most 103
/// bytes long, and the rights are `0600`. Anything else is a usage error (exit 64).
#[must_use]
pub fn aufruf_ok(plattform: Plattform, socket: &Path, rechte: u32) -> bool {
    match plattform {
        Plattform::Linux => true,
        Plattform::MacOs => {
            socket.is_absolute() && socket.as_os_str().len() <= SOCKET_MAX_MACOS && rechte == 0o600
        }
    }
}

/// The check of the parent folder before `bind` (section 3, point 1): a folder and no link, owned by `euid`, with exactly the
/// mode 0700. The error is the text for the error output; the start ends with exit 74 and leaves the path as it is.
///
/// # Errors
/// The parent folder is missing, a link or no folder, owned by another user or not 0700.
pub fn ordner_pruefen(plattform: Plattform, socket: &Path, euid: u32) -> Result<(), String> {
    if plattform == Plattform::Linux {
        return Ok(());
    }
    let eltern = socket
        .parent()
        .filter(|eltern| !eltern.as_os_str().is_empty())
        .ok_or("the socket has no parent folder")?;
    let meta = fs::symlink_metadata(eltern)
        .map_err(|fehler| format!("the parent folder cannot be read: {fehler}"))?;
    if !meta.is_dir() {
        return Err("the parent folder is not a folder (a link does not count)".to_owned());
    }
    if meta.uid() != euid {
        return Err("the parent folder belongs to another user".to_owned());
    }
    if meta.mode() & 0o7777 != 0o700 {
        return Err("the parent folder has not exactly the mode 0700".to_owned());
    }
    Ok(())
}

/// The decision about a peer (section 4): `None` accepts, `Some(grund)` turns the connection away with the reason of the log
/// line. `gemessen` is the euid of the peer, or `None` when `getpeereid` failed; `eigene` is the euid of this process. Under
/// Linux the peer is not checked.
#[must_use]
pub fn peer_entscheiden(
    plattform: Plattform,
    gemessen: Option<u32>,
    eigene: u32,
) -> Option<&'static str> {
    if plattform == Plattform::Linux {
        return None;
    }
    match gemessen {
        None => Some(GRUND_UNMESSBAR),
        Some(euid) if euid != eigene => Some(GRUND_FREMD),
        Some(_) => None,
    }
}

/// The measurement of the peer: the euid that `getpeereid` names for the connection.
///
/// # Errors
/// `getpeereid` fails.
#[cfg(target_os = "macos")]
pub fn gemessene_euid(strom: &UnixStream) -> std::io::Result<u32> {
    nix::unistd::getpeereid(strom)
        .map(|(euid, _gid)| euid.as_raw())
        .map_err(std::io::Error::from)
}

/// Other systems measure nothing, because no rule asks for it; the call is an error.
///
/// # Errors
/// Always: the measurement is not supported.
#[cfg(not(target_os = "macos"))]
pub fn gemessene_euid(_strom: &UnixStream) -> std::io::Result<u32> {
    Err(std::io::Error::from(std::io::ErrorKind::Unsupported))
}

/// The own euid of this process.
#[must_use]
pub fn eigene_euid() -> u32 {
    rustix::process::geteuid().as_raw()
}
