//! Single functions of the library: form of the token, reading of `config.json`, the account name, the search of
//! a program in the PATH, the keychain question and the error table.
#![cfg(unix)] // the crate is Unix only (see `src/lib.rs`)

mod gemeinsam;

use std::ffi::OsString;
use std::fs;

use anmelden::fehler::Fehler;
use anmelden::form::{Fund, config_hat_anmeldung, platzhalter, token_aus_config, token_form_ok};
use anmelden::lauf::{
    Bund, bund_pruefen, finde, name_aus_passwd, nutzer_ordner, pfad_ohne_gh, pid_aus_name,
    reste_raeumen,
};
use gemeinsam::{aufbau, rest_anlegen, rest_name, skript_schreiben, tote_pid};
use std::os::unix::fs::{MetadataExt, PermissionsExt};

fn tok(praefix: &str, laenge: usize) -> String {
    format!("{praefix}{}", "a".repeat(laenge - praefix.len()))
}

#[test]
fn the_form_accepts_the_three_prefixes_and_the_bounds() {
    assert!(token_form_ok(&tok("gho_", 40)));
    assert!(token_form_ok(&tok("ghu_", 40)));
    assert!(token_form_ok(&tok("github_pat_", 93)));
    assert!(token_form_ok(&tok("gho_", 20)));
    assert!(!token_form_ok(&tok("gho_", 19)));
    assert!(token_form_ok(&tok("gho_", 255)));
    assert!(!token_form_ok(&tok("gho_", 256)));
    assert!(token_form_ok(&format!(
        "gho_{}_{}",
        "a".repeat(10),
        "B9".repeat(8)
    )));
}

#[test]
fn the_form_rejects_other_prefixes_characters_and_the_placeholder() {
    assert!(!token_form_ok(&tok("ghp_", 40)));
    assert!(!token_form_ok(&format!("x{}", tok("gho_", 40))));
    assert!(!token_form_ok(&tok("gh_", 40)));
    assert!(!token_form_ok(&tok("github_", 40)));
    assert!(!token_form_ok(&format!("{} ", tok("gho_", 40))));
    assert!(!token_form_ok(&format!("{}-", tok("gho_", 40))));
    assert!(!token_form_ok(&format!("{}\n", tok("gho_", 40))));
    assert!(!token_form_ok(&format!("{}\u{e4}", tok("gho_", 40))));
    assert!(!token_form_ok(""));
    assert!(!token_form_ok(&platzhalter()));
    assert_eq!(platzhalter().len(), 40);
    assert!(platzhalter().starts_with("gho_") && platzhalter()[4..].bytes().all(|b| b == b'0'));
}

fn config(inhalt: &str) -> Fund {
    token_aus_config(inhalt.as_bytes())
}

/// The measured form of CLI 1.0.88 (21.11.2026) with an invented token: two whole comment lines, then JSON with the
/// six fields of the top level and the token under `authTokens` in the depth object, object, text.
fn gemessen(t: &str) -> String {
    format!(
        "// User settings belong in settings.json.\n// This file is managed automatically.\n{{\n  \"appTipShown\": true,\n  \
         \"authTokens\": {{\n    \"https://github.com\": {{\n      \"attrappe\": \"{t}\"\n    }}\n  }},\n  \
         \"firstLaunchAt\": \"2026-11-21T09:00:00.000Z\",\n  \"lastLoggedInUser\": {{\n    \"host\": \"https://github.com\",\n    \
         \"login\": \"attrappe\"\n  }},\n  \"loggedInUsers\": [\n    {{\n      \"host\": \"https://github.com\",\n      \
         \"login\": \"attrappe\"\n    }}\n  ],\n  \"reasoningSummariesCleanupDone\": true\n}}\n"
    )
}

fn g(t: &str) -> String {
    format!("{{\"authTokens\":{{\"h\":{{\"a\":\"{t}\"}}}}}}")
}

#[test]
fn config_gives_one_token_none_or_many() {
    let t = tok("gho_", 40);
    match config(&gemessen(&t)) {
        Fund::Eins(x) => assert_eq!(x.as_str(), t),
        _ => panic!("one token expected"),
    }
    match config(&g(&t)) {
        Fund::Eins(x) => assert_eq!(x.as_str(), t),
        _ => panic!("one token expected"),
    }
    // none: empty, empty text, not a text, wrong depth, other field, no object
    for text in [
        "{\"authTokens\":{}}",
        "{\"authTokens\":{\"h\":{}}}",
        "{\"authTokens\":{\"h\":{\"a\":\"\"}}}",
        "{\"authTokens\":{\"h\":{\"a\":5}}}",
        "{\"authTokens\":{\"h\":[\"x\"]}}",
        "{\"authTokens\":{\"h\":\"x\"}}",
        "{\"authTokens\":{\"h\":{\"g\":{\"a\":\"x\"}}}}",
        "{\"anders\":{\"h\":{\"a\":\"x\"}}}",
        "{\"authTokens\":[]}",
        "[]",
    ] {
        assert!(matches!(config(text), Fund::Keins), "{text}");
    }
    // a token in another depth is none; so is the old form (never measured)
    for text in [
        format!("{{\"authTokens\":{{\"h\":\"{t}\"}}}}"),
        format!("{{\"authTokens\":{{\"h\":{{\"g\":{{\"a\":\"{t}\"}}}}}}}}"),
        format!("{{\"copilot_tokens\":{{\"a\":\"{t}\"}}}}"),
    ] {
        assert!(matches!(config(&text), Fund::Keins), "{text}");
    }
    // two texts in the depth
    for text in [
        "{\"authTokens\":{\"h\":{\"a\":\"x\",\"b\":\"y\"}}}",
        "{\"authTokens\":{\"h\":{\"a\":\"x\"},\"i\":{\"b\":\"y\"}}}",
    ] {
        assert!(matches!(config(text), Fund::Mehrere), "{text}");
    }
    // a second text with a token prefix anywhere else makes it more than one
    let u = tok("ghu_", 40);
    let pat = tok("github_pat_", 93);
    for text in [
        format!(
            "{{\"authTokens\":{{\"h\":{{\"a\":\"{t}\"}}}},\"loggedInUsers\":[{{\"x\":\"{u}\"}}]}}"
        ),
        format!("{{\"authTokens\":{{\"h\":{{\"a\":\"{t}\"}}}},\"x\":\"{pat}\"}}"),
    ] {
        assert!(matches!(config(&text), Fund::Mehrere), "{text}");
    }
    // a text without the prefix elsewhere is no second token
    assert!(matches!(
        config(&format!(
            "{{\"authTokens\":{{\"h\":{{\"a\":\"{t}\"}}}},\"x\":\"nein\"}}"
        )),
        Fund::Eins(_)
    ));
    // only whole leading comment lines are skipped, nothing else
    for text in [
        "{kein json".to_owned(),
        String::new(),
        "// nur ein Kommentar\n".to_owned(),
        "// ohne Zeilenende".to_owned(),
        "//\n//\n".to_owned(),
        // a comment in the middle
        format!("// kopf\n{{\n// mitte\n\"authTokens\":{{\"h\":{{\"a\":\"{t}\"}}}}}}\n"),
        // a comment after the JSON
        format!("{}\n// ende\n", g(&t)),
        // a block comment, also after a comment line
        format!("\x2f* kopf *\x2f\n{}", g(&t)),
        format!("// kopf\n\x2f* mehr *\x2f\n{}", g(&t)),
        // a comment that does not start the line
        format!(" // kopf\n{}", g(&t)),
        format!("{{}} // kopf\n{}", g(&t)),
        // a comma before } or ]
        format!("{{\"authTokens\":{{\"h\":{{\"a\":\"{t}\",}}}}}}"),
        format!("{{\"authTokens\":{{\"h\":{{\"a\":\"{t}\"}},}}}}"),
        format!("{{\"authTokens\":{{\"h\":{{\"a\":\"{t}\"}}}},\"l\":[1,]}}"),
        // text before the first brace that is no whole comment line
        format!("x\n{}", g(&t)),
        format!("\u{feff}{}", g(&t)),
    ] {
        assert!(matches!(config(&text), Fund::KeinJson), "{text}");
    }
    // a CRLF comment line is a whole line (it ends with LF)
    assert!(matches!(
        config(&format!("// kopf\r\n{}", g(&t))),
        Fund::Eins(_)
    ));
}

#[test]
fn a_login_in_the_real_home_is_found_fail_closed() {
    let t = tok("gho_", 40);
    let h = |s: &str| config_hat_anmeldung(s.as_bytes());
    assert!(h(&gemessen(&t)), "the measured form");
    assert!(h(&g(&t)));
    assert!(
        h("{\"authTokens\":{\"h\":{\"a\":\"irgendein-text\"}}}"),
        "any non-empty text in the depth of the field"
    );
    assert!(
        h(&format!("{{\"anderes\":\"{t}\"}}")),
        "token-shaped text in another field"
    );
    assert!(
        h(&format!("{{\"copilot_tokens\":{{\"a\":\"{t}\"}}}}")),
        "token-shaped text in the old form"
    );
    assert!(
        h(&format!("{{\"x\":[{{\"y\":\"{t}\"}}]}}")),
        "token-shaped text in a list"
    );
    assert!(h(&format!(
        "{{\"x\":{{\"y\":{{\"z\":\"{}\"}}}}}}",
        tok("github_pat_", 93)
    )));
    assert!(h(&format!("{{\"x\":\"{}\"}}", tok("ghu_", 40))));
    assert!(h("{nicht json"), "unreadable counts as a login");
    assert!(h(""));
    assert!(
        h("// kopf\n// mitte\n\x2f* x *\x2f\n{\"authTokens\":{}}"),
        "another form than the measured one is unreadable, so a login"
    );
    assert!(h("// nur\n"), "comment lines without JSON are unreadable");
    assert!(!h("{\"banner\":\"never\"}"));
    assert!(!h("{\"authTokens\":{}}"));
    assert!(!h("{\"authTokens\":{\"h\":{}}}"));
    assert!(!h("{\"authTokens\":{\"h\":{\"a\":\"\"}}}"));
    assert!(
        !h("// eins\n// zwei\n{\"authTokens\":{\"h\":{}},\"loggedInUsers\":[]}\n"),
        "the measured form after the token is gone"
    );
    assert!(
        !h("{\"authTokens\":{\"h\":\"irgendein-text\"}}"),
        "a text in another depth is no login of the field"
    );
    assert!(
        !h("{\"x\":\"gho_kurz\"}"),
        "a short text with the prefix is no token"
    );
    assert!(!h(&format!("{{\"x\":\"{}\"}}", tok("ghp_", 40))));
    assert!(!h("{\"x\":5,\"y\":null,\"z\":true}"));
}

#[test]
fn the_account_name_comes_from_passwd_by_uid_and_must_be_plain() {
    let text = "root:x:0:0:root:/root:/bin/sh\nnutzer:x:1000:1000::/srv/beispiel:/bin/sh\nbad/name:x:1001:1::/h:/bin/sh\n_dienst:x:1002:1::/h:/bin/sh\nGross:x:1003:1::/h:/bin/sh\n9ab:x:1004:1::/h:/bin/sh\nkurz\n";
    assert_eq!(name_aus_passwd(text, 1000).as_deref(), Some("nutzer"));
    assert_eq!(name_aus_passwd(text, 0).as_deref(), Some("root"));
    assert_eq!(name_aus_passwd(text, 1002).as_deref(), Some("_dienst"));
    assert_eq!(name_aus_passwd(text, 1001), None);
    assert_eq!(name_aus_passwd(text, 1003), None);
    assert_eq!(name_aus_passwd(text, 1004), None);
    assert_eq!(name_aus_passwd(text, 4242), None);
    let lang = format!("{}:x:7:7::/h:/bin/sh\n", "a".repeat(33));
    assert_eq!(name_aus_passwd(&lang, 7), None);
    let grenze = format!("{}:x:7:7::/h:/bin/sh\n", "a".repeat(32));
    assert!(name_aus_passwd(&grenze, 7).is_some());
    assert_eq!(name_aus_passwd(":x:7:7::/h:/bin/sh\n", 7), None);
    assert_eq!(
        name_aus_passwd("a-b_c9:x:7:7:\n", 7).as_deref(),
        Some("a-b_c9")
    );
    assert_eq!(name_aus_passwd("a.b:x:7:7:\n", 7), None);
    assert_eq!(name_aus_passwd("a:x:zahl:7:\n", 7), None);
}

#[test]
fn the_search_takes_the_first_executable_regular_file() {
    let a = aufbau();
    let zwei = a.wurzel.join("zwei");
    fs::create_dir_all(&zwei).expect("dir");
    skript_schreiben(&a.bin.join("werkzeug"), "#!/bin/sh\n");
    skript_schreiben(&zwei.join("werkzeug"), "#!/bin/sh\n");
    let pfad = std::env::join_paths([&a.gh, &a.bin, &zwei]).expect("paths");
    assert_eq!(finde(&pfad, "werkzeug"), Some(a.bin.join("werkzeug")));
    fs::write(a.gh.join("nichtausfuehrbar"), "x").expect("file");
    assert_eq!(finde(&pfad, "nichtausfuehrbar"), None);
    fs::create_dir_all(a.gh.join("ordnername")).expect("dir");
    assert_eq!(finde(&pfad, "ordnername"), None, "a folder is no program");
    assert_eq!(finde(&pfad, "gibtesnicht"), None);
    assert_eq!(finde(&OsString::new(), "werkzeug"), None);
}

#[test]
fn the_path_of_the_cli_loses_every_folder_with_a_gh() {
    let a = aufbau();
    let pfad = std::env::join_paths([&a.bin, &a.gh, &a.wegwerf]).expect("paths");
    let ohne = pfad_ohne_gh(&pfad);
    let teile: Vec<_> = std::env::split_paths(&ohne).collect();
    assert_eq!(teile, vec![a.bin.clone(), a.wegwerf.clone()]);
    fs::write(a.bin.join("gh"), "x").expect("file");
    assert_eq!(
        std::env::split_paths(&pfad_ohne_gh(&pfad)).count(),
        2,
        "a gh without the execute bit is no gh"
    );
}

#[test]
fn the_keychain_question_reads_the_exit_of_the_tool_that_exists() {
    let a = aufbau();
    let pfad = a.bin.clone().into_os_string();
    assert_eq!(bund_pruefen(&pfad), Bund::NichtPruefbar);
    skript_schreiben(&a.bin.join("security"), "#!/bin/sh\nexit 44\n");
    assert_eq!(bund_pruefen(&pfad), Bund::Leer);
    skript_schreiben(&a.bin.join("security"), "#!/bin/sh\nexit 0\n");
    assert_eq!(bund_pruefen(&pfad), Bund::Gefunden);
    skript_schreiben(&a.bin.join("security"), "#!/bin/sh\nexit 44\n");
    skript_schreiben(&a.bin.join("secret-tool"), "#!/bin/sh\nexit 0\n");
    assert_eq!(
        bund_pruefen(&pfad),
        Bund::Gefunden,
        "any tool that finds an entry counts"
    );
    skript_schreiben(&a.bin.join("secret-tool"), "#!/bin/sh\nexit 1\n");
    assert_eq!(bund_pruefen(&pfad), Bund::Leer);
}

#[test]
fn the_keychain_tool_gets_the_documented_arguments_and_no_output_is_kept() {
    let a = aufbau();
    let pfad = a.bin.clone().into_os_string();
    let spur = a.wurzel.join("args.txt");
    skript_schreiben(
        &a.bin.join("secret-tool"),
        &format!(
            "#!/bin/sh\necho \"$*\" > '{}'\necho GEHEIM\nexit 1\n",
            spur.display()
        ),
    );
    assert_eq!(bund_pruefen(&pfad), Bund::Leer);
    assert_eq!(
        fs::read_to_string(&spur).expect("trace").trim(),
        "lookup service copilot-cli"
    );
    fs::remove_file(a.bin.join("secret-tool")).expect("remove");
    skript_schreiben(
        &a.bin.join("security"),
        &format!("#!/bin/sh\necho \"$*\" > '{}'\nexit 1\n", spur.display()),
    );
    assert_eq!(bund_pruefen(&pfad), Bund::Leer);
    assert_eq!(
        fs::read_to_string(&spur).expect("trace").trim(),
        "find-generic-password -s copilot-cli"
    );
}

#[test]
fn every_error_has_its_code_exit_and_sentence() {
    let alle = [
        (
            Fehler::Aufruf,
            "anmelden.aufruf",
            64,
            "Aufruf: werkbank-anmelden copilot [--uebernehmen] [--neu]",
        ),
        (
            Fehler::CliFehlt,
            "anmelden.cli_fehlt",
            10,
            "Die Copilot CLI wurde nicht gefunden.",
        ),
        (
            Fehler::LoginAbgebrochen,
            "anmelden.login_abgebrochen",
            11,
            "Die Anmeldung wurde nicht abgeschlossen.",
        ),
        (
            Fehler::KeinToken,
            "anmelden.kein_token",
            12,
            "Die Anmeldung hat kein Token geliefert.",
        ),
        (
            Fehler::FormFalsch,
            "anmelden.form_falsch",
            13,
            "Das Token hat nicht die erwartete Form.",
        ),
        (
            Fehler::ZielNichtSchreibbar,
            "anmelden.ziel_nicht_schreibbar",
            14,
            "Der Zielordner ist nicht schreibbar.",
        ),
        (
            Fehler::DateiDa,
            "anmelden.datei_da",
            15,
            "Die Schluesseldatei ist schon da. Mit --neu wird sie ersetzt.",
        ),
        (
            Fehler::EchteAnmeldung,
            "anmelden.echte_anmeldung",
            16,
            "Im Heim des Nutzers liegt eine Anmeldung der Copilot CLI. Sie muss weg, bevor eine Sitzung startet.",
        ),
        (
            Fehler::HeimNichtAnlegbar,
            "anmelden.heim_nicht_anlegbar",
            17,
            "Das Wegwerf-Heim ist nicht anlegbar.",
        ),
        (
            Fehler::RestNichtLoeschbar,
            "anmelden.rest_nicht_loeschbar",
            18,
            "Ein Rest eines frueheren Laufs ist nicht loeschbar.",
        ),
        (
            Fehler::Signal(2),
            "anmelden.signal",
            130,
            "Abgebrochen durch ein Signal.",
        ),
    ];
    for (f, code, exit, satz) in alle {
        assert_eq!(f.code(), code);
        assert_eq!(f.exit(), exit);
        assert_eq!(f.satz(), satz);
        assert_eq!(f.zeile(), format!("werkbank-anmelden: {code}: {satz}\n"));
    }
    assert_eq!(Fehler::Signal(15).exit(), 143);
    assert_eq!(Fehler::Signal(1).exit(), 129);
    assert_eq!(Fehler::Signal(1000).exit(), 255);
}

#[test]
fn the_folder_of_the_user_is_the_place_plus_the_name() {
    let text = "nutzer:x:1000:1000::/srv/beispiel:/bin/sh\n";
    assert_eq!(
        nutzer_ordner(std::path::Path::new("/srv/ort"), text, 1000),
        Some(std::path::PathBuf::from("/srv/ort/nutzer"))
    );
    assert_eq!(
        nutzer_ordner(std::path::Path::new("/srv/ort"), text, 1),
        None
    );
}

fn eigene_pid() -> u32 {
    std::process::id()
}

fn uid_von(pfad: &std::path::Path) -> u32 {
    fs::metadata(pfad).expect("meta").uid()
}

#[test]
fn the_name_of_a_left_over_must_have_exactly_the_form_with_pid_and_nanos() {
    let pid = |n: &str| pid_aus_name(n).map(rustix::process::Pid::as_raw_pid);
    assert_eq!(pid("wb-anmelden-123-456"), Some(123));
    assert_eq!(pid("wb-anmelden-1-0"), Some(1));
    for falsch in [
        "wb-anmelden-",
        "wb-anmelden-12",
        "wb-anmelden-12-",
        "wb-anmelden--5",
        "wb-anmelden-x-5",
        "wb-anmelden-12-x",
        "wb-anmelden-12-5x",
        "wb-anmelden-12-5-6",
        "wb-anmelden-0-5",
        "wb-anmelden-99999999999-5",
        "wb-anmelden-+5-5",
        "xwb-anmelden-12-5",
        "wb-anmelden-12-5/",
        "",
    ] {
        assert!(pid_aus_name(falsch).is_none(), "{falsch}");
    }
}

#[test]
fn a_left_over_of_a_dead_run_is_deleted_and_counted() {
    let a = aufbau();
    let zwei = rest_anlegen(&a.wegwerf, tote_pid());
    let eins = rest_anlegen(&a.wegwerf, tote_pid());
    let n = reste_raeumen(&a.wegwerf, uid_von(&a.wegwerf), eigene_pid());
    assert_eq!(n, Ok(2));
    assert!(!zwei.exists() && !eins.exists());
    assert_eq!(fs::read_dir(&a.wegwerf).expect("dir").count(), 0);
}

#[test]
fn a_left_over_of_a_living_run_stays() {
    let mut a = aufbau();
    let kind = std::process::Command::new("sleep")
        .arg("60")
        .spawn()
        .expect("sleep");
    let ordner = rest_anlegen(&a.wegwerf, kind.id());
    a.hilfskind = Some(kind);
    assert_eq!(
        reste_raeumen(&a.wegwerf, uid_von(&a.wegwerf), eigene_pid()),
        Ok(0)
    );
    assert!(ordner.join("heim/.copilot/config.json").exists());
}

#[test]
fn a_folder_of_a_foreign_owner_stays_even_when_its_run_is_dead() {
    let a = aufbau();
    let ordner = rest_anlegen(&a.wegwerf, tote_pid());
    let fremd = uid_von(&a.wegwerf) + 1;
    assert_eq!(reste_raeumen(&a.wegwerf, fremd, eigene_pid()), Ok(0));
    assert!(ordner.exists());
    assert_eq!(
        reste_raeumen(&a.wegwerf, uid_von(&a.wegwerf), eigene_pid()),
        Ok(1)
    );
    assert!(!ordner.exists());
}

#[test]
fn a_link_with_the_name_of_a_left_over_stays_and_is_not_followed() {
    let a = aufbau();
    fs::create_dir_all(&a.opfer).expect("victim");
    fs::write(a.opfer.join("probe.txt"), "bleibt").expect("probe");
    let verweis = a.wegwerf.join(rest_name(tote_pid()));
    std::os::unix::fs::symlink(&a.opfer, &verweis).expect("link");
    let tot_verweis = a.wegwerf.join(rest_name(tote_pid()));
    std::os::unix::fs::symlink(a.wegwerf.join("gibt-es-nicht"), &tot_verweis).expect("link");
    assert_eq!(
        reste_raeumen(&a.wegwerf, uid_von(&a.wegwerf), eigene_pid()),
        Ok(0)
    );
    assert!(fs::symlink_metadata(&verweis).is_ok());
    assert!(fs::symlink_metadata(&tot_verweis).is_ok());
    assert_eq!(
        fs::read_to_string(a.opfer.join("probe.txt")).expect("probe"),
        "bleibt"
    );
}

#[test]
fn a_link_inside_a_left_over_is_removed_without_touching_its_target() {
    let a = aufbau();
    fs::create_dir_all(&a.opfer).expect("victim");
    fs::write(a.opfer.join("probe.txt"), "bleibt").expect("probe");
    let ordner = rest_anlegen(&a.wegwerf, tote_pid());
    std::os::unix::fs::symlink(&a.opfer, ordner.join("heim/verweis")).expect("link");
    assert_eq!(
        reste_raeumen(&a.wegwerf, uid_von(&a.wegwerf), eigene_pid()),
        Ok(1)
    );
    assert!(!ordner.exists());
    assert_eq!(
        fs::read_to_string(a.opfer.join("probe.txt")).expect("probe"),
        "bleibt"
    );
}

#[test]
fn other_names_and_plain_files_stay() {
    let a = aufbau();
    let pid = tote_pid();
    for name in [
        format!("wb-anmelden-{pid}"),
        format!("wb-anmelden-{pid}-x"),
        format!("anderer-{pid}-5"),
        "wb-anmelden-0-5".to_owned(),
    ] {
        fs::create_dir(a.wegwerf.join(&name)).expect("folder");
    }
    fs::write(a.wegwerf.join(rest_name(pid)), "keine Ordner").expect("file");
    assert_eq!(
        reste_raeumen(&a.wegwerf, uid_von(&a.wegwerf), eigene_pid()),
        Ok(0)
    );
    assert_eq!(fs::read_dir(&a.wegwerf).expect("dir").count(), 5);
}

#[test]
fn a_folder_with_the_own_pid_is_old_at_the_start_and_deleted() {
    let a = aufbau();
    let ordner = rest_anlegen(&a.wegwerf, eigene_pid());
    let andere = rest_anlegen(&a.wegwerf, eigene_pid() + 1_000_000_000);
    assert_eq!(
        reste_raeumen(&a.wegwerf, uid_von(&a.wegwerf), eigene_pid()),
        Ok(2)
    );
    assert!(!ordner.exists() && !andere.exists());
}

#[test]
fn a_folder_of_a_process_the_user_may_not_signal_is_deleted() {
    if rustix::process::getuid().as_raw() == 0 {
        // pid 1 belongs to root; as root itself the signal test passes and the folder would stay.
        return;
    }
    let a = aufbau();
    let ordner = rest_anlegen(&a.wegwerf, 1);
    assert_eq!(
        reste_raeumen(&a.wegwerf, uid_von(&a.wegwerf), eigene_pid()),
        Ok(1)
    );
    assert!(!ordner.exists());
}

#[test]
fn a_left_over_that_cannot_be_deleted_is_an_error_and_stays() {
    if rustix::process::getuid().as_raw() == 0 {
        return;
    }
    let a = aufbau();
    let ordner = rest_anlegen(&a.wegwerf, tote_pid());
    fs::set_permissions(ordner.join("heim"), fs::Permissions::from_mode(0o500)).expect("mode");
    assert_eq!(
        reste_raeumen(&a.wegwerf, uid_von(&a.wegwerf), eigene_pid()),
        Err(Fehler::RestNichtLoeschbar)
    );
    assert!(ordner.exists());
}

#[test]
fn a_missing_or_unreadable_root_has_no_left_overs() {
    let a = aufbau();
    assert_eq!(
        reste_raeumen(&a.wegwerf.join("fehlt"), uid_von(&a.wegwerf), eigene_pid()),
        Ok(0)
    );
    let datei = a.wegwerf.join("datei");
    fs::write(&datei, "x").expect("file");
    assert_eq!(
        reste_raeumen(&datei, uid_von(&a.wegwerf), eigene_pid()),
        Ok(0)
    );
}
