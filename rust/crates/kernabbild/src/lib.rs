//! The limit of core images "1/1" (rule "Kernabbild-Grenze 1/1").
//!
//! A program of the Werkbank that holds a secret in its memory must not leave a core image behind. Under Ubuntu the kernel hands a core
//! image through a pipe to `apport` or `systemd-coredump`, and the limit 0 alone does not stop that (measured in a trial run: a process that
//! could be dumped left a report with the secret in its memory, one that could not be dumped left none). [`abschirmen`] therefore does two things:
//!
//! 1. Under Linux `prctl(PR_SET_DUMPABLE, 0)`: the process cannot be dumped and its `/proc/<pid>` files belong to root.
//! 2. On every system `RLIMIT_CORE` soft 1 and hard 1. The reason for 1 and not 0: with a pipe pattern the kernel stops at the limit 1; with a file
//!    pattern 1 byte is below the smallest size of an image. A hard 0 could not be raised again without root by a child (the starter of the harness
//!    sets 1/1). No program sets the hard limit to 0.
//!
//! If the hard limit is 0 already when the program starts, 1 cannot be reached. Then `/proc/sys/kernel/core_pattern` is read (at most
//! [`MUSTER_MAX`] bytes): no `|` at the start means a file pattern, so no pipe can take an image, and the program goes on with a note for the caller
//! ([`Hinweis`]); a `|` at the start, or a pattern that cannot be read, ends with a fixed reason ([`Abbruch`]). A system without `core_pattern`
//! (macOS) goes on with a note. A failing `prctl` or `setrlimit` ends with a reason (fail closed).
//!
//! The crate returns reasons. It chooses no exit code and writes no text to standard error: every program picks code and message itself.
//! Default of the programs (decision of 21.11.2026): a program that cannot set up this protection ends fail closed with exit code 5. A program
//! that already uses 5 for something else keeps its own code and names the exception with its reason in its contract.
//! The decision about the pattern is the pure function [`entscheiden`]; [`lage`] and [`lage_mit`] give the same decision for the state of the
//! process without any effect.
//!
//! **Order of the call.** [`abschirmen`] is the first act of `main`: before any thread is started (for example by an async runtime) and before a secret
//! is read. After `exec` the child can be dumped again (its `/proc` files belong to the user); only the inherited limit 1/1 protects it then. That
//! holds for the starter of the harness before its `exec` of the harness.
//!
//! **Systems.** The part that needs `rustix::process` stands under `cfg(unix)` (Linux and macOS); a Windows build of the workspace does not stop
//! at this crate. What a Windows program does about core images is a decision of the Windows plan and not part of this crate.

#[cfg(unix)]
use std::path::Path;

#[cfg(unix)]
use rustix::process::{Resource, Rlimit, getrlimit, setrlimit};

/// The longest part of `/proc/sys/kernel/core_pattern` that is read, in bytes.
pub const MUSTER_MAX: u64 = 4096;

/// The limit that is set, soft and hard, in bytes: 1 (see the description of the crate).
pub const GRENZE: u64 = 1;

/// The file that names where the kernel puts a core image.
#[cfg(unix)]
const MUSTER_PFAD: &str = "/proc/sys/kernel/core_pattern";

/// Why [`abschirmen`] refuses: the program must not go on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Abbruch {
    /// `prctl(PR_SET_DUMPABLE, 0)` failed.
    AbzugSperren,
    /// `setrlimit(RLIMIT_CORE, 1/1)` failed.
    GrenzeSetzen,
    /// The hard limit is 0 and the core pattern starts with `|`: a pipe would take an image.
    MusterPipe,
    /// The hard limit is 0 and the core pattern cannot be read.
    MusterNichtLesbar,
}

impl Abbruch {
    /// The fixed reason: a short text without a path and without content, the same in every build.
    #[must_use]
    pub fn grund(self) -> &'static str {
        match self {
            Self::AbzugSperren => "kernabbild.abzug_sperren",
            Self::GrenzeSetzen => "kernabbild.grenze_setzen",
            Self::MusterPipe => "kernabbild.muster_pipe",
            Self::MusterNichtLesbar => "kernabbild.muster_nicht_lesbar",
        }
    }

    /// What the administrator can do, in one sentence.
    #[must_use]
    pub fn abhilfe(self) -> &'static str {
        match self {
            Self::AbzugSperren => {
                "the kernel refused PR_SET_DUMPABLE 0 (seccomp or a security module?); the program must not run without it"
            }
            Self::GrenzeSetzen => {
                "the kernel refused RLIMIT_CORE 1/1; the program must not run without it"
            }
            Self::MusterPipe | Self::MusterNichtLesbar => {
                "the hard limit for core is 0 and a pipe may take an image; raise the hard limit for core to at least 1 (limits.conf, systemd LimitCORE=1 or ulimit -H -c 1)"
            }
        }
    }
}

/// A note for the caller: the program goes on, the limit 1/1 was not set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hinweis {
    /// The hard limit is 0 and the core pattern is a file pattern: no pipe can take an image, no image is written.
    MusterDatei,
    /// The hard limit is 0 and the system has no core pattern (macOS).
    MusterFehlt,
}

impl Hinweis {
    /// The line for the log of the caller, without a path and without content.
    #[must_use]
    pub fn text(self) -> &'static str {
        match self {
            Self::MusterDatei => {
                "kernabbild: the hard limit for core is 0, the core pattern is a file pattern, no pipe takes an image"
            }
            Self::MusterFehlt => {
                "kernabbild: the hard limit for core is 0, this system has no core pattern"
            }
        }
    }
}

/// What was found when the core pattern was read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Muster {
    /// The first bytes of the pattern, at most [`MUSTER_MAX`].
    Inhalt(Vec<u8>),
    /// The pattern exists under this system but cannot be read.
    NichtLesbar,
    /// This system has no core pattern.
    Fehlt,
}

/// The decision of [`entscheiden`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Entscheidung {
    /// Go on and set the limit 1/1.
    Weiter,
    /// Go on without setting the limit (it is 0 and cannot be raised); the caller gets a note.
    Protokoll(Hinweis),
    /// Stop.
    Abbruch(Abbruch),
}

/// The pure decision: what to do with the hard limit `harte_grenze` (`None`: unlimited) of `RLIMIT_CORE` and the core pattern. `muster` is called
/// only when the hard limit is 0, because only then 1 cannot be reached and the pattern matters.
///
/// - the hard limit is not 0: [`Entscheidung::Weiter`], the pattern is not looked at;
/// - the hard limit is 0 and the pattern is missing (macOS): [`Entscheidung::Protokoll`] with [`Hinweis::MusterFehlt`];
/// - the hard limit is 0 and the pattern cannot be read: [`Entscheidung::Abbruch`] with [`Abbruch::MusterNichtLesbar`];
/// - the hard limit is 0 and the first byte of the pattern is `|`: [`Entscheidung::Abbruch`] with [`Abbruch::MusterPipe`];
/// - the hard limit is 0 and any other pattern: [`Entscheidung::Protokoll`] with [`Hinweis::MusterDatei`].
#[must_use]
pub fn entscheiden(harte_grenze: Option<u64>, muster: impl FnOnce() -> Muster) -> Entscheidung {
    if harte_grenze != Some(0) {
        return Entscheidung::Weiter;
    }
    match muster() {
        Muster::Fehlt => Entscheidung::Protokoll(Hinweis::MusterFehlt),
        Muster::NichtLesbar => Entscheidung::Abbruch(Abbruch::MusterNichtLesbar),
        Muster::Inhalt(roh) if roh.first() == Some(&b'|') => {
            Entscheidung::Abbruch(Abbruch::MusterPipe)
        }
        Muster::Inhalt(_) => Entscheidung::Protokoll(Hinweis::MusterDatei),
    }
}

/// The result of [`abschirmen`]: the program may go on. The note [`Abgeschirmt::hinweis`] must be read: with a hard limit 0 and a file pattern
/// the caller writes the log line (a call `abschirmen()?;` would drop it, so the type is `must_use`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[must_use = "the note (`hinweis`) must go to the log of the caller"]
pub struct Abgeschirmt {
    /// A note for the log of the caller: `Some` when the limit 1/1 could not be set (the hard limit was 0 and the pattern is no pipe).
    pub hinweis: Option<Hinweis>,
}

/// Shields the memory of this process from a core image: see the description of the crate.
///
/// # Errors
/// [`Abbruch::AbzugSperren`] or [`Abbruch::GrenzeSetzen`] when the kernel refuses; [`Abbruch::MusterPipe`] or [`Abbruch::MusterNichtLesbar`] when the hard
/// limit is 0 and a pipe may take an image.
#[cfg(unix)]
pub fn abschirmen() -> Result<Abgeschirmt, Abbruch> {
    abschirmen_mit(Path::new(MUSTER_PFAD))
}

/// [`abschirmen`] with another file in the place of `/proc/sys/kernel/core_pattern`: the hook of the tests (the probe program names a file of the test).
///
/// # Errors
/// See [`abschirmen`].
#[cfg(unix)]
pub fn abschirmen_mit(muster_pfad: &Path) -> Result<Abgeschirmt, Abbruch> {
    abzug_sperren()?;
    match lage_mit(muster_pfad) {
        Entscheidung::Abbruch(grund) => Err(grund),
        Entscheidung::Protokoll(hinweis) => Ok(Abgeschirmt {
            hinweis: Some(hinweis),
        }),
        Entscheidung::Weiter => {
            let grenze = Rlimit {
                current: Some(GRENZE),
                maximum: Some(GRENZE),
            };
            setrlimit(Resource::Core, grenze).map_err(|_| Abbruch::GrenzeSetzen)?;
            Ok(Abgeschirmt { hinweis: None })
        }
    }
}

/// The decision of [`abschirmen`] for this process, **without any effect**: no `prctl` and no `setrlimit`. It reads the hard limit of
/// `RLIMIT_CORE` and, when that is 0, the core pattern, and returns what [`abschirmen`] would do about them ([`Entscheidung`]). One check function
/// for the programs that shield themselves and for the programs that only report the state (`werkbank-pruefen`).
#[cfg(unix)]
#[must_use]
pub fn lage() -> Entscheidung {
    lage_mit(Path::new(MUSTER_PFAD))
}

/// [`lage`] with another file in the place of `/proc/sys/kernel/core_pattern` (the hook of the tests).
#[cfg(unix)]
#[must_use]
pub fn lage_mit(muster_pfad: &Path) -> Entscheidung {
    let harte_grenze = getrlimit(Resource::Core).maximum;
    // other systems than Linux have no core pattern in a file
    let muster = || {
        if cfg!(target_os = "linux") {
            muster_aus(muster_pfad)
        } else {
            Muster::Fehlt
        }
    };
    entscheiden(harte_grenze, muster)
}

/// `prctl(PR_SET_DUMPABLE, 0)` under Linux; other systems have no such call.
#[cfg(target_os = "linux")]
fn abzug_sperren() -> Result<(), Abbruch> {
    rustix::process::set_dumpable_behavior(rustix::process::DumpableBehavior::NotDumpable)
        .map_err(|_| Abbruch::AbzugSperren)
}

/// Other systems than Linux have no `PR_SET_DUMPABLE`: nothing to do.
#[cfg(all(unix, not(target_os = "linux")))]
#[allow(clippy::unnecessary_wraps)]
fn abzug_sperren() -> Result<(), Abbruch> {
    Ok(())
}

/// Reads the first [`MUSTER_MAX`] bytes of the file `pfad` (compiled on every Unix system, so the tests and the mutants reach it everywhere).
#[cfg(unix)]
fn muster_aus(pfad: &Path) -> Muster {
    use std::io::Read;
    let Ok(datei) = std::fs::File::open(pfad) else {
        return Muster::NichtLesbar;
    };
    let mut roh = Vec::new();
    if datei.take(MUSTER_MAX).read_to_end(&mut roh).is_err() {
        return Muster::NichtLesbar;
    }
    Muster::Inhalt(roh)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inhalt(text: &str) -> Muster {
        Muster::Inhalt(text.as_bytes().to_vec())
    }

    #[test]
    fn eine_harte_grenze_ueber_null_geht_weiter_und_liest_das_muster_nicht() {
        for grenze in [Some(1), Some(2), Some(u64::MAX), None] {
            assert_eq!(
                entscheiden(grenze, || unreachable!("the pattern is not read")),
                Entscheidung::Weiter,
                "{grenze:?}"
            );
        }
    }

    #[test]
    fn harte_null_mit_einem_muster_ohne_pipe_ist_ein_protokoll() {
        for muster in [
            "core",
            "core.%p",
            "/var/crash/core.%e.%p",
            "",
            " |/usr/bin/x %p",
            "\n|x",
            "c|ore",
            "||",
        ] {
            let wert = if muster == "||" {
                // a pattern that starts with `|` is a pipe even if it holds a second one
                Entscheidung::Abbruch(Abbruch::MusterPipe)
            } else {
                Entscheidung::Protokoll(Hinweis::MusterDatei)
            };
            assert_eq!(entscheiden(Some(0), || inhalt(muster)), wert, "{muster:?}");
        }
    }

    #[test]
    fn harte_null_mit_pipe_am_anfang_bricht_ab() {
        for muster in [
            "|/usr/share/apport/apport -p%p -s%s -c%c -d%d -P%P -u%u -g%g -- %E",
            "|/lib/systemd/systemd-coredump %P %u %g %s %t %c %h",
            "|",
        ] {
            assert_eq!(
                entscheiden(Some(0), || inhalt(muster)),
                Entscheidung::Abbruch(Abbruch::MusterPipe),
                "{muster:?}"
            );
        }
    }

    #[test]
    fn harte_null_mit_nicht_lesbarem_muster_bricht_ab_und_ohne_muster_geht_es_weiter() {
        assert_eq!(
            entscheiden(Some(0), || Muster::NichtLesbar),
            Entscheidung::Abbruch(Abbruch::MusterNichtLesbar)
        );
        assert_eq!(
            entscheiden(Some(0), || Muster::Fehlt),
            Entscheidung::Protokoll(Hinweis::MusterFehlt)
        );
    }

    #[test]
    fn die_gruende_und_texte_sind_fest_und_verschieden() {
        let alle = [
            Abbruch::AbzugSperren,
            Abbruch::GrenzeSetzen,
            Abbruch::MusterPipe,
            Abbruch::MusterNichtLesbar,
        ];
        let gruende: Vec<&str> = alle.iter().map(|a| a.grund()).collect();
        assert_eq!(
            gruende,
            [
                "kernabbild.abzug_sperren",
                "kernabbild.grenze_setzen",
                "kernabbild.muster_pipe",
                "kernabbild.muster_nicht_lesbar"
            ]
        );
        for a in alle {
            assert_ne!(a.abhilfe(), "");
            assert!(!a.grund().contains('/'), "no path in a reason");
        }
        // the remedy for a pipe names the hard limit for core
        for a in [Abbruch::MusterPipe, Abbruch::MusterNichtLesbar] {
            assert!(a.abhilfe().contains("hard limit for core"));
            assert!(a.abhilfe().contains("at least 1"));
        }
        assert_ne!(Hinweis::MusterDatei.text(), Hinweis::MusterFehlt.text());
        assert!(Hinweis::MusterDatei.text().starts_with("kernabbild: "));
        assert!(Hinweis::MusterFehlt.text().starts_with("kernabbild: "));
    }

    #[test]
    fn die_grenzen_sind_eins_und_viertausendsechsundneunzig() {
        assert_eq!(GRENZE, 1);
        assert_eq!(MUSTER_MAX, 4096);
    }

    #[cfg(unix)]
    #[test]
    fn lage_mit_ist_die_entscheidung_aus_grenze_und_muster_ohne_wirkung() {
        use rustix::process::{Resource, getrlimit};
        let vorher = getrlimit(Resource::Core);
        let pfad = Path::new("/nicht/da/core_pattern");
        let erwartet = entscheiden(vorher.maximum, || muster_aus(pfad));
        assert_eq!(lage_mit(pfad), erwartet);
        assert_eq!(
            lage(),
            entscheiden(vorher.maximum, || muster_aus(Path::new(MUSTER_PFAD)))
        );
        let nachher = getrlimit(Resource::Core);
        assert_eq!(
            (vorher.current, vorher.maximum),
            (nachher.current, nachher.maximum),
            "lage changes no limit"
        );
    }

    #[cfg(unix)]
    #[test]
    fn das_muster_wird_aus_einer_datei_gelesen_hoechstens_4096_byte() {
        use std::os::unix::fs::PermissionsExt;
        let ordner = std::env::temp_dir().join(format!("kernabbild-muster-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&ordner);
        std::fs::create_dir_all(&ordner).expect("folder");
        let datei = ordner.join("core_pattern");
        std::fs::write(&datei, "|/bin/x %p\n").expect("write");
        assert_eq!(muster_aus(&datei), inhalt("|/bin/x %p\n"));
        std::fs::write(&datei, "").expect("write");
        assert_eq!(muster_aus(&datei), inhalt(""));
        // longer than the bound: the first 4096 bytes only
        let lang = "a".repeat(5000);
        std::fs::write(&datei, &lang).expect("write");
        assert_eq!(muster_aus(&datei), inhalt(&lang[..4096]));
        std::fs::write(&datei, "a".repeat(4096)).expect("write");
        assert_eq!(muster_aus(&datei), inhalt(&"a".repeat(4096)));
        // missing, a folder in its place, no rights: not readable
        assert_eq!(
            muster_aus(&ordner.join("gibt-es-nicht")),
            Muster::NichtLesbar
        );
        assert_eq!(muster_aus(&ordner), Muster::NichtLesbar);
        std::fs::write(&datei, "core").expect("write");
        std::fs::set_permissions(&datei, std::fs::Permissions::from_mode(0o000)).expect("mode");
        let eigener_nutzer = rustix::process::geteuid().as_raw() != 0;
        if eigener_nutzer {
            assert_eq!(muster_aus(&datei), Muster::NichtLesbar);
        }
        let _ = std::fs::remove_dir_all(&ordner);
    }
}
