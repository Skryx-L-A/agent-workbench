from __future__ import annotations

import datetime as dt

from gardener import config, synth
from gardener.contradict import ContradictionStore
from gardener.vault import VaultWriter, load_notes, parse_note

from .conftest import make_note

DEMO_VERDICT = {
    "intro": "Demo project builds feature X. [[A]]",
    "facts": [
        "Decision one was made. [[B]]",
        "Decision two was made. (Stand: 2026-07) [[C]]",
        "This cites a note that does not exist. [[Ghost]]",
        "This has no source at all.",
    ],
}


class FixedJudge:
    """Deterministic judge stand-in: always the same verdict, call-counted so
    idempotency (no regeneration when sources are unchanged) is testable."""

    def __init__(self, verdict: dict):
        self.verdict = verdict
        self.calls = 0

    def judge(self, system: str, prompt: str) -> dict:
        self.calls += 1
        return dict(self.verdict)


def make_demo_project(root):
    make_note(root, "20-projects/demo/a.md", "A", "Feature X kickoff notes.")
    make_note(root, "20-projects/demo/b.md", "B", "Decision one details.")
    make_note(root, "20-projects/demo/c.md", "C", "Decision two details.\n\nStand: 2026-07")
    make_note(root, "20-projects/demo/d.md", "D", "Supporting note.")


# -- filter_sourced_lines: the two hard-required drop cases -------------------

def test_line_without_wikilink_is_dropped():
    resolver = {"a": object()}
    kept, no_link, dead = synth.filter_sourced_lines(["a claim with no link"], resolver)
    assert kept == []
    assert no_link == 1
    assert dead == 0


def test_line_with_dead_wikilink_is_dropped():
    resolver = {"a": object()}
    kept, no_link, dead = synth.filter_sourced_lines(["a claim about [[Ghost]]"], resolver)
    assert kept == []
    assert no_link == 0
    assert dead == 1


def test_line_with_valid_wikilink_is_kept():
    resolver = {"a": object()}
    kept, no_link, dead = synth.filter_sourced_lines(["a claim about [[A]]"], resolver)
    assert kept == ["a claim about [[A]]"]
    assert no_link == 0 and dead == 0


def test_line_with_one_dead_and_one_live_link_is_dropped():
    # "every wikilink on the line must resolve" - one bad target voids the line
    resolver = {"a": object()}
    kept, no_link, dead = synth.filter_sourced_lines(
        ["mentions [[A]] and also [[Ghost]]"], resolver)
    assert kept == []
    assert dead == 1


# -- candidate discovery -------------------------------------------------------

def test_project_branch_is_a_candidate_with_folder_membership(tmp_path):
    root = tmp_path / "vault"
    make_demo_project(root)
    make_note(root, "20-projects/small/x.md", "X", "too small")
    make_note(root, "20-projects/small/y.md", "Y", "too small")
    notes = load_notes(root)
    candidates = synth.discover_candidates(root, notes, hubs=[], vectors={})
    by_name = {c.name: c for c in candidates}
    assert set(n.title for n in by_name["demo"].sources) == {"A", "B", "C", "D"}
    assert set(n.title for n in by_name["small"].sources) == {"X", "Y"}


def test_existing_hub_membership_by_embedding_similarity(tmp_path):
    root = tmp_path / "vault"
    make_note(root, "30-topics/widgets/MOC.md", "widgets MOC",
             "Hand-curated intro about widgets.")
    make_note(root, "10-global/close.md", "Close", "Close to widgets.")
    make_note(root, "10-global/far.md", "Far", "Unrelated content.")
    notes = load_notes(root)
    hub = parse_note(root, root / "30-topics" / "widgets" / "MOC.md")
    vectors = {hub.rel: [1.0, 0.0],
              "10-global/close.md": [0.99, 0.01],
              "10-global/far.md": [0.0, 1.0]}
    candidates = synth.discover_candidates(root, notes, hubs=[hub], vectors=vectors)
    widgets = next(c for c in candidates if c.name == "widgets")
    assert [n.title for n in widgets.sources] == ["Close"]


# -- the four hard rules, end to end via run_synth -----------------------------

def test_topic_below_min_sources_creates_no_page(tmp_path):
    root = tmp_path / "vault"
    make_note(root, "20-projects/small/x.md", "X", "too small")
    make_note(root, "20-projects/small/y.md", "Y", "too small")
    notes = load_notes(root)
    writer = VaultWriter(root)
    store = ContradictionStore(root / "_meta" / "state" / "contradictions.json")
    result = synth.run_synth(root, notes, [], {}, writer, FixedJudge(DEMO_VERDICT),
                             store, min_sources=4)
    assert result.written == []
    assert "small" in result.skipped_small
    assert not (root / "30-topics" / "small" / "MOC.md").exists()


def test_dry_run_writes_nothing_to_disk(tmp_path):
    root = tmp_path / "vault"
    make_demo_project(root)
    notes = load_notes(root)
    writer = VaultWriter(root, dry_run=True)
    store = ContradictionStore(root / "_meta" / "state" / "contradictions.json")
    result = synth.run_synth(root, notes, [], {}, writer, FixedJudge(DEMO_VERDICT),
                             store, min_sources=4)
    assert result.written == ["30-topics/demo/MOC.md"]   # planned, not written
    assert not (root / "30-topics" / "demo" / "MOC.md").exists()


def test_written_page_drops_ungrounded_lines_and_keeps_sourced_ones(tmp_path):
    root = tmp_path / "vault"
    make_demo_project(root)
    notes = load_notes(root)
    writer = VaultWriter(root)
    store = ContradictionStore(root / "_meta" / "state" / "contradictions.json")
    result = synth.run_synth(root, notes, [], {}, writer, FixedJudge(DEMO_VERDICT),
                             store, min_sources=4)
    page = (root / "30-topics" / "demo" / "MOC.md").read_text()
    assert "[[A]]" in page and "[[B]]" in page and "[[C]]" in page
    assert "Ghost" not in page
    assert "This has no source at all." not in page
    assert result.lines_dropped_dead_link == 1
    assert result.lines_dropped_no_link == 1
    assert "class: derived" in page
    assert "gardener-content-hash" in page
    assert "gardener-sources-hash" in page
    # every real source is listed, regardless of whether the judge cited it
    assert "[[D]]" in page.split("## Quellnotizen")[1]


def test_second_run_without_changes_is_idempotent(tmp_path):
    root = tmp_path / "vault"
    make_demo_project(root)
    notes = load_notes(root)
    writer = VaultWriter(root)
    store = ContradictionStore(root / "_meta" / "state" / "contradictions.json")
    judge = FixedJudge(DEMO_VERDICT)
    synth.run_synth(root, notes, [], {}, writer, judge, store, min_sources=4)
    first_text = (root / "30-topics" / "demo" / "MOC.md").read_text()
    calls_after_first = judge.calls

    result2 = synth.run_synth(root, load_notes(root), [], {}, writer, judge, store,
                              min_sources=4)
    second_text = (root / "30-topics" / "demo" / "MOC.md").read_text()
    assert second_text == first_text
    assert judge.calls == calls_after_first        # unchanged sources: no re-judge
    assert "demo" in result2.unchanged
    assert result2.written == []


def test_hand_edited_page_is_not_overwritten(tmp_path):
    root = tmp_path / "vault"
    make_demo_project(root)
    notes = load_notes(root)
    writer = VaultWriter(root)
    store = ContradictionStore(root / "_meta" / "state" / "contradictions.json")
    synth.run_synth(root, notes, [], {}, writer, FixedJudge(DEMO_VERDICT), store,
                    min_sources=4)
    page_path = root / "30-topics" / "demo" / "MOC.md"
    generated = page_path.read_text()
    hand_edited = generated + "\nA human added this sentence by hand.\n"
    page_path.write_text(hand_edited)

    result2 = synth.run_synth(root, load_notes(root), [], {}, writer,
                              FixedJudge(DEMO_VERDICT), store, min_sources=4)
    assert page_path.read_text() == hand_edited    # not clobbered
    assert "demo" in result2.skipped_hand_edited
    assert result2.written == []


def test_source_change_triggers_regeneration(tmp_path):
    root = tmp_path / "vault"
    make_demo_project(root)
    writer = VaultWriter(root)
    store = ContradictionStore(root / "_meta" / "state" / "contradictions.json")
    judge = FixedJudge(DEMO_VERDICT)
    synth.run_synth(root, load_notes(root), [], {}, writer, judge, store, min_sources=4)
    calls_after_first = judge.calls

    (root / "20-projects" / "demo" / "a.md").write_text(
        "---\ntitle: A\ntype: note\n---\n\nFeature X kickoff notes, revised.\n")
    result2 = synth.run_synth(root, load_notes(root), [], {}, writer, judge, store,
                              min_sources=4)
    assert judge.calls == calls_after_first + 1
    assert result2.written == ["30-topics/demo/MOC.md"]


def test_open_contradiction_is_surfaced_not_resolved(tmp_path):
    root = tmp_path / "vault"
    make_demo_project(root)
    notes = load_notes(root)
    b = next(n for n in notes if n.title == "B")
    c = next(n for n in notes if n.title == "C")
    store = ContradictionStore(root / "_meta" / "state" / "contradictions.json")
    finding = {
        "id": "abc123", "verdict": "contradiction", "confidence": 0.9,
        "why": "conflicting dates", "found": dt.datetime.now().isoformat(timespec="seconds"),
        "status": "open", "escalation_reason": None, "resolution": None,
        "note_a": {"rel": b.rel, "id": b.rel, "title": b.title, "quote": "Decision one details."},
        "note_b": {"rel": c.rel, "id": c.rel, "title": c.title, "quote": "Decision two details."},
    }
    store.upsert(finding)
    writer = VaultWriter(root)
    result = synth.run_synth(root, notes, [], {}, writer, FixedJudge(DEMO_VERDICT),
                             store, min_sources=4)
    page = (root / "30-topics" / "demo" / "MOC.md").read_text()
    assert "## Offene Widersprueche" in page
    assert "[[B]]" in page.split("## Offene Widersprueche")[1]
    assert "[[C]]" in page.split("## Offene Widersprueche")[1]
    assert result.written == ["30-topics/demo/MOC.md"]


def test_secrets_never_appear_as_a_source_or_in_output(tmp_path):
    root = tmp_path / "vault"
    make_demo_project(root)
    make_note(root, "90-secrets/leak.md", "API Key", "SECRET_TOKEN=do-not-leak")
    notes = load_notes(root)
    assert all(n.rel.split("/")[0] != "90-secrets" for n in notes)
    writer = VaultWriter(root)
    store = ContradictionStore(root / "_meta" / "state" / "contradictions.json")
    synth.run_synth(root, notes, [], {}, writer, FixedJudge(DEMO_VERDICT), store,
                    min_sources=4)
    page = (root / "30-topics" / "demo" / "MOC.md").read_text()
    assert "SECRET_TOKEN" not in page and "90-secrets" not in page


def test_only_topic_filter_restricts_to_one_candidate(tmp_path):
    root = tmp_path / "vault"
    make_demo_project(root)
    make_note(root, "20-projects/other/p.md", "P", "one")
    make_note(root, "20-projects/other/q.md", "Q", "two")
    make_note(root, "20-projects/other/r.md", "R", "three")
    make_note(root, "20-projects/other/s.md", "S", "four")
    notes = load_notes(root)
    writer = VaultWriter(root)
    store = ContradictionStore(root / "_meta" / "state" / "contradictions.json")
    result = synth.run_synth(root, notes, [], {}, writer, FixedJudge(DEMO_VERDICT),
                             store, min_sources=4, only_topic="demo")
    assert result.written == ["30-topics/demo/MOC.md"]
    assert not (root / "30-topics" / "other" / "MOC.md").exists()


def test_sources_hash_stable_regardless_of_order():
    class N:
        def __init__(self, rel, text):
            self.rel, self.text = rel, text
        @property
        def content_hash(self):
            import hashlib
            return hashlib.sha256(self.text.encode()).hexdigest()
    a, b = N("a.md", "x"), N("b.md", "y")
    assert synth.sources_hash([a, b]) == synth.sources_hash([b, a])
