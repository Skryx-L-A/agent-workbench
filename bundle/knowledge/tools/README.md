---
id: 01KYMQ8BVBC0HVVJ0P51S6REJ7
schema: 4
title: README
type: note
permalink: main/_meta/tools/readme
class: meta
---

# Sidecar layer

Every non-`.md` file in the vault gets a sibling `<name>.<ext>.md` note (e.g.
`vertrag.pdf` -> `vertrag.pdf.md`) describing what it contains, so an agent can
skip opening the real file unless it actually needs to. Implementation:
`_meta/tools/gardener/gardener/sidecar.py`.

- **Extraction is local only**: pdftotext (PDF), macOS `textutil` (doc/rtf),
  direct read (text/CSV/JSON/code), Ollama vision `qwen3-vl:8b` (images),
  `ffprobe` metadata only for audio/video (no stt by default). A type nobody
  can extract still gets a sidecar - metadata plus a placeholder, never none.
- **Idempotent**: `sha256` in the sidecar's frontmatter decides. Unchanged ->
  skipped. Changed -> only the `<!-- wb:auto:start/end -->` block is
  regenerated; anything a human added outside it (e.g. wikilinks) survives.
  `human-edited: true` in the frontmatter protects a sidecar completely.
- **Legacy-safe**: pre-existing hand/ingest-written `_assets/*.md` stubs
  (`type: asset`, `_meta/templates/asset-stub.md` convention) are recognized and
  only get missing frontmatter fields added - their body is never touched.
- **Exclusions**: `.git`, `.obsidian`, `.claude`, `90-secrets`, `tools`,
  `__pycache__`, lock/cache files, plus a vault-root `.brainignore`
  (gitignore syntax).
- **CLI**: `brain sidecar scan|generate|check` (see `braincli/braincli/cli.py`);
  `check` exits non-zero on missing/stale sidecars (pre-commit gate). Also
  runs as the gardener phase `sidecar` inside a full `gardener --once` pass.