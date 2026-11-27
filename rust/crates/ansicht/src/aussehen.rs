//! The look (OF3, OF4) and what the view remembers per device (L8): theme, colour set of the dark theme, terminal, font size of
//! the terminal, widths of the sidebar and the inspector, the state of the inspector and the order of the sessions.
//!
//! Remembered in `localStorage` of the browser (per device, decision of 26.11.2026 on L8); per person only after the pilot.
//! The storage may be missing or refuse (private window, blocked site data): then every value keeps its default.

use wasm_bindgen::{JsCast, JsValue};

/// The prefix of every key in the storage.
const PRAEFIX: &str = "werkbank.ansicht.";

/// Theme: like the system (default), light or dark (OF3 point 2).
pub const THEMEN: [(&str, &str); 3] = [
    ("system", "Wie das System"),
    ("hell", "Hell"),
    ("dunkel", "Dunkel"),
];
/// Colour set of the dark theme: grey (default, like today) or blue-black (version of 23.11.2026).
pub const FARBSAETZE: [(&str, &str); 2] = [("grau", "Grau"), ("blauschwarz", "Blauschwarz")];
/// Terminal: dark (default, like today), light or like the theme.
pub const TERMINALS: [(&str, &str); 3] = [
    ("dunkel", "Dunkel"),
    ("hell", "Hell"),
    ("thema", "Wie das Thema"),
];
/// Font sizes of the terminal in pixels (OF4 point 2, default 13).
pub const SCHRIFTGROESSEN: [u32; 6] = [11, 12, 13, 14, 15, 16];

/// Limits of the widths (OF2 point 3).
pub const LEISTE_MIN: f64 = 180.0;
/// Upper limit of the sidebar.
pub const LEISTE_MAX: f64 = 400.0;
/// Lower limit of the inspector.
pub const INSPEKTOR_MIN: f64 = 260.0;
/// Upper limit of the inspector.
pub const INSPEKTOR_MAX: f64 = 560.0;
/// The centre keeps at least this width.
pub const MITTE_MIN: f64 = 480.0;

/// What the view remembers per device.
#[derive(Clone, Debug, PartialEq)]
pub struct Aussehen {
    /// `system`, `hell` or `dunkel`.
    pub thema: String,
    /// `grau` or `blauschwarz`.
    pub farbsatz: String,
    /// `dunkel`, `hell` or `thema`.
    pub terminal: String,
    /// Font size of the terminal in pixels.
    pub schrift: u32,
    /// Width of the sidebar in pixels.
    pub leiste: f64,
    /// Width of the inspector in pixels.
    pub inspektor_breite: f64,
    /// Whether the inspector is open (default closed, G5).
    pub inspektor_offen: bool,
    /// Whether the sidebar is shown.
    pub leiste_offen: bool,
    /// The order of the sessions, by id (B19).
    pub reihenfolge: Vec<String>,
}

impl Default for Aussehen {
    fn default() -> Self {
        Aussehen {
            thema: "system".into(),
            farbsatz: "grau".into(),
            terminal: "dunkel".into(),
            schrift: 13,
            leiste: 240.0,
            inspektor_breite: 320.0,
            inspektor_offen: false,
            leiste_offen: true,
            reihenfolge: Vec::new(),
        }
    }
}

fn speicher() -> Option<JsValue> {
    let w = web_sys::window()?;
    js_sys::Reflect::get(&w, &JsValue::from_str("localStorage"))
        .ok()
        .filter(|s| !s.is_null() && !s.is_undefined())
}

fn aufruf(name: &str, args: &[&str]) -> Option<JsValue> {
    let s = speicher()?;
    let f = js_sys::Reflect::get(&s, &JsValue::from_str(name))
        .ok()?
        .dyn_into::<js_sys::Function>()
        .ok()?;
    match args {
        [a] => f.call1(&s, &JsValue::from_str(a)).ok(),
        [a, b] => f
            .call2(&s, &JsValue::from_str(a), &JsValue::from_str(b))
            .ok(),
        _ => None,
    }
}

fn lesen(k: &str) -> Option<String> {
    aufruf("getItem", &[&format!("{PRAEFIX}{k}")])?.as_string()
}

fn schreiben(k: &str, v: &str) {
    let _ = aufruf("setItem", &[&format!("{PRAEFIX}{k}"), v]);
}

fn eins_aus(wert: Option<String>, erlaubt: &[(&str, &str)], vorgabe: &str) -> String {
    wert.filter(|w| erlaubt.iter().any(|(k, _)| k == w))
        .unwrap_or_else(|| vorgabe.to_owned())
}

/// Clamps a width into its limits.
#[must_use]
pub fn klemmen(w: f64, min: f64, max: f64) -> f64 {
    if w.is_finite() {
        w.clamp(min, max)
    } else {
        min
    }
}

impl Aussehen {
    /// The remembered values of this device; defaults for everything missing or invalid.
    #[must_use]
    pub fn laden() -> Aussehen {
        let v = Aussehen::default();
        let zahl = |k: &str| lesen(k).and_then(|s| s.parse::<f64>().ok());
        Aussehen {
            thema: eins_aus(lesen("thema"), &THEMEN, &v.thema),
            farbsatz: eins_aus(lesen("farbsatz"), &FARBSAETZE, &v.farbsatz),
            terminal: eins_aus(lesen("terminal"), &TERMINALS, &v.terminal),
            schrift: lesen("schrift")
                .and_then(|s| s.parse().ok())
                .filter(|s| SCHRIFTGROESSEN.contains(s))
                .unwrap_or(v.schrift),
            leiste: zahl("leiste").map_or(v.leiste, |w| klemmen(w, LEISTE_MIN, LEISTE_MAX)),
            inspektor_breite: zahl("inspektor_breite").map_or(v.inspektor_breite, |w| {
                klemmen(w, INSPEKTOR_MIN, INSPEKTOR_MAX)
            }),
            inspektor_offen: lesen("inspektor_offen").as_deref() == Some("1"),
            leiste_offen: lesen("leiste_offen").as_deref() != Some("0"),
            reihenfolge: lesen("reihenfolge")
                .map(|s| {
                    s.split(',')
                        .filter(|x| !x.is_empty())
                        .map(str::to_owned)
                        .collect()
                })
                .unwrap_or_default(),
        }
    }

    /// Remembers all values on this device and sets the look on the document.
    pub fn merken(&self) {
        schreiben("thema", &self.thema);
        schreiben("farbsatz", &self.farbsatz);
        schreiben("terminal", &self.terminal);
        schreiben("schrift", &self.schrift.to_string());
        schreiben("leiste", &format!("{:.0}", self.leiste));
        schreiben("inspektor_breite", &format!("{:.0}", self.inspektor_breite));
        schreiben(
            "inspektor_offen",
            if self.inspektor_offen { "1" } else { "0" },
        );
        schreiben("leiste_offen", if self.leiste_offen { "1" } else { "0" });
        schreiben("reihenfolge", &self.reihenfolge.join(","));
        self.anwenden();
    }

    /// Sets theme, colour set and terminal as attributes of the root element (`ansicht.css` reads them).
    pub fn anwenden(&self) {
        let Some(e) = web_sys::window()
            .and_then(|w| w.document())
            .and_then(|d| d.document_element())
        else {
            return;
        };
        let _ = e.set_attribute("data-thema", &self.thema);
        let _ = e.set_attribute("data-farbsatz", &self.farbsatz);
        let _ = e.set_attribute("data-terminal", &self.terminal);
        // macOS: the frame of the active tile takes the accent colour of the system (`AccentColor`, `ansicht.css`)
        let _ = e.set_attribute(
            "data-system",
            if ist_mac(&plattform()) {
                "mac"
            } else {
                "anders"
            },
        );
    }

    /// The height of one terminal line for the font size (13 px gives 17 px).
    #[must_use]
    pub fn zeilenhoehe(&self) -> usize {
        self.schrift as usize + 4
    }

    /// Sorts sessions by the remembered order; unknown ones keep their list order behind the known ones.
    #[must_use]
    pub fn sortiert<'a, T>(&self, liste: &'a [T], id: impl Fn(&T) -> &str) -> Vec<&'a T> {
        let mut v: Vec<&T> = liste.iter().collect();
        v.sort_by_key(|x| {
            self.reihenfolge
                .iter()
                .position(|r| r == id(x))
                .unwrap_or(usize::MAX)
        });
        v
    }

    /// Moves the session `von` in front of `vor` (B19).
    pub fn verschieben(&mut self, alle: &[String], von: &str, vor: &str) {
        let mut r: Vec<String> = self
            .reihenfolge
            .iter()
            .filter(|x| alle.contains(x))
            .cloned()
            .collect();
        for a in alle {
            if !r.contains(a) {
                r.push(a.clone());
            }
        }
        r.retain(|x| x != von);
        let i = r.iter().position(|x| x == vor).unwrap_or(r.len());
        r.insert(i, von.to_owned());
        self.reihenfolge = r;
    }
}

/// The platform of the browser: `navigator.userAgentData.platform` where there is one, else `navigator.platform`; empty
/// outside a browser. Read over `Reflect`, so that no further web-sys feature is needed.
fn plattform() -> String {
    let Some(w) = web_sys::window() else {
        return String::new();
    };
    let lies = |o: &JsValue, k: &str| js_sys::Reflect::get(o, &JsValue::from_str(k)).ok();
    let Some(nav) = lies(&w, "navigator") else {
        return String::new();
    };
    lies(&nav, "userAgentData")
        .filter(JsValue::is_object)
        .and_then(|d| lies(&d, "platform"))
        .and_then(|p| p.as_string())
        .filter(|p| !p.is_empty())
        .or_else(|| lies(&nav, "platform").and_then(|p| p.as_string()))
        .unwrap_or_default()
}

/// Whether the platform is macOS (`macOS` from `userAgentData`, `MacIntel` from `navigator.platform`).
#[must_use]
pub fn ist_mac(plattform: &str) -> bool {
    plattform.to_ascii_lowercase().starts_with("mac")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn widths_are_clamped_and_order_moves() {
        assert!((klemmen(50.0, LEISTE_MIN, LEISTE_MAX) - 180.0).abs() < f64::EPSILON);
        assert!((klemmen(f64::NAN, INSPEKTOR_MIN, INSPEKTOR_MAX) - 260.0).abs() < f64::EPSILON);
        let mut a = Aussehen::default();
        let alle = vec!["s-1".to_owned(), "s-2".to_owned(), "s-3".to_owned()];
        a.verschieben(&alle, "s-3", "s-1");
        assert_eq!(a.reihenfolge, vec!["s-3", "s-1", "s-2"]);
        let l = ["s-1", "s-2", "s-3"];
        let s: Vec<&&str> = a.sortiert(&l, |x| *x);
        assert_eq!(s, vec![&"s-3", &"s-1", &"s-2"]);
        assert_eq!(a.zeilenhoehe(), 17);
    }

    #[test]
    fn only_macos_counts_as_mac() {
        assert!(ist_mac("macOS"));
        assert!(ist_mac("MacIntel"));
        assert!(!ist_mac("Windows"));
        assert!(!ist_mac("Linux x86_64"));
        assert!(!ist_mac(""));
    }
}
