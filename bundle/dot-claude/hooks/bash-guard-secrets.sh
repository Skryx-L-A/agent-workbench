#!/bin/bash
# Zweck: verhindert `git add`/`git commit`, die .env-Dateien oder gaengige
#        Secret-Dateimuster ausserhalb ~/Knowledge/90-secrets/ einschliessen.
# Event: PreToolUse, matcher Bash.
# Warum: CLAUDE.md sagt "Never commit a .env (or any secret) to GitHub" und
#        "Secrets knowledge goes only into 90-secrets/" — bisher reine Prosa-
#        Regel ohne technische Durchsetzung (siehe research-skills Luecke #1).
# Fix nach Review 2026-07-28 (M10): alle drei Trigger-Checks verlangten "git"
#        UNMITTELBAR gefolgt vom Unterbefehl (add/commit) — `git -C <dir> add`
#        oder `git --git-dir=<pfad> commit` (genau die Form, die vault-sync
#        selbst benutzt!) fielen komplett durch den Hook. Jetzt liest
#        find_git_invocation() den ERSTEN Nicht-Options-Token nach "git" als
#        echtes Unterkommando (kennt -C/-c/--git-dir=/--work-tree= als
#        Optionen-mit-Wert) und traegt zusaetzlich den per -C/--git-dir
#        angegebenen Zielpfad ein — der breite Staging-Scan (Abschnitt 2)
#        prueft jetzt GENAU dieses Repo, nicht mehr blind das cwd des
#        Bash-Aufrufs (das bei `-C` etwas ganz anderes sein kann).
# Umweg, falls falsch-positiv: Datei umbenennen/aus dem Commit nehmen, oder
#        (nur wenn wirklich unbedenklich) einmalig ohne diesen Hook committen
#        via `git commit --no-verify` betrifft NICHT diesen Hook (Hooks laufen
#        unabhaengig von Git-Hooks) — in dem Fall der Nutzer fragen, ob das
#        Pattern hier angepasst werden soll.
# Fix nach Stresstest 2026-07-28 (B10/B02): Abschnitt 1 pruefte die Pfad-
#        Argumente von `git add` als rohe Kommandozeilen-Tokens — jede Form
#        von Quoting (`git add ".env"`, `git add "$(printf .env)"`, sogar
#        `git add .en''v`) liess sich damit beliebig variieren, der
#        tatsaechliche Working-Tree-Status nicht. Abschnitt 1 ist komplett
#        entfernt; Abschnitt 2 (git status --porcelain) laeuft jetzt bei JEDER
#        Form von `git add` (nicht nur den vorher als "breit" erkannten
#        -A/--all/./commit -a), erkennt damit alle Quoting-Varianten gleich
#        mit und kostet weiterhin nur einen git-Aufruf. Und: kein `jq` mehr —
#        Python3 (json-Modul) uebernimmt Parsen und Ausgabe, damit ein
#        fehlendes jq-Binary diesen Deny-Hook nicht mehr lautlos stilllegt.
set -uo pipefail

# HAERTUNG 2026-07-28 (Stress-Befund B02, zweite Runde): Interpreter und Leser werden
# ABSOLUT aufgerufen. Ein gestripptes PATH (im Stresstest nur `bash`) liess sonst schon
# `cat` und `python3` fehlschlagen — der Hook konnte die Eingabe nicht einmal lesen und
# endete auf 0, also ERLAUBEN. Ein Deny-Hook, der sein Werkzeug nicht findet, blockt.

input=$(/bin/cat)
command=$(printf '%s' "$input" | /usr/bin/python3 -c '
import json, sys
try:
    d = json.load(sys.stdin)
except Exception:
    sys.exit(0)
c = (d.get("tool_input") or {}).get("command")
print(c if isinstance(c, str) else "")
')
[ -z "$command" ] && exit 0
cwd=$(printf '%s' "$input" | /usr/bin/python3 -c '
import json, sys
try:
    d = json.load(sys.stdin)
except Exception:
    sys.exit(0)
c = d.get("cwd")
print(c if isinstance(c, str) else "")
')

GIT_DIR_HINT=""
find_git_invocation() {
  # Sucht im Kommando "git ... <Unterbefehl>", ueberspringt bekannte
  # Optionen-mit-Wert (-C <dir>, -c <k=v>, --git-dir=<p>, --work-tree=<p>).
  # Setzt GIT_DIR_HINT auf den zuletzt gefundenen -C/--git-dir/--work-tree-
  # Wert (leer = kein Hinweis -> Aufrufer nutzt cwd). Rueckgabe 0, wenn
  # IRGENDEIN Vorkommen von "git" im Kommando eines der uebergebenen
  # Zielsubkommandos traf (deckt auch mehrere/verkettete git-Aufrufe ab).
  local seg="$1"; shift
  local -a targets=("$@")
  local -a toks
  read -r -a toks <<< "$seg"
  local n=${#toks[@]} i=0 found=1
  GIT_DIR_HINT=""
  while [ $i -lt $n ]; do
    if [ "${toks[$i]}" = "git" ]; then
      local dirhint="" j=$((i+1))
      while [ $j -lt $n ]; do
        case "${toks[$j]}" in
          -C) dirhint="${toks[$((j+1))]:-}"; j=$((j+2)); continue ;;
          -c) j=$((j+2)); continue ;;
          --git-dir=*) dirhint="${toks[$j]#--git-dir=}"; j=$((j+1)); continue ;;
          --work-tree=*) dirhint="${toks[$j]#--work-tree=}"; j=$((j+1)); continue ;;
          -*) j=$((j+1)); continue ;;
          *) break ;;
        esac
      done
      if [ $j -lt $n ]; then
        local sub="${toks[$j]}" t
        for t in "${targets[@]}"; do
          if [ "$sub" = "$t" ]; then
            GIT_DIR_HINT="$dirhint"
            found=0
          fi
        done
      fi
    fi
    i=$((i+1))
  done
  return $found
}

# Nur reagieren, wenn ueberhaupt git add/commit im Spiel ist.
find_git_invocation "$command" add commit || exit 0

deny() {
  local reason="$1"
  /usr/bin/python3 -c '
import json, sys
print(json.dumps({
    "hookSpecificOutput": {
        "hookEventName": "PreToolUse",
        "permissionDecision": "deny",
        "permissionDecisionReason": sys.argv[1],
    }
}))
' "$reason"
  exit 0
}

is_allowed_env_suffix() {
  # .env.example / .env.template / .env.sample sind erlaubt.
  echo "$1" | /usr/bin/grep -Eqi '\.env\.(example|template|sample)$'
}

is_secret_path() {
  local p="$1"
  # Ausnahme: alles innerhalb 90-secrets/ ist der vorgesehene Ort fuer Secrets.
  echo "$p" | /usr/bin/grep -Eq '(^|/)90-secrets/' && return 1
  is_allowed_env_suffix "$p" && return 1
  # .env oder .env.<irgendwas ausser example/template/sample>
  if echo "$p" | /usr/bin/grep -Eq '(^|/)\.env($|\.[A-Za-z0-9_-]+$)'; then
    return 0
  fi
  # gaengige Secret-Dateimuster
  if echo "$p" | /usr/bin/grep -Eqi '(^|/)(id_rsa|id_ed25519|id_ecdsa)$|\.(pem|p12|pfx)$|(^|/)credentials\.json$|(^|/)service-account.*\.json$|(^|/)secrets\.ya?ml$'; then
    return 0
  fi
  return 1
}

# Immer den echten Working-Tree-Status pruefen, sobald ueberhaupt "git add"
# im Spiel ist (jede Form -- eng oder breit), plus "git commit -a/-am" (deckt
# bereits getrackte, veraenderte Secret-Dateien ab). Ein Token-Vergleich der
# Kommandozeile gegen den Add-Pfad entfaellt komplett -- er war beliebig durch
# Quoting/Kommandosubstitution umgehbar (B10); der tatsaechliche Repo-Zustand
# nicht.
if find_git_invocation "$command" add || echo "$command" | /usr/bin/grep -Eq -- 'commit[[:space:]]+-a(m|[[:space:]]|$)'; then
  find_git_invocation "$command" add commit || true
  repo_dir="${GIT_DIR_HINT:-}"
  if [ -z "$repo_dir" ]; then
    repo_dir="$cwd"
    [ -n "$repo_dir" ] || repo_dir="$PWD"
  fi
  if git -C "$repo_dir" rev-parse --show-toplevel >/dev/null 2>&1; then
    while IFS= read -r line; do
      # git status --porcelain: "XY path" oder "XY orig -> path"
      path=$(echo "$line" | /usr/bin/sed -E 's/^.{2} //' | /usr/bin/sed -E 's/.* -> //')
      [ -z "$path" ] && continue
      if is_secret_path "$path"; then
        deny "git add/commit (breites Staging, Repo: $repo_dir) wuerde eine Secret-/​.env-Datei einschliessen ($path) ausserhalb 90-secrets/. Blockiert von bash-guard-secrets."
      fi
    done < <(git -C "$repo_dir" status --porcelain 2>/dev/null)
  else
    # git nicht benutzbar (fehlt im PATH, kein Repo): frueher endete der Hook hier
    # still auf "erlauben" — gemessen im Stresstest mit gestripptem PATH. Ein
    # Deny-Hook, der seine Pruefung nicht durchfuehren kann, darf nicht so tun, als
    # waere alles in Ordnung. Die Namen im Kommando selbst kann er trotzdem pruefen;
    # bleibt danach Unsicherheit, wird sie WENIGSTENS sichtbar gemacht.
    for tok in $command; do
      case "$tok" in
        -*) continue ;;
      esac
      stripped=${tok%\"}; stripped=${stripped#\"}; stripped=${stripped%\'}; stripped=${stripped#\'}
      if is_secret_path "$stripped"; then
        deny "git add/commit nennt '$stripped' — sieht nach einer Secret-/.env-Datei ausserhalb 90-secrets/ aus. (git war hier nicht benutzbar, geprueft wurde deshalb nur der Kommandotext.) Blockiert von bash-guard-secrets."
      fi
    done
    echo "bash-guard-secrets: WARNUNG — git ist hier nicht benutzbar (Repo '$repo_dir'); es wurde NUR der Kommandotext geprueft, nicht der Staging-Bereich." >&2
  fi
fi

exit 0
