//! Tests of the Python-compatible text helpers.

use super::TempDir;
use crate::text::{
    exclusion_key, has_md_suffix, is_space, key_of, read_text, split_keep, split_lines, strip,
};

#[test]
fn is_space_matches_python_isspace() {
    for c in [
        ' ', '\t', '\n', '\r', '\u{b}', '\u{c}', '\u{1c}', '\u{1d}', '\u{1e}', '\u{1f}', '\u{85}',
        '\u{a0}', '\u{2028}', '\u{2029}', '\u{3000}',
    ] {
        assert!(is_space(c), "{c:?} is whitespace");
    }
    for c in [
        'a', '0', '#', '-', '\u{0}', '\u{1b}', '\u{200b}', '\u{feff}',
    ] {
        assert!(!is_space(c), "{c:?} is not whitespace");
    }
}

#[test]
fn strip_removes_python_whitespace_at_both_ends_only() {
    assert_eq!(strip("\u{1c} a b \u{a0}\n"), "a b");
    assert_eq!(strip("a  b"), "a  b");
    assert_eq!(strip("   "), "");
    assert_eq!(strip(""), "");
    assert_eq!(strip("\u{feff}a"), "\u{feff}a");
}

#[test]
fn md_suffix_is_exact_and_case_sensitive() {
    assert!(has_md_suffix("a.md"));
    assert!(has_md_suffix(".md"));
    assert!(!has_md_suffix("a.MD"));
    assert!(!has_md_suffix("md"));
    assert!(!has_md_suffix("a.mdx"));
    assert!(!has_md_suffix(""));
}

#[test]
fn split_keep_keeps_line_endings() {
    assert_eq!(split_keep(""), Vec::<&str>::new());
    assert_eq!(split_keep("a"), ["a"]);
    assert_eq!(split_keep("a\n"), ["a\n"]);
    assert_eq!(split_keep("a\nb"), ["a\n", "b"]);
    assert_eq!(split_keep("a\n\nb"), ["a\n", "\n", "b"]);
    assert_eq!(split_keep("a\r\nb"), ["a\r\n", "b"]);
    assert_eq!(split_keep("a\r\r\nb"), ["a\r", "\r\n", "b"]);
    assert_eq!(split_keep("a\rb"), ["a\r", "b"]);
    assert_eq!(split_keep("a\r"), ["a\r"]);
    assert_eq!(split_keep("\n\r\n"), ["\n", "\r\n"]);
    assert_eq!(
        split_keep("a\u{b}b\u{c}c\u{1c}d\u{1d}e\u{1e}f\u{85}g\u{2028}h\u{2029}i"),
        [
            "a\u{b}",
            "b\u{c}",
            "c\u{1c}",
            "d\u{1d}",
            "e\u{1e}",
            "f\u{85}",
            "g\u{2028}",
            "h\u{2029}",
            "i"
        ]
    );
    assert_eq!(split_keep("a\u{a0}b\u{1f}c"), ["a\u{a0}b\u{1f}c"]);
}

#[test]
fn split_lines_drops_line_endings() {
    assert_eq!(split_lines(""), Vec::<&str>::new());
    assert_eq!(split_lines("a\r\nb\rc\nd"), ["a", "b", "c", "d"]);
    assert_eq!(split_lines("a\n"), ["a"]);
    assert_eq!(split_lines("\n\n"), ["", ""]);
    assert_eq!(split_lines("a\u{2028}b"), ["a", "b"]);
    assert_eq!(split_lines("a\u{a0}b"), ["a\u{a0}b"]);
}

#[test]
fn read_text_applies_universal_newlines_and_replaces_bad_bytes() {
    let dir = TempDir::new();
    let path = dir.write("t.md", b"a\r\nb\rc\nd\xffe\xef\xbb\xbf");
    assert_eq!(
        read_text(&path).expect("file is readable"),
        "a\nb\nc\nd\u{fffd}e\u{feff}"
    );
}

#[test]
fn read_text_fails_for_missing_files_and_directories() {
    let dir = TempDir::new();
    assert!(read_text(&dir.path().join("missing.md")).is_err());
    assert!(read_text(dir.path()).is_err());
}

#[test]
fn key_of_normalises_strips_and_lowers() {
    assert_eq!(key_of("  Ha\u{308}rtung \n"), "h\u{e4}rtung");
    assert_eq!(key_of("\u{c4}B"), "\u{e4}b");
    assert_eq!(key_of("A\u{a0}"), "a");
    assert_eq!(key_of(""), "");
    assert_eq!(key_of("Straße"), "straße");
}

#[test]
fn exclusion_key_folds_like_python_casefold_and_beyond() {
    // Pinned against `unicodedata.normalize("NFKC", normalize("NFKC", s).casefold())`.
    for (given, key) in [
        ("90-SECRETS", "90-secrets"),
        ("90-\u{17f}ecrets", "90-secrets"),
        ("\u{1e9e}", "ss"),
        ("\u{fb01}", "fi"),
        ("\u{212a}elvin", "kelvin"),
        ("\u{ff28}\u{ff2f}\u{ff34}.md", "hot.md"),
        ("\u{ff10}", "0"),
        ("\u{ff0e}git", ".git"),
        ("HOT.md", "hot.md"),
        ("", ""),
    ] {
        assert_eq!(exclusion_key(given), key, "{given:?}");
    }
}

#[test]
fn exclusion_key_joins_decomposed_and_composed_spellings() {
    assert_eq!(exclusion_key("A\u{308}rger"), "\u{e4}rger");
    assert_eq!(exclusion_key("\u{c4}rger"), "\u{e4}rger");
    assert_eq!(exclusion_key("a\u{308}"), exclusion_key("\u{c4}"));
    assert_eq!(exclusion_key("H\u{d3}T"), "h\u{f3}t");
    // `İ` folds to `i` plus a combining dot, as in Python, so it does not become plain `i`.
    assert_eq!(exclusion_key("\u{130}"), "i\u{307}");
}
