//! P26 Teil 2 (Projektentscheid, 23.11.2026): the `Sitzungsliste` holds at most 256 ended sessions of `Starten` (`lauf.beendet_max`, in test mode
//! smaller). A later end that raises the count above the limit takes out the entry that ended longest ago and that no viewer
//! subscribes; the end itself takes nothing out for its own session.

#![cfg(unix)] // the Windows build runs the outside cases in the VM (contract section 21), not these tests
#![cfg(feature = "pruefbau")] // the fixed child programs `--kind` exist in the test build only

mod gemeinsam;

use std::os::unix::fs::PermissionsExt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use gemeinsam::{DIENST, Probe, Prozess, Wegwerf, startdatei};
use serde_json::{Value, json};

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

/// A start file with `lauf`; `beendet_max` lowers the limit (test mode), `None` keeps the default.
fn lauf_start(w: &Wegwerf, beendet_max: Option<u64>) -> Value {
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
    if let Some(n) = beendet_max {
        start["lauf"]["beendet_max"] = json!(n);
    }
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

/// The last `Sitzungsliste` after a barrier.
fn liste(p: &mut Probe) -> Vec<Value> {
    let z = p.zustand();
    assert!(z.is_object(), "{z}");
    p.nachrichten
        .iter()
        .rev()
        .find(|m| m["art"] == "Sitzungsliste")
        .and_then(|m| m["sitzungen"].as_array().cloned())
        .unwrap_or_default()
}

fn ids(l: &[Value]) -> Vec<String> {
    l.iter()
        .map(|s| s["id"].as_str().expect("id").to_owned())
        .collect()
}

/// Starts a session, lets its child run to the end, and waits until the list shows it ended. With `abo_bleibt` the viewer keeps
/// the subscription after the end.
fn ein_ende(v: &mut Probe, abo_bleibt: bool) -> String {
    let id = starten(v);
    v.ereignis(&json!({"art": "Abo", "sitzung": id, "an": true, "spalten": 40, "zeilen": 10}));
    warte_zustand(v, "BEREIT", |z| {
        z["dienst"]["traeger_zeilen"][&id][0] == "BEREIT"
    });
    v.ereignis(&json!({"art": "Text", "sitzung": id, "text": "g"}));
    if !abo_bleibt {
        v.ereignis(&json!({"art": "Abo", "sitzung": id, "an": false}));
    }
    let ende = Instant::now() + Duration::from_secs(30);
    loop {
        let l = liste(v);
        if l.iter().any(|s| s["id"] == id && s["zustand"] == "beendet") {
            break;
        }
        assert!(Instant::now() < ende, "{id} did not end: {l:?}");
        v.lesen(Duration::from_millis(5));
    }
    if v.nachrichten.len() > 64 {
        let rest = v.nachrichten.split_off(v.nachrichten.len() - 8);
        v.nachrichten = rest;
    }
    id
}

/// (1) and (3): 257 ended sessions with the default limit; the oldest is out, the other 256 stay with their code. Without the
/// limit the list holds 257 entries and the case is red.
#[test]
fn nach_257_beendeten_fehlt_die_aelteste_und_256_bleiben_mit_code() {
    let wurf = Wegwerf::neu("liste-grenze-257");
    let mut dienst = Prozess::starten(&wurf, &lauf_start(&wurf, None));
    let mut probe = Probe::neu(&wurf.socket(), false);
    let mut alle = Vec::new();
    for _ in 0..256 {
        alle.push(ein_ende(&mut probe, false));
    }
    let eintraege = liste(&mut probe);
    assert_eq!(eintraege.len(), 256, "at 256 ended nothing is out yet");
    alle.push(ein_ende(&mut probe, false));
    let eintraege = liste(&mut probe);
    assert_eq!(
        eintraege.len(),
        256,
        "the list holds at most 256 ended sessions"
    );
    assert_eq!(
        ids(&eintraege),
        alle[1..].to_vec(),
        "the oldest is out, in the order of the start"
    );
    for eintrag in &eintraege {
        assert_eq!(eintrag["zustand"], "beendet", "{eintrag}");
        assert_eq!(eintrag["code"], 0, "{eintrag}");
    }
    // the dropped session is gone from the state and unknown to the service
    let zustand = probe.zustand();
    assert!(
        zustand["dienst"]["nummern"].get(&alle[0]).is_none(),
        "{zustand}"
    );
    let zahl = probe.ablehnungen.len();
    probe.ereignis(
        &json!({"art": "Abo", "sitzung": alle[0], "an": true, "spalten": 40, "zeilen": 10}),
    );
    assert!(
        probe.warte(Duration::from_secs(10), |probe| probe.ablehnungen.len()
            > zahl)
    );
    assert_eq!(probe.ablehnungen[zahl], "sitzung_unbekannt");
    // starts go on
    let neu = ein_ende(&mut probe, false);
    let eintraege = liste(&mut probe);
    assert_eq!(eintraege.len(), 256);
    assert_eq!(ids(&eintraege).last(), Some(&neu));
    dienst.signal("TERM");
    assert_eq!(dienst.warte_ende(Duration::from_secs(60)), Some(0));
}

/// (2): a subscribed old entry stays; the next free one is out instead. After `Abo` off it goes at the next end, not before.
#[test]
fn ein_abonnierter_alter_eintrag_bleibt_und_der_naechste_freie_faellt() {
    let w = Wegwerf::neu("liste-grenze-abo");
    let mut p = Prozess::starten(&w, &lauf_start(&w, Some(2)));
    let mut v = Probe::neu(&w.socket(), false);
    let s1 = ein_ende(&mut v, true);
    let s2 = ein_ende(&mut v, false);
    assert_eq!(ids(&liste(&mut v)), vec![s1.clone(), s2.clone()]);
    let s3 = ein_ende(&mut v, false);
    assert_eq!(
        ids(&liste(&mut v)),
        vec![s1.clone(), s3.clone()],
        "s-1 is subscribed: s-2 is out"
    );
    let s4 = ein_ende(&mut v, false);
    assert_eq!(ids(&liste(&mut v)), vec![s1.clone(), s4.clone()]);
    // `Abo` off takes nothing out by itself
    v.ereignis(&json!({"art": "Abo", "sitzung": s1, "an": false}));
    assert_eq!(ids(&liste(&mut v)), vec![s1.clone(), s4.clone()]);
    let s5 = ein_ende(&mut v, false);
    assert_eq!(ids(&liste(&mut v)), vec![s4, s5]);
    p.signal("TERM");
    assert_eq!(p.warte_ende(Duration::from_secs(60)), Some(0));
}

/// The end itself takes nothing out for its own session: with the limit 1 and the only older entry subscribed, the list holds both.
#[test]
fn das_ende_selbst_nimmt_die_eigene_sitzung_nicht_heraus() {
    let w = Wegwerf::neu("liste-grenze-eigen");
    let mut p = Prozess::starten(&w, &lauf_start(&w, Some(1)));
    let mut v = Probe::neu(&w.socket(), false);
    let s1 = ein_ende(&mut v, true);
    let s2 = ein_ende(&mut v, false);
    assert_eq!(ids(&liste(&mut v)), vec![s1, s2]);
    p.signal("TERM");
    assert_eq!(p.warte_ende(Duration::from_secs(60)), Some(0));
}
