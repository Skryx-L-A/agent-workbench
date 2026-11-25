//! Tests of the service as a program and as a library: the start errors (DI1, DI2, DI8), the end (DI2 point 7, DI8), the hard
//! bound of the queue under a steady load (DI5), the error of the core and its rebuild (DI6), and the order of events with
//! several threads at work (section 11 of the contract). The 97 visible outside cases run in `faelle.rs`.
#![cfg(unix)] // the crate is Unix only (see `src/lib.rs`)
#![cfg(feature = "pruefbau")] // the fixed child programs `--kind` exist in the test build only; `naht.rs` runs in both builds

mod gemeinsam;

use std::os::unix::fs::PermissionsExt;
use std::process::Command;
use std::time::Duration;

use dienst::{Dienst, Optionen};
use gemeinsam::{DIENST, Probe, Prozess, Wegwerf, datei_schreiben, lebt, startdatei};
use serde_json::{Value, json};

/// Starts the program with `args`; returns the exit code, standard output and standard error.
fn lauf(args: &[&str]) -> (Option<i32>, String, Vec<String>) {
    let aus = Command::new(DIENST).args(args).output().expect("runs");
    let zeilen = String::from_utf8_lossy(&aus.stderr)
        .lines()
        .map(str::to_owned)
        .collect();
    (
        aus.status.code(),
        String::from_utf8_lossy(&aus.stdout).into_owned(),
        zeilen,
    )
}

/// The program must end with `code`, one line on stderr that starts with `wb-dienst: `, nothing on stdout, no socket file.
fn start_scheitert(w: &Wegwerf, start: &Value, code: i32, was: &str) {
    let datei = datei_schreiben(w, start);
    let (ist, aus, zeilen) = lauf(&[&datei.to_string_lossy()]);
    assert_eq!(ist, Some(code), "{was}: {zeilen:?}");
    assert_eq!(aus, "", "{was}: stdout stays empty");
    assert_eq!(
        zeilen.len(),
        1,
        "{was}: exactly one line on stderr: {zeilen:?}"
    );
    assert!(
        zeilen[0].starts_with("wb-dienst: ") && zeilen[0].len() > 11,
        "{was}: {zeilen:?}"
    );
    assert!(!w.socket().exists(), "{was}: no socket file is left");
}

#[test]
fn aufruf_ohne_datei() {
    for args in [vec![], vec!["a", "b"]] {
        let (code, aus, zeilen) = lauf(&args);
        assert_eq!(code, Some(2));
        assert_eq!(aus, "");
        assert_eq!(zeilen.len(), 1);
        assert!(zeilen[0].starts_with("wb-dienst: "));
    }
    let (code, _, zeilen) = lauf(&["/nicht/da/start.json"]);
    assert_eq!(code, Some(2));
    assert_eq!(zeilen.len(), 1);
}

#[test]
fn startfehler_und_ihre_reihenfolge() {
    let w = Wegwerf::neu("start");
    let gut = startdatei(&w, &["/bin/echo"], &json!({}));
    // DI1: exit code 2
    let mut kaputt = gut.clone();
    kaputt["unbekannt"] = json!(1);
    start_scheitert(&w, &kaputt, 2, "unknown key");
    // DI8: exit code 4, nothing is left behind
    start_scheitert(
        &w,
        &startdatei(&w, &["/nirgends/programm"], &json!({})),
        4,
        "program missing",
    );
    let mut kein_ordner = gut.clone();
    kein_ordner["kind"]["cwd"] = json!("/nirgends/ordner");
    start_scheitert(&w, &kein_ordner, 4, "working folder missing");
    let nicht_ausfuehrbar = w.0.join("kein-programm");
    std::fs::write(&nicht_ausfuehrbar, b"#!/bin/sh\n").expect("write");
    std::fs::set_permissions(&nicht_ausfuehrbar, std::fs::Permissions::from_mode(0o644))
        .expect("mode");
    start_scheitert(
        &w,
        &startdatei(&w, &[&nicht_ausfuehrbar.to_string_lossy()], &json!({})),
        4,
        "program not executable",
    );
    start_scheitert(
        &w,
        &startdatei(&w, &["/tmp"], &json!({})),
        4,
        "program is a folder",
    );
    // DI2: exit code 3, before the child (the child is missing as well)
    std::fs::set_permissions(w.ordner(), std::fs::Permissions::from_mode(0o755)).expect("mode");
    start_scheitert(&w, &gut, 3, "folder mode 0755");
    start_scheitert(
        &w,
        &startdatei(&w, &["/nirgends/programm"], &json!({})),
        3,
        "rights before child",
    );
    // DI1 before DI2: the start file is judged first
    start_scheitert(&w, &kaputt, 2, "start file before rights");
    std::fs::set_permissions(w.ordner(), std::fs::Permissions::from_mode(0o700)).expect("mode");
    // a path that exists blocks the start, and the service leaves it as it is
    std::fs::write(w.socket(), b"fremd").expect("write");
    let datei = datei_schreiben(&w, &gut);
    let (code, _, zeilen) = lauf(&[&datei.to_string_lossy()]);
    assert_eq!(code, Some(3));
    assert_eq!(zeilen.len(), 1);
    assert_eq!(std::fs::read(w.socket()).expect("read"), b"fremd");
}

#[test]
fn sigterm_beendet_alles_auch_ein_kind_das_sighup_ignoriert() {
    let w = Wegwerf::neu("term");
    let start = startdatei(
        &w,
        &[DIENST, "--kind", "warte", "60"],
        &json!({"pruefmodus": false}),
    );
    let mut p = Prozess::starten(&w, &start);
    let mut c = Probe::neu(&w.socket(), false);
    c.abo(40, 10);
    assert!(c.warte_text("BEREIT"));
    c.text("g");
    assert!(c.warte_text("PID="));
    let zeile = c
        .zeilen
        .iter()
        .find(|z| z.contains("PID="))
        .cloned()
        .expect("line");
    let pid: u32 = zeile
        .trim()
        .trim_start_matches("PID=")
        .trim()
        .parse()
        .expect("pid");
    assert!(lebt(pid), "the child runs");
    p.signal("TERM");
    assert_eq!(
        p.warte_ende(Duration::from_secs(15)),
        Some(0),
        "exit code 0 after SIGTERM"
    );
    assert!(!w.socket().exists(), "the socket file is removed");
    let frist = std::time::Instant::now() + Duration::from_secs(5);
    while lebt(pid) && std::time::Instant::now() < frist {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(
        !lebt(pid),
        "the child that ignores SIGHUP is ended (SIGKILL)"
    );
    c.lesen(Duration::from_millis(200));
    assert!(c.ende, "the viewer sees the end of the connection");
}

#[test]
fn sigint_beendet_auch() {
    let w = Wegwerf::neu("int");
    let start = startdatei(
        &w,
        &[DIENST, "--kind", "ausgabewarte", "48", "60"],
        &json!({"pruefmodus": false}),
    );
    let mut p = Prozess::starten(&w, &start);
    let mut c = Probe::neu(&w.socket(), false);
    c.abo(40, 10);
    assert!(c.warte_text("BEREIT"));
    c.text("g");
    assert!(c.warte_text("H"));
    p.signal("INT");
    assert_eq!(p.warte_ende(Duration::from_secs(15)), Some(0));
    assert!(!w.socket().exists());
}

#[test]
fn start_mit_ordner_und_socket_rechten() {
    use std::os::unix::fs::MetadataExt;
    let w = Wegwerf::neu("rechte");
    let start = startdatei(
        &w,
        &[DIENST, "--kind", "ausgabewarte", "48", "30"],
        &json!({}),
    );
    let mut p = Prozess::starten(&w, &start);
    // `Prozess::starten` returns as soon as the socket file is there, that is right after `bind`; the service sets the mode 0600
    // only after that and listens last (`rechte::socket_binden`). A connection succeeds only once it listens, so the mode is
    // read after the first connection: the state a client finds, never the moment between `bind` and `chmod`.
    let _c = Probe::neu(&w.socket(), false);
    let meta = std::fs::symlink_metadata(w.socket()).expect("socket");
    assert_eq!(meta.mode() & 0o7777, 0o600, "socket mode");
    assert_eq!(
        std::fs::metadata(w.ordner()).expect("folder").mode() & 0o7777,
        0o700,
        "the service does not change the folder"
    );
    p.signal("TERM");
    assert_eq!(p.warte_ende(Duration::from_secs(15)), Some(0));
}

/// Starts the program and returns as soon as the socket file is there, with no pause in the poll: what the program has done
/// by then is what a client that connects at once finds.
fn starten_bis_socket(w: &Wegwerf) -> std::process::Child {
    let start = startdatei(
        w,
        &[DIENST, "--kind", "ausgabewarte", "48", "30"],
        &json!({}),
    );
    let datei = datei_schreiben(w, &start);
    let kind = Command::new(DIENST)
        .arg(&datei)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .expect("wb-dienst starts");
    let frist = std::time::Instant::now() + Duration::from_secs(10);
    while std::fs::symlink_metadata(w.socket()).is_err() {
        assert!(
            std::time::Instant::now() < frist,
            "the socket did not appear"
        );
    }
    kind
}

/// Exit code 0 means the signal was caught before the socket was there: the default action of SIGTERM ends the process by the
/// signal (`None`) and leaves the socket and the child behind.
#[test]
fn sigterm_gleich_nach_dem_socket_ergibt_0_und_keinen_socket() {
    for _ in 0..20 {
        let w = Wegwerf::neu("signal-start");
        let mut kind = starten_bis_socket(&w);
        let status = Command::new("kill")
            .args(["-TERM", &kind.id().to_string()])
            .status()
            .expect("kill runs");
        assert!(status.success());
        let ende = std::time::Instant::now() + Duration::from_secs(15);
        let code = loop {
            if let Some(st) = kind.try_wait().expect("try_wait") {
                break st.code();
            }
            if std::time::Instant::now() >= ende {
                let _ = kind.kill();
                let _ = kind.wait();
                panic!("the program did not end");
            }
            std::thread::sleep(Duration::from_millis(5));
        };
        assert_eq!(code, Some(0), "SIGTERM right after the socket");
        assert!(!w.socket().exists(), "the socket is removed");
    }
}

/// The order of the start, on Linux: in the moment the socket file is there, SIGTERM and SIGINT are caught already
/// (`SigCgt` in `/proc/<pid>/status`). On macOS the order holds by the code of `programm`: the signals are caught first.
#[cfg(target_os = "linux")]
#[test]
fn die_signale_sind_abgefangen_wenn_der_socket_erscheint() {
    const SIGINT_BIT: u64 = 1 << (2 - 1);
    const SIGTERM_BIT: u64 = 1 << (15 - 1);
    for _ in 0..20 {
        let w = Wegwerf::neu("sigcgt");
        let mut kind = starten_bis_socket(&w);
        let status = std::fs::read_to_string(format!("/proc/{}/status", kind.id()));
        let _ = kind.kill();
        let _ = kind.wait();
        let status = status.expect("status of the process");
        let zeile = status
            .lines()
            .find(|z| z.starts_with("SigCgt:"))
            .expect("SigCgt line");
        let maske =
            u64::from_str_radix(zeile.trim_start_matches("SigCgt:").trim(), 16).expect("hex mask");
        assert_eq!(maske & SIGTERM_BIT, SIGTERM_BIT, "SIGTERM caught: {zeile}");
        assert_eq!(maske & SIGINT_BIT, SIGINT_BIT, "SIGINT caught: {zeile}");
    }
}

/// DI5, steady load: a viewer that never reads gets clipboard contents of 90 000 bytes one after the other; when the queue
/// passes the bound it is cut off, one line goes to the log with name and reason and no content, the other viewer and the
/// service stay, and a new viewer gets a full picture.
#[test]
fn dauerlast_trennt_den_betrachter_der_nie_liest() {
    let w = Wegwerf::neu("last");
    let start = startdatei(
        &w,
        &[DIENST, "--kind", "osc52viele", "12", "90000", "GEHEIMWORT"],
        &json!({"warteschlange_max": 200_000}),
    );
    let mut p = Prozess::starten(&w, &start);
    let mut traege = Probe::neu(&w.socket(), false);
    traege.abo(40, 10);
    let mut wach = Probe::neu(&w.socket(), false);
    wach.abo(40, 10);
    let mut t = 0;
    assert!(wach.warte_text_takt("BEREIT", &mut t));
    // the viewer that never reads is the one who types: the clipboard goes to him
    traege.text("g");
    assert!(
        wach.warte_text_takt("ENDE", &mut t),
        "the child wrote everything (end of the connection: {:?}, errors: {:?}, log: {:?}, service: {:?})",
        wach.ende_grund,
        wach.fehler,
        p.stderr_zeilen(),
        p.kind.try_wait()
    );
    let frist = std::time::Instant::now() + Duration::from_secs(10);
    let mut zeilen = p.stderr_zeilen();
    while zeilen.is_empty() && std::time::Instant::now() < frist {
        wach.lesen(Duration::from_millis(50));
        zeilen = p.stderr_zeilen();
    }
    assert_eq!(zeilen.len(), 1, "{zeilen:?}");
    let z = &zeilen[0];
    assert!(
        z.starts_with("wb-dienst: betrachter b1 getrennt: rueckstau_gerissen (ausstehend "),
        "{z}"
    );
    assert!(z.ends_with(" Byte, Grenze 200000 Byte)"), "{z}");
    assert!(
        !z.contains("GEHEIMWORT") && !z.contains("Alpha"),
        "no content in the log: {z}"
    );
    // the viewer that reads is still there and the state counts it
    let zustand = wach.zustand();
    assert!(zustand["dienst"]["betrachter"]["b2"].is_object());
    assert!(
        zustand["dienst"]["betrachter"]["b1"].is_null(),
        "b1 is gone"
    );
    assert_eq!(zustand["dienst"]["kernbau"], json!(1));
    // a new viewer gets a full picture and the name b3 (a name is never given again)
    let mut neu = Probe::neu(&w.socket(), false);
    neu.abo(40, 10);
    assert!(
        neu.warte_text("ENDE"),
        "the picture is full: {:?}",
        neu.zeilen
    );
    let z2 = neu.zustand();
    assert!(z2["dienst"]["betrachter"]["b3"].is_object());
    p.signal("TERM");
    assert_eq!(p.warte_ende(Duration::from_secs(15)), Some(0));
}

/// DI6: a step of the core that makes more bytes of messages than its limit is an error; the service builds the core again from
/// the state of the carrier and every viewer gets a full picture. Here the limit is lowered through `Optionen`.
#[test]
fn grenze_der_nachrichten_je_schritt_fuehrt_zum_neubau() {
    let ordner = Wegwerf::neu("kern");
    let start = startdatei(
        &ordner,
        &[DIENST, "--kind", "osc52viele", "1", "40000"],
        &json!({}),
    );
    let cfg = dienst::pruefen(&start).expect("start file");
    let optionen = Optionen {
        huelle: Some(DIENST.into()),
        kern_grenze: Some(30_000),
    };
    let mut service = Dienst::neu_mit(cfg, &optionen).expect("service starts");
    let anhalter = service.anhalter();
    let faden = std::thread::spawn(move || service.schleife());
    let mut probe = Probe::neu(&ordner.socket(), false);
    probe.abo(40, 10);
    let mut takt = 0;
    assert!(probe.warte_text_takt("BEREIT", &mut takt));
    assert_eq!(probe.zustand()["dienst"]["kernbau"], json!(1));
    let verbunden = probe
        .arten()
        .iter()
        .filter(|a| *a == "Einstellungen")
        .count();
    assert_eq!(verbunden, 1);
    probe.text("g");
    // the clipboard message (53 000 bytes) passes the limit of 30 000: the core is built again
    assert!(probe.warte(Duration::from_secs(10), |p| {
        p.arten().iter().filter(|a| *a == "Einstellungen").count() == 2
    }));
    assert!(
        probe.warte_text("ENDE"),
        "the picture after the rebuild: {:?}",
        probe.zeilen
    );
    let zustand = probe.zustand();
    assert_eq!(zustand["dienst"]["kernbau"], json!(2));
    assert!(
        !probe.arten().iter().any(|a| a == "Zwischenablage"),
        "the effects of the failed step are lost"
    );
    assert!(
        zustand["kern"]["betrachter"]["b1"]["abos"]
            .as_array()
            .is_some_and(|a| a.len() == 1),
        "{zustand}"
    );
    anhalter.anhalten();
    assert_eq!(faden_ende(faden), 0);
    assert!(!ordner.socket().exists());
}

/// DI6: the core is built from the state of the carrier, with its modes; several rebuilds in a row are no error.
#[test]
fn neubau_nimmt_die_modi_des_traegers_und_darf_sich_wiederholen() {
    let w = Wegwerf::neu("modi");
    // ESC [ ? 5 h (reverse video) and ESC [ ? 2004 h (bracketed paste)
    let start = startdatei(
        &w,
        &[
            DIENST,
            "--kind",
            "ausgabewarte",
            "1b5b3f35681b5b3f3230303468",
            "60",
        ],
        &json!({}),
    );
    let mut p = Prozess::starten(&w, &start);
    let mut probe = Probe::neu(&w.socket(), false);
    probe.abo(40, 10);
    let mut takt = 0;
    assert!(probe.warte_text_takt("BEREIT", &mut takt));
    probe.text("g");
    let modi = |z: &Value| {
        (
            z["kern"]["sitzungen"][0]["invers"] == json!(true),
            z["kern"]["sitzungen"][0]["bracketed_paste"] == json!(true),
        )
    };
    let frist = std::time::Instant::now() + Duration::from_secs(10);
    while modi(&probe.zustand()) != (true, true) {
        assert!(
            std::time::Instant::now() < frist,
            "the modes did not arrive"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    for _ in 0..6 {
        probe.kernfehler();
    }
    let z = probe.zustand();
    assert_eq!(
        z["dienst"]["kernbau"],
        json!(7),
        "six rebuilds after the first core: {z}"
    );
    assert_eq!(
        modi(&z),
        (true, true),
        "the new core knows the modes of the carrier"
    );
    assert_eq!(probe.fehler, Vec::<String>::new());
    p.signal("TERM");
    assert_eq!(p.warte_ende(Duration::from_secs(15)), Some(0));
}

/// DI7: the clock may stand still (the same `t` again) but never fall.
#[test]
fn uhr_gleich_ist_erlaubt_und_zurueck_ist_ein_fehler() {
    let w = Wegwerf::neu("uhr");
    let start = startdatei(
        &w,
        &[DIENST, "--kind", "ausgabewarte", "48", "60"],
        &json!({}),
    );
    let mut p = Prozess::starten(&w, &start);
    let mut probe = Probe::neu(&w.socket(), false);
    probe.abo(40, 10);
    probe.uhr(100);
    probe.uhr(100);
    probe.uhr(150);
    let z = probe.zustand();
    assert!(z["dienst"]["betrachter"]["b1"].is_object());
    assert!(probe.fehler.is_empty() && !probe.ende, "{:?}", probe.fehler);
    probe.uhr(149);
    assert!(probe.warte(Duration::from_secs(10), |q| q.fehler == ["uhr_faellt"]));
    assert!(
        probe.warte(Duration::from_secs(10), |q| q.ende),
        "the connection ends"
    );
    p.signal("TERM");
    assert_eq!(p.warte_ende(Duration::from_secs(15)), Some(0));
}

/// The picture of the screen and the state of the core of one run of a fixed scenario, for the comparison of runs.
fn szenario() -> (Vec<String>, Vec<String>, Value) {
    szenario_mit("0")
}

/// [`szenario`] with a child that sleeps `nach_ende` seconds after `ENDE` before it ends (`bilder 3 <seconds>`).
///
/// The end of the child is part of the compared sequence: the state is read only after the service has seen that end (`kindende`).
/// Read at `ENDE`, a run could show `laeuft` or `kindende`, depending only on how fast the service saw the end (decision of 22.11.2026:
/// one run in 296 under load).
fn szenario_mit(nach_ende: &str) -> (Vec<String>, Vec<String>, Value) {
    let w = Wegwerf::neu("faeden");
    let start = startdatei(
        &w,
        &[DIENST, "--kind", "bilder", "3", nach_ende],
        &json!({"deflate": true}),
    );
    let mut p = Prozess::starten(&w, &start);
    let mut c1 = Probe::neu(&w.socket(), true);
    c1.abo(40, 10);
    let mut c2 = Probe::neu(&w.socket(), true);
    c2.abo(40, 10);
    assert!(c1.warte_text("BEREIT") && c2.warte_text("BEREIT"));
    c2.text("g");
    for k in 1..=3_u64 {
        c2.uhr(100 * k);
        c2.text("x");
        assert!(c2.warte_text(&format!("Bild {k:04}")), "picture {k}");
        // the clock is the only timer: the picture of the other viewer follows with the next beat
        c2.uhr(100 * k + 50);
        assert!(
            c1.warte_text(&format!("Bild {k:04}")),
            "picture {k} for the second viewer"
        );
    }
    c2.text("x");
    assert!(c2.warte_text("ENDE"), "ENDE");
    c2.uhr(450);
    assert!(c1.warte_text("ENDE"), "ENDE for the second viewer");
    let mut z1 = c1.zustand();
    let frist = std::time::Instant::now() + Duration::from_secs(10);
    while z1["kern"]["sitzungen"][0]["zustand"] != "kindende" && std::time::Instant::now() < frist {
        c1.lesen(Duration::from_millis(20));
        z1 = c1.zustand();
    }
    assert_eq!(
        z1["kern"]["sitzungen"][0]["zustand"], "kindende",
        "the service saw the end of the child"
    );
    let z2 = c2.zustand();
    let kern =
        json!({"sitzungen": z1["kern"]["sitzungen"], "betrachter": z2["kern"]["betrachter"]});
    p.signal("TERM");
    assert_eq!(p.warte_ende(Duration::from_secs(15)), Some(0));
    (c1.zeilen.clone(), c2.zeilen.clone(), kern)
}

/// Section 11 of the contract: several threads (readers of the viewers, the reader of the PTY, the loop) give the same sequence
/// as one thread would: eight runs at the same time, each with two viewers, give equal pictures and an equal state.
#[test]
fn mehrere_faeden_geben_dieselbe_folge() {
    let ergebnisse: Vec<(Vec<String>, Vec<String>, Value)> = std::thread::scope(|s| {
        let faeden: Vec<_> = (0..8).map(|_| s.spawn(szenario)).collect();
        faeden.into_iter().map(|f| f.join().expect("run")).collect()
    });
    let erstes = &ergebnisse[0];
    assert!(
        erstes.0.iter().any(|z| z.contains("Bild 0003")),
        "{:?}",
        erstes.0
    );
    assert_eq!(erstes.0, erstes.1, "both viewers show the same picture");
    for (i, e) in ergebnisse.iter().enumerate() {
        assert_eq!(e, erstes, "run {i} differs from run 0");
    }
}

/// The compared sequence does not depend on when the child ends: a child that ends at once after `ENDE` and one that sleeps
/// 2 s first give the same pictures and the same state. Read without waiting for `kindende`, the second shows `laeuft`.
#[test]
fn die_folge_haengt_nicht_vom_zeitpunkt_des_kindendes_ab() {
    let sofort = szenario_mit("0");
    let spaeter = szenario_mit("2");
    assert_eq!(sofort, spaeter);
}

/// A service that is dropped without a loop still removes its socket and ends its child.
#[test]
fn ein_dienst_der_nie_lief_raeumt_beim_verwerfen_auf() {
    let w = Wegwerf::neu("drop");
    let start = startdatei(
        &w,
        &[DIENST, "--kind", "ausgabewarte", "48", "60"],
        &json!({}),
    );
    let cfg = dienst::pruefen(&start).expect("start file");
    let optionen = Optionen {
        huelle: Some(DIENST.into()),
        kern_grenze: None,
    };
    let service = Dienst::neu_mit(cfg, &optionen).expect("service starts");
    assert!(w.socket().exists());
    drop(service);
    assert!(!w.socket().exists(), "the socket file is removed");
}

/// An idle service sleeps: with no timer due the loop turns about every 2 ms, not in a tight loop.
#[test]
fn ein_ruhender_dienst_schlaeft() {
    for pruefmodus in [true, false] {
        let w = Wegwerf::neu("ruhe");
        let start = startdatei(
            &w,
            &[DIENST, "--kind", "ausgabewarte", "48", "60"],
            &json!({"pruefmodus": pruefmodus}),
        );
        let cfg = dienst::pruefen(&start).expect("start file");
        let optionen = Optionen {
            huelle: Some(DIENST.into()),
            kern_grenze: None,
        };
        let mut service = Dienst::neu_mit(cfg, &optionen).expect("service starts");
        let anhalter = service.anhalter();
        let faden = std::thread::spawn(move || service.schleife());
        std::thread::sleep(Duration::from_millis(500));
        let runden = anhalter.runden();
        anhalter.anhalten();
        assert_eq!(faden_ende(faden), 0);
        assert!(
            runden > 20,
            "the loop turns (pruefmodus {pruefmodus}): {runden}"
        );
        assert!(
            runden < 400,
            "the loop sleeps between turns (pruefmodus {pruefmodus}): {runden}"
        );
    }
}

/// The service in a thread of this process, ended by `Anhalter`.
fn im_faden(w: &Wegwerf, start: &Value) -> (dienst::Anhalter, std::thread::JoinHandle<i32>) {
    let cfg = dienst::pruefen(start).expect("start file");
    let optionen = Optionen {
        huelle: Some(DIENST.into()),
        kern_grenze: None,
    };
    let mut service = Dienst::neu_mit(cfg, &optionen).expect("service starts");
    assert!(w.socket().exists());
    let anhalter = service.anhalter();
    (anhalter, std::thread::spawn(move || service.schleife()))
}

/// Waits for the end of the loop thread, at most 15 s.
fn faden_ende(faden: std::thread::JoinHandle<i32>) -> i32 {
    let frist = std::time::Instant::now() + Duration::from_secs(15);
    while !faden.is_finished() && std::time::Instant::now() < frist {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(faden.is_finished(), "the loop ends after `anhalten`");
    faden.join().expect("loop thread")
}

/// DI8: at the end the child gets SIGHUP and time to end by itself (up to 1 s); what ignores SIGHUP is killed.
#[test]
fn ende_des_dienstes_gibt_dem_kind_zeit_und_beendet_notfalls_hart() {
    // a child that ends by itself after SIGHUP, but only after 0.3 s: the file shows that it got the time
    let w = Wegwerf::neu("sanft");
    let fertig = w.0.join("fertig");
    let gestartet = w.0.join("gestartet");
    let skript = format!(
        "trap 'sleep 0.3; echo ok > {}; exit 0' HUP; echo ok > {}; while true; do sleep 0.05; done",
        fertig.display(),
        gestartet.display()
    );
    let start = startdatei(&w, &["/bin/sh", "-c", &skript], &json!({}));
    let (anhalter, faden) = im_faden(&w, &start);
    let frist = std::time::Instant::now() + Duration::from_secs(10);
    while !gestartet.exists() && std::time::Instant::now() < frist {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(gestartet.exists(), "the child runs");
    anhalter.anhalten();
    assert_eq!(faden_ende(faden), 0);
    assert!(fertig.exists(), "the child had time to end by itself");
    // a child that ignores SIGHUP is killed
    let w = Wegwerf::neu("hart");
    let start = startdatei(
        &w,
        &[DIENST, "--kind", "warte", "60"],
        &json!({"pruefmodus": false}),
    );
    let (anhalter, faden) = im_faden(&w, &start);
    let mut probe = Probe::neu(&w.socket(), false);
    probe.abo(40, 10);
    assert!(probe.warte_text("BEREIT"));
    probe.text("g");
    assert!(probe.warte_text("PID="));
    let zeile = probe
        .zeilen
        .iter()
        .find(|z| z.contains("PID="))
        .cloned()
        .expect("line");
    let pid: u32 = zeile
        .trim()
        .trim_start_matches("PID=")
        .trim()
        .parse()
        .expect("pid");
    assert!(lebt(pid));
    anhalter.anhalten();
    assert_eq!(faden_ende(faden), 0);
    assert!(!lebt(pid), "the child that ignores SIGHUP is gone");
}
