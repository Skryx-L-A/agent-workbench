//! `rangkern sitzung` with `--embed`: the line `suche.rueckfall` in the chain `brain` when the configured embedding fails
//! (contract 4, "Rückfall der Einbettung"; contract 5, section 6.5).
//!
//! Every vault here is invented. A small server on loopback stands in for Ollama. Each program runs with a cleared
//! environment and only `PATH`, `LANG`, `TZ` and `HOME`. The sockets lie under `/tmp`, because the path of a Unix socket
//! must be short.

#![cfg(unix)]

use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{Value, json};

const BIN: &str = env!("CARGO_BIN_EXE_rangkern");
const SITZUNG: &str = "s-rueck-1";

fn notiz(titel: &str, front: &str, text: &str) -> String {
    format!("---\ntitle: \"{titel}\"\ntype: \"note\"\n{front}---\n\n{text}\n")
}

/// Two allowed notes and one blocked for the route `cloud`.
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
                "Lieferplan QKX1: Lieferplan und Preise des Kunden.",
            ),
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

fn einstellungen() -> Value {
    json!({
        "fassung": 1,
        "klassen": ["oeffentlich", "intern", "kundendaten"],
        "vorgabestufe": "kundendaten",
        "routen": [{"schluessel": "cloud", "art": "cloud", "klasse": "intern"}],
        "kunden": [{"schluessel": "k-nord", "erlaubt": [], "verboten": []}],
        "kundendaten_an_cloud": {"vorgabe": "nie", "freigaben": []}
    })
}

/// A stand-in for Ollama on a free loopback port: every request gets the vector `[1.0, 0.0]`. `aus` closes the port, so
/// that a connect is refused.
struct Ollama {
    url: String,
    stopp: Arc<AtomicBool>,
    aufrufe: Arc<AtomicUsize>,
}

impl Ollama {
    fn an() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("a free port");
        let url = format!(
            "http://127.0.0.1:{}",
            listener.local_addr().expect("addr").port()
        );
        listener.set_nonblocking(true).expect("nonblocking");
        let stopp = Arc::new(AtomicBool::new(false));
        let aufrufe = Arc::new(AtomicUsize::new(0));
        let (halt, zaehler) = (Arc::clone(&stopp), Arc::clone(&aufrufe));
        thread::spawn(move || {
            while !halt.load(Ordering::SeqCst) {
                match listener.accept() {
                    Ok((strom, _)) => {
                        zaehler.fetch_add(1, Ordering::SeqCst);
                        antworten(strom);
                    }
                    Err(_) => thread::sleep(Duration::from_millis(5)),
                }
            }
        });
        Self {
            url,
            stopp,
            aufrufe,
        }
    }

    /// A closed port: bound and dropped at once.
    fn aus() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("a free port");
        let url = format!(
            "http://127.0.0.1:{}",
            listener.local_addr().expect("addr").port()
        );
        drop(listener);
        Self {
            url,
            stopp: Arc::new(AtomicBool::new(true)),
            aufrufe: Arc::new(AtomicUsize::new(0)),
        }
    }
}

impl Drop for Ollama {
    fn drop(&mut self) {
        self.stopp.store(true, Ordering::SeqCst);
    }
}

fn antworten(mut strom: TcpStream) {
    strom.set_nonblocking(false).expect("blocking");
    strom
        .set_read_timeout(Some(Duration::from_secs(5)))
        .expect("timeout");
    let mut gelesen = Vec::new();
    let mut stueck = [0_u8; 4096];
    loop {
        if let Some(ende) = gelesen.windows(4).position(|w| w == b"\r\n\r\n") {
            let kopf = String::from_utf8_lossy(&gelesen[..ende]).to_ascii_lowercase();
            let laenge: usize = kopf
                .lines()
                .find_map(|z| z.strip_prefix("content-length:"))
                .and_then(|v| v.trim().parse().ok())
                .unwrap_or(0);
            if gelesen.len() >= ende + 4 + laenge {
                break;
            }
        }
        match strom.read(&mut stueck) {
            Ok(0) | Err(_) => return,
            Ok(n) => gelesen.extend_from_slice(&stueck[..n]),
        }
    }
    let koerper = r#"{"embeddings": [[1.0, 0.0]]}"#;
    let _ = write!(
        strom,
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{koerper}",
        koerper.len()
    );
}

/// A folder under `/tmp` with the vault, the settings, the log folder and the index database of one test.
struct Welt {
    root: PathBuf,
}

impl Welt {
    /// `vektoren`: the paths of the index database, each with the vector `[1.0, 0.0]`.
    fn new(vektoren: &[&str]) -> Self {
        static ZAEHLER: AtomicUsize = AtomicUsize::new(0);
        let nr = ZAEHLER.fetch_add(1, Ordering::Relaxed);
        let root = PathBuf::from(format!("/tmp/rkr-{}-{nr}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        for ordner in ["home", "protokoll", "aux", "vault"] {
            fs::create_dir_all(root.join(ordner)).expect("folder is made");
        }
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).expect("mode of the folder");
        let welt = Self { root };
        welt.schreiben("aux/einstellungen.json", &einstellungen().to_string());
        for (rel, text) in vault() {
            welt.schreiben(&format!("vault/{rel}"), &text);
        }
        let db = rusqlite::Connection::open(welt.pfad("aux/index.db")).expect("database");
        db.execute(
            "CREATE TABLE embeddings (rel TEXT PRIMARY KEY, hash TEXT NOT NULL, vector TEXT NOT NULL, updated REAL NOT NULL)",
            [],
        )
        .expect("table");
        for (i, rel) in (1_u32..).zip(vektoren) {
            db.execute(
                "INSERT INTO embeddings VALUES (?1, ?2, '[1.0, 0.0]', ?3)",
                rusqlite::params![rel, format!("h{i}"), f64::from(i)],
            )
            .expect("row");
        }
        drop(db);
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

    /// Starts `rangkern sitzung` on the route `cloud` with the index database, then `extra`, and waits for the readiness line.
    fn starten(&self, extra: &[&str]) -> Lauf {
        let p = |rel: &str| self.pfad(rel).to_string_lossy().into_owned();
        let mut args = vec![
            "sitzung".to_owned(),
            p("vault"),
            "--socket".to_owned(),
            self.sock().to_string_lossy().into_owned(),
            "--route".to_owned(),
            "cloud".to_owned(),
            "--sitzung".to_owned(),
            SITZUNG.to_owned(),
            "--einstellungen".to_owned(),
            p("aux/einstellungen.json"),
            "--protokoll".to_owned(),
            p("protokoll"),
            "--index-db".to_owned(),
            p("aux/index.db"),
        ];
        args.extend(extra.iter().map(|s| (*s).to_owned()));
        let mut kind = Command::new(BIN)
            .current_dir(&self.root)
            .env_clear()
            .env("PATH", std::env::var_os("PATH").unwrap_or_default())
            .env("LANG", "C.UTF-8")
            .env("TZ", "UTC")
            .env("HOME", self.pfad("home"))
            .args(&args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("program starts");
        let mut stdout = BufReader::new(kind.stdout.take().expect("stdout is piped"));
        let mut zeile = String::new();
        stdout.read_line(&mut zeile).expect("a line");
        assert_eq!(zeile, "{\"bereit\":true}\n", "readiness line");
        Lauf {
            kind,
            sock: self.sock(),
        }
    }

    /// The lines of the log, in the order of the files and the lines.
    fn protokoll(&self) -> Vec<Value> {
        let mut dateien: Vec<PathBuf> = fs::read_dir(self.pfad("protokoll"))
            .expect("log folder")
            .map(|e| e.expect("entry").path())
            .filter(|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.starts_with("brain-") && !n.contains("anker"))
            })
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

    /// The lines `suche.rueckfall` of the log.
    fn rueckfaelle(&self) -> Vec<Value> {
        self.protokoll()
            .into_iter()
            .filter(|z| z["aktionsname"] == "suche.rueckfall")
            .collect()
    }
}

impl Drop for Welt {
    fn drop(&mut self) {
        let _ = fs::set_permissions(self.pfad("protokoll"), fs::Permissions::from_mode(0o755));
        let _ = fs::remove_dir_all(&self.root);
    }
}

struct Lauf {
    kind: Child,
    sock: PathBuf,
}

impl Lauf {
    fn verbinden(&self) -> Verbindung {
        let strom = UnixStream::connect(&self.sock).expect("the socket accepts");
        strom
            .set_read_timeout(Some(Duration::from_secs(15)))
            .expect("timeout");
        Verbindung {
            lesen: BufReader::new(strom.try_clone().expect("clone")),
            schreiben: strom,
        }
    }

    /// SIGTERM, then the exit code and the whole error output.
    fn beenden(mut self) -> (Option<i32>, String) {
        let status = Command::new("kill")
            .arg("-TERM")
            .arg(self.kind.id().to_string())
            .status()
            .expect("kill runs");
        assert!(status.success());
        let ende = Instant::now() + Duration::from_secs(5);
        let code = loop {
            if let Some(status) = self.kind.try_wait().expect("wait") {
                break status.code();
            }
            assert!(Instant::now() < ende, "the program ends in time");
            thread::sleep(Duration::from_millis(20));
        };
        let mut fehler = String::new();
        if let Some(mut stderr) = self.kind.stderr.take() {
            stderr.read_to_string(&mut fehler).expect("stderr");
        }
        (code, fehler)
    }
}

impl Drop for Lauf {
    fn drop(&mut self) {
        let _ = self.kind.kill();
        let _ = self.kind.wait();
    }
}

struct Verbindung {
    lesen: BufReader<UnixStream>,
    schreiben: UnixStream,
}

impl Verbindung {
    fn roh(&mut self, zeile: &str) -> Value {
        self.schreiben
            .write_all(format!("{zeile}\n").as_bytes())
            .expect("request is written");
        let mut antwort = String::new();
        let n = self.lesen.read_line(&mut antwort).expect("an answer");
        assert!(n > 0, "the connection answers");
        serde_json::from_str(&antwort).expect("an answer in JSON")
    }

    fn senden(&mut self, anfrage: &Value) -> Value {
        self.roh(&anfrage.to_string())
    }

    fn suchen(&mut self, query: &str) -> Value {
        self.senden(&json!({"op": "search", "query": query, "k": 5}))
    }
}

/// The line `suche.rueckfall` with the reason `grund`, checked field by field (`vorher` and `zeit` by their form).
fn ist_rueckfall(zeile: &Value, grund: &str) {
    let felder: Vec<&str> = zeile
        .as_object()
        .expect("an object")
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(
        felder,
        [
            "aktionsname",
            "ergebnis",
            "fassung",
            "grund",
            "route",
            "sitzung",
            "vorher",
            "zeit"
        ],
        "exactly the eight fields: {zeile}"
    );
    assert_eq!(zeile["aktionsname"], "suche.rueckfall");
    assert_eq!(zeile["ergebnis"], "lexikalisch");
    assert_eq!(zeile["fassung"], 2);
    assert_eq!(zeile["grund"], grund);
    assert_eq!(zeile["route"], "cloud");
    assert_eq!(zeile["sitzung"], SITZUNG);
    let vorher = zeile["vorher"].as_str().expect("vorher");
    assert!(vorher.len() == 64 && vorher.bytes().all(|b| b.is_ascii_hexdigit()));
    assert!(zeile["zeit"].as_str().expect("zeit").ends_with('Z'));
}

/// The options of an embedding at `ollama` with a deadline of 300 ms per try.
fn embed(ollama: &Ollama) -> [&str; 4] {
    ["--embed", &ollama.url, "--embed-timeout-ms", "300"]
}

/// The answer without the field `id`.
fn ohne_id(mut antwort: Value) -> Value {
    if let Some(objekt) = antwort.as_object_mut() {
        objekt.remove("id");
    }
    antwort
}

#[test]
fn an_embedding_that_is_off_writes_one_line_per_search_with_no_connection() {
    let welt = Welt::new(&["projekte/lieferplan.md"]);
    let ollama = Ollama::aus();
    let lauf = welt.starten(&embed(&ollama));
    let mut v = lauf.verbinden();
    let antwort = v.suchen("Wetter");
    assert_eq!(antwort["fallback"], true);
    assert_eq!(antwort["gesperrt"], 0);
    assert_eq!(welt.rueckfaelle().len(), 1);
    let antwort = v.suchen("Wetter");
    assert_eq!(antwort["fallback"], true);
    let zeilen = welt.protokoll();
    assert_eq!(zeilen.len(), 2, "one line per answer: {zeilen:?}");
    for zeile in &zeilen {
        ist_rueckfall(zeile, "keine_verbindung");
    }
    let (code, fehler) = lauf.beenden();
    assert_eq!(code, Some(0));
    assert_eq!(fehler, "", "nothing on the error output");
}

#[test]
fn an_embedding_that_answers_writes_no_line() {
    let welt = Welt::new(&["projekte/lieferplan.md"]);
    let ollama = Ollama::an();
    let lauf = welt.starten(&embed(&ollama));
    let mut v = lauf.verbinden();
    let antwort = v.suchen("Wetter");
    assert_eq!(antwort["fallback"], false);
    assert!(
        ollama.aufrufe.load(Ordering::SeqCst) >= 1,
        "Ollama is asked"
    );
    assert_eq!(welt.protokoll(), Vec::<Value>::new());
    let _ = lauf.beenden();
}

#[test]
fn without_embed_the_answer_is_lexical_and_writes_no_line() {
    let welt = Welt::new(&["projekte/lieferplan.md"]);
    let lauf = welt.starten(&[]);
    let mut v = lauf.verbinden();
    let antwort = v.suchen("Wetter");
    assert_eq!(antwort["fallback"], true);
    assert_eq!(welt.protokoll(), Vec::<Value>::new());
    let _ = lauf.beenden();
}

#[test]
fn no_note_vector_in_the_corpus_asks_no_ollama_and_writes_no_line() {
    // The only vector belongs to a note that is not in the vault: Ollama is not asked, the vector is empty.
    let welt = Welt::new(&["weg/verschwunden.md"]);
    let ollama = Ollama::aus();
    let lauf = welt.starten(&embed(&ollama));
    let mut v = lauf.verbinden();
    let antwort = v.suchen("Wetter");
    assert_eq!(antwort["fallback"], false);
    assert_eq!(welt.protokoll(), Vec::<Value>::new());
    let _ = lauf.beenden();
}

#[test]
fn the_line_follows_the_lines_of_the_blocked_hits_of_the_same_answer() {
    let welt = Welt::new(&["projekte/lieferplan.md"]);
    let ollama = Ollama::aus();
    let lauf = welt.starten(&embed(&ollama));
    let mut v = lauf.verbinden();
    let antwort = v.suchen("Lieferplan");
    assert_eq!(antwort["fallback"], true);
    assert_eq!(antwort["gesperrt"], 1);
    let zeilen = welt.protokoll();
    assert_eq!(zeilen.len(), 2, "{zeilen:?}");
    assert_eq!(zeilen[0]["aktionsname"], "ausgabe.gesperrt");
    ist_rueckfall(&zeilen[1], "keine_verbindung");
    let _ = lauf.beenden();
}

#[test]
fn ping_and_an_answer_with_an_error_write_no_line() {
    let welt = Welt::new(&["projekte/lieferplan.md"]);
    let ollama = Ollama::aus();
    let lauf = welt.starten(&embed(&ollama));
    let mut v = lauf.verbinden();
    assert_eq!(
        v.senden(&json!({"op": "ping", "id": 1})),
        json!({"ok": true, "id": 1})
    );
    assert_eq!(
        v.senden(&json!({"op": "search", "query": "Wetter", "route": "cloud"})),
        json!({"error": "bad_request"})
    );
    assert_eq!(v.roh("kein json"), json!({"error": "bad_request"}));
    assert_eq!(welt.protokoll(), Vec::<Value>::new());
    // A blocked hit whose line cannot be written: `{"error":"protokoll"}` and no line `suche.rueckfall` either.
    fs::set_permissions(welt.pfad("protokoll"), fs::Permissions::from_mode(0o500)).expect("mode");
    assert_eq!(
        v.senden(&json!({"op": "search", "query": "Lieferplan", "id": 2})),
        json!({"error": "protokoll", "id": 2})
    );
    fs::set_permissions(welt.pfad("protokoll"), fs::Permissions::from_mode(0o755)).expect("mode");
    assert_eq!(welt.protokoll(), Vec::<Value>::new());
    let _ = lauf.beenden();
}

#[test]
fn a_log_folder_that_cannot_be_written_after_the_start_leaves_the_answer_as_it_is() {
    // The same answer from a process whose log folder can be written.
    let erwartet = {
        let welt = Welt::new(&["projekte/lieferplan.md"]);
        let ollama = Ollama::aus();
        let lauf = welt.starten(&embed(&ollama));
        let antwort = lauf
            .verbinden()
            .senden(&json!({"op": "search", "query": "Wetter", "id": 7}));
        assert_eq!(welt.rueckfaelle().len(), 1);
        let _ = lauf.beenden();
        antwort
    };
    let welt = Welt::new(&["projekte/lieferplan.md"]);
    let ollama = Ollama::aus();
    let lauf = welt.starten(&embed(&ollama));
    fs::set_permissions(welt.pfad("protokoll"), fs::Permissions::from_mode(0o500)).expect("mode");
    let mut v = lauf.verbinden();
    let antwort = v.senden(&json!({"op": "search", "query": "Wetter", "id": 7}));
    assert_eq!(
        antwort, erwartet,
        "the same answer as with a log that can be written"
    );
    assert_eq!(antwort["fallback"], true);
    assert_ne!(antwort["hits"], json!([]), "the answer has hits");
    let zweite = v.senden(&json!({"op": "search", "query": "Wetter", "id": 7}));
    assert_eq!(
        ohne_id(zweite),
        ohne_id(erwartet),
        "no second try changes the next answer"
    );
    fs::set_permissions(welt.pfad("protokoll"), fs::Permissions::from_mode(0o755)).expect("mode");
    assert_eq!(
        welt.protokoll(),
        Vec::<Value>::new(),
        "no line, no buffer written later"
    );
    let (code, fehler) = lauf.beenden();
    assert_eq!(code, Some(0));
    assert_eq!(fehler, "", "nothing on the error output");
}

#[test]
fn the_log_folder_of_a_test_is_below_tmp() {
    // The sockets of these tests need short paths.
    let welt = Welt::new(&[]);
    assert!(welt.sock().starts_with(Path::new("/tmp")));
}
