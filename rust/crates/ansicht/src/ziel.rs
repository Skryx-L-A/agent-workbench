//! The target protocol of the view: every name and field the view reads from the core and the service, or sends to them, for the
//! gaps L1 to L11 (`rust/spec/VERTRAEGE.md` section 4), at exactly one place: one read function per message, one builder per event.
//!
//! The names and fields are those of the contracts:
//! - `zustandskern` and `dienst`, section "Nachtrag 26.11.2026: Oberfläche L1, L2, L3, L5, L6, L11" (R-L1, R-L2, R-L6, DI-L1 to DI-L6);
//! - `dienst`, section "Nachtrag 26.11.2026: Ordner lesen und durchsuchen (L9)";
//! - `dienst`, section "Nachtrag 27.11.2026: Ereignisse des Dienstes beim Verbinden" (DI-E5, the message `Ereignisliste`);
//! - `regelebenen` section 15 (L4) and `start` "Fassung 2" with the message `Startwahl` with efforts (L10).
//!
//! A message in the form before these addenda gives `None` here. By which field the view knows that a gap is closed, and which
//! gaps have no such field, says [`crate::kern::Kern::luecken`].

use serde_json::{Map, Value, json};

fn text(v: &Value, name: &str) -> Option<String> {
    v.get(name)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
}

/// A field that must be a text, possibly empty (`ordner` of the session from `kind` is `""`, DI-L5 point 1).
fn text_auch_leer(v: &Value, name: &str) -> Option<String> {
    v.get(name).and_then(Value::as_str).map(str::to_owned)
}

fn liste<'a>(v: &'a Value, name: &str) -> &'a [Value] {
    v.get(name)
        .and_then(Value::as_array)
        .map_or(&[], Vec::as_slice)
}

fn nummer(v: &Value, name: &str) -> Option<u8> {
    v.get(name)
        .and_then(Value::as_u64)
        .and_then(|n| u8::try_from(n).ok())
}

fn wahr(v: &Value, name: &str) -> bool {
    v.get(name).and_then(Value::as_bool) == Some(true)
}

// ---- DI-E5: the events this connection carries (`dienst`, addendum of 27.11.2026) ----

/// The events of L3 (DI-L3): rename, continue, revive, delete.
pub const L3_EREIGNISSE: [&str; 4] = ["Umbenennen", "Fortsetzen", "Wiederbeleben", "Loeschen"];
/// The events of L9 (addendum L9): read the working folder and search its content.
pub const L9_EREIGNISSE: [&str; 2] = ["OrdnerLesen", "InhaltSuchen"];
/// The event of deciding a request (DI-L6); only the local path carries it (DI-E5 point 5).
pub const FREIGABE: &str = "Freigabe";
/// The most entries a list may have (DI-E5 point 3).
const EREIGNISSE_MAX: usize = 64;

/// The message `Ereignisliste {fassung: 1, ereignisse}` (DI-E5 point 1): the kinds of event the switch of this connection takes.
/// `Some` only in version 1 with a list of at most 64 texts; any other form is `None`, and the view then counts no event as carried
/// (DI-E5 point 9). Unknown names stay in the list; the view asks only for the names it knows.
#[must_use]
pub fn ereignisliste_lesen(n: &Value) -> Option<Vec<String>> {
    if n.get("fassung").and_then(Value::as_u64) != Some(1) {
        return None;
    }
    let a = n.get("ereignisse")?.as_array()?;
    if a.len() > EREIGNISSE_MAX {
        return None;
    }
    a.iter().map(|x| x.as_str().map(str::to_owned)).collect()
}

// ---- L1: workers with their session (`Agenten`, `zustandskern` R-L1 and `dienst` DI-L1) ----

/// One worker of `Agenten`. A worker is a session of its own (DI-L1 point 1): `worker` is the id of that session, `sitzung` the id of
/// its orchestrator.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Agent {
    /// The id of the session of the worker.
    pub worker: String,
    /// The id of the session of its orchestrator.
    pub sitzung: String,
    /// Its role (DI-L1 point 1, default `worker`; shown in the accent colour of the pane head).
    pub rolle: String,
    /// Its model (the text of the start structure); empty in the list of the core (R-L1 point 3).
    pub modell: String,
    /// `laeuft` or `beendet` (DI-L1 point 3); the service knows no other state.
    pub zustand: String,
    /// The number of its raster as in `Sitzungsliste` (DI-L1 point 3); `None` in the list of the core.
    pub nr: Option<u8>,
    /// The exit code once ended, as in `Sitzungsliste`.
    pub code: Option<String>,
}

impl Agent {
    /// Whether the worker runs (the pill, the foot and the session row count these).
    #[must_use]
    pub fn laeuft(&self) -> bool {
        self.zustand == "laeuft"
    }
}

/// One entry of `Agenten` or the message `Agentenzustand`; `None` without the field `sitzung` (the form before R-L1).
#[must_use]
pub fn agent_lesen(v: &Value) -> Option<Agent> {
    let sitzung = text(v, "sitzung")?;
    Some(Agent {
        worker: text(v, "worker").unwrap_or_default(),
        rolle: text(v, "rolle").unwrap_or_else(|| "worker".to_owned()),
        modell: text(v, "modell").unwrap_or_default(),
        zustand: text(v, "zustand").unwrap_or_default(),
        nr: nummer(v, "nr"),
        code: v.get("code").filter(|c| !c.is_null()).map(|c| match c {
            Value::String(s) => s.clone(),
            x => x.to_string(),
        }),
        sitzung,
    })
}

/// The message `Agenten`: `Some` only when the list is not empty and every entry names its session (R-L1 point 1). An empty list is
/// the same before and after R-L1 (DI-L1 point 4) and says nothing.
#[must_use]
pub fn agenten_lesen(n: &Value) -> Option<Vec<Agent>> {
    let a = liste(n, "agenten");
    let gelesen: Vec<Agent> = a.iter().filter_map(agent_lesen).collect();
    (!gelesen.is_empty() && gelesen.len() == a.len()).then_some(gelesen)
}

/// Whether every entry of `Agenten` carries the key `nr`: only the list of the service does (DI-L1 point 3), the one of the core
/// does not (R-L1 point 1). The view then knows the raster of each worker (L2).
#[must_use]
pub fn agenten_mit_raster(n: &Value) -> bool {
    let a = liste(n, "agenten");
    !a.is_empty()
        && a.iter()
            .all(|x| x.as_object().is_some_and(|o| o.contains_key("nr")))
}

// ---- L2: one subscription per session (`zustandskern` R-L2, `dienst` DI-L2) ----

/// `Abo` on or off for a session; a worker pane subscribes the session of its worker. There is no field `nr` (DI-L2 point 1).
#[must_use]
pub fn abo(sitzung: &str, an: bool, groesse: (usize, usize)) -> Value {
    if an {
        json!({"art": "Abo", "sitzung": sitzung, "an": true, "spalten": groesse.0, "zeilen": groesse.1})
    } else {
        json!({"art": "Abo", "sitzung": sitzung, "an": false})
    }
}

/// `Groesse` for one subscription (R-L2 point 2).
#[must_use]
pub fn groesse(sitzung: &str, groesse: (usize, usize)) -> Value {
    json!({"art": "Groesse", "sitzung": sitzung, "spalten": groesse.0, "zeilen": groesse.1})
}

// ---- L3: rename, continue, revive, delete (`dienst` DI-L3) ----

/// `Umbenennen {sitzung, name}` (DI-L3 point 1, B12).
#[must_use]
pub fn umbenennen(sitzung: &str, name: &str) -> Value {
    json!({"art": "Umbenennen", "sitzung": sitzung, "name": name})
}

/// `Fortsetzen {sitzung}` (DI-L3 point 2, B11).
#[must_use]
pub fn fortsetzen(sitzung: &str) -> Value {
    json!({"art": "Fortsetzen", "sitzung": sitzung})
}

/// `Wiederbeleben {sitzung}` (DI-L3 point 3, B17).
#[must_use]
pub fn wiederbeleben(sitzung: &str) -> Value {
    json!({"art": "Wiederbeleben", "sitzung": sitzung})
}

/// `Loeschen {sitzung, bestaetigung}` with the confirmation equal to the id (DI-L3 point 4); the view sends it only after its question
/// (B16).
#[must_use]
pub fn loeschen(sitzung: &str) -> Value {
    json!({"art": "Loeschen", "sitzung": sitzung, "bestaetigung": sitzung})
}

/// The message `Gestartet {sitzung}`: the answer to `Fortsetzen` and `Wiederbeleben` (DI-L3 point 2) and to `Starten` on the local path
/// (DI-L11 point 8).
#[must_use]
pub fn gestartet_lesen(n: &Value) -> Option<String> {
    text(n, "sitzung")
}

// ---- L5: name, folder, project and history of a session in `Sitzungsliste` (`dienst` DI-L5) ----

/// The fields of DI-L5 of one entry of `Sitzungsliste`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Herkunft {
    /// The name; `""` when the session has none.
    pub name: String,
    /// The working folder; `""` for the session from `kind`.
    pub ordner: String,
    /// The project: the last part of `ordner`; `""` when `ordner` is empty or `/`.
    pub projekt: String,
    /// Whether an ended session crashed (DI-L5 point 2); `false` while it runs.
    pub abgestuerzt: bool,
    /// The id of the session this one continues.
    pub fortsetzung_von: Option<String>,
    /// The id of the session that continues this one.
    pub fortgesetzt_als: Option<String>,
}

/// DI-L5 of one entry: `Some` only when `name`, `ordner` and `projekt` are texts (they may be empty).
#[must_use]
pub fn herkunft_lesen(eintrag: &Value) -> Option<Herkunft> {
    Some(Herkunft {
        name: text_auch_leer(eintrag, "name")?,
        ordner: text_auch_leer(eintrag, "ordner")?,
        projekt: text_auch_leer(eintrag, "projekt")?,
        abgestuerzt: wahr(eintrag, "abgestuerzt"),
        fortsetzung_von: text(eintrag, "fortsetzung_von"),
        fortgesetzt_als: text(eintrag, "fortgesetzt_als"),
    })
}

// ---- L4: settings of version 2 (`regelebenen` sections 15.1, 15.5, 15.9, 15.10) ----

/// The catalog of the pilot (`regelebenen` 15.1): key, area (`nutzer` or `admin`) and the values of a choice. The message
/// `Einstellungen` names neither area nor choices; they are fixed here like the harness table in the service.
pub const KATALOG: [(&str, &str, &[&str]); 19] = [
    ("aussehen.thema", "nutzer", &["system", "hell", "dunkel"]),
    ("aussehen.farbsatz", "nutzer", &["grau", "blauschwarz"]),
    ("aussehen.terminal", "nutzer", &["dunkel", "hell", "thema"]),
    ("terminal.schriftgroesse", "nutzer", &[]),
    (
        "sitzung.harness",
        "nutzer",
        &["claude", "copilot", "pi", "opencode"],
    ),
    ("sitzung.modell", "nutzer", &[]),
    ("sitzung.effort", "nutzer", &[]),
    ("harnesses", "admin", &[]),
    ("modelle", "admin", &[]),
    ("effort.grenzen", "admin", &[]),
    ("optionen.copilot_ohne_rueckfrage", "admin", &[]),
    ("optionen.herabstufen", "admin", &[]),
    ("optionen.lokale_ca", "admin", &[]),
    ("optionen.copilot_telemetrie", "admin", &[]),
    ("optionen.copilot_eingeschraenktes_token", "admin", &[]),
    ("optionen.herstellerzugang", "admin", &[]),
    ("leseklassen", "admin", &[]),
    ("aufsicht.freigabefrist_ms", "admin", &[]),
    ("aufsicht.einloesefrist_ms", "admin", &[]),
];

/// The limit of the admins that binds a choice of the user (`regelebenen` 5.2): `harnesses` for `sitzung.harness`, `modelle`
/// for `sitzung.modell`.
#[must_use]
pub fn grenze_von(schluessel: &str) -> Option<&'static str> {
    match schluessel {
        "sitzung.harness" => Some("harnesses"),
        "sitzung.modell" => Some("modelle"),
        _ => None,
    }
}

/// The values of `wahl` that stand in at least one class of the limit `grenze`, in the order of `wahl` (`regelebenen` 5.2,
/// code `ebenen.ausser_grenze`). Without the key or with `{}` nothing is restricted (default of 15.1). A class with
/// another value than a list allows nothing. The view knows only the classes the limit names; a class of the company
/// that the limit leaves out is open for the service, but not here (stricter, never looser).
#[must_use]
pub fn in_grenze<S: AsRef<str>>(wahl: &[S], grenze: Option<&Value>) -> Vec<String> {
    let erlaubt = |w: &str| match grenze {
        None => true,
        Some(Value::Object(o)) => {
            o.is_empty()
                || o.values().any(|l| {
                    l.as_array()
                        .is_some_and(|l| l.iter().any(|x| x.as_str() == Some(w)))
                })
        }
        Some(_) => false,
    };
    wahl.iter()
        .map(AsRef::as_ref)
        .filter(|w| erlaubt(w))
        .map(str::to_owned)
        .collect()
}

/// One setting of `Einstellungen` version 2 (`regelebenen` 15.5).
#[derive(Clone, Debug, PartialEq)]
pub struct Einstellung {
    /// The key.
    pub schluessel: String,
    /// The effective value.
    pub wert: Value,
    /// The level that changed it last: `firma`, `gruppe` or `nutzer`.
    pub herkunft: String,
    /// The group, when the value comes from a group (`herkunft` `gruppe <schluessel>`).
    pub gruppe: Option<String>,
    /// Whether the viewer may change it.
    pub aenderbar: bool,
    /// The reason of a lock: `nur_strenger`, `nur_admin` or `fest`.
    pub grund: Option<String>,
    /// `nutzer` or `admin` (from [`KATALOG`]).
    pub bereich: String,
    /// The values of a choice (from [`KATALOG`]).
    pub wahl: Vec<String>,
}

/// The message `Einstellungen`: `Some((admin, werte))` only in version 2 (`fassung: 2`, `regelebenen` 15.10). A key outside the
/// catalog stays away: `setzen` refuses it (`ebenen.einstellung_unbekannt`).
#[must_use]
pub fn einstellungen_lesen(n: &Value) -> Option<(bool, Vec<Einstellung>)> {
    if n.get("fassung").and_then(Value::as_i64) != Some(2) {
        return None;
    }
    let werte = n.get("werte")?.as_object()?;
    let mut aus = Vec::new();
    for (schluessel, bereich, wahl) in KATALOG {
        let Some(v) = werte.get(schluessel) else {
            continue;
        };
        let (Some(wert), Some(herkunft), Some(aenderbar)) = (
            v.get("wert"),
            text(v, "herkunft"),
            v.get("aenderbar").and_then(Value::as_bool),
        ) else {
            return None;
        };
        let (herkunft, gruppe) = match herkunft.split_once(' ') {
            Some(("gruppe", g)) => ("gruppe".to_owned(), Some(g.to_owned())),
            _ => (herkunft, None),
        };
        aus.push(Einstellung {
            schluessel: schluessel.to_owned(),
            wert: wert.clone(),
            herkunft,
            gruppe,
            aenderbar,
            grund: text(v, "grund"),
            bereich: bereich.to_owned(),
            wahl: wahl.iter().map(|x| (*x).to_owned()).collect(),
        });
    }
    Some((wahr(n, "admin"), aus))
}

/// `EinstellungSetzen {schluessel, wert, bereich, ebene}` (`regelebenen` 15.9); whether it holds decides `setzen`.
#[must_use]
pub fn einstellung_setzen(schluessel: &str, wert: &Value, bereich: &str, ebene: &str) -> Value {
    json!({"art": "EinstellungSetzen", "schluessel": schluessel, "wert": wert, "bereich": bereich, "ebene": ebene})
}

// ---- protection layers in effect: read only, page "Aufsicht und Meldungen" (proposal, like the gaps L1 to L11) ----

/// The message `Schutzstand` (decision of 27.11.2026; the service sends it at connect and after each change to every viewer). Only
/// what was measured, each with its date:
/// `{"art": "Schutzstand", "einschluss": {"probelauf": "<JJJJ-MM-TT>" | null}, "halter": {"eingerichtet": "<JJJJ-MM-TT>" | null},
/// "letzter_start": {"zeit": <ms>, "ergebnis": "gestartet" | "<lauf.*>"} | null, "netzziele": [{"herkunft", "anzahl"}]}`.
/// Numbers per origin, never a host. Information only: the view shows it and sends nothing back.
#[derive(Clone, Debug, PartialEq)]
pub struct Schutzstand {
    /// The date of the green trial run of the confinement at the installation, if known.
    pub probelauf: Option<String>,
    /// The date the holder was set up at the installation, if known.
    pub eingerichtet: Option<String>,
    /// The last start since the service started: time in ms and `gestartet` or the reason of `lauf`.
    pub letzter_start: Option<(f64, String)>,
    /// The number of released network destinations per origin, as the service names the origin.
    pub netzziele: Vec<(String, u64)>,
}

/// A date field: a string or `null`; anything else is not the form (`Err`).
fn datum(v: &Value, objekt: &str, feld: &str) -> Result<Option<String>, ()> {
    match v
        .get(objekt)
        .and_then(Value::as_object)
        .and_then(|o| o.get(feld))
    {
        Some(Value::Null) => Ok(None),
        Some(Value::String(d)) => Ok(Some(d.clone())),
        _ => Err(()),
    }
}

/// Reads `Schutzstand`; a message in another form gives `None`, and the section stays away (OF8 point 3).
#[must_use]
pub fn schutzstand_lesen(n: &Value) -> Option<Schutzstand> {
    let netzziele = n
        .get("netzziele")?
        .as_array()?
        .iter()
        .map(|z| Some((text(z, "herkunft")?, z.get("anzahl")?.as_u64()?)))
        .collect::<Option<Vec<_>>>()?;
    let letzter_start = match n.get("letzter_start")? {
        Value::Null => None,
        l => Some((l.get("zeit")?.as_f64()?, text(l, "ergebnis")?)),
    };
    Some(Schutzstand {
        probelauf: datum(n, "einschluss", "probelauf").ok()?,
        eingerichtet: datum(n, "halter", "eingerichtet").ok()?,
        letzter_start,
        netzziele,
    })
}

/// One deadline of a MCP server of the message `Fristen` (decision of 27.11.2026, form agreed with the app window):
/// `{"art": "Fristen", "fristen": [{"schluessel": "aufsicht.freigabefrist_ms" | "aufsicht.einloesefrist_ms", "server": "<name>",
/// "wert": <ms>, "herkunft": "firmendatei" | "katalog"}]}`. Information only.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Frist {
    /// The key of the deadline.
    pub schluessel: String,
    /// The MCP server.
    pub server: String,
    /// The deadline in milliseconds.
    pub wert: u64,
    /// `firmendatei` or `katalog`.
    pub herkunft: String,
}

/// Reads `Fristen`; a message in another form gives `None`, and the table stays away (OF8 point 3).
#[must_use]
pub fn fristen_lesen(n: &Value) -> Option<Vec<Frist>> {
    n.get("fristen")?
        .as_array()?
        .iter()
        .map(|f| {
            Some(Frist {
                schluessel: text(f, "schluessel")?,
                server: text(f, "server")?,
                wert: f.get("wert")?.as_u64()?,
                herkunft: text(f, "herkunft")?,
            })
        })
        .collect()
}

// ---- L6: requests, decisions and the guard history (`zustandskern` R-L6, `dienst` DI-L6) ----

/// The states that end a request (R-L6 point 5).
pub const ENDZUSTAENDE: [&str; 3] = ["erteilt", "abgelehnt", "abgelaufen"];

/// One message `Anfrage` (`zustandskern` section 5): a request changed its state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Anfragestand {
    /// Request id.
    pub id: String,
    /// The session that asks.
    pub sitzung: String,
    /// The tool.
    pub werkzeug: String,
    /// Preview of the command.
    pub vorschau: String,
    /// `offen`, `erteilt`, `abgelehnt` or `abgelaufen`.
    pub zustand: String,
    /// The reason of a refusal (R-L6 point 3); empty otherwise.
    pub begruendung: String,
}

impl Anfragestand {
    /// Whether the request has ended: then the message is one line of the guard history (R-L6 point 5).
    #[must_use]
    pub fn beendet(&self) -> bool {
        ENDZUSTAENDE.contains(&self.zustand.as_str())
    }
}

/// The message `Anfrage`; `None` without `id` or `zustand`.
#[must_use]
pub fn anfrage_lesen(n: &Value) -> Option<Anfragestand> {
    Some(Anfragestand {
        id: text(n, "id")?,
        zustand: text(n, "zustand")?,
        sitzung: text(n, "sitzung").unwrap_or_default(),
        werkzeug: text(n, "werkzeug").unwrap_or_default(),
        vorschau: text(n, "vorschau").unwrap_or_default(),
        begruendung: text(n, "begruendung").unwrap_or_default(),
    })
}

/// `Freigabe {anfragen, entscheidung, (begruendung)}` (R-L6 point 1, DI-L6 point 4): `erteilen` or `ablehnen`; a reason only with
/// `ablehnen` and only when not empty (R-L6 point 2), never with `sitzung` and never with `nachweis` (the service takes the person from
/// the partner of the local path; a field `nachweis` is `feld_verboten`). One id per event: more are `sammelfreigabe`.
#[must_use]
pub fn freigabe(anfrage: &str, erteilen: bool, begruendung: &str) -> Value {
    let mut o = Map::new();
    o.insert("art".into(), json!("Freigabe"));
    o.insert("anfragen".into(), json!([anfrage]));
    o.insert(
        "entscheidung".into(),
        json!(if erteilen { "erteilen" } else { "ablehnen" }),
    );
    if !erteilen && !begruendung.is_empty() {
        o.insert("begruendung".into(), json!(begruendung));
    }
    Value::Object(o)
}

// ---- L9: the working folder of a session (`dienst`, addendum L9) ----

/// One entry of `Ordnerinhalt`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Eintrag {
    /// The name.
    pub name: String,
    /// `datei`, `ordner` or `anderes`.
    pub art: String,
    /// The size of a regular file in bytes.
    pub groesse: Option<u64>,
}

impl Eintrag {
    /// Whether it is a folder.
    #[must_use]
    pub fn ordner(&self) -> bool {
        self.art == "ordner"
    }
}

/// The message `Ordnerinhalt {sitzung, pfad, eintraege, abgeschnitten, t}`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ordnerinhalt {
    /// The session.
    pub sitzung: String,
    /// The path as sent.
    pub pfad: String,
    /// The entries in byte order of their names.
    pub eintraege: Vec<Eintrag>,
    /// Whether a limit or an unreadable entry cut the list.
    pub abgeschnitten: bool,
}

/// Reads `Ordnerinhalt`.
#[must_use]
pub fn ordnerinhalt_lesen(n: &Value) -> Option<Ordnerinhalt> {
    Some(Ordnerinhalt {
        sitzung: text(n, "sitzung")?,
        pfad: text_auch_leer(n, "pfad").unwrap_or_default(),
        eintraege: liste(n, "eintraege")
            .iter()
            .filter_map(|x| {
                Some(Eintrag {
                    name: text(x, "name")?,
                    art: text(x, "art").unwrap_or_else(|| "anderes".to_owned()),
                    groesse: x.get("groesse").and_then(Value::as_u64),
                })
            })
            .collect(),
        abgeschnitten: wahr(n, "abgeschnitten"),
    })
}

/// One hit of `Suchtreffer`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Treffer {
    /// Path below the working folder, parts separated by `/`.
    pub pfad: String,
    /// Line number from 1.
    pub zeile: u64,
    /// The line, at most 200 bytes.
    pub auszug: String,
}

/// The message `Suchtreffer {sitzung, muster, treffer, abgeschnitten, t}`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Suchtreffer {
    /// The session.
    pub sitzung: String,
    /// The pattern as sent.
    pub muster: String,
    /// The hits.
    pub treffer: Vec<Treffer>,
    /// Whether a limit ended the search.
    pub abgeschnitten: bool,
}

/// Reads `Suchtreffer`.
#[must_use]
pub fn suchtreffer_lesen(n: &Value) -> Option<Suchtreffer> {
    Some(Suchtreffer {
        sitzung: text(n, "sitzung")?,
        muster: text_auch_leer(n, "muster").unwrap_or_default(),
        treffer: liste(n, "treffer")
            .iter()
            .filter_map(|x| {
                Some(Treffer {
                    pfad: text(x, "pfad")?,
                    zeile: x.get("zeile").and_then(Value::as_u64).unwrap_or(0),
                    auszug: text_auch_leer(x, "auszug").unwrap_or_default(),
                })
            })
            .collect(),
        abgeschnitten: wahr(n, "abgeschnitten"),
    })
}

/// `OrdnerLesen {sitzung, pfad}`.
#[must_use]
pub fn ordner_lesen(sitzung: &str, pfad: &str) -> Value {
    json!({"art": "OrdnerLesen", "sitzung": sitzung, "pfad": pfad})
}

/// `InhaltSuchen {sitzung, muster}`.
#[must_use]
pub fn inhalt_suchen(sitzung: &str, muster: &str) -> Value {
    json!({"art": "InhaltSuchen", "sitzung": sitzung, "muster": muster})
}

// ---- L10: `Startwahl` with efforts and the start structure of version 2 (`start` "Fassung 2") ----

/// One entry of `routen[].efforts` of `Startwahl` version 2.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Effort {
    /// The harness.
    pub harness: String,
    /// The model for which `erlaubt` holds: the model of the route, else `auto`.
    pub modell: String,
    /// The allowed efforts; empty means only `null`.
    pub erlaubt: Vec<String>,
    /// The default of the user when it is allowed, else `None`.
    pub vorgabe: Option<String>,
}

/// The efforts of one route of `Startwahl` version 2; empty without the field.
#[must_use]
pub fn efforts_lesen(route: &Value) -> Vec<Effort> {
    liste(route, "efforts")
        .iter()
        .filter_map(|e| {
            Some(Effort {
                harness: text(e, "harness")?,
                modell: text(e, "modell").unwrap_or_default(),
                erlaubt: liste(e, "erlaubt")
                    .iter()
                    .filter_map(|x| x.as_str().filter(|s| !s.is_empty()).map(str::to_owned))
                    .collect(),
                vorgabe: text(e, "vorgabe"),
            })
        })
        .collect()
}

/// The start structure of version 2: version 1 with `name` and `effort`. Both keys are required; `null` stands explicitly
/// (`start`, "Gemeinsame Regeln der Fassung 2").
#[must_use]
pub fn struktur_fassung2(mut fassung1: Value, name: Option<&str>, effort: Option<&str>) -> Value {
    if let Some(o) = fassung1.as_object_mut() {
        o.insert("fassung".into(), json!(2));
        o.insert("name".into(), name.map_or(Value::Null, |n| json!(n)));
        o.insert("effort".into(), effort.map_or(Value::Null, |e| json!(e)));
    }
    fassung1
}

#[cfg(test)]
mod tests_schutz {
    use super::{fristen_lesen, schutzstand_lesen};

    #[test]
    fn the_deadlines_per_server_are_read_in_one_form_only() {
        let n = json!({"art": "Fristen", "fristen": [
            {"schluessel": "aufsicht.freigabefrist_ms", "server": "suche", "wert": 60000, "herkunft": "firmendatei"},
            {"schluessel": "aufsicht.einloesefrist_ms", "server": "suche", "wert": 300_000, "herkunft": "katalog"}]});
        let f = fristen_lesen(&n).expect("agreed form");
        assert_eq!(
            (f.len(), f[0].wert, f[1].herkunft.as_str()),
            (2, 60000, "katalog")
        );
        assert!(
            fristen_lesen(&json!({"art": "Fristen", "fristen": [{"server": "suche", "wert": 1}]}))
                .is_none()
        );
        assert!(fristen_lesen(&json!({"art": "Fristen"})).is_none());
    }
    use serde_json::json;

    #[test]
    fn the_protection_layers_are_read_in_one_form_only() {
        let n = json!({"art": "Schutzstand", "einschluss": {"probelauf": "2026-11-27"}, "halter": {"eingerichtet": null},
                       "letzter_start": {"zeit": 1_764_280_680_000_u64, "ergebnis": "lauf.einschluss_fehlt"},
                       "netzziele": [{"herkunft": "firma", "anzahl": 3}, {"herkunft": "gruppe g-eins", "anzahl": 1}]});
        let s = schutzstand_lesen(&n).expect("the form of 27.11.2026");
        assert_eq!(s.probelauf.as_deref(), Some("2026-11-27"));
        assert_eq!(s.eingerichtet, None);
        assert_eq!(
            s.letzter_start.as_ref().map(|(_, e)| e.as_str()),
            Some("lauf.einschluss_fehlt")
        );
        assert_eq!(
            s.netzziele,
            [("firma".to_owned(), 3), ("gruppe g-eins".to_owned(), 1)]
        );
        let ohne_start = json!({"art": "Schutzstand", "einschluss": {"probelauf": null}, "halter": {"eingerichtet": "2026-11-27"},
                                "letzter_start": null, "netzziele": []});
        assert!(schutzstand_lesen(&ohne_start).is_some_and(|s| s.letzter_start.is_none()));
        // the form of the first proposal and an „an“ are not this form
        assert!(schutzstand_lesen(&json!({"art": "Schutzstand", "einschluss": true, "halter": true, "letzter_start": null,
                                          "netzziele": []})).is_none());
        assert!(
            schutzstand_lesen(&json!({"art": "Schutzstand", "einschluss": {"probelauf": "x"}}))
                .is_none()
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn earlier_forms_give_nothing() {
        // the list of the core before R-L1, and an empty list (the same before and after)
        assert_eq!(
            agenten_lesen(&json!({"agenten": [{"worker": "w1", "zustand": "laeuft"}]})),
            None
        );
        assert_eq!(agenten_lesen(&json!({"agenten": []})), None);
        assert_eq!(herkunft_lesen(&json!({"id": "s-1"})), None);
        // settings of version 1: value, default and origin without the right to change
        assert_eq!(
            einstellungen_lesen(
                &json!({"werte": {"sitzung.effort": {"wert": "low", "vorgabe": null, "herkunft": "firma"}}})
            ),
            None
        );
        assert_eq!(efforts_lesen(&json!({"schluessel": "r"})), Vec::new());
    }

    #[test]
    fn the_list_of_the_service_is_read() {
        let n = json!({"agenten": [
            {"worker": "s-4", "sitzung": "s-1", "rolle": "bauer", "modell": "m", "zustand": "laeuft", "nr": 4},
            {"worker": "s-5", "sitzung": "s-1", "rolle": "pruefer", "modell": "m", "zustand": "beendet", "nr": null, "code": 1}]});
        let a = agenten_lesen(&n).expect("L1");
        assert_eq!((a[0].nr, a[0].laeuft()), (Some(4), true));
        assert_eq!(
            (a[1].nr, a[1].laeuft(), a[1].code.as_deref()),
            (None, false, Some("1"))
        );
        assert!(agenten_mit_raster(&n));
        // the list of the core (R-L1): session, no role, no number
        let kern = json!({"agenten": [{"worker": "w1", "sitzung": "s-1", "zustand": "x", "gesehen_um": 0, "quelle": "q"}]});
        assert_eq!(agenten_lesen(&kern).expect("R-L1")[0].rolle, "worker");
        assert!(!agenten_mit_raster(&kern));
    }

    #[test]
    fn events_have_the_names_of_the_contracts() {
        assert_eq!(
            abo("s-4", true, (80, 24)),
            json!({"art": "Abo", "sitzung": "s-4", "an": true, "spalten": 80, "zeilen": 24})
        );
        assert_eq!(
            loeschen("s-2"),
            json!({"art": "Loeschen", "sitzung": "s-2", "bestaetigung": "s-2"})
        );
        assert_eq!(
            freigabe("a1", true, "egal"),
            json!({"art": "Freigabe", "anfragen": ["a1"], "entscheidung": "erteilen"})
        );
        assert_eq!(
            freigabe("a1", false, "zu breit"),
            json!({"art": "Freigabe", "anfragen": ["a1"], "entscheidung": "ablehnen", "begruendung": "zu breit"})
        );
        assert_eq!(freigabe("a1", false, "").get("begruendung"), None);
    }

    /// DI-E5 points 1, 3 and 9: version 1 with at most 64 texts, any other form says nothing.
    #[test]
    fn the_list_of_events_in_version_1_only() {
        let l = ereignisliste_lesen(
            &json!({"art": "Ereignisliste", "fassung": 1, "ereignisse": ["Abo", "Loeschen", "Neu"]}),
        );
        assert_eq!(
            l,
            Some(vec![
                "Abo".to_owned(),
                "Loeschen".to_owned(),
                "Neu".to_owned()
            ])
        );
        assert_eq!(
            ereignisliste_lesen(&json!({"fassung": 1, "ereignisse": []})),
            Some(Vec::new())
        );
        let voll: Vec<String> = (0..64).map(|i| format!("A{i}")).collect();
        assert_eq!(
            ereignisliste_lesen(&json!({"fassung": 1, "ereignisse": voll})).map(|v| v.len()),
            Some(64)
        );
        let zu_viele: Vec<String> = (0..65).map(|i| format!("A{i}")).collect();
        for n in [
            json!({"fassung": 2, "ereignisse": ["Abo"]}),
            json!({"ereignisse": ["Abo"]}),
            json!({"fassung": 1}),
            json!({"fassung": 1, "ereignisse": "Abo"}),
            json!({"fassung": 1, "ereignisse": ["Abo", 3]}),
            json!({"fassung": 1, "ereignisse": zu_viele}),
        ] {
            assert_eq!(ereignisliste_lesen(&n), None, "{n}");
        }
    }

    #[test]
    fn the_fields_of_l5_may_be_empty() {
        let h = herkunft_lesen(&json!({"id": "s-0", "name": "", "ordner": "", "projekt": ""}))
            .expect("the session from kind");
        assert!(h.ordner.is_empty() && !h.abgestuerzt);
        let h = herkunft_lesen(
            &json!({"name": "n", "ordner": "/a/kunde-x", "projekt": "kunde-x",
            "abgestuerzt": true, "fortgesetzt_als": "s-9"}),
        )
        .expect("L5");
        assert!(h.abgestuerzt);
        assert_eq!(h.fortgesetzt_als.as_deref(), Some("s-9"));
    }

    #[test]
    fn settings_of_version_2_take_area_and_choice_from_the_catalog() {
        let (admin, e) = einstellungen_lesen(&json!({"fassung": 2, "admin": true, "werte": {
            "aussehen.farbsatz": {"wert": "grau", "herkunft": "gruppe g-eins", "aenderbar": true, "grund": null},
            "optionen.herstellerzugang": {"wert": false, "herkunft": "firma", "aenderbar": false, "grund": "fest"},
            "unbekannt": {"wert": 1, "herkunft": "firma", "aenderbar": true, "grund": null}}}))
        .expect("L4");
        assert!(admin);
        assert_eq!(e.len(), 2);
        assert_eq!(
            (
                e[0].herkunft.as_str(),
                e[0].gruppe.as_deref(),
                e[0].wahl.len()
            ),
            ("gruppe", Some("g-eins"), 2)
        );
        assert_eq!(
            (e[1].bereich.as_str(), e[1].grund.as_deref()),
            ("admin", Some("fest"))
        );
    }

    #[test]
    fn requests_and_the_start_structure_of_version_2() {
        let a = anfrage_lesen(&json!({"art": "Anfrage", "id": "a1", "sitzung": "s-4", "zustand": "abgelehnt", "begruendung": "nein"}))
            .expect("Anfrage");
        assert!(a.beendet());
        assert_eq!(a.begruendung, "nein");
        let e = efforts_lesen(
            &json!({"efforts": [{"harness": "pi", "modell": "q", "erlaubt": ["off"], "vorgabe": null}]}),
        );
        assert_eq!(e[0].erlaubt, ["off"]);
        let s = struktur_fassung2(json!({"fassung": 1, "harness": "h"}), None, Some("high"));
        assert_eq!(
            (&s["fassung"], &s["name"], &s["effort"]),
            (&json!(2), &Value::Null, &json!("high"))
        );
    }
}
