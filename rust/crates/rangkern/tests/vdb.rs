//! The note vectors straight from the databases, against the vector file.
//!
//! - `search --index-db` against `search --vectors F`, where `F` holds what the model's
//!   `load_merged_embeddings` returns for the same databases (written here from an independent
//!   model of the merge): 600 invented states of two databases (overlap, order, a missing second
//!   database, relics, paths of excluded folders, damaged files, repeated paths), output and exit
//!   code equal to the byte.
//! - a warm server against a fresh read after random changes of the databases, on three clocks.
//! - the program on pipes: a second process writes into the database right after every answer, and
//!   no file appears in the folder of the databases.
//!
//! Every vault and every database here is invented; nothing reads a real one.

// Permissions and process handling here need Unix; the program itself builds for Windows too.
#![cfg(unix)]

use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread::sleep;
use std::time::Duration;

use rangkern::hit::SearchHit;
use rangkern::search::{Params, search};
use rangkern::serve::{SearchRequest, Server};
use rangkern::stamp::{Detect, now_ns};
use rangkern::vault::Vectors;
use rangkern::vdb::{DbPaths, load_merged};
use rusqlite::Connection;
use serde_json::{Value, json};

/// A small deterministic generator (xorshift64*), so that a failure names its seed.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn below(&mut self, bound: usize) -> usize {
        usize::try_from(self.next() % u64::try_from(bound).expect("fits")).expect("fits")
    }

    fn chance(&mut self, percent: usize) -> bool {
        self.below(100) < percent
    }

    fn pick<'a, T>(&mut self, list: &'a [T]) -> &'a T {
        &list[self.below(list.len())]
    }

    fn shuffle<T>(&mut self, list: &mut [T]) {
        for i in (1..list.len()).rev() {
            list.swap(i, self.below(i + 1));
        }
    }
}

/// The notes of the invented vault.
const NOTES: [(&str, &str); 7] = [
    ("a.md", "---\ntitle: Garten\n---\nBeet und Gartenarbeit\n"),
    ("b.md", "Kaffee Beet Beet Kaffeemaschine\n"),
    (
        "c.md",
        "---\ntitle: Wasserfall\n---\nWasserfall und Berge\n",
    ),
    ("sub/d.md", "Bergwanderung im Garten\nBeet\n"),
    ("e.md", "Entscheidung zur Aufgabe\n"),
    (
        "f.md",
        "---\nclass: derived\n---\nBeet Kaffee Zusammenfassung\n",
    ),
    ("90-secrets/geheim.md", "Beet Kaffee geheim\n"),
];

/// Paths that a database can hold: the notes, an excluded one, and paths without a file (relics).
const RELS: [&str; 10] = [
    "a.md",
    "b.md",
    "c.md",
    "sub/d.md",
    "e.md",
    "f.md",
    "90-secrets/geheim.md",
    "gone.md",
    "sub/x.md",
    "90-secrets/x.md",
];

const QUERIES: [&str; 8] = [
    "beet",
    "garten",
    "kaffee",
    "beet kaffee",
    "wasserfall berge",
    "zzz",
    "entscheidung",
    "der die das",
];

const SCHEMA: &str = "CREATE TABLE embeddings (rel TEXT PRIMARY KEY, hash TEXT NOT NULL, \
                      vector TEXT NOT NULL, updated REAL NOT NULL)";
/// The same without the key, so that a path can repeat in one database.
const LOOSE_SCHEMA: &str =
    "CREATE TABLE embeddings (rel TEXT, hash TEXT, vector TEXT, updated REAL)";

struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("vdb-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("vault/sub")).expect("vault is created");
        fs::create_dir_all(root.join("vault/90-secrets")).expect("vault is created");
        fs::create_dir_all(root.join("state")).expect("state is created");
        for (rel, text) in NOTES {
            fs::write(root.join("vault").join(rel), text).expect("note is written");
        }
        Self(root)
    }

    fn vault(&self) -> String {
        self.0.join("vault").to_string_lossy().into_owned()
    }

    fn index(&self) -> PathBuf {
        self.0.join("state/gardener.db")
    }

    fn extra(&self) -> PathBuf {
        self.0.join("state/extra-embeddings.db")
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// The text of one number between -1 and 1 in one of several spellings.
fn number(rng: &mut Rng) -> String {
    let value = i32::try_from(rng.below(2001)).expect("small") - 1000;
    let value = f64::from(value) / 1000.0;
    match rng.below(5) {
        0 => format!("{value:.3}"),
        1 => format!("{value}"),
        2 => format!("{value:e}"),
        3 => format!("{:.1}", value.round()),
        _ => format!("{value:.6}"),
    }
}

/// The text of a vector of `dim` numbers with some spacing.
fn vector_text(rng: &mut Rng, dim: usize) -> String {
    let numbers: Vec<String> = (0..dim).map(|_| number(rng)).collect();
    match rng.below(3) {
        0 => format!("[{}]", numbers.join(",")),
        1 => format!("[{}]", numbers.join(", ")),
        _ => format!(" [ {} ]\n", numbers.join(" , ")),
    }
}

/// `(path, text of the vector)` rows of one database in the order they are inserted: some paths of
/// `RELS`, and sometimes a repeated one when `repeat` is set.
fn rows(rng: &mut Rng, dim: usize, repeat: bool) -> Vec<(String, String)> {
    let mut paths: Vec<&str> = RELS.to_vec();
    rng.shuffle(&mut paths);
    let count = rng.below(RELS.len() + 1);
    let mut rows: Vec<(String, String)> = paths[..count]
        .iter()
        .map(|rel| ((*rel).to_string(), vector_text(rng, dim)))
        .collect();
    if repeat && !rows.is_empty() && rng.chance(60) {
        let again = rows[rng.below(rows.len())].0.clone();
        rows.push((again, vector_text(rng, dim)));
    }
    rows
}

/// Writes a database with the rows in this order.
fn write_db(path: &Path, rows: &[(String, String)], keyed: bool) {
    let _ = fs::remove_file(path);
    let connection = Connection::open(path).expect("database is created");
    connection
        .pragma_update(None, "synchronous", "OFF")
        .expect("pragma");
    connection
        .execute_batch(&format!(
            "{}; BEGIN;",
            if keyed { SCHEMA } else { LOOSE_SCHEMA }
        ))
        .expect("table is created");
    for (rel, vector) in rows {
        connection
            .execute(
                "INSERT INTO embeddings VALUES (?1, 'h', ?2, 0.0)",
                (rel, vector),
            )
            .expect("row is written");
    }
    connection.execute_batch("COMMIT").expect("committed");
}

/// What `load_merged_embeddings` makes of two row lists: a dict from the first, updated by the
/// second (a path keeps its place, a new path goes last). The texts of the vectors stay as written.
fn model_merge(index: &[(String, String)], extra: &[(String, String)]) -> Vec<(String, String)> {
    let mut merged: Vec<(String, String)> = Vec::new();
    for (rel, vector) in index.iter().chain(extra) {
        match merged.iter_mut().find(|(have, _)| have == rel) {
            Some(entry) => entry.1.clone_from(vector),
            None => merged.push((rel.clone(), vector.clone())),
        }
    }
    merged
}

fn call(args: &[String]) -> (u8, String) {
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = rangkern::run(args, &mut out, &mut err);
    (code, String::from_utf8(out).expect("UTF-8"))
}

fn strings(args: &[&str]) -> Vec<String> {
    args.iter().map(ToString::to_string).collect()
}

/// How a state of the databases is damaged, if at all.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Damage {
    None,
    /// A vector that is no list of numbers.
    BadVector,
    /// A first database that is no database.
    Garbage,
    /// A first database without the table.
    NoTable,
    /// No first database.
    NoIndex,
    /// A second database that is no database.
    BadExtra,
}

#[test]
#[allow(clippy::too_many_lines)]
// One flat generator of the states of the databases and the two calls to compare; split up, the
// seed of a failing state would be harder to follow.
fn search_with_the_databases_equals_search_with_the_vector_file_in_600_states() {
    let scratch = Scratch::new("states");
    let mut rng = Rng(0x5EED_0000_D00D_F00D);
    let unreadable = (3_u8, "{\"error\":\"unreadable\"}\n".to_string());
    let mut hybrid = 0;
    let mut errors = 0;
    for state in 0..600 {
        let dim = 3 + rng.below(3);
        let keyed = rng.chance(75);
        let mut index_rows = rows(&mut rng, dim, !keyed);
        let mut extra_rows = rows(&mut rng, dim, !keyed);
        // Overlap on purpose, in a different order and with another vector.
        if rng.chance(50) && !index_rows.is_empty() {
            let (rel, _) = rng.pick(&index_rows).clone();
            extra_rows.insert(
                rng.below(extra_rows.len() + 1),
                (rel, vector_text(&mut rng, dim)),
            );
            if keyed {
                let mut seen = Vec::new();
                extra_rows.retain(|(rel, _)| {
                    let new = !seen.contains(rel);
                    seen.push(rel.clone());
                    new
                });
            }
        }
        let has_extra = rng.chance(70);
        let damage = match rng.below(20) {
            0 => Damage::BadVector,
            1 => Damage::Garbage,
            2 => Damage::NoTable,
            3 => Damage::NoIndex,
            4 => Damage::BadExtra,
            _ => Damage::None,
        };
        if damage == Damage::BadVector {
            let bad = *rng.pick(&["[1, 2", "nonsense", "[NaN]", "{\"a\": 1}", ""]);
            index_rows.push(("b.md".to_string(), bad.to_string()));
        }
        // The keyed table cannot hold a repeated path of the bad row: a loose one can.
        let keyed_index = keyed && damage != Damage::BadVector;
        write_db(&scratch.index(), &index_rows, keyed_index);
        match damage {
            Damage::Garbage => {
                fs::write(scratch.index(), b"garbage, not a database, but long enough")
                    .expect("written");
            }
            Damage::NoTable => {
                let _ = fs::remove_file(scratch.index());
                Connection::open(scratch.index())
                    .expect("created")
                    .execute("CREATE TABLE other (rel TEXT)", [])
                    .expect("table");
            }
            Damage::NoIndex => fs::remove_file(scratch.index()).expect("removed"),
            _ => {}
        }
        if has_extra {
            write_db(&scratch.extra(), &extra_rows, keyed);
            if damage == Damage::BadExtra {
                fs::write(scratch.extra(), b"garbage, not a database, but long enough")
                    .expect("written");
            }
        } else {
            let _ = fs::remove_file(scratch.extra());
        }
        let query_vector = vector_text(&mut rng, dim);
        fs::write(scratch.0.join("q.json"), &query_vector).expect("written");
        let readable = matches!(damage, Damage::None) || (damage == Damage::BadExtra && !has_extra);
        let expected_file = readable.then(|| {
            let merged = model_merge(&index_rows, if has_extra { &extra_rows } else { &[] });
            let notes: Vec<String> = merged
                .iter()
                .map(|(rel, vector)| format!("\"{rel}\": {vector}"))
                .collect();
            let text = format!(
                "{{\"query\": {query_vector}, \"notes\": {{{}}}}}",
                notes.join(", ")
            );
            fs::write(scratch.0.join("f.json"), text).expect("written");
            scratch.0.join("f.json")
        });
        let query = *rng.pick(&QUERIES);
        let k = rng.pick(&["1", "3", "5", "8"]).to_string();
        let today = rng.pick(&["2026-11-18", "2027-03-01"]).to_string();
        let mut args = strings(&[
            "search",
            &scratch.vault(),
            query,
            "-k",
            &k,
            "--today",
            &today,
        ]);
        let mut db_args = args.clone();
        db_args.extend(strings(&[
            "--index-db",
            &scratch.index().to_string_lossy(),
            "--query-vector",
            &scratch.0.join("q.json").to_string_lossy(),
        ]));
        if has_extra || rng.chance(30) {
            db_args.extend(strings(&["--extra-db", &scratch.extra().to_string_lossy()]));
        }
        if let Some(file) = &expected_file {
            args.extend(strings(&["--vectors", &file.to_string_lossy()]));
        }
        let with_db = call(&db_args);
        let (state_note, expected) = if expected_file.is_some() {
            hybrid += 1;
            ("hybrid", call(&args))
        } else {
            errors += 1;
            ("unreadable", unreadable.clone())
        };
        assert_eq!(
            with_db, expected,
            "state {state} ({state_note}, {damage:?}, extra {has_extra}, keyed {keyed}) query {query:?}"
        );
        if state_note == "hybrid" {
            assert_eq!(with_db.0, 0);
            assert!(with_db.1.contains("\"fallback\":false"), "{}", with_db.1);
        }
        // The same question without the vector: the databases are not read, damaged or not.
        let mut lexical = args.clone();
        lexical.truncate(7);
        let mut lexical_db = lexical.clone();
        lexical_db.extend(strings(&["--index-db", &scratch.index().to_string_lossy()]));
        let plain = call(&lexical);
        assert_eq!(call(&lexical_db), plain, "state {state}");
        assert!(plain.1.contains("\"fallback\":true"));
    }
    assert!(hybrid >= 400, "{hybrid} readable states");
    assert!(errors >= 100, "{errors} unreadable states");
}

/// What `rangkern search` gives for the request with the databases as they are now.
fn fresh(scratch: &Scratch, request: &SearchRequest) -> Result<(Vec<SearchHit>, bool), ()> {
    let vectors = match &request.query_vector {
        Some(query) => {
            let paths = DbPaths {
                index: scratch.index(),
                extra: Some(scratch.extra()),
            };
            Some(Vectors {
                query: query.clone(),
                notes: load_merged(&paths).map_err(|_| ())?,
            })
        }
        None => None,
    };
    let params = Params {
        k: request.k,
        vectors: vectors.as_ref().map(Vectors::view),
        space: request.space.as_deref(),
        today: request.today.as_deref().expect("the test gives a date"),
    };
    search(Path::new(&scratch.vault()), &request.query, &params).map_err(|_| ())
}

fn random_request(rng: &mut Rng) -> SearchRequest {
    let dim = 3 + rng.below(3);
    SearchRequest {
        query: (*rng.pick(&QUERIES)).to_string(),
        k: *rng.pick(&[1, 3, 5, 8]),
        space: None,
        today: Some("2026-11-18".to_string()),
        query_vector: rng.chance(80).then(|| {
            (0..dim)
                .map(|_| f64::from(i32::try_from(rng.below(2001)).expect("small") - 1000) / 1000.0)
                .collect()
        }),
    }
}

/// One random change of the databases.
fn mutate(scratch: &Scratch, rng: &mut Rng, dim: usize, log: &mut Vec<String>) {
    let which = if rng.chance(50) {
        scratch.index()
    } else {
        scratch.extra()
    };
    let entry = match rng.below(100) {
        0..=34 => {
            // Rows written or written again.
            if which.exists()
                && let Ok(connection) = Connection::open(&which)
            {
                for _ in 0..=rng.below(3) {
                    let rel = *rng.pick(&RELS);
                    let text = vector_text(rng, dim);
                    let _ = connection.execute(
                        "INSERT OR REPLACE INTO embeddings VALUES (?1, 'h', ?2, 0.0)",
                        (rel, &text),
                    );
                }
            }
            format!("rows {}", which.display())
        }
        35..=49 => {
            if which.exists()
                && let Ok(connection) = Connection::open(&which)
            {
                let _ = connection
                    .execute("DELETE FROM embeddings WHERE rel = ?1", (*rng.pick(&RELS),));
            }
            format!("delete row {}", which.display())
        }
        50..=64 => {
            let new = rows(rng, dim, false);
            write_db(&which, &new, true);
            format!("new database {}", which.display())
        }
        65..=74 => {
            let _ = fs::remove_file(&which);
            format!("remove {}", which.display())
        }
        75..=84 => {
            // Replaced by another file (a new inode).
            let new = scratch.0.join("state/replacement.db");
            write_db(&new, &rows(rng, dim, false), true);
            let _ = fs::rename(&new, &which);
            format!("replace {}", which.display())
        }
        85..=92 => {
            let _ = fs::write(&which, b"garbage, not a database, but long enough");
            format!("garbage {}", which.display())
        }
        _ => "no change".to_string(),
    };
    log.push(entry);
}

fn run_server(name: &str, seed: u64, steps: usize, detect: Detect, settled: bool) {
    let scratch = Scratch::new(name);
    let mut rng = Rng(seed);
    let dim = 3 + rng.below(3);
    write_db(&scratch.index(), &rows(&mut rng, dim, false), true);
    write_db(&scratch.extra(), &rows(&mut rng, dim, false), true);
    let mut server = Server::start_dbs_with(
        PathBuf::from(scratch.vault()),
        DbPaths {
            index: scratch.index(),
            extra: Some(scratch.extra()),
        },
        detect,
    );
    let mut log: Vec<String> = Vec::new();
    for step in 0..steps {
        if settled {
            sleep(Duration::from_millis(12));
        }
        mutate(&scratch, &mut rng, dim, &mut log);
        for _ in 0..2 {
            let request = random_request(&mut rng);
            let now = if settled {
                now_ns() + 10_000_000_000
            } else {
                now_ns()
            };
            let warm = server.search_hits(&request, now).map_err(|_| ());
            let cold = fresh(&scratch, &request);
            assert!(
                warm == cold,
                "seed {seed}, step {step}, request {request:?}\nlast changes: {:#?}\nserver: {warm:#?}\nsearch: {cold:#?}",
                &log[log.len().saturating_sub(6)..]
            );
        }
    }
}

#[test]
fn server_equals_a_fresh_read_after_random_changes_of_the_databases_1() {
    run_server("s1", 0x9E37_79B9_7F4A_7C15, 400, Detect::Metadata, false);
}

#[test]
fn server_equals_a_fresh_read_after_random_changes_of_the_databases_2() {
    run_server("s2", 0xD1B5_4A32_D192_ED03, 400, Detect::Metadata, false);
}

#[test]
fn server_equals_a_fresh_read_with_every_stamp_trusted() {
    run_server(
        "settled",
        0xA5A5_1234_FEED_BEEF,
        300,
        Detect::Metadata,
        true,
    );
}

#[test]
fn server_equals_a_fresh_read_when_changes_are_found_by_content() {
    run_server(
        "content",
        0x0BAD_CAFE_1357_9BDF,
        400,
        Detect::Content,
        false,
    );
}

/// The program `rangkern serve` on pipes.
struct Serve {
    child: Arc<Mutex<Child>>,
    stdin: Option<ChildStdin>,
    stdout: BufReader<ChildStdout>,
}

impl Serve {
    fn start(root: &Path, args: &[&str]) -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_rangkern"))
            .current_dir(root)
            .env_clear()
            .env("PATH", std::env::var_os("PATH").unwrap_or_default())
            .env("LANG", "C.UTF-8")
            .env("TZ", "UTC")
            .env("HOME", root.join("home"))
            .arg("serve")
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("program starts");
        let stdin = child.stdin.take();
        let stdout = BufReader::new(child.stdout.take().expect("stdout is piped"));
        // A program that does not answer would block the test for ever: the watchdog ends it.
        let child = Arc::new(Mutex::new(child));
        let watched = Arc::clone(&child);
        std::thread::spawn(move || {
            sleep(Duration::from_secs(60));
            let _ = watched.lock().map(|mut child| child.kill());
        });
        Self {
            child,
            stdin,
            stdout,
        }
    }

    fn ask(&mut self, request: &Value) -> Value {
        let stdin = self.stdin.as_mut().expect("stdin is open");
        stdin
            .write_all(format!("{request}\n").as_bytes())
            .expect("request is written");
        stdin.flush().expect("request is flushed");
        let mut line = String::new();
        let count = self.stdout.read_line(&mut line).expect("answer is read");
        assert!(count > 0, "the program ended without an answer");
        serde_json::from_str(&line).expect("answer is JSON")
    }
}

impl Drop for Serve {
    fn drop(&mut self) {
        self.stdin = None;
        if let Ok(mut child) = self.child.lock() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

fn names(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(dir)
        .expect("folder is listed")
        .map(|entry| {
            entry
                .expect("entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    names.sort();
    names
}

#[test]
fn a_second_process_can_write_right_after_every_answer_and_no_file_appears() {
    let scratch = Scratch::new("process");
    fs::create_dir_all(scratch.0.join("home")).expect("home");
    write_db(
        &scratch.index(),
        &[("a.md".to_string(), "[1, 0]".to_string())],
        true,
    );
    write_db(
        &scratch.extra(),
        &[("b.md".to_string(), "[0, 1]".to_string())],
        true,
    );
    let folder = scratch.0.join("state");
    let before = names(&folder);
    assert_eq!(before, ["extra-embeddings.db", "gardener.db"]);
    let index = scratch.index();
    let extra = scratch.extra();
    let mut serve = Serve::start(
        &scratch.0,
        &[
            &scratch.vault(),
            "--index-db",
            &index.to_string_lossy(),
            "--extra-db",
            &extra.to_string_lossy(),
        ],
    );
    let request = json!({"op": "search", "query": "zzz", "k": 8, "today": "2026-11-18", "query_vector": [0, 1]});
    let first = serve.ask(&request);
    assert_eq!(first["fallback"], json!(false));
    assert_eq!(first["hits"][0]["rel"], json!("b.md"));
    fs::write(scratch.0.join("q.json"), "[0, 1]").expect("written");
    for round in 0..12 {
        // A writer that does not wait: a lock left by the program would be `SQLITE_BUSY`.
        let target = if round % 2 == 0 { &index } else { &extra };
        let writer = Connection::open(target).expect("opens");
        writer.busy_timeout(Duration::ZERO).expect("set");
        let vector = if round % 4 < 2 { "[1, 0]" } else { "[0, 1]" };
        let rel = if round % 3 == 0 { "a.md" } else { "b.md" };
        writer
            .execute_batch(&format!(
                "BEGIN EXCLUSIVE; INSERT OR REPLACE INTO embeddings VALUES ('{rel}', 'h', '{vector}', 0.0); COMMIT;"
            ))
            .expect("the writer is not held up");
        drop(writer);
        let answer = serve.ask(&request);
        assert_eq!(answer["fallback"], json!(false), "round {round}");
        // The same as a fresh program on the databases as they are now.
        let (code, out) = call(&strings(&[
            "search",
            &scratch.vault(),
            "zzz",
            "-k",
            "8",
            "--today",
            "2026-11-18",
            "--index-db",
            &index.to_string_lossy(),
            "--extra-db",
            &extra.to_string_lossy(),
            "--query-vector",
            &scratch.0.join("q.json").to_string_lossy(),
        ]));
        assert_eq!(code, 0);
        assert_eq!(
            answer,
            serde_json::from_str::<Value>(&out).expect("JSON"),
            "round {round}"
        );
        assert_eq!(names(&folder), before, "round {round}");
    }
}
