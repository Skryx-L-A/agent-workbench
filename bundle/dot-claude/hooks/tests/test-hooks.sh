#!/bin/bash
# Testet jeden neuen Hook einzeln mit gefaketem stdin-JSON: je ein Fall, der
# greifen MUSS, und einer, der NICHT greifen darf. Laeuft komplett isoliert:
# eigenes HOME (mktemp), eigener tmux-Socket (-L wbtest) fuer die Hooks, die
# @wb_role abfragen. Fasst NIE die echte ~/.claude/hooks-Konfiguration, das
# echte HOME oder eine LIVE-tmux-Session an.
set -uo pipefail
unset TMUX TMUX_PANE

HOOKS_DIR="$HOME/.claude/hooks"
PASS=0
FAIL=0

section() { printf '\n=== %s ===\n' "$1"; }

# Fix nach Review 2026-07-28 (M8): klassifiziert Exit-Code+Decision in genau
# drei Ergebnisse — "allow", "deny", oder "error" (jeder Code ausser 0/2, z.B.
# 1 aus set -euo pipefail, 127 bei fehlendem Binary, abgebrochenes grep).
# Vorher wurde JEDER Nicht-2-Code stillschweigend als "allow" gezaehlt — ein
# abgestuerzter Guard waere durch alle "allow"-Checks (rund die Haelfte der
# Faelle) als "funktionierend" durchgerutscht.
classify_result() {
  local code="$1" decision="$2"
  case "$code" in
    2) echo "deny" ;;
    0)
      if [ "$decision" = "deny" ]; then echo "deny"; else echo "allow"; fi
      ;;
    *) echo "error" ;;
  esac
}

check() {
  # check <label> <expect: deny|allow> <actual_output> <exit_code>
  local label="$1" expect="$2" output="$3" code="$4"
  local decision got
  decision=$(printf '%s' "$output" | jq -r '.hookSpecificOutput.permissionDecision // empty' 2>/dev/null)
  got=$(classify_result "$code" "$decision")
  if [ "$got" = "$expect" ]; then
    printf '  PASS  %-55s (erwartet=%s, erhalten=%s, exit=%s)\n' "$label" "$expect" "$got" "$code"
    PASS=$((PASS+1))
  else
    printf '  FAIL  %-55s (erwartet=%s, erhalten=%s, exit=%s)\n' "$label" "$expect" "$got" "$code"
    printf '        output: %s\n' "$output"
    FAIL=$((FAIL+1))
  fi
}

TMPHOME=$(mktemp -d)
REPO=$(mktemp -d)
git -C "$REPO" init -q
git -C "$REPO" config user.email test@test.local
git -C "$REPO" config user.name test

# ---------------------------------------------------------------------------
section "1) bash-guard-secrets.sh"
# MUSS greifen: git add .env
# Fix nach Stresstest 2026-07-28 (B10): der Hook prueft nicht mehr die
# Kommandozeilen-Tokens (durch Quoting beliebig umgehbar), sondern IMMER den
# echten Working-Tree-Status -- die Datei muss also tatsaechlich existieren,
# genau wie im echten Vorfall (und wie `git add .env` in der Praxis: ohne
# Datei gaebe es nur einen git-Fehler, kein Risiko).
echo "SECRET=1" > "$REPO/.env"
input=$(jq -n --arg cwd "$REPO" '{tool_name:"Bash",tool_input:{command:"git add .env"},cwd:$cwd}')
out=$(echo "$input" | HOME="$TMPHOME" bash "$HOOKS_DIR/bash-guard-secrets.sh"); code=$?
check "git add .env -> deny" deny "$out" "$code"
rm -f "$REPO/.env"

# NICHT greifen: git add .env.example
input=$(jq -n --arg cwd "$REPO" '{tool_name:"Bash",tool_input:{command:"git add .env.example"},cwd:$cwd}')
out=$(echo "$input" | HOME="$TMPHOME" bash "$HOOKS_DIR/bash-guard-secrets.sh"); code=$?
check "git add .env.example -> allow" allow "$out" "$code"

# MUSS greifen: breites Staging (git add -A) mit .env im Working Tree
echo "SECRET=1" > "$REPO/.env"
input=$(jq -n --arg cwd "$REPO" '{tool_name:"Bash",tool_input:{command:"git add -A"},cwd:$cwd}')
out=$(echo "$input" | HOME="$TMPHOME" bash "$HOOKS_DIR/bash-guard-secrets.sh"); code=$?
check "git add -A mit .env im Working Tree -> deny" deny "$out" "$code"
rm -f "$REPO/.env"

# NICHT greifen: git commit -m mit ".env" im Nachrichtentext (False-Positive-Schutz)
input=$(jq -n --arg cwd "$REPO" '{tool_name:"Bash",tool_input:{command:"git commit -m \"document .env handling\""},cwd:$cwd}')
out=$(echo "$input" | HOME="$TMPHOME" bash "$HOOKS_DIR/bash-guard-secrets.sh"); code=$?
check "git commit -m mit .env im Text -> allow (kein Fund)" allow "$out" "$code"

# B10-Regression: Quoting/Kommandosubstitution im Add-Argument darf die
# Erkennung nicht mehr umgehen koennen, weil sie den Token gar nicht mehr
# anschaut -- ".env" existiert, egal wie das Kommando es benennt.
echo "SECRET=1" > "$REPO/.env"
input=$(jq -n --arg cwd "$REPO" '{tool_name:"Bash",tool_input:{command:"git add \".env\""},cwd:$cwd}')
out=$(echo "$input" | HOME="$TMPHOME" bash "$HOOKS_DIR/bash-guard-secrets.sh"); code=$?
check "git add \\\".env\\\" (quoted) -> deny (B10-Fix)" deny "$out" "$code"
rm -f "$REPO/.env"

# M10-Regression: `git -C <dir> add .env` — die Optionsform, die vault-sync
# selbst benutzt, fiel vorher komplett durch den Trigger-Check.
echo "SECRET=1" > "$REPO/.env"
input=$(jq -n --arg cwd "/tmp/irrelevant" --arg repo "$REPO" \
  '{tool_name:"Bash",tool_input:{command:("git -C " + $repo + " add .env")},cwd:$cwd}')
out=$(echo "$input" | HOME="$TMPHOME" bash "$HOOKS_DIR/bash-guard-secrets.sh"); code=$?
check "git -C <dir> add .env -> deny (M10-Fix)" deny "$out" "$code"
rm -f "$REPO/.env"

# ---------------------------------------------------------------------------
section "2) bash-guard-kill-pattern.sh"
# MUSS greifen: exakter Vorfall (der urspruengliche, real ausgefuehrte Befehl,
# siehe ~/Knowledge/10-global/incident-2026-07-25-killmuster-beendete-live-client.md)
input=$(jq -n '{tool_name:"Bash",tool_input:{command:"pkill -f \"tmux attach -t =wb-\""}}')
out=$(echo "$input" | bash "$HOOKS_DIR/bash-guard-kill-pattern.sh"); code=$?
check "pkill -f tmux attach =wb- -> deny (echter Vorfall, Original-Kommando)" deny "$out" "$code"

# NICHT greifen: konkrete PID
input=$(jq -n '{tool_name:"Bash",tool_input:{command:"kill 12345"}}')
out=$(echo "$input" | bash "$HOOKS_DIR/bash-guard-kill-pattern.sh"); code=$?
check "kill 12345 -> allow" allow "$out" "$code"

# NICHT greifen: eigener Test-Socket im Muster
input=$(jq -n '{tool_name:"Bash",tool_input:{command:"tmux -L wbtest kill-server"}}')
out=$(echo "$input" | bash "$HOOKS_DIR/bash-guard-kill-pattern.sh"); code=$?
check "tmux -L wbtest kill-server -> allow (kein pkill/killall)" allow "$out" "$code"

# MUSS greifen: killall claude (systemweit, kein Scope)
input=$(jq -n '{tool_name:"Bash",tool_input:{command:"killall claude"}}')
out=$(echo "$input" | bash "$HOOKS_DIR/bash-guard-kill-pattern.sh"); code=$?
check "killall claude -> deny" deny "$out" "$code"

# H5-Regression 1: der zweite reale Vorfall-Mechanismus — genau die
# Session, die im Incident-Note als Live-Client des Nutzers benannt ist
# (`tmux attach -t =wb-claude-workbench-0df4e2`) direkt per kill-session
# beendet. War VORHER komplett unerkannt (Trigger kannte nur pkill/killall).
input=$(jq -n '{tool_name:"Bash",tool_input:{command:"tmux kill-session -t =wb-claude-workbench-0df4e2"}}')
out=$(echo "$input" | bash "$HOOKS_DIR/bash-guard-kill-pattern.sh"); code=$?
check "tmux kill-session -t =wb-claude-workbench-0df4e2 -> deny (H5-Fix, echte Session aus dem Incident)" deny "$out" "$code"

# H5-Regression 2: bare tmux kill-server (kein -L, trifft den Default-Server)
input=$(jq -n '{tool_name:"Bash",tool_input:{command:"tmux kill-server"}}')
out=$(echo "$input" | bash "$HOOKS_DIR/bash-guard-kill-pattern.sh"); code=$?
check "tmux kill-server (ohne -L) -> deny (H5-Fix)" deny "$out" "$code"

# H5-Regression 3: die Allowlist durfte "wbtest" nur im SELBEN Teilbefehl wie
# der gefaehrliche Aufruf gelten lassen, nicht als Substring irgendwo in der
# Zeile (Kommentar oder verketteter Folgebefehl).
input=$(jq -n '{tool_name:"Bash",tool_input:{command:"pkill -f \"tmux attach -t =wb-\"   # wbtest"}}')
out=$(echo "$input" | bash "$HOOKS_DIR/bash-guard-kill-pattern.sh"); code=$?
check "pkill ... =wb-  # wbtest (Kommentar) -> deny (H5-Fix, kein Bypass ueber Kommentar)" deny "$out" "$code"

input=$(jq -n '{tool_name:"Bash",tool_input:{command:"pkill -f claude; echo wbtest"}}')
out=$(echo "$input" | bash "$HOOKS_DIR/bash-guard-kill-pattern.sh"); code=$?
check "pkill -f claude; echo wbtest -> deny (H5-Fix, kein Bypass ueber Folgebefehl)" deny "$out" "$code"

# ---------------------------------------------------------------------------
section "3) bash-guard-live-config.sh (warn-only, blockt nie)"
mkdir -p "$TMPHOME/.claude/workbench"
input=$(jq -n --arg cwd "/tmp/some/test/dir" --arg fp "$TMPHOME/.claude/workbench/settings.json" \
  '{tool_name:"Write",tool_input:{file_path:$fp,content:"{}"},cwd:$cwd}')
out=$(echo "$input" | HOME="$TMPHOME" bash "$HOOKS_DIR/bash-guard-live-config.sh"); code=$?
warned=$(printf '%s' "$out" | jq -r '.systemMessage // empty' 2>/dev/null)
if [ -n "$warned" ] && [ "$code" = "0" ]; then
  printf '  PASS  %-55s (exit=0, systemMessage gesetzt)\n' "Write auf echte Settings aus Test-cwd -> warn, kein Block"
  PASS=$((PASS+1))
else
  printf '  FAIL  %-55s (exit=%s, systemMessage=%s)\n' "Write auf echte Settings aus Test-cwd -> warn" "$code" "$warned"
  FAIL=$((FAIL+1))
fi

input=$(jq -n --arg cwd "$HOME/AI/some-project" --arg fp "$TMPHOME/.claude/workbench/settings.json" \
  '{tool_name:"Write",tool_input:{file_path:$fp,content:"{}"},cwd:$cwd}')
out=$(echo "$input" | HOME="$TMPHOME" bash "$HOOKS_DIR/bash-guard-live-config.sh"); code=$?
warned=$(printf '%s' "$out" | jq -r '.systemMessage // empty' 2>/dev/null)
if [ -z "$warned" ] && [ "$code" = "0" ]; then
  printf '  PASS  %-55s (exit=0, keine Warnung)\n' "Write auf echte Settings aus normalem cwd -> still"
  PASS=$((PASS+1))
else
  printf '  FAIL  %-55s (exit=%s, systemMessage=%s)\n' "Write aus normalem cwd -> still" "$code" "$warned"
  FAIL=$((FAIL+1))
fi

# ---------------------------------------------------------------------------
section "5) precompact-handoff-gate.sh"
TMUX_SOCK="wbtest"
tmux -L "$TMUX_SOCK" kill-server >/dev/null 2>&1 || true
tmux -L "$TMUX_SOCK" new-session -d -s wbtest-precompact -x 80 -y 24
PANE=$(tmux -L "$TMUX_SOCK" list-panes -t wbtest-precompact -F '#{pane_id}')
tmux -L "$TMUX_SOCK" set -p -t "$PANE" @wb_role worker
SOCK_PATH=$(tmux -L "$TMUX_SOCK" display -p '#{socket_path}')

PROJDIR=$(mktemp -d)
# MUSS blocken: Worker-Rolle, kein HANDOFF-Datei
input=$(jq -n --arg cwd "$PROJDIR" '{cwd:$cwd}')
out=$(echo "$input" | HOME="$TMPHOME" TMUX="$SOCK_PATH,0,0" TMUX_PANE="$PANE" bash "$HOOKS_DIR/precompact-handoff-gate.sh" 2>/tmp/pcg-err.$$); code=$?
if [ "$code" = "2" ]; then
  printf '  PASS  %-55s (exit=%s): %s\n' "Worker ohne HANDOFF -> block" "$code" "$(cat /tmp/pcg-err.$$)"
  PASS=$((PASS+1))
else
  printf '  FAIL  %-55s (exit=%s)\n' "Worker ohne HANDOFF -> block" "$code"
  FAIL=$((FAIL+1))
fi
rm -f /tmp/pcg-err.$$

# NICHT blocken: frisches HANDOFF vorhanden
echo "handoff" > "$PROJDIR/HANDOFF-testworker.md"
out=$(echo "$input" | HOME="$TMPHOME" TMUX="$SOCK_PATH,0,0" TMUX_PANE="$PANE" bash "$HOOKS_DIR/precompact-handoff-gate.sh"); code=$?
if [ "$code" = "0" ]; then
  printf '  PASS  %-55s (exit=%s)\n' "Worker MIT frischem HANDOFF -> allow" "$code"
  PASS=$((PASS+1))
else
  printf '  FAIL  %-55s (exit=%s)\n' "Worker MIT frischem HANDOFF -> allow" "$code"
  FAIL=$((FAIL+1))
fi

# NICHT blocken: kein @wb_role gesetzt (normale Session) -> Gate greift gar nicht
PANE2=$(tmux -L "$TMUX_SOCK" split-window -t wbtest-precompact -P -F '#{pane_id}')
out=$(echo "$input" | HOME="$TMPHOME" TMUX="$SOCK_PATH,0,0" TMUX_PANE="$PANE2" bash "$HOOKS_DIR/precompact-handoff-gate.sh"); code=$?
if [ "$code" = "0" ]; then
  printf '  PASS  %-55s (exit=%s)\n' "Kein @wb_role -> Gate greift nicht, allow" "$code"
  PASS=$((PASS+1))
else
  printf '  FAIL  %-55s (exit=%s)\n' "Kein @wb_role -> allow" "$code"
  FAIL=$((FAIL+1))
fi

# Override-Datei testen
touch "$TMPHOME/.claude/.allow-compact"
out=$(echo "$input" | HOME="$TMPHOME" TMUX="$SOCK_PATH,0,0" TMUX_PANE="$PANE" bash "$HOOKS_DIR/precompact-handoff-gate.sh"); code=$?
if [ "$code" = "0" ]; then
  printf '  PASS  %-55s (exit=%s)\n' "Override-Datei gesetzt -> allow trotz fehlendem Handoff" "$code"
  PASS=$((PASS+1))
else
  printf '  FAIL  %-55s (exit=%s)\n' "Override-Datei -> allow" "$code"
  FAIL=$((FAIL+1))
fi
rm -f "$TMPHOME/.claude/.allow-compact"

# Sauberer Zustand fuer die H4-Regression: das HANDOFF aus dem vorigen
# Testfall entfernen, sonst wuerde "kein HANDOFF" nicht mehr stimmen.
rm -f "$PROJDIR/HANDOFF-testworker.md"

# H4-Regression: AUTOMATISCHE Kompaktierung (trigger=auto) darf NIEMALS
# geblockt werden, auch wenn Worker-Rolle + kein frisches HANDOFF vorliegen —
# genau das Szenario, in dem der Kontext gerade ausgeht und der Harness sich
# selbst retten will. Vorher fehlte das trigger-Feld komplett, der Gate hat
# auch auto-Kompaktierung blockiert (Session-Stillstand-Risiko).
input_auto=$(jq -n --arg cwd "$PROJDIR" '{cwd:$cwd, trigger:"auto"}')
out=$(echo "$input_auto" | HOME="$TMPHOME" TMUX="$SOCK_PATH,0,0" TMUX_PANE="$PANE" bash "$HOOKS_DIR/precompact-handoff-gate.sh" 2>/tmp/pcg-auto-err.$$); code=$?
if [ "$code" = "0" ]; then
  printf '  PASS  %-55s (exit=%s, Warnung: %s) (H4-Fix)\n' "trigger=auto, Worker ohne HANDOFF -> NIE blocken" "$code" "$(cat /tmp/pcg-auto-err.$$)"
  PASS=$((PASS+1))
else
  printf '  FAIL  %-55s (exit=%s) (H4-Fix)\n' "trigger=auto, Worker ohne HANDOFF -> NIE blocken" "$code"
  FAIL=$((FAIL+1))
fi
rm -f /tmp/pcg-auto-err.$$

# Gegenprobe: trigger=manual im selben Szenario (kein Override, kein HANDOFF)
# muss weiterhin blocken -- der Fix darf den bestehenden Schutz nicht aufweichen.
input_manual=$(jq -n --arg cwd "$PROJDIR" '{cwd:$cwd, trigger:"manual"}')
out=$(echo "$input_manual" | HOME="$TMPHOME" TMUX="$SOCK_PATH,0,0" TMUX_PANE="$PANE" bash "$HOOKS_DIR/precompact-handoff-gate.sh" 2>/dev/null); code=$?
if [ "$code" = "2" ]; then
  printf '  PASS  %-55s (exit=%s)\n' "trigger=manual, Worker ohne HANDOFF -> weiterhin block" "$code"
  PASS=$((PASS+1))
else
  printf '  FAIL  %-55s (exit=%s)\n' "trigger=manual -> weiterhin block" "$code"
  FAIL=$((FAIL+1))
fi

tmux -L "$TMUX_SOCK" kill-server >/dev/null 2>&1 || true

# ---------------------------------------------------------------------------
section "7) sessionstart-baseline.sh + sessionend-orphan-check.sh"
SID="test-session-$$"
input=$(jq -n --arg sid "$SID" '{session_id:$sid}')
t0=$(date +%s%N)
echo "$input" | HOME="$TMPHOME" bash "$HOOKS_DIR/sessionstart-baseline.sh"
t1=$(date +%s%N)
baseline_ms=$(( (t1 - t0) / 1000000 ))
if [ -f "$TMPHOME/.local/state/wb-session-baseline-${SID}.json" ]; then
  printf '  PASS  %-55s (%sms)\n' "SessionStart schreibt Baseline-Datei" "$baseline_ms"
  PASS=$((PASS+1))
else
  printf '  FAIL  %-55s\n' "SessionStart schreibt Baseline-Datei"
  FAIL=$((FAIL+1))
fi

t0=$(date +%s%N)
out=$(echo "$input" | HOME="$TMPHOME" bash "$HOOKS_DIR/sessionend-orphan-check.sh" 2>&1)
t1=$(date +%s%N)
end_ms=$(( (t1 - t0) / 1000000 ))
printf '  INFO  SessionEnd-Diff Laufzeit: %sms | Ausgabe: %s\n' "$end_ms" "$(echo "$out" | tr '\n' ' ')"
if [ ! -f "$TMPHOME/.local/state/wb-session-baseline-${SID}.json" ]; then
  printf '  PASS  %-55s\n' "SessionEnd raeumt Baseline-Datei wieder auf"
  PASS=$((PASS+1))
else
  printf '  FAIL  %-55s\n' "SessionEnd raeumt Baseline-Datei wieder auf"
  FAIL=$((FAIL+1))
fi
if [ "$end_ms" -lt 1000 ]; then
  printf '  PASS  %-55s (%sms < 1000ms)\n' "SessionEnd Laufzeit < 1s" "$end_ms"
  PASS=$((PASS+1))
else
  printf '  FAIL  %-55s (%sms >= 1000ms)\n' "SessionEnd Laufzeit < 1s" "$end_ms"
  FAIL=$((FAIL+1))
fi

# ---------------------------------------------------------------------------
section "8) push-gate-worker.sh"
tmux -L "$TMUX_SOCK" kill-server >/dev/null 2>&1 || true
tmux -L "$TMUX_SOCK" new-session -d -s wbtest-push -x 80 -y 24
PANE_W=$(tmux -L "$TMUX_SOCK" list-panes -t wbtest-push -F '#{pane_id}')
tmux -L "$TMUX_SOCK" set -p -t "$PANE_W" @wb_role worker
PANE_O=$(tmux -L "$TMUX_SOCK" split-window -t wbtest-push -P -F '#{pane_id}')
tmux -L "$TMUX_SOCK" set -p -t "$PANE_O" @wb_role orchestrator
SOCK_PATH2=$(tmux -L "$TMUX_SOCK" display -p '#{socket_path}')

input=$(jq -n '{tool_name:"Bash",tool_input:{command:"git push origin main"}}')
out=$(echo "$input" | TMUX="$SOCK_PATH2,0,0" TMUX_PANE="$PANE_W" bash "$HOOKS_DIR/push-gate-worker.sh"); code=$?
check "git push aus Worker-Pane -> deny" deny "$out" "$code"

out=$(echo "$input" | TMUX="$SOCK_PATH2,0,0" TMUX_PANE="$PANE_O" bash "$HOOKS_DIR/push-gate-worker.sh"); code=$?
check "git push aus Orchestrator-Pane -> allow" allow "$out" "$code"

input2=$(jq -n '{tool_name:"Bash",tool_input:{command:"gh pr create --title x --body y"}}')
out=$(echo "$input2" | TMUX="$SOCK_PATH2,0,0" TMUX_PANE="$PANE_W" bash "$HOOKS_DIR/push-gate-worker.sh"); code=$?
check "gh pr create aus Worker-Pane -> deny" deny "$out" "$code"

input3=$(jq -n '{tool_name:"Bash",tool_input:{command:"git log --oneline"}}')
out=$(echo "$input3" | TMUX="$SOCK_PATH2,0,0" TMUX_PANE="$PANE_W" bash "$HOOKS_DIR/push-gate-worker.sh"); code=$?
check "git log (kein push) aus Worker-Pane -> allow" allow "$out" "$code"

# M10-Regression: `git -C <dir> push` — dieselbe Optionsform wie beim
# Secrets-Guard, vorher komplett unerkannt.
input4=$(jq -n --arg repo "$REPO" '{tool_name:"Bash",tool_input:{command:("git -C " + $repo + " push origin main")}}')
out=$(echo "$input4" | TMUX="$SOCK_PATH2,0,0" TMUX_PANE="$PANE_W" bash "$HOOKS_DIR/push-gate-worker.sh"); code=$?
check "git -C <dir> push aus Worker-Pane -> deny (M10-Fix)" deny "$out" "$code"

tmux -L "$TMUX_SOCK" kill-server >/dev/null 2>&1 || true

# ---------------------------------------------------------------------------
section "9) media-cloud-guard.sh"
input=$(jq -n '{tool_name:"Bash",tool_input:{command:"curl -X POST https://api.openai.com/v1/images/generations"}}')
out=$(echo "$input" | bash "$HOOKS_DIR/media-cloud-guard.sh"); code=$?
warned=$(printf '%s' "$out" | jq -r '.systemMessage // empty' 2>/dev/null)
if [ -n "$warned" ] && [ "$code" = "0" ]; then
  printf '  PASS  %-55s\n' "curl an api.openai.com -> warn, kein Block"
  PASS=$((PASS+1))
else
  printf '  FAIL  %-55s (exit=%s warned=%s)\n' "curl an api.openai.com -> warn" "$code" "$warned"
  FAIL=$((FAIL+1))
fi

input=$(jq -n '{tool_name:"Bash",tool_input:{command:"bild \"a cat\" --schnell"}}')
out=$(echo "$input" | bash "$HOOKS_DIR/media-cloud-guard.sh"); code=$?
warned=$(printf '%s' "$out" | jq -r '.systemMessage // empty' 2>/dev/null)
if [ -z "$warned" ] && [ "$code" = "0" ]; then
  printf '  PASS  %-55s\n' "lokales bild-Tool -> keine Warnung"
  PASS=$((PASS+1))
else
  printf '  FAIL  %-55s (exit=%s warned=%s)\n' "lokales bild-Tool -> still" "$code" "$warned"
  FAIL=$((FAIL+1))
fi

input=$(jq -n '{tool_name:"WebFetch",tool_input:{url:"https://api.replicate.com/v1/predictions"}}')
out=$(echo "$input" | bash "$HOOKS_DIR/media-cloud-guard.sh"); code=$?
warned=$(printf '%s' "$out" | jq -r '.systemMessage // empty' 2>/dev/null)
if [ -n "$warned" ]; then
  printf '  PASS  %-55s\n' "WebFetch an api.replicate.com -> warn"
  PASS=$((PASS+1))
else
  printf '  FAIL  %-55s\n' "WebFetch an api.replicate.com -> warn"
  FAIL=$((FAIL+1))
fi

# ---------------------------------------------------------------------------
section "18) configchange-guard.sh"
input=$(jq -n --arg cwd "$PROJDIR" '{config_source:"user_settings",cwd:$cwd,session_id:"t1"}')
out=$(echo "$input" | HOME="$TMPHOME" bash "$HOOKS_DIR/configchange-guard.sh" 2>&1)
if echo "$out" | grep -q "user_settings" && [ -f "$TMPHOME/.claude/hooks/logs/configchange.log" ]; then
  printf '  PASS  %-55s\n' "ConfigChange user_settings -> geloggt + Meldung"
  PASS=$((PASS+1))
else
  printf '  FAIL  %-55s (%s)\n' "ConfigChange user_settings -> log" "$out"
  FAIL=$((FAIL+1))
fi

# ---------------------------------------------------------------------------
section "M8-Regression: classify_result() darf einen Absturz nie als allow zaehlen"
# Simuliert einen abstuerzenden Hook (Exit-Code 1, wie es set -euo pipefail,
# ein fehlendes Binary oder ein frueh abbrechendes grep produzieren wuerde).
# Vorher wertete check() JEDEN Nicht-2-Code als "allow" — ein kaputter Guard
# waere durch die Haelfte der Testfaelle als "funktionierend" gerutscht.
BROKEN_HOOK=$(mktemp)
cat > "$BROKEN_HOOK" <<'EOF'
#!/bin/bash
set -euo pipefail
false
EOF
chmod +x "$BROKEN_HOOK"
out=$(echo '{}' | bash "$BROKEN_HOOK" 2>/dev/null); code=$?
decision=$(printf '%s' "$out" | jq -r '.hookSpecificOutput.permissionDecision // empty' 2>/dev/null)
got=$(classify_result "$code" "$decision")
if [ "$code" != "0" ] && [ "$code" != "2" ] && [ "$got" = "error" ]; then
  printf '  PASS  %-55s (code=%s -> klassifiziert als "%s", nicht als allow) (M8-Fix)\n' "abgestuerzter Hook (exit!=0,2) -> error" "$code" "$got"
  PASS=$((PASS+1))
else
  printf '  FAIL  %-55s (code=%s -> klassifiziert als "%s") (M8-Fix)\n' "abgestuerzter Hook -> error" "$code" "$got"
  FAIL=$((FAIL+1))
fi
# "error" darf per Definition nie gleich "allow" oder "deny" sein -- damit
# wuerde check() diesen Fall gegen JEDE Erwartung als FAIL werten (das ist
# der eigentliche Fix; siehe classify_result() oben). Direkt geprueft statt
# ueber check() demonstriert, um keine irrefuehrenden FAIL-Zeilen fuer einen
# gewollten Demo-Fall in die Testausgabe zu schreiben.
if [ "$got" != "allow" ] && [ "$got" != "deny" ]; then
  printf '  PASS  %-55s ("%s" matcht weder allow noch deny -> check() wuerde FAIL zaehlen)\n' "error ist von allow/deny unterscheidbar" "$got"
  PASS=$((PASS+1))
else
  printf '  FAIL  %-55s ("%s" waere faelschlich als allow/deny durchgerutscht)\n' "error ist von allow/deny unterscheidbar" "$got"
  FAIL=$((FAIL+1))
fi
rm -f "$BROKEN_HOOK"

# ---------------------------------------------------------------------------
section "ZUSAMMENFASSUNG"
printf 'PASS=%d FAIL=%d\n' "$PASS" "$FAIL"

rm -rf "$TMPHOME" "$REPO" "$PROJDIR"

if [ "$FAIL" -gt 0 ]; then
  exit 1
fi
exit 0
