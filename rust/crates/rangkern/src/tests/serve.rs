use std::io::{BufReader, Cursor, Write};

use serde_json::{Value, json};

use super::{ROUNDING_CASES, TempDir, guarded};
use crate::cli::run_with_input;
use crate::serve::{
    LINE_MAX, Line, Op, Pairs, Request, SearchRequest, Server, next_line, parse_request,
    serve_lines,
};
use crate::stamp::Detect;

/// Reads every line of `input` through a reader with a tiny buffer, so that a line spans chunks.
/// A reader that does not advance would loop for ever; see `guarded`.
fn lines_of(input: &[u8]) -> Vec<(Vec<u8>, bool)> {
    let input = input.to_vec();
    guarded(move || {
        let mut reader = BufReader::with_capacity(3, Cursor::new(input));
        let mut out = Vec::new();
        while let Some(Line { bytes, too_long }) = next_line(&mut reader).expect("reads") {
            out.push((bytes, too_long));
            if out.len() > 10 {
                break;
            }
        }
        out
    })
}

#[test]
fn a_line_may_have_one_mebibyte() {
    assert_eq!(LINE_MAX, 1_048_576);
}

#[test]
fn a_carriage_return_inside_the_cut_end_does_not_hide_a_long_line() {
    let mut bytes = vec![b'x'; LINE_MAX];
    bytes.extend_from_slice(b"\ry\nok\n");
    let lines = lines_of(&bytes);
    assert_eq!(lines.len(), 2);
    assert!(lines[0].1);
    assert_eq!(lines[1], (b"ok".to_vec(), false));
}

#[test]
fn lines_end_at_a_newline_lose_a_carriage_return_and_may_end_the_input() {
    let plain = |text: &str| (text.as_bytes().to_vec(), false);
    assert_eq!(lines_of(b""), []);
    assert_eq!(lines_of(b"\n"), [plain("")]);
    assert_eq!(lines_of(b"a\nbc\n"), [plain("a"), plain("bc")]);
    assert_eq!(lines_of(b"a\r\nbc\r\n"), [plain("a"), plain("bc")]);
    assert_eq!(lines_of(b"a\nlast"), [plain("a"), plain("last")]);
    assert_eq!(lines_of(b"a\r"), [plain("a")]);
    assert_eq!(lines_of(b"a\r\r\n"), [plain("a\r")]);
    assert_eq!(lines_of(b"\r\n\n"), [plain(""), plain("")]);
    assert_eq!(lines_of(b"x\n\ny"), [plain("x"), plain(""), plain("y")]);
}

#[test]
fn a_line_over_one_mebibyte_is_too_long_and_the_next_one_is_read_on() {
    let line = |len: usize, ending: &[u8]| {
        let mut bytes = vec![b'x'; len];
        bytes.extend_from_slice(ending);
        bytes.extend_from_slice(b"ok\n");
        lines_of(&bytes)
    };
    for ending in [&b"\n"[..], b"\r\n"] {
        let fits = line(LINE_MAX, ending);
        assert_eq!(fits.len(), 2);
        assert!(!fits[0].1 && fits[0].0.len() == LINE_MAX);
        assert_eq!(fits[1], (b"ok".to_vec(), false));
        let over = line(LINE_MAX + 1, ending);
        assert_eq!(over.len(), 2);
        assert!(over[0].1);
        assert_eq!(over[1], (b"ok".to_vec(), false));
        let far = line(LINE_MAX * 2 + 5, ending);
        assert!(far[0].1 && far.len() == 2);
    }
    let unended = lines_of(&vec![b'y'; LINE_MAX + 1]);
    assert_eq!(unended.len(), 1);
    assert!(unended[0].1);
    let exact = lines_of(&vec![b'y'; LINE_MAX]);
    assert!(!exact[0].1 && exact[0].0.len() == LINE_MAX);
}

#[test]
fn a_reader_that_fails_gives_the_error() {
    struct Failing;
    impl std::io::Read for Failing {
        fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
            Err(std::io::Error::other("broken"))
        }
    }
    assert!(next_line(&mut BufReader::new(Failing)).is_err());
}

fn parsed(line: &str) -> Result<Request, Option<Value>> {
    parse_request(line.as_bytes())
}

fn search_of(line: &str) -> SearchRequest {
    match parsed(line).expect("valid").op {
        Op::Search(search) => search,
        other => panic!("not a search: {other:?}"),
    }
}

#[test]
fn a_request_has_an_operation_and_optional_fields() {
    assert_eq!(
        parsed(r#"{"op": "ping"}"#),
        Ok(Request {
            id: None,
            op: Op::Ping
        })
    );
    assert_eq!(
        parsed(r#"{"op":"quit","id":"q1"}"#),
        Ok(Request {
            id: Some(json!("q1")),
            op: Op::Quit
        })
    );
    assert_eq!(
        parsed(r#" {"op":"ping","id":-7} "#).expect("valid").id,
        Some(json!(-7))
    );
    let search = search_of(r#"{"op":"search","query":"-x"}"#);
    assert_eq!(
        search,
        SearchRequest {
            query: "-x".into(),
            k: 5,
            space: None,
            today: None,
            query_vector: None
        }
    );
    let full = search_of(
        r#"{"op":"search","query":"","k":18446744073709551615,"space":"a,b","today":"2026-02-03","query_vector":[1,2.5,-3e2]}"#,
    );
    assert_eq!(full.query, "");
    assert_eq!(full.k, usize::MAX);
    for (text, k) in [
        ("1", 1),
        ("7", 7),
        ("1048576", 1_048_576),
        ("99999999999999999999999999", usize::MAX),
    ] {
        let line = format!(r#"{{"op":"search","query":"q","k":{text}}}"#);
        assert_eq!(search_of(&line).k, k, "{text}");
    }
    assert_eq!(full.space.as_deref(), Some("a,b"));
    assert_eq!(full.today.as_deref(), Some("2026-02-03"));
    assert_eq!(full.query_vector, Some(vec![1.0, 2.5, -300.0]));
    let nulls =
        search_of(r#"{"op":"search","query":"q","space":null,"today":null,"query_vector":null}"#);
    assert_eq!(
        (nulls.space, nulls.today, nulls.query_vector),
        (None, None, None)
    );
    assert_eq!(
        parsed(r#"{"op":"ping","id":18446744073709551615}"#)
            .expect("valid")
            .id,
        Some(json!(18_446_744_073_709_551_615_u64))
    );
}

#[test]
fn every_invalid_line_is_refused() {
    let invalid = [
        "",
        " ",
        "x",
        "[]",
        "5",
        "null",
        "\"op\"",
        "{",
        r#"{"op":"ping"} x"#,
        r#"{"op":"ping"}{"op":"ping"}"#,
        "{\"op\":\"ping\"}\u{0}",
        "{}",
        r#"{"op":"ping","op":"ping"}"#,
        r#"{"op":"ping","k":1,"k":2}"#,
        r#"{"op":"nope"}"#,
        r#"{"op":5}"#,
        r#"{"op":null}"#,
        r#"{"op":"PING"}"#,
        r#"{"op":"ping","extra":1}"#,
        r#"{"Op":"ping"}"#,
        r#"{"op":"search"}"#,
        r#"{"op":"search","query":5}"#,
        r#"{"op":"search","query":null}"#,
        r#"{"op":"search","query":["a"]}"#,
        r#"{"op":"ping","query":"x"}"#,
        r#"{"op":"quit","query":"x"}"#,
        r#"{"op":"ping","query":null}"#,
        r#"{"op":"search","query":"x","k":5.0}"#,
        r#"{"op":"search","query":"x","k":"5"}"#,
        r#"{"op":"search","query":"x","k":true}"#,
        r#"{"op":"search","query":"x","k":null}"#,
        r#"{"op":"search","query":"x","k":0}"#,
        r#"{"op":"search","query":"x","k":-1}"#,
        r#"{"op":"search","query":"x","k":1e2}"#,
        r#"{"op":"search","query":"x","k":[1]}"#,
        r#"{"op":"search","query":"x","space":5}"#,
        r#"{"op":"search","query":"x","space":["a"]}"#,
        r#"{"op":"search","query":"x","today":"2026-1-1"}"#,
        r#"{"op":"search","query":"x","today":"20261118"}"#,
        r#"{"op":"search","query":"x","today":"2026/11/18"}"#,
        r#"{"op":"search","query":"x","today":"٢٠٢٦-١١-١٨"}"#,
        r#"{"op":"search","query":"x","today":"2026-11-18 "}"#,
        r#"{"op":"search","query":"x","today":5}"#,
        r#"{"op":"search","query":"x","query_vector":[]}"#,
        r#"{"op":"search","query":"x","query_vector":[1,"a"]}"#,
        r#"{"op":"search","query":"x","query_vector":[true]}"#,
        r#"{"op":"search","query":"x","query_vector":[null]}"#,
        r#"{"op":"search","query":"x","query_vector":[[1]]}"#,
        r#"{"op":"search","query":"x","query_vector":1}"#,
        r#"{"op":"search","query":"x","query_vector":"1"}"#,
        r#"{"op":"ping","id":[1]}"#,
        r#"{"op":"ping","id":{"a":1}}"#,
        r#"{"op":"ping","id":1.5}"#,
        r#"{"op":"ping","id":5.0}"#,
        r#"{"op":"ping","id":true}"#,
        r#"{"op":"ping","id":null}"#,
        r#"{"op":"ping","id":1,"id":2}"#,
        r#"{"op":"ping","k":0}"#,
        r#"{"op":"ping","k":1}"#,
        r#"{"op":"quit","space":null}"#,
        r#"{"op":"ping","today":"2026-01-01"}"#,
        r#"{"op":"quit","query_vector":[1]}"#,
        r#"{"op":"quit","today":"x"}"#,
    ];
    for line in invalid {
        assert!(parsed(line).is_err(), "{line:?}");
    }
    assert!(parse_request(b"{\"op\":\"ping\",\"id\":\"\xff\"}").is_err());
    assert!(parse_request(b"\xff").is_err());
    let deep = format!(
        "{{\"op\":\"ping\",\"query_vector\":{}1{}}}",
        "[".repeat(300),
        "]".repeat(300)
    );
    assert!(parsed(&deep).is_err());
}

#[test]
fn a_refusal_carries_the_id_when_the_line_is_an_object_with_one_valid_id() {
    let id = |line: &str| parsed(line).expect_err("invalid");
    assert_eq!(id(r#"{"op":"nope","id":7}"#), Some(json!(7)));
    assert_eq!(id(r#"{"op":"nope","id":"a"}"#), Some(json!("a")));
    assert_eq!(id(r#"{"id":-1}"#), Some(json!(-1)));
    assert_eq!(id(r#"{"op":"ping","id":3,"k":0}"#), Some(json!(3)));
    assert_eq!(id(r#"{"op":"ping","id":3,"k":1,"k":2}"#), Some(json!(3)));
    assert_eq!(id(r#"{"op":"ping","id":3,"extra":1}"#), Some(json!(3)));
    assert_eq!(id(r#"{"op":"ping","id":3,"query":"x"}"#), Some(json!(3)));
    assert_eq!(id(r#"{"op":"nope","id":1.5}"#), None);
    assert_eq!(id(r#"{"op":"nope","id":[1]}"#), None);
    assert_eq!(id(r#"{"op":"nope","id":true}"#), None);
    assert_eq!(id(r#"{"op":"nope","id":1,"id":1}"#), None);
    assert_eq!(id(r#"{"op":"nope"}"#), None);
    assert_eq!(id(r#"[{"id":1}]"#), None);
    assert_eq!(id(r#"{"id":1} x"#), None);
}

fn server(dir: &TempDir) -> Server {
    Server::start_with(dir.path().join("vault"), None, Detect::Metadata)
}

fn ask(server: &mut Server, line: &str) -> (Value, bool) {
    server.answer(&Line {
        bytes: line.as_bytes().to_vec(),
        too_long: false,
    })
}

#[test]
fn ping_and_quit_answer_ok_with_the_id_and_a_bad_line_is_refused_with_it() {
    let dir = TempDir::new();
    let mut server = server(&dir);
    assert_eq!(
        ask(&mut server, r#"{"op":"ping"}"#),
        (json!({"ok": true}), false)
    );
    assert_eq!(
        ask(&mut server, r#"{"op":"ping","id":"a"}"#),
        (json!({"ok": true, "id": "a"}), false)
    );
    assert_eq!(
        ask(&mut server, r#"{"op":"quit","id":4}"#),
        (json!({"ok": true, "id": 4}), true)
    );
    assert_eq!(
        ask(&mut server, r#"{"op":"quit"}"#),
        (json!({"ok": true}), true)
    );
    assert_eq!(
        ask(&mut server, r#"{"op":"x","id":4}"#),
        (json!({"error": "bad_request", "id": 4}), false)
    );
    assert_eq!(
        ask(&mut server, "nonsense"),
        (json!({"error": "bad_request"}), false)
    );
    let long = Line {
        bytes: br#"{"op":"ping","id":1}"#.to_vec(),
        too_long: true,
    };
    assert_eq!(
        server.answer(&long),
        (json!({"error": "bad_request"}), false)
    );
}

#[test]
fn a_search_answers_what_search_prints_with_the_id() {
    let dir = TempDir::new();
    dir.write(
        "vault/a.md",
        b"---\ntitle: Garten\n---\nBeet und Gartenarbeit\n",
    );
    dir.write("vault/b.md", b"Kaffee\n");
    let mut server = server(&dir);
    let (answer, quit) = ask(
        &mut server,
        r#"{"op":"search","query":"garten","id":9,"today":"2026-11-18"}"#,
    );
    assert!(!quit);
    assert_eq!(answer["id"], json!(9));
    assert_eq!(answer["fallback"], json!(true));
    assert_eq!(answer["hits"][0]["rel"], json!("a.md"));
    assert_eq!(answer["hits"][0]["title"], json!("Garten"));
    assert_eq!(answer["hits"].as_array().expect("list").len(), 1);
    let (without_id, _) = ask(&mut server, r#"{"op":"search","query":"kaffee"}"#);
    assert!(without_id.get("id").is_none());
    assert_eq!(without_id["hits"][0]["rel"], json!("b.md"));
}

#[test]
fn an_unreadable_vault_is_answered_unreadable_until_it_can_be_read() {
    let dir = TempDir::new();
    let mut server = server(&dir);
    let line = r#"{"op":"search","query":"x","id":1}"#;
    assert_eq!(
        ask(&mut server, line),
        (json!({"error": "unreadable", "id": 1}), false)
    );
    dir.write("vault/a.md", b"x\n");
    let (answer, _) = ask(&mut server, line);
    assert_eq!(answer["hits"][0]["rel"], json!("a.md"));
}

#[test]
fn the_vector_file_is_read_only_for_a_request_with_a_vector() {
    let dir = TempDir::new();
    dir.write("vault/a.md", b"Beet\n");
    dir.write("vault/b.md", b"Beet Beet\n");
    let vectors = dir.path().join("vectors.json");
    let mut server = Server::start_with(
        dir.path().join("vault"),
        Some(vectors.clone()),
        Detect::Metadata,
    );
    let plain = r#"{"op":"search","query":"beet","today":"2026-11-18"}"#;
    let with = r#"{"op":"search","query":"beet","today":"2026-11-18","query_vector":[1,0],"id":2}"#;
    assert_eq!(ask(&mut server, plain).0["fallback"], json!(true));
    assert_eq!(
        ask(&mut server, with).0,
        json!({"error": "unreadable", "id": 2})
    );
    dir.write(
        "vectors.json",
        b"{\"notes\": {\"a.md\": [1, 0], \"b.md\": [0, 1]}}",
    );
    let (answer, _) = ask(&mut server, with);
    assert_eq!(answer["fallback"], json!(false));
    assert_eq!(answer["hits"][0]["rel"], json!("a.md"));
    assert_eq!(answer["hits"][0]["vektor_rang"], json!(1));
    dir.write(
        "vectors.json",
        b"{\"notes\": {\"a.md\": [0, 1], \"b.md\": [1, 0]}}",
    );
    assert_eq!(ask(&mut server, with).0["hits"][0]["rel"], json!("b.md"));
    dir.write("vectors.json", b"{\"notes\": [");
    assert_eq!(
        ask(&mut server, with).0,
        json!({"error": "unreadable", "id": 2})
    );
    assert_eq!(ask(&mut server, plain).0["fallback"], json!(true));
    std::fs::remove_file(&vectors).expect("removed");
    assert_eq!(
        ask(&mut server, with).0,
        json!({"error": "unreadable", "id": 2})
    );
}

#[test]
fn no_vector_file_means_no_note_vectors_and_the_hybrid_way() {
    let dir = TempDir::new();
    dir.write("vault/a.md", b"Beet Beet\n");
    dir.write("vault/b.md", b"Kaffee\n");
    dir.write("vault/c.md", b"Wasserfall\n");
    let mut server = server(&dir);
    let (answer, _) = ask(
        &mut server,
        r#"{"op":"search","query":"beet","query_vector":[1],"today":"2026-11-18"}"#,
    );
    assert_eq!(answer["fallback"], json!(false));
    assert_eq!(answer["hits"][0]["rel"], json!("a.md"));
    assert!(answer["hits"][0].get("vektor_rang").is_none());
}

#[test]
fn a_vector_file_rewritten_with_the_same_content_is_kept_and_a_changed_one_is_read() {
    let dir = TempDir::new();
    dir.write("vault/a.md", b"Beet\n");
    dir.write("vault/b.md", b"Beet\n");
    let vectors = dir.path().join("vectors.json");
    let mut server = Server::start_with(
        dir.path().join("vault"),
        Some(vectors.clone()),
        Detect::Content,
    );
    let with = r#"{"op":"search","query":"beet","today":"2026-11-18","query_vector":[1,0]}"#;
    let first = b"{\"notes\": {\"a.md\": [1, 0], \"b.md\": [0, 1]}}";
    dir.write("vectors.json", first);
    let a = ask(&mut server, with).0;
    dir.write("vectors.json", first);
    assert_eq!(ask(&mut server, with).0, a);
    dir.write(
        "vectors.json",
        b"{\"notes\": {\"a.md\": [0, 1], \"b.md\": [1, 0]}}",
    );
    let b = ask(&mut server, with).0;
    assert_ne!(a, b);
    assert_eq!(b["hits"][0]["rel"], json!("b.md"));
}

/// A writer that refuses every write or every flush.
struct Refusing {
    refuse_write: bool,
}

impl Write for Refusing {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if self.refuse_write {
            Err(std::io::Error::other("refused"))
        } else {
            Ok(bytes.len())
        }
    }
    fn flush(&mut self) -> std::io::Result<()> {
        if self.refuse_write {
            Ok(())
        } else {
            Err(std::io::Error::other("refused"))
        }
    }
}

#[test]
fn the_lines_are_answered_in_order_until_quit_or_the_end_and_a_failed_write_is_exit_3() {
    let dir = TempDir::new();
    let server = std::sync::Arc::new(std::sync::Mutex::new(server(&dir)));
    let run = |input: &'static str| {
        let server = std::sync::Arc::clone(&server);
        guarded(move || {
            let mut out = Vec::new();
            let code = serve_lines(
                &mut server.lock().expect("lock"),
                &mut Cursor::new(input.as_bytes().to_vec()),
                &mut out,
            );
            (code, String::from_utf8(out).expect("UTF-8"))
        })
    };
    assert_eq!(run(""), (0, String::new()));
    let (code, out) = run(
        "{\"op\":\"ping\",\"id\":1}\r\n\nbad\n{\"op\":\"quit\",\"id\":2}\n{\"op\":\"ping\",\"id\":3}\n",
    );
    assert_eq!(code, 0);
    let answers: Vec<Value> = out
        .lines()
        .map(|line| serde_json::from_str(line).expect("JSON"))
        .collect();
    assert_eq!(
        answers,
        [
            json!({"ok": true, "id": 1}),
            json!({"error": "bad_request"}),
            json!({"error": "bad_request"}),
            json!({"ok": true, "id": 2}),
        ]
    );
    assert!(out.ends_with("}\n"));
    let (code, out) = run("{\"op\":\"ping\"}");
    assert_eq!((code, out.as_str()), (0, "{\"ok\":true}\n"));
    for refuse_write in [true, false] {
        let server = std::sync::Arc::clone(&server);
        let code = guarded(move || {
            let mut out = Refusing { refuse_write };
            let input = b"{\"op\":\"ping\"}\n{\"op\":\"ping\"}\n";
            let mut server = server.lock().expect("lock");
            serve_lines(&mut server, &mut Cursor::new(input.to_vec()), &mut out)
        });
        assert_eq!(code, 3, "refuse write {refuse_write}");
    }
}

fn exchange(args: &[&str], input: &str) -> (u8, String, String) {
    let args: Vec<String> = args.iter().map(ToString::to_string).collect();
    let input = input.as_bytes().to_vec();
    guarded(move || {
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let code = run_with_input(&args, &mut Cursor::new(input), &mut out, &mut err);
        (
            code,
            String::from_utf8(out).expect("UTF-8"),
            String::from_utf8(err).expect("UTF-8"),
        )
    })
}

#[test]
fn the_command_takes_a_vault_and_the_option_vectors_and_nothing_else() {
    let dir = TempDir::new();
    let vault = dir.path().join("v").to_string_lossy().into_owned();
    let (code, out, err) = exchange(&["serve", &vault], "{\"op\":\"ping\"}\n");
    assert_eq!(
        (code, out.as_str(), err.as_str()),
        (0, "{\"ok\":true}\n", "")
    );
    let (code, out, _) = exchange(
        &["serve", &vault, "--vectors", "x.json"],
        "{\"op\":\"ping\"}\n",
    );
    assert_eq!((code, out.as_str()), (0, "{\"ok\":true}\n"));
    let (code, out, _) = exchange(
        &["serve", "--vectors=x.json", &vault],
        "{\"op\":\"ping\"}\n",
    );
    assert_eq!((code, out.as_str()), (0, "{\"ok\":true}\n"));
    let (code, out, _) = exchange(&["serve", "--", "-v"], "{\"op\":\"ping\"}\n");
    assert_eq!((code, out.as_str()), (0, "{\"ok\":true}\n"));
    let bad: [&[&str]; 11] = [
        &["serve"],
        &["serve", "a", "b"],
        &["serve", "a", "--vec", "x"],
        &["serve", "a", "-v", "x"],
        &["serve", "a", "--vectors"],
        &["serve", "a", "--space", "x"],
        &["serve", "a", "--today", "2026-01-01"],
        &["serve", "--licenses"],
        &["serve", "a", "--licenses"],
        &["serve", "-h"],
        &["serve", "--vectors", "x"],
    ];
    for args in bad {
        let (code, out, err) = exchange(args, "{\"op\":\"ping\"}\n");
        assert_eq!((code, out.as_str()), (64, ""), "{args:?}");
        assert!(
            err.starts_with("usage:") && err.contains("serve"),
            "{args:?}"
        );
    }
}

#[test]
fn run_without_input_serves_nothing() {
    let args = vec!["serve".to_string(), "somewhere".to_string()];
    let (code, out, err) = guarded(move || {
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let code = crate::cli::run(&args, &mut out, &mut err);
        (code, out, err)
    });
    assert_eq!(code, 0);
    assert!(out.is_empty() && err.is_empty());
}

#[test]
fn the_visitor_says_what_it_expected() {
    let message = |text: &str| {
        serde_json::from_str::<Pairs>(text)
            .expect_err("invalid")
            .to_string()
    };
    assert!(message("5").contains("an object"), "{}", message("5"));
    let pairs = serde_json::from_str::<Pairs>(r#"{"a": 1, "a": [2], "b": null}"#).expect("valid");
    let shown: Vec<(&str, &str)> = pairs
        .0
        .iter()
        .map(|(key, text)| (key.as_str(), text.get()))
        .collect();
    assert_eq!(shown, [("a", "1"), ("a", "[2]"), ("b", "null")]);
}

#[test]
fn the_query_vector_of_a_request_is_read_bit_for_bit_as_python_reads_it() {
    // The same numbers as for the vector file: the request goes through `serde_json` with `float_roundtrip`.
    let texts: Vec<&str> = ROUNDING_CASES.iter().map(|&(text, _)| text).collect();
    let line = format!(
        r#"{{"op":"search","query":"q","query_vector":[{}]}}"#,
        texts.join(",")
    );
    let vector = search_of(&line).query_vector.expect("a query vector");
    let bits: Vec<u64> = vector.iter().map(|number| number.to_bits()).collect();
    let want: Vec<u64> = ROUNDING_CASES.iter().map(|&(_, bits)| bits).collect();
    assert_eq!(bits, want);
}
