//! Frontmatter blocks and fields: a small reader in which every value stays text.
//!
//! A YAML library would turn `yes`, `0755` or `2026-09-28` into other types, so the reader is
//! hand-written, as in the Python model.

use std::collections::BTreeMap;

use crate::text::{is_space, split_lines, strip};

/// A frontmatter value. Scalars stay text; `key:` without a value is an empty list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Field {
    /// A scalar, possibly wrapped onto continuation lines.
    Text(String),
    /// A flow list `[a, b]`, a block list of `- x` lines, or an empty value.
    List(Vec<String>),
}

impl Field {
    /// The text of a non-empty scalar; `None` for lists and the empty string.
    #[must_use]
    pub fn non_empty_text(&self) -> Option<&str> {
        match self {
            Field::Text(text) if !text.is_empty() => Some(text),
            Field::Text(_) | Field::List(_) => None,
        }
    }
}

/// The merged fields of a note, by key.
pub type Fields = BTreeMap<String, Field>;

/// A space or a tab: the separator after the colon in the schema reader.
#[must_use]
pub fn is_blank(c: char) -> bool {
    matches!(c, ' ' | '\t')
}

/// Matches `^([A-Za-z0-9_-]+):<ws>*(.*)$` and returns the key and the value after the separator.
#[must_use]
pub fn key_match(line: &str, is_ws: fn(char) -> bool) -> Option<(&str, &str)> {
    let key_len = line
        .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '-'))
        .unwrap_or(line.len());
    if key_len == 0 {
        return None;
    }
    let value = line[key_len..].strip_prefix(':')?;
    Some((&line[..key_len], value.trim_start_matches(is_ws)))
}

/// Cuts a trailing comment: from the first whitespace run of at least `min` characters that is
/// followed by `#`, to the end.
#[must_use]
pub fn cut_comment(s: &str, min: usize) -> &str {
    let mut run = 0;
    let mut run_start = 0;
    for (i, c) in s.char_indices() {
        if is_space(c) {
            if run == 0 {
                run_start = i;
            }
            run += 1;
        } else {
            if c == '#' && run >= min {
                return &s[..run_start];
            }
            run = 0;
        }
    }
    s
}

/// Resolves the escapes `\n \t \r \" \\` of a double-quoted scalar; any other backslash stays.
fn unescape(inner: &str) -> String {
    let mut out = String::with_capacity(inner.len());
    let mut chars = inner.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' {
            let mapped = match chars.peek() {
                Some('n') => Some('\n'),
                Some('t') => Some('\t'),
                Some('r') => Some('\r'),
                Some('"') => Some('"'),
                Some('\\') => Some('\\'),
                _ => None,
            };
            if let Some(mapped) = mapped {
                chars.next();
                out.push(mapped);
                continue;
            }
        }
        out.push(c);
    }
    out
}

/// The text of one scalar: quotes removed, double-quoted escapes resolved.
#[must_use]
pub fn scalar(raw: &str) -> String {
    let s = strip(raw);
    if let Some(inner) = s.strip_prefix('"').and_then(|r| r.strip_suffix('"')) {
        return unescape(inner);
    }
    if let Some(inner) = s.strip_prefix('\'').and_then(|r| r.strip_suffix('\'')) {
        return inner.replace("''", "'");
    }
    strip(s.trim_matches(['"', '\''])).to_string()
}

/// The first block of `candidate` as `(content, end offset after the closing line)`.
///
/// A block opens with `---` at the start and closes at the next line that is `---`; both
/// delimiters may carry trailing spaces or tabs.
fn first_block(candidate: &str) -> Option<(&str, usize)> {
    let inner = candidate
        .strip_prefix("---")?
        .trim_start_matches([' ', '\t'])
        .strip_prefix('\n')?;
    for (i, _) in inner.match_indices('\n') {
        let Some(after) = inner[i + 1..].strip_prefix("---") else {
            continue;
        };
        let after = after.trim_start_matches([' ', '\t']);
        if after.is_empty() || after.starts_with('\n') {
            let consumed_newline = usize::from(!after.is_empty());
            return Some((
                &inner[..i],
                candidate.len() - after.len() + consumed_newline,
            ));
        }
    }
    None
}

/// True when the first line of the block that is neither empty nor a comment is `key: value`.
fn looks_like_yaml(block: &str) -> bool {
    split_lines(block)
        .into_iter()
        .find(|line| !strip(line).is_empty() && !strip(line).starts_with('#'))
        .is_some_and(|line| key_match(line, is_space).is_some())
}

/// The leading frontmatter blocks without delimiters, and the rest of the text.
///
/// Only newlines are skipped before a further block. With `skip_bom`, leading U+FEFF characters
/// are dropped first, as the schema reader does.
#[must_use]
pub fn split_blocks(text: &str, skip_bom: bool) -> (Vec<&str>, &str) {
    let mut rest = if skip_bom {
        text.trim_start_matches('\u{feff}')
    } else {
        text
    };
    let mut blocks = Vec::new();
    loop {
        let candidate = rest.trim_start_matches('\n');
        match first_block(candidate) {
            Some((block, end)) if looks_like_yaml(block) => {
                blocks.push(block);
                rest = &candidate[end..];
            }
            _ => return (blocks, rest),
        }
    }
}

/// The value of `key: raw`: empty list, flow list, or scalar.
fn value_of(raw: &str) -> Field {
    if raw.is_empty() {
        return Field::List(Vec::new());
    }
    if let Some(inner) = raw.strip_prefix('[').and_then(|r| r.strip_suffix(']')) {
        return Field::List(
            strip(inner)
                .split(',')
                .filter(|item| !strip(item).is_empty())
                .map(scalar)
                .collect(),
        );
    }
    Field::Text(scalar(raw))
}

/// Applies an indented or `-` line to the field `key` of the current block.
fn continue_field(fields: &mut Fields, key: &str, stripped: &str) {
    if let Some(item) = stripped.strip_prefix("- ") {
        let item = scalar(cut_comment(item, 2));
        match fields.get_mut(key) {
            Some(Field::List(list)) => list.push(item),
            _ => {
                fields.insert(key.to_string(), Field::List(vec![item]));
            }
        }
    } else if let Some(Field::Text(text)) = fields.get_mut(key) {
        let joined = format!("{text} {}", strip(cut_comment(stripped, 2)));
        *text = strip(&joined).to_string();
    }
}

/// Merges the blocks into one field map. Later blocks and later lines override earlier values.
#[must_use]
pub fn parse_fields(blocks: &[&str]) -> Fields {
    let mut fields = Fields::new();
    for block in blocks {
        let mut key: Option<String> = None;
        for line in split_lines(block) {
            let stripped = strip(line);
            if stripped.is_empty() || stripped.starts_with('#') {
                continue;
            }
            if let (Some(current), Some(' ' | '\t' | '-')) = (&key, line.chars().next()) {
                continue_field(&mut fields, current, stripped);
                continue;
            }
            let Some((name, value)) = key_match(line, is_space) else {
                key = None;
                continue;
            };
            fields.insert(name.to_string(), value_of(strip(cut_comment(value, 2))));
            key = Some(name.to_string());
        }
    }
    fields
}
