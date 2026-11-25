//! `rangkern sitzung` under Windows (addendum to contract 4, `rust/spec/VERTRAEGE.md`): the named pipe
//! `\\.\pipe\wb-such-<32 hex>` through the crate `peerkanal`, never through Win32 directly (this crate forbids `unsafe`).
//!
//! The start checks the same things in the same order as under Unix (`kern::vor_dem_kanal`, exit 78), then creates the pipe
//! with the first instance (exit 74 if that fails), then starts the probe run and loads the notes, then writes the readiness
//! line. After it, four threads accept connections; each accepted client is checked by `peerkanal` (hello, SID, package SID,
//! process) and then by the platform service (`dienst`), and only then gets the hello and the answers of `kern`.
//!
//! Windows has no SIGTERM: the process ends with the job of its starter or through `TerminateProcess`, and the pipe goes with
//! its last handle (section 5). There is nothing to remove.
//!
//! Not yet here: the idle time of clarification 13 (10 s without a byte). `peerkanal` (version 1) reads a connection without a
//! time limit and offers no way to set one, so a silent agent holds its instance until it closes it; the number of instances
//! (4) bounds that.

use std::io::Write;
use std::os::windows::fs::OpenOptionsExt;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use peerkanal::{
    Annahme, Aufrufergrund, Erlaubnis, HALLO, Kennung, PackageSid, Server, ServerVorgabe, Sid,
    Vorlaeufig,
};

use super::dienst::{self, Messung};
use super::kern::{self, Platz, Sitzung};
use super::konfig::Konfig;
use super::plattform::kanalname;
use super::{BEREIT, EXIT_CONFIG, EXIT_SOCKET};

/// Connections at the same time (section 3): a fifth client waits (`WaitNamedPipe`) or fails with 231.
pub const INSTANZEN: u8 = 4;

/// The name the search process gives `peerkanal` when its own check turns a client away; it never reaches a log line (the
/// line carries the reason `such.*` instead).
const AUFRUFER: &str = "such-prozess";

/// `GENERIC_READ | FILE_WRITE_DATA`: the access of a client (section 6); `GENERIC_WRITE` would ask for the right to create an
/// instance.
const CLIENT_ZUGRIFF: u32 = 0x8000_0000 | 0x0000_0002;

/// `SECURITY_IDENTIFICATION` (`SecurityIdentification << 16`); `security_qos_flags` adds `SECURITY_SQOS_PRESENT`.
const IDENTIFIKATION: u32 = 0x0001_0000;

/// SYSTEM: the only entry of the list of the short pipe that measures the own SID, so that the own process is turned away and
/// its measured identity comes back in the event.
const SYSTEM: &str = "S-1-5-18";

/// The start under Windows. Returns an exit code only when the start fails; after the readiness line the process runs until
/// it is ended from outside.
pub fn starten<O: Write, E: Write>(konfig: Konfig, out: &mut O, err: &mut E) -> u8 {
    let einst = match kern::vor_dem_kanal(&konfig, err) {
        Ok(einst) => einst,
        Err(code) => return code,
    };
    let server = match pipe_anlegen(&konfig) {
        Ok(server) => server,
        Err(fehler) => {
            let _ = writeln!(err, "rangkern sitzung: pipe: {fehler}");
            return EXIT_SOCKET;
        }
    };
    let zaehlung = match kern::probelauf_starten(&konfig, err) {
        Ok(zaehlung) => zaehlung,
        Err(fehler) => {
            let _ = writeln!(err, "rangkern sitzung: probe run: {fehler}");
            // The pipe goes with the server.
            drop(server);
            return EXIT_CONFIG;
        }
    };
    let sitzung = Arc::new(kern::sitzung_bauen(konfig, einst, zaehlung));
    // The starter may start the harness only after this line. A closed pipe leaves nobody to tell.
    let _ = out.write_all(BEREIT).and_then(|()| out.flush());
    let server = Arc::new(server);
    let offen = Arc::new(AtomicUsize::new(0));
    let faeden: Vec<_> = (0..INSTANZEN)
        .map(|_| {
            let (server, sitzung, offen) = (
                Arc::clone(&server),
                Arc::clone(&sitzung),
                Arc::clone(&offen),
            );
            thread::spawn(move || annehmen(&server, &sitzung, &offen))
        })
        .collect();
    // The loops never end; the process ends from outside.
    for faden in faeden {
        let _ = faden.join();
    }
    EXIT_SOCKET
}

/// Creates the pipe: the list holds exactly the pair of the own SID and the package SID of `--paket-sid`, four instances, the
/// hello within 2 s, the process must be bound, no job (section 3).
///
/// # Errors
/// The text for the error output: the name is taken, the own SID cannot be measured, or the pipe cannot be created.
fn pipe_anlegen(konfig: &Konfig) -> Result<Server, String> {
    let name = konfig
        .socket
        .to_str()
        .and_then(kanalname)
        .ok_or("the pipe name has not the form of the addendum")?;
    let kanal = Kennung::neu(name).map_err(|fehler| fehler.to_string())?;
    let paket = konfig
        .paket_sid
        .as_deref()
        .ok_or("no package SID")
        .and_then(|sid| PackageSid::neu(sid).map_err(|_| "the package SID has not the form"))?;
    let eigene = eigene_sid()?;
    let mut vorgabe = ServerVorgabe::neu(
        kanal,
        vec![Erlaubnis::Windows {
            sid: eigene,
            appcontainer_sid: Some(paket),
        }],
        INSTANZEN,
    );
    vorgabe.prozess_pflicht = true;
    Server::anlegen(vorgabe).map_err(|fehler| fehler.to_string())
}

/// The `TokenUser` SID of this process, measured by the kernel: a short pipe of its own (first instance, a list with SYSTEM
/// only), a client of this process with identification only, and the identity that `peerkanal` measured for the refused
/// client. `peerkanal` (version 1) has no call for the own SID; this way needs no `unsafe` and starts no process. A name that
/// someone else holds already makes the creation fail, so no other server can answer.
///
/// # Errors
/// The short pipe cannot be created or reached, or the measurement names no SID.
fn eigene_sid() -> Result<Sid, String> {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|dauer| dauer.as_nanos())
        .unwrap_or_default();
    let name = format!("wb-sid-{}-{nanos}", std::process::id());
    let kanal = Kennung::neu(&name).map_err(|fehler| fehler.to_string())?;
    let system = Sid::neu(SYSTEM).map_err(|fehler| fehler.to_string())?;
    let vorgabe = ServerVorgabe::neu(
        kanal,
        vec![Erlaubnis::Windows {
            sid: system,
            appcontainer_sid: None,
        }],
        1,
    );
    let server =
        Server::anlegen(vorgabe).map_err(|fehler| format!("own SID: short pipe: {fehler}"))?;
    // The instance exists from `anlegen` on, so the client connects before the server waits; a failed client never leaves
    // `annehmen` waiting.
    let mut client = std::fs::OpenOptions::new()
        .access_mode(CLIENT_ZUGRIFF)
        .security_qos_flags(IDENTIFIKATION)
        .open(format!(r"\\.\pipe\{name}"))
        .map_err(|fehler| format!("own SID: client: {fehler}"))?;
    client
        .write_all(&HALLO)
        .map_err(|fehler| format!("own SID: hello: {fehler}"))?;
    let peer = match server.annehmen() {
        Ok(Annahme::Abgewiesen(ereignis)) => ereignis.peer,
        // Only a process that runs as SYSTEM is on the list.
        Ok(Annahme::Vorlaeufig(vorlaeufig)) => Some(vorlaeufig.peer().clone()),
        Err(fehler) => return Err(format!("own SID: {fehler}")),
    };
    drop(client);
    peer.and_then(|peer| peer.sid)
        .ok_or_else(|| "own SID: the measurement names no SID".to_owned())
        .and_then(|sid| Sid::neu(&sid).map_err(|fehler| fehler.to_string()))
}

/// One of the accepting threads. A client that `peerkanal` refused gets one log line with the reason of `peerkanal`, written
/// after `peerkanal` has closed the connection (version 1 returns the event only then); an
/// accepted one goes to a thread of its own, so the next `annehmen` waits at once. A failure of `annehmen` is reported once
/// per run of failures (as `accept` under Unix, review B9).
fn annehmen(server: &Server, sitzung: &Arc<Sitzung>, offen: &Arc<AtomicUsize>) {
    let mut in_folge = false;
    loop {
        match server.annehmen() {
            Ok(Annahme::Abgewiesen(ereignis)) => {
                in_folge = false;
                sitzung.abgewiesen(&ereignis.grund.als_text());
            }
            Ok(Annahme::Vorlaeufig(vorlaeufig)) => {
                in_folge = false;
                // Over the limit the client is dropped here: closed without a byte and without a line.
                let Some(platz) = Platz::nehmen(offen) else {
                    continue;
                };
                let sitzung = Arc::clone(sitzung);
                thread::spawn(move || {
                    let _platz = platz;
                    verbindung(vorlaeufig, &sitzung);
                });
            }
            Err(fehler) => {
                if !in_folge {
                    let _ = writeln!(std::io::stderr(), "rangkern sitzung: accept: {fehler}");
                }
                in_folge = true;
                thread::sleep(Duration::from_millis(50));
            }
        }
    }
}

/// One client that `peerkanal` let through: the platform service decides (section 4, the last three steps), before any byte
/// goes back. A refusal closes without a byte and writes one line with the reason `such.*`; an acceptance sends the hello
/// and serves the lines.
fn verbindung(vorlaeufig: Vorlaeufig, sitzung: &Sitzung) {
    let messung = vorlaeufig.prozess().map(|prozess| Messung {
        pid: prozess.pid,
        startzeit: prozess.startzeit,
        paket_sid: vorlaeufig.peer().appcontainer_sid.clone(),
    });
    let grund = match &messung {
        Some(messung) => dienst::entscheiden(
            dienst::fragen(messung.pid).as_deref(),
            messung,
            &sitzung.kennung,
        ),
        None => Some(dienst::GRUND_LAUF_UNBEKANNT),
    };
    if let Some(grund) = grund {
        // The line first, then the end of the connection, as under Unix: a client that sees the end finds the line in the
        // chain. Dropped without a decision, the connection closes without an answer as well.
        sitzung.abgewiesen(grund);
        if let Ok(name) = Kennung::neu(AUFRUFER) {
            let _ = vorlaeufig.ablehnen(Aufrufergrund(name));
        }
        return;
    }
    let Ok(verbindung) = vorlaeufig.bestaetigen() else {
        return;
    };
    kern::bedienen(verbindung, sitzung);
}
