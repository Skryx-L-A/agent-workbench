from gardener import config
from gardener.linking import (embed_notes, mention_candidates,
                              neighbor_candidates, run_linking)
from gardener.vault import VaultWriter, load_notes

from .conftest import FakeOllama, make_note


def _vectors(notes):
    # everything similar to everything (same direction) except distinct axes
    return {n.rel: [1.0, 0.1 * i] for i, n in enumerate(notes)}


def test_embed_cache_reuses_by_content_hash(tmp_vault, store):
    notes = load_notes(tmp_vault)
    client = FakeOllama()
    embed_notes(notes, store, client)
    first = len(client.embed_calls)
    assert first == len(notes)
    embed_notes(notes, store, client)          # unchanged: all cached
    assert len(client.embed_calls) == first
    notes[0].path.write_text(notes[0].text + "\nchanged")
    notes2 = load_notes(tmp_vault)
    embed_notes(notes2, store, client)         # only the changed note re-embedded
    assert len(client.embed_calls) == first + 1


def test_mention_candidates_finds_unlinked_title(tmp_vault):
    notes = load_notes(tmp_vault)
    pairs = {(a.title, b.title) for a, b, _ in mention_candidates(notes)}
    assert ("Alpha", "Beta") in pairs          # "Beta" mentioned, not linked
    # Gamma already links [[Alpha]] -> not a candidate
    assert ("Gamma", "Alpha") not in pairs


def test_neighbor_candidates_threshold():
    class N:
        def __init__(self, rel):
            self.rel = rel
    notes = [N("a"), N("b"), N("c")]
    vecs = {"a": [1, 0], "b": [1, 0.01], "c": [0, 1]}
    pairs = {tuple(sorted((x.rel, y.rel))) for x, y, _ in
             neighbor_candidates(notes, vecs, top_k=2, min_sim=0.9)}
    assert ("a", "b") in pairs
    assert ("a", "c") not in pairs


def test_run_linking_applies_bidirectional_and_blocklists(tmp_vault, store):
    notes = load_notes(tmp_vault)
    client = FakeOllama(verdicts=[
        {"link": True, "type": "relates-to", "placement": "relations", "reason": "yes"},
        {"link": False, "reason": "unrelated"},
        {"link": False, "reason": "unrelated"},
    ])
    writer = VaultWriter(tmp_vault)
    res = run_linking(notes, _vectors(notes), store, client, writer)
    assert len(res.added) == 1
    a_rel, b_rel, _ = res.added[0]
    a = next(n for n in load_notes(tmp_vault) if n.rel == a_rel)
    b = next(n for n in load_notes(tmp_vault) if n.rel == b_rel)
    assert b.title.lower() in a.links and a.title.lower() in b.links
    # rejected pairs are blocklisted persistently
    for x, y, _ in res.rejected:
        assert store.is_blocked(x, y, "link")
    # second run: blocked pairs not judged again
    calls_before = len(client.judge_calls)
    run_linking(load_notes(tmp_vault), _vectors(notes), store, client,
                VaultWriter(tmp_vault))
    assert len(client.judge_calls) == calls_before


def test_run_linking_respects_cap(tmp_vault, store, monkeypatch):
    # hub gets mentioned by 8 notes -> at most 5 new links for hub per run
    make_note(tmp_vault, "10-global/hub.md", "Hubnote", "Central topic.")
    for i in range(8):
        make_note(tmp_vault, f"10-global/spoke{i}.md", f"Spoke{i}",
                  "This spoke discusses Hubnote in detail.")
    notes = load_notes(tmp_vault)
    client = FakeOllama(verdicts=[
        {"link": True, "type": "relates-to", "placement": "relations"}
        for _ in range(100)])
    monkeypatch.setattr(config, "LINK_MIN_SIMILARITY", 2.0)  # neighbors off
    writer = VaultWriter(tmp_vault)
    res = run_linking(notes, {n.rel: [1.0, 0.0] for n in notes}, store, client, writer)
    hub_links = [p for p in res.added if "10-global/hub.md" in (p[0], p[1])]
    assert len(hub_links) == config.MAX_NEW_LINKS_PER_NOTE
    assert any(s.startswith("cap:") for s in res.skipped)


def test_run_linking_dry_run_touches_nothing(tmp_vault, store):
    notes = load_notes(tmp_vault)
    before = {n.rel: n.text for n in notes}
    client = FakeOllama(verdicts=[
        {"link": True, "type": "relates-to", "placement": "relations"}
        for _ in range(10)])
    writer = VaultWriter(tmp_vault, dry_run=True)
    res = run_linking(notes, _vectors(notes), store, client, writer)
    assert res.added
    after = {n.rel: n.text for n in load_notes(tmp_vault)}
    assert before == after

def test_inline_placement_never_touches_body(tmp_vault, store):
    # regression: run 1 corrupted frontmatter/paths/existing links via inline substitution
    make_note(tmp_vault, "10-global/src.md", "Sourcenote",
              "Path /home/x/Targetnote and link [[pre-Targetnote]] mention Targetnote.")
    make_note(tmp_vault, "10-global/dst.md", "Targetnote", "Target body.")
    notes = load_notes(tmp_vault)
    client = FakeOllama(verdicts=[
        {"link": True, "type": "relates-to", "placement": "inline"}
        for _ in range(20)])
    run_linking(notes, {n.rel: [1.0, 0.0] for n in notes}, store, client,
                VaultWriter(tmp_vault))
    src = next(n for n in load_notes(tmp_vault) if n.rel == "10-global/src.md")
    body = src.text.split("## Relations")[0]
    assert "[[Targetnote]]" not in body            # body untouched
    assert "/home/x/Targetnote" in src.text        # path intact
    assert "[[pre-Targetnote]]" in src.text        # existing link intact
    assert "- relates-to [[Targetnote]]" in src.text


def test_apply_relation_inserts_inside_existing_section(tmp_vault):
    # regression: relations were appended at EOF, landing below the
    # "Stand:" recency marker outside the Relations section
    from gardener.linking import _apply_relation
    text = ("---\ntitle: X\n---\n\nBody.\n\n## Relations\n"
            "- relates-to [[Old]]\n\nStand: 2026-07\n")
    out = _apply_relation(text, "depends-on", "New")
    section = out.split("## Relations\n")[1].split("\nStand:")[0]
    assert "- depends-on [[New]]" in section
    assert out.rstrip().endswith("Stand: 2026-07")
    # no section yet -> created at end
    out2 = _apply_relation("---\ntitle: Y\n---\n\nBody.\n", "relates-to", "Z")
    assert out2.endswith("## Relations\n- relates-to [[Z]]\n")


def test_failed_judge_skips_pair_without_blocklisting(tmp_vault, store):
    # regression: {} verdict (judge non-JSON/transient failure) must NOT be
    # treated as "no" and permanently blocklisted
    notes = load_notes(tmp_vault)
    client = FakeOllama(verdicts=[{} for _ in range(50)])
    res = run_linking(notes, _vectors(notes), store, client, VaultWriter(tmp_vault))
    assert not res.added and not res.rejected
    assert any(s.startswith("judge failed") for s in res.skipped)
    for a in notes:
        for b in notes:
            assert not store.is_blocked(a.rel, b.rel, "link")


def test_unsafe_title_pair_skipped(tmp_vault, store):
    # titles with wikilink syntax would produce broken [[...]] targets
    make_note(tmp_vault, "10-global/weird.md", "Weird [x]|y", "Alpha related text.")
    notes = load_notes(tmp_vault)
    client = FakeOllama(verdicts=[
        {"link": True, "type": "relates-to", "placement": "relations"}
        for _ in range(50)])
    run_linking(notes, {n.rel: [1.0, 0.0] for n in notes}, store, client,
                VaultWriter(tmp_vault))
    for n in load_notes(tmp_vault):
        assert "[[Weird" not in n.text


def test_embed_notes_respects_deadline(tmp_vault, store):
    class Expired:
        def expired(self):
            return True
    notes = load_notes(tmp_vault)
    client = FakeOllama()
    vectors = embed_notes(notes, store, client, deadline=Expired())
    assert vectors == {}                      # nothing embedded, no crash
    assert client.embed_calls == []


def test_same_title_pair_skipped(tmp_vault, store):
    # regression: [[overview]] links between projects resolve to self
    make_note(tmp_vault, "20-projects/p1/overview.md", "overview", "Project one overview.")
    make_note(tmp_vault, "20-projects/p2/overview.md", "overview", "Project two overview.")
    notes = load_notes(tmp_vault)
    client = FakeOllama(verdicts=[
        {"link": True, "type": "relates-to", "placement": "relations"}
        for _ in range(20)])
    run_linking(notes, {n.rel: [1.0, 0.0] for n in notes}, store, client,
                VaultWriter(tmp_vault))
    for n in load_notes(tmp_vault):
        if n.rel.endswith("overview.md"):
            assert "[[overview]]" not in n.text
