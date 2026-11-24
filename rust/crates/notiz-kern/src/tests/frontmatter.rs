//! Tests of the frontmatter reader.

use crate::frontmatter::{
    Field, cut_comment, is_blank, key_match, parse_fields, scalar, split_blocks,
};
use crate::text::is_space;

fn text(value: &str) -> Field {
    Field::Text(value.to_string())
}

fn list(values: &[&str]) -> Field {
    Field::List(values.iter().map(ToString::to_string).collect())
}

#[test]
fn non_empty_text_returns_only_non_empty_scalars() {
    assert_eq!(text("a").non_empty_text(), Some("a"));
    assert_eq!(text("").non_empty_text(), None);
    assert_eq!(list(&["a"]).non_empty_text(), None);
    assert_eq!(list(&[]).non_empty_text(), None);
}

#[test]
fn is_blank_is_space_or_tab_only() {
    assert!(is_blank(' '));
    assert!(is_blank('\t'));
    assert!(!is_blank('\n'));
    assert!(!is_blank('\u{a0}'));
    assert!(!is_blank('a'));
}

#[test]
fn key_match_splits_key_and_value() {
    assert_eq!(key_match("a-b_1: x", is_space), Some(("a-b_1", "x")));
    assert_eq!(key_match("a:   x  ", is_space), Some(("a", "x  ")));
    assert_eq!(key_match("a:", is_space), Some(("a", "")));
    assert_eq!(key_match("A9:x", is_space), Some(("A9", "x")));
    assert_eq!(key_match("a:\u{a0}x", is_space), Some(("a", "x")));
    assert_eq!(key_match("a:\u{a0}x", is_blank), Some(("a", "\u{a0}x")));
    assert_eq!(key_match("a:\t x", is_blank), Some(("a", "x")));
    assert_eq!(key_match("a: b: c", is_space), Some(("a", "b: c")));
}

#[test]
fn key_match_rejects_everything_but_a_plain_key_and_colon() {
    for line in [
        ":x",
        "a b: x",
        "a",
        "",
        "\u{e4}: 1",
        "a :x",
        " a: 1",
        "a.b: 1",
        "-: x:",
    ] {
        let found = key_match(line, is_space);
        if line == "-: x:" {
            assert_eq!(found, Some(("-", "x:")));
        } else {
            assert_eq!(found, None, "{line:?}");
        }
    }
}

#[test]
fn cut_comment_needs_a_whitespace_run_of_the_minimum_length() {
    assert_eq!(cut_comment("a  # c", 2), "a");
    assert_eq!(cut_comment("a # c", 2), "a # c");
    assert_eq!(cut_comment("a # c", 1), "a");
    assert_eq!(cut_comment("a \u{a0}#c", 2), "a");
    assert_eq!(cut_comment("a  b  # c", 2), "a  b");
    assert_eq!(cut_comment("a  b # c", 2), "a  b # c");
    assert_eq!(cut_comment("a   x#", 2), "a   x#");
    assert_eq!(cut_comment("#x", 2), "#x");
    assert_eq!(cut_comment("#x", 1), "#x");
    assert_eq!(cut_comment("  #x", 2), "");
    assert_eq!(cut_comment("a\t #", 2), "a");
    assert_eq!(cut_comment("a   ", 2), "a   ");
    assert_eq!(cut_comment("", 1), "");
    assert_eq!(cut_comment("\u{e4}  #", 2), "\u{e4}");
}

#[test]
fn scalar_removes_quotes_and_resolves_double_quote_escapes() {
    assert_eq!(scalar(r#""a\nb\tc\rd\"e\\f\qg""#), "a\nb\tc\rd\"e\\f\\qg");
    assert_eq!(scalar(r#""a\""#), "a\\");
    assert_eq!(scalar(r#""\n""#), "\n");
    assert_eq!(scalar(r#""\t""#), "\t");
    assert_eq!(scalar(r#""\r""#), "\r");
    assert_eq!(scalar(r#""\"""#), "\"");
    assert_eq!(scalar(r#""\\""#), "\\");
    assert_eq!(scalar("'it''s'"), "it's");
    assert_eq!(scalar(r"'a\n'"), "a\\n");
    assert_eq!(scalar("' x '"), " x ");
    assert_eq!(scalar("  x  "), "x");
    assert_eq!(scalar("\"\""), "");
    assert_eq!(scalar("''"), "");
    assert_eq!(scalar("\""), "");
    assert_eq!(scalar("'"), "");
    assert_eq!(scalar("\"a"), "a");
    assert_eq!(scalar("a\""), "a");
    assert_eq!(scalar("\"a'"), "a");
    assert_eq!(scalar("'a\""), "a");
    assert_eq!(scalar("\" a \" "), " a ");
    assert_eq!(scalar("x\"y"), "x\"y");
}

#[test]
fn split_blocks_finds_one_block_and_the_body() {
    let (blocks, body) = split_blocks("---\na: 1\n---\nbody", false);
    assert_eq!(blocks, ["a: 1"]);
    assert_eq!(body, "body");
}

#[test]
fn split_blocks_accepts_trailing_blanks_on_both_delimiters() {
    let (blocks, body) = split_blocks("--- \t\na: 1\n---  \nbody", false);
    assert_eq!(blocks, ["a: 1"]);
    assert_eq!(body, "body");
}

#[test]
fn split_blocks_closing_delimiter_may_end_the_text() {
    assert_eq!(split_blocks("---\na: 1\n---", false), (vec!["a: 1"], ""));
    assert_eq!(split_blocks("---\na: 1\n--- ", false), (vec!["a: 1"], ""));
    assert_eq!(split_blocks("---\na: 1\n---\n", false), (vec!["a: 1"], ""));
}

#[test]
fn split_blocks_keeps_what_follows_the_closing_line() {
    let (blocks, body) = split_blocks("---\na: 1\n---\n\nbody", false);
    assert_eq!(blocks, ["a: 1"]);
    assert_eq!(body, "\nbody");
}

#[test]
fn split_blocks_rejects_malformed_delimiters() {
    for text in [
        "---\na: 1\n",
        "--- x\na: 1\n---\n",
        "---a\na: 1\n---\n",
        "---\na: 1\n---x\n",
        "---\na: 1\n----\n",
        "-- -\na: 1\n---\n",
        " ---\na: 1\n---\n",
        "---\n---\n",
        "---",
        "",
    ] {
        let (blocks, body) = split_blocks(text, false);
        assert!(blocks.is_empty(), "{text:?}");
        assert_eq!(body, text);
    }
}

#[test]
fn split_blocks_closer_inside_a_line_does_not_close() {
    let (blocks, body) = split_blocks("---\na: 1\n--- x\nb: 2\n---\nrest", false);
    assert_eq!(blocks, ["a: 1\n--- x\nb: 2"]);
    assert_eq!(body, "rest");
}

#[test]
fn split_blocks_reads_several_blocks_and_skips_only_newlines_between_them() {
    let (blocks, body) = split_blocks("---\na: 1\n---\n\n\n---\nb: 2\n---\nbody", false);
    assert_eq!(blocks, ["a: 1", "b: 2"]);
    assert_eq!(body, "body");
    let (blocks, body) = split_blocks("---\na: 1\n---\n \n---\nb: 2\n---\n", false);
    assert_eq!(blocks, ["a: 1"]);
    assert_eq!(body, " \n---\nb: 2\n---\n");
}

#[test]
fn split_blocks_skips_leading_newlines_but_keeps_them_in_a_body() {
    let (blocks, body) = split_blocks("\n\n---\na: 1\n---\nx", false);
    assert_eq!(blocks, ["a: 1"]);
    assert_eq!(body, "x");
    let (blocks, body) = split_blocks("\n\nabc", false);
    assert_eq!(blocks.len(), 0);
    assert_eq!(body, "\n\nabc");
}

#[test]
fn split_blocks_requires_a_key_value_line_first() {
    let (blocks, _) = split_blocks("---\n# note\n\na: 1\n---\n", false);
    assert_eq!(blocks, ["# note\n\na: 1"]);
    for text in [
        "---\nplain text\n---\nbody",
        "---\n\n---\nbody",
        "---\n  \n# x\n---\nbody",
        "---\n a: 1\n---\nbody",
        "---\nx y: 1\na: 1\n---\nbody",
    ] {
        let (blocks, body) = split_blocks(text, false);
        assert!(blocks.is_empty(), "{text:?}");
        assert_eq!(body, text);
    }
}

#[test]
fn split_blocks_drops_leading_boms_only_when_asked() {
    let text = "\u{feff}\u{feff}---\na: 1\n---\nbody";
    assert_eq!(split_blocks(text, true), (vec!["a: 1"], "body"));
    assert_eq!(split_blocks(text, false), (Vec::<&str>::new(), text));
    assert_eq!(
        split_blocks("---\na: 1\n---\n\u{feff}---\nb: 2\n---\n", true).0,
        ["a: 1"]
    );
}

fn fields_of(block: &str) -> Vec<(String, Field)> {
    parse_fields(&[block]).into_iter().collect()
}

#[test]
fn parse_fields_reads_scalars_and_cuts_comments() {
    let fields = parse_fields(&[
        "title: Hello\nname: \"x # y\"  # c\nurl: http://x\nplain: a # b\nq: 'it''s'",
    ]);
    assert_eq!(fields["title"], text("Hello"));
    assert_eq!(fields["name"], text("x # y"));
    assert_eq!(fields["url"], text("http://x"));
    assert_eq!(fields["plain"], text("a # b"));
    assert_eq!(fields["q"], text("it's"));
    assert_eq!(fields.len(), 5);
}

#[test]
fn parse_fields_a_comment_directly_after_the_colon_is_a_value() {
    let fields = parse_fields(&["key:  # c\nother: x   # c"]);
    assert_eq!(fields["key"], text("# c"));
    assert_eq!(fields["other"], text("x"));
}

#[test]
fn parse_fields_reads_flow_lists() {
    let fields = parse_fields(&[
        "a: [x, y]\nb: []\nc: [ , a,, b ]\nd: [\"a,b\"]\ne: [a\nf: a]\ng: ['p', \"q\"]\nh: [x]  # c",
    ]);
    assert_eq!(fields["a"], list(&["x", "y"]));
    assert_eq!(fields["b"], list(&[]));
    assert_eq!(fields["c"], list(&["a", "b"]));
    assert_eq!(fields["d"], list(&["a", "b"]));
    assert_eq!(fields["e"], text("[a"));
    assert_eq!(fields["f"], text("a]"));
    assert_eq!(fields["g"], list(&["p", "q"]));
    assert_eq!(fields["h"], list(&["x"]));
}

#[test]
fn parse_fields_an_empty_value_is_an_empty_list() {
    let fields = parse_fields(&["a:\nb:   \nc: # x"]);
    assert_eq!(fields["a"], list(&[]));
    assert_eq!(fields["b"], list(&[]));
    assert_eq!(fields["c"], text("# x"));
}

#[test]
fn parse_fields_reads_block_lists() {
    let fields = parse_fields(&["tags:\n  - a  # c\n  - 'b'\n- c\n\t- \"d\""]);
    assert_eq!(fields["tags"], list(&["a", "b", "c", "d"]));
}

#[test]
fn parse_fields_a_dash_item_replaces_a_scalar_by_a_list() {
    let fields = parse_fields(&["title: A\n- x\n- y"]);
    assert_eq!(fields["title"], list(&["x", "y"]));
}

#[test]
fn parse_fields_joins_continuation_lines_with_a_space() {
    let fields = parse_fields(&["title: Long\n  wrapped  # cm\n\tmore\n-x\nnext: y"]);
    assert_eq!(fields["title"], text("Long wrapped more -x"));
    assert_eq!(fields["next"], text("y"));
}

#[test]
fn parse_fields_continuation_of_an_empty_scalar_is_trimmed() {
    let fields = parse_fields(&["title: ''\n  more"]);
    assert_eq!(fields["title"], text("more"));
}

#[test]
fn parse_fields_continuation_lines_do_not_extend_lists() {
    let fields = parse_fields(&["tags: [a]\n  more\nempty:\n  text"]);
    assert_eq!(fields["tags"], list(&["a"]));
    assert_eq!(fields["empty"], list(&[]));
}

#[test]
fn parse_fields_skips_blank_and_comment_lines() {
    let fields = parse_fields(&["# c\n\na: 1\n  # inner\n\n  \nb: 2"]);
    assert_eq!(fields["a"], text("1"));
    assert_eq!(fields["b"], text("2"));
    assert_eq!(fields.len(), 2);
}

#[test]
fn parse_fields_an_unmatched_line_ends_the_current_key() {
    let fields = parse_fields(&["a: 1\n??? x\n  - y\n  more"]);
    assert_eq!(fields["a"], text("1"));
    assert_eq!(fields.len(), 1);
    assert_eq!(fields_of("\u{e4}: 1\nb: 2").len(), 1);
}

#[test]
fn parse_fields_later_lines_and_blocks_win() {
    let fields = parse_fields(&["a: 1\na: 2\nb: x", "a: 3\nc: [z]"]);
    assert_eq!(fields["a"], text("3"));
    assert_eq!(fields["b"], text("x"));
    assert_eq!(fields["c"], list(&["z"]));
}

#[test]
fn parse_fields_a_new_block_starts_without_a_current_key() {
    let fields = parse_fields(&["a: 1", "  more\nb: 2"]);
    assert_eq!(fields["a"], text("1"));
    assert_eq!(fields["b"], text("2"));
}

#[test]
fn parse_fields_keeps_values_as_text() {
    let fields = parse_fields(&["a: yes\nb: 0755\nc: ~\nd: 2026-09-28\ne: null"]);
    assert_eq!(fields["a"], text("yes"));
    assert_eq!(fields["b"], text("0755"));
    assert_eq!(fields["c"], text("~"));
    assert_eq!(fields["d"], text("2026-09-28"));
    assert_eq!(fields["e"], text("null"));
}
