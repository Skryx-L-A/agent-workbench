//! The hand-over to the enclosure (step 8): the delivered build writes the description into a pipe of its own, puts
//! the read end on descriptor 3 and starts the starter as its child through the trampoline (`werkbank-lauf-kind <ppid>
//! --halten`), then stays its parent until it ends ([`halten`]). Without `unsafe`: descriptor 3 is reserved at the very
//! start by opening `/dev/null` while 3 is the lowest free number, and `dup2` puts the read end over it (which clears
//! close-on-exec on 3 only). Every other descriptor of this process carries close-on-exec.
//!
//! Why a parent and no `exec` (contract 1, "Lebensdauer"): `bwrap` sets the death signal of its inner process only after
//! the set-up of the namespaces (bubblewrap 0.12.0, `do_init`), so a SIGKILL of the outer `bwrap` in its first
//! milliseconds left the inner one running below PID 1 (measured 21.11.2026). As the parent and child subreaper this
//! process adopts such an orphan and ends it. When this process itself dies during the set-up, the anchor in the
//! enclosure does not start the program: it holds the read end of the lifeline on descriptor 5 (reserved at the start
//! with 3, [`Platz::reservieren`]), and this process holds the only write end until it ends
//! (`Platz::leine_legen`, Linux only; [`einschluss::ANKER_SKRIPT`]).
//!
//! The test build (feature `pruefbau`) builds the profile itself, checks the host and calls `bwrap` directly, through
//! the same parent.

use std::os::fd::{AsRawFd, OwnedFd};
use std::path::Path;

use crate::fehler::Fehler;
use crate::meldung::Meldung;

/// The descriptor of the description (contract of the starter).
pub const BESCHREIBUNG_FD: i32 = 3;

/// [`einschluss::LEINE_FD`] as a count: the reservation opens at most this many fillers below it.
const LEINE_FD_ZAHL: usize = einschluss::LEINE_FD.unsigned_abs() as usize;

/// The reserved descriptor 3, if it was the lowest free number at the start, and the reserved descriptor 5 of the
/// lifeline ([`Platz::leine_reservieren`]).
#[derive(Debug)]
pub struct Platz(Option<OwnedFd>, Option<OwnedFd>);

impl Platz {
    /// Opens `/dev/null` (close-on-exec) and keeps it only when it got the number 3, then reserves descriptor 5 of the
    /// lifeline ([`Platz::leine_reservieren`]). Number 4 stays free for the report, reserved right after.
    #[must_use]
    pub fn reservieren() -> Platz {
        let fd = rustix::fs::open(
            "/dev/null",
            rustix::fs::OFlags::RDONLY | rustix::fs::OFlags::CLOEXEC,
            rustix::fs::Mode::empty(),
        )
        .ok();
        let mut platz = Platz(fd.filter(|f| f.as_raw_fd() == BESCHREIBUNG_FD), None);
        platz.leine_reservieren();
        platz
    }

    /// Reserves descriptor 5 for the lifeline ([`einschluss::LEINE_FD`]): opens `/dev/null` (close-on-exec) until it gets
    /// the number 5 and keeps that one; the lower numbers it got on the way (4, and 3 when it was not the place) are
    /// closed again. Part of [`Platz::reservieren`], before the first other file.
    fn leine_reservieren(&mut self) {
        let mut fueller = Vec::new();
        while fueller.len() <= LEINE_FD_ZAHL {
            // read-only is the flag 0, so close-on-exec alone opens it read-only
            let Ok(fd) = rustix::fs::open(
                "/dev/null",
                rustix::fs::OFlags::CLOEXEC,
                rustix::fs::Mode::empty(),
            ) else {
                break;
            };
            match fd.as_raw_fd().cmp(&einschluss::LEINE_FD) {
                std::cmp::Ordering::Less => fueller.push(fd),
                std::cmp::Ordering::Equal => {
                    self.1 = Some(fd);
                    break;
                }
                std::cmp::Ordering::Greater => break,
            }
        }
    }

    /// Whether descriptor 5 is held for the lifeline.
    #[must_use]
    pub fn leine_da(&self) -> bool {
        self.1.is_some()
    }

    /// The descriptor of the lifeline this process holds (step 6 does not count it as inherited).
    #[must_use]
    pub fn eigene(&self) -> Vec<i32> {
        self.1.iter().map(AsRawFd::as_raw_fd).collect()
    }

    /// Makes the lifeline right before the start of the trampoline: a new pipe, its read end laid over descriptor 5
    /// without close-on-exec (the trampoline, the starter and `bwrap` pass it on to the anchor in the enclosure), its
    /// write end kept with close-on-exec. The caller holds the write end until it ends: the anchor starts the program
    /// only while it is open (see [`einschluss::ANKER_SKRIPT`]).
    ///
    /// # Errors
    /// `lauf.einschluss_fehlt` when descriptor 5 is not held, the pipe cannot be made, or 5 is not the read end without
    /// close-on-exec afterwards.
    #[cfg(target_os = "linux")]
    pub fn leine_legen(&mut self) -> Result<OwnedFd, Fehler> {
        let ziel = self.1.as_mut().ok_or(Fehler::EinschlussFehlt)?;
        let (lesen, schreiben) = rustix::pipe::pipe_with(rustix::pipe::PipeFlags::CLOEXEC)
            .map_err(|_| Fehler::EinschlussFehlt)?;
        rustix::io::dup2(&lesen, ziel).map_err(|_| Fehler::EinschlussFehlt)?;
        drop(lesen);
        if ziel.as_raw_fd() != einschluss::LEINE_FD {
            return Err(Fehler::EinschlussFehlt);
        }
        let flags = rustix::io::fcntl_getfd(&*ziel).map_err(|_| Fehler::EinschlussFehlt)?;
        if flags.contains(rustix::io::FdFlags::CLOEXEC) {
            return Err(Fehler::EinschlussFehlt);
        }
        Ok(schreiben)
    }

    /// Whether descriptor 3 is held.
    #[must_use]
    pub fn da(&self) -> bool {
        self.0.is_some()
    }
}

/// Whether descriptor 3 is open in this process (Linux: `/proc/self/fd`). It is when the caller handed one down
/// without close-on-exec, or when [`Platz`] holds it.
#[must_use]
pub fn drei_belegt() -> bool {
    std::fs::symlink_metadata("/proc/self/fd/3").is_ok()
}

/// Step 6 on descriptor 3: the delivered build needs the reserved place. The test build (`pruefbau`) needs none, but
/// refuses a descriptor 3 that was handed down, since `bwrap` would pass it into the enclosure.
#[must_use]
pub fn platz_gut(pruefbau: bool, da: bool, drei_belegt: bool) -> bool {
    da || (pruefbau && !drei_belegt)
}

/// The descriptors this process holds besides 0, 1, 2 and 3 (Linux: `/proc/self/fd`, without the one of the listing
/// itself). At step 6 the program holds no file of its own, so every number here was inherited without close-on-exec
/// and would reach the enclosure. `None` where the list cannot be read.
#[must_use]
pub fn fremde_deskriptoren() -> Option<Vec<i32>> {
    let mut fremd = Vec::new();
    let mut eigene_liste = false;
    for e in std::fs::read_dir("/proc/self/fd").ok()? {
        let e = e.ok()?;
        let n = e.file_name().to_str()?.parse::<i32>().ok()?;
        if n <= BESCHREIBUNG_FD {
            continue;
        }
        // the listing holds one descriptor on its own folder
        let ziel = std::fs::read_link(e.path()).ok();
        if !eigene_liste
            && ziel
                .as_deref()
                .is_some_and(|z| z.ends_with("fd") && z.starts_with("/proc"))
        {
            eigene_liste = true;
            continue;
        }
        fremd.push(n);
    }
    Some(fremd)
}

/// Puts `text` into a new pipe, closes the write end and lays the read end over descriptor 3.
///
/// # Errors
/// `lauf.einschluss_fehlt` when the pipe cannot be made or written, or descriptor 3 is not the read end afterwards.
#[cfg(target_os = "linux")]
pub fn auf_drei_legen(platz: &mut Platz, text: &[u8]) -> Result<(), Fehler> {
    use std::io::Write;
    // a longer description would block `write_all` (nobody reads yet) and the start would hang with running children;
    // the starter refuses it anyway
    if text.len() > einschluss::EINGABE_MAX {
        return Err(Fehler::EinschlussFehlt);
    }
    let ziel = platz.0.as_mut().ok_or(Fehler::EinschlussFehlt)?;
    let (lesen, schreiben) = rustix::pipe::pipe_with(rustix::pipe::PipeFlags::CLOEXEC)
        .map_err(|_| Fehler::EinschlussFehlt)?;
    // the description is at most `EINGABE_MAX` = 65536 bytes, the capacity of a pipe on Linux: the write does not block
    let mut s = std::fs::File::from(schreiben);
    s.write_all(text).map_err(|_| Fehler::EinschlussFehlt)?;
    drop(s);
    rustix::io::dup2(&lesen, ziel).map_err(|_| Fehler::EinschlussFehlt)?;
    drop(lesen);
    if ziel.as_raw_fd() != BESCHREIBUNG_FD {
        return Err(Fehler::EinschlussFehlt);
    }
    let flags = rustix::io::fcntl_getfd(&*ziel).map_err(|_| Fehler::EinschlussFehlt)?;
    if flags.contains(rustix::io::FdFlags::CLOEXEC) {
        return Err(Fehler::EinschlussFehlt);
    }
    Ok(())
}

#[cfg(not(target_os = "linux"))]
/// Not available: the hand-over exists on Linux only.
///
/// # Errors
/// Always `lauf.einschluss_fehlt`.
pub fn auf_drei_legen(_platz: &mut Platz, _text: &[u8]) -> Result<(), Fehler> {
    Err(Fehler::EinschlussFehlt)
}

/// Step 8 of the delivered build: starts the starter as the child of this process and stays its parent
/// ([`halten`]). Returns only when the start failed; otherwise the process ends with the end of the starter.
#[must_use]
pub fn starter_halten(
    kind: &Path,
    starter: &Path,
    platz: &mut Platz,
    meldung: &mut Meldung,
    beschreibung: &einschluss::Beschreibung,
    bekannt: &[u32],
) -> Fehler {
    let text = einschluss::beschreibung_schreiben(beschreibung);
    if let Err(f) = auf_drei_legen(platz, text.as_bytes()) {
        return f;
    }
    halten(
        kind,
        &einschluss::starter::start_befehl(starter),
        platz,
        meldung,
        bekannt,
    )
}

/// Step 8 of the test build: builds the profile, checks the host and starts `bwrap` as the child of this process
/// ([`halten`]).
#[cfg(feature = "pruefbau")]
#[must_use]
pub fn bwrap_halten(
    kind: &Path,
    bwrap: Option<&Path>,
    platz: &mut Platz,
    meldung: &mut Meldung,
    beschreibung: &einschluss::Beschreibung,
    bekannt: &[u32],
) -> Fehler {
    let Ok(mut profil) = einschluss::Profil::bauen(beschreibung) else {
        return Fehler::EinschlussFehlt;
    };
    if let Some(b) = bwrap {
        profil = profil.mit_bwrap(b.to_path_buf());
    }
    match profil.vorpruefen() {
        Ok(()) => {}
        Err(einschluss::Fehler::ArbeitsordnerFehlt) => return Fehler::OrdnerFehlt,
        Err(_) => return Fehler::EinschlussFehlt,
    }
    halten(kind, &profil.befehl(), platz, meldung, bekannt)
}

/// The signals this process passes on to the starter while it runs (the core service ends a session with SIGHUP and
/// SIGTERM to this PID).
pub const WEITERREICHEN: [i32; 2] = [signal_hook::consts::SIGHUP, signal_hook::consts::SIGTERM];

/// The signals this process takes and does not pass on: the terminal sends them to the whole foreground group, so the
/// starter has them already; passed on, the harness would get each twice.
pub const AUFFANGEN: [i32; 2] = [signal_hook::consts::SIGINT, signal_hook::consts::SIGQUIT];

/// Whether a signal of this process goes on to the starter.
#[must_use]
pub fn weiterreichen(signal: i32) -> bool {
    WEITERREICHEN.contains(&signal)
}

/// How the starter ended: with a code, or by a signal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ausgang {
    /// The exit code of the starter.
    Code(i32),
    /// The number of the signal that ended the starter.
    Signal(i32),
}

/// The end of the starter from its wait status.
#[cfg(unix)]
#[must_use]
pub fn ausgang(status: std::process::ExitStatus) -> Ausgang {
    use std::os::unix::process::ExitStatusExt;
    match (status.code(), status.signal()) {
        (Some(c), _) => Ausgang::Code(c),
        (None, Some(s)) => Ausgang::Signal(s),
        (None, None) => Ausgang::Code(i32::from(Fehler::Intern.exitcode())),
    }
}

/// The children in `kinder` that are not in `bekannt`: the starter and what this process adopted below it. The search
/// process, the model path and the watcher stay; the watcher ends the first two and removes the folders after this
/// process has ended (contract 1, "Lebensdauer").
#[must_use]
pub fn fremde(kinder: &[u32], bekannt: &[u32]) -> Vec<u32> {
    kinder
        .iter()
        .copied()
        .filter(|k| !bekannt.contains(k))
        .collect()
}

/// The children of this process (Linux: `/proc/self/task/<tid>/children` of every thread). Empty where it cannot be read.
#[must_use]
pub fn eigene_kinder() -> Vec<u32> {
    let Ok(faeden) = std::fs::read_dir("/proc/self/task") else {
        return Vec::new();
    };
    let mut kinder = Vec::new();
    for f in faeden.filter_map(Result::ok) {
        if let Ok(text) = std::fs::read_to_string(f.path().join("children")) {
            kinder.extend(
                text.split_whitespace()
                    .filter_map(|w| w.parse::<u32>().ok()),
            );
        }
    }
    kinder
}

/// The parent of the starter: child subreaper, signals taken before the start ([`WEITERREICHEN`], [`AUFFANGEN`]), the
/// starter started through the trampoline `kind` (which sets `PR_SET_PDEATHSIG = SIGKILL` for it and compares
/// `getppid()`, without `setsid`: the starter keeps the terminal and the foreground group). Waits for the starter,
/// ends every other child that is not in `bekannt` (adopted below the starter) by SIGKILL through a pidfd and waits for
/// it, then ends with the code of the starter or by its signal ([`enden`]). Returns only when the start failed.
///
/// With a report ([`Meldung`]) the connection lies on descriptor 4 from right before the start of the trampoline
/// (`--halten-melden`) until right after it; in between this process starts no other child, and no other thread runs
/// (the thread of the signals starts after the close).
#[cfg(target_os = "linux")]
#[must_use]
pub fn halten(
    kind: &Path,
    befehl: &std::process::Command,
    platz: &mut Platz,
    meldung: &mut Meldung,
    bekannt: &[u32],
) -> Fehler {
    use rustix::process::{PidfdFlags, Signal};
    if rustix::process::set_child_subreaper(Some(rustix::process::getpid())).is_err() {
        return Fehler::EinschlussFehlt;
    }
    let Ok(mut signale) =
        signal_hook::iterator::Signals::new(WEITERREICHEN.iter().chain(&AUFFANGEN))
    else {
        return Fehler::Intern;
    };
    // the write end of the lifeline stays in this process until it ends (close-on-exec: no child gets a copy)
    let _leine = match platz.leine_legen() {
        Ok(l) => l,
        Err(f) => return f,
    };
    let melden = match meldung.auf_vier_legen() {
        Ok(m) => m,
        Err(f) => return f,
    };
    let mut c = std::process::Command::new(kind);
    c.arg(std::process::id().to_string())
        .arg(if melden {
            crate::kind::HALTEN_MELDEN
        } else {
            crate::kind::HALTEN
        })
        .arg(befehl.get_program())
        .args(befehl.get_args())
        .env_clear();
    let gestartet = c.spawn();
    // descriptor 4 is the trampoline's now (or nobody's): this process closes its copy before anything else
    meldung.vier_schliessen();
    let Ok(mut starter) = gestartet else {
        return Fehler::HarnessFehlt;
    };
    // the trampoline holds the connection until its `exec`; the end of the last copy tells the service the starter runs
    meldung.schliessen();
    // the description and the read end of the lifeline are in the starter now; this process needs neither any more
    platz.0 = None;
    platz.1 = None;
    let pidfd = i32::try_from(starter.id())
        .ok()
        .and_then(rustix::process::Pid::from_raw)
        .and_then(|p| rustix::process::pidfd_open(p, PidfdFlags::empty()).ok())
        .map(std::sync::Arc::new);
    let Some(fd) = pidfd else {
        // without a pidfd no signal could reach the starter safely: it ends at once (fail closed)
        let _ = starter.kill();
        let _ = starter.wait();
        fremde_beenden(bekannt);
        enden(Ausgang::Code(i32::from(Fehler::Intern.exitcode())));
    };
    // the starter was forked from the main thread, which lives until the end: its death signal stays bound
    let weiter = std::thread::Builder::new().spawn(move || {
        for s in signale.forever() {
            if weiterreichen(s)
                && let Some(sig) = Signal::from_named_raw(s)
            {
                let _ = rustix::process::pidfd_send_signal(&*fd, sig);
            }
        }
    });
    if weiter.is_err() {
        let _ = starter.kill();
    }
    let status = starter.wait();
    fremde_beenden(bekannt);
    match status {
        Ok(s) => enden(ausgang(s)),
        Err(_) => enden(Ausgang::Code(i32::from(Fehler::Intern.exitcode()))),
    }
}

/// Not available: the parent exists on Linux only.
#[cfg(not(target_os = "linux"))]
#[must_use]
pub fn halten(
    _kind: &Path,
    _befehl: &std::process::Command,
    _platz: &mut Platz,
    _meldung: &mut Meldung,
    _bekannt: &[u32],
) -> Fehler {
    Fehler::EinschlussFehlt
}

/// How long [`fremde_beenden`] keeps looking for adopted children.
pub const FREMDE_FRIST: std::time::Duration = std::time::Duration::from_secs(5);

/// Ends every child of this process that is not in `bekannt` by SIGKILL through a pidfd and waits for it, until none is
/// left or [`FREMDE_FRIST`] has passed. A child is waited for by its pidfd, so no other number is ever touched: an
/// unwaited child keeps its number.
#[cfg(target_os = "linux")]
fn fremde_beenden(bekannt: &[u32]) {
    use rustix::process::{PidfdFlags, Signal, WaitId, WaitIdOptions};
    use std::os::fd::AsFd;
    let ende = std::time::Instant::now() + FREMDE_FRIST;
    loop {
        let liste = fremde(&eigene_kinder(), bekannt);
        if liste.is_empty() || std::time::Instant::now() >= ende {
            return;
        }
        for k in liste {
            let Some(fd) = i32::try_from(k)
                .ok()
                .and_then(rustix::process::Pid::from_raw)
                .and_then(|p| rustix::process::pidfd_open(p, PidfdFlags::empty()).ok())
            else {
                continue;
            };
            let _ = rustix::process::pidfd_send_signal(&fd, Signal::KILL);
            let _ = rustix::process::waitid(WaitId::PidFd(fd.as_fd()), WaitIdOptions::EXITED);
        }
    }
}

/// Ends this process like the starter ended: with its code, or by the same signal (default action), so that the
/// carrier counts `128 + n` as before. A signal whose default action cannot be called up here (SIGILL, SIGFPE, SIGSEGV)
/// or that does not end the process gives the code `128 + n`.
#[cfg(unix)]
pub fn enden(a: Ausgang) -> ! {
    match a {
        Ausgang::Code(c) => std::process::exit(c),
        Ausgang::Signal(s) => {
            if s == signal_hook::consts::SIGKILL || !signal_hook::consts::FORBIDDEN.contains(&s) {
                let _ = signal_hook::low_level::emulate_default_handler(s);
            }
            std::process::exit(128 + s)
        }
    }
}

#[cfg(test)]
mod tests;
