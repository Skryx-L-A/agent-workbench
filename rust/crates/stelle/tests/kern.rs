//! Limits and seams of the core beyond the visible cases: sizes of the input, bounds of time and deadlines, the
//! defaults of a server, a checker that returns no pseudonym, and the purity of `entscheide`.

use serde_json::{Value, json};
use stelle::{
    Antwort, Aufrufer, Code, Ereignis, Nachweispruefer, PrueferH1, Vorgang, ZEIT_MAX, Zustand,
    entscheide, laden,
};

const LADEN: i64 = 1_795_179_540_000;
const T0: i64 = LADEN + 60_000;

fn einstellungen() -> String {
    json!({
        "fassung": 2, "klassen": ["oeffentlich", "intern", "kundendaten"], "vorgabestufe": "kundendaten",
        "routen": [{"schluessel": "cloud-beispiel", "art": "cloud", "klasse": "intern",
                    "netzziele": [{"host": "api.beispiel.invalid", "port": 443, "zugriff": "kopf-einsetzen", "schluessel": true,
                                   "pfade": [{"methode": "GET", "pfad": "/modelle/*"}]}]},
                   {"schluessel": "lokal-beispiel", "art": "lokal", "klasse": "kundendaten",
                    "ziel": "http://127.0.0.1:11434/v1", "modell": "qwen3:8b", "netzziele": []}],
        "kunden": [{"schluessel": "k-beispiel", "erlaubt": ["lokal-beispiel"], "verboten": []}],
        "kundendaten_an_cloud": {"vorgabe": "nie", "freigaben": []},
        "mcp": {"tickets": {"lesen": ["get"], "schreiben": ["update"], "freigabe": true, "frist_ms": 60000, "einloesefrist_ms": 30000},
                "notizen": {"lesen": [], "schreiben": ["anlegen"], "freigabe": false},
                "such-mcp": {"lesen": ["brain_suche"], "schreiben": []}}
    })
    .to_string()
}

/// A checker that accepts every proof and returns its text as the pseudonym.
struct Echo;

impl Nachweispruefer for Echo {
    fn pruefe(&self, _herausforderung: &[u8], nachweis: &str) -> Result<String, ()> {
        Ok(nachweis.to_owned())
    }
}

fn person() -> String {
    format!("ps_{}", "0".repeat(31) + "1")
}

fn gestartet(pruefer: Box<dyn Nachweispruefer>) -> Zustand {
    let z = laden(einstellungen().as_bytes(), LADEN, pruefer).expect("load");
    let start = json!({"fassung": 1, "sitzung": "s-1", "harness": "claude", "modell": "m", "route": "cloud-beispiel",
                       "kunde": null, "arbeitsordner": "/a"});
    let r = schritt(
        &z,
        Aufrufer::Kern,
        "sitzung.starten",
        json!({"start": {"text": start.to_string()}}),
        T0,
    );
    assert_eq!(r.antwort.json()["ergebnis"], "gestartet");
    r.zustand
}

fn schritt(
    z: &Zustand,
    aufrufer: Aufrufer,
    vorgang: &str,
    eingabe: Value,
    jetzt: i64,
) -> stelle::Ergebnis {
    let ereignis = Ereignis {
        aufrufer,
        vorgang: Vorgang::aus_name(vorgang, eingabe).expect("known operation"),
    };
    let vorher = z.abbild();
    let r = entscheide(z, &ereignis, jetzt);
    assert_eq!(z.abbild(), vorher, "entscheide changed the given state");
    r
}

fn anfrage(o: &Value) -> Value {
    let mut e = json!({"argumente": {"id": 1}, "frist_ms": 60000, "person": person(), "server": "tickets",
                       "sitzung": "s-1", "vorschau": null, "werkzeug": "update"});
    for (k, v) in o.as_object().expect("object") {
        e[k] = v.clone();
    }
    e
}

fn code(r: &stelle::Ergebnis) -> Option<&'static str> {
    match r.antwort {
        Antwort::Nein(c) => Some(c.code()),
        _ => None,
    }
}

#[test]
fn groessen_der_eingabe() {
    let z = gestartet(Box::new(PrueferH1::neu([1; 32])));
    // {"a":"…"} in RFC 8785 is the text plus 8 bytes.
    let arg = |n: usize| json!({"argumente": {"a": "x".repeat(n - 8)}});
    let r = schritt(
        &z,
        Aufrufer::Hub,
        "mcp.anfragen",
        anfrage(&arg(1_048_576)),
        T0,
    );
    assert_eq!(r.antwort.json()["ergebnis"], "offen");
    let r = schritt(
        &z,
        Aufrufer::Hub,
        "mcp.anfragen",
        anfrage(&arg(1_048_577)),
        T0,
    );
    assert_eq!(code(&r), Some("stelle.eingabe_ungueltig"));
    let r = schritt(
        &z,
        Aufrufer::Hub,
        "mcp.anfragen",
        anfrage(&json!({"vorschau": "v".repeat(65_536)})),
        T0,
    );
    assert_eq!(r.antwort.json()["ergebnis"], "offen");
    let r = schritt(
        &z,
        Aufrufer::Hub,
        "mcp.anfragen",
        anfrage(&json!({"vorschau": "v".repeat(65_537)})),
        T0,
    );
    assert_eq!(code(&r), Some("stelle.eingabe_ungueltig"));
    let r = schritt(
        &z,
        Aufrufer::Hub,
        "mcp.anfragen",
        anfrage(&json!({"vorschau": 5})),
        T0,
    );
    assert_eq!(code(&r), Some("stelle.eingabe_ungueltig"));
    let r = schritt(
        &z,
        Aufrufer::Hub,
        "mcp.anfragen",
        anfrage(&json!({"argumente": "x"})),
        T0,
    );
    assert_eq!(code(&r), Some("stelle.eingabe_ungueltig"));
}

#[test]
fn formen_der_namen() {
    let z = gestartet(Box::new(PrueferH1::neu([1; 32])));
    for o in [
        json!({"server": "Tickets"}),
        json!({"server": 5}),
        json!({"sitzung": "S 1"}),
        json!({"werkzeug": "up date"}),
        json!({"werkzeug": "_update"}),
        json!({"werkzeug": 5}),
        json!({"person": "ps_kurz"}),
    ] {
        let r = schritt(&z, Aufrufer::Hub, "mcp.anfragen", anfrage(&o), T0);
        assert_eq!(code(&r), Some("stelle.eingabe_ungueltig"), "{o}");
    }
    let r = schritt(&z, Aufrufer::Hub, "mcp.anfragen", anfrage(&json!({})), T0);
    let kennung = r.antwort.json()["anfrage"].clone();
    for (k, v) in [("server", "Tickets"), ("werkzeug", "up date")] {
        let mut e = json!({"anfrage": kennung, "argumente": {"id": 1}, "server": "tickets", "werkzeug": "update"});
        e[k] = json!(v);
        let r = schritt(&r.zustand, Aufrufer::Hub, "mcp.einloesen", e, T0);
        assert_eq!(code(&r), Some("stelle.eingabe_ungueltig"), "{k}");
    }
}

#[test]
fn grenzen_der_frist() {
    let z = gestartet(Box::new(PrueferH1::neu([1; 32])));
    for (frist, soll) in [
        (json!(0), Some("stelle.eingabe_ungueltig")),
        (json!(1), None),
        (json!(60_000), None),
        (json!(60_001), Some("stelle.frist_ungueltig")),
        (json!(86_400_000), Some("stelle.frist_ungueltig")),
        (json!(86_400_001), Some("stelle.eingabe_ungueltig")),
        (json!(1.0), Some("stelle.eingabe_ungueltig")),
        (json!("1"), Some("stelle.eingabe_ungueltig")),
    ] {
        let r = schritt(
            &z,
            Aufrufer::Hub,
            "mcp.anfragen",
            anfrage(&json!({"frist_ms": frist})),
            T0,
        );
        assert_eq!(code(&r), soll, "frist_ms {frist}");
    }
    // such-mcp names no deadline: 60000 is its limit, and 60000 is its redeem deadline.
    let such = |frist: i64| {
        anfrage(&json!({"server": "such-mcp", "werkzeug": "brain_schreibe", "frist_ms": frist}))
    };
    let r = schritt(&z, Aufrufer::Hub, "mcp.anfragen", such(60_001), T0);
    assert_eq!(code(&r), Some("stelle.frist_ungueltig"));
    let r = schritt(&z, Aufrufer::Hub, "mcp.anfragen", such(60_000), T0);
    assert_eq!(r.antwort.json()["frist_bis_ms"], T0 + 60_000);
}

#[test]
fn einloesefrist_der_vorgabe() {
    let z = gestartet(Box::new(Echo));
    let r = schritt(
        &z,
        Aufrufer::Hub,
        "mcp.anfragen",
        anfrage(&json!({"server": "such-mcp", "werkzeug": "brain_schreibe"})),
        T0,
    );
    let kennung = r.antwort.json()["anfrage"].clone();
    let r = schritt(
        &r.zustand,
        Aufrufer::Mensch,
        "mcp.entscheiden",
        json!({"anfrage": kennung, "entscheidung": "erteilen", "nachweis": person()}),
        T0 + 10,
    );
    assert_eq!(r.antwort.json()["einloesen_bis_ms"], T0 + 10 + 60_000);
}

#[test]
fn pruefer_ohne_pseudonym_gilt_als_err() {
    let z = gestartet(Box::new(Echo));
    let r = schritt(&z, Aufrufer::Hub, "mcp.anfragen", anfrage(&json!({})), T0);
    let kennung = r.antwort.json()["anfrage"].clone();
    let z = r.zustand;
    for nachweis in ["ps_kurz", "Max", "PS_00000000000000000000000000000001"] {
        let r = schritt(
            &z,
            Aufrufer::Mensch,
            "mcp.entscheiden",
            json!({"anfrage": kennung, "entscheidung": "erteilen", "nachweis": nachweis}),
            T0,
        );
        assert_eq!(code(&r), Some("stelle.nachweis_ungueltig"), "{nachweis}");
    }
    let r = schritt(
        &z,
        Aufrufer::Mensch,
        "mcp.entscheiden",
        json!({"anfrage": kennung, "entscheidung": "ablehnen", "nachweis": person()}),
        T0,
    );
    assert_eq!(r.antwort.json()["ergebnis"], "abgelehnt");
    let zeile = &r.zeilen_texte(formate::kette::NULL_HASH).expect("lines")[0];
    assert!(
        zeile.contains(&format!("\"entscheidend\":\"{}\"", person())),
        "{zeile}"
    );
}

#[test]
fn laenge_des_nachweises() {
    let z = gestartet(Box::new(PrueferH1::neu([1; 32])));
    let r = schritt(&z, Aufrufer::Hub, "mcp.anfragen", anfrage(&json!({})), T0);
    let kennung = r.antwort.json()["anfrage"].clone();
    let z = r.zustand;
    for (nachweis, soll) in [
        (String::new(), "stelle.eingabe_ungueltig"),
        ("x".to_owned(), "stelle.nachweis_ungueltig"),
        ("x".repeat(4096), "stelle.nachweis_ungueltig"),
        ("\u{e4}".repeat(4096), "stelle.nachweis_ungueltig"),
        ("x".repeat(4097), "stelle.eingabe_ungueltig"),
    ] {
        let r = schritt(
            &z,
            Aufrufer::Mensch,
            "mcp.entscheiden",
            json!({"anfrage": kennung, "entscheidung": "erteilen", "nachweis": nachweis}),
            T0,
        );
        assert_eq!(code(&r), Some(soll), "length {}", nachweis.chars().count());
    }
    let r = schritt(
        &z,
        Aufrufer::Mensch,
        "mcp.entscheiden",
        json!({"anfrage": kennung, "entscheidung": "pruefen", "nachweis": "x"}),
        T0,
    );
    assert_eq!(code(&r), Some("stelle.eingabe_ungueltig"));
}

#[test]
fn kennungen_anderer_ladung_sind_unbekannt() {
    let z = gestartet(Box::new(PrueferH1::neu([1; 32])));
    let r = schritt(&z, Aufrufer::Hub, "mcp.anfragen", anfrage(&json!({})), T0);
    assert_eq!(r.antwort.json()["anfrage"], format!("mf-{LADEN}-1"));
    let z = r.zustand;
    for kennung in [
        format!("mf-{}-1", LADEN + 1),
        format!("mf-0{LADEN}-1"),
        format!("mf-{LADEN}-2"),
    ] {
        let r = schritt(
            &z,
            Aufrufer::Hub,
            "mcp.stand",
            json!({"anfrage": kennung}),
            T0,
        );
        assert_eq!(code(&r), Some("stelle.anfrage_unbekannt"), "{kennung}");
    }
    let r = schritt(
        &z,
        Aufrufer::Hub,
        "mcp.stand",
        json!({"anfrage": format!("mf-{LADEN}-1")}),
        T0,
    );
    assert_eq!(r.antwort.json()["zustand"], "offen");
}

#[test]
fn leere_eingaben() {
    let z = gestartet(Box::new(PrueferH1::neu([1; 32])));
    for (aufrufer, vorgang) in [
        (Aufrufer::Takt, "ablauf.pruefen"),
        (Aufrufer::Mensch, "mcp.offene"),
    ] {
        assert!(schritt(&z, aufrufer, vorgang, json!({}), T0).antwort.ok());
        for eingabe in [json!({"x": 1}), json!([]), json!(null)] {
            let r = schritt(&z, aufrufer, vorgang, eingabe, T0);
            assert_eq!(code(&r), Some("stelle.eingabe_ungueltig"), "{vorgang}");
        }
    }
    for eingabe in [
        json!({"start": {"text": "{}", "b64": ""}}),
        json!({"start": {"hex": "00"}}),
        json!({"start": {"text": 5}}),
        json!({"start": {"b64": "!!"}}),
        json!({"start": {"text": "{}"}, "x": 1}),
        json!({"anders": {"text": "{}"}}),
    ] {
        let r = schritt(&z, Aufrufer::Kern, "sitzung.starten", eingabe.clone(), T0);
        assert_eq!(code(&r), Some("stelle.eingabe_ungueltig"), "{eingabe}");
    }
}

#[test]
fn zeit_grenzen() {
    let pruefer = || Box::new(PrueferH1::neu([1; 32]));
    let e = einstellungen();
    assert_eq!(
        laden(e.as_bytes(), -1, pruefer()).map(|_| ()),
        Err(Code::ZeitUngueltig)
    );
    assert_eq!(
        laden(e.as_bytes(), ZEIT_MAX + 1, pruefer()).map(|_| ()),
        Err(Code::ZeitUngueltig)
    );
    assert_eq!(laden(e.as_bytes(), 0, pruefer()).expect("0").zeit_max(), 0);
    let z = laden(e.as_bytes(), ZEIT_MAX, pruefer()).expect("max");
    assert_eq!(
        (z.laden_ms(), z.zeit_max(), z.zaehler()),
        (ZEIT_MAX, ZEIT_MAX, 0)
    );
    let z = gestartet(pruefer());
    for jetzt in [-1, ZEIT_MAX + 1, i64::MIN, i64::MAX] {
        let r = schritt(&z, Aufrufer::Takt, "ablauf.pruefen", json!({}), jetzt);
        assert_eq!(code(&r), Some("stelle.zeit_ungueltig"));
        assert_eq!(r.zeilen, Vec::new());
        assert_eq!(r.zeit_ms, T0);
        assert_eq!(r.zustand.abbild(), z.abbild());
    }
    let r = schritt(&z, Aufrufer::Takt, "ablauf.pruefen", json!({}), ZEIT_MAX);
    assert!(r.antwort.ok());
    assert_eq!((r.zeit_ms, r.zustand.zeit_max()), (ZEIT_MAX, ZEIT_MAX));
    // A smaller time counts as the highest one seen.
    let r = schritt(&r.zustand, Aufrufer::Takt, "ablauf.pruefen", json!({}), 0);
    assert_eq!((r.zeit_ms, r.zustand.zeit_max()), (ZEIT_MAX, ZEIT_MAX));
}

#[test]
fn ein_neuer_zustand_ist_ein_anderes_abbild() {
    let z = gestartet(Box::new(PrueferH1::neu([1; 32])));
    let r = schritt(
        &z,
        Aufrufer::Hub,
        "mcp.anfragen",
        anfrage(&json!({"vorschau": "sichtbar"})),
        T0 + 5,
    );
    assert_ne!(r.zustand.abbild(), z.abbild());
    let a = &r.zustand.abbild()["anfragen"][format!("mf-{LADEN}-1")];
    assert_eq!(a["vorschau"], "sichtbar");
    assert_eq!(a["zustand"], "offen");
    assert_eq!(a["bis"], T0 + 5 + 60_000);
    assert_eq!(
        r.zustand.abbild()["sitzungen"]["s-1"]["route"],
        "cloud-beispiel"
    );
    assert_eq!(r.zustand.einstellungen().mcp.len(), 3);
    assert!(format!("{:?}", r.zustand).starts_with("Zustand {"));
}

#[test]
fn vorgaenge_und_arten() {
    let alle = [
        ("sitzung.starten", Aufrufer::Kern),
        ("mcp.anfragen", Aufrufer::Hub),
        ("mcp.entscheiden", Aufrufer::Mensch),
        ("mcp.einloesen", Aufrufer::Hub),
        ("mcp.offene", Aufrufer::Mensch),
        ("mcp.stand", Aufrufer::Hub),
        ("ablauf.pruefen", Aufrufer::Takt),
    ];
    for (name, art) in alle {
        let v = Vorgang::aus_name(name, json!({"n": name})).expect("known");
        assert_eq!(v.name(), name);
        assert_eq!(v.erlaubt(), art, "{name}");
        assert_eq!(v.eingabe(), &json!({"n": name}));
    }
    assert!(Vorgang::aus_name("mcp.ausgefuehrt", json!({})).is_none());
    assert!(Vorgang::aus_name("", json!({})).is_none());
}
