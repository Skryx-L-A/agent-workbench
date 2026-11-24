use super::data;
use crate::snippet::{
    best_snippet_span, body_blocks, clip, file_lines, hook_snippet_span, sentences,
};
use crate::token::Analyzer;

fn span(value: &serde_json::Value) -> Option<(usize, usize)> {
    value.as_array().map(|pair| {
        (
            usize::try_from(pair[0].as_u64().expect("line")).expect("fits"),
            usize::try_from(pair[1].as_u64().expect("line")).expect("fits"),
        )
    })
}

#[test]
fn excerpts_and_hooks_agree_with_python_on_every_text() {
    let rows = data("snippets");
    let rows = rows.as_array().expect("rows");
    assert!(rows.len() > 200);
    let mut analyzer = Analyzer::new();
    for row in rows {
        let text = row["text"].as_str().expect("text");
        let query = row["query"].as_str().expect("query");
        let (snippet, lines) = best_snippet_span(&mut analyzer, text, query);
        assert_eq!(
            snippet,
            row["best"][0].as_str().expect("snippet"),
            "snippet of {text:?} for {query:?}"
        );
        assert_eq!(
            lines,
            span(&row["best"][1]),
            "lines of {text:?} for {query:?}"
        );
        let (hook, hook_lines) = hook_snippet_span(text);
        assert_eq!(
            hook,
            row["hook"][0].as_str().expect("hook"),
            "hook of {text:?}"
        );
        assert_eq!(hook_lines, span(&row["hook"][1]), "hook lines of {text:?}");
    }
}

#[test]
fn lines_are_split_at_newline_only() {
    assert_eq!(
        file_lines("a\u{2028}b\u{c}c\nd\r\ne\r"),
        ["a\u{2028}b\u{c}c", "d", "e"]
    );
    assert_eq!(file_lines(""), [""]);
}

#[test]
fn blocks_carry_their_heading_and_file_lines() {
    let blocks = body_blocks("---\ntitle: t\n---\n# Kopf\nA b\nc\n\n\n## Zwei\nd\n");
    assert_eq!(blocks.len(), 2);
    assert_eq!(
        (blocks[0].heading.as_str(), blocks[0].text.as_str()),
        ("Kopf", "A b c")
    );
    assert_eq!((blocks[0].first, blocks[0].last), (5, 6));
    assert_eq!(
        (blocks[1].heading.as_str(), blocks[1].text.as_str()),
        ("Zwei", "d")
    );
    assert_eq!((blocks[1].first, blocks[1].last), (10, 10));
    assert!(body_blocks("---\nopen\n").len() == 1);
    assert!(body_blocks("").is_empty());
}

#[test]
fn sentences_split_after_a_stop_and_white_space() {
    assert_eq!(sentences("A. B! C? D"), ["A.", "B!", "C?", "D"]);
    assert_eq!(sentences("A.\u{a0} B"), ["A.", "B"]);
    assert_eq!(sentences("A.B C"), ["A.B C"]);
    assert_eq!(sentences("a , b"), ["a , b"]);
    assert_eq!(sentences(".  "), ["."]);
    assert!(sentences("").is_empty());
}

#[test]
fn clip_cuts_at_a_word_boundary_after_220_code_points() {
    let short = "x".repeat(220);
    assert_eq!(clip(&short), short);
    let words = "ab ".repeat(100);
    let clipped = clip(words.trim_end());
    assert!(clipped.ends_with("ab..."));
    assert!(clipped.chars().count() <= 223);
    let unbroken = "x".repeat(300);
    assert_eq!(clip(&unbroken), format!("{}...", "x".repeat(220)));
    let comma = format!("{},;: {}", "y".repeat(215), "z".repeat(20));
    assert_eq!(clip(&comma), format!("{}...", "y".repeat(215)));
}
