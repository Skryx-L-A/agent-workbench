# Installing the workbench

Roughly twenty minutes, most of it `npm ci`. Every step is repeatable: running it twice does the
same thing as running it once.

If you would rather not do it by hand, hand this file to a coding agent and tell it to work
through the steps. That is the intended path, and the last section says what to check afterwards.

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
role prompts and hooks were written against; Codex, aider, opencode and a local Ollama model all
work through the same registry.

## 1. Get the repository

```bash
git clone https://github.com/<your-github-user>/agent-workbench.git ~/agent-workbench
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

## 4. Install the agent configuration

```bash
mkdir -p ~/.claude/workbench
cp -a claude/roles claude/regeln claude/skills claude/hooks claude/commands ~/.claude/
cp    claude/statusline-command.sh ~/.claude/
chmod +x ~/.claude/hooks/*.sh ~/.claude/hooks/*.py ~/.claude/statusline-command.sh
```

The execute bit matters. A hook without it does not fail loudly — it fails silently, and you find
out weeks later that nothing was ever blocked.

Then the two files that are yours to fill in:

```bash
cp claude/CLAUDE.md.template     ~/.claude/CLAUDE.md
cp claude/settings.json.template ~/.claude/settings.json
```

`CLAUDE.md` is the contract every agent reads. The template is a scaffold: it has the sections and
says what belongs in each, and the placeholders in angle brackets are yours to replace. Read it
once from top to bottom before you start — a rule you did not mean is worse than a missing one.

`settings.json` wires the hooks to the events they run on. It is copied as it is used, so the
paths in it are `$HOME`-relative; if you keep your setup somewhere else, adjust them there.

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

## 6. The knowledge base, if you want one

The workbench runs without it. If you do want one:

```bash
cp -a knowledge ~/Knowledge
cd ~/Knowledge
git init
cp IDENTITY.md.example IDENTITY.md    # fill in, never commit
```

The tooling under `_meta/tools/` is Python and uses [uv](https://docs.astral.sh/uv/):

```bash
cd ~/Knowledge/_meta/tools/braincli && uv sync
cd ~/Knowledge/_meta/tools/gardener && uv sync
```

Both build their index on first run; nothing is shipped pre-built. The gardener talks to a local
Ollama for embeddings and for its judging passes, so `ollama serve` has to be running and a model
pulled before its first run.

`beispiele/notiz.md` shows what a finished note looks like, written to `_meta/templates/note.md`.

## 7. Check that it worked

```bash
wb-doctor
```

It prints one line per check and exits non-zero if something is wrong. Then start one worker and
watch it arrive:

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
| `~/.claude/` | roles, rules, skills, hooks, `CLAUDE.md`, `settings.json` |
| `~/.claude/workbench/models.json` | the model registry |
| `~/Knowledge/` | the knowledge base, if you installed it |
| `~/.config/agent-workbench/` | the workbench window's own state |

## If you have a second machine

`wb-sync-setup`, `wb-ssh-worker`, `peer-shot` and `wb-shot-remote` expect a second host reachable
over SSH with the same layout. The hostnames in them are placeholders — open each one and put your
own in before you rely on it. Without a second host they simply do nothing, and nothing else in
the workbench depends on them.

The launchd property lists in `shell/` (macOS) and the unit files in `shell/systemd/` (Linux) are
templates for the optional background jobs: the nightly test run, the session sweep, the local
model proxy. Each one carries paths that have to be yours before it is loaded.
