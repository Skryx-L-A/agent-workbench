//! `Starten` (DI12): the checks and the files of a new session.
//!
//! SECURITY: [`start_form`] and [`start_pruefen`] are the ONE place that checks the structure of a start (DI12 points 3.1, 3.2 and 4). They call
//! `start::lesen_anfrage` (stages 1 to 13 of contract 1) and `start::pruefen` (stages 14 to 17, against the settings file read by the crate
//! `einstellungen`). A refusal carries `grund` `start_pruefung`, the code of the stage (`start.<code>`) and the fixed text of the stage, unchanged: the
//! service has no list of codes of its own. The service never compares a code with a fixed list.
//!
//! The paths that this module reads and writes come from the start file of the service only: the folder of the sessions is
//! `socket.ordner`, the settings file is `lauf.einstellungen`. No request, no environment variable and no argument names a path.

use std::fs::File;
#[cfg(unix)]
use std::fs::{DirBuilder, OpenOptions};
use std::io::Read;
#[cfg(unix)]
use std::io::Write;
#[cfg(unix)]
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};

#[cfg(unix)]
use crate::sys::BorrowedFd;
use std::path::{Path, PathBuf};

use einstellungen::Einstellungen;
use serde_json::{Map, Value, json};
use start::{Routenart, Startanfrage, Startform};

use crate::startdatei::Lauf;

/// The most bytes of the settings file (stage 1 of contract 2).
const EINST_MAX: u64 = 65_536;
/// The most sessions that run at the same time.
pub(crate) const SITZUNGEN_MAX: usize = 8;
/// How long `Beenden` and the end of the service wait between SIGHUP and SIGKILL.
pub(crate) const KILL_FRIST_MS: u64 = 1000;

/// The refusal of `Starten`: the reason, the code and the fixed text (DI12 point 3). The code is passed on unchanged.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Abgelehnt {
    pub(crate) grund: String,
    pub(crate) code: String,
    pub(crate) text: String,
}

impl Abgelehnt {
    /// A refusal of the service itself: `code` is the same word as `grund`, the text is fixed.
    pub(crate) fn fest(grund: &str) -> Self {
        Self {
            grund: grund.to_owned(),
            code: grund.to_owned(),
            text: text_zu(grund).to_owned(),
        }
    }

    /// A refusal of the check of the start structure: `grund` `start_pruefung`, the code of the stage and its fixed text, as the crate `start` gives them.
    pub(crate) fn pruefung(fehler: &start::Fehler) -> Self {
        Self {
            grund: "start_pruefung".to_owned(),
            code: fehler.code().to_owned(),
            text: fehler.grund().to_owned(),
        }
    }

    /// The payload of the frame `Ablehnung`.
    pub(crate) fn nutzlast(&self) -> Value {
        json!({"grund": self.grund, "code": self.code, "text": self.text})
    }
}

/// The fixed text of a reason of the service.
pub(crate) fn text_zu(grund: &str) -> &'static str {
    match grund {
        "start_form" => "Start hat falsche Form",
        "einst_unlesbar" => "Einstellungsdatei nicht lesbar",
        "zu_viele_sitzungen" => "Zu viele Sitzungen",
        "nummern_erschoepft" => "Keine Sitzungsnummer frei",
        "start_ordner" => "Ordner der Sitzung nicht anlegbar",
        "programm_nicht_startbar" => "Programm nicht startbar",
        "meldung_kanal" => "Meldekanal der Sitzung nicht anlegbar",
        // Nachtrag 26.11.2026 (DI-L1, DI-L3)
        "orchestrator_unbekannt" => "Orchestrator unbekannt",
        "orchestrator_beendet" => "Orchestrator ist beendet",
        "orchestrator_ist_worker" => "Orchestrator ist selbst ein Worker",
        "orchestrator_fremder_kunde" => "Kunde weicht vom Orchestrator ab",
        "fortsetzen_form" => "Fortsetzen hat falsche Form",
        "wiederbeleben_form" => "Wiederbeleben hat falsche Form",
        "sitzung_unbekannt" => "Sitzung unbekannt",
        "sitzung_laeuft" => "Sitzung läuft",
        "schon_fortgesetzt" => "Sitzung ist schon fortgesetzt",
        "nicht_fortsetzbar" => "Sitzung lässt sich nicht fortsetzen",
        "nicht_abgestuerzt" => "Sitzung ist nicht abgestürzt",
        _ => "",
    }
}

/// The canonical form of a JSON value: compact, keys sorted, UTF-8, non-ASCII without a `\u` escape.
pub(crate) fn kanon(wert: &Value) -> Vec<u8> {
    serde_json::to_vec(wert).unwrap_or_default()
}

/// THE place of the check of the structure itself (stages 1 to 13 of contract 1; see the head of the file): the structure travels as a JSON value, the
/// crate reads its canonical bytes.
///
/// # Errors
/// The refusal `start_pruefung` with the code and the fixed text of the first violated stage.
pub(crate) fn start_form(start: &Value) -> Result<Startanfrage, Abgelehnt> {
    start::lesen_anfrage(&kanon(start)).map_err(|f| Abgelehnt::pruefung(&f))
}

/// THE place of the check against the settings file (stages 14 to 17 of contract 1).
///
/// # Errors
/// The refusal `start_pruefung` with the code and the fixed text of the first violated stage.
pub(crate) fn start_pruefen(
    anfrage: &Startanfrage,
    einstellungen: &Einstellungen,
) -> Result<(), Abgelehnt> {
    start::pruefen(anfrage, einstellungen)
        .map(|_| ())
        .map_err(|f| Abgelehnt::pruefung(&f))
}

/// The event `Starten` has the keys `art` and `start`, with a worker also `orchestrator` and `rolle` (DI-L1 point 1; the service sets
/// `t` and `betrachter` before; a client may not).
pub(crate) fn starten_form(ereignis: &Map<String, Value>) -> Result<&Value, Abgelehnt> {
    let nur_erlaubte = ereignis.keys().all(|k| {
        matches!(
            k.as_str(),
            "art" | "start" | "t" | "betrachter" | "orchestrator" | "rolle"
        )
    });
    match ereignis.get("start") {
        Some(s) if nur_erlaubte => Ok(s),
        _ => Err(Abgelehnt::fest("start_form")),
    }
}

/// The harness table of contract 1 (fassung 1 with the addenda of 20 and 27 November): name, kind and allowed kind of route. The crate `start` checks the
/// harness itself; this table only feeds `Startwahl`. The test `harness_tabelle_gleicht_dem_crate` holds it equal to the crate.
pub(crate) const HARNESS: [(&str, &str, Routenart); 4] = [
    ("claude", "cloud", Routenart::Cloud),
    ("copilot", "cloud", Routenart::Cloud),
    ("pi", "lokal", Routenart::Lokal),
    ("opencode", "lokal", Routenart::Lokal),
];

/// Reads the settings file (path from the start file only): a regular file of at most [`EINST_MAX`] bytes that the crate `einstellungen` reads
/// (contract 2). `None` if it is no regular file, too large or refused by the crate.
pub(crate) fn einstellungen_laden(pfad: &str) -> Option<Einstellungen> {
    if !std::fs::metadata(pfad).ok()?.is_file() {
        return None;
    }
    // one byte more than the bound is read, so that a file over the bound is seen as such and never cut to fit
    let mut roh = Vec::new();
    File::open(pfad)
        .ok()?
        .take(EINST_MAX + 1)
        .read_to_end(&mut roh)
        .ok()?;
    if u64::try_from(roh.len()).ok()? > EINST_MAX {
        return None;
    }
    einstellungen::lesen(&roh).ok()
}

/// The pieces of `Startwahl`: `fassung`, `harness`, `routen` (only `schluessel`, `art`, `klasse` and `modell`, the text of the route or `null`: no target,
/// no network target, no key, no path) and `kunden` (the keys, `null` last).
pub(crate) fn startwahl_von(einst: &Einstellungen) -> Value {
    let harness: Vec<Value> = HARNESS
        .iter()
        .map(|(name, art, routenart)| json!({"name": name, "art": art, "routenart": routenart.kennung()}))
        .collect();
    let routen: Vec<Value> = einst
        .routen()
        .iter()
        .map(|r| {
            json!({"schluessel": r.schluessel(), "art": r.art().kennung(), "klasse": r.klasse(), "modell": r.modell()})
        })
        .collect();
    let mut kunden: Vec<Value> = einst
        .kunden()
        .iter()
        .map(|k| json!(k.schluessel()))
        .collect();
    kunden.push(Value::Null);
    json!({"fassung": 1, "harness": harness, "routen": routen, "kunden": kunden})
}

/// The meta data of a session for the list: the four texts of the start structure, `""` for a session without a customer.
/// The working folder of a session of `Starten`: `arbeitsordner` of the start structure (absolute and normalized, crate `start`).
pub(crate) fn arbeitsordner_von(start: &Startanfrage) -> &str {
    start.arbeitsordner()
}

pub(crate) fn meta_von(start: &Startanfrage) -> [String; 4] {
    [
        start.harness().to_owned(),
        start.modell().to_owned(),
        start.route().to_owned(),
        start.kunde().unwrap_or("").to_owned(),
    ]
}

/// The entries of the number `n` (DI12 points 13 and 15): one line for each file that a session of this number makes. An entry counts only when its key is set.
/// This is the one place; a new entry is one line here and one case.
///
/// | entry | key | made by |
/// |---|---|---|
/// | `<sitzungen>/s-<n>` | `lauf.sitzungen` | the session folder of `werkbank-lauf` |
/// | `<laufzeit>/s-<n>` | `lauf.laufzeit` | the socket folder of `werkbank-lauf` |
/// | `<ziellisten>/s-<n>.zielliste.json` | `lauf.ziellisten` | the target list of `werkbank-lauf` |
/// | `<marker_ordner>/s-<n>.suche.json` and `<marker_ordner>/s-<n>.modellweg.json` | `lauf.marker_ordner` | the count files of search and model path |
/// | `<gespraeche>/s-<n>` | `lauf.gespraeche` | the conversation of the harness (Nachtrag 27.11.2026) |
pub(crate) fn eintraege_der_nummer(lauf: &Lauf, n: usize) -> Vec<PathBuf> {
    let mut eintraege = Vec::new();
    let mut dazu = |ordner: &Option<String>, name: String| {
        if let Some(o) = ordner.as_deref() {
            eintraege.push(Path::new(o).join(name));
        }
    };
    dazu(&lauf.sitzungen, format!("s-{n}"));
    dazu(&lauf.laufzeit, format!("s-{n}"));
    dazu(&lauf.ziellisten, zielliste_name(n));
    dazu(&lauf.marker_ordner, format!("s-{n}.suche.json"));
    dazu(&lauf.marker_ordner, format!("s-{n}.modellweg.json"));
    dazu(&lauf.gespraeche, format!("s-{n}"));
    eintraege
}

/// The raster number for the session `s-<n>` (DI12 point 4, P25). `vergeben[x]`: the number `x` was handed out in this run; `frei`:
/// the numbers of ended sessions that no viewer subscribes, with the place of their end. First a number never handed out: `n` itself
/// when it is at most `max` (so the first sessions keep the number of their identifier), else the smallest one; then the free number
/// whose session ended first. `None`: every number from 1 to `max` is held. 0 belongs to the session of `kind`.
pub(crate) fn rahmennummer(
    n: usize,
    max: usize,
    vergeben: &[bool; 256],
    frei: &[(u64, u8)],
) -> Option<u8> {
    let nie = |x: usize| u8::try_from(x).ok().filter(|_| !vergeben[x]);
    if (1..=max).contains(&n)
        && let Some(x) = nie(n)
    {
        return Some(x);
    }
    if let Some(x) = (1..=max).find_map(nie) {
        return Some(x);
    }
    frei.iter()
        .filter(|(_, x)| (1..=max).contains(&usize::from(*x)))
        .min()
        .map(|(_, x)| *x)
}

/// Is the number `n` taken? It is when ANY entry of [`eintraege_der_nummer`] exists. Only `lstat` looks (it follows no link): an entry of any kind,
/// a dead link too, takes the number; only `NotFound` leaves an entry free, and the number is free only when every entry is free. `Err(())`: another
/// error (no rights, a name too long), the start is refused. The service creates, reads and removes nothing there. Without the keys no number is taken.
pub(crate) fn kennung_belegt(lauf: &Lauf, n: usize) -> Result<bool, ()> {
    for pfad in eintraege_der_nummer(lauf, n) {
        // Windows (section 21, addendum of 25.11.2026): every user may bypass the traverse check, so `lstat` in a folder without
        // rights of the own user does not fail there. The folder must grant the own user the right to look into it (the counterpart
        // of the search bit); a folder that is missing leaves the entry free as under Unix, any other error refuses the start
        // (`sys::eintrag_durchsuchbar`).
        #[cfg(windows)]
        crate::sys::eintrag_durchsuchbar(&pfad)?;
        match std::fs::symlink_metadata(&pfad) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Ok(_) => return Ok(true),
            Err(_) => return Err(()),
        }
    }
    Ok(false)
}

/// The name of the target list of the number `n` in `lauf.ziellisten`.
pub(crate) fn zielliste_name(n: usize) -> String {
    format!("s-{n}.zielliste.json")
}

/// DI12 point 15a (2026-11-23): removes the target list of the number `n` when it is the only entry that takes the number,
/// and reports whether it did. The caller asks only for a number above every number this run of the service handed out, so the list was
/// left by an earlier run (or a start outside the service) whose session has ended: its session folder and socket folder are gone. The
/// lists of this run stay, the runner reads them after its sessions (`werkbank-modellweg suche --zielliste`).
///
/// SECURITY: removing only through descriptors. The folder `lauf.ziellisten` is opened with `O_DIRECTORY | O_NOFOLLOW` and must belong to
/// the own user; the entry is looked at with `statat(AT_SYMLINK_NOFOLLOW)` relative to that descriptor and removed with `unlinkat` relative
/// to it. Only a regular file of the own user is removed; a link, a folder or a foreign file stays and keeps the number taken. Every
/// other entry of the number must be missing (`NotFound`); in doubt nothing is removed.
#[cfg(unix)]
pub(crate) fn alte_zielliste_raeumen(lauf: &Lauf, n: usize) -> bool {
    use rustix::fs::{AtFlags, CWD, FileType, Mode, fstat, openat, statat, unlinkat};
    let Some(ordner) = lauf.ziellisten.as_deref() else {
        return false;
    };
    let liste = Path::new(ordner).join(zielliste_name(n));
    let andere_frei = eintraege_der_nummer(lauf, n)
        .iter()
        .filter(|p| **p != liste)
        .all(|p| matches!(std::fs::symlink_metadata(p), Err(e) if e.kind() == std::io::ErrorKind::NotFound));
    if !andere_frei {
        return false;
    }
    let eigen = rustix::process::geteuid().as_raw();
    let Ok(fd) = openat(CWD, ordner, crate::loeschen::ORDNER_OFFEN, Mode::empty()) else {
        return false;
    };
    if !fstat(&fd).is_ok_and(|st| st.st_uid == eigen) {
        return false;
    }
    let Ok(name) = std::ffi::CString::new(zielliste_name(n)) else {
        return false;
    };
    let regulaer_eigen = statat(&fd, &name, AtFlags::SYMLINK_NOFOLLOW).is_ok_and(|st| {
        FileType::from_raw_mode(st.st_mode) == FileType::RegularFile && st.st_uid == eigen
    });
    regulaer_eigen && unlinkat(&fd, &name, AtFlags::empty()).is_ok()
}

// Windows: [`crate::sys::alte_zielliste_raeumen`] (the same rule with the owner of the access list, section 21).
#[cfg(windows)]
pub(crate) use crate::sys::alte_zielliste_raeumen;

/// The answer of [`kennung_belegt`] after DI12 point 15a: called only for a number above every number this run of the service handed out,
/// so a target list that alone takes it was left by an earlier run; [`alte_zielliste_raeumen`] removes it and the number is asked again.
/// `Ok(true)`: taken, `Ok(false)`: free, `Err(())`: refuse the start.
pub(crate) fn nummer_vergebbar(lauf: &Lauf, n: usize) -> Result<bool, ()> {
    match kennung_belegt(lauf, n) {
        Ok(true) if alte_zielliste_raeumen(lauf, n) => kennung_belegt(lauf, n),
        andere => andere,
    }
}

/// The folder `<ordner>/<kennung>` (0700) with `start.json` (0600, exclusive) that holds the start structure with `sitzung`
/// set. SECURITY: a folder that exists already is never replaced or deleted (`Err`, nothing removed); a folder this call made
/// and could not fill is removed again.
///
/// # Errors
/// The folder exists, cannot be made, or the file cannot be written.
#[cfg(unix)]
pub(crate) fn ordner_anlegen(
    ordner: &Path,
    wurzel: BorrowedFd<'_>,
    kennung: &str,
    start: &Startanfrage,
    marker: Option<&start::Marker>,
    fortsetzung_von: Option<&str>,
) -> Result<(PathBuf, PathBuf), ()> {
    // the file is what `start::schreiben` gives for the structure with the key set (and, for `Fortsetzen` and `Wiederbeleben`, the old
    // key); `Lauf starten` reads it with `start::lesen_datei`
    let inhalt = start::schreiben(
        &start
            .clone()
            .mit_fortsetzung(kennung, marker.cloned(), fortsetzung_von)
            .map_err(|_| ())?,
    );
    let sitzung = ordner.join(kennung);
    DirBuilder::new()
        .mode(0o700)
        .create(&sitzung)
        .map_err(|_| ())?;
    let fertig = || -> std::io::Result<PathBuf> {
        std::fs::set_permissions(&sitzung, std::fs::Permissions::from_mode(0o700))?;
        let datei = sitzung.join("start.json");
        let mut f = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&datei)?;
        f.set_permissions(std::fs::Permissions::from_mode(0o600))?;
        f.write_all(&inhalt)?;
        Ok(datei)
    };
    if let Ok(datei) = fertig() {
        Ok((sitzung, datei))
    } else {
        crate::loeschen::sitzung_entfernen(wurzel, kennung);
        Err(())
    }
}

// Windows: [`crate::sys::ordner_anlegen`] (the same folder and file with access lists, section 21).
#[cfg(windows)]
pub(crate) use crate::sys::ordner_anlegen;

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    const EINST_1: &str = include_str!("../tests/werkzeug/einstellungen/vorfuehrung.json");
    /// Settings of fassung 2: a cloud route without a model, a cloud route with a model, a local route with target and model.
    const EINST_2: &str = r#"{"fassung":2,"klassen":["oeffentlich","intern","kundendaten"],"vorgabestufe":"kundendaten",
        "routen":[
          {"schluessel":"cloud-beispiel","art":"cloud","klasse":"intern","netzziele":[{"host":"api.cloud.example","port":443,"zugriff":"kopf-einsetzen","schluessel":true,"pfade":[{"methode":"GET","pfad":"/copilot_internal/*"}]}]},
          {"schluessel":"cloud-fest","art":"cloud","klasse":"intern","modell":"gpt-5.6-sol","netzziele":[{"host":"api.cloud.example","port":443,"zugriff":"kopf-einsetzen","schluessel":true,"pfade":[{"methode":"GET","pfad":"/copilot_internal/*"}]}]},
          {"schluessel":"lokal-beispiel","art":"lokal","klasse":"kundendaten","ziel":"http://127.0.0.1:11434/v1","modell":"qwen3:8b","netzziele":[]}],
        "kunden":[{"schluessel":"k-beispiel","erlaubt":["lokal-beispiel"],"verboten":[]}],
        "kundendaten_an_cloud":{"vorgabe":"nie","freigaben":[]}}"#;

    fn gut() -> Value {
        json!({"fassung": 1, "harness": "claude", "modell": "claude-sonnet-5-5", "route": "cloud-beispiel", "kunde": "k-beispiel", "arbeitsordner": "/tmp/p"})
    }

    #[allow(clippy::needless_pass_by_value)] // the call sites pass a `json!` literal
    fn mit(wert: &Value, aenderung: Value) -> Value {
        let mut neu = wert.clone();
        if let (Some(a), Some(b)) = (neu.as_object_mut(), aenderung.as_object()) {
            for (k, v) in b {
                if v == &json!("<weg>") {
                    a.remove(k);
                } else {
                    a.insert(k.clone(), v.clone());
                }
            }
        }
        neu
    }

    fn einst(text: &str) -> Einstellungen {
        einstellungen::lesen(text.as_bytes()).expect("the settings are valid")
    }

    fn pruef(wert: &Value, e: &Einstellungen) -> Result<(), Abgelehnt> {
        start_pruefen(&start_form(wert)?, e)
    }

    fn tripel(a: &Abgelehnt) -> (&str, &str, &str) {
        (&a.grund, &a.code, &a.text)
    }

    #[test]
    fn a_valid_start_passes_both_checks() {
        assert!(pruef(&gut(), &einst(EINST_1)).is_ok());
        let z = mit(
            &gut(),
            json!({"harness": "copilot", "modell": "auto", "route": "cloud-beispiel", "kunde": null}),
        );
        assert!(pruef(&z, &einst(EINST_2)).is_ok());
    }

    #[test]
    #[allow(clippy::too_many_lines)] // one table row per stage of contract 1
    fn every_stage_gives_its_code_and_fixed_text() {
        let e1 = einst(EINST_1);
        let e2 = einst(EINST_2);
        let mut tabelle: Vec<(Value, &Einstellungen, &str, &str)> = vec![
            (
                json!("text"),
                &e1,
                "start.json_ungueltig",
                "kein gueltiges JSON",
            ),
            (
                mit(&gut(), json!({"sitzung": "s-1"})),
                &e1,
                "start.sitzung_nicht_erlaubt",
                "Sitzung vergibt der Dienst",
            ),
            (
                mit(&gut(), json!({"marker": 1})),
                &e1,
                "start.marker_nicht_erlaubt",
                "Marker setzt der Dienst",
            ),
            (
                mit(&gut(), json!({"x": 1})),
                &e1,
                "start.schluessel_unbekannt",
                "unbekannter Schluessel",
            ),
            (
                mit(&gut(), json!({"kunde": "<weg>"})),
                &e1,
                "start.schluessel_fehlt",
                "Pflichtfeld fehlt",
            ),
            (
                mit(&gut(), json!({"fassung": 1.0})),
                &e1,
                "start.typ_falsch",
                "falscher Typ",
            ),
            (
                mit(&gut(), json!({"fassung": 3})),
                &e1,
                "start.fassung_unbekannt",
                "Fassung unbekannt",
            ),
            (
                mit(&gut(), json!({"route": "A"})),
                &e1,
                "start.kennung_form",
                "Kennung hat falsche Form",
            ),
            (
                mit(&gut(), json!({"modell": "a b"})),
                &e1,
                "start.modell_form",
                "Modell hat falsche Form",
            ),
            (
                mit(&gut(), json!({"arbeitsordner": "rel"})),
                &e1,
                "start.ordner_ungueltig",
                "Arbeitsordner ungueltig",
            ),
            (
                mit(&gut(), json!({"harness": "gemini"})),
                &e1,
                "start.harness_unbekannt",
                "Harness unbekannt",
            ),
            (
                mit(&gut(), json!({"route": "gibt-es-nicht"})),
                &e1,
                "start.route_unbekannt",
                "Route unbekannt",
            ),
            (
                mit(&gut(), json!({"route": "cloud-fest", "modell": "o3"})),
                &e2,
                "start.modell_route",
                "Modell passt nicht zur Route",
            ),
            (
                mit(&gut(), json!({"kunde": "k-fremd"})),
                &e1,
                "start.kunde_unbekannt",
                "Kunde unbekannt",
            ),
            (
                mit(&gut(), json!({"harness": "pi"})),
                &e1,
                "start.harness_route_unvertraeglich",
                "Harness passt nicht zur Route",
            ),
        ];
        tabelle.push((
            json!({"fassung": 1, "x": "a".repeat(4097)}),
            &e1,
            "start.zu_gross",
            "Start zu gross",
        ));
        for (wert, e, code, text) in tabelle {
            let a = pruef(&wert, e).expect_err(code);
            assert_eq!(tripel(&a), ("start_pruefung", code, text), "{wert}");
            assert_eq!(
                a.nutzlast(),
                json!({"grund": "start_pruefung", "code": code, "text": text})
            );
        }
    }

    #[test]
    fn der_arbeitsordner_kommt_aus_der_start_struktur() {
        let a = start_form(&gut()).expect("form");
        assert_eq!(arbeitsordner_von(&a), "/tmp/p");
    }

    #[test]
    fn the_lowest_stage_wins_and_the_form_comes_before_the_settings() {
        let e1 = einst(EINST_1);
        let code = |w: &Value| pruef(w, &e1).expect_err("refused").code;
        assert_eq!(
            code(&mit(&gut(), json!({"sitzung": "s-1", "marker": 1, "x": 1}))),
            "start.sitzung_nicht_erlaubt"
        );
        assert_eq!(
            code(&mit(
                &gut(),
                json!({"route": "gibt-es-nicht", "kunde": "k-fremd"})
            )),
            "start.route_unbekannt"
        );
        assert_eq!(
            code(&mit(&gut(), json!({"kunde": "k-fremd", "harness": "pi"}))),
            "start.kunde_unbekannt"
        );
        // the form check does not need the settings file at all
        assert_eq!(
            start_form(&mit(&gut(), json!({"harness": "gemini"})))
                .expect_err("form")
                .code,
            "start.harness_unbekannt"
        );
        assert!(start_form(&mit(&gut(), json!({"route": "gibt-es-nicht"}))).is_ok());
    }

    #[test]
    fn four_thousand_ninety_six_bytes_pass_stage_one_and_four_thousand_ninety_seven_do_not() {
        let mit_n = |n: usize| {
            let leer = kanon(&json!({"fassung": 1, "x": ""})).len();
            json!({"fassung": 1, "x": "a".repeat(n - leer)})
        };
        assert_eq!(kanon(&mit_n(4096)).len(), 4096);
        assert_eq!(
            start_form(&mit_n(4096)).expect_err("form").code,
            "start.schluessel_unbekannt"
        );
        assert_eq!(
            start_form(&mit_n(4097)).expect_err("form").code,
            "start.zu_gross"
        );
    }

    #[test]
    fn harness_table_equals_the_crate() {
        // the crate has no table to read: every row is checked against the crate with a cloud and a local route
        let e = einst(EINST_1);
        for (name, art, routenart) in HARNESS {
            assert_eq!(routenart.kennung(), art);
            for route in ["cloud-beispiel", "lokal-beispiel"] {
                let w = mit(&gut(), json!({"harness": name, "route": route}));
                let erg = pruef(&w, &e);
                let passt = e.route(route).expect("route").art() == routenart;
                assert_eq!(erg.is_ok(), passt, "{name} on {route}");
                if !passt {
                    assert_eq!(
                        erg.expect_err("refused").code,
                        "start.harness_route_unvertraeglich"
                    );
                }
            }
        }
        let w = mit(&gut(), json!({"harness": "gemini"}));
        assert_eq!(
            start_form(&w).expect_err("form").code,
            "start.harness_unbekannt"
        );
    }

    #[test]
    fn startwahl_has_four_texts_per_route_and_no_target() {
        let w = startwahl_von(&einst(EINST_2));
        assert_eq!(
            w["routen"],
            json!([
                {"schluessel": "cloud-beispiel", "art": "cloud", "klasse": "intern", "modell": null},
                {"schluessel": "cloud-fest", "art": "cloud", "klasse": "intern", "modell": "gpt-5.6-sol"},
                {"schluessel": "lokal-beispiel", "art": "lokal", "klasse": "kundendaten", "modell": "qwen3:8b"}
            ])
        );
        assert_eq!(w["kunden"], json!(["k-beispiel", null]));
        assert_eq!(w["fassung"], json!(1));
        assert_eq!(
            w["harness"],
            json!([
                {"name": "claude", "art": "cloud", "routenart": "cloud"},
                {"name": "copilot", "art": "cloud", "routenart": "cloud"},
                {"name": "pi", "art": "lokal", "routenart": "lokal"},
                {"name": "opencode", "art": "lokal", "routenart": "lokal"}
            ])
        );
        let text = w.to_string();
        for geheim in [
            "127.0.0.1",
            "api.cloud.example",
            "ziel",
            "netzziele",
            "erlaubt",
        ] {
            assert!(!text.contains(geheim), "{geheim}");
        }
        let v1 = startwahl_von(&einst(EINST_1));
        assert!(
            v1["routen"]
                .as_array()
                .expect("list")
                .iter()
                .all(|r| r["modell"].is_null())
        );
    }

    #[test]
    fn meta_is_the_four_texts_and_empty_without_a_customer() {
        let a = start_form(&mit(&gut(), json!({"kunde": null}))).expect("valid");
        assert_eq!(
            meta_von(&a),
            ["claude", "claude-sonnet-5-5", "cloud-beispiel", ""]
        );
        let b = start_form(&gut()).expect("valid");
        assert_eq!(meta_von(&b)[3], "k-beispiel");
    }

    #[test]
    fn abgelehnt_traegt_grund_code_text() {
        let a = Abgelehnt::fest("zu_viele_sitzungen").nutzlast();
        assert_eq!(
            a,
            json!({"grund": "zu_viele_sitzungen", "code": "zu_viele_sitzungen", "text": "Zu viele Sitzungen"})
        );
        for (g, t) in [
            ("start_form", "Start hat falsche Form"),
            ("einst_unlesbar", "Einstellungsdatei nicht lesbar"),
            ("nummern_erschoepft", "Keine Sitzungsnummer frei"),
            ("start_ordner", "Ordner der Sitzung nicht anlegbar"),
            ("programm_nicht_startbar", "Programm nicht startbar"),
            ("meldung_kanal", "Meldekanal der Sitzung nicht anlegbar"),
        ] {
            assert_eq!(text_zu(g), t);
        }
        assert_eq!(text_zu("start.irgendein_code"), "");
    }

    #[test]
    fn starten_nur_art_und_start() {
        let ok = json!({"art": "Starten", "start": {"fassung": 1}, "t": 1, "betrachter": "b1"});
        assert!(starten_form(ok.as_object().expect("o")).is_ok());
        let fremd = json!({"art": "Starten", "start": {}, "x": 1});
        assert!(starten_form(fremd.as_object().expect("o")).is_err());
        let ohne = json!({"art": "Starten"});
        assert!(starten_form(ohne.as_object().expect("o")).is_err());
    }

    fn temp(name: &str) -> PathBuf {
        let p = PathBuf::from(format!("/tmp/dienst-st-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).expect("dir");
        p
    }

    /// The folder opened as the service holds it.
    fn offen(p: &Path) -> std::os::fd::OwnedFd {
        use rustix::fs::{CWD, Mode, openat};
        openat(CWD, p, crate::loeschen::ORDNER_OFFEN, Mode::empty()).expect("open")
    }

    #[test]
    fn ordner_und_datei_mit_rechten_und_inhalt() {
        let o = temp("ordner");
        let start = start_form(&gut()).expect("valid");
        let (dir, datei) = ordner_anlegen(
            &o,
            std::os::fd::AsFd::as_fd(&offen(&o)),
            "s-3",
            &start,
            None,
            None,
        )
        .expect("created");
        assert_eq!(dir, o.join("s-3"));
        let mode = |p: &Path| std::fs::metadata(p).expect("meta").permissions().mode() & 0o7777;
        assert_eq!(mode(&dir), 0o700);
        assert_eq!(mode(&datei), 0o600);
        // what the file holds is a start file that `Lauf starten` reads again: the structure with the key
        let gelesen =
            start::lesen_datei(&std::fs::read(&datei).expect("read")).expect("start file");
        assert_eq!(gelesen.sitzung(), "s-3");
        assert_eq!(gelesen.harness(), "claude");
        assert_eq!(gelesen.arbeitsordner(), "/tmp/p");
        assert_eq!(gelesen.kunde(), Some("k-beispiel"));
        // an existing folder is neither replaced nor removed
        std::fs::write(dir.join("fremd"), b"x").expect("write");
        assert!(
            ordner_anlegen(
                &o,
                std::os::fd::AsFd::as_fd(&offen(&o)),
                "s-3",
                &start,
                None,
                None
            )
            .is_err()
        );
        assert!(dir.join("fremd").exists(), "nothing was deleted");
        let _ = std::fs::remove_dir_all(&o);
    }

    #[test]
    fn der_marker_der_startdatei_steht_in_der_start_json_und_sonst_keiner() {
        let o = temp("marker");
        let start = start_form(&gut()).expect("valid");
        let wurzel = offen(&o);
        let marker =
            start::Marker::neu(vec!["A-1".to_owned(), "B2".to_owned()], "/var/lib/zaehlung")
                .expect("marker");
        let (_, mit) = ordner_anlegen(
            &o,
            std::os::fd::AsFd::as_fd(&wurzel),
            "s-1",
            &start,
            Some(&marker),
            None,
        )
        .expect("created");
        let gelesen = start::lesen_datei(&std::fs::read(&mit).expect("read")).expect("start file");
        assert_eq!(gelesen.marker(), Some(&marker));
        let (_, ohne) = ordner_anlegen(
            &o,
            std::os::fd::AsFd::as_fd(&wurzel),
            "s-2",
            &start,
            None,
            None,
        )
        .expect("created");
        let gelesen = start::lesen_datei(&std::fs::read(&ohne).expect("read")).expect("start file");
        assert_eq!(gelesen.marker(), None);
        let _ = std::fs::remove_dir_all(&o);
    }

    /// The fixed texts of the reasons of the addendum of 26.11.2026 (DI-L1, DI-L3), as the contract names them.
    #[test]
    fn die_texte_der_neuen_gruende() {
        for (grund, text) in [
            ("orchestrator_unbekannt", "Orchestrator unbekannt"),
            ("orchestrator_beendet", "Orchestrator ist beendet"),
            (
                "orchestrator_ist_worker",
                "Orchestrator ist selbst ein Worker",
            ),
            (
                "orchestrator_fremder_kunde",
                "Kunde weicht vom Orchestrator ab",
            ),
            ("fortsetzen_form", "Fortsetzen hat falsche Form"),
            ("wiederbeleben_form", "Wiederbeleben hat falsche Form"),
            ("sitzung_unbekannt", "Sitzung unbekannt"),
            ("sitzung_laeuft", "Sitzung läuft"),
            ("schon_fortgesetzt", "Sitzung ist schon fortgesetzt"),
            ("nicht_fortsetzbar", "Sitzung lässt sich nicht fortsetzen"),
            ("nicht_abgestuerzt", "Sitzung ist nicht abgestürzt"),
        ] {
            assert_eq!(text_zu(grund), text, "{grund}");
            assert_eq!(
                Abgelehnt::fest(grund).nutzlast(),
                json!({"grund": grund, "code": grund, "text": text})
            );
        }
    }

    /// A `Lauf` with the folders of the keys `sitzungen`, `ziellisten`, `laufzeit` and `marker_ordner` (each optional).
    fn lauf_mit(
        sitzungen: Option<&Path>,
        ziellisten: Option<&Path>,
        laufzeit: Option<&Path>,
        marker_ordner: Option<&Path>,
    ) -> Lauf {
        let text = |p: Option<&Path>| p.map(|p| p.to_string_lossy().into_owned());
        Lauf {
            programm: "/p".to_owned(),
            env: Vec::new(),
            groesse: (80, 24),
            einstellungen: "/e".to_owned(),
            sitzungen: text(sitzungen),
            ziellisten: text(ziellisten),
            laufzeit: text(laufzeit),
            marker_datei: None,
            marker_ordner: text(marker_ordner),
            nummer_max: 255,
            rahmen_max: 255,
            beendet_max: 256,
            stelle: None,
            bestand: None,
            gespraeche: None,
        }
    }

    /// DI12 point 15 (Nachtrag 27.11.2026): an entry `<gespraeche>/s-<n>` of any kind takes the number, also a dead link; only a
    /// missing entry or a missing folder leaves it free, and without the key nothing there counts.
    #[test]
    fn ein_gespraech_belegt_seine_nummer() {
        use std::os::unix::fs::symlink;
        let o = temp("gespraech");
        let g = o.join("g");
        let mut lauf = lauf_mit(None, None, None, None);
        lauf.gespraeche = Some(g.to_string_lossy().into_owned());
        assert_eq!(kennung_belegt(&lauf, 1), Ok(false), "no folder");
        std::fs::create_dir_all(g.join("s-1")).expect("dir");
        symlink("/nirgends-xyz", g.join("s-2")).expect("dead link");
        std::fs::write(g.join("s-3.zielliste.json"), b"x").expect("file");
        assert_eq!(kennung_belegt(&lauf, 1), Ok(true));
        assert_eq!(kennung_belegt(&lauf, 2), Ok(true));
        assert_eq!(kennung_belegt(&lauf, 3), Ok(false), "another name");
        assert_eq!(
            kennung_belegt(&lauf_mit(None, None, None, None), 1),
            Ok(false),
            "without the key"
        );
        let _ = std::fs::remove_dir_all(&o);
    }

    /// DI12 point 4 (P25): first the number of the identifier while it was never handed out, then the smallest never handed out,
    /// then the free number whose session ended first; with every number held, none.
    #[test]
    fn rahmennummer_nie_vergeben_dann_am_laengsten_beendet() {
        let mut v = [false; 256];
        assert_eq!(rahmennummer(1, 255, &v, &[]), Some(1));
        assert_eq!(rahmennummer(255, 255, &v, &[]), Some(255));
        // an identifier beyond the raster numbers takes the smallest number never handed out
        v[1] = true;
        assert_eq!(rahmennummer(300, 255, &v, &[]), Some(2));
        // a skipped identifier leaves its number unused, and a later identifier takes its own
        assert_eq!(rahmennummer(5, 255, &v, &[]), Some(5));
        // every number handed out: the free one of the session that ended first (place 3 before 7 and 9), not the latest
        let alle = [true; 256];
        assert_eq!(
            rahmennummer(256, 255, &alle, &[(7, 4), (3, 9), (9, 2)]),
            Some(9)
        );
        assert_eq!(rahmennummer(256, 255, &alle, &[]), None);
        // the bound of the test mode: numbers above it are never handed out
        assert_eq!(rahmennummer(3, 2, &v, &[]), Some(2));
        assert_eq!(rahmennummer(3, 2, &alle, &[(1, 200)]), None);
        // the identifier at the bound of the run is no exception
        assert_eq!(
            rahmennummer(crate::startdatei::KENNUNG_MAX, 255, &alle, &[(1, 17)]),
            Some(17)
        );
    }

    /// Four own folders `s`, `z`, `l`, `m` below one test folder, and a `Lauf` that names all four.
    fn vier(name: &str) -> (PathBuf, Lauf) {
        let o = temp(name);
        for d in ["s", "z", "l", "m"] {
            std::fs::create_dir_all(o.join(d)).expect("dir");
        }
        let lauf = lauf_mit(
            Some(&o.join("s")),
            Some(&o.join("z")),
            Some(&o.join("l")),
            Some(&o.join("m")),
        );
        (o, lauf)
    }

    #[test]
    fn jede_datei_der_nummer_belegt_sie() {
        use std::os::unix::fs::symlink;
        let (o, lauf) = vier("belegt");
        assert_eq!(kennung_belegt(&lauf, 1), Ok(false));
        assert_eq!(
            kennung_belegt(&lauf_mit(None, None, None, None), 1),
            Ok(false)
        );
        // each of the five entries alone takes its number (and no other number), in every kind: file, folder, link, dead link
        let eintraege: [(&str, &str); 5] = [
            ("s", "s-1"),
            ("l", "s-2"),
            ("z", "s-3.zielliste.json"),
            ("m", "s-4.suche.json"),
            ("m", "s-5.modellweg.json"),
        ];
        for (k, (d, name)) in eintraege.iter().enumerate() {
            let n = k + 1;
            assert_eq!(kennung_belegt(&lauf, n), Ok(false), "{n} before");
            std::fs::write(o.join(d).join(name), b"x").expect("file");
            assert_eq!(kennung_belegt(&lauf, n), Ok(true), "{n} file");
            for andere in 1..=5 {
                if andere != n {
                    assert_eq!(
                        kennung_belegt(&lauf, andere),
                        Ok(andere < n),
                        "{n} / {andere}"
                    );
                }
            }
        }
        // the kinds of entry, each for a fresh number 6 to 9 in the folder `l` (the socket folder)
        std::fs::create_dir(o.join("l/s-6")).expect("dir");
        symlink(&o, o.join("l/s-7")).expect("link");
        symlink("/nirgends-xyz", o.join("l/s-8")).expect("dead link");
        for n in 6..=8 {
            assert_eq!(kennung_belegt(&lauf, n), Ok(true), "{n}");
        }
        // two folders at once and a dead link in the folder of the count files
        std::fs::create_dir(o.join("s/s-9")).expect("dir");
        std::fs::create_dir(o.join("l/s-9")).expect("dir");
        symlink("/nirgends-xyz", o.join("m/s-10.modellweg.json")).expect("dead link");
        symlink("/nirgends-xyz", o.join("m/s-11.suche.json")).expect("dead link");
        for n in 9..=11 {
            assert_eq!(kennung_belegt(&lauf, n), Ok(true), "{n}");
        }
        assert_eq!(kennung_belegt(&lauf, 12), Ok(false));
        let _ = std::fs::remove_dir_all(&o);
    }

    #[test]
    fn die_namen_sind_genau_und_ein_eintrag_zaehlt_nur_mit_seinem_schluessel() {
        let (o, lauf) = vier("namen");
        // wrong names take nothing
        for (d, name) in [
            ("z", "s-1"),
            ("z", "s-1.json"),
            ("z", "s-1.zielliste.json.alt"),
            ("z", "s-2.suche.json"),
            ("m", "s-1"),
            ("m", "s-1.zielliste.json"),
            ("m", "s-1.suche"),
            ("l", "s-1.zielliste.json"),
            ("s", "s-1.suche.json"),
        ] {
            std::fs::write(o.join(d).join(name), b"x").expect("file");
        }
        assert_eq!(kennung_belegt(&lauf, 1), Ok(false));
        assert_eq!(kennung_belegt(&lauf, 2), Ok(false));
        // each entry counts only with its own key
        std::fs::write(o.join("z/s-3.zielliste.json"), b"x").expect("file");
        std::fs::write(o.join("m/s-4.suche.json"), b"x").expect("file");
        std::fs::write(o.join("m/s-5.modellweg.json"), b"x").expect("file");
        std::fs::write(o.join("l/s-6"), b"x").expect("file");
        std::fs::write(o.join("s/s-7"), b"x").expect("file");
        let (sitz, ziel, lz, mark) = (o.join("s"), o.join("z"), o.join("l"), o.join("m"));
        let ohne = [
            (lauf_mit(Some(&sitz), None, Some(&lz), Some(&mark)), 3),
            (lauf_mit(Some(&sitz), Some(&ziel), Some(&lz), None), 4),
            (lauf_mit(Some(&sitz), Some(&ziel), Some(&lz), None), 5),
            (lauf_mit(Some(&sitz), Some(&ziel), None, Some(&mark)), 6),
            (lauf_mit(None, Some(&ziel), Some(&lz), Some(&mark)), 7),
        ];
        for (ohne_schluessel, nummer) in ohne {
            assert_eq!(
                kennung_belegt(&ohne_schluessel, nummer),
                Ok(false),
                "{nummer}"
            );
        }
        // with the key the same entries take their numbers
        for n in 3..=7 {
            assert_eq!(kennung_belegt(&lauf, n), Ok(true), "{n}");
        }
        // a missing folder is free
        let fehlt = o.join("gibt-es-nicht");
        let allein = lauf_mit(Some(&fehlt), Some(&fehlt), Some(&fehlt), Some(&fehlt));
        assert_eq!(kennung_belegt(&allein, 1), Ok(false));
        let _ = std::fs::remove_dir_all(&o);
    }

    #[test]
    fn ein_anderer_fehler_als_fehlt_ist_ein_fehler() {
        use std::os::unix::fs::PermissionsExt;
        let (o, lauf) = vier("fehler");
        std::fs::write(o.join("m/s-1.suche.json"), b"x").expect("file");
        // every folder without rights gives an error for a number whose earlier entries are free
        for d in ["s", "z", "l", "m"] {
            std::fs::set_permissions(o.join(d), std::fs::Permissions::from_mode(0o000))
                .expect("mode");
            assert_eq!(kennung_belegt(&lauf, 2), Err(()), "{d}");
            std::fs::set_permissions(o.join(d), std::fs::Permissions::from_mode(0o700))
                .expect("mode");
            assert_eq!(kennung_belegt(&lauf, 2), Ok(false), "{d} again");
        }
        // an entry that is there answers before a later folder without rights is looked at
        std::fs::set_permissions(o.join("m"), std::fs::Permissions::from_mode(0o000))
            .expect("mode");
        std::fs::create_dir(o.join("s/s-3")).expect("dir");
        assert_eq!(kennung_belegt(&lauf, 3), Ok(true));
        std::fs::set_permissions(o.join("m"), std::fs::Permissions::from_mode(0o700))
            .expect("mode");
        let _ = std::fs::remove_dir_all(&o);
    }

    #[test]
    fn ordner_fehlt_ist_ein_fehler() {
        let start = start_form(&gut()).expect("valid");
        // descriptor and path belong to the same own test folder; the missing folder lies below it
        let o = temp("fehlt");
        let wurzel = offen(&o);
        assert!(
            ordner_anlegen(
                &o.join("gibt-es-nicht"),
                std::os::fd::AsFd::as_fd(&wurzel),
                "s-1",
                &start,
                None,
                None
            )
            .is_err()
        );
        assert!(std::fs::read_dir(&o).expect("read").next().is_none());
        let _ = std::fs::remove_dir_all(&o);
    }

    #[test]
    fn einstellungen_nur_regulaere_datei_im_rahmen_und_vom_crate_gelesen() {
        let o = temp("einst");
        let f = o.join("e.json");
        let pfad = f.to_str().expect("utf8");
        std::fs::write(&f, EINST_1).expect("write");
        assert!(einstellungen_laden(pfad).is_some());
        std::fs::write(&f, EINST_2).expect("write");
        assert_eq!(einstellungen_laden(pfad).expect("fassung 2").fassung(), 2);
        for roh in [
            "[]",
            "{}",
            r#"{"fassung":1,"routen":5}"#,
            "kein json",
            r#"{"routen":[],"kunden":[]}"#,
        ] {
            std::fs::write(&f, roh).expect("write");
            assert!(einstellungen_laden(pfad).is_none(), "{roh}");
        }
        assert!(
            einstellungen_laden(o.to_str().expect("utf8")).is_none(),
            "a folder"
        );
        assert!(einstellungen_laden("/nirgends/da.json").is_none());
        // the bound is 65 536 bytes: exactly that many is read, one more is not
        let mit_laenge = |n: u64| {
            let mut t = EINST_1.as_bytes().to_vec();
            t.resize(usize::try_from(n).expect("size"), b' ');
            std::fs::write(&f, t).expect("write");
            einstellungen_laden(pfad)
        };
        assert!(mit_laenge(EINST_MAX).is_some(), "exactly the bound");
        assert!(mit_laenge(EINST_MAX + 1).is_none(), "one byte over");
        let _ = std::fs::remove_dir_all(&o);
    }

    /// DI12 point 15a: the own regular list that alone takes the number is removed; nothing else is.
    #[test]
    fn eine_alte_eigene_zielliste_allein_wird_geraeumt() {
        use std::os::unix::fs::symlink;
        let (o, lauf) = vier("raeumen");
        let z = o.join("z");
        // the list alone: removed, the number is free again; nothing else in the folder changes
        std::fs::write(z.join("s-1.zielliste.json"), b"{}").expect("file");
        std::fs::write(z.join("s-2.zielliste.json"), b"{}").expect("file");
        assert!(alte_zielliste_raeumen(&lauf, 1));
        assert_eq!(kennung_belegt(&lauf, 1), Ok(false));
        assert!(
            z.join("s-2.zielliste.json").exists(),
            "only the list of the number asked for"
        );
        // nothing there: nothing to do
        assert!(!alte_zielliste_raeumen(&lauf, 1));
        // a link, a dead link and a folder stay and keep the number taken; the target of the link stays
        let ziel = o.join("ziel.json");
        std::fs::write(&ziel, b"ziel").expect("file");
        symlink(&ziel, z.join("s-3.zielliste.json")).expect("link");
        symlink(o.join("fehlt"), z.join("s-4.zielliste.json")).expect("dead link");
        std::fs::create_dir(z.join("s-5.zielliste.json")).expect("dir");
        for n in 3..=5 {
            assert!(!alte_zielliste_raeumen(&lauf, n), "{n}");
            assert_eq!(kennung_belegt(&lauf, n), Ok(true), "{n}");
        }
        assert_eq!(std::fs::read(&ziel).expect("target"), b"ziel");
        // another entry of the number lies: the list stays (a session of that number may still run or its files are unread)
        for (d, name) in [
            ("s", "s-6"),
            ("l", "s-6"),
            ("m", "s-6.suche.json"),
            ("m", "s-6.modellweg.json"),
        ] {
            std::fs::write(z.join("s-6.zielliste.json"), b"{}").expect("file");
            std::fs::write(o.join(d).join(name), b"x").expect("entry");
            assert!(!alte_zielliste_raeumen(&lauf, 6), "{d}/{name}");
            assert!(z.join("s-6.zielliste.json").exists(), "{d}/{name}");
            std::fs::remove_file(o.join(d).join(name)).expect("rm");
        }
        assert!(alte_zielliste_raeumen(&lauf, 6), "alone again");
        // without the key nothing is removed
        std::fs::write(z.join("s-7.zielliste.json"), b"{}").expect("file");
        let ohne = lauf_mit(
            Some(&o.join("s")),
            None,
            Some(&o.join("l")),
            Some(&o.join("m")),
        );
        assert!(!alte_zielliste_raeumen(&ohne, 7));
        assert!(z.join("s-7.zielliste.json").exists());
        // the key names a link to the folder: O_NOFOLLOW refuses it, nothing is removed below it
        let verweis = o.join("z-verweis");
        symlink(&z, &verweis).expect("link");
        let ueber_verweis = lauf_mit(
            Some(&o.join("s")),
            Some(&verweis),
            Some(&o.join("l")),
            Some(&o.join("m")),
        );
        assert!(!alte_zielliste_raeumen(&ueber_verweis, 7));
        assert!(z.join("s-7.zielliste.json").exists());
        // the number has no upper bound here: the identifier counts, not the frame number
        std::fs::write(z.join("s-300.zielliste.json"), b"{}").expect("file");
        assert!(alte_zielliste_raeumen(&lauf, 300));
        let _ = std::fs::remove_dir_all(&o);
    }
}
