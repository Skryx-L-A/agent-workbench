//! The visible cases of contract 5 (`rust/spec/protokollzeile/faelle/`): every case of the four files runs here.
//! Cases that name a file run on a throw-away folder; the command cases start the program `kette`.

mod gemeinsam;

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs;
use std::process::Command;

use formate::kette::{
    Fehler, NULL_HASH, abschnitt, anhaengen_gesperrt, befehl, kanonisch, pruefe, sha256_hex,
    zeile_gesperrt, zeit_text,
};
use gemeinsam::{Wegwerf, base64, fall_datei, gesperrt_aus, liste, text};
use serde_json::Value;

/// Compares a result with `erwartet`: a field `fehler` names the code, otherwise `soll` is the value.
#[allow(
    clippy::needless_pass_by_value,
    reason = "the callers hand over the value they just built"
)]
fn pruefen<T: PartialEq + std::fmt::Debug>(
    ist: Result<T, Fehler>,
    erwartet: &Value,
    soll: Option<T>,
    fall: &str,
) {
    match (ist, erwartet["fehler"].as_str()) {
        (Err(fehler), Some(code)) => assert_eq!(fehler.code(), code, "{fall}"),
        (Ok(wert), None) => assert_eq!(Some(wert), soll, "{fall}"),
        (ist, soll_fehler) => panic!("{fall}: got {ist:?}, wanted error {soll_fehler:?}"),
    }
}

#[test]
fn kanonisch_cases() {
    let datei = fall_datei("kanonisch.json");
    let faelle = liste(&datei, "faelle");
    assert_eq!(faelle.len(), 9);
    for fall in faelle {
        let wert: Value = serde_json::from_str(text(fall, "eingabe_text")).expect("input is JSON");
        let name = text(fall, "fall");
        assert_eq!(
            kanonisch(&wert).as_deref(),
            Ok(text(fall, "erwartet")),
            "{name}"
        );
    }
}

#[test]
fn zeile_cases() {
    let datei = fall_datei("zeile.json");
    let faelle = liste(&datei, "zeile");
    assert_eq!(faelle.len(), 18);
    for fall in faelle {
        let name = text(fall, "fall");
        let g = gesperrt_aus(&fall["eingabe"]);
        let ist = zeile_gesperrt(&g, text(fall, "vorher"));
        let erwartet = &fall["erwartet"];
        if erwartet["fehler"].is_null() {
            let zeile = ist.expect(name);
            assert_eq!(zeile, text(erwartet, "zeile"), "{name}");
            assert_eq!(
                sha256_hex(zeile.as_bytes()),
                text(erwartet, "sha256"),
                "{name}"
            );
        } else {
            pruefen(ist, erwartet, None, name);
        }
    }
}

#[test]
fn zeit_text_cases() {
    let datei = fall_datei("zeile.json");
    let faelle = liste(&datei, "zeit_text");
    assert_eq!(faelle.len(), 6);
    for fall in faelle {
        let ms = fall["ms"].as_i64().expect("ms");
        let erwartet = &fall["erwartet"];
        pruefen(
            zeit_text(ms),
            erwartet,
            erwartet["text"].as_str().map(str::to_owned),
            &format!("zeit_text({ms})"),
        );
    }
}

#[test]
fn abschnitt_cases() {
    let datei = fall_datei("zeile.json");
    let faelle = liste(&datei, "abschnitt");
    assert_eq!(faelle.len(), 8);
    for fall in faelle {
        let (quelle, ms) = (text(fall, "quelle"), fall["ms"].as_i64().expect("ms"));
        let erwartet = &fall["erwartet"];
        pruefen(
            abschnitt(quelle, ms),
            erwartet,
            erwartet["name"].as_str().map(str::to_owned),
            &format!("abschnitt({quelle}, {ms})"),
        );
    }
}

#[test]
fn anhaengen_cases() {
    let datei = fall_datei("anhaengen.json");
    let faelle = liste(&datei, "faelle");
    assert_eq!(faelle.len(), 7);
    for (nummer, fall) in faelle.iter().enumerate() {
        let name = text(fall, "fall");
        let ordner = Wegwerf::neu(&format!("anhaengen-{nummer}"));
        let pfad = ordner.pfad(text(fall, "datei"));
        if let Some(anfang) = fall["anfang"].as_str() {
            fs::write(&pfad, anfang).expect("write the start");
        }
        for aufruf in liste(fall, "aufrufe") {
            let g = gesperrt_aus(&aufruf["eingabe"]);
            let erwartet = &aufruf["erwartet"];
            pruefen(
                anhaengen_gesperrt(&pfad, &g),
                erwartet,
                erwartet["sha256"].as_str().map(str::to_owned),
                name,
            );
        }
        let inhalt = fs::read_to_string(&pfad).expect("the file exists");
        assert_eq!(inhalt, text(fall, "ende"), "{name}: content at the end");
        #[cfg(unix)]
        if let Some(rechte) = fall["rechte_neu_unix"].as_str() {
            use std::os::unix::fs::PermissionsExt;
            let modus = fs::metadata(&pfad).expect("metadata").permissions().mode() & 0o777;
            assert_eq!(format!("{modus:04o}"), rechte, "{name}: mode of a new file");
        }
    }
}

#[test]
fn two_writers_keep_one_chain() {
    let datei = fall_datei("anhaengen.json");
    let fall = &datei["zwei_schreiber"];
    let basis = gesperrt_aus(&fall["basis"]);
    let je = fall["je_schreiber"].as_u64().expect("je_schreiber");
    assert_eq!(je, 50);
    let ordner = Wegwerf::neu("zwei-schreiber");
    let pfad = ordner.pfad(text(fall, "datei"));
    std::thread::scope(|bereich| {
        for sitzung in ["s-a", "s-b"] {
            let (pfad, basis) = (&pfad, &basis);
            bereich.spawn(move || {
                for i in 1..=je {
                    let mut g = basis.clone();
                    g.sitzung = sitzung.to_owned();
                    g.anzahl = u32::try_from(i).expect("small");
                    g.zeit_ms += i64::try_from(i).expect("small");
                    anhaengen_gesperrt(pfad, &g).expect("append under the lock");
                }
            });
        }
    });
    let erwartet = &fall["erwartet"];
    let soll_zeilen = erwartet["pruefe"]["zeilen"].as_u64().expect("zeilen");
    assert_eq!(soll_zeilen, 100);

    let pruefung = pruefe(std::slice::from_ref(&pfad), NULL_HASH, None);
    assert_eq!(pruefung.bruch, None);
    assert_eq!(pruefung.zeilen, soll_zeilen);

    let aus = Command::new(env!("CARGO_BIN_EXE_kette"))
        .arg("pruefe")
        .arg(&pfad)
        .output()
        .expect("start kette");
    assert_eq!(aus.status.code(), Some(0));
    let stdout = String::from_utf8(aus.stdout).expect("UTF-8");
    assert!(
        stdout.contains(&format!("\"zeilen\":{soll_zeilen}")),
        "{stdout}"
    );

    // Both writers wrote all their lines: the counts 1 to 50 of each session, in the order of its calls.
    let inhalt = fs::read_to_string(&pfad).expect("file");
    let mut je_sitzung: BTreeMap<String, Vec<u64>> = BTreeMap::new();
    for zeile in inhalt.lines() {
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
    assert_eq!(je_sitzung.len(), 2);
    for (sitzung, anzahlen) in je_sitzung {
        assert_eq!(anzahlen, soll, "{sitzung}");
    }
}

#[test]
fn a_wrong_call_explains_itself_on_stderr_only() {
    let aus = Command::new(env!("CARGO_BIN_EXE_kette"))
        .output()
        .expect("start kette");
    assert_eq!(aus.status.code(), Some(64));
    assert_eq!(aus.stdout, Vec::<u8>::new());
    let meldung = String::from_utf8(aus.stderr).expect("UTF-8");
    assert!(meldung.contains("Aufruf: kette pruefe"), "{meldung}");

    let ordner = Wegwerf::neu("stderr");
    let datei = ordner.schreibe("brain-2026-11.jsonl", b"");
    let aus = Command::new(env!("CARGO_BIN_EXE_kette"))
        .arg("pruefe")
        .arg(&datei)
        .output()
        .expect("start kette");
    assert_eq!(aus.status.code(), Some(0));
    assert_eq!(aus.stderr, Vec::<u8>::new());
}

#[test]
fn base64_helper_decodes() {
    assert_eq!(base64("TWFu"), b"Man");
    assert_eq!(base64("TWE="), b"Ma");
    assert_eq!(base64("TQ=="), b"M");
    assert_eq!(base64(""), b"");
}

#[test]
fn pruefe_cases() {
    let datei = fall_datei("pruefe.json");
    let faelle = liste(&datei, "faelle");
    assert_eq!(faelle.len(), 39);
    for (nummer, fall) in faelle.iter().enumerate() {
        let name = text(fall, "fall");
        let ordner = Wegwerf::neu(&format!("pruefe-{nummer}"));
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
                match a.strip_prefix('@') {
                    Some(datei) => ordner.pfad(datei).into_os_string(),
                    None => OsString::from(a),
                }
            })
            .collect();
        let soll_exit = fall["erwartet"]["exit"].as_i64().expect("exit");
        let soll_stdout = text(&fall["erwartet"], "stdout");

        let aus = Command::new(env!("CARGO_BIN_EXE_kette"))
            .args(&args)
            .output()
            .expect("start kette");
        assert_eq!(
            i64::from(aus.status.code().expect("an exit code")),
            soll_exit,
            "{name}: exit"
        );
        assert_eq!(aus.stdout, soll_stdout.as_bytes(), "{name}: stdout bytes");

        // The same call without a process gives the same answer.
        let ohne = befehl(&args);
        assert_eq!(i64::from(ohne.exit), soll_exit, "{name}: exit (function)");
        assert_eq!(ohne.stdout, soll_stdout, "{name}: stdout (function)");
    }
}
