# ~/.pi — local pi worker config (OPTIONAL)

Only needed if you run local pi workers (Ollama + local models). The orchestrator
setup works fine without this directory — skip it if you have no local models.

Contents:
- `agent/WORKER.md` — system prompt for local pi worker agents. Copy to `~/.pi/agent/WORKER.md`.

Deliberately NOT included (and never share these from any machine):
- `auth.json` — credentials
- `sessions/` — conversation history
- `settings.json` / `models.json` — did not exist on the source machine; pi runs with
  CLI flags only (see `bin/pi-worker`). Create your own if your pi setup needs them.

Model aliases: `bin/pi-worker` maps short names to Ollama model IDs (case block near the
top). Edit those to the models you actually pulled, e.g.:

```
qwen)    MODEL="qwen2.5-coder:32b" ;;
small)   MODEL="qwen2.5-coder:7b" ;;
```
