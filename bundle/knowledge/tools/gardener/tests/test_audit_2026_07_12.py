"""Regressions for the 2026-07-12 gardener audit.

Each test pins one confirmed bug: silent overwrite of concurrent edits, marker-
block corruption, frontmatter leaking into MOC hooks, hallucinated structural
links, dry-run state writes, non-idempotent merges, symlink handling, NFC/NFD.
"""
from __future__ import annotations

import argparse
import datetime as dt
import json
import unicodedata
from pathlib import Path

import pytest

from gardener import (blocks, cli, config, consolidate, frontmatter,
                      ingest as ingest_mod, linking, maintain)
from gardener.queue import ReviewQueue
from gardener.runtime import read_last_run
from gardener.store import Store
from gardener.vault import VaultWriter, load_notes, parse_note

from .conftest import FakeOllama, make_note
from .test_dry_run import isolated, args_for, snapshot   # noqa: F401


# --- write gate: never overwrite what changed under us ----------------------

def test_write_refuses_when_file_changed_since_read(tmp_vault):
    writer = VaultWriter(tmp_vault)
    path = tmp_vault / "10-global" / "alpha.md"
    stale = path.read_text()
    path.write_text(stale + "\nedited by Obsidian while the run was going\n")

    ok = writer.write(path, "gardener text derived from the stale copy",
                      expect=stale)

    assert ok is False
    assert "Obsidian" in path.read_text()          # the human's edit survives
    assert writer.conflicts == ["10-global/alpha.md"]


def test_write_does_not_resurrect_a_deleted_note(tmp_vault):
    writer = VaultWriter(tmp_vault)
    path = tmp_vault / "10-global" / "beta.md"
    stale = path.read_text()
    path.unlink()

    assert writer.write(path, stale + "\nStand: 2026-07\n", expect=stale) is False
    assert not path.exists()


def test_write_without_expect_is_unconditional(tmp_vault):
    """Generated files (HOT.md, reports) are owned by the gardener: no guard."""
    writer = VaultWriter(tmp_vault)
    assert writer.write(tmp_vault / "HOT.md", "# HOT\n") is True
    assert (tmp_vault / "HOT.md").read_text() == "# HOT\n"


def test_add_link_does_not_clobber_a_concurrent_edit(tmp_vault):
    notes = load_notes(tmp_vault)
    a = next(n for n in notes if n.title == "Alpha")
    b = next(n for n in notes if n.title == "Beta")
    writer = VaultWriter(tmp_vault)
    a.path.write_text(a.text + "\nbasic-memory sync appended this\n")

    assert linking.add_link(writer, a, b, "relates-to", "relations") is False
    assert "basic-memory sync appended this" in a.path.read_text()
    assert "[[Beta]]" not in a.path.read_text()


def test_recency_marker_does_not_clobber_a_concurrent_edit(tmp_vault):
    notes = load_notes(tmp_vault)
    alpha = next(n for n in notes if n.title == "Alpha")
    writer = VaultWriter(tmp_vault)
    alpha.path.write_text("---\ntitle: Alpha\ntype: note\n---\n\nrewritten by hand\n")

    added = maintain.add_recency_markers([alpha], writer)

    assert added == []
    assert alpha.path.read_text() == \
        "---\ntitle: Alpha\ntype: note\n---\n\nrewritten by hand\n"
    assert writer.conflicts == ["10-global/alpha.md"]


# --- reviewer finding 1: "expect nothing" != "do not check" -----------------

def test_creating_write_refuses_a_file_that_appeared_meanwhile(tmp_vault):
    """update_mocs passes expect=None when it believes the MOC does not exist.
    If someone created it between the exists() check and the write, expect=None
    used to mean 'do not check' and silently clobbered it."""
    writer = VaultWriter(tmp_vault)
    moc = tmp_vault / "20-projects" / "demo" / "MOC.md"
    moc.write_text("von Hand angelegt, waehrend der Lauf lief\n")

    ok = writer.write(moc, "gardener-generated MOC", expect=None)

    assert ok is False
    assert moc.read_text() == "von Hand angelegt, waehrend der Lauf lief\n"
    assert writer.conflicts == ["20-projects/demo/MOC.md"]


def test_creating_write_succeeds_when_the_file_really_is_absent(tmp_vault):
    writer = VaultWriter(tmp_vault)
    moc = tmp_vault / "20-projects" / "demo" / "MOC.md"

    assert writer.write(moc, "neu", expect=None) is True
    assert moc.read_text() == "neu"


def test_generated_files_are_still_written_unconditionally(tmp_vault):
    """HOT.md and the reports are gardener-owned: NO_CHECK, not expect=None."""
    from gardener.vault import NO_CHECK

    writer = VaultWriter(tmp_vault)
    hot = tmp_vault / "HOT.md"
    hot.write_text("alter Stand")

    assert writer.write(hot, "neuer Stand") is True          # default = NO_CHECK
    assert writer.write(hot, "noch neuer", expect=NO_CHECK) is True
    assert hot.read_text() == "noch neuer"
    assert writer.conflicts == []


def test_moc_creation_asks_for_a_nonexistent_file_not_no_check(tmp_path):
    """update_mocs must create the MOC with expect=None ("I expect no file"),
    never with NO_CHECK - otherwise a MOC created during the run is clobbered."""
    from gardener.vault import NO_CHECK

    v = tmp_path / "vault"
    make_note(v, "20-projects/demo/note.md", "Demo Note", "body")
    writer = VaultWriter(v)
    seen: list = []
    real_write = writer.write

    def spy(path, text, expect=NO_CHECK):
        seen.append((Path(path).name, expect))
        return real_write(path, text, expect)

    writer.write = spy
    maintain.update_mocs(load_notes(v), writer)

    expect_used = next(e for name, e in seen if name == "MOC.md")
    assert expect_used is None                 # a creating write, checked
    assert expect_used is not NO_CHECK


# --- reviewer finding 2: a --- rule in the body is not frontmatter -----------

def test_body_starting_with_a_horizontal_rule_is_not_eaten():
    text = ("---\ntitle: T\ntype: note\n---\n\n"
            "---\n"
            "WICHTIGER INHALT ZWISCHEN ZWEI TRENNLINIEN\n"
            "---\n\n"
            "Rest der Note.\n")

    fields, body = frontmatter.parse(text)

    assert fields == {"title": "T", "type": "note"}
    assert "WICHTIGER INHALT ZWISCHEN ZWEI TRENNLINIEN" in body
    assert "Rest der Note." in body


def test_ensure_single_does_not_drop_body_between_rules():
    """The merge path: ensure_single() runs over free-form LLM output."""
    merged = ("---\ntitle: A\ntype: note\n---\n\n"
              "---\nFakt der beim Merge nicht verloren gehen darf\n---\n\n"
              "Weiterer Text.\n")

    out = frontmatter.ensure_single(merged)

    assert "Fakt der beim Merge nicht verloren gehen darf" in out
    assert "Weiterer Text." in out


def test_basic_memory_double_block_is_still_merged():
    """The real double-frontmatter case must keep working."""
    text = ("---\npermalink: main/x\n---\n\n"
            "---\ntitle: X\ntype: note\n---\n\n"
            "Body.\n")

    blocks_found, body = frontmatter.split_blocks(text)
    fields, _ = frontmatter.parse(text)

    assert len(blocks_found) == 2
    assert fields == {"permalink": "main/x", "title": "X", "type": "note"}
    assert body.strip() == "Body."


def test_looks_like_yaml_discriminates():
    assert frontmatter.looks_like_yaml("title: X\ntype: note") is True
    assert frontmatter.looks_like_yaml("einfach nur Prosa") is False
    assert frontmatter.looks_like_yaml("") is False


# --- consolidation: a half-applied merge must never destroy a note ----------

def _merge_setup(tmp_path: Path):
    v = tmp_path / "vault"
    make_note(v, "10-global/a.md", "A", "Long note A. " * 40)
    make_note(v, "10-global/b.md", "B", "Long note B, nearly identical. " * 40)
    notes = load_notes(v)
    return v, notes[0], notes[1]


def test_merge_aborts_when_target_changed_and_keeps_the_source(tmp_path, store):
    v, a, b = _merge_setup(tmp_path)
    writer = VaultWriter(v)
    client = FakeOllama(verdicts=[{"action": "merge", "confidence": 0.99,
                                   "merged_markdown": "---\ntitle: A\ntype: note\n"
                                                      "---\n\n" + "merged. " * 200}])
    a.path.write_text("---\ntitle: A\ntype: note\n---\n\nhuman rewrote A\n")

    res = consolidate.run_consolidation([a, b], {a.rel: [1.0, 0.0], b.rel: [1.0, 0.0]},
                                        store, client, writer, queue=ReviewQueue(writer))

    assert res.merged == []
    assert "human rewrote A" in a.path.read_text()
    assert "Long note B" in b.path.read_text()      # B was NOT stubbed away
    assert not store.is_blocked(a.rel, b.rel, "merge")


def test_merge_blocks_the_pair_so_a_second_run_is_a_no_op(tmp_path, store):
    v, a, b = _merge_setup(tmp_path)
    writer = VaultWriter(v)
    merged_md = "---\ntitle: A\ntype: note\n---\n\n" + "merged content. " * 100
    client = FakeOllama(verdicts=[{"action": "merge", "confidence": 0.99,
                                   "merged_markdown": merged_md}])

    res = consolidate.run_consolidation([a, b], {a.rel: [1.0, 0.0], b.rel: [1.0, 0.0]},
                                        store, client, writer, queue=ReviewQueue(writer))

    assert res.merged == [(a.rel, b.rel)]
    assert store.is_blocked(a.rel, b.rel, "merge")
    # second run: the merged note must not be re-judged against its own stub
    after = a.path.read_text()
    notes2 = load_notes(v)
    res2 = consolidate.run_consolidation(
        notes2, {n.rel: [1.0, 0.0] for n in notes2}, store,
        FakeOllama(verdicts=[]), writer, queue=ReviewQueue(writer))
    assert res2.merged == []
    assert a.path.read_text() == after


# --- marker blocks: unbalanced markers must never be regex-swallowed --------

def test_replace_block_refuses_unbalanced_markers():
    text = "keep me\n<!-- s -->\nlost?\nkeep me too\n<!-- s -->\nblock\n<!-- e -->\n"
    _new, ok = blocks.replace_block(text, "<!-- s -->", "<!-- e -->", "NEW")
    assert ok is False


def test_replace_block_refuses_end_before_start():
    _new, ok = blocks.replace_block("<!-- e -->\nx\n<!-- s -->\n",
                                    "<!-- s -->", "<!-- e -->", "NEW")
    assert ok is False


def test_moc_with_orphan_start_marker_is_left_alone(tmp_path):
    """The corruption path: a MOC that has a START but no END used to get a
    SECOND block appended, and the next run then deleted everything between the
    orphan START and the new END."""
    v = tmp_path / "vault"
    make_note(v, "20-projects/demo/note.md", "Demo Note", "body text")
    moc = v / "20-projects" / "demo" / "MOC.md"
    broken = (f"---\ntitle: demo MOC\ntype: note\n---\n\n# demo\n\n"
              f"{maintain.MOC_START}\n- [[Demo Note]]\n\n"
              f"## Handgepflegt\n- wichtig: nicht loeschen\n")
    moc.write_text(broken)
    writer = VaultWriter(v)

    updated = maintain.update_mocs(load_notes(v), writer)

    assert updated == []
    assert moc.read_text() == broken
    assert "nicht loeschen" in moc.read_text()


def test_moc_block_is_replaced_in_place_when_well_formed(tmp_path):
    v = tmp_path / "vault"
    make_note(v, "20-projects/demo/note.md", "Demo Note", "the hook line")
    moc = v / "20-projects" / "demo" / "MOC.md"
    moc.write_text(f"---\ntitle: demo MOC\ntype: note\n---\n\n# demo\n\n"
                   f"## Kurator\n- von Hand\n\n"
                   f"{maintain.MOC_START}\n- veraltet\n{maintain.MOC_END}\n")
    writer = VaultWriter(v)

    updated = maintain.update_mocs(load_notes(v), writer)

    out = moc.read_text()
    assert updated == ["20-projects/demo/MOC.md"]
    assert "- von Hand" in out                  # curated part untouched
    assert "- [[Demo Note]] - the hook line" in out
    assert "veraltet" not in out
    assert out.count(maintain.MOC_START) == 1


def test_set_description_refuses_malformed_stub(tmp_path):
    text = f"stub\n{ingest_mod.DESC_START}\nold\n{ingest_mod.DESC_START}\n"
    new, ok = ingest_mod.set_description(text, "neu")
    assert (new, ok) == (text, False)


# --- MOC hooks: no frontmatter leakage, no phantom links -------------------

def test_hook_line_skips_a_basic_memory_double_frontmatter_block(tmp_path):
    v = tmp_path / "vault"
    p = v / "10-global" / "synced.md"
    p.parent.mkdir(parents=True)
    p.write_text("---\npermalink: main/10-global/synced\n---\n\n"
                 "---\ntitle: Synced\ntype: note\n---\n\n"
                 "Die eigentliche Aussage der Note.\n")

    assert maintain.first_hook_line(parse_note(v, p)) == \
        "Die eigentliche Aussage der Note."


def test_hook_line_flattens_wikilinks(tmp_path):
    v = tmp_path / "vault"
    p = make_note(v, "10-global/x.md", "X", "Gilt fuer [[Beispielfirma]] und mehr.")
    assert maintain.first_hook_line(parse_note(v, p)) == "Gilt fuer Beispielfirma und mehr."


# --- judge quality: structural claims need evidence ------------------------

def _pair(tmp_path, a_body: str, b_body: str):
    v = tmp_path / "vault"
    pa = make_note(v, "20-projects/grundschule-musterstadt/overview.md",
                   "grundschule-musterstadt overview", a_body)
    pb = make_note(v, "10-global/BRAIN3-PLAN.md", "BRAIN3-PLAN", b_body)
    return parse_note(v, pa), parse_note(v, pb)


def test_depends_on_without_textual_evidence_is_downgraded(tmp_path):
    a, b = _pair(tmp_path, "Schule in Musterstadt, Elternabend.", "Brain-Umbau.")
    v = linking.validate_verdict({"link": True, "type": "depends-on",
                                  "confidence": 0.9}, a, b)
    assert v["type"] == "relates-to"


def test_depends_on_with_textual_evidence_survives(tmp_path):
    a, b = _pair(tmp_path, "Setzt den Umbau aus BRAIN3-PLAN voraus.", "Brain-Umbau.")
    v = linking.validate_verdict({"link": True, "type": "depends-on",
                                  "confidence": 0.9}, a, b)
    assert v["type"] == "depends-on"


def test_the_gardeners_own_relations_section_is_not_evidence(tmp_path):
    """Otherwise a hallucinated link confirms itself on the next run."""
    a, b = _pair(tmp_path,
                 "Schule in Musterstadt.\n\n## Relations\n- depends-on [[BRAIN3-PLAN]]\n",
                 "Brain-Umbau.")
    assert linking.mentions(a, b) is False
    v = linking.validate_verdict({"link": True, "type": "depends-on"}, a, b)
    assert v["type"] == "relates-to"


def test_mined_inbox_candidate_can_never_supersede_a_canonical_note(tmp_path):
    """Live 2026-07-12: a mined 00-sources candidate claimed to SUPERSEDE the very
    note it was extracted from. It quotes that note, so the evidence gate let it
    through - but nothing unreviewed may make a structural claim."""
    v = tmp_path / "vault"
    mined = make_note(v, "00-sources/mined/mined-2026-07-12-session-end.md",
                      "Session-End Skill", "UNVERIFIED. Zum session-end-skill.")
    canon = make_note(v, "10-global/session-end-skill.md", "session-end-skill",
                      "Der session-end-skill macht den Wissens-Flush.")
    a, b = parse_note(v, mined), parse_note(v, canon)

    verdict = linking.validate_verdict({"link": True, "type": "supersedes",
                                        "confidence": 0.95}, a, b)

    assert verdict["type"] == "relates-to"


def test_inbox_downgrade_applies_in_both_directions(tmp_path):
    """`brain-cli depends-on Brain.app` was simply inverted; direction is not
    something the gate can judge, so 00-sources is barred either way round."""
    v = tmp_path / "vault"
    canon = make_note(v, "10-global/kanon.md", "Kanon", "Nennt den kandidat klar.")
    mined = make_note(v, "00-sources/mined/mined-kandidat.md", "kandidat", "UNVERIFIED.")
    a, b = parse_note(v, canon), parse_note(v, mined)   # canonical note FIRST

    verdict = linking.validate_verdict({"link": True, "type": "depends-on"}, a, b)

    assert verdict["type"] == "relates-to"
    assert linking.is_staging(b) and not linking.is_staging(a)


def test_structural_link_outside_the_inbox_still_survives(tmp_path):
    """The bar must not become "no structural links at all"."""
    v = tmp_path / "vault"
    src = make_note(v, "20-projects/p/a.md", "A", "Baut auf Kanon auf.")
    dst = make_note(v, "10-global/kanon.md", "Kanon", "Grundlage.")
    a, b = parse_note(v, src), parse_note(v, dst)

    verdict = linking.validate_verdict({"link": True, "type": "depends-on"}, a, b)

    assert verdict["type"] == "depends-on"


def test_low_confidence_link_is_skipped_but_not_blocklisted(tmp_vault, store):
    notes = [n for n in load_notes(tmp_vault) if n.title in ("Alpha", "Beta")]
    writer = VaultWriter(tmp_vault)
    client = FakeOllama(verdicts=[{"link": True, "type": "relates-to",
                                   "confidence": 0.3, "reason": "maybe"}] * 4)
    vectors = {n.rel: [1.0, 0.0, 0.0] for n in notes}

    res = linking.run_linking(notes, vectors, store, client, writer)

    assert res.added == []
    assert any("low confidence" in s for s in res.skipped)
    assert not store.is_blocked(notes[0].rel, notes[1].rel, "link")   # retried later


def test_missing_confidence_still_links(tmp_vault, store):
    """A judge model that omits `confidence` must not silently kill all links."""
    notes = [n for n in load_notes(tmp_vault) if n.title in ("Alpha", "Beta")]
    writer = VaultWriter(tmp_vault)
    client = FakeOllama(verdicts=[{"link": True, "type": "relates-to",
                                   "reason": "same topic"}])
    vectors = {n.rel: [1.0, 0.0, 0.0] for n in notes}

    res = linking.run_linking(notes, vectors, store, client, writer)

    assert len(res.added) == 1


# --- embedding: a long note's tail must not be silently dropped -------------

def test_chunk_text_keeps_short_text_whole():
    assert linking.chunk_text("kurz", size=4000) == ["kurz"]


def test_chunk_text_covers_a_long_note_with_overlap():
    text = "".join(str(i % 10) for i in range(12000))
    chunks = linking.chunk_text(text, size=4000, overlap=200, max_chunks=8)

    assert len(chunks) > 1
    assert all(len(c) <= 4000 for c in chunks)
    assert chunks[0][-200:] == chunks[1][:200]        # the overlap
    assert text[-50:] in chunks[-1]                   # the TAIL is covered


def test_long_note_is_embedded_in_chunks_not_truncated(tmp_path, store):
    """Regression: voxtype/overview.md is 19932 chars; embeddinggemma's 2048-token
    context meant only its first ~8000 chars ever reached the model."""
    v = tmp_path / "vault"
    tail = "GEHEIMES STICHWORT AM ENDE DER NOTE"
    make_note(v, "10-global/lang.md", "Lang", ("fuelltext. " * 1500) + tail)
    notes = load_notes(v)
    client = FakeOllama()

    vectors = linking.embed_notes(notes, store, client)

    assert notes[0].rel in vectors
    assert len(client.embed_calls) > 1                       # chunked
    assert all(len(c) <= config.EMBED_CHUNK_CHARS for c in client.embed_calls)
    assert any(tail in c for c in client.embed_calls)        # the tail WAS embedded


def test_mean_pool_averages_the_chunk_vectors():
    assert linking.mean_pool([[1.0, 0.0], [0.0, 2.0]]) == [0.5, 1.0]


def test_embedding_cache_is_versioned(tmp_path, store):
    """Vectors cached by the old truncating path must not be reused."""
    v = tmp_path / "vault"
    make_note(v, "10-global/a.md", "A", "body")
    note = load_notes(v)[0]
    store.put_embedding(note.rel, note.content_hash, [9.9, 9.9])   # pre-fix entry

    vectors = linking.embed_notes([note], store, FakeOllama())

    assert vectors[note.rel] != [9.9, 9.9]      # recomputed, not the stale vector


# --- corpus loading: unicode + symlinks ------------------------------------

def test_wikilink_resolves_across_nfc_nfd_filenames(tmp_path):
    v = tmp_path / "vault"
    nfd = unicodedata.normalize("NFD", "Haertung-Ubung")
    make_note(v, f"10-global/{nfd}.md", unicodedata.normalize("NFD", "Ubung"), "body")
    make_note(v, "10-global/src.md", "Src",
              f"siehe [[{unicodedata.normalize('NFC', 'Ubung')}]]")
    notes = load_notes(v)
    from gardener.audit import dead_links

    assert dead_links(notes) == []


def test_symlinked_notes_and_dirs_are_skipped(tmp_path):
    v = tmp_path / "vault"
    make_note(v, "10-global/real.md", "Real", "body")
    outside = tmp_path / "outside"
    outside.mkdir()
    (outside / "foreign.md").write_text("---\ntitle: Foreign\n---\n\nnot ours\n")
    (v / "10-global" / "link.md").symlink_to(outside / "foreign.md")
    (v / "20-projects").mkdir(parents=True)
    (v / "20-projects" / "loop").symlink_to(v)          # would make rglob loop

    rels = {n.rel for n in load_notes(v)}

    assert rels == {"10-global/real.md"}


# --- dry-run purity + last-run state ---------------------------------------

def test_dry_run_does_not_create_the_state_db(tmp_vault, isolated):
    state, _logs, _t, _c = isolated
    (state / "gardener.db").unlink(missing_ok=True)

    assert cli.run(args_for(tmp_vault, dry_run=True)) == 0
    assert not (state / "gardener.db").exists()
    assert not (state / "last-run.json").exists()


def test_real_run_records_last_run_for_brain_app(tmp_vault, isolated):
    state, _logs, _t, _c = isolated

    assert cli.run(args_for(tmp_vault, phase="lint", dry_run=False)) == 0

    last = read_last_run(state)
    assert last["phase"] == "lint"
    assert last["status"] == "ok"
    assert last["conflicts"] == 0
    assert "findings" in last and "finished" in last
    assert json.loads((state / "last-run.json").read_text())["summary"].startswith(
        "gardener[lint]")


def test_ingest_phase_runs_standalone_for_the_drop_button(tmp_vault, isolated):
    """Brain.app's drop button calls exactly this: --phase ingest, nothing else."""
    drop = tmp_vault / config.DROP_DIR
    drop.mkdir(parents=True)
    (drop / "notiz.md").write_text("hand-dropped note\n")

    assert cli.run(args_for(tmp_vault, phase="ingest", dry_run=False)) == 0

    assert (tmp_vault / "00-sources" / "notiz.md").exists()
    assert not (drop / "notiz.md").exists()
    assert not (tmp_vault / "HOT.md").exists()      # maintenance did NOT run


# --- a slow Ollama call must not kill the run -------------------------------

class FlakyOllama(FakeOllama):
    """Ollama that times out on the first `fail_first` chat calls."""

    def __init__(self, fail_first: int, **kw):
        super().__init__(**kw)
        self.left = fail_first
        self.failures = 0
        self.transient_failures = 0

    def judge(self, system, prompt):
        if self.left > 0:
            self.left -= 1
            self.transient_failures += 1
            return {}          # what the hardened client returns on a timeout
        return super().judge(system, prompt)


def test_one_timed_out_judge_call_does_not_kill_the_run(tmp_vault, isolated,
                                                        monkeypatch):
    """2026-07-12: a single 120 s judge timeout aborted the whole run (exit 2)."""
    client = FlakyOllama(fail_first=2,
                         verdicts=[{"link": True, "type": "relates-to",
                                    "confidence": 0.9, "reason": "same topic"}])
    monkeypatch.setattr(cli, "OllamaClient", lambda *a, **k: client)

    rc = cli.run(args_for(tmp_vault, dry_run=False))

    assert rc == 0                                   # ran to completion
    assert (tmp_vault / "HOT.md").exists()           # later phases still ran
    today = dt.date.today().isoformat()
    report = (tmp_vault / "00-sources" / f"gardener-report-{today}.md").read_text()
    assert "Ollama-Aussetzer" in report


def test_judge_returns_empty_on_transient_failure_but_trips_the_breaker():
    from gardener.ollama import OllamaClient, OllamaError, OllamaUnavailable

    client = OllamaClient()
    calls = {"n": 0}

    def boom(path, payload):
        calls["n"] += 1
        raise OllamaError("timed out")

    client._post = boom

    # tolerated: the caller sees "could not judge this pair"
    for _ in range(client.MAX_CONSECUTIVE_FAILURES - 1):
        assert client.judge("sys", "prompt") == {}

    with pytest.raises(OllamaUnavailable):
        client.judge("sys", "prompt")                # Ollama really is down


def test_embed_failure_skips_one_note_instead_of_the_run(tmp_vault, store):
    from gardener.ollama import OllamaError

    notes = load_notes(tmp_vault)

    class HalfDeadEmbedder(FakeOllama):
        def embed(self, text):
            if text.startswith("Beta"):     # embed_text starts with the title
                raise OllamaError("timed out")
            return [1.0, 0.0, 0.0]

    vectors = linking.embed_notes(notes, store, HalfDeadEmbedder())

    assert "10-global/beta.md" not in vectors        # sat this run out
    assert "10-global/alpha.md" in vectors           # the rest still embedded


def test_second_full_run_touches_no_note_and_no_queue(tmp_vault, isolated):
    """Idempotency: a second run must not rewrite a single note, MOC or queue
    entry. Only the per-run artifacts (HOT.md, the report, the health report)
    are regenerated - they exist to describe THAT run."""
    assert cli.run(args_for(tmp_vault, dry_run=False)) == 0
    first = snapshot(tmp_vault)

    assert cli.run(args_for(tmp_vault, dry_run=False)) == 0
    second = snapshot(tmp_vault)

    def regenerated(rel: str) -> bool:
        name = Path(rel).name
        return (name in ("HOT.md", "OPEN-QUESTIONS.md")
                or name.startswith(("gardener-report-", "brain-health-")))

    changed = {k for k in first if first[k] != second.get(k) and not regenerated(k)}
    assert changed == set()
    assert set(second) - set(first) == set()      # no new files on a rerun
    # the review-queue in particular must not grow the same lines again
    assert first["review-queue.md"] == second["review-queue.md"]


def test_links_to_corpus_excluded_files_are_not_dead(tmp_path):
    """INDEX.md/MOC.md are excluded from the corpus (never rewritten/judged)
    but they are legitimate link targets: a [[brain3 MOC]] link must not be
    reported dead. Regression: 63 false dead-link findings on the real vault."""
    from gardener.audit import dead_links
    from gardener import vault as v
    (tmp_path / "INDEX.md").write_text("---\ntitle: INDEX\n---\nroot\n")
    proj = tmp_path / "20-projects" / "demo"
    proj.mkdir(parents=True)
    (proj / "MOC.md").write_text("---\ntitle: demo MOC\n---\nhub\n")
    (proj / "note.md").write_text(
        "---\ntitle: eine Note\n---\nText.\n\n## Relations\n"
        "- part-of [[demo MOC]]\n- relates-to [[INDEX]]\n- relates-to [[wirklich tot]]\n")
    notes = v.load_notes(tmp_path)
    dead = dead_links(notes, tmp_path)
    assert [t for _, t in dead] == ["wirklich tot"]
