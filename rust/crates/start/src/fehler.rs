use std::fmt;

/// A violated stage of contract 1. Each variant has one code and one fixed reason.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Fehler {
    /// Stage 1: more than 4096 bytes.
    ZuGross,
    /// Stage 2: not UTF-8, a byte order mark, a syntax error, a duplicate key, or text after the value.
    JsonUngueltig,
    /// Stage 3: the request carries `sitzung`; the service assigns it.
    SitzungNichtErlaubt,
    /// Stage 4: the request carries `marker`; the service sets it.
    MarkerNichtErlaubt,
    /// Stage 5: a key outside the form, also inside `marker`.
    SchluesselUnbekannt,
    /// Stage 6: a required key is missing, also inside `marker`.
    SchluesselFehlt,
    /// Stage 7: a value has the wrong JSON type.
    TypFalsch,
    /// Stage 8: `fassung` is an integer, but not `1`.
    FassungUnbekannt,
    /// Stage 9: `sitzung`, `harness`, `route` or `kunde` does not have the form of a key.
    KennungForm,
    /// Stage 10: `modell` is empty, longer than 128 characters, or has a character outside its set.
    ModellForm,
    /// Stage 11: `arbeitsordner` breaks the path rules.
    OrdnerUngueltig,
    /// Stage 12: `marker.liste` or `marker.ordner` violates the form.
    MarkerForm,
    /// Stage 13: `harness` is not in the table of harnesses.
    HarnessUnbekannt,
    /// Stage 14: `route` is not in the settings file.
    RouteUnbekannt,
    /// Stage 15: the route carries a model and `modell` of the start is not exactly equal to it.
    ModellRoute,
    /// Stage 16: `kunde` is not `null` and not in the settings file.
    KundeUnbekannt,
    /// Stage 17: the kind of the route does not suit the harness.
    HarnessRouteUnvertraeglich,
}

impl Fehler {
    /// The code of the contract, such as `start.zu_gross`.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::ZuGross => "start.zu_gross",
            Self::JsonUngueltig => "start.json_ungueltig",
            Self::SitzungNichtErlaubt => "start.sitzung_nicht_erlaubt",
            Self::MarkerNichtErlaubt => "start.marker_nicht_erlaubt",
            Self::SchluesselUnbekannt => "start.schluessel_unbekannt",
            Self::SchluesselFehlt => "start.schluessel_fehlt",
            Self::TypFalsch => "start.typ_falsch",
            Self::FassungUnbekannt => "start.fassung_unbekannt",
            Self::KennungForm => "start.kennung_form",
            Self::ModellForm => "start.modell_form",
            Self::OrdnerUngueltig => "start.ordner_ungueltig",
            Self::MarkerForm => "start.marker_form",
            Self::HarnessUnbekannt => "start.harness_unbekannt",
            Self::RouteUnbekannt => "start.route_unbekannt",
            Self::ModellRoute => "start.modell_route",
            Self::KundeUnbekannt => "start.kunde_unbekannt",
            Self::HarnessRouteUnvertraeglich => "start.harness_route_unvertraeglich",
        }
    }

    /// The fixed reason of the contract, in ASCII.
    #[must_use]
    pub fn grund(&self) -> &'static str {
        match self {
            Self::ZuGross => "Start zu gross",
            Self::JsonUngueltig => "kein gueltiges JSON",
            Self::SitzungNichtErlaubt => "Sitzung vergibt der Dienst",
            Self::MarkerNichtErlaubt => "Marker setzt der Dienst",
            Self::SchluesselUnbekannt => "unbekannter Schluessel",
            Self::SchluesselFehlt => "Pflichtfeld fehlt",
            Self::TypFalsch => "falscher Typ",
            Self::FassungUnbekannt => "Fassung unbekannt",
            Self::KennungForm => "Kennung hat falsche Form",
            Self::ModellForm => "Modell hat falsche Form",
            Self::OrdnerUngueltig => "Arbeitsordner ungueltig",
            Self::MarkerForm => "Marker haben falsche Form",
            Self::HarnessUnbekannt => "Harness unbekannt",
            Self::RouteUnbekannt => "Route unbekannt",
            Self::ModellRoute => "Modell passt nicht zur Route",
            Self::KundeUnbekannt => "Kunde unbekannt",
            Self::HarnessRouteUnvertraeglich => "Harness passt nicht zur Route",
        }
    }

    /// The stage number in the order of the contract; the reader reports the lowest one.
    pub(crate) fn stufe(&self) -> u8 {
        match self {
            Self::ZuGross => 1,
            Self::JsonUngueltig => 2,
            Self::SitzungNichtErlaubt => 3,
            Self::MarkerNichtErlaubt => 4,
            Self::SchluesselUnbekannt => 5,
            Self::SchluesselFehlt => 6,
            Self::TypFalsch => 7,
            Self::FassungUnbekannt => 8,
            Self::KennungForm => 9,
            Self::ModellForm => 10,
            Self::OrdnerUngueltig => 11,
            Self::MarkerForm => 12,
            Self::HarnessUnbekannt => 13,
            Self::RouteUnbekannt => 14,
            Self::ModellRoute => 15,
            Self::KundeUnbekannt => 16,
            Self::HarnessRouteUnvertraeglich => 17,
        }
    }
}

impl fmt::Display for Fehler {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code(), self.grund())
    }
}

impl std::error::Error for Fehler {}

#[cfg(test)]
mod tests {
    use super::Fehler;

    #[test]
    fn every_stage_has_its_code_and_reason() {
        let tabelle = [
            (Fehler::ZuGross, 1, "start.zu_gross", "Start zu gross"),
            (
                Fehler::JsonUngueltig,
                2,
                "start.json_ungueltig",
                "kein gueltiges JSON",
            ),
            (
                Fehler::SitzungNichtErlaubt,
                3,
                "start.sitzung_nicht_erlaubt",
                "Sitzung vergibt der Dienst",
            ),
            (
                Fehler::MarkerNichtErlaubt,
                4,
                "start.marker_nicht_erlaubt",
                "Marker setzt der Dienst",
            ),
            (
                Fehler::SchluesselUnbekannt,
                5,
                "start.schluessel_unbekannt",
                "unbekannter Schluessel",
            ),
            (
                Fehler::SchluesselFehlt,
                6,
                "start.schluessel_fehlt",
                "Pflichtfeld fehlt",
            ),
            (Fehler::TypFalsch, 7, "start.typ_falsch", "falscher Typ"),
            (
                Fehler::FassungUnbekannt,
                8,
                "start.fassung_unbekannt",
                "Fassung unbekannt",
            ),
            (
                Fehler::KennungForm,
                9,
                "start.kennung_form",
                "Kennung hat falsche Form",
            ),
            (
                Fehler::ModellForm,
                10,
                "start.modell_form",
                "Modell hat falsche Form",
            ),
            (
                Fehler::OrdnerUngueltig,
                11,
                "start.ordner_ungueltig",
                "Arbeitsordner ungueltig",
            ),
            (
                Fehler::MarkerForm,
                12,
                "start.marker_form",
                "Marker haben falsche Form",
            ),
            (
                Fehler::HarnessUnbekannt,
                13,
                "start.harness_unbekannt",
                "Harness unbekannt",
            ),
            (
                Fehler::RouteUnbekannt,
                14,
                "start.route_unbekannt",
                "Route unbekannt",
            ),
            (
                Fehler::ModellRoute,
                15,
                "start.modell_route",
                "Modell passt nicht zur Route",
            ),
            (
                Fehler::KundeUnbekannt,
                16,
                "start.kunde_unbekannt",
                "Kunde unbekannt",
            ),
            (
                Fehler::HarnessRouteUnvertraeglich,
                17,
                "start.harness_route_unvertraeglich",
                "Harness passt nicht zur Route",
            ),
        ];
        for (fehler, stufe, code, grund) in tabelle {
            assert_eq!(fehler.stufe(), stufe, "{code}");
            assert_eq!(fehler.code(), code);
            assert_eq!(fehler.grund(), grund);
            assert_eq!(fehler.to_string(), format!("{code}: {grund}"));
        }
    }
}
