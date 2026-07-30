# agent-workbench

A working environment for orchestrating coding agents: one lead session that plans and verifies,
several worker panes that do the work in parallel, and the machinery that keeps that from falling
apart — a context guard, a result protocol, enforcing hooks, and a model registry that treats
every harness as data rather than a code branch.

It is not tied to one vendor. Claude Code, Codex, aider, opencode, Antigravity and local models
via Ollama are all just entries in the registry; a purely local, zero-cost setup runs the same
way as a subscription one.

**→ Setting it up: hand [`AGENT-SETUP.md`](AGENT-SETUP.md) to a coding agent and tell it to work
through the file.** That is the intended path. Doing it by hand is possible and documented, but
the point of the repo is that an agent installs it for you.

---

## What you have afterwards

- **An orchestrator session** in tmux: your lead agent in one pane, worker panes beside it, all
  visible at once.
- **Workers you spawn by name and model.** `claude-worker review sonnet5:high ~/project "…"`
  opens a pane, waits for the harness to actually be ready, types the task, and verifies that it
  was submitted. Same name later means same pane, same context.
- **A context guard** that watches every pane's context usage, makes a worker write a handoff
  before it fills up, compacts it, and sends it back to work — because nothing can compact
  itself, and a full agent does not fail loudly, it just quietly gets worse.
- **A result protocol.** Every worker writes its outcome to a file. You wait on the file, not on
  what the pane looks like: a spinner is not a status.
- **A model registry.** Harnesses, providers and models are JSON, not code. `wb-state models
  discover` imports whatever your installed CLIs and providers currently offer, so a newly pulled
  local model shows up on its own.
- **Enforcing hooks** that block the mistakes agents actually make: secrets in a command line,
  a kill pattern wide enough to hit someone else's process, a commit that was never asked for.
- **A knowledge-base skeleton** — layout, rules, templates, tooling. Empty. The notes are yours.
- **The workbench itself, in source.** The VS Code extension ships prebuilt as a `.vsix`, and
  its TypeScript sits next to it with 319 tests — read it, change it, rebuild it.

## Requirements

Everything the setup can install, it installs. These it cannot:

| Needed | Why | Cost |
|---|---|---|
| git, tmux, python3 | the workbench is built on them | free |
| An agent CLI | at least one, or there is nothing to orchestrate | see below |
| A subscription or API key | only for cloud harnesses | varies |
| Ollama + a local model | only for the local lane; ~6 GB for a small model, ~25 GB for a 35B | free |
| WSL2 on Windows | tmux has no native Windows equivalent | free |

A setup with no account at all is a supported path: Ollama plus the local worker lane runs
entirely on your machine.

## Quick start

```bash
git clone https://github.com/<your-github-user>/agent-workbench.git
cd agent-workbench
./bootstrap.sh --dry-run     # see what it would do
./bootstrap.sh               # do it
./verify.sh                  # eleven checks, PASS/FAIL, no green tick over gaps
```

On Windows: `powershell -ExecutionPolicy Bypass -File .\bootstrap.ps1` — see the caveat below.

## Honest limitations

- **The Windows path is untested.** `bootstrap.ps1` was written against the official WSL2 and
  VS Code documentation and has never run on a real Windows machine. Read it first.
- **The shipped role prompts are in German.** They are the operating instructions for the lead
  agent and its workers, and they work as they are, but a non-German speaker will want to
  translate or replace `bundle/dot-claude/roles/`. Any agent can do that in one pass.
- **The registry ships with the harnesses that were actually measured** on macOS and Linux.
  A harness you add yourself needs its ready pattern measured once (`wb-harness-probe`), because
  a guessed pattern produces workers that receive nothing and never say so.
- **No telemetry, no phone-home, and no bundled credentials.** The repo contains no keys and no
  personal data; it is mechanics only.

## Optional layers

Each is off unless you ask for it, and the workbench runs without all of them.

- [**Local media generation**](docs/MEDIA.md) — images, speech, short video and transcription on
  your own hardware, so an agent that needs an asset does not reach for a paid API.
- [**Background services**](docs/SERVICES.md) — shared MCP servers instead of one per session,
  rate-limit recovery, vault backups, and the protected-service list that keeps a job from being
  displaced.
- [**Two machines**](docs/CROSS-MACHINE.md) — run work on whichever box fits it, without either
  agent stepping on the other's GPU.
- **A knowledge base** — structure, rules, templates and tooling for a linked markdown vault,
  plus the hooks that make an agent search it before it starts and write to it when it is done.
- [**Design skills**](docs/DESIGN.md) — the part where an agent has to produce something a person
  will look at: a build procedure from brief to deliverable, a two-pass review before anything
  ships, and a document/deck skill that never calls a PDF finished before every page has been
  rendered to an image and looked at. Two companion tools are optional; the skills say what to do
  by hand when they are absent.

## What `bootstrap.sh` actually does

In order, and every step is skippable and repeatable:

1. **Asks you six things** — which agent CLIs, which subscriptions or keys, local models yes/no,
   knowledge base yes/no, whether agents may run commands without confirming, and whether there
   is a second machine. Answers are stored; a second run does not ask again.
2. **Installs what is missing** via your package manager, after asking for `sudo`.
3. **Writes the agent configuration**: the rule file every agent reads, the orchestrator and
   worker roles, nine skills, the commands, and nine enforcing hooks — made executable, because
   a hook without the execute bit fails silently.
4. **Installs the tools** into `~/.local/bin`: worker spawners, context guard, registry, session
   management, cross-machine helpers, the media commands.
5. **Installs the model registry** so providers and harnesses exist from the first minute;
   `wb-state models discover --all` then finds whatever you actually have.
6. **Creates your knowledge base** — the structure, rules, templates and tooling, with no notes
   in it. It is yours to fill.
7. **Generates the instruction file for every other harness**, so a worker running under Codex
   or aider reads the same rules as one under Claude Code, from the filename that harness expects.
8. **Sets up the VS Code workbench**: its own profile, the extension, and on macOS a Dock app.
   From there: settings, sessions, new sessions, worker panes — local or on the second machine
   over SSH.
9. **`./verify.sh`** prints PASS/FAIL per check and exits non-zero if anything failed.

## Layout

```
AGENT-SETUP.md      the instruction file you hand to an agent
bootstrap.sh        macOS / Linux / WSL2 installer, idempotent, backs up what it replaces
bootstrap.ps1       Windows entry point (installs WSL2, then runs bootstrap.sh inside it)
verify.sh           self-test, PASS/FAIL per check
bundle/
  bin/              the wb-* tools, worker spawners, context guard, cross-machine helpers
  dot-claude/       agent config: roles, hooks, skills, commands
  dot-pi/           config for the local-model harness
  knowledge/        knowledge-base skeleton — structure and tooling, no notes
  bin-linux/        the media tools in their CUDA variants
  services/         launchd / systemd templates for the optional background services
  workbench/        the VS Code extension, its installer, and its full source
    source/         extension TypeScript + the shell test suites — build it yourself
docs/               media stack, services, two-machine setup
```

## License

MIT. See [LICENSE](LICENSE).
