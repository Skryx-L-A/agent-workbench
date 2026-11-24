//! Tests of the subreaper (contract `dienst`, section 17; Linux only): the program `wb-dienst` adopts the orphans of its sessions,
//! collects them without taking the status of its own children, ends the orphans of an ended session (SIGHUP, up to 1 s, SIGKILL),
//! leaves processes with a session of their own standing and ends the orphans of every session at its own end.
//!
//! The program of a session is a shell script (`lauf.programm`) that leaves orphans behind and writes their pids into the throw-away
//! folder (`WAISEN_ORDNER`). Every process these tests leave is ended through a pidfd at the end of the test.

#![cfg(target_os = "linux")]

mod gemeinsam;

use std::os::unix::fs::PermissionsExt;
use std::time::{Duration, Instant};

use gemeinsam::{DIENST, Probe, Prozess, Wegwerf, lebt, startdatei};
use serde_json::{Value, json};

/// A start file with `lauf` and no first session; the program of the sessions is the script `text`.
fn lauf_start(w: &Wegwerf, text: &str) -> Value {
    let programm = w.0.join("lauf.sh");
    std::fs::write(&programm, text).expect("script");
    std::fs::set_permissions(&programm, std::fs::Permissions::from_mode(0o700)).expect("mode");
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
        "programm": programm.to_string_lossy(),
        "env": {"PATH": "/usr/bin:/bin", "LC_ALL": "C.UTF-8", "WAISEN_ORDNER": w.0.to_string_lossy()},
        "groesse": [40, 10],
        "einstellungen": einst.to_string_lossy(),
    });
    start
}

fn gut() -> Value {
    json!({"fassung": 1, "harness": "claude", "modell": "m", "route": "cloud-beispiel", "kunde": "k-beispiel", "arbeitsordner": "/tmp"})
}

/// Sends `Starten` and returns the identifier of `Gestartet`.
fn starten(p: &mut Probe) -> String {
    let n = p.nachrichten.len();
    p.ereignis(&json!({"art": "Starten", "start": gut()}));
    assert!(
        p.warte(Duration::from_secs(10), |p| p.nachrichten[n..]
            .iter()
            .any(|m| m["art"] == "Gestartet")),
        "no Gestartet: {:?}",
        p.ablehnungen
    );
    p.nachrichten[n..]
        .iter()
        .find(|m| m["art"] == "Gestartet")
        .and_then(|m| m["sitzung"].as_str())
        .expect("id")
        .to_owned()
}

/// The entry of the session `id` in the last list, after a barrier.
fn eintrag(p: &mut Probe, id: &str) -> Option<Value> {
    let z = p.zustand();
    assert!(z.is_object());
    p.nachrichten
        .iter()
        .rev()
        .find(|m| m["art"] == "Sitzungsliste")
        .and_then(|m| m["sitzungen"].as_array().cloned())
        .and_then(|l| l.into_iter().find(|s| s["id"] == id))
}

/// Reads the pid in the file `name` of the throw-away folder, waiting for it at most 10 s.
fn pid_datei(w: &Wegwerf, name: &str) -> u32 {
    let ende = Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(pid) = std::fs::read_to_string(w.0.join(name))
            .ok()
            .and_then(|t| t.trim().parse().ok())
        {
            return pid;
        }
        assert!(Instant::now() < ende, "{name} was not written");
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// Fields 3 (state) and 4 (parent) of `/proc/<pid>/stat`.
fn zustand_und_eltern(pid: u32) -> Option<(char, u32)> {
    let text = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    let mut f = text[text.rfind(')')? + 1..].split_ascii_whitespace();
    let zustand = f.next()?.chars().next()?;
    Some((zustand, f.next()?.parse().ok()?))
}

/// Waits at most `frist` until `pid` is gone (also no zombie: a zombie answers `kill -0`); returns whether it went.
fn warte_weg(pid: u32, frist: Duration) -> bool {
    let ende = Instant::now() + frist;
    while lebt(pid) {
        if Instant::now() >= ende {
            return false;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    true
}

/// Ends a process the test left behind, through a pidfd (no signal to a number whose process is not held).
fn beenden(pid: u32) {
    let Some(p) = i32::try_from(pid)
        .ok()
        .and_then(rustix::process::Pid::from_raw)
    else {
        return;
    };
    if let Ok(fd) = rustix::process::pidfd_open(p, rustix::process::PidfdFlags::empty()) {
        let _ = rustix::process::pidfd_send_signal(&fd, rustix::process::Signal::KILL);
    }
}

/// Ends the listed processes when the test ends, also after a failed assertion.
struct Aufraeumen(Vec<u32>);

impl Drop for Aufraeumen {
    fn drop(&mut self) {
        for pid in &self.0 {
            beenden(*pid);
        }
    }
}

/// Waits until the session `id` is `beendet` and returns its code.
fn warte_beendet(v: &mut Probe, id: &str, frist: Duration) -> Option<Value> {
    let ende = Instant::now() + frist;
    loop {
        if let Some(s) = eintrag(v, id)
            && s["zustand"] == "beendet"
        {
            return Some(s["code"].clone());
        }
        if Instant::now() >= ende {
            return None;
        }
        v.lesen(Duration::from_millis(20));
    }
}

#[test]
fn ein_eigenes_kind_und_eine_waise_enden_zugleich_der_code_stimmt_und_die_waise_ist_geerntet() {
    let w = Wegwerf::neu("waise-zugleich");
    // the orphan ends the moment the program is gone (`kill -0` fails once the carrier has collected it); its output does not hold
    // the PTY, so the end of the child and the end of the orphan meet
    let start = lauf_start(
        &w,
        "#!/bin/sh\np=$$\n( while kill -0 $p 2>/dev/null; do :; done; exit 5 ) </dev/null >/dev/null 2>&1 &\n\
         echo $! > \"$WAISEN_ORDNER/waise.pid\"\nexit 7\n",
    );
    let mut p = Prozess::starten(&w, &start);
    let mut v = Probe::neu(&w.socket(), false);
    for runde in 0..5 {
        let _ = std::fs::remove_file(w.0.join("waise.pid"));
        let id = starten(&mut v);
        let waise = pid_datei(&w, "waise.pid");
        let _weg = Aufraeumen(vec![waise]);
        assert_eq!(
            warte_beendet(&mut v, &id, Duration::from_secs(15)),
            Some(json!(7)),
            "round {runde}: Kindende has the code of the own child (its status was not taken)"
        );
        assert!(
            warte_weg(waise, Duration::from_secs(5)),
            "round {runde}: the orphan was collected (no zombie): {:?}",
            zustand_und_eltern(waise)
        );
    }
    p.signal("TERM");
    assert_eq!(p.warte_ende(Duration::from_secs(15)), Some(0));
}

#[test]
fn die_waisen_einer_beendeten_sitzung_enden_mit_sighup_und_sigkill_ein_eigenes_setsid_bleibt() {
    let w = Wegwerf::neu("waisen-jagd");
    // `taub` ignores SIGHUP and holds the PTY: only SIGKILL ends it, after the second. `gruppe` leaves the foreground group (the
    // kernel's SIGHUP at the end of the session leader does not reach it) and ends by the SIGHUP of the service. `waechter` has a
    // session of its own and stays.
    let start = lauf_start(
        &w,
        "#!/bin/sh\nd=\"$WAISEN_ORDNER\"\n( trap '' HUP; exec sleep 120 ) &\necho $! > \"$d/taub.pid\"\n\
         /usr/bin/python3 -I -c 'import os, time; os.setpgid(0, 0); open(os.environ[\"WAISEN_ORDNER\"] + \"/gruppe.tmp\", \"w\").write(str(os.getpid())); os.rename(os.environ[\"WAISEN_ORDNER\"] + \"/gruppe.tmp\", os.environ[\"WAISEN_ORDNER\"] + \"/gruppe.pid\"); time.sleep(120)' &\n\
         setsid sh -c 'echo $$ > \"$1/waechter.tmp\"; mv \"$1/waechter.tmp\" \"$1/waechter.pid\"; exec sleep 120' sh \"$d\" </dev/null >/dev/null 2>&1 &\n\
         while [ ! -s \"$d/gruppe.pid\" ] || [ ! -s \"$d/waechter.pid\" ]; do sleep 0.01; done\n\
         sleep 0.5\n: > \"$d/ende\"\nexit 3\n",
    );
    let mut p = Prozess::starten(&w, &start);
    let dienst = p.kind.id();
    let mut v = Probe::neu(&w.socket(), false);
    let id = starten(&mut v);
    let taub = pid_datei(&w, "taub.pid");
    let gruppe = pid_datei(&w, "gruppe.pid");
    let waechter = pid_datei(&w, "waechter.pid");
    let _weg = Aufraeumen(vec![taub, gruppe, waechter]);
    // the program lives on for half a second after its orphans exist (a hunt that began too early would find nothing and end); it
    // writes `ende` right before its end, and from then on the orphans belong to the service
    let ende = Instant::now() + Duration::from_secs(15);
    while !w.0.join("ende").exists() {
        assert!(Instant::now() < ende, "the program did not reach its end");
        std::thread::sleep(Duration::from_millis(2));
    }
    let ende_lauf = Instant::now();
    let mut gruppe_weg = None;
    let mut taub_weg = None;
    let frist = ende_lauf + Duration::from_secs(15);
    while (gruppe_weg.is_none() || taub_weg.is_none()) && Instant::now() < frist {
        if gruppe_weg.is_none() && !lebt(gruppe) {
            gruppe_weg = Some(ende_lauf.elapsed());
        }
        if taub_weg.is_none() && !lebt(taub) {
            taub_weg = Some(ende_lauf.elapsed());
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    let (Some(dauer_gruppe), Some(dauer_taub)) = (gruppe_weg, taub_weg) else {
        panic!(
            "orphans of the ended session survived: gruppe {:?} taub {:?}",
            zustand_und_eltern(gruppe),
            zustand_und_eltern(taub)
        );
    };
    assert!(
        dauer_gruppe < Duration::from_millis(800),
        "SIGHUP ended the orphan that takes it: {dauer_gruppe:?}"
    );
    assert!(
        dauer_taub >= Duration::from_millis(900),
        "SIGKILL only after the second: {dauer_taub:?}"
    );
    assert_eq!(
        warte_beendet(&mut v, &id, Duration::from_secs(15)),
        Some(json!(3)),
        "Kindende with the code of the program, once the orphans no longer hold the PTY"
    );
    assert_eq!(
        zustand_und_eltern(waechter).map(|(_, e)| e),
        Some(dienst),
        "the watcher with its own session lives on, adopted by the service"
    );
    assert!(lebt(waechter));
    // its end is collected by the service as well
    beenden(waechter);
    assert!(
        warte_weg(waechter, Duration::from_secs(5)),
        "the ended watcher was collected: {:?}",
        zustand_und_eltern(waechter)
    );
    p.signal("TERM");
    assert_eq!(p.warte_ende(Duration::from_secs(15)), Some(0));
}

#[test]
fn das_dienstende_beendet_die_waisen_jeder_sitzung() {
    let w = Wegwerf::neu("waisen-dienstende");
    // the program runs until the service ends it (SIGHUP); its child ignores SIGHUP and becomes an orphan of the service then
    let start = lauf_start(
        &w,
        "#!/bin/sh\n( trap '' HUP; exec sleep 120 ) </dev/null >/dev/null 2>&1 &\necho $! > \"$WAISEN_ORDNER/taub.pid\"\n\
         exec sleep 120\n",
    );
    let mut p = Prozess::starten(&w, &start);
    let mut v = Probe::neu(&w.socket(), false);
    let _id = starten(&mut v);
    let taub = pid_datei(&w, "taub.pid");
    let _weg = Aufraeumen(vec![taub]);
    assert!(lebt(taub));
    p.signal("TERM");
    assert_eq!(p.warte_ende(Duration::from_secs(20)), Some(0));
    assert!(
        warte_weg(taub, Duration::from_secs(2)),
        "the orphan of the session did not outlive the service: {:?}",
        zustand_und_eltern(taub)
    );
    assert!(!w.socket().exists());
}
