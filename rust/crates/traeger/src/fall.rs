//! The case runner of the program `tr`: reads one case (contract sections 2 and 3), runs it through [`Raster`] or [`Kind`]
//! and returns the result as a JSON value.

use std::fmt;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde_json::{Map, Value, json};

use crate::ereignis::Ereignis;
use crate::hex::{hex, unhex};
use crate::kind::{Kind, Start};
use crate::raster::{RUECKLAUF_VORGABE, Raster, groesse_gueltig};

/// The largest case in bytes; a longer input is an invalid case.
pub const EINGABE_MAX: usize = 4_194_304;

/// The largest number a step `Groesse` with `versuch` takes (contract section 3); more is an invalid case.
const VERSUCH_MAX: u64 = 100_000;

/// How long a step `Warte` waits for its goal.
const FRIST: Duration = Duration::from_secs(20);

/// The case is invalid; the text says why (it goes to stderr).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ungueltig(pub String);

impl fmt::Display for Ungueltig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Ungueltig {}

fn ungueltig<T>(was: &str) -> Result<T, Ungueltig> {
    Err(Ungueltig(was.to_owned()))
}

/// What a step of the child part waits for.
struct Ziel {
    text: Option<String>,
    kindende: bool,
    /// The cursor `(line, column, visible)` that the state shows (TR15, Nachtrag 21.11.): the text of a line is there before the
    /// line feed behind it is read, so a case that needs the cursor of the finished line waits for the cursor.
    cursor: Option<(usize, usize, bool)>,
}

/// The cursor of a goal `{"cursor": [line, column, visible]}`; `None` if the value has another form.
fn cursor_lesen(wert: &Value) -> Option<(usize, usize, bool)> {
    let liste = wert.as_array()?;
    match liste.as_slice() {
        [zeile, spalte, sichtbar] => Some((
            usize::try_from(zeile.as_u64()?).ok()?,
            usize::try_from(spalte.as_u64()?).ok()?,
            sichtbar.as_bool()?,
        )),
        _ => None,
    }
}

/// One step of a case.
enum Schritt {
    Text(String),
    Bytes(Vec<u8>),
    /// A size step; with `versuch` the size may lie outside the limits and the step is then refused, not invalid.
    Groesse {
        spalten: usize,
        zeilen: usize,
        versuch: bool,
    },
    Zustand,
    Warte(Ziel),
    Tasten(Vec<u8>),
}

fn text<'a>(wert: &'a Value, schluessel: &str) -> Result<&'a str, Ungueltig> {
    match wert.get(schluessel).and_then(Value::as_str) {
        Some(t) => Ok(t),
        None => ungueltig(schluessel),
    }
}

fn hex_feld(wert: &Value) -> Result<Vec<u8>, Ungueltig> {
    unhex(text(wert, "hex")?).map_or_else(|| ungueltig("hex"), Ok)
}

fn zahl(wert: &Value, schluessel: &str, von: u64, bis: u64) -> Result<usize, Ungueltig> {
    match wert.get(schluessel).and_then(Value::as_u64) {
        Some(n) if (von..=bis).contains(&n) => {
            usize::try_from(n).or_else(|_| ungueltig(schluessel))
        }
        _ => ungueltig(schluessel),
    }
}

fn groesse_lesen(fall: &Value) -> Result<(usize, usize), Ungueltig> {
    match fall
        .get("groesse")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
    {
        Some([spalten, zeilen]) => {
            let feld = |w: &Value| w.as_u64().and_then(|n| usize::try_from(n).ok());
            match (feld(spalten), feld(zeilen)) {
                (Some(s), Some(z)) if groesse_gueltig(s, z) => Ok((s, z)),
                _ => ungueltig("groesse"),
            }
        }
        _ => ungueltig("groesse"),
    }
}

/// The scrollback setting of the case: a whole number, else the default of 2000 (TR4).
fn ruecklauf_lesen(fall: &Value) -> usize {
    fall.get("ruecklauf")
        .and_then(Value::as_u64)
        .and_then(|n| usize::try_from(n).ok())
        .unwrap_or(RUECKLAUF_VORGABE)
}

fn schritt_lesen(wert: &Value, kindteil: bool) -> Result<Schritt, Ungueltig> {
    let art = text(wert, "art")?;
    match (art, kindteil) {
        ("Text", false) => Ok(Schritt::Text(text(wert, "text")?.to_owned())),
        ("Bytes", false) => Ok(Schritt::Bytes(hex_feld(wert)?)),
        ("Tasten", true) => {
            // `mal` (optional, default 1): the bytes are written that many times in ONE write (TR15); the whole write is at
            // most `EINGABE_MAX` bytes.
            let bytes = hex_feld(wert)?;
            let mal = match wert.get("mal") {
                None => 1,
                Some(_) => zahl(wert, "mal", 1, EINGABE_MAX as u64)?,
            };
            if bytes.len().saturating_mul(mal) > EINGABE_MAX {
                return ungueltig("mal");
            }
            Ok(Schritt::Tasten(bytes.repeat(mal)))
        }
        ("Groesse", _) => {
            // `versuch` (optional, default false): a size outside the limits is refused instead of making the case invalid.
            let versuch = match wert.get("versuch") {
                None => false,
                Some(Value::Bool(b)) => *b,
                Some(_) => return ungueltig("versuch"),
            };
            let (spalten, zeilen) = if versuch {
                (
                    zahl(wert, "spalten", 0, VERSUCH_MAX)?,
                    zahl(wert, "zeilen", 0, VERSUCH_MAX)?,
                )
            } else {
                // the limits of `groesse_gueltig`: 2 to 1000 columns, 1 to 1000 rows
                (
                    zahl(wert, "spalten", 2, 1000)?,
                    zahl(wert, "zeilen", 1, 1000)?,
                )
            };
            Ok(Schritt::Groesse {
                spalten,
                zeilen,
                versuch,
            })
        }
        ("Zustand", _) => Ok(Schritt::Zustand),
        ("Warte", true) => {
            let Some(bis) = wert.get("bis") else {
                return ungueltig("bis");
            };
            let cursor = match bis.get("cursor") {
                None => None,
                Some(c) => match cursor_lesen(c) {
                    Some(c) => Some(c),
                    None => return ungueltig("bis"),
                },
            };
            let ziel = Ziel {
                text: bis.get("text").and_then(Value::as_str).map(str::to_owned),
                kindende: bis
                    .get("kindende")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
                cursor,
            };
            if ziel.text.is_none() && !ziel.kindende && ziel.cursor.is_none() {
                return ungueltig("bis");
            }
            Ok(Schritt::Warte(ziel))
        }
        _ => ungueltig("art"),
    }
}

fn schritte_lesen(fall: &Value, kindteil: bool) -> Result<Vec<Schritt>, Ungueltig> {
    match fall.get("schritte").and_then(Value::as_array) {
        Some(liste) => liste
            .iter()
            .map(|wert| schritt_lesen(wert, kindteil))
            .collect(),
        None => ungueltig("schritte"),
    }
}

/// The state of the screen as the JSON object of contract section 3. `abschluss` is `stand` (stream part) or the number
/// of `Ausgabe` events after `Kindende` (child part, where `stand` depends on the chunking and is left out).
fn zustand(raster: &Raster, nach_kindende: Option<u64>) -> Value {
    let (zeile, spalte, sichtbar) = raster.cursor();
    let (spalten, zeilen) = raster.groesse_jetzt();
    let (erste, letzte) = match raster.ruecklauf_enden() {
        Some((erste, letzte)) => (json!(erste), json!(letzte)),
        None => (Value::Null, Value::Null),
    };
    let mut aus = json!({
        "zeilen": raster.zeilen(),
        "cursor": [zeile, spalte, sichtbar],
        "groesse": [spalten, zeilen],
        "ruecklauf": {"zahl": raster.ruecklauf_zahl(), "erste": erste, "letzte": letzte},
        "abbild_gleich": raster.abbild_gleich(),
    });
    match nach_kindende {
        Some(zahl) => aus["ausgaben_nach_kindende"] = json!(zahl),
        None => aus["stand"] = json!(raster.stand()),
    }
    aus
}

fn strom(fall: &Value) -> Result<Value, Ungueltig> {
    let (spalten, zeilen) = groesse_lesen(fall)?;
    let schritte = schritte_lesen(fall, false)?;
    let mut raster = Raster::neu(spalten, zeilen, ruecklauf_lesen(fall))
        .map_err(|e| Ungueltig(e.to_string()))?;
    let mut ergebnisse = Vec::new();
    for schritt in &schritte {
        let mut mit_zustand = false;
        // `Some` for a step `Groesse` with `versuch`: whether the size was taken
        let mut angenommen = None;
        let ereignisse = match schritt {
            Schritt::Text(t) => raster.bytes(t.as_bytes()),
            Schritt::Bytes(bytes) => raster.bytes(bytes),
            Schritt::Groesse {
                spalten,
                zeilen,
                versuch,
            } => {
                if let Ok(ereignisse) = raster.groesse(*spalten, *zeilen) {
                    angenommen = versuch.then_some(true);
                    ereignisse
                } else {
                    angenommen = Some(false);
                    Vec::new()
                }
            }
            _ => {
                mit_zustand = true;
                Vec::new()
            }
        };
        let mut eintrag = Map::new();
        if let Some(ok) = angenommen {
            eintrag.insert("ok".to_owned(), json!(ok));
            if !ok {
                eintrag.insert("fehler".to_owned(), json!("groesse_ungueltig"));
            }
        }
        eintrag.insert(
            "ereignisse".to_owned(),
            ereignisse.iter().map(Ereignis::zu_json).collect(),
        );
        eintrag.insert("pty_antworten".to_owned(), json!(hex(&raster.antworten())));
        if mit_zustand {
            eintrag.insert("zustand".to_owned(), zustand(&raster, None));
        }
        ergebnisse.push(Value::Object(eintrag));
    }
    Ok(json!({"ergebnisse": ergebnisse, "ende": zustand(&raster, None)}))
}

/// What the child part has seen so far: the events of the current step and the count behind `Kindende`.
#[derive(Default)]
struct Buch {
    ereignisse: Vec<Ereignis>,
    kindende: bool,
    ausgaben_danach: u64,
}

impl Buch {
    fn aufnehmen(&mut self, neu: Vec<Ereignis>) {
        for ereignis in neu {
            if matches!(ereignis, Ereignis::Kindende { .. }) {
                self.kindende = true;
            } else if self.kindende && ereignis.ist_ausgabe() {
                self.ausgaben_danach += 1;
            }
            self.ereignisse.push(ereignis);
        }
    }
}

/// Waits for the goal of a step `Warte`; returns the reason when it fails (`zeit`: the deadline; `nie`: the child has ended
/// and the text did not show).
fn warten(kind: &mut Kind, ziel: &Ziel, buch: &mut Buch, frist: Duration) -> Option<&'static str> {
    let ende = Instant::now() + frist;
    loop {
        if let Some(gesucht) = &ziel.text
            && kind
                .raster()
                .klartext()
                .iter()
                .any(|zeile| zeile.contains(gesucht.as_str()))
        {
            return None;
        }
        if ziel.cursor.is_some_and(|c| kind.raster().cursor() == c) {
            return None;
        }
        if kind.ist_zu_ende() {
            return if ziel.kindende { None } else { Some("nie") };
        }
        let Some(rest) = ende.checked_duration_since(Instant::now()) else {
            return Some("zeit");
        };
        buch.aufnehmen(kind.lies(rest.min(Duration::from_millis(100))));
    }
}

fn kindteil(fall: &Value, exe: &Path, frist: Duration) -> Result<Value, Ungueltig> {
    let (spalten, zeilen) = groesse_lesen(fall)?;
    let Some(start) = fall.get("start") else {
        return ungueltig("start");
    };
    let name = text(start, "kind")?;
    let mut argv = vec![
        exe.to_string_lossy().into_owned(),
        "--kind".to_owned(),
        name.to_owned(),
    ];
    match start.get("argv") {
        None => {}
        Some(Value::Array(liste)) => {
            for arg in liste {
                match arg.as_str() {
                    Some(a) => argv.push(a.to_owned()),
                    None => return ungueltig("argv"),
                }
            }
        }
        Some(_) => return ungueltig("argv"),
    }
    let Some(Value::Object(umgebung)) = start.get("env") else {
        return ungueltig("env");
    };
    let mut env = Vec::new();
    for (name, wert) in umgebung {
        match wert.as_str() {
            Some(w) => env.push((name.clone(), w.to_owned())),
            None => return ungueltig("env"),
        }
    }
    // `cwd` (optional, default `/`): the working folder of the child; if it is not a folder the start fails (TR15).
    let cwd = match start.get("cwd") {
        None => "/",
        Some(wert) => match wert.as_str() {
            Some(pfad) => pfad,
            None => return ungueltig("cwd"),
        },
    };
    let schritte = schritte_lesen(fall, true)?;
    let mut kind = Kind::starten(Start {
        argv,
        env,
        cwd: PathBuf::from(cwd),
        spalten,
        zeilen,
        ruecklauf: ruecklauf_lesen(fall),
    })
    .map_err(|e| Ungueltig(e.to_string()))?;
    let mut buch = Buch::default();
    let mut ergebnisse = Vec::new();
    for schritt in &schritte {
        let mut fehler = None;
        let mut mit_zustand = false;
        match schritt {
            Schritt::Warte(ziel) => fehler = warten(&mut kind, ziel, &mut buch, frist),
            Schritt::Tasten(bytes) => {
                // TR15: only a full queue is a result; any other refusal (the child is gone) stays unseen, as before
                if let Err(e) = kind.schreibe(bytes)
                    && e.kind() == std::io::ErrorKind::WouldBlock
                {
                    fehler = Some("schlange_voll");
                }
            }
            Schritt::Groesse {
                spalten, zeilen, ..
            } => match kind.groesse(*spalten, *zeilen) {
                Ok(ereignisse) => buch.aufnehmen(ereignisse),
                Err(_) => fehler = Some("groesse_ungueltig"),
            },
            _ => mit_zustand = true,
        }
        // The `Ausgabe` events depend on how the reads were cut and are not compared; every other event keeps its order.
        let ereignisse: Vec<Value> = std::mem::take(&mut buch.ereignisse)
            .iter()
            .filter(|e| !e.ist_ausgabe())
            .map(Ereignis::zu_json)
            .collect();
        let mut eintrag = Map::new();
        eintrag.insert("ok".to_owned(), json!(fehler.is_none()));
        if let Some(grund) = fehler {
            eintrag.insert("fehler".to_owned(), json!(grund));
        }
        eintrag.insert("ereignisse".to_owned(), Value::Array(ereignisse));
        if mit_zustand {
            eintrag.insert(
                "zustand".to_owned(),
                zustand(kind.raster(), Some(buch.ausgaben_danach)),
            );
        }
        ergebnisse.push(Value::Object(eintrag));
    }
    let ende = zustand(kind.raster(), Some(buch.ausgaben_danach));
    Ok(json!({"ergebnisse": ergebnisse, "ende": ende}))
}

/// Runs one case and returns its result, with the deadline of 20 s for each step `Warte`. `exe` is the program `tr` itself,
/// which the child cases start as the fixed child program (`tr --kind <name> ...`).
///
/// # Errors
/// The case is invalid (contract section 2): too long, not JSON, nested too deep, not an object, or a wrong field.
pub fn laufen(fall: &[u8], exe: &Path) -> Result<Value, Ungueltig> {
    laufen_mit_frist(fall, exe, FRIST)
}

/// Like [`laufen`], with another deadline for the steps `Warte` (the tests use a short one).
///
/// # Errors
/// As [`laufen`].
pub fn laufen_mit_frist(fall: &[u8], exe: &Path, frist: Duration) -> Result<Value, Ungueltig> {
    if fall.len() > EINGABE_MAX {
        return ungueltig("Eingabe zu gross");
    }
    let wert: Value = serde_json::from_slice(fall).map_err(|e| Ungueltig(e.to_string()))?;
    match wert.get("art").and_then(Value::as_str) {
        Some("strom") => strom(&wert),
        Some("kind") => kindteil(&wert, exe, frist),
        _ => ungueltig("art"),
    }
}
