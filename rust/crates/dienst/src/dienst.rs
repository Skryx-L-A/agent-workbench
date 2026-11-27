//! The service: one child in a PTY, the state core, one extract per viewer, a Unix socket (contract sections 1, 5 and 7).
//!
//! It only translates. Every rule of the core stays in `zustandskern`, every rule of the extract in `rasterauszug`, every rule
//! of the screen in `traeger`; the service gives events to the core, carries out its effects and makes frames of its messages.
//!
//! Threads: one thread (the loop) does all the work and owns every state, so that the steps of the core come in one order. It
//! waits for the listener, every connection and a wake socket in one place (`bereit.rs`: `epoll` on Linux, `poll` elsewhere) and
//! reads the connections that are ready in the order the wait names them (on Linux the order of arrival, so that frames of two
//! viewers are handled in the order they were sent, also when the loop wakes late; a reading thread per connection made the order
//! depend on which thread wakes first). The only other threads are the reading thread inside `traeger::Kind` (it keeps the PTY
//! drained) and, in `laufen`, one that waits for SIGTERM and SIGINT; they call [`Anhalter::anhalten`], which sets a flag and
//! writes one byte to the wake socket. All sockets are non-blocking. The loop wakes for every readable connection and at least
//! every 2 ms (1 ms while a viewer has a backlog) to accept connections, take the output of the child, write the queues and run
//! the timers of the core.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::io::{ErrorKind, Read, Write};
#[cfg(unix)]
use std::os::fd::AsFd;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

use einstellungen::Einstellungen;
use serde_json::{Map, Value, json};
use traeger::Ereignis;
use zustandskern::Wirkungen;

use crate::arbeitsordner;
use crate::bereit::Bereitschaft;
use crate::betrachter::{Betrachter, json_rahmen};
use crate::fehler::StartFehler;
use crate::json::{ereignis_lesen, unhex};
use crate::meldung::{Meldung, SOCKET_NAME};
use crate::rahmen::{
    A_ABLEHNUNG, A_EREIGNIS, A_FEHLER, A_KERNFEHLER, A_NACHRICHT, A_RASTER, A_STUMM, A_UHR,
    A_ZUSTAND, rahmen,
};
use crate::rechte::{ordner_oeffnen, socket_binden};
use crate::sitzung::{
    KindStart, Sitzung, Traegerstand, anfang_fuer, anfang_raum, eintrag_fuer, kind_starten,
    kinder_beenden,
};
use crate::startdatei::{KLEINSTE_SPALTEN, Startdatei};
use crate::starten::{
    Abgelehnt, SITZUNGEN_MAX, arbeitsordner_von, einstellungen_laden, meta_von, nummer_vergebbar,
    ordner_anlegen, rahmennummer, start_form, start_pruefen, starten_form, startwahl_von,
};
use crate::sys::{Ordner, UnixListener, UnixStream};
use crate::verwaltung::worker_form;
use crate::waisen::{Waisen, kinder_verwerfen, start_verwerfen};

/// The kinds of event a viewer may give (DI3): the lines `Ansicht` of `VT` section 4 without `Verbinden` and `Trennen`.
const ERLAUBT: [&str; 12] = [
    "Abo",
    "Groesse",
    "Text",
    "Taste",
    "Einfuegen",
    "Antwort",
    "Freigabe",
    "RouteWaehlen",
    "EinstellungSetzen",
    "RaumWechsel",
    "OrdnerLesen",
    "InhaltSuchen",
];

/// How long the loop waits when nothing happens.
const TAKT: Duration = Duration::from_millis(2);
/// How long the loop waits while a viewer has a backlog (the socket may take bytes again at any moment).
const TAKT_STAU: Duration = Duration::from_millis(1);
/// How many times per turn the output of a child is taken.
const KIND_LESEN_MAX: usize = 256;
/// The key of the wake socket in the set of sockets the loop waits for. The key of a viewer is its number (counting from 1); the
/// two keys here are no number of a viewer, so a wait that names one of them finds no viewer and the loop goes on.
const SCHLUESSEL_WECK: u64 = 0;
/// The key of the listener (see [`SCHLUESSEL_WECK`]).
const SCHLUESSEL_HOERER: u64 = u64::MAX;
/// How many bytes the loop reads from one connection in one turn.
const LESE_GROESSE: usize = 65536;

/// An event that waits for its step: the event, who gave it (for a rejection) and the index of the core.
type Arbeit = (Value, Option<u64>, usize);

/// Stops a running [`Dienst`] from another thread (the signal thread does the same on SIGTERM and SIGINT).
#[derive(Clone)]
pub struct Anhalter {
    halt: Arc<AtomicBool>,
    /// The write end of the wake socket (non-blocking).
    weck: Arc<UnixStream>,
    runden: Arc<AtomicU64>,
}

impl Anhalter {
    /// Asks the loop to end: it removes the socket, ends the child and returns 0.
    pub fn anhalten(&self) {
        self.halt.store(true, Ordering::SeqCst);
        // a full wake socket already holds a byte that wakes the loop: the error can be ignored
        let _ = (&*self.weck).write(&[1]);
    }

    /// How many turns the loop has made so far. A loop that waits has few; a loop that does not wait (a timer that is always due)
    /// has many. A test uses it to see that an idle service sleeps.
    #[must_use]
    pub fn runden(&self) -> u64 {
        self.runden.load(Ordering::Relaxed)
    }
}

/// What the result of one read means for the loop.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Gelesen {
    /// This many bytes came.
    Daten(usize),
    /// The connection ended or cannot be read any more.
    Ende,
    /// The read was interrupted or would block: nothing now (the wait says when there is more).
    Nochmal,
}

pub(crate) fn gelesen(ergebnis: std::io::Result<usize>) -> Gelesen {
    match ergebnis {
        Ok(0) => Gelesen::Ende,
        Ok(n) => Gelesen::Daten(n),
        Err(e) if matches!(e.kind(), ErrorKind::Interrupted | ErrorKind::WouldBlock) => {
            Gelesen::Nochmal
        }
        Err(_) => Gelesen::Ende,
    }
}

/// What the result of `accept` means for the loop.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Annahme<T> {
    /// A connection.
    Neu(T),
    /// Interrupted: ask again.
    Nochmal,
    /// Nothing more to accept now (or the listener is broken).
    Keine,
}

pub(crate) fn annahme<T>(ergebnis: std::io::Result<T>) -> Annahme<T> {
    match ergebnis {
        Ok(strom) => Annahme::Neu(strom),
        Err(e) if e.kind() == ErrorKind::Interrupted => Annahme::Nochmal,
        Err(_) => Annahme::Keine,
    }
}

/// Milliseconds from `jetzt` to the timer `frist`; negative when it is due already.
pub(crate) fn rest_ms(frist: i64, jetzt: i64) -> i64 {
    frist - jetzt
}

/// `true` if a viewer may get frames: it exists and is not ending after a protocol error.
pub(crate) fn sendbar(betrachter: Option<&Betrachter>) -> bool {
    betrachter.is_some_and(|b| !b.schliessen)
}

/// `true` if a queue got shorter by a write: only then the report `Socket` follows (DI5).
pub(crate) fn kuerzer(vorher: usize, nachher: usize) -> bool {
    nachher < vorher
}

/// `true` if a viewer has frames that the socket does not take at the moment (a muted viewer has none that count).
pub(crate) fn hat_rueckstand(betrachter: &BTreeMap<u64, Betrachter>) -> bool {
    betrachter
        .values()
        .any(|b| !b.schlange.is_empty() && !b.stumm)
}

/// Remembers the size of the last accepted `Abo` and `Groesse` of a viewer for one session (for the rebuild of the core, DI6):
/// a `Abo` with `an` true or without `an` sets it, with `an` false it is forgotten, a `Groesse` changes the size of an existing one.
pub(crate) fn abo_nachfuehren(
    abos: &mut BTreeMap<String, (i64, i64)>,
    sid: &str,
    art: &str,
    e: &Value,
) {
    let groesse = ganz(e.get("spalten")).zip(ganz(e.get("zeilen")));
    let an = e.get("an");
    if art == "Abo" && matches!(an, None | Some(Value::Bool(true))) {
        if let Some(g) = groesse {
            abos.insert(sid.to_owned(), g);
        }
    } else if art == "Abo" && an == Some(&Value::Bool(false)) {
        abos.remove(sid);
    } else if art == "Groesse"
        && abos.contains_key(sid)
        && let Some(g) = groesse
    {
        abos.insert(sid.to_owned(), g);
    }
}

/// How long the loop waits for the next turn: the beat, shorter when a timer of the core is due sooner.
pub(crate) fn warte_dauer(rueckstand: bool, bis_frist_ms: Option<i64>) -> Duration {
    let takt = if rueckstand { TAKT_STAU } else { TAKT };
    match bis_frist_ms {
        Some(ms) => takt.min(Duration::from_millis(u64::try_from(ms).unwrap_or(0))),
        None => takt,
    }
}

/// `ganz` of the reference: a whole number that is no truth value.
fn ganz(wert: Option<&Value>) -> Option<i64> {
    wert.and_then(Value::as_i64)
}

/// The viewer of the next core joins the one of the cores before (DI12, `Zustand`): the `abos` are added in the order of the cores,
/// `verfallen` is the sum, `rueckstand` holds the entries of both; `ausstehend` stays that of the first core (the room core).
pub(crate) fn betrachter_zusammenfuehren(da: &mut Map<String, Value>, neu: &Value) {
    if let (Some(Value::Array(a)), Some(Value::Array(n))) = (da.get_mut("abos"), neu.get("abos")) {
        a.extend(n.iter().cloned());
    }
    let summe = da.get("verfallen").and_then(Value::as_u64).unwrap_or(0)
        + neu.get("verfallen").and_then(Value::as_u64).unwrap_or(0);
    da.insert("verfallen".to_owned(), json!(summe));
    if let (Some(Value::Object(r)), Some(Value::Object(n))) =
        (da.get_mut("rueckstand"), neu.get("rueckstand"))
    {
        r.extend(n.clone());
    }
}

/// The keys of `wert` named in `namen`, as an object.
fn waehle(wert: &Value, namen: &[&str]) -> Value {
    let mut aus = Map::new();
    if let Some(karte) = wert.as_object() {
        for n in namen {
            if let Some(w) = karte.get(*n) {
                aus.insert((*n).to_owned(), w.clone());
            }
        }
    }
    Value::Object(aus)
}

/// The service.
pub struct Dienst {
    pub(crate) cfg: Startdatei,
    hoerer: Option<UnixListener>,
    /// What the loop waits for: the wake socket, the listener and every connection.
    bereit: Bereitschaft,
    /// The write end, for the handles of [`Anhalter`].
    weck_schreib: Arc<UnixStream>,
    halt: Arc<AtomicBool>,
    runden: Arc<AtomicU64>,
    /// The cores in the order of their start: the room core first (only with `lauf`), then the session of `kind` (number 0), then
    /// the sessions of the Bestand (DI-L5 point 4), then the sessions of `Starten` (DI12). An index stays valid while a step
    /// waits: a session leaves only at the end of a turn (P26) or by `Loeschen`, between two events (DI-L3 point 5).
    pub(crate) sitzungen: Vec<Sitzung>,
    pub(crate) betrachter: BTreeMap<u64, Betrachter>,
    folge: u64,
    start: Instant,
    t_zuletzt: i64,
    t_ein: i64,
    arbeit: VecDeque<Arbeit>,
    neubau_tiefe: u32,
    fatal: bool,
    socket_da: bool,
    kern_grenze: usize,
    huelle: Option<PathBuf>,
    /// The socket folder, opened once after the check of its rights; every removal below it goes through this descriptor (`loeschen.rs`).
    pub(crate) wurzel: Arc<Ordner>,
    /// The markers of a trial run, read once from `lauf.marker_datei` at the start; they go into every `start.json` and nowhere else.
    marker: Option<start::Marker>,
    /// The identifiers `Starten` handed out so far (`s-<n>`, never used again, DI12 point 4); not the raster number.
    pub(crate) naechste: usize,
    /// The raster numbers handed out in this run, by number (DI12 point 4, P25): a number never handed out is taken first.
    vergeben: [bool; 256],
    /// The ends of sessions so far (the order of `Sitzung::beendet_als`).
    enden: u64,
    /// P26: the sessions out of the `Sitzungsliste` (more than `lauf.beendet_max` ended), by identifier; they are dropped at the end
    /// of the turn of the loop ([`Self::entfernte_wegnehmen`]).
    pub(crate) entfernt: BTreeSet<String>,
    /// The content of the last `Startwahl` that went out (without `t`).
    startwahl_letzt: Option<Value>,
    /// The list of the workers that went to everyone last (DI-L1 point 4).
    pub(crate) agenten_letzt: Vec<Value>,
    /// The raster numbers of deleted sessions with the place of their end (DI-L3 point 5): free, like the number of an ended session.
    pub(crate) frei_geloescht: Vec<(u64, u8)>,
    /// The adopted children and the hunts for the orphans of ended sessions (Linux, `waisen.rs`).
    pub(crate) waisen: Waisen,
}

/// DI12 point 14: the marker file is read exactly once, at the start of the service (after the start file and the rights, before the socket).
/// No file or no key gives no markers.
fn marker_lesen(cfg: &Startdatei) -> Result<Option<start::Marker>, StartFehler> {
    let Some(lauf) = cfg.lauf.as_ref() else {
        return Ok(None);
    };
    match lauf.marker_datei.as_deref() {
        Some(pfad) => crate::markerdatei::lesen(pfad, lauf.marker_ordner.as_deref()),
        None => Ok(None),
    }
}

/// What a test may set. The program `wb-dienst` sets nothing: it always runs with [`Optionen::default`].
#[derive(Debug, Clone, Default)]
pub struct Optionen {
    /// The program the child is started through (`wb-dienst`, see `huelle.rs`); the running program if `None`. A test that runs
    /// the service in its own process names the built `wb-dienst` here.
    pub huelle: Option<PathBuf>,
    /// The most bytes the messages of one step of the core may hold (DI6); [`zustandskern::NACHRICHTEN_MAX`] if `None`. A test
    /// lowers it to reach the error of the core with a small output.
    pub kern_grenze: Option<usize>,
}

impl Dienst {
    /// Starts the child, builds the core and binds the socket, in this order (DI8): so there is never a socket while a start
    /// error of the child is still possible. The rights are checked before ([`crate::rechte_pruefen`]).
    ///
    /// # Errors
    /// Exit code 4: the child cannot be started. Exit code 1: the socket cannot be bound (the child is ended again).
    pub fn neu(cfg: Startdatei) -> Result<Self, StartFehler> {
        Self::neu_mit(cfg, &Optionen::default())
    }

    /// Like [`Dienst::neu`] with the options of a test.
    ///
    /// # Errors
    /// See [`Dienst::neu`].
    #[allow(clippy::too_many_lines)] // the order of the start (DI8, DI-L5 point 4) reads in one place
    pub fn neu_mit(cfg: Startdatei, optionen: &Optionen) -> Result<Self, StartFehler> {
        let kern_grenze = optionen
            .kern_grenze
            .unwrap_or(zustandskern::NACHRICHTEN_MAX);
        // section 17, point 1: a subreaper that cannot list its children would end every hunt silently (fail closed)
        if !crate::waisen::kinder_lesbar() {
            return Err(StartFehler::neu(1, crate::waisen::KINDER_FEHLT));
        }
        let wurzel = Arc::new(ordner_oeffnen(Path::new(&cfg.ordner), crate::sys::euid())?);
        let marker = marker_lesen(&cfg)?;
        // DI-L5 point 4: the Bestand is read once, after the rights and before any child starts
        let belegt: Vec<&str> = if cfg.mit_kind {
            vec![cfg.sid.as_str()]
        } else {
            Vec::new()
        };
        let bestand = crate::bestand::lesen(&cfg, &belegt)?;
        let mut sitzungen = Vec::new();
        if cfg.lauf.is_some() {
            let kern = zustandskern::Sitzungskern::neu_mit(&anfang_raum(&cfg.anfang), kern_grenze)
                .map_err(|e| StartFehler::neu(2, &format!("anfang: {e}")))?;
            sitzungen.push(Sitzung::raum(kern));
        }
        if cfg.mit_kind {
            let kind = kind_starten(&KindStart::von_start(&cfg), optionen.huelle.as_deref())?;
            let anfang = anfang_fuer(&cfg.anfang, None, &Traegerstand::anfang(cfg.groesse));
            let kern = match zustandskern::Sitzungskern::neu_mit(&anfang, kern_grenze) {
                Ok(k) => k,
                Err(e) => {
                    start_verwerfen(vec![kind]);
                    return Err(StartFehler::neu(2, &format!("anfang: {e}")));
                }
            };
            let mut sitzung = Sitzung::neu(cfg.sid.clone(), kern, kind);
            sitzung.arbeitsordner = Some(PathBuf::from(&cfg.cwd));
            sitzungen.push(mit_job(sitzung)?);
        }
        let aus_bestand = sitzungen.len();
        let gestartet = match Self::bestand_aufnehmen(&cfg, bestand, kern_grenze, &mut sitzungen) {
            Ok(n) => n,
            Err(f) => {
                start_verwerfen(Self::kinder_nehmen(&mut sitzungen));
                return Err(f);
            }
        };
        let hoerer = match socket_binden(Path::new(&cfg.pfad), wurzel.as_fd()) {
            Ok(h) => h,
            Err(f) => {
                start_verwerfen(Self::kinder_nehmen(&mut sitzungen));
                return Err(f);
            }
        };
        if let Err(e) = hoerer.set_nonblocking(true) {
            drop(hoerer);
            crate::loeschen::datei_entfernen(wurzel.as_fd(), &cfg.name);
            start_verwerfen(Self::kinder_nehmen(&mut sitzungen));
            return Err(StartFehler::neu(1, &format!("nonblocking: {}", e.kind())));
        }
        let (weck_lese, weck_schreib) = match weckpaar() {
            Ok(paar) => paar,
            Err(e) => {
                drop(hoerer);
                crate::loeschen::datei_entfernen(wurzel.as_fd(), &cfg.name);
                start_verwerfen(Self::kinder_nehmen(&mut sitzungen));
                return Err(StartFehler::neu(1, &format!("wake socket: {}", e.kind())));
            }
        };
        let bereit = Bereitschaft::neu()
            .and_then(|mut b| {
                b.hinzu(&weck_lese, SCHLUESSEL_WECK)?;
                b.hinzu(&hoerer, SCHLUESSEL_HOERER)?;
                Ok(b)
            })
            .map_err(|e| {
                crate::loeschen::datei_entfernen(wurzel.as_fd(), &cfg.name);
                StartFehler::neu(1, &format!("poll: {}", e.kind()))
            });
        let bereit = match bereit {
            Ok(b) => b,
            Err(f) => {
                drop(hoerer);
                start_verwerfen(Self::kinder_nehmen(&mut sitzungen));
                return Err(f);
            }
        };
        // the ends of the sessions of the Bestand come first in the order of the ends
        let enden = sitzungen
            .iter()
            .filter_map(|s| s.beendet_als)
            .max()
            .unwrap_or_default();
        let mut dienst = Self {
            cfg,
            hoerer: Some(hoerer),
            bereit,
            weck_schreib: Arc::new(weck_schreib),
            halt: Arc::new(AtomicBool::new(false)),
            runden: Arc::new(AtomicU64::new(0)),
            sitzungen,
            betrachter: BTreeMap::new(),
            folge: 0,
            start: Instant::now(),
            t_zuletzt: 0,
            t_ein: 0,
            arbeit: VecDeque::new(),
            entfernt: BTreeSet::new(),
            neubau_tiefe: 0,
            fatal: false,
            socket_da: true,
            kern_grenze,
            huelle: optionen.huelle.clone(),
            wurzel,
            marker,
            naechste: gestartet,
            vergeben: [false; 256],
            enden,
            startwahl_letzt: None,
            agenten_letzt: Vec::new(),
            frei_geloescht: Vec::new(),
            waisen: Waisen::default(),
        };
        dienst.bestand_beenden(aus_bestand);
        Ok(dienst)
    }

    /// The sessions of the Bestand join the list after the session of `kind` (DI-L5 point 4); returns `gestartet`.
    fn bestand_aufnehmen(
        cfg: &Startdatei,
        bestand: Option<crate::bestand::Bestand>,
        kern_grenze: usize,
        sitzungen: &mut Vec<Sitzung>,
    ) -> Result<usize, StartFehler> {
        let (Some(b), Some(lauf)) = (bestand, cfg.lauf.as_ref()) else {
            return Ok(0);
        };
        for (ende, x) in (1_u64..).zip(&b.eintraege) {
            sitzungen.push(Self::sitzung_aus_bestand(cfg, lauf, x, ende, kern_grenze)?);
        }
        Ok(b.gestartet)
    }

    /// DI-L5 point 4: the core of a session of the Bestand (from index `ab` on) has ended and refuses every input with
    /// `sitzung_beendet`; the list of the workers counts as sent.
    fn bestand_beenden(&mut self, ab: usize) {
        for k in ab..self.sitzungen.len() {
            let s = &self.sitzungen[k];
            let e = json!({"t": 0, "art": "Kindende", "sitzung": s.sid, "code": s.kind_ende.unwrap_or(0)});
            self.ereignis(k, e, None);
        }
        self.agenten_letzt = self.agenten_liste();
    }

    /// One entry of the Bestand as an ended session without a raster number (DI-L5 point 4): the core is built from its entry like a
    /// session of `Starten`; an entry that still ran is `beendet` with `abgestuerzt` and the reason `dienst.neustart`, without a code.
    fn sitzung_aus_bestand(
        cfg: &Startdatei,
        lauf: &crate::startdatei::Lauf,
        x: &Map<String, Value>,
        ende: u64,
        kern_grenze: usize,
    ) -> Result<Sitzung, StartFehler> {
        let text = |k: &str| {
            x.get(k)
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned()
        };
        let wahl = |k: &str| x.get(k).and_then(Value::as_str).map(str::to_owned);
        let sid = text("id");
        let meta = [
            text("harness"),
            text("modell"),
            text("route"),
            text("kunde"),
        ];
        let eintrag = eintrag_fuer(&sid, &meta[2], &cfg.raum, &cfg.person, lauf.groesse);
        let anfang = anfang_fuer(
            &cfg.anfang,
            Some(&eintrag),
            &Traegerstand::anfang(lauf.groesse),
        );
        let kern = zustandskern::Sitzungskern::neu_mit(&anfang, kern_grenze)
            .map_err(|_| StartFehler::neu(2, "lauf.bestand ungueltig"))?;
        let mut s = Sitzung::aus_bestand(sid, kern, eintrag, meta);
        let a = &mut s.angaben;
        a.name = text("name");
        a.ordner = text("ordner");
        a.start = x.get("start").cloned();
        a.orchestrator = wahl("orchestrator");
        a.rolle = wahl("rolle");
        a.fortsetzung_von = wahl("fortsetzung_von");
        a.fortgesetzt_als = wahl("fortgesetzt_als");
        let code = x
            .get("code")
            .and_then(Value::as_u64)
            .and_then(|c| u32::try_from(c).ok());
        if x.get("zustand").and_then(Value::as_str) == Some("laeuft") {
            a.ende = Some(crate::verwaltung::EndeArt::Neustart);
            s.grund = Some("dienst.neustart".to_owned());
            s.kind_ende = Some(0);
            s.angaben.ohne_code = true;
        } else {
            a.ende = Some(if x.get("abgestuerzt") == Some(&Value::Bool(true)) {
                crate::verwaltung::EndeArt::Selbst
            } else {
                crate::verwaltung::EndeArt::Beenden
            });
            s.grund = wahl("grund");
            s.kind_ende = Some(code.unwrap_or(0));
            s.angaben.ohne_code = code.is_none();
        }
        s.arbeitsordner = Some(PathBuf::from(&s.angaben.ordner));
        s.beendet_als = Some(ende);
        Ok(s)
    }

    /// Takes the children out of the sessions.
    pub(crate) fn kinder_nehmen(sitzungen: &mut [Sitzung]) -> Vec<traeger::Kind> {
        sitzungen.iter_mut().filter_map(|s| s.kind.take()).collect()
    }

    /// A handle that ends [`Dienst::schleife`] from another thread.
    #[must_use]
    pub fn anhalter(&self) -> Anhalter {
        Anhalter {
            halt: Arc::clone(&self.halt),
            weck: Arc::clone(&self.weck_schreib),
            runden: Arc::clone(&self.runden),
        }
    }

    /// The loop until SIGTERM or SIGINT; then viewers, socket and child are closed and the exit code 0 is returned.
    ///
    /// The signals are caught before, with [`signale_anmelden`] and before anything becomes visible to the outside; a signal
    /// that came during the start ends the loop at once and cleanly.
    #[cfg(unix)]
    pub fn laufen(&mut self, mut signale: signal_hook::iterator::Signals) -> i32 {
        let handle = signale.handle();
        let anhalter = self.anhalter();
        let faden = std::thread::spawn(move || {
            if signale.forever().next().is_some() {
                anhalter.anhalten();
            }
        });
        let code = self.schleife();
        handle.close();
        let _ = faden.join();
        code
    }

    /// Windows (Nachtrag 25.11.2026, contract section 21): the loop until one of the console events `CTRL_C_EVENT` or
    /// `CTRL_BREAK_EVENT` comes; then the same end as under Unix (`sys::laufen`).
    #[cfg(windows)]
    pub fn laufen(&mut self, signale: Signale) -> i32 {
        crate::sys::laufen(self, signale)
    }

    /// The loop until [`Anhalter::anhalten`] is called; then the same end as [`Dienst::laufen`]. It installs no signal handler.
    pub fn schleife(&mut self) -> i32 {
        let mut puffer = vec![0_u8; LESE_GROESSE];
        while !self.halt.load(Ordering::SeqCst) {
            self.runden.fetch_add(1, Ordering::Relaxed);
            let frist = warte_dauer(hat_rueckstand(&self.betrachter), self.bis_frist());
            // the keys that are ready in the order of their readiness (a viewer number, or the key of the wake socket or the
            // listener, which no viewer has: `lesen_von` ignores it, the listener is accepted from below)
            for nr in self.bereit.warten(frist) {
                self.lesen_von(nr, &mut puffer);
            }
            self.annehmen();
            self.meldungen_abfragen();
            self.kinder_lesen();
            self.flush_alle();
            self.takt_pruefen();
            self.kill_pruefen();
            self.waisen_pruefen();
            self.entfernte_wegnehmen();
            if self.fatal {
                self.aufraeumen();
                return 1;
            }
        }
        self.aufraeumen();
        0
    }

    /// Reads once from the connection `nr` and handles what came; the end of the connection ends the viewer. What comes on a
    /// connection that ends after a protocol error is read and dropped (so that its end is seen).
    fn lesen_von(&mut self, nr: u64, puffer: &mut [u8]) {
        let Some(b) = self.betrachter.get(&nr) else {
            return;
        };
        match gelesen((&b.strom).read(puffer)) {
            Gelesen::Daten(n) => {
                if let Some(b) = self.betrachter.get_mut(&nr).filter(|b| !b.schliessen) {
                    b.eingang.anhaengen(&puffer[..n]);
                    self.eingang_verarbeiten(nr);
                    self.flush_alle();
                }
            }
            Gelesen::Nochmal => {}
            Gelesen::Ende => self.getrennt(nr),
        }
    }

    // ---------------------------------------------------------------------------------------------- time (DI7)

    /// The clock: in test mode what the frames `Uhr` set, else the milliseconds since the start, never falling.
    pub(crate) fn jetzt(&mut self) -> i64 {
        if self.cfg.pruefmodus {
            return self.t_ein;
        }
        let t = i64::try_from(self.start.elapsed().as_millis()).unwrap_or(i64::MAX);
        self.t_zuletzt = self.t_zuletzt.max(t);
        self.t_zuletzt
    }

    /// Milliseconds until the next timer of a core, `None` in test mode (the clock stands still there) or without a timer.
    fn bis_frist(&mut self) -> Option<i64> {
        if self.cfg.pruefmodus {
            return None;
        }
        let jetzt = self.jetzt();
        self.sitzungen
            .iter()
            .filter_map(|s| s.kern.naechste_frist())
            .min()
            .map(|f| rest_ms(f, jetzt))
    }

    /// Gives `Takt` to the cores whose timer is due (DI7, operation only).
    fn takt_pruefen(&mut self) {
        if self.bis_frist().is_some_and(|ms| ms <= 0) {
            let t = self.jetzt();
            self.an_alle_kerne(&json!({"t": t, "art": "Takt"}));
            self.flush_alle();
        }
    }

    /// SIGKILL for a child that `Beenden` asked to end and that ignored SIGHUP for a second (DI12 point 6).
    fn kill_pruefen(&mut self) {
        let jetzt = Instant::now();
        for s in &mut self.sitzungen {
            s.kill_pruefen(jetzt);
        }
    }

    /// The orphans (contract, section 17): a session whose child has ended starts the hunt for its orphans, and one look collects
    /// the ended adopted children and signals the orphans of the hunted sessions. The own children of the sessions are left out.
    fn waisen_pruefen(&mut self) {
        let jetzt = Instant::now();
        if !self.waisen.faellig(jetzt) {
            return;
        }
        for s in &mut self.sitzungen {
            if let Some(sid) = s.waisen_ab() {
                self.waisen.jagen(sid, jetzt);
            }
        }
        let eigene = self.eigene_kinder();
        let laufende: Vec<u32> = self
            .sitzungen
            .iter()
            .filter(|s| !s.gejagt)
            .filter_map(Sitzung::eigenes_kind)
            .collect();
        self.waisen.runde(&eigene, &laufende, jetzt);
    }

    /// The pids of the own children of the sessions that the carriers have not collected yet: no orphans (`waisen.rs`), and never
    /// signalled by the end of another session (`aufraeumen.rs`).
    fn eigene_kinder(&self) -> Vec<u32> {
        kinder_der(&self.sitzungen)
    }

    /// `Verbinden`, `Trennen`, `Takt`, `Socket` go to every core: the room core first, then the sessions in the order of their
    /// start (DI12 point 2).
    fn an_alle_kerne(&mut self, e: &Value) {
        for k in 0..self.sitzungen.len() {
            self.ereignis(k, e.clone(), None);
        }
    }

    // ---------------------------------------------------------------------------------------------- core steps

    /// One step of the core `k`. The time of the event never falls below that of the step before (the events that wait in
    /// `arbeit` were made a little earlier than the ones that came in between). `Err`: the core is spent (DI6).
    fn schritt(&mut self, k: usize, mut e: Value) -> Result<Wirkungen, ()> {
        let s = self.sitzungen.get_mut(k).ok_or(())?;
        let t = e
            .get("t")
            .and_then(Value::as_i64)
            .unwrap_or(0)
            .max(s.zuletzt);
        e["t"] = json!(t);
        s.zuletzt = t;
        s.kern.schritt(&e).map_err(|_| ())
    }

    /// A step and the carrying out of its effects; a spent core is built again.
    fn schritt_und_wirkungen(&mut self, k: usize, e: Value, quelle: Option<u64>) {
        match self.schritt(k, e) {
            Ok(w) => self.wirkungen(k, w, quelle),
            Err(()) => self.kern_neu(k),
        }
    }

    /// An event for the core, then every event that waits (the reports `Socket`, breadth first, DI5).
    fn ereignis(&mut self, k: usize, e: Value, quelle: Option<u64>) {
        self.schritt_und_wirkungen(k, e, quelle);
        self.arbeit_abarbeiten();
    }

    fn arbeit_abarbeiten(&mut self) {
        while let Some((e, quelle, k)) = self.arbeit.pop_front() {
            self.schritt_und_wirkungen(k, e, quelle);
        }
    }

    /// The effects of a step in the order of DI4: sizes, then bytes to the PTY (DI8), then the messages per viewer by number,
    /// then the rejection to the sender, then the events of the carrier for a new size (they make the next step). `protokoll`, `an_stelle` and `abfragen` are not passed on (V).
    ///
    /// With `lauf` (DI12 point 2) the messages `Einstellungen`, `Routen`, `Anfragen`, `Agenten` and `Leeren` come from the room
    /// core only, and the `Sitzungsliste` of a core is never passed on: the room core's place in the order is kept and the
    /// service's own list takes it.
    fn wirkungen(&mut self, k: usize, w: Wirkungen, quelle: Option<u64>) {
        // The events the carrier gives for a new size (rows cut or reflowed) go to the core only after the messages of this step are
        // out: the step holds the full picture (`Abzug`) for the new size, and a change against it, sent first, would meet a model
        // picture of an older stand and end the connection (`raster_luecke`).
        let mut nach_der_groesse: Vec<Ereignis> = Vec::new();
        for g in &w.groessen {
            nach_der_groesse.extend(self.groesse_setzen(k, g));
        }
        for p in &w.pty {
            let bytes = unhex(p.get("hex").and_then(Value::as_str).unwrap_or(""));
            if let Some(kind) = self.sitzungen.get_mut(k).and_then(|s| s.kind.as_mut()) {
                // the record of the test build (contract section 21c)
                #[cfg(all(windows, feature = "pruefbau"))]
                crate::sys::mitschrift_eingabe(k, &bytes);
                let _ = kind.schreibe(&bytes);
            }
        }
        let mut je_nr: Vec<(u64, Vec<Value>)> = w
            .nachrichten
            .into_iter()
            .filter_map(|(name, liste)| Some((name.strip_prefix('b')?.parse().ok()?, liste)))
            .collect();
        je_nr.sort_by_key(|(nr, _)| *nr);
        let raum = self.sitzungen.get(k).is_some_and(Sitzung::ist_raum);
        for (nr, liste) in &je_nr {
            for nachricht in liste {
                if !sendbar(self.betrachter.get(nr)) {
                    break;
                }
                let art = nachricht.get("art").and_then(Value::as_str);
                if self.cfg.lauf.is_some() {
                    match art {
                        Some("Sitzungsliste") if raum => {
                            let liste = self.liste_nachricht();
                            self.nachricht_an(*nr, &liste, None);
                            continue;
                        }
                        Some("Sitzungsliste") => continue,
                        // DI-L1 point 3: the room core has no workers; the service's own list takes the place of its message
                        Some("Agenten") if raum => {
                            let agenten = self.agenten_nachricht();
                            self.nachricht_an(*nr, &agenten, None);
                            continue;
                        }
                        Some("Einstellungen" | "Routen" | "Anfragen" | "Agenten" | "Leeren")
                            if !raum =>
                        {
                            continue;
                        }
                        _ => {}
                    }
                }
                self.nachricht_an(*nr, nachricht, Some(k));
            }
        }
        if let Some(nr) = quelle {
            for ablehnung in &w.ablehnungen {
                if sendbar(self.betrachter.get(&nr)) {
                    let grund = ablehnung.get("grund").cloned().unwrap_or(Value::Null);
                    self.ablehnen(nr, &json!({"grund": grund}));
                }
            }
        }
        for x in &nach_der_groesse {
            self.traeger_ereignis(k, x);
        }
    }

    /// The frame `Ablehnung` to one viewer.
    pub(crate) fn ablehnen(&mut self, nr: u64, nutzlast: &Value) {
        self.senden(nr, json_rahmen(A_ABLEHNUNG, nutzlast));
    }

    /// The effect `groessen`: the PTY and the screen get the size; the caller passes the events of the carrier on to the core (DI8).
    fn groesse_setzen(&mut self, k: usize, g: &Value) -> Vec<Ereignis> {
        let zahl = |n: &str| {
            g.get(n)
                .and_then(Value::as_u64)
                .and_then(|n| usize::try_from(n).ok())
        };
        let (Some(spalten), Some(zeilen)) = (zahl("spalten"), zahl("zeilen")) else {
            return Vec::new();
        };
        // the record of the test build (contract section 21c)
        #[cfg(all(windows, feature = "pruefbau"))]
        crate::sys::mitschrift_groesse(k, spalten, zeilen);
        let ereignisse = self
            .sitzungen
            .get_mut(k)
            .and_then(|s| s.kind.as_mut())
            .and_then(|kind| kind.groesse(spalten, zeilen).ok())
            .unwrap_or_default();
        #[cfg(all(windows, feature = "pruefbau"))]
        crate::sys::mitschrift_ereignisse(k, &ereignisse);
        ereignisse
    }

    /// An event of the carrier goes to the core with the session and the time (DI8). The end of the child makes the session
    /// `beendet` and the new list goes to every viewer (DI12 point 5).
    fn traeger_ereignis(&mut self, k: usize, x: &Ereignis) {
        // the own children of the sessions, for the end of this one (read before the session is borrowed)
        let eigene = if let Ereignis::Kindende { .. } = x {
            self.eigene_kinder()
        } else {
            Vec::new()
        };
        let Some(s) = self.sitzungen.get_mut(k) else {
            return;
        };
        // section 18: the code of the end is the one of the report when `lauf` gave one; the core gets the same code
        let mut gemeldet = None;
        match x {
            Ereignis::Invers { an, .. } => s.invers = *an,
            Ereignis::KlammerPaste { an } => s.bracketed_paste = *an,
            Ereignis::Kindende { code } => {
                let code = s.ende_auswerten(*code);
                gemeldet = Some(Ereignis::Kindende { code });
                s.kind_ende = Some(code);
                s.angaben.ende = Some(if s.angaben.beendet_von_uns {
                    crate::verwaltung::EndeArt::Beenden
                } else {
                    crate::verwaltung::EndeArt::Selbst
                });
                s.kill_ab = None;
                self.enden += 1;
                s.beendet_als = Some(self.enden);
                // SECURITY: every end of a session cleans up, before the list shows `beendet`
                s.ende_aufraeumen(&eigene);
                // P26: the PTY and the threads of the carrier go back now; the entry, the core and the last picture stay
                if let Some(kind) = s.kind.as_mut() {
                    kind.freigeben();
                }
            }
            _ => {}
        }
        let x = gemeldet.as_ref().unwrap_or(x);
        let sid = s.sid.clone();
        let mut e = x.zu_json();
        e["t"] = json!(self.jetzt());
        e["sitzung"] = json!(sid);
        self.ereignis(k, e, None);
        if matches!(x, Ereignis::Kindende { .. }) && self.cfg.lauf.is_some() {
            self.beendete_begrenzen(k);
            self.liste_und_agenten();
        }
    }

    /// DI12 point 5 (P26, Projektentscheid, 23.11.2026): the list holds at most `lauf.beendet_max` ended sessions of `Starten`. The end of the
    /// session `k` itself takes nothing out of the list for it; while more than the limit have ended, the one that ended longest ago
    /// and that no viewer subscribes goes into `entfernt` (it leaves the list at once, the session goes at the end of the turn,
    /// [`Self::entfernte_wegnehmen`]). With every old entry subscribed, the list holds more until a later end.
    fn beendete_begrenzen(&mut self, k: usize) {
        let Some(max) = self.cfg.lauf.as_ref().map(|l| l.beendet_max) else {
            return;
        };
        loop {
            let entfernt = &self.entfernt;
            let beendet = |s: &Sitzung| {
                s.eintrag.is_some()
                    && s.beendet_als.is_some()
                    && !s.sid.as_ref().is_some_and(|sid| entfernt.contains(sid))
            };
            if self.sitzungen.iter().filter(|s| beendet(s)).count() <= max {
                return;
            }
            let aeltester = self
                .sitzungen
                .iter()
                .enumerate()
                .filter(|(i, s)| *i != k && beendet(s))
                .filter(|(_, s)| {
                    let sid = s.sid.as_deref().unwrap_or_default();
                    !self.betrachter.values().any(|b| b.abos.contains_key(sid))
                })
                .min_by_key(|(_, s)| s.beendet_als)
                .map(|(i, _)| i);
            match aeltester.and_then(|i| self.sitzungen.get(i)?.sid.clone()) {
                Some(sid) => {
                    self.entfernt.insert(sid);
                }
                None => return,
            }
        }
    }

    /// Drops the sessions marked `entfernt` (P26), at the end of a turn of the loop, when no step waits (the indexes of the sessions
    /// change). The raster number of a dropped session counts as never handed out again, and every connection forgets it (RA R13).
    fn entfernte_wegnehmen(&mut self) {
        if !self.arbeit.is_empty() || self.entfernt.is_empty() {
            return;
        }
        let weg = std::mem::take(&mut self.entfernt);
        let ist_weg = |s: &Sitzung| s.sid.as_ref().is_some_and(|sid| weg.contains(sid));
        let nummern: Vec<u8> = self
            .sitzungen
            .iter()
            .filter(|s| ist_weg(s))
            .filter_map(|s| s.nr)
            .collect();
        for nummer in nummern {
            for b in self.betrachter.values_mut() {
                b.raster.vergessen(nummer);
            }
            self.vergeben[usize::from(nummer)] = false;
        }
        // `Kind::drop`: the child has ended and was collected; the carrier was given back at `Kindende`
        self.sitzungen.retain(|s| !ist_weg(s));
    }

    /// DI6: the core gave an error and is spent. Builds it again from the state of the carrier (lines, `stand`, modes and
    /// cursor from the same moment) and every viewer gets a full picture, as if it had subscribed anew. The effects of the
    /// failed step are lost. Only this core is built again (DI12); the room core has no carrier and gets the start without
    /// sessions.
    fn kern_neu(&mut self, idx: usize) {
        if self.neubau_tiefe >= 4 {
            self.fatal = true;
            return;
        }
        let Some(sitz) = self.sitzungen.get(idx) else {
            return;
        };
        let anfang = if sitz.ist_raum() {
            anfang_raum(&self.cfg.anfang)
        } else {
            let Some(stand) = sitz.traegerstand() else {
                return;
            };
            anfang_fuer(&self.cfg.anfang, sitz.eintrag.as_ref(), &stand)
        };
        let Ok(kern) = zustandskern::Sitzungskern::neu_mit(&anfang, self.kern_grenze) else {
            self.fatal = true;
            return;
        };
        let (sid, kind_ende) = {
            let Some(sitz) = self.sitzungen.get_mut(idx) else {
                return;
            };
            sitz.kern = kern;
            sitz.kernbau += 1;
            (sitz.sid.clone(), sitz.kind_ende)
        };
        self.neubau_tiefe += 1;
        let t = self.jetzt();
        if let (Some(code), Some(sid)) = (kind_ende, &sid) {
            let ev = json!({"t": t, "art": "Kindende", "sitzung": sid, "code": code});
            self.ereignis(idx, ev, None);
        }
        let nummern: Vec<u64> = self.betrachter.keys().copied().collect();
        for nr in nummern {
            let Some(b) = self.betrachter.get(&nr) else {
                continue;
            };
            if b.schliessen {
                continue;
            }
            let abo = sid.as_ref().and_then(|sid| b.abos.get(sid).copied());
            let ev = self.verbinden_ereignis(t, &b.name);
            self.ereignis(idx, ev, None);
            if let (Some((spalten, zeilen)), Some(sid)) = (abo, &sid) {
                let name = format!("b{nr}");
                let ev = json!({"t": t, "art": "Abo", "betrachter": name, "sitzung": sid, "an": true,
                               "spalten": spalten, "zeilen": zeilen});
                self.ereignis(idx, ev, None);
            }
        }
        self.flush_alle();
        self.neubau_tiefe -= 1;
    }

    /// The event `Verbinden` of a viewer: the person, room and view of the start file are the only identity (DI4).
    fn verbinden_ereignis(&self, t: i64, name: &str) -> Value {
        json!({"t": t, "art": "Verbinden", "betrachter": name, "person": self.cfg.person,
               "raum": self.cfg.raum, "ansicht": self.cfg.ansicht})
    }

    // ---------------------------------------------------------------------------------------------- viewers (DI4)

    fn annehmen(&mut self) {
        loop {
            let Some(hoerer) = &self.hoerer else {
                return;
            };
            let strom = match annahme(hoerer.accept().map(|(strom, _)| strom)) {
                Annahme::Neu(strom) => strom,
                Annahme::Nochmal => continue,
                Annahme::Keine => return,
            };
            // on some systems an accepted socket does not take the mode of the listener: the loop wants it not to block
            if strom.set_nonblocking(true).is_err() {
                continue;
            }
            self.folge += 1;
            let nr = self.folge;
            if self.bereit.hinzu(&strom, nr).is_err() {
                continue;
            }
            let b = Betrachter::neu(nr, strom, self.cfg.deflate);
            let name = b.name.clone();
            self.betrachter.insert(b.nr, b);
            let t = self.jetzt();
            let e = self.verbinden_ereignis(t, &name);
            self.an_alle_kerne(&e);
            // the last message of the connect set (DI12 point 10)
            self.startwahl_senden(Some(nr), false);
        }
    }

    /// The sockets of the reports of `lauf` (section 18): accepts the one connection and reads what came, without blocking.
    fn meldungen_abfragen(&mut self) {
        for s in &mut self.sitzungen {
            if let Some(m) = s.meldung.as_mut() {
                m.abfragen();
            }
        }
    }

    /// Takes the output of every child: what is there now, in several pieces if need be, at most [`KIND_LESEN_MAX`] per child and
    /// turn. The turn of a child ends when a read brought neither data nor an event ([`lesen_endet`]); a piece of data without an
    /// event goes on (the rest comes with the next turn).
    fn kinder_lesen(&mut self) {
        for k in 0..self.sitzungen.len() {
            for _ in 0..KIND_LESEN_MAX {
                let Some(kind) = self.sitzungen.get_mut(k).and_then(|s| s.kind.as_mut()) else {
                    break;
                };
                let ereignisse = kind.lies(Duration::ZERO);
                // the record of the test build (contract section 21c)
                #[cfg(all(windows, feature = "pruefbau"))]
                crate::sys::mitschrift_ereignisse(k, &ereignisse);
                // a piece of output without an event (the bytes of an unfinished sequence, such as a long clipboard write) is no reason
                // to stop: the next pieces are there already, and a stop would cost the beat of the loop for every piece
                if lesen_endet(ereignisse.is_empty(), kind.daten_gelesen()) {
                    break;
                }
                for x in &ereignisse {
                    self.traeger_ereignis(k, x);
                }
                self.flush_alle();
            }
        }
    }

    fn eingang_verarbeiten(&mut self, nr: u64) {
        loop {
            let Some(b) = self.betrachter.get_mut(&nr) else {
                return;
            };
            if b.schliessen {
                return;
            }
            match b.eingang.naechster() {
                Ok(Some((art, nutzlast))) => self.rahmen_verarbeiten(nr, art, &nutzlast),
                Ok(None) => return,
                Err(f) => {
                    self.fehler(nr, f.code());
                    return;
                }
            }
        }
    }

    /// The end of a connection: the core gets `Trennen` (at most once, DI4); the auszug ends with the connection.
    fn getrennt(&mut self, nr: u64) {
        let Some(b) = self.betrachter.remove(&nr) else {
            return;
        };
        self.bereit.weg(nr);
        let (name, gemeldet) = (b.name.clone(), b.trennen_gemeldet);
        b.schliessen_strom();
        drop(b);
        if !gemeldet {
            self.trennen_melden(&name);
        }
    }

    /// DI12 point 4 (P25, Projektentscheid, 23.11.2026): the raster number of a new session `s-<n>`. A number is free when its session has ended and
    /// no viewer subscribes it; it is taken only when a new session needs one: first a number never handed out in this run, then the
    /// free one whose session ended longest ago. The session that gave it shows `nr: null` from then on, and every connection forgets
    /// the number (RA R13), so the first frame of the new session is a full picture. A subscribed number is never taken.
    fn rahmen_waehlen(&self, n: usize, max: usize) -> Option<u8> {
        let mut frei: Vec<(u64, u8)> = self
            .sitzungen
            .iter()
            .filter(|s| s.eintrag.is_some())
            .filter_map(|s| {
                let (sid, nr, ende) = (s.sid.as_ref()?, s.nr?, s.beendet_als?);
                let abonniert = self.betrachter.values().any(|b| b.abos.contains_key(sid));
                (!abonniert).then_some((ende, nr))
            })
            .collect();
        // DI-L3 point 5: the number of a deleted session is free in the same way
        frei.extend_from_slice(&self.frei_geloescht);
        rahmennummer(n, max, &self.vergeben, &frei)
    }

    /// Takes the number chosen by [`Self::rahmen_waehlen`]: the session that held it loses it, every connection forgets it.
    fn rahmen_nehmen(&mut self, nummer: u8) {
        for s in &mut self.sitzungen {
            if s.eintrag.is_some() && s.nr == Some(nummer) {
                s.nr = None;
            }
        }
        for b in self.betrachter.values_mut() {
            b.raster.vergessen(nummer);
        }
        self.vergeben[usize::from(nummer)] = true;
        self.frei_geloescht.retain(|&(_, n)| n != nummer);
    }

    fn trennen_melden(&mut self, name: &str) {
        let t = self.jetzt();
        self.an_alle_kerne(&json!({"t": t, "art": "Trennen", "betrachter": name}));
    }

    /// A protocol error (DI3): the viewer is gone for the core at once; one frame `Fehler` goes behind the frames that wait, then
    /// the connection ends. Nothing more is read from it. Other viewers and the core stay as they are.
    fn fehler(&mut self, nr: u64, code: &str) {
        let Some(b) = self.betrachter.get_mut(&nr) else {
            return;
        };
        if b.schliessen {
            return;
        }
        b.eingang.leeren();
        b.stumm = false;
        b.schliessen = true;
        let melden = !b.trennen_gemeldet;
        b.trennen_gemeldet = true;
        let name = b.name.clone();
        if melden {
            self.trennen_melden(&name);
        }
        self.senden(nr, json_rahmen(A_FEHLER, &json!({"fehler": code})));
    }

    // ---------------------------------------------------------------------------------------------- queue (DI5)

    /// A message of the core `k` for a viewer: a raster frame (with the number of the session) or a JSON frame; then the report
    /// `Socket`. Messages that belong to no core (`k` is `None`) are JSON frames.
    pub(crate) fn nachricht_an(&mut self, nr: u64, nachricht: &Value, k: Option<usize>) {
        let art = nachricht.get("art").and_then(Value::as_str);
        if let (Some(k), Some("Abzug" | "Rasteraenderung")) = (k, art) {
            // a session without a raster number sends no frame (DI12 point 4, P25): its number belongs to another session now
            let Some(sitzungsnr) = self.sitzungen.get(k).map_or(Some(0), |s| s.nr) else {
                return;
            };
            let Some(b) = self.betrachter.get_mut(&nr) else {
                return;
            };
            match b.raster.nimm(sitzungsnr, nachricht) {
                Ok(Some(bytes)) => self.senden(nr, rahmen(A_RASTER, &bytes)),
                Ok(None) => {}
                Err(f) => {
                    self.fehler(nr, &format!("raster_{}", f.code()));
                    return;
                }
            }
        } else {
            self.senden(nr, json_rahmen(A_NACHRICHT, nachricht));
        }
        self.melden(nr);
    }

    /// Puts a frame in the queue, tries to write, and ends the connection if the queue passed the bound (DI5).
    fn senden(&mut self, nr: u64, daten: Vec<u8>) {
        let Some(b) = self.betrachter.get_mut(&nr) else {
            return;
        };
        b.einreihen(daten);
        self.schreiben(nr);
        if self
            .betrachter
            .get(&nr)
            .is_some_and(|b| b.laenge > self.cfg.warteschlange_max)
        {
            self.abtrennen(nr);
        }
    }

    /// Writes what the socket takes; the rest stays at the front of the queue in order (DI5). The service never blocks here.
    fn schreiben(&mut self, nr: u64) {
        let Some(b) = self.betrachter.get_mut(&nr) else {
            return;
        };
        if !b.schreiben() || (b.schliessen && b.schlange.is_empty()) {
            self.getrennt(nr);
        }
    }

    /// The hard bound (DI5): the queue is dropped, the core gets `Trennen`, the socket is closed (no frame `Fehler`: the queue
    /// is full and is not written any more) and one line goes to the log (stderr). The line holds the name of the viewer, the
    /// reason and two numbers; no byte of a line, title, clipboard, input or of the start file.
    fn abtrennen(&mut self, nr: u64) {
        let Some(b) = self.betrachter.get_mut(&nr) else {
            return;
        };
        let _ = writeln!(
            std::io::stderr(),
            "wb-dienst: betrachter {} getrennt: rueckstau_gerissen (ausstehend {} Byte, Grenze {} Byte)",
            b.name,
            b.laenge,
            self.cfg.warteschlange_max
        );
        b.verwerfen();
        self.getrennt(nr);
    }

    /// The report `Socket` (DI5): it waits in `arbeit` until the step is done, so that the order of the core stays. The queue
    /// belongs to the viewer, so every core gets the same number (DI12 point 2).
    fn melden(&mut self, nr: u64) {
        let t = self.jetzt();
        let Some(b) = self.betrachter.get(&nr) else {
            return;
        };
        if b.schliessen {
            return;
        }
        let e = json!({"t": t, "art": "Socket", "betrachter": b.name, "ausstehend": b.laenge});
        for k in 0..self.sitzungen.len() {
            self.arbeit.push_back((e.clone(), None, k));
        }
    }

    /// Writes the queues; a queue that got shorter is reported. Then the reports are carried out.
    fn flush_alle(&mut self) {
        let nummern: Vec<u64> = self.betrachter.keys().copied().collect();
        for nr in nummern {
            let Some(vorher) = self.betrachter.get(&nr).map(|b| b.laenge) else {
                continue;
            };
            self.schreiben(nr);
            if self
                .betrachter
                .get(&nr)
                .is_some_and(|b| kuerzer(vorher, b.laenge))
            {
                self.melden(nr);
            }
        }
        self.arbeit_abarbeiten();
    }

    // ---------------------------------------------------------------------------------------------- frames (DI3, DI9)

    /// The index of the session with this identifier; the room core has none.
    pub(crate) fn finde(&self, sid: &str) -> Option<usize> {
        self.sitzungen
            .iter()
            .position(|s| s.sid.as_deref() == Some(sid))
    }

    fn rahmen_verarbeiten(&mut self, nr: u64, art: u8, nutzlast: &[u8]) {
        match art {
            A_EREIGNIS => self.ereignis_rahmen(nr, nutzlast),
            A_UHR | A_STUMM | A_ZUSTAND | A_KERNFEHLER => {
                if self.cfg.pruefmodus {
                    self.pruef_rahmen(nr, art, nutzlast);
                } else {
                    self.fehler(nr, "art_nicht_erlaubt");
                }
            }
            _ => self.fehler(nr, "art_unbekannt"),
        }
    }

    /// The payload of a frame as a JSON value; `None` after the error `json_ungueltig`.
    fn json_nutzlast(&mut self, nr: u64, nutzlast: &[u8]) -> Option<Value> {
        let wert = std::str::from_utf8(nutzlast)
            .ok()
            .and_then(|t| serde_json::from_str::<Value>(t).ok());
        if wert.is_none() {
            self.fehler(nr, "json_ungueltig");
        }
        wert
    }

    /// The payload of a frame `Ereignis` as a JSON value; `None` after the error `json_ungueltig`. The event `Starten` may be one level
    /// deeper than other payloads (see [`ereignis_lesen`]).
    fn ereignis_nutzlast(&mut self, nr: u64, nutzlast: &[u8]) -> Option<Value> {
        let wert = ereignis_lesen(nutzlast);
        if wert.is_none() {
            self.fehler(nr, "json_ungueltig");
        }
        wert
    }

    /// A frame `Ereignis` (DI3): checks in the order of the contract, then gives the event to the core with `betrachter` and `t`.
    /// With `lauf` the events `Starten`, `Beenden`, `Umbenennen`, `Fortsetzen`, `Wiederbeleben` and `Loeschen` are the service's own
    /// (Nachtrag 26.11.2026, DI-L3) and the others go to the core of their session.
    fn ereignis_rahmen(&mut self, nr: u64, nutzlast: &[u8]) {
        let Some(wert) = self.ereignis_nutzlast(nr, nutzlast) else {
            return;
        };
        let Some(objekt) = wert.as_object() else {
            return self.fehler(nr, "ereignis_ungueltig");
        };
        let Some(art) = objekt.get("art").and_then(Value::as_str) else {
            return self.fehler(nr, "ereignis_ungueltig");
        };
        let lauf = self.cfg.lauf.is_some();
        let eigene = lauf
            && matches!(
                art,
                "Starten" | "Beenden" | "Umbenennen" | "Fortsetzen" | "Wiederbeleben" | "Loeschen"
            );
        if !eigene && !ERLAUBT.contains(&art) {
            return self.fehler(nr, "art_nicht_erlaubt");
        }
        if objekt.contains_key("t") || objekt.contains_key("betrachter") {
            return self.fehler(nr, "feld_verboten");
        }
        let art = art.to_owned();
        if arbeitsordner::ist_eigen(&art) {
            self.arbeitsordner_ereignis(nr, objekt);
            self.flush_alle();
            return;
        }
        if lauf && art == "Starten" {
            self.starten(nr, objekt);
            self.flush_alle();
            return;
        }
        if eigene {
            match art.as_str() {
                "Beenden" => self.beenden_ereignis(nr, objekt),
                "Umbenennen" => self.umbenennen(nr, objekt),
                "Fortsetzen" => self.fortsetzen(nr, objekt, false),
                "Wiederbeleben" => self.fortsetzen(nr, objekt, true),
                _ => self.loeschen(nr, objekt),
            }
            self.flush_alle();
            return;
        }
        let Some(name) = self.betrachter.get(&nr).map(|b| b.name.clone()) else {
            return;
        };
        let mut e = wert.clone();
        e["t"] = json!(self.jetzt());
        e["betrachter"] = json!(name);
        // A size with fewer than the smallest number of columns is refused here, before the core sees it (decision of 20.11.2026).
        let spalten = ganz(e.get("spalten"));
        if matches!(art.as_str(), "Abo" | "Groesse")
            && spalten.is_some_and(|s| s < i64::try_from(KLEINSTE_SPALTEN).unwrap_or(2))
        {
            self.ablehnen(nr, &json!({"grund": "groesse_ungueltig"}));
            return;
        }
        let k = if lauf {
            if matches!(art.as_str(), "RaumWechsel" | "EinstellungSetzen") {
                return self.ablehnen(nr, &json!({"grund": "nicht_im_lauf"}));
            }
            match e
                .get("sitzung")
                .and_then(Value::as_str)
                .and_then(|s| self.finde(s))
            {
                Some(k) => k,
                None => return self.ablehnen(nr, &json!({"grund": "sitzung_unbekannt"})),
            }
        } else {
            // the single mode has one core, at index 0
            0
        };
        // DI12 point 4 (P25): a session whose raster number went to a later session cannot be watched live any more
        if lauf
            && art == "Abo"
            && e.get("an") != Some(&Value::Bool(false))
            && self.sitzungen.get(k).is_some_and(|s| s.nr.is_none())
        {
            return self.ablehnen(nr, &json!({"grund": "sitzung_ohne_nummer"}));
        }
        let sid = self.sitzungen.get(k).and_then(|s| s.sid.clone());
        if let Ok(w) = self.schritt(k, e.clone()) {
            if w.ablehnungen.is_empty()
                && let (Some(b), Some(sid)) = (self.betrachter.get_mut(&nr), &sid)
            {
                abo_nachfuehren(&mut b.abos, sid, &art, &e);
            }
            self.wirkungen(k, w, Some(nr));
        } else {
            self.kern_neu(k);
        }
        self.flush_alle();
    }

    // ---------------------------------------------------------------------------------------------- working folder (L9)

    /// `OrdnerLesen` and `InhaltSuchen` (contract, "Nachtrag 26.11.2026: Ordner lesen und durchsuchen (L9)"): the answer goes to the
    /// sender alone, as a message `Ordnerinhalt` or `Suchtreffer` or as an `Ablehnung` with a fixed reason. The core sees neither.
    fn arbeitsordner_ereignis(&mut self, nr: u64, ereignis: &Map<String, Value>) {
        match self.arbeitsordner_antwort(nr, ereignis) {
            Ok(mut antwort) => {
                antwort["t"] = json!(self.jetzt());
                self.nachricht_an(nr, &antwort, None);
            }
            Err(grund) => self.ablehnen(nr, &json!({"grund": grund})),
        }
    }

    /// The order of the checks: the form, the session, its room, then the text, the class and the file system (`arbeitsordner.rs`).
    fn arbeitsordner_antwort(
        &mut self,
        nr: u64,
        ereignis: &Map<String, Value>,
    ) -> Result<Value, &'static str> {
        let anfrage = arbeitsordner::form(ereignis)?;
        let k = self
            .finde(anfrage.sitzung())
            .ok_or(arbeitsordner::SITZUNG_UNBEKANNT)?;
        if !self.im_raum(nr, k) {
            return Err(arbeitsordner::FREMDER_KUNDENRAUM);
        }
        let wurzel = self.sitzungen.get(k).and_then(|s| s.arbeitsordner.clone());
        // the clock of the service (DI7): in test mode it stands still, so the limit of the run time never ends a search there
        let pruefmodus = self.cfg.pruefmodus;
        let beginn = self.start;
        let mut uhr = move || {
            if pruefmodus {
                0
            } else {
                u64::try_from(beginn.elapsed().as_millis()).unwrap_or(u64::MAX)
            }
        };
        arbeitsordner::beantworten(
            &anfrage,
            wurzel.as_deref(),
            &arbeitsordner::GRENZEN,
            &mut uhr,
        )
    }

    /// Whether the viewer `nr` is in the customer room of the session `k`, as its core knows both (the core refuses `Abo` with
    /// `fremder_kundenraum` by the same comparison). A viewer or a session the core does not name is not in the room.
    fn im_raum(&self, nr: u64, k: usize) -> bool {
        let (Some(b), Some(s)) = (self.betrachter.get(&nr), self.sitzungen.get(k)) else {
            return false;
        };
        let zustand = s.kern.zustand();
        let raum = zustand
            .get("betrachter")
            .and_then(|x| x.get(&b.name))
            .and_then(|x| x.get("raum"))
            .and_then(Value::as_str);
        let kundenraum = zustand
            .get("sitzungen")
            .and_then(Value::as_array)
            .and_then(|l| {
                l.iter()
                    .find(|x| x.get("id").and_then(Value::as_str) == s.sid.as_deref())
            })
            .and_then(|x| x.get("kundenraum"))
            .and_then(Value::as_str);
        raum.is_some() && raum == kundenraum
    }

    // ---------------------------------------------------------------------------------------------- Starten, Beenden (DI12)

    /// The list of the sessions (DI12 point 5): in the order of the start, the session of `kind` first; ended ones stay, at most
    /// `lauf.beendet_max` of them (P26).
    fn liste_nachricht(&mut self) -> Value {
        let sitzungen: Vec<Value> = self
            .sitzungen
            .iter()
            .filter(|s| !s.sid.as_ref().is_some_and(|sid| self.entfernt.contains(sid)))
            .filter_map(|s| {
                let sid = s.sid.as_ref()?;
                let mut x = json!({"id": sid, "harness": s.meta[0], "modell": s.meta[1], "route": s.meta[2],
                                   "kunde": s.meta[3], "zustand": if s.ist_beendet() { "beendet" } else { "laeuft" },
                                   "nr": s.nr});
                if let Some(code) = s.ende_code() {
                    x["code"] = json!(code);
                }
                if let Some(grund) = &s.grund {
                    x["grund"] = json!(grund);
                }
                // Nachtrag 26.11.2026, DI-L5 point 1
                s.listenfelder(&mut x);
                Some(x)
            })
            .collect();
        json!({"t": self.jetzt(), "art": "Sitzungsliste", "sitzungen": sitzungen})
    }

    /// The list to every viewer that can take it (after a start and after the end of a child).
    pub(crate) fn liste_senden(&mut self) {
        let nummern: Vec<u64> = self.betrachter.keys().copied().collect();
        for nr in nummern {
            if sendbar(self.betrachter.get(&nr)) {
                let liste = self.liste_nachricht();
                self.nachricht_an(nr, &liste, None);
            }
        }
    }

    /// The message `Startwahl` (DI12 point 10): what the page needs for the start form, read from `lauf.einstellungen`. `nur` is
    /// the viewer that just connected; if the content differs from the last one that went out, every viewer gets it. With
    /// `nur_geaendert` nothing goes out when the content is the same. A file that cannot be read gives no message.
    fn startwahl_senden(&mut self, nur: Option<u64>, nur_geaendert: bool) {
        let Some(lauf) = &self.cfg.lauf else {
            return;
        };
        let Some(einst) = einstellungen_laden(&lauf.einstellungen) else {
            return;
        };
        self.startwahl_aus(&einst, nur, nur_geaendert);
    }

    /// `Startwahl` from settings that are read already (see `startwahl_senden`).
    fn startwahl_aus(&mut self, einst: &Einstellungen, nur: Option<u64>, nur_geaendert: bool) {
        let inhalt = startwahl_von(einst);
        let gleich = self.startwahl_letzt.as_ref() == Some(&inhalt);
        if nur_geaendert && gleich {
            return;
        }
        let nur = if gleich { nur } else { None };
        self.startwahl_letzt = Some(inhalt.clone());
        let mut nachricht = inhalt;
        nachricht["t"] = json!(self.jetzt());
        nachricht["art"] = json!("Startwahl");
        let nummern: Vec<u64> = self.betrachter.keys().copied().collect();
        for nr in nummern {
            if nur.is_some_and(|n| n != nr) || !sendbar(self.betrachter.get(&nr)) {
                continue;
            }
            self.nachricht_an(nr, &nachricht, None);
        }
    }

    /// The refusal of `Starten`: `grund`, `code` and `text` (DI12 point 3).
    pub(crate) fn start_ablehnen(&mut self, nr: u64, a: &Abgelehnt) {
        self.ablehnen(nr, &a.nutzlast());
    }

    /// Sessions that run: not ended.
    fn laufende(&self) -> usize {
        self.sitzungen
            .iter()
            .filter(|s| s.sid.is_some() && s.kind_ende.is_none())
            .count()
    }

    /// `Starten` (DI12 point 3). The first violation answers and nothing starts. The core never sees the event.
    pub(crate) fn starten(&mut self, nr: u64, ereignis: &Map<String, Value>) {
        let start = match starten_form(ereignis) {
            Ok(s) => s.clone(),
            Err(a) => return self.start_ablehnen(nr, &a),
        };
        // DI-L1 point 2: the form of `orchestrator` and `rolle` is checked after stages 1 to 13 (in `starten_mit`)
        let worker = worker_form(ereignis);
        self.starten_mit(nr, &start, worker);
    }

    /// The way of `Starten` from DI12 point 3.1 on. `worker` is the orchestrator and the role (DI-L1).
    #[allow(clippy::too_many_lines)] // the checks of DI12 point 3 in the order of the contract
    pub(crate) fn starten_mit(
        &mut self,
        nr: u64,
        start: &Value,
        worker: Result<Option<(String, String)>, Abgelehnt>,
    ) {
        let Some(lauf) = self.cfg.lauf.clone() else {
            return;
        };
        // SECURITY: the one place of the check of the structure (see `starten.rs`): stages 1 to 13 of the crate `start`, then the settings file
        // (`einstellungen`), then stages 14 to 17; the first violation answers and nothing starts
        let anfrage = match start_form(start) {
            Ok(a) => a,
            Err(a) => return self.start_ablehnen(nr, &a),
        };
        let worker = match worker {
            Ok(w) => w,
            Err(a) => return self.start_ablehnen(nr, &a),
        };
        if let Some((orchestrator, _)) = &worker
            && let Err(a) = self.orchestrator_pruefen(orchestrator, &meta_von(&anfrage)[3])
        {
            return self.start_ablehnen(nr, &a);
        }
        let Some(einst) = einstellungen_laden(&lauf.einstellungen) else {
            return self.start_ablehnen(nr, &Abgelehnt::fest("einst_unlesbar"));
        };
        self.startwahl_aus(&einst, None, true);
        if let Err(a) = start_pruefen(&anfrage, &einst) {
            return self.start_ablehnen(nr, &a);
        }
        if self.laufende() >= SITZUNGEN_MAX {
            return self.start_ablehnen(nr, &Abgelehnt::fest("zu_viele_sitzungen"));
        }
        // DI12 point 4 (P25): with every raster number held the start is refused before the identifier loop, so point 15a removes an
        // old target list only for a start that is handed out (whether a raster number is free does not depend on the identifier)
        if self
            .rahmen_waehlen(self.naechste + 1, lauf.rahmen_max)
            .is_none()
        {
            return self.start_ablehnen(nr, &Abgelehnt::fest("nummern_erschoepft"));
        }
        // DI12 points 13 and 15: skip every identifier whose entry under `lauf.sitzungen` or whose target list under `lauf.ziellisten`
        // still exists (`lstat`: any kind, a dead link too); point 15a in `nummer_vergebbar`: the identifiers count from `s-1` again
        // after a restart, so this is where an identifier of an ended session of an earlier run is handed out again
        let mut n = self.naechste + 1;
        loop {
            if n > lauf.nummer_max {
                return self.start_ablehnen(nr, &Abgelehnt::fest("nummern_erschoepft"));
            }
            match nummer_vergebbar(&lauf, n) {
                Ok(false) => break,
                Ok(true) => n += 1,
                Err(()) => return self.start_ablehnen(nr, &Abgelehnt::fest("start_ordner")),
            }
        }
        // DI12 point 4 (P25): the raster number is separate from the identifier. Every number held: the start is refused, before
        // anything is made. The number is taken only after the folder, the child and the core exist (see below).
        let Some(nummer) = self.rahmen_waehlen(n, lauf.rahmen_max) else {
            return self.start_ablehnen(nr, &Abgelehnt::fest("nummern_erschoepft"));
        };
        let kennung = format!("s-{n}");
        let Ok((ordner, datei)) = ordner_anlegen(
            Path::new(&self.cfg.ordner),
            self.wurzel.as_fd(),
            &kennung,
            &anfrage,
            self.marker.as_ref(),
        ) else {
            return self.start_ablehnen(nr, &Abgelehnt::fest("start_ordner"));
        };
        let (kind, meldung) = match self.lauf_starten(&kennung, &ordner, &datei) {
            Ok(x) => x,
            Err(a) => return self.start_ablehnen(nr, &a),
        };
        let meta = meta_von(&anfrage);
        let eintrag = eintrag_fuer(
            &kennung,
            &meta[2],
            &self.cfg.raum,
            &self.cfg.person,
            lauf.groesse,
        );
        let anfang = anfang_fuer(
            &self.cfg.anfang,
            Some(&eintrag),
            &Traegerstand::anfang(lauf.groesse),
        );
        let Ok(kern) = zustandskern::Sitzungskern::neu_mit(&anfang, self.kern_grenze) else {
            for sid in kinder_verwerfen(vec![kind]) {
                self.waisen.jagen(sid, Instant::now());
            }
            crate::loeschen::sitzung_entfernen(self.wurzel.as_fd(), &kennung);
            return self.start_ablehnen(nr, &Abgelehnt::fest("programm_nicht_startbar"));
        };
        let mut sitzung = Sitzung::gestartet(
            kennung.clone(),
            nummer,
            kern,
            kind,
            eintrag,
            meta,
            (ordner, Arc::clone(&self.wurzel), meldung),
        );
        sitzung.arbeitsordner = Some(PathBuf::from(arbeitsordner_von(&anfrage)));
        angaben_setzen(&mut sitzung, &anfrage, start, worker);
        if !self.gestartet_aufnehmen(sitzung, nr, n, nummer) {
            return;
        }
        let k = self.sitzungen.len() - 1;
        let t = self.jetzt();
        let nummern: Vec<u64> = self.betrachter.keys().copied().collect();
        for b in &nummern {
            let name = format!("b{b}");
            let e = self.verbinden_ereignis(t, &name);
            self.ereignis(k, e, None);
        }
        // first the list to everyone (with `Agenten` behind it when the workers changed, DI-L1 point 4), then `Gestartet` to the
        // sender alone (DI12 point 3.7)
        self.liste_und_agenten();
        let gestartet = json!({"t": self.jetzt(), "art": "Gestartet", "sitzung": kennung});
        self.nachricht_an(nr, &gestartet, None);
    }

    /// DI-L1 point 2: the orchestrator is a running session of `Starten` in the list, no worker itself, with the same customer.
    fn orchestrator_pruefen(&self, orchestrator: &str, kunde: &str) -> Result<(), Abgelehnt> {
        let o = self
            .finde(orchestrator)
            .and_then(|k| self.sitzungen.get(k))
            .filter(|o| o.eintrag.is_some())
            .ok_or_else(|| Abgelehnt::fest("orchestrator_unbekannt"))?;
        if o.ist_beendet() {
            return Err(Abgelehnt::fest("orchestrator_beendet"));
        }
        if o.angaben.orchestrator.is_some() {
            return Err(Abgelehnt::fest("orchestrator_ist_worker"));
        }
        if o.meta[3] != kunde {
            return Err(Abgelehnt::fest("orchestrator_fremder_kunde"));
        }
        Ok(())
    }

    /// The new session of `Starten` takes its identifier `s-<n>` and the raster number `nummer` and joins the list. Under Windows a
    /// child without a job is ended instead, the folder of the session removed, and the start refused (section 21a point 4, fail
    /// closed); then `false`.
    #[cfg_attr(not(windows), allow(unused_variables))]
    fn gestartet_aufnehmen(&mut self, sitzung: Sitzung, nr: u64, n: usize, nummer: u8) -> bool {
        #[cfg(windows)]
        let Some(sitzung) = crate::sys::sitzung_mit_job(self, sitzung, nr) else {
            return false;
        };
        self.naechste = n;
        self.rahmen_nehmen(nummer);
        self.sitzungen.push(sitzung);
        true
    }

    /// The socket of the report (section 18) and the child `lauf` of a session of `Starten`: the socket exists, listening, before the child starts. A path too
    /// long for `sun_path` or a child that cannot start refuses the start; the folder of the session is removed again, with the socket in it.
    fn lauf_starten(
        &self,
        kennung: &str,
        ordner: &Path,
        datei: &Path,
    ) -> Result<(traeger::Kind, Meldung), Abgelehnt> {
        let meldung_pfad = ordner.join(SOCKET_NAME);
        let Ok(mut meldung) = Meldung::binden(&meldung_pfad) else {
            crate::loeschen::sitzung_entfernen(self.wurzel.as_fd(), kennung);
            return Err(Abgelehnt::fest("meldung_kanal"));
        };
        let kind = KindStart::von_lauf(&self.cfg, datei, &meldung_pfad)
            .ok_or(())
            .and_then(|ks| kind_starten(&ks, self.huelle.as_deref()).map_err(|_| ()));
        let Ok(kind) = kind else {
            crate::loeschen::sitzung_entfernen(self.wurzel.as_fd(), kennung);
            return Err(Abgelehnt::fest("programm_nicht_startbar"));
        };
        meldung.kind_setzen(kind.pid().unwrap_or(0));
        Ok((kind, meldung))
    }

    /// `Beenden` (DI12 point 6): SIGHUP, up to 1 s, SIGKILL; the loop does the waiting. The session stays in the list.
    fn beenden_ereignis(&mut self, nr: u64, ereignis: &Map<String, Value>) {
        let Some(sid) = ereignis
            .get("sitzung")
            .and_then(Value::as_str)
            .filter(|_| ereignis.keys().all(|k| k == "art" || k == "sitzung"))
        else {
            return self.ablehnen(nr, &json!({"grund": "beenden_form"}));
        };
        let Some(k) = self.finde(sid) else {
            return self.ablehnen(nr, &json!({"grund": "sitzung_unbekannt"}));
        };
        if self.sitzungen[k].ist_beendet() {
            return self.ablehnen(nr, &json!({"grund": "sitzung_beendet"}));
        }
        self.sitzungen[k].angaben.beendet_von_uns = true;
        self.sitzungen[k].beenden_anstossen();
    }

    /// The frames of the test mode (DI9).
    fn pruef_rahmen(&mut self, nr: u64, art: u8, nutzlast: &[u8]) {
        match art {
            A_UHR => {
                let Some(wert) = self.json_nutzlast(nr, nutzlast) else {
                    return;
                };
                let t = wert
                    .as_object()
                    .filter(|o| o.len() == 1)
                    .and_then(|o| ganz(o.get("t")))
                    .filter(|t| *t >= 0);
                let Some(t) = t else {
                    return self.fehler(nr, "uhr_ungueltig");
                };
                if t < self.t_ein {
                    return self.fehler(nr, "uhr_faellt");
                }
                self.t_ein = t;
                self.an_alle_kerne(&json!({"t": t, "art": "Takt"}));
                self.flush_alle();
            }
            A_STUMM => {
                let Some(wert) = self.json_nutzlast(nr, nutzlast) else {
                    return;
                };
                let an = wert
                    .as_object()
                    .filter(|o| o.len() == 1)
                    .and_then(|o| o.get("an"))
                    .and_then(Value::as_bool);
                let Some(an) = an else {
                    return self.fehler(nr, "stumm_ungueltig");
                };
                if an {
                    // the receipt goes out before the viewer turns mute
                    self.senden(nr, rahmen(A_STUMM, &[]));
                    if let Some(b) = self.betrachter.get_mut(&nr) {
                        b.stumm = true;
                    }
                } else {
                    if let Some(b) = self.betrachter.get_mut(&nr) {
                        b.stumm = false;
                    }
                    self.flush_alle();
                    // and behind the queue when it turns loud again
                    self.senden(nr, rahmen(A_STUMM, &[]));
                }
                self.flush_alle();
            }
            A_ZUSTAND => {
                let z = self.pruefzustand();
                self.senden(nr, json_rahmen(A_ZUSTAND, &z));
            }
            _ => {
                // with `lauf` the frame is not allowed (DI12 point 7); the rebuild per session stays a duty of the service
                if self.cfg.lauf.is_some() {
                    return self.fehler(nr, "art_nicht_erlaubt");
                }
                self.kern_neu(0);
            }
        }
    }

    /// The answer to the frame `Zustand` (DI9): the state of the core without `stand`, and the state of the service. With `lauf`
    /// the sessions come in the order of their start, the viewers hold the `abos` of all cores and `ausstehend` of the room core.
    fn pruefzustand(&self) -> Value {
        const SITZUNG: [&str; 10] = [
            "id",
            "titel",
            "zustand",
            "code",
            "groesse",
            "letzte_eingabe",
            "dialog",
            "bracketed_paste",
            "invers",
            "cursor",
        ];
        const BETRACHTER: [&str; 6] = [
            "person",
            "raum",
            "abos",
            "ausstehend",
            "verfallen",
            "rueckstand",
        ];
        let lauf = self.cfg.lauf.is_some();
        let mut sitzungen: Vec<Value> = Vec::new();
        let mut betrachter = Map::new();
        for s in &self.sitzungen {
            let z = s.kern.zustand();
            if let Some(liste) = z.get("sitzungen").and_then(Value::as_array) {
                sitzungen.extend(liste.iter().map(|x| waehle(x, &SITZUNG)));
            }
            let Some(karte) = z.get("betrachter").and_then(Value::as_object) else {
                continue;
            };
            for (name, b) in karte {
                let neu = waehle(b, &BETRACHTER);
                match betrachter.get_mut(name) {
                    // the first core (the room core, or the only one) gives the shape; the others add their `abos`
                    Some(Value::Object(da)) => betrachter_zusammenfuehren(da, &neu),
                    Some(_) => {}
                    None => {
                        betrachter.insert(name.clone(), neu);
                    }
                }
            }
        }
        let dienst_betrachter: Map<String, Value> = self
            .betrachter
            .values()
            .map(|b| {
                (
                    b.name.clone(),
                    json!({"warteschlange": b.laenge, "stumm": b.stumm}),
                )
            })
            .collect();
        let kernbau: u32 = self.sitzungen.iter().map(|s| s.kernbau).sum();
        let mut dienst = json!({
            "betrachter": dienst_betrachter,
            "kernbau": kernbau,
            "warteschlange_max": self.cfg.warteschlange_max,
        });
        let kind_von = |s: &Sitzung| json!({"ende": s.kind_ende.is_some(), "code": s.kind_ende});
        let zeilen_von = |s: &Sitzung| {
            s.kind
                .as_ref()
                .map(|k| k.raster().zeilen())
                .unwrap_or_default()
        };
        if lauf {
            let mut kind = Map::new();
            let mut zeilen = Map::new();
            let mut nummern = Map::new();
            for s in self.sitzungen.iter().filter(|s| s.sid.is_some()) {
                let sid = s.sid.clone().unwrap_or_default();
                kind.insert(sid.clone(), kind_von(s));
                zeilen.insert(sid.clone(), json!(zeilen_von(s)));
                nummern.insert(sid, json!(s.nr));
            }
            dienst["kind"] = Value::Object(kind);
            dienst["traeger_zeilen"] = Value::Object(zeilen);
            dienst["nummern"] = Value::Object(nummern);
            dienst["gestartet"] = json!(self.naechste);
        } else if let Some(s) = self.sitzungen.first() {
            dienst["kind"] = kind_von(s);
            dienst["traeger_zeilen"] = json!(zeilen_von(s));
        }
        json!({"kern": {"sitzungen": sitzungen, "betrachter": betrachter}, "dienst": dienst})
    }

    // ---------------------------------------------------------------------------------------------- end

    /// Closes the viewers, removes the socket file, ends every child and removes the folders of `Starten` (DI2 point 7, DI8,
    /// DI12 point 8). Can be called again. SECURITY: no child survives; only folders the service made itself are removed.
    fn aufraeumen(&mut self) {
        // DI-L5 point 2: every running session is `beendet` with `dienst.ende` in the Bestand before the children are ended
        // (a second call, from `drop`, writes the same content again)
        if self.cfg.lauf.as_ref().is_some_and(|l| l.bestand.is_some()) {
            for s in &mut self.sitzungen {
                if s.eintrag.is_some() && !s.ist_beendet() {
                    s.angaben.ende = Some(crate::verwaltung::EndeArt::Dienstende);
                    s.grund = Some("dienst.ende".to_owned());
                }
            }
            self.bestand_schreiben();
        }
        for b in self.betrachter.values() {
            b.schliessen_strom();
        }
        for nr in self.betrachter.keys() {
            self.bereit.weg(*nr);
        }
        self.betrachter.clear();
        self.bereit.weg(SCHLUESSEL_HOERER);
        self.hoerer = None;
        if self.socket_da {
            crate::loeschen::datei_entfernen(self.wurzel.as_fd(), &self.cfg.name);
            self.socket_da = false;
        }
        let sids: Vec<u32> = self
            .sitzungen
            .iter()
            .filter(|s| s.kind.is_some())
            .filter_map(|s| s.blick.pid())
            .collect();
        kinder_beenden(Self::kinder_nehmen(&mut self.sitzungen));
        // the children are collected now: their orphans get the whole order (SIGHUP, up to 1 s, SIGKILL) before the folders go
        self.waisen.alle_beenden(&sids, &[]);
        // the carriers have collected every child: no own child is left
        for s in &mut self.sitzungen {
            s.ende_aufraeumen(&[]);
        }
    }
}

impl Drop for Dienst {
    fn drop(&mut self) {
        self.aufraeumen();
    }
}

/// The turn of reading a child ends when a piece gave no event and no data: only then is the rest not there yet.
fn lesen_endet(ohne_ereignis: bool, daten_gelesen: bool) -> bool {
    ohne_ereignis && !daten_gelesen
}

/// Catches SIGTERM and SIGINT (the signals wait in the returned value until [`Dienst::laufen`] takes them).
///
/// # Errors
/// The error of the system when a handler cannot be set.
#[cfg(unix)]
pub fn signale_anmelden() -> std::io::Result<signal_hook::iterator::Signals> {
    use signal_hook::consts::{SIGINT, SIGTERM};
    signal_hook::iterator::Signals::new([SIGTERM, SIGINT])
}

// Windows: the caught console events and their registration are in `sys/windows.rs` (contract section 21).
#[cfg(windows)]
pub use crate::sys::{Signale, signale_anmelden};

/// The session of `kind` at the start of the service. Under Windows a child without a job is a start error (section 21a point 4, fail
/// closed; DI8: exit code 4): the child is ended and nothing stays (`sys::mit_job`).
#[cfg(unix)]
#[allow(clippy::unnecessary_wraps)]
fn mit_job(sitzung: Sitzung) -> Result<Sitzung, StartFehler> {
    Ok(sitzung)
}

#[cfg(windows)]
use crate::sys::mit_job;

/// The wake socket pair of the loop, both ends non-blocking.
fn weckpaar() -> std::io::Result<(UnixStream, UnixStream)> {
    let (lese, schreib) = UnixStream::pair()?;
    lese.set_nonblocking(true)?;
    schreib.set_nonblocking(true)?;
    Ok((lese, schreib))
}

/// The pids of the own children of `sitzungen` that the carriers have not collected yet ([`Dienst::eigene_kinder`]).
fn kinder_der(sitzungen: &[Sitzung]) -> Vec<u32> {
    sitzungen.iter().filter_map(Sitzung::eigenes_kind).collect()
}

/// Nachtrag 26.11.2026: the fields of the list and of the Bestand of a new session (DI-L1, DI-L5).
fn angaben_setzen(
    sitzung: &mut Sitzung,
    anfrage: &start::Startanfrage,
    start: &Value,
    worker: Option<(String, String)>,
) {
    let a = &mut sitzung.angaben;
    arbeitsordner_von(anfrage).clone_into(&mut a.ordner);
    a.start = Some(start.clone());
    if let Some((orchestrator, rolle)) = worker {
        a.orchestrator = Some(orchestrator);
        a.rolle = Some(rolle);
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::net::UnixStream;

    /// P23 after P26: a session whose child has ended and whose carrier was given back keeps the pid of its child
    /// (`Kind::pid`), but that child is collected, so the own children hold only the pid of the child that still runs. A collected
    /// pid in the list would protect a stranger that took the number; a missing running one would let the end of another session
    /// signal it.
    #[test]
    fn eigene_kinder_nach_der_freigabe_nur_das_laufende_kind() {
        let start = |argv: &[&str]| traeger::Start {
            argv: argv.iter().map(|a| (*a).to_owned()).collect(),
            env: Vec::new(),
            cwd: PathBuf::from("/"),
            spalten: 20,
            zeilen: 5,
            ruecklauf: 10,
        };
        let kern = || {
            let basis = json!({"personen": {"anna": {}}, "sitzungen": []});
            zustandskern::Sitzungskern::neu(&basis).expect("core")
        };
        let kind = |argv: &[&str]| traeger::Kind::starten(start(argv)).expect("child starts");
        let mut sitzungen = vec![
            Sitzung::raum(kern()),
            Sitzung::neu("s-1".to_owned(), kern(), kind(&["/bin/sh", "-c", "exit 0"])),
            Sitzung::neu("s-2".to_owned(), kern(), kind(&["/bin/sleep", "30"])),
        ];
        let pid = |s: &Sitzung| s.kind.as_ref().and_then(traeger::Kind::pid).expect("pid");
        let (a, b) = (pid(&sitzungen[1]), pid(&sitzungen[2]));
        let mut beide = kinder_der(&sitzungen);
        beide.sort_unstable();
        let mut erwartet = vec![a, b];
        erwartet.sort_unstable();
        assert_eq!(beide, erwartet, "both children run");
        // the child of s-1 ends: the carrier reads up to `Kindende` and collects it, then gives the PTY back (P26)
        let frist = Instant::now() + Duration::from_secs(10);
        let ende = loop {
            let kind = sitzungen[1].kind.as_mut().expect("carrier");
            let ereignisse = kind.lies(Duration::from_millis(50));
            if let Some(e) = ereignisse
                .iter()
                .find(|e| matches!(e, Ereignis::Kindende { .. }))
            {
                break e.clone();
            }
            assert!(Instant::now() < frist, "the child of s-1 ends within 10 s");
        };
        assert!(matches!(ende, Ereignis::Kindende { code: 0 }), "{ende:?}");
        // as at the end of a session: the own children are read before the carrier is given back
        assert_eq!(kinder_der(&sitzungen), vec![b], "before the release");
        let kind = sitzungen[1].kind.as_mut().expect("carrier");
        kind.freigeben();
        assert!(kind.freigegeben(), "the carrier of s-1 is given back");
        assert_eq!(
            kind.pid(),
            Some(a),
            "the pid stays with the released carrier"
        );
        assert_eq!(kinder_der(&sitzungen), vec![b], "after the release");
    }

    #[test]
    fn the_reading_of_a_child_goes_on_past_data_without_an_event() {
        assert!(lesen_endet(true, false));
        assert!(!lesen_endet(true, true));
        assert!(!lesen_endet(false, true));
        assert!(!lesen_endet(false, false));
    }

    #[test]
    fn takt_und_frist() {
        assert_eq!(warte_dauer(false, None), Duration::from_millis(2));
        assert_eq!(warte_dauer(true, None), Duration::from_millis(1));
        assert_eq!(warte_dauer(false, Some(1)), Duration::from_millis(1));
        assert_eq!(warte_dauer(false, Some(0)), Duration::ZERO);
        assert_eq!(warte_dauer(false, Some(-5)), Duration::ZERO);
        assert_eq!(warte_dauer(false, Some(500)), Duration::from_millis(2));
        assert_eq!(warte_dauer(true, Some(1)), Duration::from_millis(1));
    }

    fn fehler(art: ErrorKind) -> std::io::Result<usize> {
        Err(std::io::Error::from(art))
    }

    #[test]
    fn restzeit_bis_zur_frist() {
        assert_eq!(rest_ms(1016, 1000), 16);
        assert_eq!(rest_ms(1000, 1000), 0);
        assert_eq!(rest_ms(990, 1000), -10);
        assert_eq!(rest_ms(5, 0), 5);
    }

    #[test]
    fn sendbar_nur_ein_vorhandener_nicht_endender() {
        let (a, _g) = UnixStream::pair().expect("pair");
        let mut b = Betrachter::neu(1, a, false);
        assert!(sendbar(Some(&b)));
        assert!(!sendbar(None));
        b.schliessen = true;
        assert!(!sendbar(Some(&b)));
    }

    #[test]
    fn nur_eine_kuerzere_schlange_wird_gemeldet() {
        assert!(kuerzer(10, 9));
        assert!(kuerzer(1, 0));
        assert!(!kuerzer(10, 10));
        assert!(!kuerzer(0, 0));
        assert!(!kuerzer(10, 11));
    }

    #[test]
    fn lesen_ergebnis_bedeutung() {
        assert_eq!(gelesen(Ok(0)), Gelesen::Ende);
        assert_eq!(gelesen(Ok(1)), Gelesen::Daten(1));
        assert_eq!(gelesen(Ok(65536)), Gelesen::Daten(65536));
        assert_eq!(gelesen(fehler(ErrorKind::WouldBlock)), Gelesen::Nochmal);
        assert_eq!(gelesen(fehler(ErrorKind::Interrupted)), Gelesen::Nochmal);
        for art in [
            ErrorKind::ConnectionReset,
            ErrorKind::BrokenPipe,
            ErrorKind::PermissionDenied,
            ErrorKind::UnexpectedEof,
            ErrorKind::TimedOut,
        ] {
            assert_eq!(gelesen(fehler(art)), Gelesen::Ende, "{art:?}");
        }
    }

    #[test]
    fn annahme_ergebnis_bedeutung() {
        assert_eq!(annahme(Ok(7)), Annahme::Neu(7));
        let leer = |art| annahme::<u8>(Err(std::io::Error::from(art)));
        assert_eq!(leer(ErrorKind::Interrupted), Annahme::Nochmal);
        assert_eq!(leer(ErrorKind::WouldBlock), Annahme::Keine);
        assert_eq!(leer(ErrorKind::PermissionDenied), Annahme::Keine);
        assert_eq!(leer(ErrorKind::ConnectionAborted), Annahme::Keine);
    }

    #[test]
    fn rueckstand_nur_bei_wartenden_frames_eines_nicht_stummen() {
        let mut karte = BTreeMap::new();
        assert!(!hat_rueckstand(&karte));
        let (a, _ga) = UnixStream::pair().expect("pair");
        let (b, _gb) = UnixStream::pair().expect("pair");
        karte.insert(1, Betrachter::neu(1, a, false));
        karte.insert(2, Betrachter::neu(2, b, false));
        assert!(!hat_rueckstand(&karte), "empty queues");
        if let Some(b) = karte.get_mut(&2) {
            b.stumm = true;
            b.einreihen(vec![1]);
        }
        assert!(!hat_rueckstand(&karte), "a muted viewer does not count");
        if let Some(b) = karte.get_mut(&1) {
            b.einreihen(vec![1]);
        }
        assert!(
            hat_rueckstand(&karte),
            "a frame waits for a viewer that is not muted"
        );
        if let Some(b) = karte.get_mut(&2) {
            b.stumm = false;
        }
        if let Some(b) = karte.get_mut(&1) {
            b.verwerfen();
        }
        assert!(hat_rueckstand(&karte), "the other viewer has one now");
    }

    #[test]
    fn abo_wird_fuer_den_neubau_nachgefuehrt() {
        let mut abos = BTreeMap::new();
        let abo = |an: Option<Value>, sp: Value, ze: Value| {
            let mut e = json!({"art": "Abo", "spalten": sp, "zeilen": ze});
            if let Some(an) = an {
                e["an"] = an;
            }
            e
        };
        // a Groesse without a Abo is nothing
        abo_nachfuehren(
            &mut abos,
            "s",
            "Groesse",
            &json!({"spalten": 5, "zeilen": 6}),
        );
        assert!(abos.is_empty());
        // a Abo without `an` and with `an` true
        abo_nachfuehren(&mut abos, "s", "Abo", &abo(None, json!(40), json!(10)));
        assert_eq!(abos.get("s"), Some(&(40, 10)));
        abo_nachfuehren(
            &mut abos,
            "s",
            "Abo",
            &abo(Some(json!(true)), json!(41), json!(11)),
        );
        assert_eq!(abos.get("s"), Some(&(41, 11)));
        // a Groesse of a session that has a Abo changes the size
        abo_nachfuehren(
            &mut abos,
            "s",
            "Groesse",
            &json!({"spalten": 30, "zeilen": 8}),
        );
        assert_eq!(abos.get("s"), Some(&(30, 8)));
        // a Groesse for another session does nothing
        abo_nachfuehren(
            &mut abos,
            "t",
            "Groesse",
            &json!({"spalten": 3, "zeilen": 4}),
        );
        assert_eq!(abos.len(), 1);
        // sizes that are no whole numbers change nothing
        abo_nachfuehren(
            &mut abos,
            "s",
            "Groesse",
            &json!({"spalten": 30.5, "zeilen": 8}),
        );
        abo_nachfuehren(
            &mut abos,
            "s",
            "Groesse",
            &json!({"spalten": true, "zeilen": 8}),
        );
        abo_nachfuehren(&mut abos, "s", "Abo", &abo(None, json!("40"), json!(10)));
        assert_eq!(abos.get("s"), Some(&(30, 8)));
        // an other `an` than true or false does nothing
        abo_nachfuehren(
            &mut abos,
            "s",
            "Abo",
            &abo(Some(json!("nein")), json!(1), json!(1)),
        );
        abo_nachfuehren(
            &mut abos,
            "s",
            "Abo",
            &abo(Some(json!(0)), json!(1), json!(1)),
        );
        assert_eq!(abos.get("s"), Some(&(30, 8)));
        // any other kind of event does nothing
        abo_nachfuehren(&mut abos, "s", "Text", &json!({"spalten": 1, "zeilen": 1}));
        assert_eq!(abos.get("s"), Some(&(30, 8)));
        // `an` false forgets it, and a later Groesse does not bring it back
        abo_nachfuehren(
            &mut abos,
            "s",
            "Abo",
            &abo(Some(json!(false)), json!(40), json!(10)),
        );
        assert!(abos.is_empty());
        abo_nachfuehren(
            &mut abos,
            "s",
            "Groesse",
            &json!({"spalten": 30, "zeilen": 8}),
        );
        assert!(abos.is_empty());
    }

    #[test]
    fn betrachter_der_kerne_werden_zusammengefuehrt() {
        let mut da = json!({"abos": [{"sitzung": "s-1"}], "ausstehend": 7, "verfallen": 2, "rueckstand": {"a": 1}});
        let neu = json!({"abos": [{"sitzung": "s-2"}], "ausstehend": 9, "verfallen": 3, "rueckstand": {"b": 2}});
        let Some(karte) = da.as_object_mut() else {
            panic!("object");
        };
        betrachter_zusammenfuehren(karte, &neu);
        assert_eq!(
            da,
            json!({"abos": [{"sitzung": "s-1"}, {"sitzung": "s-2"}], "ausstehend": 7, "verfallen": 5,
                   "rueckstand": {"a": 1, "b": 2}})
        );
        // a viewer without the keys keeps what it had
        let mut leer = json!({});
        let Some(karte) = leer.as_object_mut() else {
            panic!("object");
        };
        betrachter_zusammenfuehren(karte, &json!({}));
        assert_eq!(leer, json!({"verfallen": 0}));
    }

    #[test]
    fn waehle_nimmt_nur_genannte_schluessel() {
        let w = json!({"a": 1, "b": 2, "c": 3});
        assert_eq!(waehle(&w, &["a", "c", "z"]), json!({"a": 1, "c": 3}));
        assert_eq!(waehle(&json!(5), &["a"]), json!({}));
    }

    #[test]
    fn ganz_ist_keine_wahrheit_und_keine_kommazahl() {
        assert_eq!(ganz(Some(&json!(5))), Some(5));
        assert_eq!(ganz(Some(&json!(-5))), Some(-5));
        assert_eq!(ganz(Some(&json!(true))), None);
        assert_eq!(ganz(Some(&json!(5.5))), None);
        assert_eq!(ganz(Some(&json!("5"))), None);
        assert_eq!(ganz(None), None);
    }
}
