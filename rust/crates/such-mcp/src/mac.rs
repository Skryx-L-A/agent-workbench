//! The rules of `such-mcp` for macOS (addendum to contract 4, `rust/spec/VERTRAEGE.md`, section 6).
//!
//! As in `rangkern sitzung`, every rule is a pure function with the platform as a parameter, so that the tests run under Linux
//! with `MacOs`; the program calls them with [`Plattform::AKTUELL`]. Under Linux nothing changes. Only the measurement of the
//! server ([`gemessene_euid`]) and the own euid ([`eigene_euid`]) exist for macOS alone.

#[cfg(unix)]
use std::os::unix::net::UnixStream;
use std::path::Path;

/// The system whose rules apply.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Plattform {
    /// Linux and every other system: the rules of contract 4 only.
    Linux,
    /// macOS: the addendum.
    MacOs,
    /// Windows: the addendum for Windows (a named pipe, `crate::pipe`).
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

/// Whether the socket path is usable for the platform: under macOS it is absolute and at most 103 bytes long; under Windows it
/// is a pipe name of the form of the addendum (`crate::pipe::name_ok`).
#[must_use]
pub fn pfad_ok(plattform: Plattform, pfad: &Path) -> bool {
    match plattform {
        Plattform::Linux => true,
        Plattform::MacOs => pfad.is_absolute() && pfad.as_os_str().len() <= SOCKET_MAX_MACOS,
        Plattform::Windows => pfad.to_str().is_some_and(crate::pipe::name_ok),
    }
}

/// Whether the server may be asked (section 6): under macOS its euid, measured after `connect`, is the own euid. A failed
/// measurement or another euid is `false`. Under Linux the server is not checked. Under Windows there is no euid: `peerkanal`
/// checks the owner of the pipe instance, so this rule says no.
#[must_use]
pub fn server_ok<F>(plattform: Plattform, gemessen: Result<u32, F>, eigene: u32) -> bool {
    match plattform {
        Plattform::Linux => true,
        Plattform::MacOs => gemessen.is_ok_and(|euid| euid == eigene),
        Plattform::Windows => false,
    }
}

#[cfg(unix)]
/// The check of the server after `connect` and before the first byte: the platform, the own euid and the measurement. The
/// expected euid comes from `geteuid` alone, never from an argument, the environment or a setting.
#[derive(Debug, Clone, Copy)]
pub struct Serverpruefung {
    /// The rules to apply.
    pub plattform: Plattform,
    /// The own euid, the only euid the server may have.
    pub euid: u32,
    /// The euid of the server at the other end of the connection, or why it cannot be measured.
    pub messen: fn(&UnixStream) -> std::io::Result<u32>,
}

#[cfg(unix)]
impl Serverpruefung {
    /// The check of this program: its platform, its euid, `getpeereid`.
    #[must_use]
    pub fn eigene() -> Self {
        Self {
            plattform: Plattform::AKTUELL,
            euid: eigene_euid(),
            messen: gemessene_euid,
        }
    }

    /// Whether the server of the connection may be asked.
    #[must_use]
    pub fn erlaubt(&self, strom: &UnixStream) -> bool {
        if self.plattform == Plattform::Linux {
            return true;
        }
        server_ok(self.plattform, (self.messen)(strom), self.euid)
    }
}

/// The euid of the server: what `getpeereid` names for the connection.
#[cfg(target_os = "macos")]
fn gemessene_euid(strom: &UnixStream) -> std::io::Result<u32> {
    nix::unistd::getpeereid(strom)
        .map(|(euid, _gid)| euid.as_raw())
        .map_err(std::io::Error::from)
}

/// Other systems measure nothing, because no rule asks for it.
#[cfg(all(unix, not(target_os = "macos")))]
fn gemessene_euid(_strom: &UnixStream) -> std::io::Result<u32> {
    Err(std::io::Error::from(std::io::ErrorKind::Unsupported))
}

/// The own euid of this process.
#[cfg(target_os = "macos")]
fn eigene_euid() -> u32 {
    nix::unistd::geteuid().as_raw()
}

/// Other systems check no server, so no euid is expected.
#[cfg(all(unix, not(target_os = "macos")))]
fn eigene_euid() -> u32 {
    0
}
