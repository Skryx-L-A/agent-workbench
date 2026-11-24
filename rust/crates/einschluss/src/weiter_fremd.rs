//! The stand-in of the forwarder on a system without Unix sockets. It exists only so that the program
//! `einschluss-weiter` builds everywhere; the enclosure is Linux only.

use std::io::Write;
use std::process::ExitCode;

/// Says that the forwarder is not available here and ends with code 2.
#[must_use]
pub fn starten_fremd(_args: &[String]) -> ExitCode {
    let _ = std::io::stderr().write_all(b"einschluss-weiter: only on Unix systems\n");
    ExitCode::from(2)
}
