//! The block `effort` of version 2 (contract 2, addendum of 26 November 2026): the limits of the
//! administrators for the effort of a session, and the effort values of each harness.
//!
//! The block is optional. Without it no effort can be chosen for any harness (fail closed); a start
//! without an effort stays valid.

use crate::aufbau::{Befund, anzahl, ohne_doppel, pflicht, unbekannte};
use crate::json::{Wert, feld};
use crate::netz::ist_modell;
use crate::typen::Einstellungen;
use crate::{Fehler, ist_kennung};

const BLOCK: [&str; 1] = ["grenzen"];
const GRENZE: [&str; 5] = ["harness", "klasse", "modell", "erlaubt", "vorgabe"];
const GRENZEN_MAX: usize = 64;
const ERLAUBT_MAX: usize = 7;

const CLAUDE: [&str; 5] = ["low", "medium", "high", "xhigh", "max"];
const COPILOT: [&str; 7] = ["none", "minimal", "low", "medium", "high", "xhigh", "max"];
const PI: [&str; 7] = ["off", "minimal", "low", "medium", "high", "xhigh", "max"];
// opencode 1.19.5: the value of `options.reasoningEffort` of the model, sent as `reasoning_effort` (measured 27.11.2026)
const OPENCODE: [&str; 7] = ["none", "minimal", "low", "medium", "high", "xhigh", "max"];

/// The effort values of a harness of the table of contract 1, in the order of contract 2; `None` for a
/// harness outside the table. The values are compared literally: `none` exists only for `copilot` and `opencode`, `off`
/// only for `pi`.
#[must_use]
pub fn effort_werte(harness: &str) -> Option<&'static [&'static str]> {
    match harness {
        "claude" => Some(&CLAUDE),
        "copilot" => Some(&COPILOT),
        "pi" => Some(&PI),
        "opencode" => Some(&OPENCODE),
        _ => None,
    }
}

/// One limit of the block `effort`: for a harness, optionally only for a class and only for a model, the
/// allowed effort values.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffortGrenze {
    pub(crate) harness: String,
    pub(crate) klasse: Option<String>,
    pub(crate) modell: Option<String>,
    pub(crate) erlaubt: Vec<String>,
    pub(crate) vorgabe: Option<String>,
}

impl EffortGrenze {
    /// The harness the limit is for.
    #[must_use]
    pub fn harness(&self) -> &str {
        &self.harness
    }

    /// The class of the routes the limit is for; `None` for every class.
    #[must_use]
    pub fn klasse(&self) -> Option<&str> {
        self.klasse.as_deref()
    }

    /// The model of the start the limit is for; `None` for every model.
    #[must_use]
    pub fn modell(&self) -> Option<&str> {
        self.modell.as_deref()
    }

    /// The allowed values, in the order of the file.
    #[must_use]
    pub fn erlaubt(&self) -> &[String] {
        &self.erlaubt
    }

    /// The default effort of the limit (decision of the operator, 28 November 2026): the effort "Lauf starten" passes on
    /// for a start without one; `None` when the limit has none.
    #[must_use]
    pub fn vorgabe(&self) -> Option<&str> {
        self.vorgabe.as_deref()
    }

    fn passt(&self, harness: &str, klasse: &str, modell: &str) -> bool {
        self.harness == harness
            && self.klasse.as_deref().is_none_or(|k| k == klasse)
            && self.modell.as_deref().is_none_or(|m| m == modell)
    }
}

impl Einstellungen {
    /// All limits of the block `effort`, in the order of the file. Empty when the block is absent, the list is
    /// empty or the file has version 1.
    #[must_use]
    pub fn effort_grenzen(&self) -> &[EffortGrenze] {
        &self.effort
    }

    /// The efforts a start may choose for `harness` on the route `route` with the model `modell` (contract 2,
    /// "Erlaubte Efforts eines Starts"): the values of the harness that stand in every matching limit, in the
    /// order of the harness. Empty when no limit matches, the block is absent, the route or the harness is
    /// unknown: then only a start without an effort is valid.
    #[must_use]
    pub fn efforts(&self, harness: &str, route: &str, modell: &str) -> Vec<&'static str> {
        let (Some(werte), Some(route)) = (effort_werte(harness), self.route(route)) else {
            return Vec::new();
        };
        let passend: Vec<&EffortGrenze> = self
            .effort
            .iter()
            .filter(|grenze| grenze.passt(harness, route.klasse(), modell))
            .collect();
        if passend.is_empty() {
            return Vec::new();
        }
        werte
            .iter()
            .copied()
            .filter(|wert| {
                passend
                    .iter()
                    .all(|grenze| grenze.erlaubt.iter().any(|e| e == wert))
            })
            .collect()
    }

    /// The default effort of the admins for `harness` on the route `route` with the model `modell` (contract 2,
    /// "Vorgabe eines Starts"): the `vorgabe` of the first matching limit that has one, in the order of the file. `None`
    /// when no matching limit has one, the block is absent or the route is unknown. The value is a value of the harness,
    /// but it need not be allowed: the check of the start (contract 1, stage 18) decides that.
    #[must_use]
    pub fn effort_vorgabe(&self, harness: &str, route: &str, modell: &str) -> Option<&str> {
        let route = self.route(route)?;
        self.effort
            .iter()
            .filter(|grenze| grenze.passt(harness, route.klasse(), modell))
            .find_map(EffortGrenze::vorgabe)
    }
}

/// The optional block `effort` of version 2: stages 3, 5, 6, 8, 9, 10, 23, 35 and 36. Stage 12 (the class)
/// needs the classes and is checked by [`querverweise`].
pub(crate) fn block(felder: &[(String, Wert)], befund: &mut Befund) -> Vec<EffortGrenze> {
    let Some(wert) = feld(felder, "effort") else {
        return Vec::new();
    };
    let Wert::Objekt(block) = wert else {
        befund.melde(Fehler::TypFalsch);
        return Vec::new();
    };
    unbekannte(block, &BLOCK, befund);
    let Some(liste) = pflicht(block, "grenzen", befund) else {
        return Vec::new();
    };
    let Wert::Liste(elemente) = liste else {
        befund.melde(Fehler::TypFalsch);
        return Vec::new();
    };
    anzahl(elemente.len(), 0, GRENZEN_MAX, befund);
    let mut grenzen = Vec::new();
    for element in elemente {
        match element {
            Wert::Objekt(felder) => grenzen.extend(grenze(felder, befund)),
            _ => befund.melde(Fehler::TypFalsch),
        }
    }
    ohne_doppel(
        grenzen
            .iter()
            .map(|grenze| (&grenze.harness, &grenze.klasse, &grenze.modell)),
        befund,
    );
    grenzen
}

/// One limit; `None` when a required key is missing or has the wrong type.
fn grenze(felder: &[(String, Wert)], befund: &mut Befund) -> Option<EffortGrenze> {
    unbekannte(felder, &GRENZE, befund);
    let harness = pflicht(felder, "harness", befund);
    let erlaubt = pflicht(felder, "erlaubt", befund);
    let Wert::Text(harness) = harness? else {
        befund.melde(Fehler::TypFalsch);
        return None;
    };
    let klasse = wahl_text(felder, "klasse", befund).ok()?;
    let modell = wahl_text(felder, "modell", befund).ok()?;
    let vorgabe = wahl_text(felder, "vorgabe", befund).ok()?;
    let Wert::Liste(elemente) = erlaubt? else {
        befund.melde(Fehler::TypFalsch);
        return None;
    };
    let mut werte = Vec::new();
    for element in elemente {
        let Wert::Text(t) = element else {
            befund.melde(Fehler::TypFalsch);
            return None;
        };
        werte.push(t.clone());
    }
    anzahl(werte.len(), 1, ERLAUBT_MAX, befund);
    if !ist_kennung(harness) || klasse.as_deref().is_some_and(|k| !ist_kennung(k)) {
        befund.melde(Fehler::KennungForm);
    }
    ohne_doppel(werte.iter(), befund);
    if modell.as_deref().is_some_and(|m| !ist_modell(m)) {
        befund.melde(Fehler::ModellForm);
    }
    match effort_werte(harness) {
        None => befund.melde(Fehler::EffortHarness),
        Some(erlaubt) => {
            if werte
                .iter()
                .chain(vorgabe.iter())
                .any(|wert| !erlaubt.contains(&wert.as_str()))
            {
                befund.melde(Fehler::EffortWert);
            }
        }
    }
    Some(EffortGrenze {
        harness: harness.clone(),
        klasse,
        modell,
        erlaubt: werte,
        vorgabe,
    })
}

/// An optional key with a text value: `Ok(None)` when absent, `Err` (stage 6 recorded) for another type.
fn wahl_text(
    felder: &[(String, Wert)],
    name: &str,
    befund: &mut Befund,
) -> Result<Option<String>, Fehler> {
    match feld(felder, name) {
        None => Ok(None),
        Some(Wert::Text(t)) => Ok(Some(t.clone())),
        Some(_) => {
            befund.melde(Fehler::TypFalsch);
            Err(Fehler::TypFalsch)
        }
    }
}

/// Stage 12 of the block: the class of a limit must stand in `klassen`.
pub(crate) fn querverweise(grenzen: &[EffortGrenze], klassen: &[String], befund: &mut Befund) {
    if grenzen
        .iter()
        .filter_map(|grenze| grenze.klasse.as_ref())
        .any(|klasse| !klassen.contains(klasse))
    {
        befund.melde(Fehler::KlasseUnbekannt);
    }
}

#[cfg(test)]
mod tests {
    use super::effort_werte;

    #[test]
    fn every_harness_of_the_table_has_its_values_in_the_order_of_the_contract() {
        assert_eq!(
            effort_werte("claude"),
            Some(&["low", "medium", "high", "xhigh", "max"][..])
        );
        assert_eq!(
            effort_werte("copilot"),
            Some(&["none", "minimal", "low", "medium", "high", "xhigh", "max"][..])
        );
        assert_eq!(
            effort_werte("pi"),
            Some(&["off", "minimal", "low", "medium", "high", "xhigh", "max"][..])
        );
        assert_eq!(
            effort_werte("opencode"),
            Some(&["none", "minimal", "low", "medium", "high", "xhigh", "max"][..])
        );
        assert_eq!(effort_werte("codex"), None);
        assert_eq!(effort_werte("Pi"), None);
        assert_eq!(effort_werte(""), None);
    }
}
