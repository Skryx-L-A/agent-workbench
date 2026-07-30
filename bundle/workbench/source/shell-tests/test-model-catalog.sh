#!/bin/bash
# test-model-catalog.sh — automated tests for provider catalogs / http-json discover
# (SPEC-V3-MODELS discover, Vertrag Teil 2, scratchpad/CONTRACT-katalog.md 2026-07-29).
#
# WARNUNG wie in den Nachbar-Suiten: ruft die INSTALLIERTEN Skripte unter der eigenen
# Test-PATH auf. Nicht waehrend eines Laufs nach ~/.local/bin deployen.
#
# ISOLATION (Regel 2026-07-29): NIE ein echter Anbieter im Test. Ein eigener lokaler
# HTTP-Server auf 127.0.0.1 (stub-catalog-server.py, throwaway) steht fuer OpenRouter
# & Co ein; /usr/bin/curl selbst bleibt unangetastet (wie im Vertrag verlangt) — nur
# die URL im Fixture-Provider zeigt auf den Stub statt auf das echte Internet.
# unset TMUX TMUX_PANE, HOME=$(mktemp -d), eigener PATH.
#
# Run: shell/tests/test-model-catalog.sh
unset TMUX TMUX_PANE
set -uo pipefail

REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"     # …/claude-workbench/shell
PASS=0; FAIL=0

TESTHOME="$(cd "$(mktemp -d "${TMPDIR:-/tmp}/wb-catalog-test.XXXXXX")" && pwd)"
export HOME="$TESTHOME"
BIN="$TESTHOME/.local/bin"; mkdir -p "$BIN" "$TESTHOME/.claude/workbench" "$TESTHOME/.local/state"
export TMPDIR="$TESTHOME/tmp/"; mkdir -p "$TMPDIR"
RESP_DIR="$TESTHOME/stub-responses"; mkdir -p "$RESP_DIR"
REQLOG="$TESTHOME/stub-requests.log"; : >"$REQLOG"
PORTFILE="$TESTHOME/stub-port"
SRVPID=""

cleanup() {
  [ -n "$SRVPID" ] && kill "$SRVPID" 2>/dev/null || true
  [ -n "$SRVPID" ] && wait "$SRVPID" 2>/dev/null || true
  rm -rf "$TESTHOME"
}
trap cleanup EXIT INT TERM

ok()   { PASS=$((PASS+1)); printf '  ok   %s\n' "$1"; }
bad()  { FAIL=$((FAIL+1)); printf '  FAIL %s\n' "$1"; [ $# -gt 1 ] && printf '       %s\n' "$2"; }
have() { case "$2" in *"$3"*) ok "$1" ;; *) bad "$1" "erwartet '$3' in: $(printf '%s' "$2" | head -3 | tr '\n' ' ')" ;; esac; }
hasnt(){ case "$2" in *"$3"*) bad "$1" "'$3' steht in der Ausgabe, darf es aber nicht" ;; *) ok "$1" ;; esac; }
eq()   { if [ "$2" = "$3" ]; then ok "$1"; else bad "$1" "'$2' != '$3'"; fi; }
exists(){ "$WBS" models get "$1" --field id >/dev/null 2>&1; }
# 'models get' kennt nur Modelle, keine Provider — fuer catalogStatus/balanceStatus
# (die am PROVIDER haengen, Nachtrag 2026-07-29) direkt in der Registry-Datei
# nachsehen. Reines Lesen, kein wb-state-Aufruf.
provider_field() { # provider_field <id> <field>
  /usr/bin/python3 -c "
import json,sys
d = json.load(open(sys.argv[1]))
p = next((x for x in d.get('providers',[]) if x.get('id')==sys.argv[2]), None)
print(json.dumps((p or {}).get(sys.argv[3])))
" "$MODELSFILE" "$1" "$2"
}

cp "$REPO/wb-state" "$BIN/wb-state"; chmod +x "$BIN/wb-state"
export PATH="$BIN:$PATH"
WBS="$BIN/wb-state"
MODELSFILE="$TESTHOME/.claude/workbench/models.json"
CATALOG_CACHE_DIR="$TESTHOME/.local/state/wb-model-catalog"
CHANGELOG="$TESTHOME/.local/state/wb-models-changes.log"
DISCOVER_STATE="$TESTHOME/.local/state/wb-models-discover.json"

FAKEKEY="FAKETESTKEY-99887766-DO-NOT-LEAK-ANYWHERE"

# ── der Stub-HTTP-Server: eine eigene Wegwerf-Gegenstelle, nie ein echter Anbieter ──
/usr/bin/python3 "$REPO/tests/stub-catalog-server.py" "$RESP_DIR" "$REQLOG" "$PORTFILE" &
SRVPID=$!
d=$(( $(date +%s) + 10 ))
while [ ! -s "$PORTFILE" ]; do
  [ "$(date +%s)" -gt "$d" ] && { echo "ABBRUCH: Stub-Server kam nicht hoch." >&2; exit 1; }
  sleep 0.1
done
PORT="$(cat "$PORTFILE")"
BASE="http://127.0.0.1:$PORT"
echo "== Stub-Server auf $BASE (PID $SRVPID) =="

# 5 Modelle, 2 davon "0"/"0" (kostenlos) — deckt sich mit der Vertrags-Messung
# (17 von 367 bei OpenRouter): dieselbe Regel, andere Zahlen.
cat >"$RESP_DIR/body_models.json" <<'EOF'
{"data": [
  {"id": "vendor/model-a", "pricing": {"prompt": "0.000001", "completion": "0.000002"}},
  {"id": "vendor/model-b", "pricing": {"prompt": "0.000003", "completion": "0.000004"}},
  {"id": "vendor/free-c",  "pricing": {"prompt": "0", "completion": "0"}},
  {"id": "vendor/free-d",  "pricing": {"prompt": "0", "completion": "0"}},
  {"id": "vendor/model-e", "pricing": {"prompt": "0.000005", "completion": "0.000006"}}
]}
EOF
cp "$RESP_DIR/body_models.json" "$RESP_DIR/body_models-req.json"
echo '{"data": {"limit_remaining": 0}}' >"$RESP_DIR/body_key.json"

# ── Fixture-Provider/-Harnesses — eigene ids, nie ein ausgeliefertes Preset ──────
"$WBS" models add --kind provider "{\"id\":\"fixcat\",\"label\":\"Fixture Katalog (oeffentlich)\",\"kind\":\"cloud\",\"apiKeyEnv\":\"FIXCAT_KEY\",\"discover\":{\"source\":\"http-json\",\"url\":\"$BASE/models\",\"jsonPath\":\"data[].id\",\"auth\":{\"header\":\"Authorization\",\"value\":\"Bearer {key}\",\"required\":false},\"priceFields\":{\"in\":\"pricing.prompt\",\"out\":\"pricing.completion\"},\"cacheHours\":0.0006,\"maxModels\":1000},\"balance\":{\"url\":\"$BASE/key\",\"jsonPath\":\"data.limit_remaining\",\"freeWhenEmpty\":true},\"runners\":[\"fixrunner\"],\"defaultRunner\":\"fixrunner\"}" >/dev/null
"$WBS" models add --kind provider "{\"id\":\"fixcatreq\",\"label\":\"Fixture Katalog (Key Pflicht)\",\"kind\":\"cloud\",\"apiKeyEnv\":\"FIXCAT_KEY\",\"discover\":{\"source\":\"http-json\",\"url\":\"$BASE/models-req\",\"jsonPath\":\"data[].id\",\"auth\":{\"header\":\"Authorization\",\"value\":\"Bearer {key}\",\"required\":true},\"cacheHours\":24,\"maxModels\":1000},\"runners\":[\"fixrunner\"],\"defaultRunner\":\"fixrunner\"}" >/dev/null
"$WBS" models add --kind provider "{\"id\":\"fixcatcap\",\"label\":\"Fixture Katalog (maxModels)\",\"kind\":\"cloud\",\"apiKeyEnv\":\"FIXCAT_KEY\",\"discover\":{\"source\":\"http-json\",\"url\":\"$BASE/models\",\"jsonPath\":\"data[].id\",\"auth\":{\"header\":\"Authorization\",\"value\":\"Bearer {key}\",\"required\":true},\"cacheHours\":24,\"maxModels\":3},\"runners\":[\"fixrunner\"],\"defaultRunner\":\"fixrunner\"}" >/dev/null
"$WBS" models add --kind harness '{"id":"fixrunner","command":"/bin/echo","args":[],"cwdMode":"cd","systemPrompt":{"style":"none"},"readyPattern":"x","promptPattern":"^x","effort":{"style":"arg","args":["--effort","{effort}"],"map":{"low":"low","medium":"medium","high":"high","xhigh":"xhigh"}}}' >/dev/null
cat >"$BIN/fixlocaltool" <<'EOF'
#!/bin/bash
printf 'local-ref-one\nlocal-ref-two\n'
EOF
chmod +x "$BIN/fixlocaltool"
"$WBS" models add --kind harness '{"id":"fixlocal","command":"/bin/echo","args":[],"cwdMode":"cd","systemPrompt":{"style":"none"},"readyPattern":"x","promptPattern":"^x","discover":{"source":"command-lines","command":["fixlocaltool"],"provider":"ollama"}}' >/dev/null

echo "== 1. kein Key: oeffentlicher Katalog wird geholt und ANGEZEIGT, aber nichts registriert =="
unset FIXCAT_KEY
out1="$("$WBS" models discover fixcat --json)"
have "fixcat ohne Key: nichts hinzugefuegt" "$out1" '"fixcat": {"added": []'
have "fixcat ohne Key: Katalogumfang trotzdem sichtbar (5)" "$out1" '"catalogSize": 5'
have "fixcat ohne Key: kostenlos-Zahl sichtbar (2)" "$out1" '"freeCount": 2'
have "fixcat ohne Key: hasKey false" "$out1" '"hasKey": false'
have "fixcat ohne Key: mode no-key" "$out1" '"mode": "no-key"'
have "oeffentlicher Endpunkt wurde wirklich abgefragt" "$(cat "$REQLOG")" "/models"
eq "kein Registry-Eintrag unter fixrunner-fixcat" "$("$WBS" models list --all | grep -c '^fixrunner-fixcat-')" "0"
have "catalogStatus trotzdem auf dem Provider vermerkt" "$(provider_field fixcat catalogStatus 2>&1)" '"count": 5'

: >"$REQLOG"
out1b="$("$WBS" models discover fixcatreq --json)"
have "fixcatreq (Key Pflicht) ohne Key: gar nicht erst abgerufen" "$out1b" '"mode": "no-key"'
eq "auth.required=true: kein einziger Request beim Stub" "$(cat "$REQLOG" | wc -l | tr -d ' ')" "0"
have "fixcatreq ohne Key: kein Katalogumfang bekannt" "$out1b" '"catalogSize": null'

echo "== 2. Key ohne Guthaben: nur die kostenlosen =="
export FIXCAT_KEY="$FAKEKEY"
: >"$REQLOG"
out2="$("$WBS" models discover fixcat fixcatreq --json)"
have "fixcat mit Key, Guthaben 0: nur 2 kostenlose neu" "$out2" '"fixcat": {"added": ["fixrunner-fixcat-vendor-free-c", "fixrunner-fixcat-vendor-free-d"]'
have "fixcatreq (kein balance-Block): Key vorhanden = alles (5)" "$out2" '"fixcatreq": {"added":'
eq "fixrunner-fixcat: genau 2 registriert" "$("$WBS" models list --all | grep -c '^fixrunner-fixcat-')" "2"
eq "fixrunner-fixcatreq: alle 5 registriert (kein balance-Block)" "$("$WBS" models list --all | grep -c '^fixrunner-fixcatreq-')" "5"
have "Key kam beim Stub als Bearer-Header an" "$(cat "$REQLOG")" "Bearer $FAKEKEY"
have "fixcat-Eintrag traegt Effort-Felder vom Runner-Harness" "$("$WBS" models get fixrunner-fixcat-vendor-free-c --field efforts)" 'xhigh'
hasnt "kein automatischer Eintrag bekommt workerClass" "$("$WBS" models get fixrunner-fixcat-vendor-free-c 2>&1)" 'workerClass'

echo "== 3. Key MIT Guthaben: alle Katalogmodelle =="
echo '{"data": {"limit_remaining": 12.5}}' >"$RESP_DIR/body_key.json"
sleep 3    # cacheHours=0.0006 (~2.16s) muss abgelaufen sein, sonst greift noch der alte Cache
out3="$("$WBS" models discover fixcat --json)"
have "jetzt alle 5, die 3 fehlenden werden neu ergaenzt" "$out3" '"kept": 2'
eq "fixrunner-fixcat: jetzt alle 5" "$("$WBS" models list --all | grep -c '^fixrunner-fixcat-')" "5"
have "balanceStatus auf dem Provider (Guthaben 12.5)" "$(provider_field fixcat balanceStatus 2>&1)" '"amount": 12.5'
out3cat="$("$WBS" models catalog --json)"
have "'models catalog' zeigt fixcat: 5 Katalog, 2 kostenlos, Key ja" "$out3cat" '"fixcat": {"provider": "fixcat", "catalogSize": 5, "freeCount": 2, "hasKey": true'
have "'models catalog' zeigt registrierte Anzahl" "$out3cat" '"registered": 5'

echo "== 4. maxModels deckelt =="
out4="$("$WBS" models discover fixcatcap --json)"
have "maxModels=3: nur 3 werden registriert" "$out4" '"fixcatcap": {"added":'
eq "fixrunner-fixcatcap: genau 3 (von 5 im Katalog)" "$("$WBS" models list --all | grep -c '^fixrunner-fixcatcap-')" "3"
eq "andere Anbieter unter demselben Runner unangetastet (fixcat)" "$("$WBS" models list --all | grep -c '^fixrunner-fixcat-')" "5"
eq "andere Anbieter unter demselben Runner unangetastet (fixcatreq)" "$("$WBS" models list --all | grep -c '^fixrunner-fixcatreq-')" "5"

echo "== 4b. Guthaben nicht lesbar und Guthaben aus zwei Zahlen (Regression 2026-07-29) =="
# Der Fehler, den das hier festhaelt: `freeWhenEmpty` war invertiert — ein Guthaben,
# das sich nicht lesen laesst, fuehrte zu "alles registrieren" statt "nur kostenlose".
# Live hat das 350 unbezahlbare OpenRouter-Modelle in die Registry geschrieben, obwohl
# das Konto 0 Guthaben hat. Der Vertrag sagt: 0 ODER unbekannt -> nur kostenlose.
echo '{"data": {"limit_remaining": null}}' >"$RESP_DIR/body_keynull.json"
"$WBS" models add --kind provider "{\"id\":\"fixbalnull\",\"label\":\"Fixture Guthaben unlesbar\",\"kind\":\"cloud\",\"apiKeyEnv\":\"FIXCAT_KEY\",\"discover\":{\"source\":\"http-json\",\"url\":\"$BASE/models\",\"jsonPath\":\"data[].id\",\"auth\":{\"header\":\"Authorization\",\"value\":\"Bearer {key}\",\"required\":true},\"priceFields\":{\"in\":\"pricing.prompt\",\"out\":\"pricing.completion\"},\"cacheHours\":24,\"maxModels\":1000},\"balance\":{\"url\":\"$BASE/keynull\",\"jsonPath\":\"data.limit_remaining\",\"freeWhenEmpty\":true},\"runners\":[\"fixrunner\"],\"defaultRunner\":\"fixrunner\"}" >/dev/null
outbalnull="$("$WBS" models discover fixbalnull --json)"
have "Guthaben nicht lesbar: mode free, nicht all" "$outbalnull" '"mode": "free"'
eq "Guthaben nicht lesbar: genau die 2 kostenlosen registriert" \
  "$("$WBS" models list --all | grep -c '^fixrunner-fixbalnull-')" "2"

# Zwei Zahlen statt einer: total_credits minus total_usage (OpenRouters /api/v1/credits).
echo '{"data": {"total_credits": 5, "total_usage": 5}}' >"$RESP_DIR/body_credzero.json"
echo '{"data": {"total_credits": 10, "total_usage": 2}}' >"$RESP_DIR/body_credplus.json"
"$WBS" models add --kind provider "{\"id\":\"fixbalzero\",\"label\":\"Fixture Guthaben aufgebraucht\",\"kind\":\"cloud\",\"apiKeyEnv\":\"FIXCAT_KEY\",\"discover\":{\"source\":\"http-json\",\"url\":\"$BASE/models\",\"jsonPath\":\"data[].id\",\"auth\":{\"header\":\"Authorization\",\"value\":\"Bearer {key}\",\"required\":true},\"priceFields\":{\"in\":\"pricing.prompt\",\"out\":\"pricing.completion\"},\"cacheHours\":24,\"maxModels\":1000},\"balance\":{\"url\":\"$BASE/credzero\",\"jsonPath\":\"data.total_credits\",\"minusJsonPath\":\"data.total_usage\",\"freeWhenEmpty\":true},\"runners\":[\"fixrunner\"],\"defaultRunner\":\"fixrunner\"}" >/dev/null
"$WBS" models add --kind provider "{\"id\":\"fixbalplus\",\"label\":\"Fixture Guthaben vorhanden\",\"kind\":\"cloud\",\"apiKeyEnv\":\"FIXCAT_KEY\",\"discover\":{\"source\":\"http-json\",\"url\":\"$BASE/models\",\"jsonPath\":\"data[].id\",\"auth\":{\"header\":\"Authorization\",\"value\":\"Bearer {key}\",\"required\":true},\"priceFields\":{\"in\":\"pricing.prompt\",\"out\":\"pricing.completion\"},\"cacheHours\":24,\"maxModels\":1000},\"balance\":{\"url\":\"$BASE/credplus\",\"jsonPath\":\"data.total_credits\",\"minusJsonPath\":\"data.total_usage\",\"freeWhenEmpty\":true},\"runners\":[\"fixrunner\"],\"defaultRunner\":\"fixrunner\"}" >/dev/null
outbalzero="$("$WBS" models discover fixbalzero --json)"
outbalplus="$("$WBS" models discover fixbalplus --json)"
have "5 Guthaben minus 5 verbraucht = 0: nur kostenlose" "$outbalzero" '"mode": "free"'
eq "aufgebrauchtes Guthaben: genau 2 registriert" \
  "$("$WBS" models list --all | grep -c '^fixrunner-fixbalzero-')" "2"
have "10 Guthaben minus 2 verbraucht = 8: alle" "$outbalplus" '"mode": "all"'
eq "vorhandenes Guthaben: alle 5 registriert" \
  "$("$WBS" models list --all | grep -c '^fixrunner-fixbalplus-')" "5"
have "verrechnetes Guthaben steht am Provider" "$(provider_field fixbalplus balanceStatus)" '"amount": 8'
hasnt "auch hier kein Key in der Ausgabe" "$outbalnull$outbalzero$outbalplus" "$FAKEKEY"


echo "== 5. Netz weg: letzter Stand gilt, als alt gekennzeichnet =="
sleep 3    # dito: der letzte erfolgreiche Abruf von Abschnitt 3 muss "alt" genug sein
kill "$SRVPID" 2>/dev/null; wait "$SRVPID" 2>/dev/null; SRVPID=""
out5="$("$WBS" models discover fixcat --json)"
have "kein Fehler trotz totem Server (Cache greift)" "$out5" '"error": null'
have "als alt markiert" "$out5" '"stale": true'
eq "fixrunner-fixcat: weiterhin alle 5 (nichts entfernt)" "$("$WBS" models list --all | grep -c '^fixrunner-fixcat-')" "5"
have "kept statt neu/entfernt" "$out5" '"kept": 5'

echo "== 6. modelDiscoveryAuto=false: nur der Netzpfad steht still =="
"$WBS" settings set modelDiscoveryAuto false >/dev/null
outall="$("$WBS" models discover --all --json)"
have "--all ohne Netz: lokale Quelle laeuft normal" "$outall" '"fixlocal":'
hasnt "--all ohne Netz: kein Katalog-Anbieter taucht ungefragt auf" "$outall" '"fixcat"'
outexplicit="$("$WBS" models discover fixcat --json)"
have "explizit angefordert trotzdem: klare Meldung" "$outexplicit" "modelDiscoveryAuto ist aus"
have "klare Meldung nennt den Ausweg" "$outexplicit" "--force"

echo "== 7. --force uebergeht modelDiscoveryAuto=false =="
outforce="$("$WBS" models discover fixcat --force --json)"
hasnt "--force: keine 'ist aus'-Meldung mehr" "$outforce" "modelDiscoveryAuto ist aus"
have "--force: der Aufruf lief wirklich (Stand aus dem Cache, Server ist tot)" "$outforce" '"stale": true'

echo "== 8. --no-network uebergeht sogar --force =="
"$WBS" settings set modelDiscoveryAuto true >/dev/null
outnonet="$("$WBS" models discover fixcat --no-network --json)"
have "--no-network: eigene Meldung" "$outnonet" "no-network"
outnonetforce="$("$WBS" models discover fixcat --force --no-network --json)"
have "--no-network gewinnt auch gegen --force" "$outnonetforce" "no-network"
outallnonet="$("$WBS" models discover --all --no-network --json)"
have "--all --no-network: lokale Quelle bleibt an" "$outallnonet" '"fixlocal":'
hasnt "--all --no-network: kein Katalog-Anbieter in der Liste" "$outallnonet" '"fixcat"'

echo "== 9. der Key taucht in KEINER Datei und KEINEM Log auf =="
hasnt "nicht in models.json" "$(cat "$MODELSFILE" 2>/dev/null)" "$FAKEKEY"
hasnt "nicht im Registry-Aenderungslog" "$(cat "$CHANGELOG" 2>/dev/null)" "$FAKEKEY"
hasnt "nicht in der Discover-Zeitstempel-Datei" "$(cat "$DISCOVER_STATE" 2>/dev/null)" "$FAKEKEY"
hasnt "nicht im Katalog-Cache" "$(cat "$CATALOG_CACHE_DIR"/*.json 2>/dev/null)" "$FAKEKEY"
hasnt "nicht in 'models catalog'-Ausgabe" "$("$WBS" models catalog --json 2>&1)" "$FAKEKEY"
hasnt "nicht im Provider-Objekt der Registry" "$(provider_field fixcat catalogStatus 2>&1)$(provider_field fixcat balanceStatus 2>&1)" "$FAKEKEY"
hasnt "nicht in irgendeiner bisherigen Kommando-Ausgabe dieses Laufs" \
  "$out1$out1b$out2$out3$out3cat$out4$out5$outall$outexplicit$outforce$outnonet$outnonetforce$outallnonet" "$FAKEKEY"
eq "keine liegen gebliebene Header-Temp-Datei" \
  "$(find "$CATALOG_CACHE_DIR" -maxdepth 1 -type f ! -name '*.json' 2>/dev/null | wc -l | tr -d ' ')" "0"
have "GEGENPROBE: der Key kam beim Stub selbst wirklich an (unsere eigene Instrumentierung, kein wb-state-Artefakt)" \
  "$(cat "$REQLOG")" "$FAKEKEY"

echo
echo "PASS=$PASS FAIL=$FAIL"
[ "$FAIL" -eq 0 ]
