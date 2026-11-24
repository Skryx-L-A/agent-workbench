//! The settings file (contract 2, version 1 and 2) as `rangkern sitzung` reads it once at the start.
//!
//! The file is read by the crate `einstellungen` and by nothing else: this module has no reader of its own and no stage of
//! the contract. It only turns the typed view of a valid file into what `sitzung` needs, the [`Vorgabe`] of the gate
//! `datentor` and the keys of the routes. A violation keeps the code `einst.*` of the crate; the code is for the human on
//! the error output, the exit code is 78 for every one of them. The process keeps no second state of the settings (A44).

use std::fmt;
use std::fs;
use std::io::Read;
use std::path::Path;

use datentor::{Art, Freigabe, Kunde, Route, Vorgabe};
use einstellungen::Routenart;

/// The largest settings file in bytes; the crate refuses a file above it with `einst.zu_gross`.
const MAX_BYTES: usize = 65_536;

/// A violation of the contract: the code and the fixed reason (ASCII, with no path and no content of the file).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Fehler {
    /// The code, for example `einst.zu_gross`.
    pub code: &'static str,
    /// The fixed reason.
    pub grund: &'static str,
}

const UNLESBAR: Fehler = Fehler {
    code: "lauf.einst_unlesbar",
    grund: "Einstellungsdatei nicht lesbar",
};

/// A route kind that this process does not know: refused, never allowed (fail closed).
const ART_UNBEKANNT: Fehler = Fehler {
    code: "einst.art_unbekannt",
    grund: "Routenart unbekannt",
};

/// The settings are valid for the crate and not for the gate. The crate and the gate check the same rules, so this does not
/// happen with a file of the crate; it is refused all the same.
const TOR_ABGELEHNT: Fehler = Fehler {
    code: "einst.klasse_unbekannt",
    grund: "Klasse unbekannt",
};

impl From<einstellungen::Fehler> for Fehler {
    fn from(fehler: einstellungen::Fehler) -> Self {
        Self {
            code: fehler.code(),
            grund: fehler.grund(),
        }
    }
}

impl fmt::Display for Fehler {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code, self.grund)
    }
}

/// The settings as the process needs them: the gate's [`Vorgabe`] and the keys of the routes in the file.
#[derive(Debug)]
pub struct Einstellungen {
    /// The settings of the gate.
    pub vorgabe: Vorgabe,
    /// The keys of the routes; a route that is not here is no route of this process.
    pub routen: Vec<String>,
}

/// Reads and checks the settings file at `pfad`.
///
/// # Errors
///
/// The first violation of the contract; `lauf.einst_unlesbar` when the file is missing or cannot be read.
pub fn lesen_datei(pfad: &Path) -> Result<Einstellungen, Fehler> {
    let datei = fs::File::open(pfad).map_err(|_| UNLESBAR)?;
    let mut bytes = Vec::new();
    // One byte more than allowed: it tells a file that is too large from one that is exactly at the limit.
    datei
        .take(
            u64::try_from(MAX_BYTES)
                .unwrap_or(u64::MAX)
                .saturating_add(1),
        )
        .read_to_end(&mut bytes)
        .map_err(|_| UNLESBAR)?;
    lesen(&bytes)
}

/// Checks the bytes of a settings file with the crate `einstellungen` and builds what `sitzung` needs.
///
/// # Errors
///
/// The first violation, with the code of the crate.
pub fn lesen(bytes: &[u8]) -> Result<Einstellungen, Fehler> {
    let einst = einstellungen::lesen(bytes)?;
    let mut routen = Vec::new();
    for route in einst.routen() {
        let art = match route.art() {
            Routenart::Cloud => Art::Cloud,
            Routenart::Lokal => Art::Lokal,
            _ => return Err(ART_UNBEKANNT),
        };
        routen.push(Route {
            schluessel: route.schluessel().to_owned(),
            art,
            klasse: Some(route.klasse().to_owned()),
        });
    }
    let kunden = einst
        .kunden()
        .iter()
        .map(|kunde| Kunde {
            schluessel: kunde.schluessel().to_owned(),
            erlaubt: kunde.erlaubt().to_vec(),
            verboten: kunde.verboten().to_vec(),
        })
        .collect();
    let freigaben = einst
        .kundendaten_an_cloud()
        .freigaben()
        .iter()
        .map(|freigabe| Freigabe {
            route: freigabe.route().to_owned(),
            kunde: freigabe.kunde().to_owned(),
        })
        .collect();
    let schluessel = einst
        .routen()
        .iter()
        .map(|route| route.schluessel().to_owned())
        .collect();
    let vorgabe = Vorgabe::neu(
        einst.klassen().to_vec(),
        Some(einst.vorgabestufe().to_owned()),
        routen,
        kunden,
        freigaben,
    )
    .map_err(|_| TOR_ABGELEHNT)?;
    Ok(Einstellungen {
        vorgabe,
        routen: schluessel,
    })
}
