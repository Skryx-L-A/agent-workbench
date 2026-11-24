//! Schema 4 lint: required fields, type vocabulary, class against path, one frontmatter block.
//!
//! The schema reader differs from the note reader on purpose: it drops leading BOMs, cuts a
//! comment after a single whitespace character, knows no lists, and only splits on `---` lines.

use std::collections::BTreeMap;

use crate::frontmatter::{cut_comment, is_blank, key_match, split_blocks};
use crate::text::{has_md_suffix, split_lines, strip};

/// A note whose `created` date is on or after this day counts as new.
pub const CUTOFF: &str = "2026-09-28";

/// The top-level branches of the vault that the lint covers.
const BRANCHES: [&str; 5] = [
    "00-sources/",
    "10-global/",
    "20-projects/",
    "30-topics/",
    "40-people/",
];
/// File names the lint skips.
const EXEMPT_NAMES: [&str; 5] = [
    "MOC.md",
    "DECISIONS.md",
    "README.md",
    "brain-vorschlaege.md",
    "review-queue.md",
];
/// Prefixes of generated reports; they carry no note identity.
pub const GENERATED_PREFIXES: [&str; 5] = [
    "gardener-report-",
    "brain-health-",
    "dream-report-",
    "brain-run-",
    "brain-vorschlaege",
];
/// The allowed values of `type`, sorted.
const TYPES: [&str; 12] = [
    "asset",
    "decision",
    "incident",
    "note",
    "person",
    "pitfall",
    "reference",
    "report",
    "secret",
    "session",
    "system",
    "term",
];
/// The allowed values of `class`, sorted.
const CLASSES: [&str; 4] = ["derived", "knowledge", "meta", "source"];

/// One lint finding.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Finding {
    /// The kind: `pflichtfeld`, `id-format`, `typ`, `klasse`, `klasse-pfad`,
    /// `mehrfach-frontmatter` or `memory-rest`.
    pub kind: &'static str,
    /// The finding as a German sentence; the wording is fixed.
    pub detail: String,
}

impl Finding {
    fn new(kind: &'static str, detail: String) -> Self {
        Self { kind, detail }
    }
}

/// The result of linting one note.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LintReport {
    /// Whether the path is inside the lint scope.
    pub in_scope: bool,
    /// The top-level fields; later blocks win.
    pub fields: BTreeMap<String, String>,
    /// The number of frontmatter blocks.
    pub blocks: usize,
    /// The findings in a fixed order; empty means the note is in order.
    pub findings: Vec<Finding>,
    /// Whether `created` is on or after [`CUTOFF`].
    pub is_new: bool,
}

/// Whether the vault-relative path `rel` (with `/`) is covered by the lint.
#[must_use]
pub fn in_scope(rel: &str) -> bool {
    if !has_md_suffix(rel) || !BRANCHES.iter().any(|branch| rel.starts_with(branch)) {
        return false;
    }
    let parts: Vec<&str> = rel.split('/').collect();
    if parts
        .iter()
        .any(|part| *part == "_assets" || part.starts_with('.'))
    {
        return false;
    }
    let name = parts[parts.len() - 1];
    !EXEMPT_NAMES.contains(&name)
        && !GENERATED_PREFIXES
            .iter()
            .any(|prefix| name.starts_with(prefix))
}

/// The top-level fields of the schema reader and the number of frontmatter blocks.
#[must_use]
pub fn schema_fields(text: &str) -> (BTreeMap<String, String>, usize) {
    let (blocks, _) = split_blocks(text, true);
    let mut fields = BTreeMap::new();
    for line in blocks.iter().flat_map(|block| split_lines(block)) {
        if let Some((key, value)) = key_match(line, is_blank) {
            let value = strip(cut_comment(value, 1)).trim_matches(['\'', '"']);
            fields.insert(key.to_string(), value.to_string());
        }
    }
    (fields, blocks.len())
}

/// True for a 26-character ULID in Crockford base 32 (no `I`, `L`, `O`, `U`).
fn is_ulid(id: &str) -> bool {
    id.chars().count() == 26
        && id.chars().all(
            |c| matches!(c, '0'..='9' | 'A'..='H' | 'J' | 'K' | 'M' | 'N' | 'P'..='T' | 'V'..='Z'),
        )
}

/// The finding for missing required fields, if any.
fn missing_fields(fields: &BTreeMap<String, String>) -> Option<Finding> {
    let missing: Vec<&str> = ["id", "schema", "class", "type"]
        .into_iter()
        .filter(|name| fields.get(*name).is_none_or(String::is_empty))
        .collect();
    (!missing.is_empty()).then(|| {
        Finding::new(
            "pflichtfeld",
            format!("Pflichtfeld fehlt: {}", missing.join(", ")),
        )
    })
}

/// The findings about the vocabulary of `id`, `type` and `class`.
fn vocabulary(id: &str, kind: &str, class: &str) -> Vec<Finding> {
    let mut out = Vec::new();
    if !id.is_empty() && !is_ulid(id) {
        let shown: String = id.chars().take(30).collect();
        out.push(Finding::new(
            "id-format",
            format!("id '{shown}' ist keine ULID"),
        ));
    }
    if !kind.is_empty() && !TYPES.contains(&kind) {
        out.push(Finding::new(
            "typ",
            format!("type '{kind}' nicht im Vokabular ({})", TYPES.join("|")),
        ));
    }
    if !class.is_empty() && !CLASSES.contains(&class) {
        out.push(Finding::new(
            "klasse",
            format!("class '{class}' unbekannt ({})", CLASSES.join("|")),
        ));
    }
    out
}

/// The finding when sessions and sources are not marked `class: source`.
fn class_against_path(rel: &str, kind: &str, class: &str) -> Option<Finding> {
    let parts: Vec<&str> = rel.split('/').collect();
    let in_session_path =
        rel.starts_with("00-sources/") || parts[..parts.len() - 1].contains(&"sessions");
    if !(in_session_path || kind == "session") || !CLASSES.contains(&class) || class == "source" {
        return None;
    }
    let reason = if kind == "session" {
        "type: session"
    } else {
        "Pfad sessions/ oder 00-sources/"
    };
    Some(Finding::new(
        "klasse-pfad",
        format!("{reason} verlangt class: source, steht class: {class}"),
    ))
}

/// The findings of one note, in a fixed order. Generated pages (`class: derived`,
/// `gardener-generated`) are exempt.
#[must_use]
pub fn findings(rel: &str, text: &str) -> Vec<Finding> {
    let (fields, blocks) = schema_fields(text);
    if blocks == 0 {
        return vec![Finding::new("pflichtfeld", "kein Frontmatter".to_string())];
    }
    let get = |name: &str| fields.get(name).map_or("", String::as_str);
    if get("class") == "derived" || fields.contains_key("gardener-generated") {
        return Vec::new();
    }
    let (id, kind, class) = (get("id"), get("type"), get("class"));
    let mut out: Vec<Finding> = missing_fields(&fields).into_iter().collect();
    out.extend(vocabulary(id, kind, class));
    out.extend(class_against_path(rel, kind, class));
    if blocks > 1 {
        out.push(Finding::new(
            "mehrfach-frontmatter",
            format!("{blocks} Frontmatter-Bloecke (Basic-Memory-Rest)"),
        ));
    }
    let leftovers: Vec<String> = ["name", "metadata"]
        .into_iter()
        .filter(|name| fields.contains_key(*name))
        .map(|name| format!("{name}:"))
        .collect();
    if !leftovers.is_empty() {
        out.push(Finding::new(
            "memory-rest",
            format!("Claude-Memory-Felder: {}", leftovers.join(", ")),
        ));
    }
    out
}

/// True when `created` starts with a date `YYYY-MM-DD` that is on or after [`CUTOFF`].
fn created_on_or_after_cutoff(created: &str) -> bool {
    let Some(date) = created.get(..10) else {
        return false;
    };
    let is_date = date.bytes().enumerate().all(|(i, b)| {
        if i == 4 || i == 7 {
            b == b'-'
        } else {
            b.is_ascii_digit()
        }
    });
    is_date && date >= CUTOFF
}

/// Lints one note: scope, fields, findings and whether it counts as new.
#[must_use]
pub fn lint(rel: &str, text: &str) -> LintReport {
    let (fields, blocks) = schema_fields(text);
    let created = fields.get("created").map_or("", String::as_str);
    LintReport {
        in_scope: in_scope(rel),
        is_new: created_on_or_after_cutoff(created),
        findings: findings(rel, text),
        fields,
        blocks,
    }
}
