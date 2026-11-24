#!/usr/bin/env bash
# gleichheit.sh: baut die Ansicht frisch in einen Wegwerfordner (bauen.sh --aus) und vergleicht jede Datei Byte fuer Byte
# mit der eingecheckten Ausgabe in rust/ansicht/ausgabe/. Prueft dazu die feste Form: genau index.html, ansicht.js und
# ansicht_bg.wasm, flach, nur relative Verweise.
# Exit 0 gleich; 1 verschieden oder Form verletzt; sonst der Exit von bauen.sh.
set -u
HIER="$(cd "$(dirname "$0")/.." && pwd)" || exit 64
EIN="$HIER/ausgabe"
NEU="$(mktemp -d "${TMPDIR:-/tmp}/ansicht-gleich-XXXXXX")" || exit 1
trap 'rm -rf "$NEU"' EXIT
rc=0

form() {
  local ordner="$1" liste
  liste="$(cd "$ordner" && find . -mindepth 1 | sort | tr '\n' ' ')"
  if [ "$liste" != "./ansicht.js ./ansicht_bg.wasm ./index.html " ]; then
    echo "ROT Form $ordner: $liste" >&2
    return 1
  fi
  if ! grep -q '<script type="module" src="ansicht.js"></script>' "$ordner/index.html"; then
    echo "ROT index.html laedt ansicht.js nicht relativ" >&2
    return 1
  fi
  if ! grep -q "new URL('ansicht_bg.wasm', import.meta.url)" "$ordner/ansicht.js"; then
    echo "ROT ansicht.js laedt ansicht_bg.wasm nicht relativ" >&2
    return 1
  fi
  if grep -q -e '://' -e 'snippets/' "$ordner/index.html" "$ordner/ansicht.js"; then
    echo "ROT eine Datei nennt eine Adresse oder einen Unterordner" >&2
    return 1
  fi
}

form "$EIN" || rc=1
bash "$HIER/bauen.sh" --aus "$NEU" > "$NEU.log" 2>&1
st=$?
if [ $st -ne 0 ]; then
  cat "$NEU.log" >&2
  rm -f "$NEU.log"
  echo "ROT bauen.sh: exit $st" >&2
  exit $st
fi
rm -f "$NEU.log"
form "$NEU" || rc=1
for f in index.html ansicht.js ansicht_bg.wasm; do
  if cmp -s "$EIN/$f" "$NEU/$f"; then
    echo "gleich $f $(sha256sum "$NEU/$f" | cut -c1-64)"
  else
    echo "ROT $f: eingecheckt und frisch gebaut verschieden" >&2
    rc=1
  fi
done
[ $rc -eq 0 ] && echo "GRUEN: eingecheckte Ausgabe gleich dem frischen Bau"
exit $rc
