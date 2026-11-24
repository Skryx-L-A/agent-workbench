//! Reads the settings file: stages 1 to 33 of contract 2.
//!
//! Every check records the violation it finds. At the end the lowest stage wins, so the code is the
//! one of the first stage in the order of the contract, whatever the position in the file.

use std::collections::HashSet;
use std::hash::Hash;

use crate::Fehler;
use crate::ist_kennung;
use crate::json::{self, Wert, feld};
use crate::netz;
use crate::typen::{
    Einstellungen, Freigabe, Kunde, KundendatenAnCloud, McpServer, Route, Routenart, Werkzeug,
};

/// The default of `frist_ms` and `einloesefrist_ms` of a server (contract 2, block `mcp`).
const MCP_FRIST_VORGABE_MS: u64 = 60_000;

const GROESSE_MAX: usize = 65536;
const OBEN: [&str; 6] = [
    "fassung",
    "klassen",
    "vorgabestufe",
    "routen",
    "kunden",
    "kundendaten_an_cloud",
];
const OBEN_2: [&str; 8] = [
    "fassung",
    "klassen",
    "vorgabestufe",
    "routen",
    "kunden",
    "kundendaten_an_cloud",
    "mcp",
    "harness",
];
const MCP_SERVER: [&str; 5] = [
    "lesen",
    "schreiben",
    "freigabe",
    "frist_ms",
    "einloesefrist_ms",
];
const HARNESS: [&str; 1] = ["copilot"];
const HARNESS_COPILOT: [&str; 1] = ["werkzeuge_ohne_rueckfrage"];
const FRIST_MAX: i128 = 86_400_000;
const ROUTE: [&str; 3] = ["schluessel", "art", "klasse"];
const ROUTE_2: [&str; 6] = ["schluessel", "art", "klasse", "ziel", "modell", "netzziele"];
const KUNDE: [&str; 3] = ["schluessel", "erlaubt", "verboten"];
const KUNDENDATEN: [&str; 2] = ["vorgabe", "freigaben"];
const FREIGABE: [&str; 2] = ["route", "kunde"];

/// Reads the bytes of a settings file.
///
/// # Errors
/// Returns the [`Fehler`] of the lowest stage of contract 2 that the file violates.
pub fn lesen(bytes: &[u8]) -> Result<Einstellungen, Fehler> {
    if bytes.len() > GROESSE_MAX {
        return Err(Fehler::ZuGross);
    }
    let wert = json::lesen(bytes).map_err(|_| Fehler::JsonUngueltig)?;
    let Wert::Objekt(felder) = wert else {
        return Err(Fehler::JsonUngueltig);
    };
    let mut befund = Befund::default();
    let mut roh = Roh::formlesen(&felder, &mut befund);
    roh.querverweise(&mut befund);
    befund.ergebnis()?;
    Ok(roh.fertig())
}

/// Every violation found in one file.
#[derive(Default)]
pub(crate) struct Befund {
    fehler: Vec<Fehler>,
}

impl Befund {
    pub(crate) fn melde(&mut self, fehler: Fehler) {
        self.fehler.push(fehler);
    }

    fn ergebnis(self) -> Result<(), Fehler> {
        match self.fehler.into_iter().min_by_key(Fehler::stufe) {
            Some(fehler) => Err(fehler),
            None => Ok(()),
        }
    }
}

/// The file in its parts. Missing or wrong parts keep a placeholder; the recorded violation decides.
struct Roh {
    fassung: u8,
    klassen: Vec<String>,
    vorgabestufe: Option<String>,
    routen: Vec<Route>,
    kunden: Vec<Kunde>,
    freigaben: Vec<Freigabe>,
    copilot_ohne_rueckfrage: Vec<Werkzeug>,
    mcp: Vec<McpServer>,
}

impl Roh {
    fn formlesen(felder: &[(String, Wert)], befund: &mut Befund) -> Self {
        let zwei = bezug_zwei(felder);
        unbekannte(felder, if zwei { &OBEN_2 } else { &OBEN }, befund);
        fassung(felder, befund);
        let klassen = klassen(felder, befund);
        let vorgabestufe = vorgabestufe(felder, befund);
        let routen = routen(felder, zwei, befund);
        let kunden = kunden(felder, befund);
        let freigaben = kundendaten(felder, befund);
        let mut copilot_ohne_rueckfrage = Vec::new();
        let mut mcp_server = Vec::new();
        if zwei {
            mcp_server = mcp(felder, befund);
            copilot_ohne_rueckfrage = harness(felder, befund);
        }
        Self {
            fassung: if zwei { 2 } else { 1 },
            klassen,
            vorgabestufe,
            routen,
            kunden,
            freigaben,
            copilot_ohne_rueckfrage,
            mcp: mcp_server,
        }
    }

    /// The references between the parts: stages 12 to 17.
    fn querverweise(&mut self, befund: &mut Befund) {
        for route in &self.routen {
            if !self.klassen.contains(&route.klasse) {
                befund.melde(Fehler::KlasseUnbekannt);
            }
        }
        for kunde in &self.kunden {
            for name in kunde.erlaubt.iter().chain(&kunde.verboten) {
                if !self.routen.iter().any(|route| route.schluessel == *name) {
                    befund.melde(Fehler::RouteUnbekannt);
                }
            }
        }
        for freigabe in &self.freigaben {
            match self
                .routen
                .iter()
                .find(|route| route.schluessel == freigabe.route)
            {
                None => befund.melde(Fehler::RouteUnbekannt),
                Some(route) if route.art == Routenart::Lokal => {
                    befund.melde(Fehler::FreigabeRouteLokal);
                }
                Some(_) => {}
            }
            if !self
                .kunden
                .iter()
                .any(|kunde| kunde.schluessel == freigabe.kunde)
            {
                befund.melde(Fehler::KundeUnbekannt);
            }
        }
        if let Some(name) = &self.vorgabestufe
            && !self.klassen.contains(name)
        {
            befund.melde(Fehler::VorgabestufeUnbekannt);
        }
    }

    /// The typed view. Called only when no violation was recorded.
    fn fertig(mut self) -> Einstellungen {
        for route in &mut self.routen {
            route.rang = self
                .klassen
                .iter()
                .position(|klasse| *klasse == route.klasse)
                .unwrap_or(0);
        }
        let vorgabe_rang = match &self.vorgabestufe {
            Some(name) => self
                .klassen
                .iter()
                .position(|klasse| klasse == name)
                .unwrap_or(0),
            None => self.klassen.len().saturating_sub(1),
        };
        Einstellungen {
            fassung: self.fassung,
            klassen: self.klassen,
            vorgabe_rang,
            routen: self.routen,
            kunden: self.kunden,
            kundendaten_an_cloud: KundendatenAnCloud {
                freigaben: self.freigaben,
            },
            copilot_ohne_rueckfrage: self.copilot_ohne_rueckfrage,
            mcp: self.mcp,
        }
    }
}

pub(crate) fn unbekannte(felder: &[(String, Wert)], erlaubt: &[&str], befund: &mut Befund) {
    if felder
        .iter()
        .any(|(name, _)| !erlaubt.contains(&name.as_str()))
    {
        befund.melde(Fehler::SchluesselUnbekannt);
    }
}

/// A required key: records stage 5 when it is missing.
pub(crate) fn pflicht<'a>(
    felder: &'a [(String, Wert)],
    name: &str,
    befund: &mut Befund,
) -> Option<&'a Wert> {
    let wert = feld(felder, name);
    if wert.is_none() {
        befund.melde(Fehler::SchluesselFehlt);
    }
    wert
}

/// A required key with a text value.
fn text<'a>(felder: &'a [(String, Wert)], name: &str, befund: &mut Befund) -> Option<&'a str> {
    let Wert::Text(t) = pflicht(felder, name, befund)? else {
        befund.melde(Fehler::TypFalsch);
        return None;
    };
    Some(t)
}

/// A required key with a text value of the form of a key.
fn kennung<'a>(felder: &'a [(String, Wert)], name: &str, befund: &mut Befund) -> Option<&'a str> {
    let t = text(felder, name, befund)?;
    kennung_pruefen(t, befund);
    Some(t)
}

fn kennung_pruefen(t: &str, befund: &mut Befund) {
    if !ist_kennung(t) {
        befund.melde(Fehler::KennungForm);
    }
}

pub(crate) fn anzahl(anzahl: usize, von: usize, bis: usize, befund: &mut Befund) {
    if !(von..=bis).contains(&anzahl) {
        befund.melde(Fehler::Anzahl);
    }
}

/// Records stage 10 when a value occurs twice.
fn ohne_doppel<T: Eq + Hash>(werte: impl Iterator<Item = T>, befund: &mut Befund) {
    let mut gesehen = HashSet::new();
    if werte.into_iter().any(|wert| !gesehen.insert(wert)) {
        befund.melde(Fehler::KennungDoppelt);
    }
}

/// The elements of a list of keys. A non-text element records stage 6, a text of the wrong form stage 9.
fn kennungen(elemente: &[Wert], befund: &mut Befund) -> Vec<String> {
    let mut namen = Vec::new();
    for element in elemente {
        match element {
            Wert::Text(t) => {
                kennung_pruefen(t, befund);
                namen.push(t.clone());
            }
            _ => befund.melde(Fehler::TypFalsch),
        }
    }
    namen
}

/// The version that stages 3 and 5 use: the form of version 2 when `fassung` is a JSON number with the
/// value 2 (also `2.0`), otherwise the form of version 1.
fn bezug_zwei(felder: &[(String, Wert)]) -> bool {
    match feld(felder, "fassung") {
        Some(Wert::Ganzzahl(zahl)) => zahl == "2",
        Some(Wert::Kommazahl(zahl)) => zahl
            .parse::<f64>()
            .is_ok_and(|wert| wert.to_bits() == 2.0_f64.to_bits()),
        _ => false,
    }
}

fn fassung(felder: &[(String, Wert)], befund: &mut Befund) {
    match pflicht(felder, "fassung", befund) {
        None => {}
        Some(Wert::Ganzzahl(zahl)) if zahl == "1" || zahl == "2" => {}
        Some(Wert::Ganzzahl(_)) => befund.melde(Fehler::FassungUnbekannt),
        Some(_) => befund.melde(Fehler::TypFalsch),
    }
}

fn klassen(felder: &[(String, Wert)], befund: &mut Befund) -> Vec<String> {
    let Some(wert) = pflicht(felder, "klassen", befund) else {
        return Vec::new();
    };
    let Wert::Liste(elemente) = wert else {
        befund.melde(Fehler::TypFalsch);
        return Vec::new();
    };
    anzahl(elemente.len(), 1, 16, befund);
    let namen = kennungen(elemente, befund);
    ohne_doppel(namen.iter(), befund);
    namen
}

fn vorgabestufe(felder: &[(String, Wert)], befund: &mut Befund) -> Option<String> {
    let Wert::Text(t) = feld(felder, "vorgabestufe")? else {
        befund.melde(Fehler::TypFalsch);
        return None;
    };
    kennung_pruefen(t, befund);
    Some(t.clone())
}

fn routen(felder: &[(String, Wert)], zwei: bool, befund: &mut Befund) -> Vec<Route> {
    let Some(wert) = pflicht(felder, "routen", befund) else {
        return Vec::new();
    };
    let Wert::Liste(elemente) = wert else {
        befund.melde(Fehler::TypFalsch);
        return Vec::new();
    };
    anzahl(elemente.len(), 1, 64, befund);
    let mut routen = Vec::new();
    for element in elemente {
        match element {
            Wert::Objekt(felder) => routen.push(route(felder, zwei, befund)),
            _ => befund.melde(Fehler::TypFalsch),
        }
    }
    ohne_doppel(routen.iter().map(|route| &route.schluessel), befund);
    routen
}

fn route(felder: &[(String, Wert)], zwei: bool, befund: &mut Befund) -> Route {
    unbekannte(felder, if zwei { &ROUTE_2 } else { &ROUTE }, befund);
    let klasse = match feld(felder, "klasse") {
        None | Some(Wert::Null) => {
            befund.melde(Fehler::RouteOhneKlasse);
            String::new()
        }
        Some(Wert::Text(t)) => {
            kennung_pruefen(t, befund);
            t.clone()
        }
        Some(_) => {
            befund.melde(Fehler::TypFalsch);
            String::new()
        }
    };
    let schluessel = kennung(felder, "schluessel", befund)
        .unwrap_or_default()
        .to_owned();
    let art = art(felder, befund).unwrap_or(Routenart::Cloud);
    let mut route = Route {
        schluessel,
        art,
        klasse,
        rang: 0,
        ziel: None,
        modell: None,
        netzziele: Vec::new(),
    };
    if zwei {
        ziel_und_netz(felder, &mut route, befund);
    }
    route
}

/// Version 2 of a route: `ziel`, `modell` and `netzziele` with stages 18 to 31.
fn ziel_und_netz(felder: &[(String, Wert)], route: &mut Route, befund: &mut Befund) {
    let ziel = netz::ziel(felder, befund);
    route.modell = netz::modell(felder, befund);
    route.netzziele = netz::netzziele(felder, befund);
    route.ziel = ziel;
    match route.art {
        Routenart::Lokal => {
            if feld(felder, "ziel").is_none() || feld(felder, "modell").is_none() {
                befund.melde(Fehler::ZielArt);
            }
            if !route.netzziele.is_empty() {
                befund.melde(Fehler::NetzzielLokal);
            }
        }
        Routenart::Cloud => {
            if feld(felder, "ziel").is_some() {
                befund.melde(Fehler::ZielArt);
            }
        }
    }
    netz::schluesselregeln(&route.netzziele, befund);
}

fn art(felder: &[(String, Wert)], befund: &mut Befund) -> Option<Routenart> {
    match pflicht(felder, "art", befund)? {
        Wert::Text(t) if t == "cloud" => Some(Routenart::Cloud),
        Wert::Text(t) if t == "lokal" => Some(Routenart::Lokal),
        Wert::Text(_) => {
            befund.melde(Fehler::ArtUnbekannt);
            None
        }
        _ => {
            befund.melde(Fehler::TypFalsch);
            None
        }
    }
}

fn kunden(felder: &[(String, Wert)], befund: &mut Befund) -> Vec<Kunde> {
    let Some(wert) = pflicht(felder, "kunden", befund) else {
        return Vec::new();
    };
    let Wert::Liste(elemente) = wert else {
        befund.melde(Fehler::TypFalsch);
        return Vec::new();
    };
    anzahl(elemente.len(), 0, 256, befund);
    let mut kunden = Vec::new();
    for element in elemente {
        match element {
            Wert::Objekt(felder) => kunden.push(kunde(felder, befund)),
            _ => befund.melde(Fehler::TypFalsch),
        }
    }
    ohne_doppel(kunden.iter().map(|kunde| &kunde.schluessel), befund);
    kunden
}

fn kunde(felder: &[(String, Wert)], befund: &mut Befund) -> Kunde {
    unbekannte(felder, &KUNDE, befund);
    let schluessel = kennung(felder, "schluessel", befund)
        .unwrap_or_default()
        .to_owned();
    let erlaubt = routenliste(felder, "erlaubt", befund);
    let verboten = routenliste(felder, "verboten", befund);
    Kunde {
        schluessel,
        erlaubt,
        verboten,
    }
}

/// An optional list of route keys, at most 64 entries, without duplicates.
fn routenliste(felder: &[(String, Wert)], name: &str, befund: &mut Befund) -> Vec<String> {
    let Some(wert) = feld(felder, name) else {
        return Vec::new();
    };
    let Wert::Liste(elemente) = wert else {
        befund.melde(Fehler::TypFalsch);
        return Vec::new();
    };
    anzahl(elemente.len(), 0, 64, befund);
    let namen = kennungen(elemente, befund);
    ohne_doppel(namen.iter(), befund);
    namen
}

/// The object `kundendaten_an_cloud`: required in every version, with `vorgabe` and `freigaben`.
fn kundendaten(felder: &[(String, Wert)], befund: &mut Befund) -> Vec<Freigabe> {
    let Some(wert) = pflicht(felder, "kundendaten_an_cloud", befund) else {
        return Vec::new();
    };
    let Wert::Objekt(objekt) = wert else {
        befund.melde(Fehler::TypFalsch);
        return Vec::new();
    };
    unbekannte(objekt, &KUNDENDATEN, befund);
    vorgabe(objekt, befund);
    let Some(wert) = pflicht(objekt, "freigaben", befund) else {
        return Vec::new();
    };
    let Wert::Liste(elemente) = wert else {
        befund.melde(Fehler::TypFalsch);
        return Vec::new();
    };
    anzahl(elemente.len(), 0, 1024, befund);
    let mut freigaben = Vec::new();
    for element in elemente {
        match element {
            Wert::Objekt(felder) => freigaben.push(freigabe(felder, befund)),
            _ => befund.melde(Fehler::TypFalsch),
        }
    }
    ohne_doppel(
        freigaben
            .iter()
            .map(|freigabe| (&freigabe.route, &freigabe.kunde)),
        befund,
    );
    freigaben
}

fn vorgabe(objekt: &[(String, Wert)], befund: &mut Befund) {
    match pflicht(objekt, "vorgabe", befund) {
        None => {}
        Some(Wert::Text(t)) if t == "nie" => {}
        Some(Wert::Text(_)) => befund.melde(Fehler::VorgabeWert),
        Some(_) => befund.melde(Fehler::TypFalsch),
    }
}

fn freigabe(felder: &[(String, Wert)], befund: &mut Befund) -> Freigabe {
    unbekannte(felder, &FREIGABE, befund);
    let route = kennung(felder, "route", befund)
        .unwrap_or_default()
        .to_owned();
    let kunde = kennung(felder, "kunde", befund)
        .unwrap_or_default()
        .to_owned();
    Freigabe { route, kunde }
}

/// The optional block `harness` of version 2: the tool kinds that the Copilot CLI runs without asking. A value
/// that is a text but none of the three allowed kinds records stage 34. An invalid file keeps nothing.
fn harness(felder: &[(String, Wert)], befund: &mut Befund) -> Vec<Werkzeug> {
    let Some(wert) = feld(felder, "harness") else {
        return Vec::new();
    };
    let Wert::Objekt(eintraege) = wert else {
        befund.melde(Fehler::TypFalsch);
        return Vec::new();
    };
    unbekannte(eintraege, &HARNESS, befund);
    let Some(copilot) = feld(eintraege, "copilot") else {
        return Vec::new();
    };
    let Wert::Objekt(copilot) = copilot else {
        befund.melde(Fehler::TypFalsch);
        return Vec::new();
    };
    unbekannte(copilot, &HARNESS_COPILOT, befund);
    let Some(liste) = pflicht(copilot, "werkzeuge_ohne_rueckfrage", befund) else {
        return Vec::new();
    };
    let Wert::Liste(elemente) = liste else {
        befund.melde(Fehler::TypFalsch);
        return Vec::new();
    };
    let mut werte = Vec::new();
    let mut gueltig = Vec::new();
    for element in elemente {
        match element {
            Wert::Text(t) => {
                werte.push(t.as_str());
                match Werkzeug::aus_wert(t) {
                    Some(werkzeug) => gueltig.push(werkzeug),
                    None => befund.melde(Fehler::WerkzeugWert),
                }
            }
            _ => befund.melde(Fehler::TypFalsch),
        }
    }
    ohne_doppel(werte.into_iter(), befund);
    gueltig
}

/// The optional block `mcp` of version 2: the form is checked, the servers are kept in the order of the file.
fn mcp(felder: &[(String, Wert)], befund: &mut Befund) -> Vec<McpServer> {
    let Some(wert) = feld(felder, "mcp") else {
        return Vec::new();
    };
    let Wert::Objekt(server) = wert else {
        befund.melde(Fehler::TypFalsch);
        return Vec::new();
    };
    anzahl(server.len(), 0, 64, befund);
    let mut gelesen = Vec::new();
    for (name, wert) in server {
        kennung_pruefen(name, befund);
        match wert {
            Wert::Objekt(felder) => gelesen.push(mcp_server(name, felder, befund)),
            _ => befund.melde(Fehler::TypFalsch),
        }
    }
    gelesen
}

fn mcp_server(name: &str, felder: &[(String, Wert)], befund: &mut Befund) -> McpServer {
    unbekannte(felder, &MCP_SERVER, befund);
    let lesen = werkzeuge(felder, "lesen", befund);
    let schreiben = werkzeuge(felder, "schreiben", befund);
    let mut freigabe = true;
    if let Some(wert) = feld(felder, "freigabe") {
        match wert {
            Wert::Bool(b) => freigabe = *b,
            _ => befund.melde(Fehler::TypFalsch),
        }
    }
    let frist_ms = frist(felder, "frist_ms", befund).unwrap_or(MCP_FRIST_VORGABE_MS);
    let einloesefrist_ms =
        frist(felder, "einloesefrist_ms", befund).unwrap_or(MCP_FRIST_VORGABE_MS);
    if lesen.iter().any(|name| schreiben.contains(name)) {
        befund.melde(Fehler::McpDoppelt);
    }
    McpServer {
        name: name.to_owned(),
        lesen,
        schreiben,
        freigabe,
        frist_ms,
        einloesefrist_ms,
    }
}

/// An optional deadline: an integer from 1 to 86400000. `None` when absent or violating the form.
fn frist(felder: &[(String, Wert)], name: &str, befund: &mut Befund) -> Option<u64> {
    match feld(felder, name) {
        None => None,
        Some(Wert::Ganzzahl(zahl)) => {
            let im_bereich = zahl
                .parse::<i128>()
                .ok()
                .filter(|wert| (1..=FRIST_MAX).contains(wert))
                .and_then(|wert| u64::try_from(wert).ok());
            if im_bereich.is_none() {
                befund.melde(Fehler::FristBereich);
            }
            im_bereich
        }
        Some(_) => {
            befund.melde(Fehler::TypFalsch);
            None
        }
    }
}

/// A required list of tool names, at most 256, without duplicates.
fn werkzeuge(felder: &[(String, Wert)], name: &str, befund: &mut Befund) -> Vec<String> {
    let Some(wert) = pflicht(felder, name, befund) else {
        return Vec::new();
    };
    let Wert::Liste(elemente) = wert else {
        befund.melde(Fehler::TypFalsch);
        return Vec::new();
    };
    anzahl(elemente.len(), 0, 256, befund);
    let mut namen = Vec::new();
    for element in elemente {
        match element {
            Wert::Text(t) => {
                if !ist_werkzeug(t) {
                    befund.melde(Fehler::KennungForm);
                }
                namen.push(t.clone());
            }
            _ => befund.melde(Fehler::TypFalsch),
        }
    }
    ohne_doppel(namen.iter(), befund);
    namen
}

/// `^[A-Za-z0-9][A-Za-z0-9._-]{0,127}$`
fn ist_werkzeug(text: &str) -> bool {
    let mut zeichen = text.bytes();
    let erstes = zeichen.next().is_some_and(|b| b.is_ascii_alphanumeric());
    erstes
        && text.len() <= 128
        && zeichen.all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
}
