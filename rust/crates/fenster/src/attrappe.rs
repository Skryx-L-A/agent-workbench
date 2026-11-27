//! The mock-up of the local path to the service (gap L11) behind [`Weg`].
//!
//! It sends the frames of the stream of the bridge: raster frames made with `rasterauszug` (the encoder of the
//! service) and the messages as JSON. Two forms:
//! - [`Form::Heute`]: only the messages and fields of today's contract `dienst` (no gap closed).
//! - [`Form::Ziel`]: the target state with the proposed messages and fields of section 4 (L1 to L10), for the
//!   pictures with the caption "Attrappe, Zielzustand" (OF8 point 3).
//!
//! The names and fields of the target state are the proposal of the contract `seite`, section 4. The contracts
//! `zustandskern` and `dienst` fix them; until then they may change.

// The mock-up answers every event in one place, so its two functions are long.
#![allow(clippy::too_many_lines)]

use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

use rasterauszug::Verbindung;
use serde_json::{Value, json};

use crate::bedienwege::Luecken;
use crate::weg::{Rahmen, Weg};

/// Frame kinds (contract `dienst`, section 5).
const RASTER: u8 = 5;
const NACHRICHT: u8 = 6;
const ABLEHNUNG: u8 = 7;

/// Which messages the mock-up sends.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Form {
    /// Today's messages and fields only.
    Heute,
    /// The target state of section 4.
    Ziel,
}

/// One session of the mock-up.
#[derive(Clone, Debug)]
struct Sitzung {
    id: String,
    name: String,
    projekt: String,
    ordner: String,
    harness: String,
    modell: String,
    route: String,
    laeuft: bool,
    code: i64,
    nr: u8,
}

/// One worker of the mock-up (L1).
#[derive(Clone, Debug)]
struct Arbeiter {
    id: String,
    sitzung: String,
    rolle: String,
    modell: String,
    zustand: String,
    nr: u8,
}

/// The state of the mock-up; the runner of the window reads the events and starts situations through [`Griff`].
pub struct Stand {
    form: Form,
    sitzungen: Vec<Sitzung>,
    arbeiter: Vec<Arbeiter>,
    anfragen: Vec<Value>,
    verlauf: Vec<Value>,
    texte: Vec<(u8, Vec<String>)>,
    ausgang: VecDeque<Rahmen>,
    offen: bool,
    verbindung: Verbindung,
    groesse: Vec<(u8, (usize, usize))>,
    /// Every event the window sent, in order.
    pub ereignisse: Vec<Value>,
    admin: bool,
    einstellungen: Vec<(String, Value, String, bool)>,
}

/// A handle on the state of a mock-up (for the runner and the pictures).
pub type Griff = Rc<RefCell<Stand>>;

/// The mock-up.
pub struct Attrappe {
    stand: Griff,
}

fn sitzung(
    id: &str,
    name: &str,
    projekt: &str,
    teile: (&str, &str, &str),
    laeuft: bool,
    code: i64,
    nr: u8,
) -> Sitzung {
    Sitzung {
        id: id.to_owned(),
        name: name.to_owned(),
        projekt: projekt.to_owned(),
        ordner: format!("~/AI/{projekt}"),
        harness: teile.0.to_owned(),
        modell: teile.1.to_owned(),
        route: teile.2.to_owned(),
        laeuft,
        code,
        nr,
    }
}

fn arbeiter(id: &str, sitzung: &str, rolle: &str, modell: &str, zustand: &str, nr: u8) -> Arbeiter {
    Arbeiter {
        id: id.to_owned(),
        sitzung: sitzung.to_owned(),
        rolle: rolle.to_owned(),
        modell: modell.to_owned(),
        zustand: zustand.to_owned(),
        nr,
    }
}

/// The text of the terminal of the orchestrator in the mock-up: plain lines in the line form of `rasterauszug`.
fn orchestrator_text() -> Vec<String> {
    [
        "\u{1b}[1mclaude\u{1b}[0m · opus · ~/AI/werkbank",
        "",
        "> Plane den Pilot des App-Fensters und verteile die Arbeit.",
        "",
        "\u{1b}[32m●\u{1b}[0m Ich lese OBERFLAECHE.md, Abschnitt 2, und teile die Bedienwege auf.",
        "  └  46 Bedienwege mit Umfang Pilot, 18 danach, 4 entfallen",
        "",
        "\u{1b}[32m●\u{1b}[0m Worker 1 baut die Leiste, Worker 2 das Terminal, Worker 3 den Inspektor.",
        "",
        "┌──────────────────────────────────────────────┐",
        "│ Grüße aus dem Terminal: äöü ÄÖÜ ß € – „“ … │",
        "└──────────────────────────────────────────────┘",
        "",
        "\u{1b}[2m? für Hilfe\u{1b}[0m",
        "> ",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect()
}

fn worker_text(rolle: &str, n: usize) -> Vec<String> {
    vec![
        format!("\u{1b}[1m{rolle}\u{1b}[0m"),
        String::new(),
        format!("\u{1b}[32m●\u{1b}[0m Schritt {n}: lese Auftrag und Ordner"),
        "  └  cargo build -p fenster".to_owned(),
        "\u{1b}[33m●\u{1b}[0m wartet auf Freigabe".to_owned(),
        "> ".to_owned(),
    ]
}

impl Attrappe {
    /// A new mock-up of the given form and a handle on its state. The first frames (session list, start choice, and in
    /// the target state the workers, requests and settings) wait for the first [`Weg::abholen`].
    #[must_use]
    pub fn neu(form: Form) -> (Attrappe, Griff) {
        let mut sitzungen = vec![
            sitzung(
                "s-1",
                "Planung Pilot",
                "werkbank",
                ("claude", "opus", "cloud"),
                true,
                0,
                1,
            ),
            sitzung(
                "s-2",
                "Abnahme Fenster",
                "werkbank",
                ("claude", "sonnet", "cloud"),
                true,
                0,
                2,
            ),
            sitzung(
                "s-3",
                "Datenabgleich",
                "kunde-nord",
                ("pi", "qwen3-coder", "lokal"),
                false,
                0,
                3,
            ),
            sitzung(
                "s-4",
                "Import Altbestand",
                "kunde-nord",
                ("pi", "qwen3-coder", "lokal"),
                false,
                137,
                4,
            ),
        ];
        if form == Form::Heute {
            sitzungen.truncate(2);
        }
        let rollen = [
            ("w-1", "Leiste", "sonnet", "laeuft"),
            ("w-2", "Terminal", "opus", "laeuft"),
            ("w-3", "Inspektor", "sonnet", "wartet"),
            ("w-4", "Tests", "haiku", "laeuft"),
            ("w-5", "Bilder", "sonnet", "laeuft"),
            ("w-6", "Tastatur", "sonnet", "fertig"),
            ("w-7", "Merken", "haiku", "laeuft"),
            ("w-8", "Review", "opus", "laeuft"),
        ];
        let mut arbeiter_liste: Vec<Arbeiter> = rollen
            .iter()
            .zip(10_u8..)
            .map(|((id, rolle, modell, zustand), nr)| {
                arbeiter(id, "s-1", rolle, modell, zustand, nr)
            })
            .collect();
        arbeiter_liste.push(arbeiter("w-9", "s-2", "Prüfung", "sonnet", "laeuft", 20));
        let mut texte = vec![(1, orchestrator_text()), (2, worker_text("Abnahme", 1))];
        for (i, a) in arbeiter_liste.iter().enumerate() {
            texte.push((a.nr, worker_text(&a.rolle, i + 1)));
        }
        let einstellungen = vec![
            (
                "aussehen.thema".to_owned(),
                json!("system"),
                "du".to_owned(),
                true,
            ),
            (
                "aussehen.farbsatz".to_owned(),
                json!("grau"),
                "Firma".to_owned(),
                true,
            ),
            (
                "aussehen.terminal".to_owned(),
                json!("dunkel"),
                "Firma".to_owned(),
                true,
            ),
            (
                "aussehen.schriftgroesse".to_owned(),
                json!(13),
                "Firma".to_owned(),
                true,
            ),
            (
                "sitzung.harness".to_owned(),
                json!("claude"),
                "Gruppe Entwicklung".to_owned(),
                true,
            ),
            (
                "sitzung.modell".to_owned(),
                json!("sonnet"),
                "du".to_owned(),
                true,
            ),
            (
                "sitzung.effort".to_owned(),
                json!("medium"),
                "du".to_owned(),
                true,
            ),
            (
                "admin.harness.pi".to_owned(),
                json!(true),
                "Firma".to_owned(),
                false,
            ),
            (
                "admin.modell.opus.effort_max".to_owned(),
                json!("high"),
                "Firma".to_owned(),
                false,
            ),
            (
                "admin.aufsicht.guard".to_owned(),
                json!(false),
                "Firma".to_owned(),
                false,
            ),
        ];
        let stand = Stand {
            form,
            sitzungen,
            arbeiter: if form == Form::Ziel {
                arbeiter_liste
            } else {
                Vec::new()
            },
            // requests are a message of today (`Anfragen`); the worker of a request is a field of L1
            anfragen: {
                let mut a = vec![
                    json!({"id": "a-1", "sitzung": "s-1", "werkzeug": "Bash", "vorschau": "cargo test -p fenster --locked", "worker": "w-3"}),
                    json!({"id": "a-2", "sitzung": "s-1", "werkzeug": "Write", "vorschau": "rust/crates/fenster/src/gestalt.rs", "worker": "w-5"}),
                ];
                if form == Form::Heute {
                    for x in &mut a {
                        if let Some(o) = x.as_object_mut() {
                            o.remove("worker");
                        }
                    }
                }
                a
            },
            verlauf: vec![
                json!({"text": "Bash: cargo fmt --check", "erteilt": true, "worker": "w-1"}),
                json!({"text": "WebFetch: example.org", "erteilt": false, "begruendung": "Netz gesperrt", "worker": "w-2"}),
            ],
            texte,
            ausgang: VecDeque::new(),
            offen: true,
            verbindung: Verbindung::neu(false),
            groesse: Vec::new(),
            ereignisse: Vec::new(),
            admin: false,
            einstellungen,
        };
        let griff = Rc::new(RefCell::new(stand));
        {
            let mut s = griff.borrow_mut();
            s.liste();
            s.startwahl();
            s.anfragen_senden();
            if form == Form::Ziel {
                s.agenten();
                s.einstellungen_senden();
            }
        }
        (
            Attrappe {
                stand: Rc::clone(&griff),
            },
            griff,
        )
    }
}

impl Stand {
    fn nachricht(&mut self, n: &Value) {
        self.ausgang.push_back(Rahmen {
            art: NACHRICHT,
            nutz: n.to_string().into_bytes(),
        });
    }

    fn ablehnen(&mut self, grund: &str) {
        self.ausgang.push_back(Rahmen {
            art: ABLEHNUNG,
            nutz: json!({"grund": grund}).to_string().into_bytes(),
        });
    }

    fn liste(&mut self) {
        let ziel = self.form == Form::Ziel;
        let sitzungen: Vec<Value> = self
            .sitzungen
            .iter()
            .map(|s| {
                let mut x = json!({"id": s.id, "harness": s.harness, "modell": s.modell, "route": s.route, "kunde": "",
                    "zustand": if s.laeuft { "laeuft" } else { "beendet" }, "nr": s.nr});
                if !s.laeuft {
                    x["code"] = json!(s.code);
                }
                if ziel {
                    x["name"] = json!(s.name);
                    x["ordner"] = json!(s.ordner);
                    x["projekt"] = json!(s.projekt);
                }
                x
            })
            .collect();
        self.nachricht(&json!({"art": "Sitzungsliste", "sitzungen": sitzungen}));
    }

    fn startwahl(&mut self) {
        let mut n = json!({"art": "Startwahl", "fassung": 1,
            "harness": [{"name": "claude"}, {"name": "pi"}],
            "routen": [{"schluessel": "cloud", "art": "cloud", "klasse": "intern", "modell": "sonnet"},
                       {"schluessel": "lokal", "art": "lokal", "klasse": "vertraulich", "modell": "qwen3-coder"}],
            "kunden": ["kunde-nord", null]});
        if self.form == Form::Ziel {
            // L10: the allowed efforts per route and model and the default of the person
            n["efforts"] = json!({"cloud": ["low", "medium", "high"], "lokal": ["medium"]});
            n["effort_vorgabe"] = json!("medium");
        }
        self.nachricht(&n);
    }

    fn agenten(&mut self) {
        let agenten: Vec<Value> = self
            .arbeiter
            .iter()
            .map(|a| json!({"worker": a.id, "zustand": a.zustand, "sitzung": a.sitzung, "rolle": a.rolle, "modell": a.modell, "nr": a.nr}))
            .collect();
        self.nachricht(&json!({"art": "Agenten", "agenten": agenten}));
    }

    fn anfragen_senden(&mut self) {
        let a = self.anfragen.clone();
        self.nachricht(&json!({"art": "Anfragen", "anfragen": a}));
        // the guard history is a message of L6
        if self.form == Form::Ziel {
            let v = self.verlauf.clone();
            self.nachricht(&json!({"art": "Guardverlauf", "eintraege": v}));
        }
    }

    fn einstellungen_senden(&mut self) {
        let eintraege: Vec<Value> = self
            .einstellungen
            .iter()
            .filter(|(k, ..)| self.admin || !k.starts_with("admin."))
            .map(|(k, w, h, nutzer)| {
                json!({"schluessel": k, "wert": w, "herkunft": h, "bereich": if k.starts_with("admin.") { "admin" } else { "nutzer" },
                    "aenderbar": if k.starts_with("admin.") { self.admin } else { *nutzer }})
            })
            .collect();
        self.nachricht(&json!({"art": "Einstellungen", "admin": self.admin, "eintraege": eintraege, "werte": {}}));
    }

    /// The picture of raster `nr` at the size the window asked for, as a frame.
    fn bild(&mut self, sitzung: &str, nr: u8, groesse: (usize, usize)) {
        let texte = self
            .texte
            .iter()
            .find(|(n, _)| *n == nr)
            .map(|(_, t)| t.clone())
            .unwrap_or_default();
        let (spalten, zeilen) = (groesse.0.clamp(2, 1000), groesse.1.clamp(1, 1000));
        let mut z: Vec<String> = texte.iter().take(zeilen).cloned().collect();
        z.resize(zeilen, String::new());
        let cursor_zeile = texte.len().min(zeilen).saturating_sub(1);
        let cursor_spalte = texte
            .get(cursor_zeile)
            .map_or(0, |t| t.chars().count())
            .min(spalten - 1);
        let stand = self.ereignisse.len() as u64 + 1;
        let msg = json!({"art": "Abzug", "sitzung": sitzung, "stand": stand, "groesse": [spalten, zeilen],
            "invers": false, "zeilen": z, "cursor": [cursor_zeile, cursor_spalte, true]});
        if let Ok(Some(nutz)) = self.verbindung.nimm(nr, &msg) {
            self.ausgang.push_back(Rahmen { art: RASTER, nutz });
        }
    }

    fn nr_von(&self, sitzung: &str, nr: Option<u8>) -> Option<u8> {
        nr.or_else(|| {
            self.sitzungen
                .iter()
                .find(|s| s.id == sitzung)
                .map(|s| s.nr)
        })
    }

    fn tippen(&mut self, sitzung: &str, was: &str) {
        let Some(nr) = self.nr_von(sitzung, None) else {
            return;
        };
        if let Some((_, t)) = self.texte.iter_mut().find(|(n, _)| *n == nr) {
            if was == "\n" {
                let zeile = t.last().cloned().unwrap_or_default();
                t.push(format!("EMPFANGEN {}", zeile.trim_start_matches("> ")));
                t.push("> ".to_owned());
            } else if let Some(letzte) = t.last_mut() {
                letzte.push_str(was);
            }
        }
        let g = self
            .groesse
            .iter()
            .find(|(n, _)| *n == nr)
            .map_or((80, 24), |(_, g)| *g);
        let id = sitzung.to_owned();
        self.bild(&id, nr, g);
    }

    fn sitzung_mut(&mut self, id: &str) -> Option<&mut Sitzung> {
        self.sitzungen.iter_mut().find(|s| s.id == id)
    }

    fn ereignis(&mut self, e: &Value) {
        self.ereignisse.push(e.clone());
        let art = e.get("art").and_then(Value::as_str).unwrap_or("");
        let s = e
            .get("sitzung")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_owned();
        let ziel = self.form == Form::Ziel;
        let zahl = |k: &str| {
            e.get(k)
                .and_then(Value::as_u64)
                .and_then(|n| usize::try_from(n).ok())
        };
        match art {
            "Abo" => {
                if e.get("an") == Some(&Value::Bool(false)) {
                    return;
                }
                let nr = e
                    .get("nr")
                    .and_then(Value::as_u64)
                    .and_then(|n| u8::try_from(n).ok());
                let Some(nr) = self.nr_von(&s, nr) else {
                    return self.ablehnen("sitzung_unbekannt");
                };
                let g = (zahl("spalten").unwrap_or(80), zahl("zeilen").unwrap_or(24));
                self.groesse.retain(|(n, _)| *n != nr);
                self.groesse.push((nr, g));
                self.verbindung.vergessen(nr);
                self.bild(&s, nr, g);
            }
            "Groesse" => {
                let nr = e
                    .get("nr")
                    .and_then(Value::as_u64)
                    .and_then(|n| u8::try_from(n).ok());
                if let Some(nr) = self.nr_von(&s, nr) {
                    let g = (zahl("spalten").unwrap_or(80), zahl("zeilen").unwrap_or(24));
                    self.groesse.retain(|(n, _)| *n != nr);
                    self.groesse.push((nr, g));
                    self.verbindung.vergessen(nr);
                    self.bild(&s, nr, g);
                }
            }
            "Text" | "Einfuegen" => {
                let t = e
                    .get("text")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_owned();
                self.tippen(&s, &t);
            }
            "Taste" => {
                if e.get("taste").and_then(Value::as_str) == Some("Enter") {
                    self.tippen(&s, "\n");
                }
            }
            "Beenden" => {
                if let Some(x) = self.sitzung_mut(&s) {
                    x.laeuft = false;
                    x.code = 0;
                }
                self.liste();
            }
            "Starten" => {
                let n = self.sitzungen.len() + 1;
                let id = format!("s-{n}");
                let feld = |k: &str| e.get(k).and_then(Value::as_str).unwrap_or("").to_owned();
                let ordner = feld("arbeitsordner");
                if ordner.is_empty() {
                    self.ausgang.push_back(Rahmen {
                        art: ABLEHNUNG,
                        nutz: json!({"code": "start.arbeitsordner_fehlt", "text": "Der Arbeitsordner fehlt"})
                            .to_string()
                            .into_bytes(),
                    });
                    return;
                }
                let projekt = ordner.rsplit('/').next().unwrap_or("").to_owned();
                let mut neu = sitzung(
                    &id,
                    &feld("name"),
                    &projekt,
                    (&feld("harness"), &feld("modell"), &feld("route")),
                    true,
                    0,
                    30,
                );
                neu.ordner = ordner;
                self.sitzungen.push(neu);
                self.liste();
                self.nachricht(&json!({"art": "Gestartet", "sitzung": id}));
            }
            "Antwort" => {
                let d = e.get("dialog").cloned().unwrap_or(Value::Null);
                self.nachricht(&json!({"art": "DialogEnde", "sitzung": s, "id": d}));
            }
            "Fortsetzen" | "Wiederbeleben" if ziel => {
                if let Some(x) = self.sitzung_mut(&s) {
                    x.laeuft = true;
                }
                self.liste();
            }
            "Loeschen" if ziel => {
                self.sitzungen.retain(|x| x.id != s);
                self.liste();
            }
            "Umbenennen" if ziel => {
                let name = e
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_owned();
                if let Some(x) = self.sitzung_mut(&s) {
                    x.name = name;
                }
                self.liste();
            }
            "Freigabe" if ziel => {
                let id = e.get("id").and_then(Value::as_str).unwrap_or("");
                let erteilt = e.get("erteilt").and_then(Value::as_bool).unwrap_or(false);
                if let Some(pos) = self
                    .anfragen
                    .iter()
                    .position(|a| a.get("id").and_then(Value::as_str) == Some(id))
                {
                    let a = self.anfragen.remove(pos);
                    let text = format!(
                        "{}: {}",
                        a.get("werkzeug").and_then(Value::as_str).unwrap_or(""),
                        a.get("vorschau").and_then(Value::as_str).unwrap_or("")
                    );
                    self.verlauf.insert(
                        0,
                        json!({"text": text, "erteilt": erteilt, "begruendung": e.get("begruendung").cloned().unwrap_or(Value::Null)}),
                    );
                }
                self.anfragen_senden();
            }
            "EinstellungSetzen" if ziel => {
                let k = e
                    .get("schluessel")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_owned();
                let w = e.get("wert").cloned().unwrap_or(Value::Null);
                let admin = self.admin;
                match self.einstellungen.iter_mut().find(|(x, ..)| *x == k) {
                    Some(eintrag) if eintrag.3 || admin => {
                        eintrag.1 = w;
                        "du".clone_into(&mut eintrag.2);
                        self.einstellungen_senden();
                    }
                    _ => self.ablehnen("einstellung_gesperrt"),
                }
            }
            "OrdnerLesen" if ziel => {
                let pfad = e
                    .get("pfad")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_owned();
                let ordner = self
                    .sitzungen
                    .iter()
                    .find(|x| x.id == s)
                    .map(|x| x.ordner.clone())
                    .unwrap_or_default();
                self.nachricht(&json!({"art": "Ordnerinhalt", "sitzung": s, "pfad": if pfad.is_empty() { ordner } else { pfad },
                    "eintraege": [{"name": "rust", "ordner": true}, {"name": "spec", "ordner": true}, {"name": "README.md", "ordner": false},
                                  {"name": "SESSION-STATE.md", "ordner": false}, {"name": "Cargo.toml", "ordner": false}]}));
            }
            "InhaltSuchen" if ziel => {
                let muster = e
                    .get("muster")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_owned();
                self.nachricht(&json!({"art": "Suchtreffer", "sitzung": s, "muster": muster,
                    "treffer": [{"pfad": "rust/crates/fenster/src/gestalt.rs", "zeile": 12, "text": format!("… {muster} …")}]}));
            }
            _ => self.ablehnen("art_nicht_erlaubt"),
        }
    }

    // ------------------------------------------------------------------ situations for the runner and the pictures

    /// Ends the stream (B05).
    pub fn trennen(&mut self) {
        self.offen = false;
    }

    /// Opens a dialog of a harness in session `sitzung` (B42).
    pub fn dialog(&mut self, sitzung: &str) {
        self.nachricht(
            &json!({"art": "Dialog", "sitzung": sitzung, "id": "d-1", "dialog_art": "auswahl",
            "optionen": ["Ja, weiter", "Nein, abbrechen"]}),
        );
    }

    /// Sends a note (B67) for session `sitzung`.
    pub fn hinweis(&mut self, sitzung: &str) {
        self.nachricht(
            &json!({"art": "Hinweis", "sitzung": sitzung, "hinweis": "osc52_geschrieben"}),
        );
    }

    /// Makes the viewer an admin (L4, B78) and sends the settings again.
    pub fn admin_setzen(&mut self, admin: bool) {
        self.admin = admin;
        if self.form == Form::Ziel {
            self.einstellungen_senden();
        }
    }

    /// Ends every session (B34 after a new list).
    pub fn alle_beenden(&mut self) {
        for s in &mut self.sitzungen {
            s.laeuft = false;
        }
        self.liste();
    }

    /// Before the first frame: no session runs, so the window chooses none (SE2 point 1, B34).
    pub fn nur_beendete(&mut self) {
        for s in &mut self.sitzungen {
            s.laeuft = false;
        }
        self.ausgang.clear();
        self.liste();
        self.startwahl();
        self.anfragen_senden();
        if self.form == Form::Ziel {
            self.agenten();
            self.einstellungen_senden();
        }
    }
}

impl Weg for Attrappe {
    fn senden(&mut self, ereignis: &Value) {
        self.stand.borrow_mut().ereignis(ereignis);
    }

    fn abholen(&mut self) -> Vec<Rahmen> {
        let mut s = self.stand.borrow_mut();
        if !s.offen {
            return Vec::new();
        }
        s.ausgang.drain(..).collect()
    }

    fn offen(&self) -> bool {
        self.stand.borrow().offen
    }

    fn geschlossen(&self) -> Luecken {
        match self.stand.borrow().form {
            Form::Heute => Luecken::KEINE,
            Form::Ziel => Luecken::ALLE,
        }
    }

    fn ist_attrappe(&self) -> bool {
        true
    }
}
