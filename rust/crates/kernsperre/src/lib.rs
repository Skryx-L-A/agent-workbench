//! The lock on the core-image limit (rule "Kernabbild-Grenze 1/1", finding N3).
//!
//! The crate [`kernabbild`](../kernabbild/index.html) sets `RLIMIT_CORE` soft 1 and hard 1, and under Linux `PR_SET_DUMPABLE` 0. That holds for a
//! child as it starts, but not against the child itself: after an `exec` without a change of credentials the kernel makes the process dumpable
//! again, and a child that lowers its **soft** limit from 1 to 0 opens the pipe to `apport` once more. Measured in a throw-away VM on
//! 22.11.2026 (Ubuntu 26.04.1, kernel 7.0.0-43-generic, apport 2.34.1, case N3): the report with the secret in it was written.
//!
//! This crate closes that way with a seccomp filter: every **setting** of `RLIMIT_CORE` is refused with `EPERM`, while reading stays allowed.
//!
//! - `setrlimit(RLIMIT_CORE, …)` is refused, whatever the values.
//! - `prlimit64(…, RLIMIT_CORE, new, old)` is refused when `new` is not a null pointer, for any process id in the first argument.
//! - `prlimit64` with `new` as a null pointer and `getrlimit` are untouched: they only read. So does
//!   `rustix::process::getrlimit`, which is a `prlimit64(0, resource, NULL, &out)` (measured in rustix 1.1.6,
//!   `src/backend/linux_raw/process/syscalls.rs:200`).
//!
//! **Why `EPERM` and not the death of the process.** In the kernel source v7.0, `kernel/seccomp.c:1355-1370`, a `SECCOMP_RET_KILL_*` action
//! ends in `force_sig_seccomp(…)` under the comment "Trigger a coredump with SIGSYS": the kernel enters the very path this crate closes.
//! `SECCOMP_RET_ERRNO` sets the return value and skips the call (`:1279-1285`), without a signal and without a dump. Fail closed is given
//! either way, because the protection is the limit that stays, not the death of the caller.
//!
//! **Order of the call.** The filter must be installed after the last change of an `AppArmor` profile, because a filter needs
//! `PR_SET_NO_NEW_PRIVS` and that flag makes the kernel refuse such a change (measured 21.11.2026, see `rust/spec/VERTRAEGE.md`). It must also
//! be installed after `kernabbild::abschirmen`, whose own `setrlimit` the filter would otherwise refuse. The two seams and what they need
//! stand in `rust/spec/VERTRAEGE.md`; this crate builds no seam.
//!
//! **Inheritance.** A filter survives `fork` and `exec` and cannot be removed; every child and every child of a child carries it. Further
//! filters can only narrow it. `PR_SET_NO_NEW_PRIVS` is inherited as well and cannot be taken back.
//!
//! **Systems.** Only Linux on `x86_64` and `aarch64` has the functions [`installieren`], [`programm_bytes`] and [`sperre_bauen`]; on every
//! other target they do not exist, so a build cannot silently run without the lock. What macOS and Windows need instead is part of their own
//! contracts, not of this crate. The decision table [`verweigert`] is pure and exists everywhere.

/// The resource number of `RLIMIT_CORE`: 4 on every Linux architecture (`libc::RLIMIT_CORE`, checked against it in the tests).
pub const RESSOURCE_CORE: u64 = 4;

/// The error the filter returns for a refused call: `EPERM` (1, checked against `libc::EPERM` in the tests). It is the error the kernel itself
/// gives when a process without `CAP_SYS_RESOURCE` wants to raise a hard limit.
pub const FEHLERNUMMER: u32 = 1;

/// The place of the resource in the arguments of `setrlimit(resource, new)`.
pub const ARG_SETRLIMIT_RESSOURCE: u8 = 0;

/// The place of the resource in the arguments of `prlimit64(pid, resource, new, old)`.
pub const ARG_PRLIMIT_RESSOURCE: u8 = 1;

/// The place of the new value in the arguments of `prlimit64(pid, resource, new, old)`. A null pointer there means: only read.
pub const ARG_PRLIMIT_NEU: u8 = 2;

/// A call that the lock judges: the pure form of what the filter sees of a system call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Aufruf {
    /// `setrlimit(resource, new)`: it always writes.
    Setrlimit {
        /// The first argument, as the register holds it (the kernel cuts it to 32 bits).
        ressource: u64,
    },
    /// `prlimit64(pid, resource, new, old)`.
    Prlimit64 {
        /// The second argument, as the register holds it (the kernel cuts it to 32 bits).
        ressource: u64,
        /// The third argument: the pointer to the new value, 0 for a null pointer (only read).
        neu: u64,
    },
    /// `getrlimit(resource, old)` and every other call that only reads.
    Lesen,
}

/// Whether the lock refuses this call. The one decision table of the crate: [`sperre_bauen`] turns it into BPF, and the tests on Linux check
/// that both say the same.
///
/// - [`Aufruf::Lesen`]: never refused.
/// - [`Aufruf::Setrlimit`]: refused when the resource is `RLIMIT_CORE`.
/// - [`Aufruf::Prlimit64`]: refused when the resource is `RLIMIT_CORE` **and** `neu` is not a null pointer.
#[must_use]
pub fn verweigert(aufruf: Aufruf) -> bool {
    match aufruf {
        Aufruf::Lesen => false,
        Aufruf::Setrlimit { ressource } => ist_core(ressource),
        Aufruf::Prlimit64 { ressource, neu } => ist_core(ressource) && neu != 0,
    }
}

/// Whether this register value names `RLIMIT_CORE`. The resource is a 32-bit value in the system call, and the kernel drops the upper bits;
/// a comparison over all 64 bits would let a call with `0x1_0000_0004` through while the kernel reads `RLIMIT_CORE`. The filter compares the
/// same four bytes (`SeccompCmpArgLen::Dword`), and so does this function.
fn ist_core(ressource: u64) -> bool {
    ressource & 0xffff_ffff == RESSOURCE_CORE
}

/// Why the lock could not be installed: the program must not go on (fail closed).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Abbruch {
    /// The BPF program could not be built from the rules.
    Bauen,
    /// `prctl(PR_SET_NO_NEW_PRIVS, 1)` failed. Without it the kernel refuses a filter.
    NeueRechteSperren,
    /// `seccomp(SECCOMP_SET_MODE_FILTER, …)` failed.
    SperreSetzen,
}

impl Abbruch {
    /// The fixed reason: a short text without a path and without content, the same in every build.
    #[must_use]
    pub fn grund(self) -> &'static str {
        match self {
            Self::Bauen => "kernsperre.bauen",
            Self::NeueRechteSperren => "kernsperre.neue_rechte_sperren",
            Self::SperreSetzen => "kernsperre.sperre_setzen",
        }
    }

    /// What the administrator can do, in one sentence.
    #[must_use]
    pub fn abhilfe(self) -> &'static str {
        match self {
            Self::Bauen => {
                "the filter of this build is faulty; this is a fault of the program, not of the system"
            }
            Self::NeueRechteSperren => {
                "the kernel refused PR_SET_NO_NEW_PRIVS (a security module?); without it no system call filter can be set"
            }
            Self::SperreSetzen => {
                "the kernel refused the system call filter; it needs CONFIG_SECCOMP and CONFIG_SECCOMP_FILTER"
            }
        }
    }
}

#[cfg(all(
    target_os = "linux",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
mod sperre;

#[cfg(all(
    target_os = "linux",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
pub use sperre::{
    ARCH_AUDIT, BEFEHL_BYTES, PRLIMIT_NUMMERN, SETZEN_NUMMERN, installieren, programm_bytes,
    sperre_bauen, ziel_arch,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn das_setzen_der_kernabbild_grenze_ist_verweigert() {
        assert!(verweigert(Aufruf::Setrlimit {
            ressource: RESSOURCE_CORE
        }));
        assert!(verweigert(Aufruf::Prlimit64 {
            ressource: RESSOURCE_CORE,
            neu: 0x7fff_0000_1000,
        }));
    }

    #[test]
    fn lesen_bleibt_erlaubt() {
        assert!(!verweigert(Aufruf::Lesen));
        // prlimit64 with a null pointer as the new value only reads
        assert!(!verweigert(Aufruf::Prlimit64 {
            ressource: RESSOURCE_CORE,
            neu: 0,
        }));
    }

    #[test]
    fn eine_andere_ressource_bleibt_erlaubt() {
        for ressource in [0, 1, 2, 3, 5, 6, 7, 8, 9, 15, u64::from(u32::MAX)] {
            assert!(!verweigert(Aufruf::Setrlimit { ressource }), "{ressource}");
            assert!(
                !verweigert(Aufruf::Prlimit64 {
                    ressource,
                    neu: 0x1000
                }),
                "{ressource}"
            );
        }
    }

    #[test]
    fn hohe_bits_der_ressource_umgehen_die_sperre_nicht() {
        // the kernel cuts the resource to 32 bits: every one of these values is RLIMIT_CORE for it
        for ressource in [
            RESSOURCE_CORE,
            0x1_0000_0004,
            0xffff_ffff_0000_0004,
            0xdead_beef_0000_0004,
        ] {
            assert!(
                verweigert(Aufruf::Setrlimit { ressource }),
                "{ressource:#x}"
            );
            assert!(
                verweigert(Aufruf::Prlimit64 {
                    ressource,
                    neu: 0x1000
                }),
                "{ressource:#x}"
            );
        }
        // the upper bits alone are not RLIMIT_CORE
        assert!(!verweigert(Aufruf::Setrlimit {
            ressource: 0x0000_0004_0000_0000
        }));
    }

    #[test]
    fn die_gruende_und_texte_sind_fest_und_verschieden() {
        let alle = [
            Abbruch::Bauen,
            Abbruch::NeueRechteSperren,
            Abbruch::SperreSetzen,
        ];
        let gruende: Vec<&str> = alle.iter().map(|a| a.grund()).collect();
        assert_eq!(
            gruende,
            [
                "kernsperre.bauen",
                "kernsperre.neue_rechte_sperren",
                "kernsperre.sperre_setzen"
            ]
        );
        for a in alle {
            assert!(a.grund().starts_with("kernsperre."));
            assert!(!a.grund().contains('/'), "no path in a reason");
            assert_ne!(a.abhilfe(), "");
        }
        assert!(
            Abbruch::NeueRechteSperren
                .abhilfe()
                .contains("PR_SET_NO_NEW_PRIVS")
        );
    }

    #[test]
    fn die_zahlen_der_sperre_stehen_fest() {
        assert_eq!(RESSOURCE_CORE, 4);
        assert_eq!(FEHLERNUMMER, 1);
        assert_eq!(ARG_SETRLIMIT_RESSOURCE, 0);
        assert_eq!(ARG_PRLIMIT_RESSOURCE, 1);
        assert_eq!(ARG_PRLIMIT_NEU, 2);
    }
}
