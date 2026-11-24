//! Corpus rules: which vault paths are notes for linking and for search, and the directory walk.
//!
//! Safety rules, all enforced here and nowhere else:
//!
//! - `90-secrets`, `_meta`, `.obsidian`, `.git` and every directory whose name starts with a dot
//!   are never entered and never listed, at any depth, whatever their letter case.
//! - Symbolic links, to files and to directories, are neither followed nor listed.
//! - Only paths below the given root are ever produced: the walk starts at the root and only
//!   descends into real directories, and the rules for single paths (`path_info`) never touch
//!   the file system.
//!
//! The walk only reads. Nothing in this crate writes to a vault.

use std::fs;
use std::io;
use std::path::Path;

use crate::schema::{GENERATED_PREFIXES, in_scope};
use crate::text::{exclusion_key, has_md_suffix, key_of};

/// Directories that are never part of a corpus, written in the form of [`exclusion_key`].
pub(crate) const EXCLUDED_DIRS: [&str; 4] = ["90-secrets", ".obsidian", ".git", "_meta"];
/// Generated files at the top level only, written in the form of [`exclusion_key`].
pub(crate) const EXCLUDED_ROOT_FILES: [&str; 6] = [
    "hot.md",
    "index.md",
    "critical-facts.md",
    "review-queue.md",
    "open-questions.md",
    "log.md",
];
/// The drop zone is raw input, not corpus.
const DROP_DIR: &str = "00-sources/drop";

/// The rules for one vault-relative path.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PathInfo {
    /// The path as given.
    pub rel: String,
    /// Excluded from the linking corpus.
    pub linking_excluded: bool,
    /// Excluded from the search corpus (the same, except that `MOC.md` stays).
    pub search_excluded: bool,
    /// Inside the lint scope.
    pub lint_in_scope: bool,
    /// The owning branch: `20-projects/<p>`, `30-topics/<t>`, else the first part.
    pub branch: String,
    /// The comparison key of the file stem.
    pub stem_key: String,
}

/// The two corpora of a vault: vault-relative paths with `/`, sorted by path parts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Corpus {
    /// Notes for linking.
    pub linking: Vec<String>,
    /// Notes for search.
    pub search: Vec<String>,
}

/// Python's `PurePosixPath(rel).parts` for a relative path: empty and `.` parts are dropped,
/// `..` stays.
#[must_use]
pub fn path_parts(rel: &str) -> Vec<&str> {
    rel.split('/')
        .filter(|part| !part.is_empty() && *part != ".")
        .collect()
}

/// Python's `PurePath.stem` of a file name: the extension starts at the last dot, unless that dot
/// is the first or the last character.
#[must_use]
pub fn stem_of(name: &str) -> &str {
    match name.rfind('.') {
        Some(i) if i > 0 && i + 1 < name.len() => &name[..i],
        _ => name,
    }
}

/// Whether a directory name is never entered: a hidden name, or one of [`EXCLUDED_DIRS`] after
/// normalisation and full case folding (`90-ſecrets`, `90-SECRETS`, full-width spellings). A name
/// counts as hidden when it starts with a dot as given or after normalisation. The walk and the
/// path rules both use this one function.
fn is_excluded_dir(name: &str) -> bool {
    let key = exclusion_key(name);
    name.starts_with('.') || key.starts_with('.') || EXCLUDED_DIRS.contains(&key.as_str())
}

/// Whether the path, given as parts, is excluded from the corpus. `for_search` keeps `MOC.md`.
#[must_use]
pub fn is_excluded(parts: &[&str], for_search: bool) -> bool {
    let Some((name, dirs)) = parts.split_last() else {
        return true;
    };
    if dirs.iter().any(|dir| is_excluded_dir(dir)) {
        return true;
    }
    if dirs.is_empty() && EXCLUDED_ROOT_FILES.contains(&exclusion_key(name).as_str()) {
        return true;
    }
    if matches!(*name, "review-queue.md" | "DECISIONS.md") || (!for_search && *name == "MOC.md") {
        return true;
    }
    if GENERATED_PREFIXES
        .iter()
        .any(|prefix| name.starts_with(prefix))
    {
        return true;
    }
    !dirs.is_empty() && dirs.join("/").starts_with(DROP_DIR)
}

/// The owning branch of a path given as parts.
fn branch_of(parts: &[&str]) -> String {
    match parts {
        [first @ ("20-projects" | "30-topics"), second, _, ..] => format!("{first}/{second}"),
        [first, _, ..] => (*first).to_string(),
        _ => String::new(),
    }
}

/// The rules for one vault-relative path. The file system is not read.
#[must_use]
pub fn path_info(rel: &str) -> PathInfo {
    let parts = path_parts(rel);
    PathInfo {
        rel: rel.to_string(),
        linking_excluded: is_excluded(&parts, false),
        search_excluded: is_excluded(&parts, true),
        lint_in_scope: in_scope(rel),
        branch: branch_of(&parts),
        stem_key: key_of(stem_of(parts.last().copied().unwrap_or(""))),
    }
}

/// Every Markdown file below `root` that the walk may list, as path parts. Symbolic links and
/// the directories of [`is_excluded`] are skipped; unreadable directories are skipped silently.
fn walk(root: &Path) -> Vec<Vec<String>> {
    let mut found = Vec::new();
    let mut pending = vec![(root.to_path_buf(), Vec::<String>::new())];
    while let Some((dir, prefix)) = pending.pop() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            if kind.is_symlink() {
                continue;
            }
            let name = entry.file_name().to_string_lossy().into_owned();
            if kind.is_dir() {
                if !is_excluded_dir(&name) {
                    let mut path = prefix.clone();
                    path.push(name);
                    pending.push((entry.path(), path));
                }
            } else if has_md_suffix(&name) {
                let mut path = prefix.clone();
                path.push(name);
                found.push(path);
            }
        }
    }
    found
}

/// The linking and search corpus of the vault at `root`, sorted like Python's `sorted()` over
/// paths: by the list of parts, each part by code point.
///
/// # Errors
///
/// Returns an error when `root` is not a readable directory.
pub fn corpus(root: &Path) -> io::Result<Corpus> {
    fs::read_dir(root)?;
    let mut all = walk(root);
    all.sort();
    let pick = |for_search: bool| -> Vec<String> {
        all.iter()
            .filter(|path| {
                let parts: Vec<&str> = path.iter().map(String::as_str).collect();
                !is_excluded(&parts, for_search)
            })
            .map(|path| path.join("/"))
            .collect()
    };
    Ok(Corpus {
        linking: pick(false),
        search: pick(true),
    })
}
