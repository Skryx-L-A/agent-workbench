//! Steps of contract 5, section 5, one by one: the limits, the lock, the files that stand in the way, and the
//! values of the anchor. The visible cases are in `monate.rs`.

mod gemeinsam;

use std::ffi::OsString;
use std::fs;
use std::process::Command;

use formate::kette::{
    Fehler, NULL_HASH, ZEILE_MAX, anhaengen_gesperrt_in, anhaengen_zeile, anhaengen_zeilen,
    kanonisch, pruefe_quelle, sha256_hex,
};
use gemeinsam::{Wegwerf, muster};
use serde_json::{Value, json};

const NOV: i64 = 1_795_179_600_000; // 2026-11-20T13:00:00.000Z
const DEZ: i64 = 1_796_083_200_000; // 2026-12-01T00:00:00.000Z
const NOV_DATEI: &str = "brain-2026-11.jsonl";
const DEZ_DATEI: &str = "brain-2026-12.jsonl";
const ANKER: &str = "brain.anker.jsonl";

fn code<T: std::fmt::Debug>(r: &Result<T, Fehler>) -> String {
    r.as_ref().map_or_else(Fehler::code, |_| "ok".to_owned())
}

fn objekt(fuell: usize) -> Value {
    // A kind of the register whose form no arm checks yet, so this helper is about the length, not the form.
    json!({"aktionsname": "suche.rueckfall", "ergebnis": "ok", "x": "x".repeat(fuell)})
}

fn ordner_mit_november(name: &str) -> Wegwerf {
    let o = Wegwerf::neu(name);
    let mut g = muster();
    g.zeit_ms = NOV;
    anhaengen_gesperrt_in(o.ordner(), "brain", &g).expect("first line");
    o
}

fn anker_wert(o: &Wegwerf) -> Value {
    let text = fs::read_to_string(o.pfad(ANKER)).expect("anchor file");
    assert_eq!(text.matches('\n').count(), 1, "{text}");
    serde_json::from_str(text.trim_end()).expect("JSON")
}

#[test]
fn the_new_error_codes() {
    assert_eq!(Fehler::ZeitRueckwaerts.code(), "zeit_rueckwaerts");
    assert_eq!(Fehler::AbschnittVorhanden.code(), "abschnitt_vorhanden");
    assert_eq!(Fehler::AnkerFalsch.code(), "anker_falsch");
    assert_eq!(Fehler::VorherAbweichend.code(), "vorher_abweichend");
    assert_eq!(
        Fehler::ZeileUngueltig("form_falsch".to_owned()).code(),
        "zeile_ungueltig:form_falsch"
    );
    assert_eq!(Fehler::AnkerFalsch.to_string(), "anker_falsch");
    assert_eq!(
        Fehler::ZeileUngueltig("x".to_owned()).to_string(),
        "zeile_ungueltig:x"
    );
}

#[test]
fn the_lock_on_the_anchor_file_covers_the_switch() {
    use std::fs::OpenOptions;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::{Duration, Instant};

    let o = ordner_mit_november("sperre-wechsel");
    let halter = OpenOptions::new()
        .read(true)
        .append(true)
        .open(o.pfad(ANKER))
        .expect("open the anchor file");
    halter.lock().expect("lock");
    let gestartet = AtomicBool::new(false);
    std::thread::scope(|bereich| {
        bereich.spawn(|| {
            let mut g = muster();
            g.zeit_ms = DEZ;
            gestartet.store(true, Ordering::SeqCst);
            anhaengen_gesperrt_in(o.ordner(), "brain", &g).expect("append after the lock is free");
        });
        let frist = Instant::now() + Duration::from_secs(10);
        while !gestartet.load(Ordering::SeqCst) && Instant::now() < frist {
            std::thread::sleep(Duration::from_millis(1));
        }
        std::thread::sleep(Duration::from_millis(300));
        assert!(!o.pfad(DEZ_DATEI).exists(), "written under a foreign lock");
        assert_eq!(fs::metadata(o.pfad(ANKER)).expect("meta").len(), 0);
        halter.unlock().expect("unlock");
    });
    assert!(o.pfad(DEZ_DATEI).exists());
    assert_eq!(anker_wert(&o)["abschnitt"], NOV_DATEI);
}

#[test]
fn the_anchor_has_null_for_what_cannot_be_read() {
    for (fall, inhalt) in [
        ("no JSON", "kaputt\n".to_owned()),
        ("not an object", "[1]\n".to_owned()),
        (
            "a time before 1970",
            format!("{{\"vorher\":\"{NULL_HASH}\",\"zeit\":\"1969-12-31T23:59:59.999Z\"}}\n"),
        ),
        (
            "a date that does not exist",
            format!("{{\"vorher\":\"{NULL_HASH}\",\"zeit\":\"2026-02-31T00:00:00.000Z\"}}\n"),
        ),
        (
            "a time that is no text",
            format!("{{\"vorher\":\"{NULL_HASH}\",\"zeit\":5}}\n"),
        ),
        (
            "a time of another pattern",
            format!("{{\"vorher\":\"{NULL_HASH}\",\"zeit\":\"2026-11-20T13:00:00Z\"}}\n"),
        ),
    ] {
        let o = Wegwerf::neu("anker-null");
        o.schreibe(NOV_DATEI, inhalt.as_bytes());
        let mut g = muster();
        g.zeit_ms = DEZ;
        anhaengen_gesperrt_in(o.ordner(), "brain", &g).expect(fall);
        let a = anker_wert(&o);
        assert_eq!(a["von"], Value::Null, "{fall}");
        assert_eq!(a["bis"], Value::Null, "{fall}");
        let vorher_ok = fall.starts_with("a ");
        assert_eq!(
            a["vorher_erste"],
            if vorher_ok {
                json!(NULL_HASH)
            } else {
                Value::Null
            },
            "{fall}"
        );
        assert_eq!(
            a["letzte"].as_str(),
            Some(sha256_hex(inhalt.trim_end().as_bytes()).as_str()),
            "{fall}"
        );
    }
}

/// A line that is a JSON object of exactly `laenge` bytes: `vorher` and `zeit` as the anchor reads them.
fn zeile_der_laenge(laenge: usize, zeit: &str) -> String {
    let kopf = format!("{{\"vorher\":\"{NULL_HASH}\",\"zeit\":\"{zeit}\",\"p\":\"");
    let fuell = laenge - kopf.len() - 2;
    format!("{kopf}{}\"}}", "x".repeat(fuell))
}

#[test]
fn the_anchor_reads_a_line_up_to_the_limit() {
    let zeit = "2026-11-01T00:00:01.000Z";
    for (laenge, gelesen) in [(ZEILE_MAX, true), (ZEILE_MAX + 1, false)] {
        for erste in [true, false] {
            let o = Wegwerf::neu("anker-grenze");
            let gross = zeile_der_laenge(laenge, zeit);
            let klein =
                format!("{{\"vorher\":\"{NULL_HASH}\",\"zeit\":\"2026-11-02T00:00:00.000Z\"}}");
            let inhalt = if erste {
                format!("{gross}\n{klein}\n")
            } else {
                format!("{klein}\n{gross}\n")
            };
            o.schreibe(NOV_DATEI, inhalt.as_bytes());
            let mut g = muster();
            g.zeit_ms = DEZ;
            anhaengen_gesperrt_in(o.ordner(), "brain", &g).expect("switch");
            let a = anker_wert(&o);
            let (gross_wert, klein_wert) = (Some(1_793_491_201.0), Some(1_793_577_600.0));
            let (soll_von, soll_bis) = if erste {
                (gross_wert, klein_wert)
            } else {
                (klein_wert, gross_wert)
            };
            // The line over the limit is not read: its time is null.
            let lesbar = |soll: Option<f64>, ist: &Value, ist_gross: bool| {
                if ist_gross && !gelesen {
                    assert_eq!(ist, &Value::Null, "{laenge} {erste}");
                } else {
                    assert_eq!(ist.as_f64(), soll, "{laenge} {erste}");
                }
            };
            lesbar(soll_von, &a["von"], erste);
            lesbar(soll_bis, &a["bis"], !erste);
        }
    }
}

#[test]
fn a_general_line_has_at_most_4194304_bytes() {
    let o = Wegwerf::neu("laenge-zeile");
    anhaengen_zeile(o.ordner(), "brain", NOV, &objekt(0)).expect("small");
    let klein = fs::read_to_string(o.pfad(NOV_DATEI)).expect("file");
    let grundlaenge = klein.trim_end().len();
    let o = Wegwerf::neu("laenge-zeile-2");
    let r = anhaengen_zeile(o.ordner(), "brain", NOV, &objekt(ZEILE_MAX - grundlaenge));
    assert_eq!(code(&r), "ok");
    let inhalt = fs::read_to_string(o.pfad(NOV_DATEI)).expect("file");
    assert_eq!(inhalt.trim_end().len(), ZEILE_MAX);
    let o = Wegwerf::neu("laenge-zeile-3");
    let r = anhaengen_zeile(
        o.ordner(),
        "brain",
        NOV,
        &objekt(ZEILE_MAX - grundlaenge + 1),
    );
    assert_eq!(code(&r), "feld_ungueltig:zeile");
    assert!(!o.pfad(NOV_DATEI).exists());
    assert!(!o.pfad(ANKER).exists(), "nothing is made before the check");
}

#[test]
fn finished_lines_are_checked_like_the_checker_does() {
    let o = Wegwerf::neu("fertig");
    anhaengen_zeile(o.ordner(), "brain", NOV, &objekt(0)).expect("small");
    let zeile = fs::read_to_string(o.pfad(NOV_DATEI)).expect("file");
    let zeile = zeile.trim_end().to_owned();
    let grund = zeile.len();
    let lang = |n: usize| zeile.replace("\"x\":\"\"", &format!("\"x\":\"{}\"", "x".repeat(n)));
    for (n, soll) in [
        (ZEILE_MAX - grund, "ok"),
        (ZEILE_MAX - grund + 1, "zeile_ungueltig:zeile_unlesbar"),
    ] {
        let ziel = Wegwerf::neu("fertig-lang");
        let r = anhaengen_zeilen(ziel.ordner(), "brain", NULL_HASH, &[lang(n)]);
        assert_eq!(code(&r), soll, "{n}");
    }
}

#[test]
fn finished_lines_need_a_time_in_the_calendar_and_the_same_month() {
    let zeile = |zeit: &str, vorher: &str| {
        format!(
            "{{\"aktionsname\":\"suche.rueckfall\",\"ergebnis\":\"ok\",\"fassung\":2,\"vorher\":\"{vorher}\",\"zeit\":\"{zeit}\"}}"
        )
    };
    for zeit in ["2026-02-31T00:00:00.000Z", "1969-12-31T23:59:59.999Z"] {
        let o = Wegwerf::neu("fertig-zeit");
        let r = anhaengen_zeilen(o.ordner(), "brain", NULL_HASH, &[zeile(zeit, NULL_HASH)]);
        assert_eq!(code(&r), "zeit_ungueltig", "{zeit}");
        assert!(!o.pfad(ANKER).exists());
    }
    // The month of every line counts, not only of the first.
    let o = Wegwerf::neu("fertig-monate");
    let eins = zeile("2026-11-20T13:00:00.000Z", NULL_HASH);
    let zwei = zeile("2026-12-01T00:00:00.000Z", &sha256_hex(eins.as_bytes()));
    let r = anhaengen_zeilen(
        o.ordner(),
        "brain",
        NULL_HASH,
        &[eins.clone(), zwei.clone()],
    );
    assert_eq!(code(&r), "feld_ungueltig:zeit");
    let r = anhaengen_zeilen(o.ordner(), "brain", NULL_HASH, std::slice::from_ref(&eins));
    assert_eq!(code(&r), "ok");
    assert_eq!(
        fs::read_to_string(o.pfad(NOV_DATEI)).expect("file"),
        format!("{eins}\n")
    );
    // An empty list, a wrong expectation and a wrong source are refused before any file is touched.
    let leer = Wegwerf::neu("fertig-leer");
    assert_eq!(
        code(&anhaengen_zeilen(leer.ordner(), "brain", NULL_HASH, &[])),
        "feld_ungueltig:zeilen"
    );
    assert_eq!(
        code(&anhaengen_zeilen(
            leer.ordner(),
            "brain",
            "abc",
            std::slice::from_ref(&eins)
        )),
        "feld_ungueltig:vorher"
    );
    assert_eq!(
        code(&anhaengen_zeilen(
            leer.ordner(),
            "audit",
            NULL_HASH,
            &[eins]
        )),
        "feld_ungueltig:quelle"
    );
    assert!(fs::read_dir(leer.ordner()).expect("dir").next().is_none());
}

#[test]
fn a_month_file_that_stands_in_the_way_is_an_io_error() {
    let o = Wegwerf::neu("im-weg");
    let mut g = muster();
    g.zeit_ms = NOV;
    // A directory with the name of the month.
    fs::create_dir(o.pfad(NOV_DATEI)).expect("dir");
    let r = anhaengen_gesperrt_in(o.ordner(), "brain", &g);
    assert_eq!(code(&r), "ein_ausgabe", "{r:?}");
    // A directory as the anchor file.
    let p = Wegwerf::neu("im-weg-2");
    fs::create_dir(p.pfad(ANKER)).expect("dir");
    let r = anhaengen_gesperrt_in(p.ordner(), "brain", &g);
    assert_eq!(code(&r), "ein_ausgabe", "{r:?}");
    // A folder that is not there, and a file instead of a folder.
    let r = anhaengen_gesperrt_in(&o.pfad("fehlt"), "brain", &g);
    assert_eq!(code(&r), "ein_ausgabe", "{r:?}");
    let datei = o.schreibe("datei", b"x");
    let r = anhaengen_gesperrt_in(&datei, "brain", &g);
    assert_eq!(code(&r), "ein_ausgabe", "{r:?}");
}

#[cfg(unix)]
#[test]
fn a_folder_that_cannot_be_written_is_an_io_error() {
    use std::os::unix::fs::PermissionsExt;
    let o = Wegwerf::neu("nur-lesen");
    // The anchor file exists (opening it does not need the folder to be writable); the month file cannot be made.
    o.schreibe(ANKER, b"");
    fs::set_permissions(o.ordner(), fs::Permissions::from_mode(0o500)).expect("chmod");
    let probe = fs::File::create(o.pfad("probe"));
    let als_wurzel = probe.is_ok();
    drop(probe);
    let mut g = muster();
    g.zeit_ms = NOV;
    let r = anhaengen_gesperrt_in(o.ordner(), "brain", &g);
    fs::set_permissions(o.ordner(), fs::Permissions::from_mode(0o700)).expect("chmod back");
    if als_wurzel {
        return; // root can write anywhere; the case needs a user
    }
    assert_eq!(code(&r), "ein_ausgabe", "{r:?}");
    assert!(!o.pfad(NOV_DATEI).exists());
}

#[test]
fn the_library_check_of_a_folder_names_its_source() {
    let o = Wegwerf::neu("pruefe-quelle");
    assert_eq!(
        pruefe_quelle(o.ordner(), "audit", NULL_HASH, None).map(|p| p.zeilen),
        Err(Fehler::FeldUngueltig("quelle"))
    );
    let p = pruefe_quelle(o.ordner(), "halter", NULL_HASH, None).expect("source");
    assert!(p.ok());
    assert_eq!((p.zeilen, p.letzte), (0, None));
}

#[test]
fn an_empty_folder_with_a_wrong_last_line_names_the_folder() {
    let o = Wegwerf::neu("pruefe-quelle-leer");
    let name = o
        .ordner()
        .file_name()
        .expect("a name")
        .to_string_lossy()
        .into_owned();
    let p = pruefe_quelle(o.ordner(), "brain", NULL_HASH, Some(&"1".repeat(64))).expect("source");
    let b = p.bruch.expect("a break");
    assert_eq!(
        (b.datei, b.grund.as_str(), b.zeile),
        (name, "letzte_falsch", None)
    );
    assert_eq!(p.zeilen, 0);
}

#[test]
fn a_folder_that_cannot_be_read_is_a_break() {
    let o = Wegwerf::neu("pruefe-quelle-io");
    let datei = o.schreibe("datei", b"x");
    let p = pruefe_quelle(&datei, "brain", NULL_HASH, None).expect("source");
    let b = p.bruch.expect("a break");
    assert_eq!(
        (b.datei.as_str(), b.grund.as_str(), b.zeile),
        ("datei", "datei_nicht_lesbar", None)
    );
    // The anchor file as a directory.
    fs::create_dir(o.pfad(ANKER)).expect("dir");
    let p = pruefe_quelle(o.ordner(), "brain", NULL_HASH, None).expect("source");
    let b = p.bruch.expect("a break");
    assert_eq!(
        (b.datei.as_str(), b.grund.as_str(), b.zeile),
        (ANKER, "datei_nicht_lesbar", None)
    );
    // A month file as a directory.
    let q = Wegwerf::neu("pruefe-quelle-io-2");
    fs::create_dir(q.pfad(NOV_DATEI)).expect("dir");
    let p = pruefe_quelle(q.ordner(), "brain", NULL_HASH, None).expect("source");
    let b = p.bruch.expect("a break");
    assert_eq!(
        (b.datei.as_str(), b.grund.as_str(), b.zeile),
        (NOV_DATEI, "datei_nicht_lesbar", None)
    );
}

#[cfg(unix)]
#[test]
fn an_anchor_file_that_cannot_be_opened_is_a_break() {
    use std::os::unix::fs::PermissionsExt;
    let o = Wegwerf::neu("anker-gesperrt");
    let pfad = o.schreibe(ANKER, b"");
    fs::set_permissions(&pfad, fs::Permissions::from_mode(0o000)).expect("chmod");
    let als_wurzel = fs::File::open(&pfad).is_ok();
    let p = pruefe_quelle(o.ordner(), "brain", NULL_HASH, None).expect("source");
    fs::set_permissions(&pfad, fs::Permissions::from_mode(0o600)).expect("chmod back");
    if als_wurzel {
        return; // root opens every file; the case needs a user
    }
    let b = p.bruch.expect("a break");
    assert_eq!(
        (b.datei.as_str(), b.grund.as_str(), b.zeile),
        (ANKER, "datei_nicht_lesbar", None)
    );
}

#[test]
fn an_anchor_line_has_at_most_4194304_bytes() {
    let kopf = "{\"abschnitt\":\"brain-2026-11.jsonl\",\"p\":\"";
    for (laenge, soll) in [
        (ZEILE_MAX, "anker_ohne_datei"),
        (ZEILE_MAX + 1, "zeile_unlesbar"),
    ] {
        let o = Wegwerf::neu("anker-lang");
        let zeile = format!("{kopf}{}\"}}\n", "x".repeat(laenge - kopf.len() - 2));
        o.schreibe(ANKER, zeile.as_bytes());
        let p = pruefe_quelle(o.ordner(), "brain", NULL_HASH, None).expect("source");
        let b = p.bruch.expect("a break");
        assert_eq!(b.grund, soll, "{laenge}");
        assert_eq!(b.zeile, Some(1));
    }
}

#[test]
fn the_command_explains_a_wrong_call_for_a_folder() {
    for args in [
        vec!["pruefe-quelle"],
        vec!["pruefe-quelle", "ordner"],
        vec!["pruefe-quelle", "ordner", "audit"],
        vec!["pruefe-quelle", "a", "b", "brain"],
    ] {
        let aus = Command::new(env!("CARGO_BIN_EXE_kette"))
            .args(args.iter().map(OsString::from))
            .output()
            .expect("start kette");
        assert_eq!(aus.status.code(), Some(64), "{args:?}");
        assert_eq!(aus.stdout, Vec::<u8>::new(), "{args:?}");
        let meldung = String::from_utf8(aus.stderr).expect("UTF-8");
        assert!(meldung.contains("kette pruefe-quelle"), "{meldung}");
    }
}

#[test]
fn a_canonical_anchor_is_what_the_writer_writes() {
    let o = ordner_mit_november("kanonisch");
    let mut g = muster();
    g.zeit_ms = DEZ;
    anhaengen_gesperrt_in(o.ordner(), "brain", &g).expect("switch");
    let text = fs::read_to_string(o.pfad(ANKER)).expect("anchor");
    let wert: Value = serde_json::from_str(text.trim_end()).expect("JSON");
    assert_eq!(kanonisch(&wert).as_deref(), Ok(text.trim_end()));
    assert_eq!(wert["von"].as_f64(), Some(1_795_179_600.0));
    assert_eq!(wert["bis"], wert["von"]);
    assert_eq!(wert["fassung"], 2);
    assert_eq!(wert["monat"], "2026-11");
}
