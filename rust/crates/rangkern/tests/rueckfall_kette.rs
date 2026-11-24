//! Two processes `rangkern sitzung` write into the same chain `brain` at the same time: lines `ausgabe.gesperrt` and
//! `suche.rueckfall`, many searches each. Afterwards the chain is whole: every line carries the sha256 of the line before
//! it as `vorher`, and `kette pruefe-quelle` (the library of the program `kette`) finds no break.
//!
//! Contract 5, section 5.3: the functions with folder and source lock the source, `anhaengen_gesperrt(<file>)` only the file
//! of the month; "beide Wege nie mischen". With the two ways mixed, two lines can get the same `vorher`. The test is a guard
//! for that. Every vault here is invented; the stand-in for Ollama is a closed loopback port.

#![cfg(unix)]

use std::ffi::OsString;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;
use std::time::Duration;

use serde_json::{Value, json};

const BIN: &str = env!("CARGO_BIN_EXE_rangkern");
/// Rounds of the test, each with two fresh processes and an empty log folder.
const RUNDEN: usize = 3;
/// Searches of each process per round, alternately with and without a blocked hit.
const SUCHEN: usize = 60;

fn notiz(titel: &str, front: &str, text: &str) -> String {
    format!("---\ntitle: \"{titel}\"\ntype: \"note\"\n{front}---\n\n{text}\n")
}

/// A folder under `/tmp`: vault, settings, an index database and one log folder for both processes.
struct Welt {
    root: PathBuf,
}

impl Welt {
    fn new() -> Self {
        static ZAEHLER: AtomicUsize = AtomicUsize::new(0);
        let nr = ZAEHLER.fetch_add(1, Ordering::Relaxed);
        let root = PathBuf::from(format!("/tmp/rkk-{}-{nr}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        for ordner in ["home", "protokoll", "aux", "vault"] {
            fs::create_dir_all(root.join(ordner)).expect("folder is made");
        }
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).expect("mode of the folder");
        let welt = Self { root };
        welt.schreiben(
            "aux/einstellungen.json",
            &json!({
                "fassung": 1,
                "klassen": ["oeffentlich", "intern", "kundendaten"],
                "vorgabestufe": "kundendaten",
                "routen": [{"schluessel": "cloud", "art": "cloud", "klasse": "intern"}],
                "kunden": [{"schluessel": "k-nord", "erlaubt": [], "verboten": []}],
                "kundendaten_an_cloud": {"vorgabe": "nie", "freigaben": []}
            })
            .to_string(),
        );
        welt.schreiben(
            "vault/projekte/lieferplan.md",
            &notiz(
                "Lieferplan Lager",
                "datenklasse: \"intern\"\n",
                "Der Lieferplan regelt die Termine.",
            ),
        );
        welt.schreiben(
            "vault/kunden/nord/lieferplan-qkx1.md",
            &notiz(
                "Lieferplan QKX1",
                "datenklasse: \"kundendaten\"\nkunde: \"k-nord\"\n",
                "Lieferplan QKX1 des Kunden.",
            ),
        );
        welt.schreiben(
            "vault/sonst/wetter.md",
            &notiz("Wetter", "datenklasse: \"intern\"\n", "Sonne und Regen."),
        );
        let db = rusqlite::Connection::open(welt.pfad("aux/index.db")).expect("database");
        db.execute(
            "CREATE TABLE embeddings (rel TEXT PRIMARY KEY, hash TEXT NOT NULL, vector TEXT NOT NULL, updated REAL NOT NULL)",
            [],
        )
        .expect("table");
        db.execute(
            "INSERT INTO embeddings VALUES ('projekte/lieferplan.md', 'h0', '[1.0, 0.0]', 1.0)",
            [],
        )
        .expect("row");
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

    /// Starts one process with its own socket and session on the shared log folder; waits for the readiness line.
    fn starten(&self, nr: usize, ollama: &str) -> Lauf {
        let p = |rel: &str| self.pfad(rel).to_string_lossy().into_owned();
        let sock = self.pfad(&format!("s{nr}.sock"));
        let mut kind = Command::new(BIN)
            .current_dir(&self.root)
            .env_clear()
            .env("PATH", std::env::var_os("PATH").unwrap_or_default())
            .env("LANG", "C.UTF-8")
            .env("TZ", "UTC")
            .env("HOME", self.pfad("home"))
            .args([
                "sitzung".to_owned(),
                p("vault"),
                "--socket".to_owned(),
                sock.to_string_lossy().into_owned(),
                "--route".to_owned(),
                "cloud".to_owned(),
                "--sitzung".to_owned(),
                format!("s-kette-{nr}"),
                "--einstellungen".to_owned(),
                p("aux/einstellungen.json"),
                "--protokoll".to_owned(),
                p("protokoll"),
                "--index-db".to_owned(),
                p("aux/index.db"),
                "--embed".to_owned(),
                ollama.to_owned(),
                "--embed-timeout-ms".to_owned(),
                "300".to_owned(),
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("program starts");
        let mut stdout = BufReader::new(kind.stdout.take().expect("stdout is piped"));
        let mut zeile = String::new();
        stdout.read_line(&mut zeile).expect("a line");
        assert_eq!(zeile, "{\"bereit\":true}\n", "readiness line");
        Lauf { kind, sock }
    }

    /// The lines of the months of the chain `brain`, as bytes, in the order of the files.
    fn zeilen(&self) -> Vec<Vec<u8>> {
        let mut dateien: Vec<PathBuf> = fs::read_dir(self.pfad("protokoll"))
            .expect("log folder")
            .map(|e| e.expect("entry").path())
            .filter(|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.starts_with("brain-"))
            })
            .collect();
        dateien.sort();
        dateien
            .iter()
            .flat_map(|d| {
                fs::read(d)
                    .expect("log file")
                    .split(|b| *b == b'\n')
                    .filter(|z| !z.is_empty())
                    .map(<[u8]>::to_vec)
                    .collect::<Vec<_>>()
            })
            .collect()
    }
}

impl Drop for Welt {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

struct Lauf {
    kind: Child,
    sock: PathBuf,
}

impl Drop for Lauf {
    fn drop(&mut self) {
        let _ = self.kind.kill();
        let _ = self.kind.wait();
    }
}

/// `SUCHEN` searches on one connection, alternately with a blocked hit (two lines) and without (one line). Returns the
/// answers that carry an error.
fn suchen(sock: &Path) -> Vec<Value> {
    let strom = UnixStream::connect(sock).expect("the socket accepts");
    strom
        .set_read_timeout(Some(Duration::from_secs(15)))
        .expect("timeout");
    let mut lesen = BufReader::new(strom.try_clone().expect("clone"));
    let mut schreiben = strom;
    let mut fehler = Vec::new();
    for i in 0..SUCHEN {
        let query = if i % 2 == 0 { "Lieferplan" } else { "Wetter" };
        let anfrage = json!({"op": "search", "query": query, "k": 5});
        schreiben
            .write_all(format!("{anfrage}\n").as_bytes())
            .expect("request is written");
        let mut zeile = String::new();
        assert!(lesen.read_line(&mut zeile).expect("an answer") > 0);
        let antwort: Value = serde_json::from_str(&zeile).expect("JSON");
        if antwort.get("error").is_some() || antwort["fallback"] != true {
            fehler.push(antwort);
        }
    }
    fehler
}

/// A loopback address with no listener: every connect is refused, so every search falls back.
fn ollama_aus() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("a free port");
    let url = format!(
        "http://127.0.0.1:{}",
        listener.local_addr().expect("addr").port()
    );
    drop(listener);
    url
}

/// One round: two processes, their searches at the same time, then the whole chain is checked.
fn runde(nr: usize) {
    let welt = Welt::new();
    let ollama = ollama_aus();
    let laeufe = [welt.starten(1, &ollama), welt.starten(2, &ollama)];
    let faeden: Vec<_> = laeufe
        .iter()
        .map(|lauf| {
            let sock = lauf.sock.clone();
            thread::spawn(move || suchen(&sock))
        })
        .collect();
    for faden in faeden {
        assert_eq!(
            faden.join().expect("the searches end"),
            Vec::<Value>::new(),
            "round {nr}: every answer falls back without an error"
        );
    }
    drop(laeufe);
    let zeilen = welt.zeilen();
    // Per process: SUCHEN / 2 searches with a blocked hit (two lines) and SUCHEN / 2 without (one line).
    assert_eq!(
        zeilen.len(),
        2 * (SUCHEN / 2 * 2 + SUCHEN / 2),
        "round {nr}"
    );
    let mut vorher = formate::kette::NULL_HASH.to_owned();
    for (i, zeile) in zeilen.iter().enumerate() {
        let objekt: Value = serde_json::from_slice(zeile).expect("a JSON line");
        assert_eq!(
            objekt["vorher"],
            vorher,
            "round {nr}, line {}: the chain breaks ({})",
            i + 1,
            objekt["aktionsname"]
        );
        vorher = formate::kette::sha256_hex(zeile);
    }
    let rueckfaelle = zeilen
        .iter()
        .filter(|z| z.windows(17).any(|w| w == b"\"suche.rueckfall\""))
        .count();
    assert_eq!(rueckfaelle, 2 * SUCHEN, "round {nr}: one line per answer");
    let ausgang = formate::kette::befehl(&[
        OsString::from("pruefe-quelle"),
        welt.pfad("protokoll").into_os_string(),
        OsString::from("brain"),
    ]);
    assert_eq!(
        ausgang.exit, 0,
        "round {nr}: kette pruefe-quelle {}",
        ausgang.stdout
    );
}

#[test]
fn two_processes_on_one_log_folder_keep_the_chain_whole() {
    for nr in 0..RUNDEN {
        runde(nr);
    }
}
