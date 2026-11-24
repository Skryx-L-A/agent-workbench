//! The trial run with the marker count (contract 8, section 9): the cases of `faelle/probelauf.json` for the sequence of
//! requests, the start line and the start. The call cases run in `tests/faelle.rs`.
#![cfg(unix)] // the crate is Unix only (see `src/lib.rs`)
#![allow(
    clippy::many_single_char_names,
    reason = "short names for case, answer, line and count keep the tests readable"
)]

mod gemeinsam;

use std::ffi::OsString;
use std::fs;
use std::io;
use std::net::SocketAddr;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::sync::atomic::Ordering;

use formate::kette::sha256_hex;
use gemeinsam::{Antwort, Skript, Wegwerf, anfrage, dienst_mit, fall_datei, latin1, text, zeilen};
use modellweg::probelauf::Zaehlung;
use modellweg::protokoll::{Probelauf, zeile_probelauf};
use modellweg::start::starten;
use modellweg::ziel::FRISTEN;
use serde_json::Value;

fn rechte(p: &Path, modus: u32) {
    fs::set_permissions(p, fs::Permissions::from_mode(modus)).expect("chmod");
}

#[test]
fn folge_of_requests() {
    let f = fall_datei("probelauf.json");
    let marker: Vec<String> = f["marker"]
        .as_array()
        .expect("markers")
        .iter()
        .map(|m| text(m).to_owned())
        .collect();
    let w = Wegwerf::neu("folge");
    let ordner = w.pfad("zaehlung");
    fs::create_dir(&ordner).expect("folder");
    rechte(&ordner, 0o700);
    let datei = ordner.join("marker.json");
    let z = Zaehlung::neu(marker, datei.clone(), "s-2".to_owned());
    z.anfang().expect("first state");
    assert_eq!(fs::read(&datei).expect("read"), latin1(text(&f["anfang"])));
    assert_eq!(
        fs::metadata(&datei).expect("meta").permissions().mode() & 0o777,
        0o600
    );
    let skript = Skript::ok();
    let d = dienst_mit(
        &w,
        "https://modelle.firma.beispiel.invalid/v1",
        gemeinsam::MODELL,
        gemeinsam::SCHLUESSEL,
        Box::new(skript.clone()),
        Some(z),
    );
    let mut zeilen_vorher = 0;
    for fall in f["folge"].as_array().expect("cases") {
        let name = text(&fall["name"]);
        let e = &fall["erwartet"];
        let schreibbar = fall["datei_schreibbar"] == true;
        if !schreibbar {
            rechte(&ordner, 0o500);
        }
        let vorher = skript.verbunden.load(Ordering::SeqCst);
        let roh = anfrage(&d, &latin1(text(&fall["anfrage"])));
        rechte(&ordner, 0o700);
        let a = Antwort::lies(&roh);
        let beruehrt = skript.verbunden.load(Ordering::SeqCst) - vorher;
        assert_eq!(beruehrt == 1, e["ziel_beruehrt"] == true, "{name}");
        if e["ziel_beruehrt"] == true {
            assert_eq!(
                skript.gesehen().last(),
                Some(&latin1(text(&e["an_ziel"]))),
                "{name}"
            );
            assert_eq!(a.status, 200, "{name}");
        }
        if let Some(status) = e["status"].as_u64() {
            assert_eq!(u64::from(a.status), status, "{name}");
            assert_eq!(a.koerper, latin1(text(&e["koerper"])), "{name}");
        }
        let alle = zeilen(&w.protokoll());
        assert_eq!(alle.len(), zeilen_vorher + 1, "{name}");
        zeilen_vorher = alle.len();
        let z = &alle[alle.len() - 1];
        if let Some(code) = e["code"].as_str() {
            assert_eq!(z["grund"], code, "{name}");
        }
        if e["zeile"].is_object() {
            assert_eq!(z["ergebnis"], e["zeile"]["ergebnis"], "{name}");
            assert_eq!(z["grund"], e["zeile"]["grund"], "{name}");
        }
        assert_eq!(
            fs::read(&datei).expect("read"),
            latin1(text(&e["datei"])),
            "{name}"
        );
        let reste = fs::read_dir(&ordner).expect("dir").count();
        assert_eq!(reste, 1, "{name}: a temporary file was left");
    }
}

#[test]
fn startzeile_cases() {
    let f = fall_datei("probelauf.json");
    for fall in f["startzeile"].as_array().expect("cases") {
        let name = text(&fall["name"]);
        let a = &fall["aufruf"];
        let p = Probelauf {
            zeit_ms: a["zeit_ms"].as_i64().expect("zeit"),
            programm: text(&a["programm"]).to_owned(),
            sitzung: text(&a["sitzung"]).to_owned(),
            anzahl: a["anzahl"]
                .as_u64()
                .and_then(|n| u32::try_from(n).ok())
                .expect("anzahl"),
        };
        let ist = zeile_probelauf(&p, text(&fall["vorher"]));
        let e = &fall["erwartet"];
        if let Some(code) = e["code"].as_str() {
            assert_eq!(ist.map_err(|f| f.code()), Err(code.to_owned()), "{name}");
        } else {
            let z = ist.unwrap_or_else(|f| panic!("{name}: {}", f.code()));
            assert_eq!(z, text(&e["zeile"]), "{name}");
            assert_eq!(sha256_hex(z.as_bytes()), text(&e["sha256"]), "{name}");
        }
    }
}

/// The argv of a case with the paths moved into the folder of the test.
fn argv_lokal(argv: &[Value], w: &Wegwerf, zaehldatei: &Path) -> Vec<OsString> {
    let mut v: Vec<OsString> = Vec::new();
    let mut vorige = "";
    for a in argv {
        let a = text(a);
        let ersetzt = match vorige {
            "--socket" => w.pfad("m.sock").into_os_string(),
            "--schluessel-datei" => w.pfad("schluessel").into_os_string(),
            "--protokoll" => w.pfad("protokoll").into_os_string(),
            "--marker-zaehlung" => zaehldatei.as_os_str().to_owned(),
            _ => OsString::from(a),
        };
        v.push(ersetzt);
        vorige = a;
    }
    v
}

#[test]
fn startfolge_cases() {
    let f = fall_datei("probelauf.json");
    for fall in f["startfolge"].as_array().expect("cases") {
        let name = text(&fall["name"]);
        let w = Wegwerf::neu("start");
        w.schluessel(format!("{}\n", gemeinsam::SCHLUESSEL).as_bytes());
        let protokoll = w.protokoll();
        let zaehldatei = if fall["zaehldatei_schreibbar"] == true {
            w.pfad("marker.json")
        } else {
            w.pfad("fehlt").join("marker.json")
        };
        if fall["protokoll_schreibbar"] == false {
            rechte(&protokoll, 0o500);
        }
        let args = argv_lokal(fall["argv"].as_array().expect("argv"), &w, &zaehldatei);
        let zeit = fall["zeit_ms"].as_i64().expect("zeit");
        let aufloeser = |_: &str, _: u16| -> io::Result<Vec<SocketAddr>> {
            Ok(vec![SocketAddr::from(([127, 0, 0, 1], 9))])
        };
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let ergebnis = starten(
            &args,
            rustix::process::getuid().as_raw(),
            &aufloeser,
            FRISTEN,
            Box::new(move || zeit),
            &mut out,
            &mut err,
        );
        rechte(&protokoll, 0o700);
        let e = &fall["erwartet"];
        match (ergebnis, e["exit"].as_i64()) {
            (Ok((bereit, _signale)), None) => {
                assert!(w.pfad("m.sock").exists(), "{name}");
                drop(bereit);
            }
            (Err(exit), Some(soll)) => assert_eq!(i64::from(exit), soll, "{name}"),
            (Ok(_), Some(soll)) => panic!("{name}: started, expected exit {soll}"),
            (Err(exit), None) => panic!("{name}: exit {exit}"),
        }
        assert_eq!(String::from_utf8_lossy(&out), text(&e["stdout"]), "{name}");
        assert_eq!(String::from_utf8_lossy(&err), text(&e["stderr"]), "{name}");
        let mut kette = Vec::new();
        if let Ok(eintraege) = fs::read_dir(&protokoll) {
            for d in eintraege {
                let p = d.expect("entry").path();
                if p.extension().is_some_and(|x| x == "jsonl")
                    && !p.to_string_lossy().contains(".anker.")
                {
                    kette.extend(fs::read(&p).expect("read"));
                }
            }
        }
        assert_eq!(
            String::from_utf8_lossy(&kette),
            text(&e["protokoll_neu"]),
            "{name}"
        );
        match e.get("zaehldatei") {
            Some(Value::String(inhalt)) => {
                assert_eq!(
                    fs::read(&zaehldatei).expect("count file"),
                    latin1(inhalt),
                    "{name}"
                );
            }
            Some(Value::Null) => assert!(!zaehldatei.exists(), "{name}"),
            _ => {}
        }
        if e["socket"] == "nicht angelegt" {
            assert!(!w.pfad("m.sock").exists(), "{name}");
        }
    }
}
