//! The note vectors straight from the databases of the model: `gardener.db` of the vault and the
//! side store `extra-embeddings.db` (the table `embeddings` in both), read only.
//!
//! The merge is the one of `load_merged_embeddings`: every row of the first database, then the rows
//! of the second; a path that is already there gets the new vector and keeps its place, a new path
//! goes to the end. A missing second database counts as empty.
//!
//! A database is opened with `SQLITE_OPEN_READ_ONLY`, so the process creates no journal, no `-wal`
//! or `-shm` file and no folder. A database in WAL mode (bytes 18 and 19 of its header) would get
//! `-wal` and `-shm` files on such an open, so it is not opened and counts as unreadable. The rows are copied out in one statement and the connection is
//! closed before a single vector is parsed, so a writer (the Python gardener) waits no longer than
//! the copy takes. [`DbVectors`] is the state of `rangkern serve`: it holds no connection and no file
//! between two requests, only the vectors, and reads the databases again when their stamp changed
//! or is in doubt ([`crate::stamp`]).

use std::collections::HashMap;
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::Duration;

use rusqlite::types::ValueRef;
use rusqlite::{Connection, OpenFlags};

use crate::stamp::{Detect, Stamp};
use crate::vault::{Unreadable, Vectors, load_query_vector, parse_vector};

/// How long a read waits for a writer that holds the database: the default of Python's `sqlite3`.
const BUSY_TIMEOUT: Duration = Duration::from_secs(5);
/// The query of the model (`load_all_embeddings`), word for word.
const QUERY: &str = "SELECT rel, vector FROM embeddings";

/// `(path, vector)` in the order of the merge.
pub type Entries = Vec<(String, Vec<f64>)>;

impl From<rusqlite::Error> for Unreadable {
    fn from(_: rusqlite::Error) -> Self {
        Unreadable
    }
}

/// The databases the note vectors come from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DbPaths {
    /// `gardener.db`: read first.
    pub index: PathBuf,
    /// `extra-embeddings.db`: read second, and its vector wins; no path or a missing file is empty.
    pub extra: Option<PathBuf>,
}

/// Whether the header of the file says WAL mode: byte 18 or byte 19 is 2 (1 is the rollback journal).
///
/// A read-only open of a WAL database creates `-wal` and `-shm` files, and they stay. The program
/// creates no file, so such a database counts as unreadable and is not opened. A file with a header
/// shorter than 20 bytes is left to SQLite.
fn is_wal(path: &Path) -> bool {
    let mut head = [0_u8; 20];
    File::open(path)
        .and_then(|mut file| file.read_exact(&mut head))
        .is_ok()
        && (head[18] == 2 || head[19] == 2)
}

/// The rows of the table `embeddings` of one database as `(path, text of the vector)`, in the order
/// SQLite gives them. The connection is closed before this returns.
fn fetch(path: &Path) -> Result<Vec<(String, Vec<u8>)>, Unreadable> {
    if is_wal(path) {
        return Err(Unreadable);
    }
    // Read only: SQLite creates no journal, `-wal`, `-shm` or folder for such a connection.
    let connection = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    connection.busy_timeout(BUSY_TIMEOUT)?;
    let mut rows = Vec::new();
    {
        let mut statement = connection.prepare(QUERY)?;
        let mut cursor = statement.query([])?;
        while let Some(row) = cursor.next()? {
            let ValueRef::Text(rel) = row.get_ref(0)? else {
                return Err(Unreadable);
            };
            let (ValueRef::Text(vector) | ValueRef::Blob(vector)) = row.get_ref(1)? else {
                return Err(Unreadable);
            };
            let rel = String::from_utf8(rel.to_vec()).map_err(|_| Unreadable)?;
            rows.push((rel, vector.to_vec()));
        }
    }
    connection.close().map_err(|_| Unreadable)?;
    Ok(rows)
}

/// The merge: entries in the order of first appearance, the last vector of a path.
#[derive(Default)]
struct Merged {
    entries: Entries,
    at: HashMap<String, usize>,
}

impl Merged {
    fn put(&mut self, rel: String, vector: Vec<f64>) {
        if let Some(&i) = self.at.get(&rel) {
            self.entries[i].1 = vector;
        } else {
            self.at.insert(rel.clone(), self.entries.len());
            self.entries.push((rel, vector));
        }
    }
}

/// The note vectors of the databases, merged as `load_merged_embeddings` merges them.
///
/// # Errors
///
/// [`Unreadable`] when a database is no SQLite database, has no table `embeddings` with the
/// columns `rel` and `vector`, or holds a `vector` that is no JSON list of numbers. The first
/// database must exist; the second may be missing.
pub fn load_merged(paths: &DbPaths) -> Result<Entries, Unreadable> {
    let index = fetch(&paths.index)?;
    let extra = match paths.extra.as_deref() {
        Some(path) if path.exists() => fetch(path)?,
        _ => Vec::new(),
    };
    let mut merged = Merged::default();
    for (rel, text) in index.into_iter().chain(extra) {
        merged.put(rel, parse_vector(&text)?);
    }
    Ok(merged.entries)
}

/// The vectors for `search --index-db`: the query vector from its file, the notes from the databases.
///
/// # Errors
///
/// [`Unreadable`] as [`load_merged`] and [`load_query_vector`] give it.
pub fn load_db_vectors(query_vector: &Path, paths: &DbPaths) -> Result<Vectors, Unreadable> {
    Ok(Vectors {
        query: load_query_vector(query_vector)?,
        notes: load_merged(paths)?,
    })
}

/// The state of the databases at the last look.
#[derive(Debug)]
struct Loaded {
    /// The stamp of each database, the index first; `None` for a file that is not there.
    stamps: [Option<Stamp>; 2],
    /// The bytes the notes were read from, kept only while a stamp is in doubt.
    raw: Option<[Option<Vec<u8>>; 2]>,
    notes: Entries,
}

impl Loaded {
    /// Whether the stamps say that no file changed, appeared or vanished.
    fn unchanged(&self, new: &[Option<Stamp>; 2]) -> bool {
        self.stamps.iter().zip(new).all(|pair| match pair {
            (None, None) => true,
            (Some(old), Some(new)) => old.unchanged(new),
            _ => false,
        })
    }
}

/// The note vectors of the databases, in memory while the databases are unchanged.
#[derive(Debug)]
pub(crate) struct DbVectors {
    paths: DbPaths,
    detect: Detect,
    loaded: Option<Loaded>,
    /// How often the databases were opened and read, for the tests.
    pub(crate) reads: usize,
}

impl DbVectors {
    pub(crate) fn new(paths: DbPaths, detect: Detect) -> Self {
        Self {
            paths,
            detect,
            loaded: None,
            reads: 0,
        }
    }

    /// The note vectors as the databases hold them now; `now` is the time in nanoseconds since
    /// the epoch.
    pub(crate) fn notes(&mut self, now: i128) -> Result<&[(String, Vec<f64>)], Unreadable> {
        self.refresh(now)?;
        Ok(self
            .loaded
            .as_ref()
            .map_or(&[][..], |loaded| loaded.notes.as_slice()))
    }

    fn stamp_of(&self, path: &Path, now: i128) -> Option<Stamp> {
        let meta = fs::metadata(path).ok()?;
        Some(Stamp::new(&meta, now, self.detect))
    }

    /// The bytes of the two files; the second is `None` when it is not there (`has_extra` false).
    fn bytes(&self, has_extra: bool) -> Result<[Option<Vec<u8>>; 2], Unreadable> {
        let index = fs::read(&self.paths.index)?;
        let extra = match &self.paths.extra {
            Some(path) if has_extra => Some(fs::read(path)?),
            _ => None,
        };
        Ok([Some(index), extra])
    }

    /// Looks at the databases: kept if they are the ones read before, read again if not.
    fn refresh(&mut self, now: i128) -> Result<(), Unreadable> {
        let stamps = [
            self.stamp_of(&self.paths.index, now),
            self.paths
                .extra
                .as_deref()
                .and_then(|path| self.stamp_of(path, now)),
        ];
        if stamps[0].is_none() {
            self.loaded = None;
            return Err(Unreadable);
        }
        if self
            .loaded
            .as_ref()
            .is_some_and(|loaded| loaded.unchanged(&stamps))
        {
            return Ok(());
        }
        let racy = stamps.iter().flatten().any(Stamp::racy);
        let compare = self
            .loaded
            .as_ref()
            .is_some_and(|loaded| loaded.raw.is_some());
        let mut raw = None;
        if racy || compare {
            let Ok(bytes) = self.bytes(stamps[1].is_some()) else {
                self.loaded = None;
                return Err(Unreadable);
            };
            raw = Some(bytes);
        }
        if let Some(loaded) = &mut self.loaded
            && raw.is_some()
            && loaded.raw == raw
        {
            loaded.stamps = stamps;
            loaded.raw = if racy { raw } else { None };
            return Ok(());
        }
        self.reads += 1;
        let Ok(notes) = load_merged(&self.paths) else {
            self.loaded = None;
            return Err(Unreadable);
        };
        self.loaded = Some(Loaded {
            stamps,
            raw: if racy { raw } else { None },
            notes,
        });
        Ok(())
    }
}
