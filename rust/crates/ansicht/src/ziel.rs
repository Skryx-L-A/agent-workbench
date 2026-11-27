//! The target protocol of the view: every name and field the view reads from the core and the service, or sends to them, for the
//! gaps L1 to L11 (`rust/spec/VERTRAEGE.md` section 4), at exactly one place: one read function per message, one builder per event.
//!
//! The names and fields are those of the contracts:
//! - `zustandskern` and `dienst`, section "Nachtrag 26.11.2026: Oberfläche L1, L2, L3, L5, L6, L11" (R-L1, R-L2, R-L6, DI-L1 to DI-L6);
//! - `dienst`, section "Nachtrag 26.11.2026: Ordner lesen und durchsuchen (L9)";
//! - `regelebenen` section 15 (L4) and `start` "Fassung 2" with the message `Startwahl` with efforts (L10).
//!
//! A message in the form before these addenda gives `None` here. By which field the view knows that a gap is closed, and which
//! gaps have no such field, says [`crate::kern::Kern::luecken`].

use serde_json::{Map, Value, json};

fn text(v: &Value, name: &str) -> Option<String> {
    v.get(name)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
}

/// A field that must be a text, possibly empty (`ordner` of the session from `kind` is `""`, DI-L5 point 1).
fn text_auch_leer(v: &Value, name: &str) -> Option<String> {
    v.get(name).and_then(Value::as_str).map(str::to_owned)
}

fn liste<'a>(v: &'a Value, name: &str) -> &'a [Value] {
    v.get(name)
        .and_then(Value::as_array)
        .map_or(&[], Vec::as_slice)
}

fn nummer(v: &Value, name: &str) -> Option<u8> {
    v.get(name)
        .and_then(Value::as_u64)
        .and_then(|n| u8::try_from(n).ok())
}

fn wahr(v: &Value, name: &str) -> bool {
    v.get(name).and_then(Value::as_bool) == Some(true)
}

// ---- L1: workers with their session (`Agenten`, `zustandskern` R-L1 and `dienst` DI-L1) ----

/// One worker of `Agenten`. A worker is a session of its own (DI-L1 point 1): `worker` is the id of that session, `sitzung` the id of
/// its orchestrator.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Agent {
    /// The id of the session of the worker.
    pub worker: String,
    /// The id of the session of its orchestrator.
    pub sitzung: String,
    /// Its role (DI-L1 point 1, default `worker`; shown in the accent colour of the pane head).
    pub rolle: String,
    /// Its model (the text of the start structure); empty in the list of the core (R-L1 point 3).
    pub modell: String,
    /// `laeuft` or `beendet` (DI-L1 point 3); the service knows no other state.
    pub zustand: String,
    /// The number of its raster as in `Sitzungsliste` (DI-L1 point 3); `None` in the list of the core.
    pub nr: Option<u8>,
    /// The exit code once ended, as in `Sitzungsliste`.
    pub code: Option<String>,
}

impl Agent {
    /// Whether the worker runs (the pill, the foot and the session row count these).
    #[must_use]
    pub fn laeuft(&self) -> bool {
        self.zustand == "laeuft"
    }
}

/// One entry of `Agenten` or the message `Agentenzustand`; `None` without the field `sitzung` (the form before R-L1).
#[must_use]
pub fn agent_lesen(v: &Value) -> Option<Agent> {
    let sitzung = text(v, "sitzung")?;
    Some(Agent {
        worker: text(v, "worker").unwrap_or_default(),
        rolle: text(v, "rolle").unwrap_or_else(|| "worker".to_owned()),
        modell: text(v, "modell").unwrap_or_default(),
        zustand: text(v, "zustand").unwrap_or_default(),
        nr: nummer(v, "nr"),
        code: v.get("code").filter(|c| !c.is_null()).map(|c| match c {
            Value::String(s) => s.clone(),
            x => x.to_string(),
        }),
        sitzung,
    })
}

/// The message `Agenten`: `Some` only when the list is not empty and every entry names its session (R-L1 point 1). An empty list is
/// the same before and after R-L1 (DI-L1 point 4) and says nothing.
#[must_use]
pub fn agenten_lesen(n: &Value) -> Option<Vec<Agent>> {
    let a = liste(n, "agenten");
    let gelesen: Vec<Agent> = a.iter().filter_map(agent_lesen).collect();
    (!gelesen.is_empty() && gelesen.len() == a.len()).then_some(gelesen)
}

/// Whether every entry of `Agenten` carries the key `nr`: only the list of the service does (DI-L1 point 3), the one of the core
/// does not (R-L1 point 1). The view then knows the raster of each worker (L2).
#[must_use]
pub fn agenten_mit_raster(n: &Value) -> bool {
    let a = liste(n, "agenten");
    !a.is_empty()
        && a.iter()
            .all(|x| x.as_object().is_some_and(|o| o.contains_key("nr")))
}

// ---- L2: one subscription per session (`zustandskern` R-L2, `dienst` DI-L2) ----

/// `Abo` on or off for a session; a worker pane subscribes the session of its worker. There is no field `nr` (DI-L2 point 1).
#[must_use]
pub fn abo(sitzung: &str, an: bool, groesse: (usize, usize)) -> Value {
    if an {
        json!({"art": "Abo", "sitzung": sitzung, "an": true, "spalten": groesse.0, "zeilen": groesse.1})
    } else {
        json!({"art": "Abo", "sitzung": sitzung, "an": false})
    }
}

/// `Groesse` for one subscription (R-L2 point 2).
#[must_use]
pub fn groesse(sitzung: &str, groesse: (usize, usize)) -> Value {
    json!({"art": "Groesse", "sitzung": sitzung, "spalten": groesse.0, "zeilen": groesse.1})
}

// ---- L3: rename, continue, revive, delete (`dienst` DI-L3) ----

/// `Umbenennen {sitzung, name}` (DI-L3 point 1, B12).
#[must_use]
pub fn umbenennen(sitzung: &str, name: &str) -> Value {
    json!({"art": "Umbenennen", "sitzung": sitzung, "name": name})
}

/// `Fortsetzen {sitzung}` (DI-L3 point 2, B11).
#[must_use]
pub fn fortsetzen(sitzung: &str) -> Value {
    json!({"art": "Fortsetzen", "sitzung": sitzung})
}

/// `Wiederbeleben {sitzung}` (DI-L3 point 3, B17).
#[must_use]
pub fn wiederbeleben(sitzung: &str) -> Value {
    json!({"art": "Wiederbeleben", "sitzung": sitzung})
}

/// `Loeschen {sitzung, bestaetigung}` with the confirmation equal to the id (DI-L3 point 4); the view sends it only after its question
/// (B16).
#[must_use]
pub fn loeschen(sitzung: &str) -> Value {
    json!({"art": "Loeschen", "sitzung": sitzung, "bestaetigung": sitzung})
}

/// The message `Gestartet {sitzung}`: the answer to `Fortsetzen` and `Wiederbeleben` (DI-L3 point 2) and to `Starten` on the local path
/// (DI-L11 point 8).
#[must_use]
pub fn gestartet_lesen(n: &Value) -> Option<String> {
    text(n, "sitzung")
}

// ---- L5: name, folder, project and history of a session in `Sitzungsliste` (`dienst` DI-L5) ----

/// The fields of DI-L5 of one entry of `Sitzungsliste`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Herkunft {
    /// The name; `""` when the session has none.
    pub name: String,
    /// The working folder; `""` for the session from `kind`.
    pub ordner: String,
    /// The project: the last part of `ordner`; `""` when `ordner` is empty or `/`.
    pub projekt: String,
    /// Whether an ended session crashed (DI-L5 point 2); `false` while it runs.
    pub abgestuerzt: bool,
    /// The id of the session this one continues.
    pub fortsetzung_von: Option<String>,
    /// The id of the session that continues this one.
    pub fortgesetzt_als: Option<String>,
}

/// DI-L5 of one entry: `Some` only when `name`, `ordner` and `projekt` are texts (they may be empty).
#[must_use]
pub fn herkunft_lesen(eintrag: &Value) -> Option<Herkunft> {
    Some(Herkunft {
        name: text_auch_leer(eintrag, "name")?,
        ordner: text_auch_leer(eintrag, "ordner")?,
        projekt: text_auch_leer(eintrag, "projekt")?,
        abgestuerzt: wahr(eintrag, "abgestuerzt"),
        fortsetzung_von: text(eintrag, "fortsetzung_von"),
        fortgesetzt_als: text(eintrag, "fortgesetzt_als"),
    })
}

// ---- L4: settings of version 2 (`regelebenen` sections 15.1, 15.5, 15.9, 15.10) ----

/// The catalog of the pilot (`regelebenen` 15.1): key, area (`nutzer` or `admin`) and the values of a choice. The message
/// `Einstellungen` names neither area nor choices; they are fixed here like the harness table in the service.
pub const KATALOG: [(&str, &str, &[&str]); 17] = [
    ("aussehen.thema", "nutzer", &["system", "hell", "dunkel"]),
    ("aussehen.farbsatz", "nutzer", &["grau", "blauschwarz"]),
    ("aussehen.terminal", "nutzer", &["dunkel", "hell", "thema"]),
    ("terminal.schriftgroesse", "nutzer", &[]),
    ("sitzung.harness", "nutzer", &["claude", "copilot", "pi"]),
    ("sitzung.modell", "nutzer", &[]),
    ("sitzung.effort", "nutzer", &[]),
    ("harnesses", "admin", &[]),
    ("modelle", "admin", &[]),
    ("effort.grenzen", "admin", &[]),
    ("optionen.copilot_ohne_rueckfrage", "admin", &[]),
    ("optionen.herabstufen", "admin", &[]),
    ("optionen.lokale_ca", "admin", &[]),
    ("optionen.copilot_telemetrie", "admin", &[]),
    ("optionen.copilot_eingeschraenktes_token", "admin", &[]),
    ("optionen.herstellerzugang", "admin", &[]),
    ("leseklassen", "admin", &[]),
];

/// One setting of `Einstellungen` version 2 (`regelebenen` 15.5).
#[derive(Clone, Debug, PartialEq)]
pub struct Einstellung {
    /// The key.
    pub schluessel: String,
    /// The effective value.
    pub wert: Value,
    /// The level that changed it last: `firma`, `gruppe` or `nutzer`.
    pub herkunft: String,
    /// The group, when the value comes from a group (`herkunft` `gruppe <schluessel>`).
    pub gruppe: Option<String>,
    /// Whether the viewer may change it.
    pub aenderbar: bool,
    /// The reason of a lock: `nur_strenger`, `nur_admin` or `fest`.
    pub grund: Option<String>,
    /// `nutzer` or `admin` (from [`KATALOG`]).
    pub bereich: String,
    /// The values of a choice (from [`KATALOG`]).
    pub wahl: Vec<String>,
}

/// The message `Einstellungen`: `Some((admin, werte))` only in version 2 (`fassung: 2`, `regelebenen` 15.10). A key outside the
/// catalog stays away: `setzen` refuses it (`ebenen.einstellung_unbekannt`).
#[must_use]
pub fn einstellungen_lesen(n: &Value) -> Option<(bool, Vec<Einstellung>)> {
    if n.get("fassung").and_then(Value::as_i64) != Some(2) {
        return None;
    }
    let werte = n.get("werte")?.as_object()?;
    let mut aus = Vec::new();
    for (schluessel, bereich, wahl) in KATALOG {
        let Some(v) = werte.get(schluessel) else {
            continue;
        };
        let (Some(wert), Some(herkunft), Some(aenderbar)) = (
            v.get("wert"),
            text(v, "herkunft"),
            v.get("aenderbar").and_then(Value::as_bool),
        ) else {
            return None;
        };
        let (herkunft, gruppe) = match herkunft.split_once(' ') {
            Some(("gruppe", g)) => ("gruppe".to_owned(), Some(g.to_owned())),
            _ => (herkunft, None),
        };
        aus.push(Einstellung {
            schluessel: schluessel.to_owned(),
            wert: wert.clone(),
            herkunft,
            gruppe,
            aenderbar,
            grund: text(v, "grund"),
            bereich: bereich.to_owned(),
            wahl: wahl.iter().map(|x| (*x).to_owned()).collect(),
        });
    }
    Some((wahr(n, "admin"), aus))
}

/// `EinstellungSetzen {schluessel, wert, bereich, ebene}` (`regelebenen` 15.9); whether it holds decides `setzen`.
#[must_use]
pub fn einstellung_setzen(schluessel: &str, wert: &Value, bereich: &str, ebene: &str) -> Value {
    json!({"art": "EinstellungSetzen", "schluessel": schluessel, "wert": wert, "bereich": bereich, "ebene": ebene})
}

// ---- L6: requests, decisions and the guard history (`zustandskern` R-L6, `dienst` DI-L6) ----

/// The states that end a request (R-L6 point 5).
pub const ENDZUSTAENDE: [&str; 3] = ["erteilt", "abgelehnt", "abgelaufen"];

/// One message `Anfrage` (`zustandskern` section 5): a request changed its state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Anfragestand {
    /// Request id.
    pub id: String,
    /// The session that asks.
    pub sitzung: String,
    /// The tool.
    pub werkzeug: String,
    /// Preview of the command.
    pub vorschau: String,
    /// `offen`, `erteilt`, `abgelehnt` or `abgelaufen`.
    pub zustand: String,
    /// The reason of a refusal (R-L6 point 3); empty otherwise.
    pub begruendung: String,
}

impl Anfragestand {
    /// Whether the request has ended: then the message is one line of the guard history (R-L6 point 5).
    #[must_use]
    pub fn beendet(&self) -> bool {
        ENDZUSTAENDE.contains(&self.zustand.as_str())
    }
}

/// The message `Anfrage`; `None` without `id` or `zustand`.
#[must_use]
pub fn anfrage_lesen(n: &Value) -> Option<Anfragestand> {
    Some(Anfragestand {
        id: text(n, "id")?,
        zustand: text(n, "zustand")?,
        sitzung: text(n, "sitzung").unwrap_or_default(),
        werkzeug: text(n, "werkzeug").unwrap_or_default(),
        vorschau: text(n, "vorschau").unwrap_or_default(),
        begruendung: text(n, "begruendung").unwrap_or_default(),
    })
}

/// `Freigabe {anfragen, nachweis, entscheidung, (begruendung)}` (R-L6 point 1, DI-L6 point 4): `erteilen` or `ablehnen`; a reason only
/// with `ablehnen` and only when not empty (R-L6 point 2), never with `sitzung`.
#[must_use]
pub fn freigabe(anfrage: &str, nachweis: &str, erteilen: bool, begruendung: &str) -> Value {
    let mut o = Map::new();
    o.insert("art".into(), json!("Freigabe"));
    o.insert("anfragen".into(), json!([anfrage]));
    o.insert("nachweis".into(), json!(nachweis));
    o.insert(
        "entscheidung".into(),
        json!(if erteilen { "erteilen" } else { "ablehnen" }),
    );
    if !erteilen && !begruendung.is_empty() {
        o.insert("begruendung".into(), json!(begruendung));
    }
    Value::Object(o)
}

// ---- L9: the working folder of a session (`dienst`, addendum L9) ----

/// One entry of `Ordnerinhalt`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Eintrag {
    /// The name.
    pub name: String,
    /// `datei`, `ordner` or `anderes`.
    pub art: String,
    /// The size of a regular file in bytes.
    pub groesse: Option<u64>,
}

impl Eintrag {
    /// Whether it is a folder.
    #[must_use]
    pub fn ordner(&self) -> bool {
        self.art == "ordner"
    }
}

/// The message `Ordnerinhalt {sitzung, pfad, eintraege, abgeschnitten, t}`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ordnerinhalt {
    /// The session.
    pub sitzung: String,
    /// The path as sent.
    pub pfad: String,
    /// The entries in byte order of their names.
    pub eintraege: Vec<Eintrag>,
    /// Whether a limit or an unreadable entry cut the list.
    pub abgeschnitten: bool,
}

/// Reads `Ordnerinhalt`.
#[must_use]
pub fn ordnerinhalt_lesen(n: &Value) -> Option<Ordnerinhalt> {
    Some(Ordnerinhalt {
        sitzung: text(n, "sitzung")?,
        pfad: text_auch_leer(n, "pfad").unwrap_or_default(),
        eintraege: liste(n, "eintraege")
            .iter()
            .filter_map(|x| {
                Some(Eintrag {
                    name: text(x, "name")?,
                    art: text(x, "art").unwrap_or_else(|| "anderes".to_owned()),
                    groesse: x.get("groesse").and_then(Value::as_u64),
                })
            })
            .collect(),
        abgeschnitten: wahr(n, "abgeschnitten"),
    })
}

/// One hit of `Suchtreffer`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Treffer {
    /// Path below the working folder, parts separated by `/`.
    pub pfad: String,
    /// Line number from 1.
    pub zeile: u64,
    /// The line, at most 200 bytes.
    pub auszug: String,
}

/// The message `Suchtreffer {sitzung, muster, treffer, abgeschnitten, t}`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Suchtreffer {
    /// The session.
    pub sitzung: String,
    /// The pattern as sent.
    pub muster: String,
    /// The hits.
    pub treffer: Vec<Treffer>,
    /// Whether a limit ended the search.
    pub abgeschnitten: bool,
}

/// Reads `Suchtreffer`.
#[must_use]
pub fn suchtreffer_lesen(n: &Value) -> Option<Suchtreffer> {
    Some(Suchtreffer {
        sitzung: text(n, "sitzung")?,
        muster: text_auch_leer(n, "muster").unwrap_or_default(),
        treffer: liste(n, "treffer")
            .iter()
            .filter_map(|x| {
                Some(Treffer {
                    pfad: text(x, "pfad")?,
                    zeile: x.get("zeile").and_then(Value::as_u64).unwrap_or(0),
                    auszug: text_auch_leer(x, "auszug").unwrap_or_default(),
                })
            })
            .collect(),
        abgeschnitten: wahr(n, "abgeschnitten"),
    })
}

/// `OrdnerLesen {sitzung, pfad}`.
#[must_use]
pub fn ordner_lesen(sitzung: &str, pfad: &str) -> Value {
    json!({"art": "OrdnerLesen", "sitzung": sitzung, "pfad": pfad})
}

/// `InhaltSuchen {sitzung, muster}`.
#[must_use]
pub fn inhalt_suchen(sitzung: &str, muster: &str) -> Value {
    json!({"art": "InhaltSuchen", "sitzung": sitzung, "muster": muster})
}

// ---- L10: `Startwahl` with efforts and the start structure of version 2 (`start` "Fassung 2") ----

/// One entry of `routen[].efforts` of `Startwahl` version 2.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Effort {
    /// The harness.
    pub harness: String,
    /// The model for which `erlaubt` holds: the model of the route, else `auto`.
    pub modell: String,
    /// The allowed efforts; empty means only `null`.
    pub erlaubt: Vec<String>,
    /// The default of the user when it is allowed, else `None`.
    pub vorgabe: Option<String>,
}

/// The efforts of one route of `Startwahl` version 2; empty without the field.
#[must_use]
pub fn efforts_lesen(route: &Value) -> Vec<Effort> {
    liste(route, "efforts")
        .iter()
        .filter_map(|e| {
            Some(Effort {
                harness: text(e, "harness")?,
                modell: text(e, "modell").unwrap_or_default(),
                erlaubt: liste(e, "erlaubt")
                    .iter()
                    .filter_map(|x| x.as_str().filter(|s| !s.is_empty()).map(str::to_owned))
                    .collect(),
                vorgabe: text(e, "vorgabe"),
            })
        })
        .collect()
}

/// The start structure of version 2: version 1 with `name` and `effort`. Both keys are required; `null` stands explicitly
/// (`start`, "Gemeinsame Regeln der Fassung 2").
#[must_use]
pub fn struktur_fassung2(mut fassung1: Value, name: Option<&str>, effort: Option<&str>) -> Value {
    if let Some(o) = fassung1.as_object_mut() {
        o.insert("fassung".into(), json!(2));
        o.insert("name".into(), name.map_or(Value::Null, |n| json!(n)));
        o.insert("effort".into(), effort.map_or(Value::Null, |e| json!(e)));
    }
    fassung1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn earlier_forms_give_nothing() {
        // the list of the core before R-L1, and an empty list (the same before and after)
        assert_eq!(
            agenten_lesen(&json!({"agenten": [{"worker": "w1", "zustand": "laeuft"}]})),
            None
        );
        assert_eq!(agenten_lesen(&json!({"agenten": []})), None);
        assert_eq!(herkunft_lesen(&json!({"id": "s-1"})), None);
        // settings of version 1: value, default and origin without the right to change
        assert_eq!(
            einstellungen_lesen(
                &json!({"werte": {"sitzung.effort": {"wert": "low", "vorgabe": null, "herkunft": "firma"}}})
            ),
            None
        );
        assert_eq!(efforts_lesen(&json!({"schluessel": "r"})), Vec::new());
    }

    #[test]
    fn the_list_of_the_service_is_read() {
        let n = json!({"agenten": [
            {"worker": "s-4", "sitzung": "s-1", "rolle": "bauer", "modell": "m", "zustand": "laeuft", "nr": 4},
            {"worker": "s-5", "sitzung": "s-1", "rolle": "pruefer", "modell": "m", "zustand": "beendet", "nr": null, "code": 1}]});
        let a = agenten_lesen(&n).expect("L1");
        assert_eq!((a[0].nr, a[0].laeuft()), (Some(4), true));
        assert_eq!(
            (a[1].nr, a[1].laeuft(), a[1].code.as_deref()),
            (None, false, Some("1"))
        );
        assert!(agenten_mit_raster(&n));
        // the list of the core (R-L1): session, no role, no number
        let kern = json!({"agenten": [{"worker": "w1", "sitzung": "s-1", "zustand": "x", "gesehen_um": 0, "quelle": "q"}]});
        assert_eq!(agenten_lesen(&kern).expect("R-L1")[0].rolle, "worker");
        assert!(!agenten_mit_raster(&kern));
    }

    #[test]
    fn events_have_the_names_of_the_contracts() {
        assert_eq!(
            abo("s-4", true, (80, 24)),
            json!({"art": "Abo", "sitzung": "s-4", "an": true, "spalten": 80, "zeilen": 24})
        );
        assert_eq!(
            loeschen("s-2"),
            json!({"art": "Loeschen", "sitzung": "s-2", "bestaetigung": "s-2"})
        );
        assert_eq!(
            freigabe("a1", "n1", true, "egal"),
            json!({"art": "Freigabe", "anfragen": ["a1"], "nachweis": "n1", "entscheidung": "erteilen"})
        );
        assert_eq!(
            freigabe("a1", "n1", false, "zu breit"),
            json!({"art": "Freigabe", "anfragen": ["a1"], "nachweis": "n1", "entscheidung": "ablehnen", "begruendung": "zu breit"})
        );
        assert_eq!(freigabe("a1", "n1", false, "").get("begruendung"), None);
    }

    #[test]
    fn the_fields_of_l5_may_be_empty() {
        let h = herkunft_lesen(&json!({"id": "s-0", "name": "", "ordner": "", "projekt": ""}))
            .expect("the session from kind");
        assert!(h.ordner.is_empty() && !h.abgestuerzt);
        let h = herkunft_lesen(
            &json!({"name": "n", "ordner": "/a/kunde-x", "projekt": "kunde-x",
            "abgestuerzt": true, "fortgesetzt_als": "s-9"}),
        )
        .expect("L5");
        assert!(h.abgestuerzt);
        assert_eq!(h.fortgesetzt_als.as_deref(), Some("s-9"));
    }

    #[test]
    fn settings_of_version_2_take_area_and_choice_from_the_catalog() {
        let (admin, e) = einstellungen_lesen(&json!({"fassung": 2, "admin": true, "werte": {
            "aussehen.farbsatz": {"wert": "grau", "herkunft": "gruppe g-eins", "aenderbar": true, "grund": null},
            "optionen.herstellerzugang": {"wert": false, "herkunft": "firma", "aenderbar": false, "grund": "fest"},
            "unbekannt": {"wert": 1, "herkunft": "firma", "aenderbar": true, "grund": null}}}))
        .expect("L4");
        assert!(admin);
        assert_eq!(e.len(), 2);
        assert_eq!(
            (
                e[0].herkunft.as_str(),
                e[0].gruppe.as_deref(),
                e[0].wahl.len()
            ),
            ("gruppe", Some("g-eins"), 2)
        );
        assert_eq!(
            (e[1].bereich.as_str(), e[1].grund.as_deref()),
            ("admin", Some("fest"))
        );
    }

    #[test]
    fn requests_and_the_start_structure_of_version_2() {
        let a = anfrage_lesen(&json!({"art": "Anfrage", "id": "a1", "sitzung": "s-4", "zustand": "abgelehnt", "begruendung": "nein"}))
            .expect("Anfrage");
        assert!(a.beendet());
        assert_eq!(a.begruendung, "nein");
        let e = efforts_lesen(
            &json!({"efforts": [{"harness": "pi", "modell": "q", "erlaubt": ["off"], "vorgabe": null}]}),
        );
        assert_eq!(e[0].erlaubt, ["off"]);
        let s = struktur_fassung2(json!({"fassung": 1, "harness": "h"}), None, Some("high"));
        assert_eq!(
            (&s["fassung"], &s["name"], &s["effort"]),
            (&json!(2), &Value::Null, &json!("high"))
        );
    }
}
