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
    /// Stage 4a: the request carries `fortsetzung_von`, also with `null`; the service sets it.
    FortsetzungNichtErlaubt,
    /// Stage 5: a key outside the form, also inside `marker`.
    SchluesselUnbekannt,
    /// Stage 6: a required key is missing, also inside `marker`.
    SchluesselFehlt,
    /// Stage 7: a value has the wrong JSON type.
    TypFalsch,
    /// Stage 8: `fassung` is an integer, but neither `1` nor `2`.
    FassungUnbekannt,
    /// Stage 9: `sitzung`, `harness`, `route`, `kunde` or `fortsetzung_von` does not have the form of a key.
    KennungForm,
    /// Stage 9a: only in the start file, `fortsetzung_von` equals `sitzung`.
    FortsetzungSelbst,
    /// Stage 10: `modell` is empty, longer than 128 characters, or has a character outside its set.
    ModellForm,
    /// Stage 11: `arbeitsordner` breaks the path rules.
    OrdnerUngueltig,
    /// Stage 11a: `name` breaks the name rule.
    NameForm,
    /// Stage 11b: `effort` is a text, but not `^[a-z]{1,16}$`.
    EffortForm,
    /// Stage 12: `marker.liste` or `marker.ordner` violates the form.
    MarkerForm,
    /// Stage 13: `harness` is not in the table of harnesses.
    HarnessUnbekannt,
    /// Stage 13a: `effort` is no effort value of the harness (contract 2, block `effort`).
    EffortUnbekannt,
    /// Stage 14: `route` is not in the settings file.
    RouteUnbekannt,
    /// Stage 15: the route carries a model and `modell` of the start is not exactly equal to it.
    ModellRoute,
    /// Stage 16: `kunde` is not `null` and not in the settings file.
    KundeUnbekannt,
    /// Stage 17: the kind of the route does not suit the harness.
    HarnessRouteUnvertraeglich,
    /// Stage 18: `effort` is not among the allowed efforts for harness, route and model (contract 2, block `effort`).
    EffortGrenze,
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
            Self::FortsetzungNichtErlaubt => "start.fortsetzung_nicht_erlaubt",
            Self::SchluesselUnbekannt => "start.schluessel_unbekannt",
            Self::SchluesselFehlt => "start.schluessel_fehlt",
            Self::TypFalsch => "start.typ_falsch",
            Self::FassungUnbekannt => "start.fassung_unbekannt",
            Self::KennungForm => "start.kennung_form",
            Self::FortsetzungSelbst => "start.fortsetzung_selbst",
            Self::ModellForm => "start.modell_form",
            Self::OrdnerUngueltig => "start.ordner_ungueltig",
            Self::NameForm => "start.name_form",
            Self::EffortForm => "start.effort_form",
            Self::MarkerForm => "start.marker_form",
            Self::HarnessUnbekannt => "start.harness_unbekannt",
            Self::EffortUnbekannt => "start.effort_unbekannt",
            Self::RouteUnbekannt => "start.route_unbekannt",
            Self::ModellRoute => "start.modell_route",
            Self::KundeUnbekannt => "start.kunde_unbekannt",
            Self::HarnessRouteUnvertraeglich => "start.harness_route_unvertraeglich",
            Self::EffortGrenze => "start.effort_grenze",
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
            Self::FortsetzungNichtErlaubt => "Fortsetzung setzt der Dienst",
            Self::SchluesselUnbekannt => "unbekannter Schluessel",
            Self::SchluesselFehlt => "Pflichtfeld fehlt",
            Self::TypFalsch => "falscher Typ",
            Self::FassungUnbekannt => "Fassung unbekannt",
            Self::KennungForm => "Kennung hat falsche Form",
            Self::FortsetzungSelbst => "Fortsetzung der eigenen Sitzung",
            Self::ModellForm => "Modell hat falsche Form",
            Self::OrdnerUngueltig => "Arbeitsordner ungueltig",
            Self::NameForm => "Name hat falsche Form",
            Self::EffortForm => "Effort hat falsche Form",
            Self::MarkerForm => "Marker haben falsche Form",
            Self::HarnessUnbekannt => "Harness unbekannt",
            Self::EffortUnbekannt => "Effort fuer diesen Harness unbekannt",
            Self::RouteUnbekannt => "Route unbekannt",
            Self::ModellRoute => "Modell passt nicht zur Route",
            Self::KundeUnbekannt => "Kunde unbekannt",
            Self::HarnessRouteUnvertraeglich => "Harness passt nicht zur Route",
            Self::EffortGrenze => "Effort ausserhalb der Grenze",
        }
    }

    /// The rank of the stage in the order of the contract: ten times the stage number, plus 1 for a stage with the
    /// letter `a` and 2 for `b` (stage 4a is 41, stage 11b is 112). The reader reports the lowest rank.
    pub(crate) fn stufe(&self) -> u16 {
        match self {
            Self::ZuGross => 10,
            Self::JsonUngueltig => 20,
            Self::SitzungNichtErlaubt => 30,
            Self::MarkerNichtErlaubt => 40,
            Self::FortsetzungNichtErlaubt => 41,
            Self::SchluesselUnbekannt => 50,
            Self::SchluesselFehlt => 60,
            Self::TypFalsch => 70,
            Self::FassungUnbekannt => 80,
            Self::KennungForm => 90,
            Self::FortsetzungSelbst => 91,
            Self::ModellForm => 100,
            Self::OrdnerUngueltig => 110,
            Self::NameForm => 111,
            Self::EffortForm => 112,
            Self::MarkerForm => 120,
            Self::HarnessUnbekannt => 130,
            Self::EffortUnbekannt => 131,
            Self::RouteUnbekannt => 140,
            Self::ModellRoute => 150,
            Self::KundeUnbekannt => 160,
            Self::HarnessRouteUnvertraeglich => 170,
            Self::EffortGrenze => 180,
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
    #[allow(clippy::too_many_lines)]
    fn every_stage_has_its_code_and_reason() {
        let tabelle = [
            (Fehler::ZuGross, 10, "start.zu_gross", "Start zu gross"),
            (
                Fehler::JsonUngueltig,
                20,
                "start.json_ungueltig",
                "kein gueltiges JSON",
            ),
            (
                Fehler::SitzungNichtErlaubt,
                30,
                "start.sitzung_nicht_erlaubt",
                "Sitzung vergibt der Dienst",
            ),
            (
                Fehler::MarkerNichtErlaubt,
                40,
                "start.marker_nicht_erlaubt",
                "Marker setzt der Dienst",
            ),
            (
                Fehler::SchluesselUnbekannt,
                50,
                "start.schluessel_unbekannt",
                "unbekannter Schluessel",
            ),
            (
                Fehler::SchluesselFehlt,
                60,
                "start.schluessel_fehlt",
                "Pflichtfeld fehlt",
            ),
            (Fehler::TypFalsch, 70, "start.typ_falsch", "falscher Typ"),
            (
                Fehler::FassungUnbekannt,
                80,
                "start.fassung_unbekannt",
                "Fassung unbekannt",
            ),
            (
                Fehler::KennungForm,
                90,
                "start.kennung_form",
                "Kennung hat falsche Form",
            ),
            (
                Fehler::ModellForm,
                100,
                "start.modell_form",
                "Modell hat falsche Form",
            ),
            (
                Fehler::OrdnerUngueltig,
                110,
                "start.ordner_ungueltig",
                "Arbeitsordner ungueltig",
            ),
            (
                Fehler::MarkerForm,
                120,
                "start.marker_form",
                "Marker haben falsche Form",
            ),
            (
                Fehler::HarnessUnbekannt,
                130,
                "start.harness_unbekannt",
                "Harness unbekannt",
            ),
            (
                Fehler::RouteUnbekannt,
                140,
                "start.route_unbekannt",
                "Route unbekannt",
            ),
            (
                Fehler::ModellRoute,
                150,
                "start.modell_route",
                "Modell passt nicht zur Route",
            ),
            (
                Fehler::KundeUnbekannt,
                160,
                "start.kunde_unbekannt",
                "Kunde unbekannt",
            ),
            (
                Fehler::HarnessRouteUnvertraeglich,
                170,
                "start.harness_route_unvertraeglich",
                "Harness passt nicht zur Route",
            ),
            (
                Fehler::FortsetzungNichtErlaubt,
                41,
                "start.fortsetzung_nicht_erlaubt",
                "Fortsetzung setzt der Dienst",
            ),
            (
                Fehler::FortsetzungSelbst,
                91,
                "start.fortsetzung_selbst",
                "Fortsetzung der eigenen Sitzung",
            ),
            (
                Fehler::NameForm,
                111,
                "start.name_form",
                "Name hat falsche Form",
            ),
            (
                Fehler::EffortForm,
                112,
                "start.effort_form",
                "Effort hat falsche Form",
            ),
            (
                Fehler::EffortUnbekannt,
                131,
                "start.effort_unbekannt",
                "Effort fuer diesen Harness unbekannt",
            ),
            (
                Fehler::EffortGrenze,
                180,
                "start.effort_grenze",
                "Effort ausserhalb der Grenze",
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
