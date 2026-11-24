//! Protection of the process against a core image (contract 9, section 3c; rule "Kernabbild-Grenze 1/1" of the crate `kernabbild`). The
//! command holds the token and the text of `config.json` in its memory; a core image (for example after SIGQUIT, through `core_pattern`)
//! would put them outside the key file. [`abschirmen`] is the first act of `main`, for both ways, before anything is read. [`vor_dem_kind`]
//! checks the same state again right before the child `copilot login` starts, because the child inherits it through fork and exec.

use crate::fehler::Fehler;

/// The exit code of a start that cannot shield the memory of the process from core images (default of the Werkbank, 21.11.2026).
pub const EXIT_ABSCHIRMUNG: u8 = 5;

/// Shields this process through `kernabbild::abschirmen`: under Linux `PR_SET_DUMPABLE` 0, on every system `RLIMIT_CORE` soft 1 and hard 1;
/// with an inherited hard limit 0 the core pattern decides (see the crate). `Ok(Some(_))` is the note for the error output.
///
/// # Errors
/// `Abschirmung` with the reason of the crate (fail closed).
pub fn abschirmen() -> Result<Option<kernabbild::Hinweis>, (Fehler, kernabbild::Abbruch)> {
    abschirmung_auswerten(kernabbild::abschirmen())
}

/// The answer of the crate as the result of the command: any abort is `Abschirmung`, together with the reason of the crate.
///
/// # Errors
/// `Abschirmung` for any `Err`.
pub fn abschirmung_auswerten(
    antwort: Result<kernabbild::Abgeschirmt, kernabbild::Abbruch>,
) -> Result<Option<kernabbild::Hinweis>, (Fehler, kernabbild::Abbruch)> {
    antwort
        .map(|a| a.hinweis)
        .map_err(|grund| (Fehler::Abschirmung, grund))
}

/// The state of this process right before the child starts: the soft and the hard limit of `RLIMIT_CORE` (`None`: unlimited) and whether the
/// process can be dumped (under Linux `PR_GET_DUMPABLE`; other systems have no such flag: `false`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stand {
    /// Soft limit of `RLIMIT_CORE`.
    pub weich: Option<u64>,
    /// Hard limit of `RLIMIT_CORE`.
    pub hart: Option<u64>,
    /// The process can be dumped.
    pub abzugsfaehig: bool,
}

/// The pure check before the child: the child inherits this state through fork (it is not dumpable until its `exec`) and the limit also
/// after `exec`. It may start only when the process is not dumpable and the limit is 1/1, or 0/0 when the crate decides without abort for the
/// inherited hard limit 0 (a file pattern or no pattern). `lage` is asked only for 0/0.
///
/// # Errors
/// `Abschirmung` for every other state (fail closed): the CLI does not start.
pub fn kind_pruefen(
    stand: Stand,
    lage: impl FnOnce() -> kernabbild::Entscheidung,
) -> Result<(), Fehler> {
    if stand.abzugsfaehig {
        return Err(Fehler::Abschirmung);
    }
    match (stand.weich, stand.hart) {
        (Some(kernabbild::GRENZE), Some(kernabbild::GRENZE)) => Ok(()),
        (Some(0), Some(0)) => match lage() {
            kernabbild::Entscheidung::Abbruch(_) => Err(Fehler::Abschirmung),
            kernabbild::Entscheidung::Weiter | kernabbild::Entscheidung::Protokoll(_) => Ok(()),
        },
        _ => Err(Fehler::Abschirmung),
    }
}

/// [`kind_pruefen`] for this process.
///
/// # Errors
/// `Abschirmung` when the state is not the shielded one or cannot be read.
pub fn vor_dem_kind() -> Result<(), Fehler> {
    let grenze = rustix::process::getrlimit(rustix::process::Resource::Core);
    let stand = Stand {
        weich: grenze.current,
        hart: grenze.maximum,
        abzugsfaehig: abzugsfaehig()?,
    };
    kind_pruefen(stand, kernabbild::lage)
}

#[cfg(target_os = "linux")]
fn abzugsfaehig() -> Result<bool, Fehler> {
    rustix::process::dumpable_behavior()
        .map(|b| b != rustix::process::DumpableBehavior::NotDumpable)
        .map_err(|_| Fehler::Abschirmung)
}

#[cfg(not(target_os = "linux"))]
#[allow(clippy::unnecessary_wraps)]
fn abzugsfaehig() -> Result<bool, Fehler> {
    Ok(false)
}
