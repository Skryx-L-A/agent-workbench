//! Outside tests: run the built program on every visible case and compare exit code and JSON.
//!
//! A case `faelle/<name>.json` holds `description`, `program`, `files` (UTF-8 text), `files_b64`
//! (raw bytes as base 64), `dirs` (empty directories), `symlinks` (link name to target), `args`
//! (`{root}` stands for the case's temporary directory), `env` (extra variables) and `modes`
//! (octal permissions set after the build). The expectation `erwartet/<name>.json` holds `exit`
//! and `stdout`; a `stdout` of `null` is not compared. These are the rules of the Python runner
//! `test_aussen.py`: a fresh environment with `PATH`, `LANG=C.UTF-8`, `TZ=UTC` and `HOME=<root>/home`
//! plus the case's `env`; floats compare bit for bit (no tolerance, since the program reads numbers as Python does), an
//! integer and a float of the same value are equal, a boolean is never a number.

// Permissions and symbolic links need Unix; the program itself builds for Windows too.
#![cfg(unix)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::{Value, json};

/// The decoded value of one base 64 character, or `None` for padding and anything else.
fn base64_digit(c: u8) -> Option<u32> {
    match c {
        b'A'..=b'Z' => Some(u32::from(c - b'A')),
        b'a'..=b'z' => Some(u32::from(c - b'a') + 26),
        b'0'..=b'9' => Some(u32::from(c - b'0') + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    }
}

/// Decodes standard base 64; padding ends the input.
fn base64_decode(text: &str) -> Vec<u8> {
    let mut out = Vec::new();
    let mut bits = 0_u32;
    let mut count = 0;
    for digit in text.bytes().map_while(base64_digit) {
        bits = (bits << 6) | digit;
        count += 6;
        if count >= 8 {
            count -= 8;
            out.push(u8::try_from((bits >> count) & 0xff).expect("masked to one byte"));
        }
    }
    out
}

fn read_json(path: &Path) -> Value {
    let text = fs::read_to_string(path).expect("case file is readable");
    serde_json::from_str(&text).expect("case file is JSON")
}

fn entries<'a>(case: &'a Value, key: &str) -> Vec<(&'a String, &'a Value)> {
    case.get(key)
        .and_then(Value::as_object)
        .map(|map| map.iter().collect())
        .unwrap_or_default()
}

/// Builds the files, directories and links of a case below `root`.
fn build(case: &Value, root: &Path) {
    for dir in case
        .get("dirs")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        fs::create_dir_all(root.join(dir.as_str().expect("dir is text"))).expect("dir is created");
    }
    for (rel, text) in entries(case, "files") {
        let path = root.join(rel);
        fs::create_dir_all(path.parent().expect("file has a parent")).expect("parent is created");
        fs::write(&path, text.as_str().expect("file text").as_bytes()).expect("file is written");
    }
    for (rel, encoded) in entries(case, "files_b64") {
        let path = root.join(rel);
        fs::create_dir_all(path.parent().expect("file has a parent")).expect("parent is created");
        fs::write(&path, base64_decode(encoded.as_str().expect("base64 text")))
            .expect("file is written");
    }
    for (rel, target) in entries(case, "symlinks") {
        let path = root.join(rel);
        fs::create_dir_all(path.parent().expect("link has a parent")).expect("parent is created");
        std::os::unix::fs::symlink(target.as_str().expect("link target"), &path)
            .expect("symlink is created");
    }
}

/// Whether two JSON values are equal by the rules of the contract.
fn same(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => {
            if x.is_f64() || y.is_f64() {
                let (x, y) = (x.as_f64().expect("number"), y.as_f64().expect("number"));
                x.to_bits() == y.to_bits()
            } else {
                x == y
            }
        }
        (Value::Array(x), Value::Array(y)) => {
            x.len() == y.len() && x.iter().zip(y).all(|(p, q)| same(p, q))
        }
        (Value::Object(x), Value::Object(y)) => {
            x.len() == y.len()
                && x.iter()
                    .all(|(key, value)| y.get(key).is_some_and(|other| same(value, other)))
        }
        _ => a == b,
    }
}

/// Sets the permissions of the case, runs `f`, and restores the old permissions.
/// Returns `None` when the permissions have no effect (a process with all rights).
fn with_modes<T>(case: &Value, root: &Path, f: impl FnOnce() -> T) -> Option<T> {
    let mut saved = Vec::new();
    for (rel, octal) in entries(case, "modes") {
        let path = root.join(rel);
        let old = fs::metadata(&path).expect("path exists").permissions();
        let mode = u32::from_str_radix(octal.as_str().expect("mode text"), 8).expect("octal mode");
        fs::set_permissions(&path, fs::Permissions::from_mode(mode)).expect("mode is set");
        saved.push((path, old));
    }
    let effective = saved.iter().all(|(path, _)| {
        if path.is_dir() {
            fs::read_dir(path).is_err()
        } else {
            fs::read(path).is_err()
        }
    });
    let result = (effective || saved.is_empty()).then(f);
    for (path, old) in saved.into_iter().rev() {
        fs::set_permissions(&path, old).expect("mode is restored");
    }
    result
}

/// Runs one case; returns a description of the mismatch, or `None` when the case passes.
fn check_case(case_path: &Path, expected_path: &Path, scratch: &Path) -> Option<String> {
    let case = read_json(case_path);
    let expected = read_json(expected_path);
    let root = scratch.join(case_path.file_stem().expect("case has a name"));
    fs::create_dir_all(root.join("home")).expect("root is created");
    build(&case, &root);
    let shown = root.to_string_lossy().into_owned();
    let args: Vec<String> = case["args"]
        .as_array()
        .expect("args is a list")
        .iter()
        .map(|arg| arg.as_str().expect("arg is text").replace("{root}", &shown))
        .collect();
    let mut command = Command::new(env!("CARGO_BIN_EXE_rangkern"));
    command
        .args(&args)
        .current_dir(&root)
        .env_clear()
        .env("PATH", std::env::var_os("PATH").unwrap_or_default())
        .env("LANG", "C.UTF-8")
        .env("TZ", "UTC")
        .env("HOME", root.join("home"));
    for (name, value) in entries(&case, "env") {
        command.env(
            name,
            value.as_str().expect("env text").replace("{root}", &shown),
        );
    }
    let output = with_modes(&case, &root, || command.output().expect("program starts"))?;
    let stdout = String::from_utf8(output.stdout).expect("stdout is UTF-8");
    let code = output.status.code();
    let expected_code = expected["exit"]
        .as_i64()
        .and_then(|n| i32::try_from(n).ok());
    if code != expected_code {
        return Some(format!(
            "exit code {code:?}, expected {expected_code:?}; stdout: {stdout}"
        ));
    }
    if expected["stdout"].is_null() {
        return None;
    }
    if !stdout.ends_with('\n') || stdout[..stdout.len() - 1].contains('\n') {
        return Some(format!("stdout is not exactly one line: {stdout:?}"));
    }
    let actual: Value = match serde_json::from_str(&stdout) {
        Ok(value) => value,
        Err(error) => return Some(format!("stdout is not JSON ({error}): {stdout}")),
    };
    (!same(&actual, &expected["stdout"]))
        .then(|| format!("stdout {actual}, expected {}", expected["stdout"]))
}

/// Runs every case of the set below `tests/faelle/<set>` and fails with all mismatches.
fn run_set(set: &str) {
    let base = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/faelle")
        .join(set);
    let scratch = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("faelle-{set}-{}", std::process::id()));
    let mut names: Vec<PathBuf> = fs::read_dir(base.join("faelle"))
        .expect("case directory exists")
        .map(|entry| entry.expect("entry is readable").path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
        .collect();
    names.sort();
    assert!(!names.is_empty(), "no cases in set {set}");
    let mut failures = Vec::new();
    for case_path in &names {
        let expected_path = base
            .join("erwartet")
            .join(case_path.file_name().expect("case file name"));
        if let Some(problem) = check_case(case_path, &expected_path, &scratch) {
            failures.push(format!("{}: {problem}", case_path.display()));
        }
    }
    fs::remove_dir_all(&scratch).expect("scratch directory is removed");
    assert!(
        failures.is_empty(),
        "{} of {} cases failed:\n{}",
        failures.len(),
        names.len(),
        failures.join("\n")
    );
}

#[test]
fn visible_cases_of_package_two_pass() {
    run_set("paket2");
}

#[test]
fn visible_cases_of_package_three_pass() {
    run_set("paket3");
}

#[test]
fn visible_cases_of_package_four_pass() {
    run_set("paket4");
}

#[test]
fn invented_vaults_agree_with_the_reference_of_the_safe_starter() {
    run_set("eigen");
}

// ---- guard: no case reads the real date ------------------------------------------------------------

/// The first day the invented cases ran. Their expectations come from the reference of that day.
const FIRST_RUN_DAY: &str = "2026-11-18";

/// Whether `text` is `YYYY`, `YYYY-MM` or `YYYY-MM-DD` with ASCII digits: the values of `valid-until`
/// that the program compares with the date. Any other text never expires a note.
fn is_date_prefix(text: &str) -> bool {
    let parts: Vec<&str> = text.split('-').collect();
    let sizes_fit = match parts.len() {
        1 => parts[0].len() == 4,
        2 | 3 => parts[0].len() == 4 && parts[1..].iter().all(|part| part.len() == 2),
        _ => false,
    };
    sizes_fit
        && parts
            .iter()
            .all(|part| part.bytes().all(|b| b.is_ascii_digit()))
}

/// The date values that follow `valid-until:` in a text, quotes and blanks skipped.
fn valid_until_dates(text: &str) -> Vec<String> {
    const KEY: &str = "valid-until:";
    text.match_indices(KEY)
        .map(|(at, _)| {
            text[at + KEY.len()..]
                .trim_start_matches([' ', '\t', '\'', '"'])
                .chars()
                .take_while(|c| c.is_ascii_digit() || *c == '-')
                .collect::<String>()
        })
        .filter(|value| is_date_prefix(value))
        .collect()
}

/// Whether a `valid-until` value gives the same answer on every day from [`FIRST_RUN_DAY`] on: it is
/// already over on that day (compared in its own precision, as the program does), or it lies beyond
/// the year 2100.
fn same_on_every_day(until: &str) -> bool {
    let day: String = FIRST_RUN_DAY.chars().take(until.len()).collect();
    until < day.as_str() || until >= "2100"
}

/// Every text of a case file; the values of `files_b64` are decoded first.
fn texts_of(value: &Value, key: &str, out: &mut Vec<String>) {
    match value {
        Value::String(text) if key == "files_b64" => {
            out.push(String::from_utf8_lossy(&base64_decode(text)).into_owned());
        }
        Value::String(text) => out.push(text.clone()),
        Value::Array(items) => items.iter().for_each(|item| texts_of(item, key, out)),
        Value::Object(map) => {
            for (name, item) in map {
                texts_of(item, if key == "files_b64" { key } else { name }, out);
            }
        }
        _ => {}
    }
}

/// Whether a case fixes the date: `--today` among the arguments of a search case, or a `today` in every
/// search request of a serve case.
fn fixes_the_date(case: &Value) -> bool {
    match case["steps"].as_array() {
        Some(steps) => steps
            .iter()
            .all(|step| step["request"]["op"] != "search" || step["request"]["today"].is_string()),
        None => case["args"]
            .as_array()
            .is_some_and(|args| args.iter().any(|arg| arg == "--today")),
    }
}

/// The `valid-until` dates of a case that give different answers on different days.
fn date_dependent_values(case: &Value) -> Vec<String> {
    let mut texts = Vec::new();
    texts_of(case, "", &mut texts);
    texts
        .iter()
        .flat_map(|text| valid_until_dates(text))
        .filter(|until| !same_on_every_day(until))
        .collect()
}

/// Every case file below `tests/faelle*/<set>/faelle`. The case folders are read from the disk, so a
/// new folder of cases is guarded without a change here.
fn all_case_files() -> Vec<PathBuf> {
    let tests = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests");
    let mut files = Vec::new();
    for base in fs::read_dir(&tests).expect("tests folder exists") {
        let base = base.expect("entry is readable").path();
        let is_case_folder = base.is_dir()
            && base
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("faelle"));
        if !is_case_folder {
            continue;
        }
        for set in fs::read_dir(&base).expect("case folder is readable") {
            let dir = set.expect("entry is readable").path().join("faelle");
            if let Ok(cases) = fs::read_dir(&dir) {
                files.extend(
                    cases
                        .map(|entry| entry.expect("entry is readable").path())
                        .filter(|path| path.extension().is_some_and(|ext| ext == "json")),
                );
            }
        }
    }
    files.sort();
    files
}

#[test]
fn a_case_with_valid_until_fixes_the_date_unless_the_value_ignores_the_day() {
    let mut loose = Vec::new();
    let mut fixed = 0;
    let files = all_case_files();
    for path in &files {
        let case = read_json(path);
        let values = date_dependent_values(&case);
        if values.is_empty() {
            continue;
        }
        if fixes_the_date(&case) {
            fixed += 1;
        } else {
            loose.push(format!("{}: valid-until {values:?}", path.display()));
        }
    }
    assert!(
        files.len() > 200,
        "the guard sees only {} cases",
        files.len()
    );
    assert!(
        fixed >= 10,
        "the guard sees only {fixed} cases with a fixed date"
    );
    assert!(
        loose.is_empty(),
        "these cases read the real date; give them `--today` (or `today` in every search request):\n{}",
        loose.join("\n")
    );
}

#[test]
fn the_guard_tells_dates_that_move_from_dates_that_do_not() {
    assert_eq!(
        valid_until_dates(
            "a\nvalid-until: 2026-11-18\nb: valid-until:'2026-09' valid-until: x valid-until: 2026-1 valid-until:\n"
        ),
        ["2026-11-18", "2026-09"]
    );
    for until in [
        "2026-11-17",
        "2025",
        "2020-01-01",
        "2026-09",
        "2999-12-31",
        "2100",
    ] {
        assert!(is_date_prefix(until) && same_on_every_day(until), "{until}");
    }
    for until in [
        "2026-11-18",
        "2026-11-19",
        "2026-11",
        "2026-12-31",
        "2027-01-01",
        "2026",
        "2099-12-31",
    ] {
        assert!(
            is_date_prefix(until) && !same_on_every_day(until),
            "{until}"
        );
    }
    assert!(!is_date_prefix("2026-1") && !is_date_prefix("2026-11-1") && !is_date_prefix("20x6"));
    assert!(!is_date_prefix("") && !is_date_prefix("2026-11-18-1"));
    let search = |args: &[&str]| json!({ "args": args });
    assert!(fixes_the_date(&search(&[
        "search",
        "--today",
        "2026-11-18",
        "--",
        "q"
    ])));
    assert!(!fixes_the_date(&search(&["search", "--", "q"])));
    assert!(fixes_the_date(
        &json!({"steps": [{"request": {"op": "search", "today": "2026-11-18"}}, {"write": {}}]})
    ));
    assert!(!fixes_the_date(
        &json!({"steps": [{"request": {"op": "search", "today": null}}]})
    ));
    assert!(fixes_the_date(
        &json!({"steps": [{"request": {"op": "ping"}}]})
    ));
}
