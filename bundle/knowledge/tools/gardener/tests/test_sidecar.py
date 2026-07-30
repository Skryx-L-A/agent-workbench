import datetime as dt

import pytest

from gardener import config, frontmatter, sidecar
from gardener.queue import ReviewQueue
from gardener.vault import VaultWriter, load_notes

from .conftest import FakeOllama

TODAY = dt.date(2026, 7, 28)


def write_asset(vault, rel: str, content: bytes = b"hello world") -> None:
    p = vault / rel
    p.parent.mkdir(parents=True, exist_ok=True)
    p.write_bytes(content)


# --------------------------------------------------------------------------
# .brainignore
# --------------------------------------------------------------------------

def test_brainignore_basename_pattern_matches_anywhere():
    rules = sidecar.parse_brainignore("*.tmp\n")
    assert sidecar.is_ignored(rules, "10-global/_assets/x.tmp", is_dir=False)
    assert sidecar.is_ignored(rules, "x.tmp", is_dir=False)
    assert not sidecar.is_ignored(rules, "x.tmpx", is_dir=False)


def test_brainignore_anchored_dir_pattern_prunes_whole_subtree(tmp_vault):
    # is_ignored() decides per node (as the walker calls it while pruning);
    # an anchored dir-only rule excludes the directory itself, which is what
    # keeps iter_asset_candidates from ever descending into it.
    rules = sidecar.parse_brainignore("/20-projects/demo/_assets/\n")
    assert sidecar.is_ignored(rules, "20-projects/demo/_assets", is_dir=True)
    assert not sidecar.is_ignored(rules, "20-projects/other/_assets", is_dir=True)

    write_asset(tmp_vault, "20-projects/demo/_assets/x.bin")
    write_asset(tmp_vault, "20-projects/other/_assets/y.bin")
    (tmp_vault / ".brainignore").write_text("/20-projects/demo/_assets/\n")
    rels = {p.relative_to(tmp_vault).as_posix()
           for p in sidecar.iter_asset_candidates(tmp_vault)}
    assert "20-projects/other/_assets/y.bin" in rels
    assert not any(r.startswith("20-projects/demo/_assets/") for r in rels)


def test_brainignore_negation_overrides_earlier_ignore():
    rules = sidecar.parse_brainignore("*.bin\n!keep.bin\n")
    assert sidecar.is_ignored(rules, "a.bin", is_dir=False)
    assert not sidecar.is_ignored(rules, "keep.bin", is_dir=False)


def test_brainignore_comments_and_blank_lines_skipped():
    rules = sidecar.parse_brainignore("# comment\n\n*.log\n")
    assert len(rules) == 1
    assert sidecar.is_ignored(rules, "x.log", is_dir=False)


def test_brainignore_leading_whitespace_is_trimmed(tmp_vault):
    # sidecar-002: a line with leading spaces must match exactly like the
    # unindented form (gitignore-style tools trim it, this one silently kept
    # the spaces as part of the pattern so it never matched anything).
    rules = sidecar.parse_brainignore("   *.mitleerzeichen\n")
    assert sidecar.is_ignored(rules, "etwas.mitleerzeichen", is_dir=False)

    write_asset(tmp_vault, "30-topics/_stress/etwas.mitleerzeichen", b"x")
    (tmp_vault / ".brainignore").write_text("   *.mitleerzeichen\n")
    rels = {p.relative_to(tmp_vault).as_posix()
           for p in sidecar.iter_asset_candidates(tmp_vault)}
    assert "30-topics/_stress/etwas.mitleerzeichen" not in rels


# --------------------------------------------------------------------------
# candidate walk
# --------------------------------------------------------------------------

def test_candidate_walk_skips_excluded_dirs_and_md_files(tmp_vault):
    write_asset(tmp_vault, "10-global/photo.png")
    write_asset(tmp_vault, "90-secrets/leak.bin")
    write_asset(tmp_vault, "_meta/tools/helper.bin")
    write_asset(tmp_vault, ".obsidian/cache.bin")
    write_asset(tmp_vault, "10-global/.DS_Store")
    write_asset(tmp_vault, config.DROP_DIR + "/raw.pdf")
    candidates = sidecar.iter_asset_candidates(tmp_vault)
    rels = {p.relative_to(tmp_vault).as_posix() for p in candidates}
    assert "10-global/photo.png" in rels
    assert "90-secrets/leak.bin" not in rels
    assert "_meta/tools/helper.bin" not in rels
    assert ".obsidian/cache.bin" not in rels
    assert "10-global/.DS_Store" not in rels
    assert not any(r.startswith(config.DROP_DIR) for r in rels)
    assert not any(r.endswith(".md") for r in rels)


def test_candidate_walk_respects_brainignore(tmp_vault):
    write_asset(tmp_vault, "10-global/keepme.bin")
    write_asset(tmp_vault, "10-global/skipme.bin")
    (tmp_vault / ".brainignore").write_text("skipme.bin\n")
    rels = {p.relative_to(tmp_vault).as_posix()
           for p in sidecar.iter_asset_candidates(tmp_vault)}
    assert "10-global/keepme.bin" in rels
    assert "10-global/skipme.bin" not in rels


def test_lock_and_cache_globs_excluded(tmp_vault):
    write_asset(tmp_vault, "10-global/thing.pyc")
    write_asset(tmp_vault, "10-global/thing.lock")
    write_asset(tmp_vault, "10-global/real.bin")
    rels = {p.relative_to(tmp_vault).as_posix()
           for p in sidecar.iter_asset_candidates(tmp_vault)}
    assert "10-global/real.bin" in rels
    assert "10-global/thing.pyc" not in rels
    assert "10-global/thing.lock" not in rels


# --------------------------------------------------------------------------
# generate(): the three required asset kinds + idempotency + human-edited
# --------------------------------------------------------------------------

def test_generate_text_file_gets_sidecar_with_judge_summary(tmp_vault):
    write_asset(tmp_vault, "20-projects/demo/notes.txt",
               b"Wichtige Projektnotizen zum Demo-Rollout.")
    writer = VaultWriter(tmp_vault)
    client = FakeOllama(verdicts=[{"summary": "Notizen zum Demo-Rollout."}])
    result = sidecar.generate(tmp_vault, writer, client, today=TODAY)

    sidecar_path = tmp_vault / "20-projects/demo/notes.txt.md"
    assert sidecar_path.exists()
    assert result.generated == ["20-projects/demo/notes.txt"]
    text = sidecar_path.read_text()
    fields, body = frontmatter.parse(text)
    assert fields["type"] == "asset"
    assert fields["path"] == "20-projects/demo/notes.txt"
    assert fields["generated-by"] == config.JUDGE_MODEL
    assert "Notizen zum Demo-Rollout." in body
    assert sidecar.AUTO_START in text and sidecar.AUTO_END in text


def test_generate_image_uses_vision_model(tmp_vault):
    write_asset(tmp_vault, "10-global/diagram.png", b"\x89PNG fake bytes")
    writer = VaultWriter(tmp_vault)
    client = FakeOllama(vision="Ein Architekturdiagramm mit drei Boxen.")
    result = sidecar.generate(tmp_vault, writer, client, today=TODAY)

    sidecar_path = tmp_vault / "10-global/diagram.png.md"
    assert result.generated == ["10-global/diagram.png"]
    text = sidecar_path.read_text()
    assert "Architekturdiagramm" in text
    fields, _ = frontmatter.parse(text)
    assert fields["generated-by"] == config.VISION_MODEL


def test_generate_binary_without_extractor_gets_metadata_only_sidecar(tmp_vault):
    write_asset(tmp_vault, "10-global/blob.bin", b"\x00\x01\x02binary")
    writer = VaultWriter(tmp_vault)
    client = FakeOllama()
    result = sidecar.generate(tmp_vault, writer, client, today=TODAY)

    sidecar_path = tmp_vault / "10-global/blob.bin.md"
    assert sidecar_path.exists()
    assert result.metadata_only == ["10-global/blob.bin"]
    text = sidecar_path.read_text()
    assert sidecar.PLACEHOLDER in text
    fields, _ = frontmatter.parse(text)
    assert fields["generated-by"] == "metadata-only"
    assert fields["sha256"]
    assert fields["mime"] == "application/octet-stream"


def test_generate_is_idempotent_on_unchanged_hash(tmp_vault):
    write_asset(tmp_vault, "10-global/blob.bin", b"stable content")
    writer = VaultWriter(tmp_vault)
    client = FakeOllama()
    sidecar.generate(tmp_vault, writer, client, today=TODAY)
    sidecar_path = tmp_vault / "10-global/blob.bin.md"
    first_text = sidecar_path.read_text()

    writer2 = VaultWriter(tmp_vault)
    result2 = sidecar.generate(tmp_vault, writer2, client, today=TODAY)
    assert result2.generated == [] and result2.updated == []
    assert sidecar_path.read_text() == first_text


def test_generate_regenerates_body_when_hash_changes_but_keeps_hand_added_links(tmp_vault):
    write_asset(tmp_vault, "10-global/blob.txt", b"version one")
    writer = VaultWriter(tmp_vault)
    client = FakeOllama(verdicts=[{"summary": "Erste Version."}])
    sidecar.generate(tmp_vault, writer, client, today=TODAY)
    sidecar_path = tmp_vault / "10-global/blob.txt.md"

    # human adds a wikilink below the auto block
    text = sidecar_path.read_text()
    text = text.replace("<!-- relates-to [[...]] -->",
                        "<!-- relates-to [[...]] -->\nrelates-to [[Alpha]]")
    sidecar_path.write_text(text)

    write_asset(tmp_vault, "10-global/blob.txt", b"version two, changed")
    writer2 = VaultWriter(tmp_vault)
    client2 = FakeOllama(verdicts=[{"summary": "Zweite Version."}])
    result2 = sidecar.generate(tmp_vault, writer2, client2, today=TODAY)

    assert result2.updated == ["10-global/blob.txt"]
    new_text = sidecar_path.read_text()
    assert "Zweite Version." in new_text
    assert "Erste Version." not in new_text
    assert "relates-to [[Alpha]]" in new_text   # hand-added link survived


def test_human_edited_sidecar_is_never_touched(tmp_vault):
    write_asset(tmp_vault, "10-global/blob.txt", b"original")
    sidecar_path = tmp_vault / "10-global/blob.txt.md"
    sidecar_path.write_text(
        "---\ntitle: blob.txt\ntype: asset\npath: 10-global/blob.txt\n"
        "mime: text/plain\nbytes: 8\nsha256: deadbeef\ncreated: 2026-01-01\n"
        "source: hand\ngenerated-by: human\ngenerated-at: 2026-01-01\n"
        "human-edited: true\n---\n\nHandgeschriebene Beschreibung.\n")
    write_asset(tmp_vault, "10-global/blob.txt", b"changed content, different hash")
    writer = VaultWriter(tmp_vault)
    client = FakeOllama(verdicts=[{"summary": "sollte nie verwendet werden"}])
    result = sidecar.generate(tmp_vault, writer, client, today=TODAY)
    assert result.skipped_human_edited == ["10-global/blob.txt"]
    assert sidecar_path.read_text() == (
        "---\ntitle: blob.txt\ntype: asset\npath: 10-global/blob.txt\n"
        "mime: text/plain\nbytes: 8\nsha256: deadbeef\ncreated: 2026-01-01\n"
        "source: hand\ngenerated-by: human\ngenerated-at: 2026-01-01\n"
        "human-edited: true\n---\n\nHandgeschriebene Beschreibung.\n")


def test_malformed_markers_are_left_alone(tmp_vault):
    write_asset(tmp_vault, "10-global/blob.txt", b"v1")
    sidecar_path = tmp_vault / "10-global/blob.txt.md"
    sidecar_path.write_text(
        "---\ntitle: blob.txt\ntype: asset\npath: 10-global/blob.txt\n"
        "mime: text/plain\nbytes: 2\nsha256: oldhash\ncreated: 2026-01-01\n"
        "source: vault\ngenerated-by: x\ngenerated-at: 2026-01-01\n"
        "human-edited: false\n---\n\n"
        f"{sidecar.AUTO_START}\nbroken, no end marker\n")
    write_asset(tmp_vault, "10-global/blob.txt", b"v2, changed")
    writer = VaultWriter(tmp_vault)
    result = sidecar.generate(tmp_vault, writer, FakeOllama(
        verdicts=[{"summary": "neu"}]), today=TODAY)
    assert result.skipped_malformed == ["10-global/blob.txt"]
    assert "broken, no end marker" in sidecar_path.read_text()


def test_external_file_over_cap_is_flagged_but_still_gets_a_sidecar(tmp_vault, monkeypatch):
    monkeypatch.setattr(config, "SIDECAR_EXTERNAL_MB", 0)   # force "external"
    write_asset(tmp_vault, "10-global/big.bin", b"x" * 100)
    writer = VaultWriter(tmp_vault)
    result = sidecar.generate(tmp_vault, writer, FakeOllama(), today=TODAY)
    assert result.external == ["10-global/big.bin"]
    text = (tmp_vault / "10-global/big.bin.md").read_text()
    assert "liegt extern" in text


# --------------------------------------------------------------------------
# legacy `_assets/` stubs: recognized, enriched, body never touched
# --------------------------------------------------------------------------

def test_legacy_stub_gets_missing_fields_added_but_body_untouched(tmp_vault):
    assets = tmp_vault / "10-global" / "_assets"
    assets.mkdir(parents=True)
    (assets / "paper.txt").write_text("Ein Fachtext.")
    (assets / "paper.md").write_text(
        "---\ntitle: paper\ntype: asset\nbranch: 10-global\n"
        "path: _assets/paper.txt\nmime: text/plain\n---\n\n"
        "Handgeschriebene Beschreibung, darf nicht angefasst werden.\n")
    writer = VaultWriter(tmp_vault)
    result = sidecar.generate(tmp_vault, writer, FakeOllama(), today=TODAY)

    stub_text = (assets / "paper.md").read_text()
    assert "Handgeschriebene Beschreibung, darf nicht angefasst werden." in stub_text
    assert result.legacy_enriched == ["10-global/_assets/paper.txt"]
    fields, _ = frontmatter.parse(stub_text)
    for key in sidecar.LEGACY_REQUIRED:
        assert key in fields
    # a plain sidecar-style file must NOT also be created for a legacy asset
    assert not (assets / "paper.txt.md").exists()


def test_legacy_stub_with_all_fields_is_left_alone(tmp_vault):
    assets = tmp_vault / "10-global" / "_assets"
    assets.mkdir(parents=True)
    (assets / "paper.txt").write_text("Text.")
    original = (
        "---\ntitle: paper\ntype: asset\nbranch: 10-global\n"
        "path: _assets/paper.txt\nmime: text/plain\nbytes: 5\n"
        "sha256: abc\ncreated: 2026-01-01\nsource: hand\n"
        "generated-by: human\ngenerated-at: 2026-01-01\n"
        "human-edited: true\n---\n\nBeschreibung.\n")
    (assets / "paper.md").write_text(original)
    writer = VaultWriter(tmp_vault)
    result = sidecar.generate(tmp_vault, writer, FakeOllama(), today=TODAY)
    assert result.legacy_enriched == []
    assert (assets / "paper.md").read_text() == original


# --------------------------------------------------------------------------
# scan() / check gate
# --------------------------------------------------------------------------

def test_scan_reports_missing_and_ok_without_writing(tmp_vault):
    write_asset(tmp_vault, "10-global/a.bin", b"data")
    entries_before = sidecar.scan(tmp_vault)
    assert any(e.rel == "10-global/a.bin" and e.status == "missing"
              for e in entries_before)
    assert not (tmp_vault / "10-global/a.bin.md").exists()   # scan never writes

    writer = VaultWriter(tmp_vault)
    sidecar.generate(tmp_vault, writer, FakeOllama(), today=TODAY)
    entries_after = sidecar.scan(tmp_vault)
    assert any(e.rel == "10-global/a.bin" and e.status == "ok"
              for e in entries_after)


def test_scan_reports_stale_after_content_changes(tmp_vault):
    write_asset(tmp_vault, "10-global/a.bin", b"v1")
    writer = VaultWriter(tmp_vault)
    sidecar.generate(tmp_vault, writer, FakeOllama(), today=TODAY)
    write_asset(tmp_vault, "10-global/a.bin", b"v2 changed")
    entries = sidecar.scan(tmp_vault)
    assert any(e.rel == "10-global/a.bin" and e.status == "stale" for e in entries)


def test_scan_path_filter_restricts_to_subtree(tmp_vault):
    write_asset(tmp_vault, "10-global/a.bin", b"a")
    write_asset(tmp_vault, "20-projects/demo/b.bin", b"b")
    entries = sidecar.scan(tmp_vault, path="20-projects/demo")
    rels = {e.rel for e in entries}
    assert rels == {"20-projects/demo/b.bin"}


# --------------------------------------------------------------------------
# B09: one unreadable file must not abort the whole phase
# --------------------------------------------------------------------------

def test_generate_skips_single_unreadable_file_but_finishes_the_rest(tmp_vault):
    write_asset(tmp_vault, "10-global/good.bin", b"readable content")
    write_asset(tmp_vault, "10-global/bad.bin", b"unreadable content")
    bad_path = tmp_vault / "10-global" / "bad.bin"
    bad_path.chmod(0o000)
    try:
        writer = VaultWriter(tmp_vault)
        result = sidecar.generate(tmp_vault, writer, FakeOllama(), today=TODAY)
        assert result.skipped_unreadable == ["10-global/bad.bin"]
        assert "10-global/good.bin" in result.generated
        assert (tmp_vault / "10-global/good.bin.md").exists()
    finally:
        bad_path.chmod(0o644)


def test_unreadable_file_is_queued_for_review_not_silently_dropped(tmp_vault):
    write_asset(tmp_vault, "10-global/bad2.bin", b"x")
    bad_path = tmp_vault / "10-global" / "bad2.bin"
    bad_path.chmod(0o000)
    try:
        writer = VaultWriter(tmp_vault)
        notes = load_notes(tmp_vault)
        queue = ReviewQueue(writer)
        result = sidecar.run_sidecar_phase(
            tmp_vault, notes, writer, FakeOllama(), queue, today=TODAY)
        assert result.skipped_unreadable == ["10-global/bad2.bin"]
        assert any("bad2.bin" in line for line in queue.added)
    finally:
        bad_path.chmod(0o644)


# --------------------------------------------------------------------------
# sidecar-001: an unreadable file must still get a sidecar - metadata plus
# placeholder, never nothing (the overcorrection of B09 above)
# --------------------------------------------------------------------------

def test_unreadable_file_still_gets_a_sidecar_with_metadata_and_reason(tmp_vault):
    write_asset(tmp_vault, "30-topics/_stress/keinrecht.dat", b"nicht lesbar\n")
    bad_path = tmp_vault / "30-topics/_stress/keinrecht.dat"
    bad_path.chmod(0o000)
    try:
        writer = VaultWriter(tmp_vault)
        result = sidecar.generate(tmp_vault, writer, FakeOllama(), today=TODAY)
        assert result.skipped_unreadable == ["30-topics/_stress/keinrecht.dat"]
        sidecar_path = tmp_vault / "30-topics/_stress/keinrecht.dat.md"
        assert sidecar_path.exists()
        text = sidecar_path.read_text()
        fields, _ = frontmatter.parse(text)
        assert fields["bytes"] == "13"
        assert fields["mime"] == "application/octet-stream"
        # content unreadable: no real hash available yet
        assert fields.get("sha256", "") == ""
        assert "nicht automatisch lesbar" in text
        assert "Permission denied" in text
        assert "Groesse 13 Bytes" in text
        assert "Rechte 000" in text
    finally:
        bad_path.chmod(0o644)


def test_unreadable_file_self_heals_once_permissions_are_restored(tmp_vault):
    write_asset(tmp_vault, "10-global/heilt.bin", b"content")
    bad_path = tmp_vault / "10-global/heilt.bin"
    bad_path.chmod(0o000)
    writer = VaultWriter(tmp_vault)
    sidecar.generate(tmp_vault, writer, FakeOllama(), today=TODAY)
    bad_path.chmod(0o644)
    result = sidecar.generate(tmp_vault, writer, FakeOllama(), today=TODAY)
    assert "10-global/heilt.bin" in result.updated
    fields, _ = frontmatter.parse((tmp_vault / "10-global/heilt.bin.md").read_text())
    assert fields["sha256"] == sidecar.sha256_of(bad_path)


# --------------------------------------------------------------------------
# B18: a filename with an embedded newline must render as valid, quoted YAML
# --------------------------------------------------------------------------

def test_filename_with_newline_produces_single_line_quoted_fields(tmp_vault):
    bad_name = "zeilen\numbruch.dat"
    rel = f"30-topics/_stress/{bad_name}"
    write_asset(tmp_vault, rel, b"x")
    writer = VaultWriter(tmp_vault)
    result = sidecar.generate(tmp_vault, writer, FakeOllama(), today=TODAY)
    assert result.generated == [rel]

    sidecar_path = (tmp_vault / rel).with_name((tmp_vault / rel).name + ".md")
    text = sidecar_path.read_text()
    raw_blocks, _ = frontmatter.split_blocks(text)
    assert len(raw_blocks) == 1
    # one physical line per field: the embedded newline must not have split
    # "title" onto two lines and shifted every field after it (the raw B18
    # failure), and must not have injected a stray closing "---".
    keys = [ln.split(":", 1)[0] for ln in raw_blocks[0].splitlines()]
    # id/schema stehen vorn, weil der Schreiber jede Notiz beim Anlegen
    # stempelt (identity.py) - ein Sidecar ist eine Notiz wie jede andere.
    assert keys == ["id", "schema", "title", "type", "path", "mime", "bytes",
                    "sha256", "created", "source", "generated-by",
                    "generated-at", "human-edited"]

    fields, _ = frontmatter.parse(text)
    assert fields["title"] == bad_name
    assert fields["path"] == rel
