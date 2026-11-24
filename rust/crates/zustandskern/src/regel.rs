//! Settings: one rule with its levels (company, group, user) and the value that applies (contract, rule R12).

use std::collections::BTreeMap;

use serde_json::{Map, Value, json};

/// One setting. A value is an integer: the number itself for a `zahl` rule, the index into `stufen` for a
/// `stufen` rule (a later step is stricter). Both kinds then compare as numbers.
pub(crate) struct Regel {
    /// `true` for a rule that holds a number.
    pub zahl: bool,
    /// The steps of a `stufen` rule, mildest first.
    pub stufen: Vec<String>,
    /// The smallest number an event may set (`zahl` rules only).
    pub min: i64,
    /// The default.
    pub vorgabe: i64,
    /// Whether a user may tighten this rule.
    pub nutzer_darf: bool,
    /// The company level.
    pub firma: Option<i64>,
    /// The group level.
    pub gruppe: Option<i64>,
    /// The user level, by person.
    pub nutzer: BTreeMap<String, i64>,
}

/// The rules that exist in every case, as a case would write them.
pub(crate) fn eingebaut() -> Map<String, Value> {
    let Value::Object(regeln) = json!({
        "osc52_schreiben": {"art": "stufen", "stufen": ["an", "aus"], "vorgabe": "an", "nutzer_darf": true},
        "osc52_grenze": {"art": "zahl", "vorgabe": 100_000, "nutzer_darf": true},
        "bildrate_max": {"art": "zahl", "vorgabe": 63, "nutzer_darf": true, "min": 1},
        "rueckstau_max": {"art": "zahl", "vorgabe": 16384, "nutzer_darf": true, "min": 1},
    }) else {
        return Map::new();
    };
    regeln
}

impl Regel {
    /// The value from a JSON value of the case: an integer for a `zahl` rule, a step name for a `stufen` rule.
    pub(crate) fn wert_aus(&self, wert: &Value) -> Option<i64> {
        if self.zahl {
            wert.as_i64()
        } else {
            let index = self
                .stufen
                .iter()
                .position(|s| Some(s.as_str()) == wert.as_str())?;
            i64::try_from(index).ok()
        }
    }

    /// The JSON value of a rule value.
    pub(crate) fn wert_json(&self, wert: i64) -> Value {
        if self.zahl {
            json!(wert)
        } else {
            self.stufe(wert).map_or(Value::Null, |s| json!(s))
        }
    }

    /// The step name of a value, for a `stufen` rule.
    pub(crate) fn stufe(&self, wert: i64) -> Option<&str> {
        usize::try_from(wert)
            .ok()
            .and_then(|i| self.stufen.get(i))
            .map(String::as_str)
    }

    /// Whether `a` is stricter than `b`: a smaller number, or a later step.
    pub(crate) fn strenger(&self, a: i64, b: i64) -> bool {
        if self.zahl { a < b } else { a > b }
    }

    /// The value below the user level: group before company before default, with the name of the level.
    pub(crate) fn basis(&self) -> (i64, &'static str) {
        match (self.gruppe, self.firma) {
            (Some(gruppe), _) => (gruppe, "gruppe"),
            (None, Some(firma)) => (firma, "firma"),
            (None, None) => (self.vorgabe, "vorgabe"),
        }
    }

    /// The value that applies to a person, with the name of its level. A user value counts only while it is
    /// stricter than the base.
    pub(crate) fn wirksam(&self, person: Option<&str>) -> (i64, &'static str) {
        let (basis, ebene) = self.basis();
        match person.and_then(|p| self.nutzer.get(p)) {
            Some(&wert) if self.strenger(wert, basis) => (wert, "nutzer"),
            _ => (basis, ebene),
        }
    }
}

/// Reads one rule from its merged description (default plus the case's entry).
pub(crate) fn lesen(beschreibung: &Map<String, Value>) -> Result<Regel, String> {
    let zahl = beschreibung.get("art").and_then(Value::as_str) == Some("zahl");
    let stufen = if zahl {
        Vec::new()
    } else {
        beschreibung
            .get("stufen")
            .and_then(Value::as_array)
            .ok_or("stufen fehlen")?
            .iter()
            .map(|s| s.as_str().map(str::to_owned))
            .collect::<Option<Vec<_>>>()
            .ok_or("stufen sind kein Text")?
    };
    let mut regel = Regel {
        zahl,
        stufen,
        min: 0,
        vorgabe: 0,
        nutzer_darf: beschreibung
            .get("nutzer_darf")
            .is_some_and(crate::wert::wahr),
        firma: None,
        gruppe: None,
        nutzer: BTreeMap::new(),
    };
    if let Some(min) = beschreibung.get("min") {
        regel.min = min.as_i64().ok_or("min ist keine ganze Zahl")?;
    }
    regel.vorgabe = beschreibung
        .get("vorgabe")
        .and_then(|v| regel.wert_aus(v))
        .ok_or("vorgabe fehlt oder passt nicht")?;
    regel.firma = ebene(&regel, beschreibung.get("firma"))?;
    regel.gruppe = ebene(&regel, beschreibung.get("gruppe"))?;
    match beschreibung.get("nutzer") {
        None | Some(Value::Null) => {}
        Some(Value::Object(nutzer)) => {
            for (person, v) in nutzer {
                let wert = regel.wert_aus(v).ok_or("Nutzerwert passt nicht")?;
                regel.nutzer.insert(person.clone(), wert);
            }
        }
        Some(_) => return Err("nutzer ist kein Objekt".to_owned()),
    }
    Ok(regel)
}

/// The value of a level that may be absent or `null`.
fn ebene(regel: &Regel, wert: Option<&Value>) -> Result<Option<i64>, String> {
    match wert {
        None | Some(Value::Null) => Ok(None),
        Some(v) => regel
            .wert_aus(v)
            .map(Some)
            .ok_or_else(|| "Ebenenwert passt nicht".to_owned()),
    }
}
