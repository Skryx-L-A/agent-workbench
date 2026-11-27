//! The runner of the window (contract `seite`, part surface, OF14, means of checking): it drives the window without a
//! window over the accessibility tree of egui (AccessKit) and synthetic keys, counts the element ids `Bnn` against
//! section 2 of the contract and reports missing and surplus ones.
//!
//! With `FENSTER_BERICHT=<datei>` it writes the table per operating path of scope P (connected, mock-up only with its gap,
//! missing with the reason) as Markdown.

use std::collections::{BTreeMap, BTreeSet};

use egui::accesskit::{Action, ActionRequest, Node, NodeId, Role, TreeId};
use egui::{Event, Key, Modifiers, Pos2, RawInput, Rect, vec2};
use fenster::attrappe::{Form, Griff};
use fenster::bedienwege::{self, BEDIENWEGE, FEHLT, Luecken, Umfang};
use fenster::fenster::Fenster;
use serde_json::{Value, json};

/// One run of the window on a mock-up.
struct Lauf {
    ctx: egui::Context,
    f: Fenster,
    g: Griff,
    zeit: f64,
    baum: BTreeMap<NodeId, Node>,
    fokus: Option<NodeId>,
    gesehen: BTreeSet<String>,
    verboten: Vec<String>,
}

impl Lauf {
    fn neu(form: Form, vorher: impl FnOnce(&Griff)) -> Lauf {
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        fenster::einrichten(&ctx);
        let (mut f, g) = fenster::mit_attrappe(form);
        f.system_dunkel = Some(true);
        vorher(&g);
        let mut l = Lauf {
            ctx,
            f,
            g,
            zeit: 0.0,
            baum: BTreeMap::new(),
            fokus: None,
            gesehen: BTreeSet::new(),
            verboten: Vec::new(),
        };
        l.bilder(3);
        l
    }

    fn bild(&mut self, events: Vec<Event>) {
        self.zeit += 0.05;
        let eingabe = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1280.0, 800.0))),
            time: Some(self.zeit),
            focused: true,
            events,
            ..RawInput::default()
        };
        let f = &mut self.f;
        let mut aus = self.ctx.run_ui(eingabe, |ui| f.zeichnen(ui));
        aus.textures_delta.clear();
        if let Some(u) = aus.platform_output.accesskit_update {
            self.baum = u.nodes.into_iter().collect();
            self.fokus = Some(u.focus);
        }
        for n in self.baum.values() {
            if let Some(a) = n.author_id()
                && a.len() == 3
                && a.starts_with('B')
            {
                match bedienwege::finde(a) {
                    Some(b) if b.umfang == Umfang::P => {
                        self.gesehen.insert(a.to_owned());
                    }
                    _ => self.verboten.push(a.to_owned()),
                }
            }
        }
    }

    fn bilder(&mut self, n: usize) {
        for _ in 0..n {
            self.bild(Vec::new());
        }
    }

    fn knoten(&self, passt: impl Fn(&Node) -> bool) -> Option<NodeId> {
        self.baum.iter().find(|(_, n)| passt(n)).map(|(id, _)| *id)
    }

    fn mit_kennung(&self, k: &str) -> Option<NodeId> {
        self.knoten(|n| n.author_id() == Some(k) || n.class_name() == Some(k))
    }

    fn mit_name(&self, rolle: Role, name: &str) -> Option<NodeId> {
        self.knoten(|n| n.role() == rolle && n.label().is_some_and(|l| l.contains(name)))
    }

    fn aktion(&mut self, ziel: Option<NodeId>, action: Action, was: &str) {
        let Some(id) = ziel else {
            panic!("kein Knoten für {was}");
        };
        self.bild(vec![Event::AccessKitActionRequest(ActionRequest {
            action,
            target_tree: TreeId::ROOT,
            target_node: id,
            data: None,
        })]);
        self.bilder(2);
    }

    fn klick(&mut self, ziel: Option<NodeId>, was: &str) {
        self.aktion(ziel, Action::Click, was);
    }

    fn taste(&mut self, key: Key, modifiers: Modifiers) {
        self.bild(vec![Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        }]);
        self.bilder(1);
    }

    fn fokus_kennung(&self) -> Option<String> {
        let n = self.baum.get(&self.fokus?)?;
        n.author_id()
            .map(str::to_owned)
            .or_else(|| n.class_name().map(str::to_owned))
            .or_else(|| n.label().map(str::to_owned))
    }

    fn ereignisse(&self) -> Vec<Value> {
        self.g.borrow().ereignisse.clone()
    }
}

/// Section 2 of the contract file: number, scope, gaps and base form per row.
fn abschnitt_2() -> Vec<(String, String, Vec<u8>, bool)> {
    let pfad = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../spec/seite/OBERFLAECHE.md"
    );
    let text = std::fs::read_to_string(pfad).expect("OBERFLAECHE.md lesbar");
    let start = text.find("## 2. Bedienwege").expect("Abschnitt 2");
    let ende = text[start..].find("Zählung").expect("Zählung") + start;
    text[start..ende]
        .lines()
        .filter(|z| z.starts_with("| B"))
        .map(|z| {
            let teile: Vec<&str> = z.split('|').map(str::trim).collect();
            let luecken = teile[5]
                .split(',')
                .filter_map(|l| l.trim().strip_prefix('L').and_then(|n| n.parse().ok()))
                .collect();
            (
                teile[1].to_owned(),
                teile[3].to_owned(),
                luecken,
                teile[6] == "ja",
            )
        })
        .collect()
}

#[test]
fn the_table_of_the_window_equals_section_2() {
    let spec = abschnitt_2();
    assert_eq!(spec.len(), 69, "Abschnitt 2 hat 69 Bedienwege");
    let tabelle: Vec<(String, String, Vec<u8>, bool)> = BEDIENWEGE
        .iter()
        .map(|b| {
            let u = match b.umfang {
                Umfang::P => "P",
                Umfang::N => "N",
                Umfang::E => "E",
            };
            (
                b.nr.to_owned(),
                u.to_owned(),
                b.luecken.to_vec(),
                b.grundform,
            )
        })
        .collect();
    assert_eq!(tabelle, spec);
}

/// Drives the target state through every situation of section 2 and returns the run.
fn ziel_abfahren() -> Lauf {
    let mut l = Lauf::neu(Form::Ziel, |_| {});
    // inspector: open, tab Freigaben (default), then Ordner
    l.klick(l.mit_kennung("B04"), "B04");
    l.klick(l.mit_name(Role::Tab, "Ordner"), "Reiter Ordner");
    l.klick(l.mit_name(Role::Tab, "Freigaben"), "Reiter Freigaben");
    // the view "Worker" with more workers than one tile form holds
    l.klick(
        l.mit_name(Role::Button, "Worker")
            .or_else(|| l.knoten(|n| n.label() == Some("Worker"))),
        "Worker",
    );
    l.klick(l.mit_kennung("B26"), "B26");
    assert!(l.mit_kennung("B29").is_some(), "Worker-Liste offen");
    l.taste(Key::Escape, Modifiers::NONE);
    assert!(
        l.mit_kennung("B29").is_none(),
        "Escape schließt die Worker-Liste (B76)"
    );
    l.klick(
        l.knoten(|n| n.label() == Some("Orchestrator")),
        "Orchestrator",
    );
    // the menu of a crashed session: focus the row, Shift+F10
    l.aktion(l.mit_kennung("sitzung-s-4"), Action::Focus, "Zeile s-4");
    l.taste(Key::F10, Modifiers::SHIFT);
    assert!(l.mit_kennung("B17").is_some(), "Menü mit Wiederbeleben");
    l.taste(Key::Escape, Modifiers::NONE);
    // the window "Sitzungen": open over "+", start without a folder gives the rejection
    l.klick(l.mit_kennung("B08"), "B08");
    assert_eq!(
        l.fokus_kennung().as_deref(),
        Some("name-eingabe"),
        "Fokus im Feld Name (SE2 Punkt 2)"
    );
    l.klick(l.mit_kennung("sitzung-start"), "Starten");
    assert!(
        l.mit_kennung("B71").is_some(),
        "Meldung bei abgelehntem Start"
    );
    l.taste(Key::Escape, Modifiers::NONE);
    l.bilder(1);
    assert!(
        l.mit_kennung("B68").is_none(),
        "Escape schließt das Fenster Sitzungen"
    );
    assert_eq!(
        l.fokus_kennung().as_deref(),
        Some("B08"),
        "Fokus zurück auf +"
    );
    // settings as a user, then as an admin
    l.klick(l.mit_kennung("B23"), "B23");
    l.g.borrow_mut().admin_setzen(true);
    l.bilder(2);
    l.taste(Key::Escape, Modifiers::NONE);
    // a dialog, a note, the end of the stream
    l.g.borrow_mut().dialog("s-1");
    l.g.borrow_mut().hinweis("s-1");
    l.bilder(2);
    l.g.borrow_mut().trennen();
    l.bilder(2);
    let mut leer = Lauf::neu(Form::Ziel, |g| g.borrow_mut().nur_beendete());
    leer.bilder(1);
    l.gesehen.extend(leer.gesehen);
    l.verboten.extend(leer.verboten);
    l
}

fn heute_abfahren() -> Lauf {
    let mut l = Lauf::neu(Form::Heute, |_| {});
    // the base forms of section 2 show before their gaps (OF8 point 3): inspector with the requests of today
    l.klick(l.mit_kennung("B04"), "B04");
    assert!(l.mit_kennung("B56").is_some(), "Freigaben in Grundform");
    // the menu of a session row: closing works today, the paths of L3 stay away
    l.aktion(l.mit_kennung("sitzung-s-1"), Action::Focus, "Zeile s-1");
    l.taste(Key::F10, Modifiers::SHIFT);
    assert!(l.mit_kennung("B15").is_some(), "Menü mit Sitzung schließen");
    assert!(l.mit_kennung("B12").is_none(), "Namen ändern braucht L3");
    l.taste(Key::Escape, Modifiers::NONE);
    // starting and choosing: the window "Sitzungen" without the fields of L10
    l.klick(l.mit_kennung("B08"), "B08");
    assert!(
        l.mit_kennung("B68").is_some(),
        "Fenster Sitzungen in Grundform"
    );
    assert!(l.mit_kennung("name-eingabe").is_none(), "Name braucht L10");
    assert!(l.mit_kennung("effort-wahl").is_none(), "Effort braucht L10");
    assert!(l.mit_kennung("modell-wahl").is_some(), "Modell beim Start");
    l.klick(l.mit_kennung("sitzung-start"), "Starten");
    assert!(
        l.mit_kennung("B71").is_some(),
        "Meldung bei abgelehntem Start"
    );
    l.taste(Key::Escape, Modifiers::NONE);
    // the settings: only the look of this device
    l.klick(l.mit_kennung("B23"), "B23");
    assert!(l.mit_kennung("B77").is_some(), "Farbwahl in Grundform");
    assert!(l.mit_kennung("B78").is_none(), "Admin braucht L4");
    l.bilder(2);
    l.taste(Key::Escape, Modifiers::NONE);
    l.g.borrow_mut().dialog("s-1");
    l.g.borrow_mut().hinweis("s-1");
    l.bilder(2);
    l.g.borrow_mut().trennen();
    l.bilder(2);
    let leer = Lauf::neu(Form::Heute, |g| g.borrow_mut().nur_beendete());
    l.gesehen.extend(leer.gesehen);
    l.verboten.extend(leer.verboten);
    l
}

fn abgleich(gesehen: &BTreeSet<String>, soll: &[&str]) -> (Vec<String>, Vec<String>) {
    let soll: BTreeSet<String> = soll.iter().map(|s| (*s).to_owned()).collect();
    (
        soll.difference(gesehen).cloned().collect(),
        gesehen.difference(&soll).cloned().collect(),
    )
}

#[test]
fn the_target_state_shows_every_built_pilot_path_and_nothing_else() {
    let l = ziel_abfahren();
    let (fehlend, ueberzaehlig) = abgleich(&l.gesehen, &bedienwege::alle_sichtbaren(Luecken::ALLE));
    assert!(
        l.verboten.is_empty(),
        "Kennungen mit Umfang N oder E im Baum: {:?}",
        l.verboten
    );
    assert!(
        fehlend.is_empty() && ueberzaehlig.is_empty(),
        "fehlend {fehlend:?}, überzählig {ueberzaehlig:?}"
    );
}

#[test]
fn today_the_paths_without_a_gap_and_the_base_forms_show() {
    let l = heute_abfahren();
    let (fehlend, ueberzaehlig) =
        abgleich(&l.gesehen, &bedienwege::alle_sichtbaren(Luecken::KEINE));
    assert!(
        l.verboten.is_empty(),
        "Kennungen mit Umfang N oder E im Baum: {:?}",
        l.verboten
    );
    assert!(
        fehlend.is_empty() && ueberzaehlig.is_empty(),
        "fehlend {fehlend:?}, überzählig {ueberzaehlig:?}"
    );
}

#[test]
fn the_report_per_operating_path() {
    let ziel = ziel_abfahren();
    let heute = heute_abfahren();
    let mut zeilen = vec![
        "| Nr | Bedienweg | Ergebnis | Grund |".to_owned(),
        "|---|---|---|---|".to_owned(),
    ];
    let mut zahl = BTreeMap::new();
    for b in BEDIENWEGE.iter().filter(|b| b.umfang == Umfang::P) {
        let (stand, grund) = if let Some((_, g)) = FEHLT.iter().find(|(n, _)| *n == b.nr) {
            ("fehlt", (*g).to_owned())
        } else if heute.gesehen.contains(b.nr) && b.grundform && b.luecken.iter().any(|l| *l != 8) {
            let l: Vec<String> = b
                .luecken
                .iter()
                .filter(|l| **l != 8)
                .map(|l| format!("L{l}"))
                .collect();
            (
                "angebunden in Grundform",
                format!(
                    "ohne die Teile aus {}; L11 fehlt allen Bedienwegen des Fensters",
                    l.join(", ")
                ),
            )
        } else if heute.gesehen.contains(b.nr) {
            (
                "angebunden",
                "ohne Lücke; L11 fehlt allen Bedienwegen des Fensters".to_owned(),
            )
        } else if ziel.gesehen.contains(b.nr) {
            let l: Vec<String> = b
                .luecken
                .iter()
                .filter(|l| **l != 8)
                .map(|l| format!("L{l}"))
                .collect();
            let g = if l.is_empty() {
                let t: Vec<String> = bedienwege::traeger(b.nr)
                    .iter()
                    .map(|t| {
                        let tl: Vec<String> = bedienwege::finde(t)
                            .map(|x| {
                                x.luecken
                                    .iter()
                                    .filter(|l| **l != 8)
                                    .map(|l| format!("L{l}"))
                                    .collect()
                            })
                            .unwrap_or_default();
                        format!(
                            "{t} ({})",
                            if tl.is_empty() {
                                "über Träger".to_owned()
                            } else {
                                tl.join(", ")
                            }
                        )
                    })
                    .collect();
                format!("liegt in {}", t.join(" oder "))
            } else {
                l.join(", ")
            };
            ("nur Attrappe", g)
        } else {
            ("fehlt", "im Lauf nicht gesehen".to_owned())
        };
        *zahl.entry(stand).or_insert(0) += 1;
        zeilen.push(format!("| {} | {} | {stand} | {grund} |", b.nr, b.name));
    }
    zeilen.push(String::new());
    zeilen.push(format!("Zählung: {zahl:?}"));
    if let Some(p) = std::env::var_os("FENSTER_BERICHT") {
        std::fs::write(p, zeilen.join("\n")).expect("Bericht schreibbar");
    }
    assert_eq!(zahl.values().sum::<i32>(), 47);
}

#[test]
fn keys_in_the_terminal_go_out_as_events_of_the_view() {
    let mut l = Lauf::neu(Form::Heute, |_| {});
    l.aktion(l.mit_kennung("B35"), Action::Focus, "Terminal");
    assert_eq!(l.fokus_kennung().as_deref(), Some("B35"));
    let vorher = l.ereignisse().len();
    l.bild(vec![Event::Text("ä".into())]);
    l.bild(vec![Event::Ime(egui::ImeEvent::Commit("ê".into()))]);
    l.taste(Key::Enter, Modifiers::SHIFT);
    l.taste(Key::C, Modifiers::CTRL);
    l.taste(Key::Escape, Modifiers::NONE);
    l.taste(Key::Tab, Modifiers::NONE);
    l.bild(vec![Event::Paste("zwei\nZeilen".into())]);
    let neu: Vec<Value> = l.ereignisse()[vorher..]
        .iter()
        .filter(|e| e["art"] != "Groesse" && e["art"] != "Abo")
        .cloned()
        .collect();
    assert_eq!(
        neu,
        vec![
            json!({"art": "Text", "sitzung": "s-1", "text": "ä"}),
            json!({"art": "Text", "sitzung": "s-1", "text": "ê"}),
            json!({"art": "Taste", "sitzung": "s-1", "taste": "Enter", "umschalter": ["umschalt"]}),
            json!({"art": "Taste", "sitzung": "s-1", "taste": "c", "umschalter": ["strg"]}),
            json!({"art": "Taste", "sitzung": "s-1", "taste": "Escape", "umschalter": []}),
            json!({"art": "Taste", "sitzung": "s-1", "taste": "Tab", "umschalter": []}),
            json!({"art": "Einfuegen", "sitzung": "s-1", "text": "zwei\nZeilen"}),
        ]
    );
    // Tab and Escape stay with the program: the focus is still in the terminal (SE2 point 3)
    assert_eq!(l.fokus_kennung().as_deref(), Some("B35"));
}

#[test]
fn f6_walks_the_areas_in_a_circle() {
    let mut l = Lauf::neu(Form::Ziel, |_| {});
    l.klick(l.mit_kennung("B04"), "B04");
    l.aktion(l.mit_kennung("B35"), Action::Focus, "Terminal");
    // terminal -> inspector (open) -> title row -> sidebar -> terminal
    l.taste(Key::F6, Modifiers::NONE);
    let im_inspektor = l
        .fokus
        .and_then(|f| l.baum.get(&f))
        .is_some_and(|n| n.role() == Role::Tab);
    assert!(
        im_inspektor,
        "F6 vom Terminal in den Inspektor, Fokus {:?}",
        l.fokus_kennung()
    );
    l.taste(Key::F6, Modifiers::NONE);
    // the first stop of the title row is the button of the sidebar (B80, OF1 point 2)
    let titel = l.fokus_kennung();
    assert_eq!(titel.as_deref(), Some("B80"), "Titelleiste, erster Halt");
    l.taste(Key::F6, Modifiers::NONE);
    assert_eq!(
        l.fokus_kennung().as_deref(),
        Some("B08"),
        "Leiste, erster Halt"
    );
    l.taste(Key::F6, Modifiers::NONE);
    assert_eq!(
        l.fokus_kennung().as_deref(),
        Some("B35"),
        "zurück im Terminal"
    );
    // Shift+F6 from the terminal: the button of the chosen session (SE2 point 4)
    l.taste(Key::F6, Modifiers::SHIFT);
    assert_eq!(l.fokus_kennung().as_deref(), Some("B10"));
    // F6 never goes to the program
    assert!(
        l.ereignisse()
            .iter()
            .all(|e| e.get("taste").and_then(Value::as_str) != Some("F6"))
    );
}

#[test]
fn every_control_has_a_name() {
    let l = Lauf::neu(Form::Ziel, |_| {});
    let rollen = [
        Role::Button,
        Role::TextInput,
        Role::MultilineTextInput,
        Role::ComboBox,
        Role::Tab,
        Role::ListBoxOption,
    ];
    let ohne: Vec<String> = l
        .baum
        .values()
        .filter(|n| {
            rollen.contains(&n.role())
                && n.label().is_none_or(str::is_empty)
                && n.labelled_by().is_empty()
        })
        .map(|n| format!("{:?} {:?}", n.role(), n.author_id()))
        .collect();
    assert!(ohne.is_empty(), "Bedienelemente ohne Namen: {ohne:?}");
}
