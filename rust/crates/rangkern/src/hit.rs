//! A search hit and the rank steps that work on hit lists: cosine list, reciprocal rank fusion,
//! navigation bonus, demotion of derived pages, and the pool rules.

use std::collections::HashMap;

use crate::pystr::{lower, py_sum};

/// The constant of the reciprocal rank fusion.
pub const RRF_K: usize = 15;
/// Bonus for `MOC.md` and `STATUS.md` hits on a navigational question.
pub const NAV_BOOST: f64 = 0.15;
/// A derived page keeps this share of its score after fusion.
pub const DERIVED_FACTOR: f64 = 0.5;
/// Extra semantic candidates read so that derived pages do not shrink the pool of source notes.
pub const DERIVED_POOL_EXTRA: usize = 50;
/// File names of the navigation pages.
const HUB_BASENAMES: [&str; 2] = ["MOC.md", "STATUS.md"];
/// Parts of a question that make it navigational.
const NAV_TRIGGERS: [&str; 18] = [
    "wo ",
    "wo faengt",
    "wo fängt",
    "themen-hub",
    "themenhub",
    "stand von",
    "stand vom",
    "stand beim",
    "aktueller stand",
    "übersicht",
    "uebersicht",
    "einstieg",
    "welche entscheidungen",
    "entscheidungen zu",
    "entscheidungen gibt",
    " moc",
    "status von",
    "status vom",
];

/// One hit of a ranked list.
#[derive(Clone, Debug, PartialEq)]
pub struct SearchHit {
    /// The vault-relative path.
    pub rel: String,
    /// The score; after normalisation the top hit has 1.0.
    pub score: f64,
    /// The title of the note, or empty.
    pub title: String,
    /// The excerpt, or empty.
    pub snippet: String,
    /// `semantic`, `text`, `hybrid` or `nachfolger`.
    pub matched: String,
    /// The raw cosine, 0.0 if the semantic list never saw the note.
    pub cosine: f64,
    /// The 1-based position in the lexical list, 0 if absent.
    pub bm25_rang: usize,
    /// The lexical score divided by the lexical top score.
    pub bm25_norm: f64,
    /// The 1-based position in the semantic list, 0 if absent.
    pub vektor_rang: usize,
    /// An open contradiction names this note.
    pub contradiction: bool,
    /// The space label; empty until it is set after the ranking.
    pub space: String,
    /// A `class: derived` page.
    pub derived: bool,
    /// First and last file line of the excerpt block.
    pub lines: Option<(usize, usize)>,
    /// `superseded`, `archived` or `expired` when the note is demoted, else empty.
    pub status: String,
    /// The `superseded-by` targets as written.
    pub superseded_by: Vec<String>,
    /// The resolved paths of the successors.
    pub superseded_by_rel: Vec<String>,
}

impl SearchHit {
    /// A hit with the given path and score; every other field has its default.
    #[must_use]
    pub fn new(rel: &str, score: f64) -> Self {
        Self {
            rel: rel.to_string(),
            score,
            title: String::new(),
            snippet: String::new(),
            matched: String::new(),
            cosine: 0.0,
            bm25_rang: 0,
            bm25_norm: 0.0,
            vektor_rang: 0,
            contradiction: false,
            space: String::new(),
            derived: false,
            lines: None,
            status: String::new(),
            superseded_by: Vec::new(),
            superseded_by_rel: Vec::new(),
        }
    }
}

/// Sorts by score, highest first; hits with the same score keep their order.
pub fn sort_by_score(hits: &mut [SearchHit]) {
    hits.sort_by(|a, b| b.score.total_cmp(&a.score));
}

/// Cosine similarity; 0.0 when one vector has length 0. A longer vector is cut to the shorter for
/// the dot product, as Python's `map(mul, a, b)` does, but each norm covers the whole vector.
#[must_use]
pub fn cosine(a: &[f64], b: &[f64]) -> f64 {
    let dot = py_sum(a.iter().zip(b).map(|(x, y)| x * y));
    let na = py_sum(a.iter().map(|x| x * x)).sqrt();
    let nb = py_sum(b.iter().map(|x| x * x)).sqrt();
    if na != 0.0 && nb != 0.0 {
        dot / (na * nb)
    } else {
        0.0
    }
}

/// Divides every score by the score of the first hit when that is positive.
pub fn max_normalize(hits: &mut [SearchHit]) {
    let Some(top) = hits.first().map(|hit| hit.score) else {
        return;
    };
    if top > 0.0 {
        for hit in hits {
            hit.score /= top;
        }
    }
}

/// The first `k` source hits plus the derived pages ranked among them.
#[must_use]
pub fn pool_trim(hits: Vec<SearchHit>, k: usize) -> Vec<SearchHit> {
    let mut out = Vec::new();
    let mut sources = 0;
    for hit in hits {
        if sources >= k {
            break;
        }
        if !hit.derived {
            sources += 1;
        }
        out.push(hit);
    }
    out
}

/// The cosine list: the top `k` plus 50 candidates of the notes with a vector, best first.
#[must_use]
pub fn semantic_ranking<'a>(
    query: &[f64],
    entries: impl IntoIterator<Item = (&'a str, &'a [f64])>,
    k: usize,
) -> Vec<SearchHit> {
    let mut scored: Vec<SearchHit> = entries
        .into_iter()
        .map(|(rel, vector)| {
            let mut hit = SearchHit::new(rel, cosine(query, vector));
            hit.matched = "semantic".to_string();
            hit
        })
        .collect();
    sort_by_score(&mut scored);
    scored.truncate(k.saturating_add(DERIVED_POOL_EXTRA));
    for (i, hit) in scored.iter_mut().enumerate() {
        hit.cosine = hit.score;
        hit.vektor_rang = i + 1;
    }
    scored
}

/// What the fusion collects for one path.
struct Fused {
    score: f64,
    title: String,
    snippet: String,
    space: String,
    lines: Option<(usize, usize)>,
    text_list: bool,
    semantic_list: bool,
    cosine: f64,
    bm25: Option<(usize, f64)>,
    vector_rank: Option<usize>,
    derived: bool,
}

/// Reciprocal rank fusion of the semantic and the lexical list into the top `k`, normalised.
/// A derived page takes no rank slot from the source notes of its list.
#[must_use]
pub fn rrf_fuse(semantic: &[SearchHit], text: &[SearchHit], k: usize) -> Vec<SearchHit> {
    let mut order: Vec<String> = Vec::new();
    let mut by_rel: HashMap<String, Fused> = HashMap::new();
    for (is_text, hits) in [(false, semantic), (true, text)] {
        let mut slot = 0;
        for (i, hit) in hits.iter().enumerate() {
            let pos = if hit.derived { i } else { slot };
            if !hit.derived {
                slot += 1;
            }
            let entry = by_rel.entry(hit.rel.clone()).or_insert_with(|| {
                order.push(hit.rel.clone());
                Fused {
                    score: 0.0,
                    title: hit.title.clone(),
                    snippet: hit.snippet.clone(),
                    space: hit.space.clone(),
                    lines: hit.lines,
                    text_list: false,
                    semantic_list: false,
                    cosine: 0.0,
                    bm25: None,
                    vector_rank: None,
                    derived: false,
                }
            });
            #[allow(clippy::cast_precision_loss)]
            // A rank of a candidate list is far below 2^52; the cast is exact.
            let step = 1.0 / ((RRF_K + pos + 1) as f64);
            entry.score += step;
            if is_text {
                entry.text_list = true;
                entry.bm25 = Some((
                    if hit.bm25_rang == 0 {
                        i + 1
                    } else {
                        hit.bm25_rang
                    },
                    hit.bm25_norm,
                ));
            } else {
                entry.semantic_list = true;
                entry.vector_rank = Some(if hit.vektor_rang == 0 {
                    i + 1
                } else {
                    hit.vektor_rang
                });
            }
            entry.cosine = entry.cosine.max(hit.cosine);
            entry.derived |= hit.derived;
            if entry.title.is_empty() && !hit.title.is_empty() {
                entry.title.clone_from(&hit.title);
                entry.snippet.clone_from(&hit.snippet);
                entry.space.clone_from(&hit.space);
                entry.lines = hit.lines;
            }
        }
    }
    let mut fused: Vec<SearchHit> = order
        .into_iter()
        .filter_map(|rel| {
            let entry = by_rel.remove(&rel)?;
            let mut hit = SearchHit::new(&rel, entry.score);
            hit.title = entry.title;
            hit.snippet = entry.snippet;
            hit.space = entry.space;
            hit.lines = entry.lines;
            hit.matched = match (entry.semantic_list, entry.text_list) {
                (true, true) => "hybrid",
                (true, false) => "semantic",
                _ => "text",
            }
            .to_string();
            hit.cosine = entry.cosine;
            hit.derived = entry.derived;
            if let Some((rang, norm)) = entry.bm25 {
                hit.bm25_rang = rang;
                hit.bm25_norm = norm;
            }
            hit.vektor_rang = entry.vector_rank.unwrap_or(0);
            Some(hit)
        })
        .collect();
    sort_by_score(&mut fused);
    fused.truncate(k);
    max_normalize(&mut fused);
    fused
}

/// Whether the question looks for an entry point (`Stand von`, `Übersicht`, ...).
#[must_use]
pub fn is_navigational(query: &str) -> bool {
    let lowered = lower(query);
    NAV_TRIGGERS.iter().any(|trigger| lowered.contains(trigger))
}

/// The last part of a vault-relative path.
fn file_name(rel: &str) -> &str {
    rel.rsplit('/').next().unwrap_or(rel)
}

/// A fixed bonus for `MOC.md` and `STATUS.md` hits, only on a navigational question.
pub fn apply_nav_boost(hits: &mut [SearchHit], query: &str) {
    if hits.is_empty() || !is_navigational(query) {
        return;
    }
    for hit in hits.iter_mut() {
        if HUB_BASENAMES.contains(&file_name(&hit.rel)) {
            hit.score += NAV_BOOST;
        }
    }
    sort_by_score(hits);
}

/// Multiplies the score of every derived page by [`DERIVED_FACTOR`], sorts and normalises again.
pub fn demote_derived(hits: &mut [SearchHit]) {
    if !hits.iter().any(|hit| hit.derived) {
        return;
    }
    for hit in hits.iter_mut() {
        if hit.derived {
            hit.score *= DERIVED_FACTOR;
        }
    }
    sort_by_score(hits);
    max_normalize(hits);
}

/// The candidates per list (BM25 and vector) before the fusion. Fixed, not dependent on `k`: until
/// 19.11.2026 it was `max(k * 4, 20)` (the first hybrid commit of the model, with no measurement), which made
/// the head of the list depend on `k`. 40 by the rule of the measurement of the candidate depth: the
/// smallest depth of 40, 80, 200 and all whose top 5 on the dev half is at most one question below the best.
pub const HYBRID_POOL: usize = 40;

/// A search with a filter (space) reads the lists in full, so that a small space is not left with few hits.
pub const HYBRID_POOL_WIDE: usize = usize::MAX;

/// The candidates per list for a result of `k` hits: [`HYBRID_POOL`] (or all with a filter), the same for every `k`.
#[must_use]
pub fn hybrid_pool(_k: usize, wide: bool) -> usize {
    if wide { HYBRID_POOL_WIDE } else { HYBRID_POOL }
}

/// The hybrid ranking: fusion, navigation bonus, demotion of derived pages, the first `k`.
#[must_use]
pub fn fuse_hybrid(
    semantic: &[SearchHit],
    text: &[SearchHit],
    query: &str,
    k: usize,
) -> Vec<SearchHit> {
    fuse_hybrid_with(semantic, text, query, k, |_| {})
}

/// [`fuse_hybrid`] with `validity` applied to the whole fused list before the cut to `k`, so that the
/// list for a larger `k` always extends the list for a smaller one. The fusion is not cut before the
/// bonuses and demotions either.
#[must_use]
pub fn fuse_hybrid_with(
    semantic: &[SearchHit],
    text: &[SearchHit],
    query: &str,
    k: usize,
    validity: impl FnOnce(&mut Vec<SearchHit>),
) -> Vec<SearchHit> {
    let mut hits = rrf_fuse(semantic, text, usize::MAX);
    apply_nav_boost(&mut hits, query);
    demote_derived(&mut hits);
    validity(&mut hits);
    hits.truncate(k);
    hits
}
