//! The fixed starter `werkbank-einschluss` (contract 1, the Linux enclosure behind the `AppArmor` restriction).
//!
//! On Ubuntu with `kernel.apparmor_restrict_unprivileged_userns = 1` the only program that may build the user
//! namespace is this one: its profile carries the transition `/usr/bin/bwrap Px`. The program "Lauf starten" carries no
//! profile and replaces itself (`exec`, same process) by the starter, which checks everything and replaces itself by
//! `bwrap` in turn. No profile exists for `/usr/bin/bwrap` itself.
//!
//! The call is exactly one of three:
//!
//! - `werkbank-einschluss selbsttest`: a fixed enclosure with `/usr/bin/true`, no input. Exit 0 when it ran.
//! - `werkbank-einschluss start --beschreibung-fd 3`: the description arrives on descriptor 3 and nowhere else (no
//!   environment variable, no file, no free argument).
//! - `werkbank-einschluss vorentpacken --beschreibung-fd 3`: the small enclosure that extracts the package of the cloud
//!   harness ([`crate::Vorentpacken`], its text form on descriptor 3 by the same rules); replaces itself by `bwrap`
//!   around `<program> --version`, so the exit code is the one of the program.
//!
//! Every other call is refused with exit 2.
//!
//! `start` needs a pipe on descriptor 3 (a file or a `memfd` is refused: the descriptor stays open in the harness, see
//! [`Eingabe::KeinRohr`]). It reads the descriptor to its end (at most [`EINGABE_MAX`] bytes), reads the strict JSON into a
//! [`Beschreibung`] (see [`crate::eingabe`]), runs every check of this crate (form, deny lists, overlaps, the profile as
//! text, the host, the test run) and only then replaces itself by `/usr/bin/bwrap` ([`BWRAP_PFAD`], a constant: never
//! a search path, never taken from the input or the environment). The environment of `bwrap` is empty; the harness
//! gets its variables from the allow list of the profile.
//!
//! `start` runs the session below the anchor ([`Profil::argumente_gebunden`]): the caller of the starter (its parent)
//! puts the read end of the lifeline on descriptor [`crate::LEINE_FD`] and holds the only write end as long as it lives.
//! The starter passes the descriptor on to `bwrap` and does not look at it; without it the anchor does not start the
//! program (exit [`crate::ANKER_ABBRUCH`]). `selbsttest` and `vorentpacken` have no anchor.
//!
//! A failure writes the line `<code>: <reason>` on standard error (the fixed texts of [`Fehler::code`] and
//! [`Fehler::grund`], never a path or a file content) and ends with the exit code of the table at [`Abbruch::exitcode`].
//! When the code is `lauf.einschluss_fehlt`, a second fixed line follows: `lauf.einschluss_teil: <exit code> <word>`
//! ([`Abbruch::teil_zeile`]), which names the check that refused. After the `exec` the starter is the only process that
//! knows its own exit code, so it is the one that writes the line.

use std::io::{Read, Write};
#[cfg(unix)]
use std::os::unix::fs::FileTypeExt;
use std::path::Path;
use std::process::{Command, ExitCode};
use std::time::Duration;

use crate::beschreibung::{Beschreibung, System};
use crate::eingabe::{EINGABE_MAX, beschreibung_lesen};
use crate::fehler::Fehler;
use crate::pfad::normal;
#[cfg(any(unix, test))]
use crate::profil::BWRAP_PFAD;
use crate::profil::Profil;

/// Where the starter stands on a system with the package. The profile of `AppArmor` is attached to this path.
pub const STARTER_PFAD: &str = "/usr/lib/werkbank/werkbank-einschluss";

/// The descriptor of the description, as the text of the argument.
pub const EINGABE_FD: &str = "3";

/// The only two calls of the starter, as lists of arguments (without the program name).
const SELBSTTEST: [&str; 1] = ["selbsttest"];
const START: [&str; 3] = ["start", "--beschreibung-fd", EINGABE_FD];
const VORENTPACKEN: [&str; 3] = ["vorentpacken", "--beschreibung-fd", EINGABE_FD];

/// The line of a refused call (the only line that is not of the form `<code>: <reason>`).
const AUFRUF_ZEILE: &str = "werkbank-einschluss: Aufruf ungueltig";

/// What the arguments of the starter ask for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Aufruf {
    /// `selbsttest`.
    Selbsttest,
    /// `start --beschreibung-fd 3`.
    Start,
    /// `vorentpacken --beschreibung-fd 3`.
    Vorentpacken,
}

/// Why the input was refused (before any check of the description itself).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Eingabe {
    /// Descriptor 3 cannot be read (not open, or not a file or pipe).
    NichtLesbar,
    /// Descriptor 3 is not a pipe. A pipe read to its end holds nothing and can stay open in the harness (the starter
    /// cannot close a descriptor in safe code, and `bwrap` passes it on); a file or a `memfd` would still hold the
    /// text there.
    KeinRohr,
    /// More than [`EINGABE_MAX`] bytes.
    ZuGross,
    /// Not JSON, an unknown key, a missing field or a wrong type.
    Ungueltig,
}

/// The reasons a call of the starter ends without the replacement by `bwrap`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Abbruch {
    /// The arguments are not one of the two calls.
    Aufruf,
    /// The input was refused.
    Eingabe(Eingabe),
    /// A check of the crate failed.
    Fehler(Fehler),
    /// `bwrap` could not be replaced into (the call of `exec` returned).
    Exec,
}

impl Abbruch {
    /// The exit code. The table: 2 call refused; 3 input refused; 4 working folder missing; 5 the description is
    /// invalid, forbidden or overlaps, or a source is unusable; 6 the host cannot (no platform, `bwrap` missing or not
    /// runnable, terminal injection open); 7 the system refuses the user namespace (the profile of the starter is not
    /// loaded, or the kernel switch is off); 8 the test run failed; 9 the replacement by `bwrap` failed.
    #[must_use]
    pub fn exitcode(&self) -> u8 {
        match self {
            Abbruch::Aufruf => 2,
            Abbruch::Eingabe(_) => 3,
            Abbruch::Fehler(f) => match f {
                Fehler::ArbeitsordnerFehlt => 4,
                Fehler::BeschreibungUngueltig(_)
                | Fehler::Verboten(_)
                | Fehler::Ueberlappt(_)
                | Fehler::QuelleUnbrauchbar(_) => 5,
                Fehler::NichtVerfuegbar
                | Fehler::BwrapFehlt
                | Fehler::BwrapNichtAusfuehrbar
                | Fehler::TiocstiOffen => 6,
                Fehler::NamensraumGesperrt => 7,
                Fehler::Probelauf { .. } => 8,
            },
            Abbruch::Exec => 9,
        }
    }

    /// The fixed word of the second line, for every refusal with the code `lauf.einschluss_fehlt`: the check that refused
    /// (`ueberlappt`, `verboten`, `bwrap-fehlt`, `namensraum-gesperrt`, `exec-gescheitert`, `eingabe-kein-rohr`, ...).
    fn teil(&self) -> Option<&'static str> {
        match self {
            Abbruch::Aufruf | Abbruch::Fehler(Fehler::ArbeitsordnerFehlt) => None,
            Abbruch::Eingabe(Eingabe::NichtLesbar) => Some("eingabe-nicht-lesbar"),
            Abbruch::Eingabe(Eingabe::KeinRohr) => Some("eingabe-kein-rohr"),
            Abbruch::Eingabe(Eingabe::ZuGross) => Some("eingabe-zu-gross"),
            Abbruch::Eingabe(Eingabe::Ungueltig) => Some("eingabe-ungueltig"),
            Abbruch::Fehler(f) => Some(f.teil()),
            Abbruch::Exec => Some("exec-gescheitert"),
        }
    }

    /// The second line on standard error, without the line end: `lauf.einschluss_teil: <exit code> <word>`. `None` for a
    /// refused call (exit 2) and for the missing working folder (exit 4, which has a code of its own).
    #[must_use]
    pub fn teil_zeile(&self) -> Option<String> {
        self.teil()
            .map(|w| format!("lauf.einschluss_teil: {} {w}", self.exitcode()))
    }

    /// The first line on standard error, without the line end.
    #[must_use]
    pub fn zeile(&self) -> String {
        match self {
            Abbruch::Aufruf => AUFRUF_ZEILE.to_owned(),
            Abbruch::Fehler(f) => format!("{}: {}", f.code(), f.grund()),
            Abbruch::Eingabe(_) | Abbruch::Exec => {
                let f = Fehler::NichtVerfuegbar;
                format!("{}: {}", f.code(), f.grund())
            }
        }
    }
}

/// The call of the arguments (without the program name), or `None` for any other.
#[must_use]
pub fn aufruf_lesen(args: &[String]) -> Option<Aufruf> {
    let gleich =
        |soll: &[&str]| args.len() == soll.len() && args.iter().zip(soll).all(|(a, s)| a == s);
    if gleich(&SELBSTTEST) {
        Some(Aufruf::Selbsttest)
    } else if gleich(&START) {
        Some(Aufruf::Start)
    } else if gleich(&VORENTPACKEN) {
        Some(Aufruf::Vorentpacken)
    } else {
        None
    }
}

/// Reads the input to its end. More than [`EINGABE_MAX`] bytes is a refusal; the reading stops one byte after the
/// limit and does not take the rest.
///
/// # Errors
/// [`Eingabe::NichtLesbar`] or [`Eingabe::ZuGross`].
pub fn eingabe_lesen(quelle: impl Read) -> Result<Vec<u8>, Eingabe> {
    let mut puffer = Vec::new();
    quelle
        .take(EINGABE_MAX as u64 + 1)
        .read_to_end(&mut puffer)
        .map_err(|_| Eingabe::NichtLesbar)?;
    if puffer.len() > EINGABE_MAX {
        return Err(Eingabe::ZuGross);
    }
    Ok(puffer)
}

/// How long the input may take. A pipe that its writer never closes ends the call instead of holding it.
pub const EINGABE_FRIST: Duration = Duration::from_secs(5);

/// [`eingabe_lesen`] with a time limit: the reading runs in a thread, and the call gives up after `frist`. The thread
/// ends with the process (the starter replaces itself or exits right after).
///
/// # Errors
/// [`Eingabe::NichtLesbar`] also when the time is up; [`Eingabe::ZuGross`].
pub fn eingabe_lesen_frist(
    quelle: impl Read + Send + 'static,
    frist: Duration,
) -> Result<Vec<u8>, Eingabe> {
    let (sender, empfaenger) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = sender.send(eingabe_lesen(quelle));
    });
    empfaenger
        .recv_timeout(frist)
        .map_err(|_| Eingabe::NichtLesbar)?
}

/// The program of the description is an absolute, normal path (no `..`, no `//`, no relative name).
fn programm_pruefen(b: &Beschreibung) -> Result<(), Fehler> {
    match b.argv.first() {
        Some(programm) if !normal(Path::new(programm)) => Err(Fehler::BeschreibungUngueltig(
            "argv[0] is not an absolute path",
        )),
        _ => Ok(()),
    }
}

/// From the text form to the checked profile: read, then every pure check of the crate. Nothing is run and no file is
/// read; the host part is given.
///
/// # Errors
/// [`Abbruch::Eingabe`] for a refused text, [`Abbruch::Fehler`] for a refused description.
pub fn profil_bauen(bytes: &[u8], system: System) -> Result<Profil, Abbruch> {
    let beschreibung =
        beschreibung_lesen(bytes, system).ok_or(Abbruch::Eingabe(Eingabe::Ungueltig))?;
    programm_pruefen(&beschreibung).map_err(Abbruch::Fehler)?;
    Profil::bauen(&beschreibung).map_err(Abbruch::Fehler)
}

/// The arguments of the self test: the fixed enclosure of the profile (own namespaces, `/usr` read-only, no network,
/// empty environment) around `/usr/bin/true`, with no input at all.
#[must_use]
pub fn selbsttest_argumente(system: &System) -> Vec<String> {
    let mut a: Vec<String> = [
        "--unshare-user",
        "--disable-userns",
        "--unshare-pid",
        "--unshare-net",
        "--unshare-ipc",
        "--unshare-uts",
        "--unshare-cgroup",
        "--die-with-parent",
        "--cap-drop",
        "ALL",
        "--clearenv",
        "--ro-bind",
        "/usr",
        "/usr",
    ]
    .map(str::to_owned)
    .to_vec();
    for (text, pfad) in &system.symlinks {
        a.extend(["--symlink".to_owned(), text.clone(), pfad.clone()]);
    }
    for (quelle, ziel) in &system.ro_binds {
        a.extend(["--ro-bind".to_owned(), quelle.clone(), ziel.clone()]);
    }
    a.extend(
        [
            "--proc",
            "/proc",
            "--dev",
            "/dev",
            "--tmpfs",
            "/tmp",
            "--",
            "/usr/bin/true",
        ]
        .map(str::to_owned),
    );
    a
}

/// The command that starts the starter with the fixed arguments of `vorentpacken` and an empty environment. The caller
/// puts the text form of [`crate::Vorentpacken`] on descriptor 3 and waits for the child.
#[must_use]
pub fn vorentpacken_befehl(starter: &Path) -> Command {
    let mut c = Command::new(starter);
    c.args(VORENTPACKEN).env_clear();
    c
}

/// The command that starts the starter with the fixed arguments of `start` and an empty environment. The caller puts
/// the text form on descriptor 3 and replaces its process by this command (`exec`), so the process stays the same.
#[must_use]
pub fn start_befehl(starter: &Path) -> Command {
    let mut c = Command::new(starter);
    c.args(START).env_clear();
    c
}

fn melden(abbruch: &Abbruch) -> ExitCode {
    // a closed standard error must not change the exit code
    let mut zeilen = abbruch.zeile();
    if let Some(teil) = abbruch.teil_zeile() {
        zeilen.push('\n');
        zeilen.push_str(&teil);
    }
    let _ = writeln!(std::io::stderr(), "{zeilen}");
    ExitCode::from(abbruch.exitcode())
}

/// The whole program: `args` are the arguments without the program name.
#[must_use]
pub fn starten(args: &[String]) -> ExitCode {
    let Some(aufruf) = aufruf_lesen(args) else {
        return melden(&Abbruch::Aufruf);
    };
    let ergebnis = match aufruf {
        Aufruf::Selbsttest => selbsttest().map(|()| ExitCode::SUCCESS),
        Aufruf::Start => start().map(|never| match never {}),
        Aufruf::Vorentpacken => vorentpacken().map(|never| match never {}),
    };
    ergebnis.unwrap_or_else(|a| melden(&a))
}

/// The self test on the real host.
fn selbsttest() -> Result<(), Abbruch> {
    let system = System::vom_host().map_err(Abbruch::Fehler)?;
    selbsttest_lauf(&system).map_err(Abbruch::Fehler)
}

// One body per function with a `cfg` block per system: cargo-mutants reads the text and does not evaluate `cfg`, so a
// function that exists only for another system would be changed without a build that could notice it.
fn selbsttest_lauf(system: &System) -> Result<(), Fehler> {
    #[cfg(unix)]
    {
        crate::vorpruefung::lauf_pruefen(
            Path::new(BWRAP_PFAD),
            &selbsttest_argumente(system),
            crate::vorpruefung::PROBE_FRIST,
        )
    }
    #[cfg(not(unix))]
    {
        let _ = system;
        Err(Fehler::NichtVerfuegbar)
    }
}

/// Descriptor 3 read to its end: a pipe only, within the limit and the time.
fn eingabe_holen() -> Result<Vec<u8>, Abbruch> {
    let ort = Path::new("/dev/fd/3");
    let meta = std::fs::metadata(ort).map_err(|_| Abbruch::Eingabe(Eingabe::NichtLesbar))?;
    if !is_rohr(&meta) {
        return Err(Abbruch::Eingabe(Eingabe::KeinRohr));
    }
    let datei = std::fs::File::open(ort).map_err(|_| Abbruch::Eingabe(Eingabe::NichtLesbar))?;
    eingabe_lesen_frist(datei, EINGABE_FRIST).map_err(Abbruch::Eingabe)
}

/// `start`: read, check, replace. Returns only a refusal.
fn start() -> Result<std::convert::Infallible, Abbruch> {
    let system = System::vom_host().map_err(Abbruch::Fehler)?;
    let bytes = eingabe_holen()?;
    let profil = profil_bauen(&bytes, system)?;
    profil.vorpruefen().map_err(Abbruch::Fehler)?;
    ersetzen(&profil)
}

/// From the text form of the small enclosure to the checked arguments of `bwrap`: read, then every pure check.
///
/// # Errors
/// [`Abbruch::Eingabe`] for a refused text, [`Abbruch::Fehler`] for a refused description.
pub fn vorentpacken_bauen(
    bytes: &[u8],
    system: System,
) -> Result<(crate::Vorentpacken, Vec<String>), Abbruch> {
    let v = crate::vorentpacken_lesen(bytes, system).ok_or(Abbruch::Eingabe(Eingabe::Ungueltig))?;
    let args = v.argumente().map_err(Abbruch::Fehler)?;
    Ok((v, args))
}

/// `vorentpacken`: read, check, replace by `bwrap` around `<program> --version`. Returns only a refusal.
fn vorentpacken() -> Result<std::convert::Infallible, Abbruch> {
    let system = System::vom_host().map_err(Abbruch::Fehler)?;
    let bytes = eingabe_holen()?;
    let (v, args) = vorentpacken_bauen(&bytes, system)?;
    vorentpacken_ersetzen(&v, &args)
}

fn vorentpacken_ersetzen(
    v: &crate::Vorentpacken,
    args: &[String],
) -> Result<std::convert::Infallible, Abbruch> {
    #[cfg(unix)]
    let fehler = {
        use std::os::unix::process::CommandExt;
        crate::vorentpacken_pruefen(v, Path::new(BWRAP_PFAD)).map_err(Abbruch::Fehler)?;
        let _fehler = Command::new(BWRAP_PFAD).args(args).env_clear().exec();
        Abbruch::Exec
    };
    #[cfg(not(unix))]
    let fehler = {
        let _ = (v, args);
        Abbruch::Fehler(Fehler::NichtVerfuegbar)
    };
    Err(fehler)
}

/// The descriptor is a pipe (a FIFO). Without Unix there is none.
fn is_rohr(meta: &std::fs::Metadata) -> bool {
    #[cfg(unix)]
    {
        meta.file_type().is_fifo()
    }
    #[cfg(not(unix))]
    {
        let _ = meta;
        false
    }
}

fn ersetzen(profil: &Profil) -> Result<std::convert::Infallible, Abbruch> {
    #[cfg(unix)]
    let fehler = {
        use std::os::unix::process::CommandExt;
        // `exec` returns only when it failed
        let _fehler = profil.befehl().exec();
        Abbruch::Exec
    };
    #[cfg(not(unix))]
    let fehler = {
        let _ = profil;
        Abbruch::Fehler(Fehler::NichtVerfuegbar)
    };
    Err(fehler)
}

#[cfg(test)]
mod tests;
