//! Own tests of the lines `ordnermarke.geloest` and `ordnermarke.gesenkt` beyond the visible cases (contract 5,
//! section 6.4): every character class of `marke`, every part of the coupling, the order of the errors, the
//! closed form in the general append and in the check of a folder and of a file.

#![allow(
    clippy::unnecessary_wraps,
    reason = "the helper wraps the text in Option like the fields of the line it fills"
)]

mod gemeinsam;

use std::ffi::OsString;

use formate::kette::{
    Fehler, NULL_HASH, Ordnermarke, abschnitt, anhaengen_ordnermarke, anhaengen_zeile,
    anhaengen_zeilen, befehl, pruefe_quelle, sha256_hex, zeile_ordnermarke,
};
use gemeinsam::Wegwerf;
use serde_json::{Value, json};

const ZEIT: i64 = 1_795_179_600_000;
const MARKE: &str = "a1b2c3d4e5f60718293a4b5c6d7e8f90";

fn geloest() -> Ordnermarke {
    Ordnermarke {
        zeit_ms: ZEIT,
        aktionsname: "ordnermarke.geloest".to_owned(),
        marke: MARKE.to_owned(),
        klasse_alt: Some("kundendaten".to_owned()),
        klasse_neu: None,
        kunde_alt: Some("k-beispiel".to_owned()),
        kunde_neu: None,
    }
}

fn gesenkt() -> Ordnermarke {
    Ordnermarke {
        zeit_ms: ZEIT,
        aktionsname: "ordnermarke.gesenkt".to_owned(),
        marke: MARKE.to_owned(),
        klasse_alt: Some("kundendaten".to_owned()),
        klasse_neu: Some("intern".to_owned()),
        kunde_alt: Some("k-beispiel".to_owned()),
        kunde_neu: Some("k-beispiel".to_owned()),
    }
}

fn s(text: &str) -> Option<String> {
    Some(text.to_owned())
}

fn code(o: &Ordnermarke) -> String {
    match zeile_ordnermarke(o, NULL_HASH) {
        Ok(_) => "ok".to_owned(),
        Err(fehler) => fehler.code(),
    }
}

/// The object of a general append for the kind and the four values.
fn objekt(art: &str, marke: &str) -> Value {
    json!({
        "aktionsname": art,
        "ergebnis": "ausgefuehrt",
        "klasse_alt": "kundendaten",
        "klasse_neu": if art == "ordnermarke.gesenkt" { json!("intern") } else { Value::Null },
        "kunde_alt": "k-beispiel",
        "kunde_neu": if art == "ordnermarke.gesenkt" { json!("k-beispiel") } else { Value::Null },
        "marke": marke,
    })
}

#[test]
fn both_kinds_are_valid_and_the_line_has_the_ten_fields_in_canonical_order() {
    let zeile = zeile_ordnermarke(&geloest(), NULL_HASH).expect("valid");
    assert_eq!(
        zeile,
        format!(
            "{{\"aktionsname\":\"ordnermarke.geloest\",\"ergebnis\":\"ausgefuehrt\",\"fassung\":2,\"klasse_alt\":\"kundendaten\",\"klasse_neu\":null,\"kunde_alt\":\"k-beispiel\",\"kunde_neu\":null,\"marke\":\"{MARKE}\",\"vorher\":\"{NULL_HASH}\",\"zeit\":\"2026-11-20T13:00:00.000Z\"}}"
        )
    );
    assert_eq!(code(&gesenkt()), "ok");
}

#[test]
fn the_mark_is_32_lower_case_hex_digits_and_nothing_else() {
    let mit = |marke: &str| {
        let mut o = geloest();
        o.marke = marke.to_owned();
        code(&o)
    };
    assert_eq!(mit("0123456789abcdef0123456789abcdef"), "ok");
    assert_eq!(mit(&"0".repeat(32)), "ok");
    assert_eq!(mit(&"9".repeat(32)), "ok");
    assert_eq!(mit(&"a".repeat(32)), "ok");
    assert_eq!(mit(&"f".repeat(32)), "ok");
    for falsch in [
        "",
        &"a".repeat(31),
        &"a".repeat(33),
        &"A".repeat(32),
        &"F".repeat(32),
        &"g".repeat(32),
        &"/".repeat(32),
        &":".repeat(32),
        &"`".repeat(32),
        &"-".repeat(32),
        &sha256_hex(b"/srv/beispiel/ordner"),
        "/srv/beispiel/ordner",
    ] {
        assert_eq!(mit(falsch), "feld_ungueltig:marke", "{falsch}");
    }
    for stelle in [0, 15, 31] {
        let mut marke = MARKE.to_owned();
        marke.replace_range(stelle..=stelle, "G");
        assert_eq!(mit(&marke), "feld_ungueltig:marke", "{marke}");
    }
}

#[test]
fn the_kind_must_be_one_of_the_two() {
    for art in [
        "ordnermarke.angelegt",
        "ordnermarke.",
        "ordnermarke",
        "",
        "Ordnermarke.geloest",
    ] {
        let mut o = geloest();
        o.aktionsname = art.to_owned();
        assert_eq!(code(&o), "feld_ungueltig:aktionsname", "{art}");
    }
}

#[test]
fn every_value_is_a_key_or_null() {
    let schlecht = ["", "Gross", "-a", ".a", "a b", &"a".repeat(65)];
    for wert in schlecht {
        let mut o = gesenkt();
        o.klasse_alt = s(wert);
        assert_eq!(code(&o), "feld_ungueltig:klasse_alt", "{wert}");
        let mut o = gesenkt();
        o.klasse_neu = s(wert);
        assert_eq!(code(&o), "feld_ungueltig:klasse_neu", "{wert}");
        let mut o = gesenkt();
        o.kunde_alt = s(wert);
        assert_eq!(code(&o), "feld_ungueltig:kunde_alt", "{wert}");
        let mut o = gesenkt();
        o.kunde_neu = s(wert);
        assert_eq!(code(&o), "feld_ungueltig:kunde_neu", "{wert}");
    }
    let mut o = gesenkt();
    o.klasse_alt = s(&"a".repeat(64));
    o.kunde_alt = s(&"0".repeat(64));
    o.kunde_neu = s(&"0".repeat(64));
    assert_eq!(code(&o), "ok");
}

#[test]
fn the_form_of_every_field_comes_before_the_coupling_and_in_a_fixed_order() {
    // geloest with klasse_neu set AND marke wrong: the form of marke is reported first.
    let mut o = geloest();
    o.klasse_neu = s("intern");
    o.marke = "x".to_owned();
    assert_eq!(code(&o), "feld_ungueltig:marke");
    // wrong aktionsname comes before wrong marke.
    let mut o = geloest();
    o.aktionsname = "ordnermarke.x".to_owned();
    o.marke = "x".to_owned();
    assert_eq!(code(&o), "feld_ungueltig:aktionsname");
    // marke before klasse_alt before klasse_neu before kunde_alt before kunde_neu.
    let mut o = geloest();
    o.marke = "x".to_owned();
    o.klasse_alt = s("A");
    assert_eq!(code(&o), "feld_ungueltig:marke");
    let mut o = geloest();
    o.klasse_alt = s("A");
    o.klasse_neu = s("B");
    assert_eq!(code(&o), "feld_ungueltig:klasse_alt");
    let mut o = geloest();
    o.klasse_neu = s("B");
    o.kunde_alt = s("C");
    assert_eq!(code(&o), "feld_ungueltig:klasse_neu");
    let mut o = geloest();
    o.kunde_alt = s("C");
    o.kunde_neu = s("D");
    assert_eq!(code(&o), "feld_ungueltig:kunde_alt");
    // a bad form beats a violated coupling in a later field.
    let mut o = geloest();
    o.kunde_neu = s("D");
    assert_eq!(code(&o), "feld_ungueltig:kunde_neu");
    let mut o = gesenkt();
    o.kunde_neu = s("D");
    o.klasse_alt = s("A");
    assert_eq!(code(&o), "feld_ungueltig:klasse_alt");
}

#[test]
fn released_needs_one_old_value_and_no_new_value() {
    let mut o = geloest();
    o.klasse_alt = None;
    assert_eq!(code(&o), "ok", "only the old customer");
    let mut o = geloest();
    o.kunde_alt = None;
    assert_eq!(code(&o), "ok", "only the old class");
    let mut o = geloest();
    o.klasse_alt = None;
    o.kunde_alt = None;
    assert_eq!(code(&o), "feld_ungueltig:klasse_alt");
    let mut o = geloest();
    o.klasse_neu = s("intern");
    assert_eq!(code(&o), "feld_ungueltig:klasse_neu");
    let mut o = geloest();
    o.kunde_neu = s("k-beispiel");
    assert_eq!(code(&o), "feld_ungueltig:kunde_neu");
    // both new values set: the class is reported first.
    let mut o = geloest();
    o.klasse_neu = s("intern");
    o.kunde_neu = s("k-beispiel");
    assert_eq!(code(&o), "feld_ungueltig:klasse_neu");
    // both old values null and a new value set: klasse_alt is reported first.
    let mut o = geloest();
    o.klasse_alt = None;
    o.kunde_alt = None;
    o.klasse_neu = s("intern");
    assert_eq!(code(&o), "feld_ungueltig:klasse_alt");
}

#[test]
fn lowered_needs_a_new_class_that_differs_and_the_same_customer() {
    let mut o = gesenkt();
    o.klasse_neu = None;
    assert_eq!(code(&o), "feld_ungueltig:klasse_neu", "klasse_neu null");
    let mut o = gesenkt();
    o.klasse_neu = o.klasse_alt.clone();
    assert_eq!(code(&o), "feld_ungueltig:klasse_neu", "same class");
    let mut o = gesenkt();
    o.klasse_alt = None;
    assert_eq!(code(&o), "ok", "the old class may be null");
    let mut o = gesenkt();
    o.kunde_neu = s("k-anderer");
    assert_eq!(code(&o), "feld_ungueltig:kunde_neu", "another customer");
    let mut o = gesenkt();
    o.kunde_neu = None;
    assert_eq!(code(&o), "feld_ungueltig:kunde_neu", "customer removed");
    let mut o = gesenkt();
    o.kunde_alt = None;
    assert_eq!(code(&o), "feld_ungueltig:kunde_neu", "customer added");
    let mut o = gesenkt();
    o.kunde_alt = None;
    o.kunde_neu = None;
    assert_eq!(code(&o), "ok", "no customer before and after");
    // the class is checked before the customer.
    let mut o = gesenkt();
    o.klasse_neu = o.klasse_alt.clone();
    o.kunde_neu = s("k-anderer");
    assert_eq!(code(&o), "feld_ungueltig:klasse_neu");
}

#[test]
fn the_time_and_the_previous_hash_come_last() {
    let mut o = geloest();
    o.zeit_ms = -1;
    assert_eq!(zeile_ordnermarke(&o, NULL_HASH), Err(Fehler::ZeitUngueltig));
    assert_eq!(
        zeile_ordnermarke(&geloest(), "abc"),
        Err(Fehler::FeldUngueltig("vorher"))
    );
    assert_eq!(
        zeile_ordnermarke(&geloest(), &"A".repeat(64)),
        Err(Fehler::FeldUngueltig("vorher"))
    );
    let mut o = geloest();
    o.zeit_ms = -1;
    assert_eq!(
        zeile_ordnermarke(&o, "abc"),
        Err(Fehler::FeldUngueltig("vorher")),
        "vorher before the time"
    );
    let mut o = geloest();
    o.zeit_ms = -1;
    o.marke = "x".to_owned();
    assert_eq!(code(&o), "feld_ungueltig:marke", "a field before the time");
}

#[test]
fn the_source_ordnermarke_is_known_and_names_its_file() {
    assert_eq!(
        abschnitt("ordnermarke", ZEIT),
        Ok("ordnermarke-2026-11.jsonl".to_owned())
    );
    assert_eq!(
        abschnitt("ordnermarken", ZEIT),
        Err(Fehler::FeldUngueltig("quelle"))
    );
    assert_eq!(
        abschnitt("Ordnermarke", ZEIT),
        Err(Fehler::FeldUngueltig("quelle"))
    );
    let ordner = Wegwerf::neu("om-quelle");
    assert_eq!(
        anhaengen_ordnermarke(ordner.ordner(), "ordnermarken", &geloest()),
        Err(Fehler::FeldUngueltig("quelle"))
    );
    assert!(anhaengen_ordnermarke(ordner.ordner(), "ordnermarke", &geloest()).is_ok());
    assert!(ordner.pfad("ordnermarke-2026-11.jsonl").is_file());
    assert!(ordner.pfad("ordnermarke.anker.jsonl").is_file());
}

#[test]
fn the_general_append_keeps_the_closed_form() {
    let ordner = Wegwerf::neu("om-allgemein");
    for art in ["ordnermarke.geloest", "ordnermarke.gesenkt"] {
        assert!(
            anhaengen_zeile(ordner.ordner(), "ordnermarke", ZEIT, &objekt(art, MARKE)).is_ok(),
            "{art}"
        );
    }
    let falsch = |aendern: &dyn Fn(&mut serde_json::Map<String, Value>)| {
        let mut o = objekt("ordnermarke.gesenkt", MARKE);
        aendern(o.as_object_mut().expect("object"));
        anhaengen_zeile(ordner.ordner(), "ordnermarke", ZEIT, &o)
    };
    let abgelehnt = Err(Fehler::FeldUngueltig("zeile"));
    assert_eq!(
        falsch(&|m| drop(m.insert("ordner".into(), json!("x")))),
        abgelehnt
    );
    assert_eq!(
        falsch(&|m| drop(m.insert("ordner_hash".into(), json!(NULL_HASH)))),
        abgelehnt
    );
    assert_eq!(falsch(&|m| drop(m.remove("kunde_alt"))), abgelehnt);
    assert_eq!(falsch(&|m| drop(m.remove("marke"))), abgelehnt);
    assert_eq!(
        falsch(&|m| drop(m.insert("marke".into(), json!(MARKE.to_uppercase())))),
        abgelehnt
    );
    assert_eq!(
        falsch(&|m| drop(m.insert("marke".into(), Value::Null))),
        abgelehnt
    );
    assert_eq!(
        falsch(&|m| drop(m.insert("marke".into(), json!(7)))),
        abgelehnt
    );
    assert_eq!(
        falsch(&|m| drop(m.insert("ergebnis".into(), json!("abgewiesen")))),
        abgelehnt
    );
    assert_eq!(
        falsch(&|m| drop(m.insert("klasse_neu".into(), json!(5)))),
        abgelehnt
    );
    assert_eq!(
        falsch(&|m| drop(m.insert("kunde_neu".into(), json!("Gross")))),
        abgelehnt
    );
    assert_eq!(
        falsch(&|m| drop(m.insert("klasse_neu".into(), json!("kundendaten")))),
        abgelehnt
    );
    assert_eq!(
        falsch(&|m| drop(m.insert("kunde_neu".into(), json!("k-anderer")))),
        abgelehnt
    );
    assert_eq!(
        falsch(&|m| drop(m.insert("aktionsname".into(), json!("ordnermarke.geloest")))),
        abgelehnt
    );
    // another kind with the prefix has no closed form and passes.
    let mut frei = objekt("ordnermarke.angelegt", MARKE);
    frei["zusatz"] = json!(1);
    assert!(anhaengen_zeile(ordner.ordner(), "ordnermarke", ZEIT, &frei).is_ok());
    // nothing of the refused lines was written: the two valid ones and the free one.
    let p = pruefe_quelle(ordner.ordner(), "ordnermarke", NULL_HASH, None).expect("source");
    assert!(p.ok());
    assert_eq!(p.zeilen, 3);
}

#[test]
fn appended_lines_of_the_chain_keep_the_closed_form() {
    let ordner = Wegwerf::neu("om-zeilen");
    let erste = zeile_ordnermarke(&geloest(), NULL_HASH).expect("line");
    let zweite = zeile_ordnermarke(&gesenkt(), &sha256_hex(erste.as_bytes())).expect("line");
    assert_eq!(
        anhaengen_zeilen(
            ordner.ordner(),
            "ordnermarke",
            NULL_HASH,
            &[erste.clone(), zweite.clone()]
        ),
        Ok(sha256_hex(zweite.as_bytes()))
    );
    // the line has a foreign field: refused as form_falsch.
    let ordner = Wegwerf::neu("om-zeilen-falsch");
    let mut wert: Value = serde_json::from_str(&erste).expect("json");
    wert["ordner"] = json!("x");
    let falsch = formate::kette::kanonisch(&wert).expect("canonical");
    assert_eq!(
        anhaengen_zeilen(ordner.ordner(), "ordnermarke", NULL_HASH, &[falsch]),
        Err(Fehler::ZeileUngueltig("form_falsch".to_owned()))
    );
}

#[test]
fn the_command_checks_the_source_ordnermarke() {
    let ordner = Wegwerf::neu("om-befehl");
    anhaengen_ordnermarke(ordner.ordner(), "ordnermarke", &geloest()).expect("append");
    let aufruf = |quelle: &str| -> Vec<OsString> {
        vec![
            "pruefe-quelle".into(),
            ordner.ordner().as_os_str().to_owned(),
            quelle.into(),
        ]
    };
    let ok = befehl(&aufruf("ordnermarke"));
    assert_eq!(ok.exit, 0, "{}", ok.stderr);
    assert!(ok.stdout.contains("\"zeilen\":1"), "{}", ok.stdout);
    let nein = befehl(&aufruf("ordnermarken"));
    assert_eq!(nein.exit, 64);
    assert_eq!(nein.stdout, "");
    assert!(nein.stderr.contains("ordnermarke"), "{}", nein.stderr);
}

#[test]
fn a_check_of_a_file_rejects_every_form_break() {
    let datei = |wert: &Value| -> (i32, String) {
        let ordner = Wegwerf::neu("om-datei");
        let zeile = formate::kette::kanonisch(wert).expect("canonical");
        let pfad = ordner.schreibe("brain-2026-11.jsonl", format!("{zeile}\n").as_bytes());
        let aus = befehl(&["pruefe".into(), pfad.into_os_string()]);
        (i32::from(aus.exit), aus.stdout)
    };
    let gut = |art: &str| {
        let mut wert = objekt(art, MARKE);
        wert["fassung"] = json!(2);
        wert["vorher"] = json!(NULL_HASH);
        wert["zeit"] = json!("2026-11-20T13:00:00.000Z");
        wert
    };
    for art in ["ordnermarke.geloest", "ordnermarke.gesenkt"] {
        assert_eq!(datei(&gut(art)).0, 0, "{art}");
        let mut zwei = gut(art);
        zwei["fassung"] = json!(1);
        assert_eq!(datei(&zwei).0, 1, "{art} fassung 1");
        let mut ohne = gut(art);
        ohne.as_object_mut().expect("object").remove("fassung");
        assert_eq!(datei(&ohne).0, 1, "{art} without fassung");
        let mut zeit = gut(art);
        zeit["zeit"] = json!("2026-11-20 13:00:00");
        assert_eq!(datei(&zeit).0, 1, "{art} zeit");
        let mut vorher = gut(art);
        vorher["vorher"] = json!("abc");
        assert_eq!(datei(&vorher).0, 1, "{art} vorher");
        let mut marke = gut(art);
        marke["marke"] = json!(sha256_hex(b"x"));
        let (exit, aus) = datei(&marke);
        assert_eq!(exit, 1, "{art} marke");
        assert!(aus.contains("form_falsch"), "{aus}");
    }
    // another kind with the prefix has no closed form.
    let mut frei = gut("ordnermarke.angelegt");
    frei["zusatz"] = json!(1);
    assert_eq!(datei(&frei).0, 0);
    // the coupling alone breaks the check.
    let mut o = gut("ordnermarke.gesenkt");
    o["klasse_neu"] = o["klasse_alt"].clone();
    assert_eq!(datei(&o).0, 1, "same class");
    let mut o = gut("ordnermarke.gesenkt");
    o["klasse_neu"] = Value::Null;
    assert_eq!(datei(&o).0, 1, "klasse_neu null");
    let mut o = gut("ordnermarke.gesenkt");
    o["kunde_neu"] = json!("k-anderer");
    assert_eq!(datei(&o).0, 1, "another customer");
    let mut o = gut("ordnermarke.geloest");
    o["klasse_alt"] = Value::Null;
    o["kunde_alt"] = Value::Null;
    assert_eq!(datei(&o).0, 1, "nothing to release");
    let mut o = gut("ordnermarke.geloest");
    o["klasse_neu"] = json!("intern");
    assert_eq!(datei(&o).0, 1, "new class after release");
    let mut o = gut("ordnermarke.geloest");
    o["kunde_neu"] = json!("k-beispiel");
    assert_eq!(datei(&o).0, 1, "new customer after release");
}
