#!/usr/bin/env bash
# make-launcher-linux.sh — create a desktop launcher for the "Claude Workbench"
# VSCode profile. Linux ONLY (the macOS counterpart is make-app.sh, which builds
# a Dock .app). This writes a freedesktop .desktop entry that opens VSCode with
# the dedicated profile in a new window — tested target: Nobara/GNOME.
#
# Idempotent: re-running overwrites the entry. Needs `code` on PATH.
set -euo pipefail

[ "$(uname -s)" = "Linux" ] || { echo "make-launcher-linux.sh ist Linux-only (übersprungen)."; exit 0; }

CODE_BIN="$(command -v code || true)"
[ -n "$CODE_BIN" ] || { echo "FEHLER: 'code' nicht im PATH — VSCode installieren oder 'code' verlinken." >&2; exit 1; }

APPS="$HOME/.local/share/applications"
DESKTOP="$APPS/claude-workbench.desktop"
mkdir -p "$APPS"

# Optional icon: reuse the bundle's .icns only if a PNG sibling exists; otherwise
# fall back to the generic VSCode icon name.
ICON="code"

cat > "$DESKTOP" <<EOF
[Desktop Entry]
Type=Application
Name=Claude Workbench
Comment=VSCode with the dedicated "Claude Workbench" profile
Exec=$CODE_BIN --profile "Claude Workbench" --new-window
Icon=$ICON
Terminal=false
Categories=Development;IDE;
StartupNotify=true
EOF
chmod +x "$DESKTOP"

# Refresh the desktop database if the tool is present (harmless otherwise).
command -v update-desktop-database >/dev/null 2>&1 && update-desktop-database "$APPS" 2>/dev/null || true

echo "Launcher geschrieben: $DESKTOP"
echo "Start: über das App-Menü 'Claude Workbench' oder direkt:"
echo "  code --profile \"Claude Workbench\" --new-window"
