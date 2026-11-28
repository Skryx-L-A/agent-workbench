//! Start form (contract `seite` SE6 points 3 and 4, contract `dienst` DI12 point 10): the message `Startwahl` fills
//! the choice fields; submitting gives the start structure, "ohne Kunde" as `null`.
//!
//! Version 2 (contract `start`, "Fassung 2"): `Startwahl` with `fassung: 2` says that the service takes the start structure of version
//! 2. From then on the form has the field `Name`, whatever the route. The field `Effort` stands only while the chosen route carries
//! efforts for the chosen harness; for a route without efforts the start structure has `effort: null`.

use serde_json::{Value, json};

/// The entry for no customer; it sends `null`.
pub const OHNE_KUNDE: &str = "ohne Kunde";
/// The model that holds without a model of the route.
pub const MODELL_AUTO: &str = "auto";
/// The entry of the field `Effort` that sends `null` (contract `start`, message `Startwahl` with efforts).
pub const VORGABE_HARNESS: &str = crate::woerter::VORGABE_DES_PROGRAMMS;

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
    /// Whether the last `Startwahl` has version 2: the service takes the start structure of version 2 (`name` and `effort`).
    pub fassung2: bool,
    /// The name of the new session (window "Sitzungen", SE2 point 2); sent with version 2, empty as `null`.
    pub name: String,
    /// The efforts of each route (`routen[].efforts` of `Startwahl` version 2); empty with version 1.
    pub efforts: Vec<(String, Vec<crate::ziel::Effort>)>,
    /// The effort of the new session: "Vorgabe des Harness" (value `""`, sent as `null`) and the allowed efforts of the chosen route
    /// and harness; without allowed efforts the field `effort-wahl` is absent.
    pub effort: Wahlfeld,
}

impl Default for Startform {
    /// The fixed sample entries of the contract `seite`, valid until the first `Startwahl`.
    fn default() -> Self {
        Startform {
            harness: Wahlfeld::neu(
                vec![
                    Eintrag::gleich("claude"),
                    Eintrag::gleich("pi"),
                    Eintrag::gleich("opencode"),
                ],
                None,
            ),
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
            fassung2: false,
            name: String::new(),
            efforts: Vec::new(),
            effort: Wahlfeld::neu(Vec::new(), None),
        }
    }
}

/// The visible text of an effort value: `off` and `none` read as the word for "aus" ([`crate::woerter::DENKSTUFE_AUS`]), every
/// other value as it is (`einstellungen` version 2, values per harness).
#[must_use]
pub fn effort_text(wert: &str) -> &str {
    match wert {
        "off" | "none" => crate::woerter::DENKSTUFE_AUS,
        x => x,
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
    /// Takes a message `Startwahl` (SE6 point 4) of version 1 or 2; another version changes nothing.
    pub fn startwahl(&mut self, n: &Value) {
        let (Some(fassung @ (1 | 2)), Some(harness), Some(routen), Some(kunden)) = (
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
        self.fassung2 = fassung == 2;
        self.efforts = if self.fassung2 {
            routen
                .iter()
                .filter_map(|r| {
                    let s = text(r.get("schluessel"))?;
                    Some((s.to_owned(), crate::ziel::efforts_lesen(r)))
                })
                .collect()
        } else {
            Vec::new()
        };
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
        self.effort_fuellen();
    }

    /// The field `Effort` after the choice of route and harness: the allowed efforts of the entry of the harness in the route, with
    /// "Vorgabe des Harness" first and the default of the user chosen; no entries when the route carries no allowed effort for it.
    fn effort_fuellen(&mut self) {
        let e = self
            .efforts
            .iter()
            .find(|(r, _)| *r == self.route.wert)
            .and_then(|(_, e)| e.iter().find(|x| x.harness == self.harness.wert))
            .filter(|e| !e.erlaubt.is_empty());
        self.effort = match e {
            Some(e) => {
                let mut eintraege = vec![Eintrag {
                    wert: String::new(),
                    text: VORGABE_HARNESS.to_owned(),
                }];
                eintraege.extend(e.erlaubt.iter().map(|x| Eintrag {
                    wert: x.clone(),
                    text: effort_text(x).to_owned(),
                }));
                Wahlfeld::neu(eintraege, Some(e.vorgabe.as_deref().unwrap_or("")))
            }
            None => Wahlfeld::neu(Vec::new(), None),
        };
    }

    /// Whether the form has the field `Name`: the service takes version 2 of the start structure (`Startwahl` with `fassung: 2`). The
    /// name does not hang on the route.
    #[must_use]
    pub fn mit_name(&self) -> bool {
        self.fassung2
    }

    /// Whether the form has the field `Effort`: version 2 and the chosen route carries allowed efforts for the chosen harness.
    #[must_use]
    pub fn mit_effort(&self) -> bool {
        self.fassung2 && !self.effort.eintraege.is_empty()
    }

    /// A new choice of the harness; with version 2 the field `Effort` follows it.
    pub fn harness_waehlen(&mut self, wert: &str) {
        wert.clone_into(&mut self.harness.wert);
        if self.startwahl_da {
            self.effort_fuellen();
        }
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
        if !self.fassung2 {
            return f1;
        }
        let effort = Some(self.effort.wert.as_str()).filter(|e| self.mit_effort() && !e.is_empty());
        crate::ziel::struktur_fassung2(
            f1,
            Some(self.name.as_str()).filter(|n| !n.is_empty()),
            effort,
        )
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
        f.startwahl(&json!({"fassung": 3, "harness": [], "routen": [], "kunden": []}));
        assert_eq!(f.harness.wert, "b");
    }

    fn wahl2() -> Value {
        json!({"fassung": 2,
            "harness": [{"name": "claude", "art": "cloud", "routenart": "cloud"}, {"name": "copilot", "art": "cloud", "routenart": "cloud"}],
            "routen": [{"schluessel": "copilot", "art": "cloud", "klasse": "intern", "modell": null,
                        "efforts": [{"harness": "claude", "modell": "auto", "erlaubt": [], "vorgabe": null},
                                    {"harness": "copilot", "modell": "auto", "erlaubt": ["low", "medium", "high"], "vorgabe": "medium"}]},
                       {"schluessel": "ohne", "art": "cloud", "klasse": "intern", "modell": null,
                        "efforts": [{"harness": "claude", "modell": "auto", "erlaubt": [], "vorgabe": null},
                                    {"harness": "copilot", "modell": "auto", "erlaubt": [], "vorgabe": null}]}],
            "kunden": [null]})
    }

    /// Decision of 27.11.2026: the name appears with version 2 and does not hang on the route; the effort appears while
    /// the chosen route carries efforts and goes away for a route without; then the start sends no effort.
    #[test]
    fn name_with_version_2_and_effort_with_the_route() {
        let mut f = Startform::default();
        assert!(!f.mit_name() && !f.mit_effort());
        f.startwahl(&wahl2());
        f.leeren();
        assert!(f.mit_name());
        assert!(
            !f.mit_effort(),
            "claude has no allowed effort on this route"
        );
        f.harness_waehlen("copilot");
        assert!(f.mit_effort());
        assert_eq!(f.effort.wert, "medium");
        assert_eq!(f.effort.eintraege[0].text, VORGABE_HARNESS);
        f.name = "Bau".into();
        let s = f.struktur();
        assert_eq!(
            (&s["fassung"], &s["name"], &s["effort"]),
            (&json!(2), &json!("Bau"), &json!("medium"))
        );
        f.route_waehlen("ohne");
        assert!(f.mit_name() && !f.mit_effort());
        let s = f.struktur();
        assert_eq!((&s["name"], &s["effort"]), (&json!("Bau"), &Value::Null));
        f.route_waehlen("copilot");
        f.effort.wert = String::new();
        f.name.clear();
        let s = f.struktur();
        assert_eq!((&s["name"], &s["effort"]), (&Value::Null, &Value::Null));
        // decision of 28.11.2026: `off` and `none` read as the word for "aus", the value stays
        f.startwahl(&json!({"fassung": 2, "harness": [{"name": "pi"}],
            "routen": [{"schluessel": "lokal", "modell": "q", "efforts": [{"harness": "pi", "modell": "q", "erlaubt": ["off", "low"], "vorgabe": null}]}],
            "kunden": []}));
        assert_eq!(
            f.effort
                .eintraege
                .iter()
                .map(|e| (e.wert.as_str(), e.text.as_str()))
                .collect::<Vec<_>>(),
            [
                ("", VORGABE_HARNESS),
                ("off", crate::woerter::DENKSTUFE_AUS),
                ("low", "low")
            ]
        );
        assert_eq!((effort_text("none"), effort_text("high")), ("aus", "high"));
        // back to version 1: no name, no effort, version 1
        f.startwahl(&json!({"fassung": 1, "harness": [{"name": "claude"}], "routen": [{"schluessel": "r"}], "kunden": []}));
        assert!(!f.mit_name() && !f.mit_effort());
        assert_eq!(f.struktur()["fassung"], 1);
    }
}
