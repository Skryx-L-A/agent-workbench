#!/bin/bash
# WARNUNG an alle, die hier debuggen: dieser Test ruft die INSTALLIERTEN Skripte unter
# ~/.local/bin auf. Wer waehrend eines Laufs dorthin kopiert (Deploy), bekommt Fehlschlaege,
# die nichts mit dem Code zu tun haben — am 2026-07-28 sahen so aus dem Nichts sechs FAILs
# aus, von denen zwei echt waren. Erst deployen, dann testen; nie gleichzeitig.
# test-registry.sh — automated tests for the V3 model/harness registry.
#
# ISOLATION IS THE POINT (rules of 2026-07-25, both after real incidents):
#   * `unset TMUX TMUX_PANE` in the FIRST line of code, before anything else. A
#     script that only redirects the socket still lets every helper it CALLS talk to
#     the LIVE tmux server, because $TMUX beats TMUX_TMPDIR. Everything below would
#     look right while poking alice's running session.
#   * own tmux server: `tmux -L wbtest`, reached through a wrapper on PATH, so every
#     helper (pi-worker, wb-grid, wb-state) lands on the test socket too.
#   * own HOME: mktemp -d. The real ~/.claude/workbench/settings.json and models.json
#     are NEVER read or written here — a test that needs the live file is built wrong.
#   * cleanup kills the test server by SOCKET (`tmux -L wbtest kill-server`) and the
#     fake agents by REMEMBERED PID. No pkill pattern, ever: a pattern ending in
#     "wb-" has already killed alice's live client twice.
#
# Run:  shell/tests/test-registry.sh            (from the repo, or anywhere)
unset TMUX TMUX_PANE
set -uo pipefail

SOCK=wbtest
REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"     # …/claude-workbench/shell
SESS="wb-registrytest-$$"
PASS=0; FAIL=0

# `cd … && pwd` normalises the path: $TMPDIR ends in '/' on macOS, so mktemp hands
# back a DOUBLE slash while every script that resolves its dir the usual way reports
# the single-slash form — two different state-file slugs for one directory.
TESTHOME="$(cd "$(mktemp -d "${TMPDIR:-/tmp}/wb-registry-test.XXXXXX")" && pwd)"
export HOME="$TESTHOME"
# wb-code kickt seit dem discover-Vertrag (2026-07-29) bei jedem Aufruf im
# Hintergrund 'wb-state models discover --all --if-stale' an. Ohne diesen Haken
# wuerde JEDER wb-code-Aufruf unten ueber die reale PATH gegen die echten lokalen
# CLIs dieser Maschine laufen (Regel 2026-07-29: kein Test haengt am Rechnerzustand).
export WB_NO_DISCOVER=1
BIN="$TESTHOME/.local/bin"; mkdir -p "$BIN" "$TESTHOME/.claude/workbench" "$TESTHOME/work"
export TMPDIR="$TESTHOME/tmp/"; mkdir -p "$TMPDIR"
FAKELOG="$TESTHOME/fake.log"; export FAKELOG

cleanup() {
  tmux -L "$SOCK" kill-session -t "=$SESS" 2>/dev/null || true
  tmux -L "$SOCK" kill-server 2>/dev/null || true
  if tmux -L "$SOCK" ls >/dev/null 2>&1; then
    echo "WARNUNG: tmux-Server auf Socket '$SOCK' laeuft noch: tmux -L $SOCK ls" >&2
  fi
  rm -rf "$TESTHOME"
}
trap cleanup EXIT INT TERM

ok()   { PASS=$((PASS+1)); printf '  ok   %s\n' "$1"; }
bad()  { FAIL=$((FAIL+1)); printf '  FAIL %s\n' "$1"; [ $# -gt 1 ] && printf '       %s\n' "$2"; }
have() { # have <label> <haystack> <needle>
  case "$2" in *"$3"*) ok "$1" ;; *) bad "$1" "erwartet '$3' in: $(printf '%s' "$2" | head -3 | tr '\n' ' ')" ;; esac
}
hasnt() {
  case "$2" in *"$3"*) bad "$1" "'$3' steht in der Ausgabe, darf es aber nicht" ;; *) ok "$1" ;; esac
}
eq() { if [ "$2" = "$3" ]; then ok "$1"; else bad "$1" "'$2' != '$3'"; fi; }

# ── fakes: no third-party CLI is installed for these tests ──────────────────────
# The wrapper must call tmux by ABSOLUTE path. `env tmux` would find the wrapper
# again through the very PATH we are about to set and re-exec itself forever
# (measured: one process accumulating '-L wbtest' until it was killed).
REALTMUX="$(command -v tmux)"
[ -x "$REALTMUX" ] || { echo "tmux nicht gefunden — Test kann nicht laufen." >&2; exit 1; }
cat >"$BIN/tmux" <<EOF
#!/bin/bash
exec "$REALTMUX" -L $SOCK "\$@"
EOF
mkfake() { # mkfake <name> — logs argv + a chosen env var, then behaves like a REPL
  cat >"$BIN/$1" <<EOF
#!/bin/bash
{ echo "ARGV $1 \$*"; echo "ENV $1 FAKE_KEY=\${FAKE_KEY:-}"; } >>"$FAKELOG"
while :; do printf '❯ '; IFS= read -r _line || sleep 1; done
EOF
  chmod +x "$BIN/$1"
}
mkfake claude; mkfake pi; mkfake faketui
# A CLI that shows an input line and never consumes it: the paste stays visible, so
# the verification has to retry and then fail loudly. This is the mechanism behind
# Reviewer-Befund H1 — without a test it only ever gets exercised by accident.
cat >"$BIN/fakestuck" <<'EOF'
#!/bin/bash
# \xHH, never \u: macOS ships bash 3.2, whose printf has no \u escape and would
# print the escape text itself — the ready-wait would then run into its 60s deadline.
printf '\xe2\x9d\xaf haengengebliebener Text\n'
while :; do sleep 5; done
EOF
chmod +x "$BIN/fakestuck"
# A CLI that draws its frame at once and shows the input line only seconds later —
# aider 0.86.2 behaves exactly like this while its model loads (Befund 2026-07-28).
cat >"$BIN/fakeslow" <<'EOF'
#!/bin/bash
echo "Fake slow CLI"
printf '\xe2\x94\x80%.0s' $(seq 1 60); echo
sleep 6
printf '\xe2\x94\x80%.0s' $(seq 1 60); echo
printf '> '
while :; do IFS= read -r _l || sleep 1; done
EOF
chmod +x "$BIN/fakeslow"
chmod +x "$BIN/tmux"
# COPIES, not symlinks: the suite has to test one fixed snapshot of the repo. With
# symlinks an edit during the run changes the code under the running test — that is
# how the reviewer got 9 FAIL, then 1, then 0 without any code being wrong (M7).
for s in wb-state pi-worker claude-worker wb-grid wb-harness-run wb-harness-probe wb-code; do
  cp "$REPO/$s" "$BIN/$s"; chmod +x "$BIN/$s"
done
export PATH="$BIN:$PATH"
WBS="$BIN/wb-state"

echo "== 1. ohne models.json: eingebaute Adapter, heutiges Verhalten =="
out="$("$WBS" models list 2>&1)"; eq "models list ist leer" "$out" ""
out="$("$WBS" harness get claude 2>&1)"; have "harness get claude (eingebaut)" "$out" '"readyPattern": "❯|░|●"'
out="$("$WBS" harness get pi 2>&1)";     have "harness get pi (eingebaut)" "$out" '"id": "pi"'
out="$("$WBS" models get gpt-5-codex 2>&1)"; have "unbekanntes Modell -> Fehler" "$out" "nicht registriert"

# Creating the test session can fail transiently (seen 2026-07-28: the server was
# gone right after new-session, most likely pty pressure from parallel workers). If
# that is not caught, EVERY later check fails as "empty" and reads like a content
# bug. Retry, then abort with the real tmux message.
mkses() {
  local try err
  for try in 1 2 3; do
    err="$(tmux -L "$SOCK" new-session -d -s "$SESS" -x 200 -y 50 \
             "bash -c 'while :; do sleep 5; done'" 2>&1)"
    tmux -L "$SOCK" has-session -t "=$SESS" 2>/dev/null && return 0
    echo "  (Testsession-Start $try/3 fehlgeschlagen: ${err:-keine Meldung})" >&2
    sleep 2
  done
  echo "ABBRUCH: Testsession '$SESS' laesst sich auf Socket '$SOCK' nicht anlegen." >&2
  exit 1
}
mkses
tmux -L "$SOCK" set -p -t "$SESS" @wb_role orchestrator
export WB_SESSION="$SESS"

# tmux renders pane_start_command with backslash escapes; strip them so the
# assertions compare the COMMAND, not tmux's quoting.
startline() {
  # Seen once (2026-07-28): the tmux server on the TEST socket vanished mid-run and
  # every start-line check then failed as "empty", which reads like a content bug.
  # Say what really happened instead.
  tmux -L "$SOCK" has-session -t "=$SESS" 2>/dev/null || { echo "TESTSERVER-WEG"; return; }
  tmux -L "$SOCK" list-panes -s -t "=$SESS" -F '#{@wb_worker}#{?pane_dead,,} #{pane_start_command}' \
                | awk -v n="$1" '$1==n{$1="";print;exit}' | tr -d '\\'; }

"$BIN/claude-worker" w-claude sonnet5:high "$TESTHOME/work" >/dev/null 2>&1
"$BIN/pi-worker"     w-pi     ornith9      "$TESTHOME/work" >/dev/null 2>&1
CLAUDE_BEFORE="$(startline w-claude)"; PI_BEFORE="$(startline w-pi)"
have "claude-Worker Startzeile (ohne Registry)" "$CLAUDE_BEFORE" "claude --model claude-sonnet-5 --effort high --dangerously-skip-permissions"
have "pi-Worker Startzeile (ohne Registry)"     "$PI_BEFORE" "pi --provider ollama --model ornith:9b"
have "fake claude bekam die Flags"              "$(cat "$FAKELOG")" "ARGV claude --model claude-sonnet-5 --effort high"

echo "== 2. mit models.json: Registry liest, resolve baut =="
cp "$REPO/models.default.json" "$TESTHOME/.claude/workbench/models.json"
out="$("$WBS" models list)"; have "models list kennt die eingebauten Modelle" "$out" "claude-sonnet-5"
out="$("$WBS" models list --role orchestrator)"; hasnt "Rollenfilter: haiku ist kein Orchestrator" "$out" "claude-haiku-4-5"
out="$("$WBS" models get claude-sonnet-5 --field harness)"; eq "get --field" "$out" "claude"
out="$("$WBS" models resolve claude-sonnet-5 --role worker --effort high --dir /tmp/x --name w1 2>/dev/null)"
have "resolve claude: Kommando" "$out" "--model claude-sonnet-5 --effort high --dangerously-skip-permissions"
have "resolve claude: ready"    "$out" "ready	❯|░|●"
out="$("$WBS" models resolve ornith-9b --role worker --dir /tmp/x --name w1 2>/dev/null)"
have "resolve pi: Kommando"  "$out" "pi --provider ollama --model ornith:9b"
have "resolve pi: session-dir" "$out" ".pi-workers/sessions/w1"
out="$("$WBS" models table)"; have "table ist Markdown" "$out" "| Aufgabe | So spawnen | Harness | Eignung |"
# The table has to be usable WITHOUT looking anything up: the spelling in it is the
# one you type, and a model that fits several jobs gets one row per job.
have "Tabelle zeigt den Alias mit Effort, nicht die volle id" "$out" '| mechanisch | `haiku45:low` | claude |'
have "sonnet5 als kurze Spur"        "$out" '| kurz + gut spezifiziert | `sonnet5:high` | claude |'
have "sonnet5 als groessere Spur"    "$out" '| groesser, aber Spez klar | `sonnet5:xhigh` | claude |'
have "opus5 als Reasoning-Spur"      "$out" '| lang/mehrstufig, Debugging, Ambiguitaet | `opus5:xhigh` | claude |'
have "opus5 als Reviewer-Spur"       "$out" '| unabhaengiger Reviewer-Pass | `opus5:high` | claude |'
have "opus48 als Zweitmeinung"       "$out" '| Zweitmeinung / A-B | `opus48:xhigh` | claude |'
have "fable5 kundengerichtet visuell" "$out" '| kundengerichtet visuell | `fable5:medium` | claude |'
have "pi-Alias ohne Effort-Suffix"   "$out" '| Bulk / Inventur / DSGVO / Overnight | `ornith` | pi |'
# Mit Effort-Block, sonst haengt die Tabelle keinen `:high`-Suffix an und der Fall
# pruefte nur die halbe Aussage (die id, nicht die tippbare Schreibweise).
"$WBS" models add --kind harness '{"id":"fremdcli","label":"Fremd","command":"/bin/echo","args":[],"cwdMode":"cd","systemPrompt":{"style":"none"},"readyPattern":"x","promptPattern":"^x","effort":{"style":"arg","args":["--effort","{effort}"],"map":{"low":"low","medium":"medium","high":"high","xhigh":"high"}}}' >/dev/null
"$WBS" models add '{"id":"fremd-1","harness":"fremdcli","provider":"ollama","modelRef":"m","roles":["worker"],"workerClass":["coding-lang"],"defaultEffort":"high","supportsEffort":true,"goodFor":"Fixture"}' >/dev/null
out="$("$WBS" models table)"
have "Fremdmodell zeigt seine id"    "$out" '| `fremd-1:high` | fremdcli |'
# A row nobody can act on has to say so IN the row (Reviewer-Befund H2).
# Kein AUSGELIEFERTES Preset mehr als Stellvertreter fuer "ungemessen": sobald jemand
# den CLI installiert und misst (am 2026-07-28/29 fuer codex, gemini->agy, opencode und
# aider passiert), testete der Fall den Zustand der Registry statt den Code. Eigener
# Wegwerf-Harness, der garantiert ungemessen bleibt.
"$WBS" models add --kind harness '{"id":"ungemessencli","label":"ungemessen","command":"/bin/echo","args":[],"cwdMode":"cd","systemPrompt":{"style":"none"}}' >/dev/null
"$WBS" models add '{"id":"ungemessen-1","harness":"ungemessencli","provider":"ollama","modelRef":"m","roles":["worker"],"workerClass":["bulk"],"goodFor":"nur fuer den Test"}' >/dev/null
out="$("$WBS" models table)"
have "ungemessener Harness ist als nicht startbar markiert" "$out" "NICHT STARTBAR (Ready ungemessen, wb-harness-probe ungemessencli)"
hasnt "gemessener Harness traegt keine Warnung" "$out" "NICHT STARTBAR (Ready ungemessen, wb-harness-probe aider)"
"$WBS" models remove ungemessen-1 >/dev/null
"$WBS" models remove --kind harness ungemessencli >/dev/null
out="$("$WBS" models table)"
# DSGVO-Bulk heisst: die Daten verlassen die Maschine NICHT. Ein Cloud-Modell darf
# das Kriterium nie ueber die Aufgabenklasse erben (Befund 2026-07-28).
have "lokales Modell behaelt DSGVO" "$out" '| Bulk / Inventur / DSGVO / Overnight | `ornith` | pi |'
"$WBS" models add '{"id":"cloud-bulk-1","harness":"fremdcli","provider":"openai","modelRef":"m","roles":["worker"],"workerClass":["bulk"],"goodFor":"Fixture"}' >/dev/null
out="$("$WBS" models table)"
# Der Fixture-Harness hat einen Effort-Block und der Provider ist Cloud, also haengt die
# Tabelle korrekt `:medium` an — die Aussage des Falls ist die FEHLENDE DSGVO-Angabe,
# nicht die Schreibweise.
have "Cloud-Modell ohne DSGVO"      "$out" '| Bulk / Overnight | `cloud-bulk-1:medium` | fremdcli |'
"$WBS" models remove cloud-bulk-1 >/dev/null; "$WBS" models remove fremd-1 >/dev/null
"$WBS" models remove --kind harness fremdcli >/dev/null
out="$("$WBS" models table)"
hasnt "kein DSGVO an einem Cloud-Modell" "$out" 'DSGVO / Overnight | `gemini-3-pro`'
out="$("$WBS" models resolve aider-ornith-9b --role worker --effort medium --dir /tmp/x --name w1 2>/dev/null)"
have "aider ist startbar (Muster gemessen)" "$out" 'ready	^>'
hasnt "kein --reasoning-effort an ein lokales Modell" "$out" "--reasoning-effort"
out="$("$WBS" models resolve ornith-35b --role worker --effort high --dir /tmp/x --name w1 2>/dev/null)"
have "pi behaelt --thinking (supportsEffort=true)" "$out" "--thinking high"
hasnt "keine volle Claude-ID in der Tabelle" "$out" '`claude-sonnet-5'
hasnt "keine Schema-Sprache in der Aufgabenspalte" "$out" '| coding-kurz |'

echo "== 3. Adapter ohne gemessenes readyPattern startet NICHT =="
# Eigener Adapter mit VORHANDENEM Binary: sonst meldet zuerst die Binary-Pruefung
# (Stress-Befund B17), und die ist hier nicht gemeint. Reihenfolge ist Absicht —
# messen kann man nur, was installiert ist.
"$WBS" models add --kind harness '{"id":"unmeasured","command":"/bin/echo","args":[],"cwdMode":"cd","systemPrompt":{"style":"none"}}' >/dev/null
"$WBS" models add '{"id":"unmeasured-1","harness":"unmeasured","provider":"ollama","modelRef":"m","roles":["worker"],"workerClass":["bulk"]}' >/dev/null
out="$("$WBS" models resolve unmeasured-1 --role worker --dir /tmp/x --name w1 2>&1)"
have "resolve blockt ungemessenen Adapter" "$out" "readyPattern fuer unmeasured ist nicht gemessen"
have "resolve nennt den Messbefehl dazu" "$out" "wb-harness-probe unmeasured"
out="$("$BIN/claude-worker" w-unmeasured unmeasured-1 "$TESTHOME/work" "task" 2>&1)"
have "Spawn blockt ungemessenen Adapter" "$out" "readyPattern fuer unmeasured ist nicht gemessen"
eq "kein Pane fuer den geblockten Spawn" "$(startline w-unmeasured)" ""
# Eigener Wegwerf-Adapter statt des echten codex-Presets: der Fall heisst "Binary fehlt"
# und muss das auch dann noch pruefen, wenn jemand codex installiert (am 2026-07-28
# passiert — der Test wurde gruen/rot je nach Zustand des Rechners, nicht je nach Code).
"$WBS" models add --kind harness '{"id":"nixdacli","label":"gibt es nicht","command":"nixda-binary-2f9c","args":[],"cwdMode":"cd","systemPrompt":{"style":"none"},"readyPattern":"x","promptPattern":"^x"}' >/dev/null
"$WBS" models add '{"id":"nixda-1","harness":"nixdacli","provider":"ollama","modelRef":"m","roles":["worker"],"workerClass":["bulk"]}' >/dev/null
out="$("$WBS" models resolve nixda-1 --role worker --dir /tmp/x --name w1 2>&1)"
have "nicht installierter Adapter nennt das Binary zuerst" "$out" "Binary 'nixda-binary-2f9c' fuer Harness 'nixdacli' ist nicht installiert"
"$WBS" models remove nixda-1 >/dev/null
"$WBS" models remove unmeasured-1 >/dev/null; "$WBS" models remove --kind harness unmeasured >/dev/null

echo "== 4. Policy-Caps im Code, nicht in der Datei =="
out="$("$WBS" models resolve claude-opus-5 --role worker --effort max --dir /tmp/x 2>&1)"
have "effort max wird nie gespawnt" "$out" "effort 'max' wird nie gespawnt"
"$WBS" models set claude-fable-5 maxEffort '"xhigh"' >/dev/null 2>&1
"$WBS" models set claude-fable-5 efforts '["low","medium","high","xhigh"]' >/dev/null 2>&1
out="$("$WBS" models resolve claude-fable-5 --role worker --effort high --dir /tmp/x 2>&1)"
have "fable bleibt bei medium, auch wenn die Datei mehr erlaubt" "$out" "fable ist auf effort 'medium' gedeckelt"
out="$("$WBS" models resolve claude-haiku-4-5 --role worker --effort xhigh --dir /tmp/x 2>&1 >/dev/null)"
have "maxEffort deckelt (Warnung, kein Abbruch)" "$out" "wird auf 'high' gesenkt"
out="$("$WBS" models resolve claude-haiku-4-5 --role orchestrator --dir /tmp/x 2>&1)"
have "Rollenfilter greift beim resolve" "$out" "nicht freigegeben"
out="$("$WBS" models add '{"id":"sonnet5","harness":"claude","provider":"claude-subscription","modelRef":"x"}' 2>&1)"
have "Namenskollision mit eingebautem Alias" "$out" "eingebauter Alias"
out="$("$WBS" models add '{"id":"weird","harness":"gibtsnicht","provider":"openai","modelRef":"x"}' 2>&1)"
have "unbekannter Harness wird abgelehnt" "$out" "nicht registriert"
out="$("$WBS" models set claude-sonnet-5 alias '"sonnet-fantasie"' 2>&1)"
have "erfundener Alias wird abgelehnt" "$out" "kein eingebauter Alias"
out="$("$WBS" models set claude-fable-5 workerClass '[{"class":"visuell","effort":"high"}]' 2>&1)"
have "Tabellenzeile darf keinen verbotenen Effort empfehlen" "$out" "fable bleibt bei 'medium'"
out="$("$WBS" models set ornith-9b workerClass '[{"class":"gibtsnicht"}]' 2>&1)"
have "unbekannte workerClass wird abgelehnt" "$out" "workerClass 'gibtsnicht' ungueltig"
out="$("$WBS" models set ornith-9b workerClass '[{"class":"bulk","effort":"max"}]' 2>&1)"
have "effort max auch in der workerClass verboten" "$out" "'max' nie"
out="$("$WBS" models set ornith-9b supportsEffort '"ja"' 2>&1)"
have "supportsEffort muss bool sein" "$out" "muss true oder false sein"

echo "== 5. Secrets: Platzhalter statt Wert, Mode 600, harter Fehler ohne Key =="
"$WBS" models add --kind provider '{"id":"fakecloud","label":"Fake","kind":"cloud","baseUrl":"http://example.invalid","apiKeyEnv":"FAKE_KEY","keychainService":"wb-fakecloud-test"}' >/dev/null
"$WBS" models add --kind harness '{"id":"fakecli","label":"Fake CLI","command":"faketui","args":["--model","{model}"],"cwdMode":"cd","env":{"FAKE_KEY":"{secret:fakecloud}"},"systemPrompt":{"style":"none"},"readyPattern":"❯"}' >/dev/null
"$WBS" models add '{"id":"fake-cloud-1","label":"Fake Cloud","harness":"fakecli","provider":"fakecloud","modelRef":"fake-model","roles":["worker","orchestrator"],"maxEffort":"high","defaultEffort":"medium","machines":["mac","peer"],"workerClass":"mechanisch","enabled":true}' >/dev/null
out="$("$WBS" models resolve fake-cloud-1 --role worker --dir /tmp/x --name w1 2>&1)"
have "ohne Key: harter Fehler" "$out" "kein Key hinterlegt"

mkdir -p "$TESTHOME/.secrets-sync/api-keys"
printf 'sk-TESTSECRET-must-never-be-printed\n' >"$TESTHOME/.secrets-sync/api-keys/fakecloud"
chmod 644 "$TESTHOME/.secrets-sync/api-keys/fakecloud"
out="$("$WBS" models resolve fake-cloud-1 --role worker --dir /tmp/x --name w1 2>&1)"
have "Key-Datei mit Mode 644 wird abgelehnt" "$out" "verlangt 600"
chmod 600 "$TESTHOME/.secrets-sync/api-keys/fakecloud"
out="$("$WBS" models resolve fake-cloud-1 --role worker --dir /tmp/x --name w1 2>&1)"
have "Platzhalterkette statt Wert" "$out" "env	FAKE_KEY	@keychain:wb-fakecloud-test"
have "Platzhalterkette: Datei"     "$out" "@file:$TESTHOME/.secrets-sync/api-keys/fakecloud"
hasnt "resolve druckt NIE den Key" "$out" "sk-TESTSECRET"
out="$("$WBS" models table; "$WBS" models list; "$WBS" models get fake-cloud-1)"
hasnt "kein Key in list/get/table" "$out" "sk-TESTSECRET"

echo "== 6. Registry-Worker startet wirklich (fremder Harness, Ende zu Ende) =="
: >"$FAKELOG"
# add-worker only records into an EXISTING workbench state file — create one for
# this test dir so the sidebar entry (kind = harness id) can be checked at all.
"$WBS" touch "$TESTHOME/work" "$SESS" >/dev/null
out="$("$BIN/claude-worker" w-reg fake-cloud-1 "$TESTHOME/work" "Testaufgabe" 2>&1)"
LINE="$(startline w-reg)"
have "Pane laeuft ueber wb-harness-run" "$LINE" "wb-harness-run --model fake-cloud-1 --role worker"
hasnt "Startkommando enthaelt kein Secret" "$LINE" "sk-TESTSECRET"
hasnt "tmux-Optionen enthalten kein Secret" "$(tmux -L "$SOCK" show-options -p -t "$(tmux -L "$SOCK" list-panes -s -t "=$SESS" -F '#{pane_id} #{@wb_worker}' | awk '$2=="w-reg"{print $1;exit}')" 2>/dev/null)" "sk-TESTSECRET"
# Deadline instead of a fixed window — the same discipline the production path uses.
d=$(( $(date +%s) + 20 ))
until grep -q "ARGV faketui" "$FAKELOG" 2>/dev/null; do
  [ "$(date +%s)" -gt "$d" ] && break
  sleep 1
done
have "fremder CLI bekam das Modell" "$(cat "$FAKELOG")" "ARGV faketui --model fake-model"
have "Key kam als Umgebungsvariable an" "$(cat "$FAKELOG")" "ENV faketui FAKE_KEY=sk-TESTSECRET-must-never-be-printed"
# Reviewer-Befund H1: without a measured promptPattern nothing IS verified, so the
# output must not claim it was. fakecli has readyPattern but no promptPattern.
have "ohne promptPattern: ehrliche Meldung" "$out" "Submission NICHT verifizierbar"
have "ohne promptPattern: nennt den Ausweg" "$out" "wb-harness-probe fakecli"
hasnt "ohne promptPattern: keine Erfolgsbehauptung" "$out" "(Submission verifiziert)"
KINDLINE="$(/usr/bin/python3 - "$TESTHOME/.claude/workbench/sessions" <<'PY'
import json, os, sys
d = sys.argv[1]
for fn in sorted(os.listdir(d)) if os.path.isdir(d) else []:
    try: st = json.load(open(os.path.join(d, fn)))
    except Exception: continue
    for w in st.get("workers", []):
        print("%s|%s|%s" % (w.get("name"), w.get("kind"), w.get("model")))
PY
)"
have "State-Eintrag traegt den Harness als kind" "$KINDLINE" "w-reg|fakecli|fake-cloud-1:medium"
# With a measured promptPattern the same spawn verifies for real: the fake echoes the
# pasted text after '❯', so a hanging prompt would be caught.
"$WBS" models set --kind harness fakecli promptPattern '"^❯"' >/dev/null
out="$("$BIN/claude-worker" w-reg2 fake-cloud-1 "$TESTHOME/work" "Zweite Testaufgabe" 2>&1)"
have "mit promptPattern: wieder echte Verifikation" "$out" "(Submission verifiziert)"
"$WBS" models add --kind harness '{"id":"fakestuckcli","command":"fakestuck","args":[],"cwdMode":"cd","systemPrompt":{"style":"none"},"readyPattern":"\u276f","promptPattern":"^\u276f"}' >/dev/null
"$WBS" models add '{"id":"fake-stuck-1","harness":"fakestuckcli","provider":"ollama","modelRef":"fake-stuck","roles":["worker"],"defaultEffort":"low","workerClass":["bulk"]}' >/dev/null
out="$("$BIN/claude-worker" w-stuck fake-stuck-1 "$TESTHOME/work" "Task, der haengen bleibt" 2>&1)"; rc=$?
have "haengender Prompt wird erkannt" "$out" "Prompt haengt nach 3 Versuchen"
[ "$rc" -ne 0 ] && ok "haengender Prompt endet mit Fehler" || bad "haengender Prompt endet mit Fehler" "rc=$rc"

# M6: the SHELL side lets a registered foreign model be the default for both roles.
"$WBS" settings set workerModel fake-cloud-1 >/dev/null
"$WBS" settings set workerEffort medium >/dev/null
"$BIN/claude-worker" w-default default "$TESTHOME/work" >/dev/null 2>&1
have "Registry-Modell als Worker-Default" "$(startline w-default)" "wb-harness-run --model fake-cloud-1 --role worker"

echo "== 7. Startzeilen der eingebauten Aliase sind mit Registry unveraendert =="
tmux -L "$SOCK" kill-session -t "=$SESS" 2>/dev/null
mkses
tmux -L "$SOCK" set -p -t "$SESS" @wb_role orchestrator
"$BIN/claude-worker" w-claude sonnet5:high "$TESTHOME/work" >/dev/null 2>&1
"$BIN/pi-worker"     w-pi     ornith9      "$TESTHOME/work" >/dev/null 2>&1
eq "claude-Startzeile identisch" "$(startline w-claude)" "$CLAUDE_BEFORE"
eq "pi-Startzeile identisch"     "$(startline w-pi)"     "$PI_BEFORE"

echo "== 8. Schreiben: add/set/remove, Aenderungslog, kaputte Datei =="
"$WBS" models set fake-cloud-1 goodFor '"Testeintrag"' >/dev/null
eq "set schreibt" "$("$WBS" models get fake-cloud-1 --field goodFor)" "Testeintrag"
out="$("$WBS" models remove --kind harness fakecli 2>&1)"
have "benutzter Harness wird nicht entfernt" "$out" "wird noch benutzt von: fake-cloud-1"
"$WBS" models remove fake-cloud-1 >/dev/null
eq "remove entfernt" "$("$WBS" models list | grep -c fake-cloud-1)" "0"
have "Aenderungslog geschrieben" "$(cat "$TESTHOME/.local/state/wb-models-changes.log" 2>/dev/null)" "model:fake-cloud-1"

# `-` reads the JSON from stdin. This has to go through a real pipe into the real
# CLI: passing the same JSON as an argument exercises a different branch, which is
# exactly why the path could stay broken while the suite was green.
out="$(printf '%s' '{"id":"stdin-1","harness":"claude","provider":"claude-subscription","modelRef":"claude-sonnet-5","roles":["worker"],"workerClass":["bulk"]}' | "$WBS" models add --kind model - 2>&1)"
have "add liest JSON von stdin" "$out" "model 'stdin-1' hinzugefuegt"
eq "stdin-Eintrag ist wirklich da" "$("$WBS" models get stdin-1 --field modelRef)" "claude-sonnet-5"
out="$(printf '%s' '"Wert von stdin"' | "$WBS" models set stdin-1 goodFor - 2>&1)"
have "set liest den Wert von stdin" "$out" "gesetzt"
eq "stdin-Wert ist angekommen" "$("$WBS" models get stdin-1 --field goodFor)" "Wert von stdin"
out="$("$WBS" models remove --kind model - 2>&1)"
have "remove hat keinen stdin-Weg" "$out" "hat keine Eingabe von stdin"
out="$("$WBS" models add --kind model - </dev/null 2>&1)"
have "leeres stdin ist ein klarer Fehler" "$out" "kein gueltiges JSON"
eq "keine stdin-Temporaerdatei bleibt liegen" "$(ls "$TMPDIR"wb-state-stdin.* 2>/dev/null | wc -l | tr -d ' ')" "0"
"$WBS" models remove stdin-1 >/dev/null
# A write must leave a file that still matches the shipped default byte for byte —
# the missing trailing newline made every deploy check report drift after the first
# UI/CLI write (measured 2026-07-28).
cp "$REPO/models.default.json" "$TESTHOME/.claude/workbench/models.json"
"$WBS" models set --kind harness aider probe '"drift-test"' >/dev/null
# Lookup by id, not a fixed array index: a hardcoded [4] silently drifted onto
# 'opencode' once 'codex' was inserted earlier in the array and rotted this
# assertion long before the discover work (found 2026-07-29 while touching this
# file for other reasons — reproduced on a clean HEAD checkout, unrelated to it).
"$WBS" models set --kind harness aider probe "$(/usr/bin/python3 -c 'import json,sys
d=json.load(open(sys.argv[1]))
print(json.dumps(next(h for h in d["harnesses"] if h["id"]=="aider")["probe"]))' "$REPO/models.default.json")" >/dev/null
if diff -q "$REPO/models.default.json" "$TESTHOME/.claude/workbench/models.json" >/dev/null; then
  ok "Schreibvorgang laesst die Datei byteweise gleich"
else
  bad "Schreibvorgang laesst die Datei byteweise gleich" "$(diff "$REPO/models.default.json" "$TESTHOME/.claude/workbench/models.json" | head -3 | tr '\n' ' ')"
fi
# N4: a hand-written partial override keeps every field it does not mention.
printf '%s' '{"version":1,"harnesses":[{"id":"claude","label":"Claude Code (angepasst)"}]}' \
  >"$TESTHOME/.claude/workbench/models.json"
out="$("$WBS" harness get claude 2>/dev/null)"
have "Teil-Override behaelt das Ready-Muster" "$out" '"readyPattern": "❯|░|●"'
have "Teil-Override behaelt das Prompt-Muster" "$out" '"promptPattern": "^❯"'
have "Teil-Override uebernimmt das eigene Feld" "$out" "Claude Code (angepasst)"
out="$("$WBS" harness get claude 2>&1 >/dev/null)"
have "Teil-Override wird einmal gemeldet" "$out" "aus dem eingebauten Adapter ergaenzt"

printf 'kaputt{{{' >"$TESTHOME/.claude/workbench/models.json"
out="$("$WBS" models list 2>&1)"; eq "kaputte Registry: leer statt Absturz" "$out" ""
out="$("$WBS" harness get pi 2>&1)"; have "kaputte Registry: Eingebaute antworten" "$out" '"id": "pi"'

echo "== 9. wb-harness-probe: eigener Socket, eigenes HOME, raeumt restlos auf =="
# NB the probe calls `tmux -L wbprobe`, which the wrapper on PATH turns into
# `tmux -L wbtest -L wbprobe` — tmux takes the LAST -L (verified), so the probe
# still gets its own server and this test cannot reach the live one either way.
cp "$REPO/models.default.json" "$TESTHOME/.claude/workbench/models.json"
"$WBS" models add --kind harness '{"id":"fakecli","label":"Fake CLI","command":"faketui","args":["--model","{model}"],"cwdMode":"cd","systemPrompt":{"style":"none"}}' >/dev/null
"$WBS" models add '{"id":"fake-1","harness":"fakecli","provider":"ollama","modelRef":"fake-model","roles":["worker","orchestrator"],"defaultEffort":"medium","workerClass":"mechanisch"}' >/dev/null
out="$("$BIN/wb-harness-probe" fakecli --seconds 12 2>&1)"
have "probe schlaegt ein Muster vor" "$out" "VORSCHLAG readyPattern"
have "probe sah die Eingabezeile"    "$out" "❯"
have "probe nennt den Eintrag-Befehl" "$out" "models set --kind harness fakecli readyPattern"
tmux -L wbprobe ls >/dev/null 2>&1 && bad "probe raeumt seinen Socket ab" "Server laeuft noch" || ok "probe raeumt seinen Socket ab"
# Den Wegwerf-Adapter hier NEU anlegen: der aus Abschnitt 3 ist zu diesem Zeitpunkt
# nicht mehr registriert, und dann meldet die Probe "Harness unbekannt" statt "Binary
# fehlt" — der Test pruefte also eine andere Meldung als sein Name behauptet.
"$WBS" models add --kind harness '{"id":"nixdacli2","label":"gibt es nicht","command":"nixda-binary-7b3e","args":[],"cwdMode":"cd","systemPrompt":{"style":"none"},"readyPattern":"x","promptPattern":"^x"}' >/dev/null
out="$("$BIN/wb-harness-probe" nixdacli2 2>&1)"
have "probe ohne installiertes Binary" "$out" "nicht installiert — nichts gemessen"
"$WBS" models remove --kind harness nixdacli2 >/dev/null
"$WBS" models add --kind harness '{"id":"fakeslowcli","label":"Fake slow","command":"fakeslow","args":[],"cwdMode":"cd","systemPrompt":{"style":"none"}}' >/dev/null
"$WBS" models add '{"id":"fake-slow-1","harness":"fakeslowcli","provider":"ollama","modelRef":"fake-slow","roles":["worker"],"defaultEffort":"low","workerClass":["bulk"]}' >/dev/null
out="$("$BIN/wb-harness-probe" fakeslowcli --seconds 25 2>&1)"
have "probe wartet auf die echte Eingabezeile" "$out" "VORSCHLAG readyPattern: ^>"
BOX="$(printf '\xe2\x94\x80')"
hasnt "probe faellt nicht auf den Rahmen herein" "$out" "VORSCHLAG readyPattern: ^$BOX"
have "probe meldet die Wartezeit"  "$out" "Eingabezeile nach"

echo "== 10. wb-code: Orchestrator mit einem registrierten Harness =="
# wb-code ends in `exec tmux attach`, which fails without a terminal — by then the
# session exists with its start command, which is exactly what is checked here.
ODIR="$TESTHOME/orch"; mkdir -p "$ODIR"
# the workflow the probe above ends in: record the measured pattern, THEN the
# adapter is startable (before this line every start refuses, see section 3).
"$WBS" models set --kind harness fakecli readyPattern '"❯"' >/dev/null
"$BIN/wb-code" "$ODIR" --model fake-1 >/dev/null 2>&1
OSESS="$("$WBS" session "$ODIR")"
OLINE="$(tmux -L "$SOCK" list-panes -t "=$OSESS" -F '#{pane_start_command}' 2>/dev/null | head -1 | tr -d '\\')"
have "Harness kommt aus der Registry" "$OLINE" "wb-harness-run --model fake-1 --role orchestrator"
have "Orchestrator-Pane markiert" "$(tmux -L "$SOCK" list-panes -t "=$OSESS" -F '#{@wb_role}' 2>/dev/null | head -1)" "orchestrator"
ODIR2="$TESTHOME/orch2"; mkdir -p "$ODIR2"
"$WBS" settings set orchestratorHarness fakecli >/dev/null
"$WBS" settings set orchestratorModel fake-1 >/dev/null
"$WBS" settings set orchestratorEffort medium >/dev/null
"$BIN/wb-code" "$ODIR2" >/dev/null 2>&1
OSESS2="$("$WBS" session "$ODIR2")"
OLINE2="$(tmux -L "$SOCK" list-panes -t "=$OSESS2" -F '#{pane_start_command}' 2>/dev/null | head -1 | tr -d '\\')"
have "Registry-Modell als Orchestrator-Default" "$OLINE2" "wb-harness-run --model fake-1 --role orchestrator"
tmux -L "$SOCK" kill-session -t "=$OSESS2" 2>/dev/null || true

out="$("$BIN/wb-code" "$ODIR" --harness fakecli --model claude-opus-5 2>&1)"
have "Modell/Harness-Mischmasch wird abgelehnt" "$out" "gehoert zu Harness 'claude'"
out="$("$BIN/wb-code" "$ODIR" --harness gibtsnicht 2>&1)"
have "unbekannter Harness wird abgelehnt" "$out" "unbekannter Harness"
tmux -L "$SOCK" kill-session -t "=$OSESS" 2>/dev/null || true

echo "== 11. Stress-Befunde B04 B05 B07 B08 B15 B16 B17 =="
cp "$REPO/models.default.json" "$TESTHOME/.claude/workbench/models.json"

# B17 — kein Startkommando fuer ein Binary, das es nicht gibt.
"$WBS" models add --kind harness '{"id":"nobincli","command":"binary-gibts-nicht-4711","args":[],"cwdMode":"cd","systemPrompt":{"style":"none"},"readyPattern":"x","promptPattern":"^x"}' >/dev/null
"$WBS" models add '{"id":"nobin-model","harness":"nobincli","provider":"ollama","modelRef":"m","roles":["worker"],"workerClass":["bulk"]}' >/dev/null
out="$("$WBS" models resolve nobin-model --role worker --dir /tmp --name w1 2>&1)"; rc=$?
have "B17: fehlendes Binary bricht ab" "$out" "ist nicht installiert."
# Alle Startsperren kommen jetzt GEMEINSAM: wer ein frisches Preset benutzt, sieht in
# einem Durchgang, was ihm fehlt, statt es sich in drei Anlaeufen abzuholen.
have "Startsperren werden gesammelt gemeldet" "$out" "ist nicht startbar"
[ "$rc" -ne 0 ] && ok "B17: resolve endet mit Fehler" || bad "B17: resolve endet mit Fehler" "rc=$rc"
hasnt "B17: keine Startzeile" "$out" "cmd	"

# B04 — der Key-Gate haengt am PROVIDER, nicht an einem {secret:…} des Adapters.
# Binary existiert hier (/bin/echo), damit wirklich der Gate greift und nicht B17.
"$WBS" models add --kind harness '{"id":"echocli","command":"/bin/echo","args":["--model","{model}"],"cwdMode":"cd","systemPrompt":{"style":"none"},"readyPattern":"x","promptPattern":"^x"}' >/dev/null
"$WBS" models add '{"id":"echo-cloud","harness":"echocli","provider":"openai","modelRef":"m","roles":["worker"],"workerClass":["bulk"]}' >/dev/null
out="$("$WBS" models resolve echo-cloud --role worker --dir /tmp --name w1 2>&1)"
have "B04: Cloud-Modell ohne Key startet nicht" "$out" "kein Key hinterlegt (Provider 'openai')"
hasnt "B04: keine Startzeile ohne Key" "$out" "cmd	"
mkdir -p "$TESTHOME/.secrets-sync/api-keys"
printf 'sk-testkey-0123456789abcdefghij\n' >"$TESTHOME/.secrets-sync/api-keys/openai"
chmod 600 "$TESTHOME/.secrets-sync/api-keys/openai"
out="$("$WBS" models resolve echo-cloud --role worker --dir /tmp --name w1 2>/dev/null)"
have "B04: mit Key kommt der Key auch an" "$out" "env	OPENAI_API_KEY	@keychain:wb-openai"
hasnt "B04: der Wert selbst bleibt draussen" "$out" "sk-testkey"

# B08 — ein Klartext-Key wird beim SETZEN abgelehnt …
out="$("$WBS" models set --kind harness echocli env '{"OPENAI_API_KEY":"sk-abcdefghijklmnopqrstuvwxyz012345"}' 2>&1)"
have "B08: Klartext-Key wird abgelehnt" "$out" "sieht aus wie ein Schluessel"
have "B08: Ablehnung nennt den Platzhalter" "$out" "{secret:<provider>}"
# … und was von Hand in die Datei geschrieben wurde, wird angezeigt maskiert und
# ausgefuehrt gar nicht (Anzeige maskiert, Ausfuehrung verweigert).
/usr/bin/python3 - "$TESTHOME/.claude/workbench/models.json" <<'PY'
import json, sys
p = sys.argv[1]; d = json.load(open(p))
for h in d["harnesses"]:
    if h["id"] == "echocli":
        h["env"] = {"OPENAI_API_KEY": "sk-handgeschrieben-0123456789abcdef"}
json.dump(d, open(p, "w"), indent=2, ensure_ascii=False); open(p, "a").write("\n")
PY
out="$("$WBS" harness get echocli 2>&1)"
have "B08: Anzeige maskiert den Wert" "$out" "<maskiert:sk-h"
hasnt "B08: Anzeige zeigt den Key nicht" "$out" "handgeschrieben-0123"
out="$("$WBS" models resolve echo-cloud --role worker --dir /tmp --name w1 2>&1)"
have "B08: resolve verweigert einen Literalwert" "$out" "enthaelt einen Klartext-Schluessel"

# B07 — der alte Wert darf beim Korrigieren nicht ins Log wandern.
"$WBS" models set --kind harness echocli env '{"OPENAI_API_KEY":"{secret:openai}"}' >/dev/null
LOG="$TESTHOME/.local/state/wb-models-changes.log"
eq "B07: kein Klartext-Key im Aenderungslog" "$(grep -c 'handgeschrieben-0123' "$LOG" 2>/dev/null | head -1)" "0"
have "B07: das Log haelt die Aenderung trotzdem fest" "$(tail -1 "$LOG")" "<maskiert:sk-h"

# B15 — numerische Settings werden beim Setzen geprueft.
out="$("$WBS" settings set maxWorkers keine-zahl 2>&1)"; rc=$?
have "B15: unsinniger Zahlenwert wird abgelehnt" "$out" "ist eine ganze Zahl von 1 bis 64"
[ "$rc" -ne 0 ] && ok "B15: Ablehnung endet mit Fehler" || bad "B15: Ablehnung endet mit Fehler" "rc=$rc"
hasnt "B15: der Wert wurde nicht gespeichert" "$("$WBS" settings get maxWorkers)" "keine-zahl"
out="$("$WBS" settings set workerLayout quer 2>&1)"
have "B15: auch Enums werden geprueft" "$out" "kennt nur split|window"

# B16 — eine von Hand eingetragene Unsinnszahl darf keine rohe bash-Meldung erzeugen.
/usr/bin/python3 - "$TESTHOME/.claude/workbench/settings.json" <<'PY'
import json, os, sys
p = sys.argv[1]
cfg = json.load(open(p)) if os.path.exists(p) else {}
cfg["maxWorkers"] = "99999999999999999999"
json.dump(cfg, open(p, "w"), indent=2)
PY
out="$("$BIN/claude-worker" w-b16 sonnet5:low "$TESTHOME/work" 2>&1)"
hasnt "B16: keine rohe bash-Meldung im Terminal" "$out" "integer expression expected"
have "B16: der Sanitizer sagt, was er tut" "$out" "ist keine benutzbare Zahl"
"$WBS" settings set maxWorkers 8 >/dev/null

# B05 — ein Worker-Name wird nie zu einem Pfad ausserhalb von results/.
out="$("$BIN/claude-worker" '../evil' sonnet5:low "$TESTHOME/work" 'Aufgabe' 2>&1)"; rc=$?
have "B05: Pfadtrenner im Namen wird abgelehnt" "$out" "unzulaessiger Worker-Name"
[ "$rc" -ne 0 ] && ok "B05: Ablehnung endet mit Fehler" || bad "B05: Ablehnung endet mit Fehler" "rc=$rc"
[ ! -e "$TESTHOME/.pi-workers/evil" ] && ok "B05: nichts ausserhalb von results/ angelegt" \
  || bad "B05: nichts ausserhalb von results/ angelegt" "~/.pi-workers/evil existiert"

echo
echo "PASS=$PASS FAIL=$FAIL"
[ "$FAIL" -eq 0 ]
