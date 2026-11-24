//! The start of contract 1 beyond the visible cases: the block `marker` in every stage, the harness `copilot` and
//! the model of the route (stage 15).

use serde_json::{Value, json};
use stelle::einstellungen::{self, Einstellungen};
use stelle::start::{self, Form};

fn einstellungen() -> Einstellungen {
    let d = json!({
        "fassung": 2, "klassen": ["oeffentlich", "intern", "kundendaten"], "vorgabestufe": "kundendaten",
        "routen": [
            {"schluessel": "cloud-fest", "art": "cloud", "klasse": "intern", "modell": "gpt-5.6-sol", "netzziele": []},
            {"schluessel": "cloud-frei", "art": "cloud", "klasse": "intern", "netzziele": []},
            {"schluessel": "lokal-beispiel", "art": "lokal", "klasse": "kundendaten",
             "ziel": "http://127.0.0.1:11434/v1", "modell": "qwen3:8b", "netzziele": []}],
        "kunden": [{"schluessel": "k-beispiel", "erlaubt": ["lokal-beispiel"], "verboten": []}],
        "kundendaten_an_cloud": {"vorgabe": "nie", "freigaben": []}
    });
    einstellungen::lesen(d.to_string().as_bytes()).expect("valid settings")
}

fn datei() -> Value {
    json!({"fassung": 1, "sitzung": "s-1", "harness": "pi", "modell": "qwen3:8b", "route": "lokal-beispiel",
           "kunde": "k-beispiel", "arbeitsordner": "/srv/beispiel/projekt",
           "marker": {"liste": ["MK-1", "MK-2"], "ordner": "/srv/beispiel/zaehlung"}})
}

fn mit(mut d: Value, pfad: &[&str], wert: Option<Value>) -> Value {
    let (letzter, weg) = pfad.split_last().expect("a path");
    let mut ort = &mut d;
    for teil in weg {
        ort = &mut ort[*teil];
    }
    match wert {
        Some(w) => ort[*letzter] = w,
        None => {
            ort.as_object_mut().expect("object").remove(*letzter);
        }
    }
    d
}

fn code(d: &Value, form: Form) -> Result<(), &'static str> {
    start::pruefen(d.to_string().as_bytes(), form, &einstellungen())
        .map(|_| ())
        .map_err(|a| a.code.code())
}

#[test]
fn marker_in_jeder_stufe() {
    let m = |pfad: &[&str], wert: Option<Value>| mit(datei(), pfad, wert);
    assert_eq!(code(&datei(), Form::Datei), Ok(()));
    let faelle = [
        (
            m(&["marker", "liste"], Some(json!(["A", 1]))),
            "start.typ_falsch",
        ),
        (
            m(&["marker", "liste"], Some(json!("MK-1"))),
            "start.typ_falsch",
        ),
        (m(&["marker", "ordner"], Some(json!(5))), "start.typ_falsch"),
        (m(&["marker"], Some(json!("x"))), "start.typ_falsch"),
        (m(&["marker"], Some(json!([]))), "start.typ_falsch"),
        (m(&["marker", "ordner"], None), "start.schluessel_fehlt"),
        (m(&["marker", "liste"], None), "start.schluessel_fehlt"),
        (
            m(&["marker", "x"], Some(json!(1))),
            "start.schluessel_unbekannt",
        ),
        (
            m(&["marker", "liste"], Some(json!([]))),
            "start.marker_form",
        ),
        (
            m(&["marker", "liste"], Some(json!(["A", "A"]))),
            "start.marker_form",
        ),
        (
            m(&["marker", "liste"], Some(json!(["a"]))),
            "start.marker_form",
        ),
        (
            m(&["marker", "ordner"], Some(json!("relativ"))),
            "start.marker_form",
        ),
        // Stage order: the working folder (11) comes before the marker (12), the marker before the harness (13).
        (
            mit(
                m(&["marker", "ordner"], Some(json!("relativ"))),
                &["arbeitsordner"],
                Some(json!("relativ")),
            ),
            "start.ordner_ungueltig",
        ),
        (
            mit(
                m(&["marker", "liste"], Some(json!([]))),
                &["harness"],
                Some(json!("unbekannt")),
            ),
            "start.marker_form",
        ),
    ];
    for (d, soll) in faelle {
        assert_eq!(code(&d, Form::Datei), Err(soll), "{d}");
    }
    let sechzehn: Vec<String> = (0..16).map(|i| format!("M{i}")).collect();
    let mut siebzehn = sechzehn.clone();
    siebzehn.push("M16".to_owned());
    assert_eq!(
        code(&m(&["marker", "liste"], Some(json!(sechzehn))), Form::Datei),
        Ok(())
    );
    assert_eq!(
        code(&m(&["marker", "liste"], Some(json!(siebzehn))), Form::Datei),
        Err("start.marker_form")
    );
    assert_eq!(
        code(
            &m(&["marker", "liste"], Some(json!(["A".repeat(32)]))),
            Form::Datei
        ),
        Ok(())
    );
    assert_eq!(
        code(
            &m(&["marker", "liste"], Some(json!(["A".repeat(33)]))),
            Form::Datei
        ),
        Err("start.marker_form")
    );
    // The request of the view may not carry a session and may not carry a marker; the first of the two wins.
    let anfrage = mit(datei(), &["sitzung"], None);
    assert_eq!(
        code(&anfrage, Form::Anfrage),
        Err("start.marker_nicht_erlaubt")
    );
    assert_eq!(
        code(&mit(anfrage, &["marker"], None), Form::Anfrage),
        Ok(())
    );
    assert_eq!(
        code(&datei(), Form::Anfrage),
        Err("start.sitzung_nicht_erlaubt")
    );
}

#[test]
fn copilot_und_modell_der_route() {
    let copilot = |route: &str, modell: &str| {
        mit(
            mit(
                mit(datei(), &["harness"], Some(json!("copilot"))),
                &["route"],
                Some(json!(route)),
            ),
            &["modell"],
            Some(json!(modell)),
        )
    };
    assert_eq!(
        code(&copilot("cloud-fest", "gpt-5.6-sol"), Form::Datei),
        Ok(())
    );
    assert_eq!(
        code(&copilot("cloud-frei", "irgendeins"), Form::Datei),
        Ok(())
    );
    assert_eq!(
        code(&copilot("cloud-fest", "anderes"), Form::Datei),
        Err("start.modell_route")
    );
    assert_eq!(
        code(&copilot("lokal-beispiel", "qwen3:8b"), Form::Datei),
        Err("start.harness_route_unvertraeglich")
    );
    // The local route names its model, too: another model is refused before the harness is compared.
    assert_eq!(
        code(
            &mit(datei(), &["modell"], Some(json!("anderes"))),
            Form::Datei
        ),
        Err("start.modell_route")
    );
    assert_eq!(
        code(
            &mit(datei(), &["harness"], Some(json!("claude"))),
            Form::Datei
        ),
        Err("start.harness_route_unvertraeglich")
    );
    // Stage order: the route (14) before its model (15) before the customer (16) before the harness (17).
    assert_eq!(
        code(
            &mit(
                mit(datei(), &["route"], Some(json!("unbekannt"))),
                &["modell"],
                Some(json!("anderes"))
            ),
            Form::Datei
        ),
        Err("start.route_unbekannt")
    );
    assert_eq!(
        code(
            &mit(
                mit(datei(), &["kunde"], Some(json!("k-fremd"))),
                &["modell"],
                Some(json!("anderes"))
            ),
            Form::Datei
        ),
        Err("start.modell_route")
    );
    assert_eq!(
        code(
            &mit(
                mit(datei(), &["kunde"], Some(json!("k-fremd"))),
                &["harness"],
                Some(json!("claude"))
            ),
            Form::Datei
        ),
        Err("start.kunde_unbekannt")
    );
}

/// A refused start carries the identifier values of the file only up to 4096 bytes; the crate refuses a larger
/// start as `start.zu_gross`, and the line then shows no value.
#[test]
fn die_werte_einer_abgelehnten_datei_gelten_bis_4096_bytes() {
    let rumpf = |fuell: usize| {
        format!(
            "{{\"fassung\":1,\"sitzung\":\"s-1\",\"harness\":\"pi\",\"modell\":\"qwen3:8b\",\"route\":\"lokal-beispiel\",\"kunde\":null,\"arbeitsordner\":\"/srv/beispiel/projekt\",\"x\":\"{}\"}}",
            "a".repeat(fuell)
        )
    };
    let leer = rumpf(0).len();
    let genau = rumpf(4096 - leer);
    assert_eq!(genau.len(), 4096);
    let a =
        start::pruefen(genau.as_bytes(), Form::Datei, &einstellungen()).expect_err("unknown key");
    assert_eq!(a.code.code(), "start.schluessel_unbekannt");
    assert_eq!(a.code.grund(), "unbekannter Schluessel");
    assert_eq!(a.sicht.harness.as_deref(), Some("pi"));
    assert_eq!(a.sicht.route.as_deref(), Some("lokal-beispiel"));
    assert_eq!(a.sicht.sitzung.as_deref(), Some("s-1"));
    let gross = rumpf(4097 - leer);
    let a = start::pruefen(gross.as_bytes(), Form::Datei, &einstellungen()).expect_err("too large");
    assert_eq!(a.code.code(), "start.zu_gross");
    assert_eq!(*a.sicht, start::Sicht::default());
}

/// Every refusal of the crates keeps the code and the reason of the crate; nothing maps one to success.
#[test]
fn jede_ablehnung_behaelt_code_und_grund_des_crates() {
    let e = |bytes: &[u8]| {
        einstellungen::lesen(bytes)
            .map(|_| ())
            .map_err(|c| (c.code(), c.grund()))
    };
    assert_eq!(
        e(b"{"),
        Err(("einst.json_ungueltig", "kein gueltiges JSON"))
    );
    let s = |d: &Value, f| {
        start::pruefen(d.to_string().as_bytes(), f, &einstellungen())
            .map(|_| ())
            .map_err(|a| (a.code.code(), a.code.grund()))
    };
    assert_eq!(
        s(
            &mit(datei(), &["harness"], Some(json!("claude"))),
            Form::Datei
        ),
        Err((
            "start.harness_route_unvertraeglich",
            "Harness passt nicht zur Route"
        ))
    );
    assert_eq!(
        s(&datei(), Form::Anfrage),
        Err(("start.sitzung_nicht_erlaubt", "Sitzung vergibt der Dienst"))
    );
    assert_eq!(s(&datei(), Form::Datei), Ok(()));
}

/// B4 (Review of 23.11.2026): The largest start, 4096 bytes, stands twice in the code: `GROESSE_MAX` of
/// `rust/crates/stelle/src/start.rs` builds the readable values of a refused start, and the limit of
/// `rust/crates/start/src/lesen.rs` refuses with `start.zu_gross`. A shared constant would have to be public in
/// the crate `start`, so the two values are bound by a test instead.
/// `die_werte_einer_abgelehnten_datei_gelten_bis_4096_bytes` does that for the start file; this one does it for
/// the request of the view, which the crate reads with its own function.
#[test]
fn groesse_max_haengt_an_der_grenze_des_crates_start_auch_fuer_die_anfrage() {
    const KOPF: &str = r#"{"fassung":1,"harness":"pi","modell":"qwen3:8b","route":"lokal-beispiel","kunde":"k-beispiel","arbeitsordner":"/srv/beispiel/projekt","unbekannt":""#;
    let text = |n: usize| format!("{KOPF}{}\"}}", "x".repeat(n - KOPF.len() - 2));
    assert_eq!(text(4096).len(), 4096);
    assert_eq!(text(4097).len(), 4097);

    let bis = start::pruefen(text(4096).as_bytes(), Form::Anfrage, &einstellungen())
        .expect_err("an unknown key is refused");
    assert_eq!(bis.code.code(), "start.schluessel_unbekannt");
    assert_eq!(bis.sicht.harness.as_deref(), Some("pi"));
    assert_eq!(bis.sicht.route.as_deref(), Some("lokal-beispiel"));
    assert_eq!(bis.sicht.kunde.as_deref(), Some("k-beispiel"));
    assert_eq!(bis.sicht.sitzung, None);

    let ueber = start::pruefen(text(4097).as_bytes(), Form::Anfrage, &einstellungen())
        .expect_err("a start above the limit is refused");
    assert_eq!(ueber.code.code(), "start.zu_gross");
    assert_eq!(*ueber.sicht, start::Sicht::default());
}
