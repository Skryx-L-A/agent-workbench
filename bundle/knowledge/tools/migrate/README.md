---
title: brain4 migration tool
type: reference
branch: _meta/tools/migrate
permalink: main/tools/migrate/readme-1
---

Migration tool for the Brain 4.0 vault re-layout. Contract:
[[BRAIN4-PLAN]] (`20-projects/brain3/BRAIN4-PLAN.md`).

## Use

```sh
python3 tools/migrate/brain4.py plan    --vault /path/to/copy   # manifest only, writes nothing else
python3 tools/migrate/brain4.py apply   --vault /path/to/copy   # git mv
python3 tools/migrate/brain4.py rewrite --vault /path/to/copy   # frontmatter v4 + path references
python3 tools/migrate/brain4.py verify  --vault /path/to/copy   # exit != 0 on any violation
```

Stdlib only, no dependencies. Every phase is idempotent: a second run assigns no
new ids and changes no bytes.

Built-in guards, so that skipping the runbook cannot cost data:

- `apply` refuses a dirty working tree (the manifest is ignored, `plan` writes it
  moments earlier) and writes a rollback copy to
  `~/.local/trash-snapshots/<date>-brain4/` — full worktree plus
  `HEAD-before-brain4.txt`, without `90-secrets` and without `.venv`.
  `--snapshot-dir` moves it, `--no-snapshot` skips it (throwaway copies only).
  Two states are not "dirty" and do not block:
  - **staged renames the plan predicted** = a run that was interrupted (SIGKILL,
    closed laptop). `apply` says so and carries out the moves still missing.
    Committing that state instead would commit half a migration, so the message
    never suggests it.
  - **untracked files no move touches** — Obsidian, the Basic-Memory sync or a
    second agent writing during the run. They are named and left alone; they are
    in the snapshot, and a rollback's `git clean -fd` would remove them.

  Modified or deleted tracked files still block: they would be swept into the
  migration commit.
- Order: **check, then snapshot, then write**, in `apply` and in `rewrite`.
  `apply` verifies beforehand that every directory it moves in or out of is
  writable; `rewrite` computes all changes first and writes only once every
  target file is writable. One read-only file therefore costs a clear refusal
  with nothing changed, not a half-migrated vault. Should the OS refuse a rename
  anyway, `apply` puts this run's moves back before it exits.
- `rewrite` refuses when the manifest records no moves, i.e. the vault is
  already migrated. The path rules are unconditional and would hit text that
  names the old paths on purpose — the `LOG.md` entry documenting the rename,
  the `STATUS.md` instruction telling people what to change. `--force` overrides.
- `verify` without a manifest says so and exits 2 (2 = cannot run, 1 = the vault
  violates a check). Deleting the manifest is the last step of the runbook, so
  this is the normal state after a finished migration. `--vault ~/Knowledge` is refused unless
`--allow-real-vault` is passed as well, so a mistyped path cannot hit the real
vault. The manifest lands at `<vault>/.brain4-manifest.json` (vault root, the one
directory `apply` never renames); delete it after the migration commit.

Tests: `uv run --with pytest python -m pytest tools/migrate/tests -q`. They build
their own vaults under tmp_path and never read `~/Knowledge`.

## What it does not do

- `90-secrets/` is excluded from every walk. `verify` proves the tree is
  unchanged from name/size/mtime only - the contents are never read.
- `_meta/templates/**` gets path rewrites but no `id`: an id in a template
  propagates into every note copied from it, producing colliding ids by design.
  Same for generated tool output (`logs/`, `results/`, `state/`).
- Nothing outside the vault is touched. The externally registered hook paths
  (see below) must be fixed by hand.

## Must be fixed outside the vault, or the migration breaks them

`rewrite` is vault-scoped and cannot reach these:

- `~/.claude/settings.json` - three hooks registered by absolute path:
  `$HOME/Knowledge/tools/session-context.sh`, `tools/hooks/auto-recall.sh`,
  `tools/hooks/read-tracking.sh` -> `$HOME/Knowledge/_meta/tools/...`
- `~/.local/bin/brain` - `uv run --project "$HOME/Knowledge/tools/braincli"`
- `~/.local/bin/ai-scout` - writes `$HOME/Knowledge/00-inbox/ai-scout-*.md`
- `~/.local/bin/check-ollama-kv-ssd` - marker in `$HOME/Knowledge/00-inbox/`
- every `.venv` under `_meta/tools/*` - 16 console scripts carry the shebang
  `#!$HOME/Knowledge/tools/braincli/.venv/bin/python3`. Moving a venv
  does not rewrite it; run `uv sync` per tool after the migration.
- the vault's own git hook install (`tools/git-hooks/pre-push` is copied into
  `.git/hooks/`; the copy keeps working, but reinstall from the new path).

## Gardener / eval patch

`gardener-brain4-taxonomy.patch` applies to the tree **after** `apply` +
`rewrite`. It is not applied automatically because `tools/gardener/` and
`tools/eval/` belong to other work in flight.

```sh
git apply tools/migrate/gardener-brain4-taxonomy.patch   # from the vault root, post-migration
```

Required - these fail without it:

- `_meta/tools/eval/retrieval_eval/search.py`, `_meta/tools/eval/tests/test_queries.py`:
  `parents[3]` -> `parents[4]`, and `VAULT_ROOT / "tools" / "braincli"` ->
  `VAULT_ROOT / "_meta" / "tools" / "braincli"`. The vault root is computed by
  path depth, and `_meta` adds a level - measured: 3 eval tests fail without it,
  all 15 pass with it.
- `_meta/tools/ingest/ingest.py`: `parents[2]` -> `parents[3]`, same reason.
  (Dieses Werkzeug wurde am 2026-07-29 durch `brain ingest` abgeloest und geloescht;
  der Eintrag bleibt als Protokoll der Migration stehen.)
- `_meta/tools/gardener/gardener/config.py`: `MINED_DIR = "00-sources"` ->
  `"00-sources/mined"`, otherwise mined notes land beside the reports instead of
  in `mined/`.

Recommended - the suite is green either way, which is the problem:

- `config.EXCLUDE_DIRS`: `{"90-secrets", ".obsidian", "templates", ".git", "tools"}`
  -> `{"90-secrets", ".obsidian", ".git", "_meta"}`. The old set still excludes
  the right files *by accident*, because exclusion matches path segment names and
  `_meta/tools/x` still contains a `tools` segment. But `_meta` itself is not
  excluded, so anything placed directly in `_meta/` would enter the linking
  corpus.
- `gardener/tests/conftest.py`, `test_vault.py`, `test_sidecar.py`: the fixtures
  create and assert on `tools/` and `Templates/`, directories that no longer
  exist after the migration. They keep passing while testing nothing.