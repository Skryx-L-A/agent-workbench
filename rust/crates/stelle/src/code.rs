//! Every code of the point with its fixed reason (contract 6, section 8; contract 1 and contract 2 for `start.*`
//! and `einst.*`).

/// A fixed code with a fixed ASCII reason.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[allow(
    missing_docs,
    reason = "each variant is the code of the same name in the table of `Code::code`"
)]
pub enum Code {
    ZeitUngueltig,
    AufruferNichtErlaubt,
    EingabeUngueltig,
    SitzungVergeben,
    ServerUnbekannt,
    SitzungUnbekannt,
    FristUngueltig,
    AnfragenErschoepft,
    AnfrageUnbekannt,
    AnfrageAbgelaufen,
    BereitsEntschieden,
    NichtFreigegeben,
    AnfrageAbgelehnt,
    BereitsEingeloest,
    DigestAbweichend,
    NachweisUngueltig,
    ProtokollFehler,
    /// A code and a reason of contract 1 or 2, as the crate `start` or `einstellungen` reports them.
    Vertrag(&'static str, &'static str),
}

impl Code {
    /// `einst.frist_bereich` of contract 2: a deadline of a server outside of its range. The point refuses with
    /// it a deadline of the crate that does not fit in `i64`.
    pub(crate) const FRIST_BEREICH: Self =
        Self::Vertrag("einst.frist_bereich", "Frist ausserhalb des Bereichs");

    /// The code and its reason, as the tables of the contracts list them.
    #[must_use]
    #[allow(clippy::too_many_lines, reason = "one arm per code of the tables")]
    pub fn paar(self) -> (&'static str, &'static str) {
        match self {
            Self::ZeitUngueltig => ("stelle.zeit_ungueltig", "Zeit ungueltig"),
            Self::AufruferNichtErlaubt => {
                ("stelle.aufrufer_nicht_erlaubt", "Aufrufer nicht erlaubt")
            }
            Self::EingabeUngueltig => ("stelle.eingabe_ungueltig", "Eingabe ungueltig"),
            Self::SitzungVergeben => ("stelle.sitzung_vergeben", "Sitzung schon vergeben"),
            Self::ServerUnbekannt => ("stelle.server_unbekannt", "Server unbekannt"),
            Self::SitzungUnbekannt => ("stelle.sitzung_unbekannt", "Sitzung unbekannt"),
            Self::FristUngueltig => ("stelle.frist_ungueltig", "Frist ausserhalb der Vorgabe"),
            Self::AnfragenErschoepft => (
                "stelle.anfragen_erschoepft",
                "Kennungen erschoepft, Neustart noetig",
            ),
            Self::AnfrageUnbekannt => ("stelle.anfrage_unbekannt", "Anfrage unbekannt"),
            Self::AnfrageAbgelaufen => ("stelle.anfrage_abgelaufen", "Anfrage abgelaufen"),
            Self::BereitsEntschieden => ("stelle.bereits_entschieden", "Anfrage schon entschieden"),
            Self::NichtFreigegeben => ("stelle.nicht_freigegeben", "Anfrage nicht freigegeben"),
            Self::AnfrageAbgelehnt => ("stelle.anfrage_abgelehnt", "Anfrage abgelehnt"),
            Self::BereitsEingeloest => ("stelle.bereits_eingeloest", "Freigabe schon eingeloest"),
            Self::DigestAbweichend => (
                "stelle.digest_abweichend",
                "Aufruf weicht von der Anfrage ab",
            ),
            Self::NachweisUngueltig => ("stelle.nachweis_ungueltig", "Nachweis ungueltig"),
            Self::ProtokollFehler => ("stelle.protokoll_fehler", "Protokoll nicht schreibbar"),
            Self::Vertrag(code, grund) => (code, grund),
        }
    }

    /// The fixed code, such as `stelle.anfrage_unbekannt`.
    #[must_use]
    pub fn code(self) -> &'static str {
        self.paar().0
    }

    /// The fixed ASCII reason, such as `Anfrage unbekannt`.
    #[must_use]
    pub fn grund(self) -> &'static str {
        self.paar().1
    }
}

/// The code and the reason of a violated stage of contract 2, as the crate reports them. Nothing maps the
/// violation to another code or to success: a violation that this version does not know is refused as well.
impl From<einstellungen::Fehler> for Code {
    fn from(fehler: einstellungen::Fehler) -> Self {
        Self::Vertrag(fehler.code(), fehler.grund())
    }
}

/// The code and the reason of a violated stage of contract 1, as the crate reports them.
impl From<start::Fehler> for Code {
    fn from(fehler: start::Fehler) -> Self {
        Self::Vertrag(fehler.code(), fehler.grund())
    }
}

#[cfg(test)]
mod tests {
    use super::Code;

    #[test]
    fn a_violation_of_the_crates_keeps_its_code_and_reason() {
        let c = Code::from(einstellungen::Fehler::ZuGross);
        assert_eq!(
            (c.code(), c.grund()),
            ("einst.zu_gross", "Einstellungsdatei zu gross")
        );
        let c = Code::from(einstellungen::Fehler::WerkzeugWert);
        assert_eq!(
            (c.code(), c.grund()),
            ("einst.werkzeug_wert", "Werkzeugwert nicht erlaubt")
        );
        let c = Code::from(start::Fehler::ZuGross);
        assert_eq!((c.code(), c.grund()), ("start.zu_gross", "Start zu gross"));
        let c = Code::from(start::Fehler::HarnessRouteUnvertraeglich);
        assert_eq!(
            (c.code(), c.grund()),
            (
                "start.harness_route_unvertraeglich",
                "Harness passt nicht zur Route"
            )
        );
    }

    /// The tables of contracts 1, 2 and 6, copied by hand: every code with its reason.
    #[test]
    #[allow(clippy::too_many_lines, reason = "one row per code of the tables")]
    fn jeder_code_mit_seinem_grund() {
        let tabelle = [
            (
                Code::ZeitUngueltig,
                "stelle.zeit_ungueltig",
                "Zeit ungueltig",
            ),
            (
                Code::AufruferNichtErlaubt,
                "stelle.aufrufer_nicht_erlaubt",
                "Aufrufer nicht erlaubt",
            ),
            (
                Code::EingabeUngueltig,
                "stelle.eingabe_ungueltig",
                "Eingabe ungueltig",
            ),
            (
                Code::SitzungVergeben,
                "stelle.sitzung_vergeben",
                "Sitzung schon vergeben",
            ),
            (
                Code::ServerUnbekannt,
                "stelle.server_unbekannt",
                "Server unbekannt",
            ),
            (
                Code::SitzungUnbekannt,
                "stelle.sitzung_unbekannt",
                "Sitzung unbekannt",
            ),
            (
                Code::FristUngueltig,
                "stelle.frist_ungueltig",
                "Frist ausserhalb der Vorgabe",
            ),
            (
                Code::AnfragenErschoepft,
                "stelle.anfragen_erschoepft",
                "Kennungen erschoepft, Neustart noetig",
            ),
            (
                Code::AnfrageUnbekannt,
                "stelle.anfrage_unbekannt",
                "Anfrage unbekannt",
            ),
            (
                Code::AnfrageAbgelaufen,
                "stelle.anfrage_abgelaufen",
                "Anfrage abgelaufen",
            ),
            (
                Code::BereitsEntschieden,
                "stelle.bereits_entschieden",
                "Anfrage schon entschieden",
            ),
            (
                Code::NichtFreigegeben,
                "stelle.nicht_freigegeben",
                "Anfrage nicht freigegeben",
            ),
            (
                Code::AnfrageAbgelehnt,
                "stelle.anfrage_abgelehnt",
                "Anfrage abgelehnt",
            ),
            (
                Code::BereitsEingeloest,
                "stelle.bereits_eingeloest",
                "Freigabe schon eingeloest",
            ),
            (
                Code::NachweisUngueltig,
                "stelle.nachweis_ungueltig",
                "Nachweis ungueltig",
            ),
            (
                Code::ProtokollFehler,
                "stelle.protokoll_fehler",
                "Protokoll nicht schreibbar",
            ),
            (
                Code::DigestAbweichend,
                "stelle.digest_abweichend",
                "Aufruf weicht von der Anfrage ab",
            ),
        ];
        for (code, text, grund) in tabelle {
            assert_eq!((code.code(), code.grund()), (text, grund));
            assert!(grund.is_ascii());
        }
    }
}
