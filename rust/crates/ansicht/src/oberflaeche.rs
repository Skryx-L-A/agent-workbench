//! The visible view: the look of today's Mac app (`rust/spec/VERTRAEGE.md` version 2, decisions of 26.11.2026) as Dioxus components
//! over [`Kern`].
//!
//! Title bar over the full width, sidebar on the left, the terminal in the centre, the inspector on the right (closed by
//! default). Only what belongs to the pilot and is bound appears (OF8): a path whose data the stream does not carry (gaps L1
//! to L11, [`crate::ziel`]) has no element. Every element of an operating path carries `data-bw="Bnn"` (OF7). The window
//! "Sitzungen", the settings and the questions live in [`crate::fenster`].

// The expansion of `rsx!` names event paths that the prelude already imports, and the handlers of a component bind
// many one-letter names (`a`, `e`, `k`, `w`) by the convention of this file; both lints fire on generated code.
// The handle `A` holds `Copy` signals; it is copied into every handler on purpose.
#![allow(
    unused_qualifications,
    clippy::many_single_char_names,
    clippy::large_types_passed_by_value,
    clippy::needless_pass_by_value
)]

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use dioxus::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{AbortController, HtmlElement, HtmlTextAreaElement};

use crate::aussehen::{self, Aussehen};
use crate::fenster;
use crate::kern::{Kern, Sitzung, Verbindung, Wirkung, klemme};
use crate::netz::{self, Sender, Zerleger};
use crate::tasten::{self, Deutung};
use crate::woerter;
use crate::zeile;
use crate::ziel::{self, Agent};

/// The most announcements kept in the live region.
const BEHALTEN: usize = 20;
/// Worker panes per tab (B30: the tab strip appears only above this number).
pub const JE_TAB: usize = 6;

thread_local! {
    static SENDER: RefCell<Option<Rc<Sender>>> = const { RefCell::new(None) };
    static ABBRUCH: RefCell<Option<AbortController>> = const { RefCell::new(None) };
    static TOKEN: RefCell<Option<String>> = const { RefCell::new(None) };
    static BEENDET: Cell<bool> = const { Cell::new(false) };
}

/// What lies over the view.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Ueber {
    /// Nothing.
    Nichts,
    /// The window "Sitzungen" (B68).
    Sitzungen,
    /// The settings on a page (B72, B78).
    Einstellungen(&'static str),
    /// The worker list under the pill (B29).
    Workerliste,
    /// The question before renaming (B12).
    Umbenennen(String),
    /// The question before deleting (B16).
    Loeschen(String),
}

/// The view in the centre (B27).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ansicht {
    /// One terminal pane of the session.
    Orchestrator,
    /// The panes of the workers of the session, tiled.
    Worker,
}

/// The sheet of the inspector (B51).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reiter {
    /// Requests (B56).
    Freigaben,
    /// The working folder (B52).
    Ordner,
}

/// The reactive parts of the view; all of them are `Copy`.
#[derive(Clone, Copy)]
pub struct A {
    /// The state of the view and the rules of the contract.
    pub kern: Signal<Kern>,
    /// What lies over the view.
    pub ueber: Signal<Ueber>,
    /// The context menu of a session: id and position.
    pub menue: Signal<Option<(String, f64, f64)>>,
    /// The short note (B67).
    pub notiz: Signal<Option<(u32, String)>>,
    /// The announcements of the live region.
    pub ansagen: Signal<Vec<(u32, String)>>,
    /// A counter for keys.
    pub zaehler: Signal<u32>,
    /// What this device remembers (L8) and the look (OF3).
    pub aussehen: Signal<Aussehen>,
    /// Orchestrator or workers (B27).
    pub ansicht: Signal<Ansicht>,
    /// The sheet of the inspector.
    pub reiter: Signal<Reiter>,
    /// The session chosen in the window "Sitzungen".
    pub fensterwahl: Signal<Option<String>>,
    /// The width being dragged: `true` the sidebar, `false` the inspector.
    pub ziehen: Signal<Option<bool>>,
    /// The session dragged in the sidebar (B19).
    pub gezogen: Signal<Option<String>>,
    /// The tab and the zoomed pane of the worker view (B30, B31).
    pub worker_tab: Signal<usize>,
    /// The zoomed worker pane, if any.
    pub gezoomt: Signal<Option<u8>>,
}

pub(crate) fn dokument() -> Option<web_sys::Document> {
    web_sys::window().and_then(|w| w.document())
}

pub(crate) fn element(id: &str) -> Option<HtmlElement> {
    dokument()?
        .get_element_by_id(id)?
        .dyn_into::<HtmlElement>()
        .ok()
}

pub(crate) fn fokus(id: &str) {
    if let Some(e) = element(id) {
        fokus_auf(&e);
    }
}

fn fokus_auf(e: &HtmlElement) {
    let o = web_sys::FocusOptions::new();
    o.set_prevent_scroll(true);
    let _ = e.focus_with_options(&o);
}

/// Focus that comes back after a window closed: it shows its ring (option `focusVisible`), also after a closing click; the
/// ring of a button otherwise shows only with keyboard focus.
fn fokus_zurueck(id: &str) {
    if let Some(e) = element(id) {
        let o = web_sys::FocusOptions::new();
        o.set_prevent_scroll(true);
        let _ = js_sys::Reflect::set(
            o.as_ref(),
            &wasm_bindgen::JsValue::from_str("focusVisible"),
            &wasm_bindgen::JsValue::TRUE,
        );
        let _ = e.focus_with_options(&o);
    }
}

/// The token of the address, once read.
pub(crate) fn token() -> Option<String> {
    TOKEN.with(|t| t.borrow().clone())
}

fn px(cs: &web_sys::CssStyleDeclaration, name: &str) -> f64 {
    cs.get_property_value(name)
        .ok()
        .and_then(|v| v.trim_end_matches("px").parse().ok())
        .unwrap_or(0.0)
}

/// Columns and rows that fit into the raster with this id (SE5 point 2).
fn messen_in(id: &str) -> (usize, usize) {
    let (Some(w), Some(r), Some(m)) = (web_sys::window(), element(id), element("messung")) else {
        return klemme(0.0, 0.0, 0.0, 0.0);
    };
    let rect = r.get_bounding_client_rect();
    let (mut breite, mut hoehe) = (rect.width(), rect.height());
    if let Ok(Some(cs)) = w.get_computed_style(&r) {
        breite -= px(&cs, "padding-left") + px(&cs, "padding-right");
        breite -= px(&cs, "border-left-width") + px(&cs, "border-right-width");
        hoehe -= px(&cs, "padding-top") + px(&cs, "padding-bottom");
        hoehe -= px(&cs, "border-top-width") + px(&cs, "border-bottom-width");
    }
    let z = m.get_bounding_client_rect();
    klemme(breite, hoehe, z.width() / 10.0, z.height())
}

fn messen() -> (usize, usize) {
    messen_in("raster")
}

// ---- focus: areas and tab stops (SE2) ----

const BEREICHE: [&str; 4] = ["titelleiste", "leiste", "terminal", "inspektor"];

fn sichtbar_fokussierbar(e: &HtmlElement) -> bool {
    e.get_attribute("disabled").is_none()
        && e.tab_index() >= 0
        && e.closest("[hidden]").ok().flatten().is_none()
        && {
            let r = e.get_bounding_client_rect();
            r.width() > 0.0 || r.height() > 0.0
        }
}

/// The tab stops below an element, in document order.
pub(crate) fn halte_in(wurzel: &web_sys::Element) -> Vec<HtmlElement> {
    let Ok(liste) = wurzel.query_selector_all("button, input, select, textarea, [tabindex=\"0\"]")
    else {
        return Vec::new();
    };
    (0..liste.length())
        .filter_map(|i| liste.item(i))
        .filter_map(|n| n.dyn_into::<HtmlElement>().ok())
        .filter(sichtbar_fokussierbar)
        .collect()
}

fn bereich_von(e: &web_sys::Element) -> Option<&'static str> {
    if e.closest(".terminal").ok().flatten().is_some() {
        return Some("terminal");
    }
    BEREICHE
        .iter()
        .find(|b| e.closest(&format!("#{b}")).ok().flatten().is_some())
        .copied()
}

fn erster_halt(bereich: &str) -> Option<HtmlElement> {
    if bereich == "terminal" {
        let d = dokument()?;
        let t = d
            .query_selector(".terminal:not([hidden]) textarea")
            .ok()??;
        return t.dyn_into::<HtmlElement>().ok();
    }
    let w = dokument()?.get_element_by_id(bereich)?;
    halte_in(&w).into_iter().next()
}

/// F6 and Shift+F6: the next or previous area (SE2 point 4); from the terminal Shift+F6 goes to the chosen session.
fn bereich_wechseln(a: A, vorwaerts: bool) {
    let Some(d) = dokument() else { return };
    let jetzt = d.active_element().and_then(|e| bereich_von(&e));
    if jetzt == Some("terminal") && !vorwaerts {
        let aktiv = a.kern.peek().aktiv.clone();
        if let Some(e) = aktiv.and_then(|s| element(&format!("zeile-{s}"))) {
            fokus_auf(&e);
            return;
        }
    }
    let da: Vec<&str> = BEREICHE
        .iter()
        .copied()
        .filter(|b| erster_halt(b).is_some())
        .collect();
    if da.is_empty() {
        return;
    }
    let i = jetzt.and_then(|j| da.iter().position(|b| *b == j));
    let n = da.len();
    let ziel = match (i, vorwaerts) {
        (Some(i), true) => (i + 1) % n,
        (Some(i), false) => (i + n - 1) % n,
        (None, true) => 0,
        (None, false) => n - 1,
    };
    if let Some(e) = erster_halt(da[ziel]) {
        fokus_auf(&e);
    }
}

/// Tab and Shift+Tab stay inside a window (focus trap, SE2 point 2).
pub(crate) fn falle(id: &str, e: &KeyboardEvent) {
    if e.key() != Key::Tab {
        return;
    }
    let Some(w) = dokument().and_then(|d| d.get_element_by_id(id)) else {
        return;
    };
    let halte = halte_in(&w);
    if halte.is_empty() {
        // a window without a tab stop (the worker list) keeps the focus on itself
        e.prevent_default();
        return;
    }
    let aktiv = dokument().and_then(|d| d.active_element());
    let i = aktiv.and_then(|x| halte.iter().position(|h| x == **h));
    let n = halte.len();
    let ziel = match (i, e.modifiers().contains(Modifiers::SHIFT)) {
        (Some(i), false) => (i + 1) % n,
        (Some(i), true) => (i + n - 1) % n,
        (None, false) => 0,
        (None, true) => n - 1,
    };
    e.prevent_default();
    fokus_auf(&halte[ziel]);
}

// ---- announcements, notes, effects ----

fn ansage(a: A, text: String) {
    let mut z = a.zaehler;
    let n = *z.peek() + 1;
    z.set(n);
    let mut ansagen = a.ansagen;
    let mut l = ansagen.write();
    l.push((n, text));
    let zu_viel = l.len().saturating_sub(BEHALTEN);
    l.drain(..zu_viel);
}

fn notiz(a: A, text: String) {
    let mut z = a.zaehler;
    let n = *z.peek() + 1;
    z.set(n);
    let mut nz = a.notiz;
    nz.set(Some((n, text)));
    spawn(async move {
        netz::warten(4000.0).await;
        let mut nz = a.notiz;
        if nz.peek().as_ref().is_some_and(|(m, _)| *m == n) {
            nz.set(None);
        }
    });
}

fn strom_ende(a: A) {
    if BEENDET.with(|b| b.replace(true)) {
        return;
    }
    SENDER.with(|s| {
        if let Some(s) = s.borrow().as_ref() {
            s.verbindung(None);
        }
    });
    let mut k = a.kern;
    let w = k.write().ende();
    // without the core nothing can be operated: windows and menus close, the view stays readable (B05)
    let mut u = a.ueber;
    u.set(Ueber::Nichts);
    let mut m = a.menue;
    m.set(None);
    wirken(a, w);
}

/// Sends one event of the view.
pub(crate) fn senden(v: &serde_json::Value) {
    SENDER.with(|s| {
        if let Some(s) = s.borrow().as_ref() {
            s.sende(v);
        }
    });
}

/// Carries out the effects of an event.
pub(crate) fn wirken(a: A, wirkungen: Vec<Wirkung>) {
    for w in wirkungen {
        match w {
            Wirkung::Senden(v) => senden(&v),
            Wirkung::Ansage(t) => ansage(a, t),
            Wirkung::Notiz(t) => notiz(a, t),
            Wirkung::Wecker(ms) => {
                spawn(async move {
                    netz::warten(ms).await;
                    let mut k = a.kern;
                    let w = k.write().wecken(netz::jetzt());
                    wirken(a, w);
                });
            }
            Wirkung::FokusTerminal => fokus("eingang"),
            Wirkung::FokusStart => fokus("neue-sitzung"),
            Wirkung::StromZu => {
                ABBRUCH.with(|c| {
                    if let Some(c) = c.borrow().as_ref() {
                        c.abort();
                    }
                });
                strom_ende(a);
            }
        }
    }
}

async fn strom_lauf(a: A, token: String) {
    let Ok(abbruch) = AbortController::new() else {
        strom_ende(a);
        return;
    };
    ABBRUCH.with(|c| *c.borrow_mut() = Some(abbruch.clone()));
    let Some(strom) = netz::strom_oeffnen(&token, &abbruch).await else {
        strom_ende(a);
        return;
    };
    SENDER.with(|s| {
        if let Some(s) = s.borrow().as_ref() {
            s.verbindung(strom.verbindung.clone());
        }
    });
    let mut k = a.kern;
    k.write().offen(strom.deflate);
    let mut zerleger = Zerleger::default();
    'lesen: while let Some(stueck) = strom.lesen().await {
        let rahmen = match zerleger.nimm(&stueck) {
            Ok(r) => r,
            // like `RahmenZuGross` of a raster frame: the error field, then the stream ends
            Err(e) => {
                k.write().fehler = Some((
                    e.code().to_owned(),
                    "Strom der Verbindung nicht lesbar".to_owned(),
                ));
                wirken(a, vec![Wirkung::StromZu]);
                break;
            }
        };
        for (art, nutz) in rahmen {
            if BEENDET.with(Cell::get) {
                break 'lesen;
            }
            let g = if art == crate::kern::ART_NACHRICHT {
                messen()
            } else {
                (2, 1)
            };
            let w = k.write().rahmen(art, &nutz, netz::jetzt(), g);
            wirken(a, w);
        }
        if BEENDET.with(Cell::get) {
            break;
        }
    }
    strom_ende(a);
}

/// The choice of a session (SE2 point 5).
pub(crate) fn waehle(a: A, id: &str) {
    let g = messen();
    let mut k = a.kern;
    let w = k.write().waehle(id, g);
    let mut z = a.gezoomt;
    z.set(None);
    wirken(a, w);
    if *a.ansicht.peek() == Ansicht::Worker {
        spawn(async move {
            netz::warten(0.0).await;
            worker_messen(a);
        });
    }
}

/// Opens or closes the inspector (B04) and remembers it per device (L8).
fn inspektor_umschalten(a: A) {
    let mut s = a.aussehen;
    let mut x = s.write();
    x.inspektor_offen = !x.inspektor_offen;
    x.merken();
}

/// The view.
///
/// # Errors
///
/// None of its own: the result is the rendered element tree of Dioxus.
#[allow(clippy::too_many_lines)]
pub fn app() -> Element {
    let a = A {
        kern: use_signal(Kern::default),
        ueber: use_signal(|| Ueber::Nichts),
        menue: use_signal(|| None),
        notiz: use_signal(|| None),
        ansagen: use_signal(Vec::new),
        zaehler: use_signal(|| 0),
        aussehen: use_signal(Aussehen::laden),
        ansicht: use_signal(|| Ansicht::Orchestrator),
        reiter: use_signal(|| Reiter::Freigaben),
        fensterwahl: use_signal(|| None),
        ziehen: use_signal(|| None),
        gezogen: use_signal(|| None),
        worker_tab: use_signal(|| 0),
        gezoomt: use_signal(|| None),
    };
    use_hook(move || {
        a.aussehen.peek().anwenden();
        spawn(async move {
            let hash = web_sys::window()
                .and_then(|w| w.location().hash().ok())
                .unwrap_or_default();
            match netz::token_aus_fragment(&hash) {
                None => {
                    let mut k = a.kern;
                    k.write().ohne_token();
                }
                Some(t) => {
                    SENDER.with(|s| *s.borrow_mut() = Some(Sender::neu(&t)));
                    TOKEN.with(|x| *x.borrow_mut() = Some(t.clone()));
                    strom_lauf(a, t).await;
                }
            }
        });
    });

    let ueber = a.ueber.read().clone();
    let au = a.aussehen.read().clone();
    let insp = au.inspektor_offen;
    // after the end of the stream every control is locked; the terminals keep their last picture (B05)
    let gesperrt = matches!(a.kern.read().verbindung, Verbindung::Zu(_)).then_some("");
    let zeilenhoehe = au.zeilenhoehe();
    let stil = format!(
        "--leiste-breite:{:.0}px;--inspektor-breite:{:.0}px;--term-schrift:{}px;--term-zeile:{}px;",
        au.leiste, au.inspektor_breite, au.schrift, zeilenhoehe
    );
    rsx! {
        div {
            id: "rahmen",
            style: "{stil}",
            onkeydown: move |e: KeyboardEvent| {
                if e.key() == Key::F6 {
                    e.prevent_default();
                    if *a.ueber.peek() == Ueber::Nichts {
                        bereich_wechseln(a, !e.modifiers().contains(Modifiers::SHIFT));
                    }
                    return;
                }
                if e.key() == Key::Escape {
                    let mut u = a.ueber;
                    let mut m = a.menue;
                    let war = u.peek().clone();
                    if war != Ueber::Nichts || m.peek().is_some() {
                        e.prevent_default();
                        u.set(Ueber::Nichts);
                        m.set(None);
                        nach_schliessen(&war);
                    }
                }
            },
            {titelleiste(a, gesperrt)}
            {kanalwarnung(a)}
            div { id: "app", class: if insp { "mit-inspektor" } else { "" }, "inert": gesperrt,
                if au.leiste_offen {
                    {leiste(a)}
                    div { class: "griff", id: "griff-links", "data-bw": "B24", "aria-hidden": "true",
                        onmousedown: move |e: MouseEvent| { e.prevent_default(); let mut z = a.ziehen; z.set(Some(true)); } }
                }
                {mitte(a, zeilenhoehe)}
                if insp {
                    div { class: "griff", id: "griff-rechts", "data-bw": "B57", "aria-hidden": "true",
                        onmousedown: move |e: MouseEvent| { e.prevent_default(); let mut z = a.ziehen; z.set(Some(false)); } }
                    {inspektor(a)}
                }
            }
            if a.ziehen.read().is_some() {
                div { class: "zieh-flaeche",
                    onmousemove: move |e: MouseEvent| breite_ziehen(a, e.client_coordinates().x),
                    onmouseup: move |_| { let mut z = a.ziehen; z.set(None); a.aussehen.peek().merken(); },
                }
            }
            if ueber == Ueber::Workerliste { {workerliste(a)} }
            if ueber == Ueber::Sitzungen { {fenster::sitzungen(a)} }
            if let Ueber::Einstellungen(seite) = ueber { {fenster::einstellungen(a, seite)} }
            if let Ueber::Umbenennen(id) = ueber.clone() { {fenster::umbenennen(a, id)} }
            if let Ueber::Loeschen(id) = ueber.clone() { {fenster::loeschen(a, id)} }
            {kontextmenue(a)}
            div { class: "versteckt", id: "ansagen", role: "log", "aria-live": "polite",
                for (n, t) in a.ansagen.read().iter().cloned() {
                    div { key: "{n}", "{t}" }
                }
            }
        }
    }
}

/// After a window closes, the focus goes back to its opener (SE2 point 2: "+" for the window "Sitzungen").
pub(crate) fn nach_schliessen(war: &Ueber) {
    match war {
        Ueber::Sitzungen => fokus_zurueck("neue-sitzung"),
        Ueber::Einstellungen(_) => fokus_zurueck("zahnrad-fuss"),
        Ueber::Workerliste => fokus_zurueck("statuspille"),
        Ueber::Umbenennen(_) | Ueber::Loeschen(_) | Ueber::Nichts => {}
    }
}

fn breite_ziehen(a: A, x: f64) {
    let Some(z) = *a.ziehen.peek() else { return };
    let breite = web_sys::window()
        .and_then(|w| w.inner_width().ok())
        .and_then(|v| v.as_f64())
        .unwrap_or(1280.0);
    let mut s = a.aussehen;
    let mut au = s.write();
    if z {
        let max = aussehen::LEISTE_MAX.min(
            breite
                - aussehen::MITTE_MIN
                - if au.inspektor_offen {
                    au.inspektor_breite
                } else {
                    0.0
                },
        );
        au.leiste = aussehen::klemmen(x, aussehen::LEISTE_MIN, max.max(aussehen::LEISTE_MIN));
    } else {
        let links = if au.leiste_offen { au.leiste } else { 0.0 };
        let max = aussehen::INSPEKTOR_MAX.min(breite - aussehen::MITTE_MIN - links);
        au.inspektor_breite = aussehen::klemmen(
            breite - x,
            aussehen::INSPEKTOR_MIN,
            max.max(aussehen::INSPEKTOR_MIN),
        );
    }
}

pub(crate) fn punkt_klasse(s: Option<&Sitzung>) -> &'static str {
    match s {
        Some(s) if s.laeuft => "punkt laeuft",
        Some(_) => "punkt aus",
        None => "punkt ruhig",
    }
}

fn agent_punkt(a: &Agent) -> &'static str {
    match a.zustand.as_str() {
        "laeuft" => "punkt laeuft",
        "beendet" => "punkt aus",
        _ => "punkt ruhig",
    }
}

/// The text of a worker state.
pub(crate) fn zustand_wort(z: &str) -> &str {
    match z {
        "laeuft" => "läuft",
        "beendet" => "läuft nicht mehr",
        x => x,
    }
}

#[allow(clippy::too_many_lines)]
fn titelleiste(a: A, gesperrt: Option<&'static str>) -> Element {
    let k = a.kern.read();
    let s = k.aktive_sitzung().cloned();
    let anzahl = k.anfragen.len();
    let worker = k.worker_der_sitzung();
    let (b26, b27, b28) = (k.zeigt("B26"), k.zeigt("B27"), k.zeigt("B28"));
    drop(k);
    let au = a.aussehen.read().clone();
    let ansicht = *a.ansicht.read();
    let (name, herkunft) = match &s {
        Some(s) => {
            let mut teile: Vec<&str> = s.teile();
            if let Some(o) = s.ordner() {
                teile.push(o);
            }
            let name = s.projekt().unwrap_or(s.id.as_str()).to_owned();
            (name, teile.join(" · "))
        }
        None => ("Keine Sitzung gewählt".to_owned(), String::new()),
    };
    let laufen = worker
        .as_ref()
        .filter(|_| b26)
        .map(|w| w.iter().filter(|x| x.laeuft()).count());
    let blase = *a.ueber.read() == Ueber::Workerliste;
    rsx! {
        header { id: "titelleiste", class: if au.leiste_offen { "mit-leiste" } else { "" }, "inert": gesperrt,
            button { r#type: "button", id: "leiste-knopf", class: "knopf-symbol", "data-bw": "B80", "aria-label": "Leiste ein- und ausblenden",
                "aria-expanded": if au.leiste_offen { "true" } else { "false" }, title: "Leiste ein- und ausblenden",
                onclick: move |_| { let mut s = a.aussehen; let mut x = s.write(); x.leiste_offen = !x.leiste_offen; x.merken(); },
                {symbol_leiste()}
            }
            div { id: "kopf-titel", "data-bw": "B02",
                div { class: "kt-name", title: "{name}", "{name}" }
                if !herkunft.is_empty() { div { class: "kt-herkunft", title: "{herkunft}", "{herkunft}" } }
            }
            div { id: "kopf-rechts",
                if anzahl > 0 {
                    button { r#type: "button", id: "freigabe-abzeichen", class: "abzeichen", "data-bw": "B03",
                        "aria-label": "Freigaben: {anzahl} offen", title: "Freigaben: {anzahl} offen",
                        onclick: move |_| {
                            let mut r = a.reiter; r.set(Reiter::Freigaben);
                            if !a.aussehen.peek().inspektor_offen { inspektor_umschalten(a); }
                        },
                        {symbol_schild(12)}
                        span { "{anzahl}" }
                    }
                }
                if let Some(n) = laufen {
                    button { r#type: "button", id: "statuspille", class: "statuspille", "data-bw": "B26",
                        "aria-label": "{n} laufen, Worker-Liste", "aria-expanded": if blase { "true" } else { "false" },
                        onclick: move |_| { let mut u = a.ueber; let auf = *u.peek() == Ueber::Workerliste; u.set(if auf { Ueber::Nichts } else { Ueber::Workerliste }); },
                        span { class: "punkt laeuft" }
                        span { class: "zahl", "{n} laufen" }
                    }
                }
                if b27 && laufen.is_some() {
                    div { class: "segment", role: "group", "aria-label": "Ansicht", "data-testid": "ansicht-umschalter", "data-bw": "B27",
                        button { r#type: "button", class: if ansicht == Ansicht::Orchestrator { "gewaehlt" } else { "" },
                            "aria-pressed": if ansicht == Ansicht::Orchestrator { "true" } else { "false" },
                            onclick: move |_| ansicht_setzen(a, Ansicht::Orchestrator), "Orchestrator" }
                        button { r#type: "button", class: if ansicht == Ansicht::Worker { "gewaehlt" } else { "" },
                            "aria-pressed": if ansicht == Ansicht::Worker { "true" } else { "false" },
                            onclick: move |_| ansicht_setzen(a, Ansicht::Worker), "Worker" }
                    }
                }
                div { class: "knopfgruppe",
                    button { r#type: "button", id: "inspektor-knopf", class: "knopf-symbol", "data-testid": "inspektor-umschalter", "data-bw": "B04",
                        "aria-label": "Inspektor ein- und ausblenden", "aria-expanded": if au.inspektor_offen { "true" } else { "false" },
                        title: "Inspektor ein- und ausblenden",
                        onclick: move |_| inspektor_umschalten(a),
                        {symbol_inspektor()}
                    }
                    if b28 {
                        button { r#type: "button", id: "zahnrad-titel", class: "knopf-symbol", "data-bw": "B28",
                            "aria-label": "Einstellungen der Sitzung", title: "Einstellungen der Sitzung",
                            onclick: move |_| { let mut u = a.ueber; u.set(Ueber::Einstellungen("sitzung")); },
                            {symbol_zahnrad(17)}
                        }
                    }
                }
            }
        }
    }
}

fn ansicht_setzen(a: A, neu: Ansicht) {
    let mut s = a.ansicht;
    if *s.peek() == neu {
        return;
    }
    s.set(neu);
    if neu == Ansicht::Orchestrator {
        let mut k = a.kern;
        let w = k.write().worker_zeigen(&[]);
        wirken(a, w);
    } else {
        spawn(async move {
            netz::warten(0.0).await;
            worker_messen(a);
        });
    }
}

fn kanalwarnung(a: A) -> Element {
    let k = a.kern.read();
    let Verbindung::Zu(text) = k.verbindung else {
        return rsx! {};
    };
    // over the centre, the bar below does not move (B05); a reload only helps after the end of the stream
    let neu_laden = text == crate::kern::GETRENNT;
    rsx! {
        div { id: "kanalwarnung", "data-bw": "B05", "data-zustand": "zu", role: "status",
            span { class: "kw-text", "Verbindung zum Kern fehlt: {text}" }
            if neu_laden {
                button { r#type: "button", class: "knopf-rand", id: "neu-laden",
                    onclick: move |_| { if let Some(w) = web_sys::window() { let _ = w.location().reload(); } },
                    "Neu laden" }
            }
        }
    }
}

#[allow(clippy::too_many_lines)]
fn leiste(a: A) -> Element {
    let k = a.kern.read();
    // a worker is a session of its own (DI-L1 point 1); the sidebar shows it under its orchestrator, not as a row
    let sitzungen: Vec<Sitzung> = k
        .sitzungen
        .iter()
        .filter(|s| !k.ist_worker(&s.id))
        .cloned()
        .collect();
    let aktiv = k.aktiv.clone();
    let agenten = k.ziel_agenten.clone();
    let (b09, b22) = (k.zeigt("B09"), k.zeigt("B22"));
    drop(k);
    let au = a.aussehen.read().clone();
    let sortiert: Vec<Sitzung> = au
        .sortiert(&sitzungen, |s| s.id.as_str())
        .into_iter()
        .cloned()
        .collect();
    // projects (L5): the sessions grouped by project, in the order of their first session
    let mit_projekt = b09;
    let mut gruppen: Vec<(String, Vec<Sitzung>)> = Vec::new();
    for s in sortiert {
        let p = s.projekt().unwrap_or_default().to_owned();
        match gruppen.iter_mut().find(|(q, _)| *q == p) {
            Some((_, l)) => l.push(s),
            None => gruppen.push((p, vec![s])),
        }
    }
    let laufen = agenten
        .as_ref()
        .filter(|_| b22)
        .map(|a| a.iter().filter(|x| x.laeuft()).count());
    rsx! {
        nav { id: "leiste", "aria-label": "Sitzungen nach Projekt",
            div { class: "leiste-kopf", "data-bw": "B08",
                span { class: "leiste-titel", "Sitzungen" }
                button { r#type: "button", id: "neue-sitzung", class: "knopf-symbol", "data-testid": "neue-sitzung",
                    "aria-label": "Neue Sitzung", title: "Neue Sitzung",
                    onclick: move |_| fenster::sitzungen_oeffnen(a),
                    {symbol_plus()}
                }
            }
            div { id: "sitzungen",
                if sitzungen.is_empty() {
                    p { class: "leer-leiste", "data-bw": "B34", "Noch keine Sitzung. Lege mit + eine an." }
                }
                ul { id: "liste", role: "list", "aria-label": "Sitzungen", "data-testid": "sitzung-liste", "data-bw": "B10",
                    for (projekt, liste) in gruppen {
                        if mit_projekt && !projekt.is_empty() {
                            li { key: "p-{projekt}", role: "presentation", class: "projekt-zeile", "data-bw": "B09",
                                span { class: "projekt-name", "{projekt}" }
                                span { class: "projekt-zahl", "{liste.len()}" }
                            }
                        }
                        for s in liste {
                            {sitzungszeile(a, &s, aktiv.as_deref(), agenten.as_deref())}
                        }
                    }
                }
            }
            footer { id: "leiste-fuss",
                if let Some(n) = laufen {
                    div { class: "fuss-worker", "data-bw": "B22", b { "{n}" } " Worker laufen" }
                }
                button { r#type: "button", id: "zahnrad-fuss", class: "knopf-symbol", "data-bw": "B23", "aria-label": "Einstellungen",
                    title: "Einstellungen",
                    onclick: move |_| { let mut u = a.ueber; u.set(Ueber::Einstellungen("aussehen")); },
                    {symbol_zahnrad(15)}
                }
            }
        }
    }
}

#[allow(clippy::too_many_lines)]
fn sitzungszeile(a: A, s: &Sitzung, aktiv: Option<&str>, agenten: Option<&[Agent]>) -> Element {
    let gewaehlt = aktiv == Some(s.id.as_str());
    let zustand = s.zustand_text();
    let teile = s.teile();
    let label = format!(
        "{}, {zustand}",
        [
            vec![s.anzeigename()],
            if s.herkunft.is_some() {
                vec![]
            } else {
                teile.clone()
            }
        ]
        .concat()
        .join(" ")
    );
    let label = if s.herkunft.is_some() && s.anzeigename() != s.id {
        format!("{label}, {}", s.id)
    } else {
        label
    };
    // second line: "N Worker · läuft" with L1, else the state alone
    let zusatz = if let Some(ag) = agenten {
        let n = ag
            .iter()
            .filter(|x| x.sitzung == s.id && x.laeuft())
            .count();
        format!("{n} Worker{}{}", woerter::TRENNER, s.zustand_text())
    } else {
        // without L1 the number of workers is unknown: only the state; harness and model stand in the title bar (B02, G8)
        s.zustand_text()
    };
    let testid = format!("sitzung-{}", s.id);
    let id_klick = s.id.clone();
    let id_menue = s.id.clone();
    let id_taste = s.id.clone();
    let id_zieh = s.id.clone();
    let id_drop = s.id.clone();
    let datenzustand = if s.laeuft { "laeuft" } else { "beendet" };
    let code = (!s.laeuft).then(|| s.code.clone());
    let name = s.anzeigename().to_owned();
    rsx! {
        li { key: "{s.id}", draggable: "true", class: "sitzung-li",
            ondragstart: move |_| { let mut g = a.gezogen; g.set(Some(id_zieh.clone())); },
            ondragover: move |e: DragEvent| e.prevent_default(),
            ondrop: move |e: DragEvent| {
                e.prevent_default();
                let von = a.gezogen.peek().clone();
                if let Some(von) = von {
                    let alle: Vec<String> = a.kern.peek().sitzungen.iter().map(|x| x.id.clone()).collect();
                    let mut s = a.aussehen;
                    let mut x = s.write();
                    x.verschieben(&alle, &von, &id_drop);
                    x.merken();
                }
                let mut g = a.gezogen; g.set(None);
            },
            button { r#type: "button", class: if gewaehlt { "sitzung gewaehlt" } else { "sitzung" }, "data-testid": "{testid}",
                "data-zustand": datenzustand, "data-code": code, "aria-label": "{label}",
                "aria-current": if gewaehlt { "true" } else { "false" }, "aria-haspopup": "menu", "data-bw": "B19",
                id: "zeile-{s.id}",
                onclick: move |_| waehle(a, &id_klick),
                oncontextmenu: move |e: MouseEvent| {
                    e.prevent_default();
                    let p = e.client_coordinates();
                    let mut m = a.menue;
                    m.set(Some((id_menue.clone(), p.x, p.y)));
                },
                onkeydown: move |e: KeyboardEvent| {
                    if e.key() == Key::ContextMenu || (e.key() == Key::F10 && e.modifiers().contains(Modifiers::SHIFT)) {
                        e.prevent_default();
                        let r = element(&format!("zeile-{id_taste}")).map(|x| x.get_bounding_client_rect());
                        let (x, y) = r.map_or((20.0, 120.0), |r| (r.left() + 24.0, r.bottom()));
                        let mut m = a.menue;
                        m.set(Some((id_taste.clone(), x, y)));
                    }
                },
                span { class: if s.laeuft { "punkt laeuft" } else { "punkt aus" } }
                span { class: "sitzung-leib",
                    span { class: "sitzung-name", "{name}" }
                    span { class: "sitzung-zusatz", "{zusatz}" }
                }
            }
        }
    }
}

fn kontextmenue(a: A) -> Element {
    let Some((id, x, y)) = a.menue.read().clone() else {
        return rsx! {};
    };
    let k = a.kern.read();
    let Some(s) = k.sitzungen.iter().find(|s| s.id == id).cloned() else {
        return rsx! {};
    };
    let (b11, b12, b16, b17) = (
        k.zeigt("B11"),
        k.zeigt("B12"),
        k.zeigt("B16"),
        k.zeigt("B17"),
    );
    drop(k);
    let stil = format!("left:{x:.0}px;top:{y:.0}px;");
    let schliessen = move || {
        let mut m = a.menue;
        m.set(None);
    };
    let (i1, i2, i3, i4, i5) = (id.clone(), id.clone(), id.clone(), id.clone(), id.clone());
    rsx! {
        div { class: "menue-grund", onclick: move |_| schliessen() }
        div { class: "menue", role: "menu", "aria-label": "Sitzung {s.anzeigename()}", style: "{stil}",
            onmounted: move |_| {
                if let Some(d) = dokument()
                    && let Ok(Some(e)) = d.query_selector(".menue button:not([disabled])")
                    && let Ok(e) = e.dyn_into::<HtmlElement>()
                {
                    let _ = e.focus();
                }
            },
            if b11 && !s.laeuft && s.fortsetzbar() && s.herkunft.as_ref().is_none_or(|h| h.fortgesetzt_als.is_none()) {
                button { r#type: "button", role: "menuitem", "data-bw": "B11",
                    onclick: move |_| { schliessen(); senden(&ziel::fortsetzen(&i1)); }, "Fortsetzen" }
            }
            if b12 {
                button { r#type: "button", role: "menuitem", "data-bw": "B12",
                    onclick: move |_| { schliessen(); let mut u = a.ueber; u.set(Ueber::Umbenennen(i2.clone())); }, "Namen ändern …" }
            }
            button { r#type: "button", role: "menuitem", "data-bw": "B15", disabled: !s.laeuft,
                title: if s.laeuft { "" } else { "Sitzung ist beendet" },
                onclick: move |_| { let w = a.kern.read().beenden(&i3); schliessen(); wirken(a, w); },
                {woerter::BEENDEN}
            }
            if b17 && s.abgestuerzt() && s.fortsetzbar() {
                button { r#type: "button", role: "menuitem", "data-bw": "B17",
                    onclick: move |_| { schliessen(); senden(&ziel::wiederbeleben(&i4)); }, "Wiederbeleben …" }
            }
            if b16 && !s.laeuft {
                button { r#type: "button", role: "menuitem", "data-bw": "B16", class: "gefahr",
                    onclick: move |_| { schliessen(); let mut u = a.ueber; u.set(Ueber::Loeschen(i5.clone())); }, "Endgültig löschen …" }
            }
        }
    }
}

// ---- centre: orchestrator pane or worker panes ----

#[allow(clippy::too_many_lines)]
fn mitte(a: A, zeilenhoehe: usize) -> Element {
    let k = a.kern.read();
    let s = k.aktive_sitzung().cloned();
    let aktiv = k.aktiv.clone();
    let bild = k.bild.clone();
    let fehler = k.fehler.clone();
    let worker = k.worker_der_sitzung().filter(|_| k.zeigt("B27"));
    let dialog = aktiv
        .as_ref()
        .and_then(|id| k.dialoge.iter().find(|d| d.sitzung == *id).cloned());
    drop(k);
    let ansicht = if worker.is_some() {
        *a.ansicht.read()
    } else {
        Ansicht::Orchestrator
    };
    let fenster_offen = *a.ueber.read() == Ueber::Sitzungen;
    let modell = s.as_ref().map(|s| s.modell.clone()).unwrap_or_default();
    let terminal_name = aktiv.as_ref().map_or_else(
        || "Terminal, keine Sitzung".to_owned(),
        |id| format!("Terminal {id}"),
    );
    rsx! {
        main { id: "mitte",
            if ansicht == Ansicht::Worker {
                {worker_flaeche(a, worker.unwrap_or_default(), zeilenhoehe)}
            }
            div { class: "pane orchestrator", hidden: ansicht == Ansicht::Worker,
                // the head stands before the first choice too: the area of the terminal keeps its size, and the size of the first
                // `Abo` stays valid (SE5 point 2: no second `Groesse` right after it)
                div { class: if aktiv.is_some() { "pane-kopf aktiv" } else { "pane-kopf" }, "data-bw": if aktiv.is_some() { Some("B41") } else { None },
                    if aktiv.is_some() {
                        span { class: punkt_klasse(s.as_ref()) }
                        span { class: "pane-rolle", "Orchestrator" }
                        span { class: "pane-modell", "{modell}" }
                        // one pane: enlarging has no effect, as in today's app
                        // outside the tab order like the borderless buttons of today's app; the terminal follows the title row (SE2)
                        button { r#type: "button", class: "pane-knopf", tabindex: "-1", "aria-label": "Kachel vergrößern", title: "Diese Kachel allein zeigen",
                            {symbol_zoom()}
                        }
                        {sprechblase()}
                    }
                }
                {terminal(a, "terminal", "raster", "eingang", bild.as_ref(), &terminal_name, zeilenhoehe, None, true, ansicht == Ansicht::Worker)}
                if aktiv.is_none() {
                    div { id: "leer", "data-bw": "B34", "Keine Sitzung gewählt. Lege mit + eine an oder wähle links eine Sitzung." }
                }
            }
            p { class: "versteckt", id: "hinweis-f6", "F6 wechselt in den nächsten Bereich, Umschalt+F6 in den vorigen." }
            if let Some(d) = dialog {
                {dialog_teil(a, &d)}
            }
            div { class: "notiz-anker", {notiz_teil(a)} }
            if !fenster_offen {
                {fehlerfeld(fehler.as_ref())}
            }
        }
    }
}

/// The error field (SE7 point 5); it stands in the window "Sitzungen" under "Starten" while the window is open (B71).
pub(crate) fn fehlerfeld(fehler: Option<&(String, String)>) -> Element {
    rsx! {
        div { class: "fehler", id: "fehler", "data-testid": "fehler", "data-bw": "B71", hidden: fehler.is_none(),
            span { class: "fehler-marke", "aria-hidden": "true", "Abgelehnt" }
            span { class: "fehler-text", id: "fehler-text", "data-testid": "fehler-text",
                {fehler.map(|f| f.1.clone()).unwrap_or_default()} }
            code { class: "fehler-code", id: "fehler-code", "data-testid": "fehler-code",
                {fehler.map(|f| f.0.clone()).unwrap_or_default()} }
        }
    }
}

/// One terminal: the raster as text (SE4, SE7 point 3) and the input element for keys (SE1). The orchestrator terminal carries
/// the fixed selectors; a worker pane (`pane`, the session of its worker) carries none.
#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn terminal(
    a: A,
    id: &str,
    raster_id: &str,
    eingang_id: &str,
    bild: Option<&rasterauszug::Bild>,
    name: &str,
    zeilenhoehe: usize,
    pane: Option<String>,
    fest: bool,
    versteckt: bool,
) -> Element {
    let worker_pane = pane.is_some();
    let cursor = bild.map_or((0, 0, false), |b| {
        (b.cursor.zeile, b.cursor.spalte, b.cursor.sichtbar)
    });
    let data_cursor = format!("{},{},{}", cursor.0, cursor.1, cursor.2);
    let cursor_stil = format!("left:{}ch;top:{}px;", cursor.1, cursor.0 * zeilenhoehe);
    let cursor_zeigen = bild.is_some() && cursor.2;
    let invers = bild.is_some_and(|b| b.invers);
    let zeilen: Vec<Vec<rasterauszug::Lauf>> = bild.map(|b| b.lines.clone()).unwrap_or_default();
    let raster_id_k = raster_id.to_owned();
    let eingang_k = eingang_id.to_owned();
    rsx! {
        section { class: if invers { "terminal invers" } else { "terminal" }, id: "{id}",
            "data-testid": if fest { Some("terminal-raster") } else { None },
            "data-cursor": if fest { Some(data_cursor.clone()) } else { None },
            "aria-label": "{name}", "data-bw": "B35", hidden: versteckt,
            div { class: "raster", id: "{raster_id}", "data-bw": "B39",
                onresize: move |_| {
                    if worker_pane { worker_messen(a); return; }
                    let g = messen();
                    let mut k = a.kern;
                    let w = k.write().groesse(g);
                    wirken(a, w);
                },
                onclick: move |_| {
                    let leer = web_sys::window()
                        .and_then(|w| w.get_selection().ok().flatten())
                        .is_none_or(|s| s.is_collapsed());
                    if leer { fokus(&eingang_k); }
                },
                div { class: "raster-inhalt", "data-bw": "B38",
                    for (i, z) in zeilen.iter().enumerate() {
                        div { key: "{i}", class: "raster-zeile",
                            "data-testid": if fest { Some(format!("terminal-zeile-{i}")) } else { None },
                            for l in z.iter() {
                                if zeile::vorgabe(l) {
                                    "{l.text}"
                                } else {
                                    {lauf(l)}
                                }
                            }
                        }
                    }
                    div { class: "cursor", "aria-hidden": "true", hidden: !cursor_zeigen, style: "{cursor_stil}" }
                    textarea { class: "eingang", id: "{eingang_id}", "data-bw": "B36",
                        "aria-label": "{name}", "aria-roledescription": "Terminal",
                        "aria-describedby": "hinweis-f6", autocapitalize: "off", autocomplete: "off",
                        "autocorrect": "off", spellcheck: "false", rows: "1", style: "{cursor_stil}",
                        onfocus: move |_| { let mut k = a.kern; if k.peek().pane != pane { k.write().pane.clone_from(&pane); } },
                        onkeydown: move |e: KeyboardEvent| terminal_taste(a, &e),
                        oncompositionend: move |e: CompositionEvent| {
                            let w = a.kern.read().text(&e.data().data());
                            wirken(a, w);
                            leeren(&raster_id_k);
                        },
                        oninput: move |e: FormEvent| terminal_eingabe(a, &e),
                        onpaste: move |e: ClipboardEvent| {
                            e.prevent_default();
                            let t = e.data().downcast::<web_sys::Event>()
                                .and_then(|x| x.dyn_ref::<web_sys::ClipboardEvent>().cloned())
                                .and_then(|x| x.clipboard_data())
                                .and_then(|d| d.get_data("text/plain").ok())
                                .unwrap_or_default();
                            let w = a.kern.read().einfuegen(&t);
                            wirken(a, w);
                        },
                    }
                    if fest {
                        span { class: "messung", id: "messung", "aria-hidden": "true", "MMMMMMMMMM" }
                    }
                }
            }
        }
    }
}

/// The worker panes of the session (B27, B30 to B33, L1, L2): tiled, six per tab; a zoomed pane fills the area.
#[allow(clippy::too_many_lines)]
fn worker_flaeche(a: A, worker: Vec<Agent>, zeilenhoehe: usize) -> Element {
    let k = a.kern.read();
    // each worker with the number of its raster (DI-L2 point 1); a worker without a number has no picture
    let mit_raster: Vec<(Agent, u8)> = worker
        .into_iter()
        .filter_map(|w| k.worker_nr(&w).map(|n| (w, n)))
        .collect();
    let tabs = mit_raster.len().div_ceil(JE_TAB).max(1);
    let tab = (*a.worker_tab.read()).min(tabs - 1);
    let gezoomt = *a.gezoomt.read();
    let sichtbar: Vec<(Agent, u8)> = match gezoomt {
        Some(z) if mit_raster.iter().any(|(_, n)| *n == z) => mit_raster
            .iter()
            .filter(|(_, n)| *n == z)
            .cloned()
            .collect(),
        _ => mit_raster
            .iter()
            .skip(tab * JE_TAB)
            .take(JE_TAB)
            .cloned()
            .collect(),
    };
    let spalten = if sichtbar.len() > 2 { 2 } else { 1 };
    // the frame of the active tile only where there is more than one tile (as today's app)
    let kacheln_klasse = if sichtbar.len() > 1 {
        "kacheln mehrere"
    } else {
        "kacheln"
    };
    let raster_stil = format!("grid-template-columns:repeat({spalten},minmax(0,1fr));");
    let bilder: Vec<Option<rasterauszug::Bild>> = sichtbar
        .iter()
        .map(|(_, n)| k.abbild(*n).cloned())
        .collect();
    let fokus_pane = k.pane.clone();
    drop(k);
    rsx! {
        div { class: "worker-flaeche",
            if mit_raster.len() > JE_TAB {
                div { id: "tabstreifen", role: "tablist", "aria-label": "Worker-Tabs", "data-bw": "B30",
                    for t in 0..tabs {
                        button { key: "{t}", r#type: "button", role: "tab", class: if t == tab { "tab gewaehlt" } else { "tab" },
                            "aria-selected": if t == tab { "true" } else { "false" },
                            onclick: move |_| { let mut x = a.worker_tab; x.set(t); let mut z = a.gezoomt; z.set(None); },
                            "Tab {t + 1}" }
                    }
                    span { class: "hinweis", "data-bw": "B33", "{JE_TAB} Panes je Tab" }
                    div { class: "segment", role: "group", "aria-label": "Anordnung", "data-bw": "B31",
                        button { r#type: "button", class: if gezoomt.is_none() { "gewaehlt" } else { "" },
                            onclick: move |_| { let mut z = a.gezoomt; z.set(None); }, "Gekachelt" }
                    }
                    button { r#type: "button", class: "knopf-rand", "data-bw": "B32",
                        onclick: move |_| { let mut x = a.worker_tab; x.set(0); let mut z = a.gezoomt; z.set(None); }, "Ordnen" }
                }
            }
            if sichtbar.is_empty() {
                div { class: "leer-mitte", "data-bw": "B34", "Diese Sitzung hat keine laufenden Worker." }
            }
            div { class: kacheln_klasse, style: "{raster_stil}",
                for ((w, nr), b) in sichtbar.into_iter().zip(bilder) {
                    {
                        let aktiv = fokus_pane.as_deref() == Some(w.worker.as_str());
                        let name = format!("Terminal {}", w.rolle);
                        let tid = format!("w-terminal-{nr}");
                        let rid = format!("raster-w{nr}");
                        let eid = format!("eingang-w{nr}");
                        rsx! {
                            div { key: "{nr}", class: if aktiv { "pane worker aktiv" } else { "pane worker" },
                                div { class: if aktiv { "pane-kopf aktiv" } else { "pane-kopf" }, "data-bw": "B41",
                                    span { class: agent_punkt(&w) }
                                    span { class: "pane-rolle", "{w.rolle}" }
                                    span { class: "pane-modell", "{w.modell}" }
                                    button { r#type: "button", class: "pane-knopf", "data-bw": "B31",
                                        "aria-label": if gezoomt == Some(nr) { "Alle Kacheln zeigen" } else { "Kachel vergrößern" },
                                        title: if gezoomt == Some(nr) { "Alle Kacheln zeigen" } else { "Diese Kachel allein zeigen" },
                                        onclick: move |_| { let mut z = a.gezoomt; let v = *z.peek(); z.set(if v == Some(nr) { None } else { Some(nr) }); },
                                        if gezoomt == Some(nr) { {symbol_verkleinern()} } else { {symbol_zoom()} }
                                    }
                                    {sprechblase()}
                                }
                                {terminal(a, &tid, &rid, &eid, b.as_ref(), &name, zeilenhoehe, Some(w.worker.clone()), false, false)}
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Measures the visible worker panes and gives the subscriptions to the core (L2).
fn worker_messen(a: A) {
    let Some(worker) = a.kern.peek().worker_der_sitzung() else {
        return;
    };
    if *a.ansicht.peek() != Ansicht::Worker {
        return;
    }
    let k = a.kern.peek();
    let panes: Vec<(String, (usize, usize))> = worker
        .iter()
        .filter_map(|w| k.worker_nr(w).map(|nr| (w.worker.clone(), nr)))
        .filter(|(_, nr)| element(&format!("raster-w{nr}")).is_some())
        .map(|(s, nr)| (s, messen_in(&format!("raster-w{nr}"))))
        .collect();
    drop(k);
    let mut k = a.kern;
    let w = k.write().worker_zeigen(&panes);
    wirken(a, w);
}

fn lauf(l: &rasterauszug::Lauf) -> Element {
    let a = zeile::aussehen(l.stil);
    let stil = a.stil();
    rsx! { span { class: "{a.klassen}", style: "{stil}", "{l.text}" } }
}

/// Empties the input of a terminal: what was typed went out as an event.
fn leeren(raster_id: &str) {
    let Some(d) = dokument() else { return };
    let sel = format!("#{raster_id} textarea");
    if let Ok(Some(t)) = d.query_selector(&sel)
        && let Ok(t) = t.dyn_into::<HtmlTextAreaElement>()
    {
        t.set_value("");
    }
}

fn terminal_taste(a: A, e: &KeyboardEvent) {
    let Some(k) = e.data().downcast::<web_sys::KeyboardEvent>().cloned() else {
        return;
    };
    let key = k.key();
    let t = tasten::Taste {
        key: &key,
        umschalt: k.shift_key(),
        strg: k.ctrl_key(),
        alt: k.alt_key(),
        meta: k.meta_key(),
        komponiert: k.is_composing(),
        key_code: k.key_code(),
    };
    let d = tasten::deute(&t);
    if d == Deutung::Nichts {
        return;
    }
    e.prevent_default();
    e.stop_propagation();
    if let Deutung::Fokus(r) = d {
        bereich_wechseln(a, r > 0);
        return;
    }
    let w = a.kern.read().taste(&d);
    wirken(a, w);
}

fn terminal_eingabe(a: A, e: &FormEvent) {
    let Some(ev) = e.data().downcast::<web_sys::Event>().cloned() else {
        return;
    };
    if let Some(ie) = ev.dyn_ref::<web_sys::InputEvent>() {
        if ie.is_composing() {
            return;
        }
        let art = ie.input_type();
        if art == "insertText" || art == "insertReplacementText" {
            let w = a.kern.read().text(&ie.data().unwrap_or_default());
            wirken(a, w);
        }
    }
    if let Some(t) = ev
        .target()
        .and_then(|t| t.dyn_into::<HtmlTextAreaElement>().ok())
    {
        t.set_value("");
    }
}

fn dialog_teil(a: A, d: &crate::kern::Dialog) -> Element {
    let optionen: Vec<(usize, String)> = d
        .optionen
        .iter()
        .cloned()
        .enumerate()
        .map(|(i, o)| (i + 1, o))
        .collect();
    rsx! {
        div { class: "dialog-leiste", "data-bw": "B42", role: "group", "aria-label": "Dialog offen: {d.art}",
            span { class: "dl-marke", "Dialog offen" }
            span { class: "dl-text", "{d.art}" }
            div { class: "dl-knoepfe",
                for (n, o) in optionen {
                    {
                        let d = d.clone();
                        rsx! {
                            button { key: "{n}", r#type: "button", class: "knopf-rand",
                                onclick: move |_| { let w = a.kern.read().dialog_antwort(&d, n); wirken(a, w); },
                                "{n}. {o}"
                            }
                        }
                    }
                }
            }
        }
    }
}

// ---- inspector ----

#[allow(clippy::too_many_lines)]
fn inspektor(a: A) -> Element {
    let k = a.kern.read();
    let ordner_da = k.zeigt("B52") && k.aktive_sitzung().and_then(Sitzung::ordner).is_some();
    drop(k);
    let reiter = if ordner_da {
        *a.reiter.read()
    } else {
        Reiter::Freigaben
    };
    rsx! {
        aside { id: "inspektor", "data-testid": "inspektor", "aria-label": "Inspektor",
            div { class: "insp-reiter", role: "tablist", "aria-label": "Inspektor", "data-bw": "B51",
                button { r#type: "button", role: "tab", id: "reiter-freigaben", class: "reiter",
                    "aria-selected": if reiter == Reiter::Freigaben { "true" } else { "false" },
                    "aria-controls": "blatt-freigaben", "aria-label": "Freigaben", title: "Freigaben",
                    onclick: move |_| { let mut r = a.reiter; r.set(Reiter::Freigaben); },
                    {symbol_schild(16)}
                }
                if ordner_da {
                    button { r#type: "button", role: "tab", id: "reiter-ordner", class: "reiter",
                        "aria-selected": if reiter == Reiter::Ordner { "true" } else { "false" },
                        "aria-controls": "blatt-ordner", "aria-label": "Ordner", title: "Ordner",
                        onclick: move |_| { let mut r = a.reiter; r.set(Reiter::Ordner); ordner_anfragen(a); },
                        {symbol_ordner()}
                    }
                }
            }
            if reiter == Reiter::Freigaben { {freigaben_blatt(a)} } else { {ordner_blatt(a)} }
        }
    }
}

fn ordner_anfragen(a: A) {
    let aktiv = a.kern.peek().aktiv.clone();
    if let Some(s) = aktiv {
        senden(&ziel::ordner_lesen(&s, ""));
    }
}

#[allow(clippy::too_many_lines)]
fn freigaben_blatt(a: A) -> Element {
    let k = a.kern.read();
    let anfragen = k.anfragen.clone();
    let verlauf = k.verlauf.clone();
    let b07 = k.zeigt("B07") && !verlauf.is_empty();
    let angehalten: Option<Vec<Agent>> = k.angehaltene_worker();
    // deciding only where the connection carries `Freigabe` (DI-E5 point 5): in the browser requests are only read
    let entscheiden = k.traegt(ziel::FREIGABE);
    drop(k);
    let n = anfragen.len();
    rsx! {
        div { class: "insp-blatt", id: "blatt-freigaben", role: "tabpanel", "aria-labelledby": "reiter-freigaben",
            "data-testid": "freigaben-blatt", "data-bw": "B56",
            h3 { class: "blatt-titel", "Anträge " span { class: "blatt-zahl", "{n}" } }
            if anfragen.is_empty() { p { class: "blatt-leer", "Kein offener Antrag." } }
            for f in anfragen {
                {
                    let id = f.id.clone();
                    let id2 = f.id.clone();
                    let sitzung = f.sitzung.clone();
                    let feld_id = format!("begruendung-{}", f.id);
                    let feld_id2 = feld_id.clone();
                    let feld_anzeige = feld_id.clone();
                    rsx! {
                        div { key: "{f.id}", class: "antrag",
                            div { class: "antrag-kopf", span { class: "antrag-werkzeug", "{f.werkzeug}" } span { class: "antrag-wer", "{f.sitzung}" } }
                            code { class: "antrag-vorschau", "{f.vorschau}" }
                            if entscheiden {
                                input { r#type: "text", class: "feld", id: "{feld_anzeige}", placeholder: "Begründung (freiwillig)", "aria-label": "Begründung" }
                                div { class: "antrag-knoepfe",
                                    button { r#type: "button", class: "knopf-rand", onclick: move |_| waehle(a, &sitzung), "Zum Worker" }
                                    button { r#type: "button", class: "knopf-rand", "data-bw": "B07",
                                        onclick: move |_| freigabe_senden(&id, false, &feld_id), "Ablehnen" }
                                    button { r#type: "button", class: "knopf-voll", "data-bw": "B07",
                                        onclick: move |_| freigabe_senden(&id2, true, &feld_id2), "Freigeben" }
                                }
                            } else {
                                // without `Freigabe` in the list of the connection (`freigabe_nur_lokal`): where to decide, no buttons
                                p { class: "antrag-hinweis", "Freigeben im App-Fenster" }
                            }
                        }
                    }
                }
            }
            if let Some(w) = angehalten {
                h3 { class: "blatt-titel", "Angehaltene Worker " span { class: "blatt-zahl", "{w.len()}" } }
                if w.is_empty() { p { class: "blatt-leer", "Kein Worker wartet gerade auf eine Guard-Entscheidung." } }
                for x in w {
                    div { key: "{x.worker}", class: "worker-zeile", span { class: "punkt will" } span { class: "wname", "{x.rolle}" } span { class: "wmodell", "{x.modell}" } }
                }
            }
            if b07 {
                h3 { class: "blatt-titel", "Guard-Verlauf " span { class: "blatt-zahl", "{verlauf.len()}" } }
                for (i, x) in verlauf.into_iter().enumerate() {
                    div { key: "{i}", class: "verlauf-zeile", "data-bw": "B07",
                        span { class: if x.zustand == "erteilt" { "marke gut" } else { "marke schlecht" },
                            {verlauf_text(&x.zustand)} }
                        span { class: "wname", "{x.werkzeug}" }
                        if !x.begruendung.is_empty() { span { class: "wmodell", "{x.begruendung}" } }
                    }
                }
            }
        }
    }
}

/// The text of one line of the guard history (B07).
fn verlauf_text(zustand: &str) -> &'static str {
    match zustand {
        "erteilt" => "Freigabe erteilt",
        "abgelehnt" => "Freigabe abgelehnt",
        _ => "Frist abgelaufen",
    }
}

/// Sends `Freigabe` (DI-L6 point 4) without `nachweis`; the buttons show only when the connection carries it (DI-E5 point 5).
fn freigabe_senden(id: &str, erteilt: bool, feld: &str) {
    let begruendung = element(feld)
        .and_then(|e| e.dyn_into::<web_sys::HtmlInputElement>().ok())
        .map(|e| e.value())
        .unwrap_or_default();
    senden(&ziel::freigabe(id, erteilt, &begruendung));
}

fn ordner_blatt(a: A) -> Element {
    let k = a.kern.read();
    let s = k.aktive_sitzung().cloned();
    let ordner = k.ordner.clone();
    let treffer = k.treffer.clone();
    drop(k);
    let pfad = s
        .as_ref()
        .and_then(Sitzung::ordner)
        .unwrap_or_default()
        .to_owned();
    let id = s.map(|s| s.id).unwrap_or_default();
    let id2 = id.clone();
    let eintraege = ordner.filter(|o| o.sitzung == id).map(|o| o.eintraege);
    let treffer = treffer
        .filter(|t| t.sitzung == id && !t.muster.is_empty())
        .map(|t| (t.muster, t.treffer));
    let inhalt = match eintraege {
        Some(e) if e.is_empty() => rsx! { p { class: "blatt-leer", "Dieser Ordner ist leer." } },
        Some(e) => rsx! {
            ul { class: "ordner-liste", "aria-label": "Inhalt des Arbeitsordners",
                for x in e {
                    li { key: "{x.name}", class: if x.ordner() { "eintrag ordner" } else { "eintrag datei" },
                        if x.ordner() { {symbol_ordner()} } else { {symbol_datei()} }
                        span { "{x.name}" }
                    }
                }
            }
        },
        None => rsx! { p { class: "blatt-leer", "Der Inhalt wird geladen." } },
    };
    rsx! {
        div { class: "insp-blatt", id: "blatt-ordner", role: "tabpanel", "aria-labelledby": "reiter-ordner",
            "data-testid": "ordner-blatt", "data-bw": "B52",
            div { class: "ordner-pfad", title: "{pfad}", span { dir: "ltr", "{pfad}" } }
            input { r#type: "search", class: "feld suche", id: "inhalt-suchen", placeholder: "Im Inhalt suchen", "aria-label": "Im Inhalt suchen",
                "data-bw": "B53",
                onkeydown: move |e: KeyboardEvent| {
                    if e.key() == Key::Enter {
                        let m = element("inhalt-suchen").and_then(|x| x.dyn_into::<web_sys::HtmlInputElement>().ok()).map(|x| x.value()).unwrap_or_default();
                        senden(&ziel::inhalt_suchen(&id2, &m));
                    }
                },
            }
            if let Some((m, t)) = treffer {
                p { class: "blatt-leer", "{t.len()} Treffer für „{m}“" }
                for (i, x) in t.into_iter().enumerate() {
                    div { key: "{i}", class: "treffer", span { class: "treffer-pfad", "{x.pfad}:{x.zeile}" } code { "{x.auszug}" } }
                }
            } else {
                {inhalt}
            }
        }
    }
}

// ---- worker list (B29) ----

fn workerliste(a: A) -> Element {
    let worker = a.kern.read().worker_der_sitzung().unwrap_or_default();
    let n = worker.len();
    // the bubble hangs under the pill (B26) with its tip on the middle of the pill, like today
    let lage = element("statuspille").map_or_else(String::new, |p| {
        let r = p.get_bounding_client_rect();
        let mitte = r.left() + r.width() / 2.0;
        let links = (mitte - 165.0).max(8.0);
        format!(
            "left:{links:.0}px;right:auto;top:{:.0}px;--spitze:{:.0}px;",
            r.bottom() + 10.0,
            mitte - links
        )
    });
    rsx! {
        div { class: "menue-grund", onclick: move |_| { let mut u = a.ueber; u.set(Ueber::Nichts); } }
        div { class: "blase mit-spitze", id: "worker-liste", role: "dialog", "aria-label": "Worker", "data-testid": "worker-liste", "data-bw": "B29",
            style: "{lage}", tabindex: "-1",
            onkeydown: move |e: KeyboardEvent| falle("worker-liste", &e),
            onmounted: move |_| fokus("worker-liste"),
            div { class: "blase-kopf",
                h3 { "Worker " span { class: "blatt-zahl", "{n}" } }
            }
            if worker.is_empty() { p { class: "blatt-leer", "Diese Sitzung hat keine Worker." } }
            for w in worker {
                div { key: "{w.worker}", class: "blase-zeile",
                    span { class: agent_punkt(&w) }
                    div { class: "blase-text",
                        span { class: "wname", "{w.rolle}" } " " span { class: "wmodell", "{w.modell}" }
                    }
                    span { class: "wzustand", "{zustand_wort(&w.zustand)}" }
                }
            }
            p { class: "blase-fuss", "data-bw": "B76", "Escape schließt" }
        }
    }
}

fn notiz_teil(a: A) -> Element {
    let Some((n, t)) = a.notiz.read().clone() else {
        return rsx! { div { id: "notiz", "data-bw": "B67", hidden: true } };
    };
    rsx! { div { id: "notiz", key: "{n}", "data-bw": "B67", class: "an", "aria-hidden": "true", "{t}" } }
}

// ---- symbols: outlines drawn here, 12 to 18 px, stroke `currentColor` ----

pub(crate) fn symbol_plus() -> Element {
    rsx! { svg { width: "14", height: "14", "viewBox": "0 0 16 16", fill: "none", stroke: "currentColor", "stroke-width": "1.5", "stroke-linecap": "round", "aria-hidden": "true",
    path { d: "M8 3v10M3 8h10" } } }
}
fn symbol_leiste() -> Element {
    rsx! { svg { width: "18", height: "18", "viewBox": "0 0 20 20", fill: "none", stroke: "currentColor", "stroke-width": "1.3", "aria-hidden": "true",
    rect { x: "2.5", y: "4", width: "15", height: "12", rx: "2.5" } path { d: "M7.5 4v12M4.3 7h1.6M4.3 9.5h1.6M4.3 12h1.6" } } }
}
pub(crate) fn symbol_schild(g: u32) -> Element {
    rsx! { svg { width: "{g}", height: "{g}", "viewBox": "0 0 16 16", fill: "none", stroke: "currentColor", "stroke-width": "1.4", "stroke-linecap": "round", "stroke-linejoin": "round", "aria-hidden": "true",
    path { d: "M8 1.8l5.2 2v4c0 3.1-2.2 5.3-5.2 6.4-3-1.1-5.2-3.3-5.2-6.4v-4z" } path { d: "M5.8 8.1l1.6 1.6 2.9-3.1" } } }
}
fn symbol_inspektor() -> Element {
    rsx! { svg { width: "18", height: "18", "viewBox": "0 0 20 20", fill: "none", stroke: "currentColor", "stroke-width": "1.3", "aria-hidden": "true",
    rect { x: "2.5", y: "4", width: "15", height: "12", rx: "2.5" } path { d: "M12.5 4v12M14.3 7h1.6M14.3 9.5h1.6M14.3 12h1.6" } } }
}
fn symbol_zoom() -> Element {
    rsx! { svg { width: "13", height: "13", "viewBox": "0 0 16 16", fill: "none", stroke: "currentColor", "stroke-width": "1.4", "stroke-linecap": "round", "aria-hidden": "true",
    path { d: "M9.5 2.5h4v4M6.5 13.5h-4v-4M13.5 2.5L9 7M2.5 13.5L7 9" } } }
}
pub(crate) fn symbol_ordner() -> Element {
    rsx! { svg { width: "16", height: "16", "viewBox": "0 0 16 16", fill: "none", stroke: "currentColor", "stroke-width": "1.3", "stroke-linejoin": "round", "aria-hidden": "true",
    path { d: "M2 4.2h4l1.4 1.5H14v6.8H2z" } } }
}
fn symbol_verkleinern() -> Element {
    rsx! { svg { width: "13", height: "13", "viewBox": "0 0 16 16", fill: "none", stroke: "currentColor", "stroke-width": "1.4", "stroke-linecap": "round", "aria-hidden": "true",
    path { d: "M13.5 6.5h-4v-4M2.5 9.5h4v4M9.5 6.5L14 2M6.5 9.5L2 14" } } }
}
/// The chat bubble of a pane head (B41): locked as in today's app, its target comes with the chat view after the pilot.
fn sprechblase() -> Element {
    rsx! {
        button { r#type: "button", class: "pane-knopf", disabled: true, "aria-label": "Als Gespräch zeigen, noch nicht verfügbar",
            title: "Als Gespräch zeigen – kommt mit der Chat-Bühne",
            svg { width: "14", height: "14", "viewBox": "0 0 16 16", fill: "none", stroke: "currentColor", "stroke-width": "1.3", "stroke-linejoin": "round", "aria-hidden": "true",
                path { d: "M3 3h10a1 1 0 0 1 1 1v6a1 1 0 0 1-1 1H7.5L4.5 13.5V11H3a1 1 0 0 1-1-1V4a1 1 0 0 1 1-1z" } }
        }
    }
}
fn symbol_datei() -> Element {
    rsx! { svg { width: "16", height: "16", "viewBox": "0 0 16 16", fill: "none", stroke: "currentColor", "stroke-width": "1.3", "stroke-linejoin": "round", "aria-hidden": "true",
    path { d: "M4 2h5l3 3v9H4zM9 2v3h3" } } }
}
pub(crate) fn symbol_kreuz() -> Element {
    rsx! { svg { width: "14", height: "14", "viewBox": "0 0 16 16", fill: "none", stroke: "currentColor", "stroke-width": "1.5", "stroke-linecap": "round", "aria-hidden": "true",
    path { d: "M4 4l8 8M12 4l-8 8" } } }
}
/// The gear: eight teeth, tip circle 8, root circle 6.1, hub 2.6, computed for a 20 by 20 box.
pub(crate) fn symbol_zahnrad(groesse: u32) -> Element {
    rsx! { svg { width: "{groesse}", height: "{groesse}", "viewBox": "0 0 20 20", fill: "none", stroke: "currentColor", "stroke-width": "1.3", "stroke-linejoin": "round", "aria-hidden": "true",
    path { d: "M8.84 4.01 L9.03 2.06 L10.97 2.06 L11.16 4.01 L13.41 4.94 L14.93 3.70 L16.30 5.07 L15.06 6.59 L15.99 8.84 L17.94 9.03 L17.94 10.97 L15.99 11.16 L15.06 13.41 L16.30 14.93 L14.93 16.30 L13.41 15.06 L11.16 15.99 L10.97 17.94 L9.03 17.94 L8.84 15.99 L6.59 15.06 L5.07 16.30 L3.70 14.93 L4.94 13.41 L4.01 11.16 L2.06 10.97 L2.06 9.03 L4.01 8.84 L4.94 6.59 L3.70 5.07 L5.07 3.70 L6.59 4.94 Z" }
    circle { cx: "10", cy: "10", r: "2.6" } } }
}
