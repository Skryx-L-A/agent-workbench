//! The text form of the description: the one input of the fixed starter (see [`crate::starter`]).
//!
//! The JSON object is closed: every field below is required, no other key is allowed, every value has the type named
//! here, and a path is a string. The fields `modell_socket`, `lokal_weiter`, `cloud` and `paket` are required but may be `null`;
//! `dateien` is required and may be empty.
//! There is no field `system`: the starter reads the user and the layout of `/usr` from the host itself, so the
//! caller cannot bind a path of its own through it.
//!
//! ```text
//! { "harness": str, "arbeitsordner": str, "programmordner": [str],
//!   "zustand": {"host": str, "name": str, "art": "neu"|"fortsetzen"},
//!   "heim": str, "such_socket": str, "modell_socket": str|null, "lokal_weiter": str|null,
//!   "cloud": null | {"weiterreicher": str, "weiter_programm": str, "bundle": str, "verteilung": "ubuntu"|"arch"},
//!   "paket": null | {"quelle": str, "plattform": str, "fassung": str},
//!   "dateien": [{"quelle": str, "ziel": str}],
//!   "umgebung": {"home": str, "path": str, "term": str, "lang": str, "tmpdir": str},
//!   "geschuetzt": [str], "argv": [str] }
//! ```

use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::beschreibung::{
    Beschreibung, Cloud, Datei, Paket, System, Umgebung, Verteilung, Zustand,
};
use crate::json::{Wert, lesen, text_schreiben};

/// The most bytes of the text form.
pub const EINGABE_MAX: usize = 65_536;

/// A closed object: takes the fields one by one, and `fertig` fails when one is left over.
pub(crate) struct Felder(BTreeMap<String, Wert>);

impl Felder {
    pub(crate) fn aus(wert: Wert) -> Option<Felder> {
        match wert {
            Wert::Objekt(f) => Some(Felder(f)),
            _ => None,
        }
    }

    pub(crate) fn nehmen(&mut self, name: &str) -> Option<Wert> {
        self.0.remove(name)
    }

    pub(crate) fn text(&mut self, name: &str) -> Option<String> {
        match self.nehmen(name)? {
            Wert::Text(t) => Some(t),
            _ => None,
        }
    }

    pub(crate) fn pfad(&mut self, name: &str) -> Option<PathBuf> {
        self.text(name).map(PathBuf::from)
    }

    pub(crate) fn texte(&mut self, name: &str) -> Option<Vec<String>> {
        match self.nehmen(name)? {
            Wert::Liste(l) => l
                .into_iter()
                .map(|w| match w {
                    Wert::Text(t) => Some(t),
                    _ => None,
                })
                .collect(),
            _ => None,
        }
    }

    pub(crate) fn pfade(&mut self, name: &str) -> Option<Vec<PathBuf>> {
        Some(self.texte(name)?.into_iter().map(PathBuf::from).collect())
    }

    pub(crate) fn unterobjekt(&mut self, name: &str) -> Option<Felder> {
        Felder::aus(self.nehmen(name)?)
    }

    /// `Ok(None)` for `null`, `Ok(Some(_))` for a path, `Err` for a missing field or another type.
    pub(crate) fn pfad_oder_null(&mut self, name: &str) -> Result<Option<PathBuf>, ()> {
        match self.nehmen(name) {
            Some(Wert::Null) => Ok(None),
            Some(Wert::Text(t)) => Ok(Some(PathBuf::from(t))),
            _ => Err(()),
        }
    }

    /// No field is left over.
    pub(crate) fn fertig(self) -> Option<()> {
        self.0.is_empty().then_some(())
    }
}

fn cloud_lesen(wert: Wert) -> Option<Cloud> {
    let mut f = Felder::aus(wert)?;
    let verteilung = match f.text("verteilung")?.as_str() {
        "ubuntu" => Verteilung::Ubuntu,
        "arch" => Verteilung::Arch,
        _ => return None,
    };
    let cloud = Cloud {
        weiterreicher: f.pfad("weiterreicher")?,
        weiter_programm: f.pfad("weiter_programm")?,
        bundle: f.pfad("bundle")?,
        verteilung,
    };
    f.fertig()?;
    Some(cloud)
}

/// `Ok(None)` for `null`, `Ok(Some(_))` for a closed object of the three texts, `Err` for every other value.
fn paket_lesen(wert: Wert) -> Result<Option<Paket>, ()> {
    if wert == Wert::Null {
        return Ok(None);
    }
    let lesen = |wert: Wert| {
        let mut f = Felder::aus(wert)?;
        let p = Paket {
            quelle: f.pfad("quelle")?,
            plattform: f.text("plattform")?,
            fassung: f.text("fassung")?,
        };
        f.fertig()?;
        Some(p)
    };
    lesen(wert).map(Some).ok_or(())
}

fn dateien_lesen(wert: Wert) -> Option<Vec<Datei>> {
    let Wert::Liste(liste) = wert else {
        return None;
    };
    liste
        .into_iter()
        .map(|w| {
            let mut f = Felder::aus(w)?;
            let d = Datei {
                quelle: f.pfad("quelle")?,
                ziel: f.pfad("ziel")?,
            };
            f.fertig()?;
            Some(d)
        })
        .collect()
}

/// Reads the text form into a description. The host part (`system`) comes from the caller of this function, never
/// from the text. `None` for every refusal: no JSON, an unknown key, a missing field, a wrong type.
#[must_use]
pub fn beschreibung_lesen(bytes: &[u8], system: System) -> Option<Beschreibung> {
    if bytes.len() > EINGABE_MAX {
        return None;
    }
    let mut f = Felder::aus(lesen(bytes)?)?;
    let harness = f.text("harness")?;
    let arbeitsordner = f.pfad("arbeitsordner")?;
    let programmordner = f.pfade("programmordner")?;
    let mut z = f.unterobjekt("zustand")?;
    let zustand = Zustand {
        host: z.pfad("host")?,
        name: z.text("name")?,
        fortsetzen: match z.text("art")?.as_str() {
            "neu" => false,
            "fortsetzen" => true,
            _ => return None,
        },
    };
    z.fertig()?;
    let heim = f.pfad("heim")?;
    let such_socket = f.pfad("such_socket")?;
    let modell_socket = f.pfad_oder_null("modell_socket").ok()?;
    let lokal_weiter = f.pfad_oder_null("lokal_weiter").ok()?;
    let cloud = match f.nehmen("cloud")? {
        Wert::Null => None,
        andere => Some(cloud_lesen(andere)?),
    };
    let paket = paket_lesen(f.nehmen("paket")?).ok()?;
    let dateien = dateien_lesen(f.nehmen("dateien")?)?;
    let mut u = f.unterobjekt("umgebung")?;
    let umgebung = Umgebung {
        home: u.text("home")?,
        path: u.text("path")?,
        term: u.text("term")?,
        lang: u.text("lang")?,
        tmpdir: u.text("tmpdir")?,
    };
    u.fertig()?;
    let geschuetzt = f.pfade("geschuetzt")?;
    let argv = f.texte("argv")?;
    f.fertig()?;
    Some(Beschreibung {
        harness,
        arbeitsordner,
        programmordner,
        zustand,
        heim,
        such_socket,
        modell_socket,
        lokal_weiter,
        cloud,
        paket,
        dateien,
        umgebung,
        geschuetzt,
        argv,
        system,
    })
}

fn pfad_text(aus: &mut String, pfad: &std::path::Path) {
    text_schreiben(aus, &pfad.to_string_lossy());
}

fn liste_schreiben<T>(aus: &mut String, liste: &[T], eins: impl Fn(&mut String, &T)) {
    aus.push('[');
    for (i, t) in liste.iter().enumerate() {
        if i > 0 {
            aus.push(',');
        }
        eins(aus, t);
    }
    aus.push(']');
}

fn feld(aus: &mut String, name: &str, erstes: bool) {
    if !erstes {
        aus.push(',');
    }
    text_schreiben(aus, name);
    aus.push(':');
}

/// The word of the field `art` of the state folder: `fortsetzen` for a continued conversation, else `neu`.
fn zustand_art(fortsetzen: bool) -> &'static str {
    if fortsetzen { "fortsetzen" } else { "neu" }
}

/// The text form of a description (the field `system` is left out: the starter reads the host itself). The caller
/// puts these bytes on the descriptor of the starter.
#[must_use]
pub fn beschreibung_schreiben(b: &Beschreibung) -> String {
    let mut s = String::from("{");
    feld(&mut s, "harness", true);
    text_schreiben(&mut s, &b.harness);
    feld(&mut s, "arbeitsordner", false);
    pfad_text(&mut s, &b.arbeitsordner);
    feld(&mut s, "programmordner", false);
    liste_schreiben(&mut s, &b.programmordner, |a, p| pfad_text(a, p));
    feld(&mut s, "zustand", false);
    s.push('{');
    feld(&mut s, "host", true);
    pfad_text(&mut s, &b.zustand.host);
    feld(&mut s, "name", false);
    text_schreiben(&mut s, &b.zustand.name);
    feld(&mut s, "art", false);
    text_schreiben(&mut s, zustand_art(b.zustand.fortsetzen));
    s.push('}');
    feld(&mut s, "heim", false);
    pfad_text(&mut s, &b.heim);
    feld(&mut s, "such_socket", false);
    pfad_text(&mut s, &b.such_socket);
    for (name, wert) in [
        ("modell_socket", &b.modell_socket),
        ("lokal_weiter", &b.lokal_weiter),
    ] {
        feld(&mut s, name, false);
        match wert {
            Some(p) => pfad_text(&mut s, p),
            None => s.push_str("null"),
        }
    }
    feld(&mut s, "cloud", false);
    match &b.cloud {
        Some(c) => {
            s.push('{');
            feld(&mut s, "weiterreicher", true);
            pfad_text(&mut s, &c.weiterreicher);
            feld(&mut s, "weiter_programm", false);
            pfad_text(&mut s, &c.weiter_programm);
            feld(&mut s, "bundle", false);
            pfad_text(&mut s, &c.bundle);
            feld(&mut s, "verteilung", false);
            text_schreiben(
                &mut s,
                match c.verteilung {
                    Verteilung::Ubuntu => "ubuntu",
                    Verteilung::Arch => "arch",
                },
            );
            s.push('}');
        }
        None => s.push_str("null"),
    }
    feld(&mut s, "paket", false);
    match &b.paket {
        Some(p) => {
            s.push('{');
            feld(&mut s, "quelle", true);
            pfad_text(&mut s, &p.quelle);
            feld(&mut s, "plattform", false);
            text_schreiben(&mut s, &p.plattform);
            feld(&mut s, "fassung", false);
            text_schreiben(&mut s, &p.fassung);
            s.push('}');
        }
        None => s.push_str("null"),
    }
    feld(&mut s, "dateien", false);
    liste_schreiben(&mut s, &b.dateien, |a, d| {
        a.push('{');
        feld(a, "quelle", true);
        pfad_text(a, &d.quelle);
        feld(a, "ziel", false);
        pfad_text(a, &d.ziel);
        a.push('}');
    });
    feld(&mut s, "umgebung", false);
    s.push('{');
    let u = &b.umgebung;
    for (i, (name, wert)) in [
        ("home", &u.home),
        ("path", &u.path),
        ("term", &u.term),
        ("lang", &u.lang),
        ("tmpdir", &u.tmpdir),
    ]
    .into_iter()
    .enumerate()
    {
        feld(&mut s, name, i == 0);
        text_schreiben(&mut s, wert);
    }
    s.push('}');
    feld(&mut s, "geschuetzt", false);
    liste_schreiben(&mut s, &b.geschuetzt, |a, p| pfad_text(a, p));
    feld(&mut s, "argv", false);
    liste_schreiben(&mut s, &b.argv, |a, t| text_schreiben(a, t));
    s.push('}');
    s
}

#[cfg(test)]
mod tests;
