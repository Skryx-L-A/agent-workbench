use std::path::Path;

use super::{ROUNDING_CASES, TempDir};
use crate::vault::{
    Manifest, NoteVectors, Vectors, contradicted_rels, corpus_rels, load_query_vector,
    load_search_notes, load_vectors, parse_note_vectors, parse_vector, read_note, scalar_text,
};

fn rels(dir: &TempDir) -> Vec<String> {
    load_search_notes(dir.path())
        .expect("vault is readable")
        .into_iter()
        .map(|note| note.rel)
        .collect()
}

#[test]
fn the_corpus_comes_from_notiz_kern_in_path_order() {
    let dir = TempDir::new();
    for rel in [
        "b.md",
        "a/z.md",
        "a.md",
        "90-secrets/s.md",
        "x/90-secrets/s.md",
        "_meta/m.md",
        ".h/n.md",
        "HOT.md",
        "MOC.md",
        "x/DECISIONS.md",
        "t.txt",
    ] {
        dir.write(rel, b"text\n");
    }
    assert_eq!(rels(&dir), ["MOC.md", "a/z.md", "a.md", "b.md"]);
}

#[test]
fn an_unlistable_vault_is_empty_and_an_unreadable_note_is_unreadable() {
    let dir = TempDir::new();
    assert!(
        load_search_notes(&dir.path().join("fehlt"))
            .expect("empty")
            .is_empty()
    );
    assert!(
        load_search_notes(&dir.write("datei.md", b"x"))
            .expect("empty")
            .is_empty()
    );
}

#[test]
fn originals_of_a_source_are_no_notes() {
    let dir = TempDir::new();
    dir.write("q/.space.yml", br#"{"art": "quellen"}"#);
    dir.write("q/quellen/orig.md", b"original\n");
    dir.write("q/quellen/leer.md", b"---\nquelle:\n---\n");
    dir.write("q/quellen/notiz.md", b"---\nquelle: orig.md\n---\n");
    dir.write("q/quellen/NICHT-LESBAR.md", b"x\n");
    dir.write("q/quellen/sub/NICHT-LESBAR.md", b"x\n");
    dir.write("q/anderes.md", b"x\n");
    dir.write("p/.space.yml", br#"{"art": "projekt"}"#);
    dir.write("p/quellen/o.md", b"x\n");
    dir.write("n/quellen/o.md", b"x\n");
    dir.write("k/.space.yml", b"{kaputt");
    dir.write("k/quellen/o.md", b"x\n");
    dir.write("l/.space.yml", br#"{"art": "quellen "}"#);
    dir.write("l/quellen/o.md", b"x\n");
    dir.write("b/.space.yml", b"\xef\xbb\xbf{\"art\": \"quellen\"}");
    dir.write("b/quellen/o.md", b"x\n");
    dir.write("a/.space.yml", br#"["art"]"#);
    dir.write("a/quellen/o.md", b"x\n");
    dir.write("g/.space.yml", &vec![b' '; 70_000]);
    dir.write("g/quellen/o.md", b"x\n");
    assert_eq!(
        rels(&dir),
        [
            "a/quellen/o.md",
            "g/quellen/o.md",
            "k/quellen/o.md",
            "l/quellen/o.md",
            "n/quellen/o.md",
            "p/quellen/o.md",
            "q/anderes.md",
            "q/quellen/NICHT-LESBAR.md",
            "q/quellen/notiz.md"
        ]
    );
    let root = TempDir::new();
    root.write(".space.yml", br#"{"art": "quellen"}"#);
    root.write("quellen/x.md", b"x\n");
    root.write("quellen/y.md", b"---\nquelle: [a]\n---\n");
    root.write("rest.md", b"x\n");
    assert_eq!(rels(&root), ["quellen/y.md", "rest.md"]);
}

#[test]
fn a_nested_quellen_folder_with_another_kind_is_looked_at_in_turn() {
    let dir = TempDir::new();
    dir.write("a/.space.yml", br#"{"art": "projekt"}"#);
    dir.write("a/quellen/b/.space.yml", br#"{"art": "quellen"}"#);
    dir.write("a/quellen/b/quellen/c.md", b"x\n");
    dir.write("a/quellen/b/d.md", b"x\n");
    dir.write("a/quellen/e.md", b"x\n");
    assert_eq!(rels(&dir), ["a/quellen/b/d.md", "a/quellen/e.md"]);
}

#[test]
fn the_space_of_a_path_is_the_longest_mount_prefix() {
    let dir = TempDir::new();
    dir.write(
        ".brain/spaces.lock",
        br#"{"spaces": [{"space": "a", "mount": "garten"}, {"space": "b", "mount": "garten/tief/"}, {"space": "c", "mount": "/20-projects/p"}, {"space": "leer", "mount": ""}, {"space": "ohne"}, {"mount": "kein-name"}, 5, {"space": "n", "mount": 7}, {"space": "liste", "mount": ["x"]}, {"space": 3, "mount": "zahl"}]}"#,
    );
    let manifest = Manifest::load(dir.path());
    assert_eq!(manifest.space_for("garten/x.md"), "a");
    assert_eq!(manifest.space_for("garten/tief/x.md"), "b");
    assert_eq!(manifest.space_for("garten"), "a");
    assert_eq!(manifest.space_for("garten2/x.md"), "kern");
    assert_eq!(manifest.space_for("20-projects/p/KERN.md"), "c");
    assert_eq!(manifest.space_for("/garten/x.md"), "a");
    assert_eq!(manifest.space_for("7/x.md"), "n");
    assert_eq!(manifest.space_for("andere/x.md"), "kern");
    assert_eq!(manifest.space_for("kein-name/x.md"), "kern");
    assert_eq!(manifest.space_for("zahl/x.md"), "kern");
    assert_eq!(manifest.space_for("x/y.md"), "kern");
}

#[test]
fn an_unreadable_manifest_is_an_empty_one() {
    for text in [
        &b"{kaputt"[..],
        b"[]",
        br#"{"spaces": 5}"#,
        br#"{"spaces": {"a": 1}}"#,
        b"\xff\xfe",
        b"",
    ] {
        let dir = TempDir::new();
        dir.write(".brain/spaces.lock", text);
        assert_eq!(Manifest::load(dir.path()).space_for("garten/x.md"), "kern");
    }
    assert_eq!(
        Manifest::load(TempDir::new().path()).space_for("a/b.md"),
        "kern"
    );
    let first = TempDir::new();
    first.write(
        ".brain/spaces.lock",
        br#"{"spaces": [{"space": "eins", "mount": "a"}, {"space": "zwei", "mount": "b"}]}"#,
    );
    assert_eq!(Manifest::load(first.path()).space_for("b/x.md"), "zwei");
    let tie = TempDir::new();
    tie.write(
        ".brain/spaces.lock",
        br#"{"spaces": [{"space": "eins", "mount": "a"}, {"space": "zwei", "mount": "a/"}]}"#,
    );
    assert_eq!(Manifest::load(tie.path()).space_for("a/x.md"), "eins");
}

#[test]
fn scalars_of_a_manifest_are_written_as_python_writes_them() {
    use serde_json::json;
    assert_eq!(scalar_text(&json!("a")).as_deref(), Some("a"));
    assert_eq!(scalar_text(&json!(null)).as_deref(), Some("None"));
    assert_eq!(scalar_text(&json!(true)).as_deref(), Some("True"));
    assert_eq!(scalar_text(&json!(false)).as_deref(), Some("False"));
    assert_eq!(scalar_text(&json!(7)).as_deref(), Some("7"));
    assert_eq!(scalar_text(&json!([1])), None);
    assert_eq!(scalar_text(&json!({})), None);
}

fn flagged(text: &[u8]) -> Vec<String> {
    let dir = TempDir::new();
    dir.write("_meta/state/contradictions.json", text);
    let mut rels: Vec<String> = contradicted_rels(dir.path()).into_iter().collect();
    rels.sort();
    rels
}

#[test]
fn only_open_and_escalated_findings_flag_their_two_notes() {
    let text = br#"{"findings": {
        "1": {"status": "open", "note_a": {"rel": "a.md"}, "note_b": {"rel": "b.md"}},
        "2": {"status": "escalated", "note_a": {"rel": "c.md"}},
        "3": {"status": "closed", "note_a": {"rel": "d.md"}, "note_b": {"rel": "e.md"}},
        "4": {"status": "open", "note_a": "text", "note_b": {"rel": ""}},
        "5": {"note_a": {"rel": "f.md"}},
        "6": 5,
        "7": {"status": "open", "note_b": {"rel": "g.md"}}
    }}"#;
    assert_eq!(flagged(text), ["a.md", "b.md", "c.md", "g.md"]);
}

#[test]
fn a_broken_findings_file_flags_nothing() {
    for text in [
        &b"{kaputt"[..],
        b"[]",
        br#"{"findings": []}"#,
        br#"{"findings": 5}"#,
        b"{}",
        b"\xff",
        b"",
    ] {
        assert!(flagged(text).is_empty());
    }
    assert!(contradicted_rels(TempDir::new().path()).is_empty());
}

type Loaded = (Vec<f64>, Vec<(String, Vec<f64>)>);

fn vectors(text: &[u8]) -> Option<Loaded> {
    let dir = TempDir::new();
    let path = dir.write("v.json", text);
    load_vectors(&path).ok().map(|v| (v.query, v.notes))
}

#[test]
fn the_vector_file_keeps_the_order_of_the_notes_and_the_last_value_of_a_repeated_path() {
    let (query, notes) = vectors(br#"{"query": [1, 2.5], "notes": {"b.md": [1], "a.md": [2, 3], "b.md": [9], "c.md": []}, "extra": {"x": [true]}}"#).expect("valid");
    assert_eq!(query, [1.0, 2.5]);
    assert_eq!(
        notes,
        [
            ("b.md".to_string(), vec![9.0]),
            ("a.md".to_string(), vec![2.0, 3.0]),
            ("c.md".to_string(), vec![])
        ]
    );
    let (_, none) = vectors(br#"{"query": [], "notes": {}}"#).expect("valid");
    assert!(none.is_empty());
}

#[test]
fn a_vector_file_of_the_wrong_form_is_unreadable() {
    for text in [
        &br#"{"query": [1], "notes": {"a": [true]}}"#[..],
        br#"{"query": [null], "notes": {}}"#,
        br#"{"query": ["1"], "notes": {}}"#,
        br#"{"query": [1], "notes": []}"#,
        br#"{"query": 1, "notes": {}}"#,
        br#"{"notes": {}}"#,
        br#"{"query": [1]}"#,
        b"[1]",
        br#"{"query": [1], "notes": {"a": 1}}"#,
        br#"{"query": [1], "notes": {}} x"#,
        b"\xef\xbb\xbf{\"query\": [1], \"notes\": {}}",
        b"{kaputt",
        b"",
    ] {
        assert!(vectors(text).is_none(), "{}", String::from_utf8_lossy(text));
    }
    assert!(load_vectors(Path::new("/nicht/da/v.json")).is_err());
}

#[test]
fn the_visitors_say_what_they_expected() {
    use crate::vault::Vectors;
    let message = |text: &str| {
        serde_json::from_str::<Vectors>(text)
            .expect_err("invalid")
            .to_string()
    };
    assert!(
        message("5").contains("an object with `query` and `notes`"),
        "{}",
        message("5")
    );
    assert!(message(r#"{"query": [true], "notes": {}}"#).contains("a number"));
    assert!(message(r#"{"query": [], "notes": []}"#).contains("an object of lists of numbers"));
    assert!(message(r#"{"query": []}"#).contains("`query` and `notes` are required"));
}

#[test]
fn the_manifest_of_a_space_is_read_up_to_64_kib() {
    let sized = |size: usize| -> bool {
        let dir = TempDir::new();
        let mut text = br#"{"art": "quellen"}"#.to_vec();
        text.resize(size, b' ');
        dir.write("s/.space.yml", &text);
        dir.write("s/quellen/o.md", b"x\n");
        rels(&dir).is_empty()
    };
    assert!(sized(5_000));
    assert!(sized(65_535));
    assert!(sized(65_536));
    assert!(!sized(65_537));
}

#[test]
fn note_vectors_are_the_part_notes_and_the_key_query_is_not_used() {
    let notes = |text: &str| parse_note_vectors(text.as_bytes()).ok();
    assert_eq!(
        notes(r#"{"notes": {"b.md": [1], "a.md": [2, 3], "b.md": [9], "c.md": []}}"#),
        Some(vec![
            ("b.md".to_string(), vec![9.0]),
            ("a.md".to_string(), vec![2.0, 3.0]),
            ("c.md".to_string(), vec![]),
        ])
    );
    assert_eq!(notes(r#"{"notes": {}}"#), Some(vec![]));
    for with_query in [
        r#"{"query": [1, 2], "notes": {"a.md": [1]}}"#,
        r#"{"query": "kein Vektor", "notes": {"a.md": [1]}}"#,
        r#"{"notes": {"a.md": [1]}, "query": null, "query": {"x": [true]}}"#,
        r#"{"extra": 1, "notes": {"a.md": [1]}}"#,
    ] {
        assert_eq!(
            notes(with_query),
            Some(vec![("a.md".to_string(), vec![1.0])]),
            "{with_query}"
        );
    }
    assert_eq!(
        notes(r#"{"notes": {"a.md": [1]}, "notes": {"b.md": [2]}}"#),
        Some(vec![("b.md".to_string(), vec![2.0])])
    );
}

#[test]
fn note_vectors_of_the_wrong_form_are_unreadable() {
    for text in [
        &br#"{"query": [1]}"#[..],
        br"{}",
        br#"{"notes": []}"#,
        br#"{"notes": {"a": [true]}}"#,
        br#"{"notes": {"a": [null]}}"#,
        br#"{"notes": {"a": ["1"]}}"#,
        br#"{"notes": {"a": 1}}"#,
        br#"{"notes": {"a": [NaN]}}"#,
        br#"{"notes": {"a": [1e999]}}"#,
        br#"{"notes": {}} x"#,
        b"[1]",
        b"5",
        b"\xff",
        b"\xef\xbb\xbf{\"notes\": {}}",
        b"{kaputt",
        b"",
    ] {
        assert!(
            parse_note_vectors(text).is_err(),
            "{}",
            String::from_utf8_lossy(text)
        );
    }
}

#[test]
fn the_note_vector_visitor_says_what_it_expected() {
    let message = |text: &str| {
        serde_json::from_str::<NoteVectors>(text)
            .expect_err("invalid")
            .to_string()
    };
    assert!(
        message("5").contains("an object with `notes`"),
        "{}",
        message("5")
    );
    assert!(message(r#"{"query": []}"#).contains("`notes` is required"));
}

#[test]
fn the_borrowed_form_of_the_vectors_shows_the_same_numbers() {
    let vectors = Vectors {
        query: vec![1.0, 2.0],
        notes: vec![("a.md".to_string(), vec![3.0])],
    };
    let view = vectors.view();
    assert_eq!(view.query, [1.0, 2.0]);
    assert_eq!(view.notes, [("a.md".to_string(), vec![3.0])]);
}

#[test]
fn one_note_is_read_by_its_path_and_the_corpus_paths_are_listed() {
    let dir = TempDir::new();
    dir.write("a/x.md", b"---\ntitle: Eins\n---\ntext\r\nmehr\n");
    dir.write("b.md", b"zwei\n");
    dir.write("90-secrets/s.md", b"geheim\n");
    let note = read_note(dir.path(), "a/x.md").expect("readable");
    assert_eq!((note.rel.as_str(), note.title.as_str()), ("a/x.md", "Eins"));
    assert!(note.text.ends_with("text\nmehr\n"));
    assert!(read_note(dir.path(), "fehlt.md").is_err());
    assert!(read_note(dir.path(), "a").is_err());
    assert_eq!(corpus_rels(dir.path()), ["a/x.md", "b.md"]);
    assert!(corpus_rels(&dir.path().join("fehlt")).is_empty());
}

/// The bits of `ROUNDING_CASES`, in the order of the cases.
fn rounding_bits() -> Vec<u64> {
    ROUNDING_CASES.iter().map(|&(_, bits)| bits).collect()
}

/// The texts of `ROUNDING_CASES` as the text of one JSON list.
fn rounding_list() -> String {
    let texts: Vec<&str> = ROUNDING_CASES.iter().map(|&(text, _)| text).collect();
    format!("[{}]", texts.join(", "))
}

fn bits_of(numbers: &[f64]) -> Vec<u64> {
    numbers.iter().map(|number| number.to_bits()).collect()
}

#[test]
fn a_vector_is_read_bit_for_bit_as_python_reads_it() {
    // The feature `float_roundtrip` of `serde_json`: without it each of these texts is read one ULP away.
    // The comparison is on the bits, no tolerance would show the difference.
    let list = rounding_list();
    let vector = parse_vector(list.as_bytes()).expect("a list of numbers");
    assert_eq!(bits_of(&vector), rounding_bits());
    for (text, bits) in ROUNDING_CASES {
        let single = parse_vector(format!("[{text}]").as_bytes()).expect("a list of numbers");
        assert_eq!(bits_of(&single), [bits], "{text}");
    }
}

#[test]
fn the_files_of_vectors_are_read_bit_for_bit_as_python_reads_them() {
    let list = rounding_list();
    let dir = TempDir::new();
    let query_file = dir.write("query.json", list.as_bytes());
    let query = load_query_vector(&query_file).expect("a query vector");
    assert_eq!(bits_of(&query), rounding_bits());
    let file = format!(r#"{{"query": {list}, "notes": {{"a.md": {list}}}}}"#);
    let loaded = load_vectors(&dir.write("v.json", file.as_bytes())).expect("a vector file");
    assert_eq!(bits_of(&loaded.query), rounding_bits());
    assert_eq!(bits_of(&loaded.notes[0].1), rounding_bits());
    let notes = parse_note_vectors(file.as_bytes()).expect("note vectors");
    assert_eq!(bits_of(&notes[0].1), rounding_bits());
}
