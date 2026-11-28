//! Outside tests of the effort "Lauf starten" passes on (contract 1, "Effort immer ausdrücklich"; decision of the
//! operator, 28 November 2026): `start::effort` and the same value in stage 18 of `pruefen`.

use einstellungen::{Einstellungen, lesen as lesen_einstellungen};
use serde_json::{Value, json};
use start::{COPILOT_RUECKFALL, effort, lesen_anfrage, pruefen};

/// Settings of version 2 with a cloud route `cloud` (class `a`) and a local route `lokal` (class `b`), and the limits
/// `grenzen` (no block when `None`).
fn einst(grenzen: Option<Value>) -> Einstellungen {
    let mut wert = json!({"fassung": 2, "klassen": ["a", "b"],
        "routen": [{"schluessel": "cloud", "art": "cloud", "klasse": "a", "netzziele": []},
                   {"schluessel": "lokal", "art": "lokal", "klasse": "b",
                    "ziel": "http://127.0.0.1:11434/v1", "modell": "qwen3:8b", "netzziele": []}],
        "kunden": [{"schluessel": "k"}],
        "kundendaten_an_cloud": {"vorgabe": "nie", "freigaben": []}});
    if let Some(g) = grenzen {
        wert["effort"] = json!({ "grenzen": g });
    }
    lesen_einstellungen(wert.to_string().as_bytes()).expect("settings valid")
}

fn start(harness: &str, effort: Option<&str>) -> Value {
    let (route, modell) = if harness == "copilot" || harness == "claude" {
        ("cloud", "gpt-6-luna")
    } else {
        ("lokal", "qwen3:8b")
    };
    json!({"fassung": 2, "harness": harness, "modell": modell, "route": route, "kunde": null,
        "arbeitsordner": "/heim/beispiel/projekt", "effort": effort, "name": null})
}

/// The effort passed on and the result of the check (`Ok` or the code).
fn lauf(start: &Value, einst: &Einstellungen) -> (Option<String>, Result<(), &'static str>) {
    let a = lesen_anfrage(start.to_string().as_bytes()).expect("request valid");
    (
        effort(&a, einst).map(str::to_owned),
        pruefen(&a, einst).map(|_| ()).map_err(|f| f.code()),
    )
}

const LUNA: &str = "gpt-6-luna";

#[test]
fn copilot_without_an_effort_and_without_a_default_gets_none() {
    assert_eq!(COPILOT_RUECKFALL, "none");
    let grenze = einst(Some(
        json!([{"harness": "copilot", "modell": LUNA, "erlaubt": ["none", "low"]}]),
    ));
    assert_eq!(
        lauf(&start("copilot", None), &grenze),
        (Some("none".to_owned()), Ok(()))
    );
    // no effort can be chosen (no block, or no matching limit): `none` stands for the start without an effort
    for e in [
        einst(None),
        einst(Some(json!([]))),
        einst(Some(json!([{"harness": "pi", "erlaubt": ["low"]}]))),
    ] {
        assert_eq!(
            lauf(&start("copilot", None), &e),
            (Some("none".to_owned()), Ok(()))
        );
    }
    // a limit that allows efforts, but not `none`: the inserted value is outside
    let ohne_none = einst(Some(
        json!([{"harness": "copilot", "erlaubt": ["low", "medium"]}]),
    ));
    assert_eq!(
        lauf(&start("copilot", None), &ohne_none),
        (Some("none".to_owned()), Err("start.effort_grenze"))
    );
}

#[test]
fn a_start_without_an_effort_takes_the_default_of_the_matching_limit() {
    let mit_low = einst(Some(json!([
        {"harness": "copilot", "klasse": "a", "modell": LUNA, "erlaubt": ["none", "low"], "vorgabe": "low"}
    ])));
    assert_eq!(
        lauf(&start("copilot", None), &mit_low),
        (Some("low".to_owned()), Ok(()))
    );
    // an effort of the start goes before the default
    assert_eq!(
        lauf(&start("copilot", Some("none")), &mit_low),
        (Some("none".to_owned()), Ok(()))
    );
    assert_eq!(
        lauf(&start("copilot", Some("low")), &mit_low),
        (Some("low".to_owned()), Ok(()))
    );
    assert_eq!(
        lauf(&start("copilot", Some("medium")), &mit_low),
        (Some("medium".to_owned()), Err("start.effort_grenze"))
    );
}

#[test]
fn a_default_outside_the_limit_ends_with_the_limit_code() {
    // the default is a value of copilot, but not allowed by the limit
    let aussen = einst(Some(json!([
        {"harness": "copilot", "erlaubt": ["none", "low"], "vorgabe": "medium"}
    ])));
    assert_eq!(
        lauf(&start("copilot", None), &aussen),
        (Some("medium".to_owned()), Err("start.effort_grenze"))
    );
    // a second matching limit narrows the choice below the default of the first
    let enger = einst(Some(json!([
        {"harness": "copilot", "erlaubt": ["none", "low"], "vorgabe": "low"},
        {"harness": "copilot", "modell": LUNA, "erlaubt": ["none"]}
    ])));
    assert_eq!(
        lauf(&start("copilot", None), &enger),
        (Some("low".to_owned()), Err("start.effort_grenze"))
    );
}

#[test]
fn pi_and_opencode_get_an_effort_only_from_a_default() {
    for h in ["pi", "opencode"] {
        assert_eq!(lauf(&start(h, None), &einst(None)), (None, Ok(())));
        let ohne_vorgabe = einst(Some(json!([{"harness": h, "erlaubt": ["low"]}])));
        assert_eq!(lauf(&start(h, None), &ohne_vorgabe), (None, Ok(())));
    }
    let pi = einst(Some(json!([
        {"harness": "pi", "klasse": "b", "erlaubt": ["off", "low"], "vorgabe": "off"}
    ])));
    assert_eq!(
        lauf(&start("pi", None), &pi),
        (Some("off".to_owned()), Ok(()))
    );
    let opencode = einst(Some(json!([
        {"harness": "opencode", "erlaubt": ["none", "high"], "vorgabe": "high"}
    ])));
    assert_eq!(
        lauf(&start("opencode", None), &opencode),
        (Some("high".to_owned()), Ok(()))
    );
    // the default of another harness or of another class does not apply
    let fremd = einst(Some(json!([
        {"harness": "pi", "klasse": "a", "erlaubt": ["low"], "vorgabe": "low"},
        {"harness": "copilot", "erlaubt": ["none", "low"], "vorgabe": "low"}
    ])));
    assert_eq!(lauf(&start("pi", None), &fremd), (None, Ok(())));
    assert_eq!(lauf(&start("opencode", None), &fremd), (None, Ok(())));
}

#[test]
fn claude_without_an_effort_keeps_no_option() {
    assert_eq!(lauf(&start("claude", None), &einst(None)), (None, Ok(())));
}
