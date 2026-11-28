//! Single functions of the library: form of the token, reading of `config.json`, the account name, the search of
//! a program in the PATH, the keychain question and the error table.
#![cfg(unix)] // the crate is Unix only (see `src/lib.rs`)

mod gemeinsam;

use std::ffi::OsString;
use std::fs;

use anmelden::fehler::Fehler;
use anmelden::form::{Fund, config_hat_anmeldung, platzhalter, token_aus_config, token_form_ok};
use anmelden::lauf::{
    ABLAGE, Bund, TempArt, ablage_suchen, bund_pruefen, finde, gruppen_schreibbar, heim_aus_passwd,
    pfad_ohne_gh, pid_aus_name, reste_raeumen, temp_reste_in, ziel_oeffnen,
};
use gemeinsam::{ablage_ebenen, aufbau, rest_anlegen, rest_name, skript_schreiben, tote_pid};
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

/// The skip of the comment lines must move forward: a skip that stays in place never ends. The read runs in a
/// thread with a deadline. A hang ends the whole test program with a failure, because the other tests of this file
/// that read comment lines would hang as well and the test run would never finish.
#[test]
fn the_skip_of_the_comment_lines_ends() {
    let t = tok("gho_", 40);
    let text = gemessen(&t);
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send((
            config_hat_anmeldung(text.as_bytes()),
            matches!(token_aus_config(text.as_bytes()), Fund::Eins(_)),
        ));
    });
    let Ok(antwort) = rx.recv_timeout(std::time::Duration::from_secs(10)) else {
        let _ = std::io::Write::write_all(
            &mut std::io::stderr(),
            b"the read of a config.json with comment lines did not end within 10 s\n",
        );
        std::process::exit(101);
    };
    assert_eq!(
        antwort,
        (true, true),
        "the read of a config.json with comment lines must find the token"
    );
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
fn the_home_comes_from_field_six_of_passwd_by_uid_and_must_be_absolute() {
    let text = "root:x:0:0:root:/root:/bin/sh\nnutzer:x:1000:1000::/srv/beispiel:/bin/sh\nrel:x:1001:1::srv/rel:/bin/sh\nleer:x:1002:1:::/bin/sh\nkurz:x:1003:1:\nzwei:x:1004:1::/srv/erst:/bin/sh\nzwei:x:1004:1::/srv/zweit:/bin/sh\n";
    let h = |uid| heim_aus_passwd(text, uid);
    assert_eq!(h(1000), Some(std::path::PathBuf::from("/srv/beispiel")));
    assert_eq!(h(0), Some(std::path::PathBuf::from("/root")));
    assert_eq!(h(1001), None, "a relative home is refused");
    assert_eq!(h(1002), None, "an empty home is refused");
    assert_eq!(h(1003), None, "an entry without field 6");
    assert_eq!(
        h(1004),
        Some(std::path::PathBuf::from("/srv/erst")),
        "the first entry wins"
    );
    assert_eq!(h(4242), None);
    assert_eq!(heim_aus_passwd("a:x:zahl:7::/srv/a:/bin/sh\n", 7), None);
    assert_eq!(
        heim_aus_passwd("a:x:7:7::/srv/a/b:/bin/sh", 7),
        Some(std::path::PathBuf::from("/srv/a/b")),
        "the last line without LF"
    );
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
fn the_store_has_five_levels_below_the_home() {
    assert_eq!(
        ABLAGE,
        [".local", "state", "agent-workbench", "halter", "geheim"]
    );
}

#[test]
fn missing_levels_of_the_store_are_made_with_0700() {
    let a = aufbau();
    fs::remove_dir_all(a.heim.join(".local")).expect("remove");
    assert!(ablage_suchen(&a.heim).is_none(), "the search makes nothing");
    assert!(!a.heim.join(".local").exists());
    let uid = uid_von(&a.wurzel);
    assert!(ziel_oeffnen(&a.heim, uid, None).is_ok());
    for ebene in ablage_ebenen(&a.heim) {
        let m = fs::symlink_metadata(&ebene).expect("level");
        assert!(m.is_dir(), "{}", ebene.display());
        assert_eq!(
            m.permissions().mode() & 0o7777,
            0o700,
            "{}",
            ebene.display()
        );
    }
    assert!(ablage_suchen(&a.heim).is_some());
    assert!(
        ziel_oeffnen(&a.heim, uid, None).is_ok(),
        "a second call finds the levels"
    );
}

#[test]
fn every_rule_of_a_level_of_the_store_is_checked() {
    let a = aufbau();
    let uid = uid_von(&a.wurzel);
    assert!(ziel_oeffnen(&a.heim, uid, None).is_ok());
    assert_eq!(
        ziel_oeffnen(&a.heim, uid + 1, None).err(),
        Some(Fehler::ZielNichtSchreibbar),
        "the home of another owner"
    );
    let ebenen = ablage_ebenen(&a.heim);
    let mut alle = vec![a.heim.clone()];
    alle.extend(ebenen.iter().cloned());
    for ebene in &alle {
        let alt = fs::metadata(ebene).expect("meta").permissions().mode() & 0o7777;
        for modus in [0o720, 0o702] {
            fs::set_permissions(ebene, fs::Permissions::from_mode(modus)).expect("mode");
            assert_eq!(
                ziel_oeffnen(&a.heim, uid, None).err(),
                Some(Fehler::ZielNichtSchreibbar),
                "{} with {modus:o}",
                ebene.display()
            );
        }
        fs::set_permissions(ebene, fs::Permissions::from_mode(alt)).expect("mode");
    }
    let geheim = ebenen.last().expect("geheim");
    for modus in [0o750, 0o705, 0o500, 0o1700] {
        fs::set_permissions(geheim, fs::Permissions::from_mode(modus)).expect("mode");
        assert_eq!(
            ziel_oeffnen(&a.heim, uid, None).err(),
            Some(Fehler::ZielNichtSchreibbar),
            "geheim with {modus:o}"
        );
    }
    fs::set_permissions(geheim, fs::Permissions::from_mode(0o700)).expect("mode");
    let halter = &ebenen[3];
    fs::set_permissions(halter, fs::Permissions::from_mode(0o750)).expect("mode");
    assert!(
        ziel_oeffnen(&a.heim, uid, None).is_ok(),
        "only geheim needs exactly 0700"
    );
    fs::set_permissions(halter, fs::Permissions::from_mode(0o700)).expect("mode");
    // A level that is a file or a link, also at the home itself.
    let local = &ebenen[0];
    fs::rename(local, a.wurzel.join("local-weg")).expect("move");
    fs::write(local, "x").expect("file");
    assert_eq!(
        ziel_oeffnen(&a.heim, uid, None).err(),
        Some(Fehler::ZielNichtSchreibbar)
    );
    assert!(ablage_suchen(&a.heim).is_none());
    fs::remove_file(local).expect("remove");
    std::os::unix::fs::symlink(a.wurzel.join("local-weg"), local).expect("link");
    assert_eq!(
        ziel_oeffnen(&a.heim, uid, None).err(),
        Some(Fehler::ZielNichtSchreibbar)
    );
    assert!(
        ablage_suchen(&a.heim).is_none(),
        "the search never follows a link"
    );
    let heim_verweis = a.wurzel.join("heim-verweis");
    std::os::unix::fs::symlink(&a.heim, &heim_verweis).expect("link");
    assert_eq!(
        ziel_oeffnen(&heim_verweis, uid, None).err(),
        Some(Fehler::ZielNichtSchreibbar)
    );
    assert!(ablage_suchen(&heim_verweis).is_none());
    assert_eq!(
        ziel_oeffnen(&a.wurzel.join("kein-heim"), uid, None).err(),
        Some(Fehler::ZielNichtSchreibbar)
    );
}

/// Decision of the operator, 28.11.2026: the home and a level above `geheim` may be writable for their group only when
/// that group is the private group handed in; o+w never, and `geheim` stays exactly 0700.
#[test]
fn only_the_private_group_may_write_a_level_above_geheim() {
    let a = aufbau();
    let uid = uid_von(&a.wurzel);
    let gid = fs::metadata(&a.wurzel).expect("meta").gid();
    let ebenen = ablage_ebenen(&a.heim);
    let mut oben = vec![a.heim.clone()];
    oben.extend(ebenen[..4].iter().cloned());
    for modus in [0o775, 0o770] {
        for ebene in &oben {
            fs::set_permissions(ebene, fs::Permissions::from_mode(modus)).expect("mode");
        }
        assert!(
            ziel_oeffnen(&a.heim, uid, Some(gid)).is_ok(),
            "{modus:o} with the private group"
        );
        for gruppe in [None, Some(gid + 1)] {
            assert_eq!(
                ziel_oeffnen(&a.heim, uid, gruppe).err(),
                Some(Fehler::ZielNichtSchreibbar),
                "{modus:o} with {gruppe:?}"
            );
        }
    }
    for ebene in &oben {
        for modus in [0o777, 0o757, 0o702] {
            fs::set_permissions(ebene, fs::Permissions::from_mode(modus)).expect("mode");
            assert_eq!(
                ziel_oeffnen(&a.heim, uid, Some(gid)).err(),
                Some(Fehler::ZielNichtSchreibbar),
                "{} with {modus:o}: o+w never",
                ebene.display()
            );
        }
        fs::set_permissions(ebene, fs::Permissions::from_mode(0o775)).expect("mode");
    }
    let geheim = ebenen.last().expect("geheim");
    for modus in [0o770, 0o750] {
        fs::set_permissions(geheim, fs::Permissions::from_mode(modus)).expect("mode");
        assert_eq!(
            ziel_oeffnen(&a.heim, uid, Some(gid)).err(),
            Some(Fehler::ZielNichtSchreibbar),
            "geheim with {modus:o}"
        );
    }
    fs::set_permissions(geheim, fs::Permissions::from_mode(0o700)).expect("mode");
    // a missing level is made with 0700 below a level of the private group
    fs::remove_dir_all(&ebenen[2]).expect("remove");
    assert!(ziel_oeffnen(&a.heim, uid, Some(gid)).is_ok());
    for ebene in &ebenen[2..] {
        let m = fs::metadata(ebene).expect("level").permissions().mode() & 0o7777;
        assert_eq!(m, 0o700, "{}", ebene.display());
    }
}

/// Addendum 1 of 28.11.2026: the remedy names the home and the levels above `geheim` that are writable for a group that
/// is not the private one, in the order of the chain; o+w, no group write, another owner and a missing home give none.
#[test]
fn the_remedy_names_the_levels_writable_for_another_group() {
    let a = aufbau();
    let uid = uid_von(&a.wurzel);
    let gid = fs::metadata(&a.wurzel).expect("meta").gid();
    let ebenen = ablage_ebenen(&a.heim);
    let mut oben = vec![a.heim.clone()];
    oben.extend(ebenen[..4].iter().cloned());
    let pfade: Vec<String> = oben.iter().map(|p| p.display().to_string()).collect();
    assert!(
        gruppen_schreibbar(&a.heim, uid, None).is_empty(),
        "0700 and 0755"
    );
    for ebene in &oben {
        fs::set_permissions(ebene, fs::Permissions::from_mode(0o775)).expect("mode");
    }
    fs::set_permissions(&ebenen[4], fs::Permissions::from_mode(0o770)).expect("mode");
    assert_eq!(
        gruppen_schreibbar(&a.heim, uid, None),
        pfade,
        "geheim is not named"
    );
    assert_eq!(gruppen_schreibbar(&a.heim, uid, Some(gid + 1)), pfade);
    assert!(
        gruppen_schreibbar(&a.heim, uid, Some(gid)).is_empty(),
        "the private group"
    );
    assert!(
        gruppen_schreibbar(&a.heim, uid + 1, None).is_empty(),
        "another owner"
    );
    assert!(
        gruppen_schreibbar(&a.wurzel.join("kein-heim"), uid, None).is_empty(),
        "no home"
    );
    for (modus, grund) in [(0o777, "o+w"), (0o755, "no group write")] {
        fs::set_permissions(&oben[1], fs::Permissions::from_mode(modus)).expect("mode");
        let mut ohne = pfade.clone();
        ohne.remove(1);
        assert_eq!(gruppen_schreibbar(&a.heim, uid, None), ohne, "{grund}");
    }
    fs::set_permissions(&ebenen[4], fs::Permissions::from_mode(0o700)).expect("mode");
}

#[test]
fn the_temp_left_overs_of_the_key_are_found_in_the_open_store() {
    let a = aufbau();
    let uid = uid_von(&a.wurzel);
    let rest = a.ziel.join(TempArt::Schluessel.name(tote_pid()));
    fs::write(&rest, "rest\n").expect("temp");
    let ablage = ablage_suchen(&a.heim).expect("store");
    assert_eq!(
        temp_reste_in(&ablage, TempArt::Schluessel, uid + 1, eigene_pid()),
        Ok(0)
    );
    assert!(rest.exists());
    assert_eq!(
        temp_reste_in(&ablage, TempArt::Schluessel, uid, eigene_pid()),
        Ok(1)
    );
    assert!(!rest.exists());
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
    if fs::metadata("/proc/1").is_ok_and(|m| m.uid() == rustix::process::getuid().as_raw()) {
        // In an own pid namespace (the enclosure of the mutation gate), pid 1 is the user's own init: the user may
        // signal it, so there is no process here that the user may not signal.
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

#[test]
fn the_throw_away_homes_lie_in_the_folder_of_the_holder_above_the_store() {
    let heim = std::path::Path::new("/srv/beispiel/heim");
    assert_eq!(
        anmelden::lauf::wegwerf_wurzel(heim),
        std::path::Path::new("/srv/beispiel/heim/.local/state/agent-workbench/halter")
    );
    assert_eq!(
        anmelden::lauf::wegwerf_wurzel(heim).join("geheim"),
        ABLAGE.iter().fold(heim.to_path_buf(), |p, e| p.join(e)),
        "the level right above the store"
    );
}
