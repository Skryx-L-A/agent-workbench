//! The command line: `tokens`, `snippet`, `search`; argument rules of Python's `argparse` as the
//! reference uses it (no abbreviations), JSON output and exit codes.
//!
//! Every call with exit code 0 or 3 writes exactly one JSON line to the output; diagnostics for
//! humans go to the error output. The program reads files and writes nothing but that line.

use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use notiz_kern::text::{is_space, read_text};
use serde_json::{Map, Value, json};

use crate::embed::{DEFAULT_KEEP_ALIVE, DEFAULT_MODEL, DEFAULT_TIMEOUT_MS, Embedder};
use crate::hit::SearchHit;
use crate::search::{Params, search, search_embedding};
use crate::serve::{Server, serve_lines};
use crate::snippet::{Excerpt, best_snippet_span, hook_snippet_span};
use crate::token::Analyzer;
use crate::vault::{Unreadable, Vectors, load_vectors};
use crate::vdb::{DbPaths, load_db_vectors, load_merged};

/// Success.
pub const EXIT_OK: u8 = 0;
/// Wrong usage of any command: no or an unknown command, unknown option, missing argument, abbreviation, `-k` below 1,
/// an argument that is no UTF-8 (`EX_USAGE` of `sysexits.h`; one code for the whole program since 23.11.2026, before
/// that 2 outside `sitzung`).
pub const EXIT_USAGE: u8 = 64;
/// The usage error of `rangkern sitzung`: the same code as [`EXIT_USAGE`], kept as a name for the module `sitzung`.
pub const EXIT_USAGE_SITZUNG: u8 = EXIT_USAGE;
/// The vault, a file or the vector file cannot be read.
pub const EXIT_UNREADABLE: u8 = 3;

/// The outcome of one command: an exit code and the JSON object to print, if any.
type Outcome = (u8, Option<Value>);

fn usage() -> Outcome {
    (EXIT_USAGE, None)
}

fn unreadable() -> Outcome {
    (EXIT_UNREADABLE, Some(json!({"error": "unreadable"})))
}

/// How `argparse` sees one argument.
pub(crate) enum Kind<'a> {
    /// A value for a positional argument or an option.
    Positional,
    /// A known option, with the value written after `=` or directly after a short option.
    Known(&'a str, Option<&'a str>),
    /// Starts with `-` but is no known option.
    Unknown,
}

/// Python's `^-\d+$|^-\d*\.\d+$`: a negative number is a value, not an option.
pub(crate) fn looks_negative_number(arg: &str) -> bool {
    let Some(rest) = arg.strip_prefix('-') else {
        return false;
    };
    let digits = |s: &str| s.bytes().all(|b| b.is_ascii_digit());
    match rest.split_once('.') {
        None => !rest.is_empty() && digits(rest),
        Some((whole, fraction)) => digits(whole) && !fraction.is_empty() && digits(fraction),
    }
}

/// `argparse._parse_optional` for the options `known` (without abbreviations).
pub(crate) fn classify<'a>(arg: &'a str, known: &[&'a str]) -> Kind<'a> {
    if !arg.starts_with('-') || arg == "-" {
        return Kind::Positional;
    }
    if let Some(name) = known.iter().find(|name| **name == arg) {
        return Kind::Known(name, None);
    }
    if let Some((name, value)) = arg.split_once('=')
        && let Some(name) = known.iter().find(|n| **n == name)
    {
        return Kind::Known(name, Some(value));
    }
    if !arg.starts_with("--") {
        let (head, value) = arg.split_at(arg.char_indices().nth(2).map_or(arg.len(), |(i, _)| i));
        if let Some(name) = known.iter().find(|n| **n == head) {
            return Kind::Known(name, Some(value));
        }
    }
    if looks_negative_number(arg) || arg.contains(' ') {
        return Kind::Positional;
    }
    Kind::Unknown
}

/// The positional arguments and the values of the options of one command.
pub(crate) struct Args {
    pub(crate) positional: Vec<String>,
    options: Vec<(String, String)>,
}

impl Args {
    /// How often an option is given.
    pub(crate) fn count(&self, name: &str) -> usize {
        self.options.iter().filter(|(key, _)| key == name).count()
    }

    /// The value of the last occurrence of an option.
    pub(crate) fn option(&self, name: &str) -> Option<&str> {
        self.options
            .iter()
            .rev()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
    }
}

/// Splits `args` by the rules of `argparse`; `None` for a usage error. Everything after `--` is
/// a positional argument; an option that needs a value takes the next argument only if that is
/// a value too.
pub(crate) fn parse_args(args: &[String], known: &[&str], count: usize) -> Option<Args> {
    let mut positional = Vec::new();
    let mut options = Vec::new();
    let mut iter = args.iter().peekable();
    while let Some(arg) = iter.next() {
        if arg == "--" {
            positional.extend(iter.by_ref().cloned());
            break;
        }
        match classify(arg, known) {
            Kind::Positional => positional.push(arg.clone()),
            Kind::Unknown => return None,
            Kind::Known(name, Some(value)) => options.push((name.to_string(), value.to_string())),
            Kind::Known(name, None) => {
                let next = iter.next()?;
                if next == "--" || !matches!(classify(next, known), Kind::Positional) {
                    return None;
                }
                options.push((name.to_string(), next.clone()));
            }
        }
    }
    (positional.len() == count).then_some(Args {
        positional,
        options,
    })
}

/// Python's `int()` of an option value, as far as the result is a positive number: surrounding
/// white space, an optional sign, digits with single underscores between them.
pub(crate) fn parse_positive(text: &str) -> Option<usize> {
    let trimmed = text.trim_matches(is_space);
    let digits = trimmed.strip_prefix('+').unwrap_or(trimmed);
    let well_formed = !digits.is_empty()
        && !digits.starts_with('_')
        && !digits.ends_with('_')
        && !digits.contains("__")
        && digits.bytes().all(|b| b.is_ascii_digit() || b == b'_');
    if !well_formed {
        return None;
    }
    let cleaned: String = digits.chars().filter(|c| *c != '_').collect();
    let value = cleaned
        .bytes()
        .try_fold(0_usize, |acc, b| {
            acc.checked_mul(10)?.checked_add(usize::from(b - b'0'))
        })
        .unwrap_or(usize::MAX);
    (value >= 1).then_some(value)
}

/// Whether `text` is `YYYY-MM-DD` with ASCII digits.
pub(crate) fn is_date(text: &str) -> bool {
    let bytes = text.as_bytes();
    bytes.len() == 10
        && bytes.iter().enumerate().all(|(i, b)| match i {
            4 | 7 => *b == b'-',
            _ => b.is_ascii_digit(),
        })
}

/// The civil date `(year, month, day)` of a day number counted from 1970-01-01 (proleptic
/// Gregorian calendar).
pub(crate) fn civil_date(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let day_of_era = z.rem_euclid(146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let shifted_month = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * shifted_month + 2) / 5 + 1;
    let month = if shifted_month < 10 {
        shifted_month + 3
    } else {
        shifted_month - 9
    };
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

/// Today's date (UTC) as `YYYY-MM-DD`.
pub(crate) fn today() -> String {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs());
    let (year, month, day) = civil_date(i64::try_from(seconds / 86_400).unwrap_or(0));
    format!("{year:04}-{month:02}-{day:02}")
}

fn span_json(span: Option<(usize, usize)>) -> Value {
    span.map_or(Value::Null, |(first, last)| json!([first, last]))
}

fn tokens_command(args: &[String]) -> Outcome {
    let Some(parsed) = parse_args(args, &[], 1) else {
        return usage();
    };
    let text = &parsed.positional[0];
    let mut analyzer = Analyzer::new();
    (
        EXIT_OK,
        Some(json!({
            "words": crate::token::tokenize(text),
            "stems": analyzer.stem_tokens(text),
            "index": analyzer.index_tokens(text),
        })),
    )
}

fn snippet_command(args: &[String]) -> Outcome {
    let Some(parsed) = parse_args(args, &[], 2) else {
        return usage();
    };
    let Ok(text) = read_text(Path::new(&parsed.positional[0])) else {
        return unreadable();
    };
    let mut analyzer = Analyzer::new();
    let (snippet, lines): Excerpt = best_snippet_span(&mut analyzer, &text, &parsed.positional[1]);
    let (hook, hook_lines) = hook_snippet_span(&text);
    (
        EXIT_OK,
        Some(json!({
            "snippet": snippet,
            "lines": span_json(lines),
            "hook": hook,
            "hook_lines": span_json(hook_lines),
        })),
    )
}

/// One hit as the model's `hit_to_json` writes it: a field with its default value is left out.
pub(crate) fn hit_json(hit: &SearchHit) -> Value {
    let mut out = Map::new();
    out.insert("rel".into(), json!(hit.rel));
    out.insert("score".into(), json!(hit.score));
    if !hit.title.is_empty() {
        out.insert("title".into(), json!(hit.title));
    }
    if !hit.snippet.is_empty() {
        out.insert("snippet".into(), json!(hit.snippet));
    }
    if let Some((first, last)) = hit.lines {
        out.insert("lines".into(), json!([first, last]));
    }
    if !hit.matched.is_empty() {
        out.insert("match".into(), json!(hit.matched));
    }
    if hit.cosine != 0.0 {
        out.insert("cosine".into(), json!(hit.cosine));
    }
    if hit.bm25_rang != 0 {
        out.insert("bm25_rang".into(), json!(hit.bm25_rang));
        out.insert("bm25_norm".into(), json!(hit.bm25_norm));
    }
    if hit.vektor_rang != 0 {
        out.insert("vektor_rang".into(), json!(hit.vektor_rang));
    }
    if !hit.space.is_empty() && hit.space != "kern" {
        out.insert("space".into(), json!(hit.space));
    }
    if hit.derived {
        out.insert("derived".into(), json!(true));
    }
    if hit.contradiction {
        out.insert("contradiction".into(), json!(true));
    }
    if !hit.status.is_empty() {
        out.insert("status".into(), json!(hit.status));
        if !hit.superseded_by.is_empty() {
            let shown = if hit.superseded_by_rel.is_empty() {
                &hit.superseded_by
            } else {
                &hit.superseded_by_rel
            };
            out.insert("abgeloest_durch".into(), json!(shown));
        }
    }
    Value::Object(out)
}

/// A usage error found while the options are combined.
pub(crate) struct Usage;

/// The databases named by `--index-db` and `--extra-db`, none without `--index-db`.
///
/// `--extra-db` and `--query-vector` need `--index-db`, and `--index-db` excludes `--vectors`.
pub(crate) fn db_paths(parsed: &Args) -> Result<Option<DbPaths>, Usage> {
    let extra = parsed.option("--extra-db");
    let Some(index) = parsed.option("--index-db") else {
        let stray = extra.is_some() || parsed.option("--query-vector").is_some();
        return if stray { Err(Usage) } else { Ok(None) };
    };
    if parsed.option("--vectors").is_some() {
        return Err(Usage);
    }
    Ok(Some(DbPaths {
        index: PathBuf::from(index),
        extra: extra.map(PathBuf::from),
    }))
}

/// The options that make `--embed` work.
const EMBED_OPTIONS: [&str; 4] = [
    "--embed",
    "--embed-model",
    "--keep-alive",
    "--embed-timeout-ms",
];

/// The call to Ollama named by `--embed`, `None` without it.
///
/// `--embed` needs `--index-db`, excludes `--query-vector`, and takes a loopback address; the other
/// embedding options need `--embed`. The timeout is a whole number of milliseconds of 1 or more.
pub(crate) fn embedder_of(parsed: &Args, dbs: Option<&DbPaths>) -> Result<Option<Embedder>, Usage> {
    let Some(url) = parsed.option("--embed") else {
        let stray = EMBED_OPTIONS.iter().any(|name| parsed.count(name) > 0);
        return if stray { Err(Usage) } else { Ok(None) };
    };
    if dbs.is_none() || parsed.option("--query-vector").is_some() {
        return Err(Usage);
    }
    let timeout = match parsed.option("--embed-timeout-ms") {
        None => DEFAULT_TIMEOUT_MS,
        Some(text) => parse_positive(text)
            .and_then(|ms| u64::try_from(ms).ok())
            .ok_or(Usage)?,
    };
    Embedder::new(
        url,
        parsed.option("--embed-model").unwrap_or(DEFAULT_MODEL),
        parsed.option("--keep-alive").unwrap_or(DEFAULT_KEEP_ALIVE),
        timeout,
    )
    .map(Some)
    .ok_or(Usage)
}

/// The fixed vectors of a `search`: from the vector file, from the databases and the query vector
/// file, or none (the lexical way).
fn search_vectors(parsed: &Args, dbs: Option<&DbPaths>) -> Result<Option<Vectors>, Unreadable> {
    match (
        dbs,
        parsed.option("--query-vector"),
        parsed.option("--vectors"),
    ) {
        (Some(dbs), Some(query), _) => load_db_vectors(Path::new(query), dbs).map(Some),
        (None, _, Some(path)) => load_vectors(Path::new(path)).map(Some),
        _ => Ok(None),
    }
}

fn search_command(args: &[String]) -> Outcome {
    let known = [
        "-k",
        "--vectors",
        "--space",
        "--today",
        "--index-db",
        "--extra-db",
        "--query-vector",
        "--embed",
        "--embed-model",
        "--keep-alive",
        "--embed-timeout-ms",
    ];
    let Some(parsed) = parse_args(args, &known, 2) else {
        return usage();
    };
    let Ok(dbs) = db_paths(&parsed) else {
        return usage();
    };
    let Ok(embedder) = embedder_of(&parsed, dbs.as_ref()) else {
        return usage();
    };
    let k = match parsed.option("-k") {
        None => 5,
        Some(text) => match parse_positive(text) {
            Some(k) => k,
            None => return usage(),
        },
    };
    let today_text = match parsed.option("--today") {
        None => today(),
        Some(text) if is_date(text) => text.to_string(),
        Some(_) => return usage(),
    };
    let root = Path::new(&parsed.positional[0]);
    if !root.is_dir() {
        return unreadable();
    }
    let question = &parsed.positional[1];
    let found = if let (Some(embedder), Some(dbs)) = (&embedder, &dbs) {
        let Ok(notes) = load_merged(dbs) else {
            return unreadable();
        };
        let params = Params {
            k,
            vectors: None,
            space: parsed.option("--space"),
            today: &today_text,
        };
        search_embedding(root, question, &params, (embedder, &notes))
    } else {
        let Ok(vectors) = search_vectors(&parsed, dbs.as_ref()) else {
            return unreadable();
        };
        let params = Params {
            k,
            vectors: vectors.as_ref().map(Vectors::view),
            space: parsed.option("--space"),
            today: &today_text,
        };
        search(root, question, &params)
    };
    match found {
        Ok((hits, fallback)) => (
            EXIT_OK,
            Some(json!({
                "fallback": fallback,
                "hits": hits.iter().map(hit_json).collect::<Vec<_>>(),
            })),
        ),
        Err(_) => unreadable(),
    }
}

/// The licence notices that must travel with every copy of the program, source or binary: the ported
/// stemmer (BSD-3-Clause) and the Unicode data tables (Unicode-3.0). `rangkern --licenses` prints them.
pub const NOTICE: &str = include_str!("../NOTICE");

/// The usage line, written to the error output after a usage error.
const USAGE: &str = "usage: rangkern tokens <text> | snippet <file> <question> | search <vault> <question> \
     [-k K] [--vectors FILE | --index-db DB [--extra-db DB] [--query-vector FILE | --embed URL \
     [--embed-model NAME] [--keep-alive TEXT] [--embed-timeout-ms MS]]] [--space LIST] \
     [--today YYYY-MM-DD]; options before `--` \
     | serve <vault> [--vectors FILE | --index-db DB [--extra-db DB] [--embed URL \
     [--embed-model NAME] [--keep-alive TEXT] [--embed-timeout-ms MS]]] \
     (requests on standard input, one JSON line each) \
     | sitzung <vault> --socket PATH --route KEY --sitzung KEY --einstellungen FILE --protokoll FOLDER \
     [--socket-rechte 0600|0666] [--vectors FILE | --index-db DB [--extra-db DB] [--embed URL \
     [--embed-timeout-ms MS]]] (requests on the socket, Unix only) | --licenses";

/// `serve <vault> [--vectors <file> | --index-db <db> [--extra-db <db>] [--embed <url>]]`: answers the request lines
/// of `input` until its end or `quit`.
fn serve_command<I: BufRead, O: Write>(args: &[String], input: &mut I, out: &mut O) -> u8 {
    let known = [
        "--vectors",
        "--index-db",
        "--extra-db",
        "--embed",
        "--embed-model",
        "--keep-alive",
        "--embed-timeout-ms",
    ];
    let Some(parsed) = parse_args(args, &known, 1) else {
        return EXIT_USAGE;
    };
    // `serve` takes each option at most once (`search` keeps the rule of `argparse`: the last one wins).
    let repeated = known.iter().any(|name| parsed.count(name) > 1);
    let Ok(dbs) = db_paths(&parsed) else {
        return EXIT_USAGE;
    };
    let Ok(embedder) = embedder_of(&parsed, dbs.as_ref()) else {
        return EXIT_USAGE;
    };
    if repeated {
        return EXIT_USAGE;
    }
    let root = PathBuf::from(&parsed.positional[0]);
    let mut server = match dbs {
        Some(dbs) => Server::start_dbs(root, dbs).with_embedder(embedder),
        None => Server::start(root, parsed.option("--vectors").map(PathBuf::from)),
    };
    serve_lines(&mut server, input, out)
}

/// The error output that `main` hands to [`run_with_input`]: the handle without a lock. `Stderr` locks per write; a lock held by
/// `main` would live as long as `sitzung` runs, and a connection thread that writes to the error output would wait for ever
/// (review B1, 23.11.2026).
#[must_use]
pub fn fehlerausgabe() -> std::io::Stderr {
    std::io::stderr()
}

/// Runs the program with the arguments after the program name, `serve` reading no request.
/// Returns the exit code.
pub fn run<O: Write, E: Write>(args: &[String], out: &mut O, err: &mut E) -> u8 {
    run_with_input(args, &mut std::io::empty(), out, err)
}

/// Runs the program with the arguments after the program name; `input` is the standard input,
/// which only `serve` reads. Returns the exit code.
pub fn run_with_input<I: BufRead, O: Write, E: Write>(
    args: &[String],
    input: &mut I,
    out: &mut O,
    err: &mut E,
) -> u8 {
    if let [only] = args
        && only == "--licenses"
    {
        return if out.write_all(NOTICE.as_bytes()).is_ok() {
            EXIT_OK
        } else {
            EXIT_UNREADABLE
        };
    }
    if let Some((command, rest)) = args.split_first()
        && command == "serve"
    {
        let code = serve_command(rest, input, out);
        if code == EXIT_USAGE {
            let _ = writeln!(err, "{USAGE}");
        }
        return code;
    }
    if let Some((command, rest)) = args.split_first()
        && command == "sitzung"
    {
        // The search socket needs Unix sockets; the named pipe for Windows follows with its own part of the contract.
        // Until then the command ends with exit 64 (the usage error) and one line, and does not print the usage text.
        #[cfg(unix)]
        let code = {
            let code = crate::sitzung::run(rest, out, err);
            if code == EXIT_USAGE {
                let _ = writeln!(err, "{USAGE}");
            }
            code
        };
        #[cfg(not(unix))]
        let code = {
            let _ = (rest, &out);
            let _ = writeln!(
                err,
                "rangkern sitzung: unter Windows noch nicht unterstützt"
            );
            EXIT_USAGE
        };
        return code;
    }
    let (code, value) = match args.split_first() {
        Some((command, rest)) => match command.as_str() {
            "tokens" => tokens_command(rest),
            "snippet" => snippet_command(rest),
            "search" => search_command(rest),
            _ => usage(),
        },
        None => usage(),
    };
    if code == EXIT_USAGE {
        let _ = writeln!(err, "{USAGE}");
    }
    if let Some(value) = value {
        let written = serde_json::to_writer(&mut *out, &value).is_ok() && writeln!(out).is_ok();
        if !written {
            return EXIT_UNREADABLE;
        }
    }
    code
}
