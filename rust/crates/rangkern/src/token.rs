//! The tokenizer: raw words, Snowball stems, and index tokens with 4-grams of long stems.

use std::collections::HashMap;

use crate::pystr::{all_alpha, is_word, lower};
use crate::stem::stem;

/// Length of the character n-grams of long stems.
pub const NGRAM_N: usize = 4;
/// Stems of at least this many code points also yield n-grams.
pub const NGRAM_MIN: usize = 6;
/// Prefix of an n-gram token; no real word can start with it.
pub const NGRAM_MARK: char = '#';

/// Python's `re.findall(r"[\w']+", text.lower())`.
#[must_use]
pub fn tokenize(text: &str) -> Vec<String> {
    lower(text)
        .split(|c: char| !(is_word(c) || c == '\''))
        .filter(|word| !word.is_empty())
        .map(ToString::to_string)
        .collect()
}

/// The token functions with a per-process stem cache. The cache never changes a result.
#[derive(Debug, Default)]
pub struct Analyzer {
    stems: HashMap<String, String>,
}

impl Analyzer {
    /// An analyzer with an empty cache.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The stem of a lower-case word; a word with a digit, underscore or apostrophe stays as it is.
    pub fn stem_word(&mut self, word: &str) -> String {
        if let Some(found) = self.stems.get(word) {
            return found.clone();
        }
        if !all_alpha(word) {
            return word.to_string();
        }
        let stemmed = stem(word);
        self.stems.insert(word.to_string(), stemmed.clone());
        stemmed
    }

    /// The stems of the words of `text`, without n-grams.
    pub fn stem_tokens(&mut self, text: &str) -> Vec<String> {
        tokenize(text)
            .iter()
            .map(|word| self.stem_word(word))
            .collect()
    }

    /// The index forms of one word: the stem, then the n-grams of a long stem.
    pub fn index_forms(&mut self, word: &str) -> Vec<String> {
        let stemmed = self.stem_word(word);
        let chars: Vec<char> = stemmed.chars().collect();
        let mut forms = vec![stemmed];
        if chars.len() >= NGRAM_MIN {
            for window in chars.windows(NGRAM_N) {
                let mut gram = String::from(NGRAM_MARK);
                gram.extend(window);
                forms.push(gram);
            }
        }
        forms
    }

    /// The index tokens of `text`: per word its stem, then the n-grams of a long stem.
    pub fn index_tokens(&mut self, text: &str) -> Vec<String> {
        tokenize(text)
            .iter()
            .flat_map(|word| self.index_forms(word))
            .collect()
    }

    /// The counts of the index tokens of `text`, in the order of their first occurrence in the
    /// token list (the order of Python's `Counter`).
    pub fn index_counts(&mut self, text: &str) -> Vec<(String, u32)> {
        let mut words: Vec<(String, u32)> = Vec::new();
        let mut at: HashMap<String, usize> = HashMap::new();
        for word in tokenize(text) {
            if let Some(&i) = at.get(&word) {
                words[i].1 += 1;
            } else {
                at.insert(word.clone(), words.len());
                words.push((word, 1));
            }
        }
        let mut counts: Vec<(String, u32)> = Vec::new();
        let mut index: HashMap<String, usize> = HashMap::new();
        for (word, times) in words {
            for form in self.index_forms(&word) {
                if let Some(&i) = index.get(&form) {
                    counts[i].1 += times;
                } else {
                    index.insert(form.clone(), counts.len());
                    counts.push((form, times));
                }
            }
        }
        counts
    }
}
