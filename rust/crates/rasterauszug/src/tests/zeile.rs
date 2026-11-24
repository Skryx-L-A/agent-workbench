//! The line form: every SGR number, every error, the canonical form and its writing (RA1 to RA4).

use super::zeile;
use crate::zeile::{
    DIMM, DURCHGESTRICHEN, FETT, INVERS, KURSIV, UNTERSTRICHEN, VERDECKT, kanonisch, zeichen_ok,
    zeile_lesen, zeile_pruefen, zeile_schreiben,
};
use crate::{Fehler, Stil};

/// A run as the tests write it: foreground, background, flags and text.
type Lauf = (u32, u32, u8, &'static str);

/// The style of a line of one run, or the error.
fn stil_von(sgr: &str) -> Result<Stil, Fehler> {
    let z = zeile_lesen(&format!("{sgr}X"))?;
    assert_eq!(z.len(), 1, "{sgr:?}");
    assert_eq!(z[0].text, "X");
    Ok(z[0].stil)
}

fn stil(fg: u32, bg: u32, fl: u8) -> Stil {
    Stil { fg, bg, fl }
}

#[test]
fn every_attribute_number_sets_its_bit() {
    for (nr, bit) in [
        (1, FETT),
        (2, DIMM),
        (3, KURSIV),
        (4, UNTERSTRICHEN),
        (7, INVERS),
        (8, VERDECKT),
        (9, DURCHGESTRICHEN),
    ] {
        assert_eq!(
            stil_von(&format!("\u{1b}[31;42;{nr}m")),
            Ok(stil(2, 3, bit)),
            "number {nr}"
        );
        assert_eq!(
            stil_von(&format!("\u{1b}[{nr}m")),
            Ok(stil(0, 0, bit)),
            "number {nr}"
        );
    }
    assert_eq!(FETT, 1);
    assert_eq!(DIMM, 2);
    assert_eq!(KURSIV, 4);
    assert_eq!(UNTERSTRICHEN, 8);
    assert_eq!(INVERS, 16);
    assert_eq!(VERDECKT, 32);
    assert_eq!(DURCHGESTRICHEN, 64);
}

#[test]
fn every_off_number_clears_its_bits() {
    let alle = "\u{1b}[1;2;3;4;7;8;9;31;42m";
    assert_eq!(stil_von(alle), Ok(stil(2, 3, 127)));
    for (nr, rest) in [
        (22, 124),
        (23, 123),
        (24, 119),
        (27, 111),
        (28, 95),
        (29, 63),
    ] {
        assert_eq!(
            stil_von(&format!("{alle}\u{1b}[{nr}m")),
            Ok(stil(2, 3, rest)),
            "number {nr}"
        );
    }
}

#[test]
fn colour_numbers_map_to_the_palette() {
    for k in 0..8_u32 {
        assert_eq!(
            stil_von(&format!("\u{1b}[{}m", 30 + k)),
            Ok(stil(1 + k, 0, 0))
        );
        assert_eq!(
            stil_von(&format!("\u{1b}[{}m", 40 + k)),
            Ok(stil(0, 1 + k, 0))
        );
        assert_eq!(
            stil_von(&format!("\u{1b}[{}m", 90 + k)),
            Ok(stil(9 + k, 0, 0))
        );
        assert_eq!(
            stil_von(&format!("\u{1b}[{}m", 100 + k)),
            Ok(stil(0, 9 + k, 0))
        );
    }
    assert_eq!(stil_von("\u{1b}[31;42;39m"), Ok(stil(0, 3, 0)));
    assert_eq!(stil_von("\u{1b}[31;42;49m"), Ok(stil(2, 0, 0)));
    assert_eq!(stil_von("\u{1b}[1;31;42;0m"), Ok(Stil::VORGABE));
}

#[test]
fn a_single_parameter_is_valid_exactly_for_the_known_numbers() {
    let bekannt: Vec<u16> = [0, 1, 2, 3, 4, 7, 8, 9, 22, 23, 24, 27, 28, 29, 39, 49]
        .into_iter()
        .chain(30..=37)
        .chain(40..=47)
        .chain(90..=97)
        .chain(100..=107)
        .collect();
    for p in 0..1000_u16 {
        let ergebnis = zeile_lesen(&format!("\u{1b}[{p}mX"));
        if bekannt.contains(&p) {
            assert!(ergebnis.is_ok(), "number {p}");
        } else {
            assert_eq!(ergebnis, Err(Fehler::SgrParameter), "number {p}");
        }
    }
}

#[test]
fn extended_colours_read_five_and_two_forms() {
    assert_eq!(stil_von("\u{1b}[38;5;0m"), Ok(stil(1, 0, 0)));
    assert_eq!(stil_von("\u{1b}[38;5;255m"), Ok(stil(256, 0, 0)));
    assert_eq!(stil_von("\u{1b}[48;5;17m"), Ok(stil(0, 18, 0)));
    assert_eq!(stil_von("\u{1b}[38;5;256m"), Err(Fehler::SgrParameter));
    assert_eq!(stil_von("\u{1b}[38;5m"), Err(Fehler::SgrParameter));
    assert_eq!(stil_von("\u{1b}[38m"), Err(Fehler::SgrParameter));
    assert_eq!(stil_von("\u{1b}[48m"), Err(Fehler::SgrParameter));
    assert_eq!(stil_von("\u{1b}[38;3;1m"), Err(Fehler::SgrParameter));
    assert_eq!(stil_von("\u{1b}[38;2;0;0;0m"), Ok(stil(0x0100_0000, 0, 0)));
    assert_eq!(stil_von("\u{1b}[38;2;1;2;3m"), Ok(stil(0x0101_0203, 0, 0)));
    assert_eq!(
        stil_von("\u{1b}[48;2;255;254;253m"),
        Ok(stil(0, 0x01ff_fefd, 0))
    );
    for text in [
        "38;2;256;0;0",
        "38;2;0;256;0",
        "38;2;0;0;256",
        "38;2;1;2",
        "38;2;1",
        "38;2",
        "48;2;0;0;256",
    ] {
        assert_eq!(
            stil_von(&format!("\u{1b}[{text}m")),
            Err(Fehler::SgrParameter),
            "{text}"
        );
    }
}

#[test]
fn an_extended_colour_takes_exactly_its_parameters() {
    assert_eq!(stil_von("\u{1b}[38;5;1;1m"), Ok(stil(2, 0, 1)));
    assert_eq!(
        stil_von("\u{1b}[38;2;1;2;3;1m"),
        Ok(stil(0x0101_0203, 0, 1))
    );
    assert_eq!(stil_von("\u{1b}[1;48;5;4;31m"), Ok(stil(2, 5, 1)));
    assert_eq!(stil_von("\u{1b}[38;5;5;5m"), Err(Fehler::SgrParameter));
    assert_eq!(stil_von("\u{1b}[38;2;1;2;3;5m"), Err(Fehler::SgrParameter));
}

#[test]
fn parameters_have_at_most_three_digits_and_empty_ones_are_zero() {
    assert_eq!(stil_von("\u{1b}[001m"), Ok(stil(0, 0, 1)));
    assert_eq!(stil_von("\u{1b}[0001m"), Err(Fehler::SgrParameter));
    assert_eq!(stil_von("\u{1b}[31;1m\u{1b}[m"), Ok(Stil::VORGABE));
    assert_eq!(stil_von("\u{1b}[31;;1m"), Ok(stil(0, 0, 1)));
    assert_eq!(stil_von("\u{1b}[;1m"), Ok(stil(0, 0, 1)));
    assert_eq!(stil_von("\u{1b}[1;m"), Ok(Stil::VORGABE));
    assert_eq!(stil_von("\u{1b}[1m\u{1b}[31m"), Ok(stil(2, 0, 1)));
    assert_eq!(stil_von("\u{1b}[999m"), Err(Fehler::SgrParameter));
}

#[test]
fn bad_sequences_give_their_error() {
    let faelle = [
        ("a\u{1b}", Fehler::SgrUnvollstaendig),
        ("\u{1b}X", Fehler::EscUngueltig),
        ("\u{1b}]", Fehler::EscUngueltig),
        ("\u{1b}[", Fehler::SgrUnvollstaendig),
        ("\u{1b}[1", Fehler::SgrUnvollstaendig),
        ("\u{1b}[1;", Fehler::SgrUnvollstaendig),
        ("\u{1b}[1J", Fehler::SgrUnbekannt),
        ("\u{1b}[H", Fehler::SgrUnbekannt),
        ("\u{1b}[?25h", Fehler::SgrParameter),
        ("\u{1b}[@", Fehler::SgrUnbekannt),
        ("\u{1b}[~", Fehler::SgrUnbekannt),
        ("\u{1b}[\u{7f}", Fehler::SgrParameter),
        ("\u{1b}[1:2m", Fehler::SgrParameter),
        ("\u{1b}[ m", Fehler::SgrParameter),
        ("\u{1b}[\u{e9}m", Fehler::SgrParameter),
        ("\u{1b}[1\u{1b}m", Fehler::SgrParameter),
        ("\u{1b}[+m", Fehler::SgrParameter),
    ];
    for (text, soll) in faelle {
        assert_eq!(zeile_lesen(text), Err(soll), "{text:?}");
    }
}

#[test]
fn characters_are_checked_in_reading_order() {
    for (c, ok) in [
        ('\u{0}', false),
        ('\u{1}', false),
        ('\u{1f}', false),
        ('\u{20}', true),
        ('\u{7e}', true),
        ('\u{7f}', false),
        ('\u{80}', false),
        ('\u{9f}', false),
        ('\u{a0}', true),
        ('\u{2028}', true),
        ('\u{200b}', true),
        ('\u{feff}', true),
        ('\u{10ffff}', true),
    ] {
        assert_eq!(zeichen_ok(c), ok, "{c:?}");
        assert_eq!(zeile_lesen(&format!("a{c}b")).is_ok(), ok, "{c:?}");
    }
    assert_eq!(zeile_lesen("\u{1}\u{1b}"), Err(Fehler::ZeichenUngueltig));
    assert_eq!(zeile_lesen("\u{1b}[1J\u{1}"), Err(Fehler::SgrUnbekannt));
    assert_eq!(
        zeile_lesen("a\u{1b}[1mb\u{1}"),
        Err(Fehler::ZeichenUngueltig)
    );
}

#[test]
fn neighbours_of_one_style_merge_and_empty_sequences_make_no_run() {
    assert_eq!(
        zeile_lesen("\u{1b}[1ma\u{1b}[1mb"),
        Ok(zeile(&[(0, 0, 1, "ab")]))
    );
    assert_eq!(
        zeile_lesen("\u{1b}[1ma\u{1b}[0m\u{1b}[1mb"),
        Ok(zeile(&[(0, 0, 1, "ab")]))
    );
    assert_eq!(
        zeile_lesen("a\u{1b}[1mb\u{1b}[0mc"),
        Ok(zeile(&[(0, 0, 0, "a"), (0, 0, 1, "b"), (0, 0, 0, "c")]))
    );
    assert_eq!(zeile_lesen("\u{1b}[1m"), Ok(Vec::new()));
    assert_eq!(zeile_lesen(""), Ok(Vec::new()));
    assert_eq!(
        zeile_lesen("\u{e9}\u{65e5}\u{1f600}"),
        Ok(zeile(&[(0, 0, 0, "\u{e9}\u{65e5}\u{1f600}")]))
    );
}

#[test]
fn spaces_at_the_end_in_the_default_style_go() {
    assert_eq!(zeile_lesen("a  "), Ok(zeile(&[(0, 0, 0, "a")])));
    assert_eq!(zeile_lesen("   "), Ok(Vec::new()));
    assert_eq!(zeile_lesen("\u{1b}[1m  "), Ok(zeile(&[(0, 0, 1, "  ")])));
    assert_eq!(zeile_lesen("a b "), Ok(zeile(&[(0, 0, 0, "a b")])));
    assert_eq!(
        zeile_lesen("a\u{1b}[1m \u{1b}[0m  "),
        Ok(zeile(&[(0, 0, 0, "a"), (0, 0, 1, " ")]))
    );
    assert_eq!(
        zeile_lesen("a\u{1b}[1mb\u{1b}[0m "),
        Ok(zeile(&[(0, 0, 0, "a"), (0, 0, 1, "b")]))
    );
    assert_eq!(
        zeile_lesen("\u{1b}[1m \u{1b}[0m \u{1b}[1m "),
        Ok(zeile(&[(0, 0, 1, " "), (0, 0, 0, " "), (0, 0, 1, " ")]))
    );
    assert_eq!(zeile_lesen(" a"), Ok(zeile(&[(0, 0, 0, " a")])));
    assert_eq!(zeile_lesen("a\u{a0}"), Ok(zeile(&[(0, 0, 0, "a\u{a0}")])));
    assert_eq!(
        zeile_lesen("\u{1b}[31m \u{1b}[0m"),
        Ok(zeile(&[(2, 0, 0, " ")]))
    );
}

#[test]
fn the_canonical_test_looks_at_each_rule() {
    assert!(kanonisch(&[]));
    assert!(kanonisch(&zeile(&[(0, 0, 0, "a"), (1, 0, 0, "b")])));
    assert!(!kanonisch(&zeile(&[(0, 0, 0, "a"), (0, 0, 0, "b")])));
    assert!(!kanonisch(&zeile(&[(0, 0, 0, "")])));
    assert!(!kanonisch(&zeile(&[(1, 0, 0, "a"), (0, 0, 0, "")])));
    assert!(!kanonisch(&zeile(&[(0, 0, 0, "a ")])));
    assert!(kanonisch(&zeile(&[(1, 0, 0, "a ")])));
    assert!(kanonisch(&zeile(&[(0, 0, 0, "a"), (1, 0, 0, " ")])));
    assert!(kanonisch(&zeile(&[(0, 0, 0, " a")])));
    assert!(!kanonisch(&zeile(&[
        (1, 0, 0, "a"),
        (1, 0, 0, "b"),
        (0, 0, 0, "c")
    ])));
    assert!(!kanonisch(&zeile(&[
        (1, 0, 0, "a"),
        (2, 0, 0, "b"),
        (2, 0, 0, "c")
    ])));
}

#[test]
fn style_values_are_checked_at_their_edges() {
    assert!(Stil::gueltig(0, 0, 0));
    assert!(Stil::gueltig(256, 256, 127));
    assert!(!Stil::gueltig(257, 0, 0));
    assert!(!Stil::gueltig(0, 257, 0));
    assert!(!Stil::gueltig(0, 0, 128));
    assert!(!Stil::gueltig(-1, 0, 0));
    assert!(!Stil::gueltig(0, -1, 0));
    assert!(!Stil::gueltig(0, 0, -1));
    assert!(!Stil::gueltig(0x00ff_ffff, 0, 0));
    assert!(Stil::gueltig(0x0100_0000, 0x01ff_ffff, 0));
    assert!(!Stil::gueltig(0x0200_0000, 0, 0));
    assert!(!Stil::gueltig(0, 0x0200_0000, 0));
    assert_eq!(Stil::neu(0x0100_0001, 7, 99), Ok(stil(0x0100_0001, 7, 99)));
    assert_eq!(
        Stil::neu(1, 0x01ff_ffff, 127),
        Ok(stil(1, 0x01ff_ffff, 127))
    );
    assert_eq!(Stil::neu(257, 0, 0), Err(Fehler::StilUngueltig));
}

#[test]
fn runs_are_written_in_the_canonical_form() {
    let faelle: Vec<(Vec<Lauf>, &str)> = vec![
        (vec![], ""),
        (vec![(0, 0, 0, "a")], "a"),
        (vec![(0, 0, 1, "a")], "\u{1b}[0;1ma"),
        (vec![(0, 0, 127, "a")], "\u{1b}[0;1;2;3;4;7;8;9ma"),
        (vec![(1, 0, 0, "a")], "\u{1b}[0;30ma"),
        (vec![(8, 0, 0, "a")], "\u{1b}[0;37ma"),
        (vec![(9, 0, 0, "a")], "\u{1b}[0;90ma"),
        (vec![(16, 0, 0, "a")], "\u{1b}[0;97ma"),
        (vec![(17, 0, 0, "a")], "\u{1b}[0;38;5;16ma"),
        (vec![(256, 0, 0, "a")], "\u{1b}[0;38;5;255ma"),
        (vec![(0, 1, 0, "a")], "\u{1b}[0;40ma"),
        (vec![(0, 8, 0, "a")], "\u{1b}[0;47ma"),
        (vec![(0, 9, 0, "a")], "\u{1b}[0;100ma"),
        (vec![(0, 16, 0, "a")], "\u{1b}[0;107ma"),
        (vec![(0, 17, 0, "a")], "\u{1b}[0;48;5;16ma"),
        (vec![(0, 256, 0, "a")], "\u{1b}[0;48;5;255ma"),
        (vec![(0x0101_0203, 0, 0, "a")], "\u{1b}[0;38;2;1;2;3ma"),
        (vec![(0, 0x01ff_00fe, 0, "a")], "\u{1b}[0;48;2;255;0;254ma"),
        (vec![(3, 10, 5, "a")], "\u{1b}[0;1;3;32;101ma"),
        (
            vec![(1, 0, 0, "a"), (0, 0, 0, "b")],
            "\u{1b}[0;30ma\u{1b}[0mb",
        ),
        (
            vec![(0, 0, 0, "a"), (2, 0, 0, "b"), (3, 0, 0, "c")],
            "a\u{1b}[0;31mb\u{1b}[0;32mc",
        ),
    ];
    for (laeufe, soll) in faelle {
        let z = zeile(&laeufe);
        assert_eq!(zeile_schreiben(&z), soll, "{laeufe:?}");
        assert_eq!(zeile_lesen(soll), Ok(z), "{soll:?}");
    }
}

#[test]
fn checking_runs_orders_style_then_character_then_form() {
    let ok = vec![(1, 2, 3, "ab".to_owned())];
    assert_eq!(zeile_pruefen(&ok), Ok(zeile(&[(1, 2, 3, "ab")])));
    assert_eq!(zeile_pruefen(&[]), Ok(Vec::new()));
    let bad_style = (300, 0, 0, "\u{1}".to_owned());
    let bad_char = (1, 0, 0, "\u{1}".to_owned());
    let not_canonical = (1, 0, 0, String::new());
    assert_eq!(
        zeile_pruefen(&[bad_char.clone(), bad_style.clone()]),
        Err(Fehler::StilUngueltig)
    );
    assert_eq!(
        zeile_pruefen(&[bad_char, not_canonical.clone()]),
        Err(Fehler::ZeichenUngueltig)
    );
    assert_eq!(
        zeile_pruefen(&[not_canonical]),
        Err(Fehler::ZeileNichtKanonisch)
    );
    for stil in [
        (-1, 0, 0),
        (0, -1, 0),
        (0, 0, -1),
        (0, 0, 128),
        (0, 257, 0),
        (257, 0, 0),
        (i64::MAX, 0, 0),
        (0, i64::MIN, 0),
    ] {
        assert_eq!(
            zeile_pruefen(&[(stil.0, stil.1, stil.2, "a".to_owned())]),
            Err(Fehler::StilUngueltig),
            "{stil:?}"
        );
    }
    assert_eq!(
        zeile_pruefen(&[(0, 0, 0, "a".to_owned()), (0, 0, 0, "b".to_owned())]),
        Err(Fehler::ZeileNichtKanonisch)
    );
    assert_eq!(
        zeile_pruefen(&[(0, 0, 0, "a ".to_owned())]),
        Err(Fehler::ZeileNichtKanonisch)
    );
    assert_eq!(
        zeile_pruefen(&[(0, 0, 0, "a\u{1b}".to_owned())]),
        Err(Fehler::ZeichenUngueltig)
    );
    assert_eq!(
        zeile_pruefen(&[(0, 0, 0, "a\u{9f}".to_owned())]),
        Err(Fehler::ZeichenUngueltig)
    );
}
