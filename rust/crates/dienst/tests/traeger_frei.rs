//! P26 (Nachtrag 23.11.2026): an ended session gives its carrier back. Once `Kindende` is processed the PTY master is closed, the
//! threads of the carrier end, and the child is collected; the entry, the code and the last picture stay (DI12 point 5).
//! Before, every ended session kept two `/dev/ptmx` descriptors and a thread until the end of the service, and macOS (511 PTYs
//! for the whole system) refused new starts after about 400 to 450 of them.

#![cfg(feature = "pruefbau")] // the fixed child programs `--kind` exist in the test build only

mod gemeinsam;

use std::os::unix::fs::PermissionsExt;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use gemeinsam::{DIENST, Probe, Prozess, Wegwerf, startdatei};
use serde_json::{Value, json};

/// Sessions started and ended one after the other in one run.
const ZAHL: usize = 24;

fn programm(w: &Wegwerf) -> String {
    let pfad = w.0.join("laufkind.sh");
    std::fs::write(
        &pfad,
        format!("#!/bin/sh\nexec {DIENST} --kind lauf \"$0\" \"$@\"\n"),
    )
    .expect("script");
    std::fs::set_permissions(&pfad, std::fs::Permissions::from_mode(0o700)).expect("mode");
    pfad.to_string_lossy().into_owned()
}

fn lauf_start(w: &Wegwerf) -> Value {
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
        "env": {"KIND_PLAN": json!({"*": {"modus": "ende", "code": 0}}).to_string(), "LC_ALL": "C.UTF-8"},
        "groesse": [40, 10],
        "einstellungen": einst.to_string_lossy(),
    });
    start
}

fn gut() -> Value {
    json!({"fassung": 1, "harness": "claude", "modell": "m", "route": "cloud-beispiel", "kunde": "k-beispiel", "arbeitsordner": "/tmp"})
}

fn takt(p: &mut Probe) {
    static T: AtomicU64 = AtomicU64::new(0);
    p.uhr(T.fetch_add(100, Ordering::Relaxed) + 100);
}

fn starten(p: &mut Probe) -> String {
    let n = p.nachrichten.len();
    p.ereignis(&json!({"art": "Starten", "start": gut()}));
    assert!(
        p.warte(Duration::from_secs(30), |p| p.nachrichten[n..]
            .iter()
            .any(|m| m["art"] == "Gestartet")),
        "no Gestartet; rejections {:?}",
        p.ablehnungen
    );
    let g = p.nachrichten[n..]
        .iter()
        .find(|m| m["art"] == "Gestartet")
        .expect("Gestartet");
    g["sitzung"].as_str().expect("id").to_owned()
}

fn warte_zustand(p: &mut Probe, was: &str, bis: impl Fn(&Value) -> bool) -> Value {
    let ende = Instant::now() + Duration::from_secs(30);
    loop {
        let z = p.zustand();
        if bis(&z) {
            return z;
        }
        assert!(Instant::now() < ende, "{was}: {z}");
        takt(p);
        p.lesen(Duration::from_millis(5));
    }
}

/// Lets the child of `id` run to its end and waits until the state shows the end.
fn beenden_lassen(v: &mut Probe, id: &str) {
    v.ereignis(&json!({"art": "Abo", "sitzung": id, "an": true, "spalten": 40, "zeilen": 10}));
    warte_zustand(v, "BEREIT", |z| {
        z["dienst"]["traeger_zeilen"][id][0] == "BEREIT"
    });
    v.ereignis(&json!({"art": "Text", "sitzung": id, "text": "g"}));
    v.ereignis(&json!({"art": "Abo", "sitzung": id, "an": false}));
    warte_zustand(v, "Kindende", |z| z["dienst"]["kind"][id]["ende"] == true);
    if v.nachrichten.len() > 64 {
        let rest = v.nachrichten.split_off(v.nachrichten.len() - 8);
        v.nachrichten = rest;
    }
}

/// The open PTY masters of the service (`lsof -nP -p <pid>`, lines with `/dev/ptmx`; Linux names them `/dev/ptmx` or `ptmx`).
fn ptmx_zahl(pid: u32) -> usize {
    let aus = Command::new("lsof")
        .args(["-nP", "-p", &pid.to_string()])
        .output()
        .expect("lsof");
    String::from_utf8_lossy(&aus.stdout)
        .lines()
        .filter(|z| z.contains("ptmx"))
        .count()
}

/// The threads of the service (`ps -M` on macOS, `/proc/<pid>/task` on Linux).
fn faeden(pid: u32) -> usize {
    if let Ok(eintraege) = std::fs::read_dir(format!("/proc/{pid}/task")) {
        return eintraege.count();
    }
    let aus = Command::new("ps")
        .args(["-M", "-p", &pid.to_string()])
        .output()
        .expect("ps");
    String::from_utf8_lossy(&aus.stdout)
        .lines()
        .count()
        .saturating_sub(1)
}

#[test]
fn beendete_sitzungen_geben_pty_und_faeden_zurueck_und_behalten_bild_und_code() {
    let w = Wegwerf::neu("traeger-frei");
    let mut p = Prozess::starten(&w, &lauf_start(&w));
    let pid = p.kind.id();
    let mut v = Probe::neu(&w.socket(), false);
    // one session first: the threads the service starts once are there then
    let erste = starten(&mut v);
    beenden_lassen(&mut v, &erste);
    let (ptmx_anfang, faeden_anfang) = (ptmx_zahl(pid), faeden(pid));
    let mut ids = vec![erste];
    for _ in 1..ZAHL {
        let id = starten(&mut v);
        beenden_lassen(&mut v, &id);
        ids.push(id);
    }
    let (ptmx_ende, faeden_ende) = (ptmx_zahl(pid), faeden(pid));
    // before P26 every ended session kept 2 lines `/dev/ptmx` and 1 thread: 46 and 23 more here
    assert!(
        ptmx_ende <= ptmx_anfang + 2,
        "PTY masters grew with ended sessions: {ptmx_anfang} after 1, {ptmx_ende} after {ZAHL}"
    );
    assert!(
        faeden_ende <= faeden_anfang + 2,
        "threads grew with ended sessions: {faeden_anfang} after 1, {faeden_ende} after {ZAHL}"
    );
    // the entry, the code and the last picture stay
    let z = p_zustand(&mut v);
    for id in &ids {
        assert_eq!(z["dienst"]["kind"][id]["ende"], true, "{id}: {z}");
        assert_eq!(z["dienst"]["kind"][id]["code"], 0, "{id}: {z}");
        assert_eq!(
            z["dienst"]["traeger_zeilen"][id][0], "BEREIT",
            "{id}: the last picture stays: {z}"
        );
    }
    // a size and bytes for an ended session break nothing: the service answers and keeps running
    let alt = &ids[0];
    v.ereignis(&json!({"art": "Abo", "sitzung": alt, "an": true, "spalten": 30, "zeilen": 8}));
    v.ereignis(&json!({"art": "Text", "sitzung": alt, "text": "x"}));
    v.ereignis(&json!({"art": "Abo", "sitzung": alt, "an": false}));
    let z = p_zustand(&mut v);
    assert_eq!(z["dienst"]["kind"][alt]["ende"], true, "{z}");
    assert!(
        p.kind.try_wait().ok().flatten().is_none(),
        "the service still runs"
    );
    // a new start still works after the ended ones
    let neu = starten(&mut v);
    beenden_lassen(&mut v, &neu);
    p.signal("TERM");
    assert_eq!(p.warte_ende(Duration::from_secs(60)), Some(0));
}

fn p_zustand(v: &mut Probe) -> Value {
    let z = v.zustand();
    assert!(z.is_object(), "{z}");
    z
}
