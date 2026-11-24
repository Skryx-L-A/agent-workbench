//! Validity from the frontmatter: `status`, `superseded-by`, `valid-until`.
//!
//! A note that is superseded, archived or expired is marked and demoted, never removed. A missing
//! successor joins the list with the score the superseded hit had before the demotion.

use std::collections::{HashMap, HashSet};

use notiz_kern::frontmatter::{Field, Fields};
use notiz_kern::text::{key_of, strip};

use crate::hit::{SearchHit, sort_by_score};
use crate::notes::{NoteRec, is_derived, truthy_str};
use crate::pystr::lower;

/// The demotion of a note that is no longer valid: it keeps `1 - 0.51` of its score.
pub const VALIDITY_DEMOTION: f64 = 0.51;
/// How many `superseded-by` steps are followed to the successor.
pub const SUCCESSOR_MAX_DEPTH: usize = 4;
/// The states that demote a note.
const STATUS_DEMOTED: [&str; 3] = ["superseded", "archived", "expired"];

/// The validity a note states about itself.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Validity {
    /// `superseded`, `archived`, `expired`, `active` or another word as written.
    pub status: String,
    /// The `superseded-by` targets.
    pub targets: Vec<String>,
}

/// The link targets of every `[[target` in the text; a target ends at `]`, `|`, `#` or a newline.
pub(crate) fn wikilink_targets(text: &str) -> Vec<&str> {
    let mut found = Vec::new();
    let mut rest = text;
    while let Some((_, after)) = rest.split_once("[[") {
        let end = after.find([']', '|', '#', '\n']).unwrap_or(after.len());
        if end == 0 {
            rest = after;
        } else {
            found.push(&after[..end]);
            rest = &after[end..];
        }
    }
    found
}

/// The `superseded-by` value as a list of link targets: `[[x]]`, `x` and lists of both.
#[must_use]
pub fn link_targets(field: Option<&Field>) -> Vec<String> {
    let items: Vec<&str> = match field {
        None => return Vec::new(),
        Some(Field::Text(text)) => vec![text.as_str()],
        Some(Field::List(items)) => items.iter().map(String::as_str).collect(),
    };
    let mut out: Vec<String> = Vec::new();
    for item in items {
        let text = strip(item);
        let found = wikilink_targets(text);
        let candidates: Vec<&str> = if found.is_empty() {
            if text.is_empty() {
                Vec::new()
            } else {
                vec![text]
            }
        } else {
            found
        };
        for target in candidates {
            let target = strip(target);
            if !target.is_empty() && !out.iter().any(|seen| seen == target) {
                out.push(target.to_string());
            }
        }
    }
    out
}

/// Whether `until` is `YYYY`, `YYYY-MM` or `YYYY-MM-DD` with ASCII digits.
pub(crate) fn is_date_prefix(until: &str) -> bool {
    let parts: Vec<&str> = until.split('-').collect();
    let sizes_ok = match parts.len() {
        1 => parts[0].len() == 4,
        2 | 3 => parts[0].len() == 4 && parts[1..].iter().all(|part| part.len() == 2),
        _ => false,
    };
    sizes_ok
        && parts
            .iter()
            .all(|part| part.bytes().all(|b| b.is_ascii_digit()))
}

/// The validity of a note from its frontmatter, or `None` when it carries none of the fields.
/// `expired` compares in the precision of `valid-until`: `2026-09` is expired from October on.
#[must_use]
pub fn frontmatter_validity(fields: &Fields, today: &str) -> Option<Validity> {
    if fields.is_empty() {
        return None;
    }
    let raw_status = fields.get("status");
    let targets = link_targets(fields.get("superseded-by"));
    let until = strip(&truthy_str(fields.get("valid-until"))).to_string();
    if raw_status.is_none() && targets.is_empty() && until.is_empty() {
        return None;
    }
    let mut status = lower(strip(&truthy_str(raw_status)));
    if status.is_empty() {
        status = "active".to_string();
    }
    if status != "superseded" && status != "archived" {
        if !targets.is_empty() {
            status = "superseded".to_string();
        } else if !until.is_empty() && is_date_prefix(&until) {
            let prefix: String = today.chars().take(until.chars().count()).collect();
            if prefix > until {
                status = "expired".to_string();
            }
        }
    }
    Some(Validity { status, targets })
}

/// The notes a list of hits can be checked against, and the link resolver over them.
pub struct Lookup<'a> {
    notes: Vec<&'a NoteRec>,
    by_rel: HashMap<&'a str, usize>,
    by_key: HashMap<&'a str, usize>,
}

impl<'a> Lookup<'a> {
    /// Indexes the notes by path and by every key they answer to (first note wins a key).
    #[must_use]
    pub fn new(notes: impl IntoIterator<Item = &'a NoteRec>) -> Self {
        let notes: Vec<&'a NoteRec> = notes.into_iter().collect();
        let mut by_rel = HashMap::new();
        let mut by_key: HashMap<&str, usize> = HashMap::new();
        for (i, note) in notes.iter().enumerate() {
            by_rel.insert(note.rel.as_str(), i);
            for key in &note.keys {
                by_key.entry(key.as_str()).or_insert(i);
            }
        }
        Self {
            notes,
            by_rel,
            by_key,
        }
    }

    /// The note at `rel`, if it is in the corpus.
    #[must_use]
    pub fn note(&self, rel: &str) -> Option<&'a NoteRec> {
        self.by_rel.get(rel).map(|&i| self.notes[i])
    }

    /// The note a link target names: by title, file stem or alias.
    #[must_use]
    pub fn resolve(&self, target: &str) -> Option<usize> {
        self.by_key.get(key_of(target).as_str()).copied()
    }
}

/// Follows `superseded-by` to the first note that is not superseded itself (at most four steps;
/// a cycle ends at the first note seen twice). `None` when the target does not resolve.
fn final_successor(lookup: &Lookup<'_>, target: &str, today: &str) -> Option<usize> {
    let mut seen: HashSet<usize> = HashSet::new();
    let mut found = lookup.resolve(target);
    for _ in 0..SUCCESSOR_MAX_DEPTH {
        let index = found?;
        if !seen.insert(index) {
            return found;
        }
        let Some(validity) = frontmatter_validity(&lookup.notes[index].fields, today) else {
            return found;
        };
        if validity.status != "superseded" || validity.targets.is_empty() {
            return found;
        }
        match lookup.resolve(&validity.targets[0]) {
            Some(next) => found = Some(next),
            None => return found,
        }
    }
    found
}

/// Marks and demotes the hits of a final list by the validity of their notes and puts the
/// successors in. Nothing is removed; the list can become longer.
pub fn apply_validity(hits: &mut Vec<SearchHit>, lookup: &Lookup<'_>, today: &str) {
    let mut pending: Vec<(usize, f64)> = Vec::new();
    for (i, hit) in hits.iter_mut().enumerate() {
        let Some(note) = lookup.note(&hit.rel) else {
            continue;
        };
        let Some(validity) = frontmatter_validity(&note.fields, today) else {
            continue;
        };
        if !STATUS_DEMOTED.contains(&validity.status.as_str()) {
            continue;
        }
        let before = hit.score;
        hit.status = validity.status;
        hit.superseded_by.clone_from(&validity.targets);
        hit.score *= 1.0 - VALIDITY_DEMOTION;
        if !validity.targets.is_empty() {
            pending.push((i, before));
        }
    }
    let mut present: HashMap<String, usize> = hits
        .iter()
        .enumerate()
        .map(|(i, hit)| (hit.rel.clone(), i))
        .collect();
    for (old, before) in pending {
        let targets = hits[old].superseded_by.clone();
        for target in targets {
            let Some(index) = final_successor(lookup, &target, today) else {
                continue;
            };
            let note = &lookup.notes[index];
            hits[old].superseded_by_rel.push(note.rel.clone());
            if let Some(&there) = present.get(&note.rel) {
                if hits[there].status.is_empty() {
                    hits[there].score = hits[there].score.max(before);
                }
                continue;
            }
            let mut successor = SearchHit::new(&note.rel, before);
            successor.title.clone_from(&note.title);
            successor.matched = "nachfolger".to_string();
            successor.derived = is_derived(&note.fields);
            present.insert(note.rel.clone(), hits.len());
            hits.push(successor);
        }
    }
    sort_by_score(hits);
}
