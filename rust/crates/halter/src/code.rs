//! The codes of the holder (contract section 7) with their fixed ASCII reasons, and the exit codes of the start
//! (section 2.1, table "Start und Exitcodes").

/// One code of a refused or failed request. The order of the variants is the order of the stages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Code {
    /// Stage 1.
    ZuGross,
    /// Stage 2.
    Peer,
    /// Stage 3.
    JsonUngueltig,
    /// Stage 4.
    NichtKanonisch,
    /// Stage 5.
    FassungUnbekannt,
    /// Stage 6.
    OpUnbekannt,
    /// Stage 7.
    Rolle,
    /// Stage 7a.
    Strom,
    /// Stage 8.
    SchluesselUnbekannt,
    /// Stage 9.
    SchluesselFehlt,
    /// Stage 10.
    TypFalsch,
    /// Stage 11.
    WertForm,
    /// Stage 12.
    KopfVerboten,
    /// Stage 13.
    Bereich,
    /// Stage 14.
    ZielUnbekannt,
    /// Stage 15.
    PfadVerboten,
    /// Stage 16.
    KeinSchluessel,
    /// Stage 17.
    Person,
    /// Stage 18, no ticket where one is needed.
    ScheinFehlt,
    /// Stage 18, a ticket where none is needed.
    ScheinUnerwartet,
    /// Stage 19.
    ScheinKid,
    /// Stage 20.
    ScheinSignatur,
    /// Stage 21.
    ScheinArt,
    /// Stage 22.
    ScheinLauf,
    /// Stage 23.
    ScheinDigest,
    /// Stage 24.
    ScheinZeit,
    /// Stage 25.
    ScheinAbgelaufen,
    /// Stage 26.
    ScheinBenutzt,
    /// Stage 27.
    Nachweis,
    /// Stage 28.
    ZielFehler,
    /// Stage 28a.
    ZielKomprimiert,
    /// Stage 29.
    McpFehler,
    /// Stage 30.
    LeckGesperrt,
    /// Stage 31.
    Intern,
}

impl Code {
    /// The code and its fixed reason.
    #[must_use]
    pub fn paar(self) -> (&'static str, &'static str) {
        match self {
            Self::ZuGross => ("halter.zu_gross", "Zeile zu gross"),
            Self::Peer => ("halter.peer", "Aufrufer nicht zugelassen"),
            Self::JsonUngueltig => ("halter.json_ungueltig", "kein gueltiges JSON"),
            Self::NichtKanonisch => ("halter.nicht_kanonisch", "nicht kanonisch"),
            Self::FassungUnbekannt => ("halter.fassung_unbekannt", "Fassung unbekannt"),
            Self::OpUnbekannt => ("halter.op_unbekannt", "Op unbekannt"),
            Self::Rolle => ("halter.rolle", "Op fuer Rolle nicht erlaubt"),
            Self::Strom => ("halter.strom", "kein passender Strom"),
            Self::SchluesselUnbekannt => ("halter.schluessel_unbekannt", "unbekannter Schluessel"),
            Self::SchluesselFehlt => ("halter.schluessel_fehlt", "Pflichtfeld fehlt"),
            Self::TypFalsch => ("halter.typ_falsch", "falscher Typ"),
            Self::WertForm => ("halter.wert_form", "Wert hat falsche Form"),
            Self::KopfVerboten => ("halter.kopf_verboten", "Kopf nicht erlaubt"),
            Self::Bereich => ("halter.bereich", "Bereich nicht erlaubt"),
            Self::ZielUnbekannt => ("halter.ziel_unbekannt", "Ziel unbekannt"),
            Self::PfadVerboten => ("halter.pfad_verboten", "Pfad nicht erlaubt"),
            Self::KeinSchluessel => ("halter.kein_schluessel", "kein Schluessel hinterlegt"),
            Self::Person => ("halter.person", "Person passt nicht"),
            Self::ScheinFehlt => ("halter.schein_fehlt", "Schein fehlt"),
            Self::ScheinUnerwartet => ("halter.schein_unerwartet", "Schein nicht erwartet"),
            Self::ScheinKid => ("halter.schein_kid", "Schluesselkennung unbekannt"),
            Self::ScheinSignatur => ("halter.schein_signatur", "Signatur ungueltig"),
            Self::ScheinArt => ("halter.schein_art", "falsche Art des Scheins"),
            Self::ScheinLauf => ("halter.schein_lauf", "Schein fuer anderen Lauf"),
            Self::ScheinDigest => ("halter.schein_digest", "Schein passt nicht zum Aufruf"),
            Self::ScheinZeit => ("halter.schein_zeit", "Zeiten des Scheins ungueltig"),
            Self::ScheinAbgelaufen => ("halter.schein_abgelaufen", "Schein abgelaufen"),
            Self::ScheinBenutzt => ("halter.schein_benutzt", "Schein schon benutzt"),
            Self::Nachweis => ("halter.nachweis", "Nachweis ungueltig"),
            Self::ZielFehler => ("halter.ziel_fehler", "Ziel nicht erreichbar"),
            Self::ZielKomprimiert => ("halter.ziel_komprimiert", "Antwort komprimiert"),
            Self::McpFehler => ("halter.mcp_fehler", "MCP-Server meldet Fehler"),
            Self::LeckGesperrt => ("halter.leck_gesperrt", "Antwort enthielt einen Schluessel"),
            Self::Intern => ("halter.intern", "interner Fehler"),
        }
    }

    /// The code, `halter.<name>`.
    #[must_use]
    pub fn code(self) -> &'static str {
        self.paar().0
    }

    /// The fixed reason.
    #[must_use]
    pub fn grund(self) -> &'static str {
        self.paar().1
    }

    /// The reason of the protocol line (section 9): of a refused ticket, or `leck` for the leak lock; `None` where the
    /// fixed list has none (stage 18, OPEN 2).
    #[must_use]
    pub fn kettengrund(self) -> Option<&'static str> {
        match self {
            Self::ScheinKid | Self::ScheinSignatur | Self::ScheinArt => Some("signatur"),
            Self::ScheinLauf => Some("peer"),
            Self::ScheinDigest => Some("digest"),
            Self::ScheinZeit | Self::ScheinAbgelaufen => Some("abgelaufen"),
            Self::ScheinBenutzt => Some("benutzt"),
            Self::LeckGesperrt => Some("leck"),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Code, Startfehler};

    /// The contract with the tables of section 2.1 ("Start und Exitcodes") and section 7.
    const VERTRAG: &str = include_str!("../../../spec/halter/README.md");

    const ALLE: [Code; 34] = [
        Code::ZuGross,
        Code::Peer,
        Code::JsonUngueltig,
        Code::NichtKanonisch,
        Code::FassungUnbekannt,
        Code::OpUnbekannt,
        Code::Rolle,
        Code::Strom,
        Code::SchluesselUnbekannt,
        Code::SchluesselFehlt,
        Code::TypFalsch,
        Code::WertForm,
        Code::KopfVerboten,
        Code::Bereich,
        Code::ZielUnbekannt,
        Code::PfadVerboten,
        Code::KeinSchluessel,
        Code::Person,
        Code::ScheinFehlt,
        Code::ScheinUnerwartet,
        Code::ScheinKid,
        Code::ScheinSignatur,
        Code::ScheinArt,
        Code::ScheinLauf,
        Code::ScheinDigest,
        Code::ScheinZeit,
        Code::ScheinAbgelaufen,
        Code::ScheinBenutzt,
        Code::Nachweis,
        Code::ZielFehler,
        Code::ZielKomprimiert,
        Code::McpFehler,
        Code::LeckGesperrt,
        Code::Intern,
    ];

    /// Code and reason of every code stand together in one row of the table of section 7.
    #[test]
    fn code_und_grund_wie_im_vertrag() {
        for c in ALLE {
            let (code, grund) = (c.code(), c.grund());
            assert_eq!(c.paar(), (code, grund));
            assert!(code.starts_with("halter."), "{c:?}");
            assert!(
                VERTRAG.lines().any(|z| z.starts_with("| ")
                    && z.contains(&format!("`{code}`"))
                    && z.contains(&format!("`{grund}`"))),
                "{c:?}: {code} with {grund} is no row of the contract"
            );
        }
        let codes: std::collections::BTreeSet<&str> = ALLE.iter().map(|c| c.code()).collect();
        assert_eq!(codes.len(), ALLE.len(), "two variants with one code");
    }

    /// Exit code, code and reason of every failed start step as in the table of section 2.1, and the line of the
    /// journal `<code>: <grund>`.
    #[test]
    fn startfehler_wie_im_vertrag() {
        for (f, exit) in [
            (Startfehler::Kernabbild, 5),
            (Startfehler::Aufruf, 64),
            (Startfehler::Konfig, 78),
            (Startfehler::AblageRechte, 78),
            (Startfehler::PipeBelegt, 74),
            (Startfehler::Intern, 70),
        ] {
            let (e, code, grund) = f.teile();
            assert_eq!(e, exit, "{f:?}");
            assert!(
                VERTRAG.contains(&format!("| {exit}, `{code}`, `{grund}` |")),
                "{f:?}: {exit}, {code}, {grund} is no row of the contract"
            );
            assert_eq!(f.zeile(), format!("{code}: {grund}"));
        }
        assert_eq!(
            Startfehler::Aufruf.zeile(),
            "halter.aufruf: falscher Aufruf"
        );
    }

    #[test]
    fn gruende_der_kette() {
        let soll = [
            (Code::ScheinKid, Some("signatur")),
            (Code::ScheinSignatur, Some("signatur")),
            (Code::ScheinArt, Some("signatur")),
            (Code::ScheinLauf, Some("peer")),
            (Code::ScheinDigest, Some("digest")),
            (Code::ScheinZeit, Some("abgelaufen")),
            (Code::ScheinAbgelaufen, Some("abgelaufen")),
            (Code::ScheinBenutzt, Some("benutzt")),
            (Code::ScheinFehlt, None),
            (Code::ScheinUnerwartet, None),
            (Code::LeckGesperrt, Some("leck")),
            (Code::ZielFehler, None),
        ];
        for (c, g) in soll {
            assert_eq!(c.kettengrund(), g, "{c:?}");
        }
    }
}

/// One failed step of the start; the first one ends the holder (section 2.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Startfehler {
    /// Step 0: the shielding against a core image failed.
    Kernabbild,
    /// Step 1: an argument was given.
    Aufruf,
    /// Step 2: `halter.json` is not readable, not valid, not root's or writable for the person.
    Konfig,
    /// Step 3: a level of the store breaks the rule.
    AblageRechte,
    /// Step 4: the endpoint exists already or cannot be made exclusive.
    PipeBelegt,
    /// Any unexpected failure.
    Intern,
}

impl Startfehler {
    /// Exit code, code and fixed reason.
    #[must_use]
    pub fn teile(self) -> (u8, &'static str, &'static str) {
        match self {
            Self::Kernabbild => (5, "halter.kernabbild", "Kernabbild nicht abschirmbar"),
            Self::Aufruf => (64, "halter.aufruf", "falscher Aufruf"),
            Self::Konfig => (78, "halter.konfig", "Konfiguration ungueltig"),
            Self::AblageRechte => (78, "halter.ablage_rechte", "Ablage hat falsche Rechte"),
            Self::PipeBelegt => (74, "halter.pipe_belegt", "Endpunkt schon belegt"),
            Self::Intern => (70, "halter.intern", "interner Fehler"),
        }
    }

    /// The one line `<code>: <grund>` (ASCII, at most 200 bytes, without path, content or secret).
    #[must_use]
    pub fn zeile(self) -> String {
        let (_, code, grund) = self.teile();
        format!("{code}: {grund}")
    }
}
