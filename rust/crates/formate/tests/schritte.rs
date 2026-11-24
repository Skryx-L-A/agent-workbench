//! Own tests of the check and the writer beyond the visible cases: every step of the contract alone, the order of
//! the steps, the limits (line length, lock, chunks), the arguments of the command and the exact output.

#![allow(
    clippy::many_single_char_names,
    clippy::too_many_lines,
    clippy::unnecessary_wraps,
    reason = "tables of cases with short names; the helper wraps the expected break in Option like the value it is compared with"
)]

mod gemeinsam;

use std::ffi::OsString;
use std::fs;
use std::path::Path;

use formate::kette::{
    Bruch, Fehler, NULL_HASH, Pruefung, ZEILE_MAX, anhaengen_gesperrt, befehl, kanonisch, pruefe,
    sha256_hex,
};
use gemeinsam::{Wegwerf, als_datei, bruch_von, muster, pruefe_datei, zeilen};
use serde_json::{Map, Value, json};

const BRAIN: &str = "brain-2026-11.jsonl";
const GATEWAY: &str = "gateway-2026-11.jsonl";

fn kanon(wert: &Value) -> String {
    kanonisch(wert).expect("finite numbers")
}

/// A closed, valid line `ausgabe.gesperrt` with the given `vorher`, as a JSON value.
fn gueltig(vorher: &str) -> Value {
    json!({
        "aktionsname": "ausgabe.gesperrt",
        "anzahl": 1,
        "ergebnis": "gesperrt",
        "fassung": 2,
        "grund": "kundendaten-an-cloud",
        "klasse": "kundendaten",
        "kunde": "k-beispiel",
        "route": "cloud-beispiel",
        "sitzung": "s-0001",
        "vorher": vorher,
        "zeit": "2026-11-20T13:00:00.000Z",
    })
}

fn objekt(wert: &mut Value) -> &mut Map<String, Value> {
    wert.as_object_mut().expect("an object")
}

fn einzeln(ordner: &Wegwerf, name: &str, zeile: &str) -> Option<(String, String, Option<u64>)> {
    bruch_von(&pruefe_datei(ordner, name, format!("{zeile}\n").as_bytes()))
}

fn bruch_erwartet(
    datei: &str,
    grund: &str,
    zeile: Option<u64>,
) -> Option<(String, String, Option<u64>)> {
    Some((datei.to_owned(), grund.to_owned(), zeile))
}

fn os(args: &[&str]) -> Vec<OsString> {
    args.iter().map(OsString::from).collect()
}

// ---- the steps of the contract, one by one ----

#[test]
fn the_valid_line_is_whole() {
    let o = Wegwerf::neu("gueltig");
    let p = pruefe_datei(
        &o,
        BRAIN,
        format!("{}\n", kanon(&gueltig(NULL_HASH))).as_bytes(),
    );
    assert_eq!(p.bruch, None);
    assert_eq!(p.zeilen, 1);
    assert!(p.ok());
    assert_eq!(
        p.letzte.as_deref(),
        Some(sha256_hex(kanon(&gueltig(NULL_HASH)).as_bytes()).as_str())
    );
}

#[test]
fn step_unreadable_line() {
    let o = Wegwerf::neu("unlesbar");
    let tief = format!("{{\"a\":{}1{}}}", "[".repeat(200), "]".repeat(200));
    let faelle: Vec<(&str, Vec<u8>)> = vec![
        ("empty line", b"\n".to_vec()),
        ("array", b"[1]\n".to_vec()),
        ("text", b"\"x\"\n".to_vec()),
        ("number", b"5\n".to_vec()),
        ("null", b"null\n".to_vec()),
        ("broken", b"{\"a\":\n".to_vec()),
        ("text after the object", b"{\"a\":1} x\n".to_vec()),
        ("lone high surrogate", b"{\"a\":\"\\ud800\"}\n".to_vec()),
        ("lone low surrogate", b"{\"a\":\"\\udc00\"}\n".to_vec()),
        (
            "high surrogate and a letter",
            b"{\"a\":\"\\ud800x\"}\n".to_vec(),
        ),
        ("not UTF-8", b"{\"a\":\"\xff\"}\n".to_vec()),
        ("BOM", b"\xef\xbb\xbf{\"a\":1}\n".to_vec()),
        ("too deep", format!("{tief}\n").into_bytes()),
    ];
    for (name, inhalt) in faelle {
        let p = pruefe_datei(&o, GATEWAY, &inhalt);
        assert_eq!(
            bruch_von(&p),
            bruch_erwartet(GATEWAY, "zeile_unlesbar", Some(1)),
            "{name}"
        );
        assert_eq!(p.zeilen, 0, "{name}");
    }
    // A pair of surrogates is one character and fine (the line then fails at a later step, not here).
    let p = pruefe_datei(&o, GATEWAY, b"{\"a\":\"\\ud83d\\ude00\"}\n");
    assert_eq!(
        bruch_von(&p),
        bruch_erwartet(GATEWAY, "vorher_fehlt", Some(1))
    );
}

#[test]
fn step_incomplete_last_line_comes_before_unreadable() {
    let o = Wegwerf::neu("unvollstaendig");
    let p = pruefe_datei(&o, GATEWAY, b"kaputt");
    assert_eq!(
        bruch_von(&p),
        bruch_erwartet(GATEWAY, "letzte_zeile_unvollstaendig", Some(1))
    );
    let z = zeilen(2);
    let inhalt = format!("{}\n{}", z[0], z[1]);
    let p = pruefe_datei(&o, BRAIN, inhalt.as_bytes());
    assert_eq!(
        bruch_von(&p),
        bruch_erwartet(BRAIN, "letzte_zeile_unvollstaendig", Some(2))
    );
    assert_eq!(p.zeilen, 1);
}

#[test]
fn step_version_field() {
    let o = Wegwerf::neu("fassung");
    let wirkung = |fassung: Value| {
        let mut z = gueltig(NULL_HASH);
        objekt(&mut z).insert("fassung".to_owned(), fassung);
        einzeln(&o, BRAIN, &kanon(&z))
    };
    for falsch in [
        json!("2"),
        json!(1),
        json!(3),
        json!(null),
        json!(true),
        json!([2]),
        json!(2.5),
    ] {
        assert_eq!(
            wirkung(falsch.clone()),
            bruch_erwartet(BRAIN, "form_falsch", Some(1)),
            "{falsch}"
        );
    }
    assert_eq!(wirkung(json!(2)), None);
    // 2.0 and 2e0 pass the version step and fail as not canonical.
    for schreibweise in ["2.0", "2e0", "20E-1", "2.00"] {
        let zeile = kanon(&gueltig(NULL_HASH))
            .replace("\"fassung\":2", &format!("\"fassung\":{schreibweise}"));
        assert_eq!(
            einzeln(&o, BRAIN, &zeile),
            bruch_erwartet(BRAIN, "nicht_kanonisch", Some(1)),
            "{schreibweise}"
        );
    }
}

#[test]
fn step_version_missing_depends_on_the_file_name() {
    let o = Wegwerf::neu("fassung-fehlt");
    let zeile = kanon(
        &json!({"aktionsname": "x", "ergebnis": "ok", "vorher": NULL_HASH, "zeit": "2026-11-20T13:00:00.000Z"}),
    );
    for name in [
        "gateway-2026-11.jsonl",
        "gateway-1999-01.jsonl",
        "audit.jsonl",
        "audit.2026-11.jsonl",
        "audit.2026-11.3.jsonl",
        "audit.2026-11.12.jsonl",
    ] {
        assert_eq!(einzeln(&o, name, &zeile), None, "{name}");
    }
    for name in [
        "brain-2026-11.jsonl",
        "stelle-2026-11.jsonl",
        "halter-2026-11.jsonl",
        "gateway-2026-13.jsonl",
        "gateway.jsonl",
        "gateway-2026-11.json",
        "audit.2026-11.x.jsonl",
        "audit-2026-11.jsonl",
        "daten.jsonl",
    ] {
        assert_eq!(
            einzeln(&o, name, &zeile),
            bruch_erwartet(name, "fassung_fehlt", Some(1)),
            "{name}"
        );
    }
}

#[test]
fn step_announced_break() {
    let o = Wegwerf::neu("kettenbruch");
    let mit = |bruch: Value, vorher: Option<&str>| {
        let mut z = json!({"aktionsname": "x", "ergebnis": "ok", "kettenbruch": bruch, "zeit": "2026-11-20T13:00:00.000Z"});
        if let Some(v) = vorher {
            objekt(&mut z).insert("vorher".to_owned(), json!(v));
        }
        kanon(&z)
    };
    assert_eq!(
        einzeln(&o, GATEWAY, &mit(json!("letzte_zeile_unlesbar"), None)),
        bruch_erwartet(GATEWAY, "angekuendigt:letzte_zeile_unlesbar", Some(1))
    );
    // The announcement comes before the check of `vorher`.
    let f64er = "f".repeat(64);
    assert_eq!(
        einzeln(&o, GATEWAY, &mit(json!("x"), Some(f64er.as_str()))),
        bruch_erwartet(GATEWAY, "angekuendigt:x", Some(1))
    );
    for kein_text in [json!(5), json!(null), json!(["x"]), json!(true)] {
        assert_eq!(
            einzeln(&o, GATEWAY, &mit(kein_text.clone(), None)),
            bruch_erwartet(GATEWAY, "form_falsch", Some(1)),
            "{kein_text}"
        );
    }
    // The version step comes before the announcement.
    assert_eq!(
        einzeln(&o, BRAIN, &mit(json!("x"), None)),
        bruch_erwartet(BRAIN, "fassung_fehlt", Some(1))
    );
    let mut z = gueltig(NULL_HASH);
    objekt(&mut z).insert("kettenbruch".to_owned(), json!("y"));
    assert_eq!(
        einzeln(&o, BRAIN, &kanon(&z)),
        bruch_erwartet(BRAIN, "angekuendigt:y", Some(1))
    );
}

#[test]
fn step_before_field_and_chain_link() {
    let o = Wegwerf::neu("vorher");
    let ohne =
        kanon(&json!({"aktionsname": "x", "ergebnis": "ok", "zeit": "2026-11-20T13:00:00.000Z"}));
    assert_eq!(
        einzeln(&o, GATEWAY, &ohne),
        bruch_erwartet(GATEWAY, "vorher_fehlt", Some(1))
    );
    for vorher in [
        json!(5),
        json!(null),
        json!("x"),
        json!("0".repeat(63)),
        json!("A".repeat(64)),
        json!("1".repeat(64)),
    ] {
        let zeile = kanon(
            &json!({"aktionsname": "x", "ergebnis": "ok", "vorher": vorher, "zeit": "2026-11-20T13:00:00.000Z"}),
        );
        assert_eq!(
            einzeln(&o, GATEWAY, &zeile),
            bruch_erwartet(GATEWAY, "kette_gebrochen", Some(1)),
            "{vorher}"
        );
    }
}

#[test]
fn steps_after_the_chain_link_run_in_order() {
    let o = Wegwerf::neu("reihenfolge");
    let gut = kanon(&gueltig(NULL_HASH));
    // Not canonical and a wrong `vorher`: the chain link comes first.
    let verschoben = gut
        .replace("\"anzahl\":1", "\"anzahl\": 1")
        .replace(NULL_HASH, &"1".repeat(64));
    assert_eq!(
        einzeln(&o, BRAIN, &verschoben),
        bruch_erwartet(BRAIN, "kette_gebrochen", Some(1))
    );
    // Not canonical with a right `vorher`.
    let leerzeichen = gut.replace("\"anzahl\":1", "\"anzahl\": 1");
    assert_eq!(
        einzeln(&o, BRAIN, &leerzeichen),
        bruch_erwartet(BRAIN, "nicht_kanonisch", Some(1))
    );
    // Not canonical and a wrong time: canonical first.
    let beides = leerzeichen.replace("13:00:00.000Z", "13:00:00Z");
    assert_eq!(
        einzeln(&o, BRAIN, &beides),
        bruch_erwartet(BRAIN, "nicht_kanonisch", Some(1))
    );
    // Key order is part of the canonical form.
    let (a, b) = ("\"aktionsname\":\"ausgabe.gesperrt\"", "\"anzahl\":1");
    let vertauscht = gut.replace(&format!("{a},{b}"), &format!("{b},{a}"));
    assert_ne!(vertauscht, gut);
    assert_eq!(
        einzeln(&o, BRAIN, &vertauscht),
        bruch_erwartet(BRAIN, "nicht_kanonisch", Some(1))
    );
    // A space after the object is not canonical either.
    assert_eq!(
        einzeln(&o, BRAIN, &format!("{gut} ")),
        bruch_erwartet(BRAIN, "nicht_kanonisch", Some(1))
    );
}

#[test]
fn step_form_of_a_version_2_line() {
    let o = Wegwerf::neu("form2");
    let andere = |aendern: &dyn Fn(&mut Map<String, Value>)| {
        // A kind of the register whose form no arm checks yet, so this test stays about the step, not the form.
        let mut z = json!({"aktionsname": "suche.rueckfall", "ergebnis": "ok", "fassung": 2, "vorher": NULL_HASH, "zeit": "2026-11-20T13:00:00.000Z"});
        aendern(objekt(&mut z));
        einzeln(&o, BRAIN, &kanon(&z))
    };
    assert_eq!(andere(&|_| {}), None);
    assert_eq!(
        andere(&|m| {
            m.insert("extra".to_owned(), json!([1, {"a": null}]));
        }),
        None
    );
    let form_falsch = bruch_erwartet(BRAIN, "form_falsch", Some(1));
    // Register, fail closed for version 2: a kind contract 5 does not name fails, with and without a field of
    // its own; the same line in a gateway file (version 1g) stays a plain link (see the end of this test).
    for art in ["test.zeile", "mcp.schreibaufrufe", "ausgabe.gesperrte"] {
        assert_eq!(
            andere(&|m| {
                m.insert("aktionsname".to_owned(), json!(art));
            }),
            form_falsch,
            "kind {art}"
        );
    }
    // A version that is not 2 fails even when the action is no `ausgabe.gesperrt`.
    for fassung in [json!(1), json!(3), json!(2.5), json!("2")] {
        assert_eq!(
            andere(&|m| {
                m.insert("fassung".to_owned(), fassung.clone());
            }),
            form_falsch,
            "fassung {fassung}"
        );
    }
    assert_eq!(
        andere(&|m| {
            m.remove("zeit");
        }),
        form_falsch
    );
    assert_eq!(
        andere(&|m| {
            m.insert("zeit".to_owned(), json!(5));
        }),
        form_falsch
    );
    assert_eq!(
        andere(&|m| {
            m.insert("zeit".to_owned(), json!("2026-11-20T13:00:00Z"));
        }),
        form_falsch
    );
    assert_eq!(
        andere(&|m| {
            m.remove("aktionsname");
        }),
        form_falsch
    );
    assert_eq!(
        andere(&|m| {
            m.insert("aktionsname".to_owned(), json!(5));
        }),
        form_falsch
    );
    assert_eq!(
        andere(&|m| {
            m.remove("ergebnis");
        }),
        form_falsch
    );
    assert_eq!(
        andere(&|m| {
            m.insert("ergebnis".to_owned(), json!(null));
        }),
        form_falsch
    );
}

#[test]
fn step_closed_form_of_ausgabe_gesperrt_by_field() {
    let o = Wegwerf::neu("geschlossen");
    let wirkung = |aendern: &dyn Fn(&mut Map<String, Value>)| {
        let mut z = gueltig(NULL_HASH);
        aendern(objekt(&mut z));
        einzeln(&o, BRAIN, &kanon(&z))
    };
    assert_eq!(wirkung(&|_| {}), None);
    let setze = |m: &mut Map<String, Value>, k: &str, v: Value| {
        m.insert(k.to_owned(), v);
    };
    // Values that are allowed.
    assert_eq!(
        wirkung(&|m| setze(m, "anzahl", json!(4_294_967_295_u64))),
        None
    );
    assert_eq!(
        wirkung(&|m| {
            setze(m, "klasse", json!(null));
            setze(m, "kunde", json!(null));
        }),
        None
    );
    assert_eq!(
        wirkung(&|m| setze(m, "sitzung", json!(format!("a{}", "b".repeat(63))))),
        None
    );
    // Values that are not.
    let falsch = bruch_erwartet(BRAIN, "form_falsch", Some(1));
    for (schluessel, werte) in [
        (
            "ergebnis",
            vec![json!("ok"), json!("Gesperrt"), json!(null)],
        ),
        (
            "anzahl",
            vec![
                json!(0),
                json!(4_294_967_296_u64),
                json!(1.5),
                json!("1"),
                json!(null),
                json!(-1),
                json!(true),
            ],
        ),
        (
            "grund",
            vec![
                json!(""),
                json!("A"),
                json!("a b"),
                json!(5),
                json!(null),
                json!(format!("a{}", "b".repeat(64))),
            ],
        ),
        (
            "route",
            vec![
                json!(""),
                json!("Cloud"),
                json!("-x"),
                json!(5),
                json!(null),
            ],
        ),
        (
            "sitzung",
            vec![json!(""), json!("S1"), json!(".x"), json!(5), json!(null)],
        ),
        (
            "klasse",
            vec![
                json!(""),
                json!("Kunden"),
                json!(5),
                json!(true),
                json!(["a"]),
            ],
        ),
        (
            "kunde",
            vec![json!(""), json!("K 1"), json!(5), json!(false), json!({})],
        ),
        (
            "zeit",
            vec![
                json!("2026-11-20T13:00:00Z"),
                json!("2026-13-20T13:00:00.000Z"),
                json!(1_795_179_600),
            ],
        ),
    ] {
        for wert in werte {
            assert_eq!(
                wirkung(&|m| setze(m, schluessel, wert.clone())),
                falsch,
                "{schluessel} = {wert}"
            );
        }
    }
    // A key that is missing or added.
    for schluessel in [
        "anzahl",
        "grund",
        "klasse",
        "kunde",
        "route",
        "sitzung",
        "zeit",
        "ergebnis",
        "aktionsname",
    ] {
        assert_eq!(
            wirkung(&|m| {
                m.remove(schluessel);
            }),
            falsch,
            "without {schluessel}"
        );
    }
    assert_eq!(
        wirkung(&|m| {
            m.remove("fassung");
        }),
        bruch_erwartet(BRAIN, "fassung_fehlt", Some(1))
    );
    assert_eq!(
        wirkung(&|m| {
            m.remove("vorher");
        }),
        bruch_erwartet(BRAIN, "vorher_fehlt", Some(1))
    );
    for extra in ["titel", "pfad", "a", "zzz"] {
        assert_eq!(
            wirkung(&|m| setze(m, extra, json!("x"))),
            falsch,
            "extra {extra}"
        );
    }
}

#[test]
fn closed_form_needs_version_2_and_the_vorher_a_hash() {
    let o = Wegwerf::neu("geschlossen-2");
    // In a gateway file a line without `fassung` may exist, but not as `ausgabe.gesperrt`.
    let mut z = gueltig(NULL_HASH);
    objekt(&mut z).remove("fassung");
    assert_eq!(
        einzeln(&o, GATEWAY, &kanon(&z)),
        bruch_erwartet(GATEWAY, "form_falsch", Some(1))
    );
    // Another action name in a gateway file stays a plain link.
    let mut z = gueltig(NULL_HASH);
    objekt(&mut z).remove("fassung");
    objekt(&mut z).insert("aktionsname".to_owned(), json!("test.zeile"));
    assert_eq!(einzeln(&o, GATEWAY, &kanon(&z)), None);
    // The library takes any text as the start `vorher`; a line that carries the same non-hash is no `ausgabe.gesperrt`.
    let pfad = o.schreibe(
        BRAIN,
        format!("{}\n", kanon(&gueltig(&"A".repeat(64)))).as_bytes(),
    );
    let p = pruefe(&[pfad], &"A".repeat(64), None);
    assert_eq!(bruch_von(&p), bruch_erwartet(BRAIN, "form_falsch", Some(1)));
    let pfad = o.schreibe(GATEWAY, format!("{}\n", kanon(&gueltig("kurz"))).as_bytes());
    let p = pruefe(&[pfad], "kurz", None);
    assert_eq!(
        bruch_von(&p),
        bruch_erwartet(GATEWAY, "form_falsch", Some(1))
    );
}

// ---- files and the chain ----

#[test]
fn a_directory_is_an_unreadable_file() {
    let o = Wegwerf::neu("ordner");
    let p = pruefe(&[o.ordner().to_path_buf()], NULL_HASH, None);
    let name = o
        .ordner()
        .file_name()
        .expect("a name")
        .to_string_lossy()
        .into_owned();
    assert_eq!(
        bruch_von(&p),
        bruch_erwartet(&name, "datei_nicht_lesbar", None)
    );
    assert_eq!(p.letzte, None);
}

#[test]
fn a_chain_of_many_lines_over_two_files() {
    let o = Wegwerf::neu("zwei-dateien");
    let z = zeilen(5);
    let a = o.schreibe("brain-2026-10.jsonl", als_datei(&z[..2]).as_bytes());
    let b = o.schreibe(BRAIN, als_datei(&z[2..]).as_bytes());
    let p = pruefe(&[a.clone(), b.clone()], NULL_HASH, None);
    assert_eq!(p.bruch, None);
    assert_eq!(p.zeilen, 5);
    let letzte = sha256_hex(z[4].as_bytes());
    assert_eq!(p.letzte.as_deref(), Some(letzte.as_str()));
    // The other order breaks at the first line of the file that comes first.
    let p = pruefe(&[b.clone(), a.clone()], NULL_HASH, None);
    assert_eq!(
        bruch_von(&p),
        bruch_erwartet(BRAIN, "kette_gebrochen", Some(1))
    );
    assert_eq!(p.zeilen, 0);
    // A break in the second file counts the line inside that file and all good lines before it.
    let kaputt = als_datei(&z[2..]).replace(&z[3], &z[3].replace("\"anzahl\":4", "\"anzahl\":9"));
    let b2 = o.schreibe("brain-2026-12.jsonl", kaputt.as_bytes());
    let p = pruefe(&[a.clone(), b2], NULL_HASH, None);
    assert_eq!(
        bruch_von(&p),
        bruch_erwartet("brain-2026-12.jsonl", "kette_gebrochen", Some(3))
    );
    assert_eq!(p.zeilen, 4);
    // An empty file between two files changes nothing; a file in a folder is named without the folder.
    let leer = o.schreibe("brain-2026-11a.jsonl", b"");
    let p = pruefe(&[a, leer, b], NULL_HASH, None);
    assert_eq!(p.bruch, None);
    assert_eq!(p.zeilen, 5);
}

#[test]
fn start_value_of_the_chain() {
    let o = Wegwerf::neu("start");
    let z = zeilen(3);
    let pfad = o.schreibe(BRAIN, als_datei(&z[1..]).as_bytes());
    let anker = sha256_hex(z[0].as_bytes());
    let p = pruefe(std::slice::from_ref(&pfad), &anker, None);
    assert_eq!((p.bruch, p.zeilen), (None, 2));
    let p = pruefe(std::slice::from_ref(&pfad), NULL_HASH, None);
    assert_eq!(
        bruch_von(&p),
        bruch_erwartet(BRAIN, "kette_gebrochen", Some(1))
    );
    // Only the first line takes the start value.
    let p = pruefe(&[pfad.clone(), pfad], &anker, None);
    assert_eq!(
        bruch_von(&p),
        bruch_erwartet(BRAIN, "kette_gebrochen", Some(1))
    );
    assert_eq!(p.zeilen, 2);
}

#[test]
fn last_line_anchor() {
    let o = Wegwerf::neu("letzte");
    let z = zeilen(3);
    let pfad = o.schreibe(BRAIN, als_datei(&z).as_bytes());
    let echt = sha256_hex(z[2].as_bytes());
    let p = pruefe(std::slice::from_ref(&pfad), NULL_HASH, Some(echt.as_str()));
    assert_eq!((p.bruch, p.zeilen), (None, 3));
    let vorletzte = sha256_hex(z[1].as_bytes());
    let p = pruefe(
        std::slice::from_ref(&pfad),
        NULL_HASH,
        Some(vorletzte.as_str()),
    );
    assert_eq!(
        bruch_von(&p),
        bruch_erwartet(BRAIN, "letzte_falsch", Some(3))
    );
    assert_eq!((p.zeilen, p.letzte), (3, None));
    // The break names the file that holds the last line, not the last file of the call.
    let leer = o.schreibe("brain-2026-12.jsonl", b"");
    let p = pruefe(&[pfad.clone(), leer.clone()], NULL_HASH, Some(NULL_HASH));
    assert_eq!(
        bruch_von(&p),
        bruch_erwartet(BRAIN, "letzte_falsch", Some(3))
    );
    let p = pruefe(&[pfad, leer], NULL_HASH, Some(echt.as_str()));
    assert_eq!(p.bruch, None);
    // No line at all: any anchor is wrong; the break has no line and names the last file.
    let nichts = o.schreibe("brain-2027-01.jsonl", b"");
    let p = pruefe(&[nichts], NULL_HASH, Some(NULL_HASH));
    assert_eq!(
        bruch_von(&p),
        bruch_erwartet("brain-2027-01.jsonl", "letzte_falsch", None)
    );
    assert_eq!(p.zeilen, 0);
}

// ---- the limit of a line ----

/// A line without `fassung` of exactly `laenge` bytes, valid for a gateway file.
fn zeile_mit_laenge(laenge: usize) -> String {
    let kopf = format!(
        "{{\"aktionsname\":\"x\",\"ergebnis\":\"ok\",\"vorher\":\"{NULL_HASH}\",\"zeit\":\"2026-11-20T13:00:00.000Z\",\"z\":\""
    );
    let schluss = "\"}";
    let fuellung = laenge - kopf.len() - schluss.len();
    let zeile = format!("{kopf}{}{schluss}", "a".repeat(fuellung));
    assert_eq!(zeile.len(), laenge);
    zeile
}

#[test]
fn the_limit_of_a_line_is_4194304_bytes() {
    assert_eq!(ZEILE_MAX, 4_194_304);
    let o = Wegwerf::neu("grenze");
    let genau = zeile_mit_laenge(ZEILE_MAX);
    let p = pruefe_datei(&o, GATEWAY, format!("{genau}\n").as_bytes());
    assert_eq!((p.bruch, p.zeilen), (None, 1));
    // One byte more, two bytes more, and far more, each with a line feed: too long, which reads as unreadable.
    for mehr in [1, 2, 3, 100_000] {
        let zeile = zeile_mit_laenge(ZEILE_MAX + mehr);
        let p = pruefe_datei(&o, GATEWAY, format!("{zeile}\n").as_bytes());
        assert_eq!(
            bruch_von(&p),
            bruch_erwartet(GATEWAY, "zeile_unlesbar", Some(1)),
            "+{mehr}"
        );
        assert_eq!(p.zeilen, 0, "+{mehr}");
    }
    // Without the line feed the file is incomplete, however long the line is.
    for mehr in [0, 1, 2, 3, 100_000] {
        let zeile = zeile_mit_laenge(ZEILE_MAX + mehr);
        let p = pruefe_datei(&o, GATEWAY, zeile.as_bytes());
        assert_eq!(
            bruch_von(&p),
            bruch_erwartet(GATEWAY, "letzte_zeile_unvollstaendig", Some(1)),
            "+{mehr}"
        );
    }
    // A good line before a too long one stays counted.
    let inhalt = format!(
        "{}\n{}\n",
        zeile_mit_laenge(200),
        zeile_mit_laenge(ZEILE_MAX + 5)
    );
    let p = pruefe_datei(&o, GATEWAY, inhalt.as_bytes());
    assert_eq!(
        bruch_von(&p),
        bruch_erwartet(GATEWAY, "zeile_unlesbar", Some(2))
    );
    assert_eq!(p.zeilen, 1);
}

// ---- the output ----

#[test]
fn output_bytes() {
    let ok = Pruefung {
        zeilen: 3,
        letzte: Some("ab".repeat(32)),
        bruch: None,
    };
    assert_eq!(
        ok.ausgabe(),
        format!(
            "{{\"bruch\":null,\"letzte\":\"{}\",\"ok\":true,\"zeilen\":3}}\n",
            "ab".repeat(32)
        )
    );
    let leer = Pruefung {
        zeilen: 0,
        letzte: None,
        bruch: None,
    };
    assert_eq!(
        leer.ausgabe(),
        "{\"bruch\":null,\"letzte\":null,\"ok\":true,\"zeilen\":0}\n"
    );
    let kaputt = Pruefung {
        zeilen: 2,
        letzte: None,
        bruch: Some(Bruch {
            datei: "a\"b.jsonl".to_owned(),
            grund: "kette_gebrochen".to_owned(),
            zeile: Some(3),
        }),
    };
    assert!(!kaputt.ok());
    assert_eq!(
        kaputt.ausgabe(),
        "{\"bruch\":{\"datei\":\"a\\\"b.jsonl\",\"grund\":\"kette_gebrochen\",\"zeile\":3},\"letzte\":null,\"ok\":false,\"zeilen\":2}\n"
    );
    let ohne_zeile = Pruefung {
        zeilen: 0,
        letzte: None,
        bruch: Some(Bruch {
            datei: "x".to_owned(),
            grund: "datei_nicht_lesbar".to_owned(),
            zeile: None,
        }),
    };
    assert_eq!(
        ohne_zeile.ausgabe(),
        "{\"bruch\":{\"datei\":\"x\",\"grund\":\"datei_nicht_lesbar\",\"zeile\":null},\"letzte\":null,\"ok\":false,\"zeilen\":0}\n"
    );
}

// ---- the command ----

#[test]
fn command_arguments() {
    let o = Wegwerf::neu("befehl");
    let z = zeilen(2);
    let pfad = o.schreibe(BRAIN, als_datei(&z).as_bytes());
    let datei = pfad.to_string_lossy().into_owned();
    let echt = sha256_hex(z[1].as_bytes());
    let nullen = NULL_HASH;

    let a = befehl(&os(&["pruefe", &datei]));
    assert_eq!(a.exit, 0);
    assert_eq!(a.stderr, "");
    assert_eq!(
        a.stdout,
        format!("{{\"bruch\":null,\"letzte\":\"{echt}\",\"ok\":true,\"zeilen\":2}}\n")
    );
    // Switches in any place, with their values.
    let b = befehl(&os(&[
        "pruefe", "--letzte", &echt, &datei, "--vorher", nullen,
    ]));
    assert_eq!((b.exit, b.stdout), (0, a.stdout.clone()));
    let c = befehl(&os(&["pruefe", &datei, "--letzte", &sha256_hex(b"x")]));
    assert_eq!(c.exit, 1);
    assert!(
        c.stdout.contains("\"grund\":\"letzte_falsch\""),
        "{}",
        c.stdout
    );
    assert_eq!(c.stderr, "");
    // A name that starts with one dash is a file.
    let d = befehl(&os(&["pruefe", "-x"]));
    assert_eq!(d.exit, 1);
    assert!(d.stdout.contains("\"datei\":\"-x\""), "{}", d.stdout);

    let falsch: Vec<Vec<OsString>> = vec![
        os(&[]),
        os(&["pruefe"]),
        os(&["zeige", &datei]),
        os(&["Pruefe", &datei]),
        os(&["pruefe", "--alles", &datei]),
        os(&["pruefe", "--", &datei]),
        os(&["pruefe", "--vorher"]),
        os(&["pruefe", &datei, "--vorher"]),
        os(&["pruefe", &datei, "--letzte"]),
        os(&["pruefe", "--vorher", "abc", &datei]),
        os(&["pruefe", "--vorher", &"A".repeat(64), &datei]),
        os(&["pruefe", "--vorher", &"g".repeat(64), &datei]),
        os(&["pruefe", "--vorher", &"0".repeat(65), &datei]),
        os(&["pruefe", "--letzte", &"0".repeat(63), &datei]),
        os(&["pruefe", "--vorher", "--letzte", &datei]),
        os(&["pruefe", "--vorher", nullen, "--vorher", nullen, &datei]),
        os(&["pruefe", "--letzte", &echt, "--letzte", &echt, &datei]),
        os(&["pruefe", "--vorher", nullen]),
        os(&["pruefe", "--letzte", &echt]),
    ];
    for args in falsch {
        let r = befehl(&args);
        assert_eq!(r.exit, 64, "{args:?}");
        assert_eq!(r.stdout, "", "{args:?}");
        assert!(r.stderr.starts_with("kette: "), "{args:?}: {}", r.stderr);
        assert!(
            r.stderr.contains("Aufruf: kette pruefe"),
            "{args:?}: {}",
            r.stderr
        );
    }
}

#[cfg(unix)]
#[test]
fn command_refuses_values_that_are_no_text() {
    use std::os::unix::ffi::OsStringExt;
    let o = Wegwerf::neu("befehl-bytes");
    let pfad = o.schreibe(BRAIN, b"");
    let mut args = os(&["pruefe", "--vorher"]);
    args.push(OsString::from_vec(vec![0xff; 64]));
    args.push(pfad.clone().into_os_string());
    let r = befehl(&args);
    assert_eq!((r.exit, r.stdout.as_str()), (64, ""));
    // A file name that is no text is fine: it is reported lossily.
    let datei = OsString::from_vec(vec![b'x', 0xff, b'y']);
    let r = befehl(&[OsString::from("pruefe"), datei]);
    assert_eq!(r.exit, 1);
    assert!(
        r.stdout.contains("\"grund\":\"datei_nicht_lesbar\""),
        "{}",
        r.stdout
    );
}

// ---- the writer ----

fn vorher_der_letzten(datei: &Path) -> String {
    let inhalt = fs::read_to_string(datei).expect("file");
    let letzte = inhalt
        .trim_end_matches('\n')
        .rsplit('\n')
        .next()
        .expect("a line");
    let wert: Value = serde_json::from_str(letzte).expect("JSON");
    wert["vorher"].as_str().expect("vorher").to_owned()
}

#[test]
fn the_writer_reads_the_last_line_from_the_end() {
    let o = Wegwerf::neu("lesen-vom-ende");
    let pfad = o.pfad(BRAIN);
    for laenge in [
        0usize, 1, 2, 4094, 4095, 4096, 4097, 8191, 8192, 8193, 12_287, 12_288, 12_289, 20_000,
        100_000,
    ] {
        let letzte = "y".repeat(laenge);
        for kopf in ["", "p\n", "xx\n\n"] {
            fs::write(&pfad, format!("{kopf}{letzte}\n")).expect("write");
            let vor = fs::metadata(&pfad).expect("meta").len();
            let hash = anhaengen_gesperrt(&pfad, &muster()).expect("append");
            let soll = sha256_hex(letzte.as_bytes());
            assert_eq!(
                vorher_der_letzten(&pfad),
                soll,
                "last line of {laenge} bytes after {kopf:?}"
            );
            assert_eq!(hash.len(), 64);
            assert!(fs::metadata(&pfad).expect("meta").len() > vor);
        }
    }
}

#[test]
fn the_writer_changes_nothing_on_an_error() {
    let o = Wegwerf::neu("fehler");
    let pfad = o.pfad(BRAIN);
    let z = als_datei(&zeilen(2));
    fs::write(&pfad, &z).expect("write");
    let mut schlecht = muster();
    schlecht.anzahl = 0;
    assert_eq!(
        anhaengen_gesperrt(&pfad, &schlecht),
        Err(Fehler::FeldUngueltig("anzahl"))
    );
    let mut schlecht = muster();
    schlecht.zeit_ms = 253_402_300_800_000;
    assert_eq!(
        anhaengen_gesperrt(&pfad, &schlecht),
        Err(Fehler::ZeitUngueltig)
    );
    assert_eq!(fs::read_to_string(&pfad).expect("read"), z);
    // An incomplete last line.
    let halb = format!("{z}{{\"a\":1");
    fs::write(&pfad, &halb).expect("write");
    assert_eq!(
        anhaengen_gesperrt(&pfad, &muster()),
        Err(Fehler::LetzteZeileUnvollstaendig)
    );
    assert_eq!(fs::read_to_string(&pfad).expect("read"), halb);
    // A wrong field on a name that does not exist yet creates no file.
    let neu = o.pfad("neu.jsonl");
    let mut schlecht = muster();
    schlecht.route = "X".to_owned();
    assert_eq!(
        anhaengen_gesperrt(&neu, &schlecht),
        Err(Fehler::FeldUngueltig("route"))
    );
    assert!(!neu.exists());
}

#[test]
fn the_writer_reports_io_errors() {
    let o = Wegwerf::neu("io");
    let r = anhaengen_gesperrt(o.ordner(), &muster());
    assert_eq!(
        r.as_ref().map_err(Fehler::code),
        Err("ein_ausgabe".to_owned()),
        "{r:?}"
    );
    let r = anhaengen_gesperrt(&o.pfad("fehlt/brain.jsonl"), &muster());
    assert_eq!(
        r.as_ref().map_err(Fehler::code),
        Err("ein_ausgabe".to_owned()),
        "{r:?}"
    );
}

#[cfg(target_os = "linux")]
#[test]
fn a_failed_write_is_an_error() {
    // /dev/full accepts every open and refuses every write with "no space left".
    if !Path::new("/dev/full").exists() {
        return;
    }
    let r = anhaengen_gesperrt(Path::new("/dev/full"), &muster());
    assert_eq!(
        r.as_ref().map_err(Fehler::code),
        Err("ein_ausgabe".to_owned()),
        "{r:?}"
    );
}

#[test]
fn the_lock_keeps_a_second_writer_out() {
    use std::fs::OpenOptions;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::{Duration, Instant};

    let o = Wegwerf::neu("sperre");
    let pfad = o.pfad(BRAIN);
    let halter = OpenOptions::new()
        .read(true)
        .append(true)
        .create(true)
        .open(&pfad)
        .expect("open");
    halter.lock().expect("lock");
    let gestartet = AtomicBool::new(false);
    std::thread::scope(|bereich| {
        bereich.spawn(|| {
            gestartet.store(true, Ordering::SeqCst);
            anhaengen_gesperrt(&pfad, &muster()).expect("append after the lock is free");
        });
        let frist = Instant::now() + Duration::from_secs(10);
        while !gestartet.load(Ordering::SeqCst) && Instant::now() < frist {
            std::thread::sleep(Duration::from_millis(1));
        }
        // The writer had every chance to write; it must still wait for the lock.
        std::thread::sleep(Duration::from_millis(300));
        assert_eq!(
            fs::metadata(&pfad).expect("meta").len(),
            0,
            "written under a foreign lock"
        );
        halter.unlock().expect("unlock");
    });
    assert_eq!(pruefe(&[pfad], NULL_HASH, None).zeilen, 1);
}

#[test]
fn numbers_in_a_line_are_read_as_doubles() {
    // 9007199254740993 does not fit a double; JavaScript reads and writes it as 9007199254740992.
    let wert: Value =
        serde_json::from_str("{\"a\":9007199254740993,\"b\":-0,\"c\":-0.0,\"d\":1E400000}")
            .unwrap_or(Value::Null);
    assert_eq!(
        wert,
        Value::Null,
        "a number out of range is refused by the reader"
    );
    let wert: Value = serde_json::from_str(
        "{\"a\":9007199254740993,\"b\":-0,\"c\":-0.0,\"d\":0.30000000000000004}",
    )
    .expect("JSON");
    assert_eq!(
        kanon(&wert),
        "{\"a\":9007199254740992,\"b\":0,\"c\":0,\"d\":0.30000000000000004}"
    );
}
