//! Tests of the duties of the build for DI12 (one service, several sessions; contract section 11): the numbers and identifiers of
//! `Starten`, `Beenden` with SIGKILL for a child that ignores SIGHUP, the folders of the sessions and their removal (also after the
//! crash of a session), the rebuild of one core, and no listener. The visible outside cases (`di12_*`) run in `faelle.rs`.
#![cfg(unix)] // the crate is Unix only (see `src/lib.rs`)

mod gemeinsam;

use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::time::{Duration, Instant};

use dienst::{Dienst, Optionen};
use gemeinsam::{DIENST, Probe, Prozess, Wegwerf, lebt, netz_sockets_pruefen, startdatei};
use serde_json::{Value, json};

/// The program of the started sessions: a script that runs the fixed child `lauf` of `wb-dienst` with its two arguments.
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

/// The settings file of the contract (fassung 1: the demonstration file of contract 2), read by the crate `einstellungen`.
fn einstellungen(w: &Wegwerf) -> String {
    let pfad = w.0.join("einst.json");
    std::fs::write(
        &pfad,
        include_str!("werkzeug/einstellungen/vorfuehrung.json"),
    )
    .expect("settings");
    pfad.to_string_lossy().into_owned()
}

/// A start file with `lauf` and no first session; `plan` is the `KIND_PLAN` of the children.
fn lauf_start(w: &Wegwerf, plan: &Value, zusatz: &Value) -> Value {
    let mut start = startdatei(w, &[DIENST], &json!({}));
    let o = start.as_object_mut().expect("object");
    o.remove("kind");
    o.remove("groesse");
    start["anfang"]["sitzungen"] = json!([]);
    start["lauf"] = json!({
        "programm": programm(w),
        "env": {"KIND_PLAN": plan.to_string(), "LC_ALL": "C.UTF-8"},
        "groesse": [40, 10],
        "einstellungen": einstellungen(w),
    });
    for (k, v) in zusatz.as_object().expect("object") {
        if k == "lauf" {
            for (lk, lv) in v.as_object().expect("object") {
                start["lauf"][lk] = lv.clone();
            }
        } else {
            start[k] = v.clone();
        }
    }
    start
}

fn gut() -> Value {
    json!({"fassung": 1, "harness": "claude", "modell": "m", "route": "cloud-beispiel", "kunde": "k-beispiel", "arbeitsordner": "/tmp"})
}

/// Sends `Starten` and waits for `Gestartet` or an refusal; returns the identifier or the refusal reason.
fn starten(p: &mut Probe, start: &Value) -> Result<String, Value> {
    let (n_nachrichten, n_ablehnungen) = (p.nachrichten.len(), p.ablehnungen.len());
    p.ereignis(&json!({"art": "Starten", "start": start}));
    let fertig = p.warte(Duration::from_secs(10), |p| {
        p.ablehnungen.len() > n_ablehnungen
            || p.nachrichten[n_nachrichten..]
                .iter()
                .any(|m| m["art"] == "Gestartet")
    });
    assert!(fertig, "no answer to Starten");
    if let Some(g) = p.nachrichten[n_nachrichten..]
        .iter()
        .find(|m| m["art"] == "Gestartet")
    {
        return Ok(g["sitzung"].as_str().expect("id").to_owned());
    }
    Err(json!(p.ablehnungen.last()))
}

/// The last list of sessions the viewer saw, after a barrier (the answer to `Zustand`).
fn liste(p: &mut Probe) -> Vec<Value> {
    let z = p.zustand();
    assert!(z.is_object());
    p.nachrichten
        .iter()
        .rev()
        .find(|m| m["art"] == "Sitzungsliste")
        .and_then(|m| m["sitzungen"].as_array().cloned())
        .unwrap_or_default()
}

fn ids(l: &[Value]) -> Vec<String> {
    l.iter()
        .map(|s| s["id"].as_str().unwrap_or("").to_owned())
        .collect()
}

/// The test mode clock only moves by the frame `Uhr`; every wait loop moves it, so that bundled pictures and timers come.
fn takt(p: &mut Probe) {
    static T: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    p.uhr(T.fetch_add(100, std::sync::atomic::Ordering::Relaxed) + 100);
}

/// Like `Probe::warte_text`, with the clock moving.
fn warte_text(p: &mut Probe, text: &str) -> bool {
    let ende = Instant::now() + Duration::from_secs(15);
    while Instant::now() < ende {
        if p.zeilen.iter().any(|z| z.contains(text)) {
            return true;
        }
        takt(p);
        p.lesen(Duration::from_millis(20));
    }
    false
}

/// Waits until the carrier of the session shows `BEREIT` (the child waits for its start byte then): a byte sent earlier would meet
/// a terminal that is not in raw mode yet.
fn warte_bereit(p: &mut Probe, sid: &str) {
    let ende = Instant::now() + Duration::from_secs(15);
    loop {
        let z = p.zustand();
        if z["dienst"]["traeger_zeilen"][sid][0] == "BEREIT" {
            return;
        }
        assert!(Instant::now() < ende, "{sid} shows no BEREIT: {z}");
        takt(p);
        p.lesen(Duration::from_millis(50));
    }
}

fn warte_zustand(p: &mut Probe, id: &str, zustand: &str) -> Value {
    let ende = Instant::now() + Duration::from_secs(15);
    loop {
        let l = liste(p);
        if let Some(s) = l.iter().find(|s| s["id"] == id)
            && s["zustand"] == zustand
        {
            return s.clone();
        }
        assert!(
            Instant::now() < ende,
            "session {id} is not {zustand}: {l:?}"
        );
        p.lesen(Duration::from_millis(50));
    }
}

#[test]
fn nummern_und_kennungen_nie_wieder_und_grenze() {
    let w = Wegwerf::neu("nummern");
    let start = lauf_start(
        &w,
        &json!({"*": {"modus": "marke", "text": "x", "sek": 60}}),
        &json!({"lauf": {"nummer_max": 3}}),
    );
    let mut p = Prozess::starten(&w, &start);
    let mut v = Probe::neu(&w.socket(), false);
    // a start that is refused (wrong form) uses no number
    assert!(starten(&mut v, &json!({"fassung": 2})).is_err());
    assert_eq!(starten(&mut v, &gut()).as_deref(), Ok("s-1"));
    assert_eq!(starten(&mut v, &gut()).as_deref(), Ok("s-2"));
    // a session that ended keeps its number: the next one is s-3, never s-1 again
    v.ereignis(&json!({"art": "Beenden", "sitzung": "s-1"}));
    warte_zustand(&mut v, "s-1", "beendet");
    assert_eq!(starten(&mut v, &gut()).as_deref(), Ok("s-3"));
    // the numbers are used up (`nummer_max` 3): the refusal carries grund, code and text
    v.ereignis(&json!({"art": "Starten", "start": gut()}));
    assert!(v.warte(Duration::from_secs(10), |p| {
        p.ablehnungen
            .last()
            .is_some_and(|g| g == "nummern_erschoepft")
    }));
    assert_eq!(
        ids(&liste(&mut v)),
        ["s-1", "s-2", "s-3"],
        "ended sessions stay in the list"
    );
    let z = v.zustand();
    assert_eq!(
        z["dienst"]["nummern"],
        json!({"s-1": 1, "s-2": 2, "s-3": 3})
    );
    assert_eq!(z["dienst"]["gestartet"], json!(3));
    p.signal("TERM");
    assert_eq!(p.warte_ende(Duration::from_secs(15)), Some(0));
}

/// DI12 point 13 with a real restart: the child makes `<lauf.sitzungen>/<session>` like "Lauf starten" does and the folder stays after the end of
/// the service; the service that starts again counts from `s-1` and must not hand out `s-1` again.
#[test]
fn nach_dem_neustart_keine_kennung_deren_ordner_besteht() {
    let w = Wegwerf::neu("kennung-neustart");
    let sitzungen = w.0.join("sitzungen");
    std::fs::create_dir(&sitzungen).expect("sitzungen");
    let skript = w.0.join("laufkind-ordner.sh");
    std::fs::write(
        &skript,
        format!(
            "#!/bin/sh\nfor f; do :; done\nmkdir \"{}/$(basename \"$(dirname \"$f\")\")\" || exit 9\nexec {DIENST} --kind lauf \"$0\" \"$@\"\n",
            sitzungen.display()
        ),
    )
    .expect("script");
    std::fs::set_permissions(&skript, std::fs::Permissions::from_mode(0o700)).expect("mode");
    let start = lauf_start(
        &w,
        &json!({"*": {"modus": "marke", "text": "x", "sek": 60}}),
        &json!({"lauf": {"programm": skript.to_string_lossy(), "sitzungen": sitzungen.to_string_lossy()}}),
    );
    // first run: s-1
    let mut p = Prozess::starten(&w, &start);
    let mut v = Probe::neu(&w.socket(), false);
    assert_eq!(starten(&mut v, &gut()).as_deref(), Ok("s-1"));
    warte_zustand(&mut v, "s-1", "laeuft");
    let ende = Instant::now() + Duration::from_secs(10);
    while !sitzungen.join("s-1").is_dir() {
        assert!(Instant::now() < ende, "the child made no folder s-1");
        std::thread::sleep(Duration::from_millis(20));
    }
    drop(v);
    p.signal("TERM");
    assert_eq!(p.warte_ende(Duration::from_secs(15)), Some(0));
    drop(p);
    // the folder of the child stays (the service removes nothing there), the socket is gone
    assert!(sitzungen.join("s-1").is_dir());
    assert!(!w.socket().exists());
    // second run: the service counts from s-1 again and skips the folder that is left
    let mut p = Prozess::starten(&w, &start);
    let mut v = Probe::neu(&w.socket(), false);
    assert_eq!(starten(&mut v, &gut()).as_deref(), Ok("s-2"));
    warte_zustand(&mut v, "s-2", "laeuft");
    assert_eq!(v.zustand()["dienst"]["gestartet"], json!(2));
    drop(v);
    p.signal("TERM");
    assert_eq!(p.warte_ende(Duration::from_secs(15)), Some(0));
    let mut namen: Vec<String> = std::fs::read_dir(&sitzungen)
        .expect("dir")
        .map(|e| e.expect("entry").file_name().to_string_lossy().into_owned())
        .collect();
    namen.sort();
    assert_eq!(
        namen,
        ["s-1", "s-2"],
        "the service made and removed nothing there"
    );
}

/// DI12 point 15 with a real restart (a finding of a trial run): the child writes `<lauf.ziellisten>/<session>.zielliste.json` like `werkbank-lauf`
/// does, never over a file that lies there (else code 73), and the session ends with a `lauf.*` code (73). The list stays after the end
/// of the service; the service that starts again counts from `s-1` and must not hand out `s-1` again.
#[test]
fn nach_dem_neustart_keine_kennung_deren_zielliste_liegt() {
    let w = Wegwerf::neu("zielliste-neustart");
    let ziellisten = w.0.join("ziellisten");
    std::fs::create_dir(&ziellisten).expect("ziellisten");
    let skript = w.0.join("laufkind-zielliste.sh");
    std::fs::write(
        &skript,
        format!(
            "#!/bin/sh\nset -C\nfor f; do :; done\nsid=$(basename \"$(dirname \"$f\")\")\nprintf '{{\"sitzung\":\"%s\"}}\\n' \"$sid\" > \"{}/$sid.zielliste.json\" || exit 73\nexec {DIENST} --kind lauf \"$0\" \"$@\"\n",
            ziellisten.display()
        ),
    )
    .expect("script");
    std::fs::set_permissions(&skript, std::fs::Permissions::from_mode(0o700)).expect("mode");
    let start = lauf_start(
        &w,
        &json!({"s-1": {"modus": "ende", "code": 73}, "*": {"modus": "marke", "text": "x", "sek": 60}}),
        &json!({"lauf": {"programm": skript.to_string_lossy(), "ziellisten": ziellisten.to_string_lossy()}}),
    );
    let liste_s1 = ziellisten.join("s-1.zielliste.json");
    // first run: s-1 writes its list and ends with code 73
    let mut p = Prozess::starten(&w, &start);
    let mut v = Probe::neu(&w.socket(), false);
    assert_eq!(starten(&mut v, &gut()).as_deref(), Ok("s-1"));
    v.ereignis(&json!({"art": "Abo", "sitzung": "s-1", "an": true, "spalten": 40, "zeilen": 10}));
    warte_bereit(&mut v, "s-1");
    v.ereignis(&json!({"art": "Text", "sitzung": "s-1", "text": "g"}));
    let s1 = warte_zustand(&mut v, "s-1", "beendet");
    assert_eq!(s1["code"], json!(73));
    let inhalt = std::fs::read(&liste_s1).expect("the child wrote the list of s-1");
    assert_eq!(inhalt, b"{\"sitzung\":\"s-1\"}\n");
    drop(v);
    p.signal("TERM");
    assert_eq!(p.warte_ende(Duration::from_secs(15)), Some(0));
    drop(p);
    // the list stays (the service removes nothing there), the socket is gone
    assert_eq!(std::fs::read(&liste_s1).expect("list"), inhalt);
    assert!(!w.socket().exists());
    // second run: the service counts from s-1 again and skips the number whose list still lies
    let mut p = Prozess::starten(&w, &start);
    let mut v = Probe::neu(&w.socket(), false);
    assert_eq!(starten(&mut v, &gut()).as_deref(), Ok("s-2"));
    warte_zustand(&mut v, "s-2", "laeuft");
    assert_eq!(v.zustand()["dienst"]["gestartet"], json!(2));
    drop(v);
    p.signal("TERM");
    assert_eq!(p.warte_ende(Duration::from_secs(15)), Some(0));
    assert_eq!(
        std::fs::read(&liste_s1).expect("list"),
        inhalt,
        "the list of s-1 is unchanged"
    );
    let mut namen: Vec<String> = std::fs::read_dir(&ziellisten)
        .expect("dir")
        .map(|e| e.expect("entry").file_name().to_string_lossy().into_owned())
        .collect();
    namen.sort();
    assert_eq!(
        namen,
        ["s-1.zielliste.json", "s-2.zielliste.json"],
        "the service made and removed nothing there"
    );
}

#[test]
fn beenden_trifft_mit_sigkill_auch_ein_kind_das_sighup_ignoriert() {
    let w = Wegwerf::neu("beenden");
    let start = lauf_start(
        &w,
        &json!({"*": {"modus": "warte", "sek": 120}}),
        &json!({}),
    );
    let mut p = Prozess::starten(&w, &start);
    let mut v = Probe::neu(&w.socket(), false);
    assert_eq!(starten(&mut v, &gut()).as_deref(), Ok("s-1"));
    v.ereignis(&json!({"art": "Abo", "sitzung": "s-1", "an": true, "spalten": 40, "zeilen": 10}));
    warte_bereit(&mut v, "s-1");
    v.ereignis(&json!({"art": "Text", "sitzung": "s-1", "text": "g"}));
    assert!(
        warte_text(&mut v, "PID="),
        "{:?} {:?} {:?}",
        v.zeilen,
        v.ablehnungen,
        v.fehler
    );
    let pid: u32 = v
        .zeilen
        .iter()
        .find_map(|z| z.trim().strip_prefix("PID="))
        .and_then(|n| n.trim().parse().ok())
        .expect("pid");
    assert!(lebt(pid));
    let vor = Instant::now();
    v.ereignis(&json!({"art": "Beenden", "sitzung": "s-1"}));
    let s = warte_zustand(&mut v, "s-1", "beendet");
    assert_eq!(
        s["code"],
        json!(137),
        "SIGKILL after SIGHUP was ignored: {s}"
    );
    assert!(
        vor.elapsed() >= Duration::from_millis(900),
        "it got the second to end by itself"
    );
    assert!(!lebt(pid), "no child survives Beenden");
    // ended or unknown sessions are refused; the list is the answer otherwise
    v.ereignis(&json!({"art": "Beenden", "sitzung": "s-1"}));
    v.ereignis(&json!({"art": "Beenden", "sitzung": "s-9"}));
    v.ereignis(&json!({"art": "Beenden", "sitzung": 5}));
    v.ereignis(&json!({"art": "Beenden", "sitzung": "s-1", "x": 1}));
    assert!(v.warte(Duration::from_secs(10), |p| p.ablehnungen.len() >= 4));
    assert_eq!(
        v.ablehnungen,
        [
            "sitzung_beendet",
            "sitzung_unbekannt",
            "beenden_form",
            "beenden_form"
        ]
    );
    p.signal("TERM");
    assert_eq!(p.warte_ende(Duration::from_secs(15)), Some(0));
}

fn modus(pfad: &Path) -> u32 {
    std::fs::metadata(pfad).expect("meta").permissions().mode() & 0o7777
}

#[test]
fn ordner_der_sitzungen_rechte_inhalt_und_aufraeumen_auch_nach_dem_absturz() {
    let w = Wegwerf::neu("ordner");
    // s-1 ends by itself with code 7 after the start byte (a crash), s-2 waits and ignores SIGHUP
    let plan = json!({"s-1": {"modus": "ende", "code": 7}, "s-2": {"modus": "warte", "sek": 120}});
    let start = lauf_start(&w, &plan, &json!({}));
    let mut p = Prozess::starten_beobachtbar(&w, &start);
    let mut v = Probe::neu(&w.socket(), false);
    assert_eq!(starten(&mut v, &gut()).as_deref(), Ok("s-1"));
    assert_eq!(starten(&mut v, &gut()).as_deref(), Ok("s-2"));
    let (o1, o2) = (w.ordner().join("s-1"), w.ordner().join("s-2"));
    for o in [&o1, &o2] {
        assert_eq!(modus(o), 0o700, "{o:?}");
        assert_eq!(modus(&o.join("start.json")), 0o600);
    }
    let inhalt: Value =
        serde_json::from_slice(&std::fs::read(o1.join("start.json")).expect("read")).expect("json");
    assert_eq!(inhalt["sitzung"], json!("s-1"));
    assert_eq!(inhalt["route"], json!("cloud-beispiel"));
    for sid in ["s-1", "s-2"] {
        v.ereignis(&json!({"art": "Abo", "sitzung": sid, "an": true, "spalten": 40, "zeilen": 10}));
    }
    warte_bereit(&mut v, "s-1");
    warte_bereit(&mut v, "s-2");
    v.ereignis(&json!({"art": "Text", "sitzung": "s-1", "text": "g"}));
    v.ereignis(&json!({"art": "Text", "sitzung": "s-2", "text": "g"}));
    let s1 = warte_zustand(&mut v, "s-1", "beendet");
    assert_eq!(s1["code"], json!(7));
    // DI12 point 12: the end of a session cleans up; the folder of the running session stays until its end
    assert!(
        !o1.exists(),
        "the folder of an ended session is removed at its end"
    );
    assert!(o2.exists(), "the folder of a running session stays");
    assert!(warte_text(&mut v, "PID="));
    let pid: u32 = v
        .zeilen
        .iter()
        .find_map(|z| z.trim().strip_prefix("PID="))
        .and_then(|n| n.trim().parse().ok())
        .expect("pid");
    // the service has no listener of any kind (after the positive control: `lsof` sees the program, see `netz_sockets_pruefen`)
    if let Err(grund) = netz_sockets_pruefen(p.kind.id(), &w.socket()) {
        panic!("{grund}");
    }
    p.signal("TERM");
    assert_eq!(p.warte_ende(Duration::from_secs(15)), Some(0));
    assert!(
        !o1.exists() && !o2.exists(),
        "the folders are removed at the end"
    );
    assert!(!lebt(pid), "no child survives the end of the service");
    assert!(!w.socket().exists());
}

#[test]
fn ein_vorhandener_ordner_wird_nie_ersetzt_und_verbraucht_keine_nummer() {
    let w = Wegwerf::neu("vorhanden");
    let start = lauf_start(
        &w,
        &json!({"*": {"modus": "marke", "text": "x", "sek": 60}}),
        &json!({}),
    );
    let mut p = Prozess::starten(&w, &start);
    let mut v = Probe::neu(&w.socket(), false);
    let alt = w.ordner().join("s-1");
    std::fs::create_dir(&alt).expect("folder");
    std::fs::write(alt.join("meins"), b"x").expect("file");
    assert_eq!(starten(&mut v, &gut()), Err(json!("start_ordner")));
    assert!(alt.join("meins").exists(), "nothing was deleted");
    std::fs::remove_dir_all(&alt).expect("remove");
    assert_eq!(
        starten(&mut v, &gut()).as_deref(),
        Ok("s-1"),
        "the number was not used"
    );
    p.signal("TERM");
    assert_eq!(p.warte_ende(Duration::from_secs(15)), Some(0));
    // a folder that was there before the start stays; here none was made by the service apart from s-1, which is gone
    assert!(!alt.exists());
}

/// DI6 with `lauf`: the core of one session that gives an error is built again; the other session and the room core stay.
#[test]
fn neubau_baut_nur_den_kern_der_einen_sitzung() {
    let w = Wegwerf::neu("neubau");
    // every started session runs the fixed child that writes one long clipboard text (53 000 bytes of messages) and `ENDE`
    let osc = w.0.join("osc.sh");
    std::fs::write(
        &osc,
        format!("#!/bin/sh\nexec {DIENST} --kind osc52viele 1 40000\n"),
    )
    .expect("script");
    std::fs::set_permissions(&osc, std::fs::Permissions::from_mode(0o700)).expect("mode");
    let start = lauf_start(
        &w,
        &json!({}),
        &json!({"lauf": {"programm": osc.to_string_lossy()}}),
    );
    let cfg = dienst::pruefen(&start).expect("start file");
    let optionen = Optionen {
        huelle: Some(DIENST.into()),
        kern_grenze: Some(30_000),
    };
    let mut service = Dienst::neu_mit(cfg, &optionen).expect("service starts");
    let anhalter = service.anhalter();
    let faden = std::thread::spawn(move || service.schleife());
    let mut v = Probe::neu(&w.socket(), false);
    assert_eq!(starten(&mut v, &gut()).as_deref(), Ok("s-1"));
    assert_eq!(starten(&mut v, &gut()).as_deref(), Ok("s-2"));
    for s in ["s-1", "s-2"] {
        v.ereignis(&json!({"art": "Abo", "sitzung": s, "an": true, "spalten": 40, "zeilen": 10}));
    }
    let z = v.zustand();
    assert_eq!(
        z["dienst"]["kernbau"],
        json!(3),
        "room core and two sessions: {z}"
    );
    warte_bereit(&mut v, "s-1");
    // only the first session gets its start byte: its clipboard message passes the limit and its core is built again
    v.ereignis(&json!({"art": "Text", "sitzung": "s-1", "text": "g"}));
    assert!(
        warte_text(&mut v, "ENDE"),
        "the picture after the rebuild: {:?}",
        v.zeilen
    );
    let z = v.zustand();
    assert_eq!(
        z["dienst"]["kernbau"],
        json!(4),
        "one core was built again: {z}"
    );
    assert!(
        !v.arten().iter().any(|a| a == "Zwischenablage"),
        "the effects of the failed step are lost"
    );
    let abos = z["kern"]["betrachter"]["b1"]["abos"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    assert_eq!(abos.len(), 2, "both subscriptions stay: {z}");
    assert_eq!(z["kern"]["sitzungen"].as_array().map(Vec::len), Some(2));
    anhalter.anhalten();
    let ende = Instant::now() + Duration::from_secs(15);
    while !faden.is_finished() && Instant::now() < ende {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(faden.is_finished());
    assert_eq!(faden.join().expect("loop"), 0);
    assert!(
        !w.ordner().join("s-1").exists(),
        "the folders are removed at the end"
    );
}
