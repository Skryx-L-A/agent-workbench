# Installing the workbench

Roughly fifteen minutes, most of it `npm ci`. Every step is repeatable: running it twice does the
same thing as running it once.

If you would rather not do it by hand, hand this file to a coding agent and tell it to work
through the steps. That is the intended path, and the last section says what to check afterwards.

This repository holds the workbench and its tools. The rules, skills, hooks, `CLAUDE.md` and
`settings.json` templates and the knowledge base are in
**[agent-setup](https://github.com/Skryx-L-A/agent-setup)**; if you want those too,
install this first and then follow the instructions there. `<your-github-user>` stands for the
account this repository is hosted under — the tool that extracts it from a working machine removes
account names everywhere and cannot tell a public URL from a private path.

## 0. What has to be there first

```bash
# macOS
brew install git tmux node python@3.12 rsync

# Debian / Ubuntu / WSL2
sudo apt install git tmux nodejs npm python3 rsync
```

Node has to be version 22 or newer — `node -v` says which one you have. On Debian the packaged
Node is often older; use [nodesource](https://github.com/nodesource/distributions) or `nvm` then.

You also need at least one agent CLI, or there is nothing to orchestrate. Claude Code is what the
role prompts were written against; Codex, aider, opencode and a local Ollama model all work
through the same registry.

## 1. Get the repository

```bash
git clone https://github.com/Skryx-L-A/agent-workbench.git ~/agent-workbench
cd ~/agent-workbench
```

Any directory works. The tools do not care where the checkout lives.

## 2. Build the workbench

```bash
cd app
npm ci
npm run build
npm start          # the window opens
```

`npm run check` runs the type check and the build together — use that one when you change
something. On macOS you can also turn the program into a real application bundle, so that
Spotlight, the Dock and a double-click in the Finder start it:

```bash
./tools/buendel-bauen.sh
```

The bundle does not copy the source. It points at this directory, so a plain `npm run build` is
enough to update it; the bundle itself only has to be rebuilt when Electron changes.

## 3. Put the tools on your PATH

```bash
mkdir -p ~/.local/bin
cp -a shell/. ~/.local/bin/
chmod +x ~/.local/bin/wb-* ~/.local/bin/claude-worker ~/.local/bin/pi-worker \
         ~/.local/bin/context-guard ~/.local/bin/check-resources
```

Copy the whole directory, not only the executables: `wb-harness-config` looks for its templates
next to itself, and without them two harnesses fail to start with no useful message.

Make sure `~/.local/bin` is on your `PATH`. In `~/.zshrc` or `~/.bashrc`:

```bash
export PATH="$HOME/.local/bin:$PATH"
```

## 4. Install the role prompts

```bash
mkdir -p ~/.claude/workbench
cp -a claude/roles ~/.claude/
cp    claude/statusline-command.sh ~/.claude/
chmod +x ~/.claude/statusline-command.sh
```

The two prompts are what a session loads to know whether it leads or works. A lead session reads
`orchestrator.md`, a worker pane reads `agent.md`, and the tools pass the right one when they
spawn a pane.

There is no `CLAUDE.md` and no `settings.json` in this repository. The workbench does not need
either to run, and both are shipped as templates in
**[agent-setup](https://github.com/Skryx-L-A/agent-setup)** because they carry the
working agreements rather than the machinery. Until you have them, your agents run with
whatever configuration you already had, and the hooks that would block a bad command are simply
not installed.

## 5. Install the model registry

```bash
cp shell/models.default.json ~/.claude/workbench/models.json
wb-state models discover --all
wb-harness-config apply
```

`discover` asks the CLIs you actually have installed what they currently offer and writes the
result into the registry, so a model you pull tomorrow shows up without an edit. `apply` writes
the config files that some harnesses cannot start without.

Check what came out:

```bash
wb-state models table
```

## 6. Check that it worked

```bash
wb-doctor
```

It prints one line per check and exits non-zero if something is wrong. Checks that belong to the
parts not shipped here — hooks, rules, the knowledge base — report as missing, which is correct
for this repository on its own. Then start one worker and watch it arrive:

```bash
tmux new -s work
claude-worker probe sonnet5 ~/agent-workbench "Sag Hallo und beende dich."
```

The pane opens, the tool waits until the harness is genuinely ready, types the task and confirms
that it was submitted. If it reports that it never became ready, the harness's ready pattern does
not fit your version — measure it once with `wb-harness-probe` and put the result in the registry.
A guessed pattern produces workers that receive nothing and never say so.

## Where things end up

| Path | What |
|---|---|
| `~/.local/bin/` | the command-line tools |
| `~/.claude/roles/` | the two role prompts |
| `~/.claude/workbench/models.json` | the model registry |
| `~/.config/agent-workbench/` | the workbench window's own state |

## If you have a second machine

`wb-sync-setup`, `wb-ssh-worker` and `wb-shot-remote` expect a second host reachable over SSH with
the same layout. The hostnames in them are placeholders — open each one and put your own in before
you rely on it. Without a second host they simply do nothing, and nothing else in the workbench
depends on them.

The launchd property list in `shell/` belongs to one optional background job, the local model
proxy. It is a real file rather than a template and carries a literal `$HOME`, which launchd does
not expand — substitute it before you load it.

Sending mail is deliberately not here. The tools that did it named the account and the place their
credentials live, as values in the file rather than as a description, so neither is shipped. The
rule that governs sending survives in the role prompts: an agent drafts, a person releases. Any
file that names a tool this repository does not carry says so in a note at its end, added while
the repository was built.
