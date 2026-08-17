# agent-workbench

A desktop workbench for running coding agents side by side. One lead session plans and verifies,
several worker panes do the work in parallel, and the tooling around them keeps that from falling
apart: a context guard, a result protocol, and a model registry that treats every harness as data
instead of a code branch.

The window is an Electron program that drives tmux over its control mode. Everything it shows —
sessions, worker panes, output, the editor, the chat stage — is a live view on real tmux panes,
so the same work is reachable from a terminal on the same machine or over SSH from another one.

It is not tied to one vendor. Claude Code, Codex, aider, opencode and local models via Ollama are
entries in a JSON registry. A setup with no account at all is a supported path: Ollama plus the
local worker lane runs entirely on your own hardware.

## This repository is the workbench alone

It carries the program and the tools it needs to run. The wider setup that grew around it — the
agent rules, the skills, the hooks that block mistakes at the point where they would happen, the
knowledge base and its harvesting pipeline — lives in a second public repository,
**[agent-setup](https://github.com/Skryx-L-A/agent-setup)**. Both are built from the same private
source tree by the same script, so they never drift apart.

Take this one if you want the window and the worker machinery and intend to bring your own agent
configuration. Take the other one if you want the setup as it is actually used.

`<your-github-user>` stands for the account these repositories are hosted under. It is a
placeholder on purpose: the tool that extracts this repository from a working machine removes
account names everywhere, and it cannot tell the account in a public URL from the one on the
machine. The address bar you cloned from shows which one it is.

## What is in here

```
app/         the workbench itself — Electron main process, preload bridges, renderer, and
             awb-ctl, a dependency-free CLI that talks to the running program over a socket
extension/   the modules the app imports rather than duplicates. They started life in a
             VS Code extension and still carry its directory name; there is exactly one copy
             of each, and this is it
shell/       the command-line tools: worker spawners, context guard, model registry, session
             management, budget and quota, cross-machine helpers. Which of them ship here is
             worked out at build time — the tools the app itself calls, plus everything those
             call in turn — because a hand-kept list is right until the day somebody adds a
             call and forgets the list
claude/roles/  the two role prompts, one for the lead session and one for a worker. They are
             what makes a pane behave like part of a workbench instead of a lone agent
claude/statusline-command.sh  the status line the lead session prints
INSTALL.md   how to put all of it on your machine
```

## What is deliberately not in here

None of this is missing by accident, and none of it is needed to start the program:

- **Agent rules and skills** (`claude/regeln/`, `claude/skills/`) — the working agreements a
  session reads before it acts.
- **Hooks and slash commands** (`claude/hooks/`, `claude/commands/`) — the guards that refuse a
  secret on a command line, a `pkill` pattern wide enough to hit someone else's process, or a
  commit nobody asked for.
- **Templates for `CLAUDE.md` and `settings.json`** — the contract every agent reads, and the
  wiring that binds each hook to the event it runs on.
- **The knowledge base skeleton** and the tooling that indexes and searches it.
- **Worked examples** of a project rule file, a note, and an `AGENTS.md`.

All of it is in **[agent-setup](https://github.com/Skryx-L-A/agent-setup)**. Without the hooks and
the settings that wire them up,
the workbench runs and the guards do not exist — worth knowing before you rely on them.

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

- **The role prompts and the code comments are in German.** They work as they are, but if you do
  not read German you will want to translate `claude/roles/` first. Any agent does that in one
  pass.
- **Some tools describe a two-machine setup that is not yours.** `wb-sync-setup`,
  `wb-shot-remote` and `wb-ssh-worker` assume a second host reachable over SSH, and
  `wb-modell-proxy` assumes a local model server on it. They are inert without one, and the
  hostnames in them are placeholders you have to fill in. The same goes for the launchd property
  list in `shell/`: it carries a literal `$HOME`, which launchd does not expand, so substitute it
  before you load it.
- **Sending mail is not here.** The tools that did it named the account and the place their
  credentials live, as values in the file rather than as a description, so neither is shipped.
  The rule that governs sending survives in the role prompts: an agent drafts, a person releases.
  Wire your own sender to that rule and nothing else changes.
- **Any file that names a tool this repository does not carry says so** in a note at the end,
  added while the repository was built. Nothing has to be cross-checked by hand.
- **The registry ships with the harnesses that were actually measured** on macOS and Linux. A
  harness you add yourself needs its ready pattern measured once with `wb-harness-probe` —
  a guessed pattern produces workers that receive nothing and never say so.
- **Windows is untested.** The tools assume a POSIX shell and tmux; WSL2 is the path, and nobody
  has walked it end to end.
- **No telemetry, no phone-home, no bundled credentials.** There are no keys and no personal data
  in this repository. It is mechanics only.

## Getting started

Read [INSTALL.md](INSTALL.md). Handing that file to a coding agent and telling it to work through
the steps is a reasonable way to do it, and it is how the setup is meant to spread.

## License

MIT. See [LICENSE](LICENSE).
