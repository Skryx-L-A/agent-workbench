//! Tests of the own configuration: a valid text, and one refusal per kind of fault.

use super::*;

const GUT: &str = r#"{"fassung":1,"einstellungen":"/srv/beispiel/e.json","vault":"/srv/beispiel/vault","protokoll":"/srv/beispiel/log","zustand":"/srv/beispiel/zustand","laufzeit":"/run/wb","sitzungen":"/srv/beispiel/s","gespraeche":"/srv/beispiel/zustand/gespraeche","heim":"/srv/beispiel/heim","path":"/usr/bin:/bin","programme":{"rangkern":"/usr/lib/werkbank/rangkern","modellweg":"/usr/lib/werkbank/werkbank-modellweg","kind":"/usr/lib/werkbank/werkbank-lauf-kind","such_mcp":"/usr/lib/werkbank/such-mcp","weiter":"/usr/lib/werkbank/einschluss-weiter"},"harness":{"pi":"/srv/beispiel/bin/pi","copilot":"/srv/beispiel/bin/copilot"},"programmordner":["/srv/beispiel/bin"],"schluessel":{"lokal":"/srv/beispiel/k/lokal"},"ca_zulassung":"offen"}"#;

#[test]
fn a_valid_configuration_is_read_into_its_fields() {
    let k = lesen_text(GUT.as_bytes()).expect("valid");
    assert_eq!(k.einstellungen, PathBuf::from("/srv/beispiel/e.json"));
    assert_eq!(k.laufzeit, PathBuf::from("/run/wb"));
    assert_eq!(
        k.programme.kind,
        PathBuf::from("/usr/lib/werkbank/werkbank-lauf-kind")
    );
    assert_eq!(k.harness.len(), 2);
    assert_eq!(
        k.schluessel.get("lokal"),
        Some(&PathBuf::from("/srv/beispiel/k/lokal"))
    );
    assert_eq!(k.ca_zulassung, "offen");
    assert_eq!(k.bwrap, None);
}

#[test]
fn the_harness_opencode_is_read_like_the_others() {
    let mit = GUT.replacen(
        "\"harness\":{",
        "\"harness\":{\"opencode\":\"/srv/beispiel/bin/opencode\",",
        1,
    );
    let k = lesen_text(mit.as_bytes()).expect("valid");
    assert_eq!(
        k.harness.get("opencode"),
        Some(&PathBuf::from("/srv/beispiel/bin/opencode"))
    );
    assert_eq!(k.harness.len(), 3);
    let anders = GUT.replacen("\"pi\":", "\"open-code\":", 1);
    assert_eq!(lesen_text(anders.as_bytes()), None);
}

#[test]
fn every_fault_is_refused() {
    let ersetzt = |alt: &str, neu: &str| {
        assert!(GUT.contains(alt), "{alt}");
        GUT.replacen(alt, neu, 1)
    };
    for schlecht in [
        ersetzt("\"fassung\":1", "\"fassung\":2"),
        ersetzt("\"fassung\":1", "\"fassung\":1.0"),
        ersetzt("\"vault\":\"/srv/beispiel/vault\",", ""),
        ersetzt("\"vault\":\"/srv/beispiel/vault\"", "\"vault\":\"relativ\""),
        ersetzt(
            "\"vault\":\"/srv/beispiel/vault\"",
            "\"vault\":\"/srv/../x\"",
        ),
        ersetzt("\"vault\":\"/srv/beispiel/vault\"", "\"vault\":1"),
        ersetzt("\"path\":\"/usr/bin:/bin\"", "\"path\":\"\""),
        ersetzt("\"path\":\"/usr/bin:/bin\"", "\"path\":\"/usr/bin\\n\""),
        ersetzt("\"ca_zulassung\":\"offen\"", "\"ca_zulassung\":\"ja\""),
        ersetzt("\"pi\":", "\"codex\":"),
        ersetzt("\"lokal\":", "\"Lokal\":"),
        ersetzt("\"kind\":\"/usr/lib/werkbank/werkbank-lauf-kind\",", ""),
        ersetzt("\"kind\":", "\"x\":\"/a\",\"kind\":"),
        ersetzt(
            "\"programmordner\":[\"/srv/beispiel/bin\"]",
            "\"programmordner\":[1]",
        ),
        ersetzt(
            "\"programmordner\":[\"/srv/beispiel/bin\"]",
            "\"programmordner\":\"/a\"",
        ),
        ersetzt("\"harness\":{", "\"harness\":{\"claude\":\"rel\","),
        ersetzt("{\"fassung\":1", "{\"fassung\":1,\"unbekannt\":1"),
        ersetzt(
            "{\"fassung\":1",
            "{\"fassung\":1,\"bwrap\":\"/usr/bin/bwrap\"",
        )
        .repeat(usize::from(!cfg!(feature = "pruefbau"))),
        ersetzt("{\"fassung\":1", "{\"fassung\":1,\"fassung\":1"),
    ] {
        if schlecht.is_empty() {
            continue;
        }
        assert_eq!(lesen_text(schlecht.as_bytes()), None, "{schlecht}");
    }
}

#[test]
fn the_test_build_alone_takes_a_bwrap() {
    let mit = GUT.replacen(
        "{\"fassung\":1",
        "{\"fassung\":1,\"bwrap\":\"/srv/beispiel/bwrap\"",
        1,
    );
    let k = lesen_text(mit.as_bytes());
    if cfg!(feature = "pruefbau") {
        assert_eq!(
            k.and_then(|k| k.bwrap),
            Some(PathBuf::from("/srv/beispiel/bwrap"))
        );
    } else {
        assert_eq!(k, None, "the delivered build has no way to name a bwrap");
    }
}

#[test]
fn the_delivered_build_reads_the_fixed_path() {
    if cfg!(feature = "pruefbau") {
        assert!(pfad().is_some_and(|p| p.ends_with("lauf.json") && p != Path::new(KONFIG_PFAD)));
    } else {
        assert_eq!(pfad(), Some(PathBuf::from(KONFIG_PFAD)));
    }
}

#[test]
fn a_file_others_may_write_is_refused() {
    use std::os::unix::fs::PermissionsExt;
    let p = std::env::temp_dir().join(format!("lauf-konfig-{}.json", std::process::id()));
    std::fs::write(&p, GUT).expect("write");
    for (modus, gut) in [(0o644, true), (0o600, true), (0o664, false), (0o646, false)] {
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(modus)).expect("mode");
        assert_eq!(lesen_datei(&p).is_ok(), gut, "{modus:o}");
        if !gut {
            assert_eq!(lesen_datei(&p), Err(Fehler::KonfigKaputt), "{modus:o}");
        }
    }
    let _ = std::fs::remove_file(&p);
    assert_eq!(lesen_datei(&p), Err(Fehler::KonfigFehlt), "missing");
}

#[test]
fn a_relative_program_folder_and_a_file_over_64_kib_are_refused() {
    use std::os::unix::fs::PermissionsExt;
    let rel = GUT.replacen(
        "\"programmordner\":[\"/srv/beispiel/bin\"]",
        "\"programmordner\":[\"bin\"]",
        1,
    );
    assert_eq!(lesen_text(rel.as_bytes()), None);
    let p = std::env::temp_dir().join(format!("lauf-konfig-gross-{}.json", std::process::id()));
    for (n, gut) in [(65_536, true), (65_537, false)] {
        let mut t = GUT.as_bytes().to_vec();
        t.resize(n, b' ');
        std::fs::write(&p, &t).expect("write");
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o600)).expect("mode");
        assert_eq!(lesen_datei(&p).is_ok(), gut, "{n}");
        if !gut {
            assert_eq!(lesen_datei(&p), Err(Fehler::KonfigKaputt), "{n}");
        }
    }
    let _ = std::fs::remove_file(&p);
}

#[test]
fn the_rule_of_the_file_takes_kind_mode_and_owner() {
    // own file and root file pass, a foreign owner does not
    assert!(modus_gut(true, 0o100_644, 5, 5));
    assert!(modus_gut(true, 0o100_644, 0, 5));
    assert!(modus_gut(true, 0o100_600, 0, 0));
    assert!(!modus_gut(true, 0o100_644, 6, 5));
    assert!(!modus_gut(true, 0o100_644, 6, 0));
    // another user may write: group or other
    assert!(!modus_gut(true, 0o100_664, 5, 5));
    assert!(!modus_gut(true, 0o100_646, 0, 5));
    assert!(!modus_gut(true, 0o100_620, 0, 5));
    assert!(!modus_gut(true, 0o100_602, 0, 5));
    // not a regular file
    assert!(!modus_gut(false, 0o100_644, 5, 5));
}

#[test]
fn a_missing_file_is_another_fault_than_a_broken_one() {
    use std::os::unix::fs::PermissionsExt;
    let o = std::env::temp_dir().join(format!("lauf-konfig-art-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&o);
    std::fs::create_dir_all(&o).expect("folder");
    // a missing file, and a file below a missing folder: missing
    assert_eq!(lesen_datei(&o.join("fehlt.json")), Err(Fehler::KonfigFehlt));
    assert_eq!(
        lesen_datei(&o.join("unter/fehlt.json")),
        Err(Fehler::KonfigFehlt)
    );
    // a folder instead of a file, a text that is no JSON, a file that cannot be read: broken
    assert_eq!(lesen_datei(&o), Err(Fehler::KonfigKaputt));
    let p = o.join("lauf.json");
    std::fs::write(&p, b"{").expect("write");
    std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o600)).expect("mode");
    assert_eq!(lesen_datei(&p), Err(Fehler::KonfigKaputt));
    std::fs::write(&p, GUT).expect("write");
    assert!(lesen_datei(&p).is_ok());
    std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o000)).expect("mode");
    // the owner may read a 0000 file only as root; the open fault is `broken`, never `missing`
    if rustix::process::geteuid().as_raw() != 0 {
        assert_eq!(lesen_datei(&p), Err(Fehler::KonfigKaputt));
    }
    let _ = std::fs::remove_dir_all(&o);
}
