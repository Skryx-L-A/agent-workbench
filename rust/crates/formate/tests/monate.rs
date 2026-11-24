//! The visible cases of contract 5, section 5 (`faelle/monate.json`, `faelle/pruefe-quelle.json`):
//! writing over months with anchors, and the check of a folder. Every case runs on a throw-away folder; the
//! command cases start the program `kette`.

mod gemeinsam;

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs;
use std::path::Path;
use std::process::Command;
use std::sync::Barrier;

use formate::kette::{
    Abgewiesen, Fehler, NULL_HASH, Ordnermarke, Probelauf, anhaengen_abgewiesen,
    anhaengen_gesperrt_in, anhaengen_ordnermarke, anhaengen_probelauf, anhaengen_zeile,
    anhaengen_zeilen, befehl, letzte_zeile_der_art, pruefe_quelle, sha256_hex, zeile_abgewiesen,
    zeile_ordnermarke, zeile_probelauf,
};
use gemeinsam::{Wegwerf, base64, fall_datei, gesperrt_aus, liste, text};
use serde_json::Value;

/// The files of a folder: name to content.
fn ordner_inhalt(ordner: &Path) -> BTreeMap<String, String> {
    let mut aus = BTreeMap::new();
    for eintrag in fs::read_dir(ordner).expect("read the folder") {
        let eintrag = eintrag.expect("an entry");
        aus.insert(
            eintrag.file_name().to_string_lossy().into_owned(),
            fs::read_to_string(eintrag.path()).expect("a text file"),
        );
    }
    aus
}

fn aufruf(ordner: &Path, a: &Value) -> Result<String, Fehler> {
    let quelle = text(a, "quelle");
    match text(a, "art") {
        "gesperrt" => anhaengen_gesperrt_in(ordner, quelle, &gesperrt_aus(&a["eingabe"])),
        "zeile" => anhaengen_zeile(
            ordner,
            quelle,
            a["zeit_ms"].as_i64().expect("zeit_ms"),
            &a["objekt"],
        ),
        "zeilen" => {
            let zeilen: Vec<String> = liste(a, "zeilen")
                .iter()
                .map(|z| z.as_str().expect("a line").to_owned())
                .collect();
            anhaengen_zeilen(ordner, quelle, text(a, "erwartet_vorher"), &zeilen)
        }
        "abgewiesen" => anhaengen_abgewiesen(ordner, quelle, &abgewiesen_aus(&a["eingabe"])),
        "probelauf" => anhaengen_probelauf(ordner, quelle, &probelauf_aus(&a["eingabe"])),
        "ordnermarke" => anhaengen_ordnermarke(ordner, quelle, &ordnermarke_aus(&a["eingabe"])),
        art => panic!("unknown call kind {art}"),
    }
}

/// The data of a line `kanal.abgewiesen` from the input of a case; a value that is no text becomes an empty text,
/// which no field accepts.
fn abgewiesen_aus(eingabe: &Value) -> Abgewiesen {
    let pflicht = |schluessel: &str| eingabe[schluessel].as_str().unwrap_or_default().to_owned();
    let option = |schluessel: &str| eingabe[schluessel].as_str().map(str::to_owned);
    Abgewiesen {
        zeit_ms: eingabe["zeit_ms"].as_i64().expect("zeit_ms"),
        grund: pflicht("grund"),
        kanal: pflicht("kanal"),
        route: option("route"),
        sitzung: option("sitzung"),
    }
}

/// The data of a line `probelauf.zaehlung` from the input of a case; a value that is no text becomes an empty text.
fn probelauf_aus(eingabe: &Value) -> Probelauf {
    let pflicht = |schluessel: &str| eingabe[schluessel].as_str().unwrap_or_default().to_owned();
    Probelauf {
        zeit_ms: eingabe["zeit_ms"].as_i64().expect("zeit_ms"),
        programm: pflicht("programm"),
        sitzung: pflicht("sitzung"),
        anzahl: u32::try_from(eingabe["anzahl"].as_u64().expect("anzahl")).expect("u32"),
    }
}

/// The data of a line `ordnermarke.*` from the input of a case; a value that is no text becomes an empty text for
/// a field that must be text, and `None` for a field that may be `null`.
fn ordnermarke_aus(eingabe: &Value) -> Ordnermarke {
    let pflicht = |schluessel: &str| eingabe[schluessel].as_str().unwrap_or_default().to_owned();
    let option = |schluessel: &str| eingabe[schluessel].as_str().map(str::to_owned);
    Ordnermarke {
        zeit_ms: eingabe["zeit_ms"].as_i64().expect("zeit_ms"),
        aktionsname: pflicht("aktionsname"),
        marke: pflicht("marke"),
        klasse_alt: option("klasse_alt"),
        klasse_neu: option("klasse_neu"),
        kunde_alt: option("kunde_alt"),
        kunde_neu: option("kunde_neu"),
    }
}

fn kette(args: &[OsString]) -> (i32, String) {
    let aus = Command::new(env!("CARGO_BIN_EXE_kette"))
        .args(args)
        .output()
        .expect("start kette");
    (
        aus.status.code().expect("an exit code"),
        String::from_utf8(aus.stdout).expect("UTF-8"),
    )
}

#[test]
fn month_cases() {
    let datei = fall_datei("monate.json");
    let faelle = liste(&datei, "faelle");
    assert_eq!(faelle.len(), 40);
    spiele(faelle, "monate");
}

/// Runs the cases of writing: a folder with the start, the calls, the files at the end and the check of the sources.
fn spiele(faelle: &[Value], kennung: &str) {
    for (nummer, fall) in faelle.iter().enumerate() {
        let name = text(fall, "fall");
        let ordner = Wegwerf::neu(&format!("{kennung}-{nummer}"));
        for (datei, inhalt) in fall["anfang"].as_object().expect("anfang") {
            ordner.schreibe(datei, inhalt.as_str().expect("text").as_bytes());
        }
        for a in liste(fall, "aufrufe") {
            let ist = aufruf(ordner.ordner(), a);
            let erwartet = &a["erwartet"];
            match (ist, erwartet["fehler"].as_str()) {
                (Err(fehler), Some(code)) => assert_eq!(fehler.code(), code, "{name}"),
                (Ok(hash), None) => assert_eq!(hash, text(erwartet, "sha256"), "{name}"),
                (ist, soll) => panic!("{name}: got {ist:?}, wanted error {soll:?}"),
            }
        }
        let soll: BTreeMap<String, String> = fall["ende"]
            .as_object()
            .expect("ende")
            .iter()
            .map(|(k, v)| (k.clone(), v.as_str().expect("text").to_owned()))
            .collect();
        assert_eq!(
            ordner_inhalt(ordner.ordner()),
            soll,
            "{name}: files at the end"
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            // Files the case put there are not ours; the files the writer made have 0600.
            for datei in soll
                .keys()
                .filter(|d| fall["anfang"].get(d.as_str()).is_none())
            {
                let modus = fs::metadata(ordner.pfad(datei))
                    .expect("metadata")
                    .permissions()
                    .mode()
                    & 0o777;
                assert_eq!(modus, 0o600, "{name}: mode of {datei}");
            }
        }
        for (quelle, erwartet) in fall["pruefe_quelle"].as_object().expect("pruefe_quelle") {
            let args: Vec<OsString> = vec![
                "pruefe-quelle".into(),
                ordner.ordner().into(),
                quelle.into(),
            ];
            let (exit, stdout) = kette(&args);
            assert_eq!(
                i64::from(exit),
                erwartet["exit"].as_i64().expect("exit"),
                "{name}: check {quelle}"
            );
            assert_eq!(stdout, text(erwartet, "stdout"), "{name}: check {quelle}");
        }
    }
}

#[test]
fn two_writers_over_a_month_border() {
    let datei = fall_datei("monate.json");
    let fall = &datei["zwei_schreiber_monatsgrenze"];
    let basis = gesperrt_aus(&fall["basis"]);
    let je = fall["je_schreiber"].as_u64().expect("je_schreiber");
    let runden = fall["runden"].as_u64().expect("runden");
    assert_eq!((je, runden), (50, 10));
    let erwartet = &fall["erwartet"];
    let soll_zeilen = erwartet["pruefe_quelle"]["zeilen"]
        .as_u64()
        .expect("zeilen");
    assert_eq!(soll_zeilen, 102);
    for runde in 0..runden {
        let ordner = Wegwerf::neu(&format!("grenze-{runde}"));
        for (datei, inhalt) in fall["anfang"].as_object().expect("anfang") {
            ordner.schreibe(datei, inhalt.as_str().expect("text").as_bytes());
        }
        let start = Barrier::new(2);
        std::thread::scope(|bereich| {
            for sitzung in ["s-a", "s-b"] {
                let (pfad, basis, start) = (ordner.ordner(), &basis, &start);
                bereich.spawn(move || {
                    start.wait();
                    for i in 1..=je {
                        let mut g = basis.clone();
                        g.sitzung = sitzung.to_owned();
                        g.anzahl = u32::try_from(i).expect("small");
                        g.zeit_ms += i64::try_from(i).expect("small");
                        anhaengen_gesperrt_in(pfad, "brain", &g).expect("append under the lock");
                    }
                });
            }
        });
        let anker = fs::read_to_string(ordner.pfad("brain.anker.jsonl")).expect("anchor file");
        assert_eq!(
            anker,
            format!("{}\n", text(erwartet, "anker")),
            "round {runde}: exactly one anchor"
        );
        assert_eq!(erwartet["anker_zeilen"].as_u64(), Some(1));
        let pruefung = pruefe_quelle(ordner.ordner(), "brain", NULL_HASH, None).expect("source");
        assert_eq!(pruefung.bruch, None, "round {runde}");
        assert_eq!(pruefung.zeilen, soll_zeilen, "round {runde}");
        let dezember = fs::read_to_string(ordner.pfad("brain-2026-12.jsonl")).expect("december");
        let mut je_sitzung: BTreeMap<String, Vec<u64>> = BTreeMap::new();
        for zeile in dezember.lines() {
            let wert: Value = serde_json::from_str(zeile).expect("a line is JSON");
            je_sitzung
                .entry(text(&wert, "sitzung").to_owned())
                .or_default()
                .push(wert["anzahl"].as_u64().expect("anzahl"));
        }
        let soll: Vec<u64> = erwartet["je_sitzung_anzahl"]
            .as_array()
            .expect("list")
            .iter()
            .map(|n| n.as_u64().expect("number"))
            .collect();
        assert_eq!(je_sitzung.len(), 2, "round {runde}");
        for (sitzung, anzahlen) in je_sitzung {
            assert_eq!(anzahlen, soll, "round {runde}: {sitzung}");
        }
    }
}

#[test]
fn check_cases_of_a_folder() {
    let datei = fall_datei("pruefe-quelle.json");
    let faelle = liste(&datei, "faelle");
    assert_eq!(faelle.len(), 62);
    for (nummer, fall) in faelle.iter().enumerate() {
        let name = text(fall, "fall");
        let ordner = Wegwerf::neu(&format!("quelle-{nummer}"));
        for datei in liste(fall, "dateien") {
            if datei["fehlt"].as_bool() == Some(true) {
                continue;
            }
            let inhalt = match datei["inhalt"].as_str() {
                Some(text) => text.as_bytes().to_vec(),
                None => base64(text(datei, "inhalt_b64")),
            };
            ordner.schreibe(text(datei, "name"), &inhalt);
        }
        let args: Vec<OsString> = liste(fall, "aufruf")
            .iter()
            .map(|a| {
                let a = a.as_str().expect("an argument");
                match a {
                    "@@" => ordner.ordner().as_os_str().to_owned(),
                    _ => match a.strip_prefix('@') {
                        Some(datei) => ordner.pfad(datei).into_os_string(),
                        None => OsString::from(a),
                    },
                }
            })
            .collect();
        let soll_exit = fall["erwartet"]["exit"].as_i64().expect("exit");
        let soll_stdout = text(&fall["erwartet"], "stdout");
        let (exit, stdout) = kette(&args);
        assert_eq!(i64::from(exit), soll_exit, "{name}: exit");
        assert_eq!(stdout, soll_stdout, "{name}: stdout");
        let ohne = befehl(&args);
        assert_eq!(i64::from(ohne.exit), soll_exit, "{name}: exit (function)");
        assert_eq!(ohne.stdout, soll_stdout, "{name}: stdout (function)");
    }
}

#[test]
fn closed_line_cases_of_a_channel_turned_away() {
    let datei = fall_datei("abgewiesen.json");
    let zeilen = liste(&datei, "zeile");
    assert_eq!(zeilen.len(), 51);
    for fall in zeilen {
        let name = text(fall, "fall");
        let ist = zeile_abgewiesen(&abgewiesen_aus(&fall["eingabe"]), text(fall, "vorher"));
        let erwartet = &fall["erwartet"];
        match (ist, erwartet["fehler"].as_str()) {
            (Err(fehler), Some(code)) => assert_eq!(fehler.code(), code, "{name}"),
            (Ok(zeile), None) => {
                assert_eq!(zeile, text(erwartet, "zeile"), "{name}");
                assert_eq!(
                    sha256_hex(zeile.as_bytes()),
                    text(erwartet, "sha256"),
                    "{name}"
                );
            }
            (ist, soll) => panic!("{name}: got {ist:?}, wanted error {soll:?}"),
        }
    }
}

/// Runs the check cases of a line kind: files with text, one call, exit and stdout.
fn pruefe_zeilenfaelle(faelle: &[Value], kennung: &str) {
    for (nummer, fall) in faelle.iter().enumerate() {
        let name = text(fall, "fall");
        let ordner = Wegwerf::neu(&format!("{kennung}-{nummer}"));
        for datei in liste(fall, "dateien") {
            ordner.schreibe(text(datei, "name"), text(datei, "inhalt").as_bytes());
        }
        let args: Vec<OsString> = liste(fall, "aufruf")
            .iter()
            .map(|a| {
                let a = a.as_str().expect("an argument");
                match a.strip_prefix('@') {
                    Some(datei) => ordner.pfad(datei).into_os_string(),
                    None => OsString::from(a),
                }
            })
            .collect();
        let soll_stdout = text(&fall["erwartet"], "stdout");
        let (exit, stdout) = kette(&args);
        assert_eq!(
            i64::from(exit),
            fall["erwartet"]["exit"].as_i64().expect("exit"),
            "{name}: exit"
        );
        assert_eq!(stdout, soll_stdout, "{name}: stdout");
        assert_eq!(
            befehl(&args).stdout,
            soll_stdout,
            "{name}: stdout (function)"
        );
    }
}

#[test]
fn check_cases_of_a_channel_turned_away() {
    let datei = fall_datei("abgewiesen.json");
    let faelle = liste(&datei, "pruefe");
    assert_eq!(faelle.len(), 26);
    pruefe_zeilenfaelle(faelle, "abgewiesen-pruefe");
}

#[test]
fn write_cases_of_a_channel_turned_away() {
    let datei = fall_datei("abgewiesen.json");
    let faelle = liste(&datei, "anhaengen");
    assert_eq!(faelle.len(), 9);
    spiele(faelle, "abgewiesen");
}

#[test]
fn the_writer_writes_the_line_the_builder_builds() {
    let ordner = Wegwerf::neu("abgewiesen-gleich");
    let a = Abgewiesen {
        zeit_ms: 1_795_179_600_000,
        grund: "pk.job_unbekannt".to_owned(),
        kanal: "weiterreicher".to_owned(),
        route: None,
        sitzung: Some("s-1".to_owned()),
    };
    let hash = anhaengen_abgewiesen(ordner.ordner(), "stelle", &a).expect("append");
    let zeile = zeile_abgewiesen(&a, NULL_HASH).expect("line");
    assert_eq!(hash, sha256_hex(zeile.as_bytes()));
    assert_eq!(
        fs::read_to_string(ordner.pfad("stelle-2026-11.jsonl")).expect("file"),
        format!("{zeile}\n")
    );
}

#[test]
fn closed_line_cases_of_a_trial_run_count() {
    let datei = fall_datei("probelauf.json");
    let zeilen = liste(&datei, "zeile");
    assert_eq!(zeilen.len(), 20);
    for fall in zeilen {
        let name = text(fall, "fall");
        let ist = zeile_probelauf(&probelauf_aus(&fall["eingabe"]), text(fall, "vorher"));
        let erwartet = &fall["erwartet"];
        match (ist, erwartet["fehler"].as_str()) {
            (Err(fehler), Some(code)) => assert_eq!(fehler.code(), code, "{name}"),
            (Ok(zeile), None) => {
                assert_eq!(zeile, text(erwartet, "zeile"), "{name}");
                assert_eq!(
                    sha256_hex(zeile.as_bytes()),
                    text(erwartet, "sha256"),
                    "{name}"
                );
            }
            (ist, soll) => panic!("{name}: got {ist:?}, wanted error {soll:?}"),
        }
    }
}

#[test]
fn check_cases_of_a_trial_run_count() {
    let datei = fall_datei("probelauf.json");
    let faelle = liste(&datei, "pruefe");
    assert_eq!(faelle.len(), 23);
    pruefe_zeilenfaelle(faelle, "probelauf-pruefe");
}

#[test]
fn write_cases_of_a_trial_run_count() {
    let datei = fall_datei("probelauf.json");
    let faelle = liste(&datei, "anhaengen");
    assert_eq!(faelle.len(), 7);
    spiele(faelle, "probelauf");
}

#[test]
fn the_trial_run_writer_writes_the_line_the_builder_builds() {
    let ordner = Wegwerf::neu("probelauf-gleich");
    let p = Probelauf {
        zeit_ms: 1_795_179_600_000,
        programm: "modellweg".to_owned(),
        sitzung: "s-1".to_owned(),
        anzahl: 16,
    };
    let hash = anhaengen_probelauf(ordner.ordner(), "stelle", &p).expect("append");
    let zeile = zeile_probelauf(&p, NULL_HASH).expect("line");
    assert_eq!(hash, sha256_hex(zeile.as_bytes()));
    assert_eq!(
        fs::read_to_string(ordner.pfad("stelle-2026-11.jsonl")).expect("file"),
        format!("{zeile}\n")
    );
}

#[test]
fn check_cases_of_the_person_lines() {
    let datei = fall_datei("personenzeilen.json");
    let faelle = liste(&datei, "pruefe");
    assert_eq!(faelle.len(), 43);
    pruefe_zeilenfaelle(faelle, "personen-pruefe");
    let schreiben = liste(&datei, "anhaengen");
    assert_eq!(schreiben.len(), 2);
    spiele(schreiben, "personen-anhaengen");
}

#[test]
fn check_cases_of_the_four_line_kinds_of_the_point() {
    let datei = fall_datei("stellezeilen.json");
    let faelle = liste(&datei, "pruefe");
    assert_eq!(faelle.len(), 51);
    pruefe_zeilenfaelle(faelle, "stelle-pruefe");
    let schreiben = liste(&datei, "anhaengen");
    assert_eq!(schreiben.len(), 2);
    spiele(schreiben, "stelle-anhaengen");
}

#[test]
fn last_line_cases() {
    let datei = fall_datei("letzte-zeile.json");
    let faelle = liste(&datei, "faelle");
    assert_eq!(faelle.len(), 24);
    for (nummer, fall) in faelle.iter().enumerate() {
        let name = text(fall, "fall");
        let ordner = Wegwerf::neu(&format!("letzte-zeile-{nummer}"));
        for datei in liste(fall, "dateien") {
            ordner.schreibe(text(datei, "name"), text(datei, "inhalt").as_bytes());
        }
        let pfad = match text(fall, "ordner") {
            "@@" => ordner.ordner().to_path_buf(),
            andere => ordner.pfad(andere.trim_start_matches('@')),
        };
        let filter: Vec<(&str, Value)> = fall["filter"]
            .as_object()
            .expect("filter")
            .iter()
            .map(|(k, v)| (k.as_str(), v.clone()))
            .collect();
        let ist = letzte_zeile_der_art(&pfad, text(fall, "quelle"), text(fall, "vorher"), &filter);
        let erwartet = &fall["erwartet"];
        match (ist, erwartet["fehler"].as_str()) {
            (Err(fehler), Some(code)) => {
                assert_eq!(fehler.code(), code, "{name}");
                if let Fehler::KetteBruch(bruch) = &fehler {
                    assert_eq!(
                        &bruch.datei,
                        erwartet["datei"].as_str().expect("datei"),
                        "{name}"
                    );
                    assert_eq!(bruch.zeile, erwartet["zeile"].as_u64(), "{name}");
                }
            }
            (Ok(None), None) => assert_eq!(erwartet["keine"], true, "{name}"),
            (Ok(Some(gefunden)), None) => {
                assert_eq!(gefunden.zeile, erwartet["zeile"], "{name}");
                assert_eq!(gefunden.sha256, text(erwartet, "sha256"), "{name}");
            }
            (ist, soll) => panic!("{name}: got {ist:?}, wanted error {soll:?}"),
        }
    }
}

#[test]
fn closed_line_cases_of_a_folder_mark() {
    let datei = fall_datei("ordnermarke.json");
    let zeilen = liste(&datei, "zeile");
    assert_eq!(zeilen.len(), 43);
    for fall in zeilen {
        let name = text(fall, "fall");
        let ist = zeile_ordnermarke(&ordnermarke_aus(&fall["eingabe"]), text(fall, "vorher"));
        let erwartet = &fall["erwartet"];
        match (ist, erwartet["fehler"].as_str()) {
            (Err(fehler), Some(code)) => assert_eq!(fehler.code(), code, "{name}"),
            (Ok(zeile), None) => {
                assert_eq!(zeile, text(erwartet, "zeile"), "{name}");
                assert_eq!(
                    sha256_hex(zeile.as_bytes()),
                    text(erwartet, "sha256"),
                    "{name}"
                );
            }
            (ist, soll) => panic!("{name}: got {ist:?}, wanted error {soll:?}"),
        }
    }
}

#[test]
fn check_cases_of_a_folder_mark() {
    let datei = fall_datei("ordnermarke.json");
    let faelle = liste(&datei, "pruefe");
    assert_eq!(faelle.len(), 39);
    pruefe_zeilenfaelle(faelle, "ordnermarke-pruefe");
}

#[test]
fn write_cases_of_a_folder_mark() {
    let datei = fall_datei("ordnermarke.json");
    let faelle = liste(&datei, "anhaengen");
    assert_eq!(faelle.len(), 7);
    spiele(faelle, "ordnermarke");
}

#[test]
fn the_folder_mark_writer_writes_the_line_the_builder_builds() {
    let ordner = Wegwerf::neu("ordnermarke-gleich");
    let o = Ordnermarke {
        zeit_ms: 1_795_179_600_000,
        aktionsname: "ordnermarke.gesenkt".to_owned(),
        marke: "a1b2c3d4e5f60718293a4b5c6d7e8f90".to_owned(),
        klasse_alt: None,
        klasse_neu: Some("intern".to_owned()),
        kunde_alt: Some("k-beispiel".to_owned()),
        kunde_neu: Some("k-beispiel".to_owned()),
    };
    let hash = anhaengen_ordnermarke(ordner.ordner(), "ordnermarke", &o).expect("append");
    let zeile = zeile_ordnermarke(&o, NULL_HASH).expect("line");
    assert_eq!(hash, sha256_hex(zeile.as_bytes()));
    assert_eq!(
        fs::read_to_string(ordner.pfad("ordnermarke-2026-11.jsonl")).expect("file"),
        format!("{zeile}\n")
    );
    let ergebnis = pruefe_quelle(ordner.ordner(), "ordnermarke", NULL_HASH, None).expect("source");
    assert!(ergebnis.ok());
    assert_eq!(ergebnis.zeilen, 1);
}
