//! The subcommand `datentor person` (partial contract `person/VERTRAG.md`, section 6): reads one case with read classes and
//! writes one answer per request. Limits, exit codes, depth and duplicate keys are those of contract 3 (`programm.rs`).

use datentor::Einheit;
use datentor::person::{
    Leseeintrag, Person, PersonAntwort, PersonEbene, Personen, darf_person, lesen_person,
};
use serde_json::{Map, Value, json};

use crate::programm::{
    anfragen_liste, fall_lesen, felder, objekt, objekte, pflicht, text, text_oder_null, texte,
    vorgabe_lesen,
};

/// Checks one case of the person format and returns the output, or the reason the case is invalid.
pub fn verarbeite(eingabe: &[u8]) -> Result<String, String> {
    let fall = fall_lesen(eingabe)?;
    let fall = fall.as_object().ok_or("case is not an object")?;
    felder(
        fall,
        &[
            "name",
            "regeln",
            "beschreibung",
            "vorgabe",
            "personen",
            "anfragen",
        ],
        "case",
    )?;
    let vorgabe = vorgabe_lesen(fall.get("vorgabe").ok_or("vorgabe is missing")?)?;
    let personen = personen_lesen(&vorgabe, fall.get("personen").ok_or("personen is missing")?)?;
    let anfragen = anfragen_liste(fall)?
        .iter()
        .map(anfrage_lesen)
        .collect::<Result<Vec<_>, _>>()?;
    let antworten: Vec<Value> = anfragen
        .iter()
        .map(|a| {
            let gruppen: Vec<&str> = a.gruppen.iter().map(String::as_str).collect();
            let person = Person {
                nutzer: a.nutzer.as_deref(),
                gruppen: &gruppen,
            };
            let einheit = Einheit {
                klasse: a.klasse.as_deref(),
            };
            let antwort = if a.lesen {
                lesen_person(&vorgabe, &personen, &einheit, a.kunde.as_deref(), &person)
            } else {
                darf_person(
                    &vorgabe,
                    &personen,
                    &einheit,
                    a.route.as_deref(),
                    a.kunde.as_deref(),
                    &person,
                )
            };
            antwort_person_json(&antwort, a.route.as_deref())
        })
        .collect();
    Ok(json!({ "antworten": antworten }).to_string())
}

/// Reads `personen` (section 3) and checks it through `Personen::neu`.
fn personen_lesen(vorgabe: &datentor::Vorgabe, wert: &Value) -> Result<Personen, String> {
    let m = objekt(wert, "personen")?;
    felder(m, &["leseklassen"], "personen")?;
    let eintraege = objekte(pflicht(m, "leseklassen", "personen")?, "leseklassen")?
        .into_iter()
        .map(eintrag_lesen)
        .collect::<Result<Vec<_>, _>>()?;
    Personen::neu(vorgabe, eintraege).map_err(|e| format!("personen: {e}"))
}

/// Reads one entry: all three fields are required, each a text.
fn eintrag_lesen(e: &Map<String, Value>) -> Result<Leseeintrag, String> {
    felder(e, &["ebene", "wer", "klasse"], "entry")?;
    let ebene = match text(pflicht(e, "ebene", "entry")?, "ebene")?.as_str() {
        "nutzer" => PersonEbene::Nutzer,
        "gruppe" => PersonEbene::Gruppe,
        _ => return Err("entry: ebene is neither nutzer nor gruppe".to_owned()),
    };
    Ok(Leseeintrag {
        ebene,
        wer: text(pflicht(e, "wer", "entry")?, "wer of an entry")?,
        klasse: text(pflicht(e, "klasse", "entry")?, "klasse of an entry")?,
    })
}

/// One request of the person format.
struct Anfrage {
    /// The class of the unit as in contract 3; a class that is not a text becomes the empty text, which is no valid class.
    klasse: Option<String>,
    route: Option<String>,
    kunde: Option<String>,
    nutzer: Option<String>,
    gruppen: Vec<String>,
    /// True: the person reads herself, without a route (P7).
    lesen: bool,
}

/// Reads one request with its context (section 6).
fn anfrage_lesen(wert: &Value) -> Result<Anfrage, String> {
    let a = objekt(wert, "request")?;
    felder(
        a,
        &["einheit", "route", "kunde", "kontext", "lesen"],
        "request",
    )?;
    let lesen = match a.get("lesen") {
        None => false,
        Some(Value::Bool(true)) => true,
        Some(_) => return Err("request: lesen is not true".to_owned()),
    };
    if lesen && a.contains_key("route") {
        return Err("request: a read without a route names a route".to_owned());
    }
    let e = objekt(pflicht(a, "einheit", "request")?, "einheit")?;
    felder(e, &["klasse"], "einheit")?;
    let klasse = match e.get("klasse") {
        None | Some(Value::Null) => None,
        Some(Value::String(s)) => Some(s.clone()),
        Some(_) => Some(String::new()),
    };
    // A missing context is no person (P1); `kontext: null` is invalid.
    let leer = Map::new();
    let k = match a.get("kontext") {
        None => &leer,
        Some(k) => objekt(k, "kontext")?,
    };
    felder(k, &["nutzer", "gruppen"], "kontext")?;
    Ok(Anfrage {
        klasse,
        route: text_oder_null(a.get("route"), "route")?,
        kunde: text_oder_null(a.get("kunde"), "kunde")?,
        nutzer: text_oder_null(k.get("nutzer"), "nutzer")?,
        gruppen: k
            .get("gruppen")
            .map_or_else(|| Ok(Vec::new()), |g| texte(g, "gruppen"))?,
        lesen,
    })
}

/// The answer as JSON: the fields of contract 3 and `leseklasse`.
fn antwort_person_json(antwort: &PersonAntwort, route: Option<&str>) -> Value {
    json!({
        "darf": antwort.darf(),
        "grund": antwort.grund.code(),
        "klasse": antwort.klasse,
        "meldung": antwort.meldung(route),
        "leseklasse": antwort.leseklasse,
    })
}
