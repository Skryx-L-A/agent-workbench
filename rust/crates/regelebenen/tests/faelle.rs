//! The outside test against the cases of the contract (`rust/spec/regelebenen/`).
//!
//! - `faelle-setzen/` with `erwartet-setzen/` (15.8): every step `setzen` and `sicht`, compared as a whole with the
//!   expectation, and every code of 15.4 in at least one case.
//! - `faelle-schreiben/` with `erwartet-schreiben/` (15.12.5), the parts of the library: H3 and H4 of the state of each
//!   case against the code of its first step; per step `schreiben` the stage 2 (`setzen` on a copy of the state that
//!   the service loaded) and its line; per step `nachricht` the view. The mapping of accounts to pseudonyms and the
//!   account in place of the pseudonym for an admin (15.13) are the work of the service; this test does them as the
//!   reference does (`lauf_schreiben` in `bau-faelle.py`).

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use regelebenen::katalog::LESEKLASSEN;
use regelebenen::vorgabe::{self, Angabe, Art, Lage, Vorgabefehler};
use regelebenen::{Code, Stand, Werte, setzen, sicht};
use serde_json::{Value, json};

const HELFER_UID: u32 = 990;

fn spec() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../spec/regelebenen")
}

fn lies(p: &Path) -> Value {
    serde_json::from_slice(&std::fs::read(p).expect("read")).expect("json")
}

/// Every case of `ordner` with its expectation in `erwartet`, by name.
fn faelle(ordner: &str, erwartet: &str) -> Vec<(String, Value, Value)> {
    let mut namen: Vec<PathBuf> = std::fs::read_dir(spec().join(ordner))
        .expect("folder")
        .map(|e| e.expect("entry").path())
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .collect();
    namen.sort();
    namen
        .into_iter()
        .map(|p| {
            let name = p.file_name().expect("name").to_string_lossy().into_owned();
            let soll = lies(&spec().join(erwartet).join(&name));
            (name, lies(&p), soll)
        })
        .collect()
}

#[test]
fn faelle_setzen() {
    let alle = faelle("faelle-setzen", "erwartet-setzen");
    assert!(alle.len() >= 35, "{} cases", alle.len());
    let mut gesehen: BTreeSet<String> = BTreeSet::new();
    for (name, fall, soll) in &alle {
        let mut stand = Stand::aus_wert(&fall["stand"]).expect("state");
        let schritte = fall["schritte"].as_array().expect("steps");
        let soll = soll["schritte"].as_array().expect("expected steps");
        assert_eq!(schritte.len(), soll.len(), "{name}");
        for (i, (s, e)) in schritte.iter().zip(soll).enumerate() {
            let ist = if let Some(a) = s.get("setzen") {
                let wer = a["wer"].as_str().expect("wer");
                let p = setzen(
                    &mut stand,
                    wer,
                    &a["bereich"],
                    &a["ebene"],
                    &a["schluessel"],
                    &a["wert"],
                );
                if let Some(c) = p.code {
                    gesehen.insert(c.text().to_owned());
                }
                p.als_wert()
            } else {
                let wer = s["sicht"]["wer"].as_str().expect("wer");
                json!({"einstellungen": sicht(&stand, wer).als_wert()})
            };
            assert_eq!(&ist, e, "{name}, step {i}");
        }
    }
    let alle_codes: BTreeSet<String> = Code::ALLE.iter().map(|c| c.text().to_owned()).collect();
    assert_eq!(gesehen, alle_codes, "every code in at least one case");
}

fn art(t: &Value) -> Art {
    match t.as_str() {
        Some("datei") => Art::Datei,
        Some("ordner") => Art::Ordner,
        Some("verweis") => Art::Verweis,
        _ => Art::Andere,
    }
}

fn angabe(ort: &Value, konten: &BTreeMap<String, u32>) -> Angabe {
    let uid = match ort["eigentuemer"].as_str().expect("owner") {
        "root" => 0,
        "werkbank-vorgabe" => HELFER_UID,
        k => konten[k],
    };
    let modus = u32::from_str_radix(ort["modus"].as_str().expect("mode"), 8).expect("octal");
    Angabe {
        art: art(&ort["art"]),
        uid,
        modus,
    }
}

/// The state of a case as H3 sees it: `eltern` stands for every parent up to `/`.
fn lage(fall: &Value, konten: &BTreeMap<String, u32>) -> Lage {
    Lage {
        eltern: vec![Some(angabe(&fall["eltern"], konten)); 3],
        ordner: Some(angabe(&fall["ordner"], konten)),
        datei: Some(angabe(&fall["firmenvorgabe"], konten)),
        helfer_uid: HELFER_UID,
    }
}

/// The code of a step of the write way: of the service (`code`) or of the helper (`antwort.code`).
fn code_des_schritts(e: &Value) -> Option<&str> {
    e.get("code")
        .or_else(|| e.get("antwort").map(|a| &a["code"]))
        .and_then(Value::as_str)
}

/// Stage 0 of the service (15.12.3): every account at `ebene` `nutzer` to its pseudonym; an account without one and a
/// pseudonym become `wer: null`.
fn in_pseudonyme(wert: &Value, zu: &BTreeMap<String, String>) -> Value {
    let Some(liste) = wert.as_array() else {
        return wert.clone();
    };
    Value::Array(
        liste
            .iter()
            .map(|x| {
                let mut x = x.clone();
                if let Some(o) = x.as_object_mut()
                    && o.get("ebene").and_then(Value::as_str) == Some("nutzer")
                {
                    let p = o
                        .get("wer")
                        .and_then(Value::as_str)
                        .and_then(|k| zu.get(k))
                        .map_or(Value::Null, |p| Value::String(p.clone()));
                    o.insert("wer".to_owned(), p);
                }
                x
            })
            .collect(),
    )
}

fn stand_aus(fall: &Value, geladen: &Value, zu: &BTreeMap<String, String>) -> Stand {
    let bytes = serde_json::to_vec(geladen).expect("bytes");
    let v = vorgabe::laden(&bytes).expect("the loaded default is valid");
    let klassen: Vec<String> =
        serde_json::from_value(fall["firma"]["klassen"].clone()).expect("classes");
    let gruppen: Vec<(String, Vec<String>)> = fall["firma"]["gruppen"]
        .as_object()
        .expect("groups")
        .iter()
        .map(|(g, m)| {
            (
                g.clone(),
                serde_json::from_value(m.clone()).expect("members"),
            )
        })
        .collect();
    v.stand(klassen, gruppen, Werte::default(), |k| zu.get(k).cloned())
        .expect("state")
}

/// The part of a case of the write way that the library decides.
fn schreibfall(name: &str, fall: &Value, soll: &Value) -> usize {
    let konten: BTreeMap<String, u32> =
        serde_json::from_value(fall["konten"].clone()).expect("accounts");
    let zu: BTreeMap<String, String> =
        serde_json::from_value(fall["zuordnung"].clone()).expect("mapping");
    let konto_von: BTreeMap<u32, &String> = konten.iter().map(|(k, u)| (*u, k)).collect();
    let schritte = fall["schritte"].as_array().expect("steps");
    let soll = soll["schritte"].as_array().expect("expected steps");
    assert_eq!(schritte.len(), soll.len(), "{name}");

    // H3 and H4 of the state of the case against the code of the first step.
    let geprueft = vorgabe::h3(&lage(fall, &konten)).and_then(|()| {
        vorgabe::laden(&serde_json::to_vec(&fall["firmenvorgabe"]["inhalt"]).expect("bytes"))
            .map(|_| ())
    });
    match code_des_schritts(&soll[0]) {
        Some("firmenvorgabe.partner_unbekannt" | "firmenvorgabe.partner_eingeschlossen") => {}
        Some("firmenvorgabe.datei_unsicher") => {
            assert_eq!(geprueft, Err(Vorgabefehler::DateiUnsicher), "{name}");
        }
        Some("firmenvorgabe.ladefehler") => {
            assert_eq!(vorgabe::h3(&lage(fall, &konten)), Ok(()), "{name}");
            assert_eq!(geprueft, Err(Vorgabefehler::Ladefehler), "{name}");
        }
        _ => assert_eq!(geprueft, Ok(()), "{name}"),
    }
    if geprueft.is_err() {
        return 0;
    }

    // The steps, with the content the service loaded last.
    let mut geladen = fall["firmenvorgabe"]["inhalt"].clone();
    let mut datei = geladen.clone();
    let mut gesperrt = false;
    let mut gelaufen = 0;
    for (i, (s, e)) in schritte.iter().zip(soll).enumerate() {
        if s.get("neuladen").is_some() {
            geladen = datei.clone();
            gesperrt = false;
        } else if let Some(n) = s.get("nachricht") {
            let stand = stand_aus(fall, &geladen, &zu);
            let uid = u32::try_from(n["partner"].as_u64().expect("uid")).expect("u32");
            let wer = &zu[konto_von[&uid]];
            let admin = stand.ist_admin(wer);
            let mut werte = sicht(&stand, wer).als_wert();
            if admin {
                // 15.13: for an admin the service puts the account in place of each pseudonym.
                let zurueck: BTreeMap<&String, &String> = zu.iter().map(|(k, p)| (p, k)).collect();
                for x in werte[LESEKLASSEN]["wert"].as_array_mut().expect("list") {
                    if x["ebene"] == "nutzer"
                        && let Some(k) = x["wer"].as_str().and_then(|p| zurueck.get(&p.to_owned()))
                    {
                        x["wer"] = Value::String((*k).clone());
                    }
                }
            }
            let ist = json!({"einstellungen": {"art": "Einstellungen", "fassung": 2, "admin": admin, "werte": werte}});
            assert_eq!(&ist, e, "{name}, step {i}");
            gelaufen += 1;
        } else if let Some(a) = s.get("schreiben") {
            let uid = u32::try_from(a["partner"].as_u64().expect("uid")).expect("u32");
            let wer = &zu[konto_von[&uid]];
            let mut stand = stand_aus(fall, &geladen, &zu);
            let wert = in_pseudonyme(&a["wert"], &zu);
            let p = setzen(
                &mut stand,
                wer,
                &json!("admin"),
                &json!("firma"),
                &json!(LESEKLASSEN),
                &wert,
            );
            let soll_code = e["code"].as_str();
            let zeile = match soll_code {
                Some(c) if c.starts_with("firmenvorgabe.") => {
                    assert!(
                        p.code.is_none() || gesperrt,
                        "{name}, step {i}: {:?}",
                        p.code
                    );
                    let c: &'static str = Box::leak(c.to_owned().into_boxed_str());
                    p.zeile.clone().mit_code(c)
                }
                _ => {
                    assert_eq!(p.code.map(Code::text), soll_code, "{name}, step {i}");
                    p.zeile.clone()
                }
            };
            assert_eq!(zeile.als_wert(), e["zeile"], "{name}, step {i}");
            if soll_code.is_none() {
                geladen = e["datei"]["inhalt"].clone();
            }
            gesperrt |= soll_code == Some("firmenvorgabe.zustand_unklar");
            gelaufen += 1;
        }
        if let Some(d) = e.get("datei") {
            datei = d["inhalt"].clone();
        }
    }
    gelaufen
}

#[test]
fn faelle_schreiben() {
    let alle = faelle("faelle-schreiben", "erwartet-schreiben");
    assert!(alle.len() >= 37, "{} cases", alle.len());
    let mut codes: BTreeSet<String> = BTreeSet::new();
    let mut schritte = 0;
    for (name, fall, soll) in &alle {
        schritte += schreibfall(name, fall, soll);
        if let Some(c) = code_des_schritts(&soll["schritte"][0]) {
            codes.insert(c.to_owned());
        }
    }
    for c in ["firmenvorgabe.datei_unsicher", "firmenvorgabe.ladefehler"] {
        assert!(codes.contains(c), "{c} in at least one case");
    }
    assert!(schritte >= 20, "{schritte} steps of the library");
}
