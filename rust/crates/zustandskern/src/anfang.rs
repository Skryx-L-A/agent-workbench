//! Reading the start of a case (`anfang`) into the state of the core.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Map, Value};

use crate::kern::{
    Anfrage, Aus, Betrachter, Kern, MAX_GROESSE, MIN_SPALTEN, MIN_ZEILEN, Nachweis, Person, Route,
    Sitzung, Worker,
};
use crate::regel::{self, Regel};
use crate::wert::wahr;

/// The entries of an object that may be absent or `null`.
fn eintraege<'a>(
    anfang: &'a Map<String, Value>,
    schluessel: &str,
) -> Result<Vec<(&'a String, &'a Value)>, String> {
    match anfang.get(schluessel) {
        None | Some(Value::Null) => Ok(Vec::new()),
        Some(Value::Object(m)) => Ok(m.iter().collect()),
        Some(_) => Err(format!("{schluessel} ist kein Objekt")),
    }
}

/// The items of an array that may be absent or `null`.
fn liste<'a>(anfang: &'a Map<String, Value>, schluessel: &str) -> Result<&'a [Value], String> {
    match anfang.get(schluessel) {
        None | Some(Value::Null) => Ok(&[]),
        Some(Value::Array(a)) => Ok(a),
        Some(_) => Err(format!("{schluessel} ist keine Liste")),
    }
}

/// An object of the case.
fn objekt<'a>(wert: &'a Value, wo: &str) -> Result<&'a Map<String, Value>, String> {
    wert.as_object()
        .ok_or_else(|| format!("{wo} ist kein Objekt"))
}

/// A text, or `None` for an absent or `null` field; any other type is an error.
fn text_oder_leer(objekt: &Map<String, Value>, schluessel: &str) -> Result<Option<String>, String> {
    match objekt.get(schluessel) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s)) => Ok(Some(s.clone())),
        Some(_) => Err(format!("{schluessel} ist kein Text")),
    }
}

/// A whole number of the case: the default when absent, an error for another type.
fn ganz(objekt: &Map<String, Value>, schluessel: &str, vorgabe: i64) -> Result<i64, String> {
    objekt.get(schluessel).map_or(Ok(vorgabe), |v| {
        v.as_i64()
            .ok_or_else(|| format!("{schluessel} ist keine ganze Zahl"))
    })
}

fn personen(anfang: &Map<String, Value>) -> Result<BTreeMap<String, Person>, String> {
    let mut aus = BTreeMap::new();
    for (pid, p) in eintraege(anfang, "personen")? {
        let p = objekt(p, "person")?;
        aus.insert(
            pid.clone(),
            Person {
                admin: p.get("rolle").and_then(Value::as_str).unwrap_or("nutzer") == "admin",
                mensch: p.get("art").is_none_or(|a| a == "mensch"),
            },
        );
    }
    Ok(aus)
}

fn nachweise(anfang: &Map<String, Value>) -> Result<BTreeMap<String, Nachweis>, String> {
    let mut aus = BTreeMap::new();
    for (nid, n) in eintraege(anfang, "nachweise")? {
        let person = text_oder_leer(objekt(n, "nachweis")?, "person")?;
        aus.insert(
            nid.clone(),
            Nachweis {
                person,
                benutzt: false,
            },
        );
    }
    Ok(aus)
}

fn routen(anfang: &Map<String, Value>) -> Result<Vec<Route>, String> {
    liste(anfang, "routen")?
        .iter()
        .map(|r| {
            let r = objekt(r, "route")?;
            Ok(Route {
                name: r.get("name").cloned().ok_or("route ohne name")?,
                status: r
                    .get("status")
                    .cloned()
                    .unwrap_or_else(|| Value::from("frei")),
                grund: r.get("grund").cloned().unwrap_or(Value::Null),
                zone: r.get("zone").cloned().unwrap_or(Value::Null),
            })
        })
        .collect()
}

fn regeln(anfang: &Map<String, Value>) -> Result<BTreeMap<String, Regel>, String> {
    let mut beschreibungen: BTreeMap<String, Map<String, Value>> = regel::eingebaut()
        .into_iter()
        .map(|(k, v)| (k, v.as_object().cloned().unwrap_or_default()))
        .collect();
    for (k, eigen) in eintraege(anfang, "regeln")? {
        beschreibungen
            .entry(k.clone())
            .or_default()
            .extend(objekt(eigen, "regel")?.clone());
    }
    let mut aus = BTreeMap::new();
    for (k, beschreibung) in &beschreibungen {
        aus.insert(
            k.clone(),
            regel::lesen(beschreibung).map_err(|e| format!("regel {k}: {e}"))?,
        );
    }
    for (k, zahl) in [
        ("osc52_schreiben", false),
        ("osc52_grenze", true),
        ("bildrate_max", true),
        ("rueckstau_max", true),
    ] {
        if aus.get(k).is_none_or(|r| r.zahl != zahl) {
            return Err(format!("regel {k} hat die falsche Art"));
        }
    }
    Ok(aus)
}

fn sitzung(s: &Map<String, Value>) -> Result<Sitzung, String> {
    let id = s
        .get("id")
        .and_then(Value::as_str)
        .ok_or("sitzung ohne id")?
        .to_owned();
    let (spalten, zeilen) = match s.get("groesse") {
        None => (80, 24),
        Some(Value::Array(g)) => match g.as_slice() {
            [a, b] => (
                a.as_i64().ok_or("groesse ist keine ganze Zahl")?,
                b.as_i64().ok_or("groesse ist keine ganze Zahl")?,
            ),
            _ => return Err("groesse hat nicht zwei Werte".to_owned()),
        },
        Some(_) => return Err("groesse ist keine Liste".to_owned()),
    };
    if !(MIN_SPALTEN..=MAX_GROESSE).contains(&spalten)
        || !(MIN_ZEILEN..=MAX_GROESSE).contains(&zeilen)
    {
        return Err(format!(
            "groesse von {id} liegt nicht in {MIN_SPALTEN} bis {MAX_GROESSE} Spalten und {MIN_ZEILEN} bis {MAX_GROESSE} Zeilen"
        ));
    }
    let mut text: Vec<String> = match s.get("zeilen") {
        None | Some(Value::Null) => Vec::new(),
        Some(Value::Array(a)) => a
            .iter()
            .map(|z| z.as_str().map(str::to_owned))
            .collect::<Option<_>>()
            .ok_or("zeilen sind kein Text")?,
        Some(_) => return Err("zeilen ist keine Liste".to_owned()),
    };
    text.resize(usize::try_from(zeilen).unwrap_or(0), String::new());
    // `cursor` is optional; a field of the wrong form or outside the size, `null` included, is an invalid case (R15).
    let cursor = match s.get("cursor") {
        None => None,
        Some(c) => Some(
            Kern::cursor_pruefen(c, (spalten, zeilen))
                .map_err(|_| format!("cursor von {id} ist ungueltig"))?,
        ),
    };
    Ok(Sitzung {
        id,
        titel: s.get("titel").cloned().unwrap_or_else(|| Value::from("")),
        route: s.get("route").cloned().unwrap_or(Value::Null),
        kundenraum: text_oder_leer(s, "kundenraum")?,
        person: text_oder_leer(s, "person")?,
        groesse: (spalten, zeilen),
        beendet: false,
        code: None,
        bracketed_paste: s.get("bracketed_paste").is_some_and(wahr),
        zeilen: text,
        stand: ganz(s, "stand", 0)?,
        schmutzig: BTreeSet::new(),
        letzter_versand: None,
        faellig: None,
        sync: None,
        invers: s.get("invers").is_some_and(wahr),
        cursor,
        letzte_eingabe: None,
        dialog: None,
    })
}

impl Kern {
    /// Builds the core from the start of a case. `grenze` limits the bytes of all messages.
    ///
    /// # Errors
    ///
    /// A text that says which part of the start is malformed.
    pub(crate) fn neu(anfang: &Map<String, Value>, grenze: usize) -> Result<Self, String> {
        let mut sitzungen = BTreeMap::new();
        let mut reihenfolge = Vec::new();
        for s in liste(anfang, "sitzungen")? {
            let s = sitzung(objekt(s, "sitzung")?)?;
            reihenfolge.push(s.id.clone());
            sitzungen.insert(s.id.clone(), s);
        }
        let mut worker = BTreeMap::new();
        for w in liste(anfang, "worker")? {
            let w = objekt(w, "worker")?;
            let id = w
                .get("id")
                .and_then(Value::as_str)
                .ok_or("worker ohne id")?;
            worker.insert(
                id.to_owned(),
                Worker {
                    sitzung: text_oder_leer(w, "sitzung")?,
                    stand: None,
                },
            );
        }
        Ok(Self {
            t: 0,
            idx: 0,
            timer: BTreeMap::new(),
            folge: 0,
            aus: Aus::neu(grenze),
            personen: personen(anfang)?,
            nachweise: nachweise(anfang)?,
            routen: routen(anfang)?,
            regeln: regeln(anfang)?,
            sitzungen,
            reihenfolge,
            worker,
            anfragen: BTreeMap::<String, Anfrage>::new(),
            anfrage_folge: Vec::new(),
            betrachter: Vec::<(String, Betrachter)>::new(),
        })
    }
}
