//! Protections beyond the case set: exact modes under a hostile umask, no replacing without `--neu` under a race,
//! the real home stays untouched, the program ends the CLI on a signal, a second call without `--neu`, an inherited hard limit 0 for core.
//! The program runs with the seams of the test build (feature `pruefbau`); the delivered build has none (`tests/naht.rs`).
#![cfg(all(unix, feature = "pruefbau"))] // the crate is Unix only (see `src/lib.rs`)

mod gemeinsam;

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::process::{Command, Stdio};
use std::time::Duration;

use gemeinsam::{
    Ausgang, BIN, attrappe_text, aufbau, ausgang, befehl, dateien, skript_schreiben, token_text,
};
use serde_json::json;

fn token() -> String {
    token_text(&json!({"praefix": "gho_", "fuell": "Q", "laenge": 40}))
}

#[test]
fn modes_are_exact_under_a_hostile_umask() {
    let a = aufbau();
    let t = token();
    skript_schreiben(&a.skript, &attrappe_text(&a, "erfolg", &t));
    let b = befehl(&a, &["copilot".to_owned()]);
    let mut c = Command::new("/bin/sh");
    c.arg("-c")
        .arg("umask 0277; exec \"$0\" \"$@\"")
        .arg(BIN)
        .arg("copilot")
        .env_clear();
    for (k, v) in b.get_envs() {
        if let Some(v) = v {
            c.env(k, v);
        }
    }
    let aus: Ausgang = ausgang(
        c.stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn"),
    );
    assert_eq!(aus.exit, 0, "{}", aus.stderr);
    let datei = a.ziel.join("copilot.schluessel");
    assert_eq!(
        fs::metadata(&datei).expect("meta").permissions().mode() & 0o7777,
        0o600
    );
    let log = fs::read_to_string(&a.log).expect("log");
    assert!(
        log.contains("modus:drwx------"),
        "home folders are 0700 under umask 0277"
    );
    assert!(
        log.contains("settingsmodus:-rw-------"),
        "settings are 0600 under umask 0277"
    );
}

#[test]
fn a_second_call_without_neu_leaves_the_first_key_and_does_not_start_the_cli() {
    let a = aufbau();
    let t = token();
    skript_schreiben(&a.skript, &attrappe_text(&a, "erfolg", &t));
    let erst = gemeinsam::laufen(&a, &["copilot".to_owned()]);
    assert_eq!(erst.exit, 0, "{}", erst.stderr);
    fs::remove_file(&a.log).expect("log");
    let zweit = gemeinsam::laufen(&a, &["copilot".to_owned()]);
    assert_eq!(zweit.exit, 15);
    assert!(!a.log.exists(), "the CLI did not start");
    assert_eq!(
        fs::read_to_string(a.ziel.join("copilot.schluessel")).expect("read"),
        format!("{t}\n")
    );
    let dritt = gemeinsam::laufen(&a, &["copilot".to_owned(), "--neu".to_owned()]);
    assert_eq!(dritt.exit, 0, "{}", dritt.stderr);
}

#[test]
fn a_link_at_the_target_counts_as_a_file_and_is_never_followed() {
    let a = aufbau();
    skript_schreiben(&a.skript, &attrappe_text(&a, "erfolg", &token()));
    let fremd = a.wurzel.join("fremd.txt");
    fs::write(&fremd, "fremd\n").expect("file");
    std::os::unix::fs::symlink(&fremd, a.ziel.join("copilot.schluessel")).expect("link");
    let aus = gemeinsam::laufen(&a, &["copilot".to_owned()]);
    assert_eq!(aus.exit, 15);
    let neu = gemeinsam::laufen(&a, &["copilot".to_owned(), "--neu".to_owned()]);
    assert_eq!(neu.exit, 0, "{}", neu.stderr);
    assert_eq!(
        fs::read_to_string(&fremd).expect("read"),
        "fremd\n",
        "the link target is untouched"
    );
    let meta = fs::symlink_metadata(a.ziel.join("copilot.schluessel")).expect("meta");
    assert!(meta.is_file(), "--neu replaces the link by a regular file");
}

#[test]
fn the_real_home_is_not_touched() {
    let a = aufbau();
    skript_schreiben(&a.skript, &attrappe_text(&a, "erfolg", &token()));
    let vorher = dateien(&a.wurzel.join("echt"));
    let aus = gemeinsam::laufen(&a, &["copilot".to_owned()]);
    assert_eq!(aus.exit, 0, "{}", aus.stderr);
    assert_eq!(dateien(&a.wurzel.join("echt")), vorher);
    assert!(a.echt.is_dir() && !a.echt.join("config.json").exists());
}

#[test]
fn a_target_folder_that_is_a_file_is_not_writable() {
    let a = aufbau();
    skript_schreiben(&a.skript, &attrappe_text(&a, "erfolg", &token()));
    fs::remove_dir(&a.ziel).expect("remove");
    fs::write(&a.ziel, "x").expect("file");
    let aus = gemeinsam::laufen(&a, &["copilot".to_owned()]);
    assert_eq!(aus.exit, 14);
    assert!(!a.log.exists(), "the CLI did not start");
    let _ = fs::remove_file(&a.ziel);
}

#[test]
fn a_cli_that_cannot_start_is_a_cancelled_login_and_leaves_nothing() {
    let a = aufbau();
    skript_schreiben(&a.skript, "#!/nonexistent/interpreter\n");
    let aus = gemeinsam::laufen(&a, &["copilot".to_owned()]);
    assert_eq!(aus.exit, 11);
    assert!(fs::read_dir(&a.wegwerf).expect("dir").next().is_none());
}

#[test]
fn sigterm_ends_the_cli_and_cleans_up() {
    let a = aufbau();
    skript_schreiben(&a.skript, &attrappe_text(&a, "haengt", &token()));
    let mut kind = befehl(&a, &["copilot".to_owned()]).spawn().expect("spawn");
    assert!(gemeinsam::warte_auf(&a.pidfile, Duration::from_secs(10)));
    let pid: u32 = fs::read_to_string(&a.pidfile)
        .expect("pid")
        .trim()
        .parse()
        .expect("n");
    gemeinsam::signal_senden("TERM", kind.id());
    gemeinsam::warte_auf_ende(&mut kind, Duration::from_secs(15));
    let aus = ausgang(kind);
    assert_eq!(aus.exit, 143);
    assert!(!gemeinsam::prozess_lebt(pid));
    assert!(fs::read_dir(&a.wegwerf).expect("dir").next().is_none());
    assert_eq!(dateien(&a.ziel).len(), 0);
}

#[test]
fn a_cli_that_ignores_sigterm_is_killed_after_the_grace_time() {
    let a = aufbau();
    skript_schreiben(
        &a.skript,
        &format!(
            "#!/bin/sh\ntrap '' TERM\necho $$ > '{}'\nwhile true; do /bin/sleep 1; done\n",
            a.pidfile.display()
        ),
    );
    let mut kind = befehl(&a, &["copilot".to_owned()]).spawn().expect("spawn");
    assert!(gemeinsam::warte_auf(&a.pidfile, Duration::from_secs(10)));
    std::thread::sleep(Duration::from_millis(300));
    let pid: u32 = fs::read_to_string(&a.pidfile)
        .expect("pid")
        .trim()
        .parse()
        .expect("n");
    gemeinsam::signal_senden("INT", kind.id());
    gemeinsam::warte_auf_ende(&mut kind, Duration::from_secs(15));
    let aus = ausgang(kind);
    assert_eq!(aus.exit, 130);
    for _ in 0..100 {
        if !gemeinsam::prozess_lebt(pid) {
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(!gemeinsam::prozess_lebt(pid), "the stubborn CLI is killed");
    assert!(fs::read_dir(&a.wegwerf).expect("dir").next().is_none());
}

#[test]
fn a_file_that_appears_during_the_login_is_never_replaced_without_neu() {
    let a = aufbau();
    skript_schreiben(&a.skript, &attrappe_text(&a, "wettlauf", &token()));
    let aus = gemeinsam::laufen(&a, &["copilot".to_owned()]);
    assert_eq!(aus.exit, 15, "{}", aus.stderr);
    assert_eq!(
        fs::read_to_string(a.ziel.join("copilot.schluessel")).expect("read"),
        "fremd\n"
    );
    assert_eq!(dateien(&a.ziel).len(), 1, "no temp file is left");
}

#[test]
fn sigterm_gives_the_cli_its_grace_time_before_the_kill() {
    let a = aufbau();
    let marke = a.wurzel.join("beendet.txt");
    skript_schreiben(
        &a.skript,
        &format!(
            "#!/bin/sh\ntrap \"echo ende > '{}'; exit 0\" TERM\necho $$ > '{}'\nwhile true; do /bin/sleep 0.05; done\n",
            marke.display(),
            a.pidfile.display()
        ),
    );
    let mut kind = befehl(&a, &["copilot".to_owned()]).spawn().expect("spawn");
    assert!(gemeinsam::warte_auf(&a.pidfile, Duration::from_secs(10)));
    std::thread::sleep(Duration::from_millis(300));
    let start = std::time::Instant::now();
    gemeinsam::signal_senden("TERM", kind.id());
    gemeinsam::warte_auf_ende(&mut kind, Duration::from_secs(15));
    assert!(
        start.elapsed() < Duration::from_millis(1900),
        "no waiting for the full grace time"
    );
    assert_eq!(ausgang(kind).exit, 143);
    assert!(
        marke.exists(),
        "the CLI ended itself on SIGTERM and was not killed at once"
    );
}

#[test]
fn a_run_killed_hard_leaves_a_home_that_the_next_start_deletes_before_the_login() {
    let a = aufbau();
    skript_schreiben(&a.skript, &attrappe_text(&a, "haengt", &token()));
    let mut kind = befehl(&a, &["copilot".to_owned()]).spawn().expect("spawn");
    assert!(gemeinsam::warte_auf(&a.pidfile, Duration::from_secs(10)));
    let cli: u32 = fs::read_to_string(&a.pidfile)
        .expect("pid")
        .trim()
        .parse()
        .expect("n");
    gemeinsam::signal_senden("KILL", kind.id());
    gemeinsam::warte_auf_ende(&mut kind, Duration::from_secs(15));
    let _ = kind.wait();
    // The stand-in outlives its parent; end it by its exact pid.
    gemeinsam::signal_senden("KILL", cli);
    let reste: Vec<_> = fs::read_dir(&a.wegwerf).expect("dir").flatten().collect();
    assert_eq!(reste.len(), 1, "a hard kill leaves the home");
    skript_schreiben(&a.skript, &attrappe_text(&a, "erfolg", &token()));
    let aus = gemeinsam::laufen(&a, &["copilot".to_owned()]);
    assert_eq!(aus.exit, 0, "{}", aus.stderr);
    assert!(
        aus.stderr
            .starts_with("Hinweis: Reste frueherer Anmeldungen wurden geloescht: 1\n"),
        "{}",
        aus.stderr
    );
    assert!(fs::read_dir(&a.wegwerf).expect("dir").next().is_none());
}

#[test]
fn the_notice_has_no_path_and_no_name() {
    let a = aufbau();
    skript_schreiben(&a.skript, &attrappe_text(&a, "erfolg", &token()));
    let ordner = gemeinsam::rest_anlegen(&a.wegwerf, gemeinsam::tote_pid());
    let aus = gemeinsam::laufen(&a, &["copilot".to_owned()]);
    assert!(!ordner.exists());
    let zeile = aus
        .stderr
        .lines()
        .find(|z| z.starts_with("Hinweis: Reste"))
        .expect("notice");
    assert_eq!(
        zeile,
        "Hinweis: Reste frueherer Anmeldungen wurden geloescht: 1"
    );
    assert!(!aus.stderr.contains("wb-anmelden"));
    assert!(!aus.stderr.contains(&a.wegwerf.display().to_string()));
}

/// The command under an inherited hard limit 0 for core (`ulimit -c 0`: soft and hard). The crate `kernabbild` decides by the core pattern:
/// a pipe at its start (or a pattern that cannot be read) ends the command with exit 5 before the CLI starts; a file pattern or no pattern
/// (macOS) goes on with a note, and the CLI inherits 0/0.
#[test]
fn an_inherited_hard_limit_0_follows_the_core_pattern() {
    let a = aufbau();
    let t = token();
    skript_schreiben(&a.skript, &attrappe_text(&a, "erfolg", &t));
    let b = befehl(&a, &["copilot".to_owned()]);
    let mut c = Command::new("/bin/sh");
    c.arg("-c")
        .arg("ulimit -c 0 && exec \"$0\" \"$@\"")
        .arg(BIN)
        .arg("copilot")
        .env_clear();
    for (k, v) in b.get_envs() {
        if let Some(v) = v {
            c.env(k, v);
        }
    }
    let aus: Ausgang = ausgang(
        c.stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn"),
    );
    let muster = if cfg!(target_os = "linux") {
        Some(fs::read("/proc/sys/kernel/core_pattern"))
    } else {
        None
    };
    match muster {
        Some(Ok(roh)) if roh.first() == Some(&b'|') => {
            assert_eq!(aus.exit, 5, "{}", aus.stderr);
            assert_eq!(
                aus.stderr,
                format!(
                    "werkbank-anmelden: anmelden.abschirmung: Der Schutz vor einem Kernabbild ist nicht setzbar.\n\
                     werkbank-anmelden: kernabbild.muster_pipe: {}\n",
                    kernabbild::Abbruch::MusterPipe.abhilfe()
                )
            );
            assert!(!a.log.exists(), "the CLI did not start");
            assert_eq!(dateien(&a.ziel).len(), 0);
        }
        Some(Err(_)) => {
            assert_eq!(aus.exit, 5, "{}", aus.stderr);
            assert!(aus.stderr.contains("kernabbild.muster_nicht_lesbar"));
            assert!(!a.log.exists(), "the CLI did not start");
        }
        weiter => {
            let hinweis = if weiter.is_some() {
                kernabbild::Hinweis::MusterDatei
            } else {
                kernabbild::Hinweis::MusterFehlt
            };
            assert_eq!(aus.exit, 0, "{}", aus.stderr);
            assert!(
                aus.stderr
                    .starts_with(&format!("Hinweis: {}\n", hinweis.text())),
                "{}",
                aus.stderr
            );
            assert_eq!(
                fs::read_to_string(a.ziel.join("copilot.schluessel")).expect("read"),
                format!("{t}\n")
            );
            if cfg!(target_os = "linux") {
                let log = fs::read_to_string(&a.log).expect("log");
                let zeile = log
                    .lines()
                    .find_map(|z| z.strip_prefix("kernlimit:"))
                    .expect("the stand-in read its own limits");
                let felder: Vec<&str> = zeile.split_whitespace().collect();
                assert_eq!(&felder[4..6], ["0", "0"], "the CLI inherits 0/0: {zeile}");
            }
        }
    }
}

/// The check right before the child (contract 9, section 3c): a soft limit 0 below the hard 1 would let a pipe take an image of the CLI. The
/// probe `grenze-weich-0` (test seam) sets it after the shielding; the command ends with exit 5, the CLI does not start, nothing is left.
#[test]
fn a_child_without_the_limit_1_1_does_not_start() {
    let a = aufbau();
    skript_schreiben(&a.skript, &attrappe_text(&a, "erfolg", &token()));
    let mut c = befehl(&a, &["copilot".to_owned()]);
    c.env("WB_ANMELDEN_PROBE", "grenze-weich-0");
    let aus = ausgang(
        c.stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn"),
    );
    assert_eq!(aus.exit, 5, "{}", aus.stderr);
    assert_eq!(
        aus.stderr,
        "werkbank-anmelden: anmelden.abschirmung: Der Schutz vor einem Kernabbild ist nicht setzbar.\n"
    );
    assert!(!a.log.exists(), "the CLI did not start");
    assert!(fs::read_dir(&a.wegwerf).expect("dir").next().is_none());
    assert_eq!(dateien(&a.ziel).len(), 0);
    // Counter-check: without the probe the same set-up logs in.
    let gut = gemeinsam::laufen(&a, &["copilot".to_owned()]);
    assert_eq!(gut.exit, 0, "{}", gut.stderr);
    assert!(a.log.exists(), "the CLI started");
}
