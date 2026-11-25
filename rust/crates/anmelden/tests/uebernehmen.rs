//! The takeover `werkbank-anmelden copilot --uebernehmen [--neu]` (contract 9, section 3b): the visible cases of
//! `rust/spec/anmelden/faelle-uebernehmen/`, a hard kill at every point between the steps and the restart, signals,
//! the undo after a failed replace, the temp left-overs, the foreign owner (by a substituted user id) and the reader
//! of the spans of `config.json`. No real home, no real `config.json`: everything lies in a throw-away folder and the
//! token is a dummy built from prefix and filler.
//! The program runs with the seams of the test build (feature `pruefbau`); the delivered build has none (`tests/naht.rs`).
#![cfg(all(unix, feature = "pruefbau"))] // the crate is Unix only (see `src/lib.rs`)

mod gemeinsam;

use std::fmt::Write as _;
use std::fs;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::os::unix::process::ExitStatusExt;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::atomic::AtomicUsize;
use std::time::Duration;

use anmelden::fehler::Fehler;
use anmelden::form::{Fund, ohne_token, token_aus_config};
use anmelden::lauf::{TempArt, Umgebung, einfacher_name, temp_reste_raeumen};
use anmelden::uebernehmen::{
    Punkt, config_lesen, quelle_oeffnen, schluessel_ist_token, uebernehmen, ziel_oeffnen,
};
use gemeinsam::{
    Aufbau, attrappe_text, aufbau, baum, befehl, dateien, eigene_uid, fall_ordner, json_lesen,
    skript_schreiben, token_text, tote_pid, ziel_bauen,
};
use serde_json::{Value, json};

const MARKE: &str = "@EINTRAG@";
const KEY_TEMP: &str = ".anmeldung.copilot.";
const CONFIG_TEMP: &str = ".config.json.wb-anmelden-";

/// A `config.json` with further fields: the template (with the mark where the entry stands) and the entry.
fn vorlage(quelle: &str, token: &str) -> (String, String) {
    match quelle {
        "erfolg-letzter" => (
            format!(
                "{{\"a\": 1, \"authTokens\": {{\"h\": {{\"erst\": 3{MARKE}}}}}, \"z\": [], \"text\": \"mit \\\"Anfuehrung\\\" und \\\\ und \\u00e4 und {{\\\"authTokens\\\"}}\" }}\n"
            ),
            format!(", \"attrappe\": \"{token}\""),
        ),
        "erfolg-allein" => (
            format!("{{\"authTokens\":{{\"h\":{{{MARKE}}}}},\"store\":\"plain\"}}"),
            format!("\"k\":\"{token}\""),
        ),
        // The measured form of CLI 1.0.88 (21.11.2026): two whole comment lines, then the JSON with six fields.
        _ => (
            format!(
                "// User settings belong in settings.json.\n// This file is managed automatically.\n{{\n  \"appTipShown\": true,\n  \
                 \"authTokens\": {{\n    \"https://github.com\": {{\n      \"erst\": \"\",\n      {MARKE}\"zahl\": 7\n    }}\n  }},\n  \
                 \"firstLaunchAt\": \"2026-11-21T09:00:00.000Z\",\n  \"lastLoggedInUser\": {{\"host\": \"https://github.com\", \"login\": \
                 \"attrappe\"}},\n  \"loggedInUsers\": [{{\"host\": \"https://github.com\", \"login\": \"attrappe\"}}],\n  \
                 \"reasoningSummariesCleanupDone\": true\n}}\n"
            ),
            format!("\"attrappe\": \"{token}\",\n      "),
        ),
    }
}

/// The bytes of `config.json` before the run and the bytes after a finished takeover.
fn config_vorher_nachher(quelle: &str, token: &str) -> (String, String) {
    let (v, e) = vorlage(quelle, token);
    (v.replace(MARKE, &e), v.replace(MARKE, ""))
}

fn zweite_anmeldung() -> String {
    format!("gho_{}", "Z".repeat(36))
}

/// Sets up the source of a case in `a.echt` (the folder of the CLI). Returns the bytes of `config.json` (if regular).
fn quelle_bauen(a: &Aufbau, quelle: &str, token: &str) -> Option<String> {
    let datei = a.echt.join("config.json");
    let text = match quelle {
        "erfolg" | "erfolg-letzter" | "erfolg-allein" | "verweis" | "hardlink" => {
            Some(config_vorher_nachher(quelle, token).0)
        }
        "ohne-config" | "fifo" | "ordner" => None,
        "kein-json" => Some("{nicht json".to_owned()),
        "ohne-feld" => Some("{\"banner\":\"never\"}".to_owned()),
        "zwei-tokens" => Some(format!(
            "{{\"authTokens\":{{\"h\":{{\"a\":\"{token}\",\"b\":\"{token}x\"}}}}}}"
        )),
        "weitere-anmeldung" => Some(format!(
            "{{\"authTokens\":{{\"h\":{{\"k\":\"{token}\"}}}},\"alt\":\"{}\"}}",
            zweite_anmeldung()
        )),
        "token-ausserhalb" => Some(format!(
            "{{\"authTokens\":{{\"h\":{{\"k\":\"{token}\"}}}},\"loggedInUsers\":[{{\"t\":\"{}\"}}]}}",
            zweite_anmeldung()
        )),
        "token-andere-tiefe" => Some(format!("{{\"authTokens\":{{\"h\":\"{token}\"}}}}")),
        "kommentar-mitte" => Some(format!(
            "// kopf\n{{\n  \"a\": 1,\n// mitte\n  \"authTokens\": {{\"h\": {{\"k\": \"{token}\"}}}}\n}}\n"
        )),
        "blockkommentar" => Some(format!(
            "\x2f* kopf *\x2f\n{{\"authTokens\":{{\"h\":{{\"k\":\"{token}\"}}}}}}\n"
        )),
        "komma-vor-klammer" => Some(format!(
            "// kopf\n{{\"authTokens\":{{\"h\":{{\"k\":\"{token}\",}}}}}}\n"
        )),
        "alte-form" => Some(format!(
            "{{\"copilot_tokens\":{{\"https://github.com:attrappe\":\"{token}\"}}}}"
        )),
        "zu-gross" => {
            let (v, _) = config_vorher_nachher("erfolg-allein", token);
            let fuell = "a".repeat(1 << 20);
            Some(v.replace("\"store\"", &format!("\"fuell\":\"{fuell}\",\"store\"")))
        }
        andere => panic!("unknown source {andere}"),
    };
    match quelle {
        "verweis" => {
            let echt = a.wurzel.join("anderswo.json");
            fs::write(&echt, text.as_deref().expect("text")).expect("file");
            std::os::unix::fs::symlink(&echt, &datei).expect("link");
        }
        "fifo" => {
            let st = Command::new("mkfifo").arg(&datei).status().expect("mkfifo");
            assert!(st.success());
        }
        "ordner" => fs::create_dir(&datei).expect("folder"),
        _ => {
            if let Some(t) = &text {
                fs::write(&datei, t).expect("config");
                fs::set_permissions(&datei, fs::Permissions::from_mode(0o640)).expect("mode");
            }
            if quelle == "hardlink" {
                fs::hard_link(&datei, a.wurzel.join("zweitname.json")).expect("hard link");
            }
        }
    }
    text
}

fn args_von(fall: &Value) -> Vec<String> {
    fall["args"]
        .as_array()
        .expect("args")
        .iter()
        .map(|x| x.as_str().expect("arg").to_owned())
        .collect()
}

/// Every temp file of the command (by name) in the target folder and the folder of the CLI.
fn temp_namen(a: &Aufbau) -> Vec<String> {
    let mut aus = Vec::new();
    for ordner in [&a.ziel, &a.echt] {
        if let Ok(liste) = fs::read_dir(ordner) {
            for e in liste.flatten() {
                let n = e.file_name().to_string_lossy().into_owned();
                if n.starts_with(KEY_TEMP) || n.starts_with(CONFIG_TEMP) {
                    aus.extend([n]);
                }
            }
        }
    }
    aus.sort();
    aus
}

/// The token is in no output and in no file below the set-up except the files in `erlaubt`.
fn token_nirgends(name: &str, a: &Aufbau, token: &str, aus: &Output, erlaubt: &[PathBuf]) {
    assert!(
        !String::from_utf8_lossy(&aus.stdout).contains(token),
        "{name}: token in stdout"
    );
    assert!(
        !String::from_utf8_lossy(&aus.stderr).contains(token),
        "{name}: token in stderr"
    );
    for datei in dateien(&a.wurzel) {
        // Only regular files are read: a FIFO of a case would block the read.
        if erlaubt.contains(&datei)
            || datei == a.skript
            || !fs::metadata(&datei).is_ok_and(|m| m.is_file())
        {
            continue;
        }
        let roh = fs::read(&datei).unwrap_or_default();
        assert!(
            !String::from_utf8_lossy(&roh).contains(token),
            "{name}: token in {}",
            datei.display()
        );
    }
}

fn laufen_mit(a: &Aufbau, args: &[String], probe: Option<&str>) -> Output {
    let mut c = befehl(a, args);
    if let Some(p) = probe {
        c.env("WB_ANMELDEN_PROBE", p);
    }
    c.output().expect("run")
}

fn schluessel(a: &Aufbau) -> PathBuf {
    a.ziel.join("anmeldung.copilot")
}

fn reste_bauen(a: &mut Aufbau, reste: &str) {
    let tot = tote_pid();
    let schluessel_temp = |a: &Aufbau, pid: u32| {
        fs::write(a.ziel.join(format!("{KEY_TEMP}{pid}.tmp")), "rest\n").expect("temp");
    };
    let config_temp = |a: &Aufbau, pid: u32| {
        fs::write(a.echt.join(format!("{CONFIG_TEMP}{pid}.tmp")), "{}").expect("temp");
    };
    match reste {
        "keine" => {}
        "schluessel-temp" => schluessel_temp(a, tot),
        "config-temp" => config_temp(a, tot),
        "beide" => {
            schluessel_temp(a, tot);
            config_temp(a, tot);
        }
        "lebend" => {
            let k = Command::new("sleep").arg("60").spawn().expect("sleep");
            schluessel_temp(a, k.id());
            a.hilfskind = Some(k);
        }
        "verweis" => {
            fs::create_dir_all(&a.opfer).expect("victim");
            fs::write(a.opfer.join("probe.txt"), "bleibt\n").expect("probe");
            std::os::unix::fs::symlink(
                a.opfer.join("probe.txt"),
                a.ziel.join(format!("{KEY_TEMP}{tot}.tmp")),
            )
            .expect("link");
        }
        andere => panic!("unknown left-over {andere}"),
    }
}

#[allow(clippy::too_many_lines)]
fn fall_laufen(name: &str) {
    let ordner = fall_ordner();
    let fall = json_lesen(
        &ordner
            .join("faelle-uebernehmen")
            .join(format!("{name}.json")),
    );
    let erwartet = json_lesen(
        &ordner
            .join("erwartet-uebernehmen")
            .join(format!("{name}.json")),
    );
    let mut a = aufbau();
    let token = token_text(&fall["token"]);
    fs::set_permissions(&a.echt, fs::Permissions::from_mode(0o700)).expect("mode");
    let quelle = fall["quelle"].as_str().expect("source");
    let vorher = quelle_bauen(&a, quelle, &token);
    let vorher_meta = fs::symlink_metadata(a.echt.join("config.json")).ok();
    match fall["bundtool"].as_str().expect("tool") {
        "keins" => {}
        "leer" => skript_schreiben(&a.bin.join("secret-tool"), "#!/bin/sh\nexit 1\n"),
        "gefunden" => skript_schreiben(&a.bin.join("secret-tool"), "#!/bin/sh\nexit 0\n"),
        andere => panic!("unknown tool {andere}"),
    }
    match fall["ziel"].as_str().expect("target") {
        "frei" => {}
        "datei-da" => {
            fs::write(schluessel(&a), "alt-inhalt\n").expect("old");
            fs::set_permissions(schluessel(&a), fs::Permissions::from_mode(0o600)).expect("mode");
        }
        "datei-gleich" => {
            fs::write(schluessel(&a), format!("{token}\n")).expect("same");
            fs::set_permissions(schluessel(&a), fs::Permissions::from_mode(0o600)).expect("mode");
        }
        andere => ziel_bauen(&a, andere),
    }
    reste_bauen(&mut a, fall["reste"].as_str().expect("left-overs"));
    match fall["quellordner"].as_str().expect("folder") {
        "ok" => {}
        "fehlt" => fs::remove_dir_all(&a.echt).expect("remove"),
        "verweis" => {
            let anders = a.wurzel.join("echt/anders");
            fs::rename(&a.echt, &anders).expect("move");
            std::os::unix::fs::symlink(&anders, &a.echt).expect("link");
        }
        "gruppe-schreibbar" => {
            fs::set_permissions(&a.echt, fs::Permissions::from_mode(0o770)).expect("mode");
        }
        andere => panic!("unknown folder {andere}"),
    }
    let args = args_von(&fall);
    let ebenen_vorher = (baum(&a.heim), baum(&a.aussen));
    let aus = laufen_mit(&a, &args, fall["probe"].as_str());
    assert_eq!(
        aus.status.code(),
        erwartet["exit"]
            .as_i64()
            .and_then(|n| i32::try_from(n).ok()),
        "{name}: exit; stderr {}",
        String::from_utf8_lossy(&aus.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&aus.stdout),
        erwartet["stdout"].as_str().expect("stdout"),
        "{name}: stdout"
    );
    assert_eq!(
        String::from_utf8_lossy(&aus.stderr),
        erwartet["stderr"].as_str().expect("stderr"),
        "{name}: stderr"
    );
    if erwartet["code"] == "anmelden.ziel_nicht_schreibbar" {
        assert_eq!(
            (baum(&a.heim), baum(&a.aussen)),
            ebenen_vorher,
            "{name}: no level is new, no mode changed, the target of a link is unchanged"
        );
        assert!(
            !a.wurzel.join("kein-heim").exists(),
            "{name}: no home is made"
        );
    }
    let _ = fs::set_permissions(&a.ziel, fs::Permissions::from_mode(0o700));
    match erwartet["ziel_datei"].as_str().expect("state") {
        "token" => {
            let meta = fs::symlink_metadata(schluessel(&a)).expect("key file");
            assert!(meta.is_file(), "{name}: regular key file");
            assert_eq!(meta.permissions().mode() & 0o7777, 0o600, "{name}: 0600");
            assert_eq!(meta.uid(), eigene_uid(&a), "{name}: owner");
            assert_eq!(meta.nlink(), 1, "{name}: one name");
            assert_eq!(
                fs::read_to_string(schluessel(&a)).expect("read"),
                format!("{token}\n"),
                "{name}: token and LF"
            );
        }
        "alt" => assert_eq!(
            fs::read_to_string(schluessel(&a)).expect("read"),
            "alt-inhalt\n",
            "{name}: the old key is back"
        ),
        "keine" => assert!(
            fs::symlink_metadata(schluessel(&a)).is_err(),
            "{name}: no key file"
        ),
        andere => panic!("unknown state {andere}"),
    }
    let config = a.echt.join("config.json");
    let mut erlaubt = vec![schluessel(&a)];
    match erwartet["config"].as_str().expect("config") {
        "ohne-token" => {
            let (_, nachher) = config_vorher_nachher(quelle, &token);
            assert_eq!(
                fs::read_to_string(&config).expect("config"),
                nachher,
                "{name}: config.json lost exactly the entry"
            );
            let meta = fs::symlink_metadata(&config).expect("meta");
            let alt = vorher_meta.expect("old meta");
            assert_eq!(
                meta.permissions().mode() & 0o7777,
                0o640,
                "{name}: mode kept"
            );
            assert_eq!(
                (meta.uid(), meta.gid()),
                (alt.uid(), alt.gid()),
                "{name}: owner kept"
            );
            assert_eq!(meta.nlink(), 1, "{name}");
        }
        "unveraendert" => {
            if let Some(t) = vorher.as_ref().filter(|_| fall["quellordner"] != "fehlt") {
                if quelle != "verweis" {
                    assert_eq!(
                        fs::read_to_string(&config).expect("config"),
                        *t,
                        "{name}: config.json unchanged"
                    );
                }
                erlaubt.extend([
                    config.clone(),
                    a.wurzel.join("anderswo.json"),
                    a.wurzel.join("zweitname.json"),
                    a.wurzel.join("echt/anders/config.json"),
                ]);
            }
            if let Some(alt) = vorher_meta
                .as_ref()
                .filter(|_| fall["quellordner"] != "fehlt")
            {
                let jetzt = fs::symlink_metadata(&config).expect("still there");
                assert_eq!(jetzt.file_type(), alt.file_type(), "{name}: same kind");
                assert_eq!(jetzt.ino(), alt.ino(), "{name}: same inode");
            }
            if quelle == "verweis" {
                assert_eq!(
                    fs::read_to_string(a.wurzel.join("anderswo.json")).expect("target"),
                    *vorher.as_ref().expect("text"),
                    "{name}: the target of the link is untouched"
                );
            }
        }
        andere => panic!("unknown config {andere}"),
    }
    let uebrig = usize::try_from(erwartet["reste_uebrig"].as_u64().expect("n")).expect("fits");
    assert_eq!(temp_namen(&a).len(), uebrig, "{name}: {:?}", temp_namen(&a));
    if a.opfer.exists() {
        assert_eq!(
            fs::read_to_string(a.opfer.join("probe.txt")).expect("probe"),
            "bleibt\n",
            "{name}: the target of a link left-over is untouched"
        );
    }
    token_nirgends(name, &a, &token, &aus, &erlaubt);
}

fn namen() -> Vec<String> {
    let mut n: Vec<String> = fs::read_dir(fall_ordner().join("faelle-uebernehmen"))
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

#[test]
fn every_visible_takeover_case_runs_as_the_contract_says() {
    let namen = namen();
    assert!(namen.len() >= 49, "the case set shrank: {}", namen.len());
    for name in &namen {
        fall_laufen(name);
    }
}

#[test]
fn the_new_codes_have_their_exit_and_sentence() {
    for (f, code, exit, satz) in [
        (
            Fehler::QuelleOhneToken,
            "anmelden.quelle_ohne_token",
            19,
            "In der Konfiguration der Copilot CLI liegt nicht genau ein Token.",
        ),
        (
            Fehler::QuelleUnsicher,
            "anmelden.quelle_unsicher",
            20,
            "Die Konfiguration der Copilot CLI ist keine eigene regulaere Datei bis 1 MiB.",
        ),
        (
            Fehler::QuelleNichtSchreibbar,
            "anmelden.quelle_nicht_schreibbar",
            21,
            "Die Konfiguration der Copilot CLI ist nicht neu schreibbar.",
        ),
    ] {
        assert_eq!((f.code(), f.exit(), f.satz()), (code, exit, satz));
    }
}

// ---- hard kill between the steps, then the restart ----

/// A set-up for the restart: `config.json` with the token, and with `alt` an old key file.
fn wiederanlauf_aufbau(alt: bool) -> (Aufbau, String) {
    let a = aufbau();
    fs::set_permissions(&a.echt, fs::Permissions::from_mode(0o700)).expect("mode");
    let token = token_text(&json!({"praefix": "gho_", "fuell": "K", "laenge": 40}));
    quelle_bauen(&a, "erfolg", &token);
    skript_schreiben(&a.bin.join("secret-tool"), "#!/bin/sh\nexit 1\n");
    if alt {
        fs::write(schluessel(&a), "alt-inhalt\n").expect("old");
        fs::set_permissions(schluessel(&a), fs::Permissions::from_mode(0o600)).expect("mode");
    }
    (a, token)
}

fn ziel_erreicht(name: &str, a: &Aufbau, token: &str) {
    assert_eq!(
        fs::read_to_string(schluessel(a)).expect("key"),
        format!("{token}\n"),
        "{name}: key"
    );
    assert_eq!(
        fs::read_to_string(a.echt.join("config.json")).expect("config"),
        config_vorher_nachher("erfolg", token).1,
        "{name}: config.json without the token"
    );
    assert!(temp_namen(a).is_empty(), "{name}: {:?}", temp_namen(a));
}

/// Kill at `punkt` (with `--neu` over an old key or without), check that the token is never lost, then run again
/// and reach the goal. `zweit` is the exit of the second run and the notice count it prints.
fn toeten_und_weiter(punkt: &str, neu: bool, zweit_exit: i32, reste: usize, wieder: bool) {
    let name = format!("{punkt} neu={neu}");
    let (a, token) = wiederanlauf_aufbau(neu);
    let mut args = vec!["copilot".to_owned(), "--uebernehmen".to_owned()];
    if neu {
        args.extend(["--neu".to_owned()]);
    }
    let erst = laufen_mit(&a, &args, Some(&format!("toeten:{punkt}")));
    assert_eq!(erst.status.signal(), Some(9), "{name}: killed hard");
    assert!(
        erst.stdout.is_empty() && erst.stderr.is_empty(),
        "{name}: no output"
    );
    let config = fs::read_to_string(a.echt.join("config.json")).expect("config");
    let im_schluessel = fs::read_to_string(schluessel(&a)).is_ok_and(|t| t == format!("{token}\n"));
    assert!(
        config.contains(&token) || im_schluessel,
        "{name}: the token is never lost"
    );
    let mut erlaubt = vec![schluessel(&a), a.echt.join("config.json")];
    erlaubt.extend(temp_namen(&a).iter().map(|n| a.ziel.join(n)));
    token_nirgends(&name, &a, &token, &erst, &erlaubt);
    let zweit = laufen_mit(&a, &args, None);
    let mut stderr = String::new();
    if reste > 0 {
        let _ = writeln!(
            stderr,
            "Hinweis: Reste frueherer Anmeldungen wurden geloescht: {reste}"
        );
    }
    if zweit_exit == 0 {
        if wieder {
            stderr += "Hinweis: Eine unterbrochene Uebernahme wurde abgeschlossen.\n";
        }
        assert_eq!(
            String::from_utf8_lossy(&zweit.stdout),
            "Anmeldung gespeichert.\n",
            "{name}"
        );
    } else {
        stderr += &Fehler::QuelleOhneToken.zeile();
    }
    assert_eq!(zweit.status.code(), Some(zweit_exit), "{name}: second exit");
    assert_eq!(
        String::from_utf8_lossy(&zweit.stderr),
        stderr,
        "{name}: second stderr"
    );
    ziel_erreicht(&name, &a, &token);
    token_nirgends(&name, &a, &token, &zweit, &[schluessel(&a)]);
}

#[test]
fn a_hard_kill_at_every_point_without_neu_is_finished_by_the_next_call() {
    toeten_und_weiter("vor-schreiben", false, 0, 0, false);
    toeten_und_weiter("config-temp", false, 0, 1, false);
    toeten_und_weiter("schluessel-temp", false, 0, 2, false);
    toeten_und_weiter("schluessel", false, 0, 2, true);
    toeten_und_weiter("temp-geloescht", false, 0, 1, true);
    toeten_und_weiter("config", false, 19, 0, false);
}

#[test]
fn a_hard_kill_at_every_point_with_neu_over_an_old_key_is_finished_by_the_next_call() {
    toeten_und_weiter("vor-schreiben", true, 0, 0, false);
    toeten_und_weiter("config-temp", true, 0, 1, false);
    toeten_und_weiter("schluessel-temp", true, 0, 2, false);
    toeten_und_weiter("schluessel", true, 0, 2, true);
    toeten_und_weiter("config", true, 19, 1, false);
}

#[test]
fn the_old_key_waits_in_the_temp_name_after_the_exchange_and_goes_with_the_next_call() {
    let (a, token) = wiederanlauf_aufbau(true);
    let args = ["copilot", "--uebernehmen", "--neu"].map(str::to_owned);
    let erst = laufen_mit(&a, &args, Some("toeten:schluessel"));
    assert_eq!(erst.status.signal(), Some(9));
    let temp: Vec<String> = temp_namen(&a)
        .into_iter()
        .filter(|n| n.starts_with(KEY_TEMP))
        .collect();
    assert_eq!(temp.len(), 1, "{temp:?}");
    assert_eq!(
        fs::read_to_string(a.ziel.join(&temp[0])).expect("old"),
        "alt-inhalt\n",
        "the exchange put the old key at the temp name"
    );
    assert_eq!(
        fs::read_to_string(schluessel(&a)).expect("key"),
        format!("{token}\n")
    );
    let zweit = laufen_mit(&a, &args[..2], None);
    assert_eq!(
        zweit.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&zweit.stderr)
    );
    ziel_erreicht("exchange", &a, &token);
}

// ---- signals ----

fn signal_bei(punkt: &str, signal: &str) -> (Aufbau, String, Output) {
    let (a, token) = wiederanlauf_aufbau(false);
    let marke = a.wurzel.join("marke");
    let mut c = befehl(&a, &["copilot".to_owned(), "--uebernehmen".to_owned()]);
    c.env("WB_ANMELDEN_PROBE", format!("warten:{punkt}"))
        .env("WB_ANMELDEN_PROBE_MARKE", &marke);
    let mut kind = c.spawn().expect("spawn");
    assert!(
        gemeinsam::warte_auf(&marke, Duration::from_secs(10)),
        "{punkt}: the probe was reached"
    );
    gemeinsam::signal_senden(signal, kind.id());
    gemeinsam::warte_auf_ende(&mut kind, Duration::from_secs(15));
    let aus = gemeinsam::ausgabe_beendet(kind);
    (a, token, aus)
}

#[test]
fn a_signal_before_the_first_write_changes_nothing() {
    for (signal, exit) in [("INT", 130), ("TERM", 143), ("HUP", 129)] {
        let (a, token, aus) = signal_bei("vor-schreiben", signal);
        assert_eq!(aus.status.code(), Some(exit), "{signal}");
        assert_eq!(
            String::from_utf8_lossy(&aus.stderr),
            "werkbank-anmelden: anmelden.signal: Abgebrochen durch ein Signal.\n"
        );
        assert!(
            fs::symlink_metadata(schluessel(&a)).is_err(),
            "{signal}: no key"
        );
        assert_eq!(
            fs::read_to_string(a.echt.join("config.json")).expect("config"),
            config_vorher_nachher("erfolg", &token).0,
            "{signal}: config.json unchanged"
        );
        assert!(temp_namen(&a).is_empty(), "{signal}");
        token_nirgends(signal, &a, &token, &aus, &[a.echt.join("config.json")]);
    }
}

#[test]
fn a_signal_after_the_first_write_lets_the_steps_finish() {
    for punkt in ["config-temp", "schluessel"] {
        let (a, token, aus) = signal_bei(punkt, "TERM");
        assert_eq!(
            aus.status.code(),
            Some(0),
            "{punkt}: {}",
            String::from_utf8_lossy(&aus.stderr)
        );
        ziel_erreicht(punkt, &a, &token);
    }
}

// ---- undo, races, left-overs ----

#[test]
fn a_failed_replace_of_config_json_takes_the_new_key_back_and_leaves_the_temp_files_gone() {
    let (a, token) = wiederanlauf_aufbau(false);
    let aus = laufen_mit(
        &a,
        &["copilot".to_owned(), "--uebernehmen".to_owned()],
        Some("config-stoeren"),
    );
    assert_eq!(aus.status.code(), Some(21));
    assert!(
        fs::symlink_metadata(schluessel(&a)).is_err(),
        "the new key is gone"
    );
    assert_eq!(temp_namen(&a), Vec::<String>::new());
    assert_eq!(
        fs::read_to_string(a.echt.join("config.json")).expect("config"),
        config_vorher_nachher("erfolg", &token).0
    );
}

#[test]
fn a_restart_leaves_a_key_file_with_another_token_alone_without_neu() {
    let (a, token) = wiederanlauf_aufbau(false);
    let anders = token_text(&json!({"praefix": "gho_", "fuell": "L", "laenge": 40}));
    fs::write(schluessel(&a), format!("{anders}\n")).expect("other");
    fs::set_permissions(schluessel(&a), fs::Permissions::from_mode(0o600)).expect("mode");
    let aus = laufen_mit(
        &a,
        &["copilot".to_owned(), "--uebernehmen".to_owned()],
        None,
    );
    assert_eq!(aus.status.code(), Some(15));
    assert_eq!(
        fs::read_to_string(schluessel(&a)).expect("key"),
        format!("{anders}\n")
    );
    assert!(
        fs::read_to_string(a.echt.join("config.json"))
            .expect("c")
            .contains(&token)
    );
}

#[test]
fn a_key_file_with_the_token_but_another_mode_or_a_second_name_is_no_restart() {
    for art in ["modus", "zweitname", "verweis"] {
        let (a, token) = wiederanlauf_aufbau(false);
        let inhalt = format!("{token}\n");
        match art {
            "modus" => {
                fs::write(schluessel(&a), &inhalt).expect("key");
                fs::set_permissions(schluessel(&a), fs::Permissions::from_mode(0o644)).expect("m");
            }
            "zweitname" => {
                fs::write(schluessel(&a), &inhalt).expect("key");
                fs::set_permissions(schluessel(&a), fs::Permissions::from_mode(0o600)).expect("m");
                fs::hard_link(schluessel(&a), a.wurzel.join("zweit")).expect("link");
            }
            _ => {
                let woanders = a.wurzel.join("woanders");
                fs::write(&woanders, &inhalt).expect("key");
                fs::set_permissions(&woanders, fs::Permissions::from_mode(0o600)).expect("m");
                std::os::unix::fs::symlink(&woanders, schluessel(&a)).expect("link");
            }
        }
        let aus = laufen_mit(
            &a,
            &["copilot".to_owned(), "--uebernehmen".to_owned()],
            None,
        );
        assert_eq!(aus.status.code(), Some(15), "{art}: datei_da, no restart");
        assert!(
            fs::read_to_string(a.echt.join("config.json"))
                .expect("c")
                .contains(&token)
        );
    }
}

#[test]
fn the_device_flow_also_deletes_a_dead_temp_file_of_the_key_before_the_login() {
    let a = aufbau();
    let token = token_text(&json!({"praefix": "gho_", "fuell": "M", "laenge": 40}));
    skript_schreiben(&a.skript, &attrappe_text(&a, "erfolg", &token));
    let rest = a.ziel.join(format!("{KEY_TEMP}{}.tmp", tote_pid()));
    fs::write(&rest, "rest\n").expect("temp");
    let aus = laufen_mit(&a, &["copilot".to_owned()], None);
    assert_eq!(
        aus.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&aus.stderr)
    );
    assert!(
        String::from_utf8_lossy(&aus.stderr)
            .starts_with("Hinweis: Reste frueherer Anmeldungen wurden geloescht: 1\n")
    );
    assert!(!rest.exists());
}

#[test]
fn the_device_flow_leaves_temp_files_of_config_json_alone() {
    let a = aufbau();
    let token = token_text(&json!({"praefix": "gho_", "fuell": "N", "laenge": 40}));
    skript_schreiben(&a.skript, &attrappe_text(&a, "erfolg", &token));
    let rest = a.echt.join(format!("{CONFIG_TEMP}{}.tmp", tote_pid()));
    fs::write(&rest, "{}").expect("temp");
    let aus = laufen_mit(&a, &["copilot".to_owned()], None);
    assert_eq!(
        aus.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&aus.stderr)
    );
    assert!(
        rest.exists(),
        "the real home stays untouched by the device flow"
    );
}

#[test]
fn a_temp_left_over_that_cannot_be_deleted_stops_the_takeover() {
    let (a, token) = wiederanlauf_aufbau(false);
    fs::write(
        a.echt.join(format!("{CONFIG_TEMP}{}.tmp", tote_pid())),
        "{}",
    )
    .expect("temp");
    fs::set_permissions(&a.echt, fs::Permissions::from_mode(0o500)).expect("mode");
    let aus = laufen_mit(
        &a,
        &["copilot".to_owned(), "--uebernehmen".to_owned()],
        None,
    );
    fs::set_permissions(&a.echt, fs::Permissions::from_mode(0o700)).expect("mode");
    assert_eq!(aus.status.code(), Some(18));
    assert_eq!(
        String::from_utf8_lossy(&aus.stderr),
        Fehler::RestNichtLoeschbar.zeile()
    );
    assert!(fs::symlink_metadata(schluessel(&a)).is_err());
    assert!(
        fs::read_to_string(a.echt.join("config.json"))
            .expect("c")
            .contains(&token)
    );
}

#[test]
fn a_config_json_that_is_not_writable_in_its_folder_changes_nothing() {
    let (a, token) = wiederanlauf_aufbau(false);
    fs::set_permissions(&a.echt, fs::Permissions::from_mode(0o500)).expect("mode");
    let aus = laufen_mit(
        &a,
        &["copilot".to_owned(), "--uebernehmen".to_owned()],
        None,
    );
    fs::set_permissions(&a.echt, fs::Permissions::from_mode(0o700)).expect("mode");
    assert_eq!(aus.status.code(), Some(21));
    assert!(fs::symlink_metadata(schluessel(&a)).is_err());
    assert_eq!(temp_namen(&a), Vec::<String>::new());
    assert!(
        fs::read_to_string(a.echt.join("config.json"))
            .expect("c")
            .contains(&token)
    );
}

// ---- the library: foreign owner by a substituted user id, the temp names, the reader of the spans ----

fn fremde_uid(a: &Aufbau) -> u32 {
    eigene_uid(a) + 1
}

#[test]
fn a_folder_a_config_json_or_a_target_of_another_owner_is_refused() {
    let (a, _token) = wiederanlauf_aufbau(false);
    let eigen = eigene_uid(&a);
    assert!(quelle_oeffnen(&a.echt, eigen).is_ok());
    assert_eq!(
        quelle_oeffnen(&a.echt, fremde_uid(&a)).err(),
        Some(Fehler::QuelleUnsicher)
    );
    let quelle = quelle_oeffnen(&a.echt, eigen).expect("own folder");
    assert!(config_lesen(&quelle, eigen).is_ok());
    assert_eq!(
        config_lesen(&quelle, fremde_uid(&a)).err(),
        Some(Fehler::QuelleUnsicher)
    );
    assert!(ziel_oeffnen(&a.heim, eigen).is_ok());
    assert_eq!(
        ziel_oeffnen(&a.heim, fremde_uid(&a)).err(),
        Some(Fehler::ZielNichtSchreibbar)
    );
    let mut umg = umgebung(&a);
    umg.uid = fremde_uid(&a);
    assert_eq!(
        uebernehmen(&umg, false, &AtomicUsize::new(0), None).err(),
        Some(Fehler::QuelleUnsicher)
    );
}

#[test]
fn a_key_file_of_another_owner_is_no_restart() {
    let (a, inhalt) = wiederanlauf_aufbau(false);
    fs::write(schluessel(&a), format!("{inhalt}\n")).expect("key");
    fs::set_permissions(schluessel(&a), fs::Permissions::from_mode(0o600)).expect("mode");
    let text = fs::read(a.echt.join("config.json")).expect("config");
    let Fund::Eins(token) = token_aus_config(&text) else {
        panic!("one token");
    };
    let ziel = ziel_oeffnen(&a.heim, eigene_uid(&a)).expect("target");
    let st = rustix::fs::statat(
        &ziel,
        "anmeldung.copilot",
        rustix::fs::AtFlags::SYMLINK_NOFOLLOW,
    )
    .expect("stat");
    assert!(schluessel_ist_token(&ziel, &st, &token, eigene_uid(&a)));
    assert!(!schluessel_ist_token(&ziel, &st, &token, fremde_uid(&a)));
}

fn umgebung(a: &Aufbau) -> Umgebung {
    Umgebung {
        heim: a.heim.clone(),
        suchpfad: a.bin.clone().into_os_string(),
        echtes_copilot_heim: a.echt.clone(),
        wegwerf_wurzel: a.wegwerf.clone(),
        uid: eigene_uid(a),
        durchreichen: Vec::new(),
    }
}

#[test]
fn the_takeover_through_the_library_reports_a_restart() {
    let (a, token) = wiederanlauf_aufbau(false);
    let umg = umgebung(&a);
    let erst = uebernehmen(&umg, false, &AtomicUsize::new(0), None).expect("first");
    assert!(!erst.wiederanlauf && erst.erfolg.bund_geprueft);
    ziel_erreicht("library", &a, &token);
    fs::write(
        a.echt.join("config.json"),
        config_vorher_nachher("erfolg", &token).0,
    )
    .expect("again");
    let zweit = uebernehmen(&umg, false, &AtomicUsize::new(0), None).expect("second");
    assert!(zweit.wiederanlauf, "the key file already held the token");
    ziel_erreicht("library", &a, &token);
    let signal = AtomicUsize::new(15);
    fs::write(
        a.echt.join("config.json"),
        config_vorher_nachher("erfolg", &token).0,
    )
    .expect("again");
    assert_eq!(
        uebernehmen(&umg, false, &signal, None).err(),
        Some(Fehler::Signal(15))
    );
}

#[test]
fn the_temp_names_have_exactly_the_form_with_a_pid() {
    assert_eq!(TempArt::Schluessel.name(42), ".anmeldung.copilot.42.tmp");
    assert_eq!(TempArt::Config.name(42), ".config.json.wb-anmelden-42.tmp");
    for art in [TempArt::Schluessel, TempArt::Config] {
        assert_eq!(
            art.pid(&art.name(7)).map(rustix::process::Pid::as_raw_pid),
            Some(7)
        );
        let voll = art.name(7);
        for falsch in [
            voll.replace('7', ""),
            voll.replace('7', "0"),
            voll.replace('7', "7a"),
            voll.replace('7', "-7"),
            voll.replace(".tmp", ".tmpx"),
            format!("x{voll}"),
            voll.replace('7', "99999999999"),
        ] {
            assert!(art.pid(&falsch).is_none(), "{falsch}");
        }
    }
    assert!(TempArt::Config.pid(&TempArt::Schluessel.name(7)).is_none());
    assert!(TempArt::Schluessel.pid(&TempArt::Config.name(7)).is_none());
}

#[test]
fn a_name_for_a_call_relative_to_a_folder_is_one_part() {
    assert!(einfacher_name("config.json"));
    for falsch in ["", ".", "..", "a/b", "/x", "x/"] {
        assert!(!einfacher_name(falsch), "{falsch:?}");
    }
}

#[test]
fn temp_left_overs_are_deleted_only_when_dead_regular_and_own() {
    let a = aufbau();
    let tot = tote_pid();
    let uid = eigene_uid(&a);
    let eigen = std::process::id();
    let datei = |name: String| {
        fs::write(a.ziel.join(&name), "x").expect("file");
        a.ziel.join(name)
    };
    let tote = datei(TempArt::Schluessel.name(tot));
    let mut lebend = Command::new("sleep").arg("60").spawn().expect("sleep");
    let lebende = datei(TempArt::Schluessel.name(lebend.id()));
    let eigene = datei(TempArt::Schluessel.name(eigen));
    let andere_art = datei(TempArt::Config.name(tot));
    let ordner = a.ziel.join(TempArt::Schluessel.name(2_147_480_001));
    fs::create_dir(&ordner).expect("folder");
    fs::write(a.wurzel.join("ziel.txt"), "bleibt").expect("target");
    let verweis = a.ziel.join(TempArt::Schluessel.name(2_147_480_002));
    std::os::unix::fs::symlink(a.wurzel.join("ziel.txt"), &verweis).expect("link");
    assert_eq!(
        temp_reste_raeumen(&a.ziel, TempArt::Schluessel, fremde_uid(&a), eigen),
        Ok(0),
        "files of another owner stay"
    );
    assert_eq!(
        temp_reste_raeumen(&a.ziel, TempArt::Schluessel, uid, eigen),
        Ok(2)
    );
    assert!(
        !tote.exists() && !eigene.exists(),
        "dead and own pid are gone"
    );
    assert!(lebende.exists() && andere_art.exists() && ordner.is_dir());
    assert!(fs::symlink_metadata(&verweis).is_ok());
    assert_eq!(
        fs::read_to_string(a.wurzel.join("ziel.txt")).expect("t"),
        "bleibt"
    );
    assert_eq!(
        temp_reste_raeumen(&a.wurzel.join("fehlt"), TempArt::Schluessel, uid, eigen),
        Ok(0)
    );
    std::os::unix::fs::symlink(&a.ziel, a.wurzel.join("ort-verweis")).expect("link");
    assert_eq!(
        temp_reste_raeumen(&a.wurzel.join("ort-verweis"), TempArt::Config, uid, eigen),
        Ok(0),
        "a folder that is a link is not opened"
    );
    assert!(andere_art.exists());
    fs::set_permissions(&a.ziel, fs::Permissions::from_mode(0o500)).expect("mode");
    assert_eq!(
        temp_reste_raeumen(&a.ziel, TempArt::Config, uid, eigen),
        Err(Fehler::RestNichtLoeschbar)
    );
    fs::set_permissions(&a.ziel, fs::Permissions::from_mode(0o700)).expect("mode");
    let _ = lebend.kill();
    let _ = lebend.wait();
}

fn token_von(text: &str) -> anmelden::form::Token {
    let Fund::Eins(t) = token_aus_config(text.as_bytes()) else {
        panic!("one token in {text}");
    };
    t
}

#[test]
fn the_entry_is_cut_out_byte_exact_and_everything_else_stays() {
    let t = format!("gho_{}", "R".repeat(36));
    for (vorher, nachher) in [
        (
            format!("{{\"authTokens\":{{\"h\":{{\"k\":\"{t}\"}}}}}}"),
            "{\"authTokens\":{\"h\":{}}}".to_owned(),
        ),
        (
            format!("{{\"authTokens\" : {{ \"h\" : {{ \"k\" : \"{t}\" , \"z\" : 1 }} }} }}"),
            "{\"authTokens\" : { \"h\" : { \"z\" : 1 } } }".to_owned(),
        ),
        (
            format!(
                "{{\"authTokens\":{{\"h\":{{\"a\":[1,{{\"b\":\"]\"}}],\"k\":\"{t}\"}}}},\"x\":\"\\\"\"}}"
            ),
            "{\"authTokens\":{\"h\":{\"a\":[1,{\"b\":\"]\"}]}},\"x\":\"\\\"\"}".to_owned(),
        ),
        (
            format!(
                "\n{{\"pre\":{{\"authTokens\":\"nein\"}},\"authTokens\":{{\"h\":{{\"a\":false,\"k\":\"{t}\",\"c\":null}}}}}}\n"
            ),
            "\n{\"pre\":{\"authTokens\":\"nein\"},\"authTokens\":{\"h\":{\"a\":false,\"c\":null}}}\n"
                .to_owned(),
        ),
        (
            format!(
                "{{\"authTokens\":{{\"h\":{{\"k\":\"{}\"}}}}}}",
                t.replacen('g', "\\u0067", 1)
            ),
            "{\"authTokens\":{\"h\":{}}}".to_owned(),
        ),
        // the comment lines stay word for word, the leaf of the second parent object is the one that goes,
        // a parent that becomes empty stays, a parent that is no object is passed over
        (
            format!(
                "// eins\n// zwei\n{{\n  \"authTokens\": {{\n    \"p\": 5,\n    \"q\": {{\"k\": \"{t}\"}},\n    \"r\": {{\"m\": 7}}\n  }}\n}}\n"
            ),
            "// eins\n// zwei\n{\n  \"authTokens\": {\n    \"p\": 5,\n    \"q\": {},\n    \"r\": {\"m\": 7}\n  }\n}\n"
                .to_owned(),
        ),
    ] {
        let token = token_von(&vorher);
        let geschrieben =
            ohne_token(vorher.as_bytes(), &token).map(|v| String::from_utf8(v).expect("utf-8"));
        assert_eq!(geschrieben, Some(nachher.clone()), "{vorher}");
        // the same reading takes the written file as valid and without a token
        assert!(
            matches!(token_aus_config(nachher.as_bytes()), Fund::Keins),
            "{nachher}"
        );
    }
}

#[test]
fn the_cut_refuses_what_it_cannot_make_clean() {
    let t = format!("gho_{}", "S".repeat(36));
    for text in [
        // the field twice at the top (serde takes the last one)
        format!(
            "{{\"authTokens\":{{\"h\":{{\"a\":\"x\"}}}},\"authTokens\":{{\"h\":{{\"k\":\"{t}\"}}}}}}"
        ),
        // another token-like text remains
        format!(
            "{{\"authTokens\":{{\"h\":{{\"k\":\"{t}\"}}}},\"z\":\"{}\"}}",
            zweite_anmeldung()
        ),
        // the bytes of the token remain elsewhere
        format!("{{\"authTokens\":{{\"h\":{{\"k\":\"{t}\"}}}},\"n\":{{\"{t}\":1}}}}"),
        // the bytes of the token remain in a comment line
        format!("// {t}\n{{\"authTokens\":{{\"h\":{{\"k\":\"{t}\"}}}}}}"),
    ] {
        // the token of the clean form (a second token-like text makes the reading refuse it already)
        let token = token_von(&format!("{{\"authTokens\":{{\"h\":{{\"k\":\"{t}\"}}}}}}"));
        assert!(ohne_token(text.as_bytes(), &token).is_none(), "{text}");
    }
}

#[test]
fn a_takeover_with_a_found_keychain_entry_stores_the_key_and_cleans_config_json() {
    let (a, token) = wiederanlauf_aufbau(false);
    skript_schreiben(&a.bin.join("secret-tool"), "#!/bin/sh\nexit 0\n");
    let aus = laufen_mit(
        &a,
        &["copilot".to_owned(), "--uebernehmen".to_owned()],
        None,
    );
    assert_eq!(aus.status.code(), Some(16));
    ziel_erreicht("bund", &a, &token);
}

#[test]
fn every_point_name_is_known() {
    for (name, p) in [
        ("vor-schreiben", Punkt::VorSchreiben),
        ("config-temp", Punkt::ConfigTemp),
        ("schluessel-temp", Punkt::SchluesselTemp),
        ("schluessel", Punkt::Schluessel),
        ("temp-geloescht", Punkt::TempGeloescht),
        ("config", Punkt::Config),
    ] {
        assert_eq!(Punkt::aus_name(name), Some(p));
    }
    assert_eq!(Punkt::aus_name("anders"), None);
}

// ---- changes by others while the run waits at a point ----

/// Starts the takeover with `warten:<punkt>`, waits for the mark, runs `dazwischen`, sends SIGTERM (after the first
/// write the steps go on) and returns the output.
fn mit_eingriff(a: &Aufbau, args: &[&str], punkt: &str, dazwischen: impl FnOnce()) -> Output {
    let marke = a.wurzel.join("marke");
    let args: Vec<String> = args.iter().map(|s| (*s).to_owned()).collect();
    let mut c = befehl(a, &args);
    c.env("WB_ANMELDEN_PROBE", format!("warten:{punkt}"))
        .env("WB_ANMELDEN_PROBE_MARKE", &marke);
    let mut kind = c.spawn().expect("spawn");
    assert!(
        gemeinsam::warte_auf(&marke, Duration::from_secs(10)),
        "{punkt}: the probe was reached"
    );
    dazwischen();
    gemeinsam::signal_senden("TERM", kind.id());
    gemeinsam::warte_auf_ende(&mut kind, Duration::from_secs(15));
    gemeinsam::ausgabe_beendet(kind)
}

/// Writes `config.json` anew in place with the same size; only the nanoseconds of the time of the last change
/// differ from before (same device, inode, size and second).
fn config_still_aendern(a: &Aufbau) -> String {
    let datei = a.echt.join("config.json");
    let alt = fs::metadata(&datei).expect("meta");
    let text = fs::read_to_string(&datei)
        .expect("config")
        .replacen("never", "nevex", 1);
    fs::write(&datei, &text).expect("write in place");
    let zeit = rustix::fs::Timespec {
        tv_sec: alt.mtime(),
        tv_nsec: (alt.mtime_nsec() + 1) % 1_000_000_000,
    };
    rustix::fs::utimensat(
        rustix::fs::CWD,
        &datei,
        &rustix::fs::Timestamps {
            last_access: zeit,
            last_modification: zeit,
        },
        rustix::fs::AtFlags::empty(),
    )
    .expect("time");
    let neu = fs::metadata(&datei).expect("meta");
    assert_eq!(
        (neu.ino(), neu.len(), neu.mtime()),
        (alt.ino(), alt.len(), alt.mtime())
    );
    text
}

#[test]
fn a_config_json_changed_after_the_key_takes_the_key_back() {
    let (a, token) = wiederanlauf_aufbau(false);
    let mut geaendert = String::new();
    let aus = mit_eingriff(&a, &["copilot", "--uebernehmen"], "schluessel", || {
        geaendert = config_still_aendern(&a);
    });
    assert_eq!(
        aus.status.code(),
        Some(21),
        "{}",
        String::from_utf8_lossy(&aus.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&aus.stderr),
        Fehler::QuelleNichtSchreibbar.zeile()
    );
    assert!(
        fs::symlink_metadata(schluessel(&a)).is_err(),
        "the new key is taken back"
    );
    assert_eq!(
        fs::read_to_string(a.echt.join("config.json")).expect("config"),
        geaendert,
        "the change of the other writer stays"
    );
    assert!(geaendert.contains(&token));
    assert_eq!(temp_namen(&a), Vec::<String>::new());
}

#[test]
fn the_undo_never_deletes_a_key_file_that_another_put_in_its_place() {
    let (a, _token) = wiederanlauf_aufbau(false);
    let aus = mit_eingriff(&a, &["copilot", "--uebernehmen"], "temp-geloescht", || {
        let neu = a.ziel.join("fremd.tmp");
        fs::write(&neu, "fremd\n").expect("other key");
        fs::rename(&neu, schluessel(&a)).expect("replace");
        config_still_aendern(&a);
    });
    assert_eq!(aus.status.code(), Some(21));
    assert_eq!(
        fs::read_to_string(schluessel(&a)).expect("key"),
        "fremd\n",
        "the key of the other stays"
    );
}

#[test]
fn a_key_file_that_appears_during_the_run_is_never_replaced_without_neu() {
    let (a, token) = wiederanlauf_aufbau(false);
    let aus = mit_eingriff(&a, &["copilot", "--uebernehmen"], "schluessel-temp", || {
        fs::write(schluessel(&a), "fremd\n").expect("other key");
    });
    assert_eq!(
        aus.status.code(),
        Some(15),
        "{}",
        String::from_utf8_lossy(&aus.stderr)
    );
    assert_eq!(fs::read_to_string(schluessel(&a)).expect("key"), "fremd\n");
    assert_eq!(
        fs::read_to_string(a.echt.join("config.json")).expect("config"),
        config_vorher_nachher("erfolg", &token).0
    );
    assert_eq!(temp_namen(&a), Vec::<String>::new());
}

#[test]
fn the_wait_of_the_probe_ends_at_once_with_the_signal() {
    let (a, _token) = wiederanlauf_aufbau(false);
    let start = std::time::Instant::now();
    // A wait that does not end with the signal makes every test of this file with a probe wait as well, and the
    // whole test program would outlast its time limit. So the watch ends the test program with a failure at 5 s.
    let fertig = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let wache = std::sync::Arc::clone(&fertig);
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(5));
        if !wache.load(std::sync::atomic::Ordering::SeqCst) {
            let _ = std::io::Write::write_all(
                &mut std::io::stderr(),
                b"the wait of the probe did not end with the signal within 5 s\n",
            );
            std::process::exit(101);
        }
    });
    let aus = mit_eingriff(&a, &["copilot", "--uebernehmen"], "config-temp", || {});
    fertig.store(true, std::sync::atomic::Ordering::SeqCst);
    assert_eq!(aus.status.code(), Some(0));
    assert!(
        start.elapsed() < Duration::from_secs(5),
        "the probe waits only until the signal"
    );
}

#[test]
fn a_target_without_the_write_bit_of_the_owner_is_refused() {
    let a = aufbau();
    fs::set_permissions(&a.ziel, fs::Permissions::from_mode(0o500)).expect("mode");
    assert_eq!(
        ziel_oeffnen(&a.heim, eigene_uid(&a)).err(),
        Some(Fehler::ZielNichtSchreibbar)
    );
    fs::set_permissions(&a.ziel, fs::Permissions::from_mode(0o700)).expect("mode");
}

#[test]
fn config_json_of_exactly_one_mib_is_read_and_one_byte_more_is_refused() {
    let a = aufbau();
    let uid = eigene_uid(&a);
    let datei = a.echt.join("config.json");
    fs::write(&datei, vec![b' '; 1 << 20]).expect("1 MiB");
    let quelle = quelle_oeffnen(&a.echt, uid).expect("folder");
    let (_, text) = config_lesen(&quelle, uid).expect("exactly the limit is read");
    assert_eq!(text.len(), 1 << 20);
    fs::write(&datei, vec![b' '; (1 << 20) + 1]).expect("1 MiB + 1");
    assert_eq!(
        config_lesen(&quelle, uid).err(),
        Some(Fehler::QuelleUnsicher)
    );
}

#[test]
fn the_group_of_config_json_stays_when_it_is_not_the_first_group() {
    let (a, token) = wiederanlauf_aufbau(false);
    let datei = a.echt.join("config.json");
    let eigen = fs::metadata(&datei).expect("meta").gid();
    let Some(andere) = rustix::process::getgroups()
        .expect("groups")
        .into_iter()
        .map(rustix::process::Gid::as_raw)
        .find(|g| *g != eigen)
    else {
        // No second group: nothing to show here (the other cases keep the one group).
        return;
    };
    if rustix::fs::chown(&datei, None, Some(rustix::fs::Gid::from_raw(andere))).is_err() {
        // In a user namespace without a mapping of the group (an enclosure of the tests) the change is refused.
        return;
    }
    let aus = laufen_mit(
        &a,
        &["copilot".to_owned(), "--uebernehmen".to_owned()],
        None,
    );
    assert_eq!(
        aus.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&aus.stderr)
    );
    assert_eq!(
        fs::metadata(&datei).expect("meta").gid(),
        andere,
        "group kept"
    );
    ziel_erreicht("group", &a, &token);
}

// ---- protection against a core dump (section 3c) ----

#[test]
fn a_refused_protection_is_its_own_code_and_fail_closed() {
    use anmelden::schutz::abschirmung_auswerten;
    use kernabbild::{Abbruch, Abgeschirmt, Hinweis};
    assert_eq!(
        abschirmung_auswerten(Ok(Abgeschirmt { hinweis: None })),
        Ok(None)
    );
    assert_eq!(
        abschirmung_auswerten(Ok(Abgeschirmt {
            hinweis: Some(Hinweis::MusterDatei)
        })),
        Ok(Some(Hinweis::MusterDatei))
    );
    for grund in [
        Abbruch::AbzugSperren,
        Abbruch::GrenzeSetzen,
        Abbruch::MusterPipe,
        Abbruch::MusterNichtLesbar,
    ] {
        assert_eq!(
            abschirmung_auswerten(Err(grund)),
            Err((Fehler::Abschirmung, grund))
        );
    }
    assert_eq!(
        (
            Fehler::Abschirmung.code(),
            Fehler::Abschirmung.exit(),
            Fehler::Abschirmung.satz()
        ),
        (
            "anmelden.abschirmung",
            5,
            "Der Schutz vor einem Kernabbild ist nicht setzbar."
        )
    );
    // 5 is the code of this error only
    for f in [
        Fehler::Aufruf,
        Fehler::CliFehlt,
        Fehler::LoginAbgebrochen,
        Fehler::KeinToken,
        Fehler::FormFalsch,
        Fehler::ZielNichtSchreibbar,
        Fehler::DateiDa,
        Fehler::EchteAnmeldung,
        Fehler::HeimNichtAnlegbar,
        Fehler::Signal(1),
        Fehler::Signal(15),
        Fehler::RestNichtLoeschbar,
        Fehler::QuelleOhneToken,
        Fehler::QuelleUnsicher,
        Fehler::QuelleNichtSchreibbar,
    ] {
        assert_ne!(f.exit(), 5, "{f:?}");
    }
}

#[test]
fn the_child_starts_only_from_the_shielded_state() {
    use anmelden::schutz::{Stand, kind_pruefen};
    use kernabbild::{Abbruch, Entscheidung, Hinweis};
    let stand = |weich, hart, abzugsfaehig| Stand {
        weich,
        hart,
        abzugsfaehig,
    };
    let nie = || -> Entscheidung { unreachable!("the pattern is asked only for 0/0") };
    assert_eq!(kind_pruefen(stand(Some(1), Some(1), false), nie), Ok(()));
    // dumpable: never, whatever the limit
    for (w, h) in [(Some(1), Some(1)), (Some(0), Some(0))] {
        assert_eq!(
            kind_pruefen(stand(w, h, true), nie),
            Err(Fehler::Abschirmung),
            "{w:?}/{h:?}"
        );
    }
    // a soft limit 0 below the hard 1 would let a pipe take an image; other limits are no shield
    for (w, h) in [
        (Some(0), Some(1)),
        (Some(1), Some(2)),
        (Some(2), Some(2)),
        (None, None),
        (Some(1), None),
        (Some(0), None),
        (None, Some(1)),
    ] {
        assert_eq!(
            kind_pruefen(stand(w, h, false), nie),
            Err(Fehler::Abschirmung),
            "{w:?}/{h:?}"
        );
    }
    // 0/0: the decision of the crate for the inherited hard limit 0
    for (lage, wert) in [
        (Entscheidung::Protokoll(Hinweis::MusterDatei), Ok(())),
        (Entscheidung::Protokoll(Hinweis::MusterFehlt), Ok(())),
        (Entscheidung::Weiter, Ok(())),
        (
            Entscheidung::Abbruch(Abbruch::MusterPipe),
            Err(Fehler::Abschirmung),
        ),
        (
            Entscheidung::Abbruch(Abbruch::MusterNichtLesbar),
            Err(Fehler::Abschirmung),
        ),
    ] {
        assert_eq!(
            kind_pruefen(stand(Some(0), Some(0), false), || lage),
            wert,
            "{lage:?}"
        );
    }
}

/// The line of `/proc/<pid>/limits` for core files.
#[cfg(target_os = "linux")]
fn kernlimit(pid: u32) -> String {
    fs::read_to_string(format!("/proc/{pid}/limits"))
        .expect("limits")
        .lines()
        .find(|z| z.starts_with("Max core file size"))
        .expect("core line")
        .to_owned()
}

#[cfg(target_os = "linux")]
fn kein_kernabbild(name: &str, pid: u32, a: &Aufbau) {
    // Positive control: the entry of this test process (dumpable) belongs to the own user, and its limit line is readable.
    let selbst = fs::metadata(format!("/proc/{}/stat", std::process::id())).expect("own entry");
    assert_eq!(selbst.uid(), eigene_uid(a), "{name}: the own entry is ours");
    assert!(kernlimit(std::process::id()).starts_with("Max core file size"));
    // PR_SET_DUMPABLE 0 makes the entries under /proc/<pid> belong to root (or to nobody in a user namespace).
    let meta = fs::metadata(format!("/proc/{pid}/stat")).expect("the entry");
    assert_ne!(
        meta.uid(),
        eigene_uid(a),
        "{name}: the process is not dumpable"
    );
    let zeile = kernlimit(pid);
    let felder: Vec<&str> = zeile.split_whitespace().collect();
    assert_eq!(
        &felder[4..6],
        ["1", "1"],
        "{name}: RLIMIT_CORE 1 soft and hard: {zeile}"
    );
}

#[cfg(target_os = "linux")]
#[test]
fn the_takeover_runs_undumpable_and_without_core_files() {
    let (a, _token) = wiederanlauf_aufbau(false);
    let marke = a.wurzel.join("marke");
    let mut c = befehl(&a, &["copilot".to_owned(), "--uebernehmen".to_owned()]);
    c.env("WB_ANMELDEN_PROBE", "warten:vor-schreiben")
        .env("WB_ANMELDEN_PROBE_MARKE", &marke);
    let mut kind = c.spawn().expect("spawn");
    assert!(gemeinsam::warte_auf(&marke, Duration::from_secs(10)));
    kein_kernabbild("takeover", kind.id(), &a);
    gemeinsam::signal_senden("TERM", kind.id());
    gemeinsam::warte_auf_ende(&mut kind, Duration::from_secs(15));
}

#[cfg(target_os = "linux")]
#[test]
fn the_device_flow_runs_undumpable_and_the_cli_inherits_rlimit_core_1() {
    let a = aufbau();
    let token = token_text(&json!({"praefix": "gho_", "fuell": "P", "laenge": 40}));
    skript_schreiben(&a.skript, &attrappe_text(&a, "haengt", &token));
    let mut kind = befehl(&a, &["copilot".to_owned()]).spawn().expect("spawn");
    assert!(gemeinsam::warte_auf(&a.pidfile, Duration::from_secs(10)));
    std::thread::sleep(Duration::from_millis(200));
    kein_kernabbild("device flow", kind.id(), &a);
    let cli: u32 = fs::read_to_string(&a.pidfile)
        .expect("pid")
        .trim()
        .parse()
        .expect("n");
    // execve resets PR_SET_DUMPABLE for a program without setuid: the CLI is dumpable again (its entries are ours).
    let meta = fs::metadata(format!("/proc/{cli}/stat")).expect("entry of the CLI");
    assert_eq!(meta.uid(), eigene_uid(&a), "the CLI is dumpable after exec");
    let zeile = kernlimit(cli);
    let felder: Vec<&str> = zeile.split_whitespace().collect();
    assert_eq!(&felder[4..6], ["1", "1"], "the CLI: {zeile}");
    let log = gemeinsam::log_lesen(&a).expect("log");
    let selbst = log
        .lines()
        .find_map(|z| z.strip_prefix("kernlimit:"))
        .expect("the stand-in read its own limits");
    let felder: Vec<&str> = selbst.split_whitespace().collect();
    assert_eq!(&felder[4..6], ["1", "1"], "the CLI read: {selbst}");
    gemeinsam::signal_senden("TERM", kind.id());
    gemeinsam::warte_auf_ende(&mut kind, Duration::from_secs(15));
}
