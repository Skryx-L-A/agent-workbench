#!/bin/sh
# UserPromptSubmit hook: semantic recall from the Knowledge vault via `brain search`.
# Contract: NEVER blocks the prompt. Any error, timeout, or empty result -> silent exit 0.

BRAIN_BIN="$HOME/Knowledge/_meta/tools/braincli/.venv/bin/brain"
TIMEOUT_SECONDS="1.5"
K="4"
# gemessen 2026-07-29 ueber die 37 Eval-Fragen (hook_gates in BASELINE.json):
# 0.35 -> Praezision 36,2 %, Trefferquote 91,9 %
# 0.40 -> Praezision 40,5 %, Trefferquote 86,5 %  <- gewaehlt
# 0.45 -> Praezision 46,7 %, Trefferquote 75,7 %  (zu viel Verlust)
SCORE_THRESHOLD="0.40"
# Wie weit ein Treffer hinter dem besten zurueckliegen darf (cosine).
RELATIVE_MARGIN="0.08"
MAX_HITS="3"
SNIPPET_CHARS="150"

[ -x "$BRAIN_BIN" ] || exit 0
# Portable timeout: gtimeout on macOS (coreutils), timeout on Linux.
if command -v gtimeout >/dev/null 2>&1; then TIMEOUT_BIN=gtimeout
elif command -v timeout >/dev/null 2>&1; then TIMEOUT_BIN=timeout
else exit 0
fi
command -v python3 >/dev/null 2>&1 || exit 0

input="$(cat)"
prompt="$(printf '%s' "$input" | python3 -c '
import json, sys
try:
    data = json.load(sys.stdin)
    p = data.get("prompt", "")
    if isinstance(p, str):
        sys.stdout.write(p)
except Exception:
    pass
' 2>/dev/null)"

[ -n "$prompt" ] || exit 0

result="$("$TIMEOUT_BIN" "$TIMEOUT_SECONDS" "$BRAIN_BIN" search "$prompt" -k "$K" --json 2>/dev/null)"
[ -n "$result" ] || exit 0

printf '%s' "$result" | WB_THRESHOLD="$SCORE_THRESHOLD" WB_RELATIVE_MARGIN="$RELATIVE_MARGIN" WB_MAX_HITS="$MAX_HITS" WB_SNIPPET_CHARS="$SNIPPET_CHARS" python3 "$(dirname "$0")/auto-recall-format.py" 2>/dev/null

exit 0
