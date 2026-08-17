# agent-workbench

A desktop workbench for running coding agents side by side. One lead session plans and verifies,
several worker panes do the work in parallel, and the machinery around them keeps that from
falling apart: a context guard, a result protocol, hooks that block the mistakes agents actually
make, and a model registry that treats every harness as data instead of a code branch.

The window is an Electron program that drives tmux over its control mode. Everything it shows —
sessions, worker panes, output, the editor, the chat stage — is a live view on real tmux panes,
so the same work is reachable from a terminal on the same machine or over SSH from another one.

It is not tied to one vendor. Claude Code, Codex, aider, opencode and local models via Ollama are
entries in a JSON registry. A setup with no account at all is a supported path: Ollama plus the
local worker lane runs entirely on your own hardware.

## What is in here

```
app/         the workbench itself — Electron main process, preload bridges, renderer, and
             awb-ctl, a dependency-free CLI that talks to the running program over a socket
extension/   sixteen modules the app imports rather than duplicates. They started life in a
             VS Code extension and still carry its directory name; there is exactly one copy
             of each, and this is it
shell/       59 command-line tools: worker spawners, context guard, model registry, session
             management, budget and quota, cross-machine helpers
claude/      what an agent reads before it starts — two role prompts, thirteen rule files,
             sixteen skills, eighteen hooks, and templates for CLAUDE.md and settings.json
knowledge/   an empty knowledge-base skeleton: branch layout, note templates, and the tooling
             that indexes and searches it. The notes are yours; none ship here
beispiele/   three worked examples — a project rule file, a note written to the template, and
             an AGENTS.md for a harness that reads that filename instead
INSTALL.md   how to put all of it on your machine
```

## What you have after installing

- **A workbench window** with your sessions on one side and every worker pane visible at once.
- **Workers you spawn by name and model.** `claude-worker review sonnet5:high ~/project "…"`
  opens a pane, waits until the harness is genuinely ready, types the task, and checks that it
  was submitted. The same name later means the same pane with the same context.
- **A context guard** that watches how full each pane's context is, makes a worker write a
  handoff before it runs out, compacts it, and sends it back to work. Nothing can compact
  itself, and an agent with a full context does not fail loudly — it quietly gets worse.
- **A result protocol.** Every worker writes its outcome to a file, and you wait on the file
  rather than on what the pane looks like. A spinner is not a status.
- **A model registry.** Harnesses, providers and models are JSON. `wb-state models discover`
  imports whatever your installed CLIs currently offer, so a model you just pulled shows up on
  its own.
- **Hooks that refuse.** A secret on a command line, a `pkill` pattern wide enough to hit
  someone else's process, a commit nobody asked for, a screenshot of a live window — each is
  blocked at the point where it would happen.
- **A knowledge base skeleton.** Structure, templates and tooling, empty.

## Requirements

| Needed | Why |
|---|---|
| git, tmux, python3 | the tools are built on them |
| Node.js 22 or newer | the workbench is an Electron program and is built from source |
| At least one agent CLI | otherwise there is nothing to orchestrate |
| A subscription or API key | only for cloud harnesses |
| Ollama and a local model | only for the local lane — roughly 6 GB for a small model |
| WSL2 on Windows | tmux has no native Windows equivalent |

## Honest limitations

- **The role prompts, rules and code comments are in German.** They work as they are, but if you
  do not read German you will want to translate `claude/roles/` and `claude/regeln/` first. Any
  agent does that in one pass.
- **Some tools describe a two-machine setup that is not yours.** `wb-sync-setup`, `peer-shot`,
  `wb-shot-remote` and `wb-ssh-worker` assume a second host reachable over SSH, and
  `wb-modell-proxy` assumes a local model server on it. They are inert without one, and the
  hostnames in them are placeholders you have to fill in. The same goes for the launchd property
  lists in `shell/` (macOS) and the unit files in `shell/systemd/` (Linux): they are templates
  with paths that belong to whoever installs them.
- **The registry ships with the harnesses that were actually measured** on macOS and Linux. A
  harness you add yourself needs its ready pattern measured once with `wb-harness-probe` —
  a guessed pattern produces workers that receive nothing and never say so.
- **Windows is untested.** The tools assume a POSIX shell and tmux; WSL2 is the path, and nobody
  has walked it end to end.
- **No telemetry, no phone-home, no bundled credentials.** There are no keys and no personal data
  in this repository. It is mechanics only.
- **The knowledge harvester reads your private transcripts, by design.** `knowledge/_meta/tools/`
  contains a harvesting pipeline that mines agent sessions for durable facts, and to do that it
  reads `~/.claude/projects/*.jsonl`, `~/.pi-workers/results/` and your project directories.
  Everything stays on your machine and nothing is sent anywhere, but you should know this before
  you start it rather than after. Its secret gate can be given extra patterns of your own —
  see the note below.

## Two files you should create before the first harvest

Neither is shipped, both are read from outside any repository, and both exist for the same reason:
a list of personal patterns is itself personal data, so it can never live next to the code that
uses it.

- `~/.config/gardener-dream/secret-patterns.local.json` — extra secret patterns for the harvester's
  gate. It already knows the common provider prefixes, private keys, JWTs and
  `password = <value>`-style assignments; what it cannot know are the shapes that only occur in
  your world. Template with the full explanation:
  `knowledge/_meta/tools/gardener/secret-patterns.local.json.example`. Without the file the gate
  runs with its built-in list, which is not an error and is not reported — which is exactly why
  this paragraph exists.
- `~/.config/agent-workbench/depersonalize.rules` — only needed if you ever publish an extract of
  your own setup. The publishing gate that produced this repository lives in the private source
  tree, not here, and it reads its replacement table from that path.

## Getting started

Read [INSTALL.md](INSTALL.md). Handing that file to a coding agent and telling it to work through
the steps is a reasonable way to do it, and it is how the setup is meant to spread.

## License

MIT. See [LICENSE](LICENSE).
