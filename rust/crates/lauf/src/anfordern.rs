//! The worker request over the report to the service (contract 1, "Worker anfordern"; Projektentscheid, 27.11.2026).
//!
//! The connection to the service never reaches the trampoline any more. With a report, "Lauf starten" starts a holding
//! process outside the enclosure (`werkbank-lauf-kind --halteprozess <pid>`) right before the hand-over. It holds the
//! connection to the service on its standard output, the read end of a pipe on its standard input and the listening
//! socket of the worker request on its standard error; nothing else. The trampoline gets the write end of the pipe on
//! descriptor 4 in place of the connection:
//!
//! - a code line from the pipe goes on to the service unchanged, and the holding process ends;
//! - the end of the pipe without data (the `exec` of the trampoline closed descriptor 4) gives [`LAEUFT`] once;
//! - after it, the holding process passes requests of the agent in exactly the form of [`anforderung_gut`] to the
//!   service and the answers of the service in the form of [`antwort_gut`] back. It checks length and grammar before
//!   it passes anything on and drops every other form itself: nothing from the enclosure reaches the service
//!   unchecked, and the agent never holds a connection to the service.
//!
//! The agent reaches the holding process only over the socket of the session at [`einschluss::GAST_ANFORDERN_SOCKET`],
//! with the tool `werkbank-lauf-kind --anfordern <rolle> <harness> <modell> <effort>` ([`fragen`]): one line, one
//! answer, one connection. Whether a worker may ask for a worker is the decision of the deciding office, never of this
//! program.

use std::io::{Read, Write};
#[cfg(target_os = "linux")]
use std::os::unix::net::UnixListener;
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::time::Duration;

/// The line of a start that reached the starter: the `exec` of the trampoline succeeded.
pub const LAEUFT: &[u8] = b"laeuft\n";

/// The first word of a request.
pub const ANFORDERN_WORT: &str = "anfordern";

/// The longest request in bytes, with its line end (form agreed with the service).
pub const ANFORDERUNG_MAX: usize = 256;

/// The longest answer of the service in bytes, with its line end.
pub const ANTWORT_MAX: usize = 128;

/// The longest field of the form of an identifier (`rolle`, `harness`, `effort`, `sitzung`).
pub const KENNUNG_MAX: usize = 64;

/// The longest model name (contract 1, field `modell`).
pub const MODELL_MAX: usize = 128;

/// The answer of the holding process to a second request while one is open (the service answers the same).
pub const OFFEN: &[u8] = b"abgelehnt anfordern_offen\n";

/// First argument of the trampoline that makes it the holding process.
pub const HALTEPROZESS: &str = "--halteprozess";

/// First argument of the trampoline that makes it the tool of the agent.
pub const ANFORDERN: &str = "--anfordern";

/// Option of the tool before the four fields: another socket than the guest path (tests).
pub const AN: &str = "--an";

/// How long the holding process waits for the line of an agent after its `connect`.
pub const ZEILEN_FRIST: Duration = Duration::from_secs(2);

/// How long the tool waits for the answer: the deciding office and the start of the worker take their time.
pub const ANTWORT_FRIST: Duration = Duration::from_secs(600);

/// Exit code of the tool for a wrong call or a field of the wrong form.
pub const KLIENT_AUFRUF: i32 = 64;
/// Exit code of the tool when the holding process cannot be reached or gives no answer.
pub const KLIENT_FEHLT: i32 = 69;
/// Exit code of the tool for the answer `abgelehnt`.
pub const KLIENT_ABGELEHNT: i32 = 1;

/// Whether `t` has the form of an identifier: `^[a-z0-9][a-z0-9._-]{0,63}$`.
#[must_use]
pub fn kennung_gut(t: &[u8]) -> bool {
    let klein = |b: &u8| b.is_ascii_lowercase() || b.is_ascii_digit();
    t.first().is_some_and(klein)
        && t.len() <= KENNUNG_MAX
        && t.iter()
            .all(|b| klein(b) || matches!(b, b'.' | b'_' | b'-'))
}

/// Whether `t` has the form of `modell` (contract 1): 1 to [`MODELL_MAX`] characters of `[A-Za-z0-9._:/@+-]`.
#[must_use]
pub fn modell_gut(t: &[u8]) -> bool {
    (1..=MODELL_MAX).contains(&t.len())
        && t.iter().all(|b| {
            b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b':' | b'/' | b'@' | b'+' | b'-')
        })
}

/// Whether `daten` is exactly one request: `anfordern <rolle> <harness> <modell> <effort>\n`, at most
/// [`ANFORDERUNG_MAX`] bytes, the fields separated by exactly one space, nothing after the line end.
#[must_use]
pub fn anforderung_gut(daten: &[u8]) -> bool {
    if daten.len() > ANFORDERUNG_MAX {
        return false;
    }
    let Some(text) = daten.strip_suffix(b"\n") else {
        return false;
    };
    let teile: Vec<&[u8]> = text.split(|b| *b == b' ').collect();
    let [wort, rolle, harness, modell, effort] = teile.as_slice() else {
        return false;
    };
    *wort == ANFORDERN_WORT.as_bytes()
        && kennung_gut(rolle)
        && kennung_gut(harness)
        && modell_gut(modell)
        && kennung_gut(effort)
}

/// The request line of the four fields, if each has its form.
#[must_use]
pub fn anforderung(rolle: &str, harness: &str, modell: &str, effort: &str) -> Option<String> {
    let zeile = format!("{ANFORDERN_WORT} {rolle} {harness} {modell} {effort}\n");
    anforderung_gut(zeile.as_bytes()).then_some(zeile)
}

/// Whether `daten` is exactly one answer of the service: `frei <sitzung>\n` with a session of the form of an
/// identifier, or `abgelehnt <code>\n` with a code of the form of the report ([`crate::meldung::grund_gut`]).
#[must_use]
pub fn antwort_gut(daten: &[u8]) -> bool {
    // no check of the length of its own: the form bounds an answer at 10 + 64 + 1 = 75 bytes, below ANTWORT_MAX, so a
    // longer `daten` always fails the form (a check `> ANTWORT_MAX` could never refuse on its own)
    let Some(text) = daten.strip_suffix(b"\n") else {
        return false;
    };
    let Ok(text) = std::str::from_utf8(text) else {
        return false;
    };
    match text.split_once(' ') {
        Some(("frei", sitzung)) => kennung_gut(sitzung.as_bytes()),
        Some(("abgelehnt", code)) => crate::meldung::grund_gut(code),
        _ => false,
    }
}

/// Reads from `quelle` up to the first line end, at most `max` bytes. `None` when the line is longer, the source ends
/// before the line end or its time is up.
fn zeile_lesen(quelle: &mut impl Read, max: usize) -> Option<Vec<u8>> {
    let mut zeile = Vec::new();
    let mut byte = [0u8; 1];
    // byte by byte: nothing after the line end is taken from the connection
    while zeile.len() < max {
        match quelle.read(&mut byte) {
            Ok(1) => {
                zeile.push(byte[0]);
                if byte[0] == b'\n' {
                    return Some(zeile);
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
            _ => return None,
        }
    }
    None
}

/// The tool of the agent: `werkbank-lauf-kind --anfordern [--an <socket>] <rolle> <harness> <modell> <effort>`.
/// Sends the one request to the holding process of the session, prints its answer on standard output and ends with 0
/// for `frei`, [`KLIENT_ABGELEHNT`] for `abgelehnt`, [`KLIENT_FEHLT`] when no answer comes and [`KLIENT_AUFRUF`] for
/// a wrong call. It knows nothing of the service: the socket leads only to the holding process.
#[must_use]
pub fn fragen(args: &[String]) -> i32 {
    let (socket, felder) = match args {
        [an, s, rest @ ..] if an == AN => (Path::new(s.as_str()), rest),
        rest => (Path::new(einschluss::GAST_ANFORDERN_SOCKET), rest),
    };
    let [rolle, harness, modell, effort] = felder else {
        return melden_fehler(
            "anfordern: genau rolle, harness, modell und effort erwartet\n",
            KLIENT_AUFRUF,
        );
    };
    let Some(zeile) = anforderung(rolle, harness, modell, effort) else {
        return melden_fehler("anfordern: ein Feld hat falsche Form\n", KLIENT_AUFRUF);
    };
    let antwort = UnixStream::connect(socket).ok().and_then(|mut s| {
        s.set_read_timeout(Some(ANTWORT_FRIST)).ok()?;
        s.write_all(zeile.as_bytes()).ok()?;
        zeile_lesen(&mut s, ANTWORT_MAX)
    });
    let Some(antwort) = antwort.filter(|a| antwort_gut(a)) else {
        return melden_fehler("anfordern: keine Antwort\n", KLIENT_FEHLT);
    };
    let _ = std::io::stdout().write_all(&antwort);
    if antwort.starts_with(b"frei ") {
        0
    } else {
        KLIENT_ABGELEHNT
    }
}

fn melden_fehler(text: &str, code: i32) -> i32 {
    let _ = std::io::stderr().write_all(text.as_bytes());
    code
}

/// The open request: the connection of the agent that waits for its answer.
#[derive(Debug, Default)]
pub struct Offen(Option<UnixStream>);

/// What the holding process does with a line of an agent: pass it on (the first open request), answer it itself with
/// [`OFFEN`], or drop it (a wrong form).
#[derive(Debug, PartialEq, Eq)]
pub enum Annahme {
    /// The request goes to the service.
    Weiter,
    /// A request is open already: the agent gets [`OFFEN`].
    Offen,
    /// Not a request: dropped without an answer, nothing reaches the service.
    Verworfen,
}

/// The decision on a line of an agent, from its form and whether a request is open.
#[must_use]
pub fn annahme(zeile: Option<&[u8]>, offen: bool) -> Annahme {
    match zeile {
        Some(z) if anforderung_gut(z) => {
            if offen {
                Annahme::Offen
            } else {
                Annahme::Weiter
            }
        }
        _ => Annahme::Verworfen,
    }
}

/// Phase 1 of the holding process: what goes to the service once the pipe of the trampoline has ended. The code line
/// (everything that came, at most [`crate::meldung::ZEILE_MAX`] bytes, judged by the service) or, after an end
/// without data, [`LAEUFT`] while "Lauf starten" lives. `None`: nothing (the parent ended before, so no starter runs
/// for certain; the service judges the end of the child as before).
#[must_use]
pub fn erste_zeile(rohr: &[u8], eltern_lebt: bool) -> Option<Vec<u8>> {
    if !rohr.is_empty() {
        return Some(rohr.to_vec());
    }
    eltern_lebt.then(|| LAEUFT.to_vec())
}

/// The holding process (arguments: the PID of "Lauf starten"). Descriptors: 0 the read end of the pipe of the
/// trampoline, 1 the connection to the service, 2 the listening socket of the worker request. Ends when the parent
/// ends, when the service closes the connection, after a code line, or at once for a wrong call. Linux only.
#[must_use]
pub fn halteprozess(args: &[String]) -> i32 {
    #[cfg(not(target_os = "linux"))]
    let code = {
        let _ = args;
        crate::kind::KIND_AUFRUF_FEHLER
    };
    #[cfg(target_os = "linux")]
    let code = halteprozess_linux(args);
    code
}

#[cfg(target_os = "linux")]
fn halteprozess_linux(args: &[String]) -> i32 {
    use rustix::process::{Pid, PidfdFlags};
    use std::os::fd::AsFd;
    let Some(eltern) = args.first().and_then(|p| p.parse::<i32>().ok()) else {
        return crate::kind::KIND_AUFRUF_FEHLER;
    };
    // its own session: a SIGINT of the terminal to the foreground group never reaches it
    let _ = rustix::process::setsid();
    let pidfd = Pid::from_raw(eltern)
        .and_then(|p| rustix::process::pidfd_open(p, PidfdFlags::empty()).ok())
        .filter(|_| {
            rustix::process::getppid().map(Pid::as_raw_nonzero)
                == Pid::from_raw(eltern).map(Pid::as_raw_nonzero)
        });
    let Some(pidfd) = pidfd else {
        return crate::kind::KIND_AUFRUF_FEHLER;
    };
    let nimm = |fd: std::os::fd::BorrowedFd<'_>| fd.try_clone_to_owned().ok();
    let (Some(rohr), Some(dienst), Some(hoerer)) = (
        nimm(std::io::stdin().as_fd()),
        nimm(std::io::stdout().as_fd()),
        nimm(std::io::stderr().as_fd()),
    ) else {
        return crate::kind::KIND_AUFRUF_FEHLER;
    };
    // nothing of the three stays on 0, 1 and 2: a write to standard error (a panic) must never reach the listener
    let Ok(null) = rustix::fs::open(
        "/dev/null",
        rustix::fs::OFlags::RDWR.union(rustix::fs::OFlags::CLOEXEC),
        rustix::fs::Mode::empty(),
    ) else {
        return crate::kind::KIND_AUFRUF_FEHLER;
    };
    // `dup2` of an open descriptor onto 0, 1 and 2 has no way to fail here (both numbers are valid and open); the
    // result is not judged
    let _ = rustix::stdio::dup2_stdin(&null);
    let _ = rustix::stdio::dup2_stdout(&null);
    let _ = rustix::stdio::dup2_stderr(&null);
    drop(null);
    let mut rohr = std::fs::File::from(rohr);
    let mut dienst = UnixStream::from(dienst);
    let hoerer = UnixListener::from(hoerer);
    let mut daten = Vec::new();
    let _ = (&mut rohr)
        .take(crate::meldung::ZEILE_MAX as u64)
        .read_to_end(&mut daten);
    drop(rohr);
    let Some(erste) = erste_zeile(&daten, !eltern_beendet(&pidfd)) else {
        return 0;
    };
    if dienst.write_all(&erste).is_err() || erste != LAEUFT {
        return 0;
    }
    if hoerer.set_nonblocking(true).is_err() {
        return 0;
    }
    vermitteln(&pidfd, &mut dienst, &hoerer);
    0
}

/// Whether the process of the pidfd has ended (its pidfd is readable).
#[cfg(target_os = "linux")]
fn eltern_beendet(pidfd: &std::os::fd::OwnedFd) -> bool {
    use rustix::event::{PollFd, PollFlags, Timespec};
    let mut fds = [PollFd::new(pidfd, PollFlags::IN)];
    let null = Timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    matches!(rustix::event::poll(&mut fds, Some(&null)), Ok(n) if n > 0)
}

/// Phase 2: passes requests and answers until the parent ends or the service closes the connection.
#[cfg(target_os = "linux")]
fn vermitteln(pidfd: &std::os::fd::OwnedFd, dienst: &mut UnixStream, hoerer: &UnixListener) {
    use rustix::event::{PollFd, PollFlags};
    let mut offen = Offen::default();
    let mut eingang: Vec<u8> = Vec::new();
    loop {
        let (eltern, vom_dienst, neu) = {
            let mut fds = [
                PollFd::new(pidfd, PollFlags::IN),
                PollFd::new(&*dienst, PollFlags::IN),
                PollFd::new(hoerer, PollFlags::IN),
            ];
            match rustix::event::poll(&mut fds, None) {
                Ok(_) => {}
                Err(rustix::io::Errno::INTR) => continue,
                Err(_) => return,
            }
            let an = |f: &PollFd<'_>| !f.revents().is_empty();
            (an(&fds[0]), an(&fds[1]), an(&fds[2]))
        };
        if eltern {
            return;
        }
        if vom_dienst && !vom_dienst_lesen(dienst, &mut eingang, &mut offen) {
            return;
        }
        if neu && !annehmen(dienst, hoerer, &mut offen) {
            return;
        }
    }
}

/// Reads what the service sent and hands each whole answer to the open request. `false` when the service closed the
/// connection or a line grew past [`ANTWORT_MAX`] (the form of the channel is broken: the holding process ends).
#[cfg(target_os = "linux")]
fn vom_dienst_lesen(dienst: &mut UnixStream, eingang: &mut Vec<u8>, offen: &mut Offen) -> bool {
    let mut puffer = [0u8; ANTWORT_MAX];
    let n = match dienst.read(&mut puffer) {
        Ok(0) | Err(_) => return false,
        Ok(n) => n,
    };
    eingang.extend_from_slice(&puffer[..n]);
    while let Some(lf) = eingang.iter().position(|b| *b == b'\n') {
        let zeile: Vec<u8> = eingang.drain(..=lf).collect();
        antwort_geben(&zeile, offen);
    }
    eingang.len() < ANTWORT_MAX
}

/// Gives an answer of the service to the open request and closes its connection. An answer of another form, or one
/// without an open request, is dropped.
pub fn antwort_geben(zeile: &[u8], offen: &mut Offen) {
    if !antwort_gut(zeile) {
        return;
    }
    if let Some(mut agent) = offen.0.take() {
        let _ = agent.write_all(zeile);
    }
}

/// Takes every waiting connection of an agent: reads its one line (at most [`ZEILEN_FRIST`]) and decides
/// ([`annahme`]). `false` when the request could not be written to the service.
#[cfg(target_os = "linux")]
fn annehmen(dienst: &mut UnixStream, hoerer: &UnixListener, offen: &mut Offen) -> bool {
    while let Ok((mut agent, _)) = hoerer.accept() {
        let zeile = agent
            .set_nonblocking(false)
            .and_then(|()| agent.set_read_timeout(Some(ZEILEN_FRIST)))
            .ok()
            .and_then(|()| zeile_lesen(&mut agent, ANFORDERUNG_MAX));
        match annahme(zeile.as_deref(), offen.0.is_some()) {
            Annahme::Weiter => {
                let Some(z) = zeile else { continue };
                if dienst.write_all(&z).is_err() {
                    return false;
                }
                offen.0 = Some(agent);
            }
            Annahme::Offen => {
                let _ = agent.write_all(OFFEN);
            }
            Annahme::Verworfen => {}
        }
    }
    true
}

#[cfg(test)]
mod tests;
