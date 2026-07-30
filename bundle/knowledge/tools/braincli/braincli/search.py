"""Hybrid search over the vault: BM25 (lexical) + Gardener embeddings
(semantic), fused with Reciprocal Rank Fusion. Falls back to BM25-only
when Ollama is unreachable."""
from __future__ import annotations

import json
import math
import re
import subprocess
from dataclasses import dataclass
from pathlib import Path

from gardener import config
from gardener.ollama import OllamaClient, OllamaError
from gardener.store import Store
from gardener.vault import parse_note

from .vault import load_search_notes

SESSION_HOOK_RE_PREFIXES = ("Für künftige Sessions", "Fuer kuenftige Sessions")

# Reciprocal Rank Fusion constant (standard default from the RRF paper).
RRF_K = 60

TOKEN_RE = re.compile(r"[\w']+", re.UNICODE)

# MOC.md/STATUS.md retrieval fix (2026-07-28), measured in
# _meta/tools/eval/results/moc-root-cause.md + moc-fix-comparison.md. Root cause was
# structural: gardener excludes MOC.md from its own linking corpus
# (gardener/topics.py), and braincli's search reused that same walk, so MOC.md
# got zero BM25 signal and (except the 4 30-topics hubs gardener embeds for
# its own hub logic) zero embedding signal either -- see .vault.load_search_notes
# and .extra_embed for the corpus/embedding side of the fix. These two
# constants are the second half: even after MOC.md re-enters both rankers,
# Reciprocal Rank Fusion still structurally favors a mediocre hit-in-both-lists
# over a note that is the single strongest semantic match but appears in only
# one list. Tuned via a small grid (title 1-4, nav 0.10-0.30) on 28 real
# queries: TITLE_BOOST=2.0 (title/query token-overlap bonus, any note) and
# NAV_BOOST=0.15 (MOC.md/STATUS.md bonus, navigational-looking queries only)
# together took overall recall@1 0.571->0.750 and MRR 0.707->0.839 with no
# regression, and the MOC/STATUS query subset to recall@1/3/5/MRR=1.0. A third
# option (append linked-note titles to a hub's indexed text) measured
# flat-to-negative on its own and was dropped from the combination -- the
# vault's own MOC.md files already carry real prose, not just link lists.
TITLE_BOOST = 2.0
NAV_BOOST = 0.15
HUB_BASENAMES = {"MOC.md", "STATUS.md"}
NAV_TRIGGERS = (
    "wo ", "wo faengt", "wo fängt", "themen-hub", "themenhub", "stand von",
    "stand vom", "stand beim", "aktueller stand", "übersicht", "uebersicht",
    "einstieg", "welche entscheidungen", "entscheidungen zu", "entscheidungen gibt",
    " moc", "status von", "status vom",
)


@dataclass
class SearchHit:
    rel: str
    score: float
    title: str = ""
    snippet: str = ""
    match: str = ""  # "semantic" | "text" | "hybrid"
    cosine: float = 0.0  # raw cosine similarity, 0.0 if never seen by semantic_search
    contradiction: bool = False  # True if `brain contradict` has an open finding on this note


def cosine(a: list[float], b: list[float]) -> float:
    dot = sum(x * y for x, y in zip(a, b))
    na = math.sqrt(sum(x * x for x in a))
    nb = math.sqrt(sum(y * y for y in b))
    return dot / (na * nb) if na and nb else 0.0


def rank(query_vector: list[float], entries: list[tuple[str, list[float]]],
         k: int) -> list[SearchHit]:
    """Pure ranking: cosine-similarity top-k over (rel, vector) entries."""
    scored = [SearchHit(rel=rel, score=cosine(query_vector, vec))
              for rel, vec in entries]
    scored.sort(key=lambda h: h.score, reverse=True)
    return scored[:k]


def hook_snippet(text: str) -> str:
    """First line matching the vault's 'Für künftige Sessions' recall convention,
    else the first non-empty, non-frontmatter body line."""
    lines = text.splitlines()
    in_frontmatter = False
    body_start = 0
    for i, line in enumerate(lines):
        if i == 0 and line.strip() == "---":
            in_frontmatter = True
            continue
        if in_frontmatter:
            if line.strip() == "---":
                body_start = i + 1
                in_frontmatter = False
            continue
        body_start = i
        break
    for line in lines[body_start:]:
        s = line.strip()
        if s.startswith(SESSION_HOOK_RE_PREFIXES):
            return s
    for line in lines[body_start:]:
        s = line.strip()
        if s and not s.startswith("#"):
            return s
    return ""


def load_all_embeddings(store: Store) -> list[tuple[str, list[float]]]:
    rows = store.conn.execute("SELECT rel, vector FROM embeddings").fetchall()
    import json
    return [(rel, json.loads(vec)) for rel, vec in rows]


def load_merged_embeddings(store: Store) -> list[tuple[str, list[float]]]:
    """Gardener's own embeddings, topped up with braincli's supplementary
    MOC.md cache (see extra_embed.py) so project MOC.md notes -- which
    gardener never embeds -- get a semantic signal too. On a rel present in
    both (the 4 30-topics hubs gardener embeds for its own hub-membership
    logic), the supplementary store wins: it is guaranteed freshly recomputed
    against the current file content, gardener's may be stale."""
    from .extra_embed import EXTRA_DB_PATH

    merged: dict[str, list[float]] = dict(load_all_embeddings(store))
    if EXTRA_DB_PATH.exists():
        extra_store = Store(EXTRA_DB_PATH, read_only=True)
        try:
            merged.update(dict(load_all_embeddings(extra_store)))
        finally:
            extra_store.close()
    return list(merged.items())


def tokenize(text: str) -> list[str]:
    return TOKEN_RE.findall(text.lower())


def _max_normalize(hits: list[SearchHit]) -> list[SearchHit]:
    """Rescale scores to [0, 1] by dividing by the top hit's score, so callers
    (the auto-recall hook's score threshold) see a comparable range regardless
    of which ranker - or fusion - produced the list."""
    if not hits:
        return hits
    top = hits[0].score
    if top > 0:
        for h in hits:
            h.score = h.score / top
    return hits


def _title_boosted_scores(notes: list, query_tokens: set[str], scores) -> list[float]:
    """Adds TITLE_BOOST per query/title token overlap -- a note whose title
    literally names what's asked for should not have to out-lexical a longer,
    more verbose note on raw term frequency alone."""
    if not query_tokens:
        return list(scores)
    boosted = list(scores)
    for i, n in enumerate(notes):
        overlap = len(query_tokens & set(tokenize(n.title)))
        if overlap:
            boosted[i] += TITLE_BOOST * overlap
    return boosted


def bm25_rank(vault: Path, query: str, k: int) -> list[SearchHit]:
    """Lexical top-k over the whole corpus, built in-memory at query time
    (the vault is small enough - ~100 notes, single-digit ms - that a
    persistent index isn't worth the staleness risk). Title/query token
    overlap gets a fixed bonus on top of the raw BM25 score (TITLE_BOOST)."""
    from rank_bm25 import BM25Okapi

    notes = load_search_notes(vault)
    if not notes:
        return []
    corpus_tokens = [tokenize(n.text) for n in notes]
    bm25 = BM25Okapi(corpus_tokens)
    scores = _title_boosted_scores(notes, set(tokenize(query)), bm25.get_scores(tokenize(query)))
    ranked = sorted(zip(notes, scores), key=lambda p: p[1], reverse=True)
    hits = [SearchHit(rel=n.rel, score=float(s), title=n.title,
                       snippet=hook_snippet(n.text), match="text")
            for n, s in ranked[:k] if s > 0]
    return _max_normalize(hits)


def semantic_search(vault: Path, query: str, k: int) -> list[SearchHit]:
    store = Store(config.STATE_DIR / "gardener.db", read_only=True)
    try:
        entries = load_merged_embeddings(store)
    finally:
        store.close()
    if not entries:
        return []
    client = OllamaClient()
    qvec = client.embed(query)  # raises OllamaError if Ollama is down
    hits = rank(qvec, entries, k)
    for h in hits:
        h.match = "semantic"
        h.cosine = h.score  # score gets rescaled downstream (RRF/normalize); cosine stays raw
        note_path = vault / h.rel
        if note_path.exists():
            note = parse_note(vault, note_path)
            h.title = note.title
            h.snippet = hook_snippet(note.text)
    return hits


def rrf_fuse(rankings: list[tuple[str, list[SearchHit]]], k: int,
             rrf_k: int = RRF_K) -> list[SearchHit]:
    """Reciprocal Rank Fusion: combine several ranked hit lists (each tagged
    with a source label, e.g. "semantic"/"text") into one ranking. A note that
    scores well in *both* rankers - a hybrid match - naturally floats up
    without any hand-tuned weight between BM25 and cosine scores, which live
    on incomparable scales."""
    rrf_scores: dict[str, float] = {}
    best_hit: dict[str, SearchHit] = {}
    sources: dict[str, set[str]] = {}
    cosines: dict[str, float] = {}
    for label, hits in rankings:
        for i, h in enumerate(hits):
            rrf_scores[h.rel] = rrf_scores.get(h.rel, 0.0) + 1.0 / (rrf_k + i + 1)
            sources.setdefault(h.rel, set()).add(label)
            cosines[h.rel] = max(cosines.get(h.rel, 0.0), h.cosine)
            if h.rel not in best_hit or (h.title and not best_hit[h.rel].title):
                best_hit[h.rel] = h

    fused = []
    for rel, score in rrf_scores.items():
        h = best_hit[rel]
        labels = sources[rel]
        match = "hybrid" if len(labels) > 1 else next(iter(labels))
        fused.append(SearchHit(rel=rel, score=score, title=h.title,
                                snippet=h.snippet, match=match, cosine=cosines[rel]))
    fused.sort(key=lambda h: h.score, reverse=True)
    return _max_normalize(fused[:k])


def _is_navigational(query: str) -> bool:
    q = query.lower()
    return any(t in q for t in NAV_TRIGGERS)


def _apply_nav_boost(hits: list[SearchHit], query: str) -> list[SearchHit]:
    """Fixed post-fusion bonus for MOC.md/STATUS.md hits, only on
    navigational-looking queries ("Themen-Hub fuer X", "Stand von Y", ...).
    See the NAV_BOOST/TITLE_BOOST comment above for why RRF needs this."""
    if not _is_navigational(query) or not hits:
        return hits
    boosted = [
        SearchHit(rel=h.rel,
                  score=h.score + NAV_BOOST if Path(h.rel).name in HUB_BASENAMES else h.score,
                  title=h.title, snippet=h.snippet, match=h.match, cosine=h.cosine)
        for h in hits
    ]
    boosted.sort(key=lambda h: h.score, reverse=True)
    return boosted


def hybrid_search(vault: Path, query: str, k: int) -> list[SearchHit]:
    """BM25 + embedding search, fused with RRF, then a small structural boost
    for MOC.md/STATUS.md hits on navigational-looking queries. Raises
    OllamaError if Ollama is unreachable - callers fall back to bm25_rank alone."""
    pool = max(k * 4, 20)  # wider candidate pool per ranker before fusion
    semantic_hits = semantic_search(vault, query, pool)
    text_hits = bm25_rank(vault, query, pool)
    fused = rrf_fuse([("semantic", semantic_hits), ("text", text_hits)], pool)
    return _apply_nav_boost(fused, query)[:k]


def fulltext_fallback(vault: Path, query: str, k: int) -> list[str]:
    """rg full-text fallback, used only if BM25 itself can't run (e.g. the
    vault can't be walked at all)."""
    try:
        proc = subprocess.run(
            ["rg", "-i", "-l", "--glob", "!90-secrets/**", "--glob", "*.md",
             query, str(vault)],
            capture_output=True, text=True, timeout=30)
    except FileNotFoundError:
        return []
    lines = [line for line in proc.stdout.splitlines() if line.strip()]
    return lines[:k]


def load_contradicted_rels(vault: Path) -> set[str]:
    """Notes carrying an open (or escalated) `brain contradict` finding.

    Best-effort: a missing/corrupt/absent-tool findings file just means "no
    known contradictions" - search must never fail because this file doesn't
    exist yet on a vault where the contradict tool hasn't run.
    """
    path = vault / config.CONTRADICTIONS_FILE
    try:
        raw = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, ValueError):
        return set()
    findings = raw.get("findings", {}) if isinstance(raw, dict) else {}
    rels: set[str] = set()
    for f in findings.values():
        if not isinstance(f, dict) or f.get("status") not in ("open", "escalated"):
            continue
        for side in ("note_a", "note_b"):
            rel = (f.get(side) or {}).get("rel")
            if rel:
                rels.add(rel)
    return rels


def _flag_contradictions(hits: list[SearchHit], vault: Path) -> list[SearchHit]:
    flagged = load_contradicted_rels(vault)
    if not flagged:
        return hits
    for h in hits:
        if h.rel in flagged:
            h.contradiction = True
    return hits


def search(vault: Path, query: str, k: int = 5) -> tuple[list[SearchHit], bool]:
    """Returns (hits, used_fallback). used_fallback means Ollama was down and
    results are BM25-only (no semantic signal)."""
    try:
        hits = hybrid_search(vault, query, k)
        return _flag_contradictions(hits, vault), False
    except OllamaError:
        pass
    hits = bm25_rank(vault, query, k)
    if not hits:
        paths = fulltext_fallback(vault, query, k)
        hits = [SearchHit(rel=str(Path(p).relative_to(vault)), score=0.0, match="text")
                for p in paths]
    return _flag_contradictions(hits, vault), True
