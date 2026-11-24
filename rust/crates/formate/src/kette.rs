//! The audit line `ausgabe.gesperrt` (contract 5, `rust/spec/VERTRAEGE.md`): building a line,
//! appending it to a chain file under an exclusive lock, and checking a chain. Over months: the writer of a
//! source (`anhaengen_gesperrt_in`, `anhaengen_zeile`, `anhaengen_zeilen`) switches the month under one lock and
//! writes the anchor of the closed month; `pruefe_quelle` checks the folder of a source against its anchors.
//!
//! Every error is a value with a fixed code ([`Fehler::code`]); the cases check the code, never a sentence.
//! Nothing here reads the clock: the caller gives the time in milliseconds since 1970.

mod befehl;
mod datum;
mod hash;
mod kanonisch;
#[cfg(unix)]
mod loeschen;
mod monate;
mod pruefen;
mod quelle;
mod schreiber;

use std::fmt;

use serde_json::json;

pub use befehl::{Ausgang, befehl};
pub use datum::zeit_text;
pub use hash::sha256_hex;
pub use kanonisch::kanonisch;
pub use monate::{
    anhaengen_abgewiesen, anhaengen_gesperrt_in, anhaengen_ordnermarke, anhaengen_probelauf,
    anhaengen_zeile, anhaengen_zeilen,
};
pub use pruefen::{Bruch, Pruefung, ZEILE_MAX, pruefe};
pub use quelle::{Gefunden, letzte_zeile_der_art, pruefe_quelle};
pub use schreiber::anhaengen_gesperrt;

/// The `vorher` of the first line of a chain: 64 zeros.
pub const NULL_HASH: &str = "0000000000000000000000000000000000000000000000000000000000000000";

/// An error with a fixed code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Fehler {
    /// A number is not a finite double (`nicht_endlich`).
    NichtEndlich,
    /// The time lies outside of 1970-01-01 up to 9999-12-31 (`zeit_ungueltig`).
    ZeitUngueltig,
    /// A field has a wrong value; the name of the field follows (`feld_ungueltig:<feld>`).
    FeldUngueltig(&'static str),
    /// The file is not empty and does not end with a line feed (`letzte_zeile_unvollstaendig`).
    LetzteZeileUnvollstaendig,
    /// Opening, locking, reading or writing the file failed (`ein_ausgabe`); the text is the system's message.
    EinAusgabe(String),
    /// The month of the time is closed: a later month file or an anchor for it exists (`zeit_rueckwaerts`).
    ZeitRueckwaerts,
    /// The file of the new month exists although the month before is not closed (`abschnitt_vorhanden`).
    AbschnittVorhanden,
    /// The anchor file is unreadable, or an anchor does not fit its month file (`anker_falsch`).
    AnkerFalsch,
    /// The last line under the lock is not the one the caller expected (`vorher_abweichend`).
    VorherAbweichend,
    /// A finished line fails a check of section 3; the reason follows (`zeile_ungueltig:<grund>`).
    ZeileUngueltig(String),
    /// The check of a folder found a break; the place is in the value (`kette_bruch:<grund>`).
    KetteBruch(Bruch),
}

impl Fehler {
    /// The fixed code of the error, as the contract lists it.
    #[must_use]
    pub fn code(&self) -> String {
        match self {
            Self::NichtEndlich => "nicht_endlich".to_owned(),
            Self::ZeitUngueltig => "zeit_ungueltig".to_owned(),
            Self::FeldUngueltig(feld) => format!("feld_ungueltig:{feld}"),
            Self::LetzteZeileUnvollstaendig => "letzte_zeile_unvollstaendig".to_owned(),
            Self::EinAusgabe(_) => "ein_ausgabe".to_owned(),
            Self::ZeitRueckwaerts => "zeit_rueckwaerts".to_owned(),
            Self::AbschnittVorhanden => "abschnitt_vorhanden".to_owned(),
            Self::AnkerFalsch => "anker_falsch".to_owned(),
            Self::VorherAbweichend => "vorher_abweichend".to_owned(),
            Self::ZeileUngueltig(grund) => format!("zeile_ungueltig:{grund}"),
            Self::KetteBruch(bruch) => format!("kette_bruch:{}", bruch.grund),
        }
    }
}

impl fmt::Display for Fehler {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EinAusgabe(text) => write!(f, "ein_ausgabe: {text}"),
            andere => f.write_str(&andere.code()),
        }
    }
}

impl std::error::Error for Fehler {}

/// The data of one line `ausgabe.gesperrt`; the fields are checked when a line is built.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Gesperrt {
    /// Time in milliseconds since 1970 (UTC).
    pub zeit_ms: i64,
    /// Key of the route of the session.
    pub route: String,
    /// Identifier of the session.
    pub sitzung: String,
    /// Code with which `darf` blocked.
    pub grund: String,
    /// Effective class of the blocked units; `None` for the reason `einheit-ohne-klasse`.
    pub klasse: Option<String>,
    /// Key of the customer, or `None` when the units belong to no customer.
    pub kunde: Option<String>,
    /// Number of blocked units, 1 up to 4294967295.
    pub anzahl: u32,
}

/// A key or a code: `^[a-z0-9][a-z0-9._-]{0,63}$`.
fn schluessel_ok(text: &str) -> bool {
    match text.as_bytes().split_first() {
        Some((erstes, rest)) => {
            matches!(erstes, b'a'..=b'z' | b'0'..=b'9')
                && rest.len() < 64
                && rest
                    .iter()
                    .all(|c| matches!(c, b'a'..=b'z' | b'0'..=b'9' | b'.' | b'_' | b'-'))
        }
        None => false,
    }
}

/// 64 lower-case hex digits.
fn ist_sha256(text: &str) -> bool {
    text.len() == 64 && text.bytes().all(|c| matches!(c, b'0'..=b'9' | b'a'..=b'f'))
}

/// The pattern of `zeit` of a line of version 2: `JJJJ-MM-TTThh:mm:ss.mmmZ` (the day is not checked against the month).
fn zeit_muster(text: &str) -> bool {
    let b = text.as_bytes();
    let zahl = |von: usize, bis: usize| -> Option<u32> {
        b.get(von..bis)?.iter().try_fold(0u32, |summe, c| {
            c.is_ascii_digit().then(|| summe * 10 + u32::from(c - b'0'))
        })
    };
    let zeichen = |stelle: usize, soll: u8| b.get(stelle) == Some(&soll);
    b.len() == 24
        && zahl(0, 4).is_some()
        && zeichen(4, b'-')
        && zahl(5, 7).is_some_and(|m| (1..=12).contains(&m))
        && zeichen(7, b'-')
        && zahl(8, 10).is_some_and(|t| (1..=31).contains(&t))
        && zeichen(10, b'T')
        && zahl(11, 13).is_some_and(|h| h <= 23)
        && zeichen(13, b':')
        && zahl(14, 16).is_some_and(|m| m <= 59)
        && zeichen(16, b':')
        && zahl(17, 19).is_some_and(|s| s <= 59)
        && zeichen(19, b'.')
        && zahl(20, 23).is_some()
        && zeichen(23, b'Z')
}

/// The first wrong field in the order `route`, `klasse`, `kunde`, `sitzung`, `grund`, `anzahl`, `vorher`.
fn felder_pruefen(g: &Gesperrt, vorher: &str) -> Result<(), Fehler> {
    let reihe = [
        ("route", schluessel_ok(&g.route)),
        ("klasse", g.klasse.as_deref().is_none_or(schluessel_ok)),
        ("kunde", g.kunde.as_deref().is_none_or(schluessel_ok)),
        ("sitzung", schluessel_ok(&g.sitzung)),
        ("grund", schluessel_ok(&g.grund)),
        ("anzahl", g.anzahl >= 1),
        ("vorher", ist_sha256(vorher)),
    ];
    match reihe.into_iter().find(|(_, ok)| !ok) {
        Some((feld, _)) => Err(Fehler::FeldUngueltig(feld)),
        None => Ok(()),
    }
}

/// The line `ausgabe.gesperrt` for the data and the `vorher` of the line before it, without the line feed.
///
/// # Errors
/// `feld_ungueltig:<feld>` for the first wrong value in the order `route`, `klasse`, `kunde`, `sitzung`,
/// `grund`, `anzahl`, `vorher`; after that `zeit_ungueltig`.
pub fn zeile_gesperrt(g: &Gesperrt, vorher: &str) -> Result<String, Fehler> {
    felder_pruefen(g, vorher)?;
    let zeit = zeit_text(g.zeit_ms)?;
    kanonisch(&json!({
        "aktionsname": "ausgabe.gesperrt",
        "anzahl": g.anzahl,
        "ergebnis": "gesperrt",
        "fassung": 2,
        "grund": g.grund,
        "klasse": g.klasse,
        "kunde": g.kunde,
        "route": g.route,
        "sitzung": g.sitzung,
        "vorher": vorher,
        "zeit": zeit,
    }))
}

/// The data of one line `kanal.abgewiesen` (contract 5, section 6); the fields are checked when a line is built.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Abgewiesen {
    /// Time in milliseconds since 1970 (UTC).
    pub zeit_ms: i64,
    /// Why the connection was turned away: one of [`GRUENDE_PEERKANAL`], or an own reason of the channel.
    pub grund: String,
    /// Key of the channel (`such`, `stelle`, `weiterreicher`, ...).
    pub kanal: String,
    /// Key of the route, or `None` when the connection had none.
    pub route: Option<String>,
    /// Identifier of the session, or `None` when the connection had none.
    pub sitzung: Option<String>,
}

/// The reasons of a line `kanal.abgewiesen` that every channel may use: the fixed reasons of the library
/// `peerkanal`, with the prefix `pk.`.
pub const GRUENDE_PEERKANAL: [&str; 13] = [
    "pk.hallo_fehlt",
    "pk.hallo_falsch",
    "pk.messung_gescheitert",
    "pk.prozess_nicht_offen",
    "pk.job_unbekannt",
    "pk.prozess_beendet",
    "pk.startzeit_weicht_ab",
    "pk.pidversion_weicht_ab",
    "pk.eingeschraenkt",
    "pk.sid_nicht_erlaubt",
    "pk.appcontainer_falsch",
    "pk.uid_nicht_erlaubt",
    "pk.gid_falsch",
];

/// The own reasons of the channel `such`; the prefix is the channel. `stelle` and `weiterreicher` have none today.
pub const GRUENDE_SUCH: [&str; 5] = [
    "such.dienst_stumm",
    "such.lauf_unbekannt",
    "such.sitzung_fremd",
    "such.kennung_fremd",
    "such.kennung_unmessbar",
];

/// A reason of a line `kanal.abgewiesen` for a channel: one of [`GRUENDE_PEERKANAL`], or an own reason whose prefix
/// is the channel (`such.*` only for the channel `such`).
fn grund_abgewiesen_ok(kanal: &str, grund: &str) -> bool {
    GRUENDE_PEERKANAL.contains(&grund) || (kanal == "such" && GRUENDE_SUCH.contains(&grund))
}

/// The first wrong field in the order `grund`, `kanal`, `route`, `sitzung`, `vorher`.
fn felder_abgewiesen(a: &Abgewiesen, vorher: &str) -> Result<(), Fehler> {
    let reihe = [
        ("grund", grund_abgewiesen_ok(&a.kanal, &a.grund)),
        ("kanal", schluessel_ok(&a.kanal)),
        ("route", a.route.as_deref().is_none_or(schluessel_ok)),
        ("sitzung", a.sitzung.as_deref().is_none_or(schluessel_ok)),
        ("vorher", ist_sha256(vorher)),
    ];
    match reihe.into_iter().find(|(_, ok)| !ok) {
        Some((feld, _)) => Err(Fehler::FeldUngueltig(feld)),
        None => Ok(()),
    }
}

/// The line `kanal.abgewiesen` for the data and the `vorher` of the line before it, without the line feed.
///
/// # Errors
/// `feld_ungueltig:<feld>` for the first wrong value in the order `grund`, `kanal`, `route`, `sitzung`,
/// `vorher`; after that `zeit_ungueltig`.
pub fn zeile_abgewiesen(a: &Abgewiesen, vorher: &str) -> Result<String, Fehler> {
    felder_abgewiesen(a, vorher)?;
    let zeit = zeit_text(a.zeit_ms)?;
    kanonisch(&json!({
        "aktionsname": "kanal.abgewiesen",
        "ergebnis": "abgewiesen",
        "fassung": 2,
        "grund": a.grund,
        "kanal": a.kanal,
        "route": a.route,
        "sitzung": a.sitzung,
        "vorher": vorher,
        "zeit": zeit,
    }))
}

/// The data of one line `probelauf.zaehlung` (contract 5, section 6): the use of the marker count in a trial run.
/// The line carries no marker, since a marker could reveal content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Probelauf {
    /// Time in milliseconds since 1970 (UTC).
    pub zeit_ms: i64,
    /// The program that counted: one of [`PROGRAMME_PROBELAUF`].
    pub programm: String,
    /// Identifier of the session.
    pub sitzung: String,
    /// Number counted, 1 up to 16.
    pub anzahl: u32,
}

/// The programs of a line `probelauf.zaehlung`.
pub const PROGRAMME_PROBELAUF: [&str; 2] = ["suche", "modellweg"];

/// The largest `anzahl` of a line `probelauf.zaehlung`.
pub const ANZAHL_PROBELAUF_MAX: u32 = 16;

/// The first wrong field in the order `programm`, `sitzung`, `anzahl`, `vorher`.
fn felder_probelauf(p: &Probelauf, vorher: &str) -> Result<(), Fehler> {
    let reihe = [
        (
            "programm",
            PROGRAMME_PROBELAUF.contains(&p.programm.as_str()),
        ),
        ("sitzung", schluessel_ok(&p.sitzung)),
        ("anzahl", (1..=ANZAHL_PROBELAUF_MAX).contains(&p.anzahl)),
        ("vorher", ist_sha256(vorher)),
    ];
    match reihe.into_iter().find(|(_, ok)| !ok) {
        Some((feld, _)) => Err(Fehler::FeldUngueltig(feld)),
        None => Ok(()),
    }
}

/// The line `probelauf.zaehlung` for the data and the `vorher` of the line before it, without the line feed.
///
/// # Errors
/// `feld_ungueltig:<feld>` for the first wrong value in the order `programm`, `sitzung`, `anzahl`, `vorher`;
/// after that `zeit_ungueltig`.
pub fn zeile_probelauf(p: &Probelauf, vorher: &str) -> Result<String, Fehler> {
    felder_probelauf(p, vorher)?;
    let zeit = zeit_text(p.zeit_ms)?;
    kanonisch(&json!({
        "aktionsname": "probelauf.zaehlung",
        "anzahl": p.anzahl,
        "ergebnis": "an",
        "fassung": 2,
        "programm": p.programm,
        "sitzung": p.sitzung,
        "vorher": vorher,
        "zeit": zeit,
    }))
}

/// The data of one line `ordnermarke.geloest` or `ordnermarke.gesenkt` (contract 5, section 6.4); the fields are
/// checked when a line is built. The `ergebnis` is fixed (`ausgefuehrt`) and no field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ordnermarke {
    /// Time in milliseconds since 1970 (UTC).
    pub zeit_ms: i64,
    /// `ordnermarke.geloest` or `ordnermarke.gesenkt`.
    pub aktionsname: String,
    /// The random identifier of the mark: 32 lower-case hex digits; no path and no hash of a path.
    pub marke: String,
    /// Class of the mark before the command, or `None`.
    pub klasse_alt: Option<String>,
    /// Class of the mark after the command, or `None`.
    pub klasse_neu: Option<String>,
    /// Customer of the mark before the command, or `None`.
    pub kunde_alt: Option<String>,
    /// Customer of the mark after the command, or `None`.
    pub kunde_neu: Option<String>,
}

/// The identifier of a mark: `^[0-9a-f]{32}$`.
fn marke_ok(text: &str) -> bool {
    text.len() == 32 && text.bytes().all(|c| matches!(c, b'0'..=b'9' | b'a'..=b'f'))
}

/// The coupling of the four values of a line `ordnermarke.*` for its kind (contract 5, section 6.4): the first
/// field that breaks it, or `None`. `geloest`: `klasse_alt` and `kunde_alt` not both `None`, then `klasse_neu`
/// and `kunde_neu` are `None`. `gesenkt`: `klasse_neu` is not `None` and not `klasse_alt`, `kunde_neu` is `kunde_alt`.
/// Another kind has no coupling.
fn ordnermarke_kopplung(
    aktionsname: &str,
    klasse_alt: Option<&str>,
    klasse_neu: Option<&str>,
    kunde_alt: Option<&str>,
    kunde_neu: Option<&str>,
) -> Option<&'static str> {
    match aktionsname {
        "ordnermarke.geloest" => [
            ("klasse_alt", klasse_alt.is_some() || kunde_alt.is_some()),
            ("klasse_neu", klasse_neu.is_none()),
            ("kunde_neu", kunde_neu.is_none()),
        ]
        .into_iter()
        .find(|(_, ok)| !ok)
        .map(|(feld, _)| feld),
        "ordnermarke.gesenkt" => [
            (
                "klasse_neu",
                klasse_neu.is_some() && klasse_neu != klasse_alt,
            ),
            ("kunde_neu", kunde_neu == kunde_alt),
        ]
        .into_iter()
        .find(|(_, ok)| !ok)
        .map(|(feld, _)| feld),
        _ => None,
    }
}

/// The first wrong field: the form of each field in the order `aktionsname`, `marke`, `klasse_alt`, `klasse_neu`,
/// `kunde_alt`, `kunde_neu`; then the coupling; then `vorher`.
fn felder_ordnermarke(o: &Ordnermarke, vorher: &str) -> Result<(), Fehler> {
    let schluessel = |wert: &Option<String>| wert.as_deref().is_none_or(schluessel_ok);
    let reihe = [
        (
            "aktionsname",
            matches!(
                o.aktionsname.as_str(),
                "ordnermarke.geloest" | "ordnermarke.gesenkt"
            ),
        ),
        ("marke", marke_ok(&o.marke)),
        ("klasse_alt", schluessel(&o.klasse_alt)),
        ("klasse_neu", schluessel(&o.klasse_neu)),
        ("kunde_alt", schluessel(&o.kunde_alt)),
        ("kunde_neu", schluessel(&o.kunde_neu)),
    ];
    if let Some((feld, _)) = reihe.into_iter().find(|(_, ok)| !ok) {
        return Err(Fehler::FeldUngueltig(feld));
    }
    if let Some(feld) = ordnermarke_kopplung(
        &o.aktionsname,
        o.klasse_alt.as_deref(),
        o.klasse_neu.as_deref(),
        o.kunde_alt.as_deref(),
        o.kunde_neu.as_deref(),
    ) {
        return Err(Fehler::FeldUngueltig(feld));
    }
    if !ist_sha256(vorher) {
        return Err(Fehler::FeldUngueltig("vorher"));
    }
    Ok(())
}

/// The line `ordnermarke.geloest` or `ordnermarke.gesenkt` for the data and the `vorher` of the line before it,
/// without the line feed.
///
/// # Errors
/// `feld_ungueltig:<feld>` for the first wrong value: the form of each field in the order `aktionsname`, `marke`,
/// `klasse_alt`, `klasse_neu`, `kunde_alt`, `kunde_neu`; then the coupling of the kind; then `vorher`; after that
/// `zeit_ungueltig`.
pub fn zeile_ordnermarke(o: &Ordnermarke, vorher: &str) -> Result<String, Fehler> {
    felder_ordnermarke(o, vorher)?;
    let zeit = zeit_text(o.zeit_ms)?;
    kanonisch(&json!({
        "aktionsname": o.aktionsname,
        "ergebnis": "ausgefuehrt",
        "fassung": 2,
        "klasse_alt": o.klasse_alt,
        "klasse_neu": o.klasse_neu,
        "kunde_alt": o.kunde_alt,
        "kunde_neu": o.kunde_neu,
        "marke": o.marke,
        "vorher": vorher,
        "zeit": zeit,
    }))
}

/// One of the five sources: `gateway`, `stelle`, `brain`, `halter` or `ordnermarke`.
fn quelle_ok(quelle: &str) -> bool {
    matches!(
        quelle,
        "gateway" | "stelle" | "brain" | "halter" | "ordnermarke"
    )
}

/// The name of the chain file for a source and a time: `<quelle>-<JJJJ-MM>.jsonl`.
///
/// # Errors
/// `feld_ungueltig:quelle` when `quelle` is not one of the five sources; `zeit_ungueltig`.
pub fn abschnitt(quelle: &str, ms: i64) -> Result<String, Fehler> {
    if !quelle_ok(quelle) {
        return Err(Fehler::FeldUngueltig("quelle"));
    }
    let zeit = zeit_text(ms)?;
    Ok(format!("{quelle}-{}.jsonl", &zeit[..7]))
}

#[cfg(test)]
mod tests;
