//! The start file (contract 1, `rust/spec/VERTRAEGE.md`, stages 1 to 17).
//!
//! The crate `start` reads the request and the start file and checks them against the settings. This module
//! maps the result to the code of the point and builds the values that may stand in a line.

use ::start::{Startform, lesen_anfrage, lesen_datei};
use serde_json::{Map, Value};

use crate::einstellungen::Einstellungen;
use crate::form;
use crate::{Code, Routenart};

/// Largest start in bytes.
const GROESSE_MAX: usize = 4096;

/// The two forms of a start.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Form {
    /// The start request of the view: without `sitzung`.
    Anfrage,
    /// The start file of the core service: with `sitzung`.
    Datei,
}

/// The values of a start that may stand in a line: each one only when the start was readable and the value
/// holds the identifier form, otherwise `None`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Sicht {
    /// `harness`.
    pub harness: Option<String>,
    /// `kunde`.
    pub kunde: Option<String>,
    /// `route`.
    pub route: Option<String>,
    /// `sitzung`.
    pub sitzung: Option<String>,
}

impl Sicht {
    /// The values of a refused start. A refused start has no typed value, so they come from the bytes: none when
    /// the start is too large or not strict JSON, otherwise the values that hold the identifier form.
    fn aus_bytes(bytes: &[u8]) -> Self {
        if bytes.len() > GROESSE_MAX {
            return Self::default();
        }
        let Some(d) = form::objekt_lesen(bytes) else {
            return Self::default();
        };
        Self::aus(&d)
    }

    fn aus(d: &Map<String, Value>) -> Self {
        let sicher = |k: &str| {
            d.get(k)
                .and_then(Value::as_str)
                .filter(|t| form::kennung(t))
                .map(str::to_owned)
        };
        Self {
            harness: sicher("harness"),
            kunde: sicher("kunde"),
            route: sicher("route"),
            sitzung: sicher("sitzung"),
        }
    }
}

/// A start that holds every stage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Start {
    /// Identifier of the session (`None` in the form "Anfrage").
    pub sitzung: Option<String>,
    /// Harness.
    pub harness: String,
    /// Route key.
    pub route: String,
    /// Customer key or `None`.
    pub kunde: Option<String>,
    /// Kind of the route.
    pub route_art: Routenart,
}

/// A refused start: the code and the values that may stand in the line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Abgelehnt {
    /// The code and the reason of the crate for the first violated stage.
    pub code: Code,
    /// The readable values (boxed: a refusal is rare and the value is large).
    pub sicht: Box<Sicht>,
}

/// Checks a read start against the settings: stages 14 to 17 of contract 1.
fn gegen_einstellungen<S: Startform>(
    start: &S,
    sitzung: Option<&str>,
    einstellungen: &Einstellungen,
) -> Result<Start, Code> {
    let route_art = ::start::pruefen(start, &einstellungen.datei)?;
    Ok(Start {
        sitzung: sitzung.map(str::to_owned),
        harness: start.harness().to_owned(),
        route: start.route().to_owned(),
        kunde: start.kunde().map(str::to_owned),
        route_art,
    })
}

/// Checks a start against the settings: stages 1 to 17 of contract 1 (stages 3 and 4 only for the form "Anfrage").
///
/// # Errors
/// The code and the reason of the crate for the first violated stage, with the readable values ([`Abgelehnt`]).
/// A violation that this version of the point does not know is refused as well and keeps its code.
pub fn pruefen(bytes: &[u8], art: Form, einstellungen: &Einstellungen) -> Result<Start, Abgelehnt> {
    let ergebnis = match art {
        Form::Anfrage => lesen_anfrage(bytes)
            .map_err(Code::from)
            .and_then(|start| gegen_einstellungen(&start, None, einstellungen)),
        Form::Datei => lesen_datei(bytes)
            .map_err(Code::from)
            .and_then(|start| gegen_einstellungen(&start, Some(start.sitzung()), einstellungen)),
    };
    ergebnis.map_err(|code| Abgelehnt {
        code,
        sicht: Box::new(Sicht::aus_bytes(bytes)),
    })
}
