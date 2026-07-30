"""Eine handgeschriebene Themenseite gehoert nicht der Synthese.

Am 2026-07-29 hat ein `gardener run --phase all` die vier handkuratierten Hubs
(claude-orchestration, dsgvo-recht, local-models, macos-setup) in erzeugte Seiten
verwandelt und dabei 114 handgeschriebene Zeilen ersetzt. Der Code erlaubte das
ausdruecklich ("no stored hash yet ... including a hand-curated hub converting"),
waehrend die festgehaltene Entscheidung genau das verbot. Der Code hat gewonnen.

`class: knowledge` ist die Aussage des Autors, dass die Seite ihm gehoert.
"""
from __future__ import annotations

from pathlib import Path

from gardener import config, synth
from gardener.vault import VaultWriter, parse_note


class _NoClient:
    """Wird nie aufgerufen: eine uebersprungene Seite fragt kein Modell."""

    def generate(self, *a, **kw):  # pragma: no cover - darf nicht passieren
        raise AssertionError("das Modell wurde fuer eine handgeschriebene Seite befragt")


def _hub(vault: Path, name: str, klass: str, body: str) -> Path:
    p = vault / "30-topics" / name / "MOC.md"
    p.parent.mkdir(parents=True, exist_ok=True)
    p.write_text(f"---\ntitle: {name} MOC\ntype: moc\nclass: {klass}\n---\n\n{body}\n",
                 encoding="utf-8")
    return p


def _sources(vault: Path, name: str, n: int) -> list:
    out = []
    for i in range(n):
        p = vault / "10-global" / f"{name}-quelle-{i}.md"
        p.parent.mkdir(parents=True, exist_ok=True)
        p.write_text(f"---\ntitle: {name} Quelle {i}\nclass: knowledge\n---\n\n"
                     f"Aussage {i} zum Thema {name}.\n", encoding="utf-8")
        out.append(parse_note(vault, p))
    return out


def test_a_hand_written_hub_is_never_converted(tmp_path: Path, monkeypatch):
    hand = "Von Hand geschrieben, mit Nuancen, die kein Modell rekonstruiert."
    page = _hub(tmp_path, "thema", "knowledge", hand)
    sources = _sources(tmp_path, "thema", 6)

    cand = synth.TopicCandidate(name="thema", sources=sources)
    monkeypatch.setattr(synth, "discover_candidates", lambda *a, **kw: [cand])

    result = synth.run_synth(tmp_path, sources, [], {}, VaultWriter(tmp_path),
                             _NoClient(), contra_store=None, min_sources=4)

    assert result.skipped_hand_written == ["thema"]
    assert result.written == []
    assert hand in page.read_text(encoding="utf-8")
    assert "class: knowledge" in page.read_text(encoding="utf-8")


def test_a_derived_page_is_still_regenerated(tmp_path: Path, monkeypatch):
    """Die Sperre darf die eigenen Seiten der Synthese nicht mitsperren."""
    page = _hub(tmp_path, "thema", "derived", "Alter erzeugter Text [[thema Quelle 0]]")
    sources = _sources(tmp_path, "thema", 6)

    cand = synth.TopicCandidate(name="thema", sources=sources)
    monkeypatch.setattr(synth, "discover_candidates", lambda *a, **kw: [cand])
    monkeypatch.setattr(synth, "synthesize",
                        lambda client, c: ("Einleitung.",
                                           ["Neue Aussage [[thema Quelle 1]]"], 0, 0))
    monkeypatch.setattr(synth, "contradiction_lines", lambda *a, **kw: [])

    result = synth.run_synth(tmp_path, sources, [], {}, VaultWriter(tmp_path),
                             object(), contra_store=None, min_sources=4)

    assert result.skipped_hand_written == []
    assert result.written == ["30-topics/thema/MOC.md"]
    assert "Neue Aussage" in page.read_text(encoding="utf-8")


def test_a_page_without_class_is_treated_as_hand_written(tmp_path: Path, monkeypatch):
    """Im Zweifel gehoert die Seite dem Menschen, nicht dem Gardener.

    Ohne `class` ist nicht belegt, dass die Synthese sie geschrieben hat - und
    ein faelschlich uebersprungener Lauf kostet nichts, ein faelschlich
    ueberschriebener Text ist weg.
    """
    hand = "Ohne Klassenangabe, aber von Hand."
    page = _hub(tmp_path, "thema", "knowledge", hand)
    text = page.read_text(encoding="utf-8").replace("class: knowledge\n", "")
    page.write_text(text, encoding="utf-8")
    sources = _sources(tmp_path, "thema", 6)

    cand = synth.TopicCandidate(name="thema", sources=sources)
    monkeypatch.setattr(synth, "discover_candidates", lambda *a, **kw: [cand])
    monkeypatch.setattr(synth, "synthesize",
                        lambda client, c: ("x", ["y [[thema Quelle 1]]"], 0, 0))
    monkeypatch.setattr(synth, "contradiction_lines", lambda *a, **kw: [])

    result = synth.run_synth(tmp_path, sources, [], {}, VaultWriter(tmp_path),
                             object(), contra_store=None, min_sources=4)

    assert result.skipped_hand_written == ["thema"]
    assert hand in page.read_text(encoding="utf-8")
