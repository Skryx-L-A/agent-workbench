"""Jede neue Notiz bekommt eine unveraenderliche ULID - sonst gilt Brain 4.0
nur fuer die Notizen, die zufaellig vor der Migration existierten.

Gemessen 2026-07-29: 22 echte Notizen ohne `id`, darunter alle zehn Themenseiten,
die die Synthese-Phase kurz zuvor erzeugt hatte. Gestempelt wird am einen
Schreib-Tor, nicht in jedem Schreiber.
"""
from __future__ import annotations

from pathlib import Path

from gardener import identity
from gardener.vault import VaultWriter

NOTE = "---\ntitle: Neu\ntype: note\n---\n\nInhalt.\n"


def test_ulid_shape():
    v = identity.ulid()
    assert identity.ULID_RE.match(v), v
    assert len({identity.ulid() for _ in range(50)}) == 50


def test_a_new_note_gets_id_and_schema(tmp_path: Path):
    writer = VaultWriter(tmp_path)
    p = tmp_path / "10-global" / "neu.md"
    assert writer.write(p, NOTE)

    text = p.read_text(encoding="utf-8")
    assert identity.ULID_RE.match(identity.id_of(text) or "")
    assert "schema: 4" in text
    assert "title: Neu" in text and "Inhalt." in text


def test_a_rewrite_keeps_the_original_id(tmp_path: Path):
    writer = VaultWriter(tmp_path)
    p = tmp_path / "10-global" / "neu.md"
    writer.write(p, NOTE)
    first = identity.id_of(p.read_text(encoding="utf-8"))

    writer.write(p, NOTE.replace("Inhalt.", "Anderer Inhalt."))

    assert identity.id_of(p.read_text(encoding="utf-8")) == first


def test_an_existing_id_survives_a_caller_that_supplies_a_different_one():
    text = "---\nid: 01KYMQ8BNHCRCMQXYZQVX058KN\nschema: 4\ntitle: Alt\n---\n\nX\n"
    out = identity.stamp(text, keep_id=identity.ulid())
    assert out == text


def test_text_without_frontmatter_is_left_alone(tmp_path: Path):
    """HOT.md, Berichte und die Review-Queue sind keine Notizen."""
    writer = VaultWriter(tmp_path)
    p = tmp_path / "HOT.md"
    body = "# HOT\n\n- [[irgendwas]]\n"
    writer.write(p, body)
    assert p.read_text(encoding="utf-8") == body


def test_the_gate_still_refuses_non_markdown(tmp_path: Path):
    """Die Suffix-Pruefung beim Stempeln ist Guertel und Hosentraeger: das Tor
    laesst ohnehin nur .md durch."""
    import pytest

    from gardener.vault import UnsafeWriteError

    writer = VaultWriter(tmp_path)
    with pytest.raises(UnsafeWriteError):
        writer.write(tmp_path / "10-global" / "daten.json", '{"a": 1}\n')


def test_schema_is_added_to_a_note_that_already_has_an_id():
    text = "---\nid: 01KYMQ8BNHCRCMQXYZQVX058KN\ntitle: Alt\n---\n\nX\n"
    out = identity.stamp(text)
    assert "schema: 4" in out
    assert out.index("schema:") > out.index("id:")
    assert out.count("id:") == 1


def test_a_broken_id_is_replaced_not_kept():
    text = "---\nid: nicht-ulid\ntitle: Alt\n---\n\nX\n"
    out = identity.stamp(text)
    assert identity.ULID_RE.match(identity.id_of(out) or "")
    assert "nicht-ulid" not in out
