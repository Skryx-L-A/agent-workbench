//! The target protocol of the view: every name and field the view needs from the core and the service for the gaps L1 to
//! L11 (`rust/spec/VERTRAEGE.md` section 4), at exactly one place, one read function per message and one builder per event.
//!
//! The names and fields are the proposal of `rust/spec/VERTRAEGE.md` section 4. The contracts `zustandskern` and `dienst` fix them
//! before the pilot; until then the real run sends none of these messages, and every operating path that needs one is
//! absent (OF8 point 3). A message counts as the target form only when it carries the field the proposal adds; a message in
//! today's form gives `None` here and leaves the path away.
//!
//! Fields beyond the proposal are marked "(addition)"; they are open for the contracts.

use serde_json::{Value, json};

fn text(v: &Value, name: &str) -> Option<String> {
    v.get(name)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
}

fn liste<'a>(v: &'a Value, name: &str) -> &'a [Value] {
    v.get(name)
        .and_then(Value::as_array)
        .map_or(&[], Vec::as_slice)
}

// ---- L1: workers with their session (messages `Agenten` and `Agentenzustand`) ----

/// One worker of the target form of `Agenten` (L1): `sitzung`, `rolle`, `modell`, `zustand`, `nr`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Agent {
    /// The worker id (`worker`, as today).
    pub worker: String,
    /// The session it belongs to.
    pub sitzung: String,
    /// Its role (shown in the accent colour of the pane head).
    pub rolle: String,
    /// Its model.
    pub modell: String,
    /// Its state (`laeuft`, `wartet`, `fertig`, ...).
    pub zustand: String,
    /// The number of its raster.
    pub nr: Option<u8>,
}

impl Agent {
    /// Whether the worker counts as running (the pill and the foot count these).
    #[must_use]
    pub fn laeuft(&self) -> bool {
        matches!(self.zustand.as_str(), "laeuft" | "wartet")
    }
}

/// One entry of `Agenten` or the message `Agentenzustand` in the target form; `None` without the field `sitzung` (today's form).
#[must_use]
pub fn agent_lesen(v: &Value) -> Option<Agent> {
    let sitzung = text(v, "sitzung")?;
    Some(Agent {
        worker: text(v, "worker").unwrap_or_default(),
        rolle: text(v, "rolle").unwrap_or_else(|| text(v, "worker").unwrap_or_default()),
        modell: text(v, "modell").unwrap_or_default(),
        zustand: text(v, "zustand").unwrap_or_default(),
        nr: v
            .get("nr")
            .and_then(Value::as_u64)
            .and_then(|n| u8::try_from(n).ok()),
        sitzung,
    })
}

/// The message `Agenten`: `Some` with the list only in the target form (every entry names its session).
#[must_use]
pub fn agenten_lesen(n: &Value) -> Option<Vec<Agent>> {
    let a = liste(n, "agenten");
    let gelesen: Vec<Agent> = a.iter().filter_map(agent_lesen).collect();
    (!gelesen.is_empty() && gelesen.len() == a.len()).then_some(gelesen)
}

// ---- L2: one subscription per raster ----

/// `Abo` for the raster `nr` of a session (L2: one subscription per visible pane, each with its own size).
#[must_use]
pub fn abo_raster(sitzung: &str, nr: u8, an: bool, groesse: (usize, usize)) -> Value {
    if an {
        json!({"art": "Abo", "sitzung": sitzung, "nr": nr, "an": true, "spalten": groesse.0, "zeilen": groesse.1})
    } else {
        json!({"art": "Abo", "sitzung": sitzung, "nr": nr, "an": false})
    }
}

/// `Groesse` for the raster `nr` (L2).
#[must_use]
pub fn groesse_raster(sitzung: &str, nr: u8, groesse: (usize, usize)) -> Value {
    json!({"art": "Groesse", "sitzung": sitzung, "nr": nr, "spalten": groesse.0, "zeilen": groesse.1})
}

/// Input into the pane of a worker: the event of the terminal with the field `nr` (addition: the proposal of L2 names only
/// `Abo` and `Groesse` per raster).
#[must_use]
pub fn mit_raster(mut ereignis: Value, nr: u8) -> Value {
    if let Some(o) = ereignis.as_object_mut() {
        o.insert("nr".into(), json!(nr));
    }
    ereignis
}

// ---- L3: continue, revive, delete, rename ----

/// `Fortsetzen {sitzung}` (L3, B11).
#[must_use]
pub fn fortsetzen(sitzung: &str) -> Value {
    json!({"art": "Fortsetzen", "sitzung": sitzung})
}

/// `Wiederbeleben {sitzung}` (L3, B17).
#[must_use]
pub fn wiederbeleben(sitzung: &str) -> Value {
    json!({"art": "Wiederbeleben", "sitzung": sitzung})
}

/// `Loeschen {sitzung}` (L3, B16).
#[must_use]
pub fn loeschen(sitzung: &str) -> Value {
    json!({"art": "Loeschen", "sitzung": sitzung})
}

/// `Umbenennen {sitzung, name}` (L3, B12).
#[must_use]
pub fn umbenennen(sitzung: &str, name: &str) -> Value {
    json!({"art": "Umbenennen", "sitzung": sitzung, "name": name})
}

// ---- L5: name, folder and project of a session in `Sitzungsliste` ----

/// The fields of L5 of one entry of `Sitzungsliste`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Herkunft {
    /// The name of the session.
    pub name: String,
    /// The working folder.
    pub ordner: String,
    /// The project (shown name).
    pub projekt: String,
}

/// L5 of one entry: `Some` only with `name`, `ordner` and `projekt` (the target form).
#[must_use]
pub fn herkunft_lesen(eintrag: &Value) -> Option<Herkunft> {
    Some(Herkunft {
        name: text(eintrag, "name")?,
        ordner: text(eintrag, "ordner")?,
        projekt: text(eintrag, "projekt")?,
    })
}

// ---- L4: settings with their effective value, origin and right to change ----

/// One setting of the target form of `Einstellungen` (L4).
#[derive(Clone, Debug, PartialEq)]
pub struct Einstellung {
    /// The key.
    pub schluessel: String,
    /// The effective value.
    pub wert: Value,
    /// Where the effective value comes from: `firma`, `gruppe` or `nutzer`.
    pub herkunft: String,
    /// The group, when the value comes from a group (addition: the proposal names only the origin).
    pub gruppe: Option<String>,
    /// Whether the viewer may change it.
    pub aenderbar: bool,
    /// `nutzer` or `admin`: the area it belongs to.
    pub bereich: String,
    /// The allowed values of a choice (addition: limits of the admins, E2).
    pub wahl: Vec<String>,
}

/// The message `Einstellungen` in the target form: `Some` only when each value carries `wert`, `herkunft` and the right to change it
/// (`aenderbar`). Today's core sends `wert`, `vorgabe` and `herkunft` without `aenderbar` (`zustandskern/src/kern.rs`, `einstellungen_fuer`);
/// that form gives `None`, because without the right and without `EinstellungSetzen` in the run nothing could be set.
#[must_use]
pub fn einstellungen_lesen(n: &Value) -> Option<Vec<Einstellung>> {
    let werte = n.get("werte")?.as_object()?;
    let mut aus = Vec::new();
    for (k, v) in werte {
        let (Some(wert), Some(herkunft), Some(aenderbar)) = (
            v.get("wert"),
            text(v, "herkunft"),
            v.get("aenderbar").and_then(Value::as_bool),
        ) else {
            return None;
        };
        aus.push(Einstellung {
            schluessel: k.clone(),
            wert: wert.clone(),
            herkunft,
            gruppe: text(v, "gruppe"),
            aenderbar,
            bereich: text(v, "bereich").unwrap_or_else(|| "nutzer".to_owned()),
            wahl: liste(v, "wahl")
                .iter()
                .filter_map(|x| x.as_str().map(str::to_owned))
                .collect(),
        });
    }
    (!aus.is_empty()).then_some(aus)
}

/// `EinstellungSetzen {schluessel, wert, bereich, ebene}` (L4); whether it holds is decided by the service.
#[must_use]
pub fn einstellung_setzen(schluessel: &str, wert: &Value, bereich: &str, ebene: &str) -> Value {
    json!({"art": "EinstellungSetzen", "schluessel": schluessel, "wert": wert, "bereich": bereich, "ebene": ebene})
}

// ---- L6: requests with the proof of the person and the guard history ----

/// The proof of the person of one entry of `Anfragen` (addition: the core asks `nachweis` for `Freigabe`, the view has
/// none today; with it the buttons Freigeben and Ablehnen appear).
#[must_use]
pub fn nachweis_lesen(anfrage: &Value) -> Option<String> {
    text(anfrage, "nachweis")
}

/// One line of the guard history (B07).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Verlauf {
    /// Request id.
    pub anfrage: String,
    /// `erteilt` or `abgelehnt`.
    pub entscheid: String,
    /// The tool.
    pub werkzeug: String,
    /// The reason, may be empty.
    pub begruendung: String,
}

/// The guard history of `Anfragen` (addition: field `verlauf`); `None` in today's form.
#[must_use]
pub fn verlauf_lesen(n: &Value) -> Option<Vec<Verlauf>> {
    let v = n.get("verlauf")?.as_array()?;
    Some(
        v.iter()
            .map(|x| Verlauf {
                anfrage: text(x, "anfrage").unwrap_or_default(),
                entscheid: text(x, "entscheid").unwrap_or_default(),
                werkzeug: text(x, "werkzeug").unwrap_or_default(),
                begruendung: text(x, "begruendung").unwrap_or_default(),
            })
            .collect(),
    )
}

/// `Freigabe` with the proof (L6): `entscheid` `erteilt` or `abgelehnt`, a reason (may be empty).
#[must_use]
pub fn freigabe(anfrage: &str, nachweis: &str, erteilt: bool, begruendung: &str) -> Value {
    json!({"art": "Freigabe", "anfragen": [anfrage], "nachweis": nachweis,
           "entscheid": if erteilt { "erteilt" } else { "abgelehnt" }, "begruendung": begruendung})
}

// ---- L9: the working folder of a session ----

/// One entry of `Ordnerinhalt`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Eintrag {
    /// The name.
    pub name: String,
    /// Whether it is a folder.
    pub ordner: bool,
}

/// The message `Ordnerinhalt {sitzung, pfad, eintraege}` (L9).
#[must_use]
pub fn ordnerinhalt_lesen(n: &Value) -> Option<(String, String, Vec<Eintrag>)> {
    let sitzung = text(n, "sitzung")?;
    let pfad = n
        .get("pfad")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_owned();
    let e = liste(n, "eintraege")
        .iter()
        .filter_map(|x| {
            Some(Eintrag {
                name: text(x, "name")?,
                ordner: x.get("art").and_then(Value::as_str) == Some("ordner"),
            })
        })
        .collect();
    Some((sitzung, pfad, e))
}

/// One hit of `Suchtreffer`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Treffer {
    /// Path below the working folder.
    pub pfad: String,
    /// Line number.
    pub zeile: u64,
    /// The text of the line.
    pub text: String,
}

/// The message `Suchtreffer {sitzung, muster, treffer}` (L9).
#[must_use]
pub fn suchtreffer_lesen(n: &Value) -> Option<(String, String, Vec<Treffer>)> {
    let sitzung = text(n, "sitzung")?;
    let muster = n
        .get("muster")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_owned();
    let t = liste(n, "treffer")
        .iter()
        .filter_map(|x| {
            Some(Treffer {
                pfad: text(x, "pfad")?,
                zeile: x.get("zeile").and_then(Value::as_u64).unwrap_or(0),
                text: x
                    .get("text")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_owned(),
            })
        })
        .collect();
    Some((sitzung, muster, t))
}

/// `OrdnerLesen {sitzung, pfad}` (L9).
#[must_use]
pub fn ordner_lesen(sitzung: &str, pfad: &str) -> Value {
    json!({"art": "OrdnerLesen", "sitzung": sitzung, "pfad": pfad})
}

/// `InhaltSuchen {sitzung, muster}` (L9).
#[must_use]
pub fn inhalt_suchen(sitzung: &str, muster: &str) -> Value {
    json!({"art": "InhaltSuchen", "sitzung": sitzung, "muster": muster})
}

// ---- L10: effort in `Startwahl` and in the start structure ----

/// The efforts of one route of `Startwahl` (L10): the allowed ones (limits of the admins) and the default of the user.
#[must_use]
pub fn efforts_lesen(route: &Value) -> Option<(Vec<String>, Option<String>)> {
    let e: Vec<String> = route
        .get("efforts")?
        .as_array()?
        .iter()
        .filter_map(|x| x.as_str().map(str::to_owned))
        .collect();
    (!e.is_empty()).then(|| (e, text(route, "effort_vorgabe")))
}

/// The start structure of version 2 (L10, SE6 point 3): version 1 with `name` and `effort`.
#[must_use]
pub fn struktur_fassung2(mut fassung1: Value, name: &str, effort: &str) -> Value {
    if let Some(o) = fassung1.as_object_mut() {
        o.insert("fassung".into(), json!(2));
        o.insert("name".into(), json!(name));
        o.insert("effort".into(), json!(effort));
    }
    fassung1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn todays_forms_give_nothing() {
        assert_eq!(
            agenten_lesen(&json!({"agenten": [{"worker": "w1", "zustand": "laeuft"}]})),
            None
        );
        assert_eq!(herkunft_lesen(&json!({"id": "s-1"})), None);
        assert_eq!(einstellungen_lesen(&json!({"werte": {"a": true}})), None);
        // today's core: value, default and origin, no right to change
        assert_eq!(
            einstellungen_lesen(
                &json!({"werte": {"a": {"wert": true, "vorgabe": false, "herkunft": "firma"}}})
            ),
            None
        );
        assert_eq!(verlauf_lesen(&json!({"anfragen": []})), None);
        assert_eq!(efforts_lesen(&json!({"schluessel": "r"})), None);
    }

    #[test]
    fn target_forms_are_read() {
        let a = agenten_lesen(&json!({"agenten": [
            {"worker": "w1", "sitzung": "s-1", "rolle": "bauer", "modell": "m", "zustand": "laeuft", "nr": 3}]}))
        .expect("L1");
        assert_eq!(a[0].nr, Some(3));
        assert!(a[0].laeuft());
        let e = einstellungen_lesen(&json!({"werte": {"sitzung.effort":
            {"wert": "mittel", "herkunft": "firma", "aenderbar": true, "bereich": "nutzer", "wahl": ["niedrig", "mittel"]}}}))
        .expect("L4");
        assert_eq!(e[0].wahl.len(), 2);
        let s = struktur_fassung2(json!({"fassung": 1, "harness": "h"}), "n", "hoch");
        assert_eq!(s["fassung"], 2);
        assert_eq!(s["effort"], "hoch");
        assert_eq!(mit_raster(json!({"art": "Text"}), 4)["nr"], 4);
    }
}
