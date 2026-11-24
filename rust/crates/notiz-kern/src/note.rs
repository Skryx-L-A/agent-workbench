//! A parsed note: frontmatter, body, title, aliases, link targets, comparison keys and embedding text.

use std::collections::BTreeSet;

use crate::frontmatter::{Field, Fields, parse_fields, split_blocks};
use crate::text::{key_of, strip};

/// A note as the search and linking code sees it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Note {
    /// The leading frontmatter blocks without delimiters, in file order.
    pub blocks: Vec<String>,
    /// The merged frontmatter fields.
    pub fields: Fields,
    /// The text after the frontmatter blocks.
    pub body: String,
    /// The field `title`, else the file stem.
    pub title: String,
    /// The field `type`, else `note`.
    pub kind: String,
    /// The aliases in field order, stripped, without empty ones.
    pub aliases: Vec<String>,
    /// Wikilink targets of the whole text, as comparison keys.
    pub links: BTreeSet<String>,
    /// Comparison keys of title, file stem and aliases.
    pub keys: BTreeSet<String>,
    /// The text handed to an embedding model: a head of content fields, then the body without
    /// its `## Relations` section.
    pub embed_text: String,
}

/// True for the heading line of the managed section: `## Relations`, optional blanks, newline.
fn is_relations_heading(line: &str) -> bool {
    line.strip_prefix("## Relations")
        .is_some_and(|rest| rest.trim_start_matches([' ', '\t']) == "\n")
}

/// True for a line that belongs to the section body: optional blanks, then a dash.
fn is_relations_item(line: &str) -> bool {
    line.trim_start_matches([' ', '\t']).starts_with('-')
}

/// `text` without its `## Relations` sections: the heading line and every directly following
/// line that starts with optional blanks and a dash.
#[must_use]
pub fn cut_relations(text: &str) -> String {
    let mut out = String::new();
    let mut in_section = false;
    for line in text.split_inclusive('\n') {
        if in_section && is_relations_item(line) {
            continue;
        }
        in_section = is_relations_heading(line);
        if !in_section {
            out.push_str(line);
        }
    }
    out
}

/// The comparison keys of every `[[target` in the text; a target ends at `]`, `|`, `#` or a newline.
///
/// The scan is sequential, as a regular expression would do it: a target may contain `[`, and the
/// text of a target is not scanned again. An empty target is skipped.
#[must_use]
pub fn wikilinks(text: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let mut rest = text;
    while let Some((_, after)) = rest.split_once("[[") {
        let (target, tail) =
            after.split_at(after.find([']', '|', '#', '\n']).unwrap_or(after.len()));
        if target.is_empty() {
            rest = after;
        } else {
            out.insert(key_of(target));
            rest = tail;
        }
    }
    out
}

/// The stripped text of a scalar field, or `default` when it is missing or empty.
fn text_or(fields: &Fields, name: &str, default: &str) -> String {
    match fields.get(name) {
        Some(Field::Text(text)) if !strip(text).is_empty() => strip(text).to_string(),
        _ => default.to_string(),
    }
}

/// The aliases: a list is stripped and filtered, a scalar is the only alias.
fn aliases_of(fields: &Fields) -> Vec<String> {
    match fields.get("aliases") {
        Some(Field::List(list)) => list
            .iter()
            .map(|alias| strip(alias).to_string())
            .filter(|alias| !alias.is_empty())
            .collect(),
        Some(Field::Text(text)) if !text.is_empty() => vec![strip(text).to_string()],
        _ => Vec::new(),
    }
}

/// The head of the embedding text: title, aliases, tags, type, branch and description.
fn embed_head(fields: &Fields, title: &str, aliases: &[String]) -> String {
    let mut lines = vec![title.to_string()];
    if !aliases.is_empty() {
        lines.push(format!("aliases: {}", aliases.join(", ")));
    }
    match fields.get("tags") {
        Some(Field::List(tags)) if !tags.is_empty() => {
            lines.push(format!("tags: {}", tags.join(", ")));
        }
        Some(Field::Text(tag)) if !tag.is_empty() => lines.push(format!("tags: {tag}")),
        _ => {}
    }
    for name in ["type", "branch"] {
        if let Some(value) = fields.get(name).and_then(Field::non_empty_text) {
            lines.push(format!("{name}: {value}"));
        }
    }
    if let Some(value) = fields.get("description").and_then(Field::non_empty_text) {
        lines.push(value.to_string());
    }
    lines.join("\n")
}

/// Parses the text of a note whose file name has the stem `file_stem`.
#[must_use]
pub fn parse_note(text: &str, file_stem: &str) -> Note {
    let (blocks, body) = split_blocks(text, false);
    let fields = parse_fields(&blocks);
    let title = text_or(&fields, "title", file_stem);
    let kind = text_or(&fields, "type", "note");
    let aliases = aliases_of(&fields);
    let keys = [key_of(&title), key_of(file_stem)]
        .into_iter()
        .chain(aliases.iter().map(|alias| key_of(alias)))
        .collect();
    let embed_text = format!(
        "{}\n\n{}",
        embed_head(&fields, &title, &aliases),
        cut_relations(body)
    );
    Note {
        blocks: blocks.iter().map(ToString::to_string).collect(),
        fields,
        body: body.to_string(),
        title,
        kind,
        aliases,
        links: wikilinks(text),
        keys,
        embed_text,
    }
}
