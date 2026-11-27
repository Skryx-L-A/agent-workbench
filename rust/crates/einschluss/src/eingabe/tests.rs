//! Tests of the text form: the round trip, and one case for every refused input.

use super::*;

fn muster(cloud: bool) -> Beschreibung {
    Beschreibung {
        harness: if cloud { "copilot" } else { "claude" }.to_owned(),
        arbeitsordner: PathBuf::from("/srv/beispiel/arbeit"),
        programmordner: vec![
            PathBuf::from("/srv/beispiel/bin"),
            PathBuf::from("/srv/beispiel/lib"),
        ],
        zustand: Zustand {
            host: PathBuf::from("/srv/beispiel/zustand"),
            name: ".claude".to_owned(),
            fortsetzen: false,
            gespraech: None,
        },
        heim: PathBuf::from("/srv/beispiel/heim"),
        such_socket: PathBuf::from("/srv/beispiel/s/such.sock"),
        modell_socket: (!cloud).then(|| PathBuf::from("/srv/beispiel/s/modell.sock")),
        lokal_weiter: None,
        dateien: Vec::new(),
        cloud: cloud.then(|| Cloud {
            weiterreicher: PathBuf::from("/srv/beispiel/s/weiter.sock"),
            weiter_programm: PathBuf::from("/srv/beispiel/weiter"),
            bundle: PathBuf::from("/srv/beispiel/bundle.pem"),
            verteilung: Verteilung::Arch,
        }),
        paket: None,
        umgebung: Umgebung {
            home: "/srv/beispiel/heim".to_owned(),
            path: "/usr/bin".to_owned(),
            term: "xterm".to_owned(),
            lang: "C.UTF-8".to_owned(),
            tmpdir: "/tmp".to_owned(),
        },
        geschuetzt: vec![PathBuf::from("/srv/beispiel/vault")],
        argv: vec!["/srv/beispiel/bin/h".to_owned(), "--x \"y\"\n".to_owned()],
        system: System::usr_merge(1000),
    }
}

fn lesen_mit(text: &str) -> Option<Beschreibung> {
    beschreibung_lesen(text.as_bytes(), System::usr_merge(1000))
}

#[test]
fn the_round_trip_keeps_every_field() {
    for cloud in [false, true] {
        let b = muster(cloud);
        let text = beschreibung_schreiben(&b);
        assert_eq!(lesen_mit(&text), Some(b), "{text}");
    }
}

#[test]
fn the_ubuntu_distribution_round_trips_too() {
    let mut b = muster(true);
    if let Some(c) = b.cloud.as_mut() {
        c.verteilung = Verteilung::Ubuntu;
    }
    let text = beschreibung_schreiben(&b);
    assert!(text.contains("\"ubuntu\""), "{text}");
    assert_eq!(lesen_mit(&text), Some(b));
}

#[test]
fn the_host_part_comes_from_the_caller_and_is_not_in_the_text() {
    let text = beschreibung_schreiben(&muster(false));
    assert!(!text.contains("system"), "{text}");
    let b = beschreibung_lesen(text.as_bytes(), System::usr_merge(42)).expect("reads");
    assert_eq!(b.system.uid, 42);
}

/// The text with one change: `ersetze` text of the valid base by `durch`.
fn geaendert(ersetze: &str, durch: &str) -> String {
    let basis = beschreibung_schreiben(&muster(false));
    assert!(basis.contains(ersetze), "base lacks {ersetze}: {basis}");
    basis.replacen(ersetze, durch, 1)
}

#[test]
fn not_json_and_wrong_top_level_are_refused() {
    assert_eq!(lesen_mit(""), None);
    assert_eq!(lesen_mit("nicht json"), None);
    assert_eq!(lesen_mit("null"), None);
    assert_eq!(lesen_mit("[]"), None);
    assert_eq!(lesen_mit("\"x\""), None);
    let basis = beschreibung_schreiben(&muster(false));
    assert_eq!(lesen_mit(&format!("{basis} x")), None);
    assert_eq!(lesen_mit(&format!("{basis}{basis}")), None);
    assert_eq!(lesen_mit(&basis[..basis.len() - 1]), None);
}

#[test]
fn an_unknown_key_is_refused_at_every_level() {
    assert_eq!(
        lesen_mit(&geaendert("{\"harness\"", "{\"extra\":1,\"harness\"")),
        None
    );
    assert_eq!(
        lesen_mit(&geaendert("\"name\":", "\"extra\":\"x\",\"name\":")),
        None
    );
    assert_eq!(
        lesen_mit(&geaendert("\"home\":", "\"extra\":\"x\",\"home\":")),
        None
    );
    let wolke = beschreibung_schreiben(&muster(true));
    let mit_extra = wolke.replacen("\"bundle\":", "\"extra\":\"x\",\"bundle\":", 1);
    assert_eq!(lesen_mit(&mit_extra), None);
}

#[test]
fn the_host_field_system_is_an_unknown_key() {
    let text = geaendert(
        "{\"harness\"",
        "{\"system\":{\"uid\":0,\"symlinks\":[],\"ro_binds\":[]},\"harness\"",
    );
    assert_eq!(lesen_mit(&text), None);
}

#[test]
fn a_missing_field_is_refused_for_every_field() {
    let namen = [
        "harness",
        "arbeitsordner",
        "programmordner",
        "zustand",
        "heim",
        "such_socket",
        "modell_socket",
        "lokal_weiter",
        "cloud",
        "dateien",
        "umgebung",
        "geschuetzt",
        "argv",
    ];
    for name in namen {
        let basis = beschreibung_schreiben(&muster(false));
        let schluessel = format!("\"{name}\":");
        assert!(basis.contains(&schluessel), "{name}");
        // the key renamed to another one leaves the field missing and adds an unknown key; the check below removes it
        let ohne = basis.replacen(&schluessel, "\"zzz\":", 1);
        assert_eq!(lesen_mit(&ohne), None, "{name}");
    }
}

#[test]
fn a_missing_field_in_a_sub_object_is_refused() {
    let host = "\"host\":\"/srv/beispiel/zustand\",";
    assert_eq!(lesen_mit(&geaendert(host, "")), None);
    assert_eq!(lesen_mit(&geaendert(",\"name\":\".claude\"", "")), None);
    assert_eq!(
        lesen_mit(&geaendert("\"home\":\"/srv/beispiel/heim\",", "")),
        None
    );
    assert_eq!(lesen_mit(&geaendert(",\"tmpdir\":\"/tmp\"", "")), None);
    assert_eq!(lesen_mit(&geaendert("\"path\":\"/usr/bin\",", "")), None);
    assert_eq!(lesen_mit(&geaendert("\"term\":\"xterm\",", "")), None);
    assert_eq!(lesen_mit(&geaendert("\"lang\":\"C.UTF-8\",", "")), None);
    let wolke = beschreibung_schreiben(&muster(true));
    for teil in [
        "\"weiterreicher\":\"/srv/beispiel/s/weiter.sock\",",
        "\"weiter_programm\":\"/srv/beispiel/weiter\",",
        "\"bundle\":\"/srv/beispiel/bundle.pem\",",
        ",\"verteilung\":\"arch\"",
    ] {
        assert!(wolke.contains(teil), "{teil}");
        assert_eq!(lesen_mit(&wolke.replacen(teil, "", 1)), None, "{teil}");
    }
}

#[test]
fn a_wrong_type_is_refused_for_every_field() {
    let faelle = [
        ("\"harness\":\"claude\"", "\"harness\":1"),
        (
            "\"arbeitsordner\":\"/srv/beispiel/arbeit\"",
            "\"arbeitsordner\":null",
        ),
        (
            "\"programmordner\":[\"/srv/beispiel/bin\",\"/srv/beispiel/lib\"]",
            "\"programmordner\":\"/srv/beispiel/bin\"",
        ),
        (
            "\"programmordner\":[\"/srv/beispiel/bin\",\"/srv/beispiel/lib\"]",
            "\"programmordner\":[\"/srv/beispiel/bin\",1]",
        ),
        ("\"zustand\":{", "\"zustand\":[],\"x\":{"),
        ("\"host\":\"/srv/beispiel/zustand\"", "\"host\":2"),
        ("\"name\":\".claude\"", "\"name\":[]"),
        ("\"heim\":\"/srv/beispiel/heim\"", "\"heim\":{}"),
        (
            "\"such_socket\":\"/srv/beispiel/s/such.sock\"",
            "\"such_socket\":3",
        ),
        (
            "\"modell_socket\":\"/srv/beispiel/s/modell.sock\"",
            "\"modell_socket\":4",
        ),
        ("\"cloud\":null", "\"cloud\":[]"),
        ("\"cloud\":null", "\"cloud\":\"x\""),
        ("\"home\":\"/srv/beispiel/heim\"", "\"home\":5"),
        ("\"path\":\"/usr/bin\"", "\"path\":6"),
        ("\"term\":\"xterm\"", "\"term\":7"),
        ("\"lang\":\"C.UTF-8\"", "\"lang\":8"),
        ("\"tmpdir\":\"/tmp\"", "\"tmpdir\":9"),
        (
            "\"geschuetzt\":[\"/srv/beispiel/vault\"]",
            "\"geschuetzt\":[null]",
        ),
        (
            "\"geschuetzt\":[\"/srv/beispiel/vault\"]",
            "\"geschuetzt\":\"x\"",
        ),
        ("\"argv\":[", "\"argv\":[1,"),
        ("\"umgebung\":{", "\"umgebung\":[],\"x\":{"),
    ];
    for (alt, neu) in faelle {
        assert_eq!(lesen_mit(&geaendert(alt, neu)), None, "{alt} -> {neu}");
    }
}

#[test]
fn a_wrong_type_in_the_cloud_object_is_refused() {
    let wolke = beschreibung_schreiben(&muster(true));
    for (alt, neu) in [
        (
            "\"weiterreicher\":\"/srv/beispiel/s/weiter.sock\"",
            "\"weiterreicher\":1",
        ),
        (
            "\"weiter_programm\":\"/srv/beispiel/weiter\"",
            "\"weiter_programm\":1",
        ),
        ("\"bundle\":\"/srv/beispiel/bundle.pem\"", "\"bundle\":1"),
        ("\"verteilung\":\"arch\"", "\"verteilung\":1"),
        ("\"verteilung\":\"arch\"", "\"verteilung\":\"fedora\""),
        ("\"verteilung\":\"arch\"", "\"verteilung\":\"Arch\""),
    ] {
        assert!(wolke.contains(alt), "{alt}");
        assert_eq!(lesen_mit(&wolke.replacen(alt, neu, 1)), None, "{alt}");
    }
}

#[test]
fn a_key_twice_is_refused() {
    assert_eq!(
        lesen_mit(&geaendert(
            "{\"harness\":\"claude\"",
            "{\"harness\":\"claude\",\"harness\":\"claude\""
        )),
        None
    );
}

#[test]
fn the_size_limit_is_checked_before_the_reading() {
    let basis = beschreibung_schreiben(&muster(false));
    assert!(basis.len() < EINGABE_MAX);
    // a valid text padded with white space: at the limit it reads, one byte more is refused
    let mut an_der_grenze = basis.clone();
    an_der_grenze.push_str(&" ".repeat(EINGABE_MAX - basis.len()));
    assert_eq!(an_der_grenze.len(), EINGABE_MAX);
    assert!(lesen_mit(&an_der_grenze).is_some());
    an_der_grenze.push(' ');
    assert_eq!(lesen_mit(&an_der_grenze), None);
}

#[test]
fn an_empty_list_and_a_null_socket_are_allowed() {
    let mut b = muster(false);
    b.programmordner.clear();
    b.geschuetzt.clear();
    b.modell_socket = None;
    let text = beschreibung_schreiben(&b);
    assert_eq!(lesen_mit(&text), Some(b));
}

fn mit_dateien() -> Beschreibung {
    let mut b = muster(false);
    b.harness = "pi".to_owned();
    b.lokal_weiter = Some(PathBuf::from("/srv/beispiel/weiter"));
    b.dateien = vec![Datei {
        quelle: PathBuf::from("/srv/beispiel/s/models.json"),
        ziel: PathBuf::from("/srv/beispiel/heim/.pi/agent/models.json"),
    }];
    b
}

#[test]
fn the_files_and_the_local_forwarder_round_trip() {
    let b = mit_dateien();
    let text = beschreibung_schreiben(&b);
    assert!(
        text.contains("\"lokal_weiter\":\"/srv/beispiel/weiter\""),
        "{text}"
    );
    assert!(
        text.contains("\"dateien\":[{\"quelle\":\"/srv/beispiel/s/models.json\",\"ziel\":"),
        "{text}"
    );
    assert_eq!(lesen_mit(&text), Some(b));
}

#[test]
fn a_wrong_file_entry_is_refused() {
    let text = beschreibung_schreiben(&mit_dateien());
    let quelle = "{\"quelle\":\"/srv/beispiel/s/models.json\",";
    for (alt, neu) in [
        (quelle, "{"),
        (",\"ziel\":\"/srv/beispiel/heim/.pi/agent/models.json\"", ""),
        (quelle, "{\"quelle\":1,"),
        (
            quelle,
            "{\"x\":\"y\",\"quelle\":\"/srv/beispiel/s/models.json\",",
        ),
        ("\"dateien\":[", "\"dateien\":[1,"),
        (
            "\"lokal_weiter\":\"/srv/beispiel/weiter\"",
            "\"lokal_weiter\":5",
        ),
    ] {
        assert!(text.contains(alt), "{alt}");
        assert_eq!(lesen_mit(&text.replacen(alt, neu, 1)), None, "{neu}");
    }
    let liste = text.replacen("\"dateien\":[", "\"dateien\":{\"a\":", 1);
    assert_eq!(lesen_mit(&liste), None);
}

#[test]
fn the_kind_of_the_state_folder_round_trips_and_has_two_words() {
    let mut b = muster(false);
    b.zustand.fortsetzen = true;
    let text = beschreibung_schreiben(&b);
    assert!(text.contains("\"art\":\"fortsetzen\""), "{text}");
    assert_eq!(lesen_mit(&text), Some(b.clone()));
    b.zustand.fortsetzen = false;
    let text = beschreibung_schreiben(&b);
    assert!(text.contains("\"art\":\"neu\""), "{text}");
    assert_eq!(lesen_mit(&text), Some(b));
    for falsch in [
        "\"art\":\"Neu\"",
        "\"art\":\"\"",
        "\"art\":1",
        "\"art\":null",
    ] {
        assert_eq!(
            lesen_mit(&geaendert("\"art\":\"neu\"", falsch)),
            None,
            "{falsch}"
        );
    }
    assert_eq!(lesen_mit(&geaendert(",\"art\":\"neu\"", "")), None);
}

#[test]
fn the_folder_of_the_conversation_round_trips_as_a_path_or_null() {
    let mut b = muster(false);
    let text = beschreibung_schreiben(&b);
    assert!(
        text.contains("\"art\":\"neu\",\"gespraech\":null}"),
        "{text}"
    );
    b.zustand.gespraech = Some(PathBuf::from("/srv/beispiel/gespraeche/s-1/zustand"));
    b.zustand.fortsetzen = true;
    let text = beschreibung_schreiben(&b);
    assert!(
        text.contains("\"gespraech\":\"/srv/beispiel/gespraeche/s-1/zustand\"}"),
        "{text}"
    );
    assert_eq!(lesen_mit(&text), Some(b));
    for falsch in ["\"gespraech\":1", "\"gespraech\":[]", "\"gespraech\":{}"] {
        assert_eq!(
            lesen_mit(&geaendert("\"gespraech\":null", falsch)),
            None,
            "{falsch}"
        );
    }
    assert_eq!(lesen_mit(&geaendert(",\"gespraech\":null", "")), None);
}
