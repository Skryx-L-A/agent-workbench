//! Unit tests of the helpers of `kette.rs`; the cases of the contract run in `tests/`.

use super::{
    Fehler, Gesperrt, NULL_HASH, abschnitt, ist_sha256, schluessel_ok, zeile_gesperrt, zeit_muster,
};

type Aenderung = Box<dyn Fn(&mut Gesperrt)>;

fn muster() -> Gesperrt {
    Gesperrt {
        zeit_ms: 1_795_179_600_000,
        route: "cloud-beispiel".to_owned(),
        sitzung: "s-0001".to_owned(),
        grund: "kundendaten-an-cloud".to_owned(),
        klasse: Some("kundendaten".to_owned()),
        kunde: Some("k-beispiel".to_owned()),
        anzahl: 1,
    }
}

#[test]
fn keys_follow_the_pattern() {
    let lang_ok = format!("a{}", "b".repeat(63));
    let lang_falsch = format!("a{}", "b".repeat(64));
    for ok in ["a", "0", "a.b-c_d", "s-0001", lang_ok.as_str(), "9z"] {
        assert!(schluessel_ok(ok), "{ok}");
    }
    for falsch in [
        "",
        "A",
        "-a",
        ".a",
        "_a",
        "a b",
        "a/b",
        "ä",
        "a\n",
        "a:b",
        "a@b",
        "`",
        "{",
        lang_falsch.as_str(),
    ] {
        assert!(!schluessel_ok(falsch), "{falsch}");
    }
}

#[test]
fn sha256_text_is_64_lower_case_hex() {
    assert!(ist_sha256(NULL_HASH));
    assert!(ist_sha256(&"0123456789abcdef".repeat(4)));
    assert!(!ist_sha256(&"A".repeat(64)));
    assert!(!ist_sha256(&"g".repeat(64)));
    assert!(!ist_sha256(&"a".repeat(63)));
    assert!(!ist_sha256(&"a".repeat(65)));
    assert!(!ist_sha256(""));
    assert!(!ist_sha256(&format!("{}`", "a".repeat(63))));
    assert!(!ist_sha256(&format!("{}{{", "a".repeat(63))));
}

#[test]
fn time_pattern() {
    for ok in [
        "2026-11-20T13:00:00.000Z",
        "0000-01-01T00:00:00.000Z",
        "9999-12-31T23:59:59.999Z",
        "2026-02-31T13:00:00.000Z",
    ] {
        assert!(zeit_muster(ok), "{ok}");
    }
    for falsch in [
        "",
        "2026-11-20T13:00:00Z",
        "2026-11-20T13:00:00.000",
        "2026-11-20 13:00:00.000Z",
        "2026-13-20T13:00:00.000Z",
        "2026-00-20T13:00:00.000Z",
        "2026-11-00T13:00:00.000Z",
        "2026-11-32T13:00:00.000Z",
        "2026-11-20T24:00:00.000Z",
        "2026-11-20T13:60:00.000Z",
        "2026-11-20T13:00:60.000Z",
        "2026-11-20T13:00:00.00Z",
        "2026-11-20T13:00:00.0000Z",
        "2026-11-20T13:00:00,000Z",
        "2026/11/20T13:00:00.000Z",
        "2026-11-20T13-00:00.000Z",
        "2026-11-20T13:00-00.000Z",
        "2026-1-20T13:00:00.000Z",
        "202a-11-20T13:00:00.000Z",
        "2026-1a-20T13:00:00.000Z",
        "2026-11-2aT13:00:00.000Z",
        "2026-11-20T1a:00:00.000Z",
        "2026-11-20T13:0a:00.000Z",
        "2026-11-20T13:00:0a.000Z",
        "2026-11-20T13:00:00.00aZ",
        "2026-11-20T13:00:00.000z",
        "+026-11-20T13:00:00.000Z",
        "2026-11-20T13:00:00.000Zx",
        "２０２６-11-20T13:00:00.000Z",
    ] {
        assert!(!zeit_muster(falsch), "{falsch}");
    }
}

#[test]
fn the_first_wrong_field_wins_in_the_order_of_the_contract() {
    let falsch = "X";
    let faelle: [(&str, Aenderung); 6] = [
        ("route", Box::new(|g| g.route = falsch.to_owned())),
        ("klasse", Box::new(|g| g.klasse = Some(String::new()))),
        ("kunde", Box::new(|g| g.kunde = Some(falsch.to_owned()))),
        ("sitzung", Box::new(|g| g.sitzung = falsch.to_owned())),
        ("grund", Box::new(|g| g.grund = String::new())),
        ("anzahl", Box::new(|g| g.anzahl = 0)),
    ];
    // Breaking field i and every later one must report field i.
    for (i, (name, _)) in faelle.iter().enumerate() {
        let mut g = muster();
        for (_, aendern) in &faelle[i..] {
            aendern(&mut g);
        }
        g.zeit_ms = -1;
        assert_eq!(
            zeile_gesperrt(&g, "zzz"),
            Err(Fehler::FeldUngueltig(name)),
            "field {name}"
        );
    }
    let mut g = muster();
    g.zeit_ms = -1;
    assert_eq!(
        zeile_gesperrt(&g, "zzz"),
        Err(Fehler::FeldUngueltig("vorher"))
    );
    assert_eq!(zeile_gesperrt(&g, NULL_HASH), Err(Fehler::ZeitUngueltig));
}

#[test]
fn null_class_and_customer_are_allowed_and_written_as_null() {
    let mut g = muster();
    g.klasse = None;
    g.kunde = None;
    let zeile = zeile_gesperrt(&g, NULL_HASH).expect("valid line");
    assert!(zeile.contains("\"klasse\":null,\"kunde\":null,"), "{zeile}");
}

#[test]
fn abschnitt_checks_the_source_before_the_time() {
    assert_eq!(abschnitt("audit", -1), Err(Fehler::FeldUngueltig("quelle")));
    assert_eq!(abschnitt("brain", -1), Err(Fehler::ZeitUngueltig));
    for quelle in ["gateway", "stelle", "brain", "halter", "ordnermarke"] {
        assert_eq!(
            abschnitt(quelle, 1_795_179_600_000),
            Ok(format!("{quelle}-2026-11.jsonl"))
        );
    }
}

#[test]
fn error_codes_and_texts() {
    assert_eq!(Fehler::NichtEndlich.code(), "nicht_endlich");
    assert_eq!(Fehler::ZeitUngueltig.code(), "zeit_ungueltig");
    assert_eq!(
        Fehler::FeldUngueltig("route").code(),
        "feld_ungueltig:route"
    );
    assert_eq!(
        Fehler::LetzteZeileUnvollstaendig.code(),
        "letzte_zeile_unvollstaendig"
    );
    assert_eq!(Fehler::EinAusgabe("x".to_owned()).code(), "ein_ausgabe");
    assert_eq!(Fehler::ZeitUngueltig.to_string(), "zeit_ungueltig");
    assert_eq!(
        Fehler::EinAusgabe("kaputt".to_owned()).to_string(),
        "ein_ausgabe: kaputt"
    );
}
