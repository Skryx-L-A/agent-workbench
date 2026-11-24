//! Case tests: every file under `tests/faelle/` runs through the library and is compared with
//! `tests/erwartet/<name>.json` (`exit`, and `ausgabe` for exit 0).
//!
//! The files are the visible outside cases of the contract package (compacted copies; a case that is not JSON is
//! copied as it is). The Deflate bytes are never compared: like the outside runner, the test checks `leitung_hex`
//! and `leitung_laenge` on their own (without Deflate the wire equals `hex`; with Deflate it inflates to `hex`
//! with one context per connection and does not end on `00 00 ff ff`), and the ratios of `leitung_erwartung`.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use flate2::{Decompress, FlushDecompress};
use serde_json::Value;

fn ordner(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join(name)
}

fn lesen(pfad: &PathBuf) -> Value {
    let text = fs::read_to_string(pfad).expect("file is readable");
    serde_json::from_str(&text).expect("file is JSON")
}

/// The first difference of `ist` against `soll`, as a path.
fn unterschied(ist: &Value, soll: &Value, pfad: &str) -> Option<String> {
    match (ist, soll) {
        (Value::Object(a), Value::Object(b)) => {
            for k in a.keys().chain(b.keys()) {
                match (a.get(k), b.get(k)) {
                    (Some(x), Some(y)) => {
                        if let Some(d) = unterschied(x, y, &format!("{pfad}.{k}")) {
                            return Some(d);
                        }
                    }
                    _ => return Some(format!("{pfad}.{k}: fehlt oder zu viel")),
                }
            }
            None
        }
        (Value::Array(a), Value::Array(b)) => {
            for (i, (x, y)) in a.iter().zip(b).enumerate() {
                if let Some(d) = unterschied(x, y, &format!("{pfad}[{i}]")) {
                    return Some(d);
                }
            }
            (a.len() != b.len()).then(|| format!("{pfad}: Laenge {} statt {}", a.len(), b.len()))
        }
        _ => (ist != soll).then(|| format!("{pfad}: {ist} statt {soll}")),
    }
}

fn hex_bytes(text: &str) -> Vec<u8> {
    (0..text.len() / 2)
        .map(|i| u8::from_str_radix(&text[2 * i..2 * i + 2], 16).expect("hex"))
        .collect()
}

/// Inflates one frame with the context `d` (the four end bytes put back).
fn entpacken(d: &mut Decompress, wire: &[u8]) -> Vec<u8> {
    let mut eingabe = wire.to_vec();
    eingabe.extend_from_slice(&[0, 0, 0xff, 0xff]);
    let mut aus = Vec::new();
    let mut puffer = vec![0_u8; 65_536];
    let mut ab = 0;
    loop {
        let (vor_ein, vor_aus) = (d.total_in(), d.total_out());
        d.decompress(&eingabe[ab..], &mut puffer, FlushDecompress::Sync)
            .expect("the wire inflates");
        ab += usize::try_from(d.total_in() - vor_ein).expect("fits");
        let erzeugt = usize::try_from(d.total_out() - vor_aus).expect("fits");
        aus.extend_from_slice(&puffer[..erzeugt]);
        if ab >= eingabe.len() && erzeugt < puffer.len() {
            return aus;
        }
    }
}

/// Checks the wire bytes of every frame; returns a reason or `None`.
fn leitung_pruefen(fall: &Value, ist: &Value) -> Option<String> {
    let mut deflate: HashMap<String, bool> = HashMap::new();
    let mut entpacker: HashMap<String, Decompress> = HashMap::new();
    let mut laengen: HashMap<usize, u64> = HashMap::new();
    let schritte = fall["schritte"].as_array().expect("steps");
    let ergebnisse = ist["ergebnisse"].as_array().expect("results");
    for (i, (sch, erg)) in schritte.iter().zip(ergebnisse).enumerate() {
        let verbindung = sch["verbindung"].as_str().unwrap_or_default().to_owned();
        match sch["art"].as_str() {
            Some("Verbinden") => {
                deflate.insert(verbindung.clone(), sch["deflate"] == Value::Bool(true));
                entpacker.insert(verbindung, Decompress::new(false));
            }
            Some("Nachricht") if erg["ok"] == Value::Bool(true) && !erg["hex"].is_null() => {
                let Some(wire_hex) = erg["leitung_hex"].as_str() else {
                    return Some(format!("Schritt {i}: leitung_hex fehlt"));
                };
                let wire = hex_bytes(wire_hex);
                if erg["leitung_laenge"].as_u64() != Some(wire.len() as u64) {
                    return Some(format!("Schritt {i}: leitung_laenge passt nicht"));
                }
                laengen.insert(i, wire.len() as u64);
                let roh = hex_bytes(erg["hex"].as_str().expect("hex"));
                if deflate.get(&verbindung) != Some(&true) {
                    if wire != roh {
                        return Some(format!(
                            "Schritt {i}: ohne Deflate muss die Leitung gleich hex sein"
                        ));
                    }
                    continue;
                }
                if wire.ends_with(&[0, 0, 0xff, 0xff]) {
                    return Some(format!(
                        "Schritt {i}: Deflate-Ende 00 00 ff ff nicht entfernt"
                    ));
                }
                let d = entpacker.get_mut(&verbindung).expect("inflater");
                if entpacken(d, &wire) != roh {
                    return Some(format!("Schritt {i}: Leitung entpackt nicht zu hex"));
                }
            }
            _ => {}
        }
    }
    for e in fall["leitung_erwartung"].as_array().into_iter().flatten() {
        let a = usize::try_from(e["schritt"].as_u64().expect("step")).expect("fits");
        let g = usize::try_from(e["gegen"].as_u64().expect("step")).expect("fits");
        let (Some(la), Some(lg)) = (laengen.get(&a), laengen.get(&g)) else {
            return Some(format!(
                "leitung_erwartung: Schritt {a} oder {g} ohne Rahmen"
            ));
        };
        #[allow(clippy::cast_precision_loss)]
        let zu_lang = *la as f64 > e["hoechstens"].as_f64().expect("factor") * *lg as f64;
        if zu_lang {
            return Some(format!(
                "Schritt {a}: Leitung {la} B nicht hoechstens {} mal {lg} B",
                e["hoechstens"]
            ));
        }
    }
    None
}

/// Takes the two keys the runner does not compare out of every result.
fn ohne_leitung(mut aus: Value) -> Value {
    if let Some(liste) = aus["ergebnisse"].as_array_mut() {
        for e in liste {
            if let Some(o) = e.as_object_mut() {
                o.remove("leitung_hex");
                o.remove("leitung_laenge");
            }
        }
    }
    aus
}

#[test]
fn every_case_gives_the_expected_results() {
    let mut namen: Vec<String> = fs::read_dir(ordner("faelle"))
        .expect("case folder exists")
        .map(|e| e.expect("entry").file_name().to_string_lossy().into_owned())
        .filter(|n| Path::new(n).extension().is_some_and(|e| e == "json"))
        .collect();
    namen.sort();
    assert!(namen.len() >= 96, "only {} cases found", namen.len());
    let mut rot = Vec::new();
    for name in &namen {
        let fall = fs::read(ordner("faelle").join(name)).expect("case is readable");
        let erwartet = lesen(&ordner("erwartet").join(name));
        let ergebnis = rasterauszug::laufen(&fall);
        let grund = match (erwartet["exit"].as_i64(), ergebnis) {
            (Some(0), Ok(ist)) => {
                let fall_json: Value =
                    serde_json::from_slice(&fall).expect("an exit 0 case is JSON");
                leitung_pruefen(&fall_json, &ist)
                    .or_else(|| unterschied(&ohne_leitung(ist), &erwartet["ausgabe"], "$"))
            }
            (Some(0), Err(e)) => Some(format!("Fehler statt Ausgabe: {e}")),
            (Some(2), Err(_)) => None,
            (Some(2), Ok(_)) => Some("Ausgabe statt Fehler".to_owned()),
            (andere, _) => Some(format!("unerwarteter Exitcode {andere:?} in der Erwartung")),
        };
        if let Some(g) = grund {
            rot.push(format!("{name}: {g}"));
        }
    }
    assert!(
        rot.is_empty(),
        "{} von {} rot:\n{}",
        rot.len(),
        namen.len(),
        rot.join("\n")
    );
}
