//! The state of the view and its rules (contract `seite`). Events in, effects out; no DOM, no clock, no network.
//!
//! The view keeps only what it shows (A118: the logic of the session lives in the core of the service). The frames of
//! the stream are read with [`rasterauszug::Empfang`], the same decoder the rest of the new build uses.

use std::collections::HashMap;

use rasterauszug::{Bild, Empfang, Fehler as RasterFehler};
use serde_json::{Value, json};

use crate::ansagen::{Ansager, Ausgabe};
use crate::start::Startform;
use crate::tasten::Deutung;
use crate::zeile;

/// Texts of rejections without their own field `text` (SE3 point 3); every other reason stands with its code.
const TEXTE: [(&str, &str); 5] = [
    ("taste_unbekannt", "Taste nicht belegt"),
    ("sitzung_beendet", "Sitzung beendet"),
    ("kein_abo", "kein Abo"),
    ("pane_busy", "Dialog offen"),
    ("steuerzeichen_im_text", "Steuerzeichen im Text"),
];

/// Frame kinds of the stream (contract `dienst` section 5).
pub const ART_RASTER: u8 = 5;
/// A message of the core or the service (JSON).
pub const ART_NACHRICHT: u8 = 6;
/// A rejection (JSON).
pub const ART_ABLEHNUNG: u8 = 7;
/// The end of the connection with an error (JSON).
pub const ART_FEHLER: u8 = 8;

/// What the view must do after an event.
#[derive(Clone, Debug, PartialEq)]
pub enum Wirkung {
    /// Send this event of the view (`POST /eingabe`), in order.
    Senden(Value),
    /// Announce this text in the polite live region.
    Ansage(String),
    /// Call [`Kern::wecken`] after this many milliseconds.
    Wecker(f64),
    /// Put the focus into the terminal.
    FokusTerminal,
    /// Put the focus into the first field of the start form.
    FokusStart,
    /// Close the stream (the picture of the connection is broken).
    StromZu,
    /// Show a short note at the bottom of the stage (B67); the live region carries the text for the screen reader.
    Notiz(String),
}

/// One session of the list (`Sitzungsliste`, `dienst.rs:1089`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sitzung {
    /// Id, `s-<n>`.
    pub id: String,
    /// Harness.
    pub harness: String,
    /// Model.
    pub modell: String,
    /// Route.
    pub route: String,
    /// Customer, may be empty.
    pub kunde: String,
    /// Whether it runs.
    pub laeuft: bool,
    /// The exit code as text (when ended).
    pub code: String,
    /// The raster number of its frames (field `nr` of the list, DI12 point 5, P25); `None` once a later session took it.
    pub nr: Option<u8>,
}

impl Sitzung {
    /// "läuft" or "beendet, Code <n>".
    #[must_use]
    pub fn zustand_text(&self) -> String {
        if self.laeuft {
            "läuft".to_owned()
        } else {
            format!("beendet, Code {}", self.code)
        }
    }

    /// Harness and model, as far as known.
    #[must_use]
    pub fn teile(&self) -> Vec<&str> {
        [self.harness.as_str(), self.modell.as_str()]
            .into_iter()
            .filter(|s| !s.is_empty())
            .collect()
    }
}

/// The state of the connection to the bridge.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Verbindung {
    /// Not yet open.
    Warte,
    /// Open.
    Offen,
    /// Closed, with the visible text.
    Zu(&'static str),
}

/// An open dialog of a harness (`Dialog`, `ereignis.rs:547`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Dialog {
    /// Session.
    pub sitzung: String,
    /// Dialog id.
    pub id: String,
    /// Kind of dialog.
    pub art: String,
    /// The options, numbered from 1.
    pub optionen: Vec<String>,
}

/// One open request (`Anfragen`, `kern.rs:482`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Anfrage {
    /// Id.
    pub id: String,
    /// Session.
    pub sitzung: String,
    /// Tool.
    pub werkzeug: String,
    /// Preview of the command.
    pub vorschau: String,
}

/// The state of the view.
pub struct Kern {
    /// The sessions of the last list, in list order.
    pub sitzungen: Vec<Sitzung>,
    bekannt: HashMap<String, bool>,
    erste_liste: bool,
    /// The subscribed session.
    pub aktiv: Option<String>,
    gemeldet: HashMap<String, (usize, usize)>,
    /// The error field: code and text of the last rejection (SE7 point 5).
    pub fehler: Option<(String, String)>,
    /// The connection.
    pub verbindung: Verbindung,
    /// The start form.
    pub start: Startform,
    empfang: Option<Empfang>,
    /// The picture of the subscribed session.
    pub bild: Option<Bild>,
    ansager: Ansager,
    /// Open requests (B03, B06, B56).
    pub anfragen: Vec<Anfrage>,
    /// Workers seen (B22, B26, B29): id and state.
    pub agenten: Vec<(String, String)>,
    /// The settings of the person, as sent (B23, B72).
    pub einstellungen: Option<Value>,
    /// The routes, as sent (B69).
    pub routen: Option<Value>,
    /// Open dialogs of harnesses (B42).
    pub dialoge: Vec<Dialog>,
}

impl Default for Kern {
    fn default() -> Self {
        Kern {
            sitzungen: Vec::new(),
            bekannt: HashMap::new(),
            erste_liste: true,
            aktiv: None,
            gemeldet: HashMap::new(),
            fehler: None,
            verbindung: Verbindung::Warte,
            start: Startform::default(),
            empfang: None,
            bild: None,
            ansager: Ansager::default(),
            anfragen: Vec::new(),
            agenten: Vec::new(),
            einstellungen: None,
            routen: None,
            dialoge: Vec::new(),
        }
    }
}

/// Columns and rows from the area of the raster and the size of one character, clamped to 2 to 1000 columns and
/// 1 to 1000 rows (SE5 point 2). A missing character size counts as 8 by 17 pixels.
#[must_use]
pub fn klemme(breite: f64, hoehe: f64, zeichenbreite: f64, zeilenhoehe: f64) -> (usize, usize) {
    let zw = if zeichenbreite > 0.0 {
        zeichenbreite
    } else {
        8.0
    };
    let zh = if zeilenhoehe > 0.0 { zeilenhoehe } else { 17.0 };
    // the value is finite, at least 1 and at most 1000 before the cast
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let ganz = |x: f64, min: usize| {
        if x.is_finite() && x >= 1.0 {
            (x.floor().min(1000.0) as usize).max(min)
        } else {
            min
        }
    };
    (ganz(breite / zw, 2), ganz(hoehe / zh, 1))
}

/// The raster number of the session `id` in the list (DI12 point 5, P25): the field `nr`, never the digits of the identifier.
fn nr_der(sitzungen: &[Sitzung], id: &str) -> Option<u8> {
    sitzungen.iter().find(|s| s.id == id).and_then(|s| s.nr)
}

fn feld(v: &Value, name: &str) -> String {
    match v.get(name) {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Number(n)) => n.to_string(),
        Some(Value::Bool(b)) => b.to_string(),
        _ => String::new(),
    }
}

impl Kern {
    /// The stream is open: `X-Verbindung` is known, `X-Deflate` says whether raster payloads are packed.
    pub fn offen(&mut self, deflate: bool) {
        self.empfang = Some(Empfang::neu(deflate));
        self.verbindung = Verbindung::Offen;
    }

    /// The stream ended.
    pub fn ende(&mut self) -> Vec<Wirkung> {
        self.empfang = None;
        self.verbindung = Verbindung::Zu("getrennt, Seite neu laden");
        vec![Wirkung::Ansage("Verbindung getrennt".into())]
    }

    /// Without a token in the fragment the view calls nothing.
    pub fn ohne_token(&mut self) {
        self.verbindung = Verbindung::Zu("Adresse ohne Zugangsschlüssel");
    }

    /// One frame of the stream. `groesse` is the measured size of the terminal (for the first subscription).
    pub fn rahmen(
        &mut self,
        art: u8,
        nutz: &[u8],
        jetzt: f64,
        groesse: (usize, usize),
    ) -> Vec<Wirkung> {
        if art == ART_RASTER {
            return self.raster(nutz, jetzt);
        }
        let Ok(j) = serde_json::from_slice::<Value>(nutz) else {
            return Vec::new();
        };
        match art {
            ART_NACHRICHT => self.nachricht(&j, groesse),
            ART_ABLEHNUNG => self.ablehnung(&j),
            ART_FEHLER => {
                if let Some(code) = j.get("fehler").and_then(Value::as_str) {
                    self.fehler =
                        Some((code.to_owned(), "Verbindung mit Fehler beendet".to_owned()));
                }
                Vec::new()
            }
            _ => Vec::new(),
        }
    }

    fn raster(&mut self, nutz: &[u8], jetzt: f64) -> Vec<Wirkung> {
        let Some(empfang) = self.empfang.as_mut() else {
            return Vec::new();
        };
        let (auszug, abbild) = match empfang.nimm(nutz) {
            Ok(x) => x,
            Err(e @ (RasterFehler::InflateFehler | RasterFehler::RahmenZuGross)) => {
                self.fehler = Some((
                    e.code().to_owned(),
                    "Bild der Verbindung nicht lesbar".to_owned(),
                ));
                return vec![Wirkung::StromZu];
            }
            // an invalid frame changes nothing (RA10)
            Err(_) => return Vec::new(),
        };
        let Some(aktiv) = &self.aktiv else {
            return Vec::new();
        };
        if nr_der(&self.sitzungen, aktiv) != Some(auszug.nr) {
            return Vec::new();
        }
        self.bild = Some(abbild.clone());
        if auszug.groesse.is_some() {
            return Vec::new(); // a full picture announces nothing
        }
        let neu: Vec<(usize, String)> = auszug
            .zeilen
            .iter()
            .filter(|(i, _)| *i != abbild.cursor.zeile)
            .map(|(i, _)| (*i, zeile::text(&abbild.lines[*i])))
            .filter(|(_, t)| !t.is_empty())
            .collect();
        match self.ansager.zeilen(neu, jetzt) {
            Ausgabe::Nichts => Vec::new(),
            Ausgabe::Sofort(t) => vec![Wirkung::Ansage(t)],
            Ausgabe::Wecker(ms) => vec![Wirkung::Wecker(ms)],
        }
    }

    /// The alarm of the announcer went off.
    pub fn wecken(&mut self, jetzt: f64) -> Vec<Wirkung> {
        self.ansager
            .wecken(jetzt)
            .map(Wirkung::Ansage)
            .into_iter()
            .collect()
    }

    /// A message `Hinweis` (SE3 point 3): a fixed text for each known reason, only without `sitzung` or for the shown session;
    /// every other reason stays quiet.
    fn hinweis(&self, n: &Value) -> Vec<Wirkung> {
        let fuer_aktiv = match n.get("sitzung") {
            None => true,
            Some(s) => s.as_str().is_some() && s.as_str() == self.aktiv.as_deref(),
        };
        let text = match n.get("hinweis").and_then(Value::as_str) {
            Some("osc52_geschrieben") => "Das Programm hat in die Zwischenablage geschrieben",
            // the service let an oversized message not out (contract `dienst`, section 8 point 19)
            Some("zwischenablage_zu_gross") => {
                "Die Zwischenablage des Programms war zu groß und wurde nicht übertragen"
            }
            Some("nachricht_zu_gross") => "Eine Nachricht war zu groß und wurde nicht übertragen",
            _ => return Vec::new(),
        };
        if fuer_aktiv {
            vec![Wirkung::Ansage(text.into()), Wirkung::Notiz(text.into())]
        } else {
            Vec::new()
        }
    }

    fn nachricht(&mut self, n: &Value, groesse: (usize, usize)) -> Vec<Wirkung> {
        match n.get("art").and_then(Value::as_str) {
            Some("Sitzungsliste") => self.sitzungsliste(n, groesse),
            Some("Startwahl") => {
                self.start.startwahl(n);
                Vec::new()
            }
            Some("Hinweis") => self.hinweis(n),
            Some("Anfragen") => {
                self.anfragen = n
                    .get("anfragen")
                    .and_then(Value::as_array)
                    .map(|a| {
                        a.iter()
                            .map(|x| Anfrage {
                                id: feld(x, "id"),
                                sitzung: feld(x, "sitzung"),
                                werkzeug: feld(x, "werkzeug"),
                                vorschau: feld(x, "vorschau"),
                            })
                            .filter(|x| !x.id.is_empty())
                            .collect()
                    })
                    .unwrap_or_default();
                Vec::new()
            }
            Some("Agenten") => {
                self.agenten = n
                    .get("agenten")
                    .and_then(Value::as_array)
                    .map(|a| {
                        a.iter()
                            .map(|x| (feld(x, "worker"), feld(x, "zustand")))
                            .collect()
                    })
                    .unwrap_or_default();
                Vec::new()
            }
            Some("Agentenzustand") => {
                let (w, z) = (feld(n, "worker"), feld(n, "zustand"));
                if !w.is_empty() {
                    match self.agenten.iter_mut().find(|(id, _)| *id == w) {
                        Some(e) => e.1 = z,
                        None => self.agenten.push((w, z)),
                    }
                }
                Vec::new()
            }
            Some("Einstellungen") => {
                self.einstellungen = n.get("werte").cloned();
                Vec::new()
            }
            Some("Routen") => {
                self.routen = n.get("routen").cloned();
                Vec::new()
            }
            Some("Dialog") => {
                let d = Dialog {
                    sitzung: feld(n, "sitzung"),
                    id: feld(n, "id"),
                    art: feld(n, "dialog_art"),
                    optionen: n
                        .get("optionen")
                        .and_then(Value::as_array)
                        .map(|o| {
                            o.iter()
                                .map(|x| x.as_str().map_or_else(|| x.to_string(), str::to_owned))
                                .collect()
                        })
                        .unwrap_or_default(),
                };
                self.dialoge.retain(|x| x.sitzung != d.sitzung);
                if !d.id.is_empty() {
                    self.dialoge.push(d);
                }
                Vec::new()
            }
            Some("DialogEnde") => {
                let (s, id) = (feld(n, "sitzung"), feld(n, "id"));
                self.dialoge.retain(|x| !(x.sitzung == s && x.id == id));
                Vec::new()
            }
            _ => Vec::new(),
        }
    }

    fn sitzungsliste(&mut self, n: &Value, groesse: (usize, usize)) -> Vec<Wirkung> {
        let mut w = Vec::new();
        let alt = std::mem::take(&mut self.sitzungen);
        self.sitzungen = n
            .get("sitzungen")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter(|s| s.get("id").and_then(Value::as_str).is_some())
                    .map(|s| Sitzung {
                        id: feld(s, "id"),
                        harness: feld(s, "harness"),
                        modell: feld(s, "modell"),
                        route: feld(s, "route"),
                        kunde: feld(s, "kunde"),
                        laeuft: s.get("zustand").and_then(Value::as_str) == Some("laeuft"),
                        code: feld(s, "code"),
                        // without the key (the list of the core in the single mode, no `lauf`) the one session has 0
                        nr: match s.get("nr") {
                            None => Some(0),
                            Some(n) => n.as_u64().and_then(|n| u8::try_from(n).ok()),
                        },
                    })
                    .collect()
            })
            .unwrap_or_default();
        // a number that now belongs to another session: the picture under it is the one of the old session (P25)
        for neu in &self.sitzungen {
            let Some(nr) = neu.nr else {
                continue;
            };
            if nr_der(&alt, &neu.id) != Some(nr)
                && let Some(e) = self.empfang.as_mut()
            {
                e.vergessen(nr);
            }
        }
        for s in &self.sitzungen {
            if !s.laeuft && self.bekannt.get(&s.id) == Some(&true) {
                w.push(Wirkung::Ansage(format!(
                    "Sitzung {} beendet, Code {}",
                    s.id, s.code
                )));
            }
        }
        self.bekannt = self
            .sitzungen
            .iter()
            .map(|s| (s.id.clone(), s.laeuft))
            .collect();
        if self.erste_liste {
            self.erste_liste = false;
            match self
                .sitzungen
                .iter()
                .find(|s| s.laeuft)
                .map(|s| s.id.clone())
            {
                Some(id) => w.extend(self.waehle(&id, groesse)),
                None => w.push(Wirkung::FokusStart),
            }
        }
        w
    }

    /// The choice of a session (SE2 point 5): `Abo` off for the old one, `Abo` on with the size for the new one.
    pub fn waehle(&mut self, id: &str, groesse: (usize, usize)) -> Vec<Wirkung> {
        if self.aktiv.as_deref() == Some(id) {
            return vec![Wirkung::FokusTerminal];
        }
        let mut w = Vec::new();
        if let Some(alt) = &self.aktiv {
            w.push(Wirkung::Senden(
                json!({"art": "Abo", "sitzung": alt, "an": false}),
            ));
        }
        self.aktiv = Some(id.to_owned());
        let nr = nr_der(&self.sitzungen, id);
        self.bild = self
            .empfang
            .as_ref()
            .and_then(|e| nr.and_then(|n| e.abbild(n)))
            .cloned();
        let (sp, ze) = groesse;
        self.gemeldet.insert(id.to_owned(), groesse);
        w.push(Wirkung::Senden(
            json!({"art": "Abo", "sitzung": id, "an": true, "spalten": sp, "zeilen": ze}),
        ));
        w.push(Wirkung::FokusTerminal);
        w
    }

    /// A new measured size (SE5 point 2): sent only when it differs from the last one sent for this session.
    pub fn groesse(&mut self, groesse: (usize, usize)) -> Vec<Wirkung> {
        let Some(id) = self.aktiv.clone() else {
            return Vec::new();
        };
        match self.gemeldet.get(&id) {
            Some(alt) if *alt != groesse => {}
            _ => return Vec::new(),
        }
        self.gemeldet.insert(id.clone(), groesse);
        vec![Wirkung::Senden(
            json!({"art": "Groesse", "sitzung": id, "spalten": groesse.0, "zeilen": groesse.1}),
        )]
    }

    /// A key in the terminal (SE1).
    #[must_use]
    pub fn taste(&self, d: &Deutung) -> Vec<Wirkung> {
        match d {
            Deutung::Taste { taste, umschalter } => self.sende_taste(taste, umschalter),
            Deutung::Text(t) => self.text(t),
            Deutung::Nichts | Deutung::Fokus(_) => Vec::new(),
        }
    }

    fn sende_taste(&self, taste: &str, umschalter: &[&str]) -> Vec<Wirkung> {
        let Some(s) = &self.aktiv else {
            return Vec::new();
        };
        vec![Wirkung::Senden(
            json!({"art": "Taste", "sitzung": s, "taste": taste, "umschalter": umschalter}),
        )]
    }

    /// Text into the terminal (a key, a composition, inserted text).
    #[must_use]
    pub fn text(&self, t: &str) -> Vec<Wirkung> {
        match &self.aktiv {
            Some(s) if !t.is_empty() => {
                vec![Wirkung::Senden(
                    json!({"art": "Text", "sitzung": s, "text": t}),
                )]
            }
            _ => Vec::new(),
        }
    }

    /// Paste into the terminal (SE1 point 10): unchanged, never as `Text`.
    #[must_use]
    pub fn einfuegen(&self, t: &str) -> Vec<Wirkung> {
        match &self.aktiv {
            Some(s) if !t.is_empty() => {
                vec![Wirkung::Senden(
                    json!({"art": "Einfuegen", "sitzung": s, "text": t}),
                )]
            }
            _ => Vec::new(),
        }
    }

    /// The input line is submitted (SE5 point 3): `Text` with the content, if any, then `Taste Enter`.
    #[must_use]
    pub fn zeile_absenden(&self, t: &str) -> Vec<Wirkung> {
        let mut w = self.text(t);
        w.extend(self.sende_taste("Enter", &[]));
        w
    }

    /// "Sitzung schließen" (B15): the event `Beenden` (`dienst.rs:1256`).
    #[must_use]
    pub fn beenden(&self, id: &str) -> Vec<Wirkung> {
        vec![
            Wirkung::Senden(json!({"art": "Beenden", "sitzung": id})),
            Wirkung::Notiz(format!("Sitzung {id} wird beendet")),
        ]
    }

    /// The answer to a dialog of a harness (B42): the event `Antwort` (`ereignis.rs:281`), options from 1.
    #[must_use]
    pub fn dialog_antwort(&self, d: &Dialog, wahl: usize) -> Vec<Wirkung> {
        vec![Wirkung::Senden(
            json!({"art": "Antwort", "sitzung": d.sitzung, "dialog": d.id, "wahl": wahl}),
        )]
    }

    /// A rejection (frame 7 or answer 422 of `/starten`, SE3 point 3).
    pub fn ablehnung(&mut self, j: &Value) -> Vec<Wirkung> {
        let code = j
            .get("code")
            .and_then(Value::as_str)
            .filter(|c| !c.is_empty())
            .or_else(|| j.get("grund").and_then(Value::as_str))
            .unwrap_or("")
            .to_owned();
        let text = j
            .get("text")
            .and_then(Value::as_str)
            .filter(|t| !t.is_empty())
            .map(str::to_owned)
            .or_else(|| {
                TEXTE
                    .iter()
                    .find(|(c, _)| *c == code)
                    .map(|(_, t)| (*t).to_owned())
            })
            .unwrap_or_else(|| code.clone());
        self.fehler = Some((code, text.clone()));
        vec![Wirkung::Ansage(format!("Abgelehnt: {text}"))]
    }

    /// The answer of `POST /starten` (SE6 point 3): status 0 is no connection.
    pub fn start_antwort(&mut self, status: u16, antwort: Option<&Value>) -> Vec<Wirkung> {
        let sitzung = antwort
            .and_then(|a| a.get("sitzung"))
            .and_then(Value::as_str);
        match (status, sitzung) {
            (200, Some(s)) => {
                self.fehler = None;
                let t = format!("Sitzung {s} gestartet");
                vec![Wirkung::Ansage(t.clone()), Wirkung::Notiz(t)]
            }
            (422, _) => self.ablehnung(antwort.unwrap_or(&Value::Null)),
            _ => {
                let code = if status == 0 {
                    "netz".to_owned()
                } else {
                    format!("http_{status}")
                };
                self.fehler = Some((code, "Start gescheitert".to_owned()));
                vec![Wirkung::Ansage("Start gescheitert".into())]
            }
        }
    }

    /// The text of line `i` of the shown picture.
    #[must_use]
    pub fn zeilentext(&self, i: usize) -> String {
        self.bild
            .as_ref()
            .and_then(|b| b.lines.get(i))
            .map(|z| zeile::text(z))
            .unwrap_or_default()
    }

    /// The session shown, if any.
    #[must_use]
    pub fn aktive_sitzung(&self) -> Option<&Sitzung> {
        let a = self.aktiv.as_deref()?;
        self.sitzungen.iter().find(|s| s.id == a)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn liste(k: &mut Kern, v: &Value) -> Vec<Wirkung> {
        k.rahmen(ART_NACHRICHT, v.to_string().as_bytes(), 0.0, (80, 24))
    }

    #[test]
    fn the_first_list_subscribes_the_first_running_session() {
        let mut k = Kern::default();
        k.offen(false);
        let w = liste(
            &mut k,
            &json!({"art": "Sitzungsliste", "sitzungen": [
                {"id": "s-2", "zustand": "beendet", "code": 1},
                {"id": "s-3", "zustand": "laeuft", "harness": "h", "modell": "m"}]}),
        );
        assert_eq!(
            w,
            vec![
                Wirkung::Senden(
                    json!({"art": "Abo", "sitzung": "s-3", "an": true, "spalten": 80, "zeilen": 24})
                ),
                Wirkung::FokusTerminal
            ]
        );
        assert_eq!(k.sitzungen[0].zustand_text(), "beendet, Code 1");
        // the next list announces the end once
        let w = liste(
            &mut k,
            &json!({"art": "Sitzungsliste", "sitzungen": [{"id": "s-3", "zustand": "beendet", "code": 0}]}),
        );
        assert_eq!(
            w,
            vec![Wirkung::Ansage("Sitzung s-3 beendet, Code 0".into())]
        );
        let w = liste(
            &mut k,
            &json!({"art": "Sitzungsliste", "sitzungen": [{"id": "s-3", "zustand": "beendet", "code": 0}]}),
        );
        assert_eq!(w, Vec::new());
    }

    #[test]
    fn without_a_running_session_the_focus_goes_to_the_form() {
        let mut k = Kern::default();
        assert_eq!(
            liste(&mut k, &json!({"art": "Sitzungsliste", "sitzungen": []})),
            vec![Wirkung::FokusStart]
        );
    }

    #[test]
    fn switching_unsubscribes_and_sizes_are_sent_once() {
        let mut k = Kern::default();
        k.waehle("s-1", (80, 24));
        let w = k.waehle("s-2", (100, 30));
        assert_eq!(
            w[0],
            Wirkung::Senden(json!({"art": "Abo", "sitzung": "s-1", "an": false}))
        );
        assert_eq!(k.groesse((100, 30)), Vec::new());
        assert_eq!(
            k.groesse((101, 30)),
            vec![Wirkung::Senden(
                json!({"art": "Groesse", "sitzung": "s-2", "spalten": 101, "zeilen": 30})
            )]
        );
        assert_eq!(k.groesse((101, 30)), Vec::new());
    }

    #[test]
    fn rejections_fill_the_error_field() {
        let mut k = Kern::default();
        assert_eq!(
            k.ablehnung(&json!({"grund": "taste_unbekannt"})),
            vec![Wirkung::Ansage("Abgelehnt: Taste nicht belegt".into())]
        );
        assert_eq!(
            k.fehler,
            Some(("taste_unbekannt".into(), "Taste nicht belegt".into()))
        );
        k.ablehnung(&json!({"grund": "x", "code": "start.form", "text": "Fester Text"}));
        assert_eq!(k.fehler, Some(("start.form".into(), "Fester Text".into())));
        k.start_antwort(200, Some(&json!({"sitzung": "s-4"})));
        assert_eq!(k.fehler, None);
        k.start_antwort(0, None);
        assert_eq!(k.fehler, Some(("netz".into(), "Start gescheitert".into())));
        k.rahmen(ART_FEHLER, br#"{"fehler":"zu"}"#, 0.0, (2, 1));
        assert_eq!(
            k.fehler,
            Some(("zu".into(), "Verbindung mit Fehler beendet".into()))
        );
    }

    #[test]
    fn oversized_messages_are_announced_for_the_shown_session() {
        let mut k = Kern::default();
        k.waehle("s-1", (2, 1));
        let gross = "Die Zwischenablage des Programms war zu groß und wurde nicht übertragen";
        let nachricht = "Eine Nachricht war zu groß und wurde nicht übertragen";
        let beide = |t: &str| vec![Wirkung::Ansage(t.into()), Wirkung::Notiz(t.into())];
        let w = liste(
            &mut k,
            &json!({"art": "Hinweis", "hinweis": "zwischenablage_zu_gross", "sitzung": "s-1", "t": 9}),
        );
        assert_eq!(w, beide(gross));
        let w = liste(
            &mut k,
            &json!({"art": "Hinweis", "hinweis": "nachricht_zu_gross"}),
        );
        assert_eq!(w, beide(nachricht));
        // another session, a session that is no text and an unknown reason stay quiet
        for n in [
            json!({"art": "Hinweis", "hinweis": "zwischenablage_zu_gross", "sitzung": "s-2"}),
            json!({"art": "Hinweis", "hinweis": "nachricht_zu_gross", "sitzung": 1}),
            json!({"art": "Hinweis", "hinweis": "ganz_neu", "sitzung": "s-1"}),
            json!({"art": "Hinweis", "hinweis": "titel", "sitzung": "s-1", "titel": "neu"}),
        ] {
            assert_eq!(liste(&mut k, &n), Vec::new(), "{n}");
        }
        // the old reason keeps its text
        let w = liste(
            &mut k,
            &json!({"art": "Hinweis", "hinweis": "osc52_geschrieben", "sitzung": "s-1", "laenge": 5}),
        );
        assert_eq!(
            w,
            beide("Das Programm hat in die Zwischenablage geschrieben")
        );
    }

    #[test]
    fn the_input_line_sends_text_then_enter() {
        let mut k = Kern::default();
        assert_eq!(k.zeile_absenden("ls"), Vec::new());
        k.waehle("s-1", (2, 1));
        assert_eq!(
            k.zeile_absenden(""),
            vec![Wirkung::Senden(
                json!({"art": "Taste", "sitzung": "s-1", "taste": "Enter", "umschalter": []})
            )]
        );
        assert_eq!(k.zeile_absenden("ls").len(), 2);
        assert_eq!(k.einfuegen(""), Vec::new());
    }

    #[test]
    fn the_clamp_keeps_two_columns_and_one_row() {
        assert_eq!(klemme(800.0, 340.0, 8.0, 17.0), (100, 20));
        assert_eq!(klemme(5.0, 3.0, 8.0, 17.0), (2, 1));
        assert_eq!(klemme(-5.0, f64::NAN, 0.0, 0.0), (2, 1));
        assert_eq!(klemme(1e9, 1e9, 8.0, 17.0), (1000, 1000));
    }

    #[test]
    fn session_numbers_come_from_the_list() {
        let mut k = Kern::default();
        liste(
            &mut k,
            &json!({"art": "Sitzungsliste", "sitzungen": [
                {"id": "s-12", "zustand": "laeuft", "nr": 3},
                {"id": "s-1", "zustand": "beendet", "code": 0, "nr": null},
                {"id": "s-2", "zustand": "beendet", "code": 0},
                {"id": "s-4", "zustand": "beendet", "code": 0, "nr": 256}]}),
        );
        let nrn: Vec<Option<u8>> = k.sitzungen.iter().map(|s| s.nr).collect();
        assert_eq!(nrn, [Some(3), None, Some(0), None]);
        assert_eq!(nr_der(&k.sitzungen, "s-12"), Some(3));
        assert_eq!(nr_der(&k.sitzungen, "s-1"), None);
        assert_eq!(nr_der(&k.sitzungen, "s-9"), None);
    }

    /// A full picture of 10 x 2 with the number `nr` and the first line `text`.
    fn vollbild(nr: u8, text: &str) -> Vec<u8> {
        let auszug = rasterauszug::Auszug {
            nr,
            stand: 1,
            groesse: Some((10, 2)),
            invers: false,
            schub: None,
            cursor: None,
            zeilen: vec![(0, rasterauszug::zeile_lesen(text).expect("line"))],
        };
        rasterauszug::rahmen_kodieren(&auszug)
    }

    /// P25: the frames of the chosen session are those with its number from the list, not the digits of its identifier; a number
    /// that goes to another session drops the old picture under it.
    #[test]
    fn frames_follow_the_number_of_the_list() {
        let mut k = Kern::default();
        k.offen(false);
        liste(
            &mut k,
            &json!({"art": "Sitzungsliste", "sitzungen": [{"id": "s-300", "zustand": "laeuft", "nr": 7}]}),
        );
        assert_eq!(k.aktiv.as_deref(), Some("s-300"));
        // a frame with number 44 (300 mod 256) is not this session
        k.rahmen(ART_RASTER, &vollbild(44, "fremd"), 0.0, (80, 24));
        assert!(k.bild.is_none());
        k.rahmen(ART_RASTER, &vollbild(7, "eigen"), 0.0, (80, 24));
        assert!(k.bild.is_some());
        // number 7 goes to s-301 (s-300 shows null): choosing s-301 shows no old picture of s-300
        liste(
            &mut k,
            &json!({"art": "Sitzungsliste", "sitzungen": [{"id": "s-300", "zustand": "beendet", "code": 0, "nr": null},
                                                          {"id": "s-301", "zustand": "laeuft", "nr": 7}]}),
        );
        k.waehle("s-301", (80, 24));
        assert!(k.bild.is_none(), "the picture of s-300 is gone");
    }
}
