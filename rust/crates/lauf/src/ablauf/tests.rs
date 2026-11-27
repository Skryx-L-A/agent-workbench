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

// ---- the steps with a world of their own ---------------------------------------------------------------------------
//
// The delivered build reads its configuration below `/etc`, so the steps after step 2 run here on a world of the test:
// a configuration, the settings, a start file and stand-ins for the children. The trampoline is a shell script that
// replaces itself by the program (the real one binds itself to the parent, which `tests/trampolin.rs` checks).

/// Settings of version 2 with a cloud route (one bound and one unbound target) and a local route.
const EINST: &str = r#"{"fassung":2,"klassen":["oeffentlich","intern","kundendaten"],"vorgabestufe":"kundendaten","routen":[{"schluessel":"cloud-beispiel","art":"cloud","klasse":"intern","netzziele":[{"host":"api.cloud.example","port":443,"zugriff":"kopf-einsetzen","schluessel":true,"pfade":[{"methode":"GET","pfad":"/copilot_internal/*"}]},{"host":"telemetry.cloud.example","port":443,"zugriff":"kopf-einsetzen","schluessel":false,"pfade":[{"methode":"POST","pfad":"/telemetry"}]}]},{"schluessel":"lokal-beispiel","art":"lokal","klasse":"kundendaten","ziel":"http://127.0.0.1:11434/v1","modell":"qwen3:8b","netzziele":[]}],"kunden":[{"schluessel":"k-beispiel","erlaubt":["lokal-beispiel"],"verboten":[]}],"kundendaten_an_cloud":{"vorgabe":"nie","freigaben":[]}}"#;

/// The stand-in of the trampoline: the watcher sleeps, every other call becomes the program.
const TRAMPOLIN: &str =
    "#!/bin/sh\nif [ \"$1\" = \"--wache\" ]; then exec /bin/sleep 30; fi\nshift 2\nexec \"$@\"\n";

/// The stand-in of a child: writes its arguments next to itself, gives the ready line and sleeps.
const KIND: &str = "#!/bin/sh\nprintf '%s\\n' \"$@\" > \"$0.args\"\nprintf '{\"bereit\":true}\\n'\nexec /bin/sleep 30\n";

fn schreiben(p: &Path, text: &str, modus: u32) {
    use std::os::unix::fs::PermissionsExt;
    std::fs::write(p, text).expect("write");
    std::fs::set_permissions(p, std::fs::Permissions::from_mode(modus)).expect("mode");
}

/// Writes a program that a test runs, through a helper process: this process never holds the file open for writing.
///
/// A child that another test of this process forks while the file is open for writing keeps that descriptor until its
/// `exec`; an `exec` of the file then fails with `ETXTBSY`, and the start counts as missing. The helper is a process of
/// its own, which no test forks.
fn programm_schreiben(p: &Path, text: &str) {
    let status = std::process::Command::new("/bin/sh")
        .args([
            "-c",
            "printf '%s' \"$2\" > \"$1\" && chmod 755 \"$1\"",
            "sh",
        ])
        .arg(p)
        .arg(text)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .stdin(std::process::Stdio::null())
        .status()
        .expect("helper");
    assert!(status.success(), "write {}", p.display());
}

fn ordner_0700(p: &Path) {
    use std::os::unix::fs::PermissionsExt;
    std::fs::create_dir_all(p).expect("folder");
    std::fs::set_permissions(p, std::fs::Permissions::from_mode(0o700)).expect("mode");
}

/// The folders, the configuration and the settings of a world below `w`; returns the path of the configuration.
fn welt(w: &Path) -> PathBuf {
    for d in [
        "vault",
        "protokoll",
        "laufzeit",
        "sitzungen",
        "heim",
        "arbeit",
        "start",
        "bin",
        "einst",
        "k",
    ] {
        ordner_0700(&w.join(d));
    }
    ordner_0700(&w.join("zustand"));
    ordner_0700(&w.join("zustand/sitzungen"));
    programm_schreiben(&w.join("bin/kind"), TRAMPOLIN);
    for p in ["rangkern", "modellweg", "pi", "copilot"] {
        programm_schreiben(&w.join("bin").join(p), KIND);
    }
    schreiben(&w.join("einst/e.json"), EINST, 0o600);
    schreiben(&w.join("k/lokal"), "schluessel\n", 0o600);
    schreiben(&w.join("k/cloud"), "schluessel\n", 0o600);
    let t = |p: &str| w.join(p).display().to_string();
    let konfig = format!(
        r#"{{"fassung":1,"einstellungen":"{}","vault":"{}","protokoll":"{}","zustand":"{}","laufzeit":"{}","sitzungen":"{}","heim":"{}","path":"/usr/bin:/bin","programme":{{"rangkern":"{}","modellweg":"{}","kind":"{}","such_mcp":"/usr/lib/werkbank/such-mcp","weiter":"/usr/lib/werkbank/einschluss-weiter"}},"harness":{{"pi":"{}","copilot":"{}"}},"programmordner":["{}"],"schluessel":{{"lokal-beispiel":"{}","cloud-beispiel":"{}"}},"ca_zulassung":"offen"}}"#,
        t("einst/e.json"),
        t("vault"),
        t("protokoll"),
        t("zustand"),
        t("laufzeit"),
        t("sitzungen"),
        t("heim"),
        t("bin/rangkern"),
        t("bin/modellweg"),
        t("bin/kind"),
        t("bin/pi"),
        t("bin/copilot"),
        t("bin"),
        t("k/lokal"),
        t("k/cloud"),
    );
    let pfad = w.join("lauf.json");
    schreiben(&pfad, &konfig, 0o600);
    pfad
}

/// The text of a start file of the world `w`.
fn start_text(w: &Path, harness: &str, route: &str, marker: Option<&Path>) -> String {
    let marker = marker.map_or_else(String::new, |m| {
        format!(
            r#","marker":{{"liste":["M-1","M-2"],"ordner":"{}"}}"#,
            m.display()
        )
    });
    let modell = if harness == "copilot" {
        "claude-sonnet-5.5"
    } else {
        "qwen3:8b"
    };
    format!(
        r#"{{"fassung":1,"sitzung":"s-1","harness":"{harness}","modell":"{modell}","route":"{route}","kunde":"k-beispiel","arbeitsordner":"{}"{marker}}}"#,
        w.join("arbeit").display()
    )
}

/// A checked start of the world `w`, built without the checks of steps 2 to 5 (those have tests of their own below).
fn lage(w: &Path, harness: &str, route: &str, marker: Option<&Path>) -> Lage {
    let konfig = crate::konfig::lesen_datei(&welt(w)).expect("configuration");
    Lage {
        start: start::lesen_datei(start_text(w, harness, route, marker).as_bytes())
            .expect("start file"),
        einst: einstellungen::lesen(EINST.as_bytes()).expect("settings"),
        konfig,
        startordner: w.join("start"),
        ordner: w.join("arbeit"),
    }
}

#[cfg(target_os = "linux")]
fn route<'a>(l: &'a Lage, name: &str) -> &'a Route {
    l.einst.route(name).expect("route")
}

fn strings(w: &[&str]) -> Vec<String> {
    w.iter().map(|s| (*s).to_owned()).collect()
}

/// A child through the stand-in trampoline: `sh -c <skript>`.
fn kind_von(l: &Lage, skript: &str) -> Kind {
    kind::starten(
        &l.konfig.programme.kind,
        Path::new("/bin/sh"),
        &strings(&["-c", skript]),
        BEREIT_FRIST,
    )
    .expect("child")
}

fn lebt(pid: u32) -> bool {
    std::fs::read_to_string(format!("/proc/{pid}/stat")).is_ok_and(|s| {
        s.rsplit_once(") ")
            .is_some_and(|(_, r)| !r.starts_with('Z'))
    })
}

const SCHLAEFT: &str = "printf '{\"bereit\":true}\\n'; exec /bin/sleep 30";

#[test]
fn a_program_is_executable_when_it_is_a_file_with_one_execute_bit() {
    let o = crate::testhilfe::Ordner::neu("ausfuehrbar");
    let p = o.0.join("p");
    for (modus, gut) in [
        (0o644, false),
        (0o600, false),
        (0o700, true),
        (0o610, true),
        (0o601, true),
        (0o111, true),
    ] {
        schreiben(&p, "x", modus);
        assert_eq!(ausfuehrbar(&p), gut, "{modus:o}");
    }
    assert!(!ausfuehrbar(&o.0), "a folder");
    assert!(!ausfuehrbar(&o.0.join("fehlt")));
}

#[test]
fn the_program_of_step_6_is_the_starter_or_in_the_test_build_bwrap() {
    let o = crate::testhilfe::Ordner::neu("programm");
    let l = lage(&o.0, "pi", "lokal-beispiel", None);
    let erwartet = if cfg!(feature = "pruefbau") {
        PathBuf::from(einschluss::BWRAP_PFAD)
    } else {
        PathBuf::from(einschluss::starter::STARTER_PFAD)
    };
    assert_eq!(einschluss_programm(&l.konfig), erwartet);
}

#[test]
fn the_socket_folder_is_made_once_and_its_longest_path_fits_sun_path() {
    let o = crate::testhilfe::Ordner::neu("sockel");
    let mut l = lage(&o.0, "pi", "lokal-beispiel", None);
    let s = socket_ordner(&l).expect("socket folder");
    assert_eq!(s, o.0.join("laufzeit/s-1"));
    let st = std::fs::symlink_metadata(&s).expect("made");
    assert!(st.is_dir());
    assert_eq!(socket_ordner(&l), Err(Fehler::SucheFehlt), "made once");
    // a run folder whose longest socket path has exactly the limit, and one byte more
    let rest = "/s-1/modell.sock".len();
    for (extra, gut) in [(0, true), (1, false)] {
        let basis = o.0.join("l");
        let n = SOCKET_MAX + extra - rest - basis.as_os_str().len();
        let laufzeit = PathBuf::from(format!("{}{}", basis.display(), "x".repeat(n)));
        std::fs::create_dir_all(&laufzeit).expect("run folder");
        l.konfig.laufzeit.clone_from(&laufzeit);
        assert_eq!(
            laufzeit.join("s-1/modell.sock").as_os_str().len(),
            SOCKET_MAX + extra
        );
        assert_eq!(socket_ordner(&l).is_ok(), gut, "{extra}");
    }
}

#[test]
fn the_arguments_of_the_search_process_and_of_the_model_path() {
    let basis = crate::testhilfe::Ordner::neu("argumente");
    let w = &basis.0;
    let pfad = |p: &str| w.join(p).display().to_string();
    let l = lage(w, "pi", "lokal-beispiel", None);
    let sock = w.join("s.sock");
    assert_eq!(
        such_argumente(&l, &sock),
        vec![
            "sitzung".to_owned(),
            pfad("vault"),
            "--socket".to_owned(),
            pfad("s.sock"),
            "--route".to_owned(),
            "lokal-beispiel".to_owned(),
            "--sitzung".to_owned(),
            "s-1".to_owned(),
            "--einstellungen".to_owned(),
            pfad("einst/e.json"),
            "--protokoll".to_owned(),
            pfad("protokoll"),
            "--socket-rechte".to_owned(),
            "0600".to_owned(),
        ]
    );
    assert_eq!(
        modellweg_basis(&l, "openai-lokal", &sock),
        vec![
            "--betriebsart".to_owned(),
            "openai-lokal".to_owned(),
            "--socket".to_owned(),
            pfad("s.sock"),
            "--socket-rechte".to_owned(),
            "0600".to_owned(),
            "--sitzung".to_owned(),
            "s-1".to_owned(),
            "--route".to_owned(),
            "lokal-beispiel".to_owned(),
            "--protokoll".to_owned(),
            pfad("protokoll"),
        ]
    );
    assert_eq!(marker_optionen(&l.start, "suche"), Vec::<String>::new());
    let z = w.join("zaehl");
    let mit = lage(w, "pi", "lokal-beispiel", Some(&z));
    let optionen = vec![
        "--probelauf".to_owned(),
        "--marker".to_owned(),
        "M-1,M-2".to_owned(),
        "--marker-zaehlung".to_owned(),
        format!("{}/s-1.suche.json", z.display()),
    ];
    assert_eq!(marker_optionen(&mit.start, "suche"), optionen);
    let such = such_argumente(&mit, &sock);
    assert_eq!(such[14..], optionen[..]);
    assert_eq!(text(Path::new("/srv/beispiel/a b")), "/srv/beispiel/a b");
}

// the CA of the cloud route exists on Linux only
#[cfg(target_os = "linux")]
#[test]
fn the_cloud_route_writes_its_target_list_and_ca_and_names_them() {
    let o = crate::testhilfe::Ordner::neu("cloud");
    let w = &o.0;
    let l = lage(w, "pi", "cloud-beispiel", None);
    let sock = w.join("m.sock");
    let mut spuren = Spuren::default();
    let (args, cloud) =
        cloud_vorbereiten(&l, route(&l, "cloud-beispiel"), &sock, &mut spuren).expect("cloud");
    let liste = w.join("zustand/sitzungen/s-1.zielliste.json");
    let mut erwartet = modellweg_basis(&l, "kopf-einsetzen", &sock);
    erwartet.extend([
        "--zielliste".to_owned(),
        liste.display().to_string(),
        "--ca-schluessel".to_owned(),
        w.join("start/ca.key").display().to_string(),
        "--ca-zertifikat".to_owned(),
        w.join("start/ca.crt").display().to_string(),
    ]);
    assert_eq!(args, erwartet);
    assert!(
        cloud.is_none(),
        "no bundle for a harness other than copilot"
    );
    assert!(liste.is_file());
    assert_eq!(spuren.0, [w.join("start/ca.key"), w.join("start/ca.crt")]);
    spuren.loeschen();
    // the harness copilot gets the bundle of the enclosure: the host bundle with the certificate of the CA
    let o2 = crate::testhilfe::Ordner::neu("cloud-copilot");
    let l = lage(&o2.0, "copilot", "cloud-beispiel", None);
    let mut spuren = Spuren::default();
    let r = cloud_vorbereiten(&l, route(&l, "cloud-beispiel"), &sock, &mut spuren);
    if einschluss::Verteilung::vom_host().is_ok() {
        let (_, cloud) = r.expect("cloud");
        let bundle = cloud.expect("bundle").bundle;
        assert_eq!(bundle, o2.0.join("start/ca-bundle.pem"));
        let ca = std::fs::read(o2.0.join("start/ca.crt")).expect("ca");
        assert!(std::fs::read(&bundle).expect("bundle").ends_with(&ca));
    } else {
        assert_eq!(r.map(|_| ()), Err(Fehler::EinschlussFehlt));
    }
    spuren.loeschen();
}

// the CA of the cloud route exists on Linux only
#[cfg(target_os = "linux")]
#[test]
fn the_cloud_route_refuses_a_missing_key_a_foreign_state_folder_and_a_taken_list() {
    let o = crate::testhilfe::Ordner::neu("cloud-nein");
    let w = &o.0;
    let sock = w.join("m.sock");
    // a bound target needs the key of the route in the configuration
    let mut l = lage(w, "pi", "cloud-beispiel", None);
    l.konfig.schluessel.remove("cloud-beispiel");
    let mut spuren = Spuren::default();
    assert_eq!(
        cloud_vorbereiten(&l, route(&l, "cloud-beispiel"), &sock, &mut spuren).map(|_| ()),
        Err(Fehler::ModellwegFehlt)
    );
    // the folder of the lists must be an own folder 0700
    let l = lage(w, "pi", "cloud-beispiel", None);
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(
            w.join("zustand/sitzungen"),
            std::fs::Permissions::from_mode(0o755),
        )
        .expect("mode");
    }
    assert_eq!(
        cloud_vorbereiten(&l, route(&l, "cloud-beispiel"), &sock, &mut spuren).map(|_| ()),
        Err(Fehler::Zielliste)
    );
    ordner_0700(&w.join("zustand/sitzungen"));
    // a list of that session exists already
    schreiben(&w.join("zustand/sitzungen/s-1.zielliste.json"), "[]", 0o600);
    assert_eq!(
        cloud_vorbereiten(&l, route(&l, "cloud-beispiel"), &sock, &mut spuren).map(|_| ()),
        Err(Fehler::Zielliste)
    );
    assert_eq!(spuren.0, Vec::<PathBuf>::new());
}

/// Settings with one cloud route of `n` unbound targets.
#[cfg(target_os = "linux")]
fn einst_mit_zielen(n: usize) -> Einstellungen {
    let ziele: Vec<String> = (0..n)
        .map(|i| {
            format!(
                r#"{{"host":"z{i}.cloud.example","port":443,"zugriff":"kopf-einsetzen","schluessel":false,"pfade":[{{"methode":"GET","pfad":"/a"}}]}}"#
            )
        })
        .collect();
    let text = format!(
        r#"{{"fassung":2,"klassen":["oeffentlich","intern","kundendaten"],"vorgabestufe":"kundendaten","routen":[{{"schluessel":"cloud-beispiel","art":"cloud","klasse":"intern","netzziele":[{}]}}],"kunden":[],"kundendaten_an_cloud":{{"vorgabe":"nie","freigaben":[]}}}}"#,
        ziele.join(",")
    );
    einstellungen::lesen(text.as_bytes()).expect("settings")
}

// the CA of the cloud route exists on Linux only
#[cfg(target_os = "linux")]
#[test]
fn the_target_list_takes_one_to_the_limit_of_targets() {
    for (n, gut) in [
        (0, false),
        (1, true),
        (crate::ziele::ZIELE_MAX, true),
        (crate::ziele::ZIELE_MAX + 1, false),
    ] {
        let o = crate::testhilfe::Ordner::neu("ziele");
        let mut l = lage(&o.0, "pi", "cloud-beispiel", None);
        let Ok(einst) = std::panic::catch_unwind(|| einst_mit_zielen(n)) else {
            // the settings themselves refuse this count: nothing for step 7a to decide
            continue;
        };
        l.einst = einst;
        let mut spuren = Spuren::default();
        let r = cloud_vorbereiten(
            &l,
            route(&l, "cloud-beispiel"),
            &o.0.join("m.sock"),
            &mut spuren,
        );
        if gut {
            assert!(r.is_ok(), "{n}");
        } else {
            assert_eq!(r.map(|_| ()), Err(Fehler::Zielliste), "{n}");
        }
        spuren.loeschen();
    }
}

#[test]
fn the_pid_files_the_known_children_and_the_end_of_the_children() {
    let o = crate::testhilfe::Ordner::neu("kinder");
    let l = lage(&o.0, "pi", "lokal-beispiel", None);
    let mut kinder = Kinder::default();
    assert_eq!(
        pids_schreiben(&l, &mut kinder),
        Err(Fehler::Intern),
        "no children yet"
    );
    kinder.suche = Some(kind_von(&l, SCHLAEFT));
    assert_eq!(
        pids_schreiben(&l, &mut kinder),
        Err(Fehler::Intern),
        "the model path is missing"
    );
    kinder.spuren.loeschen();
    kinder.modellweg = Some(kind_von(&l, SCHLAEFT));
    let s = kinder.suche.as_ref().map(Kind::pid).expect("search");
    let m = kinder
        .modellweg
        .as_ref()
        .map(Kind::pid)
        .expect("model path");
    pids_schreiben(&l, &mut kinder).expect("pid files");
    assert_eq!(
        std::fs::read_to_string(o.0.join("start/suche.pid")).expect("read"),
        format!("{s}\n")
    );
    assert_eq!(
        std::fs::read_to_string(o.0.join("start/modellweg.pid")).expect("read"),
        format!("{m}\n")
    );
    assert_eq!(kinder.spuren.0.len(), 2);
    kinder.wache = Some(77);
    assert_eq!(bekannte(&mut kinder), vec![s, m, 77]);
    kinder.beenden();
    assert!(kinder.suche.is_none() && kinder.modellweg.is_none());
    assert!(!lebt(s) && !lebt(m));
    kinder.spuren.loeschen();
}

#[cfg(target_os = "linux")]
#[test]
fn the_watcher_starts_with_both_children_and_a_session_folder() {
    let o = crate::testhilfe::Ordner::neu("wache");
    let l = lage(&o.0, "pi", "lokal-beispiel", None);
    let sockel = o.0.join("laufzeit/s-1");
    let mut kinder = Kinder::default();
    assert_eq!(wache_starten(&l, &sockel, &kinder), Err(Fehler::Intern));
    kinder.suche = Some(kind_von(&l, SCHLAEFT));
    kinder.modellweg = Some(kind_von(&l, SCHLAEFT));
    assert_eq!(
        wache_starten(&l, &sockel, &kinder),
        Err(Fehler::Intern),
        "no session folder yet"
    );
    kinder.sitzung = Some(o.0.join("sitzungen/s-1"));
    let w = wache_starten(&l, &sockel, &kinder).expect("watcher");
    // the watcher is a child of this process (the stand-in replaces itself by `sleep`, so its name says nothing)
    let stat = std::fs::read_to_string(format!("/proc/{w}/stat")).expect("watcher runs");
    let eltern = stat
        .rsplit_once(") ")
        .and_then(|(_, r)| r.split_whitespace().nth(1))
        .expect("ppid");
    assert_eq!(eltern, std::process::id().to_string());
    let _ = std::process::Command::new("kill")
        .arg(w.to_string())
        .status();
    kinder.beenden();
}

#[test]
fn the_description_names_the_harness_its_folders_and_the_exit() {
    let basis = crate::testhilfe::Ordner::neu("beschreibung");
    let w = &basis.0;
    let l = lage(w, "pi", "lokal-beispiel", None);
    let sockel = w.join("laufzeit/s-1");
    let umgebung = Umgebung {
        namen: Vec::new(),
        term: Some("xterm".to_owned()),
        xdg_cache: None,
    };
    let mut angelegt = None;
    let b = beschreibung_bauen(&l, &umgebung, &sockel, None, None, None, &mut angelegt)
        .expect("description");
    assert_eq!(angelegt, Some(w.join("sitzungen/s-1")));
    assert!(w.join("sitzungen/s-1/zustand").is_dir());
    assert_eq!(b.harness, "pi");
    assert_eq!(b.arbeitsordner, w.join("arbeit"));
    assert_eq!(b.such_socket, sockel.join("such.sock"));
    assert_eq!(b.modell_socket, Some(sockel.join("modell.sock")));
    assert_eq!(
        b.lokal_weiter,
        Some(l.konfig.programme.weiter.clone()),
        "the local route has the forwarder"
    );
    assert_eq!(b.cloud, None);
    assert_eq!(b.umgebung.term, "xterm");
    // a session that ran before: its folder exists
    let mut zweites = None;
    assert_eq!(
        beschreibung_bauen(&l, &umgebung, &sockel, None, None, None, &mut zweites).map(|_| ()),
        Err(Fehler::SitzungBelegt)
    );
    assert_eq!(zweites, None, "a refused name is never recorded");
    // the cloud route: no forwarder of the local route; with the bundle and the distribution the cloud exit
    let o2 = crate::testhilfe::Ordner::neu("beschreibung-cloud");
    let l = lage(&o2.0, "copilot", "cloud-beispiel", None);
    let teile = CloudTeile {
        bundle: o2.0.join("start/ca-bundle.pem"),
    };
    let mut angelegt = None;
    let b = beschreibung_bauen(
        &l,
        &umgebung,
        &sockel,
        Some(&teile),
        Some(einschluss::Verteilung::Ubuntu),
        None,
        &mut angelegt,
    )
    .expect("description");
    assert_eq!(b.lokal_weiter, None);
    assert_eq!(b.modell_socket, None, "the cloud exit takes the socket");
    let cloud = b.cloud.expect("cloud exit");
    assert_eq!(cloud.bundle, teile.bundle);
    assert_eq!(cloud.weiterreicher, sockel.join("modell.sock"));
    // a harness program that is not executable
    let o3 = crate::testhilfe::Ordner::neu("beschreibung-nein");
    let l = lage(&o3.0, "pi", "lokal-beispiel", None);
    schreiben(&o3.0.join("bin/pi"), "x", 0o644);
    let mut angelegt = None;
    assert_eq!(
        beschreibung_bauen(&l, &umgebung, &sockel, None, None, None, &mut angelegt).map(|_| ()),
        Err(Fehler::HarnessFehlt)
    );
}

#[test]
fn step_6a_is_for_the_harness_copilot_alone() {
    let o = crate::testhilfe::Ordner::neu("paket");
    let u = Umgebung {
        namen: Vec::new(),
        term: None,
        xdg_cache: None,
    };
    let mut platz = Platz::reservieren();
    // another harness: nothing to extract, even with an executable copilot in the configuration
    let l = lage(&o.0, "pi", "lokal-beispiel", None);
    assert_eq!(
        paket_bereitstellen(&l, &u, &mut platz).map(|p| p.is_none()),
        Ok(true)
    );
    // copilot with a program that is no package: the form of step 6a refuses it
    let l = lage(&o.0, "copilot", "cloud-beispiel", None);
    assert!(paket_bereitstellen(&l, &u, &mut platz).is_err());
    // copilot without an executable program: left to step 8
    schreiben(&o.0.join("bin/copilot"), "x", 0o644);
    assert_eq!(
        paket_bereitstellen(&l, &u, &mut platz).map(|p| p.is_none()),
        Ok(true)
    );
}

#[test]
fn steps_1_to_5c_pass_a_valid_start_and_refuse_a_missing_folder() {
    for (harness, r) in [("pi", "lokal-beispiel"), ("copilot", "cloud-beispiel")] {
        let o = crate::testhilfe::Ordner::neu("pruefen");
        let w = &o.0;
        let konfig = welt(w);
        let start = w.join("start/start.json");
        schreiben(&start, &start_text(w, harness, r, None), 0o600);
        let args = [start.clone().into_os_string()];
        let l = pruefen_mit(&args, || Some(konfig.clone()), || Ok(vorgabe_gleich()))
            .unwrap_or_else(|f| panic!("{harness}: {f:?}"));
        assert_eq!(l.ordner, w.join("arbeit"));
        assert_eq!(l.startordner, w.join("start"));
        assert_eq!(l.start.route(), r);
        // the folder is gone (the mark of the first check stays; a new folder of that name is not checked here)
        std::fs::remove_dir_all(w.join("arbeit")).expect("remove");
        assert_eq!(
            pruefen_mit(&args, || Some(konfig.clone()), || Ok(vorgabe_gleich())).map(|_| ()),
            Err(Fehler::OrdnerFehlt),
            "{harness}"
        );
        // no configuration
        assert_eq!(
            pruefen_mit(&args, || None, || Ok(vorgabe_gleich())).map(|_| ()),
            Err(Fehler::KonfigFehlt)
        );
    }
}

/// The default of the firm equal to the settings of the world.
fn vorgabe_gleich() -> Einstellungen {
    einstellungen::lesen(EINST.as_bytes()).expect("settings")
}

#[test]
fn step_4b_stops_a_cloud_route_without_a_safe_default_or_with_an_added_target() {
    let o = crate::testhilfe::Ordner::neu("vorgabe");
    let w = &o.0;
    let konfig = welt(w);
    let start = w.join("start/start.json");
    schreiben(
        &start,
        &start_text(w, "copilot", "cloud-beispiel", None),
        0o600,
    );
    let args = [start.clone().into_os_string()];
    let lauf = |v: Result<Einstellungen, Fehler>| {
        pruefen_mit(&args, || Some(konfig.clone()), || v).map(|_| ())
    };
    // a fault of the default stops the start with its own code: never the settings of the user in its place
    for f in [
        Fehler::VorgabeFehlt,
        Fehler::VorgabeUnsicher,
        Fehler::VorgabeKaputt,
    ] {
        assert_eq!(lauf(Err(f.clone())), Err(f));
    }
    // a default without the telemetry target: the user's file adds it
    let ohne = EINST.replacen(
        r#",{"host":"telemetry.cloud.example","port":443,"zugriff":"kopf-einsetzen","schluessel":false,"pfade":[{"methode":"POST","pfad":"/telemetry"}]}"#,
        "",
        1,
    );
    assert_ne!(ohne, EINST);
    assert_eq!(
        lauf(Ok(einstellungen::lesen(ohne.as_bytes()).expect("settings"))),
        Err(Fehler::VorgabeErweitert)
    );
    // a default without the route of the user: refused, never "no default, everything goes"
    let ohne_route = EINST.replacen(
        r#""schluessel":"cloud-beispiel""#,
        r#""schluessel":"cloud-anders""#,
        1,
    );
    assert_eq!(
        lauf(Ok(
            einstellungen::lesen(ohne_route.as_bytes()).expect("settings")
        )),
        Err(Fehler::VorgabeRouteFremd)
    );
    assert_eq!(lauf(Ok(vorgabe_gleich())), Ok(()));
}

#[test]
fn step_4b_checks_the_whole_route_table_also_for_a_local_start() {
    let o = crate::testhilfe::Ordner::neu("vorgabe-lokal");
    let w = &o.0;
    let konfig = welt(w);
    let start = w.join("start/start.json");
    schreiben(&start, &start_text(w, "pi", "lokal-beispiel", None), 0o600);
    let args = [start.clone().into_os_string()];
    let lauf = |v: Result<Einstellungen, Fehler>| {
        pruefen_mit(&args, || Some(konfig.clone()), || v).map(|_| ())
    };
    assert_eq!(lauf(Err(Fehler::VorgabeFehlt)), Err(Fehler::VorgabeFehlt));
    // the default fixes another local target: the user's file must not move it
    let anderes_ziel = EINST.replacen("http://127.0.0.1:11434/v1", "http://127.0.0.1:11435/v1", 1);
    assert_ne!(anderes_ziel, EINST);
    assert_eq!(
        lauf(Ok(
            einstellungen::lesen(anderes_ziel.as_bytes()).expect("settings")
        )),
        Err(Fehler::VorgabeRouteAbweichend)
    );
    // the cloud route of the table counts as well, though the start takes the local route
    let ohne = EINST.replacen(
        r#",{"host":"telemetry.cloud.example","port":443,"zugriff":"kopf-einsetzen","schluessel":false,"pfade":[{"methode":"POST","pfad":"/telemetry"}]}"#,
        "",
        1,
    );
    assert_eq!(
        lauf(Ok(einstellungen::lesen(ohne.as_bytes()).expect("settings"))),
        Err(Fehler::VorgabeErweitert)
    );
    assert_eq!(lauf(Ok(vorgabe_gleich())), Ok(()));
}

/// The helper process of the cases below (see `crate::testhilfe`): one thread, descriptor 3 free.
#[test]
fn kind_fall() {
    #[cfg(target_os = "linux")]
    {
        let Some(fall) = crate::testhilfe::fall() else {
            return;
        };
        match fall.as_str() {
            "einschluss" => fall_einschluss(),
            andere => panic!("unknown case {andere}"),
        }
        crate::testhilfe::fertig();
    }
}

#[cfg(target_os = "linux")]
const KIND_FALL: &str = "ablauf::tests::kind_fall";

/// Step 6 with the place held and no inherited descriptor.
#[cfg(target_os = "linux")]
fn fall_einschluss() {
    let o = crate::testhilfe::Ordner::neu("einschluss");
    let platz = Platz::reservieren();
    assert!(platz.da());
    // descriptor 5 of the lifeline is the place's own
    let leine = platz.eigene();
    assert_eq!(leine.len(), 1, "{leine:?}");
    let sh = Path::new("/bin/sh");
    let lokal = lage(&o.0, "pi", "lokal-beispiel", None);
    let r = route(&lokal, "lokal-beispiel");
    assert_eq!(einschluss_pruefen(&lokal, r, sh, &platz, &leine), Ok(None));
    assert_eq!(
        einschluss_pruefen(&lokal, r, &o.0.join("fehlt"), &platz, &leine),
        Err(Fehler::EinschlussFehlt),
        "no program"
    );
    // a descriptor of the report is the program's own; any other is inherited
    let eigen = std::fs::File::open("/dev/null").expect("null");
    let n = std::os::fd::AsRawFd::as_raw_fd(&eigen);
    assert_eq!(
        einschluss_pruefen(&lokal, r, sh, &platz, &[leine.clone(), vec![n]].concat()),
        Ok(None)
    );
    assert_eq!(
        einschluss_pruefen(&lokal, r, sh, &platz, &leine),
        Err(Fehler::EinschlussFehlt)
    );
    drop(eigen);
    // the harness copilot on the local route, and another harness on the cloud route: no distribution
    let o2 = crate::testhilfe::Ordner::neu("einschluss-2");
    let copilot_lokal = lage(&o2.0, "copilot", "lokal-beispiel", None);
    assert_eq!(
        einschluss_pruefen(&copilot_lokal, r, sh, &platz, &leine),
        Ok(None)
    );
    let o3 = crate::testhilfe::Ordner::neu("einschluss-3");
    let pi_cloud = lage(&o3.0, "pi", "cloud-beispiel", None);
    let c = route(&pi_cloud, "cloud-beispiel");
    assert_eq!(
        einschluss_pruefen(&pi_cloud, c, sh, &platz, &leine),
        Ok(None)
    );
    // copilot on the cloud route: the distribution of the host
    let o4 = crate::testhilfe::Ordner::neu("einschluss-4");
    let copilot_cloud = lage(&o4.0, "copilot", "cloud-beispiel", None);
    let c = route(&copilot_cloud, "cloud-beispiel");
    assert_eq!(
        einschluss_pruefen(&copilot_cloud, c, sh, &platz, &leine),
        einschluss::Verteilung::vom_host()
            .map(Some)
            .map_err(|_| Fehler::EinschlussFehlt)
    );
    // without the place the delivered build has no enclosure
    if !cfg!(feature = "pruefbau") {
        drop(platz);
        // descriptor 3 taken by another file: the reservation keeps nothing
        let drei = std::fs::File::open("/dev/null").expect("null");
        let ohne = Platz::reservieren();
        assert!(!ohne.da());
        assert_eq!(
            einschluss_pruefen(&lokal, r, sh, &ohne, &ohne.eigene()),
            Err(Fehler::EinschlussFehlt)
        );
        drop(drei);
    }
}

/// Step 7a: the model path of each kind of route, and the search process (the fork condition is checked once before
/// both, in `starten`).
#[cfg(target_os = "linux")]
#[test]
fn step_7a_starts_the_model_path_of_each_kind_of_route() {
    let basis = crate::testhilfe::Ordner::neu("modellweg");
    let w = &basis.0;
    let sock = w.join("m.sock");
    let l = lage(w, "pi", "lokal-beispiel", None);
    let mut spuren = Spuren::default();
    let (k, cloud) =
        modellweg_starten(&l, route(&l, "lokal-beispiel"), &sock, &mut spuren).expect("local");
    assert!(cloud.is_none());
    let args = std::fs::read_to_string(w.join("bin/modellweg.args")).expect("arguments");
    let mut erwartet = modellweg_basis(&l, "openai-lokal", &sock);
    erwartet.extend(strings(&[
        "--ziel",
        "http://127.0.0.1:11434/v1",
        "--modell",
        "qwen3:8b",
        "--schluessel-datei",
    ]));
    erwartet.push(w.join("k/lokal").display().to_string());
    assert_eq!(args.lines().collect::<Vec<_>>(), erwartet);
    k.beenden();
    let basis2 = crate::testhilfe::Ordner::neu("modellweg-cloud");
    let l = lage(&basis2.0, "pi", "cloud-beispiel", None);
    let (k, cloud) =
        modellweg_starten(&l, route(&l, "cloud-beispiel"), &sock, &mut spuren).expect("cloud");
    assert!(cloud.is_none());
    let args = std::fs::read_to_string(basis2.0.join("bin/modellweg.args")).expect("arguments");
    assert!(args.lines().any(|z| z == "kopf-einsetzen"), "{args}");
    k.beenden();
    spuren.loeschen();
    // the search process too
    let suche = such_starten(&l, &basis2.0.join("laufzeit")).expect("search process");
    let args = std::fs::read_to_string(basis2.0.join("bin/rangkern.args")).expect("arguments");
    assert_eq!(args.lines().next(), Some("sitzung"));
    suche.beenden();
}

#[cfg(target_os = "linux")]
#[test]
fn step_6_takes_the_program_the_place_the_descriptors_and_the_distribution() {
    crate::testhilfe::helfer_gut(KIND_FALL, "einschluss", "3>&-");
}
