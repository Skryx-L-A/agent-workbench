//! The error codes are the closed set of the contract, each with its fixed text.

use crate::Fehler;

#[test]
fn the_codes_are_those_of_the_contract_table_in_its_order() {
    let soll = [
        "esc_ungueltig",
        "sgr_unvollstaendig",
        "sgr_unbekannt",
        "sgr_parameter",
        "zeichen_ungueltig",
        "stil_ungueltig",
        "zeile_nicht_kanonisch",
        "art_unbekannt",
        "nachricht_ungueltig",
        "groesse_ungueltig",
        "cursor_ungueltig",
        "kein_abzug",
        "luecke",
        "stand_alt",
        "zeile_ausserhalb",
        "zeile_ungueltig",
        "rahmen_abgeschnitten",
        "rahmen_rest",
        "rahmen_zu_gross",
        "art_ungueltig",
        "flags_unbekannt",
        "rahmen_ungueltig",
        "vu_ungueltig",
        "utf8_ungueltig",
        "stilnr_ungueltig",
        "schub_ungueltig",
        "zeile_reihenfolge",
        "inflate_fehler",
    ];
    let ist: Vec<&str> = Fehler::ALLE.iter().map(|f| f.code()).collect();
    assert_eq!(ist, soll);
    for f in Fehler::ALLE {
        assert_eq!(f.to_string(), f.code());
        assert_eq!(format!("{f}"), f.code());
    }
}
