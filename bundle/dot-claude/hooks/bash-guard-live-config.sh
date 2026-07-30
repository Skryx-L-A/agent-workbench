#!/bin/bash
# Zweck: WARNT (blockt NICHT) vor Test-artigen Schreibzugriffen auf die echte
#        Workbench-Settings-Datei (~/.claude/workbench/settings.json) oder vor
#        Eingaben/Kommandos gegen eine LIVE-tmux-Session, wenn der Aufruf nach
#        einem Test aussieht, aber keine Test-Isolation (HOME=$(mktemp -d),
#        -L wbtest) erkennbar ist.
# Event: PreToolUse, matcher Bash + Write + Edit.
# Warum: ECHTER Vorfall — ein Worker hat `workerLayout` zum Testen auf
#        "window" und zurueckgeschaltet, dabei wanderten vier des Nutzers
#        laufende Worker in ein tmux-Fenster ohne Client — fuer ihn sahen sie
#        aus, als liefen keine Worker. CLAUDE.md: "Tests fassen nie die Live-
#        Umgebung an und nie Fenster des Nutzers."
# Policy: bewusst NUR warnen (nie deny) — die Heuristik "sieht nach Test aus"
#        ist unscharf (Pfad-/Kommando-Substring-Suche), ein falscher Block
#        waere schlimmer als eine verpasste Warnung (siehe Auftrag). Wer die
#        Warnung sieht und weiss, dass es kein Test ist, ignoriert sie einfach
#        — es gibt keinen Override-Mechanismus noetig, weil nichts blockiert.
set -euo pipefail

input=$(cat)
tool_name=$(printf '%s' "$input" | jq -r '.tool_name // empty' 2>/dev/null) || exit 0
cwd=$(printf '%s' "$input" | jq -r '.cwd // empty' 2>/dev/null) || true
[ -z "$tool_name" ] && exit 0

REAL_LIVE_SETTINGS="$HOME/.claude/workbench/settings.json"

looks_like_test_context() {
  # Testmarker im cwd (grobe Heuristik, bewusst konservativ).
  echo "$cwd" | grep -Eqi '(^|/)tests?(/|$)|test' 2>/dev/null
}

warn() {
  local reason="$1"
  jq -n --arg reason "$reason" '{
    hookSpecificOutput: {
      hookEventName: "PreToolUse",
      permissionDecision: "allow",
      permissionDecisionReason: $reason,
      additionalContext: $reason
    },
    systemMessage: $reason
  }'
  exit 0
}

case "$tool_name" in
  Write|Edit)
    file_path=$(printf '%s' "$input" | jq -r '.tool_input.file_path // empty' 2>/dev/null) || exit 0
    [ -z "$file_path" ] && exit 0
    if [ "$file_path" = "$REAL_LIVE_SETTINGS" ]; then
      if looks_like_test_context; then
        warn "WARNUNG bash-guard-live-config: Schreibzugriff auf die ECHTE Workbench-Settings ($REAL_LIVE_SETTINGS) aus einem nach-Test-aussehenden Kontext (cwd enthaelt 'test'), aber kein Hinweis auf HOME=\$(mktemp -d)-Isolation erkennbar. Falls das ein Test ist: HOME umleiten statt der echten Datei. Falls kein Test: ignorieren."
      fi
    fi
    ;;
  Bash)
    command=$(printf '%s' "$input" | jq -r '.tool_input.command // empty' 2>/dev/null) || exit 0
    [ -z "$command" ] && exit 0
    # tmux send-keys/attach gegen eine Session mit dem Live-Praefix "wb-",
    # ohne eigenen Test-Socket (-L wbtest / TMUX_TMPDIR).
    if echo "$command" | grep -Eq 'tmux[[:space:]]+.*(send-keys|attach)' \
       && echo "$command" | grep -Eqi '=?wb-' \
       && ! echo "$command" | grep -Eqi 'wbtest|-L[[:space:]]+wbtest|TMUX_TMPDIR'; then
      if looks_like_test_context || echo "$command" | grep -Eqi 'test'; then
        warn "WARNUNG bash-guard-live-config: tmux-Kommando gegen eine Session mit Live-Praefix 'wb-' aus einem nach-Test-aussehenden Kontext, aber kein eigener Test-Socket (-L wbtest) und kein HOME-Redirect erkennbar. Falls das ein Test ist: eigenen Socket verwenden (tmux -L wbtest ...). Falls kein Test: ignorieren."
      fi
    fi
    # Direkter Schreibversuch auf die echte Settings-Datei via Bash (z.B. cat >, echo >, cp).
    if echo "$command" | grep -F -- "$REAL_LIVE_SETTINGS" >/dev/null 2>&1 \
       && echo "$command" | grep -Eq '(>|>>|cp[[:space:]]+.*[[:space:]]|mv[[:space:]]+.*[[:space:]])' \
       && ! echo "$command" | grep -Eqi 'HOME=|mktemp'; then
      if looks_like_test_context || echo "$command" | grep -Eqi 'test'; then
        warn "WARNUNG bash-guard-live-config: Bash-Kommando schreibt moeglicherweise direkt auf die ECHTE Workbench-Settings ($REAL_LIVE_SETTINGS) aus einem nach-Test-aussehenden Kontext, ohne erkennbares HOME=\$(mktemp -d). Falls das ein Test ist: HOME umleiten. Falls kein Test: ignorieren."
      fi
    fi
    ;;
esac

exit 0
