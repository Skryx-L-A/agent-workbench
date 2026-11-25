//! The command line: `such-mcp` or `such-mcp --socket <path>` (under macOS and Windows only the second form; under Windows the
//! path is a pipe name). Every other call is a usage error (exit 64).

use std::ffi::OsString;
use std::path::PathBuf;

use crate::mac::{Plattform, pfad_ok};

/// The fixed socket path inside the enclosure.
pub const SOCKET_IM_EINSCHLUSS: &str = "/run/brain/such.sock";

/// The socket path for a command line (without the program name), or `None` for a usage error.
///
/// No argument means the fixed path. `--socket <path>` replaces it (for tests outside the enclosure); the option
/// stands at most once, in the full spelling, with a non-empty path as its own argument.
#[must_use]
pub fn socket_pfad(args: &[OsString]) -> Option<PathBuf> {
    match args {
        [] => Some(PathBuf::from(SOCKET_IM_EINSCHLUSS)),
        [option, pfad] if option == "--socket" && !pfad.is_empty() => Some(PathBuf::from(pfad)),
        _ => None,
    }
}

/// [`socket_pfad`] for a platform. Under macOS the option is required (there is no fixed path), and its path is absolute and at
/// most 103 bytes long; under Windows the option is required too, with a pipe name of the form of the addendum for Windows
/// (section 6). Every other call is a usage error. Linux is unchanged.
#[must_use]
pub fn socket_pfad_fuer(plattform: Plattform, args: &[OsString]) -> Option<PathBuf> {
    match plattform {
        Plattform::Linux => socket_pfad(args),
        Plattform::MacOs | Plattform::Windows => match args {
            [option, pfad] if option == "--socket" => {
                Some(PathBuf::from(pfad)).filter(|pfad| pfad_ok(plattform, pfad))
            }
            _ => None,
        },
    }
}
