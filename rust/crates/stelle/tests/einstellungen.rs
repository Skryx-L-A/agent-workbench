//! The stages of the settings file that the visible cases of contracts 2 and 6 do not reach: the block `mcp`
//! in every stage, the limits of every list, and the order of the stages over the whole file.

use serde_json::{Value, json};
use stelle::Routenart;
use stelle::einstellungen::{Server, lesen};

fn basis() -> Value {
    json!({
        "fassung": 2, "klassen": ["oeffentlich", "intern", "kundendaten"], "vorgabestufe": "kundendaten",
        "routen": [{"schluessel": "cloud-beispiel", "art": "cloud", "klasse": "intern",
                    "netzziele": [{"host": "api.beispiel.invalid", "port": 443, "zugriff": "kopf-einsetzen", "schluessel": true,
                                   "pfade": [{"methode": "GET", "pfad": "/modelle/*"}]}]},
                   {"schluessel": "lokal-beispiel", "art": "lokal", "klasse": "kundendaten",
                    "ziel": "http://127.0.0.1:11434/v1", "modell": "qwen3:8b", "netzziele": []}],
        "kunden": [{"schluessel": "k-beispiel", "erlaubt": ["lokal-beispiel"], "verboten": []}],
        "kundendaten_an_cloud": {"vorgabe": "nie", "freigaben": []},
        "mcp": {"tickets": {"lesen": ["get"], "schreiben": ["update"]}}
    })
}

/// The base with the value at the path (keys and list positions) replaced; `None` removes it.
fn mit(pfad: &[&str], wert: Option<Value>) -> Value {
    let mut d = basis();
    let (letzter, weg) = pfad.split_last().expect("a path");
    let mut ort = &mut d;
    for teil in weg {
        ort = match teil.parse::<usize>() {
            Ok(n) => &mut ort[n],
            Err(_) => &mut ort[*teil],
        };
    }
    match (wert, letzter.parse::<usize>()) {
        (Some(w), Ok(n)) => ort[n] = w,
        (Some(w), Err(_)) => ort[*letzter] = w,
        (None, _) => {
            ort.as_object_mut().expect("object").remove(*letzter);
        }
    }
    d
}

fn code(d: &Value) -> Result<(), &'static str> {
    lesen(d.to_string().as_bytes())
        .map(|_| ())
        .map_err(stelle::Code::code)
}

fn liste(n: usize, vorsatz: &str) -> Value {
    Value::from((0..n).map(|i| format!("{vorsatz}{i}")).collect::<Vec<_>>())
}

#[test]
fn basis_ist_gueltig_und_getypt() {
    let e = lesen(basis().to_string().as_bytes()).expect("valid");
    let art = |schluessel: &str| e.datei.route(schluessel).map(einstellungen::Route::art);
    assert_eq!(art("cloud-beispiel"), Some(Routenart::Cloud));
    assert_eq!(art("lokal-beispiel"), Some(Routenart::Lokal));
    assert!(e.datei.kunde("k-beispiel").is_some());
    assert_eq!(
        e.mcp.get("tickets"),
        Some(&Server {
            lesen: ["get".to_owned()].into_iter().collect(),
            freigabe: true,
            frist_ms: 60_000,
            einloesefrist_ms: 60_000,
        })
    );
    let e = lesen(
        mit(&["mcp", "tickets"], Some(json!({"lesen": [], "schreiben": [], "freigabe": false, "frist_ms": 1, "einloesefrist_ms": 86_400_000})))
            .to_string()
            .as_bytes(),
    )
    .expect("valid");
    let s = &e.mcp["tickets"];
    assert_eq!(
        (s.freigabe, s.frist_ms, s.einloesefrist_ms),
        (false, 1, 86_400_000)
    );
    assert!(
        lesen(mit(&["mcp"], None).to_string().as_bytes())
            .expect("valid")
            .mcp
            .is_empty()
    );
    assert_eq!(code(&mit(&["mcp"], Some(json!({})))), Ok(()));
    assert_eq!(
        code(
            &mit(&["fassung"], Some(json!(1)))
                .as_object()
                .map(|o| {
                    let mut o = o.clone();
                    o.remove("mcp");
                    for route in o
                        .get_mut("routen")
                        .and_then(Value::as_array_mut)
                        .expect("routes")
                    {
                        for feld in ["ziel", "modell", "netzziele"] {
                            route.as_object_mut().expect("route").remove(feld);
                        }
                    }
                    Value::Object(o)
                })
                .expect("object")
        ),
        Ok(())
    );
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one table of cases, one row per stage"
)]
fn block_mcp_in_jeder_stufe() {
    let srv = |w: Value| mit(&["mcp", "tickets"], Some(w));
    let faelle = [
        (
            mit(&["fassung"], Some(json!(1))),
            "einst.schluessel_unbekannt",
        ),
        (
            mit(&["fassung"], Some(json!("2"))),
            "einst.schluessel_unbekannt",
        ),
        (
            srv(json!({"lesen": [], "schreiben": [], "x": 1})),
            "einst.schluessel_unbekannt",
        ),
        (srv(json!({"lesen": []})), "einst.schluessel_fehlt"),
        (srv(json!({"schreiben": []})), "einst.schluessel_fehlt"),
        (mit(&["mcp"], Some(json!([]))), "einst.typ_falsch"),
        (srv(json!([])), "einst.typ_falsch"),
        (
            srv(json!({"lesen": "get", "schreiben": []})),
            "einst.typ_falsch",
        ),
        (
            srv(json!({"lesen": [], "schreiben": [1]})),
            "einst.typ_falsch",
        ),
        (
            srv(json!({"lesen": [], "schreiben": [], "freigabe": "ja"})),
            "einst.typ_falsch",
        ),
        (
            srv(json!({"lesen": [], "schreiben": [], "frist_ms": 1.0})),
            "einst.typ_falsch",
        ),
        (
            srv(json!({"lesen": [], "schreiben": [], "einloesefrist_ms": "5"})),
            "einst.typ_falsch",
        ),
        (
            srv(json!({"lesen": liste(257, "w"), "schreiben": []})),
            "einst.anzahl",
        ),
        (
            srv(json!({"lesen": [], "schreiben": liste(257, "w")})),
            "einst.anzahl",
        ),
        (
            srv(json!({"lesen": [], "schreiben": ["a b"]})),
            "einst.kennung_form",
        ),
        (
            srv(json!({"lesen": ["_a"], "schreiben": []})),
            "einst.kennung_form",
        ),
        (
            mit(
                &["mcp"],
                Some(json!({"Tickets": {"lesen": [], "schreiben": []}})),
            ),
            "einst.kennung_form",
        ),
        (
            srv(json!({"lesen": ["a", "a"], "schreiben": []})),
            "einst.kennung_doppelt",
        ),
        (
            srv(json!({"lesen": [], "schreiben": ["a", "a"]})),
            "einst.kennung_doppelt",
        ),
        (
            srv(json!({"lesen": [], "schreiben": [], "frist_ms": 0})),
            "einst.frist_bereich",
        ),
        (
            srv(json!({"lesen": [], "schreiben": [], "frist_ms": -1})),
            "einst.frist_bereich",
        ),
        (
            srv(json!({"lesen": [], "schreiben": [], "frist_ms": 86_400_001})),
            "einst.frist_bereich",
        ),
        (
            srv(json!({"lesen": [], "schreiben": [], "einloesefrist_ms": 0})),
            "einst.frist_bereich",
        ),
        (
            srv(json!({"lesen": [], "schreiben": [], "einloesefrist_ms": u64::MAX})),
            "einst.frist_bereich",
        ),
        (
            srv(json!({"lesen": ["a"], "schreiben": ["b", "a"]})),
            "einst.mcp_doppelt",
        ),
        // Order: an earlier stage wins over a later one.
        (
            srv(json!({"lesen": ["a"], "schreiben": ["a"], "frist_ms": 0})),
            "einst.frist_bereich",
        ),
        (
            srv(json!({"lesen": ["a", "a"], "schreiben": [], "frist_ms": 0})),
            "einst.kennung_doppelt",
        ),
        (
            srv(json!({"lesen": [], "schreiben": [], "x": 1, "frist_ms": "a"})),
            "einst.schluessel_unbekannt",
        ),
        // With the block `mcp` a version other than 2 fails at stage 3 already; without it at stage 7.
        (
            mit(&["fassung"], Some(json!(3))),
            "einst.schluessel_unbekannt",
        ),
    ];
    for (d, soll) in faelle {
        assert_eq!(code(&d), Err(soll), "{d}");
    }
    let viele: serde_json::Map<String, Value> = (0..65)
        .map(|i| (format!("s{i}"), json!({"lesen": [], "schreiben": []})))
        .collect();
    assert_eq!(
        code(&mit(&["mcp"], Some(Value::Object(viele.clone())))),
        Err("einst.anzahl")
    );
    let genug: serde_json::Map<String, Value> = viele.into_iter().take(64).collect();
    assert_eq!(code(&mit(&["mcp"], Some(Value::Object(genug)))), Ok(()));
    let srv = |w: Value| mit(&["mcp", "tickets"], Some(w));
    assert_eq!(
        code(&srv(
            json!({"lesen": liste(256, "w"), "schreiben": liste(256, "x")})
        )),
        Ok(())
    );
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one table of cases, one row per stage"
)]
fn listen_und_querverweise() {
    let route = |n: usize| json!({"schluessel": format!("r{n}"), "art": "cloud", "klasse": "intern", "netzziele": []});
    let kunde = |n: usize| json!({"schluessel": format!("k{n}")});
    let freigabe = |n: usize| json!({"route": "cloud-beispiel", "kunde": format!("k{n}")});
    let routen = |n: usize| Value::from((0..n).map(route).collect::<Vec<_>>());
    let kunden = |n: usize| Value::from((0..n).map(kunde).collect::<Vec<_>>());
    let faelle = [
        (mit(&["klassen"], Some(liste(17, "c"))), Err("einst.anzahl")),
        (
            mit(&["klassen"], Some(json!(["a"])))
                .as_object()
                .map(|_| {
                    let mut d = mit(&["klassen"], Some(json!(["a"])));
                    d["vorgabestufe"] = json!("a");
                    d["routen"] =
                        json!([{"schluessel": "r", "art": "cloud", "klasse": "a", "netzziele": []}]);
                    d["kunden"] = json!([]);
                    d
                })
                .expect("object"),
            Ok(()),
        ),
        (mit(&["routen"], Some(routen(65))), Err("einst.anzahl")),
        (mit(&["kunden"], Some(kunden(257))), Err("einst.anzahl")),
        (mit(&["kunden"], Some(kunden(256))), Ok(())),
        (
            mit(
                &["kunden", "0", "erlaubt"],
                Some(Value::from(vec!["lokal-beispiel"; 65])),
            ),
            Err("einst.anzahl"),
        ),
        (
            mit(
                &["kunden", "0", "verboten"],
                Some(Value::from(vec!["lokal-beispiel"; 65])),
            ),
            Err("einst.anzahl"),
        ),
        (
            mit(
                &["kunden", "0", "verboten"],
                Some(json!(["lokal-beispiel", "lokal-beispiel"])),
            ),
            Err("einst.kennung_doppelt"),
        ),
        (
            mit(
                &["kunden", "0", "erlaubt"],
                Some(json!(["lokal-beispiel", "lokal-beispiel"])),
            ),
            Err("einst.kennung_doppelt"),
        ),
        (
            mit(&["kunden", "0", "erlaubt"], Some(json!("lokal-beispiel"))),
            Err("einst.typ_falsch"),
        ),
        (
            mit(&["kunden", "0", "verboten"], Some(json!([1]))),
            Err("einst.typ_falsch"),
        ),
        (
            mit(&["kunden", "0", "verboten"], Some(json!(["fremd"]))),
            Err("einst.route_unbekannt"),
        ),
        (
            mit(&["kunden", "0", "verboten"], Some(json!(["Fremd"]))),
            Err("einst.kennung_form"),
        ),
        (
            mit(&["kunden", "0", "schluessel"], Some(json!(5))),
            Err("einst.typ_falsch"),
        ),
        (
            mit(&["kunden", "0", "schluessel"], None),
            Err("einst.schluessel_fehlt"),
        ),
        (
            mit(&["kunden", "0", "x"], Some(json!(1))),
            Err("einst.schluessel_unbekannt"),
        ),
        (
            mit(&["kunden", "0"], Some(json!("k"))),
            Err("einst.typ_falsch"),
        ),
        (mit(&["kunden"], Some(json!({}))), Err("einst.typ_falsch")),
        (mit(&["kunden"], None), Err("einst.schluessel_fehlt")),
        (
            mit(&["routen", "0"], Some(json!("r"))),
            Err("einst.typ_falsch"),
        ),
        (
            mit(&["routen", "0", "klasse"], Some(json!(5))),
            Err("einst.typ_falsch"),
        ),
        (
            mit(&["routen", "0", "art"], Some(json!(5))),
            Err("einst.typ_falsch"),
        ),
        (
            mit(&["routen", "0", "art"], None),
            Err("einst.schluessel_fehlt"),
        ),
        (
            mit(&["routen", "0", "schluessel"], None),
            Err("einst.schluessel_fehlt"),
        ),
        (
            mit(&["routen", "0", "klasse"], Some(json!("Intern"))),
            Err("einst.kennung_form"),
        ),
        (mit(&["routen"], Some(json!("r"))), Err("einst.typ_falsch")),
        (mit(&["klassen"], Some(json!("a"))), Err("einst.typ_falsch")),
        (
            mit(&["vorgabestufe"], Some(json!(1))),
            Err("einst.typ_falsch"),
        ),
        (
            mit(&["vorgabestufe"], Some(json!("Hoch"))),
            Err("einst.kennung_form"),
        ),
        (
            mit(&["kundendaten_an_cloud"], Some(json!("nie"))),
            Err("einst.typ_falsch"),
        ),
        (
            mit(&["kundendaten_an_cloud", "x"], Some(json!(1))),
            Err("einst.schluessel_unbekannt"),
        ),
        (
            mit(&["kundendaten_an_cloud", "vorgabe"], None),
            Err("einst.schluessel_fehlt"),
        ),
        (
            mit(&["kundendaten_an_cloud", "freigaben"], None),
            Err("einst.schluessel_fehlt"),
        ),
        (
            mit(&["kundendaten_an_cloud", "freigaben"], Some(json!({}))),
            Err("einst.typ_falsch"),
        ),
        (
            mit(&["kundendaten_an_cloud", "freigaben"], Some(json!(["f"]))),
            Err("einst.typ_falsch"),
        ),
        (
            mit(
                &["kundendaten_an_cloud", "freigaben"],
                Some(json!([{"route": "cloud-beispiel"}])),
            ),
            Err("einst.schluessel_fehlt"),
        ),
        (
            mit(
                &["kundendaten_an_cloud", "freigaben"],
                Some(json!([{"route": "cloud-beispiel", "kunde": 1}])),
            ),
            Err("einst.typ_falsch"),
        ),
        (
            mit(
                &["kundendaten_an_cloud", "freigaben"],
                Some(json!([{"route": "cloud-beispiel", "kunde": "k-beispiel", "x": 1}])),
            ),
            Err("einst.schluessel_unbekannt"),
        ),
        (
            mit(
                &["kundendaten_an_cloud", "freigaben"],
                Some(json!([{"route": "cloud-beispiel", "kunde": "K"}])),
            ),
            Err("einst.kennung_form"),
        ),
        (
            mit(
                &["kundendaten_an_cloud", "freigaben"],
                Some(json!([{"route": "Cloud", "kunde": "k-beispiel"}])),
            ),
            Err("einst.kennung_form"),
        ),
        (
            mit(
                &["kundendaten_an_cloud", "freigaben"],
                Some(
                    json!([{"route": "cloud-beispiel", "kunde": "k-beispiel"}, {"route": "cloud-beispiel", "kunde": "k-beispiel"}]),
                ),
            ),
            Err("einst.kennung_doppelt"),
        ),
        (
            mit(
                &["kundendaten_an_cloud", "freigaben"],
                Some(
                    json!([{"route": "cloud-beispiel", "kunde": "k-beispiel"}, {"route": "lokal-beispiel", "kunde": "k-beispiel"}]),
                ),
            ),
            Err("einst.freigabe_route_lokal"),
        ),
        (
            mit(
                &["kundendaten_an_cloud", "freigaben"],
                Some(json!([{"route": "cloud-beispiel", "kunde": "k-beispiel"}])),
            ),
            Ok(()),
        ),
        (
            mit(
                &["kundendaten_an_cloud", "freigaben"],
                Some(json!([{"route": "cloud-fremd", "kunde": "k-beispiel"}])),
            ),
            Err("einst.route_unbekannt"),
        ),
        (
            mit(&["kundendaten_an_cloud", "vorgabe"], Some(json!("immer"))),
            Err("einst.vorgabe_wert"),
        ),
        (
            mit(&["kundendaten_an_cloud", "vorgabe"], Some(json!(false))),
            Err("einst.typ_falsch"),
        ),
    ];
    for (d, soll) in faelle {
        assert_eq!(code(&d), soll, "{d}");
    }
    // 1024 releases are allowed, 1025 are too many (with enough customers for distinct pairs).
    let mut d = mit(&["kunden"], Some(kunden(256)));
    d["kundendaten_an_cloud"]["freigaben"] =
        Value::from((0..256).map(freigabe).collect::<Vec<_>>());
    assert_eq!(code(&d), Ok(()));
    let mut d = mit(&["kunden"], Some(kunden(256)));
    let viele: Vec<Value> = (0..1025)
        .map(|i| json!({"route": format!("r{}", i / 256), "kunde": format!("k{}", i % 256)}))
        .collect();
    d["routen"] = routen(5);
    d["kundendaten_an_cloud"]["freigaben"] = Value::from(viele);
    assert_eq!(code(&d), Err("einst.anzahl"));
    d["kundendaten_an_cloud"]["freigaben"]
        .as_array_mut()
        .expect("list")
        .pop();
    assert_eq!(code(&d), Ok(()));
    let mut d = mit(&["routen"], Some(routen(64)));
    d["kunden"] = json!([]);
    assert_eq!(code(&d), Ok(()));
    assert_eq!(
        code(&mit(&["klassen"], Some(json!([])))),
        Err("einst.anzahl")
    );
    assert_eq!(
        code(&mit(&["routen"], Some(json!([])))),
        Err("einst.anzahl")
    );
}

#[test]
fn vierundsechzig_routen_je_kunde() {
    let routen: Vec<Value> = (0..64)
        .map(|n| {
            json!({"schluessel": format!("r{n}"), "art": "lokal", "klasse": "intern",
                   "ziel": "http://127.0.0.1:11434/v1", "modell": "qwen3:8b", "netzziele": []})
        })
        .collect();
    let namen = liste(64, "r");
    for feld in ["erlaubt", "verboten"] {
        let mut d = mit(&["routen"], Some(Value::from(routen.clone())));
        d["kunden"][0] = json!({"schluessel": "k-beispiel", feld: namen.clone()});
        assert_eq!(code(&d), Ok(()), "{feld}");
    }
}

#[test]
fn groesse_und_lesen() {
    let ablehnung = |bytes: &[u8]| lesen(bytes).map(|_| ()).map_err(|c| (c.code(), c.grund()));
    assert_eq!(
        ablehnung(&vec![b' '; 65_537]),
        Err(("einst.zu_gross", "Einstellungsdatei zu gross"))
    );
    assert_eq!(
        ablehnung(&vec![b' '; 65_536]),
        Err(("einst.json_ungueltig", "kein gueltiges JSON"))
    );
    assert_eq!(
        ablehnung(b"[]"),
        Err(("einst.json_ungueltig", "kein gueltiges JSON"))
    );
}

/// A `ziel` of the local route replaced.
fn ziel(text: &str) -> Value {
    mit(&["routen", "1", "ziel"], Some(json!(text)))
}

/// The first net target of the cloud route replaced by the object.
fn netz(objekt: Value) -> Value {
    mit(&["routen", "0", "netzziele", "0"], Some(objekt))
}

/// The base net target with one field replaced; `None` removes it.
fn netzfeld(feld: &str, wert: Option<Value>) -> Value {
    mit(&["routen", "0", "netzziele", "0", feld], wert)
}

/// The path list of the base net target replaced by one entry.
fn pfad_eintrag(wert: &Value) -> Value {
    mit(
        &["routen", "0", "netzziele", "0", "pfade"],
        Some(json!([wert])),
    )
}

/// Two cloud routes with the given net targets instead of the routes of the base (no customer names a route).
fn zwei_cloud_routen(erste: &Value, zweite: &Value) -> Value {
    let mut d = mit(
        &["routen"],
        Some(json!([
            {"schluessel": "c1", "art": "cloud", "klasse": "intern", "netzziele": erste},
            {"schluessel": "c2", "art": "cloud", "klasse": "intern", "netzziele": zweite}
        ])),
    );
    d["kunden"][0]["erlaubt"] = json!([]);
    d
}

type Faelle = Vec<(Value, Result<(), &'static str>)>;

fn pruefe(faelle: Faelle) {
    for (d, soll) in faelle {
        assert_eq!(code(&d), soll, "{d}");
    }
}

#[test]
fn typen_modelle_und_felder_der_fassung_2() {
    let e = lesen(basis().to_string().as_bytes()).expect("valid");
    assert_eq!(
        e.datei
            .route("lokal-beispiel")
            .and_then(einstellungen::Route::modell),
        Some("qwen3:8b")
    );
    assert_eq!(
        e.datei
            .route("cloud-beispiel")
            .and_then(einstellungen::Route::modell),
        None
    );
    let e = lesen(
        mit(&["routen", "0", "modell"], Some(json!("gpt-5.6-sol")))
            .to_string()
            .as_bytes(),
    )
    .expect("valid");
    assert_eq!(
        e.datei
            .route("cloud-beispiel")
            .and_then(einstellungen::Route::modell),
        Some("gpt-5.6-sol")
    );
    let pfad_feld = |feld: &str, wert: Option<Value>| {
        mit(&["routen", "0", "netzziele", "0", "pfade", "0", feld], wert)
    };
    pruefe(vec![
        // Stage 3: the reference version decides which keys exist on a route, a net target and a path entry.
        (mit(&["fassung"], Some(json!(2.0))), Err("einst.typ_falsch")),
        (
            mit(&["fassung"], Some(json!(1.0))),
            Err("einst.schluessel_unbekannt"),
        ),
        (
            mit(&["routen", "0", "x"], Some(json!(1))),
            Err("einst.schluessel_unbekannt"),
        ),
        (
            mit(&["x"], Some(json!(1))),
            Err("einst.schluessel_unbekannt"),
        ),
        (
            netzfeld("x", Some(json!(1))),
            Err("einst.schluessel_unbekannt"),
        ),
        (
            pfad_feld("x", Some(json!(1))),
            Err("einst.schluessel_unbekannt"),
        ),
        // Stage 5.
        (
            mit(&["routen", "0", "netzziele"], None),
            Err("einst.schluessel_fehlt"),
        ),
        (netzfeld("host", None), Err("einst.schluessel_fehlt")),
        (netzfeld("port", None), Err("einst.schluessel_fehlt")),
        (netzfeld("zugriff", None), Err("einst.schluessel_fehlt")),
        (pfad_feld("methode", None), Err("einst.schluessel_fehlt")),
        (pfad_feld("pfad", None), Err("einst.schluessel_fehlt")),
        // Stage 6.
        (
            mit(&["routen", "0", "netzziele"], Some(json!("x"))),
            Err("einst.typ_falsch"),
        ),
        (
            mit(&["routen", "0", "netzziele"], Some(json!([1]))),
            Err("einst.typ_falsch"),
        ),
        (netzfeld("host", Some(json!(1))), Err("einst.typ_falsch")),
        (
            netzfeld("port", Some(json!(443.0))),
            Err("einst.typ_falsch"),
        ),
        (
            netzfeld("port", Some(json!("443"))),
            Err("einst.typ_falsch"),
        ),
        (netzfeld("zugriff", Some(json!(1))), Err("einst.typ_falsch")),
        (
            netzfeld("schluessel", Some(json!("true"))),
            Err("einst.typ_falsch"),
        ),
        (netzfeld("pfade", Some(json!("x"))), Err("einst.typ_falsch")),
        (netzfeld("pfade", Some(json!([1]))), Err("einst.typ_falsch")),
        (
            pfad_feld("methode", Some(json!(1))),
            Err("einst.typ_falsch"),
        ),
        (pfad_feld("pfad", Some(json!(1))), Err("einst.typ_falsch")),
        (
            mit(&["routen", "1", "ziel"], Some(json!(1))),
            Err("einst.typ_falsch"),
        ),
        (
            mit(&["routen", "0", "modell"], Some(json!(1))),
            Err("einst.typ_falsch"),
        ),
    ]);
}

#[test]
fn anzahl_und_doppelte_der_netzziele() {
    let ziele = |n: usize| {
        Value::from(
            (0..n)
                .map(|i| json!({"host": format!("h{i}.beispiel.invalid"), "port": 443, "zugriff": "verbinden"}))
                .collect::<Vec<_>>(),
        )
    };
    let netzziele = |w: Value| mit(&["routen", "0", "netzziele"], Some(w));
    let pfade = |n: usize| {
        Value::from(
            (0..n)
                .map(|i| json!({"methode": "GET", "pfad": format!("/p{i}")}))
                .collect::<Vec<_>>(),
        )
    };
    let a = json!({"host": "a.beispiel.invalid", "port": 443, "zugriff": "verbinden"});
    let a_444 = json!({"host": "a.beispiel.invalid", "port": 444, "zugriff": "verbinden"});
    let b = json!({"host": "b.beispiel.invalid", "port": 443, "zugriff": "verbinden"});
    let a_lesen = json!({"host": "a.beispiel.invalid", "port": 443, "zugriff": "http-lesen"});
    let get = |p: &str| json!({"methode": "GET", "pfad": p});
    let post = |p: &str| json!({"methode": "POST", "pfad": p});
    pruefe(vec![
        (netzziele(ziele(64)), Ok(())),
        (netzziele(ziele(65)), Err("einst.anzahl")),
        (netzfeld("pfade", Some(pfade(32))), Ok(())),
        (netzfeld("pfade", Some(pfade(33))), Err("einst.anzahl")),
        (netzfeld("pfade", Some(json!([]))), Err("einst.anzahl")),
        // The same host with another port is no duplicate; the same pair is, whatever the access kind.
        (netzziele(json!([a, a_444])), Ok(())),
        (netzziele(json!([a, b])), Ok(())),
        (netzziele(json!([a, a])), Err("einst.kennung_doppelt")),
        (netzziele(json!([a, a_lesen])), Err("einst.kennung_doppelt")),
        // Two routes may hold the same target.
        (zwei_cloud_routen(&json!([a]), &json!([a])), Ok(())),
        // The same method with another path, the same path with another method: no duplicate.
        (
            netzfeld("pfade", Some(json!([get("/a"), get("/b")]))),
            Ok(()),
        ),
        (
            netzfeld("pfade", Some(json!([get("/a"), post("/a")]))),
            Ok(()),
        ),
        (
            netzfeld("pfade", Some(json!([get("/a"), get("/a")]))),
            Err("einst.kennung_doppelt"),
        ),
    ]);
}

#[test]
fn ziel_der_lokalen_route() {
    let lang = |n: usize| format!("http://127.0.0.1:11434/{}/v1", "a".repeat(n));
    let label = |n: usize| format!("https://{}.invalid:1/v1", "a".repeat(n));
    let ok = |t: &str| (ziel(t), Ok(()));
    let schema = |t: &str| (ziel(t), Err("einst.ziel_schema"));
    let form = |t: &str| (ziel(t), Err("einst.ziel_form"));
    let port = |t: &str| (ziel(t), Err("einst.ziel_port"));
    let host = |t: &str| (ziel(t), Err("einst.ziel_host"));
    pruefe(vec![
        ok("http://127.0.0.1:11434/v1"),
        ok("http://127.0.0.1:1/v1"),
        ok("http://127.0.0.1:65535/v1"),
        ok("https://localhost:1/v1"),
        ok("https://127.0.0.2:1/v1"),
        ok("https://10.0.0.1:443/api/v1"),
        ok("https://172.16.0.1:1/v1"),
        ok("https://172.31.255.255:1/v1"),
        ok("https://192.168.1.1:1/v1"),
        ok("https://gateway:1/a.b_c~d-e/v1"),
        ok(&label(63)),
        ok(&lang(230)),
        schema(""),
        schema("ftp://127.0.0.1:1/v1"),
        schema("127.0.0.1:11434/v1"),
        schema("gateway"),
        form("http:/127.0.0.1:1/v1"),
        form("http:127.0.0.1:1/v1"),
        form("http://127.0.0.1:11434"),
        form("http://127.0.0.1/v1"),
        form("http://:11434/v1"),
        form("http://127.0.0.1:/v1"),
        form("http://127.0.0.1:011434/v1"),
        form("http://127.0.0.1:00/v1"),
        form("http://127.0.0.1:100000/v1"),
        form("http://127.0.0.1:1a/v1"),
        form("http://127.0.0.1:1:2/v1"),
        form("http://127.0.0.1:1/v1/"),
        form("http://127.0.0.1:1//v1"),
        form("http://127.0.0.1:1/./v1"),
        form("http://127.0.0.1:1/../v1"),
        form("http://127.0.0.1:1/a b/v1"),
        form("http://127.0.0.1:1/v2"),
        form("http://127.0.0.1:1/v1/x"),
        form("http://127.0.0.1:1/V1"),
        form("http://127.0.0.1:1/v1?x=1"),
        form("http://127.0.0.1:1/v1#x"),
        form("http://u@127.0.0.1:1/v1"),
        form("http://[::1]:1/v1"),
        form("https://-a.invalid:1/v1"),
        form("https://a-.invalid:1/v1"),
        form("https://a_b.invalid:1/v1"),
        form("https://A.invalid:1/v1"),
        form("https://a..invalid:1/v1"),
        form("https://a.1b:1/v1"),
        form("https://1.2.3:1/v1"),
        form("https://1.2.3.4.5:1/v1"),
        form("https://01.2.3.4:1/v1"),
        form("https://1.2.3.256:1/v1"),
        form("https://1.2.3.:1/v1"),
        form("https://ex\u{e4}mple.invalid:1/v1"),
        form(&label(64)),
        form(&lang(231)),
        port("http://127.0.0.1:0/v1"),
        port("http://127.0.0.1:65536/v1"),
        port("http://127.0.0.1:99999/v1"),
        host("https://8.8.8.8:1/v1"),
        host("https://0.0.0.0:1/v1"),
        host("https://9.0.0.1:1/v1"),
        host("https://11.0.0.1:1/v1"),
        host("https://126.0.0.1:1/v1"),
        host("https://128.0.0.1:1/v1"),
        host("https://172.15.0.1:1/v1"),
        host("https://172.32.0.1:1/v1"),
        host("https://171.16.0.1:1/v1"),
        host("https://192.167.0.1:1/v1"),
        host("https://192.169.0.1:1/v1"),
        host("https://193.168.0.1:1/v1"),
        host("http://127.0.0.2:1/v1"),
        host("http://10.0.0.1:1/v1"),
        host("http://localhost:1/v1"),
        host("http://gateway:1/v1"),
    ]);
}

#[test]
fn modell_und_art_der_route() {
    let modell = |t: &str| mit(&["routen", "1", "modell"], Some(json!(t)));
    let netzziel_lokal = mit(
        &["routen", "1", "netzziele"],
        Some(json!([{"host": "A", "port": 0, "zugriff": "x"}])),
    );
    // The stages run in their order: target host (22) before model form (23) before net target on a local
    // route (24) before the form of the net target (25).
    let mut host_vor_modell = modell("a b");
    host_vor_modell["routen"][1]["ziel"] = json!("https://8.8.8.8:1/v1");
    let mut modell_vor_netzziel = netzziel_lokal.clone();
    modell_vor_netzziel["routen"][1]["modell"] = json!("a b");
    pruefe(vec![
        (modell("qwen3:8b"), Ok(())),
        (modell(&"a".repeat(128)), Ok(())),
        (modell("ns/modell:8b-q4_K_M@x+y"), Ok(())),
        (modell(&"a".repeat(129)), Err("einst.modell_form")),
        (modell(""), Err("einst.modell_form")),
        (modell("a b"), Err("einst.modell_form")),
        (modell("a!"), Err("einst.modell_form")),
        (
            mit(&["routen", "0", "modell"], Some(json!("a b"))),
            Err("einst.modell_form"),
        ),
        (mit(&["routen", "1", "modell"], None), Err("einst.ziel_art")),
        (mit(&["routen", "1", "ziel"], None), Err("einst.ziel_art")),
        (
            mit(
                &["routen", "0", "ziel"],
                Some(json!("http://127.0.0.1:1/v1")),
            ),
            Err("einst.ziel_art"),
        ),
        (host_vor_modell, Err("einst.ziel_host")),
        (modell_vor_netzziel, Err("einst.modell_form")),
        (netzziel_lokal, Err("einst.netzziel_lokal")),
    ]);
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one table of cases, one row per rule"
)]
fn netzziele_host_port_zugriff_und_pfade() {
    let host = |t: &str, gut: bool| {
        (
            netzfeld("host", Some(json!(t))),
            if gut {
                Ok(())
            } else {
                Err("einst.netzziel_form")
            },
        )
    };
    let port = |p: Value, gut: bool| {
        (
            netzfeld("port", Some(p)),
            if gut {
                Ok(())
            } else {
                Err("einst.netzziel_port")
            },
        )
    };
    let methode = |m: &str| {
        (
            pfad_eintrag(&json!({"methode": m, "pfad": "/a"})),
            if ["GET", "HEAD", "POST", "PUT", "PATCH", "DELETE"].contains(&m) {
                Ok(())
            } else {
                Err("einst.pfad_form")
            },
        )
    };
    let pfad = |t: &str, gut: bool| {
        (
            pfad_eintrag(&json!({"methode": "GET", "pfad": t})),
            if gut { Ok(()) } else { Err("einst.pfad_form") },
        )
    };
    let get_a = json!([{"methode": "GET", "pfad": "/a"}]);
    let kopf = |schluessel: bool, host: &str| {
        netz(
            json!({"host": host, "port": 443, "zugriff": "kopf-einsetzen", "schluessel": schluessel,
                    "pfade": get_a}),
        )
    };
    let tunnel = json!({"host": "t.beispiel.invalid", "port": 443, "zugriff": "verbinden"});
    let kopf_ziel = |schluessel: bool, host: &str| json!({"host": host, "port": 443, "zugriff": "kopf-einsetzen", "schluessel": schluessel, "pfade": get_a});
    let mit_tunnel = |schluessel: bool| {
        mit(
            &["routen", "0", "netzziele"],
            Some(json!([kopf_ziel(schluessel, "k.beispiel.invalid"), tunnel])),
        )
    };
    let mut faelle: Faelle = vec![
        host("a.b", true),
        host("a-b.c1", true),
        host("localhost.invalid", true),
        host("foo.xlocalhost", true),
        host("a", false),
        host("*", false),
        host("*.a", false),
        host("a.*.b", false),
        host("a.1", false),
        host("-a.b", false),
        host("a-.b", false),
        host("a..b", false),
        host("A.b", false),
        host("a.b.", false),
        host("1.2.3.4", false),
        host("localhost", false),
        host("x.localhost", false),
        host("*.x.localhost", false),
        host("http://a.b", false),
        host(&format!("{}.b", "a".repeat(64)), false),
        port(json!(1), true),
        port(json!(65535), true),
        port(json!(0), false),
        port(json!(65536), false),
        port(json!(-1), false),
        port(json!(u64::MAX), false),
        (
            netzfeld("zugriff", Some(json!("schreiben"))),
            Err("einst.zugriff_unbekannt"),
        ),
        (
            netzfeld("zugriff", Some(json!("KOPF-EINSETZEN"))),
            Err("einst.zugriff_unbekannt"),
        ),
        (
            netz(json!({"host": "a.b", "port": 1, "zugriff": "http-lesen", "pfade": get_a})),
            Err("einst.zugriff_felder"),
        ),
        (
            netz(json!({"host": "a.b", "port": 1, "zugriff": "verbinden", "schluessel": false})),
            Err("einst.zugriff_felder"),
        ),
        (
            netz(json!({"host": "a.b", "port": 1, "zugriff": "kopf-einsetzen", "pfade": get_a})),
            Err("einst.zugriff_felder"),
        ),
        (
            netz(
                json!({"host": "a.b", "port": 1, "zugriff": "kopf-einsetzen", "schluessel": true}),
            ),
            Err("einst.zugriff_felder"),
        ),
        (
            netz(json!({"host": "a.b", "port": 1, "zugriff": "http-lesen"})),
            Ok(()),
        ),
        (kopf(false, "a.b"), Ok(())),
        (kopf(true, "a.b"), Ok(())),
        (kopf(true, "*.b.c"), Err("einst.schluessel_unterdomaene")),
        (kopf(false, "*.b.c"), Ok(())),
        (mit_tunnel(true), Err("einst.verbinden_mit_schluessel")),
        (mit_tunnel(false), Ok(())),
        // The tunnel and the key stand on different routes: no violation.
        (
            zwei_cloud_routen(
                &json!([kopf_ziel(true, "k.beispiel.invalid")]),
                &json!([tunnel]),
            ),
            Ok(()),
        ),
        // A wildcard target with a key and a tunnel on one route: the tunnel rule (30) comes first.
        (
            mit(
                &["routen", "0", "netzziele"],
                Some(json!([kopf_ziel(true, "*.k.beispiel.invalid"), tunnel])),
            ),
            Err("einst.verbinden_mit_schluessel"),
        ),
    ];
    for m in [
        "GET", "HEAD", "POST", "PUT", "PATCH", "DELETE", "OPTIONS", "get", "CONNECT", "",
    ] {
        faelle.push(methode(m));
    }
    let lang = format!("/{}", "a".repeat(255));
    let zu_lang = format!("/{}", "a".repeat(256));
    for (t, gut) in [
        ("/*", true),
        ("/a", true),
        ("/a/*", true),
        ("/a/b/c", true),
        ("/a.b_c~d-e/X9", true),
        ("/a/.b", true),
        (lang.as_str(), true),
        (zu_lang.as_str(), false),
        ("", false),
        ("/", false),
        ("*", false),
        ("a", false),
        ("/a/", false),
        ("/a//b", false),
        ("//a", false),
        ("/.", false),
        ("/..", false),
        ("/a/./b", false),
        ("/a/../b", false),
        ("/a*", false),
        ("/a/**", false),
        ("/a/*/b", false),
        ("/a/*/", false),
        ("/*/a", false),
        ("/a%20b", false),
        ("/a?x=1", false),
        ("/a b", false),
        ("/a#x", false),
    ] {
        faelle.push(pfad(t, gut));
    }
    pruefe(faelle);
}

/// The block `harness` (contract 2, version 2, stage 34).
#[test]
fn block_harness_der_fassung_2() {
    let hw = |w: Value| {
        mit(
            &["harness"],
            Some(json!({"copilot": {"werkzeuge_ohne_rueckfrage": w}})),
        )
    };
    assert_eq!(code(&hw(json!(["shell", "write", "such-mcp"]))), Ok(()));
    assert_eq!(code(&hw(json!([]))), Ok(()));
    assert_eq!(code(&mit(&["harness"], Some(json!({})))), Ok(()));
    // Version 1 does not know the block.
    let mut v1 = hw(json!(["shell"]));
    v1["fassung"] = json!(1);
    for k in ["ziel", "modell", "netzziele"] {
        v1["routen"][1].as_object_mut().expect("object").remove(k);
    }
    v1["routen"][0]
        .as_object_mut()
        .expect("object")
        .remove("netzziele");
    v1.as_object_mut().expect("object").remove("mcp");
    assert_eq!(code(&v1), Err("einst.schluessel_unbekannt"));
    // Closed form.
    assert_eq!(
        code(&mit(&["harness"], Some(json!({"claude": {}})))),
        Err("einst.schluessel_unbekannt")
    );
    assert_eq!(
        code(&mit(
            &["harness"],
            Some(json!({"copilot": {"werkzeuge_ohne_rueckfrage": [], "x": 1}}))
        )),
        Err("einst.schluessel_unbekannt")
    );
    assert_eq!(
        code(&mit(&["harness"], Some(json!({"copilot": {}})))),
        Err("einst.schluessel_fehlt")
    );
    // Types.
    for falsch in [
        json!([]),
        json!("x"),
        json!({"copilot": []}),
        json!({"copilot": "x"}),
    ] {
        assert_eq!(
            code(&mit(&["harness"], Some(falsch))),
            Err("einst.typ_falsch")
        );
    }
    for falsch in [json!("shell"), json!([1]), json!(["shell", null])] {
        assert_eq!(code(&hw(falsch)), Err("einst.typ_falsch"));
    }
    // Duplicate, also of a wrong value.
    assert_eq!(
        code(&hw(json!(["shell", "shell"]))),
        Err("einst.kennung_doppelt")
    );
    assert_eq!(
        code(&hw(json!(["yolo", "yolo"]))),
        Err("einst.kennung_doppelt")
    );
    // Stage 34.
    for wert in [
        "yolo",
        "Shell",
        "",
        "shell(git:*)",
        "allow-all-tools",
        "allow-all",
    ] {
        assert_eq!(
            code(&hw(json!(["shell", wert]))),
            Err("einst.werkzeug_wert"),
            "{wert}"
        );
    }
    // After every other stage, also after the stages of the block `mcp`.
    let mut d = hw(json!(["yolo"]));
    d["mcp"]["tickets"] = json!({"lesen": ["x"], "schreiben": ["x"]});
    assert_eq!(code(&d), Err("einst.mcp_doppelt"));
    d["mcp"]["tickets"]["frist_ms"] = json!(0);
    assert_eq!(code(&d), Err("einst.frist_bereich"));
    let mut d = hw(json!(["yolo"]));
    d["routen"][0]["netzziele"][0]["host"] = json!("localhost");
    assert_eq!(code(&d), Err("einst.netzziel_form"));
}
