//! Start form (contract `seite` SE6 points 3 and 4, contract `dienst` DI12 point 10): the message `Startwahl` fills
//! the choice fields; submitting gives the start structure of version 1, "ohne Kunde" as `null`.

use serde_json::{Value, json};

/// The entry for no customer; it sends `null`.
pub const OHNE_KUNDE: &str = "ohne Kunde";
/// The model that holds without a model of the route.
pub const MODELL_AUTO: &str = "auto";

/// One option: value and visible text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Eintrag {
    /// The value sent.
    pub wert: String,
    /// The text shown.
    pub text: String,
}

impl Eintrag {
    /// An option whose value is its text.
    #[must_use]
    pub fn gleich(t: &str) -> Eintrag {
        Eintrag {
            wert: t.to_owned(),
            text: t.to_owned(),
        }
    }
}

/// One choice field: its options and the chosen value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Wahlfeld {
    /// The options in order.
    pub eintraege: Vec<Eintrag>,
    /// The chosen value.
    pub wert: String,
}

impl Wahlfeld {
    /// A field with these options; the first is chosen unless `wert` names one.
    #[must_use]
    pub fn neu(eintraege: Vec<Eintrag>, wert: Option<&str>) -> Wahlfeld {
        let wert = wert
            .filter(|w| eintraege.iter().any(|e| e.wert == *w))
            .map(str::to_owned)
            .or_else(|| eintraege.first().map(|e| e.wert.clone()))
            .unwrap_or_default();
        Wahlfeld { eintraege, wert }
    }

    /// New options; the choice stays when its entry is still there, else `vorgabe` or the first entry.
    pub fn setzen(&mut self, eintraege: Vec<Eintrag>, vorgabe: Option<&str>) {
        let bisher = self.wert.clone();
        let bleibt = eintraege.iter().any(|e| e.wert == bisher);
        *self = Wahlfeld::neu(eintraege, if bleibt { Some(&bisher) } else { vorgabe });
    }
}

/// The start form: four choice fields and the folder.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Startform {
    /// Harness.
    pub harness: Wahlfeld,
    /// Model.
    pub modell: Wahlfeld,
    /// Route.
    pub route: Wahlfeld,
    /// Customer ("ohne Kunde" has the value "").
    pub kunde: Wahlfeld,
    /// Working folder.
    pub arbeitsordner: String,
    /// The model of each route (`None`: `auto`); empty until the first `Startwahl`.
    pub modelle: Vec<(String, Option<String>)>,
    /// Whether a `Startwahl` came.
    pub startwahl_da: bool,
    /// The name of the new session (window "Sitzungen", SE2 point 2); sent from version 2 of the start structure on (L10).
    pub name: String,
    /// The efforts of each route (L10): allowed ones and the default of the user; empty in today's form of `Startwahl`.
    pub efforts: Vec<(String, Vec<String>, Option<String>)>,
    /// The effort of the new session (L10); without efforts of the route the field `effort-wahl` is absent.
    pub effort: Wahlfeld,
}

impl Default for Startform {
    /// The fixed sample entries of the contract `seite`, valid until the first `Startwahl`.
    fn default() -> Self {
        Startform {
            harness: Wahlfeld::neu(vec![Eintrag::gleich("claude"), Eintrag::gleich("pi")], None),
            modell: Wahlfeld::neu(
                vec![
                    Eintrag::gleich("cloud-beispiel"),
                    Eintrag::gleich("lokal-beispiel"),
                ],
                None,
            ),
            route: Wahlfeld::neu(
                vec![Eintrag::gleich("cloud"), Eintrag::gleich("lokal")],
                None,
            ),
            kunde: Wahlfeld::neu(
                vec![
                    Eintrag::gleich("k-beispiel"),
                    Eintrag::gleich("k-zwei"),
                    ohne_kunde(),
                ],
                Some(""),
            ),
            arbeitsordner: String::new(),
            modelle: Vec::new(),
            startwahl_da: false,
            name: String::new(),
            efforts: Vec::new(),
            effort: Wahlfeld::neu(Vec::new(), None),
        }
    }
}

fn ohne_kunde() -> Eintrag {
    Eintrag {
        wert: String::new(),
        text: OHNE_KUNDE.to_owned(),
    }
}

fn text(v: Option<&Value>) -> Option<&str> {
    v.and_then(Value::as_str).filter(|s| !s.is_empty())
}

impl Startform {
    /// Takes a message `Startwahl` (SE6 point 4); a message not of version 1 changes nothing.
    pub fn startwahl(&mut self, n: &Value) {
        let (Some(1), Some(harness), Some(routen), Some(kunden)) = (
            n.get("fassung").and_then(Value::as_i64),
            n.get("harness").and_then(Value::as_array),
            n.get("routen").and_then(Value::as_array),
            n.get("kunden").and_then(Value::as_array),
        ) else {
            return;
        };
        let harness: Vec<Eintrag> = harness
            .iter()
            .filter_map(|h| text(h.get("name")))
            .map(Eintrag::gleich)
            .collect();
        let mit_schluessel: Vec<(&str, Option<String>)> = routen
            .iter()
            .filter_map(|r| {
                text(r.get("schluessel")).map(|s| (s, text(r.get("modell")).map(str::to_owned)))
            })
            .collect();
        let mut kunden: Vec<Eintrag> = kunden
            .iter()
            .filter_map(|k| text(Some(k)))
            .map(Eintrag::gleich)
            .collect();
        kunden.push(ohne_kunde());
        if !harness.is_empty() {
            self.harness.setzen(harness, None);
        }
        if !mit_schluessel.is_empty() {
            self.route.setzen(
                mit_schluessel
                    .iter()
                    .map(|(s, _)| Eintrag::gleich(s))
                    .collect(),
                None,
            );
        }
        self.kunde.setzen(kunden, Some(""));
        self.modelle = mit_schluessel
            .into_iter()
            .map(|(s, m)| (s.to_owned(), m))
            .collect();
        self.efforts = routen
            .iter()
            .filter_map(|r| {
                let s = text(r.get("schluessel"))?;
                let (e, v) = crate::ziel::efforts_lesen(r)?;
                Some((s.to_owned(), e, v))
            })
            .collect();
        self.startwahl_da = true;
        self.modell_fuellen();
    }

    /// "+" opens the window with an empty form (decision F1): every choice on its first entry, the customer on "ohne Kunde",
    /// name and folder empty, the effort on the default of the user. The entries stay.
    pub fn leeren(&mut self) {
        for f in [&mut self.harness, &mut self.route] {
            f.wert = f
                .eintraege
                .first()
                .map(|e| e.wert.clone())
                .unwrap_or_default();
        }
        let ohne = self.kunde.eintraege.iter().any(|e| e.wert.is_empty());
        self.kunde.wert = if ohne {
            String::new()
        } else {
            self.kunde
                .eintraege
                .first()
                .map(|e| e.wert.clone())
                .unwrap_or_default()
        };
        self.name.clear();
        self.arbeitsordner.clear();
        if self.startwahl_da {
            self.modell_fuellen();
        } else {
            self.modell.wert = self
                .modell
                .eintraege
                .first()
                .map(|e| e.wert.clone())
                .unwrap_or_default();
        }
    }

    /// The field `Modell` has exactly one entry after a `Startwahl`: the model of the chosen route, else `auto`.
    fn modell_fuellen(&mut self) {
        let m = self
            .modelle
            .iter()
            .find(|(r, _)| *r == self.route.wert)
            .and_then(|(_, m)| m.clone())
            .unwrap_or_else(|| MODELL_AUTO.to_owned());
        self.modell = Wahlfeld::neu(vec![Eintrag::gleich(&m)], None);
        let (erlaubt, vorgabe) = self
            .efforts
            .iter()
            .find(|(r, _, _)| *r == self.route.wert)
            .map(|(_, e, v)| (e.clone(), v.clone()))
            .unwrap_or_default();
        self.effort = Wahlfeld::neu(
            erlaubt.iter().map(|e| Eintrag::gleich(e)).collect(),
            vorgabe.as_deref(),
        );
    }

    /// Whether the start structure has version 2 (L10: `Startwahl` carries efforts).
    #[must_use]
    pub fn mit_effort(&self) -> bool {
        !self.effort.eintraege.is_empty()
    }

    /// A new choice of the route; after a `Startwahl` the model follows it.
    pub fn route_waehlen(&mut self, wert: &str) {
        wert.clone_into(&mut self.route.wert);
        if self.startwahl_da {
            self.modell_fuellen();
        }
    }

    /// The start structure (SE6 point 3).
    #[must_use]
    pub fn struktur(&self) -> Value {
        let f1 = json!({
            "fassung": 1,
            "harness": self.harness.wert,
            "modell": self.modell.wert,
            "route": self.route.wert,
            "kunde": if self.kunde.wert.is_empty() { Value::Null } else { json!(self.kunde.wert) },
            "arbeitsordner": self.arbeitsordner,
        });
        if self.mit_effort() {
            crate::ziel::struktur_fassung2(f1, &self.name, &self.effort.wert)
        } else {
            f1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn samples_until_the_first_startwahl() {
        let f = Startform::default();
        assert_eq!(f.kunde.wert, "");
        assert_eq!(f.modell.wert, "cloud-beispiel");
        let s = f.struktur();
        assert_eq!(s["kunde"], Value::Null);
        assert_eq!(s["fassung"], 1);
    }

    #[test]
    fn startwahl_fills_and_the_model_follows_the_route() {
        let mut f = Startform::default();
        f.startwahl(
            &json!({"fassung": 1, "harness": [{"name": "h1"}, {"name": ""}],
            "routen": [{"schluessel": "r1", "modell": "m1"}, {"schluessel": "r2", "modell": null}],
            "kunden": [null, "k1"]}),
        );
        assert_eq!(f.harness.eintraege, vec![Eintrag::gleich("h1")]);
        assert_eq!(
            f.kunde.eintraege.last().map(|e| e.text.as_str()),
            Some(OHNE_KUNDE)
        );
        assert_eq!(f.kunde.wert, "");
        assert_eq!(f.modell.eintraege, vec![Eintrag::gleich("m1")]);
        f.route_waehlen("r2");
        assert_eq!(f.modell.wert, MODELL_AUTO);
        f.kunde.wert = "k1".into();
        assert_eq!(f.struktur()["kunde"], "k1");
        assert_eq!(f.struktur()["modell"], MODELL_AUTO);
    }

    #[test]
    fn a_choice_stays_when_its_entry_stays() {
        let mut f = Startform::default();
        f.startwahl(&json!({"fassung": 1, "harness": [{"name": "a"}, {"name": "b"}], "routen": [], "kunden": []}));
        f.harness.wert = "b".into();
        f.startwahl(&json!({"fassung": 1, "harness": [{"name": "b"}, {"name": "c"}], "routen": [], "kunden": []}));
        assert_eq!(f.harness.wert, "b");
        f.startwahl(&json!({"fassung": 2, "harness": [], "routen": [], "kunden": []}));
        assert_eq!(f.harness.wert, "b");
    }
}
