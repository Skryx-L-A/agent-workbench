from __future__ import annotations

from pathlib import Path

import pytest

from gardener.store import Store


def make_note(root: Path, rel: str, title: str, body: str) -> Path:
    p = root / rel
    p.parent.mkdir(parents=True, exist_ok=True)
    p.write_text(f"---\ntitle: {title}\ntype: note\n---\n\n{body}\n")
    return p


@pytest.fixture
def tmp_vault(tmp_path: Path) -> Path:
    v = tmp_path / "vault"
    make_note(v, "10-global/alpha.md", "Alpha", "Alpha talks about Beta topics.")
    make_note(v, "10-global/beta.md", "Beta", "Beta content, standalone.")
    make_note(v, "20-projects/demo/gamma.md", "Gamma",
              "Gamma links to [[Alpha]] already.\n\nStand: 2026-06")
    make_note(v, "90-secrets/secret.md", "Secret", "API_KEY=xyz")
    make_note(v, "_meta/templates/note.md", "Template", "{{title}}")
    (v / ".obsidian").mkdir()
    (v / ".obsidian" / "graph.md").write_text("obsidian internals")
    (v / "_meta" / "tools").mkdir(parents=True)
    (v / "_meta" / "tools" / "helper.md").write_text("tool docs")
    make_note(v, "00-sources/gardener-report-2026-01-01.md", "old report", "generated")
    return v


@pytest.fixture
def store(tmp_path: Path) -> Store:
    return Store(tmp_path / "state" / "test.db")


class FakeOllama:
    """Deterministic Ollama stand-in: fixed embeddings + scripted verdicts."""

    def __init__(self, embeddings: dict[str, list[float]] | None = None,
                 verdicts: list[dict] | None = None,
                 vision: str | None = None):
        self.embeddings = embeddings or {}
        self.verdicts = list(verdicts or [])
        self.vision = vision            # None -> no vision model available
        self.embed_calls: list[str] = []
        self.judge_calls: list[str] = []
        self.vision_calls: list[str] = []

    def embed(self, text: str) -> list[float]:
        self.embed_calls.append(text)
        for key, vec in self.embeddings.items():
            if key in text:
                return vec
        return [1.0, 0.0, 0.0]

    def judge(self, system: str, prompt: str) -> dict:
        self.judge_calls.append(prompt)
        if self.verdicts:
            return self.verdicts.pop(0)
        return {"link": False, "reason": "default no"}

    def big_model_loaded(self):
        return None

    def has_model(self, name: str) -> bool:
        return self.vision is not None

    def describe_image(self, image_b64: str, prompt: str, model=None) -> str:
        self.vision_calls.append(prompt)
        return self.vision or ""
