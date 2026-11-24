//! Tests of the ready line and of the trampoline call, with `sh` in place of the children.

use super::*;

fn lesen_von(skript: &str, frist: Duration) -> Lesen {
    let mut k = Command::new("/bin/sh")
        .args(["-c", skript])
        .stdout(Stdio::piped())
        .stdin(Stdio::null())
        .spawn()
        .expect("sh");
    let mut aus = k.stdout.take().expect("stdout");
    let r = bereit_lesen(&mut aus, frist);
    let _ = k.kill();
    let _ = k.wait();
    r
}

fn name(l: &Lesen) -> &'static str {
    match l {
        Lesen::Bereit => "bereit",
        Lesen::Anders => "anders",
        Lesen::Ende => "ende",
    }
}

#[test]
fn only_exactly_the_ready_line_counts() {
    let f = Duration::from_secs(5);
    assert_eq!(
        name(&lesen_von("printf '{\"bereit\":true}\\n'; sleep 5", f)),
        "bereit"
    );
    // in two pieces
    assert_eq!(
        name(&lesen_von(
            "printf '{\"bereit\"'; sleep 0.2; printf ':true}\\n'; sleep 5",
            f
        )),
        "bereit"
    );
    // the first line decides; later output is not judged, in the same chunk or later
    for bereit in [
        "printf '{\"bereit\":true}\\nmehr\\n'; sleep 5",
        "printf '{\"bereit\":true}\\n'; sleep 0.2; printf 'mehr\\n'; sleep 5",
        "printf '{\"bereit\":true}\\n{\"bereit\":false}\\n'; sleep 5",
        "printf '{\"bere'; sleep 0.2; printf 'it\":tr'; sleep 0.2; printf 'ue}\\nmehr'; sleep 5",
    ] {
        assert_eq!(name(&lesen_von(bereit, f)), "bereit", "{bereit}");
    }
    for anders in [
        "printf 'mehr\\n{\"bereit\":true}\\n'; sleep 5",
        "printf '{\"bereit\":true}\\r\\n'; sleep 5",
        "printf '{\"bereit\": true}\\n'; sleep 5",
        "printf '{\"bereit\":false}\\n'; sleep 5",
        "printf 'x{\"bereit\":true}\\n'; sleep 5",
        "printf '{\"bereit\":true}'; exit 0",
        "printf '{\"bereit\":true}xxxxxxxxxxxxxxxxxx'; sleep 5",
    ] {
        assert_eq!(name(&lesen_von(anders, f)), "anders", "{anders}");
    }
    assert_eq!(name(&lesen_von("exit 3", f)), "ende");
    // a child that ends before the LF, after a part of the line
    assert_eq!(
        name(&lesen_von("printf '{\"bereit\":tr'; exit 0", f)),
        "anders"
    );
}

#[test]
fn the_length_limit_is_the_ready_line_itself() {
    // the time limit is far away, so "at once" means: long before it; the bound below is only a guard against a hang
    let f = Duration::from_secs(60);
    // exactly the ready line, the LF in a later piece: still ready
    assert_eq!(
        name(&lesen_von(
            "printf '{\"bereit\":true}'; sleep 0.2; printf '\\n'; sleep 5",
            f
        )),
        "bereit"
    );
    // no LF within the length of the ready line: ends at once, not at the time limit
    let beginn = Instant::now();
    assert_eq!(
        name(&lesen_von("printf '{\"bereit\":true}x'; sleep 5", f)),
        "anders"
    );
    assert!(
        beginn.elapsed() < Duration::from_secs(30),
        "{:?}",
        beginn.elapsed()
    );
    let beginn = Instant::now();
    assert_eq!(
        name(&lesen_von(
            "printf '{\"bereit\":true}xxxxxxxxxxxxxxxxxx'; sleep 5",
            f
        )),
        "anders"
    );
    assert!(
        beginn.elapsed() < Duration::from_secs(30),
        "{:?}",
        beginn.elapsed()
    );
}

#[test]
fn silence_ends_at_the_time_limit() {
    let beginn = Instant::now();
    assert_eq!(
        name(&lesen_von("sleep 5", Duration::from_millis(300))),
        "anders"
    );
    let dauer = beginn.elapsed();
    assert!(
        dauer >= Duration::from_millis(300) && dauer < Duration::from_secs(30),
        "{dauer:?}"
    );
}

#[test]
fn the_trampoline_refuses_a_wrong_call() {
    for args in [
        vec![],
        vec!["x".to_owned(), "--".to_owned(), "/bin/true".to_owned()],
        vec!["1".to_owned(), "-".to_owned(), "/bin/true".to_owned()],
        vec!["1".to_owned(), "--".to_owned()],
        vec!["1".to_owned(), HALTEN.to_owned()],
        vec!["1".to_owned(), "--halte".to_owned(), "/bin/true".to_owned()],
    ] {
        assert_eq!(trampolin(&args), KIND_AUFRUF_FEHLER, "{args:?}");
    }
    // a parent that is not the parent: refused before the program runs, also for the starter
    let fremd = (std::process::id() + 1).to_string();
    for art in ["--", HALTEN] {
        assert_eq!(
            trampolin(&[fremd.clone(), art.to_owned(), "/bin/true".to_owned()]),
            KIND_AUFRUF_FEHLER,
            "{art}"
        );
    }
}

#[test]
fn a_missing_program_is_a_failed_start() {
    let r = starten(
        Path::new("/nicht/da/werkbank-lauf-kind"),
        Path::new("/bin/true"),
        &[],
        Duration::from_secs(1),
    );
    assert_eq!(r.map(|k| k.pid()), Err(Ausfall::Fehlt));
}

#[test]
fn the_process_is_single_threaded_until_a_thread_starts() {
    if !cfg!(target_os = "linux") {
        return; // no /proc: the count cannot be read here
    }
    assert!(faeden().is_some_and(|n| n >= 1), "Linux reads the count");
    // the test harness itself runs tests in threads; the check sees more than one
    let (tx, rx) = std::sync::mpsc::channel::<()>();
    let f = std::thread::spawn(move || {
        let _ = rx.recv();
    });
    assert!(!einthreadig());
    drop(tx);
    let _ = f.join();
}
