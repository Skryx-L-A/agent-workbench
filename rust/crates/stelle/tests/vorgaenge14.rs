//! The operations of section 14 in the core: `person.pseudonym` with the store behind the trait, and
//! `worker.anfordern` with the rules of the admins. The visible cases of `worker.anfordern` stand in `faelle/`.

use std::sync::Arc;

use serde_json::{Value, json};
use stelle::{
    Aufrufer, Ereignis, Personenspeicher, PrueferH1, Vorgang, Zustand, entscheide, laden,
    worker_regeln_lesen,
};

const LADEN: i64 = 1_795_179_540_000;
const T0: i64 = LADEN + 60_000;

fn einstellungen() -> String {
    json!({
        "fassung": 2, "klassen": ["oeffentlich", "intern", "kundendaten"], "vorgabestufe": "kundendaten",
        "routen": [{"schluessel": "cloud-beispiel", "art": "cloud", "klasse": "intern", "netzziele": []}],
        "kunden": [], "kundendaten_an_cloud": {"vorgabe": "nie", "freigaben": []}
    })
    .to_string()
}

fn zustand() -> Zustand {
    laden(
        einstellungen().as_bytes(),
        LADEN,
        Box::new(PrueferH1::neu([1; 32])),
    )
    .expect("load")
}

fn schritt(z: &Zustand, aufrufer: Aufrufer, vorgang: &str, eingabe: Value) -> (Value, Vec<String>) {
    let ereignis = Ereignis {
        aufrufer,
        vorgang: Vorgang::aus_name(vorgang, eingabe).expect("operation"),
    };
    let r = entscheide(z, &ereignis, T0);
    let zeilen = r.zeilen_texte(&"0".repeat(64)).expect("lines");
    (r.antwort.json(), zeilen)
}

/// A store that gives a fixed answer and counts its calls.
struct Fest(Option<String>, std::sync::atomic::AtomicUsize);

impl Personenspeicher for Fest {
    fn pseudonym(&self, kennung: &str, _jetzt_ms: i64) -> Option<String> {
        assert_eq!(kennung, "uid-1000");
        self.1.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        self.0.clone()
    }
}

fn mit(antwort: Option<&str>) -> (Zustand, Arc<Fest>) {
    let fest = Arc::new(Fest(antwort.map(str::to_owned), 0.into()));
    (zustand().mit_personen(fest.clone()), fest)
}

const PS: &str = "ps_0123456789abcdef0123456789abcdef";

#[test]
fn das_pseudonym_kommt_aus_dem_speicher_und_schreibt_keine_zeile() {
    let (z, fest) = mit(Some(PS));
    let (a, zeilen) = schritt(
        &z,
        Aufrufer::Kern,
        "person.pseudonym",
        json!({"kennung": "uid-1000"}),
    );
    assert_eq!(
        a,
        json!({"ok": true, "ergebnis": "pseudonym", "pseudonym": PS})
    );
    assert_eq!(zeilen, Vec::<String>::new());
    assert_eq!(fest.1.load(std::sync::atomic::Ordering::SeqCst), 1);
}

#[test]
fn ohne_speicher_oder_mit_fehler_gibt_es_kein_pseudonym() {
    let nein =
        json!({"ok": false, "code": "stelle.person_fehler", "grund": "Pseudonym nicht bildbar"});
    for z in [
        zustand(),
        mit(None).0,
        mit(Some("ps_0123")).0,
        mit(Some("uid-1000")).0,
        mit(Some("PS_0123456789ABCDEF0123456789ABCDEF")).0,
    ] {
        let (a, zeilen) = schritt(
            &z,
            Aufrufer::Kern,
            "person.pseudonym",
            json!({"kennung": "uid-1000"}),
        );
        assert_eq!(a, nein);
        assert_eq!(zeilen.len(), 1);
        assert!(
            zeilen[0].contains(r#""vorgang":"person.pseudonym""#),
            "{}",
            zeilen[0]
        );
        assert!(zeilen[0].contains(r#""grund":"stelle.person_fehler""#));
        assert!(
            !zeilen[0].contains("uid-1000"),
            "the identifier never stands in a line"
        );
    }
}

#[test]
fn nur_der_kern_fragt_und_nur_mit_einer_kennung() {
    let (z, fest) = mit(Some(PS));
    for art in [
        Aufrufer::Hub,
        Aufrufer::Mensch,
        Aufrufer::Agent,
        Aufrufer::Takt,
        Aufrufer::Unbekannt,
    ] {
        let (a, _) = schritt(&z, art, "person.pseudonym", json!({"kennung": "uid-1000"}));
        assert_eq!(a["code"], "stelle.aufrufer_nicht_erlaubt");
    }
    for eingabe in [
        json!({}),
        json!({"kennung": "uid-1000", "name": "konto-beispiel"}),
        json!({"kennung": "Konto Beispiel"}),
        json!({"kennung": 1000}),
        json!({"kennung": ""}),
    ] {
        let (a, _) = schritt(&z, Aufrufer::Kern, "person.pseudonym", eingabe);
        assert_eq!(a["code"], "stelle.eingabe_ungueltig");
    }
    assert_eq!(
        fest.1.load(std::sync::atomic::Ordering::SeqCst),
        0,
        "the store is asked only after stage 3"
    );
}

#[test]
fn der_worker_folgt_nur_den_regeln_im_zustand() {
    let regeln = worker_regeln_lesen(
        br#"{"fassung":1,"worker":{"hoechstzahl":1,"harness":["claude"],"modelle":["m-1"],"effort":["mittel"]}}"#,
    );
    assert!(regeln.is_some());
    let start = json!({"fassung": 1, "sitzung": "s-1", "harness": "claude", "modell": "m", "route": "cloud-beispiel",
                       "kunde": null, "arbeitsordner": "/a"});
    let z = zustand().mit_worker_regeln(regeln);
    let ereignis = Ereignis {
        aufrufer: Aufrufer::Kern,
        vorgang: Vorgang::aus_name(
            "sitzung.starten",
            json!({"start": {"text": start.to_string()}}),
        )
        .expect("start"),
    };
    let z = entscheide(&z, &ereignis, T0).zustand;
    // section 15.3: route and kunde of the orchestrator; the answer carries the promise and its deadline
    let anfrage = json!({"sitzung": "s-1", "rolle": "pruefer", "harness": "claude", "modell": "m-1", "effort": "mittel", "laufend": 0,
                         "route": "cloud-beispiel", "kunde": null});
    let (a, zeilen) = schritt(&z, Aufrufer::Kern, "worker.anfordern", anfrage.clone());
    assert_eq!(
        a,
        json!({"ok": true, "ergebnis": "frei", "sitzung": "s-1", "zusage": format!("zs-{}-1", z.laden_ms()),
               "zusage_bis_ms": T0 + 30_000})
    );
    assert_eq!(zeilen, Vec::<String>::new());
    let ohne = z.clone().mit_worker_regeln(None);
    let (a, zeilen) = schritt(&ohne, Aufrufer::Kern, "worker.anfordern", anfrage);
    assert_eq!(a["code"], "stelle.vorgabe_fehlt");
    assert!(zeilen[0].contains(r#""vorgang":"worker.anfordern""#));
}
