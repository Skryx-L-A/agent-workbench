//! Tests of the conversation of a session: the form of `gespraech.json`, step 5d with every code and the commitments
//! to the service ("Räumen von `gespraeche/`": fixed path, no link, one conversation per session).

use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::Path;

use super::*;
use crate::testhilfe::Ordner;

const K: &str = "6f1c2e3a-4b5d-4e6f-8a7b-9c0d1e2f3a4b";

fn ordner_0700(p: &Path) {
    std::fs::create_dir_all(p).expect("folder");
    std::fs::set_permissions(p, std::fs::Permissions::from_mode(0o700)).expect("mode");
}

fn datei(p: &Path, inhalt: &str, modus: u32) {
    std::fs::write(p, inhalt).expect("file");
    std::fs::set_permissions(p, std::fs::Permissions::from_mode(modus)).expect("mode");
}

fn modus(p: &Path) -> u32 {
    std::fs::symlink_metadata(p).expect("meta").mode() & 0o7777
}

/// A start file of version 2: session `s-4`, harness `pi`, route `lokal-beispiel`, customer `null`, working folder
/// `/heim/beispiel/projekt`.
fn start_f2(fortsetzung_von: &str) -> Startdatei {
    start_mit(fortsetzung_von, "null")
}

/// The start file of [`start_f2`] with the customer `kunde` (a JSON value).
fn start_mit(fortsetzung_von: &str, kunde: &str) -> Startdatei {
    start::lesen_datei(
        format!(
            r#"{{"fassung":2,"sitzung":"s-4","fortsetzung_von":{fortsetzung_von},"harness":"pi","modell":"qwen3:8b","route":"lokal-beispiel","kunde":{kunde},"arbeitsordner":"/heim/beispiel/projekt","effort":null,"name":null}}"#
        )
        .as_bytes(),
    )
    .expect("start file")
}

/// `gespraech.json` with the route and the customer of [`start_f2`].
fn json(harness: &str, ordner: &str, kennung: &str) -> String {
    json_mit(harness, ordner, "\"lokal-beispiel\"", "null", kennung)
}

/// `gespraech.json` of the contract; `route`, `kunde` and `kennung` are JSON values.
fn json_mit(harness: &str, ordner: &str, route: &str, kunde: &str, kennung: &str) -> String {
    format!(
        r#"{{"fassung":1,"harness":"{harness}","arbeitsordner":"{ordner}","route":{route},"kunde":{kunde},"kennung":{kennung}}}"#
    )
}

/// `gespraeche/<sitzung>` with the text `inhalt` in `gespraech.json`.
fn legen(gespraeche: &Path, sitzung: &str, inhalt: &str) {
    ordner_0700(&gespraeche.join(sitzung));
    datei(&gespraeche.join(sitzung).join(DATEI), inhalt, 0o600);
}

fn gut() -> String {
    json("pi", "/heim/beispiel/projekt", &format!("\"{K}\""))
}

#[test]
fn the_key_is_a_lower_case_uuid_v4() {
    assert!(ist_kennung(K));
    for schlecht in [
        "",
        "--yolo",
        "6F1C2E3A-4B5D-4E6F-8A7B-9C0D1E2F3A4B",
        "6f1c2e3a-4b5d-3e6f-8a7b-9c0d1e2f3a4b",
        "6f1c2e3a-4b5d-4e6f-7a7b-9c0d1e2f3a4b",
        "6f1c2e3a-4b5d-4e6f-ca7b-9c0d1e2f3a4b",
        "6f1c2e3a4b5d-4e6f-8a7b-9c0d1e2f3a4b-",
        "6f1c2e3a-4b5d-4e6f-8a7b-9c0d1e2f3a4",
        "6f1c2e3a-4b5d-4e6f-8a7b-9c0d1e2f3a4bb",
        "6f1c2e3a-4b5d-4e6f-8a7b-9c0d1e2f3a4g",
        "6f1c2e3a-4b5d-4e6f-8a7b_9c0d1e2f3a4b",
    ] {
        assert!(!ist_kennung(schlecht), "{schlecht:?}");
    }
    for v in ["8", "9", "a", "b"] {
        assert!(ist_kennung(&format!(
            "6f1c2e3a-4b5d-4e6f-{v}a7b-9c0d1e2f3a4b"
        )));
    }
}

#[test]
fn a_new_key_carries_the_version_and_the_variant() {
    assert_eq!(kennung_aus([0; 16]), "00000000-0000-4000-8000-000000000000");
    assert_eq!(
        kennung_aus([0xff; 16]),
        "ffffffff-ffff-4fff-bfff-ffffffffffff"
    );
    let a = neue_kennung().expect("key");
    let b = neue_kennung().expect("key");
    assert!(ist_kennung(&a) && ist_kennung(&b));
    assert_ne!(a, b, "drawn, not fixed");
}

#[test]
fn the_file_has_exactly_the_closed_form() {
    let g = lesen_text(gut().as_bytes()).expect("form");
    assert_eq!(
        g,
        Gespraech {
            harness: "pi".to_owned(),
            arbeitsordner: "/heim/beispiel/projekt".to_owned(),
            route: "lokal-beispiel".to_owned(),
            kunde: None,
            kennung: Some(K.to_owned())
        }
    );
    assert_eq!(lesen_text(text(&g).as_bytes()), Some(g.clone()));
    assert_eq!(
        text(&g),
        format!("{}\n", gut()),
        "the order of the contract"
    );
    let ohne = Gespraech {
        kennung: None,
        ..g.clone()
    };
    assert_eq!(
        text(&ohne),
        format!("{}\n", json("pi", "/heim/beispiel/projekt", "null"))
    );
    assert_eq!(lesen_text(text(&ohne).as_bytes()), Some(ohne));
    let mit_kunde = Gespraech {
        kunde: Some("k-beispiel".to_owned()),
        ..g.clone()
    };
    assert_eq!(
        text(&mit_kunde),
        format!(
            "{}\n",
            json_mit(
                "pi",
                "/heim/beispiel/projekt",
                "\"lokal-beispiel\"",
                "\"k-beispiel\"",
                &format!("\"{K}\"")
            )
        )
    );
    assert_eq!(lesen_text(text(&mit_kunde).as_bytes()), Some(mit_kunde));
    for schlecht in [
        String::new(),
        "[]".to_owned(),
        json("pi", "/p", "\"--yolo\""),
        json("pi", "/p", "1"),
        gut().replace("\"fassung\":1", "\"fassung\":2"),
        gut().replace("\"fassung\":1", "\"fassung\":\"1\""),
        gut().replace("\"harness\":\"pi\"", "\"harness\":null"),
        gut().replace(
            "\"arbeitsordner\":\"/heim/beispiel/projekt\"",
            "\"arbeitsordner\":1",
        ),
        gut().replace("\"kennung\"", "\"name\""),
        gut().replace('}', ",\"mehr\":1}"),
        gut().replace('}', ",\"harness\":\"pi\"}"),
        gut().replace(",\"harness\":\"pi\"", ""),
        gut().replace(",\"arbeitsordner\":\"/heim/beispiel/projekt\"", ""),
        // without `route` or `kunde`, or of another type (`kunde` may be `null`, `route` never)
        gut().replace(",\"route\":\"lokal-beispiel\"", ""),
        gut().replace(",\"kunde\":null", ""),
        gut().replace("\"route\":\"lokal-beispiel\"", "\"route\":null"),
        gut().replace("\"route\":\"lokal-beispiel\"", "\"route\":1"),
        gut().replace("\"kunde\":null", "\"kunde\":1"),
        gut().replace("\"kunde\":null", "\"kunde\":[]"),
        gut().replace("{\"fassung\":1,", "{"),
    ] {
        assert_eq!(lesen_text(schlecht.as_bytes()), None, "{schlecht}");
    }
}

#[test]
fn a_new_start_needs_no_conversation() {
    let o = Ordner::neu("g-neu");
    assert_eq!(
        pruefen(&o.0.join("fehlt"), &start_f2("null"), true),
        Ok(None)
    );
    assert_eq!(
        pruefen(&o.0.join("fehlt"), &start_f2("null"), false),
        Ok(None)
    );
}

#[test]
fn step_5d_continues_the_conversation_of_the_predecessor() {
    let o = Ordner::neu("g-gut");
    let g = o.0.join("gespraeche");
    ordner_0700(&g);
    legen(&g, "s-1", &gut());
    assert_eq!(
        pruefen(&g, &start_f2("\"s-1\""), true),
        Ok(Some(Fortsetzung {
            alt: "s-1".to_owned(),
            kennung: K.to_owned()
        }))
    );
    // 5d.1 comes first: without an option to continue nothing else is looked at
    assert_eq!(
        pruefen(&o.0.join("fehlt"), &start_f2("\"s-1\""), false),
        Err(Fehler::FortsetzungUnmoeglich)
    );
}

#[test]
fn step_5d_gives_each_code_of_the_contract() {
    let o = Ordner::neu("g-codes");
    let g = o.0.join("gespraeche");
    let s = start_f2("\"s-1\"");
    // 5d.2: no folder `gespraeche`, no folder of the predecessor
    assert_eq!(pruefen(&g, &s, true), Err(Fehler::FortsetzungFehlt));
    ordner_0700(&g);
    assert_eq!(pruefen(&g, &s, true), Err(Fehler::FortsetzungFehlt));
    // 5d.3: the folder of the predecessor with another mode, as a link, or as a file
    legen(&g, "s-1", &gut());
    std::fs::set_permissions(g.join("s-1"), std::fs::Permissions::from_mode(0o750)).expect("mode");
    assert_eq!(pruefen(&g, &s, true), Err(Fehler::FortsetzungKaputt));
    std::fs::set_permissions(g.join("s-1"), std::fs::Permissions::from_mode(0o700)).expect("mode");
    std::fs::rename(g.join("s-1"), o.0.join("echt")).expect("move");
    std::os::unix::fs::symlink(o.0.join("echt"), g.join("s-1")).expect("link");
    assert_eq!(pruefen(&g, &s, true), Err(Fehler::FortsetzungKaputt));
    std::fs::remove_file(g.join("s-1")).expect("remove");
    datei(&g.join("s-1"), &gut(), 0o600);
    assert_eq!(pruefen(&g, &s, true), Err(Fehler::FortsetzungKaputt));
    std::fs::remove_file(g.join("s-1")).expect("remove");
    // 5d.3: the file: missing, another mode, a link, too long, of another form
    ordner_0700(&g.join("s-1"));
    assert_eq!(pruefen(&g, &s, true), Err(Fehler::FortsetzungKaputt));
    datei(&g.join("s-1").join(DATEI), &gut(), 0o644);
    assert_eq!(pruefen(&g, &s, true), Err(Fehler::FortsetzungKaputt));
    std::fs::remove_file(g.join("s-1").join(DATEI)).expect("remove");
    datei(&o.0.join("echt.json"), &gut(), 0o600);
    std::os::unix::fs::symlink(o.0.join("echt.json"), g.join("s-1").join(DATEI)).expect("link");
    assert_eq!(pruefen(&g, &s, true), Err(Fehler::FortsetzungKaputt));
    std::fs::remove_file(g.join("s-1").join(DATEI)).expect("remove");
    let lang = format!("{}{}", gut(), " ".repeat(4097 - gut().len()));
    datei(&g.join("s-1").join(DATEI), &lang, 0o600);
    assert_eq!(pruefen(&g, &s, true), Err(Fehler::FortsetzungKaputt));
    let genau = format!("{}{}", gut(), " ".repeat(4096 - gut().len()));
    datei(&g.join("s-1").join(DATEI), &genau, 0o600);
    assert!(
        pruefen(&g, &s, true).is_ok(),
        "4096 bytes are within the limit"
    );
    datei(
        &g.join("s-1").join(DATEI),
        &json("pi", "/heim/beispiel/projekt", "\"--yolo\""),
        0o600,
    );
    assert_eq!(pruefen(&g, &s, true), Err(Fehler::FortsetzungKaputt));
    // 5d.3a: the conversation of a session of version 1
    datei(
        &g.join("s-1").join(DATEI),
        &json("pi", "/heim/beispiel/projekt", "null"),
        0o600,
    );
    assert_eq!(pruefen(&g, &s, true), Err(Fehler::FortsetzungUnmoeglich));
    // 5d.4: another harness, another working folder (exactly, not resolved)
    for (h, a) in [
        ("claude", "/heim/beispiel/projekt"),
        ("pi", "/heim/beispiel/projekt/"),
        ("pi", "/heim/beispiel/anders"),
    ] {
        datei(
            &g.join("s-1").join(DATEI),
            &json(h, a, &format!("\"{K}\"")),
            0o600,
        );
        assert_eq!(
            pruefen(&g, &s, true),
            Err(Fehler::FortsetzungFremd),
            "{h} {a}"
        );
    }
    // 5d.5: the new session has a conversation already, also as a link or a file
    datei(&g.join("s-1").join(DATEI), &gut(), 0o600);
    ordner_0700(&g.join("s-4"));
    assert_eq!(pruefen(&g, &s, true), Err(Fehler::SitzungBelegt));
    std::fs::remove_dir(g.join("s-4")).expect("remove");
    std::os::unix::fs::symlink(o.0.join("nirgends"), g.join("s-4")).expect("link");
    assert_eq!(pruefen(&g, &s, true), Err(Fehler::SitzungBelegt));
    std::fs::remove_file(g.join("s-4")).expect("remove");
    assert!(pruefen(&g, &s, true).is_ok());
    // the folder `gespraeche` itself: another mode or a link is unusable
    std::fs::set_permissions(&g, std::fs::Permissions::from_mode(0o755)).expect("mode");
    assert_eq!(pruefen(&g, &s, true), Err(Fehler::FortsetzungKaputt));
    std::fs::set_permissions(&g, std::fs::Permissions::from_mode(0o700)).expect("mode");
    let verweis = o.0.join("verweis");
    std::os::unix::fs::symlink(&g, &verweis).expect("link");
    assert_eq!(pruefen(&verweis, &s, true), Err(Fehler::FortsetzungKaputt));
    // nothing of step 5d changes the conversation
    assert_eq!(
        std::fs::read_to_string(g.join("s-1").join(DATEI)).expect("file"),
        gut()
    );
    assert!(!g.join("s-4").exists());
}

#[test]
fn step_5d_4_wants_route_and_customer_exactly_equal() {
    let o = Ordner::neu("g-klasse");
    let g = o.0.join("gespraeche");
    ordner_0700(&g);
    legen(&g, "s-1", &gut());
    let s = start_f2("\"s-1\"");
    // 5d.4: another route (of the same kind), another customer, `null` against a customer: equality only
    let k = format!("\"{K}\"");
    let pfad = "/heim/beispiel/projekt";
    for (route, kunde, start_kunde) in [
        ("\"lokal-anders\"", "null", "null"),
        ("\"lokal-beispiel \"", "null", "null"),
        ("\"lokal-beispiel\"", "\"k-anders\"", "\"k-beispiel\""),
        ("\"lokal-beispiel\"", "\"k-beispiel\"", "null"),
        ("\"lokal-beispiel\"", "null", "\"k-beispiel\""),
        ("\"lokal-beispiel\"", "\"\"", "null"),
    ] {
        datei(
            &g.join("s-1").join(DATEI),
            &json_mit("pi", pfad, route, kunde, &k),
            0o600,
        );
        assert_eq!(
            pruefen(&g, &start_mit("\"s-1\"", start_kunde), true),
            Err(Fehler::FortsetzungFremd),
            "{route} {kunde} {start_kunde}"
        );
    }
    datei(
        &g.join("s-1").join(DATEI),
        &json_mit("pi", pfad, "\"lokal-beispiel\"", "\"k-beispiel\"", &k),
        0o600,
    );
    assert!(
        pruefen(&g, &start_mit("\"s-1\"", "\"k-beispiel\""), true).is_ok(),
        "the same customer"
    );
    // 5d.3 before 5d.4: without `route` the file is broken, not foreign
    datei(
        &g.join("s-1").join(DATEI),
        &gut().replace(",\"route\":\"lokal-beispiel\"", ""),
        0o600,
    );
    assert_eq!(pruefen(&g, &s, true), Err(Fehler::FortsetzungKaputt));
}

#[test]
fn a_new_start_makes_its_conversation_once_at_the_fixed_place() {
    let o = Ordner::neu("g-anlegen");
    let g = o.0.join("gespraeche");
    let neu = Gespraech {
        harness: "pi".to_owned(),
        arbeitsordner: "/heim/beispiel/projekt".to_owned(),
        route: "lokal-beispiel".to_owned(),
        kunde: Some("k-beispiel".to_owned()),
        kennung: Some(K.to_owned()),
    };
    // the folder `gespraeche` is made when it is missing, with mode 0700
    let ordner = anlegen(&g, "s-2", &neu, true).expect("made");
    assert_eq!(ordner, g.join("s-2"), "fixed path: <gespraeche>/<sitzung>");
    assert_eq!(modus(&g), 0o700);
    assert_eq!(modus(&ordner), 0o700);
    assert_eq!(modus(&ordner.join(DATEI)), 0o600);
    assert_eq!(modus(&ordner.join(ZUSTAND)), 0o700);
    assert!(
        std::fs::read_dir(ordner.join(ZUSTAND))
            .expect("list")
            .next()
            .is_none()
    );
    assert_eq!(
        std::fs::read(ordner.join(DATEI)).expect("file"),
        text(&neu).into_bytes()
    );
    let mut namen: Vec<String> = std::fs::read_dir(&ordner)
        .expect("list")
        .map(|e| e.expect("entry").file_name().to_string_lossy().into_owned())
        .collect();
    namen.sort();
    assert_eq!(namen, ["gespraech.json", "zustand"]);
    // a second start of the same session: refused, the first conversation stays unchanged
    let anders = Gespraech {
        kennung: None,
        ..neu.clone()
    };
    assert_eq!(
        anlegen(&g, "s-2", &anders, false),
        Err(Fehler::SitzungBelegt)
    );
    assert_eq!(
        std::fs::read(ordner.join(DATEI)).expect("file"),
        text(&neu).into_bytes()
    );
    // version 1: only the file, no state folder
    let ordner = anlegen(&g, "s-3", &anders, false).expect("made");
    assert!(!ordner.join(ZUSTAND).exists());
    // a link in the place of the new folder is never followed
    std::os::unix::fs::symlink(o.0.join("ziel"), g.join("s-5")).expect("link");
    assert_eq!(anlegen(&g, "s-5", &neu, true), Err(Fehler::SitzungBelegt));
    assert!(!o.0.join("ziel").exists());
    // an unusable folder `gespraeche`: another mode, a link, a file
    std::fs::set_permissions(&g, std::fs::Permissions::from_mode(0o755)).expect("mode");
    assert_eq!(anlegen(&g, "s-6", &neu, true), Err(Fehler::Intern));
    std::fs::set_permissions(&g, std::fs::Permissions::from_mode(0o700)).expect("mode");
    let verweis = o.0.join("verweis");
    std::os::unix::fs::symlink(&g, &verweis).expect("link");
    assert_eq!(anlegen(&verweis, "s-6", &neu, true), Err(Fehler::Intern));
    datei(&o.0.join("datei"), "x", 0o600);
    assert_eq!(
        anlegen(&o.0.join("datei"), "s-6", &neu, true),
        Err(Fehler::Intern)
    );
    assert_eq!(
        anlegen(&o.0.join("fehlt/gespraeche"), "s-6", &neu, true),
        Err(Fehler::Intern),
        "only the last folder is made"
    );
    assert!(!g.join("s-6").exists());
}

#[test]
fn continuing_moves_the_conversation_to_the_new_session_and_back() {
    let o = Ordner::neu("g-umbenennen");
    let g = o.0.join("gespraeche");
    ordner_0700(&g);
    legen(&g, "s-1", &gut());
    ordner_0700(&g.join("s-1").join(ZUSTAND));
    datei(&g.join("s-1").join(ZUSTAND).join("verlauf"), "alt", 0o600);
    assert_eq!(uebernehmen(&g, "s-1", "s-4"), Ok(g.join("s-4")));
    // one conversation per session: under the old key nothing is left
    assert!(!g.join("s-1").exists());
    assert_eq!(
        std::fs::read_to_string(g.join("s-4").join(ZUSTAND).join("verlauf")).expect("state"),
        "alt"
    );
    assert_eq!(
        std::fs::read_to_string(g.join("s-4").join(DATEI)).expect("file"),
        gut(),
        "the file keeps its content"
    );
    // a second continuation of the same old key finds nothing
    assert_eq!(uebernehmen(&g, "s-1", "s-5"), Err(Fehler::FortsetzungFehlt));
    // never over another conversation
    legen(&g, "s-6", &gut());
    assert_eq!(uebernehmen(&g, "s-4", "s-6"), Err(Fehler::SitzungBelegt));
    assert!(g.join("s-4").is_dir());
    // back after a fault, again never over another one
    zurueck(&g, "s-4", "s-1");
    assert!(g.join("s-1").is_dir() && !g.join("s-4").exists());
    legen(&g, "s-4", &gut());
    zurueck(&g, "s-1", "s-4");
    assert!(g.join("s-1").is_dir(), "a taken name stops the way back");
    // the folder `gespraeche` must be usable
    assert_eq!(
        uebernehmen(&o.0.join("fehlt"), "s-1", "s-7"),
        Err(Fehler::Intern)
    );
    assert_eq!(name("a\0b"), Err(Fehler::Intern));
}
