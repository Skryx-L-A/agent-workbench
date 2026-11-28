//! The thin decision point (contract 6, `rust/spec/stelle/README.md`): session start and the release of each
//! MCP write call.
//!
//! The core is two functions without input and output, clock or randomness: [`laden`] checks the settings file
//! and builds the state, [`entscheide`] decides one event and returns the new state, the answer and the lines of
//! the event. The shell around the core (contract 6, section 13: sockets, writing the lines) is not here; it gets
//! the lines of one event as one sequence through [`Ergebnis::zeilen_texte`] or [`verketten`].

mod code;
pub mod einstellungen;
mod form;
#[cfg(unix)]
pub mod huelle;
mod kern;
pub mod pruefer;
pub mod start;
mod worker;
mod zeile;

use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

use serde_json::{Value, json};

pub use crate::einstellungen::Einstellungen;
pub use ::einstellungen::Routenart;
pub use code::Code;
pub use kern::entscheide;
pub use pruefer::{Nachweispruefer, PrueferH1, herausforderung, nachweis_h1};

/// The key of `h1` for the service of the human: under Unix [`huelle::schluessel_lesen`]. Other systems read no key, so their
/// service issues no proof and every release ends with `schluessel_fehlt`.
#[cfg(unix)]
#[must_use]
pub fn schluessel_h1(pfad: &std::path::Path) -> Option<[u8; 32]> {
    huelle::schluessel_lesen(pfad).ok()
}

/// See the Unix function of the same name.
#[cfg(not(unix))]
#[must_use]
pub fn schluessel_h1(_pfad: &std::path::Path) -> Option<[u8; 32]> {
    None
}
pub use worker::{VORGABE_MAX, WorkerRegeln, worker_regeln_lesen};
pub use zeile::{Ende, Protokollzeile, Schreibaufruf, verketten};

/// The largest time in milliseconds: 9999-12-31T23:59:59.999Z.
pub const ZEIT_MAX: i64 = 253_402_300_799_999;

/// The kind of the caller, as the transport layer measures it (section 3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Aufrufer {
    /// The core service of the sessions.
    Kern,
    /// The MCP hub of a session.
    Hub,
    /// The bridge of the interface for the signed-in human.
    Mensch,
    /// Any process inside the enclosure of a run.
    Agent,
    /// The clock of the point itself.
    Takt,
    /// The measurement failed or found no kind.
    Unbekannt,
}

impl Aufrufer {
    /// The fixed text of the kind.
    #[must_use]
    pub fn text(self) -> &'static str {
        match self {
            Self::Kern => "kern",
            Self::Hub => "hub",
            Self::Mensch => "mensch",
            Self::Agent => "agent",
            Self::Takt => "takt",
            Self::Unbekannt => "unbekannt",
        }
    }
}

/// One of the seven operations with its input as JSON value (section 4).
#[derive(Debug, Clone, PartialEq)]
pub enum Vorgang {
    /// `sitzung.starten`.
    SitzungStarten(Value),
    /// `mcp.anfragen`.
    McpAnfragen(Value),
    /// `mcp.entscheiden`.
    McpEntscheiden(Value),
    /// `mcp.einloesen`.
    McpEinloesen(Value),
    /// `mcp.offene`.
    McpOffene(Value),
    /// `mcp.stand`.
    McpStand(Value),
    /// `ablauf.pruefen`.
    AblaufPruefen(Value),
    /// `person.pseudonym` (section 14.4).
    PersonPseudonym(Value),
    /// `worker.anfordern` (section 14.5).
    WorkerAnfordern(Value),
}

impl Vorgang {
    /// The operation of this name with its input, or `None` for a name the core does not know.
    #[must_use]
    pub fn aus_name(name: &str, eingabe: Value) -> Option<Self> {
        Some(match name {
            "sitzung.starten" => Self::SitzungStarten(eingabe),
            "mcp.anfragen" => Self::McpAnfragen(eingabe),
            "mcp.entscheiden" => Self::McpEntscheiden(eingabe),
            "mcp.einloesen" => Self::McpEinloesen(eingabe),
            "mcp.offene" => Self::McpOffene(eingabe),
            "mcp.stand" => Self::McpStand(eingabe),
            "ablauf.pruefen" => Self::AblaufPruefen(eingabe),
            "person.pseudonym" => Self::PersonPseudonym(eingabe),
            "worker.anfordern" => Self::WorkerAnfordern(eingabe),
            _ => return None,
        })
    }

    /// The name of the operation.
    #[must_use]
    pub fn name(&self) -> &'static str {
        match self {
            Self::SitzungStarten(_) => "sitzung.starten",
            Self::McpAnfragen(_) => "mcp.anfragen",
            Self::McpEntscheiden(_) => "mcp.entscheiden",
            Self::McpEinloesen(_) => "mcp.einloesen",
            Self::McpOffene(_) => "mcp.offene",
            Self::McpStand(_) => "mcp.stand",
            Self::AblaufPruefen(_) => "ablauf.pruefen",
            Self::PersonPseudonym(_) => "person.pseudonym",
            Self::WorkerAnfordern(_) => "worker.anfordern",
        }
    }

    /// The input of the operation.
    #[must_use]
    pub fn eingabe(&self) -> &Value {
        match self {
            Self::SitzungStarten(e)
            | Self::McpAnfragen(e)
            | Self::McpEntscheiden(e)
            | Self::McpEinloesen(e)
            | Self::McpOffene(e)
            | Self::McpStand(e)
            | Self::AblaufPruefen(e)
            | Self::PersonPseudonym(e)
            | Self::WorkerAnfordern(e) => e,
        }
    }

    /// The single kind of caller allowed for the operation (section 3).
    #[must_use]
    pub fn erlaubt(&self) -> Aufrufer {
        match self {
            Self::SitzungStarten(_) | Self::PersonPseudonym(_) | Self::WorkerAnfordern(_) => {
                Aufrufer::Kern
            }
            Self::McpAnfragen(_) | Self::McpEinloesen(_) | Self::McpStand(_) => Aufrufer::Hub,
            Self::McpEntscheiden(_) | Self::McpOffene(_) => Aufrufer::Mensch,
            Self::AblaufPruefen(_) => Aufrufer::Takt,
        }
    }
}

/// One event: the measured caller and the operation with its input.
#[derive(Debug, Clone, PartialEq)]
pub struct Ereignis {
    /// Kind of the caller, measured by the transport layer.
    pub aufrufer: Aufrufer,
    /// Operation with its input.
    pub vorgang: Vorgang,
}

/// The state of a request (section 5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Anfragezustand {
    /// Waiting for the decision of a human.
    Offen,
    /// Released, waiting for the hub to redeem.
    Erteilt,
    /// Refused by a human (end).
    Abgelehnt,
    /// The deadline passed (end).
    Abgelaufen,
    /// Redeemed by the hub (end).
    Eingeloest,
}

impl Anfragezustand {
    /// The fixed text of the state.
    #[must_use]
    pub fn text(self) -> &'static str {
        match self {
            Self::Offen => "offen",
            Self::Erteilt => "erteilt",
            Self::Abgelehnt => "abgelehnt",
            Self::Abgelaufen => "abgelaufen",
            Self::Eingeloest => "eingeloest",
        }
    }
}

/// A started session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Sitzung {
    pub(crate) route: String,
    pub(crate) kunde: Option<String>,
    pub(crate) harness: String,
    /// The orchestrator of a worker (started with a promise, section 15.2), `None` for any other session.
    pub(crate) orchestrator: Option<String>,
}

/// A promise of `worker.anfordern` (section 15.3): for which orchestrator and with which values the worker may start, and
/// until when. It is used up by the start it binds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Zusage {
    pub(crate) orchestrator: String,
    pub(crate) harness: String,
    pub(crate) modell: String,
    pub(crate) effort: String,
    pub(crate) route: String,
    pub(crate) kunde: Option<String>,
    /// The end of the deadline: the promise holds while the time is below it.
    pub(crate) bis: i64,
}

impl Zusage {
    /// Section 15.2, stage 4 step 2: the promise belongs to this orchestrator, and `harness`, `modell`, `effort`, `route` and
    /// `kunde` of the start equal its values exactly. A start without `effort` (start structure version 1) never matches.
    pub(crate) fn bindet(&self, orchestrator: &str, start: &start::Start) -> bool {
        self.orchestrator == orchestrator
            && self.harness == start.harness
            && self.modell == start.modell
            && start.effort.as_deref() == Some(self.effort.as_str())
            && self.route == start.route
            && self.kunde == start.kunde
    }
}

/// A request with a release (never an argument, only its digest).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Anfrage {
    pub(crate) server: String,
    pub(crate) werkzeug: String,
    pub(crate) sitzung: String,
    pub(crate) person: String,
    pub(crate) digest: String,
    /// The preview, only while the request is open.
    pub(crate) vorschau: Option<String>,
    pub(crate) vorschau_sha256: Option<String>,
    pub(crate) angefragt_ms: i64,
    pub(crate) entscheidend: Option<String>,
    pub(crate) entschieden_ms: Option<i64>,
    pub(crate) zustand: Anfragezustand,
    /// End of the running deadline.
    pub(crate) bis: i64,
    /// Redeem deadline of the server, taken when the request was made.
    pub(crate) einloesefrist_ms: i64,
}

/// The store of contract 9 (section 4) as the core sees it: the pseudonym of a person to her identifier. The shell
/// holds the store; the core calls it once for each `person.pseudonym` after stages 0 to 3, like the proof checker.
pub trait Personenspeicher: Send + Sync {
    /// The pseudonym `ps_<32 hex>` of the person with this identifier, a new person when unknown; `None` for any
    /// failure of the store, then there is no pseudonym (`stelle.person_fehler`).
    fn pseudonym(&self, kennung: &str, jetzt_ms: i64) -> Option<String>;
}

/// The state of the point. It lives in memory only (E1): a new [`laden`] starts empty.
#[derive(Clone)]
pub struct Zustand {
    pub(crate) einstellungen: Einstellungen,
    pub(crate) pruefer: Arc<dyn Nachweispruefer>,
    /// The store of contract 9 behind `person.pseudonym`, set by the shell; without it there is no pseudonym.
    pub(crate) personen: Option<Arc<dyn Personenspeicher>>,
    /// The rules of the admins for workers, set by the shell from the default; without them every request is refused.
    pub(crate) worker: Option<WorkerRegeln>,
    pub(crate) sitzungen: BTreeMap<String, Sitzung>,
    /// Requests by their number n.
    pub(crate) anfragen: BTreeMap<u64, Anfrage>,
    pub(crate) zaehler: u64,
    /// Promises of `worker.anfordern` by their number n (section 15.3); a restart drops them all.
    pub(crate) zusagen: BTreeMap<u64, Zusage>,
    /// The counter of promise identifiers: only issued promises count.
    pub(crate) zusage_zaehler: u64,
    pub(crate) laden_ms: i64,
    pub(crate) zeit_max: i64,
}

impl Zustand {
    /// The request identifier of the number n: `mf-<laden_ms>-<n>`.
    pub(crate) fn kennung(&self, n: u64) -> String {
        format!("mf-{}-{n}", self.laden_ms)
    }

    /// The promise identifier of the number n: `zs-<laden_ms>-<n>` (section 15.3).
    pub(crate) fn zusage_kennung(&self, n: u64) -> String {
        format!("zs-{}-{n}", self.laden_ms)
    }

    /// The started sessions, sorted.
    #[must_use]
    pub fn sitzungen(&self) -> Vec<String> {
        self.sitzungen.keys().cloned().collect()
    }

    /// Every request (also the ended ones) with its state, by identifier.
    #[must_use]
    pub fn anfragen(&self) -> BTreeMap<String, Anfragezustand> {
        self.anfragen
            .iter()
            .map(|(n, a)| (self.kennung(*n), a.zustand))
            .collect()
    }

    /// The counter of request identifiers.
    #[must_use]
    pub fn zaehler(&self) -> u64 {
        self.zaehler
    }

    /// The load time.
    #[must_use]
    pub fn laden_ms(&self) -> i64 {
        self.laden_ms
    }

    /// The highest time seen.
    #[must_use]
    pub fn zeit_max(&self) -> i64 {
        self.zeit_max
    }

    /// The checked settings.
    #[must_use]
    pub fn einstellungen(&self) -> &Einstellungen {
        &self.einstellungen
    }

    /// The state with the store of contract 9 behind `person.pseudonym` (section 14.4).
    #[must_use]
    pub fn mit_personen(mut self, personen: Arc<dyn Personenspeicher>) -> Self {
        self.personen = Some(personen);
        self
    }

    /// The state with the rules of the admins for workers (section 14.5), or without any (`None`).
    #[must_use]
    pub fn mit_worker_regeln(mut self, regeln: Option<WorkerRegeln>) -> Self {
        self.worker = regeln;
        self
    }

    /// Every field of the state except the checker as one JSON value, for tests and diagnosis. It holds the
    /// previews of open requests; it never goes into a line.
    #[must_use]
    pub fn abbild(&self) -> Value {
        let sitzungen: serde_json::Map<String, Value> = self
            .sitzungen
            .iter()
            .map(|(k, s)| {
                (
                    k.clone(),
                    json!({"route": s.route, "kunde": s.kunde, "harness": s.harness, "orchestrator": s.orchestrator}),
                )
            })
            .collect();
        let anfragen: serde_json::Map<String, Value> = self
            .anfragen
            .iter()
            .map(|(n, a)| {
                let wert = json!({
                    "server": a.server, "werkzeug": a.werkzeug, "sitzung": a.sitzung, "person": a.person,
                    "digest": a.digest, "vorschau": a.vorschau, "vorschau_sha256": a.vorschau_sha256,
                    "zeit_anfrage": a.angefragt_ms, "entscheidend": a.entscheidend,
                    "zeit_entscheidung": a.entschieden_ms, "zustand": a.zustand.text(), "bis": a.bis,
                    "einloesefrist_ms": a.einloesefrist_ms,
                });
                (self.kennung(*n), wert)
            })
            .collect();
        let zusagen: serde_json::Map<String, Value> = self
            .zusagen
            .iter()
            .map(|(n, z)| {
                let wert = json!({
                    "orchestrator": z.orchestrator, "harness": z.harness, "modell": z.modell, "effort": z.effort,
                    "route": z.route, "kunde": z.kunde, "bis": z.bis,
                });
                (self.zusage_kennung(*n), wert)
            })
            .collect();
        json!({
            "einstellungen": format!("{:?}", self.einstellungen),
            "sitzungen": sitzungen,
            "anfragen": anfragen,
            "zusagen": zusagen,
            "zusage_zaehler": self.zusage_zaehler,
            "zaehler": self.zaehler,
            "laden_ms": self.laden_ms,
            "zeit_max": self.zeit_max,
        })
    }
}

impl fmt::Debug for Zustand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Zustand {}", self.abbild())
    }
}

/// One entry of the answer of `mcp.offene`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Offen {
    /// Request identifier.
    pub anfrage: String,
    /// Server key.
    pub server: String,
    /// Session of the request (section 14.3).
    pub sitzung: String,
    /// Tool name.
    pub werkzeug: String,
    /// The preview of the hub, or `None`.
    pub vorschau: Option<String>,
    /// sha256 of the preview, or `None`.
    pub vorschau_sha256: Option<String>,
    /// `digest` of the call.
    pub digest: String,
    /// End of the deadline for the decision.
    pub frist_bis_ms: i64,
}

/// The answer of one event (section 4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Antwort {
    /// `sitzung.starten`: the session is started.
    Gestartet {
        /// Session identifier.
        sitzung: String,
        /// Route key.
        route: String,
        /// Kind of the route.
        route_art: Routenart,
    },
    /// `mcp.anfragen`: the tool only reads.
    Lesend,
    /// `mcp.anfragen`: the option is off; the hub may run the call.
    OhneFreigabe {
        /// Request identifier.
        anfrage: String,
        /// `digest` of the call.
        digest: String,
    },
    /// `mcp.anfragen`: the request waits for a human.
    Offen {
        /// Request identifier.
        anfrage: String,
        /// `digest` of the call.
        digest: String,
        /// End of the deadline.
        frist_bis_ms: i64,
        /// sha256 of the preview, or `None`.
        vorschau_sha256: Option<String>,
    },
    /// `mcp.entscheiden`: released.
    Erteilt {
        /// Request identifier.
        anfrage: String,
        /// End of the redeem deadline.
        einloesen_bis_ms: i64,
    },
    /// `mcp.entscheiden`: refused.
    Abgelehnt {
        /// Request identifier.
        anfrage: String,
    },
    /// `mcp.einloesen`: the hub may run the call, exactly once.
    Eingeloest {
        /// Request identifier.
        anfrage: String,
        /// `digest` of the call.
        digest: String,
    },
    /// `mcp.offene`: the open requests, by number.
    Offene(Vec<Offen>),
    /// `mcp.stand`: the state of a request.
    Stand {
        /// Request identifier.
        anfrage: String,
        /// State after the deadline work of stage 1.
        zustand: Anfragezustand,
        /// End of the running deadline; `None` in the end states.
        frist_bis_ms: Option<i64>,
    },
    /// `ablauf.pruefen`: the number of requests that ran out in this event.
    Geprueft {
        /// Number of expired requests.
        abgelaufen: u64,
    },
    /// `person.pseudonym`: the pseudonym of the person (section 14.4).
    Pseudonym {
        /// The pseudonym `ps_<32 hex>`.
        pseudonym: String,
    },
    /// `worker.anfordern`: the worker may start (sections 14.5 and 15.3), bound by a single-use promise.
    Frei {
        /// The session of the orchestrator.
        sitzung: String,
        /// The promise `zs-<laden_ms>-<n>` the start of the worker must name.
        zusage: String,
        /// The end of the deadline of the promise.
        zusage_bis_ms: i64,
    },
    /// A refusal with its code.
    Nein(Code),
}

impl Antwort {
    /// True for every answer except a refusal.
    #[must_use]
    pub fn ok(&self) -> bool {
        !matches!(self, Self::Nein(_))
    }

    /// The answer as JSON value, as the shell sends it (in RFC 8785).
    #[must_use]
    pub fn json(&self) -> Value {
        match self {
            Self::Gestartet {
                sitzung,
                route,
                route_art,
            } => {
                json!({"ok": true, "ergebnis": "gestartet", "sitzung": sitzung, "route": route, "route_art": route_art.kennung()})
            }
            Self::Lesend => json!({"ok": true, "ergebnis": "lesend"}),
            Self::OhneFreigabe { anfrage, digest } => {
                json!({"ok": true, "ergebnis": "ohne_freigabe", "anfrage": anfrage, "digest": digest})
            }
            Self::Offen {
                anfrage,
                digest,
                frist_bis_ms,
                vorschau_sha256,
            } => {
                json!({"ok": true, "ergebnis": "offen", "anfrage": anfrage, "digest": digest, "frist_bis_ms": frist_bis_ms, "vorschau_sha256": vorschau_sha256})
            }
            Self::Erteilt {
                anfrage,
                einloesen_bis_ms,
            } => {
                json!({"ok": true, "ergebnis": "erteilt", "anfrage": anfrage, "einloesen_bis_ms": einloesen_bis_ms})
            }
            Self::Abgelehnt { anfrage } => {
                json!({"ok": true, "ergebnis": "abgelehnt", "anfrage": anfrage})
            }
            Self::Eingeloest { anfrage, digest } => {
                json!({"ok": true, "ergebnis": "eingeloest", "anfrage": anfrage, "digest": digest})
            }
            Self::Offene(liste) => {
                let offene: Vec<Value> = liste
                    .iter()
                    .map(|o| json!({"anfrage": o.anfrage, "server": o.server, "sitzung": o.sitzung, "werkzeug": o.werkzeug, "vorschau": o.vorschau, "vorschau_sha256": o.vorschau_sha256, "digest": o.digest, "frist_bis_ms": o.frist_bis_ms}))
                    .collect();
                json!({"ok": true, "ergebnis": "offene", "offene": offene})
            }
            Self::Stand {
                anfrage,
                zustand,
                frist_bis_ms,
            } => {
                json!({"ok": true, "ergebnis": "stand", "anfrage": anfrage, "zustand": zustand.text(), "frist_bis_ms": frist_bis_ms})
            }
            Self::Geprueft { abgelaufen } => {
                json!({"ok": true, "ergebnis": "geprueft", "abgelaufen": abgelaufen})
            }
            Self::Pseudonym { pseudonym } => {
                json!({"ok": true, "ergebnis": "pseudonym", "pseudonym": pseudonym})
            }
            Self::Frei {
                sitzung,
                zusage,
                zusage_bis_ms,
            } => json!({
                "ok": true, "ergebnis": "frei", "sitzung": sitzung, "zusage": zusage, "zusage_bis_ms": zusage_bis_ms,
            }),
            Self::Nein(code) => json!({"ok": false, "code": code.code(), "grund": code.grund()}),
        }
    }
}

/// The result of one event: the new state, the answer, the lines and the effective time (section 5).
#[derive(Debug, Clone)]
pub struct Ergebnis {
    /// The new state; the shell takes it only after the lines are written (E2).
    pub zustand: Zustand,
    /// The answer.
    pub antwort: Antwort,
    /// The lines of the event in their order (section 7.4): first the expiries, then the line of the operation.
    pub zeilen: Vec<Protokollzeile>,
    /// The effective time: the highest time seen, which is also `zeit` of every line. For a refusal of stage 0
    /// (no line, no change) it is the highest time of the unchanged state.
    pub zeit_ms: i64,
}

impl Ergebnis {
    /// The lines of the event as one sequence of texts after `vorher` (see [`verketten`]).
    ///
    /// # Errors
    /// `zeit_ungueltig` when a time lies outside of 1970 up to 9999.
    pub fn zeilen_texte(&self, vorher: &str) -> Result<Vec<String>, formate::kette::Fehler> {
        verketten(&self.zeilen, vorher, self.zeit_ms)
    }
}

/// Checks the settings file and builds a fresh state: no session, no request, the counter at 0 and the highest
/// time at `laden_ms`. It writes no line.
///
/// # Errors
/// `stelle.zeit_ungueltig` when `laden_ms` lies outside of 0 up to [`ZEIT_MAX`]; otherwise the `einst.*` code of
/// the first violated stage of the settings.
pub fn laden(
    einstellungen: &[u8],
    laden_ms: i64,
    pruefer: Box<dyn Nachweispruefer>,
) -> Result<Zustand, Code> {
    if !(0..=ZEIT_MAX).contains(&laden_ms) {
        return Err(Code::ZeitUngueltig);
    }
    Ok(Zustand {
        einstellungen: einstellungen::lesen(einstellungen)?,
        pruefer: Arc::from(pruefer),
        personen: None,
        worker: None,
        sitzungen: BTreeMap::new(),
        anfragen: BTreeMap::new(),
        zaehler: 0,
        zusagen: BTreeMap::new(),
        zusage_zaehler: 0,
        laden_ms,
        zeit_max: laden_ms,
    })
}
