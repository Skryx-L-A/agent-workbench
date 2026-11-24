//! Tests of the section reader.

use crate::section::section;

const TEXT: &str = "# Title\nintro\n## A\na1\n### A1\nx\n## B\nb1\n";

fn find(text: &str, heading: &str) -> Option<String> {
    section(text, heading)
}

#[test]
fn a_section_runs_to_the_next_heading_of_the_same_or_a_higher_level() {
    assert_eq!(find(TEXT, "A").as_deref(), Some("## A\na1\n### A1\nx"));
    assert_eq!(find(TEXT, "A1").as_deref(), Some("### A1\nx"));
    assert_eq!(find(TEXT, "B").as_deref(), Some("## B\nb1"));
    assert_eq!(find(TEXT, "Title").as_deref(), Some(TEXT.trim_end()));
    assert_eq!(find("## A\nx\n# C\ny", "A").as_deref(), Some("## A\nx"));
    assert_eq!(find("## A\nx\n## B\ny", "A").as_deref(), Some("## A\nx"));
    assert_eq!(
        find("## A\nx\n### B\ny", "A").as_deref(),
        Some("## A\nx\n### B\ny")
    );
}

#[test]
fn the_requested_heading_ignores_hashes_case_and_outer_whitespace() {
    for requested in ["A", "a", "## A", "#A", " ## a ", "###   A  ", "\u{a0}A"] {
        assert_eq!(
            find(TEXT, requested).as_deref(),
            Some("## A\na1\n### A1\nx"),
            "{requested:?}"
        );
    }
}

#[test]
fn the_heading_line_ignores_indentation_and_closing_hashes() {
    assert_eq!(find("  ## A ##\nx\n", "A").as_deref(), Some("  ## A ##\nx"));
    assert_eq!(find("\u{a0}## A\nx", "A").as_deref(), Some("\u{a0}## A\nx"));
    assert_eq!(find("## A #x\ny", "A"), None);
    assert_eq!(find("## A###\ny", "a").as_deref(), Some("## A###\ny"));
}

#[test]
fn trailing_whitespace_of_the_section_is_removed() {
    assert_eq!(find("## A\nx\n\n\n## B", "A").as_deref(), Some("## A\nx"));
    assert_eq!(find("## A\nx  \n \n", "A").as_deref(), Some("## A\nx"));
    assert_eq!(find("## A", "A").as_deref(), Some("## A"));
}

#[test]
fn the_first_matching_heading_wins() {
    assert_eq!(find("## A\n1\n## A\n2", "A").as_deref(), Some("## A\n1"));
}

#[test]
fn a_hash_without_a_space_is_a_heading() {
    assert_eq!(find("#tag\nx", "tag").as_deref(), Some("#tag\nx"));
    assert_eq!(find("text #tag\nx", "tag"), None);
}

#[test]
fn a_missing_heading_is_none() {
    assert_eq!(find(TEXT, "C"), None);
    assert_eq!(find("", "A"), None);
    assert_eq!(find("no headings", "no headings"), None);
    assert_eq!(find("## A B\nx", "A"), None);
}

#[test]
fn an_empty_request_matches_an_empty_heading() {
    assert_eq!(
        find("# \nbody\n## x", "").as_deref(),
        Some("# \nbody\n## x")
    );
    assert_eq!(find("# x\n#\nbody", "#").as_deref(), Some("#\nbody"));
    assert_eq!(find("# x", ""), None);
}

#[test]
fn a_section_ends_at_a_line_break_character_of_python() {
    assert_eq!(
        find("## A\u{2028}x\u{2028}# B\u{2028}y", "A").as_deref(),
        Some("## A\u{2028}x")
    );
}
