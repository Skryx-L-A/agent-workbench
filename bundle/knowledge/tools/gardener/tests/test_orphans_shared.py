"""Waisen-Definition: EINE, geteilt von Gardener und `brain stats`.

Es gab zwei. Der Gardener sah die Links aus den `MOC.md`-Hubs nicht (die sind aus
SEINEM Korpus ausgeschlossen) und stellte deshalb Notizen in die Queue, auf die
eine Hub-Seite zeigt - gemessen 2026-07-29 an macos-steam-civ6-damaged-fix.md.
"""
from __future__ import annotations

from pathlib import Path

from gardener import maintain, orphans
from gardener.vault import VaultWriter, parse_note


def _note(vault: Path, rel: str, title: str, klass: str, body: str = "") -> None:
    p = vault / rel
    p.parent.mkdir(parents=True, exist_ok=True)
    p.write_text(f"---\ntitle: {title}\nclass: {klass}\n---\n\n{body}\n", encoding="utf-8")


def _load(vault: Path, *rels: str):
    return [parse_note(vault, vault / r) for r in rels]


def test_a_note_a_hub_links_is_not_an_orphan(tmp_path: Path):
    _note(tmp_path, "10-global/civ6.md", "Civ6 Fix", "knowledge", "Ein Fix ohne Links.")
    (tmp_path / "30-topics" / "macos").mkdir(parents=True)
    (tmp_path / "30-topics/macos/MOC.md").write_text(
        "---\ntitle: macos MOC\nclass: knowledge\n---\n\n- [[Civ6 Fix]]\n", encoding="utf-8")

    notes = _load(tmp_path, "10-global/civ6.md")
    assert maintain.find_orphans(notes, tmp_path) == []


def test_without_the_vault_the_hub_is_invisible(tmp_path: Path):
    """Beleg, dass genau das Weglassen des Vault-Pfads der alte Fehler war."""
    _note(tmp_path, "10-global/civ6.md", "Civ6 Fix", "knowledge", "Ein Fix ohne Links.")
    (tmp_path / "30-topics" / "macos").mkdir(parents=True)
    (tmp_path / "30-topics/macos/MOC.md").write_text(
        "---\ntitle: macos MOC\n---\n\n- [[Civ6 Fix]]\n", encoding="utf-8")

    notes = _load(tmp_path, "10-global/civ6.md")
    assert [n.rel for n in maintain.find_orphans(notes)] == ["10-global/civ6.md"]


def test_a_source_note_is_never_queued(tmp_path: Path):
    """Quellnotizen werden ueber die Suche gefunden, nicht ueber Links."""
    _note(tmp_path, "00-sources/mined/roh.md", "Rohfund", "source", "Ein Fund.")

    notes = _load(tmp_path, "00-sources/mined/roh.md")
    assert maintain.find_orphans(notes, tmp_path) == []


def test_a_truly_disconnected_knowledge_note_is_still_found(tmp_path: Path):
    _note(tmp_path, "10-global/allein.md", "Ganz allein", "knowledge", "Niemand kennt mich.")

    notes = _load(tmp_path, "10-global/allein.md")
    assert [n.rel for n in maintain.find_orphans(notes, tmp_path)] == ["10-global/allein.md"]


def test_the_review_queue_is_not_a_catalog(tmp_path: Path):
    """Die Queue verlinkt Waisen, WEIL sie Waisen sind."""
    _note(tmp_path, "10-global/allein.md", "Ganz allein", "knowledge", "Niemand kennt mich.")
    (tmp_path / "review-queue.md").write_text("- orphan [[Ganz allein]]\n", encoding="utf-8")

    assert orphans.catalog_incoming(tmp_path) == set()
    notes = _load(tmp_path, "10-global/allein.md")
    assert [n.rel for n in maintain.find_orphans(notes, tmp_path)] == ["10-global/allein.md"]


def test_heal_orphans_passes_the_vault_through(tmp_path: Path):
    """Der echte Aufrufpfad, nicht nur die Hilfsfunktion."""
    _note(tmp_path, "10-global/civ6.md", "Civ6 Fix", "knowledge", "Ein Fix ohne Links.")
    (tmp_path / "30-topics" / "macos").mkdir(parents=True)
    (tmp_path / "30-topics/macos/MOC.md").write_text(
        "---\ntitle: macos MOC\n---\n\n- [[Civ6 Fix]]\n", encoding="utf-8")

    notes = _load(tmp_path, "10-global/civ6.md")
    assert maintain.heal_orphans(notes, VaultWriter(tmp_path)) == []
