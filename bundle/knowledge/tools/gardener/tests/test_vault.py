from pathlib import Path

import pytest

from gardener.vault import (UnsafeWriteError, VaultWriter, check_writable,
                            is_excluded, linked_pair, load_notes)


def test_load_notes_excludes_protected_paths(tmp_vault):
    rels = {n.rel for n in load_notes(tmp_vault)}
    assert "10-global/alpha.md" in rels
    assert "20-projects/demo/gamma.md" in rels
    assert not any(r.startswith("90-secrets") for r in rels)
    assert not any(r.startswith((".obsidian", "_meta")) for r in rels)
    assert not any("gardener-report" in r for r in rels)


def test_is_excluded_rules():
    assert is_excluded(Path("90-secrets/x.md"))
    assert is_excluded(Path("90-secrets/deep/x.md"))
    assert is_excluded(Path(".obsidian/x.md"))
    assert is_excluded(Path("_meta/templates/note.md"))
    assert is_excluded(Path("HOT.md"))
    assert is_excluded(Path("review-queue.md"))
    assert is_excluded(Path("00-sources/brain-health-2026-27.md"))
    assert not is_excluded(Path("10-global/note.md"))
    # generated MOCs at any depth are not corpus (design: writable, not read)
    assert is_excluded(Path("20-projects/demo/MOC.md"))
    # macOS is case-insensitive: exclusion rules must be too
    assert is_excluded(Path("90-Secrets/x.md"))
    assert is_excluded(Path("_Meta/templates/note.md"))
    assert is_excluded(Path("hot.md"))


def test_writer_refuses_case_variant_secrets(tmp_vault):
    w = VaultWriter(tmp_vault)
    with pytest.raises(UnsafeWriteError):
        w.write(tmp_vault / "90-Secrets" / "new.md", "x")


def test_writer_refuses_secrets_and_non_md(tmp_vault):
    w = VaultWriter(tmp_vault)
    with pytest.raises(UnsafeWriteError):
        w.write(tmp_vault / "90-secrets" / "new.md", "x")
    with pytest.raises(UnsafeWriteError):
        w.write(tmp_vault / "10-global" / "script.py", "x")
    with pytest.raises(UnsafeWriteError):
        w.write(tmp_vault.parent / "outside.md", "x")
    with pytest.raises(UnsafeWriteError):
        w.write(tmp_vault / ".obsidian" / "x.md", "x")
    with pytest.raises(UnsafeWriteError):
        check_writable(tmp_vault, tmp_vault / "90-secrets" / "deep" / "y.md")


def test_writer_dry_run_writes_nothing(tmp_vault):
    w = VaultWriter(tmp_vault, dry_run=True)
    target = tmp_vault / "10-global" / "new.md"
    w.write(target, "content")
    assert not target.exists()
    assert w.planned == ["10-global/new.md"]
    assert w.written == []


def test_writer_real_write_and_append(tmp_vault):
    w = VaultWriter(tmp_vault)
    target = tmp_vault / "review-queue.md"
    w.write(target, "# Q\n")
    w.append(target, "- item\n")
    assert target.read_text() == "# Q\n- item\n"


def test_linked_pair_detects_existing_links(tmp_vault):
    notes = {n.title: n for n in load_notes(tmp_vault)}
    assert linked_pair(notes["Gamma"], notes["Alpha"])
    assert linked_pair(notes["Alpha"], notes["Gamma"])
    assert not linked_pair(notes["Alpha"], notes["Beta"])

def test_title_strips_yaml_quotes(tmp_path):
    # regression: quoted YAML titles produced [['Session note: ...']] links
    from gardener.vault import parse_note
    p = tmp_path / "n.md"
    p.write_text("---\ntitle: 'Session note: stuff (2026)'\n---\nbody\n")
    assert parse_note(tmp_path, p).title == "Session note: stuff (2026)"


def test_parse_note_reads_type(tmp_path):
    from gardener.vault import parse_note
    p = tmp_path / "s.md"
    p.write_text("---\ntitle: S\ntype: session\n---\nbody\n")
    assert parse_note(tmp_path, p).ntype == "session"
    p2 = tmp_path / "n.md"
    p2.write_text("no frontmatter\n")
    assert parse_note(tmp_path, p2).ntype == "note"
