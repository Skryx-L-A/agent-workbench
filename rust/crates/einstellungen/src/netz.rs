//! Version 2 of contract 2: the target of a local route, the model name and the network targets.
//!
//! Each check records the violation it finds in the [`Befund`]; the lowest stage wins at the end.

use std::collections::HashSet;

use crate::Fehler;
use crate::aufbau::{Befund, anzahl, pflicht, unbekannte};
use crate::json::{Wert, feld};
use crate::typen::{Netzziel, PfadRegel, Schema, Ziel, Zugriff};

const NETZZIEL: [&str; 5] = ["host", "port", "zugriff", "schluessel", "pfade"];
const PFAD: [&str; 2] = ["methode", "pfad"];
const ZIEL_LAENGE_MAX: usize = 256;
const HOST_LAENGE_MAX: usize = 253;
const PFAD_LAENGE_MAX: usize = 256;
const MODELL_LAENGE_MAX: usize = 128;
const METHODEN: [&str; 6] = ["GET", "HEAD", "POST", "PUT", "PATCH", "DELETE"];

/// Tells whether `text` is a model name: 1 to 128 characters of `A-Za-z0-9._:/@+-`.
pub(crate) fn ist_modell(text: &str) -> bool {
    (1..=MODELL_LAENGE_MAX).contains(&text.len())
        && text.bytes().all(|c| {
            c.is_ascii_alphanumeric() || matches!(c, b'.' | b'_' | b':' | b'/' | b'@' | b'+' | b'-')
        })
}

/// One label of a host name: 1 to 63 characters of `a-z0-9-`, not starting or ending with `-`.
fn ist_namensteil(teil: &str) -> bool {
    let bytes = teil.as_bytes();
    (1..=63).contains(&bytes.len())
        && bytes
            .iter()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == b'-')
        && bytes.first() != Some(&b'-')
        && bytes.last() != Some(&b'-')
}

/// A host name of at least `teile_min` labels; the last label starts with a letter.
fn ist_name(text: &str, teile_min: usize) -> bool {
    if text.len() > HOST_LAENGE_MAX {
        return false;
    }
    let teile: Vec<&str> = text.split('.').collect();
    teile.len() >= teile_min
        && teile.iter().all(|teil| ist_namensteil(teil))
        && teile
            .last()
            .and_then(|teil| teil.bytes().next())
            .is_some_and(|c| c.is_ascii_lowercase())
}

/// An IPv4 address of four decimal parts from 0 to 255 without a leading zero. The text consists of
/// digits and dots only; the caller checks that.
fn ipv4(text: &str) -> Option<[u8; 4]> {
    let mut teile = text.split('.');
    let mut adresse = [0_u8; 4];
    for stelle in &mut adresse {
        let teil = teile.next()?;
        if teil.len() > 1 && teil.starts_with('0') {
            return None;
        }
        *stelle = teil.parse().ok()?;
    }
    teile.next().is_none().then_some(adresse)
}

/// One part of a path: characters of `A-Za-z0-9._~-`, not empty, not `.` and not `..`.
fn ist_teil(teil: &str) -> bool {
    !matches!(teil, "" | "." | "..")
        && teil
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'.' | b'_' | b'~' | b'-'))
}

/// A sequence of `/<part>` with at least one part.
fn ist_pfad_folge(text: &str) -> bool {
    text.strip_prefix('/')
        .is_some_and(|rest| rest.split('/').all(ist_teil))
}

/// The host of a network target: a name of at least two labels, optionally with the prefix `*.`.
/// Returns `Some(true)` for the subdomain form, `Some(false)` for a plain name and `None` for a violation.
fn netz_host(text: &str) -> Option<bool> {
    let (name, stern) = match text.strip_prefix("*.") {
        Some(rest) => (rest, true),
        None => (text, false),
    };
    let lokal = name == "localhost" || name.ends_with(".localhost");
    (ist_name(name, 2) && !lokal).then_some(stern)
}

/// The key `name` as text; a wrong type records stage 6.
fn text<'a>(felder: &'a [(String, Wert)], name: &str, befund: &mut Befund) -> Option<&'a str> {
    let wert = feld(felder, name)?;
    if let Wert::Text(t) = wert {
        Some(t)
    } else {
        befund.melde(Fehler::TypFalsch);
        None
    }
}

/// The optional `modell` of a route. A wrong form records stage 23.
pub(crate) fn modell(felder: &[(String, Wert)], befund: &mut Befund) -> Option<String> {
    let t = text(felder, "modell", befund)?;
    if !ist_modell(t) {
        befund.melde(Fehler::ModellForm);
    }
    Some(t.to_owned())
}

/// The optional `ziel` of a route. Returns `None` when the key is absent, not a text, or violates a
/// rule of the stages 19 to 22 (the violation is recorded).
pub(crate) fn ziel(felder: &[(String, Wert)], befund: &mut Befund) -> Option<Ziel> {
    let t = text(felder, "ziel", befund)?;
    ziel_lesen(t, befund)
}

fn ziel_lesen(text: &str, befund: &mut Befund) -> Option<Ziel> {
    let schema = match text.split_once(':').map(|(schema, _)| schema) {
        Some("http") => Schema::Http,
        Some("https") => Schema::Https,
        _ => {
            befund.melde(Fehler::ZielSchema);
            return None;
        }
    };
    let rest = match text.split_once("://") {
        Some((_, rest)) if text.len() <= ZIEL_LAENGE_MAX && text.is_ascii() => rest,
        _ => {
            befund.melde(Fehler::ZielForm);
            return None;
        }
    };
    let (host_port, pfad) = rest
        .find('/')
        .map_or((rest, ""), |stelle| rest.split_at(stelle));
    let Some((host, port)) = host_port.split_once(':') else {
        befund.melde(Fehler::ZielForm);
        return None;
    };
    let pfad_gut = pfad
        .strip_suffix("/v1")
        .is_some_and(|davor| davor.is_empty() || ist_pfad_folge(davor));
    let host_art = if host.bytes().all(|c| c.is_ascii_digit() || c == b'.') {
        ipv4(host).map(Some)
    } else {
        ist_name(host, 1).then_some(None)
    };
    let port_gut = !port.is_empty()
        && port.len() <= 5
        && port.bytes().all(|c| c.is_ascii_digit())
        && !(port.len() > 1 && port.starts_with('0'));
    let (Some(host_art), true, true) = (host_art, pfad_gut, port_gut) else {
        befund.melde(Fehler::ZielForm);
        return None;
    };
    let Some(portwert) = port.parse::<u16>().ok().filter(|p| *p >= 1) else {
        befund.melde(Fehler::ZielPort);
        return None;
    };
    let erlaubt_host = match host_art {
        Some([127 | 10, ..] | [192, 168, ..]) | None => true,
        Some([172, zweites, ..]) => (16..=31).contains(&zweites),
        Some(_) => false,
    };
    if !erlaubt_host || (schema == Schema::Http && host != "127.0.0.1") {
        befund.melde(Fehler::ZielHost);
        return None;
    }
    Some(Ziel {
        schema,
        host: host.to_owned(),
        port: portwert,
        pfad: pfad.to_owned(),
    })
}

/// The list `netzziele` of a version 2 route; the field is required.
pub(crate) fn netzziele(felder: &[(String, Wert)], befund: &mut Befund) -> Vec<Netzziel> {
    let Some(wert) = pflicht(felder, "netzziele", befund) else {
        return Vec::new();
    };
    let Wert::Liste(elemente) = wert else {
        befund.melde(Fehler::TypFalsch);
        return Vec::new();
    };
    anzahl(elemente.len(), 0, 64, befund);
    let mut ziele = Vec::new();
    for element in elemente {
        match element {
            Wert::Objekt(felder) => ziele.push(netzziel(felder, befund)),
            _ => befund.melde(Fehler::TypFalsch),
        }
    }
    let mut gesehen = HashSet::new();
    if !ziele
        .iter()
        .all(|ziel| gesehen.insert((ziel.host.clone(), ziel.port)))
    {
        befund.melde(Fehler::KennungDoppelt);
    }
    ziele
}

fn netzziel(felder: &[(String, Wert)], befund: &mut Befund) -> Netzziel {
    unbekannte(felder, &NETZZIEL, befund);
    let host = pflicht(felder, "host", befund);
    let port = pflicht(felder, "port", befund);
    let zugriff = pflicht(felder, "zugriff", befund);
    let mut ziel = Netzziel {
        host: String::new(),
        port: 0,
        zugriff: Zugriff::Verbinden,
        schluessel: false,
        pfade: Vec::new(),
        unterdomaene: false,
    };
    match host {
        Some(Wert::Text(t)) => match netz_host(t) {
            Some(stern) => {
                t.clone_into(&mut ziel.host);
                ziel.unterdomaene = stern;
            }
            None => befund.melde(Fehler::NetzzielForm),
        },
        Some(_) => befund.melde(Fehler::TypFalsch),
        None => {}
    }
    match port {
        Some(Wert::Ganzzahl(zahl)) => match zahl.parse::<u16>().ok().filter(|p| *p >= 1) {
            Some(p) => ziel.port = p,
            None => befund.melde(Fehler::NetzzielPort),
        },
        Some(_) => befund.melde(Fehler::TypFalsch),
        None => {}
    }
    let art = match zugriff {
        Some(Wert::Text(t)) => match t.as_str() {
            "verbinden" => Some(Zugriff::Verbinden),
            "http-lesen" => Some(Zugriff::HttpLesen),
            "kopf-einsetzen" => Some(Zugriff::KopfEinsetzen),
            _ => {
                befund.melde(Fehler::ZugriffUnbekannt);
                None
            }
        },
        Some(_) => {
            befund.melde(Fehler::TypFalsch);
            None
        }
        None => None,
    };
    let schluessel = match feld(felder, "schluessel") {
        Some(Wert::Bool(b)) => Some(*b),
        Some(_) => {
            befund.melde(Fehler::TypFalsch);
            None
        }
        None => None,
    };
    let pfade = feld(felder, "pfade").map(|wert| pfade(wert, befund));
    if let Some(art) = art {
        ziel.zugriff = art;
        let mit_kopf = art == Zugriff::KopfEinsetzen;
        if mit_kopf != feld(felder, "schluessel").is_some() || mit_kopf != pfade.is_some() {
            befund.melde(Fehler::ZugriffFelder);
        }
    }
    ziel.schluessel = schluessel.unwrap_or(false);
    ziel.pfade = pfade.unwrap_or_default();
    ziel
}

fn pfade(wert: &Wert, befund: &mut Befund) -> Vec<PfadRegel> {
    let Wert::Liste(elemente) = wert else {
        befund.melde(Fehler::TypFalsch);
        return Vec::new();
    };
    anzahl(elemente.len(), 1, 32, befund);
    let mut regeln = Vec::new();
    for element in elemente {
        match element {
            Wert::Objekt(felder) => regeln.push(pfad_regel(felder, befund)),
            _ => befund.melde(Fehler::TypFalsch),
        }
    }
    let mut gesehen = HashSet::new();
    if !regeln
        .iter()
        .all(|regel| gesehen.insert((regel.methode.clone(), regel.pfad.clone())))
    {
        befund.melde(Fehler::KennungDoppelt);
    }
    regeln
}

fn pfad_regel(felder: &[(String, Wert)], befund: &mut Befund) -> PfadRegel {
    unbekannte(felder, &PFAD, befund);
    let methode = pflicht(felder, "methode", befund);
    let pfad = pflicht(felder, "pfad", befund);
    let mut regel = PfadRegel {
        methode: String::new(),
        pfad: String::new(),
    };
    match methode {
        Some(Wert::Text(t)) if METHODEN.contains(&t.as_str()) => t.clone_into(&mut regel.methode),
        Some(Wert::Text(_)) => befund.melde(Fehler::PfadForm),
        Some(_) => befund.melde(Fehler::TypFalsch),
        None => {}
    }
    match pfad {
        Some(Wert::Text(t)) if ist_pfadmuster(t) => t.clone_into(&mut regel.pfad),
        Some(Wert::Text(_)) => befund.melde(Fehler::PfadForm),
        Some(_) => befund.melde(Fehler::TypFalsch),
        None => {}
    }
    regel
}

/// A path pattern: `/<part>...`, or such a sequence followed by `/*`, or `/*` alone.
fn ist_pfadmuster(text: &str) -> bool {
    if text.len() > PFAD_LAENGE_MAX {
        return false;
    }
    match text.strip_suffix("/*") {
        Some("") => true,
        Some(davor) => ist_pfad_folge(davor),
        None => ist_pfad_folge(text),
    }
}

/// The two rules for a route with a bound key (stages 30 and 31).
pub(crate) fn schluesselregeln(netzziele: &[Netzziel], befund: &mut Befund) {
    let gebunden = |ziel: &&Netzziel| ziel.zugriff == Zugriff::KopfEinsetzen && ziel.schluessel;
    if netzziele.iter().any(|ziel| gebunden(&ziel))
        && netzziele
            .iter()
            .any(|ziel| ziel.zugriff == Zugriff::Verbinden)
    {
        befund.melde(Fehler::VerbindenMitSchluessel);
    }
    if netzziele
        .iter()
        .filter(gebunden)
        .any(|ziel| ziel.unterdomaene)
    {
        befund.melde(Fehler::SchluesselUnterdomaene);
    }
}
