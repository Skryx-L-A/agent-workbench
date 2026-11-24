//! Python-compatible text helpers: whitespace, line splitting, file reading and comparison keys.

use std::fs;
use std::io;
use std::path::Path;

use caseless::default_case_fold_str;
use unicode_normalization::UnicodeNormalization;

/// Python's `str.isspace()` for one character.
#[must_use]
pub fn is_space(c: char) -> bool {
    c.is_whitespace() || ('\u{1c}'..='\u{1f}').contains(&c)
}

/// True when `name` ends in the exact, case-sensitive suffix `.md`, as Python's `endswith(".md")`.
/// A file named `.md` counts; `NOTE.MD` does not.
#[must_use]
pub fn has_md_suffix(name: &str) -> bool {
    // Python compares case-sensitively and `Path::extension` would reject the name `.md`, so the
    // lint that prefers a case-insensitive extension check does not apply here.
    #[allow(clippy::case_sensitive_file_extension_comparisons)]
    name.ends_with(".md")
}

/// Python's `str.strip()` without argument.
#[must_use]
pub fn strip(s: &str) -> &str {
    s.trim_matches(is_space)
}

/// The characters at which Python's `str.splitlines()` ends a line.
fn is_break(c: char) -> bool {
    matches!(
        c,
        '\n' | '\r'
            | '\u{b}'
            | '\u{c}'
            | '\u{1c}'
            | '\u{1d}'
            | '\u{1e}'
            | '\u{85}'
            | '\u{2028}'
            | '\u{2029}'
    )
}

/// Python's `str.splitlines(keepends=True)`.
#[must_use]
pub fn split_keep(s: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut start = 0;
    let mut chars = s.char_indices().peekable();
    while let Some((i, c)) = chars.next() {
        if !is_break(c) {
            continue;
        }
        let mut end = i + c.len_utf8();
        if c == '\r' && chars.next_if(|&(_, next)| next == '\n').is_some() {
            end += 1;
        }
        out.push(&s[start..end]);
        start = end;
    }
    if start < s.len() {
        out.push(&s[start..]);
    }
    out
}

/// Python's `str.splitlines()`.
#[must_use]
pub fn split_lines(s: &str) -> Vec<&str> {
    split_keep(s)
        .into_iter()
        .map(|line| line.trim_end_matches(is_break))
        .collect()
}

/// Reads a file the way Python's text mode does: lossy UTF-8, then `\r\n` and `\r` become `\n`.
///
/// # Errors
///
/// Returns the I/O error when the path is missing, a directory, or not readable.
pub fn read_text(path: &Path) -> io::Result<String> {
    let bytes = fs::read(path)?;
    Ok(String::from_utf8_lossy(&bytes)
        .replace("\r\n", "\n")
        .replace('\r', "\n"))
}

/// Comparison key of a title, file stem or link target: NFC, stripped, lower case.
#[must_use]
pub fn key_of(value: &str) -> String {
    strip(&value.nfc().collect::<String>()).to_lowercase()
}

/// Comparison key for the exclusion rules of the corpus: NFKC, full Unicode case folding, NFKC.
///
/// Python compares with `str.casefold()`, which is the full folding (`ſ` and `ẞ` fold to `s` and
/// `ss`) but does not normalise. A case-insensitive file system folds names on its own terms
/// (APFS folds by Unicode and stores names decomposed), so a name given from outside could open an
/// excluded folder through a spelling that `to_lowercase` does not see. The key is therefore never
/// less strict than `casefold`: NFKC first joins decomposed forms (`a` plus U+0308 becomes `ä`) and
/// compatibility forms (full-width letters and dots, ligatures), the folding follows, and a final
/// NFKC makes the result stable, as the Unicode standard prescribes for compatibility caseless
/// matching. NFKC rather than NFC because the stricter key can only exclude more, and this key
/// guards secrets; it is never used for titles or links (those keep [`key_of`]).
#[must_use]
pub fn exclusion_key(name: &str) -> String {
    default_case_fold_str(&name.nfkc().collect::<String>())
        .nfkc()
        .collect()
}
