//! G2 to G4 and E3/E5 inside a real enclosure (`/usr/bin/bwrap`, test build): the harness is the probe of the
//! stand-in. It reports its environment, its descriptors, the entries of `/proc/*/environ` that hold the mark, whether
//! the key of the CA can be opened and whether the search process can be signalled.
//!
//! The forwarder of the enclosure is the real `einschluss-weiter`; build it first in the same target folder
//! (`cargo build -p einschluss --bins`). The cases need a host where `bwrap` may make a user namespace.

#![cfg(all(feature = "pruefbau", target_os = "linux"))]
#![allow(
    clippy::print_stderr,
    clippy::too_many_lines,
    clippy::zombie_processes,
    clippy::case_sensitive_file_extension_comparisons
)]

mod gemeinsam;

use std::path::PathBuf;
use std::process::Command;

use gemeinsam::{Gesperrt, Welt, gleicher_prozess, kopie, nachkommen, prozesse_unter};

const MARKE: &str = "MARKE-G4-8k2q";

fn bwrap_geht() -> bool {
    Command::new("/usr/bin/bwrap")
        .args([
            "--unshare-user",
            "--unshare-net",
            "--ro-bind",
            "/",
            "/",
            "--",
            "/usr/bin/true",
        ])
        .env_clear()
        .gesperrt_status()
        .is_ok_and(|s| s.success())
}

fn weiter() -> PathBuf {
    // next to the program, or (for a tree copied by cargo mutants, which builds only this package) where the test runner
    // names it
    let neben = PathBuf::from(gemeinsam::LAUF)
        .parent()
        .expect("target")
        .join("einschluss-weiter");
    let p = if neben.is_file() {
        neben
    } else {
        std::env::var_os("LAUF_TEST_WEITER").map_or(neben, PathBuf::from)
    };
    assert!(
        p.is_file(),
        "build einschluss-weiter first: {}",
        p.display()
    );
    p
}

/// A world with the real `bwrap` and the real forwarder.
fn echte_welt() -> Welt {
    let mut w = Welt::neu();
    w.bwrap = None;
    kopie(
        &weiter().to_string_lossy(),
        &w.bin.join("einschluss-weiter"),
    );
    w
}

fn zeilen<'a>(aus: &'a str, anfang: &str) -> Vec<&'a str> {
    aus.lines().filter(|z| z.starts_with(anfang)).collect()
}

fn fremde_umgebung() -> Vec<(String, String)> {
    [
        "HTTP_PROXY",
        "HTTPS_PROXY",
        "NO_PROXY",
        "DBUS_SESSION_BUS_ADDRESS",
        "XDG_RUNTIME_DIR",
        "GH_TOKEN",
        "GITHUB_TOKEN",
        "CLAUDE_CODE_OAUTH_TOKEN",
        "CLAUDECODE",
        "COPILOT_GITHUB_TOKEN",
        "NODE_EXTRA_CA_CERTS",
        "LD_PRELOAD",
        "HOME",
        "PATH",
    ]
    .iter()
    .map(|n| ((*n).to_owned(), format!("/{MARKE}/{n}")))
    .collect()
}

/// The kernel keeps only the first 15 characters of a program name in `comm`.
fn comm_von(name: &str) -> String {
    name.chars().take(15).collect()
}

/// Checks the lines of the probe about the start environment of the processes it sees. The probe writes one line for
/// each PID, `environ <pid> lesbar` or `environ <pid> unlesbar <errno>`, and `prozess <pid> <comm>`; a process whose
/// environment cannot be read says nothing about the mark, so the count of hits alone must never be the proof.
///
/// Required: the positive control (the probe reads itself), bwrap as PID 1, and for each name of `erwartet` at least one
/// process of that name, every one of them readable. Any other process that cannot be read is named too, unless it has
/// ended between the listing and the read.
fn environ_pruefen(aus: &str, erwartet: &[&str]) -> Result<(), String> {
    let mut selbst: Option<u32> = None;
    let mut lesbar: Vec<u32> = Vec::new();
    let mut unlesbar: Vec<(u32, String)> = Vec::new();
    let mut namen: Vec<(u32, String)> = Vec::new();
    for z in aus.lines() {
        let teile: Vec<&str> = z.split(' ').collect();
        match teile.as_slice() {
            ["selbst", pid] => selbst = pid.parse().ok(),
            ["environ", pid, "lesbar"] => lesbar.extend(pid.parse::<u32>().ok()),
            ["environ", pid, "unlesbar", errno] => {
                unlesbar.extend(pid.parse::<u32>().ok().map(|p| (p, (*errno).to_owned())));
            }
            ["prozess", pid, comm] => {
                namen.extend(pid.parse::<u32>().ok().map(|p| (p, (*comm).to_owned())));
            }
            _ => {}
        }
    }
    let selbst = selbst.ok_or("the probe names no own PID")?;
    if !lesbar.contains(&selbst) {
        return Err(format!(
            "positive control failed: the probe cannot read its own environment (pid {selbst}); the count of hits proves nothing"
        ));
    }
    let grund = |pid: u32| -> String {
        unlesbar.iter().find(|(p, _)| *p == pid).map_or_else(
            || format!("pid {pid}: no line `environ` at all"),
            |(_, e)| format!("pid {pid}: unreadable, errno {e}"),
        )
    };
    let eins = namen.iter().find(|(p, _)| *p == 1);
    if eins.is_none_or(|(_, c)| c != "bwrap") {
        return Err(format!("pid 1 is not bwrap: {eins:?}"));
    }
    if !lesbar.contains(&1) {
        return Err(format!("bwrap as PID 1 is not readable: {}", grund(1)));
    }
    for name in erwartet {
        let soll = comm_von(name);
        let gefunden: Vec<u32> = namen
            .iter()
            .filter(|(_, c)| *c == soll)
            .map(|(p, _)| *p)
            .collect();
        if gefunden.is_empty() {
            return Err(format!("no process `{name}` seen by the probe"));
        }
        for pid in gefunden {
            if !lesbar.contains(&pid) {
                return Err(format!("`{name}` is not readable: {}", grund(pid)));
            }
        }
    }
    if let Some((pid, errno)) = unlesbar.iter().find(|(_, e)| e != "ENOENT" && e != "ESRCH") {
        return Err(format!(
            "a process hides its environment: pid {pid}, errno {errno}"
        ));
    }
    Ok(())
}

fn probe_zeilen(selbst: u32, rest: &str) -> String {
    format!("selbst {selbst}\n{rest}environ_treffer 0\n")
}

#[test]
fn g4_the_check_of_the_probe_names_every_blind_spot() {
    let gut = probe_zeilen(
        7,
        "environ 1 lesbar\nprozess 1 bwrap\nenviron 5 lesbar\nprozess 5 einschluss-weit\nenviron 7 lesbar\nprozess 7 copilot\n",
    );
    assert_eq!(
        environ_pruefen(&gut, &["bwrap", "einschluss-weiter"]),
        Ok(())
    );
    assert_eq!(environ_pruefen(&gut, &["bwrap"]), Ok(()));
    // the forwarder is not seen
    assert!(
        environ_pruefen(&gut, &["bwrap", "modellweg"])
            .is_err_and(|e| e.contains("no process `modellweg`")),
    );
    // bwrap hides its environment: the count of 0 hits is blind
    let blind = probe_zeilen(
        7,
        "environ 1 unlesbar EACCES\nprozess 1 bwrap\nenviron 7 lesbar\nprozess 7 copilot\n",
    );
    let fehler = environ_pruefen(&blind, &["bwrap"]).expect_err("blind");
    assert!(
        fehler.contains("pid 1") && fehler.contains("EACCES"),
        "{fehler}"
    );
    // the forwarder hides its environment
    let weiter = probe_zeilen(
        7,
        "environ 1 lesbar\nprozess 1 bwrap\nenviron 5 unlesbar EPERM\nprozess 5 einschluss-weit\nenviron 7 lesbar\nprozess 7 copilot\n",
    );
    let fehler = environ_pruefen(&weiter, &["bwrap", "einschluss-weiter"]).expect_err("weiter");
    assert!(
        fehler.contains("einschluss-weiter")
            && fehler.contains("pid 5")
            && fehler.contains("EPERM"),
        "{fehler}"
    );
    // no line at all for the forwarder
    let ohne = probe_zeilen(
        7,
        "environ 1 lesbar\nprozess 1 bwrap\nprozess 5 einschluss-weit\nenviron 7 lesbar\nprozess 7 copilot\n",
    );
    let fehler = environ_pruefen(&ohne, &["einschluss-weiter"]).expect_err("ohne");
    assert!(fehler.contains("no line"), "{fehler}");
    // positive control: the probe does not read itself
    let selbst_blind = probe_zeilen(
        7,
        "environ 1 lesbar\nprozess 1 bwrap\nenviron 7 unlesbar EACCES\nprozess 7 copilot\n",
    );
    assert!(
        environ_pruefen(&selbst_blind, &["bwrap"]).is_err_and(|e| e.contains("positive control")),
    );
    // pid 1 is not bwrap
    let falsch = probe_zeilen(
        7,
        "environ 1 lesbar\nprozess 1 sleep\nenviron 7 lesbar\nprozess 7 copilot\n",
    );
    assert!(environ_pruefen(&falsch, &["bwrap"]).is_err_and(|e| e.contains("pid 1 is not bwrap")));
    // some other process hides its environment
    let fremd = probe_zeilen(
        7,
        "environ 1 lesbar\nprozess 1 bwrap\nenviron 9 unlesbar EACCES\nprozess 9 anderes\nenviron 7 lesbar\nprozess 7 copilot\n",
    );
    assert!(environ_pruefen(&fremd, &["bwrap"]).is_err_and(|e| e.contains("pid 9")));
    // a process that ended between the listing and the read is no blind spot
    let weg = probe_zeilen(
        7,
        "environ 1 lesbar\nprozess 1 bwrap\nenviron 9 unlesbar ENOENT\nprozess 9 ?\nenviron 7 lesbar\nprozess 7 copilot\n",
    );
    assert_eq!(environ_pruefen(&weg, &["bwrap"]), Ok(()));
}

#[test]
fn g4_nothing_of_the_caller_reaches_the_enclosure() {
    assert!(
        bwrap_geht(),
        "bwrap cannot make a user namespace on this host"
    );
    for (route, harness) in [("lokal", "pi"), ("cloud", "copilot")] {
        let w = echte_welt();
        let args = if route == "lokal" {
            w.lokal()
        } else {
            w.cloud()
        };
        w.verhalten(harness, &format!("probe {MARKE}"));
        let lauf = w.lauf(&args, &fremde_umgebung());
        let aus = lauf.stdout();
        assert_eq!(lauf.exit(), Some(0), "{route}: {}{aus}", lauf.stderr());
        let env = zeilen(&aus, "env ");
        assert!(!env.is_empty(), "{route}: the probe ran: {aus}");
        assert!(
            !aus.contains(MARKE),
            "{route}: the mark reached the enclosure: {aus}"
        );
        assert_eq!(
            zeilen(&aus, "environ_treffer "),
            ["environ_treffer 0"],
            "{route}"
        );
        // "0 hits" counts only where the probe could read: one readable line for each expected process
        let erwartet: &[&str] = if route == "lokal" {
            &["bwrap"]
        } else {
            // inside the enclosure the forwarder is the program `weiter` of the guest (`einschluss-weiter` outside)
            &["bwrap", "weiter"]
        };
        if let Err(grund) = environ_pruefen(&aus, erwartet) {
            panic!("{route}: {grund}\n{aus}");
        }
        let mut namen: Vec<&str> = env
            .iter()
            .filter_map(|z| z.strip_prefix("env ")?.split_once('=').map(|(n, _)| n))
            .collect();
        namen.sort_unstable();
        // PWD is set by bwrap itself (the working folder), the others by the profile
        let mut soll = vec!["HOME", "LANG", "PATH", "PWD", "TERM", "TMPDIR"];
        let arbeit = w.wurzel.join("home/beispiel/projekt");
        assert!(
            env.contains(&format!("env PWD={}", arbeit.display()).as_str()),
            "{route}: {env:?}"
        );
        if harness == "pi" {
            soll.extend(["PI_OFFLINE", "PI_TELEMETRY"]);
        } else {
            soll.extend([
                "COPILOT_AUTO_UPDATE",
                "COPILOT_GITHUB_TOKEN",
                "COPILOT_MASCOT_BLINK",
                "HTTPS_PROXY",
            ]);
        }
        soll.sort_unstable();
        assert_eq!(namen, soll, "{route}: the fixed list of contract 1");
        eprintln!("G4 {route}: env {namen:?}, environ_treffer 0, mark absent");
        // the children are ended by the test (the service would do it at the end of the session)
        for p in w.kinder_pids() {
            if let Some(pid) = rustix::process::Pid::from_raw(p) {
                let _ = rustix::process::kill_process(pid, rustix::process::Signal::TERM);
            }
        }
    }
}

#[test]
fn g4_counter_probe_without_the_enclosure_the_mark_is_found() {
    let w = Welt::neu();
    w.verhalten("pi", &format!("probe {MARKE}"));
    let aus = Command::new(w.bin.join("pi"))
        .env_clear()
        .envs(fremde_umgebung())
        .gesperrt_output()
        .expect("probe");
    let aus = String::from_utf8_lossy(&aus.stdout).into_owned();
    assert!(aus.contains(MARKE));
    assert_ne!(
        zeilen(&aus, "environ_treffer "),
        ["environ_treffer 0"],
        "{aus}"
    );
}

#[test]
fn g4_a_tls_variable_of_the_caller_ends_the_start_before_any_child() {
    let w = Welt::neu();
    let args = w.lokal();
    for name in ["SSL_CERT_FILE", "SSL_CERT_DIR", "OPENSSL_CONF"] {
        let lauf = w.lauf(&args, &[(name.to_owned(), format!("/{MARKE}"))]);
        assert_eq!(lauf.exit(), Some(78), "{name}");
        assert_eq!(
            lauf.stderr(),
            "lauf.tls_umgebung: TLS-Umgebung gesetzt\n",
            "{name}"
        );
        assert_eq!(w.pid_von("rangkern"), None, "{name}: no child");
    }
}

#[test]
fn g2_e3_the_ca_key_and_the_search_process_are_out_of_reach() {
    assert!(
        bwrap_geht(),
        "bwrap cannot make a user namespace on this host"
    );
    let w = echte_welt();
    let args = w.cloud();
    let ca = w.start.join("ca.key");
    let pid_datei = w.bin.join("rangkern.pid");
    w.verhalten(
        "copilot",
        &format!(
            "probe {MARKE} {} {} /etc/ssl/certs/ca-certificates.crt {} kill:{}",
            ca.display(),
            w.start.join("start.json").display(),
            w.zustand.join("ordnermarken.json").display(),
            pid_datei.display()
        ),
    );
    let lauf = w.lauf(&args, &[]);
    let aus = lauf.stdout();
    assert_eq!(lauf.exit(), Some(0), "{}{aus}", lauf.stderr());
    // outside the files exist
    assert!(ca.is_file() && w.zustand.join("ordnermarken.json").is_file());
    assert!(
        aus.contains(&format!("pfad {} ENOENT", ca.display())),
        "{aus}"
    );
    assert!(
        aus.contains(&format!(
            "pfad {} ENOENT",
            w.start.join("start.json").display()
        )),
        "{aus}"
    );
    assert!(
        aus.contains(&format!(
            "pfad {} ENOENT",
            w.zustand.join("ordnermarken.json").display()
        )),
        "{aus}"
    );
    assert!(
        aus.contains("pfad /etc/ssl/certs/ca-certificates.crt OK"),
        "{aus}"
    );
    assert!(
        aus.contains(&format!("kill {} ESRCH", pid_datei.display())),
        "E3: {aus}"
    );
    // the bundle of the enclosure is the host bundle with the CA appended; the host bundle is unchanged
    let bundle = std::fs::read(w.start.join("ca-bundle.pem")).expect("bundle");
    let zert = std::fs::read(w.start.join("ca.crt")).expect("cert");
    assert!(bundle.ends_with(&zert));
    let wirt = std::fs::read("/etc/ssl/certs/ca-certificates.crt").expect("host bundle");
    assert!(
        !wirt
            .windows(zert.len().min(200))
            .any(|f| f == &zert[..zert.len().min(200)])
    );
    eprintln!("G2/E3: ca.key ENOENT inside, search process ESRCH inside");
    for p in w.kinder_pids() {
        if let Some(pid) = rustix::process::Pid::from_raw(p) {
            let _ = rustix::process::kill_process(pid, rustix::process::Signal::TERM);
        }
    }
}

/// Contract 1, "Lösen und Senken nur durch einen Menschen": inside the enclosure the command changes nothing. The list of
/// marks lies in the state folder of the installation, which the profile does not bind (the probe of the test above finds
/// it `ENOENT`), and the delivered build reads its configuration from `/etc/werkbank/lauf.json`, which is none of the five
/// files of `/etc` the profile binds. Here the program folder of the test world is bound, so the command finds its
/// configuration next to the program and stops at the settings file, which lies outside as well: a fixed fault, exit 78.
/// This is the first line (the list of marks and the configuration are out of reach). The second line, the program
/// hidden by a bind of `/dev/null`, needs the file in `/usr/bin` of the host; the bind and its check are the tests of
/// the crate `einschluss` (`profil/tests.rs`).
#[test]
fn the_command_changes_nothing_inside_the_enclosure() {
    assert!(
        bwrap_geht(),
        "bwrap cannot make a user namespace on this host"
    );
    let w = echte_welt();
    let args = w.cloud();
    let marken = w.zustand.join("ordnermarken.json");
    let alt = format!(
        "{{\"fassung\":1,\"marken\":[{{\"ordner\":\"{}\",\"kennung\":\"a1b2c3d4e5f60718293a4b5c6d7e8f90\",\"kunde\":null,\"klasse_hoch\":\"intern\"}}]}}\n",
        w.wurzel.join("fremder-ordner").display()
    );
    gemeinsam::datei(&marken, alt.as_bytes(), 0o600);
    let programm = w.bin.join("werkbank-ordnermarke");
    let arbeit = w.wurzel.join("fremder-ordner");
    let aufruf = |befehl: &str| {
        format!(
            "befehl:{},{befehl},{}",
            programm.display(),
            arbeit.display()
        )
    };
    w.verhalten(
        "copilot",
        &format!(
            "probe {MARKE} {} {} {}",
            marken.display(),
            aufruf("loesen"),
            aufruf("zeigen"),
        ),
    );
    let lauf = w.lauf(&args, &[]);
    let aus = lauf.stdout();
    assert_eq!(lauf.exit(), Some(0), "{}{aus}", lauf.stderr());
    assert!(
        aus.contains(&format!("pfad {} ENOENT", marken.display())),
        "the list of marks is out of reach: {aus}"
    );
    assert_eq!(
        zeilen(&aus, "befehl exit "),
        ["befehl exit Some(78)", "befehl exit Some(78)"],
        "{aus}"
    );
    for z in zeilen(&aus, "befehl stderr ") {
        assert!(
            z.starts_with("befehl stderr ordnermarke.konfig_"),
            "a fixed fault of the configuration: {z}"
        );
    }
    assert_eq!(
        zeilen(&aus, "befehl stdout "),
        ["befehl stdout 0", "befehl stdout 0"]
    );
    // the start itself marked its own working folder after the entry was written; the entry stays as it was
    let danach = std::fs::read_to_string(&marken).expect("marks");
    assert!(
        danach.starts_with(alt.trim_end_matches("]}\n")),
        "the entry is unchanged: {danach}"
    );
    assert_eq!(danach.matches("\"ordner\"").count(), 2, "{danach}");
    assert!(
        w.protokoll.read_dir().is_ok_and(|mut l| l.next().is_none()),
        "no log line"
    );
    for p in w.kinder_pids() {
        if let Some(pid) = rustix::process::Pid::from_raw(p) {
            let _ = rustix::process::kill_process(pid, rustix::process::Signal::TERM);
        }
    }
}

#[test]
fn the_harness_holds_no_descriptor_of_the_program() {
    assert!(
        bwrap_geht(),
        "bwrap cannot make a user namespace on this host"
    );
    let w = echte_welt();
    let args = w.lokal();
    w.verhalten("pi", &format!("probe {MARKE}"));
    let lauf = w.lauf(&args, &[]);
    let aus = lauf.stdout();
    assert_eq!(lauf.exit(), Some(0), "{}{aus}", lauf.stderr());
    let fds: Vec<&str> = zeilen(&aus, "fd ");
    // 0, 1, 2 and the descriptor of the listing itself
    let nummern: Vec<&str> = fds
        .iter()
        .filter_map(|z| z.split_whitespace().nth(1))
        .collect();
    assert!(nummern.len() <= 4, "{fds:?}");
    for n in &nummern[..3.min(nummern.len())] {
        assert!(["0", "1", "2"].contains(n), "{fds:?}");
    }
    eprintln!("descriptors in the harness: {fds:?}");
    for p in w.kinder_pids() {
        if let Some(pid) = rustix::process::Pid::from_raw(p) {
            let _ = rustix::process::kill_process(pid, rustix::process::Signal::TERM);
        }
    }
}

/// G1 for the forwarder of the cloud route: the starter ends (by SIGKILL, and as the counter probe by SIGTERM) while the
/// harness runs inside the real enclosure; ten seconds later no process of the world is left, the forwarder
/// `einschluss-weiter` and the children outside included. The harness is a stand-in that waits for ever; the forwarder
/// holds the way of the key (the proxy address of the cloud route).
#[test]
fn g1c_the_forwarder_and_the_children_end_with_the_starter() {
    assert!(
        bwrap_geht(),
        "bwrap cannot make a user namespace on this host"
    );
    for (signal, name) in [
        (rustix::process::Signal::KILL, "SIGKILL"),
        (rustix::process::Signal::TERM, "SIGTERM"),
    ] {
        let w = echte_welt();
        let args = w.cloud();
        w.verhalten("copilot", "stumm");
        let mut kind = gemeinsam::ohne_fremde_deskriptoren(&w.bin.join("werkbank-lauf"))
            .args(&args)
            .env_clear()
            .env("TERM", "xterm")
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::piped())
            .gesperrt_spawn()
            .expect("start");
        // the forwarder and the harness run: wait until the descendants of the starter include both (inside the
        // enclosure their programs carry the paths of the guest, so they are found by their parent, not by a folder)
        let ende = std::time::Instant::now() + std::time::Duration::from_secs(30);
        let vor = loop {
            let liste = nachkommen(i32::try_from(kind.id()).expect("pid"));
            if programm_laeuft(&liste, "weiter") && programm_laeuft(&liste, "copilot") {
                break liste;
            }
            assert!(
                std::time::Instant::now() < ende,
                "{name}: the session did not start: {liste:?}"
            );
            std::thread::sleep(std::time::Duration::from_millis(50));
        };
        let welt_vor = prozesse_unter(&w.wurzel);
        assert!(
            ["rangkern", "modellweg"]
                .iter()
                .all(|n| welt_vor.iter().any(|p| p.befehl.contains(n))),
            "{name}: the children outside run: {welt_vor:?}"
        );
        // the starter is the child of the program (contract 1, "Lebensdauer"; before 21.11.2026 the program itself
        // after its exec)
        let starter = starter_von(kind.id()).expect("the starter runs");
        let pid = rustix::process::Pid::from_raw(starter).expect("pid");
        let _ = rustix::process::kill_process(pid, signal);
        let _ = kind.wait();
        let ende = std::time::Instant::now() + std::time::Duration::from_secs(10);
        let uebrig = || -> Vec<_> {
            vor.iter()
                .filter(|p| gleicher_prozess(p))
                .cloned()
                .collect()
        };
        while std::time::Instant::now() < ende
            && (!uebrig().is_empty() || !prozesse_unter(&w.wurzel).is_empty())
        {
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        let uebrig = uebrig();
        let rest = prozesse_unter(&w.wurzel);
        eprintln!(
            "G1c {name}: {} descendants of the starter and {} programs of the world before, {} and {} after 10 s",
            vor.len(),
            welt_vor.len(),
            uebrig.len(),
            rest.len()
        );
        assert!(
            uebrig.is_empty() && rest.is_empty(),
            "{name}: left after 10 s: descendants {uebrig:?}, programs of the world {rest:?}"
        );
    }
}

/// The start environment of `pid` as an observer reads it from inside the user namespace of that process (`nsenter -U`).
/// An observer outside could be shut out later when the forwarder hides itself from other readers; one inside the same
/// user namespace still sees it, so this is the reader that must find nothing.
fn umgebung_im_namensraum(pid: i32) -> Result<Vec<u8>, String> {
    let aus = Command::new("nsenter")
        .args([
            "-U",
            "--preserve-credentials",
            "-t",
            &pid.to_string(),
            "--",
            "/bin/sh",
            "-c",
        ])
        .arg("cat \"/proc/$0/environ\"")
        .arg(pid.to_string())
        .gesperrt_output()
        .map_err(|e| format!("nsenter: {e}"))?;
    if aus.status.success() {
        Ok(aus.stdout)
    } else {
        Err(format!(
            "observer cannot read pid {pid}: {:?} {}",
            aus.status.code(),
            String::from_utf8_lossy(&aus.stderr)
        ))
    }
}

/// The starter: the child of `lauf` whose program is `bwrap`, whose command after `--` is the forwarder (the cloud
/// route) and that does not end with `/usr/bin/true`. Every other `bwrap` of the start is a child of `lauf` too: the
/// extraction of the package, and the probe run of the profile, which carries the same command with `/usr/bin/true`
/// appended. Measured: a choice by the name alone, or by the forwarder alone, took one of them; the start went on (a
/// hang of the wait) or ended with `lauf.einschluss_fehlt`, and the starter was never hit.
fn starter_von(lauf: u32) -> Option<i32> {
    kind_namens(lauf, "bwrap").filter(|p| {
        std::fs::read(format!("/proc/{p}/cmdline")).is_ok_and(|c| {
            let teile: Vec<&[u8]> = c.split(|b| *b == 0).filter(|t| !t.is_empty()).collect();
            teile
                .windows(2)
                .any(|f| f[0] == b"--" && f[1] == b"/run/brain/weiter")
                && teile.last().is_some_and(|l| *l != b"/usr/bin/true")
        })
    })
}

/// The child of `eltern` whose program is `name` (`/proc/<eltern>/task/*/children`, then `comm`).
fn kind_namens(eltern: u32, name: &str) -> Option<i32> {
    for t in std::fs::read_dir(format!("/proc/{eltern}/task"))
        .ok()?
        .filter_map(Result::ok)
    {
        let Ok(k) = std::fs::read_to_string(t.path().join("children")) else {
            continue;
        };
        if let Some(p) = k
            .split_whitespace()
            .filter_map(|x| x.parse::<i32>().ok())
            .find(|p| name_von(*p) == name)
        {
            return Some(p);
        }
    }
    None
}

/// The name of the program of `pid` as the kernel keeps it (`comm`); empty when the process has ended.
fn name_von(pid: i32) -> String {
    std::fs::read_to_string(format!("/proc/{pid}/comm"))
        .unwrap_or_default()
        .trim_end()
        .to_owned()
}

/// Whether one of `liste` runs the program `name`. The name is the one of the program itself, never a word in the
/// command line: the command line of `bwrap` names the forwarder and the harness from its first moment on, long before
/// either of them runs (measured: the observer took `bwrap` for the forwarder and joined a namespace it was in already;
/// G1c ended the starter while only `bwrap` existed).
fn programm_laeuft(liste: &[gemeinsam::Prozess], name: &str) -> bool {
    liste.iter().any(|p| name_von(p.pid) == name)
}

fn treffer_in(umgebung: &[u8]) -> usize {
    umgebung
        .windows(MARKE.len())
        .filter(|w| *w == MARKE.as_bytes())
        .count()
}

/// The environment of the forwarder `einschluss-weiter`, read by an observer in its user namespace.
/// Counter probe first: a forwarder started in a user namespace of the test (`unshare -Ur`) with the mark in its
/// environment is found with exactly one hit; only then the forwarder of the real enclosure counts, and it must show
/// none. There is no switch in the product for this: a later shielding of the forwarder must not blind the check.
#[test]
fn g4_an_observer_in_the_user_namespace_finds_nothing_in_the_environment_of_the_forwarder() {
    assert!(
        bwrap_geht(),
        "bwrap cannot make a user namespace on this host"
    );
    // counter probe: the planted mark
    let w = echte_welt();
    let mut gefunden = None;
    for _ in 0..5 {
        let port = std::net::TcpListener::bind("127.0.0.1:0")
            .expect("port")
            .local_addr()
            .expect("addr")
            .port();
        let mut kind = Command::new("unshare")
            .args(["-Ur", "--"])
            .arg(w.bin.join("einschluss-weiter"))
            .arg(port.to_string())
            .arg(w.wurzel.join("weiter.sock"))
            .args(["--", "/bin/sleep", "20"])
            .env_clear()
            .env("WB_GEPFLANZT", MARKE)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .gesperrt_spawn()
            .expect("unshare");
        let pid = i32::try_from(kind.id()).expect("pid");
        let ende = std::time::Instant::now() + std::time::Duration::from_secs(5);
        let mut laeuft = false;
        while std::time::Instant::now() < ende {
            let comm = std::fs::read_to_string(format!("/proc/{pid}/comm")).unwrap_or_default();
            if comm.trim_end() == comm_von("einschluss-weiter") {
                // the forwarder holds the port once its program runs
                laeuft = !nachkommen(pid).is_empty();
                if laeuft {
                    break;
                }
            }
            if kind.try_wait().expect("wait").is_some() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        if laeuft {
            let gelesen = umgebung_im_namensraum(pid);
            for p in nachkommen(pid) {
                if gleicher_prozess(&p)
                    && let Some(r) = rustix::process::Pid::from_raw(p.pid)
                {
                    let _ = rustix::process::kill_process(r, rustix::process::Signal::KILL);
                }
            }
            let _ = kind.kill();
            let _ = kind.wait();
            gefunden = Some(gelesen);
            break;
        }
        let _ = kind.kill();
        let _ = kind.wait();
    }
    let gepflanzt = gefunden
        .expect("the forwarder of the counter probe did not start in five tries")
        .expect("counter probe: the observer reads the planted forwarder");
    assert_eq!(
        treffer_in(&gepflanzt),
        1,
        "counter probe: the planted mark is found by the observer"
    );
    // the real enclosure
    let args = w.cloud();
    w.verhalten("copilot", "stumm");
    let mut kind = gemeinsam::ohne_fremde_deskriptoren(&w.bin.join("werkbank-lauf"))
        .args(&args)
        .env_clear()
        .envs(fremde_umgebung())
        .env("TERM", "xterm")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .gesperrt_spawn()
        .expect("start");
    let ende = std::time::Instant::now() + std::time::Duration::from_secs(30);
    let weiter_pid = loop {
        let liste = nachkommen(i32::try_from(kind.id()).expect("pid"));
        if let Some(p) = liste.iter().find(|p| name_von(p.pid) == "weiter")
            && programm_laeuft(&liste, "copilot")
        {
            break p.pid;
        }
        assert!(
            std::time::Instant::now() < ende,
            "the session did not start: {liste:?}"
        );
        std::thread::sleep(std::time::Duration::from_millis(50));
    };
    let gelesen = umgebung_im_namensraum(weiter_pid);
    let pid = rustix::process::Pid::from_raw(i32::try_from(kind.id()).expect("pid")).expect("pid");
    let _ = rustix::process::kill_process(pid, rustix::process::Signal::KILL);
    let _ = kind.wait();
    let umgebung = gelesen.expect("the observer reads the forwarder of the enclosure");
    // the forwarder is started with an empty environment, so an empty read is a valid read: what counts is that the
    // read worked (an error above is red) and that the same reader found the planted mark in the counter probe
    assert_eq!(
        treffer_in(&umgebung),
        0,
        "the mark reached the forwarder of the enclosure"
    );
    eprintln!(
        "G4 forwarder: observer in the user namespace read {} bytes, 0 hits (counter probe: 1 hit)",
        umgebung.len()
    );
}

/// The processes whose command line names `wurzel` (the folder of one test world), found by a full scan: the enclosure
/// and its payload carry the paths of the world, also when their parent has gone and they hang below PID 1. Zombies are
/// left out. Each is named by PID, command line and start time (field 22 of `/proc/<pid>/stat`).
fn lebende_der_welt(wurzel: &std::path::Path) -> Vec<(i32, String, String)> {
    let Ok(liste) = std::fs::read_dir("/proc") else {
        return Vec::new();
    };
    let eigen = std::process::id();
    let mut gefunden = Vec::new();
    for e in liste.filter_map(Result::ok) {
        let Some(pid) = e.file_name().to_str().and_then(|n| n.parse::<i32>().ok()) else {
            continue;
        };
        if u32::try_from(pid).is_ok_and(|p| p == eigen) {
            continue;
        }
        let Ok(befehl) = std::fs::read(format!("/proc/{pid}/cmdline")) else {
            continue;
        };
        let befehl = String::from_utf8_lossy(&befehl).replace('\0', " ");
        let Ok(stat) = std::fs::read_to_string(format!("/proc/{pid}/stat")) else {
            continue;
        };
        let Some((_, rest)) = stat.rsplit_once(')') else {
            continue;
        };
        let felder: Vec<&str> = rest.split_whitespace().collect();
        if felder.first() == Some(&"Z") || !befehl.contains(&*wurzel.to_string_lossy()) {
            continue;
        }
        gefunden.push((pid, befehl, (*felder.get(19).unwrap_or(&"?")).to_owned()));
    }
    gefunden
}

/// The hang guard of G1 while it waits for the start of a world. It is no measure: the claim of G1 begins when the
/// starter has become `bwrap`, and a start may take long on a loaded machine (an `fsync` of the program waits for the
/// disk). The wait ends on events: the thing awaited, or the end of the program, which is red with its status and its
/// standard error.
const START_GRENZE: std::time::Duration = std::time::Duration::from_secs(300);

/// Waits until `gesucht` finds something, the program `kind` ends (red, with status and standard error from
/// `fehler_datei`), or [`START_GRENZE`] has passed (red, with the threads of the program, their state, system call and
/// `wchan`, and the processes of the world ended). `spin` polls without sleeping (for the first milliseconds).
fn start_abwarten<T>(
    kind: &mut std::process::Child,
    w: &Welt,
    fehler_datei: &std::path::Path,
    was: &str,
    spin: bool,
    mut gesucht: impl FnMut(&mut std::process::Child) -> Option<T>,
) -> T {
    let anfang = std::time::Instant::now();
    loop {
        if let Some(t) = gesucht(kind) {
            return t;
        }
        if let Ok(Some(status)) = kind.try_wait() {
            let fehler = std::fs::read_to_string(fehler_datei).unwrap_or_default();
            panic!(
                "{was}: the program ended after {} ms: {status:?}, standard error {fehler:?}",
                anfang.elapsed().as_millis()
            );
        }
        if anfang.elapsed() >= START_GRENZE {
            let faeden = faeden_von(kind.id());
            let namen: Vec<String> = nachkommen(i32::try_from(kind.id()).expect("pid"))
                .iter()
                .map(|p| format!("{}:{}", p.pid, name_von(p.pid)))
                .collect();
            let _ = kind.kill();
            let _ = kind.wait();
            gemeinsam::prozesse_unter_beenden(&w.wurzel);
            panic!("{was}: nothing in {START_GRENZE:?}; threads {faeden:?}, below it {namen:?}");
        }
        if spin {
            std::hint::spin_loop();
        } else {
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
    }
}

/// Each thread of `pid`: its number, state, system call (number and first argument) and `wchan`.
fn faeden_von(pid: u32) -> Vec<String> {
    let Ok(liste) = std::fs::read_dir(format!("/proc/{pid}/task")) else {
        return Vec::new();
    };
    liste
        .filter_map(Result::ok)
        .map(|t| {
            let pfad = t.path();
            let aufruf = std::fs::read_to_string(pfad.join("syscall")).unwrap_or_default();
            let aufruf: Vec<&str> = aufruf.split_whitespace().take(2).collect();
            let wchan = std::fs::read_to_string(pfad.join("wchan")).unwrap_or_default();
            let stat = std::fs::read_to_string(pfad.join("stat")).unwrap_or_default();
            let zustand = stat
                .rsplit_once(')')
                .and_then(|(_, r)| r.split_whitespace().next().map(str::to_owned))
                .unwrap_or_default();
            format!(
                "{}:{zustand}:{}:{wchan}",
                t.file_name().to_string_lossy(),
                aufruf.join(",")
            )
        })
        .collect()
}

/// G1 at the earliest moment. Positive control first: the scan finds the processes of a world that runs. Then the
/// starter (the child of the program, which is `bwrap` after the exec of the trampoline) is ended by SIGKILL after a
/// delay of 0 to 4 ms after it has become `bwrap`, 60 times, each in a world of its own; two seconds later nothing of
/// the world may live.
///
/// Measured red before the program stayed the parent of the starter (21.11.2026, the Linux build host, 2 of 60 rounds): the inner
/// `bwrap` (the first process of the new process namespace) lived on below PID 1 and started the forwarder and the
/// harness, because `bwrap` sets its death signal only after the set-up of the namespaces (bubblewrap 0.12.0, `do_init`).
/// Now the program is the child subreaper of the starter: the orphan falls to it, and it ends it (contract 1,
/// "Lebensdauer").
#[test]
fn g1_the_enclosure_ends_also_when_the_starter_is_killed_in_its_first_milliseconds() {
    assert!(
        bwrap_geht(),
        "bwrap cannot make a user namespace on this host"
    );
    // positive control: a running world is found by the scan
    {
        let w = echte_welt();
        let args = w.cloud();
        w.verhalten("copilot", "stumm");
        let mut kind = gemeinsam::ohne_fremde_deskriptoren(&w.bin.join("werkbank-lauf"))
            .args(&args)
            .env_clear()
            .env("TERM", "xterm")
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::fs::File::create(w.wurzel.join("lauf-stderr")).expect("stderr file"))
            .gesperrt_spawn()
            .expect("start");
        start_abwarten(
            &mut kind,
            &w,
            &w.wurzel.join("lauf-stderr"),
            "the control world",
            false,
            |k| {
                programm_laeuft(&nachkommen(i32::try_from(k.id()).expect("pid")), "copilot")
                    .then_some(())
            },
        );
        assert!(
            !lebende_der_welt(&w.wurzel).is_empty(),
            "positive control: the scan finds nothing of a running world"
        );
        let _ = kind.kill();
        let _ = kind.wait();
    }
    let mut zufall: u64 = 0x9e37_79b9_7f4a_7c15 ^ u64::from(std::process::id());
    let mut uebrig_runden = Vec::new();
    for runde in 0..60 {
        zufall ^= zufall << 13;
        zufall ^= zufall >> 7;
        zufall ^= zufall << 17;
        let verzoegerung = zufall % 4000;
        let w = echte_welt();
        let args = w.cloud();
        w.verhalten("copilot", "stumm");
        let mut kind = gemeinsam::ohne_fremde_deskriptoren(&w.bin.join("werkbank-lauf"))
            .args(&args)
            .env_clear()
            .env("TERM", "xterm")
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::fs::File::create(w.wurzel.join("lauf-stderr")).expect("stderr file"))
            .gesperrt_spawn()
            .expect("start");
        let kpid = start_abwarten(
            &mut kind,
            &w,
            &w.wurzel.join("lauf-stderr"),
            &format!("round {runde}: the starter did not become bwrap"),
            true,
            |k| starter_von(k.id()),
        );
        let t0 = std::time::Instant::now();
        while t0.elapsed() < std::time::Duration::from_micros(verzoegerung) {
            std::hint::spin_loop();
        }
        let pid = rustix::process::Pid::from_raw(kpid).expect("pid");
        let _ = rustix::process::kill_process(pid, rustix::process::Signal::KILL);
        // the program ends with its starter; a program that lives on is red, not a hang
        let ende = std::time::Instant::now() + std::time::Duration::from_secs(30);
        let status = loop {
            if let Ok(Some(st)) = kind.try_wait() {
                break st;
            }
            if std::time::Instant::now() >= ende {
                let _ = kind.kill();
                let _ = kind.wait();
                gemeinsam::prozesse_unter_beenden(&w.wurzel);
                panic!(
                    "round {runde}: the program lives on 30 s after the end of its starter {kpid}"
                );
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        };
        // the signal of the starter is the end of the program: the SIGKILL hit the starter and nothing else
        assert_eq!(
            std::os::unix::process::ExitStatusExt::signal(&status),
            Some(9),
            "round {runde}: {status:?}"
        );
        std::thread::sleep(std::time::Duration::from_secs(2));
        let lebende = lebende_der_welt(&w.wurzel);
        if !lebende.is_empty() {
            eprintln!("round {runde}, delay {verzoegerung} us: left {lebende:?}");
            uebrig_runden.push((runde, verzoegerung, lebende.len()));
            // end them by exact PID, the command line and the start time compared again
            for (p, befehl, start) in lebende {
                let gleich = lebende_der_welt(&w.wurzel)
                    .iter()
                    .any(|(q, b, s)| *q == p && *b == befehl && *s == start);
                if gleich && let Some(r) = rustix::process::Pid::from_raw(p) {
                    let _ = rustix::process::kill_process(r, rustix::process::Signal::KILL);
                }
            }
            gemeinsam::prozesse_unter_beenden(&w.wurzel);
        }
    }
    eprintln!(
        "G1 early: {} of 60 rounds left processes: {uebrig_runden:?}",
        uebrig_runden.len()
    );
    assert!(
        uebrig_runden.is_empty(),
        "the enclosure lives on after the starter is ended in its first milliseconds: {uebrig_runden:?}"
    );
}
