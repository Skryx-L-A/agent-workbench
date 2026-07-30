#!/usr/bin/env bash
# SessionStart hook: injects INDEX + CRITICAL-FACTS + HOT (+ project MOC if cwd matches).
# Must never fail the hook: always exit 0, silently skip missing files.

VAULT="$HOME/Knowledge"

cat "$VAULT/INDEX.md" 2>/dev/null
cat "$VAULT/CRITICAL-FACTS.md" 2>/dev/null
# IDENTITY.md is gitignored/per-machine — says which shared-brain person is at the keyboard.
if [ -f "$VAULT/IDENTITY.md" ]; then
  cat "$VAULT/IDENTITY.md"
else
  echo "IDENTITY.md fehlt — lege sie aus IDENTITY.md.example an (kopieren, ausfüllen, nie committen)."
fi
cat "$VAULT/HOT.md" 2>/dev/null

project_dir="${CLAUDE_PROJECT_DIR:-$(pwd)}"
project_name="$(basename "$project_dir" 2>/dev/null)"
project_name_lower="$(printf '%s' "$project_name" | tr '[:upper:]' '[:lower:]')"

if [ -n "$project_name_lower" ]; then
  for dir in "$VAULT"/20-projects/*/; do
    [ -d "$dir" ] || continue
    entry="$(basename "$dir")"
    if [ "$(printf '%s' "$entry" | tr '[:upper:]' '[:lower:]')" = "$project_name_lower" ]; then
      cat "$dir/MOC.md" 2>/dev/null
      break
    fi
  done
fi

exit 0
