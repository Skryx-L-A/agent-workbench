//! B5 (Review of 23.11.2026): the promise of contract 6, section 1, "it raises no panic on any input".
//!
//! The test drives a deterministic set of crooked inputs through every operation and every kind of caller. It
//! asks for nothing but the promise itself: `entscheide` returns, the answer holds a valid form, the lines chain,
//! and the given state stays untouched. No new dependency: the numbers come from one fixed seed, the inputs are
//! built as JSON text and read with `serde_json`, so the depth limit of the reader applies as it does in the
//! shell.

use serde_json::{Value, json};
use stelle::{
    Antwort, Aufrufer, Ereignis, Nachweispruefer, PrueferH1, Vorgang, ZEIT_MAX, Zustand,
    entscheide, laden,
};

const LADEN: i64 = 1_795_179_540_000;
const T0: i64 = LADEN + 60_000;
const VORGAENGE: [&str; 7] = [
    "sitzung.starten",
    "mcp.anfragen",
    "mcp.entscheiden",
    "mcp.einloesen",
    "mcp.offene",
    "mcp.stand",
    "ablauf.pruefen",
];
const ARTEN: [Aufrufer; 6] = [
    Aufrufer::Kern,
    Aufrufer::Hub,
    Aufrufer::Mensch,
    Aufrufer::Agent,
    Aufrufer::Takt,
    Aufrufer::Unbekannt,
];
const ERGEBNISSE: [&str; 10] = [
    "gestartet",
    "lesend",
    "ohne_freigabe",
    "offen",
    "erteilt",
    "abgelehnt",
    "eingeloest",
    "offene",
    "stand",
    "geprueft",
];
/// Field names of the seven operations, so that a crooked value lands on a field the point really reads.
const FELDER: [&str; 12] = [
    "anfrage",
    "argumente",
    "entscheidung",
    "frist_ms",
    "nachweis",
    "person",
    "server",
    "sitzung",
    "start",
    "text",
    "vorschau",
    "werkzeug",
];

/// xorshift64*: one fixed seed, so every run drives the same inputs.
struct Zahlen(u64);

impl Zahlen {
    fn naechste(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }

    /// A number from 0 up to `bis` (excluded).
    fn bis(&mut self, bis: usize) -> usize {
        usize::try_from(self.naechste() % bis as u64).unwrap_or(0)
    }
}

fn einstellungen() -> String {
    json!({
        "fassung": 2, "klassen": ["oeffentlich", "intern", "kundendaten"], "vorgabestufe": "kundendaten",
        "routen": [{"schluessel": "cloud-beispiel", "art": "cloud", "klasse": "intern", "netzziele": []},
                   {"schluessel": "lokal-beispiel", "art": "lokal", "klasse": "kundendaten",
                    "ziel": "http://127.0.0.1:11434/v1", "modell": "qwen3:8b", "netzziele": []}],
        "kunden": [{"schluessel": "k-beispiel", "erlaubt": ["lokal-beispiel"], "verboten": []}],
        "kundendaten_an_cloud": {"vorgabe": "nie", "freigaben": []},
        "mcp": {"tickets": {"lesen": ["get"], "schreiben": ["update"], "freigabe": true, "frist_ms": 60000, "einloesefrist_ms": 30000},
                "notizen": {"lesen": [], "schreiben": ["anlegen"], "freigabe": false}}
    })
    .to_string()
}

fn pruefer() -> Box<dyn Nachweispruefer> {
    Box::new(PrueferH1::neu([1; 32]))
}

/// A state with one open request, so that the crooked inputs also meet a request and a session.
fn mit_anfrage() -> Zustand {
    let z = laden(einstellungen().as_bytes(), LADEN, pruefer()).expect("load");
    let start = json!({"fassung": 1, "sitzung": "s-1", "harness": "pi", "modell": "qwen3:8b",
                       "route": "lokal-beispiel", "kunde": "k-beispiel", "arbeitsordner": "/srv/beispiel/projekt"});
    let z = lauf(
        &z,
        Aufrufer::Kern,
        "sitzung.starten",
        json!({"start": {"text": start.to_string()}}),
        T0,
    );
    let person = format!("ps_{}", "0".repeat(31) + "1");
    lauf(
        &z,
        Aufrufer::Hub,
        "mcp.anfragen",
        json!({"argumente": {"id": 1}, "frist_ms": 60000, "person": person, "server": "tickets",
               "sitzung": "s-1", "vorschau": null, "werkzeug": "update"}),
        T0 + 1000,
    )
}

fn lauf(z: &Zustand, aufrufer: Aufrufer, vorgang: &str, eingabe: Value, jetzt: i64) -> Zustand {
    let ereignis = Ereignis {
        aufrufer,
        vorgang: Vorgang::aus_name(vorgang, eingabe).expect("known operation"),
    };
    let r = entscheide(z, &ereignis, jetzt);
    assert!(r.antwort.ok(), "{:?}", r.antwort.json());
    r.zustand
}

/// One crooked text for a JSON value, built from the seed alone. `tiefe` bounds the nesting.
fn krummer_text(z: &mut Zahlen, tiefe: usize) -> String {
    match z.bis(if tiefe == 0 { 10 } else { 13 }) {
        0 => "null".to_owned(),
        1 => "true".to_owned(),
        2 => "false".to_owned(),
        3 => z.naechste().to_string(),
        4 => format!("-{}", z.naechste()),
        5 => [
            "0",
            "-0",
            "1e308",
            "-1e308",
            "0.1",
            "9007199254740993",
            "9223372036854775807",
            "-9223372036854775808",
            "18446744073709551616",
        ][z.bis(9)]
        .to_owned(),
        6 => format!("\"{}\"", krummer_string(z)),
        7 => "[]".to_owned(),
        8 => "{}".to_owned(),
        9 => format!("\"{}\"", "x".repeat(1 + z.bis(1024))),
        10 => {
            let n = 1 + z.bis(4);
            let teile: Vec<String> = (0..n).map(|_| krummer_text(z, tiefe - 1)).collect();
            format!("[{}]", teile.join(","))
        }
        11 => {
            let n = 1 + z.bis(4);
            let teile: Vec<String> = (0..n)
                .map(|i| {
                    let name = if z.bis(2) == 0 {
                        FELDER[z.bis(FELDER.len())].to_owned()
                    } else {
                        format!("{}{i}", krummer_string(z))
                    };
                    format!("\"{name}\":{}", krummer_text(z, tiefe - 1))
                })
                .collect();
            format!("{{{}}}", teile.join(","))
        }
        _ => {
            let n = 1 + z.bis(tiefe);
            format!("{}1{}", "[".repeat(n), "]".repeat(n))
        }
    }
}

/// Short crooked texts for a JSON string: forms of the identifiers, escapes, ASCII and beyond.
fn krummer_string(z: &mut Zahlen) -> String {
    [
        "",
        " ",
        "..",
        "-",
        "_",
        "A",
        "ps_",
        "ps_00000000000000000000000000000001",
        "mf-1795179540000-1",
        "mf-0-0",
        "s-1",
        "tickets",
        "update",
        "erteilen",
        "ablehnen",
        "\\u0000",
        "\\n",
        "\\\\",
        "\\\"",
        "ä",
        "😀",
        "/srv/beispiel/projekt",
        "../srv",
    ][z.bis(23)]
    .to_owned()
}

/// Every answer holds the form of contract 6, section 4: an object with `ok`, and either a result or a code with
/// its reason. Nothing else.
fn form_geprueft(antwort: &Antwort) {
    let j = antwort.json();
    let o = j.as_object().expect("the answer is an object");
    assert_eq!(o["ok"], json!(antwort.ok()), "{j}");
    if antwort.ok() {
        let ergebnis = o["ergebnis"].as_str().expect("a result");
        assert!(ERGEBNISSE.contains(&ergebnis), "{j}");
    } else {
        assert_eq!(o.len(), 3, "{j}");
        let code = o["code"].as_str().expect("a code");
        let grund = o["grund"].as_str().expect("a reason");
        let (bereich, rest) = code.split_once('.').expect("code with its area");
        assert!(["stelle", "start", "einst"].contains(&bereich), "{j}");
        assert!(
            !rest.is_empty()
                && rest
                    .bytes()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'_'),
            "{j}"
        );
        assert!(!grund.is_empty() && grund.is_ascii(), "{j}");
    }
}

#[test]
fn krumme_eingaben_loesen_keine_panik_aus() {
    let mut zahlen = Zahlen(0x5465_7374_5f42_3500);
    let zustaende = [
        laden(einstellungen().as_bytes(), LADEN, pruefer()).expect("load"),
        mit_anfrage(),
    ];
    let zeiten = [-1, 0, 1, LADEN, T0 + 2000, ZEIT_MAX, ZEIT_MAX + 1];
    let mut laeufe = 0_u32;
    let mut abgewiesen = 0_u32;
    for zustand in &zustaende {
        let vorher = zustand.abbild();
        for name in VORGAENGE {
            for art in ARTEN {
                for runde in 0..14 {
                    let text = krummer_text(&mut zahlen, 6);
                    let eingabe = serde_json::from_str::<Value>(&text).unwrap_or_default();
                    let ereignis = Ereignis {
                        aufrufer: art,
                        vorgang: Vorgang::aus_name(name, eingabe).expect("known operation"),
                    };
                    let jetzt = zeiten[runde % zeiten.len()];
                    let r = entscheide(zustand, &ereignis, jetzt);
                    laeufe += 1;

                    form_geprueft(&r.antwort);
                    if !r.antwort.ok() {
                        abgewiesen += 1;
                    }
                    assert!((0..=ZEIT_MAX).contains(&r.zeit_ms), "{text}");
                    assert!(r.zeit_ms >= zustand.laden_ms(), "{text}");
                    assert!(r.zustand.zaehler() >= zustand.zaehler(), "{text}");
                    assert!(r.zustand.zeit_max() >= zustand.zeit_max(), "{text}");
                    for zeile in r.zeilen_texte(&"0".repeat(64)).expect("the lines chain") {
                        let w: Value = serde_json::from_str(&zeile).expect("a line is JSON");
                        assert!(w.is_object(), "{zeile}");
                    }
                    // The kind of the caller decides before the input (section 4, stage 0).
                    if art != ereignis.vorgang.erlaubt() && (0..=ZEIT_MAX).contains(&jetzt) {
                        assert_eq!(
                            r.antwort.json()["code"],
                            json!("stelle.aufrufer_nicht_erlaubt"),
                            "{text}"
                        );
                    }
                }
            }
        }
        assert_eq!(zustand.abbild(), vorher, "entscheide changed the state");
    }
    assert_eq!(laeufe, 2 * 7 * 6 * 14);
    // Without a refusal the set would say nothing: the crooked inputs have to meet the checks.
    assert!(abgewiesen > laeufe / 2, "{abgewiesen} of {laeufe}");
}

/// The deepest input that the reader of the shell lets through, and one level more, for every operation: the
/// point answers, it does not break. The bound belongs to the reader, so the test measures it instead of
/// writing a number of its own.
#[test]
fn tiefe_eingaben_bis_zur_grenze_des_lesers() {
    let zustand = mit_anfrage();
    let text = |tiefe: usize| {
        format!(
            "{{\"argumente\":{}1{}}}",
            "[".repeat(tiefe),
            "]".repeat(tiefe)
        )
    };
    let gelesen = |tiefe: usize| serde_json::from_str::<Value>(&text(tiefe)).ok();
    let grenze = (1..=512)
        .take_while(|tiefe| gelesen(*tiefe).is_some())
        .last()
        .expect("the reader takes at least one level");
    assert!(grenze >= 64, "{grenze}");
    assert!(gelesen(grenze + 1).is_none(), "{grenze}");
    for tiefe in [1, 2, 64, grenze] {
        let eingabe = gelesen(tiefe).expect("the reader takes this depth");
        for name in VORGAENGE {
            let ereignis = Ereignis {
                aufrufer: Vorgang::aus_name(name, Value::Null)
                    .expect("known operation")
                    .erlaubt(),
                vorgang: Vorgang::aus_name(name, eingabe.clone()).expect("known operation"),
            };
            let r = entscheide(&zustand, &ereignis, T0 + 2000);
            form_geprueft(&r.antwort);
        }
    }
}
