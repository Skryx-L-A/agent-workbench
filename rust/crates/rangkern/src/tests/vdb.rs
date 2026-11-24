//! The note vectors from the databases: the merge, what is unreadable, no new file, the state of
//! `serve` (read again only when a stamp changed or is in doubt), and the options of the commands.

use std::fs;
use std::path::Path;

use rusqlite::Connection;
use serde_json::{Value, json};

use super::TempDir;
use crate::cli::run;
use crate::stamp::{Detect, now_ns};
use crate::vault::Unreadable;
use crate::vdb::{DbPaths, DbVectors, load_db_vectors, load_merged};

const SCHEMA: &str = "CREATE TABLE embeddings (rel TEXT PRIMARY KEY, hash TEXT NOT NULL, \
                      vector TEXT NOT NULL, updated REAL NOT NULL)";

/// Writes a database with the table of the model and the rows in this order.
fn make_db(path: &Path, rows: &[(&str, &str)]) {
    let connection = Connection::open(path).expect("database is created");
    connection.execute(SCHEMA, []).expect("table is created");
    put(&connection, rows);
}

fn put(connection: &Connection, rows: &[(&str, &str)]) {
    for (rel, vector) in rows {
        connection
            .execute(
                "INSERT OR REPLACE INTO embeddings VALUES (?1, 'h', ?2, 0.0)",
                (rel, vector),
            )
            .expect("row is written");
    }
}

/// Writes rows into an existing database.
fn add_rows(path: &Path, rows: &[(&str, &str)]) {
    put(&Connection::open(path).expect("database opens"), rows);
}

fn merged(index: &Path, extra: Option<&Path>) -> Result<Vec<(String, Vec<f64>)>, Unreadable> {
    load_merged(&DbPaths {
        index: index.to_path_buf(),
        extra: extra.map(Path::to_path_buf),
    })
}

fn entry(rel: &str, vector: &[f64]) -> (String, Vec<f64>) {
    (rel.to_string(), vector.to_vec())
}

/// The names of the files and folders in `dir`, sorted.
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
fn the_rows_of_one_database_come_in_the_order_sqlite_gives_them() {
    let dir = TempDir::new();
    let index = dir.path().join("gardener.db");
    make_db(
        &index,
        &[("b.md", "[0, 1]"), ("a.md", "[1.5, -2e0]"), ("c.md", "[]")],
    );
    assert_eq!(
        merged(&index, None).expect("readable"),
        [
            entry("b.md", &[0.0, 1.0]),
            entry("a.md", &[1.5, -2.0]),
            entry("c.md", &[]),
        ]
    );
}

#[test]
fn the_second_database_wins_on_a_shared_path_and_keeps_the_place_and_new_paths_go_last() {
    let dir = TempDir::new();
    let index = dir.path().join("gardener.db");
    let extra = dir.path().join("extra.db");
    make_db(&index, &[("a.md", "[1]"), ("b.md", "[2]"), ("c.md", "[3]")]);
    make_db(
        &extra,
        &[("z.md", "[9]"), ("b.md", "[20]"), ("y.md", "[8]")],
    );
    assert_eq!(
        merged(&index, Some(&extra)).expect("readable"),
        [
            entry("a.md", &[1.0]),
            entry("b.md", &[20.0]),
            entry("c.md", &[3.0]),
            entry("z.md", &[9.0]),
            entry("y.md", &[8.0]),
        ]
    );
}

#[test]
fn a_missing_second_database_is_empty_and_a_second_one_that_is_not_readable_is_not() {
    let dir = TempDir::new();
    let index = dir.path().join("gardener.db");
    make_db(&index, &[("a.md", "[1]")]);
    let missing = dir.path().join("none.db");
    assert_eq!(
        merged(&index, Some(&missing)).expect("readable"),
        [entry("a.md", &[1.0])]
    );
    assert_eq!(
        merged(&index, None).expect("readable"),
        [entry("a.md", &[1.0])]
    );
    assert!(!missing.exists(), "reading creates no database");
    let garbage = dir.write(
        "extra.db",
        b"this is no database, and long enough to be one page",
    );
    assert!(merged(&index, Some(&garbage)).is_err());
    let folder = dir.path().join("folder.db");
    fs::create_dir(&folder).expect("folder");
    assert!(merged(&index, Some(&folder)).is_err());
}

#[test]
fn the_first_database_must_be_a_database_with_the_table_and_both_columns() {
    let dir = TempDir::new();
    let path = |name: &str| dir.path().join(name);
    assert!(merged(&path("missing.db"), None).is_err());
    assert!(!path("missing.db").exists());
    assert!(merged(dir.path(), None).is_err());
    let garbage = dir.write(
        "garbage.db",
        b"SQLite? no. Just text that fills a page or so.",
    );
    assert!(merged(&garbage, None).is_err());
    // An empty file is an empty database: no table.
    let empty = dir.write("empty.db", b"");
    assert!(merged(&empty, None).is_err());
    let other = path("other.db");
    Connection::open(&other)
        .expect("created")
        .execute("CREATE TABLE notes (rel TEXT, vector TEXT)", [])
        .expect("table");
    assert!(merged(&other, None).is_err());
    let no_vector = path("novector.db");
    Connection::open(&no_vector)
        .expect("created")
        .execute("CREATE TABLE embeddings (rel TEXT, hash TEXT)", [])
        .expect("table");
    assert!(merged(&no_vector, None).is_err());
    let no_rel = path("norel.db");
    Connection::open(&no_rel)
        .expect("created")
        .execute("CREATE TABLE embeddings (name TEXT, vector TEXT)", [])
        .expect("table");
    assert!(merged(&no_rel, None).is_err());
    // An empty table is readable and has no vectors.
    let nothing = path("nothing.db");
    make_db(&nothing, &[]);
    assert_eq!(merged(&nothing, None).expect("readable"), []);
}

#[test]
fn a_vector_that_is_no_json_list_of_numbers_makes_the_database_unreadable() {
    let dir = TempDir::new();
    for (i, text) in [
        "not json",
        "",
        "{\"a\": 1}",
        "[1, \"2\"]",
        "[1, null]",
        "[true]",
        "[1, 2",
        "[1, 2] [3]",
        "[NaN]",
        "[Infinity]",
        "[-Infinity]",
        "5",
        "null",
    ]
    .into_iter()
    .enumerate()
    {
        let index = dir.path().join(format!("bad{i}.db"));
        make_db(&index, &[("a.md", "[1]"), ("b.md", text)]);
        assert!(merged(&index, None).is_err(), "{text:?}");
        let good = dir.path().join(format!("good{i}.db"));
        make_db(&good, &[("a.md", "[1]")]);
        assert!(merged(&good, Some(&index)).is_err(), "extra {text:?}");
    }
}

#[test]
fn a_bad_vector_of_a_row_that_the_other_database_replaces_still_counts() {
    let dir = TempDir::new();
    let index = dir.path().join("gardener.db");
    let extra = dir.path().join("extra.db");
    make_db(&index, &[("a.md", "garbage")]);
    make_db(&extra, &[("a.md", "[1]")]);
    assert!(merged(&index, Some(&extra)).is_err());
}

/// A database whose table has no column types, so that a cell can hold any type.
fn loose_db(path: &Path, rows: &[(rusqlite::types::Value, rusqlite::types::Value)]) {
    let connection = Connection::open(path).expect("created");
    connection
        .execute("CREATE TABLE embeddings (rel, vector)", [])
        .expect("table without types");
    for (rel, vector) in rows {
        connection
            .execute("INSERT INTO embeddings VALUES (?1, ?2)", (rel, vector))
            .expect("row");
    }
}

#[test]
fn a_cell_of_another_type_is_read_as_text_or_makes_the_database_unreadable() {
    use rusqlite::types::Value as Cell;
    let dir = TempDir::new();
    let text = |value: &str| Cell::Text(value.to_string());
    let path = dir.path().join("loose.db");
    loose_db(
        &path,
        &[
            (text("a.md"), Cell::Blob(b"[1, 2]".to_vec())),
            (text("a.md"), text("[3]")),
        ],
    );
    assert_eq!(
        merged(&path, None).expect("readable"),
        [entry("a.md", &[3.0])],
        "a blob is the text of a vector; a repeated path keeps its place and its last vector"
    );
    let cases = [
        (text("b.md"), Cell::Integer(5)),
        (text("b.md"), Cell::Real(1.5)),
        (text("b.md"), Cell::Null),
        (Cell::Integer(7), text("[1]")),
        (Cell::Real(0.5), text("[1]")),
        (Cell::Null, text("[1]")),
        (Cell::Blob(b"b.md".to_vec()), text("[1]")),
    ];
    for (i, (rel, vector)) in cases.into_iter().enumerate() {
        let path = dir.path().join(format!("case{i}.db"));
        loose_db(
            &path,
            &[(text("a.md"), text("[1]")), (rel.clone(), vector.clone())],
        );
        assert!(merged(&path, None).is_err(), "{rel:?} {vector:?}");
    }
    // A path that is no UTF-8 text.
    let path = dir.path().join("latin.db");
    loose_db(&path, &[(text("a.md"), text("[1]"))]);
    Connection::open(&path)
        .expect("opens")
        .execute(
            "INSERT INTO embeddings VALUES (CAST(x'ff' AS TEXT), '[1]')",
            [],
        )
        .expect("row");
    assert!(merged(&path, None).is_err());
}

#[test]
fn reading_creates_no_file_and_changes_no_file() {
    let dir = TempDir::new();
    let folder = dir.path().join("state");
    fs::create_dir(&folder).expect("folder");
    let index = folder.join("gardener.db");
    let extra = folder.join("extra.db");
    make_db(&index, &[("a.md", "[1]")]);
    make_db(&extra, &[("b.md", "[2]")]);
    let before = (
        names(&folder),
        fs::read(&index).expect("bytes"),
        fs::read(&extra).expect("bytes"),
    );
    assert_eq!(before.0, ["extra.db", "gardener.db"]);
    for _ in 0..3 {
        merged(&index, Some(&extra)).expect("readable");
    }
    let mut state = DbVectors::new(
        DbPaths {
            index: index.clone(),
            extra: Some(extra.clone()),
        },
        Detect::Metadata,
    );
    state.notes(now_ns()).expect("readable");
    let after = (
        names(&folder),
        fs::read(&index).expect("bytes"),
        fs::read(&extra).expect("bytes"),
    );
    assert_eq!(before, after);
    assert_eq!(names(dir.path()), ["state"]);
}

#[test]
fn a_writer_is_not_held_up_after_the_read() {
    let dir = TempDir::new();
    let index = dir.path().join("gardener.db");
    make_db(&index, &[("a.md", "[1]")]);
    let mut state = DbVectors::new(
        DbPaths {
            index: index.clone(),
            extra: None,
        },
        Detect::Metadata,
    );
    for round in 0..5 {
        state.notes(now_ns()).expect("readable");
        // A writer that does not wait for a lock: any lock the read left would be `SQLITE_BUSY`.
        let writer = Connection::open(&index).expect("opens");
        writer.busy_timeout(std::time::Duration::ZERO).expect("set");
        writer
            .execute_batch(&format!(
                "BEGIN EXCLUSIVE; INSERT OR REPLACE INTO embeddings VALUES ('n{round}.md', 'h', '[1]', 0.0); COMMIT;"
            ))
            .expect("the writer is not held up");
    }
    assert_eq!(state.notes(now_ns()).expect("readable").len(), 6);
}

#[test]
fn the_query_vector_comes_from_a_file_with_a_non_empty_list_of_numbers() {
    let dir = TempDir::new();
    let index = dir.path().join("gardener.db");
    make_db(&index, &[("a.md", "[1]")]);
    let paths = DbPaths { index, extra: None };
    let query = dir.write("q.json", b" [0.5, 2] \n");
    let vectors = load_db_vectors(&query, &paths).expect("readable");
    assert_eq!(vectors.query, [0.5, 2.0]);
    assert_eq!(vectors.notes, [entry("a.md", &[1.0])]);
    for (name, text) in [
        ("empty.json", &b"[]"[..]),
        ("object.json", b"{\"query\": [1]}"),
        ("text.json", b"[\"a\"]"),
        ("nan.json", b"[NaN]"),
        ("cut.json", b"[1,"),
        ("two.json", b"[1] [2]"),
        ("none.json", b""),
    ] {
        let path = dir.write(name, text);
        assert!(load_db_vectors(&path, &paths).is_err(), "{name}");
    }
    assert!(load_db_vectors(&dir.path().join("absent.json"), &paths).is_err());
    let bad_paths = DbPaths {
        index: dir.path().join("absent.db"),
        extra: None,
    };
    assert!(load_db_vectors(&query, &bad_paths).is_err());
}

/// 10 s after `now_ns()`: every stamp is trusted.
fn settled() -> i128 {
    now_ns() + 10_000_000_000
}

fn state_of(index: &Path, extra: Option<&Path>) -> DbVectors {
    DbVectors::new(
        DbPaths {
            index: index.to_path_buf(),
            extra: extra.map(Path::to_path_buf),
        },
        Detect::Metadata,
    )
}

fn rels(state: &mut DbVectors, now: i128) -> Vec<String> {
    state
        .notes(now)
        .expect("readable")
        .iter()
        .map(|(rel, _)| rel.clone())
        .collect()
}

#[test]
fn databases_that_did_not_change_are_not_read_again() {
    let dir = TempDir::new();
    let index = dir.path().join("gardener.db");
    let extra = dir.path().join("extra.db");
    make_db(&index, &[("a.md", "[1]")]);
    make_db(&extra, &[("b.md", "[2]")]);
    let mut state = state_of(&index, Some(&extra));
    assert_eq!(rels(&mut state, settled()), ["a.md", "b.md"]);
    assert_eq!(state.reads, 1);
    for _ in 0..3 {
        assert_eq!(rels(&mut state, settled()), ["a.md", "b.md"]);
    }
    assert_eq!(state.reads, 1);
    // No second database named: nothing to change there either.
    let mut single = state_of(&index, None);
    assert_eq!(rels(&mut single, settled()), ["a.md"]);
    assert_eq!(rels(&mut single, settled()), ["a.md"]);
    assert_eq!(single.reads, 1);
    // A second database that is not there stays not there without a new read.
    let absent = dir.path().join("absent.db");
    let mut lone = state_of(&index, Some(&absent));
    assert_eq!(rels(&mut lone, settled()), ["a.md"]);
    assert_eq!(rels(&mut lone, settled()), ["a.md"]);
    assert_eq!(lone.reads, 1);
}

#[test]
fn a_changed_appeared_or_vanished_database_is_read_again() {
    let dir = TempDir::new();
    let index = dir.path().join("gardener.db");
    let extra = dir.path().join("extra.db");
    make_db(&index, &[("a.md", "[1]")]);
    let mut state = state_of(&index, Some(&extra));
    assert_eq!(rels(&mut state, settled()), ["a.md"]);
    // The second database appears.
    make_db(&extra, &[("b.md", "[2]")]);
    assert_eq!(rels(&mut state, settled()), ["a.md", "b.md"]);
    assert_eq!(state.reads, 2);
    // A row of the first changes, one is added, one goes.
    add_rows(&index, &[("a.md", "[10]"), ("c.md", "[3]")]);
    let notes = state.notes(settled()).expect("readable").to_vec();
    assert_eq!(
        notes,
        [
            entry("a.md", &[10.0]),
            entry("c.md", &[3.0]),
            entry("b.md", &[2.0])
        ],
        "the first database first, then the new paths of the second"
    );
    assert_eq!(state.reads, 3);
    // A row of the second changes.
    add_rows(&extra, &[("b.md", "[20]")]);
    assert_eq!(state.notes(settled()).expect("readable")[2].1, [20.0]);
    assert_eq!(state.reads, 4);
    Connection::open(&index)
        .expect("opens")
        .execute("DELETE FROM embeddings WHERE rel = 'c.md'", [])
        .expect("row is deleted");
    assert_eq!(rels(&mut state, settled()), ["a.md", "b.md"]);
    assert_eq!(state.reads, 5);
    // The second database vanishes.
    fs::remove_file(&extra).expect("removed");
    assert_eq!(rels(&mut state, settled()), ["a.md"]);
    assert_eq!(state.reads, 6);
    assert_eq!(rels(&mut state, settled()), ["a.md"]);
    assert_eq!(state.reads, 6);
}

#[test]
fn a_database_that_is_replaced_by_another_file_is_read_again_and_one_that_cannot_be_read_is_an_error()
 {
    let dir = TempDir::new();
    let index = dir.path().join("gardener.db");
    make_db(&index, &[("a.md", "[1]")]);
    let mut state = state_of(&index, None);
    assert_eq!(rels(&mut state, settled()), ["a.md"]);
    // Replaced by a new file (new inode).
    let new = dir.path().join("new.db");
    make_db(&new, &[("n.md", "[1]")]);
    fs::rename(&new, &index).expect("replaced");
    assert_eq!(rels(&mut state, settled()), ["n.md"]);
    assert_eq!(state.reads, 2);
    // Gone: an error; and it is looked for again.
    fs::remove_file(&index).expect("removed");
    assert!(state.notes(settled()).is_err());
    assert!(state.notes(settled()).is_err());
    make_db(&index, &[("m.md", "[1]")]);
    assert_eq!(rels(&mut state, settled()), ["m.md"]);
    // Damaged: an error, then readable again.
    fs::write(
        &index,
        b"damaged, and not a database at all, but long enough",
    )
    .expect("written");
    assert!(state.notes(settled()).is_err());
    assert!(state.notes(settled()).is_err());
    fs::remove_file(&index).expect("removed");
    make_db(&index, &[("k.md", "[1]")]);
    assert_eq!(rels(&mut state, settled()), ["k.md"]);
}

#[test]
fn files_in_doubt_are_compared_by_their_content_and_kept_when_it_is_the_same() {
    let dir = TempDir::new();
    let index = dir.path().join("gardener.db");
    let extra = dir.path().join("extra.db");
    make_db(&index, &[("a.md", "[1]")]);
    make_db(&extra, &[("b.md", "[2]")]);
    let mut state = state_of(&index, Some(&extra));
    // Just written: the stamps prove nothing yet.
    assert_eq!(rels(&mut state, now_ns()), ["a.md", "b.md"]);
    assert_eq!(state.reads, 1);
    assert_eq!(rels(&mut state, now_ns()), ["a.md", "b.md"]);
    assert_eq!(state.reads, 1, "same content: kept");
    // Only the second database changes, and the first stays as it was.
    add_rows(&extra, &[("b.md", "[20]")]);
    assert_eq!(state.notes(now_ns()).expect("readable")[1].1, [20.0]);
    assert_eq!(state.reads, 2);
    // Later the stamps are trusted, and the content that was kept is dropped.
    assert_eq!(state.notes(settled()).expect("readable")[1].1, [20.0]);
    assert_eq!(state.reads, 2);
    assert_eq!(rels(&mut state, settled()), ["a.md", "b.md"]);
    assert_eq!(state.reads, 2);
    // Trusted stamps that differ: the change is seen at once.
    add_rows(&index, &[("a.md", "[5]")]);
    assert_eq!(state.notes(settled()).expect("readable")[0].1, [5.0]);
    assert_eq!(state.reads, 3);
}

#[test]
fn files_in_doubt_with_a_second_database_that_is_not_there_are_compared_too() {
    let dir = TempDir::new();
    let index = dir.path().join("gardener.db");
    let extra = dir.path().join("extra.db");
    make_db(&index, &[("a.md", "[1]")]);
    let mut state = state_of(&index, Some(&extra));
    assert_eq!(rels(&mut state, now_ns()), ["a.md"]);
    assert_eq!(rels(&mut state, now_ns()), ["a.md"]);
    assert_eq!(state.reads, 1);
    make_db(&extra, &[("b.md", "[2]")]);
    assert_eq!(rels(&mut state, now_ns()), ["a.md", "b.md"]);
    assert_eq!(state.reads, 2);
    fs::remove_file(&extra).expect("removed");
    assert_eq!(rels(&mut state, now_ns()), ["a.md"]);
    assert_eq!(state.reads, 3);
}

#[test]
fn change_detection_by_content_reads_the_files_on_every_look_and_reads_the_tables_only_when_they_differ()
 {
    let dir = TempDir::new();
    let index = dir.path().join("gardener.db");
    make_db(&index, &[("a.md", "[1]")]);
    let mut state = DbVectors::new(
        DbPaths {
            index: index.clone(),
            extra: None,
        },
        Detect::Content,
    );
    for _ in 0..3 {
        assert_eq!(rels(&mut state, settled()), ["a.md"]);
    }
    assert_eq!(state.reads, 1);
    add_rows(&index, &[("b.md", "[2]")]);
    assert_eq!(rels(&mut state, settled()), ["a.md", "b.md"]);
    assert_eq!(state.reads, 2);
}

#[test]
fn an_unreadable_file_in_doubt_is_an_error_not_a_stale_answer() {
    let dir = TempDir::new();
    let index = dir.path().join("gardener.db");
    make_db(&index, &[("a.md", "[1]")]);
    let mut state = state_of(&index, None);
    assert_eq!(rels(&mut state, now_ns()), ["a.md"]);
    // A folder where the file was: it has a stamp and cannot be read as bytes.
    fs::remove_file(&index).expect("removed");
    fs::create_dir(&index).expect("folder");
    assert!(state.notes(now_ns()).is_err());
    fs::remove_dir(&index).expect("removed");
    make_db(&index, &[("b.md", "[1]")]);
    assert_eq!(rels(&mut state, now_ns()), ["b.md"]);
}

fn call(args: &[&str]) -> (u8, String) {
    let args: Vec<String> = args.iter().map(ToString::to_string).collect();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = run(&args, &mut out, &mut err);
    assert!(
        err.is_empty() || String::from_utf8_lossy(&err).starts_with("usage:"),
        "{err:?}"
    );
    (code, String::from_utf8(out).expect("UTF-8"))
}

fn path(dir: &TempDir, name: &str) -> String {
    dir.path().join(name).to_string_lossy().into_owned()
}

/// A vault of three notes, two databases and a query vector file, all invented.
fn world(dir: &TempDir) {
    dir.write("vault/a.md", b"Beet im Garten\n");
    dir.write("vault/b.md", b"Beet Beet Kaffee\n");
    dir.write("vault/c.md", b"Wasserfall\n");
    make_db(
        &dir.path().join("gardener.db"),
        &[
            ("a.md", "[1, 0]"),
            ("b.md", "[0, 1]"),
            ("90-secrets/x.md", "[1, 0]"),
        ],
    );
    make_db(
        &dir.path().join("extra.db"),
        &[
            ("b.md", "[0.5, 0.5]"),
            ("c.md", "[1, 0.2]"),
            ("gone.md", "[1, 1]"),
        ],
    );
    dir.write("q.json", b"[1, 0.1]");
}

fn the_same_vectors_as_a_file(dir: &TempDir) -> String {
    dir.write(
        "f.json",
        br#"{"query": [1, 0.1], "notes": {"a.md": [1, 0], "b.md": [0.5, 0.5], "90-secrets/x.md": [1, 0], "c.md": [1, 0.2], "gone.md": [1, 1]}}"#,
    );
    path(dir, "f.json")
}

#[test]
fn search_with_the_databases_prints_what_search_prints_with_the_same_vectors_as_a_file() {
    let dir = TempDir::new();
    world(&dir);
    let vault = path(&dir, "vault");
    let file = the_same_vectors_as_a_file(&dir);
    for k in ["3", "2"] {
        let with_db = call(&[
            "search",
            &vault,
            "beet",
            "--index-db",
            &path(&dir, "gardener.db"),
            "--extra-db",
            &path(&dir, "extra.db"),
            "--query-vector",
            &path(&dir, "q.json"),
            "--today",
            "2026-11-18",
            "-k",
            k,
        ]);
        let with_file = call(&[
            "search",
            &vault,
            "beet",
            "--vectors",
            &file,
            "--today",
            "2026-11-18",
            "-k",
            k,
        ]);
        assert_eq!(with_db.0, 0);
        assert_eq!(with_db, with_file, "k {k}");
        let answer: Value = serde_json::from_str(&with_db.1).expect("JSON");
        assert_eq!(answer["fallback"], json!(false));
        let rels: Vec<&str> = answer["hits"]
            .as_array()
            .expect("list")
            .iter()
            .map(|hit| hit["rel"].as_str().expect("rel"))
            .collect();
        assert!(!rels.contains(&"gone.md") && !rels.contains(&"90-secrets/x.md"));
        if k == "3" {
            assert!(
                rels.contains(&"c.md"),
                "c.md has a vector only in the second database: {rels:?}"
            );
        }
    }
}

#[test]
fn search_without_the_second_database_reads_the_first_alone() {
    let dir = TempDir::new();
    world(&dir);
    let vault = path(&dir, "vault");
    dir.write(
        "f.json",
        br#"{"query": [1, 0.1], "notes": {"a.md": [1, 0], "b.md": [0, 1], "90-secrets/x.md": [1, 0]}}"#,
    );
    let args = |extra: &[&'static str]| -> Vec<String> {
        let mut args = vec!["search".to_string(), vault.clone(), "beet".to_string()];
        args.extend(["--today", "2026-11-18"].map(String::from));
        args.extend(extra.iter().map(ToString::to_string));
        args
    };
    let run_args = |args: Vec<String>| {
        let refs: Vec<&str> = args.iter().map(String::as_str).collect();
        call(&refs)
    };
    let mut db = args(&["--query-vector"]);
    db.push(path(&dir, "q.json"));
    db.extend(["--index-db".to_string(), path(&dir, "gardener.db")]);
    let mut file = args(&["--vectors"]);
    file.push(path(&dir, "f.json"));
    assert_eq!(run_args(db.clone()), run_args(file));
    // The second database named but not there: the same.
    let mut named = db;
    named.extend(["--extra-db".to_string(), path(&dir, "absent.db")]);
    let (code, out) = run_args(named);
    assert_eq!(code, 0);
    assert!(out.contains("\"fallback\":false"));
}

#[test]
fn without_a_query_vector_search_takes_the_lexical_way_and_reads_no_database() {
    let dir = TempDir::new();
    world(&dir);
    let vault = path(&dir, "vault");
    let plain = call(&["search", &vault, "beet", "--today", "2026-11-18"]);
    assert_eq!(plain.0, 0);
    assert!(plain.1.contains("\"fallback\":true"));
    for index in [path(&dir, "gardener.db"), path(&dir, "absent.db")] {
        let db = call(&[
            "search",
            &vault,
            "beet",
            "--today",
            "2026-11-18",
            "--index-db",
            &index,
            "--extra-db",
            &path(&dir, "extra.db"),
        ]);
        assert_eq!(db, plain, "{index}");
    }
}

#[test]
fn search_with_a_database_or_query_vector_that_cannot_be_read_is_exit_3() {
    let dir = TempDir::new();
    world(&dir);
    let vault = path(&dir, "vault");
    let (q, index, extra) = (
        path(&dir, "q.json"),
        path(&dir, "gardener.db"),
        path(&dir, "extra.db"),
    );
    let unreadable = (3, "{\"error\":\"unreadable\"}\n".to_string());
    let go = |index: &str, extra: &str, q: &str| {
        call(&[
            "search",
            &vault,
            "beet",
            "--index-db",
            index,
            "--extra-db",
            extra,
            "--query-vector",
            q,
        ])
    };
    assert_eq!(go(&index, &extra, &q).0, 0);
    assert_eq!(go(&path(&dir, "absent.db"), &extra, &q), unreadable);
    assert_eq!(go(&path(&dir, "q.json"), &extra, &q), unreadable);
    assert_eq!(go(&index, &path(&dir, "q.json"), &q), unreadable);
    assert_eq!(go(&index, &extra, &path(&dir, "absent.json")), unreadable);
    dir.write("empty.json", b"[]");
    assert_eq!(go(&index, &extra, &path(&dir, "empty.json")), unreadable);
    dir.write("bad.json", b"{\"query\": [1]}");
    assert_eq!(go(&index, &extra, &path(&dir, "bad.json")), unreadable);
    // The vault is checked first: not a folder is unreadable, too, and nothing is created.
    assert_eq!(
        call(&[
            "search",
            &path(&dir, "q.json"),
            "beet",
            "--index-db",
            &index,
            "--query-vector",
            &q
        ]),
        unreadable
    );
}

#[test]
fn the_database_options_go_with_each_other_and_not_with_the_vector_file() {
    let dir = TempDir::new();
    world(&dir);
    let vault = path(&dir, "vault");
    let (q, index, extra) = (
        path(&dir, "q.json"),
        path(&dir, "gardener.db"),
        path(&dir, "extra.db"),
    );
    let file = the_same_vectors_as_a_file(&dir);
    let bad: [&[&str]; 9] = [
        &["--index-db", &index, "--vectors", &file],
        &[
            "--vectors",
            &file,
            "--index-db",
            &index,
            "--query-vector",
            &q,
        ],
        &["--extra-db", &extra],
        &["--query-vector", &q],
        &["--extra-db", &extra, "--query-vector", &q],
        &["--extra-db", &extra, "--vectors", &file],
        &["--query-vector", &q, "--vectors", &file],
        &["--index-db"],
        &["--index-db", &index, "--query-vector"],
    ];
    for options in bad {
        let mut args = vec!["search", &vault, "beet"];
        args.extend_from_slice(options);
        assert_eq!(call(&args), (64, String::new()), "{options:?}");
    }
    // `search` keeps `argparse`: a repeated option counts with its last value.
    assert_eq!(
        call(&[
            "search",
            &vault,
            "beet",
            "--index-db",
            "absent",
            "--index-db",
            &index
        ])
        .0,
        0
    );
    // `--vectors` alone and the database options alone are fine.
    assert_eq!(call(&["search", &vault, "beet", "--vectors", &file]).0, 0);
    assert_eq!(call(&["search", &vault, "beet", "--index-db", &index]).0, 0);
    // The abbreviations and the short forms of `argparse` do not exist.
    for options in [["--index", index.as_str()], ["--query", q.as_str()]] {
        assert_eq!(
            call(&["search", &vault, "beet", options[0], options[1]]),
            (64, String::new())
        );
    }
    assert_eq!(
        call(&[
            "search",
            &vault,
            "beet",
            &format!("--index-db={index}"),
            &format!("--query-vector={q}")
        ])
        .0,
        0
    );
}

fn serve_call(args: &[&str], input: &str) -> (u8, String) {
    let args: Vec<String> = args.iter().map(ToString::to_string).collect();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = crate::cli::run_with_input(
        &args,
        &mut std::io::Cursor::new(input.as_bytes().to_vec()),
        &mut out,
        &mut err,
    );
    (code, String::from_utf8(out).expect("UTF-8"))
}

#[test]
fn serve_with_the_databases_answers_like_search_with_the_same_databases() {
    let dir = TempDir::new();
    world(&dir);
    let vault = path(&dir, "vault");
    let (index, extra) = (path(&dir, "gardener.db"), path(&dir, "extra.db"));
    let request = r#"{"op":"search","query":"beet","k":3,"today":"2026-11-18","query_vector":[1,0.1],"id":1}"#;
    let (code, out) = serve_call(
        &["serve", &vault, "--index-db", &index, "--extra-db", &extra],
        &format!("{request}\n"),
    );
    assert_eq!(code, 0);
    let mut served: Value = serde_json::from_str(out.trim_end()).expect("JSON");
    assert_eq!(served["id"], json!(1));
    served.as_object_mut().expect("object").remove("id");
    let (code, out) = call(&[
        "search",
        &vault,
        "beet",
        "-k",
        "3",
        "--today",
        "2026-11-18",
        "--index-db",
        &index,
        "--extra-db",
        &extra,
        "--query-vector",
        &path(&dir, "q.json"),
    ]);
    assert_eq!(code, 0);
    assert_eq!(served, serde_json::from_str::<Value>(&out).expect("JSON"));
    // Without the second database named, the first alone is read.
    let (_, alone) = serve_call(
        &["serve", &vault, "--index-db", &index],
        &format!("{request}\n"),
    );
    assert_ne!(alone, out);
    assert!(!alone.contains("c.md"));
    // A request without a vector does not read the databases.
    let (_, lexical) = serve_call(
        &["serve", &vault, "--index-db", &path(&dir, "absent.db")],
        "{\"op\":\"search\",\"query\":\"beet\",\"today\":\"2026-11-18\"}\n",
    );
    assert!(lexical.contains("\"fallback\":true"));
    // With a vector and a database that cannot be read: the answer says so, and the process lives on.
    let (code, out) = serve_call(
        &["serve", &vault, "--index-db", &path(&dir, "absent.db")],
        &format!("{request}\n{{\"op\":\"ping\"}}\n"),
    );
    assert_eq!(code, 0);
    assert_eq!(out, "{\"error\":\"unreadable\",\"id\":1}\n{\"ok\":true}\n");
}

#[test]
fn the_server_sees_the_rows_of_the_databases_change() {
    let dir = TempDir::new();
    world(&dir);
    let mut server = crate::serve::Server::start_dbs_with(
        dir.path().join("vault"),
        DbPaths {
            index: dir.path().join("gardener.db"),
            extra: Some(dir.path().join("extra.db")),
        },
        Detect::Content,
    );
    let ask = |server: &mut crate::serve::Server| -> Value {
        let line =
            r#"{"op":"search","query":"zzz","k":5,"today":"2026-11-18","query_vector":[0,1]}"#;
        server
            .answer(&crate::serve::Line {
                bytes: line.as_bytes().to_vec(),
                too_long: false,
            })
            .0
    };
    let first = |answer: &Value| answer["hits"][0]["rel"].clone();
    let before = ask(&mut server);
    assert_eq!(before["fallback"], json!(false));
    add_rows(&dir.path().join("extra.db"), &[("c.md", "[0, 2]")]);
    let after = ask(&mut server);
    assert_ne!(before, after);
    assert!(first(&after) == json!("b.md") || first(&after) == json!("c.md"));
    fs::remove_file(dir.path().join("extra.db")).expect("removed");
    let without = ask(&mut server);
    assert_ne!(after, without);
    fs::remove_file(dir.path().join("gardener.db")).expect("removed");
    assert_eq!(ask(&mut server), json!({"error": "unreadable"}));
}

#[test]
fn serve_takes_the_database_options_by_the_same_rules() {
    let dir = TempDir::new();
    world(&dir);
    let vault = path(&dir, "vault");
    let (q, index, extra) = (
        path(&dir, "q.json"),
        path(&dir, "gardener.db"),
        path(&dir, "extra.db"),
    );
    let file = the_same_vectors_as_a_file(&dir);
    let ping = "{\"op\":\"ping\"}\n";
    let good: [&[&str]; 5] = [
        &["--index-db", &index],
        &["--index-db", &index, "--extra-db", &extra],
        &["--extra-db", &extra, "--index-db", &index],
        &["--vectors", &file],
        &[],
    ];
    for options in good {
        let mut args = vec!["serve", vault.as_str()];
        args.extend_from_slice(options);
        assert_eq!(serve_call(&args, ping), (0, "{\"ok\":true}\n".to_string()));
    }
    let bad: [&[&str]; 5] = [
        &["--index-db", &index, "--vectors", &file],
        &["--extra-db", &extra],
        &["--extra-db", &extra, "--vectors", &file],
        &["--index-db", &index, "--query-vector", &q],
        &["--query-vector", &q],
    ];
    for options in bad {
        let mut args = vec!["serve", vault.as_str()];
        args.extend_from_slice(options);
        assert_eq!(serve_call(&args, ping), (64, String::new()), "{options:?}");
    }
    // Each option at most once.
    let twice: [&[&str]; 5] = [
        &["--index-db", &index, "--index-db", &index],
        &[
            "--index-db",
            &index,
            "--extra-db",
            &extra,
            "--extra-db",
            &extra,
        ],
        &["--vectors", &file, "--vectors", &file],
        &["--index-db", &index, "--extra-db=x", "--extra-db", &extra],
        &["--index-db", &index, "--index-db=y"],
    ];
    for options in twice {
        let mut args = vec!["serve", vault.as_str()];
        args.extend_from_slice(options);
        assert_eq!(serve_call(&args, ping), (64, String::new()), "{options:?}");
    }
}

/// Sets one byte of the file.
fn patch_byte(path: &Path, at: usize, value: u8) {
    let mut bytes = fs::read(path).expect("bytes");
    bytes[at] = value;
    fs::write(path, bytes).expect("written");
}

#[test]
fn a_database_in_wal_mode_is_unreadable_and_leaves_no_file() {
    let dir = TempDir::new();
    let folder = dir.path().join("state");
    fs::create_dir(&folder).expect("folder");
    let index = folder.join("gardener.db");
    {
        let connection = Connection::open(&index).expect("created");
        let _mode: String = connection
            .pragma_update_and_check(None, "journal_mode", "WAL", |row| row.get(0))
            .expect("wal");
        connection.execute(SCHEMA, []).expect("table");
        put(&connection, &[("a.md", "[1]")]);
    }
    let before = names(&folder);
    assert_eq!(before, ["gardener.db"]);
    assert!(merged(&index, None).is_err());
    let extra = folder.join("extra.db");
    assert!(merged(&index, Some(&extra)).is_err());
    assert_eq!(names(&folder), before, "no -wal and no -shm file");
    // As the second database it counts, too.
    let plain = folder.join("plain.db");
    make_db(&plain, &[("b.md", "[1]")]);
    let before = names(&folder);
    assert!(merged(&plain, Some(&index)).is_err());
    assert_eq!(names(&folder), before);
    // Either byte of the header is enough, and 1 in both is the rollback journal.
    for (at, other) in [(18, 19), (19, 18)] {
        let copy = folder.join(format!("copy{at}.db"));
        make_db(&copy, &[("c.md", "[1]")]);
        assert_eq!(fs::read(&copy).expect("bytes")[at], 1);
        assert!(merged(&copy, None).is_ok());
        patch_byte(&copy, at, 2);
        assert_eq!(fs::read(&copy).expect("bytes")[other], 1);
        assert!(merged(&copy, None).is_err(), "byte {at}");
    }
}

#[test]
fn a_file_with_a_short_header_is_left_to_sqlite() {
    let dir = TempDir::new();
    for (i, bytes) in [&b""[..], b"short", &[2_u8; 19]].into_iter().enumerate() {
        let path = dir.write(&format!("short{i}.db"), bytes);
        assert!(merged(&path, None).is_err());
    }
    let wal_bytes = dir.write("exact.db", &[2_u8; 20]);
    assert!(merged(&wal_bytes, None).is_err());
}
