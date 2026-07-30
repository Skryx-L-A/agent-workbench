"""Ein Waisen-Zaehler, der jede Session um eins waechst, ist kein Zaehler mehr.

Quellnotizen (00-sources/, sessions/) haben per Bauart keinen eingehenden Link -
sie werden gesucht, nicht verlinkt. Zusammen mit den Wissensnotizen gezaehlt,
verschwindet die eine Notiz, auf die wirklich niemand zeigt, im Rauschen.
"""
from __future__ import annotations

from pathlib import Path

from braincli import stats
from gardener.vault import parse_note


def _note(vault: Path, rel: str, klass: str | None, body: str = "") -> None:
    p = vault / rel
    p.parent.mkdir(parents=True, exist_ok=True)
    fm = f"---\ntitle: {Path(rel).stem}\n"
    if klass is not None:
        fm += f"class: {klass}\n"
    fm += "---\n\n"
    p.write_text(fm + body, encoding="utf-8")


def _load(vault: Path):
    return [parse_note(vault, p) for p in sorted(vault.rglob("*.md"))]


def test_source_orphans_do_not_hide_a_knowledge_orphan(tmp_path: Path):
    _note(tmp_path, "10-global/sessions/session-a.md", "source")
    _note(tmp_path, "00-sources/mined/roh.md", "source")
    _note(tmp_path, "10-global/niemand-zeigt-hierhin.md", "knowledge")
    _note(tmp_path, "10-global/verlinkt.md", "knowledge")
    _note(tmp_path, "10-global/zeiger.md", "knowledge", "siehe [[verlinkt]]")

    notes = _load(tmp_path)
    orphans = stats.find_orphans(notes)
    split = stats.split_orphans_by_layer(notes, orphans)

    assert split["knowledge"] == ["10-global/niemand-zeigt-hierhin.md",
                                  "10-global/zeiger.md"]
    assert sorted(split["source"]) == ["00-sources/mined/roh.md",
                                       "10-global/sessions/session-a.md"]


def test_a_note_without_class_counts_as_knowledge(tmp_path: Path):
    """Unklassifiziert heisst nachsehen, nicht wegsortieren."""
    _note(tmp_path, "10-global/ohne-klasse.md", None)

    notes = _load(tmp_path)
    split = stats.split_orphans_by_layer(notes, stats.find_orphans(notes))

    assert split["knowledge"] == ["10-global/ohne-klasse.md"]
    assert split["source"] == []


def test_derived_topic_pages_are_not_excused(tmp_path: Path):
    """Eine erzeugte Themenseite, auf die niemand zeigt, ist eine echte Luecke."""
    _note(tmp_path, "30-topics/thema/MOC.md", "derived")

    notes = _load(tmp_path)
    split = stats.split_orphans_by_layer(notes, stats.find_orphans(notes))

    assert split["knowledge"] == ["30-topics/thema/MOC.md"]


def test_a_note_the_catalog_names_is_not_an_orphan(tmp_path: Path):
    """INDEX.md ist die zweite Sprosse der Suchleiter, steht aber nicht im
    Notizkorpus - seine Verweise zaehlten deshalb fuer nichts."""
    _note(tmp_path, "10-global/nur-im-index.md", "knowledge")
    (tmp_path / "INDEX.md").write_text("# Index\n\n- [[nur-im-index]]\n", encoding="utf-8")

    result = stats.collect(tmp_path)

    assert result["orphans_knowledge"] == []


def test_the_review_queue_does_not_launder_an_orphan(tmp_path: Path):
    """Die Queue verlinkt Waisen, WEIL sie Waisen sind. Zaehlte sie als
    eingehender Verweis, loeschte sie genau den Befund aus, fuer den sie da ist."""
    _note(tmp_path, "10-global/wirklich-allein.md", "knowledge")
    (tmp_path / "review-queue.md").write_text(
        "- 2026-07-29: orphan [[wirklich-allein]] - kein Partner gefunden\n",
        encoding="utf-8")

    result = stats.collect(tmp_path)

    assert result["orphans_knowledge"] == ["10-global/wirklich-allein.md"]


def test_collect_reports_both_lists(tmp_path: Path):
    _note(tmp_path, "10-global/sessions/session-a.md", "source")
    _note(tmp_path, "10-global/allein.md", "knowledge")

    result = stats.collect(tmp_path)

    assert result["orphans_total"] == 2
    assert result["orphans_knowledge"] == ["10-global/allein.md"]
    assert result["orphans_source"] == ["10-global/sessions/session-a.md"]
