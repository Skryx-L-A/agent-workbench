//! The end of a session cleans up (contract `dienst`, DI12 "Sitzungsende räumt auf"; contract 1, `rust/spec/VERTRAEGE.md`
//! lines 696 to 704).
//!
//! At every end of a session (the child ended, also after a failed start of `Lauf starten`; `Beenden`; the end of the service) the
//! service first sends SIGTERM to the process groups named in `suche.pid` and `modellweg.pid` of the session folder, and then removes
//! the folder `s-<n>` with everything in it (start file, the two pid files, the key of the throw-away CA with its copy of the bundle).
//! The one function [`sitzung_ende_raeumt_auf`] is that rule; the callers only name the session.
//!
//! SECURITY: a pid file is written by the child of the session, so the pid in it is no more than a claim. A signal goes out only when
//! every condition of [`pidfd_pruefen`] holds (the file is a regular file of the session folder holding exactly one decimal number with a
//! line feed; the process lives; it leads its own session; it was started after the child of the session: later start time, or the same
//! tick of 10 ms and a larger pid, [`spaeter_gestartet`]; its parent is this service and it is no own child of the service). Under Linux
//! the process is read behind a pidfd that was opened first. The parent is the proof that the process belongs to the service: the
//! helpers are children of the child of the session, and at the end of the session that child has ended, so the kernel has handed its
//! live children to this service, the child subreaper (`waisen.rs`). A process that took the pid of a dead helper (a new shell of the
//! person, a later leader of a session of the same user) has another parent and gets no signal. An adopted child cannot be collected by
//! anyone but this service, and the service collects only in its own loop (`waisen.rs`), never between the check and the signal, so its
//! pid stays bound to the process of the pidfd. The own children of the service (the children of the other sessions) are named by the
//! caller and get no signal either. The signal
//! goes to the process group of the checked leader: a group number is not handed to another process while any process belongs to the
//! group, and a number comes back only after the whole pid space has run through, so between the check and the signal the number cannot
//! name another group. In doubt no signal goes out; the files are removed in any case. Removing never follows a link and acts only below the own session folder `s-<n>`.
//! The signal exists under Linux only (start times from `/proc/<pid>/stat`); under other systems only the files are removed (the signal
//! is step II of the order and open in the contract).

use std::fs::{File, Metadata};
use std::io::Read;
use std::os::fd::BorrowedFd;
use std::os::unix::fs::MetadataExt;
use std::path::Path;

use rustix::process::{Pid, Signal, kill_process_group};

/// The two files of a session folder whose pid is signalled.
const PID_DATEIEN: [&str; 2] = ["suche.pid", "modellweg.pid"];
/// The bound of the read: one byte more than a valid file holds (ten digits and a line feed, a pid is below 2^31), so that a longer file is seen as longer.
const PID_LESEN_MAX: u64 = 12;

/// What the service keeps of the child of a session to judge a pid later: its pid and the moment it was started (clock ticks
/// since the start of the system, field 22 of `/proc/<pid>/stat`). `start` is `None` where it cannot be read; then no signal goes out.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct KindZeit {
    pub(crate) pid: Option<u32>,
    pub(crate) start: Option<u64>,
}

impl KindZeit {
    /// Takes the moment of the start of the live child `pid`.
    pub(crate) fn von(pid: Option<u32>) -> Self {
        Self {
            pid,
            start: pid.and_then(startzeit),
        }
    }
}

/// The fields of `/proc/<pid>/stat` the check needs.
#[derive(Debug, PartialEq, Eq)]
struct ProzessStand {
    zustand: char,
    eltern: u64,
    gruppe: u64,
    sitzung: u64,
    start: u64,
}

/// Parses the text of `/proc/<pid>/stat`. The name of the process stands in brackets and may hold spaces and brackets itself, so the
/// fields are counted from the last closing bracket: state is field 3, parent 4, group 5, session 6, start time 22.
fn stat_lesen(text: &str) -> Option<ProzessStand> {
    let rest = &text[text.rfind(')')? + 1..];
    let felder: Vec<&str> = rest.split_ascii_whitespace().collect();
    Some(ProzessStand {
        zustand: felder.first()?.chars().next()?,
        eltern: felder.get(1)?.parse().ok()?,
        gruppe: felder.get(2)?.parse().ok()?,
        sitzung: felder.get(3)?.parse().ok()?,
        start: felder.get(19)?.parse().ok()?,
    })
}

fn prozess_stand(pid: u32) -> Option<ProzessStand> {
    if !cfg!(target_os = "linux") {
        return None;
    }
    // a length bound: the file is a few hundred bytes
    let mut text = String::new();
    File::open(format!("/proc/{pid}/stat"))
        .ok()?
        .take(4096)
        .read_to_string(&mut text)
        .ok()?;
    stat_lesen(&text)
}

/// The start time of the process `pid` (Linux), `None` where it is not readable.
pub(crate) fn startzeit(pid: u32) -> Option<u64> {
    prozess_stand(pid).map(|s| s.start)
}

/// The pid of a pid file: exactly decimal digits and one line feed, 1 to 10 digits, no sign, no blank, at least 2 and a value `rustix`
/// takes as a pid. Anything else is `None` (garbage in the file: no signal).
fn pid_text(roh: &[u8]) -> Option<u32> {
    let ziffern = roh.strip_suffix(b"\n")?;
    // an empty text has no digits to parse and ends as `None` below
    if ziffern.len() > 10 || !ziffern.iter().all(u8::is_ascii_digit) {
        return None;
    }
    let pid: u32 = std::str::from_utf8(ziffern).ok()?.parse().ok()?;
    // pid 1 is the init process; 0 and the values over `i32::MAX` are no process
    (pid >= 2 && i32::try_from(pid).is_ok()).then_some(pid)
}

fn gleiche_datei(a: &Metadata, b: &Metadata) -> bool {
    a.dev() == b.dev() && a.ino() == b.ino()
}

/// Reads the pid file `name` of the session folder. SECURITY: the file must be a regular file (checked with `lstat`, a link is refused),
/// the opened file must be the file `lstat` saw (device and inode, so that a swap between the check and the opening is seen), and at
/// most 11 bytes are read. `None` for anything else.
fn pid_lesen(ordner: &Path, name: &str) -> Option<u32> {
    let pfad = ordner.join(name);
    let vorher = std::fs::symlink_metadata(&pfad).ok()?;
    if !vorher.file_type().is_file() {
        return None;
    }
    let mut datei = File::open(&pfad).ok()?;
    let nachher = datei.metadata().ok()?;
    // the same device and inode as `lstat` saw: so it is the regular file, not a swapped one
    if !gleiche_datei(&vorher, &nachher) {
        return None;
    }
    let mut roh = Vec::new();
    datei
        .by_ref()
        .take(PID_LESEN_MAX)
        .read_to_end(&mut roh)
        .ok()?;
    pid_text(&roh)
}

/// `true` when a process with the start time `start` and the pid `pid` was started after the child (start time `kind_start`, pid
/// `kind_pid`). The start times count in ticks of 10 ms, so two starts in one tick cannot be told apart by the time. Within one tick
/// the pid decides: the kernel hands out pids in rising order and wraps only after the whole pid space (at most 2^22 numbers) is used,
/// which no machine does in 10 ms, so in one tick the larger pid is the later start. A start before the tick of the child, or in its
/// tick with the smaller pid, is not "after" (the process may be older than the child: no signal).
#[cfg(any(target_os = "linux", test))]
fn spaeter_gestartet(start: u64, pid: u32, kind_start: u64, kind_pid: u32) -> bool {
    start > kind_start || (start == kind_start && pid > kind_pid)
}

/// The conditions on the fields of the process behind a pid, free of any system call. `true` only when all of these hold: the
/// process is no zombie, it is not the child itself, not this service and none of the own children `eigene` of the service, its
/// parent is this service (the child of the session has ended, so its live children were handed to the service, see the module text),
/// it leads its own session (session and group equal its pid), and it was started after the child of the session
/// ([`spaeter_gestartet`]). The start time and the pid of the child must be known. In doubt `false`.
#[cfg(any(target_os = "linux", test))]
fn stand_zulaessig(pid: u32, stand: &ProzessStand, kind: KindZeit, eigene: &[u32]) -> bool {
    let (Some(kind_start), Some(kind_pid)) = (kind.start, kind.pid) else {
        return false;
    };
    let ich = std::process::id();
    if kind_pid == pid || pid == ich || eigene.contains(&pid) {
        return false;
    }
    let pid64 = u64::from(pid);
    !matches!(stand.zustand, 'Z' | 'X' | 'x')
        && stand.eltern == u64::from(ich)
        && stand.sitzung == pid64
        && stand.gruppe == pid64
        && spaeter_gestartet(stand.start, pid, kind_start, kind_pid)
}

/// The protection against a reused or foreign pid: the pidfd of the pid when the process that really has this pid now passes
/// [`stand_zulaessig`], else `None`; the caller keeps the pidfd open until the signal went out. Under Linux the order is
/// SECURITY-critical: first a pidfd of the pid, then the read of the fields of the process, then the question whether the process
/// behind the pidfd has ended meanwhile (a pidfd that has not ended names the process that held the pid during the whole read,
/// never a later owner of the number). Any failure gives `None`: in doubt no signal. Linux only; [`signalisieren`] sends nothing elsewhere.
#[cfg(target_os = "linux")]
fn pidfd_pruefen(pid: u32, kind: KindZeit, eigene: &[u32]) -> Option<std::os::fd::OwnedFd> {
    use rustix::event::{PollFd, PollFlags, Timespec, poll};
    use rustix::process::{PidfdFlags, pidfd_open};
    let roh = i32::try_from(pid).ok().and_then(Pid::from_raw)?;
    let fd = pidfd_open(roh, PidfdFlags::empty()).ok()?;
    let stand = prozess_stand(pid)?;
    if !stand_zulaessig(pid, &stand, kind, eigene) {
        return None;
    }
    // a pidfd is readable once its process has ended: then the fields may be those of a process that is gone
    let null = Timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    let mut fds = [PollFd::new(&fd, PollFlags::IN)];
    match poll(&mut fds, Some(&null)) {
        Ok(0) => Some(fd),
        _ => None,
    }
}

/// SIGTERM to the process group of the pid in the pid file, when the protection allows it. Returns whether a signal went out.
fn signalisieren(ordner: &Path, name: &str, kind: KindZeit, eigene: &[u32]) -> bool {
    let Some(pid) = pid_lesen(ordner, name) else {
        return false;
    };
    // outside Linux there is no pidfd and no `/proc`: never a signal (no second variant of `pidfd_pruefen`, a mutant in it could not fail a test here)
    #[cfg(target_os = "linux")]
    let pidfd = pidfd_pruefen(pid, kind, eigene);
    #[cfg(not(target_os = "linux"))]
    let pidfd: Option<()> = {
        let _ = (pid, kind, eigene);
        None
    };
    let Some(_pidfd) = pidfd else {
        return false;
    };
    // `pid_text` kept the pid within `i32`, so `from_raw` gives a pid
    let Some(gruppe) = i32::try_from(pid).ok().and_then(Pid::from_raw) else {
        return false;
    };
    // the group of the leader that passed the check: see the module text for why the number names no other group meanwhile
    kill_process_group(gruppe, Signal::TERM).is_ok()
}

/// `true` when the last part of the path is `s-` and digits: the form of the folder the service makes for a session.
fn ist_sitzungsordner(ordner: &Path) -> bool {
    ordner
        .file_name()
        .and_then(|n| n.to_str())
        .and_then(|n| n.strip_prefix("s-"))
        .is_some_and(|z| !z.is_empty() && z.bytes().all(|b| b.is_ascii_digit()))
}

/// Removes the session folder without following a link, only through the open socket folder `wurzel` (`loeschen.rs`): the name is the last
/// part of the path and must be `s-<n>`; no composed path is used for the removal.
fn loeschen(wurzel: BorrowedFd<'_>, ordner: &Path) {
    if let Some(name) = ordner.file_name().and_then(|n| n.to_str()) {
        crate::loeschen::sitzung_entfernen(wurzel, name);
    }
}

/// The rule "the end of a session cleans up": first the signals (SIGTERM to the groups of `suche.pid` and `modellweg.pid`, each only
/// with the protection of [`pidfd_pruefen`]), then the removal of the folder of the session. `eigene` are the pids of the own children
/// of the service that have not been collected (the children of the other sessions); they never get a signal. Returns the number of
/// signals sent.
///
/// SECURITY: this is the one place. The files are removed even when no signal goes out. Called at the end of the child, for
/// `Beenden` (through the end of the child) and at the end of the service; a second call finds nothing.
pub(crate) fn sitzung_ende_raeumt_auf(
    ordner: &Path,
    wurzel: BorrowedFd<'_>,
    kind: KindZeit,
    eigene: &[u32],
) -> usize {
    let mut gesendet = 0;
    if ist_sitzungsordner(ordner) {
        for name in PID_DATEIEN {
            if signalisieren(ordner, name, kind, eigene) {
                gesendet += 1;
            }
        }
    }
    loeschen(wurzel, ordner);
    gesendet
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;
    use std::path::PathBuf;
    use std::time::{Duration, Instant};
    use traeger::{Kind, Start};

    /// The rule through the open parent folder of the session folder, as the service does it with the socket folder.
    fn raeumen(ordner: &Path, kind: KindZeit) -> usize {
        raeumen_ohne(ordner, kind, &[])
    }

    /// [`raeumen`] with the own children `eigene` of the service.
    fn raeumen_ohne(ordner: &Path, kind: KindZeit, eigene: &[u32]) -> usize {
        use rustix::fs::{CWD, Mode, openat};
        let eltern = ordner.parent().expect("parent");
        let fd =
            openat(CWD, eltern, crate::loeschen::ORDNER_OFFEN, Mode::empty()).expect("open parent");
        sitzung_ende_raeumt_auf(ordner, std::os::fd::AsFd::as_fd(&fd), kind, eigene)
    }

    /// `loeschen` through the open parent folder of the path.
    fn loeschen_t(ordner: &Path) {
        use rustix::fs::{CWD, Mode, openat};
        let fd = openat(
            CWD,
            ordner.parent().expect("parent"),
            crate::loeschen::ORDNER_OFFEN,
            Mode::empty(),
        )
        .expect("open parent");
        loeschen(std::os::fd::AsFd::as_fd(&fd), ordner);
    }

    fn wegwerf(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("dienst-aufr-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).expect("folder");
        p
    }

    fn schlaf() -> Kind {
        Kind::starten(Start {
            argv: vec!["/bin/sleep".to_owned(), "30".to_owned()],
            env: Vec::new(),
            cwd: PathBuf::from("/"),
            spalten: 20,
            zeilen: 5,
            ruecklauf: 10,
        })
        .expect("child starts")
    }

    /// A child of the carrier becomes the leader of its session a moment after the start; the test waits for it (under Linux).
    #[cfg(target_os = "linux")]
    fn sitzungsleiter_warten(kind: &Kind) {
        let pid = kind.pid().expect("pid");
        let bis = Instant::now() + Duration::from_secs(5);
        while Instant::now() < bis {
            if prozess_stand(pid)
                .is_some_and(|s| s.sitzung == u64::from(pid) && s.gruppe == u64::from(pid))
            {
                return;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        panic!("the child did not become a session leader");
    }

    fn ende_in(kind: &mut Kind, frist: Duration) -> bool {
        let bis = Instant::now() + frist;
        while Instant::now() < bis {
            let _ = kind.lies(Duration::from_millis(20));
            if kind.ist_zu_ende() {
                return true;
            }
        }
        kind.ist_zu_ende()
    }

    #[test]
    fn pid_text_nur_ziffern_und_ein_zeilenende() {
        assert_eq!(pid_text(b"1234\n"), Some(1234));
        assert_eq!(pid_text(b"2\n"), Some(2));
        for schlecht in [
            &b""[..],
            b"\n",
            b"1234",
            b"1234\n\n",
            b"1234\r\n",
            b" 1234\n",
            b"+1234\n",
            b"-5\n",
            b"12 34\n",
            b"12a\n",
            b"0\n",
            b"1\n",
            b"01\x00\n",
            b"99999999999\n",
            b"2147483648\n",
            b"\xe0\xa5\xa7\n",
        ] {
            assert_eq!(pid_text(schlecht), None, "{schlecht:?}");
        }
        assert_eq!(pid_text(b"2147483647\n"), Some(i32::MAX as u32));
    }

    #[test]
    fn stat_wird_vom_letzten_klammerende_gezaehlt() {
        // name with spaces and a closing bracket; field 22 is the start time
        let text = "123 (a b) c) S 77 123 123 0 -1 4194560 100 0 0 0 1 2 0 0 20 0 1 0 987654 1000 100 18446744073709551615";
        let s = stat_lesen(text).expect("stat");
        assert_eq!(
            s,
            ProzessStand {
                zustand: 'S',
                eltern: 77,
                gruppe: 123,
                sitzung: 123,
                start: 987_654
            }
        );
        assert!(stat_lesen("kaputt").is_none());
        assert!(stat_lesen("1 (x) S 1 2").is_none());
        assert!(stat_lesen("1 (x) S y 2 2 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 5").is_none());
    }

    #[test]
    fn pid_text_mit_elf_ziffern_ist_keine_pid_auch_wenn_der_wert_klein_ist() {
        assert_eq!(pid_text(b"00000000002\n"), None);
        assert_eq!(pid_text(b"0000000002\n"), Some(2));
    }

    #[test]
    fn gleiche_datei_nur_bei_gleichem_geraet_und_gleicher_inode() {
        let o = wegwerf("gleich");
        std::fs::write(o.join("a"), b"x").expect("write");
        std::fs::write(o.join("b"), b"x").expect("write");
        let m = |n: &str| std::fs::metadata(o.join(n)).expect("meta");
        assert!(gleiche_datei(&m("a"), &m("a")));
        assert!(!gleiche_datei(&m("a"), &m("b")), "same device, other inode");
        let _ = std::fs::remove_dir_all(&o);
    }

    #[test]
    fn pid_datei_grenzen_der_laenge() {
        let o = wegwerf("laenge");
        // the longest valid file: ten digits and the line feed (11 bytes)
        std::fs::write(o.join("p"), b"2147483647\n").expect("write");
        assert_eq!(pid_lesen(&o, "p"), Some(2_147_483_647));
        // one byte more and a file with a long run of digits are no pid
        std::fs::write(o.join("p"), b"21474836470\n").expect("write");
        assert_eq!(pid_lesen(&o, "p"), None);
        std::fs::write(o.join("p"), b"0000000002\n0").expect("write");
        assert_eq!(pid_lesen(&o, "p"), None);
        let _ = std::fs::remove_dir_all(&o);
    }

    #[test]
    fn pid_datei_nur_regulaer_im_ordner_und_ein_link_nie() {
        let o = wegwerf("lesen");
        std::fs::write(o.join("suche.pid"), b"4242\n").expect("write");
        assert_eq!(pid_lesen(&o, "suche.pid"), Some(4242));
        assert_eq!(pid_lesen(&o, "modellweg.pid"), None, "missing");
        // garbage
        std::fs::write(o.join("modellweg.pid"), b"abc\n").expect("write");
        assert_eq!(pid_lesen(&o, "modellweg.pid"), None);
        // too long: no read of more than the bound
        std::fs::write(o.join("modellweg.pid"), vec![b'7'; 100_000]).expect("write");
        assert_eq!(pid_lesen(&o, "modellweg.pid"), None);
        // a link to a good file is no pid file
        let fremd = wegwerf("lesen-fremd");
        std::fs::write(fremd.join("pid"), b"4242\n").expect("write");
        std::fs::remove_file(o.join("modellweg.pid")).expect("rm");
        symlink(fremd.join("pid"), o.join("modellweg.pid")).expect("link");
        assert_eq!(pid_lesen(&o, "modellweg.pid"), None);
        // a folder
        std::fs::create_dir(o.join("x.pid")).expect("dir");
        assert_eq!(pid_lesen(&o, "x.pid"), None);
        let _ = std::fs::remove_dir_all(&o);
        let _ = std::fs::remove_dir_all(&fremd);
    }

    #[test]
    fn nur_s_n_ist_ein_sitzungsordner() {
        for gut in ["/a/s-1", "/a/s-12", "s-007"] {
            assert!(ist_sitzungsordner(Path::new(gut)), "{gut}");
        }
        for schlecht in ["/a/s-", "/a/s-1x", "/a/x-1", "/a/s--1", "/", "", "/a/S-1"] {
            assert!(!ist_sitzungsordner(Path::new(schlecht)), "{schlecht}");
        }
    }

    #[test]
    fn loeschen_folgt_keinem_link_und_bleibt_im_ordner() {
        let o = wegwerf("loeschen");
        let aussen = wegwerf("loeschen-aussen");
        std::fs::write(aussen.join("wichtig"), b"x").expect("write");
        std::fs::create_dir(aussen.join("unter")).expect("dir");
        std::fs::write(aussen.join("unter").join("datei"), b"y").expect("write");
        let s = o.join("s-1");
        std::fs::create_dir(&s).expect("dir");
        std::fs::write(s.join("start.json"), b"{}").expect("write");
        symlink(aussen.join("wichtig"), s.join("ca.key")).expect("link to a file");
        symlink(&aussen, s.join("bundle")).expect("link to a folder");
        std::fs::create_dir(s.join("innen")).expect("dir");
        symlink(aussen.join("unter"), s.join("innen").join("link")).expect("link in a sub folder");
        loeschen_t(&s);
        assert!(!s.exists() && s.symlink_metadata().is_err(), "folder gone");
        assert!(aussen.join("wichtig").exists(), "target of the file link");
        assert!(
            aussen.join("unter").join("datei").exists(),
            "target of the folder link"
        );
        // the folder itself is a link: the link goes, the target stays
        let s2 = o.join("s-2");
        symlink(&aussen, &s2).expect("link");
        loeschen_t(&s2);
        assert!(s2.symlink_metadata().is_err(), "link gone");
        assert!(aussen.join("wichtig").exists(), "target of the folder link");
        // the folder is a file
        let s3 = o.join("s-3");
        std::fs::write(&s3, b"x").expect("write");
        loeschen_t(&s3);
        assert!(s3.symlink_metadata().is_err());
        // a name that is no `s-<n>`: nothing happens
        let fremd = o.join("anderer");
        std::fs::create_dir(&fremd).expect("dir");
        loeschen_t(&fremd);
        assert!(fremd.exists(), "not named s-<n>");
        let _ = std::fs::remove_dir_all(&o);
        let _ = std::fs::remove_dir_all(&aussen);
    }

    #[test]
    fn zweiter_aufruf_und_fehlender_ordner_sind_still() {
        let o = wegwerf("zweimal");
        let s = o.join("s-1");
        std::fs::create_dir(&s).expect("dir");
        assert_eq!(raeumen(&s, KindZeit::default()), 0);
        assert!(!s.exists());
        assert_eq!(raeumen(&s, KindZeit::default()), 0);
        let _ = std::fs::remove_dir_all(&o);
    }

    #[test]
    fn ohne_bekannte_startzeit_des_kindes_kein_signal_aber_die_dateien_gehen() {
        let o = wegwerf("ohnezeit");
        let s = o.join("s-1");
        std::fs::create_dir(&s).expect("dir");
        let mut kind = schlaf();
        let pid = kind.pid().expect("pid");
        std::fs::write(s.join("suche.pid"), format!("{pid}\n")).expect("write");
        assert_eq!(raeumen(&s, KindZeit::default()), 0);
        assert!(!s.exists());
        assert!(!ende_in(&mut kind, Duration::from_millis(300)), "no signal");
        drop(kind);
        let _ = std::fs::remove_dir_all(&o);
    }

    /// The protection and the signal need `/proc` and sessions of their own: Linux.
    #[cfg(target_os = "linux")]
    #[test]
    fn signal_nur_an_einen_spaeter_gestarteten_sitzungsleiter() {
        let o = wegwerf("signal");
        // the "child of the session" is this test process: it started long before the helpers
        let kind = KindZeit::von(Some(std::process::id()));
        assert!(kind.start.is_some());
        // the start time counts in ticks of 10 ms: the helpers must start in a later tick than this process
        std::thread::sleep(Duration::from_millis(60));
        let mut helfer = schlaf();
        let mut aelter = schlaf();
        sitzungsleiter_warten(&helfer);
        sitzungsleiter_warten(&aelter);
        let hp = helfer.pid().expect("pid");
        let ap = aelter.pid().expect("pid");
        // 1. a helper that started after the child leads its session: it gets SIGTERM
        let s1 = o.join("s-1");
        std::fs::create_dir(&s1).expect("dir");
        std::fs::write(s1.join("suche.pid"), format!("{hp}\n")).expect("write");
        assert_eq!(raeumen(&s1, kind), 1);
        assert!(ende_in(&mut helfer, Duration::from_secs(3)), "SIGTERM");
        assert!(!s1.exists());
        // 2. the "child" started after the process of the pid file: no signal
        let spaeter = KindZeit {
            pid: Some(1),
            start: startzeit(ap).map(|t| t + 1),
        };
        let s2 = o.join("s-2");
        std::fs::create_dir(&s2).expect("dir");
        std::fs::write(s2.join("modellweg.pid"), format!("{ap}\n")).expect("write");
        assert_eq!(raeumen(&s2, spaeter), 0);
        assert!(!s2.exists(), "files go in any case");
        assert!(
            !ende_in(&mut aelter, Duration::from_millis(300)),
            "no signal"
        );
        // 3. the very same start time is "after" only with a smaller pid of the child: here the child has a larger pid
        let gleich = KindZeit {
            pid: Some(ap + 1),
            start: startzeit(ap),
        };
        let s3 = o.join("s-3");
        std::fs::create_dir(&s3).expect("dir");
        std::fs::write(s3.join("suche.pid"), format!("{ap}\n")).expect("write");
        assert_eq!(raeumen(&s3, gleich), 0);
        assert!(
            !ende_in(&mut aelter, Duration::from_millis(300)),
            "no signal"
        );
        // 4. the pid of the child itself: no signal even when the times say "after"
        let selbst = KindZeit {
            pid: Some(ap),
            start: Some(0),
        };
        let s4 = o.join("s-4");
        std::fs::create_dir(&s4).expect("dir");
        std::fs::write(s4.join("suche.pid"), format!("{ap}\n")).expect("write");
        assert_eq!(raeumen(&s4, selbst), 0);
        assert!(
            !ende_in(&mut aelter, Duration::from_millis(300)),
            "no signal"
        );
        // 5. not a session leader: the test process itself is none of ours to signal (it is no leader of a session of its own, or
        // it started before the "child")
        let s5 = o.join("s-5");
        std::fs::create_dir(&s5).expect("dir");
        std::fs::write(s5.join("suche.pid"), format!("{}\n", std::process::id())).expect("write");
        assert_eq!(raeumen(&s5, kind), 0);
        // 6. a process that does not exist
        let s6 = o.join("s-6");
        std::fs::create_dir(&s6).expect("dir");
        std::fs::write(s6.join("suche.pid"), b"2147483647\n").expect("write");
        assert_eq!(raeumen(&s6, kind), 0);
        assert!(!s6.exists());
        // 7. garbage and a link: no signal, folder gone
        let s7 = o.join("s-7");
        std::fs::create_dir(&s7).expect("dir");
        std::fs::write(s7.join("suche.pid"), format!("{ap} \n")).expect("write");
        let ziel = o.join("ziel.pid");
        std::fs::write(&ziel, format!("{ap}\n")).expect("write");
        symlink(&ziel, s7.join("modellweg.pid")).expect("link");
        assert_eq!(raeumen(&s7, kind), 0);
        assert!(!s7.exists());
        assert!(ziel.exists(), "the target of the link stays");
        assert!(
            !ende_in(&mut aelter, Duration::from_millis(300)),
            "no signal"
        );
        drop((helfer, aelter));
        let _ = std::fs::remove_dir_all(&o);
    }
    #[test]
    fn im_selben_takt_entscheidet_die_pid() {
        // later tick: after, whatever the pids say
        assert!(spaeter_gestartet(6, 1, 5, 9));
        // the same tick: after only with the larger pid
        assert!(spaeter_gestartet(5, 10, 5, 9));
        assert!(!spaeter_gestartet(5, 9, 5, 9));
        assert!(!spaeter_gestartet(5, 8, 5, 9));
        // an earlier tick: never after, even with the larger pid
        assert!(!spaeter_gestartet(4, 99, 5, 1));
    }

    /// The fields of a process whose parent is this service (the test process).
    fn stand(zustand: char, gruppe: u64, sitzung: u64, start: u64) -> ProzessStand {
        ProzessStand {
            zustand,
            eltern: u64::from(std::process::id()),
            gruppe,
            sitzung,
            start,
        }
    }

    #[test]
    fn stand_zulaessig_prueft_jede_bedingung_einzeln() {
        let kind = KindZeit {
            pid: Some(100),
            start: Some(50),
        };
        // the leader of its session and group, started in the same tick with a larger pid: allowed
        assert!(stand_zulaessig(101, &stand('S', 101, 101, 50), kind, &[]));
        // each condition alone
        assert!(
            !stand_zulaessig(101, &stand('Z', 101, 101, 50), kind, &[]),
            "zombie"
        );
        assert!(
            !stand_zulaessig(101, &stand('X', 101, 101, 50), kind, &[]),
            "dead"
        );
        assert!(
            !stand_zulaessig(101, &stand('x', 101, 101, 50), kind, &[]),
            "dead"
        );
        assert!(
            !stand_zulaessig(101, &stand('S', 101, 7, 50), kind, &[]),
            "no session leader"
        );
        assert!(
            !stand_zulaessig(101, &stand('S', 7, 101, 50), kind, &[]),
            "no group leader"
        );
        assert!(
            !stand_zulaessig(99, &stand('S', 99, 99, 50), kind, &[]),
            "same tick, smaller pid"
        );
        assert!(
            !stand_zulaessig(101, &stand('S', 101, 101, 49), kind, &[]),
            "earlier tick"
        );
        assert!(
            !stand_zulaessig(100, &stand('S', 100, 100, 51), kind, &[]),
            "the child itself"
        );
        assert!(
            !stand_zulaessig(std::process::id(), &stand('S', 0, 0, 51), kind, &[]),
            "this service"
        );
        // the parent must be this service: a process with another parent (a later leader that took the pid of a dead helper) is none of ours
        let mut fremd = stand('S', 101, 101, 50);
        fremd.eltern = 1;
        assert!(!stand_zulaessig(101, &fremd, kind, &[]), "another parent");
        fremd.eltern = u64::from(std::process::id()) + 1;
        assert!(!stand_zulaessig(101, &fremd, kind, &[]), "another parent");
        // an own child of the service (the child of another session) gets no signal
        assert!(
            !stand_zulaessig(101, &stand('S', 101, 101, 50), kind, &[7, 101]),
            "an own child"
        );
        assert!(stand_zulaessig(
            101,
            &stand('S', 101, 101, 50),
            kind,
            &[7, 100, 102]
        ));
        // the child must be known with both its pid and its start
        let ohne_start = KindZeit {
            pid: Some(100),
            start: None,
        };
        let ohne_pid = KindZeit {
            pid: None,
            start: Some(50),
        };
        assert!(!stand_zulaessig(
            101,
            &stand('S', 101, 101, 60),
            ohne_start,
            &[]
        ));
        assert!(!stand_zulaessig(
            101,
            &stand('S', 101, 101, 60),
            ohne_pid,
            &[]
        ));
        assert!(!stand_zulaessig(
            101,
            &stand('S', 101, 101, 60),
            KindZeit::default(),
            &[]
        ));
    }

    /// The forced condition of the flaky case `di12_sende_signal`: the helper started in the very tick of the child, with the larger pid.
    /// Before the fix (`start > kind_start` only) no signal went out here.
    #[cfg(target_os = "linux")]
    #[test]
    fn helfer_im_takt_des_kindes_mit_groesserer_pid_bekommt_das_signal() {
        let o = wegwerf("gleichertakt");
        let mut helfer = schlaf();
        sitzungsleiter_warten(&helfer);
        let hp = helfer.pid().expect("pid");
        let start = startzeit(hp).expect("start");
        // 1. the child has the same start tick and a smaller pid: the helper is later, SIGTERM
        let s1 = o.join("s-1");
        std::fs::create_dir(&s1).expect("dir");
        std::fs::write(s1.join("suche.pid"), format!("{hp}\n")).expect("write");
        let kind = KindZeit {
            pid: Some(hp - 1),
            start: Some(start),
        };
        assert_eq!(raeumen(&s1, kind), 1);
        assert!(ende_in(&mut helfer, Duration::from_secs(3)), "SIGTERM");
        assert!(!s1.exists());
        // 2. a second helper of the same tick, but a smaller pid than the child: no signal
        let mut zweiter = schlaf();
        sitzungsleiter_warten(&zweiter);
        let zp = zweiter.pid().expect("pid");
        let s2 = o.join("s-2");
        std::fs::create_dir(&s2).expect("dir");
        std::fs::write(s2.join("suche.pid"), format!("{zp}\n")).expect("write");
        let kind2 = KindZeit {
            pid: Some(zp + 1),
            start: startzeit(zp),
        };
        assert_eq!(raeumen(&s2, kind2), 0);
        assert!(
            !ende_in(&mut zweiter, Duration::from_millis(300)),
            "no signal"
        );
        drop((helfer, zweiter));
        let _ = std::fs::remove_dir_all(&o);
    }

    /// A session leader of the same user that started after the child of the session and is no child of this service: the test
    /// starts it through a shell that ends at once (`setsid` in a job of `sh`, which leads no group, so `setsid` makes the session
    /// itself and execs `sleep`). It goes to the next subreaper above this process, as a new shell of the person would. Returns its pid
    /// and a pidfd of it, opened while it sleeps; the test ends it only through that pidfd.
    #[cfg(target_os = "linux")]
    fn fremder_sitzungsleiter() -> (u32, std::os::fd::OwnedFd) {
        use rustix::process::{PidfdFlags, pidfd_open};
        let aus = std::process::Command::new("/bin/sh")
            .args([
                "-c",
                "setsid /bin/sleep 30 </dev/null >/dev/null 2>&1 & echo $!",
            ])
            .output()
            .expect("sh");
        let pid: u32 = String::from_utf8_lossy(&aus.stdout)
            .trim()
            .parse()
            .expect("pid");
        let fd = pidfd_open(
            Pid::from_raw(i32::try_from(pid).expect("pid")).expect("pid"),
            PidfdFlags::empty(),
        )
        .expect("pidfd");
        let bis = Instant::now() + Duration::from_secs(5);
        while Instant::now() < bis {
            if prozess_stand(pid)
                .is_some_and(|s| s.sitzung == u64::from(pid) && s.gruppe == u64::from(pid))
            {
                return (pid, fd);
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        panic!("the foreign process did not become a session leader");
    }

    /// `true` once the process of the pidfd has ended (a pidfd is readable then).
    #[cfg(target_os = "linux")]
    fn fd_beendet(fd: &std::os::fd::OwnedFd, frist: Duration) -> bool {
        use rustix::event::{PollFd, PollFlags, Timespec, poll};
        let t = Timespec {
            tv_sec: 0,
            tv_nsec: i64::try_from(frist.as_nanos()).expect("short"),
        };
        let mut fds = [PollFd::new(fd, PollFlags::IN)];
        matches!(poll(&mut fds, Some(&t)), Ok(n) if n > 0)
    }

    /// The forced case of P23: a helper writes its pid file and ends; a session leader of the same user that started later and is no
    /// child of this service stands behind the pid in the file. The kernel gives the number of the dead helper anew only after the
    /// whole pid space, which no test can force without rights over the pid namespace (`ns_last_pid`), so the test writes the pid of the
    /// foreign leader into the file: the same state as a real reuse. Before the fix (no check of the parent) it got SIGTERM.
    #[cfg(target_os = "linux")]
    #[test]
    fn ein_fremder_sitzungsleiter_mit_der_pid_eines_toten_helfers_bekommt_kein_signal() {
        let o = wegwerf("fremd");
        let kind = KindZeit::von(Some(std::process::id()));
        std::thread::sleep(Duration::from_millis(60));
        // 1. the helper writes its pid file and ends
        let helfer = schlaf();
        sitzungsleiter_warten(&helfer);
        let hp = helfer.pid().expect("pid");
        let s = o.join("s-1");
        std::fs::create_dir(&s).expect("dir");
        std::fs::write(s.join("suche.pid"), format!("{hp}\n")).expect("write");
        drop(helfer);
        // `Kind::drop` sent SIGKILL and collected it: the number is free
        assert!(prozess_stand(hp).is_none(), "the helper is gone");
        // 2. the foreign leader takes the pid of the file
        let (fp, fd) = fremder_sitzungsleiter();
        std::fs::write(s.join("suche.pid"), format!("{fp}\n")).expect("write");
        let st = crate::waisen::stat_lesen(
            &std::fs::read_to_string(format!("/proc/{fp}/stat")).expect("stat"),
        )
        .expect("stat");
        assert_ne!(st.eltern, std::process::id(), "no child of this process");
        assert!(
            startzeit(fp).zip(kind.start).is_some_and(|(f, k)| f > k),
            "started after the child"
        );
        // 3. no signal: it still sleeps
        assert_eq!(raeumen(&s, kind), 0);
        assert!(!s.exists(), "files go in any case");
        assert!(!fd_beendet(&fd, Duration::from_millis(300)), "no signal");
        let _ = rustix::process::pidfd_send_signal(std::os::fd::AsFd::as_fd(&fd), Signal::KILL);
        assert!(
            fd_beendet(&fd, Duration::from_millis(900)),
            "the test ends its own process"
        );
        let _ = std::fs::remove_dir_all(&o);
    }

    /// The own children of the service (the children of the other sessions) get no signal, even when they pass every other condition.
    #[cfg(target_os = "linux")]
    #[test]
    fn ein_eigenes_kind_des_dienstes_bekommt_kein_signal() {
        let o = wegwerf("eigene");
        let kind = KindZeit::von(Some(std::process::id()));
        std::thread::sleep(Duration::from_millis(60));
        let mut anderes = schlaf();
        sitzungsleiter_warten(&anderes);
        let ap = anderes.pid().expect("pid");
        let s = o.join("s-1");
        std::fs::create_dir(&s).expect("dir");
        std::fs::write(s.join("modellweg.pid"), format!("{ap}\n")).expect("write");
        assert_eq!(raeumen_ohne(&s, kind, &[ap]), 0);
        assert!(!s.exists());
        assert!(
            !ende_in(&mut anderes, Duration::from_millis(300)),
            "no signal"
        );
        // the same process, not named as an own child: the signal goes out
        let s2 = o.join("s-2");
        std::fs::create_dir(&s2).expect("dir");
        std::fs::write(s2.join("modellweg.pid"), format!("{ap}\n")).expect("write");
        assert_eq!(raeumen_ohne(&s2, kind, &[ap + 1]), 1);
        assert!(ende_in(&mut anderes, Duration::from_secs(3)), "SIGTERM");
        drop(anderes);
        let _ = std::fs::remove_dir_all(&o);
    }
}
