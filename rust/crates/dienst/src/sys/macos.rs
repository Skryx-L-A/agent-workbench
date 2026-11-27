//! macOS: whether a process runs under the sandbox (Seatbelt), for the hook of the local way (contract `dienst`, DI-L11 point 5,
//! `aufrufer:sandbox`). `peerkanal` leaves `sandbox` of the peer `null` under macOS in its first Unix stage, so the service asks the
//! system itself; the hook itself stands here too, since only macOS compiles it. This file holds the only `unsafe` of the crate under Unix: one call of `sandbox_check` of `libSystem`, which every
//! program under macOS links already (no new package).

use std::ffi::{c_char, c_int};

unsafe extern "C" {
    /// `libsystem_sandbox` (part of `libSystem`): with no operation and the filter type 0 (`SANDBOX_FILTER_NONE`) it answers whether
    /// the process `pid` runs under a sandbox: 1 yes, 0 no, another value an error. For a pid without a process it answers 1
    /// (measured on 27.11.2026 under macOS 26.5.1), which the hook refuses like a sandbox.
    fn sandbox_check(pid: c_int, operation: *const c_char, filter: c_int, ...) -> c_int;
}

/// `Some(true)` when the process `pid` runs under the sandbox, `Some(false)` when it does not, `None` when the system gives no answer
/// (the caller refuses then).
pub(crate) fn sandkasten(pid: u32) -> Option<bool> {
    let pid = c_int::try_from(pid).ok()?;
    // SAFETY: `sandbox_check` reads only its arguments: a pid, a null pointer for "no operation" (allowed with filter type 0, which
    // reads no further argument) and the filter type. It writes no memory of this process.
    let antwort = unsafe { sandbox_check(pid, std::ptr::null(), 0) };
    match antwort {
        0 => Some(false),
        1 => Some(true),
        _ => None,
    }
}

/// The name of the reason of the hook (`aufrufer:sandbox`, DI-L11 point 5).
const SANDBOX: &str = "sandbox";

/// The hook under macOS: the bound process runs under the sandbox, or the system gives no answer (for example because the process is
/// gone): the name of the reason. `None` only when the system says "no sandbox". `dienst` (the pid of the service) is for Linux.
pub(crate) fn einschluss(
    prozess: Option<&peerkanal::Prozessbindung>,
    _dienst: u32,
) -> Option<&'static str> {
    let Some(p) = prozess else {
        return Some(SANDBOX);
    };
    (sandkasten(p.pid) != Some(false)).then_some(SANDBOX)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The test process runs without a sandbox; a pid no process has is never "without a sandbox", a pid beyond `pid_t` gives no
    /// answer.
    #[test]
    fn eigener_prozess_ohne_sandbox_und_keiner_nie_frei() {
        assert_eq!(sandkasten(std::process::id()), Some(false));
        assert_ne!(sandkasten(99_999_999), Some(false));
        assert_eq!(sandkasten(u32::MAX), None);
    }

    /// Without a process, and for a process the system gives no answer for, the hook refuses; the own process passes.
    #[test]
    fn haken_ohne_prozess_und_ohne_antwort() {
        let binde = |pid| peerkanal::Prozessbindung {
            pid,
            startzeit: None,
        };
        assert_eq!(einschluss(None, 0), Some("sandbox"));
        assert_eq!(einschluss(Some(&binde(u32::MAX)), 0), Some("sandbox"));
        assert_eq!(einschluss(Some(&binde(99_999_999)), 0), Some("sandbox"));
        assert_eq!(einschluss(Some(&binde(std::process::id())), 0), None);
    }
}
