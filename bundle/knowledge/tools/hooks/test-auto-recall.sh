#!/bin/sh
# Regressionstest fuer den Recall-Hook. Ohne Vault, ohne Modell: er speist
# vorgefertigtes JSON in den Formatierer und prueft, was herauskommt.
#
# Anlass: der Python-Teil stand in `python3 -c "..."` in DOPPELTEN
# Anfuehrungszeichen, die Shell ersetzte darin Backticks - ein Kommentar mit dem
# Wort cosine in Backticks liess sie den Befehl `cosine` suchen. Die Ausgabe
# stimmte trotzdem, der Fehler ging nach stderr und fiel wochenlang nicht auf.
# Deshalb prueft dieser Test AUCH stderr, nicht nur das Ergebnis.
set -e
DIR="$(cd "$(dirname "$0")" && pwd)"
FMT="$DIR/auto-recall-format.py"
FAIL=0

run() {   # run <json> ; setzt OUT und ERR
    OUT="$(printf '%s' "$1" | WB_THRESHOLD=0.40 WB_RELATIVE_MARGIN=0.08 \
        WB_MAX_HITS=3 WB_SNIPPET_CHARS=150 python3 "$FMT" 2>/tmp/wb-recall-err)"
    ERR="$(cat /tmp/wb-recall-err)"
}

check() {  # check <name> <bedingung-erfuellt?>
    if [ "$2" = "ok" ]; then printf '  PASS  %s\n' "$1"
    else printf '  FAIL  %s\n' "$1"; FAIL=1; fi
}

HIT='{"hits":[{"rel":"10-global/a.md","title":"Notiz A","cosine":0.71,"snippet":"Inhalt A"},
              {"rel":"10-global/b.md","title":"Notiz B","cosine":0.69,"snippet":"Inhalt B"},
              {"rel":"10-global/c.md","title":"Notiz C","cosine":0.41,"snippet":"weit weg"}]}'

run "$HIT"
check "Treffer werden ausgegeben" "$(echo "$OUT" | grep -q 'Notiz A' && echo ok)"
check "kein Fehler auf stderr"    "$([ -z "$ERR" ] && echo ok)"
check "relativer Filter greift"   "$(echo "$OUT" | grep -qv 'Notiz C' && \
                                     ! echo "$OUT" | grep -q 'Notiz C' && echo ok)"
check "naher zweiter bleibt"      "$(echo "$OUT" | grep -q 'Notiz B' && echo ok)"
check "Naehe steht dabei"         "$(echo "$OUT" | grep -q '0.71' && echo ok)"

run '{"hits":[{"rel":"x.md","title":"Schwach","cosine":0.12,"snippet":"nix"}]}'
check "unter der Schwelle: nichts" "$([ -z "$OUT" ] && echo ok)"

run '{"hits":[]}'
check "keine Treffer: nichts"      "$([ -z "$OUT" ] && echo ok)"

run 'kein json'
check "kaputte Eingabe: still"     "$([ -z "$OUT" ] && echo ok)"

# Der ganze Hook, inklusive Shell-Teil: darf bei leerer Eingabe nicht meckern.
ERR="$(printf '' | sh "$DIR/auto-recall.sh" 2>&1 >/dev/null)"
check "Hook ohne Eingabe: stumm"   "$([ -z "$ERR" ] && echo ok)"

rm -f /tmp/wb-recall-err
[ "$FAIL" = 0 ] && echo "alle Pruefungen bestanden" || echo "FEHLGESCHLAGEN"
exit "$FAIL"
