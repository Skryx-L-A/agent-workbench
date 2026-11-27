//! What the window remembers per device (gap L8, OF14 point 8): size, position, the widths of sidebar and inspector,
//! whether the inspector is open, the order of the sessions per project, and the look chosen on this device while the
//! settings of the service (L4) are missing. One JSON file of the user.

use std::path::{Path, PathBuf};

use serde_json::{Map, Value, json};

use crate::gestalt::MASSE;

/// The remembered values.
#[derive(Clone, Debug, PartialEq)]
pub struct Merken {
    /// Inner size of the window in points.
    pub groesse: Option<(f32, f32)>,
    /// Position of the window on the screen in points.
    pub lage: Option<(f32, f32)>,
    /// Width of the sidebar.
    pub leiste: f32,
    /// Width of the inspector.
    pub inspektor: f32,
    /// Whether the inspector is open (default closed, G5).
    pub inspektor_offen: bool,
    /// The order of the sessions per project: project name and the session ids.
    pub reihenfolge: Vec<(String, Vec<String>)>,
    /// The look chosen on this device (key `aussehen.*` and value as the settings write it), base form of B72.
    pub aussehen: Vec<(String, String)>,
}

impl Default for Merken {
    fn default() -> Self {
        Merken {
            groesse: None,
            lage: None,
            leiste: MASSE.leiste,
            inspektor: MASSE.inspektor,
            inspektor_offen: false,
            reihenfolge: Vec::new(),
            aussehen: Vec::new(),
        }
    }
}

/// The file of the user: `$XDG_CONFIG_HOME/werkbank/fenster.json` (else `~/.config/…`) under Linux,
/// `%APPDATA%\Werkbank\fenster.json` under Windows, `~/Library/Application Support/Werkbank/fenster.json` under macOS.
#[must_use]
pub fn datei() -> Option<PathBuf> {
    let var = |n: &str| {
        std::env::var_os(n)
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
    };
    if cfg!(windows) {
        var("APPDATA").map(|p| p.join("Werkbank").join("fenster.json"))
    } else if cfg!(target_os = "macos") {
        var("HOME").map(|p| {
            p.join("Library")
                .join("Application Support")
                .join("Werkbank")
                .join("fenster.json")
        })
    } else {
        var("XDG_CONFIG_HOME")
            .or_else(|| var("HOME").map(|h| h.join(".config")))
            .map(|p| p.join("werkbank").join("fenster.json"))
    }
}

fn paar(v: Option<&Value>) -> Option<(f32, f32)> {
    let a = v?.as_array()?;
    // the values are sizes of a screen; f32 is enough
    #[allow(clippy::cast_possible_truncation)]
    match a.as_slice() {
        [x, y] => Some((x.as_f64()? as f32, y.as_f64()? as f32)),
        _ => None,
    }
}

impl Merken {
    /// Reads the file; a missing or broken file gives the defaults, a value outside its bounds is clamped.
    #[must_use]
    pub fn lesen(pfad: &Path) -> Merken {
        let Ok(text) = std::fs::read_to_string(pfad) else {
            return Merken::default();
        };
        let Ok(Value::Object(m)) = serde_json::from_str::<Value>(&text) else {
            return Merken::default();
        };
        let mut aus = Merken {
            groesse: paar(m.get("groesse")).filter(|(b, h)| *b >= 200.0 && *h >= 150.0),
            lage: paar(m.get("lage")),
            ..Merken::default()
        };
        #[allow(clippy::cast_possible_truncation)]
        let zahl = |k: &str| m.get(k).and_then(Value::as_f64).map(|x| x as f32);
        if let Some(l) = zahl("leiste") {
            aus.leiste = l.clamp(MASSE.leiste_grenzen.0, MASSE.leiste_grenzen.1);
        }
        if let Some(i) = zahl("inspektor") {
            aus.inspektor = i.clamp(MASSE.inspektor_grenzen.0, MASSE.inspektor_grenzen.1);
        }
        aus.inspektor_offen = m
            .get("inspektor_offen")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        if let Some(Value::Object(r)) = m.get("reihenfolge") {
            aus.reihenfolge = r
                .iter()
                .map(|(p, l)| {
                    (
                        p.clone(),
                        l.as_array()
                            .map(|a| {
                                a.iter()
                                    .filter_map(Value::as_str)
                                    .map(str::to_owned)
                                    .collect()
                            })
                            .unwrap_or_default(),
                    )
                })
                .collect();
        }
        if let Some(Value::Object(a)) = m.get("aussehen") {
            aus.aussehen = a
                .iter()
                .filter(|(k, _)| k.starts_with("aussehen."))
                .filter_map(|(k, v)| v.as_str().map(|v| (k.clone(), v.to_owned())))
                .collect();
        }
        aus
    }

    /// Remembers a look choice of this device.
    pub fn aussehen_setzen(&mut self, schluessel: &str, wert: &str) {
        match self.aussehen.iter_mut().find(|(k, _)| k == schluessel) {
            Some(e) => wert.clone_into(&mut e.1),
            None => self.aussehen.push((schluessel.to_owned(), wert.to_owned())),
        }
    }

    /// The values as JSON.
    #[must_use]
    pub fn als_wert(&self) -> Value {
        let mut r = Map::new();
        for (p, l) in &self.reihenfolge {
            r.insert(p.clone(), json!(l));
        }
        let mut a = Map::new();
        for (k, v) in &self.aussehen {
            a.insert(k.clone(), json!(v));
        }
        json!({
            "aussehen": a,
            "groesse": self.groesse.map(|(b, h)| json!([b, h])),
            "lage": self.lage.map(|(x, y)| json!([x, y])),
            "leiste": self.leiste,
            "inspektor": self.inspektor,
            "inspektor_offen": self.inspektor_offen,
            "reihenfolge": r,
        })
    }

    /// Writes the file (the folder is made if needed); written to a neighbour first and renamed, so a broken write
    /// leaves the old file.
    ///
    /// # Errors
    ///
    /// Every error of the file system.
    pub fn schreiben(&self, pfad: &Path) -> std::io::Result<()> {
        if let Some(ordner) = pfad.parent() {
            std::fs::create_dir_all(ordner)?;
        }
        let neben = pfad.with_extension("json.neu");
        std::fs::write(&neben, self.als_wert().to_string())?;
        std::fs::rename(&neben, pfad)
    }

    /// The order of the sessions `ids` of project `projekt`: remembered ones first in their order, new ones after them.
    #[must_use]
    pub fn ordnen(&self, projekt: &str, ids: &[String]) -> Vec<String> {
        let gemerkt = self
            .reihenfolge
            .iter()
            .find(|(p, _)| p == projekt)
            .map(|(_, l)| l.as_slice())
            .unwrap_or_default();
        let mut aus: Vec<String> = gemerkt
            .iter()
            .filter(|g| ids.contains(g))
            .cloned()
            .collect();
        aus.extend(ids.iter().filter(|i| !gemerkt.contains(i)).cloned());
        aus
    }

    /// Moves session `id` of project `projekt` to position `nach` (B19).
    pub fn verschieben(&mut self, projekt: &str, ids: &[String], id: &str, nach: usize) {
        let mut liste = self.ordnen(projekt, ids);
        if let Some(pos) = liste.iter().position(|x| x == id) {
            let x = liste.remove(pos);
            liste.insert(nach.min(liste.len()), x);
        }
        self.reihenfolge.retain(|(p, _)| p != projekt);
        self.reihenfolge.push((projekt.to_owned(), liste));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_written_file_reads_back_the_same() {
        let ordner = std::env::temp_dir().join(format!("fenster-merken-{}", std::process::id()));
        let pfad = ordner.join("fenster.json");
        let mut m = Merken {
            groesse: Some((1280.0, 800.0)),
            lage: Some((40.0, 30.0)),
            leiste: 250.0,
            inspektor: 300.0,
            inspektor_offen: true,
            reihenfolge: Vec::new(),
            aussehen: vec![("aussehen.thema".into(), "hell".into())],
        };
        m.verschieben("werkbank", &["s-1".into(), "s-2".into()], "s-2", 0);
        assert!(m.schreiben(&pfad).is_ok());
        let zurueck = Merken::lesen(&pfad);
        let _ = std::fs::remove_dir_all(&ordner);
        assert_eq!(zurueck, m);
        assert_eq!(
            zurueck.ordnen("werkbank", &["s-1".into(), "s-2".into(), "s-3".into()]),
            ["s-2", "s-1", "s-3"]
        );
    }

    #[test]
    fn widths_outside_their_bounds_are_clamped_and_a_broken_file_gives_defaults() {
        let ordner =
            std::env::temp_dir().join(format!("fenster-merken-grenze-{}", std::process::id()));
        let pfad = ordner.join("fenster.json");
        let _ = std::fs::create_dir_all(&ordner);
        let _ = std::fs::write(&pfad, r#"{"leiste": 10, "inspektor": 9000}"#);
        let m = Merken::lesen(&pfad);
        assert_eq!((m.leiste, m.inspektor), (180.0, 560.0));
        let _ = std::fs::write(&pfad, "kein json");
        assert_eq!(Merken::lesen(&pfad), Merken::default());
        let _ = std::fs::remove_dir_all(&ordner);
    }
}
