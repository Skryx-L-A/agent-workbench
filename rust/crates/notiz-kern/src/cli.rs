//! The command line: argument handling, JSON output and exit codes.
//!
//! Every call with exit code 0, 3 or 4 writes exactly one JSON line to the output; diagnostics for
//! humans go to the error output. The commands only read.

use std::io::Write;
use std::path::Path;

use serde_json::{Map, Value, json};

use crate::blocks::{block_of, block_state, replace_block, strip_blocks};
use crate::corpus::{corpus, path_info, stem_of};
use crate::frontmatter::Field;
use crate::note::parse_note;
use crate::schema::lint;
use crate::section::section;
use crate::text::read_text;

/// Success.
pub const EXIT_OK: u8 = 0;
/// Wrong usage: unknown command or missing argument.
pub const EXIT_USAGE: u8 = 2;
/// A file or directory is not readable.
pub const EXIT_UNREADABLE: u8 = 3;
/// The requested section does not exist.
pub const EXIT_NOT_FOUND: u8 = 4;

/// The outcome of one command: an exit code and the JSON object to print, if any.
type Outcome = (u8, Option<Value>);

fn unreadable() -> Outcome {
    (EXIT_UNREADABLE, Some(json!({"error": "unreadable"})))
}

fn usage() -> Outcome {
    (EXIT_USAGE, None)
}

/// Splits `args` into positional arguments and the values of the named options.
///
/// Returns `None` when an option has no value. A later occurrence of an option wins.
fn split_options<'a, const N: usize>(
    args: &'a [String],
    names: [&str; N],
) -> Option<(Vec<&'a str>, [Option<&'a str>; N])> {
    let mut positional = Vec::new();
    let mut values = [None; N];
    let mut iter = args.iter().map(String::as_str);
    while let Some(arg) = iter.next() {
        if let Some(slot) = names.iter().position(|name| *name == arg) {
            values[slot] = Some(iter.next()?);
        } else {
            positional.push(arg);
        }
    }
    Some((positional, values))
}

fn field_json(field: &Field) -> Value {
    match field {
        Field::Text(text) => json!(text),
        Field::List(list) => json!(list),
    }
}

/// The file name of `path` as text; empty when the path has none.
fn file_name(path: &str) -> String {
    Path::new(path)
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn parse_command(file: &str) -> Outcome {
    let Ok(text) = read_text(Path::new(file)) else {
        return unreadable();
    };
    let name = file_name(file);
    let note = parse_note(&text, stem_of(&name));
    let fields: Map<String, Value> = note
        .fields
        .iter()
        .map(|(key, field)| (key.clone(), field_json(field)))
        .collect();
    let value = json!({
        "blocks": note.blocks,
        "fields": fields,
        "body": note.body,
        "title": note.title,
        "type": note.kind,
        "aliases": note.aliases,
        "links": note.links,
        "keys": note.keys,
        "embed_text": note.embed_text,
    });
    (EXIT_OK, Some(value))
}

fn lint_command(file: &str, rel: &str) -> Outcome {
    let Ok(text) = read_text(Path::new(file)) else {
        return unreadable();
    };
    let report = lint(rel, &text);
    let findings: Vec<Value> = report
        .findings
        .iter()
        .map(|finding| json!({"kind": finding.kind, "detail": finding.detail}))
        .collect();
    let value = json!({
        "in_scope": report.in_scope,
        "fields": report.fields,
        "blocks": report.blocks,
        "findings": findings,
        "new": report.is_new,
    });
    (EXIT_OK, Some(value))
}

fn paths_command(rels: &[&str]) -> Outcome {
    let infos: Vec<Value> = rels
        .iter()
        .map(|rel| {
            let info = path_info(rel);
            json!({
                "rel": info.rel,
                "linking_excluded": info.linking_excluded,
                "search_excluded": info.search_excluded,
                "lint_in_scope": info.lint_in_scope,
                "branch": info.branch,
                "stem_key": info.stem_key,
            })
        })
        .collect();
    (EXIT_OK, Some(Value::Array(infos)))
}

fn corpus_command(vault: &str) -> Outcome {
    match corpus(Path::new(vault)) {
        Ok(found) => (
            EXIT_OK,
            Some(json!({"linking": found.linking, "search": found.search})),
        ),
        Err(_) => unreadable(),
    }
}

fn markers_command(file: &str, start: &str, end: &str, replace: Option<&str>) -> Outcome {
    let Ok(text) = read_text(Path::new(file)) else {
        return unreadable();
    };
    let mut value = json!({
        "state": block_state(&text, start, end).as_str(),
        "block": block_of(&text, start, end),
        "stripped": strip_blocks(&text, start, end),
    });
    if let Some(new_block) = replace {
        value["replaced"] = json!(replace_block(&text, start, end, new_block));
    }
    (EXIT_OK, Some(value))
}

fn section_command(file: &str, heading: &str) -> Outcome {
    let Ok(text) = read_text(Path::new(file)) else {
        return unreadable();
    };
    match section(&text, heading) {
        Some(found) => (EXIT_OK, Some(json!({"section": found}))),
        None => (EXIT_NOT_FOUND, Some(json!({"error": "section-not-found"}))),
    }
}

/// Dispatches one command line (without the program name).
fn dispatch(args: &[String]) -> Outcome {
    let Some((command, rest)) = args.split_first() else {
        return usage();
    };
    match command.as_str() {
        "parse" => match rest {
            [file] => parse_command(file),
            _ => usage(),
        },
        "section" => match rest {
            [file, heading] => section_command(file, heading),
            _ => usage(),
        },
        "paths" if !rest.is_empty() => {
            let rels: Vec<&str> = rest.iter().map(String::as_str).collect();
            paths_command(&rels)
        }
        "corpus" => match rest {
            [vault] => corpus_command(vault),
            _ => usage(),
        },
        "lint" => match split_options(rest, ["--rel"]) {
            Some((positional, [Some(rel)])) if positional.len() == 1 => {
                lint_command(positional[0], rel)
            }
            _ => usage(),
        },
        "markers" => match split_options(rest, ["--start", "--end", "--replace"]) {
            Some((positional, [Some(start), Some(end), replace])) if positional.len() == 1 => {
                markers_command(positional[0], start, end, replace)
            }
            _ => usage(),
        },
        _ => usage(),
    }
}

/// Runs one command line (without the program name) and returns the exit code.
///
/// The JSON result goes to `out` as one line; a usage error is explained on `err`.
pub fn run<O: Write, E: Write>(args: &[String], out: &mut O, err: &mut E) -> u8 {
    let (code, value) = dispatch(args);
    if let Some(value) = value {
        // A closed output pipe is the reader's decision; the exit code still tells the result.
        let _ = writeln!(out, "{value}");
    }
    if code == EXIT_USAGE {
        let _ = writeln!(
            err,
            "usage: notiz-kern parse <file> | lint <file> --rel <path> | paths <path>... | corpus <vault> | markers <file> --start <marker> --end <marker> [--replace <text>] | section <file> <heading>"
        );
    }
    code
}
