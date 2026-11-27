//! The peer of a connection under Linux (contract 15.12.3, H1 and H2).
//!
//! - uid and pid come from `SO_PEERCRED`; the kernel keeps the uid of the time of `connect`.
//! - H2 asks whether the peer runs in the unit of a run. The helper reads `/proc/<pid>/cgroup` (line `0::`) and
//!   `/proc/<pid>/status` (line `NSpid:`) while it holds a pidfd of the process, and checks afterwards that the same
//!   process still runs (start time before and after, as the holder does in `rust/crates/halter/src/peer.rs`).
//! - The peer counts as enclosed when its cgroup path has a part that starts with `werkbank-lauf` (the unit of a run),
//!   when it runs in a nested pid namespace (`NSpid` with more than one number; the enclosure starts every agent with
//!   its own pid namespace), or when one of the two cannot be read or the process changed: fail closed. The criterion
//!   of the unit is derived; no contract of this state fixes the name of the unit of a run (open point of the result).

use std::os::unix::net::UnixStream;

use rustix::event::{PollFd, PollFlags, poll};
use rustix::process::{Pid, PidfdFlags, pidfd_open};

use crate::helfer::Partner;

/// Start of a part of the cgroup path that marks the unit of a run.
pub const LAUF_EINHEIT: &str = "werkbank-lauf";

/// `Some(true)` if the cgroup v2 line of `/proc/<pid>/cgroup` names the unit of a run, `Some(false)` if not, `None`
/// without a line `0::` with an absolute path.
#[must_use]
pub fn lauf_in_cgroup(datei: &str) -> Option<bool> {
    let pfad = datei.lines().find_map(|z| z.strip_prefix("0::"))?;
    if !pfad.starts_with('/') || pfad.contains('\0') {
        return None;
    }
    Some(pfad.split('/').any(|teil| teil.starts_with(LAUF_EINHEIT)))
}

/// `Some(true)` if the line `NSpid:` of `/proc/<pid>/status` has more than one number (a nested pid namespace),
/// `Some(false)` with exactly one, `None` without the line or with a field that is no number.
#[must_use]
pub fn verschachtelt(status: &str) -> Option<bool> {
    let zeile = status.lines().find_map(|z| z.strip_prefix("NSpid:"))?;
    let mut n = 0usize;
    for feld in zeile.split_whitespace() {
        feld.parse::<u32>().ok()?;
        n += 1;
    }
    (n > 0).then_some(n > 1)
}

/// H2 from the two files; `None` is a file that could not be read. Only two readable files that both say "no" let
/// the peer through.
#[must_use]
pub fn eingeschlossen(cgroup: Option<&str>, status: Option<&str>) -> bool {
    !matches!(
        (
            cgroup.and_then(lauf_in_cgroup),
            status.and_then(verschachtelt)
        ),
        (Some(false), Some(false))
    )
}

fn startzeit(pid: u32) -> Option<u64> {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    // the name in brackets may hold spaces; the fields after the last ')' start with field 3
    let rest = stat.rsplit_once(')')?.1;
    rest.split_whitespace().nth(19)?.parse().ok()
}

fn lebt(pidfd: &std::os::fd::OwnedFd) -> bool {
    let mut fds = [PollFd::new(pidfd, PollFlags::IN)];
    let null = rustix::time::Timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    matches!(poll(&mut fds, Some(&null)), Ok(0))
}

/// Measures the peer of the accepted `strom`.
#[must_use]
pub fn messen(strom: &UnixStream) -> Partner {
    let Ok(cred) = rustix::net::sockopt::socket_peercred(strom) else {
        return Partner {
            uid: None,
            eingeschlossen: true,
        };
    };
    let uid = Some(cred.uid.as_raw());
    let pid_roh = cred.pid.as_raw_nonzero().get();
    let geschlossen = Partner {
        uid,
        eingeschlossen: true,
    };
    let (Ok(pid), Some(p)) = (u32::try_from(pid_roh), Pid::from_raw(pid_roh)) else {
        return geschlossen;
    };
    let Ok(pidfd) = pidfd_open(p, PidfdFlags::empty()) else {
        return geschlossen;
    };
    let Some(vorher) = startzeit(pid) else {
        return geschlossen;
    };
    let cgroup = std::fs::read_to_string(format!("/proc/{pid}/cgroup")).ok();
    let status = std::fs::read_to_string(format!("/proc/{pid}/status")).ok();
    // the same process still runs behind the pid: same start time, and the pidfd shows no end
    if startzeit(pid) != Some(vorher) {
        return geschlossen;
    }
    if !lebt(&pidfd) {
        return geschlossen;
    }
    Partner {
        uid,
        eingeschlossen: eingeschlossen(cgroup.as_deref(), status.as_deref()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cgroup_der_einheit() {
        assert_eq!(
            lauf_in_cgroup("0::/user.slice/user-1001.slice/session-2.scope\n"),
            Some(false)
        );
        assert_eq!(
            // The unit name of the user manager holds an "@"; split, so the export gate does not read it as a mail address.
            lauf_in_cgroup(concat!(
                "0::/user.slice/user-1001.slice/user",
                "@1001.service/app.slice/werkbank-lauf-7.scope\n"
            )),
            Some(true)
        );
        assert_eq!(
            lauf_in_cgroup("0::/system.slice/werkbank-lauf.service"),
            Some(true)
        );
        assert_eq!(
            lauf_in_cgroup("0::/system.slice/x-werkbank-lauf.service"),
            Some(false)
        );
        assert_eq!(
            lauf_in_cgroup("12:pids:/a\n0::/werkbank-lauf-1/b\n"),
            Some(true)
        );
        assert_eq!(
            lauf_in_cgroup("12:pids:/werkbank-lauf\n"),
            None,
            "cgroup v1 only"
        );
        assert_eq!(lauf_in_cgroup("0::relativ"), None);
        assert_eq!(lauf_in_cgroup("0::/a\0b"), None);
        assert_eq!(lauf_in_cgroup(""), None);
        assert_eq!(lauf_in_cgroup("0::/"), Some(false));
    }

    #[test]
    fn nspid() {
        assert_eq!(verschachtelt("Name:\tx\nNSpid:\t4711\n"), Some(false));
        assert_eq!(verschachtelt("NSpid:\t4711\t3\n"), Some(true));
        assert_eq!(verschachtelt("NSpid:\t4711\t3\t1\n"), Some(true));
        assert_eq!(verschachtelt("NSpid:\t\n"), None);
        assert_eq!(verschachtelt("NSpid:\t47x\n"), None);
        assert_eq!(verschachtelt("Pid:\t4711\n"), None);
    }

    #[test]
    fn nur_zwei_mal_nein_laesst_durch() {
        let c = Some("0::/user.slice/a.scope");
        let s = Some("NSpid:\t9\n");
        assert!(!eingeschlossen(c, s));
        assert!(eingeschlossen(Some("0::/werkbank-lauf-1.scope"), s));
        assert!(eingeschlossen(c, Some("NSpid:\t9\t1\n")));
        assert!(eingeschlossen(None, s));
        assert!(eingeschlossen(c, None));
        assert!(eingeschlossen(Some("1:x:/"), s));
        assert!(eingeschlossen(c, Some("")));
    }

    /// A peer that connected and ended, but is not reaped yet: `pidfd_open` and `/proc/<pid>/stat` still work, the pidfd
    /// shows the end. The peer then counts as enclosed (fail closed); its uid stays known.
    #[test]
    fn beendeter_partner_ist_geschlossen() {
        let ort = std::env::temp_dir().join(format!("vh-partner-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&ort);
        std::fs::create_dir(&ort).expect("folder");
        let sock = ort.join("s");
        let hoerer = std::os::unix::net::UnixListener::bind(&sock).expect("bind");
        let mut kind = std::process::Command::new("python3")
            .args([
                "-I",
                "-c",
                "import socket, sys; s = socket.socket(socket.AF_UNIX); s.connect(sys.argv[1])",
            ])
            .arg(&sock)
            .spawn()
            .expect("python3");
        let (strom, _) = hoerer.accept().expect("accept");
        let kind_pid = Pid::from_raw(i32::try_from(kind.id()).expect("pid")).expect("pid");
        let pidfd = pidfd_open(kind_pid, PidfdFlags::empty()).expect("pidfd");
        // wait for the end of the child without reaping it (the pidfd becomes readable)
        let mut fds = [PollFd::new(&pidfd, PollFlags::IN)];
        let frist = rustix::time::Timespec {
            tv_sec: 10,
            tv_nsec: 0,
        };
        assert_eq!(poll(&mut fds, Some(&frist)).expect("poll"), 1);
        assert!(
            startzeit(kind.id()).is_some(),
            "a zombie still has its stat file"
        );
        let p = messen(&strom);
        assert_eq!(p.uid, Some(rustix::process::getuid().as_raw()));
        assert!(p.eingeschlossen, "an ended peer is never let through");
        let _ = kind.wait();
        drop(strom);
        drop(hoerer);
        std::fs::remove_file(&sock).expect("rm socket");
        std::fs::remove_dir(&ort).expect("rm folder");
    }

    /// The own process through a pair of sockets: uid, and the cgroup and pid namespace of the test (a test that runs
    /// inside a unit of a run or in a nested pid namespace would see itself as enclosed).
    #[test]
    fn eigener_prozess() {
        let (a, _b) = UnixStream::pair().expect("pair");
        let p = messen(&a);
        assert_eq!(p.uid, Some(rustix::process::getuid().as_raw()));
        let cgroup = std::fs::read_to_string("/proc/self/cgroup").ok();
        let status = std::fs::read_to_string("/proc/self/status").ok();
        assert_eq!(
            p.eingeschlossen,
            eingeschlossen(cgroup.as_deref(), status.as_deref())
        );
        // the test runs outside a unit of a run (the gate and the enclosure of the mutants start it so)
        assert!(!p.eingeschlossen, "{cgroup:?} {status:?}");
        let start = startzeit(std::process::id());
        assert!(start.is_some_and(|s| s > 0));
        assert_eq!(startzeit(u32::MAX), None);
    }
}
