#!/usr/bin/env bash
# Test und Gegenprobe des Katalogwerkzeugs (Vertrag Mutationskatalog, Abschnitt A5 und A7).
#
#   bash rust/spec/mutationskatalog/gegenprobe.sh
#
# Vier Laeufe: der echte Katalog muss 0 ergeben, jede Gegenprobe 3. Das Skript liest nur
# und schreibt nichts. Exit 0: alle vier Codes wie erwartet; Exit 1: ein Code weicht ab.
set -u

HIER="$(cd "$(dirname "$0")" && pwd)"
WURZEL="$(cd "$HIER/../../.." && pwd)"
PY=/usr/bin/python3
PRUEFEN="$HIER/katalog-pruefen.py"

lauf() { # <erwarteter code> <katalog> <was>
  local soll="$1" katalog="$2" was="$3" rc=0
  "$PY" -I "$PRUEFEN" "$katalog" --wurzel "$WURZEL" >/dev/null || rc=$?
  if [ "$rc" = "$soll" ]; then
    echo "ok   rc=$rc (erwartet $soll)  $was"
    return 0
  fi
  echo "FUND rc=$rc (erwartet $soll)  $was"
  return 1
}

fehler=0
lauf 0 "$HIER/katalog.json" "der Katalog: jeder Anker trifft genau einmal" || fehler=1
lauf 3 "$HIER/gegenproben/anker-doppelt.json" "Gegenprobe: ein Anker steht mehrfach" || fehler=1
lauf 3 "$HIER/gegenproben/anker-fehlt.json" "Gegenprobe: ein Mutant ohne Ziel" || fehler=1
lauf 3 "$HIER/gegenproben/anker-verschoben.json" "Gegenprobe: ein verschobener Mutant" || fehler=1

if [ "$fehler" = 0 ]; then
  echo "gegenprobe.sh: vier Laeufe, jeder Code wie erwartet"
else
  echo "gegenprobe.sh: mindestens ein Code weicht ab" >&2
fi
exit "$fehler"
