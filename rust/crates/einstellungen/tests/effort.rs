//! Outside tests of the block `effort` of contract 2 (version 2, addendum of 26 November 2026): the typed
//! view of the limits and the rule of the allowed efforts of a start.
//!
//! The case files of `rust/spec/einstellungen` fix the codes and some allowed lists; these tests pin the
//! accessors and the edges of the rule.

#![allow(clippy::needless_pass_by_value)]

use einstellungen::{Einstellungen, effort_werte, lesen};
use serde_json::{Value, json};

/// No effort is allowed.
const KEINER: [&str; 0] = [];

/// A valid file of version 2 with a cloud route of class `a` and a local route of class `b`.
fn mit_effort(block: Value) -> Einstellungen {
    let datei = json!({
        "fassung": 2,
        "klassen": ["a", "b"],
        "routen": [
            {"schluessel": "c", "art": "cloud", "klasse": "a", "netzziele": []},
            {"schluessel": "l", "art": "lokal", "klasse": "b", "ziel": "http://127.0.0.1:11434/v1",
             "modell": "qwen3:8b", "netzziele": []}
        ],
        "kunden": [],
        "kundendaten_an_cloud": {"vorgabe": "nie", "freigaben": []},
        "effort": block
    });
    lesen(datei.to_string().as_bytes()).expect("settings valid")
}

#[test]
fn the_limits_are_kept_in_the_order_of_the_file_with_all_fields() {
    let einst = mit_effort(json!({"grenzen": [
        {"harness": "copilot", "erlaubt": ["high", "low"]},
        {"harness": "pi", "klasse": "b", "modell": "qwen3:8b", "erlaubt": ["off"]}
    ]}));
    let grenzen = einst.effort_grenzen();
    assert_eq!(grenzen.len(), 2);
    assert_eq!(grenzen[0].harness(), "copilot");
    assert_eq!(grenzen[0].klasse(), None);
    assert_eq!(grenzen[0].modell(), None);
    assert_eq!(grenzen[0].erlaubt(), ["high", "low"]);
    assert_eq!(grenzen[1].harness(), "pi");
    assert_eq!(grenzen[1].klasse(), Some("b"));
    assert_eq!(grenzen[1].modell(), Some("qwen3:8b"));
    assert_eq!(grenzen[1].erlaubt(), ["off"]);
}

#[test]
fn the_allowed_efforts_follow_the_order_of_the_harness_and_every_matching_limit() {
    let einst = mit_effort(json!({"grenzen": [
        {"harness": "copilot", "erlaubt": ["max", "low", "none", "medium"]},
        {"harness": "copilot", "klasse": "a", "erlaubt": ["medium", "low", "max"]},
        {"harness": "copilot", "modell": "gpt-6-luna", "erlaubt": ["low"]},
        {"harness": "copilot", "klasse": "b", "erlaubt": ["none"]},
        {"harness": "pi", "erlaubt": ["xhigh", "off", "minimal"]}
    ]}));
    assert_eq!(
        einst.efforts("copilot", "c", "auto"),
        ["low", "medium", "max"]
    );
    assert_eq!(einst.efforts("copilot", "c", "gpt-6-luna"), ["low"]);
    assert_eq!(
        einst.efforts("pi", "l", "qwen3:8b"),
        ["off", "minimal", "xhigh"]
    );
    assert_eq!(einst.efforts("claude", "c", "auto"), KEINER);
    assert_eq!(einst.efforts("copilot", "unbekannt", "auto"), KEINER);
    assert_eq!(einst.efforts("codex", "c", "auto"), KEINER);
}

#[test]
fn a_limit_of_another_class_or_model_does_not_match() {
    let einst = mit_effort(json!({"grenzen": [
        {"harness": "pi", "klasse": "a", "erlaubt": ["low"]},
        {"harness": "copilot", "modell": "m1", "erlaubt": ["low"]}
    ]}));
    assert_eq!(einst.efforts("pi", "l", "qwen3:8b"), KEINER);
    assert_eq!(einst.efforts("copilot", "c", "m2"), KEINER);
    assert_eq!(einst.efforts("copilot", "c", "auto"), KEINER);
    assert_eq!(einst.efforts("copilot", "c", "m1"), ["low"]);
}

#[test]
fn without_the_block_or_with_an_empty_list_no_effort_is_allowed() {
    let leer = mit_effort(json!({"grenzen": []}));
    assert_eq!(leer.effort_grenzen().len(), 0);
    assert_eq!(leer.efforts("copilot", "c", "auto"), KEINER);
    let ohne = lesen(
        json!({"fassung": 2, "klassen": ["a"],
            "routen": [{"schluessel": "c", "art": "cloud", "klasse": "a", "netzziele": []}],
            "kunden": [], "kundendaten_an_cloud": {"vorgabe": "nie", "freigaben": []}})
        .to_string()
        .as_bytes(),
    )
    .expect("settings valid");
    assert_eq!(ohne.effort_grenzen().len(), 0);
    assert_eq!(ohne.efforts("copilot", "c", "auto"), KEINER);
}

#[test]
fn every_value_of_every_harness_is_accepted_in_a_limit() {
    for harness in ["claude", "copilot", "pi"] {
        let werte = effort_werte(harness).expect("harness of the table");
        let einst = mit_effort(json!({"grenzen": [{"harness": harness, "erlaubt": werte}]}));
        let route = if harness == "pi" { "l" } else { "c" };
        assert_eq!(einst.efforts(harness, route, "qwen3:8b"), werte);
    }
}

#[test]
fn a_harness_of_the_wrong_form_is_stage_9_even_without_a_class() {
    let datei = json!({
        "fassung": 2,
        "klassen": ["a"],
        "routen": [{"schluessel": "c", "art": "cloud", "klasse": "a", "netzziele": []}],
        "kunden": [],
        "kundendaten_an_cloud": {"vorgabe": "nie", "freigaben": []},
        "effort": {"grenzen": [{"harness": "Pi", "erlaubt": ["low"]}]}
    });
    assert_eq!(
        lesen(datei.to_string().as_bytes())
            .map(|_| ())
            .map_err(|f| f.code()),
        Err("einst.kennung_form")
    );
}

/// The code of reading a file with the block `block`, or `Ok`.
fn code(block: Value) -> Result<(), &'static str> {
    let datei = json!({
        "fassung": 2,
        "klassen": ["a", "b"],
        "routen": [{"schluessel": "c", "art": "cloud", "klasse": "a", "netzziele": []}],
        "kunden": [],
        "kundendaten_an_cloud": {"vorgabe": "nie", "freigaben": []},
        "effort": block
    });
    lesen(datei.to_string().as_bytes())
        .map(|_| ())
        .map_err(|f| f.code())
}

#[test]
fn the_default_is_the_one_of_the_first_matching_limit_that_has_one() {
    let einst = mit_effort(json!({"grenzen": [
        {"harness": "copilot", "klasse": "b", "erlaubt": ["none"], "vorgabe": "none"},
        {"harness": "copilot", "modell": "m1", "erlaubt": ["low"]},
        {"harness": "copilot", "erlaubt": ["none", "low"], "vorgabe": "low"},
        {"harness": "copilot", "modell": "m2", "erlaubt": ["none", "low"], "vorgabe": "none"},
        {"harness": "pi", "erlaubt": ["off", "low"]}
    ]}));
    let grenzen = einst.effort_grenzen();
    assert_eq!(grenzen[0].vorgabe(), Some("none"));
    assert_eq!(grenzen[1].vorgabe(), None);
    assert_eq!(grenzen[2].vorgabe(), Some("low"));
    // the limit of class b does not match the cloud route; the limit of m1 has no default
    assert_eq!(einst.effort_vorgabe("copilot", "c", "m1"), Some("low"));
    assert_eq!(einst.effort_vorgabe("copilot", "c", "m2"), Some("low"));
    assert_eq!(einst.effort_vorgabe("copilot", "c", "auto"), Some("low"));
    assert_eq!(einst.effort_vorgabe("pi", "l", "qwen3:8b"), None);
    assert_eq!(einst.effort_vorgabe("claude", "c", "auto"), None);
    assert_eq!(einst.effort_vorgabe("copilot", "unbekannt", "auto"), None);
    let ohne = mit_effort(json!({"grenzen": []}));
    assert_eq!(ohne.effort_vorgabe("copilot", "c", "auto"), None);
}

#[test]
fn the_default_is_a_value_of_the_harness_but_need_not_be_allowed() {
    assert_eq!(
        code(json!({"grenzen": [{"harness": "copilot", "erlaubt": ["low"], "vorgabe": "medium"}]})),
        Ok(())
    );
    assert_eq!(
        code(json!({"grenzen": [{"harness": "pi", "erlaubt": ["low"], "vorgabe": "none"}]})),
        Err("einst.effort_wert")
    );
    assert_eq!(
        code(json!({"grenzen": [{"harness": "copilot", "erlaubt": ["low"], "vorgabe": "Low"}]})),
        Err("einst.effort_wert")
    );
    assert_eq!(
        code(json!({"grenzen": [{"harness": "copilot", "erlaubt": ["low"], "vorgabe": 1}]})),
        Err("einst.typ_falsch")
    );
    assert_eq!(
        code(json!({"grenzen": [{"harness": "copilot", "erlaubt": ["low"], "vorgabe": null}]})),
        Err("einst.typ_falsch")
    );
}
