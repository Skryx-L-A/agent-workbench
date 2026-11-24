//! The visible cases of contract 9 (`rust/spec/anmelden/faelle/`): every case runs the program against the
//! stand-in of the CLI and compares stdout, stderr and exit byte for byte, then the key file, the throw-away
//! home and what the stand-in saw.
//! The program runs with the seams of the test build (feature `pruefbau`); the delivered build has none (`tests/naht.rs`).
#![cfg(all(unix, feature = "pruefbau"))] // the crate is Unix only (see `src/lib.rs`)

mod gemeinsam;

use std::fs;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::time::Duration;

use gemeinsam::{
    Aufbau, PROXY, VERBOTEN, ausgang, befehl, dateien, eigene_uid, fall_aufbauen, fall_ordner,
    json_lesen, log_lesen, prozess_lebt, signal_senden, warte_auf, warte_auf_ende,
};
use serde_json::Value;

fn namen() -> Vec<String> {
    let mut n: Vec<String> = fs::read_dir(fall_ordner().join("faelle"))
        .expect("case folder")
        .map(|e| {
            e.expect("entry")
                .path()
                .file_stem()
                .expect("stem")
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    n.sort();
    n
}

fn args_von(fall: &Value) -> Vec<String> {
    fall["args"]
        .as_array()
        .expect("args")
        .iter()
        .map(|a| a.as_str().expect("arg").to_owned())
        .collect()
}

fn fall_laufen(name: &str) {
    let fall = json_lesen(&fall_ordner().join("faelle").join(format!("{name}.json")));
    let erwartet = json_lesen(&fall_ordner().join("erwartet").join(format!("{name}.json")));
    let (a, token) = fall_aufbauen(&fall);
    let args = args_von(&fall);
    let aus = if let Some(sig) = fall["signal"].as_str() {
        let mut kind = befehl(&a, &args).spawn().expect("spawn");
        assert!(
            warte_auf(&a.pidfile, Duration::from_secs(10)),
            "{name}: the stand-in did not start"
        );
        let pid: u32 = fs::read_to_string(&a.pidfile)
            .expect("pid")
            .trim()
            .parse()
            .expect("pid number");
        signal_senden(sig, kind.id());
        warte_auf_ende(&mut kind, Duration::from_secs(15));
        let aus = ausgang(kind);
        for _ in 0..150 {
            if !prozess_lebt(pid) {
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(
            !prozess_lebt(pid),
            "{name}: the CLI lives on after the signal"
        );
        aus
    } else {
        gemeinsam::laufen(&a, &args)
    };
    assert_eq!(aus.exit, erwartet["exit"], "{name}: exit");
    assert_eq!(
        aus.stdout,
        erwartet["stdout"].as_str().expect("stdout"),
        "{name}: stdout"
    );
    assert_eq!(
        aus.stderr,
        erwartet["stderr"].as_str().expect("stderr"),
        "{name}: stderr"
    );
    schluesseldatei_pruefen(
        name,
        &a,
        &token,
        erwartet["ziel_datei"].as_str().expect("state"),
    );
    assert_eq!(
        log_lesen(&a).is_some(),
        erwartet["login_gestartet"].as_bool().expect("flag"),
        "{name}: login_gestartet"
    );
    wegwerf_pruefen(name, &a, &erwartet);
    if let Some(log) = log_lesen(&a) {
        umgebung_der_cli_pruefen(name, &a, &log);
    }
    keine_marke(name, &a, &token, &aus.stdout, &aus.stderr, &args);
}

fn schluesseldatei_pruefen(name: &str, a: &Aufbau, token: &str, stand: &str) {
    let datei = a.ziel.join("copilot.schluessel");
    match stand {
        "neu" | "ersetzt" => {
            let meta = fs::metadata(&datei).expect("key file");
            assert!(meta.is_file(), "{name}: regular file");
            assert_eq!(
                meta.permissions().mode() & 0o7777,
                0o600,
                "{name}: mode 0600"
            );
            assert_eq!(meta.uid(), eigene_uid(a), "{name}: owner");
            assert_eq!(
                fs::read_to_string(&datei).expect("read"),
                format!("{token}\n"),
                "{name}: content is token and LF"
            );
            assert_eq!(
                dateien(&a.ziel),
                vec![datei],
                "{name}: no temp file is left"
            );
        }
        "unveraendert" => {
            assert_eq!(
                fs::read_to_string(&datei).expect("read"),
                "alt-inhalt\n",
                "{name}"
            );
            assert_eq!(
                dateien(&a.ziel),
                vec![datei],
                "{name}: no temp file is left"
            );
        }
        "nicht-angelegt" => {
            assert!(!datei.exists(), "{name}: no key file");
            assert!(
                dateien(&a.ziel).is_empty(),
                "{name}: nothing in the target folder"
            );
        }
        andere => panic!("unknown state {andere}"),
    }
}

/// After the run only the planned left-overs lie in the folder (the throw-away home of the run itself is gone), and
/// the folder a link left-over points to is unchanged.
fn wegwerf_pruefen(name: &str, a: &Aufbau, erwartet: &Value) {
    let uebrig = usize::try_from(erwartet["reste_uebrig"].as_u64().expect("count")).expect("fits");
    if a.wegwerf.exists() {
        let rest: Vec<_> = fs::read_dir(&a.wegwerf).expect("read").flatten().collect();
        assert_eq!(
            rest.len(),
            uebrig,
            "{name}: left in the throw-away folder: {rest:?}"
        );
    } else {
        assert_eq!(uebrig, 0, "{name}");
    }
    if a.opfer.exists() {
        assert_eq!(
            fs::read_to_string(a.opfer.join("probe.txt")).expect("probe"),
            "bleibt\n",
            "{name}: the target of the link was touched"
        );
    }
}

fn umgebung_der_cli_pruefen(name: &str, a: &Aufbau, log: &str) {
    let zeile = |kopf: &str| {
        log.lines()
            .find_map(|z| z.strip_prefix(kopf))
            .unwrap_or_else(|| panic!("{name}: no line {kopf}"))
            .to_owned()
    };
    assert_eq!(zeile("argv:"), "login --device-code", "{name}");
    let heim = zeile("home:");
    assert!(
        heim.starts_with(&a.wegwerf.display().to_string()) && heim.ends_with("/heim"),
        "{name}: HOME is in the throw-away folder: {heim}"
    );
    assert_eq!(zeile("chome:"), format!("{heim}/.copilot"), "{name}");
    assert_eq!(zeile("gh:"), "", "{name}: no gh in the PATH of the CLI");
    assert!(
        zeile("path:").contains(&a.bin.display().to_string()),
        "{name}"
    );
    assert!(
        !zeile("path:").contains(&a.gh.display().to_string()),
        "{name}: gh folder is cut"
    );
    assert!(
        zeile("modus:").starts_with("drwx------"),
        "{name}: HOME is 0700"
    );
    assert!(
        log.lines().filter(|z| z.starts_with("drwx------")).count() >= 1,
        "{name}: COPILOT_HOME is 0700"
    );
    assert!(
        zeile("settingsmodus:").starts_with("-rw-------"),
        "{name}: settings are 0600"
    );
    assert_eq!(
        zeile("settings:"),
        "{\"storeTokenPlaintext\":true}",
        "{name}"
    );
    let umgebung = log.split("umgebung:\n").nth(1).expect("environment");
    for verboten in VERBOTEN {
        assert!(
            !umgebung
                .lines()
                .any(|z| z.starts_with(&format!("{verboten}="))),
            "{name}: {verboten} reached the CLI"
        );
    }
    assert!(
        !umgebung
            .lines()
            .any(|z| z.starts_with("UNRELATED_VARIABLE=")),
        "{name}: an unrelated variable reached the CLI"
    );
    assert!(
        umgebung
            .lines()
            .any(|z| z == format!("HTTPS_PROXY={PROXY}")),
        "{name}: the proxy of the caller is kept"
    );
}

/// The token is in no output, no argument, no variable and in no file except the key file and the stand-in script.
fn keine_marke(name: &str, a: &Aufbau, token: &str, stdout: &str, stderr: &str, args: &[String]) {
    assert!(!stdout.contains(token), "{name}: token in stdout");
    assert!(!stderr.contains(token), "{name}: token in stderr");
    assert!(
        !args.iter().any(|x| x.contains(token)),
        "{name}: token in argv"
    );
    if let Some(log) = log_lesen(a) {
        assert!(
            !log.contains(token),
            "{name}: token in argv or environment of the CLI"
        );
    }
    let schluessel = a.ziel.join("copilot.schluessel");
    for datei in dateien(&a.wurzel) {
        if datei == schluessel || datei == a.skript {
            continue;
        }
        let roh = fs::read(&datei).unwrap_or_default();
        let text = String::from_utf8_lossy(&roh);
        assert!(
            !text.contains(token),
            "{name}: token in {}",
            datei.display()
        );
    }
}

#[test]
fn every_visible_case_runs_as_the_contract_says() {
    let namen = namen();
    assert!(namen.len() >= 36, "the case set shrank: {}", namen.len());
    for name in &namen {
        fall_laufen(name);
    }
}

#[test]
fn every_code_of_the_contract_has_a_case() {
    let mut codes: Vec<String> = namen()
        .iter()
        .filter_map(|n| {
            json_lesen(&fall_ordner().join("erwartet").join(format!("{n}.json")))["code"]
                .as_str()
                .map(str::to_owned)
        })
        .collect();
    codes.sort();
    codes.dedup();
    assert_eq!(codes.len(), 11, "{codes:?}");
}
