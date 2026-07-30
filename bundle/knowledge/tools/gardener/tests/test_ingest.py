import datetime as dt

from gardener import config, extract, ingest
from gardener.queue import ReviewQueue
from gardener.vault import UnsafeWriteError, VaultWriter, load_notes, parse_note

from .conftest import FakeOllama

TODAY = dt.date(2026, 7, 12)


def drop(vault, name, content=b"data"):
    d = vault / config.DROP_DIR
    d.mkdir(parents=True, exist_ok=True)
    p = d / name
    p.write_bytes(content)
    return p


def test_drop_file_lands_in_assets_with_stub(tmp_vault):
    drop(tmp_vault, "demo-handbuch.txt", b"Alles ueber das demo Projekt. " * 5)
    writer = VaultWriter(tmp_vault)
    client = FakeOllama(verdicts=[{"summary": "Handbuch zum Demo-Projekt."}])
    res = ingest.ingest_drop(tmp_vault, writer, client, ReviewQueue(writer), TODAY)

    asset = tmp_vault / "20-projects/demo/_assets/demo-handbuch.txt"
    stub = tmp_vault / "20-projects/demo/_assets/demo-handbuch.md"
    assert asset.exists()                       # branch guessed from the filename
    assert not (tmp_vault / config.DROP_DIR / "demo-handbuch.txt").exists()
    assert stub.exists()
    note = parse_note(tmp_vault, stub)
    assert note.ntype == "asset"
    assert note.fm["path"] == "_assets/demo-handbuch.txt"
    assert note.fm["branch"] == "20-projects/demo"
    assert "Handbuch zum Demo-Projekt." in note.text
    assert res.ingested == [("demo-handbuch.txt", "20-projects/demo/_assets/demo-handbuch.txt")]


def test_unclear_branch_goes_to_global_and_review_queue(tmp_vault):
    drop(tmp_vault, "scan.bin", b"\x00\x01binary")
    writer = VaultWriter(tmp_vault)
    queue = ReviewQueue(writer)
    res = ingest.ingest_drop(tmp_vault, writer, FakeOllama(), queue, TODAY)
    assert (tmp_vault / "10-global/_assets/scan.bin").exists()
    q = (tmp_vault / "review-queue.md").read_text()
    assert "Zielbranch unklar" in q
    assert "Asset ohne Beschreibung" in q      # no extractor for .bin
    assert res.queued


def test_stub_frontmatter_is_a_single_block(tmp_vault):
    drop(tmp_vault, "x.bin", b"\x00")
    writer = VaultWriter(tmp_vault)
    ingest.ingest_drop(tmp_vault, writer, FakeOllama(), ReviewQueue(writer), TODAY)
    from gardener import frontmatter
    text = (tmp_vault / "10-global/_assets/x.md").read_text()
    blocks, _body = frontmatter.split_blocks(text)
    assert len(blocks) == 1


def test_image_uses_local_vision_when_available(tmp_vault, monkeypatch):
    drop(tmp_vault, "diagram.png", b"\x89PNG fake")
    writer = VaultWriter(tmp_vault)
    client = FakeOllama(vision="Ein Architekturdiagramm mit drei Boxen.")
    ingest.ingest_drop(tmp_vault, writer, client, ReviewQueue(writer), TODAY)
    stub = (tmp_vault / "10-global/_assets/diagram.md").read_text()
    assert "Architekturdiagramm" in stub
    assert client.vision_calls


def test_image_without_vision_model_gets_placeholder(tmp_vault):
    drop(tmp_vault, "photo.png", b"\x89PNG fake")
    writer = VaultWriter(tmp_vault)
    ingest.ingest_drop(tmp_vault, writer, FakeOllama(vision=None),
                       ReviewQueue(writer), TODAY)
    stub = (tmp_vault / "10-global/_assets/photo.md").read_text()
    assert ingest.PLACEHOLDER in stub
    assert "Asset ohne Beschreibung" in (tmp_vault / "review-queue.md").read_text()


def test_enrich_existing_stub_without_description(tmp_vault):
    assets = tmp_vault / "10-global" / "_assets"
    assets.mkdir(parents=True)
    (assets / "paper.txt").write_text("Ein langer Text ueber Embeddings.")
    (assets / "paper.md").write_text(
        "---\ntitle: paper\ntype: asset\nbranch: 10-global\n"
        "path: _assets/paper.txt\nmime: text/plain\n---\n\n"
        f"{ingest.DESC_START}\n{ingest.PLACEHOLDER}\n{ingest.DESC_END}\n")
    notes = load_notes(tmp_vault)
    writer = VaultWriter(tmp_vault)
    client = FakeOllama(verdicts=[{"summary": "Paper ueber Embeddings."}])
    res = ingest.enrich_stubs(tmp_vault, notes, writer, client, ReviewQueue(writer),
                              today=TODAY)
    assert res.enriched == ["10-global/_assets/paper.md"]
    assert "Paper ueber Embeddings." in (assets / "paper.md").read_text()
    assert ingest.PLACEHOLDER not in (assets / "paper.md").read_text()


def test_dropped_markdown_moves_to_inbox(tmp_vault):
    drop(tmp_vault, "notiz.md", b"---\ntitle: Notiz\n---\n\ntext\n")
    writer = VaultWriter(tmp_vault)
    ingest.ingest_drop(tmp_vault, writer, FakeOllama(), ReviewQueue(writer), TODAY)
    assert (tmp_vault / "00-sources/notiz.md").exists()
    assert not (tmp_vault / config.DROP_DIR / "notiz.md").exists()


def test_drop_zone_is_not_part_of_the_corpus(tmp_vault):
    drop(tmp_vault, "raw.md", b"---\ntitle: Raw\n---\n\nnot yet a note\n")
    assert not any(n.rel.startswith(config.DROP_DIR) for n in load_notes(tmp_vault))


def test_assets_write_gate_rejects_targets_outside_assets(tmp_vault):
    writer = VaultWriter(tmp_vault)
    import pytest
    with pytest.raises(UnsafeWriteError):
        writer.move_asset(tmp_vault / "x", tmp_vault / "10-global" / "loose.bin")
    with pytest.raises(UnsafeWriteError):
        writer.move_asset(tmp_vault / "x", tmp_vault / "90-secrets/_assets/k.bin")


def test_guess_branch_prefers_filename_over_content(tmp_vault):
    branch, confident = ingest.guess_branch(tmp_vault, "demo-report.pdf", "")
    assert (branch, confident) == ("20-projects/demo", True)
    branch, confident = ingest.guess_branch(tmp_vault, "irgendwas.pdf", "nichts")
    assert (branch, confident) == ("10-global", False)


def test_extract_degrades_without_tools(tmp_path):
    assert extract.pdf_text(tmp_path / "missing.pdf") == ""
    assert extract.summarize(None, "text") == ""
    assert extract.describe_image(None, tmp_path / "x.png") == ""


def test_oversized_image_never_reaches_the_vision_model(tmp_path):
    big = tmp_path / "huge.png"
    big.write_bytes(b"\x89PNG" + b"\x00" * (config.MAX_VISION_BYTES + 1))
    client = FakeOllama(vision="sollte nie aufgerufen werden")
    assert extract.describe_image(client, big) == ""
    assert client.vision_calls == []


def _asset_note(tmp_path, body: str):
    from gardener.vault import parse_note
    p = tmp_path / "20-projects" / "x" / "_assets" / "a-asset.md"
    p.parent.mkdir(parents=True, exist_ok=True)
    p.write_text("---\ntitle: a (asset)\ntype: asset\nbranch: 20-projects/x\n"
                 "path: _assets/a.md\n---\n\n" + body, encoding="utf-8")
    return parse_note(tmp_path, p)


def test_a_hand_written_description_without_markers_counts(tmp_path):
    """Fehlender Marker heisst 'nicht von uns geschrieben', nicht 'leer'.

    Fuenf Sidecars im Vault trugen eine ordentliche Beschreibung ohne Marker.
    Sie als unbeschrieben zu behandeln liess den Gardener jeden Lauf das Modell
    fragen, an ihrem unbekannten Mime scheitern und dieselben fuenf Zeilen neu
    in die Queue stellen (gemessen 2026-07-29).
    """
    n = _asset_note(tmp_path,
                    "Für künftige Sessions: öffnen für die vollständigen, code-seitig zu "
                    "erzwingenden Risiko-Leitplanken des Paper-Traders.\n\n"
                    "part-of [[beispielprojekt]]\n")
    assert not ingest.needs_description(n)


def test_an_empty_stub_still_needs_one(tmp_path):
    n = _asset_note(tmp_path, "part-of [[irgendwas]]\n")
    assert ingest.needs_description(n)


def test_the_placeholder_does_not_count_as_a_description(tmp_path):
    n = _asset_note(tmp_path, ingest.PLACEHOLDER + "\n\npart-of [[x]]\n")
    assert ingest.needs_description(n)


def test_an_empty_marker_block_still_needs_one(tmp_path):
    n = _asset_note(tmp_path,
                    f"{ingest.DESC_START}\n\n{ingest.DESC_END}\n\n"
                    "Sonstiger langer Text, der nicht die Beschreibung ist und "
                    "trotzdem ueber achtzig Zeichen lang ausfaellt.\n")
    assert ingest.needs_description(n)
