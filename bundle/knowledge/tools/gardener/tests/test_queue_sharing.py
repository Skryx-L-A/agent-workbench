"""Die Review-Queue hat zwei Schreiber und muss beide ueberleben.

Der Gardener haengt unklare Faelle an (append-only), der Widerspruchs-Scanner
regeneriert seine Befunde vollstaendig - ein aufgeloester Widerspruch soll
verschwinden, ein offener Gardener-Fall nicht. Seit beide dieselbe Datei
benutzen (2026-07-29), ist das nur getrennt haltbar, wenn der regenerierende
Schreiber ausschliesslich seinen eigenen, markierten Abschnitt anfasst.
"""
from __future__ import annotations

import datetime as dt
from pathlib import Path

from gardener import config, contradict


def _finding(fid: str, quote_a: str = "A sagt X") -> dict:
    return {
        "id": fid,
        "status": "open",
        "verdict": "contradiction",
        "confidence": 0.9,
        "found": "2026-07-29T08:00:00",
        "note_a": {"rel": "10-global/a.md", "title": "A", "quote": quote_a, "id": "1"},
        "note_b": {"rel": "10-global/b.md", "title": "B", "quote": "B sagt nicht-X", "id": "2"},
    }


def test_gardener_entries_survive_a_contradiction_run(tmp_path: Path):
    queue = tmp_path / config.CONTRADICT_REVIEW_QUEUE
    queue.parent.mkdir(parents=True, exist_ok=True)
    queue.write_text(
        "# Review-Queue (Gardener)\n\nUnklare Faelle.\n\n"
        "- 2026-07-20: Waise ohne Verweis: 10-global/verwaist.md\n",
        encoding="utf-8")

    contradict.write_review_queue(tmp_path, [_finding("f1")], dry_run=False)

    text = queue.read_text(encoding="utf-8")
    assert "Waise ohne Verweis" in text, "der Gardener-Eintrag wurde ueberschrieben"
    assert "f1" in text or "A sagt X" in text


def test_second_run_replaces_only_its_own_section(tmp_path: Path):
    queue = tmp_path / config.CONTRADICT_REVIEW_QUEUE
    queue.parent.mkdir(parents=True, exist_ok=True)
    queue.write_text("# Review-Queue (Gardener)\n\n- 2026-07-20: Handeintrag\n",
                     encoding="utf-8")

    contradict.write_review_queue(tmp_path, [_finding("f1", "erster Befund")], dry_run=False)
    contradict.write_review_queue(tmp_path, [_finding("f2", "zweiter Befund")], dry_run=False)

    text = queue.read_text(encoding="utf-8")
    assert "Handeintrag" in text
    assert "zweiter Befund" in text
    assert "erster Befund" not in text, "alter Befund haengt noch drin statt ersetzt zu sein"
    assert text.count(contradict.QUEUE_SECTION_START) == 1, "Abschnitt verdoppelt"


def test_resolved_findings_disappear_but_the_file_stays(tmp_path: Path):
    queue = tmp_path / config.CONTRADICT_REVIEW_QUEUE
    queue.parent.mkdir(parents=True, exist_ok=True)
    queue.write_text("# Review-Queue (Gardener)\n\n- 2026-07-20: Handeintrag\n",
                     encoding="utf-8")

    contradict.write_review_queue(tmp_path, [_finding("f1")], dry_run=False)
    contradict.write_review_queue(tmp_path, [], dry_run=False)

    text = queue.read_text(encoding="utf-8")
    assert "Handeintrag" in text
    assert "keine offenen Widersprueche" in text


def test_dry_run_writes_nothing(tmp_path: Path):
    queue = tmp_path / config.CONTRADICT_REVIEW_QUEUE
    contradict.write_review_queue(tmp_path, [_finding("f1")], dry_run=True)
    assert not queue.exists()
