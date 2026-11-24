//! The Linux part of the lock: the rules, the BPF program and the two ways to use it. Only `x86_64` and `aarch64`.
//!
//! The numbers come from `libc` for the architecture that is built, so none of them is written down twice:
//! `SYS_setrlimit` (`x86_64` 160, `aarch64` 164), `SYS_prlimit64` (`x86_64` 302, `aarch64` 261).
//!
//! **Foreign ABIs.** A seccomp program sees the ABI of a call in `seccomp_data.arch`. The program built here starts with a comparison of that
//! field against [`ARCH_AUDIT`]; a call through the 32-bit ABI of the same machine (i386 on `x86_64`, arm on `aarch64`) carries another value and
//! ends the process (`SECCOMP_RET_KILL_PROCESS`, fixed in seccompiler, `src/backend/bpf.rs:73-80`). The numbers of those ABIs are therefore
//! never compared, and a 32-bit program cannot run under this lock.
//!
//! **x32 on `x86_64`.** That ABI carries the same audit value, but its numbers have bit 30 set (`__X32_SYSCALL_BIT`). The rules therefore hold
//! both forms of each number on `x86_64`. Whether the kernel of the system has `CONFIG_X86_X32_ABI` at all is not measured; the four extra
//! comparisons close the gap either way.

use std::collections::BTreeMap;

use seccompiler::{
    BpfProgram, SeccompAction, SeccompCmpArgLen, SeccompCmpOp, SeccompCondition, SeccompFilter,
    SeccompRule, TargetArch,
};

use crate::{
    ARG_PRLIMIT_NEU, ARG_PRLIMIT_RESSOURCE, ARG_SETRLIMIT_RESSOURCE, Abbruch, FEHLERNUMMER,
    RESSOURCE_CORE,
};

/// How long a `struct sock_filter` is on the wire: `code` (2 bytes), `jt` (1), `jf` (1), `k` (4). `bwrap --seccomp <fd>` refuses a length that
/// is not a multiple of this (measured in bubblewrap, `bubblewrap.c:260`).
pub const BEFEHL_BYTES: usize = 8;

/// Bit 30 of a system call number on `x86_64`: the mark of the x32 ABI (`__X32_SYSCALL_BIT`).
#[cfg(target_arch = "x86_64")]
const X32: i64 = 0x4000_0000;

/// The audit value of this ABI, the value the program compares `seccomp_data.arch` against: `EM_X86_64` (62) with the bits for 64 bit
/// (`0x8000_0000`) and little endian (`0x4000_0000`).
#[cfg(target_arch = "x86_64")]
pub const ARCH_AUDIT: u32 = 0xc000_003e;

/// The audit value of this ABI: `EM_AARCH64` (183) with the bits for 64 bit and little endian.
#[cfg(target_arch = "aarch64")]
pub const ARCH_AUDIT: u32 = 0xc000_00b7;

/// The numbers of `setrlimit` that the lock refuses for `RLIMIT_CORE`: on `x86_64` the native one and its x32 form.
#[cfg(target_arch = "x86_64")]
pub const SETZEN_NUMMERN: &[i64] = &[libc::SYS_setrlimit, libc::SYS_setrlimit | X32];

/// The numbers of `prlimit64` that the lock refuses for `RLIMIT_CORE` with a new value: on `x86_64` the native one and its x32 form.
#[cfg(target_arch = "x86_64")]
pub const PRLIMIT_NUMMERN: &[i64] = &[libc::SYS_prlimit64, libc::SYS_prlimit64 | X32];

/// The numbers of `setrlimit` that the lock refuses for `RLIMIT_CORE`.
#[cfg(target_arch = "aarch64")]
pub const SETZEN_NUMMERN: &[i64] = &[libc::SYS_setrlimit];

/// The numbers of `prlimit64` that the lock refuses for `RLIMIT_CORE` with a new value.
#[cfg(target_arch = "aarch64")]
pub const PRLIMIT_NUMMERN: &[i64] = &[libc::SYS_prlimit64];

/// The target of the program: the architecture that is built.
#[must_use]
pub fn ziel_arch() -> TargetArch {
    #[cfg(target_arch = "x86_64")]
    {
        TargetArch::x86_64
    }
    #[cfg(target_arch = "aarch64")]
    {
        TargetArch::aarch64
    }
}

/// Builds the BPF program of the lock: everything is allowed except the calls of [`crate::verweigert`], which get [`FEHLERNUMMER`].
///
/// # Errors
/// [`Abbruch::Bauen`] when a rule or the program cannot be built. That is a fault of this build, not of the system.
pub fn sperre_bauen() -> Result<BpfProgram, Abbruch> {
    let ist_core = |stelle: u8| {
        // four bytes, not eight: the kernel cuts the resource to 32 bits (see `crate::ist_core`)
        SeccompCondition::new(
            stelle,
            SeccompCmpArgLen::Dword,
            SeccompCmpOp::Eq,
            RESSOURCE_CORE,
        )
        .map_err(|_| Abbruch::Bauen)
    };
    let mut regeln: BTreeMap<i64, Vec<SeccompRule>> = BTreeMap::new();
    for nummer in SETZEN_NUMMERN {
        let regel = SeccompRule::new(vec![ist_core(ARG_SETRLIMIT_RESSOURCE)?])
            .map_err(|_| Abbruch::Bauen)?;
        regeln.insert(*nummer, vec![regel]);
    }
    for nummer in PRLIMIT_NUMMERN {
        // eight bytes against 0: a pointer whose lower half is 0 is not a null pointer
        let neu_gesetzt = SeccompCondition::new(
            ARG_PRLIMIT_NEU,
            SeccompCmpArgLen::Qword,
            SeccompCmpOp::Ne,
            0,
        )
        .map_err(|_| Abbruch::Bauen)?;
        let regel = SeccompRule::new(vec![ist_core(ARG_PRLIMIT_RESSOURCE)?, neu_gesetzt])
            .map_err(|_| Abbruch::Bauen)?;
        regeln.insert(*nummer, vec![regel]);
    }
    let filter = SeccompFilter::new(
        regeln,
        SeccompAction::Allow,
        SeccompAction::Errno(FEHLERNUMMER),
        ziel_arch(),
    )
    .map_err(|_| Abbruch::Bauen)?;
    BpfProgram::try_from(filter).map_err(|_| Abbruch::Bauen)
}

/// The program as the bytes that `bwrap --seccomp <fd>` reads: [`BEFEHL_BYTES`] per instruction, in the order `code`, `jt`, `jf`, `k`, in the
/// byte order of this machine. The seam that hands them over is not part of this crate (see `rust/spec/VERTRAEGE.md`, section 4a).
///
/// # Errors
/// See [`sperre_bauen`].
pub fn programm_bytes() -> Result<Vec<u8>, Abbruch> {
    let programm = sperre_bauen()?;
    let mut bytes = Vec::with_capacity(programm.len() * BEFEHL_BYTES);
    for befehl in &programm {
        bytes.extend_from_slice(&befehl.code.to_ne_bytes());
        bytes.push(befehl.jt);
        bytes.push(befehl.jf);
        bytes.extend_from_slice(&befehl.k.to_ne_bytes());
    }
    Ok(bytes)
}

/// Installs the lock for this thread: first `prctl(PR_SET_NO_NEW_PRIVS, 1)`, then the program. Both are inherited through `fork` and `exec`
/// and cannot be taken back.
///
/// The caller must set the limit `RLIMIT_CORE` 1/1 **before** this call (`kernabbild::abschirmen`): afterwards the lock refuses its own
/// `setrlimit` as well. A program that cannot install the lock must not go on (fail closed, exit code 5 by the default of the Werkbank).
///
/// # Errors
/// [`Abbruch::NeueRechteSperren`] when `prctl` fails, [`Abbruch::SperreSetzen`] when the kernel refuses the filter, [`Abbruch::Bauen`] from
/// [`sperre_bauen`].
pub fn installieren() -> Result<(), Abbruch> {
    let programm = sperre_bauen()?;
    rustix::thread::set_no_new_privs(true).map_err(|_| Abbruch::NeueRechteSperren)?;
    seccompiler::apply_filter(&programm).map_err(|_| Abbruch::SperreSetzen)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The opcodes of the instructions this test reads (`linux/bpf_common.h`), each the sum of its parts:
    /// `BPF_LD | BPF_W | BPF_ABS` (load a word at a fixed place), `BPF_JMP | BPF_JEQ | BPF_K` (jump on equal) and `BPF_RET | BPF_K`.
    const LD_W_ABS: u16 = 0x20;
    const JMP_JEQ_K: u16 = 0x15;
    const RET_K: u16 = 0x06;
    /// The place of `arch` in `struct seccomp_data` (`nr` is at 0).
    const ARCH_OFFSET: u32 = 4;
    /// The place of the first argument in `struct seccomp_data` and the size of one argument: the lower half of
    /// argument `n` lies at `ARGS_OFFSET + n * ARG_SIZE`, the upper half four bytes behind it.
    const ARGS_OFFSET: u32 = 16;
    const ARG_SIZE: u32 = 8;
    /// `SECCOMP_RET_ALLOW`, `SECCOMP_RET_ERRNO` and `SECCOMP_RET_KILL_PROCESS` (`linux/seccomp.h`).
    const RET_ALLOW: u32 = 0x7fff_0000;
    const RET_ERRNO: u32 = 0x0005_0000;
    const RET_KILL_PROCESS: u32 = 0x8000_0000;

    #[test]
    fn das_programm_prueft_zuerst_die_abi_und_toetet_bei_einer_fremden() {
        let programm = sperre_bauen().expect("the program is built");
        assert_eq!(programm[0].code, LD_W_ABS);
        assert_eq!(programm[0].k, ARCH_OFFSET);
        assert_eq!(programm[1].code, JMP_JEQ_K);
        assert_eq!(programm[1].k, ARCH_AUDIT);
        assert_eq!(programm[1].jt, 1, "a fitting ABI jumps over the kill");
        assert_eq!(programm[1].jf, 0);
        assert_eq!(programm[2].code, RET_K);
        assert_eq!(
            programm[2].k, RET_KILL_PROCESS,
            "a foreign ABI ends the process"
        );
    }

    #[test]
    fn jede_nummer_der_sperre_steht_genau_einmal_im_programm() {
        let programm = sperre_bauen().expect("the program is built");
        for nummer in SETZEN_NUMMERN.iter().chain(PRLIMIT_NUMMERN) {
            let gesucht = u32::try_from(*nummer).expect("a syscall number fits in 32 bits");
            let treffer = programm
                .iter()
                .filter(|b| b.code == JMP_JEQ_K && b.k == gesucht)
                .count();
            assert_eq!(treffer, 1, "number {nummer}");
        }
    }

    #[test]
    fn das_programm_erlaubt_als_letztes_und_verweigert_mit_eperm() {
        let programm = sperre_bauen().expect("the program is built");
        let letzter = programm.last().expect("the program is not empty");
        assert_eq!(letzter.code, RET_K);
        assert_eq!(
            letzter.k, RET_ALLOW,
            "what no rule matches stays allowed (denylist)"
        );
        let verweigerungen = programm
            .iter()
            .filter(|b| b.code == RET_K && b.k == (RET_ERRNO | FEHLERNUMMER))
            .count();
        assert_eq!(
            verweigerungen,
            SETZEN_NUMMERN.len() + PRLIMIT_NUMMERN.len(),
            "one refusal per number"
        );
    }

    /// The place of the lower and the upper half of argument `n` in `struct seccomp_data`.
    fn haelften(n: u32) -> (u32, u32) {
        let unten = ARGS_OFFSET + n * ARG_SIZE;
        (unten, unten + ARG_SIZE / 2)
    }

    /// Whether the program loads a word at this place.
    fn laedt(programm: &[seccompiler::sock_filter], stelle: u32) -> bool {
        programm.iter().any(|b| b.code == LD_W_ABS && b.k == stelle)
    }

    /// The security-critical widths: the resource over four bytes, the pointer over eight. A resource compared over
    /// eight bytes would let a call with `0x1_0000_0004` through, because the kernel cuts the resource to 32 bits
    /// and would see `RLIMIT_CORE`. A pointer compared over four bytes would read a pointer whose lower half is 0
    /// as a null pointer, so `prlimit64` with that pointer would count as reading. The widths cannot be measured
    /// with a live call from this workspace (no `unsafe`, and `rustix` builds the argument itself), so the program
    /// itself is the evidence: a comparison over eight bytes loads the upper half, one over four bytes does not.
    #[test]
    fn die_ressource_wird_ueber_vier_byte_und_der_zeiger_ueber_acht_verglichen() {
        let programm = sperre_bauen().expect("the program is built");
        for arg in [
            u32::from(ARG_SETRLIMIT_RESSOURCE),
            u32::from(ARG_PRLIMIT_RESSOURCE),
        ] {
            let (unten, oben) = haelften(arg);
            assert!(laedt(&programm, unten), "argument {arg}, lower half");
            assert!(
                !laedt(&programm, oben),
                "argument {arg}: the resource must not be compared over eight bytes"
            );
        }
        let (unten, oben) = haelften(u32::from(ARG_PRLIMIT_NEU));
        assert!(laedt(&programm, unten), "the pointer, lower half");
        assert!(
            laedt(&programm, oben),
            "the pointer must be compared over eight bytes"
        );
    }

    #[test]
    fn die_zahlen_sind_die_von_libc_fuer_diese_architektur() {
        assert_eq!(RESSOURCE_CORE, u64::from(libc::RLIMIT_CORE));
        assert_eq!(
            FEHLERNUMMER,
            u32::try_from(libc::EPERM).expect("EPERM is not negative")
        );
        assert!(SETZEN_NUMMERN.contains(&libc::SYS_setrlimit));
        assert!(PRLIMIT_NUMMERN.contains(&libc::SYS_prlimit64));
    }

    #[test]
    fn die_bytes_sind_acht_je_befehl_und_tragen_jeden_wert() {
        let programm = sperre_bauen().expect("the program is built");
        let bytes = programm_bytes().expect("the bytes are built");
        assert_eq!(bytes.len(), programm.len() * BEFEHL_BYTES);
        assert_eq!(
            bytes.len() % BEFEHL_BYTES,
            0,
            "bwrap refuses another length"
        );
        for (i, befehl) in programm.iter().enumerate() {
            let stueck = &bytes[i * BEFEHL_BYTES..(i + 1) * BEFEHL_BYTES];
            assert_eq!(stueck[0..2], befehl.code.to_ne_bytes());
            assert_eq!(stueck[2], befehl.jt);
            assert_eq!(stueck[3], befehl.jf);
            assert_eq!(stueck[4..8], befehl.k.to_ne_bytes());
        }
    }
}
