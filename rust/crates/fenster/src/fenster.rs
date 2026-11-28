//! The window: it reads the view core, draws, and sends the events of the view through the [`Weg`]. The state of the
//! sessions, the choice of a session, the keyboard rules, the texts of rejections and announcements live in the view
//! core of the crate `ansicht` ([`ansicht::kern::Kern`]); this module keeps only what it draws (open windows, widths,
//! the tab of the inspector, the focus area).

// Each drawing function draws one area of the window in its layout order, so several are long.
#![allow(clippy::too_many_lines, clippy::many_single_char_names)]

use std::path::PathBuf;

use ansicht::kern::{Kern, Sitzung, Wirkung};
use ansicht::tasten::Deutung;
use egui::accesskit::{Live, Role};
use egui::{
    Align, Align2, Color32, CornerRadius, FontId, Frame, Id, Key, Layout, Margin, Modifiers, Pos2,
    Rect, Response, RichText, Sense, Stroke, StrokeKind, Ui, UiBuilder, Vec2, pos2, vec2,
};
use rasterauszug::Empfang;
use serde_json::{Value, json};

use crate::Fett;
use crate::bedienwege::{Luecken, sichtbar};
use crate::gestalt::{self, Farbsatz, MASSE, Rollen, Terminalwahl, Thema, Wahl};
use crate::menue::{Eintrag, Tat as MenueTat, Untermenue};
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

/// A page of the settings (B72, B78): the pages of the pilot (F2) for the user and, apart, for the admins (OF9 point 1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Seite {
    /// Look (B77), a choice of this device.
    Aussehen,
    /// Defaults for new sessions.
    Sitzung,
    /// Programs and models: the release of the admins as it holds for the user.
    Harnesses,
    /// Permissions as they hold for the user.
    Erlaubnisse,
    /// Supervision as it holds for the user.
    Aufsicht,
    /// Admin: release of programs and models, limits of the effort.
    AdminHarnesses,
    /// Admin: permissions (the options of the pilot).
    AdminErlaubnisse,
    /// Admin: supervision.
    AdminAufsicht,
}

impl Seite {
    /// The pages of the user area, in their order.
    pub const NUTZER: [Seite; 5] = [
        Seite::Aussehen,
        Seite::Sitzung,
        Seite::Harnesses,
        Seite::Erlaubnisse,
        Seite::Aufsicht,
    ];
    /// The pages of the admin area, in their order.
    pub const ADMIN: [Seite; 3] = [
        Seite::AdminHarnesses,
        Seite::AdminErlaubnisse,
        Seite::AdminAufsicht,
    ];

    /// Whether the page belongs to the admin area.
    #[must_use]
    pub fn admin(self) -> bool {
        Seite::ADMIN.contains(&self)
    }

    /// Title, short description and symbol (OF9 point 6).
    #[must_use]
    pub fn kopf(self) -> (&'static str, &'static str, Symbol) {
        use crate::einstellungstexte::seite as t;
        match self {
            Seite::Aussehen => (t::AUSSEHEN, t::AUSSEHEN_KURZ, Symbol::Kontrast),
            Seite::Sitzung => (t::SITZUNG, t::SITZUNG_KURZ, Symbol::Terminal),
            Seite::Harnesses => (t::PROGRAMME, t::PROGRAMME_KURZ, Symbol::Chip),
            Seite::AdminHarnesses => (t::PROGRAMME, t::PROGRAMME_ADMIN_KURZ, Symbol::Chip),
            Seite::Erlaubnisse | Seite::AdminErlaubnisse => {
                (t::ERLAUBNISSE, t::ERLAUBNISSE_KURZ, Symbol::Schild)
            }
            Seite::Aufsicht | Seite::AdminAufsicht => (t::AUFSICHT, t::AUFSICHT_KURZ, Symbol::Auge),
        }
    }

    /// The element id of its row in the list of pages (for the tree of the platform).
    #[must_use]
    pub fn kennung(self) -> &'static str {
        match self {
            Seite::Aussehen => "seite-aussehen",
            Seite::Sitzung => "seite-sitzung",
            Seite::Harnesses => "seite-programme",
            Seite::Erlaubnisse => "seite-erlaubnisse",
            Seite::Aufsicht => "seite-aufsicht",
            Seite::AdminHarnesses => "seite-admin-programme",
            Seite::AdminErlaubnisse => "seite-admin-erlaubnisse",
            Seite::AdminAufsicht => "seite-admin-aufsicht",
        }
    }

    /// The groups of a page of the service with their rows (`regelebenen` 15.1); the look has its own rows.
    #[must_use]
    pub fn gruppen(self) -> Vec<(&'static str, Vec<crate::einstellungstexte::Zeile>)> {
        use crate::einstellungstexte::{gruppe as g, werte as w};
        match self {
            Seite::Aussehen => Vec::new(),
            Seite::Sitzung => vec![(g::NEUE_SITZUNGEN, vec![w::HARNESS, w::MODELL, w::EFFORT])],
            Seite::Harnesses | Seite::AdminHarnesses => vec![
                (g::FREIGABE, vec![w::HARNESSES, w::MODELLE]),
                (g::EFFORT, vec![w::EFFORT_GRENZEN]),
            ],
            Seite::Erlaubnisse | Seite::AdminErlaubnisse => vec![
                (
                    g::COPILOT,
                    vec![w::OHNE_RUECKFRAGE, w::TELEMETRIE, w::TOKEN],
                ),
                (g::NETZ, vec![w::LOKALE_CA]),
                (g::HERSTELLER, vec![w::HERSTELLERZUGANG]),
            ],
            // only what the pilot evaluates (OF8): the deadlines of a release; no switch that turns a check off
            Seite::Aufsicht | Seite::AdminAufsicht => {
                vec![(g::FRISTEN, vec![w::FREIGABEFRIST, w::EINLOESEFRIST])]
            }
        }
    }
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
    /// View "Worker" with one tile, zoomed: no frame.
    WorkerEinzeln,
    /// View "Worker" with the worker list open.
    WorkerListe,
    /// The window "Sitzungen" with an empty form.
    SitzungenLeer,
    /// The window "Sitzungen" with this ended session chosen.
    SitzungenGewaehlt(String),
    /// The settings at this page.
    Einstellungen(Seite),
}

/// Where the focus of the window came from.
#[derive(Clone, Copy, Debug)]
struct Eingabe {
    /// Whether the window had the focus in the last frame: on gaining it, the accent colour is read again.
    hatte_fokus: bool,
    /// Whether the focus was last moved with the keyboard (or an assistive technology): only then the focus frame shows.
    tastatur: bool,
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
    /// The accent colour of the system for the frame of the active tile (macOS; `None`: the accent token).
    pub system_akzent: Option<Color32>,
    /// Where the focus came from: window and keyboard.
    eingabe: Eingabe,
    wahl: Wahl,
    terminalschrift: f32,
    /// The menu bar of macOS (B75), once the program has set it up.
    #[cfg(target_os = "macos")]
    menueleiste: Option<crate::menue::Menueleiste>,
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

/// One row of the settings: key, name, one line of description and the choices (value, text); without choices the
/// value stands as text, or as a switch when it is true or false.
struct Einstellungszeile {
    schluessel: &'static str,
    name: &'static str,
    beschreibung: &'static str,
    wahlen: Vec<(String, String)>,
}

/// The track of a locked switch: the step from the ground of a group (`erhoben`) towards the muted text that first holds
/// 3:1 against that ground (OF3 point 5), so a locked switch stays visible in every colour set.
#[must_use]
pub fn gesperrte_spur(r: &Rollen) -> Color32 {
    let (a, b) = (r.erhoben, r.gedaempft);
    let mut farbe = b;
    for i in 0..=20_u8 {
        let t = f32::from(i) / 20.0;
        let mische = |x: u8, y: u8| {
            // between two channel values, so within 0 to 255
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let v = (f32::from(x) + (f32::from(y) - f32::from(x)) * t).round() as u8;
            v
        };
        let c = Color32::from_rgb(
            mische(a.r(), b.r()),
            mische(a.g(), b.g()),
            mische(a.b(), b.b()),
        );
        if gestalt::kontrast(c, a) >= 3.2 {
            farbe = c;
            break;
        }
    }
    farbe
}

/// How much the ground behind a dialog is dimmed: black at 28 %, like the browser view (`.fenster-grund`).
const GRUND_DECKUNG: u8 = 71;

/// The ground of the symbol of an admin page, like the browser view (`.einst-gross.admin`).
const ADMIN_FARBE: Color32 = Color32::from_rgb(0x6e, 0x56, 0xcf);

/// The place of the settings (B72): the size of the browser view, centred, and with a margin below the title row and
/// above the foot of the sidebar, so that both stay visible (OF2 point 2, OF5).
#[must_use]
pub fn einstellungen_lage(platz: Rect) -> Rect {
    let oben = platz.top() + MASSE.titelleiste + MASSE.dialog_rand;
    let unten = platz.bottom() - MASSE.leistenfuss - MASSE.dialog_rand;
    let breite = MASSE
        .einstellungsfenster
        .0
        .min(platz.width() - 2.0 * MASSE.dialog_rand)
        .max(0.0);
    let hoehe = MASSE.einstellungsfenster.1.min(unten - oben).max(0.0);
    let y = (platz.center().y - hoehe / 2.0).clamp(oben, (unten - hoehe).max(oben));
    Rect::from_min_size(
        pos2(platz.center().x - breite / 2.0, y),
        vec2(breite, hoehe),
    )
}

/// A switch (on: the accent surface, off: the choice colour, a light knob), named for the accessibility tree. A locked
/// switch is painted at full opacity: its track holds 3:1 to the ground and its knob has the colour of the ground.
fn schalter_malen(ui: &mut Ui, r: &Rollen, an: bool, name: &str) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(38.0, 22.0), Sense::click());
    let offen = ui.is_enabled();
    let mut p = ui.painter().clone();
    p.set_opacity(1.0);
    let (spur, knopf) = if !offen {
        (gesperrte_spur(r), r.erhoben)
    } else if an {
        (r.akzent_flaeche, Color32::WHITE)
    } else {
        (r.wahl, Color32::WHITE)
    };
    p.rect_filled(rect, gestalt::radius(11.0), spur);
    let x = if an {
        rect.right() - 11.0
    } else {
        rect.left() + 11.0
    };
    p.circle_filled(pos2(x, rect.center().y), 9.0, knopf);
    let name = name.to_owned();
    resp.widget_info(|| egui::WidgetInfo::selected(Role::Switch, offen, an, &name));
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

// The words of the window "Sitzungen" come from the table both views share (`ansicht::woerter`, OF1).
const T_HINWEIS: &str = ansicht::woerter::SITZUNGEN_HINWEIS;
const T_KEINE_SITZUNG: &str = ansicht::woerter::KEINE_SITZUNG_BEKANNT;
const T_INFO_LEER: &str = ansicht::woerter::SITZUNG_WAEHLEN;
const T_KEINE_GEWAEHLT: &str = ansicht::woerter::GRUND_KEINE_GEWAEHLT;
const T_IST_BEENDET: &str = ansicht::woerter::GRUND_IST_BEENDET;
const T_LAEUFT: &str = ansicht::woerter::GRUND_LAEUFT;
const T_SCHON_FORTGESETZT: &str = ansicht::woerter::GRUND_SCHON_FORTGESETZT;
const T_OHNE_FORTSETZEN: &str = ansicht::woerter::GRUND_NICHT_FORTSETZBAR;

/// The programs whose sessions cannot be continued: the run refuses a continuation of opencode (exit code 69).
const OHNE_FORTSETZEN: [&str; 1] = ["opencode"];

/// Why "Fortsetzen" is locked for the chosen session; empty when it may be clicked.
fn fortsetzen_grund(s: Option<&Sitzung>) -> &'static str {
    match s {
        None => T_KEINE_GEWAEHLT,
        Some(s) if s.laeuft => T_LAEUFT,
        Some(s)
            if s.herkunft
                .as_ref()
                .is_some_and(|h| h.fortgesetzt_als.is_some()) =>
        {
            T_SCHON_FORTGESETZT
        }
        Some(s) if OHNE_FORTSETZEN.contains(&s.harness.as_str()) => T_OHNE_FORTSETZEN,
        Some(_) => "",
    }
}
const T_OPTIONAL: &str = ansicht::woerter::NAME_OPTIONAL;
const T_PFAD: &str = ansicht::woerter::PFAD_PLATZHALTER;
const T_PROGRAMM: &str = ansicht::woerter::PROGRAMM;
const T_DENKSTUFE: &str = ansicht::woerter::DENKSTUFE;

/// Height of the head and of the foot of the window "Sitzungen", width of its list (browser view: 12 px around a 15 px
/// title, 10 px around the buttons, 300 px).
const FENSTER_KOPF: f32 = 44.0;
const FENSTER_FUSS: f32 = 50.0;
const LISTE_SPALTE: f32 = 300.0;

/// The sessions by working folder (L5), the folders in the order of their first session and the sessions of a folder
/// in the order of the list (B68, F1). Sessions without a folder form the group "".
pub(crate) fn nach_ordner(sitzungen: &[Sitzung], zusatz: &Zusatz) -> Vec<(String, Vec<String>)> {
    let mut gruppen: Vec<(String, Vec<String>)> = Vec::new();
    for s in sitzungen {
        let o = zusatz
            .sitzung(&s.id)
            .map(|x| x.ordner.clone())
            .unwrap_or_default();
        match gruppen.iter_mut().find(|(g, _)| *g == o) {
            Some((_, ids)) => ids.push(s.id.clone()),
            None => gruppen.push((o, vec![s.id.clone()])),
        }
    }
    gruppen
}

/// The colour of the point of a session, the same in the sidebar and in the window "Sitzungen": running, ended with code
/// 0, or ended with another code.
fn sitzung_farbe(r: &Rollen, s: &Sitzung) -> Color32 {
    if s.laeuft {
        r.laeuft
    } else if s.code == "0" {
        r.gedaempft
    } else {
        r.tot
    }
}

/// The second line of a row of the sidebar: with L1 "N Worker · <state>", else the state. The state is the one name of
/// the view core ([`Sitzung::zustand_text`]), the same as in the window "Sitzungen" and in the browser view.
pub(crate) fn zeile_zusatz(s: &Sitzung, worker: Option<usize>) -> String {
    match worker {
        Some(n) => format!("{n} Worker · {}", s.zustand_text()),
        None => s.zustand_text(),
    }
}

/// A group of the start form: a bold heading, then the rows on a raised card with a line around (F1, like the browser
/// view).
fn formgruppe(ui: &mut Ui, r: &Rollen, titel: &str, inhalt: impl FnOnce(&mut Ui)) {
    if ui.min_rect().height() > 0.0 {
        ui.add_space(MASSE.innen);
    }
    ui.label(
        RichText::new(titel)
            .fett()
            .size(MASSE.schrift_grund)
            .color(r.schrift),
    );
    ui.add_space(2.0);
    Frame::new()
        .fill(r.erhoben)
        .stroke(Stroke::new(MASSE.linie, r.linie))
        .corner_radius(gestalt::radius(MASSE.radius_gruppe))
        .inner_margin(Margin::symmetric(14, 2))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = 0.0;
            inhalt(ui);
        });
}

/// One row of the start form, 40 points high: the name on the left, the field on the right over 55 % of the width, a
/// line above every row but the first. `feld` gets the width of the field.
fn formzeile<R>(
    ui: &mut Ui,
    r: &Rollen,
    name: &str,
    erste: bool,
    feld: impl FnOnce(&mut Ui, f32) -> R,
) -> R {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 40.0), Sense::hover());
    if !erste {
        ui.painter().hline(
            rect.x_range(),
            rect.top(),
            Stroke::new(MASSE.linie, r.linie),
        );
    }
    let breite = (rect.width() * 0.55).round();
    text_gekuerzt(
        ui,
        pos2(rect.left(), rect.center().y - MASSE.schrift_grund * 0.62),
        name,
        FontId::proportional(MASSE.schrift_grund),
        r.schrift,
        rect.width() - breite - 14.0,
    );
    // the band of the field is as high as a field, like in the rows of the settings: a segmented choice keeps its
    // ground off the lines between the rows
    let band = Rect::from_center_size(
        pos2(rect.right() - breite / 2.0, rect.center().y),
        vec2(breite, MASSE.formfeld + 2.0),
    );
    let mut c = ui.new_child(
        UiBuilder::new()
            .max_rect(band)
            .layout(Layout::left_to_right(Align::Center)),
    );
    feld(&mut c, breite)
}

/// A value of the service as one line: a list joined with commas, an object as "key: value" joined with " · ", an
/// effort limit as "harness: values" (`regelebenen` 15.1); no value as "–" like the browser view.
fn wert_anzeige(v: &Value) -> String {
    use crate::einstellungstexte::{AN, AUS};
    match v {
        Value::Null => "–".to_owned(),
        Value::Bool(true) => AN.to_owned(),
        Value::Bool(false) => AUS.to_owned(),
        Value::String(s) => s.clone(),
        Value::Array(a) => a
            .iter()
            .map(|x| match (x.get("harness"), x.get("erlaubt")) {
                (Some(h), Some(e)) => format!("{}: {}", wert_anzeige(h), wert_anzeige(e)),
                _ => wert_anzeige(x),
            })
            .collect::<Vec<_>>()
            .join(if a.iter().any(Value::is_object) {
                " · "
            } else {
                ", "
            }),
        Value::Object(m) => m
            .iter()
            .map(|(k, x)| format!("{k}: {}", wert_anzeige(x)))
            .collect::<Vec<_>>()
            .join(" · "),
        x => x.to_string(),
    }
}

/// A deadline in milliseconds in the largest whole unit: "30 s", "5 min", "24 h"; otherwise in milliseconds.
fn frist_text(ms: u64) -> String {
    match ms {
        m if m >= 3_600_000 && m % 3_600_000 == 0 => format!("{} h", m / 3_600_000),
        m if m >= 60_000 && m % 60_000 == 0 => format!("{} min", m / 60_000),
        m if m >= 1000 && m % 1000 == 0 => format!("{} s", m / 1000),
        m => format!("{m} ms"),
    }
}

/// The choices of a deadline of a release (`regelebenen` 15.1.2), with the effective value when it is none of them.
fn frist_wahlen(feste: &[u64], wert: &Value) -> Vec<(String, String)> {
    let mut l: Vec<u64> = feste.to_vec();
    if let Some(w) = wert.as_u64()
        && !l.contains(&w)
    {
        l.push(w);
        l.sort_unstable();
    }
    l.into_iter()
        .map(|m| (m.to_string(), frist_text(m)))
        .collect()
}

/// The choices of `sitzung.harness` (the catalog of the view core, `regelebenen` 15.1) and `sitzung.modell`, bound to the
/// limit of the admins like in the browser view (`ansicht::ziel::in_grenze`; decision of the operator, 28.11.2026). An
/// effective value outside them is no choice: no segment is marked.
fn sitzung_wahlen(schluessel: &str, zusatz: &Zusatz) -> Vec<(String, String)> {
    let werte: &[&str] = match schluessel {
        "sitzung.harness" => ansicht::ziel::KATALOG
            .iter()
            .find(|(k, ..)| *k == schluessel)
            .map_or(&[], |(_, _, w)| *w),
        "sitzung.modell" => &["sonnet", "opus"],
        _ => &[],
    };
    let grenze = ansicht::ziel::grenze_von(schluessel)
        .and_then(|g| zusatz.einstellung(g))
        .map(|e| &e.wert);
    ansicht::ziel::in_grenze(werte, grenze)
        .into_iter()
        .map(|x| (x.clone(), x))
        .collect()
}

/// The largest number of values a segmented choice shows; a choice of more values is a drop-down. One rule for both
/// views (decision of the operator, 28.11.2026).
pub const SEGMENTE_HOECHSTENS: usize = 4;

/// The form of a choice.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Wahlform {
    /// The values side by side, the chosen one marked.
    Segmente,
    /// One field with the chosen value that opens the list of values.
    Ausklapper,
}

/// The form of a choice of `anzahl` values: up to [`SEGMENTE_HOECHSTENS`] segments, from one more a drop-down. Every
/// choice of the window takes its form from here.
#[must_use]
pub fn wahlform(anzahl: usize) -> Wahlform {
    if anzahl <= SEGMENTE_HOECHSTENS {
        Wahlform::Segmente
    } else {
        Wahlform::Ausklapper
    }
}

/// The width of a drop-down in a row of the settings.
const AUSKLAPPER_BREITE: f32 = 150.0;

/// A choice in the form of [`wahlform`]: segments on a sunken ground, or a drop-down of `breite` points. `kennung` is the
/// element id of every control of the choice, `name` its name; `wahlen` are value and text, `gewaehlt` the chosen
/// value (none is marked when it is no choice). Returns the value clicked and the ids of the controls.
fn wahl_malen(
    ui: &mut Ui,
    r: &Rollen,
    kennung: &str,
    name: &str,
    wahlen: &[(String, String)],
    gewaehlt: &str,
    breite: f32,
) -> (Option<String>, Vec<Id>) {
    let mut neu = None;
    let mut ids = Vec::new();
    match wahlform(wahlen.len()) {
        Wahlform::Segmente => {
            ui.scope_builder(UiBuilder::new().id_salt(("segmente", kennung)), |ui| {
                Frame::new()
                    .fill(r.linie)
                    .corner_radius(gestalt::radius(MASSE.radius_feld))
                    .inner_margin(Margin::same(2))
                    .show(ui, |ui| {
                        // the Ui inside the frame holds the segments: it is the group with the name of the choice
                        behaelter(ui, Role::RadioGroup, name, None);
                        ui.spacing_mut().item_spacing.x = 0.0;
                        ui.spacing_mut().interact_size.y = MASSE.knopf - 2.0;
                        ui.visuals_mut().selection.bg_fill = r.reiter_wahl;
                        let w = &mut ui.visuals_mut().widgets;
                        for x in [&mut w.inactive, &mut w.hovered, &mut w.active] {
                            x.corner_radius = gestalt::radius(MASSE.radius_feld - 1.0);
                            x.bg_stroke = Stroke::NONE;
                        }
                        // in a row from right to left the last choice comes first
                        let von_rechts = ui.layout().main_dir() == egui::Direction::RightToLeft;
                        let reihe: Vec<&(String, String)> = if von_rechts {
                            wahlen.iter().rev().collect()
                        } else {
                            wahlen.iter().collect()
                        };
                        for (w, t) in reihe {
                            let k = ui.selectable_label(gewaehlt == w, t.as_str());
                            let s = kennung.to_owned();
                            ui.ctx()
                                .accesskit_node_builder(k.id, |n| n.set_author_id(s));
                            ids.push(k.id);
                            if k.clicked() {
                                neu = Some(w.clone());
                            }
                        }
                    });
            });
        }
        Wahlform::Ausklapper => {
            let aktuell = wahlen
                .iter()
                .find(|(w, _)| w == gewaehlt)
                .map_or_else(|| gewaehlt.to_owned(), |(_, t)| t.clone());
            let cb = egui::ComboBox::from_id_salt(kennung)
                .selected_text(aktuell)
                .width(breite)
                .show_ui(ui, |ui| {
                    for (w, t) in wahlen {
                        let k = ui.selectable_label(gewaehlt == w, t.as_str());
                        let s = kennung.to_owned();
                        ui.ctx()
                            .accesskit_node_builder(k.id, |n| n.set_author_id(s));
                        if k.clicked() {
                            neu = Some(w.clone());
                        }
                    }
                });
            ui.ctx().accesskit_node_builder(cb.response.id, |n| {
                n.set_role(Role::ComboBox);
                n.set_label(name.to_owned());
                n.set_author_id(kennung.to_owned());
            });
            ids.push(cb.response.id);
        }
    }
    (neu, ids)
}

/// One cell of the table of deadlines: the deadline and its origin.
type Fristzelle = Option<(String, String)>;

/// A deadline in the table in seconds like in the browser view: "60 s"; a deadline below a whole second in milliseconds.
fn sekunden_text(ms: u64) -> String {
    if ms.is_multiple_of(1000) {
        format!(
            "{} {}",
            ms / 1000,
            crate::einstellungstexte::server::SEKUNDEN
        )
    } else {
        format!("{ms} ms")
    }
}

/// The table of the effective deadlines per MCP server (`regelebenen` 15.1.2; decision of the operator, 28.11.2026): one
/// row per server in the order of the message, the deadline of the decision and the deadline to redeem, each with its
/// origin "Firma" (file of the company) or "Vorgabe" (catalogue); an origin in another form stays as it is, a missing
/// deadline is `None`.
fn fristen_tabelle(fristen: &[crate::ziel::Frist]) -> Vec<(String, [Fristzelle; 2])> {
    use crate::einstellungstexte::{server as t, werte};
    let mut server: Vec<&str> = Vec::new();
    for f in fristen {
        if !server.contains(&f.server.as_str()) {
            server.push(&f.server);
        }
    }
    let zelle = |s: &str, schluessel: &str| -> Fristzelle {
        fristen
            .iter()
            .find(|f| f.server == s && f.schluessel == schluessel)
            .map(|f| {
                let herkunft = match f.herkunft.as_str() {
                    "firmendatei" => t::HERKUNFT_FIRMENDATEI,
                    "katalog" => t::HERKUNFT_KATALOG,
                    x => x,
                };
                (sekunden_text(f.wert), herkunft.to_owned())
            })
    };
    server
        .into_iter()
        .map(|s| {
            (
                s.to_owned(),
                [
                    zelle(s, werte::FREIGABEFRIST.0),
                    zelle(s, werte::EINLOESEFRIST.0),
                ],
            )
        })
        .collect()
}

/// The word of a thinking level: `none` (copilot, opencode) and `off` (pi) are „aus“ from the shared words, every other
/// value stays as the program names it.
fn effort_wort(wert: &str) -> String {
    match wert {
        "none" | "off" => crate::einstellungstexte::wahl::AUS.to_owned(),
        x => x.to_owned(),
    }
}

/// The choices of `sitzung.effort` from the source of the browser view, the efforts of the routes in `Startwahl` (the
/// service takes them from `effort.grenzen` per program, class of the route and model; contract 2, block `effort`):
/// „Vorgabe des Programms“ first (value `""`, sent as `null`), then each effort a route allows for the program `harness`,
/// without doubles and in the order of the routes; without a program the efforts of every program, like the browser
/// view. Decision of the operator, 28.11.2026.
fn denkstufen(
    efforts: &[(String, Vec<ansicht::ziel::Effort>)],
    harness: Option<&str>,
) -> Vec<(String, String)> {
    let mut werte: Vec<String> = Vec::new();
    for e in efforts
        .iter()
        .flat_map(|(_, l)| l.iter())
        .filter(|e| harness.is_none_or(|h| e.harness == h))
    {
        // a value a route adds goes behind the value before it in that route, so each route keeps its order
        let mut nach = 0;
        for x in &e.erlaubt {
            if let Some(i) = werte.iter().position(|w| w == x) {
                nach = i + 1;
            } else {
                werte.insert(nach, x.clone());
                nach += 1;
            }
        }
    }
    let mut wahlen = vec![(
        String::new(),
        crate::einstellungstexte::wahl::VORGABE_DES_PROGRAMMS.to_owned(),
    )];
    wahlen.extend(werte.into_iter().map(|w| {
        let t = effort_wort(&w);
        (w, t)
    }));
    wahlen
}

/// A date `JJJJ-MM-TT` as `TT.MM.JJJJ`; any other text stays as it is.
fn datum_text(d: &str) -> String {
    let t: Vec<&str> = d.split('-').collect();
    match t.as_slice() {
        [j, m, tag] if j.len() == 4 && m.len() == 2 && tag.len() == 2 => format!("{tag}.{m}.{j}"),
        _ => d.to_owned(),
    }
}

/// Milliseconds since 1970 as "TT.MM.JJJJ, HH:MM UTC" (the civil calendar of the days since 1970, without a time zone).
fn zeit_text(ms: u64) -> String {
    let s = ms / 1000;
    let (tage, rest) = (s / 86_400, s % 86_400);
    // the civil date of a day count (days from civil, inverted; valid for every day after 1970)
    let z = tage + 719_468;
    let era = z / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let tag = doy - (153 * mp + 2) / 5 + 1;
    let monat = if mp < 10 { mp + 3 } else { mp - 9 };
    let jahr = yoe + era * 400 + u64::from(monat <= 2);
    format!(
        "{tag:02}.{monat:02}.{jahr}, {:02}:{:02} {}",
        rest / 3600,
        rest % 3600 / 60,
        crate::einstellungstexte::schutz::UTC
    )
}

/// The rows of the protection layers: only what was measured, with its date; a part the service did not report says
/// so (decision of the operator, 27.11.2026).
fn schutz_zeilen(s: &crate::ziel::Schutz) -> Vec<(String, String, String)> {
    use crate::einstellungstexte::schutz as t;
    let oder = |w: Option<String>| w.unwrap_or_else(|| t::NICHT_GEMELDET.to_owned());
    let mut zeilen = vec![
        (
            t::EINSCHLUSS.to_owned(),
            oder(
                s.probelauf
                    .as_deref()
                    .map(|d| format!("{} {}", t::PROBELAUF, datum_text(d))),
            ),
            String::new(),
        ),
        (
            t::HALTER.to_owned(),
            oder(
                s.eingerichtet
                    .as_deref()
                    .map(|d| format!("{} {}", t::EINGERICHTET, datum_text(d))),
            ),
            String::new(),
        ),
        (
            t::LETZTER_START.to_owned(),
            match &s.letzter_start {
                None => t::NICHT_GEMELDET.to_owned(),
                Some(None) => t::KEIN_START.to_owned(),
                Some(Some((zeit, ergebnis))) => format!("{ergebnis}, {}", zeit_text(*zeit)),
            },
            String::new(),
        ),
    ];
    match &s.netzziele {
        None => zeilen.push((
            t::NETZZIELE.to_owned(),
            t::NICHT_GEMELDET.to_owned(),
            String::new(),
        )),
        Some(l) if l.is_empty() => {
            zeilen.push((t::NETZZIELE.to_owned(), "0".to_owned(), String::new()));
        }
        Some(l) => {
            for (herkunft, anzahl) in l {
                zeilen.push((
                    t::NETZZIELE.to_owned(),
                    anzahl.to_string(),
                    herkunft_anzeige(herkunft),
                ));
            }
        }
    }
    zeilen
}

/// The origin as text (`regelebenen` 15.5): "Firma", "Gruppe <name>" or "du"; an origin in another form stays as it is.
fn herkunft_anzeige(h: &str) -> String {
    use crate::einstellungstexte::{HERKUNFT_DU, HERKUNFT_FIRMA, HERKUNFT_GRUPPE};
    match h.split_once(' ') {
        Some(("gruppe", g)) => format!("{HERKUNFT_GRUPPE} {g}"),
        _ => match h {
            "firma" => HERKUNFT_FIRMA.to_owned(),
            "nutzer" => HERKUNFT_DU.to_owned(),
            "gruppe" => HERKUNFT_GRUPPE.to_owned(),
            x => x.to_owned(),
        },
    }
}

/// The short reason of a lock (`regelebenen` 15.5, OF9 point 6); empty when nothing is locked.
fn grund_anzeige(grund: Option<&str>, aenderbar: bool, herkunft: &str) -> String {
    use crate::einstellungstexte::{
        GRUND_FEST, GRUND_NUR_ADMIN, GRUND_NUR_STRENGER, GRUND_VORGABE,
    };
    match grund {
        Some("nur_admin") => GRUND_NUR_ADMIN.to_owned(),
        Some("nur_strenger") => GRUND_NUR_STRENGER.to_owned(),
        Some("fest") => GRUND_FEST.to_owned(),
        Some(x) => x.to_owned(),
        None if aenderbar => String::new(),
        None => format!("{GRUND_VORGABE} {herkunft}"),
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
            system_akzent: crate::akzent::system(),
            eingabe: Eingabe {
                hatte_fokus: true,
                tastatur: false,
            },
            wahl: Wahl::default(),
            terminalschrift: MASSE.schrift_terminal,
            #[cfg(target_os = "macos")]
            menueleiste: None,
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
                self.b.kachelform = Kachelform::Gekachelt;
            }
            Szene::WorkerEinzeln => {
                self.b.mitte = Mitte::Worker;
                self.merken.inspektor_offen = false;
                self.b.kachelform = Kachelform::Gezoomt;
            }
            Szene::WorkerListe => {
                self.b.mitte = Mitte::Worker;
                self.merken.inspektor_offen = false;
                self.b.worker_liste = true;
            }
            Szene::SitzungenLeer => {
                self.b.sitzungen = Some(Sitzungsfenster::default());
                // like "+": the focus in the first field of the form (SE2 point 2)
                self.fokus = Some(FokusZiel::Name);
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
            .einstellung(crate::einstellungstexte::SCHLUESSEL_SCHRIFTGROESSE)
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
        // the accent colour may have changed in the settings of the system while the window was in the background
        let fokus = ctx.input(|i| i.viewport().focused.unwrap_or(true));
        if fokus && !self.eingabe.hatte_fokus {
            self.system_akzent = crate::akzent::system();
        }
        self.eingabe.hatte_fokus = fokus;
        // a key or a request of an assistive technology moves the focus with the keyboard, a press of the mouse does not
        for e in ctx.input(|i| i.events.clone()) {
            match e {
                egui::Event::Key { pressed: true, .. } | egui::Event::AccessKitActionRequest(_) => {
                    self.eingabe.tastatur = true;
                }
                egui::Event::PointerButton { pressed: true, .. } => self.eingabe.tastatur = false,
                _ => {}
            }
        }
        self.empfangen();
        #[cfg(target_os = "macos")]
        self.menueleiste_abgleichen(&ctx);
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
        if self.eingabe.tastatur {
            Self::fokusrahmen(ui, &rollen);
        }
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
                // left: the sidebar button only, as today; the application menu (B75) is in the foot of the sidebar or the
                // menu bar of macOS
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
                                        RichText::new(&projekt).fett().size(MASSE.schrift_titel),
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

    /// The application menu in the foot of the sidebar next to the gear (B75, Windows and Linux; G3). Under macOS the
    /// menu bar of the system shows the same list, see [`Fenster::anwendungsmenue`]. It opens upwards.
    fn menue(&mut self, ui: &mut Ui) {
        let k = symbole::knopf(ui, Symbol::Menue, "Menü", MASSE.kapsel_segment, false);
        kennung(ui, k.id, "B75");
        ui.ctx().accesskit_node_builder(k.id, |n| {
            n.set_role(Role::MenuBar);
            n.set_class_name("anwendungsmenue");
        });
        self.halt(LEISTE, &k);
        let liste = self.anwendungsmenue();
        let mut gewaehlt = None;
        egui::Popup::menu(&k)
            .align(egui::RectAlign::TOP_START)
            .show(|ui| {
                ui.set_min_width(220.0);
                for m in &liste {
                    ui.menu_button(m.titel, |ui| {
                        for e in &m.eintraege {
                            let b = ui.add_enabled(
                                e.an,
                                egui::Button::new(e.text)
                                    .frame(false)
                                    .min_size(vec2(200.0, 22.0)),
                            );
                            rolle_setzen(ui, b.id, Role::MenuItem);
                            if b.clicked() {
                                gewaehlt = Some(e.tat);
                            }
                        }
                    });
                }
            });
        if let Some(t) = gewaehlt {
            self.menue_tat(ui.ctx(), t);
        }
    }

    /// The entries of the application menu (B75), for the menu in the foot of the sidebar and the menu bar of macOS
    /// alike. It holds only entries whose operating path shows (OF8); a menu without entries is left out.
    #[must_use]
    pub fn anwendungsmenue(&self) -> Vec<Untermenue> {
        if !self.zeigt("B75") {
            return Vec::new();
        }
        let e = |tat, text, an, weg| Eintrag { tat, text, an, weg };
        let aktiv = self.kern.aktive_sitzung();
        let mut werkbank = Vec::new();
        if self.zeigt("B72") {
            werkbank.push(e(MenueTat::Einstellungen, "Einstellungen …", true, "B72"));
        }
        werkbank.push(e(MenueTat::Beenden, "Werkbank beenden", true, "B75"));
        let mut ablage = Vec::new();
        if self.zeigt("B68") {
            ablage.push(e(MenueTat::NeueSitzung, "Neue Sitzung …", true, "B68"));
        }
        let mut bearbeiten = Vec::new();
        if self.zeigt("B38") {
            bearbeiten.push(e(
                MenueTat::Kopieren,
                "Kopieren",
                self.markierung.is_some(),
                "B38",
            ));
            bearbeiten.push(e(MenueTat::Einfuegen, "Einfügen", aktiv.is_some(), "B38"));
        }
        let mut sitzung = Vec::new();
        if let Some(s) = aktiv {
            if self.zeigt("B11") {
                sitzung.push(e(MenueTat::Fortsetzen, "Fortsetzen", !s.laeuft, "B11"));
            }
            if self.zeigt("B12") {
                sitzung.push(e(MenueTat::Umbenennen, "Namen ändern …", true, "B12"));
            }
            if self.zeigt("B15") {
                sitzung.push(e(
                    MenueTat::Schliessen,
                    "Sitzung schließen",
                    s.laeuft,
                    "B15",
                ));
            }
            if self.zeigt("B16") {
                sitzung.push(e(
                    MenueTat::Loeschen,
                    "Endgültig löschen …",
                    !s.laeuft,
                    "B16",
                ));
            }
        }
        let mut darstellung = Vec::new();
        if self.zeigt("B80") {
            darstellung.push(e(
                MenueTat::Leiste,
                "Leiste ein- oder ausblenden",
                true,
                "B80",
            ));
        }
        if self.zeigt("B04") {
            darstellung.push(e(
                MenueTat::Inspektor,
                "Inspektor ein- oder ausblenden",
                true,
                "B04",
            ));
        }
        if self.zeigt("B27") {
            darstellung.push(e(
                MenueTat::Orchestrator,
                "Orchestrator",
                self.b.mitte != Mitte::Orchestrator,
                "B27",
            ));
            darstellung.push(e(
                MenueTat::Worker,
                "Worker",
                self.b.mitte != Mitte::Worker,
                "B27",
            ));
        }
        if self.zeigt("B29") {
            darstellung.push(e(MenueTat::WorkerListe, "Worker-Liste", true, "B29"));
        }
        [
            ("Werkbank", werkbank),
            ("Ablage", ablage),
            ("Bearbeiten", bearbeiten),
            ("Sitzung", sitzung),
            ("Darstellung", darstellung),
        ]
        .into_iter()
        .filter(|(_, l)| !l.is_empty())
        .map(|(titel, eintraege)| Untermenue { titel, eintraege })
        .collect()
    }

    /// Carries out an entry of the application menu, from the foot of the sidebar or the menu bar of macOS.
    pub fn menue_tat(&mut self, ctx: &egui::Context, tat: MenueTat) {
        let aktiv = self.kern.aktive_sitzung().cloned();
        match tat {
            MenueTat::Einstellungen => self.b.einstellungen = Some(Seite::Aussehen),
            MenueTat::Beenden => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
            MenueTat::NeueSitzung => {
                self.b.sitzungen = Some(Sitzungsfenster::default());
                self.fokus = Some(FokusZiel::Name);
            }
            MenueTat::Kopieren => {
                if let (Some(b), Some(m)) = (self.kern.bild.as_ref(), self.markierung.as_ref()) {
                    let t = terminal::markierter_text(b, m);
                    if !t.is_empty() {
                        ctx.copy_text(t);
                    }
                }
            }
            MenueTat::Einfuegen => {
                if aktiv.is_some() {
                    self.fokus = Some(FokusZiel::Terminal);
                    ctx.send_viewport_cmd(egui::ViewportCommand::RequestPaste);
                }
            }
            MenueTat::Fortsetzen => {
                if let Some(s) = aktiv.filter(|s| !s.laeuft) {
                    self.senden(&json!({"art": "Fortsetzen", "sitzung": s.id}));
                }
            }
            MenueTat::Umbenennen => {
                if let Some(s) = aktiv {
                    let alt = self
                        .zusatz
                        .sitzung(&s.id)
                        .map(|x| x.name.clone())
                        .unwrap_or_default();
                    self.b.rueckfrage = Some(Rueckfrage::Umbenennen(s.id.clone(), alt));
                }
            }
            MenueTat::Schliessen => {
                if let Some(s) = aktiv.filter(|s| s.laeuft) {
                    let w = self.kern.beenden(&s.id);
                    self.wirkungen(w);
                }
            }
            MenueTat::Loeschen => {
                if let Some(s) = aktiv.filter(|s| !s.laeuft) {
                    self.b.rueckfrage = Some(Rueckfrage::Loeschen(s.id.clone()));
                }
            }
            MenueTat::Leiste => self.b.leiste_offen = !self.b.leiste_offen,
            MenueTat::Inspektor => {
                self.merken.inspektor_offen = !self.merken.inspektor_offen;
                self.merken_geaendert = true;
            }
            MenueTat::Orchestrator | MenueTat::Worker => {
                let m = if tat == MenueTat::Worker {
                    Mitte::Worker
                } else {
                    Mitte::Orchestrator
                };
                if self.b.mitte != m {
                    self.b.mitte = m;
                    self.kachel_abo.clear();
                }
            }
            MenueTat::WorkerListe => self.b.worker_liste = !self.b.worker_liste,
        }
    }

    /// Builds the menu bar of macOS (B75) from [`Fenster::anwendungsmenue`]. Call it once, in the creator of eframe;
    /// without it (tests, pictures) the window touches no menu of the system.
    #[cfg(target_os = "macos")]
    pub fn menueleiste_einrichten(&mut self, ctx: &egui::Context) {
        self.menueleiste = Some(crate::menue::Menueleiste::neu(ctx));
    }

    /// Brings the menu bar of macOS to the list and carries out the entries chosen there.
    #[cfg(target_os = "macos")]
    fn menueleiste_abgleichen(&mut self, ctx: &egui::Context) {
        let liste = self.anwendungsmenue();
        let gewaehlt = match self.menueleiste.as_mut() {
            Some(m) => m.abgleichen(&liste),
            None => return,
        };
        for t in gewaehlt {
            self.menue_tat(ctx, t);
        }
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
                    // bold as today (B26)
                    RichText::new(format!("    {n} laufen"))
                        .fett()
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
                            .fett()
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
                // the number in the bold cut, the words in the regular one (as today, B22)
                let mut job = egui::text::LayoutJob::default();
                let f = FontId::proportional(MASSE.schrift_grund);
                job.append(
                    &n.to_string(),
                    0.0,
                    egui::TextFormat {
                        font_id: FontId::new(
                            MASSE.schrift_grund,
                            egui::FontFamily::Name(crate::FETT.into()),
                        ),
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
                // under Windows and Linux the application menu stands next to the gear (B75, G3); macOS has the menu bar
                if self.zeigt("B75") && !cfg!(target_os = "macos") {
                    self.menue(ui);
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
                            // a small grey heading in the regular cut, as today (B09)
                            ui.label(
                                RichText::new(&p)
                                    .size(MASSE.schrift_klein)
                                    .color(r.gedaempft),
                            );
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                ui.add_space(MASSE.innen);
                                ui.label(
                                    RichText::new(ids.len().to_string())
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
        let farbe = sitzung_farbe(r, &s);
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
            &zeile_zusatz(&s, self.hat(1).then_some(worker)),
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
                        ui.label(RichText::new("Dialog offen").fett());
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
            self.pane_kopf(
                ui,
                r,
                kopf,
                "Orchestrator",
                &s.modell,
                zustand,
                true,
                None,
                CornerRadius::ZERO,
            );
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
            // the marking starts where the button went down: egui reports the drag only after some movement, and a
            // slow frame (llvmpipe under Xvfb, 27.11.2026) brings press and movement in one frame
            if resp.drag_started()
                && let Some(p) = ui
                    .input(|i| i.pointer.press_origin())
                    .or_else(|| resp.interact_pointer_pos())
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
                CornerRadius::ZERO,
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
                n.set_label(
                    if cfg!(target_os = "macos") {
                        "Kopieren mit Befehl+C, Einfügen mit Befehl+V"
                    } else {
                        "Kopieren mit Strg+Umschalt+C oder Strg+Einfg, Einfügen mit Strg+V oder Umschalt+Einfg"
                    }
                    .to_owned(),
                );
                n.set_author_id("B38".to_owned());
            });
            let _ = r;
        });
    }

    fn tasten(&mut self, ui: &Ui, resp: &Response) {
        let events = ui.input(|i| i.events.clone());
        for t in tasten::lesen(&events, tasten::System::HIER) {
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
        let flaeche = ui.available_rect_before_wrap();
        // the stage: its ground shows in the joints and at the rounded corners of the tiles
        ui.painter().rect_filled(flaeche, 0.0, r.buehne);
        let lagen = gestalt::kachel_lagen(flaeche, n);
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
        for (i, (a, rect)) in sichtbar.iter().zip(lagen).enumerate() {
            self.kachel(
                ui,
                r,
                &tf,
                &schrift,
                (zw, zh),
                rect,
                a,
                &sitzung,
                (i == aktiv, gestalt::kachel_rahmen(i == aktiv, n)),
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
        (aktiv, rahmen): (bool, bool),
    ) {
        let kopf = Rect::from_min_size(rect.min, vec2(rect.width(), MASSE.pane_kopf));
        let bild_rect = Rect::from_min_max(pos2(rect.left(), kopf.bottom()), rect.max);
        // the tile has rounded corners (today's app, 8 points): the head carries the upper ones, the terminal the lower
        let ecke = gestalt::radius(MASSE.radius_kachel).nw;
        let unten = CornerRadius {
            sw: ecke,
            se: ecke,
            ..CornerRadius::ZERO
        };
        let oben = CornerRadius {
            nw: ecke,
            ne: ecke,
            ..CornerRadius::ZERO
        };
        ui.painter().rect_filled(bild_rect, unten, tf.grund);
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
                oben,
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
        terminal::malen(
            ui,
            bild_rect,
            unten,
            bild.as_ref(),
            tf,
            schrift,
            None,
            id,
            true,
        );
        // the frame of the active tile over head and terminal, in the accent colour of the system under macOS
        if rahmen {
            ui.painter().rect_stroke(
                rect,
                gestalt::radius(MASSE.radius_kachel),
                Stroke::new(MASSE.fokusrahmen, self.rahmenfarbe(r)),
                StrokeKind::Inside,
            );
        }
    }

    /// The colour of the frame of the active tile: the accent colour of the system under macOS, else the accent token.
    #[must_use]
    pub fn rahmenfarbe(&self, r: &Rollen) -> Color32 {
        self.system_akzent.unwrap_or(r.akzent)
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
        ecken: CornerRadius,
    ) {
        let mut ku = ui.new_child(
            UiBuilder::new()
                .max_rect(kopf)
                .layout(Layout::left_to_right(Align::Center))
                .id_salt(("kachel", rolle, zoom.unwrap_or("orchestrator"))),
        );
        ku.painter()
            .rect_filled(kopf, ecken, if aktiv { r.kopf_aktiv } else { r.kopf });
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
            // the role in the bold cut, as today (`fontWeight(.semibold)`)
            ui.label(
                RichText::new(rolle)
                    .fett()
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
            // right the button to enlarge (B41): a worker tile alone and back; the orchestrator stands alone like a zoomed
            // tile, and its button leads back to the tiles, as in today's app (`kachelnZurueck`)
            let gezoomt = match zoom {
                Some(id) => {
                    self.b.zoom.as_deref() == Some(id) || self.b.kachelform == Kachelform::Gezoomt
                }
                None => true,
            };
            if zoom.is_some() || self.zeigt("B27") {
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.add_space(MASSE.abstand);
                    let (symbol, name) = if gezoomt {
                        (Symbol::Verkleinern, "Verkleinern")
                    } else {
                        (Symbol::Vergroessern, "Vergrößern")
                    };
                    let z = symbole::knopf(ui, symbol, name, MASSE.pane_kopf - 4.0, false);
                    if z.clicked() {
                        match zoom {
                            Some(_) if gezoomt => {
                                self.b.zoom = None;
                                if self.b.kachelform == Kachelform::Gezoomt {
                                    self.b.kachelform = Kachelform::Gekachelt;
                                }
                            }
                            Some(id) => self.b.zoom = Some(id.to_owned()),
                            None => self.b.mitte = Mitte::Worker,
                        }
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
                    .fett()
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
                    ui.label(RichText::new(&a.werkzeug).fett());
                    ui.add(egui::Label::new(RichText::new(&a.vorschau).monospace().size(MASSE.schrift_klein)).truncate());
                    // the worker of a request is a field of L1; the base form names the session only (OF8 point 3)
                    let herkunft = if worker.is_empty() { a.sitzung.clone() } else { format!("{} · {}", a.sitzung, worker) };
                    ui.label(RichText::new(herkunft).size(MASSE.schrift_klein).color(r.gedaempft));
                    ui.horizontal(|ui| {
                        if ui.button("Freigeben").clicked() {
                            // `anfragen` and always `entscheidung` (R-L6 point 1), as `ansicht::ziel::freigabe` builds it
                            self.senden(&json!({"art": "Freigabe", "anfragen": [a.id], "entscheidung": "erteilen"}));
                        }
                        if ui.button("Ablehnen").clicked() {
                            let b = std::mem::take(&mut self.b.begruendung);
                            self.senden(&ansicht::ziel::freigabe(&a.id, false, &b));
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
                                        RichText::new(format!("Worker  {}", arbeiter.len())).fett(),
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
        let platz = ctx.content_rect();
        // like the browser view: at most 900 by 652, 20 points from the edge of a smaller window
        let groesse = vec2(
            MASSE.sitzungsfenster.0.min(platz.width() - 40.0),
            MASSE.sitzungsfenster.1.min(platz.height() - 40.0),
        );
        let mut schliessen = false;
        let antwort = egui::Modal::new(Id::unique("sitzungen-fenster"))
            .frame(
                Frame::window(&ctx.global_style())
                    .fill(r.grund)
                    .corner_radius(gestalt::radius(MASSE.radius_fenster))
                    .inner_margin(Margin::ZERO),
            )
            .show(&ctx, |ui| {
                ui.set_min_size(groesse);
                ui.set_max_size(groesse);
                ui.scope_builder(UiBuilder::new().id_salt("sitzungen-inhalt"), |ui| {
                    ui.set_min_size(groesse);
                    behaelter(ui, Role::Dialog, "Sitzungen", Some("B68"));
                    ui.ctx().accesskit_node_builder(ui.unique_id(), |n| {
                        n.set_modal();
                        n.set_class_name("sitzungen-fenster");
                    });
                    let voll = Rect::from_min_size(ui.max_rect().min, groesse);
                    let rad = gestalt::radius(MASSE.radius_fenster);
                    let kopf = Rect::from_min_size(voll.min, vec2(voll.width(), FENSTER_KOPF));
                    let fuss = Rect::from_min_max(
                        pos2(voll.left(), voll.bottom() - FENSTER_FUSS),
                        voll.max,
                    );
                    let leib = Rect::from_min_max(kopf.left_bottom(), fuss.right_top());
                    let liste = Rect::from_min_max(
                        leib.min,
                        pos2(leib.left() + LISTE_SPALTE, leib.bottom()),
                    );
                    let form = Rect::from_min_max(liste.right_top(), leib.max);
                    let p = ui.painter();
                    p.rect_filled(
                        kopf,
                        CornerRadius {
                            sw: 0,
                            se: 0,
                            ..rad
                        },
                        r.fenster,
                    );
                    p.rect_filled(
                        fuss,
                        CornerRadius {
                            nw: 0,
                            ne: 0,
                            ..rad
                        },
                        r.fenster,
                    );
                    p.rect_filled(liste, 0, r.leiste);
                    let linie = Stroke::new(MASSE.linie, r.linie);
                    p.hline(voll.x_range(), kopf.bottom(), linie);
                    p.hline(voll.x_range(), fuss.top(), linie);
                    p.vline(liste.right(), leib.y_range(), linie);
                    let mut ku = ui.new_child(
                        UiBuilder::new()
                            .max_rect(kopf.shrink2(vec2(18.0, 0.0)))
                            .layout(Layout::left_to_right(Align::Center)),
                    );
                    ku.label(
                        RichText::new("Sitzungen")
                            .fett()
                            .size(15.0)
                            .color(r.schrift),
                    );
                    let mut lu = ui.new_child(
                        UiBuilder::new()
                            .max_rect(liste.shrink2(vec2(18.0, 14.0)))
                            .layout(Layout::top_down(Align::Min)),
                    );
                    self.bekannte_sitzungen(&mut lu, r, &mut f);
                    let mut fu = ui.new_child(
                        UiBuilder::new()
                            .max_rect(form.shrink2(vec2(18.0, 14.0)))
                            .layout(Layout::top_down(Align::Min)),
                    );
                    egui::ScrollArea::vertical()
                        .id_salt("startformular-rollen")
                        .auto_shrink(false)
                        .show(&mut fu, |ui| self.startformular(ui, r, &mut f));
                    let mut uu = ui.new_child(
                        UiBuilder::new()
                            .max_rect(fuss.shrink2(vec2(18.0, 0.0)))
                            .layout(Layout::left_to_right(Align::Center)),
                    );
                    if self.sitzungen_fuss(&mut uu, r, &f) {
                        schliessen = true;
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

    /// The foot of the window "Sitzungen": the chosen session on the left, "Beenden", "Fortsetzen" and "Schließen" on
    /// the right (F1, like the browser view). Returns whether "Schließen" was clicked.
    fn sitzungen_fuss(&mut self, ui: &mut Ui, r: &Rollen, f: &Sitzungsfenster) -> bool {
        let gew = f.gewaehlt.clone();
        let s = gew
            .as_ref()
            .and_then(|g| self.kern.sitzungen.iter().find(|x| &x.id == g).cloned());
        let info = s.as_ref().map_or_else(
            || T_INFO_LEER.to_owned(),
            |s| {
                let mut t = vec![self.sitzungsname(&s.id), s.zustand_text()];
                let o = self.ordner_von(&s.id);
                if !o.is_empty() {
                    t.push(o);
                }
                t.join(" · ")
            },
        );
        let beenden_grund = match &s {
            None => T_KEINE_GEWAEHLT,
            Some(s) if !s.laeuft => T_IST_BEENDET,
            Some(_) => "",
        };
        let fortsetzen_grund = fortsetzen_grund(s.as_ref());
        let fortsetzen = self.zeigt("B11");
        // the buttons stand on the right; their width is measured first so that the order of the keyboard stays
        // "Beenden", "Fortsetzen", "Schließen" from left to right
        let knoepfe: Vec<&str> = if fortsetzen {
            vec!["Beenden", "Fortsetzen", "Schließen"]
        } else {
            vec!["Beenden", "Schließen"]
        };
        let schrift = egui::TextStyle::Button.resolve(ui.style());
        let polster = ui.spacing().button_padding.x;
        let abstand = ui.spacing().item_spacing.x;
        let breite_knoepfe: f32 = knoepfe
            .iter()
            .map(|t| {
                ui.painter()
                    .layout_no_wrap((*t).to_owned(), schrift.clone(), Color32::WHITE)
                    .size()
                    .x
                    + 2.0 * polster
                    + abstand
            })
            .sum();
        let breite_info = (ui.available_width() - breite_knoepfe - abstand).max(0.0);
        let h = ui.available_height();
        ui.allocate_ui_with_layout(
            vec2(breite_info, h),
            Layout::left_to_right(Align::Center),
            |ui| {
                ui.set_min_width(breite_info);
                ui.add(
                    egui::Label::new(
                        RichText::new(&info)
                            .size(MASSE.schrift_klein + 1.5)
                            .color(r.gedaempft),
                    )
                    .truncate(),
                );
            },
        );
        let k = ui
            .add_enabled(beenden_grund.is_empty(), egui::Button::new("Beenden"))
            .on_disabled_hover_text(beenden_grund);
        ui.ctx()
            .accesskit_node_builder(k.id, |n| n.set_class_name("sitzung-beenden"));
        self.modal_halte_neu.push(k.id);
        if k.clicked()
            && let Some(g) = &gew
        {
            let w = self.kern.beenden(g);
            self.wirkungen(w);
        }
        if fortsetzen {
            let k = ui
                .add_enabled(fortsetzen_grund.is_empty(), egui::Button::new("Fortsetzen"))
                .on_disabled_hover_text(fortsetzen_grund);
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
        // B76: Escape closes the window; the browser view carries the mark on "Schließen"
        let k = ui.button("Schließen");
        ui.ctx().accesskit_node_builder(k.id, |n| {
            n.set_class_name("sitzungen-schliessen");
            n.set_author_id("B76".to_owned());
        });
        self.modal_halte_neu.push(k.id);
        k.clicked()
    }

    /// The name of session `id` as the window shows it (L5), else the id.
    fn sitzungsname(&self, id: &str) -> String {
        self.zusatz
            .sitzung(id)
            .map(|x| x.name.clone())
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| id.to_owned())
    }

    /// The working folder of session `id` (L5), empty without it.
    fn ordner_von(&self, id: &str) -> String {
        self.zusatz
            .sitzung(id)
            .map(|x| x.ordner.clone())
            .unwrap_or_default()
    }

    fn bekannte_sitzungen(&mut self, ui: &mut Ui, r: &Rollen, f: &mut Sitzungsfenster) {
        ui.label(
            RichText::new(T_HINWEIS)
                .size(MASSE.schrift_klein + 1.5)
                .color(r.gedaempft),
        );
        ui.add_space(4.0);
        if self.kern.sitzungen.is_empty() {
            ui.label(
                RichText::new(T_KEINE_SITZUNG)
                    .size(MASSE.schrift_klein + 1.5)
                    .color(r.gedaempft),
            );
            return;
        }
        let gruppen = nach_ordner(&self.kern.sitzungen, &self.zusatz);
        egui::ScrollArea::vertical()
            .id_salt("bekannte-rollen")
            .auto_shrink(false)
            .show(ui, |ui| {
                ui.scope_builder(UiBuilder::new().id_salt("bekannte"), |ui| {
                    behaelter(ui, Role::ListBox, "Bekannte Sitzungen", None);
                    ui.spacing_mut().item_spacing.y = 2.0;
                    for (i, (ordner, ids)) in gruppen.iter().enumerate() {
                        // the folder is a field of L5; without it the sessions stand under no heading
                        if !ordner.is_empty() {
                            if i > 0 {
                                ui.add_space(MASSE.abstand);
                            }
                            let k = ui.label(
                                RichText::new(ordner)
                                    .fett()
                                    .size(MASSE.schrift_klein)
                                    .color(r.gedaempft),
                            );
                            ui.ctx().accesskit_node_builder(k.id, |n| {
                                n.set_role(Role::Heading);
                                n.set_label(ordner.clone());
                                n.set_class_name("sitzungen-ordner");
                            });
                        }
                        for id in ids {
                            let Some(s) = self.kern.sitzungen.iter().find(|x| &x.id == id).cloned()
                            else {
                                continue;
                            };
                            let text = format!("{} · {}", self.sitzungsname(id), s.zustand_text());
                            let gew = f.gewaehlt.as_ref() == Some(id);
                            let (rect, k) = ui.allocate_exact_size(
                                vec2(ui.available_width(), 30.0),
                                Sense::click(),
                            );
                            let rad = gestalt::radius(MASSE.radius_wahl);
                            if gew {
                                ui.painter().rect_filled(rect, rad, r.wahl);
                            } else if k.hovered() {
                                ui.painter()
                                    .rect_filled(rect, rad, r.wahl.gamma_multiply(0.5));
                            }
                            let farbe = sitzung_farbe(r, &s);
                            symbole::punkt(
                                ui,
                                pos2(rect.left() + 10.0 + MASSE.punkt / 2.0, rect.center().y),
                                farbe,
                                s.laeuft,
                            );
                            text_gekuerzt(
                                ui,
                                pos2(
                                    rect.left() + 26.0,
                                    rect.center().y - MASSE.schrift_grund * 0.62,
                                ),
                                &text,
                                FontId::proportional(MASSE.schrift_grund),
                                r.schrift,
                                rect.width() - 26.0 - 8.0,
                            );
                            k.widget_info(|| {
                                egui::WidgetInfo::selected(Role::ListBoxOption, true, gew, &text)
                            });
                            ui.ctx()
                                .accesskit_node_builder(k.id, |n| n.set_label(text.clone()));
                            self.modal_halte_neu.push(k.id);
                            if k.clicked() {
                                f.gewaehlt = Some(id.clone());
                            }
                        }
                    }
                });
            });
    }

    fn startformular(&mut self, ui: &mut Ui, r: &Rollen, f: &mut Sitzungsfenster) {
        ui.scope_builder(UiBuilder::new().id_salt("startformular"), |ui| {
            behaelter(ui, Role::Form, "Sitzung starten", None);
            let feld = |ui: &mut Ui,
                        name: &str,
                        erste: bool,
                        text: &mut String,
                        platzhalter: &str,
                        kennung: &str|
             -> Response {
                formzeile(ui, r, name, erste, |ui, breite| {
                    let e = ui.add(
                        egui::TextEdit::singleline(text)
                            .hint_text(platzhalter)
                            .desired_width(breite),
                    );
                    ui.ctx().accesskit_node_builder(e.id, |n| {
                        n.set_label(name.to_owned());
                        n.set_author_id(kennung.to_owned());
                    });
                    e
                })
            };
            // the name of a session is a field of L10; without it the folder is the first field
            let mit_name = self.hat(10);
            formgruppe(ui, r, "Ordner", |ui| {
                if mit_name {
                    let e = feld(ui, "Name", true, &mut f.name, T_OPTIONAL, "name-eingabe");
                    self.modal_halte_neu.push(e.id);
                    self.fokus_nehmen(&e, &FokusZiel::Name);
                }
                let mut ordner = self.kern.start.arbeitsordner.clone();
                let e = feld(
                    ui,
                    "Arbeitsordner",
                    !mit_name,
                    &mut ordner,
                    T_PFAD,
                    "arbeitsordner-eingabe",
                );
                self.modal_halte_neu.push(e.id);
                if !mit_name {
                    self.fokus_nehmen(&e, &FokusZiel::Name);
                }
                self.kern.start.arbeitsordner = ordner;
            });
            let wahl = |ui: &mut Ui,
                        name: &str,
                        erste: bool,
                        feld: &ansicht::start::Wahlfeld,
                        kennung: &str|
             -> (Option<String>, Vec<Id>) {
                // segments or a drop-down by the number of entries, like every choice of the window
                let wahlen: Vec<(String, String)> = feld
                    .eintraege
                    .iter()
                    .map(|e| (e.wert.clone(), e.text.clone()))
                    .collect();
                formzeile(ui, r, name, erste, |ui, breite| {
                    wahl_malen(ui, r, kennung, name, &wahlen, &feld.wert, breite)
                })
            };
            // B69: model and effort at the start; the base form is the model, the effort comes with L10
            let mit_gruppe = self.zeigt("B69");
            formgruppe(ui, r, "Programm und Modell", |ui| {
                ui.scope_builder(UiBuilder::new().id_salt("modell-effort"), |ui| {
                    if mit_gruppe {
                        behaelter(ui, Role::Group, "Modell und Effort", Some("B69"));
                    }
                    let (h, ids) = wahl(
                        ui,
                        T_PROGRAMM,
                        true,
                        &self.kern.start.harness.clone(),
                        "harness-wahl",
                    );
                    self.modal_halte_neu.extend(ids);
                    if let Some(h) = h {
                        self.kern.start.harness.wert = h;
                    }
                    let (m, ids) = wahl(
                        ui,
                        "Modell",
                        false,
                        &self.kern.start.modell.clone(),
                        "modell-wahl",
                    );
                    self.modal_halte_neu.extend(ids);
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
                        let wahlen: Vec<(String, String)> = erlaubt
                            .iter()
                            .map(|e| (e.clone(), effort_wort(e)))
                            .collect();
                        let (neu, ids) = formzeile(ui, r, T_DENKSTUFE, false, |ui, breite| {
                            wahl_malen(
                                ui,
                                r,
                                "effort-wahl",
                                T_DENKSTUFE,
                                &wahlen,
                                &f.effort,
                                breite,
                            )
                        });
                        if let Some(e) = neu {
                            f.effort = e;
                        }
                        self.modal_halte_neu.extend(ids);
                    }
                    let (ro, ids) = wahl(
                        ui,
                        "Route",
                        false,
                        &self.kern.start.route.clone(),
                        "route-wahl",
                    );
                    self.modal_halte_neu.extend(ids);
                    if let Some(ro) = ro {
                        self.kern.start.route_waehlen(&ro);
                    }
                });
            });
            formgruppe(ui, r, "Kunde", |ui| {
                let (k, ids) = wahl(
                    ui,
                    "Kunde",
                    true,
                    &self.kern.start.kunde.clone(),
                    "kunde-wahl",
                );
                self.modal_halte_neu.extend(ids);
                if let Some(k) = k {
                    self.kern.start.kunde.wert = k;
                }
            });
            ui.add_space(10.0);
            // "Starten" on the right with its text in the middle, like today and the browser view (F1)
            let k = ui
                .allocate_ui_with_layout(
                    vec2(ui.available_width(), MASSE.formfeld + 4.0),
                    Layout::right_to_left(Align::Center),
                    |ui| {
                        ui.spacing_mut().button_padding = vec2(14.0, 5.0);
                        ui.add(
                            egui::Button::new(
                                RichText::new("Starten").fett().color(r.akzent_tinte),
                            )
                            .fill(r.akzent_flaeche)
                            .stroke(Stroke::NONE)
                            .corner_radius(gestalt::radius(MASSE.radius_knopf)),
                        )
                    },
                )
                .inner;
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

    /// Whether the service sent a value for a row of page `s` (a page without values stays away, OF8 point 1).
    fn seite_hat_werte(&self, s: Seite) -> bool {
        s == Seite::Aussehen
            || (matches!(s, Seite::Aufsicht | Seite::AdminAufsicht)
                && (self.zusatz.schutz.is_some() || !self.zusatz.fristen.is_empty()))
            || s.gruppen().iter().any(|(_, zeilen)| {
                zeilen
                    .iter()
                    .any(|(k, ..)| self.zusatz.einstellung(k).is_some())
            })
    }

    fn einstellungen_fenster(&mut self, ui: &mut Ui, r: &Rollen) {
        use crate::einstellungstexte::{BEREICH_ADMIN, BEREICH_NUTZER, SCHLIESSEN};
        let Some(mut seite) = self.b.einstellungen else {
            return;
        };
        if !self.zeigt("B72") {
            self.b.einstellungen = None;
            return;
        }
        // without L4 the base form has only the page of the window's own choices (OF8 point 3); the admin area only
        // for an admin, which the service decides (OF9 point 2)
        let mit_l4 = self.hat(4);
        let nutzer: Vec<Seite> = Seite::NUTZER
            .into_iter()
            .filter(|s| *s == Seite::Aussehen || (mit_l4 && self.seite_hat_werte(*s)))
            .collect();
        let admin: Vec<Seite> = if mit_l4 && self.zusatz.admin && self.zeigt("B78") {
            Seite::ADMIN
                .into_iter()
                .filter(|s| self.seite_hat_werte(*s))
                .collect()
        } else {
            Vec::new()
        };
        if !nutzer.contains(&seite) && !admin.contains(&seite) {
            seite = Seite::Aussehen;
        }
        let ctx = ui.ctx().clone();
        let mut offen = true;
        let platz = ctx.content_rect();
        let lage = einstellungen_lage(platz);
        // the ground behind the dialog is dimmed like the browser view (`.fenster-grund`) and takes the pointer
        egui::Area::new(Id::unique("einstellungen-grund"))
            .order(egui::Order::Middle)
            .fixed_pos(platz.min)
            .show(&ctx, |ui| {
                let (rect, _) = ui.allocate_exact_size(platz.size(), Sense::hover());
                ui.painter()
                    .rect_filled(rect, 0.0, Color32::from_black_alpha(GRUND_DECKUNG));
            });
        egui::Window::new("Einstellungen")
            .id(Id::unique("einstellungen-fenster"))
            .order(egui::Order::Foreground)
            .title_bar(false)
            .collapsible(false)
            .resizable(false)
            .fixed_pos(lage.min)
            .fixed_size(lage.size())
            .frame(
                Frame::window(&ctx.global_style())
                    .fill(r.grund)
                    .inner_margin(Margin::ZERO)
                    .corner_radius(gestalt::radius(MASSE.radius_fenster)),
            )
            .show(&ctx, |ui| {
                ui.set_min_size(lage.size());
                let voll = Rect::from_min_size(ui.max_rect().min, lage.size());
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
                ui.painter().vline(
                    leiste.right(),
                    leiste.y_range(),
                    Stroke::new(MASSE.linie, r.linie),
                );
                // the list of pages: the user area, then apart the admin area; only the open page is marked (OF9 point 1)
                let mut lu = ui.new_child(
                    UiBuilder::new()
                        .max_rect(leiste.shrink2(vec2(0.0, MASSE.innen)))
                        .layout(Layout::top_down(Align::Min)),
                );
                lu.add_space(MASSE.innen);
                lu.scope_builder(UiBuilder::new().id_salt("einstellungen-nutzer"), |ui| {
                    behaelter(ui, Role::Navigation, "Einstellungen", Some("B72"));
                    ui.ctx().accesskit_node_builder(ui.unique_id(), |n| {
                        n.set_class_name("einstellungen");
                    });
                    Self::seitengruppe(ui, r, BEREICH_NUTZER, false);
                    for s in &nutzer {
                        if Self::seitenzeile(ui, r, *s, seite == *s) {
                            seite = *s;
                        }
                    }
                });
                if !admin.is_empty() {
                    lu.add_space(MASSE.gruppenabstand);
                    let y = lu.cursor().top();
                    lu.painter().hline(
                        lu.max_rect().x_range().shrink(MASSE.innen),
                        y,
                        Stroke::new(MASSE.linie, r.linie),
                    );
                    lu.add_space(MASSE.gruppenabstand);
                    lu.scope_builder(UiBuilder::new().id_salt("einstellungen-admin"), |ui| {
                        behaelter(ui, Role::Navigation, BEREICH_ADMIN, Some("B78"));
                        ui.ctx().accesskit_node_builder(ui.unique_id(), |n| {
                            n.set_class_name("admin-einstellungen");
                        });
                        Self::seitengruppe(ui, r, BEREICH_ADMIN, true);
                        for s in &admin {
                            if Self::seitenzeile(ui, r, *s, seite == *s) {
                                seite = *s;
                            }
                        }
                    });
                }
                // the page: the head with symbol, title and short description (OF9 point 6), then the groups
                let inhalt = Rect::from_min_max(pos2(leiste.right(), voll.top()), voll.max)
                    .shrink2(vec2(MASSE.einstellungsrand, MASSE.innen * 1.8));
                let mut cu = ui.new_child(
                    UiBuilder::new()
                        .max_rect(inhalt)
                        .layout(Layout::top_down(Align::Min)),
                );
                let (titel, kurz, symbol) = seite.kopf();
                let (kopf, _) = cu.allocate_exact_size(
                    vec2(inhalt.width(), MASSE.einstellungskopf),
                    Sense::hover(),
                );
                let feld = Rect::from_min_size(kopf.min, Vec2::splat(MASSE.einstellungskopf));
                cu.painter().rect_filled(
                    feld,
                    gestalt::radius(MASSE.radius_gruppe - 2.0),
                    if seite.admin() {
                        ADMIN_FARBE
                    } else {
                        r.akzent_flaeche
                    },
                );
                symbole::malen(
                    &cu,
                    symbol,
                    feld.shrink(MASSE.einstellungskopf * 0.27),
                    r.akzent_tinte,
                );
                let x = feld.right() + MASSE.abstand * 2.0;
                let t = cu.painter().layout_no_wrap(
                    titel.to_owned(),
                    FontId::new(
                        MASSE.schrift_seite,
                        egui::FontFamily::Name(crate::FETT.into()),
                    ),
                    r.schrift,
                );
                let tb = t.size().x;
                cu.painter().galley(pos2(x, kopf.top() + 1.0), t, r.schrift);
                if seite.admin() {
                    Self::marke_admin(&cu, r, pos2(x + tb + MASSE.abstand, kopf.top() + 4.0));
                }
                text_gekuerzt(
                    &cu,
                    pos2(x, kopf.top() + MASSE.schrift_seite + 6.0),
                    kurz,
                    FontId::proportional(MASSE.schrift_grund),
                    r.gedaempft,
                    kopf.right() - x - MASSE.kapsel_segment - MASSE.innen,
                );
                let mut zu = cu.new_child(
                    UiBuilder::new()
                        .max_rect(Rect::from_min_size(
                            pos2(kopf.right() - MASSE.kapsel_segment, kopf.top()),
                            Vec2::splat(MASSE.kapsel_segment),
                        ))
                        .layout(Layout::left_to_right(Align::Min)),
                );
                let k = symbole::knopf(
                    &mut zu,
                    Symbol::Schliessen,
                    SCHLIESSEN,
                    MASSE.kapsel_segment,
                    false,
                );
                if k.clicked() {
                    offen = false;
                }
                cu.add_space(MASSE.abstand);
                let seite_rect =
                    Rect::from_min_max(pos2(inhalt.left(), cu.cursor().top()), inhalt.max);
                let mut su = cu.new_child(
                    UiBuilder::new()
                        .max_rect(seite_rect)
                        .layout(Layout::top_down(Align::Min)),
                );
                egui::ScrollArea::vertical()
                    .auto_shrink(false)
                    .show(&mut su, |ui| {
                        // the page as a container of its own, so that its controls can be told apart from the list
                        ui.scope_builder(UiBuilder::new().id_salt("einstellungsseite"), |ui| {
                            behaelter(ui, Role::Group, kurz, Some("einstellungsseite"));
                            self.einstellungsseite(ui, r, seite);
                        });
                    });
            });
        if offen && !ctx.input(|i| i.key_pressed(Key::Escape)) {
            self.b.einstellungen = Some(seite);
        } else {
            self.b.einstellungen = None;
        }
    }

    /// The small heading of an area of pages ("Für dich", "Verwaltung" with the badge "Admin").
    fn seitengruppe(ui: &mut Ui, r: &Rollen, text: &str, admin: bool) {
        let (rect, _) = ui.allocate_exact_size(
            vec2(ui.available_width(), MASSE.schrift_klein + 10.0),
            Sense::hover(),
        );
        let g = ui.painter().layout_no_wrap(
            text.to_owned(),
            FontId::new(
                MASSE.schrift_klein,
                egui::FontFamily::Name(crate::FETT.into()),
            ),
            r.gedaempft,
        );
        let breite = g.size().x;
        let p = pos2(rect.left() + MASSE.kopf_einzug + MASSE.innen, rect.top());
        ui.painter().galley(p, g, r.gedaempft);
        if admin {
            Self::marke_admin(ui, r, pos2(p.x + breite + MASSE.abstand, rect.top() - 2.0));
        }
        ui.add_space(2.0);
    }

    /// The badge "Admin" at `p` (top left), like the browser view (`.marke.admin`).
    fn marke_admin(ui: &Ui, r: &Rollen, p: Pos2) {
        let g = ui.painter().layout_no_wrap(
            crate::einstellungstexte::MARKE_ADMIN.to_owned(),
            FontId::new(
                MASSE.schrift_klein,
                egui::FontFamily::Name(crate::FETT.into()),
            ),
            r.akzent_text,
        );
        let rect = Rect::from_min_size(p, g.size() + vec2(14.0, 4.0));
        ui.painter().rect_filled(
            rect,
            gestalt::radius(rect.height() / 2.0),
            r.akzent.gamma_multiply(0.22),
        );
        ui.painter().galley(p + vec2(7.0, 2.0), g, r.akzent_text);
    }

    /// One row of the list of pages: symbol and name; only the open page is on grey (OF9 point 1 and 6).
    fn seitenzeile(ui: &mut Ui, r: &Rollen, s: Seite, gewaehlt: bool) -> bool {
        let (text, _, sym) = s.kopf();
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
        let q = Rect::from_center_size(
            pos2(feld.left() + 11.0 + 9.0, feld.center().y),
            Vec2::splat(16.0),
        );
        symbole::malen(ui, sym, q, r.akzent_text);
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
        let kennung = s.kennung();
        ui.ctx()
            .accesskit_node_builder(resp.id, |n| n.set_author_id(kennung.to_owned()));
        resp.clicked()
    }

    /// A group of settings: heading, then the rows on a raised ground with lines between them (OF9 point 6).
    fn gruppe(
        &mut self,
        ui: &mut Ui,
        r: &Rollen,
        titel: &str,
        zeilen: &[Einstellungszeile],
        admin: bool,
    ) {
        ui.add_space(MASSE.gruppenabstand * 1.5);
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(titel)
                    .fett()
                    .size(MASSE.schrift_grund)
                    .color(r.schrift),
            );
        });
        ui.add_space(MASSE.gruppe_kopf - 8.0);
        Frame::new()
            .fill(r.erhoben)
            .corner_radius(gestalt::radius(MASSE.radius_gruppe))
            .inner_margin(Margin::symmetric(12, 2))
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
                    self.einstellung_zeile(ui, r, z, admin);
                }
            });
    }

    fn einstellung_zeile(&mut self, ui: &mut Ui, r: &Rollen, z: &Einstellungszeile, admin: bool) {
        use crate::einstellungstexte::HERKUNFT_GERAET;
        // the look is a choice of this device and works at once (base form of B72, L8)
        let lokal = z.schluessel.starts_with("aussehen.") && !self.hat(4);
        let e = self.zusatz.einstellung(z.schluessel).cloned();
        let (wert, herkunft, aenderbar, grund) = if lokal {
            (
                Value::String(self.lokal_wert(z.schluessel)),
                HERKUNFT_GERAET.to_owned(),
                true,
                String::new(),
            )
        } else {
            let wert = e.as_ref().map_or(Value::Null, |x| x.wert.clone());
            let herkunft = e
                .as_ref()
                .map(|x| herkunft_anzeige(&x.herkunft))
                .unwrap_or_default();
            let aenderbar = e.as_ref().is_some_and(|x| x.aenderbar);
            let grund = grund_anzeige(
                e.as_ref().and_then(|x| x.grund.as_deref()),
                aenderbar,
                &herkunft,
            );
            (wert, herkunft, aenderbar, grund)
        };
        let hoehe = if grund.is_empty() { 50.0 } else { 62.0 };
        let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), hoehe), Sense::hover());
        // left: the symbol of the page; right: the switch or the choice, before it the origin; between them name, one
        // line of description and the reason of a lock (OF9 point 6)
        let sym = Rect::from_min_size(pos2(rect.left(), rect.center().y - 13.0), Vec2::splat(26.0));
        ui.painter()
            .rect_filled(sym, gestalt::radius(MASSE.radius_knopf), r.kopf);
        // the symbol of the row, like the browser view: the colour set as two swatches, the terminal, the font as "Aa",
        // every other row with the symbol of its page
        match z.schluessel {
            "aussehen.farbsatz" => {
                for (i, c) in [gestalt::GRAU_DUNKEL.leiste, gestalt::BLAUSCHWARZ.erhoben]
                    .into_iter()
                    .enumerate()
                {
                    let dx = if i == 0 { -3.5 } else { 3.5 };
                    let m = sym.center() + vec2(dx, 0.0);
                    ui.painter().circle_filled(m, 5.0, c);
                    ui.painter()
                        .circle_stroke(m, 5.0, Stroke::new(1.0, r.gedaempft));
                }
            }
            "aussehen.terminal" => {
                symbole::malen(ui, Symbol::Terminal, sym.shrink(6.0), r.akzent_text);
            }
            crate::einstellungstexte::SCHLUESSEL_SCHRIFTGROESSE => {
                ui.painter().text(
                    sym.center(),
                    Align2::CENTER_CENTER,
                    "Aa",
                    FontId::new(
                        MASSE.schrift_grund,
                        egui::FontFamily::Name(crate::FETT.into()),
                    ),
                    r.akzent_text,
                );
            }
            _ => {
                let seite = self.b.einstellungen.unwrap_or(Seite::Aussehen);
                symbole::malen(ui, seite.kopf().2, sym.shrink(6.0), r.akzent_text);
            }
        }
        let band = Rect::from_center_size(rect.center(), vec2(rect.width(), MASSE.formfeld));
        let mut rechts = ui.new_child(
            UiBuilder::new()
                .max_rect(band)
                .layout(Layout::right_to_left(Align::Center)),
        );
        let wert_t = wert_text(&wert);
        let ebene = if admin { "firma" } else { "nutzer" };
        rechts.add_enabled_ui(aenderbar, |ui| {
            if let Value::Bool(an) = wert
                && z.wahlen.is_empty()
            {
                let k = schalter_malen(ui, r, an, z.name);
                // the key of the row on its control, so a test can tie every control to the catalog
                let s = z.schluessel.to_owned();
                ui.ctx()
                    .accesskit_node_builder(k.id, |n| n.set_author_id(s));
                if k.clicked() {
                    self.einstellung_senden(z.schluessel, &json!(!an), ebene);
                }
            } else if z.wahlen.is_empty() {
                let breite = (band.width() * 0.42).max(80.0);
                let t = wert_anzeige(&wert);
                ui.allocate_ui(vec2(breite, MASSE.formfeld), |ui| {
                    ui.add(egui::Label::new(RichText::new(t).fett()).truncate());
                });
            } else {
                let (neu, _) = wahl_malen(
                    ui,
                    r,
                    z.schluessel,
                    z.name,
                    &z.wahlen,
                    &wert_t,
                    AUSKLAPPER_BREITE,
                );
                if let Some(w) = neu {
                    if lokal {
                        self.lokal_setzen(z.schluessel, &w);
                    } else {
                        // the first choice of the thinking level, „Vorgabe des Programms“, is `null`
                        let v: Value = if w.is_empty() {
                            Value::Null
                        } else {
                            serde_json::from_str(&w).unwrap_or_else(|_| json!(w))
                        };
                        self.einstellung_senden(z.schluessel, &v, ebene);
                    }
                }
            }
        });
        rechts.add_space(MASSE.abstand * 2.0);
        if !herkunft.is_empty() {
            Frame::new()
                .fill(r.kopf)
                .corner_radius(gestalt::radius(MASSE.radius_kapsel))
                .inner_margin(Margin::symmetric(7, 2))
                .show(&mut rechts, |ui| {
                    ui.label(
                        RichText::new(&herkunft)
                            .size(MASSE.schrift_klein)
                            .color(r.gedaempft),
                    );
                });
        }
        let links_ende = rechts.min_rect().left() - MASSE.innen;
        let x = sym.right() + MASSE.innen + 2.0;
        let breite = links_ende - x;
        let mut y = rect.top() + if grund.is_empty() { 8.0 } else { 7.0 };
        text_gekuerzt(
            ui,
            pos2(x, y),
            z.name,
            FontId::proportional(MASSE.schrift_grund),
            if aenderbar { r.schrift } else { r.gedaempft },
            breite,
        );
        y += 18.0;
        text_gekuerzt(
            ui,
            pos2(x, y),
            z.beschreibung,
            FontId::proportional(MASSE.schrift_klein),
            r.gedaempft,
            breite,
        );
        if !grund.is_empty() {
            text_gekuerzt(
                ui,
                pos2(x, y + 15.0),
                &grund,
                FontId::proportional(MASSE.schrift_klein),
                r.gedaempft,
                breite,
            );
        }
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
            crate::einstellungstexte::SCHLUESSEL_SCHRIFTGROESSE => {
                format!("{}", self.terminalschrift.round())
            }
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
            (crate::einstellungstexte::SCHLUESSEL_SCHRIFTGROESSE, g) => {
                if let Ok(g) = g.parse::<f32>()
                    && (8.0..=32.0).contains(&g)
                {
                    self.terminalschrift = g;
                }
            }
            _ => {}
        }
    }

    /// `EinstellungSetzen` (`regelebenen` 15.9): the area of the key from the catalog of the view core, the level of the
    /// user in the user area and of the company in the admin area; whether it holds decides the service.
    fn einstellung_senden(&mut self, schluessel: &str, wert: &Value, ebene: &str) {
        let bereich = ansicht::ziel::KATALOG
            .iter()
            .find(|(k, ..)| *k == schluessel)
            .map_or("admin", |(_, b, _)| *b);
        self.senden(
            &json!({"art": "EinstellungSetzen", "schluessel": schluessel, "wert": wert,
            "bereich": bereich, "ebene": ebene}),
        );
    }

    fn einstellungsseite(&mut self, ui: &mut Ui, r: &Rollen, seite: Seite) {
        use crate::einstellungstexte::{KEINE_WERTE, aussehen as a, gruppe as g};
        let z = |(schluessel, name, beschreibung): crate::einstellungstexte::Zeile,
                 wahlen: Vec<(String, String)>| Einstellungszeile {
            schluessel,
            name,
            beschreibung,
            wahlen,
        };
        let paare = |l: &[(&str, &str)]| -> Vec<(String, String)> {
            l.iter()
                .map(|(w, t)| ((*w).to_owned(), (*t).to_owned()))
                .collect()
        };
        if seite == Seite::Aussehen {
            // the words of the choices come from the view core of the browser view (`ansicht::aussehen`); the window
            // writes the terminal like the theme as `wie_thema` (L8)
            let terminals: Vec<(String, String)> = ansicht::aussehen::TERMINALS
                .iter()
                .map(|(w, t)| {
                    let w = if *w == "thema" { "wie_thema" } else { w };
                    (w.to_owned(), (*t).to_owned())
                })
                .collect();
            if self.zeigt("B77") {
                ui.scope_builder(UiBuilder::new().id_salt("farbwahl"), |ui| {
                    behaelter(ui, Role::Group, "Farbwahl", Some("B77"));
                    self.gruppe(
                        ui,
                        r,
                        g::FARBEN,
                        &[
                            z(a::THEMA, paare(&ansicht::aussehen::THEMEN)),
                            z(a::FARBSATZ, paare(&ansicht::aussehen::FARBSAETZE)),
                            z(a::TERMINAL, terminals),
                        ],
                        false,
                    );
                });
            }
            // the size with its unit like in the browser view: „13 Punkt“
            let groessen = ansicht::aussehen::SCHRIFTGROESSEN
                .iter()
                .map(|g| {
                    (
                        g.to_string(),
                        format!("{g} {}", crate::einstellungstexte::wahl::PUNKT),
                    )
                })
                .collect();
            self.gruppe(ui, r, g::SCHRIFT, &[z(a::SCHRIFTGROESSE, groessen)], false);
            return;
        }
        let mut leer = true;
        for (titel, zeilen) in seite.gruppen() {
            let zeilen: Vec<Einstellungszeile> = zeilen
                .into_iter()
                .filter(|(k, ..)| self.zusatz.einstellung(k).is_some())
                .map(|zeile| {
                    let wahlen = match zeile.0 {
                        "sitzung.harness" | "sitzung.modell" => {
                            sitzung_wahlen(zeile.0, &self.zusatz)
                        }
                        "sitzung.effort" => denkstufen(
                            &self.kern.start.efforts,
                            self.zusatz
                                .einstellung("sitzung.harness")
                                .and_then(|e| e.wert.as_str()),
                        ),
                        // the fixed choices of the shared words; 24 h is the limit of contract 2 (15.1.2)
                        "aufsicht.freigabefrist_ms" => frist_wahlen(
                            &ansicht::woerter::werte::FREIGABEFRIST_WAHLEN,
                            &self
                                .zusatz
                                .einstellung(zeile.0)
                                .map_or(Value::Null, |e| e.wert.clone()),
                        ),
                        "aufsicht.einloesefrist_ms" => frist_wahlen(
                            &ansicht::woerter::werte::EINLOESEFRIST_WAHLEN,
                            &self
                                .zusatz
                                .einstellung(zeile.0)
                                .map_or(Value::Null, |e| e.wert.clone()),
                        ),
                        _ => Vec::new(),
                    };
                    z(zeile, wahlen)
                })
                .collect();
            if zeilen.is_empty() {
                continue;
            }
            leer = false;
            self.gruppe(ui, r, titel, &zeilen, seite.admin());
        }
        if matches!(seite, Seite::Aufsicht | Seite::AdminAufsicht) {
            let zeilen = fristen_tabelle(&self.zusatz.fristen);
            if !zeilen.is_empty() {
                leer = false;
                Self::fristen_gruppe(ui, r, &zeilen);
            }
            if let Some(s) = self.zusatz.schutz.clone() {
                leer = false;
                Self::schutz_gruppe(ui, r, &s);
            }
        }
        if leer {
            ui.add_space(MASSE.gruppenabstand * 2.0);
            ui.label(RichText::new(KEINE_WERTE).color(r.gedaempft));
        }
    }

    /// The deadlines per MCP server as one table to read, like in the browser view (decision of the operator, 28.11.2026):
    /// the heading, a row of column heads (server, deadline of the decision, deadline to redeem), then one row per server
    /// with each deadline and its origin; no control.
    fn fristen_gruppe(ui: &mut Ui, r: &Rollen, zeilen: &[(String, [Fristzelle; 2])]) {
        use crate::einstellungstexte::{server as t, werte};
        ui.add_space(MASSE.gruppenabstand * 1.5);
        ui.scope_builder(UiBuilder::new().id_salt("fristen-je-server"), |ui| {
            ui.label(
                RichText::new(t::TABELLE)
                    .fett()
                    .size(MASSE.schrift_grund)
                    .color(r.schrift),
            );
            ui.add_space(MASSE.gruppe_kopf - 8.0);
            Frame::new()
                .fill(r.erhoben)
                .corner_radius(gestalt::radius(MASSE.radius_gruppe))
                .inner_margin(Margin::symmetric(12, 2))
                .show(ui, |ui| {
                    // egui gives every inner Ui a node of its own: the table is the Ui inside the frame, so its rows
                    // are its children
                    behaelter(ui, Role::Table, t::TABELLE, Some("fristen-je-server"));
                    ui.set_width(ui.available_width());
                    ui.spacing_mut().item_spacing.y = 0.0;
                    let kopf: Vec<Fristzelle> =
                        [t::SERVER, werte::FREIGABEFRIST.1, werte::EINLOESEFRIST.1]
                            .iter()
                            .map(|k| Some(((*k).to_owned(), String::new())))
                            .collect();
                    Self::fristen_zeile(ui, r, 0, &kopf);
                    for (i, (server, zellen)) in zeilen.iter().enumerate() {
                        let y = ui.cursor().top();
                        ui.painter().hline(
                            ui.max_rect().x_range(),
                            y,
                            Stroke::new(MASSE.linie, r.linie),
                        );
                        let mut z: Vec<Fristzelle> = vec![Some((server.clone(), String::new()))];
                        z.extend(zellen.iter().cloned());
                        Self::fristen_zeile(ui, r, i + 1, &z);
                    }
                });
        });
    }

    /// One row of the table of deadlines; row 0 holds the column heads. A cell is a text and an origin (empty: none);
    /// a missing deadline shows „–“ like the browser view.
    fn fristen_zeile(ui: &mut Ui, r: &Rollen, nr: usize, zellen: &[Fristzelle]) {
        use crate::einstellungstexte::wahl::TRENNER;
        let kopf = nr == 0;
        let hoehe = if kopf { 30.0 } else { 40.0 };
        ui.scope_builder(UiBuilder::new().id_salt(("fristen-zeile", nr)), |ui| {
            behaelter(ui, Role::Row, "", None);
            let (rect, _) =
                ui.allocate_exact_size(vec2(ui.available_width(), hoehe), Sense::hover());
            // the server takes 30 % of the width, each deadline 35 %
            let mut x = rect.left();
            for (j, zelle) in zellen.iter().enumerate() {
                let breite = rect.width() * if j == 0 { 0.30 } else { 0.35 };
                let zr = Rect::from_min_size(pos2(x, rect.top()), vec2(breite, hoehe));
                x += breite;
                let (text, herkunft) = zelle
                    .clone()
                    .unwrap_or_else(|| ("–".to_owned(), String::new()));
                let rolle = match (kopf, j) {
                    (true, _) => Role::ColumnHeader,
                    (false, 0) => Role::RowHeader,
                    _ => Role::Cell,
                };
                let name = if herkunft.is_empty() {
                    text.clone()
                } else {
                    format!("{text}{TRENNER}{herkunft}")
                };
                let zu = ui.new_child(UiBuilder::new().id_salt(("fristen-zelle", j)).max_rect(zr));
                behaelter(&zu, rolle, &name, None);
                let p = ui.painter();
                if kopf || j == 0 {
                    let (groesse, farbe) = if kopf {
                        (MASSE.schrift_klein, r.gedaempft)
                    } else {
                        (MASSE.schrift_grund, r.schrift)
                    };
                    text_gekuerzt(
                        ui,
                        pos2(zr.left(), zr.center().y - groesse * 0.62),
                        &text,
                        FontId::proportional(groesse),
                        farbe,
                        breite - MASSE.innen,
                    );
                    continue;
                }
                // the deadline in bold, the origin after it as a chip like in the rows of the settings
                let wert = p.text(
                    pos2(zr.left(), zr.center().y),
                    Align2::LEFT_CENTER,
                    &text,
                    FontId::new(
                        MASSE.schrift_grund,
                        egui::FontFamily::Name(crate::FETT.into()),
                    ),
                    r.schrift,
                );
                if !herkunft.is_empty() {
                    let g = p.layout_no_wrap(
                        herkunft,
                        FontId::proportional(MASSE.schrift_klein),
                        r.gedaempft,
                    );
                    let chip = Rect::from_min_size(
                        pos2(wert.right() + 8.0, zr.center().y - g.size().y / 2.0 - 2.0),
                        g.size() + vec2(14.0, 4.0),
                    );
                    p.rect_filled(chip, gestalt::radius(MASSE.radius_kapsel), r.kopf);
                    p.galley(chip.min + vec2(7.0, 2.0), g, r.gedaempft);
                }
            }
        });
    }

    /// The effective state of the protection layers, read only: a group like the others, but every row has a value and
    /// no control (decision of the operator, 27.11.2026).
    fn schutz_gruppe(ui: &mut Ui, r: &Rollen, s: &crate::ziel::Schutz) {
        let zeilen = schutz_zeilen(s);
        Self::lesegruppe(
            ui,
            r,
            "schutzschichten",
            crate::einstellungstexte::schutz::GRUPPE,
            &zeilen,
        );
    }

    /// A group of information: a heading and rows of name, value and origin, read only, without a control.
    fn lesegruppe(
        ui: &mut Ui,
        r: &Rollen,
        id: &str,
        titel: &str,
        zeilen: &[(String, String, String)],
    ) {
        ui.add_space(MASSE.gruppenabstand * 1.5);
        ui.scope_builder(UiBuilder::new().id_salt(id), |ui| {
            behaelter(ui, Role::Group, titel, None);
            ui.label(
                RichText::new(titel)
                    .fett()
                    .size(MASSE.schrift_grund)
                    .color(r.schrift),
            );
            ui.add_space(MASSE.gruppe_kopf - 8.0);
            Frame::new()
                .fill(r.erhoben)
                .corner_radius(gestalt::radius(MASSE.radius_gruppe))
                .inner_margin(Margin::symmetric(12, 2))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    for (i, (name, wert, herkunft)) in zeilen.iter().enumerate() {
                        if i > 0 {
                            let y = ui.cursor().top();
                            ui.painter().hline(
                                ui.max_rect().x_range(),
                                y,
                                Stroke::new(MASSE.linie, r.linie),
                            );
                        }
                        Self::schutz_zeile(ui, r, name, wert, herkunft);
                    }
                });
        });
    }

    /// One row of the information: symbol, name, and on the right the value with its origin; no control.
    fn schutz_zeile(ui: &mut Ui, r: &Rollen, name: &str, wert: &str, herkunft: &str) {
        let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 40.0), Sense::hover());
        let sym = Rect::from_min_size(pos2(rect.left(), rect.center().y - 13.0), Vec2::splat(26.0));
        ui.painter()
            .rect_filled(sym, gestalt::radius(MASSE.radius_knopf), r.kopf);
        symbole::malen(ui, Symbol::Schild, sym.shrink(6.0), r.akzent_text);
        let band = Rect::from_center_size(rect.center(), vec2(rect.width(), MASSE.formfeld));
        let mut rechts = ui.new_child(
            UiBuilder::new()
                .max_rect(band)
                .layout(Layout::right_to_left(Align::Center)),
        );
        let l = rechts.label(RichText::new(wert).fett().color(r.schrift));
        let beschriftung = format!("{name}: {wert}");
        l.widget_info(|| egui::WidgetInfo::labeled(Role::Label, true, &beschriftung));
        if !herkunft.is_empty() {
            rechts.add_space(MASSE.abstand * 2.0);
            Frame::new()
                .fill(r.kopf)
                .corner_radius(gestalt::radius(MASSE.radius_kapsel))
                .inner_margin(Margin::symmetric(7, 2))
                .show(&mut rechts, |ui| {
                    ui.label(
                        RichText::new(herkunft)
                            .size(MASSE.schrift_klein)
                            .color(r.gedaempft),
                    );
                });
        }
        let x = sym.right() + MASSE.innen + 2.0;
        text_gekuerzt(
            ui,
            pos2(x, rect.center().y - MASSE.schrift_grund * 0.62),
            name,
            FontId::proportional(MASSE.schrift_grund),
            r.schrift,
            rechts.min_rect().left() - MASSE.innen - x,
        );
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ziel::Schutz;

    fn sitzung(id: &str, laeuft: bool, code: &str) -> Sitzung {
        Sitzung {
            id: id.to_owned(),
            harness: "claude".to_owned(),
            modell: "opus".to_owned(),
            route: "cloud".to_owned(),
            kunde: String::new(),
            laeuft,
            code: code.to_owned(),
            nr: None,
            herkunft: None,
        }
    }

    fn mit_grenze(harnesses: Option<Value>, modelle: Option<Value>) -> Zusatz {
        let e = |schluessel: &str, wert: Value| crate::ziel::Einstellung {
            schluessel: schluessel.to_owned(),
            wert,
            herkunft: "firma".to_owned(),
            bereich: "admin".to_owned(),
            aenderbar: false,
            grund: None,
        };
        let mut einstellungen = vec![
            e("sitzung.harness", json!("claude")),
            e("sitzung.modell", json!("opus")),
        ];
        einstellungen.extend(harnesses.map(|w| e("harnesses", w)));
        einstellungen.extend(modelle.map(|w| e("modelle", w)));
        Zusatz {
            einstellungen,
            ..Zusatz::default()
        }
    }

    fn wahlwerte(wahlen: &[(String, String)]) -> Vec<&str> {
        wahlen.iter().map(|(w, _)| w.as_str()).collect()
    }

    #[test]
    fn the_choice_of_a_session_offers_only_programs_the_limit_allows() {
        // decision of the operator, 28.11.2026: without `claude` in `harnesses` the choice has no `claude`
        let z = mit_grenze(Some(json!({"intern": ["copilot", "pi", "opencode"]})), None);
        assert_eq!(
            wahlwerte(&sitzung_wahlen("sitzung.harness", &z)),
            ["copilot", "pi", "opencode"]
        );
        let alle = json!({"intern": ["opencode", "pi", "copilot", "claude"]});
        let z = mit_grenze(Some(alle), None);
        assert_eq!(
            wahlwerte(&sitzung_wahlen("sitzung.harness", &z)),
            ["claude", "copilot", "pi", "opencode"],
            "the order of the catalog"
        );
        let z = mit_grenze(
            Some(json!({"intern": ["copilot"], "kundendaten": ["pi"]})),
            None,
        );
        assert_eq!(
            wahlwerte(&sitzung_wahlen("sitzung.harness", &z)),
            ["copilot", "pi"],
            "a program of at least one class"
        );
        let z = mit_grenze(Some(json!({"intern": []})), None);
        assert_eq!(
            sitzung_wahlen("sitzung.harness", &z),
            Vec::new(),
            "no release"
        );
        // `{}` and a missing key are the default of the contract: no restriction (`regelebenen` 15.1)
        for z in [mit_grenze(Some(json!({})), None), mit_grenze(None, None)] {
            assert_eq!(
                wahlwerte(&sitzung_wahlen("sitzung.harness", &z)),
                ["claude", "copilot", "pi", "opencode"]
            );
        }
    }

    #[test]
    fn the_choice_of_a_model_offers_only_models_the_limit_allows() {
        let z = mit_grenze(None, Some(json!({"intern": ["sonnet"]})));
        assert_eq!(wahlwerte(&sitzung_wahlen("sitzung.modell", &z)), ["sonnet"]);
        let z = mit_grenze(None, Some(json!({"intern": []})));
        assert_eq!(sitzung_wahlen("sitzung.modell", &z), Vec::new());
        let z = mit_grenze(None, None);
        assert_eq!(
            wahlwerte(&sitzung_wahlen("sitzung.modell", &z)),
            ["sonnet", "opus"]
        );
    }

    fn plus(id: &str, ordner: &str) -> crate::ziel::Sitzungsplus {
        crate::ziel::Sitzungsplus {
            id: id.to_owned(),
            name: id.to_uppercase(),
            ordner: ordner.to_owned(),
            projekt: String::new(),
        }
    }

    #[test]
    fn the_known_sessions_stand_by_folder_even_when_the_list_mixes_them() {
        let liste = [
            sitzung("s-1", true, ""),
            sitzung("s-2", false, "0"),
            sitzung("s-3", true, ""),
            sitzung("s-4", false, "1"),
            sitzung("s-5", true, ""),
        ];
        let zusatz = Zusatz {
            sitzungen: vec![
                plus("s-1", "~/AI/a"),
                plus("s-2", "~/AI/b"),
                plus("s-3", "~/AI/a"),
                plus("s-4", "~/AI/b"),
            ],
            ..Zusatz::default()
        };
        let g = nach_ordner(&liste, &zusatz);
        let erwartet: Vec<(String, Vec<String>)> = [
            ("~/AI/a", vec!["s-1", "s-3"]),
            ("~/AI/b", vec!["s-2", "s-4"]),
            ("", vec!["s-5"]),
        ]
        .into_iter()
        .map(|(o, ids)| (o.to_owned(), ids.into_iter().map(str::to_owned).collect()))
        .collect();
        assert_eq!(g, erwartet);
    }

    #[test]
    fn an_ended_opencode_session_is_not_offered_to_continue() {
        let mut s = sitzung("s-4", false, "0");
        assert_eq!(fortsetzen_grund(Some(&s)), "");
        for h in ["copilot", "pi", "claude"] {
            s.harness = h.to_owned();
            assert_eq!(fortsetzen_grund(Some(&s)), "", "{h}");
        }
        s.harness = "opencode".to_owned();
        assert_eq!(fortsetzen_grund(Some(&s)), T_OHNE_FORTSETZEN);
        s.laeuft = true;
        assert_eq!(fortsetzen_grund(Some(&s)), T_LAEUFT);
        assert_eq!(fortsetzen_grund(None), T_KEINE_GEWAEHLT);
    }

    #[test]
    fn a_session_has_one_state_name_from_the_core() {
        for s in [
            sitzung("s-1", true, ""),
            sitzung("s-2", false, "0"),
            sitzung("s-3", false, "137"),
        ] {
            let kern = s.zustand_text();
            assert_eq!(zeile_zusatz(&s, None), kern);
            assert_eq!(zeile_zusatz(&s, Some(2)), format!("2 Worker · {kern}"));
        }
        assert_eq!(
            zeile_zusatz(&sitzung("s-3", false, "137"), None),
            "beendet · Code 137"
        );
    }

    fn werte(s: &Schutz) -> Vec<String> {
        schutz_zeilen(s)
            .into_iter()
            .map(|(n, w, h)| format!("{n}: {w} [{h}]"))
            .collect()
    }

    #[test]
    fn the_layers_show_only_what_was_measured_with_its_date() {
        let mut z = Zusatz::default();
        z.nachricht(&json!({"art": "Schutzstand", "einschluss": {"probelauf": "2026-11-27"},
            "halter": {"eingerichtet": "2026-11-26"}, "letzter_start": {"zeit": 1_000_000_000_000_u64, "ergebnis": "gestartet"},
            "netzziele": [{"herkunft": "firma", "anzahl": 3}, {"herkunft": "gruppe g-eins", "anzahl": 9}, {"herkunft": "nutzer", "anzahl": 2}]}));
        // The time of the last start is a neutral example time; its text form is checked in the test of zeit_text.
        let letzter = format!(
            "Letzter Start: gestartet, {} []",
            zeit_text(1_000_000_000_000)
        );
        assert_eq!(
            werte(z.schutz.as_ref().expect("state")),
            [
                "Einschluss: Probelauf bei der Installation grün, 27.11.2026 []",
                "Halter: eingerichtet bei der Installation, 26.11.2026 []",
                letzter.as_str(),
                "Freigegebene Netzziele: 3 [Firma]",
                "Freigegebene Netzziele: 2 [du]",
            ]
        );
    }

    #[test]
    fn a_missing_part_says_so_and_never_on() {
        let mut z = Zusatz::default();
        z.nachricht(&json!({"art": "Schutzstand", "einschluss": null, "letzter_start": null}));
        let w = werte(z.schutz.as_ref().expect("state"));
        assert_eq!(
            w,
            [
                "Einschluss: nicht bekannt []",
                "Halter: nicht bekannt []",
                "Letzter Start: noch kein Start seit dem Start des Dienstes []",
                "Freigegebene Netzziele: nicht bekannt []",
            ]
        );
        z.nachricht(&json!({"art": "Schutzstand"}));
        assert_eq!(
            werte(z.schutz.as_ref().expect("state"))[2],
            "Letzter Start: nicht bekannt []"
        );
        assert!(
            w.iter()
                .all(|x| !x.contains(": an ") && !x.contains("läuft"))
        );
    }

    #[test]
    fn the_time_is_the_civil_date_in_utc() {
        // Date and time are compared as two parts, so the source carries no date with a time of day.
        let teile = |ms: u64| {
            zeit_text(ms)
                .split(", ")
                .map(str::to_owned)
                .collect::<Vec<_>>()
        };
        assert_eq!(teile(0), ["01.01.1970", "00:00 UTC"]);
        assert_eq!(teile(951_782_400_000), ["29.02.2000", "00:00 UTC"]);
        assert_eq!(teile(1_000_000_019_999), ["09.09.2001", "01:46 UTC"]);
    }

    /// Decision of the operator, 28.11.2026: four values are segments, five a drop-down.
    #[test]
    fn four_values_are_segments_and_five_a_drop_down() {
        assert_eq!(SEGMENTE_HOECHSTENS, 4);
        assert_eq!(wahlform(1), Wahlform::Segmente);
        assert_eq!(wahlform(4), Wahlform::Segmente);
        assert_eq!(wahlform(5), Wahlform::Ausklapper);
        assert_eq!(
            wahlform(ansicht::aussehen::SCHRIFTGROESSEN.len()),
            Wahlform::Ausklapper
        );
    }

    fn effort(harness: &str, erlaubt: &[&str]) -> ansicht::ziel::Effort {
        ansicht::ziel::Effort {
            harness: harness.to_owned(),
            modell: "auto".to_owned(),
            erlaubt: erlaubt.iter().map(|x| (*x).to_owned()).collect(),
            vorgabe: None,
        }
    }

    #[test]
    fn the_thinking_level_takes_the_efforts_of_the_routes_for_the_chosen_program() {
        use ansicht::woerter as w;
        let efforts = vec![
            (
                "a".to_owned(),
                vec![
                    effort("copilot", &["medium"]),
                    effort("pi", &["off", "low"]),
                ],
            ),
            (
                "b".to_owned(),
                vec![effort("copilot", &["none", "low", "medium", "high"])],
            ),
        ];
        let texte = |h: Option<&str>| -> Vec<String> {
            denkstufen(&efforts, h)
                .into_iter()
                .map(|(_, t)| t)
                .collect()
        };
        // each route keeps its order; a value of one route goes behind the value before it there
        assert_eq!(
            texte(Some("copilot")),
            [w::VORGABE_DES_PROGRAMMS, w::AUS, "low", "medium", "high"]
        );
        assert_eq!(texte(Some("pi")), [w::VORGABE_DES_PROGRAMMS, w::AUS, "low"]);
        assert_eq!(texte(Some("opencode")), [w::VORGABE_DES_PROGRAMMS]);
        // the first choice sends `null`, the others their value as the program names it
        let werte: Vec<String> = denkstufen(&efforts, Some("copilot"))
            .into_iter()
            .map(|(x, _)| x)
            .collect();
        assert_eq!(werte, ["", "none", "low", "medium", "high"]);
        // without a program every program, like the browser view
        assert!(texte(None).len() > texte(Some("pi")).len());
    }

    #[test]
    fn the_table_of_deadlines_has_one_row_per_server_in_seconds() {
        use ansicht::woerter as w;
        let f = |schluessel: &str, server: &str, wert: u64, herkunft: &str| crate::ziel::Frist {
            schluessel: schluessel.to_owned(),
            server: server.to_owned(),
            wert,
            herkunft: herkunft.to_owned(),
        };
        let (e, l) = ("aufsicht.freigabefrist_ms", "aufsicht.einloesefrist_ms");
        let t = fristen_tabelle(&[
            f(l, "wiki", 60_000, "firmendatei"),
            f(e, "tickets", 1500, "katalog"),
            f(e, "wiki", 120_000, "anders"),
        ]);
        let zelle = |w: &str, h: &str| Some((w.to_owned(), h.to_owned()));
        assert_eq!(
            t,
            [
                (
                    "wiki".to_owned(),
                    [
                        zelle("120 s", "anders"),
                        zelle("60 s", w::HERKUNFT_FIRMENDATEI)
                    ]
                ),
                (
                    "tickets".to_owned(),
                    [zelle("1500 ms", w::HERKUNFT_KATALOG), None]
                ),
            ]
        );
    }
}
