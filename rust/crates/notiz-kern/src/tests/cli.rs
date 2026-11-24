//! Tests of the command line: arguments, exit codes and the JSON lines.

use serde_json::{Value, json};

use super::TempDir;
use crate::cli::{EXIT_NOT_FOUND, EXIT_OK, EXIT_UNREADABLE, EXIT_USAGE, run};

/// Runs the command line; returns exit code, standard output and error output.
fn call(args: &[&str]) -> (u8, String, String) {
    let args: Vec<String> = args.iter().map(ToString::to_string).collect();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = run(&args, &mut out, &mut err);
    (
        code,
        String::from_utf8(out).expect("stdout is UTF-8"),
        String::from_utf8(err).expect("stderr is UTF-8"),
    )
}

/// Runs the command line and expects exactly one JSON line on standard output.
fn json_call(args: &[&str]) -> (u8, Value) {
    let (code, out, err) = call(args);
    assert!(
        out.ends_with('\n') && !out[..out.len() - 1].contains('\n'),
        "{out:?}"
    );
    assert!(err.is_empty(), "{err:?}");
    (code, serde_json::from_str(&out).expect("stdout is JSON"))
}

fn path_of(path: &std::path::Path) -> String {
    path.to_string_lossy().into_owned()
}

fn usage_error(args: &[&str]) {
    let (code, out, err) = call(args);
    assert_eq!(code, EXIT_USAGE, "{args:?}");
    assert!(out.is_empty(), "{args:?}: {out:?}");
    assert!(err.starts_with("usage: notiz-kern"), "{args:?}: {err:?}");
}

#[test]
fn exit_codes_have_their_contract_values() {
    assert_eq!(EXIT_OK, 0);
    assert_eq!(EXIT_USAGE, 2);
    assert_eq!(EXIT_UNREADABLE, 3);
    assert_eq!(EXIT_NOT_FOUND, 4);
}

#[test]
fn a_wrong_call_is_a_usage_error_with_empty_stdout() {
    usage_error(&[]);
    usage_error(&["unknown"]);
    usage_error(&["parse"]);
    usage_error(&["parse", "a", "b"]);
    usage_error(&["section", "a"]);
    usage_error(&["section", "a", "b", "c"]);
    usage_error(&["paths"]);
    usage_error(&["corpus"]);
    usage_error(&["corpus", "a", "b"]);
    usage_error(&["lint", "a"]);
    usage_error(&["lint", "--rel", "r"]);
    usage_error(&["lint", "a", "--rel"]);
    usage_error(&["lint", "a", "b", "--rel", "r"]);
    usage_error(&["markers", "a", "--start", "s"]);
    usage_error(&["markers", "a", "--end", "e"]);
    usage_error(&["markers", "--start", "s", "--end", "e"]);
    usage_error(&["markers", "a", "--start", "s", "--end", "e", "--replace"]);
    usage_error(&["markers", "a", "b", "--start", "s", "--end", "e"]);
}

#[test]
fn parse_prints_the_note_as_one_json_line() {
    let dir = TempDir::new();
    let file = dir.write(
        "My Note.md",
        "---\ntitle: T\naliases: [A]\ntags:\n  - x\n---\nSee [[Other|o]].\n## Relations\n- [[R]]\n"
            .as_bytes(),
    );
    let (code, value) = json_call(&["parse", &path_of(&file)]);
    assert_eq!(code, 0);
    assert_eq!(
        value,
        json!({
            "blocks": ["title: T\naliases: [A]\ntags:\n  - x"],
            "fields": {"title": "T", "aliases": ["A"], "tags": ["x"]},
            "body": "See [[Other|o]].\n## Relations\n- [[R]]\n",
            "title": "T",
            "type": "note",
            "aliases": ["A"],
            "links": ["other", "r"],
            "keys": ["a", "my note", "t"],
            "embed_text": "T\naliases: A\ntags: x\n\nSee [[Other|o]].\n",
        })
    );
}

#[test]
fn parse_takes_the_stem_from_the_file_name() {
    let dir = TempDir::new();
    let file = dir.write("sub/Name.With.Dots.md", b"text");
    let (_, value) = json_call(&["parse", &path_of(&file)]);
    assert_eq!(value["title"], "Name.With.Dots");
    let file = dir.write("sub/.md", b"text");
    let (_, value) = json_call(&["parse", &path_of(&file)]);
    assert_eq!(value["title"], ".md");
}

#[test]
fn an_unreadable_file_is_exit_3_with_an_error_object() {
    let dir = TempDir::new();
    let missing = path_of(&dir.path().join("missing.md"));
    let folder = path_of(dir.path());
    for args in [
        vec!["parse", &missing],
        vec!["parse", &folder],
        vec!["lint", &missing, "--rel", "10-global/a.md"],
        vec!["section", &missing, "A"],
        vec!["markers", &missing, "--start", "s", "--end", "e"],
        vec!["corpus", &missing],
    ] {
        let (code, value) = json_call(&args);
        assert_eq!(code, EXIT_UNREADABLE, "{args:?}");
        assert_eq!(value, json!({"error": "unreadable"}), "{args:?}");
    }
    let file = dir.write("f.md", b"x");
    let (code, value) = json_call(&["corpus", &path_of(&file)]);
    assert_eq!(code, EXIT_UNREADABLE);
    assert_eq!(value, json!({"error": "unreadable"}));
}

#[test]
fn lint_accepts_the_option_before_or_after_the_file() {
    let dir = TempDir::new();
    let file = path_of(&dir.write("n.md", b"---\nid: x\ncreated: 2026-10-01\n---\n"));
    let expected = json!({
        "in_scope": true,
        "fields": {"id": "x", "created": "2026-10-01"},
        "blocks": 1,
        "findings": [
            {"kind": "pflichtfeld", "detail": "Pflichtfeld fehlt: schema, class, type"},
            {"kind": "id-format", "detail": "id 'x' ist keine ULID"},
        ],
        "new": true,
    });
    let (code, value) = json_call(&["lint", &file, "--rel", "10-global/n.md"]);
    assert_eq!((code, &value), (0, &expected));
    let (code, value) = json_call(&["lint", "--rel", "10-global/n.md", &file]);
    assert_eq!((code, &value), (0, &expected));
}

#[test]
fn lint_the_last_rel_wins() {
    let dir = TempDir::new();
    let file = path_of(&dir.write("n.md", b"plain"));
    let (_, value) = json_call(&["lint", &file, "--rel", "10-global/n.md", "--rel", "n.md"]);
    assert_eq!(value["in_scope"], false);
    assert_eq!(
        value["findings"],
        json!([{"kind": "pflichtfeld", "detail": "kein Frontmatter"}])
    );
    assert_eq!(value["blocks"], 0);
    assert_eq!(value["new"], false);
}

#[test]
fn paths_prints_one_object_per_path_in_input_order() {
    let (code, value) = json_call(&["paths", "30-topics/t/MOC.md", "90-secrets/x.md", "a.md"]);
    assert_eq!(code, 0);
    assert_eq!(
        value,
        json!([
            {"rel": "30-topics/t/MOC.md", "linking_excluded": true, "search_excluded": false,
             "lint_in_scope": false, "branch": "30-topics/t", "stem_key": "moc"},
            {"rel": "90-secrets/x.md", "linking_excluded": true, "search_excluded": true,
             "lint_in_scope": false, "branch": "90-secrets", "stem_key": "x"},
            {"rel": "a.md", "linking_excluded": false, "search_excluded": false,
             "lint_in_scope": false, "branch": "", "stem_key": "a"},
        ])
    );
}

#[test]
fn corpus_prints_both_lists() {
    let dir = TempDir::new();
    dir.write("a/x.md", b"x");
    dir.write("a-b/MOC.md", b"x");
    dir.write("90-secrets/s.md", b"x");
    let (code, value) = json_call(&["corpus", &path_of(dir.path())]);
    assert_eq!(code, 0);
    assert_eq!(
        value,
        json!({"linking": ["a/x.md"], "search": ["a/x.md", "a-b/MOC.md"]})
    );
}

#[test]
fn markers_reports_state_block_and_stripped_text() {
    let dir = TempDir::new();
    let file = path_of(&dir.write("n.md", b"a\n\n<!-- s -->\nx\n<!-- e -->\n\nb\n"));
    let (code, value) = json_call(&[
        "markers",
        &file,
        "--start",
        "<!-- s -->",
        "--end",
        "<!-- e -->",
    ]);
    assert_eq!(code, 0);
    assert_eq!(
        value,
        json!({"state": "one", "block": "<!-- s -->\nx\n<!-- e -->", "stripped": "a\n\nb\n"})
    );
}

#[test]
fn markers_replace_is_reported_only_when_asked() {
    let dir = TempDir::new();
    let file = path_of(&dir.write("n.md", b"a\n<!-- s -->\nx\n<!-- e -->\nb\n"));
    let args = [
        "markers",
        &file,
        "--end",
        "<!-- e -->",
        "--replace",
        "NEW",
        "--start",
        "<!-- s -->",
    ];
    let (code, value) = json_call(&args);
    assert_eq!(code, 0);
    assert_eq!(value["replaced"], "a\nNEW\nb\n");
    let (_, value) = json_call(&[
        "markers",
        &file,
        "--start",
        "<!-- s -->",
        "--end",
        "<!-- e -->",
    ]);
    assert!(value.get("replaced").is_none());
    let (_, value) = json_call(&[
        "markers",
        &file,
        "--start",
        "<!-- s -->",
        "--end",
        "<!-- e -->",
        "--replace",
        "",
    ]);
    assert_eq!(value["replaced"], "a\n\nb\n");
}

#[test]
fn markers_malformed_and_missing_blocks() {
    let dir = TempDir::new();
    let malformed = path_of(&dir.write("m.md", b"<!-- s -->\nx\n"));
    let (_, value) = json_call(&[
        "markers",
        &malformed,
        "--start",
        "<!-- s -->",
        "--end",
        "<!-- e -->",
        "--replace",
        "N",
    ]);
    assert_eq!(
        value,
        json!({"state": "malformed", "block": null, "stripped": "<!-- s -->\nx\n", "replaced": null})
    );
    let none = path_of(&dir.write("n.md", b"plain\n"));
    let (_, value) = json_call(&[
        "markers",
        &none,
        "--start",
        "<!-- s -->",
        "--end",
        "<!-- e -->",
        "--replace",
        "N",
    ]);
    assert_eq!(
        value,
        json!({"state": "none", "block": null, "stripped": "plain\n", "replaced": "plain\n"})
    );
}

#[test]
fn section_prints_the_section_or_exits_with_4() {
    let dir = TempDir::new();
    let file = path_of(&dir.write("n.md", b"# T\n## A\nx\n## B\ny\n"));
    let (code, value) = json_call(&["section", &file, "a"]);
    assert_eq!(code, 0);
    assert_eq!(value, json!({"section": "## A\nx"}));
    let (code, value) = json_call(&["section", &file, "C"]);
    assert_eq!(code, EXIT_NOT_FOUND);
    assert_eq!(value, json!({"error": "section-not-found"}));
}

#[test]
fn text_is_read_with_universal_newlines() {
    let dir = TempDir::new();
    let file = path_of(&dir.write("n.md", b"# T\r\n## A\rx\r\n## B\r\n"));
    let (_, value) = json_call(&["section", &file, "A"]);
    assert_eq!(value, json!({"section": "## A\nx"}));
}
