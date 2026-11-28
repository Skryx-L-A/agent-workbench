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
use crate::woerter::{self, TRENNER};
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

/// The id after a name; a session without a name shows its id once.
fn option_id(s: &Sitzung) -> Option<String> {
    (s.anzeigename() != s.id).then(|| s.id.clone())
}

/// The known sessions grouped by working folder like today (F1), in the order of their first session; without any folder (before
/// L5) one group without a heading.
fn nach_ordner(sitzungen: &[Sitzung]) -> Vec<(Option<String>, Vec<Sitzung>)> {
    let mit_ordner = sitzungen.iter().any(|s| s.ordner().is_some());
    let mut gruppen: Vec<(Option<String>, Vec<Sitzung>)> = Vec::new();
    for s in sitzungen {
        let o = mit_ordner.then(|| s.ordner().unwrap_or(woerter::OHNE_ORDNER).to_owned());
        match gruppen.iter_mut().find(|(x, _)| *x == o) {
            Some((_, l)) => l.push(s.clone()),
            None => gruppen.push((o, vec![s.clone()])),
        }
    }
    gruppen
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
        || woerter::SITZUNG_WAEHLEN.to_owned(),
        |s| {
            let mut t = vec![s.anzeigename().to_owned(), s.zustand_text()];
            if let Some(o) = s.ordner() {
                t.push(o.to_owned());
            }
            t.join(TRENNER)
        },
    );
    let beenden_grund = match &gewaehlt {
        None => woerter::GRUND_KEINE_GEWAEHLT,
        Some(s) if !s.laeuft => woerter::GRUND_IST_BEENDET,
        Some(_) => "",
    };
    let fortsetzen_grund = match &gewaehlt {
        None => woerter::GRUND_KEINE_GEWAEHLT,
        Some(s) if s.laeuft => woerter::GRUND_LAEUFT,
        Some(s) if !s.fortsetzbar() => woerter::GRUND_NICHT_FORTSETZBAR,
        Some(s)
            if s.herkunft
                .as_ref()
                .is_some_and(|h| h.fortgesetzt_als.is_some()) =>
        {
            woerter::GRUND_SCHON_FORTGESETZT
        }
        Some(_) => "",
    };
    let gruppen = nach_ordner(&sitzungen);
    // the arrow keys follow the order on screen
    let ids: Vec<String> = gruppen
        .iter()
        .flat_map(|(_, l)| l.iter().map(|s| s.id.clone()))
        .collect();
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
                    p { class: "fenster-hinweis", {woerter::SITZUNGEN_HINWEIS} }
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
                        for (ordner, liste) in gruppen {
                            li { key: "g-{ordner.clone().unwrap_or_default()}", role: "group", class: "ordner-gruppe",
                                "aria-label": ordner.clone(),
                                if let Some(o) = ordner.clone() {
                                    div { class: "ordner-kopf", "aria-hidden": "true", title: "{o}", span { dir: "ltr", "{o}" } }
                                }
                                ul { class: "ordner-sitzungen", role: "presentation",
                                    for s in liste {
                                        {
                                            let id = s.id.clone();
                                            let gew = gewaehlt_id.as_deref() == Some(s.id.as_str());
                                            let kennung = option_id(&s);
                                            let zustand = s.zustand_text();
                                            rsx! {
                                                li { key: "{s.id}", id: "opt-{s.id}", role: "option", class: if gew { "option gewaehlt" } else { "option" },
                                                    "aria-selected": if gew { "true" } else { "false" },
                                                    onclick: move |_| { let mut w = a.fensterwahl; w.set(Some(id.clone())); fokus("bekannte"); },
                                                    span { class: punkt_klasse(Some(&s)) }
                                                    span { class: "option-leib",
                                                        span { class: "option-name", "{s.anzeigename()}" }
                                                        if let Some(k) = kennung { " " span { class: "option-id", "{k}" } }
                                                        span { class: "option-zustand", "{TRENNER}{zustand}" }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    if ids2.is_empty() { p { class: "fenster-hinweis", {woerter::KEINE_SITZUNG_BEKANNT} } }
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
                                    placeholder: woerter::NAME_OPTIONAL, autocomplete: "off", spellcheck: "false",
                                    oninput: move |e: FormEvent| { let mut k = a.kern; k.write().start.name = e.value(); } }
                            }
                        }
                        div { class: "form-zeile",
                            label { class: "form-name", r#for: "wahl-arbeitsordner", "Arbeitsordner" }
                            input { class: "feld", id: "wahl-arbeitsordner", "data-testid": "arbeitsordner-eingabe", r#type: "text",
                                value: "{f.arbeitsordner}", placeholder: woerter::PFAD_PLATZHALTER,
                                autocapitalize: "off", autocomplete: "off", "autocorrect": "off", spellcheck: "false",
                                oninput: move |e: FormEvent| { let mut k = a.kern; k.write().start.arbeitsordner = e.value(); } }
                        }
                    }
                    h3 { class: "gruppe-titel", "Programm und Modell" }
                    div { class: "karte", "data-bw": "B69",
                        {wahl(a, "wahl-harness", "harness-wahl", woerter::PROGRAMM, f.harness.clone(), |k, v| k.start.harness_waehlen(&v))}
                        {wahl(a, "wahl-modell", "modell-wahl", woerter::MODELL, f.modell.clone(), |k, v| k.start.modell.wert = v)}
                        if mit_effort {
                            {wahl(a, "wahl-effort", "effort-wahl", woerter::DENKSTUFE, f.effort.clone(), |k, v| k.start.effort.wert = v)}
                        } else if f.fassung2 {
                            // the row stays with its reason, the form does not jump; no field without effect (OF8)
                            div { class: "form-zeile", "data-testid": "effort-grund",
                                span { class: "form-name", {woerter::DENKSTUFE} }
                                span { class: "form-grund", {woerter::GRUND_KEINE_DENKSTUFE} }
                            }
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
                    {woerter::BEENDEN} }
                if verwaltung {
                    button { r#type: "button", class: "knopf-rand", "data-testid": "sitzung-fortsetzen", "data-bw": "B11",
                        disabled: !fortsetzen_grund.is_empty(), title: "{fortsetzen_grund}",
                        onclick: move |_| { let g = a.fensterwahl.peek().clone(); if let Some(g) = g { senden(&ziel::fortsetzen(&g)); } },
                        {woerter::FORTSETZEN} }
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
            p { class: "fenster-hinweis", "Die Sitzung verschwindet aus der Liste. Dein Gespräch wird mit gelöscht. Arbeitsordner und Protokoll bleiben erhalten." }
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
        woerter::seite::AUSSEHEN,
        woerter::seite::AUSSEHEN_KURZ,
    ),
    (
        "sitzung",
        woerter::seite::SITZUNG,
        woerter::seite::SITZUNG_KURZ,
    ),
    (
        "modelle",
        woerter::seite::PROGRAMME,
        woerter::seite::PROGRAMME_KURZ,
    ),
    (
        "regeln",
        woerter::seite::REGELN,
        woerter::seite::REGELN_KURZ,
    ),
];
/// The pages of the admin area.
const SEITEN_ADMIN: [(&str, &str, &str); 4] = [
    (
        "admin.modelle",
        woerter::seite::PROGRAMME,
        woerter::seite::PROGRAMME_ADMIN_KURZ,
    ),
    (
        "admin.erlaubnisse",
        woerter::seite::ERLAUBNISSE,
        woerter::seite::ERLAUBNISSE_KURZ,
    ),
    (
        "admin.aufsicht",
        woerter::seite::AUFSICHT,
        woerter::seite::AUFSICHT_KURZ,
    ),
    (
        "admin.routen",
        woerter::seite::ROUTEN,
        woerter::seite::ROUTEN_KURZ,
    ),
];

/// The keys of the catalog of the pilot (`regelebenen` 15.1) with page and group; title and description come from
/// [`woerter::werte`]. The keys `aussehen.*` and `terminal.schriftgroesse` stay on the page "Aussehen" of this device (L8).
const KATALOG: [(&str, &str, woerter::Zeile); 15] = [
    (
        "sitzung",
        woerter::gruppe::NEUE_SITZUNGEN,
        woerter::werte::HARNESS,
    ),
    (
        "sitzung",
        woerter::gruppe::NEUE_SITZUNGEN,
        woerter::werte::MODELL,
    ),
    (
        "sitzung",
        woerter::gruppe::NEUE_SITZUNGEN,
        woerter::werte::EFFORT,
    ),
    (
        "admin.modelle",
        woerter::gruppe::FREIGABE,
        woerter::werte::HARNESSES,
    ),
    (
        "admin.modelle",
        woerter::gruppe::FREIGABE,
        woerter::werte::MODELLE,
    ),
    (
        "admin.modelle",
        woerter::gruppe::EFFORT,
        woerter::werte::EFFORT_GRENZEN,
    ),
    (
        "admin.erlaubnisse",
        woerter::gruppe::COPILOT,
        woerter::werte::OHNE_RUECKFRAGE,
    ),
    (
        "admin.erlaubnisse",
        woerter::gruppe::COPILOT,
        woerter::werte::TELEMETRIE,
    ),
    (
        "admin.erlaubnisse",
        woerter::gruppe::COPILOT,
        woerter::werte::TOKEN,
    ),
    (
        "admin.erlaubnisse",
        woerter::gruppe::NETZ,
        woerter::werte::LOKALE_CA,
    ),
    (
        "admin.erlaubnisse",
        woerter::gruppe::HERSTELLER,
        woerter::werte::HERSTELLERZUGANG,
    ),
    (
        "admin.routen",
        woerter::gruppe::DATENKLASSEN,
        woerter::werte::HERABSTUFEN,
    ),
    (
        "admin.routen",
        woerter::gruppe::DATENKLASSEN,
        woerter::werte::LESEKLASSEN,
    ),
    (
        "admin.aufsicht",
        woerter::gruppe::FRISTEN,
        woerter::werte::FREIGABEFRIST,
    ),
    (
        "admin.aufsicht",
        woerter::gruppe::FRISTEN,
        woerter::werte::EINLOESEFRIST,
    ),
];

fn seite_von(schluessel: &str) -> &'static str {
    KATALOG
        .iter()
        .find(|(_, _, (k, ..))| *k == schluessel)
        .map_or("", |(s, ..)| *s)
}

fn katalog(schluessel: &str) -> (String, String, String) {
    KATALOG
        .iter()
        .find(|(_, _, (k, ..))| *k == schluessel)
        .map_or_else(
            || ("Weitere".into(), schluessel.to_owned(), String::new()),
            |(_, g, (_, t, b))| ((*g).to_owned(), (*t).to_owned(), (*b).to_owned()),
        )
}

/// The origin as text (`regelebenen` 15.5): "Firma", "Gruppe <name>" or "du".
fn herkunft_text(e: &Einstellung) -> String {
    match e.herkunft.as_str() {
        "firma" => woerter::HERKUNFT_FIRMA.into(),
        "gruppe" => format!(
            "{} {}",
            woerter::HERKUNFT_GRUPPE,
            e.gruppe.clone().unwrap_or_default()
        )
        .trim()
        .to_owned(),
        "nutzer" => woerter::HERKUNFT_DU.into(),
        x => x.to_owned(),
    }
}

fn wert_text(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Bool(true) => woerter::AN.into(),
        Value::Bool(false) => woerter::AUS.into(),
        Value::Null => "–".into(),
        x => x.to_string(),
    }
}

/// The most values of a choice shown as a segment; from one more it is a drop-down (decision of 28.11.2026, both views).
const SEGMENT_HOECHSTENS: usize = 4;

/// Whether a choice of `n` values is a segment (else a drop-down).
fn als_segment(n: usize) -> bool {
    n <= SEGMENT_HOECHSTENS
}

fn wert_von<'a>(werte: &'a [Einstellung], schluessel: &str) -> Option<&'a Value> {
    werte
        .iter()
        .find(|e| e.schluessel == schluessel)
        .map(|e| &e.wert)
}

/// The efforts of a new session of the program `harness` with the model `modell` within the limits of the admins (`einstellungen`
/// version 2, block `effort`, "Erlaubte Efforts eines Starts"): for each data class in which `harnesses` releases the program, the
/// values that stand in every matching limit, in the order of the first; then the union over the classes. Without a release per
/// class only the limits without a class count; without a program, every program of the limits.
fn efforts_der_grenzen(
    grenzen: &[Value],
    harnesses: &Value,
    harness: Option<&str>,
    modell: Option<&str>,
) -> Vec<String> {
    let feld = |g: &Value, f: &str| g.get(f).and_then(Value::as_str).map(str::to_owned);
    let mut programme: Vec<String> = Vec::new();
    match harness {
        Some(h) => programme.push(h.to_owned()),
        None => {
            for h in grenzen.iter().filter_map(|g| feld(g, "harness")) {
                if !programme.contains(&h) {
                    programme.push(h);
                }
            }
        }
    }
    let mut aus: Vec<String> = Vec::new();
    for h in &programme {
        let klassen: Vec<Option<String>> = match harnesses.as_object() {
            Some(o) if !o.is_empty() => o
                .iter()
                .filter(|(_, l)| {
                    l.as_array()
                        .is_some_and(|l| l.iter().any(|x| x.as_str() == Some(h.as_str())))
                })
                .map(|(k, _)| Some(k.clone()))
                .collect(),
            _ => vec![None],
        };
        for k in &klassen {
            let passend: Vec<Vec<String>> = grenzen
                .iter()
                .filter(|g| {
                    feld(g, "harness").as_deref() == Some(h.as_str())
                        && feld(g, "klasse").is_none_or(|x| Some(&x) == k.as_ref())
                        && feld(g, "modell").is_none_or(|x| Some(x.as_str()) == modell)
                })
                .map(|g| {
                    g.get("erlaubt")
                        .and_then(Value::as_array)
                        .map(|l| {
                            l.iter()
                                .filter_map(Value::as_str)
                                .map(str::to_owned)
                                .collect()
                        })
                        .unwrap_or_default()
                })
                .collect();
            let Some((erste, rest)) = passend.split_first() else {
                continue;
            };
            for w in erste {
                if rest.iter().all(|l| l.contains(w)) && !aus.contains(w) {
                    aus.push(w.clone());
                }
            }
        }
    }
    aus
}

/// The choices of the user for model and effort when the catalog names none: the models of the routes (`Startwahl`) and the
/// efforts of the chosen program (`sitzung.harness`) within the limits of the admins (`effort.grenzen`, see
/// [`efforts_der_grenzen`]); without the limits in the message, the efforts `Startwahl` allows the program on a route. Program
/// and model are then bound to the limit of the admins (`harnesses`, `modelle`; decision of the operator, 28.11.2026): only
/// values of at least one class remain, also the effective value.
fn wahlen_ergaenzen(werte: &mut [Einstellung], start: &crate::start::Startform) {
    ergaenzen_ohne_grenze(werte, start);
    let grenzen: Vec<(String, Value)> = werte
        .iter()
        .filter(|e| matches!(e.schluessel.as_str(), "harnesses" | "modelle"))
        .map(|e| (e.schluessel.clone(), e.wert.clone()))
        .collect();
    for e in werte.iter_mut() {
        if let Some(g) = ziel::grenze_von(&e.schluessel) {
            let grenze = grenzen.iter().find(|(k, _)| k == g).map(|(_, w)| w);
            e.wahl = ziel::in_grenze(&e.wahl, grenze);
        }
    }
}

fn ergaenzen_ohne_grenze(werte: &mut [Einstellung], start: &crate::start::Startform) {
    let mut modelle: Vec<String> = start
        .modelle
        .iter()
        .filter_map(|(_, m)| m.clone())
        .collect();
    let harness = wert_von(werte, "sitzung.harness")
        .and_then(Value::as_str)
        .map(str::to_owned);
    let modell = wert_von(werte, "sitzung.modell")
        .and_then(Value::as_str)
        .map(str::to_owned);
    let efforts = if let Some(g) = wert_von(werte, "effort.grenzen").and_then(Value::as_array) {
        efforts_der_grenzen(
            g,
            wert_von(werte, "harnesses").unwrap_or(&Value::Null),
            harness.as_deref(),
            modell.as_deref(),
        )
    } else {
        let mut l: Vec<String> = Vec::new();
        for e in start
            .efforts
            .iter()
            .flat_map(|(_, l)| l.iter())
            .filter(|e| harness.as_deref().is_none_or(|h| e.harness == h))
        {
            for x in &e.erlaubt {
                if !l.contains(x) {
                    l.push(x.clone());
                }
            }
        }
        l
    };
    for e in werte.iter_mut().filter(|e| e.wahl.is_empty()) {
        let mut wahl = match e.schluessel.as_str() {
            "sitzung.modell" => std::mem::take(&mut modelle),
            "sitzung.effort" => efforts.clone(),
            _ => continue,
        };
        if wahl.is_empty() {
            continue;
        }
        if let Some(w) = e.wert.as_str()
            && !wahl.iter().any(|x| x == w)
        {
            wahl.insert(0, w.to_owned());
        }
        wahl.dedup();
        e.wahl = wahl;
    }
}

/// A deadline in milliseconds in the largest whole unit, like the app window: „30 s“, „5 min“, „24 h“, else in milliseconds.
fn frist_text(ms: u64) -> String {
    match ms {
        m if m >= 3_600_000 && m % 3_600_000 == 0 => format!("{} h", m / 3_600_000),
        m if m >= 60_000 && m % 60_000 == 0 => format!("{} min", m / 60_000),
        m if m >= 1000 && m % 1000 == 0 => format!("{} s", m / 1000),
        m => format!("{m} ms"),
    }
}

/// The choices of a deadline of a release (`regelebenen` 15.1.2) with the effective value when it is none of them.
fn frist_wahlen(feste: &[u64], wert: &Value) -> Vec<u64> {
    let mut l: Vec<u64> = feste.to_vec();
    if let Some(w) = wert.as_u64()
        && !l.contains(&w)
    {
        l.push(w);
        l.sort_unstable();
    }
    l
}

/// A list of names as text: „claude, copilot“; an empty list allows nothing.
fn namen_text(v: &Value) -> String {
    match v {
        Value::Array(l) if l.is_empty() => "keine Freigabe".into(),
        Value::Array(l) => l.iter().map(wert_text).collect::<Vec<_>>().join(", "),
        x => wert_text(x),
    }
}

/// A value of an object or a list in plain text, one line each (`regelebenen` 15.1): a list per data class („intern: claude,
/// copilot“), a limit of the effort („copilot · kundendaten: low“), never raw JSON.
fn klartext(schluessel: &str, v: &Value) -> Vec<String> {
    match v {
        Value::Object(o) if o.is_empty() => {
            vec![if schluessel == "harnesses" || schluessel == "modelle" {
                "keine Einschränkung".into()
            } else {
                "keine Angabe".into()
            }]
        }
        Value::Object(o) => o
            .iter()
            .map(|(k, w)| format!("{k}: {}", namen_text(w)))
            .collect(),
        Value::Array(l) if l.is_empty() => vec![if schluessel == "effort.grenzen" {
            "kein Effort wählbar".into()
        } else {
            "keine Angabe".into()
        }],
        Value::Array(l) => l
            .iter()
            .map(|e| match e {
                Value::Object(o) if o.contains_key("erlaubt") => {
                    let wer: Vec<String> = ["harness", "modell", "klasse"]
                        .iter()
                        .filter_map(|f| o.get(*f).map(wert_text))
                        .collect();
                    let wer = if wer.is_empty() {
                        "alle".to_owned()
                    } else {
                        wer.join(crate::woerter::TRENNER)
                    };
                    let erlaubt: Vec<String> = o["erlaubt"]
                        .as_array()
                        .map(|l| {
                            l.iter()
                                .map(|x| crate::start::effort_text(&wert_text(x)).to_owned())
                                .collect()
                        })
                        .unwrap_or_default();
                    if erlaubt.is_empty() {
                        format!("{wer}: {}", namen_text(&o["erlaubt"]))
                    } else {
                        format!("{wer}: {}", erlaubt.join(", "))
                    }
                }
                Value::Object(o) => o
                    .iter()
                    .map(|(k, w)| format!("{k}: {}", namen_text(w)))
                    .collect::<Vec<_>>()
                    .join(crate::woerter::TRENNER),
                x => wert_text(x),
            })
            .collect(),
        x => vec![wert_text(x)],
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
    let mut werte = k.ziel_einstellungen.clone();
    // model and effort of the user are choices (E2): the models of the routes and the efforts the admins allow, from `Startwahl`
    if let Some(w) = werte.as_mut() {
        wahlen_ergaenzen(w, &k.start);
    }
    // the area of the admins only when `Einstellungen` version 2 says `admin: true` (`regelebenen` 15.10, B78)
    let admin = werte.is_some() && k.admin;
    // the protection layers in effect: information on the page "Aufsicht und Meldungen", only with the message (OF8 point 3)
    let schutz = k.schutz.clone();
    let fristen = k.fristen.clone();
    let auskunft = schutz.is_some() || fristen.is_some();
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
        .filter(|(k, ..)| admin && (hat(k) || (*k == "admin.aufsicht" && auskunft)))
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
    let schutz_hier = schutz.filter(|_| seite == "admin.aufsicht");
    let fristen_hier = fristen.filter(|_| seite == "admin.aufsicht");
    let auskunft_hier = schutz_hier.is_some() || fristen_hier.is_some();
    rsx! {
        div { class: "fenster-grund" }
        div { class: "fenster einstellungen", id: "einstellungen-fenster", role: "dialog", "aria-modal": "true", "aria-label": "Einstellungen",
            "data-testid": "einstellungen", "data-bw": "B72",
            onkeydown: move |e: KeyboardEvent| falle("einstellungen-fenster", &e),
            onmounted: move |_| fokus("seite-aktiv"),
            nav { class: "einst-seiten", "aria-label": "Seiten der Einstellungen",
                h3 { class: "einst-bereich", {woerter::BEREICH_NUTZER} }
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
                        h3 { class: "einst-bereich", {woerter::VERWALTUNG} span { class: "marke admin", {woerter::ADMIN} } }
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
                        h2 { "{titel}" if ist_admin_seite { span { class: "marke admin", {woerter::ADMIN} } } }
                        p { class: "einst-unter", "{beschreibung}" }
                    }
                    button { r#type: "button", class: "knopf-symbol einst-zu", "aria-label": woerter::SCHLIESSEN, "data-bw": "B76",
                        onclick: move |_| schliessen(a), {symbol_kreuz()} }
                }
                if seite == "aussehen" {
                    {aussehen_seite(a)}
                } else {
                    {werte_seite(seite, werte.unwrap_or_default(), auskunft_hier)}
                    if let Some(f) = fristen_hier {
                        {fristen_teil(f)}
                    }
                    if let Some(x) = schutz_hier {
                        {schutz_teil(x)}
                    }
                }
            }
        }
    }
}

/// A deadline in the table in seconds: „60 s“; a deadline below a whole second in milliseconds.
fn sekunden_text(ms: u64) -> String {
    if ms.is_multiple_of(1000) {
        format!("{} {}", ms / 1000, woerter::EINHEIT_SEKUNDEN)
    } else {
        format!("{ms} ms")
    }
}

/// One cell of the table of deadlines: the deadline, its origin and the class of the origin chip.
type Fristzelle = Option<(String, String, &'static str)>;

/// The deadlines per MCP server (message `Fristen`): a table to read, one row per server, each deadline with its origin.
fn fristen_teil(f: Vec<ziel::Frist>) -> Element {
    let mut server: Vec<String> = Vec::new();
    for x in &f {
        if !server.contains(&x.server) {
            server.push(x.server.clone());
        }
    }
    let zelle = |s: &str, k: &str| {
        f.iter()
            .find(|x| x.server == s && x.schluessel == k)
            .map(|x| {
                let h = if x.herkunft == "firmendatei" {
                    woerter::HERKUNFT_FIRMENDATEI
                } else if x.herkunft == "katalog" {
                    woerter::HERKUNFT_KATALOG
                } else {
                    x.herkunft.as_str()
                };
                (
                    sekunden_text(x.wert),
                    h.to_owned(),
                    if x.herkunft == "firmendatei" {
                        "herkunft firma"
                    } else {
                        "herkunft geraet"
                    },
                )
            })
    };
    let zeilen: Vec<(String, Vec<Fristzelle>)> = server
        .iter()
        .map(|s| {
            (
                s.clone(),
                vec![
                    zelle(s, woerter::werte::FREIGABEFRIST.0),
                    zelle(s, woerter::werte::EINLOESEFRIST.0),
                ],
            )
        })
        .collect();
    rsx! {
        div { class: "gruppe", "data-testid": "fristen-je-server",
            h3 { class: "gruppe-titel", {woerter::FRISTEN_JE_SERVER} }
            div { class: "karte",
                table { class: "fristen",
                    thead { tr {
                        th { scope: "col", {woerter::SERVER} }
                        th { scope: "col", {woerter::werte::FREIGABEFRIST.1} }
                        th { scope: "col", {woerter::werte::EINLOESEFRIST.1} }
                    } }
                    tbody {
                        for (s, zellen) in zeilen {
                            tr { key: "{s}",
                                th { scope: "row", "{s}" }
                                for (i, z) in zellen.into_iter().enumerate() {
                                    td { key: "{i}",
                                        if let Some((w, h, k)) = z {
                                            span { class: "fristen-wert", "{w}" }
                                            span { class: "{k}", "{h}" }
                                        } else {
                                            "–"
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// A date of the service (`JJJJ-MM-TT`) as a German date; another form stays as it is.
fn datum_text(d: &str) -> String {
    match d.split('-').collect::<Vec<_>>()[..] {
        [j, m, t] if j.len() == 4 && m.len() == 2 && t.len() == 2 => format!("{t}.{m}.{j}"),
        _ => d.to_owned(),
    }
}

/// A time in ms since 1970 as local date and time: day, month, year, then hours and minutes with two digits.
fn zeit_text(ms: f64) -> String {
    let d = js_sys::Date::new(&wasm_bindgen::JsValue::from_f64(ms));
    format!(
        "{:02}.{:02}.{}, {:02}:{:02}",
        d.get_date(),
        d.get_month() + 1,
        d.get_full_year(),
        d.get_hours(),
        d.get_minutes()
    )
}

/// The reason of `lauf` in plain text.
fn grund_text(g: &str) -> String {
    woerter::LAUF_GRUENDE
        .iter()
        .find(|(k, _)| *k == g)
        .map_or_else(|| g.to_owned(), |(_, t)| (*t).to_owned())
}

/// The protection layers in effect: only what was measured, each with its date, no control and no sign that means „on“ (decision
/// of 27.11.2026); the network destinations as a number per origin, never a host.
fn schutz_teil(x: ziel::Schutzstand) -> Element {
    let einschluss = x.probelauf.as_deref().map_or_else(
        || woerter::SCHUTZ_NICHT_BEKANNT.to_owned(),
        |d| format!("{} {}", woerter::SCHUTZ_PROBELAUF, datum_text(d)),
    );
    let halter = x.eingerichtet.as_deref().map_or_else(
        || woerter::SCHUTZ_NICHT_BEKANNT.to_owned(),
        |d| format!("{} {}", woerter::SCHUTZ_EINGERICHTET, datum_text(d)),
    );
    let start = x.letzter_start.as_ref().map_or_else(
        || woerter::SCHUTZ_KEIN_START.to_owned(),
        |(ms, e)| {
            let ergebnis = if e == "gestartet" {
                woerter::SCHUTZ_GESTARTET.to_owned()
            } else {
                grund_text(e)
            };
            format!("{}, {ergebnis}", zeit_text(*ms))
        },
    );
    let netz: Vec<(String, String, u64)> = x
        .netzziele
        .iter()
        .map(|(h, n)| {
            let (klasse, text) = match h.split_once(' ') {
                Some(("gruppe", g)) => ("gruppe", format!("{} {g}", woerter::HERKUNFT_GRUPPE)),
                _ => match h.as_str() {
                    "firma" => ("firma", woerter::HERKUNFT_FIRMA.to_owned()),
                    "nutzer" => ("nutzer", woerter::HERKUNFT_DU.to_owned()),
                    y => ("", y.to_owned()),
                },
            };
            (format!("herkunft {klasse}"), text, *n)
        })
        .collect();
    let zeilen = [
        (woerter::SCHUTZ_EINSCHLUSS, einschluss),
        (woerter::SCHUTZ_HALTER, halter),
        (woerter::SCHUTZ_LETZTER_START, start),
    ];
    rsx! {
        div { class: "gruppe", "data-testid": "schutzstand",
            h3 { class: "gruppe-titel", {woerter::SCHUTZ} }
            p { class: "fenster-hinweis schutz-kurz", {woerter::SCHUTZ_KURZ} }
            div { class: "karte",
                for (name, wert) in zeilen {
                    div { key: "{name}", class: "einst-zeile",
                        div { class: "zeile-text", div { class: "zeile-name", {name} } }
                        span { class: "zeile-wert schutz-wert", "{wert}" }
                    }
                }
                if netz.is_empty() {
                    div { class: "einst-zeile",
                        div { class: "zeile-text", div { class: "zeile-name", {woerter::SCHUTZ_NETZZIELE} } }
                        span { class: "zeile-wert", "0" }
                    }
                }
                for (i, (klasse, herkunft, n)) in netz.into_iter().enumerate() {
                    div { key: "n{i}", class: "einst-zeile",
                        div { class: "zeile-text", div { class: "zeile-name", {woerter::SCHUTZ_NETZZIELE} } }
                        span { class: "{klasse}", "{herkunft}" }
                        span { class: "zeile-wert", "{n}" }
                    }
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
        h3 { class: "gruppe-titel", {woerter::gruppe::FARBEN} }
        div { class: "karte", "data-bw": "B77",
            div { class: "einst-zeile",
                span { class: "zeile-symbol", {seiten_symbol("aussehen")} }
                div { class: "zeile-text", div { class: "zeile-name", {woerter::aussehen::THEMA.1} } div { class: "zeile-beschreibung", {woerter::aussehen::THEMA.2} } }
                span { class: "herkunft nutzer", {woerter::HERKUNFT_DU} }
                {segment(a, woerter::THEMA, THEMEN, &au.thema, |x, v| v.clone_into(&mut x.thema))}
            }
            div { class: "einst-zeile",
                span { class: "zeile-symbol farbmuster", span { class: "muster grau" } span { class: "muster blauschwarz" } }
                div { class: "zeile-text", div { class: "zeile-name", {woerter::aussehen::FARBSATZ.1} } div { class: "zeile-beschreibung", {woerter::aussehen::FARBSATZ.2} } }
                span { class: "herkunft nutzer", {woerter::HERKUNFT_DU} }
                {segment(a, woerter::FARBSATZ, FARBSAETZE, &au.farbsatz, |x, v| v.clone_into(&mut x.farbsatz))}
            }
            div { class: "einst-zeile",
                span { class: "zeile-symbol", {seiten_symbol("sitzung")} }
                div { class: "zeile-text", div { class: "zeile-name", {woerter::aussehen::TERMINAL.1} } div { class: "zeile-beschreibung", {woerter::aussehen::TERMINAL.2} } }
                span { class: "herkunft nutzer", {woerter::HERKUNFT_DU} }
                {segment(a, woerter::TERMINAL, TERMINALS, &au.terminal, |x, v| v.clone_into(&mut x.terminal))}
            }
        }
        h3 { class: "gruppe-titel", {woerter::gruppe::SCHRIFT} }
        div { class: "karte",
            div { class: "einst-zeile",
                span { class: "zeile-symbol schrift-symbol", "Aa" }
                div { class: "zeile-text", div { class: "zeile-name", {woerter::aussehen::SCHRIFTGROESSE.1} } div { class: "zeile-beschreibung", {woerter::aussehen::SCHRIFTGROESSE.2} } }
                span { class: "herkunft nutzer", {woerter::HERKUNFT_DU} }
                select { class: "feld", "aria-label": woerter::SCHRIFTGROESSE,
                    onchange: move |e: FormEvent| {
                        if let Ok(v) = e.value().parse::<u32>() { let mut s = a.aussehen; let mut x = s.write(); x.schrift = v; x.merken(); }
                    },
                    for g in SCHRIFTGROESSEN {
                        option { key: "{g}", value: "{g}", selected: g.to_string() == schrift, {format!("{g} {}", woerter::EINHEIT_PUNKT)} }
                    }
                }
            }
        }
    }
}

#[allow(clippy::too_many_lines)]
/// The rows of one page: key, title, description.
type Zeilen = Vec<(Einstellung, String, String)>;

/// The rows of one page; `auskunft`: the page carries a section to read, so no rows is no gap.
fn werte_seite(seite: &str, werte: Vec<Einstellung>, auskunft: bool) -> Element {
    let mut gruppen: Vec<(String, Zeilen)> = Vec::new();
    let mut werte: Vec<Einstellung> = werte
        .into_iter()
        .filter(|e| seite_von(&e.schluessel) == seite)
        .collect();
    werte.sort_by_key(|e| {
        KATALOG
            .iter()
            .position(|(_, _, (k, ..))| *k == e.schluessel)
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
        if gruppen.is_empty() && !auskunft { p { class: "fenster-hinweis", {woerter::KEINE_WERTE} } }
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
    let wirksam = klartext(&e.schluessel, &e.wert).join("; ");
    // the reason of a lock with its fixed short text (`regelebenen` 15.5, OF9 point 6)
    let gesperrt_grund = match e.grund.as_deref() {
        Some("nur_admin") => woerter::GRUND_NUR_ADMIN.to_owned(),
        Some("nur_strenger") => woerter::GRUND_NUR_STRENGER.to_owned(),
        Some("fest") => woerter::GRUND_FEST.to_owned(),
        Some(x) => x.to_owned(),
        None if e.aenderbar => String::new(),
        None => format!("{} {herkunft}", woerter::GRUND_VORGABE),
    };
    // the user sets their own level, an admin the level of the company (`regelebenen` 15.9)
    let ebene = if e.bereich == "admin" {
        "firma".to_owned()
    } else {
        "nutzer".to_owned()
    };
    let (k1, b1, eb1) = (e.schluessel.clone(), e.bereich.clone(), ebene.clone());
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
        // a deadline of a release as a choice in s, min or h (`regelebenen` 15.1.2)
        Value::Number(n)
            if matches!(
                e.schluessel.as_str(),
                "aufsicht.freigabefrist_ms" | "aufsicht.einloesefrist_ms"
            ) =>
        {
            let w = n.as_u64().unwrap_or_default();
            let feste: &[u64] = if e.schluessel == "aufsicht.freigabefrist_ms" {
                &woerter::werte::FREIGABEFRIST_WAHLEN
            } else {
                &woerter::werte::EINLOESEFRIST_WAHLEN
            };
            let wahl = frist_wahlen(feste, &e.wert);
            let (k4, b4, eb4) = (e.schluessel.clone(), e.bereich.clone(), ebene.clone());
            rsx! {
                select { class: "feld", "aria-label": "{titel}", disabled: !e.aenderbar, title: "{gesperrt_grund}",
                    onchange: move |x: FormEvent| {
                        if let Ok(v) = x.value().parse::<u64>() { senden(&ziel::einstellung_setzen(&k4, &json!(v), &b4, &eb4)); }
                    },
                    for o in wahl {
                        option { key: "{o}", value: "{o}", selected: o == w, {frist_text(o)} }
                    }
                }
            }
        }
        // one rule for the form of a choice in both views (decision of 28.11.2026): up to four values a segment, from five a
        // drop-down; the effort has the default of the program first (E2) and reads `off` and `none` as a word
        Value::String(_) | Value::Null if !e.wahl.is_empty() => {
            let effort = e.schluessel == "sitzung.effort";
            let w = e.wert.as_str().unwrap_or_default().to_owned();
            let mut wahl: Vec<(String, String)> = Vec::new();
            if effort {
                wahl.push((String::new(), woerter::VORGABE_DES_PROGRAMMS.to_owned()));
            }
            wahl.extend(e.wahl.iter().map(|x| {
                let t = if effort {
                    crate::start::effort_text(x).to_owned()
                } else {
                    x.clone()
                };
                (x.clone(), t)
            }));
            // an effective value outside the choice (for instance outside the limit of the admins) shows as it is, but is no
            // choice: no segment is marked, the drop-down shows it disabled; without it the browser would show the first choice
            let ausserhalb = !wahl.iter().any(|(x, _)| *x == w);
            let (k3, b3, eb3) = (e.schluessel.clone(), e.bereich.clone(), ebene.clone());
            if als_segment(wahl.len()) {
                rsx! {
                    div { class: "segment", role: "radiogroup", "aria-label": "{titel}", title: "{gesperrt_grund}",
                        for (wert, text) in wahl {
                            {
                                let an = wert == w;
                                let (k, b, eb) = (k3.clone(), b3.clone(), eb3.clone());
                                let neu = if wert.is_empty() { Value::Null } else { json!(wert) };
                                rsx! {
                                    button { key: "{wert}", r#type: "button", role: "radio", class: if an { "gewaehlt" } else { "" },
                                        "aria-checked": if an { "true" } else { "false" }, disabled: !e.aenderbar,
                                        onclick: move |_| senden(&ziel::einstellung_setzen(&k, &neu, &b, &eb)),
                                        "{text}" }
                                }
                            }
                        }
                    }
                }
            } else {
                rsx! {
                    select { class: "feld", "aria-label": "{titel}", disabled: !e.aenderbar, title: "{gesperrt_grund}",
                        onchange: move |x: FormEvent| {
                            let v = x.value();
                            let neu = if v.is_empty() { Value::Null } else { json!(v) };
                            senden(&ziel::einstellung_setzen(&k3, &neu, &b3, &eb3));
                        },
                        if ausserhalb {
                            option { key: "{w}", value: "{w}", selected: true, disabled: true, "{w}" }
                        }
                        for (wert, text) in wahl {
                            option { key: "{wert}", value: "{wert}", selected: wert == w, "{text}" }
                        }
                    }
                }
            }
        }
        v @ (Value::Object(_) | Value::Array(_)) => {
            let zeilen = klartext(&e.schluessel, v);
            rsx! {
                div { class: "zeile-werte",
                    for (i, z) in zeilen.into_iter().enumerate() {
                        div { key: "{i}", class: "zeile-wert", "{z}" }
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
            span { class: "{herkunft_klasse}", title: "Wirksam: {wirksam}, Herkunft: {herkunft}", "{herkunft}" }
            {steuerung}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{datum_text, grund_text, klartext, wahlen_ergaenzen};
    use crate::start::Startform;
    use crate::ziel::{Einstellung, einstellungen_lesen};

    /// The settings of version 2 with `sitzung.harness` and `sitzung.modell` and the given limits of the admins.
    fn mit_grenze(
        harnesses: Option<serde_json::Value>,
        modelle: Option<serde_json::Value>,
    ) -> Vec<Einstellung> {
        let w =
            |wert: serde_json::Value| json!({"wert": wert, "herkunft": "firma", "aenderbar": true});
        let mut werte =
            json!({"sitzung.harness": w(json!("claude")), "sitzung.modell": w(json!("opus"))});
        if let Some(h) = harnesses {
            werte["harnesses"] = w(h);
        }
        if let Some(m) = modelle {
            werte["modelle"] = w(m);
        }
        einstellungen_lesen(&json!({"fassung": 2, "werte": werte}))
            .expect("version 2")
            .1
    }

    fn wahl(werte: &[Einstellung], schluessel: &str) -> Vec<String> {
        werte
            .iter()
            .find(|e| e.schluessel == schluessel)
            .map(|e| e.wahl.clone())
            .unwrap_or_default()
    }

    fn harness_wahl(harnesses: Option<serde_json::Value>) -> Vec<String> {
        let mut w = mit_grenze(harnesses, None);
        wahlen_ergaenzen(&mut w, &Startform::default());
        wahl(&w, "sitzung.harness")
    }

    #[test]
    fn the_choice_of_a_session_offers_only_programs_the_limit_allows() {
        // decision of the operator, 28.11.2026: without `claude` in `harnesses` the choice has no `claude`
        assert_eq!(
            harness_wahl(Some(json!({"intern": ["copilot", "pi", "opencode"]}))),
            ["copilot", "pi", "opencode"]
        );
        assert_eq!(
            harness_wahl(Some(
                json!({"intern": ["opencode", "pi", "copilot", "claude"]})
            )),
            ["claude", "copilot", "pi", "opencode"],
            "the order of the catalog"
        );
        assert_eq!(
            harness_wahl(Some(json!({"intern": ["copilot"], "kundendaten": ["pi"]}))),
            ["copilot", "pi"],
            "a program of at least one class"
        );
        assert_eq!(
            harness_wahl(Some(json!({"intern": []}))),
            Vec::<String>::new(),
            "no release"
        );
        // `{}` and a missing key are the default of the contract: no restriction (`regelebenen` 15.1)
        for h in [Some(json!({})), None] {
            assert_eq!(harness_wahl(h), ["claude", "copilot", "pi", "opencode"]);
        }
    }

    #[test]
    fn the_choice_of_a_model_offers_only_models_the_limit_allows() {
        let start = Startform {
            modelle: vec![
                ("cloud".into(), Some("sonnet".into())),
                ("lokal".into(), Some("qwen3:8b".into())),
                ("auto".into(), None),
            ],
            ..Startform::default()
        };
        let modell_wahl = |modelle: Option<serde_json::Value>| {
            let mut w = mit_grenze(None, modelle);
            wahlen_ergaenzen(&mut w, &start);
            wahl(&w, "sitzung.modell")
        };
        // the effective value `opus` lies outside the limit and is no choice
        assert_eq!(modell_wahl(Some(json!({"intern": ["sonnet"]}))), ["sonnet"]);
        assert_eq!(
            modell_wahl(Some(json!({"intern": []}))),
            Vec::<String>::new()
        );
        assert_eq!(modell_wahl(None), ["opus", "sonnet", "qwen3:8b"]);
        assert_eq!(
            modell_wahl(Some(json!({"intern": ["opus", "qwen3:8b"]}))),
            ["opus", "qwen3:8b"]
        );
    }

    #[test]
    fn dates_and_reasons_of_the_protection_layers_read_plainly() {
        assert_eq!(datum_text("2026-11-27"), "27.11.2026");
        assert_eq!(datum_text("gestern"), "gestern");
        assert_eq!(
            grund_text("lauf.einschluss_fehlt"),
            "Einschluss nicht verfügbar"
        );
        assert_eq!(grund_text("lauf.neu"), "lauf.neu");
    }
    use serde_json::json;

    #[test]
    fn admin_values_read_as_plain_text_per_data_class() {
        assert_eq!(
            klartext(
                "harnesses",
                &json!({"intern": ["claude", "copilot"], "kundendaten": ["pi"]})
            ),
            ["intern: claude, copilot", "kundendaten: pi"]
        );
        assert_eq!(klartext("modelle", &json!({})), ["keine Einschränkung"]);
        assert_eq!(
            klartext("modelle", &json!({"intern": []})),
            ["intern: keine Freigabe"]
        );
        assert_eq!(
            klartext("effort.grenzen", &json!([])),
            ["kein Effort wählbar"]
        );
        assert_eq!(
            klartext(
                "effort.grenzen",
                &json!([{"harness": "copilot", "klasse": "kundendaten", "erlaubt": ["low"]},
                        {"harness": "pi", "modell": "qwen3:8b", "erlaubt": ["off", "low"]}])
            ),
            ["copilot · kundendaten: low", "pi · qwen3:8b: aus, low"]
        );
        for z in klartext("leseklassen", &json!([{"a": 1}, "b"])) {
            assert!(!z.contains('{') && !z.contains('['), "{z}");
        }
    }

    /// Decision of 28.11.2026: up to four values a segment, from five a drop-down; the font size (six values) stays a drop-down.
    #[test]
    fn up_to_four_values_a_segment_from_five_a_drop_down() {
        use super::als_segment;
        assert!(als_segment(2) && als_segment(3) && als_segment(4));
        assert!(!als_segment(5) && !als_segment(crate::aussehen::SCHRIFTGROESSEN.len()));
    }

    /// Decision of 28.11.2026: the thinking level of a session takes its values from the limit of the chosen program and of the
    /// class in which it is released, like the window of the app.
    #[test]
    fn the_thinking_level_follows_the_limit_of_program_and_class() {
        use super::efforts_der_grenzen;
        let grenzen = json!([
            {"harness": "copilot", "erlaubt": ["low", "medium", "high"]},
            {"harness": "copilot", "klasse": "intern", "modell": "gpt-6-luna", "erlaubt": ["low", "medium"]},
            {"harness": "pi", "klasse": "kundendaten", "erlaubt": ["off", "low"]}]);
        let g = grenzen.as_array().expect("Liste");
        let frei = json!({"intern": ["copilot", "pi", "opencode"], "kundendaten": ["pi"]});
        let e = |h: Option<&str>, m: Option<&str>| efforts_der_grenzen(g, &frei, h, m);
        assert_eq!(
            e(Some("copilot"), Some("sonnet")),
            ["low", "medium", "high"]
        );
        // a second matching limit can only narrow
        assert_eq!(e(Some("copilot"), Some("gpt-6-luna")), ["low", "medium"]);
        // pi is released for intern (no limit) and kundendaten (off, low)
        assert_eq!(e(Some("pi"), None), ["off", "low"]);
        assert_eq!(e(Some("opencode"), None), Vec::<String>::new());
        // without a release per class only limits without a class count
        assert_eq!(
            efforts_der_grenzen(g, &json!({}), Some("pi"), None),
            Vec::<String>::new()
        );
        assert_eq!(e(None, None), ["low", "medium", "high", "off"]);
    }
}
