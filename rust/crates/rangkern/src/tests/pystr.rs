use super::{data, float, strings};
use crate::pystr::{all_alpha, is_alpha, is_word, lower, np_sum, py_sum, repr_list, repr_str};
use crate::tables::{ALPHA, WORD};

#[test]
fn character_classes_agree_with_python_for_every_sampled_code_point() {
    let rows = data("chars");
    let rows = rows.as_array().expect("rows");
    assert!(rows.len() > 6000);
    for row in rows {
        let cp = u32::try_from(row[0].as_u64().expect("code point")).expect("fits");
        let c = char::from_u32(cp).expect("a char");
        assert_eq!(
            is_alpha(c),
            row[1].as_bool().expect("bool"),
            "isalpha U+{cp:X}"
        );
        assert_eq!(is_word(c), row[2].as_bool().expect("bool"), "word U+{cp:X}");
        if let Some(expected) = row[3].as_str() {
            assert_eq!(lower(&c.to_string()), expected, "lower U+{cp:X}");
        } else {
            assert_eq!(lower(&c.to_string()), c.to_string(), "lower U+{cp:X}");
        }
    }
}

/// Every range starts a run: the code point before is outside, the first and last are inside, the
/// one after is outside (so ranges are sorted, disjoint and not adjacent).
fn check_table(table: &[(u32, u32)], inside: fn(char) -> bool) {
    let mut previous_end = None;
    for &(lo, hi) in table {
        assert!(lo <= hi);
        if let Some(end) = previous_end {
            assert!(end + 1 < lo, "ranges overlap or touch at {lo:X}");
        }
        previous_end = Some(hi);
        let at = |cp: u32| char::from_u32(cp).is_some_and(inside);
        assert!(at(lo) && at(hi));
        assert!(lo == 0 || !at(lo - 1), "before {lo:X}");
        assert!(!at(hi + 1), "after {hi:X}");
    }
}

#[test]
fn tables_are_sorted_disjoint_and_exact_at_every_edge() {
    check_table(ALPHA, is_alpha);
    check_table(WORD, is_word);
}

#[test]
fn word_characters_follow_pythons_isalnum_and_underscore() {
    assert!(
        is_word('_')
            && is_word('ä')
            && is_word('٣')
            && is_word('²')
            && is_word('Ⅷ')
            && is_word('½')
    );
    assert!(!is_word('\'') && !is_word(' ') && !is_word('\u{301}') && !is_word('-'));
    assert!(is_alpha('ß') && is_alpha('日') && is_alpha('ª'));
    assert!(!is_alpha('٣') && !is_alpha('_') && !is_alpha('\u{301}') && !is_alpha('Ⅷ'));
}

#[test]
fn all_alpha_needs_a_non_empty_word_of_letters() {
    assert!(all_alpha("straße") && all_alpha("ä"));
    assert!(!all_alpha("") && !all_alpha("a1") && !all_alpha("a'b") && !all_alpha("a_b"));
}

#[test]
fn lower_follows_the_final_sigma_rule_of_python() {
    for row in data("lower").as_array().expect("rows") {
        let text = row[0].as_str().expect("text");
        assert_eq!(lower(text), row[1].as_str().expect("expected"), "{text:?}");
    }
}

#[test]
fn sums_agree_with_python_and_numpy() {
    let rows = data("sums");
    let rows = rows.as_array().expect("rows");
    assert!(rows.len() > 80);
    for row in rows {
        let values: Vec<f64> = strings(&row["values"])
            .iter()
            .map(|s| s.parse().expect("float"))
            .collect();
        let want_py = float(&row["py_sum"]);
        let want_np = float(&row["np_sum"]);
        assert_eq!(
            py_sum(values.iter().copied()),
            want_py,
            "py_sum of {} values",
            values.len()
        );
        assert_eq!(
            np_sum(&values),
            want_np,
            "np_sum of {} values",
            values.len()
        );
    }
}

#[test]
fn the_sum_is_compensated() {
    assert_eq!(py_sum([1e100, 1.0, -1e100]), 1.0);
    assert_eq!(py_sum([1e16, 1.0, 1.0, -1e16]), 2.0);
    assert_eq!(py_sum(std::iter::empty()), 0.0);
    assert_eq!(py_sum([0.5]), 0.5);
    assert_eq!(np_sum(&[]), 0.0);
    assert_eq!(np_sum(&[2.5]), 2.5);
    assert_eq!(np_sum(&[1e100, 1.0, -1e100]), 0.0);
}

#[test]
fn repr_matches_python_for_the_text_of_a_list() {
    assert_eq!(repr_str("a"), "'a'");
    assert_eq!(repr_str("it's"), "\"it's\"");
    assert_eq!(repr_str("a'b\"c"), "'a\\'b\"c'");
    assert_eq!(repr_str("a\\b\nc\rd\te"), "'a\\\\b\\nc\\rd\\te'");
    assert_eq!(repr_str("\u{1}\u{7f}\u{80}"), "'\\x01\\x7f\\x80'");
    assert_eq!(repr_str("\u{100}ä"), "'\u{100}ä'");
    assert_eq!(repr_list(&["a".into(), "b".into()]), "['a', 'b']");
    assert_eq!(repr_list(&[]), "[]");
}

#[test]
fn lower_casing_and_white_space_agree_with_python_for_every_code_point() {
    use std::collections::{HashMap, HashSet};

    let map = data("lowermap");
    let lowered: HashMap<u32, String> = map["lower"]
        .as_array()
        .expect("rows")
        .iter()
        .map(|row| {
            (
                u32::try_from(row[0].as_u64().expect("cp")).expect("fits"),
                row[1].as_str().expect("text").to_string(),
            )
        })
        .collect();
    let spaces: HashSet<u32> = map["space"]
        .as_array()
        .expect("rows")
        .iter()
        .map(|cp| u32::try_from(cp.as_u64().expect("cp")).expect("fits"))
        .collect();
    assert!(
        lowered.len() > 1400 && spaces.len() == 29,
        "{} {}",
        lowered.len(),
        spaces.len()
    );
    for cp in 0..=0x10_FFFF_u32 {
        let Some(c) = char::from_u32(cp) else {
            continue;
        };
        let single = c.to_string();
        let want = lowered.get(&cp).unwrap_or(&single);
        assert_eq!(&lower(&single), want, "lower U+{cp:X}");
        assert_eq!(
            notiz_kern::text::is_space(c),
            spaces.contains(&cp),
            "space U+{cp:X}"
        );
    }
}

#[test]
fn an_overflowing_sum_is_infinite_not_nan() {
    assert_eq!(py_sum([1e308, 1e308]), f64::INFINITY);
    assert_eq!(py_sum([-1e308, -1e308]), f64::NEG_INFINITY);
}
