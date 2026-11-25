//! `rangkern sitzung` as a program: the start in its fixed order, the readiness line, the socket and its rights, the end on
//! SIGTERM and SIGINT, the answers with the gate in front, and the log lines of the blocked hits.
//!
//! Every vault here is invented. Each program runs with a cleared environment and only `PATH`, `LANG`, `TZ` and `HOME`, as the
//! runner of the contract does. The sockets lie under `/tmp`, because the path of a Unix socket must be short.

#![cfg(unix)]

use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
#[cfg(target_os = "linux")]
use std::os::unix::fs::MetadataExt;
use std::os::unix::fs::{FileTypeExt, PermissionsExt, symlink};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};

const BIN: &str = env!("CARGO_BIN_EXE_rangkern");

/// The settings of the contract's cases: `cloud` sees `intern`, `lokal` everything of the customer `k-nord`.
fn einstellungen() -> Value {
    json!({
        "fassung": 1,
        "klassen": ["oeffentlich", "intern", "kundendaten"],
        "vorgabestufe": "kundendaten",
        "routen": [
            {"schluessel": "cloud", "art": "cloud", "klasse": "intern"},
            {"schluessel": "lokal", "art": "lokal", "klasse": "kundendaten"}
        ],
        "kunden": [{"schluessel": "k-nord", "erlaubt": ["lokal"], "verboten": []}],
        "kundendaten_an_cloud": {"vorgabe": "nie", "freigaben": []}
    })
}

fn notiz(titel: &str, front: &str, text: &str) -> String {
    format!("---\ntitle: \"{titel}\"\ntype: \"note\"\n{front}---\n\n{text}\n")
}

/// The vault of the cases: two allowed notes, one blocked by customer, one blocked for want of a class.
fn vault() -> Vec<(&'static str, String)> {
    vec![
        (
            "projekte/lieferplan.md",
            notiz(
                "Lieferplan Lager",
                "datenklasse: \"intern\"\n",
                "Der Lieferplan regelt die Termine im Lager.",
            ),
        ),
        (
            "kunden/nord/lieferplan-qkx1.md",
            notiz(
                "Lieferplan QKX1",
                "datenklasse: \"kundendaten\"\nkunde: \"k-nord\"\n",
                "Lieferplan QKX1: Lieferplan und Preise des Kunden. Lieferplan QKX1 bleibt intern.",
            ),
        ),
        (
            "handbuch/lieferplan-lesen.md",
            notiz(
                "Lieferplan lesen",
                "datenklasse: \"oeffentlich\"\n",
                "Wie man einen Lieferplan liest.",
            ),
        ),
        (
            "eingang/lieferplan-qkx2.md",
            notiz("Lieferplan QKX2", "", "Lieferplan QKX2 kam ohne Klasse an."),
        ),
        (
            "sonst/wetter.md",
            notiz(
                "Wetter",
                "datenklasse: \"intern\"\n",
                "Sonne und Regen im Herbst.",
            ),
        ),
    ]
}

/// A folder under `/tmp` with the vault, the settings and the log folder of one test.
struct Welt {
    root: PathBuf,
}

impl Welt {
    fn new() -> Self {
        static ZAEHLER: AtomicUsize = AtomicUsize::new(0);
        let nr = ZAEHLER.fetch_add(1, Ordering::Relaxed);
        let root = PathBuf::from(format!("/tmp/rks-{}-{nr}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        for ordner in ["home", "protokoll", "aux", "vault"] {
            fs::create_dir_all(root.join(ordner)).expect("folder is made");
        }
        // The folder of the socket: 0700, as the contract asks for it under macOS (a check of the parent folder before `bind`);
        // under Linux it changes nothing of what the tests expect.
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).expect("mode of the folder");
        let welt = Self { root };
        welt.schreiben("aux/einstellungen.json", &einstellungen().to_string());
        for (rel, text) in vault() {
            welt.schreiben(&format!("vault/{rel}"), &text);
        }
        welt
    }

    fn schreiben(&self, rel: &str, text: &str) {
        let pfad = self.root.join(rel);
        fs::create_dir_all(pfad.parent().expect("a parent")).expect("parent is made");
        fs::write(pfad, text).expect("file is written");
    }

    fn pfad(&self, rel: &str) -> PathBuf {
        self.root.join(rel)
    }

    fn sock(&self) -> PathBuf {
        self.root.join("s.sock")
    }

    /// The arguments of a valid start for `route`, then `extra`.
    fn args(&self, route: &str, extra: &[&str]) -> Vec<String> {
        let mut args: Vec<String> = [
            "sitzung".to_string(),
            self.pfad("vault").to_string_lossy().to_string(),
            "--socket".to_string(),
            self.sock().to_string_lossy().to_string(),
            "--route".to_string(),
            route.to_string(),
            "--sitzung".to_string(),
            "s-test-1".to_string(),
            "--einstellungen".to_string(),
            self.pfad("aux/einstellungen.json")
                .to_string_lossy()
                .to_string(),
            "--protokoll".to_string(),
            self.pfad("protokoll").to_string_lossy().to_string(),
        ]
        .to_vec();
        args.extend(extra.iter().map(|s| (*s).to_string()));
        args
    }

    fn befehl(&self) -> Command {
        let mut befehl = Command::new(BIN);
        befehl
            .current_dir(&self.root)
            .env_clear()
            .env("PATH", std::env::var_os("PATH").unwrap_or_default())
            .env("LANG", "C.UTF-8")
            .env("TZ", "UTC")
            .env("HOME", self.pfad("home"));
        befehl
    }

    /// Starts the program and waits for the readiness line.
    fn starten(&self, route: &str, extra: &[&str]) -> Lauf {
        let mut lauf = self.starten_ohne_warten(&self.args(route, extra));
        assert_eq!(lauf.zeile(), "{\"bereit\":true}\n", "readiness line");
        lauf.bereit = true;
        lauf
    }

    fn starten_ohne_warten(&self, args: &[String]) -> Lauf {
        let mut kind = self
            .befehl()
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("program starts");
        let stdout = BufReader::new(kind.stdout.take().expect("stdout is piped"));
        let kind = Arc::new(Mutex::new(kind));
        // A program that does not answer would block the test for ever. After a while the watchdog ends it.
        let wache = Arc::clone(&kind);
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_secs(25));
            let _ = wache.lock().map(|mut kind| kind.kill());
        });
        Lauf {
            kind,
            stdout,
            sock: self.sock(),
            bereit: false,
        }
    }

    /// Runs a start that must fail: the exit code, with nothing on standard output. A start that does not end (it
    /// wrongly succeeded) is killed after eight seconds and gives no exit code, which fails the comparison.
    fn scheitert(&self, args: &[String]) -> i32 {
        let mut kind = self
            .befehl()
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("program starts");
        let ende = Instant::now() + Duration::from_secs(8);
        while kind.try_wait().expect("wait").is_none() {
            if Instant::now() > ende {
                kind.kill().expect("kill");
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        let ausgabe = kind.wait_with_output().expect("the end");
        assert!(ausgabe.stdout.is_empty(), "standard output is empty");
        ausgabe.status.code().unwrap_or(-1)
    }

    /// The lines of the log of the month.
    fn protokoll(&self) -> Vec<Value> {
        let mut dateien: Vec<PathBuf> = fs::read_dir(self.pfad("protokoll"))
            .expect("log folder")
            .map(|e| e.expect("entry").path())
            .collect();
        dateien.sort();
        dateien
            .iter()
            .flat_map(|d| {
                fs::read_to_string(d)
                    .expect("log file")
                    .lines()
                    .map(str::to_owned)
                    .collect::<Vec<_>>()
            })
            .map(|z| serde_json::from_str(&z).expect("a JSON line"))
            .collect()
    }
}

impl Drop for Welt {
    fn drop(&mut self) {
        // The log folder and the folder of the count file may have lost their rights in a test.
        for name in ["protokoll", "zaehl"] {
            let _ = fs::set_permissions(self.pfad(name), fs::Permissions::from_mode(0o755));
        }
        let _ = fs::remove_dir_all(&self.root);
    }
}

/// A running `rangkern sitzung`.
struct Lauf {
    kind: Arc<Mutex<Child>>,
    stdout: BufReader<std::process::ChildStdout>,
    sock: PathBuf,
    bereit: bool,
}

impl Lauf {
    /// The error output of the program; the rest of it after the end.
    fn stderr(&mut self) -> std::process::ChildStderr {
        self.kind
            .lock()
            .expect("lock")
            .stderr
            .take()
            .expect("stderr is piped")
    }

    fn pid(&self) -> u32 {
        self.kind.lock().expect("lock").id()
    }

    fn zeile(&mut self) -> String {
        let mut zeile = String::new();
        self.stdout.read_line(&mut zeile).expect("a line");
        zeile
    }

    fn verbinden(&self) -> Verbindung {
        let strom = UnixStream::connect(&self.sock).expect("the socket accepts");
        // macOS refuses `setsockopt` (EINVAL) once the process has closed its end, as it does at once for a 17th connection.
        // A closed end cannot hold a read up, so the failure is ignored there; every other error still fails the test.
        if let Err(fehler) = strom.set_read_timeout(Some(Duration::from_secs(15))) {
            assert!(
                cfg!(target_os = "macos") && fehler.kind() == std::io::ErrorKind::InvalidInput,
                "timeout: {fehler}"
            );
        }
        Verbindung {
            lesen: BufReader::new(strom.try_clone().expect("clone")),
            schreiben: strom,
        }
    }

    fn signal(&self, name: &str) {
        let status = Command::new("kill")
            .arg(format!("-{name}"))
            .arg(self.pid().to_string())
            .status()
            .expect("kill runs");
        assert!(status.success());
    }

    /// Waits for the end; the exit code.
    fn warten(&mut self) -> Option<i32> {
        let ende = Instant::now() + Duration::from_secs(5);
        loop {
            if let Some(status) = self.kind.lock().expect("lock").try_wait().expect("wait") {
                return status.code();
            }
            assert!(Instant::now() < ende, "the program ends in time");
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    /// SIGTERM, then the exit code.
    fn beenden(&mut self) -> Option<i32> {
        self.signal("TERM");
        self.warten()
    }
}

impl Drop for Lauf {
    fn drop(&mut self) {
        if let Ok(mut kind) = self.kind.lock() {
            let _ = kind.kill();
            let _ = kind.wait();
        }
    }
}

struct Verbindung {
    lesen: BufReader<UnixStream>,
    schreiben: UnixStream,
}

impl Verbindung {
    fn roh_senden(&mut self, bytes: &[u8]) -> String {
        self.schreiben.write_all(bytes).expect("request is written");
        let mut zeile = String::new();
        let n = self.lesen.read_line(&mut zeile).expect("an answer");
        assert!(n > 0, "the connection answers");
        zeile
    }

    /// The raw answer line to a request.
    fn zeile(&mut self, anfrage: &Value) -> String {
        let mut bytes = anfrage.to_string().into_bytes();
        bytes.push(b'\n');
        self.roh_senden(&bytes)
    }

    fn senden(&mut self, anfrage: &Value) -> Value {
        let zeile = self.zeile(anfrage);
        assert!(zeile.ends_with('\n') && zeile.matches('\n').count() == 1);
        serde_json::from_str(&zeile).expect("an answer in JSON")
    }

    fn suchen(&mut self, query: &str, k: usize) -> Value {
        self.senden(&json!({"op": "search", "query": query, "k": k}))
    }
}

fn rels(antwort: &Value) -> Vec<&str> {
    antwort["hits"]
        .as_array()
        .expect("hits")
        .iter()
        .map(|h| h["rel"].as_str().expect("rel"))
        .collect()
}

// ---- the start ----------------------------------------------------------------------------------------------------

#[test]
fn the_readiness_line_is_the_only_output_and_the_end_removes_the_socket() {
    let welt = Welt::new();
    let mut lauf = welt.starten("cloud", &[]);
    assert!(welt.sock().exists());
    assert_eq!(lauf.beenden(), Some(0));
    let mut rest = Vec::new();
    lauf.stdout.read_to_end(&mut rest).expect("the rest");
    assert!(rest.is_empty(), "nothing after the readiness line");
    assert!(!welt.sock().exists());
}

/// The settings file that the package ships (Fassung 2), copied into the world of a test.
fn vorfuehrung(welt: &Welt) {
    let datei =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../spec/einstellungen/vorfuehrung.json");
    welt.schreiben(
        "aux/einstellungen.json",
        &fs::read_to_string(datei).expect("the settings of the demonstration"),
    );
}

#[test]
fn the_settings_of_the_demonstration_start_the_routes_copilot_and_lokal() {
    for route in ["copilot", "lokal"] {
        let welt = Welt::new();
        vorfuehrung(&welt);
        // `starten` checks the readiness line.
        let mut lauf = welt.starten(route, &[]);
        assert_eq!(lauf.beenden(), Some(0), "{route}");
    }
}

#[test]
fn a_route_that_is_not_in_the_settings_of_the_demonstration_ends_with_78() {
    let welt = Welt::new();
    vorfuehrung(&welt);
    // `cloud` is a route of the other settings of these tests, not of the demonstration.
    assert_eq!(welt.scheitert(&welt.args("cloud", &[])), 78);
}

#[test]
fn a_refused_settings_file_ends_with_78_and_names_its_code_on_the_error_output() {
    let welt = Welt::new();
    let mut datei: Value = serde_json::from_str(
        &fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../spec/einstellungen/vorfuehrung.json"),
        )
        .expect("the settings of the demonstration"),
    )
    .expect("JSON");
    datei["x"] = json!(1);
    welt.schreiben("aux/einstellungen.json", &datei.to_string());
    let ausgabe = welt
        .befehl()
        .args(welt.args("copilot", &[]))
        .stdin(Stdio::null())
        .output()
        .expect("program runs");
    assert_eq!(ausgabe.status.code(), Some(78));
    assert_eq!(ausgabe.stdout, b"", "standard output is empty");
    let fehler = String::from_utf8_lossy(&ausgabe.stderr);
    assert!(
        fehler.contains("einst.schluessel_unbekannt"),
        "the code is on the error output: {fehler}"
    );
}

#[test]
fn sigint_ends_the_process_the_same_way() {
    let welt = Welt::new();
    let mut lauf = welt.starten("cloud", &[]);
    lauf.signal("INT");
    assert_eq!(lauf.warten(), Some(0));
    assert!(!welt.sock().exists());
}

#[test]
fn the_socket_has_the_rights_of_the_option() {
    let welt = Welt::new();
    for (extra, rechte) in [(vec![], 0o600), (vec!["--socket-rechte", "0600"], 0o600)] {
        let lauf = welt.starten("cloud", &extra);
        let meta = fs::symlink_metadata(welt.sock()).expect("the socket");
        assert!(meta.file_type().is_socket(), "a socket");
        assert_eq!(meta.permissions().mode() & 0o7777, rechte, "{extra:?}");
        drop(lauf);
        let _ = fs::remove_file(welt.sock());
    }
}

/// `--socket-rechte 0666` exists under Linux only (the agent may run under its own user there). Under macOS it is a usage
/// error: see `under_macos_the_rights_0666_end_with_64_and_make_no_socket`.
#[cfg(target_os = "linux")]
#[test]
fn the_socket_has_the_rights_0666_when_the_option_asks_for_them() {
    let welt = Welt::new();
    let lauf = welt.starten("cloud", &["--socket-rechte", "0666"]);
    let meta = fs::symlink_metadata(welt.sock()).expect("the socket");
    assert!(meta.file_type().is_socket(), "a socket");
    assert_eq!(meta.permissions().mode() & 0o7777, 0o666);
    drop(lauf);
}

#[cfg(target_os = "macos")]
#[test]
fn under_macos_the_rights_0666_end_with_64_and_make_no_socket() {
    let welt = Welt::new();
    assert_eq!(
        welt.scheitert(&welt.args("cloud", &["--socket-rechte", "0666"])),
        64
    );
    assert!(!welt.sock().exists(), "nothing is made");
}

/// The counterparts of the checks of the start under macOS (addendum to contract 4, sections 2 and 3), through the program.
#[cfg(target_os = "macos")]
#[test]
fn under_macos_a_relative_path_a_path_of_104_bytes_and_a_folder_that_is_not_0700_end_the_start() {
    let welt = Welt::new();
    let ersetzen = |wert: &str| -> Vec<String> {
        let mut args = welt.args("cloud", &[]);
        let at = args.iter().position(|a| a == "--socket").expect("option");
        args[at + 1] = wert.to_string();
        args
    };
    assert_eq!(welt.scheitert(&ersetzen("such.sock")), 64, "relative");
    let lang = format!("/tmp/{}", "a".repeat(99));
    assert_eq!(lang.len(), 104);
    assert_eq!(welt.scheitert(&ersetzen(&lang)), 64, "104 bytes");
    // A parent folder with the mode 0755 is exit 74 and stays as it is.
    fs::set_permissions(&welt.root, fs::Permissions::from_mode(0o755)).expect("mode");
    assert_eq!(welt.scheitert(&welt.args("cloud", &[])), 74, "0755");
    assert!(!welt.sock().exists());
    assert_eq!(
        fs::metadata(&welt.root)
            .expect("folder")
            .permissions()
            .mode()
            & 0o7777,
        0o755
    );
    fs::set_permissions(&welt.root, fs::Permissions::from_mode(0o700)).expect("mode");
}

/// Review B7 (23.11.2026): under Linux the process checks the folder of the socket as under macOS, also with
/// `--socket-rechte 0666`. A folder that is not 0700 ends the start with 74 and leaves the path empty; the positive control is
/// the same start with the folder 0700.
#[cfg(target_os = "linux")]
#[test]
fn under_linux_a_socket_folder_that_is_not_0700_ends_the_start_with_74_also_with_0666() {
    let welt = Welt::new();
    for modus in [0o755, 0o750, 0o711] {
        fs::set_permissions(&welt.root, fs::Permissions::from_mode(modus)).expect("mode");
        for rechte in ["0600", "0666"] {
            assert_eq!(
                welt.scheitert(&welt.args("cloud", &["--socket-rechte", rechte])),
                74,
                "{modus:o} {rechte}"
            );
            assert!(!welt.sock().exists(), "nothing is made");
        }
    }
    fs::set_permissions(&welt.root, fs::Permissions::from_mode(0o700)).expect("mode");
    let lauf = welt.starten("cloud", &["--socket-rechte", "0666"]);
    assert!(welt.sock().exists());
    drop(lauf);
}

#[cfg(target_os = "linux")]
#[test]
fn the_process_is_not_dumpable() {
    // PR_SET_DUMPABLE 0 makes the entries under /proc/<pid> belong to root.
    let welt = Welt::new();
    let lauf = welt.starten("cloud", &[]);
    let meta = fs::metadata(format!("/proc/{}/stat", lauf.pid())).expect("the entry");
    // Root, or 65534 where root is not mapped (inside the user namespace of the enclosure of the hand mutants); never the user.
    let ich = fs::metadata("/proc/self").expect("own entry").uid();
    assert_ne!(meta.uid(), ich, "the process is dumpable");
    assert!(
        matches!(meta.uid(), 0 | 65534),
        "the owner is neither root (0) nor the unmapped root (65534)"
    );
}

#[test]
fn the_exit_codes_of_the_start() {
    let welt = Welt::new();
    let ersetzen = |args: &[String], name: &str, wert: &str| -> Vec<String> {
        let mut neu = args.to_vec();
        let at = neu.iter().position(|a| a == name).expect("option is there");
        neu[at + 1] = wert.to_string();
        neu
    };
    let ohne = |args: &[String], name: &str| -> Vec<String> {
        let mut neu = args.to_vec();
        let at = neu.iter().position(|a| a == name).expect("option is there");
        neu.drain(at..at + 2);
        neu
    };
    let gut = welt.args("cloud", &[]);
    // Exit 64: a usage error.
    let mut kurz = gut.clone();
    kurz.extend(["--rout".to_string(), "cloud".to_string()]);
    assert_eq!(welt.scheitert(&kurz), 64, "short form");
    assert_eq!(welt.scheitert(&["sitzung".to_string()]), 64, "no arguments");
    assert_eq!(welt.scheitert(&["tor".to_string()]), 64, "unknown command");
    assert_eq!(welt.scheitert(&ohne(&gut, "--sitzung")), 64, "no session");
    assert_eq!(welt.scheitert(&ohne(&gut, "--route")), 64, "no route");
    assert_eq!(welt.scheitert(&ohne(&gut, "--socket")), 64, "no socket");
    assert_eq!(
        welt.scheitert(&ersetzen(&gut, "--route", "Cloud")),
        64,
        "route form"
    );
    assert_eq!(
        welt.scheitert(&ersetzen(&gut, "--sitzung", "S 1")),
        64,
        "session form"
    );
    let mut rechte = gut.clone();
    rechte.extend(["--socket-rechte".to_string(), "0644".to_string()]);
    assert_eq!(welt.scheitert(&rechte), 64, "rights");
    let mut ohne_vault = gut.clone();
    ohne_vault[1] = welt.pfad("fehlt").to_string_lossy().to_string();
    assert_eq!(welt.scheitert(&ohne_vault), 64, "vault is missing");
    // A usage error comes before every error of the settings and of the socket.
    fs::write(welt.sock(), "fremd").expect("a file");
    fs::remove_file(welt.pfad("aux/einstellungen.json")).expect("remove");
    assert_eq!(
        welt.scheitert(&ersetzen(&gut, "--route", "Cloud")),
        64,
        "64 before 78 and 74"
    );
    // Exit 78: settings, route, log folder; and 78 comes before 74.
    assert_eq!(
        welt.scheitert(&gut),
        78,
        "no settings file, with the socket taken"
    );
    assert_eq!(
        fs::read(welt.sock()).expect("the file"),
        b"fremd",
        "the file stays"
    );
    fs::remove_file(welt.sock()).expect("remove");
    welt.schreiben("aux/einstellungen.json", "{");
    assert_eq!(welt.scheitert(&gut), 78, "settings are no JSON");
    welt.schreiben("aux/einstellungen.json", &einstellungen().to_string());
    assert_eq!(
        welt.scheitert(&ersetzen(&gut, "--route", "satellit")),
        78,
        "route not in the file"
    );
    assert_eq!(
        welt.scheitert(&ersetzen(
            &gut,
            "--protokoll",
            &welt.pfad("fehlt").to_string_lossy()
        )),
        78,
        "log folder is missing"
    );
    welt.schreiben("aux/datei", "x");
    assert_eq!(
        welt.scheitert(&ersetzen(
            &gut,
            "--protokoll",
            &welt.pfad("aux/datei").to_string_lossy()
        )),
        78,
        "log folder is a file"
    );
    assert_eq!(
        welt.scheitert(&ersetzen(
            &gut,
            "--einstellungen",
            &welt.pfad("aux").to_string_lossy()
        )),
        78,
        "settings are a folder"
    );
    assert!(!welt.sock().exists(), "no socket after a failed start");
}

#[test]
fn only_a_usage_error_prints_the_usage_line() {
    let welt = Welt::new();
    let fehler = |args: &[String]| -> (Option<i32>, String) {
        let ausgabe = welt
            .befehl()
            .args(args)
            .stdin(Stdio::null())
            .output()
            .expect("program runs");
        (
            ausgabe.status.code(),
            String::from_utf8_lossy(&ausgabe.stderr).to_string(),
        )
    };
    let (code, err) = fehler(&["sitzung".to_string()]);
    assert_eq!(code, Some(64));
    assert!(err.starts_with("usage: rangkern "), "{err}");
    assert!(err.contains("| sitzung <vault> --socket"), "{err}");
    fs::remove_file(welt.pfad("aux/einstellungen.json")).expect("remove");
    let (code, err) = fehler(&welt.args("cloud", &[]));
    assert_eq!(code, Some(78));
    assert!(err.contains("settings") && !err.contains("usage"), "{err}");
}

#[test]
fn a_socket_path_that_is_taken_stays_as_it_is_and_the_start_ends_with_74() {
    let welt = Welt::new();
    let gut = welt.args("cloud", &[]);
    // A file.
    fs::write(welt.sock(), "fremd\n").expect("a file");
    assert_eq!(welt.scheitert(&gut), 74);
    assert_eq!(fs::read(welt.sock()).expect("file"), b"fremd\n");
    fs::remove_file(welt.sock()).expect("remove");
    // A folder with content.
    fs::create_dir(welt.sock()).expect("folder");
    fs::write(welt.sock().join("x"), "y").expect("file");
    assert_eq!(welt.scheitert(&gut), 74);
    assert!(welt.sock().join("x").exists());
    fs::remove_dir_all(welt.sock()).expect("remove");
    // A link, also one that leads nowhere.
    symlink(welt.pfad("fehlt"), welt.sock()).expect("link");
    assert_eq!(welt.scheitert(&gut), 74);
    assert!(
        fs::symlink_metadata(welt.sock())
            .expect("link")
            .file_type()
            .is_symlink()
    );
    assert!(
        !welt.pfad("fehlt").exists(),
        "the link's target is not made"
    );
    fs::remove_file(welt.sock()).expect("remove");
    // The socket of another process: it is not taken over.
    let erster = welt.starten("cloud", &[]);
    assert_eq!(welt.scheitert(&gut), 74);
    erster.verbinden().senden(&json!({"op": "ping"}));
    // A path in a folder that does not exist.
    let mut tief = gut.clone();
    let at = tief.iter().position(|a| a == "--socket").expect("option");
    tief[at + 1] = welt.pfad("fehlt/s.sock").to_string_lossy().to_string();
    assert_eq!(welt.scheitert(&tief), 74);
}

#[test]
fn stdin_is_not_read_and_its_end_does_not_end_the_process() {
    let welt = Welt::new();
    let mut kind = welt
        .befehl()
        .args(welt.args("cloud", &[]))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("program starts");
    let mut stdout = BufReader::new(kind.stdout.take().expect("stdout"));
    let mut zeile = String::new();
    stdout.read_line(&mut zeile).expect("line");
    assert_eq!(zeile, "{\"bereit\":true}\n");
    drop(kind.stdin.take());
    std::thread::sleep(Duration::from_millis(300));
    assert!(kind.try_wait().expect("wait").is_none(), "still running");
    let strom = UnixStream::connect(welt.sock()).expect("connects");
    strom
        .set_read_timeout(Some(Duration::from_secs(15)))
        .expect("timeout");
    let mut verbindung = Verbindung {
        lesen: BufReader::new(strom.try_clone().expect("clone")),
        schreiben: strom,
    };
    assert_eq!(
        verbindung.senden(&json!({"op": "ping"})),
        json!({"ok": true})
    );
    kind.kill().expect("kill");
    kind.wait().expect("wait");
}

// ---- the answers --------------------------------------------------------------------------------------------------

#[test]
fn ping_search_and_bad_requests_on_one_connection() {
    let welt = Welt::new();
    let lauf = welt.starten("cloud", &[]);
    let mut c = lauf.verbinden();
    assert_eq!(
        c.senden(&json!({"op": "ping", "id": 1})),
        json!({"ok": true, "id": 1})
    );
    assert_eq!(
        c.senden(&json!({"op": "quit", "id": 7})),
        json!({"error": "bad_request", "id": 7})
    );
    assert_eq!(c.senden(&json!({"op": "ping"})), json!({"ok": true}));
    for (anfrage, soll) in [
        (
            json!({"op": "search", "query": "x", "route": "lokal"}),
            json!({"error": "bad_request"}),
        ),
        (
            json!({"op": "search", "query": "x", "k": 0, "id": "a"}),
            json!({"error": "bad_request", "id": "a"}),
        ),
        (
            json!({"op": "search", "query": "x", "k": 51}),
            json!({"error": "bad_request"}),
        ),
    ] {
        assert_eq!(c.senden(&anfrage), soll);
    }
    assert_eq!(
        c.roh_senden(b"kein json\n"),
        "{\"error\":\"bad_request\"}\n"
    );
    assert_eq!(c.roh_senden(b"\n"), "{\"error\":\"bad_request\"}\n");
    // A carriage return before the line feed is no part of the line.
    assert_eq!(c.roh_senden(b"{\"op\":\"ping\"}\r\n"), "{\"ok\":true}\n");
    // A line over 1 MiB is invalid; the connection goes on.
    let mut lang = vec![b'x'; 1024 * 1024 + 10];
    lang.push(b'\n');
    assert_eq!(c.roh_senden(&lang), "{\"error\":\"bad_request\"}\n");
    assert_eq!(c.senden(&json!({"op": "ping"})), json!({"ok": true}));
    // The largest line that is valid: a query that fills 1 MiB with its frame.
    let rahmen = "{\"op\":\"search\",\"query\":\"\"}".len();
    let query = "a".repeat(1024 * 1024 - rahmen);
    let mut bytes = format!("{{\"op\":\"search\",\"query\":\"{query}\"}}").into_bytes();
    assert_eq!(bytes.len(), 1024 * 1024);
    bytes.push(b'\n');
    let antwort: Value = serde_json::from_str(&c.roh_senden(&bytes)).expect("JSON");
    assert!(antwort.get("hits").is_some(), "{antwort}");
}

#[test]
fn a_search_answers_with_the_allowed_hits_and_the_number_of_the_blocked() {
    let welt = Welt::new();
    let lauf = welt.starten("cloud", &[]);
    let mut c = lauf.verbinden();
    let a = c.suchen("Lieferplan", 5);
    assert_eq!(a["fallback"], json!(true));
    assert_eq!(
        rels(&a),
        ["handbuch/lieferplan-lesen.md", "projekte/lieferplan.md"]
    );
    assert_eq!(a["gesperrt"], json!(2));
    assert_eq!(a.as_object().expect("object").len(), 3);
    // The same answer for the same request, and `id` comes back.
    let b = c.senden(&json!({"op": "search", "query": "Lieferplan", "id": "x"}));
    assert_eq!(b["id"], json!("x"));
    assert_eq!(rels(&b), rels(&a));
    // The hit fields are those of `rangkern serve`.
    let hit = &a["hits"][1];
    assert_eq!(hit["title"], json!("Lieferplan Lager"));
    assert!(
        hit["snippet"]
            .as_str()
            .expect("snippet")
            .contains("Lieferplan")
    );
    // Nothing found: no hits and no blocked ones.
    let leer = c.suchen("Zyklotron", 5);
    assert_eq!(leer["hits"], json!([]));
    assert_eq!(leer["gesperrt"], json!(0));
    // The local route sees the customer's note; the blocked count is then that of the note without a class.
    let lokal = welt.starten_zweite_sitzung("lokal");
    let mut l = lokal.verbinden();
    let a = l.suchen("Lieferplan", 5);
    assert_eq!(a["gesperrt"], json!(1));
    assert!(rels(&a).contains(&"kunden/nord/lieferplan-qkx1.md"));
    assert!(!rels(&a).contains(&"eingang/lieferplan-qkx2.md"));
}

impl Welt {
    /// A second process on the same vault with another route and another socket.
    fn starten_zweite_sitzung(&self, route: &str) -> Lauf {
        let mut args = self.args(route, &[]);
        let at = args.iter().position(|a| a == "--socket").expect("option");
        args[at + 1] = self.pfad("zweite.sock").to_string_lossy().to_string();
        let mut lauf = self.starten_ohne_warten(&args);
        lauf.sock = self.pfad("zweite.sock");
        assert_eq!(lauf.zeile(), "{\"bereit\":true}\n");
        lauf
    }
}

#[test]
fn the_gate_acts_before_the_cut_for_every_k() {
    let welt = Welt::new();
    let lauf = welt.starten("cloud", &[]);
    let mut c = lauf.verbinden();
    let ganze = c.suchen("Lieferplan", 50);
    let alle: Vec<&str> = rels(&ganze);
    for k in 1..=4 {
        let a = c.suchen("Lieferplan", k);
        let soll: Vec<&str> = alle.iter().copied().take(k).collect();
        assert_eq!(rels(&a), soll, "k {k}");
    }
    // The first hit of the list is a blocked note; with k = 1 it still takes no place.
    let eins = c.suchen("Lieferplan", 1);
    assert_eq!(rels(&eins), ["handbuch/lieferplan-lesen.md"]);
    assert_eq!(eins["gesperrt"], json!(1));
}

/// Adds to the vault of `welt`: a blocked note with the marker that is the successor of an allowed note, and a blocked
/// partner of a contradiction.
fn verweise_schreiben(welt: &Welt) {
    // A blocked note with the marker in path, title, text, an alias, and as successor of an allowed note and as the partner
    // of a contradiction.
    welt.schreiben(
        "vault/projekte/regel-alt.md",
        &notiz(
            "Regel alt",
            "datenklasse: \"intern\"\nstatus: \"superseded\"\nsuperseded-by: [\"[[regel-qkx5]]\"]\n",
            "Die Regel Torzeit gilt bis Herbst. Torzeit.",
        ),
    );
    welt.schreiben(
        "vault/kunden/nord/regel-qkx5.md",
        &notiz(
            "Regel QKX5",
            "datenklasse: \"kundendaten\"\nkunde: \"k-nord\"\naliases: [\"qkx5-alias\"]\n",
            "Nachfolgetext des Kunden QKX5 zur Torzeit.",
        ),
    );
    welt.schreiben(
        "vault/projekte/vertrag.md",
        &notiz(
            "Vertrag",
            "datenklasse: \"intern\"\n",
            "Der Vertrag gilt, Vertragstext der Lieferung.",
        ),
    );
    welt.schreiben(
        "vault/kunden/nord/vertrag-qkx6.md",
        &notiz(
            "Vertrag QKX6",
            "datenklasse: \"kundendaten\"\nkunde: \"k-nord\"\n",
            "Widerspruch zum Vertrag, Vertragstext QKX6.",
        ),
    );
    welt.schreiben(
        "vault/_meta/state/contradictions.json",
        &json!({"findings": {
            "w1": {"status": "open", "note_a": {"rel": "projekte/vertrag.md"}, "note_b": {"rel": "kunden/nord/vertrag-qkx6.md"}}
        }})
        .to_string(),
    );
}

#[test]
fn a_blocked_note_is_in_no_byte_of_an_answer() {
    let welt = Welt::new();
    verweise_schreiben(&welt);
    let lauf = welt.starten("cloud", &[]);
    let mut c = lauf.verbinden();
    let mut alles = String::new();
    for query in [
        "Lieferplan",
        "qkx1",
        "QKX2",
        "Torzeit",
        "Regel",
        "qkx5-alias",
        "Vertrag",
        "Vertragstext",
        "QKX6",
        "Widerspruch",
        "Nachfolgetext",
        "kunden nord",
    ] {
        for k in [1, 2, 5, 50] {
            // The `id` is a number: an `id` that is text comes back as it is, and the query is the marker itself.
            let zeile = c.zeile(&json!({"op": "search", "query": query, "k": k, "id": k}));
            alles.push_str(&zeile);
        }
    }
    // The marker is not trivially absent: the search does find the blocked notes, and counts them.
    for query in ["qkx1", "QKX2", "Torzeit", "QKX6", "Nachfolgetext"] {
        let a = c.suchen(query, 50);
        assert!(a["gesperrt"].as_u64().expect("count") >= 1, "{query}: {a}");
    }
    let klein = alles.to_lowercase();
    for marke in [
        "qkx1",
        "qkx2",
        "qkx5",
        "qkx6",
        "k-nord",
        "kunden/",
        "nachfolgetext",
        "preise des kunden",
    ] {
        assert!(!klein.contains(marke), "{marke} stands in an answer");
    }
    // The allowed notes are there: the successor and the partner of the contradiction are blocked, and the fields are gone.
    let antwort = c.suchen("Torzeit", 5);
    assert_eq!(rels(&antwort), ["projekte/regel-alt.md"]);
    assert_eq!(antwort["hits"][0]["status"], json!("superseded"));
    assert!(antwort["hits"][0].get("abgeloest_durch").is_none());
    assert_eq!(antwort["gesperrt"], json!(1));
    let antwort = c.suchen("Vertragstext", 5);
    assert_eq!(rels(&antwort), ["projekte/vertrag.md"]);
    assert!(antwort["hits"][0].get("contradiction").is_none());
    // On the local route the partner is allowed: the successor and the mark show.
    let lokal = welt.starten_zweite_sitzung("lokal");
    let mut l = lokal.verbinden();
    let antwort = l.suchen("Torzeit", 5);
    let alt = antwort["hits"]
        .as_array()
        .expect("hits")
        .iter()
        .find(|h| h["rel"] == "projekte/regel-alt.md")
        .expect("the old rule");
    assert_eq!(alt["abgeloest_durch"], json!(["kunden/nord/regel-qkx5.md"]));
    let antwort = l.suchen("Vertragstext", 5);
    let vertrag = antwort["hits"]
        .as_array()
        .expect("hits")
        .iter()
        .find(|h| h["rel"] == "projekte/vertrag.md")
        .expect("the contract");
    assert_eq!(vertrag["contradiction"], json!(true));
}

#[test]
fn a_note_that_changes_between_two_requests_counts_with_its_new_class() {
    let welt = Welt::new();
    let lauf = welt.starten("cloud", &[]);
    let mut c = lauf.verbinden();
    let vorher = c.suchen("Lieferplan", 5);
    assert!(!rels(&vorher).contains(&"kunden/nord/lieferplan-qkx1.md"));
    welt.schreiben(
        "vault/kunden/nord/lieferplan-qkx1.md",
        &notiz(
            "Lieferplan QKX1",
            "datenklasse: \"oeffentlich\"\n",
            "Lieferplan QKX1: Lieferplan und Preise des Kunden. Lieferplan QKX1 bleibt intern.",
        ),
    );
    let nachher = c.suchen("Lieferplan", 5);
    assert!(rels(&nachher).contains(&"kunden/nord/lieferplan-qkx1.md"));
    assert_eq!(nachher["gesperrt"], json!(1));
}

#[test]
fn two_connections_answer_at_the_same_time_with_the_same_route() {
    let welt = Welt::new();
    let lauf = welt.starten("cloud", &[]);
    let mut a = lauf.verbinden();
    let mut b = lauf.verbinden();
    assert_eq!(
        a.senden(&json!({"op": "ping", "id": 1})),
        json!({"ok": true, "id": 1})
    );
    assert_eq!(
        b.senden(&json!({"op": "ping", "id": 2})),
        json!({"ok": true, "id": 2})
    );
    let ra = a.suchen("Lieferplan", 5);
    let rb = b.suchen("Lieferplan", 5);
    assert_eq!(rels(&ra), rels(&rb));
    // The end of one connection ends nothing.
    drop(a);
    assert_eq!(b.senden(&json!({"op": "ping"})), json!({"ok": true}));
    let mut c = lauf.verbinden();
    assert_eq!(c.senden(&json!({"op": "ping"})), json!({"ok": true}));
    // Two requests in one write are answered in order.
    let mut zwei = json!({"op": "ping", "id": 1}).to_string().into_bytes();
    zwei.push(b'\n');
    zwei.extend(json!({"op": "ping", "id": 2}).to_string().into_bytes());
    zwei.push(b'\n');
    c.schreiben.write_all(&zwei).expect("written");
    let mut eins = String::new();
    let mut zwei_antwort = String::new();
    c.lesen.read_line(&mut eins).expect("first");
    c.lesen.read_line(&mut zwei_antwort).expect("second");
    assert_eq!(eins, "{\"id\":1,\"ok\":true}\n");
    assert_eq!(zwei_antwort, "{\"id\":2,\"ok\":true}\n");
}

#[test]
fn an_unreadable_vault_answers_unreadable() {
    let welt = Welt::new();
    let lauf = welt.starten("cloud", &[]);
    let mut c = lauf.verbinden();
    fs::remove_dir_all(welt.pfad("vault")).expect("remove the vault");
    assert_eq!(
        c.senden(&json!({"op": "search", "query": "Lieferplan", "id": 3})),
        json!({"error": "unreadable", "id": 3})
    );
    assert_eq!(c.senden(&json!({"op": "ping"})), json!({"ok": true}));
}

// ---- the log ------------------------------------------------------------------------------------------------------

/// The UTC time of `ms` as the log writes it, `JJJJ-MM-TTThh:mm:ss.mmmZ`.
fn zeit_text(ms: u128) -> String {
    let tage = i64::try_from(ms / 86_400_000).expect("days");
    let tagesms = u64::try_from(ms % 86_400_000).expect("ms of the day");
    let z = tage + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let tag = doy - (153 * mp + 2) / 5 + 1;
    let monat = if mp < 10 { mp + 3 } else { mp - 9 };
    let jahr = yoe + era * 400 + i64::from(monat <= 2);
    format!(
        "{jahr:04}-{monat:02}-{tag:02}T{:02}:{:02}:{:02}.{:03}Z",
        tagesms / 3_600_000,
        tagesms / 60_000 % 60,
        tagesms / 1000 % 60,
        tagesms % 1000
    )
}

fn jetzt_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_millis()
}

#[test]
fn a_search_with_blocked_hits_writes_one_line_per_group_with_the_time_of_now() {
    let welt = Welt::new();
    let lauf = welt.starten("cloud", &[]);
    let mut c = lauf.verbinden();
    let vor = jetzt_ms();
    let a = c.suchen("Lieferplan", 5);
    let nach = jetzt_ms();
    assert_eq!(a["gesperrt"], json!(2));
    let zeilen = welt.protokoll();
    assert_eq!(zeilen.len(), 2);
    let erwartet = [
        (Value::Null, "klasse-ueber-route"),
        (json!("k-nord"), "klasse-ueber-route"),
    ];
    for (zeile, (kunde, grund)) in zeilen.iter().zip(erwartet) {
        let felder: Vec<&str> = zeile
            .as_object()
            .expect("object")
            .keys()
            .map(String::as_str)
            .collect();
        assert_eq!(felder.len(), 11, "{felder:?}");
        assert_eq!(zeile["aktionsname"], json!("ausgabe.gesperrt"));
        assert_eq!(zeile["ergebnis"], json!("gesperrt"));
        assert_eq!(zeile["fassung"], json!(2));
        assert_eq!(zeile["route"], json!("cloud"));
        assert_eq!(zeile["sitzung"], json!("s-test-1"));
        assert_eq!(zeile["klasse"], json!("kundendaten"));
        assert_eq!(zeile["kunde"], kunde);
        assert_eq!(zeile["grund"], json!(grund));
        assert_eq!(zeile["anzahl"], json!(1));
    }
    // The time is the time of the write.
    let zeit = zeilen[0]["zeit"].as_str().expect("time");
    assert!(
        zeit >= zeit_text(vor).as_str() && zeit <= zeit_text(nach).as_str(),
        "{zeit}"
    );
    // The file is the log of the month of that time, and the chain is whole.
    let datei = welt
        .pfad("protokoll")
        .join(format!("brain-{}.jsonl", &zeit[..7]));
    assert!(datei.exists());
    let pruefung = formate::kette::pruefe(&[datei], formate::kette::NULL_HASH, None);
    assert!(pruefung.ok(), "{pruefung:?}");
    assert_eq!(pruefung.zeilen, 2);
    // A search without blocked hits writes nothing.
    let _ = c.suchen("Wetter", 5);
    assert_eq!(welt.protokoll().len(), 2);
    // Another search adds to the chain.
    let _ = c.suchen("Lieferplan", 1);
    assert_eq!(welt.protokoll().len(), 3);
}

#[test]
fn a_log_line_that_cannot_be_written_gives_an_error_and_no_hit() {
    let welt = Welt::new();
    let lauf = welt.starten("cloud", &[]);
    let mut c = lauf.verbinden();
    fs::set_permissions(welt.pfad("protokoll"), fs::Permissions::from_mode(0o000)).expect("chmod");
    if fs::read_dir(welt.pfad("protokoll")).is_ok() {
        // The test runs with the rights of root, which no mode keeps out.
        return;
    }
    let a = c.senden(&json!({"op": "search", "query": "Lieferplan", "id": 1}));
    assert_eq!(a, json!({"error": "protokoll", "id": 1}));
    // A search without blocked hits needs no log.
    let b = c.senden(&json!({"op": "search", "query": "Wetter", "id": 2}));
    assert_eq!(b["gesperrt"], json!(0));
    assert_eq!(rels(&b), ["sonst/wetter.md"]);
    fs::set_permissions(welt.pfad("protokoll"), fs::Permissions::from_mode(0o755)).expect("chmod");
    let c2 = c.suchen("Lieferplan", 5);
    assert_eq!(c2["gesperrt"], json!(2));
    assert_eq!(welt.protokoll().len(), 2);
}

#[test]
fn the_session_and_the_route_in_the_log_are_those_of_the_start() {
    let welt = Welt::new();
    let mut args = welt.args("lokal", &[]);
    let at = args.iter().position(|a| a == "--sitzung").expect("option");
    args[at + 1] = "andere.sitzung-9".to_string();
    let mut lauf = welt.starten_ohne_warten(&args);
    assert_eq!(lauf.zeile(), "{\"bereit\":true}\n");
    let mut c = lauf.verbinden();
    let a = c.suchen("Lieferplan", 5);
    assert_eq!(a["gesperrt"], json!(1));
    let zeilen = welt.protokoll();
    assert_eq!(zeilen.len(), 1);
    assert_eq!(zeilen[0]["route"], json!("lokal"));
    assert_eq!(zeilen[0]["sitzung"], json!("andere.sitzung-9"));
    assert_eq!(zeilen[0]["klasse"], json!("kundendaten"));
    assert_eq!(zeilen[0]["kunde"], Value::Null);
    assert_eq!(zeilen[0]["grund"], json!("kunde-fehlt"));
}

#[test]
fn the_settings_are_read_once_at_the_start() {
    let welt = Welt::new();
    let lauf = welt.starten("cloud", &[]);
    let mut c = lauf.verbinden();
    let vorher = c.suchen("Lieferplan", 5);
    welt.schreiben("aux/einstellungen.json", "{");
    let nachher = c.suchen("Lieferplan", 5);
    assert_eq!(rels(&vorher), rels(&nachher));
    assert_eq!(nachher["gesperrt"], json!(2));
}

#[test]
fn the_vault_is_not_written() {
    let welt = Welt::new();
    let vorher: Vec<(PathBuf, u64)> = walk(&welt.pfad("vault"));
    let lauf = welt.starten("cloud", &[]);
    let mut c = lauf.verbinden();
    let _ = c.suchen("Lieferplan", 5);
    drop(lauf);
    assert_eq!(walk(&welt.pfad("vault")), vorher);
}

fn walk(ordner: &Path) -> Vec<(PathBuf, u64)> {
    let mut out = Vec::new();
    for eintrag in fs::read_dir(ordner).expect("folder") {
        let pfad = eintrag.expect("entry").path();
        let meta = fs::symlink_metadata(&pfad).expect("meta");
        if meta.is_dir() {
            out.extend(walk(&pfad));
        } else {
            out.push((pfad, meta.len()));
        }
    }
    out.sort();
    out
}

// ---- the counting of the markers (probe run) --------------------------------------------------------------------------

/// Two notes with a marker each: `MI-1A` in an allowed note, `MK-1B` in a note of the customer.
fn marker_notizen(welt: &Welt) {
    welt.schreiben(
        "vault/a/pumpe-mi1a.md",
        &notiz(
            "Pumpe MI-1A",
            "datenklasse: \"intern\"\n",
            "Die Pumpe MI-1A steht in Halle 3.",
        ),
    );
    welt.schreiben(
        "vault/k/pumpe-mk1b.md",
        &notiz(
            "Pumpe MK-1B",
            "datenklasse: \"kundendaten\"\nkunde: \"k-nord\"\n",
            "Die Pumpe MK-1B gehoert dem Kunden.",
        ),
    );
}

fn probe_args() -> [&'static str; 5] {
    [
        "--probelauf",
        "--marker",
        "MI-1A,MK-1B",
        "--marker-zaehlung",
        "ZAEHL",
    ]
}

impl Welt {
    /// Starts a probe run of `route`; the count file is `zaehl/zaehlung.json`.
    fn starten_probe(&self, route: &str) -> Lauf {
        fs::create_dir_all(self.pfad("zaehl")).expect("folder");
        let datei = self.pfad("zaehl/zaehlung.json");
        let mut extra: Vec<String> = probe_args().iter().map(ToString::to_string).collect();
        extra[4] = datei.to_string_lossy().to_string();
        let extra: Vec<&str> = extra.iter().map(String::as_str).collect();
        self.starten(route, &extra)
    }

    fn zaehlung(&self) -> String {
        fs::read_to_string(self.pfad("zaehl/zaehlung.json")).expect("the count file")
    }
}

#[test]
fn a_probe_run_counts_the_markers_of_every_answer_and_says_so_before_the_readiness_line() {
    let welt = Welt::new();
    marker_notizen(&welt);
    let mut lauf = welt.starten_probe("lokal");
    // The file stands at zero before the first request, with the rights 0600.
    assert_eq!(
        welt.zaehlung(),
        r#"{"antworten":0,"marker":{"MI-1A":0,"MK-1B":0},"sitzung":"s-test-1"}"#
    );
    let meta = fs::metadata(welt.pfad("zaehl/zaehlung.json")).expect("meta");
    assert_eq!(meta.permissions().mode() & 0o7777, 0o600);
    let mut c = lauf.verbinden();
    let a = c.suchen("Pumpe", 5);
    assert_eq!(rels(&a), ["a/pumpe-mi1a.md", "k/pumpe-mk1b.md"]);
    let zeile_text = a.to_string();
    let vorn_zahl = zeile_text.matches("MI-1A").count();
    let hinten_zahl = zeile_text.matches("MK-1B").count();
    assert!(vorn_zahl >= 2 && hinten_zahl >= 2);
    assert_eq!(
        welt.zaehlung(),
        format!(
            r#"{{"antworten":1,"marker":{{"MI-1A":{vorn_zahl},"MK-1B":{hinten_zahl}}},"sitzung":"s-test-1"}}"#
        )
    );
    // The `id` stands in the buffer too: it is counted. A ping and a bad request are not answers to `search`.
    let b = c.senden(&json!({"op": "search", "query": "Zyklotron", "id": "MI-1A"}));
    assert_eq!(b["hits"], json!([]));
    c.senden(&json!({"op": "ping", "id": "MI-1A"}));
    c.roh_senden(b"kein json\n");
    assert_eq!(
        welt.zaehlung(),
        format!(
            r#"{{"antworten":2,"marker":{{"MI-1A":{},"MK-1B":{hinten_zahl}}},"sitzung":"s-test-1"}}"#,
            vorn_zahl + 1
        )
    );
    assert_eq!(
        fs::metadata(welt.pfad("zaehl/zaehlung.json"))
            .expect("meta")
            .permissions()
            .mode()
            & 0o7777,
        0o600
    );
    // No file beside the count file stays.
    assert_eq!(fs::read_dir(welt.pfad("zaehl")).expect("dir").count(), 1);
    assert_eq!(lauf.beenden(), Some(0));
    let mut err = String::new();
    lauf.stderr().read_to_string(&mut err).expect("stderr");
    assert_eq!(err, "such.probelauf: Marker werden gez\u{e4}hlt\n");
}

#[test]
fn on_the_cloud_route_the_marker_of_a_blocked_note_counts_zero() {
    let welt = Welt::new();
    marker_notizen(&welt);
    let lauf = welt.starten_probe("cloud");
    let mut c = lauf.verbinden();
    let a = c.suchen("Pumpe", 5);
    assert_eq!(rels(&a), ["a/pumpe-mi1a.md"]);
    assert_eq!(a["gesperrt"], json!(1));
    let zaehlung: Value = serde_json::from_str(&welt.zaehlung()).expect("JSON");
    assert_eq!(zaehlung["marker"]["MK-1B"], json!(0));
    assert!(zaehlung["marker"]["MI-1A"].as_u64().expect("count") >= 2);
    assert_eq!(zaehlung["antworten"], json!(1));
}

#[test]
fn a_count_file_that_cannot_be_written_gives_an_error_and_no_log_line() {
    let welt = Welt::new();
    marker_notizen(&welt);
    let lauf = welt.starten_probe("cloud");
    let mut c = lauf.verbinden();
    fs::set_permissions(welt.pfad("zaehl"), fs::Permissions::from_mode(0o000)).expect("chmod");
    if fs::read_dir(welt.pfad("zaehl")).is_ok() {
        fs::set_permissions(welt.pfad("zaehl"), fs::Permissions::from_mode(0o755)).expect("chmod");
        return;
    }
    let a = c.senden(&json!({"op": "search", "query": "Pumpe", "id": 1}));
    assert_eq!(a, json!({"error": "protokoll", "id": 1}));
    // Only the line of the start stands in the log: the counting comes before the log line of the hit.
    let zeilen = welt.protokoll();
    assert_eq!(zeilen.len(), 1, "the counting comes before the log line");
    assert_eq!(zeilen[0]["aktionsname"], json!("probelauf.zaehlung"));
    fs::set_permissions(welt.pfad("zaehl"), fs::Permissions::from_mode(0o755)).expect("chmod");
    assert_eq!(
        welt.zaehlung(),
        r#"{"antworten":0,"marker":{"MI-1A":0,"MK-1B":0},"sitzung":"s-test-1"}"#
    );
    let b = c.suchen("Pumpe", 5);
    assert_eq!(b["gesperrt"], json!(1));
    assert_eq!(welt.protokoll().len(), 2);
}

#[test]
fn without_the_probe_run_nothing_is_counted_and_nothing_is_said() {
    let welt = Welt::new();
    marker_notizen(&welt);
    let mut lauf = welt.starten("cloud", &[]);
    let mut c = lauf.verbinden();
    c.suchen("Pumpe", 5);
    assert_eq!(lauf.beenden(), Some(0));
    let mut err = String::new();
    lauf.stderr().read_to_string(&mut err).expect("stderr");
    assert_eq!(err, "");
    assert!(!welt.pfad("zaehl").exists());
}

#[test]
fn the_start_of_the_probe_run_ends_with_78_when_the_count_file_cannot_be_written() {
    let welt = Welt::new();
    let mut args = welt.args("cloud", &probe_args());
    let at = args.iter().position(|a| a == "ZAEHL").expect("placeholder");
    args[at] = welt
        .pfad("fehlt/zaehlung.json")
        .to_string_lossy()
        .to_string();
    assert_eq!(welt.scheitert(&args), 78);
    assert!(!welt.sock().exists(), "the socket is removed again");
}

/// The count file belongs to 78, and 78 comes before 74 (contract 4, table of the start; decision of 23.11.2026): with a taken
/// socket path and a count file that cannot be written the start ends with 78, makes no socket, writes no line and leaves the
/// file at the socket path as it is. The positive control: the same start with a free socket path and a usable count file
/// starts.
#[test]
fn the_count_file_is_checked_before_the_socket_and_ends_with_78() {
    let welt = Welt::new();
    let mit_zaehlung = |rel: &str| {
        let mut args = welt.args("cloud", &probe_args());
        let at = args.iter().position(|a| a == "ZAEHL").expect("placeholder");
        args[at] = welt.pfad(rel).to_string_lossy().to_string();
        args
    };
    fs::write(welt.sock(), "fremd").expect("a file at the socket path");
    // The folder of the count file is missing.
    assert_eq!(
        welt.scheitert(&mit_zaehlung("fehlt/zaehlung.json")),
        78,
        "missing folder"
    );
    // The folder is a link to a usable folder.
    fs::create_dir_all(welt.pfad("echt")).expect("folder");
    symlink(welt.pfad("echt"), welt.pfad("verweis")).expect("link");
    assert_eq!(
        welt.scheitert(&mit_zaehlung("verweis/zaehlung.json")),
        78,
        "linked folder"
    );
    // At the name stands a link.
    fs::create_dir_all(welt.pfad("zaehl")).expect("folder");
    symlink(
        welt.pfad("echt/ziel.json"),
        welt.pfad("zaehl/zaehlung.json"),
    )
    .expect("link");
    assert_eq!(
        welt.scheitert(&mit_zaehlung("zaehl/zaehlung.json")),
        78,
        "link at the name"
    );
    fs::remove_file(welt.pfad("zaehl/zaehlung.json")).expect("remove the link");
    // The folder cannot be written (not as root: root writes anyway).
    fs::set_permissions(welt.pfad("zaehl"), fs::Permissions::from_mode(0o500)).expect("chmod");
    if fs::write(welt.pfad("zaehl/probe"), "x").is_err() {
        assert_eq!(
            welt.scheitert(&mit_zaehlung("zaehl/zaehlung.json")),
            78,
            "folder not writable"
        );
    }
    fs::set_permissions(welt.pfad("zaehl"), fs::Permissions::from_mode(0o755)).expect("chmod");
    let _ = fs::remove_file(welt.pfad("zaehl/probe"));
    assert_eq!(fs::read_to_string(welt.sock()).expect("untouched"), "fremd");
    assert!(
        welt.protokoll().is_empty(),
        "no line for a start that failed"
    );
    assert!(!welt.pfad("echt/zaehlung.json").exists() && !welt.pfad("echt/ziel.json").exists());
    // The positive control: a free socket path and a usable count file start.
    fs::remove_file(welt.sock()).expect("free the socket path");
    let lauf = welt.starten_probe("cloud");
    assert!(welt.sock().exists());
    drop(lauf);
}

#[test]
fn the_usage_errors_of_the_probe_run() {
    let welt = Welt::new();
    fs::create_dir_all(welt.pfad("zaehl")).expect("folder");
    let datei = welt
        .pfad("zaehl/zaehlung.json")
        .to_string_lossy()
        .to_string();
    for extra in [
        vec!["--probelauf"],
        vec!["--marker", "A", "--marker-zaehlung", datei.as_str()],
        vec![
            "--probelauf",
            "--marker",
            "mi-1",
            "--marker-zaehlung",
            datei.as_str(),
        ],
        vec![
            "--probelauf",
            "--marker",
            "A,,B",
            "--marker-zaehlung",
            datei.as_str(),
        ],
        vec!["--probelauf", "--marker-zaehlung", datei.as_str()],
    ] {
        assert_eq!(welt.scheitert(&welt.args("cloud", &extra)), 64, "{extra:?}");
    }
    assert!(!welt.pfad("zaehl/zaehlung.json").exists());
}

// ---- the clarifications of 20.11. ---------------------------------------------------------------------------------

#[test]
fn an_empty_value_ends_with_64_before_any_look_at_a_file_or_the_socket() {
    let welt = Welt::new();
    // The settings are missing and the socket path is taken: 78 and 74 would follow, but the empty value comes first.
    fs::remove_file(welt.pfad("aux/einstellungen.json")).expect("remove");
    fs::write(welt.sock(), "fremd").expect("a file");
    for name in [
        "--socket",
        "--einstellungen",
        "--protokoll",
        "--route",
        "--sitzung",
    ] {
        let mut args = welt.args("cloud", &[]);
        let at = args.iter().position(|a| a == name).expect("option");
        args[at + 1] = String::new();
        assert_eq!(welt.scheitert(&args), 64, "{name}");
    }
    assert_eq!(fs::read(welt.sock()).expect("file"), b"fremd");
    let mit_rechten = welt.args("cloud", &["--socket-rechte", ""]);
    assert_eq!(welt.scheitert(&mit_rechten), 64);
    // The same call with the values in place ends with 78: the settings are missing.
    assert_eq!(welt.scheitert(&welt.args("cloud", &[])), 78);
}

#[test]
fn a_line_over_1_mib_is_bad_request_without_an_id_even_if_it_starts_with_a_valid_one() {
    let welt = Welt::new();
    let lauf = welt.starten("cloud", &[]);
    let mut c = lauf.verbinden();
    let fuell = "a".repeat(1024 * 1024 + 1);
    let mut zeile = format!("{{\"op\":\"ping\",\"id\":5,\"query\":\"{fuell}\"}}").into_bytes();
    zeile.push(b'\n');
    assert_eq!(c.roh_senden(&zeile), "{\"error\":\"bad_request\"}\n");
    // The connection goes on after the long line.
    assert_eq!(
        c.senden(&json!({"op": "ping", "id": 6})),
        json!({"ok": true, "id": 6})
    );
}

#[test]
fn a_lone_surrogate_is_bad_request_without_an_id() {
    let welt = Welt::new();
    let lauf = welt.starten("cloud", &[]);
    let mut c = lauf.verbinden();
    for zeile in [
        &br#"{"op":"search","query":"\ud800","id":1}"#[..],
        br#"{"op":"search","id":2,"query":"x\udc00y"}"#,
        br#"{"op":"search","query":"\ud800\ud800","id":3}"#,
        br#"{"op":"ping","id":4,"space":"\ud83d"}"#,
        br#"{"\ud800":1,"op":"ping","id":5}"#,
    ] {
        let mut bytes = zeile.to_vec();
        bytes.push(b'\n');
        assert_eq!(
            c.roh_senden(&bytes),
            "{\"error\":\"bad_request\"}\n",
            "{zeile:?}"
        );
    }
    // A pair of surrogates is a valid character.
    let a = c.senden(&json!({"op": "search", "query": "\u{1F600}", "id": 7}));
    assert_eq!(a["id"], json!(7));
    let mut paar = br#"{"op":"search","query":"\ud83d\ude00","id":8}"#.to_vec();
    paar.push(b'\n');
    let antwort: Value = serde_json::from_str(&c.roh_senden(&paar)).expect("JSON");
    assert_eq!(antwort["id"], json!(8));
    assert!(antwort.get("hits").is_some());
}

#[test]
fn a_failed_log_line_after_the_counting_leaves_the_count_and_sends_an_error() {
    let welt = Welt::new();
    marker_notizen(&welt);
    // A blocked hit on the cloud route needs a log line; the log folder has no rights, the count file is in another folder.
    let lauf = welt.starten_probe("cloud");
    let mut c = lauf.verbinden();
    fs::set_permissions(welt.pfad("protokoll"), fs::Permissions::from_mode(0o000)).expect("chmod");
    if fs::read_dir(welt.pfad("protokoll")).is_ok() {
        return;
    }
    let a = c.senden(&json!({"op": "search", "query": "Pumpe", "id": 1}));
    assert_eq!(a, json!({"error": "protokoll", "id": 1}));
    let zaehlung: Value = serde_json::from_str(&welt.zaehlung()).expect("JSON");
    assert_eq!(
        zaehlung["antworten"],
        json!(1),
        "the answer was counted before the log line"
    );
    assert!(zaehlung["marker"]["MI-1A"].as_u64().expect("count") >= 2);
    assert_eq!(zaehlung["marker"]["MK-1B"], json!(0));
}

#[test]
fn a_marker_twice_in_the_option_ends_with_64() {
    let welt = Welt::new();
    fs::create_dir_all(welt.pfad("zaehl")).expect("folder");
    let datei = welt
        .pfad("zaehl/zaehlung.json")
        .to_string_lossy()
        .to_string();
    let extra = [
        "--probelauf",
        "--marker",
        "MI-1A,MI-1A",
        "--marker-zaehlung",
        datei.as_str(),
    ];
    assert_eq!(welt.scheitert(&welt.args("cloud", &extra)), 64);
    assert!(!welt.pfad("zaehl/zaehlung.json").exists());
}

#[test]
fn the_17th_connection_is_closed_without_a_byte_and_a_freed_place_is_free_again() {
    let welt = Welt::new();
    let lauf = welt.starten("cloud", &[]);
    let mut offen: Vec<Verbindung> = (0..16).map(|_| lauf.verbinden()).collect();
    // The answer to a ping shows that the process has accepted the connection.
    for (nr, v) in offen.iter_mut().enumerate() {
        assert_eq!(
            v.senden(&json!({"op": "ping", "id": nr})),
            json!({"ok": true, "id": nr})
        );
    }
    let mut siebzehnte = lauf.verbinden();
    let mut gelesen = Vec::new();
    // `Ok(0)` is the end without a byte; a reset is no byte either, but only the end is the answer of the process.
    assert_eq!(siebzehnte.lesen.read_to_end(&mut gelesen).ok(), Some(0));
    assert!(gelesen.is_empty(), "no byte for the 17th");
    // The 16 stay served.
    assert_eq!(
        offen[3].senden(&json!({"op": "ping", "id": "n"})),
        json!({"ok": true, "id": "n"})
    );
    // A place that is freed is free: the process sees the end of the connection a moment later.
    drop(offen.remove(0));
    let ende = Instant::now() + Duration::from_secs(10);
    loop {
        let mut neue = lauf.verbinden();
        // A connection that is closed at once may refuse the write; the loop then tries again.
        let _ = neue.schreiben.write_all(b"{\"op\":\"ping\"}\n");
        let mut zeile = String::new();
        if neue.lesen.read_line(&mut zeile).is_ok() && zeile == "{\"ok\":true}\n" {
            break;
        }
        assert!(Instant::now() < ende, "a freed place is served again");
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// The idle time of the process in seconds (clarification 13 of contract 4, decision of 24.11.2026).
const LEERLAUF_S: u64 = 10;

/// Waits for the end of a connection: the time until `read` gives `Ok(0)`, and the bytes read until then.
fn bis_zum_ende(v: &mut Verbindung) -> (Duration, Vec<u8>) {
    let start = Instant::now();
    let mut gelesen = Vec::new();
    assert_eq!(v.lesen.read_to_end(&mut gelesen).ok(), Some(0), "the end");
    (start.elapsed(), gelesen)
}

#[test]
fn a_silent_connection_is_closed_after_10_seconds_from_its_last_byte_and_an_earlier_line_is_answered()
 {
    let welt = Welt::new();
    let lauf = welt.starten("cloud", &[]);
    let mut v = lauf.verbinden();
    // Positive control: a line well before the idle time gets its answer.
    std::thread::sleep(Duration::from_secs(LEERLAUF_S / 2));
    assert_eq!(
        v.senden(&json!({"op": "ping", "id": 1})),
        json!({"ok": true, "id": 1})
    );
    // The time counts from this line, not from the connect: 10 s after the connect the connection is still open.
    std::thread::sleep(Duration::from_secs(LEERLAUF_S / 2 + 2));
    assert_eq!(
        v.senden(&json!({"op": "ping", "id": 2})),
        json!({"ok": true, "id": 2})
    );
    // A half line, then silence: the end after the idle time from its last byte, without an answer.
    v.schreiben.write_all(b"{\"op\":\"pi").expect("half a line");
    let (dauer, gelesen) = bis_zum_ende(&mut v);
    assert!(gelesen.is_empty(), "no byte for a silent connection");
    assert!(
        dauer >= Duration::from_secs(LEERLAUF_S - 1)
            && dauer <= Duration::from_secs(LEERLAUF_S + 3),
        "closed after {dauer:?}"
    );
    // The process goes on and writes no log line for it.
    let mut neu = lauf.verbinden();
    assert_eq!(neu.senden(&json!({"op": "ping"})), json!({"ok": true}));
    assert!(
        fs::read_dir(welt.pfad("protokoll"))
            .expect("log folder")
            .next()
            .is_none(),
        "no log line"
    );
}

#[test]
fn sixteen_silent_connections_block_the_17th_only_until_the_idle_time() {
    let welt = Welt::new();
    let lauf = welt.starten("cloud", &[]);
    let start = Instant::now();
    let mut stumme: Vec<Verbindung> = (0..16).map(|_| lauf.verbinden()).collect();
    std::thread::sleep(Duration::from_millis(500));
    // The places are taken: a 17th is closed at once, without a byte.
    let (dauer, gelesen) = bis_zum_ende(&mut lauf.verbinden());
    assert!(gelesen.is_empty(), "no byte for the 17th");
    assert!(
        dauer < Duration::from_secs(2),
        "the 17th is closed at once, after {dauer:?}"
    );
    // The silent ones are closed after the idle time, each without a byte.
    for v in &mut stumme {
        let (_, gelesen) = bis_zum_ende(v);
        assert!(gelesen.is_empty(), "no byte for a silent connection");
    }
    let seit = start.elapsed();
    assert!(
        seit >= Duration::from_secs(LEERLAUF_S) && seit <= Duration::from_secs(LEERLAUF_S + 4),
        "all 16 closed after {seit:?}"
    );
    // The places are free: a new connection is served.
    let mut neu = lauf.verbinden();
    assert_eq!(
        neu.senden(&json!({"op": "ping", "id": "n"})),
        json!({"ok": true, "id": "n"})
    );
}
