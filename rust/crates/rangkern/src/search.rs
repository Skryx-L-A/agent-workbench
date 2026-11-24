//! The search: hybrid ranking with the fixed vectors, or the lexical way without them, then the
//! validity, the contradiction mark, the space label and the space filter.
//!
//! Only notes of the search corpus are candidates: a vector for a path that is not a corpus note
//! counts for nothing, and the full-text fallback lists corpus notes only.

use std::path::Path;

use notiz_kern::text::strip;

use crate::bm25::LexKorpus;
use crate::embed::Embedder;
use crate::fulltext::fulltext_rels;
use crate::hit::{
    SearchHit, demote_derived, fuse_hybrid_with, hybrid_pool, pool_trim, semantic_ranking,
};
use crate::snippet::best_snippet_span;
use crate::token::Analyzer;
use crate::validity::{Lookup, apply_validity};
use crate::vault::{Manifest, Unreadable, VectorView, contradicted_rels, load_search_notes};

/// What a search needs besides the vault and the question.
#[derive(Clone, Copy, Debug)]
pub struct Params<'a> {
    /// How many hits to return.
    pub k: usize,
    /// The fixed vectors; without them the lexical way is taken.
    pub vectors: Option<VectorView<'a>>,
    /// A space or a comma list of spaces to keep.
    pub space: Option<&'a str>,
    /// The date for `valid-until`, `YYYY-MM-DD`.
    pub today: &'a str,
}

/// Sets the excerpt of `hit` from the text of its note.
fn fill_snippet(analyzer: &mut Analyzer, hit: &mut SearchHit, text: &str, query: &str) {
    let (snippet, lines) = best_snippet_span(analyzer, text, query);
    hit.snippet = snippet;
    hit.lines = lines;
}

/// What acts on the whole candidate list before the cut to `k`: the validity and the space filter.
type BeforeCut<'a> = &'a dyn Fn(&mut Vec<SearchHit>);

/// The hybrid list of the first `k` hits. `wide`: a filter follows, the lists are read in full.
fn hybrid(
    korpus: &LexKorpus,
    lookup: &Lookup<'_>,
    analyzer: &mut Analyzer,
    vectors: VectorView<'_>,
    (query, k, wide): (&str, usize, bool),
    before_cut: BeforeCut<'_>,
) -> Vec<SearchHit> {
    let pool = hybrid_pool(k, wide);
    let text_hits = korpus.hits(analyzer, query, pool);
    let entries: Vec<(&str, &[f64])> = vectors
        .notes
        .iter()
        .filter(|(rel, _)| lookup.note(rel).is_some())
        .map(|(rel, vector)| (rel.as_str(), vector.as_slice()))
        .collect();
    let semantic_hits = if entries.is_empty() {
        Vec::new()
    } else {
        let mut hits = semantic_ranking(vectors.query, entries, pool);
        for hit in &mut hits {
            if let Some(note) = lookup.note(&hit.rel) {
                hit.title.clone_from(&note.title);
                hit.derived = crate::notes::is_derived(&note.fields);
            }
        }
        pool_trim(hits, pool)
    };
    let mut hits = fuse_hybrid_with(&semantic_hits, &text_hits, query, k, |hits| {
        before_cut(hits);
    });
    for hit in &mut hits {
        if let Some(note) = lookup.note(&hit.rel) {
            fill_snippet(analyzer, hit, &note.text, query);
        }
    }
    hits
}

/// The lexical list, and the full-text fallback when it is empty. The depth is [`hybrid_pool`], not a
/// function of `k`, and the validity and the filter act on the whole list before the cut to `k`.
fn lexical(
    root: &Path,
    korpus: &LexKorpus,
    lookup: &Lookup<'_>,
    analyzer: &mut Analyzer,
    (query, k, wide): (&str, usize, bool),
    before_cut: BeforeCut<'_>,
) -> Vec<SearchHit> {
    let mut hits = korpus.hits(analyzer, query, hybrid_pool(k, wide));
    demote_derived(&mut hits);
    let nothing_found = hits.is_empty();
    before_cut(&mut hits);
    hits.truncate(k);
    for hit in &mut hits {
        if let Some(note) = lookup.note(&hit.rel) {
            fill_snippet(analyzer, hit, &note.text, query);
        }
    }
    if nothing_found {
        hits = fulltext_rels(root, korpus.notes(), query)
            .into_iter()
            .take(k)
            .map(|rel| {
                let mut hit = SearchHit::new(&rel, 0.0);
                hit.matched = "text".to_string();
                hit
            })
            .collect();
        before_cut(&mut hits);
        hits.truncate(k);
    }
    hits
}

/// Marks the hits with open contradictions.
pub(crate) fn flag_contradictions(hits: &mut [SearchHit], root: &Path) {
    let flagged = contradicted_rels(root);
    if flagged.is_empty() {
        return;
    }
    for hit in hits {
        if flagged.contains(&hit.rel) {
            hit.contradiction = true;
        }
    }
}

/// Keeps the hits of the listed spaces; `space` is one name or a comma list.
pub(crate) fn filter_space(hits: Vec<SearchHit>, space: Option<&str>) -> Vec<SearchHit> {
    let Some(space) = space.filter(|space| !space.is_empty()) else {
        return hits;
    };
    let allowed: Vec<&str> = space
        .split(',')
        .map(strip)
        .filter(|name| !name.is_empty())
        .collect();
    hits.into_iter()
        .filter(|hit| allowed.contains(&hit.space.as_str()))
        .collect()
}

/// Ranks the notes of an index for `query`: the part of the search after loading and indexing.
///
/// `root` is read again for the contradictions, the space manifest and the full-text fallback.
/// Returns the hits and whether the lexical way was taken (no vectors).
pub fn rank(
    root: &Path,
    korpus: &LexKorpus,
    analyzer: &mut Analyzer,
    query: &str,
    params: &Params<'_>,
) -> (Vec<SearchHit>, bool) {
    rank_with(
        root,
        korpus,
        &Lookup::new(korpus.notes()),
        analyzer,
        query,
        params,
    )
}

/// [`rank`] for a question that the process embeds itself: `params.vectors` is not used. The note
/// vectors are `notes`. Ollama is asked only if one of them belongs to a note of the corpus (a
/// vector of an excluded or vanished note is a relic and counts for nothing); then the question
/// goes to `embedder`. If both tries fail, the lexical way is taken, as when the model's own call
/// raises `OllamaError`. Without a note vector of the corpus the hybrid way runs with no semantic
/// list, and Ollama is not asked.
pub fn rank_embedding(
    root: &Path,
    korpus: &LexKorpus,
    analyzer: &mut Analyzer,
    query: &str,
    params: &Params<'_>,
    (embedder, notes): (&Embedder, &[(String, Vec<f64>)]),
) -> (Vec<SearchHit>, bool) {
    let lookup = Lookup::new(korpus.notes());
    let vector = if notes.iter().any(|(rel, _)| lookup.note(rel).is_some()) {
        embedder.embed(query)
    } else {
        Some(Vec::new())
    };
    let params = Params {
        vectors: vector.as_deref().map(|query| VectorView { query, notes }),
        ..*params
    };
    rank_with(root, korpus, &lookup, analyzer, query, &params)
}

fn rank_with(
    root: &Path,
    korpus: &LexKorpus,
    lookup: &Lookup<'_>,
    analyzer: &mut Analyzer,
    query: &str,
    params: &Params<'_>,
) -> (Vec<SearchHit>, bool) {
    let wide = params.space.is_some_and(|space| !space.is_empty());
    let manifest = Manifest::load(root);
    // Validity and space filter act on the whole candidate list, the cut to `k` follows (until 19.11.2026 the
    // list was cut to `max(k * 8, 40)` before the filter, which made the head depend on `k`).
    let before_cut = |hits: &mut Vec<SearchHit>| {
        apply_validity(hits, lookup, params.today);
        if wide {
            for hit in hits.iter_mut() {
                hit.space = manifest.space_for(&hit.rel).to_string();
            }
            *hits = filter_space(std::mem::take(hits), params.space);
        }
    };
    let (mut hits, fallback) = match params.vectors {
        Some(vectors) => (
            hybrid(
                korpus,
                lookup,
                analyzer,
                vectors,
                (query, params.k, wide),
                &before_cut,
            ),
            false,
        ),
        None => (
            lexical(
                root,
                korpus,
                lookup,
                analyzer,
                (query, params.k, wide),
                &before_cut,
            ),
            true,
        ),
    };
    for hit in &mut hits {
        if hit.matched == "nachfolger"
            && hit.snippet.is_empty()
            && let Some(note) = lookup.note(&hit.rel)
        {
            fill_snippet(analyzer, hit, &note.text, query);
        }
    }
    flag_contradictions(&mut hits, root);
    for hit in &mut hits {
        hit.space = manifest.space_for(&hit.rel).to_string();
    }
    let mut hits = filter_space(hits, params.space);
    hits.truncate(params.k);
    (hits, fallback)
}

/// [`search`] with a question that the process embeds itself ([`rank_embedding`]); `notes` are the
/// note vectors of the databases.
///
/// # Errors
///
/// [`Unreadable`] when the vault or a note of it cannot be read.
pub fn search_embedding(
    root: &Path,
    query: &str,
    params: &Params<'_>,
    embedding: (&Embedder, &[(String, Vec<f64>)]),
) -> Result<(Vec<SearchHit>, bool), Unreadable> {
    let notes = load_search_notes(root)?;
    let mut analyzer = Analyzer::new();
    let korpus = LexKorpus::new(notes, &mut analyzer);
    Ok(rank_embedding(
        root,
        &korpus,
        &mut analyzer,
        query,
        params,
        embedding,
    ))
}

/// The search of the vault at `root`: loads and indexes the notes, then ranks them.
///
/// Returns the hits and whether the lexical way was taken (no vectors).
///
/// # Errors
///
/// [`Unreadable`] when the vault or a note of it cannot be read.
pub fn search(
    root: &Path,
    query: &str,
    params: &Params<'_>,
) -> Result<(Vec<SearchHit>, bool), Unreadable> {
    let notes = load_search_notes(root)?;
    let mut analyzer = Analyzer::new();
    let korpus = LexKorpus::new(notes, &mut analyzer);
    Ok(rank(root, &korpus, &mut analyzer, query, params))
}
