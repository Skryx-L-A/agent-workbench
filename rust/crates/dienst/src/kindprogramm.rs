//! DI8: the fixed child programs of the tests (`wb-dienst --kind <name> [arguments]`). They are part of the program, so that
//! no other interpreter is needed. Each one puts the terminal in raw mode without echo, writes `BEREIT\r\n`, reads one start
//! byte and then acts; so no output runs before the viewer is there and no sequence depends on timing.
//!
//! The standard library does the work. Two things need the system: the raw mode and the window size go through `stty` (as in
//! `tr --kind` of the carrier), found by an absolute path because the environment of the child is exactly what the start file
//! says and may have no `PATH`; a signal to itself and the ignoring of `SIGHUP` go through `rustix` and `signal-hook`.

use std::fmt::Write as _;
use std::io::{Read, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use crate::json::unhex;

/// The hex text of bytes, lower case.
pub(crate) fn hex(bytes: &[u8]) -> String {
    bytes.iter().fold(String::new(), |mut aus, b| {
        let _ = write!(aus, "{b:02x}");
        aus
    })
}

/// Standard Base64 with padding.
pub(crate) fn base64(bytes: &[u8]) -> String {
    const TAFEL: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut aus = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for stueck in bytes.chunks(3) {
        let b = [
            stueck[0],
            *stueck.get(1).unwrap_or(&0),
            *stueck.get(2).unwrap_or(&0),
        ];
        let n = (u32::from(b[0]) << 16) + (u32::from(b[1]) << 8) + u32::from(b[2]);
        let z = |schub: u32| char::from(TAFEL[(n >> schub & 63) as usize]);
        aus.push(z(18));
        aus.push(z(12));
        aus.push(if stueck.len() > 1 { z(6) } else { '=' });
        aus.push(if stueck.len() > 2 { z(0) } else { '=' });
    }
    aus
}

/// `stty` with the arguments; its standard input is the terminal of the child. Returns the standard output.
#[cfg(unix)]
fn stty(argumente: &[&str]) -> String {
    for pfad in ["/bin/stty", "/usr/bin/stty"] {
        if Path::new(pfad).exists() {
            return Command::new(pfad)
                .args(argumente)
                .stdin(Stdio::inherit())
                .stderr(Stdio::null())
                .output()
                .map(|a| String::from_utf8_lossy(&a.stdout).trim().to_owned())
                .unwrap_or_default();
        }
    }
    String::new()
}

// Windows: [`crate::sys::stty`] (the two uses of `stty` through the console: `raw -echo` and `size`, contract section 21).
#[cfg(windows)]
use crate::sys::stty;

/// A path as the report writes it. Unix: as it is. Windows (section 21): in the form of the start file, without the drive and with
/// `/` (`C:\\t\\o` is `/t/o`), the way the service reads a path of the start file on the drive of the working folder.
fn pfad_text(p: &Path) -> String {
    #[cfg(windows)]
    {
        crate::sys::pfad_text(p)
    }
    #[cfg(unix)]
    {
        p.display().to_string()
    }
}

fn schreibe(bytes: &[u8]) {
    let mut aus = std::io::stdout().lock();
    let _ = aus.write_all(bytes);
    let _ = aus.flush();
}

/// Reads up to `n` bytes (fewer at the end of the input).
fn lies(n: usize) -> Vec<u8> {
    let mut eingang = std::io::stdin().lock();
    let mut daten = vec![0_u8; n];
    let mut da = 0;
    while da < n {
        match eingang.read(&mut daten[da..]) {
            Ok(0) | Err(_) => break,
            Ok(k) => da += k,
        }
    }
    daten.truncate(da);
    daten
}

fn zahl(argumente: &[String], i: usize) -> usize {
    argumente
        .get(i)
        .and_then(|a| a.parse::<usize>().ok())
        .unwrap_or(0)
}

fn text(argumente: &[String], i: usize) -> &str {
    argumente.get(i).map_or("", String::as_str)
}

/// Waits `sekunden` (a decimal text); a value that is no number, negative or too large waits 0.
fn schlafe(sekunden: &str) {
    let frist = sekunden
        .parse::<f64>()
        .ok()
        .and_then(|s| Duration::try_from_secs_f64(s).ok())
        .unwrap_or_default();
    std::thread::sleep(frist);
}

/// The names of the fixed child programs that read a start byte (`still` and the unknown names are handled before).
const NAMEN: [&str; 17] = [
    "ausgabe",
    "geteilt",
    "ausgabewarte",
    "ende",
    "zeilen",
    "klammer",
    "eingabe",
    "bilder",
    "groesse",
    "umgebung",
    "signal",
    "titel",
    "warte",
    "stufen",
    "fuellen",
    "osc52viele",
    "lauf",
];

/// Runs the child program `args[0]` with the arguments after it and returns its exit code. An unknown name ends with 64.
#[must_use]
pub fn lauf(args: &[String]) -> i32 {
    let name = text(args, 0);
    let a: &[String] = args.get(1..).unwrap_or(&[]);
    if name == "still" {
        // writes nothing and reads nothing: the start cursor before any output (DI11)
        schlafe(text(a, 0));
        return 0;
    }
    if name == "zeuge" {
        // the witness of the cases of DI12 point 12: no terminal, no `BEREIT`
        return zeuge(a);
    }
    if !NAMEN.contains(&name) {
        return 64;
    }
    stty(&["raw", "-echo"]);
    schreibe(b"BEREIT\r\n");
    lies(1);
    handeln(name, a)
}

/// What the child does after `BEREIT` and the start byte.
fn handeln(name: &str, a: &[String]) -> i32 {
    match name {
        "stufen" => stufen(a),
        "fuellen" => fuellen(a),
        "osc52viele" => osc52viele(a),
        "ausgabe" => schreibe(&unhex(text(a, 0))),
        "geteilt" => {
            schreibe(&unhex(text(a, 0)));
            std::thread::sleep(Duration::from_millis(3));
            schreibe(&unhex(text(a, 1)));
            if let Some(sekunden) = a.get(2) {
                schlafe(sekunden);
            }
        }
        "ausgabewarte" => {
            schreibe(&unhex(text(a, 0)));
            schlafe(text(a, 1));
        }
        "ende" => return i32::try_from(zahl(a, 0) & 0xff).unwrap_or(0),
        "zeilen" => {
            for i in 1..=zahl(a, 0) {
                schreibe(format!("Z{i:06}\r\n").as_bytes());
            }
        }
        "klammer" => {
            schreibe(b"\x1b[?2004hKLAMMER\r\n");
            let d = lies(zahl(a, 0));
            schreibe(format!("EINGABE {}\r\n", hex(&d)).as_bytes());
        }
        "eingabe" => {
            let d = lies(zahl(a, 0));
            schreibe(format!("EINGABE {}\r\n", hex(&d)).as_bytes());
        }
        "bilder" => bilder(a),
        "groesse" => schreibe(format!("GROESSE {}\r\n", stty(&["size"])).as_bytes()),
        "umgebung" => umgebung(a),
        "signal" => signal_an_sich(a),
        "titel" => schreibe(format!("\x1b]2;{}\x07fertig\r\n", text(a, 0)).as_bytes()),
        "warte" => warte(a),
        "lauf" => return lauf_kind(a),
        _ => {}
    }
    0
}

/// The arguments without the pair `--meldung <pfad>` that follows the program (section 18).
fn ohne_meldung(a: &[String]) -> Vec<String> {
    if a.get(1).map(String::as_str) == Some("--meldung") && a.len() >= 3 {
        a[..1].iter().chain(&a[3..]).cloned().collect()
    } else {
        a.to_vec()
    }
}

/// `lauf <argv des Kindes ...>` (DI12 point 9): the fixed child of the sessions that `Starten` starts; `argv` is `[program, start.json]`.
/// What it does after `BEREIT` and the start byte is chosen by the plan in the environment (`KIND_PLAN`, a JSON object from the
/// identifier in the `start.json` to an entry, `"*"` for all): `bericht` (the default), `marke`, `eingabe`, `ende`, `warte`,
/// `groesse`.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // the plan holds small whole numbers; the range is checked
fn lauf_kind(a: &[String]) -> i32 {
    // section 18: the service gives `--meldung <pfad>` before the start file; this check program has nothing to report and
    // leaves the pair out, so that what it reports of its arguments is that of the program without the report
    let a = &ohne_meldung(a);
    let roh = std::fs::read(text(a, 1)).unwrap_or_default();
    let start: Option<serde_json::Value> = serde_json::from_slice(&roh).ok();
    let sid = start
        .as_ref()
        .and_then(|s| s.get("sitzung"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or("");
    let plan: serde_json::Value = std::env::var("KIND_PLAN")
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_else(|| serde_json::json!({}));
    let p = plan
        .get(sid)
        .or_else(|| plan.get("*"))
        .cloned()
        .unwrap_or_else(|| serde_json::json!({"modus": "bericht"}));
    let zahl_von = |k: &str, vorgabe: f64| {
        p.get(k)
            .and_then(serde_json::Value::as_f64)
            .unwrap_or(vorgabe)
    };
    let warten = |k: &str, vorgabe: f64| {
        std::thread::sleep(Duration::try_from_secs_f64(zahl_von(k, vorgabe)).unwrap_or_default());
    };
    match p
        .get("modus")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("bericht")
    {
        "bericht" => {
            lauf_bericht(a, start.as_ref());
            warten("sek", 0.0);
        }
        modus @ ("marke" | "eingabe") => {
            let marke = p
                .get("text")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("");
            schreibe(format!("MARKE {marke}\r\n").as_bytes());
            if modus == "eingabe" {
                let n = zahl_von("n", 1.0);
                let n = if (0.0..1e9).contains(&n) {
                    n as usize
                } else {
                    1
                };
                let d = lies(n);
                schreibe(format!("EINGABE {}\r\n", hex(&d)).as_bytes());
            }
            warten("sek", 30.0);
        }
        "ende" => {
            let c = zahl_von("code", 0.0);
            return if (0.0..1e9).contains(&c) {
                (c as i32) & 0xff
            } else {
                0
            };
        }
        "scheitern" => return scheitern(a),
        "raeumen" => return raeumen(a, &p),
        "warte" => {
            #[cfg(unix)]
            let _ = signal_hook::flag::register(
                signal_hook::consts::SIGHUP,
                Arc::new(AtomicBool::new(false)),
            );
            schreibe(format!("PID={}\r\n", std::process::id()).as_bytes());
            warten("sek", 20.0);
        }
        "groesse" => {
            schreibe(format!("GROESSE {}\r\n", stty(&["size"])).as_bytes());
            warten("sek", 30.0);
        }
        _ => {}
    }
    0
}

/// The plan `scheitern` (DI12 point 12): the child writes the files of the session and ends with 78.
fn scheitern(a: &[String]) -> i32 {
    let ordner = Path::new(text(a, 1)).parent().unwrap_or(Path::new("/"));
    let eigene = format!("{}\n", std::process::id());
    for (name, inhalt) in [
        ("suche.pid", eigene.as_str()),
        ("modellweg.pid", "999999\n"),
        ("ca.key", "ATTRAPPE\n"),
        ("bundle.pem", "ATTRAPPE\n"),
    ] {
        let _ = std::fs::write(ordner.join(name), inhalt);
    }
    78
}

/// The witness of the cases (DI12 point 12): `zeuge <marker> <bereit> <sek>`. It ignores SIGHUP, writes `TERM` into the marker file when
/// it gets SIGTERM and ends; once the handlers stand it writes its pid (and a line feed) into the ready file; after `sek` seconds it ends by itself.
fn zeuge(a: &[String]) -> i32 {
    let term = Arc::new(AtomicBool::new(false));
    let _ = signal_hook::flag::register(signal_hook::consts::SIGTERM, Arc::clone(&term));
    #[cfg(unix)]
    let _ = signal_hook::flag::register(
        signal_hook::consts::SIGHUP,
        Arc::new(AtomicBool::new(false)),
    );
    let bereit = text(a, 1);
    let tmp = format!("{bereit}.tmp");
    if std::fs::write(&tmp, format!("{}\n", std::process::id())).is_err() {
        return 1;
    }
    if std::fs::rename(&tmp, bereit).is_err() {
        return 1;
    }
    let ende = std::time::Instant::now()
        + Duration::try_from_secs_f64(text(a, 2).parse().unwrap_or(0.0)).unwrap_or_default();
    while ende
        .checked_duration_since(std::time::Instant::now())
        .is_some()
    {
        if term.load(std::sync::atomic::Ordering::SeqCst) {
            let _ = std::fs::write(text(a, 0), "TERM\n");
            return 0;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    0
}

/// Starts a witness for the plan `raeumen`: `leiter` in a session of its own (through the carrier, a child of a PTY leads its session),
/// `gruppenleiter` in a group of its own in the session of this child. Waits for its ready file; returns its pid.
fn zeuge_starten(ordner: &Path, art: &str) -> Option<u32> {
    let o = ordner.parent()?;
    let bereit = ordner.join("zeuge.bereit");
    let exe = std::env::current_exe().ok()?;
    let argv = [
        "--kind".to_owned(),
        "zeuge".to_owned(),
        o.join("zeuge.term").to_string_lossy().into_owned(),
        bereit.to_string_lossy().into_owned(),
        "6".to_owned(),
    ];
    let pid = if art == "leiter" {
        // the carrier gives the PTY and with it a session of its own
        let mut v = vec![exe.to_string_lossy().into_owned()];
        v.extend(argv);
        let kind = traeger::Kind::starten(traeger::Start {
            argv: v,
            env: Vec::new(),
            cwd: std::path::PathBuf::from("/"),
            spalten: 20,
            zeilen: 5,
            ruecklauf: 10,
        })
        .ok()?;
        let pid = kind.pid()?;
        std::mem::forget(kind);
        pid
    } else {
        #[cfg(unix)]
        use std::os::unix::process::CommandExt as _;
        let mut befehl = Command::new(&exe);
        befehl.args(&argv);
        // Windows has no process group of Unix (section 21)
        #[cfg(unix)]
        befehl.process_group(0);
        let kind = befehl
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .ok()?;
        kind.id()
    };
    for _ in 0..500 {
        if bereit.exists() {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    Some(pid)
}

/// The plan `raeumen` (DI12 point 12): pid files and links of a session; the keys are listed in the contract. Writes `FERTIG` at the end,
/// stays `sek` seconds, ends with `code`.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // the plan holds small whole numbers
fn raeumen(a: &[String], p: &serde_json::Value) -> i32 {
    #[cfg(unix)]
    use std::os::unix::fs::symlink;
    // Windows (section 21): a link to a folder and a link to a file are two calls (`sys::symlink`)
    #[cfg(windows)]
    use crate::sys::symlink;
    let ordner = Path::new(text(a, 1)).parent().unwrap_or(Path::new("/"));
    let o = ordner.parent().unwrap_or(Path::new("/"));
    let wort = |k: &str, vorgabe: &'static str| {
        p.get(k)
            .and_then(serde_json::Value::as_str)
            .unwrap_or(vorgabe)
            .to_owned()
    };
    let art = wort("pid", "leiter");
    let datei = wort("datei", "suche.pid");
    if p.get("links").and_then(serde_json::Value::as_bool) == Some(true) {
        let _ = std::fs::write(o.join("fremd.txt"), "FREMD");
        let _ = symlink(o.join("fremd.txt"), ordner.join("ca.key"));
        let _ = symlink(o, ordner.join("bundle"));
    }
    if art != "keine" {
        let pid = if art == "vor" {
            o.parent()
                .and_then(|b| std::fs::read_to_string(b.join("vorzeuge.pid")).ok())
                .and_then(|t| t.trim().parse::<u32>().ok())
        } else {
            zeuge_starten(ordner, &art)
        };
        let pid = pid.unwrap_or(0);
        let text = match wort("form", "ok").as_str() {
            "ohne_zeilenende" => format!("{pid}"),
            "leerzeichen" => format!(" {pid}\n"),
            "zwei_zeilen" => format!("{pid}\n\n"),
            _ => format!("{pid}\n"),
        };
        if wort("ziel", "") == "link" {
            let _ = std::fs::write(o.join("ziel.pid"), &text);
            let _ = symlink(o.join("ziel.pid"), ordner.join(&datei));
        } else {
            let _ = std::fs::write(ordner.join(&datei), &text);
        }
    }
    if let Some(inhalt) = p.get("inhalt").and_then(serde_json::Value::as_str) {
        let _ = std::fs::write(ordner.join(&datei), inhalt);
    }
    schreibe(b"FERTIG\r\n");
    let sek = p
        .get("sek")
        .and_then(serde_json::Value::as_f64)
        .unwrap_or(0.0);
    std::thread::sleep(Duration::try_from_secs_f64(sek).unwrap_or_default());
    let code = p
        .get("code")
        .and_then(serde_json::Value::as_f64)
        .unwrap_or(0.0);
    if (0.0..1e9).contains(&code) {
        (code as i32) & 0xff
    } else {
        0
    }
}

/// The report of the mode `bericht`: the start structure in canonical form, the modes of the file and of the folder, the name, the
/// number of the arguments, the environment (without `KIND_PLAN` and the five variables of the Xcode Python), the working
/// directory.
fn lauf_bericht(a: &[String], start: Option<&serde_json::Value>) {
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;
    let datei = Path::new(text(a, 1));
    #[cfg(unix)]
    let modus =
        |p: &Path| std::fs::metadata(p).map(|m| format!("{:04o}", m.permissions().mode() & 0o7777));
    // Windows (section 21): the mode the access list stands for
    #[cfg(windows)]
    let modus = crate::sys::modus_von;
    let (modi, name) = match (modus(datei), datei.parent().map(modus)) {
        (Ok(d), Some(Ok(o))) => (
            format!("{d}/{o}"),
            format!(
                "{}/{}",
                datei
                    .parent()
                    .and_then(Path::file_name)
                    .map_or(String::new(), |n| n.to_string_lossy().into_owned()),
                datei
                    .file_name()
                    .map_or(String::new(), |n| n.to_string_lossy().into_owned())
            ),
        ),
        _ => ("?".to_owned(), "?".to_owned()),
    };
    let start_text = start.map_or_else(
        || "?".to_owned(),
        |s| String::from_utf8_lossy(&serde_json::to_vec(s).unwrap_or_default()).into_owned(),
    );
    let env: std::collections::BTreeMap<String, String> = std::env::vars_os()
        .map(|(k, v)| {
            (
                k.to_string_lossy().into_owned(),
                v.to_string_lossy().into_owned(),
            )
        })
        .filter(|(k, _)| k != "KIND_PLAN" && !AUSGENOMMEN.contains(&k.as_str()))
        .collect();
    let env_text = serde_json::to_string(&env).unwrap_or_default();
    let name0 = Path::new(text(a, 0))
        .file_name()
        .map_or(String::new(), |n| n.to_string_lossy().into_owned());
    // Windows (section 21): a program carries `.exe`; the report names it without, as the name of the program under Unix
    #[cfg(windows)]
    let name0 = crate::sys::ohne_exe(name0);
    let cwd = std::env::current_dir().map_or_else(|_| String::new(), |p| pfad_text(&p));
    for z in [
        format!("START={start_text}"),
        format!("MODI={modi}"),
        format!("NAME={name}"),
        format!("ARGV={}/{name0}", a.len()),
        format!("ENV={env_text}"),
        format!("CWD={cwd}"),
        "ENDE".to_owned(),
    ] {
        schreibe(format!("{z}\r\n").as_bytes());
    }
}

/// `stufen <hex1> [<hex2> ...]`: `hex1` now, every further piece after one more byte of the client.
fn stufen(a: &[String]) {
    schreibe(&unhex(text(a, 0)));
    for h in a.iter().skip(1) {
        lies(1);
        schreibe(&unhex(h));
    }
}

/// `fuellen <n1> [<n2> ...]`: per picture `ESC[H` and `n` times a letter (a new letter per picture), the next picture after one
/// more byte, at the end `ESC[2;1H` and `ENDE`.
fn fuellen(a: &[String]) {
    // a prelude after the start byte (a title, for the check that the protocol line holds no content of the session): the viewer has
    // seen `BEREIT`, so it is there when the title comes, whatever the speed of the start (no wait for a time)
    let a = if text(a, 0).starts_with('@') {
        schreibe(&unhex(&text(a, 0)[1..]));
        &a[1..]
    } else {
        a
    };
    for (k, laenge) in a.iter().enumerate() {
        if k > 0 {
            lies(1);
        }
        let buchstabe = b'a' + u8::try_from(k % 26).unwrap_or(0);
        let mut bild = b"\x1b[H".to_vec();
        bild.resize(3 + laenge.parse::<usize>().unwrap_or(0), buchstabe);
        schreibe(&bild);
    }
    lies(1);
    schreibe(b"\x1b[2;1HENDE\r\n");
}

/// `osc52viele <n> <b> [wort]`: `n` clipboard writes of `b` bytes (the bytes begin with `wort`, else `k`), then `ENDE`.
fn osc52viele(a: &[String]) {
    let b = zahl(a, 1);
    let mut roh = text(a, 2).as_bytes().to_vec();
    roh.resize(roh.len().max(b), b'k');
    roh.truncate(b);
    let folge = format!("\x1b]52;c;{}\x07", base64(&roh));
    for _ in 0..zahl(a, 0) {
        schreibe(folge.as_bytes());
    }
    schreibe(b"ENDE\r\n");
}

/// `signal <nr>`: sends itself the signal and waits; the default action ends the child.
#[cfg(unix)]
fn signal_an_sich(a: &[String]) {
    let nummer = i32::try_from(zahl(a, 0)).unwrap_or(0);
    if let Some(signal) = rustix::process::Signal::from_named_raw(nummer) {
        let _ = rustix::process::kill_process(rustix::process::getpid(), signal);
    }
    std::thread::sleep(Duration::from_secs(5));
}

// Windows: [`crate::sys::signal_an_sich`] (no signal to itself, section 21; the child ends after 5 s by itself).
#[cfg(windows)]
use crate::sys::signal_an_sich;

/// `warte <s>`: ignores SIGHUP, writes its process id and waits; only SIGKILL ends it, so the service must see to it.
fn warte(a: &[String]) {
    #[cfg(unix)]
    let _ = signal_hook::flag::register(
        signal_hook::consts::SIGHUP,
        Arc::new(AtomicBool::new(false)),
    );
    schreibe(format!("PID={}\r\n", std::process::id()).as_bytes());
    schlafe(text(a, 0));
}

/// `bilder <n> [s]`: one picture per further byte, then `ENDE`, then optionally a wait.
fn bilder(a: &[String]) {
    for k in 1..=zahl(a, 0) {
        lies(1);
        let mut bild = String::from("\x1b[H");
        for j in 0..4 {
            let _ = write!(bild, "Bild {k:04} Zeile {j} {}\r\n", "x".repeat(40));
        }
        schreibe(bild.as_bytes());
    }
    lies(1);
    schreibe(b"ENDE\r\n");
    if let Some(sekunden) = a.get(1) {
        schlafe(sekunden);
    }
}

/// The variables that `umgebung` does not count: the Python of the Xcode wrapper adds them on the Mac, a child in Rust does not
/// know them.
const AUSGENOMMEN: [&str; 5] = [
    "CPATH",
    "LIBRARY_PATH",
    "MANPATH",
    "SDKROOT",
    "__CF_USER_TEXT_ENCODING",
];

/// `umgebung <NAME...>`: `NAME=<value>` or `NAME=-`, then `CWD=` and `ANZAHL=`. The count leaves out five variables that the
/// Python of the Xcode wrapper adds on the Mac; a child in Rust does not know them.
fn umgebung(a: &[String]) {
    for name in a {
        let wert = std::env::var_os(name)
            .map_or_else(|| "-".to_owned(), |w| w.to_string_lossy().into_owned());
        schreibe(format!("{name}={wert}\r\n").as_bytes());
    }
    let ordner = std::env::current_dir().map_or_else(|_| "-".to_owned(), |p| pfad_text(&p));
    schreibe(format!("CWD={ordner}\r\n").as_bytes());
    let anzahl = std::env::vars_os()
        .filter(|(k, _)| !AUSGENOMMEN.iter().any(|a| k == a))
        .count();
    schreibe(format!("ANZAHL={anzahl}\r\n").as_bytes());
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn hex_hin_und_zurueck() {
        assert_eq!(unhex("48414c4c4f0d0a"), b"HALLO\r\n".to_vec());
        assert_eq!(unhex("4A"), vec![0x4a]);
        assert_eq!(unhex(""), Vec::<u8>::new());
        assert_eq!(unhex("4"), Vec::<u8>::new());
        assert_eq!(unhex("zz"), Vec::<u8>::new());
        assert_eq!(unhex("4g"), Vec::<u8>::new());
        assert_eq!(unhex("g4"), Vec::<u8>::new());
        assert_eq!(hex(&[0, 0x0f, 0xa0, 0xff]), "000fa0ff");
        assert_eq!(unhex(&hex(&[1, 2, 253])), vec![1, 2, 253]);
    }

    #[test]
    fn base64_wie_rfc4648() {
        for (roh, soll) in [
            ("", ""),
            ("f", "Zg=="),
            ("fo", "Zm8="),
            ("foo", "Zm9v"),
            ("foob", "Zm9vYg=="),
            ("fooba", "Zm9vYmE="),
            ("foobar", "Zm9vYmFy"),
        ] {
            assert_eq!(base64(roh.as_bytes()), soll, "{roh}");
        }
        assert_eq!(base64(&[0xff, 0xfe, 0xfd]), "//79");
        assert_eq!(base64(&[0xfb, 0xef]), "++8=");
        assert_eq!(base64(&vec![b'k'; 90_000]).len(), 120_000);
    }

    #[test]
    fn zahlen_und_texte() {
        let a: Vec<String> = ["x", "12", "-3", "abc"]
            .iter()
            .map(|s| (*s).to_owned())
            .collect();
        assert_eq!(zahl(&a, 1), 12);
        assert_eq!(zahl(&a, 2), 0);
        assert_eq!(zahl(&a, 3), 0);
        assert_eq!(zahl(&a, 9), 0);
        assert_eq!(text(&a, 0), "x");
        assert_eq!(text(&a, 9), "");
    }

    #[test]
    fn die_meldung_vor_der_startdatei_wird_weggelassen() {
        let v = |l: &[&str]| l.iter().map(|s| (*s).to_owned()).collect::<Vec<String>>();
        assert_eq!(
            ohne_meldung(&v(&["p", "--meldung", "/s/meldung.sock", "/s/start.json"])),
            v(&["p", "/s/start.json"])
        );
        assert_eq!(
            ohne_meldung(&v(&["p", "/s/start.json"])),
            v(&["p", "/s/start.json"])
        );
        assert_eq!(
            ohne_meldung(&v(&["p", "--meldung"])),
            v(&["p", "--meldung"])
        );
        assert_eq!(
            ohne_meldung(&v(&["--meldung", "x", "y", "z"])),
            v(&["--meldung", "x", "y", "z"])
        );
    }

    #[test]
    fn unbekannt_endet_mit_64() {
        assert_eq!(lauf(&["nirgends".to_owned()]), 64);
        assert_eq!(lauf(&[]), 64);
    }

    #[test]
    fn still_schreibt_nichts_und_endet_mit_0() {
        assert_eq!(lauf(&["still".to_owned(), "0".to_owned()]), 0);
        assert_eq!(lauf(&["still".to_owned(), "kein-zahl".to_owned()]), 0);
    }
}
