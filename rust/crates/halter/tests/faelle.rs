//! The visible cases of the contract (`rust/spec/halter/faelle/` and `erwartet/`): every request runs through the order
//! of checks against the `lage` of its case, every answer through the closed form of the answer lines. No case and no
//! expectation is changed here; a case that does not fit fails this test.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::PathBuf;

use halter::antwort;
use halter::b64;
use halter::konfig;
use halter::pruefung::{self, Aufrufer, Eintrag, Klasse, Rolle, ServerRegel, Sicht, ZielRegel};
use serde_json::Value;

fn ordner() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../spec/halter")
}

fn lesen(p: &std::path::Path) -> Value {
    serde_json::from_slice(&fs::read(p).expect("read case")).expect("case JSON")
}

fn rohbytes(fall: &Value) -> Vec<u8> {
    if let Some(e) = fall.get("erzeugt") {
        let vorne = e["vorne"].as_str().expect("vorne").as_bytes();
        let hinten = e["hinten"].as_str().expect("hinten").as_bytes();
        let fuell = e["fuellbyte"].as_str().expect("fuellbyte").as_bytes()[0];
        let laenge = usize::try_from(e["laenge"].as_u64().expect("laenge")).expect("usize");
        let mut b = vorne.to_vec();
        b.resize(laenge - hinten.len(), fuell);
        b.extend_from_slice(hinten);
        return b;
    }
    if let Some(t) = fall.get("text") {
        return t.as_str().expect("text").as_bytes().to_vec();
    }
    b64::dekodieren(fall["b64"].as_str().expect("b64")).expect("b64 of the case")
}

fn opt_text(v: &Value) -> Option<String> {
    v.as_str().map(str::to_owned)
}

fn sicht(lage: &Value) -> Sicht {
    let a = &lage["aufrufer"];
    let konfig = &lage["konfig"];
    let mut ziele = BTreeMap::new();
    for (n, z) in konfig["ziele"].as_object().expect("ziele") {
        ziele.insert(
            n.clone(),
            ZielRegel {
                schluessel: z["schluessel"].as_str().expect("schluessel").to_owned(),
                pfade: z["pfade"]
                    .as_array()
                    .expect("pfade")
                    .iter()
                    .map(|r| r.as_str().expect("regel").to_owned())
                    .collect(),
            },
        );
    }
    let mut server = BTreeMap::new();
    for (n, z) in konfig["server"].as_object().expect("server") {
        let werkzeuge = z["werkzeuge"]
            .as_object()
            .expect("werkzeuge")
            .iter()
            .map(|(w, k)| {
                let k = if k == "lesend" {
                    Klasse::Lesend
                } else {
                    Klasse::Schreibend
                };
                (w.clone(), k)
            })
            .collect();
        server.insert(
            n.clone(),
            ServerRegel {
                schluessel: z["schluessel"].as_str().expect("schluessel").to_owned(),
                freigabe: z["freigabe"].as_bool().expect("freigabe"),
                werkzeuge,
            },
        );
    }
    let kids = lage["kids"]
        .as_object()
        .expect("kids")
        .iter()
        .map(|(k, v)| {
            let b = b64::dekodieren(v.as_str().expect("kid")).expect("kid b64");
            (k.clone(), <[u8; 32]>::try_from(b).expect("32 bytes"))
        })
        .collect();
    Sicht {
        aufrufer: Aufrufer {
            rolle: a["rolle"]
                .as_str()
                .map(|r| Rolle::aus_text(r).expect("rolle")),
            konto: a["konto"].as_str().expect("konto").to_owned(),
            vertreter: a["vertreter"].as_bool().unwrap_or(false),
        },
        jetzt_ms: lage["jetzt_ms"].as_i64().expect("jetzt"),
        start_ms: lage["start_ms"].as_i64().expect("start"),
        kids,
        benutzt: lage["benutzt"]
            .as_array()
            .expect("benutzt")
            .iter()
            .map(|e| e.as_str().expect("einmal").to_owned())
            .collect::<BTreeSet<_>>(),
        nachweis_gueltig: lage["nachweis_gueltig"].as_bool().expect("nachweis"),
        ziele,
        server,
        geheimnisse: lage["geheimnisse"]
            .as_array()
            .expect("geheimnisse")
            .iter()
            .map(|g| Eintrag {
                name: g["name"].as_str().expect("name").to_owned(),
                konto: opt_text(&g["konto"]),
                person: opt_text(&g["person"]),
            })
            .collect(),
        strom: lage.get("strom").filter(|s| !s.is_null()).cloned(),
    }
}

/// The time borders of stages 24 and 25 around the valid case `a-mcp-schreibend` (ausgestellt 1794999990000, ablauf
/// 1795000030000): the signed ticket stays the same, only the clock and the start of the holder move.
#[test]
fn zeitgrenzen_des_scheins() {
    let fall = lesen(&ordner().join("faelle/a-mcp-schreibend.json"));
    let roh = rohbytes(&fall);
    let ausgestellt = 1_794_999_990_000_i64;
    let ablauf = 1_795_000_030_000_i64;
    let code = |jetzt: i64, start: i64| {
        let mut s = sicht(&fall["lage"]);
        s.jetzt_ms = jetzt;
        s.start_ms = start;
        pruefung::pruefen(&roh, &s).err().map(|a| a.code.code())
    };
    let start = ausgestellt - 1;
    assert_eq!(
        code(ausgestellt - 5000, start),
        None,
        "5000 ms ahead of the own clock is allowed"
    );
    assert_eq!(code(ausgestellt - 5001, start), Some("halter.schein_zeit"));
    assert_eq!(code(ablauf - 1, start), None);
    assert_eq!(code(ablauf, start), Some("halter.schein_abgelaufen"));
    assert_eq!(
        code(ausgestellt, ausgestellt),
        None,
        "issued in the millisecond of the start"
    );
    assert_eq!(
        code(ausgestellt, ausgestellt + 1),
        Some("halter.schein_abgelaufen")
    );
    let mut s = sicht(&fall["lage"]);
    s.benutzt
        .insert("5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e".to_owned());
    assert_eq!(
        pruefung::pruefen(&roh, &s).err().map(|a| a.code),
        Some(halter::code::Code::ScheinBenutzt)
    );
    let ok = pruefung::pruefen(&roh, &sicht(&fall["lage"])).expect("valid");
    assert_eq!(
        ok.einloesen,
        Some(("5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e".to_owned(), ablauf))
    );
    assert_eq!(
        ok.digest.as_deref(),
        Some("ffc2016a50770971ab9be5aac167e9165ac7a0a189d6e5f7c9fac57af4acb258")
    );
}

#[test]
fn alle_faelle() {
    let mut namen: Vec<String> = fs::read_dir(ordner().join("faelle"))
        .expect("case folder")
        .map(|e| e.expect("entry").file_name().to_string_lossy().into_owned())
        .collect();
    namen.sort();
    let mut fehler = Vec::new();
    for name in &namen {
        let fall = lesen(&ordner().join("faelle").join(name));
        let erw = lesen(&ordner().join("erwartet").join(name));
        let soll = if erw["gueltig"].as_bool() == Some(true) {
            None
        } else {
            Some(erw["code"].as_str().expect("code").to_owned())
        };
        let roh = rohbytes(&fall);
        let ist = if fall["form"] == "antwort" {
            (!antwort::gueltig(&roh)).then(|| "antwort.form".to_owned())
        } else if fall["form"] == "konfig" {
            // section 2.3a: halter.json with the environment of the start (own uid, /etc/group, /etc/passwd)
            let u = &fall["umgebung"];
            let uid = u32::try_from(u["uid"].as_u64().expect("uid")).expect("u32");
            konfig::fuer_start(
                &roh,
                uid,
                u["group"].as_str().expect("group"),
                u["passwd"].as_str().expect("passwd"),
            )
            .err()
            .map(|_| "halter.konfig".to_owned())
        } else {
            match pruefung::pruefen(&roh, &sicht(&fall["lage"])) {
                Ok(_) => None,
                Err(a) => {
                    // N1: a refused path (stage 15) carries op, target, method and path for its chain line; no other
                    // refusal does
                    let anfrage: Value = serde_json::from_slice(&roh).unwrap_or(Value::Null);
                    let erwartet = (a.code == halter::code::Code::PfadVerboten).then(|| {
                        let strom = anfrage["op"] == "modell.strom";
                        pruefung::PfadAbgelehnt {
                            op: anfrage["op"].as_str().unwrap_or("").to_owned(),
                            ziel: anfrage["ziel"].as_str().unwrap_or("").to_owned(),
                            methode: if strom {
                                "GET"
                            } else {
                                anfrage["methode"].as_str().unwrap_or("")
                            }
                            .to_owned(),
                            pfad: anfrage["pfad"].as_str().unwrap_or("").to_owned(),
                        }
                    });
                    if a.pfad != erwartet {
                        fehler.push(format!("{name}: path of the chain line {:?}", a.pfad));
                    }
                    Some(a.code.code().to_owned())
                }
            }
        };
        if ist != soll {
            fehler.push(format!("{name}: {ist:?}, expected {soll:?}"));
        }
    }
    assert_eq!(namen.len(), 188, "number of cases");
    assert_eq!(
        namen.iter().filter(|n| n.starts_with("k-")).count(),
        29,
        "cases of the form konfig"
    );
    assert!(
        fehler.is_empty(),
        "{} of {} cases fail:\n{}",
        fehler.len(),
        namen.len(),
        fehler.join("\n")
    );
}
