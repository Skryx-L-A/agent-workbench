"""CLI-Verdrahtung von `brain contradict --resolve`.

`resolve_finding()` selbst ist in der Gardener-Suite abgedeckt. Hier geht es um
den Weg dorthin: das Kommando zweigt VOR jedem Scan ab (kein Modellaufruf, keine
Nachbarsuche), und es weist zwei Faelle ab, die im Protokoll Schaden anrichten
wuerden - eine Aufloesung ohne Begruendung und eine id, die es nicht gibt.
"""
from __future__ import annotations

import json
from pathlib import Path

import pytest

from braincli import cli


def _vault(tmp_path: Path) -> Path:
    (tmp_path / "_meta" / "state").mkdir(parents=True)
    (tmp_path / "10-global").mkdir()
    return tmp_path


def _run(argv: list[str]) -> int:
    parser = cli.build_parser()
    args = parser.parse_args(argv)
    return args.func(args)


def test_resolve_without_why_is_refused(tmp_path: Path, capsys):
    """Ein Befund, der ohne Begruendung als erledigt gilt, ist im Protokoll
    wertlos - der naechste Leser weiss nicht, warum eine Seite gewonnen hat."""
    v = _vault(tmp_path)
    rc = _run(["--vault", str(v), "contradict", "--resolve", "abc123"])
    assert rc == 2
    assert "--why" in capsys.readouterr().err


def test_resolve_unknown_id_is_refused(tmp_path: Path, capsys):
    v = _vault(tmp_path)
    (v / "_meta" / "state" / "contradictions.json").write_text(
        json.dumps({"findings": {}}), encoding="utf-8")
    rc = _run(["--vault", str(v), "contradict", "--resolve", "gibtsnicht",
               "--why", "geprueft"])
    assert rc == 1
    assert "gibtsnicht" in capsys.readouterr().err


def test_resolve_dry_run_leaves_the_store_untouched(tmp_path: Path):
    """Ohne --write darf eine Aufloesung nichts festschreiben - sonst kann man
    sie nicht gefahrlos vorher ansehen."""
    v = _vault(tmp_path)
    store_file = v / "_meta" / "state" / "contradictions.json"
    finding = {
        "id": "f1", "status": "open", "confidence": 0.9, "verdict": "contradiction",
        "found": "2026-07-29T08:00:00",
        "note_a": {"rel": "10-global/a.md", "title": "A", "quote": "x", "id": "1"},
        "note_b": {"rel": "10-global/b.md", "title": "B", "quote": "y", "id": "2"},
    }
    store_file.write_text(json.dumps({"findings": {"f1": finding}}), encoding="utf-8")
    before = store_file.read_text(encoding="utf-8")

    rc = _run(["--vault", str(v), "contradict", "--resolve", "f1",
               "--why", "Messung schlaegt Absicht", "--rule", "1"])

    assert rc == 0
    assert store_file.read_text(encoding="utf-8") == before


def test_resolve_with_write_records_who_and_why(tmp_path: Path):
    v = _vault(tmp_path)
    store_file = v / "_meta" / "state" / "contradictions.json"
    finding = {
        "id": "f1", "status": "open", "confidence": 0.9, "verdict": "contradiction",
        "found": "2026-07-29T08:00:00",
        "note_a": {"rel": "10-global/a.md", "title": "A", "quote": "x", "id": "1"},
        "note_b": {"rel": "10-global/b.md", "title": "B", "quote": "y", "id": "2"},
    }
    store_file.write_text(json.dumps({"findings": {"f1": finding}}), encoding="utf-8")
    (v / "10-global" / "a.md").write_text("# A\nx\n", encoding="utf-8")
    (v / "10-global" / "b.md").write_text("# B\ny\n", encoding="utf-8")

    rc = _run(["--vault", str(v), "contradict", "--resolve", "f1", "--by", "orchestrator",
               "--why", "Messung schlaegt Absicht", "--rule", "1", "--write"])

    assert rc == 0
    saved = json.loads(store_file.read_text(encoding="utf-8"))["findings"]["f1"]
    assert saved["status"] == "resolved"
    res = saved.get("resolution") or {}
    assert res.get("by") == "orchestrator"
    assert "Messung" in (res.get("why") or "")


def _finding(fid: str = "abc123") -> dict:
    return {
        "id": fid, "status": "open", "verdict": "contradiction", "confidence": 0.9,
        "found": "2026-07-29T08:00:00",
        "note_a": {"rel": "10-global/a.md", "title": "A", "quote": "A sagt X", "id": "1"},
        "note_b": {"rel": "10-global/b.md", "title": "B", "quote": "B sagt nicht-X", "id": "2"},
    }


def _store(vault: Path, fid: str = "abc123") -> None:
    (vault / "_meta" / "state" / "contradictions.json").write_text(
        json.dumps({"findings": {fid: _finding(fid)}}), encoding="utf-8")


def test_resolving_also_takes_the_finding_out_of_the_queue(tmp_path: Path):
    """Sonst steht ein aufgeloester Befund fuer immer in der Liste.

    Gemessen 2026-07-29: der Marker in den Notizen sagte `resolved`, die
    Review-Queue meldete denselben Befund unveraendert als offen, weil
    `--resolve` sie nicht mitschrieb.
    """
    v = _vault(tmp_path)
    _store(v)
    for rel in ("10-global/a.md", "10-global/b.md"):
        (v / rel).write_text("---\ntitle: X\n---\n\nText.\n", encoding="utf-8")
    queue = v / "review-queue.md"
    queue.write_text("# Review-Queue\n\n- 2026-07-20: Handeintrag vom Gardener\n",
                     encoding="utf-8")

    rc = _run(["--vault", str(v), "contradict", "--resolve", "abc123",
               "--by", "der Nutzer", "--why", "kein Widerspruch", "--write"])

    assert rc == 0
    text = queue.read_text(encoding="utf-8")
    assert "abc123" not in text and "A sagt X" not in text
    assert "Handeintrag vom Gardener" in text, "Fremdeintrag wurde mitgeloescht"


def test_a_dry_run_resolve_leaves_the_queue_alone(tmp_path: Path):
    v = _vault(tmp_path)
    _store(v)
    for rel in ("10-global/a.md", "10-global/b.md"):
        (v / rel).write_text("---\ntitle: X\n---\n\nText.\n", encoding="utf-8")
    queue = v / "review-queue.md"
    queue.write_text("# Review-Queue\n\n- 2026-07-20: Handeintrag\n", encoding="utf-8")
    before = queue.read_text(encoding="utf-8")

    rc = _run(["--vault", str(v), "contradict", "--resolve", "abc123",
               "--by", "der Nutzer", "--why", "kein Widerspruch"])

    assert rc == 0
    assert queue.read_text(encoding="utf-8") == before
