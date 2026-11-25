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
//! The idle time of clarification 13 (10 s without a byte) holds for the pipe as for the socket: `Verbindung::lesefrist` of
//! `peerkanal` limits each read of an accepted connection ([`bedienen`]).

use std::io::Write;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::thread;
use std::time::Duration;

use peerkanal::{
    Annahme, Aufrufergrund, Ereignis, Erlaubnis, Grund, Kennung, PackageSid, Server, ServerVorgabe,
    Verbindung, Vorlaeufig,
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

/// The start under Windows. Returns an exit code only when the start fails; after the readiness line the process runs until
/// it is ended from outside.
pub fn starten<O: Write, E: Write>(konfig: Konfig, out: &mut O, err: &mut E) -> u8 {
    let einst = match kern::vor_dem_kanal(&konfig, err) {
        Ok(einst) => einst,
        Err(code) => return code,
    };
    let mut server = match pipe_anlegen(&konfig) {
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
    // Decision 4 of `peerkanal`: the line of a refusal is written in the hook, before the library ends the connection, so a
    // client that sees the end finds the line in the chain. Refusals of this process (`aufrufer:*`) have their line `such.*`
    // already.
    let fuer_haken = Arc::clone(&sitzung);
    server.bei_abweisung(move |ereignis: &Ereignis| {
        if !matches!(ereignis.grund, Grund::Aufrufer(_)) {
            fuer_haken.abgewiesen(&ereignis.grund.als_text());
        }
    });
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
    let eigene = peerkanal::eigene_sid().map_err(|fehler| format!("own SID: {fehler}"))?;
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

/// One of the accepting threads. A client that `peerkanal` refused has its log line from the hook of [`starten`], written
/// before `peerkanal` ends the connection; an accepted one goes to a thread of its own, so the next `annehmen` waits at once. A failure of `annehmen` is reported once
/// per run of failures (as `accept` under Unix, review B9).
fn annehmen(server: &Server, sitzung: &Arc<Sitzung>, offen: &Arc<AtomicUsize>) {
    let mut in_folge = false;
    loop {
        match server.annehmen() {
            Ok(Annahme::Abgewiesen(_)) => {
                // The hook wrote the line.
                in_folge = false;
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
    bedienen(verbindung, sitzung, kern::LEERLAUF);
}

/// An accepted connection with the idle time `leerlauf` (clarification 13; [`kern::LEERLAUF`], the seam of the tests): each
/// read waits at most this long for a byte, counted from the last one, and never while the process answers. A read that runs
/// out ends the loop like the end of the input: the connection closes without an answer and without a log line, and the
/// instance is free again.
fn bedienen(mut verbindung: Verbindung, sitzung: &Sitzung, leerlauf: Duration) {
    verbindung.lesefrist(Some(leerlauf));
    kern::bedienen(verbindung, sitzung);
}

/// Real pipes of this process, so these tests run in the Windows VM (test program of the crate). Server and client are the same
/// account; the server allows its own SID without a package SID.
#[cfg(test)]
mod tests {
    use std::fs;
    use std::io::{Read, Write};
    use std::path::Path;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::{Arc, Mutex, mpsc};
    use std::thread;
    use std::time::{Duration, Instant};

    use peerkanal::{
        Annahme, ClientVorgabe, Erlaubnis, Erwartet, Grund, Kennung, Server, ServerVorgabe,
        eigene_sid,
    };

    use super::super::dienst;
    use super::super::einst;
    use super::super::kern::Sitzung;
    use super::super::tor::Tor;
    use crate::serve::Server as Suche;

    fn ordner() -> std::path::PathBuf {
        static N: AtomicU32 = AtomicU32::new(0);
        let n = N.fetch_add(1, Ordering::SeqCst);
        std::env::temp_dir().join(format!("rk-win-{}-{n}", std::process::id()))
    }

    fn sitzung(ordner: &Path) -> Sitzung {
        fs::create_dir_all(ordner.join("vault")).expect("vault");
        fs::create_dir_all(ordner.join("protokoll")).expect("log folder");
        let datei = ordner.join("einst.json");
        fs::write(
            &datei,
            r#"{"fassung":1,"klassen":["oeffentlich","intern"],"vorgabestufe":"intern","routen":[{"schluessel":"cloud","art":"cloud","klasse":"intern"}],"kunden":[],"kundendaten_an_cloud":{"vorgabe":"nie","freigaben":[]}}"#,
        )
        .expect("settings");
        let einst = einst::lesen_datei(&datei).expect("settings are read");
        Sitzung {
            server: Mutex::new(Suche::start(ordner.join("vault"), None)),
            tor: Tor::new(einst.vorgabe, "cloud".to_owned()),
            route: "cloud".to_owned(),
            kennung: "s-1".to_owned(),
            protokoll: ordner.join("protokoll"),
            zaehlung: None,
        }
    }

    fn server(name: &str, hallo_ms: u64) -> Server {
        let mut v = ServerVorgabe::neu(
            Kennung::neu(name).expect("form"),
            vec![Erlaubnis::Windows {
                sid: eigene_sid().expect("own SID"),
                appcontainer_sid: None,
            }],
            2,
        );
        v.hallo_frist = Duration::from_millis(hallo_ms);
        Server::anlegen(v).expect("create")
    }

    /// Clarification 13 for the pipe: a client that answers a ping and then stays silent loses its connection after the idle
    /// time, without an answer; it sees the end. Mutant "Lesefrist aus": the client waits for 5 s and the test fails.
    #[test]
    fn a_silent_client_loses_the_pipe_after_the_idle_time() {
        let o = ordner();
        let _ = fs::remove_dir_all(&o);
        let sitzung = Arc::new(sitzung(&o));
        let name = format!("wb-rk-leer-{}", std::process::id());
        let s = server(&name, 2000);
        let vorgabe = ClientVorgabe {
            name: Kennung::neu(&name).expect("form"),
            erwartet: Erwartet::Windows {
                sid: eigene_sid().expect("own SID"),
            },
            frist: Duration::from_secs(2),
            unix_ordner: None,
        };
        let client = thread::spawn(move || {
            let mut k = peerkanal::verbinden(&vorgabe).expect("connect");
            k.write_all(b"{\"op\":\"ping\",\"id\":1}\n").expect("write");
            k.lesefrist(Some(Duration::from_secs(5)));
            let mut gelesen = Vec::new();
            let mut stueck = [0u8; 256];
            let start = Instant::now();
            let ende = loop {
                match k.read(&mut stueck) {
                    Ok(0) => break Ok(start.elapsed()),
                    Ok(n) => gelesen.extend_from_slice(&stueck[..n]),
                    Err(e) => break Err(e.kind()),
                }
            };
            (String::from_utf8(gelesen).expect("text"), ende)
        });
        let Annahme::Vorlaeufig(vl) = s.annehmen().expect("accept") else {
            panic!("refused");
        };
        let verbindung = vl.bestaetigen().expect("confirm");
        let start = Instant::now();
        super::bedienen(verbindung, &sitzung, Duration::from_millis(400));
        let dauer = start.elapsed();
        let (text, ende) = client.join().expect("thread");
        assert_eq!(text, "{\"id\":1,\"ok\":true}\n");
        assert!(
            ende.is_ok(),
            "the client sees the end, not its own limit: {ende:?}"
        );
        assert!(dauer < Duration::from_secs(3), "{dauer:?}");
        assert!(
            fs::read_dir(o.join("protokoll"))
                .expect("log folder")
                .next()
                .is_none(),
            "no log line"
        );
        drop(s);
        let _ = fs::remove_dir_all(&o);
    }

    /// The question to the platform service: no service is no answer (fail closed); a server under the name of the service but
    /// with another owner (this account) gets no byte and no answer counts (`pk.falscher_server` inside `fragen`).
    #[test]
    fn the_service_is_asked_only_at_its_own_owner() {
        assert_eq!(dienst::fragen(4242), None, "no service");
        let s = Arc::new(server("wb-plattform", 1000));
        let gesehen = Arc::new(Mutex::new(Vec::new()));
        let (tx, rx) = mpsc::channel();
        let (s2, g2) = (Arc::clone(&s), Arc::clone(&gesehen));
        thread::spawn(move || {
            if let Ok(Annahme::Abgewiesen(e)) = s2.annehmen() {
                g2.lock().expect("lock").push(e.grund.clone());
            }
            let _ = tx.send(());
        });
        thread::sleep(Duration::from_millis(300));
        let start = Instant::now();
        assert_eq!(dienst::fragen(4242), None, "wrong owner");
        assert!(start.elapsed() < Duration::from_secs(2));
        rx.recv_timeout(Duration::from_secs(10))
            .expect("accept returned");
        assert_eq!(
            *gesehen.lock().expect("lock"),
            vec![Grund::HalloFehlt],
            "0 bytes reached the false service"
        );
    }
}
