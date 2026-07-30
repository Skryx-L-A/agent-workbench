# Background services

Nothing here is required. The workbench runs without a single one of them; each solves a
specific problem that only appears once you use it daily.

Templates live in `bundle/services/`. Two placeholders need replacing before you load one:
`__HOME__` (your home directory) and `__USER__` (your username). `bootstrap.sh` does that for
you for the services you enable.

## What each one is for

| Service | Problem it solves |
|---|---|
| `mcp-basic-memory`, `mcp-playwright` | MCP servers started per session pile up. Measured on a working machine: 72 helper processes holding 6.6 GB. These run **one** shared server per tool over HTTP on localhost instead. |
| `mcp-reaper` | Cleans up MCP processes whose session died. |
| `limit-survivor` | When an agent hits its rate limit, this nudges the pane back to work **after the reset time named in the message** — not before, not repeatedly, and never into a pane that is only displaying a remote session. |
| `wb-request-review` | Periodically reviews the worker requests that piled up, so cheap-worker delegation gets evaluated instead of forgotten. |
| `brain-backup` | Bundles the knowledge vault so a bad `git` day is not a lost brain. |
| `ollama` (Homebrew) | The local model server. The shipped template sets `OLLAMA_CONTEXT_LENGTH=131072` — without it every local model silently serves a fraction of its real context. |

## macOS

```bash
sed -e "s|__HOME__|$HOME|g" -e "s|__USER__|$(id -un)|g" \
    bundle/services/launchd/agent-workbench.limit-survivor.plist \
    > ~/Library/LaunchAgents/agent-workbench.limit-survivor.plist
launchctl load ~/Library/LaunchAgents/agent-workbench.limit-survivor.plist
launchctl list | grep agent-workbench       # verify it is actually running
```

To stop one: `launchctl unload <plist>`. Changing a plist requires unload, edit, load — a running
agent does not re-read its file.

## Linux

Same jobs as systemd **user** units (`systemctl --user`), not system units — they belong to your
session and need no root:

```bash
systemctl --user enable --now agent-workbench-mcp-basic-memory.service
systemctl --user status agent-workbench-mcp-basic-memory
```

`bundle/services/systemd/` holds the equivalents. If a unit you need is missing there, the
launchd plist tells you the command and the interval; the translation is mechanical.

## Protected services

A service that must never be stopped — an inference server holding a model resident, a live
consumer — is listed one per line in `~/.config/agent-workbench/protected`:

```
my-inference-service	holds a 5 GB model resident during market hours
```

`check-resources` then reports it as PROTECTED, and `run-on` refuses any job that would displace
it. The rule the agent roles enforce on top: never stop a protected process on your own — ask.
