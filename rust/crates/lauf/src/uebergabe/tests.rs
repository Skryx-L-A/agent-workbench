//! Tests of the hand-over without root and without the starter. The reserved descriptor 3 exists only in a process
//! that starts with 0, 1, 2 as the lowest used numbers, so these tests start the test program again as a helper
//! process (`--exact kind_fall`), once with a clean set of descriptors and once with descriptor 3 handed down.

#[cfg(target_os = "linux")]
use std::process::{Command, Stdio};
#[cfg(target_os = "linux")]
use std::time::{Duration, Instant};

use super::*;

/// The variable that names the case of the helper process; the helper test does nothing without it.
#[cfg(target_os = "linux")]
const FALL: &str = "WB_LAUF_UEBERGABE_FALL";
#[cfg(target_os = "linux")]
const OK: &str = "UEBERGABE-OK";

#[test]
fn the_platform_rule_takes_the_build_the_place_and_a_handed_down_descriptor() {
    // delivered build: only the place counts
    assert!(platz_gut(false, true, true));
    assert!(!platz_gut(false, false, false));
    assert!(!platz_gut(false, false, true));
    // test build: no place is needed, but a descriptor 3 that was handed down is refused
    assert!(platz_gut(true, true, true));
    assert!(platz_gut(true, false, false));
    assert!(!platz_gut(true, false, true));
}

#[cfg(target_os = "linux")]
fn kind_laufen(fall: &str, umlenkung: &str) -> (Option<i32>, String) {
    let exe = std::env::current_exe().expect("test program");
    // the shell sets descriptor 3 for the helper: handed down (an open file) or closed. The test runner of cargo may
    // hold a descriptor 3 itself (the job server), which would take the place of the reserved one.
    // Every descriptor above 3 that the shell holds is closed, whatever its number: a runner can hand down descriptors far
    // above 63 without close-on-exec (the GitHub runner: 142 and 145).
    // (bash, not a shell that knows single-digit descriptors only: the numbers above 9 are closed too)
    let mut c = Command::new("bash");
    c.args([
        "-c",
        &format!(
            "for f in /proc/$$/fd/*; do n=${{f##*/}}; if [ \"$n\" -gt 3 ]; then eval \"exec $n>&-\"; fi; done; exec \"$0\" \"$@\" {umlenkung}"
        ),
    ])
    .arg(&exe);
    c.args([
        "--exact",
        "uebergabe::tests::kind_fall",
        "--nocapture",
        "--test-threads=1",
    ])
    .env(FALL, fall)
    .stdin(Stdio::null())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped());
    let mut kind = c.spawn().expect("helper process");
    let frist = Instant::now() + Duration::from_secs(30);
    let status = loop {
        if let Some(s) = kind.try_wait().expect("wait") {
            break Some(s);
        }
        if Instant::now() > frist {
            let _ = kind.kill();
            let _ = kind.wait();
            break None;
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    let mut aus = String::new();
    let mut fehler = String::new();
    {
        use std::io::Read;
        if let Some(mut o) = kind.stdout.take() {
            let _ = o.read_to_string(&mut aus);
        }
        if let Some(mut e) = kind.stderr.take() {
            let _ = e.read_to_string(&mut fehler);
        }
    }
    (
        status.map(|s| s.code().unwrap_or(-1)),
        format!("{aus}{fehler}"),
    )
}

#[cfg(target_os = "linux")]
fn kind_gut(fall: &str, umlenkung: &str) {
    let (code, text) = kind_laufen(fall, umlenkung);
    assert_eq!(code, Some(0), "{fall}: {text}");
    assert!(text.contains(OK), "{fall}: {text}");
}

/// The helper process: runs the case named in the environment, else nothing.
#[test]
fn kind_fall() {
    #[cfg(target_os = "linux")]
    {
        let Ok(fall) = std::env::var(FALL) else {
            return;
        };
        match fall.as_str() {
            "legen" => fall_legen(),
            "gross" => fall_gross(),
            "vererbt" => fall_vererbt(),
            "fremde" => fall_fremde(),
            "frei" => fall_frei(),
            "leine" => fall_leine(),
            "leine-vier" => fall_leine_vier(),
            "leine-belegt" => fall_leine_belegt(),
            andere => panic!("unknown case {andere}"),
        }
        #[allow(clippy::print_stdout)]
        {
            println!("{OK}");
        }
    }
}

#[cfg(target_os = "linux")]
fn fall_legen() {
    use std::io::Read;
    let mut platz = Platz::reservieren();
    assert!(platz.da(), "descriptor 3 is the lowest free number");
    assert!(drei_belegt());
    // the reserved descriptor itself carries close-on-exec until the hand-over
    let vorher = rustix::io::fcntl_getfd(platz.0.as_ref().expect("place")).expect("flags");
    assert!(vorher.contains(rustix::io::FdFlags::CLOEXEC));
    auf_drei_legen(&mut platz, b"hallo").expect("hand over");
    // descriptor 3 is the read end of a pipe that holds the text and ends there; close-on-exec is cleared
    let mut text = String::new();
    std::fs::File::open("/proc/self/fd/3")
        .expect("descriptor 3")
        .read_to_string(&mut text)
        .expect("read");
    assert_eq!(text, "hallo");
    let flags = rustix::io::fcntl_getfd(platz.0.as_ref().expect("place")).expect("flags");
    assert!(!flags.contains(rustix::io::FdFlags::CLOEXEC));
}

#[cfg(target_os = "linux")]
fn fall_gross() {
    use std::io::Read;
    let mut platz = Platz::reservieren();
    assert!(platz.da());
    // one byte over the limit of the starter: refused before anything is written, no hang (a write that blocks fails
    // the case within its limit instead of hanging it)
    let zu_gross = vec![b'x'; einschluss::EINGABE_MAX + 1];
    let (r, mut platz) = crate::testhilfe::mit_frist(Duration::from_secs(5), move || {
        let r = auf_drei_legen(&mut platz, &zu_gross);
        (r, platz)
    });
    assert_eq!(r, Err(Fehler::EinschlussFehlt));
    // the limit itself fits into the pipe
    let genau = vec![b'y'; einschluss::EINGABE_MAX];
    auf_drei_legen(&mut platz, &genau).expect("exactly the limit");
    let mut n = 0;
    std::fs::File::open("/proc/self/fd/3")
        .expect("descriptor 3")
        .read_to_end(&mut Vec::new())
        .map(|k| n = k)
        .expect("read");
    assert_eq!(n, einschluss::EINGABE_MAX);
}

#[cfg(target_os = "linux")]
fn fall_vererbt() {
    // the shell handed down descriptor 3: the place is not held, the descriptor is taken, and nothing is handed over
    let mut platz = Platz::reservieren();
    assert!(!platz.da());
    assert!(drei_belegt());
    assert!(!platz_gut(true, platz.da(), drei_belegt()));
    assert_eq!(
        auf_drei_legen(&mut platz, b"x"),
        Err(Fehler::EinschlussFehlt)
    );
}

#[cfg(target_os = "linux")]
#[test]
fn the_place_holds_descriptor_3_and_the_text_reaches_it() {
    kind_gut("legen", "3>&-");
}

#[cfg(target_os = "linux")]
#[test]
fn a_description_over_the_limit_of_the_starter_ends_the_start_instead_of_hanging() {
    kind_gut("gross", "3>&-");
}

#[cfg(target_os = "linux")]
#[test]
fn a_descriptor_3_handed_down_is_no_place() {
    kind_gut("vererbt", "3</dev/null");
}

#[cfg(target_os = "linux")]
fn fall_fremde() {
    // the shell handed down descriptor 4 (a file under /proc: not the listing of the descriptors) and closed 3 and 5 to 9
    let platz = Platz::reservieren();
    assert!(platz.da());
    // the place holds 5 itself (the lifeline): it is its own, not inherited
    assert_eq!(platz.eigene(), vec![5]);
    assert_eq!(fremde_deskriptoren(), Some(vec![4, 5]));
}

#[cfg(target_os = "linux")]
fn fall_frei() {
    // descriptor 3 is open while the place holds it and free after the place is dropped
    let platz = Platz::reservieren();
    assert!(platz.da());
    assert!(drei_belegt());
    drop(platz);
    assert!(!drei_belegt());
}

#[cfg(target_os = "linux")]
#[test]
fn a_handed_down_descriptor_is_named_and_the_own_listing_is_not() {
    kind_gut("fremde", "3>&- 4</proc/version");
}

#[cfg(target_os = "linux")]
#[test]
fn descriptor_3_is_taken_while_the_place_holds_it_and_free_after() {
    kind_gut("frei", "3>&-");
}

/// Reads descriptor 5 once without blocking (a new open of the pipe): `Some(true)` for end of file, `Some(false)` for
/// "no data yet", `None` for anything else.
#[cfg(target_os = "linux")]
fn leine_zu() -> Option<bool> {
    let fd = rustix::fs::open(
        "/proc/self/fd/5",
        rustix::fs::OFlags::RDONLY | rustix::fs::OFlags::NONBLOCK | rustix::fs::OFlags::CLOEXEC,
        rustix::fs::Mode::empty(),
    )
    .ok()?;
    let mut puffer = [0u8; 1];
    match rustix::io::read(&fd, &mut puffer) {
        Ok(0) => Some(true),
        Err(rustix::io::Errno::AGAIN) => Some(false),
        _ => None,
    }
}

#[cfg(target_os = "linux")]
fn offen(n: i32) -> bool {
    std::fs::symlink_metadata(format!("/proc/self/fd/{n}")).is_ok()
}

#[cfg(target_os = "linux")]
fn fall_leine() {
    // 3 and 4 are free at the start (no report): the place takes 3, the lifeline 5, and 4 is free again
    let mut platz = Platz::reservieren();
    assert!(platz.da());
    assert!(platz.leine_da(), "descriptor 5 reserved");
    assert_eq!(platz.eigene(), vec![einschluss::LEINE_FD]);
    assert!(!offen(4), "the filler on 4 is closed again");
    let vorher = rustix::io::fcntl_getfd(platz.1.as_ref().expect("lifeline")).expect("flags");
    assert!(vorher.contains(rustix::io::FdFlags::CLOEXEC));
    let schreiben = platz.leine_legen().expect("lifeline");
    // 5 is the read end of a pipe without close-on-exec, the write end keeps close-on-exec
    let flags = rustix::io::fcntl_getfd(platz.1.as_ref().expect("lifeline")).expect("flags");
    assert!(!flags.contains(rustix::io::FdFlags::CLOEXEC));
    assert!(
        rustix::io::fcntl_getfd(&schreiben)
            .expect("flags")
            .contains(rustix::io::FdFlags::CLOEXEC)
    );
    assert_eq!(
        leine_zu(),
        Some(false),
        "the lifeline lives while the writer is open"
    );
    drop(schreiben);
    assert_eq!(
        leine_zu(),
        Some(true),
        "end of file once the writer is gone"
    );
    platz.1 = None;
    assert!(!offen(5));
}

#[cfg(target_os = "linux")]
fn fall_leine_vier() {
    // descriptor 4 is held (as by the report): the lifeline takes 5 and 4 stays open
    let platz = Platz::reservieren();
    assert!(platz.leine_da());
    assert!(offen(4));
    assert_eq!(platz.eigene(), vec![5]);
}

#[cfg(target_os = "linux")]
fn fall_leine_belegt() {
    // descriptor 5 was handed down: no lifeline, and the start refuses
    let mut platz = Platz::reservieren();
    assert!(platz.da());
    assert!(!platz.leine_da());
    assert_eq!(platz.eigene(), Vec::<i32>::new());
    assert!(matches!(platz.leine_legen(), Err(Fehler::EinschlussFehlt)));
    assert!(!offen(4), "the filler on 4 is closed again");
}

#[cfg(target_os = "linux")]
#[test]
fn the_lifeline_takes_descriptor_5_and_reads_end_of_file_once_its_writer_is_gone() {
    kind_gut("leine", "3>&-");
    kind_gut("leine-vier", "3>&- 4</proc/version");
    kind_gut("leine-belegt", "3>&- 5</proc/version");
}

#[test]
fn only_sighup_and_sigterm_go_on_to_the_starter() {
    use signal_hook::consts::{SIGHUP, SIGINT, SIGKILL, SIGQUIT, SIGTERM, SIGUSR1, SIGWINCH};
    assert!(weiterreichen(SIGHUP));
    assert!(weiterreichen(SIGTERM));
    for s in [SIGINT, SIGQUIT, SIGUSR1, SIGWINCH, SIGKILL, 0] {
        assert!(!weiterreichen(s), "{s}");
    }
    // the taken signals are never passed on
    assert!(AUFFANGEN.iter().all(|s| !weiterreichen(*s)));
}

#[test]
fn the_known_children_stay_and_every_other_is_foreign() {
    assert_eq!(fremde(&[10, 11, 12, 13], &[11, 13]), vec![10, 12]);
    assert_eq!(fremde(&[10, 11], &[]), vec![10, 11]);
    assert_eq!(fremde(&[10, 11], &[10, 11, 12]), Vec::<u32>::new());
    assert_eq!(fremde(&[], &[1]), Vec::<u32>::new());
}

#[cfg(target_os = "linux")]
#[test]
fn the_end_of_the_starter_is_its_code_or_its_signal() {
    let lauf = |skript: &str| {
        Command::new("sh")
            .args(["-c", skript])
            .status()
            .expect("sh")
    };
    assert_eq!(ausgang(lauf("exit 0")), Ausgang::Code(0));
    assert_eq!(ausgang(lauf("exit 42")), Ausgang::Code(42));
    assert_eq!(ausgang(lauf("kill -TERM $$")), Ausgang::Signal(15));
    assert_eq!(ausgang(lauf("kill -KILL $$")), Ausgang::Signal(9));
}

#[cfg(target_os = "linux")]
#[test]
fn the_own_children_are_found_until_they_are_waited_for() {
    let mut k = Command::new("sleep").arg("30").spawn().expect("sleep");
    assert!(
        eigene_kinder().contains(&k.id()),
        "a running child is listed"
    );
    let _ = k.kill();
    let _ = k.wait();
    assert!(!eigene_kinder().contains(&k.id()), "a waited child is gone");
}

/// The helper process of the cases of the end (see `crate::testhilfe`): a case that ends the process never writes the
/// line of success; its end is what the test checks.
#[test]
fn kind_ende() {
    #[cfg(target_os = "linux")]
    {
        let Some(fall) = crate::testhilfe::fall() else {
            return;
        };
        match fall.as_str() {
            "code" => enden(Ausgang::Code(7)),
            "term" => enden(Ausgang::Signal(signal_hook::consts::SIGTERM)),
            "kill" => enden(Ausgang::Signal(signal_hook::consts::SIGKILL)),
            "segv" => enden(Ausgang::Signal(signal_hook::consts::SIGSEGV)),
            "fremde" => fall_fremde_beenden(),
            "keine-fremden" => fall_keine_fremden(),
            andere => panic!("unknown case {andere}"),
        }
        crate::testhilfe::fertig();
    }
}

#[cfg(target_os = "linux")]
const KIND_ENDE: &str = "uebergabe::tests::kind_ende";

#[cfg(target_os = "linux")]
#[test]
fn the_process_ends_with_the_code_or_by_the_signal_of_the_starter() {
    use std::os::unix::process::ExitStatusExt;
    let lauf = |fall: &str| {
        let (status, text) =
            crate::testhilfe::helfer(KIND_ENDE, fall, "3>&-", Duration::from_secs(30));
        let status = status.unwrap_or_else(|| panic!("{fall} did not end: {text}"));
        (status.code(), status.signal())
    };
    assert_eq!(lauf("code"), (Some(7), None));
    assert_eq!(lauf("term"), (None, Some(signal_hook::consts::SIGTERM)));
    assert_eq!(lauf("kill"), (None, Some(signal_hook::consts::SIGKILL)));
    // a signal whose default action cannot be called up here: the code 128 + n
    assert_eq!(
        lauf("segv"),
        (Some(128 + signal_hook::consts::SIGSEGV), None)
    );
}

/// Two children: one in the known list, one not. Only the second is ended and waited for, and at once.
#[cfg(target_os = "linux")]
fn fall_fremde_beenden() {
    // without the output of the helper: a child left behind by a failed case must not hold the read of the test
    let schlaf = || {
        Command::new("sleep")
            .arg("30")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("sleep")
    };
    let mut bekannt = schlaf();
    let fremd = schlaf();
    let fremd_pid = fremd.id();
    // the foreign child is waited for by its pidfd, not through the handle
    std::mem::forget(fremd);
    let beginn = Instant::now();
    fremde_beenden(&[bekannt.id()]);
    assert!(
        beginn.elapsed() < Duration::from_secs(3),
        "{:?}",
        beginn.elapsed()
    );
    let kinder = eigene_kinder();
    assert!(
        !kinder.contains(&fremd_pid),
        "ended and waited for: {kinder:?}"
    );
    assert!(kinder.contains(&bekannt.id()), "the known child stays");
    assert!(
        std::fs::metadata(format!("/proc/{fremd_pid}")).is_err(),
        "no zombie left"
    );
    let _ = bekannt.kill();
    let _ = bekannt.wait();
}

/// Without a foreign child the end returns at once, not at its limit.
#[cfg(target_os = "linux")]
fn fall_keine_fremden() {
    let beginn = Instant::now();
    fremde_beenden(&[]);
    assert!(
        beginn.elapsed() < Duration::from_secs(3),
        "{:?}",
        beginn.elapsed()
    );
}

#[cfg(target_os = "linux")]
#[test]
fn the_foreign_children_are_ended_and_the_known_ones_stay() {
    crate::testhilfe::helfer_gut(KIND_ENDE, "fremde", "3>&-");
    crate::testhilfe::helfer_gut(KIND_ENDE, "keine-fremden", "3>&-");
}
