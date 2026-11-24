//! `rangkern serve <vault> [--vectors <file> | --index-db <db> [--extra-db <db>]]`: the search as a
//! warm process.
//!
//! The process holds the notes, the index and the note vectors in memory and answers one JSON line
//! per request. The note vectors come from a vector file or straight from the databases of the model
//! ([`crate::vdb`]). Every answer is the one `rangkern search` would give at that moment on the same
//! files: before a search the state is compared with the disk ([`crate::warm`], [`crate::stamp`]),
//! with no file watcher and no waiting. Between two requests no file is open, nothing is written
//! and no environment variable is read.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::{self, BufRead, Read, Write};
use std::path::{Path, PathBuf};

use serde::Deserialize;
use serde::de::{Deserializer, MapAccess, Visitor};
use serde_json::value::RawValue;
use serde_json::{Map, Value, json};

use crate::cli::{EXIT_OK, EXIT_UNREADABLE, hit_json, is_date, today};
use crate::embed::Embedder;
use crate::hit::SearchHit;
use crate::notes::NoteRec;
use crate::search::{Params, rank, rank_embedding};
use crate::stamp::{Detect, Stamp, now_ns};
use crate::token::Analyzer;
use crate::vault::{Unreadable, VectorView, parse_note_vectors};
use crate::vdb::{DbPaths, DbVectors};
use crate::warm::Warm;

/// A line longer than this many bytes (without its line ending) is an invalid request.
pub const LINE_MAX: usize = 1024 * 1024;
/// The number of hits when a request gives no `k`.
const K_DEFAULT: usize = 5;
/// The fields a request may have.
const FIELDS: [&str; 7] = ["op", "id", "query", "k", "space", "today", "query_vector"];

/// One line of the input, without its line ending.
#[derive(Debug, PartialEq, Eq)]
pub struct Line {
    /// The bytes, at most [`LINE_MAX`] + 2 of them.
    pub bytes: Vec<u8>,
    /// Whether the line is longer than [`LINE_MAX`] bytes.
    pub too_long: bool,
}

/// Reads the next line: up to `\n`, or up to the end of the input for a last line without one. A
/// `\r` right before the `\n` is dropped. `None` at the end of the input.
///
/// # Errors
///
/// The error of the reader.
pub fn next_line<R: BufRead>(input: &mut R) -> io::Result<Option<Line>> {
    // Room for the longest valid line with its `\r\n`.
    let cap = u64::try_from(LINE_MAX)
        .unwrap_or(u64::MAX)
        .saturating_add(2);
    let mut bytes: Vec<u8> = Vec::new();
    if Read::take(&mut *input, cap).read_until(b'\n', &mut bytes)? == 0 {
        return Ok(None);
    }
    let ended = bytes.last() == Some(&b'\n');
    // The cap was reached inside the line: the rest of it is skipped unread.
    let over = !ended && u64::try_from(bytes.len()).is_ok_and(|len| len >= cap);
    if over {
        input.skip_until(b'\n')?;
    }
    if ended {
        bytes.pop();
    }
    if bytes.last() == Some(&b'\r') {
        bytes.pop();
    }
    let too_long = over || bytes.len() > LINE_MAX;
    Ok(Some(Line { bytes, too_long }))
}

/// A JSON object with its keys in order and repeated keys kept.
#[derive(Debug)]
pub(crate) struct Pairs(pub(crate) Vec<(String, Box<RawValue>)>);

impl<'de> Deserialize<'de> for Pairs {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct PairsVisitor;
        impl<'de> Visitor<'de> for PairsVisitor {
            type Value = Pairs;
            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("an object")
            }
            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Pairs, A::Error> {
                let mut pairs = Vec::new();
                while let Some(entry) = map.next_entry::<String, Box<RawValue>>()? {
                    pairs.push(entry);
                }
                Ok(Pairs(pairs))
            }
        }
        deserializer.deserialize_map(PairsVisitor)
    }
}

/// The question of a search request and how to answer it.
#[derive(Debug, PartialEq)]
pub struct SearchRequest {
    /// The question.
    pub query: String,
    /// The number of hits.
    pub k: usize,
    /// A space or a comma list of spaces.
    pub space: Option<String>,
    /// The date for `valid-until`.
    pub today: Option<String>,
    /// The vector of the question.
    pub query_vector: Option<Vec<f64>>,
}

/// What a request asks for.
#[derive(Debug, PartialEq)]
pub enum Op {
    /// `search`.
    Search(SearchRequest),
    /// `ping`.
    Ping,
    /// `quit`.
    Quit,
}

/// A valid request.
#[derive(Debug, PartialEq)]
pub struct Request {
    /// The `id`, to be returned unchanged.
    pub id: Option<Value>,
    /// The operation.
    pub op: Op,
}

/// A valid `id`: a text or a whole number.
pub(crate) fn is_id(value: &Value) -> bool {
    value.is_string() || value.is_i64() || value.is_u64()
}

/// A field that is not valid.
pub(crate) struct Invalid;

/// A text or `null` (no value).
pub(crate) fn text_or_null(value: &Value) -> Result<Option<String>, Invalid> {
    match value {
        Value::Null => Ok(None),
        Value::String(text) => Ok(Some(text.clone())),
        _ => Err(Invalid),
    }
}

/// A non-empty list of numbers, or `null` (no value).
fn vector_or_null(value: &Value) -> Result<Option<Vec<f64>>, Invalid> {
    match value {
        Value::Null => Ok(None),
        Value::Array(items) if !items.is_empty() => items
            .iter()
            .map(|item| match item {
                Value::Number(number) => number.as_f64(),
                _ => None,
            })
            .collect::<Option<Vec<f64>>>()
            .map(Some)
            .ok_or(Invalid),
        _ => Err(Invalid),
    }
}

/// A date of the form `YYYY-MM-DD`, or `null` (no value).
fn date_or_null(value: &Value) -> Result<Option<String>, Invalid> {
    match text_or_null(value)? {
        Some(text) if !is_date(&text) => Err(Invalid),
        date => Ok(date),
    }
}

/// The value of an optional field, `None` (no value) when the field is absent.
fn optional<T>(
    fields: &Map<String, Value>,
    name: &str,
    read: fn(&Value) -> Result<Option<T>, Invalid>,
) -> Result<Option<T>, Invalid> {
    fields.get(name).map_or(Ok(None), read)
}

/// `k` from the text of its number: a whole number of 1 or more with no fraction and no exponent;
/// a number beyond the range is as large as the range allows, as `rangkern search` reads `-k`.
pub(crate) fn k_of(text: &str) -> Option<usize> {
    let digits_only = !text.is_empty() && text.bytes().all(|b| b.is_ascii_digit());
    if !digits_only || text == "0" {
        return None;
    }
    Some(
        text.bytes()
            .try_fold(0_usize, |acc, b| {
                acc.checked_mul(10)?.checked_add(usize::from(b - b'0'))
            })
            .unwrap_or(usize::MAX),
    )
}

/// Whether the request has no field besides `op` and `id` (`ping` and `quit` take no other).
fn only_op_and_id(fields: &Map<String, Value>) -> bool {
    fields.keys().all(|key| key == "op" || key == "id")
}

/// Reads the fields of a request that has no repeated key and no unknown one.
fn request_of(fields: &Map<String, Value>, k_text: Option<&str>) -> Result<Request, Invalid> {
    let id = match fields.get("id") {
        None => None,
        Some(id) if is_id(id) => Some(id.clone()),
        Some(_) => return Err(Invalid),
    };
    let k = match k_text {
        None => K_DEFAULT,
        Some(text) => k_of(text).ok_or(Invalid)?,
    };
    let space = optional(fields, "space", text_or_null)?;
    let today = optional(fields, "today", date_or_null)?;
    let query_vector = optional(fields, "query_vector", vector_or_null)?;
    let op = fields.get("op").and_then(Value::as_str).ok_or(Invalid)?;
    let op = match op {
        "search" => Op::Search(SearchRequest {
            query: fields
                .get("query")
                .and_then(Value::as_str)
                .ok_or(Invalid)?
                .to_string(),
            k,
            space,
            today,
            query_vector,
        }),
        "ping" if only_op_and_id(fields) => Op::Ping,
        "quit" if only_op_and_id(fields) => Op::Quit,
        _ => return Err(Invalid),
    };
    Ok(Request { id, op })
}

/// A line read as a JSON object with no repeated key and no key outside the allowed ones.
#[derive(Debug)]
pub(crate) struct Fields {
    /// The `id` of the line: the one valid `id`, if there is exactly one.
    pub(crate) id: Option<Value>,
    /// The fields.
    pub(crate) values: Map<String, Value>,
    /// The text of the number of `k`, as written.
    pub(crate) k_text: Option<String>,
}

/// Reads one line as a JSON object whose keys are all in `known` and none twice. `Err` carries the `id` for the answer
/// `bad_request`: the `id` of the line if it is a JSON object with exactly one valid `id`.
pub(crate) fn read_fields(line: &[u8], known: &[&str]) -> Result<Fields, Option<Value>> {
    let Ok(Pairs(raw)) = serde_json::from_slice::<Pairs>(line) else {
        return Err(None);
    };
    // A value nested deeper than serde_json reads makes the line invalid.
    let Ok(pairs) = raw
        .iter()
        .map(|(key, text)| serde_json::from_str(text.get()).map(|value| (key.clone(), value)))
        .collect::<Result<Vec<(String, Value)>, _>>()
    else {
        return Err(None);
    };
    let mut ids = pairs.iter().filter(|(key, _)| key == "id");
    let id = match (ids.next(), ids.next()) {
        (Some((_, id)), None) if is_id(id) => Some(id.clone()),
        _ => None,
    };
    let mut keys = HashSet::new();
    let unique = pairs.iter().all(|(key, _)| keys.insert(key.as_str()));
    let known = pairs.iter().all(|(key, _)| known.contains(&key.as_str()));
    if !(unique && known) {
        return Err(id);
    }
    let k_text = raw
        .iter()
        .find(|(key, _)| key == "k")
        .map(|(_, text)| text.get().to_owned());
    Ok(Fields {
        id,
        values: pairs.into_iter().collect(),
        k_text,
    })
}

/// Reads one line as a request. `Err` carries the `id` for the answer `bad_request`: the `id` of
/// the line if it is a JSON object with exactly one valid `id`.
///
/// # Errors
///
/// The `id` (if any) of a line that is no valid request.
pub fn parse_request(line: &[u8]) -> Result<Request, Option<Value>> {
    let Fields { id, values, k_text } = read_fields(line, &FIELDS)?;
    request_of(&values, k_text.as_deref()).map_err(|Invalid| id)
}

/// The note vectors of the start file, in memory while the file is unchanged.
#[derive(Debug)]
struct VectorFile {
    path: Option<PathBuf>,
    loaded: Option<Loaded>,
    detect: Detect,
}

/// The state of the vector file at the last look.
#[derive(Debug)]
struct Loaded {
    stamp: Stamp,
    /// The bytes the notes were read from, kept only while the stamp is in doubt.
    raw: Option<Vec<u8>>,
    notes: Vec<(String, Vec<f64>)>,
}

impl VectorFile {
    fn new(path: Option<PathBuf>, detect: Detect) -> Self {
        Self {
            path,
            loaded: None,
            detect,
        }
    }

    /// The note vectors as the file holds them now; none without a file.
    fn notes(&mut self, now: i128) -> Result<&[(String, Vec<f64>)], Unreadable> {
        let Some(path) = self.path.clone() else {
            return Ok(&[]);
        };
        self.refresh(&path, now)?;
        Ok(self
            .loaded
            .as_ref()
            .map_or(&[][..], |loaded| loaded.notes.as_slice()))
    }

    /// Looks at the file: kept if it is the one read before, read again if not.
    fn refresh(&mut self, path: &Path, now: i128) -> Result<(), Unreadable> {
        let Ok(meta) = fs::metadata(path) else {
            self.loaded = None;
            return Err(Unreadable);
        };
        let stamp = Stamp::new(&meta, now, self.detect);
        if self
            .loaded
            .as_ref()
            .is_some_and(|loaded| loaded.stamp.unchanged(&stamp))
        {
            return Ok(());
        }
        let Ok(bytes) = fs::read(path) else {
            self.loaded = None;
            return Err(Unreadable);
        };
        if let Some(loaded) = &mut self.loaded
            && loaded.raw.as_deref() == Some(bytes.as_slice())
        {
            loaded.stamp = stamp;
            loaded.raw = stamp.racy().then_some(bytes);
            return Ok(());
        }
        let Ok(notes) = parse_note_vectors(&bytes) else {
            self.loaded = None;
            return Err(Unreadable);
        };
        self.loaded = Some(Loaded {
            stamp,
            raw: stamp.racy().then_some(bytes),
            notes,
        });
        Ok(())
    }
}

/// Where the note vectors come from.
#[derive(Debug)]
enum NoteSource {
    /// The start file (or none).
    File(VectorFile),
    /// The databases.
    Dbs(DbVectors),
}

impl NoteSource {
    /// The note vectors as the source holds them now.
    fn notes(&mut self, now: i128) -> Result<&[(String, Vec<f64>)], Unreadable> {
        match self {
            Self::File(file) => file.notes(now),
            Self::Dbs(dbs) => dbs.notes(now),
        }
    }
}

/// The warm process: the vault, the index, the note vectors.
#[derive(Debug)]
pub struct Server {
    root: PathBuf,
    warm: Warm,
    analyzer: Analyzer,
    vectors: NoteSource,
    /// The call to Ollama for a request without `query_vector`; none without `--embed`.
    embedder: Option<Embedder>,
}

impl Server {
    /// A server for the vault at `root` and the vector file `vectors`, with the change detection
    /// of the platform. The notes are read at once; a vault that cannot be read now is tried again
    /// with every search.
    #[must_use]
    pub fn start(root: PathBuf, vectors: Option<PathBuf>) -> Self {
        Self::start_with(root, vectors, Detect::native())
    }

    /// As [`Server::start`], with the given change detection.
    #[must_use]
    pub fn start_with(root: PathBuf, vectors: Option<PathBuf>, detect: Detect) -> Self {
        Self::with_source(
            root,
            NoteSource::File(VectorFile::new(vectors, detect)),
            detect,
        )
    }

    /// A server for the vault at `root` whose note vectors come from the databases `dbs`, with the
    /// change detection of the platform.
    #[must_use]
    pub fn start_dbs(root: PathBuf, dbs: DbPaths) -> Self {
        Self::start_dbs_with(root, dbs, Detect::native())
    }

    /// As [`Server::start_dbs`], with the given change detection.
    #[must_use]
    pub fn start_dbs_with(root: PathBuf, dbs: DbPaths, detect: Detect) -> Self {
        Self::with_source(root, NoteSource::Dbs(DbVectors::new(dbs, detect)), detect)
    }

    fn with_source(root: PathBuf, vectors: NoteSource, detect: Detect) -> Self {
        let mut server = Self {
            root,
            warm: Warm::new(detect),
            analyzer: Analyzer::new(),
            vectors,
            embedder: None,
        };
        let _ = server
            .warm
            .refresh(&server.root, &mut server.analyzer, now_ns());
        server
    }

    /// The same server, which embeds the question of a request without `query_vector` with `embedder`
    /// (`--embed`); `None` leaves such a request lexical, as before.
    #[must_use]
    pub fn with_embedder(mut self, embedder: Option<Embedder>) -> Self {
        self.embedder = embedder;
        self
    }

    /// The note the index holds at `rel`, as of the last refresh (every search refreshes first).
    #[must_use]
    pub fn note(&self, rel: &str) -> Option<&NoteRec> {
        self.warm
            .korpus()
            .entry_notes()
            .find(|note| note.rel == rel)
    }

    /// The notes the index holds, by path, as of the last refresh. A note left out of the corpus is in it too.
    #[must_use]
    pub fn notes_by_rel(&self) -> HashMap<&str, &NoteRec> {
        self.warm
            .korpus()
            .entry_notes()
            .map(|note| (note.rel.as_str(), note))
            .collect()
    }

    /// The vault this server reads.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The answer to a search: the object `rangkern search` prints, or `Err` when the vault, a
    /// note of the corpus or (for a request with a vector) the vector file cannot be read.
    ///
    /// # Errors
    ///
    /// [`Unreadable`] as described.
    pub fn search(&mut self, request: &SearchRequest) -> Result<Value, Unreadable> {
        let (hits, fallback) = self.search_hits(request, now_ns())?;
        Ok(json!({
            "fallback": fallback,
            "hits": hits.iter().map(hit_json).collect::<Vec<_>>(),
        }))
    }

    /// The hits of a search and whether the lexical way was taken, with the clock set to `now`
    /// (nanoseconds since the epoch; a test passes a later time to take files as settled).
    ///
    /// # Errors
    ///
    /// [`Unreadable`] when the vault, a note of the corpus or (for a request with a vector) the
    /// vector file cannot be read.
    pub fn search_hits(
        &mut self,
        request: &SearchRequest,
        now: i128,
    ) -> Result<(Vec<SearchHit>, bool), Unreadable> {
        let today_text = request.today.clone().unwrap_or_else(today);
        self.warm.refresh(&self.root, &mut self.analyzer, now)?;
        let mut params = Params {
            k: request.k,
            vectors: None,
            space: request.space.as_deref(),
            today: &today_text,
        };
        if let Some(query) = &request.query_vector {
            let notes = self.vectors.notes(now)?;
            params.vectors = Some(VectorView { query, notes });
        } else if let Some(embedder) = &self.embedder {
            let notes = self.vectors.notes(now)?;
            return Ok(rank_embedding(
                &self.root,
                self.warm.korpus(),
                &mut self.analyzer,
                &request.query,
                &params,
                (embedder, notes),
            ));
        }
        Ok(rank(
            &self.root,
            self.warm.korpus(),
            &mut self.analyzer,
            &request.query,
            &params,
        ))
    }

    /// The answer to one line, and whether the process ends after it.
    pub fn answer(&mut self, line: &Line) -> (Value, bool) {
        if line.too_long {
            return (bad_request(None), false);
        }
        let request = match parse_request(&line.bytes) {
            Ok(request) => request,
            Err(id) => return (bad_request(id), false),
        };
        let (mut body, quit) = match &request.op {
            Op::Ping => (json!({"ok": true}), false),
            Op::Quit => (json!({"ok": true}), true),
            Op::Search(search) => (
                self.search(search)
                    .unwrap_or_else(|_| json!({"error": "unreadable"})),
                false,
            ),
        };
        if let (Some(id), Some(object)) = (request.id, body.as_object_mut()) {
            object.insert("id".to_string(), id);
        }
        (body, quit)
    }
}

/// The answer to a line that is no valid request.
pub(crate) fn bad_request(id: Option<Value>) -> Value {
    let mut body = json!({"error": "bad_request"});
    if let (Some(id), Some(object)) = (id, body.as_object_mut()) {
        object.insert("id".to_string(), id);
    }
    body
}

/// Answers the lines of `input` one by one until the end of the input or `quit`. Returns the exit
/// code: 0, or 3 when an answer cannot be written.
pub fn serve_lines<R: BufRead, O: Write>(server: &mut Server, input: &mut R, out: &mut O) -> u8 {
    while let Ok(Some(line)) = next_line(input) {
        let (body, quit) = server.answer(&line);
        let written = serde_json::to_writer(&mut *out, &body).is_ok()
            && out.write_all(b"\n").is_ok()
            && out.flush().is_ok();
        if !written {
            return EXIT_UNREADABLE;
        }
        if quit {
            break;
        }
    }
    EXIT_OK
}
