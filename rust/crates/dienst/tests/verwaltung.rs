//! The sessions as the page manages them (contract, "Nachtrag 26.11.2026: Oberfläche L1, L2, L3, L5, L6, L11"), with children of the
//! system (`/bin/sh`), so that they run in the build without the feature `pruefbau`: workers with their orchestrator (DI-L1),
//! `Umbenennen`, `Fortsetzen`, `Wiederbeleben` and `Loeschen` (DI-L3), the new fields of the list and the Bestand (DI-L5). The cases
//! of the contract with the fixed child programs are in `tests/faelle/` (`dil1_*`, `dil3_*`, `dil5_*`).
//! Every wait is for a state (a file, the end of the process, the state of a session) with a deadline.
#![cfg(unix)] // the crate is Unix only (see `src/lib.rs`)

mod gemeinsam;

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use gemeinsam::{DIENST, Probe, Prozess, Wegwerf, datei_schreiben, startdatei};
use serde_json::{Value, json};

/// The child: a shell script. It copies its start file (the last argument, after `--meldung <pfad>`) to `start-<id>.json` next to
/// itself and writes `pid-<id>`, then waits until the file `ende-<id>` exists and ends with the code in it.
fn programm(w: &Wegwerf) -> String {
    let pfad = w.0.join("kind.sh");
    let d = w.0.display();
    let skript = format!(
        "#!/bin/sh\nfor a; do D=\"$a\"; done\nS=$(basename \"$(dirname \"$D\")\")\ncp \"$D\" \"{d}/start-$S.json\"\n\
         echo $$ > \"{d}/pid-$S\"\n\
         while [ ! -e \"{d}/ende-$S\" ]; do sleep 0.05; done\nexit $(cat \"{d}/ende-$S\")\n"
    );
    std::fs::write(&pfad, skript).expect("script");
    std::fs::set_permissions(&pfad, std::fs::Permissions::from_mode(0o700)).expect("mode");
    pfad.to_string_lossy().into_owned()
}

/// A start file with `lauf` and no first session; `lauf_zusatz` adds keys to `lauf`.
fn lauf_start(w: &Wegwerf, lauf_zusatz: &Value) -> Value {
    let einst = w.0.join("einst.json");
    std::fs::write(
        &einst,
        include_str!("werkzeug/einstellungen/vorfuehrung.json"),
    )
    .expect("settings");
    let mut start = startdatei(w, &[DIENST], &json!({}));
    let o = start.as_object_mut().expect("object");
    o.remove("kind");
    o.remove("groesse");
    start["anfang"]["sitzungen"] = json!([]);
    start["lauf"] = json!({
        "programm": programm(w),
        "env": {"LC_ALL": "C.UTF-8", "PATH": "/usr/bin:/bin"},
        "groesse": [40, 10],
        "einstellungen": einst.to_string_lossy(),
    });
    if let Some(z) = lauf_zusatz.as_object() {
        for (k, v) in z {
            start["lauf"][k] = v.clone();
        }
    }
    start
}

fn gut(ordner: &str) -> Value {
    json!({"fassung": 1, "harness": "claude", "modell": "m", "route": "cloud-beispiel", "kunde": "k-beispiel", "arbeitsordner": ordner})
}

/// The same start structure in version 2 (contract `start`, Nachtrag 26.11.2026): without effort and name.
fn gut2(ordner: &str) -> Value {
    json!({"fassung": 2, "harness": "claude", "modell": "m", "route": "cloud-beispiel", "kunde": "k-beispiel", "arbeitsordner": ordner,
        "effort": null, "name": null})
}

/// Sends `e` and waits for `Gestartet` or a refusal; returns the identifier or the whole refusal.
fn antwort(v: &mut Probe, e: &Value) -> Result<String, Value> {
    let (n, a) = (v.nachrichten.len(), v.ablehnungen_voll.len());
    v.ereignis(e);
    assert!(
        v.warte(Duration::from_secs(10), |p| p.ablehnungen_voll.len() > a
            || p.nachrichten[n..].iter().any(|m| m["art"] == "Gestartet")),
        "no answer to {e}"
    );
    if let Some(ab) = v.ablehnungen_voll.get(a) {
        return Err(ab.clone());
    }
    Ok(v.nachrichten[n..]
        .iter()
        .find(|m| m["art"] == "Gestartet")
        .and_then(|m| m["sitzung"].as_str())
        .expect("identifier")
        .to_owned())
}

fn starten(v: &mut Probe, e: &Value) -> String {
    antwort(v, e).unwrap_or_else(|a| panic!("refused: {a}"))
}

/// The refusal of a start: `grund`, `code` (the same word) and the fixed text.
fn start_abgelehnt(v: &mut Probe, e: &Value, grund: &str, text: &str) {
    assert_eq!(
        antwort(v, e),
        Err(json!({"grund": grund, "code": grund, "text": text})),
        "{e}"
    );
}

/// Sends `e` and returns the reason of the refusal that comes (a barrier follows, so a refusal that does not come is `None`).
fn grund(v: &mut Probe, e: &Value) -> Option<String> {
    let a = v.ablehnungen.len();
    v.ereignis(e);
    v.zustand();
    v.ablehnungen.get(a).cloned()
}

/// The last `Sitzungsliste` after a barrier.
fn liste(v: &mut Probe) -> Vec<Value> {
    v.zustand();
    v.nachrichten
        .iter()
        .rev()
        .find(|m| m["art"] == "Sitzungsliste")
        .and_then(|m| m["sitzungen"].as_array().cloned())
        .unwrap_or_default()
}

fn eintrag(v: &mut Probe, id: &str) -> Option<Value> {
    liste(v).into_iter().find(|s| s["id"] == id)
}

/// The entry of `id`, waiting until it is `zustand` (at most 15 s).
fn warte_zustand(v: &mut Probe, id: &str, zustand: &str) -> Value {
    let ende = Instant::now() + Duration::from_secs(15);
    loop {
        if let Some(s) = eintrag(v, id).filter(|s| s["zustand"] == zustand) {
            return s;
        }
        assert!(Instant::now() < ende, "session {id} is not {zustand}");
        v.lesen(Duration::from_millis(50));
    }
}

fn warte_datei(pfad: &Path) {
    let ende = Instant::now() + Duration::from_secs(15);
    while !pfad.exists() {
        assert!(Instant::now() < ende, "{} did not appear", pfad.display());
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// Lets the child of `id` end by itself with `code` and waits until the list shows the end.
fn ende(w: &Wegwerf, v: &mut Probe, id: &str, code: u8) -> Value {
    warte_datei(&w.0.join(format!("pid-{id}")));
    std::fs::write(w.0.join(format!("ende-{id}")), code.to_string()).expect("end file");
    warte_zustand(v, id, "beendet")
}

/// The messages `Agenten` so far.
fn agenten(v: &Probe) -> Vec<Value> {
    v.nachrichten
        .iter()
        .filter(|m| m["art"] == "Agenten")
        .map(|m| m["agenten"].clone())
        .collect()
}

fn worker(orchestrator: &str) -> Value {
    json!({"art": "Starten", "start": gut("/tmp"), "orchestrator": orchestrator})
}

/// DI-L1: a worker names its orchestrator; the service sends its own `Agenten` right after the list of the same change, and only when
/// the list of the workers changed; the four checks of the orchestrator come with `grund`, `code` and `text`.
#[test]
#[allow(clippy::many_single_char_names)] // w, p, v as in the other tests; o is the orchestrator
fn worker_mit_orchestrator_und_eigene_agenten() {
    let w = Wegwerf::neu("worker");
    let mut p = Prozess::starten(&w, &lauf_start(&w, &json!({})));
    let mut v = Probe::neu(&w.socket(), false);
    v.zustand();
    assert_eq!(
        agenten(&v),
        vec![json!([])],
        "the list at the connect, empty"
    );
    let o = starten(&mut v, &json!({"art": "Starten", "start": gut("/tmp")}));
    assert_eq!(
        agenten(&v).len(),
        1,
        "a start without a worker sends no new list"
    );
    let n = v.nachrichten.len();
    let s2 = starten(&mut v, &worker(&o));
    let arten: Vec<&str> = v.nachrichten[n..]
        .iter()
        .filter_map(|m| m["art"].as_str())
        .collect();
    assert_eq!(arten, ["Sitzungsliste", "Agenten", "Gestartet"]);
    let soll = json!([{"worker": s2, "sitzung": o, "rolle": "worker", "modell": "m", "zustand": "laeuft", "nr": 2}]);
    assert_eq!(agenten(&v).last(), Some(&soll));
    // a second viewer gets the list of the service at the connect, at the place of the one of the room core
    let mut v2 = Probe::neu(&w.socket(), false);
    v2.zustand();
    assert_eq!(agenten(&v2), vec![soll]);
    let s3 = starten(
        &mut v,
        &json!({"art": "Starten", "start": gut("/tmp"), "orchestrator": o, "rolle": "pruefer"}),
    );
    assert_eq!(
        agenten(&v)
            .last()
            .and_then(|l| l.get(1))
            .map(|x| x["rolle"].clone()),
        Some(json!("pruefer"))
    );
    start_abgelehnt(
        &mut v,
        &worker("s-99"),
        "orchestrator_unbekannt",
        "Orchestrator unbekannt",
    );
    start_abgelehnt(
        &mut v,
        &worker(&s2),
        "orchestrator_ist_worker",
        "Orchestrator ist selbst ein Worker",
    );
    let mut fremd = worker(&o);
    fremd["start"]["kunde"] = Value::Null;
    fremd["start"]["route"] = json!("cloud-beispiel");
    start_abgelehnt(
        &mut v,
        &fremd,
        "orchestrator_fremder_kunde",
        "Kunde weicht vom Orchestrator ab",
    );
    for falsch in [
        json!({"art": "Starten", "start": gut("/tmp"), "rolle": "worker"}),
        json!({"art": "Starten", "start": gut("/tmp"), "orchestrator": "S-1"}),
        json!({"art": "Starten", "start": gut("/tmp"), "orchestrator": 1}),
        json!({"art": "Starten", "start": gut("/tmp"), "orchestrator": o, "rolle": "Pruefer"}),
        json!({"art": "Starten", "start": gut("/tmp"), "orchestrator": o, "rolle": null}),
    ] {
        start_abgelehnt(&mut v, &falsch, "start_form", "Start hat falsche Form");
    }
    // the end of a worker changes the list of the workers: it comes again, with the code
    let vorher = agenten(&v).len();
    ende(&w, &mut v, &s3, 4);
    v.zustand();
    assert_eq!(agenten(&v).len(), vorher + 1);
    let letzte = agenten(&v).last().cloned().unwrap_or_default();
    assert_eq!(letzte[1]["zustand"], json!("beendet"));
    assert_eq!(letzte[1]["code"], json!(4));
    // an ended orchestrator takes no worker; its end does not end the workers (DI-L1 point 6)
    ende(&w, &mut v, &o, 0);
    start_abgelehnt(
        &mut v,
        &worker(&o),
        "orchestrator_beendet",
        "Orchestrator ist beendet",
    );
    assert_eq!(
        eintrag(&mut v, &s2).map(|s| s["zustand"].clone()),
        Some(json!("laeuft"))
    );
    ende(&w, &mut v, &s2, 0);
    p.signal("TERM");
    assert_eq!(p.warte_ende(Duration::from_secs(15)), Some(0));
}

/// DI-L3 point 1 and DI-L5 points 1 and 2: the new fields of the list, `Umbenennen` with its checks, and `abgestuerzt` by the kind of
/// the end.
#[test]
#[allow(clippy::too_many_lines)] // one run of the service through every field of the list
fn liste_umbenennen_und_absturz() {
    let w = Wegwerf::neu("umbenennen");
    let mut p = Prozess::starten(&w, &lauf_start(&w, &json!({})));
    let mut v = Probe::neu(&w.socket(), false);
    let s1 = starten(
        &mut v,
        &json!({"art": "Starten", "start": gut("/home/a/kunde-x")}),
    );
    let e = eintrag(&mut v, &s1).expect("entry");
    assert_eq!(
        (&e["name"], &e["ordner"], &e["projekt"]),
        (&json!(""), &json!("/home/a/kunde-x"), &json!("kunde-x"))
    );
    assert!(e.get("abgestuerzt").is_none(), "only an ended session: {e}");
    assert!(e.get("fortsetzung_von").is_none() && e.get("fortgesetzt_als").is_none());
    assert_eq!(
        grund(
            &mut v,
            &json!({"art": "Umbenennen", "sitzung": s1, "name": "Plan ä"})
        ),
        None
    );
    assert_eq!(
        eintrag(&mut v, &s1).map(|s| s["name"].clone()),
        Some(json!("Plan ä"))
    );
    let lang = "ä".repeat(64);
    assert_eq!(
        grund(
            &mut v,
            &json!({"art": "Umbenennen", "sitzung": s1, "name": lang})
        ),
        None
    );
    for name in [
        json!("ä".repeat(65)),
        json!(""),
        json!(" vorn"),
        json!("hinten "),
        json!("zwei\nZeilen"),
        json!("x\u{7f}"),
        json!("x\u{9f}"),
        json!(5),
    ] {
        assert_eq!(
            grund(
                &mut v,
                &json!({"art": "Umbenennen", "sitzung": s1, "name": name})
            )
            .as_deref(),
            Some("name_ungueltig"),
            "{name}"
        );
    }
    assert_eq!(
        eintrag(&mut v, &s1).map(|s| s["name"].clone()),
        Some(json!(lang))
    );
    for falsch in [
        json!({"art": "Umbenennen", "sitzung": s1}),
        json!({"art": "Umbenennen", "sitzung": 1, "name": "x"}),
        json!({"art": "Umbenennen", "sitzung": s1, "name": "x", "y": 1}),
    ] {
        assert_eq!(
            grund(&mut v, &falsch).as_deref(),
            Some("umbenennen_form"),
            "{falsch}"
        );
    }
    assert_eq!(
        grund(
            &mut v,
            &json!({"art": "Umbenennen", "sitzung": "s-9", "name": "x"})
        )
        .as_deref(),
        Some("sitzung_unbekannt")
    );
    // by itself with a code other than 0: crashed; by itself with 0 and by `Beenden`: not
    let e = ende(&w, &mut v, &s1, 3);
    assert_eq!((&e["code"], &e["abgestuerzt"]), (&json!(3), &json!(true)));
    // the crate `start` takes no `/` as the working folder; `projekt` of `/` is in the unit tests of `verwaltung.rs`
    let s2 = starten(&mut v, &json!({"art": "Starten", "start": gut("/x")}));
    let e = eintrag(&mut v, &s2).expect("entry");
    assert_eq!((&e["ordner"], &e["projekt"]), (&json!("/x"), &json!("x")));
    let e = ende(&w, &mut v, &s2, 0);
    assert_eq!(e["abgestuerzt"], json!(false));
    let s3 = starten(&mut v, &json!({"art": "Starten", "start": gut("/tmp")}));
    warte_datei(&w.0.join(format!("pid-{s3}")));
    v.ereignis(&json!({"art": "Beenden", "sitzung": s3}));
    let e = warte_zustand(&mut v, &s3, "beendet");
    assert_ne!(e["code"], json!(0), "the shell ends on SIGHUP: {e}");
    assert_eq!(e["abgestuerzt"], json!(false), "{e}");
    assert_eq!(
        grund(
            &mut v,
            &json!({"art": "Umbenennen", "sitzung": s3, "name": "fertig"})
        ),
        None
    );
    assert_eq!(
        eintrag(&mut v, &s3).map(|s| s["name"].clone()),
        Some(json!("fertig"))
    );
    p.signal("TERM");
    assert_eq!(p.warte_ende(Duration::from_secs(15)), Some(0));
}

/// DI-L3 points 2 and 3: `Fortsetzen` and `Wiederbeleben` start the stored structure of version 2 again with a new identifier,
/// `fortsetzung_von` in `start.json` (written by the crate `start`), the name of the old session and the references in both entries;
/// their checks answer like `Starten`. A session of version 1 is `nicht_fortsetzbar` (Nachtrag 27.11.2026).
#[test]
#[allow(clippy::too_many_lines)] // every reason of DI-L3 points 2 and 3 in the order of the contract, in one run of the service
fn fortsetzen_und_wiederbeleben() {
    let w = Wegwerf::neu("fortsetzen");
    let mut p = Prozess::starten(&w, &lauf_start(&w, &json!({})));
    let mut v = Probe::neu(&w.socket(), false);
    let s1 = starten(&mut v, &json!({"art": "Starten", "start": gut2("/tmp")}));
    assert_eq!(
        grund(
            &mut v,
            &json!({"art": "Umbenennen", "sitzung": s1, "name": "eins"})
        ),
        None
    );
    let fort = |s: &str| json!({"art": "Fortsetzen", "sitzung": s});
    let wieder = |s: &str| json!({"art": "Wiederbeleben", "sitzung": s});
    start_abgelehnt(&mut v, &fort(&s1), "sitzung_laeuft", "Sitzung läuft");
    start_abgelehnt(&mut v, &wieder(&s1), "sitzung_laeuft", "Sitzung läuft");
    start_abgelehnt(
        &mut v,
        &fort("s-9"),
        "sitzung_unbekannt",
        "Sitzung unbekannt",
    );
    start_abgelehnt(
        &mut v,
        &wieder("s-9"),
        "sitzung_unbekannt",
        "Sitzung unbekannt",
    );
    start_abgelehnt(
        &mut v,
        &json!({"art": "Fortsetzen", "sitzung": s1, "x": 1}),
        "fortsetzen_form",
        "Fortsetzen hat falsche Form",
    );
    start_abgelehnt(
        &mut v,
        &json!({"art": "Wiederbeleben"}),
        "wiederbeleben_form",
        "Wiederbeleben hat falsche Form",
    );
    ende(&w, &mut v, &s1, 0);
    start_abgelehnt(
        &mut v,
        &wieder(&s1),
        "nicht_abgestuerzt",
        "Sitzung ist nicht abgestürzt",
    );
    let s2 = starten(&mut v, &fort(&s1));
    warte_datei(&w.0.join(format!("pid-{s2}")));
    let datei: Value = serde_json::from_slice(
        &std::fs::read(w.0.join(format!("start-{s2}.json")))
            .expect("start.json of the new session"),
    )
    .expect("json");
    assert_eq!(
        (&datei["sitzung"], &datei["fortsetzung_von"]),
        (&json!(s2), &json!(s1))
    );
    let erste: Value =
        serde_json::from_slice(&std::fs::read(w.0.join(format!("start-{s1}.json"))).expect("read"))
            .expect("json");
    assert_eq!(
        erste.get("fortsetzung_von"),
        Some(&Value::Null),
        "a plain start of version 2 has null: {erste}"
    );
    let alt = eintrag(&mut v, &s1).expect("old");
    let neu = eintrag(&mut v, &s2).expect("new");
    assert_eq!(alt["fortgesetzt_als"], json!(s2));
    assert_eq!(
        (&neu["fortsetzung_von"], &neu["name"]),
        (&json!(s1), &json!("eins"))
    );
    start_abgelehnt(
        &mut v,
        &fort(&s1),
        "schon_fortgesetzt",
        "Sitzung ist schon fortgesetzt",
    );
    // a crashed session can be brought back; it continues the old one in the same way
    ende(&w, &mut v, &s2, 7);
    start_abgelehnt(
        &mut v,
        &fort(&s1),
        "schon_fortgesetzt",
        "Sitzung ist schon fortgesetzt",
    );
    let s3 = starten(&mut v, &wieder(&s2));
    assert_eq!(
        eintrag(&mut v, &s3).map(|s| s["fortsetzung_von"].clone()),
        Some(json!(s2))
    );
    start_abgelehnt(
        &mut v,
        &wieder(&s2),
        "schon_fortgesetzt",
        "Sitzung ist schon fortgesetzt",
    );
    ende(&w, &mut v, &s3, 0);
    // a session of version 1 passes every check and is not continued: no folder, no file, no number
    let s4 = starten(&mut v, &json!({"art": "Starten", "start": gut("/tmp")}));
    ende(&w, &mut v, &s4, 9);
    let vorher = liste(&mut v).len();
    for e in [fort(&s4), wieder(&s4)] {
        start_abgelehnt(
            &mut v,
            &e,
            "nicht_fortsetzbar",
            "Sitzung lässt sich nicht fortsetzen",
        );
    }
    assert_eq!(liste(&mut v).len(), vorher, "no session started");
    let s5 = starten(&mut v, &json!({"art": "Starten", "start": gut2("/tmp")}));
    assert_eq!(s5, "s-5", "no number was used");
    ende(&w, &mut v, &s5, 0);
    p.signal("TERM");
    assert_eq!(p.warte_ende(Duration::from_secs(15)), Some(0));
}

/// DI-L3 points 4 and 5: only an ended session with its identifier as the confirmation, never the orchestrator of a running worker;
/// the entry and its ended workers leave the list, the identifier is never handed out again, the raster number is free again.
#[test]
fn loeschen_mit_bestaetigung() {
    let w = Wegwerf::neu("loeschen");
    let mut p = Prozess::starten(&w, &lauf_start(&w, &json!({"rahmen_max": 3})));
    let mut v = Probe::neu(&w.socket(), false);
    let o = starten(&mut v, &json!({"art": "Starten", "start": gut("/tmp")}));
    let lo = |s: &str, b: Value| json!({"art": "Loeschen", "sitzung": s, "bestaetigung": b});
    assert_eq!(
        grund(&mut v, &lo(&o, json!(o))).as_deref(),
        Some("sitzung_laeuft")
    );
    let s2 = starten(&mut v, &worker(&o));
    let s3 = starten(&mut v, &json!({"art": "Starten", "start": gut("/tmp")}));
    ende(&w, &mut v, &o, 0);
    assert_eq!(
        grund(&mut v, &lo(&o, json!(o))).as_deref(),
        Some("worker_laeuft")
    );
    ende(&w, &mut v, &s2, 0);
    assert_eq!(
        grund(&mut v, &json!({"art": "Loeschen", "sitzung": o})).as_deref(),
        Some("bestaetigung_fehlt")
    );
    assert_eq!(
        grund(&mut v, &lo(&o, json!(s2))).as_deref(),
        Some("bestaetigung_fehlt")
    );
    assert_eq!(
        grund(&mut v, &lo(&o, json!(1))).as_deref(),
        Some("bestaetigung_fehlt")
    );
    assert_eq!(
        grund(&mut v, &lo("s-9", json!("s-9"))).as_deref(),
        Some("sitzung_unbekannt")
    );
    for falsch in [
        json!({"art": "Loeschen", "sitzung": o, "bestaetigung": o, "x": 1}),
        json!({"art": "Loeschen", "sitzung": 3, "bestaetigung": o}),
    ] {
        assert_eq!(
            grund(&mut v, &falsch).as_deref(),
            Some("loeschen_form"),
            "{falsch}"
        );
    }
    let vorher = agenten(&v).len();
    assert_eq!(grund(&mut v, &lo(&o, json!(o))), None);
    let ids: Vec<Value> = liste(&mut v).iter().map(|s| s["id"].clone()).collect();
    assert_eq!(
        ids,
        vec![json!(s3)],
        "the orchestrator and its ended worker are gone"
    );
    assert_eq!(agenten(&v).len(), vorher + 1);
    assert_eq!(agenten(&v).last(), Some(&json!([])));
    for e in [
        json!({"art": "Umbenennen", "sitzung": o, "name": "x"}),
        json!({"art": "Fortsetzen", "sitzung": o}),
        json!({"art": "Abo", "sitzung": o, "an": true, "spalten": 40, "zeilen": 10}),
    ] {
        assert_eq!(
            grund(&mut v, &e).as_deref(),
            Some("sitzung_unbekannt"),
            "{e}"
        );
    }
    // the raster numbers 1 and 2 are free now (deleted), 3 runs: two more starts take 1 and 2, a third finds every number held
    let s4 = starten(&mut v, &json!({"art": "Starten", "start": gut("/tmp")}));
    let s5 = starten(&mut v, &json!({"art": "Starten", "start": gut("/tmp")}));
    assert_eq!(
        (s4.as_str(), s5.as_str()),
        ("s-4", "s-5"),
        "an identifier is never handed out again"
    );
    let nummern: Vec<Value> = liste(&mut v).iter().map(|s| s["nr"].clone()).collect();
    assert_eq!(nummern, vec![json!(3), json!(1), json!(2)]);
    start_abgelehnt(
        &mut v,
        &json!({"art": "Starten", "start": gut("/tmp")}),
        "nummern_erschoepft",
        "Keine Sitzungsnummer frei",
    );
    for s in [&s3, &s4, &s5] {
        ende(&w, &mut v, s, 0);
    }
    p.signal("TERM");
    assert_eq!(p.warte_ende(Duration::from_secs(15)), Some(0));
}

/// The session of `kind` cannot be deleted (DI-L3 point 4); without `lauf` the four events are protocol errors (DI3).
#[test]
fn kind_sitzung_und_ohne_lauf() {
    let w = Wegwerf::neu("kind-sitzung");
    let mut start = lauf_start(&w, &json!({}));
    start["kind"] = json!({"argv": ["/bin/sh", "-c", "exit 0"], "env": {"LC_ALL": "C.UTF-8"}});
    start["groesse"] = json!([40, 10]);
    start["anfang"]["sitzungen"] = json!([{"id": "s-alpha", "kundenraum": "raum-nord", "person": "anna", "route": "r1", "titel": "Alpha"}]);
    let mut p = Prozess::starten(&w, &start);
    let mut v = Probe::neu(&w.socket(), false);
    warte_zustand(&mut v, "s-alpha", "beendet");
    let e = eintrag(&mut v, "s-alpha").expect("entry");
    assert_eq!((&e["ordner"], &e["projekt"]), (&json!(""), &json!("")));
    let lo = json!({"art": "Loeschen", "sitzung": "s-alpha", "bestaetigung": "s-alpha"});
    assert_eq!(grund(&mut v, &lo).as_deref(), Some("nicht_loeschbar"));
    start_abgelehnt(
        &mut v,
        &json!({"art": "Fortsetzen", "sitzung": "s-alpha"}),
        "nicht_fortsetzbar",
        "Sitzung lässt sich nicht fortsetzen",
    );
    assert_eq!(
        grund(
            &mut v,
            &json!({"art": "Umbenennen", "sitzung": "s-alpha", "name": "k"})
        ),
        None
    );
    p.signal("TERM");
    assert_eq!(p.warte_ende(Duration::from_secs(15)), Some(0));
    for art in ["Umbenennen", "Loeschen", "Fortsetzen", "Wiederbeleben"] {
        let w = Wegwerf::neu("ohne-lauf");
        let mut p = Prozess::starten(
            &w,
            &startdatei(&w, &["/bin/sh", "-c", "sleep 5"], &json!({})),
        );
        let mut v = Probe::neu(&w.socket(), false);
        v.ereignis(&json!({"art": art, "sitzung": "s-alpha"}));
        assert!(
            v.warte(Duration::from_secs(10), |p| p.ende),
            "{art}: the connection ends"
        );
        assert_eq!(v.fehler, ["art_nicht_erlaubt"], "{art}");
        p.signal("TERM");
        assert_eq!(p.warte_ende(Duration::from_secs(15)), Some(0));
    }
}

/// The folder of the Bestand (mode 0700) and its file.
fn bestand_ort(w: &Wegwerf) -> (PathBuf, PathBuf) {
    let ordner = w.0.join("bestand");
    std::fs::create_dir(&ordner).expect("folder");
    std::fs::set_permissions(&ordner, std::fs::Permissions::from_mode(0o700)).expect("mode");
    let datei = ordner.join("bestand.json");
    (ordner, datei)
}

fn lesen(datei: &Path) -> Value {
    serde_json::from_slice(&std::fs::read(datei).expect("Bestand")).expect("json")
}

/// DI-L5 points 2 to 4: the Bestand after every change and at the end of the service, and the list after a restart.
#[test]
#[allow(clippy::too_many_lines, clippy::many_single_char_names)] // two runs of the service, one after the other
fn bestand_ueber_den_neustart() {
    let w = Wegwerf::neu("bestand");
    let (ordner, datei) = bestand_ort(&w);
    let start = lauf_start(&w, &json!({"bestand": datei.to_string_lossy()}));
    let mut p = Prozess::starten(&w, &start);
    let mut v = Probe::neu(&w.socket(), false);
    v.zustand();
    assert!(!datei.exists(), "no change, no file");
    let s1 = starten(
        &mut v,
        &json!({"art": "Starten", "start": gut("/home/a/proj")}),
    );
    v.zustand();
    let b = lesen(&datei);
    assert_eq!((&b["fassung"], &b["gestartet"]), (&json!(1), &json!(1)));
    assert_eq!(
        b["sitzungen"][0],
        json!({"id": s1, "harness": "claude", "modell": "m", "route": "cloud-beispiel", "kunde": "k-beispiel", "name": "",
               "ordner": "/home/a/proj", "projekt": "proj", "zustand": "laeuft", "start": gut("/home/a/proj")})
    );
    let mode = std::fs::metadata(&datei)
        .expect("meta")
        .permissions()
        .mode()
        & 0o7777;
    assert_eq!(mode, 0o600);
    assert_eq!(
        grund(
            &mut v,
            &json!({"art": "Umbenennen", "sitzung": s1, "name": "n"})
        ),
        None
    );
    assert_eq!(lesen(&datei)["sitzungen"][0]["name"], json!("n"));
    ende(&w, &mut v, &s1, 5);
    v.zustand();
    let e = lesen(&datei)["sitzungen"][0].clone();
    assert_eq!(
        (&e["zustand"], &e["code"], &e["abgestuerzt"]),
        (&json!("beendet"), &json!(5), &json!(true))
    );
    let s2 = starten(&mut v, &json!({"art": "Starten", "start": gut("/x")}));
    let s3 = starten(&mut v, &worker(&s2));
    warte_datei(&w.0.join(format!("pid-{s3}")));
    // the end of the service: every running session is `beendet` with `dienst.ende`, before the children end
    p.signal("TERM");
    assert_eq!(p.warte_ende(Duration::from_secs(15)), Some(0));
    let b = lesen(&datei);
    assert_eq!(b["gestartet"], json!(3));
    let s = b["sitzungen"].as_array().cloned().unwrap_or_default();
    assert_eq!(s.len(), 3);
    assert!(
        s[0].get("grund").is_none(),
        "an ended session keeps its end: {}",
        s[0]
    );
    assert_eq!(s[0]["abgestuerzt"], json!(true));
    for e in &s[1..] {
        assert_eq!(
            (&e["zustand"], &e["grund"], &e["abgestuerzt"]),
            (&json!("beendet"), &json!("dienst.ende"), &json!(false))
        );
        assert!(e.get("code").is_none(), "{e}");
    }
    assert_eq!(
        (&s[2]["orchestrator"], &s[2]["rolle"]),
        (&json!(s2), &json!("worker"))
    );
    let rest: Vec<String> = std::fs::read_dir(&ordner)
        .expect("folder")
        .filter_map(Result::ok)
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(rest, ["bestand.json"], "no rest of a new file");
    // an entry that still ran (a service that died without an orderly end)
    let mut b = b;
    b["sitzungen"][1]["zustand"] = json!("laeuft");
    b["sitzungen"][1].as_object_mut().map(|o| o.remove("grund"));
    b["sitzungen"][1]
        .as_object_mut()
        .map(|o| o.remove("abgestuerzt"));
    b["gestartet"] = json!(1);
    b["sitzungen"][0]["fortgesetzt_als"] = json!("s-9");
    std::fs::write(&datei, b.to_string()).expect("write");
    let mut p = Prozess::starten(&w, &start);
    let mut v = Probe::neu(&w.socket(), false);
    let l = liste(&mut v);
    let ids: Vec<Value> = l.iter().map(|s| s["id"].clone()).collect();
    assert_eq!(
        ids,
        vec![json!(s1), json!(s2), json!(s3)],
        "in the stored order"
    );
    assert!(
        l.iter()
            .all(|s| s["zustand"] == "beendet" && s["nr"].is_null()),
        "{l:?}"
    );
    assert_eq!(
        (&l[0]["code"], &l[0]["abgestuerzt"], &l[0]["name"]),
        (&json!(5), &json!(true), &json!("n"))
    );
    assert_eq!(
        (&l[1]["abgestuerzt"], &l[1]["grund"]),
        (&json!(true), &json!("dienst.neustart"))
    );
    assert!(l[1].get("code").is_none(), "{}", l[1]);
    assert_eq!(
        (&l[2]["abgestuerzt"], &l[2]["grund"]),
        (&json!(false), &json!("dienst.ende"))
    );
    assert_eq!(
        agenten(&v).last().map(|a| a[0]["worker"].clone()),
        Some(json!(s3))
    );
    assert_eq!(
        grund(
            &mut v,
            &json!({"art": "Abo", "sitzung": s1, "an": true, "spalten": 40, "zeilen": 10})
        )
        .as_deref(),
        Some("sitzung_ohne_nummer")
    );
    // the core of a session of the Bestand has ended (an input could not reach it anyway: it has no subscription)
    let z = v.zustand();
    let kern = z["kern"]["sitzungen"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let k1 = kern
        .iter()
        .find(|s| s["id"] == json!(s1))
        .cloned()
        .unwrap_or_default();
    assert_eq!(
        (&k1["zustand"], &k1["code"]),
        (&json!("kindende"), &json!(5)),
        "{k1}"
    );
    assert_eq!(z["dienst"]["gestartet"], json!(3));
    assert_eq!(
        grund(&mut v, &json!({"art": "Text", "sitzung": s1, "text": "x"})).as_deref(),
        Some("kein_abo")
    );
    // a session of the Bestand that names its continuation is `schon_fortgesetzt`; the others pass the checks and are not
    // continued yet (`nicht_fortsetzbar`)
    for e in [
        json!({"art": "Fortsetzen", "sitzung": s1}),
        json!({"art": "Wiederbeleben", "sitzung": s1}),
    ] {
        start_abgelehnt(
            &mut v,
            &e,
            "schon_fortgesetzt",
            "Sitzung ist schon fortgesetzt",
        );
    }
    start_abgelehnt(
        &mut v,
        &json!({"art": "Wiederbeleben", "sitzung": s2}),
        "nicht_fortsetzbar",
        "Sitzung lässt sich nicht fortsetzen",
    );
    // the highest identifier counts: the next start is s-4, though `gestartet` says 1
    let s4 = starten(&mut v, &json!({"art": "Starten", "start": gut("/tmp")}));
    assert_eq!(s4, "s-4");
    ende(&w, &mut v, &s4, 0);
    p.signal("TERM");
    assert_eq!(p.warte_ende(Duration::from_secs(15)), Some(0));
}

/// Runs the service with `start` and returns its exit code and its standard error; a service that does not end within 10 s is ended
/// and gives `None`.
fn start_ende(w: &Wegwerf, start: &Value) -> (Option<i32>, String) {
    let datei = datei_schreiben(w, start);
    let fehler = w.0.join("stderr-start.txt");
    let mut kind = Command::new(DIENST)
        .arg(&datei)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(std::fs::File::create(&fehler).expect("stderr file"))
        .spawn()
        .expect("wb-dienst runs");
    let ende = Instant::now() + Duration::from_secs(10);
    let code = loop {
        if let Some(status) = kind.try_wait().expect("try_wait") {
            break status.code();
        }
        if Instant::now() >= ende {
            let _ = kind.kill();
            let _ = kind.wait();
            break None;
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    (code, std::fs::read_to_string(&fehler).unwrap_or_default())
}

/// A good Bestand of exactly `n` bytes: the last byte closes the object, so a file read short is no JSON (`mit_rand`: padding with
/// spaces after the object instead, so a file read short is still JSON).
fn bestand_text(n: usize, mit_rand: bool) -> String {
    let kopf = json!({"fassung": 1, "gestartet": 0, "sitzungen": []}).to_string();
    if mit_rand {
        return format!("{kopf}{}", " ".repeat(n - kopf.len()));
    }
    let leer = r#"{"fassung":1,"gestartet":0,"sitzungen":[],"x":""}"#;
    let text = format!(
        r#"{{"fassung":1,"gestartet":0,"sitzungen":[],"x":"{}"}}"#,
        "a".repeat(n - leer.len())
    );
    assert_eq!(text.len(), n);
    text
}

/// DI-L5 points 3 and 4: a Bestand the service cannot trust ends the start; a folder with wrong rights is exit code 3 first.
#[test]
fn bestand_fail_closed() {
    let w = Wegwerf::neu("bestand-zu");
    let (ordner, datei) = bestand_ort(&w);
    let start = lauf_start(&w, &json!({"bestand": datei.to_string_lossy()}));
    let gut_inhalt = json!({"fassung": 1, "gestartet": 0, "sitzungen": []}).to_string();
    let mit = |inhalt: &str, modus: u32| {
        std::fs::write(&datei, inhalt).expect("write");
        std::fs::set_permissions(&datei, std::fs::Permissions::from_mode(modus)).expect("mode");
    };
    mit(&gut_inhalt, 0o644);
    let (code, fehler) = start_ende(&w, &start);
    assert_eq!(code, Some(2), "{fehler}");
    assert!(fehler.contains("lauf.bestand ungueltig"), "{fehler}");
    mit("[]", 0o600);
    assert_eq!(start_ende(&w, &start).0, Some(2));
    mit(&bestand_text(4_194_305, true), 0o600);
    assert_eq!(start_ende(&w, &start).0, Some(2), "one byte over the limit");
    // a link to a good Bestand is never followed
    let ziel = w.0.join("anderer.json");
    std::fs::write(&ziel, &gut_inhalt).expect("write");
    std::fs::set_permissions(&ziel, std::fs::Permissions::from_mode(0o600)).expect("mode");
    std::fs::remove_file(&datei).expect("remove");
    std::os::unix::fs::symlink(&ziel, &datei).expect("link");
    assert_eq!(
        start_ende(&w, &start).0,
        Some(2),
        "a link is never followed"
    );
    std::fs::remove_file(&datei).expect("remove");
    // a FIFO is no regular file, and opening it does not block the start
    let fifo = Command::new("mkfifo").arg(&datei).status().expect("mkfifo");
    assert!(fifo.success());
    std::fs::set_permissions(&datei, std::fs::Permissions::from_mode(0o600)).expect("mode");
    assert_eq!(start_ende(&w, &start).0, Some(2), "a FIFO");
    std::fs::remove_file(&datei).expect("remove");
    mit(&gut_inhalt, 0o600);
    std::fs::set_permissions(&ordner, std::fs::Permissions::from_mode(0o755)).expect("mode");
    let (code, fehler) = start_ende(&w, &start);
    assert_eq!(code, Some(3), "the rights come first: {fehler}");
    std::fs::set_permissions(&ordner, std::fs::Permissions::from_mode(0o700)).expect("mode");
    let mut relativ = start.clone();
    relativ["lauf"]["bestand"] = json!("bestand.json");
    assert_eq!(start_ende(&w, &relativ).0, Some(2));
    // a good file of exactly the limit starts the service
    mit(&bestand_text(4_194_304, false), 0o600);
    let mut p = Prozess::starten(&w, &start);
    let mut v = Probe::neu(&w.socket(), false);
    assert_eq!(liste(&mut v), Vec::<Value>::new());
    p.signal("TERM");
    assert_eq!(p.warte_ende(Duration::from_secs(15)), Some(0));
}

/// P26 with the Bestand and the workers (DI-L1 point 4, DI-L5 point 3): a session that leaves the list leaves the list of the workers
/// and the Bestand too.
#[test]
fn wegfall_nach_p26() {
    let w = Wegwerf::neu("p26");
    let (_, datei) = bestand_ort(&w);
    let start = lauf_start(
        &w,
        &json!({"bestand": datei.to_string_lossy(), "beendet_max": 1}),
    );
    let mut p = Prozess::starten(&w, &start);
    let mut v = Probe::neu(&w.socket(), false);
    let o = starten(&mut v, &json!({"art": "Starten", "start": gut("/tmp")}));
    let w1 = starten(&mut v, &worker(&o));
    let w2 = starten(&mut v, &worker(&o));
    ende(&w, &mut v, &w1, 0);
    ende(&w, &mut v, &w2, 0);
    let ids: Vec<Value> = liste(&mut v).iter().map(|s| s["id"].clone()).collect();
    assert_eq!(ids, vec![json!(o), json!(w2)]);
    let ids_von = |l: &Value| -> Vec<Value> {
        l.as_array()
            .map(|l| {
                l.iter()
                    .map(|x| {
                        x.get("worker")
                            .or_else(|| x.get("id"))
                            .cloned()
                            .unwrap_or_default()
                    })
                    .collect()
            })
            .unwrap_or_default()
    };
    assert_eq!(
        ids_von(&agenten(&v).last().cloned().unwrap_or_default()),
        vec![json!(w2)]
    );
    assert_eq!(
        ids_von(&lesen(&datei)["sitzungen"]),
        vec![json!(o), json!(w2)]
    );
    ende(&w, &mut v, &o, 0);
    p.signal("TERM");
    assert_eq!(p.warte_ende(Duration::from_secs(15)), Some(0));
}
