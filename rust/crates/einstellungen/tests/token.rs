//! Outside tests of the admin switch `eingeschraenktes_token` and of the targets of the plan Business (contract 2,
//! version 2, addendum Business): the typed view, the default "off" and the place of stage 37.
//!
//! The case files of `rust/spec/einstellungen` fix the codes of single files; these tests pin what the reader hands
//! on: the switch is `false` unless the file says `true`, and the delivered files bind the key only to exact hosts.

use std::path::Path;

use einstellungen::{Zugriff, lesen};
use serde_json::{Value, json};

/// A valid file of version 2 with one cloud route with one bound target and one local route.
fn basis() -> Value {
    json!({
        "fassung": 2,
        "klassen": ["a", "b"],
        "routen": [
            {"schluessel": "c", "art": "cloud", "klasse": "a", "netzziele": [
                {"host": "api.cloud.example", "port": 443, "zugriff": "kopf-einsetzen", "schluessel": true,
                 "pfade": [{"methode": "POST", "pfad": "/chat/completions"}]}]},
            {"schluessel": "l", "art": "lokal", "klasse": "b", "ziel": "http://127.0.0.1:11434/v1",
             "modell": "m", "netzziele": []}
        ],
        "kunden": [],
        "kundendaten_an_cloud": {"vorgabe": "nie", "freigaben": []}
    })
}

fn code(wert: &Value) -> &'static str {
    match lesen(wert.to_string().as_bytes()) {
        Ok(_) => "ok",
        Err(fehler) => fehler.code(),
    }
}

fn schalter(wert: &Value, route: &str) -> bool {
    lesen(wert.to_string().as_bytes())
        .expect("the file is valid")
        .route(route)
        .expect("the route exists")
        .eingeschraenktes_token()
}

#[test]
fn without_the_field_the_switch_is_off() {
    assert!(!schalter(&basis(), "c"));
    assert!(!schalter(&basis(), "l"));
}

#[test]
fn the_switch_is_read_as_written() {
    let mut an = basis();
    an["routen"][0]["eingeschraenktes_token"] = json!(true);
    assert!(schalter(&an, "c"));
    assert!(
        !schalter(&an, "l"),
        "the switch of one route never reaches another"
    );
    let mut aus = basis();
    aus["routen"][0]["eingeschraenktes_token"] = json!(false);
    assert!(!schalter(&aus, "c"));
}

#[test]
fn a_switch_that_reaches_no_bound_target_is_stage_35() {
    let mut lokal = basis();
    lokal["routen"][1]["eingeschraenktes_token"] = json!(true);
    assert_eq!(code(&lokal), "einst.token_ohne_schluessel");
    let mut ungebunden = basis();
    ungebunden["routen"][0]["netzziele"][0]["schluessel"] = json!(false);
    ungebunden["routen"][0]["eingeschraenktes_token"] = json!(true);
    assert_eq!(code(&ungebunden), "einst.token_ohne_schluessel");
    // off at a route without a bound target stays valid: it changes nothing
    ungebunden["routen"][0]["eingeschraenktes_token"] = json!(false);
    assert_eq!(code(&ungebunden), "ok");
}

#[test]
fn a_switch_that_is_no_boolean_is_stage_6() {
    for wert in [json!("ja"), json!(1), json!(null), json!([true])] {
        let mut datei = basis();
        datei["routen"][0]["eingeschraenktes_token"] = wert.clone();
        assert_eq!(code(&datei), "einst.typ_falsch", "{wert}");
    }
}

#[test]
fn the_switch_is_unknown_in_version_1() {
    let datei = json!({
        "fassung": 1,
        "klassen": ["a"],
        "routen": [{"schluessel": "c", "art": "cloud", "klasse": "a", "eingeschraenktes_token": true}],
        "kunden": [],
        "kundendaten_an_cloud": {"vorgabe": "nie", "freigaben": []}
    });
    assert_eq!(code(&datei), "einst.schluessel_unbekannt");
}

/// The hosts of the route `copilot` in a delivered file, with their key binding.
fn copilot_ziele(datei: &str) -> Vec<(String, bool)> {
    let pfad = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../spec/einstellungen")
        .join(datei);
    let roh = std::fs::read(&pfad).expect("the delivered file is readable");
    let einst = lesen(&roh).expect("the delivered file is valid");
    let route = einst.route("copilot").expect("route copilot");
    assert!(
        !route.eingeschraenktes_token(),
        "{datei}: the delivered file keeps the switch off"
    );
    route
        .netzziele()
        .iter()
        .filter(|z| z.zugriff() == Zugriff::KopfEinsetzen)
        .map(|z| (z.host().to_owned(), z.schluessel()))
        .collect()
}

#[test]
fn the_delivered_files_carry_both_plans_and_bind_the_key_only_to_exact_hosts() {
    for datei in ["vorfuehrung.json", "erststart.json"] {
        let ziele = copilot_ziele(datei);
        let mut gebunden: Vec<&str> = ziele
            .iter()
            .filter(|(_, s)| *s)
            .map(|(h, _)| h.as_str())
            .collect();
        gebunden.sort_unstable();
        assert_eq!(
            gebunden,
            [
                "api.business.githubcopilot.com",
                "api.github.com",
                "api.githubcopilot.com",
                "api.individual.githubcopilot.com"
            ],
            "{datei}"
        );
        for telemetrie in [
            "telemetry.business.githubcopilot.com",
            "telemetry.individual.githubcopilot.com",
        ] {
            assert!(
                ziele.iter().any(|(h, s)| h == telemetrie && !*s),
                "{datei}: {telemetrie} without key"
            );
        }
        assert!(
            ziele.iter().all(|(h, s)| !(*s && h.starts_with("*."))),
            "{datei}: no key at a subdomain form"
        );
    }
}
