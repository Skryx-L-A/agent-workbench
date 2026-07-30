#!/usr/bin/env bash
# verify.sh — self-test after bootstrap.sh. Prints a PASS/FAIL list.
#
#   ./verify.sh [--spawn-claude]
#
# --spawn-claude additionally spawns a REAL Claude worker pane (needs a working
# login and burns a few tokens). Without it, the tmux worker mechanics are
# tested with a dummy pane instead.
set -uo pipefail

SPAWN_CLAUDE=0
[ "${1:-}" = "--spawn-claude" ] && SPAWN_CLAUDE=1

PASS=0; FAIL=0
ok()   { printf '  \033[32mPASS\033[0m  %s\n' "$*"; PASS=$((PASS+1)); }
bad()  { printf '  \033[31mFAIL\033[0m  %s\n' "$*"; FAIL=$((FAIL+1)); }
skip() { printf '  \033[33mSKIP\033[0m  %s\n' "$*"; }

printf '\nClaude-Setup — Selbsttest\n\n'

# 1. tools on PATH
for t in git tmux rg jq python3 claude; do
  if command -v "$t" >/dev/null; then ok "$t auf PATH"; else bad "$t fehlt"; fi
done
TIMEOUT_CMD=timeout; [ "$(uname -s)" = Darwin ] && TIMEOUT_CMD=gtimeout
if command -v "$TIMEOUT_CMD" >/dev/null; then ok "$TIMEOUT_CMD auf PATH"; else bad "$TIMEOUT_CMD fehlt (coreutils)"; fi

# 2. claude runs
if claude --version >/dev/null 2>&1; then ok "claude --version: $(claude --version 2>/dev/null)"; else bad "claude --version schlägt fehl"; fi

# 3. config files
for f in "$HOME/.claude/CLAUDE.md" "$HOME/.claude/settings.json" "$HOME/.claude/statusline-command.sh" \
         "$HOME/.claude/roles/orchestrator.md" "$HOME/.claude/roles/agent.md"; do
  if [ -s "$f" ]; then ok "vorhanden: ${f/#$HOME/~}"; else bad "fehlt/leer: ${f/#$HOME/~}"; fi
done
if jq -e . "$HOME/.claude/settings.json" >/dev/null 2>&1; then ok "settings.json ist valides JSON"; else bad "settings.json ist kein valides JSON"; fi
# Berechtigungsmodus: nur Information, kein FAIL — die Person entscheidet ihn bewusst.
MODE=$(jq -r '.permissions.defaultMode // "default (nicht gesetzt)"' "$HOME/.claude/settings.json" 2>/dev/null || echo "unbekannt")
case "$MODE" in
  bypassPermissions) printf '  \033[36mINFO\033[0m  defaultMode=%s (Claude fragt vor Befehlen/Änderungen NICHT — so fährt auch der Nutzer)\n' "$MODE" ;;
  acceptEdits)       printf '  \033[36mINFO\033[0m  defaultMode=%s (Datei-Änderungen ohne Rückfrage, Shell-Befehle mit)\n' "$MODE" ;;
  *)                 printf '  \033[36mINFO\033[0m  defaultMode=%s (Rückfrage vor fast allem — Orchestrator-Setup blockiert damit oft)\n' "$MODE" ;;
esac

# 4. worker + orchestration scripts
for s in claude-worker pi-worker; do
  if [ -x "$HOME/.local/bin/$s" ]; then ok "ausführbar: ~/.local/bin/$s"; else bad "fehlt: ~/.local/bin/$s"; fi
done
# Orchestrierungs-/Vault-Werkzeuge (von bootstrap.sh alle nach ~/.local/bin kopiert).
missing_tools=""
for s in workbench wb-code wb-state wb-agent wb-grid wb-pi wb-revive wb-autorevive \
         context-guard brain bm framer-inspo offline check-ollama-kv-ssd ai-scout rerank; do
  [ -x "$HOME/.local/bin/$s" ] || missing_tools="$missing_tools $s"
done
if [ -z "$missing_tools" ]; then ok "Orchestrierungs-Werkzeuge installiert (wb-*, context-guard, brain, framer-inspo …)"; else bad "fehlende Werkzeuge:$missing_tools"; fi
# Syntax-Sanity der bash-Tools (kein Ausführen, nur Parsen).
syn_bad=""
for s in workbench wb-code wb-state wb-agent wb-grid wb-pi wb-revive wb-autorevive \
         context-guard claude-worker pi-worker framer-inspo offline check-ollama-kv-ssd ai-scout; do
  f="$HOME/.local/bin/$s"; [ -f "$f" ] || continue
  head -1 "$f" | grep -q 'bash\|/sh' && { bash -n "$f" 2>/dev/null || syn_bad="$syn_bad $s"; }
done
if [ -z "$syn_bad" ]; then ok "Werkzeug-Skripte parsen fehlerfrei"; else bad "Syntaxfehler in:$syn_bad"; fi
case ":$PATH:" in *":$HOME/.local/bin:"*) ok "~/.local/bin im PATH" ;; *) bad "~/.local/bin NICHT im PATH" ;; esac

# 5. no leftover placeholders
PH='\{\{(USER_NAME|GITHUB_HANDLE|USER_EMAIL|OS_USERNAME|MACHINE_DESCRIPTION|PROJECTS_DIR|RAM_GB|VAULT_REMOTE|VAULT_REMOTE_LINE|PROJECT_LIST|PROJECT_KIT_REPO|OS_NAME|TIMEOUT_CMD|OS_SPECIFIC)'
hits=$(rg -l --no-messages -e "$PH" "$HOME/.claude" "$HOME/Knowledge" "$HOME/.pi" 2>/dev/null | grep -v 'templates/note.md' | grep -v 'gardener/tests/' | grep -v '/projects/' \
  | grep -v '/paste-cache/' | grep -v '/file-history/' | grep -v '/shell-snapshots/' \
  | grep -v '/sessions/' | grep -v '/backups/' || true)
if [ -z "$hits" ]; then ok "keine offenen {{Platzhalter}}"; else bad "Platzhalter übrig in:"$'\n'"$hits"; fi

# 6. statusline renders
SL_IN='{"model":{"display_name":"Opus"},"workspace":{"current_dir":"'"$HOME"'"},"context_window":{"total_input_tokens":46000,"context_window_size":1000000},"rate_limits":{"five_hour":{"used_percentage":12.4,"resets_at":1800000000},"seven_day":{"used_percentage":3.0}}}'
out=$(printf '%s' "$SL_IN" | bash "$HOME/.claude/statusline-command.sh" 2>/dev/null || true)
if [ -n "$out" ] && printf '%s' "$out" | grep -q 'Opus'; then ok "Statusline rendert: $(printf '%s' "$out" | tr -d '\033' | cut -c1-70)"; else bad "Statusline rendert nicht"; fi

# 7. vault + SessionStart hook
if [ -d "$HOME/Knowledge" ]; then ok "Vault vorhanden: ~/Knowledge$( [ -d "$HOME/Knowledge/.git" ] && echo ' (git)' )"; else bad "~/Knowledge fehlt"; fi
if [ -d "$HOME/Knowledge/90-secrets" ] && [ -e "$HOME/Knowledge/90-secrets/.gitignore" ]; then ok "90-secrets/ lokal, gitignored"; else bad "90-secrets/ fehlt oder ohne .gitignore"; fi

# IDENTITY.md: existiert, ausgefüllt, und NICHT im git-Index (gitignored, pro Maschine)
ID="$HOME/Knowledge/IDENTITY.md"
# Brain 4.0 legt die Vault-Werkzeuge unter _meta/ ab, aeltere Vaults unter tools/.
# Beide Orte gelten, sonst meldet der Selbsttest bei einem aktuellen Vault Fehler.
SESSION_CTX="$HOME/Knowledge/_meta/tools/session-context.sh"
[ -f "$SESSION_CTX" ] || SESSION_CTX="$HOME/Knowledge/tools/session-context.sh"

if [ -s "$ID" ]; then
  ok "IDENTITY.md vorhanden"
  if grep -q '{{' "$ID"; then bad "IDENTITY.md enthält noch Platzhalter"; else ok "IDENTITY.md ausgefüllt (keine Platzhalter)"; fi
  if [ -d "$HOME/Knowledge/.git" ]; then
    if git -C "$HOME/Knowledge" ls-files --error-unmatch IDENTITY.md >/dev/null 2>&1; then
      bad "IDENTITY.md liegt im git-Index — sie darf NIE committed werden (gitignoren, 'git rm --cached IDENTITY.md')"
    else
      ok "IDENTITY.md nicht im git-Index (bleibt lokal)"
    fi
  else
    skip "IDENTITY.md-Git-Check (Vault ist kein git-Repo — Skelett)"
  fi
else
  bad "IDENTITY.md fehlt (aus IDENTITY.md.example anlegen und ausfüllen)"
fi
if [ -f "$SESSION_CTX" ]; then
  ctx=$(bash "$SESSION_CTX" 2>/dev/null | head -c 200)
  if [ -n "$ctx" ]; then ok "SessionStart-Hook liefert Vault-Kontext ($(bash "$SESSION_CTX" | wc -l | tr -d ' ') Zeilen)"; else bad "session-context.sh liefert nichts"; fi
else
  bad "session-context.sh fehlt"
fi
if grep -q 'session-context.sh' "$HOME/.claude/settings.json" 2>/dev/null; then ok "SessionStart-Hook in settings.json verdrahtet"; else bad "SessionStart-Hook nicht in settings.json"; fi

# 8. VSCode + extensions
if command -v code >/dev/null; then
  exts=$(code --list-extensions 2>/dev/null)
  printf '%s' "$exts" | grep -qix 'anthropic.claude-code' && ok "VSCode-Extension anthropic.claude-code installiert" || bad "VSCode-Extension anthropic.claude-code fehlt"
  if [ -x "$HOME/.local/bin/wb-code" ]; then
    printf '%s' "$exts" | grep -qix 'agent-workbench.claude-workbench' && ok "Workbench-Extension installiert" || \
      printf '  \033[33mSKIP\033[0m  Workbench-Extension nicht im Default-Profil (sie lebt im Profil "Claude Workbench")\n'
    ok "wb-code/wb-state installiert"
  else
    skip "Workbench nicht installiert (ohne --workbench gebootstrappt)"
  fi
else
  bad "'code' (VSCode-CLI) nicht im PATH"
fi

# 9. tmux worker mechanics
S="wb-verify-$$"
if command -v tmux >/dev/null; then
  tmux new-session -d -s "$S" -c "$HOME" 'sleep 30' 2>/dev/null
  pane=$(tmux split-window -d -t "$S" -P -F '#{pane_id}' 'sleep 30' 2>/dev/null)
  if [ -n "${pane:-}" ]; then
    tmux set -p -t "$pane" @wb_worker vtest
    found=$(tmux list-panes -s -t "$S" -F '#{pane_id} #{@wb_worker}' | awk '$2=="vtest"{print $1}')
    [ "$found" = "$pane" ] && ok "tmux-Worker-Pane spawn + @wb_worker-Lookup" || bad "tmux-Pane-Lookup schlägt fehl"
  else
    bad "tmux split-window schlägt fehl"
  fi
  if [ "$SPAWN_CLAUDE" = 1 ]; then
    res="$HOME/.pi-workers/results/vtest/latest.md"; rm -f "$res"
    if "$HOME/.local/bin/claude-worker" vtest haiku:low "$HOME" "Antworte nur mit OK." >/dev/null 2>&1; then
      d=$(( $(date +%s) + 180 ))
      while [ ! -s "$res" ] && [ "$(date +%s)" -lt "$d" ]; do sleep 5; done
      [ -s "$res" ] && ok "echter Claude-Worker hat Ergebnisdatei geschrieben" || bad "Claude-Worker lieferte in 180s keine Ergebnisdatei"
    else
      bad "claude-worker Spawn schlug fehl"
    fi
  else
    skip "echter Claude-Worker-Spawn (mit --spawn-claude testen)"
  fi
  tmux kill-session -t "$S" 2>/dev/null
else
  bad "tmux fehlt — Worker-Panes nicht möglich"
fi


# --- Modell-Registry: ohne sie gibt es keine Anbieter und keine Modellauswahl ---
if [ -f "$HOME/.claude/workbench/models.json" ]; then
  if python3 -c 'import json,sys; json.load(open(sys.argv[1]))' "$HOME/.claude/workbench/models.json" 2>/dev/null; then
    n_h="$(python3 -c 'import json,sys; print(len(json.load(open(sys.argv[1])).get("harnesses",[])))' "$HOME/.claude/workbench/models.json" 2>/dev/null || echo 0)"
    n_m="$(python3 -c 'import json,sys; print(len(json.load(open(sys.argv[1])).get("models",[])))' "$HOME/.claude/workbench/models.json" 2>/dev/null || echo 0)"
    ok "Modell-Registry lesbar ($n_h Harnesses, $n_m Modelle)"
  else
    bad "~/.claude/workbench/models.json ist kein gueltiges JSON"
  fi
else
  bad "~/.claude/workbench/models.json fehlt — bootstrap.sh erneut laufen lassen"
fi

if command -v wb-state >/dev/null; then
  if wb-state models list >/dev/null 2>&1; then
    ok "wb-state models list antwortet"
  else
    bad "wb-state models list schlaegt fehl"
  fi
  startable="$(wb-state models list --role worker 2>/dev/null | wc -l | tr -d ' ')"
  if [ "${startable:-0}" -gt 0 ]; then
    ok "$startable Worker-Modelle registriert"
  else
    bad "kein einziges Worker-Modell registriert — 'wb-state models discover --all'"
  fi
else
  bad "wb-state nicht im PATH"
fi

# --- Hooks: verdrahtet ist nicht dasselbe wie ausfuehrbar ---
if [ -d "$HOME/.claude/hooks" ]; then
  n_hooks="$(find "$HOME/.claude/hooks" -maxdepth 1 -name '*.sh' | wc -l | tr -d ' ')"
  n_exec="$(find "$HOME/.claude/hooks" -maxdepth 1 -name '*.sh' -perm -u+x | wc -l | tr -d ' ')"
  if [ "${n_hooks:-0}" -eq 0 ]; then
    bad "~/.claude/hooks ist leer"
  elif [ "$n_hooks" = "$n_exec" ]; then
    ok "$n_hooks Hook-Skripte, alle ausfuehrbar"
  else
    bad "$((n_hooks - n_exec)) von $n_hooks Hook-Skripten sind NICHT ausfuehrbar (schlagen still fehl)"
  fi
else
  bad "~/.claude/hooks fehlt"
fi

# --- Regelwerk: die Datei, die jeder Agent liest ---
if [ -f "$HOME/.claude/CLAUDE.md" ]; then
  if grep -q '{{' "$HOME/.claude/CLAUDE.md" 2>/dev/null; then
    bad "~/.claude/CLAUDE.md enthaelt noch Platzhalter ({{...}})"
  else
    ok "~/.claude/CLAUDE.md vorhanden ($(wc -l < "$HOME/.claude/CLAUDE.md" | tr -d ' ') Zeilen Regeln)"
  fi
else
  bad "~/.claude/CLAUDE.md fehlt — die Regeln, die jeder Agent liest"
fi


printf '\n  %d PASS, %d FAIL\n\n' "$PASS" "$FAIL"
[ "$FAIL" -eq 0 ] || exit 1
