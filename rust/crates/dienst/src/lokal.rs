//! The local way for viewers without the bridge (contract `dienst`, DI-L11, Nachtrag 26.11.2026). Security point: the project reads
//! it itself.
//!
//! The server is `peerkanal` version 1 with exactly one permission: the own effective uid without gid (Unix) or the own SID outside
//! a container (Windows, `appcontainer_sid` null); `instanzen` 8, `hallo_frist` 2 s, `prozess_pflicht`, under Unix mode 0600 (point 2). `peerkanal` measures
//! the other side from the kernel and refuses every other account itself. Then the hook of this module (step 7 of `peerkanal`) refuses
//! with `aufrufer:konto-unbekannt` (no name for the uid or SID), under Linux with `aufrufer:einschluss` and under macOS with
//! `aufrufer:sandbox` (point 5). Only after the hook the hello goes back.
//!
//! `wer` (point 4) is built once per connection, before the first frame, from the measured peer alone: never from an event, the
//! start file, the environment, the name of the channel or the hello. A connection without `wer` does not exist: the type that
//! reaches the loop ([`Neu`]) holds it, and every check that cannot decide refuses (point 6, fail closed).
//!
//! Threads: [`ANNEHMER`] threads wait in `Server::annehmen`. For a connection that passed the hook, the thread makes a pair of
//! streams, starts one pump thread for the connection and hands the other end with `wer` to the loop as [`Botschaft::Lokal`] through
//! the wake socket (`weck.rs`). The pump carries the bytes between the connection of `peerkanal` and its end of the pair; the loop
//! treats its end like a connection of the socket of the bridge (the same frames, the same queue, the same numbers `b<n>`).

use std::io::{ErrorKind, Read, Write};
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use peerkanal::{
    Annahme, Aufrufergrund, Code, Ereignis, Fehler, Kanalname, Kennung, Peer, Prozessbindung,
    Server, ServerVorgabe, System, Verbindung, Vorlaeufig,
};
use serde_json::{Value, json};

use crate::dienst::{Gelesen, gelesen};
use crate::fehler::StartFehler;
use crate::sys::UnixStream;
use crate::weck::{Botschaft, Wecker};

/// At most this many connections at the same time (DI-L11 point 2).
pub(crate) const INSTANZEN: u8 = 8;
/// The time the client has for its hello (DI-L11 point 2).
pub(crate) const HALLO_FRIST: Duration = Duration::from_secs(2);
/// How many threads wait for a client at the same time: a client that is slow with its hello holds one of them up to
/// [`HALLO_FRIST`], the other keeps accepting.
const ANNEHMER: usize = 2;
/// How long a thread waits after an error of `annehmen` before it asks again (no busy loop on a broken server).
const NACH_FEHLER: Duration = Duration::from_millis(50);
/// How long the pump waits for bytes of the client before it looks at the bytes of the loop again.
const PUMPE_TAKT: Duration = Duration::from_millis(2);
/// How long the pump waits when the end of the loop takes no more bytes for the moment.
const PUMPE_STAU: Duration = Duration::from_millis(1);
/// The bytes the pump moves in one read.
const PUMPE_PUFFER: usize = 65536;
/// How many reads of the end of the loop one step of the pump takes at most.
const PUMPE_RUNDEN: usize = 16;

/// The reasons of the hook (DI-L11 point 5), each as `aufrufer:<name>` in the line of the protocol.
const KONTO_UNBEKANNT: &str = "konto-unbekannt";
#[cfg(any(target_os = "linux", all(test, unix)))]
const EINSCHLUSS: &str = "einschluss";
#[cfg(all(test, unix))]
const SANDBOX: &str = "sandbox";

/// The account of the other side of a connection of the local way (DI-L11 point 4). It goes into no message to the view, no file and
/// no line of the protocol; only the answer to `Zustand` in test mode shows it ([`Wer::json`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Wer {
    system: &'static str,
    uid: Option<u32>,
    sid: Option<String>,
    name: String,
}

impl Wer {
    /// Unix: the uid and its name `pw_name`.
    #[cfg(unix)]
    pub(crate) fn unix(system: &'static str, uid: u32, name: String) -> Self {
        Self {
            system,
            uid: Some(uid),
            sid: None,
            name,
        }
    }

    /// Windows: the SID and its name `DOMAIN\name`.
    #[cfg_attr(not(any(windows, test)), allow(dead_code))]
    pub(crate) fn windows(sid: String, name: String) -> Self {
        Self {
            system: "windows",
            uid: None,
            sid: Some(sid),
            name,
        }
    }

    /// The form of the answer to `Zustand` (test mode only): `{"system", "uid", "sid", "name"}`.
    pub(crate) fn json(&self) -> Value {
        json!({"system": self.system, "uid": self.uid, "sid": self.sid, "name": self.name})
    }
}

/// A connection of the local way for the loop: its end of the pair and the measured account.
pub(crate) struct Neu {
    /// The end of the loop (non-blocking).
    pub(crate) strom: UnixStream,
    /// The account of the other side.
    pub(crate) wer: Wer,
}

/// The server of the local way.
pub(crate) struct Lokal {
    server: Arc<Server>,
    /// `<kanal>.sock` in the socket folder (Unix): the service removes it at its end. `None` for the pipe of Windows.
    pub(crate) datei: Option<String>,
    /// Set at the end of the service: the threads that wait for a client stop after their next client.
    ende: Arc<AtomicBool>,
    /// The pid of the service, for the check of the enclosure under Linux (a descendant of the service belongs to a run).
    dienst: u32,
}

/// The text of a system as `wer` names it.
#[cfg_attr(not(unix), allow(dead_code))]
fn system_text(system: System) -> &'static str {
    match system {
        System::Linux => "linux",
        System::Macos => "macos",
        System::Windows => "windows",
    }
}

/// The settings of the server (DI-L11 point 2): exactly one permission, the own account.
fn vorgabe(kanal: &str, ordner: &Path) -> Result<ServerVorgabe, StartFehler> {
    let name = Kanalname::neu(kanal).map_err(|_| StartFehler::neu(2, "lokal.kanal"))?;
    let mut v = ServerVorgabe::neu(name, vec![erlaubnis()?], INSTANZEN);
    v.hallo_frist = HALLO_FRIST;
    v.prozess_pflicht = true;
    v.unix = ort(ordner);
    Ok(v)
}

/// The one permission under Unix: the effective uid of the service, without gid.
#[cfg(unix)]
#[allow(clippy::unnecessary_wraps)]
fn erlaubnis() -> Result<peerkanal::Erlaubnis, StartFehler> {
    Ok(peerkanal::Erlaubnis::Unix {
        uid: crate::sys::euid(),
        gid: None,
    })
}

#[cfg(windows)]
use crate::sys::lokal_erlaubnis as erlaubnis;

/// The place of the socket under Unix: the socket folder, mode 0600.
#[cfg(unix)]
#[allow(clippy::unnecessary_wraps)]
fn ort(ordner: &Path) -> Option<peerkanal::UnixOrt> {
    Some(peerkanal::UnixOrt {
        ordner: ordner.to_path_buf(),
        modus: peerkanal::SocketModus::M0600,
    })
}

#[cfg(windows)]
use crate::sys::lokal_ort as ort;

/// The exit code of an error of "Server anlegen" (DI-L11 point 3): a taken name and the rights of the folder are exit code 3 like
/// DI2, a path that does not fit is exit code 2 like DI1, every other error exit code 1 like the socket of the bridge.
fn anlegen_code(f: &Fehler) -> i32 {
    match f.code {
        Code::NameBelegt | Code::OrdnerRechte => 3,
        Code::PfadZuLang => 2,
        _ => 1,
    }
}

/// The line of a refusal (DI-L11 point 5): only the reason, no name, uid or SID.
fn abweisung_zeile(e: &Ereignis) -> String {
    format!("wb-dienst: lokal abgewiesen: {}", e.grund.als_text())
}

impl Lokal {
    /// Creates the server (DI-L11 points 1 to 3). Nothing waits for a client yet ([`Lokal::starten`]).
    ///
    /// # Errors
    /// Exit code 3 when the path is taken or the folder has the wrong rights, 2 for a path that is too long, 1 otherwise.
    pub(crate) fn anlegen(kanal: &str, ordner: &Path) -> Result<Self, StartFehler> {
        let v = vorgabe(kanal, ordner)?;
        let datei = v.unix.as_ref().map(|_| format!("{kanal}.sock"));
        let mut server = Server::anlegen(v).map_err(|f| {
            let text = match f.os_fehler {
                Some(n) => format!("lokal: {} ({n})", f.code.als_str()),
                None => format!("lokal: {}", f.code.als_str()),
            };
            StartFehler::neu(anlegen_code(&f), &text)
        })?;
        server.bei_abweisung(|e| {
            let _ = writeln!(std::io::stderr(), "{}", abweisung_zeile(e));
        });
        Ok(Self {
            server: Arc::new(server),
            datei,
            ende: Arc::new(AtomicBool::new(false)),
            dienst: std::process::id(),
        })
    }

    /// Starts the threads that wait for clients.
    ///
    /// # Errors
    /// Exit code 1 when a thread cannot be started.
    pub(crate) fn starten(&self, wecker: &Wecker) -> Result<(), StartFehler> {
        for n in 0..ANNEHMER {
            let server = Arc::clone(&self.server);
            let ende = Arc::clone(&self.ende);
            let wecker = wecker.clone();
            let dienst = self.dienst;
            std::thread::Builder::new()
                .name(format!("lokal-{n}"))
                .spawn(move || annehmer(&server, &wecker, &ende, dienst))
                .map_err(|e| StartFehler::neu(1, &format!("lokal: thread: {}", e.kind())))?;
        }
        Ok(())
    }

    /// Test only: the pid that counts as the service. The client of a test of the whole way is the test process itself, which the
    /// hook under Linux refuses as the service (or a descendant of it); a pid no process has makes that check pass.
    #[cfg(test)]
    #[cfg(unix)]
    fn mit_dienst(mut self, pid: u32) -> Self {
        self.dienst = pid;
        self
    }

    /// The end of the service: no thread hands a connection to the loop any more.
    pub(crate) fn beenden(&self) {
        self.ende.store(true, Ordering::SeqCst);
    }
}

/// A thread that waits for clients until the end of the service.
fn annehmer(server: &Server, wecker: &Wecker, ende: &AtomicBool, dienst: u32) {
    while !ende.load(Ordering::SeqCst) {
        match server.annehmen() {
            Ok(Annahme::Vorlaeufig(v)) => {
                if ende.load(Ordering::SeqCst) {
                    // dropped without a decision: closed without an answer
                    return;
                }
                aufnehmen(v, wecker, dienst);
            }
            // the hook of `bei_abweisung` wrote the line of the protocol
            Ok(Annahme::Abgewiesen(_)) => {}
            Err(_) => std::thread::sleep(NACH_FEHLER),
        }
    }
}

/// Step 7 for one connection: the hook, then the hello, the pump and the hand-over to the loop. Every step that fails ends the
/// connection; the loop never sees it.
fn aufnehmen(v: Vorlaeufig, wecker: &Wecker, dienst: u32) {
    let wer = match pruefen(v.peer(), v.prozess(), dienst) {
        Ok(wer) => wer,
        Err(grund) => {
            let _ = v.ablehnen(grund);
            return;
        }
    };
    let Ok((schleife, pumpe)) = UnixStream::pair() else {
        return;
    };
    if schleife.set_nonblocking(true).is_err() {
        return;
    }
    if pumpe.set_nonblocking(true).is_err() {
        return;
    }
    let Ok(verbindung) = v.bestaetigen() else {
        return;
    };
    let gestartet = std::thread::Builder::new()
        .name("lokal-pumpe".to_owned())
        .spawn(move || pumpen(verbindung, &pumpe));
    if gestartet.is_err() {
        return;
    }
    // without the loop the message comes back and is dropped: the pump sees the end of its pair and closes the connection
    let _ = wecker.senden(Botschaft::Lokal(Neu {
        strom: schleife,
        wer,
    }));
}

/// The name of a reason of the hook.
fn grund(name: &str) -> Aufrufergrund {
    // the names of this module are in the form of a `Kennung` (checked by the test `gruende_sind_kennungen`)
    Aufrufergrund(Kennung::neu(name).unwrap_or_else(|_| unreachable!("fixed name {name}")))
}

/// The hook (DI-L11 points 4 and 5): `wer` from the measured peer, or the reason of the refusal. The order is the order of the
/// contract: the account, then the enclosure (Linux), then the sandbox (macOS). `dienst` is the pid of the service.
pub(crate) fn pruefen(
    peer: &Peer,
    prozess: Option<&Prozessbindung>,
    dienst: u32,
) -> Result<Wer, Aufrufergrund> {
    let Some(wer) = konto(peer) else {
        return Err(grund(KONTO_UNBEKANNT));
    };
    if let Some(name) = einschluss(prozess, dienst) {
        return Err(grund(name));
    }
    Ok(wer)
}

/// Unix: the measured uid and its name from `getpwuid_r`. No uid, a system that is not this one, no name or an empty name: `None`.
#[cfg(unix)]
fn konto(peer: &Peer) -> Option<Wer> {
    if peer.system != System::aktuell() {
        return None;
    }
    let uid = peer.uid?;
    let name = konto_name(uid)?;
    Some(Wer::unix(system_text(peer.system), uid, name))
}

#[cfg(windows)]
use crate::sys::lokal_konto as konto;

/// `pw_name` of the uid (`getpwuid_r`), `None` when the system knows no account with it.
#[cfg(unix)]
pub(crate) fn konto_name(uid: u32) -> Option<String> {
    let benutzer = nix::unistd::User::from_uid(nix::unistd::Uid::from_raw(uid)).ok()??;
    (!benutzer.name.is_empty()).then_some(benutzer.name)
}

/// The enclosure and the sandbox of the other side: the name of the reason, or `None` when the process may connect.
#[cfg(target_os = "linux")]
fn einschluss(prozess: Option<&Prozessbindung>, dienst: u32) -> Option<&'static str> {
    let Some(p) = prozess else {
        return Some(EINSCHLUSS);
    };
    (!linux::frei(p, dienst)).then_some(EINSCHLUSS)
}

/// macOS: the sandbox of the other side (`aufrufer:sandbox`), in `src/sys/macos.rs` with the call of the system.
#[cfg(target_os = "macos")]
use crate::sys::sandbox_einschluss as einschluss;

/// Windows: `peerkanal` refuses a process in a container (`pk.appcontainer_falsch`) and restricting SIDs (`pk.eingeschraenkt`) itself.
#[cfg(windows)]
use crate::sys::lokal_einschluss as einschluss;

/// Linux (DI-L11 points 5 and 7): a process belongs to a run when it descends from the service (every process of a run is a child
/// of the service or of its children, and the service is their subreaper, section 17) or when it lives in a deeper pid namespace
/// than the service (every enclosure of the crate `einschluss` has its own, `--unshare-pid`, with `--unshare-cgroup`). Both come
/// from files that every process of the account may read (`/proc/<pid>/stat`, `/proc/<pid>/status`); the namespace links of
/// `/proc/<pid>/ns/` would need the right to trace, which a service in a user namespace of its own does not have for a process
/// outside (measured on 27.11.2026: `Permission denied` under `unshare -Ur`). Every value that cannot be read counts as enclosed.
/// The start time of the bound process is checked again after the reads: a pid given to another process in between refuses.
#[cfg(target_os = "linux")]
pub(crate) mod linux {
    use peerkanal::Prozessbindung;

    /// The most steps from a process up to the first process (a chain longer than the space of pids cannot be).
    const KETTE_MAX: usize = 4_194_304;

    /// `true` when the bound process may connect: as deep a pid namespace as the service, no descendant of `eigen`, and the same
    /// start time after the reads.
    pub(crate) fn frei(p: &Prozessbindung, eigen: u32) -> bool {
        let Some(startzeit) = p.startzeit else {
            return false;
        };
        gleicher_pid_raum(p.pid)
            && !nachfahre(p.pid, eigen)
            && startzeit_von(p.pid) == Some(startzeit)
    }

    /// The number of pid namespaces of a process, seen from the `/proc` of the service: the entries of the line `NSpid:` of
    /// `/proc/<pid>/status` (one per namespace, from this one down to the namespace of the process).
    fn pid_tiefe(wer: &str) -> Option<usize> {
        tiefe_aus(&std::fs::read_to_string(format!("/proc/{wer}/status")).ok()?)
    }

    /// The entries of the line `NSpid:` of a status file; `None` without the line or with an empty one.
    pub(crate) fn tiefe_aus(status: &str) -> Option<usize> {
        let zeile = status.lines().find_map(|z| z.strip_prefix("NSpid:"))?;
        let n = zeile.split_whitespace().count();
        (n > 0).then_some(n)
    }

    /// `pid` lives in a pid namespace as deep as the one of this process (an enclosure has one more).
    pub(crate) fn gleicher_pid_raum(pid: u32) -> bool {
        match (pid_tiefe(&pid.to_string()), pid_tiefe("self")) {
            (Some(er), Some(ich)) => er == ich,
            _ => false,
        }
    }

    /// Field 4 (the parent) and field 22 (the start time) of `/proc/<pid>/stat`, read after the last `)`, since the name of the
    /// program may hold spaces and brackets.
    fn stat_felder(pid: u32) -> Option<(u32, u64)> {
        let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
        let (_, rest) = stat.rsplit_once(')')?;
        let felder: Vec<&str> = rest.split_whitespace().collect();
        // after the name: field 3 is the first, so field 4 has the index 1 and field 22 the index 19
        let eltern = felder.get(1)?.parse().ok()?;
        let beginn = felder.get(19)?.parse().ok()?;
        Some((eltern, beginn))
    }

    /// The start time of `pid` (field 22 of `/proc/<pid>/stat`).
    pub(crate) fn startzeit_von(pid: u32) -> Option<u64> {
        stat_felder(pid).map(|(_, s)| s)
    }

    /// `true` when `eigen` is `pid` or one of its ancestors; also when the chain cannot be read to its end (fail closed).
    pub(crate) fn nachfahre(pid: u32, eigen: u32) -> bool {
        let mut jetzt = pid;
        for _ in 0..KETTE_MAX {
            if jetzt == eigen {
                return true;
            }
            // the parent of the first process (and of the threads of the kernel) is 0: the top of the chain
            if jetzt == 0 {
                return false;
            }
            match stat_felder(jetzt) {
                Some((eltern, _)) => jetzt = eltern,
                None => return true,
            }
        }
        true
    }
}

/// The pump of one connection: the bytes of the client go to the end of the pair, the bytes of the loop to the client, until one side
/// ends. Then both are closed.
fn pumpen(mut verbindung: Verbindung, pumpe: &UnixStream) {
    verbindung.lesefrist(Some(PUMPE_TAKT));
    let mut puffer = vec![0_u8; PUMPE_PUFFER];
    while pumpe_schritt(&mut verbindung, pumpe, &mut puffer) {}
    let _ = pumpe.shutdown(std::net::Shutdown::Both);
}

/// One step of the pump: first what the loop has written (at most [`PUMPE_RUNDEN`] reads, so that the client gets its turn), then
/// what the client sends within [`PUMPE_TAKT`]. `false` at the end.
fn pumpe_schritt(client: &mut (impl Read + Write), pumpe: &UnixStream, puffer: &mut [u8]) -> bool {
    for _ in 0..PUMPE_RUNDEN {
        match gelesen((&*pumpe).read(puffer)) {
            Gelesen::Daten(n) => {
                if client.write_all(&puffer[..n]).is_err() {
                    return false;
                }
            }
            Gelesen::Nochmal => break,
            Gelesen::Ende => return false,
        }
    }
    match pumpe_gelesen(client.read(puffer)) {
        Gelesen::Daten(n) => alles_an(pumpe, &puffer[..n]),
        Gelesen::Nochmal => true,
        Gelesen::Ende => false,
    }
}

/// A read of the client: the time limit of `peerkanal` (`TimedOut`) means "nothing now", like `WouldBlock` for the loop.
fn pumpe_gelesen(ergebnis: std::io::Result<usize>) -> Gelesen {
    match ergebnis {
        Err(e) if e.kind() == ErrorKind::TimedOut => Gelesen::Nochmal,
        anderes => gelesen(anderes),
    }
}

/// What a write into the non-blocking end of the pair means for the pump.
#[derive(Debug, PartialEq, Eq)]
enum Geschrieben {
    /// This many bytes went in.
    Teil(usize),
    /// Full or interrupted: once more after a moment.
    Warten,
    /// The end is closed or broken.
    Ende,
}

fn geschrieben(ergebnis: std::io::Result<usize>) -> Geschrieben {
    match ergebnis {
        Ok(0) => Geschrieben::Ende,
        Ok(n) => Geschrieben::Teil(n),
        Err(e) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::Interrupted) => {
            Geschrieben::Warten
        }
        Err(_) => Geschrieben::Ende,
    }
}

/// Writes all of `daten` into the non-blocking end of the pair; a full end is waited for. `false` when the end is closed.
fn alles_an(pumpe: &UnixStream, mut daten: &[u8]) -> bool {
    while !daten.is_empty() {
        match geschrieben((&*pumpe).write(daten)) {
            Geschrieben::Teil(n) => daten = &daten[n..],
            Geschrieben::Warten => std::thread::sleep(PUMPE_STAU),
            Geschrieben::Ende => return false,
        }
    }
    true
}

// `cfg(test)` on its own line: cargo-mutants leaves a module with it out, but not one with `cfg(all(test, ..))`
#[cfg(test)]
#[cfg(unix)]
mod tests {
    use super::*;
    use peerkanal::Erlaubnis;
    use std::os::unix::fs::PermissionsExt;
    use std::os::unix::net::UnixStream as StdStream;
    use std::path::PathBuf;
    use std::time::Instant;

    fn eigen() -> u32 {
        crate::sys::euid()
    }

    fn peer_mit(uid: Option<u32>) -> Peer {
        let mut p = Peer::leer(System::aktuell());
        p.uid = uid;
        p
    }

    /// The parent of the test process: an own process that does not descend from this one.
    fn eltern() -> Prozessbindung {
        let pid = std::os::unix::process::parent_id();
        Prozessbindung {
            pid,
            startzeit: startzeit(pid),
        }
    }

    #[cfg(target_os = "linux")]
    fn startzeit(pid: u32) -> Option<u64> {
        linux::startzeit_von(pid)
    }

    #[cfg(not(target_os = "linux"))]
    fn startzeit(_pid: u32) -> Option<u64> {
        None
    }

    fn grund_von(r: Result<Wer, Aufrufergrund>) -> String {
        match r {
            Err(g) => g.0.als_str().to_owned(),
            Ok(w) => panic!("accepted: {w:?}"),
        }
    }

    /// The reason of the enclosure on this system.
    fn eingeschlossen() -> &'static str {
        if cfg!(target_os = "macos") {
            SANDBOX
        } else {
            EINSCHLUSS
        }
    }

    /// The name of the own account as the system program `id` says it (not through this module).
    fn name_aus_id() -> String {
        let aus = std::process::Command::new("id")
            .arg("-un")
            .output()
            .expect("id");
        String::from_utf8(aus.stdout)
            .expect("UTF-8")
            .trim()
            .to_owned()
    }

    /// A stream that never waits longer than 5 s for a read (a mutant that drops bytes fails, it does not hang).
    fn mit_frist(s: &StdStream) {
        s.set_read_timeout(Some(Duration::from_secs(5)))
            .expect("timeout");
    }

    /// Steps the pump until it ends, at most 5 s; `true` when it ended. Another test thread that starts a child holds a copy of
    /// each socket of this process until its `exec`, so the end of a closed peer can show a step later. The pump of the service
    /// steps in a loop, so this is the behaviour it relies on; a pump that never ends still fails here.
    fn endet_bald(mut schritt: impl FnMut() -> bool) -> bool {
        let ende = Instant::now() + Duration::from_secs(5);
        while Instant::now() < ende {
            if !schritt() {
                return true;
            }
        }
        false
    }

    /// Ends a child of a test and collects it.
    fn beenden(kind: &mut std::process::Child) {
        let _ = kind.kill();
        let _ = kind.wait();
    }

    /// Every reason of the hook is in the form of a `Kennung`, so `aufrufer:<name>` is a valid reason of `peerkanal`.
    #[test]
    fn gruende_sind_kennungen() {
        for name in [KONTO_UNBEKANNT, EINSCHLUSS, SANDBOX] {
            assert!(Kennung::neu(name).is_ok(), "{name}");
            assert_eq!(grund(name).0.als_str(), name);
        }
    }

    /// `wer` has exactly the four fields of DI-L11 point 4, the other kind of account `null`.
    #[test]
    fn wer_in_der_form_des_vertrags() {
        assert_eq!(
            Wer::unix("linux", 1000, "anna".to_owned()).json(),
            json!({"system": "linux", "uid": 1000, "sid": null, "name": "anna"})
        );
        assert_eq!(
            Wer::windows("S-1-5-21-1-2-3-1001".to_owned(), "PC\\anna".to_owned()).json(),
            json!({"system": "windows", "uid": null, "sid": "S-1-5-21-1-2-3-1001", "name": "PC\\anna"})
        );
        assert_eq!(system_text(System::Linux), "linux");
        assert_eq!(system_text(System::Macos), "macos");
        assert_eq!(system_text(System::Windows), "windows");
    }

    /// DI-L11 point 2: exactly one permission, the own effective uid without gid; 8 instances, 2 s for the hello, the process is
    /// mandatory, the socket in the socket folder with mode 0600.
    #[test]
    fn genau_eine_erlaubnis_das_eigene_konto() {
        let v = vorgabe("fenster", Path::new("/tmp/beispiel")).expect("settings");
        assert_eq!(v.name.als_str(), "fenster");
        assert_eq!(
            v.erlaubt,
            vec![Erlaubnis::Unix {
                uid: eigen(),
                gid: None
            }]
        );
        assert_eq!(v.instanzen, 8);
        assert_eq!(v.hallo_frist, Duration::from_secs(2));
        assert!(v.prozess_pflicht);
        assert!(v.job.is_none());
        let ort = v.unix.expect("Unix part");
        assert_eq!(ort.ordner, PathBuf::from("/tmp/beispiel"));
        assert_eq!(ort.modus, peerkanal::SocketModus::M0600);
        assert!(vorgabe("Fenster", Path::new("/tmp/beispiel")).is_err());
    }

    /// DI-L11 point 3: a taken name and the rights of the folder are exit code 3, a path that does not fit 2, anything else 1.
    #[test]
    fn exitcodes_beim_anlegen() {
        let mit = |code| Fehler {
            code,
            os_fehler: None,
        };
        assert_eq!(anlegen_code(&mit(Code::NameBelegt)), 3);
        assert_eq!(anlegen_code(&mit(Code::OrdnerRechte)), 3);
        assert_eq!(anlegen_code(&mit(Code::PfadZuLang)), 2);
        assert_eq!(anlegen_code(&mit(Code::AnlegenFehlgeschlagen)), 1);
        assert_eq!(anlegen_code(&mit(Code::VorgabeUngueltig)), 1);
    }

    /// DI-L11 point 5: the line of a refusal names only the reason, never a name, uid or pid.
    #[test]
    fn zeile_der_abweisung_ohne_konto() {
        let mut peer = peer_mit(Some(4321));
        peer.pid = Some(9876);
        let e = Ereignis {
            kanal: Kennung::neu("fenster").expect("name"),
            grund: peerkanal::Grund::HalloFalsch,
            peer: Some(peer),
        };
        assert_eq!(
            abweisung_zeile(&e),
            "wb-dienst: lokal abgewiesen: pk.hallo_falsch"
        );
        let e = Ereignis {
            kanal: Kennung::neu("fenster").expect("name"),
            grund: peerkanal::Grund::Aufrufer(Kennung::neu(KONTO_UNBEKANNT).expect("name")),
            peer: None,
        };
        assert_eq!(
            abweisung_zeile(&e),
            "wb-dienst: lokal abgewiesen: aufrufer:konto-unbekannt"
        );
    }

    /// The hook with the own account and an own process that is not enclosed: `wer` is the uid with its name, measured, never from
    /// the environment.
    #[test]
    fn eigenes_konto_kommt_durch() {
        let p = eltern();
        let wer =
            pruefen(&peer_mit(Some(eigen())), Some(&p), std::process::id()).expect("own account");
        assert_eq!(
            wer,
            Wer::unix(system_text(System::aktuell()), eigen(), name_aus_id())
        );
        assert_eq!(konto_name(eigen()), Some(name_aus_id()));
    }

    /// The hook with foreign values (DI-L11 points 5 to 7): no uid, a uid the system does not know, a peer of another system:
    /// `aufrufer:konto-unbekannt`, before any check of the process.
    #[test]
    fn fremde_werte_sind_konto_unbekannt() {
        let p = eltern();
        assert_eq!(
            grund_von(pruefen(&peer_mit(None), Some(&p), std::process::id())),
            KONTO_UNBEKANNT
        );
        assert_eq!(
            grund_von(pruefen(
                &peer_mit(Some(3_999_999_999)),
                Some(&p),
                std::process::id()
            )),
            KONTO_UNBEKANNT
        );
        let mut anderes = Peer::leer(System::Windows);
        anderes.uid = Some(eigen());
        anderes.sid = Some("S-1-5-18".to_owned());
        assert_eq!(
            grund_von(pruefen(&anderes, Some(&p), std::process::id())),
            KONTO_UNBEKANNT
        );
        // the account comes first: without uid and without process the reason is the account
        assert_eq!(
            grund_von(pruefen(&peer_mit(None), None, std::process::id())),
            KONTO_UNBEKANNT
        );
        assert_eq!(konto_name(3_999_999_999), None);
    }

    /// Fail closed: without a bound process the hook refuses as enclosed.
    #[test]
    fn ohne_prozess_eingeschlossen() {
        assert_eq!(
            grund_von(pruefen(&peer_mit(Some(eigen())), None, std::process::id())),
            eingeschlossen()
        );
    }

    /// A process that no longer exists (a pid no process has) is refused.
    #[test]
    fn ein_prozess_ohne_messung_wird_abgewiesen() {
        let p = Prozessbindung {
            pid: 99_999_999,
            startzeit: Some(1),
        };
        assert_eq!(
            grund_von(pruefen(
                &peer_mit(Some(eigen())),
                Some(&p),
                std::process::id()
            )),
            eingeschlossen()
        );
    }

    /// A child of the service is part of a run (Linux) and is refused, the service itself too.
    #[cfg(target_os = "linux")]
    #[test]
    fn linux_nachfahre_und_eigener_prozess_eingeschlossen() {
        let eigen_pid = std::process::id();
        let selbst = Prozessbindung {
            pid: eigen_pid,
            startzeit: linux::startzeit_von(eigen_pid),
        };
        assert_eq!(
            grund_von(pruefen(
                &peer_mit(Some(eigen())),
                Some(&selbst),
                std::process::id()
            )),
            EINSCHLUSS
        );
        let mut kind = std::process::Command::new("sleep")
            .arg("30")
            .spawn()
            .expect("child");
        let p = Prozessbindung {
            pid: kind.id(),
            startzeit: linux::startzeit_von(kind.id()),
        };
        assert!(linux::nachfahre(kind.id(), eigen_pid));
        assert!(linux::gleicher_pid_raum(kind.id()));
        assert_eq!(
            grund_von(pruefen(
                &peer_mit(Some(eigen())),
                Some(&p),
                std::process::id()
            )),
            EINSCHLUSS
        );
        beenden(&mut kind);
        // the parent is no descendant; the chain ends at the first process
        assert!(!linux::nachfahre(
            std::os::unix::process::parent_id(),
            eigen_pid
        ));
        assert!(!linux::nachfahre(1, eigen_pid));
        // a pid without a process: the chain cannot be read, fail closed
        assert!(linux::nachfahre(99_999_999, eigen_pid));
    }

    /// Linux: another start time than the one bound (a pid that went to another process) is refused; a missing start time too.
    #[cfg(target_os = "linux")]
    #[test]
    fn linux_andere_startzeit_eingeschlossen() {
        let p = eltern();
        let start = p.startzeit.expect("start time of the parent");
        assert!(linux::frei(&p, std::process::id()));
        for falsch in [Some(start + 1), Some(start - 1), None] {
            let q = Prozessbindung {
                pid: p.pid,
                startzeit: falsch,
            };
            assert!(!linux::frei(&q, std::process::id()), "{falsch:?}");
        }
        // the parent seen as the service: the bound process is the service itself
        assert!(!linux::frei(&p, p.pid));
    }

    /// Linux: a process in a deeper pid namespace (as every enclosure of `einschluss`) is refused, also when it does not descend from
    /// the service (the own parent stands for the service here, so the check of the namespace decides alone).
    #[cfg(target_os = "linux")]
    #[test]
    fn linux_tieferer_pid_raum() {
        // `unshare` forks the child into the new namespace; `--kill-child` ends it with `unshare`
        let mut kind = std::process::Command::new("unshare")
            .args(["-Ur", "--pid", "--fork", "--kill-child", "sleep", "30"])
            .spawn()
            .expect("unshare");
        let u = kind.id();
        let ende = Instant::now() + Duration::from_secs(5);
        let mut innen = None;
        while innen.is_none() && Instant::now() < ende {
            innen = std::fs::read_to_string(format!("/proc/{u}/task/{u}/children"))
                .ok()
                .and_then(|t| {
                    t.split_whitespace()
                        .next()
                        .and_then(|p| p.parse::<u32>().ok())
                });
            std::thread::sleep(Duration::from_millis(10));
        }
        let pid = innen.expect("the child of unshare");
        assert!(!linux::gleicher_pid_raum(pid));
        let p = Prozessbindung {
            pid,
            startzeit: linux::startzeit_von(pid),
        };
        assert!(!linux::frei(&p, std::os::unix::process::parent_id()));
        // `unshare` itself stays in the namespace of the test
        assert!(linux::gleicher_pid_raum(u));
        beenden(&mut kind);
        assert!(!linux::gleicher_pid_raum(99_999_999));
        assert!(linux::gleicher_pid_raum(std::process::id()));
    }

    /// Linux: the depth of the pid namespace from the line `NSpid:`; an empty or missing line has none (fail closed).
    #[cfg(target_os = "linux")]
    #[test]
    fn linux_tiefe_aus_der_statusdatei() {
        assert_eq!(linux::tiefe_aus("Name:\tx\nNSpid:\t4711\n"), Some(1));
        assert_eq!(linux::tiefe_aus("NSpid:\t4711\t1\nPPid:\t1\n"), Some(2));
        assert_eq!(linux::tiefe_aus("NSpid:\t\n"), None);
        assert_eq!(linux::tiefe_aus("NSpid:"), None);
        assert_eq!(linux::tiefe_aus("Pid:\t4711\n"), None);
    }

    /// macOS: a process under the sandbox is refused with `aufrufer:sandbox`.
    #[cfg(target_os = "macos")]
    #[test]
    fn macos_sandbox_abgewiesen() {
        let mut kind = std::process::Command::new("/usr/bin/sandbox-exec")
            .args(["-p", "(version 1)(allow default)", "/bin/sleep", "30"])
            .spawn()
            .expect("sandbox-exec");
        let pid = kind.id();
        let ende = Instant::now() + Duration::from_secs(5);
        while crate::sys::sandkasten(pid) != Some(true) && Instant::now() < ende {
            std::thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(
            crate::sys::sandkasten(pid),
            Some(true),
            "the system says: sandbox"
        );
        let p = Prozessbindung {
            pid,
            startzeit: None,
        };
        assert_eq!(
            grund_von(pruefen(
                &peer_mit(Some(eigen())),
                Some(&p),
                std::process::id()
            )),
            SANDBOX
        );
        beenden(&mut kind);
    }

    /// The pump carries both directions and ends when the loop closes its end, and when the client closes.
    #[test]
    fn pumpe_traegt_beide_richtungen_und_endet() {
        let (mut client_innen, mut client_aussen) = StdStream::pair().expect("pair");
        client_innen
            .set_read_timeout(Some(PUMPE_TAKT))
            .expect("timeout");
        mit_frist(&client_aussen);
        let (schleife, pumpe) = UnixStream::pair().expect("pair");
        mit_frist(&schleife);
        pumpe.set_nonblocking(true).expect("non-blocking");
        let mut puffer = vec![0_u8; 16];
        // nothing anywhere: the step goes on
        assert!(pumpe_schritt(&mut client_innen, &pumpe, &mut puffer));
        // the client sends: the bytes reach the end of the loop
        client_aussen.write_all(b"vom client").expect("write");
        assert!(pumpe_schritt(&mut client_innen, &pumpe, &mut puffer));
        let mut gelesen = [0_u8; 10];
        (&schleife).read_exact(&mut gelesen).expect("read");
        assert_eq!(&gelesen, b"vom client");
        // the loop writes more than the buffer: the client gets all of it in one step
        let lang: Vec<u8> = (0..40_u8).collect();
        (&schleife).write_all(&lang).expect("write");
        assert!(pumpe_schritt(&mut client_innen, &pumpe, &mut puffer));
        let mut beim_client = vec![0_u8; 40];
        client_aussen.read_exact(&mut beim_client).expect("read");
        assert_eq!(beim_client, lang);
        // more than the reads of one step: the rest comes with the next step, in order
        let mehr: Vec<u8> = (0..=255_u8).cycle().take(16 * (PUMPE_RUNDEN + 2)).collect();
        (&schleife).write_all(&mehr).expect("write");
        assert!(pumpe_schritt(&mut client_innen, &pumpe, &mut puffer));
        let mut erster = vec![0_u8; 16 * PUMPE_RUNDEN];
        client_aussen.read_exact(&mut erster).expect("read");
        assert_eq!(erster, mehr[..16 * PUMPE_RUNDEN]);
        client_aussen
            .set_read_timeout(Some(Duration::from_millis(50)))
            .expect("timeout");
        let mut nichts = [0_u8; 1];
        assert!(
            client_aussen.read(&mut nichts).is_err(),
            "one step takes at most its reads"
        );
        mit_frist(&client_aussen);
        assert!(pumpe_schritt(&mut client_innen, &pumpe, &mut puffer));
        let mut rest = vec![0_u8; 32];
        client_aussen.read_exact(&mut rest).expect("read");
        assert_eq!(rest, mehr[16 * PUMPE_RUNDEN..]);
        // the loop closes its end: the pump ends
        drop(schleife);
        assert!(endet_bald(|| pumpe_schritt(
            &mut client_innen,
            &pumpe,
            &mut puffer
        )));
        // the client closes: the pump ends
        let (mut innen2, aussen2) = StdStream::pair().expect("pair");
        innen2.set_read_timeout(Some(PUMPE_TAKT)).expect("timeout");
        let (schleife2, pumpe2) = UnixStream::pair().expect("pair");
        pumpe2.set_nonblocking(true).expect("non-blocking");
        drop(aussen2);
        assert!(endet_bald(|| pumpe_schritt(
            &mut innen2,
            &pumpe2,
            &mut puffer
        )));
        drop(schleife2);
    }

    /// The classes of reads and writes of the pump: the time limit of the client is "nothing now", every other error the end.
    #[test]
    fn lesen_und_schreiben_der_pumpe() {
        let fehler = |k: ErrorKind| Err(std::io::Error::from(k));
        assert_eq!(pumpe_gelesen(Ok(3)), Gelesen::Daten(3));
        assert_eq!(pumpe_gelesen(Ok(0)), Gelesen::Ende);
        for k in [
            ErrorKind::TimedOut,
            ErrorKind::WouldBlock,
            ErrorKind::Interrupted,
        ] {
            assert_eq!(pumpe_gelesen(fehler(k)), Gelesen::Nochmal, "{k:?}");
        }
        for k in [
            ErrorKind::BrokenPipe,
            ErrorKind::ConnectionReset,
            ErrorKind::Other,
        ] {
            assert_eq!(pumpe_gelesen(fehler(k)), Gelesen::Ende, "{k:?}");
        }
        assert_eq!(geschrieben(Ok(5)), Geschrieben::Teil(5));
        assert_eq!(geschrieben(Ok(0)), Geschrieben::Ende);
        for k in [ErrorKind::WouldBlock, ErrorKind::Interrupted] {
            assert_eq!(geschrieben(fehler(k)), Geschrieben::Warten, "{k:?}");
        }
        for k in [ErrorKind::BrokenPipe, ErrorKind::TimedOut, ErrorKind::Other] {
            assert_eq!(geschrieben(fehler(k)), Geschrieben::Ende, "{k:?}");
        }
    }

    /// A full end of the loop is waited for: everything arrives, in order.
    #[test]
    fn alles_an_wartet() {
        const VIEL: usize = 4 * 1024 * 1024;
        let (schleife, pumpe) = UnixStream::pair().expect("pair");
        pumpe.set_nonblocking(true).expect("non-blocking");
        let viel = vec![7_u8; VIEL];
        mit_frist(&schleife);
        let leser = std::thread::spawn(move || {
            let mut n = 0;
            let mut p = vec![0_u8; 65536];
            while n < VIEL {
                match (&schleife).read(&mut p) {
                    Ok(0) | Err(_) => break,
                    Ok(k) => n += k,
                }
            }
            n
        });
        assert!(alles_an(&pumpe, &viel));
        assert_eq!(leser.join().expect("reader"), VIEL);
        assert!(alles_an(&pumpe, b""));
    }

    /// A folder with mode 0700 below `/tmp` (short, so that the path fits into `sun_path`).
    struct Ordner(PathBuf);

    impl Ordner {
        fn neu(name: &str) -> Self {
            let p = PathBuf::from(format!("/tmp/wbl-{}-{name}", std::process::id()));
            let _ = std::fs::remove_dir_all(&p);
            std::fs::create_dir(&p).expect("folder");
            std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o700)).expect("mode");
            Self(p)
        }
    }

    impl Drop for Ordner {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn hallo(strom: &mut StdStream, hallo: &[u8]) -> Vec<u8> {
        strom
            .set_read_timeout(Some(Duration::from_secs(5)))
            .expect("timeout");
        strom.write_all(hallo).expect("hello");
        let mut antwort = Vec::new();
        let mut p = [0_u8; 4];
        while antwort.len() < 4 {
            match strom.read(&mut p) {
                Ok(0) | Err(_) => break,
                Ok(n) => antwort.extend_from_slice(&p[..n]),
            }
        }
        antwort
    }

    /// The whole way with the server of `peerkanal` (needs its Unix side): the socket with mode 0600, the hello, `wer` of the own
    /// account to the loop through the wake socket, the bytes in both directions; a wrong hello gets no connection and no message;
    /// a taken name is exit code 3 and leaves the entry.
    #[test]
    fn der_ganze_weg() {
        let o = Ordner::neu("weg");
        // the client is this test process: as the service counts a pid no process has (the check of descendants has its own tests)
        let lokal = Lokal::anlegen("fenster", &o.0)
            .expect("server")
            .mit_dienst(u32::MAX);
        let pfad = o.0.join("fenster.sock");
        assert_eq!(lokal.datei.as_deref(), Some("fenster.sock"));
        let m = std::fs::symlink_metadata(&pfad).expect("socket");
        assert_eq!(m.permissions().mode() & 0o7777, 0o600);
        let belegt = Lokal::anlegen("fenster", &o.0).err().expect("taken");
        assert_eq!(belegt.code, 3);
        assert!(std::fs::symlink_metadata(&pfad).is_ok());
        let (wecker, mut post, _schreib) = crate::weck::paar().expect("wake socket");
        lokal.starten(&wecker).expect("threads");
        // a wrong hello: no answer, no message
        let mut falsch = StdStream::connect(&pfad).expect("connect");
        assert_eq!(hallo(&mut falsch, b"pk2\n"), Vec::<u8>::new());
        // the right hello
        let mut client = StdStream::connect(&pfad).expect("connect");
        assert_eq!(
            hallo(&mut client, &peerkanal::HALLO),
            peerkanal::HALLO.to_vec()
        );
        let ende = Instant::now() + Duration::from_secs(5);
        let mut neu = Vec::new();
        while neu.is_empty() && Instant::now() < ende {
            neu = post.abholen();
            std::thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(neu.len(), 1, "exactly the one right connection");
        let Some(Botschaft::Lokal(n)) = neu.pop() else {
            panic!("a message of the local way");
        };
        assert_eq!(
            n.wer,
            Wer::unix(system_text(System::aktuell()), eigen(), name_aus_id())
        );
        client.write_all(b"abc").expect("write");
        n.strom.set_nonblocking(false).expect("blocking");
        mit_frist(&n.strom);
        let mut drei = [0_u8; 3];
        (&n.strom).read_exact(&mut drei).expect("read");
        assert_eq!(&drei, b"abc");
        (&n.strom).write_all(b"xyz").expect("write");
        client.read_exact(&mut drei).expect("read");
        assert_eq!(&drei, b"xyz");
        // the loop closes: the client sees the end
        drop(n);
        let mut rest = [0_u8; 1];
        assert_eq!(client.read(&mut rest).ok(), Some(0));
        // after the end no thread hands a connection over: the hello gets no answer (each of the waiting threads takes one client)
        lokal.beenden();
        for _ in 0..ANNEHMER {
            let mut spaet = StdStream::connect(&pfad).expect("connect");
            assert_eq!(hallo(&mut spaet, &peerkanal::HALLO), Vec::<u8>::new());
        }
        assert!(post.abholen().is_empty());
    }
}
