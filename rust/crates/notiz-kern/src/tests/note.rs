//! Tests of note parsing, link extraction and the embedding text.

use std::collections::BTreeSet;

use crate::frontmatter::Field;
use crate::note::{cut_relations, parse_note, wikilinks};

fn set(items: &[&str]) -> BTreeSet<String> {
    items.iter().map(ToString::to_string).collect()
}

#[test]
fn parse_note_builds_every_field() {
    let text = "---\ntitle: Hello World\naliases: [Hi, ' ', Hey]\ntags: [x, y]\ntype: report\nbranch: b\ndescription: A desc\n---\nBody [[Target One|alias]] and [[Other#sec]].\n## Relations\n- [[R]]\n  - nested\n\nTail\n";
    let note = parse_note(text, "Stem");
    assert_eq!(note.blocks.len(), 1);
    assert_eq!(note.fields["type"], Field::Text("report".to_string()));
    assert_eq!(
        note.body,
        "Body [[Target One|alias]] and [[Other#sec]].\n## Relations\n- [[R]]\n  - nested\n\nTail\n"
    );
    assert_eq!(note.title, "Hello World");
    assert_eq!(note.kind, "report");
    assert_eq!(note.aliases, ["Hi", "Hey"]);
    assert_eq!(note.links, set(&["target one", "other", "r"]));
    assert_eq!(note.keys, set(&["hello world", "stem", "hi", "hey"]));
    assert_eq!(
        note.embed_text,
        "Hello World\naliases: Hi, Hey\ntags: x, y\ntype: report\nbranch: b\nA desc\n\nBody [[Target One|alias]] and [[Other#sec]].\n\nTail\n"
    );
}

#[test]
fn parse_note_without_frontmatter_uses_the_stem() {
    let note = parse_note("just text [[X]]", "My Note");
    assert_eq!(note.blocks.len(), 0);
    assert!(note.fields.is_empty());
    assert_eq!(note.body, "just text [[X]]");
    assert_eq!(note.title, "My Note");
    assert_eq!(note.kind, "note");
    assert_eq!(note.aliases.len(), 0);
    assert_eq!(note.keys, set(&["my note"]));
    assert_eq!(note.embed_text, "My Note\n\njust text [[X]]");
}

#[test]
fn parse_note_empty_values_count_as_missing() {
    let note = parse_note("---\ntitle:\ntype:\naliases:\ntags:\n---\nx", "stem");
    assert_eq!(note.title, "stem");
    assert_eq!(note.kind, "note");
    assert_eq!(note.aliases.len(), 0);
    assert_eq!(note.embed_text, "stem\n\nx");
}

#[test]
fn parse_note_strips_title_and_type() {
    let note = parse_note("---\ntitle: \"  Big  \"\ntype: \" session \"\n---\n", "s");
    assert_eq!(note.title, "Big");
    assert_eq!(note.kind, "session");
    assert_eq!(note.keys, set(&["big", "s"]));
    assert_eq!(note.embed_text, "Big\ntype:  session \n\n");
}

#[test]
fn parse_note_blank_title_falls_back_to_the_stem() {
    let note = parse_note("---\ntitle: \"   \"\n---\n", "s");
    assert_eq!(note.title, "s");
}

#[test]
fn parse_note_a_scalar_alias_and_a_scalar_tag() {
    let note = parse_note("---\naliases: Solo\ntags: single\n---\nb", "s");
    assert_eq!(note.aliases, ["Solo"]);
    assert_eq!(note.embed_text, "s\naliases: Solo\ntags: single\n\nb");
    let note = parse_note("---\naliases: \"\"\ntags: ''\n---\nb", "s");
    assert_eq!(note.aliases.len(), 0);
    assert_eq!(note.embed_text, "s\n\nb");
}

#[test]
fn parse_note_block_list_aliases_are_stripped_and_filtered() {
    let note = parse_note("---\naliases:\n  - \" a \"\n  - ''\n  - b\n---\n", "s");
    assert_eq!(note.aliases, ["a", "b"]);
    assert_eq!(note.keys, set(&["s", "a", "b"]));
}

#[test]
fn parse_note_keys_are_normalised() {
    let note = parse_note(
        "---\ntitle: Ha\u{308}rtung\n---\n[[Ha\u{308}rtung]]",
        "H\u{c4}RTUNG",
    );
    assert_eq!(note.keys, set(&["h\u{e4}rtung"]));
    assert_eq!(note.links, set(&["h\u{e4}rtung"]));
}

#[test]
fn parse_note_embed_head_skips_empty_fields_and_lists_in_type_and_branch() {
    let note = parse_note("---\ntype: [a]\nbranch: x\ndescription: []\n---\nb", "s");
    assert_eq!(note.embed_text, "s\nbranch: x\n\nb");
}

#[test]
fn parse_note_links_cover_the_frontmatter_too() {
    let note = parse_note("---\nrelated: \"[[Fm]]\"\n---\n[[Body]]", "s");
    assert_eq!(note.links, set(&["fm", "body"]));
}

#[test]
fn cut_relations_removes_the_section_and_its_dash_lines() {
    assert_eq!(cut_relations("a\n## Relations\n- x\nb"), "a\nb");
    assert_eq!(cut_relations("## Relations\n- x"), "");
    assert_eq!(cut_relations("## Relations\n-\nz"), "z");
    assert_eq!(cut_relations("## Relations\n   - a\n\t- b\nc"), "c");
    assert_eq!(cut_relations("## Relations  \t\n-x\n\n- y"), "\n- y");
    assert_eq!(cut_relations("## Relations\ntext"), "text");
    assert_eq!(
        cut_relations("## Relations\n- a\n## Relations\n- b\nend"),
        "end"
    );
    assert_eq!(
        cut_relations("a\n## Relations\n- a\nb\n## Relations\n- b\nc"),
        "a\nb\nc"
    );
}

#[test]
fn cut_relations_keeps_everything_else() {
    for text in [
        "## Relations",
        "## Relations ",
        "## RelationsX\n- a",
        "foo ## Relations\n- a",
        " ## Relations\n- a",
        "## relations\n- a",
        "### Relations\n- a",
        "plain",
        "",
    ] {
        assert_eq!(cut_relations(text), text, "{text:?}");
    }
}

#[test]
fn wikilinks_ends_a_target_at_bracket_pipe_hash_or_newline() {
    assert_eq!(wikilinks("[[A]] [[b|c]] [[D#e]]"), set(&["a", "b", "d"]));
    assert_eq!(wikilinks("[[a\nb]]"), set(&["a"]));
    assert_eq!(wikilinks("[[a"), set(&["a"]));
    assert_eq!(wikilinks("[[a]b]]"), set(&["a"]));
    assert_eq!(wikilinks("[[ ]]"), set(&[""]));
    assert_eq!(wikilinks("[[  X  ]]"), set(&["x"]));
}

#[test]
fn wikilinks_skips_empty_targets_and_nests_like_the_regex() {
    assert!(wikilinks("[[]] [[|a]] [[#b]] [[").is_empty());
    assert_eq!(wikilinks("[[[a]]"), set(&["[a"]));
    assert_eq!(wikilinks("[[|x]] [[y]]"), set(&["y"]));
    assert_eq!(wikilinks("x [[]]]] [[z]]"), set(&["z"]));
    assert_eq!(wikilinks("[[a]][[b]]"), set(&["a", "b"]));
    assert_eq!(wikilinks("[[a]] [[A]]"), set(&["a"]));
    assert_eq!(wikilinks("[[Ha\u{308}]]"), set(&["h\u{e4}"]));
    assert!(wikilinks("").is_empty());
    assert!(wikilinks("[a] [b]").is_empty());
}
