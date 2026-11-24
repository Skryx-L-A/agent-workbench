//! The children outside the enclosure (contract 1, "Lebensdauer des Such-Prozesses"): the search process and the model
//! path. Each starts through the trampoline `werkbank-lauf-kind`, which sets `PR_SET_PDEATHSIG = SIGTERM`, compares
//! `getppid()` with the PID of "Lauf starten", calls `setsid` and replaces itself by the program. The signal hangs on
//! the thread that forked; "Lauf starten" forks from its main thread and is single threaded at the fork (checked).
//!
//! The trampoline is a program of its own because the workspace forbids `unsafe`, and with it `pre_exec`.
//!
//! The signal is not enough on its own: when the process of "Lauf starten" has become the starter of the enclosure and
//! its `AppArmor` profile has no `signal` rule, the kernel does not deliver the death signal to the children (measured
//! on Ubuntu 26.04, audit `operation="signal" ... peer="unconfined"`). The second line is the watcher
//! (`werkbank-lauf-kind --wache`, [`wache`]): an own process outside every profile that ends the children and removes
//! the session folder of the harness and the socket folder of the session when the process of "Lauf starten" ends,
//! whatever the reason. The files next to the start file stay: they belong to the core service, which removes them at
//! the end of the session (contract 1, "Dateien der Sitzung").

use std::io::Read;
use std::os::fd::AsFd;
use std::path::Path;
use std::process::{Child, ChildStdout, Command, Stdio};
use std::time::{Duration, Instant};

/// The ready line of both children (contract 4 and 8).
pub const BEREIT: &[u8] = b"{\"bereit\":true}\n";
/// How long a child may take for its ready line.
pub const BEREIT_FRIST: Duration = Duration::from_secs(10);

/// Exit code of the trampoline when the program cannot be started (the shell convention for "not found").
pub const KIND_START_FEHLER: i32 = 127;
/// Exit code of the trampoline for a wrong call, a parent that is gone, or a refused `prctl`/`setsid`.
pub const KIND_AUFRUF_FEHLER: i32 = 125;

/// Why a child did not become ready.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ausfall {
    /// The start failed: the trampoline or the program could not be run.
    Fehlt,
    /// The child ran but gave no ready line in time, another line, or ended before it.
    NichtBereit,
}

/// A running child.
#[derive(Debug)]
pub struct Kind {
    kind: Child,
    /// The read end of its standard output; kept open until the hand-over to the starter ([`Kind::ausgabe_schliessen`]).
    aus: Option<ChildStdout>,
}

impl Kind {
    /// The PID (the same after the `exec` of the trampoline).
    #[must_use]
    pub fn pid(&self) -> u32 {
        self.kind.id()
    }

    /// Closes the read end of its standard output, as the `exec` did before: the child is not held by a full pipe.
    pub fn ausgabe_schliessen(&mut self) {
        drop(self.aus.take());
    }

    /// Ends the child with SIGTERM and waits for it (at most 2 s, then SIGKILL).
    pub fn beenden(mut self) {
        drop(self.aus.take());
        if let Some(pid) = i32::try_from(self.kind.id())
            .ok()
            .and_then(rustix::process::Pid::from_raw)
        {
            let _ = rustix::process::kill_process(pid, rustix::process::Signal::TERM);
        }
        let ende = Instant::now() + Duration::from_secs(2);
        while Instant::now() < ende {
            if matches!(self.kind.try_wait(), Ok(Some(_))) {
                return;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        let _ = self.kind.kill();
        let _ = self.kind.wait();
    }
}

/// First argument of the trampoline that makes it the watcher of a session.
pub const WACHE: &str = "--wache";

/// How long the watcher waits for the children after SIGTERM before it sends SIGKILL.
pub const WACHE_GNADE: Duration = Duration::from_secs(2);

/// Starts the watcher: `werkbank-lauf-kind --wache <pid of this process> <socket folder> <session folder>`, with the
/// pidfds of the two children on its standard input and standard output (a pidfd names the process, never a reused
/// number). It runs in a session of its own and ends with both folders removed once this process has ended. The session
/// folder must exist and be made by this run: the watcher removes it whatever it finds there at the end. Returns the PID
/// of the watcher (the parent of the starter leaves it alone, [`crate::uebergabe::halten`]).
///
/// # Errors
/// A pidfd cannot be opened or the watcher does not start.
#[cfg(target_os = "linux")]
pub fn wache_starten(
    kind: &Path,
    kinder: [&Kind; 2],
    ordner: &Path,
    sitzung: &Path,
) -> std::io::Result<u32> {
    let pidfd = |k: &Kind| -> std::io::Result<std::os::fd::OwnedFd> {
        let pid = i32::try_from(k.pid())
            .ok()
            .and_then(rustix::process::Pid::from_raw)
            .ok_or(std::io::ErrorKind::InvalidInput)?;
        rustix::process::pidfd_open(pid, rustix::process::PidfdFlags::empty())
            .map_err(std::io::Error::from)
    };
    let (a, b) = (pidfd(kinder[0])?, pidfd(kinder[1])?);
    Command::new(kind)
        .arg(WACHE)
        .arg(std::process::id().to_string())
        .arg(ordner)
        .arg(sitzung)
        .env_clear()
        .stdin(Stdio::from(a))
        .stdout(Stdio::from(b))
        .stderr(Stdio::null())
        .spawn()
        .map(|w| w.id())
}

/// Not available: the watcher exists on Linux only.
///
/// # Errors
/// Always.
#[cfg(not(target_os = "linux"))]
pub fn wache_starten(
    _kind: &Path,
    _kinder: [&Kind; 2],
    _ordner: &Path,
    _sitzung: &Path,
) -> std::io::Result<u32> {
    Err(std::io::ErrorKind::Unsupported.into())
}

/// The number of threads of this process (Linux: `/proc/self/status`). `None` where it cannot be read.
#[must_use]
pub fn faeden() -> Option<usize> {
    let text = std::fs::read_to_string("/proc/self/status").ok()?;
    text.lines()
        .find_map(|z| z.strip_prefix("Threads:"))
        .and_then(|w| w.trim().parse().ok())
}

/// The fork condition of contract 1: the process is single threaded. Where the count cannot be read (no `/proc`), the
/// program has no thread of its own either; it never starts one before the `exec`.
#[must_use]
pub fn einthreadig() -> bool {
    faeden().is_none_or(|n| n == 1)
}

/// Starts `programm` with `args` through the trampoline `kind`: empty environment, standard input `/dev/null`,
/// standard error discarded, standard output read for the ready line. Waits at most `frist` for exactly [`BEREIT`].
///
/// # Errors
/// [`Ausfall::Fehlt`] when the trampoline or the program does not start, [`Ausfall::NichtBereit`] otherwise. A child
/// that started is ended before the error returns.
pub fn starten(
    kind: &Path,
    programm: &Path,
    args: &[String],
    frist: Duration,
) -> Result<Kind, Ausfall> {
    let mut c = Command::new(kind);
    c.arg(std::process::id().to_string())
        .arg("--")
        .arg(programm)
        .args(args)
        .env_clear()
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    let mut kind = c.spawn().map_err(|_| Ausfall::Fehlt)?;
    let Some(mut aus) = kind.stdout.take() else {
        let _ = kind.kill();
        let _ = kind.wait();
        return Err(Ausfall::Fehlt);
    };
    let mut k = Kind { kind, aus: None };
    match bereit_lesen(&mut aus, frist) {
        Lesen::Bereit => {
            k.aus = Some(aus);
            Ok(k)
        }
        Lesen::Anders => {
            k.beenden();
            Err(Ausfall::NichtBereit)
        }
        Lesen::Ende => {
            let ausfall = match ende_abwarten(&mut k.kind) {
                Some(KIND_START_FEHLER | KIND_AUFRUF_FEHLER) => Ausfall::Fehlt,
                _ => Ausfall::NichtBereit,
            };
            k.beenden();
            Err(ausfall)
        }
    }
}

/// The exit code of a child whose output ended, if it ends within 1 s.
fn ende_abwarten(kind: &mut Child) -> Option<i32> {
    let ende = Instant::now() + Duration::from_secs(1);
    while Instant::now() < ende {
        if let Ok(Some(status)) = kind.try_wait() {
            return status.code();
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    None
}

enum Lesen {
    Bereit,
    Anders,
    Ende,
}

/// Reads up to the first LF within `frist`, without a thread: `poll` on the pipe. The verdict rests on the first line
/// alone (at most [`BEREIT`] long); nothing is read after it.
fn bereit_lesen(aus: &mut ChildStdout, frist: Duration) -> Lesen {
    let ende = Instant::now() + frist;
    let mut zeile: Vec<u8> = Vec::new();
    loop {
        let rest = ende.saturating_duration_since(Instant::now());
        if rest.is_zero() {
            return Lesen::Anders;
        }
        let zeit = rustix::event::Timespec {
            tv_sec: i64::try_from(rest.as_secs()).unwrap_or(i64::MAX),
            tv_nsec: i64::from(rest.subsec_nanos()),
        };
        let bereit = {
            let fd = aus.as_fd();
            let mut fds = [rustix::event::PollFd::new(
                &fd,
                rustix::event::PollFlags::IN,
            )];
            rustix::event::poll(&mut fds, Some(&zeit))
        };
        match bereit {
            Ok(0) => return Lesen::Anders,
            Ok(_) => {}
            Err(rustix::io::Errno::INTR) => continue,
            Err(_) => return Lesen::Anders,
        }
        let mut puffer = [0u8; 64];
        match aus.read(&mut puffer) {
            Ok(0) => {
                return if zeile.is_empty() {
                    Lesen::Ende
                } else {
                    Lesen::Anders
                };
            }
            Ok(n) => {
                zeile.extend_from_slice(&puffer[..n]);
                if let Some(lf) = zeile.iter().position(|b| *b == b'\n') {
                    // only the first line counts; what follows the first LF is never judged, whether it arrived in
                    // the same read or later
                    return if &zeile[..=lf] == BEREIT {
                        Lesen::Bereit
                    } else {
                        Lesen::Anders
                    };
                }
                // sixteen bytes without an LF can no longer become the ready line (it is sixteen with its LF)
                if zeile.len() >= BEREIT.len() {
                    return Lesen::Anders;
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
            Err(_) => return Lesen::Anders,
        }
    }
}

/// Second argument of the trampoline that makes it the starter of the enclosure (`werkbank-lauf-kind <ppid> --halten
/// <starter> [argument ...]`): `PR_SET_PDEATHSIG = SIGKILL`, the parent check, no `setsid` (the starter keeps the
/// terminal and the foreground group of "Lauf starten"), descriptor 3 stays open.
pub const HALTEN: &str = "--halten";

/// Like [`HALTEN`], with the report to the service on descriptor 4 (contract 1, "Meldung an den Dienst"): the trampoline
/// sets close-on-exec on 4 before its `exec`, so the starter never holds the connection, and writes the line of
/// `lauf.harness_fehlt` there when the `exec` fails (`lauf.intern` when the binding fails). Its code is then the one of
/// the line.
pub const HALTEN_MELDEN: &str = "--halten-melden";

/// The trampoline `werkbank-lauf-kind <ppid> -- <program> [argument ...]`: binds itself to the parent and replaces
/// itself by the program. With [`HALTEN`] in place of `--` it becomes the starter of the enclosure. Returns only on a
/// fault, with the exit code; a starter that cannot be run writes the line of `lauf.harness_fehlt` like the `exec` of
/// "Lauf starten" did before.
#[must_use]
pub fn trampolin(args: &[String]) -> i32 {
    use std::os::unix::process::CommandExt;
    if args.first().map(String::as_str) == Some(WACHE) {
        return wache(&args[1..]);
    }
    let (Some(ppid), Some(art), Some(programm)) = (
        args.first().and_then(|p| p.parse::<i32>().ok()),
        args.get(1).map(String::as_str),
        args.get(2),
    ) else {
        return KIND_AUFRUF_FEHLER;
    };
    let (starter, melden) = match art {
        "--" => (false, false),
        HALTEN => (true, false),
        HALTEN_MELDEN => (true, true),
        _ => return KIND_AUFRUF_FEHLER,
    };
    if !binden(ppid, starter) {
        if melden {
            return vier_melden(&crate::Fehler::Intern);
        }
        return KIND_AUFRUF_FEHLER;
    }
    // the starter never holds the connection: close-on-exec on 4 before the `exec`
    if melden && !vier_bei_exec_schliessen() {
        return vier_melden(&crate::Fehler::Intern);
    }
    let fehler = Command::new(programm).args(&args[3..]).env_clear().exec();
    drop(fehler);
    if starter {
        let f = crate::Fehler::HarnessFehlt;
        let _ = std::io::Write::write_all(&mut std::io::stderr(), f.zeile().as_bytes());
        if melden {
            return vier_melden(&f);
        }
        return i32::from(f.exitcode());
    }
    KIND_START_FEHLER
}

/// Sets close-on-exec on descriptor 4 (`nix`: `fcntl` on a number this process got from its parent; `rustix` and `std`
/// take only descriptors they own, and the workspace forbids `unsafe`).
#[cfg(target_os = "linux")]
fn vier_bei_exec_schliessen() -> bool {
    use nix::fcntl::{FcntlArg, FdFlag, fcntl};
    fcntl(
        crate::meldung::MELDUNG_FD,
        FcntlArg::F_SETFD(FdFlag::FD_CLOEXEC),
    )
    .is_ok()
}

#[cfg(not(target_os = "linux"))]
fn vier_bei_exec_schliessen() -> bool {
    false
}

/// Writes the line of `f` to the service on descriptor 4 (best effort, no SIGPIPE) and gives the code of the line.
#[cfg(target_os = "linux")]
fn vier_melden(f: &crate::Fehler) -> i32 {
    use nix::sys::socket::{MsgFlags, send};
    let _ = send(
        crate::meldung::MELDUNG_FD,
        crate::meldung::zeile(f).as_bytes(),
        MsgFlags::MSG_NOSIGNAL,
    );
    i32::from(f.exitcode())
}

#[cfg(not(target_os = "linux"))]
fn vier_melden(f: &crate::Fehler) -> i32 {
    i32::from(f.exitcode())
}

/// `PR_SET_PDEATHSIG` (SIGTERM for a child, SIGKILL for the starter), then the parent check (the parent may have died
/// before the signal was set), then `setsid` for a child only. Linux only; elsewhere the binding is not available and
/// the trampoline refuses.
#[cfg(target_os = "linux")]
fn binden(ppid: i32, starter: bool) -> bool {
    let signal = if starter {
        rustix::process::Signal::KILL
    } else {
        rustix::process::Signal::TERM
    };
    if rustix::process::set_parent_process_death_signal(Some(signal)).is_err() {
        return false;
    }
    if rustix::process::getppid().map(rustix::process::Pid::as_raw_nonzero)
        != rustix::process::Pid::from_raw(ppid).map(rustix::process::Pid::as_raw_nonzero)
    {
        return false;
    }
    starter || rustix::process::setsid().is_ok()
}

/// The watcher (arguments: the PID of "Lauf starten", the socket folder, the session folder). Waits until that process
/// has ended, ends the two children named by the pidfds on its standard input and output (SIGTERM, after
/// [`WACHE_GNADE`] SIGKILL), then removes the session folder and last the socket folder (best effort; the socket folder
/// going last tells a waiting test that the watcher is done). The session folder is removed by its own name, never
/// through a link ([`crate::dateien::ordner_entfernen`]). A parent that is already gone counts as ended.
#[cfg(target_os = "linux")]
fn wache(args: &[String]) -> i32 {
    use rustix::event::{PollFd, PollFlags, Timespec};
    use rustix::process::{Pid, PidfdFlags, Signal};
    let (Some(lauf), Some(ordner), Some(sitzung)) = (
        args.first().and_then(|p| p.parse::<i32>().ok()),
        args.get(1),
        args.get(2),
    ) else {
        return KIND_AUFRUF_FEHLER;
    };
    let _ = rustix::process::setsid();
    let eltern = Pid::from_raw(lauf)
        .and_then(|p| rustix::process::pidfd_open(p, PidfdFlags::empty()).ok())
        // a process that is not the parent (the parent ended, the watcher was adopted) is no parent
        .filter(|_| {
            rustix::process::getppid().map(Pid::as_raw_nonzero)
                == Pid::from_raw(lauf).map(Pid::as_raw_nonzero)
        });
    if let Some(fd) = &eltern {
        loop {
            let mut fds = [PollFd::new(fd, PollFlags::IN)];
            // without a time limit `poll` returns only when the parent has ended (or on a signal)
            match rustix::event::poll(&mut fds, None) {
                Err(rustix::io::Errno::INTR) => {}
                _ => break,
            }
        }
    }
    let kinder = [rustix::stdio::stdin(), rustix::stdio::stdout()];
    for k in kinder {
        let _ = rustix::process::pidfd_send_signal(k, Signal::TERM);
    }
    let ende = Instant::now() + WACHE_GNADE;
    let fertig = |k: std::os::fd::BorrowedFd<'_>, frist: Option<&Timespec>| {
        let mut fds = [PollFd::new(&k, PollFlags::IN)];
        matches!(rustix::event::poll(&mut fds, frist), Ok(n) if n > 0)
    };
    for k in kinder {
        loop {
            let rest = ende.saturating_duration_since(Instant::now());
            let zeit = Timespec {
                tv_sec: i64::try_from(rest.as_secs()).unwrap_or(i64::MAX),
                tv_nsec: i64::from(rest.subsec_nanos()),
            };
            if fertig(k, Some(&zeit)) || rest.is_zero() {
                break;
            }
        }
        if !fertig(
            k,
            Some(&Timespec {
                tv_sec: 0,
                tv_nsec: 0,
            }),
        ) {
            let _ = rustix::process::pidfd_send_signal(k, Signal::KILL);
        }
    }
    let _ = crate::dateien::ordner_entfernen(Path::new(sitzung));
    let _ = std::fs::remove_dir_all(ordner);
    0
}

#[cfg(not(target_os = "linux"))]
fn wache(_args: &[String]) -> i32 {
    KIND_AUFRUF_FEHLER
}

#[cfg(not(target_os = "linux"))]
fn binden(_ppid: i32, _starter: bool) -> bool {
    false
}

#[cfg(test)]
mod tests;
