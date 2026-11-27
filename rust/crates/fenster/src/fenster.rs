//! The window: it reads the view core, draws, and sends the events of the view through the [`Weg`]. The state of the
//! sessions, the choice of a session, the keyboard rules, the texts of rejections and announcements live in the view
//! core of the crate `ansicht` ([`ansicht::kern::Kern`]); this module keeps only what it draws (open windows, widths,
//! the tab of the inspector, the focus area).

// Each drawing function draws one area of the window in its layout order, so several are long.
#![allow(clippy::too_many_lines, clippy::many_single_char_names)]

use std::path::PathBuf;

use ansicht::kern::{Kern, Wirkung};
use ansicht::tasten::Deutung;
use egui::accesskit::{Live, Role};
use egui::{
    Align, Align2, Color32, CornerRadius, FontId, Frame, Id, Key, Layout, Margin, Modifiers, Pos2,
    Rect, Response, RichText, Sense, Stroke, StrokeKind, Ui, UiBuilder, Vec2, pos2, vec2,
};
use rasterauszug::Empfang;
use serde_json::{Value, json};

use crate::bedienwege::{Luecken, sichtbar};
use crate::gestalt::{self, Farbsatz, MASSE, Rollen, Terminalwahl, Thema, Wahl};
use crate::merken::Merken;
use crate::symbole::{self, Symbol};
use crate::tasten::{self, Tat};
use crate::terminal::{self, Markierung};
use crate::weg::Weg;
use crate::ziel::Zusatz;

/// The caption of every picture of the mock-up (OF8 point 3).
pub const BESCHRIFTUNG: &str = "Attrappe, Zielzustand";

/// The areas of the window for F6 (SE2 point 4), in their order.
const BEREICHE: usize = 4;
const TITEL: usize = 0;
const LEISTE: usize = 1;
const TERMINAL: usize = 2;
const INSPEKTOR: usize = 3;

/// The tab of the inspector (B51).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reiter {
    /// Requests (B56).
    Freigaben,
    /// Folder (B52).
    Ordner,
}

/// The view of the middle (B27).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mitte {
    /// One terminal over the whole area.
    Orchestrator,
    /// The panes of the workers, tiled.
    Worker,
}

/// The form of the tiles (B31).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kachelform {
    /// Every worker, small.
    Uebersicht,
    /// Six per tab.
    Gekachelt,
    /// One worker over the whole area.
    Gezoomt,
}

/// A page of the settings (B72, B78).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Seite {
    /// Look (B77).
    Aussehen,
    /// Defaults for new sessions.
    Sitzung,
    /// Harnesses and models (as a user read only, as an admin editable).
    Harnesses,
    /// Admin: limits of the effort.
    AdminEffort,
}

/// A question before an irreversible step (B16, B17).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Rueckfrage {
    /// Delete session.
    Loeschen(String),
    /// Revive session.
    Wiederbeleben(String),
    /// Rename session, with the text of the field.
    Umbenennen(String, String),
}

/// Where the focus shall go in the next drawing.
#[derive(Clone, Debug, PartialEq, Eq)]
enum FokusZiel {
    Terminal,
    Plus,
    Name,
    Sitzung(String),
    Bereich(usize),
}

/// The state of the window "Sitzungen" (B68).
#[derive(Clone, Debug, Default)]
struct Sitzungsfenster {
    gewaehlt: Option<String>,
    name: String,
    effort: String,
}

/// What is open and chosen in the window.
#[derive(Clone, Debug)]
struct Bedienung {
    leiste_offen: bool,
    reiter: Reiter,
    mitte: Mitte,
    worker_liste: bool,
    sitzungen: Option<Sitzungsfenster>,
    einstellungen: Option<Seite>,
    menue: Option<(String, Pos2)>,
    rueckfrage: Option<Rueckfrage>,
    kachelform: Kachelform,
    tab: usize,
    zoom: Option<String>,
    suche: String,
    begruendung: String,
    ordner_angefragt: Option<String>,
    ziehen: Option<String>,
}

impl Default for Bedienung {
    fn default() -> Self {
        Bedienung {
            leiste_offen: true,
            reiter: Reiter::Freigaben,
            mitte: Mitte::Orchestrator,
            worker_liste: false,
            sitzungen: None,
            einstellungen: None,
            menue: None,
            rueckfrage: None,
            kachelform: Kachelform::Gekachelt,
            tab: 0,
            zoom: None,
            suche: String::new(),
            begruendung: String::new(),
            ordner_angefragt: None,
            ziehen: None,
        }
    }
}

/// A situation of the window that the pictures and the runner set directly.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Szene {
    /// View "Orchestrator", inspector closed.
    OrchestratorZu,
    /// View "Orchestrator", inspector open at this tab.
    OrchestratorOffen(Reiter),
    /// View "Worker" with tiles.
    WorkerKacheln,
    /// View "Worker" with the worker list open.
    WorkerListe,
    /// The window "Sitzungen" with an empty form.
    SitzungenLeer,
    /// The window "Sitzungen" with this ended session chosen.
    SitzungenGewaehlt(String),
    /// The settings at this page.
    Einstellungen(Seite),
}

/// The window.
pub struct Fenster {
    kern: Kern,
    zusatz: Zusatz,
    weg: Box<dyn Weg>,
    geschlossen: Luecken,
    attrappe: bool,
    merken: Merken,
    merken_datei: Option<PathBuf>,
    b: Bedienung,
    kacheln: Empfang,
    kachel_abo: Vec<(u8, (usize, usize))>,
    ansage: String,
    wecker: Option<f64>,
    kurz: Option<(String, f64)>,
    getrennt: bool,
    groesse: (usize, usize),
    markierung: Option<Markierung>,
    fokus: Option<FokusZiel>,
    bereiche: [Vec<Id>; BEREICHE],
    bereiche_neu: [Vec<Id>; BEREICHE],
    modal_halte: Vec<Id>,
    modal_halte_neu: Vec<Id>,
    terminal_id: Id,
    jetzt: f64,
    /// Fixed theme of the system for the pictures (`None`: ask egui).
    pub system_dunkel: Option<bool>,
    wahl: Wahl,
    terminalschrift: f32,
    merken_geaendert: bool,
}

fn kennung(ui: &Ui, id: Id, bnn: &str) {
    ui.ctx()
        .accesskit_node_builder(id, |n| n.set_author_id(bnn.to_owned()));
}

fn name_setzen(ui: &Ui, id: Id, name: &str) {
    ui.ctx()
        .accesskit_node_builder(id, |n| n.set_label(name.to_owned()));
}

fn rolle_setzen(ui: &Ui, id: Id, rolle: Role) {
    ui.ctx().accesskit_node_builder(id, |n| n.set_role(rolle));
}

/// A node for a container (`ui.unique_id()`), with role, name and an identifier.
fn behaelter(ui: &Ui, rolle: Role, name: &str, bnn: Option<&str>) {
    let id = ui.unique_id();
    ui.ctx().accesskit_node_builder(id, |n| {
        n.set_role(rolle);
        if !name.is_empty() {
            n.set_label(name.to_owned());
        }
        if let Some(b) = bnn {
            n.set_author_id(b.to_owned());
        }
    });
}

fn zustand_farbe(r: &Rollen, zustand: &str) -> Color32 {
    match zustand {
        "laeuft" => r.laeuft,
        "wartet" => r.wartet,
        "fertig" => r.fertig,
        _ => r.tot,
    }
}

fn punkt(ui: &mut Ui, farbe: Color32, voll: bool) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(MASSE.punkt), Sense::hover());
    let p = ui.painter();
    if voll {
        p.circle_filled(rect.center(), MASSE.punkt / 2.0, farbe);
    } else {
        p.circle_stroke(
            rect.center(),
            MASSE.punkt / 2.0 - 0.75,
            Stroke::new(1.5, farbe),
        );
    }
}

/// A capsule of the title row (36 pt, ground of the sidebar, fully round) around `inhalt`.
fn kapsel<R>(ui: &mut Ui, r: &Rollen, inhalt: impl FnOnce(&mut Ui) -> R) -> R {
    let rand = (MASSE.kapsel - MASSE.kapsel_segment) / 2.0;
    // the margin is a whole number of points
    #[allow(clippy::cast_possible_truncation)]
    let m = rand.round() as i8;
    Frame::new()
        .fill(r.leiste)
        .corner_radius(gestalt::radius(MASSE.radius_kapsel))
        .inner_margin(Margin::same(m))
        .show(ui, |ui| {
            ui.set_min_height(MASSE.kapsel_segment);
            inhalt(ui)
        })
        .inner
}

/// The look of the segments inside a capsule or a segmented field: 28 pt high and round.
fn segment_stil(ui: &mut Ui) {
    ui.spacing_mut().item_spacing.x = 0.0;
    ui.spacing_mut().interact_size.y = MASSE.kapsel_segment;
    ui.spacing_mut().button_padding = vec2(14.0, 4.0);
    let rad = gestalt::radius(MASSE.kapsel_segment / 2.0);
    let w = &mut ui.visuals_mut().widgets;
    for x in [&mut w.inactive, &mut w.hovered, &mut w.active, &mut w.open] {
        x.corner_radius = rad;
        x.bg_stroke = Stroke::NONE;
    }
}

/// Paints `text` in one line from `pos`; longer text ends with "…" at `breite` (OF5 point 2).
fn text_gekuerzt(ui: &Ui, pos: Pos2, text: &str, font: FontId, farbe: Color32, breite: f32) {
    let mut job = egui::text::LayoutJob::simple_singleline(text.to_owned(), font, farbe);
    job.wrap = egui::text::TextWrapping::truncate_at_width(breite.max(1.0));
    let g = ui.painter().layout_job(job);
    ui.painter().galley(pos, g, farbe);
}

/// `text` cut at the front with "…" so that it fits `breite` (paths keep their end, like the reference).
fn vorn_gekuerzt(ui: &Ui, text: &str, font: &FontId, breite: f32) -> String {
    let breite_von = |t: &str| {
        ui.painter()
            .layout_no_wrap(t.to_owned(), font.clone(), Color32::WHITE)
            .size()
            .x
    };
    if breite_von(text) <= breite {
        return text.to_owned();
    }
    let zeichen: Vec<char> = text.chars().collect();
    for i in 1..zeichen.len() {
        let t: String = std::iter::once('…')
            .chain(zeichen[i..].iter().copied())
            .collect();
        if breite_von(&t) <= breite {
            return t;
        }
    }
    "…".to_owned()
}

/// One row of the settings: key, name, one line of description and the choices (value, text).
struct Einstellungszeile {
    schluessel: &'static str,
    name: &'static str,
    beschreibung: &'static str,
    wahlen: &'static [(&'static str, &'static str)],
}

/// A switch (on: the accent surface, off: the line colour, a light knob), named for the accessibility tree.
fn schalter_malen(ui: &mut Ui, r: &Rollen, an: bool, name: &str) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(38.0, 22.0), Sense::click());
    let farbe = if !ui.is_enabled() {
        r.linie
    } else if an {
        r.akzent_flaeche
    } else {
        r.wahl
    };
    ui.painter().rect_filled(rect, gestalt::radius(11.0), farbe);
    let x = if an {
        rect.right() - 11.0
    } else {
        rect.left() + 11.0
    };
    ui.painter().circle_filled(
        pos2(x, rect.center().y),
        9.0,
        Color32::from_rgb(0xff, 0xff, 0xff),
    );
    let name = name.to_owned();
    resp.widget_info(|| egui::WidgetInfo::selected(Role::Switch, ui.is_enabled(), an, &name));
    resp
}

/// The word of a state as the reference writes it ("läuft", not "laeuft").
fn zustand_wort(z: &str) -> &str {
    match z {
        "laeuft" => "läuft",
        anders => anders,
    }
}

fn wert_text(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Null => String::new(),
        x => x.to_string(),
    }
}

impl Fenster {
    /// A new window on the path `weg`; `merken` are the remembered values, `merken_datei` the file to write them to
    /// (`None`: nothing is written, as in the tests and the pictures).
    #[must_use]
    pub fn neu(weg: Box<dyn Weg>, merken: Merken, merken_datei: Option<PathBuf>) -> Fenster {
        let mut kern = Kern::default();
        kern.offen(false);
        let geschlossen = weg.geschlossen();
        let attrappe = weg.ist_attrappe();
        let gemerkt = merken.aussehen.clone();
        let mut f = Fenster {
            kern,
            zusatz: Zusatz::default(),
            weg,
            geschlossen,
            attrappe,
            merken,
            merken_datei,
            b: Bedienung::default(),
            kacheln: Empfang::neu(false),
            kachel_abo: Vec::new(),
            ansage: String::new(),
            wecker: None,
            kurz: None,
            getrennt: false,
            groesse: (80, 24),
            markierung: None,
            fokus: None,
            bereiche: Default::default(),
            bereiche_neu: Default::default(),
            modal_halte: Vec::new(),
            modal_halte_neu: Vec::new(),
            terminal_id: Id::unique("terminal-raster"),
            jetzt: 0.0,
            system_dunkel: None,
            wahl: Wahl::default(),
            terminalschrift: MASSE.schrift_terminal,
            merken_geaendert: false,
        };
        // the look chosen on this device; the settings of the service (L4) come later and win
        for (k, v) in &gemerkt {
            f.aussehen_anwenden(k, v);
        }
        f
    }

    /// Whether operating path `nr` is shown with the gaps of this path (OF7, OF8).
    #[must_use]
    pub fn zeigt(&self, nr: &str) -> bool {
        sichtbar(nr, self.geschlossen)
    }

    /// Whether gap `l` of section 4 is closed: the parts of a base form that need it show only then (OF8 point 3).
    fn hat(&self, l: u8) -> bool {
        self.geschlossen.hat(l)
    }

    /// The remembered values (L8).
    #[must_use]
    pub fn merken(&self) -> &Merken {
        &self.merken
    }

    /// The view core (read only).
    #[must_use]
    pub fn kern(&self) -> &Kern {
        &self.kern
    }

    /// The last announcement of the live region (SE3 point 3).
    #[must_use]
    pub fn ansage(&self) -> &str {
        &self.ansage
    }

    /// Sets a situation directly (pictures, runner).
    pub fn szene(&mut self, s: &Szene) {
        self.b.sitzungen = None;
        self.b.einstellungen = None;
        self.b.worker_liste = false;
        self.b.menue = None;
        match s {
            Szene::OrchestratorZu => {
                self.b.mitte = Mitte::Orchestrator;
                self.merken.inspektor_offen = false;
            }
            Szene::OrchestratorOffen(r) => {
                self.b.mitte = Mitte::Orchestrator;
                self.merken.inspektor_offen = true;
                self.b.reiter = *r;
            }
            Szene::WorkerKacheln => {
                self.b.mitte = Mitte::Worker;
                self.merken.inspektor_offen = false;
            }
            Szene::WorkerListe => {
                self.b.mitte = Mitte::Worker;
                self.merken.inspektor_offen = false;
                self.b.worker_liste = true;
            }
            Szene::SitzungenLeer => {
                self.b.sitzungen = Some(Sitzungsfenster::default());
            }
            Szene::SitzungenGewaehlt(id) => {
                self.b.sitzungen = Some(Sitzungsfenster {
                    gewaehlt: Some(id.clone()),
                    ..Sitzungsfenster::default()
                });
            }
            Szene::Einstellungen(seite) => self.b.einstellungen = Some(*seite),
        }
    }

    fn wirkungen(&mut self, w: Vec<Wirkung>) {
        for x in w {
            match x {
                Wirkung::Senden(v) => self.weg.senden(&v),
                Wirkung::Ansage(t) => self.ansage = t,
                Wirkung::Wecker(ms) => self.wecker = Some(self.jetzt + ms),
                Wirkung::FokusTerminal => self.fokus = Some(FokusZiel::Terminal),
                Wirkung::FokusStart => self.fokus = Some(FokusZiel::Plus),
                Wirkung::StromZu => self.getrennt = true,
                Wirkung::Notiz(t) => {
                    self.kurz = Some((t, self.jetzt + MASSE.kurzmeldung_s * 1000.0));
                }
            }
        }
    }

    fn senden(&mut self, e: &Value) {
        self.weg.senden(e);
    }

    /// Takes the frames of the stream.
    fn empfangen(&mut self) {
        for r in self.weg.abholen() {
            if r.art == ansicht::kern::ART_RASTER {
                // the pictures of the worker panes (L2): the view core holds only the picture of the chosen session
                let _ = self.kacheln.nimm(&r.nutz);
            }
            if r.art == ansicht::kern::ART_NACHRICHT
                && let Ok(j) = serde_json::from_slice::<Value>(&r.nutz)
            {
                self.zusatz.nachricht(&j);
                if j.get("art").and_then(Value::as_str) == Some("Gestartet") {
                    let w = self.kern.start_antwort(200, Some(&j));
                    self.wirkungen(w);
                    self.b.sitzungen = None;
                }
                if j.get("art").and_then(Value::as_str) == Some("Einstellungen") {
                    self.wahl_aus_einstellungen();
                }
            }
            let w = self.kern.rahmen(r.art, &r.nutz, self.jetzt, self.groesse);
            self.wirkungen(w);
        }
        if !self.weg.offen() && !self.getrennt {
            self.getrennt = true;
            let w = self.kern.ende();
            self.wirkungen(w);
        }
    }

    fn wahl_aus_einstellungen(&mut self) {
        let wert = |k: &str| {
            self.zusatz
                .einstellung(k)
                .and_then(|e| e.wert.as_str().map(str::to_owned))
                .unwrap_or_default()
        };
        self.wahl.thema = match wert("aussehen.thema").as_str() {
            "hell" => Thema::Hell,
            "dunkel" => Thema::Dunkel,
            _ => Thema::System,
        };
        self.wahl.farbsatz = match wert("aussehen.farbsatz").as_str() {
            "blauschwarz" => Farbsatz::Blauschwarz,
            _ => Farbsatz::Grau,
        };
        #[allow(clippy::cast_possible_truncation)]
        if let Some(g) = self
            .zusatz
            .einstellung("aussehen.schriftgroesse")
            .and_then(|e| e.wert.as_f64())
            .filter(|g| (8.0..=32.0).contains(g))
        {
            self.terminalschrift = g as f32;
        }
        self.wahl.terminal = match wert("aussehen.terminal").as_str() {
            "hell" => Terminalwahl::Hell,
            "wie_thema" => Terminalwahl::WieThema,
            _ => Terminalwahl::Dunkel,
        };
    }

    /// Sets the look choice directly (pictures).
    pub fn wahl_setzen(&mut self, w: Wahl) {
        self.wahl = w;
    }

    fn rollen(&self, ui: &Ui) -> &'static Rollen {
        let sys = self
            .system_dunkel
            .unwrap_or_else(|| ui.ctx().system_theme() != Some(egui::Theme::Light));
        self.wahl.rollen(sys)
    }

    fn system(&self, ui: &Ui) -> bool {
        self.system_dunkel
            .unwrap_or_else(|| ui.ctx().system_theme() != Some(egui::Theme::Light))
    }

    fn halt(&mut self, bereich: usize, r: &Response) {
        if self.b.sitzungen.is_some()
            || self.b.einstellungen.is_some()
            || self.b.rueckfrage.is_some()
        {
            return;
        }
        self.bereiche_neu[bereich].push(r.id);
    }

    /// F6 and Shift+F6 (SE2 point 4): the first stop of the next or previous area.
    fn bereich_wechseln(&mut self, ui: &Ui, richtung: i8) {
        let fokus = ui.ctx().memory(egui::Memory::focused);
        let jetzt = fokus.and_then(|f| self.bereiche.iter().position(|b| b.contains(&f)));
        if jetzt == Some(TERMINAL)
            && richtung < 0
            && let Some(a) = self.kern.aktiv.clone()
            && self.zeigt("B10")
        {
            self.fokus = Some(FokusZiel::Sitzung(a));
            return;
        }
        let start = jetzt.unwrap_or(TERMINAL);
        for schritt in 1..=BEREICHE {
            #[allow(clippy::cast_possible_wrap, clippy::cast_sign_loss)]
            let i = (start as isize + isize::from(richtung) * schritt as isize)
                .rem_euclid(BEREICHE as isize) as usize;
            if !self.bereiche[i].is_empty() {
                self.fokus = Some(FokusZiel::Bereich(i));
                return;
            }
        }
    }

    fn fokus_nehmen(&mut self, r: &Response, ziel: &FokusZiel) {
        if self.fokus.as_ref() == Some(ziel) {
            r.request_focus();
            self.fokus = None;
        }
    }

    /// The whole window; called by `eframe` and by the runner and the pictures without a window.
    pub fn zeichnen(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        self.jetzt = ctx.input(|i| i.time) * 1000.0;
        let rollen = *self.rollen(ui);
        ctx.set_visuals(gestalt::visuals(&rollen));
        self.empfangen();
        if let Some(t) = self.wecker
            && self.jetzt >= t
        {
            self.wecker = None;
            let w = self.kern.wecken(self.jetzt);
            self.wirkungen(w);
        }
        if let Some((_, bis)) = &self.kurz
            && self.jetzt >= *bis
        {
            self.kurz = None;
        }
        self.bereiche = std::mem::take(&mut self.bereiche_neu);
        self.modal_halte = std::mem::take(&mut self.modal_halte_neu);
        let terminal_fokus = ctx.memory(|m| m.has_focus(self.terminal_id));
        // F6 outside the terminal; inside it the view core reads it (SE2 point 4)
        if !terminal_fokus && self.b.sitzungen.is_none() && self.b.einstellungen.is_none() {
            let (vor, zurueck) = ctx.input_mut(|i| {
                (
                    i.consume_key(Modifiers::NONE, Key::F6),
                    i.consume_key(Modifiers::SHIFT, Key::F6),
                )
            });
            if vor {
                self.bereich_wechseln(ui, 1);
            }
            if zurueck {
                self.bereich_wechseln(ui, -1);
            }
        }
        // Escape closes a window or bubble (B76); in the terminal it belongs to the program
        if !terminal_fokus
            && (self.b.worker_liste || self.b.menue.is_some())
            && ctx.input(|i| i.key_pressed(Key::Escape))
        {
            self.b.worker_liste = false;
            self.b.menue = None;
        }
        // Shift+F10 opens the menu of the focused session row
        let menue_taste = ctx.input_mut(|i| i.consume_key(Modifiers::SHIFT, Key::F10));

        ctx.global_style_mut(gestalt::stil);
        let grund = rollen.grund;
        egui::Panel::top("titelleiste")
            .exact_size(MASSE.titelleiste + MASSE.linie)
            .show_separator_line(false)
            .frame(Frame::new().fill(rollen.fenster).stroke(Stroke::NONE))
            .show(ui, |ui| self.titelleiste(ui, &rollen));
        if self.b.leiste_offen {
            let leiste = egui::Panel::left("leiste")
                .resizable(true)
                .show_separator_line(false)
                .default_size(self.merken.leiste)
                .size_range(MASSE.leiste_grenzen.0..=MASSE.leiste_grenzen.1)
                .frame(Frame::new().fill(rollen.leiste))
                .show(ui, |ui| self.leiste(ui, &rollen, menue_taste));
            let breite = leiste.response.rect.width();
            if (breite - self.merken.leiste).abs() > 0.5 {
                self.merken.leiste = breite;
                self.merken_geaendert = true;
            }
            if self.zeigt("B24") {
                let r = Rect::from_x_y_ranges(
                    leiste.response.rect.right() - 3.0..=leiste.response.rect.right() + 3.0,
                    leiste.response.rect.y_range(),
                );
                Self::griff(ui, r, "B24", "Breite der Leiste ziehen");
            }
        }
        let mut inspektor_rect = None;
        if self.merken.inspektor_offen && self.zeigt("B51") {
            let r = egui::Panel::right("inspektor")
                .resizable(true)
                .default_size(self.merken.inspektor)
                .size_range(MASSE.inspektor_grenzen.0..=MASSE.inspektor_grenzen.1)
                .show_separator_line(false)
                .frame(Frame::new().fill(rollen.inspektor))
                .show(ui, |ui| {
                    ui.allocate_space(ui.available_size());
                });
            let breite = r.response.rect.width();
            if (breite - self.merken.inspektor).abs() > 0.5 {
                self.merken.inspektor = breite;
                self.merken_geaendert = true;
            }
            inspektor_rect = Some(r.response.rect);
        }
        egui::CentralPanel::default()
            .frame(Frame::new().fill(grund))
            .show(ui, |ui| self.mitte(ui, &rollen));
        if let Some(rect) = inspektor_rect {
            let mut iu = ui.new_child(UiBuilder::new().max_rect(rect).id_salt("inspektor-inhalt"));
            iu.scope_builder(UiBuilder::new().id_salt("inspektor-bereich"), |ui| {
                behaelter(ui, Role::Complementary, "Inspektor", None);
                ui.ctx()
                    .accesskit_node_builder(ui.unique_id(), |n| n.set_class_name("inspektor"));
                self.inspektor(ui, &rollen);
            });
            if self.zeigt("B57") {
                let g =
                    Rect::from_x_y_ranges(rect.left() - 3.0..=rect.left() + 3.0, rect.y_range());
                Self::griff(ui, g, "B57", "Breite des Inspektors ziehen");
            }
        }
        self.blasen(ui, &rollen);
        self.sitzungen_fenster(ui, &rollen);
        self.einstellungen_fenster(ui, &rollen);
        self.rueckfrage(ui);
        self.live_region(ui);
        Self::fokusrahmen(ui, &rollen);
        if self.attrappe {
            Self::beschriftung(ui, &rollen);
        }
        // the focus trap of an open window (SE2 point 2)
        if (self.b.sitzungen.is_some() || self.b.rueckfrage.is_some())
            && !self.modal_halte.is_empty()
        {
            let f = ctx.memory(egui::Memory::focused);
            if f.is_none_or(|f| {
                !self.modal_halte.contains(&f) && !self.modal_halte_neu.contains(&f)
            }) && let Some(erst) = self.modal_halte_neu.first()
            {
                ctx.memory_mut(|m| m.request_focus(*erst));
            }
        }
        if let Some(FokusZiel::Bereich(i)) = self.fokus.clone() {
            if let Some(id) = self.bereiche_neu[i].first() {
                ctx.memory_mut(|m| m.request_focus(*id));
            }
            self.fokus = None;
        }
        if self.wecker.is_some() || self.kurz.is_some() {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
        self.merken_sichern(&ctx);
    }

    fn merken_sichern(&mut self, ctx: &egui::Context) {
        let (innen, aussen) = ctx.input(|i| {
            let v = i.viewport();
            (v.inner_rect, v.outer_rect)
        });
        if let Some(r) = innen {
            let g = Some((r.width(), r.height()));
            if g != self.merken.groesse {
                self.merken.groesse = g;
                self.merken_geaendert = true;
            }
        }
        if let Some(r) = aussen {
            let l = Some((r.left(), r.top()));
            if l != self.merken.lage {
                self.merken.lage = l;
                self.merken_geaendert = true;
            }
        }
        let losgelassen = ctx.input(|i| !i.pointer.any_down());
        if self.merken_geaendert && losgelassen {
            self.merken_geaendert = false;
            if let Some(p) = &self.merken_datei {
                let _ = self.merken.schreiben(p);
            }
        }
    }

    /// Writes the remembered values now (at the end of the program).
    pub fn merken_schreiben(&self) {
        if let Some(p) = &self.merken_datei {
            let _ = self.merken.schreiben(p);
        }
    }

    fn griff(ui: &Ui, rect: Rect, bnn: &str, name: &str) {
        let id = Id::unique(("griff", bnn));
        let _ = ui.interact(rect, id, Sense::hover());
        ui.ctx().accesskit_node_builder(id, |n| {
            n.set_role(Role::Splitter);
            n.set_label(name.to_owned());
            n.set_author_id(bnn.to_owned());
        });
    }

    // ------------------------------------------------------------------------------------------------ title row

    fn titelleiste(&mut self, ui: &mut Ui, r: &Rollen) {
        let voll = ui.max_rect();
        let zeile =
            Rect::from_min_max(voll.min, pos2(voll.right(), voll.top() + MASSE.titelleiste));
        ui.painter().hline(
            voll.x_range(),
            zeile.bottom() + MASSE.linie / 2.0,
            Stroke::new(MASSE.linie, r.linie),
        );
        let innen = zeile.shrink2(vec2(MASSE.innen, 0.0));
        ui.scope_builder(
            UiBuilder::new().id_salt("titel-bereich").max_rect(zeile),
            |ui| {
                behaelter(ui, Role::Banner, "Titelleiste", None);
                // left: the sidebar button and, under Windows and Linux, the application menu (B75)
                let links = ui
                    .scope_builder(
                        UiBuilder::new()
                            .max_rect(innen)
                            .layout(Layout::left_to_right(Align::Center)),
                        |ui| {
                            if self.zeigt("B80") {
                                let knopf = symbole::knopf(
                                    ui,
                                    Symbol::Leiste,
                                    "Leiste",
                                    MASSE.kapsel_segment,
                                    false,
                                );
                                name_setzen(
                                    ui,
                                    knopf.id,
                                    if self.b.leiste_offen {
                                        "Leiste ausblenden"
                                    } else {
                                        "Leiste einblenden"
                                    },
                                );
                                kennung(ui, knopf.id, "B80");
                                self.halt(TITEL, &knopf);
                                if knopf.clicked() {
                                    self.b.leiste_offen = !self.b.leiste_offen;
                                }
                            }
                            if self.zeigt("B75") {
                                self.menue(ui, r);
                            }
                            ui.min_rect().right()
                        },
                    )
                    .inner;
                // right: the capsules
                let rechts = ui
                    .scope_builder(
                        UiBuilder::new()
                            .max_rect(innen)
                            .layout(Layout::right_to_left(Align::Center)),
                        |ui| {
                            self.titel_rechts(ui, r);
                            ui.min_rect().left()
                        },
                    )
                    .inner;
                // between them the project name and its origin; both end before the capsules and cut with "…" (OF5 point 2)
                if self.zeigt("B02") {
                    let start = if self.b.leiste_offen {
                        (zeile.left() + self.merken.leiste + MASSE.titel_einzug)
                            .max(links + MASSE.abstand_kapsel)
                    } else {
                        links + MASSE.abstand_kapsel
                    };
                    let ende = rechts - MASSE.abstand_kapsel;
                    let (projekt, herkunft) = self.herkunft();
                    let feld = Rect::from_min_max(
                        pos2(start, zeile.top()),
                        pos2(ende.max(start + 1.0), zeile.bottom()),
                    );
                    let r2 = ui
                        .scope_builder(
                            UiBuilder::new()
                                .max_rect(feld)
                                .layout(Layout::top_down(Align::Min)),
                            |ui| {
                                ui.set_clip_rect(feld);
                                ui.add_space(10.0);
                                ui.spacing_mut().item_spacing.y = 1.0;
                                ui.add(
                                    egui::Label::new(
                                        RichText::new(&projekt).strong().size(MASSE.schrift_titel),
                                    )
                                    .truncate(),
                                );
                                ui.add(
                                    egui::Label::new(
                                        RichText::new(&herkunft)
                                            .size(MASSE.schrift_klein)
                                            .color(r.gedaempft),
                                    )
                                    .truncate(),
                                );
                            },
                        )
                        .response;
                    let id = r2.id;
                    ui.ctx().accesskit_node_builder(id, |n| {
                        n.set_role(Role::Heading);
                        n.set_label(format!("{projekt}, {herkunft}"));
                        n.set_author_id("B02".to_owned());
                    });
                }
            },
        );
    }

    /// The application menu in the title row (B75; under macOS it would be the menu bar of the system). It holds only
    /// entries whose operating path shows (OF8).
    fn menue(&mut self, ui: &mut Ui, r: &Rollen) {
        let k = symbole::knopf(ui, Symbol::Menue, "Menü", MASSE.kapsel_segment, false);
        kennung(ui, k.id, "B75");
        ui.ctx().accesskit_node_builder(k.id, |n| {
            n.set_role(Role::MenuBar);
            n.set_class_name("anwendungsmenue");
        });
        self.halt(TITEL, &k);
        let aktiv = self.kern.aktive_sitzung().cloned();
        egui::Popup::menu(&k).show(|ui| {
            ui.set_min_width(220.0);
            let eintrag = |ui: &mut Ui, text: &str, an: bool| -> bool {
                let e = ui.add_enabled(
                    an,
                    egui::Button::new(text)
                        .frame(false)
                        .min_size(vec2(200.0, 22.0)),
                );
                rolle_setzen(ui, e.id, Role::MenuItem);
                e.clicked()
            };
            ui.menu_button("Werkbank", |ui| {
                if self.zeigt("B72") && eintrag(ui, "Einstellungen …", true) {
                    self.b.einstellungen = Some(Seite::Aussehen);
                }
                if eintrag(ui, "Werkbank beenden", true) {
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                }
            });
            if self.zeigt("B68") {
                ui.menu_button("Ablage", |ui| {
                    if eintrag(ui, "Neue Sitzung …", true) {
                        self.b.sitzungen = Some(Sitzungsfenster::default());
                        self.fokus = Some(FokusZiel::Name);
                    }
                });
            }
            if self.zeigt("B38") {
                ui.menu_button("Bearbeiten", |ui| {
                    if eintrag(ui, "Kopieren", self.markierung.is_some())
                        && let (Some(b), Some(m)) =
                            (self.kern.bild.as_ref(), self.markierung.as_ref())
                    {
                        let t = terminal::markierter_text(b, m);
                        if !t.is_empty() {
                            ui.ctx().copy_text(t);
                        }
                    }
                    if eintrag(ui, "Einfügen", aktiv.is_some()) {
                        self.fokus = Some(FokusZiel::Terminal);
                        ui.ctx()
                            .send_viewport_cmd(egui::ViewportCommand::RequestPaste);
                    }
                });
            }
            if let Some(s) = &aktiv
                && (self.zeigt("B11")
                    || self.zeigt("B12")
                    || self.zeigt("B15")
                    || self.zeigt("B16"))
            {
                ui.menu_button("Sitzung", |ui| {
                    if self.zeigt("B11") && eintrag(ui, "Fortsetzen", !s.laeuft) {
                        self.senden(&json!({"art": "Fortsetzen", "sitzung": s.id}));
                    }
                    if self.zeigt("B12") && eintrag(ui, "Namen ändern …", true) {
                        let alt = self
                            .zusatz
                            .sitzung(&s.id)
                            .map(|x| x.name.clone())
                            .unwrap_or_default();
                        self.b.rueckfrage = Some(Rueckfrage::Umbenennen(s.id.clone(), alt));
                    }
                    if self.zeigt("B15") && eintrag(ui, "Sitzung schließen", s.laeuft) {
                        let w = self.kern.beenden(&s.id);
                        self.wirkungen(w);
                    }
                    if self.zeigt("B16") && eintrag(ui, "Endgültig löschen …", !s.laeuft) {
                        self.b.rueckfrage = Some(Rueckfrage::Loeschen(s.id.clone()));
                    }
                });
            }
            ui.menu_button("Darstellung", |ui| {
                if self.zeigt("B80") && eintrag(ui, "Leiste ein- oder ausblenden", true) {
                    self.b.leiste_offen = !self.b.leiste_offen;
                }
                if self.zeigt("B04") && eintrag(ui, "Inspektor ein- oder ausblenden", true) {
                    self.merken.inspektor_offen = !self.merken.inspektor_offen;
                    self.merken_geaendert = true;
                }
                if self.zeigt("B27") {
                    if eintrag(ui, "Orchestrator", self.b.mitte != Mitte::Orchestrator) {
                        self.b.mitte = Mitte::Orchestrator;
                        self.kachel_abo.clear();
                    }
                    if eintrag(ui, "Worker", self.b.mitte != Mitte::Worker) {
                        self.b.mitte = Mitte::Worker;
                        self.kachel_abo.clear();
                    }
                }
                if self.zeigt("B29") && eintrag(ui, "Worker-Liste", true) {
                    self.b.worker_liste = !self.b.worker_liste;
                }
            });
            let _ = r;
        });
    }

    fn herkunft(&self) -> (String, String) {
        let Some(s) = self.kern.aktive_sitzung() else {
            return ("Werkbank".to_owned(), "keine Sitzung gewählt".to_owned());
        };
        let plus = self.zusatz.sitzung(&s.id);
        let projekt = plus.map_or_else(|| s.id.clone(), |p| p.projekt.clone());
        let ordner = plus.map(|p| p.ordner.clone()).unwrap_or_default();
        let mut teile: Vec<String> = s.teile().iter().map(|x| (*x).to_owned()).collect();
        if !ordner.is_empty() {
            teile.push(ordner);
        }
        (projekt, teile.join(" · "))
    }

    fn titel_rechts(&mut self, ui: &mut Ui, r: &Rollen) {
        // right to left: the capsule of inspector and gear, view, pill, badge (OF1 point 2)
        ui.spacing_mut().item_spacing.x = MASSE.abstand_kapsel;
        let b28 = self.zeigt("B28");
        let b04 = self.zeigt("B04");
        if b28 || b04 {
            kapsel(ui, r, |ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                if b28 {
                    let k = symbole::knopf(
                        ui,
                        Symbol::Zahnrad,
                        "Einstellungen der Sitzung",
                        MASSE.kapsel_segment,
                        false,
                    );
                    kennung(ui, k.id, "B28");
                    self.halt(TITEL, &k);
                    if k.clicked() {
                        self.b.einstellungen = Some(Seite::Sitzung);
                    }
                }
                if b04 {
                    let offen = self.merken.inspektor_offen;
                    let k = symbole::knopf(
                        ui,
                        Symbol::Inspektor,
                        "Inspektor",
                        MASSE.kapsel_segment,
                        offen,
                    );
                    kennung(ui, k.id, "B04");
                    ui.ctx().accesskit_node_builder(k.id, |n| {
                        n.set_class_name("inspektor-umschalter");
                        n.set_toggled(if offen {
                            egui::accesskit::Toggled::True
                        } else {
                            egui::accesskit::Toggled::False
                        });
                    });
                    self.halt(TITEL, &k);
                    if k.clicked() {
                        self.merken.inspektor_offen = !offen;
                        self.merken_geaendert = true;
                    }
                }
            });
            // the reference sets this capsule apart (18 pt instead of 8 pt)
            ui.add_space(MASSE.abstand_kapsel_rechts - MASSE.abstand_kapsel);
        }
        if self.zeigt("B27") {
            kapsel(ui, r, |ui| {
                ui.scope_builder(UiBuilder::new().id_salt("ansicht-umschalter"), |ui| {
                    behaelter(ui, Role::RadioGroup, "Ansicht", Some("B27"));
                    ui.ctx().accesskit_node_builder(ui.unique_id(), |n| {
                        n.set_class_name("ansicht-umschalter");
                    });
                    segment_stil(ui);
                    for (m, t) in [
                        (Mitte::Worker, "Worker"),
                        (Mitte::Orchestrator, "Orchestrator"),
                    ] {
                        let k = ui.selectable_label(
                            self.b.mitte == m,
                            RichText::new(t).size(MASSE.schrift_kapsel),
                        );
                        self.halt(TITEL, &k);
                        if k.clicked() {
                            self.b.mitte = m;
                            self.kachel_abo.clear();
                        }
                    }
                });
            });
        }
        if self.zeigt("B26") {
            let n = self.kern.aktiv.as_deref().map_or(0, |a| {
                self.zusatz
                    .arbeiter_von(a)
                    .iter()
                    .filter(|w| w.zustand == "laeuft")
                    .count()
            });
            let k = ui.add(
                egui::Button::new(
                    RichText::new(format!("    {n} laufen"))
                        .size(MASSE.schrift_kapsel)
                        .color(r.schrift),
                )
                .fill(r.leiste)
                .stroke(Stroke::NONE)
                .corner_radius(gestalt::radius(MASSE.radius_kapsel))
                .min_size(vec2(0.0, MASSE.kapsel)),
            );
            name_setzen(ui, k.id, &format!("{n} laufen, Worker-Liste"));
            kennung(ui, k.id, "B26");
            self.halt(TITEL, &k);
            if k.clicked() {
                self.b.worker_liste = !self.b.worker_liste;
            }
            symbole::punkt(ui, k.rect.left_center() + vec2(16.0, 0.0), r.laeuft, true);
            ui.data_mut(|d| d.insert_temp(Id::unique("pille-rect"), k.rect));
        }
        if self.zeigt("B03") && !self.kern.anfragen.is_empty() {
            let n = self.kern.anfragen.len();
            let k = ui.add(
                egui::Button::new(
                    RichText::new(format!("     {n}"))
                        .color(r.wartet_tinte)
                        .size(MASSE.schrift_kapsel),
                )
                .fill(r.wartet_flaeche)
                .stroke(Stroke::NONE)
                .corner_radius(gestalt::radius(MASSE.radius_kapsel))
                .min_size(vec2(0.0, MASSE.kapsel)),
            );
            symbole::malen(
                ui,
                Symbol::Schild,
                Rect::from_center_size(k.rect.left_center() + vec2(16.0, 0.0), Vec2::splat(13.0)),
                r.wartet_tinte,
            );
            name_setzen(ui, k.id, &format!("{n} offene Freigaben"));
            kennung(ui, k.id, "B03");
            self.halt(TITEL, &k);
            if k.clicked() {
                self.merken.inspektor_offen = true;
                self.b.reiter = Reiter::Freigaben;
            }
        }
    }

    // ------------------------------------------------------------------------------------------------ sidebar

    fn leiste(&mut self, ui: &mut Ui, r: &Rollen, menue_taste: bool) {
        ui.scope_builder(UiBuilder::new().id_salt("leiste-bereich"), |ui| {
            behaelter(ui, Role::Navigation, "Leiste", None);
            let voll = ui.max_rect();
            // head with "+"
            ui.allocate_ui_with_layout(
                vec2(ui.available_width(), MASSE.leistenkopf),
                Layout::left_to_right(Align::Center),
                |ui| {
                    ui.add_space(MASSE.kopf_einzug);
                    ui.label(
                        RichText::new("Sitzungen")
                            .strong()
                            .size(MASSE.schrift_klein)
                            .color(r.gedaempft),
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.add_space(MASSE.innen);
                        if self.zeigt("B08") {
                            let k = symbole::knopf(
                                ui,
                                Symbol::Plus,
                                "Neue Sitzung",
                                MASSE.knopf,
                                false,
                            );
                            kennung(ui, k.id, "B08");
                            ui.ctx()
                                .accesskit_node_builder(k.id, |n| n.set_class_name("neue-sitzung"));
                            self.halt(LEISTE, &k);
                            let z = FokusZiel::Plus;
                            self.fokus_nehmen(&k, &z);
                            if k.clicked() {
                                self.b.sitzungen = Some(Sitzungsfenster::default());
                                self.fokus = Some(FokusZiel::Name);
                            }
                        }
                    });
                },
            );
            let fuss_h = MASSE.leistenfuss;
            let liste_h = (ui.available_height() - fuss_h).max(0.0);
            ui.allocate_ui_with_layout(
                vec2(ui.available_width(), liste_h),
                Layout::top_down(Align::Min),
                |ui| {
                    egui::ScrollArea::vertical()
                        .auto_shrink(false)
                        .show(ui, |ui| {
                            if self.zeigt("B10") {
                                self.sitzungsliste(ui, r, menue_taste);
                            } else if self.kern.aktiv.is_none() {
                                ui.add_space(8.0);
                                ui.horizontal(|ui| {
                                    ui.add_space(MASSE.innen);
                                    ui.label(
                                        RichText::new("Keine Sitzung gewählt").color(r.gedaempft),
                                    );
                                });
                            }
                        });
                },
            );
            // foot
            let fuss = Rect::from_min_max(
                pos2(voll.left(), voll.bottom() - fuss_h),
                voll.right_bottom(),
            );
            let mut fu = ui.new_child(
                UiBuilder::new()
                    .max_rect(fuss)
                    .layout(Layout::left_to_right(Align::Center)),
            );
            fu.painter().hline(
                fuss.x_range(),
                fuss.top() + MASSE.linie / 2.0,
                Stroke::new(MASSE.linie, r.linie),
            );
            fu.add_space(MASSE.kopf_einzug);
            if self.zeigt("B22") {
                let n = self
                    .zusatz
                    .arbeiter
                    .iter()
                    .filter(|a| a.zustand == "laeuft")
                    .count();
                let mut job = egui::text::LayoutJob::default();
                let f = FontId::proportional(MASSE.schrift_grund);
                job.append(
                    &n.to_string(),
                    0.0,
                    egui::TextFormat {
                        font_id: f.clone(),
                        color: r.schrift,
                        ..Default::default()
                    },
                );
                job.append(
                    " Worker laufen",
                    0.0,
                    egui::TextFormat {
                        font_id: f,
                        color: r.schrift,
                        ..Default::default()
                    },
                );
                let l = fu.label(job);
                ui.ctx().accesskit_node_builder(l.id, |nb| {
                    nb.set_label(format!("{n} Worker laufen"));
                    nb.set_author_id("B22".to_owned());
                });
            }
            fu.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.add_space(MASSE.innen);
                if self.zeigt("B23") {
                    let k = symbole::knopf(
                        ui,
                        Symbol::Zahnrad,
                        "Einstellungen",
                        MASSE.kapsel_segment,
                        false,
                    );
                    kennung(ui, k.id, "B23");
                    self.halt(LEISTE, &k);
                    if k.clicked() {
                        self.b.einstellungen = Some(Seite::Aussehen);
                    }
                }
            });
        });
    }

    fn sitzungsliste(&mut self, ui: &mut Ui, r: &Rollen, menue_taste: bool) {
        // group by project, in the order of the list; within a project in the remembered order (B19)
        let mut projekte: Vec<String> = Vec::new();
        for s in &self.kern.sitzungen {
            let p = self
                .zusatz
                .sitzung(&s.id)
                .map_or_else(String::new, |x| x.projekt.clone());
            if !projekte.contains(&p) {
                projekte.push(p);
            }
        }
        ui.scope_builder(UiBuilder::new().id_salt("sitzung-liste"), |ui| {
            behaelter(ui, Role::List, "Sitzungen", None);
            ui.ctx()
                .accesskit_node_builder(ui.unique_id(), |n| n.set_class_name("sitzung-liste"));
            for p in projekte {
                let ids: Vec<String> = self
                    .kern
                    .sitzungen
                    .iter()
                    .filter(|s| {
                        self.zusatz
                            .sitzung(&s.id)
                            .map_or("", |x| x.projekt.as_str())
                            == p
                    })
                    .map(|s| s.id.clone())
                    .collect();
                let geordnet = self.merken.ordnen(&p, &ids);
                if self.zeigt("B09") {
                    ui.add_space(MASSE.gruppenabstand);
                    let kopf = ui.allocate_ui_with_layout(
                        vec2(ui.available_width(), MASSE.projektzeile),
                        Layout::left_to_right(Align::Center),
                        |ui| {
                            ui.add_space(MASSE.kopf_einzug);
                            ui.label(
                                RichText::new(&p)
                                    .strong()
                                    .size(MASSE.schrift_klein)
                                    .color(r.gedaempft),
                            );
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                ui.add_space(MASSE.innen);
                                ui.label(
                                    RichText::new(ids.len().to_string())
                                        .strong()
                                        .size(MASSE.schrift_klein)
                                        .color(r.gedaempft),
                                );
                            });
                        },
                    );
                    ui.ctx().accesskit_node_builder(kopf.response.id, |n| {
                        n.set_role(Role::Heading);
                        n.set_label(format!("{p}, {} Sitzungen", ids.len()));
                        n.set_author_id("B09".to_owned());
                    });
                }
                for (pos, id) in geordnet.iter().enumerate() {
                    self.sitzungszeile(ui, r, &p, &ids, id, pos, menue_taste);
                }
            }
        });
    }

    #[allow(clippy::too_many_arguments)]
    fn sitzungszeile(
        &mut self,
        ui: &mut Ui,
        r: &Rollen,
        projekt: &str,
        ids: &[String],
        id: &str,
        pos: usize,
        menue_taste: bool,
    ) {
        let Some(s) = self.kern.sitzungen.iter().find(|x| x.id == id).cloned() else {
            return;
        };
        let name = self
            .zusatz
            .sitzung(id)
            .map_or_else(|| id.to_owned(), |x| x.name.clone());
        let worker = self.zusatz.arbeiter_von(id).len();
        let gewaehlt = self.kern.aktiv.as_deref() == Some(id);
        let zustand = if s.laeuft {
            "läuft".to_owned()
        } else if s.code == "0" {
            "beendet".to_owned()
        } else {
            format!("abgestürzt, Code {}", s.code)
        };
        let breite = ui.available_width();
        let (rect, resp) =
            ui.allocate_exact_size(vec2(breite, MASSE.sitzungszeile), Sense::click_and_drag());
        let innen = rect.shrink2(vec2(MASSE.innen, 0.5));
        let rad = gestalt::radius(MASSE.radius_wahl);
        if gewaehlt {
            ui.painter().rect_filled(innen, rad, r.wahl);
        } else if resp.hovered() {
            ui.painter()
                .rect_filled(innen, rad, r.wahl.gamma_multiply(0.5));
        }
        let farbe = if s.laeuft {
            r.laeuft
        } else if s.code == "0" {
            r.gedaempft
        } else {
            r.tot
        };
        let oben = innen.top() + (innen.height() - 31.0) / 2.0;
        let punkt_mitte = pos2(
            innen.left() + MASSE.zeile_punkt + MASSE.punkt / 2.0,
            oben + 8.0,
        );
        symbole::punkt(ui, punkt_mitte, farbe, s.laeuft);
        let x = innen.left() + MASSE.zeile_text;
        let breite = innen.right() - MASSE.innen - x;
        text_gekuerzt(
            ui,
            pos2(x, oben),
            &name,
            FontId::proportional(MASSE.schrift_grund),
            r.schrift,
            breite,
        );
        text_gekuerzt(
            ui,
            pos2(x, oben + 17.0),
            &if self.hat(1) {
                format!("{worker} Worker · {zustand}")
            } else {
                zustand.clone()
            },
            FontId::proportional(MASSE.schrift_klein),
            r.gedaempft,
            breite,
        );
        let zustand_aria = s.zustand_text();
        ui.ctx().accesskit_node_builder(resp.id, |n| {
            n.set_role(Role::Button);
            n.set_label(format!("{name}, {id}, {zustand_aria}"));
            n.set_author_id("B10".to_owned());
            n.set_class_name(format!("sitzung-{id}"));
            n.set_selected(gewaehlt);
        });
        self.halt(LEISTE, &resp);
        self.fokus_nehmen(&resp, &FokusZiel::Sitzung(id.to_owned()));
        if resp.clicked() {
            let w = self.kern.waehle(id, self.groesse);
            self.wirkungen(w);
            self.kachel_abo.clear();
        }
        if resp.secondary_clicked() || (menue_taste && resp.has_focus()) {
            self.b.menue = Some((
                id.to_owned(),
                resp.interact_pointer_pos().unwrap_or(rect.left_bottom()),
            ));
        }
        // B19: drag to reorder, remembered per device
        if resp.drag_started() {
            self.b.ziehen = Some(id.to_owned());
        }
        if resp.drag_stopped()
            && let Some(z) = self.b.ziehen.take()
            && let Some(py) = ui.ctx().pointer_interact_pos()
        {
            // the place under the pointer, in rows of this project
            #[allow(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                clippy::cast_precision_loss
            )]
            let ziel = ((py.y - rect.top()) / MASSE.sitzungszeile + pos as f32)
                .max(0.0)
                .round() as usize;
            self.merken.verschieben(projekt, ids, &z, ziel);
            self.merken_geaendert = true;
        }
    }

    // ------------------------------------------------------------------------------------------------ middle

    fn mitte(&mut self, ui: &mut Ui, r: &Rollen) {
        if self.getrennt && self.zeigt("B05") {
            let l = Frame::new()
                .fill(r.tot.gamma_multiply(0.2))
                .inner_margin(Margin::symmetric(12, 6))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.label(RichText::new("Verbindung zum Kern fehlt").color(r.schrift));
                });
            ui.ctx().accesskit_node_builder(l.response.id, |n| {
                n.set_role(Role::Alert);
                n.set_label("Verbindung zum Kern fehlt".to_owned());
                n.set_author_id("B05".to_owned());
            });
        }
        if let Some(a) = self.kern.aktiv.clone()
            && let Some(d) = self.kern.dialoge.iter().find(|d| d.sitzung == a).cloned()
            && self.zeigt("B42")
        {
            let leiste = Frame::new()
                .fill(r.wartet.gamma_multiply(0.18))
                .inner_margin(Margin::symmetric(12, 6))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Dialog offen").strong());
                        ui.label(RichText::new(&d.art).color(r.gedaempft));
                        for (i, o) in d.optionen.iter().enumerate() {
                            if ui.button(o).clicked() {
                                let w = self.kern.dialog_antwort(&d, i + 1);
                                self.wirkungen(w);
                            }
                        }
                    });
                });
            ui.ctx().accesskit_node_builder(leiste.response.id, |n| {
                n.set_role(Role::Group);
                n.set_label("Dialog offen".to_owned());
                n.set_author_id("B42".to_owned());
            });
        }
        if self.kern.aktiv.is_none() {
            if self.zeigt("B34") {
                ui.centered_and_justified(|ui| {
                    let l = ui.label(
                        RichText::new(
                            "Keine Sitzung gewählt. Mit „+“ in der Leiste startest du eine.",
                        )
                        .color(r.gedaempft),
                    );
                    kennung(ui, l.id, "B34");
                });
            }
        } else if self.b.mitte == Mitte::Worker && self.zeigt("B27") {
            self.worker(ui, r);
        } else {
            self.terminal(ui, r);
        }
        self.kurzmeldung(ui, r);
    }

    fn terminal(&mut self, ui: &mut Ui, r: &Rollen) {
        let tf = *self.wahl.terminal(self.system(ui));
        let schrift = FontId::monospace(self.terminalschrift);
        let (zw, zh) = terminal::zelle(ui, &schrift);
        let mut flaeche = ui.available_rect_before_wrap();
        // the head of the pane, as over every pane of the reference (B41)
        if self.zeigt("B41")
            && let Some(s) = self.kern.aktive_sitzung().cloned()
        {
            let kopf = Rect::from_min_size(flaeche.min, vec2(flaeche.width(), MASSE.pane_kopf));
            let zustand = if s.laeuft { "laeuft" } else { "beendet" };
            self.pane_kopf(ui, r, kopf, "Orchestrator", &s.modell, zustand, true, None);
            flaeche.min.y = kopf.bottom();
        }
        ui.painter().rect_filled(flaeche, 0.0, tf.grund);
        let groesse = ansicht::kern::klemme(
            f64::from(flaeche.width() - 12.0),
            f64::from(flaeche.height() - 12.0),
            f64::from(zw),
            f64::from(zh),
        );
        if groesse != self.groesse {
            self.groesse = groesse;
            let w = self.kern.groesse(groesse);
            self.wirkungen(w);
        }
        ui.scope_builder(UiBuilder::new().id_salt("terminal-bereich"), |ui| {
            if self.zeigt("B39") {
                // a group, not a generic container: AccessKit drops generic containers from the platform trees
                // (AT-SPI, UI Automation), and B39 vanished there (real start under Xvfb, 27.11.2026)
                behaelter(ui, Role::Group, "Terminalfläche", Some("B39"));
            }
            let id = self.terminal_id;
            let resp = ui.interact(flaeche, id, Sense::click_and_drag());
            self.halt(TERMINAL, &resp);
            self.fokus_nehmen(&resp, &FokusZiel::Terminal);
            if resp.clicked() || resp.drag_started() {
                resp.request_focus();
            }
            ui.memory_mut(|m| {
                m.set_focus_lock_filter(
                    id,
                    egui::EventFilter {
                        tab: true,
                        horizontal_arrows: true,
                        vertical_arrows: true,
                        escape: true,
                    },
                );
            });
            // mark with the mouse (B38)
            if resp.drag_started()
                && let Some(p) = resp.interact_pointer_pos()
            {
                let z = terminal::zelle_bei(p, flaeche, zw, zh);
                self.markierung = Some(Markierung { von: z, bis: z });
            } else if resp.dragged()
                && let (Some(p), Some(m)) = (resp.interact_pointer_pos(), self.markierung.as_mut())
            {
                m.bis = terminal::zelle_bei(p, flaeche, zw, zh);
            } else if resp.clicked() {
                self.markierung = None;
            }
            let sitzung = self.kern.aktiv.clone().unwrap_or_default();
            let beschreibung = Id::unique("terminal-beschreibung");
            ui.ctx().accesskit_node_builder(id, |n| {
                n.set_role(Role::MultilineTextInput);
                n.set_label(format!("Terminal {sitzung}"));
                n.set_role_description("Terminal".to_owned());
                n.set_author_id("B35".to_owned());
                n.set_class_name("terminal-raster");
                n.push_described_by(beschreibung.accesskit_id());
            });
            if resp.has_focus() {
                self.tasten(ui, &resp);
            }
            terminal::malen(
                ui,
                flaeche,
                self.kern.bild.as_ref(),
                &tf,
                &schrift,
                self.markierung.as_ref(),
                id,
                true,
            );
            // the description (SE2 point 4) and the keyboard (B36), copy and paste (B38)
            let br = Rect::from_min_size(flaeche.left_bottom(), Vec2::ZERO);
            let _ = ui.interact(br, beschreibung, Sense::hover());
            ui.ctx().accesskit_node_builder(beschreibung, |n| {
                n.set_role(Role::Label);
                n.set_label(
                    "Tasten gehen an das Programm. F6 und Umschalt+F6 wechseln den Bereich."
                        .to_owned(),
                );
                n.set_author_id("B36".to_owned());
            });
            let kopieren = Id::unique("terminal-kopieren");
            let _ = ui.interact(br, kopieren, Sense::hover());
            ui.ctx().accesskit_node_builder(kopieren, |n| {
                n.set_role(Role::Label);
                n.set_label("Kopieren mit Strg+Umschalt+C, Einfügen mit Strg+V".to_owned());
                n.set_author_id("B38".to_owned());
            });
            let _ = r;
        });
    }

    fn tasten(&mut self, ui: &Ui, resp: &Response) {
        let events = ui.input(|i| i.events.clone());
        for t in tasten::lesen(&events, cfg!(windows)) {
            match t {
                Tat::Deutung(Deutung::Fokus(richtung)) => self.bereich_wechseln(ui, richtung),
                Tat::Deutung(d) => {
                    let w = self.kern.taste(&d);
                    self.wirkungen(w);
                }
                Tat::Einfuegen(text) => {
                    let w = self.kern.einfuegen(&text);
                    self.wirkungen(w);
                }
                Tat::EinfuegenAnfragen => ui
                    .ctx()
                    .send_viewport_cmd(egui::ViewportCommand::RequestPaste),
                Tat::Kopieren => {
                    if let (Some(b), Some(m)) = (self.kern.bild.as_ref(), self.markierung.as_ref())
                    {
                        let t = terminal::markierter_text(b, m);
                        if !t.is_empty() {
                            ui.ctx().copy_text(t);
                        }
                    }
                }
            }
        }
        let _ = resp;
    }

    fn worker(&mut self, ui: &mut Ui, r: &Rollen) {
        let Some(sitzung) = self.kern.aktiv.clone() else {
            return;
        };
        let arbeiter: Vec<crate::ziel::Arbeiter> = self
            .zusatz
            .arbeiter_von(&sitzung)
            .into_iter()
            .cloned()
            .collect();
        let je_tab = MASSE.kacheln_je_tab;
        let viele = arbeiter.len() > je_tab;
        if viele && self.zeigt("B30") {
            ui.horizontal(|ui| {
                ui.scope_builder(UiBuilder::new().id_salt("worker-tabs"), |ui| {
                    behaelter(ui, Role::TabList, "Worker-Tabs", Some("B30"));
                    ui.horizontal(|ui| {
                        for t in 0..arbeiter.len().div_ceil(je_tab) {
                            let von = t * je_tab + 1;
                            let bis = ((t + 1) * je_tab).min(arbeiter.len());
                            let k = ui.selectable_label(self.b.tab == t, format!("{von}–{bis}"));
                            rolle_setzen(ui, k.id, Role::Tab);
                            if k.clicked() {
                                self.b.tab = t;
                                self.kachel_abo.clear();
                            }
                        }
                    });
                });
                ui.separator();
                ui.scope_builder(UiBuilder::new().id_salt("kachelform"), |ui| {
                    behaelter(ui, Role::RadioGroup, "Kachelform", Some("B31"));
                    ui.horizontal(|ui| {
                        for (f, t) in [
                            (Kachelform::Uebersicht, "Übersicht"),
                            (Kachelform::Gekachelt, "Gekachelt"),
                            (Kachelform::Gezoomt, "Gezoomt"),
                        ] {
                            if ui.selectable_label(self.b.kachelform == f, t).clicked() {
                                self.b.kachelform = f;
                                self.kachel_abo.clear();
                            }
                        }
                    });
                });
                let k = ui.button("Ordnen");
                kennung(ui, k.id, "B32");
                if k.clicked() {
                    self.b.tab = 0;
                    self.b.zoom = None;
                    self.kachel_abo.clear();
                }
                let h = ui.label(
                    RichText::new(format!("{je_tab} je Tab"))
                        .size(MASSE.schrift_klein)
                        .color(r.gedaempft),
                );
                kennung(ui, h.id, "B33");
            });
        }
        let sichtbar: Vec<crate::ziel::Arbeiter> = match (self.b.kachelform, &self.b.zoom) {
            (Kachelform::Gezoomt, _) | (_, Some(_)) => {
                let z = self
                    .b
                    .zoom
                    .clone()
                    .or_else(|| arbeiter.first().map(|a| a.id.clone()));
                arbeiter
                    .iter()
                    .filter(|a| Some(&a.id) == z.as_ref())
                    .cloned()
                    .collect()
            }
            (Kachelform::Uebersicht, None) => arbeiter.clone(),
            (Kachelform::Gekachelt, None) => arbeiter
                .iter()
                .skip(self.b.tab * je_tab)
                .take(je_tab)
                .cloned()
                .collect(),
        };
        if sichtbar.is_empty() {
            ui.centered_and_justified(|ui| {
                ui.label(RichText::new("Diese Sitzung hat keine Worker.").color(r.gedaempft))
            });
            return;
        }
        let n = sichtbar.len();
        let spalten = if n <= 1 {
            1
        } else if n <= 4 {
            2
        } else {
            3
        };
        let reihen = n.div_ceil(spalten);
        let flaeche = ui.available_rect_before_wrap();
        #[allow(clippy::cast_precision_loss)]
        let (kw, kh) = (
            flaeche.width() / spalten as f32,
            flaeche.height() / reihen as f32,
        );
        let tf = *self.wahl.terminal(self.system(ui));
        let schrift = FontId::monospace(self.terminalschrift - 1.0);
        let (zw, zh) = terminal::zelle(ui, &schrift);
        // the tile with the focus carries the frame; without a focused tile it is the first, as in the reference
        let aktiv = sichtbar
            .iter()
            .position(|a| {
                a.nr.is_some_and(|nr| ui.memory(|m| m.has_focus(Id::unique(("kachel-raster", nr)))))
            })
            .unwrap_or(0);
        for (i, a) in sichtbar.iter().enumerate() {
            #[allow(clippy::cast_precision_loss)]
            let rect = Rect::from_min_size(
                pos2(
                    flaeche.left() + (i % spalten) as f32 * kw,
                    flaeche.top() + (i / spalten) as f32 * kh,
                ),
                vec2(kw, kh),
            )
            .shrink(MASSE.kachelabstand / 2.0);
            self.kachel(
                ui,
                r,
                &tf,
                &schrift,
                (zw, zh),
                rect,
                a,
                &sitzung,
                i == aktiv,
            );
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn kachel(
        &mut self,
        ui: &mut Ui,
        r: &Rollen,
        tf: &gestalt::Terminalfarben,
        schrift: &FontId,
        zelle: (f32, f32),
        rect: Rect,
        a: &crate::ziel::Arbeiter,
        sitzung: &str,
        aktiv: bool,
    ) {
        let kopf = Rect::from_min_size(rect.min, vec2(rect.width(), MASSE.pane_kopf));
        let bild_rect = Rect::from_min_max(pos2(rect.left(), kopf.bottom()), rect.max);
        ui.painter().rect_filled(bild_rect, 0.0, tf.grund);
        if self.zeigt("B41") {
            self.pane_kopf(
                ui,
                r,
                kopf,
                &a.rolle,
                &a.modell,
                &a.zustand,
                aktiv,
                Some(&a.id),
            );
        }
        if aktiv {
            ui.painter().rect_stroke(
                rect,
                0.0,
                Stroke::new(MASSE.fokusrahmen, r.akzent),
                StrokeKind::Inside,
            );
        }
        let Some(nr) = a.nr else {
            return;
        };
        let groesse = ansicht::kern::klemme(
            f64::from(bild_rect.width() - 12.0),
            f64::from(bild_rect.height() - 12.0),
            f64::from(zelle.0),
            f64::from(zelle.1),
        );
        if !self.kachel_abo.contains(&(nr, groesse)) {
            self.kachel_abo.retain(|(n, _)| *n != nr);
            self.kachel_abo.push((nr, groesse));
            // L2: one subscription per raster, each with its own size
            self.senden(&json!({"art": "Abo", "sitzung": sitzung, "nr": nr, "an": true, "spalten": groesse.0, "zeilen": groesse.1}));
        }
        let bild = self.kacheln.abbild(nr).cloned();
        let id = Id::unique(("kachel-raster", nr));
        let resp = ui.interact(bild_rect, id, Sense::click());
        ui.ctx().accesskit_node_builder(id, |n| {
            n.set_role(Role::MultilineTextInput);
            n.set_label(format!("Terminal {} {}", sitzung, a.rolle));
            n.set_role_description("Terminal".to_owned());
        });
        self.halt(TERMINAL, &resp);
        terminal::malen(ui, bild_rect, bild.as_ref(), tf, schrift, None, id, true);
    }

    /// The head of a pane (B41): state dot, role, model; on a worker tile the button to enlarge it.
    #[allow(clippy::too_many_arguments)]
    fn pane_kopf(
        &mut self,
        ui: &mut Ui,
        r: &Rollen,
        kopf: Rect,
        rolle: &str,
        modell: &str,
        zustand: &str,
        aktiv: bool,
        zoom: Option<&str>,
    ) {
        let mut ku = ui.new_child(
            UiBuilder::new()
                .max_rect(kopf)
                .layout(Layout::left_to_right(Align::Center))
                .id_salt(("kachel", rolle, zoom.unwrap_or("orchestrator"))),
        );
        ku.painter()
            .rect_filled(kopf, 0.0, if aktiv { r.kopf_aktiv } else { r.kopf });
        ku.set_clip_rect(kopf);
        ku.scope_builder(UiBuilder::new().id_salt("kopf"), |ui| {
            behaelter(
                ui,
                Role::Group,
                &format!("{rolle}, {modell}, {zustand}"),
                Some("B41"),
            );
            ui.spacing_mut().item_spacing.x = MASSE.abstand + 2.0;
            ui.add_space(MASSE.innen);
            punkt(ui, zustand_farbe(r, zustand), zustand == "laeuft");
            ui.label(
                RichText::new(rolle)
                    .color(if aktiv { r.akzent_text } else { r.schrift })
                    .size(MASSE.schrift_grund),
            );
            ui.add(
                egui::Label::new(
                    RichText::new(modell)
                        .monospace()
                        .color(r.gedaempft)
                        .size(MASSE.schrift_klein),
                )
                .truncate(),
            );
            if let Some(id) = zoom {
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.add_space(MASSE.abstand);
                    let z = symbole::knopf(
                        ui,
                        Symbol::Vergroessern,
                        &format!("{rolle} vergrößern"),
                        MASSE.pane_kopf - 4.0,
                        false,
                    );
                    if z.clicked() {
                        self.b.zoom = if self.b.zoom.as_deref() == Some(id) {
                            None
                        } else {
                            Some(id.to_owned())
                        };
                        self.kachel_abo.clear();
                    }
                });
            }
        });
    }

    fn kurzmeldung(&self, ui: &Ui, r: &Rollen) {
        let Some((t, _)) = &self.kurz else {
            return;
        };
        if !self.zeigt("B67") {
            return;
        }
        let mitte = ui.max_rect();
        let ctx = ui.ctx();
        let antwort = egui::Area::new(Id::unique("kurzmeldung"))
            .order(egui::Order::Foreground)
            .pivot(Align2::CENTER_BOTTOM)
            .fixed_pos(pos2(mitte.center().x, mitte.bottom() - 24.0))
            .interactable(false)
            .show(ctx, |ui| {
                Frame::new()
                    .fill(r.erhoben)
                    .stroke(Stroke::new(1.0, r.linie))
                    .corner_radius(CornerRadius::same(8))
                    .inner_margin(Margin::symmetric(14, 8))
                    .show(ui, |ui| ui.label(t.as_str()))
                    .inner
            });
        kennung(ui, antwort.inner.id, "B67");
    }

    // ------------------------------------------------------------------------------------------------ inspector

    fn inspektor(&mut self, ui: &mut Ui, r: &Rollen) {
        let voll = ui.max_rect();
        let kopf = Rect::from_min_size(voll.min, vec2(voll.width(), MASSE.inspektor_kopf));
        ui.painter().rect_filled(kopf, 0.0, r.inspektor_kopf);
        ui.painter().hline(
            kopf.x_range(),
            kopf.bottom() + MASSE.linie / 2.0,
            Stroke::new(MASSE.linie, r.linie),
        );
        let reiter: Vec<(Reiter, Symbol, &str, &str)> = [
            (Reiter::Freigaben, Symbol::Schild, "Freigaben", "B56"),
            (Reiter::Ordner, Symbol::Ordner, "Ordner", "B52"),
        ]
        .into_iter()
        .filter(|x| self.zeigt(x.3))
        .collect();
        // the tabs stand together in the middle of the row, as in the reference
        #[allow(clippy::cast_precision_loss)]
        let gruppe = Rect::from_center_size(
            kopf.center(),
            vec2(reiter.len() as f32 * MASSE.reiter.0, MASSE.reiter.1),
        );
        ui.painter()
            .rect_filled(gruppe, gestalt::radius(MASSE.radius_reiter + 1.0), r.wahl);
        let mut ru = ui.new_child(
            UiBuilder::new()
                .max_rect(gruppe)
                .layout(Layout::left_to_right(Align::Center)),
        );
        ru.scope_builder(UiBuilder::new().id_salt("reiter"), |ui| {
            behaelter(ui, Role::TabList, "Reiter des Inspektors", Some("B51"));
            ui.spacing_mut().item_spacing.x = 0.0;
            ui.visuals_mut().selection.bg_fill = r.reiter_wahl;
            ui.visuals_mut().widgets.hovered.weak_bg_fill = r.reiter_wahl.gamma_multiply(0.6);
            for (reiter, symbol, name, _) in &reiter {
                let k = symbole::knopf_form(
                    ui,
                    *symbol,
                    name,
                    vec2(MASSE.reiter.0, MASSE.reiter.1),
                    MASSE.radius_reiter,
                    self.b.reiter == *reiter,
                );
                rolle_setzen(ui, k.id, Role::Tab);
                self.halt(INSPEKTOR, &k);
                if k.clicked() {
                    self.b.reiter = *reiter;
                }
            }
        });
        if !self.zeigt("B56") {
            self.b.reiter = Reiter::Ordner;
        }
        let inhalt = Rect::from_min_max(
            pos2(
                voll.left() + MASSE.innen,
                kopf.bottom() + MASSE.linie + MASSE.innen,
            ),
            pos2(voll.right() - MASSE.innen, voll.bottom() - MASSE.innen),
        );
        let mut cu = ui.new_child(
            UiBuilder::new()
                .max_rect(inhalt)
                .layout(Layout::top_down(Align::Min))
                .id_salt("inspektor-blatt"),
        );
        match self.b.reiter {
            Reiter::Freigaben if self.zeigt("B56") => self.freigaben(&mut cu, r),
            Reiter::Ordner if self.zeigt("B52") => self.ordner(&mut cu, r),
            _ => {}
        }
    }

    /// A heading of the inspector with its number, as in the reference ("Anträge  0").
    fn ueberschrift(ui: &mut Ui, r: &Rollen, text: &str, n: usize) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = MASSE.abstand + 2.0;
            ui.label(
                RichText::new(text)
                    .strong()
                    .size(MASSE.schrift_grund)
                    .color(r.schrift),
            );
            ui.label(
                RichText::new(n.to_string())
                    .size(MASSE.schrift_grund)
                    .color(r.gedaempft),
            );
        });
    }

    fn freigaben(&mut self, ui: &mut Ui, r: &Rollen) {
        ui.scope_builder(UiBuilder::new().id_salt("freigaben-blatt"), |ui| {
            behaelter(ui, Role::TabPanel, "Freigaben", Some("B56"));
            ui.ctx().accesskit_node_builder(ui.unique_id(), |n| n.set_class_name("freigaben-blatt"));
            egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
                let anfragen = self.kern.anfragen.clone();
                Self::ueberschrift(ui, r, "Anträge", anfragen.len());
                if anfragen.is_empty() {
                    ui.label(RichText::new("Kein offener Antrag.").color(r.gedaempft));
                }
                for a in &anfragen {
                    let worker = self
                        .zusatz
                        .anfrage_worker
                        .iter()
                        .find(|(i, _)| *i == a.id)
                        .map(|(_, w)| w.clone())
                        .unwrap_or_default();
                    // flat like the rest of the inspector: no card, a line below each request (OF2 point 1)
                    ui.add_space(MASSE.abstand);
                    ui.label(RichText::new(&a.werkzeug).strong());
                    ui.add(egui::Label::new(RichText::new(&a.vorschau).monospace().size(MASSE.schrift_klein)).truncate());
                    // the worker of a request is a field of L1; the base form names the session only (OF8 point 3)
                    let herkunft = if worker.is_empty() { a.sitzung.clone() } else { format!("{} · {}", a.sitzung, worker) };
                    ui.label(RichText::new(herkunft).size(MASSE.schrift_klein).color(r.gedaempft));
                    ui.horizontal(|ui| {
                        if ui.button("Freigeben").clicked() {
                            self.senden(&json!({"art": "Freigabe", "id": a.id, "erteilt": true}));
                        }
                        if ui.button("Ablehnen").clicked() {
                            let b = std::mem::take(&mut self.b.begruendung);
                            self.senden(&json!({"art": "Freigabe", "id": a.id, "erteilt": false, "begruendung": b}));
                        }
                        if self.zeigt("B27") && !worker.is_empty() && ui.button("Zum Worker").clicked() {
                            self.b.mitte = Mitte::Worker;
                            self.b.zoom = Some(worker.clone());
                        }
                    });
                    let f = ui.add(
                        egui::TextEdit::singleline(&mut self.b.begruendung)
                            .hint_text("Begründung")
                            .desired_width(f32::INFINITY)
                            .min_size(vec2(0.0, MASSE.feld)),
                    );
                    name_setzen(ui, f.id, "Begründung");
                    ui.add_space(MASSE.abstand);
                    let y = ui.cursor().top();
                    ui.painter().hline(ui.max_rect().x_range(), y, Stroke::new(MASSE.linie, r.linie));
                    ui.add_space(MASSE.abstand);
                }
                // the workers stopped by the guard need L1, the lines of the history L6 (B07)
                if self.hat(1) {
                    let angehalten: Vec<crate::ziel::Arbeiter> =
                        self.zusatz.arbeiter.iter().filter(|w| w.zustand == "wartet").cloned().collect();
                    ui.add_space(MASSE.innen);
                    Self::ueberschrift(ui, r, "Angehaltene Worker", angehalten.len());
                    if angehalten.is_empty() {
                        ui.label(RichText::new("Kein Worker wartet gerade auf eine Guard-Entscheidung.").color(r.gedaempft));
                    }
                    for w in &angehalten {
                        ui.horizontal(|ui| {
                            punkt(ui, r.wartet, true);
                            ui.label(format!("{} · {}", w.rolle, w.modell));
                        });
                    }
                }
                if !self.zeigt("B07") {
                    return;
                }
                ui.add_space(MASSE.innen);
                Self::ueberschrift(ui, r, "Guard-Verlauf", self.zusatz.verlauf.len());
                if self.zusatz.verlauf.is_empty() {
                    ui.label(RichText::new("Noch keine Ablehnung aufgezeichnet.").color(r.gedaempft));
                }
                for v in self.zusatz.verlauf.clone() {
                    let text = if v.erteilt {
                        format!("Freigabe erteilt: {}", v.text)
                    } else if v.begruendung.is_empty() {
                        format!("Freigabe abgelehnt: {}", v.text)
                    } else {
                        format!("Freigabe abgelehnt: {} ({})", v.text, v.begruendung)
                    };
                    // the state shows as a dot; the text keeps the text colour (OF3 point 5)
                    ui.horizontal_top(|ui| {
                        ui.add_space(1.0);
                        ui.vertical(|ui| {
                            ui.add_space(4.0);
                            punkt(ui, if v.erteilt { r.fertig } else { r.tot }, true);
                        });
                        let l = ui.add(egui::Label::new(RichText::new(&text).size(MASSE.schrift_klein)).wrap());
                        kennung(ui, l.id, "B07");
                    });
                }
            });
        });
    }

    fn ordner(&mut self, ui: &mut Ui, r: &Rollen) {
        let Some(sitzung) = self.kern.aktiv.clone() else {
            ui.label(RichText::new("Keine Sitzung gewählt").color(r.gedaempft));
            return;
        };
        if self.b.ordner_angefragt.as_ref() != Some(&sitzung) {
            self.b.ordner_angefragt = Some(sitzung.clone());
            self.senden(&json!({"art": "OrdnerLesen", "sitzung": sitzung, "pfad": ""}));
        }
        ui.scope_builder(UiBuilder::new().id_salt("ordner-blatt"), |ui| {
            behaelter(ui, Role::TabPanel, "Ordner", Some("B52"));
            ui.ctx()
                .accesskit_node_builder(ui.unique_id(), |n| n.set_class_name("ordner-blatt"));
            let pfad = self
                .zusatz
                .ordner
                .as_ref()
                .map(|(p, _)| p.clone())
                .unwrap_or_default();
            let font = FontId::proportional(MASSE.schrift_grund);
            let kurz = vorn_gekuerzt(ui, &pfad, &font, ui.available_width());
            ui.label(RichText::new(kurz).font(font).color(r.gedaempft));
            ui.add_space(MASSE.abstand);
            if self.zeigt("B53") {
                let f = ui.add(
                    egui::TextEdit::singleline(&mut self.b.suche)
                        .hint_text("Im Inhalt suchen")
                        .desired_width(f32::INFINITY)
                        .min_size(vec2(0.0, MASSE.feld))
                        .align(Align2::LEFT_CENTER),
                );
                ui.ctx().accesskit_node_builder(f.id, |n| {
                    n.set_role(Role::SearchInput);
                    n.set_label("Im Inhalt suchen".to_owned());
                    n.set_author_id("B53".to_owned());
                });
                self.halt(INSPEKTOR, &f);
                if f.lost_focus()
                    && ui.input(|i| i.key_pressed(Key::Enter))
                    && !self.b.suche.is_empty()
                {
                    let m = self.b.suche.clone();
                    self.senden(&json!({"art": "InhaltSuchen", "sitzung": sitzung, "muster": m}));
                }
            }
            ui.add_space(6.0);
            if !self.zusatz.treffer.is_empty() && !self.b.suche.is_empty() {
                for (p, z, t) in &self.zusatz.treffer {
                    ui.label(
                        RichText::new(format!("{p}:{z}"))
                            .size(MASSE.schrift_klein)
                            .color(r.schrift)
                            .underline(),
                    );
                    ui.label(RichText::new(t).monospace().size(MASSE.schrift_klein));
                }
                ui.separator();
            }
            if let Some((_, eintraege)) = &self.zusatz.ordner {
                for (name, ist_ordner) in eintraege {
                    ui.horizontal(|ui| {
                        // the fonts of egui have no folder sign; the symbol is painted
                        let (f, _) = ui.allocate_exact_size(Vec2::splat(14.0), Sense::hover());
                        if *ist_ordner {
                            symbole::malen(ui, Symbol::Ordner, f, r.gedaempft);
                        }
                        ui.label(name);
                    });
                }
            }
        });
    }

    // ------------------------------------------------------------------------------------------------ bubbles, windows

    fn blasen(&mut self, ui: &mut Ui, r: &Rollen) {
        let ctx = ui.ctx().clone();
        if self.b.worker_liste && self.zeigt("B29") {
            let unter = ctx
                .data(|d| d.get_temp::<Rect>(Id::unique("pille-rect")))
                .map_or(pos2(900.0, MASSE.titelleiste), |x| x.left_bottom());
            let sitzung = self.kern.aktiv.clone().unwrap_or_default();
            let arbeiter: Vec<crate::ziel::Arbeiter> = self
                .zusatz
                .arbeiter_von(&sitzung)
                .into_iter()
                .cloned()
                .collect();
            let a = egui::Area::new(Id::unique("worker-liste"))
                .order(egui::Order::Foreground)
                .fixed_pos(unter + vec2(-120.0, 6.0))
                .show(&ctx, |ui| {
                    Frame::popup(ui.style())
                        .fill(r.erhoben)
                        .inner_margin(Margin::same(10))
                        .show(ui, |ui| {
                            ui.set_width(300.0);
                            ui.scope_builder(
                                UiBuilder::new().id_salt("worker-liste-inhalt"),
                                |ui| {
                                    behaelter(ui, Role::Dialog, "Worker-Liste", Some("B29"));
                                    ui.ctx().accesskit_node_builder(ui.unique_id(), |n| {
                                        n.set_class_name("worker-liste");
                                    });
                                    ui.label(
                                        RichText::new(format!("Worker  {}", arbeiter.len()))
                                            .strong(),
                                    );
                                    for w in &arbeiter {
                                        ui.horizontal(|ui| {
                                            punkt(
                                                ui,
                                                zustand_farbe(r, &w.zustand),
                                                w.zustand == "laeuft",
                                            );
                                            ui.label(&w.rolle);
                                            ui.label(
                                                RichText::new(&w.modell)
                                                    .color(r.gedaempft)
                                                    .size(MASSE.schrift_klein),
                                            );
                                            ui.with_layout(
                                                Layout::right_to_left(Align::Center),
                                                |ui| {
                                                    ui.label(
                                                        RichText::new(zustand_wort(&w.zustand))
                                                            .color(r.gedaempft)
                                                            .size(MASSE.schrift_klein),
                                                    );
                                                },
                                            );
                                        });
                                    }
                                    if self.zeigt("B76") {
                                        let h = ui.label(
                                            RichText::new("Escape schließt")
                                                .size(MASSE.schrift_klein)
                                                .color(r.gedaempft),
                                        );
                                        kennung(ui, h.id, "B76");
                                    }
                                },
                            );
                        });
                });
            if ctx.input(|i| i.pointer.any_click())
                && !a.response.contains_pointer()
                && !ctx
                    .data(|d| d.get_temp::<Rect>(Id::unique("pille-rect")))
                    .is_some_and(|p| ctx.pointer_interact_pos().is_some_and(|q| p.contains(q)))
            {
                self.b.worker_liste = false;
            }
        }
        if let Some((id, pos)) = self.b.menue.clone() {
            let Some(s) = self.kern.sitzungen.iter().find(|x| x.id == id).cloned() else {
                self.b.menue = None;
                return;
            };
            let mut zu = false;
            let a = egui::Area::new(Id::unique("sitzungsmenue"))
                .order(egui::Order::Foreground)
                .fixed_pos(pos)
                .show(&ctx, |ui| {
                    Frame::menu(ui.style()).fill(r.erhoben).show(ui, |ui| {
                        ui.set_width(220.0);
                        ui.scope_builder(UiBuilder::new().id_salt("menue-inhalt"), |ui| {
                            behaelter(ui, Role::Menu, "Sitzung", None);
                            let eintrag = |ui: &mut Ui,
                                           text: &str,
                                           bnn: &str,
                                           an: bool,
                                           grund: &str|
                             -> bool {
                                let k = ui.add_enabled(
                                    an,
                                    egui::Button::new(text)
                                        .frame(false)
                                        .min_size(vec2(200.0, 24.0)),
                                );
                                let k = if an || grund.is_empty() {
                                    k
                                } else {
                                    k.on_disabled_hover_text(grund)
                                };
                                ui.ctx().accesskit_node_builder(k.id, |n| {
                                    n.set_role(Role::MenuItem);
                                    n.set_author_id(bnn.to_owned());
                                    if !an && !grund.is_empty() {
                                        n.set_description(grund.to_owned());
                                    }
                                });
                                k.clicked()
                            };
                            let beendet = !s.laeuft;
                            if beendet
                                && self.zeigt("B11")
                                && eintrag(ui, "Fortsetzen", "B11", true, "")
                            {
                                self.senden(&json!({"art": "Fortsetzen", "sitzung": id}));
                                zu = true;
                            }
                            if self.zeigt("B12") && eintrag(ui, "Namen ändern …", "B12", true, "")
                            {
                                let alt = self
                                    .zusatz
                                    .sitzung(&id)
                                    .map(|x| x.name.clone())
                                    .unwrap_or_default();
                                self.b.rueckfrage = Some(Rueckfrage::Umbenennen(id.clone(), alt));
                                zu = true;
                            }
                            if self.zeigt("B15")
                                && eintrag(
                                    ui,
                                    "Sitzung schließen",
                                    "B15",
                                    !beendet,
                                    "Sitzung ist beendet",
                                )
                            {
                                let w = self.kern.beenden(&id);
                                self.wirkungen(w);
                                zu = true;
                            }
                            if beendet
                                && s.code != "0"
                                && self.zeigt("B17")
                                && eintrag(ui, "Wiederbeleben …", "B17", true, "")
                            {
                                self.b.rueckfrage = Some(Rueckfrage::Wiederbeleben(id.clone()));
                                zu = true;
                            }
                            if beendet
                                && self.zeigt("B16")
                                && eintrag(ui, "Endgültig löschen …", "B16", true, "")
                            {
                                self.b.rueckfrage = Some(Rueckfrage::Loeschen(id.clone()));
                                zu = true;
                            }
                            if self.zeigt("B19") {
                                let h = ui.label(
                                    RichText::new("Ziehen ordnet die Sitzungen")
                                        .size(MASSE.schrift_klein)
                                        .color(r.gedaempft),
                                );
                                kennung(ui, h.id, "B19");
                            }
                            if self.zeigt("B76") {
                                let h = ui.label(
                                    RichText::new("Escape schließt")
                                        .size(MASSE.schrift_klein)
                                        .color(r.gedaempft),
                                );
                                kennung(ui, h.id, "B76");
                            }
                        });
                    });
                });
            if zu
                || (ctx.input(|i| i.pointer.any_click())
                    && !a.response.contains_pointer()
                    && !ctx.input(|i| i.pointer.secondary_clicked()))
            {
                self.b.menue = None;
            }
        }
    }

    fn sitzungen_fenster(&mut self, ui: &mut Ui, r: &Rollen) {
        let Some(mut f) = self.b.sitzungen.clone() else {
            return;
        };
        if !self.zeigt("B68") {
            self.b.sitzungen = None;
            return;
        }
        let ctx = ui.ctx().clone();
        let mut schliessen = false;
        let antwort = egui::Modal::new(Id::unique("sitzungen-fenster"))
            .frame(
                Frame::window(&ctx.global_style())
                    .fill(r.grund)
                    .corner_radius(gestalt::radius(MASSE.radius_fenster))
                    .inner_margin(Margin::same(16)),
            )
            .show(&ctx, |ui| {
                ui.set_width(MASSE.sitzungsfenster.0 - 32.0);
                ui.set_min_height(MASSE.sitzungsfenster.1 - 32.0);
                ui.scope_builder(UiBuilder::new().id_salt("sitzungen-inhalt"), |ui| {
                    behaelter(ui, Role::Dialog, "Sitzungen", Some("B68"));
                    ui.ctx().accesskit_node_builder(ui.unique_id(), |n| {
                        n.set_modal();
                        n.set_class_name("sitzungen-fenster");
                    });
                    ui.label(
                        RichText::new("Sitzungen")
                            .size(MASSE.schrift_titel)
                            .color(r.gedaempft),
                    );
                    ui.add_space(MASSE.innen);
                    ui.columns(2, |sp| {
                        for (i, s) in sp.iter_mut().enumerate() {
                            Frame::new()
                                .fill(r.erhoben)
                                .corner_radius(gestalt::radius(MASSE.radius_gruppe))
                                .inner_margin(Margin::same(10))
                                .show(s, |ui| {
                                    ui.set_width(ui.available_width());
                                    ui.set_min_height(MASSE.sitzungsfenster.1 - 150.0);
                                    if i == 0 {
                                        self.bekannte_sitzungen(ui, r, &mut f);
                                    } else {
                                        self.startformular(ui, r, &mut f);
                                    }
                                });
                        }
                    });
                    ui.add_space(MASSE.innen);
                    let y = ui.cursor().top();
                    ui.painter().hline(
                        ui.max_rect().x_range(),
                        y,
                        Stroke::new(MASSE.linie, r.linie),
                    );
                    ui.add_space(MASSE.innen);
                    ui.horizontal(|ui| {
                        let gew = f.gewaehlt.clone();
                        let s = gew
                            .as_ref()
                            .and_then(|g| self.kern.sitzungen.iter().find(|x| &x.id == g).cloned());
                        let laeuft = s.as_ref().is_some_and(|x| x.laeuft);
                        let k = ui.add_enabled(laeuft, egui::Button::new("Beenden"));
                        ui.ctx()
                            .accesskit_node_builder(k.id, |n| n.set_class_name("sitzung-beenden"));
                        self.modal_halte_neu.push(k.id);
                        if k.clicked()
                            && let Some(g) = &gew
                        {
                            let w = self.kern.beenden(g);
                            self.wirkungen(w);
                        }
                        if self.zeigt("B11") {
                            let k = ui.add_enabled(
                                s.as_ref().is_some_and(|x| !x.laeuft),
                                egui::Button::new("Fortsetzen"),
                            );
                            ui.ctx().accesskit_node_builder(k.id, |n| {
                                n.set_class_name("sitzung-fortsetzen");
                            });
                            self.modal_halte_neu.push(k.id);
                            if k.clicked()
                                && let Some(g) = &gew
                            {
                                self.senden(&json!({"art": "Fortsetzen", "sitzung": g}));
                            }
                        }
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            let k = ui.button("Schließen");
                            ui.ctx().accesskit_node_builder(k.id, |n| {
                                n.set_class_name("sitzungen-schliessen");
                            });
                            self.modal_halte_neu.push(k.id);
                            if k.clicked() {
                                schliessen = true;
                            }
                        });
                    });
                    if self.zeigt("B76") {
                        let h = ui.label(
                            RichText::new("Escape schließt das Fenster")
                                .size(MASSE.schrift_klein)
                                .color(r.gedaempft),
                        );
                        kennung(ui, h.id, "B76");
                    }
                });
            });
        if antwort.should_close() || schliessen {
            self.b.sitzungen = None;
            self.fokus = Some(FokusZiel::Plus);
        } else if self.b.sitzungen.is_some() {
            self.b.sitzungen = Some(f);
        }
    }

    fn bekannte_sitzungen(&mut self, ui: &mut Ui, r: &Rollen, f: &mut Sitzungsfenster) {
        ui.label(RichText::new("Bekannte Sitzungen").strong());
        ui.scope_builder(UiBuilder::new().id_salt("bekannte"), |ui| {
            behaelter(ui, Role::ListBox, "Bekannte Sitzungen", None);
            let mut ordner: Vec<String> = Vec::new();
            for s in &self.kern.sitzungen {
                let o = self
                    .zusatz
                    .sitzung(&s.id)
                    .map(|x| x.ordner.clone())
                    .unwrap_or_default();
                if !ordner.contains(&o) {
                    ordner.push(o);
                }
            }
            for o in ordner {
                // the folder is a field of L5; without it the sessions stand under no heading
                if !o.is_empty() {
                    ui.label(
                        RichText::new(&o)
                            .size(MASSE.schrift_klein)
                            .color(r.gedaempft),
                    );
                }
                for s in self.kern.sitzungen.clone() {
                    if self
                        .zusatz
                        .sitzung(&s.id)
                        .map(|x| x.ordner.clone())
                        .unwrap_or_default()
                        != o
                    {
                        continue;
                    }
                    let name = self
                        .zusatz
                        .sitzung(&s.id)
                        .map_or_else(|| s.id.clone(), |x| x.name.clone());
                    let text = format!("{}  ·  {}", name, s.zustand_text());
                    let k = ui.selectable_label(f.gewaehlt.as_ref() == Some(&s.id), text);
                    rolle_setzen(ui, k.id, Role::ListBoxOption);
                    self.modal_halte_neu.push(k.id);
                    if k.clicked() {
                        f.gewaehlt = Some(s.id.clone());
                    }
                }
            }
        });
    }

    fn startformular(&mut self, ui: &mut Ui, r: &Rollen, f: &mut Sitzungsfenster) {
        ui.label(RichText::new("Sitzung starten").strong());
        ui.scope_builder(UiBuilder::new().id_salt("startformular"), |ui| {
            behaelter(ui, Role::Form, "Sitzung starten", None);
            let feld = |ui: &mut Ui, name: &str, text: &mut String, kennung: &str| -> Response {
                ui.label(
                    RichText::new(name)
                        .size(MASSE.schrift_klein)
                        .color(r.gedaempft),
                );
                let e = ui.add(egui::TextEdit::singleline(text).desired_width(f32::INFINITY));
                ui.ctx().accesskit_node_builder(e.id, |n| {
                    n.set_label(name.to_owned());
                    n.set_author_id(kennung.to_owned());
                });
                e
            };
            // the name of a session is a field of L10; without it the folder is the first field
            let mit_name = self.hat(10);
            if mit_name {
                let e = feld(ui, "Name", &mut f.name, "name-eingabe");
                self.modal_halte_neu.push(e.id);
                self.fokus_nehmen(&e, &FokusZiel::Name);
            }
            let mut ordner = self.kern.start.arbeitsordner.clone();
            let e = feld(ui, "Arbeitsordner", &mut ordner, "arbeitsordner-eingabe");
            self.modal_halte_neu.push(e.id);
            if !mit_name {
                self.fokus_nehmen(&e, &FokusZiel::Name);
            }
            self.kern.start.arbeitsordner = ordner;
            let wahl = |ui: &mut Ui,
                        name: &str,
                        feld: &ansicht::start::Wahlfeld,
                        kennung: &str|
             -> (Option<String>, Id) {
                ui.label(
                    RichText::new(name)
                        .size(MASSE.schrift_klein)
                        .color(r.gedaempft),
                );
                let mut neu = None;
                let aktuell = feld
                    .eintraege
                    .iter()
                    .find(|e| e.wert == feld.wert)
                    .map_or(feld.wert.clone(), |e| e.text.clone());
                let cb = egui::ComboBox::from_id_salt(kennung)
                    .selected_text(aktuell)
                    .width(ui.available_width() - 8.0)
                    .show_ui(ui, |ui| {
                        for e in &feld.eintraege {
                            if ui.selectable_label(e.wert == feld.wert, &e.text).clicked() {
                                neu = Some(e.wert.clone());
                            }
                        }
                    });
                ui.ctx().accesskit_node_builder(cb.response.id, |n| {
                    n.set_role(Role::ComboBox);
                    n.set_label(name.to_owned());
                    n.set_author_id(kennung.to_owned());
                });
                (neu, cb.response.id)
            };
            let (h, id) = wahl(
                ui,
                "Harness",
                &self.kern.start.harness.clone(),
                "harness-wahl",
            );
            self.modal_halte_neu.push(id);
            if let Some(h) = h {
                self.kern.start.harness.wert = h;
            }
            // B69: model and effort at the start; the base form is the model, the effort comes with L10
            let mit_gruppe = self.zeigt("B69");
            ui.scope_builder(UiBuilder::new().id_salt("modell-effort"), |ui| {
                if mit_gruppe {
                    behaelter(ui, Role::Group, "Modell und Effort", Some("B69"));
                }
                let (m, id) = wahl(ui, "Modell", &self.kern.start.modell.clone(), "modell-wahl");
                self.modal_halte_neu.push(id);
                if let Some(m) = m {
                    self.kern.start.modell.wert = m;
                }
                if self.hat(10) {
                    let route = self.kern.start.route.wert.clone();
                    let erlaubt = self
                        .zusatz
                        .efforts
                        .iter()
                        .find(|(r, _)| *r == route)
                        .map(|(_, l)| l.clone())
                        .unwrap_or_default();
                    if f.effort.is_empty() || !erlaubt.contains(&f.effort) {
                        f.effort = if erlaubt.contains(&self.zusatz.effort_vorgabe) {
                            self.zusatz.effort_vorgabe.clone()
                        } else {
                            erlaubt.first().cloned().unwrap_or_default()
                        };
                    }
                    ui.label(
                        RichText::new("Effort")
                            .size(MASSE.schrift_klein)
                            .color(r.gedaempft),
                    );
                    let cb = egui::ComboBox::from_id_salt("effort-wahl")
                        .selected_text(f.effort.clone())
                        .width(ui.available_width() - 8.0)
                        .show_ui(ui, |ui| {
                            for e in &erlaubt {
                                if ui.selectable_label(*e == f.effort, e).clicked() {
                                    f.effort.clone_from(e);
                                }
                            }
                        });
                    ui.ctx().accesskit_node_builder(cb.response.id, |n| {
                        n.set_role(Role::ComboBox);
                        n.set_label("Effort".to_owned());
                        n.set_class_name("effort-wahl");
                    });
                    self.modal_halte_neu.push(cb.response.id);
                }
            });
            let (ro, id) = wahl(ui, "Route", &self.kern.start.route.clone(), "route-wahl");
            self.modal_halte_neu.push(id);
            if let Some(ro) = ro {
                self.kern.start.route_waehlen(&ro);
            }
            let (k, id) = wahl(ui, "Kunde", &self.kern.start.kunde.clone(), "kunde-wahl");
            self.modal_halte_neu.push(id);
            if let Some(k) = k {
                self.kern.start.kunde.wert = k;
            }
            ui.add_space(8.0);
            let k = ui.add(
                egui::Button::new(RichText::new("Starten").color(r.akzent_tinte))
                    .fill(r.akzent_flaeche)
                    .stroke(Stroke::NONE),
            );
            ui.ctx()
                .accesskit_node_builder(k.id, |n| n.set_author_id("sitzung-start".to_owned()));
            self.modal_halte_neu.push(k.id);
            if k.clicked() {
                let mut s = self.kern.start.struktur();
                s["art"] = json!("Starten");
                if self.hat(10) {
                    // L10: version 2 of the start structure with name and effort (SE6 point 3)
                    s["fassung"] = json!(2);
                    s["name"] = json!(f.name);
                    s["effort"] = json!(f.effort);
                }
                self.senden(&s);
            }
            if let Some((code, text)) = &self.kern.fehler
                && self.zeigt("B71")
            {
                let l = ui
                    .horizontal(|ui| {
                        punkt(ui, r.tot, true);
                        ui.label(RichText::new(format!("{text} ({code})")).color(r.schrift))
                    })
                    .inner;
                ui.ctx().accesskit_node_builder(l.id, |n| {
                    n.set_role(Role::Alert);
                    n.set_author_id("B71".to_owned());
                    n.set_class_name("fehler");
                });
            }
        });
    }

    fn einstellungen_fenster(&mut self, ui: &mut Ui, r: &Rollen) {
        let Some(mut seite) = self.b.einstellungen else {
            return;
        };
        if !self.zeigt("B72") {
            self.b.einstellungen = None;
            return;
        }
        let ctx = ui.ctx().clone();
        let mut offen = true;
        let platz = ctx.content_rect();
        let groesse = vec2(
            MASSE
                .einstellungsfenster
                .0
                .min(platz.width() - 2.0 * MASSE.innen),
            MASSE
                .einstellungsfenster
                .1
                .min(platz.height() - 2.0 * MASSE.innen),
        );
        egui::Window::new("Einstellungen")
            .id(Id::unique("einstellungen-fenster"))
            .title_bar(false)
            .collapsible(false)
            .resizable(false)
            .fixed_size(groesse)
            .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
            .frame(
                Frame::window(&ctx.global_style())
                    .fill(r.grund)
                    .inner_margin(Margin::ZERO)
                    .corner_radius(gestalt::radius(MASSE.radius_fenster)),
            )
            .show(&ctx, |ui| {
                ui.set_min_size(groesse);
                let voll = Rect::from_min_size(ui.max_rect().min, groesse);
                let leiste =
                    Rect::from_min_size(voll.min, vec2(MASSE.einstellungsleiste, voll.height()));
                ui.painter().rect_filled(
                    leiste,
                    CornerRadius {
                        nw: gestalt::radius(MASSE.radius_fenster).nw,
                        sw: gestalt::radius(MASSE.radius_fenster).sw,
                        ne: 0,
                        se: 0,
                    },
                    r.leiste,
                );
                // the list of pages, each with its symbol (OF9 point 6)
                let mut lu = ui.new_child(
                    UiBuilder::new()
                        .max_rect(leiste.shrink2(vec2(0.0, MASSE.innen)))
                        .layout(Layout::top_down(Align::Min)),
                );
                lu.add_space(MASSE.titelleiste - MASSE.innen);
                lu.scope_builder(UiBuilder::new().id_salt("einstellungen-nutzer"), |ui| {
                    behaelter(ui, Role::Navigation, "Einstellungen", Some("B72"));
                    ui.ctx().accesskit_node_builder(ui.unique_id(), |n| {
                        n.set_class_name("einstellungen");
                    });
                    Self::seitengruppe(ui, r, "Für dich");
                    // without L4 the base form has only the page of the window's own choices (OF8 point 3)
                    let mit_l4 = self.hat(4);
                    if !mit_l4 {
                        seite = Seite::Aussehen;
                    }
                    for (s, sym, t) in [
                        (Seite::Aussehen, Symbol::Kontrast, "Aussehen"),
                        (Seite::Sitzung, Symbol::Terminal, "Sitzung"),
                        (Seite::Harnesses, Symbol::Chip, "Harnesses und Modelle"),
                    ] {
                        if !mit_l4 && s != Seite::Aussehen {
                            continue;
                        }
                        if Self::seitenzeile(ui, r, sym, t, seite == s) {
                            seite = s;
                        }
                    }
                });
                if self.zusatz.admin && self.zeigt("B78") {
                    lu.add_space(MASSE.gruppenabstand);
                    lu.scope_builder(UiBuilder::new().id_salt("einstellungen-admin"), |ui| {
                        behaelter(ui, Role::Navigation, "Admin-Einstellungen", Some("B78"));
                        ui.ctx().accesskit_node_builder(ui.unique_id(), |n| {
                            n.set_class_name("admin-einstellungen");
                        });
                        Self::seitengruppe(ui, r, "Admin");
                        for (s, sym, t) in [
                            (Seite::Harnesses, Symbol::Chip, "Harnesses und Modelle"),
                            (Seite::AdminEffort, Symbol::Regler, "Grenzen des Efforts"),
                        ] {
                            if Self::seitenzeile(ui, r, sym, t, seite == s) {
                                seite = s;
                            }
                        }
                    });
                }
                // the page: a title row with the name of the window and the close button, then the groups
                let inhalt = Rect::from_min_max(pos2(leiste.right(), voll.top()), voll.max);
                let mut cu = ui.new_child(
                    UiBuilder::new()
                        .max_rect(inhalt)
                        .layout(Layout::top_down(Align::Min)),
                );
                cu.allocate_ui_with_layout(
                    vec2(inhalt.width(), MASSE.titelleiste),
                    Layout::left_to_right(Align::Center),
                    |ui| {
                        ui.add_space(MASSE.titel_einzug);
                        ui.label(
                            RichText::new("Einstellungen")
                                .size(MASSE.schrift_titel)
                                .color(r.gedaempft),
                        );
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.add_space(MASSE.innen);
                            let k = symbole::knopf(
                                ui,
                                Symbol::Schliessen,
                                "Einstellungen schließen",
                                MASSE.kapsel_segment,
                                false,
                            );
                            if k.clicked() {
                                offen = false;
                            }
                        });
                    },
                );
                let breite = MASSE
                    .einstellungsseite
                    .min(inhalt.width() - 4.0 * MASSE.titel_einzug);
                let rand = (inhalt.width() - breite) / 2.0;
                let seite_rect = Rect::from_min_max(
                    pos2(inhalt.left() + rand, inhalt.top() + MASSE.titelleiste),
                    pos2(inhalt.right() - rand, inhalt.bottom() - MASSE.innen),
                );
                let mut su = cu.new_child(
                    UiBuilder::new()
                        .max_rect(seite_rect)
                        .layout(Layout::top_down(Align::Min)),
                );
                egui::ScrollArea::vertical()
                    .auto_shrink(false)
                    .show(&mut su, |ui| self.einstellungsseite(ui, r, seite));
            });
        if offen && !ctx.input(|i| i.key_pressed(Key::Escape)) {
            self.b.einstellungen = Some(seite);
        } else {
            self.b.einstellungen = None;
        }
    }

    /// The small heading of a group of pages ("Für dich", "Admin").
    fn seitengruppe(ui: &mut Ui, r: &Rollen, text: &str) {
        ui.horizontal(|ui| {
            ui.add_space(MASSE.kopf_einzug);
            ui.label(
                RichText::new(text)
                    .strong()
                    .size(MASSE.schrift_klein)
                    .color(r.gedaempft),
            );
        });
        ui.add_space(2.0);
    }

    /// One row of the list of pages: symbol and name, the chosen one on grey (OF9 point 6).
    fn seitenzeile(ui: &mut Ui, r: &Rollen, sym: Symbol, text: &str, gewaehlt: bool) -> bool {
        let (rect, resp) = ui.allocate_exact_size(
            vec2(ui.available_width(), MASSE.seitenzeile),
            Sense::click(),
        );
        let feld = rect.shrink2(vec2(MASSE.innen, 0.0));
        let rad = gestalt::radius(MASSE.radius_wahl);
        if gewaehlt {
            ui.painter().rect_filled(feld, rad, r.wahl);
        } else if resp.hovered() {
            ui.painter()
                .rect_filled(feld, rad, r.wahl.gamma_multiply(0.5));
        }
        let s = Rect::from_center_size(
            pos2(feld.left() + 11.0 + 9.0, feld.center().y),
            Vec2::splat(16.0),
        );
        symbole::malen(ui, sym, s, r.schrift);
        text_gekuerzt(
            ui,
            pos2(
                feld.left() + MASSE.zeile_text,
                feld.center().y - MASSE.schrift_grund * 0.62,
            ),
            text,
            FontId::proportional(MASSE.schrift_grund),
            r.schrift,
            feld.right() - feld.left() - MASSE.zeile_text - MASSE.innen,
        );
        let name = text.to_owned();
        resp.widget_info(|| egui::WidgetInfo::selected(Role::Button, true, gewaehlt, &name));
        resp.clicked()
    }

    /// A group of settings: heading, then the rows on a raised ground with lines between them (OF9 point 6).
    fn gruppe(&mut self, ui: &mut Ui, r: &Rollen, titel: &str, zeilen: &[Einstellungszeile]) {
        ui.add_space(MASSE.gruppenabstand * 2.0);
        ui.horizontal(|ui| {
            ui.add_space(MASSE.innen);
            ui.label(
                RichText::new(titel)
                    .strong()
                    .size(MASSE.schrift_grund)
                    .color(r.schrift),
            );
        });
        ui.add_space(MASSE.gruppe_kopf - 8.0);
        Frame::new()
            .fill(r.erhoben)
            .corner_radius(gestalt::radius(MASSE.radius_gruppe))
            .inner_margin(Margin::symmetric(10, 2))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                for (i, z) in zeilen.iter().enumerate() {
                    if i > 0 {
                        let y = ui.cursor().top();
                        ui.painter().hline(
                            ui.max_rect().x_range(),
                            y,
                            Stroke::new(MASSE.linie, r.linie),
                        );
                    }
                    self.einstellung_zeile(ui, r, z);
                }
            });
    }

    fn einstellung_zeile(&mut self, ui: &mut Ui, r: &Rollen, z: &Einstellungszeile) {
        // without L4 the look is a choice of this device and works at once (base form of B72)
        let lokal = !self.hat(4) && z.schluessel.starts_with("aussehen.");
        let e = self.zusatz.einstellung(z.schluessel).cloned();
        let (wert, herkunft, aenderbar) = if lokal {
            (
                self.lokal_wert(z.schluessel),
                "dieses Gerät".to_owned(),
                true,
            )
        } else {
            (
                e.as_ref().map(|x| wert_text(&x.wert)).unwrap_or_default(),
                e.as_ref().map(|x| x.herkunft.clone()).unwrap_or_default(),
                e.as_ref().is_some_and(|x| x.aenderbar),
            )
        };
        // a locked value names its reason briefly (OF9 point 6)
        let hinweis = match (aenderbar, herkunft.is_empty()) {
            (true, _) => herkunft,
            (false, true) => "Vorgabe der Admins".to_owned(),
            (false, false) => format!("Vorgabe der {herkunft}"),
        };
        let hoehe = 44.0;
        let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), hoehe), Sense::hover());
        // right: the switch or the choice, before it the origin; left: name and one line of description
        let band = Rect::from_center_size(rect.center(), vec2(rect.width(), MASSE.formfeld));
        let mut rechts = ui.new_child(
            UiBuilder::new()
                .max_rect(band)
                .layout(Layout::right_to_left(Align::Center)),
        );
        let schalter = z.wahlen.len() == 2 && z.wahlen[0].0 == "true";
        rechts.add_enabled_ui(aenderbar, |ui| {
            if z.wahlen.is_empty() {
                ui.label(RichText::new(&wert).strong());
            } else if schalter {
                let an = wert == "true";
                let k = schalter_malen(ui, r, an, z.name);
                if k.clicked() {
                    self.einstellung_senden(z.schluessel, if an { "false" } else { "true" });
                }
            } else {
                Frame::new()
                    .fill(r.linie)
                    .corner_radius(gestalt::radius(MASSE.radius_feld))
                    .inner_margin(Margin::same(2))
                    .show(ui, |ui| {
                        ui.spacing_mut().item_spacing.x = 0.0;
                        ui.spacing_mut().interact_size.y = MASSE.knopf - 2.0;
                        ui.visuals_mut().selection.bg_fill = r.reiter_wahl;
                        let w = &mut ui.visuals_mut().widgets;
                        for x in [&mut w.inactive, &mut w.hovered, &mut w.active] {
                            x.corner_radius = gestalt::radius(MASSE.radius_feld - 1.0);
                            x.bg_stroke = Stroke::NONE;
                        }
                        // right to left: the last choice first
                        for (w, t) in z.wahlen.iter().rev() {
                            if ui.selectable_label(wert == *w, *t).clicked() {
                                if lokal {
                                    self.lokal_setzen(z.schluessel, w);
                                } else {
                                    self.einstellung_senden(z.schluessel, w);
                                }
                            }
                        }
                    });
            }
        });
        rechts.add_space(MASSE.abstand);
        Frame::new()
            .fill(r.kopf)
            .corner_radius(gestalt::radius(MASSE.radius_kapsel))
            .inner_margin(Margin::symmetric(7, 2))
            .show(&mut rechts, |ui| {
                ui.label(
                    RichText::new(&hinweis)
                        .size(MASSE.schrift_klein)
                        .color(r.gedaempft),
                );
            });
        let links_ende = rechts.min_rect().left() - MASSE.innen;
        let x = rect.left();
        let breite = links_ende - x;
        text_gekuerzt(
            ui,
            pos2(x, rect.top() + 6.0),
            z.name,
            FontId::proportional(MASSE.schrift_grund),
            r.schrift,
            breite,
        );
        text_gekuerzt(
            ui,
            pos2(x, rect.top() + 24.0),
            z.beschreibung,
            FontId::proportional(MASSE.schrift_klein),
            r.gedaempft,
            breite,
        );
    }

    /// The value of a look choice of this device, as the settings write it.
    fn lokal_wert(&self, schluessel: &str) -> String {
        match schluessel {
            "aussehen.thema" => match self.wahl.thema {
                Thema::System => "system",
                Thema::Hell => "hell",
                Thema::Dunkel => "dunkel",
            }
            .to_owned(),
            "aussehen.farbsatz" => match self.wahl.farbsatz {
                Farbsatz::Grau => "grau",
                Farbsatz::Blauschwarz => "blauschwarz",
            }
            .to_owned(),
            "aussehen.terminal" => match self.wahl.terminal {
                Terminalwahl::Dunkel => "dunkel",
                Terminalwahl::Hell => "hell",
                Terminalwahl::WieThema => "wie_thema",
            }
            .to_owned(),
            "aussehen.schriftgroesse" => format!("{}", self.terminalschrift.round()),
            _ => String::new(),
        }
    }

    /// Sets a look choice of this device at once and remembers it (L8).
    fn lokal_setzen(&mut self, schluessel: &str, w: &str) {
        self.aussehen_anwenden(schluessel, w);
        self.merken.aussehen_setzen(schluessel, w);
        self.merken_geaendert = true;
    }

    /// Applies a look choice of this device.
    fn aussehen_anwenden(&mut self, schluessel: &str, w: &str) {
        match (schluessel, w) {
            ("aussehen.thema", "hell") => self.wahl.thema = Thema::Hell,
            ("aussehen.thema", "dunkel") => self.wahl.thema = Thema::Dunkel,
            ("aussehen.thema", _) => self.wahl.thema = Thema::System,
            ("aussehen.farbsatz", "blauschwarz") => self.wahl.farbsatz = Farbsatz::Blauschwarz,
            ("aussehen.farbsatz", _) => self.wahl.farbsatz = Farbsatz::Grau,
            ("aussehen.terminal", "hell") => self.wahl.terminal = Terminalwahl::Hell,
            ("aussehen.terminal", "wie_thema") => self.wahl.terminal = Terminalwahl::WieThema,
            ("aussehen.terminal", _) => self.wahl.terminal = Terminalwahl::Dunkel,
            ("aussehen.schriftgroesse", g) => {
                if let Ok(g) = g.parse::<f32>()
                    && (8.0..=32.0).contains(&g)
                {
                    self.terminalschrift = g;
                }
            }
            _ => {}
        }
    }

    fn einstellung_senden(&mut self, schluessel: &str, w: &str) {
        // the value in its type: a switch as true or false, a size as a number, else text
        let wert: Value = serde_json::from_str(w).unwrap_or_else(|_| json!(w));
        let admin = schluessel.starts_with("admin.");
        self.senden(&json!({"art": "EinstellungSetzen", "schluessel": schluessel, "wert": wert,
            "bereich": if admin { "admin" } else { "nutzer" }, "ebene": if admin { "firma" } else { "nutzer" }}));
    }

    fn einstellungsseite(&mut self, ui: &mut Ui, r: &Rollen, seite: Seite) {
        let titel = match seite {
            Seite::Aussehen => "Aussehen",
            Seite::Sitzung => "Sitzung",
            Seite::Harnesses => "Harnesses und Modelle",
            Seite::AdminEffort => "Grenzen des Efforts",
        };
        ui.horizontal(|ui| {
            ui.add_space(MASSE.innen);
            ui.label(RichText::new(titel).strong().size(MASSE.schrift_seite));
        });
        let z = |schluessel, name, beschreibung, wahlen| Einstellungszeile {
            schluessel,
            name,
            beschreibung,
            wahlen,
        };
        match seite {
            Seite::Aussehen => {
                if self.zeigt("B77") {
                    ui.scope_builder(UiBuilder::new().id_salt("farbwahl"), |ui| {
                        behaelter(ui, Role::Group, "Farbwahl", Some("B77"));
                        self.gruppe(
                            ui,
                            r,
                            "Farben",
                            &[
                                z(
                                    "aussehen.thema",
                                    "Thema",
                                    "Hell, dunkel oder wie das System",
                                    &[("system", "System"), ("hell", "Hell"), ("dunkel", "Dunkel")],
                                ),
                                z(
                                    "aussehen.farbsatz",
                                    "Farbsatz dunkel",
                                    "Grau oder Blauschwarz",
                                    &[("grau", "Grau"), ("blauschwarz", "Blauschwarz")],
                                ),
                                z(
                                    "aussehen.terminal",
                                    "Terminal",
                                    "Dunkel, hell oder wie das Thema",
                                    &[
                                        ("dunkel", "Dunkel"),
                                        ("hell", "Hell"),
                                        ("wie_thema", "Wie Thema"),
                                    ],
                                ),
                            ],
                        );
                    });
                }
                self.gruppe(
                    ui,
                    r,
                    "Schrift",
                    &[z(
                        "aussehen.schriftgroesse",
                        "Terminalschrift",
                        "Größe in Punkten",
                        &[("12", "12"), ("13", "13"), ("14", "14")],
                    )],
                );
            }
            Seite::Sitzung => {
                self.gruppe(
                    ui,
                    r,
                    "Vorgaben für neue Sitzungen",
                    &[
                        z(
                            "sitzung.harness",
                            "Harness",
                            "Aus der Freigabe der Admins",
                            &[("claude", "claude"), ("pi", "pi")],
                        ),
                        z(
                            "sitzung.modell",
                            "Modell",
                            "Vorbelegt beim Start",
                            &[("sonnet", "sonnet"), ("opus", "opus")],
                        ),
                        z(
                            "sitzung.effort",
                            "Effort",
                            "Innerhalb der Grenzen der Admins",
                            &[("low", "low"), ("medium", "medium"), ("high", "high")],
                        ),
                    ],
                );
            }
            Seite::Harnesses => {
                self.gruppe(
                    ui,
                    r,
                    "Freigegeben",
                    &[
                        z(
                            "admin.harness.pi",
                            "Harness pi",
                            "Lokale Modelle über pi",
                            &[("true", "an"), ("false", "aus")],
                        ),
                        z(
                            "admin.aufsicht.guard",
                            "Guard",
                            "Prüft Werkzeugaufrufe vor der Ausführung",
                            &[("true", "an"), ("false", "aus")],
                        ),
                    ],
                );
                if !self.zusatz.admin {
                    ui.add_space(MASSE.abstand);
                    ui.horizontal(|ui| {
                        ui.add_space(MASSE.innen);
                        ui.label(
                            RichText::new("Nur Admins ändern diese Werte.")
                                .size(MASSE.schrift_klein)
                                .color(r.gedaempft),
                        );
                    });
                }
            }
            Seite::AdminEffort => {
                self.gruppe(
                    ui,
                    r,
                    "Grenzen",
                    &[z(
                        "admin.modell.opus.effort_max",
                        "opus",
                        "Höchster Effort",
                        &[("medium", "medium"), ("high", "high")],
                    )],
                );
            }
        }
    }

    fn rueckfrage(&mut self, ui: &mut Ui) {
        let Some(rf) = self.b.rueckfrage.clone() else {
            return;
        };
        let ctx = ui.ctx().clone();
        let mut neu = Some(rf.clone());
        let antwort = egui::Modal::new(Id::unique("rueckfrage")).show(&ctx, |ui| {
            ui.set_width(360.0);
            behaelter(ui, Role::AlertDialog, "Rückfrage", None);
            match &rf {
                Rueckfrage::Loeschen(id) | Rueckfrage::Wiederbeleben(id) => {
                    let loeschen = matches!(rf, Rueckfrage::Loeschen(_));
                    ui.label(if loeschen {
                        format!("Sitzung {id} endgültig löschen? Das lässt sich nicht rückgängig machen.")
                    } else {
                        format!("Sitzung {id} wiederbeleben?")
                    });
                    ui.horizontal(|ui| {
                        let k = ui.button(if loeschen { "Löschen" } else { "Wiederbeleben" });
                        self.modal_halte_neu.push(k.id);
                        if k.clicked() {
                            let art = if loeschen { "Loeschen" } else { "Wiederbeleben" };
                            self.senden(&json!({"art": art, "sitzung": id}));
                            neu = None;
                        }
                        let a = ui.button("Abbrechen");
                        self.modal_halte_neu.push(a.id);
                        if a.clicked() {
                            neu = None;
                        }
                    });
                }
                Rueckfrage::Umbenennen(id, text) => {
                    let mut t = text.clone();
                    ui.label("Neuer Name");
                    let e = ui.add(egui::TextEdit::singleline(&mut t));
                    name_setzen(ui, e.id, "Neuer Name");
                    self.modal_halte_neu.push(e.id);
                    ui.horizontal(|ui| {
                        let k = ui.button("Umbenennen");
                        self.modal_halte_neu.push(k.id);
                        if k.clicked() {
                            self.senden(&json!({"art": "Umbenennen", "sitzung": id, "name": t}));
                            neu = None;
                        }
                        let a = ui.button("Abbrechen");
                        self.modal_halte_neu.push(a.id);
                        if a.clicked() {
                            neu = None;
                        }
                    });
                    if neu.is_some() {
                        neu = Some(Rueckfrage::Umbenennen(id.clone(), t));
                    }
                }
            }
        });
        self.b.rueckfrage = if antwort.should_close() { None } else { neu };
    }

    // ------------------------------------------------------------------------------------------------ overlays

    fn live_region(&self, ui: &Ui) {
        let id = Id::unique("ansage");
        let _ = ui.interact(
            Rect::from_min_size(Pos2::ZERO, Vec2::ZERO),
            id,
            Sense::hover(),
        );
        let t = self.ansage.clone();
        ui.ctx().accesskit_node_builder(id, |n| {
            n.set_role(Role::Status);
            n.set_live(Live::Polite);
            n.set_label(t);
        });
    }

    fn fokusrahmen(ui: &Ui, r: &Rollen) {
        let ctx = ui.ctx();
        let Some(f) = ctx.memory(egui::Memory::focused) else {
            return;
        };
        let Some(resp) = ctx.read_response(f) else {
            return;
        };
        // on the layer of the focused element, so that a window above it covers the frame too
        ctx.layer_painter(resp.layer_id).rect_stroke(
            resp.rect.shrink(1.0),
            CornerRadius::same(4),
            Stroke::new(MASSE.fokusrahmen, r.akzent),
            StrokeKind::Inside,
        );
    }

    fn beschriftung(ui: &Ui, r: &Rollen) {
        let ctx = ui.ctx();
        let rect = ctx.content_rect();
        let p = ctx.layer_painter(egui::LayerId::new(
            egui::Order::Tooltip,
            Id::unique("beschriftung"),
        ));
        let pos = pos2(rect.right() - 12.0, rect.bottom() - 10.0);
        let galley = p.layout_no_wrap(
            BESCHRIFTUNG.to_owned(),
            FontId::proportional(MASSE.schrift_klein),
            r.wartet_tinte,
        );
        let bg = Rect::from_min_size(
            pos - galley.size() - vec2(8.0, 4.0),
            galley.size() + vec2(16.0, 8.0),
        );
        p.rect_filled(bg, gestalt::radius(MASSE.radius_knopf), r.wartet_flaeche);
        p.galley(bg.min + vec2(8.0, 4.0), galley, r.wartet_tinte);
    }
}

impl eframe::App for Fenster {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        self.zeichnen(ui);
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.merken_schreiben();
    }
}
