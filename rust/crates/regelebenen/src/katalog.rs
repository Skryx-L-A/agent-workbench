//! The catalogue of the pilot (contract, section 15.1) and the check of a value against it (stage 7 of 15.4, the form
//! of `leseklassen` in 15.11).

use serde_json::{Map, Value};

/// The area of a key: page B72 (`nutzer`) or page B78 (`admin`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Bereich {
    /// Settings of the person.
    Nutzer,
    /// Settings that only admins change.
    Admin,
}

impl Bereich {
    /// The text of the area, as `bereich` of `EinstellungSetzen` carries it.
    #[must_use]
    pub fn text(self) -> &'static str {
        match self {
            Bereich::Nutzer => "nutzer",
            Bereich::Admin => "admin",
        }
    }
}

/// The kind of a place in the state (15.3): the company, a group or the person.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platz {
    /// The level `firma`.
    Firma,
    /// The level `gruppe <schluessel>`.
    Gruppe,
    /// The level of the person itself.
    Nutzer,
}

impl Platz {
    /// The text of the place as the audit line carries it in `ebene` (15.6).
    #[must_use]
    pub fn text(self) -> &'static str {
        match self {
            Platz::Firma => "firma",
            Platz::Gruppe => "gruppe",
            Platz::Nutzer => "nutzer",
        }
    }
}

/// The direction of a key (section 5.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Richtung {
    /// A free choice: the lowest level with a value counts.
    Frei,
    /// A list per class: the intersection of all levels.
    Schnitt,
    /// A switch: `true` only if no level says `false`.
    AusStrenger,
    /// Only the company sets it.
    NurFirma,
    /// No level sets it; the default always counts.
    Fest,
}

/// What a list per class holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Element {
    /// Names of harnesses.
    Harness,
    /// Names of models.
    Modell,
}

/// The type of a key (column "Typ" of 15.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Typ {
    /// One of the given texts.
    Wahl(&'static [&'static str]),
    /// An integer from the first to the second number.
    Ganzzahl(i64, i64),
    /// A model name (`^[A-Za-z0-9._:/@+-]{1,128}$`).
    Modell,
    /// A truth value.
    Schalter,
    /// Object: class to list of at most 16 distinct elements.
    Klassenliste(Element),
    /// The list of the limits of the effort (contract 2, block `effort`).
    Grenzen,
    /// The list of 15.11.
    Leseklassen,
}

/// The default of a key, as a value without a heap.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Vorgabe {
    /// A text.
    Text(&'static str),
    /// An integer.
    Zahl(i64),
    /// `null`.
    Null,
    /// The empty object.
    Objekt,
    /// The empty list.
    Liste,
    /// `false` ("aus").
    Aus,
}

impl Vorgabe {
    /// The default as a JSON value.
    #[must_use]
    pub fn wert(self) -> Value {
        match self {
            Vorgabe::Text(t) => Value::String(t.to_owned()),
            Vorgabe::Zahl(z) => Value::from(z),
            Vorgabe::Null => Value::Null,
            Vorgabe::Objekt => Value::Object(Map::new()),
            Vorgabe::Liste => Value::Array(Vec::new()),
            Vorgabe::Aus => Value::Bool(false),
        }
    }
}

/// One line of the catalogue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Eintrag {
    /// The key.
    pub schluessel: &'static str,
    /// The area.
    pub bereich: Bereich,
    /// The places on which the key may stand (column "Ebenen").
    pub ebenen: &'static [Platz],
    /// The type.
    pub typ: Typ,
    /// The default.
    pub vorgabe: Vorgabe,
    /// The direction.
    pub richtung: Richtung,
}

impl Eintrag {
    /// `true` if the key may stand on a place of this kind.
    #[must_use]
    pub fn erlaubt_auf(&self, p: Platz) -> bool {
        self.ebenen.contains(&p)
    }
}

/// The harnesses of the pilot.
pub const HARNESSES: [&str; 3] = ["claude", "copilot", "pi"];
/// The effort values per harness, word for word from the help of the programs (contract 2, block `effort`).
pub const EFFORTS: [(&str, &[&str]); 3] = [
    ("claude", &["low", "medium", "high", "xhigh", "max"]),
    (
        "copilot",
        &["none", "minimal", "low", "medium", "high", "xhigh", "max"],
    ),
    (
        "pi",
        &["off", "minimal", "low", "medium", "high", "xhigh", "max"],
    ),
];
/// Every effort value of any harness, sorted.
pub const ALLE_EFFORTS: [&str; 8] = [
    "high", "low", "max", "medium", "minimal", "none", "off", "xhigh",
];
/// Most entries of `leseklassen` (15.11).
pub const LESEKLASSEN_MAX: usize = 4096;
/// Most entries of a list per class (15.1).
pub const KLASSENLISTE_MAX: usize = 16;
/// Most limits of the effort (15.1).
pub const GRENZEN_MAX: usize = 64;

const ALLE: &[Platz] = &[Platz::Firma, Platz::Gruppe, Platz::Nutzer];
const NUR_FIRMA: &[Platz] = &[Platz::Firma];

const fn e(
    schluessel: &'static str,
    bereich: Bereich,
    ebenen: &'static [Platz],
    typ: Typ,
    vorgabe: Vorgabe,
    richtung: Richtung,
) -> Eintrag {
    Eintrag {
        schluessel,
        bereich,
        ebenen,
        typ,
        vorgabe,
        richtung,
    }
}

/// The catalogue of the pilot, in the order of the table of 15.1.
pub const KATALOG: [Eintrag; 17] = [
    e(
        "aussehen.thema",
        Bereich::Nutzer,
        ALLE,
        Typ::Wahl(&["system", "hell", "dunkel"]),
        Vorgabe::Text("system"),
        Richtung::Frei,
    ),
    e(
        "aussehen.farbsatz",
        Bereich::Nutzer,
        ALLE,
        Typ::Wahl(&["grau", "blauschwarz"]),
        Vorgabe::Text("grau"),
        Richtung::Frei,
    ),
    e(
        "aussehen.terminal",
        Bereich::Nutzer,
        ALLE,
        Typ::Wahl(&["dunkel", "hell", "thema"]),
        Vorgabe::Text("dunkel"),
        Richtung::Frei,
    ),
    e(
        "terminal.schriftgroesse",
        Bereich::Nutzer,
        ALLE,
        Typ::Ganzzahl(9, 32),
        Vorgabe::Zahl(13),
        Richtung::Frei,
    ),
    e(
        "sitzung.harness",
        Bereich::Nutzer,
        ALLE,
        Typ::Wahl(&HARNESSES),
        Vorgabe::Null,
        Richtung::Frei,
    ),
    e(
        "sitzung.modell",
        Bereich::Nutzer,
        ALLE,
        Typ::Modell,
        Vorgabe::Null,
        Richtung::Frei,
    ),
    e(
        "sitzung.effort",
        Bereich::Nutzer,
        ALLE,
        Typ::Wahl(&ALLE_EFFORTS),
        Vorgabe::Null,
        Richtung::Frei,
    ),
    e(
        "harnesses",
        Bereich::Admin,
        ALLE,
        Typ::Klassenliste(Element::Harness),
        Vorgabe::Objekt,
        Richtung::Schnitt,
    ),
    e(
        "modelle",
        Bereich::Admin,
        ALLE,
        Typ::Klassenliste(Element::Modell),
        Vorgabe::Objekt,
        Richtung::Schnitt,
    ),
    e(
        "effort.grenzen",
        Bereich::Admin,
        NUR_FIRMA,
        Typ::Grenzen,
        Vorgabe::Liste,
        Richtung::NurFirma,
    ),
    e(
        "optionen.copilot_ohne_rueckfrage",
        Bereich::Admin,
        ALLE,
        Typ::Schalter,
        Vorgabe::Aus,
        Richtung::AusStrenger,
    ),
    e(
        "optionen.herabstufen",
        Bereich::Admin,
        ALLE,
        Typ::Schalter,
        Vorgabe::Aus,
        Richtung::AusStrenger,
    ),
    e(
        "optionen.lokale_ca",
        Bereich::Admin,
        NUR_FIRMA,
        Typ::Schalter,
        Vorgabe::Aus,
        Richtung::NurFirma,
    ),
    e(
        "optionen.copilot_telemetrie",
        Bereich::Admin,
        ALLE,
        Typ::Schalter,
        Vorgabe::Aus,
        Richtung::AusStrenger,
    ),
    e(
        "optionen.copilot_eingeschraenktes_token",
        Bereich::Admin,
        NUR_FIRMA,
        Typ::Schalter,
        Vorgabe::Aus,
        Richtung::NurFirma,
    ),
    e(
        "optionen.herstellerzugang",
        Bereich::Admin,
        &[],
        Typ::Schalter,
        Vorgabe::Aus,
        Richtung::Fest,
    ),
    e(
        "leseklassen",
        Bereich::Admin,
        NUR_FIRMA,
        Typ::Leseklassen,
        Vorgabe::Liste,
        Richtung::NurFirma,
    ),
];

/// Name of the key `leseklassen`.
pub const LESEKLASSEN: &str = "leseklassen";

/// The line of the catalogue for `schluessel`, `None` for a key that is not in it.
#[must_use]
pub fn eintrag(schluessel: &str) -> Option<&'static Eintrag> {
    KATALOG.iter().find(|e| e.schluessel == schluessel)
}

/// `true` for a model name: 1 to 128 characters from `A-Z a-z 0-9 . _ : / @ + -`.
#[must_use]
pub fn ist_modell(s: &str) -> bool {
    (1..=128).contains(&s.len())
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._:/@+-".contains(&b))
}

/// `true` for a key in identifier form: `^[a-z0-9][a-z0-9._-]{0,63}$`.
#[must_use]
pub fn ist_kennung(s: &str) -> bool {
    let mut b = s.bytes();
    let Some(erstes) = b.next() else {
        return false;
    };
    s.len() <= 64
        && (erstes.is_ascii_lowercase() || erstes.is_ascii_digit())
        && b.all(|x| x.is_ascii_lowercase() || x.is_ascii_digit() || b"._-".contains(&x))
}

/// `true` for a pseudonym: `ps_` and 32 hex digits in lower case (`^ps_[0-9a-f]{32}$`).
#[must_use]
pub fn ist_pseudonym(s: &str) -> bool {
    s.strip_prefix("ps_").is_some_and(|rest| {
        rest.len() == 32
            && rest
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    })
}

fn ist_ganzzahl_in(w: &Value, von: i64, bis: i64) -> bool {
    w.as_i64().is_some_and(|z| (von..=bis).contains(&z))
}

/// `true` if every element of `liste` is a text, every text occurs once, and every text satisfies `gut`.
fn texte_ohne_doppel(liste: &[Value], gut: impl Fn(&str) -> bool) -> bool {
    let mut gesehen: Vec<&str> = Vec::with_capacity(liste.len());
    for x in liste {
        let Some(t) = x.as_str() else {
            return false;
        };
        if !gut(t) || gesehen.contains(&t) {
            return false;
        }
        gesehen.push(t);
    }
    true
}

fn klassenliste_gut(wert: &Value, element: Element, klassen: &[String]) -> bool {
    let Some(o) = wert.as_object() else {
        return false;
    };
    o.iter().all(|(k, liste)| {
        klassen.contains(k)
            && liste.as_array().is_some_and(|l| {
                l.len() <= KLASSENLISTE_MAX
                    && texte_ohne_doppel(l, |t| match element {
                        Element::Harness => HARNESSES.contains(&t),
                        Element::Modell => ist_modell(t),
                    })
            })
    })
}

fn efforts_von(harness: &str) -> &'static [&'static str] {
    EFFORTS
        .iter()
        .find(|(h, _)| *h == harness)
        .map_or(&[], |(_, w)| w)
}

fn grenzen_gut(wert: &Value, klassen: &[String]) -> bool {
    let Some(liste) = wert.as_array() else {
        return false;
    };
    if liste.len() > GRENZEN_MAX {
        return false;
    }
    let mut gesehen: Vec<(&str, Option<&str>, Option<&str>)> = Vec::with_capacity(liste.len());
    for g in liste {
        let Some(g) = g.as_object() else {
            return false;
        };
        if g.keys()
            .any(|k| !matches!(k.as_str(), "harness" | "klasse" | "modell" | "erlaubt"))
        {
            return false;
        }
        let Some(h) = g.get("harness").and_then(Value::as_str) else {
            return false;
        };
        if !HARNESSES.contains(&h) {
            return false;
        }
        let klasse = match g.get("klasse") {
            None => None,
            Some(k) => match k.as_str() {
                Some(k) if klassen.iter().any(|x| x == k) => Some(k),
                _ => return false,
            },
        };
        let modell = match g.get("modell") {
            None => None,
            Some(m) => match m.as_str() {
                Some(m) if ist_modell(m) => Some(m),
                _ => return false,
            },
        };
        let Some(erlaubt) = g.get("erlaubt").and_then(Value::as_array) else {
            return false;
        };
        let werte = efforts_von(h);
        if !(1..=werte.len()).contains(&erlaubt.len())
            || !texte_ohne_doppel(erlaubt, |t| werte.contains(&t))
        {
            return false;
        }
        let schluessel = (h, klasse, modell);
        if gesehen.contains(&schluessel) {
            return false;
        }
        gesehen.push(schluessel);
    }
    true
}

/// The form of `leseklassen` with pseudonyms (15.11): at most 4096 objects with exactly `ebene`, `wer` and `klasse`
/// as texts; `ebene` `gruppe` with a key of `gruppen`, `ebene` `nutzer` with a pseudonym; `klasse` byte for byte in
/// `klassen`; each pair of `ebene` and `wer` once.
#[must_use]
pub fn leseklassen_gut(
    wert: &Value,
    klassen: &[String],
    gruppen: &[(String, Vec<String>)],
) -> bool {
    let Some(liste) = wert.as_array() else {
        return false;
    };
    if liste.len() > LESEKLASSEN_MAX {
        return false;
    }
    let mut paare: Vec<(&str, &str)> = Vec::with_capacity(liste.len());
    for x in liste {
        let Some(x) = x.as_object() else {
            return false;
        };
        if x.len() != 3 {
            return false;
        }
        let (Some(ebene), Some(wer), Some(klasse)) = (
            x.get("ebene").and_then(Value::as_str),
            x.get("wer").and_then(Value::as_str),
            x.get("klasse").and_then(Value::as_str),
        ) else {
            return false;
        };
        let wer_gut = match ebene {
            "gruppe" => ist_kennung(wer) && gruppen.iter().any(|(g, _)| g == wer),
            "nutzer" => ist_pseudonym(wer),
            _ => false,
        };
        if !wer_gut || !klassen.iter().any(|k| k == klasse) || paare.contains(&(ebene, wer)) {
            return false;
        }
        paare.push((ebene, wer));
    }
    true
}

/// `true` if `wert` (not `null`) has type, values and limits of the line `e` (stage 7 of 15.4).
#[must_use]
pub fn wert_gueltig(
    e: &Eintrag,
    wert: &Value,
    klassen: &[String],
    gruppen: &[(String, Vec<String>)],
) -> bool {
    match e.typ {
        Typ::Wahl(werte) => wert.as_str().is_some_and(|t| werte.contains(&t)),
        Typ::Ganzzahl(von, bis) => ist_ganzzahl_in(wert, von, bis),
        Typ::Modell => wert.as_str().is_some_and(ist_modell),
        Typ::Schalter => wert.is_boolean(),
        Typ::Klassenliste(element) => klassenliste_gut(wert, element, klassen),
        Typ::Grenzen => grenzen_gut(wert, klassen),
        Typ::Leseklassen => leseklassen_gut(wert, klassen, gruppen),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn klassen() -> Vec<String> {
        ["oeffentlich", "intern", "kundendaten"]
            .map(String::from)
            .to_vec()
    }

    fn gruppen() -> Vec<(String, Vec<String>)> {
        vec![("g-eins".to_owned(), vec![])]
    }

    fn gueltig(schluessel: &str, w: &Value) -> bool {
        wert_gueltig(eintrag(schluessel).expect("key"), w, &klassen(), &gruppen())
    }

    #[test]
    fn katalog_wie_15_1() {
        let namen: Vec<&str> = KATALOG.iter().map(|e| e.schluessel).collect();
        assert_eq!(namen.len(), 17);
        assert!(eintrag("admins").is_none());
        assert_eq!(Bereich::Nutzer.text(), "nutzer");
        assert_eq!(Bereich::Admin.text(), "admin");
        assert_eq!(
            [Platz::Firma, Platz::Gruppe, Platz::Nutzer].map(Platz::text),
            ["firma", "gruppe", "nutzer"]
        );
        let mut alle: Vec<&str> = EFFORTS
            .iter()
            .flat_map(|(_, w)| w.iter().copied())
            .collect();
        alle.sort_unstable();
        alle.dedup();
        assert_eq!(alle, ALLE_EFFORTS);
        assert_eq!(Vorgabe::Zahl(13).wert(), json!(13));
        assert_eq!(Vorgabe::Objekt.wert(), json!({}));
    }

    #[test]
    fn formen() {
        for gut in ["a", "A.b_c:d/e@f+g-h0", &"x".repeat(128)] {
            assert!(ist_modell(gut), "{gut}");
        }
        for schlecht in ["", "a b", "ä", &"x".repeat(129), "a\n"] {
            assert!(!ist_modell(schlecht), "{schlecht:?}");
        }
        for gut in ["g", "0", "g-eins.zwei_3", &"g".repeat(64)] {
            assert!(ist_kennung(gut), "{gut}");
        }
        for schlecht in [
            "",
            "-g",
            ".g",
            "_g",
            "G",
            "g eins",
            &"g".repeat(65),
            "g\n",
            "g/x",
        ] {
            assert!(!ist_kennung(schlecht), "{schlecht:?}");
        }
        assert!(ist_pseudonym("ps_0123456789abcdef0123456789abcdef"));
        for schlecht in [
            "ps_0123456789abcdef0123456789abcde",
            "ps_0123456789abcdef0123456789abcdef0",
            "ps_0123456789ABCDEF0123456789abcdef",
            "ps_0123456789abcdef0123456789abcdeg",
            "px_0123456789abcdef0123456789abcdef",
            "konto-beispiel",
        ] {
            assert!(!ist_pseudonym(schlecht), "{schlecht}");
        }
    }

    #[test]
    fn werte() {
        assert!(gueltig("terminal.schriftgroesse", &json!(9)));
        assert!(gueltig("terminal.schriftgroesse", &json!(32)));
        for w in [json!(8), json!(33), json!(13.0), json!("13"), json!(true)] {
            assert!(!gueltig("terminal.schriftgroesse", &w), "{w}");
        }
        assert!(gueltig("sitzung.effort", &json!("none")));
        assert!(!gueltig("sitzung.effort", &json!("ultra")));
        assert!(gueltig("optionen.herabstufen", &json!(false)));
        assert!(!gueltig("optionen.herabstufen", &json!(0)));
        assert!(gueltig("harnesses", &json!({"intern": ["pi", "claude"]})));
        for w in [
            json!({"geheim": ["pi"]}),
            json!({"intern": ["pi", "pi"]}),
            json!({"intern": ["vi"]}),
            json!({"intern": "pi"}),
            json!(["pi"]),
            json!({"intern": [1]}),
        ] {
            assert!(!gueltig("harnesses", &w), "{w}");
        }
        let siebzehn: Vec<String> = (0..17).map(|i| format!("m{i}")).collect();
        assert!(gueltig("modelle", &json!({"intern": siebzehn[..16]})));
        assert!(!gueltig("modelle", &json!({"intern": siebzehn})));
        assert!(!gueltig("modelle", &json!({"intern": ["a b"]})));
    }

    #[test]
    fn grenzen() {
        let g = |w: Value| gueltig("effort.grenzen", &w);
        assert!(g(json!([
            {"harness": "copilot", "erlaubt": ["low"]},
            {"harness": "copilot", "klasse": "intern", "erlaubt": ["none"]},
            {"harness": "pi", "modell": "qwen3:8b", "erlaubt": ["off", "low"]},
            {"harness": "claude", "erlaubt": ["low", "medium", "high", "xhigh", "max"]}
        ])));
        for w in [
            json!({}),
            json!([1]),
            json!([{"harness": "copilot"}]),
            json!([{"erlaubt": ["low"]}]),
            json!([{"harness": "vi", "erlaubt": ["low"]}]),
            json!([{"harness": "claude", "erlaubt": ["none"]}]),
            json!([{"harness": "claude", "erlaubt": []}]),
            json!([{"harness": "claude", "erlaubt": ["low", "low"]}]),
            json!([{"harness": "claude", "erlaubt": "low"}]),
            json!([{"harness": "claude", "klasse": "geheim", "erlaubt": ["low"]}]),
            json!([{"harness": "claude", "klasse": 1, "erlaubt": ["low"]}]),
            json!([{"harness": "claude", "modell": "a b", "erlaubt": ["low"]}]),
            json!([{"harness": "claude", "modell": 1, "erlaubt": ["low"]}]),
            json!([{"harness": "claude", "erlaubt": ["low"], "route": "r"}]),
            json!([{"harness": "claude", "erlaubt": ["low"]}, {"harness": "claude", "erlaubt": ["max"]}]),
        ] {
            assert!(!g(w.clone()), "{w}");
        }
        let viele: Vec<Value> = (0..65)
            .map(|i| json!({"harness": "pi", "modell": format!("m{i}"), "erlaubt": ["off"]}))
            .collect();
        assert!(g(Value::Array(viele[..64].to_vec())));
        assert!(!g(Value::Array(viele)));
    }

    #[test]
    fn leseklassen() {
        let p = "ps_00000000000000000000000000000c01";
        let l = |w: Value| gueltig(LESEKLASSEN, &w);
        assert!(l(json!([])));
        assert!(l(json!([
            {"ebene": "gruppe", "wer": "g-eins", "klasse": "intern"},
            {"ebene": "nutzer", "wer": p, "klasse": "oeffentlich"}
        ])));
        for w in [
            json!(null),
            json!({}),
            json!([null]),
            json!([{"ebene": "gruppe", "wer": "g-zwei", "klasse": "intern"}]),
            json!([{"ebene": "nutzer", "wer": "konto-beispiel", "klasse": "intern"}]),
            json!([{"ebene": "rolle", "wer": p, "klasse": "intern"}]),
            json!([{"ebene": "nutzer", "wer": p, "klasse": "Intern"}]),
            json!([{"ebene": "nutzer", "wer": p}]),
            json!([{"ebene": "nutzer", "wer": p, "klasse": "intern", "kunden": []}]),
            json!([{"ebene": "nutzer", "wer": p, "klasse": null}]),
            json!([{"ebene": "nutzer", "wer": p, "klasse": "intern"}, {"ebene": "nutzer", "wer": p, "klasse": "intern"}]),
        ] {
            assert!(!l(w.clone()), "{w}");
        }
    }
}
