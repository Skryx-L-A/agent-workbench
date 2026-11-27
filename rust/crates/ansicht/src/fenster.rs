//! Windows inside the view: "Sitzungen" (F1, B68), the settings of the user and of the admins (OF9, B72, B78) and the questions
//! before renaming and deleting (B12, B16). Each window is modal: Tab stays inside, Escape closes it (SE2 point 2, B76).

#![allow(
    unused_qualifications,
    clippy::many_single_char_names,
    clippy::large_types_passed_by_value,
    clippy::needless_pass_by_value
)]

use dioxus::prelude::*;
use serde_json::{Value, json};
use wasm_bindgen::JsCast;

use crate::aussehen::{FARBSAETZE, SCHRIFTGROESSEN, TERMINALS, THEMEN};
use crate::kern::Sitzung;
use crate::netz;
use crate::oberflaeche::{
    A, Ueber, element, falle, fehlerfeld, fokus, nach_schliessen, punkt_klasse, senden,
    symbol_kreuz, symbol_schild, token, wirken,
};
use crate::start::Wahlfeld;
use crate::ziel::{self, Einstellung};

fn schliessen(a: A) {
    let mut u = a.ueber;
    let war = u.peek().clone();
    u.set(Ueber::Nichts);
    nach_schliessen(&war);
}

// ---- window "Sitzungen" ----

/// "+" opens the window "Sitzungen" with an empty form (F1).
pub fn sitzungen_oeffnen(a: A) {
    let mut k = a.kern;
    k.write().start.leeren();
    let mut w = a.fensterwahl;
    w.set(None);
    let mut u = a.ueber;
    u.set(Ueber::Sitzungen);
}

fn wahl(
    a: A,
    id: &'static str,
    testid: &'static str,
    beschriftung: &'static str,
    feld: Wahlfeld,
    setzen: fn(&mut crate::kern::Kern, String),
) -> Element {
    let wert = feld.wert.clone();
    rsx! {
        div { class: "form-zeile",
            label { class: "form-name", r#for: id, "{beschriftung}" }
            select { class: "feld", id: id, name: id, "data-testid": testid,
                onchange: move |e: FormEvent| { let mut k = a.kern; setzen(&mut k.write(), e.value()); },
                for o in feld.eintraege {
                    option { key: "{o.wert}", value: "{o.wert}", selected: o.wert == wert, "{o.text}" }
                }
            }
        }
    }
}

fn option_text(s: &Sitzung) -> String {
    let mut t = vec![s.anzeigename().to_owned()];
    // the id after a name; a session without a name shows its id once
    if s.anzeigename() != s.id {
        t.push(s.id.clone());
    }
    t.extend(s.teile().iter().map(|x| (*x).to_owned()));
    format!("{}, {}", t.join(" "), s.zustand_text())
}

/// The window "Sitzungen" (F1, B68): the known sessions, the start form, "Beenden" and "Fortsetzen" for the chosen one.
#[allow(clippy::too_many_lines)]
pub(crate) fn sitzungen(a: A) -> Element {
    let k = a.kern.read();
    let f = k.start.clone();
    let sitzungen = k.sitzungen.clone();
    let fehler = k.fehler.clone();
    let verwaltung = k.zeigt("B11");
    drop(k);
    let gewaehlt_id = a.fensterwahl.read().clone();
    let gewaehlt = gewaehlt_id
        .as_ref()
        .and_then(|g| sitzungen.iter().find(|s| s.id == *g).cloned());
    let aktiv_opt = gewaehlt.as_ref().map(|s| format!("opt-{}", s.id));
    let info = gewaehlt.as_ref().map_or_else(
        || "Eine Sitzung wählen: Beenden und Fortsetzen gelten für sie.".to_owned(),
        |s| {
            let mut t = vec![s.anzeigename().to_owned(), s.zustand_text()];
            if let Some(o) = s.ordner() {
                t.push(o.to_owned());
            }
            t.join(" · ")
        },
    );
    let beenden_grund = match &gewaehlt {
        None => "Keine Sitzung gewählt",
        Some(s) if !s.laeuft => "Sitzung ist beendet",
        Some(_) => "",
    };
    let fortsetzen_grund = match &gewaehlt {
        None => "Keine Sitzung gewählt",
        Some(s) if s.laeuft => "Sitzung läuft",
        Some(s)
            if s.herkunft
                .as_ref()
                .is_some_and(|h| h.fortgesetzt_als.is_some()) =>
        {
            "Sitzung ist schon fortgesetzt"
        }
        Some(_) => "",
    };
    let ids: Vec<String> = sitzungen.iter().map(|s| s.id.clone()).collect();
    let ids2 = ids.clone();
    // Name with version 2 of the start structure (`Startwahl` with `fassung: 2`), whatever the route; Effort only while the chosen
    // route carries efforts (decision of 27.11.2026; SE2 point 2, SE7 point 2)
    let mit_name = f.mit_name();
    let mit_effort = f.mit_effort();
    rsx! {
        div { class: "fenster-grund" }
        div { class: "fenster sitzungen", id: "sitzungen-fenster", role: "dialog", "aria-modal": "true", "aria-label": "Sitzungen",
            "data-testid": "sitzungen-fenster", "data-bw": "B68",
            onkeydown: move |e: KeyboardEvent| falle("sitzungen-fenster", &e),
            // The focus starts in the first field of the form: `Name` with version 2, before that `Arbeitsordner` (SE2 point 2).
            onmounted: move |_| fokus(if mit_name { "wahl-name" } else { "wahl-arbeitsordner" }),
            div { class: "fenster-kopf", h2 { "Sitzungen" } }
            div { class: "fenster-leib",
                div { class: "fenster-spalte liste-spalte",
                    p { class: "fenster-hinweis", "Jede bekannte Sitzung, auch die beendeten." }
                    ul { class: "bekannte", id: "bekannte", role: "listbox", tabindex: "0", "aria-label": "Bekannte Sitzungen",
                        "aria-activedescendant": aktiv_opt,
                        onkeydown: move |e: KeyboardEvent| {
                            let runter = e.key() == Key::ArrowDown;
                            if !runter && e.key() != Key::ArrowUp { return; }
                            e.prevent_default();
                            if ids.is_empty() { return; }
                            let mut w = a.fensterwahl;
                            let i = w.peek().as_ref().and_then(|g| ids.iter().position(|x| x == g));
                            let n = match (i, runter) {
                                (None, _) => 0,
                                (Some(i), true) => (i + 1).min(ids.len() - 1),
                                (Some(i), false) => i.saturating_sub(1),
                            };
                            w.set(Some(ids[n].clone()));
                        },
                        for s in sitzungen.iter() {
                            {
                                let id = s.id.clone();
                                let gew = gewaehlt_id.as_deref() == Some(s.id.as_str());
                                rsx! {
                                    li { key: "{s.id}", id: "opt-{s.id}", role: "option", class: if gew { "option gewaehlt" } else { "option" },
                                        "aria-selected": if gew { "true" } else { "false" },
                                        onclick: move |_| { let mut w = a.fensterwahl; w.set(Some(id.clone())); fokus("bekannte"); },
                                        span { class: punkt_klasse(Some(s)) }
                                        span { class: "option-leib", "{option_text(s)}" }
                                    }
                                }
                            }
                        }
                    }
                    if ids2.is_empty() { p { class: "fenster-hinweis", "Noch keine Sitzung." } }
                }
                form { class: "fenster-spalte form-spalte", id: "start", "aria-label": "Sitzung starten",
                    onsubmit: move |e: FormEvent| {
                        e.prevent_default();
                        let Some(t) = token() else { return; };
                        let start = a.kern.read().start.struktur();
                        spawn(async move {
                            let (status, antwort) = netz::starten(&t, &start).await;
                            let mut k = a.kern;
                            let w = k.write().start_antwort(status, antwort.as_ref());
                            wirken(a, w);
                        });
                    },
                    h3 { class: "gruppe-titel", "Ordner" }
                    div { class: "karte",
                        // `Name` comes with version 2 only: before it the start structure has no field for it (OF8).
                        if mit_name {
                            div { class: "form-zeile",
                                label { class: "form-name", r#for: "wahl-name", "Name" }
                                input { class: "feld", id: "wahl-name", "data-testid": "name-eingabe", r#type: "text", value: "{f.name}",
                                    placeholder: "optional", autocomplete: "off", spellcheck: "false",
                                    oninput: move |e: FormEvent| { let mut k = a.kern; k.write().start.name = e.value(); } }
                            }
                        }
                        div { class: "form-zeile",
                            label { class: "form-name", r#for: "wahl-arbeitsordner", "Arbeitsordner" }
                            input { class: "feld", id: "wahl-arbeitsordner", "data-testid": "arbeitsordner-eingabe", r#type: "text",
                                value: "{f.arbeitsordner}", placeholder: "/pfad/zum/projekt",
                                autocapitalize: "off", autocomplete: "off", "autocorrect": "off", spellcheck: "false",
                                oninput: move |e: FormEvent| { let mut k = a.kern; k.write().start.arbeitsordner = e.value(); } }
                        }
                    }
                    h3 { class: "gruppe-titel", "Programm und Modell" }
                    div { class: "karte", "data-bw": "B69",
                        {wahl(a, "wahl-harness", "harness-wahl", "Harness", f.harness.clone(), |k, v| k.start.harness_waehlen(&v))}
                        {wahl(a, "wahl-modell", "modell-wahl", "Modell", f.modell.clone(), |k, v| k.start.modell.wert = v)}
                        if mit_effort {
                            {wahl(a, "wahl-effort", "effort-wahl", "Effort", f.effort.clone(), |k, v| k.start.effort.wert = v)}
                        }
                        {wahl(a, "wahl-route", "route-wahl", "Route", f.route.clone(), |k, v| k.start.route_waehlen(&v))}
                    }
                    h3 { class: "gruppe-titel", "Kunde" }
                    div { class: "karte",
                        {wahl(a, "wahl-kunde", "kunde-wahl", "Kunde", f.kunde.clone(), |k, v| k.start.kunde.wert = v)}
                    }
                    div { class: "form-fuss",
                        button { class: "knopf-voll", r#type: "submit", "data-testid": "sitzung-start", "Starten" }
                    }
                    {fehlerfeld(fehler.as_ref())}
                }
            }
            div { class: "fenster-fuss",
                p { class: "fenster-info", "{info}" }
                button { r#type: "button", class: "knopf-rand", "data-testid": "sitzung-beenden", "data-bw": "B15",
                    disabled: !beenden_grund.is_empty(), title: "{beenden_grund}",
                    onclick: move |_| {
                        let g = a.fensterwahl.peek().clone();
                        if let Some(g) = g { let w = a.kern.read().beenden(&g); wirken(a, w); }
                    },
                    "Beenden" }
                if verwaltung {
                    button { r#type: "button", class: "knopf-rand", "data-testid": "sitzung-fortsetzen", "data-bw": "B11",
                        disabled: !fortsetzen_grund.is_empty(), title: "{fortsetzen_grund}",
                        onclick: move |_| { let g = a.fensterwahl.peek().clone(); if let Some(g) = g { senden(&ziel::fortsetzen(&g)); } },
                        "Fortsetzen" }
                }
                button { r#type: "button", class: "knopf-rand", "data-testid": "sitzungen-schliessen", "data-bw": "B76",
                    onclick: move |_| schliessen(a), "Schließen" }
            }
        }
    }
}

// ---- questions ----

/// The question before renaming (B12, L3).
pub(crate) fn umbenennen(a: A, id: String) -> Element {
    let alt = a
        .kern
        .read()
        .sitzungen
        .iter()
        .find(|s| s.id == id)
        .map(|s| s.anzeigename().to_owned())
        .unwrap_or_default();
    rsx! {
        div { class: "fenster-grund" }
        div { class: "fenster frage", id: "frage", role: "dialog", "aria-modal": "true", "aria-label": "Namen ändern",
            onkeydown: move |e: KeyboardEvent| falle("frage", &e),
            onmounted: move |_| fokus("frage-name"),
            h2 { "Namen ändern" }
            form { onsubmit: move |e: FormEvent| {
                    e.prevent_default();
                    let n = element("frage-name").and_then(|x| x.dyn_into::<web_sys::HtmlInputElement>().ok()).map(|x| x.value()).unwrap_or_default();
                    if !n.trim().is_empty() { senden(&ziel::umbenennen(&id, n.trim())); }
                    schliessen(a);
                },
                label { class: "form-name", r#for: "frage-name", "Neuer Name" }
                input { class: "feld", id: "frage-name", r#type: "text", value: "{alt}", autocomplete: "off" }
                div { class: "form-fuss",
                    button { r#type: "button", class: "knopf-rand", onclick: move |_| schliessen(a), "Abbrechen" }
                    button { r#type: "submit", class: "knopf-voll", "Umbenennen" }
                }
            }
        }
    }
}

/// The question before deleting for good (B16, L3).
pub(crate) fn loeschen(a: A, id: String) -> Element {
    let name = a
        .kern
        .read()
        .sitzungen
        .iter()
        .find(|s| s.id == id)
        .map(|s| s.anzeigename().to_owned())
        .unwrap_or_default();
    rsx! {
        div { class: "fenster-grund" }
        div { class: "fenster frage", id: "frage", role: "alertdialog", "aria-modal": "true", "aria-label": "Endgültig löschen",
            onkeydown: move |e: KeyboardEvent| falle("frage", &e),
            onmounted: move |_| fokus("frage-nein"),
            h2 { "„{name}“ endgültig löschen?" }
            p { class: "fenster-hinweis", "Die Sitzung verschwindet aus der Liste. Ihr Gespräch wird mit entfernt. Arbeitsordner und Protokoll bleiben erhalten." }
            div { class: "form-fuss",
                button { r#type: "button", id: "frage-nein", class: "knopf-rand", onclick: move |_| schliessen(a), "Abbrechen" }
                button { r#type: "button", class: "knopf-voll gefahr", onclick: move |_| { senden(&ziel::loeschen(&id)); schliessen(a); }, "Endgültig löschen" }
            }
        }
    }
}

// ---- settings (OF9) ----

/// The pages of the user area: key, name, one-line description. A page without a key of the catalog in `Einstellungen` stays away,
/// except "Aussehen" of this device.
const SEITEN_NUTZER: [(&str, &str, &str); 4] = [
    (
        "aussehen",
        "Aussehen",
        "Farben und Schrift auf diesem Gerät.",
    ),
    ("sitzung", "Sitzung", "Vorgaben für neue Sitzungen."),
    (
        "modelle",
        "Harnesses und Modelle",
        "Deine Auswahl aus der Freigabe.",
    ),
    ("regeln", "Regeln", "Was gerade für dich gilt."),
];
/// The pages of the admin area.
const SEITEN_ADMIN: [(&str, &str, &str); 4] = [
    (
        "admin.modelle",
        "Harnesses und Modelle",
        "Freigabe und Effort je Stufe.",
    ),
    (
        "admin.erlaubnisse",
        "Erlaubnisse",
        "Was Worker ohne Rückfrage dürfen.",
    ),
    (
        "admin.aufsicht",
        "Aufsicht und Wachen",
        "Wer mitliest und eingreift.",
    ),
    (
        "admin.routen",
        "Routen und Datenklassen",
        "Wohin welche Daten gehen.",
    ),
];

/// The keys of the catalog of the pilot (`regelebenen` 15.1) with page, group, title and a description of at most one line. The keys
/// `aussehen.*` and `terminal.schriftgroesse` stay on the page "Aussehen" of this device (L8) and are not listed here.
const KATALOG: [(&str, &str, &str, &str, &str); 13] = [
    (
        "sitzung.harness",
        "sitzung",
        "Neue Sitzungen",
        "Harness",
        "Womit eine neue Sitzung startet.",
    ),
    (
        "sitzung.modell",
        "sitzung",
        "Neue Sitzungen",
        "Modell",
        "Vorgabe für das Modell.",
    ),
    (
        "sitzung.effort",
        "sitzung",
        "Neue Sitzungen",
        "Effort",
        "Wie gründlich das Modell denkt.",
    ),
    (
        "harnesses",
        "admin.modelle",
        "Freigabe",
        "Harnesses je Datenklasse",
        "Was Nutzer je Klasse wählen dürfen.",
    ),
    (
        "modelle",
        "admin.modelle",
        "Freigabe",
        "Modelle je Datenklasse",
        "Was Nutzer je Klasse wählen dürfen.",
    ),
    (
        "effort.grenzen",
        "admin.modelle",
        "Effort",
        "Grenzen des Efforts",
        "Höchster Effort je Harness und Klasse.",
    ),
    (
        "optionen.copilot_ohne_rueckfrage",
        "admin.erlaubnisse",
        "Copilot",
        "Ohne Rückfrage",
        "Werkzeuge der Firmenliste ohne Frage.",
    ),
    (
        "optionen.copilot_telemetrie",
        "admin.erlaubnisse",
        "Copilot",
        "Telemetrie",
        "Das Telemetrie-Ziel der Copilot CLI.",
    ),
    (
        "optionen.copilot_eingeschraenktes_token",
        "admin.erlaubnisse",
        "Copilot",
        "Eingeschränktes Token",
        "Statt der Geräte-Anmeldung.",
    ),
    (
        "optionen.lokale_ca",
        "admin.erlaubnisse",
        "Netz",
        "Lokale CA zulassen",
        "Cloud-Sitzungen über die lokale CA.",
    ),
    (
        "optionen.herstellerzugang",
        "admin.erlaubnisse",
        "Hersteller",
        "Herstellerzugang",
        "Zugang des Herstellers.",
    ),
    (
        "optionen.herabstufen",
        "admin.routen",
        "Datenklassen",
        "Herabstufen",
        "Die Automatik darf eine Klasse senken.",
    ),
    (
        "leseklassen",
        "admin.routen",
        "Datenklassen",
        "Leseklassen",
        "Wer welche Klasse lesen darf.",
    ),
];

fn seite_von(schluessel: &str) -> &'static str {
    KATALOG
        .iter()
        .find(|(k, ..)| *k == schluessel)
        .map_or("", |(_, s, ..)| *s)
}

fn katalog(schluessel: &str) -> (String, String, String) {
    KATALOG.iter().find(|(k, ..)| *k == schluessel).map_or_else(
        || ("Weitere".into(), schluessel.to_owned(), String::new()),
        |(_, _, g, t, b)| ((*g).to_owned(), (*t).to_owned(), (*b).to_owned()),
    )
}

/// The origin as text (`regelebenen` 15.5): "Firma", "Gruppe <name>" or "du".
fn herkunft_text(e: &Einstellung) -> String {
    match e.herkunft.as_str() {
        "firma" => "Firma".into(),
        "gruppe" => format!("Gruppe {}", e.gruppe.clone().unwrap_or_default())
            .trim()
            .to_owned(),
        "nutzer" => "du".into(),
        x => x.to_owned(),
    }
}

fn wert_text(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Bool(true) => "an".into(),
        Value::Bool(false) => "aus".into(),
        Value::Null => "–".into(),
        x => x.to_string(),
    }
}

fn seiten_symbol(seite: &str) -> Element {
    match seite {
        "aussehen" => {
            rsx! { svg { width: "18", height: "18", "viewBox": "0 0 20 20", fill: "none", stroke: "currentColor", "stroke-width": "1.4", "aria-hidden": "true",
            circle { cx: "10", cy: "10", r: "7" } path { d: "M10 3a7 7 0 0 1 0 14z", fill: "currentColor" } } }
        }
        "sitzung" => {
            rsx! { svg { width: "18", height: "18", "viewBox": "0 0 20 20", fill: "none", stroke: "currentColor", "stroke-width": "1.4", "stroke-linecap": "round", "stroke-linejoin": "round", "aria-hidden": "true",
            rect { x: "2.5", y: "4", width: "15", height: "12", rx: "2" } path { d: "M5.5 8l2.5 2-2.5 2M10 12.5h4" } } }
        }
        "modelle" | "admin.modelle" => {
            rsx! { svg { width: "18", height: "18", "viewBox": "0 0 20 20", fill: "none", stroke: "currentColor", "stroke-width": "1.4", "aria-hidden": "true",
            rect { x: "5", y: "5", width: "10", height: "10", rx: "1.5" } path { d: "M8 2.5v2.5M12 2.5v2.5M8 15v2.5M12 15v2.5M2.5 8h2.5M2.5 12h2.5M15 8h2.5M15 12h2.5" } } }
        }
        "admin.aufsicht" => {
            rsx! { svg { width: "18", height: "18", "viewBox": "0 0 20 20", fill: "none", stroke: "currentColor", "stroke-width": "1.4", "aria-hidden": "true",
            path { d: "M2 10s3-5.5 8-5.5S18 10 18 10s-3 5.5-8 5.5S2 10 2 10z" } circle { cx: "10", cy: "10", r: "2.5" } } }
        }
        "admin.routen" => {
            rsx! { svg { width: "18", height: "18", "viewBox": "0 0 20 20", fill: "none", stroke: "currentColor", "stroke-width": "1.4", "stroke-linecap": "round", "aria-hidden": "true",
            circle { cx: "5", cy: "15", r: "2" } circle { cx: "15", cy: "5", r: "2" } path { d: "M7 15h4a3 3 0 0 0 0-6H9a3 3 0 0 1 0-6h4" } } }
        }
        _ => symbol_schild(18),
    }
}

fn symbol_schloss() -> Element {
    rsx! { svg { width: "12", height: "12", "viewBox": "0 0 16 16", fill: "none", stroke: "currentColor", "stroke-width": "1.4", "aria-hidden": "true",
    rect { x: "3.5", y: "7", width: "9", height: "7", rx: "1.5" } path { d: "M5.5 7V5a2.5 2.5 0 0 1 5 0v2" } } }
}

/// The settings (OF9): the user area and, for admins only, the admin area; every row shows its effective value and origin.
#[allow(clippy::too_many_lines)]
pub(crate) fn einstellungen(a: A, seite: &'static str) -> Element {
    let k = a.kern.read();
    let werte = k.ziel_einstellungen.clone();
    // the area of the admins only when `Einstellungen` version 2 says `admin: true` (`regelebenen` 15.10, B78)
    let admin = werte.is_some() && k.admin;
    drop(k);
    let hat = |seite: &str| {
        werte
            .as_ref()
            .is_some_and(|w| w.iter().any(|e| seite_von(&e.schluessel) == seite))
    };
    // without L4 only the look of this device can be set; a page without keys stays away
    let nutzer_seiten: Vec<(&str, &str, &str)> = SEITEN_NUTZER
        .iter()
        .copied()
        .filter(|(k, ..)| *k == "aussehen" || hat(k))
        .collect();
    let admin_seiten: Vec<(&str, &str, &str)> = SEITEN_ADMIN
        .iter()
        .copied()
        .filter(|(k, ..)| admin && hat(k))
        .collect();
    let seite = if nutzer_seiten
        .iter()
        .chain(admin_seiten.iter())
        .any(|(k, ..)| *k == seite)
    {
        seite
    } else {
        "aussehen"
    };
    let (_, titel, beschreibung) = nutzer_seiten
        .iter()
        .chain(admin_seiten.iter())
        .find(|(k, ..)| *k == seite)
        .copied()
        .unwrap_or(SEITEN_NUTZER[0]);
    let ist_admin_seite = seite.starts_with("admin.");
    rsx! {
        div { class: "fenster-grund" }
        div { class: "fenster einstellungen", id: "einstellungen-fenster", role: "dialog", "aria-modal": "true", "aria-label": "Einstellungen",
            "data-testid": "einstellungen", "data-bw": "B72",
            onkeydown: move |e: KeyboardEvent| falle("einstellungen-fenster", &e),
            onmounted: move |_| fokus("seite-aktiv"),
            nav { class: "einst-seiten", "aria-label": "Seiten der Einstellungen",
                h3 { class: "einst-bereich", "Für dich" }
                for (k, n, _) in nutzer_seiten {
                    button { key: "{k}", r#type: "button", class: if k == seite { "einst-seite gewaehlt" } else { "einst-seite" },
                        id: if k == seite { "seite-aktiv" } else { "" },
                        "aria-current": if k == seite { "page" } else { "false" },
                        onclick: move |_| { let mut u = a.ueber; u.set(Ueber::Einstellungen(k)); },
                        span { class: "einst-symbol", {seiten_symbol(k)} }
                        span { "{n}" }
                    }
                }
                if admin {
                    div { class: "admin-bereich", "data-testid": "admin-einstellungen", "data-bw": "B78",
                        h3 { class: "einst-bereich", "Verwaltung" span { class: "marke admin", "Admin" } }
                        for (k, n, _) in admin_seiten {
                            button { key: "{k}", r#type: "button", class: if k == seite { "einst-seite gewaehlt" } else { "einst-seite" },
                                id: if k == seite { "seite-aktiv" } else { "" },
                                "aria-current": if k == seite { "page" } else { "false" },
                                onclick: move |_| { let mut u = a.ueber; u.set(Ueber::Einstellungen(k)); },
                                span { class: "einst-symbol", {seiten_symbol(k)} }
                                span { "{n}" }
                            }
                        }
                    }
                }
            }
            div { class: "einst-inhalt",
                div { class: "einst-kopf",
                    span { class: if ist_admin_seite { "einst-gross admin" } else { "einst-gross" }, {seiten_symbol(seite)} }
                    div {
                        h2 { "{titel}" if ist_admin_seite { span { class: "marke admin", "Admin" } } }
                        p { class: "einst-unter", "{beschreibung}" }
                    }
                    button { r#type: "button", class: "knopf-symbol einst-zu", "aria-label": "Schließen", "data-bw": "B76",
                        onclick: move |_| schliessen(a), {symbol_kreuz()} }
                }
                if seite == "aussehen" {
                    {aussehen_seite(a)}
                } else {
                    {werte_seite(seite, werte.unwrap_or_default())}
                }
            }
        }
    }
}

fn segment<const N: usize>(
    a: A,
    name: &'static str,
    wahl: [(&'static str, &'static str); N],
    wert: &str,
    setzen: fn(&mut crate::aussehen::Aussehen, &str),
) -> Element {
    let wert = wert.to_owned();
    rsx! {
        div { class: "segment", role: "radiogroup", "aria-label": "{name}",
            for (k, t) in wahl {
                button { key: "{k}", r#type: "button", role: "radio", class: if k == wert { "gewaehlt" } else { "" },
                    "aria-checked": if k == wert { "true" } else { "false" },
                    onclick: move |_| { let mut s = a.aussehen; let mut x = s.write(); setzen(&mut x, k); x.merken(); },
                    "{t}" }
            }
        }
    }
}

#[allow(clippy::too_many_lines)]
fn aussehen_seite(a: A) -> Element {
    let au = a.aussehen.read().clone();
    let schrift = au.schrift.to_string();
    rsx! {
        h3 { class: "gruppe-titel", "Farben" }
        div { class: "karte", "data-bw": "B77",
            div { class: "einst-zeile",
                span { class: "zeile-symbol", {seiten_symbol("aussehen")} }
                div { class: "zeile-text", div { class: "zeile-name", "Thema" } div { class: "zeile-beschreibung", "Hell, dunkel oder wie das System." } }
                span { class: "herkunft geraet", "dieses Gerät" }
                {segment(a, "Thema", THEMEN, &au.thema, |x, v| v.clone_into(&mut x.thema))}
            }
            div { class: "einst-zeile",
                span { class: "zeile-symbol farbmuster", span { class: "muster grau" } span { class: "muster blauschwarz" } }
                div { class: "zeile-text", div { class: "zeile-name", "Farben im dunklen Thema" } div { class: "zeile-beschreibung", "Neutral grau wie heute oder blauschwarz." } }
                span { class: "herkunft geraet", "dieses Gerät" }
                {segment(a, "Farben im dunklen Thema", FARBSAETZE, &au.farbsatz, |x, v| v.clone_into(&mut x.farbsatz))}
            }
            div { class: "einst-zeile",
                span { class: "zeile-symbol", {seiten_symbol("sitzung")} }
                div { class: "zeile-text", div { class: "zeile-name", "Terminal" } div { class: "zeile-beschreibung", "Vorgabe: dunkel wie heute." } }
                span { class: "herkunft geraet", "dieses Gerät" }
                {segment(a, "Terminal", TERMINALS, &au.terminal, |x, v| v.clone_into(&mut x.terminal))}
            }
        }
        h3 { class: "gruppe-titel", "Schrift" }
        div { class: "karte",
            div { class: "einst-zeile",
                span { class: "zeile-symbol schrift-symbol", "Aa" }
                div { class: "zeile-text", div { class: "zeile-name", "Schriftgröße im Terminal" } div { class: "zeile-beschreibung", "Gilt sofort in jedem Terminal." } }
                span { class: "herkunft geraet", "dieses Gerät" }
                select { class: "feld", "aria-label": "Schriftgröße im Terminal",
                    onchange: move |e: FormEvent| {
                        if let Ok(v) = e.value().parse::<u32>() { let mut s = a.aussehen; let mut x = s.write(); x.schrift = v; x.merken(); }
                    },
                    for g in SCHRIFTGROESSEN {
                        option { key: "{g}", value: "{g}", selected: g.to_string() == schrift, "{g} px" }
                    }
                }
            }
        }
    }
}

#[allow(clippy::too_many_lines)]
/// The rows of one page: key, title, description.
type Zeilen = Vec<(Einstellung, String, String)>;

fn werte_seite(seite: &str, werte: Vec<Einstellung>) -> Element {
    let mut gruppen: Vec<(String, Zeilen)> = Vec::new();
    let mut werte: Vec<Einstellung> = werte
        .into_iter()
        .filter(|e| seite_von(&e.schluessel) == seite)
        .collect();
    werte.sort_by_key(|e| {
        KATALOG
            .iter()
            .position(|(k, ..)| *k == e.schluessel)
            .unwrap_or(usize::MAX)
    });
    for e in werte {
        let (g, t, b) = katalog(&e.schluessel);
        match gruppen.iter_mut().find(|(x, _)| *x == g) {
            Some((_, l)) => l.push((e, t, b)),
            None => gruppen.push((g, vec![(e, t, b)])),
        }
    }
    rsx! {
        if gruppen.is_empty() { p { class: "fenster-hinweis", "Der Dienst hat für diese Seite keine Werte geschickt." } }
        for (g, zeilen) in gruppen {
            div { key: "g-{g}", class: "gruppe",
                h3 { class: "gruppe-titel", "{g}" }
                div { class: "karte",
                    for (e, titel, beschreibung) in zeilen {
                        {zeile(e, titel, beschreibung)}
                    }
                }
            }
        }
    }
}

#[allow(clippy::too_many_lines)]
fn zeile(e: Einstellung, titel: String, beschreibung: String) -> Element {
    let herkunft = herkunft_text(&e);
    let herkunft_klasse = format!("herkunft {}", e.herkunft);
    // the reason of a lock with its fixed short text (`regelebenen` 15.5, OF9 point 6)
    let gesperrt_grund = match e.grund.as_deref() {
        Some("nur_admin") => "Vorgabe der Firma".to_owned(),
        Some("nur_strenger") => "Nur strenger als die Vorgabe".to_owned(),
        Some("fest") => "Im Pilot fest aus".to_owned(),
        Some(x) => x.to_owned(),
        None if e.aenderbar => String::new(),
        None => format!("Vorgabe: {herkunft}"),
    };
    // the user sets their own level, an admin the level of the company (`regelebenen` 15.9)
    let ebene = if e.bereich == "admin" {
        "firma".to_owned()
    } else {
        "nutzer".to_owned()
    };
    let (k1, k2) = (e.schluessel.clone(), e.schluessel.clone());
    let (b1, b2) = (e.bereich.clone(), e.bereich.clone());
    let (eb1, eb2) = (ebene.clone(), ebene);
    let steuerung = match &e.wert {
        Value::Bool(an) => {
            let an = *an;
            rsx! {
                button { r#type: "button", role: "switch", class: if an { "schalter an" } else { "schalter" },
                    "aria-checked": if an { "true" } else { "false" }, "aria-label": "{titel}",
                    disabled: !e.aenderbar, title: "{gesperrt_grund}",
                    onclick: move |_| senden(&ziel::einstellung_setzen(&k1, &json!(!an), &b1, &eb1)),
                    span { class: "schalter-knopf" }
                }
            }
        }
        Value::String(w) if !e.wahl.is_empty() => {
            let w = w.clone();
            rsx! {
                select { class: "feld", "aria-label": "{titel}", disabled: !e.aenderbar, title: "{gesperrt_grund}",
                    onchange: move |x: FormEvent| senden(&ziel::einstellung_setzen(&k2, &json!(x.value()), &b2, &eb2)),
                    for o in e.wahl.clone() {
                        option { key: "{o}", value: "{o}", selected: o == w, "{o}" }
                    }
                }
            }
        }
        v => rsx! { span { class: "zeile-wert", "{wert_text(v)}" } },
    };
    rsx! {
        div { key: "{e.schluessel}", class: if e.aenderbar { "einst-zeile" } else { "einst-zeile gesperrt" },
            span { class: "zeile-symbol", {seiten_symbol(seite_von(&e.schluessel))} }
            div { class: "zeile-text",
                div { class: "zeile-name", "{titel}" }
                if !beschreibung.is_empty() { div { class: "zeile-beschreibung", "{beschreibung}" } }
                if !gesperrt_grund.is_empty() { div { class: "zeile-grund", {symbol_schloss()} span { "{gesperrt_grund}" } } }
            }
            span { class: "{herkunft_klasse}", title: "Wirksam: {wert_text(&e.wert)}, Herkunft: {herkunft}", "{herkunft}" }
            {steuerung}
        }
    }
}
