//! The part of `rangkern sitzung` that is the same on every system: the checks of the start before the socket or the pipe
//! (exit 78), the start of the probe run, the session that answers one line of a request, and the loop of one connection.
//! The transport lives elsewhere: the Unix socket in `lauf` (Linux and macOS), the named pipe in `windows` (addendum for
//! Windows); both hand their connections to the same [`Sitzung`].

use std::collections::HashMap;
use std::io::{BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use formate::kette::{
    Abgewiesen, Gesperrt, Probelauf, anhaengen_abgewiesen, anhaengen_gesperrt_in,
    anhaengen_probelauf, anhaengen_zeile,
};
use serde_json::{Value, json};

use super::EXIT_CONFIG;
use super::anfrage::{self, Op};
use super::einst::{self, Einstellungen};
#[cfg(windows)]
use super::konfig::Vektoren;
use super::konfig::{Konfig, Probelauf as ProbeKonfig};
use super::probelauf::{STDERR_ZEILE, Zaehlung, zaehldatei_pruefen};
use super::tor::{Gruppe, Tor};
use crate::cli::hit_json;
use crate::embed::Grund;
use crate::serve::{Line, SearchRequest, Server, bad_request, next_line};
use crate::stamp::now_ns;
use crate::vault::contradiction_pairs;

/// Everything a connection needs to answer.
pub(crate) struct Sitzung {
    pub(crate) server: Mutex<Server>,
    pub(crate) tor: Tor,
    pub(crate) route: String,
    pub(crate) kennung: String,
    pub(crate) protokoll: PathBuf,
    /// The counting of the markers, only in the probe-run mode.
    pub(crate) zaehlung: Option<Zaehlung>,
    /// The check of the peer of each connection (macOS only; under Windows `peerkanal` checks the peer before the
    /// connection reaches the session).
    #[cfg(unix)]
    pub(crate) peer: super::lauf::Peerpruefung,
}

/// The most connections the process serves at the same time. A fixed number, no option: a further connection is accepted
/// and closed at once without a byte, and the connections that are open stay served.
pub const MAX_VERBINDUNGEN: usize = 16;

/// How long a connection may stay silent (clarification 13 of contract 4, decision of 24.11.2026): when no byte arrives for this
/// long, the process closes the connection without an answer and without a log line, and its place is free. The time counts from
/// the last received byte, also inside a line, and it does not run while the process works on an answer.
pub const LEERLAUF: Duration = Duration::from_secs(10);

/// A place among the open connections: it counts as long as it lives.
pub(crate) struct Platz(Arc<AtomicUsize>);

impl Platz {
    /// Takes a place, or `None` if `MAX_VERBINDUNGEN` connections are open.
    pub(crate) fn nehmen(offen: &Arc<AtomicUsize>) -> Option<Self> {
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

/// The checks of the start that come before the socket or the pipe, in the fixed order of contract 4: the settings, the route
/// in them, the log folder, the count file of the probe run. Each failure writes one line to `err` and gives exit 78; 78 comes
/// before 74 (W3).
///
/// # Errors
/// The exit code 78.
pub(crate) fn vor_dem_kanal<E: Write>(konfig: &Konfig, err: &mut E) -> Result<Einstellungen, u8> {
    let einst = match einst::lesen_datei(&konfig.einstellungen) {
        Ok(einst) => einst,
        Err(fehler) => {
            let _ = writeln!(err, "rangkern sitzung: settings: {fehler}");
            return Err(EXIT_CONFIG);
        }
    };
    if !einst.routen.contains(&konfig.route) {
        let _ = writeln!(err, "rangkern sitzung: the route is not in the settings");
        return Err(EXIT_CONFIG);
    }
    if !konfig.protokoll.is_dir() {
        let _ = writeln!(err, "rangkern sitzung: the log folder is not a folder");
        return Err(EXIT_CONFIG);
    }
    // The count file belongs to 78, and 78 comes before 74: it is checked before the socket (W3).
    if let Some(probe) = &konfig.probelauf
        && let Err(fehler) = zaehldatei_pruefen(&probe.zaehlung)
    {
        let _ = writeln!(err, "rangkern sitzung: probe run: {fehler}");
        return Err(EXIT_CONFIG);
    }
    Ok(einst)
}

/// The session of a start whose checks passed: the notes and the index are loaded here, after the socket or the pipe exists.
#[cfg(windows)]
pub(crate) fn sitzung_bauen(
    konfig: Konfig,
    einst: Einstellungen,
    zaehlung: Option<Zaehlung>,
) -> Sitzung {
    let server = match konfig.vektoren {
        Vektoren::Datei(datei) => Server::start(konfig.vault, datei),
        Vektoren::Datenbanken(dbs) => Server::start_dbs(konfig.vault, dbs),
    }
    .with_embedder(konfig.embedder);
    Sitzung {
        server: Mutex::new(server),
        tor: Tor::new(einst.vorgabe, konfig.route.clone()),
        route: konfig.route,
        kennung: konfig.sitzung,
        protokoll: konfig.protokoll,
        zaehlung,
    }
}

/// One connection of any transport after its peer was accepted: one answer line per request line, in the order of the
/// requests, until the input ends, a read fails (also the idle time of the transport) or a write fails. The end of the
/// connection ends nothing else.
#[cfg_attr(not(windows), allow(dead_code))] // the Unix socket keeps its own loop with the idle time (`lauf::verbindung_mit`)
pub(crate) fn bedienen<S: Read + Write>(strom: S, sitzung: &Sitzung) {
    let mut leser = BufReader::new(strom);
    while let Ok(Some(zeile)) = next_line(&mut leser) {
        let bytes = sitzung.antworten(&zeile);
        if leser.get_mut().write_all(&bytes).is_err() {
            break;
        }
    }
}

/// The start of the probe-run mode: the file of the counts with the count 0 first, then the log line `probelauf.zaehlung`, then
/// the fixed line on the error output. `None` without the mode. A failure of the file or of the log line ends the start (the
/// caller removes the socket); a failed log line removes the file again, so nothing of the failed start stays (review B8). The
/// line on the error output comes after the log line and its failure ends nothing: the line is in the chain then, and a closed
/// error output leaves nobody to tell (review B8: no failed start with a line in the chain).
pub(crate) fn probelauf_starten<E: Write>(
    konfig: &Konfig,
    err: &mut E,
) -> std::io::Result<Option<Zaehlung>> {
    let Some(ProbeKonfig { marker, zaehlung }) = &konfig.probelauf else {
        return Ok(None);
    };
    let zaehlung = Zaehlung::anlegen(zaehlung.clone(), konfig.sitzung.clone(), marker.clone())?;
    if let Err(fehler) =
        probelauf_zeile(&konfig.protokoll, &konfig.sitzung, zaehlung.anzahl_marker())
    {
        let _ = zaehlung.entfernen();
        return Err(fehler);
    }
    let _ = writeln!(err, "{STDERR_ZEILE}");
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

/// The current time in milliseconds since 1970.
pub(crate) fn jetzt_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|dauer| i64::try_from(dauer.as_millis()).ok())
        .unwrap_or_default()
}

/// The buffer of an answer: one JSON line with its line feed.
pub(crate) fn zeile_von(antwort: &Value) -> Vec<u8> {
    let mut bytes = serde_json::to_vec(antwort).unwrap_or_default();
    bytes.push(b'\n');
    bytes
}

/// The answer with the `id` of the request, if it had one.
pub(crate) fn mit_id(mut antwort: Value, id: Option<Value>) -> Value {
    if let (Some(id), Some(objekt)) = (id, antwort.as_object_mut()) {
        objekt.insert("id".to_owned(), id);
    }
    antwort
}

impl Sitzung {
    pub(crate) fn server(&self) -> MutexGuard<'_, Server> {
        self.server.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// The answer to one line, as the buffer that goes to the socket: one JSON line with its line feed.
    pub(crate) fn antworten(&self, zeile: &Line) -> Vec<u8> {
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
    /// blocked hits, then the line `suche.rueckfall` when the embedding of `--embed` failed, then the buffer of the answer.
    /// If the counting or a line of a blocked hit cannot be written, the answer is `{"error":"protokoll"}` with no hit; a
    /// failed counting writes no log line. A failed line `suche.rueckfall` changes nothing of the answer.
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
        let (antwort, gruppen, ausfall) = match server.search_hits_ausfall(&frage, now_ns()) {
            Err(_) => (json!({"error": "unreadable"}), Vec::new(), None),
            Ok((liste, fallback, ausfall)) => {
                let notes: HashMap<_, _> = server.notes_by_rel();
                let paare = contradiction_pairs(server.root());
                let durch = self.tor.durchlassen(liste, k, &notes, &paare);
                let antwort = json!({
                    "fallback": fallback,
                    "hits": durch.hits.iter().map(hit_json).collect::<Vec<_>>(),
                    "gesperrt": durch.gesperrt(),
                });
                (antwort, durch.gruppen, ausfall)
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
        if let Some(grund) = ausfall {
            self.rueckfall(grund);
        }
        puffer
    }

    /// The log line `suche.rueckfall` (contract 5, section 6.5): the search answers lexically because the embedding of
    /// `--embed` failed. A call of its own after the lines of the blocked hits. A failure is dropped: no second try, no
    /// buffer, no text on the error output, no field in the answer (contract 4, "Rückfall der Einbettung").
    fn rueckfall(&self, grund: Grund) {
        let _ = anhaengen_zeile(
            &self.protokoll,
            "brain",
            jetzt_ms(),
            &json!({
                "aktionsname": "suche.rueckfall",
                "ergebnis": "lexikalisch",
                "grund": grund.text(),
                "route": self.route,
                "sitzung": self.kennung,
            }),
        );
    }

    /// The log line `kanal.abgewiesen` for a connection turned away: channel `such`, route and session from the start, no uid.
    /// If it cannot be written, one line without content goes to the error output; the process goes on.
    pub(crate) fn abgewiesen(&self, grund: &str) {
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

    /// Writes one line per group to the chain `brain`, with the time of now. Every line of the process goes through the
    /// functions with folder and source, which lock the source (contract 5, section 5.3: "beide Wege nie mischen"); until
    /// 23.11.2026 these lines locked only the file of the month (`anhaengen_gesperrt`), while the other lines locked the source.
    fn protokollieren(&self, gruppen: &[Gruppe]) -> Result<(), formate::kette::Fehler> {
        let zeit_ms = jetzt_ms();
        for gruppe in gruppen {
            anhaengen_gesperrt_in(
                &self.protokoll,
                "brain",
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
    use std::fs;
    use std::io::Cursor;

    /// A stream of the tests: the request bytes to read, and what the session wrote.
    struct Strom {
        lesen: Cursor<Vec<u8>>,
        geschrieben: Vec<u8>,
    }

    impl Read for Strom {
        fn read(&mut self, puffer: &mut [u8]) -> std::io::Result<usize> {
            self.lesen.read(puffer)
        }
    }

    impl Write for Strom {
        fn write(&mut self, daten: &[u8]) -> std::io::Result<usize> {
            self.geschrieben.write(daten)
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
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
            server: Mutex::new(Server::start(ordner.join("vault"), None)),
            tor: Tor::new(einst.vorgabe, "cloud".to_owned()),
            route: "cloud".to_owned(),
            kennung: "s-1".to_owned(),
            protokoll: ordner.join("protokoll"),
            zaehlung: None,
            #[cfg(unix)]
            peer: super::super::lauf::Peerpruefung::ohne_pruefung(),
        }
    }

    /// The loop of the pipe: one answer line per request line, in order, and the end of the input ends it.
    #[test]
    fn one_answer_line_per_request_line_until_the_end_of_the_input() {
        let ordner = std::env::temp_dir().join(format!("rk-kern-{}", std::process::id()));
        let _ = fs::remove_dir_all(&ordner);
        let sitzung = sitzung(&ordner);
        let mut strom = Strom {
            lesen: Cursor::new(
                b"{\"op\":\"ping\",\"id\":1}\n{\"op\":\"quit\"}\n{\"op\":\"ping\",\"id\":2}\n"
                    .to_vec(),
            ),
            geschrieben: Vec::new(),
        };
        bedienen(&mut strom, &sitzung);
        assert_eq!(
            String::from_utf8(strom.geschrieben).expect("text"),
            "{\"id\":1,\"ok\":true}\n{\"error\":\"bad_request\"}\n{\"id\":2,\"ok\":true}\n"
        );
        let _ = fs::remove_dir_all(&ordner);
    }
}
