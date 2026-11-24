//! Outside tests: run the built program on every visible case and compare exit code and JSON.
//!
//! A case `faelle/<name>.json` holds `description`, `files` (UTF-8 text), `files_b64` (raw bytes
//! as base 64), `dirs` (empty directories), `symlinks` (link name to target) and `args`, where
//! `{root}` stands for the case's temporary directory. The expectation `erwartet/<name>.json`
//! holds `exit` and `stdout`; a `stdout` of `null` is not compared. These are the same rules as
//! the Python runner `test_aussen.py`.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

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

#[cfg(unix)]
fn make_symlink(target: &str, link: &Path) {
    std::os::unix::fs::symlink(target, link).expect("symlink is created");
}

#[cfg(not(unix))]
fn make_symlink(_target: &str, _link: &Path) {
    unreachable!("cases with symlinks are skipped off Unix");
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
        make_symlink(target.as_str().expect("link target"), &path);
    }
}

/// Runs one case; returns a description of the mismatch, or `None` when the case passes.
fn check_case(case_path: &Path, expected_path: &Path, scratch: &Path) -> Option<String> {
    let case = read_json(case_path);
    let expected = read_json(expected_path);
    if !cfg!(unix) && case.get("symlinks").is_some() {
        return None;
    }
    let root = scratch.join(case_path.file_stem().expect("case has a name"));
    fs::create_dir_all(&root).expect("root is created");
    build(&case, &root);
    let args: Vec<String> = case["args"]
        .as_array()
        .expect("args is a list")
        .iter()
        .map(|arg| {
            arg.as_str()
                .expect("arg is text")
                .replace("{root}", &root.to_string_lossy())
        })
        .collect();
    let output = Command::new(env!("CARGO_BIN_EXE_notiz-kern"))
        .args(&args)
        .current_dir(&root)
        .output()
        .expect("program starts");
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
    (actual != expected["stdout"])
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
fn visible_cases_of_the_probe_set_pass() {
    run_set("probe");
}

#[test]
fn visible_cases_of_package_one_pass() {
    run_set("paket1");
}

#[test]
fn folded_spellings_of_excluded_names_stay_excluded() {
    run_set("nachtrag1");
}
