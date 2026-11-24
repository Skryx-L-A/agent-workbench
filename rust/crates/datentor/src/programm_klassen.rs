//! The subcommand `datentor klassen` (partial contract `klassen/VERTRAG.md`, section 6): reads one case with the options
//! `tiefe` and writes one answer per request. Limits, exit codes, depth and duplicate keys are those of contract 3.

use datentor::klassen::{
    Angabe, Befund, Ebene, Gruppe, Inhalt, Klassenergebnis, Lage, Modell, NutzerSenkung, Tiefe,
    VORGABE_ARTEN, ganze_klasse, wirksame_klasse,
};
use datentor::{Vorgabe, darf};
use serde_json::{Map, Value, json};

use crate::programm::{
    anfragen_liste, antwort_json, fall_lesen, felder, objekt, objekte, pflicht, text,
    text_oder_null, texte, vorgabe_lesen,
};

/// The most parts of one unit.
const TEILE_MAX: usize = 64;
/// The fields of the place of the whole unit.
const LAGE: [&str; 4] = ["geerbt", "gruppe", "senkung_nutzer", "bestaetigt"];
/// The fields of the content of a unit or a part.
const INHALT: [&str; 5] = ["klasse", "kunde", "art", "befund", "modell"];

/// Checks one case of the class format and returns the output, or the reason the case is invalid.
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
            "tiefe",
            "anfragen",
        ],
        "case",
    )?;
    let vorgabe = vorgabe_lesen(fall.get("vorgabe").ok_or("vorgabe is missing")?)?;
    let tiefe = match fall.get("tiefe") {
        None => Tiefe::default(),
        Some(t) => tiefe_lesen(t)?,
    };
    let anfragen = anfragen_liste(fall)?
        .iter()
        .map(anfrage_lesen)
        .collect::<Result<Vec<_>, _>>()?;
    let antworten: Vec<Value> = anfragen
        .iter()
        .map(|a| antwort(&vorgabe, &tiefe, a))
        .collect();
    Ok(json!({ "antworten": antworten }).to_string())
}

/// A boolean.
fn wahr(wert: &Value, wo: &str) -> Result<bool, String> {
    wert.as_bool()
        .ok_or_else(|| format!("{wo} is neither true nor false"))
}

/// Reads `tiefe` (section 2) and checks it through `Tiefe::neu`.
fn tiefe_lesen(wert: &Value) -> Result<Tiefe, String> {
    let t = objekt(wert, "tiefe")?;
    felder(t, &["aufteilen", "herabstufen"], "tiefe")?;
    let aufteilen = t
        .get("aufteilen")
        .map_or(Ok(true), |x| wahr(x, "aufteilen"))?;
    let leer = Map::new();
    let h = match t.get("herabstufen") {
        None => &leer,
        Some(h) => objekt(h, "herabstufen")?,
    };
    felder(h, &["an", "arten", "gruppen"], "herabstufen")?;
    let an = h.get("an").map_or(Ok(false), |x| wahr(x, "an"))?;
    let arten = match h.get("arten") {
        None => VORGABE_ARTEN.iter().map(|a| (*a).to_owned()).collect(),
        Some(a) => texte(a, "arten")?,
    };
    let gruppen = match h.get("gruppen") {
        None => Vec::new(),
        Some(g) => objekte(g, "gruppen")?
            .into_iter()
            .map(|g| {
                felder(g, &["schluessel", "streicht"], "gruppe")?;
                Ok(Gruppe {
                    schluessel: text(pflicht(g, "schluessel", "gruppe")?, "schluessel of a group")?,
                    streicht: g
                        .get("streicht")
                        .map_or_else(|| Ok(Vec::new()), |s| texte(s, "streicht"))?,
                })
            })
            .collect::<Result<Vec<_>, String>>()?,
    };
    Tiefe::neu(aufteilen, an, arten, gruppen).map_err(|e| format!("tiefe: {e}"))
}

/// A class field: any JSON value.
fn angabe(wert: Option<&Value>) -> Angabe<'_> {
    match wert {
        None | Some(Value::Null) => Angabe::Keine,
        Some(Value::String(s)) => Angabe::Text(s),
        Some(_) => Angabe::KeinText,
    }
}

/// An optional text borrowed from the case: missing and `null` are none.
fn text_ref<'a>(wert: Option<&'a Value>, wo: &str) -> Result<Option<&'a str>, String> {
    text_oder_null(wert, wo)?;
    Ok(wert.and_then(Value::as_str))
}

/// The content of a unit without parts, or one content per part.
enum Inhalte<'a> {
    Ganz(Inhalt<'a>),
    Teile(Vec<Inhalt<'a>>),
}

/// One request: the unit with its place, its content or its parts, and the route.
struct Anfrage<'a> {
    lage: Lage<'a>,
    inhalt: Inhalte<'a>,
    route: Option<String>,
}

/// Reads one request (sections 3 and 6).
fn anfrage_lesen(wert: &Value) -> Result<Anfrage<'_>, String> {
    let a = objekt(wert, "request")?;
    felder(a, &["einheit", "route"], "request")?;
    let e = objekt(pflicht(a, "einheit", "request")?, "einheit")?;
    let lage = lage_lesen(e)?;
    let inhalt = match e.get("teile") {
        None => {
            let erlaubt: Vec<&str> = LAGE.iter().chain(&INHALT).copied().collect();
            felder(e, &erlaubt, "einheit")?;
            Inhalte::Ganz(inhalt_lesen(e)?)
        }
        Some(t) => {
            let erlaubt: Vec<&str> = LAGE.iter().copied().chain(["teile"]).collect();
            felder(e, &erlaubt, "einheit with teile")?;
            let teile = objekte(t, "teile")?;
            if teile.is_empty() || teile.len() > TEILE_MAX {
                return Err(format!(
                    "teile has {} parts, not 1 to {TEILE_MAX}",
                    teile.len()
                ));
            }
            Inhalte::Teile(
                teile
                    .into_iter()
                    .map(|x| {
                        felder(x, &INHALT, "teil")?;
                        inhalt_lesen(x)
                    })
                    .collect::<Result<Vec<_>, _>>()?,
            )
        }
    };
    Ok(Anfrage {
        lage,
        inhalt,
        route: text_oder_null(a.get("route"), "route")?,
    })
}

/// Reads the place of the whole unit.
fn lage_lesen(e: &Map<String, Value>) -> Result<Lage<'_>, String> {
    let nutzer = match e.get("senkung_nutzer") {
        None => NutzerSenkung::Folgt,
        Some(n) => match n.as_str() {
            Some("folgt") => NutzerSenkung::Folgt,
            Some("aus") => NutzerSenkung::Aus,
            Some("bestaetigung") => NutzerSenkung::Bestaetigung,
            _ => return Err("senkung_nutzer has an unknown value".to_owned()),
        },
    };
    Ok(Lage {
        geerbt: angabe(e.get("geerbt")),
        gruppe: text_ref(e.get("gruppe"), "gruppe")?,
        nutzer,
        bestaetigt: e
            .get("bestaetigt")
            .map_or(Ok(false), |b| wahr(b, "bestaetigt"))?,
    })
}

/// Reads the content of a unit or a part.
fn inhalt_lesen(i: &Map<String, Value>) -> Result<Inhalt<'_>, String> {
    let befund = match i.get("befund") {
        None | Some(Value::Null) => None,
        Some(b) => {
            let b = objekt(b, "befund")?;
            felder(
                b,
                &["geheimnis", "personenbezug", "interner_name"],
                "befund",
            )?;
            let feld = |f: &str| wahr(pflicht(b, f, "befund")?, f);
            Some(Befund {
                geheimnis: feld("geheimnis")?,
                personenbezug: feld("personenbezug")?,
                interner_name: feld("interner_name")?,
            })
        }
    };
    let modell = match i.get("modell") {
        None | Some(Value::Null) => None,
        Some(m) => Some(match m.as_str() {
            Some("frei") => Modell::Frei,
            Some("verhindert") => Modell::Verhindert,
            Some("senken") => Modell::Senken,
            _ => return Err("modell has an unknown value".to_owned()),
        }),
    };
    Ok(Inhalt {
        klasse: angabe(i.get("klasse")),
        kunde: text_ref(i.get("kunde"), "kunde")?,
        art: text_ref(i.get("art"), "art")?,
        befund,
        modell,
    })
}

/// The answer of one request.
fn antwort(v: &Vorgabe, t: &Tiefe, a: &Anfrage<'_>) -> Value {
    let route = a.route.as_deref();
    let inhalte = match &a.inhalt {
        Inhalte::Ganz(i) => return ergebnis_json(v, &wirksame_klasse(v, t, &a.lage, i), route),
        Inhalte::Teile(x) if !t.aufteilen() => {
            return ergebnis_json(v, &ganze_klasse(v, &a.lage, x), route);
        }
        Inhalte::Teile(x) => x,
    };
    let teile: Vec<(bool, Value)> = inhalte
        .iter()
        .map(|i| {
            let e = wirksame_klasse(v, t, &a.lage, i);
            let frei = darf(v, &e.einheit(), route, e.kunde.as_deref()).darf();
            (frei, ergebnis_json(v, &e, route))
        })
        .collect();
    let ausgeliefert: Vec<usize> = teile
        .iter()
        .enumerate()
        .filter_map(|(n, (frei, _))| frei.then_some(n))
        .collect();
    json!({
        "gesperrt": teile.len() - ausgeliefert.len(),
        "ausgeliefert": ausgeliefert,
        "teile": teile.into_iter().map(|(_, x)| x).collect::<Vec<_>>(),
    })
}

/// One result as JSON, with the answer of `darf` (K6).
fn ergebnis_json(v: &Vorgabe, e: &Klassenergebnis, route: Option<&str>) -> Value {
    let darf_antwort = darf(v, &e.einheit(), route, e.kunde.as_deref());
    let protokoll = e.protokoll.as_ref().map(|p| {
        json!({
            "art": p.art,
            "erkannt_durch": p.erkannt_durch,
            "regel_ebene": match p.regel_ebene {
                Ebene::Firma => "firma",
                Ebene::Gruppe => "gruppe",
            },
            "klasse_alt": p.klasse_alt,
            "klasse_neu": p.klasse_neu,
        })
    });
    json!({
        "klasse": e.klasse,
        "herkunft": e.herkunft.code(),
        "senkung": {"grund": e.senkung.code(), "vorschlag": e.vorschlag},
        "protokoll": protokoll,
        "darf": antwort_json(&darf_antwort, route),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const V: &str = r#"{"klassen":["a","b"],"routen":[{"schluessel":"r","art":"lokal","klasse":"b"}],"kunden":[]}"#;

    fn mit_teilen(n: usize) -> String {
        let teile = vec!["{}"; n].join(",");
        format!(
            r#"{{"vorgabe":{V},"anfragen":[{{"einheit":{{"geerbt":"a","teile":[{teile}]}},"route":"r"}}]}}"#
        )
    }

    #[test]
    fn hoechstens_64_teile() {
        let aus = verarbeite(mit_teilen(64).as_bytes()).expect("64 parts are valid");
        let wert: Value = serde_json::from_str(&aus).expect("JSON");
        assert_eq!(
            wert["antworten"][0]["ausgeliefert"]
                .as_array()
                .map(Vec::len),
            Some(64)
        );
        assert_eq!(wert["antworten"][0]["gesperrt"], 0);
        assert!(verarbeite(mit_teilen(1).as_bytes()).is_ok());
        assert!(verarbeite(mit_teilen(65).as_bytes()).is_err());
        assert!(verarbeite(mit_teilen(0).as_bytes()).is_err());
    }
}
