//! The settings reader of `rangkern sitzung`: the cases of contract 2, the bounds of every list, every level of the
//! closed form, and the settings the gate gets from a valid file.

use std::fs;
use std::path::Path;

use datentor::{Einheit, darf};
use serde_json::{Value, json};

use super::TempDir;
use crate::sitzung::einst::{lesen, lesen_datei};

/// A valid file with every kind of entry: two routes, a customer with a release, one with a forbidden route.
fn gueltig() -> Value {
    json!({
        "fassung": 1,
        "klassen": ["oeffentlich", "intern", "kundendaten"],
        "vorgabestufe": "intern",
        "routen": [
            {"schluessel": "cloud", "art": "cloud", "klasse": "intern"},
            {"schluessel": "cloud2", "art": "cloud", "klasse": "kundendaten"},
            {"schluessel": "lokal", "art": "lokal", "klasse": "kundendaten"}
        ],
        "kunden": [
            {"schluessel": "k-a", "erlaubt": ["lokal", "cloud2"], "verboten": []},
            {"schluessel": "k-b", "erlaubt": ["lokal"], "verboten": ["lokal"]},
            {"schluessel": "k-c"}
        ],
        "kundendaten_an_cloud": {"vorgabe": "nie", "freigaben": [{"route": "cloud2", "kunde": "k-a"}]}
    })
}

fn code(wert: &Value) -> Option<&'static str> {
    lesen(wert.to_string().as_bytes()).err().map(|f| f.code)
}

/// The code of the file after `aendern` has changed the valid one.
fn nach(aendern: impl FnOnce(&mut Value)) -> Option<&'static str> {
    let mut wert = gueltig();
    aendern(&mut wert);
    code(&wert)
}

fn base64(text: &str) -> Vec<u8> {
    let mut out = Vec::new();
    let (mut sammler, mut bits) = (0_u32, 0);
    for zeichen in text.bytes().filter(|b| *b != b'=') {
        let wert = match zeichen {
            b'A'..=b'Z' => zeichen - b'A',
            b'a'..=b'z' => zeichen - b'a' + 26,
            b'0'..=b'9' => zeichen - b'0' + 52,
            b'+' => 62,
            _ => 63,
        };
        sammler = (sammler << 6) | u32::from(wert);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push(u8::try_from((sammler >> bits) & 0xff).expect("a byte"));
        }
    }
    out
}

#[test]
fn every_case_of_the_contract_has_its_code() {
    let ordner = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../spec/einstellungen");
    let mut gezaehlt = 0;
    for eintrag in fs::read_dir(ordner.join("faelle")).expect("the cases are there") {
        let datei = eintrag.expect("an entry").path();
        let name = datei
            .file_stem()
            .expect("a name")
            .to_string_lossy()
            .to_string();
        let fall: Value = serde_json::from_slice(&fs::read(&datei).expect("case")).expect("JSON");
        let bytes = match (fall.get("text"), fall.get("b64")) {
            (Some(text), _) => text.as_str().expect("text").as_bytes().to_vec(),
            (None, Some(b64)) => base64(b64.as_str().expect("b64")),
            _ => panic!("case {name} has neither text nor b64"),
        };
        let erwartet: Value = serde_json::from_slice(
            &fs::read(ordner.join("erwartet").join(format!("{name}.json"))).expect("expected"),
        )
        .expect("JSON");
        let ist = lesen(&bytes).err().map(|f| f.code);
        let soll = erwartet.get("code").and_then(Value::as_str);
        assert_eq!(ist, soll, "case {name}");
        gezaehlt += 1;
    }
    // Every file in faelle/ is read and checked above, Fassung 1 and 2 alike; new cases of the contract keep this test green.
    // The minimum guards against an empty or misplaced folder (stand 21.11.2026: 180 case files).
    let dateien = fs::read_dir(ordner.join("faelle"))
        .expect("the cases are there")
        .count();
    assert_eq!(gezaehlt, dateien, "every case file is counted once");
    assert!(gezaehlt >= 180, "too few cases: {gezaehlt}");
}

#[test]
fn a_valid_file_gives_the_routes_and_the_gate_of_its_settings() {
    let einst = lesen(gueltig().to_string().as_bytes()).expect("valid");
    assert_eq!(einst.routen, ["cloud", "cloud2", "lokal"]);
    let frage = |klasse: Option<&str>, route: &str, kunde: Option<&str>| {
        darf(&einst.vorgabe, &Einheit { klasse }, Some(route), kunde)
            .grund
            .code()
    };
    // The class of a route, and the vorgabestufe `intern` for a unit without a class.
    assert_eq!(frage(Some("intern"), "cloud", None), "frei");
    assert_eq!(
        frage(Some("kundendaten"), "cloud", None),
        "klasse-ueber-route"
    );
    assert_eq!(frage(None, "cloud", None), "frei");
    assert_eq!(frage(Some("oeffentlich"), "lokal", None), "frei");
    // A customer: allowed on `lokal` (a local route), `cloud2` needs the release, which `k-a` has.
    assert_eq!(frage(Some("intern"), "lokal", Some("k-a")), "frei");
    assert_eq!(frage(Some("intern"), "cloud2", Some("k-a")), "frei");
    assert_eq!(
        frage(Some("intern"), "cloud", Some("k-a")),
        "klasse-ueber-route"
    );
    // `verboten` goes before `erlaubt`; a customer without `erlaubt` gets no route; an unknown customer has no entry.
    assert_eq!(
        frage(Some("intern"), "lokal", Some("k-b")),
        "route-fuer-kunde-verboten"
    );
    assert_eq!(
        frage(Some("intern"), "lokal", Some("k-c")),
        "route-fuer-kunde-nicht-erlaubt"
    );
    assert_eq!(
        frage(Some("intern"), "lokal", Some("k-x")),
        "kunde-ohne-eintrag"
    );
    // Without the release a cloud route does not get the customer data of the customer.
    let ohne = {
        let mut wert = gueltig();
        wert["kundendaten_an_cloud"]["freigaben"] = json!([]);
        lesen(wert.to_string().as_bytes()).expect("valid")
    };
    assert_eq!(
        darf(
            &ohne.vorgabe,
            &Einheit {
                klasse: Some("intern")
            },
            Some("cloud2"),
            Some("k-a")
        )
        .grund
        .code(),
        "kundendaten-an-cloud"
    );
    // Without `vorgabestufe` the highest class is the one of a unit without a class.
    let hoechste = {
        let mut wert = gueltig();
        wert.as_object_mut().expect("object").remove("vorgabestufe");
        lesen(wert.to_string().as_bytes()).expect("valid")
    };
    assert_eq!(
        darf(
            &hoechste.vorgabe,
            &Einheit { klasse: None },
            Some("cloud"),
            None
        )
        .grund
        .code(),
        "klasse-ueber-route"
    );
}

#[test]
fn the_settings_of_the_demonstration_give_the_routes_and_the_gate() {
    let ordner = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../spec/einstellungen");
    let einst = lesen_datei(&ordner.join("vorfuehrung.json")).expect("Fassung 2 is read");
    assert_eq!(einst.routen, ["copilot", "lokal"]);
    let frage = |klasse: Option<&str>, route: &str, kunde: Option<&str>| {
        darf(&einst.vorgabe, &Einheit { klasse }, Some(route), kunde)
            .grund
            .code()
    };
    assert_eq!(frage(Some("internal"), "copilot", None), "frei");
    assert_eq!(
        frage(Some("confidential"), "copilot", None),
        "klasse-ueber-route"
    );
    assert_eq!(frage(Some("customer"), "lokal", Some("k-nord")), "frei");
    // The vorgabestufe is `customer`, the highest class: a unit without a class does not go to the cloud route.
    assert_eq!(frage(None, "copilot", None), "klasse-ueber-route");
    assert_eq!(
        frage(Some("customer"), "lokal", Some("k-sued")),
        "route-fuer-kunde-nicht-erlaubt"
    );
}

#[test]
fn the_file_is_read_from_the_disk_with_its_size_limit() {
    let dir = TempDir::new();
    let text = gueltig().to_string();
    let mit_laenge = |laenge: usize| {
        let mut bytes = text.clone().into_bytes();
        bytes.resize(laenge, b' ');
        bytes
    };
    let pfad = dir.write("einst.json", &mit_laenge(65_536));
    assert!(lesen_datei(&pfad).is_ok());
    let pfad = dir.write("einst.json", &mit_laenge(65_537));
    assert_eq!(
        lesen_datei(&pfad).err().map(|f| f.code),
        Some("einst.zu_gross")
    );
    assert_eq!(
        lesen_datei(&dir.path().join("fehlt.json"))
            .err()
            .map(|f| f.code),
        Some("lauf.einst_unlesbar")
    );
    assert_eq!(
        lesen_datei(dir.path()).err().map(|f| f.code),
        Some("lauf.einst_unlesbar")
    );
    // The reason is fixed text without a path.
    let fehler = lesen_datei(&dir.path().join("fehlt.json")).expect_err("missing");
    assert_eq!(fehler.grund, "Einstellungsdatei nicht lesbar");
    assert_eq!(
        fehler.to_string(),
        "lauf.einst_unlesbar: Einstellungsdatei nicht lesbar"
    );
}

#[test]
fn json_that_is_no_object_is_refused() {
    for bytes in [
        &b""[..],
        b"[]",
        b"null",
        b"1",
        b"\"x\"",
        b"{",
        b"{} {}",
        b"{\"a\":1,\"a\":2}",
        b"\xEF\xBB\xBF{}",
        b"{\"fassung\":1,\"fassung\":1}",
    ] {
        assert_eq!(
            lesen(bytes).err().map(|f| f.code),
            Some("einst.json_ungueltig"),
            "{bytes:?}"
        );
    }
}

#[test]
fn a_repeated_key_is_refused_on_every_level() {
    let text = gueltig().to_string();
    for (suche, ersatz) in [
        ("\"fassung\":1", "\"fassung\":1,\"fassung\":1"),
        (
            "\"schluessel\":\"cloud\"",
            "\"schluessel\":\"cloud\",\"schluessel\":\"cloud\"",
        ),
        (
            "\"vorgabe\":\"nie\"",
            "\"vorgabe\":\"nie\",\"vorgabe\":\"nie\"",
        ),
        (
            "\"route\":\"cloud2\"",
            "\"route\":\"cloud2\",\"route\":\"cloud2\"",
        ),
    ] {
        assert!(text.contains(suche), "{suche}");
        let doppelt = text.replacen(suche, ersatz, 1);
        assert_eq!(
            lesen(doppelt.as_bytes()).err().map(|f| f.code),
            Some("einst.json_ungueltig"),
            "{ersatz}"
        );
    }
}

#[test]
fn a_key_outside_the_form_is_refused_on_every_level() {
    assert_eq!(
        nach(|w| w["x"] = json!(1)),
        Some("einst.schluessel_unbekannt")
    );
    assert_eq!(
        nach(|w| w["routen"][0]["x"] = json!(1)),
        Some("einst.schluessel_unbekannt")
    );
    assert_eq!(
        nach(|w| w["kunden"][2]["x"] = json!(1)),
        Some("einst.schluessel_unbekannt")
    );
    assert_eq!(
        nach(|w| w["kundendaten_an_cloud"]["x"] = json!(1)),
        Some("einst.schluessel_unbekannt")
    );
    assert_eq!(
        nach(|w| w["kundendaten_an_cloud"]["freigaben"][0]["x"] = json!(1)),
        Some("einst.schluessel_unbekannt")
    );
    // The key of another level is unknown too.
    assert_eq!(
        nach(|w| w["routen"][0]["erlaubt"] = json!([])),
        Some("einst.schluessel_unbekannt")
    );
    assert_eq!(
        nach(|w| w["kunden"][0]["klasse"] = json!("intern")),
        Some("einst.schluessel_unbekannt")
    );
}

#[test]
fn a_route_without_a_class_comes_before_every_other_missing_key() {
    assert_eq!(
        nach(|w| {
            w["routen"][1]
                .as_object_mut()
                .expect("object")
                .remove("klasse");
        }),
        Some("einst.route_ohne_klasse")
    );
    assert_eq!(
        nach(|w| w["routen"][1]["klasse"] = Value::Null),
        Some("einst.route_ohne_klasse")
    );
    assert_eq!(
        nach(|w| {
            w["routen"][1]
                .as_object_mut()
                .expect("object")
                .remove("klasse");
            w.as_object_mut().expect("object").remove("kunden");
        }),
        Some("einst.route_ohne_klasse")
    );
}

#[test]
fn a_missing_required_key_is_refused_on_every_level() {
    for feld in [
        "fassung",
        "klassen",
        "routen",
        "kunden",
        "kundendaten_an_cloud",
    ] {
        assert_eq!(
            nach(|w| {
                w.as_object_mut().expect("object").remove(feld);
            }),
            Some("einst.schluessel_fehlt"),
            "{feld}"
        );
    }
    for feld in ["schluessel", "art"] {
        assert_eq!(
            nach(|w| {
                w["routen"][0].as_object_mut().expect("object").remove(feld);
            }),
            Some("einst.schluessel_fehlt"),
            "route {feld}"
        );
    }
    assert_eq!(
        nach(|w| {
            w["kunden"][0]
                .as_object_mut()
                .expect("object")
                .remove("schluessel");
        }),
        Some("einst.schluessel_fehlt")
    );
    for feld in ["vorgabe", "freigaben"] {
        assert_eq!(
            nach(|w| {
                w["kundendaten_an_cloud"]
                    .as_object_mut()
                    .expect("object")
                    .remove(feld);
            }),
            Some("einst.schluessel_fehlt"),
            "cloud {feld}"
        );
    }
    for feld in ["route", "kunde"] {
        assert_eq!(
            nach(|w| {
                w["kundendaten_an_cloud"]["freigaben"][0]
                    .as_object_mut()
                    .expect("object")
                    .remove(feld);
            }),
            Some("einst.schluessel_fehlt"),
            "freigabe {feld}"
        );
    }
    // `vorgabestufe`, `erlaubt` and `verboten` are optional.
    assert_eq!(
        nach(|w| {
            w.as_object_mut().expect("object").remove("vorgabestufe");
        }),
        None
    );
}

#[test]
fn a_wrong_json_type_is_refused_everywhere() {
    let falsch = Some("einst.typ_falsch");
    assert_eq!(nach(|w| w["fassung"] = json!("1")), falsch);
    assert_eq!(nach(|w| w["fassung"] = json!(1.0)), falsch);
    assert_eq!(nach(|w| w["klassen"] = json!("intern")), falsch);
    assert_eq!(nach(|w| w["klassen"] = json!(["intern", 1])), falsch);
    assert_eq!(nach(|w| w["vorgabestufe"] = Value::Null), falsch);
    assert_eq!(nach(|w| w["vorgabestufe"] = json!(1)), falsch);
    assert_eq!(nach(|w| w["routen"] = json!({})), falsch);
    assert_eq!(nach(|w| w["routen"] = json!([1])), falsch);
    assert_eq!(nach(|w| w["kunden"] = json!({})), falsch);
    assert_eq!(nach(|w| w["kunden"] = json!(["k"])), falsch);
    assert_eq!(nach(|w| w["kundendaten_an_cloud"] = json!("nie")), falsch);
    for feld in ["schluessel", "art", "klasse"] {
        assert_eq!(nach(|w| w["routen"][0][feld] = json!(1)), falsch, "{feld}");
    }
    assert_eq!(nach(|w| w["kunden"][0]["schluessel"] = json!(1)), falsch);
    assert_eq!(nach(|w| w["kunden"][0]["erlaubt"] = json!("lokal")), falsch);
    assert_eq!(nach(|w| w["kunden"][0]["erlaubt"] = json!([1])), falsch);
    assert_eq!(
        nach(|w| w["kunden"][0]["verboten"] = json!("lokal")),
        falsch
    );
    assert_eq!(nach(|w| w["kunden"][0]["verboten"] = json!([1])), falsch);
    assert_eq!(
        nach(|w| w["kundendaten_an_cloud"]["vorgabe"] = json!(true)),
        falsch
    );
    assert_eq!(
        nach(|w| w["kundendaten_an_cloud"]["freigaben"] = json!({})),
        falsch
    );
    assert_eq!(
        nach(|w| w["kundendaten_an_cloud"]["freigaben"] = json!([1])),
        falsch
    );
    assert_eq!(
        nach(|w| w["kundendaten_an_cloud"]["freigaben"][0]["route"] = json!(1)),
        falsch
    );
    assert_eq!(
        nach(|w| w["kundendaten_an_cloud"]["freigaben"][0]["kunde"] = json!(1)),
        falsch
    );
}

#[test]
fn only_the_whole_numbers_one_and_two_are_versions() {
    // Changed with the crate `einstellungen`: 2 is a version too. The content of this file has no `netzziele` and no `ziel`,
    // which version 2 requires; the old reader answered `einst.fassung_unbekannt` for the number alone.
    assert_eq!(
        nach(|w| w["fassung"] = json!(2)),
        Some("einst.schluessel_fehlt")
    );
    assert_eq!(
        nach(|w| w["fassung"] = json!(3)),
        Some("einst.fassung_unbekannt")
    );
    assert_eq!(
        nach(|w| w["fassung"] = json!(0)),
        Some("einst.fassung_unbekannt")
    );
    assert_eq!(
        nach(|w| w["fassung"] = json!(-1)),
        Some("einst.fassung_unbekannt")
    );
    assert_eq!(
        nach(|w| w["fassung"] = json!(18_446_744_073_709_551_615_u64)),
        Some("einst.fassung_unbekannt")
    );
    assert_eq!(nach(|w| w["fassung"] = json!(1)), None);
}

/// A list of `n` keys made up for a bound.
fn schluessel(n: usize) -> Vec<String> {
    (0..n).map(|i| format!("s{i}")).collect()
}

#[test]
fn every_list_has_its_bounds() {
    let anzahl = Some("einst.anzahl");
    // klassen: 1 to 16.
    assert_eq!(
        nach(|w| {
            w["klassen"] = json!(schluessel(16));
            w["vorgabestufe"] = json!("s0");
            w["routen"] = json!([{"schluessel": "r", "art": "cloud", "klasse": "s15"}]);
            w["kunden"] = json!([]);
            w["kundendaten_an_cloud"]["freigaben"] = json!([]);
        }),
        None
    );
    assert_eq!(
        nach(|w| {
            w["klassen"] = json!(schluessel(17));
            w["vorgabestufe"] = json!("s0");
            w["routen"] = json!([{"schluessel": "r", "art": "cloud", "klasse": "s15"}]);
            w["kunden"] = json!([]);
            w["kundendaten_an_cloud"]["freigaben"] = json!([]);
        }),
        anzahl
    );
    assert_eq!(
        nach(|w| {
            w["klassen"] = json!(["a"]);
            w["vorgabestufe"] = json!("a");
            w["routen"] = json!([{"schluessel": "r", "art": "cloud", "klasse": "a"}]);
            w["kunden"] = json!([]);
            w["kundendaten_an_cloud"]["freigaben"] = json!([]);
        }),
        None
    );
    assert_eq!(nach(|w| w["klassen"] = json!([])), anzahl);
    // routen: 1 to 64.
    let routen = |n: usize| -> Value {
        Value::Array(
            schluessel(n)
                .into_iter()
                .map(|s| json!({"schluessel": s, "art": "cloud", "klasse": "intern"}))
                .collect(),
        )
    };
    let ohne_verweise = |w: &mut Value, n: usize| {
        w["routen"] = routen(n);
        w["kunden"] = json!([]);
        w["kundendaten_an_cloud"]["freigaben"] = json!([]);
    };
    assert_eq!(nach(|w| ohne_verweise(w, 64)), None);
    assert_eq!(nach(|w| ohne_verweise(w, 65)), anzahl);
    assert_eq!(nach(|w| ohne_verweise(w, 1)), None);
    assert_eq!(nach(|w| ohne_verweise(w, 0)), anzahl);
    // kunden: 0 to 256.
    let kunden = |w: &mut Value, n: usize| {
        w["kunden"] = Value::Array(
            schluessel(n)
                .into_iter()
                .map(|s| json!({"schluessel": s}))
                .collect(),
        );
        w["kundendaten_an_cloud"]["freigaben"] = json!([]);
    };
    assert_eq!(nach(|w| kunden(w, 256)), None);
    assert_eq!(nach(|w| kunden(w, 257)), anzahl);
    assert_eq!(nach(|w| kunden(w, 0)), None);
    // erlaubt and verboten: 0 to 64 entries each, with 64 routes.
    let mit_listen = |w: &mut Value, feld: &str, n: usize| {
        w["routen"] = routen(64);
        w["kunden"] = json!([{"schluessel": "k", feld: schluessel(n)}]);
        w["kundendaten_an_cloud"]["freigaben"] = json!([]);
    };
    for feld in ["erlaubt", "verboten"] {
        assert_eq!(nach(|w| mit_listen(w, feld, 64)), None, "{feld}");
        assert_eq!(nach(|w| mit_listen(w, feld, 65)), anzahl, "{feld}");
    }
    // freigaben: 0 to 1024 pairs, which needs 1024 customers and cloud routes: 64 routes times 16 customers.
    let freigaben = |w: &mut Value, n: usize| {
        w["routen"] = routen(64);
        w["kunden"] = Value::Array(
            schluessel(17)
                .into_iter()
                .map(|s| json!({"schluessel": s}))
                .collect(),
        );
        let paare: Vec<Value> = (0..64)
            .flat_map(|r| {
                (0..17).map(move |k| json!({"route": format!("s{r}"), "kunde": format!("s{k}")}))
            })
            .take(n)
            .collect();
        w["kundendaten_an_cloud"]["freigaben"] = Value::Array(paare);
    };
    assert_eq!(nach(|w| freigaben(w, 1024)), None);
    assert_eq!(nach(|w| freigaben(w, 1025)), anzahl);
}

#[test]
fn a_key_has_the_key_form_everywhere() {
    let form = Some("einst.kennung_form");
    let lang = "a".repeat(64);
    let zu_lang = "a".repeat(65);
    for schlecht in ["", "A", "-a", "a b", "ä", zu_lang.as_str()] {
        assert_eq!(
            nach(|w| w["klassen"][0] = json!(schlecht)),
            form,
            "{schlecht:?}"
        );
        assert_eq!(
            nach(|w| w["routen"][0]["schluessel"] = json!(schlecht)),
            form,
            "{schlecht:?}"
        );
        assert_eq!(
            nach(|w| w["routen"][0]["klasse"] = json!(schlecht)),
            form,
            "{schlecht:?}"
        );
        assert_eq!(
            nach(|w| w["kunden"][0]["schluessel"] = json!(schlecht)),
            form,
            "{schlecht:?}"
        );
        assert_eq!(
            nach(|w| w["kunden"][0]["erlaubt"][0] = json!(schlecht)),
            form,
            "{schlecht:?}"
        );
        assert_eq!(
            nach(|w| w["kunden"][0]["verboten"] = json!([schlecht])),
            form,
            "{schlecht:?}"
        );
        assert_eq!(
            nach(|w| w["vorgabestufe"] = json!(schlecht)),
            form,
            "{schlecht:?}"
        );
        assert_eq!(
            nach(|w| w["kundendaten_an_cloud"]["freigaben"][0]["route"] = json!(schlecht)),
            form,
            "{schlecht:?}"
        );
        assert_eq!(
            nach(|w| w["kundendaten_an_cloud"]["freigaben"][0]["kunde"] = json!(schlecht)),
            form,
            "{schlecht:?}"
        );
    }
    assert_eq!(nach(|w| w["kunden"][2]["schluessel"] = json!(lang)), None);
}

#[test]
fn a_key_twice_in_its_list_is_refused() {
    let doppelt = Some("einst.kennung_doppelt");
    assert_eq!(
        nach(|w| w["klassen"] = json!(["intern", "intern", "kundendaten"])),
        doppelt
    );
    assert_eq!(
        nach(|w| w["routen"][1]["schluessel"] = json!("cloud")),
        doppelt
    );
    assert_eq!(
        nach(|w| w["kunden"][1]["schluessel"] = json!("k-a")),
        doppelt
    );
    assert_eq!(
        nach(|w| w["kunden"][0]["erlaubt"] = json!(["lokal", "lokal"])),
        doppelt
    );
    assert_eq!(
        nach(|w| w["kunden"][0]["verboten"] = json!(["lokal", "lokal"])),
        doppelt
    );
    assert_eq!(
        nach(|w| {
            let paar = w["kundendaten_an_cloud"]["freigaben"][0].clone();
            w["kundendaten_an_cloud"]["freigaben"] = json!([paar.clone(), paar]);
        }),
        doppelt
    );
    // The same route with two customers, and the same customer with two routes, are no duplicates.
    assert_eq!(
        nach(|w| {
            w["kunden"][1]["erlaubt"] = json!(["lokal", "cloud2"]);
            w["kundendaten_an_cloud"]["freigaben"] = json!([
                {"route": "cloud2", "kunde": "k-a"},
                {"route": "cloud2", "kunde": "k-b"},
                {"route": "cloud", "kunde": "k-a"}
            ]);
        }),
        None
    );
}

#[test]
fn the_references_are_checked_in_the_order_of_the_stages() {
    assert_eq!(
        nach(|w| w["routen"][0]["art"] = json!("wolke")),
        Some("einst.art_unbekannt")
    );
    assert_eq!(
        nach(|w| w["routen"][2]["klasse"] = json!("geheim")),
        Some("einst.klasse_unbekannt")
    );
    assert_eq!(
        nach(|w| w["vorgabestufe"] = json!("geheim")),
        Some("einst.vorgabestufe_unbekannt")
    );
    assert_eq!(
        nach(|w| w["kunden"][0]["erlaubt"] = json!(["lokal", "weg"])),
        Some("einst.route_unbekannt")
    );
    assert_eq!(
        nach(|w| w["kunden"][0]["verboten"] = json!(["weg"])),
        Some("einst.route_unbekannt")
    );
    assert_eq!(
        nach(|w| w["kundendaten_an_cloud"]["freigaben"][0]["route"] = json!("weg")),
        Some("einst.route_unbekannt")
    );
    assert_eq!(
        nach(|w| w["kundendaten_an_cloud"]["freigaben"][0]["kunde"] = json!("k-x")),
        Some("einst.kunde_unbekannt")
    );
    assert_eq!(
        nach(|w| w["kundendaten_an_cloud"]["vorgabe"] = json!("immer")),
        Some("einst.vorgabe_wert")
    );
    assert_eq!(
        nach(|w| w["kundendaten_an_cloud"]["freigaben"][0]["route"] = json!("lokal")),
        Some("einst.freigabe_route_lokal")
    );
    // The first stage that is violated names the code: a kind that is unknown comes before a class that is unknown.
    assert_eq!(
        nach(|w| {
            w["routen"][0]["art"] = json!("wolke");
            w["routen"][2]["klasse"] = json!("geheim");
        }),
        Some("einst.art_unbekannt")
    );
    assert_eq!(
        nach(|w| {
            w["routen"][2]["klasse"] = json!("geheim");
            w["vorgabestufe"] = json!("geheim");
        }),
        Some("einst.klasse_unbekannt")
    );
    assert_eq!(
        nach(|w| {
            w["vorgabestufe"] = json!("geheim");
            w["kunden"][0]["erlaubt"] = json!(["weg"]);
        }),
        Some("einst.vorgabestufe_unbekannt")
    );
    assert_eq!(
        nach(|w| {
            w["kunden"][0]["erlaubt"] = json!(["weg"]);
            w["kundendaten_an_cloud"]["freigaben"][0]["kunde"] = json!("k-x");
        }),
        Some("einst.route_unbekannt")
    );
    assert_eq!(
        nach(|w| {
            w["kundendaten_an_cloud"]["freigaben"][0]["kunde"] = json!("k-x");
            w["kundendaten_an_cloud"]["vorgabe"] = json!("immer");
        }),
        Some("einst.kunde_unbekannt")
    );
    assert_eq!(
        nach(|w| {
            w["kundendaten_an_cloud"]["vorgabe"] = json!("immer");
            w["kundendaten_an_cloud"]["freigaben"][0]["route"] = json!("lokal");
        }),
        Some("einst.vorgabe_wert")
    );
}
