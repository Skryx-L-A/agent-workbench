use serde_json::{Value, json};

use super::TempDir;
use crate::cli::{
    Kind, NOTICE, civil_date, classify, hit_json, is_date, looks_negative_number, parse_args,
    parse_positive, run, today,
};
use crate::hit::SearchHit;

fn call(args: &[&str]) -> (u8, String, String) {
    let args: Vec<String> = args.iter().map(ToString::to_string).collect();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = run(&args, &mut out, &mut err);
    (
        code,
        String::from_utf8(out).expect("UTF-8"),
        String::from_utf8(err).expect("UTF-8"),
    )
}

fn json_of(args: &[&str]) -> (u8, Value) {
    let (code, out, _) = call(args);
    assert!(
        out.ends_with('\n') && !out[..out.len() - 1].contains('\n'),
        "{out:?}"
    );
    (code, serde_json::from_str(&out).expect("JSON"))
}

#[test]
fn tokens_prints_words_stems_and_index_tokens() {
    let (code, value) = json_of(&["tokens", "Einbettungen Suchindex"]);
    assert_eq!(code, 0);
    assert_eq!(value["words"], json!(["einbettungen", "suchindex"]));
    assert_eq!(value["stems"], json!(["einbett", "suchindex"]));
    assert_eq!(value["index"][0], "einbett");
    assert_eq!(value["index"][1], "#einb");
    assert_eq!(json_of(&["tokens", "--", "-x"]).1["words"], json!(["x"]));
    assert_eq!(json_of(&["tokens", "-5"]).1["words"], json!(["5"]));
    assert_eq!(json_of(&["tokens", "-a b"]).1["words"], json!(["a", "b"]));
}

#[test]
fn wrong_usage_is_exit_64_with_a_message_and_no_output() {
    let bad: [&[&str]; 22] = [
        &[],
        &["zerlege", "x"],
        &["tokens"],
        &["tokens", "a", "b"],
        &["tokens", "-x"],
        &["tokens", "--vectors", "v"],
        &["snippet", "-f", "datei", "Frage"],
        &["snippet", "datei"],
        &["search", "vault"],
        &["search", "vault", "frage", "extra"],
        &["search", "vault", "--files"],
        &["search", "vault", "-v", "x", "frage"],
        &["search", "vault", "--vec", "x", "frage"],
        &["search", "vault", "frage", "-k"],
        &["search", "vault", "frage", "-k", "0"],
        &["search", "vault", "frage", "-k", "abc"],
        &["search", "vault", "frage", "-k", "-3"],
        &["search", "vault", "frage", "--today", "2026-1-1"],
        &["search", "vault", "frage", "--space", "--today"],
        &["search", "vault", "frage", "--space", "-x"],
        &["search", "vault", "frage", "--k", "1"],
        &["-h"],
    ];
    for args in bad {
        let (code, out, err) = call(args);
        assert_eq!((code, out.as_str()), (64, ""), "{args:?}");
        assert!(err.starts_with("usage:"), "{args:?}");
    }
}

#[test]
fn an_unreadable_file_or_vault_is_exit_three_with_one_json_line() {
    let dir = TempDir::new();
    let missing = dir.path().join("fehlt");
    let missing = missing.to_str().expect("UTF-8");
    for args in [vec!["snippet", missing, "x"], vec!["search", missing, "x"]] {
        let (code, value) = json_of(&args);
        assert_eq!((code, value), (3, json!({"error": "unreadable"})));
    }
    let vault = dir.path().to_str().expect("UTF-8");
    let (code, value) = json_of(&["search", vault, "x", "--vectors", missing]);
    assert_eq!((code, value), (3, json!({"error": "unreadable"})));
    assert_eq!(json_of(&["snippet", vault, "x"]).0, 3);
}

#[test]
fn snippet_prints_the_excerpt_the_hook_and_their_lines() {
    let dir = TempDir::new();
    let path = dir.write("n.md", "---\ntitle: T\n---\n# Kopf\nGarten und Beet. Zweiter Satz.\n\nFür künftige Sessions: lies das\n".as_bytes());
    let file = path.to_str().expect("UTF-8");
    let (code, value) = json_of(&["snippet", file, "Garten"]);
    assert_eq!(code, 0);
    assert_eq!(
        value,
        json!({"snippet": "Kopf: Garten und Beet. Zweiter Satz.", "lines": [5, 5], "hook": "Für künftige Sessions: lies das", "hook_lines": [7, 7]})
    );
    let (_, none) = json_of(&["snippet", file, "und"]);
    assert_eq!(none["lines"], json!([7, 7]));
    let empty = dir.write("leer.md", b"");
    assert_eq!(
        json_of(&["snippet", empty.to_str().expect("UTF-8"), "x"]).1,
        json!({"snippet": "", "lines": null, "hook": "", "hook_lines": null})
    );
}

#[test]
fn search_runs_on_a_vault_and_takes_options_in_every_written_form() {
    let dir = TempDir::new();
    dir.write("v/a.md", b"Kaffee Maschine\n");
    dir.write("v/b.md", b"Kaffee Bohnen\n");
    let vault = dir.path().join("v");
    let vault = vault.to_str().expect("UTF-8");
    let hits = |extra: &[&str]| -> usize {
        let mut args = vec!["search", vault, "Kaffee", "--today", "2026-11-18"];
        args.extend(extra);
        let (code, value) = json_of(&args);
        assert_eq!(code, 0, "{extra:?}");
        assert_eq!(value["fallback"], true);
        value["hits"].as_array().expect("hits").len()
    };
    assert_eq!(hits(&[]), 2);
    assert_eq!(hits(&["-k", "1"]), 1);
    assert_eq!(hits(&["-k1"]), 1);
    assert_eq!(hits(&["-k=1"]), 1);
    assert_eq!(hits(&["-k", " 1 "]), 1);
    assert_eq!(hits(&["--space=kern"]), 2);
    assert_eq!(hits(&["--space", "nirgends"]), 0);
    assert_eq!(hits(&["--space", ""]), 2);
    assert_eq!(hits(&["-k", "1", "-k", "2"]), 2);
    let (_, value) = json_of(&["search", vault, "--today", "2026-11-18", "--", "-Kaffee"]);
    assert_eq!(value["hits"].as_array().expect("hits").len(), 0);
    assert_eq!(json_of(&["search", vault, "Kaffee"]).0, 0);
}

#[test]
fn classify_follows_argparse() {
    let known = ["-k", "--vectors"];
    assert!(matches!(classify("text", &known), Kind::Positional));
    assert!(matches!(classify("", &known), Kind::Positional));
    assert!(matches!(classify("-", &known), Kind::Positional));
    assert!(matches!(classify("-k", &known), Kind::Known("-k", None)));
    assert!(matches!(
        classify("-k5", &known),
        Kind::Known("-k", Some("5"))
    ));
    assert!(
        matches!(classify("-k=5", &known), Kind::Known("-k", Some("=5")))
            || matches!(classify("-k=5", &known), Kind::Known("-k", Some("5")))
    );
    assert!(matches!(
        classify("--vectors=f", &known),
        Kind::Known("--vectors", Some("f"))
    ));
    assert!(matches!(
        classify("--vectors", &known),
        Kind::Known("--vectors", None)
    ));
    assert!(matches!(classify("--vec", &known), Kind::Unknown));
    assert!(matches!(classify("--vecx=1", &known), Kind::Unknown));
    assert!(matches!(classify("-v", &known), Kind::Unknown));
    assert!(matches!(classify("-3", &known), Kind::Positional));
    assert!(matches!(classify("-.5", &known), Kind::Positional));
    assert!(matches!(classify("-a b", &known), Kind::Positional));
    assert!(matches!(classify("-ab", &known), Kind::Unknown));
    assert!(matches!(classify("-é", &known), Kind::Unknown));
}

#[test]
fn negative_numbers_are_values() {
    for yes in ["-1", "-12", "-1.5", "-.5", "-0.25"] {
        assert!(looks_negative_number(yes), "{yes}");
    }
    for no in ["-", "1", "-a", "-1a", "-1.", "-.", "-1.5.2", "--1", "-1e3"] {
        assert!(!looks_negative_number(no), "{no}");
    }
}

#[test]
fn arguments_are_split_by_the_rules_of_argparse() {
    let args = |list: &[&str]| -> Vec<String> { list.iter().map(ToString::to_string).collect() };
    let parsed = parse_args(&args(&["v", "-k", "3", "--", "--files"]), &["-k"], 2).expect("valid");
    assert_eq!(parsed.positional, ["v", "--files"]);
    assert_eq!(parsed.option("-k"), Some("3"));
    assert_eq!(parsed.option("--vectors"), None);
    let twice = parse_args(&args(&["-k", "1", "a", "-k", "2"]), &["-k"], 1).expect("valid");
    assert_eq!(twice.option("-k"), Some("2"));
    assert!(parse_args(&args(&["-k", "--", "a"]), &["-k"], 1).is_none());
    assert!(parse_args(&args(&["a", "-k"]), &["-k"], 1).is_none());
    assert!(parse_args(&args(&["a"]), &["-k"], 2).is_none());
    assert!(parse_args(&args(&["a", "b", "c"]), &["-k"], 2).is_none());
    assert!(parse_args(&args(&["--", "--", "x"]), &[], 2).is_some());
}

#[test]
fn k_is_a_positive_integer_as_python_reads_it() {
    for (text, want) in [
        ("1", 1),
        (" 5 ", 5),
        ("+5", 5),
        ("5_0", 50),
        ("007", 7),
        ("99999999999999999999999", usize::MAX),
        ("\u{a0}3", 3),
    ] {
        assert_eq!(parse_positive(text), Some(want), "{text:?}");
    }
    for text in [
        "", "0", "-1", "+", "_5", "5_", "5__0", "1.5", "1e3", "abc", "+-1", " ",
    ] {
        assert_eq!(parse_positive(text), None, "{text:?}");
    }
}

#[test]
fn a_date_is_year_month_day_with_ascii_digits() {
    assert!(is_date("2026-11-18") && is_date("0000-00-00"));
    for text in [
        "",
        "2026-1-18",
        "2026-11-1",
        "20261118",
        "2026/11/18",
        "2026-11-18 ",
        "٢٠٢٦-١١-١٨",
        "abcd-ef-gh",
    ] {
        assert!(!is_date(text), "{text:?}");
    }
}

#[test]
fn day_numbers_become_civil_dates() {
    assert_eq!(civil_date(0), (1970, 1, 1));
    assert_eq!(civil_date(-1), (1969, 12, 31));
    assert_eq!(civil_date(11_016), (2000, 2, 29));
    assert_eq!(civil_date(11_017), (2000, 3, 1));
    assert_eq!(civil_date(20_775), (2026, 11, 18));
    assert_eq!(civil_date(47_541), (2100, 3, 1));
    assert_eq!(civil_date(19_723), (2024, 1, 1));
    assert_eq!(civil_date(19_782), (2024, 2, 29));
    assert_eq!(civil_date(365), (1971, 1, 1));
}

#[test]
fn today_is_a_plausible_date() {
    let days = || {
        let seconds = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("after 1970")
            .as_secs();
        let (year, month, day) = civil_date(i64::try_from(seconds / 86_400).expect("fits"));
        format!("{year:04}-{month:02}-{day:02}")
    };
    let before = days();
    let text = today();
    let after = days();
    assert!(is_date(&text), "{text}");
    assert!(
        text == before || text == after,
        "{text} not {before} or {after}"
    );
    assert!(text.as_str() >= "2026-11-18", "{text}");
}

#[test]
fn a_hit_prints_only_the_fields_that_differ_from_their_default() {
    let plain = SearchHit::new("a.md", 0.5);
    assert_eq!(hit_json(&plain), json!({"rel": "a.md", "score": 0.5}));
    let mut full = SearchHit::new("a.md", 1.0);
    full.title = "T".into();
    full.snippet = "S".into();
    full.lines = Some((3, 4));
    full.matched = "hybrid".into();
    full.cosine = -0.25;
    full.bm25_rang = 2;
    full.bm25_norm = 0.5;
    full.vektor_rang = 7;
    full.space = "pp".into();
    full.derived = true;
    full.contradiction = true;
    full.status = "superseded".into();
    full.superseded_by = vec!["neu".into()];
    full.superseded_by_rel = vec!["neu.md".into()];
    assert_eq!(
        hit_json(&full),
        json!({"rel": "a.md", "score": 1.0, "title": "T", "snippet": "S", "lines": [3, 4], "match": "hybrid", "cosine": -0.25,
               "bm25_rang": 2, "bm25_norm": 0.5, "vektor_rang": 7, "space": "pp", "derived": true, "contradiction": true,
               "status": "superseded", "abgeloest_durch": ["neu.md"]})
    );
    full.superseded_by_rel.clear();
    assert_eq!(hit_json(&full)["abgeloest_durch"], json!(["neu"]));
    full.superseded_by.clear();
    assert!(hit_json(&full).get("abgeloest_durch").is_none());
    let mut kern = SearchHit::new("a.md", 1.0);
    kern.space = "kern".into();
    assert!(hit_json(&kern).get("space").is_none());
    let mut rank_only = SearchHit::new("a.md", 1.0);
    rank_only.bm25_rang = 1;
    assert_eq!(
        hit_json(&rank_only),
        json!({"rel": "a.md", "score": 1.0, "bm25_rang": 1, "bm25_norm": 0.0})
    );
    let mut no_status = SearchHit::new("a.md", 1.0);
    no_status.superseded_by = vec!["x".into()];
    assert!(hit_json(&no_status).get("abgeloest_durch").is_none());
}

/// A writer that refuses every write, for the exit code of a failed output.
struct Refusing;

impl std::io::Write for Refusing {
    fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
        Err(std::io::Error::other("refused"))
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[test]
fn licenses_prints_the_notice_and_nothing_else() {
    let (code, out, err) = call(&["--licenses"]);
    assert_eq!(code, 0);
    assert_eq!(out, NOTICE);
    assert!(err.is_empty(), "{err:?}");
    assert!(
        NOTICE.contains("Snowball")
            && NOTICE.contains("Redistribution and use in source and binary forms")
    );
    assert!(NOTICE.contains("UNICODE LICENSE V3") && NOTICE.contains("src/tables.rs"));
}

#[test]
fn licenses_takes_no_further_argument_and_is_no_command_option() {
    let (code, out, err) = call(&["--licenses", "x"]);
    assert_eq!((code, out.as_str()), (64, ""));
    assert!(err.contains("--licenses"), "{err:?}");
    let (code, out, _) = call(&["tokens", "--licenses"]);
    assert_eq!((code, out.as_str()), (64, ""));
}

#[test]
fn licenses_that_cannot_be_written_are_exit_3() {
    let args = vec!["--licenses".to_string()];
    let mut err = Vec::new();
    assert_eq!(run(&args, &mut Refusing, &mut err), 3);
}
