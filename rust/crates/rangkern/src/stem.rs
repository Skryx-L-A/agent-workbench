//! The Snowball German stemmer, as `snowballstemmer` 3.1.1 (`german_stemmer.py`) implements it.
//!
//! Copyright (c) 2001, Dr Martin Porter; (c) 2004, 2005, Richard Boulton; (c) 2013, Yoshiki
//! Shibukawa; (c) 2006-2025, Olly Betts. All rights reserved. The algorithm and the suffix lists
//! are taken from the Snowball project (BSD-3-Clause); the licence text stands in the file
//! `NOTICE` of this crate. This is a hand-written port of the generated Python code, checked
//! against it word by word (see the tests and the result report).
//!
//! The word is a vector of code points. The steps work on the end of the word; `p1` and `p2` are
//! the starts of the regions R1 and R2.

/// The vowels of German, with the umlauts: `g_v` of the Snowball script.
const VOWELS: [char; 9] = ['a', 'e', 'i', 'o', 'u', 'y', 'ä', 'ö', 'ü'];
/// Characters before the ending `et` that allow its removal: `g_et_ending`.
const ET_ENDING: [char; 13] = [
    'U', 'd', 'f', 'g', 'k', 'l', 'm', 'n', 'r', 's', 't', 'z', 'ä',
];
/// Characters before a final `s` that allow its removal: `g_s_ending`.
const S_ENDING: [char; 11] = ['b', 'd', 'f', 'g', 'h', 'k', 'l', 'm', 'n', 'r', 't'];
/// Characters before the ending `st` that allow its removal: `g_st_ending`.
const ST_ENDING: [char; 10] = ['b', 'd', 'f', 'g', 'h', 'k', 'l', 'm', 'n', 't'];

/// Step 1 suffixes with their action number.
const STEP1: [(&str, u8); 11] = [
    ("e", 3),
    ("em", 1),
    ("en", 3),
    ("erinnen", 2),
    ("erin", 2),
    ("ln", 5),
    ("ern", 2),
    ("er", 2),
    ("s", 4),
    ("es", 3),
    ("lns", 5),
];
/// Step 2 suffixes with their action number.
const STEP2: [(&str, u8); 5] = [("en", 1), ("er", 1), ("et", 3), ("st", 2), ("est", 1)];
/// Stems before `et` that keep the ending.
const KEEP_ET: [&str; 5] = ["tick", "plan", "geordn", "intern", "tr"];
/// Step 3 suffixes with their action number.
const STEP3: [(&str, u8); 8] = [
    ("end", 1),
    ("ig", 2),
    ("ung", 1),
    ("lich", 3),
    ("isch", 2),
    ("ik", 2),
    ("heit", 3),
    ("keit", 4),
];
/// Suffixes removed after `keit`.
const AFTER_KEIT: [(&str, u8); 2] = [("ig", 1), ("lich", 1)];
/// Apostrophe endings removed at the very end.
const APOSTROPHE: [(&str, u8); 3] = [("'", 1), ("'sch", 1), ("'s", 1)];

/// The length (in code points) and action of the longest entry that ends the word before `end`.
fn longest_suffix(word: &[char], end: usize, list: &[(&str, u8)]) -> Option<(usize, u8)> {
    list.iter()
        .filter_map(|&(suffix, action)| {
            let len = suffix.chars().count();
            (len <= end && word[end - len..end].iter().copied().eq(suffix.chars()))
                .then_some((len, action))
        })
        .max_by_key(|&(len, _)| len)
}

/// Whether the word before `end` ends with `suffix`.
fn ends_with_at(word: &[char], end: usize, suffix: &str) -> bool {
    let len = suffix.chars().count();
    len <= end && word[end - len..end].iter().copied().eq(suffix.chars())
}

/// `u` and `y` between vowels become `U` and `Y`; then `ae`, `oe`, `ue` become umlauts, `ß`
/// becomes `ss`, and `qu` is kept as it is.
fn prelude(word: &str) -> Vec<char> {
    let mut chars: Vec<char> = word.chars().collect();
    for i in 0..chars.len() {
        if i + 2 < chars.len()
            && VOWELS.contains(&chars[i])
            && VOWELS.contains(&chars[i + 2])
            && matches!(chars[i + 1], 'u' | 'y')
        {
            chars[i + 1] = if chars[i + 1] == 'u' { 'U' } else { 'Y' };
        }
    }
    let mut out = Vec::with_capacity(chars.len());
    // Characters of a pair that is already written; the loop is bounded by the word length.
    let mut consumed = 0;
    for i in 0..chars.len() {
        if consumed > 0 {
            consumed -= 1;
            continue;
        }
        match (chars[i], chars.get(i..i + 2)) {
            ('ß', _) => out.extend(['s', 's']),
            (_, Some(['a', 'e'])) => {
                out.push('ä');
                consumed = 1;
            }
            (_, Some(['o', 'e'])) => {
                out.push('ö');
                consumed = 1;
            }
            (_, Some(['u', 'e'])) => {
                out.push('ü');
                consumed = 1;
            }
            (_, Some(['q', 'u'])) => {
                out.extend(['q', 'u']);
                consumed = 1;
            }
            (c, _) => out.push(c),
        }
    }
    out
}

/// The regions R1 and R2 as start indexes; both are the word length when the region is missing.
/// R1 starts at least after the third character.
fn regions(word: &[char]) -> (usize, usize) {
    let len = word.len();
    let is_vowel = |c: &char| VOWELS.contains(c);
    // The index after the first non-vowel that follows a vowel, searching from `from`.
    let after_pair = |from: usize| -> Option<usize> {
        let vowel = from + word[from..].iter().position(is_vowel)?;
        let non_vowel = vowel + 1 + word[vowel + 1..].iter().position(|c| !is_vowel(c))?;
        Some(non_vowel + 1)
    };
    let Some(first) = after_pair(0) else {
        return (len, len);
    };
    let p1 = first.max(3);
    (p1, after_pair(first).unwrap_or(len))
}

/// Steps 1 to 3 and the apostrophe ending; they only remove or replace the end of the word.
fn suffixes(word: &mut Vec<char>, p1: usize, p2: usize) {
    step1(word, p1);
    step2(word, p1);
    step3(word, p1, p2);
    if let Some((len, _)) = longest_suffix(word, word.len(), &APOSTROPHE) {
        let start = word.len() - len;
        if start >= 2 {
            word.truncate(start);
        }
    }
}

/// Step 1: the endings `e em en erinnen erin ln ern er s es lns`.
fn step1(word: &mut Vec<char>, p1: usize) {
    let Some((len, action)) = longest_suffix(word, word.len(), &STEP1) else {
        return;
    };
    let start = word.len() - len;
    if start < p1 {
        return;
    }
    match action {
        1 => {
            if !ends_with_at(word, start, "syst") {
                word.truncate(start);
            }
        }
        2 => word.truncate(start),
        3 => {
            word.truncate(start);
            if word.last() == Some(&'s') && ends_with_at(word, word.len() - 1, "nis") {
                word.pop();
            }
        }
        4 => {
            if S_ENDING.contains(&word[start - 1]) {
                word.truncate(start);
            }
        }
        _ => {
            word.truncate(start);
            word.push('l');
        }
    }
}

/// Step 2: the endings `en er est et st`.
fn step2(word: &mut Vec<char>, p1: usize) {
    let Some((len, action)) = longest_suffix(word, word.len(), &STEP2) else {
        return;
    };
    let start = word.len() - len;
    if start < p1 {
        return;
    }
    match action {
        1 => word.truncate(start),
        2 => {
            if start >= 4 && ST_ENDING.contains(&word[start - 1]) {
                word.truncate(start);
            }
        }
        _ => {
            let kept = KEEP_ET.iter().any(|stem| ends_with_at(word, start, stem));
            if ET_ENDING.contains(&word[start - 1]) && !kept {
                word.truncate(start);
            }
        }
    }
}

/// Whether the character before `end` is `e`.
fn preceded_by_e(word: &[char], end: usize) -> bool {
    word[end - 1] == 'e'
}

/// Step 3: the endings `end ig ung lich isch ik heit keit` and what follows them.
fn step3(word: &mut Vec<char>, p1: usize, p2: usize) {
    let Some((len, action)) = longest_suffix(word, word.len(), &STEP3) else {
        return;
    };
    let start = word.len() - len;
    if start < p2 {
        return;
    }
    match action {
        1 => {
            word.truncate(start);
            if ends_with_at(word, word.len(), "ig") {
                let at = word.len() - 2;
                if at >= p2 && !preceded_by_e(word, at) {
                    word.truncate(at);
                }
            }
        }
        2 => {
            if !preceded_by_e(word, start) {
                word.truncate(start);
            }
        }
        3 => {
            word.truncate(start);
            if ends_with_at(word, word.len(), "er") || ends_with_at(word, word.len(), "en") {
                let at = word.len() - 2;
                if at >= p1 {
                    word.truncate(at);
                }
            }
        }
        _ => {
            word.truncate(start);
            if let Some((len, _)) = longest_suffix(word, word.len(), &AFTER_KEIT) {
                let at = word.len() - len;
                if at >= p2 {
                    word.truncate(at);
                }
            }
        }
    }
}

/// `U` and `Y` become `u` and `y` again, the umlauts lose their dots.
fn postlude(word: Vec<char>) -> String {
    word.into_iter()
        .map(|c| match c {
            'U' | 'ü' => 'u',
            'Y' => 'y',
            'ä' => 'a',
            'ö' => 'o',
            c => c,
        })
        .collect()
}

/// The Snowball German stem of `word`. The word is stemmed as it is written; the caller lower
/// cases it first.
#[must_use]
pub fn stem(word: &str) -> String {
    let mut chars = prelude(word);
    let (p1, p2) = regions(&chars);
    suffixes(&mut chars, p1, p2);
    postlude(chars)
}
