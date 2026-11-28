//! The fields and messages of the gaps L1 to L10 (contract `seite`, part surface, section 4) that the view core of
//! the crate `ansicht` does not read yet.
//!
//! The window reads them here only so that the mock-up can show the target state. They are a proposal; once the
//! contracts `zustandskern` and `dienst` fix them, they belong into the shared view core, and this module goes away.

use serde_json::Value;

fn text(v: &Value, k: &str) -> String {
    match v.get(k) {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Number(n)) => n.to_string(),
        _ => String::new(),
    }
}

/// The extra fields of a session (L5).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Sitzungsplus {
    /// Id.
    pub id: String,
    /// Name.
    pub name: String,
    /// Working folder.
    pub ordner: String,
    /// Display name of the project.
    pub projekt: String,
}

/// A worker with its session (L1).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Arbeiter {
    /// Id.
    pub id: String,
    /// Session.
    pub sitzung: String,
    /// Role.
    pub rolle: String,
    /// Model.
    pub modell: String,
    /// State.
    pub zustand: String,
    /// Raster number (L2).
    pub nr: Option<u8>,
}

/// One setting with its effective value and origin (L4).
#[derive(Clone, Debug, PartialEq)]
pub struct Einstellung {
    /// Key.
    pub schluessel: String,
    /// Effective value.
    pub wert: Value,
    /// Origin as `sicht` gives it (`regelebenen` 15.5): `firma`, `gruppe <name>` or `nutzer`.
    pub herkunft: String,
    /// "nutzer" or "admin".
    pub bereich: String,
    /// Whether the viewer may change it.
    pub aenderbar: bool,
    /// The reason of a lock (`regelebenen` 15.5): `nur_admin`, `nur_strenger` or `fest`.
    pub grund: Option<String>,
}

/// One entry of the guard history (L6).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Verlauf {
    /// Text.
    pub text: String,
    /// Granted or refused.
    pub erteilt: bool,
    /// Reason of a refusal.
    pub begruendung: String,
}

/// The measured state of the protection layers (decision of the operator, 27.11.2026): information, no switch, only what
/// was measured, with its date; never "on" or "running". No message of the service carries it yet; `Schutzstand` is the
/// proposal of the window: `{"einschluss": {"probelauf": "<date>"}, "halter": {"eingerichtet": "<date>"},
/// "letzter_start": {"zeit": <ms>, "ergebnis": "<text>"}, "netzziele": [{"herkunft", "anzahl"}]}`. A part that is `null`
/// or missing is `None` here, and the page says so.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Schutz {
    /// The date (`JJJJ-MM-TT`) of the green trial run of the enclosure at the installation.
    pub probelauf: Option<String>,
    /// The date (`JJJJ-MM-TT`) on which the installation set up the holder.
    pub eingerichtet: Option<String>,
    /// The last start: `None` when the service did not report it, `Some(None)` when there was no start yet (`null`),
    /// else the time in milliseconds since 1970 (UTC) and the result.
    pub letzter_start: Option<Option<(u64, String)>>,
    /// The number of released network targets per origin `firma` and `nutzer` (`gruppe` has no source and is left out);
    /// `None` when the service did not report them.
    pub netzziele: Option<Vec<(String, u64)>>,
}

/// The effective deadline of a release of one MCP server (`regelebenen` 15.1.2, decision of the operator, 27.11.2026):
/// the smaller value of the catalogue and the company file. No message of the service carries it yet; `Fristen` is the
/// proposal of the window.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Frist {
    /// The key of the catalogue (`aufsicht.freigabefrist_ms` or `aufsicht.einloesefrist_ms`).
    pub schluessel: String,
    /// The key of the server in the block `mcp`.
    pub server: String,
    /// The effective value in milliseconds.
    pub wert: u64,
    /// `firmendatei` or `katalog`.
    pub herkunft: String,
}

/// Everything of the gaps the window has read so far.
#[derive(Clone, Debug, Default)]
pub struct Zusatz {
    /// Per session (L5).
    pub sitzungen: Vec<Sitzungsplus>,
    /// Workers (L1).
    pub arbeiter: Vec<Arbeiter>,
    /// Settings (L4).
    pub einstellungen: Vec<Einstellung>,
    /// Whether the viewer is an admin (L4; the service decides).
    pub admin: bool,
    /// Guard history (L6).
    pub verlauf: Vec<Verlauf>,
    /// Workers of the open requests (L6), by request id.
    pub anfrage_worker: Vec<(String, String)>,
    /// The listed folder: path and entries with "is a folder" (L9).
    pub ordner: Option<(String, Vec<(String, bool)>)>,
    /// Search hits: path, line and text (L9).
    pub treffer: Vec<(String, String, String)>,
    /// Allowed efforts per route (L10).
    pub efforts: Vec<(String, Vec<String>)>,
    /// The default effort of the person (L10).
    pub effort_vorgabe: String,
    /// The state of the protection layers, once the service sent it.
    pub schutz: Option<Schutz>,
    /// The effective deadlines for each MCP server, in the order of the message.
    pub fristen: Vec<Frist>,
}

impl Zusatz {
    /// Reads a message of the stream; every other message changes nothing.
    #[allow(clippy::too_many_lines)]
    pub fn nachricht(&mut self, n: &Value) {
        let liste = |k: &str| {
            n.get(k)
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default()
        };
        match n.get("art").and_then(Value::as_str) {
            Some("Sitzungsliste") => {
                self.sitzungen = liste("sitzungen")
                    .iter()
                    .filter(|s| s.get("name").is_some() || s.get("projekt").is_some())
                    .map(|s| Sitzungsplus {
                        id: text(s, "id"),
                        name: text(s, "name"),
                        ordner: text(s, "ordner"),
                        projekt: text(s, "projekt"),
                    })
                    .collect();
            }
            Some("Agenten") => {
                self.arbeiter = liste("agenten")
                    .iter()
                    .filter(|a| a.get("sitzung").is_some())
                    .map(|a| Arbeiter {
                        id: text(a, "worker"),
                        sitzung: text(a, "sitzung"),
                        rolle: text(a, "rolle"),
                        modell: text(a, "modell"),
                        zustand: text(a, "zustand"),
                        nr: a
                            .get("nr")
                            .and_then(Value::as_u64)
                            .and_then(|x| u8::try_from(x).ok()),
                    })
                    .collect();
            }
            Some("Anfragen") => {
                self.anfrage_worker = liste("anfragen")
                    .iter()
                    .map(|a| (text(a, "id"), text(a, "worker")))
                    .collect();
            }
            Some("Guardverlauf") => {
                self.verlauf = liste("eintraege")
                    .iter()
                    .map(|v| Verlauf {
                        text: text(v, "text"),
                        erteilt: v.get("erteilt").and_then(Value::as_bool).unwrap_or(false),
                        begruendung: text(v, "begruendung"),
                    })
                    .collect();
            }
            Some("Einstellungen") if n.get("eintraege").is_some() => {
                self.admin = n.get("admin").and_then(Value::as_bool).unwrap_or(false);
                self.einstellungen = liste("eintraege")
                    .iter()
                    .map(|e| Einstellung {
                        schluessel: text(e, "schluessel"),
                        wert: e.get("wert").cloned().unwrap_or(Value::Null),
                        herkunft: text(e, "herkunft"),
                        bereich: text(e, "bereich"),
                        aenderbar: e.get("aenderbar").and_then(Value::as_bool).unwrap_or(false),
                        grund: e.get("grund").and_then(Value::as_str).map(str::to_owned),
                    })
                    .collect();
            }
            Some("Startwahl") => {
                if let Some(Value::Object(m)) = n.get("efforts") {
                    self.efforts = m
                        .iter()
                        .map(|(r, l)| {
                            (
                                r.clone(),
                                l.as_array()
                                    .map(|a| {
                                        a.iter()
                                            .filter_map(Value::as_str)
                                            .map(str::to_owned)
                                            .collect()
                                    })
                                    .unwrap_or_default(),
                            )
                        })
                        .collect();
                }
                self.effort_vorgabe = text(n, "effort_vorgabe");
            }
            Some("Ordnerinhalt") => {
                let eintraege = liste("eintraege")
                    .iter()
                    .map(|e| {
                        (
                            text(e, "name"),
                            e.get("ordner").and_then(Value::as_bool).unwrap_or(false),
                        )
                    })
                    .collect();
                self.ordner = Some((text(n, "pfad"), eintraege));
            }
            Some("Schutzstand") => {
                let datum = |teil: &str, k: &str| {
                    n.get(teil)
                        .and_then(|t| t.get(k))
                        .and_then(Value::as_str)
                        .map(str::to_owned)
                };
                let letzter_start = match n.get("letzter_start") {
                    None => None,
                    Some(Value::Null) => Some(None),
                    Some(s) => s
                        .get("zeit")
                        .and_then(Value::as_u64)
                        .map(|z| Some((z, text(s, "ergebnis")))),
                };
                self.schutz = Some(Schutz {
                    probelauf: datum("einschluss", "probelauf"),
                    eingerichtet: datum("halter", "eingerichtet"),
                    letzter_start,
                    netzziele: n.get("netzziele").and_then(Value::as_array).map(|l| {
                        l.iter()
                            .filter_map(|z| Some((text(z, "herkunft"), z.get("anzahl")?.as_u64()?)))
                            .filter(|(h, _)| h == "firma" || h == "nutzer")
                            .collect()
                    }),
                });
            }
            Some("Fristen") => {
                self.fristen = liste("fristen")
                    .iter()
                    .filter_map(|f| {
                        Some(Frist {
                            schluessel: text(f, "schluessel"),
                            server: text(f, "server"),
                            wert: f.get("wert")?.as_u64()?,
                            herkunft: text(f, "herkunft"),
                        })
                    })
                    .collect();
            }
            Some("Suchtreffer") => {
                self.treffer = liste("treffer")
                    .iter()
                    .map(|t| (text(t, "pfad"), text(t, "zeile"), text(t, "text")))
                    .collect();
            }
            _ => {}
        }
    }

    /// The extra fields of session `id`.
    #[must_use]
    pub fn sitzung(&self, id: &str) -> Option<&Sitzungsplus> {
        self.sitzungen.iter().find(|s| s.id == id)
    }

    /// The workers of session `id`.
    #[must_use]
    pub fn arbeiter_von(&self, id: &str) -> Vec<&Arbeiter> {
        self.arbeiter.iter().filter(|a| a.sitzung == id).collect()
    }

    /// The setting `schluessel`.
    #[must_use]
    pub fn einstellung(&self, schluessel: &str) -> Option<&Einstellung> {
        self.einstellungen
            .iter()
            .find(|e| e.schluessel == schluessel)
    }
}
