//! The visible view: the code view of today's workbench (A157 stage 2a) as Dioxus components over [`Kern`].
//!
//! Layout, sizes, colours and texts follow appendix A of the inventory (`werkbank.css`, `thema.ts` and `texte.ts` on
//! `main`); no code of Electron or of the Mac app is used. Every operating path B01 to B77 that belongs to stage 2a
//! has a visible element with the attribute `data-bw`. A path without a route in the new build is a disabled control
//! whose name and hint say "noch nicht angebunden".

// The expansion of `rsx!` names event paths that the prelude already imports, and the handlers of a component bind
// many one-letter names (`a`, `e`, `k`, `w`) by the convention of this file; both lints fire on generated code.
// The handle `A` holds nine `Copy` signals (288 bytes); it is copied into every handler on purpose.
#![allow(
    unused_qualifications,
    clippy::many_single_char_names,
    clippy::large_types_passed_by_value
)]

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use dioxus::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{AbortController, HtmlElement, HtmlInputElement, HtmlTextAreaElement};

use crate::kern::{Kern, Sitzung, Verbindung, Wirkung, klemme};
use crate::netz::{self, Sender, Zerleger};
use crate::start::Wahlfeld;
use crate::tasten::{self, Deutung};
use crate::zeile;

/// The hint of every path without a route.
pub const NNA: &str = "noch nicht angebunden";
/// The height of a terminal line in pixels (`.raster-zeile`, `ansicht.css`).
const ZEILE_PX: usize = 17;
/// The most announcements kept in the live region.
const BEHALTEN: usize = 20;

thread_local! {
    static SENDER: RefCell<Option<Rc<Sender>>> = const { RefCell::new(None) };
    static ABBRUCH: RefCell<Option<AbortController>> = const { RefCell::new(None) };
    static TOKEN: RefCell<Option<String>> = const { RefCell::new(None) };
    static BEENDET: Cell<bool> = const { Cell::new(false) };
}

/// Which sheet lies over the stage.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Blatt {
    Zu,
    Freigaben,
    Worker,
    Einstellungen,
}

/// The reactive parts of the view; all of them are `Copy`.
#[derive(Clone, Copy)]
struct A {
    kern: Signal<Kern>,
    blatt: Signal<Blatt>,
    menue: Signal<Option<(String, f64, f64)>>,
    notiz: Signal<Option<(u32, String)>>,
    ansagen: Signal<Vec<(u32, String)>>,
    zaehler: Signal<u32>,
    freigabe_i: Signal<usize>,
    modellwahl: Signal<bool>,
    seite: Signal<usize>,
}

fn dokument() -> Option<web_sys::Document> {
    web_sys::window().and_then(|w| w.document())
}

fn element(id: &str) -> Option<HtmlElement> {
    dokument()?
        .get_element_by_id(id)?
        .dyn_into::<HtmlElement>()
        .ok()
}

fn fokus(id: &str) {
    if let Some(e) = element(id) {
        let o = web_sys::FocusOptions::new();
        o.set_prevent_scroll(true);
        let _ = e.focus_with_options(&o);
    }
}

fn px(cs: &web_sys::CssStyleDeclaration, name: &str) -> f64 {
    cs.get_property_value(name)
        .ok()
        .and_then(|v| v.trim_end_matches("px").parse().ok())
        .unwrap_or(0.0)
}

/// Columns and rows that fit into the area of the raster (SE5 point 2).
fn messen() -> (usize, usize) {
    let (Some(w), Some(r), Some(m)) = (web_sys::window(), element("raster"), element("messung"))
    else {
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

/// The tab stops outside the terminal, in document order.
fn haltestellen() -> Vec<HtmlElement> {
    let Some(liste) =
        dokument().and_then(|d| d.query_selector_all("button, input, select, textarea").ok())
    else {
        return Vec::new();
    };
    (0..liste.length())
        .filter_map(|i| liste.item(i))
        .filter_map(|n| n.dyn_into::<HtmlElement>().ok())
        .filter(|e| {
            e.get_attribute("disabled").is_none()
                && e.tab_index() >= 0
                && e.closest("[hidden]").ok().flatten().is_none()
        })
        .collect()
}

/// F6 and Shift+F6 leave the terminal (SE2 point 4).
fn fokus_wechsel(richtung: i8) {
    let halte = haltestellen();
    let Some(i) = halte.iter().position(|e| e.id() == "eingang") else {
        return;
    };
    let ziel = if richtung > 0 {
        i.checked_add(1)
    } else {
        i.checked_sub(1)
    };
    if let Some(e) = ziel.and_then(|z| halte.get(z)) {
        let _ = e.focus();
    }
}

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
    wirken(a, w);
}

/// Carries out the effects of an event.
fn wirken(a: A, wirkungen: Vec<Wirkung>) {
    for w in wirkungen {
        match w {
            Wirkung::Senden(v) => SENDER.with(|s| {
                if let Some(s) = s.borrow().as_ref() {
                    s.sende(&v);
                }
            }),
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
            Wirkung::FokusStart => fokus("wahl-harness"),
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

fn waehle(a: A, id: &str) {
    let g = messen();
    let mut k = a.kern;
    let w = k.write().waehle(id, g);
    wirken(a, w);
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
        blatt: use_signal(|| Blatt::Zu),
        menue: use_signal(|| None),
        notiz: use_signal(|| None),
        ansagen: use_signal(Vec::new),
        zaehler: use_signal(|| 0),
        freigabe_i: use_signal(|| 0),
        modellwahl: use_signal(|| false),
        seite: use_signal(|| 0),
    };
    use_hook(move || {
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

    let blatt = *a.blatt.read();
    rsx! {
        div {
            id: "rahmen",
            onkeydown: move |e: KeyboardEvent| {
                if e.key() == Key::Escape {
                    let mut b = a.blatt;
                    let mut m = a.menue;
                    let offen = *b.peek() != Blatt::Zu || m.peek().is_some();
                    if offen {
                        e.prevent_default();
                        b.set(Blatt::Zu);
                        m.set(None);
                    }
                }
            },
            {titelleiste(a)}
            {kanalwarnung(a)}
            {freigabeleiste(a)}
            div { id: "app",
                {links(a)}
                div { class: "griff", id: "griff-links", "data-bw": "B24", title: "Breite der Leiste ziehen: {NNA}", "aria-hidden": "true" }
                {mitte(a)}
                div { class: "griff", id: "griff-rechts", "data-bw": "B57", title: "Breite des Inspektors ziehen: {NNA}", "aria-hidden": "true" }
                {rechts(a)}
                {links_fuss(a)}
                {kopf_rechts(a)}
            }
            if blatt == Blatt::Freigaben { {freigaben_blatt(a)} }
            if blatt == Blatt::Worker { {worker_blatt(a)} }
            if blatt == Blatt::Einstellungen { {einstellungen_blatt(a)} }
            {kontextmenue(a)}
            div { class: "versteckt", id: "ansagen", role: "log", "aria-live": "polite",
                for (n, t) in a.ansagen.read().iter().cloned() {
                    div { key: "{n}", "{t}" }
                }
            }
        }
    }
}

fn punkt_klasse(s: Option<&Sitzung>) -> &'static str {
    match s {
        Some(s) if s.laeuft => "punkt laeuft",
        Some(_) => "punkt aus",
        None => "punkt ruhig",
    }
}

fn titelleiste(a: A) -> Element {
    let k = a.kern.read();
    let s = k.aktive_sitzung().cloned();
    let anzahl = k.anfragen.len();
    let name = k
        .aktiv
        .clone()
        .unwrap_or_else(|| "Keine Sitzung gewählt".to_owned());
    drop(k);
    let abzeichen_name = format!("Freigaben: {anzahl} offen");
    rsx! {
        header { id: "titelleiste",
            button { r#type: "button", class: "knopf-schlicht nna", disabled: true, "data-bw": "B75",
                title: "Menü (Einstellungen, Neue Sitzung, Aktualisieren, Bearbeiten, Darstellung, Vollbild): {NNA}",
                "aria-label": "Menü, {NNA}",
                {symbol_menue()}
            }
            div { id: "kontext", "data-bw": "B02",
                span { class: punkt_klasse(s.as_ref()) }
                span { id: "k-projekt", class: "nna-text", title: "Projekt: {NNA}", "Projekt" }
                span { class: "teiler", "/" }
                span { id: "k-orch", "{name}" }
            }
            div { class: "segment modi", id: "modi", "data-bw": "B01",
                button { r#type: "button", class: "gewaehlt", disabled: true, "aria-pressed": "true",
                    "aria-label": "Code, gewählt", "Code" }
                button { r#type: "button", class: "nna", disabled: true, title: "Agents: {NNA}",
                    "aria-label": "Agents, {NNA}", "Agents" }
            }
            div { id: "titel-rechts",
                button { r#type: "button", id: "freigabe-abzeichen", "data-bw": "B03",
                    class: if anzahl == 0 { "leer" } else { "" },
                    "aria-label": "{abzeichen_name}",
                    onclick: move |_| { let mut b = a.blatt; b.set(Blatt::Freigaben); },
                    {symbol_schild()}
                    span { class: "zahl", "{anzahl}" }
                }
                button { r#type: "button", class: "knopf-schlicht nna", id: "insp-knopf", disabled: true, "data-bw": "B04",
                    "aria-pressed": "true", title: "Inspektor ein- und ausklappen: {NNA}",
                    "aria-label": "Inspektor ein- und ausklappen, {NNA}",
                    {symbol_inspektor()}
                }
            }
        }
    }
}

fn kanalwarnung(a: A) -> Element {
    let k = a.kern.read();
    let Verbindung::Zu(text) = k.verbindung else {
        return rsx! {};
    };
    rsx! {
        div { id: "kanalwarnung", "data-bw": "B05", "data-zustand": "zu",
            "Verbindung zum Kern fehlt: {text}"
        }
    }
}

fn freigabeleiste(a: A) -> Element {
    let k = a.kern.read();
    let n = k.anfragen.len();
    if n == 0 {
        return rsx! {};
    }
    let i = (*a.freigabe_i.read()).min(n - 1);
    let f = k.anfragen[i].clone();
    drop(k);
    let pos = format!("{} von {n}", i + 1);
    rsx! {
        div { id: "freigabeleiste", "data-bw": "B06",
            div { class: "fr-zeile",
                span { class: "fr-marke", {symbol_schild()} }
                span { class: "fr-text",
                    span { class: "fr-wer", "{f.sitzung}" }
                    " {f.werkzeug} "
                    span { class: "leise", "{f.vorschau}" }
                }
                div { class: "fr-blaettern",
                    button { r#type: "button", "aria-label": "Vorige Freigabe", disabled: i == 0,
                        onclick: move |_| { let mut x = a.freigabe_i; let v = *x.peek(); x.set(v.saturating_sub(1)); },
                        "‹" }
                    span { class: "fr-zaehlung", "{pos}" }
                    button { r#type: "button", "aria-label": "Nächste Freigabe", disabled: i + 1 >= n,
                        onclick: move |_| { let mut x = a.freigabe_i; let v = *x.peek(); x.set(v + 1); },
                        "›" }
                }
                div { class: "fr-knoepfe",
                    button { r#type: "button", class: "knopf-rand nna", disabled: true,
                        title: "Zu diesem Worker springen: {NNA}", "aria-label": "Zu diesem Worker springen, {NNA}", "Zum Worker" }
                    button { r#type: "button", class: "knopf-rand nna", disabled: true,
                        title: "Ablehnen: {NNA}", "aria-label": "Ablehnen, {NNA}", "Ablehnen" }
                    button { r#type: "button", class: "knopf-voll nna", disabled: true,
                        title: "Freigeben: {NNA} (der Nachweis der Person fehlt in der Ansicht)",
                        "aria-label": "Freigeben, {NNA}", "Freigeben" }
                }
            }
            div { class: "fr-begruendung",
                input { r#type: "text", disabled: true, class: "nna", placeholder: "Begründung (freiwillig)",
                    "aria-label": "Begründung, {NNA}", title: NNA }
            }
        }
    }
}

fn links(a: A) -> Element {
    let k = a.kern.read();
    let sitzungen = k.sitzungen.clone();
    let aktiv = k.aktiv.clone();
    drop(k);
    let anzahl = sitzungen.len();
    rsx! {
        nav { id: "links", "aria-label": "Projekte und Sitzungen",
            div { class: "leiste-kopf",
                span { class: "titel", "Projekte" }
                button { r#type: "button", id: "neue-session", "data-bw": "B08", "aria-label": "Neue Sitzung",
                    title: "Neue Sitzung",
                    onclick: move |_| {
                        if let Some(e) = element("neue-sitzung") { e.scroll_into_view(); }
                        fokus("wahl-harness");
                    },
                    {symbol_plus()}
                }
            }
            div { id: "sessions",
                if anzahl == 0 {
                    p { class: "baum-leer", "Noch keine Sitzung. Lege mit + eine an." }
                } else {
                    div { class: "projekt-zeile nna", "data-bw": "B09", title: "Projekte nach Ordner: {NNA}",
                        span { class: "pfeil", {symbol_pfeil()} }
                        span { class: "name", "Projekt" }
                        span { class: "nna-marke", "{NNA}" }
                        span { class: "anzahl", "{anzahl}" }
                    }
                }
                ul { id: "liste", role: "list", "aria-label": "Sitzungen", "data-testid": "sitzung-liste", "data-bw": "B10",
                    for s in sitzungen {
                        {sitzungszeile(a, &s, aktiv.as_deref())}
                    }
                }
                div { class: "gespraech-zeile nna", "data-bw": "B18", title: "Gesprächssitzungen: {NNA}",
                    span { class: "zeilensymbol", {symbol_sprechblase()} }
                    span { class: "name", "Gespräche" }
                    span { class: "nna-marke", "{NNA}" }
                }
            }
        }
    }
}

fn sitzungszeile(a: A, s: &Sitzung, aktiv: Option<&str>) -> Element {
    let gewaehlt = aktiv == Some(s.id.as_str());
    let zustand = s.zustand_text();
    let teile = s.teile();
    let label = format!(
        "{}, {zustand}",
        [vec![s.id.as_str()], teile.clone()].concat().join(" ")
    );
    let mut zusatz: Vec<&str> = teile;
    if !s.kunde.is_empty() {
        zusatz.push(&s.kunde);
    }
    let zusatz = zusatz.join(" · ");
    let testid = format!("sitzung-{}", s.id);
    let id_klick = s.id.clone();
    let id_menue = s.id.clone();
    let id_taste = s.id.clone();
    let klasse = if gewaehlt {
        "orch-zeile gewaehlt"
    } else {
        "orch-zeile"
    };
    let datenzustand = if s.laeuft { "laeuft" } else { "beendet" };
    let code = (!s.laeuft).then(|| s.code.clone());
    rsx! {
        li { key: "{s.id}",
            button { r#type: "button", class: "{klasse}", "data-testid": "{testid}", "data-zustand": datenzustand,
                "data-code": code, "aria-label": "{label}", "aria-current": if gewaehlt { "true" } else { "false" },
                "aria-haspopup": "menu",
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
                id: "zeile-{s.id}",
                span { class: if s.laeuft { "punkt laeuft" } else { "punkt aus" } }
                span { class: "zeilenleib",
                    span { class: "name", "{s.id}" }
                    span { class: "zusatz", "{zustand}" if !zusatz.is_empty() { " · {zusatz}" } }
                }
            }
        }
    }
}

fn kontextmenue(a: A) -> Element {
    let Some((id, x, y)) = a.menue.read().clone() else {
        return rsx! {};
    };
    let laeuft = a
        .kern
        .read()
        .sitzungen
        .iter()
        .any(|s| s.id == id && s.laeuft);
    let id_ende = id.clone();
    let stil = format!("left:{x:.0}px;top:{y:.0}px;");
    let schliessen = move || {
        let mut m = a.menue;
        m.set(None);
    };
    rsx! {
        div { class: "menue-grund", onclick: move |_| schliessen() }
        div { class: "menue", role: "menu", "aria-label": "Sitzung {id}", style: "{stil}",
            onmounted: move |_| {
                if let Some(d) = dokument()
                    && let Ok(Some(e)) = d.query_selector(".menue button:not([disabled])")
                    && let Ok(e) = e.dyn_into::<HtmlElement>()
                {
                    let _ = e.focus();
                }
            },
            {menue_aus("B11", "Fortsetzen")}
            {menue_aus("B12", "Namen ändern …")}
            {menue_aus("B14", "Ordner im Finder zeigen")}
            button { r#type: "button", role: "menuitem", "data-bw": "B15", disabled: !laeuft,
                onclick: move |_| {
                    let w = a.kern.read().beenden(&id_ende);
                    schliessen();
                    wirken(a, w);
                },
                "Sitzung schließen"
            }
            {menue_aus("B16", "Endgültig löschen …")}
            {menue_aus("B17", "Wiederbeleben …")}
            {menue_aus("B19", "Umsortieren (Ziehen)")}
        }
    }
}

fn menue_aus(bw: &'static str, text: &'static str) -> Element {
    rsx! {
        button { r#type: "button", role: "menuitem", class: "nna", disabled: true, "data-bw": bw,
            title: "{text}: {NNA}", "aria-label": "{text}, {NNA}",
            "{text}" span { class: "nna-marke", "{NNA}" }
        }
    }
}

fn links_fuss(a: A) -> Element {
    let worker = a.kern.read().agenten.len();
    let worker_text = format!("{worker} Worker laufen");
    rsx! {
        div { id: "links-fuss",
            div { id: "maschinen",
                div { class: "masch nna", "data-bw": "B20", title: "Maschinenkarten: {NNA}",
                    div { class: "masch-kopf",
                        span { class: "punkt ruhig" }
                        span { class: "masch-name", "Diese Maschine" }
                        span { class: "nna-marke", "{NNA}" }
                    }
                    div { class: "masch-kurz", "Erreichbarkeit, Sitzungen, Worker, Prüfstand" }
                    label { class: "masch-schalter",
                        input { r#type: "checkbox", disabled: true, "aria-label": "Sitzungen laden, {NNA}" }
                        "Sitzungen laden"
                    }
                }
            }
            footer { id: "statusleiste",
                button { r#type: "button", class: "sz-stueck nna", disabled: true, "data-bw": "B21",
                    title: "Verbrauch und Verbrauchsfenster: {NNA}", "aria-label": "Verbrauch, Woche und erlaubt, {NNA}",
                    span { class: "sz-marke" }
                    "Budget: n. v. · "
                    span { "data-bw": "B73", "Verbrauch" }
                }
                div { class: "rechts",
                    div { class: "stueck", id: "st-worker", "data-bw": "B22", "{worker_text}" }
                    button { r#type: "button", class: "knopf nna", disabled: true, "data-bw": "B66",
                        title: "Meldung „Ergebnis da“: {NNA}", "aria-label": "Ergebnis da, {NNA}",
                        {symbol_glocke()}
                    }
                    button { r#type: "button", class: "knopf", id: "einstellungen-knopf", "data-bw": "B23",
                        "aria-label": "Einstellungen", title: "Einstellungen",
                        onclick: move |_| { let mut b = a.blatt; b.set(Blatt::Einstellungen); },
                        {symbol_zahnrad(14)}
                    }
                }
            }
        }
    }
}

/// The status pill. The workbench of today counts the living workers of the chosen session
/// (the renderer of the previous Electron application). The messages `Agenten` and
/// `Agentenzustand` name no session (`zustandskern/src/kern.rs:505-516`), so the number stands as not
/// bound; a click still opens the worker list (B29).
fn kopf_rechts(a: A) -> Element {
    rsx! {
        div { id: "ik-rechts",
            button { r#type: "button", class: "statuspille", "data-bw": "B26",
                "aria-expanded": if *a.blatt.read() == Blatt::Worker { "true" } else { "false" },
                title: "Laufende Worker dieser Sitzung: {NNA}. Öffnet die Worker-Liste.",
                "aria-label": "Worker-Liste. Laufende Worker dieser Sitzung: {NNA}",
                onclick: move |_| { let mut b = a.blatt; b.set(Blatt::Worker); },
                span { class: "punkt ruhig" }
                span { class: "zahl", "Worker" }
                span { class: "nna-marke", "{NNA}" }
            }
            div { class: "segment wahl-akzent", "data-bw": "B27",
                button { r#type: "button", class: "gewaehlt", disabled: true, "aria-pressed": "true",
                    "aria-label": "Orchestrator, gewählt", "Orchestrator" }
                button { r#type: "button", class: "nna", disabled: true, title: "Worker: {NNA}",
                    "aria-label": "Worker-Fläche, {NNA}", "Worker" }
            }
            button { r#type: "button", class: "knopf-schlicht nna", id: "ik-zahnrad", disabled: true, "data-bw": "B28",
                title: "Diese Sitzung einstellen: {NNA}", "aria-label": "Diese Sitzung einstellen, {NNA}",
                {symbol_zahnrad(16)}
            }
        }
    }
}

#[allow(clippy::too_many_lines)]
fn mitte(a: A) -> Element {
    let k = a.kern.read();
    let s = k.aktive_sitzung().cloned();
    let aktiv = k.aktiv.clone();
    let bild = k.bild.clone();
    let fehler = k.fehler.clone();
    let dialog = aktiv
        .as_ref()
        .and_then(|id| k.dialoge.iter().find(|d| d.sitzung == *id).cloned());
    drop(k);
    let name = aktiv
        .clone()
        .unwrap_or_else(|| "Keine Sitzung gewählt".to_owned());
    let herkunft = s
        .as_ref()
        .map(|s| {
            [
                s.harness.as_str(),
                s.modell.as_str(),
                s.route.as_str(),
                s.kunde.as_str(),
            ]
            .into_iter()
            .filter(|x| !x.is_empty())
            .collect::<Vec<_>>()
            .join(" · ")
        })
        .unwrap_or_default();
    let modell = s.as_ref().map(|s| s.modell.clone()).unwrap_or_default();
    let terminal_name = aktiv.as_ref().map_or_else(
        || "Terminal, keine Sitzung".to_owned(),
        |id| format!("Terminal {id}"),
    );
    let cursor = bild.as_ref().map_or((0, 0, false), |b| {
        (b.cursor.zeile, b.cursor.spalte, b.cursor.sichtbar)
    });
    let data_cursor = format!("{},{},{}", cursor.0, cursor.1, cursor.2);
    let cursor_stil = format!("left:{}ch;top:{}px;", cursor.1, cursor.0 * ZEILE_PX);
    let cursor_zeigen = bild.is_some() && cursor.2;
    let invers = bild.as_ref().is_some_and(|b| b.invers);
    let zeilen: Vec<Vec<rasterauszug::Lauf>> = bild.map(|b| b.lines).unwrap_or_default();
    rsx! {
        main { id: "mitte",
            div { id: "inhaltskopf", "data-bw": "B25",
                div { id: "ik-links",
                    div { id: "ik-name", "{name}" }
                    div { id: "ik-herkunft",
                        "{herkunft}"
                        span { class: "nna-text", title: "Ordner: {NNA}", if herkunft.is_empty() { "Ordner" } else { " · Ordner" } }
                    }
                }
            }
            div { id: "tabstreifen",
                div { id: "tabs", "data-bw": "B30",
                    button { r#type: "button", class: "tab gewaehlt nna", disabled: true, title: "Worker-Tabs: {NNA}",
                        "aria-label": "Tab 1, {NNA}", "Tab 1" span { class: "anzahl", "0" } }
                }
                div { id: "hinweis", class: "nna-text", "data-bw": "B33", title: "Kachelform: {NNA}", "6 Panes je Tab · 2×3" }
                div { class: "streifen-rechts",
                    div { class: "segment", "data-bw": "B31",
                        {segment_aus("Übersicht", false)}
                        {segment_aus("Gekachelt", true)}
                        {segment_aus("Gezoomt", false)}
                    }
                    button { r#type: "button", id: "ordnen", class: "nna", disabled: true, "data-bw": "B32",
                        title: "Ordnen: {NNA}", "aria-label": "Ordnen, {NNA}", "Ordnen" }
                }
            }
            div { id: "editorzeile", "data-bw": "B58",
                span { class: "etab gewaehlt", "Terminal" }
                {etab_aus("B59", "Datei öffnen")}
                {etab_aus("B63", "Diff")}
                div { class: "streifen-rechts",
                    {knopf_aus("B62", "Schnell öffnen ⌘P")}
                    {knopf_aus("B60", "Datei speichern")}
                    {knopf_aus("B61", "An Orchestrator senden")}
                    {knopf_aus("B64", "Einklappen")}
                }
            }
            div { id: "buehne",
                div { class: "panekasten",
                    div { class: "panekopf aktiv", "data-bw": "B41",
                        span { class: punkt_klasse(s.as_ref()) }
                        span { class: "pk-name", "{name}" }
                        span { class: "pk-modell", "{modell}" }
                        span { class: "pk-tokens nna-text", title: "Tokens: {NNA}", "Tokens" }
                        span { class: "pk-modell nna-text", "data-bw": "B40", title: "Rückblick über den Bildschirm hinaus: {NNA}", "Rückblick" }
                        button { r#type: "button", class: "pk-knopf nna", disabled: true, title: "Zoom: {NNA}",
                            "aria-label": "Zoom, {NNA}", {symbol_zoom()} }
                    }
                    section { class: if invers { "terminal invers" } else { "terminal" }, id: "terminal",
                        "data-testid": "terminal-raster", "data-cursor": "{data_cursor}", "aria-label": "{terminal_name}",
                        "data-bw": "B35",
                        div { class: "raster", id: "raster", "data-bw": "B39",
                            onresize: move |_| {
                                let g = messen();
                                let mut k = a.kern;
                                let w = k.write().groesse(g);
                                wirken(a, w);
                            },
                            onclick: move |_| {
                                let leer = web_sys::window()
                                    .and_then(|w| w.get_selection().ok().flatten())
                                    .is_none_or(|s| s.is_collapsed());
                                if leer { fokus("eingang"); }
                            },
                            div { class: "raster-inhalt", id: "raster-inhalt",
                                for (i, z) in zeilen.iter().enumerate() {
                                    div { key: "{i}", class: "raster-zeile", "data-testid": "terminal-zeile-{i}",
                                        for l in z.iter() {
                                            if zeile::vorgabe(l) {
                                                "{l.text}"
                                            } else {
                                                {lauf(l)}
                                            }
                                        }
                                    }
                                }
                                div { class: "cursor", id: "cursor", "aria-hidden": "true", hidden: !cursor_zeigen, style: "{cursor_stil}" }
                                textarea { class: "eingang", id: "eingang", "data-bw": "B36",
                                    "aria-label": "{terminal_name}", "aria-roledescription": "Terminal",
                                    "aria-describedby": "hinweis-f6", autocapitalize: "off", autocomplete: "off",
                                    "autocorrect": "off", spellcheck: "false", rows: "1", style: "{cursor_stil}",
                                    onkeydown: move |e: KeyboardEvent| terminal_taste(a, &e),
                                    oncompositionend: move |e: CompositionEvent| {
                                        let w = a.kern.read().text(&e.data().data());
                                        wirken(a, w);
                                        leeren();
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
                                span { class: "messung", id: "messung", "aria-hidden": "true", "MMMMMMMMMM" }
                            }
                        }
                        if aktiv.is_none() {
                            div { id: "leer", "data-bw": "B34", "Keine Sitzung. Lege mit + eine an oder wähle links ein Projekt." }
                        }
                    }
                    p { class: "versteckt", id: "hinweis-f6", "F6 verlässt das Terminal, Umschalt+F6 geht zurück." }
                }
                if let Some(d) = dialog {
                    {dialog_teil(a, &d)}
                }
            }
            div { class: "notiz-anker", {notiz_teil(a)} }
            form { class: "eingabezeile", id: "zeile-eingabe", "aria-label": "Eingabezeile", "data-bw": "B37",
                onsubmit: move |e: FormEvent| {
                    e.prevent_default();
                    let t = element("eingabe")
                        .and_then(|x| x.dyn_into::<HtmlInputElement>().ok())
                        .map(|x| { let v = x.value(); x.set_value(""); v })
                        .unwrap_or_default();
                    let w = a.kern.read().zeile_absenden(&t);
                    wirken(a, w);
                },
                input { class: "feld", id: "eingabe", "data-testid": "eingabe", r#type: "text", "aria-label": "Eingabe",
                    placeholder: "Text an das Programm", enterkeyhint: "send", autocapitalize: "off",
                    autocomplete: "off", "autocorrect": "off", spellcheck: "false" }
                button { class: "knopf-voll", r#type: "submit", "data-testid": "eingabe-senden", "data-bw": "B38",
                    title: "Einfügen im Terminal: Cmd+V oder Strg+Umschalt+V", "Senden" }
            }
            div { class: "fehler", id: "fehler", "data-testid": "fehler", "data-bw": "B71", hidden: fehler.is_none(),
                span { class: "fehler-marke", "aria-hidden": "true", "Abgelehnt" }
                span { class: "fehler-text", id: "fehler-text", "data-testid": "fehler-text",
                    {fehler.as_ref().map(|f| f.1.clone()).unwrap_or_default()} }
                code { class: "fehler-code", id: "fehler-code", "data-testid": "fehler-code",
                    {fehler.as_ref().map(|f| f.0.clone()).unwrap_or_default()} }
            }
        }
    }
}

fn lauf(l: &rasterauszug::Lauf) -> Element {
    let a = zeile::aussehen(l.stil);
    let stil = a.stil();
    rsx! { span { class: "{a.klassen}", style: "{stil}", "{l.text}" } }
}

/// Empties the input of the terminal: what was typed went out as an event.
fn leeren() {
    if let Some(t) = element("eingang").and_then(|e| e.dyn_into::<HtmlTextAreaElement>().ok()) {
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
        fokus_wechsel(r);
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
    leeren();
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
            span { class: "fr-marke", "Dialog offen" }
            span { class: "fr-text", "{d.art}" }
            div { class: "fr-knoepfe",
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

fn segment_aus(text: &'static str, gewaehlt: bool) -> Element {
    rsx! {
        button { r#type: "button", class: if gewaehlt { "gewaehlt nna" } else { "nna" }, disabled: true,
            title: "{text}: {NNA}", "aria-label": "{text}, {NNA}", "{text}" }
    }
}

fn etab_aus(bw: &'static str, text: &'static str) -> Element {
    rsx! {
        button { r#type: "button", class: "etab nna", disabled: true, "data-bw": bw,
            title: "{text}: {NNA}", "aria-label": "{text}, {NNA}", "{text}" }
    }
}

fn knopf_aus(bw: &'static str, text: &'static str) -> Element {
    rsx! {
        button { r#type: "button", class: "knopf-klein nna", disabled: true, "data-bw": bw,
            title: "{text}: {NNA}", "aria-label": "{text}, {NNA}", "{text}" }
    }
}

fn wahl(
    a: A,
    id: &'static str,
    testid: &'static str,
    beschriftung: &'static str,
    feld: Wahlfeld,
    setzen: fn(&mut Kern, String),
) -> Element {
    let wert = feld.wert.clone();
    rsx! {
        div { class: "feldgruppe",
            label { class: "beschriftung", r#for: id, "{beschriftung}" }
            select { class: "feld", id: id, name: id, "data-testid": testid,
                onchange: move |e: FormEvent| { let mut k = a.kern; setzen(&mut k.write(), e.value()); },
                for o in feld.eintraege {
                    option { key: "{o.wert}", value: "{o.wert}", selected: o.wert == wert, "{o.text}" }
                }
            }
        }
    }
}

#[allow(clippy::too_many_lines)]
fn rechts(a: A) -> Element {
    let k = a.kern.read();
    let f = k.start.clone();
    let sitzungen = k.sitzungen.clone();
    let routen = k.routen.clone();
    drop(k);
    let modellwahl = *a.modellwahl.read();
    let modelle: Vec<(String, String)> = f
        .modelle
        .iter()
        .map(|(r, m)| {
            (
                r.clone(),
                m.clone()
                    .unwrap_or_else(|| crate::start::MODELL_AUTO.to_owned()),
            )
        })
        .collect();
    let routen_zahl = routen.as_ref().and_then(|r| r.as_array().map(Vec::len));
    rsx! {
        aside { id: "rechts",
            section { class: "karte-start", id: "neue-sitzung", "aria-labelledby": "neue-sitzung-titel", "data-bw": "B68",
                h2 { class: "karte-titel", id: "neue-sitzung-titel", "Neue Sitzung" }
                div { class: "start-zusatz",
                    input { r#type: "text", class: "feld nna", disabled: true, placeholder: "Name (optional)",
                        "aria-label": "Name, {NNA}", title: "Name (optional): {NNA}" }
                    select { class: "feld nna", disabled: true, "aria-label": "Maschine, {NNA}", title: "Maschine: {NNA}",
                        option { "Diese Maschine" } }
                    div { class: "zeile",
                        input { r#type: "text", class: "feld nna", disabled: true,
                            placeholder: "Absoluter Pfad auf der gewählten Maschine …",
                            "aria-label": "Pfad auf fremder Maschine, {NNA}", title: NNA }
                        button { r#type: "button", class: "knopf-rand nna", disabled: true, title: "Prüfen: {NNA}",
                            "aria-label": "Prüfen, {NNA}", "Prüfen" }
                    }
                }
                form { class: "start", id: "start", "aria-label": "Sitzung starten",
                    onsubmit: move |e: FormEvent| {
                        e.prevent_default();
                        let Some(token) = TOKEN.with(|t| t.borrow().clone()) else { return; };
                        let start = a.kern.read().start.struktur();
                        spawn(async move {
                            let (status, antwort) = netz::starten(&token, &start).await;
                            let mut k = a.kern;
                            let w = k.write().start_antwort(status, antwort.as_ref());
                            wirken(a, w);
                        });
                    },
                    {wahl(a, "wahl-harness", "harness-wahl", "Harness", f.harness.clone(), |k, v| k.start.harness.wert = v)}
                    {wahl(a, "wahl-modell", "modell-wahl", "Modell", f.modell.clone(), |k, v| k.start.modell.wert = v)}
                    {wahl(a, "wahl-route", "route-wahl", "Route", f.route.clone(), |k, v| k.start.route_waehlen(&v))}
                    {wahl(a, "wahl-kunde", "kunde-wahl", "Kunde", f.kunde.clone(), |k, v| k.start.kunde.wert = v)}
                    div { class: "feldgruppe feldgruppe-breit",
                        label { class: "beschriftung", r#for: "wahl-arbeitsordner", "Arbeitsordner" }
                        input { class: "feld", id: "wahl-arbeitsordner", name: "arbeitsordner", "data-testid": "arbeitsordner-eingabe",
                            r#type: "text", autocapitalize: "off", autocomplete: "off", "autocorrect": "off", spellcheck: "false",
                            oninput: move |e: FormEvent| { let mut k = a.kern; k.write().start.arbeitsordner = e.value(); } }
                    }
                    button { class: "knopf-voll knopf-haupt", r#type: "submit", "data-testid": "sitzung-start", "Starten" }
                }
                button { r#type: "button", class: "knopf-rand breit", "data-bw": "B69",
                    "aria-expanded": if modellwahl { "true" } else { "false" },
                    onclick: move |_| { let mut m = a.modellwahl; let v = *m.peek(); m.set(!v); },
                    "Modell für diese Sitzung wählen …"
                }
                if modellwahl {
                    div { class: "modellwahl",
                        p { class: "leise", "Modelle je Route aus der Startwahl:" }
                        if modelle.is_empty() {
                            p { class: "leise", "Noch keine Startwahl vom Dienst." }
                        }
                        dl { class: "sk-liste",
                            for (r, m) in modelle {
                                dt { key: "{r}", "{r}" }
                                dd { "{m}" }
                            }
                        }
                        if let Some(n) = routen_zahl { p { class: "leise", "Routen des Kerns: {n}" } }
                        select { class: "feld nna", disabled: true, "aria-label": "Wie tief die Sitzung denkt, {NNA}",
                            title: NNA, option { "Denkstufe" } }
                        select { class: "feld nna", disabled: true, "aria-label": "Kontextfenster, {NNA}",
                            title: NNA, option { "Kontextfenster" } }
                    }
                }
                div { class: "fruehere", "data-bw": "B70",
                    h3 { class: "karte-untertitel", "Frühere Sitzungen" }
                    if sitzungen.is_empty() { p { class: "leise", "Noch keine." } }
                    for s in sitzungen {
                        {fruehere_zeile(a, &s)}
                    }
                }
            }
            section { class: "inspektor", "aria-label": "Inspektor",
                div { class: "insp-reiter", "data-bw": "B51",
                    {reiter_aus("Ordner", true, "B52")}
                    {reiter_aus("Aktivität", false, "B54")}
                    {reiter_aus("Protokolle", false, "B55")}
                }
                div { class: "insp-inhalt",
                    input { r#type: "search", class: "feld nna", disabled: true, "data-bw": "B53",
                        placeholder: "Inhalt durchsuchen (rg) …", "aria-label": "Inhalt durchsuchen, {NNA}", title: NNA }
                    p { class: "leise", "Ordnerbaum: {NNA}" }
                }
            }
        }
    }
}

fn reiter_aus(text: &'static str, gewaehlt: bool, bw: &'static str) -> Element {
    rsx! {
        button { r#type: "button", class: if gewaehlt { "knopf gewaehlt nna" } else { "knopf nna" }, disabled: true,
            "data-bw": bw, title: "{text}: {NNA}", "aria-label": "{text}, {NNA}",
            {symbol_ordner()}
            span { class: "beschriftung", "{text}" }
        }
    }
}

fn fruehere_zeile(a: A, s: &Sitzung) -> Element {
    let zustand = s.zustand_text();
    let id = s.id.clone();
    let name = format!("{} beenden", s.id);
    rsx! {
        div { key: "{s.id}", class: "frueher",
            span { class: if s.laeuft { "punkt laeuft" } else { "punkt aus" } }
            span { class: "name", "{s.id}" }
            span { class: "zusatz", "{zustand}" }
            button { r#type: "button", class: "knopf-klein nna", disabled: true, title: "Fortsetzen: {NNA}",
                "aria-label": "{s.id} fortsetzen, {NNA}", "Fortsetzen" }
            button { r#type: "button", class: "knopf-klein", disabled: !s.laeuft, "aria-label": "{name}",
                onclick: move |_| { let w = a.kern.read().beenden(&id); wirken(a, w); },
                "Beenden" }
            button { r#type: "button", class: "knopf-klein nna", disabled: true, title: "Modell wechseln: {NNA}",
                "aria-label": "Modell von {s.id} wechseln, {NNA}", "Modell wechseln" }
        }
    }
}

fn blatt_rahmen(a: A, titel: &'static str, bw: &'static str, inhalt: Element) -> Element {
    rsx! {
        div { class: "blatt-grund", onclick: move |_| { let mut b = a.blatt; b.set(Blatt::Zu); } }
        div { class: "blatt", role: "dialog", "aria-label": "{titel}", "data-bw": bw,
            onmounted: move |_| fokus("blatt-zu"),
            div { class: "sk-kopf",
                span { class: "sk-titel", "{titel}" }
                button { r#type: "button", id: "blatt-zu", class: "knopf-schlicht", "aria-label": "Schließen", "data-bw": "B76",
                    title: "Schließen (Esc)",
                    onclick: move |_| { let mut b = a.blatt; b.set(Blatt::Zu); },
                    {symbol_kreuz()}
                }
            }
            {inhalt}
        }
    }
}

fn freigaben_blatt(a: A) -> Element {
    let anfragen = a.kern.read().anfragen.clone();
    blatt_rahmen(
        a,
        "Freigaben",
        "B56",
        rsx! {
            h3 { class: "karte-untertitel", "Anträge" }
            if anfragen.is_empty() { p { class: "leise", "Keine offene Freigabe." } }
            for f in anfragen {
                div { key: "{f.id}", class: "fr-karte",
                    div { class: "fr-wer", "{f.sitzung} · {f.werkzeug}" }
                    div { class: "leise", "{f.vorschau}" }
                    div { class: "fr-knoepfe",
                        {knopf_aus("B56", "Pane zeigen")}
                        {knopf_aus("B56", "Annehmen")}
                        {knopf_aus("B56", "Ablehnen")}
                    }
                }
            }
            p { class: "leise nna-text", "data-bw": "B07", "Meldung „Freigabe erteilt“ oder „abgelehnt“ nach der Entscheidung: {NNA}" }
            h3 { class: "karte-untertitel", "Angehaltene Worker" }
            p { class: "leise nna-text", "{NNA}" }
            h3 { class: "karte-untertitel", "Guard-Verlauf" }
            p { class: "leise nna-text", "{NNA}" }
        },
    )
}

fn worker_blatt(a: A) -> Element {
    let agenten = a.kern.read().agenten.clone();
    blatt_rahmen(
        a,
        "Worker",
        "B29",
        rsx! {
            if agenten.is_empty() { p { class: "leise", "Keine Worker gesehen." } }
            for (w, z) in agenten {
                div { key: "{w}", class: "worker-zeile",
                    span { class: "wname", "{w}" }
                    span { class: "wmodell", "{z}" }
                }
            }
            p { class: "leise nna-text", "Rubriken Fertig, Nicht einsehbar, Unbekannt und Kontext in Prozent: {NNA}" }
        },
    )
}

const SEITEN: [&str; 7] = [
    "Sitzung",
    "Erlaubnisse",
    "Programme und Modelle",
    "Maschinen",
    "Aufsicht und Meldungen",
    "Aussehen",
    "Programm",
];

/// The options of the settings window (page, text, key of the settings file). Every option is off by default in the new
/// view (decision of the operator, 21.11.2026, addendum 2 point 4), also those that are on in Electron; themes
/// are choices.
const OPTIONEN: [(usize, &str, &str); 17] = [
    (0, "Beendete Sitzungen in der Leiste zeigen", "showStopped"),
    (
        0,
        "Terminal beim Schließen des Fensters beenden",
        "closeSessionOnWindowClose",
    ),
    (
        1,
        "Worker arbeiten ohne Rückfrage ihrer CLI",
        "workerSkipPermissions",
    ),
    (
        1,
        "Jeder Worker bekommt einen eigenen Arbeitsbaum",
        "workerWorktrees",
    ),
    (1, "Sicherungen vor jedem Befehl (Guards)", "guards"),
    (
        2,
        "Agenten-Programme automatisch aktuell halten",
        "harnessUpdateAuto",
    ),
    (
        2,
        "Modell-Kataloge von selbst abrufen",
        "modelDiscoveryAuto",
    ),
    (
        2,
        "Vorhersage für den Orchestrator",
        "orchestratorVorhersage",
    ),
    (2, "Vorhersage für Worker", "workerVorhersage"),
    (3, "Mobile Werkbank", "mobileServer"),
    (4, "Kontextwache läuft mit", "contextGuardAutostart"),
    (4, "Hauptfenster überwachen", "kontextwache.orchestrator.an"),
    (
        4,
        "Die Wache greift im Hauptfenster selbst ein",
        "kontextwache.orchestrator.eingreifen",
    ),
    (4, "Worker überwachen", "kontextwache.worker.an"),
    (
        4,
        "Die Wache greift bei Workern selbst ein",
        "kontextwache.worker.eingreifen",
    ),
    (
        4,
        "Die Wache schreibt Worker-Meldungen ins Hauptfenster",
        "guardMeldetWorkerStatus",
    ),
    (4, "Sich außerhalb des Fensters melden", "meldungen.an"),
];

fn einstellungen_blatt(a: A) -> Element {
    let werte = a.kern.read().einstellungen.clone();
    let seite = (*a.seite.read()).min(SEITEN.len() - 1);
    let paare: Vec<(String, String)> = werte
        .as_ref()
        .and_then(|w| w.as_object())
        .map(|o| o.iter().map(|(k, v)| (k.clone(), v.to_string())).collect())
        .unwrap_or_default();
    blatt_rahmen(
        a,
        "Einstellungen",
        "B72",
        rsx! {
            div { class: "segment seiten",
                for (i, t) in SEITEN.iter().enumerate() {
                    button { key: "{i}", r#type: "button", class: if i == seite { "gewaehlt" } else { "" },
                        "aria-pressed": if i == seite { "true" } else { "false" },
                        onclick: move |_| { let mut s = a.seite; s.set(i); },
                        "{t}" }
                }
            }
            div { class: "optionen",
                for (i, (_, text, schluessel)) in OPTIONEN.iter().enumerate().filter(|(_, o)| o.0 == seite) {
                    label { key: "{i}", class: "option nna", title: "{schluessel}: {NNA}",
                        input { r#type: "checkbox", disabled: true, checked: false,
                            "aria-label": "{text}, aus, {NNA}" }
                        span { "{text}" }
                        span { class: "nna-marke", "aus" }
                    }
                }
            }
            if seite == 5 {
                div { class: "feldgruppe", "data-bw": "B77",
                    label { class: "beschriftung", "Hell und dunkel" }
                    select { class: "feld nna", disabled: true, "aria-label": "Hell und dunkel, {NNA}", title: NNA,
                        option { "Wie das System" } }
                }
                div { class: "feldgruppe",
                    label { class: "beschriftung", "Terminal" }
                    select { class: "feld nna", disabled: true, "aria-label": "Thema des Terminals, {NNA}", title: NNA,
                        option { "Wie die Ansicht" } }
                }
            }
            h3 { class: "karte-untertitel", "Werte vom Dienst" }
            if paare.is_empty() { p { class: "leise", "Der Dienst hat noch keine Einstellungen geschickt." } }
            dl { class: "sk-liste",
                for (k, v) in paare {
                    dt { key: "{k}", "{k}" }
                    dd { "{v}" }
                }
            }
            p { class: "leise nna-text", "Ändern: {NNA} (im Lauf lehnt der Dienst EinstellungSetzen ab)" }
        },
    )
}

fn notiz_teil(a: A) -> Element {
    let Some((n, t)) = a.notiz.read().clone() else {
        return rsx! { div { id: "notiz", "data-bw": "B67", hidden: true } };
    };
    rsx! { div { id: "notiz", key: "{n}", "data-bw": "B67", class: "an", "aria-hidden": "true", "{t}" } }
}

// ---- symbols: outlines drawn here, 12 to 16 px, stroke `currentColor` (appendix A.4) ----

fn symbol_plus() -> Element {
    rsx! { svg { width: "13", height: "13", "viewBox": "0 0 16 16", fill: "none", stroke: "currentColor", "stroke-width": "1.5", "stroke-linecap": "round", "aria-hidden": "true",
    path { d: "M8 3v10M3 8h10" } } }
}
fn symbol_menue() -> Element {
    rsx! { svg { width: "15", height: "15", "viewBox": "0 0 16 16", fill: "none", stroke: "currentColor", "stroke-width": "1.4", "stroke-linecap": "round", "aria-hidden": "true",
    path { d: "M3 4.5h10M3 8h10M3 11.5h10" } } }
}
fn symbol_schild() -> Element {
    rsx! { svg { width: "12", height: "12", "viewBox": "0 0 16 16", fill: "none", stroke: "currentColor", "stroke-width": "1.5", "stroke-linecap": "round", "aria-hidden": "true",
    path { d: "M8 1.8l5.2 2v4c0 3.1-2.2 5.3-5.2 6.4-3-1.1-5.2-3.3-5.2-6.4v-4z" } } }
}
fn symbol_inspektor() -> Element {
    rsx! { svg { width: "15", height: "15", "viewBox": "0 0 16 16", fill: "none", stroke: "currentColor", "stroke-width": "1.3", "aria-hidden": "true",
    rect { x: "1.8", y: "2.8", width: "12.4", height: "10.4", rx: "2" } path { d: "M10 2.8v10.4" } } }
}
fn symbol_pfeil() -> Element {
    rsx! { svg { width: "10", height: "10", "viewBox": "0 0 16 16", fill: "none", stroke: "currentColor", "stroke-width": "1.6", "stroke-linecap": "round", "aria-hidden": "true",
    path { d: "M6 3.5l4.5 4.5L6 12.5" } } }
}
fn symbol_sprechblase() -> Element {
    rsx! { svg { width: "13", height: "13", "viewBox": "0 0 16 16", fill: "none", stroke: "currentColor", "stroke-width": "1.4", "stroke-linejoin": "round", "aria-hidden": "true",
    path { d: "M2.5 3.5h11v7h-6l-3 2.5v-2.5h-2z" } } }
}
fn symbol_glocke() -> Element {
    rsx! { svg { width: "14", height: "14", "viewBox": "0 0 16 16", fill: "none", stroke: "currentColor", "stroke-width": "1.4", "stroke-linejoin": "round", "aria-hidden": "true",
    path { d: "M4 11V7.2a4 4 0 0 1 8 0V11l1.2 1.3H2.8zM6.6 13.8a1.5 1.5 0 0 0 2.8 0" } } }
}
fn symbol_zoom() -> Element {
    rsx! { svg { width: "12", height: "12", "viewBox": "0 0 16 16", fill: "none", stroke: "currentColor", "stroke-width": "1.5", "stroke-linecap": "round", "aria-hidden": "true",
    path { d: "M9.5 2.5h4v4M6.5 13.5h-4v-4M13.5 2.5L9 7M2.5 13.5L7 9" } } }
}
fn symbol_ordner() -> Element {
    rsx! { svg { width: "16", height: "16", "viewBox": "0 0 16 16", fill: "none", stroke: "currentColor", "stroke-width": "1.3", "stroke-linejoin": "round", "aria-hidden": "true",
    path { d: "M2 4.5h4l1.5 1.5H14v6.5H2z" } } }
}
fn symbol_kreuz() -> Element {
    rsx! { svg { width: "14", height: "14", "viewBox": "0 0 16 16", fill: "none", stroke: "currentColor", "stroke-width": "1.5", "stroke-linecap": "round", "aria-hidden": "true",
    path { d: "M4 4l8 8M12 4l-8 8" } } }
}
/// The gear: eight teeth, tip circle 8, root circle 6.1, hub 2.6 (appendix A.4), computed for a 20 by 20 box.
fn symbol_zahnrad(groesse: u32) -> Element {
    rsx! { svg { width: "{groesse}", height: "{groesse}", "viewBox": "0 0 20 20", fill: "none", stroke: "currentColor", "stroke-width": "1.4", "stroke-linejoin": "round", "aria-hidden": "true",
    path { d: "M8.84 4.01 L9.03 2.06 L10.97 2.06 L11.16 4.01 L13.41 4.94 L14.93 3.70 L16.30 5.07 L15.06 6.59 L15.99 8.84 L17.94 9.03 L17.94 10.97 L15.99 11.16 L15.06 13.41 L16.30 14.93 L14.93 16.30 L13.41 15.06 L11.16 15.99 L10.97 17.94 L9.03 17.94 L8.84 15.99 L6.59 15.06 L5.07 16.30 L3.70 14.93 L4.94 13.41 L4.01 11.16 L2.06 10.97 L2.06 9.03 L4.01 8.84 L4.94 6.59 L3.70 5.07 L5.07 3.70 L6.59 4.94 Z" }
    circle { cx: "10", cy: "10", r: "2.6" } } }
}
