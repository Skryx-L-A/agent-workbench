# Claude Workbench — the VSCode environment

The workbench turns VSCode into a front end for the tmux orchestrator setup: a start page
with session resume, a worker sidebar, and "send to orchestrator" context menus. The
orchestrator and its workers still run as Claude Code CLI sessions in tmux panes inside
VSCode's integrated terminal — the extension only drives and observes them.

Contents of this folder:

| File | What it is |
|---|---|
| `claude-workbench-0.1.0.vsix` | the built VSCode extension (platform-neutral) |
| `scripts/install.sh` | packages + installs the extension into the "Claude Workbench" profile; installs the **Remote-SSH extension first** (it is an `extensionDependency` for the peer/peer sessions and `.vsix` installs do not pull dependencies automatically) |
| `setup-vscode-profile.sh` | creates the VSCode profile "Claude Workbench", applies `profile-settings.json`, installs the .vsix into that profile |
| `profile-settings.json` | the profile's VSCode settings (no welcome tabs, terminal panel at the bottom, telemetry off) |
| `make-app.sh` | **macOS only**: builds `~/Applications/Claude Workbench.app` (Dock icon that opens the profile) |
| `assets/AppIcon.icns` | icon for that app bundle |

The shell side (`wb-code`, `wb-state`) ships in `bundle/bin/` and is installed to
`~/.local/bin` by `bootstrap.sh`.

## How the pieces talk

- `wb-code <dir> [--resume <session-id>]` creates the tmux session
  `wb-<basename>-<md5-6-of-path>` with one orchestrator pane and attaches to it. It is
  idempotent: an existing session is only attached.
- `wb-state` maintains `~/.claude/workbench/sessions/<slug>.json` (`dir`, `tmuxSession`,
  `lastActive`, `workers[]`). Writes are atomic and lock-protected.
- `pi-worker` / `claude-worker` register every spawned worker in that state file, so the
  sidebar can show it.
- The extension reads the state files for its start page, checks liveness against
  `tmux list-panes`, and starts `wb-code` in the integrated terminal. It resolves `wb-code`
  from `PATH`, so `~/.local/bin` must be on the PATH of your login shell.

## Start it

- **macOS**: click the "Claude Workbench" app in the Dock (built by `make-app.sh`), or run
  `code --profile "Claude Workbench" --new-window`.
- **Linux / WSL**: `code --profile "Claude Workbench" --new-window` (no app bundle; make a
  `.desktop` entry running exactly that command if you want a launcher icon).

The start page opens automatically in an empty window of that profile. Pick a project, and
the extension opens the folder and runs `wb-code` in the terminal panel.

## Second machine (peer) terminal profiles

`profile-settings.json` also defines three integrated-terminal profiles that SSH into the
Linux peer `peer` for cross-machine orchestration:

- **peer — orchestrator** → `ssh -t peer tmux new -A -s wb-orch` (the boot-persistent
  orchestrator session; keep it alive with `tmux-orch.service`, see `bundle/tmux-orch.service`).
- **peer — control** → `ssh -t peer tmux new -A -s main` (the visible command shell;
  `orch-launch` is run from here).
- **peer — new session** → `ssh -t peer $HOME/.local/bin/peer-code` (folder picker that
  starts a workbench session on peer).

These need the **Remote-SSH extension** installed in the profile — `scripts/install.sh` does
that first. A working `ssh peer` (Tailscale-SSH) is assumed.

## OS matrix — what works where

| Piece | macOS | Linux | WSL2 | Native Windows (no WSL) |
|---|---|---|---|---|
| Extension (.vsix) | works | works | works (install into the WSL remote) | works, but the tmux features are dead |
| `wb-code`, `wb-state`, workers | works | works | works (inside the distro) | not supported (needs tmux) |
| Profile setup script | works | works | see note below | run the profile steps by hand |
| Dock/app icon (`make-app.sh`) | works | skipped (use `code --profile …`) | skipped | skipped |

**WSL note:** VSCode itself runs on the Windows side; the `code` inside the distro is only a
wrapper. Extensions install into the WSL remote fine, but the profile's `settings.json`
lives under the Windows `%APPDATA%\Code\User`. `setup-vscode-profile.sh` tries to resolve
that path via `cmd.exe`/`wslpath`; if it can't, it tells you to create the profile once in
Windows-VSCode and paste `profile-settings.json` into it.

**Verification status:** the macOS path is the one that has actually been run. Linux and
WSL paths of `setup-vscode-profile.sh` are code-reviewed and OS-branched, but UNTESTED — no
Linux/WSL machine was available. Report breakage instead of assuming it works.

## Rebuilding the extension

The `.vsix` here is a build artifact. Its source lives in a private repo; you only need the
source if you want to change the extension. To install a newer build, drop the new `.vsix`
in this folder and re-run `setup-vscode-profile.sh <new.vsix>`.
