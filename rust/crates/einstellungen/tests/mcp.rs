//! Outside tests of the block `mcp` of contract 2 (version 2): the typed view `Einstellungen::mcp()` and
//! `Einstellungen::mcp_server(name)`.
//!
//! The case files of `rust/spec/einstellungen` fix the codes of single files (`faelle.rs` runs the cases
//! `mcp-*`). These tests pin what the decision point (contract 6) reads: the tools of each server, the
//! defaults of the release and the two deadlines, the order of the file, and that an absent block or
//! server releases nothing (fail closed).

#![allow(clippy::needless_pass_by_value)]

use std::fs;
use std::path::Path;

use einstellungen::{Einstellungen, lesen};
use serde_json::{Value, json};

/// A valid file of version 2 with one cloud route and no block `mcp`.
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

fn mit_mcp(block: Value) -> Value {
    let mut datei = basis();
    datei["mcp"] = block;
    datei
}

fn gelesen(wert: &Value) -> Einstellungen {
    lesen(wert.to_string().as_bytes()).expect("the file is valid")
}

fn code(wert: &Value) -> &'static str {
    match lesen(wert.to_string().as_bytes()) {
        Ok(_) => "ok",
        Err(fehler) => fehler.code(),
    }
}

/// The text of a case file of contract 2.
fn fall(name: &str) -> Einstellungen {
    let pfad = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../spec/einstellungen/faelle")
        .join(format!("{name}.json"));
    let fall: Value = serde_json::from_str(&fs::read_to_string(pfad).expect("case is readable"))
        .expect("case is JSON");
    lesen(fall["text"].as_str().expect("case has text").as_bytes()).expect("the case is valid")
}

fn namen(server: &[String]) -> Vec<&str> {
    server.iter().map(String::as_str).collect()
}

#[test]
fn without_the_block_no_server_is_released() {
    for einstellungen in [gelesen(&basis()), fall("gueltig-ohne-mcp")] {
        assert_eq!(einstellungen.mcp().len(), 0);
        assert!(einstellungen.mcp_server("tickets").is_none());
        assert!(einstellungen.mcp_server("such-mcp").is_none());
        assert!(einstellungen.mcp_server("").is_none());
    }
}

#[test]
fn an_empty_block_releases_no_server_either() {
    let e = gelesen(&mit_mcp(json!({})));
    assert_eq!(e.mcp().len(), 0);
    assert!(e.mcp_server("tickets").is_none());
}

#[test]
fn the_example_of_the_contract_reads_back_with_every_field() {
    let e = fall("gueltig-mcp");
    assert_eq!(e.mcp().len(), 3);

    let tickets = e.mcp_server("tickets").expect("server tickets");
    assert_eq!(tickets.name(), "tickets");
    assert_eq!(namen(tickets.lesen()), ["get"]);
    assert_eq!(namen(tickets.schreiben()), ["update"]);
    assert!(tickets.freigabe());
    assert_eq!(tickets.frist_ms(), 60_000);
    assert_eq!(tickets.einloesefrist_ms(), 30_000);

    let notizen = e.mcp_server("notizen").expect("server notizen");
    assert_eq!(namen(notizen.lesen()).len(), 0);
    assert_eq!(namen(notizen.schreiben()), ["anlegen"]);
    assert!(!notizen.freigabe());

    let such = e.mcp_server("such-mcp").expect("server such-mcp");
    assert_eq!(namen(such.lesen()), ["brain_suche"]);
    assert_eq!(namen(such.schreiben()).len(), 0);
}

#[test]
fn the_defaults_are_a_release_60_seconds_to_decide_and_5_minutes_to_redeem() {
    let e = gelesen(&mit_mcp(json!({"s": {"lesen": [], "schreiben": ["w"]}})));
    let server = e.mcp_server("s").expect("server s");
    assert!(server.freigabe());
    assert_eq!(server.frist_ms(), 60_000);
    assert_eq!(server.einloesefrist_ms(), 300_000);
    assert_ne!(server.frist_ms(), server.einloesefrist_ms());
}

#[test]
fn each_default_applies_alone() {
    let e = gelesen(&mit_mcp(json!({
        "a": {"lesen": [], "schreiben": [], "frist_ms": 5},
        "b": {"lesen": [], "schreiben": [], "einloesefrist_ms": 7},
        "c": {"lesen": [], "schreiben": [], "freigabe": false}
    })));
    let a = e.mcp_server("a").expect("server is there");
    assert_eq!(
        (a.frist_ms(), a.einloesefrist_ms(), a.freigabe()),
        (5, 300_000, true)
    );
    let b = e.mcp_server("b").expect("server is there");
    assert_eq!(
        (b.frist_ms(), b.einloesefrist_ms(), b.freigabe()),
        (60_000, 7, true)
    );
    let c = e.mcp_server("c").expect("server is there");
    assert_eq!(
        (c.frist_ms(), c.einloesefrist_ms(), c.freigabe()),
        (60_000, 300_000, false)
    );
}

#[test]
fn the_limits_of_the_deadlines_are_read_as_written() {
    let e = fall("gueltig-mcp-fristen-grenzen");
    assert_ne!(e.mcp().len(), 0);
    let werte: Vec<(u64, u64)> = e
        .mcp()
        .iter()
        .map(|s| (s.frist_ms(), s.einloesefrist_ms()))
        .collect();
    assert!(
        werte
            .iter()
            .all(|&(a, b)| (1..=86_400_000).contains(&a) && (1..=86_400_000).contains(&b))
    );
    assert!(werte.iter().any(|&(a, b)| a == 1 || b == 1));
    assert!(
        werte
            .iter()
            .any(|&(a, b)| a == 86_400_000 || b == 86_400_000)
    );
}

#[test]
fn the_servers_keep_the_order_of_the_file() {
    // Raw text: `serde_json::Value` would sort the keys and hide the order of the file.
    let mut datei = basis().to_string();
    datei.pop();
    datei.push_str(
        r#","mcp":{"zz":{"lesen":["b","a"],"schreiben":["d","c"]},"aa":{"lesen":[],"schreiben":[]}}}"#,
    );
    let e = lesen(datei.as_bytes()).expect("the file is valid");
    let reihe: Vec<&str> = e.mcp().iter().map(einstellungen::McpServer::name).collect();
    assert_eq!(reihe, ["zz", "aa"]);
    let zz = e.mcp_server("zz").expect("server is there");
    assert_eq!(namen(zz.lesen()), ["b", "a"]);
    assert_eq!(namen(zz.schreiben()), ["d", "c"]);
}

#[test]
fn a_server_is_found_by_its_exact_key_only() {
    let e = gelesen(&mit_mcp(json!({"tickets": {"lesen": [], "schreiben": []}})));
    assert!(e.mcp_server("tickets").is_some());
    assert!(e.mcp_server("Tickets").is_none());
    assert!(e.mcp_server("ticket").is_none());
    assert!(e.mcp_server("tickets ").is_none());
}

#[test]
fn a_tool_in_no_list_is_in_neither_view() {
    let e = gelesen(&mit_mcp(json!({"s": {"lesen": ["r"], "schreiben": ["w"]}})));
    let s = e.mcp_server("s").expect("server is there");
    assert!(!s.lesen().iter().any(|t| t == "x"));
    assert!(!s.schreiben().iter().any(|t| t == "x"));
    assert!(s.lesen().iter().any(|t| t == "r"));
    assert!(s.schreiben().iter().any(|t| t == "w"));
}

#[test]
fn a_file_with_a_violating_block_gives_no_view() {
    assert_eq!(
        code(&mit_mcp(json!({"s": {"lesen": ["x"], "schreiben": ["x"]}}))),
        "einst.mcp_doppelt"
    );
    assert_eq!(
        code(&mit_mcp(
            json!({"s": {"lesen": [], "schreiben": [], "frist_ms": 0}})
        )),
        "einst.frist_bereich"
    );
    assert_eq!(
        code(&mit_mcp(
            json!({"s": {"lesen": [], "schreiben": [], "einloesefrist_ms": 86_400_001}})
        )),
        "einst.frist_bereich"
    );
    assert_eq!(
        code(&mit_mcp(
            json!({"s": {"lesen": [], "schreiben": [], "freigabe": "ja"}})
        )),
        "einst.typ_falsch"
    );
}

#[test]
fn version_one_has_no_block_and_rejects_one() {
    let mut datei = basis();
    datei["fassung"] = json!(1);
    datei["routen"] = json!([{"schluessel": "c", "art": "cloud", "klasse": "a"}]);
    assert_eq!(gelesen(&datei).mcp().len(), 0);
    datei["mcp"] = json!({"s": {"lesen": [], "schreiben": []}});
    assert_eq!(code(&datei), "einst.schluessel_unbekannt");
}
