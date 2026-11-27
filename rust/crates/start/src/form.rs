use einstellungen::Routenart;

use crate::Fehler;
use crate::lesen::ist_ordner;

/// The part of a start that both forms share.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Felder {
    pub(crate) fassung: u8,
    pub(crate) harness: String,
    pub(crate) modell: String,
    pub(crate) route: String,
    pub(crate) kunde: Option<String>,
    pub(crate) arbeitsordner: String,
    pub(crate) effort: Option<String>,
    pub(crate) name: Option<String>,
}

/// Read access to the parts of a start that both forms share. [`crate::pruefen`] takes either form.
pub trait Startform {
    /// The version of the start structure: 1 or 2.
    fn fassung(&self) -> u8;
    /// The effort of the session (version 2), or `None`: the harness takes its own default. Always `None`
    /// in version 1.
    fn effort(&self) -> Option<&str>;
    /// The display name of the session (version 2), or `None`: the view shows the session key. Always
    /// `None` in version 1.
    fn name(&self) -> Option<&str>;
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
    pub(crate) fortsetzung_von: Option<String>,
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
    /// yields the start file of a new start (`fortsetzung_von` is `None`; see [`Startanfrage::mit_fortsetzung`]).
    ///
    /// # Errors
    /// Returns `start.kennung_form` when `sitzung` does not have the form of a key.
    pub fn mit_sitzung(self, sitzung: &str, marker: Option<Marker>) -> Result<Startdatei, Fehler> {
        self.mit_fortsetzung(sitzung, marker, None)
    }

    /// Like [`Startanfrage::mit_sitzung`], and for `Fortsetzen` and `Wiederbeleben` with the key of the session whose
    /// conversation the new session continues (contract 1, field `fortsetzung_von`). `None` is a new start. In version 2
    /// the start file always carries the field, `null` for `None`; in version 1 the field does not exist.
    ///
    /// # Errors
    /// Returns `start.kennung_form` when `sitzung` or `fortsetzung_von` does not have the form of a key,
    /// `start.fortsetzung_selbst` when `fortsetzung_von` equals `sitzung`, and `start.schluessel_unbekannt` when the
    /// request has version 1 and `fortsetzung_von` is set, because a start file of version 1 has no such key.
    pub fn mit_fortsetzung(
        self,
        sitzung: &str,
        marker: Option<Marker>,
        fortsetzung_von: Option<&str>,
    ) -> Result<Startdatei, Fehler> {
        if !einstellungen::ist_kennung(sitzung) {
            return Err(Fehler::KennungForm);
        }
        if let Some(alt) = fortsetzung_von {
            if self.felder.fassung == 1 {
                return Err(Fehler::SchluesselUnbekannt);
            }
            if !einstellungen::ist_kennung(alt) {
                return Err(Fehler::KennungForm);
            }
            if alt == sitzung {
                return Err(Fehler::FortsetzungSelbst);
            }
        }
        Ok(Startdatei {
            sitzung: sitzung.to_owned(),
            felder: self.felder,
            marker,
            fortsetzung_von: fortsetzung_von.map(str::to_owned),
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

    /// The key of the session whose conversation this session continues (version 2), or `None` for a new start.
    /// Always `None` in version 1.
    #[must_use]
    pub fn fortsetzung_von(&self) -> Option<&str> {
        self.fortsetzung_von.as_deref()
    }
}

impl Startform for Startanfrage {
    fn fassung(&self) -> u8 {
        self.felder.fassung
    }
    fn effort(&self) -> Option<&str> {
        self.felder.effort.as_deref()
    }
    fn name(&self) -> Option<&str> {
        self.felder.name.as_deref()
    }
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
    fn fassung(&self) -> u8 {
        self.felder.fassung
    }
    fn effort(&self) -> Option<&str> {
        self.felder.effort.as_deref()
    }
    fn name(&self) -> Option<&str> {
        self.felder.name.as_deref()
    }
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

/// The table of harnesses (fassung 1 with the addenda of 20 November and 27 November), with the one route kind each
/// harness may use. `opencode` is a local harness for now (decision of the operator, 27.11.2026).
pub(crate) fn harness_art(name: &str) -> Option<Routenart> {
    match name {
        "claude" | "copilot" => Some(Routenart::Cloud),
        "pi" | "opencode" => Some(Routenart::Lokal),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::harness_art;
    use einstellungen::Routenart;

    #[test]
    fn each_harness_of_the_table_has_exactly_its_route_kind() {
        assert_eq!(harness_art("claude"), Some(Routenart::Cloud));
        assert_eq!(harness_art("copilot"), Some(Routenart::Cloud));
        assert_eq!(harness_art("pi"), Some(Routenart::Lokal));
        assert_eq!(harness_art("opencode"), Some(Routenart::Lokal));
        for fremd in [
            "",
            "Opencode",
            "opencode ",
            "open-code",
            "codex",
            "attrappe",
        ] {
            assert_eq!(harness_art(fremd), None, "{fremd:?}");
        }
    }
}
