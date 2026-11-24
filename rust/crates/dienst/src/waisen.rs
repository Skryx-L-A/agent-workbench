//! The core service is the child subreaper of its sessions (Linux only; contract `dienst`, section 17).
//!
//! A SIGKILL to the program of a session (`werkbank-lauf`) in its first milliseconds leaves its children behind: the starter, and
//! below it the inner `bwrap`, which sets its death signal only after the set-up of the namespaces (bubblewrap 0.12.0, `do_init`).
//! Without a subreaper they go to PID 1 and outlive the session. So the service marks itself as the child subreaper at the start
//! ([`subreaper_werden`], after `kernabbild::abschirmen`, before the first child): every orphan below a session comes to the
//! service, which collects it and, at the end of the session, ends the orphans of that session.
//!
//! SECURITY, the rules of this module:
//! - **Only adopted processes are collected.** The own children of the service are the children of the sessions; their end is
//!   read by `traeger::Kind` (`try_wait`, `wait`). This module never calls `waitpid(-1)` or `waitid(P_ALL)`: it lists the children
//!   of the process (`/proc/self/task/<tid>/children`), leaves out every pid of a running own child ([`Waisen::runde`] gets
//!   them), opens a pidfd for each other one and collects it with `waitid(P_PIDFD, WEXITED | WNOHANG)`, which touches no other
//!   process. The end of an own child is seen with `WNOWAIT` on its pidfd, which leaves it to the carrier.
//! - **A signal goes out only through a pidfd whose process was checked after the pidfd was opened**: it is a child of this
//!   service (field 4 of `/proc/<pid>/stat`), it is no zombie, and its session (field 6) is the session of the ended child (the
//!   pid of the program of the session, which leads its session in the PTY). An adopted child cannot be collected by anyone
//!   but this service, so its pid stays bound to the process of the pidfd while the check runs.
//! - **The order is that of `Beenden` (DI12 point 6):** SIGHUP, up to 1 s, then SIGKILL. A process adopted later in the same
//!   hunt (a grandchild whose parent was just ended) gets SIGHUP at once and SIGKILL at the same moment as the others.
//! - Processes with a session of their own (the watcher, the search, the model path: each calls `setsid`) are never signalled;
//!   they are only collected once they have ended.
//!
//! Under other systems than Linux there is no subreaper and no pidfd: every function here does nothing (contract, section 17).

// outside Linux the hunt has no process to act on; its parts stay for the tests
#![cfg_attr(not(target_os = "linux"), allow(dead_code))]

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

#[cfg(target_os = "linux")]
use std::os::fd::{AsFd, OwnedFd};

/// The grace between SIGHUP and SIGKILL, the same as for `Beenden` (DI12 point 6).
pub(crate) const HUP_FRIST: Duration = Duration::from_millis(crate::starten::KILL_FRIST_MS);
/// The longest a hunt goes on; a process that does not end in this time (stuck in the kernel) is left to the end of the service.
pub(crate) const JAGD_MAX: Duration = Duration::from_secs(10);
/// How often the service looks at its children while a hunt runs.
pub(crate) const TAKT_JAGD: Duration = Duration::from_millis(5);
/// How often the service collects ended orphans while no hunt runs.
pub(crate) const TAKT_RUHE: Duration = Duration::from_millis(100);

/// Set once the process is the child subreaper. Only then are children collected and orphans hunted: a test that runs the
/// service inside its own process (and is no subreaper) has children of other tests, whose end is not the business of the service.
static SUBREAPER: AtomicBool = AtomicBool::new(false);

/// Marks the service as the child subreaper (`PR_SET_CHILD_SUBREAPER`). Linux only; elsewhere nothing happens.
///
/// # Errors
/// The `prctl` was refused.
#[cfg_attr(not(target_os = "linux"), allow(clippy::unnecessary_wraps))]
pub(crate) fn subreaper_werden() -> std::io::Result<()> {
    #[cfg(target_os = "linux")]
    {
        rustix::process::set_child_subreaper(Some(rustix::process::getpid()))?;
        SUBREAPER.store(true, Ordering::SeqCst);
    }
    Ok(())
}

/// The fixed text of the start error when the subreaper cannot list its children (contract, section 17, point 1).
pub(crate) const KINDER_FEHLT: &str = "Subreaper: children fehlt";

/// Checks once at the start, after [`subreaper_werden`], that the service can list its children: the `children` file of the calling
/// thread (`/proc/thread-self/children`, the same as `/proc/self/task/<tid>/children`) must be readable. Without it (a kernel without `CONFIG_PROC_CHILDREN`, a `/proc`
/// with `hidepid` or without `task`) every hunt would end at its first look, silently (N1 of the review of 23.11.2026). `true`
/// when the file can be read, or when the process is no subreaper (outside Linux, or a test that runs the service in its own
/// process).
pub(crate) fn kinder_lesbar() -> bool {
    #[cfg(target_os = "linux")]
    {
        if ist_subreaper() {
            // `/proc/thread-self` is `/proc/self/task/<tid>` of the calling thread (Linux 3.17)
            return std::fs::read_to_string("/proc/thread-self/children").is_ok();
        }
    }
    true
}

/// `true` once [`subreaper_werden`] succeeded (never outside Linux).
pub(crate) fn ist_subreaper() -> bool {
    SUBREAPER.load(Ordering::SeqCst)
}

/// The fields of `/proc/<pid>/stat` this module needs: state (field 3), parent (field 4) and session (field 6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Stand {
    pub(crate) zustand: char,
    pub(crate) eltern: u32,
    pub(crate) sitzung: u32,
}

impl Stand {
    /// A zombie (or a process being removed): it has ended and waits to be collected.
    pub(crate) fn beendet(self) -> bool {
        matches!(self.zustand, 'Z' | 'X' | 'x')
    }
}

/// Parses the text of `/proc/<pid>/stat`; the name in brackets may hold spaces and brackets, so the fields are counted from the
/// last closing bracket.
pub(crate) fn stat_lesen(text: &str) -> Option<Stand> {
    let rest = &text[text.rfind(')')? + 1..];
    let mut felder = rest.split_ascii_whitespace();
    let zustand = felder.next()?.chars().next()?;
    let eltern = felder.next()?.parse().ok()?;
    let _gruppe = felder.next()?;
    let sitzung = felder.next()?.parse().ok()?;
    Some(Stand {
        zustand,
        eltern,
        sitzung,
    })
}

/// The pids in the text of `children` files.
pub(crate) fn pids_lesen(text: &str) -> impl Iterator<Item = u32> + '_ {
    text.split_ascii_whitespace().filter_map(|w| w.parse().ok())
}

/// What the hunt of one session does with one live process of that session at the moment `jetzt`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Tat {
    Hup,
    Kill,
    Nichts,
}

/// The hunt for the orphans of one ended session: their session id, the moment of SIGKILL and the pids that got SIGHUP.
#[derive(Debug)]
pub(crate) struct Jagd {
    /// The session of the orphans: the pid of the ended child of the session.
    pub(crate) sid: u32,
    /// From this moment on every live orphan of the session gets SIGKILL.
    pub(crate) kill_ab: Instant,
    /// After this moment the hunt is given up.
    pub(crate) ende: Instant,
    /// The pids that got SIGHUP already (an adopted child keeps its pid until this service collects it; a pid given anew to a
    /// later orphan of the same session within the hunt gets SIGKILL at the moment of SIGKILL, without SIGHUP).
    gehupt: Vec<u32>,
}

impl Jagd {
    pub(crate) fn neu(sid: u32, jetzt: Instant) -> Self {
        Self {
            sid,
            kill_ab: jetzt + HUP_FRIST,
            ende: jetzt + JAGD_MAX,
            gehupt: Vec::new(),
        }
    }

    /// SIGHUP once per process before the moment of SIGKILL, SIGKILL from then on (also again, until the process has ended).
    pub(crate) fn tat(&mut self, pid: u32, jetzt: Instant) -> Tat {
        if jetzt >= self.kill_ab {
            Tat::Kill
        } else if self.gehupt.contains(&pid) {
            Tat::Nichts
        } else {
            self.gehupt.push(pid);
            Tat::Hup
        }
    }

    /// Whether the hunt goes on before a look: not after [`JAGD_MAX`], and not when its session id is the pid of a running own
    /// child of a session that is not hunted (the number was given anew).
    pub(crate) fn bleibt(&self, jetzt: Instant, laufende: &[u32]) -> bool {
        jetzt < self.ende && !laufende.contains(&self.sid)
    }
}

/// The orphans of the service: the hunts that run, and the clock of the next look.
#[derive(Debug, Default)]
pub(crate) struct Waisen {
    jagden: Vec<Jagd>,
    naechste: Option<Instant>,
}

impl Waisen {
    /// Starts the hunt for the orphans of the session `sid` (the pid of its ended child), once per session.
    pub(crate) fn jagen(&mut self, sid: u32, jetzt: Instant) {
        if !self.jagden.iter().any(|j| j.sid == sid) {
            self.jagden.push(Jagd::neu(sid, jetzt));
            self.naechste = Some(jetzt);
        }
    }

    /// `true` while a hunt runs.
    pub(crate) fn jagt(&self) -> bool {
        !self.jagden.is_empty()
    }

    /// `true` when the clock says it is time for the next look.
    pub(crate) fn faellig(&self, jetzt: Instant) -> bool {
        self.naechste.is_none_or(|n| jetzt >= n)
    }

    /// One look at the children, at most every [`TAKT_JAGD`] (hunt) or [`TAKT_RUHE`] (no hunt): collects every ended adopted child
    /// and signals the live orphans of the hunted sessions. `eigene` are the pids of the own children that have not been
    /// collected by their carrier (also an ended child whose session is hunted); they are never touched. `laufende` are the pids of
    /// the own children whose session is not hunted: a hunt whose session id is one of them ends at once (that pid was given
    /// anew). A hunt ends as well when one look found no process of its session, or after [`JAGD_MAX`]. Without the subreaper
    /// ([`ist_subreaper`]) the look sees no child, and every hunt ends.
    pub(crate) fn runde(&mut self, eigene: &[u32], laufende: &[u32], jetzt: Instant) {
        if !self.faellig(jetzt) {
            return;
        }
        self.jagden.retain(|j| j.bleibt(jetzt, laufende));
        // outside Linux there is no subreaper, and no second variant of `durchgang` (a mutant in it could not fail a test here)
        #[cfg(target_os = "linux")]
        let gesehen = if ist_subreaper() {
            durchgang(eigene, &mut self.jagden, jetzt)
        } else {
            Vec::new()
        };
        #[cfg(not(target_os = "linux"))]
        let gesehen: Vec<u32> = {
            let _ = eigene;
            Vec::new()
        };
        self.jagden.retain(|j| gesehen.contains(&j.sid));
        let takt = if self.jagden.is_empty() {
            TAKT_RUHE
        } else {
            TAKT_JAGD
        };
        self.naechste = Some(jetzt + takt);
    }

    /// The end of the service: hunts every session in `sids` to the end (the whole order, at most [`JAGD_MAX`]) and collects
    /// what ended. `eigene` is empty then (the carriers have collected their children).
    pub(crate) fn alle_beenden(&mut self, sids: &[u32], eigene: &[u32]) {
        let jetzt = Instant::now();
        for sid in sids {
            self.jagen(*sid, jetzt);
        }
        while self.jagt() {
            let jetzt = Instant::now();
            self.naechste = None;
            self.runde(eigene, &[], jetzt);
            if self.jagt() {
                std::thread::sleep(TAKT_JAGD);
            }
        }
        self.runde(eigene, &[], Instant::now());
    }
}

/// Ends children that a failed start leaves behind (DI8) and returns their pids: each child leads its session in the PTY, so its
/// pid is the session id of its orphans. The pids are taken before the end. `Starten` hunts them in the loop (`Waisen::jagen`).
pub(crate) fn kinder_verwerfen(kinder: Vec<traeger::Kind>) -> Vec<u32> {
    let sids = kinder.iter().filter_map(traeger::Kind::pid).collect();
    crate::sitzung::kinder_beenden(kinder);
    sids
}

/// A start error of `Dienst::neu` after the child started (M1 of the review of 23.11.2026): ends the children and then hunts
/// their orphans to the end (the whole order, at most [`JAGD_MAX`]) before the service ends with the error (section 17, point 8).
pub(crate) fn start_verwerfen(kinder: Vec<traeger::Kind>) {
    let sids = kinder_verwerfen(kinder);
    Waisen::default().alle_beenden(&sids, &[]);
}

/// One pass over the adopted children; returns the session ids of the hunts that still saw a process of their session (alive,
/// or collected in this pass: its children may have come to this service after the list was read).
#[cfg(target_os = "linux")]
fn durchgang(eigene: &[u32], jagden: &mut [Jagd], jetzt: Instant) -> Vec<u32> {
    let mut gesehen = Vec::new();
    let ich = std::process::id();
    for pid in kinder() {
        if eigene.contains(&pid) {
            continue;
        }
        // SECURITY: first the pidfd, then the check of the process behind it, then the signal through the pidfd
        let Some(fd) = pidfd(pid) else {
            continue;
        };
        let Some(stand) = stand_von(pid) else {
            continue;
        };
        if stand.eltern != ich {
            continue;
        }
        let jagd = jagden.iter_mut().find(|j| j.sid == stand.sitzung);
        if let Some(j) = &jagd {
            gesehen.push(j.sid);
        }
        if stand.beendet() {
            ernten(&fd);
            continue;
        }
        let Some(j) = jagd else {
            continue;
        };
        let signal = match j.tat(pid, jetzt) {
            Tat::Hup => rustix::process::Signal::HUP,
            Tat::Kill => rustix::process::Signal::KILL,
            Tat::Nichts => continue,
        };
        let _ = rustix::process::pidfd_send_signal(fd.as_fd(), signal);
    }
    gesehen
}

/// The children of this process: the `children` file of every thread (a child is listed at the thread that forked it, an
/// orphan at the thread that adopted it). Empty where nothing can be read.
#[cfg(target_os = "linux")]
fn kinder() -> Vec<u32> {
    let Ok(faeden) = std::fs::read_dir("/proc/self/task") else {
        return Vec::new();
    };
    let mut kinder = Vec::new();
    for f in faeden.filter_map(Result::ok) {
        if let Ok(text) = std::fs::read_to_string(f.path().join("children")) {
            kinder.extend(pids_lesen(&text));
        }
    }
    kinder.sort_unstable();
    kinder.dedup();
    kinder
}

#[cfg(target_os = "linux")]
fn pidfd(pid: u32) -> Option<OwnedFd> {
    let pid = i32::try_from(pid)
        .ok()
        .and_then(rustix::process::Pid::from_raw)?;
    rustix::process::pidfd_open(pid, rustix::process::PidfdFlags::empty()).ok()
}

#[cfg(target_os = "linux")]
fn stand_von(pid: u32) -> Option<Stand> {
    use std::io::Read;
    let mut text = String::new();
    std::fs::File::open(format!("/proc/{pid}/stat"))
        .ok()?
        .take(4096)
        .read_to_string(&mut text)
        .ok()?;
    stat_lesen(&text)
}

/// Collects the process of the pidfd if it has ended (`waitid(P_PIDFD, WEXITED | WNOHANG)`): only this process, never another child.
#[cfg(target_os = "linux")]
fn ernten(fd: &OwnedFd) {
    use rustix::process::{WaitId, WaitIdOptions, waitid};
    let _ = waitid(
        WaitId::PidFd(fd.as_fd()),
        // `union`, not `|`: the bits are disjoint, so a `^` would mean the same and no test could tell them apart
        WaitIdOptions::EXITED.union(WaitIdOptions::NOHANG),
    );
}

/// A pidfd of an own child, taken right after its start (before the carrier can collect it), to see its end without collecting it.
#[derive(Debug, Default)]
pub(crate) struct Kindblick {
    #[cfg(target_os = "linux")]
    fd: Option<OwnedFd>,
    pid: Option<u32>,
}

impl Kindblick {
    /// Opens the pidfd of the own child `pid`. Called before the carrier was asked for the end of the child, so the pid is
    /// still that child.
    pub(crate) fn von(pid: Option<u32>) -> Self {
        Self {
            #[cfg(target_os = "linux")]
            fd: pid.and_then(pidfd),
            pid,
        }
    }

    /// The pid of the child, which is the id of its session (it leads its session in the PTY).
    pub(crate) fn pid(&self) -> Option<u32> {
        self.pid
    }

    /// `true` once the child has ended; it stays a zombie for the carrier (`waitid(P_PIDFD, WEXITED | WNOHANG | WNOWAIT)`).
    /// After the carrier has collected it the call fails, which counts as ended as well.
    #[cfg_attr(not(target_os = "linux"), allow(clippy::unused_self))]
    pub(crate) fn beendet(&self) -> bool {
        #[cfg(target_os = "linux")]
        {
            use rustix::process::{WaitId, WaitIdOptions, waitid};
            if let Some(fd) = &self.fd {
                return !matches!(
                    waitid(
                        WaitId::PidFd(fd.as_fd()),
                        WaitIdOptions::EXITED
                            .union(WaitIdOptions::NOHANG)
                            .union(WaitIdOptions::NOWAIT)
                    ),
                    Ok(None)
                );
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stat_mit_klammern_und_leerzeichen_im_namen() {
        let text = "4242 (a) b) c) S 17 4242 99 0 -1 4194560 0 0 0 0 0 0 0 0 20 0 1 0 12345 0 0";
        assert_eq!(
            stat_lesen(text),
            Some(Stand {
                zustand: 'S',
                eltern: 17,
                sitzung: 99
            })
        );
        assert!(stat_lesen("kaputt").is_none());
        assert!(stat_lesen("1 (x) S 2 3").is_none());
        assert!(stat_lesen("1 (x) S x 3 4").is_none());
        let z = stat_lesen("7 (z) Z 1 7 8").expect("zombie");
        assert!(z.beendet());
        for (c, ende) in [
            ('Z', true),
            ('X', true),
            ('x', true),
            ('S', false),
            ('R', false),
            ('D', false),
            ('T', false),
        ] {
            assert_eq!(
                Stand {
                    zustand: c,
                    eltern: 1,
                    sitzung: 1
                }
                .beendet(),
                ende,
                "{c}"
            );
        }
    }

    #[test]
    fn pids_der_children_datei() {
        assert_eq!(pids_lesen("12 345 6\n").collect::<Vec<_>>(), [12, 345, 6]);
        assert_eq!(pids_lesen("").count(), 0);
        assert_eq!(pids_lesen("7 x 8").collect::<Vec<_>>(), [7, 8]);
    }

    #[test]
    fn folge_sighup_einmal_dann_ab_der_frist_sigkill() {
        let t0 = Instant::now();
        let mut j = Jagd::neu(100, t0);
        assert_eq!(j.kill_ab, t0 + HUP_FRIST);
        assert_eq!(HUP_FRIST, Duration::from_secs(1), "the second of Beenden");
        assert_eq!(j.tat(5, t0), Tat::Hup);
        assert_eq!(j.tat(5, t0 + Duration::from_millis(10)), Tat::Nichts);
        assert_eq!(j.tat(6, t0 + Duration::from_millis(999)), Tat::Hup);
        assert_eq!(j.tat(5, t0 + HUP_FRIST), Tat::Kill);
        assert_eq!(j.tat(5, t0 + HUP_FRIST), Tat::Kill, "again until it ends");
        assert_eq!(
            j.tat(7, t0 + HUP_FRIST),
            Tat::Kill,
            "adopted late: SIGKILL at the same moment"
        );
        assert_eq!(j.gehupt, [5, 6]);
    }

    #[test]
    fn eine_jagd_bleibt_bis_zur_hoechstdauer_und_nicht_bei_einem_neu_vergebenen_pid() {
        let t0 = Instant::now();
        let j = Jagd::neu(100, t0);
        assert_eq!(j.ende, t0 + JAGD_MAX);
        assert!(j.bleibt(t0, &[]));
        assert_eq!(JAGD_MAX, Duration::from_secs(10));
        assert!(j.bleibt(t0 + Duration::from_millis(9_999), &[7, 8]));
        assert!(
            !j.bleibt(t0 + JAGD_MAX, &[]),
            "the end is the highest duration"
        );
        assert!(
            !j.bleibt(t0, &[7, 100]),
            "the pid of the session is a running own child"
        );
        assert!(!j.bleibt(t0 + JAGD_MAX, &[100]));
    }

    #[test]
    fn eine_jagd_je_sitzung_und_ein_wiederverwendeter_pid_beendet_sie() {
        let t0 = Instant::now();
        let mut w = Waisen::default();
        assert!(w.faellig(t0));
        w.jagen(4_000_000, t0);
        w.jagen(4_000_000, t0);
        assert_eq!(w.jagden.len(), 1);
        assert!(w.jagt());
        // the pid of the ended child is now a running own child: no process of that number is an orphan of the session
        w.runde(&[4_000_000], &[4_000_000], t0);
        assert!(!w.jagt());
        assert!(
            !ist_subreaper(),
            "the tests of the crate never make their process a subreaper"
        );
    }

    #[test]
    fn eine_jagd_ohne_prozess_ihrer_sitzung_endet_nach_einem_blick_und_nach_der_hoechstdauer() {
        let t0 = Instant::now();
        let mut w = Waisen::default();
        w.jagen(4_000_001, t0);
        w.runde(&[], &[], t0);
        assert!(!w.jagt(), "no process of the session: the hunt ends");
        assert!(!w.faellig(t0), "the next look waits for the clock");
        assert!(w.faellig(t0 + TAKT_RUHE));
        let mut w = Waisen::default();
        w.jagen(4_000_002, t0);
        w.runde(&[], &[], t0 + JAGD_MAX);
        assert!(!w.jagt());
    }

    #[test]
    fn der_takt_ist_kurz_waehrend_einer_jagd() {
        assert!(TAKT_JAGD < TAKT_RUHE);
        assert!(TAKT_JAGD <= Duration::from_millis(5));
        assert!(JAGD_MAX > HUP_FRIST * 2);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn der_blick_auf_ein_eigenes_kind_sammelt_es_nicht_ein() {
        let mut laeuft = std::process::Command::new("/bin/sleep")
            .arg("30")
            .spawn()
            .expect("sleep");
        let blick = Kindblick::von(Some(laeuft.id()));
        assert!(!blick.beendet(), "a running child has not ended");
        let _ = laeuft.kill();
        let _ = laeuft.wait();
        let mut kind = std::process::Command::new("/bin/sh")
            .args(["-c", "exit 9"])
            .spawn()
            .expect("sh");
        let blick = Kindblick::von(Some(kind.id()));
        assert_eq!(blick.pid(), Some(kind.id()));
        let ende = Instant::now() + Duration::from_secs(10);
        while !blick.beendet() {
            assert!(Instant::now() < ende, "the end of the child was not seen");
            std::thread::sleep(Duration::from_millis(5));
        }
        // WNOWAIT left the status to the owner of the child
        assert_eq!(kind.wait().expect("wait").code(), Some(9));
        assert!(blick.beendet(), "a collected child counts as ended");
    }

    /// Without the subreaper (a test in the process of other tests, every system but Linux) nothing is checked: the start goes on.
    #[test]
    fn ohne_subreaper_gilt_die_children_datei_als_lesbar() {
        if !ist_subreaper() {
            assert!(kinder_lesbar());
        }
        assert_eq!(KINDER_FEHLT, "Subreaper: children fehlt");
    }

    /// M1: a start error after the child started ends the child and then its orphans (section 17). The test runs alone in a
    /// process of its own (the subreaper takes every orphan of the process; other tests may not share it): the outer call
    /// starts the test binary again with only this test, the inner one does the work.
    #[cfg(target_os = "linux")]
    #[test]
    fn start_verwerfen_beendet_auch_die_waisen_des_kindes() {
        const ALLEIN: &str = "WB_DIENST_TEST_START_VERWERFEN";
        if std::env::var_os(ALLEIN).is_none() {
            let status = std::process::Command::new(std::env::current_exe().expect("test binary"))
                .args([
                    "--exact",
                    "waisen::tests::start_verwerfen_beendet_auch_die_waisen_des_kindes",
                    "--test-threads=1",
                    "--nocapture",
                ])
                .env(ALLEIN, "1")
                .status()
                .expect("test process");
            assert!(status.success(), "the inner test failed: {status:?}");
            return;
        }
        subreaper_werden().expect("subreaper");
        let ordner =
            std::env::temp_dir().join(format!("dienst-start-verwerfen-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&ordner);
        std::fs::create_dir(&ordner).expect("folder");
        // the child ignores SIGHUP (so the orphan exists before `kinder_beenden` ends the child with SIGKILL) and leaves a
        // SIGHUP-deaf orphan in its session; only the hunt ends that orphan, with SIGKILL after the second
        let skript = "trap '' HUP\n( exec sleep 60 ) </dev/null >/dev/null 2>&1 &\necho $! > \"$1/w.tmp\"\n\
                      mv \"$1/w.tmp\" \"$1/waise.pid\"\nexec sleep 60\n";
        let kind = traeger::Kind::starten(traeger::Start {
            argv: vec![
                "/bin/sh".to_owned(),
                "-c".to_owned(),
                skript.to_owned(),
                "sh".to_owned(),
                ordner.to_string_lossy().into_owned(),
            ],
            env: vec![("PATH".to_owned(), "/usr/bin:/bin".to_owned())],
            cwd: ordner.clone(),
            spalten: 40,
            zeilen: 10,
            ruecklauf: 100,
        })
        .expect("child");
        let ende = Instant::now() + Duration::from_secs(10);
        let waise: u32 = loop {
            if let Some(p) = std::fs::read_to_string(ordner.join("waise.pid"))
                .ok()
                .and_then(|t| t.trim().parse().ok())
            {
                break p;
            }
            assert!(Instant::now() < ende, "the orphan was not started");
            std::thread::sleep(Duration::from_millis(5));
        };
        let halt = pidfd(waise).expect("pidfd of the orphan");
        let lebt = |p: u32| {
            rustix::process::Pid::from_raw(i32::try_from(p).expect("pid"))
                .is_some_and(|p| rustix::process::test_kill_process(p).is_ok())
        };
        start_verwerfen(vec![kind]);
        let weg = !lebt(waise);
        // whatever the result: nothing of the test is left behind
        let _ = rustix::process::pidfd_send_signal(halt.as_fd(), rustix::process::Signal::KILL);
        let _ = std::fs::remove_dir_all(&ordner);
        assert!(
            weg,
            "the orphan of the discarded child is gone: {:?}",
            stand_von(waise)
        );
    }
}
