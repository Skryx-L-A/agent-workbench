//! The measurement of the peer under Linux (section 2.3; `rust/spec/VERTRAEGE.md`, "Annehmen und Messen",
//! steps 2, 4 and 5). A narrow interface, so that it can move into the crate `peerkanal` later:
//!
//! - [`messen`] takes the accepted stream and returns a [`Peer`]: `uid`, `gid` and `pid` from `SO_PEERCRED`, a pidfd
//!   bound to the process, `programm` from `/proc/<pid>/exe` and the first line of `/proc/<pid>/cgroup`;
//! - [`Peer::lebt`] says through the pidfd whether the bound process still runs;
//! - [`rolle`] decides the role from the effective gid of the peer and the role groups of `halter.json` (section 2.3,
//!   addendum "Rolle aus der Gruppe"); `programm` is a measured value only and decides nothing.
//!
//! The pidfd comes from `pidfd_open` on the pid of `SO_PEERCRED`, because `SO_PEERPIDFD` is not in rustix 1.1.6 and the
//! workspace forbids `unsafe`. After `readlink` of the program and the read of the cgroup the pidfd must still show a
//! running process, and `/proc/<pid>/stat` must show the same start time before and after; otherwise the measurement
//! fails (`pk.prozess_gewechselt`). The window between `connect` and `pidfd_open` touches only this check of life and
//! the measured values, never the role: the kernel keeps the gid of `SO_PEERCRED` from the time of `connect`.

use std::collections::BTreeMap;
use std::os::fd::OwnedFd;
use std::os::unix::net::UnixStream;
use std::path::PathBuf;

use rustix::event::{PollFd, PollFlags, poll};
use rustix::process::{Pid, PidfdFlags, pidfd_open};

use crate::pruefung::Rolle;

/// The measured peer of one connection.
#[derive(Debug)]
pub struct Peer {
    /// uid of the peer at `connect`.
    pub uid: u32,
    /// Effective gid of the peer at `connect` (`SO_PEERCRED`); the source of the role.
    pub gid: u32,
    /// pid of the peer at `connect`.
    pub pid: u32,
    /// The program of the bound process, `None` if it cannot be read (`PR_SET_DUMPABLE` 0, another uid, gone).
    pub programm: Option<PathBuf>,
    /// The first line of the cgroup file of the process, `None` if it cannot be read.
    pub cgroup: Option<String>,
    pidfd: OwnedFd,
}

/// Why the measurement failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Messfehler {
    /// `SO_PEERCRED` failed (`pk.messung_gescheitert`).
    Messung,
    /// The process could not be bound (`pk.prozess_nicht_offen`).
    NichtOffen,
    /// The bound process ended or changed (`pk.prozess_beendet`, `pk.startzeit_weicht_ab`).
    Gewechselt,
}

fn startzeit(pid: u32) -> Option<u64> {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    // the name in brackets may hold spaces; the fields after the last ')' start with field 3
    let rest = stat.rsplit_once(')')?.1;
    rest.split_whitespace().nth(19)?.parse().ok()
}

impl Peer {
    /// `true` while the process bound by the pidfd runs.
    #[must_use]
    pub fn lebt(&self) -> bool {
        let mut fds = [PollFd::new(&self.pidfd, PollFlags::IN)];
        let zero = rustix::time::Timespec {
            tv_sec: 0,
            tv_nsec: 0,
        };
        matches!(poll(&mut fds, Some(&zero)), Ok(0))
    }
}

/// Steps 2, 4 and 5 for the accepted `strom`.
///
/// # Errors
/// [`Messfehler`] in the order of the steps.
pub fn messen(strom: &UnixStream) -> Result<Peer, Messfehler> {
    let cred = rustix::net::sockopt::socket_peercred(strom).map_err(|_| Messfehler::Messung)?;
    let uid = cred.uid.as_raw();
    let gid = cred.gid.as_raw();
    let pid_raw = cred.pid.as_raw_nonzero().get();
    let pid = u32::try_from(pid_raw).map_err(|_| Messfehler::Messung)?;
    let pidfd = pidfd_open(
        Pid::from_raw(pid_raw).ok_or(Messfehler::Messung)?,
        PidfdFlags::empty(),
    )
    .map_err(|_| Messfehler::NichtOffen)?;
    let vorher = startzeit(pid);
    let programm = std::fs::read_link(format!("/proc/{pid}/exe")).ok();
    let cgroup = std::fs::read_to_string(format!("/proc/{pid}/cgroup"))
        .ok()
        .and_then(|t| t.lines().next().map(str::to_owned));
    let peer = Peer {
        uid,
        gid,
        pid,
        programm,
        cgroup,
        pidfd,
    };
    if !peer.lebt() {
        return Err(Messfehler::Gewechselt);
    }
    if vorher.is_none() || startzeit(pid) != vorher {
        return Err(Messfehler::Gewechselt);
    }
    Ok(peer)
}

/// The role of the peer: the same uid as the holder, and its effective gid is the gid of a role group of `halter.json`
/// (`gruppen`, resolved at the start). `None` is `halter.peer`; there is no fallback on the program, the pid or a
/// supplementary group (R-LAUF-HALTER). The bound process must still run.
#[must_use]
pub fn rolle(peer: &Peer, eigene_uid: u32, gruppen: &BTreeMap<u32, Rolle>) -> Option<Rolle> {
    if peer.uid != eigene_uid {
        return None;
    }
    let rolle = *gruppen.get(&peer.gid)?;
    peer.lebt().then_some(rolle)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eigener_prozess_wird_gemessen() {
        let (a, _b) = UnixStream::pair().expect("pair");
        let p = messen(&a).expect("peer");
        assert_eq!(p.uid, rustix::process::getuid().as_raw());
        assert_eq!(p.gid, rustix::process::getegid().as_raw());
        assert_eq!(p.pid, std::process::id());
        assert!(p.lebt());
        assert_eq!(p.programm, std::env::current_exe().ok());
        assert!(p.cgroup.is_some());
    }

    #[test]
    fn rolle_nur_aus_der_effektiven_gid() {
        let (a, _b) = UnixStream::pair().expect("pair");
        let p = messen(&a).expect("peer");
        let uid = p.uid;
        let gid = p.gid;
        let mit = |g: u32| BTreeMap::from([(g, Rolle::Bruecke)]);
        assert_eq!(rolle(&p, uid, &mit(gid)), Some(Rolle::Bruecke));
        assert_eq!(
            rolle(&p, uid, &mit(gid.wrapping_add(1))),
            None,
            "another gid"
        );
        assert_eq!(rolle(&p, uid, &BTreeMap::new()), None, "no role group");
        assert_eq!(
            rolle(&p, uid.wrapping_add(1), &mit(gid)),
            None,
            "another uid"
        );
        let hub = BTreeMap::from([(gid.wrapping_add(1), Rolle::Bruecke), (gid, Rolle::Hub)]);
        assert_eq!(rolle(&p, uid, &hub), Some(Rolle::Hub));
    }
}
