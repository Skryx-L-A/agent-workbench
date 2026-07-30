from pathlib import Path

import pytest

from braincli import stats
from gardener.vault import load_notes


def _write(vault: Path, rel: str, title: str, body: str = "") -> None:
    p = vault / rel
    p.parent.mkdir(parents=True, exist_ok=True)
    p.write_text(f"---\ntitle: {title}\ntype: note\n---\n\n{body}\n")


@pytest.fixture
def vault(tmp_path: Path) -> Path:
    _write(tmp_path, "10-global/a.md", "Note A", "links to [[Note B]]")
    _write(tmp_path, "10-global/b.md", "Note B", "no outgoing links")
    _write(tmp_path, "20-projects/proj/c.md", "Note C", "an isolated note")
    return tmp_path


def test_branch_of_top_level_dir():
    assert stats.branch_of("10-global/a.md") == "10-global"
    assert stats.branch_of("20-projects/proj/c.md") == "20-projects"


def test_branch_of_root_file():
    assert stats.branch_of("INDEX.md") == "(root)"


def test_count_wikilinks(vault: Path):
    notes = load_notes(vault)
    assert stats.count_wikilinks(notes) == 1  # only a.md -> b.md


def test_find_orphans_note_with_no_incoming_link(vault: Path):
    notes = load_notes(vault)
    orphans = stats.find_orphans(notes)
    # a.md is linked by nobody, b.md is linked by a.md, c.md is linked by nobody
    assert set(orphans) == {"10-global/a.md", "20-projects/proj/c.md"}


def test_find_orphans_all_linked(tmp_path: Path):
    _write(tmp_path, "x.md", "X", "see [[Y]]")
    _write(tmp_path, "y.md", "Y", "see [[X]]")
    notes = load_notes(tmp_path)
    assert stats.find_orphans(notes) == []


def test_count_assets(tmp_path: Path):
    (tmp_path / "10-global/_assets").mkdir(parents=True)
    (tmp_path / "10-global/_assets/file1.pdf").write_bytes(b"x")
    (tmp_path / "10-global/_assets/file2.png").write_bytes(b"x")
    (tmp_path / "10-global/note.md").write_text("---\ntitle: n\n---\n")
    assert stats.count_assets(tmp_path) == 2


def test_lfs_object_size_non_git_dir_returns_zero(tmp_path: Path):
    assert stats.lfs_object_size_bytes(tmp_path) == 0


def test_last_backup_time_none_when_missing(tmp_path: Path):
    assert stats.last_backup_time(tmp_path / "nonexistent") is None


def test_last_backup_time_finds_latest_bundle(tmp_path: Path):
    old = tmp_path / "2026-01" / "knowledge.bundle"
    new = tmp_path / "2026-02" / "knowledge.bundle"
    old.parent.mkdir(parents=True)
    new.parent.mkdir(parents=True)
    old.write_bytes(b"x")
    new.write_bytes(b"x")
    import os
    import time
    os.utime(old, (time.time() - 1000, time.time() - 1000))
    result = stats.last_backup_time(tmp_path)
    assert result is not None


# -- collect() corpus regression guard --------------------------------------
#
# 2026-07-28 bug: collect() used gardener.vault.load_notes() -- gardener's own
# LINKING corpus, which hard-excludes MOC.md/DECISIONS.md/review-queue.md "at
# any depth" for its own auto-link-suggestion machinery (gardener/topics.py).
# 30-topics/ consists ONLY of MOC.md files, so `brain stats` reported that
# entire branch as absent, undercounted notes_total and wikilinks_total
# vault-wide. Fixed by switching to braincli.vault.load_search_notes (the same
# corpus `brain search` uses). These tests fail again if collect() ever goes
# back to the link corpus for its primary notes_total/notes_per_branch/
# wikilinks_total/orphans.

def test_collect_includes_moc_md_and_its_branch(tmp_path: Path):
    _write(tmp_path, "10-global/a.md", "Note A", "links to [[thema MOC]]")
    _write(tmp_path, "30-topics/thema/MOC.md", "thema MOC", "curated hub text, links [[Note A]]")
    result = stats.collect(tmp_path)
    assert result["notes_total"] == 2
    assert result["notes_per_branch"].get("30-topics") == 1
    assert result["wikilinks_total"] == 2


def test_collect_excludes_decisions_md_and_review_queue(tmp_path: Path):
    _write(tmp_path, "10-global/a.md", "Note A", "no links")
    _write(tmp_path, "20-projects/proj/DECISIONS.md", "proj DECISIONS", "generated pointer")
    _write(tmp_path, "00-sources/review-queue.md", "review queue", "generated queue")
    result = stats.collect(tmp_path)
    # DECISIONS.md/review-queue.md stay excluded -- pure gardener-generated
    # aggregators, not real content (same call as braincli.search's corpus).
    assert result["notes_total"] == 1


def test_collect_reports_link_corpus_total_separately(tmp_path: Path):
    _write(tmp_path, "10-global/a.md", "Note A", "no links")
    _write(tmp_path, "20-projects/proj/MOC.md", "proj MOC", "hub text")
    result = stats.collect(tmp_path)
    assert result["notes_total"] == 2          # search corpus: MOC.md counted
    assert result["link_corpus_notes_total"] == 1  # gardener's own corpus: MOC.md excluded
    assert result["notes_total"] > result["link_corpus_notes_total"]
