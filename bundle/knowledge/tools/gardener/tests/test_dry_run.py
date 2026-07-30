"""The dry-run contract: a dry-run must not change ANYTHING - not the vault,
not the sqlite state, not git. Regression for the 2026-07-12 suspicion that
`brain gardener run --dry-run` still wrote to the vault."""
from __future__ import annotations

import argparse
import hashlib
from pathlib import Path

import pytest

from gardener import cli, config
from gardener.store import Store

from .conftest import FakeOllama


def snapshot(root: Path) -> dict[str, str]:
    out = {}
    for p in sorted(root.rglob("*")):
        if p.is_file():
            out[str(p.relative_to(root))] = hashlib.sha256(p.read_bytes()).hexdigest()
    return out


@pytest.fixture
def isolated(tmp_path, monkeypatch):
    """Point every piece of gardener state at tmp_path, and stub out Ollama+git."""
    state = tmp_path / "state"
    logs = tmp_path / "logs"
    transcripts = tmp_path / "transcripts"
    transcripts.mkdir()
    state.mkdir()
    logs.mkdir()
    monkeypatch.setattr(config, "STATE_DIR", state)
    monkeypatch.setattr(config, "LOG_DIR", logs)
    monkeypatch.setattr(config, "TRANSCRIPT_DIR", transcripts)
    client = FakeOllama(verdicts=[])
    monkeypatch.setattr(cli, "OllamaClient", lambda *a, **k: client)
    commits: list[str] = []

    def fake_commit(vault, msg, dry_run=False):
        if not dry_run:
            commits.append(msg)
        return not dry_run

    monkeypatch.setattr(cli, "git_commit", fake_commit)
    return state, logs, transcripts, commits


def args_for(vault: Path, phase: str = "all", dry_run: bool = True):
    return argparse.Namespace(vault=str(vault), phase=phase, dry_run=dry_run,
                              once=True, audit=False, verbose=False,
                              topic=None, min_notes=None)


@pytest.mark.parametrize("phase", list(config.PHASES))
def test_dry_run_writes_nothing_in_any_phase(tmp_vault, isolated, phase):
    state, logs, transcripts, commits = isolated
    (transcripts / "p").mkdir()
    (transcripts / "p" / "s.jsonl").write_text(
        '{"message": {"role": "user", "content": "' + "x" * 600 + '"}}\n')
    drop = tmp_vault / config.DROP_DIR
    drop.mkdir(parents=True)
    (drop / "contract.txt").write_text("some dropped text file")

    before = snapshot(tmp_vault)
    rc = cli.run(args_for(tmp_vault, phase=phase, dry_run=True))
    assert rc == 0
    assert snapshot(tmp_vault) == before, f"dry-run wrote to the vault in phase {phase}"
    assert commits == []                      # git_commit called with dry_run -> stubbed away

    # sqlite state must be untouched as well: a dry-run may not poison the
    # blocklist or the embedding cache of the next real run
    store = Store(state / "gardener.db")
    try:
        rows = store.conn.execute("SELECT count(*) FROM embeddings").fetchone()[0]
        blocked = store.conn.execute("SELECT count(*) FROM blocklist").fetchone()[0]
        mined = store.conn.execute("SELECT count(*) FROM mined").fetchone()[0]
    finally:
        store.close()
    assert (rows, blocked, mined) == (0, 0, 0)


def test_dry_run_report_goes_to_logdir_not_vault(tmp_vault, isolated, capsys):
    import datetime as dt
    _state, logs, _t, _c = isolated
    rc = cli.run(args_for(tmp_vault))
    assert rc == 0
    reports = list(logs.glob("gardener-report-*.md"))
    assert len(reports) == 1
    assert "DRY-RUN" in reports[0].read_text()
    today = dt.date.today().isoformat()
    assert not (tmp_vault / "00-sources" / f"gardener-report-{today}.md").exists()
    assert "DRY-RUN" in capsys.readouterr().out


def test_real_run_writes_and_commits(tmp_vault, isolated):
    state, _logs, _t, commits = isolated
    rc = cli.run(args_for(tmp_vault, dry_run=False))
    assert rc == 0
    assert (tmp_vault / "HOT.md").exists()
    assert list((tmp_vault / "00-sources").glob("gardener-report-*.md"))
    assert len(commits) == 2                  # pre-run snapshot + result commit
    store = Store(state / "gardener.db")
    try:
        rows = store.conn.execute("SELECT count(*) FROM embeddings").fetchone()[0]
    finally:
        store.close()
    assert rows > 0


def test_phase_selection_runs_only_that_phase(tmp_vault, isolated):
    rc = cli.run(args_for(tmp_vault, phase="lint", dry_run=False))
    assert rc == 0
    # lint writes the health report, but never HOT.md (that is maintenance)
    assert list((tmp_vault / "00-sources").glob("brain-health-*.md"))
    assert not (tmp_vault / "HOT.md").exists()
