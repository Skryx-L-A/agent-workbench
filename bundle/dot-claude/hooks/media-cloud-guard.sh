#!/bin/bash
# Zweck: WARNT (blockt nie) bei Aufrufen an bekannte Cloud-Bild-/Video-/Audio-
#        APIs, verweist auf die lokalen Tools bild/video/tts/stt.
# Event: PreToolUse, matcher Bash + WebFetch.
# Warum: LOCAL-FIRST-Media-Regel ("website/landing-page images ... never a
#        paid cloud model/connector") gilt fuer Orchestrator UND jeden Worker,
#        ist aber nirgends technisch erzwungen — nur Prosa in CLAUDE.md.
# Blockliste: ~/.claude/hooks/media-cloud-domains.txt (separat, leicht
#        pflegbar ohne diesen Hook neu zu schreiben).
# Policy: reines Warnen, kein Deny — die Liste veraltet schnell und manche
#        Cloud-Aufrufe sind bewusst genehmigte Ausnahmen (z.B. creative-media-
#        Skill nutzt Higgsfield gezielt); ein Hard-Block waere hier falsch.
set -uo pipefail

domains_file="$HOME/.claude/hooks/media-cloud-domains.txt"
[ -f "$domains_file" ] || exit 0

input=$(cat)
tool_name=$(printf '%s' "$input" | jq -r '.tool_name // empty' 2>/dev/null)
[ -z "$tool_name" ] && exit 0

haystack=""
case "$tool_name" in
  Bash)
    haystack=$(printf '%s' "$input" | jq -r '.tool_input.command // empty' 2>/dev/null)
    ;;
  WebFetch)
    haystack=$(printf '%s' "$input" | jq -r '.tool_input.url // empty' 2>/dev/null)
    ;;
  *)
    exit 0
    ;;
esac
[ -z "$haystack" ] && exit 0

hit=""
while IFS= read -r domain; do
  case "$domain" in
    ''|'#'*) continue ;;
  esac
  if printf '%s' "$haystack" | grep -Fq "$domain"; then
    hit="$domain"
    break
  fi
done < "$domains_file"

[ -z "$hit" ] && exit 0

reason="media-cloud-guard: Aufruf an bekannte Cloud-Media-API ($hit) erkannt. LOCAL-FIRST-Regel: bild/video/tts/stt (~/.local/bin, offline-faehig) zuerst pruefen, Cloud nur wenn lokale Qualitaet nachweislich nicht reicht oder der Nutzer es verlangt. Reine Warnung, kein Block."
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
