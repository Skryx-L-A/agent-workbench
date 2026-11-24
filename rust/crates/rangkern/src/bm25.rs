//! BM25 Okapi with the title bonus and the separate corpus for derived pages.
//!
//! The formulas and the order of every float operation follow the Python model (`Okapi` with
//! `NumPy`), so that the scores agree to the last digits.

use std::collections::{HashMap, HashSet};

use crate::hit::{SearchHit, max_normalize, pool_trim};
use crate::notes::{NoteRec, is_derived};
use crate::pystr::np_sum;
use crate::token::Analyzer;

/// Saturation of the term frequency.
const K1: f64 = 1.5;
/// Length normalisation.
const B: f64 = 0.75;
/// Floor of the idf of a term in more than half of the documents, as a share of the average idf.
const EPSILON: f64 = 0.25;
/// Bonus per stem the query and the title have in common.
pub const TITLE_BOOST: f64 = 2.0;

/// The term counts of one document. `terms` keeps the order of first occurrence.
#[derive(Debug)]
struct Doc {
    terms: Vec<u32>,
    counts: HashMap<u32, u32>,
    len: u64,
}

/// One note with what the index knows about it: its term counts, whether it is a derived page,
/// and the stems of its title.
#[derive(Debug)]
struct Entry {
    note: NoteRec,
    doc: Doc,
    derived: bool,
    title_stems: HashSet<String>,
}

/// The inverse document frequencies of a set of documents and their average length. `idf` is
/// indexed by the id of the term; a term in no document has none.
#[derive(Debug)]
struct Okapi {
    size: usize,
    avgdl: f64,
    idf: Vec<Option<f64>>,
}

impl Okapi {
    /// The statistics of `docs`, whose term ids are below `vocabulary`; there must be at least one
    /// document. The idf values are summed in the order of the first occurrence of their terms
    /// over the documents, which is what makes the average the same number on every build.
    fn new(docs: &[&Doc], vocabulary: usize) -> Self {
        let size = docs.len();
        let total: u64 = docs.iter().map(|doc| doc.len).sum();
        #[allow(clippy::cast_precision_loss)]
        // Token counts and document counts stay far below 2^52; the casts are exact.
        let avgdl = total as f64 / size as f64;
        let mut freq = vec![0_u32; vocabulary];
        let mut order: Vec<u32> = Vec::new();
        for doc in docs {
            for &term in &doc.terms {
                if let Some(count) = freq.get_mut(term as usize) {
                    if *count == 0 {
                        order.push(term);
                    }
                    *count += 1;
                }
            }
        }
        #[allow(clippy::cast_precision_loss)]
        // Document counts stay far below 2^52; the casts are exact.
        let mut values: Vec<f64> = order
            .iter()
            .map(|&term| {
                let n = f64::from(freq[term as usize]);
                ((size as f64 - n) + 0.5).ln() - (n + 0.5).ln()
            })
            .collect();
        #[allow(clippy::cast_precision_loss)]
        // The vocabulary size stays far below 2^52; the cast is exact.
        let average = if values.is_empty() {
            0.0
        } else {
            np_sum(&values) / values.len() as f64
        };
        for value in &mut values {
            if *value < 0.0 {
                *value = EPSILON * average;
            }
        }
        let mut idf = vec![None; vocabulary];
        for (term, value) in order.into_iter().zip(values) {
            idf[term as usize] = Some(value);
        }
        Self { size, avgdl, idf }
    }

    /// The BM25 score of every document for the query terms (repeated terms count again).
    fn scores(&self, docs: &[&Doc], query: &[u32]) -> Vec<f64> {
        let mut score = vec![0.0_f64; self.size];
        for term in query {
            let Some(idf) = self.idf.get(*term as usize).copied().flatten() else {
                continue;
            };
            for (total, doc) in score.iter_mut().zip(docs) {
                let freq = f64::from(doc.counts.get(term).copied().unwrap_or(0));
                #[allow(clippy::cast_precision_loss)]
                // A document length stays far below 2^52; the cast is exact.
                let len = doc.len as f64;
                let norm = (1.0 - B) + (B * len) / self.avgdl;
                *total += idf * ((freq * (K1 + 1.0)) / (freq + K1 * norm));
            }
        }
        score
    }
}

/// Where an entry of a rebuilt index comes from.
#[derive(Debug)]
pub enum Origin {
    /// The entry the index already has at this position; nothing is analysed again.
    Kept(usize),
    /// A note the index has not seen in this form; its text is analysed.
    Fresh(NoteRec),
}

/// BM25 over a list of notes. Derived pages are ranked against the whole corpus, source
/// notes against a corpus without the derived pages.
///
/// The index keeps the analysis of every note it was given, also of those it was told to leave
/// out of the corpus (originals of a source), so that [`LexKorpus::rebuild`] can change the list
/// without analysing an unchanged note again. The statistics are always computed from the
/// listed notes in list order with the same code, so a rebuilt index gives the same numbers as one
/// built from scratch.
#[derive(Debug, Default)]
pub struct LexKorpus {
    entries: Vec<Entry>,
    listed: Vec<usize>,
    vocabulary: HashMap<String, u32>,
    full: Option<Okapi>,
    sources: Option<Okapi>,
    source_index: Vec<usize>,
}

impl LexKorpus {
    /// Counts the index tokens of every note and builds the statistics.
    #[must_use]
    pub fn new(notes: Vec<NoteRec>, analyzer: &mut Analyzer) -> Self {
        let mut korpus = Self::default();
        korpus.rebuild(
            notes
                .into_iter()
                .map(|note| (Origin::Fresh(note), true))
                .collect(),
            analyzer,
        );
        korpus
    }

    /// Counts the index tokens of one note.
    fn analyze(&mut self, note: NoteRec, analyzer: &mut Analyzer) -> Entry {
        let mut terms = Vec::new();
        let mut counts = HashMap::new();
        let mut len = 0_u64;
        for (term, count) in analyzer.index_counts(&note.text) {
            let next = u32::try_from(self.vocabulary.len()).unwrap_or(u32::MAX);
            let id = *self.vocabulary.entry(term).or_insert(next);
            terms.push(id);
            counts.insert(id, count);
            len += u64::from(count);
        }
        let derived = is_derived(&note.fields);
        let title_stems = analyzer.stem_tokens(&note.title).into_iter().collect();
        Entry {
            note,
            doc: Doc { terms, counts, len },
            derived,
            title_stems,
        }
    }

    /// Replaces the list of notes by `plan`, in its order. The flag says whether the note belongs
    /// to the corpus; a note without the flag is kept for later but neither counted nor found.
    pub fn rebuild(&mut self, plan: Vec<(Origin, bool)>, analyzer: &mut Analyzer) {
        let mut old: Vec<Option<Entry>> = std::mem::take(&mut self.entries)
            .into_iter()
            .map(Some)
            .collect();
        let mut entries = Vec::with_capacity(plan.len());
        let mut listed = Vec::new();
        for (origin, in_corpus) in plan {
            let entry = match origin {
                Origin::Kept(at) => {
                    let Some(entry) = old.get_mut(at).and_then(Option::take) else {
                        continue;
                    };
                    entry
                }
                Origin::Fresh(note) => self.analyze(note, analyzer),
            };
            if in_corpus {
                listed.push(entries.len());
            }
            entries.push(entry);
        }
        self.entries = entries;
        self.listed = listed;
        self.restat();
    }

    /// Computes the statistics of the listed notes.
    fn restat(&mut self) {
        let docs: Vec<&Doc> = self
            .listed
            .iter()
            .map(|&at| &self.entries[at].doc)
            .collect();
        let vocabulary = self.vocabulary.len();
        self.full = (!docs.is_empty()).then(|| Okapi::new(&docs, vocabulary));
        let derived: Vec<bool> = self
            .listed
            .iter()
            .map(|&at| self.entries[at].derived)
            .collect();
        self.source_index = (0..docs.len()).filter(|&i| !derived[i]).collect();
        self.sources = derived.iter().any(|&d| d).then(|| {
            let subset: Vec<&Doc> = self.source_index.iter().map(|&i| docs[i]).collect();
            Okapi::new(&subset, vocabulary)
        });
    }

    /// The notes of the corpus in list order.
    pub fn notes(&self) -> impl Iterator<Item = &NoteRec> {
        self.listed.iter().map(|&at| &self.entries[at].note)
    }

    /// Every note the index holds, in list order, also those left out of the corpus.
    pub fn entry_notes(&self) -> impl Iterator<Item = &NoteRec> {
        self.entries.iter().map(|entry| &entry.note)
    }

    /// The number of notes the index holds.
    #[must_use]
    pub fn entry_count(&self) -> usize {
        self.entries.len()
    }

    /// The note at a position of the full list.
    #[must_use]
    pub fn entry_note(&self, at: usize) -> Option<&NoteRec> {
        self.entries.get(at).map(|entry| &entry.note)
    }

    /// Adds [`TITLE_BOOST`] per stem of the query that the title has too.
    fn title_boosted(titles: &[&HashSet<String>], query: &HashSet<String>, scores: &mut [f64]) {
        if query.is_empty() {
            return;
        }
        for (score, title) in scores.iter_mut().zip(titles) {
            #[allow(clippy::cast_precision_loss)]
            // The overlap is at most the number of words of a title; the cast is exact.
            let bonus = TITLE_BOOST * title.intersection(query).count() as f64;
            *score += bonus;
        }
    }

    /// The score of every note of the corpus for `query`.
    pub fn scores(&self, analyzer: &mut Analyzer, query: &str) -> Vec<f64> {
        let Some(full) = &self.full else {
            return Vec::new();
        };
        let ids: Vec<u32> = analyzer
            .index_tokens(query)
            .iter()
            .filter_map(|token| self.vocabulary.get(token).copied())
            .collect();
        let query_stems: HashSet<String> = analyzer.stem_tokens(query).into_iter().collect();
        let all: Vec<&Doc> = self
            .listed
            .iter()
            .map(|&at| &self.entries[at].doc)
            .collect();
        let titles: Vec<&HashSet<String>> = self
            .listed
            .iter()
            .map(|&at| &self.entries[at].title_stems)
            .collect();
        let mut scores = full.scores(&all, &ids);
        Self::title_boosted(&titles, &query_stems, &mut scores);
        if let Some(sources) = &self.sources {
            let subset: Vec<&Doc> = self.source_index.iter().map(|&i| all[i]).collect();
            let source_titles: Vec<&HashSet<String>> =
                self.source_index.iter().map(|&i| titles[i]).collect();
            let mut own = sources.scores(&subset, &ids);
            Self::title_boosted(&source_titles, &query_stems, &mut own);
            for (&i, value) in self.source_index.iter().zip(own) {
                scores[i] = value;
            }
        }
        scores
    }

    /// The lexical list: `k` source hits and the derived pages between them, normalised to the
    /// top hit. The excerpt is left empty.
    pub fn hits(&self, analyzer: &mut Analyzer, query: &str, k: usize) -> Vec<SearchHit> {
        let scores = self.scores(analyzer, query);
        let mut order: Vec<usize> = (0..scores.len()).collect();
        order.sort_by(|&a, &b| scores[b].total_cmp(&scores[a]));
        let hits: Vec<SearchHit> = order
            .into_iter()
            .filter(|&i| scores[i] > 0.0)
            .map(|i| {
                let entry = &self.entries[self.listed[i]];
                let mut hit = SearchHit::new(&entry.note.rel, scores[i]);
                hit.title.clone_from(&entry.note.title);
                hit.matched = "text".to_string();
                hit.derived = entry.derived;
                hit
            })
            .collect();
        let mut hits = pool_trim(hits, k);
        for (i, hit) in hits.iter_mut().enumerate() {
            hit.bm25_rang = i + 1;
        }
        max_normalize(&mut hits);
        for hit in &mut hits {
            hit.bm25_norm = hit.score;
        }
        hits
    }
}
