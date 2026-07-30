#!/bin/bash
# test-models-discover.sh — automated tests for 'wb-state models discover'
# (SPEC-V3-MODELS discover, Vertrag scratchpad/CONTRACT-discover.md 2026-07-29).
#
# WARNUNG wie in test-registry.sh: dieser Test ruft die INSTALLIERTEN Skripte
# unter der eigenen Test-PATH auf. Nicht waehrend eines Laufs nach ~/.local/bin
# deployen — das erzeugt Phantom-Fehlschlaege.
#
# ISOLATION (Regeln 2026-07-25/29, wie test-registry.sh):
#   * unset TMUX TMUX_PANE zuerst, eigenes HOME (mktemp -d) — das echte
#     ~/.claude/workbench/models.json wird hier nie gelesen oder geschrieben.
#   * JEDE discover-Quelle ist eine WEGWERF-Fixture, die auf ein Fake-Binary
#     bzw. eine Fake-Datei unter der eigenen Test-PATH/HOME zeigt — nie ein
#     ausgeliefertes Preset (pi/aider/opencode/codex/agy) und nie der echte
#     'ollama'/'opencode'/'agy'-Befehl dieser Maschine: ein Test, der davon
#     abhaengt, was hier zufaellig installiert ist, testet die Maschine statt
#     den Code (Regel 2026-07-29).
#   * eigene, harnessfremde ids (fixollama/fixcmd/fixfile/fixdry) — kollidieren
#     mit nichts Kuratiertem, egal was models.default.json gerade enthaelt.
#
# Run: shell/tests/test-models-discover.sh
unset TMUX TMUX_PANE
set -uo pipefail

REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"     # …/claude-workbench/shell
PASS=0; FAIL=0

TESTHOME="$(cd "$(mktemp -d "${TMPDIR:-/tmp}/wb-discover-test.XXXXXX")" && pwd)"
export HOME="$TESTHOME"
BIN="$TESTHOME/.local/bin"; mkdir -p "$BIN" "$TESTHOME/.claude/workbench" "$TESTHOME/.local/state"
export TMPDIR="$TESTHOME/tmp/"; mkdir -p "$TMPDIR"

cleanup() { rm -rf "$TESTHOME"; }
trap cleanup EXIT INT TERM

ok()   { PASS=$((PASS+1)); printf '  ok   %s\n' "$1"; }
bad()  { FAIL=$((FAIL+1)); printf '  FAIL %s\n' "$1"; [ $# -gt 1 ] && printf '       %s\n' "$2"; }
have() { case "$2" in *"$3"*) ok "$1" ;; *) bad "$1" "erwartet '$3' in: $(printf '%s' "$2" | head -3 | tr '\n' ' ')" ;; esac; }
hasnt(){ case "$2" in *"$3"*) bad "$1" "'$3' steht in der Ausgabe, darf es aber nicht" ;; *) ok "$1" ;; esac; }
eq()   { if [ "$2" = "$3" ]; then ok "$1"; else bad "$1" "'$2' != '$3'"; fi; }
exists(){ "$WBS" models get "$1" --field id >/dev/null 2>&1; }   # true = Eintrag da

cp "$REPO/wb-state" "$BIN/wb-state"; chmod +x "$BIN/wb-state"
export PATH="$BIN:$PATH"
WBS="$BIN/wb-state"
MODELSFILE="$TESTHOME/.claude/workbench/models.json"
STATEFILE="$TESTHOME/.local/state/wb-models-discover.json"

# ── Fixture 1: source 'ollama' — ein FAKES ollama-Binary, nie das echte ─────────
cat >"$BIN/ollama" <<EOF
#!/bin/bash
[ "\$1" = "list" ] && cat "$TESTHOME/ollama-list.txt"
EOF
chmod +x "$BIN/ollama"
cat >"$TESTHOME/ollama-list.txt" <<'EOF'
NAME                            ID        SIZE   MODIFIED
curated-ref                     a1        1 GB   1 day ago
alpha-model                     a2        1 GB   1 day ago
beta-model                      a3        1 GB   1 day ago
EOF

# ── Fixture 2: source 'command-lines' — ein Stub, der gezielt scheitern kann ────
cat >"$BIN/fixcmdtool" <<EOF
#!/bin/bash
if [ -f "$TESTHOME/fixcmd-fail" ]; then
  echo "fixcmdtool: kaputt" >&2
  exit 7
fi
cat "$TESTHOME/fixcmd-list.txt"
EOF
chmod +x "$BIN/fixcmdtool"
cat >"$TESTHOME/fixcmd-list.txt" <<'EOF'
cmd-ref-one
cmd-ref-two
EOF

# ── Fixture 3: source 'file-json' — eine Wegwerf-JSON-Datei, nie der echte
# Codex-Cache ─────────────────────────────────────────────────────────────────
FIXFILE="$TESTHOME/fixture-models.json"
cat >"$FIXFILE" <<'EOF'
{"models":[
  {"slug":"file-ref-one","visibility":"list"},
  {"slug":"file-ref-hidden","visibility":"hidden"},
  {"slug":"file-ref-two","visibility":"list"}
]}
EOF

# ── die drei Wegwerf-Harnesses registrieren ─────────────────────────────────────
# fixollama: effort.map traegt bewusst auch 'max'/'ultra', um Vertrag-3-Regel
# ("landen nie in efforts") direkt am generierten Eintrag zu pruefen. refTemplate
# 'pfx/{name}' prueft die woertliche Vertrags-Regel ueber den Slug der FERTIGEN
# modelRef (nicht der rohen Quell-Referenz).
"$WBS" models add --kind harness '{"id":"fixollama","command":"/bin/echo","args":[],"cwdMode":"cd","systemPrompt":{"style":"none"},"readyPattern":"x","promptPattern":"^x","effort":{"style":"arg","args":["--effort","{effort}"],"map":{"low":"low","medium":"medium","high":"high","xhigh":"xhigh","max":"max","ultra":"ultra"}},"discover":{"source":"ollama","provider":"ollama","refTemplate":"pfx/{name}"}}' >/dev/null

"$WBS" models add --kind harness '{"id":"fixcmd","command":"/bin/echo","args":[],"cwdMode":"cd","systemPrompt":{"style":"none"},"readyPattern":"x","promptPattern":"^x","discover":{"source":"command-lines","command":["fixcmdtool"],"provider":"ollama"}}' >/dev/null

FIXFILE_HARNESS_JSON=$(printf '{"id":"fixfile","command":"/bin/echo","args":[],"cwdMode":"cd","systemPrompt":{"style":"none"},"readyPattern":"x","promptPattern":"^x","discover":{"source":"file-json","file":"%s","jsonPath":"models[].slug","filter":{"field":"visibility","equals":"list"},"provider":"ollama"}}' "$FIXFILE")
"$WBS" models add --kind harness "$FIXFILE_HARNESS_JSON" >/dev/null

# Kuratierter Eintrag VOR dem ersten Lauf: (harness=fixollama, modelRef=pfx/curated-ref)
# muss den automatischen Zwilling verhindern (Vertrag 4.2).
"$WBS" models add '{"id":"my-curated","harness":"fixollama","provider":"ollama","modelRef":"pfx/curated-ref","roles":["worker"],"workerClass":["bulk"],"goodFor":"curated fixture"}' >/dev/null

echo "== 1. Neuanlage + kuratierter Eintrag verhindert das Duplikat + roles-Default =="
out="$("$WBS" models discover fixollama --json)"
have "neue ids: alpha" "$out" "fixollama-pfx-alpha-model"
have "neue ids: beta"  "$out" "fixollama-pfx-beta-model"
hasnt "kein Zwilling fuer den kuratierten Eintrag" "$out" "fixollama-pfx-curated-ref"
eq "curated-ref bleibt als my-curated stehen" "$("$WBS" models get my-curated --field modelRef)" "pfx/curated-ref"
have "kept ist 0 beim ersten Lauf" "$out" '"kept": 0'
eq "roles-Default: auch Orchestrator" "$("$WBS" models get fixollama-pfx-alpha-model --field roles)" '["worker", "orchestrator"]'
eq "machines fest mac+peer" "$("$WBS" models get fixollama-pfx-alpha-model --field machines)" '["mac", "peer"]'
eq "source ist auto" "$("$WBS" models get fixollama-pfx-alpha-model --field source)" "auto"

echo "== 2. max/ultra landen nie in efforts (Regel 2026-07-25) =="
EFF="$("$WBS" models get fixollama-pfx-alpha-model --field efforts)"
eq "efforts ohne max/ultra" "$EFF" '["low", "medium", "high", "xhigh"]'
hasnt "kein max in efforts" "$EFF" "max"
hasnt "kein ultra in efforts" "$EFF" "ultra"
eq "maxEffort ist xhigh" "$("$WBS" models get fixollama-pfx-alpha-model --field maxEffort)" "xhigh"
eq "defaultEffort ist medium" "$("$WBS" models get fixollama-pfx-alpha-model --field defaultEffort)" "medium"

echo "== 3. unveraendert zweiter Lauf =="
out2="$("$WBS" models discover fixollama --json)"
have "zweiter Lauf: nichts neu" "$out2" '"added": []'
have "zweiter Lauf: nichts aktualisiert" "$out2" '"updated": []'
have "zweiter Lauf: nichts entfernt" "$out2" '"removed": []'
have "zweiter Lauf: 2 unveraendert" "$out2" '"kept": 2'

echo "== 4. enabled:false ueberlebt einen erneuten Lauf =="
"$WBS" models set fixollama-pfx-alpha-model enabled false >/dev/null
"$WBS" models discover fixollama --json >/dev/null
eq "enabled bleibt false" "$("$WBS" models get fixollama-pfx-alpha-model --field enabled)" "false"

echo "== 5. verschwundene Referenz wird entfernt =="
cat >"$TESTHOME/ollama-list.txt" <<'EOF'
NAME                            ID        SIZE   MODIFIED
curated-ref                     a1        1 GB   1 day ago
alpha-model                     a2        1 GB   1 day ago
EOF
out5="$("$WBS" models discover fixollama --json)"
have "beta-model steht als entfernt im Ergebnis" "$out5" "fixollama-pfx-beta-model"
have "removed ist nicht leer" "$out5" '"removed": ["fixollama-pfx-beta-model"]'
exists fixollama-pfx-beta-model && bad "beta-model wirklich weg" "existiert noch" || ok "beta-model wirklich weg"
exists fixollama-pfx-alpha-model && ok "alpha-model bleibt (disabled, aber vorhanden)" || bad "alpha-model bleibt (disabled, aber vorhanden)"
exists my-curated && ok "kuratierter Eintrag unangetastet" || bad "kuratierter Eintrag unangetastet"

echo "== 6. file-json + filter: nur visibility==list, echte Codex-Cache-Form =="
out6="$("$WBS" models discover fixfile --json)"
have "file-ref-one erkannt" "$out6" "fixfile-file-ref-one"
have "file-ref-two erkannt" "$out6" "fixfile-file-ref-two"
hasnt "hidden bleibt draussen (filter)" "$out6" "file-ref-hidden"
exists fixfile-file-ref-hidden && bad "hidden wurde nicht angelegt" "existiert" || ok "hidden wurde nicht angelegt"
eq "opencode-Stil-Referenz style=none -> supportsEffort false" "$("$WBS" models get fixfile-file-ref-one --field supportsEffort)" "false"
OUT_GET="$("$WBS" models get fixfile-file-ref-one 2>&1)"
hasnt "keine efforts, wenn supportsEffort false" "$OUT_GET" '"efforts"'

echo "== 7. gescheiterte Quelle entfernt NICHTS =="
out7a="$("$WBS" models discover fixcmd --json)"
have "fixcmd erste Erkennung: 2 neu" "$out7a" '"added": ["fixcmd-cmd-ref-one", "fixcmd-cmd-ref-two"]'
: >"$TESTHOME/fixcmd-fail"
out7b="$("$WBS" models discover fixcmd --json)"; rc7b=$?
have "Fehler wird gemeldet" "$out7b" '"error": "fixcmdtool'
[ "$rc7b" -ne 0 ] && ok "einzig angeforderter Harness gescheitert -> exit != 0" \
  || bad "einzig angeforderter Harness gescheitert -> exit != 0" "rc=$rc7b"
exists fixcmd-cmd-ref-one && ok "cmd-ref-one bleibt trotz gescheiterter Quelle" \
  || bad "cmd-ref-one bleibt trotz gescheiterter Quelle" "wurde entfernt"
exists fixcmd-cmd-ref-two && ok "cmd-ref-two bleibt trotz gescheiterter Quelle" \
  || bad "cmd-ref-two bleibt trotz gescheiterter Quelle" "wurde entfernt"
have "kept zaehlt die unberuehrten Eintraege mit" "$out7b" '"kept": 2'

echo "== 8. Exit-Code: nur wenn ALLE angeforderten Harnesses scheitern =="
outmix="$("$WBS" models discover fixfile fixcmd --json)"; rcmix=$?
[ "$rcmix" -eq 0 ] && ok "ein Erfolg neben einem Fehlschlag -> exit 0" \
  || bad "ein Erfolg neben einem Fehlschlag -> exit 0" "rc=$rcmix"
outboth="$("$WBS" models discover fixcmd doesnotexist --json)"; rcboth=$?
have "unbekannter Harness meldet 'kein discover-Block'" "$outboth" "kein discover-Block fuer 'doesnotexist'"
[ "$rcboth" -ne 0 ] && ok "beide angeforderten Harnesses gescheitert -> exit != 0" \
  || bad "beide angeforderten Harnesses gescheitert -> exit != 0" "rc=$rcboth"
rm -f "$TESTHOME/fixcmd-fail"

echo "== 9. --dry-run rechnet, schreibt aber NICHTS =="
"$WBS" models add --kind harness '{"id":"fixdry","command":"/bin/echo","args":[],"cwdMode":"cd","systemPrompt":{"style":"none"},"readyPattern":"x","promptPattern":"^x","discover":{"source":"ollama","provider":"ollama"}}' >/dev/null
SUM_BEFORE=$(md5 -q "$MODELSFILE" 2>/dev/null || md5sum "$MODELSFILE" | cut -d' ' -f1)
[ -f "$STATEFILE" ] && SUM_STATE_BEFORE=$(md5 -q "$STATEFILE" 2>/dev/null || md5sum "$STATEFILE" | cut -d' ' -f1) || SUM_STATE_BEFORE="(fehlt)"
outdry="$("$WBS" models discover fixdry --dry-run --json)"
have "dry-run rechnet trotzdem: meldet die neuen ids" "$outdry" "fixdry-alpha-model"
exists fixdry-alpha-model && bad "dry-run legt nichts an" "fixdry-alpha-model existiert" || ok "dry-run legt nichts an"
SUM_AFTER=$(md5 -q "$MODELSFILE" 2>/dev/null || md5sum "$MODELSFILE" | cut -d' ' -f1)
eq "models.json bleibt byteweise gleich" "$SUM_BEFORE" "$SUM_AFTER"
if [ -f "$STATEFILE" ]; then SUM_STATE_AFTER=$(md5 -q "$STATEFILE" 2>/dev/null || md5sum "$STATEFILE" | cut -d' ' -f1); else SUM_STATE_AFTER="(fehlt)"; fi
eq "Discover-Zeitstempel bleibt unveraendert" "$SUM_STATE_BEFORE" "$SUM_STATE_AFTER"

echo "== 10. --if-stale ueberspringt innerhalb der TTL =="
have "Zeitstempel fuer fixfile wurde beim echten Lauf (Abschnitt 6) gesetzt" \
  "$(cat "$STATEFILE" 2>/dev/null)" '"fixfile"'
outstale="$("$WBS" models discover fixfile --if-stale --json)"
eq "frischer Harness liefert ein leeres Ergebnis" "$outstale" "{}"
# ein noch nie erkannter Harness ist NIE 'frisch genug' und wird trotzdem erkannt.
outstale2="$("$WBS" models discover fixdry --if-stale --json)"
have "unbekannter Harness wird trotz --if-stale erkannt" "$outstale2" "fixdry-alpha-model"

echo
echo "PASS=$PASS FAIL=$FAIL"
[ "$FAIL" -eq 0 ]
