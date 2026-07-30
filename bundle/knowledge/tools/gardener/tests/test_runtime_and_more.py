import datetime as dt
import os
import subprocess
import time

import pytest

from gardener import config
from gardener.audit import dead_links, run_audit, stale_claims
from gardener.consolidate import run_consolidation
from gardener.maintain import find_orphans, run_maintenance, update_mocs
from gardener.runtime import Lock, LockHeldError
from gardener.store import Store, pair_key
from gardener.vault import VaultWriter, load_notes

from .conftest import FakeOllama


# -- lock ---------------------------------------------------------------

def test_lock_blocks_fresh_and_takes_over_stale(tmp_path):
    lock_path = tmp_path / "g.lock"
    l1 = Lock(lock_path)
    l1.acquire()
    with pytest.raises(LockHeldError):
        Lock(lock_path).acquire()
    l1.release()
    assert not lock_path.exists()
    # dead holder PID -> takeover, even if young
    dead = subprocess.Popen(["true"])
    dead.wait()
    lock_path.write_text(f"{dead.pid} now\n")
    l2 = Lock(lock_path)
    l2.acquire()
    assert lock_path.read_text().split()[0] == str(os.getpid())
    l2.release()
    # unparsable lock content: falls back to age check
    lock_path.write_text("garbage\n")
    old = time.time() - config.LOCK_STALE_SECONDS - 60
    os.utime(lock_path, (old, old))
    l3 = Lock(lock_path)
    l3.acquire()
    l3.release()


def test_lock_live_holder_kept_past_stale_age(tmp_path):
    # regression: 45-min runs outlive the 15-min stale threshold; a live
    # holder must NOT lose its lock to a second invocation
    lock_path = tmp_path / "g.lock"
    l1 = Lock(lock_path)
    l1.acquire()
    old = time.time() - config.LOCK_STALE_SECONDS - 60
    os.utime(lock_path, (old, old))
    with pytest.raises(LockHeldError):
        Lock(lock_path).acquire()
    l1.release()


def test_lock_release_only_removes_own_lock(tmp_path):
    lock_path = tmp_path / "g.lock"
    lock_path.write_text("999999999 other\n")
    Lock(lock_path).release()
    assert lock_path.exists()


# -- store --------------------------------------------------------------

def test_blocklist_is_order_independent(store):
    assert pair_key("b", "a") == pair_key("a", "b")
    store.block("x.md", "y.md", "link", "no")
    assert store.is_blocked("y.md", "x.md", "link")
    assert not store.is_blocked("y.md", "x.md", "merge")


# -- consolidation ------------------------------------------------------

def test_consolidation_merges_clear_and_queues_unclear(tmp_vault, store):
    notes = load_notes(tmp_vault)
    vecs = {n.rel: [1.0, 0.0] for n in notes}  # everything sim=1.0 -> candidates
    n_pairs = len(notes) * (len(notes) - 1) // 2
    merged_body = "---\ntitle: Alpha\n---\n\nmerged content with everything kept\n"
    verdicts = [{"action": "merge", "confidence": 0.95,
                 "merged_markdown": merged_body}]
    verdicts += [{"action": "review", "confidence": 0.4, "reason": "unsure"}
                 for _ in range(n_pairs)]
    client = FakeOllama(verdicts=verdicts)
    writer = VaultWriter(tmp_vault)
    res = run_consolidation(notes, vecs, store, client, writer)
    assert len(res.merged) == 1
    a_rel, b_rel = res.merged[0]
    assert "merged content" in (tmp_vault / a_rel).read_text()
    assert "Merged into" in (tmp_vault / b_rel).read_text()
    assert res.queued
    queue = (tmp_vault / "review-queue.md").read_text()
    assert "unsure" in queue
    # queued pairs are blocked so they are not re-judged next run
    x, y, _ = res.queued[0]
    assert store.is_blocked(x, y, "merge")


def test_consolidation_skips_session_notes(tmp_vault, store):
    p = tmp_vault / "10-global" / "sess.md"
    p.write_text("---\ntitle: Sess\ntype: session\n---\n\nSession archive.\n")
    notes = load_notes(tmp_vault)
    vecs = {n.rel: [1.0, 0.0] for n in notes}
    client = FakeOllama(verdicts=[
        {"action": "merge", "confidence": 0.99,
         "merged_markdown": "x" * 500} for _ in range(50)])
    res = run_consolidation(notes, vecs, store, client, VaultWriter(tmp_vault))
    assert not any("sess.md" in pair for m in res.merged for pair in m)
    assert "Session archive." in p.read_text()


def test_consolidation_skips_failed_judge_without_blocking(tmp_vault, store):
    notes = load_notes(tmp_vault)
    vecs = {n.rel: [1.0, 0.0] for n in notes}
    client = FakeOllama(verdicts=[{} for _ in range(50)])  # judge always fails
    res = run_consolidation(notes, vecs, store, client, VaultWriter(tmp_vault))
    assert not res.merged and not res.queued and not res.distinct
    assert not (tmp_vault / "review-queue.md").exists()
    assert not store.is_blocked(notes[0].rel, notes[1].rel, "merge")


def test_consolidation_garbage_confidence_does_not_crash(tmp_vault, store):
    notes = load_notes(tmp_vault)
    vecs = {n.rel: [1.0, 0.0] for n in notes}
    client = FakeOllama(verdicts=[
        {"action": "merge", "confidence": "high", "merged_markdown": "x" * 500}
        for _ in range(50)])
    res = run_consolidation(notes, vecs, store, client, VaultWriter(tmp_vault))
    assert not res.merged                      # unparsable confidence -> 0.0


def test_merge_rejected_when_merged_text_too_short(tmp_vault, store):
    # guard is against the LARGER note: half the smaller one is data loss
    notes = load_notes(tmp_vault)
    a, b = notes[0], notes[1]
    b.text = b.text + "long unique content " * 50
    b.path.write_text(b.text)
    vecs = {n.rel: [1.0, 0.0] for n in notes}
    short = "---\ntitle: x\n---\n\nshort\n" + "y" * (len(a.text) // 2)
    assert len(short) >= 0.5 * min(len(a.text), len(b.text))  # old guard passed
    client = FakeOllama(verdicts=(
        [{"action": "merge", "confidence": 0.99, "merged_markdown": short}]
        + [{"action": "distinct", "confidence": 0.99} for _ in range(50)]))
    res = run_consolidation(notes, vecs, store, client, VaultWriter(tmp_vault))
    assert (a.rel, b.rel) not in res.merged


def test_maintenance_after_merge_keeps_merged_text(tmp_vault, store):
    # regression: stale Note.text let add_recency_markers resurrect pre-merge
    # content, silently undoing the merge
    from gardener.maintain import run_maintenance as run_maint
    notes = load_notes(tmp_vault)
    vecs = {n.rel: [1.0, 0.0] for n in notes}
    merged_body = ("---\ntitle: Alpha\ntype: note\n---\n\n"
                   "merged content with everything kept, long enough to pass "
                   "the size guard easily against both source notes\n")
    verdicts = [{"action": "merge", "confidence": 0.95,
                 "merged_markdown": merged_body}]
    verdicts += [{"action": "distinct", "confidence": 0.99} for _ in range(50)]
    client = FakeOllama(verdicts=verdicts)
    writer = VaultWriter(tmp_vault)
    res = run_consolidation(notes, vecs, store, client, writer)
    assert len(res.merged) == 1
    a_rel, b_rel = res.merged[0]
    run_maint(notes, writer, client=None)
    assert "merged content" in (tmp_vault / a_rel).read_text()
    assert "Merged into" in (tmp_vault / b_rel).read_text()
    # pre-merge body must NOT have come back
    assert "talks about Beta" not in (tmp_vault / a_rel).read_text()


# -- maintenance --------------------------------------------------------

def test_find_orphans_and_moc_update(tmp_vault):
    notes = load_notes(tmp_vault)
    orphans = {n.title for n in find_orphans(notes)}
    assert "Beta" in orphans           # no links in or out
    assert "Alpha" not in orphans      # Gamma links to it
    writer = VaultWriter(tmp_vault)
    updated = update_mocs(notes, writer)
    assert updated == ["20-projects/demo/MOC.md"]
    moc = (tmp_vault / "20-projects/demo/MOC.md").read_text()
    assert "[[Gamma]]" in moc


def test_maintenance_adds_recency_markers_and_hot(tmp_vault):
    writer = VaultWriter(tmp_vault)
    notes = load_notes(tmp_vault)
    res = run_maintenance(notes, writer, client=None)
    alpha = (tmp_vault / "10-global/alpha.md").read_text()
    assert "Stand: " in alpha
    # Gamma already has a marker -> untouched
    assert "10-global/alpha.md" in res.markers_added
    assert "20-projects/demo/gamma.md" not in res.markers_added
    assert (tmp_vault / "HOT.md").exists()


def test_maintenance_never_touches_secrets(tmp_vault):
    secret = tmp_vault / "90-secrets" / "secret.md"
    before = secret.read_text()
    writer = VaultWriter(tmp_vault)
    notes = load_notes(tmp_vault)
    run_maintenance(notes, writer, client=None)
    assert secret.read_text() == before
    assert not any("90-secrets" in w for w in writer.written)


# -- audit ---------------------------------------------------------------

def test_audit_finds_dead_links_and_stale_claims(tmp_vault):
    gamma = tmp_vault / "20-projects/demo/gamma.md"
    gamma.write_text(gamma.read_text() + "\nSee [[Nonexistent Note]].\n")
    notes = load_notes(tmp_vault)
    dead = dead_links(notes)
    assert ("20-projects/demo/gamma.md", "nonexistent note") in dead
    stale = stale_claims(notes, months=6, today=dt.date(2027, 6, 1))
    assert ("20-projects/demo/gamma.md", "2026-06") in stale
    writer = VaultWriter(tmp_vault)
    today = dt.date(2026, 7, 12)
    rel = run_audit(notes, writer, today=today)
    wk = today.isocalendar()
    assert rel == f"00-sources/brain-health-{wk.year}-{wk.week:02d}.md"
    assert (tmp_vault / rel).exists()

def test_recency_markers_skip_session_and_report(tmp_vault):
    from gardener.maintain import add_recency_markers
    from gardener.vault import VaultWriter, load_notes
    from .conftest import make_note
    p = tmp_vault / "10-global" / "sess.md"
    p.parent.mkdir(parents=True, exist_ok=True)
    p.write_text("---\ntitle: Sess\ntype: session\n---\n\nimmutable archive\n")
    make_note(tmp_vault, "10-global/plain.md", "Plain", "themen-note body")
    notes = load_notes(tmp_vault)
    added = add_recency_markers(notes, VaultWriter(tmp_vault))
    sess = p.read_text()
    assert "Stand:" not in sess
    assert "10-global/plain.md" in added
