//! G1: the search process and the model path end with the harness (contract 1, "Lebensdauer"), also when it dies by
//! SIGKILL; the counter probes show that `PR_SET_PDEATHSIG` carries it and that it hangs on the forking thread.
//!
//! The `bwrap` here is a script: the test run of the profile ends with 0, the final call replaces itself by `sleep`,
//! which stands for the harness, a child of the program (the program stays the parent of the starter, contract 1,
//! "Lebensdauer").

#![cfg(all(feature = "pruefbau", target_os = "linux"))]
#![allow(
    clippy::print_stderr,
    clippy::too_many_lines,
    clippy::zombie_processes,
    clippy::case_sensitive_file_extension_comparisons
)]

mod gemeinsam;

use std::process::{Command, Stdio};
use std::time::Duration;

use gemeinsam::{
    Gesperrt, HAENGESPERRE_MS, Welt, datei, endet_binnen, lebt, prozesse_unter,
    prozesse_unter_beenden,
};

/// A `bwrap` whose final call becomes the harness: `sleep`, a child of the program. Before its `exec` it writes its PID
/// into `harness.pid` of the world: the ready signal [`starten`] waits for.
fn schlafender_bwrap(w: &Welt) {
    verzoegerter_bwrap(w, 0);
}

/// [`schlafender_bwrap`] whose final call waits `sekunden` before the ready signal.
fn verzoegerter_bwrap(w: &Welt, sekunden: u32) {
    let skript = format!(
        "#!/bin/sh\nfor a in \"$@\"; do last=\"$a\"; done\nif [ \"$last\" = /usr/bin/true ]; then exit 0; fi\nsleep {sekunden}\necho $$ > {pid}.neu\nmv {pid}.neu {pid}\nexec sleep 60\n",
        pid = harness_datei(w).display()
    );
    datei(&w.bin.join("bwrap"), skript.as_bytes(), 0o755);
}

/// The ready signal of the harness: its PID, written by the `bwrap` stand-in right before its `exec`.
fn harness_datei(w: &Welt) -> std::path::PathBuf {
    w.wurzel.join("harness.pid")
}

/// The upper halt of a start against hanging, never the measure of it: the tests wait for a ready signal. Contract 1
/// gives the two children alone up to twice `BEREIT_FRIST` (10 s each).
const START_HALT: Duration = Duration::from_secs(120);

/// Waits until `bereit` gives a value. Ends at once with the exit and the standard error of the program when it ended
/// before (a fault of the start is never taken for slowness), and at [`START_HALT`] against hanging.
fn bereit_abwarten<T>(kind: &mut std::process::Child, mut bereit: impl FnMut() -> Option<T>) -> T {
    use std::io::Read;
    let ende = std::time::Instant::now() + START_HALT;
    loop {
        if let Some(t) = bereit() {
            return t;
        }
        if let Ok(Some(status)) = kind.try_wait() {
            let mut fehler = String::new();
            if let Some(mut e) = kind.stderr.take() {
                let _ = e.read_to_string(&mut fehler);
            }
            panic!(
                "the program ended before the harness ran: {status:?}, standard error {fehler:?}"
            );
        }
        assert!(
            std::time::Instant::now() < ende,
            "no ready signal of the harness within {START_HALT:?} (upper halt against hanging)"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// The PID of the harness: the child of the program `lauf` whose program is `sleep` (the starter stand-in replaced
/// itself by it).
fn harness_von(lauf: u32) -> Option<i32> {
    for t in std::fs::read_dir(format!("/proc/{lauf}/task"))
        .ok()?
        .filter_map(Result::ok)
    {
        let Ok(k) = std::fs::read_to_string(t.path().join("children")) else {
            continue;
        };
        for p in k.split_whitespace().filter_map(|x| x.parse::<i32>().ok()) {
            if std::fs::read_to_string(format!("/proc/{p}/comm")).is_ok_and(|c| c.trim() == "sleep")
            {
                return Some(p);
            }
        }
    }
    None
}

/// Starts the program in the background and waits for the ready signal of the harness ([`schlafender_bwrap`]): both PID
/// files are there, and the PID the harness wrote is the child `sleep` of the program. Returns the program, the PIDs of
/// both children and the PID of the harness.
fn starten(w: &Welt, args: &[String]) -> (std::process::Child, Vec<i32>, i32) {
    let mut kind = gemeinsam::ohne_fremde_deskriptoren(&w.bin.join("werkbank-lauf"))
        .args(args)
        .env_clear()
        .env("TERM", "xterm")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .gesperrt_spawn()
        .expect("start");
    let lauf = kind.id();
    let (pids, h) = bereit_abwarten(&mut kind, || {
        let gemeldet: i32 = std::fs::read_to_string(harness_datei(w))
            .ok()?
            .trim()
            .parse()
            .ok()?;
        let pids = w.kinder_pids();
        (pids.len() == 2 && harness_von(lauf) == Some(gemeldet)).then_some((pids, gemeldet))
    });
    (kind, pids, h)
}

/// Ends the harness by SIGKILL and waits for the program, which ends by the same signal.
fn hart_beenden(kind: &mut std::process::Child, harness: i32) {
    use std::os::unix::process::ExitStatusExt;
    if let Some(pid) = rustix::process::Pid::from_raw(harness) {
        let _ = rustix::process::kill_process(pid, rustix::process::Signal::KILL);
    }
    let status = kind.wait().expect("wait");
    assert_eq!(
        status.signal(),
        Some(9),
        "the program ends by the signal of the harness: {status:?}"
    );
}

#[test]
fn g1_the_children_end_when_the_harness_dies_by_sigkill() {
    for route in ["lokal", "cloud"] {
        let w = Welt::neu();
        schlafender_bwrap(&w);
        let args = if route == "lokal" {
            w.lokal()
        } else {
            w.cloud()
        };
        let (mut kind, pids, harness) = starten(&w, &args);
        for p in &pids {
            assert!(lebt(*p), "{route}: child {p} runs while the harness runs");
            // bound to the harness: its parent is the PID of the program (now the harness)
            let stat = std::fs::read_to_string(format!("/proc/{p}/stat")).expect("stat");
            let ppid: u32 = stat
                .rsplit_once(')')
                .and_then(|(_, r)| r.split_whitespace().nth(1))
                .and_then(|x| x.parse().ok())
                .expect("ppid");
            assert_eq!(ppid, kind.id(), "{route}: getppid of the child");
        }
        hart_beenden(&mut kind, harness);
        for p in &pids {
            assert!(
                endet_binnen(*p, HAENGESPERRE_MS),
                "{route}: child {p} outlived the harness"
            );
        }
        eprintln!(
            "G1 {route}: harness {} SIGKILL, children {pids:?} ended",
            kind.id()
        );
    }
}

#[test]
fn g1_counter_probe_without_the_death_signal_an_orphan_stays() {
    let w = Welt::neu();
    schlafender_bwrap(&w);
    // a trampoline without PR_SET_PDEATHSIG: drops the PID and the "--" and runs the program
    datei(
        &w.bin.join("werkbank-lauf-kind"),
        b"#!/bin/sh\nshift 2\nexec \"$@\"\n",
        0o755,
    );
    let args = w.lokal();
    let (mut kind, pids, harness) = starten(&w, &args);
    hart_beenden(&mut kind, harness);
    std::thread::sleep(Duration::from_millis(1500));
    let waisen: Vec<i32> = pids.iter().copied().filter(|p| lebt(*p)).collect();
    for p in &pids {
        if let Some(pid) = rustix::process::Pid::from_raw(*p) {
            let _ = rustix::process::kill_process(pid, rustix::process::Signal::KILL);
        }
    }
    assert_eq!(
        waisen.len(),
        2,
        "without the signal both children stay as orphans"
    );
    eprintln!("G1 counter probe: orphans {waisen:?} (ended by the test)");
}

/// The thread condition: the signal hangs on the thread that forked. A child forked from a thread that ends gets
/// SIGTERM at once, although the process lives on. "Lauf starten" therefore forks from its main thread and checks
/// that it is single threaded.
#[test]
fn g1_counter_probe_the_signal_hangs_on_the_forking_thread() {
    let w = Welt::neu();
    w.verhalten("rangkern", "stumm");
    let kind_pfad = w.bin.join("werkbank-lauf-kind");
    let programm = w.bin.join("rangkern");
    let (tx, rx) = std::sync::mpsc::channel::<u32>();
    let (halt_tx, halt_rx) = std::sync::mpsc::channel::<()>();
    let faden = std::thread::spawn(move || {
        let k = Command::new(&kind_pfad)
            .arg(std::process::id().to_string())
            .arg("--")
            .arg(&programm)
            .env_clear()
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .gesperrt_spawn()
            .expect("start");
        tx.send(k.id()).expect("send");
        let _ = halt_rx.recv();
        // the thread ends here; the child is not waited for
        std::mem::forget(k);
    });
    let pid = i32::try_from(rx.recv().expect("pid")).expect("pid");
    // the thread may end only after the trampoline set the death signal: before it, the parent check of the trampoline
    // passes after the end of the thread too (the process keeps its PID), and no signal would ever come. The `exec` into
    // the program comes after the signal, so the program in `/proc/<pid>/exe` marks that point (a fixed sleep did not
    // under load)
    assert!(
        exe_ist(pid, &w.bin.join("rangkern"), HAENGESPERRE_MS),
        "the trampoline replaces itself by the program"
    );
    assert!(lebt(pid), "the child runs while its forking thread runs");
    drop(halt_tx);
    faden.join().expect("thread");
    assert!(
        endet_binnen(pid, HAENGESPERRE_MS),
        "the child must end with its forking thread although the process lives"
    );
    // reap it
    let _ = rustix::process::waitpid(
        rustix::process::Pid::from_raw(pid),
        rustix::process::WaitOptions::NOHANG,
    );
    eprintln!("G1 thread condition: child {pid} ended with the end of its forking thread");
}

/// Waits until the program of `pid` is `programm` (`/proc/<pid>/exe`); `ms` is only the guard against a hang.
fn exe_ist(pid: i32, programm: &std::path::Path, ms: u64) -> bool {
    let ende = std::time::Instant::now() + Duration::from_millis(ms);
    while std::time::Instant::now() < ende {
        if std::fs::read_link(format!("/proc/{pid}/exe")).is_ok_and(|p| p == programm) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    false
}

#[test]
fn a_failure_after_step_7_ends_both_children() {
    let w = Welt::neu();
    let args = w.lokal();
    // the harness is missing: step 8 fails after both children started
    let _ = std::fs::remove_file(w.bin.join("pi"));
    let lauf = w.lauf(&args, &[]);
    assert_eq!(lauf.exit(), Some(69));
    assert_eq!(
        lauf.stderr(),
        "lauf.harness_fehlt: Harness nicht startbar\n"
    );
    for p in ["rangkern", "modellweg"] {
        let pid = w.pid_von(p).expect("started");
        assert!(!lebt(pid), "{p} ended");
    }
}

/// Step 6: a descriptor the caller handed down without close-on-exec would reach the enclosure; the start ends with
/// `lauf.einschluss_fehlt` before any child. The counter probe is every other case of this file: without the
/// descriptor the same start reaches the harness.
#[test]
fn an_inherited_descriptor_ends_the_start_at_step_6() {
    let w = Welt::neu();
    let args = w.lokal();
    let aus = Command::new("/bin/sh")
        .args(["-c", "exec 7</dev/null; exec \"$0\" \"$@\""])
        .arg(w.bin.join("werkbank-lauf"))
        .args(&args)
        .env_clear()
        .env("TERM", "xterm")
        .stdin(Stdio::null())
        .gesperrt_output()
        .expect("run");
    assert_eq!(aus.status.code(), Some(69));
    assert_eq!(
        String::from_utf8_lossy(&aus.stderr),
        "lauf.einschluss_fehlt: Einschluss nicht verfuegbar\n"
    );
    assert_eq!(w.pid_von("rangkern"), None, "no child");
}

fn konfig_ersetzen(w: &Welt, alt: &str, neu: &str) {
    let k = std::fs::read_to_string(w.bin.join("lauf.json")).expect("konfig");
    assert!(k.contains(alt), "{alt}");
    datei(
        &w.bin.join("lauf.json"),
        k.replacen(alt, neu, 1).as_bytes(),
        0o644,
    );
}

/// A bare file name as the argument: the session files lie in the current folder.
#[test]
fn a_bare_start_file_name_is_read_in_the_current_folder() {
    let w = Welt::neu();
    w.lokal();
    let aus = gemeinsam::ohne_fremde_deskriptoren(&w.bin.join("werkbank-lauf"))
        .arg("start.json")
        .current_dir(&w.start)
        .env_clear()
        .env("TERM", "xterm")
        .stdin(Stdio::null())
        .gesperrt_output()
        .expect("run");
    assert_eq!(
        aus.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&aus.stderr)
    );
    assert!(w.start.join("suche.pid").is_file());
}

/// Programs without an execute bit: the harness is `lauf.harness_fehlt`, `bwrap` is `lauf.einschluss_fehlt`.
#[test]
fn a_program_without_an_execute_bit_is_not_started() {
    use std::os::unix::fs::PermissionsExt;
    let w = Welt::neu();
    let args = w.lokal();
    std::fs::set_permissions(w.bin.join("pi"), std::fs::Permissions::from_mode(0o644))
        .expect("mode");
    let lauf = w.lauf(&args, &[]);
    assert_eq!(
        (lauf.exit(), lauf.stderr().as_str()),
        (Some(69), "lauf.harness_fehlt: Harness nicht startbar\n")
    );
    let w = Welt::neu();
    let args = w.lokal();
    std::fs::set_permissions(w.bin.join("bwrap"), std::fs::Permissions::from_mode(0o644))
        .expect("mode");
    let lauf = w.lauf(&args, &[]);
    assert_eq!(
        (lauf.exit(), lauf.stderr().as_str()),
        (
            Some(69),
            "lauf.einschluss_fehlt: Einschluss nicht verfuegbar\n"
        )
    );
    assert_eq!(w.pid_von("rangkern"), None);
}

/// The longest socket path: 107 bytes pass, 108 are `lauf.suche_fehlt`.
#[test]
fn the_socket_path_may_have_107_bytes_and_not_108() {
    for (laenge, gut) in [(107usize, true), (108, false)] {
        let w = Welt::neu();
        let args = w.lokal();
        let basis = w.wurzel.to_string_lossy().len() + 1;
        let rest = "/s-1/modell.sock".len();
        let lz = w.wurzel.join("z".repeat(laenge - basis - rest));
        std::fs::create_dir_all(&lz).expect("folder");
        konfig_ersetzen(
            &w,
            &format!("\"laufzeit\":\"{}\"", w.laufzeit.display()),
            &format!("\"laufzeit\":\"{}\"", lz.display()),
        );
        assert_eq!(lz.join("s-1/modell.sock").as_os_str().len(), laenge);
        let lauf = w.lauf(&args, &[]);
        if gut {
            assert_eq!(lauf.exit(), Some(0), "{laenge}: {}", lauf.stderr());
        } else {
            assert_eq!(
                (lauf.exit(), lauf.stderr().as_str()),
                (Some(69), "lauf.suche_fehlt: Suche nicht startbar\n"),
                "{laenge}"
            );
        }
    }
}

/// A cloud route needs the key file of the route only when a target is bound to the key.
#[test]
fn a_cloud_route_without_a_key_file_starts_only_without_bound_targets() {
    let ohne_schluessel = |w: &Welt| {
        konfig_ersetzen(
            w,
            &format!(
                ",\"cloud-beispiel\":\"{}\"",
                w.schluessel.join("cloud-beispiel").display()
            ),
            "",
        );
    };
    // bound target, no key file: the model path cannot start
    let w = Welt::neu();
    let args = w.cloud();
    ohne_schluessel(&w);
    let lauf = w.lauf(&args, &[]);
    assert_eq!(
        (lauf.exit(), lauf.stderr().as_str()),
        (Some(69), "lauf.modellweg_fehlt: Modellweg nicht startbar\n")
    );
    // only unbound targets: no key file needed, the list names none
    let w = Welt::neu();
    let args = w.cloud();
    let einst = gemeinsam::EINST.replacen(
        "\"zugriff\":\"kopf-einsetzen\",\"schluessel\":true",
        "\"zugriff\":\"kopf-einsetzen\",\"schluessel\":false",
        1,
    );
    datei(&w.einstellungen, einst.as_bytes(), 0o600);
    ohne_schluessel(&w);
    let lauf = w.lauf(&args, &[]);
    assert_eq!(lauf.exit(), Some(0), "{}", lauf.stderr());
    let liste =
        std::fs::read_to_string(w.zustand.join("sitzungen/s-1.zielliste.json")).expect("list");
    assert!(!liste.contains("schluessel_datei"), "{liste}");
    assert!(liste.contains("\"halter_ziel\":null"), "{liste}");
}

/// One inherited descriptor right below the descriptor of the listing is found too.
#[test]
fn an_inherited_descriptor_4_ends_the_start_at_step_6() {
    let w = Welt::neu();
    let args = w.lokal();
    let aus = Command::new(gemeinsam::BASH)
        .args([
            "-c",
            "for f in /proc/$$/fd/*; do n=${f##*/}; if [ \"$n\" -gt 2 ] 2>/dev/null; then eval \"exec $n>&-\" 2>/dev/null; fi; done; exec 4</dev/null; exec \"$0\" \"$@\"",
        ])
        .arg(w.bin.join("werkbank-lauf"))
        .args(&args)
        .env_clear()
        .env("TERM", "xterm")
        .stdin(Stdio::null())
        .gesperrt_output()
        .expect("run");
    assert_eq!(aus.status.code(), Some(69));
    assert_eq!(w.pid_von("rangkern"), None, "no child");
}

/// A `bwrap` that is refused like the real starter refuses (contract 1, "Lebensdauer"): its final call ends at once with
/// the code 5, as the child of the program.
fn ablehnender_bwrap(w: &Welt) {
    let skript = "#!/bin/sh\nfor a in \"$@\"; do last=\"$a\"; done\nif [ \"$last\" = /usr/bin/true ]; then exit 0; fi\nexit 5\n";
    datei(&w.bin.join("bwrap"), skript.as_bytes(), 0o755);
}

/// A second start file in the folder of the first, for another session.
fn zweite_sitzung(w: &Welt) -> Vec<String> {
    let erste = std::fs::read_to_string(w.startdatei()).expect("start file");
    let pfad = w.start.join("start-2.json");
    datei(&pfad, erste.replace("\"s-1\"", "\"s-2\"").as_bytes(), 0o600);
    vec![pfad.to_string_lossy().into_owned()]
}

fn laufzeit_ordner(w: &Welt, sitzung: &str) -> std::path::PathBuf {
    w.laufzeit.join(sitzung)
}

/// G1b, the case G1 did not meet: the starter refuses AFTER the `exec`. Then `kinder.beenden()` never runs again, and in
/// the delivered build the kernel does not deliver the death signal either (the profile of the starter has no `signal`
/// rule: `AppArmor` denies the send to the unconfined children, measured on Ubuntu 26.04). The children here ignore the
/// kernel signal (`taub:`), which is that case without a profile. The watcher ends them and removes the socket folder of
/// the session and the session folder of the harness; the files next to the start file stay for the core service.
#[test]
fn g1b_a_refusal_after_the_exec_leaves_no_child_and_no_folder() {
    for route in ["lokal", "cloud"] {
        let w = Welt::neu();
        ablehnender_bwrap(&w);
        w.verhalten("rangkern", "taub:bereit");
        w.verhalten("modellweg", "taub:bereit");
        let args = if route == "lokal" {
            w.lokal()
        } else {
            w.cloud()
        };
        let lauf = w.lauf(&args, &[]);
        assert_eq!(lauf.exit(), Some(5), "{route}: the refusal of the starter");
        let pids: Vec<i32> = ["rangkern", "modellweg"]
            .iter()
            .map(|p| w.pid_von(p).expect("started"))
            .collect();
        for p in &pids {
            assert!(
                endet_binnen(*p, HAENGESPERRE_MS),
                "{route}: child {p} outlived the refused start"
            );
        }
        ordner_weg(&w);
        // nothing of the world is left (the program, the parent of the starter, ended with its code)
        welt_leer(&w);
        // the files of the contract stay for the core service, which removes them at the end of the session
        assert!(
            w.start.join("suche.pid").exists(),
            "{route}: suche.pid stays"
        );
        // the session folder of the harness is gone as well (it stayed before, and the next start of the name met
        // `lauf.sitzung_belegt`)
        assert!(
            !w.sitzungen.join("s-1").exists(),
            "{route}: the session folder is gone"
        );
        for n in [
            "suche.pid",
            "modellweg.pid",
            "ca.key",
            "ca.crt",
            "ca-bundle.pem",
        ] {
            let _ = std::fs::remove_file(w.start.join(n));
        }
        // the conversation of the session stays after its end (contract 1, "Ablage des Gesprächs"): a second start of
        // the name ends at step 8 with `lauf.sitzung_belegt` and leaves the conversation as it was
        let gespraech = w.gespraeche.join("s-1/gespraech.json");
        let vorher = std::fs::read(&gespraech).expect("conversation of the first start");
        let belegt = w.lauf(&args, &[]);
        assert_eq!(belegt.exit(), Some(73), "{route}: {}", belegt.stderr());
        assert_eq!(
            std::fs::read(&gespraech).ok(),
            Some(vorher),
            "{route}: unchanged"
        );
        for n in [
            "suche.pid",
            "modellweg.pid",
            "ca.key",
            "ca.crt",
            "ca-bundle.pem",
        ] {
            let _ = std::fs::remove_file(w.start.join(n));
        }
        // once the service has cleared it, a second start of the name meets the starter again (the cloud route its
        // target list first: contract 1, step 7a0, which stays after a session)
        std::fs::remove_dir_all(w.gespraeche.join("s-1")).expect("clear the conversation");
        let noch = w.lauf(&args, &[]);
        if route == "lokal" {
            assert_eq!(noch.exit(), Some(5), "second start: {}", noch.stderr());
        } else {
            assert_eq!(noch.exit(), Some(73), "second start: {}", noch.stderr());
            assert_eq!(
                noch.stderr(),
                "lauf.zielliste: Zielliste nicht schreibbar\n"
            );
        }
        eprintln!("G1b {route}: refused start, children {pids:?} ended, folder gone");
    }
}

/// Waits for the removal of the socket folder of the session `s-1` (the watcher removes it after the children).
fn ordner_weg(w: &Welt) {
    let ende = std::time::Instant::now() + Duration::from_millis(HAENGESPERRE_MS);
    while laufzeit_ordner(w, "s-1").exists() && std::time::Instant::now() < ende {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(
        !laufzeit_ordner(w, "s-1").exists(),
        "the socket folder of the session is gone"
    );
}

/// G1b counter probe: with the children deaf to the signal and no watcher (a trampoline that only starts the program),
/// both stay as orphans after the refusal. This is the measured fault.
#[test]
fn g1b_counter_probe_without_the_watcher_the_children_stay() {
    let w = Welt::neu();
    ablehnender_bwrap(&w);
    w.verhalten("rangkern", "taub:bereit");
    w.verhalten("modellweg", "taub:bereit");
    datei(
        &w.bin.join("werkbank-lauf-kind"),
        b"#!/bin/sh\n[ \"$1\" = --wache ] && exit 0\nshift 2\nexec \"$@\"\n",
        0o755,
    );
    let args = w.lokal();
    let lauf = w.lauf(&args, &[]);
    assert_eq!(lauf.exit(), Some(5));
    std::thread::sleep(Duration::from_millis(1500));
    let pids: Vec<i32> = ["rangkern", "modellweg"]
        .iter()
        .map(|p| w.pid_von(p).expect("started"))
        .collect();
    let waisen: Vec<i32> = pids.iter().copied().filter(|p| lebt(*p)).collect();
    for p in &pids {
        if let Some(pid) = rustix::process::Pid::from_raw(*p) {
            let _ = rustix::process::kill_process(pid, rustix::process::Signal::KILL);
        }
    }
    assert_eq!(waisen.len(), 2, "without the watcher both children stay");
    assert!(laufzeit_ordner(&w, "s-1").exists(), "and the folder stays");
    assert!(w.sitzungen.join("s-1").exists(), "and the session folder");
}

/// The watcher ends the children also when the harness dies by SIGKILL.
#[test]
fn g1b_the_watcher_ends_the_children_when_the_harness_dies_by_sigkill() {
    let w = Welt::neu();
    schlafender_bwrap(&w);
    w.verhalten("rangkern", "taub:bereit");
    w.verhalten("modellweg", "taub:bereit");
    let args = w.lokal();
    let (mut kind, pids, harness) = starten(&w, &args);
    for p in &pids {
        assert!(lebt(*p), "child {p} runs while the harness runs");
    }
    assert!(laufzeit_ordner(&w, "s-1").exists());
    assert!(w.sitzungen.join("s-1").join("zustand").is_dir());
    hart_beenden(&mut kind, harness);
    for p in &pids {
        assert!(
            endet_binnen(*p, HAENGESPERRE_MS),
            "child {p} outlived the harness"
        );
    }
    ordner_weg(&w);
    assert!(
        !w.sitzungen.join("s-1").exists(),
        "the session folder is gone after SIGKILL of the harness"
    );
}

/// Finding 2 of the measurement: the second start from the same folder ended with `lauf.intern` (the first, refused
/// start had left `suche.pid`, which the core service removes). Now it ends with a code of its own, before any child.
#[test]
fn a_shared_start_folder_with_the_files_of_an_earlier_session_ends_with_its_own_code() {
    let w = Welt::neu();
    ablehnender_bwrap(&w);
    let erste = w.lokal();
    let zweite = zweite_sitzung(&w);
    assert_eq!(w.lauf(&erste, &[]).exit(), Some(5));
    let aus = w.lauf(&zweite, &[]);
    assert_eq!(aus.exit(), Some(73), "{}", aus.stderr());
    assert_eq!(
        aus.stderr(),
        "lauf.startordner_belegt: Startordner enthaelt Dateien einer fruehen Sitzung\n"
    );
}

#[test]
fn a_pid_file_of_an_earlier_session_ends_the_start_before_any_child() {
    for name in ["suche.pid", "modellweg.pid"] {
        let w = Welt::neu();
        let args = w.lokal();
        datei(&w.start.join(name), b"4242\n", 0o600);
        let aus = w.lauf(&args, &[]);
        assert_eq!(aus.exit(), Some(73), "{name}");
        assert_eq!(
            aus.stderr(),
            "lauf.startordner_belegt: Startordner enthaelt Dateien einer fruehen Sitzung\n",
            "{name}"
        );
        assert_eq!(w.pid_von("rangkern"), None, "{name}: no child");
        assert_eq!(
            std::fs::read_to_string(w.start.join(name)).expect("kept"),
            "4242\n",
            "{name}: the file of the earlier session is kept"
        );
    }
    // a link of that name counts too, and is never followed
    let w = Welt::neu();
    let args = w.lokal();
    std::os::unix::fs::symlink("/nonexistent", w.start.join("suche.pid")).expect("link");
    let aus = w.lauf(&args, &[]);
    assert_eq!(aus.exit(), Some(73));
}

/// A stand-in child that ignores SIGTERM: a shell script that sets the disposition (it is inherited over the `exec`)
/// and replaces itself by a copy of the stand-in under the name `<name>-echt`.
fn taubes_kind(w: &Welt, name: &str) {
    let echt = format!("{name}-echt");
    gemeinsam::kopie(gemeinsam::ATTRAPPE, &w.bin.join(&echt));
    w.verhalten(&echt, "bereit");
    let text = format!(
        "#!/bin/sh\ntrap '' TERM\nexec '{}' \"$@\"\n",
        w.bin.join(&echt).display()
    );
    datei(&w.bin.join(name), text.as_bytes(), 0o755);
}

/// The watcher gives SIGTERM the time of its grace and then sends SIGKILL: a child that ignores SIGTERM ends, and not
/// before the grace has passed (`WACHE_GNADE`, 2 s; the bound below is lower to stay clear of the clock).
#[test]
fn g1b_the_watcher_kills_a_child_that_ignores_sigterm_after_the_grace() {
    let w = Welt::neu();
    ablehnender_bwrap(&w);
    taubes_kind(&w, "rangkern");
    taubes_kind(&w, "modellweg");
    let args = w.lokal();
    let lauf = w.lauf(&args, &[]);
    let nach_lauf = std::time::Instant::now();
    assert_eq!(lauf.exit(), Some(5), "{}", lauf.stderr());
    for p in ["rangkern-echt", "modellweg-echt"] {
        let pid = w.pid_von(p).expect("started");
        assert!(endet_binnen(pid, HAENGESPERRE_MS), "{p} ended");
    }
    assert!(
        nach_lauf.elapsed() >= Duration::from_millis(1500),
        "SIGKILL came before the grace had passed: {:?}",
        nach_lauf.elapsed()
    );
    ordner_weg(&w);
}

/// A `bwrap` whose final call becomes a harness that ends by itself after a short while (a normal end).
fn kurzer_bwrap(w: &Welt) {
    let skript = "#!/bin/sh\nfor a in \"$@\"; do last=\"$a\"; done\nif [ \"$last\" = /usr/bin/true ]; then exit 0; fi\nexec sleep 2\n";
    datei(&w.bin.join("bwrap"), skript.as_bytes(), 0o755);
}

/// Session folder: a normal end of the harness. The folder exists while the harness runs (the harness reads its
/// configuration from it) and is gone after the end. Counter probe: with the removal taken out (a watcher without the
/// session folder, here a trampoline that does nothing) the folder stays.
#[test]
fn the_session_folder_is_removed_at_the_normal_end_of_the_harness() {
    let w = Welt::neu();
    kurzer_bwrap(&w);
    let args = w.lokal();
    let mut kind = gemeinsam::ohne_fremde_deskriptoren(&w.bin.join("werkbank-lauf"))
        .args(&args)
        .env_clear()
        .env("TERM", "xterm")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .gesperrt_spawn()
        .expect("start");
    let ende = std::time::Instant::now() + Duration::from_secs(20);
    while !w.sitzungen.join("s-1/zustand").is_dir() {
        assert!(std::time::Instant::now() < ende, "no session folder");
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(
        w.sitzungen.join("s-1").is_dir(),
        "it exists while the harness runs"
    );
    assert_eq!(kind.wait().expect("end").code(), Some(0));
    ordner_weg(&w);
    assert!(
        !w.sitzungen.join("s-1").exists(),
        "the session folder is gone after the end of the harness"
    );
}

/// Counter probe of the removal: a watcher that does nothing leaves the folder (so the tests above fail without it).
#[test]
fn counter_probe_without_the_watcher_the_session_folder_stays_at_the_end() {
    let w = Welt::neu();
    kurzer_bwrap(&w);
    datei(
        &w.bin.join("werkbank-lauf-kind"),
        b"#!/bin/sh\n[ \"$1\" = --wache ] && exit 0\nshift 2\nexec \"$@\"\n",
        0o755,
    );
    let args = w.lokal();
    let lauf = w.lauf(&args, &[]);
    assert_eq!(lauf.exit(), Some(0));
    std::thread::sleep(Duration::from_millis(500));
    assert!(w.sitzungen.join("s-1").is_dir(), "the folder stays");
}

/// A fault before the `exec` after the folder was made (the starter stand-in refuses its probe run): the program
/// removes the folder itself, and the watcher was never started.
#[test]
fn a_fault_before_the_exec_removes_the_session_folder() {
    let w = Welt::neu();
    let skript = "#!/bin/sh\nexit 1\n";
    datei(&w.bin.join("bwrap"), skript.as_bytes(), 0o755);
    let args = w.lokal();
    let lauf = w.lauf(&args, &[]);
    assert_eq!(lauf.exit(), Some(69), "{}", lauf.stderr());
    assert_eq!(
        lauf.stderr(),
        "lauf.einschluss_fehlt: Einschluss nicht verfuegbar\n"
    );
    assert!(
        !w.sitzungen.join("s-1").exists(),
        "the session folder is gone after the fault"
    );
    assert!(!laufzeit_ordner(&w, "s-1").exists());
}

/// A name that was taken before is not this run's: the start ends with `lauf.sitzung_belegt` and the folder of the
/// earlier session stays, with its content.
#[test]
fn a_session_folder_that_was_there_before_stays() {
    let w = Welt::neu();
    ablehnender_bwrap(&w);
    std::fs::create_dir(w.sitzungen.join("s-1")).expect("folder");
    std::fs::write(w.sitzungen.join("s-1/frueher.txt"), b"x").expect("file");
    let args = w.lokal();
    let lauf = w.lauf(&args, &[]);
    assert_eq!(lauf.exit(), Some(73), "{}", lauf.stderr());
    assert_eq!(
        lauf.stderr(),
        "lauf.sitzung_belegt: Sitzung wurde schon gestartet\n"
    );
    std::thread::sleep(Duration::from_millis(500));
    assert!(w.sitzungen.join("s-1/frueher.txt").is_file());
}

/// A link in the place of the session folder is never followed: the start ends with `lauf.sitzung_belegt`, the link
/// and the folder it points to stay as they were.
#[test]
fn a_link_in_the_place_of_the_session_folder_is_neither_followed_nor_removed() {
    let w = Welt::neu();
    ablehnender_bwrap(&w);
    let fremd = w.wurzel.join("fremd");
    std::fs::create_dir(&fremd).expect("folder");
    std::fs::write(fremd.join("wichtig.txt"), b"x").expect("file");
    std::os::unix::fs::symlink(&fremd, w.sitzungen.join("s-1")).expect("link");
    let args = w.lokal();
    let lauf = w.lauf(&args, &[]);
    assert_eq!(lauf.exit(), Some(73), "{}", lauf.stderr());
    std::thread::sleep(Duration::from_millis(500));
    assert!(
        fremd.join("wichtig.txt").is_file(),
        "the target is untouched"
    );
    assert!(
        std::fs::symlink_metadata(w.sitzungen.join("s-1"))
            .expect("link")
            .file_type()
            .is_symlink(),
        "the link is untouched"
    );
}

/// The name is swapped for a link while the harness runs: the watcher does not follow it. The folder it points to and
/// the link stay.
#[test]
fn the_watcher_does_not_follow_a_link_swapped_in_for_the_session_folder() {
    let w = Welt::neu();
    schlafender_bwrap(&w);
    w.verhalten("rangkern", "taub:bereit");
    w.verhalten("modellweg", "taub:bereit");
    let args = w.lokal();
    let (mut kind, _pids, harness) = starten(&w, &args);
    let fremd = w.wurzel.join("fremd");
    std::fs::create_dir(&fremd).expect("folder");
    std::fs::write(fremd.join("wichtig.txt"), b"x").expect("file");
    std::fs::remove_dir_all(w.sitzungen.join("s-1")).expect("remove");
    std::os::unix::fs::symlink(&fremd, w.sitzungen.join("s-1")).expect("link");
    hart_beenden(&mut kind, harness);
    ordner_weg(&w);
    assert!(
        fremd.join("wichtig.txt").is_file(),
        "the target is untouched"
    );
    assert!(
        std::fs::symlink_metadata(w.sitzungen.join("s-1"))
            .expect("link")
            .file_type()
            .is_symlink(),
        "the link is untouched"
    );
}

/// Starts the two stand-in children of `w` the way a test does and forgets them (no `wait`, no kill): what a red test
/// leaves behind when it stops at an assertion.
fn kinder_vergessen(w: &Welt) {
    for name in ["rangkern", "modellweg"] {
        w.verhalten(name, "stumm");
        let kind = Command::new(w.bin.join(name))
            .env_clear()
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .gesperrt_spawn()
            .expect("start");
        std::mem::forget(kind);
    }
    // `spawn` returns before the new program shows in `/proc`: wait until both are listed
    let ende = std::time::Instant::now() + Duration::from_secs(10);
    while prozesse_unter(&w.wurzel).len() < 2 {
        assert!(
            std::time::Instant::now() < ende,
            "the children did not start"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// The world ends every process that runs a program of its folder, also when the test is red: a test that panics after
/// the children started leaves none under its folder. Counter probe: a world that does not clean up leaves them (the
/// listing sees them), so the test proves the effect of the clean-up and not a blind listing.
#[test]
fn a_red_test_leaves_no_process_under_its_folder() {
    let wurzel = std::sync::Mutex::new(None);
    let rot = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let w = Welt::neu();
        *wurzel.lock().expect("lock") = Some(w.wurzel.clone());
        kinder_vergessen(&w);
        assert_eq!(prozesse_unter(&w.wurzel).len(), 2, "both children run");
        panic!("red on purpose after the start of the children");
    }));
    assert!(rot.is_err(), "the closure was red");
    let wurzel = wurzel.lock().expect("lock").clone().expect("folder");
    assert_eq!(
        prozesse_unter(&wurzel),
        [],
        "the clean-up of the world ended every process"
    );

    // counter probe: without the clean-up the same start leaves both
    let mut w = Welt::neu();
    w.ohne_aufraeumen = true;
    kinder_vergessen(&w);
    let wurzel = w.wurzel.clone();
    let da = prozesse_unter(&wurzel);
    assert_eq!(da.len(), 2, "without the clean-up both children stay");
    drop(w);
    assert_eq!(
        prozesse_unter(&wurzel).len(),
        2,
        "still there after the drop"
    );
    // the test ends them itself, by the same function the world uses
    prozesse_unter_beenden(&wurzel);
    assert_eq!(prozesse_unter(&wurzel), []);
}

/// The clean-up compares command line and start time again before a signal: a process with the PID of the list but
/// another start time is left alone.
#[test]
fn the_clean_up_leaves_a_reused_pid_alone() {
    let w = Welt::neu();
    kinder_vergessen(&w);
    let liste = prozesse_unter(&w.wurzel);
    assert_eq!(liste.len(), 2);
    // a process of the list whose start time is not the one of the live process
    let falsch = gemeinsam::Prozess {
        start: liste[0].start + 1,
        ..liste[0].clone()
    };
    assert!(!gemeinsam::gleicher_prozess(&falsch));
    assert!(gemeinsam::gleicher_prozess(&liste[0]));
}

/// A `bwrap` whose final call becomes a harness that takes SIGHUP and SIGTERM: it writes the name of the signal into
/// `signal.txt` of the world and ends with 3 (SIGHUP) or 4 (SIGTERM). `bereit.txt` tells that the traps are set. Its
/// `sleep` runs in the background, so it is left as an orphan when the harness ends: the program adopts and ends it.
fn fangender_bwrap(w: &Welt) {
    let f = w.wurzel.join("signal.txt");
    let r = w.wurzel.join("bereit.txt");
    let skript = format!(
        "#!/bin/sh\nfor a in \"$@\"; do last=\"$a\"; done\nif [ \"$last\" = /usr/bin/true ]; then exit 0; fi\nexec /bin/sh -c 'trap \"echo hup > {f}; exit 3\" HUP; trap \"echo term > {f}; exit 4\" TERM; : > {r}; while :; do sleep 1 & wait $!; done'\n",
        f = f.display(),
        r = r.display()
    );
    datei(&w.bin.join("bwrap"), skript.as_bytes(), 0o755);
}

/// Starts the program with [`fangender_bwrap`] and waits until both children run and the traps are set.
fn fangend_starten(w: &Welt) -> (std::process::Child, Vec<i32>) {
    fangender_bwrap(w);
    let mut kind = gemeinsam::ohne_fremde_deskriptoren(&w.bin.join("werkbank-lauf"))
        .args(w.lokal())
        .env_clear()
        .env("TERM", "xterm")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .gesperrt_spawn()
        .expect("start");
    let pids = bereit_abwarten(&mut kind, || {
        let pids = w.kinder_pids();
        (pids.len() == 2 && w.wurzel.join("bereit.txt").exists()).then_some(pids)
    });
    (kind, pids)
}

fn signal_an(pid: u32, signal: rustix::process::Signal) {
    let pid = i32::try_from(pid)
        .ok()
        .and_then(rustix::process::Pid::from_raw)
        .expect("pid");
    rustix::process::kill_process(pid, signal).expect("signal");
}

/// Waits until nothing runs under the folder of the world any more.
fn welt_leer(w: &Welt) {
    let ende = std::time::Instant::now() + Duration::from_millis(HAENGESPERRE_MS);
    while !prozesse_unter(&w.wurzel).is_empty() && std::time::Instant::now() < ende {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(
        prozesse_unter(&w.wurzel),
        [],
        "nothing of the world is left"
    );
}

/// SIGHUP and SIGTERM of the core service reach the starter (contract 1, "Lebensdauer"): the harness takes the signal
/// itself, so the program ends with the code of the harness (3, 4) and not by the signal. The adopted `sleep` of the
/// harness is ended, the watcher ends both children and removes the folders.
#[test]
fn the_program_passes_sighup_and_sigterm_on_to_the_starter() {
    for (signal, name, code) in [
        (rustix::process::Signal::HUP, "hup", 3),
        (rustix::process::Signal::TERM, "term", 4),
    ] {
        let w = Welt::neu();
        let (mut kind, pids) = fangend_starten(&w);
        signal_an(kind.id(), signal);
        let status = kind.wait().expect("wait");
        assert_eq!(status.code(), Some(code), "{name}: the code of the harness");
        assert_eq!(
            std::fs::read_to_string(w.wurzel.join("signal.txt"))
                .expect("the harness got the signal"),
            format!("{name}\n")
        );
        for p in &pids {
            assert!(
                endet_binnen(*p, HAENGESPERRE_MS),
                "{name}: child {p} outlived the session"
            );
        }
        ordner_weg(&w);
        assert!(!w.sitzungen.join("s-1").exists(), "{name}: session folder");
        welt_leer(&w);
    }
}

/// A starter that dies by a signal ends the program by the same signal, so the carrier counts `128 + n` as before (the
/// harness `sleep` has no trap: SIGHUP and SIGTERM end it, and with it the starter stand-in it replaced).
#[test]
fn the_program_ends_by_the_signal_of_the_starter() {
    use std::os::unix::process::ExitStatusExt;
    for (signal, nummer) in [
        (rustix::process::Signal::HUP, 1),
        (rustix::process::Signal::TERM, 15),
    ] {
        let w = Welt::neu();
        schlafender_bwrap(&w);
        let (mut kind, pids, harness) = starten(&w, &w.lokal());
        signal_an(kind.id(), signal);
        let status = kind.wait().expect("wait");
        assert_eq!(status.signal(), Some(nummer), "{status:?}");
        assert!(!lebt(harness), "the harness ended");
        for p in &pids {
            assert!(
                endet_binnen(*p, HAENGESPERRE_MS),
                "child {p} outlived the session"
            );
        }
        ordner_weg(&w);
        welt_leer(&w);
    }
}

/// SIGINT and SIGQUIT reach the program from the terminal together with the starter (one foreground group); the program
/// takes them and passes them on to nobody, so the harness gets each once. The program and the harness keep running.
#[test]
fn the_program_takes_sigint_and_sigquit_and_does_not_pass_them_on() {
    let w = Welt::neu();
    let (mut kind, _pids) = fangend_starten(&w);
    for s in [rustix::process::Signal::INT, rustix::process::Signal::QUIT] {
        signal_an(kind.id(), s);
    }
    std::thread::sleep(Duration::from_millis(500));
    assert!(
        matches!(kind.try_wait(), Ok(None)),
        "the program runs after SIGINT and SIGQUIT"
    );
    assert!(
        !w.wurzel.join("signal.txt").exists(),
        "nothing reached the harness"
    );
    signal_an(kind.id(), rustix::process::Signal::TERM);
    assert_eq!(kind.wait().expect("wait").code(), Some(4));
    ordner_weg(&w);
    welt_leer(&w);
}

/// A `bwrap` whose final call ends at once with the code 42.
fn bwrap_mit_code(w: &Welt) {
    let skript = "#!/bin/sh\nfor a in \"$@\"; do last=\"$a\"; done\nif [ \"$last\" = /usr/bin/true ]; then exit 0; fi\nexit 42\n";
    datei(&w.bin.join("bwrap"), skript.as_bytes(), 0o755);
}

/// The program ends with the code of the starter, writes no line of its own and leaves nothing.
#[test]
fn the_program_ends_with_the_code_of_the_starter() {
    let w = Welt::neu();
    bwrap_mit_code(&w);
    let lauf = w.lauf(&w.lokal(), &[]);
    assert_eq!(lauf.exit(), Some(42));
    assert_eq!(lauf.stderr(), "");
    ordner_weg(&w);
    welt_leer(&w);
}

/// The parent of the starter leaves the search process and the model path to the watcher (contract 1, "Lebensdauer"):
/// both ignore SIGTERM here ([`taubes_kind`]; the parent would end a foreign child with SIGKILL at once), so right after
/// the end of the program they still run (the watcher gives them `WACHE_GNADE`, 2 s), then the watcher ends them and
/// removes the session folder and the socket folder.
#[test]
fn the_watcher_and_not_the_parent_ends_the_children_and_then_removes_the_folders() {
    let w = Welt::neu();
    kurzer_bwrap(&w);
    taubes_kind(&w, "rangkern");
    taubes_kind(&w, "modellweg");
    let lauf = w.lauf(&w.lokal(), &[]);
    assert_eq!(lauf.exit(), Some(0));
    let pids: Vec<i32> = ["rangkern-echt", "modellweg-echt"]
        .iter()
        .map(|p| w.pid_von(p).expect("started"))
        .collect();
    for p in &pids {
        assert!(
            lebt(*p),
            "child {p} still runs right after the end of the program"
        );
    }
    assert!(
        w.sitzungen.join("s-1").exists(),
        "the folder is still there"
    );
    for p in &pids {
        assert!(
            endet_binnen(*p, HAENGESPERRE_MS),
            "child {p} outlived the watcher"
        );
    }
    ordner_weg(&w);
    assert!(
        !w.sitzungen.join("s-1").exists(),
        "the watcher removed the session folder"
    );
    welt_leer(&w);
}

/// After the hand-over the program holds the read end of the output pipe of neither child (the `exec` closed it
/// before): a child that writes more than a pipe holds is not stopped by a reader that never reads.
#[test]
fn the_program_holds_no_output_pipe_of_the_children_after_the_hand_over() {
    let w = Welt::neu();
    schlafender_bwrap(&w);
    let (mut kind, pids, harness) = starten(&w, &w.lokal());
    let ziele = |pid: String| -> Vec<String> {
        std::fs::read_dir(format!("/proc/{pid}/fd"))
            .map(|l| {
                l.filter_map(Result::ok)
                    .filter_map(|e| std::fs::read_link(e.path()).ok())
                    .map(|z| z.to_string_lossy().into_owned())
                    .collect()
            })
            .unwrap_or_default()
    };
    let eigene = ziele(kind.id().to_string());
    assert!(
        eigene
            .iter()
            .any(|z| z.starts_with("pipe:") || z.starts_with('/')),
        "positive control: the descriptors of the program are readable: {eigene:?}"
    );
    for p in &pids {
        let rohr = std::fs::read_link(format!("/proc/{p}/fd/1"))
            .expect("standard output of the child")
            .to_string_lossy()
            .into_owned();
        assert!(rohr.starts_with("pipe:"), "child {p}: {rohr}");
        assert!(
            ziele(p.to_string()).contains(&rohr),
            "positive control: the search finds the pipe {rohr} at child {p}"
        );
        assert!(
            !eigene.contains(&rohr),
            "the program still holds the output pipe {rohr} of child {p}: {eigene:?}"
        );
    }
    hart_beenden(&mut kind, harness);
}

/// A starter that cannot be run ends the trampoline with the code and the line of `lauf.harness_fehlt` (69), like the
/// `exec` of the program did before; a child that cannot be run keeps 127. The trampoline runs as its own process with
/// this test as its real parent: a failed `exec` inside the test process would swap its environment while other tests
/// start processes.
#[test]
fn a_starter_that_cannot_be_run_ends_with_the_code_of_lauf_harness_fehlt() {
    let eltern = std::process::id().to_string();
    for (art, code) in [("--halten", 69), ("--", 127)] {
        let aus = Command::new(gemeinsam::KIND)
            .args([eltern.as_str(), art, "/nicht/da/werkbank-einschluss"])
            .env_clear()
            .stdin(Stdio::null())
            .gesperrt_output()
            .expect("trampoline");
        assert_eq!(aus.status.code(), Some(code), "{art}");
        let zeile = String::from_utf8_lossy(&aus.stderr);
        assert_eq!(
            zeile.starts_with("lauf.harness_fehlt"),
            art == "--halten",
            "{art}: {zeile}"
        );
    }
}

/// A start may take longer than any fixed time and is still good: contract 1 gives each child up to `BEREIT_FRIST`
/// (10 s) for its ready line, and the starter has no deadline. Here the search process and the model path take 5 s each
/// and the starter 12 s more, so the harness runs only after at least 22 s. The tests wait for the ready signal of the
/// harness, not for a time (a fixed 20 s failed this case). Each child keeps 5 s to its deadline: with 9 s each, a
/// loaded build host once missed the 10 s of the model path (`lauf.modellweg_nicht_bereit`).
#[test]
fn a_start_slower_than_twenty_seconds_still_reaches_the_harness() {
    let w = Welt::neu();
    verzoegerter_bwrap(&w, 12);
    w.verhalten("rangkern", "spaet:5000:bereit");
    w.verhalten("modellweg", "spaet:5000:bereit");
    let beginn = std::time::Instant::now();
    let (mut kind, pids, harness) = starten(&w, &w.lokal());
    let dauer = beginn.elapsed();
    assert!(
        dauer >= Duration::from_secs(21),
        "the condition holds: the start took {dauer:?}"
    );
    for p in &pids {
        assert!(lebt(*p), "child {p} runs while the harness runs");
    }
    hart_beenden(&mut kind, harness);
    for p in &pids {
        assert!(
            endet_binnen(*p, HAENGESPERRE_MS),
            "child {p} outlived the harness"
        );
    }
    eprintln!("slow start: harness after {dauer:?}");
}
