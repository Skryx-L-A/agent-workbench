//! Keys, text and paste: the byte tables of rule R1.

use crate::eingabe::{
    ALT, STRG, UMSCHALT, einfuegen_bytes, taste_bytes, text_bytes, umschalter_bit,
};

#[test]
fn the_modifier_names_have_their_bits() {
    assert_eq!(umschalter_bit("umschalt"), Some(UMSCHALT));
    assert_eq!(umschalter_bit("strg"), Some(STRG));
    assert_eq!(umschalter_bit("alt"), Some(ALT));
    assert_eq!(umschalter_bit("Alt"), None);
    assert_eq!(umschalter_bit(""), None);
    assert_eq!([UMSCHALT, STRG, ALT], [1, 2, 4]);
}

#[test]
fn the_key_table_holds_the_measured_forms() {
    let tabelle: &[(&str, u8, &[u8])] = &[
        ("Enter", 0, b"\r"),
        ("Enter", UMSCHALT, b"\x1b\r"),
        ("Tab", 0, b"\t"),
        ("Tab", UMSCHALT, b"\x1b[Z"),
        ("Escape", 0, b"\x1b"),
        ("Ruecktaste", 0, b"\x7f"),
        ("Ruecktaste", ALT, b"\x1b\x7f"),
        ("Entf", 0, b"\x1b[3~"),
        ("Hoch", 0, b"\x1b[A"),
        ("Runter", 0, b"\x1b[B"),
        ("Rechts", 0, b"\x1b[C"),
        ("Links", 0, b"\x1b[D"),
        ("Links", STRG, b"\x1b[1;5D"),
        ("Rechts", STRG, b"\x1b[1;5C"),
        ("Links", ALT, b"\x1b[1;3D"),
        ("Rechts", ALT, b"\x1b[1;3C"),
        ("Pos1", 0, b"\x1b[H"),
        ("Ende", 0, b"\x1b[F"),
        ("BildAuf", 0, b"\x1b[5~"),
        ("BildAb", 0, b"\x1b[6~"),
    ];
    for (taste, bits, bytes) in tabelle {
        assert_eq!(
            taste_bytes(taste, *bits).as_deref(),
            Some(*bytes),
            "{taste} {bits}"
        );
    }
}

#[test]
fn a_key_with_another_modifier_is_unknown() {
    let namen = [
        "Enter",
        "Tab",
        "Escape",
        "Ruecktaste",
        "Entf",
        "Hoch",
        "Runter",
        "Rechts",
        "Links",
        "Pos1",
        "Ende",
        "BildAuf",
        "BildAb",
    ];
    let erlaubt = |taste: &str, bits: u8| {
        matches!(
            (taste, bits),
            ("Enter" | "Tab", UMSCHALT)
                | ("Ruecktaste" | "Links" | "Rechts", ALT)
                | ("Links" | "Rechts", STRG)
        ) || bits == 0
    };
    for taste in namen {
        for bits in 0..8_u8 {
            assert_eq!(
                taste_bytes(taste, bits).is_some(),
                erlaubt(taste, bits),
                "{taste} {bits}"
            );
        }
    }
}

#[test]
fn letters_take_ctrl_alt_or_both() {
    assert_eq!(taste_bytes("a", STRG), Some(vec![1]));
    assert_eq!(taste_bytes("z", STRG), Some(vec![26]));
    assert_eq!(taste_bytes("k", STRG), Some(vec![11]));
    assert_eq!(taste_bytes("a", ALT), Some(vec![0x1b, b'a']));
    assert_eq!(taste_bytes("z", ALT), Some(vec![0x1b, b'z']));
    assert_eq!(taste_bytes("d", ALT | STRG), Some(vec![0x1b, 4]));
    assert_eq!(taste_bytes("a", 0), None);
    assert_eq!(taste_bytes("a", UMSCHALT), None);
    assert_eq!(taste_bytes("a", UMSCHALT | STRG), None);
    assert_eq!(taste_bytes("a", UMSCHALT | ALT), None);
    assert_eq!(taste_bytes("A", STRG), None);
    assert_eq!(taste_bytes("`", STRG), None);
    assert_eq!(taste_bytes("{", STRG), None);
    assert_eq!(taste_bytes("ab", STRG), None);
    assert_eq!(taste_bytes("", STRG), None);
    assert_eq!(taste_bytes("F1", 0), None);
    assert_eq!(taste_bytes("\u{e4}", STRG), None);
}

#[test]
fn text_with_a_control_character_is_refused() {
    assert_eq!(text_bytes("abc"), Some(b"abc".to_vec()));
    assert_eq!(text_bytes(""), Some(Vec::new()));
    assert_eq!(text_bytes("\u{20}"), Some(b" ".to_vec()));
    assert_eq!(text_bytes("\u{7e}"), Some(b"~".to_vec()));
    assert_eq!(text_bytes("\u{a0}"), Some("\u{a0}".as_bytes().to_vec()));
    assert_eq!(
        text_bytes("\u{e4}\u{20ac}"),
        Some("\u{e4}\u{20ac}".as_bytes().to_vec())
    );
    for grenze in [
        '\u{0}', '\u{1f}', '\u{7f}', '\u{80}', '\u{9f}', '\n', '\r', '\t', '\u{1b}',
    ] {
        assert_eq!(text_bytes(&format!("a{grenze}b")), None, "{grenze:?}");
    }
}

#[test]
fn a_paste_with_brackets_turns_line_breaks_into_cr() {
    assert_eq!(einfuegen_bytes("a\nb", true), b"\x1b[200~a\rb\x1b[201~");
    assert_eq!(einfuegen_bytes("a\r\nb", true), b"\x1b[200~a\rb\x1b[201~");
    assert_eq!(einfuegen_bytes("a\rb", true), b"\x1b[200~a\rb\x1b[201~");
    assert_eq!(
        einfuegen_bytes("a\r\n\r\nb\n\n", true),
        b"\x1b[200~a\r\rb\r\r\x1b[201~"
    );
    assert_eq!(einfuegen_bytes("x", true), b"\x1b[200~x\x1b[201~");
}

#[test]
fn a_paste_without_brackets_never_sends_cr() {
    assert_eq!(einfuegen_bytes("a\nb", false), b"a\nb");
    assert_eq!(einfuegen_bytes("a\r\nb", false), b"a\nb");
    assert_eq!(einfuegen_bytes("a\rb", false), b"a\nb");
    assert_eq!(einfuegen_bytes("a\r\n\r\nb\r", false), b"a\n\nb\n");
    assert_eq!(einfuegen_bytes("x", false), b"x");
    assert_eq!(einfuegen_bytes("\u{e4}", false), "\u{e4}".as_bytes());
}

#[test]
fn bracket_marks_in_the_text_are_removed_once() {
    assert_eq!(einfuegen_bytes("a\x1b[200~b\x1b[201~c", false), b"abc");
    assert_eq!(
        einfuegen_bytes("a\x1b[200~b\x1b[201~c", true),
        b"\x1b[200~abc\x1b[201~"
    );
    assert_eq!(einfuegen_bytes("\x1b[200\x1b[200~~", false), b"\x1b[200~");
    assert_eq!(einfuegen_bytes("\x1b[201\x1b[201~~", false), b"\x1b[201~");
    assert_eq!(einfuegen_bytes("\x1b[200~\x1b[201~", false), b"");
    assert_eq!(einfuegen_bytes("\x1b[200~\x1b[201~", true), b"");
    assert_eq!(einfuegen_bytes("", true), b"");
    assert_eq!(einfuegen_bytes("", false), b"");
}
