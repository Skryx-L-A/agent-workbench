#!/usr/bin/env bash
# setup-vscode-profile.sh — create the VSCode profile "Claude Workbench", apply
# the calm workbench settings (no welcome tabs, terminal panel at the bottom,
# telemetry off) and optionally install the workbench extension into it.
#
#   setup-vscode-profile.sh [path/to/claude-workbench-<version>.vsix]
#
# Idempotent: re-running only rewrites settings.json and reinstalls the .vsix.
# Requires the `code` CLI on PATH (VSCode command palette:
# "Shell Command: Install 'code' command in PATH").
set -euo pipefail

PROFILE="Claude Workbench"
HERE="$(cd "$(dirname "$0")" && pwd)"
VSIX="${1:-}"

command -v code >/dev/null || { echo "FEHLER: 'code' (VSCode-CLI) nicht im PATH." >&2; exit 1; }

# VSCode's user-data directory differs per OS. Under WSL, VSCode itself runs on
# the WINDOWS side (the `code` in WSL is only a wrapper), so its profile store
# lives under the Windows %APPDATA%.
case "$(uname -s)" in
  Darwin) CODE_USER="$HOME/Library/Application Support/Code/User" ;;
  Linux)
    if grep -qi microsoft /proc/version 2>/dev/null; then
      appdata="$(cmd.exe /c 'echo %APPDATA%' 2>/dev/null | tr -d '\r')" || appdata=""
      if [ -n "$appdata" ] && command -v wslpath >/dev/null 2>&1; then
        CODE_USER="$(wslpath "$appdata")/Code/User"
      else
        echo "FEHLER: WSL erkannt, aber der Windows-%APPDATA%-Pfad war nicht auflösbar." >&2
        echo "Profil bitte einmalig unter Windows anlegen (VSCode: Profile > New Profile > 'Claude Workbench')" >&2
        echo "und die Settings aus $HERE/profile-settings.json dort einfügen." >&2
        exit 1
      fi
    else
      CODE_USER="$HOME/.config/Code/User"
    fi
    ;;
  *) echo "FEHLER: nicht unterstütztes OS: $(uname -s)" >&2; exit 1 ;;
esac
STORAGE="$CODE_USER/globalStorage/storage.json"

profile_dir() {
  python3 - "$STORAGE" "$PROFILE" <<'PY'
import json, sys
try:
    st = json.load(open(sys.argv[1]))
except Exception:
    sys.exit(1)
for p in st.get("userDataProfiles", []):
    if p.get("name") == sys.argv[2]:
        print(p.get("location", ""))
        sys.exit(0)
sys.exit(1)
PY
}

if ! LOC="$(profile_dir 2>/dev/null)"; then
  echo "Profil '$PROFILE' anlegen (VSCode startet kurz) ..."
  # `code --profile <name>` creates the profile when it does not exist yet.
  code --profile "$PROFILE" --new-window >/dev/null 2>&1 &
  d=$(( $(date +%s) + 120 ))
  until LOC="$(profile_dir 2>/dev/null)"; do
    [ "$(date +%s)" -gt "$d" ] && { echo "FEHLER: Profil '$PROFILE' nach 120s nicht angelegt." >&2; exit 1; }
    sleep 3
  done
fi

PDIR="$CODE_USER/profiles/$LOC"
mkdir -p "$PDIR"
cp "$HERE/profile-settings.json" "$PDIR/settings.json"
echo "Profil '$PROFILE' konfiguriert: $PDIR/settings.json"

if [ -n "$VSIX" ]; then
  [ -f "$VSIX" ] || { echo "FEHLER: .vsix nicht gefunden: $VSIX" >&2; exit 1; }
  code --profile "$PROFILE" --install-extension "$VSIX" --force
  echo "Extension installiert: $(basename "$VSIX")"
fi
