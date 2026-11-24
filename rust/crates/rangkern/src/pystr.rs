//! Python `str` semantics that differ from Rust's: character classes, lower casing, float sums.
//!
//! The tables in [`crate::tables`] are generated from the Python of the reference (Unicode 17.0.0).

use std::cmp::Ordering;
use std::fmt::Write;

use crate::tables::{ALPHA, WORD};

/// Whether `cp` lies in one of the sorted, disjoint, inclusive ranges.
fn in_ranges(table: &[(u32, u32)], cp: u32) -> bool {
    table
        .binary_search_by(|&(lo, hi)| {
            if cp < lo {
                Ordering::Greater
            } else if cp > hi {
                Ordering::Less
            } else {
                Ordering::Equal
            }
        })
        .is_ok()
}

/// Python's `str.isalpha()` for one character (the categories Lu, Ll, Lt, Lm, Lo).
#[must_use]
pub fn is_alpha(c: char) -> bool {
    in_ranges(ALPHA, u32::from(c))
}

/// Python's `\w` for text patterns: `str.isalnum()` or `_`.
#[must_use]
pub fn is_word(c: char) -> bool {
    in_ranges(WORD, u32::from(c))
}

/// Python's `str.isalpha()` for a word: not empty and every character a letter.
#[must_use]
pub fn all_alpha(word: &str) -> bool {
    !word.is_empty() && word.chars().all(is_alpha)
}

/// Python's `str.lower()`: full case mapping with the final-sigma rule, as `str::to_lowercase`.
#[must_use]
pub fn lower(text: &str) -> String {
    text.to_lowercase()
}

/// Python's `sum()` over floats (`CPython` 3.12 and later): Neumaier's compensated summation.
/// The empty sum is 0.0.
#[must_use]
pub fn py_sum<I: IntoIterator<Item = f64>>(values: I) -> f64 {
    let mut iter = values.into_iter();
    let Some(first) = iter.next() else {
        return 0.0;
    };
    // CPython adds the first float to the integer 0 before the compensated loop starts.
    let mut total = 0.0 + first;
    let mut compensation = 0.0;
    for x in iter {
        let t = total + x;
        if total.abs() >= x.abs() {
            compensation += (total - t) + x;
        } else {
            compensation += (x - t) + total;
        }
        total = t;
    }
    if compensation != 0.0 && compensation.is_finite() {
        total += compensation;
    }
    total
}

/// `NumPy`'s `ndarray.sum()` of a contiguous float array: the pairwise sum (blocks of eight
/// accumulators up to 128 elements, halves above).
#[must_use]
pub fn np_sum(values: &[f64]) -> f64 {
    pairwise(values)
}

/// The recursive part of `NumPy`'s pairwise summation.
fn pairwise(a: &[f64]) -> f64 {
    let n = a.len();
    if n < 8 {
        let mut res = 0.0;
        for x in a {
            res += x;
        }
        res
    } else if n <= 128 {
        let mut r = [a[0], a[1], a[2], a[3], a[4], a[5], a[6], a[7]];
        let (blocks, rest) = a[8..].as_chunks::<8>();
        for block in blocks {
            for (acc, x) in r.iter_mut().zip(block) {
                *acc += x;
            }
        }
        let mut res = ((r[0] + r[1]) + (r[2] + r[3])) + ((r[4] + r[5]) + (r[6] + r[7]));
        for x in rest {
            res += x;
        }
        res
    } else {
        let mut half = n / 2;
        half -= half % 8;
        pairwise(&a[..half]) + pairwise(&a[half..])
    }
}

/// Python's `repr()` of a `str`, for the text of a list value (`str(["a", "b"])`).
#[must_use]
pub fn repr_str(s: &str) -> String {
    let quote = if s.contains('\'') && !s.contains('"') {
        '"'
    } else {
        '\''
    };
    let mut out = String::new();
    out.push(quote);
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c == quote => {
                out.push('\\');
                out.push(c);
            }
            c if c.is_control() => {
                let cp = u32::from(c);
                let _ = if cp <= 0xff {
                    write!(out, "\\x{cp:02x}")
                } else {
                    write!(out, "\\u{cp:04x}")
                };
            }
            c => out.push(c),
        }
    }
    out.push(quote);
    out
}

/// Python's `str(list_of_str)`: `['a', 'b']`.
#[must_use]
pub fn repr_list(items: &[String]) -> String {
    let parts: Vec<String> = items.iter().map(|s| repr_str(s)).collect();
    format!("[{}]", parts.join(", "))
}
