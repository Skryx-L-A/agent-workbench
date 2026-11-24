#!/bin/sh
# Legt den Vorführ-Vault an einem Ort außerhalb des Home-Ordners an (Drehbuch E2, Entscheidung vom 20.11.2026).
# Aufruf: vorfuehr-vault-einrichten.sh <ziel>
# Exit 64 (EX_USAGE): Aufruffehler, relatives Ziel oder Ziel unter $HOME. Exit 73 (EX_CANTCREAT): das Ziel existiert schon.
# Das Ziel darf nicht existieren und nicht unter $HOME liegen. Ordner bekommen 0700, Dateien 0600, Eigentümer ist, wer
# das Skript ausführt. Den Einschluss baut der Starter (Vertrag 1); in ihn kommt der Vault nie.
set -eu
quelle=$(cd "$(dirname "$0")/vorfuehr-vault" && pwd -P)
[ "$#" -eq 1 ] || { echo "Aufruf: $0 <ziel>" >&2; exit 64; }
ziel=$1
case "$ziel" in
  /*) ;;
  *) echo "Ziel muss ein absoluter Pfad sein: $ziel" >&2; exit 64 ;;
esac
if [ -e "$ziel" ] || [ -L "$ziel" ]; then
  echo "Ziel existiert schon, nichts geändert: $ziel" >&2; exit 73
fi
eltern=$(cd "$(dirname "$ziel")" && pwd -P)
heim=$(cd "$HOME" && pwd -P)
case "$eltern/" in
  "$heim"/*) echo "Ziel liegt unter \$HOME, nichts geändert: $ziel" >&2; exit 64 ;;
esac
umask 077
mkdir -m 700 "$ziel"
(cd "$quelle" && find . -type d) | while IFS= read -r d; do mkdir -p -m 700 "$ziel/$d"; done
(cd "$quelle" && find . -type f -name '*.md') | while IFS= read -r f; do
  cp "$quelle/$f" "$ziel/$f"
  chmod 600 "$ziel/$f"
done
echo "Vorführ-Vault angelegt: $ziel ($(find "$ziel" -type f -name '*.md' | wc -l | tr -d ' ') Notizen)"
