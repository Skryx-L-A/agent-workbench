//! The visible cases of contract 6 (`rust/spec/stelle/faelle/`): every case, every step. Each step checks the
//! whole answer, the lines byte for byte with their sha256, and that `entscheide` leaves the given state as it
//! was; each case checks the state at its end. Steps of the type `neustart` load a fresh state.

use std::fs;
use std::path::PathBuf;

use formate::kette::NULL_HASH;
use serde_json::Value;
use stelle::{Aufrufer, Ereignis, PrueferH1, Vorgang, Zustand, entscheide, laden};

fn ordner() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../spec/stelle/faelle")
}

fn faelle() -> Vec<(String, Value)> {
    let mut namen: Vec<PathBuf> = fs::read_dir(ordner())
        .expect("the case folder is readable")
        .map(|e| e.expect("an entry").path())
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .collect();
    namen.sort();
    namen
        .into_iter()
        .map(|p| {
            let text = fs::read_to_string(&p).expect("case file is readable");
            let name = p
                .file_stem()
                .expect("a name")
                .to_string_lossy()
                .into_owned();
            (
                name,
                serde_json::from_str(&text).expect("case file is JSON"),
            )
        })
        .collect()
}

fn schluessel(fall: &Value) -> [u8; 32] {
    assert_eq!(fall["pruefer"]["art"], "h1");
    let hex = fall["pruefer"]["schluessel_hex"].as_str().expect("key hex");
    let mut aus = [0u8; 32];
    for (n, ziel) in aus.iter_mut().enumerate() {
        *ziel = u8::from_str_radix(&hex[2 * n..2 * n + 2], 16).expect("hex");
    }
    aus
}

fn aufrufer(text: &str) -> Aufrufer {
    match text {
        "kern" => Aufrufer::Kern,
        "hub" => Aufrufer::Hub,
        "mensch" => Aufrufer::Mensch,
        "agent" => Aufrufer::Agent,
        "takt" => Aufrufer::Takt,
        "unbekannt" => Aufrufer::Unbekannt,
        andere => panic!("unknown caller {andere}"),
    }
}

fn laden_fall(fall: &Value, laden_ms: i64) -> Result<Zustand, stelle::Code> {
    let text = fall["einstellungen"]["text"]
        .as_str()
        .expect("settings text");
    let regeln = fall["worker_regeln"]["text"]
        .as_str()
        .and_then(|t| stelle::worker_regeln_lesen(t.as_bytes()));
    if fall.get("worker_regeln").is_some() {
        assert!(regeln.is_some(), "the rules of a case are valid");
    }
    laden(
        text.as_bytes(),
        laden_ms,
        Box::new(PrueferH1::neu(schluessel(fall))),
    )
    .map(|z| z.mit_worker_regeln(regeln))
}

struct Zahl {
    faelle: usize,
    schritte: usize,
    zeilen: usize,
    neustarts: usize,
    ladefehler: usize,
}

fn fall_pruefen(name: &str, fall: &Value, zahl: &mut Zahl) {
    let laden_ms = fall["laden_ms"].as_i64().expect("laden_ms");
    if let Some(erwartet) = fall.get("erwartet_laden") {
        let code = laden_fall(fall, laden_ms).expect_err(name);
        assert_eq!(code.code(), erwartet["fehler"], "{name}");
        assert_eq!(code.grund(), erwartet["grund"], "{name}");
        assert!(
            fall.get("schritte").is_none(),
            "{name}: no steps after a failed load"
        );
        zahl.ladefehler += 1;
        return;
    }
    let mut z = laden_fall(fall, laden_ms).expect(name);
    let mut vorher = NULL_HASH.to_owned();
    let mut zeilen = 0;
    for (n, schritt) in fall["schritte"]
        .as_array()
        .expect("steps")
        .iter()
        .enumerate()
    {
        let wo = format!("{name} step {}", n + 1);
        if let Some(neustart) = schritt.get("neustart") {
            z = laden_fall(fall, neustart["laden_ms"].as_i64().expect("laden_ms")).expect(&wo);
            assert!(z.sitzungen().is_empty() && z.anfragen().is_empty(), "{wo}");
            zahl.neustarts += 1;
            continue;
        }
        let vorgang = Vorgang::aus_name(
            schritt["vorgang"].as_str().expect("operation"),
            schritt["eingabe"].clone(),
        )
        .expect("known operation");
        let ereignis = Ereignis {
            aufrufer: aufrufer(schritt["aufrufer"].as_str().expect("caller")),
            vorgang,
        };
        let jetzt = schritt["jetzt_ms"].as_i64().expect("jetzt_ms");
        let vorher_abbild = z.abbild();
        let r = entscheide(&z, &ereignis, jetzt);
        assert_eq!(
            z.abbild(),
            vorher_abbild,
            "{wo}: entscheide changed the given state"
        );
        let erwartet = &schritt["erwartet"];
        assert_eq!(r.antwort.json(), erwartet["antwort"], "{wo}: answer");
        assert_eq!(
            r.antwort.ok(),
            erwartet["antwort"]["ok"] == true,
            "{wo}: ok"
        );
        let texte = r.zeilen_texte(&vorher).expect(&wo);
        let soll = erwartet["zeilen"].as_array().expect("lines");
        assert_eq!(texte.len(), soll.len(), "{wo}: number of lines");
        for (text, s) in texte.iter().zip(soll) {
            assert_eq!(text, s["text"].as_str().expect("text"), "{wo}: line bytes");
            vorher = formate::kette::sha256_hex(text.as_bytes());
            assert_eq!(
                vorher,
                s["sha256"].as_str().expect("sha256"),
                "{wo}: line hash"
            );
        }
        zeilen += texte.len();
        zahl.schritte += 1;
        z = r.zustand;
    }
    let ende = &fall["ende"];
    let sitzungen: Vec<Value> = z.sitzungen().into_iter().map(Value::from).collect();
    assert_eq!(
        Value::from(sitzungen),
        ende["sitzungen"],
        "{name}: sessions"
    );
    let anfragen: serde_json::Map<String, Value> = z
        .anfragen()
        .into_iter()
        .map(|(k, a)| (k, Value::from(a.text())))
        .collect();
    assert_eq!(
        Value::Object(anfragen),
        ende["anfragen"],
        "{name}: requests"
    );
    assert_eq!(Value::from(z.zaehler()), ende["zaehler"], "{name}: counter");
    assert_eq!(Value::from(zeilen), ende["zeilen"], "{name}: lines");
    let letzte = if zeilen == 0 {
        Value::Null
    } else {
        Value::from(vorher)
    };
    assert_eq!(letzte, ende["letzte"], "{name}: last hash");
    zahl.zeilen += zeilen;
}

#[test]
fn alle_sichtbaren_faelle() {
    let mut zahl = Zahl {
        faelle: 0,
        schritte: 0,
        zeilen: 0,
        neustarts: 0,
        ladefehler: 0,
    };
    for (name, fall) in faelle() {
        fall_pruefen(&name, &fall, &mut zahl);
        zahl.faelle += 1;
    }
    // The cases as `bau-faelle.mjs` builds them (version 1 of the contract): no case may go missing.
    assert_eq!(zahl.faelle, 51, "cases");
    assert_eq!(zahl.ladefehler, 6, "cases with a failed load");
    assert_eq!(zahl.neustarts, 2, "restarts");
    assert_eq!(zahl.schritte, 255, "steps");
    assert_eq!(zahl.zeilen, 182, "lines");
}

#[test]
fn kein_fall_haengt_von_der_uhr_ab() {
    // Every step names its time; running a case twice gives the same bytes.
    for (name, fall) in faelle() {
        for schritt in fall
            .get("schritte")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            assert!(
                schritt.get("neustart").is_some() || schritt["jetzt_ms"].is_i64(),
                "{name}: a step without jetzt_ms"
            );
        }
    }
}
