use super::TempDir;
use crate::notes::{Raw, note_rec};
use crate::serve::{Line, Server};
use crate::stamp::Detect;

/// The raw values of `datenklasse` and `kunde` of a note with the given frontmatter lines.
fn raws(front: &str) -> (Raw, Raw) {
    let note = note_rec("a.md", format!("---\n{front}\n---\nText\n"));
    (note.datenklasse, note.kunde)
}

fn klasse(front: &str) -> Raw {
    raws(front).0
}

fn text(value: &str) -> Raw {
    Raw::Text(value.to_string())
}

#[test]
fn a_missing_field_is_missing_and_a_note_without_frontmatter_has_none() {
    assert_eq!(raws("title: A"), (Raw::Missing, Raw::Missing));
    let bare = note_rec("a.md", "Text ohne Kopf\n".to_string());
    assert_eq!((bare.datenklasse, bare.kunde), (Raw::Missing, Raw::Missing));
    // The key matches by its exact name.
    assert_eq!(klasse("Datenklasse: intern"), Raw::Missing);
    assert_eq!(klasse("datenklasse2: intern"), Raw::Missing);
    // The field `class` of the vault is another field.
    assert_eq!(klasse("class: derived"), Raw::Missing);
}

#[test]
fn a_field_without_a_value_or_with_null_is_null() {
    assert_eq!(klasse("datenklasse:"), Raw::Null);
    assert_eq!(klasse("datenklasse:   "), Raw::Null);
    assert_eq!(klasse("datenklasse: null"), Raw::Null);
    assert_eq!(klasse("datenklasse: Null"), Raw::Null);
    assert_eq!(klasse("datenklasse: NULL"), Raw::Null);
    assert_eq!(klasse("datenklasse: ~"), Raw::Null);
    assert_eq!(klasse("datenklasse: null  # kein Wert"), Raw::Null);
    // Another word that starts like null is text.
    assert_eq!(klasse("datenklasse: nullable"), text("nullable"));
    assert_eq!(klasse("datenklasse: nULL"), text("nULL"));
    // Text under the empty key is not read by the reader, so the value stays empty.
    assert_eq!(klasse("datenklasse:\ntitle: A"), Raw::Null);
}

#[test]
fn an_empty_text_is_a_text_not_null() {
    assert_eq!(klasse("datenklasse: \"\""), text(""));
    assert_eq!(klasse("datenklasse: ''"), text(""));
    assert_eq!(klasse("datenklasse: \"\"  # leer"), text(""));
}

#[test]
fn a_text_is_the_text_plain_or_in_quotes() {
    assert_eq!(klasse("datenklasse: intern"), text("intern"));
    assert_eq!(klasse("datenklasse: \"intern\""), text("intern"));
    assert_eq!(klasse("datenklasse: 'intern'"), text("intern"));
    assert_eq!(klasse("datenklasse:   intern   "), text("intern"));
    assert_eq!(klasse("datenklasse: intern  # Kommentar"), text("intern"));
    assert_eq!(klasse("datenklasse: Intern"), text("Intern"));
    assert_eq!(klasse("datenklasse: \"a b\""), text("a b"));
    // A quoted word that looks like null, a truth value or a number is a text.
    assert_eq!(klasse("datenklasse: \"null\""), text("null"));
    assert_eq!(klasse("datenklasse: 'true'"), text("true"));
    assert_eq!(klasse("datenklasse: \"5\""), text("5"));
    assert_eq!(klasse("datenklasse: '~'"), text("~"));
    assert_eq!(klasse("datenklasse: \"{a: 1}\""), text("{a: 1}"));
    assert_eq!(klasse("datenklasse: \"[a]\""), text("[a]"));
}

#[test]
fn a_wrapped_plain_scalar_is_a_text_whatever_its_first_line_says() {
    assert_eq!(klasse("datenklasse: 5\n  Stück"), text("5 Stück"));
    assert_eq!(klasse("datenklasse: true\n  Kopie"), text("true Kopie"));
    assert_eq!(klasse("datenklasse: null\n  Wert"), text("null Wert"));
    assert_eq!(klasse("datenklasse: ganz\n  intern"), text("ganz intern"));
}

#[test]
fn a_list_is_another_type_also_an_empty_one() {
    assert_eq!(klasse("datenklasse: [intern]"), Raw::Other);
    assert_eq!(
        klasse("datenklasse: [\"intern\", \"kundendaten\"]"),
        Raw::Other
    );
    assert_eq!(klasse("datenklasse: []"), Raw::Other);
    assert_eq!(klasse("datenklasse: [ ]"), Raw::Other);
    assert_eq!(klasse("datenklasse:\n  - intern"), Raw::Other);
    assert_eq!(klasse("datenklasse:\n- intern\n- kundendaten"), Raw::Other);
    // A list line under a scalar turns the field into a list.
    assert_eq!(klasse("datenklasse: intern\n  - a"), Raw::Other);
}

#[test]
fn a_mapping_is_another_type() {
    assert_eq!(klasse("datenklasse: {a: 1}"), Raw::Other);
    assert_eq!(klasse("datenklasse: {}"), Raw::Other);
}

#[test]
fn a_number_is_another_type() {
    for number in [
        "5", "0", "007", "0755", "-3", "+3", "3.5", "-3.5", ".5", "5.", "1e3", "1E3", "1.5e-3",
        "1e+3", "-.5e2", "0x1F", "0xff", "0o17", ".inf", "-.inf", "+.Inf", ".INF", ".nan", ".NaN",
        ".NAN",
    ] {
        assert_eq!(
            klasse(&format!("datenklasse: {number}")),
            Raw::Other,
            "{number}"
        );
    }
    // Not a number: text, so the gate sees an unknown class.
    for word in [
        "1e", "e3", "1e+", "1.5.2", "--3", "0x", "0o", "0o8", "0xg", "x1", "1_000", ".", "-", "+",
        "1,5", ".inf5", "inf", "nan", ".nan5", "-.nan", "1 2", "5x",
    ] {
        assert_eq!(
            klasse(&format!("datenklasse: {word}")),
            text(word),
            "{word}"
        );
    }
}

#[test]
fn a_truth_value_is_another_type() {
    for word in ["true", "True", "TRUE", "false", "False", "FALSE"] {
        assert_eq!(
            klasse(&format!("datenklasse: {word}")),
            Raw::Other,
            "{word}"
        );
    }
    // The words of YAML 1.1 are text here, as for the reader of `notiz-kern`.
    for word in ["yes", "no", "on", "off", "tRUE", "truee"] {
        assert_eq!(
            klasse(&format!("datenklasse: {word}")),
            text(word),
            "{word}"
        );
    }
}

#[test]
fn the_last_line_of_the_key_wins_across_lines_and_blocks() {
    assert_eq!(klasse("datenklasse: intern\ndatenklasse: 5"), Raw::Other);
    assert_eq!(
        klasse("datenklasse: 5\ndatenklasse: intern"),
        text("intern")
    );
    assert_eq!(klasse("datenklasse: intern\ndatenklasse:"), Raw::Null);
    assert_eq!(klasse("datenklasse: intern\ndatenklasse: \"\""), text(""));
    let two_blocks = note_rec(
        "a.md",
        "---\ndatenklasse: intern\n---\n\n---\ndatenklasse: 5\nkunde: k-a\n---\nText\n".to_string(),
    );
    assert_eq!(two_blocks.datenklasse, Raw::Other);
    assert_eq!(two_blocks.kunde, text("k-a"));
}

#[test]
fn a_line_inside_another_field_or_a_comment_is_no_field() {
    // An indented line belongs to the field above, however it looks.
    assert_eq!(klasse("title: A\n  datenklasse: intern"), Raw::Missing);
    assert_eq!(klasse("# datenklasse: intern"), Raw::Missing);
    assert_eq!(
        klasse("datenklasse: intern\n# datenklasse: 5"),
        text("intern")
    );
    // A block outside the head is no frontmatter.
    let late = note_rec("a.md", "Text\n---\ndatenklasse: intern\n---\n".to_string());
    assert_eq!(late.datenklasse, Raw::Missing);
    // Tab after the colon.
    assert_eq!(klasse("datenklasse:\tintern"), text("intern"));
}

#[test]
fn kunde_follows_the_same_rules_as_datenklasse() {
    assert_eq!(raws("kunde:").1, Raw::Null);
    assert_eq!(raws("kunde: null").1, Raw::Null);
    assert_eq!(raws("kunde: \"\"").1, text(""));
    assert_eq!(raws("kunde: k-nord").1, text("k-nord"));
    assert_eq!(raws("kunde: \"k-nord\"").1, text("k-nord"));
    assert_eq!(raws("kunde: [k-nord]").1, Raw::Other);
    assert_eq!(raws("kunde: 7").1, Raw::Other);
    assert_eq!(raws("kunde: true").1, Raw::Other);
    assert_eq!(raws("kunde: \"7\"").1, text("7"));
    assert_eq!(raws("datenklasse: intern").1, Raw::Missing);
    // The two fields are read independently of each other.
    assert_eq!(
        raws("datenklasse: kundendaten\nkunde: k-a"),
        (text("kundendaten"), text("k-a"))
    );
    assert_eq!(
        raws("kunde: k-a\ndatenklasse: 5"),
        (Raw::Other, text("k-a"))
    );
}

#[test]
fn the_gate_takes_no_value_as_none_a_text_as_itself_and_another_type_as_the_empty_text() {
    assert_eq!(Raw::Missing.for_gate(), None);
    assert_eq!(Raw::Null.for_gate(), None);
    assert_eq!(text("intern").for_gate(), Some("intern"));
    assert_eq!(text("").for_gate(), Some(""));
    assert_eq!(Raw::Other.for_gate(), Some(""));
}

#[test]
fn the_log_key_is_the_text_if_it_has_the_form_of_a_key_else_none() {
    assert_eq!(text("k-nord").log_key(), Some("k-nord"));
    assert_eq!(text("a").log_key(), Some("a"));
    assert_eq!(text("0").log_key(), Some("0"));
    assert_eq!(text("z9._-").log_key(), Some("z9._-"));
    let longest = format!("a{}", "b".repeat(63));
    assert_eq!(longest.len(), 64);
    assert_eq!(text(&longest).log_key(), Some(longest.as_str()));
    let too_long = format!("a{}", "b".repeat(64));
    assert_eq!(text(&too_long).log_key(), None);
    for bad in [
        "", "K-Nord", "k nord", "-k", ".k", "_k", "k/nord", "k\n", "\nk", "kü", "k,a", " k", "k ",
    ] {
        assert_eq!(text(bad).log_key(), None, "{bad:?}");
    }
    // Every other kind of value has no key.
    assert_eq!(Raw::Missing.log_key(), None);
    assert_eq!(Raw::Null.log_key(), None);
    assert_eq!(Raw::Other.log_key(), None);
}

fn server(dir: &TempDir) -> Server {
    Server::start_with(dir.path().join("vault"), None, Detect::Metadata)
}

fn ask(server: &mut Server, line: &str) -> serde_json::Value {
    server
        .answer(&Line {
            bytes: line.as_bytes().to_vec(),
            too_long: false,
        })
        .0
}

#[test]
fn the_server_gives_the_values_as_the_file_holds_them_at_the_next_request() {
    let dir = TempDir::new();
    let rel = "a.md";
    let search = r#"{"op":"search","query":"garten"}"#;
    dir.write(
        "vault/a.md",
        b"---\ntitle: Garten\ndatenklasse: intern\nkunde: k-b\n---\nBeet und Gartenarbeit\n",
    );
    let mut server = server(&dir);
    assert!(server.note("fehlt.md").is_none());
    let before = ask(&mut server, search);
    let note = server.note(rel).expect("the note is in the index");
    assert_eq!(
        (&note.datenklasse, &note.kunde),
        (&text("intern"), &text("k-b"))
    );
    // The note changes between two requests: the next request sees the new values.
    dir.write(
        "vault/a.md",
        b"---\ntitle: Garten\ndatenklasse: 5\nkunde: \"k-a\"\n---\nBeet und Gartenarbeit\n",
    );
    let after = ask(&mut server, search);
    let note = server.note(rel).expect("the note is in the index");
    assert_eq!(
        (&note.datenklasse, &note.kunde),
        (&Raw::Other, &text("k-a"))
    );
    // The answer has no new field and the same hits.
    assert_eq!(before, after);
    assert_eq!(
        after
            .as_object()
            .expect("object")
            .keys()
            .collect::<Vec<_>>(),
        ["fallback", "hits"]
    );
    // The values come back to missing when the fields go.
    dir.write(
        "vault/a.md",
        b"---\ntitle: Garten\n---\nBeet und Gartenarbeit\n",
    );
    ask(&mut server, search);
    let note = server.note(rel).expect("the note is in the index");
    assert_eq!(
        (&note.datenklasse, &note.kunde),
        (&Raw::Missing, &Raw::Missing)
    );
}
