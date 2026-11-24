//! Settings: values, strictness, effective value; and the truthiness helper.

use serde_json::{Value, json};

use crate::regel::{self, Regel, eingebaut};
use crate::wert::wahr;

fn lesen(beschreibung: &Value) -> Result<Regel, String> {
    regel::lesen(beschreibung.as_object().expect("object"))
}

#[test]
fn the_built_in_rules_are_the_four_of_the_contract() {
    let regeln = eingebaut();
    assert_eq!(regeln.len(), 4);
    let schreiben = lesen(&regeln["osc52_schreiben"]).expect("rule");
    assert!(!schreiben.zahl);
    assert_eq!(schreiben.stufen, ["an", "aus"]);
    assert_eq!(schreiben.vorgabe, 0);
    assert!(schreiben.nutzer_darf);
    let grenze = lesen(&regeln["osc52_grenze"]).expect("rule");
    assert!(grenze.zahl && grenze.nutzer_darf);
    assert_eq!((grenze.vorgabe, grenze.min), (100_000, 0));
    let rate = lesen(&regeln["bildrate_max"]).expect("rule");
    assert!(rate.zahl && rate.nutzer_darf);
    assert_eq!((rate.vorgabe, rate.min), (63, 1));
    let rueckstau = lesen(&regeln["rueckstau_max"]).expect("rule");
    assert!(rueckstau.zahl && rueckstau.nutzer_darf);
    assert_eq!((rueckstau.vorgabe, rueckstau.min), (16_384, 1));
}

#[test]
fn a_number_rule_is_stricter_when_smaller_and_a_step_rule_when_later() {
    let zahl = lesen(&json!({"art": "zahl", "vorgabe": 5})).expect("rule");
    assert!(zahl.strenger(4, 5));
    assert!(!zahl.strenger(5, 5));
    assert!(!zahl.strenger(6, 5));
    let stufen =
        lesen(&json!({"art": "stufen", "stufen": ["a", "b", "c"], "vorgabe": "a"})).expect("rule");
    assert!(stufen.strenger(2, 1));
    assert!(!stufen.strenger(1, 1));
    assert!(!stufen.strenger(0, 1));
}

#[test]
fn values_map_between_json_and_numbers() {
    let zahl = lesen(&json!({"art": "zahl", "vorgabe": 5})).expect("rule");
    assert_eq!(zahl.wert_aus(&json!(7)), Some(7));
    assert_eq!(zahl.wert_aus(&json!("7")), None);
    assert_eq!(zahl.wert_aus(&json!(7.5)), None);
    assert_eq!(zahl.wert_aus(&json!(true)), None);
    assert_eq!(zahl.wert_json(7), json!(7));
    let stufen =
        lesen(&json!({"art": "stufen", "stufen": ["a", "b", "c"], "vorgabe": "b"})).expect("rule");
    assert_eq!(stufen.vorgabe, 1);
    assert_eq!(stufen.wert_aus(&json!("c")), Some(2));
    assert_eq!(stufen.wert_aus(&json!("x")), None);
    assert_eq!(stufen.wert_aus(&json!(1)), None);
    assert_eq!(stufen.wert_json(2), json!("c"));
    assert_eq!(stufen.wert_json(0), json!("a"));
    assert_eq!(stufen.wert_json(3), Value::Null);
    assert_eq!(stufen.wert_json(-1), Value::Null);
    assert_eq!(stufen.stufe(1), Some("b"));
    assert_eq!(stufen.stufe(3), None);
}

#[test]
fn group_beats_company_beats_default() {
    let mut r = lesen(&json!({"art": "zahl", "vorgabe": 10})).expect("rule");
    assert_eq!(r.basis(), (10, "vorgabe"));
    r.firma = Some(8);
    assert_eq!(r.basis(), (8, "firma"));
    r.gruppe = Some(9);
    assert_eq!(r.basis(), (9, "gruppe"));
    r.firma = None;
    assert_eq!(r.basis(), (9, "gruppe"));
}

#[test]
fn a_user_value_counts_only_while_it_is_stricter() {
    let mut r = lesen(&json!({"art": "zahl", "vorgabe": 10, "nutzer": {"p": 4, "q": 10, "s": 12}}))
        .expect("rule");
    assert_eq!(r.wirksam(Some("p")), (4, "nutzer"));
    assert_eq!(r.wirksam(Some("q")), (10, "vorgabe"));
    assert_eq!(r.wirksam(Some("s")), (10, "vorgabe"));
    assert_eq!(r.wirksam(Some("zz")), (10, "vorgabe"));
    assert_eq!(r.wirksam(None), (10, "vorgabe"));
    r.firma = Some(3);
    assert_eq!(r.wirksam(Some("p")), (3, "firma"));
    r.firma = Some(5);
    assert_eq!(r.wirksam(Some("p")), (4, "nutzer"));
}

#[test]
fn reading_a_rule_checks_its_parts() {
    assert!(lesen(&json!({"art": "zahl"})).is_err());
    assert!(lesen(&json!({"art": "zahl", "vorgabe": "x"})).is_err());
    assert!(lesen(&json!({"art": "stufen", "vorgabe": "a"})).is_err());
    assert!(lesen(&json!({"art": "stufen", "stufen": ["a", 1], "vorgabe": "a"})).is_err());
    assert!(lesen(&json!({"art": "stufen", "stufen": ["a"], "vorgabe": "b"})).is_err());
    assert!(lesen(&json!({"art": "zahl", "vorgabe": 1, "min": "x"})).is_err());
    assert!(lesen(&json!({"art": "zahl", "vorgabe": 1, "firma": "x"})).is_err());
    assert!(lesen(&json!({"art": "zahl", "vorgabe": 1, "gruppe": 1.5})).is_err());
    assert!(lesen(&json!({"art": "zahl", "vorgabe": 1, "nutzer": [1]})).is_err());
    assert!(lesen(&json!({"art": "zahl", "vorgabe": 1, "nutzer": {"p": "x"}})).is_err());
    let r = lesen(
        &json!({"vorgabe": "a", "stufen": ["a"], "firma": null, "gruppe": null, "nutzer": null,
        "nutzer_darf": 1, "min": 3}),
    )
    .expect("rule");
    assert!(!r.zahl);
    assert_eq!((r.firma, r.gruppe, r.min), (None, None, 3));
    assert!(r.nutzer_darf && r.nutzer.is_empty());
    let r = lesen(&json!({"art": "zahl", "vorgabe": 1, "firma": 2, "gruppe": 3})).expect("rule");
    assert_eq!((r.firma, r.gruppe), (Some(2), Some(3)));
    assert!(!r.nutzer_darf);
}

#[test]
fn truthiness_follows_python() {
    for falsch in [
        json!(null),
        json!(false),
        json!(0),
        json!(0.0),
        json!(""),
        json!([]),
        json!({}),
    ] {
        assert!(!wahr(&falsch), "{falsch}");
    }
    for wahrheit in [
        json!(true),
        json!(1),
        json!(-1),
        json!(0.5),
        json!(-0.5),
        json!("0"),
        json!("a"),
        json!([0]),
        json!({"a": 0}),
        json!(1_000_000_000_000_i64),
    ] {
        assert!(wahr(&wahrheit), "{wahrheit}");
    }
}
