import datetime as dt
import http.server
import json
import subprocess
import threading
from pathlib import Path

import pytest

from braincli import ingest as ingest_mod
from braincli.search import SearchHit
from gardener import config, frontmatter


class FakeClient:
    """Stands in for OllamaClient: canned summaries, no network."""

    def __init__(self, summary="Kurze, dichte Zusammenfassung des Materials fuer Tests."):
        self.summary = summary
        self.judge_calls = []

    def judge(self, system, prompt):
        self.judge_calls.append((system, prompt))
        if "contradiction" in system.lower():
            return {"verdict": "compatible", "confidence": 0.0,
                    "claim_a": "", "claim_b": "", "why": ""}
        return {"summary": self.summary}

    def big_model_loaded(self):
        return None

    def has_model(self, name):
        return False

    def describe_image(self, image_b64, prompt, model=None):
        return ""

    def embed(self, text):
        h = hash(text)
        return [((h >> (8 * i)) & 0xFF) / 255.0 for i in range(8)]


@pytest.fixture(autouse=True)
def _no_real_search(monkeypatch):
    """Every ingest run calls find_related() -> search_mod.search(); stub it so
    no test ever opens a real socket to Ollama, regardless of what else is
    running on the machine (see CLAUDE.md: tests never depend on machine state)."""
    monkeypatch.setattr(ingest_mod.search_mod, "search", lambda vault, query, k=5: ([], False))


def _write_note(vault: Path, rel: str, title: str, body: str = "", extra_fm: str = "") -> Path:
    p = vault / rel
    p.parent.mkdir(parents=True, exist_ok=True)
    p.write_text(f"---\ntitle: {title}\ntype: note\n{extra_fm}---\n\n{body}\n", encoding="utf-8")
    return p


def _note_fields(path: Path) -> dict:
    fields, _ = frontmatter.parse(path.read_text(encoding="utf-8"))
    return fields


# ---------------------------------------------------------------------------
# Textdatei
# ---------------------------------------------------------------------------

def test_ingest_text_file_write(tmp_path: Path):
    src = tmp_path / "note.txt"
    src.write_text("Dies ist ein Test-Text ueber lokale Sprachmodelle.", encoding="utf-8")
    vault = tmp_path / "vault"
    vault.mkdir()

    outcome = ingest_mod.run_ingest(vault, str(src), write=True,
                                    check_contradict=False, client=FakeClient())

    assert outcome["duplicate"] is False
    assert outcome["kind"] == "file"
    assert outcome["content_kind"] == "text"
    assert outcome["extracted"] is True
    assert outcome["extractor"] == "ollama-judge"
    note_path = vault / outcome["note"]
    assert note_path.is_file()
    fields = _note_fields(note_path)
    assert fields["type"] == "source"
    assert fields["class"] == "source"
    assert fields["source-kind"] == "file"
    assert "Kurze, dichte Zusammenfassung" in note_path.read_text(encoding="utf-8")


# ---------------------------------------------------------------------------
# PDF (pdf_text monkeypatched: no dependency on a real PDF binary)
# ---------------------------------------------------------------------------

def test_ingest_pdf_file(tmp_path: Path, monkeypatch):
    src = tmp_path / "report.pdf"
    src.write_bytes(b"%PDF-fake")
    monkeypatch.setattr(ingest_mod.extract_mod, "pdf_text",
                        lambda path, max_chars=6000: "Inhalt des PDFs fuer den Test.")
    vault = tmp_path / "vault"
    vault.mkdir()

    outcome = ingest_mod.run_ingest(vault, str(src), write=True,
                                    check_contradict=False, client=FakeClient())

    assert outcome["content_kind"] == "pdf"
    assert outcome["extracted"] is True
    assert outcome["extractor"] == "pdftotext+ollama-judge"


# ---------------------------------------------------------------------------
# Unbekannter Typ: Notiz entsteht trotzdem, mit Vermerk
# ---------------------------------------------------------------------------

def test_ingest_unknown_type_still_creates_note(tmp_path: Path):
    src = tmp_path / "blob.xyz"
    src.write_bytes(b"\x00\x01\x02")
    vault = tmp_path / "vault"
    vault.mkdir()

    outcome = ingest_mod.run_ingest(vault, str(src), write=True,
                                    check_contradict=False, client=FakeClient())

    assert outcome["content_kind"] == "unknown"
    assert outcome["extracted"] is False
    assert outcome["extraction_error"]
    note_path = vault / outcome["note"]
    assert note_path.is_file()
    assert "nicht automatisch lesbar" in note_path.read_text(encoding="utf-8")


# ---------------------------------------------------------------------------
# Extraktion scheitert: kein Absturz, Fehler im JSON
# ---------------------------------------------------------------------------

def test_ingest_missing_file_reports_error_not_crash(tmp_path: Path):
    vault = tmp_path / "vault"
    vault.mkdir()
    missing = tmp_path / "does-not-exist.txt"

    outcome = ingest_mod.run_ingest(vault, str(missing), write=True,
                                    check_contradict=False, client=FakeClient())

    assert outcome["extracted"] is False
    assert "nicht gefunden" in outcome["extraction_error"]
    note_path = vault / outcome["note"]
    assert note_path.is_file()   # a note is still created, per contract


# ---------------------------------------------------------------------------
# stdin
# ---------------------------------------------------------------------------

def test_ingest_stdin(tmp_path: Path):
    vault = tmp_path / "vault"
    vault.mkdir()

    outcome = ingest_mod.run_ingest(vault, "-", write=True, check_contradict=False,
                                    client=FakeClient(), stdin_text="Notiz per stdin eingegeben.")

    assert outcome["kind"] == "stdin"
    assert outcome["content_kind"] == "stdin"
    assert outcome["extracted"] is True
    fields = _note_fields(vault / outcome["note"])
    assert fields["source-kind"] == "stdin"


def test_ingest_stdin_empty(tmp_path: Path):
    vault = tmp_path / "vault"
    vault.mkdir()

    outcome = ingest_mod.run_ingest(vault, "-", write=True, check_contradict=False,
                                    client=FakeClient(), stdin_text="   \n  ")

    assert outcome["extracted"] is False
    assert "leer" in outcome["extraction_error"]


# ---------------------------------------------------------------------------
# Ohne --write wird nichts geschrieben
# ---------------------------------------------------------------------------

def test_dry_run_writes_nothing(tmp_path: Path):
    src = tmp_path / "note.txt"
    src.write_text("Etwas Text.", encoding="utf-8")
    vault = tmp_path / "vault"
    vault.mkdir()

    outcome = ingest_mod.run_ingest(vault, str(src), write=False,
                                    check_contradict=False, client=FakeClient())

    assert outcome["dry_run"] is True
    assert not (vault / outcome["note"]).exists()
    assert list((vault / "00-sources").glob("**/*")) == [] if (vault / "00-sources").exists() else True
    assert not (vault / ingest_mod.INGEST_LOG_FILE).exists()
    assert outcome["log"] is None


# ---------------------------------------------------------------------------
# Zweiter Lauf derselben Quelle erkennt sie wieder
# ---------------------------------------------------------------------------

def test_second_run_of_same_source_is_recognized(tmp_path: Path):
    src = tmp_path / "note.txt"
    src.write_text("Wiederholter Inhalt.", encoding="utf-8")
    vault = tmp_path / "vault"
    vault.mkdir()

    first = ingest_mod.run_ingest(vault, str(src), write=True,
                                  check_contradict=False, client=FakeClient())
    assert first["duplicate"] is False

    second = ingest_mod.run_ingest(vault, str(src), write=True,
                                   check_contradict=False, client=FakeClient())
    assert second["duplicate"] is True
    assert second["note"] == first["note"]

    notes = list((vault / "00-sources").glob("*.md"))
    assert len(notes) == 1


# ---------------------------------------------------------------------------
# Protokollzeile entsteht
# ---------------------------------------------------------------------------

def test_write_appends_log_line(tmp_path: Path):
    src = tmp_path / "note.txt"
    src.write_text("Inhalt fuer das Protokoll.", encoding="utf-8")
    vault = tmp_path / "vault"
    vault.mkdir()

    outcome = ingest_mod.run_ingest(vault, str(src), write=True,
                                    check_contradict=False, client=FakeClient())

    log_path = vault / ingest_mod.INGEST_LOG_FILE
    assert log_path.is_file()
    lines = log_path.read_text(encoding="utf-8").splitlines()
    assert len(lines) == 1
    entry = json.loads(lines[0])
    assert entry["source"] == str(src)
    assert entry["note"] == outcome["note"]


# ---------------------------------------------------------------------------
# 90-secrets wird nie beruehrt
# ---------------------------------------------------------------------------

def test_never_touches_90_secrets(tmp_path: Path):
    src = tmp_path / "note.txt"
    src.write_text("Unabhaengiger Inhalt.", encoding="utf-8")
    vault = tmp_path / "vault"
    vault.mkdir()

    identity = ingest_mod.source_identity("file", str(src))
    fake_hash = ingest_mod.source_hash_of(identity)
    secret_path = vault / "90-secrets" / "decoy.md"
    secret_path.parent.mkdir(parents=True)
    secret_text = f"---\ntitle: Decoy\nsource-hash: {fake_hash}\n---\n\nGeheim.\n"
    secret_path.write_text(secret_text, encoding="utf-8")

    outcome = ingest_mod.run_ingest(vault, str(src), write=True,
                                    check_contradict=False, client=FakeClient())

    # a hash collision planted ONLY in 90-secrets must never be picked up as
    # a duplicate: find_duplicate must never have scanned that directory.
    assert outcome["duplicate"] is False
    assert secret_path.read_text(encoding="utf-8") == secret_text


# ---------------------------------------------------------------------------
# --branch / --title
# ---------------------------------------------------------------------------

def test_branch_and_title_override(tmp_path: Path):
    src = tmp_path / "note.txt"
    src.write_text("Inhalt.", encoding="utf-8")
    vault = tmp_path / "vault"
    vault.mkdir()

    outcome = ingest_mod.run_ingest(vault, str(src), branch="20-projects/demo",
                                    title_override="Mein Titel", write=True,
                                    check_contradict=False, client=FakeClient())

    assert outcome["title"] == "Mein Titel"
    assert outcome["branch"] == "20-projects/demo"
    fields = _note_fields(vault / outcome["note"])
    assert fields["branch"] == "20-projects/demo"
    assert fields["title"] == "Mein Titel"


# ---------------------------------------------------------------------------
# Related notes + backlinks (the one permitted edit to existing notes)
# ---------------------------------------------------------------------------

def test_related_notes_get_backlinked_on_write(tmp_path: Path, monkeypatch):
    vault = tmp_path / "vault"
    vault.mkdir()
    _write_note(vault, "10-global/existing-note.md", "Existing Note", "Vorhandener Inhalt.")

    monkeypatch.setattr(
        ingest_mod.search_mod, "search",
        lambda v, query, k=5: ([SearchHit(rel="10-global/existing-note.md",
                                          score=1.0, title="Existing Note")], False))

    src = tmp_path / "note.txt"
    src.write_text("Neuer Inhalt, der mit Existing Note verwandt ist.", encoding="utf-8")

    outcome = ingest_mod.run_ingest(vault, str(src), write=True,
                                    check_contradict=False, client=FakeClient())

    assert outcome["related_notes"] == [
        {"rel": "10-global/existing-note.md", "title": "Existing Note", "relation": "relates-to"}]
    assert outcome["touched_notes"] == ["10-global/existing-note.md"]

    new_note_text = (vault / outcome["note"]).read_text(encoding="utf-8")
    assert "relates-to [[Existing Note]]" in new_note_text

    existing_text = (vault / "10-global/existing-note.md").read_text(encoding="utf-8")
    assert f"relates-to [[{outcome['title']}]]" in existing_text
    assert "## Relations" in existing_text


def test_related_notes_backlink_not_written_in_dry_run(tmp_path: Path, monkeypatch):
    vault = tmp_path / "vault"
    vault.mkdir()
    original = _write_note(vault, "10-global/existing-note.md", "Existing Note",
                           "Vorhandener Inhalt.").read_text(encoding="utf-8")

    monkeypatch.setattr(
        ingest_mod.search_mod, "search",
        lambda v, query, k=5: ([SearchHit(rel="10-global/existing-note.md",
                                          score=1.0, title="Existing Note")], False))

    src = tmp_path / "note.txt"
    src.write_text("Neuer Inhalt.", encoding="utf-8")

    outcome = ingest_mod.run_ingest(vault, str(src), write=False,
                                    check_contradict=False, client=FakeClient())

    assert outcome["touched_notes"] == ["10-global/existing-note.md"]  # planned, not applied
    assert (vault / "10-global/existing-note.md").read_text(encoding="utf-8") == original


# ---------------------------------------------------------------------------
# Contradiction check (--no-contradict and the checked path)
# ---------------------------------------------------------------------------

def test_no_contradict_flag_skips_check(tmp_path: Path):
    src = tmp_path / "note.txt"
    src.write_text("Inhalt.", encoding="utf-8")
    vault = tmp_path / "vault"
    vault.mkdir()

    outcome = ingest_mod.run_ingest(vault, str(src), write=True,
                                    check_contradict=False, client=FakeClient())

    assert outcome["contradictions"] == {
        "checked": False, "skipped_reason": "--no-contradict",
        "pairs_checked": 0, "found": 0, "findings": []}


def test_contradiction_check_runs_and_reports(tmp_path: Path, monkeypatch):
    monkeypatch.setattr(config, "STATE_DIR", tmp_path / "gardener-state")
    src = tmp_path / "note.txt"
    src.write_text("Inhalt fuer Widerspruchspruefung.", encoding="utf-8")
    vault = tmp_path / "vault"
    vault.mkdir()

    outcome = ingest_mod.run_ingest(vault, str(src), write=True,
                                    check_contradict=True, client=FakeClient())

    cr = outcome["contradictions"]
    assert cr["checked"] is True
    assert cr["skipped_reason"] is None
    assert cr["found"] == 0   # FakeClient always answers "compatible"


# ---------------------------------------------------------------------------
# URL ingest against a local stub server (no real network)
# ---------------------------------------------------------------------------

class _StubHandler(http.server.BaseHTTPRequestHandler):
    PAGE = (b"<html><head><title>Stub Seite</title></head>"
           b"<body><script>ignored();</script><p>Inhalt der Stub-Seite fuer den Test.</p>"
           b"</body></html>")

    def do_GET(self):
        self.send_response(200)
        self.send_header("Content-Type", "text/html; charset=utf-8")
        self.end_headers()
        self.wfile.write(self.PAGE)

    def log_message(self, *args):
        pass


@pytest.fixture
def stub_server():
    server = http.server.HTTPServer(("127.0.0.1", 0), _StubHandler)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    try:
        yield f"http://127.0.0.1:{server.server_port}/"
    finally:
        server.shutdown()
        thread.join(timeout=5)


def test_ingest_url_via_stub_server(tmp_path: Path, stub_server):
    vault = tmp_path / "vault"
    vault.mkdir()

    outcome = ingest_mod.run_ingest(vault, stub_server, write=True,
                                    check_contradict=False, client=FakeClient())

    assert outcome["kind"] == "url"
    assert outcome["content_kind"] == "url"
    assert outcome["extracted"] is True
    assert outcome["title"] == "Stub Seite"
    note_text = (vault / outcome["note"]).read_text(encoding="utf-8")
    assert "Kurze, dichte Zusammenfassung" in note_text


def test_fetch_url_unreachable_reports_error(tmp_path: Path):
    text, title, err = ingest_mod.fetch_url("http://127.0.0.1:1/", timeout=2)
    assert text == ""
    assert err is not None


# ---------------------------------------------------------------------------
# YouTube (yt-dlp mocked: no real network)
# ---------------------------------------------------------------------------

def test_fetch_youtube_mocked(tmp_path: Path, monkeypatch):
    monkeypatch.setattr(ingest_mod.shutil, "which",
                        lambda name: "/usr/bin/yt-dlp" if name == "yt-dlp" else None)

    def fake_run(cmd, capture_output, text, timeout):
        out_tmpl = cmd[cmd.index("-o") + 1]
        vtt_path = Path(out_tmpl + ".de.vtt")
        vtt_path.write_text(
            "WEBVTT\n\n1\n00:00:00.000 --> 00:00:02.000\nHallo Welt.\n"
            "2\n00:00:02.000 --> 00:00:04.000\nHallo Welt.\n", encoding="utf-8")
        return subprocess.CompletedProcess(cmd, 0, stdout="", stderr="")

    monkeypatch.setattr(ingest_mod.subprocess, "run", fake_run)

    text, title, err = ingest_mod.fetch_youtube("https://youtu.be/xyz")
    assert err is None
    assert text.count("Hallo Welt.") == 1   # consecutive duplicate cue collapsed


def test_fetch_youtube_no_binary(monkeypatch):
    monkeypatch.setattr(ingest_mod.shutil, "which", lambda name: None)
    text, title, err = ingest_mod.fetch_youtube("https://youtu.be/xyz")
    assert text == ""
    assert "yt-dlp" in err


def test_vtt_to_text_strips_cues_and_dedupes():
    vtt = (
        "WEBVTT\n\n"
        "1\n00:00:00.000 --> 00:00:02.000\nHallo <c>Welt</c>.\n\n"
        "2\n00:00:02.000 --> 00:00:04.000\nHallo Welt.\n\n"
        "3\n00:00:04.000 --> 00:00:06.000\nZweiter Satz.\n")
    text = ingest_mod.vtt_to_text(vtt)
    assert text.splitlines() == ["Hallo Welt.", "Zweiter Satz."]


# ---------------------------------------------------------------------------
# Audio/video guard behavior (missing tools -> error, no crash)
# ---------------------------------------------------------------------------

def test_transcribe_audio_missing_stt(monkeypatch, tmp_path: Path):
    monkeypatch.setattr(ingest_mod.shutil, "which", lambda name: None)
    text, err = ingest_mod.transcribe_audio(tmp_path / "clip.wav")
    assert text == ""
    assert "stt" in err


def test_extract_audio_track_missing_ffmpeg(monkeypatch, tmp_path: Path):
    monkeypatch.setattr(ingest_mod.shutil, "which", lambda name: None)
    path, err = ingest_mod.extract_audio_track(tmp_path / "clip.mp4")
    assert path is None
    assert "ffmpeg" in err


# ---------------------------------------------------------------------------
# Note text builder (frontmatter shape)
# ---------------------------------------------------------------------------

def test_build_note_text_placeholder_on_empty_summary():
    result = ingest_mod.ExtractResult("unknown", error="kein Extraktor")
    text, note_id = ingest_mod.build_note_text(
        title="X", source="/tmp/x.bin", source_kind="file", branch="00-sources",
        source_hash="abc123", result=result, related=[], today=__import__("datetime").date(2026, 7, 29))
    fields, body = frontmatter.parse(text)
    assert fields["id"] == note_id
    assert fields["schema"] == "4"
    assert fields["type"] == "source"
    assert fields["class"] == "source"
    assert "nicht automatisch lesbar" in body


def test_an_ephemeral_source_path_is_marked_as_such():
    """Ein Scratchpad-Pfad traegt eine Session-ID und ist naechste Woche weg -
    als Herkunftsangabe sagt er nichts. Gemessen an der eingelesenen
    Second-Brain-Video-Notiz, deren `source:` auf eine Temp-Datei zeigte."""
    assert ingest_mod.is_ephemeral("/private/tmp/claude-501/x/scratchpad/transcript.txt")
    assert ingest_mod.is_ephemeral("/tmp/foo.txt")
    assert ingest_mod.is_ephemeral("/var/folders/km/abc/T/y.txt")
    assert not ingest_mod.is_ephemeral("/Users/alice/Downloads/bericht.pdf")
    assert not ingest_mod.is_ephemeral("https://example.com/a")


def test_the_note_says_when_the_path_is_gone():
    text, _ = ingest_mod.build_note_text(
        title="T", source="/tmp/x.txt", source_kind="file", branch="00-sources",
        source_hash="abc", result=ingest_mod.ExtractResult(summary="Inhalt.", content_kind="text"),
        related=[], today=dt.date(2026, 7, 29), ephemeral=True)
    assert "source-ephemeral: true" in text.lower()
    assert "existiert nicht mehr" in text


def test_a_known_origin_is_recorded_instead_of_the_copy():
    text, _ = ingest_mod.build_note_text(
        title="T", source="https://youtu.be/abc", source_kind="file",
        branch="00-sources", source_hash="abc",
        result=ingest_mod.ExtractResult(summary="Inhalt.", content_kind="text"), related=[],
        today=dt.date(2026, 7, 29), ephemeral=False)
    assert "https://youtu.be/abc" in text
    assert "source-ephemeral" not in text
    assert "existiert nicht mehr" not in text
