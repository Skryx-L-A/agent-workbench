//! The visible cases of contract 8 (`rust/spec/modellweg/faelle/`): every case of the four files runs here.
//! Requests and answers run through the library against a scripted target (no network); the call and the key file also
//! run through the program itself.
#![cfg(unix)] // the crate is Unix only (see `src/lib.rs`)
#![allow(
    clippy::many_single_char_names,
    reason = "short names for case, answer, line and process keep the tables of the tests readable"
)]

mod gemeinsam;

use std::ffi::OsString;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use formate::kette::{NULL_HASH, pruefe, sha256_hex};
use gemeinsam::{
    Antwort, FRIST, Prozess, Skript, Wegwerf, anfrage, argv, dienst, dienst_pfad_voll, fall_datei,
    latin1, lauf_bis_ende, text, zeilen,
};
use modellweg::aufruf::{self, Aufruffehler};
use modellweg::pfad::{pfad_feld, pfad_feld_ok};
use modellweg::protokoll::{Modellweg, anhaengen_modellweg, zeile_modellweg};
use modellweg::schluessel::{self, Schluesselfehler};
use serde_json::Value;

fn args(v: &Value) -> Vec<OsString> {
    v.as_array()
        .expect("argv")
        .iter()
        .map(|a| OsString::from(text(a)))
        .collect()
}

#[test]
fn aufruf_cases() {
    let f = fall_datei("aufruf.json");
    let faelle = f["aufruf"].as_array().expect("cases");
    assert!(faelle.len() >= 26, "{}", faelle.len());
    let p = fall_datei("probelauf.json");
    let probelauf = p["aufruf"].as_array().expect("cases");
    assert_eq!(probelauf.len(), 16);
    for fall in faelle.iter().chain(probelauf) {
        let name = text(&fall["name"]);
        let argv = args(&fall["argv"]);
        // Where mode 2 is built (Linux), a case with `erwartet_gebaut` expects that (contract 8, sections 2 and 13.5).
        let e = match fall.get("erwartet_gebaut") {
            Some(gebaut) if cfg!(target_os = "linux") => gebaut,
            _ => &fall["erwartet"],
        };
        let ist = aufruf::lies(&argv);
        if e["bis_schritt"] == "aufruf_gut" {
            assert!(ist.is_ok(), "{name}: {ist:?}");
            continue;
        }
        let code = text(&e["stderr"])
            .split(':')
            .next()
            .expect("code")
            .to_owned();
        let soll = match code.as_str() {
            "mw.aufruf" => Aufruffehler::Aufruf,
            "mw.betriebsart_unbekannt" => Aufruffehler::BetriebsartUnbekannt,
            "mw.probelauf_aufruf" => Aufruffehler::ProbelaufAufruf,
            anders => panic!("{name}: code {anders}"),
        };
        assert_eq!(ist.err(), Some(soll), "{name}");
        let (exit, out, err) = lauf_bis_ende(&argv);
        assert_eq!(i64::from(exit), e["exit"].as_i64().expect("exit"), "{name}");
        assert_eq!(out, latin1(text(&e["stdout"])), "{name}");
        assert_eq!(err, latin1(text(&e["stderr"])), "{name}");
    }
}

/// Makes the key file of a case at `pfad` and returns the uid it must belong to.
fn schluesseldatei(w: &Wegwerf, pfad: &std::path::Path, d: &Value) -> u32 {
    let eigene = rustix::process::getuid().as_raw();
    let modus = |p: &std::path::Path| {
        let m = u32::from_str_radix(text(&d["modus"]), 8).expect("mode");
        fs::set_permissions(p, fs::Permissions::from_mode(m)).expect("chmod");
    };
    match text(&d["art"]) {
        "datei" => {
            fs::write(pfad, latin1(text(&d["inhalt"]))).expect("write");
            modus(pfad);
        }
        "link" => {
            let ziel = w.pfad("linkziel");
            fs::write(&ziel, latin1(text(&d["inhalt"]))).expect("write");
            modus(&ziel);
            std::os::unix::fs::symlink(&ziel, pfad).expect("symlink");
        }
        "ordner" => {
            fs::create_dir(pfad).expect("folder");
            modus(pfad);
        }
        "fehlt" => {}
        anders => panic!("art {anders}"),
    }
    if d["eigen"] == false {
        eigene + 1
    } else {
        eigene
    }
}

#[test]
fn schluesseldatei_cases() {
    let f = fall_datei("aufruf.json");
    for fall in f["schluesseldatei"].as_array().expect("cases") {
        let name = text(&fall["name"]);
        let e = &fall["erwartet"];
        let w = Wegwerf::neu("sd");
        let pfad = w.pfad("modellweg.schluessel");
        let uid = schluesseldatei(&w, &pfad, &fall["datei"]);
        let ist = schluessel::lies(&pfad, uid);
        if e["gueltig"] == true {
            let k = ist.unwrap_or_else(|f| panic!("{name}: {f:?}"));
            assert_eq!(k.bytes(), gemeinsam::SCHLUESSEL.as_bytes(), "{name}");
        } else {
            let soll = match text(&e["stderr"]).split(':').next() {
                Some("mw.schluessel_datei") => Schluesselfehler::Datei,
                Some("mw.schluessel_form") => Schluesselfehler::Form,
                anders => panic!("{name}: {anders:?}"),
            };
            assert_eq!(ist.err(), Some(soll), "{name}");
        }
        if fall["datei"]["eigen"] == false {
            continue;
        }
        let socket = w.pfad("s.sock");
        let a = argv(&socket, "http://127.0.0.1:9/v1", &pfad, &w.protokoll());
        if e["gueltig"] == true {
            let (mut p, erste) = Prozess::start(&a, &[]);
            assert_eq!(erste, "{\"bereit\":true}", "{name}");
            assert_eq!(p.beende(), Some(0), "{name}");
        } else {
            let (exit, out, err) = lauf_bis_ende(&a);
            assert_eq!(i64::from(exit), e["exit"].as_i64().expect("exit"), "{name}");
            assert_eq!(out, latin1(text(&e["stdout"])), "{name}");
            assert_eq!(err, latin1(text(&e["stderr"])), "{name}");
            assert!(!socket.exists(), "{name}");
        }
    }
}

/// The case `ausgelastet`: eight requests wait at a closed target, the ninth gets 429 without touching it.
fn ausgelastet(w: &Wegwerf, lage: &Value, roh: &[u8]) -> (Vec<u8>, usize) {
    let tor = Arc::new((Mutex::new(false), Condvar::new()));
    let skript = Skript {
        tor: Some(Arc::clone(&tor)),
        ..Skript::ok()
    };
    let d = dienst(
        w,
        text(&lage["ziel"]),
        text(&lage["modell"]),
        text(&lage["schluessel"]),
        Box::new(skript.clone()),
    );
    let laufend = usize::try_from(lage["laufend"].as_u64().expect("laufend")).expect("small");
    let faeden: Vec<_> = (0..laufend)
        .map(|_| {
            let d = Arc::clone(&d);
            let roh = roh.to_vec();
            std::thread::spawn(move || anfrage(&d, &roh))
        })
        .collect();
    let ende = Instant::now() + FRIST;
    while skript.verbunden.load(Ordering::SeqCst) < laufend {
        assert!(
            Instant::now() < ende,
            "the requests did not reach the target"
        );
        std::thread::sleep(Duration::from_millis(2));
    }
    let antwort = anfrage(&d, roh);
    let beruehrt = skript.verbunden.load(Ordering::SeqCst) - laufend;
    skript.oeffnen();
    for f in faeden {
        let a = Antwort::lies(&f.join().expect("thread"));
        assert_eq!(a.status, 200);
    }
    (antwort, beruehrt)
}

#[test]
fn anfragen_cases() {
    let f = fall_datei("anfragen.json");
    let faelle = f["faelle"].as_array().expect("cases");
    assert_eq!(faelle.len(), 27);
    for fall in faelle {
        let name = text(&fall["name"]);
        let lage = &fall["lage"];
        let e = &fall["erwartet"];
        assert_eq!(lage["betriebsart"], "openai-lokal", "{name}");
        let roh = latin1(text(&fall["anfrage"]));
        let w = Wegwerf::neu("an");
        let (antwort, beruehrt, skript) = if lage["laufend"].as_u64() > Some(0) {
            let (a, b) = ausgelastet(&w, lage, &roh);
            (a, b, None)
        } else {
            let skript = Skript::ok();
            let d = dienst(
                &w,
                text(&lage["ziel"]),
                text(&lage["modell"]),
                text(&lage["schluessel"]),
                Box::new(skript.clone()),
            );
            if lage["protokoll_ok"] == false {
                fs::remove_dir_all(w.protokoll()).expect("remove the protocol folder");
                let erste = Antwort::lies(&anfrage(&d, b"GET /x HTTP/1.1\r\n\r\n"));
                assert_eq!(erste.status, 404, "{name}");
                w.protokoll();
            }
            let a = anfrage(&d, &roh);
            let b = skript.verbunden.load(Ordering::SeqCst);
            (a, b, Some(skript))
        };
        let antwort = Antwort::lies(&antwort);
        let alle = zeilen(&w.protokoll());
        if e["ziel_beruehrt"] == true {
            let skript = skript.expect("script");
            assert_eq!(
                skript.gesehen(),
                vec![latin1(text(&e["an_ziel"]))],
                "{name}"
            );
            assert_eq!(
                (antwort.status, antwort.koerper.as_slice()),
                (200, &b"{}"[..]),
                "{name}"
            );
            assert_eq!(alle.len(), 1, "{name}");
            assert_eq!(alle[0]["ergebnis"], "weitergereicht", "{name}");
            continue;
        }
        assert_eq!(beruehrt, 0, "{name}: the target was touched");
        assert_eq!(
            i64::from(antwort.status),
            e["status"].as_i64().expect("status"),
            "{name}"
        );
        assert_eq!(antwort.koerper, latin1(text(&e["koerper"])), "{name}");
        assert_eq!(
            antwort.ohne_rahmung(),
            vec![("content-type".to_owned(), "application/json".to_owned())],
            "{name}"
        );
        if e["zeile"].is_null() {
            assert!(alle.is_empty(), "{name}: {alle:?}");
            continue;
        }
        let z = &alle[0];
        for feld in ["ergebnis", "grund", "status", "pfad", "host"] {
            assert_eq!(z[feld], e["zeile"][feld], "{name}: {feld}");
        }
        assert_eq!(z["modell"], lage["modell"], "{name}");
        assert_eq!(
            (z["token_ein"].is_null(), z["token_aus"].is_null()),
            (true, true),
            "{name}"
        );
        assert_eq!(z["betriebsart"], "openai-lokal", "{name}");
        assert_eq!(z["zeit"], "2026-11-20T15:00:00.000Z", "{name}");
    }
}

#[test]
fn antworten_cases() {
    let f = fall_datei("antworten.json");
    let schluessel = text(&f["schluessel"]);
    let faelle = f["faelle"].as_array().expect("cases");
    assert_eq!(faelle.len(), 11);
    for fall in faelle {
        let name = text(&fall["name"]);
        let ziel = &fall["ziel"];
        let mut kopf = format!("HTTP/1.1 {} X\r\n", ziel["status"]);
        for k in ziel["kopf"].as_array().expect("head") {
            kopf.push_str(text(&k[0]));
            kopf.push_str(": ");
            kopf.push_str(text(&k[1]));
            kopf.push_str("\r\n");
        }
        kopf.push_str("\r\n");
        let mut teile = vec![latin1(&kopf)];
        teile.extend(
            ziel["teile"]
                .as_array()
                .expect("parts")
                .iter()
                .map(|t| latin1(text(t))),
        );
        let w = Wegwerf::neu("aw");
        let skript = Skript::antwortet(teile);
        let d = dienst(
            &w,
            "https://modelle.firma.beispiel.invalid/v1",
            gemeinsam::MODELL,
            schluessel,
            Box::new(skript),
        );
        let roh = anfrage(&d, &gemeinsam::post(&gemeinsam::koerper("Hallo"), ""));
        let e = &fall["erwartet"];
        let h = &e["an_harness"];
        let a = Antwort::lies(&roh);
        assert_eq!(
            i64::from(a.status),
            h["status"].as_i64().expect("status"),
            "{name}"
        );
        let kopf_soll: Vec<(String, String)> = h["kopf"]
            .as_array()
            .expect("head")
            .iter()
            .map(|k| (text(&k[0]).to_owned(), text(&k[1]).to_owned()))
            .collect();
        assert_eq!(a.ohne_rahmung(), kopf_soll, "{name}");
        let koerper_soll = match text(&h["art"]) {
            "abgerissen" => &h["gesendet"],
            _ => &h["koerper"],
        };
        assert_eq!(
            String::from_utf8_lossy(&a.koerper),
            String::from_utf8_lossy(&latin1(text(koerper_soll))),
            "{name}"
        );
        assert!(!gemeinsam::enthaelt(&roh, schluessel.as_bytes()), "{name}");
        let alle = zeilen(&w.protokoll());
        assert_eq!(alle.len(), 1, "{name}");
        for feld in [
            "ergebnis",
            "grund",
            "status",
            "token_ein",
            "token_aus",
            "pfad",
            "host",
        ] {
            assert_eq!(alle[0][feld], e["zeile"][feld], "{name}: {feld}");
        }
    }
}

fn modellweg_aus(a: &Value) -> Modellweg {
    let tok = |v: &Value| v.as_u64();
    Modellweg {
        zeit_ms: a["zeit_ms"].as_i64().expect("zeit_ms"),
        betriebsart: text(&a["betriebsart"]).to_owned(),
        sitzung: text(&a["sitzung"]).to_owned(),
        route: text(&a["route"]).to_owned(),
        modell: text(&a["modell"]).to_owned(),
        ergebnis: text(&a["ergebnis"]).to_owned(),
        grund: a["grund"].as_str().map(str::to_owned),
        status: u32::try_from(a["status"].as_u64().expect("status")).expect("u32"),
        token_ein: tok(&a["token_ein"]),
        token_aus: tok(&a["token_aus"]),
        pfad: a["pfad"].as_str().map(str::to_owned),
        host: a["host"].as_str().map(str::to_owned),
    }
}

#[test]
fn zeile_cases() {
    let f = fall_datei("zeile.json");
    let faelle = f["zeilen"].as_array().expect("cases");
    assert!(faelle.len() >= 25, "{}", faelle.len());
    for fall in faelle {
        let name = text(&fall["name"]);
        let m = modellweg_aus(&fall["aufruf"]);
        let ist = zeile_modellweg(&m, text(&fall["vorher"]));
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

/// The cases of the field `pfad` (contract 8, section 8) against the function of the library; they stand in
/// `kopf-zeile.json`, but the field is the same in both modes, and the check needs no mode.
#[test]
fn pfad_cases() {
    let f = fall_datei("kopf-zeile.json");
    let faelle = f["pfade"].as_array().expect("cases");
    assert!(faelle.len() >= 27, "{}", faelle.len());
    for fall in faelle {
        let name = text(&fall["name"]);
        let voll = fall["voll"].as_bool().expect("voll");
        let ist = pfad_feld(text(&fall["ziel"]), voll);
        assert_eq!(ist, text(&fall["erwartet"]), "{name}");
        assert!(pfad_feld_ok(&ist), "{name}: {ist}");
        assert!(!ist.contains(['?', '#']), "{name}: {ist}");
    }
}

/// The line of a rejected request carries the limited path; a request that is handed through carries none
/// (contract 8, section 8). With the admin switch the whole path stands there.
#[test]
fn eine_abgelehnte_anfrage_traegt_den_begrenzten_pfad() {
    let w = Wegwerf::neu("pf");
    let skript = Skript::ok();
    let d = dienst(
        &w,
        "https://ziel.beispiel.invalid/v1",
        "m",
        "mw-testschluessel-1",
        Box::new(skript.clone()),
    );
    let a = Antwort::lies(&anfrage(
        &d,
        b"GET /models/session/x?a=b HTTP/1.1\r\nHost: x\r\n\r\n",
    ));
    assert_eq!(a.status, 404);
    let z = zeilen(&w.protokoll());
    assert_eq!(z.len(), 1);
    assert_eq!(z[0]["ergebnis"], "abgelehnt");
    assert_eq!(z[0]["grund"], "mw.pfad_unbekannt");
    assert_eq!(z[0]["pfad"], "/models/session");
    assert!(z[0]["host"].is_null());
    assert_eq!(skript.verbunden.load(Ordering::SeqCst), 0);

    let w2 = Wegwerf::neu("pv");
    let d2 = dienst_pfad_voll(
        &w2,
        "https://ziel.beispiel.invalid/v1",
        "m",
        "mw-testschluessel-1",
        Box::new(Skript::ok()),
    );
    let a2 = Antwort::lies(&anfrage(
        &d2,
        b"GET /models/session/x?a=b HTTP/1.1\r\nHost: x\r\n\r\n",
    ));
    assert_eq!(a2.status, 404);
    let z2 = zeilen(&w2.protokoll());
    assert_eq!(z2[0]["pfad"], "/models/session/x");
}

#[test]
fn zeile_chains() {
    let f = fall_datei("zeile.json");
    for kette in f["ketten"].as_array().expect("chains") {
        let name = text(&kette["name"]);
        let w = Wegwerf::neu("kt");
        let datei = w.pfad(text(&kette["datei"]));
        fs::write(&datei, latin1(text(&kette["anfang"]))).expect("write");
        let rueckgaben: Vec<Value> = kette["aufrufe"]
            .as_array()
            .expect("calls")
            .iter()
            .map(|a| {
                Value::from(anhaengen_modellweg(w.ordner(), &modellweg_aus(a)).expect("append"))
            })
            .collect();
        let e = &kette["erwartet"];
        assert_eq!(&Value::from(rueckgaben), &e["rueckgaben"], "{name}");
        assert_eq!(
            fs::read(&datei).expect("read"),
            latin1(text(&e["inhalt"])),
            "{name}"
        );
        let pruefung: Value =
            serde_json::from_str(&pruefe(&[&datei], NULL_HASH, None).ausgabe()).expect("JSON");
        assert_eq!(pruefung, e["pruefe"], "{name}");
    }
}

#[test]
fn a_refused_line_writes_nothing() {
    let w = Wegwerf::neu("zn");
    let mut m = modellweg_aus(&fall_datei("zeile.json")["zeilen"][0]["aufruf"]);
    m.route = "Lokal".to_owned();
    assert_eq!(
        anhaengen_modellweg(w.ordner(), &m).map_err(|f| f.code()),
        Err("feld_ungueltig:route".to_owned())
    );
    assert_eq!(fs::read_dir(w.ordner()).expect("dir").count(), 0);
    m.route = "lokal-beispiel".to_owned();
    m.zeit_ms = -1;
    assert_eq!(
        anhaengen_modellweg(w.ordner(), &m).map_err(|f| f.code()),
        Err("zeit_ungueltig".to_owned())
    );
}
