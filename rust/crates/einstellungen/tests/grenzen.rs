//! Outside tests of the limits of contract 2 (version 2): each limit is tried on both sides.
//!
//! The case files of `rust/spec/einstellungen` fix the codes; these tests pin the edges of the forms
//! (lengths, ranges, address classes) that a single case cannot cover.

#![allow(clippy::needless_pass_by_value)]

use einstellungen::{Routenart, Schema, Zugriff, lesen};
use serde_json::{Value, json};

/// A valid file of version 2 with one cloud route `c` and one local route `l`.
fn basis() -> Value {
    json!({
        "fassung": 2,
        "klassen": ["a", "b"],
        "routen": [
            {"schluessel": "c", "art": "cloud", "klasse": "a", "netzziele": []},
            {"schluessel": "l", "art": "lokal", "klasse": "b",
             "ziel": "http://127.0.0.1:11434/v1", "modell": "m", "netzziele": []}
        ],
        "kunden": [],
        "kundendaten_an_cloud": {"vorgabe": "nie", "freigaben": []}
    })
}

fn code_text(text: &str) -> &'static str {
    match lesen(text.as_bytes()) {
        Ok(_) => "ok",
        Err(fehler) => fehler.code(),
    }
}

fn code(wert: &Value) -> &'static str {
    code_text(&wert.to_string())
}

fn mit_ziel(ziel: &str) -> &'static str {
    let mut datei = basis();
    datei["routen"][1]["ziel"] = json!(ziel);
    code(&datei)
}

fn mit_modell(modell: &str) -> &'static str {
    let mut datei = basis();
    datei["routen"][1]["modell"] = json!(modell);
    code(&datei)
}

fn mit_netzziel(ziel: Value) -> &'static str {
    let mut datei = basis();
    datei["routen"][0]["netzziele"] = json!([ziel]);
    code(&datei)
}

fn mit_host(host: &str) -> &'static str {
    mit_netzziel(json!({"host": host, "port": 443, "zugriff": "verbinden"}))
}

fn mit_pfad(methode: &str, pfad: &str) -> &'static str {
    mit_netzziel(
        json!({"host": "a.example", "port": 443, "zugriff": "kopf-einsetzen",
        "schluessel": false, "pfade": [{"methode": methode, "pfad": pfad}]}),
    )
}

#[test]
fn nesting_beyond_127_levels_is_refused_as_invalid_json() {
    // The top-level object is level 1; the deepest array of `x` decides.
    let tief = |ebenen: usize| format!("{{\"x\":{}{}}}", "[".repeat(ebenen), "]".repeat(ebenen));
    assert_eq!(code_text(&tief(126)), "einst.schluessel_unbekannt");
    assert_eq!(code_text(&tief(127)), "einst.json_ungueltig");
    assert_eq!(code_text(&tief(10_000)), "einst.json_ungueltig");
}

#[test]
fn the_base_file_is_valid() {
    assert_eq!(code(&basis()), "ok");
}

#[test]
fn the_size_limit_is_65536_bytes() {
    let mut datei = code_text(&basis().to_string());
    assert_eq!(datei, "ok");
    let text = basis().to_string();
    let fuellung = 65536 - text.len();
    datei = code_text(&format!("{text}{}", " ".repeat(fuellung)));
    assert_eq!(datei, "ok");
    assert_eq!(
        code_text(&format!("{text}{}", " ".repeat(fuellung + 1))),
        "einst.zu_gross"
    );
}

#[test]
fn the_version_word_decides_the_form_of_the_stages_3_and_5() {
    let text = basis().to_string();
    // 2.0 and 2e0 take the form of version 2, so the content is checked in that form, then the type.
    assert_eq!(
        code_text(&text.replace("\"fassung\":2", "\"fassung\":2.0")),
        "einst.typ_falsch"
    );
    assert_eq!(
        code_text(&text.replace("\"fassung\":2", "\"fassung\":2e0")),
        "einst.typ_falsch"
    );
    assert_eq!(
        code_text(&text.replace("\"fassung\":2", "\"fassung\":20e-1")),
        "einst.typ_falsch"
    );
    // 1.0 takes the form of version 1, so the keys of version 2 are unknown.
    assert_eq!(
        code_text(&text.replace("\"fassung\":2", "\"fassung\":1.0")),
        "einst.schluessel_unbekannt"
    );
    // 3.0 and 2.5 are no 2: version 1 form again.
    assert_eq!(
        code_text(&text.replace("\"fassung\":2", "\"fassung\":2.5")),
        "einst.schluessel_unbekannt"
    );
    assert_eq!(
        code_text(&text.replace("\"fassung\":2", "\"fassung\":3")),
        "einst.schluessel_unbekannt"
    );
    // 2.0 without `netzziele`: the missing key is stage 5, which beats the type of `fassung` (stage 6).
    let mut ohne = basis();
    ohne["routen"][0]
        .as_object_mut()
        .expect("object")
        .remove("netzziele");
    ohne["routen"][1]
        .as_object_mut()
        .expect("object")
        .remove("netzziele");
    assert_eq!(
        code_text(&ohne.to_string().replace("\"fassung\":2", "\"fassung\":2.0")),
        "einst.schluessel_fehlt"
    );
    assert_eq!(code(&ohne), "einst.schluessel_fehlt");
    // Version 1 accepts 1 and refuses 2's fields, version 2 accepts 2.
    assert_eq!(
        code_text(&text.replace("\"fassung\":2", "\"fassung\":0")),
        "einst.schluessel_unbekannt"
    );
    assert_eq!(
        code_text(&text.replace("\"fassung\":2", "\"fassung\":\"2\"")),
        "einst.schluessel_unbekannt"
    );
}

#[test]
fn the_target_of_a_local_route_follows_the_form() {
    for gut in [
        "http://127.0.0.1:11434/v1",
        "http://127.0.0.1:1/v1",
        "https://127.255.255.255:65535/v1",
        "https://10.0.0.1:443/v1",
        "https://10.255.255.255:443/v1",
        "https://172.16.0.1:443/v1",
        "https://172.31.255.255:443/v1",
        "https://192.168.0.1:443/v1",
        "https://192.168.255.255:443/v1",
        "https://gateway:443/v1",
        "https://llm.example:443/api/v1",
        "https://localhost:443/v1",
        "https://llm-1.example:8443/a.b/c_d~e-f/v1",
    ] {
        assert_eq!(mit_ziel(gut), "ok", "{gut}");
    }
    for (schlecht, erwartet) in [
        ("ftp://127.0.0.1:11434/v1", "einst.ziel_schema"),
        ("HTTP://127.0.0.1:11434/v1", "einst.ziel_schema"),
        ("127.0.0.1:11434/v1", "einst.ziel_schema"),
        ("", "einst.ziel_schema"),
        ("http:127.0.0.1:11434/v1", "einst.ziel_form"),
        ("http://127.0.0.1/v1", "einst.ziel_form"),
        ("http://127.0.0.1:/v1", "einst.ziel_form"),
        ("http://127.0.0.1:011434/v1", "einst.ziel_form"),
        ("http://127.0.0.1:00443/v1", "einst.ziel_form"),
        ("http://127.0.0.1:01/v1", "einst.ziel_form"),
        ("http://127.0.0.0001:11434/v1", "einst.ziel_form"),
        ("http://127.0.0.001:11434/v1", "einst.ziel_form"),
        ("http://127.0.0.:11434/v1", "einst.ziel_form"),
        ("http://127..0.1:11434/v1", "einst.ziel_form"),
        ("http://127.0.0.1:123456/v1", "einst.ziel_form"),
        ("http://127.0.0.1:11x34/v1", "einst.ziel_form"),
        ("http://127.0.0.1:0/v1", "einst.ziel_port"),
        ("http://127.0.0.1:65536/v1", "einst.ziel_port"),
        ("http://127.0.0.1:99999/v1", "einst.ziel_port"),
        ("http://127.0.0.1:11434", "einst.ziel_form"),
        ("http://127.0.0.1:11434/", "einst.ziel_form"),
        ("http://127.0.0.1:11434/v1/", "einst.ziel_form"),
        ("http://127.0.0.1:11434/v2", "einst.ziel_form"),
        ("http://127.0.0.1:11434/xv1", "einst.ziel_form"),
        ("http://127.0.0.1:11434/a/../v1", "einst.ziel_form"),
        ("http://127.0.0.1:11434/a/./v1", "einst.ziel_form"),
        ("http://127.0.0.1:11434//v1", "einst.ziel_form"),
        ("http://127.0.0.1:11434/a//v1", "einst.ziel_form"),
        ("http://127.0.0.1:11434/v1?x=1", "einst.ziel_form"),
        ("http://127.0.0.1:11434/v1#x", "einst.ziel_form"),
        ("http://nutzer@127.0.0.1:11434/v1", "einst.ziel_form"),
        ("http://[::1]:11434/v1", "einst.ziel_form"),
        ("http://127.0.0.256:11434/v1", "einst.ziel_form"),
        ("http://127.0.0.01:11434/v1", "einst.ziel_form"),
        ("http://127.0.0:11434/v1", "einst.ziel_form"),
        ("http://127.0.0.1.1:11434/v1", "einst.ziel_form"),
        ("https://Gateway:443/v1", "einst.ziel_form"),
        ("https://ä.example:443/v1", "einst.ziel_form"),
        ("https://-a.example:443/v1", "einst.ziel_form"),
        ("https://a-.example:443/v1", "einst.ziel_form"),
        ("https://a..example:443/v1", "einst.ziel_form"),
        ("https://:443/v1", "einst.ziel_form"),
        ("http://127.0.0.2:11434/v1", "einst.ziel_host"),
        ("http://localhost:11434/v1", "einst.ziel_host"),
        ("http://10.0.0.1:11434/v1", "einst.ziel_host"),
        ("http://gateway:11434/v1", "einst.ziel_host"),
        ("https://0.0.0.0:443/v1", "einst.ziel_host"),
        ("https://8.8.8.8:443/v1", "einst.ziel_host"),
        ("https://9.255.255.255:443/v1", "einst.ziel_host"),
        ("https://11.0.0.1:443/v1", "einst.ziel_host"),
        ("https://126.255.255.255:443/v1", "einst.ziel_host"),
        ("https://128.0.0.1:443/v1", "einst.ziel_host"),
        ("https://172.15.255.255:443/v1", "einst.ziel_host"),
        ("https://172.32.0.1:443/v1", "einst.ziel_host"),
        ("https://192.167.0.1:443/v1", "einst.ziel_host"),
        ("https://192.169.0.1:443/v1", "einst.ziel_host"),
        ("https://193.168.0.1:443/v1", "einst.ziel_host"),
    ] {
        assert_eq!(mit_ziel(schlecht), erwartet, "{schlecht}");
    }
}

#[test]
fn the_target_is_at_most_256_bytes_long() {
    let rumpf = "https://a.example:443/";
    let schluss = "/v1";
    let fuell = |laenge: usize| {
        format!(
            "{rumpf}{}{schluss}",
            "a".repeat(laenge - rumpf.len() - schluss.len())
        )
    };
    assert_eq!(fuell(256).len(), 256);
    assert_eq!(mit_ziel(&fuell(256)), "ok");
    assert_eq!(mit_ziel(&fuell(257)), "einst.ziel_form");
}

#[test]
fn a_target_needs_a_local_route_and_a_cloud_route_has_none() {
    let mut ohne_ziel = basis();
    ohne_ziel["routen"][1]
        .as_object_mut()
        .expect("object")
        .remove("ziel");
    assert_eq!(code(&ohne_ziel), "einst.ziel_art");
    let mut ohne_modell = basis();
    ohne_modell["routen"][1]
        .as_object_mut()
        .expect("object")
        .remove("modell");
    assert_eq!(code(&ohne_modell), "einst.ziel_art");
    let mut cloud_mit_ziel = basis();
    cloud_mit_ziel["routen"][0]["ziel"] = json!("http://127.0.0.1:11434/v1");
    assert_eq!(code(&cloud_mit_ziel), "einst.ziel_art");
    let mut ziel_zahl = basis();
    ziel_zahl["routen"][1]["ziel"] = json!(5);
    assert_eq!(code(&ziel_zahl), "einst.typ_falsch");
    let mut modell_zahl = basis();
    modell_zahl["routen"][1]["modell"] = json!(5);
    assert_eq!(code(&modell_zahl), "einst.typ_falsch");
}

#[test]
fn the_model_name_follows_length_and_character_set() {
    assert_eq!(mit_modell("a"), "ok");
    assert_eq!(mit_modell(&"a".repeat(128)), "ok");
    assert_eq!(mit_modell(&"a".repeat(129)), "einst.modell_form");
    assert_eq!(mit_modell(""), "einst.modell_form");
    assert_eq!(mit_modell("namensraum/qwen3:4b-q4_K_M"), "ok");
    assert_eq!(mit_modell("a@b+c.d"), "ok");
    for schlecht in ["a b", "a,b", "ä", "a\n", "a=b", "a*"] {
        assert_eq!(mit_modell(schlecht), "einst.modell_form", "{schlecht}");
    }
    // A cloud route may carry a model, and then it must have the form too.
    let mut cloud = basis();
    cloud["routen"][0]["modell"] = json!("claude-sonnet-5.5");
    assert_eq!(code(&cloud), "ok");
    cloud["routen"][0]["modell"] = json!("a b");
    assert_eq!(code(&cloud), "einst.modell_form");
}

#[test]
fn a_local_route_has_no_network_target() {
    let mut datei = basis();
    datei["routen"][1]["netzziele"] =
        json!([{"host": "a.example", "port": 443, "zugriff": "verbinden"}]);
    assert_eq!(code(&datei), "einst.netzziel_lokal");
}

#[test]
fn the_host_of_a_network_target_follows_the_form() {
    for gut in [
        "a.example",
        "api.github.com",
        "a-b.c-d.example",
        "a1.b2",
        "*.githubcopilot.com",
        "*.a.b",
        "xn--a.example",
    ] {
        assert_eq!(mit_host(gut), "ok", "{gut}");
    }
    let teil = "a".repeat(63);
    let lang = format!("{teil}.{teil}.{teil}.{}", "a".repeat(61));
    assert_eq!(lang.len(), 253);
    assert_eq!(mit_host(&lang), "ok");
    assert_eq!(mit_host(&format!("{lang}a")), "einst.netzziel_form");
    assert_eq!(mit_host(&format!("{}.example", "a".repeat(63))), "ok");
    assert_eq!(
        mit_host(&format!("{}.example", "a".repeat(64))),
        "einst.netzziel_form"
    );
    for schlecht in [
        "",
        "a",
        "localhost",
        "a.localhost",
        "*.localhost",
        "*.a.localhost",
        "*",
        "*.com",
        "*.",
        "a.*.com",
        "**.a.com",
        "*a.example",
        "A.example",
        "a.Example",
        "a.example.",
        ".a.example",
        "a..example",
        "-a.example",
        "a-.example",
        "a.-b",
        "127.0.0.1",
        "a.b.1",
        "a.1b",
        "a b.example",
        "ä.example",
        "a.example:443",
        "https://a.example",
        "a_b.example",
    ] {
        assert_eq!(mit_host(schlecht), "einst.netzziel_form", "{schlecht}");
    }
}

#[test]
fn the_port_of_a_network_target_is_1_to_65535() {
    let mit_port = |port: Value| {
        mit_netzziel(json!({"host": "a.example", "port": port, "zugriff": "verbinden"}))
    };
    assert_eq!(mit_port(json!(1)), "ok");
    assert_eq!(mit_port(json!(65535)), "ok");
    assert_eq!(mit_port(json!(0)), "einst.netzziel_port");
    assert_eq!(mit_port(json!(65536)), "einst.netzziel_port");
    assert_eq!(mit_port(json!(-1)), "einst.netzziel_port");
    assert_eq!(mit_port(json!(99_999_999_999_u64)), "einst.netzziel_port");
    assert_eq!(mit_port(json!("443")), "einst.typ_falsch");
    assert_eq!(mit_port(json!(443.5)), "einst.typ_falsch");
    assert_eq!(mit_port(json!(null)), "einst.typ_falsch");
    let mut text = basis();
    text["routen"][0]["netzziele"] =
        json!([{"host": "a.example", "port": 0, "zugriff": "verbinden"}]);
    let roh = text.to_string().replace("\"port\":0", "\"port\":443.0");
    assert_eq!(code_text(&roh), "einst.typ_falsch");
    let roh = text.to_string().replace("\"port\":0", "\"port\":4.43e2");
    assert_eq!(code_text(&roh), "einst.typ_falsch");
    let roh = text
        .to_string()
        .replace("\"port\":0", "\"port\":99999999999999999999999");
    assert_eq!(code_text(&roh), "einst.netzziel_port");
}

#[test]
fn the_access_kind_decides_which_fields_stand() {
    let kopf = |schluessel: Option<Value>, pfade: Option<Value>| {
        let mut ziel = json!({"host": "a.example", "port": 443, "zugriff": "kopf-einsetzen"});
        if let Some(wert) = schluessel {
            ziel["schluessel"] = wert;
        }
        if let Some(wert) = pfade {
            ziel["pfade"] = wert;
        }
        mit_netzziel(ziel)
    };
    let pfade = json!([{"methode": "GET", "pfad": "/a"}]);
    assert_eq!(kopf(Some(json!(true)), Some(pfade.clone())), "ok");
    assert_eq!(kopf(Some(json!(false)), Some(pfade.clone())), "ok");
    assert_eq!(kopf(None, Some(pfade.clone())), "einst.zugriff_felder");
    assert_eq!(kopf(Some(json!(true)), None), "einst.zugriff_felder");
    assert_eq!(kopf(None, None), "einst.zugriff_felder");
    assert_eq!(
        kopf(Some(json!("ja")), Some(pfade.clone())),
        "einst.typ_falsch"
    );
    assert_eq!(
        kopf(Some(json!(true)), Some(json!("x"))),
        "einst.typ_falsch"
    );
    for art in ["verbinden", "http-lesen"] {
        let mut ziel = json!({"host": "a.example", "port": 80, "zugriff": art});
        assert_eq!(mit_netzziel(ziel.clone()), "ok", "{art}");
        ziel["schluessel"] = json!(false);
        assert_eq!(
            mit_netzziel(ziel.clone()),
            "einst.zugriff_felder",
            "{art} mit schluessel"
        );
        ziel.as_object_mut().expect("object").remove("schluessel");
        ziel["pfade"] = pfade.clone();
        assert_eq!(
            mit_netzziel(ziel),
            "einst.zugriff_felder",
            "{art} mit pfade"
        );
    }
    for schlecht in ["Verbinden", "kopf_einsetzen", "", "tunnel"] {
        assert_eq!(
            mit_netzziel(json!({"host": "a.example", "port": 80, "zugriff": schlecht})),
            "einst.zugriff_unbekannt",
            "{schlecht}"
        );
    }
    assert_eq!(
        mit_netzziel(json!({"host": "a.example", "port": 80, "zugriff": true})),
        "einst.typ_falsch"
    );
}

#[test]
fn the_path_entries_follow_the_form() {
    for methode in ["GET", "HEAD", "POST", "PUT", "PATCH", "DELETE"] {
        assert_eq!(mit_pfad(methode, "/a"), "ok", "{methode}");
    }
    for schlecht in ["get", "CONNECT", "OPTIONS", "TRACE", "", "GET "] {
        assert_eq!(mit_pfad(schlecht, "/a"), "einst.pfad_form", "{schlecht}");
    }
    for gut in [
        "/a",
        "/a/b",
        "/*",
        "/a/*",
        "/a/b/*",
        "/a.b/c_d~e-f",
        "/1",
        "/..a",
        "/a..",
        "/.a",
    ] {
        assert_eq!(mit_pfad("GET", gut), "ok", "{gut}");
    }
    for schlecht in [
        "", "/", "a", "a/b", "/a/", "/a//b", "//a", "/a/*/b", "/a*", "/*a", "/**", "/a/**", "*",
        "/a/.", "/a/..", "/./a", "/../a", "/a?x=1", "/a%2f", "/a b", "/ä", "/a#x", "/a:b", "/a/*/",
        "/a/ *",
    ] {
        assert_eq!(mit_pfad("GET", schlecht), "einst.pfad_form", "{schlecht}");
    }
    let lang = |laenge: usize| format!("/{}", "a".repeat(laenge - 1));
    assert_eq!(mit_pfad("GET", &lang(256)), "ok");
    assert_eq!(mit_pfad("GET", &lang(257)), "einst.pfad_form");
}

#[test]
fn the_lists_are_bounded() {
    let regeln = |anzahl: usize| -> Vec<Value> {
        (0..anzahl)
            .map(|n| json!({"methode": "GET", "pfad": format!("/p{n}")}))
            .collect()
    };
    let mit_regeln = |anzahl: usize| {
        mit_netzziel(
            json!({"host": "a.example", "port": 443, "zugriff": "kopf-einsetzen",
            "schluessel": false, "pfade": regeln(anzahl)}),
        )
    };
    assert_eq!(mit_regeln(1), "ok");
    assert_eq!(mit_regeln(32), "ok");
    assert_eq!(mit_regeln(33), "einst.anzahl");
    assert_eq!(mit_regeln(0), "einst.anzahl");
    let mit_zielen = |anzahl: usize| {
        let mut datei = basis();
        datei["routen"][0]["netzziele"] = (0..anzahl)
            .map(|n| json!({"host": format!("h{n}.example"), "port": 443, "zugriff": "verbinden"}))
            .collect();
        code(&datei)
    };
    assert_eq!(mit_zielen(0), "ok");
    assert_eq!(mit_zielen(64), "ok");
    assert_eq!(mit_zielen(65), "einst.anzahl");
}

#[test]
fn duplicates_are_the_same_pair_only() {
    let ziel =
        |host: &str, port: u16, art: &str| json!({"host": host, "port": port, "zugriff": art});
    let mit = |ziele: Vec<Value>| {
        let mut datei = basis();
        datei["routen"][0]["netzziele"] = Value::Array(ziele);
        code(&datei)
    };
    assert_eq!(
        mit(vec![
            ziel("a.example", 443, "verbinden"),
            ziel("a.example", 80, "verbinden")
        ]),
        "ok"
    );
    assert_eq!(
        mit(vec![
            ziel("a.example", 443, "verbinden"),
            ziel("b.example", 443, "verbinden")
        ]),
        "ok"
    );
    assert_eq!(
        mit(vec![
            ziel("a.example", 443, "verbinden"),
            ziel("a.example", 443, "verbinden")
        ]),
        "einst.kennung_doppelt"
    );
    assert_eq!(
        mit(vec![
            ziel("a.example", 443, "verbinden"),
            ziel("a.example", 443, "http-lesen")
        ]),
        "einst.kennung_doppelt"
    );
    let pfade = |eintraege: Vec<(&str, &str)>| {
        let liste: Vec<Value> = eintraege
            .into_iter()
            .map(|(methode, pfad)| json!({"methode": methode, "pfad": pfad}))
            .collect();
        mit_netzziel(
            json!({"host": "a.example", "port": 443, "zugriff": "kopf-einsetzen",
            "schluessel": false, "pfade": liste}),
        )
    };
    assert_eq!(pfade(vec![("GET", "/a"), ("POST", "/a")]), "ok");
    assert_eq!(pfade(vec![("GET", "/a"), ("GET", "/b")]), "ok");
    assert_eq!(
        pfade(vec![("GET", "/a"), ("GET", "/a")]),
        "einst.kennung_doppelt"
    );
}

#[test]
fn the_two_rules_for_a_route_with_a_bound_key() {
    let kopf = |host: &str, schluessel: bool| {
        json!({"host": host, "port": 443, "zugriff": "kopf-einsetzen", "schluessel": schluessel,
            "pfade": [{"methode": "GET", "pfad": "/a"}]})
    };
    let verbinden = json!({"host": "t.example", "port": 443, "zugriff": "verbinden"});
    let lesen_http = json!({"host": "h.example", "port": 80, "zugriff": "http-lesen"});
    let mit = |ziele: Vec<Value>| {
        let mut datei = basis();
        datei["routen"][0]["netzziele"] = Value::Array(ziele);
        code(&datei)
    };
    assert_eq!(
        mit(vec![kopf("a.example", true), verbinden.clone()]),
        "einst.verbinden_mit_schluessel"
    );
    assert_eq!(
        mit(vec![verbinden.clone(), kopf("a.example", true)]),
        "einst.verbinden_mit_schluessel"
    );
    assert_eq!(mit(vec![kopf("a.example", false), verbinden.clone()]), "ok");
    assert_eq!(mit(vec![verbinden]), "ok");
    assert_eq!(mit(vec![kopf("a.example", true), lesen_http]), "ok");
    assert_eq!(
        mit(vec![kopf("*.a.example", true)]),
        "einst.schluessel_unterdomaene"
    );
    assert_eq!(
        mit(vec![kopf("b.example", false), kopf("*.a.example", true)]),
        "einst.schluessel_unterdomaene"
    );
    assert_eq!(mit(vec![kopf("*.a.example", false)]), "ok");
    assert_eq!(
        mit(vec![kopf("*.a.example", false), kopf("b.example", true)]),
        "ok"
    );
    let http_unterdomaene = json!({"host": "*.a.example", "port": 80, "zugriff": "http-lesen"});
    assert_eq!(mit(vec![kopf("b.example", true), http_unterdomaene]), "ok");
}

#[test]
fn the_unknown_keys_are_refused_at_every_level_of_version_2() {
    let mut route = basis();
    route["routen"][0]["x"] = json!(1);
    assert_eq!(code(&route), "einst.schluessel_unbekannt");
    assert_eq!(
        mit_netzziel(json!({"host": "a.example", "port": 80, "zugriff": "verbinden", "x": 1})),
        "einst.schluessel_unbekannt"
    );
    assert_eq!(
        mit_netzziel(
            json!({"host": "a.example", "port": 80, "zugriff": "kopf-einsetzen",
            "schluessel": true, "pfade": [{"methode": "GET", "pfad": "/a", "x": 1}]})
        ),
        "einst.schluessel_unbekannt"
    );
    for fehlt in ["host", "port", "zugriff"] {
        let mut ziel = json!({"host": "a.example", "port": 80, "zugriff": "verbinden"});
        ziel.as_object_mut().expect("object").remove(fehlt);
        assert_eq!(mit_netzziel(ziel), "einst.schluessel_fehlt", "{fehlt}");
    }
    for fehlt in ["methode", "pfad"] {
        let mut eintrag = json!({"methode": "GET", "pfad": "/a"});
        eintrag.as_object_mut().expect("object").remove(fehlt);
        assert_eq!(
            mit_netzziel(
                json!({"host": "a.example", "port": 80, "zugriff": "kopf-einsetzen",
                "schluessel": true, "pfade": [eintrag]})
            ),
            "einst.schluessel_fehlt",
            "{fehlt}"
        );
    }
}

#[test]
fn the_typed_view_carries_the_fields_of_version_2() {
    let mut datei = basis();
    datei["routen"][0]["modell"] = json!("claude-sonnet-5.5");
    datei["routen"][0]["netzziele"] = json!([
        {"host": "api.example", "port": 443, "zugriff": "kopf-einsetzen", "schluessel": true,
         "pfade": [{"methode": "GET", "pfad": "/a/*"}, {"methode": "POST", "pfad": "/b"}]},
        {"host": "*.cdn.example", "port": 80, "zugriff": "http-lesen"}
    ]);
    datei["routen"][1]["ziel"] = json!("https://gateway.example:8443/api/v1");
    let einst = lesen(datei.to_string().as_bytes()).expect("valid");
    assert_eq!(einst.fassung(), 2);
    let cloud = einst.route("c").expect("route c");
    assert_eq!(cloud.art(), Routenart::Cloud);
    assert_eq!(cloud.modell(), Some("claude-sonnet-5.5"));
    assert!(cloud.ziel().is_none());
    let ziele = cloud.netzziele();
    assert_eq!(ziele.len(), 2);
    assert_eq!(ziele[0].host(), "api.example");
    assert_eq!(ziele[0].port(), 443);
    assert_eq!(ziele[0].zugriff(), Zugriff::KopfEinsetzen);
    assert_eq!(ziele[0].zugriff().kennung(), "kopf-einsetzen");
    assert!(ziele[0].schluessel());
    assert!(!ziele[0].ist_unterdomaene());
    assert_eq!(ziele[0].pfade().len(), 2);
    assert_eq!(ziele[0].pfade()[0].methode(), "GET");
    assert_eq!(ziele[0].pfade()[0].pfad(), "/a/*");
    assert_eq!(ziele[0].pfade()[1].methode(), "POST");
    assert_eq!(ziele[1].host(), "*.cdn.example");
    assert!(ziele[1].ist_unterdomaene());
    assert_eq!(ziele[1].zugriff(), Zugriff::HttpLesen);
    assert_eq!(ziele[1].zugriff().kennung(), "http-lesen");
    assert!(!ziele[1].schluessel());
    assert_eq!(ziele[1].pfade().len(), 0);
    assert_eq!(Zugriff::Verbinden.kennung(), "verbinden");
    let lokal = einst.route("l").expect("route l");
    assert_eq!(lokal.art(), Routenart::Lokal);
    assert_eq!(lokal.modell(), Some("m"));
    assert_eq!(lokal.netzziele().len(), 0);
    let ziel = lokal.ziel().expect("target");
    assert_eq!(ziel.schema(), Schema::Https);
    assert_eq!(ziel.host(), "gateway.example");
    assert_eq!(ziel.port(), 8443);
    assert_eq!(ziel.pfad(), "/api/v1");
    let eins = lesen(
        json!({"fassung": 1, "klassen": ["a"], "routen": [{"schluessel": "c", "art": "cloud", "klasse": "a"}],
            "kunden": [], "kundendaten_an_cloud": {"vorgabe": "nie", "freigaben": []}})
        .to_string()
        .as_bytes(),
    )
    .expect("version 1 valid");
    assert_eq!(eins.fassung(), 1);
    let route = eins.route("c").expect("route");
    assert!(route.ziel().is_none() && route.modell().is_none() && route.netzziele().is_empty());
}

#[test]
fn the_lowest_stage_wins_among_the_stages_18_to_31() {
    // A bad target scheme (19) and a bad model (23) together: 19.
    let mut datei = basis();
    datei["routen"][1]["ziel"] = json!("ftp://127.0.0.1:1/v1");
    datei["routen"][1]["modell"] = json!("a b");
    assert_eq!(code(&datei), "einst.ziel_schema");
    // A bad model (23) and a local route with a target (24): 23.
    let mut datei = basis();
    datei["routen"][1]["modell"] = json!("a b");
    datei["routen"][1]["netzziele"] =
        json!([{"host": "a.example", "port": 1, "zugriff": "verbinden"}]);
    assert_eq!(code(&datei), "einst.modell_form");
    // A route kind that does not fit the target (18) beats a bad host (25).
    let mut datei = basis();
    datei["routen"][0]["ziel"] = json!("http://127.0.0.1:1/v1");
    datei["routen"][0]["netzziele"] = json!([{"host": "A", "port": 1, "zugriff": "verbinden"}]);
    assert_eq!(code(&datei), "einst.ziel_art");
    // An unknown class (12) beats everything from stage 18.
    let mut datei = basis();
    datei["routen"][0]["klasse"] = json!("z");
    datei["routen"][0]["ziel"] = json!("x");
    assert_eq!(code(&datei), "einst.klasse_unbekannt");
}

#[test]
fn the_text_writer_escapes_exactly_the_control_characters() {
    use einstellungen::json::text_schreiben;
    assert_eq!(text_schreiben(" "), "\" \"");
    assert_eq!(text_schreiben("\u{1f}"), "\"\\u001f\"");
    assert_eq!(text_schreiben("\u{0}"), "\"\\u0000\"");
    assert_eq!(
        text_schreiben("a\"b\\c\nd\re\tf"),
        "\"a\\\"b\\\\c\\nd\\re\\tf\""
    );
}

#[test]
fn the_typed_view_of_version_1_carries_every_part() {
    let text = json!({
        "fassung": 1,
        "klassen": ["offen", "intern", "kunde"],
        "vorgabestufe": "intern",
        "routen": [
            {"schluessel": "c1", "art": "cloud", "klasse": "intern"},
            {"schluessel": "l1", "art": "lokal", "klasse": "kunde"},
            {"schluessel": "c2", "art": "cloud", "klasse": "offen"}
        ],
        "kunden": [
            {"schluessel": "k1", "erlaubt": ["l1", "c2"], "verboten": ["c1"]},
            {"schluessel": "k2"}
        ],
        "kundendaten_an_cloud": {"vorgabe": "nie", "freigaben": [
            {"route": "c1", "kunde": "k1"}, {"route": "c2", "kunde": "k2"}
        ]}
    });
    let einst = lesen(text.to_string().as_bytes()).expect("valid");
    assert_eq!(einst.fassung(), 1);
    assert_eq!(einst.klassen(), ["offen", "intern", "kunde"]);
    assert_eq!(einst.vorgabestufe(), "intern");
    assert_eq!(einst.vorgabestufe_rang(), 1);
    assert_eq!(einst.routen().len(), 3);
    assert_eq!(einst.routen()[0].schluessel(), "c1");
    assert_eq!(einst.routen()[1].schluessel(), "l1");
    assert_eq!(einst.routen()[2].schluessel(), "c2");
    let rang = |schluessel: &str| einst.route(schluessel).expect("route").rang();
    assert_eq!((rang("c1"), rang("l1"), rang("c2")), (1, 2, 0));
    let route = einst.route("l1").expect("route");
    assert_eq!(route.klasse(), "kunde");
    assert_eq!(route.art(), Routenart::Lokal);
    assert_eq!(route.art().kennung(), "lokal");
    assert_eq!(einst.route("c1").expect("route").art().kennung(), "cloud");
    assert!(einst.route("gibt-es-nicht").is_none());
    assert_eq!(einst.kunden().len(), 2);
    assert_eq!(einst.kunden()[0].schluessel(), "k1");
    let k1 = einst.kunde("k1").expect("customer");
    assert_eq!(k1.schluessel(), "k1");
    assert_eq!(k1.erlaubt(), ["l1", "c2"]);
    assert_eq!(k1.verboten(), ["c1"]);
    let k2 = einst.kunde("k2").expect("customer");
    assert_eq!(k2.schluessel(), "k2");
    assert_eq!(k2.erlaubt().len() + k2.verboten().len(), 0);
    assert!(einst.kunde("k3").is_none());
    let freigaben = einst.kundendaten_an_cloud().freigaben();
    assert_eq!(freigaben.len(), 2);
    assert_eq!((freigaben[0].route(), freigaben[0].kunde()), ("c1", "k1"));
    assert_eq!((freigaben[1].route(), freigaben[1].kunde()), ("c2", "k2"));
}

#[test]
fn without_a_stated_default_class_the_highest_class_counts() {
    let text = json!({"fassung": 1, "klassen": ["offen", "intern", "kunde"],
        "routen": [{"schluessel": "c", "art": "cloud", "klasse": "offen"}], "kunden": [],
        "kundendaten_an_cloud": {"vorgabe": "nie", "freigaben": []}});
    let einst = lesen(text.to_string().as_bytes()).expect("valid");
    assert_eq!(einst.vorgabestufe(), "kunde");
    assert_eq!(einst.vorgabestufe_rang(), 2);
}
