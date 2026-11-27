//! Outside tests of version 2 of contract 1 (addenda of 26 and 27 November 2026): `effort`, `name` and
//! `fortsetzung_von`, the writer in the version it read, and the service API `mit_sitzung` and `mit_fortsetzung`.

use einstellungen::lesen as lesen_einstellungen;
use serde_json::{Value, json};
use start::{
    Fehler, Marker, Startanfrage, Startform, lesen_anfrage, lesen_datei, pruefen, schreiben,
};

fn anfrage(wert: &Value) -> Startanfrage {
    lesen_anfrage(wert.to_string().as_bytes()).expect("request valid")
}

fn zwei() -> Value {
    json!({"fassung": 2, "harness": "copilot", "modell": "auto", "route": "cloud", "kunde": null,
        "arbeitsordner": "/heim/beispiel/projekt", "effort": "medium", "name": "Auswertung \u{c4}"})
}

fn eins() -> Value {
    json!({"fassung": 1, "harness": "copilot", "modell": "auto", "route": "cloud", "kunde": "k",
        "arbeitsordner": "/heim/beispiel/projekt"})
}

#[test]
fn mit_sitzung_of_version_2_writes_fortsetzung_von_as_null() {
    let datei = anfrage(&zwei())
        .mit_sitzung("s-2", None)
        .expect("key valid");
    assert_eq!(datei.fortsetzung_von(), None);
    assert_eq!(datei.fassung(), 2);
    assert_eq!(datei.effort(), Some("medium"));
    assert_eq!(datei.name(), Some("Auswertung \u{c4}"));
    let text = String::from_utf8(schreiben(&datei)).expect("UTF-8");
    assert_eq!(
        text,
        "{\"fassung\":2,\"sitzung\":\"s-2\",\"harness\":\"copilot\",\"modell\":\"auto\",\"route\":\"cloud\",\
         \"kunde\":null,\"arbeitsordner\":\"/heim/beispiel/projekt\",\"effort\":\"medium\",\
         \"name\":\"Auswertung \u{c4}\",\"fortsetzung_von\":null}"
    );
    assert_eq!(lesen_datei(text.as_bytes()), Ok(datei));
}

#[test]
fn mit_fortsetzung_writes_the_old_key_and_reads_back() {
    let marker = Marker::neu(vec!["M-1".to_owned()], "/heim/beispiel/zaehlung").expect("marker");
    let datei = anfrage(&zwei())
        .mit_fortsetzung("s-4", Some(marker), Some("s-1"))
        .expect("valid");
    assert_eq!(datei.fortsetzung_von(), Some("s-1"));
    assert_eq!(datei.sitzung(), "s-4");
    let text = String::from_utf8(schreiben(&datei)).expect("UTF-8");
    assert!(
        text.ends_with(
            ",\"fortsetzung_von\":\"s-1\",\"marker\":{\"liste\":[\"M-1\"],\"ordner\":\"/heim/beispiel/zaehlung\"}}"
        ),
        "{text}"
    );
    assert_eq!(lesen_datei(text.as_bytes()), Ok(datei));
}

#[test]
fn mit_fortsetzung_without_an_old_key_equals_mit_sitzung() {
    assert_eq!(
        anfrage(&zwei()).mit_fortsetzung("s-2", None, None),
        anfrage(&zwei()).mit_sitzung("s-2", None)
    );
    assert_eq!(
        anfrage(&eins()).mit_fortsetzung("s-2", None, None),
        anfrage(&eins()).mit_sitzung("s-2", None)
    );
}

#[test]
fn mit_fortsetzung_refuses_a_wrong_key_its_own_key_and_version_1() {
    let a = || anfrage(&zwei());
    assert_eq!(
        a().mit_fortsetzung("S-4", None, Some("s-1")),
        Err(Fehler::KennungForm)
    );
    assert_eq!(
        a().mit_fortsetzung("s-4", None, Some("S-1")),
        Err(Fehler::KennungForm)
    );
    assert_eq!(
        a().mit_fortsetzung("s-4", None, Some("")),
        Err(Fehler::KennungForm)
    );
    assert_eq!(
        a().mit_fortsetzung("s-4", None, Some("../s-1")),
        Err(Fehler::KennungForm)
    );
    assert_eq!(
        a().mit_fortsetzung("s-4", None, Some("s-4")),
        Err(Fehler::FortsetzungSelbst)
    );
    assert_eq!(
        anfrage(&eins()).mit_fortsetzung("s-4", None, Some("s-1")),
        Err(Fehler::SchluesselUnbekannt)
    );
    assert_eq!(a().mit_sitzung("S-4", None), Err(Fehler::KennungForm));
}

#[test]
fn version_1_is_written_unchanged_without_the_fields_of_version_2() {
    let datei = anfrage(&eins()).mit_sitzung("s-1", None).expect("valid");
    assert_eq!(datei.fassung(), 1);
    assert_eq!(datei.effort(), None);
    assert_eq!(datei.name(), None);
    assert_eq!(datei.fortsetzung_von(), None);
    assert_eq!(
        String::from_utf8(schreiben(&datei)).expect("UTF-8"),
        "{\"fassung\":1,\"sitzung\":\"s-1\",\"harness\":\"copilot\",\"modell\":\"auto\",\"route\":\"cloud\",\
         \"kunde\":\"k\",\"arbeitsordner\":\"/heim/beispiel/projekt\"}"
    );
}

#[test]
fn version_2_with_null_fields_is_written_with_null() {
    let mut wert = zwei();
    wert["effort"] = Value::Null;
    wert["name"] = Value::Null;
    let datei = anfrage(&wert).mit_sitzung("s-3", None).expect("valid");
    let text = String::from_utf8(schreiben(&datei)).expect("UTF-8");
    assert!(
        text.ends_with(",\"effort\":null,\"name\":null,\"fortsetzung_von\":null}"),
        "{text}"
    );
    assert_eq!(lesen_datei(text.as_bytes()), Ok(datei));
}

#[test]
fn the_limit_of_stage_18_follows_the_settings_file() {
    let einst = lesen_einstellungen(
        json!({"fassung": 2, "klassen": ["a", "b"],
            "routen": [{"schluessel": "cloud", "art": "cloud", "klasse": "a", "netzziele": []},
                       {"schluessel": "lokal", "art": "lokal", "klasse": "b",
                        "ziel": "http://127.0.0.1:11434/v1", "modell": "qwen3:8b", "netzziele": []}],
            "kunden": [{"schluessel": "k"}],
            "kundendaten_an_cloud": {"vorgabe": "nie", "freigaben": []},
            "effort": {"grenzen": [{"harness": "copilot", "erlaubt": ["low", "medium"]},
                                   {"harness": "pi", "klasse": "b", "modell": "qwen3:8b", "erlaubt": ["off"]}]}})
        .to_string()
        .as_bytes(),
    )
    .expect("settings valid");
    let code = |aendern: &dyn Fn(&mut Value)| {
        let mut wert = zwei();
        aendern(&mut wert);
        match lesen_anfrage(wert.to_string().as_bytes()).and_then(|a| pruefen(&a, &einst)) {
            Ok(art) => art.kennung().to_owned(),
            Err(fehler) => fehler.code().to_owned(),
        }
    };
    assert_eq!(code(&|_| {}), "cloud");
    assert_eq!(code(&|w| w["effort"] = json!("low")), "cloud");
    assert_eq!(
        code(&|w| w["effort"] = json!("high")),
        "start.effort_grenze"
    );
    assert_eq!(code(&|w| w["effort"] = Value::Null), "cloud");
    assert_eq!(
        code(&|w| w["harness"] = json!("claude")),
        "start.effort_grenze"
    );
    let pi = |w: &mut Value| {
        w["harness"] = json!("pi");
        w["route"] = json!("lokal");
        w["modell"] = json!("qwen3:8b");
    };
    assert_eq!(
        code(&|w| {
            pi(w);
            w["effort"] = json!("off");
        }),
        "lokal"
    );
    assert_eq!(
        code(&|w| {
            pi(w);
            w["effort"] = json!("low");
        }),
        "start.effort_grenze"
    );
}

#[test]
fn fassung_2_as_a_number_with_a_fraction_is_a_wrong_type() {
    let mut wert = zwei();
    wert["fassung"] = json!(2.0);
    assert_eq!(
        lesen_anfrage(wert.to_string().as_bytes()).err(),
        Some(Fehler::SchluesselUnbekannt)
    );
    let mut ohne = eins();
    ohne["fassung"] = json!(2.0);
    assert_eq!(
        lesen_anfrage(ohne.to_string().as_bytes()).err(),
        Some(Fehler::TypFalsch)
    );
}

#[test]
fn the_request_hands_out_the_fields_of_its_version() {
    let a = anfrage(&zwei());
    assert_eq!(a.fassung(), 2);
    assert_eq!(a.effort(), Some("medium"));
    assert_eq!(a.name(), Some("Auswertung \u{c4}"));
    let mut ohne = zwei();
    ohne["effort"] = Value::Null;
    ohne["name"] = Value::Null;
    let b = anfrage(&ohne);
    assert_eq!((b.effort(), b.name()), (None, None));
    let c = anfrage(&eins());
    assert_eq!((c.fassung(), c.effort(), c.name()), (1, None, None));
    let datei = anfrage(&eins()).mit_sitzung("s-1", None).expect("valid");
    assert_eq!(
        (datei.fassung(), datei.effort(), datei.name()),
        (1, None, None)
    );
}
