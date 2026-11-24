//! What the search reads from a vault: the notes of the search corpus, the space manifest, the
//! open contradictions, and the file with the fixed vectors.
//!
//! Which paths are notes, and everything about excluded paths, comes from `notiz-kern`; this module
//! adds only the exclusion of originals of a source. Nothing here writes.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::io;
use std::path::Path;

use notiz_kern::corpus;
use notiz_kern::text::read_text;
use serde::Deserialize;
use serde::de::{Deserializer, IgnoredAny, MapAccess, Visitor};
use serde_json::Value;

use crate::notes::{NoteRec, has_value, note_rec};

/// The space manifest of a vault.
const LOCK_FILE: &str = ".brain/spaces.lock";
/// The open findings of the contradiction tool.
const CONTRADICTIONS_FILE: &str = "_meta/state/contradictions.json";
/// The manifest of a space.
const SPACE_FILE: &str = ".space.yml";
/// A manifest larger than this is not read.
const MANIFEST_MAX: usize = 64 * 1024;
/// The folder of originals in a space of the kind `quellen`.
const ORIGINAL_DIR: &str = "quellen";
/// The one file in that folder that is never an original.
const UNREADABLE_NOTE: &str = "NICHT-LESBAR.md";

/// The vault, a file or the vector file cannot be read.
#[derive(Debug)]
pub struct Unreadable;

impl From<io::Error> for Unreadable {
    fn from(_: io::Error) -> Self {
        Unreadable
    }
}

/// Whether the space at `dir` is of the kind `quellen`, from its `.space.yml` (JSON text).
pub(crate) fn is_source_space(dir: &Path) -> bool {
    let Ok(bytes) = fs::read(dir.join(SPACE_FILE)) else {
        return false;
    };
    if bytes.len() > MANIFEST_MAX {
        return false;
    }
    let body = bytes.strip_prefix(b"\xef\xbb\xbf").unwrap_or(&bytes);
    let Ok(Value::Object(map)) = serde_json::from_slice::<Value>(body) else {
        return false;
    };
    map.get("art").and_then(Value::as_str) == Some("quellen")
}

/// Whether the file is an original of a source: below `quellen/` of a space of the kind
/// `quellen`, and without a `quelle:` field in its head. `quellen/NICHT-LESBAR.md` is no original.
pub(crate) fn is_original(root: &Path, note: &NoteRec, kinds: &mut HashMap<String, bool>) -> bool {
    let parts: Vec<&str> = note.rel.split('/').collect();
    let Some((_, dirs)) = parts.split_last() else {
        return false;
    };
    for (i, part) in dirs.iter().enumerate() {
        if *part != ORIGINAL_DIR {
            continue;
        }
        let base = parts[..i].join("/");
        let source = *kinds.entry(base.clone()).or_insert_with(|| {
            let dir = if base.is_empty() {
                root.to_path_buf()
            } else {
                root.join(&base)
            };
            is_source_space(&dir)
        });
        if !source {
            continue;
        }
        if parts[i..] == [ORIGINAL_DIR, UNREADABLE_NOTE] {
            return false;
        }
        return !has_value(note.fields.get("quelle"));
    }
    false
}

/// Reads the note at `rel` below `root`.
///
/// # Errors
///
/// [`Unreadable`] when the file cannot be read.
pub(crate) fn read_note(root: &Path, rel: &str) -> Result<NoteRec, Unreadable> {
    let text = read_text(&root.join(rel))?;
    Ok(note_rec(rel, text))
}

/// The paths of the search corpus in corpus order. A vault directory that cannot be listed gives
/// an empty corpus, as the Python model does (its directory walk swallows the error); the caller
/// has checked that the path is a directory.
pub(crate) fn corpus_rels(root: &Path) -> Vec<String> {
    corpus(root).map_or_else(|_| Vec::new(), |listing| listing.search)
}

/// The notes of the search corpus in corpus order, originals of a source left out.
///
/// A vault directory that cannot be listed gives an empty corpus, as the Python model does (its
/// directory walk swallows the error); the caller has checked that the path is a directory.
///
/// # Errors
///
/// [`Unreadable`] when a note of the corpus cannot be read.
pub fn load_search_notes(root: &Path) -> Result<Vec<NoteRec>, Unreadable> {
    let rels = corpus_rels(root);
    let mut notes = Vec::with_capacity(rels.len());
    for rel in &rels {
        notes.push(read_note(root, rel)?);
    }
    let mut kinds = HashMap::new();
    notes.retain(|note| !is_original(root, note, &mut kinds));
    Ok(notes)
}

/// The mounts of the spaces, in manifest order: `(mount without slashes, space name)`.
#[derive(Clone, Debug, Default)]
pub struct Manifest {
    entries: Vec<(String, String)>,
}

/// Python's `str(value)` for the scalars a manifest can hold; `None` for lists and objects.
pub(crate) fn scalar_text(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => Some(text.clone()),
        Value::Null => Some("None".to_string()),
        Value::Bool(true) => Some("True".to_string()),
        Value::Bool(false) => Some("False".to_string()),
        Value::Number(number) => Some(number.to_string()),
        Value::Array(_) | Value::Object(_) => None,
    }
}

impl Manifest {
    /// Reads `.brain/spaces.lock`; a missing or invalid file is an empty manifest.
    #[must_use]
    pub fn load(root: &Path) -> Self {
        let Ok(bytes) = fs::read(root.join(LOCK_FILE)) else {
            return Self::default();
        };
        let Ok(text) = String::from_utf8(bytes) else {
            return Self::default();
        };
        let Ok(Value::Object(top)) = serde_json::from_str::<Value>(&text) else {
            return Self::default();
        };
        let Some(Value::Array(list)) = top.get("spaces") else {
            return Self::default();
        };
        let entries = list
            .iter()
            .filter_map(|entry| {
                let entry = entry.as_object()?;
                let mount = match entry.get("mount") {
                    None => String::new(),
                    Some(value) => scalar_text(value)?,
                };
                let space = entry.get("space")?.as_str()?;
                Some((mount.trim_matches('/').to_string(), space.to_string()))
            })
            .collect();
        Self { entries }
    }

    /// The space of a path: the entry with the longest mount that is a path prefix, else `kern`.
    #[must_use]
    pub fn space_for(&self, rel: &str) -> &str {
        let rel = rel.trim_matches('/');
        let mut best: Option<(&str, usize)> = None;
        for (mount, space) in &self.entries {
            if mount.is_empty() {
                continue;
            }
            let inside = rel == mount
                || rel
                    .strip_prefix(mount.as_str())
                    .is_some_and(|r| r.starts_with('/'));
            if inside && best.is_none_or(|(_, len)| mount.len() > len) {
                best = Some((space.as_str(), mount.len()));
            }
        }
        best.map_or("kern", |(space, _)| space)
    }
}

/// The two notes of every open or escalated finding of the contradiction tool, as the finding names them (`None` for a
/// side without a path); any problem with the file means no known contradictions.
#[must_use]
pub fn contradiction_pairs(root: &Path) -> Vec<(Option<String>, Option<String>)> {
    let Ok(bytes) = fs::read(root.join(CONTRADICTIONS_FILE)) else {
        return Vec::new();
    };
    let Ok(text) = String::from_utf8(bytes) else {
        return Vec::new();
    };
    let Ok(Value::Object(top)) = serde_json::from_str::<Value>(&text) else {
        return Vec::new();
    };
    let Some(Value::Object(findings)) = top.get("findings") else {
        return Vec::new();
    };
    let side = |finding: &serde_json::Map<String, Value>, name: &str| {
        finding
            .get(name)
            .and_then(Value::as_object)
            .and_then(|note| note.get("rel"))
            .and_then(Value::as_str)
            .filter(|rel| !rel.is_empty())
            .map(str::to_owned)
    };
    findings
        .values()
        .filter_map(Value::as_object)
        .filter(|finding| {
            matches!(
                finding.get("status").and_then(Value::as_str),
                Some("open" | "escalated")
            )
        })
        .map(|finding| (side(finding, "note_a"), side(finding, "note_b")))
        .collect()
}

/// The paths named by an open or escalated finding of the contradiction tool; any problem with
/// the file means no known contradictions.
#[must_use]
pub fn contradicted_rels(root: &Path) -> HashSet<String> {
    contradiction_pairs(root)
        .into_iter()
        .flat_map(|(a, b)| [a, b])
        .flatten()
        .collect()
}

/// The fixed vectors: the query vector and the note vectors in the order of the file.
#[derive(Clone, Debug)]
pub struct Vectors {
    /// The vector of the question.
    pub query: Vec<f64>,
    /// `(path, vector)` in file order; a repeated path keeps its first place and its last vector.
    pub notes: Vec<(String, Vec<f64>)>,
}

/// The fixed vectors as the search reads them: borrowed, so that the note vectors of a long-lived
/// process need no copy for one question.
#[derive(Clone, Copy, Debug)]
pub struct VectorView<'a> {
    /// The vector of the question.
    pub query: &'a [f64],
    /// `(path, vector)` in file order.
    pub notes: &'a [(String, Vec<f64>)],
}

impl Vectors {
    /// The borrowed form.
    #[must_use]
    pub fn view(&self) -> VectorView<'_> {
        VectorView {
            query: &self.query,
            notes: &self.notes,
        }
    }
}

/// A JSON number, and nothing else (no boolean, null or text), as a float.
struct Number(f64);

impl<'de> Deserialize<'de> for Number {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct NumberVisitor;
        impl Visitor<'_> for NumberVisitor {
            type Value = Number;
            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("a number")
            }
            fn visit_f64<E>(self, v: f64) -> Result<Number, E> {
                Ok(Number(v))
            }
            #[allow(clippy::cast_precision_loss)]
            // Python's float(int) rounds to the nearest double, as this cast does.
            fn visit_i64<E>(self, v: i64) -> Result<Number, E> {
                Ok(Number(v as f64))
            }
            #[allow(clippy::cast_precision_loss)]
            // Python's float(int) rounds to the nearest double, as this cast does.
            fn visit_u64<E>(self, v: u64) -> Result<Number, E> {
                Ok(Number(v as f64))
            }
        }
        deserializer.deserialize_any(NumberVisitor)
    }
}

/// A list of numbers.
struct Vector(Vec<f64>);

impl<'de> Deserialize<'de> for Vector {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let numbers = Vec::<Number>::deserialize(deserializer)?;
        Ok(Vector(numbers.into_iter().map(|n| n.0).collect()))
    }
}

/// An object of lists of numbers that keeps the order of its keys.
struct Notes(Vec<(String, Vec<f64>)>);

impl<'de> Deserialize<'de> for Notes {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct NotesVisitor;
        impl<'de> Visitor<'de> for NotesVisitor {
            type Value = Notes;
            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("an object of lists of numbers")
            }
            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Notes, A::Error> {
                let mut entries: Vec<(String, Vec<f64>)> = Vec::new();
                let mut at: HashMap<String, usize> = HashMap::new();
                while let Some(key) = map.next_key::<String>()? {
                    let Vector(vector) = map.next_value()?;
                    if let Some(&i) = at.get(&key) {
                        entries[i].1 = vector;
                    } else {
                        at.insert(key.clone(), entries.len());
                        entries.push((key, vector));
                    }
                }
                Ok(Notes(entries))
            }
        }
        deserializer.deserialize_map(NotesVisitor)
    }
}

impl<'de> Deserialize<'de> for Vectors {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct VectorsVisitor;
        impl<'de> Visitor<'de> for VectorsVisitor {
            type Value = Vectors;
            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("an object with `query` and `notes`")
            }
            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Vectors, A::Error> {
                let mut query = None;
                let mut notes = None;
                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "query" => query = Some(map.next_value::<Vector>()?.0),
                        "notes" => notes = Some(map.next_value::<Notes>()?.0),
                        _ => {
                            map.next_value::<IgnoredAny>()?;
                        }
                    }
                }
                match (query, notes) {
                    (Some(query), Some(notes)) => Ok(Vectors { query, notes }),
                    _ => Err(serde::de::Error::custom("`query` and `notes` are required")),
                }
            }
        }
        deserializer.deserialize_map(VectorsVisitor)
    }
}

/// Reads one vector from the bytes of a JSON text: a list of numbers, read by the rules of the
/// vector file (no `NaN`, no `Infinity`, nothing after the list).
///
/// # Errors
///
/// [`Unreadable`] when the bytes are not a JSON list of numbers.
pub fn parse_vector(bytes: &[u8]) -> Result<Vec<f64>, Unreadable> {
    serde_json::from_slice::<Vector>(bytes)
        .map(|vector| vector.0)
        .map_err(|_| Unreadable)
}

/// Reads the file of a query vector: a JSON list of numbers that is not empty.
///
/// # Errors
///
/// [`Unreadable`] when the file cannot be read, is no JSON list of numbers, or the list is empty.
pub fn load_query_vector(path: &Path) -> Result<Vec<f64>, Unreadable> {
    let vector = parse_vector(&fs::read(path)?)?;
    if vector.is_empty() {
        return Err(Unreadable);
    }
    Ok(vector)
}

/// Reads the vector file.
///
/// # Errors
///
/// [`Unreadable`] when the file cannot be read or is not JSON of the form
/// `{"query": [number, ...], "notes": {"<path>": [number, ...], ...}}`.
pub fn load_vectors(path: &Path) -> Result<Vectors, Unreadable> {
    let bytes = fs::read(path)?;
    serde_json::from_slice(&bytes).map_err(|_| Unreadable)
}

/// The part `notes` of a vector file; the key `query` may stand beside it and is not used.
#[derive(Debug)]
pub(crate) struct NoteVectors(pub(crate) Vec<(String, Vec<f64>)>);

impl<'de> Deserialize<'de> for NoteVectors {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct FileVisitor;
        impl<'de> Visitor<'de> for FileVisitor {
            type Value = NoteVectors;
            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("an object with `notes`")
            }
            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<NoteVectors, A::Error> {
                let mut notes = None;
                while let Some(key) = map.next_key::<String>()? {
                    if key == "notes" {
                        notes = Some(map.next_value::<Notes>()?.0);
                    } else {
                        map.next_value::<IgnoredAny>()?;
                    }
                }
                notes
                    .map(NoteVectors)
                    .ok_or_else(|| serde::de::Error::custom("`notes` is required"))
            }
        }
        deserializer.deserialize_map(FileVisitor)
    }
}

/// Reads the note vectors from the bytes of a vector file.
///
/// # Errors
///
/// [`Unreadable`] when the bytes are not JSON of the form `{"notes": {"<path>": [number, ...], ...}}`.
pub fn parse_note_vectors(bytes: &[u8]) -> Result<Vec<(String, Vec<f64>)>, Unreadable> {
    serde_json::from_slice::<NoteVectors>(bytes)
        .map(|file| file.0)
        .map_err(|_| Unreadable)
}
