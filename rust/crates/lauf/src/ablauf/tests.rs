//! Tests of the small pure parts of the steps: the bundle of the enclosure, the `TERM` of the harness, the target of the
//! local route.

use super::*;

#[test]
fn the_certificate_of_the_ca_stands_on_a_line_of_its_own() {
    let ca = b"-----BEGIN CERTIFICATE-----\nX\n-----END CERTIFICATE-----\n";
    let mit = bundle_inhalt(b"A\n".to_vec(), ca);
    assert_eq!(mit, [b"A\n".as_slice(), ca].concat());
    let ohne = bundle_inhalt(b"A".to_vec(), ca);
    assert_eq!(ohne, [b"A\n".as_slice(), ca].concat());
    assert_eq!(bundle_inhalt(Vec::new(), ca), ca.to_vec());
}

#[test]
fn term_is_the_own_word_or_dumb() {
    assert_eq!(
        term_wert(Some("xterm-256color".to_owned())),
        "xterm-256color"
    );
    for schlecht in ["", "a b", "x\u{1b}", "ä"] {
        assert_eq!(term_wert(Some(schlecht.to_owned())), "dumb", "{schlecht:?}");
    }
    assert_eq!(term_wert(None), "dumb");
}

#[test]
fn the_target_of_the_local_route_names_scheme_host_port_and_path() {
    let einst = einstellungen::lesen(
        br#"{"fassung":2,"klassen":["oeffentlich","intern","kundendaten"],"vorgabestufe":"kundendaten","routen":[{"schluessel":"lokal-beispiel","art":"lokal","klasse":"kundendaten","ziel":"http://127.0.0.1:11434/v1","modell":"qwen3:8b","netzziele":[]}],"kunden":[],"kundendaten_an_cloud":{"vorgabe":"nie","freigaben":[]}}"#,
    )
    .expect("settings");
    let ziel = einst
        .route("lokal-beispiel")
        .and_then(Route::ziel)
        .expect("target");
    assert_eq!(ziel_text(ziel), "http://127.0.0.1:11434/v1");
}

#[test]
fn the_folder_of_a_bare_start_file_name_is_the_current_folder() {
    let hier = std::fs::canonicalize(".").expect("cwd");
    assert_eq!(startordner(Path::new("start.json")), Some(hier.clone()));
    assert_eq!(startordner(Path::new("./start.json")), Some(hier));
    assert_eq!(
        startordner(Path::new("/nicht/da/start.json")),
        None,
        "a folder that does not exist"
    );
}

#[test]
fn the_folders_of_the_key_files_and_of_the_settings_are_protected_once() {
    let text = r#"{"fassung":1,"einstellungen":"/srv/beispiel/einst/e.json","vault":"/srv/beispiel/vault","protokoll":"/srv/beispiel/log","zustand":"/srv/beispiel/zustand","laufzeit":"/run/wb","sitzungen":"/srv/beispiel/s","heim":"/srv/beispiel/heim","path":"/usr/bin:/bin","programme":{"rangkern":"/usr/lib/werkbank/rangkern","modellweg":"/usr/lib/werkbank/werkbank-modellweg","kind":"/usr/lib/werkbank/werkbank-lauf-kind","such_mcp":"/usr/lib/werkbank/such-mcp","weiter":"/usr/lib/werkbank/einschluss-weiter"},"harness":{"pi":"/srv/beispiel/bin/pi"},"programmordner":["/srv/beispiel/bin"],"schluessel":{"lokal":"/srv/beispiel/k/lokal","cloud":"/srv/beispiel/k/cloud","im-zustand":"/srv/beispiel/zustand/x","an-der-wurzel":"/wurzel"},"ca_zulassung":"offen"}"#;
    let k = crate::konfig::lesen_text(text.as_bytes()).expect("configuration");
    let p = |t: &str| PathBuf::from(t);
    let liste = geschuetzte_ordner(
        &k,
        Path::new("/srv/beispiel/start"),
        Some(Path::new("/srv/beispiel/zaehl")),
    );
    assert_eq!(
        liste,
        [
            p("/srv/beispiel/vault"),
            p("/srv/beispiel/log"),
            p("/srv/beispiel/start"),
            p("/srv/beispiel/zustand"),
            p("/run/wb"),
            p("/srv/beispiel/zaehl"),
            p("/srv/beispiel/einst"),
            p("/srv/beispiel/k"),
        ],
        "the folder of the settings and of the keys, once each; neither the state folder again nor the root"
    );
    let ohne = geschuetzte_ordner(&k, Path::new("/srv/beispiel/start"), None);
    assert_eq!(ohne.len(), 7);
}

// Linux only: the count of threads is read from `/proc/self/status` (kind.rs `faeden`); macOS has no `/proc`, so the
// count is unreadable there and the contract treats the program as single threaded (see the test below).
#[cfg(target_os = "linux")]
#[test]
fn a_second_thread_makes_the_children_refuse_to_start() {
    // the test runner has more than one thread; the helper thread makes it certain whatever `--test-threads` says
    let (halt, wartet) = std::sync::mpsc::channel::<()>();
    let faden = std::thread::spawn(move || {
        let _ = wartet.recv();
    });
    let ergebnis = einthreadig_oder_intern();
    drop(halt);
    faden.join().expect("helper thread");
    assert_eq!(ergebnis, Err(Fehler::Intern));
}

#[cfg(not(target_os = "linux"))]
#[test]
fn without_a_readable_count_of_threads_the_fork_condition_holds_even_with_a_second_thread() {
    let (halt, wartet) = std::sync::mpsc::channel::<()>();
    let faden = std::thread::spawn(move || {
        let _ = wartet.recv();
    });
    let ergebnis = einthreadig_oder_intern();
    drop(halt);
    faden.join().expect("helper thread");
    assert_eq!(kind::faeden(), None, "no /proc here");
    assert_eq!(ergebnis, Ok(()));
}

#[test]
fn a_file_is_recorded_once_it_exists_and_a_refused_name_never() {
    let o = std::env::temp_dir().join(format!("lauf-spuren-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&o);
    std::fs::create_dir_all(&o).expect("folder");
    let mut spuren = Spuren::default();
    // a fault before the file exists (the folder is missing): nothing recorded
    assert!(spuren.schreiben(&o.join("fehlt/a"), b"x").is_err());
    assert_eq!(spuren.0, Vec::<PathBuf>::new());
    // a taken name: refused, not recorded, not removed
    std::fs::write(o.join("fremd"), b"F").expect("write");
    assert!(spuren.schreiben(&o.join("fremd"), b"x").is_err());
    assert_eq!(spuren.0, Vec::<PathBuf>::new());
    // a new file: recorded, written, removed
    spuren.schreiben(&o.join("eigen"), b"E").expect("write");
    assert_eq!(spuren.0, [o.join("eigen")]);
    assert_eq!(std::fs::read(o.join("eigen")).expect("read"), b"E");
    spuren.loeschen();
    assert_eq!(spuren.0, Vec::<PathBuf>::new());
    assert!(!o.join("eigen").exists());
    assert_eq!(std::fs::read(o.join("fremd")).expect("read"), b"F");
    let _ = std::fs::remove_dir_all(&o);
}

#[test]
fn a_start_folder_is_free_only_without_a_pid_file_or_a_link_of_that_name() {
    let o = std::env::temp_dir().join(format!("lauf-frei-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&o);
    std::fs::create_dir_all(&o).expect("folder");
    assert_eq!(startordner_frei(&o), Ok(()));
    for name in SESSIONSDATEIEN {
        std::fs::write(o.join(name), b"1\n").expect("write");
        assert_eq!(
            startordner_frei(&o),
            Err(Fehler::StartordnerBelegt),
            "{name}"
        );
        std::fs::remove_file(o.join(name)).expect("remove");
        // a link to nothing counts too
        std::os::unix::fs::symlink("/nonexistent", o.join(name)).expect("link");
        assert_eq!(
            startordner_frei(&o),
            Err(Fehler::StartordnerBelegt),
            "{name} link"
        );
        std::fs::remove_file(o.join(name)).expect("remove");
    }
    // a fault other than "not there" is no proof of "free" (fail closed): a folder that may not be searched
    if !rustix::process::geteuid().is_root() {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&o, std::fs::Permissions::from_mode(0o000)).expect("mode");
        assert_eq!(startordner_frei(&o), Err(Fehler::StartordnerBelegt));
        std::fs::set_permissions(&o, std::fs::Permissions::from_mode(0o700)).expect("mode");
    }
    let _ = std::fs::remove_dir_all(&o);
}

#[test]
fn step_1_takes_the_start_file_alone_or_after_the_option_of_the_report() {
    let os = |w: &[&str]| -> Vec<OsString> { w.iter().map(OsString::from).collect() };
    let eins = os(&["/srv/beispiel/start.json"]);
    assert_eq!(aufruf_lesen(&eins), Ok((None, &eins[0])));
    let drei = os(&[
        "--meldung",
        "/srv/beispiel/m.sock",
        "/srv/beispiel/start.json",
    ]);
    assert_eq!(
        aufruf_lesen(&drei),
        Ok((Some(Path::new("/srv/beispiel/m.sock")), &drei[2]))
    );
    for schlecht in [
        &[][..],
        &["--meldung"][..],
        &["--meldung", "m.sock", "/srv/beispiel/start.json"][..],
        &["--meldung", "./m.sock", "/srv/beispiel/start.json"][..],
        &["--meldung", "", "/srv/beispiel/start.json"][..],
        &["a", "b"][..],
        &["--meldung", "/srv/beispiel/m.sock"][..],
        &[
            "/srv/beispiel/start.json",
            "--meldung",
            "/srv/beispiel/m.sock",
        ][..],
        &["--meldung=/srv/beispiel/m.sock", "/srv/beispiel/start.json"][..],
        &[
            "--melden",
            "/srv/beispiel/m.sock",
            "/srv/beispiel/start.json",
        ][..],
        &["--meldung", "/a", "/b", "/c"][..],
    ] {
        assert_eq!(
            aufruf_lesen(&os(schlecht)),
            Err(Fehler::Aufruf),
            "{schlecht:?}"
        );
    }
}
