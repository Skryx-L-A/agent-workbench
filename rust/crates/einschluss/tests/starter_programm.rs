//! The program `werkbank-einschluss` against a real Linux host with a real `bwrap` and the stand-in harness (feature
//! `attrappe`): the two calls, the refusal of every other call, one case for every refused input and for every check of
//! the crate, the environment, the fixed `bwrap` path, and that descriptor 3 does not reach the harness.
//!
//! Descriptor 3 is put in place by the shell (`3<&0`, `3<file`): safe Rust cannot map a descriptor onto a number.
#![cfg(all(feature = "attrappe", target_os = "linux"))]

mod gemeinsam;

use std::fs;
use std::io::{Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use einschluss::{Cloud, Verteilung, beschreibung_schreiben};
use gemeinsam::Welt;

const STARTER: &str = env!("CARGO_BIN_EXE_werkbank-einschluss");
const FEST: &str = "lauf.einschluss_fehlt: Einschluss nicht verfuegbar\n";
/// The second line of a refusal with `FEST`: `lauf.einschluss_teil: <exit code> <word>`, the word of lower case letters
/// and hyphens only (the check that refused; never a path).
fn mit_teil(code: i32, wort: &str) -> String {
    format!("{FEST}lauf.einschluss_teil: {code} {wort}\n")
}

/// Whether `fehler` is `FEST` and the second line of the exit `code`, with any fixed word.
fn fest_mit_teil(fehler: &str, code: i32) -> bool {
    let Some(rest) = fehler.strip_prefix(FEST) else {
        return false;
    };
    let Some(wort) = rest
        .strip_prefix(&format!("lauf.einschluss_teil: {code} "))
        .and_then(|w| w.strip_suffix('\n'))
    else {
        return false;
    };
    !wort.is_empty() && wort.bytes().all(|b| b.is_ascii_lowercase() || b == b'-')
}

const FRIST: Duration = Duration::from_secs(60);

/// What stands on descriptor 3.
enum Fd3<'a> {
    /// Nothing: the descriptor is not open.
    Keine,
    /// A pipe that carries this text and ends.
    Text(&'a str),
    /// A pipe that carries these bytes and ends.
    Bytes(&'a [u8]),
    /// A pipe that carries these bytes and stays open for 8 seconds (the starter must give up at its limit).
    Offen(&'a [u8]),
    /// A regular file with this content (refused: the starter takes a pipe only).
    Datei(&'a str),
}

/// Runs the starter with `args`, the environment `env` (nothing else) and descriptor 3 as given. The shell puts the
/// descriptor in place (`3<&0` from a pipe on standard input, or `3<file`): safe Rust cannot map a descriptor onto a
/// number. Descriptor 5 is the lifeline (a named pipe this process holds open as a writer). Every wait has a limit.
fn starter(
    welt: &Welt,
    args: &[&str],
    fd3: &Fd3,
    env: &[(&str, &str)],
) -> (Option<i32>, String, String) {
    // this process is the parent of the starter: it holds the lifeline until the starter has ended
    let leine = gemeinsam::leine(&welt.wurzel);
    let _halter = gemeinsam::fifo_offen(&leine);
    let mut c = match fd3 {
        Fd3::Keine => {
            // descriptor 3 is closed explicitly: the test runner may have handed a descriptor of its own down
            let mut c = Command::new("/bin/sh");
            c.args(["-c", "l=$1; shift; exec \"$@\" 3<&- 5<\"$l\"", "sh"])
                .arg(&leine)
                .arg(STARTER)
                .args(args);
            c
        }
        Fd3::Text(_) | Fd3::Bytes(_) | Fd3::Offen(_) => {
            let mut c = Command::new("/bin/sh");
            c.args(["-c", "l=$1; shift; exec \"$@\" 3<&0 5<\"$l\"", "sh"])
                .arg(&leine)
                .arg(STARTER)
                .args(args);
            c
        }
        Fd3::Datei(text) => {
            let datei = welt.wurzel.join("eingabe.json");
            fs::write(&datei, text).expect("input file");
            let mut c = Command::new("/bin/sh");
            c.args([
                "-c",
                "f=$1; l=$2; shift 2; exec \"$@\" 3<\"$f\" 5<\"$l\"",
                "sh",
            ])
            .arg(&datei)
            .arg(&leine)
            .arg(STARTER)
            .args(args);
            c
        }
    };
    c.env_clear()
        .envs(env.iter().copied())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut kind = c.spawn().expect("starts");
    let mut eingang = kind.stdin.take().expect("stdin");
    let bytes: Vec<u8> = match fd3 {
        Fd3::Text(t) => t.as_bytes().to_vec(),
        Fd3::Bytes(b) | Fd3::Offen(b) => b.to_vec(),
        Fd3::Keine | Fd3::Datei(_) => Vec::new(),
    };
    // the writer ends with the pipe; it may fail when the starter stops reading at the limit
    let offen = matches!(fd3, Fd3::Offen(_));
    let schreiber = std::thread::spawn(move || {
        let _ = eingang.write_all(&bytes);
        if offen {
            std::thread::sleep(Duration::from_secs(8));
        }
    });
    let ende = Instant::now() + FRIST;
    let status = loop {
        if let Some(s) = kind.try_wait().expect("wait") {
            break s.code();
        }
        if Instant::now() > ende {
            let _ = kind.kill();
            let _ = kind.wait();
            break None;
        }
        std::thread::sleep(Duration::from_millis(5));
    };
    if !offen {
        schreiber.join().expect("writer");
    }
    let (mut aus, mut fehler) = (String::new(), String::new());
    if let Some(mut s) = kind.stdout.take() {
        let _ = s.read_to_string(&mut aus);
    }
    if let Some(mut s) = kind.stderr.take() {
        let _ = s.read_to_string(&mut fehler);
    }
    (status, aus, fehler)
}

const START: [&str; 3] = ["start", "--beschreibung-fd", "3"];

/// The description of the world with the probe of the stand-in as the program.
fn text_mit(welt: &Welt, probe: &[&str]) -> String {
    let mut b = welt.beschreibung();
    b.argv = vec![welt.attrappe_gast(), "harness".to_owned()];
    b.argv.extend(probe.iter().map(|p| (*p).to_owned()));
    beschreibung_schreiben(&b)
}

/// A refused start: no output on standard output, exactly the fixed line on standard error, the given exit code, and
/// no path of the throw-away world in the line.
fn abgelehnt(welt: &Welt, text: &str, code: i32, was: &str) {
    let (status, aus, fehler) = starter(welt, &START, &Fd3::Text(text), &[]);
    assert_eq!(status, Some(code), "{was}: stderr {fehler:?}");
    assert_eq!(aus, "", "{was}");
    assert!(
        fest_mit_teil(&fehler, code)
            || fehler == "lauf.ordner_fehlt: Arbeitsordner fehlt oder ist kein Ordner\n",
        "{was}: {fehler:?}"
    );
    assert!(!fehler.contains(&*welt.wurzel.to_string_lossy()), "{was}");
}

#[test]
fn selbsttest_runs_the_fixed_enclosure() {
    let welt = Welt::neu();
    let (status, aus, fehler) = starter(&welt, &["selbsttest"], &Fd3::Keine, &[]);
    assert_eq!((status, aus.as_str(), fehler.as_str()), (Some(0), "", ""));
}

#[test]
fn every_foreign_call_ends_with_2_and_the_fixed_line() {
    let welt = Welt::neu();
    for args in [
        &[][..],
        &["roh"][..],
        &["roh", "--bind", "/", "/"][..],
        &["start"][..],
        &["start", "--bind", "/", "/"][..],
        &["start", "--beschreibung-fd", "4"][..],
        &["start", "--beschreibung-fd", "3", "extra"][..],
        &["selbsttest", "extra"][..],
        &["--help"][..],
    ] {
        let (status, aus, fehler) = starter(&welt, args, &Fd3::Text("{}"), &[]);
        assert_eq!(status, Some(2), "{args:?}");
        assert_eq!(aus, "", "{args:?}");
        assert_eq!(
            fehler, "werkbank-einschluss: Aufruf ungueltig\n",
            "{args:?}"
        );
    }
}

#[test]
fn start_runs_the_harness_inside_the_enclosure() {
    let welt = Welt::neu();
    let (status, aus, fehler) = starter(
        &welt,
        &START,
        &Fd3::Text(&text_mit(&welt, &["kennung"])),
        &[],
    );
    assert_eq!(status, Some(0), "stderr {fehler:?}");
    assert!(
        aus.starts_with("kennung: OK einschluss-attrappe-marker-"),
        "{aus}"
    );
    assert_eq!(fehler, "");
}

#[test]
fn the_harness_gets_only_its_variables_whatever_the_caller_sets() {
    let welt = Welt::neu();
    let (status, aus, _) = starter(
        &welt,
        &START,
        &Fd3::Text(&text_mit(&welt, &["umgebung"])),
        &[
            ("LD_PRELOAD", "/x.so"),
            ("DBUS_SESSION_BUS_ADDRESS", "unix:path=/run/user/1/bus"),
            ("HTTPS_PROXY", "http://127.0.0.1:1"),
            ("BWRAP", "/tmp/x"),
            ("WERKBANK_BESCHREIBUNG", "{}"),
        ],
    );
    assert_eq!(status, Some(0));
    assert_eq!(aus.trim(), "umgebung: OK HOME,LANG,PATH,PWD,TERM,TMPDIR");
}

#[test]
fn descriptor_3_holds_nothing_in_the_harness() {
    // `bwrap` passes descriptor 3 on (measured): the starter takes a pipe only, and a pipe read to its end holds no text
    let welt = Welt::neu();
    let mut b = welt.beschreibung();
    b.programmordner.clear();
    b.argv = vec!["/usr/bin/cat".to_owned(), "/proc/self/fd/3".to_owned()];
    let (status, aus, fehler) =
        starter(&welt, &START, &Fd3::Text(&beschreibung_schreiben(&b)), &[]);
    assert_eq!((status, aus.as_str(), fehler.as_str()), (Some(0), "", ""));
}

/// Runs the starter with `start` and `text` on descriptor 3, and with the lifeline set up by the shell text `leine`
/// (its argument `$1` is the named pipe of the lifeline; this process holds no writer of it). Standard output and the
/// exit code.
fn start_mit_leine(welt: &Welt, text: &str, leine: &str) -> (Option<i32>, String) {
    let pfad = gemeinsam::leine(&welt.wurzel);
    let mut kind = Command::new("/bin/sh")
        .args([
            "-c",
            &format!("l=$1; shift; {leine}; exec \"$@\" 3<&0"),
            "sh",
        ])
        .arg(&pfad)
        .arg(STARTER)
        .args(START)
        .env_clear()
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("starts");
    let mut eingang = kind.stdin.take().expect("stdin");
    eingang.write_all(text.as_bytes()).expect("input");
    drop(eingang);
    let ende = Instant::now() + FRIST;
    let status = loop {
        if let Some(s) = kind.try_wait().expect("wait") {
            break s.code();
        }
        if Instant::now() > ende {
            let _ = kind.kill();
            let _ = kind.wait();
            break None;
        }
        std::thread::sleep(Duration::from_millis(5));
    };
    let mut aus = String::new();
    if let Some(mut s) = kind.stdout.take() {
        let _ = s.read_to_string(&mut aus);
    }
    (status, aus)
}

#[test]
fn the_harness_starts_only_while_the_lifeline_lives_and_never_holds_it() {
    let welt = Welt::neu();
    let mut b = welt.beschreibung();
    b.programmordner.clear();
    b.argv = vec![
        "/usr/bin/sh".to_owned(),
        "-c".to_owned(),
        "if [ -e /proc/self/fd/5 ]; then echo offen; else echo zu; fi".to_owned(),
    ];
    let text = beschreibung_schreiben(&b);
    // a living lifeline: the harness runs, and descriptor 5 is closed in it
    let (status, aus, fehler) = starter(&welt, &START, &Fd3::Text(&text), &[]);
    assert_eq!(
        (status, aus.as_str(), fehler.as_str()),
        (Some(0), "zu\n", "")
    );
    let abbruch = Some(einschluss::ANKER_ABBRUCH);
    // no descriptor 5: the harness never starts
    assert_eq!(
        start_mit_leine(&welt, &text, "exec 5<&-"),
        (abbruch, String::new())
    );
    // descriptor 5 is no pipe
    assert_eq!(
        start_mit_leine(&welt, &text, "exec 5</dev/null"),
        (abbruch, String::new())
    );
    // a lifeline without a writer (the starter is gone): end of file, the harness never starts
    assert_eq!(
        start_mit_leine(&welt, &text, "exec 6<>\"$l\"; exec 5<\"$l\"; exec 6>&-"),
        (abbruch, String::new())
    );
    // the counter probe of the last case: the same lifeline with the writer of the shell, which is the starter itself
    // after its `exec`
    assert_eq!(
        start_mit_leine(&welt, &text, "exec 6<>\"$l\"; exec 5<\"$l\""),
        (Some(0), "zu\n".to_owned())
    );
}

#[test]
fn bwrap_is_taken_from_the_fixed_path_never_from_a_search_path() {
    let welt = Welt::neu();
    let falsch = welt.wurzel.join("falsch");
    fs::create_dir_all(&falsch).expect("folder");
    let marke = welt.wurzel.join("falsch-gerufen");
    let skript = falsch.join("bwrap");
    fs::write(
        &skript,
        format!("#!/bin/sh\necho x > {}\nexit 0\n", marke.display()),
    )
    .expect("fake");
    fs::set_permissions(&skript, fs::Permissions::from_mode(0o755)).expect("mode");
    let pfad = falsch.to_string_lossy().into_owned();
    let env = [
        ("PATH", pfad.as_str()),
        ("BWRAP", skript.to_str().expect("text")),
    ];
    let (status, _, fehler) = starter(&welt, &["selbsttest"], &Fd3::Keine, &env);
    assert_eq!((status, fehler.as_str()), (Some(0), ""));
    let (status, aus, _) = starter(
        &welt,
        &START,
        &Fd3::Text(&text_mit(&welt, &["kennung"])),
        &env,
    );
    assert_eq!(status, Some(0));
    assert!(aus.starts_with("kennung: OK"), "{aus}");
    assert!(!marke.exists(), "the fake bwrap of the search path was run");
}

#[test]
fn the_input_cases_are_each_refused_with_3() {
    let welt = Welt::neu();
    let gut = text_mit(&welt, &["kennung"]);
    // the descriptor is not open
    let (status, aus, fehler) = starter(&welt, &START, &Fd3::Keine, &[]);
    assert_eq!(
        (status, aus.as_str(), fehler.as_str()),
        (Some(3), "", mit_teil(3, "eingabe-nicht-lesbar").as_str())
    );
    for (was, text) in [
        ("empty", String::new()),
        ("no JSON", "kein json".to_owned()),
        ("JSON of another type", "[]".to_owned()),
        ("unknown key", gut.replacen('{', "{\"extra\":1,", 1)),
        ("system as a key", gut.replacen('{', "{\"system\":{},", 1)),
        ("missing field", gut.replacen("\"harness\":", "\"zzz\":", 1)),
        ("wrong type", gut.replacen("\"heim\":\"", "\"heim\":[\"", 1)),
        ("text after the value", format!("{gut} x")),
        ("too big", format!("{gut}{}", " ".repeat(65_536))),
    ] {
        abgelehnt(&welt, &text, 3, was);
    }
}

#[test]
fn the_text_at_the_limit_is_read_and_one_byte_more_is_not() {
    let welt = Welt::neu();
    let gut = text_mit(&welt, &["kennung"]);
    let fuell = |n: usize| format!("{gut}{}", " ".repeat(n - gut.len()));
    let (status, aus, _) = starter(&welt, &START, &Fd3::Text(&fuell(65_536)), &[]);
    assert_eq!(status, Some(0));
    assert!(aus.starts_with("kennung: OK"), "{aus}");
    abgelehnt(&welt, &fuell(65_537), 3, "one byte over");
}

#[test]
fn a_file_or_a_longer_pipe_on_descriptor_3_is_refused() {
    let welt = Welt::neu();
    let gut = text_mit(&welt, &["kennung"]);
    // a regular file would stay open in the harness with its content: refused, though the text is valid
    let (status, aus, fehler) = starter(&welt, &START, &Fd3::Datei(&gut), &[]);
    assert_eq!(
        (status, aus.as_str(), fehler.as_str()),
        (Some(3), "", mit_teil(3, "eingabe-kein-rohr").as_str())
    );
    // a pipe that never ends within the limit: refused at the limit, the starter does not read it all
    let (status, _, fehler) = starter(&welt, &START, &Fd3::Bytes(&vec![b' '; 300_000]), &[]);
    assert_eq!(
        (status, fehler.as_str()),
        (Some(3), mit_teil(3, "eingabe-zu-gross").as_str())
    );
}

#[test]
fn a_pipe_that_never_ends_is_given_up_after_the_time_limit() {
    let welt = Welt::neu();
    let gut = text_mit(&welt, &["kennung"]);
    let start = Instant::now();
    let (status, aus, fehler) = starter(&welt, &START, &Fd3::Offen(gut.as_bytes()), &[]);
    assert_eq!(
        (status, aus.as_str(), fehler.as_str()),
        (Some(3), "", mit_teil(3, "eingabe-nicht-lesbar").as_str())
    );
    assert!(
        start.elapsed() < Duration::from_secs(8),
        "waited for the writer"
    );
}

#[test]
#[allow(clippy::too_many_lines)] // one world, one case per check
fn every_check_of_the_crate_is_run_by_the_starter() {
    let welt = Welt::neu();
    let b = welt.beschreibung();
    let heim = welt.heim.to_string_lossy().into_owned();
    let mit = |f: &dyn Fn(&mut einschluss::Beschreibung)| {
        let mut c = b.clone();
        f(&mut c);
        beschreibung_schreiben(&c)
    };
    // form: relative program, a path with `..` or `//`, an empty program list
    for programm in ["h", "../h", "/usr/bin/../bin/true", "/usr//bin/true"] {
        abgelehnt(
            &welt,
            &mit(&|c| c.argv = vec![programm.to_owned()]),
            5,
            programm,
        );
    }
    abgelehnt(&welt, &mit(&|c| c.argv.clear()), 5, "no program");
    let w = welt.arbeit.to_string_lossy().into_owned();
    abgelehnt(
        &welt,
        &mit(&|c| c.arbeitsordner = format!("{w}/../arbeit").into()),
        5,
        "..",
    );
    abgelehnt(
        &welt,
        &mit(&|c| c.arbeitsordner = format!("{w}//x").into()),
        5,
        "//",
    );
    abgelehnt(
        &welt,
        &mit(&|c| c.arbeitsordner = "arbeit".into()),
        5,
        "relative folder",
    );
    // deny lists: a system path, the home, a secret segment, a login store
    abgelehnt(
        &welt,
        &mit(&|c| c.arbeitsordner = "/usr/lib".into()),
        5,
        "system path",
    );
    abgelehnt(
        &welt,
        &mit(&|c| c.arbeitsordner = c.heim.clone()),
        5,
        "home itself",
    );
    abgelehnt(
        &welt,
        &mit(&|c| c.arbeitsordner = format!("{heim}/.ssh").into()),
        5,
        "login store",
    );
    abgelehnt(
        &welt,
        &mit(&|c| c.programmordner = vec![format!("{heim}/.gnupg").into()]),
        5,
        "gnupg",
    );
    abgelehnt(
        &welt,
        &mit(&|c| c.arbeitsordner = format!("{w}/90-secrets").into()),
        5,
        "secret segment",
    );
    // overlaps: the working folder in a protected folder, the state folder in the working folder
    let vault = welt.vault.to_string_lossy().into_owned();
    abgelehnt(
        &welt,
        &mit(&|c| c.arbeitsordner = format!("{vault}/p").into()),
        5,
        "protected folder",
    );
    abgelehnt(
        &welt,
        &mit(&|c| c.zustand.host = format!("{w}/z").into()),
        5,
        "state in work",
    );
    // the program and the bundle of the cloud exit lie outside the writable folders
    let cloud = |programm: String, bundle: String| Cloud {
        weiterreicher: welt.such.socket.clone(),
        weiter_programm: programm.into(),
        bundle: bundle.into(),
        verteilung: Verteilung::Ubuntu,
    };
    let aussen = welt
        .programm
        .join("einschluss-attrappe")
        .to_string_lossy()
        .into_owned();
    abgelehnt(
        &welt,
        &mit(&|c| {
            c.harness = "copilot".to_owned();
            c.cloud = Some(cloud(format!("{w}/weiter"), aussen.clone()));
        }),
        5,
        "forwarder program in the working folder",
    );
    abgelehnt(
        &welt,
        &mit(&|c| {
            c.harness = "copilot".to_owned();
            c.cloud = Some(cloud(aussen.clone(), format!("{w}/bundle.pem")));
        }),
        5,
        "bundle in the working folder",
    );
    // the checks of the host: the working folder is missing; the state folder is not empty; a socket is no socket
    let (status, aus, fehler) = starter(
        &welt,
        &START,
        &Fd3::Text(&mit(&|c| {
            c.arbeitsordner = format!("{w}/gibt-es-nicht").into();
        })),
        &[],
    );
    assert_eq!(
        (status, aus.as_str(), fehler.as_str()),
        (
            Some(4),
            "",
            "lauf.ordner_fehlt: Arbeitsordner fehlt oder ist kein Ordner\n"
        )
    );
    fs::write(welt.zustand.join("rest"), "x").expect("file");
    abgelehnt(
        &welt,
        &text_mit(&welt, &["kennung"]),
        5,
        "state folder not empty",
    );
    fs::remove_file(welt.zustand.join("rest")).expect("remove");
    let datei = welt.wurzel.join("keine-buchse");
    fs::write(&datei, "x").expect("file");
    abgelehnt(
        &welt,
        &mit(&|c| c.such_socket = datei.clone()),
        5,
        "socket is a file",
    );
    // control: the unchanged description runs
    let (status, _, _) = starter(
        &welt,
        &START,
        &Fd3::Text(&text_mit(&welt, &["kennung"])),
        &[],
    );
    assert_eq!(status, Some(0));
}

#[test]
fn selbsttest_fails_where_the_enclosure_cannot_be_built() {
    // inside an enclosure that forbids further user namespaces the inner `bwrap` cannot start: the self test must say so
    let welt = Welt::neu();
    let mut c = Command::new("/usr/bin/bwrap");
    c.args([
        "--unshare-user",
        "--disable-userns",
        "--ro-bind",
        "/usr",
        "/usr",
        "--symlink",
        "usr/lib",
        "/lib",
        "--symlink",
        "usr/lib64",
        "/lib64",
        "--symlink",
        "usr/bin",
        "/bin",
        "--proc",
        "/proc",
        "--dev",
        "/dev",
        "--ro-bind",
        STARTER,
        "/starter",
        "/starter",
        "selbsttest",
    ])
    .env_clear()
    .stdin(Stdio::null())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped());
    let aus = c.output().expect("bwrap runs");
    let _ = &welt;
    let fehler = String::from_utf8_lossy(&aus.stderr).into_owned();
    let code = aus.status.code().unwrap_or(-1);
    assert!(fest_mit_teil(&fehler, code), "{:?}: {fehler:?}", aus.status);
    // 7 where the message names the namespace (Ubuntu), 8 for another failed test run (measured: 8 on Arch)
    assert!(matches!(aus.status.code(), Some(7 | 8)), "{:?}", aus.status);
}

const VORENTPACKEN: [&str; 3] = ["vorentpacken", "--beschreibung-fd", "3"];

/// The small enclosure of the world: the cache folder of the harness in the home, a script as the program that writes
/// into the cache place inside and into the rest of the home, and reports `--version`.
fn vorentpacken_welt(welt: &Welt) -> (einschluss::Vorentpacken, std::path::PathBuf) {
    let cache = welt.heim.join(".cache/werkbank/harness/copilot");
    fs::create_dir_all(&cache).expect("cache");
    fs::set_permissions(&cache, fs::Permissions::from_mode(0o700)).expect("mode");
    let skript = welt.programm.join("copilot-probe");
    fs::write(
        &skript,
        "#!/bin/sh\n[ \"$*\" = --version ] || exit 9\necho inhalt > \"$HOME/.cache/copilot/geschrieben\" || exit 7\necho rest > \"$HOME/rest\" || exit 8\nexit 0\n",
    )
    .expect("script");
    fs::set_permissions(&skript, fs::Permissions::from_mode(0o755)).expect("mode");
    let v = einschluss::Vorentpacken {
        heim: welt.heim.clone(),
        cache: cache.clone(),
        programmordner: vec![welt.programm.clone()],
        programm: skript,
        path: "/usr/bin:/bin".to_owned(),
        system: einschluss::System::vom_host().expect("host"),
    };
    (v, cache)
}

#[test]
fn vorentpacken_runs_the_program_with_version_and_writes_only_the_cache_folder() {
    let welt = Welt::neu();
    let (v, cache) = vorentpacken_welt(&welt);
    let text = einschluss::vorentpacken_schreiben(&v);
    let (status, aus, fehler) = starter(&welt, &VORENTPACKEN, &Fd3::Text(&text), &[]);
    assert_eq!(status, Some(0), "stderr {fehler:?}");
    assert_eq!((aus.as_str(), fehler.as_str()), ("", ""));
    assert_eq!(
        fs::read_to_string(cache.join("geschrieben"))
            .ok()
            .as_deref(),
        Some("inhalt\n")
    );
    assert!(
        !welt.heim.join("rest").exists(),
        "the rest of the home is a tmpfs"
    );
}

#[test]
fn vorentpacken_refuses_a_wrong_input_and_a_wrong_cache_folder() {
    let welt = Welt::neu();
    let (v, cache) = vorentpacken_welt(&welt);
    let text = einschluss::vorentpacken_schreiben(&v);
    // an unknown key: refused as input
    let mehr = text.replacen('{', "{\"argv\":[\"/bin/sh\"],", 1);
    let (status, _, fehler) = starter(&welt, &VORENTPACKEN, &Fd3::Text(&mehr), &[]);
    assert_eq!(
        (status, fehler.as_str()),
        (Some(3), mit_teil(3, "eingabe-ungueltig").as_str())
    );
    // a cache folder outside the home
    let mut aussen = v.clone();
    aussen.cache = welt.wurzel.join("werkbank/harness/copilot");
    let (status, _, fehler) = starter(
        &welt,
        &VORENTPACKEN,
        &Fd3::Text(&einschluss::vorentpacken_schreiben(&aussen)),
        &[],
    );
    assert_eq!(
        (status, fehler.as_str()),
        (Some(5), mit_teil(5, "verboten").as_str())
    );
    // the cache folder is a link
    let echt = welt.heim.join("echt");
    fs::rename(&cache, &echt).expect("move");
    std::os::unix::fs::symlink(&echt, &cache).expect("link");
    let (status, _, fehler) = starter(&welt, &VORENTPACKEN, &Fd3::Text(&text), &[]);
    assert_eq!(
        (status, fehler.as_str()),
        (Some(5), mit_teil(5, "quelle-unbrauchbar").as_str())
    );
    assert!(!echt.join("geschrieben").exists());
    // a file on descriptor 3
    let (status, _, fehler) = starter(&welt, &VORENTPACKEN, &Fd3::Datei(&text), &[]);
    assert_eq!(
        (status, fehler.as_str()),
        (Some(3), mit_teil(3, "eingabe-kein-rohr").as_str())
    );
    for args in [
        &["vorentpacken"][..],
        &["vorentpacken", "--beschreibung-fd", "4"][..],
        &["vorentpacken", "--beschreibung-fd", "3", "extra"][..],
    ] {
        let (status, _, fehler) = starter(&welt, args, &Fd3::Text(&text), &[]);
        assert_eq!(
            (status, fehler.as_str()),
            (Some(2), "werkbank-einschluss: Aufruf ungueltig\n"),
            "{args:?}"
        );
    }
}
