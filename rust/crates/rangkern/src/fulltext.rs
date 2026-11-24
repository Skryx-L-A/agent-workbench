//! The full-text fallback, without a process: the files `rg -i -l --glob '!90-secrets/**'
//! --glob '*.md' -e <question> -- <vault>` would list, but only from the search corpus.
//!
//! Measured on `rg` 15.2.0 (see the result report): a question is a regular expression of the
//! `regex` crate, searched line by line (lines end at `\n` only, so `$` does not match before a
//! `\r`, and `\s` and `[^x]` never cross a line); the glob `*.md` also lists hidden files and
//! overrides `.ignore` files; a file with a NUL byte is binary and not listed; a UTF-8 byte order
//! mark is dropped and a UTF-16 file with a byte order mark is decoded first; invalid UTF-8 does
//! not stop the search. A question that is not a valid expression lists nothing.

use std::fs;
use std::path::Path;

use regex::bytes::RegexBuilder;

use crate::notes::NoteRec;

/// The text `rg` searches: the bytes without a byte order mark; a UTF-16 file is decoded to UTF-8.
pub(crate) fn searchable(bytes: Vec<u8>) -> Vec<u8> {
    let utf16 = |rest: &[u8], big_endian: bool| -> Vec<u8> {
        let units = rest.chunks(2).map(|pair| match *pair {
            [a, b] if big_endian => u16::from_be_bytes([a, b]),
            [a, b] => u16::from_le_bytes([a, b]),
            [a] => u16::from(a),
            _ => 0,
        });
        char::decode_utf16(units)
            .map(|unit| unit.unwrap_or(char::REPLACEMENT_CHARACTER))
            .collect::<String>()
            .into_bytes()
    };
    if let Some(rest) = bytes.strip_prefix(&[0xff, 0xfe]) {
        utf16(rest, false)
    } else if let Some(rest) = bytes.strip_prefix(&[0xfe, 0xff]) {
        utf16(rest, true)
    } else if let Some(rest) = bytes.strip_prefix(&[0xef, 0xbb, 0xbf]) {
        rest.to_vec()
    } else {
        bytes
    }
}

/// The lines of the data: separated by `\n`, the last one only if it is not empty.
pub(crate) fn lines(data: &[u8]) -> impl Iterator<Item = &[u8]> {
    let mut parts: Vec<&[u8]> = data.split(|&b| b == b'\n').collect();
    if parts.last().is_some_and(|last| last.is_empty()) {
        parts.pop();
    }
    parts.into_iter()
}

/// The paths of the corpus notes with a line that matches the question (case-insensitive),
/// sorted by the bytes of the path.
#[must_use]
pub fn fulltext_rels<'a>(
    root: &Path,
    notes: impl IntoIterator<Item = &'a NoteRec>,
    query: &str,
) -> Vec<String> {
    let Ok(regex) = RegexBuilder::new(query).case_insensitive(true).build() else {
        return Vec::new();
    };
    let mut found: Vec<String> = notes
        .into_iter()
        .filter(|note| {
            let Ok(bytes) = fs::read(root.join(&note.rel)) else {
                return false;
            };
            let data = searchable(bytes);
            !data.contains(&0) && lines(&data).any(|line| regex.is_match(line))
        })
        .map(|note| note.rel.clone())
        .collect();
    found.sort_by(|a, b| a.as_bytes().cmp(b.as_bytes()));
    found
}
