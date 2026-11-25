//! The three ways a child ends (DI12 point 6, DI8) with children of the system (`/bin/sh`), so that they run in the build without the
//! feature `pruefbau`: `Beenden` sends SIGHUP, SIGKILL follows for a child that ignores it, and the end of the service by SIGTERM ends
//! the running children. The tests of the same duties with the fixed child programs `--kind` are in `mehrere.rs` and `dienst.rs`.
//! Every wait is for a state (a file, the end of the process, the state of the session) with a deadline.
#![cfg(unix)] // the crate is Unix only (see `src/lib.rs`)

mod gemeinsam;

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use gemeinsam::{DIENST, Probe, Prozess, Wegwerf, lebt, startdatei};
use serde_json::{Value, json};

/// The child: a shell script. It writes its pid into `pid-<number>` next to itself (the number is the identifier of its start file, so
/// two sessions do not meet), then waits in short steps. `falle` is the line that sets the trap on SIGHUP.
fn programm(w: &Wegwerf, falle: &str) -> String {
    let pfad = w.0.join("kind.sh");
    let skript = format!(
        "#!/bin/sh\n{falle}\necho $$ > \"{}/pid-$$\"\nwhile :; do sleep 0.05; done\n",
        w.0.display()
    );
    std::fs::write(&pfad, skript).expect("script");
    std::fs::set_permissions(&pfad, std::fs::Permissions::from_mode(0o700)).expect("mode");
    pfad.to_string_lossy().into_owned()
}

/// A start file with `lauf` and no first session; the program is the script `programm`.
fn lauf_start(w: &Wegwerf, falle: &str) -> Value {
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
        "programm": programm(w, falle),
        "env": {"LC_ALL": "C.UTF-8"},
        "groesse": [40, 10],
        "einstellungen": einst.to_string_lossy(),
    });
    start
}

fn gut() -> Value {
    json!({"fassung": 1, "harness": "claude", "modell": "m", "route": "cloud-beispiel", "kunde": "k-beispiel", "arbeitsordner": "/tmp"})
}

/// Sends `Starten` and waits for `Gestartet`; returns the identifier.
fn starten(v: &mut Probe) -> String {
    let vorher = v.nachrichten.len();
    v.ereignis(&json!({"art": "Starten", "start": gut()}));
    assert!(
        v.warte(Duration::from_secs(10), |p| p.nachrichten[vorher..]
            .iter()
            .any(|m| m["art"] == "Gestartet")),
        "no `Gestartet`: refusals {:?}, errors {:?}",
        v.ablehnungen,
        v.fehler
    );
    v.nachrichten[vorher..]
        .iter()
        .find(|m| m["art"] == "Gestartet")
        .and_then(|m| m["sitzung"].as_str())
        .expect("identifier")
        .to_owned()
}

/// The pids the children wrote, once there are `n` of them (at most 15 s).
fn pids(w: &Wegwerf, n: usize) -> Vec<u32> {
    let ende = Instant::now() + Duration::from_secs(15);
    loop {
        let mut pids: Vec<u32> = std::fs::read_dir(&w.0)
            .expect("folder")
            .filter_map(Result::ok)
            .filter_map(|e| {
                let name = e.file_name().to_string_lossy().into_owned();
                let nr = name.strip_prefix("pid-")?.to_owned();
                // the file is written whole or not yet read: an empty file is a start that is not done
                std::fs::read_to_string(e.path())
                    .ok()?
                    .trim()
                    .parse::<u32>()
                    .ok()?;
                nr.parse().ok()
            })
            .collect();
        pids.sort_unstable();
        if pids.len() >= n {
            return pids;
        }
        assert!(
            Instant::now() < ende,
            "{n} children did not start: {pids:?}"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// The state of session `id` in the list, waiting for `zustand` (at most 15 s).
fn warte_zustand(v: &mut Probe, id: &str, zustand: &str) -> Value {
    let ende = Instant::now() + Duration::from_secs(15);
    loop {
        v.zustand();
        let liste = v
            .nachrichten
            .iter()
            .rev()
            .find(|m| m["art"] == "Sitzungsliste")
            .and_then(|m| m["sitzungen"].as_array().cloned())
            .unwrap_or_default();
        if let Some(s) = liste.iter().find(|s| s["id"] == id)
            && s["zustand"] == zustand
        {
            return s.clone();
        }
        assert!(
            Instant::now() < ende,
            "session {id} is not {zustand}: {liste:?}"
        );
        v.lesen(Duration::from_millis(50));
    }
}

/// Waits until the process `pid` is gone (at most 10 s).
fn warte_weg(pid: u32, was: &str) {
    let ende = Instant::now() + Duration::from_secs(10);
    while lebt(pid) && Instant::now() < ende {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(!lebt(pid), "{was}");
}

fn warte_datei(pfad: &Path) -> bool {
    let ende = Instant::now() + Duration::from_secs(10);
    while !pfad.exists() && Instant::now() < ende {
        std::thread::sleep(Duration::from_millis(10));
    }
    pfad.exists()
}

/// `Beenden` sends SIGHUP: a child that ends on SIGHUP is gone and has seen the signal (the file `hup`); without the signal it would run on.
#[test]
fn beenden_schickt_sighup_und_das_kind_endet() {
    let w = Wegwerf::neu("beenden-hup");
    let falle = format!("trap 'echo hup > \"{}/hup\"; exit 0' HUP", w.0.display());
    let start = lauf_start(&w, &falle);
    let mut p = Prozess::starten(&w, &start);
    let mut v = Probe::neu(&w.socket(), false);
    let sid = starten(&mut v);
    let pid = pids(&w, 1)[0];
    assert!(lebt(pid), "the child runs");
    v.ereignis(&json!({"art": "Beenden", "sitzung": sid}));
    warte_zustand(&mut v, &sid, "beendet");
    assert!(
        warte_datei(&w.0.join("hup")),
        "the child got SIGHUP and ended by itself"
    );
    warte_weg(pid, "the child that ended on SIGHUP is gone");
    p.signal("TERM");
    assert_eq!(p.warte_ende(Duration::from_secs(15)), Some(0));
}

/// A child that ignores SIGHUP ends after the second of grace by SIGKILL (code 137); it is gone then, not before.
#[test]
fn beenden_trifft_ein_kind_das_sighup_ignoriert_nach_der_frist_mit_sigkill() {
    let w = Wegwerf::neu("beenden-kill");
    let start = lauf_start(&w, "trap '' HUP");
    let mut p = Prozess::starten(&w, &start);
    let mut v = Probe::neu(&w.socket(), false);
    let sid = starten(&mut v);
    let pid = pids(&w, 1)[0];
    assert!(lebt(pid), "the child runs");
    let vor = Instant::now();
    v.ereignis(&json!({"art": "Beenden", "sitzung": sid}));
    let s = warte_zustand(&mut v, &sid, "beendet");
    assert_eq!(s["code"], json!(137), "SIGKILL after SIGHUP: {s}");
    assert!(
        vor.elapsed() >= Duration::from_millis(900),
        "it got the second to end by itself"
    );
    warte_weg(pid, "no child survives `Beenden`");
    p.signal("TERM");
    assert_eq!(p.warte_ende(Duration::from_secs(15)), Some(0));
}

/// SIGTERM to the service ends the running children of all sessions: each gets SIGHUP and the time to end by itself (here 0.3 s, the file
/// `fertig-<pid>` shows it), and the service waits for them before it ends.
#[test]
fn sigterm_beendet_die_laufenden_kinder_und_gibt_ihnen_zeit() {
    let w = Wegwerf::neu("term-kinder");
    let falle = format!(
        "trap 'sleep 0.3; echo ok > \"{}/fertig-$$\"; exit 0' HUP",
        w.0.display()
    );
    let start = lauf_start(&w, &falle);
    let mut p = Prozess::starten(&w, &start);
    let mut v = Probe::neu(&w.socket(), false);
    starten(&mut v);
    starten(&mut v);
    let pids = pids(&w, 2);
    assert!(pids.iter().all(|p| lebt(*p)), "both children run");
    p.signal("TERM");
    assert_eq!(
        p.warte_ende(Duration::from_secs(15)),
        Some(0),
        "exit code 0 after SIGTERM"
    );
    for pid in pids {
        let fertig: PathBuf = w.0.join(format!("fertig-{pid}"));
        assert!(
            fertig.exists(),
            "the child {pid} had the time to end by itself"
        );
        assert!(!lebt(pid), "the child {pid} is gone");
    }
}
