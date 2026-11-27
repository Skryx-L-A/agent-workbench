//! The fixed codes of the helper (contract `rust/spec/regelebenen/README.md`, section 15.12.3, stages H1 to H12) and the
//! exit codes of the program (`rust/spec/paket/README.md`, part A3, "Die Einheiten").

/// One code of a refused or failed request. The order of the variants is the order of the stages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Code {
    /// H1: the peer cannot be determined, or its uid has no account.
    PartnerUnbekannt,
    /// H2: the peer runs in the unit of a run.
    PartnerEingeschlossen,
    /// H3: the file, its folder or a parent folder is not in the required form.
    DateiUnsicher,
    /// H4: the valid file does not have the form.
    Ladefehler,
    /// H5: the uid of the peer belongs to no account of the list of the valid file.
    PartnerNichtAdmin,
    /// H6: the request has more than 1048576 bytes.
    AnfrageZuGross,
    /// H7: the request is not complete 2 s after the accept.
    AnfrageFrist,
    /// H8: the request does not have the form.
    AnfrageUngueltig,
    /// H9: `basis` is not the sha256 of the bytes of the file.
    Veraltet,
    /// H10: `inhalt` differs from the valid file outside `leseklassen`.
    AusserLeseklassen,
    /// H11: the temporary file cannot be made exclusively or not written in full.
    TempFehlgeschlagen,
    /// H11: `fsync` of the temporary file failed.
    FsyncFehlgeschlagen,
    /// H11: `rename` of the temporary file failed.
    RenameFehlgeschlagen,
    /// H12: `fsync` of the folder failed; the old bytes were written back.
    OrdnerFsyncFehlgeschlagen,
    /// H12: `fsync` of the folder failed and the way back failed as well.
    ZustandUnklar,
}

impl Code {
    /// The fixed text of the code, as the service passes it on as `grund`.
    #[must_use]
    pub fn text(self) -> &'static str {
        match self {
            Code::PartnerUnbekannt => "firmenvorgabe.partner_unbekannt",
            Code::PartnerEingeschlossen => "firmenvorgabe.partner_eingeschlossen",
            Code::DateiUnsicher => "firmenvorgabe.datei_unsicher",
            Code::Ladefehler => "firmenvorgabe.ladefehler",
            Code::PartnerNichtAdmin => "firmenvorgabe.partner_nicht_admin",
            Code::AnfrageZuGross => "firmenvorgabe.anfrage_zu_gross",
            Code::AnfrageFrist => "firmenvorgabe.anfrage_frist",
            Code::AnfrageUngueltig => "firmenvorgabe.anfrage_ungueltig",
            Code::Veraltet => "firmenvorgabe.veraltet",
            Code::AusserLeseklassen => "firmenvorgabe.ausser_leseklassen",
            Code::TempFehlgeschlagen => "firmenvorgabe.temp_fehlgeschlagen",
            Code::FsyncFehlgeschlagen => "firmenvorgabe.fsync_fehlgeschlagen",
            Code::RenameFehlgeschlagen => "firmenvorgabe.rename_fehlgeschlagen",
            Code::OrdnerFsyncFehlgeschlagen => "firmenvorgabe.ordner_fsync_fehlgeschlagen",
            Code::ZustandUnklar => "firmenvorgabe.zustand_unklar",
        }
    }

    /// Every code, in the order of the stages.
    pub const ALLE: [Code; 15] = [
        Code::PartnerUnbekannt,
        Code::PartnerEingeschlossen,
        Code::DateiUnsicher,
        Code::Ladefehler,
        Code::PartnerNichtAdmin,
        Code::AnfrageZuGross,
        Code::AnfrageFrist,
        Code::AnfrageUngueltig,
        Code::Veraltet,
        Code::AusserLeseklassen,
        Code::TempFehlgeschlagen,
        Code::FsyncFehlgeschlagen,
        Code::RenameFehlgeschlagen,
        Code::OrdnerFsyncFehlgeschlagen,
        Code::ZustandUnklar,
    ];
}

/// Exit code after SIGTERM or SIGINT.
pub const EXIT_ENDE: u8 = 0;
/// Exit code of a call with arguments.
pub const EXIT_AUFRUF: u8 = 64;
/// Exit code without an inherited socket, with more than one, or when the program does not run as `werkbank-vorgabe`.
pub const EXIT_KONFIG: u8 = 78;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn texte_sind_fest_und_verschieden() {
        let texte: Vec<&str> = Code::ALLE.iter().map(|c| c.text()).collect();
        assert_eq!(
            texte,
            [
                "firmenvorgabe.partner_unbekannt",
                "firmenvorgabe.partner_eingeschlossen",
                "firmenvorgabe.datei_unsicher",
                "firmenvorgabe.ladefehler",
                "firmenvorgabe.partner_nicht_admin",
                "firmenvorgabe.anfrage_zu_gross",
                "firmenvorgabe.anfrage_frist",
                "firmenvorgabe.anfrage_ungueltig",
                "firmenvorgabe.veraltet",
                "firmenvorgabe.ausser_leseklassen",
                "firmenvorgabe.temp_fehlgeschlagen",
                "firmenvorgabe.fsync_fehlgeschlagen",
                "firmenvorgabe.rename_fehlgeschlagen",
                "firmenvorgabe.ordner_fsync_fehlgeschlagen",
                "firmenvorgabe.zustand_unklar",
            ]
        );
        let mut sortiert = Code::ALLE.to_vec();
        sortiert.sort();
        assert_eq!(sortiert, Code::ALLE.to_vec(), "order of the stages");
        assert_eq!((EXIT_ENDE, EXIT_AUFRUF, EXIT_KONFIG), (0, 64, 78));
    }
}
