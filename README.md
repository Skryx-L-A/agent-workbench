<img src="assets/werkbank.svg" width="88" align="left" alt="Workbench icon">

# agent-workbench

**A desktop workbench for coding agents that work as a team.** One lead session plans, delegates
and verifies; worker panes do the work in parallel; and on the Agents tab, long-lived agents with a
profile, a memory and a specialty take tickets, talk in a channel and only come to you with the
questions that are yours to answer.

<br clear="left">

<p align="center"><img src="assets/vorschau-code-dunkel.png" alt="The Code tab: the lead session and three worker panes, each a live tmux pane running Claude Code, with projects and sessions on the left" width="900"></p>

<p align="center"><img src="assets/vorschau-agents-dunkel.png" alt="The Agents tab: the main agent's chat with a question waiting for your answer, the teams on the left and the agent's profile on the right" width="900"></p>

Both pictures are screenshots of the app itself at its normal size, taken with a demo project: the
terminal panes replay an invented session, and nothing in them belongs to a real machine.

## What it is

- **Two tabs, one program.** *Code* is the orchestrator's place: sessions on the left, the lead
  pane or a grid of worker panes in the middle. *Agents* is where autonomous agents live: worlds
  per project, a main agent, teams with leaders and members, tickets that only a person can
  accept or discard, a channel, direct chats, questions to you with an answer that lands where
  the agent reads it.
- **Everything is a live view on tmux.** The window drives tmux over its control mode, so the
  same sessions are reachable from a terminal on the same machine or over SSH from another one.
- **Not tied to one vendor.** Claude Code, Codex, aider, opencode and local models via Ollama are
  entries in a JSON registry. A setup with no account at all is a supported path: Ollama plus the
  local worker lane runs on your own hardware.
- **Agents you create by talking.** A new agent starts from a form or from a conversation with a
  model that asks what the agent should do, in what tone, with which tools, and fills the profile
  and its instruction file as you go. Both ways end in the same draft; you check it and create it.
- **You approve, the agents wait.** A ticket that asks for approval is approved or sent back
  from the world view. Pausing a world lets a running turn finish and starts no new one; stop
  halts at once, and the window says which of the two is in effect.
- **Worlds on the machine that runs them.** A world can live on a second machine; the window reads
  and writes it over SSH, wakes the agents' carrier after every message, and says when the
  machine is out of reach. Moving a world there is one command.
- **Guards instead of hope.** A context guard makes a worker write a handoff before its context
  runs out. A result protocol makes you wait on a file, not on what a pane looks like. Agents run
  with a tool list and a Bash pattern list from their profile, enforced by hooks, not by prompt.
- **Agents on Claude Code or Codex.** A Codex agent's turn runs with the same profile limits and
  tool policy as a Claude agent's, authenticated through the host's model proxy.

## Branches

- `main` (this branch) is the tested workbench. It is updated only after the full test run of
  the source tree has passed for exactly the state that is published.
- `development` is the product being built: the Rust rebuild in `rust/` and what it needs to
  build, test, run and package. It is updated after the Rust gates have passed. It is a work in
  progress and not released for production use.
- Both branches are licensed under the Business Source License 1.1, see [License](#license).
  Pull requests go against `development`, see [CONTRIBUTING.md](CONTRIBUTING.md).

The earlier repositories [agent-workbench-mac](https://github.com/Skryx-L-A/agent-workbench-mac)
and [agent-setup](https://github.com/Skryx-L-A/agent-setup) are no longer updated.

## Zweige

- `main` (dieser Zweig) ist die getestete Werkbank. Er wird nur aktualisiert, wenn der volle
  Testlauf des Quellbaums für genau den veröffentlichten Stand bestanden hat.
- `development` ist das entstehende Produkt: der Neubau in Rust unter `rust/` und was er zum
  Bauen, Testen, Betreiben und Paketieren braucht. Er wird nach bestandenen Rust-Toren
  aktualisiert. Er ist in Arbeit und nicht für den Betrieb freigegeben.
- Beide Zweige stehen unter der Business Source License 1.1, siehe [License](#license).
  Pull Requests gehen gegen `development`, siehe [CONTRIBUTING.md](CONTRIBUTING.md).

Die früheren Repositories agent-workbench-mac und agent-setup werden nicht mehr gepflegt.

## What is in here

```
app/         the workbench itself — Electron main process, preload bridges, renderer, and
             awb-ctl, a dependency-free CLI that talks to the running program over a socket
extension/   16 modules the app imports rather than duplicates. They started life in a
             VS Code extension and still carry its directory name; there is exactly one copy
             of each, and this is it
shell/       140 command-line tools: worker spawners, context guard, model registry,
             session management, budget and quota, the agents' data library and carrier,
             cross-machine helpers. Which of them ship here is worked out at build time — the
             tools the app itself calls, plus everything those call in turn — because a
             hand-kept list is right until the day somebody adds a call and forgets the list
claude/roles/  the two role prompts, one for the lead session and one for a worker. They are
             what makes a pane behave like part of a workbench instead of a lone agent
claude/statusline-command.sh  the status line the lead session prints
assets/      the icon and the two preview pictures above
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
- **The native Mac shell** — its own repository, see above.

All of the first five are in **[agent-setup](https://github.com/Skryx-L-A/agent-setup)**.
Without the hooks and the settings that wire them up, the workbench runs and the guards do not
exist — worth knowing before you rely on them.

## What you have after installing

- **A workbench window** with your sessions on one side and every worker pane visible at once,
  in light or dark, in English or German.
- **Workers you spawn by name and model.** `claude-worker review sonnet5:high ~/project "…"`
  opens a pane, waits until the harness is genuinely ready, delivers the task, and checks that it
  was received. The same name later means the same pane with the same context.
- **An Agents tab** where a world's agents work through tickets on their own, on a carrier that
  wakes them when a message or a ticket arrives and lets them sleep otherwise. The carrier needs
  systemd, so agents run on a Linux machine; the window can be anywhere.
- **A context guard** that watches how full each pane's context is, makes a worker write a
  handoff before it runs out, compacts it, and sends it back to work. Nothing can compact
  itself, and an agent with a full context does not fail loudly — it quietly gets worse.
- **A result protocol.** Every worker writes its outcome to a file, and you wait on the file
  rather than on what the pane looks like. A spinner is not a status.
- **A model registry.** Harnesses, providers and models are JSON. `wb-state models discover`
  imports whatever your installed CLIs currently offer, probes each new model once, and enables
  only the ones that answered.
- **A secret filter between harness and model API.** `wb-geheimnis-filter` listens on
  127.0.0.1, replaces secrets in outgoing requests with stable placeholders and puts the values
  back into the replies. It needs Python 3.14 or newer and runs only once you load it.
- **A board for scarce resources.** `wb-tafel` lets sessions take, queue for and release places
  such as the GPU, memory or a full test run, so two heavy runs do not start on top of each other.
- **Agent CLIs that stay current.** The window checks the installed harnesses every six hours
  and updates them, by default; `harnessUpdateAuto` in the settings switches that off.

## Requirements

| Needed | Why |
|---|---|
| git, tmux, python3, rsync | the tools are built on them |
| Node.js 22 or newer | the workbench is an Electron program and is built from source |
| At least one agent CLI | otherwise there is nothing to orchestrate |
| A subscription or API key | only for cloud harnesses |
| Ollama and a local model | only for the local lane — roughly 6 GB for a small model |
| A Linux machine with systemd | only for the Agents tab's carrier; the window itself runs on macOS and Linux |
| WSL2 on Windows | tmux has no native Windows equivalent |

## Honest limitations

- **The role prompts, the agents' instruction files and the code comments are in German.** The
  window speaks English or German; the texts an agent reads do not, yet. Any agent translates
  `claude/roles/` in one pass.
- **Some tools describe a two-machine setup that is not yours.** `wb-sync-setup`,
  `wb-shot-remote` and `wb-ssh-worker` assume a second host reachable over SSH, and
  `wb-modell-proxy` assumes a local model server on it. They are inert without one, and the
  hostnames in them are placeholders you have to fill in. The same goes for the launchd property
  lists in `shell/`: they carry a literal `$HOME`, which launchd does not expand, so substitute it
  before you load one.
- **Sending mail is not here.** The tools that did it named the account and the place their
  credentials live, as values in the file rather than as a description, so neither is shipped.
  The rule that governs sending survives in the role prompts: an agent drafts, a person releases.
- **Any file that names a tool this repository does not carry says so** in a note at the end,
  added while the repository was built. Nothing has to be cross-checked by hand.
- **The registry ships with the harnesses that were actually measured** on macOS and Linux. A
  harness you add yourself needs its ready pattern measured once with `wb-harness-probe` —
  a guessed pattern produces workers that receive nothing and never say so.
- **Windows is untested.** The tools assume a POSIX shell and tmux; WSL2 is the path, and nobody
  has walked it end to end.
- **The phone view is not ready to set up from here.** The core carries an opt-in mobile web
  app (pairing by a code shown in the window, sessions, chats, ticket approval), off by default.
  The operator path that puts HTTPS and Web Push in front of it is not part of this repository
  yet, so leave `mobileServer` off.
- **No telemetry, no phone-home, no bundled credentials.** There are no keys and no personal data
  in this repository. It is mechanics only.

## Getting started

Read [INSTALL.md](INSTALL.md). Handing that file to a coding agent and telling it to work through
the steps is a reasonable way to do it, and it is how the setup is meant to spread.

## License

Source-available under the [Business Source License 1.1](LICENSE) with an additional use grant.
Each version also becomes available under the Apache License 2.0 two years after it is first
published.

- **Free to use for everyone**, including companies: at work, in production, within an
  organization and for paid work you do with its help.
- **Free to pass on to your customers**, also adapted to a customer and also for payment.
  Setting it up, hosting, operating, supporting and teaching it for customers is free for
  everyone, also for payment.
- **Partner agreement required** only for offering it to more than one customer under your own
  product name or built into a product under another name; without one, such an offering may not
  be run or hosted, and its customers may not use it in production. Offers to more than one
  customer carry the attribution that COMMERCIAL-LICENSE.md gives. Details: [COMMERCIAL-LICENSE.md](COMMERCIAL-LICENSE.md),
  German reading version of the license: [LICENSE-de.md](LICENSE-de.md).
- Earlier versions keep their license: up to the tag `last-agpl` they
  remain available under AGPL-3.0-only, up to the tag `last-polyform` under PolyForm
  Noncommercial 1.0.0 with additional permissions.
- Only the text in [LICENSE](LICENSE) is binding.
