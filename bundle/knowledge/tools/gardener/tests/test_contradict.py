from __future__ import annotations

import datetime as dt
import re

from gardener import contradict
from gardener.vault import VaultWriter, load_notes

from .conftest import make_note

REL_RE = re.compile(r"Note [AB]: .*? \(([^)]+)\)")


class PairJudge:
    """Deterministic judge stub keyed by the unordered (rel_a, rel_b) pair -
    content-aware unlike conftest.FakeOllama's call-order queue, which this
    module's neighbor scan (order depends on cosine, not test intent) would
    make brittle. No real Ollama call anywhere in this file."""

    def __init__(self, verdicts: dict[frozenset, dict], default: dict | None = None):
        self.verdicts = verdicts
        self.default = default or {"verdict": "compatible", "confidence": 1.0,
                                   "claim_a": "", "claim_b": "", "why": ""}
        self.calls: list[tuple[str, ...]] = []

    def judge(self, system: str, prompt: str) -> dict:
        rels = tuple(REL_RE.findall(prompt))
        self.calls.append(rels)
        return dict(self.verdicts.get(frozenset(rels), self.default))


def _pair(root, rel_a, title_a, body_a, rel_b, title_b, body_b):
    make_note(root, rel_a, title_a, body_a)
    make_note(root, rel_b, title_b, body_b)
    notes = load_notes(root)
    a = next(n for n in notes if n.rel == rel_a)
    b = next(n for n in notes if n.rel == rel_b)
    vectors = {a.rel: [1.0, 0.0], b.rel: [1.0, 0.01]}
    return notes, a, b, vectors


def test_detected_contradiction_is_recorded(tmp_path):
    root = tmp_path / "vault"
    notes, a, b, vectors = _pair(
        root, "10-global/a.md", "Ziel", "Ziel: Rollout bis Ende Juli.",
        "10-global/b.md", "Status", "Rollout lief noch nicht, Stand 2026-07-20.")
    client = PairJudge({frozenset({a.rel, b.rel}): {
        "verdict": "contradiction", "confidence": 0.9,
        "claim_a": "Rollout bis Ende Juli.", "claim_b": "Rollout lief noch nicht, Stand 2026-07-20.",
        "why": "Zieldatum vs. tatsaechlicher Stand weichen ab.",
    }})
    store = contradict.ContradictionStore(root / "_meta" / "state" / "contradictions.json")
    result = contradict.run_contradict([a], notes, vectors, client, store)
    assert len(result.findings) == 1
    f = result.findings[0]
    assert f["status"] == "open"
    assert {f["note_a"]["rel"], f["note_b"]["rel"]} == {a.rel, b.rel}
    assert f["confidence"] == 0.9


def test_compatible_pair_creates_nothing(tmp_path):
    root = tmp_path / "vault"
    notes, a, b, vectors = _pair(
        root, "10-global/a.md", "A", "Some content about topic X.",
        "10-global/b.md", "B", "Some other content about topic Y.")
    client = PairJudge({frozenset({a.rel, b.rel}): {
        "verdict": "compatible", "confidence": 0.95, "claim_a": "", "claim_b": "", "why": "unrelated",
    }})
    store = contradict.ContradictionStore(root / "_meta" / "state" / "contradictions.json")
    result = contradict.run_contradict([a], notes, vectors, client, store)
    assert result.findings == []
    assert result.compatible == 1


def test_low_confidence_creates_nothing(tmp_path):
    root = tmp_path / "vault"
    notes, a, b, vectors = _pair(
        root, "10-global/a.md", "A", "Value is 5.",
        "10-global/b.md", "B", "Value is 7.")
    client = PairJudge({frozenset({a.rel, b.rel}): {
        "verdict": "contradiction", "confidence": 0.4,
        "claim_a": "Value is 5.", "claim_b": "Value is 7.", "why": "differing values",
    }})
    store = contradict.ContradictionStore(root / "_meta" / "state" / "contradictions.json")
    result = contradict.run_contradict([a], notes, vectors, client, store)
    assert result.findings == []
    assert result.below_threshold == 1


def test_hallucinated_quote_is_discarded(tmp_path):
    root = tmp_path / "vault"
    notes, a, b, vectors = _pair(
        root, "10-global/a.md", "A", "The real sentence in note A.",
        "10-global/b.md", "B", "The real sentence in note B.")
    client = PairJudge({frozenset({a.rel, b.rel}): {
        "verdict": "contradiction", "confidence": 0.9,
        "claim_a": "This quote does not appear in A at all.",
        "claim_b": "The real sentence in note B.", "why": "made up",
    }})
    store = contradict.ContradictionStore(root / "_meta" / "state" / "contradictions.json")
    result = contradict.run_contradict([a], notes, vectors, client, store)
    assert result.findings == []
    assert result.hallucinated == 1


def test_second_run_does_not_duplicate_marker_block(tmp_path):
    root = tmp_path / "vault"
    notes, a, b, vectors = _pair(
        root, "10-global/a.md", "A", "Claim in A.",
        "10-global/b.md", "B", "Claim in B.")
    verdict = {"verdict": "contradiction", "confidence": 0.9,
              "claim_a": "Claim in A.", "claim_b": "Claim in B.", "why": "opposite"}
    finding = contradict.build_finding(a, b, verdict, 0.9,
                                       today=dt.datetime(2026, 7, 29, 5, 0))
    writer = VaultWriter(root)
    assert contradict.apply_markers(writer, a, b, finding)
    # Vergleich gegen die PLATTE, nicht gegen den Speicherstand von vor dem
    # Schreiben: der Schreiber stempelt einer Notiz ohne `id` beim Anlegen eine
    # auf (identity.py), und das ist gewollt - der Test soll den zweiten Lauf
    # pruefen, nicht das Stempeln.
    first_a_text = (root / a.rel).read_text(encoding="utf-8")
    # run again with the SAME finding (e.g. a second scan rediscovering it)
    a2 = next(n for n in load_notes(root) if n.rel == a.rel)
    b2 = next(n for n in load_notes(root) if n.rel == b.rel)
    assert contradict.apply_markers(writer, a2, b2, finding)
    assert a2.text.count(f"<!-- contradiction:{finding['id']}") == 1
    assert (root / a.rel).read_text(encoding="utf-8") == first_a_text


def test_write_flag_off_touches_no_vault_file(tmp_path):
    root = tmp_path / "vault"
    notes, a, b, vectors = _pair(
        root, "10-global/a.md", "A", "Claim in A.",
        "10-global/b.md", "B", "Claim in B.")
    before_a, before_b = a.text, b.text
    verdict = {"verdict": "contradiction", "confidence": 0.9,
              "claim_a": "Claim in A.", "claim_b": "Claim in B.", "why": "opposite"}
    finding = contradict.build_finding(a, b, verdict, 0.9)
    dry_writer = VaultWriter(root, dry_run=True)
    contradict.apply_markers(dry_writer, a, b, finding)
    store = contradict.ContradictionStore(root / "_meta" / "state" / "contradictions.json")
    store.upsert(finding)
    store.save(dry_run=True)
    contradict.write_review_queue(root, [finding], dry_run=True)
    contradict.save_last_run(root, dt.datetime.now(), dry_run=True)

    reloaded_a = next(n for n in load_notes(root) if n.rel == a.rel)
    reloaded_b = next(n for n in load_notes(root) if n.rel == b.rel)
    assert reloaded_a.text == before_a
    assert reloaded_b.text == before_b
    assert not (root / "_meta" / "state" / "contradictions.json").exists()
    assert not (root / "review-queue.md").exists()
    assert not (root / "_meta" / "tools" / "state" / "contradict.json").exists()


def test_escalation_rule_fires_for_money_legal_content(tmp_path):
    root = tmp_path / "vault"
    notes, a, b, vectors = _pair(
        root, "10-global/a.md", "Vertrag-Note", "Vertrag sieht 500 EUR Miete vor.",
        "10-global/b.md", "Vertrag-Status", "Miete wurde auf 600 EUR angepasst.")
    client = PairJudge({frozenset({a.rel, b.rel}): {
        "verdict": "contradiction", "confidence": 0.9,
        "claim_a": "Vertrag sieht 500 EUR Miete vor.", "claim_b": "Miete wurde auf 600 EUR angepasst.",
        "why": "Betrag weicht ab.",
    }})
    store = contradict.ContradictionStore(root / "_meta" / "state" / "contradictions.json")
    result = contradict.run_contradict([a], notes, vectors, client, store)
    assert len(result.findings) == 1
    assert result.findings[0]["status"] == "escalated"
    assert result.findings[0]["escalation_reason"]


def test_resolve_finding_sets_status_and_who_when_why(tmp_path):
    root = tmp_path / "vault"
    notes, a, b, vectors = _pair(
        root, "10-global/a.md", "A", "Old claim.",
        "10-global/b.md", "B", "New claim.")
    verdict = {"verdict": "contradiction", "confidence": 0.9,
              "claim_a": "Old claim.", "claim_b": "New claim.", "why": "differ"}
    finding = contradict.build_finding(a, b, verdict, 0.9)
    store = contradict.ContradictionStore(root / "_meta" / "state" / "contradictions.json")
    store.upsert(finding)
    resolved = contradict.resolve_finding(
        store, finding["id"], by="der Nutzer", why="B is the current measured state",
        rule="Messung schlaegt Absicht", vault=root, writer=VaultWriter(root))
    assert resolved["status"] == "resolved"
    assert resolved["resolution"]["by"] == "der Nutzer"
    assert resolved["resolution"]["why"] == "B is the current measured state"
    assert resolved["resolution"]["rule"] == "Messung schlaegt Absicht"
    assert resolved["resolution"]["at"]
    # marker blocks updated to reflect the resolution, not left saying "open"
    reloaded_a = next(n for n in load_notes(root) if n.rel == a.rel)
    assert "status=resolved" in reloaded_a.text
    assert "Messung schlaegt Absicht" in reloaded_a.text


def test_upsert_preserves_resolved_status_on_rediscovery(tmp_path):
    root = tmp_path / "vault"
    store = contradict.ContradictionStore(root / "_meta" / "state" / "contradictions.json")
    verdict = {"verdict": "contradiction", "confidence": 0.8,
              "claim_a": "x", "claim_b": "y", "why": "z"}

    class DummyNote:
        def __init__(self, rel, title):
            self.rel, self.title, self.fm, self.text = rel, title, {}, f"{title} body"
    a, b = DummyNote("a.md", "A"), DummyNote("b.md", "B")
    finding = contradict.build_finding(a, b, verdict, 0.8)
    store.upsert(finding)
    store.data[finding["id"]]["status"] = "resolved"
    store.data[finding["id"]]["resolution"] = {"by": "x", "why": "y", "rule": "z", "at": "now"}
    rediscovered = contradict.build_finding(a, b, verdict, 0.85)
    result = store.upsert(rediscovered)
    assert result["status"] == "resolved"
    assert result["confidence"] == 0.85  # observational fields still refresh


def test_finding_id_is_order_independent():
    id1 = contradict.finding_id("ulid-a", "quote a", "ulid-b", "quote b")
    id2 = contradict.finding_id("ulid-b", "quote b", "ulid-a", "quote a")
    assert id1 == id2


def test_verify_quote_rejects_missing_and_empty():
    assert contradict._verify_quote("hello world", "say hello world now")
    assert not contradict._verify_quote("not there", "say hello world now")
    assert not contradict._verify_quote("", "say hello world now")
    assert not contradict._verify_quote(None, "say hello world now")


def test_changed_since_filters_by_mtime(tmp_path):
    root = tmp_path / "vault"
    make_note(root, "10-global/old.md", "Old", "old body")
    notes = load_notes(root)
    future_cutoff = dt.datetime.now() + dt.timedelta(days=1)
    assert contradict.changed_since(notes, future_cutoff) == []
    past_cutoff = dt.datetime.now() - dt.timedelta(days=1)
    assert contradict.changed_since(notes, past_cutoff) == notes
