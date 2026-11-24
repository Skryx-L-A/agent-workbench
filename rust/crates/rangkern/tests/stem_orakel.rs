//! Differential test of the stemmer against the Python model.
//!
//! `RANGKERN_STEM_ORAKEL` names a file with one `word<TAB>stem` pair per line, made by the oracle
//! `stem-ref` (`snowballstemmer` 3.1.1). Without the variable the test does nothing; the unit
//! tests carry a fixed sample of the same oracle. The result report names the size of the run.

use std::fs;

use rangkern::stem::stem;

#[test]
fn stems_agree_with_the_oracle_when_one_is_given() {
    let Some(path) = std::env::var_os("RANGKERN_STEM_ORAKEL") else {
        return;
    };
    let text = fs::read_to_string(path).expect("oracle file is readable");
    let mut checked = 0_usize;
    let mut failures = Vec::new();
    for line in text.lines() {
        let (word, expected) = line.split_once('\t').expect("a line has a tab");
        checked += 1;
        let got = stem(word);
        if got != expected {
            failures.push(format!("{word:?}: got {got:?}, expected {expected:?}"));
        }
    }
    assert!(checked >= 50_000, "only {checked} words");
    assert!(
        failures.is_empty(),
        "{} of {checked} words differ:\n{}",
        failures.len(),
        failures[..failures.len().min(40)].join("\n")
    );
}
