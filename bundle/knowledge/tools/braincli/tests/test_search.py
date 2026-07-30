import json

from braincli.search import (SearchHit, _flag_contradictions, cosine, hook_snippet,
                             load_contradicted_rels, rank, rrf_fuse, tokenize)


def test_cosine_identical_vectors_is_one():
    assert cosine([1.0, 0.0], [1.0, 0.0]) == 1.0


def test_cosine_orthogonal_vectors_is_zero():
    assert cosine([1.0, 0.0], [0.0, 1.0]) == 0.0


def test_cosine_zero_vector_is_zero():
    assert cosine([0.0, 0.0], [1.0, 1.0]) == 0.0


def test_rank_orders_by_similarity_descending():
    query = [1.0, 0.0]
    entries = [
        ("far.md", [0.0, 1.0]),
        ("close.md", [0.9, 0.1]),
        ("exact.md", [1.0, 0.0]),
    ]
    hits = rank(query, entries, k=3)
    assert [h.rel for h in hits] == ["exact.md", "close.md", "far.md"]
    assert hits[0].score == 1.0


def test_rank_respects_k():
    query = [1.0, 0.0]
    entries = [(f"n{i}.md", [1.0 - i * 0.01, i * 0.01]) for i in range(10)]
    hits = rank(query, entries, k=3)
    assert len(hits) == 3


def test_rank_empty_entries():
    assert rank([1.0, 0.0], [], k=5) == []


def test_hook_snippet_prefers_session_hook_line():
    text = (
        "---\ntitle: x\ntype: note\n---\n\n"
        "Some intro line.\n\n"
        "Für künftige Sessions: remember this fact.\n"
    )
    assert hook_snippet(text) == "Für künftige Sessions: remember this fact."


def test_hook_snippet_falls_back_to_first_body_line():
    text = "---\ntitle: x\n---\n\n# Heading\n\nActual content line.\n"
    assert hook_snippet(text) == "Actual content line."


def test_hook_snippet_no_frontmatter():
    text = "Just a plain note.\nSecond line.\n"
    assert hook_snippet(text) == "Just a plain note."


def test_tokenize_lowercases_and_splits():
    assert tokenize("Härtung macOS-Setup!") == ["härtung", "macos", "setup"]


def test_tokenize_empty_string():
    assert tokenize("") == []


def test_rrf_fuse_ranks_hybrid_hit_above_single_source_hit():
    semantic = [SearchHit(rel="a.md", score=0.9), SearchHit(rel="b.md", score=0.8)]
    text = [SearchHit(rel="a.md", score=5.0), SearchHit(rel="c.md", score=4.0)]
    fused = rrf_fuse([("semantic", semantic), ("text", text)], k=5)
    assert fused[0].rel == "a.md"
    assert fused[0].match == "hybrid"


def test_rrf_fuse_tags_single_source_matches():
    semantic = [SearchHit(rel="a.md", score=0.9)]
    fused = rrf_fuse([("semantic", semantic), ("text", [])], k=5)
    assert fused[0].match == "semantic"


def test_rrf_fuse_respects_k():
    semantic = [SearchHit(rel=f"n{i}.md", score=1.0 - i * 0.01) for i in range(10)]
    fused = rrf_fuse([("semantic", semantic), ("text", [])], k=3)
    assert len(fused) == 3


def test_rrf_fuse_empty_rankings():
    assert rrf_fuse([("semantic", []), ("text", [])], k=5) == []


def test_rrf_fuse_carries_raw_cosine_from_semantic_source():
    semantic = [SearchHit(rel="a.md", score=0.9, cosine=0.42)]
    text = [SearchHit(rel="a.md", score=5.0, cosine=0.0),
            SearchHit(rel="b.md", score=3.0, cosine=0.0)]
    fused = rrf_fuse([("semantic", semantic), ("text", text)], k=5)
    a = next(h for h in fused if h.rel == "a.md")
    b = next(h for h in fused if h.rel == "b.md")
    assert a.cosine == 0.42
    assert b.cosine == 0.0


def test_rrf_fuse_score_normalize_does_not_touch_cosine():
    semantic = [SearchHit(rel="a.md", score=0.9, cosine=0.9),
                SearchHit(rel="b.md", score=0.1, cosine=0.1)]
    fused = rrf_fuse([("semantic", semantic), ("text", [])], k=5)
    top = fused[0]
    assert top.score == 1.0  # max-normalized
    assert top.cosine == 0.9  # raw cosine untouched by normalization


def test_search_hit_default_cosine_is_zero():
    assert SearchHit(rel="x.md", score=1.0).cosine == 0.0


def _write_contradictions(vault, findings: dict):
    path = vault / "_meta" / "state" / "contradictions.json"
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps({"findings": findings}), encoding="utf-8")


def test_load_contradicted_rels_includes_open_and_escalated(tmp_path):
    vault = tmp_path / "vault"
    _write_contradictions(vault, {
        "f1": {"status": "open", "note_a": {"rel": "a.md"}, "note_b": {"rel": "b.md"}},
        "f2": {"status": "escalated", "note_a": {"rel": "c.md"}, "note_b": {"rel": "d.md"}},
        "f3": {"status": "resolved", "note_a": {"rel": "e.md"}, "note_b": {"rel": "f.md"}},
    })
    rels = load_contradicted_rels(vault)
    assert rels == {"a.md", "b.md", "c.md", "d.md"}


def test_load_contradicted_rels_missing_file_returns_empty(tmp_path):
    assert load_contradicted_rels(tmp_path / "no-such-vault") == set()


def test_load_contradicted_rels_corrupt_file_returns_empty(tmp_path):
    vault = tmp_path / "vault"
    path = vault / "_meta" / "state" / "contradictions.json"
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text("not json", encoding="utf-8")
    assert load_contradicted_rels(vault) == set()


def test_flag_contradictions_marks_matching_hits(tmp_path):
    vault = tmp_path / "vault"
    _write_contradictions(vault, {
        "f1": {"status": "open", "note_a": {"rel": "a.md"}, "note_b": {"rel": "b.md"}},
    })
    hits = [SearchHit(rel="a.md", score=1.0), SearchHit(rel="z.md", score=0.5)]
    flagged = _flag_contradictions(hits, vault)
    assert flagged[0].contradiction is True
    assert flagged[1].contradiction is False
