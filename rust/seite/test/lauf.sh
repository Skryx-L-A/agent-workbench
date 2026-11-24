#!/bin/sh
# Startet alle Tests der Seite mit node (keine npm-Pakete). Exit 0 nur wenn jede Pruefung gruen ist, sonst 1.
#
#   sh test/lauf.sh            die Tests gegen die mitgelieferte Referenzdatei test/daten/gleichheit.json.gz und den Buendler
#   RA_VERTRAG=<Ordner> sh test/lauf.sh --neu
#                              erzeugt die Referenzdatei zusaetzlich frisch aus der Referenz (python3, RA_REF oder
#                              $RA_VERTRAG/referenz/ra_ref.py), in einem Ordner ausserhalb des Repos, und prueft den Dekoder auch gegen
#                              diese. Ist im Vertragsordner ein Satz `verdeckt/` da, gehoert er dazu (RA_SAETZE, nur dort, nie im Repo).
here=$(cd "$(dirname "$0")" && pwd) || exit 2
cd "$here" || exit 2
command -v node >/dev/null 2>&1 || { echo "node fehlt" >&2; exit 2; }
rc=0
for t in html.mjs inflate.mjs gleichheit.mjs buendel.mjs start.mjs; do
  if node "$t"; then :; else echo "ROT $t"; rc=1; fi
done
if [ "$1" = "--neu" ]; then
  [ -n "$RA_VERTRAG" ] || { echo "RA_VERTRAG fehlt (Ordner mit aussentests/)" >&2; exit 2; }
  ref=${RA_REF:-$RA_VERTRAG/referenz/ra_ref.py}
  tmp=$(mktemp -d "${TMPDIR:-/tmp}/seite-test.XXXXXX") || exit 2
  saetze="aussentests"
  [ -d "$RA_VERTRAG/verdeckt/faelle" ] && saetze="aussentests verdeckt"
  case "$tmp" in "$here"/*) echo "Ausgabeordner liegt im Repo" >&2; exit 2;; esac
  if RA_SAETZE="$saetze" RA_REF="$ref" RA_VERTRAG="$RA_VERTRAG" python3 -I -B referenz_erzeugen.py "$tmp/neu.json.gz" && node gleichheit.mjs "$tmp/neu.json.gz"; then :; else echo "ROT gleichheit gegen frisch erzeugte Referenz"; rc=1; fi
  rm -rf "$tmp"
fi
if [ $rc -eq 0 ]; then echo "ALLES GRUEN"; else echo "ROT"; fi
exit $rc
