//! The subcommand `datentor zonen` (partial contract `zonen/VERTRAG.md`, section 7): reads one case with model zones and
//! writes one answer per request. Limits, exit codes, depth and duplicate keys are those of contract 3 (`programm.rs`).

use datentor::Einheit;
use datentor::zonen::{
    Ausnahme, Bedingung, Ebene, Klassensatz, Kontext, ProjektFreigabe, Regel, Zone, Zonen,
    ZonenAntwort, darf_zonen,
};
use serde_json::{Map, Value, json};

use crate::programm::{
    anfragen_liste, fall_lesen, felder, objekt, objekte, pflicht, text, text_oder_null, texte,
    vorgabe_lesen,
};

/// Checks one case of the zone format and returns the output, or the reason the case is invalid.
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
            "modellzonen",
            "anfragen",
        ],
        "case",
    )?;
    let vorgabe = vorgabe_lesen(fall.get("vorgabe").ok_or("vorgabe is missing")?)?;
    let zonen = zonen_lesen(
        &vorgabe,
        fall.get("modellzonen").ok_or("modellzonen is missing")?,
    )?;
    let anfragen = anfragen_liste(fall)?
        .iter()
        .map(anfrage_lesen)
        .collect::<Result<Vec<_>, _>>()?;
    let antworten: Vec<Value> = anfragen
        .iter()
        .map(|a| {
            let gruppen: Vec<&str> = a.gruppen.iter().map(String::as_str).collect();
            let kontext = Kontext {
                nutzer: a.nutzer.as_deref(),
                gruppen: &gruppen,
                jetzt: a.jetzt,
                grund: a.grund.as_deref(),
            };
            let einheit = a.einheit.as_ref().map(|k| Einheit {
                klasse: k.0.as_deref(),
            });
            let antwort = darf_zonen(
                &vorgabe,
                &zonen,
                einheit.as_ref(),
                a.route.as_deref(),
                a.kunde.as_deref(),
                a.projekt.as_deref(),
                &kontext,
            );
            antwort_zonen_json(&antwort, a.route.as_deref())
        })
        .collect();
    Ok(json!({ "antworten": antworten }).to_string())
}

/// An optional list of objects: missing is empty.
fn liste<'a>(m: &'a Map<String, Value>, feld: &str) -> Result<Vec<&'a Map<String, Value>>, String> {
    m.get(feld)
        .map_or_else(|| Ok(Vec::new()), |x| objekte(x, feld))
}

/// An optional list of texts: missing is empty.
fn textliste(m: &Map<String, Value>, feld: &str) -> Result<Vec<String>, String> {
    m.get(feld)
        .map_or_else(|| Ok(Vec::new()), |x| texte(x, feld))
}

/// A whole number from 0 to 2^64 - 1.
fn zahl(wert: &Value, wo: &str) -> Result<u64, String> {
    wert.as_u64()
        .ok_or_else(|| format!("{wo} is not a whole number from 0 to 2^64 - 1"))
}

/// A class set.
fn klassensatz(wert: &Value, wo: &str) -> Result<Klassensatz, String> {
    match text(wert, wo)?.as_str() {
        "alle" => Ok(Klassensatz::Alle),
        "niedrigste" => Ok(Klassensatz::Niedrigste),
        "keine" => Ok(Klassensatz::Keine),
        _ => Err(format!("{wo} is not a class set")),
    }
}

/// Reads `modellzonen` (section 3) and checks it through `Zonen::neu`.
fn zonen_lesen(vorgabe: &datentor::Vorgabe, wert: &Value) -> Result<Zonen, String> {
    let m = objekt(wert, "modellzonen")?;
    felder(
        m,
        &["an", "zonen", "freigaben", "ausnahmen", "regeln"],
        "modellzonen",
    )?;
    let an = pflicht(m, "an", "modellzonen")?
        .as_bool()
        .ok_or("modellzonen: an is neither true nor false")?;
    let zonen = objekte(pflicht(m, "zonen", "modellzonen")?, "zonen")?
        .into_iter()
        .map(zone_lesen)
        .collect::<Result<Vec<_>, _>>()?;
    let freigaben = liste(m, "freigaben")?
        .into_iter()
        .map(|f| {
            felder(f, &["route", "kunde", "projekt"], "freigabe")?;
            Ok(ProjektFreigabe {
                route: text(pflicht(f, "route", "freigabe")?, "route of a release")?,
                kunde: text(pflicht(f, "kunde", "freigabe")?, "kunde of a release")?,
                projekt: text(pflicht(f, "projekt", "freigabe")?, "projekt of a release")?,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    let ausnahmen = liste(m, "ausnahmen")?
        .into_iter()
        .map(ausnahme_lesen)
        .collect::<Result<Vec<_>, _>>()?;
    let regeln = liste(m, "regeln")?
        .into_iter()
        .map(regel_lesen)
        .collect::<Result<Vec<_>, _>>()?;
    Zonen::neu(vorgabe, an, zonen, freigaben, ausnahmen, regeln)
        .map_err(|e| format!("modellzonen: {e}"))
}

/// Reads one zone; a missing route list is empty.
fn zone_lesen(z: &Map<String, Value>) -> Result<Zone, String> {
    felder(z, &["schluessel", "klassen", "bedingung", "routen"], "zone")?;
    let bedingung = match text(pflicht(z, "bedingung", "zone")?, "bedingung")?.as_str() {
        "immer" => Bedingung::Immer,
        "nachrangig" => Bedingung::Nachrangig,
        "freigabe" => Bedingung::Freigabe,
        "gesperrt" => Bedingung::Gesperrt,
        _ => return Err("zone: bedingung has an unknown value".to_owned()),
    };
    Ok(Zone {
        schluessel: text(pflicht(z, "schluessel", "zone")?, "schluessel of a zone")?,
        klassen: klassensatz(pflicht(z, "klassen", "zone")?, "klassen of a zone")?,
        bedingung,
        routen: textliste(z, "routen")?,
    })
}

/// Reads one evaluation exception; missing lists are empty.
fn ausnahme_lesen(a: &Map<String, Value>) -> Result<Ausnahme, String> {
    felder(
        a,
        &["zone", "nutzer", "gruppen", "gesetzt_von", "zweck", "bis"],
        "ausnahme",
    )?;
    Ok(Ausnahme {
        zone: text(pflicht(a, "zone", "ausnahme")?, "zone of an exception")?,
        nutzer: textliste(a, "nutzer")?,
        gruppen: textliste(a, "gruppen")?,
        gesetzt_von: text(pflicht(a, "gesetzt_von", "ausnahme")?, "gesetzt_von")?,
        zweck: text(pflicht(a, "zweck", "ausnahme")?, "zweck")?,
        bis: zahl(pflicht(a, "bis", "ausnahme")?, "bis")?,
    })
}

/// Reads one rule: `gesperrt` may only be `true`; at least one of `klassen` and `gesperrt` is checked by `Zonen::neu`.
fn regel_lesen(r: &Map<String, Value>) -> Result<Regel, String> {
    felder(r, &["ebene", "wer", "zone", "klassen", "gesperrt"], "regel")?;
    let ebene = match text(pflicht(r, "ebene", "regel")?, "ebene")?.as_str() {
        "gruppe" => Ebene::Gruppe,
        "nutzer" => Ebene::Nutzer,
        _ => return Err("regel: ebene is neither gruppe nor nutzer".to_owned()),
    };
    let gesperrt = match r.get("gesperrt") {
        None => false,
        Some(Value::Bool(true)) => true,
        Some(_) => return Err("regel: gesperrt is not true".to_owned()),
    };
    Ok(Regel {
        ebene,
        wer: text(pflicht(r, "wer", "regel")?, "wer of a rule")?,
        zone: text(pflicht(r, "zone", "regel")?, "zone of a rule")?,
        klassen: r
            .get("klassen")
            .map(|k| klassensatz(k, "klassen of a rule"))
            .transpose()?,
        gesperrt,
    })
}

/// The class field of a unit: `None` if it is missing or `null`.
struct Klasse(Option<String>);

/// One request of the zone format.
struct Anfrage {
    /// `None`: no unit (the run check). `Some(klasse)`: the class of the unit as in contract 3; a class that is not a
    /// text becomes the empty text, which is no valid class.
    einheit: Option<Klasse>,
    route: Option<String>,
    kunde: Option<String>,
    projekt: Option<String>,
    nutzer: Option<String>,
    gruppen: Vec<String>,
    jetzt: Option<u64>,
    grund: Option<String>,
}

/// Reads one request with its context (section 6).
fn anfrage_lesen(wert: &Value) -> Result<Anfrage, String> {
    let a = objekt(wert, "request")?;
    felder(
        a,
        &["einheit", "route", "kunde", "projekt", "kontext"],
        "request",
    )?;
    let einheit = match pflicht(a, "einheit", "request")? {
        Value::Null => None,
        e => {
            let e = objekt(e, "einheit")?;
            felder(e, &["klasse"], "einheit")?;
            Some(Klasse(match e.get("klasse") {
                None | Some(Value::Null) => None,
                Some(Value::String(s)) => Some(s.clone()),
                Some(_) => Some(String::new()),
            }))
        }
    };
    let leer = Map::new();
    let k = match a.get("kontext") {
        None => &leer,
        Some(k) => objekt(k, "kontext")?,
    };
    felder(k, &["nutzer", "gruppen", "jetzt", "grund"], "kontext")?;
    let jetzt = match k.get("jetzt") {
        None | Some(Value::Null) => None,
        Some(j) => Some(zahl(j, "jetzt")?),
    };
    Ok(Anfrage {
        einheit,
        route: text_oder_null(a.get("route"), "route")?,
        kunde: text_oder_null(a.get("kunde"), "kunde")?,
        projekt: text_oder_null(a.get("projekt"), "projekt")?,
        nutzer: text_oder_null(k.get("nutzer"), "nutzer")?,
        gruppen: textliste(k, "gruppen")?,
        jetzt,
        grund: text_oder_null(k.get("grund"), "grund")?,
    })
}

/// The answer as JSON: the fields of contract 3 and `protokoll`.
fn antwort_zonen_json(antwort: &ZonenAntwort, route: Option<&str>) -> Value {
    let protokoll = antwort.protokoll.as_ref().map(|p| {
        json!({
            "zone": p.zone,
            "grund": p.grund,
            "ausnahme": p.ausnahme.as_ref().map(|a| json!({
                "zone": a.zone,
                "nutzer": a.nutzer,
                "gruppen": a.gruppen,
                "gesetzt_von": a.gesetzt_von,
                "zweck": a.zweck,
                "bis": a.bis,
            })),
        })
    });
    json!({
        "darf": antwort.darf(),
        "grund": antwort.grund.code(),
        "klasse": antwort.klasse,
        "meldung": antwort.meldung(route),
        "protokoll": protokoll,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fall(jetzt: &str, bis: &str) -> String {
        format!(
            r#"{{"vorgabe":{{"klassen":["a"],"routen":[{{"schluessel":"r","art":"lokal"}}],"kunden":[]}},"modellzonen":{{"an":true,"zonen":[{{"schluessel":"z","klassen":"alle","bedingung":"gesperrt","routen":["r"]}}],"ausnahmen":[{{"zone":"z","nutzer":["n"],"gesetzt_von":"a","zweck":"x","bis":{bis}}}]}},"anfragen":[{{"einheit":null,"route":"r","kontext":{{"nutzer":"n","jetzt":{jetzt}}}}}]}}"#
        )
    }

    #[test]
    fn zahlen_nur_in_ziffernform() {
        for ok in ["0", "7", "18446744073709551615"] {
            assert!(verarbeite(fall(ok, "1").as_bytes()).is_ok(), "jetzt {ok}");
            assert!(verarbeite(fall("0", ok).as_bytes()).is_ok(), "bis {ok}");
        }
        assert!(verarbeite(fall("null", "1").as_bytes()).is_ok());
        for falsch in [
            "-0",
            "-1",
            "1e9",
            "1E2",
            "1.0",
            "0.0",
            "+1",
            "18446744073709551616",
            "\"1\"",
            "true",
        ] {
            assert!(
                verarbeite(fall(falsch, "1").as_bytes()).is_err(),
                "jetzt {falsch}"
            );
            assert!(
                verarbeite(fall("0", falsch).as_bytes()).is_err(),
                "bis {falsch}"
            );
        }
        assert!(verarbeite(fall("0", "null").as_bytes()).is_err());
    }

    #[test]
    fn frist_aus_der_eingabe_wirkt() {
        let grund = |jetzt: &str, bis: &str| {
            let aus = verarbeite(fall(jetzt, bis).as_bytes()).expect("valid");
            let wert: Value = serde_json::from_str(&aus).expect("JSON");
            wert["antworten"][0]["grund"].as_str().map(str::to_owned)
        };
        assert_eq!(grund("4", "5").as_deref(), Some("frei"));
        assert_eq!(grund("5", "5").as_deref(), Some("zone-gesperrt"));
        assert_eq!(
            grund("18446744073709551614", "18446744073709551615").as_deref(),
            Some("frei")
        );
    }
}
