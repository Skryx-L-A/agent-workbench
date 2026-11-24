//! Tests of the marker blocks.

use crate::blocks::{BlockState, block_of, block_state, replace_block, strip_blocks};

const S: &str = "<!-- s -->";
const E: &str = "<!-- e -->";

#[test]
fn state_names() {
    assert_eq!(BlockState::None.as_str(), "none");
    assert_eq!(BlockState::One.as_str(), "one");
    assert_eq!(BlockState::Malformed.as_str(), "malformed");
}

#[test]
fn state_none_one_malformed() {
    assert_eq!(block_state("plain", S, E), BlockState::None);
    assert_eq!(block_state("", S, E), BlockState::None);
    assert_eq!(
        block_state("a\n<!-- s -->\nx\n<!-- e -->\nb", S, E),
        BlockState::One
    );
    assert_eq!(block_state("<!-- s -->\n<!-- e -->", S, E), BlockState::One);
    for text in [
        "<!-- e -->\n<!-- s -->",
        "<!-- s -->\nx",
        "<!-- e -->\nx",
        "<!-- s -->\n<!-- s -->\n<!-- e -->",
        "<!-- s -->\n<!-- e -->\n<!-- e -->",
        "<!-- s -->\n<!-- e -->\n<!-- s -->\n<!-- e -->",
    ] {
        assert_eq!(block_state(text, S, E), BlockState::Malformed, "{text:?}");
    }
}

#[test]
fn one_marker_used_for_start_and_end_is_never_a_well_formed_pair() {
    let marker = "<!-- m -->";
    assert_eq!(
        block_state("x\n<!-- m -->\ny", marker, marker),
        BlockState::Malformed
    );
    assert_eq!(
        block_state("<!-- m -->\nx\n<!-- m -->", marker, marker),
        BlockState::Malformed
    );
}

#[test]
fn empty_markers_match_blank_lines_and_terminate() {
    assert_eq!(strip_blocks("a\n\n\nb", "", ""), "a\n\nb");
    assert_eq!(strip_blocks("a\n\nb", "", ""), "a\n\nb");
    assert_eq!(strip_blocks("", "", ""), "");
}

#[test]
fn a_marker_counts_only_alone_on_its_line() {
    assert_eq!(
        block_state("see <!-- s --> and <!-- e --> here", S, E),
        BlockState::None
    );
    assert_eq!(
        block_state("<!-- s --> x\n<!-- e -->", S, E),
        BlockState::Malformed
    );
    assert_eq!(
        block_state("  <!-- s -->\t\n\u{a0}<!-- e -->  ", S, E),
        BlockState::One
    );
    assert_eq!(
        block_state("<!-- S -->\n<!-- E -->", S, E),
        BlockState::None
    );
}

#[test]
fn block_of_includes_markers_but_not_the_final_newline() {
    assert_eq!(
        block_of("a\n<!-- s -->\nx\n<!-- e -->\nb", S, E).as_deref(),
        Some("<!-- s -->\nx\n<!-- e -->")
    );
    assert_eq!(
        block_of("a\n<!-- s -->\nx\n<!-- e -->", S, E).as_deref(),
        Some("<!-- s -->\nx\n<!-- e -->")
    );
    assert_eq!(
        block_of("<!-- s -->\n<!-- e -->\n", S, E).as_deref(),
        Some("<!-- s -->\n<!-- e -->")
    );
    assert_eq!(
        block_of("a\n  <!-- s -->  \nx\n<!-- e -->  \nb", S, E).as_deref(),
        Some("  <!-- s -->  \nx\n<!-- e -->  ")
    );
}

#[test]
fn block_of_is_none_without_a_well_formed_pair() {
    assert_eq!(block_of("plain", S, E), None);
    assert_eq!(block_of("<!-- s -->\nx", S, E), None);
    assert_eq!(block_of("<!-- e -->\n<!-- s -->", S, E), None);
}

#[test]
fn replace_block_swaps_the_block_and_keeps_the_rest() {
    assert_eq!(
        replace_block("a\n<!-- s -->\nx\n<!-- e -->\nb", S, E, "NEW").as_deref(),
        Some("a\nNEW\nb")
    );
    assert_eq!(
        replace_block("a\n<!-- s -->\nx\n<!-- e -->", S, E, "N\nEW").as_deref(),
        Some("a\nN\nEW")
    );
    assert_eq!(
        replace_block("<!-- s -->\n<!-- e -->\n", S, E, "").as_deref(),
        Some("\n")
    );
    assert_eq!(
        replace_block("  <!-- s -->\nx\n<!-- e -->  \n", S, E, "-").as_deref(),
        Some("-\n")
    );
}

#[test]
fn replace_block_without_markers_returns_the_text_and_refuses_malformed_ones() {
    assert_eq!(
        replace_block("plain", S, E, "NEW").as_deref(),
        Some("plain")
    );
    assert_eq!(replace_block("<!-- s -->\nx", S, E, "NEW"), None);
    assert_eq!(replace_block("<!-- e -->\n<!-- s -->", S, E, "NEW"), None);
}

#[test]
fn strip_blocks_removes_the_block_and_its_separating_blank_lines() {
    assert_eq!(
        strip_blocks("a\n\n<!-- s -->\nx\n<!-- e -->\n\nb", S, E),
        "a\n\nb"
    );
    assert_eq!(
        strip_blocks("a\n<!-- s -->\nx\n<!-- e -->\nb", S, E),
        "a\n\nb"
    );
    assert_eq!(strip_blocks("a\n<!-- s -->\nx\n<!-- e -->\n", S, E), "a\n");
    assert_eq!(strip_blocks("a\n<!-- s -->\nx\n<!-- e -->", S, E), "a\n");
    assert_eq!(strip_blocks("<!-- s -->\nx\n<!-- e -->\nb", S, E), "b");
    assert_eq!(
        strip_blocks("<!-- s -->\nx\n<!-- e -->\n\n\nb\n", S, E),
        "b\n"
    );
    assert_eq!(strip_blocks("<!-- s -->\nx\n<!-- e -->", S, E), "");
}

#[test]
fn strip_blocks_removes_every_block() {
    let text = "a\n<!-- s -->\n1\n<!-- e -->\nb\n<!-- s -->\n2\n<!-- e -->\nc";
    assert_eq!(strip_blocks(text, S, E), "a\n\nb\n\nc");
}

#[test]
fn strip_blocks_leaves_unbalanced_and_quoted_markers_alone() {
    for text in [
        "plain",
        "a\n<!-- s -->\nx",
        "<!-- e -->\n<!-- s -->",
        "see <!-- s --> and <!-- e -->",
        "a\n<!-- e -->\nb",
    ] {
        assert_eq!(strip_blocks(text, S, E), text, "{text:?}");
    }
}

#[test]
fn strip_blocks_pairs_a_start_with_the_next_end_after_it() {
    assert_eq!(
        strip_blocks("<!-- e -->\na\n<!-- s -->\nx\n<!-- e -->\nb", S, E),
        "<!-- e -->\na\n\nb"
    );
    assert_eq!(
        strip_blocks("<!-- s -->\n<!-- s -->\nx\n<!-- e -->\nb", S, E),
        "b"
    );
}
