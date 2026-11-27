use std::fmt;

/// A violated stage of contract 2. Each variant has one code and one fixed reason.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Fehler {
    /// Stage 1: more than 65536 bytes.
    ZuGross,
    /// Stage 2: not UTF-8, a byte order mark, a syntax error, a duplicate key, or text after the value.
    JsonUngueltig,
    /// Stage 3: a key outside the form, on any level.
    SchluesselUnbekannt,
    /// Stage 4: a route without `klasse`, or with `klasse` set to `null`.
    RouteOhneKlasse,
    /// Stage 5: a required key is missing.
    SchluesselFehlt,
    /// Stage 6: a value has the wrong JSON type.
    TypFalsch,
    /// Stage 7: `fassung` is an integer, but neither `1` nor `2`.
    FassungUnbekannt,
    /// Stage 8: a list is outside its bounds.
    Anzahl,
    /// Stage 9: a key does not have the form of a key.
    KennungForm,
    /// Stage 10: a key occurs twice where it must be unique.
    KennungDoppelt,
    /// Stage 11: `art` is neither `cloud` nor `lokal`.
    ArtUnbekannt,
    /// Stage 12: the class of a route is not in `klassen`.
    KlasseUnbekannt,
    /// Stage 13: `vorgabestufe` is not in `klassen`.
    VorgabestufeUnbekannt,
    /// Stage 14: a route named in `erlaubt`, `verboten` or a release is not in `routen`.
    RouteUnbekannt,
    /// Stage 15: a release names a customer that is not in `kunden`.
    KundeUnbekannt,
    /// Stage 16: `vorgabe` is a text other than `nie`.
    VorgabeWert,
    /// Stage 17: a release names a local route.
    FreigabeRouteLokal,
    /// Stage 18: a local route without `ziel` or `modell`, or a cloud route with `ziel` (version 2).
    ZielArt,
    /// Stage 19: the scheme of the target is not `http` or `https`.
    ZielSchema,
    /// Stage 20: the target violates the form.
    ZielForm,
    /// Stage 21: the port of the target is 0 or above 65535.
    ZielPort,
    /// Stage 22: the host rule of the target is violated.
    ZielHost,
    /// Stage 23: `modell` of a route violates length or characters.
    ModellForm,
    /// Stage 24: a local route has a network target.
    NetzzielLokal,
    /// Stage 25: the host of a network target violates the form.
    NetzzielForm,
    /// Stage 26: the port of a network target is outside 1 to 65535.
    NetzzielPort,
    /// Stage 27: `zugriff` is not a known access kind.
    ZugriffUnbekannt,
    /// Stage 28: `schluessel` or `pfade` missing at `kopf-einsetzen`, or present at another kind.
    ZugriffFelder,
    /// Stage 29: `methode` or `pfad` of a path entry violates the form.
    PfadForm,
    /// Stage 30: a route with a bound key has a target with `verbinden`.
    VerbindenMitSchluessel,
    /// Stage 31: a target with `schluessel: true` is in subdomain form.
    SchluesselUnterdomaene,
    /// Stage 32: version 2, a deadline of a server in the block `mcp` is outside 1 to 86400000.
    FristBereich,
    /// Stage 33: version 2, a tool name stands in `lesen` and in `schreiben` of one server.
    McpDoppelt,
    /// Stage 34: version 2, a value of `harness.copilot.werkzeuge_ohne_rueckfrage` is a text, but not
    /// `shell`, `write` or `such-mcp`.
    WerkzeugWert,
    /// Stage 37: version 2, `eingeschraenktes_token: true` at a route without a target with `schluessel: true`.
    TokenOhneSchluessel,
}

impl Fehler {
    /// The code of the contract, such as `einst.zu_gross`.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::ZuGross => "einst.zu_gross",
            Self::JsonUngueltig => "einst.json_ungueltig",
            Self::SchluesselUnbekannt => "einst.schluessel_unbekannt",
            Self::RouteOhneKlasse => "einst.route_ohne_klasse",
            Self::SchluesselFehlt => "einst.schluessel_fehlt",
            Self::TypFalsch => "einst.typ_falsch",
            Self::FassungUnbekannt => "einst.fassung_unbekannt",
            Self::Anzahl => "einst.anzahl",
            Self::KennungForm => "einst.kennung_form",
            Self::KennungDoppelt => "einst.kennung_doppelt",
            Self::ArtUnbekannt => "einst.art_unbekannt",
            Self::KlasseUnbekannt => "einst.klasse_unbekannt",
            Self::VorgabestufeUnbekannt => "einst.vorgabestufe_unbekannt",
            Self::RouteUnbekannt => "einst.route_unbekannt",
            Self::KundeUnbekannt => "einst.kunde_unbekannt",
            Self::VorgabeWert => "einst.vorgabe_wert",
            Self::FreigabeRouteLokal => "einst.freigabe_route_lokal",
            Self::ZielArt => "einst.ziel_art",
            Self::ZielSchema => "einst.ziel_schema",
            Self::ZielForm => "einst.ziel_form",
            Self::ZielPort => "einst.ziel_port",
            Self::ZielHost => "einst.ziel_host",
            Self::ModellForm => "einst.modell_form",
            Self::NetzzielLokal => "einst.netzziel_lokal",
            Self::NetzzielForm => "einst.netzziel_form",
            Self::NetzzielPort => "einst.netzziel_port",
            Self::ZugriffUnbekannt => "einst.zugriff_unbekannt",
            Self::ZugriffFelder => "einst.zugriff_felder",
            Self::PfadForm => "einst.pfad_form",
            Self::VerbindenMitSchluessel => "einst.verbinden_mit_schluessel",
            Self::SchluesselUnterdomaene => "einst.schluessel_unterdomaene",
            Self::FristBereich => "einst.frist_bereich",
            Self::McpDoppelt => "einst.mcp_doppelt",
            Self::WerkzeugWert => "einst.werkzeug_wert",
            Self::TokenOhneSchluessel => "einst.token_ohne_schluessel",
        }
    }

    /// The fixed reason of the contract, in ASCII, without path and without file content.
    #[must_use]
    pub fn grund(&self) -> &'static str {
        match self {
            Self::ZuGross => "Einstellungsdatei zu gross",
            Self::JsonUngueltig => "kein gueltiges JSON",
            Self::SchluesselUnbekannt => "unbekannter Schluessel",
            Self::RouteOhneKlasse => "Route ohne Klasse",
            Self::SchluesselFehlt => "Pflichtfeld fehlt",
            Self::TypFalsch => "falscher Typ",
            Self::FassungUnbekannt => "Fassung unbekannt",
            Self::Anzahl => "Anzahl ausserhalb der Grenzen",
            Self::KennungForm => "Kennung hat falsche Form",
            Self::KennungDoppelt => "Kennung doppelt",
            Self::ArtUnbekannt => "Routenart unbekannt",
            Self::KlasseUnbekannt => "Klasse unbekannt",
            Self::VorgabestufeUnbekannt => "Vorgabestufe unbekannt",
            Self::RouteUnbekannt => "Route unbekannt",
            Self::KundeUnbekannt => "Kunde unbekannt",
            Self::VorgabeWert => "Vorgabe nicht nie",
            Self::FreigabeRouteLokal => "Freigabe fuer lokale Route",
            Self::ZielArt => "Ziel oder Modell passt nicht zur Routenart",
            Self::ZielSchema => "Ziel mit falschem Schema",
            Self::ZielForm => "Ziel hat falsche Form",
            Self::ZielPort => "Port des Ziels ausserhalb 1 bis 65535",
            Self::ZielHost => "Ziel nicht lokal",
            Self::ModellForm => "Modell hat falsche Form",
            Self::NetzzielLokal => "Netzziel an lokaler Route",
            Self::NetzzielForm => "Netzziel hat falsche Form",
            Self::NetzzielPort => "Port des Netzziels ausserhalb 1 bis 65535",
            Self::ZugriffUnbekannt => "Zugriffsart unbekannt",
            Self::ZugriffFelder => "Felder passen nicht zur Zugriffsart",
            Self::PfadForm => "Pfad hat falsche Form",
            Self::VerbindenMitSchluessel => "Tunnel neben Schluessel",
            Self::SchluesselUnterdomaene => "Schluessel an Unterdomaene",
            Self::FristBereich => "Frist ausserhalb des Bereichs",
            Self::McpDoppelt => "Werkzeug in lesen und schreiben",
            Self::WerkzeugWert => "Werkzeugwert nicht erlaubt",
            Self::TokenOhneSchluessel => "eingeschraenktes Token ohne Ziel mit Schluessel",
        }
    }

    /// The stage number in the order of the contract; the checker reports the lowest one.
    pub(crate) fn stufe(&self) -> u8 {
        match self {
            Self::ZuGross => 1,
            Self::JsonUngueltig => 2,
            Self::SchluesselUnbekannt => 3,
            Self::RouteOhneKlasse => 4,
            Self::SchluesselFehlt => 5,
            Self::TypFalsch => 6,
            Self::FassungUnbekannt => 7,
            Self::Anzahl => 8,
            Self::KennungForm => 9,
            Self::KennungDoppelt => 10,
            Self::ArtUnbekannt => 11,
            Self::KlasseUnbekannt => 12,
            Self::VorgabestufeUnbekannt => 13,
            Self::RouteUnbekannt => 14,
            Self::KundeUnbekannt => 15,
            Self::VorgabeWert => 16,
            Self::FreigabeRouteLokal => 17,
            Self::ZielArt => 18,
            Self::ZielSchema => 19,
            Self::ZielForm => 20,
            Self::ZielPort => 21,
            Self::ZielHost => 22,
            Self::ModellForm => 23,
            Self::NetzzielLokal => 24,
            Self::NetzzielForm => 25,
            Self::NetzzielPort => 26,
            Self::ZugriffUnbekannt => 27,
            Self::ZugriffFelder => 28,
            Self::PfadForm => 29,
            Self::VerbindenMitSchluessel => 30,
            Self::SchluesselUnterdomaene => 31,
            Self::FristBereich => 32,
            Self::McpDoppelt => 33,
            Self::WerkzeugWert => 34,
            Self::TokenOhneSchluessel => 37,
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
            (
                Fehler::ZuGross,
                1,
                "einst.zu_gross",
                "Einstellungsdatei zu gross",
            ),
            (
                Fehler::JsonUngueltig,
                2,
                "einst.json_ungueltig",
                "kein gueltiges JSON",
            ),
            (
                Fehler::SchluesselUnbekannt,
                3,
                "einst.schluessel_unbekannt",
                "unbekannter Schluessel",
            ),
            (
                Fehler::RouteOhneKlasse,
                4,
                "einst.route_ohne_klasse",
                "Route ohne Klasse",
            ),
            (
                Fehler::SchluesselFehlt,
                5,
                "einst.schluessel_fehlt",
                "Pflichtfeld fehlt",
            ),
            (Fehler::TypFalsch, 6, "einst.typ_falsch", "falscher Typ"),
            (
                Fehler::FassungUnbekannt,
                7,
                "einst.fassung_unbekannt",
                "Fassung unbekannt",
            ),
            (
                Fehler::Anzahl,
                8,
                "einst.anzahl",
                "Anzahl ausserhalb der Grenzen",
            ),
            (
                Fehler::KennungForm,
                9,
                "einst.kennung_form",
                "Kennung hat falsche Form",
            ),
            (
                Fehler::KennungDoppelt,
                10,
                "einst.kennung_doppelt",
                "Kennung doppelt",
            ),
            (
                Fehler::ArtUnbekannt,
                11,
                "einst.art_unbekannt",
                "Routenart unbekannt",
            ),
            (
                Fehler::KlasseUnbekannt,
                12,
                "einst.klasse_unbekannt",
                "Klasse unbekannt",
            ),
            (
                Fehler::VorgabestufeUnbekannt,
                13,
                "einst.vorgabestufe_unbekannt",
                "Vorgabestufe unbekannt",
            ),
            (
                Fehler::RouteUnbekannt,
                14,
                "einst.route_unbekannt",
                "Route unbekannt",
            ),
            (
                Fehler::KundeUnbekannt,
                15,
                "einst.kunde_unbekannt",
                "Kunde unbekannt",
            ),
            (
                Fehler::VorgabeWert,
                16,
                "einst.vorgabe_wert",
                "Vorgabe nicht nie",
            ),
            (
                Fehler::FreigabeRouteLokal,
                17,
                "einst.freigabe_route_lokal",
                "Freigabe fuer lokale Route",
            ),
            (
                Fehler::ZielArt,
                18,
                "einst.ziel_art",
                "Ziel oder Modell passt nicht zur Routenart",
            ),
            (
                Fehler::ZielSchema,
                19,
                "einst.ziel_schema",
                "Ziel mit falschem Schema",
            ),
            (
                Fehler::ZielForm,
                20,
                "einst.ziel_form",
                "Ziel hat falsche Form",
            ),
            (
                Fehler::ZielPort,
                21,
                "einst.ziel_port",
                "Port des Ziels ausserhalb 1 bis 65535",
            ),
            (Fehler::ZielHost, 22, "einst.ziel_host", "Ziel nicht lokal"),
            (
                Fehler::ModellForm,
                23,
                "einst.modell_form",
                "Modell hat falsche Form",
            ),
            (
                Fehler::NetzzielLokal,
                24,
                "einst.netzziel_lokal",
                "Netzziel an lokaler Route",
            ),
            (
                Fehler::NetzzielForm,
                25,
                "einst.netzziel_form",
                "Netzziel hat falsche Form",
            ),
            (
                Fehler::NetzzielPort,
                26,
                "einst.netzziel_port",
                "Port des Netzziels ausserhalb 1 bis 65535",
            ),
            (
                Fehler::ZugriffUnbekannt,
                27,
                "einst.zugriff_unbekannt",
                "Zugriffsart unbekannt",
            ),
            (
                Fehler::ZugriffFelder,
                28,
                "einst.zugriff_felder",
                "Felder passen nicht zur Zugriffsart",
            ),
            (
                Fehler::PfadForm,
                29,
                "einst.pfad_form",
                "Pfad hat falsche Form",
            ),
            (
                Fehler::VerbindenMitSchluessel,
                30,
                "einst.verbinden_mit_schluessel",
                "Tunnel neben Schluessel",
            ),
            (
                Fehler::SchluesselUnterdomaene,
                31,
                "einst.schluessel_unterdomaene",
                "Schluessel an Unterdomaene",
            ),
            (
                Fehler::FristBereich,
                32,
                "einst.frist_bereich",
                "Frist ausserhalb des Bereichs",
            ),
            (
                Fehler::McpDoppelt,
                33,
                "einst.mcp_doppelt",
                "Werkzeug in lesen und schreiben",
            ),
            (
                Fehler::WerkzeugWert,
                34,
                "einst.werkzeug_wert",
                "Werkzeugwert nicht erlaubt",
            ),
            (
                Fehler::TokenOhneSchluessel,
                37,
                "einst.token_ohne_schluessel",
                "eingeschraenktes Token ohne Ziel mit Schluessel",
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
