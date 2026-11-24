use einstellungen::Routenart;

use crate::Fehler;
use crate::lesen::ist_ordner;

/// The part of a start that both forms share.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Felder {
    pub(crate) harness: String,
    pub(crate) modell: String,
    pub(crate) route: String,
    pub(crate) kunde: Option<String>,
    pub(crate) arbeitsordner: String,
}

/// Read access to the parts of a start that both forms share. [`crate::pruefen`] takes either form.
pub trait Startform {
    /// The harness, such as `claude` or `pi`.
    fn harness(&self) -> &str;
    /// The model name the harness receives.
    fn modell(&self) -> &str;
    /// The key of the route.
    fn route(&self) -> &str;
    /// The key of the customer, or `None` for a session without a customer.
    fn kunde(&self) -> Option<&str>;
    /// The working folder: an absolute, normalized path.
    fn arbeitsordner(&self) -> &str;
}

/// A start as the view sends it to the service. It carries no session key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Startanfrage {
    pub(crate) felder: Felder,
}

/// A start as the service writes it for the program `Lauf starten`. It carries the session key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Startdatei {
    pub(crate) sitzung: String,
    pub(crate) felder: Felder,
    pub(crate) marker: Option<Marker>,
}

/// The count of markers in a trial run: 1 to 16 markers and the folder that holds the count. Only the
/// service sets it, and only when `werkbank-probelauf` started it with a marker list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Marker {
    pub(crate) liste: Vec<String>,
    pub(crate) ordner: String,
}

impl Marker {
    /// Builds the markers of a start file.
    ///
    /// # Errors
    /// Returns `start.marker_form` when the list or the folder violates the form.
    pub fn neu(liste: Vec<String>, ordner: &str) -> Result<Self, Fehler> {
        let namen: Vec<&str> = liste.iter().map(String::as_str).collect();
        if !ist_marker_liste(&namen) || !ist_ordner(ordner) {
            return Err(Fehler::MarkerForm);
        }
        Ok(Self {
            liste,
            ordner: ordner.to_owned(),
        })
    }

    /// The markers, in the order of the file.
    #[must_use]
    pub fn liste(&self) -> &[String] {
        &self.liste
    }

    /// The folder that holds the count: an absolute, normalized path.
    #[must_use]
    pub fn ordner(&self) -> &str {
        &self.ordner
    }
}

/// 1 to 16 markers of 1 to 32 characters of `A-Z0-9-`, without a duplicate.
pub(crate) fn ist_marker_liste(liste: &[&str]) -> bool {
    (1..=16).contains(&liste.len())
        && liste.iter().all(|marker| {
            (1..=32).contains(&marker.len())
                && marker
                    .bytes()
                    .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == b'-')
        })
        && liste
            .iter()
            .enumerate()
            .all(|(stelle, marker)| !liste[stelle + 1..].contains(marker))
}

impl Startanfrage {
    /// Adds the session key that the service assigns and, only in a trial run, the markers, and
    /// yields the start file.
    ///
    /// # Errors
    /// Returns `start.kennung_form` when `sitzung` does not have the form of a key.
    pub fn mit_sitzung(self, sitzung: &str, marker: Option<Marker>) -> Result<Startdatei, Fehler> {
        if !einstellungen::ist_kennung(sitzung) {
            return Err(Fehler::KennungForm);
        }
        Ok(Startdatei {
            sitzung: sitzung.to_owned(),
            felder: self.felder,
            marker,
        })
    }
}

impl Startdatei {
    /// The session key that the service assigned.
    #[must_use]
    pub fn sitzung(&self) -> &str {
        &self.sitzung
    }

    /// The markers of a trial run, if the service set them.
    #[must_use]
    pub fn marker(&self) -> Option<&Marker> {
        self.marker.as_ref()
    }
}

impl Startform for Startanfrage {
    fn harness(&self) -> &str {
        &self.felder.harness
    }
    fn modell(&self) -> &str {
        &self.felder.modell
    }
    fn route(&self) -> &str {
        &self.felder.route
    }
    fn kunde(&self) -> Option<&str> {
        self.felder.kunde.as_deref()
    }
    fn arbeitsordner(&self) -> &str {
        &self.felder.arbeitsordner
    }
}

impl Startform for Startdatei {
    fn harness(&self) -> &str {
        &self.felder.harness
    }
    fn modell(&self) -> &str {
        &self.felder.modell
    }
    fn route(&self) -> &str {
        &self.felder.route
    }
    fn kunde(&self) -> Option<&str> {
        self.felder.kunde.as_deref()
    }
    fn arbeitsordner(&self) -> &str {
        &self.felder.arbeitsordner
    }
}

/// The table of harnesses (fassung 1 with the addendum of 20 November), with the one route kind each harness may use.
pub(crate) fn harness_art(name: &str) -> Option<Routenart> {
    match name {
        "claude" | "copilot" => Some(Routenart::Cloud),
        "pi" => Some(Routenart::Lokal),
        _ => None,
    }
}
