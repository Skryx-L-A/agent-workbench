//! The run of `rangkern sitzung`: the start in its fixed order, the socket, the connections and the end on a signal.

use std::collections::HashMap;
use std::fs;
use std::io::{BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use formate::kette::{
    Abgewiesen, Gesperrt, Probelauf, abschnitt, anhaengen_abgewiesen, anhaengen_gesperrt,
    anhaengen_probelauf,
};
use rustix::fs::{FileType, Mode, RawMode};
use serde_json::{Value, json};
use signal_hook::consts::{SIGINT, SIGTERM};
use signal_hook::iterator::Signals;

use super::anfrage::{self, Op};
use super::einst;
use super::konfig::{Konfig, Probelauf as ProbeKonfig, Vektoren};
use super::loeschen::{Eigene, Ordner};
use super::mac::{self, Plattform};
use super::probelauf::{STDERR_ZEILE, Zaehlung};
use super::tor::{Gruppe, Tor};
use super::{BEREIT, EXIT_CONFIG, EXIT_SOCKET};
use crate::cli::hit_json;
use crate::serve::{Line, SearchRequest, Server, bad_request, next_line};
use crate::stamp::now_ns;
use crate::vault::contradiction_pairs;

/// The socket while the process owns it (its folder, name and identity), for the end on a signal.
type SocketPfad = Arc<Mutex<Option<Eigene>>>;

/// Everything a connection needs to answer.
struct Sitzung {
    server: Mutex<Server>,
    tor: Tor,
    route: String,
    kennung: String,
    protokoll: PathBuf,
    /// The counting of the markers, only in the probe-run mode.
    zaehlung: Option<Zaehlung>,
    /// The check of the peer of each connection (macOS only).
    peer: Peerpruefung,
}

/// The check of the peer before the first byte (addendum to contract 4, section 4): the platform, the own euid and the
/// measurement. Under Linux it lets every connection through and measures nothing.
struct Peerpruefung {
    plattform: Plattform,
    euid: u32,
    messen: fn(&UnixStream) -> std::io::Result<u32>,
}

impl Peerpruefung {
    /// The check of this program: its platform, its euid, `getpeereid`.
    fn eigene() -> Self {
        Self {
            plattform: Plattform::AKTUELL,
            euid: mac::eigene_euid(),
            messen: mac::gemessene_euid,
        }
    }

    /// The reason to turn the connection away, or `None` to accept it.
    fn abweisen(&self, strom: &UnixStream) -> Option<&'static str> {
        if self.plattform == Plattform::Linux {
            return None;
        }
        mac::peer_entscheiden(self.plattform, (self.messen)(strom).ok(), self.euid)
    }
}

/// The most connections the process serves at the same time. A fixed number, no option: a further connection is accepted
/// and closed at once without a byte, and the connections that are open stay served.
pub const MAX_VERBINDUNGEN: usize = 16;

/// Removes the socket of the process, if it has made one.
fn socket_loeschen(pfad: &SocketPfad) {
    let eigener = pfad.lock().unwrap_or_else(PoisonError::into_inner).take();
    if let Some(eigener) = eigener {
        // Only through the held folder and only if the name still shows the socket made at the start.
        let _ = eigener.entfernen();
    }
}

/// SIGTERM and SIGINT remove the socket and end the process with exit 0. A thread waits for the signal, so that the handler
/// itself does nothing but the library's flag.
fn signale_anmelden(pfad: SocketPfad) -> std::io::Result<()> {
    let mut signale = Signals::new([SIGTERM, SIGINT])?;
    thread::spawn(move || {
        if signale.forever().next().is_some() {
            socket_loeschen(&pfad);
            std::process::exit(0);
        }
    });
    Ok(())
}

/// Binds the socket under the mask 077: the socket exists from the first moment with no right for group and others. The mask
/// holds for the whole process; every file it makes later is 0600 or narrower anyway.
fn socket_binden(pfad: &Path) -> std::io::Result<UnixListener> {
    // Anything at the path, even a link that leads nowhere, makes it taken. `bind` alone is not enough: on macOS it
    // follows a dangling link and makes the socket at the link's target (measured 20.11.2026).
    if fs::symlink_metadata(pfad).is_ok() {
        return Err(std::io::Error::from(std::io::ErrorKind::AddrInUse));
    }
    rustix::process::umask(Mode::from_raw_mode(0o077));
    UnixListener::bind(pfad)
}

/// Makes the socket with the given rights. Under macOS the parent folder is checked first (exit 74 on a failure, the path
/// unchanged). Nothing at the path is touched if the path is taken: `bind` fails then. A socket whose rights cannot be set
/// is removed again.
fn socket_anlegen(
    pfad: &Path,
    rechte: u32,
    plattform: Plattform,
    euid: u32,
) -> std::io::Result<(UnixListener, Eigene)> {
    // The rights as `Mode` before anything is made (fail closed): `mode_t` is u32 under Linux and u16 under macOS. A value
    // that does not fit, or has a bit beyond the nine rights bits, is refused, and the path stays as it is. The bits are
    // checked here because `Mode::from_bits` takes every value (rustix declares all bits as known, `const _ = !0`).
    let modus = RawMode::try_from(rechte)
        .ok()
        .filter(|roh| roh & !0o777 == 0)
        .map(Mode::from_raw_mode)
        .ok_or_else(|| std::io::Error::from(std::io::ErrorKind::InvalidInput))?;
    mac::ordner_pruefen(plattform, pfad, euid).map_err(std::io::Error::other)?;
    // The folder is opened once and held to the end: the rights and the removal go through it, never through the path.
    let (ordner, name) = Ordner::von(pfad)?;
    let listener = socket_binden(pfad)?;
    let eigene = Eigene::merken(ordner, name, FileType::Socket)
        .ok_or_else(|| std::io::Error::other("the socket path does not show the new socket"))?;
    eigene.rechte_setzen(modus).inspect_err(|_| {
        let _ = eigene.entfernen();
    })?;
    Ok((listener, eigene))
}

/// A place among the open connections: it counts as long as it lives.
struct Platz(Arc<AtomicUsize>);

impl Platz {
    /// Takes a place, or `None` if `MAX_VERBINDUNGEN` connections are open.
    fn nehmen(offen: &Arc<AtomicUsize>) -> Option<Self> {
        if offen.fetch_add(1, Ordering::SeqCst) >= MAX_VERBINDUNGEN {
            offen.fetch_sub(1, Ordering::SeqCst);
            return None;
        }
        Some(Self(Arc::clone(offen)))
    }
}

impl Drop for Platz {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

/// The start after the arguments are valid: settings, route, log folder, socket, notes, readiness line, then the answers.
/// Returns an exit code only when the start fails or the socket stops accepting.
pub fn starten<O: Write, E: Write>(konfig: Konfig, out: &mut O, err: &mut E) -> u8 {
    let pfad: SocketPfad = Arc::default();
    if let Err(fehler) = signale_anmelden(Arc::clone(&pfad)) {
        let _ = writeln!(err, "rangkern sitzung: signals: {fehler}");
        return EXIT_SOCKET;
    }
    let einst = match einst::lesen_datei(&konfig.einstellungen) {
        Ok(einst) => einst,
        Err(fehler) => {
            let _ = writeln!(err, "rangkern sitzung: settings: {fehler}");
            return EXIT_CONFIG;
        }
    };
    if !einst.routen.contains(&konfig.route) {
        let _ = writeln!(err, "rangkern sitzung: the route is not in the settings");
        return EXIT_CONFIG;
    }
    if !konfig.protokoll.is_dir() {
        let _ = writeln!(err, "rangkern sitzung: the log folder is not a folder");
        return EXIT_CONFIG;
    }
    let (listener, eigene) = match socket_anlegen(
        &konfig.socket,
        konfig.rechte,
        Plattform::AKTUELL,
        mac::eigene_euid(),
    ) {
        Ok(angelegt) => angelegt,
        Err(fehler) => {
            let _ = writeln!(err, "rangkern sitzung: socket: {fehler}");
            return EXIT_SOCKET;
        }
    };
    *pfad.lock().unwrap_or_else(PoisonError::into_inner) = Some(eigene);
    let zaehlung = match probelauf_starten(&konfig, err) {
        Ok(zaehlung) => zaehlung,
        Err(fehler) => {
            let _ = writeln!(err, "rangkern sitzung: probe run: {fehler}");
            socket_loeschen(&pfad);
            return EXIT_CONFIG;
        }
    };
    let server = match konfig.vektoren {
        Vektoren::Datei(datei) => Server::start(konfig.vault, datei),
        Vektoren::Datenbanken(dbs) => Server::start_dbs(konfig.vault, dbs),
    }
    .with_embedder(konfig.embedder);
    let sitzung = Arc::new(Sitzung {
        server: Mutex::new(server),
        tor: Tor::new(einst.vorgabe, konfig.route.clone()),
        route: konfig.route,
        kennung: konfig.sitzung,
        protokoll: konfig.protokoll,
        zaehlung,
        peer: Peerpruefung::eigene(),
    });
    // The starter may start the harness only after this line. A closed pipe leaves nobody to tell.
    let _ = out.write_all(BEREIT).and_then(|()| out.flush());
    let offen = Arc::new(AtomicUsize::new(0));
    for strom in listener
        .incoming()
        .inspect(|strom| {
            if strom.is_err() {
                thread::sleep(Duration::from_millis(50));
            }
        })
        .flatten()
    {
        // Over the limit the stream is dropped here: closed without a byte.
        let Some(platz) = Platz::nehmen(&offen) else {
            continue;
        };
        let sitzung = Arc::clone(&sitzung);
        thread::spawn(move || {
            let _platz = platz;
            verbindung(strom, &sitzung);
        });
    }
    socket_loeschen(&pfad);
    EXIT_SOCKET
}

/// The start of the probe-run mode: the file of the counts with the count 0 first, then the log line `probelauf.zaehlung`, then
/// the fixed line on the error output. `None` without the mode. Any failure ends the start (the caller removes the socket).
fn probelauf_starten<E: Write>(konfig: &Konfig, err: &mut E) -> std::io::Result<Option<Zaehlung>> {
    let Some(ProbeKonfig { marker, zaehlung }) = &konfig.probelauf else {
        return Ok(None);
    };
    let zaehlung = Zaehlung::anlegen(zaehlung.clone(), konfig.sitzung.clone(), marker.clone())?;
    probelauf_zeile(&konfig.protokoll, &konfig.sitzung, zaehlung.anzahl_marker())?;
    writeln!(err, "{STDERR_ZEILE}")?;
    Ok(Some(zaehlung))
}

/// The log line `probelauf.zaehlung` of the start: `anhaengen_probelauf` of `formate` writes it into the log of the month.
fn probelauf_zeile(ordner: &Path, sitzung: &str, anzahl: usize) -> std::io::Result<()> {
    let zeile = Probelauf {
        zeit_ms: jetzt_ms(),
        programm: "suche".to_owned(),
        sitzung: sitzung.to_owned(),
        anzahl: u32::try_from(anzahl).unwrap_or(u32::MAX),
    };
    anhaengen_probelauf(ordner, "brain", &zeile)
        .map(|_| ())
        .map_err(|fehler| std::io::Error::other(fehler.code()))
}

/// One connection: the answers come in the order of the requests; the end of the connection ends nothing else.
fn verbindung(strom: UnixStream, sitzung: &Sitzung) {
    // Before the first read and the first write: a peer that is not ours gets no byte, and the connection ends.
    if let Some(grund) = sitzung.peer.abweisen(&strom) {
        sitzung.abgewiesen(grund);
        return;
    }
    let Ok(lesestrom) = strom.try_clone() else {
        return;
    };
    let mut leser = BufReader::new(lesestrom);
    let mut schreiber = strom;
    while let Ok(Some(zeile)) = next_line(&mut leser) {
        let bytes = sitzung.antworten(&zeile);
        if schreiber.write_all(&bytes).is_err() {
            break;
        }
    }
}

/// The current time in milliseconds since 1970.
fn jetzt_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|dauer| i64::try_from(dauer.as_millis()).ok())
        .unwrap_or_default()
}

/// The buffer of an answer: one JSON line with its line feed.
fn zeile_von(antwort: &Value) -> Vec<u8> {
    let mut bytes = serde_json::to_vec(antwort).unwrap_or_default();
    bytes.push(b'\n');
    bytes
}

/// The answer with the `id` of the request, if it had one.
fn mit_id(mut antwort: Value, id: Option<Value>) -> Value {
    if let (Some(id), Some(objekt)) = (id, antwort.as_object_mut()) {
        objekt.insert("id".to_owned(), id);
    }
    antwort
}

impl Sitzung {
    fn server(&self) -> MutexGuard<'_, Server> {
        self.server.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// The answer to one line, as the buffer that goes to the socket: one JSON line with its line feed.
    fn antworten(&self, zeile: &Line) -> Vec<u8> {
        if zeile.too_long {
            return zeile_von(&bad_request(None));
        }
        let anfrage = match anfrage::lesen(&zeile.bytes) {
            Ok(anfrage) => anfrage,
            Err(id) => return zeile_von(&bad_request(id)),
        };
        match anfrage.op {
            Op::Ping => zeile_von(&mit_id(json!({"ok": true}), anfrage.id)),
            Op::Search { query, k, space } => self.suchen(query, k, space, anfrage.id),
        }
    }

    /// A search: the whole list, the gate, then the counting of the markers (probe run only), then the log lines for the
    /// blocked hits, then the buffer of the answer. If the counting or a log line cannot be written, the answer is
    /// `{"error":"protokoll"}` with no hit; a failed counting writes no log line.
    fn suchen(&self, query: String, k: usize, space: Option<String>, id: Option<Value>) -> Vec<u8> {
        let mut server = self.server();
        // The whole list: the prefix property makes `L[..k]` the answer of `rangkern serve` for `k`.
        let frage = SearchRequest {
            query,
            k: usize::MAX,
            space,
            today: None,
            query_vector: None,
        };
        let (antwort, gruppen) = match server.search_hits(&frage, now_ns()) {
            Err(_) => (json!({"error": "unreadable"}), Vec::new()),
            Ok((liste, fallback)) => {
                let notes: HashMap<_, _> = server.notes_by_rel();
                let paare = contradiction_pairs(server.root());
                let durch = self.tor.durchlassen(liste, k, &notes, &paare);
                let antwort = json!({
                    "fallback": fallback,
                    "hits": durch.hits.iter().map(hit_json).collect::<Vec<_>>(),
                    "gesperrt": durch.gesperrt(),
                });
                (antwort, durch.gruppen)
            }
        };
        let puffer = zeile_von(&mit_id(antwort, id.clone()));
        let gezaehlt = self
            .zaehlung
            .as_ref()
            .is_none_or(|zaehlung| zaehlung.zaehlen(&puffer).is_ok());
        if !gezaehlt || self.protokollieren(&gruppen).is_err() {
            return zeile_von(&mit_id(json!({"error": "protokoll"}), id));
        }
        puffer
    }

    /// The log line `kanal.abgewiesen` for a connection turned away: channel `such`, route and session from the start, no uid.
    /// If it cannot be written, one line without content goes to the error output; the process goes on.
    fn abgewiesen(&self, grund: &str) {
        let zeile = Abgewiesen {
            zeit_ms: jetzt_ms(),
            grund: grund.to_owned(),
            kanal: "such".to_owned(),
            route: Some(self.route.clone()),
            sitzung: Some(self.kennung.clone()),
        };
        if anhaengen_abgewiesen(&self.protokoll, "brain", &zeile).is_err() {
            let _ = writeln!(
                std::io::stderr(),
                "rangkern sitzung: kanal.abgewiesen: the line could not be written"
            );
        }
    }

    /// Writes one line per group to the log of the month, with the time of now.
    fn protokollieren(&self, gruppen: &[Gruppe]) -> Result<(), formate::kette::Fehler> {
        if gruppen.is_empty() {
            return Ok(());
        }
        let zeit_ms = jetzt_ms();
        let datei = self.protokoll.join(abschnitt("brain", zeit_ms)?);
        for gruppe in gruppen {
            anhaengen_gesperrt(
                &datei,
                &Gesperrt {
                    zeit_ms,
                    route: self.route.clone(),
                    sitzung: self.kennung.clone(),
                    grund: gruppe.grund.to_owned(),
                    klasse: gruppe.klasse.clone(),
                    kunde: gruppe.kunde.clone(),
                    anzahl: gruppe.anzahl,
                },
            )?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::{MetadataExt, PermissionsExt};

    fn ordner(name: &str) -> PathBuf {
        let ordner = PathBuf::from(format!("/tmp/rkl-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&ordner);
        fs::create_dir_all(&ordner).expect("folder is made");
        ordner
    }

    #[test]
    fn the_socket_has_no_right_for_group_and_others_right_after_the_bind() {
        let ordner = ordner("umask");
        // A loose mask, as a caller may have it: only the mask of the process makes the socket narrow.
        rustix::process::umask(Mode::from_raw_mode(0o000));
        let _listener = socket_binden(&ordner.join("s.sock")).expect("the socket is bound");
        let modus = fs::symlink_metadata(ordner.join("s.sock"))
            .expect("the socket")
            .mode();
        assert_eq!(modus & 0o077, 0, "mode {modus:o} before the chmod");
        let _ = fs::remove_dir_all(&ordner);
    }

    #[test]
    fn places_are_taken_up_to_the_limit_and_freed_on_drop() {
        let offen = Arc::new(AtomicUsize::new(0));
        let mut plaetze: Vec<Platz> = (0..MAX_VERBINDUNGEN)
            .map(|_| Platz::nehmen(&offen).expect("a place"))
            .collect();
        assert_eq!(MAX_VERBINDUNGEN, 16);
        assert!(Platz::nehmen(&offen).is_none(), "the 17th has no place");
        assert_eq!(offen.load(Ordering::SeqCst), MAX_VERBINDUNGEN);
        plaetze.pop();
        assert_eq!(offen.load(Ordering::SeqCst), MAX_VERBINDUNGEN - 1);
        assert!(Platz::nehmen(&offen).is_some(), "a freed place is free");
    }

    /// A session for the tests of the peer check: an empty vault, a route `cloud`, the log folder `protokoll` below `ordner`.
    fn sitzung_fuer(ordner: &Path, peer: Peerpruefung) -> Sitzung {
        fs::create_dir_all(ordner.join("vault")).expect("vault");
        fs::create_dir_all(ordner.join("protokoll")).expect("log folder");
        let einst_datei = ordner.join("einst.json");
        fs::write(
            &einst_datei,
            r#"{"fassung":1,"klassen":["oeffentlich","intern"],"vorgabestufe":"intern","routen":[{"schluessel":"cloud","art":"cloud","klasse":"intern"}],"kunden":[],"kundendaten_an_cloud":{"vorgabe":"nie","freigaben":[]}}"#,
        )
        .expect("settings");
        let einst = einst::lesen_datei(&einst_datei).expect("settings are read");
        Sitzung {
            server: Mutex::new(Server::start(ordner.join("vault"), None)),
            tor: Tor::new(einst.vorgabe, "cloud".to_owned()),
            route: "cloud".to_owned(),
            kennung: "s-1".to_owned(),
            protokoll: ordner.join("protokoll"),
            zaehlung: None,
            peer,
        }
    }

    const FREMD: u32 = 4242;
    const PING: &str = "{\"op\":\"ping\"}\n";

    #[allow(clippy::unnecessary_wraps)] // the type of the measurement is fixed by the check
    fn messen_fremd(_: &UnixStream) -> std::io::Result<u32> {
        Ok(FREMD)
    }

    #[allow(clippy::unnecessary_wraps)] // the type of the measurement is fixed by the check
    fn messen_eigen(_: &UnixStream) -> std::io::Result<u32> {
        Ok(mac::eigene_euid())
    }

    fn messen_scheitert(_: &UnixStream) -> std::io::Result<u32> {
        Err(std::io::Error::from(std::io::ErrorKind::InvalidInput))
    }

    fn mac_peer(messen: fn(&UnixStream) -> std::io::Result<u32>) -> Peerpruefung {
        Peerpruefung {
            plattform: Plattform::MacOs,
            euid: mac::eigene_euid(),
            messen,
        }
    }

    /// One connection of a client that sends `anfrage` and its end: what the process sends back.
    fn verbinden(sitzung: &Sitzung, anfrage: &str) -> Vec<u8> {
        use std::io::Read;
        use std::net::Shutdown;
        let (server, mut client) = UnixStream::pair().expect("pair");
        client.write_all(anfrage.as_bytes()).expect("request");
        client
            .shutdown(Shutdown::Write)
            .expect("end of the request");
        verbindung(server, sitzung);
        let mut antwort = Vec::new();
        // A connection closed with the request unread may end in a reset instead of a plain end: still no byte.
        match client.read_to_end(&mut antwort) {
            Ok(_) => {}
            Err(fehler) if fehler.kind() == std::io::ErrorKind::ConnectionReset => {}
            Err(fehler) => panic!("answer: {fehler}"),
        }
        antwort
    }

    /// The lines of the log of the month in the log folder.
    fn zeilen(ordner: &Path) -> Vec<Value> {
        let mut alle = Vec::new();
        for eintrag in fs::read_dir(ordner.join("protokoll")).expect("log folder") {
            let text = fs::read_to_string(eintrag.expect("entry").path()).expect("log");
            alle.extend(text.lines().map(|z| serde_json::from_str(z).expect("JSON")));
        }
        alle
    }

    #[test]
    fn a_foreign_peer_gets_no_byte_and_one_line_without_uid_and_the_process_goes_on() {
        let ordner = ordner("peer-fremd");
        let fremd = sitzung_fuer(&ordner, mac_peer(messen_fremd));
        assert_eq!(verbinden(&fremd, PING), b"");
        assert_eq!(verbinden(&fremd, PING), b"");
        let alle = zeilen(&ordner);
        assert_eq!(alle.len(), 2, "one line per turned-away connection");
        for zeile in &alle {
            let objekt = zeile.as_object().expect("object");
            let mut felder: Vec<&str> = objekt.keys().map(String::as_str).collect();
            felder.sort_unstable();
            assert_eq!(
                felder,
                [
                    "aktionsname",
                    "ergebnis",
                    "fassung",
                    "grund",
                    "kanal",
                    "route",
                    "sitzung",
                    "vorher",
                    "zeit"
                ]
            );
            assert_eq!(zeile["aktionsname"], "kanal.abgewiesen");
            assert_eq!(zeile["grund"], "such.kennung_fremd");
            assert_eq!(zeile["kanal"], "such");
            assert_eq!(zeile["route"], "cloud");
            assert_eq!(zeile["sitzung"], "s-1");
            assert!(!zeile.to_string().contains(&FREMD.to_string()), "no uid");
        }
        // The next connection of the own user is answered: the process went on.
        let eigen = sitzung_fuer(&ordner, mac_peer(messen_eigen));
        assert_eq!(
            String::from_utf8(verbinden(&eigen, PING)).expect("text"),
            "{\"ok\":true}\n"
        );
        assert_eq!(zeilen(&ordner).len(), 2, "an accepted peer writes no line");
        let _ = fs::remove_dir_all(&ordner);
    }

    #[test]
    fn a_peer_that_cannot_be_measured_gets_no_byte_and_a_line_with_its_reason() {
        let ordner = ordner("peer-unmessbar");
        let sitzung = sitzung_fuer(&ordner, mac_peer(messen_scheitert));
        assert_eq!(verbinden(&sitzung, PING), b"");
        let alle = zeilen(&ordner);
        assert_eq!(alle.len(), 1);
        assert_eq!(alle[0]["grund"], "such.kennung_unmessbar");
        assert_eq!(alle[0]["kanal"], "such");
        let _ = fs::remove_dir_all(&ordner);
    }

    #[test]
    fn a_line_that_cannot_be_written_still_turns_the_peer_away_and_the_process_goes_on() {
        let ordner = ordner("peer-zeile-scheitert");
        let sitzung = sitzung_fuer(&ordner, mac_peer(messen_fremd));
        fs::remove_dir_all(ordner.join("protokoll")).expect("remove the log folder");
        assert_eq!(verbinden(&sitzung, PING), b"");
        let eigen = sitzung_fuer(&ordner, mac_peer(messen_eigen));
        assert_eq!(
            String::from_utf8(verbinden(&eigen, PING)).expect("text"),
            "{\"ok\":true}\n"
        );
        let _ = fs::remove_dir_all(&ordner);
    }

    #[test]
    fn linux_measures_nothing_and_turns_nobody_away() {
        let ordner = ordner("peer-linux");
        let linux = Peerpruefung {
            plattform: Plattform::Linux,
            euid: mac::eigene_euid(),
            messen: messen_scheitert,
        };
        let sitzung = sitzung_fuer(&ordner, linux);
        assert_eq!(
            String::from_utf8(verbinden(&sitzung, PING)).expect("text"),
            "{\"ok\":true}\n"
        );
        assert_eq!(zeilen(&ordner).len(), 0);
        let _ = fs::remove_dir_all(&ordner);
    }

    #[test]
    fn a_parent_folder_that_is_not_0700_ends_the_socket_before_bind_under_macos() {
        let ordner = ordner("ordner-0755");
        let eltern = ordner.join("s");
        fs::create_dir_all(&eltern).expect("parent");
        fs::set_permissions(&eltern, fs::Permissions::from_mode(0o755)).expect("mode");
        let euid = mac::eigene_euid();
        assert!(socket_anlegen(&eltern.join("such.sock"), 0o600, Plattform::MacOs, euid).is_err());
        assert!(
            !eltern.join("such.sock").exists(),
            "the path stays as it is"
        );
        fs::set_permissions(&eltern, fs::Permissions::from_mode(0o700)).expect("mode");
        assert!(socket_anlegen(&eltern.join("such.sock"), 0o600, Plattform::MacOs, euid).is_ok());
        let _ = fs::remove_dir_all(&ordner);
    }

    /// The pointer of the end on a signal for a socket just made at `pfad`.
    fn angelegt(pfad: &Path) -> (UnixListener, SocketPfad) {
        let euid = mac::eigene_euid();
        // Under macOS the folder must be 0700.
        let eltern = pfad.parent().expect("parent");
        fs::set_permissions(eltern, fs::Permissions::from_mode(0o700)).expect("mode");
        let (listener, eigene) =
            socket_anlegen(pfad, 0o600, Plattform::AKTUELL, euid).expect("socket");
        (listener, Arc::new(Mutex::new(Some(eigene))))
    }

    #[test]
    fn the_end_removes_the_own_socket_through_the_held_folder() {
        let ordner = ordner("loeschen-eigen");
        let pfad = ordner.join("s.sock");
        let (_listener, eigene) = angelegt(&pfad);
        assert!(fs::symlink_metadata(&pfad).is_ok());
        socket_loeschen(&eigene);
        assert!(fs::symlink_metadata(&pfad).is_err(), "the socket is gone");
        socket_loeschen(&eigene);
        let _ = fs::remove_dir_all(&ordner);
    }

    #[test]
    fn the_end_leaves_another_file_a_link_or_a_folder_under_the_name() {
        let ordner = ordner("loeschen-fremd");
        let pfad = ordner.join("s.sock");
        let (_listener, eigene) = angelegt(&pfad);
        // The own socket moves away; a regular file takes its name.
        let weg = ordner.join("weg.sock");
        fs::rename(&pfad, &weg).expect("move");
        fs::write(&pfad, "fremd").expect("file");
        socket_loeschen(&eigene);
        assert_eq!(fs::read_to_string(&pfad).expect("file"), "fremd");
        assert!(weg.exists(), "the moved socket is not touched");
        // A link to the own socket (same identity once followed) takes its name.
        let (_listener2, eigene2) = {
            fs::remove_file(&pfad).expect("remove");
            fs::remove_file(&weg).expect("remove");
            angelegt(&pfad)
        };
        fs::rename(&pfad, &weg).expect("move");
        std::os::unix::fs::symlink(&weg, &pfad).expect("link");
        socket_loeschen(&eigene2);
        assert!(fs::symlink_metadata(&pfad).expect("link").is_symlink());
        assert!(fs::symlink_metadata(&weg).is_ok());
        let _ = fs::remove_dir_all(&ordner);
    }

    #[test]
    fn the_rights_of_the_socket_are_set_only_on_the_own_socket() {
        let ordner = ordner("rechte-fremd");
        let pfad = ordner.join("s.sock");
        let (_listener, eigene) = angelegt(&pfad);
        let fremd = ordner.join("fremd");
        fs::write(&fremd, "x").expect("file");
        fs::set_permissions(&fremd, fs::Permissions::from_mode(0o600)).expect("chmod");
        // The own socket moves away; a link to a foreign file takes its name.
        fs::rename(&pfad, ordner.join("weg.sock")).expect("move");
        std::os::unix::fs::symlink(&fremd, &pfad).expect("link");
        let guard = eigene.lock().unwrap_or_else(PoisonError::into_inner);
        let eigene_socket = guard.as_ref().expect("own");
        assert!(
            eigene_socket
                .rechte_setzen(Mode::from_raw_mode(0o666))
                .is_err()
        );
        assert_eq!(
            fs::metadata(&fremd).expect("meta").mode() & 0o7777,
            0o600,
            "the target of the link keeps its mode"
        );
        // A regular file under the name: no change either.
        fs::remove_file(&pfad).expect("remove");
        fs::write(&pfad, "y").expect("file");
        fs::set_permissions(&pfad, fs::Permissions::from_mode(0o600)).expect("chmod");
        assert!(
            eigene_socket
                .rechte_setzen(Mode::from_raw_mode(0o666))
                .is_err()
        );
        assert_eq!(fs::metadata(&pfad).expect("meta").mode() & 0o7777, 0o600);
        drop(guard);
        let _ = fs::remove_dir_all(&ordner);
    }

    #[test]
    fn a_made_socket_has_the_asked_rights_and_a_taken_path_stays() {
        let ordner = ordner("rechte-eigen");
        let pfad = ordner.join("s.sock");
        let euid = mac::eigene_euid();
        fs::set_permissions(&ordner, fs::Permissions::from_mode(0o700)).expect("mode");
        let (_listener, eigene) =
            socket_anlegen(&pfad, 0o600, Plattform::AKTUELL, euid).expect("socket");
        assert_eq!(
            fs::symlink_metadata(&pfad).expect("socket").mode() & 0o7777,
            0o600
        );
        eigene
            .rechte_setzen(Mode::from_raw_mode(0o660))
            .expect("chmod");
        assert_eq!(
            fs::symlink_metadata(&pfad).expect("socket").mode() & 0o7777,
            0o660
        );
        // Rights beyond the nine rights bits, or too large for `mode_t`: refused before anything is made.
        let anderer = ordner.join("t.sock");
        for schlecht in [0o4600, 0o1000_0600, u32::MAX] {
            assert!(
                socket_anlegen(&anderer, schlecht, Plattform::AKTUELL, euid).is_err(),
                "{schlecht:o}"
            );
            assert!(
                fs::symlink_metadata(&anderer).is_err(),
                "nothing made for {schlecht:o}"
            );
        }
        // The path is taken: nothing changes, and the first socket is not the own one of the second call.
        assert!(socket_anlegen(&pfad, 0o666, Plattform::AKTUELL, euid).is_err());
        assert_eq!(
            fs::symlink_metadata(&pfad).expect("socket").mode() & 0o7777,
            0o660
        );
        assert!(eigene.entfernen());
        let _ = fs::remove_dir_all(&ordner);
    }
}
