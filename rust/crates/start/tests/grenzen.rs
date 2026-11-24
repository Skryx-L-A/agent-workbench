//! Outside tests of the limits of contract 1 (addendum of 20 November): markers, the model of the
//! route, the harness table, the writer and the nesting limit.

#![allow(clippy::needless_pass_by_value)]

use einstellungen::lesen as lesen_einstellungen;
use serde_json::{Value, json};
use start::{Fehler, Marker, Routenart, Startform, lesen_anfrage, lesen_datei, pruefen, schreiben};

fn einst_zwei() -> Value {
    json!({
        "fassung": 2,
        "klassen": ["a", "b"],
        "routen": [
            {"schluessel": "cloud-fest", "art": "cloud", "klasse": "a", "modell": "claude-sonnet-5.5",
             "netzziele": []},
            {"schluessel": "cloud-frei", "art": "cloud", "klasse": "a", "netzziele": []},
            {"schluessel": "lokal", "art": "lokal", "klasse": "b",
             "ziel": "http://127.0.0.1:11434/v1", "modell": "qwen3:8b", "netzziele": []}
        ],
        "kunden": [{"schluessel": "k"}],
        "kundendaten_an_cloud": {"vorgabe": "nie", "freigaben": []}
    })
}

fn einst_eins() -> Value {
    json!({
        "fassung": 1,
        "klassen": ["a", "b"],
        "routen": [
            {"schluessel": "cloud-fest", "art": "cloud", "klasse": "a"},
            {"schluessel": "lokal", "art": "lokal", "klasse": "b"}
        ],
        "kunden": [{"schluessel": "k"}],
        "kundendaten_an_cloud": {"vorgabe": "nie", "freigaben": []}
    })
}

fn start_datei(harness: &str, modell: &str, route: &str, kunde: Value) -> Value {
    json!({"fassung": 1, "sitzung": "s-1", "harness": harness, "modell": modell, "route": route,
        "kunde": kunde, "arbeitsordner": "/srv/beispiel/projekt"})
}

fn pruef_code(einst: &Value, start: &Value) -> String {
    let einst = lesen_einstellungen(einst.to_string().as_bytes()).expect("settings valid");
    match lesen_datei(start.to_string().as_bytes()).and_then(|datei| pruefen(&datei, &einst)) {
        Ok(art) => format!("ok:{}", art.kennung()),
        Err(fehler) => fehler.code().to_owned(),
    }
}

fn datei_code(wert: &Value) -> &'static str {
    match lesen_datei(wert.to_string().as_bytes()) {
        Ok(_) => "ok",
        Err(fehler) => fehler.code(),
    }
}

fn anfrage_code(wert: &Value) -> &'static str {
    match lesen_anfrage(wert.to_string().as_bytes()) {
        Ok(_) => "ok",
        Err(fehler) => fehler.code(),
    }
}

fn mit_marker(marker: Value) -> &'static str {
    let mut datei = start_datei("copilot", "m", "r", Value::Null);
    datei["marker"] = marker;
    datei_code(&datei)
}

#[test]
fn nesting_beyond_127_levels_is_refused_as_invalid_json() {
    let tief = |ebenen: usize| format!("{{\"x\":{}{}}}", "[".repeat(ebenen), "]".repeat(ebenen));
    let code = |text: &str| lesen_datei(text.as_bytes()).map_or_else(|f| f.code(), |_| "ok");
    assert_eq!(code(&tief(126)), "start.schluessel_unbekannt");
    assert_eq!(code(&tief(127)), "start.json_ungueltig");
    assert_eq!(code(&tief(1000)), "start.json_ungueltig");
}

#[test]
fn the_harness_table_has_claude_copilot_and_pi() {
    let einst = einst_zwei();
    assert_eq!(
        pruef_code(
            &einst,
            &start_datei("claude", "claude-sonnet-5.5", "cloud-fest", json!("k"))
        ),
        "ok:cloud"
    );
    assert_eq!(
        pruef_code(
            &einst,
            &start_datei("copilot", "claude-sonnet-5.5", "cloud-fest", json!("k"))
        ),
        "ok:cloud"
    );
    assert_eq!(
        pruef_code(&einst, &start_datei("pi", "qwen3:8b", "lokal", Value::Null)),
        "ok:lokal"
    );
    assert_eq!(
        pruef_code(
            &einst,
            &start_datei("pi", "claude-sonnet-5.5", "cloud-fest", Value::Null)
        ),
        "start.harness_route_unvertraeglich"
    );
    assert_eq!(
        pruef_code(
            &einst,
            &start_datei("copilot", "qwen3:8b", "lokal", Value::Null)
        ),
        "start.harness_route_unvertraeglich"
    );
    assert_eq!(
        pruef_code(
            &einst,
            &start_datei("claude", "qwen3:8b", "lokal", Value::Null)
        ),
        "start.harness_route_unvertraeglich"
    );
    assert_eq!(
        datei_code(&start_datei("codex", "m", "r", Value::Null)),
        "start.harness_unbekannt"
    );
    assert_eq!(
        datei_code(&start_datei("Copilot", "m", "r", Value::Null)),
        "start.kennung_form"
    );
}

#[test]
fn the_model_of_the_route_must_equal_the_model_of_the_start() {
    let einst = einst_zwei();
    // A cloud route with a model fixes it; a cloud route without one lets the harness choose.
    assert_eq!(
        pruef_code(
            &einst,
            &start_datei("copilot", "claude-sonnet-5.5", "cloud-fest", Value::Null)
        ),
        "ok:cloud"
    );
    assert_eq!(
        pruef_code(
            &einst,
            &start_datei("copilot", "claude-sonnet-5.6", "cloud-fest", Value::Null)
        ),
        "start.modell_route"
    );
    assert_eq!(
        pruef_code(
            &einst,
            &start_datei("copilot", "Claude-Sonnet-5.5", "cloud-fest", Value::Null)
        ),
        "start.modell_route"
    );
    assert_eq!(
        pruef_code(
            &einst,
            &start_datei("copilot", "claude-sonnet-5.5 ", "cloud-fest", Value::Null)
        ),
        "start.modell_form"
    );
    assert_eq!(
        pruef_code(
            &einst,
            &start_datei("copilot", "gpt-5.6-sol", "cloud-frei", Value::Null)
        ),
        "ok:cloud"
    );
    assert_eq!(
        pruef_code(&einst, &start_datei("pi", "qwen3:8b", "lokal", Value::Null)),
        "ok:lokal"
    );
    assert_eq!(
        pruef_code(&einst, &start_datei("pi", "qwen3:4b", "lokal", Value::Null)),
        "start.modell_route"
    );
    // Settings of version 1 carry no model, so the stage never applies.
    let eins = einst_eins();
    assert_eq!(
        pruef_code(
            &eins,
            &start_datei("claude", "anything", "cloud-fest", Value::Null)
        ),
        "ok:cloud"
    );
    assert_eq!(
        pruef_code(&eins, &start_datei("pi", "anything", "lokal", Value::Null)),
        "ok:lokal"
    );
}

#[test]
fn the_stages_14_to_17_run_in_the_order_of_the_contract() {
    let einst = einst_zwei();
    // route unknown (14) before model (15)
    assert_eq!(
        pruef_code(
            &einst,
            &start_datei("pi", "x", "gibt-es-nicht", json!("nicht-da"))
        ),
        "start.route_unbekannt"
    );
    // model (15) before customer (16)
    assert_eq!(
        pruef_code(&einst, &start_datei("pi", "x", "lokal", json!("nicht-da"))),
        "start.modell_route"
    );
    // customer (16) before harness against route (17)
    assert_eq!(
        pruef_code(
            &einst,
            &start_datei("claude", "qwen3:8b", "lokal", json!("nicht-da"))
        ),
        "start.kunde_unbekannt"
    );
    // model (15) before harness against route (17)
    assert_eq!(
        pruef_code(&einst, &start_datei("claude", "x", "lokal", Value::Null)),
        "start.modell_route"
    );
    // A request is checked the same way as a file.
    let anfrage = json!({"fassung": 1, "harness": "pi", "modell": "x", "route": "lokal", "kunde": null,
        "arbeitsordner": "/srv/beispiel/projekt"});
    let einst = lesen_lokal(&einst);
    let gelesen = lesen_anfrage(anfrage.to_string().as_bytes()).expect("request valid");
    assert_eq!(
        pruefen(&gelesen, &einst).map(Routenart::kennung),
        Err(Fehler::ModellRoute)
    );
}

fn lesen_lokal(einst: &Value) -> einstellungen::Einstellungen {
    lesen_einstellungen(einst.to_string().as_bytes()).expect("settings valid")
}

#[test]
fn the_markers_follow_the_form() {
    let gut = json!({"liste": ["A"], "ordner": "/srv/beispiel/zaehlung"});
    assert_eq!(mit_marker(gut), "ok");
    assert_eq!(
        mit_marker(json!({"liste": ["ABC-123", "X"], "ordner": "/z"})),
        "ok"
    );
    let sechzehn: Vec<String> = (0..16).map(|n| format!("M{n}")).collect();
    assert_eq!(mit_marker(json!({"liste": sechzehn, "ordner": "/z"})), "ok");
    let siebzehn: Vec<String> = (0..17).map(|n| format!("M{n}")).collect();
    assert_eq!(
        mit_marker(json!({"liste": siebzehn, "ordner": "/z"})),
        "start.marker_form"
    );
    assert_eq!(
        mit_marker(json!({"liste": [], "ordner": "/z"})),
        "start.marker_form"
    );
    assert_eq!(
        mit_marker(json!({"liste": ["A".repeat(32)], "ordner": "/z"})),
        "ok"
    );
    assert_eq!(
        mit_marker(json!({"liste": ["A".repeat(33)], "ordner": "/z"})),
        "start.marker_form"
    );
    assert_eq!(
        mit_marker(json!({"liste": [""], "ordner": "/z"})),
        "start.marker_form"
    );
    for schlecht in ["a", "A,B", "A B", "A_B", "Ä", "A\n"] {
        assert_eq!(
            mit_marker(json!({"liste": [schlecht], "ordner": "/z"})),
            "start.marker_form",
            "{schlecht}"
        );
    }
    assert_eq!(
        mit_marker(json!({"liste": ["A", "A"], "ordner": "/z"})),
        "start.marker_form"
    );
    assert_eq!(
        mit_marker(json!({"liste": ["A", "B", "A"], "ordner": "/z"})),
        "start.marker_form"
    );
    for schlecht in [
        "", "/", "z", "/z/", "/z//y", "/z/./y", "/z/../y", "/z\u{85}",
    ] {
        assert_eq!(
            mit_marker(json!({"liste": ["A"], "ordner": schlecht})),
            "start.marker_form",
            "{schlecht:?}"
        );
    }
    assert_eq!(
        mit_marker(json!({"liste": ["A"], "ordner": format!("/{}", "a".repeat(1024))})),
        "start.marker_form"
    );
    assert_eq!(
        mit_marker(json!({"liste": ["A"], "ordner": format!("/{}", "a".repeat(1023))})),
        "ok"
    );
}

#[test]
fn the_marker_object_is_closed_and_typed() {
    assert_eq!(
        mit_marker(json!({"liste": ["A"]})),
        "start.schluessel_fehlt"
    );
    assert_eq!(
        mit_marker(json!({"ordner": "/z"})),
        "start.schluessel_fehlt"
    );
    assert_eq!(mit_marker(json!({})), "start.schluessel_fehlt");
    assert_eq!(
        mit_marker(json!({"liste": ["A"], "ordner": "/z", "x": 1})),
        "start.schluessel_unbekannt"
    );
    assert_eq!(mit_marker(json!(null)), "start.typ_falsch");
    assert_eq!(mit_marker(json!("A")), "start.typ_falsch");
    assert_eq!(mit_marker(json!(["A"])), "start.typ_falsch");
    assert_eq!(
        mit_marker(json!({"liste": "A", "ordner": "/z"})),
        "start.typ_falsch"
    );
    assert_eq!(
        mit_marker(json!({"liste": [1], "ordner": "/z"})),
        "start.typ_falsch"
    );
    assert_eq!(
        mit_marker(json!({"liste": ["A"], "ordner": 1})),
        "start.typ_falsch"
    );
    assert_eq!(
        mit_marker(json!({"liste": ["A"], "ordner": null})),
        "start.typ_falsch"
    );
}

#[test]
fn a_request_with_a_marker_or_a_session_is_refused() {
    let anfrage = json!({"fassung": 1, "harness": "pi", "modell": "m", "route": "r", "kunde": null,
        "arbeitsordner": "/srv/beispiel/projekt"});
    assert_eq!(anfrage_code(&anfrage), "ok");
    let mut mit = anfrage.clone();
    mit["marker"] = json!({"liste": ["A"], "ordner": "/z"});
    assert_eq!(anfrage_code(&mit), "start.marker_nicht_erlaubt");
    let mut mit = anfrage.clone();
    mit["marker"] = json!(null);
    assert_eq!(anfrage_code(&mit), "start.marker_nicht_erlaubt");
    let mut mit = anfrage.clone();
    mit["sitzung"] = json!("s-1");
    assert_eq!(anfrage_code(&mit), "start.sitzung_nicht_erlaubt");
    // The session (3) comes before the marker (4).
    mit["marker"] = json!({"liste": ["A"], "ordner": "/z"});
    assert_eq!(anfrage_code(&mit), "start.sitzung_nicht_erlaubt");
    // A malformed marker in a request still counts as not allowed, not as a form error.
    let mut mit = anfrage;
    mit["marker"] = json!({"liste": [], "ordner": "x"});
    assert_eq!(anfrage_code(&mit), "start.marker_nicht_erlaubt");
}

#[test]
fn the_service_sets_the_session_and_the_markers() {
    let anfrage = json!({"fassung": 1, "harness": "copilot", "modell": "m", "route": "r", "kunde": "k",
        "arbeitsordner": "/srv/beispiel/projekt"});
    let lesen = || lesen_anfrage(anfrage.to_string().as_bytes()).expect("request valid");
    let ohne = lesen().mit_sitzung("s-7", None).expect("session valid");
    assert_eq!(ohne.sitzung(), "s-7");
    assert!(ohne.marker().is_none());
    assert_eq!(ohne.harness(), "copilot");
    assert_eq!(ohne.kunde(), Some("k"));
    let marker = Marker::neu(
        vec!["A".to_owned(), "B-2".to_owned()],
        "/srv/beispiel/zaehlung",
    )
    .expect("form");
    assert_eq!(marker.liste(), ["A", "B-2"]);
    assert_eq!(marker.ordner(), "/srv/beispiel/zaehlung");
    let mit = lesen()
        .mit_sitzung("s-8", Some(marker.clone()))
        .expect("session valid");
    assert_eq!(mit.marker(), Some(&marker));
    assert_eq!(lesen().mit_sitzung("S-1", None), Err(Fehler::KennungForm));
    assert_eq!(lesen().mit_sitzung("", None), Err(Fehler::KennungForm));
    assert_eq!(Marker::neu(vec![], "/z"), Err(Fehler::MarkerForm));
    assert_eq!(
        Marker::neu(vec!["a".to_owned()], "/z"),
        Err(Fehler::MarkerForm)
    );
    assert_eq!(
        Marker::neu(vec!["A".to_owned()], "z"),
        Err(Fehler::MarkerForm)
    );
    assert_eq!(
        Marker::neu(vec!["A".to_owned(), "A".to_owned()], "/z"),
        Err(Fehler::MarkerForm)
    );
}

#[test]
fn a_written_start_file_with_markers_reads_back_unchanged() {
    let anfrage = json!({"fassung": 1, "harness": "pi", "modell": "qwen3:8b", "route": "lokal", "kunde": null,
        "arbeitsordner": "/srv/beispiel/pro\"jekt\\x"});
    let marker = Marker::neu(
        vec!["A".to_owned(), "B".to_owned()],
        "/srv/beispiel/zaehlung",
    )
    .expect("form");
    let datei = lesen_anfrage(anfrage.to_string().as_bytes())
        .expect("request valid")
        .mit_sitzung("s-2", Some(marker))
        .expect("session valid");
    let bytes = schreiben(&datei);
    let text = String::from_utf8(bytes.clone()).expect("UTF-8");
    assert!(
        text.contains("\"marker\":{\"liste\":[\"A\",\"B\"],\"ordner\":\"/srv/beispiel/zaehlung\"}"),
        "{text}"
    );
    assert_eq!(lesen_datei(&bytes), Ok(datei.clone()));
    let ohne = lesen_anfrage(anfrage.to_string().as_bytes())
        .expect("request valid")
        .mit_sitzung("s-3", None)
        .expect("session valid");
    let text = String::from_utf8(schreiben(&ohne)).expect("UTF-8");
    assert!(!text.contains("marker"), "{text}");
    assert_eq!(lesen_datei(text.as_bytes()), Ok(ohne));
}

#[test]
fn the_request_gives_read_access_to_every_part() {
    let anfrage = json!({"fassung": 1, "harness": "pi", "modell": "qwen3:8b", "route": "lokal", "kunde": "k",
        "arbeitsordner": "/srv/beispiel/projekt"});
    let gelesen = lesen_anfrage(anfrage.to_string().as_bytes()).expect("request valid");
    assert_eq!(gelesen.harness(), "pi");
    assert_eq!(gelesen.modell(), "qwen3:8b");
    assert_eq!(gelesen.route(), "lokal");
    assert_eq!(gelesen.kunde(), Some("k"));
    assert_eq!(gelesen.arbeitsordner(), "/srv/beispiel/projekt");
    let datei = gelesen.mit_sitzung("s-1", None).expect("session valid");
    assert_eq!(datei.modell(), "qwen3:8b");
    assert_eq!(datei.route(), "lokal");
    assert_eq!(datei.arbeitsordner(), "/srv/beispiel/projekt");
}
