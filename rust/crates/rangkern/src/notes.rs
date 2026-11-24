//! The note of the search corpus as the ranking sees it: path, title, text, frontmatter fields.
//!
//! The values of the fields follow Python's `str()` and truthiness: a list that is not empty is
//! never equal to a plain word, an empty one counts as no value.

use notiz_kern::frontmatter::{
    Field, Fields, cut_comment, key_match, parse_fields, scalar, split_blocks,
};
use notiz_kern::text::{is_space, key_of, split_lines, strip};
use notiz_kern::{path_parts, stem_of};

use crate::pystr::repr_list;

/// A note of the search corpus.
#[derive(Clone, Debug)]
pub struct NoteRec {
    /// The vault-relative path with `/`.
    pub rel: String,
    /// The title: the field `title`, else the file stem.
    pub title: String,
    /// The whole file text.
    pub text: String,
    /// The merged frontmatter fields.
    pub fields: Fields,
    /// The comparison keys the note answers to as a link target: stem, title, aliases.
    pub keys: Vec<String>,
    /// The field `datenklasse` as the frontmatter holds it, by type.
    pub datenklasse: Raw,
    /// The field `kunde` as the frontmatter holds it, by type.
    pub kunde: Raw,
}

/// The raw value of a frontmatter field, by type. The reader of `notiz-kern` keeps every scalar as
/// text; this type tells a quoted `"5"` (text) from a plain `5` (a number), and a missing field
/// from one without a value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Raw {
    /// The note has no such field.
    Missing,
    /// The field has no value: `key:`, `key: null`, `key: ~`.
    Null,
    /// A text: plain, or in quotes, and possibly empty (`key: ""`).
    Text(String),
    /// Any other type: a number, a truth value, a list (also an empty one), a mapping.
    Other,
}

impl Raw {
    /// The value as the gate takes it: no value (missing or null) is `None`; a text is itself, the
    /// empty text included; a value of another type is the empty text, which no class and no
    /// customer of the settings has.
    #[must_use]
    pub fn for_gate(&self) -> Option<&str> {
        match self {
            Raw::Missing | Raw::Null => None,
            Raw::Text(text) => Some(text),
            Raw::Other => Some(""),
        }
    }

    /// The value as a customer key for the log line: the text if it has the form of a key
    /// (`^[a-z0-9][a-z0-9._-]{0,63}$`), else none.
    #[must_use]
    pub fn log_key(&self) -> Option<&str> {
        match self {
            Raw::Text(text) if is_key(text) => Some(text),
            _ => None,
        }
    }
}

/// Whether the text has the form of a key: `^[a-z0-9][a-z0-9._-]{0,63}$`.
pub(crate) fn is_key(text: &str) -> bool {
    let bytes = text.as_bytes();
    let (Some(first), rest) = (bytes.first(), bytes.get(1..).unwrap_or_default()) else {
        return false;
    };
    bytes.len() <= 64
        && matches!(first, b'a'..=b'z' | b'0'..=b'9')
        && rest
            .iter()
            .all(|b| matches!(b, b'a'..=b'z' | b'0'..=b'9' | b'.' | b'_' | b'-'))
}

/// Whether a plain scalar is a YAML null.
fn is_null(text: &str) -> bool {
    matches!(text, "null" | "Null" | "NULL" | "~")
}

/// Whether a plain scalar is a YAML truth value.
fn is_bool(text: &str) -> bool {
    matches!(text, "true" | "True" | "TRUE" | "false" | "False" | "FALSE")
}

/// Whether a plain scalar is a YAML number: a decimal integer or float with an optional sign and
/// exponent, `0x` and `0o` integers, `.inf` and `.nan`.
fn is_number(text: &str) -> bool {
    let unsigned = text.strip_prefix(['-', '+']).unwrap_or(text);
    if matches!(unsigned, ".inf" | ".Inf" | ".INF") {
        return true;
    }
    if matches!(text, ".nan" | ".NaN" | ".NAN") {
        return true;
    }
    if let Some(hex) = text.strip_prefix("0x") {
        return !hex.is_empty() && hex.bytes().all(|b| b.is_ascii_hexdigit());
    }
    if let Some(octal) = text.strip_prefix("0o") {
        return !octal.is_empty() && octal.bytes().all(|b| matches!(b, b'0'..=b'7'));
    }
    let (mantissa, exponent) = match unsigned.find(['e', 'E']) {
        Some(at) => (&unsigned[..at], Some(&unsigned[at + 1..])),
        None => (unsigned, None),
    };
    if let Some(exponent) = exponent {
        let digits = exponent.strip_prefix(['-', '+']).unwrap_or(exponent);
        if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
            return false;
        }
    }
    let (whole, fraction) = match mantissa.split_once('.') {
        Some((whole, fraction)) => (whole, Some(fraction)),
        None => (mantissa, None),
    };
    let digits = |part: &str| part.bytes().all(|b| b.is_ascii_digit());
    match fraction {
        None => !whole.is_empty() && digits(whole),
        Some(fraction) => {
            (!whole.is_empty() || !fraction.is_empty()) && digits(whole) && digits(fraction)
        }
    }
}

/// The raw value of the field `key` in the frontmatter `blocks`, whose merged fields are `fields`.
///
/// The last line `key: value` wins, as in [`parse_fields`]; the merged field tells a list and a
/// wrapped scalar, the line tells the rest.
fn raw_field(blocks: &[&str], fields: &Fields, key: &str) -> Raw {
    let mut last: Option<&str> = None;
    // `key_match` takes only a line that starts with the key; a comment, a blank line and a
    // continuation line (it starts with a space, a tab or `- `) are no key line, whatever the key.
    for block in blocks {
        for line in split_lines(block) {
            if let Some((name, value)) = key_match(line, is_space)
                && name == key
            {
                last = Some(strip(cut_comment(value, 2)));
            }
        }
    }
    let (Some(line), Some(field)) = (last, fields.get(key)) else {
        return Raw::Missing;
    };
    match field {
        Field::List(items) if !items.is_empty() => Raw::Other,
        Field::List(_) if line.is_empty() => Raw::Null,
        Field::List(_) => Raw::Other,
        Field::Text(text) => {
            let quoted = line.starts_with(['"', '\'']);
            let wrapped = scalar(line) != *text;
            if quoted || wrapped {
                Raw::Text(text.clone())
            } else if line.starts_with('{') || is_bool(text) || is_number(text) {
                Raw::Other
            } else if is_null(text) {
                Raw::Null
            } else {
                Raw::Text(text.clone())
            }
        }
    }
}

/// Python's `str(value or "")` of a field: an empty value is the empty string, a list its repr.
#[must_use]
pub fn truthy_str(field: Option<&Field>) -> String {
    match field {
        Some(Field::Text(text)) => text.clone(),
        Some(Field::List(items)) if !items.is_empty() => repr_list(items),
        _ => String::new(),
    }
}

/// Whether the field has a value in Python's sense (a non-empty string or list).
#[must_use]
pub fn has_value(field: Option<&Field>) -> bool {
    match field {
        Some(Field::Text(text)) => !text.is_empty(),
        Some(Field::List(items)) => !items.is_empty(),
        None => false,
    }
}

/// Whether the frontmatter says `class: derived`.
#[must_use]
pub fn is_derived(fields: &Fields) -> bool {
    strip(&truthy_str(fields.get("class"))) == "derived"
}

/// The aliases of a note: a list is stripped and filtered, a scalar is the only alias.
fn aliases_of(fields: &Fields) -> Vec<String> {
    match fields.get("aliases") {
        Some(Field::List(items)) => items.iter().map(|a| strip(a).to_string()).collect(),
        Some(Field::Text(text)) => vec![strip(text).to_string()],
        _ => Vec::new(),
    }
}

/// Builds the record of the note at `rel` with the given file text.
#[must_use]
pub fn note_rec(rel: &str, text: String) -> NoteRec {
    let name = path_parts(rel).last().copied().unwrap_or("");
    let stem = stem_of(name);
    let (blocks, _body) = split_blocks(&text, false);
    let fields = parse_fields(&blocks);
    let own = strip(&truthy_str(fields.get("title"))).to_string();
    let title = if own.is_empty() {
        stem.to_string()
    } else {
        own
    };
    let mut keys = vec![key_of(stem), key_of(&title)];
    keys.extend(
        aliases_of(&fields)
            .iter()
            .filter(|alias| !alias.is_empty())
            .map(|alias| key_of(alias)),
    );
    let datenklasse = raw_field(&blocks, &fields, "datenklasse");
    let kunde = raw_field(&blocks, &fields, "kunde");
    NoteRec {
        rel: rel.to_string(),
        title,
        text,
        fields,
        keys,
        datenklasse,
        kunde,
    }
}
