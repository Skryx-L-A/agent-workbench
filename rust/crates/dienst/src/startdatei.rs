//! The start file (contract section 3, DI1): one JSON object that says everything the service needs. Every error is an exit
//! code 2 with one line on stderr; nothing is started and nothing is left behind.

use std::fs::File;
use std::io::Read;
use std::path::Path;

use serde_json::{Map, Value};
use zustandskern::Sitzungskern;

use crate::fehler::StartFehler;

/// The most bytes of the start file.
pub const MAX_START: usize = 1_048_576;
/// The most bytes of the path of the socket (the limit of `sun_path` on the Mac).
pub const SOCKET_PFAD_MAX: usize = 100;
/// The hard upper bound of the queue of a viewer in bytes (DI5), and the largest value of `warteschlange_max`.
pub const WARTESCHLANGE_MAX: usize = 33_554_432;
/// The fewest columns of a session. The one place in the service: the carrier says it, the start file and the events of the
/// view follow it (decision of 20.11.2026).
pub const KLEINSTE_SPALTEN: usize = traeger::SPALTEN_MIN;

/// The keys on the top level of the start file; every other key is invalid.
const OBERSTE_SCHLUESSEL: [&str; 11] = [
    "socket",
    "betrachter",
    "kind",
    "groesse",
    "ruecklauf",
    "deflate",
    "pruefmodus",
    "anfang",
    "warteschlange_max",
    "lauf",
    "lokal",
];

/// The most raster numbers of the sessions of `Starten` at the same time (DI12 point 4, P25): the number is one byte in the raster
/// frame (RA), 0 is the session of `kind`. A raster number is used again once its session has ended and no viewer subscribes it.
pub const NUMMER_MAX: usize = 255;
/// The most identifiers `s-<n>` that `Starten` hands out in one run (DI12 point 4, P25): nine digits, the form `^s-[0-9]{1,9}$` that
/// the target lists and the runners check. An identifier is never used again; at a million starts a day it lasts over 2700 years.
pub const KENNUNG_MAX: usize = 999_999_999;
/// The most ended sessions of `Starten` the `Sitzungsliste` holds (DI12 point 5, P26, Projektentscheid, 23.11.2026): a later end that raises the
/// count above it takes the entry out that ended longest ago and that no viewer subscribes.
pub const BEENDET_MAX: usize = 256;

/// The key `lauf` of the start file (DI12): what `Starten` needs. The paths come from here and from nowhere else.
#[derive(Debug, Clone, PartialEq)]
pub struct Lauf {
    /// The program of every started session (absolute); its `argv` is `[programm, <start.json>]`.
    pub programm: String,
    /// The environment of every started child: exactly these variables.
    pub env: Vec<(String, String)>,
    /// Columns and rows of the PTY of a started session.
    pub groesse: (usize, usize),
    /// The path of the settings file (absolute).
    pub einstellungen: String,
    /// The folder of the session folders of the started program (absolute), optional: a number whose entry
    /// `<sitzungen>/s-<n>` still exists is not handed out again (DI12 point 13).
    pub sitzungen: Option<String>,
    /// The folder of the target lists of the sessions (optional, absolute): a number whose entry `<ziellisten>/s-<n>.zielliste.json` still
    /// exists is not handed out again either (DI12 point 15).
    pub ziellisten: Option<String>,
    /// The folder of the runtime of the sessions (optional, absolute): a number whose socket folder `<laufzeit>/s-<n>` still exists is not
    /// handed out again either (DI12 point 15).
    pub laufzeit: Option<String>,
    /// The marker file of a trial run (optional, absolute path, DI12 point 14): read once at the start of the service.
    pub marker_datei: Option<String>,
    /// The folder of the count of the markers (optional, absolute path, from the start file, DI12 point 14).
    pub marker_ordner: Option<String>,
    /// The most identifiers `Starten` hands out; [`KENNUNG_MAX`] unless the start file lowers it (test mode only).
    pub nummer_max: usize,
    /// The most raster numbers at the same time; [`NUMMER_MAX`] unless the start file lowers it (test mode only).
    pub rahmen_max: usize,
    /// The most ended sessions in the list; [`BEENDET_MAX`] unless the start file lowers it (test mode only).
    pub beendet_max: usize,
    /// Where the decision point is and who it must be (addendum "Kanal zur Stelle (L6, Nr. 98)"), optional: without it
    /// there is no channel and every `Freigabe` on the local way is `stelle_nicht_erreichbar`.
    pub stelle: Option<crate::stelle::Ort>,
    /// The file of the Bestand (optional, absolute path, Nachtrag 26.11.2026, DI-L5 point 3): the list of the sessions across a restart.
    pub bestand: Option<String>,
    /// The folder of the conversations of the harnesses (optional, absolute path, Nachtrag 27.11.2026, DI-E4 point 2 and DI12 point 15):
    /// `Loeschen` removes `<gespraeche>/<kennung>`, and a number whose entry `<gespraeche>/s-<n>` exists is not handed out again.
    pub gespraeche: Option<String>,
}

/// What the start file says, checked.
#[derive(Debug, Clone, PartialEq)]
pub struct Startdatei {
    /// The folder of the socket (absolute, not `/`).
    pub ordner: String,
    /// The file name of the socket.
    pub name: String,
    /// `ordner/name`, at most [`SOCKET_PFAD_MAX`] bytes.
    pub pfad: String,
    /// The person every viewer is.
    pub person: String,
    /// The room every viewer is in.
    pub raum: String,
    /// `fenster` or `browser`.
    pub ansicht: String,
    /// The program (absolute) and its arguments.
    pub argv: Vec<String>,
    /// The environment of the child: exactly these variables.
    pub env: Vec<(String, String)>,
    /// The working directory of the child (absolute).
    pub cwd: String,
    /// Columns and rows.
    pub groesse: (usize, usize),
    /// Lines of scrollback of the carrier.
    pub ruecklauf: usize,
    /// Raw Deflate on the raster frames.
    pub deflate: bool,
    /// The frames `Uhr`, `Stumm`, `Zustand`, `Kernfehler` and the clock from outside (DI9).
    pub pruefmodus: bool,
    /// The upper bound of the queue of a viewer (DI5); below [`WARTESCHLANGE_MAX`] only in test mode.
    pub warteschlange_max: usize,
    /// The start of the core (`VT` section 3) with exactly one session.
    pub anfang: Value,
    /// The identifier of that session; empty without a first session (`mit_kind` false).
    pub sid: String,
    /// `true` if the start file has `kind` and `groesse`, so that the service starts a first session (number 0). Without it
    /// (only with `lauf`) `argv`, `env`, `cwd` and `groesse` are empty and the service starts with no session (DI12).
    pub mit_kind: bool,
    /// The key `lauf`: several sessions (DI12). `None`: the single mode.
    pub lauf: Option<Lauf>,
    /// The channel name of the key `lokal` (DI-L11 point 1): the local way for viewers without the bridge, the Unix socket
    /// `ordner/<kanal>.sock` (Windows: the named pipe `\\.\pipe\<kanal>`). `None`: no local way.
    pub lokal: Option<String>,
}

fn ungueltig<T>(grund: &str) -> Result<T, StartFehler> {
    Err(StartFehler::neu(
        2,
        &format!("Startdatei ungueltig: {grund}"),
    ))
}

/// A whole number that is no truth value.
fn ganz(wert: &Value) -> Option<i64> {
    wert.as_i64()
}

/// The depth of a JSON value as the contract counts it: the top level is 1, every nesting adds 1.
fn tiefe(wert: &Value) -> usize {
    let mut tiefste = 0;
    let mut stapel = vec![(wert, 1_usize)];
    while let Some((x, n)) = stapel.pop() {
        tiefste = tiefste.max(n);
        match x {
            Value::Array(liste) => stapel.extend(liste.iter().map(|v| (v, n + 1))),
            Value::Object(karte) => stapel.extend(karte.values().map(|v| (v, n + 1))),
            _ => {}
        }
    }
    tiefste
}

fn env_ok(wert: &Value) -> Option<Vec<(String, String)>> {
    let karte = wert.as_object()?;
    if karte.len() > 256 {
        return None;
    }
    let mut aus = Vec::with_capacity(karte.len());
    for (name, wert) in karte {
        let wert = wert.as_str()?;
        if name.is_empty() || name.contains('=') || name.contains('\0') || wert.contains('\0') {
            return None;
        }
        aus.push((name.clone(), wert.to_owned()));
    }
    Some(aus)
}

fn text_ok(wert: &Value, laenge_max: usize) -> Option<&str> {
    let t = wert.as_str()?;
    (!t.is_empty() && t.len() <= laenge_max && !t.contains('\0')).then_some(t)
}

/// Reads and checks the start file (exit code 2 on every error).
///
/// # Errors
/// The file is missing, unreadable, larger than [`MAX_START`] bytes, not UTF-8, not JSON, deeper than 128 levels or invalid.
pub fn lesen(pfad: &Path) -> Result<Startdatei, StartFehler> {
    let nicht_lesbar =
        |e: &std::io::Error| StartFehler::neu(2, &format!("Startdatei nicht lesbar: {}", e.kind()));
    let datei = File::open(pfad).map_err(|e| nicht_lesbar(&e))?;
    let mut roh = Vec::new();
    datei
        .take(MAX_START as u64 + 1)
        .read_to_end(&mut roh)
        .map_err(|e| nicht_lesbar(&e))?;
    if roh.len() > MAX_START {
        return ungueltig_gross();
    }
    let text = std::str::from_utf8(&roh)
        .map_err(|_| StartFehler::neu(2, "Startdatei nicht lesbar: kein UTF-8"))?;
    let wert: Value = serde_json::from_str(text)
        .map_err(|_| StartFehler::neu(2, "Startdatei nicht lesbar: kein JSON"))?;
    if tiefe(&wert) > 128 {
        return Err(StartFehler::neu(
            2,
            "Startdatei ist tiefer als 128 Ebenen verschachtelt",
        ));
    }
    pruefen(&wert)
}

fn ungueltig_gross() -> Result<Startdatei, StartFehler> {
    Err(StartFehler::neu(
        2,
        "Startdatei ist groesser als 1048576 Byte",
    ))
}

/// Checks the start as a JSON value.
///
/// # Errors
/// Exit code 2 with the reason: every rule of section 3 of the contract.
pub fn pruefen(o: &Value) -> Result<Startdatei, StartFehler> {
    let Some(o) = o.as_object() else {
        return ungueltig("kein Objekt");
    };
    if let Some(k) = o.keys().find(|k| !OBERSTE_SCHLUESSEL.contains(&k.as_str())) {
        return ungueltig(&format!("unbekannter Schluessel {k}"));
    }
    let lauf_da = o.contains_key("lauf");
    let pflicht: &[&str] = if lauf_da {
        &["socket", "betrachter", "anfang"]
    } else {
        &["socket", "betrachter", "kind", "groesse", "anfang"]
    };
    for k in pflicht {
        if !o.contains_key(*k) {
            return ungueltig(&format!("Feld fehlt: {k}"));
        }
    }
    let mit_kind = o.contains_key("kind");
    if mit_kind != o.contains_key("groesse") {
        return ungueltig("kind und groesse gehoeren zusammen (beide oder keines)");
    }
    let pruefmodus = wahrheit(o.get("pruefmodus"), false)?;
    let lauf = match o.get("lauf") {
        None => None,
        Some(l) => Some(lauf_lesen(l, pruefmodus)?),
    };
    let (ordner, name) = socket_lesen(&o["socket"])?;
    let pfad = format!("{}/{name}", ordner.trim_end_matches('/'));
    if pfad.len() > SOCKET_PFAD_MAX {
        return ungueltig(&format!(
            "Socket-Pfad ist laenger als {SOCKET_PFAD_MAX} Byte"
        ));
    }
    let lokal = match o.get("lokal") {
        None => None,
        Some(l) => Some(lokal_lesen(l, &ordner, &name)?),
    };
    let (person, raum, ansicht) = betrachter_lesen(&o["betrachter"])?;
    let (argv, env, cwd) = match o.get("kind") {
        Some(k) => kind_lesen(k)?,
        None => (Vec::new(), Vec::new(), String::new()),
    };
    let groesse = match o.get("groesse") {
        Some(g) => groesse_lesen(g)?,
        None => (0, 0),
    };
    let ruecklauf = match o.get("ruecklauf") {
        None => traeger::RUECKLAUF_VORGABE,
        Some(w) => match ganz(w).and_then(|n| usize::try_from(n).ok()) {
            Some(n) if n <= 100_000 => n,
            _ => return ungueltig("ruecklauf"),
        },
    };
    let deflate = wahrheit(o.get("deflate"), true)?;
    let warteschlange_max = match o.get("warteschlange_max") {
        None => WARTESCHLANGE_MAX,
        Some(w) => match ganz(w).and_then(|n| usize::try_from(n).ok()) {
            Some(n) if (1..=WARTESCHLANGE_MAX).contains(&n) => {
                if !pruefmodus {
                    return ungueltig("warteschlange_max nur im Pruefmodus");
                }
                n
            }
            _ => return ungueltig("warteschlange_max"),
        },
    };
    let (anfang, sid) = anfang_lesen(
        &o["anfang"],
        mit_kind.then_some(groesse),
        &person,
        lauf.is_some(),
    )?;
    Ok(Startdatei {
        ordner,
        name,
        pfad,
        person,
        raum,
        ansicht,
        argv,
        env,
        cwd,
        groesse,
        ruecklauf,
        deflate,
        pruefmodus,
        warteschlange_max,
        anfang,
        sid,
        mit_kind,
        lauf,
        lokal,
    })
}

/// A path of the key `lauf`: text of 1 to 4096 bytes, no NUL, absolute.
fn lauf_pfad(wert: Option<&Value>) -> Option<String> {
    let t = text_ok(wert?, 4096)?;
    t.starts_with('/').then(|| t.to_owned())
}

/// An optional path of the key `lauf`: missing gives `None`, otherwise it must be a path of [`lauf_pfad`].
fn lauf_pfad_wahl(
    karte: &Map<String, Value>,
    schluessel: &str,
) -> Result<Option<String>, StartFehler> {
    match karte.get(schluessel) {
        None => Ok(None),
        Some(w) => match lauf_pfad(Some(w)) {
            Some(p) => Ok(Some(p)),
            None => ungueltig(&format!("lauf.{schluessel} muss ein absoluter Pfad sein")),
        },
    }
}

/// The key `lauf` (DI12): exactly `programm`, `env`, `groesse`, `einstellungen`; `nummer_max`, `rahmen_max` and `beendet_max` only in test mode.
#[allow(
    clippy::too_many_lines,
    reason = "one check per key of `lauf`, in the order of the contract"
)]
fn lauf_lesen(wert: &Value, pruefmodus: bool) -> Result<Lauf, StartFehler> {
    let Some(karte) = wert.as_object() else {
        return ungueltig("lauf");
    };
    let pflicht = ["programm", "env", "groesse", "einstellungen"];
    if karte.keys().any(|k| {
        !pflicht.contains(&k.as_str())
            && ![
                "nummer_max",
                "rahmen_max",
                "beendet_max",
                "sitzungen",
                "ziellisten",
                "laufzeit",
                "marker_datei",
                "marker_ordner",
                "stelle",
                "bestand",
                "gespraeche",
            ]
            .contains(&k.as_str())
    }) || !pflicht.iter().all(|k| karte.contains_key(*k))
    {
        return ungueltig("lauf");
    }
    let Some(programm) = lauf_pfad(karte.get("programm")) else {
        return ungueltig("lauf.programm muss ein absoluter Pfad sein");
    };
    let Some(einstellungen) = lauf_pfad(karte.get("einstellungen")) else {
        return ungueltig("lauf.einstellungen muss ein absoluter Pfad sein");
    };
    let sitzungen = lauf_pfad_wahl(karte, "sitzungen")?;
    let ziellisten = lauf_pfad_wahl(karte, "ziellisten")?;
    let laufzeit = lauf_pfad_wahl(karte, "laufzeit")?;
    let marker_datei = lauf_pfad_wahl(karte, "marker_datei")?;
    let marker_ordner = lauf_pfad_wahl(karte, "marker_ordner")?;
    let bestand = lauf_pfad_wahl(karte, "bestand")?;
    let gespraeche = lauf_pfad_wahl(karte, "gespraeche")?;
    let Some(env) = env_ok(&karte["env"]) else {
        return ungueltig("lauf.env");
    };
    let nummer_max = match karte.get("nummer_max") {
        None => KENNUNG_MAX,
        Some(w) => match ganz(w).and_then(|n| usize::try_from(n).ok()) {
            Some(n) if (1..=KENNUNG_MAX).contains(&n) && pruefmodus => n,
            _ => return ungueltig("lauf.nummer_max"),
        },
    };
    let rahmen_max = match karte.get("rahmen_max") {
        None => NUMMER_MAX,
        Some(w) => match ganz(w).and_then(|n| usize::try_from(n).ok()) {
            Some(n) if (1..=NUMMER_MAX).contains(&n) && pruefmodus => n,
            _ => return ungueltig("lauf.rahmen_max"),
        },
    };
    let Some(beendet_max) = testgrenze(karte.get("beendet_max"), BEENDET_MAX, pruefmodus) else {
        return ungueltig("lauf.beendet_max");
    };
    let Ok(groesse) = groesse_lesen(&karte["groesse"]) else {
        return ungueltig("lauf.groesse");
    };
    let stelle = stelle_lesen(karte.get("stelle"))?;
    Ok(Lauf {
        programm,
        env,
        groesse,
        einstellungen,
        sitzungen,
        ziellisten,
        laufzeit,
        marker_datei,
        marker_ordner,
        nummer_max,
        rahmen_max,
        beendet_max,
        stelle,
        bestand,
        gespraeche,
    })
}

/// `lauf.stelle` (addendum "Kanal zur Stelle (L6, Nr. 98)"): optional; `erwartet` is mandatory, so there is no channel
/// without the expected identity of the point and no fallback to whoever sits at the socket.
fn stelle_lesen(wert: Option<&Value>) -> Result<Option<crate::stelle::Ort>, StartFehler> {
    match wert {
        None => Ok(None),
        Some(w) => crate::stelle::ort_lesen(w)
            .map(Some)
            .map_or_else(|| ungueltig("lauf_stelle_ungueltig"), Ok),
    }
}

/// A limit that only test mode may lower: missing gives `max`, otherwise a whole number from 1 to `max` in test mode; anything else is
/// `None` (P26, `lauf.beendet_max`).
fn testgrenze(wert: Option<&Value>, max: usize, pruefmodus: bool) -> Option<usize> {
    let Some(w) = wert else {
        return Some(max);
    };
    ganz(w)
        .and_then(|n| usize::try_from(n).ok())
        .filter(|n| (1..=max).contains(n) && pruefmodus)
}

/// `true` for an identifier of the form `s-<n>` (digits only): those belong to `Starten` (DI12).
pub(crate) fn ist_start_kennung(id: &str) -> bool {
    id.strip_prefix("s-")
        .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
}

fn wahrheit(wert: Option<&Value>, vorgabe: bool) -> Result<bool, StartFehler> {
    match wert {
        None => Ok(vorgabe),
        Some(Value::Bool(b)) => Ok(*b),
        Some(_) => ungueltig("deflate oder pruefmodus"),
    }
}

fn socket_lesen(wert: &Value) -> Result<(String, String), StartFehler> {
    let Some(karte) = wert.as_object() else {
        return ungueltig("socket");
    };
    if karte.keys().any(|k| k != "ordner" && k != "name") {
        return ungueltig("socket");
    }
    let Some(ordner) = karte.get("ordner").and_then(Value::as_str) else {
        return ungueltig("socket");
    };
    let name = match karte.get("name") {
        None => "dienst.sock",
        Some(Value::String(n)) => n.as_str(),
        Some(_) => return ungueltig("socket.name"),
    };
    if name.is_empty()
        || name == "."
        || name == ".."
        || name.contains('\0')
        || name.len() > 64
        || name.contains('/')
        || name.starts_with("..")
        // Windows (section 21): a backslash separates folders and a colon names a stream; neither belongs in a plain name
        || (cfg!(windows) && name.contains(['\\', ':']))
    {
        return ungueltig("socket.name");
    }
    if !ordner.starts_with('/') || ordner.contains('\0') || ordner == "/" {
        return ungueltig("socket.ordner muss ein absoluter Pfad sein");
    }
    Ok((ordner.to_owned(), name.to_owned()))
}

/// The key `lokal` (DI-L11 point 1): exactly `{"kanal": <name>}`, the name in the form of a channel of `peerkanal`
/// (`^[a-z0-9][a-z0-9._-]{0,63}$`). The path `ordner/<kanal>.sock` has at most [`SOCKET_PFAD_MAX`] bytes and is not the path of
/// `socket.name`. The rule holds on every system, so that one start file means the same everywhere.
fn lokal_lesen(wert: &Value, ordner: &str, name: &str) -> Result<String, StartFehler> {
    let Some(karte) = wert.as_object() else {
        return ungueltig("lokal");
    };
    let kanal = match (karte.len(), karte.get("kanal")) {
        (1, Some(Value::String(k))) => k.as_str(),
        _ => return ungueltig("lokal"),
    };
    if peerkanal::Kanalname::neu(kanal).is_err() {
        return ungueltig("lokal.kanal");
    }
    let datei = format!("{kanal}.sock");
    if datei == name {
        return ungueltig("lokal.kanal ist der Name des Sockets");
    }
    if format!("{}/{datei}", ordner.trim_end_matches('/')).len() > SOCKET_PFAD_MAX {
        return ungueltig(&format!(
            "Pfad von lokal ist laenger als {SOCKET_PFAD_MAX} Byte"
        ));
    }
    Ok(kanal.to_owned())
}

fn betrachter_lesen(wert: &Value) -> Result<(String, String, String), StartFehler> {
    let Some(karte) = wert.as_object() else {
        return ungueltig("betrachter");
    };
    let feld = |k: &str| karte.get(k).and_then(Value::as_str);
    match (karte.len(), feld("person"), feld("raum"), feld("ansicht")) {
        (3, Some(p), Some(r), Some(a)) if a == "fenster" || a == "browser" => {
            Ok((p.to_owned(), r.to_owned(), a.to_owned()))
        }
        _ => ungueltig("betrachter"),
    }
}

type KindFelder = (Vec<String>, Vec<(String, String)>, String);

fn kind_lesen(wert: &Value) -> Result<KindFelder, StartFehler> {
    let Some(karte) = wert.as_object() else {
        return ungueltig("kind");
    };
    if karte
        .keys()
        .any(|k| !["argv", "env", "cwd"].contains(&k.as_str()))
        || !karte.contains_key("argv")
        || !karte.contains_key("env")
    {
        return ungueltig("kind");
    }
    let argv: Option<Vec<String>> = karte["argv"].as_array().and_then(|liste| {
        if (1..=256).contains(&liste.len()) {
            liste
                .iter()
                .map(|x| text_ok(x, 4096).map(str::to_owned))
                .collect()
        } else {
            None
        }
    });
    let Some(argv) = argv else {
        return ungueltig("kind.argv");
    };
    if !argv[0].starts_with('/') {
        return ungueltig("kind.argv[0] muss ein absoluter Pfad sein");
    }
    let Some(env) = env_ok(&karte["env"]) else {
        return ungueltig("kind.env");
    };
    let cwd = match karte.get("cwd") {
        None => "/",
        Some(Value::String(c)) if c.starts_with('/') && !c.contains('\0') => c.as_str(),
        Some(_) => return ungueltig("kind.cwd"),
    };
    Ok((argv, env, cwd.to_owned()))
}

fn groesse_lesen(wert: &Value) -> Result<(usize, usize), StartFehler> {
    let paar = wert.as_array().filter(|l| l.len() == 2).and_then(|l| {
        let g = |x: &Value| ganz(x).and_then(|n| usize::try_from(n).ok());
        Some((g(&l[0])?, g(&l[1])?))
    });
    let Some((spalten, zeilen)) = paar else {
        return ungueltig("groesse");
    };
    if zeilen < 1 || spalten > 1000 || zeilen > 1000 {
        return ungueltig("groesse");
    }
    if spalten < KLEINSTE_SPALTEN {
        return ungueltig(&format!(
            "groesse hat weniger als {KLEINSTE_SPALTEN} Spalten"
        ));
    }
    Ok((spalten, zeilen))
}

fn anfang_lesen(
    wert: &Value,
    groesse: Option<(usize, usize)>,
    person: &str,
    lauf: bool,
) -> Result<(Value, String), StartFehler> {
    let Some(karte) = wert.as_object() else {
        return ungueltig("anfang");
    };
    let mut wert = wert.clone();
    let sid = if let Some(groesse) = groesse {
        let Some(sitzungen) = karte
            .get("sitzungen")
            .and_then(Value::as_array)
            .filter(|l| l.len() == 1)
        else {
            return ungueltig("anfang.sitzungen muss genau eine Sitzung nennen");
        };
        let Some(s0) = sitzungen[0].as_object() else {
            return ungueltig("anfang.sitzungen");
        };
        let Some(sid) = s0.get("id").and_then(Value::as_str) else {
            return ungueltig("Sitzung ohne id");
        };
        if lauf && ist_start_kennung(sid) {
            return ungueltig("anfang.sitzungen[0].id gehoert mit lauf den Sitzungen von Starten");
        }
        for feld in ["zeilen", "stand", "bracketed_paste", "invers", "cursor"] {
            if s0.contains_key(feld) {
                return ungueltig(&format!("anfang.sitzungen[0].{feld} gehoert dem Traeger"));
            }
        }
        if s0
            .get("groesse")
            .is_some_and(|g| *g != serde_json::json!([groesse.0, groesse.1]))
        {
            return ungueltig("anfang.sitzungen[0].groesse widerspricht groesse");
        }
        sid.to_owned()
    } else {
        // DI12: without `kind` the service starts with no session; `sitzungen` is absent, null or empty
        if !matches!(karte.get("sitzungen"), None | Some(Value::Null))
            && karte.get("sitzungen").and_then(Value::as_array) != Some(&Vec::new())
        {
            return ungueltig("anfang.sitzungen muss ohne kind fehlen oder leer sein");
        }
        if let Some(o) = wert.as_object_mut() {
            o.insert("sitzungen".to_owned(), serde_json::json!([]));
        }
        String::new()
    };
    if !karte
        .get("personen")
        .and_then(Value::as_object)
        .is_some_and(|p| p.contains_key(person))
    {
        return ungueltig("betrachter.person fehlt in anfang.personen");
    }
    if let Err(e) = Sitzungskern::neu(&wert) {
        return ungueltig(&format!("anfang: {e}"));
    }
    Ok((wert, sid))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn gut() -> Value {
        json!({
            "socket": {"ordner": "/tmp/x"},
            "betrachter": {"person": "anna", "raum": "raum-nord", "ansicht": "fenster"},
            "kind": {"argv": ["/bin/echo", "a"], "env": {"TERM": "xterm"}},
            "groesse": [40, 10],
            "anfang": {"personen": {"anna": {"art": "mensch", "rolle": "nutzer"}},
                       "sitzungen": [{"id": "s-alpha", "kundenraum": "raum-nord", "person": "anna"}]}
        })
    }

    fn mit(f: impl FnOnce(&mut Map<String, Value>)) -> Result<Startdatei, StartFehler> {
        let mut v = gut();
        if let Some(o) = v.as_object_mut() {
            f(o);
        }
        pruefen(&v)
    }

    fn rot(r: &Result<Startdatei, StartFehler>) {
        match r {
            Err(f) => {
                assert_eq!(f.code, 2);
                assert_ne!(f.text, "");
            }
            Ok(_) => panic!("accepted a start file that is invalid"),
        }
    }

    #[test]
    fn gueltig_mit_vorgaben() {
        let Ok(s) = pruefen(&gut()) else {
            panic!("valid start file");
        };
        assert_eq!(s.ordner, "/tmp/x");
        assert_eq!(s.name, "dienst.sock");
        assert_eq!(s.pfad, "/tmp/x/dienst.sock");
        assert_eq!((s.person.as_str(), s.raum.as_str()), ("anna", "raum-nord"));
        assert_eq!(s.ansicht, "fenster");
        assert_eq!(s.argv, vec!["/bin/echo", "a"]);
        assert_eq!(s.env, vec![("TERM".to_owned(), "xterm".to_owned())]);
        assert_eq!(s.cwd, "/");
        assert_eq!(s.groesse, (40, 10));
        assert_eq!(s.ruecklauf, 2000);
        assert!(s.deflate);
        assert!(!s.pruefmodus);
        assert_eq!(s.warteschlange_max, 33_554_432);
        assert_eq!(s.sid, "s-alpha");
    }

    #[test]
    fn unbekannter_schluessel_und_fehlende_felder() {
        rot(&mit(|o| {
            o.insert("lauf".into(), json!({}));
        }));
        rot(&mit(|o| {
            o.insert("deflat".into(), json!(true));
        }));
        for k in ["socket", "betrachter", "kind", "groesse", "anfang"] {
            rot(&mit(|o| {
                o.remove(k);
            }));
        }
        rot(&pruefen(&json!([])));
        rot(&pruefen(&json!("x")));
    }

    /// DI-L11 point 1: `lokal` is optional, exactly `{"kanal": <name>}`, the name in the form of `peerkanal`, the path at most 100
    /// bytes and not the path of `socket.name`; every other form is exit code 2.
    #[test]
    fn lokal() {
        let Ok(s) = pruefen(&gut()) else {
            panic!("valid start file");
        };
        assert_eq!(s.lokal, None);
        let mit_lokal = |l: Value| {
            mit(move |o| {
                o.insert("lokal".into(), l);
            })
        };
        let Ok(s) = mit_lokal(json!({"kanal": "fenster"})) else {
            panic!("valid key lokal");
        };
        assert_eq!(s.lokal.as_deref(), Some("fenster"));
        for gut in ["a", "0", "fenster.1", "a-b_c", &"k".repeat(64)] {
            assert_eq!(
                mit_lokal(json!({"kanal": gut})).ok().and_then(|s| s.lokal),
                Some(gut.to_owned()),
                "{gut}"
            );
        }
        for schlecht in [
            json!({"kanal": "Fenster"}),
            json!({"kanal": ""}),
            json!({"kanal": "-a"}),
            json!({"kanal": ".a"}),
            json!({"kanal": "a/b"}),
            json!({"kanal": "k".repeat(65)}),
            json!({"kanal": 1}),
            json!({"kanal": "fenster", "x": 1}),
            json!({}),
            json!("fenster"),
            json!(null),
            // the same path as the socket of the bridge
            json!({"kanal": "dienst"}),
        ] {
            rot(&mit_lokal(schlecht));
        }
        // the name of the socket counts, not the default
        let anders = mit(|o| {
            o.insert(
                "socket".into(),
                json!({"ordner": "/tmp/x", "name": "a.sock"}),
            );
            o.insert("lokal".into(), json!({"kanal": "dienst"}));
        });
        assert_eq!(anders.ok().and_then(|s| s.lokal).as_deref(), Some("dienst"));
        rot(&mit(|o| {
            o.insert(
                "socket".into(),
                json!({"ordner": "/tmp/x", "name": "a.sock"}),
            );
            o.insert("lokal".into(), json!({"kanal": "a"}));
        }));
        // 100 bytes are the limit of the path `<ordner>/<kanal>.sock`
        let ordner = format!("/{}", "o".repeat(36));
        let mit_ordner = |kanal: &str| {
            let ordner = ordner.clone();
            let kanal = kanal.to_owned();
            mit(move |o| {
                o.insert("socket".into(), json!({"ordner": ordner}));
                o.insert("lokal".into(), json!({"kanal": kanal}));
            })
        };
        // 37 + 1 + 57 + 5 = 100 bytes
        assert!(mit_ordner(&"k".repeat(57)).is_ok());
        rot(&mit_ordner(&"k".repeat(58)));
        // a slash at the end of the folder does not count twice
        let mit_strich = mit(|o| {
            o.insert(
                "socket".into(),
                json!({"ordner": format!("/{}/", "o".repeat(36))}),
            );
            o.insert("lokal".into(), json!({"kanal": "k".repeat(57)}));
        });
        assert!(mit_strich.is_ok());
    }

    #[test]
    fn socket() {
        let name = |n: Value| {
            mit(move |o| {
                o.insert("socket".into(), json!({"ordner": "/tmp/x", "name": n}));
            })
        };
        for gut in ["a", "dienst.sock", "x.y", "a..b", "."] {
            if gut == "." {
                rot(&name(json!(gut)));
            } else {
                assert!(name(json!(gut)).is_ok(), "{gut}");
            }
        }
        for schlecht in [
            json!(""),
            json!(".."),
            json!("a/b"),
            json!("..x"),
            json!("a\u{0}b"),
            json!(1),
            json!("n".repeat(65)),
        ] {
            rot(&name(schlecht));
        }
        assert!(name(json!("n".repeat(64))).is_ok(), "64 bytes are valid");
        let ordner = |o: Value| {
            mit(move |m| {
                m.insert("socket".into(), json!({"ordner": o}));
            })
        };
        assert!(ordner(json!("/tmp/x/")).is_ok());
        for schlecht in [json!("/"), json!("tmp"), json!("/a\u{0}"), json!(5)] {
            rot(&ordner(schlecht));
        }
        rot(&mit(|o| {
            o.insert("socket".into(), json!({"ordner": "/tmp/x", "extra": 1}));
        }));
        rot(&mit(|o| {
            o.insert("socket".into(), json!({"name": "a"}));
        }));
        rot(&mit(|o| {
            o.insert("socket".into(), json!([]));
        }));
    }

    #[test]
    fn socketname_mit_backslash_oder_doppelpunkt_nur_unter_windows_ungueltig() {
        // section 21 point 10: only Windows forbids `\` and `:` in `socket.name`; elsewhere they are plain bytes of the name
        for n in ["a\\b", "a:b", ":", "\\"] {
            let r = mit(move |o| {
                o.insert("socket".into(), json!({"ordner": "/tmp/x", "name": n}));
            });
            if cfg!(windows) {
                rot(&r);
            } else {
                assert_eq!(r.map(|s| s.pfad), Ok(format!("/tmp/x/{n}")), "{n}");
            }
        }
    }

    #[test]
    fn pfad_grenze_100_byte() {
        // "/" + folder + "/" + name: exactly 100 bytes is valid, 101 is not
        let pfad = |n: usize| {
            let ordner = format!("/{}", "o".repeat(n - 1 - 1 - 4));
            mit(move |o| {
                o.insert("socket".into(), json!({"ordner": ordner, "name": "s.sk"}));
            })
        };
        let Ok(s) = pfad(100) else {
            panic!("100 bytes are valid");
        };
        assert_eq!(s.pfad.len(), 100);
        rot(&pfad(101));
        // trailing slashes of the folder do not count
        assert_eq!(
            mit(|o| {
                o.insert("socket".into(), json!({"ordner": "/tmp/x//", "name": "a"}));
            })
            .map(|s| s.pfad),
            Ok("/tmp/x/a".to_owned())
        );
    }

    #[test]
    fn betrachter() {
        let b = |v: Value| {
            mit(move |o| {
                o.insert("betrachter".into(), v);
            })
        };
        assert!(b(json!({"person": "anna", "raum": "r", "ansicht": "browser"})).is_ok());
        rot(&b(json!({"person": "anna", "raum": "r", "ansicht": "x"})));
        rot(&b(json!({"person": "anna", "raum": "r"})));
        rot(&b(
            json!({"person": "anna", "raum": "r", "ansicht": "fenster", "x": 1}),
        ));
        rot(&b(json!({"person": 1, "raum": "r", "ansicht": "fenster"})));
        rot(&b(
            json!({"person": "bert", "raum": "r", "ansicht": "fenster"}),
        ));
        rot(&b(json!([])));
    }

    #[test]
    fn kind() {
        let k = |v: Value| {
            mit(move |o| {
                o.insert("kind".into(), v);
            })
        };
        assert!(k(json!({"argv": ["/a"], "env": {}, "cwd": "/tmp"})).is_ok());
        rot(&k(json!({"argv": [], "env": {}})));
        rot(&k(json!({"argv": ["a"], "env": {}})));
        rot(&k(json!({"argv": [""], "env": {}})));
        rot(&k(json!({"argv": ["/a\u{0}"], "env": {}})));
        rot(&k(json!({"argv": ["/a"]})));
        rot(&k(json!({"env": {}})));
        rot(&k(json!({"argv": ["/a"], "env": {}, "cwd": "tmp"})));
        rot(&k(json!({"argv": ["/a"], "env": {}, "cwd": 1})));
        rot(&k(json!({"argv": ["/a"], "env": {}, "x": 1})));
        rot(&k(json!({"argv": "/a", "env": {}})));
        rot(&k(json!({"argv": [1], "env": {}})));
        rot(&k(json!([])));
        let viele: Vec<String> = (0..257).map(|_| "/a".to_owned()).collect();
        rot(&k(json!({"argv": viele, "env": {}})));
        let genau: Vec<String> = (0..256).map(|_| "/a".to_owned()).collect();
        assert!(k(json!({"argv": genau, "env": {}})).is_ok());
        assert!(k(json!({"argv": [format!("/{}", "a".repeat(4095))], "env": {}})).is_ok());
        rot(&k(
            json!({"argv": [format!("/{}", "a".repeat(4096))], "env": {}}),
        ));
    }

    #[test]
    fn umgebung() {
        let e = |v: Value| {
            mit(move |o| {
                o.insert("kind".into(), json!({"argv": ["/a"], "env": v}));
            })
        };
        assert!(e(json!({"A": "b", "B": ""})).is_ok());
        rot(&e(json!({"": "b"})));
        rot(&e(json!({"A=": "b"})));
        rot(&e(json!({"A\u{0}": "b"})));
        rot(&e(json!({"A": "b\u{0}"})));
        rot(&e(json!({"A": 1})));
        rot(&e(json!([])));
        let genau: Map<String, Value> = (0..256).map(|i| (format!("V{i}"), json!("x"))).collect();
        assert!(e(Value::Object(genau.clone())).is_ok());
        let mut zu_viel = genau;
        zu_viel.insert("V256".into(), json!("x"));
        rot(&e(Value::Object(zu_viel)));
    }

    #[test]
    fn groesse() {
        let g = |v: Value| {
            mit(move |o| {
                o.insert("groesse".into(), v);
            })
        };
        assert_eq!(g(json!([2, 1])).map(|s| s.groesse), Ok((2, 1)));
        assert_eq!(g(json!([1000, 1000])).map(|s| s.groesse), Ok((1000, 1000)));
        for schlecht in [
            json!([1, 10]),
            json!([0, 10]),
            json!([40, 0]),
            json!([1001, 10]),
            json!([40, 1001]),
            json!([40]),
            json!([40, 10, 1]),
            json!([40.5, 10]),
            json!([true, 10]),
            json!([40, false]),
            json!(["40", 10]),
            json!([-40, 10]),
            json!(40),
        ] {
            rot(&g(schlecht));
        }
        // the sitzung may name the same size, not another
        let mit_groesse = |gr: Value| {
            mit(move |o| {
                o.insert(
                    "anfang".into(),
                    json!({"personen": {"anna": {}}, "sitzungen": [{"id": "s", "groesse": gr}]}),
                );
            })
        };
        assert!(mit_groesse(json!([40, 10])).is_ok());
        rot(&mit_groesse(json!([40, 11])));
    }

    #[test]
    fn kleinste_spalten_ist_die_des_traegers() {
        assert_eq!(KLEINSTE_SPALTEN, 2);
        assert_eq!(KLEINSTE_SPALTEN, traeger::SPALTEN_MIN);
    }

    #[test]
    fn ruecklauf_deflate_pruefmodus() {
        let r = |v: Value| {
            mit(move |o| {
                o.insert("ruecklauf".into(), v);
            })
        };
        assert_eq!(r(json!(0)).map(|s| s.ruecklauf), Ok(0));
        assert_eq!(r(json!(100_000)).map(|s| s.ruecklauf), Ok(100_000));
        for schlecht in [
            json!(100_001),
            json!(-1),
            json!(1.5),
            json!("1"),
            json!(true),
        ] {
            rot(&r(schlecht));
        }
        let d = |k: &'static str, v: Value| {
            mit(move |o| {
                o.insert(k.into(), v);
            })
        };
        assert_eq!(d("deflate", json!(false)).map(|s| s.deflate), Ok(false));
        assert_eq!(d("pruefmodus", json!(true)).map(|s| s.pruefmodus), Ok(true));
        rot(&d("deflate", json!(1)));
        rot(&d("pruefmodus", json!("true")));
    }

    #[test]
    fn warteschlange_max() {
        let w = |v: Value, pm: bool| {
            mit(move |o| {
                o.insert("warteschlange_max".into(), v);
                o.insert("pruefmodus".into(), json!(pm));
            })
        };
        assert_eq!(w(json!(1), true).map(|s| s.warteschlange_max), Ok(1));
        assert_eq!(
            w(json!(33_554_432), true).map(|s| s.warteschlange_max),
            Ok(33_554_432)
        );
        rot(&w(json!(33_554_433), true));
        rot(&w(json!(0), true));
        rot(&w(json!(-5), true));
        rot(&w(json!(2.5), true));
        rot(&w(json!("9"), true));
        rot(&w(json!(100), false));
    }

    #[test]
    fn anfang() {
        let a = |v: Value| {
            mit(move |o| {
                o.insert("anfang".into(), v);
            })
        };
        rot(&a(json!([])));
        rot(&a(json!({"personen": {"anna": {}}})));
        rot(&a(json!({"personen": {"anna": {}}, "sitzungen": []})));
        rot(&a(
            json!({"personen": {"anna": {}}, "sitzungen": [{"id": "a"}, {"id": "b"}]}),
        ));
        rot(&a(
            json!({"personen": {"anna": {}}, "sitzungen": [{"titel": "x"}]}),
        ));
        rot(&a(json!({"personen": {"anna": {}}, "sitzungen": ["x"]})));
        rot(&a(json!({"sitzungen": [{"id": "a"}]})));
        rot(&a(
            json!({"personen": {"bert": {}}, "sitzungen": [{"id": "a"}]}),
        ));
        rot(&a(json!({"personen": [], "sitzungen": [{"id": "a"}]})));
        for feld in ["zeilen", "stand", "bracketed_paste", "invers", "cursor"] {
            let mut s = json!({"id": "a"});
            s[feld] = json!(1);
            rot(&a(json!({"personen": {"anna": {}}, "sitzungen": [s]})));
        }
        // the core must take it: a person that is no object
        rot(&a(
            json!({"personen": {"anna": 5}, "sitzungen": [{"id": "a"}]}),
        ));
    }

    #[test]
    fn tiefe_zaehlt_wie_der_vertrag() {
        assert_eq!(tiefe(&json!(1)), 1);
        assert_eq!(tiefe(&json!({"a": 1})), 2);
        assert_eq!(tiefe(&json!([[]])), 2);
        assert_eq!(tiefe(&json!({"a": [1, {"b": 2}]})), 4);
    }

    fn datei(inhalt: &[u8]) -> std::path::PathBuf {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static ZAEHLER: AtomicUsize = AtomicUsize::new(0);
        let pfad = std::env::temp_dir().join(format!(
            "dienst-start-{}-{}.json",
            std::process::id(),
            ZAEHLER.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::write(&pfad, inhalt).expect("write start file");
        pfad
    }

    fn gelesen(inhalt: &[u8]) -> Result<Startdatei, StartFehler> {
        let pfad = datei(inhalt);
        let r = lesen(&pfad);
        let _ = std::fs::remove_file(&pfad);
        r
    }

    #[test]
    fn datei_lesen() {
        let text = gut().to_string();
        assert!(gelesen(text.as_bytes()).is_ok());
        rot(&gelesen(b"{"));
        rot(&gelesen(b""));
        rot(&gelesen(&[0xff, 0xfe]));
        rot(&gelesen(b"[1]"));
        let mut nan = text.clone();
        nan.insert_str(1, "\"x\": NaN,");
        rot(&gelesen(nan.as_bytes()));
        rot(&lesen(Path::new("/nicht/da/start.json")));
        rot(&lesen(Path::new("/")));
    }

    #[test]
    fn datei_grenze_ein_megabyte() {
        let mut text = gut().to_string().into_bytes();
        text.resize(MAX_START, b' ');
        assert!(gelesen(&text).is_ok(), "exactly 1 MiB is valid");
        text.push(b' ');
        rot(&gelesen(&text));
    }

    #[test]
    fn datei_tiefe_128() {
        // 126 arrays around a 1 make depth 127 + 1; the start object itself holds one more level: a field with n levels
        let tief = |n: usize| {
            let mut t = String::from("1");
            for _ in 0..n {
                t = format!("[{t}]");
            }
            let mut v = gut();
            v["anfang"]["personen"]["anna"]["x"] = Value::Null;
            let text = v.to_string().replacen("null", &t, 1);
            gelesen(text.as_bytes())
        };
        // anfang (2) + personen (3) + anna (4) + x (5) + n arrays + the number
        assert!(tief(120).is_ok());
        assert!(tief(123).is_ok(), "depth 128 exactly");
        rot(&tief(124));
        rot(&tief(200));
    }

    fn lauf_gut() -> Value {
        json!({"programm": "/bin/echo", "env": {"A": "1"}, "groesse": [30, 9], "einstellungen": "/etc/e.json"})
    }

    /// A start file with `lauf` and without `kind` and `groesse`.
    fn ohne_kind(f: impl FnOnce(&mut Map<String, Value>)) -> Result<Startdatei, StartFehler> {
        mit(|o| {
            o.remove("kind");
            o.remove("groesse");
            o.insert("anfang".into(), json!({"personen": {"anna": {}}}));
            o.insert("lauf".into(), lauf_gut());
            f(o);
        })
    }

    #[test]
    fn lauf_ohne_kind_startet_ohne_sitzung() {
        let Ok(s) = ohne_kind(|_| {}) else {
            panic!("valid");
        };
        assert!(!s.mit_kind);
        assert_eq!(s.sid, "");
        assert_eq!(
            (s.argv.len(), s.env.len(), s.cwd.as_str(), s.groesse),
            (0, 0, "", (0, 0))
        );
        assert_eq!(s.anfang["sitzungen"], json!([]));
        let l = s.lauf.expect("lauf");
        assert_eq!(l.programm, "/bin/echo");
        assert_eq!(l.env, vec![("A".to_owned(), "1".to_owned())]);
        assert_eq!(
            (l.groesse, l.einstellungen.as_str()),
            ((30, 9), "/etc/e.json")
        );
        assert_eq!(l.nummer_max, KENNUNG_MAX);
        assert_eq!(l.rahmen_max, NUMMER_MAX);
        assert_eq!(l.beendet_max, BEENDET_MAX);
    }

    #[test]
    fn lauf_mit_kind_und_nur_zusammen() {
        let Ok(s) = mit(|o| {
            o.insert("lauf".into(), lauf_gut());
        }) else {
            panic!("valid");
        };
        assert!(s.mit_kind && s.lauf.is_some());
        assert_eq!(s.sid, "s-alpha");
        assert_eq!(s.groesse, (40, 10));
        // `kind` without `groesse` and the other way round: only both or none
        rot(&ohne_kind(|o| {
            o.insert("kind".into(), json!({"argv": ["/bin/echo"], "env": {}}));
        }));
        rot(&ohne_kind(|o| {
            o.insert("groesse".into(), json!([40, 10]));
        }));
        // without `lauf` both stay required
        rot(&mit(|o| {
            o.remove("kind");
            o.remove("groesse");
        }));
        // `anfang` stays required with `lauf`
        rot(&ohne_kind(|o| {
            o.remove("anfang");
        }));
    }

    #[test]
    fn lauf_schluessel_und_werte() {
        let mit_lauf = |f: &dyn Fn(&mut Value)| {
            ohne_kind(|o| {
                let mut l = lauf_gut();
                f(&mut l);
                o.insert("lauf".into(), l);
            })
        };
        assert!(mit_lauf(&|_| {}).is_ok());
        for k in ["programm", "env", "groesse", "einstellungen"] {
            rot(&mit_lauf(&|l| {
                l.as_object_mut().expect("o").remove(k);
            }));
        }
        rot(&mit_lauf(&|l| l["fremd"] = json!(1)));
        rot(&ohne_kind(|o| {
            o.insert("lauf".into(), json!([]));
        }));
        for schlecht in ["", "relativ/pfad", "a\u{0}b"] {
            rot(&mit_lauf(&|l| l["programm"] = json!(schlecht)));
            rot(&mit_lauf(&|l| l["einstellungen"] = json!(schlecht)));
        }
        rot(&mit_lauf(&|l| l["programm"] = json!(5)));
        rot(&mit_lauf(&|l| l["einstellungen"] = json!(null)));
        let lang = format!("/{}", "a".repeat(4095));
        assert!(
            mit_lauf(&|l| l["programm"] = json!(lang)).is_ok(),
            "4096 bytes"
        );
        let zu_lang = format!("/{}", "a".repeat(4096));
        rot(&mit_lauf(&|l| l["programm"] = json!(zu_lang)));
        rot(&mit_lauf(&|l| l["einstellungen"] = json!(zu_lang)));
        rot(&mit_lauf(&|l| l["env"] = json!({"A=B": "1"})));
        rot(&mit_lauf(&|l| l["env"] = json!({"A": 1})));
        for g in [
            json!([1, 9]),
            json!([30, 0]),
            json!([1001, 9]),
            json!([30, 1001]),
            json!([30]),
            json!("x"),
            json!([true, 9]),
        ] {
            rot(&mit_lauf(&|l| l["groesse"] = g.clone()));
        }
        assert!(mit_lauf(&|l| l["groesse"] = json!([2, 1])).is_ok());
        assert!(mit_lauf(&|l| l["groesse"] = json!([1000, 1000])).is_ok());
    }

    #[test]
    fn lauf_sitzungen_optional_und_ein_absoluter_pfad() {
        let mit_lauf = |f: &dyn Fn(&mut Value)| {
            ohne_kind(|o| {
                let mut l = lauf_gut();
                f(&mut l);
                o.insert("lauf".into(), l);
            })
        };
        assert_eq!(
            mit_lauf(&|_| {}).expect("ok").lauf.expect("lauf").sitzungen,
            None
        );
        let l = mit_lauf(&|l| l["sitzungen"] = json!("/var/lib/s"))
            .expect("ok")
            .lauf
            .expect("lauf");
        assert_eq!(l.sitzungen.as_deref(), Some("/var/lib/s"));
        let lang = format!("/{}", "a".repeat(4095));
        assert!(mit_lauf(&|l| l["sitzungen"] = json!(lang)).is_ok());
        for schlecht in [
            json!("relativ"),
            json!(""),
            json!(5),
            json!(null),
            json!("/a\u{0}b"),
            json!(format!("/{}", "a".repeat(4096))),
        ] {
            let e = mit_lauf(&|l| l["sitzungen"] = schlecht.clone());
            assert!(
                format!("{e:?}").contains("lauf.sitzungen muss ein absoluter Pfad sein"),
                "{schlecht}: {e:?}"
            );
        }
    }

    #[test]
    fn lauf_ziellisten_und_laufzeit_optional_und_absolute_pfade() {
        let mit_lauf = |f: &dyn Fn(&mut Value)| {
            ohne_kind(|o| {
                let mut l = lauf_gut();
                f(&mut l);
                o.insert("lauf".into(), l);
            })
        };
        let l = mit_lauf(&|_| {}).expect("ok").lauf.expect("lauf");
        assert_eq!((l.ziellisten, l.laufzeit), (None, None));
        let l = mit_lauf(&|l| l["ziellisten"] = json!("/var/lib/werkbank/nutzer/sitzungen"))
            .expect("ok")
            .lauf
            .expect("lauf");
        assert_eq!(
            l.ziellisten.as_deref(),
            Some("/var/lib/werkbank/nutzer/sitzungen")
        );
        let l = mit_lauf(&|l| l["laufzeit"] = json!("/var/lib/werkbank/l/1000"))
            .expect("ok")
            .lauf
            .expect("lauf");
        assert_eq!(l.laufzeit.as_deref(), Some("/var/lib/werkbank/l/1000"));
        let lang = format!("/{}", "a".repeat(4095));
        assert!(mit_lauf(&|l| l["ziellisten"] = json!(lang)).is_ok());
        assert!(mit_lauf(&|l| l["laufzeit"] = json!(lang)).is_ok());
        for schluessel in ["ziellisten", "laufzeit"] {
            for schlecht in [
                json!("relativ"),
                json!(""),
                json!(5),
                json!(null),
                json!("/a\u{0}b"),
                json!(format!("/{}", "a".repeat(4096))),
            ] {
                let e = mit_lauf(&|l| l[schluessel] = schlecht.clone());
                assert!(
                    format!("{e:?}")
                        .contains(&format!("lauf.{schluessel} muss ein absoluter Pfad sein")),
                    "{schluessel} {schlecht}: {e:?}"
                );
            }
        }
    }

    #[test]
    fn lauf_marker_datei_und_marker_ordner_optional_und_absolute_pfade() {
        let mit_lauf = |f: &dyn Fn(&mut Value)| {
            ohne_kind(|o| {
                let mut l = lauf_gut();
                f(&mut l);
                o.insert("lauf".into(), l);
            })
        };
        let lauf = |a: Result<Startdatei, StartFehler>| a.expect("ok").lauf.expect("lauf");
        let l = lauf(mit_lauf(&|_| {}));
        assert_eq!((l.marker_datei, l.marker_ordner), (None, None));
        let l = lauf(mit_lauf(&|l| {
            l["marker_datei"] = json!("/run/user/1000/werkbank-probelauf/marker.json");
            l["marker_ordner"] = json!("/var/lib/z");
        }));
        assert_eq!(
            l.marker_datei.as_deref(),
            Some("/run/user/1000/werkbank-probelauf/marker.json")
        );
        assert_eq!(l.marker_ordner.as_deref(), Some("/var/lib/z"));
        let lang = format!("/{}", "a".repeat(4095));
        assert!(mit_lauf(&|l| l["marker_datei"] = json!(lang)).is_ok());
        assert!(mit_lauf(&|l| l["marker_ordner"] = json!(lang)).is_ok());
        for (schluessel, zeile) in [
            (
                "marker_datei",
                "lauf.marker_datei muss ein absoluter Pfad sein",
            ),
            (
                "marker_ordner",
                "lauf.marker_ordner muss ein absoluter Pfad sein",
            ),
        ] {
            for schlecht in [
                json!("relativ"),
                json!(""),
                json!(5),
                json!(null),
                json!("/a\u{0}b"),
                json!(format!("/{}", "a".repeat(4096))),
            ] {
                let e = mit_lauf(&|l| l[schluessel] = schlecht.clone());
                assert!(format!("{e:?}").contains(zeile), "{schlecht}: {e:?}");
            }
        }
    }

    #[test]
    fn lauf_nummer_max_nur_im_pruefmodus_von_1_bis_999999999() {
        let mit_max = |n: Value, pruefmodus: bool| {
            ohne_kind(|o| {
                o.insert("pruefmodus".into(), json!(pruefmodus));
                let mut l = lauf_gut();
                l["nummer_max"] = n;
                o.insert("lauf".into(), l);
            })
        };
        assert_eq!(
            mit_max(json!(1), true)
                .expect("1")
                .lauf
                .expect("l")
                .nummer_max,
            1
        );
        assert_eq!(
            mit_max(json!(999_999_999), true)
                .expect("999999999")
                .lauf
                .expect("l")
                .nummer_max,
            999_999_999
        );
        assert_eq!(
            mit_max(json!(7), true)
                .expect("7")
                .lauf
                .expect("l")
                .nummer_max,
            7
        );
        for schlecht in [
            json!(0),
            json!(1_000_000_000),
            json!(-1),
            json!(1.5),
            json!("3"),
            json!(true),
        ] {
            rot(&mit_max(schlecht, true));
        }
        rot(&mit_max(json!(3), false));
    }

    #[test]
    fn lauf_beendet_max_nur_im_pruefmodus_von_1_bis_256() {
        let mit_max = |n: Value, pruefmodus: bool| {
            ohne_kind(|o| {
                o.insert("pruefmodus".into(), json!(pruefmodus));
                let mut l = lauf_gut();
                l["beendet_max"] = n;
                o.insert("lauf".into(), l);
            })
        };
        for gut in [1, 3, 256] {
            let l = mit_max(json!(gut), true).expect("valid").lauf.expect("l");
            assert_eq!(l.beendet_max, gut);
        }
        for schlecht in [json!(0), json!(257), json!(-1), json!(1.5), json!("3")] {
            rot(&mit_max(schlecht, true));
        }
        rot(&mit_max(json!(3), false));
    }

    #[test]
    fn lauf_rahmen_max_nur_im_pruefmodus_von_1_bis_255() {
        let mit_max = |n: Value, pruefmodus: bool| {
            ohne_kind(|o| {
                o.insert("pruefmodus".into(), json!(pruefmodus));
                let mut l = lauf_gut();
                l["rahmen_max"] = n;
                o.insert("lauf".into(), l);
            })
        };
        for gut in [1, 7, 255] {
            let l = mit_max(json!(gut), true).expect("valid").lauf.expect("l");
            assert_eq!(l.rahmen_max, gut);
        }
        for schlecht in [
            json!(0),
            json!(256),
            json!(-1),
            json!(1.5),
            json!("3"),
            json!(true),
        ] {
            rot(&mit_max(schlecht, true));
        }
        rot(&mit_max(json!(3), false));
    }

    #[test]
    fn lauf_anfang_ohne_kind_und_kennungen_von_starten() {
        for gut in [json!(null), json!([])] {
            assert!(
                ohne_kind(|o| {
                    o.insert(
                        "anfang".into(),
                        json!({"personen": {"anna": {}}, "sitzungen": gut}),
                    );
                })
                .is_ok()
            );
        }
        rot(&ohne_kind(|o| {
            o.insert(
                "anfang".into(),
                json!({"personen": {"anna": {}}, "sitzungen": [{"id": "x"}]}),
            );
        }));
        rot(&ohne_kind(|o| {
            o.insert(
                "anfang".into(),
                json!({"personen": {"anna": {}}, "sitzungen": {}}),
            );
        }));
        rot(&ohne_kind(|o| {
            o.insert("anfang".into(), json!({"personen": {"bert": {}}}));
        }));
        // the identifier of the session of `kind` may not look like `s-<n>`
        let kennung = |id: &str| {
            mit(|o| {
                o.insert("lauf".into(), lauf_gut());
                o["anfang"]["sitzungen"][0]["id"] = json!(id);
            })
        };
        for belegt in ["s-1", "s-12", "s-01", "s-0"] {
            rot(&kennung(belegt));
        }
        for frei in ["s-", "s-a", "s-1a", "x-1", "s1", "S-1", "s--1", "s-alpha"] {
            assert!(kennung(frei).is_ok(), "{frei}");
        }
        // without `lauf` the identifier is free
        assert!(mit(|o| o["anfang"]["sitzungen"][0]["id"] = json!("s-1")).is_ok());
        assert!(ist_start_kennung("s-5") && !ist_start_kennung("s-") && !ist_start_kennung("s-5x"));
    }
}
