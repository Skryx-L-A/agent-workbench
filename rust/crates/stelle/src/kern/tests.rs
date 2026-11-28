//! The limits of the request counter (contract 6, section 4) and of the promise counter (section 15.3, stage 11). No
//! visible case reaches them, so the tests set the counter of a loaded state directly.

use serde_json::{Value, json};

use super::{ANFRAGE_MAX, ZUSAGE_FRIST_MS, ZUSAGE_MAX};
use crate::{
    Antwort, Aufrufer, Ereignis, Ergebnis, Nachweispruefer, Vorgang, Zustand, entscheide, form,
    laden, worker_regeln_lesen,
};

const LADEN: i64 = 1_795_179_540_000;
const T0: i64 = LADEN + 60_000;

/// A checker that accepts every proof; this test never decides a request.
struct Echo;

impl Nachweispruefer for Echo {
    fn pruefe(&self, _herausforderung: &[u8], nachweis: &str) -> Result<String, ()> {
        Ok(nachweis.to_owned())
    }
}

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

fn schritt(z: &Zustand, aufrufer: Aufrufer, vorgang: &str, eingabe: Value) -> Ergebnis {
    let ereignis = Ereignis {
        aufrufer,
        vorgang: Vorgang::aus_name(vorgang, eingabe).expect("known operation"),
    };
    entscheide(z, &ereignis, T0)
}

fn gestartet() -> Zustand {
    let z = laden(einstellungen().as_bytes(), LADEN, Box::new(Echo)).expect("load");
    let start = json!({"fassung": 1, "sitzung": "s-1", "harness": "claude", "modell": "m", "route": "cloud-beispiel",
                       "kunde": null, "arbeitsordner": "/a"});
    let r = schritt(
        &z,
        Aufrufer::Kern,
        "sitzung.starten",
        json!({"start": {"text": start.to_string()}}),
    );
    assert_eq!(r.antwort.json()["ergebnis"], "gestartet");
    r.zustand
}

fn anfrage(server: &str, werkzeug: &str) -> Value {
    json!({"argumente": {"id": 1}, "frist_ms": 60000, "person": format!("ps_{}1", "0".repeat(31)), "server": server,
           "sitzung": "s-1", "vorschau": null, "werkzeug": werkzeug})
}

fn code(r: &Ergebnis) -> Option<&'static str> {
    match r.antwort {
        Antwort::Nein(c) => Some(c.code()),
        _ => None,
    }
}

#[test]
fn the_last_identifier_keeps_its_form_and_every_later_request_is_refused() {
    let mut z = gestartet();
    z.zaehler = ANFRAGE_MAX - 1;
    let r = schritt(
        &z,
        Aufrufer::Hub,
        "mcp.anfragen",
        anfrage("tickets", "update"),
    );
    assert_eq!(r.antwort.json()["ergebnis"], "offen");
    let kennung = r.antwort.json()["anfrage"]
        .as_str()
        .expect("an identifier")
        .to_owned();
    assert_eq!(kennung, format!("mf-{LADEN}-{ANFRAGE_MAX}"));
    assert!(form::anfrage(&kennung), "{kennung}");
    assert!(!form::anfrage(&format!("mf-{LADEN}-{}", ANFRAGE_MAX + 1)));
    assert_eq!(r.zustand.zaehler, ANFRAGE_MAX);

    // Both ways that take an identifier: with a release (open request) and without (line `ohne_freigabe`).
    for (server, werkzeug) in [("tickets", "update"), ("notizen", "anlegen")] {
        let n = schritt(
            &r.zustand,
            Aufrufer::Hub,
            "mcp.anfragen",
            anfrage(server, werkzeug),
        );
        assert_eq!(code(&n), Some("stelle.anfragen_erschoepft"), "{server}");
        assert_eq!(
            n.zustand.zaehler, ANFRAGE_MAX,
            "{server}: the counter stays"
        );
        assert_eq!(
            n.zustand.abbild()["anfragen"],
            r.zustand.abbild()["anfragen"],
            "{server}: no new request"
        );
        let zeilen = n.zeilen_texte(formate::kette::NULL_HASH).expect("lines");
        assert_eq!(zeilen.len(), 1, "{server}: {zeilen:?}");
        assert!(
            zeilen[0].contains("\"aktionsname\":\"vorgang.abgelehnt\"")
                && zeilen[0].contains("\"grund\":\"stelle.anfragen_erschoepft\""),
            "{server}: {}",
            zeilen[0]
        );
    }

    // A reading tool takes no identifier and still answers.
    let l = schritt(
        &r.zustand,
        Aufrufer::Hub,
        "mcp.anfragen",
        anfrage("tickets", "get"),
    );
    assert_eq!(l.antwort.json()["ergebnis"], "lesend");
    assert_eq!(l.zustand.zaehler, ANFRAGE_MAX);
}

#[test]
fn the_last_promise_keeps_its_form_and_every_later_request_is_refused() {
    let mut z = gestartet();
    z.worker = worker_regeln_lesen(
        br#"{"fassung":1,"worker":{"hoechstzahl":4,"harness":["claude"],"modelle":["m"],"effort":["low"]}}"#,
    );
    z.zusage_zaehler = ZUSAGE_MAX - 1;
    let anfordern = json!({"sitzung": "s-1", "rolle": "pruefer", "harness": "claude", "modell": "m", "effort": "low",
                           "laufend": 0, "route": "cloud-beispiel", "kunde": null});
    let r = schritt(&z, Aufrufer::Kern, "worker.anfordern", anfordern.clone());
    assert_eq!(
        r.antwort.json(),
        json!({"ok": true, "ergebnis": "frei", "sitzung": "s-1", "zusage": format!("zs-{LADEN}-{ZUSAGE_MAX}"),
               "zusage_bis_ms": T0 + ZUSAGE_FRIST_MS})
    );
    assert!(form::zusage(&format!("zs-{LADEN}-{ZUSAGE_MAX}")));
    assert!(!form::zusage(&format!("zs-{LADEN}-{}", ZUSAGE_MAX + 1)));
    assert_eq!(r.zustand.zusage_zaehler, ZUSAGE_MAX);
    assert!(r.zeilen.is_empty(), "frei writes no line");

    let n = schritt(&r.zustand, Aufrufer::Kern, "worker.anfordern", anfordern);
    assert_eq!(code(&n), Some("stelle.zusagen_erschoepft"));
    assert_eq!(n.zustand.zusage_zaehler, ZUSAGE_MAX, "the counter stays");
    assert_eq!(
        n.zustand.abbild()["zusagen"],
        r.zustand.abbild()["zusagen"],
        "no new promise"
    );
    let zeilen = n.zeilen_texte(formate::kette::NULL_HASH).expect("lines");
    assert_eq!(zeilen.len(), 1, "{zeilen:?}");
    assert!(
        zeilen[0].contains("\"vorgang\":\"worker.anfordern\"")
            && zeilen[0].contains("\"grund\":\"stelle.zusagen_erschoepft\"")
            && zeilen[0].contains("\"anfrage\":null"),
        "{}",
        zeilen[0]
    );
}
