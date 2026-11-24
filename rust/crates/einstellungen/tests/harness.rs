//! Outside tests of the block `harness` of contract 2 (version 2): the typed view, the closed set of
//! values and the place of stage 34 among the other stages.
//!
//! The case files of `rust/spec/einstellungen` fix the codes of single files; these tests pin what the
//! reader hands to `Lauf starten`: which kinds, in which order, and that nothing else gets through.

#![allow(clippy::needless_pass_by_value)]

use einstellungen::{Werkzeug, lesen};
use serde_json::{Value, json};

/// A valid file of version 2 with one cloud route and no block `harness`.
fn basis() -> Value {
    json!({
        "fassung": 2,
        "klassen": ["a", "b"],
        "routen": [
            {"schluessel": "c", "art": "cloud", "klasse": "a", "netzziele": []}
        ],
        "kunden": [],
        "kundendaten_an_cloud": {"vorgabe": "nie", "freigaben": []}
    })
}

fn mit_harness(block: Value) -> Value {
    let mut datei = basis();
    datei["harness"] = block;
    datei
}

fn mit_werten(werte: Value) -> Value {
    mit_harness(json!({"copilot": {"werkzeuge_ohne_rueckfrage": werte}}))
}

fn code(wert: &Value) -> &'static str {
    match lesen(wert.to_string().as_bytes()) {
        Ok(_) => "ok",
        Err(fehler) => fehler.code(),
    }
}

fn gelesen(wert: &Value) -> Vec<Werkzeug> {
    lesen(wert.to_string().as_bytes())
        .expect("the file is valid")
        .copilot_ohne_rueckfrage()
        .to_vec()
}

#[test]
fn without_the_block_the_question_stays() {
    assert_eq!(gelesen(&basis()), Vec::<Werkzeug>::new());
}

#[test]
fn an_empty_block_and_an_empty_list_keep_the_question() {
    assert_eq!(gelesen(&mit_harness(json!({}))), Vec::<Werkzeug>::new());
    assert_eq!(gelesen(&mit_werten(json!([]))), Vec::<Werkzeug>::new());
}

#[test]
fn the_values_are_kept_in_the_order_of_the_file() {
    assert_eq!(
        gelesen(&mit_werten(json!(["shell", "write", "such-mcp"]))),
        [Werkzeug::Shell, Werkzeug::Write, Werkzeug::SuchMcp]
    );
    assert_eq!(
        gelesen(&mit_werten(json!(["such-mcp", "shell"]))),
        [Werkzeug::SuchMcp, Werkzeug::Shell]
    );
    assert_eq!(gelesen(&mit_werten(json!(["write"]))), [Werkzeug::Write]);
}

#[test]
fn every_kind_has_exactly_the_value_of_the_file() {
    for (werkzeug, wert) in [
        (Werkzeug::Shell, "shell"),
        (Werkzeug::Write, "write"),
        (Werkzeug::SuchMcp, "such-mcp"),
    ] {
        assert_eq!(werkzeug.wert(), wert);
        assert_eq!(Werkzeug::aus_wert(wert), Some(werkzeug));
        assert_eq!(gelesen(&mit_werten(json!([wert]))), [werkzeug]);
    }
}

#[test]
fn no_other_text_is_a_kind() {
    for wert in [
        "allow-all-tools",
        "allow-all-paths",
        "allow-all-urls",
        "allow-all",
        "yolo",
        "Shell",
        "SHELL",
        "WRITE",
        "such_mcp",
        "Such-MCP",
        "",
        " shell",
        "shell ",
        "shell\n",
        "shell(git:*)",
        "*",
        "--allow-all-tools",
        "read",
        "url",
        "memory",
    ] {
        assert_eq!(Werkzeug::aus_wert(wert), None, "{wert:?}");
        assert_eq!(
            code(&mit_werten(json!([wert]))),
            "einst.werkzeug_wert",
            "{wert:?}"
        );
    }
}

#[test]
fn one_wrong_value_among_valid_ones_refuses_the_file() {
    for liste in [
        json!(["yolo", "shell"]),
        json!(["shell", "yolo"]),
        json!(["shell", "write", "allow-all", "such-mcp"]),
    ] {
        assert_eq!(code(&mit_werten(liste)), "einst.werkzeug_wert");
    }
}

#[test]
fn the_block_exists_only_in_version_2() {
    let mut datei = json!({
        "fassung": 1,
        "klassen": ["a"],
        "routen": [{"schluessel": "c", "art": "cloud", "klasse": "a"}],
        "kunden": [],
        "kundendaten_an_cloud": {"vorgabe": "nie", "freigaben": []}
    });
    assert_eq!(code(&datei), "ok");
    datei["harness"] = json!({"copilot": {"werkzeuge_ohne_rueckfrage": ["shell"]}});
    assert_eq!(code(&datei), "einst.schluessel_unbekannt");
}

#[test]
fn the_form_is_closed_at_every_level() {
    assert_eq!(
        code(&mit_harness(
            json!({"claude": {"werkzeuge_ohne_rueckfrage": []}})
        )),
        "einst.schluessel_unbekannt"
    );
    assert_eq!(
        code(&mit_harness(json!({"pi": {}}))),
        "einst.schluessel_unbekannt"
    );
    assert_eq!(
        code(&mit_harness(
            json!({"copilot": {"werkzeuge_ohne_rueckfrage": [], "alles": true}})
        )),
        "einst.schluessel_unbekannt"
    );
    let mut datei = mit_werten(json!([]));
    datei["harness2"] = json!({});
    assert_eq!(code(&datei), "einst.schluessel_unbekannt");
}

#[test]
fn the_list_is_required_in_the_entry() {
    assert_eq!(
        code(&mit_harness(json!({"copilot": {}}))),
        "einst.schluessel_fehlt"
    );
}

#[test]
fn wrong_json_types_are_stage_6() {
    for block in [
        json!([]),
        json!("copilot"),
        json!(null),
        json!({"copilot": "shell"}),
        json!({"copilot": []}),
        json!({"copilot": null}),
        json!({"copilot": {"werkzeuge_ohne_rueckfrage": "shell"}}),
        json!({"copilot": {"werkzeuge_ohne_rueckfrage": null}}),
        json!({"copilot": {"werkzeuge_ohne_rueckfrage": {}}}),
        json!({"copilot": {"werkzeuge_ohne_rueckfrage": [1]}}),
        json!({"copilot": {"werkzeuge_ohne_rueckfrage": [null]}}),
        json!({"copilot": {"werkzeuge_ohne_rueckfrage": [true]}}),
        json!({"copilot": {"werkzeuge_ohne_rueckfrage": [["shell"]]}}),
    ] {
        assert_eq!(
            code(&mit_harness(block.clone())),
            "einst.typ_falsch",
            "{block}"
        );
    }
}

#[test]
fn a_duplicate_is_stage_10_and_a_wrong_value_stays_stage_34() {
    assert_eq!(
        code(&mit_werten(json!(["shell", "shell"]))),
        "einst.kennung_doppelt"
    );
    assert_eq!(
        code(&mit_werten(json!(["yolo", "yolo"]))),
        "einst.kennung_doppelt"
    );
    assert_eq!(
        code(&mit_werten(json!(["shell", "write", "shell"]))),
        "einst.kennung_doppelt"
    );
    assert_eq!(
        code(&mit_werten(json!(["Shell", "shell"]))),
        "einst.werkzeug_wert"
    );
}

#[test]
fn stage_34_comes_after_every_other_stage() {
    // stage 6 before 34
    assert_eq!(code(&mit_werten(json!(["yolo", 1]))), "einst.typ_falsch");
    // stage 25 (network target) before 34
    let mut datei = mit_werten(json!(["yolo"]));
    datei["routen"][0]["netzziele"] =
        json!([{"host": "localhost", "port": 443, "zugriff": "verbinden"}]);
    assert_eq!(code(&datei), "einst.netzziel_form");
    // stage 32 and 33 (block mcp) before 34
    let mut datei = mit_werten(json!(["yolo"]));
    datei["mcp"] = json!({"t": {"lesen": [], "schreiben": [], "frist_ms": 0}});
    assert_eq!(code(&datei), "einst.frist_bereich");
    let mut datei = mit_werten(json!(["yolo"]));
    datei["mcp"] = json!({"t": {"lesen": ["x"], "schreiben": ["x"]}});
    assert_eq!(code(&datei), "einst.mcp_doppelt");
}

#[test]
fn the_block_and_the_block_mcp_work_together() {
    let mut datei = mit_werten(json!(["such-mcp"]));
    datei["mcp"] = json!({"such-mcp": {"lesen": ["brain_suche"], "schreiben": []}});
    assert_eq!(gelesen(&datei), [Werkzeug::SuchMcp]);
}

#[test]
fn a_repeated_key_in_the_block_is_invalid_json() {
    let text = r#"{"fassung":2,"klassen":["a"],"routen":[{"schluessel":"c","art":"cloud","klasse":"a","netzziele":[]}],"kunden":[],"kundendaten_an_cloud":{"vorgabe":"nie","freigaben":[]},"harness":{"copilot":{"werkzeuge_ohne_rueckfrage":[]},"copilot":{"werkzeuge_ohne_rueckfrage":["shell"]}}}"#;
    let ergebnis = lesen(text.as_bytes());
    assert_eq!(
        ergebnis.err().map(|f| f.code()),
        Some("einst.json_ungueltig")
    );
}
