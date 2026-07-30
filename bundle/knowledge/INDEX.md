---
title: INDEX
type: note
permalink: main/index
---

# Knowledge Vault — Index

One markdown vault, one graph. Source of truth for everything {{USER_NAME}} + agents know.

## Branches
- `00-inbox/` — drop zone; `tools/ingest` converts files/URLs into clean notes
- `10-global/` — user profile, standing rules/feedback, cross-project knowledge
- `20-projects/<project>/` — per-project knowledge: {{PROJECT_LIST}}
- `90-secrets/` — gitignored, never synced; secrets notes only
- `templates/note.md` — frontmatter + typed-link convention

## How to query
- Grep: `rg -i "<term>" ~/Knowledge --glob '!90-secrets/**'`
- Semantic + graph (optional): Basic Memory MCP (`basic-memory` tools) over this vault

## Rules
- Auto-search this vault before tasks; write + link a distilled session note after.
- Never put secrets outside `90-secrets/`. Never sync `90-secrets/` anywhere.

## Staged loading (Brain 2.0)
Every session automatically gets (SessionStart hook, `tools/session-context.sh`):
`INDEX.md` (this file) + `CRITICAL-FACTS.md` (~120-token mandatory facts) + `HOT.md`
(recent-context cache, regenerated nightly by the gardener) + — if the working directory
matches a project — that project's `20-projects/<p>/MOC.md` (curated entry point with the
project's most important notes). Everything else stays "pull": query with `rg` or Basic
Memory MCP.

## Two note classes
- **Session/source notes** (`type: session`, raw material from `00-inbox/`): immutable
  archive, never rewritten after the fact.
- **Topic/fact notes** (`type: note/reference/decision`, everything else): rewrite-over-
  append — new knowledge rewrites the existing text, contradictions get resolved in the
  prose instead of appended. Details + recency-marker convention (`Stand: YYYY-MM`):
  `templates/note.md`.
