#!/bin/bash
# install.sh — package the extension and install it into the "Claude Workbench" profile.
set -euo pipefail
cd "$(dirname "$0")/.."

PROFILE="Claude Workbench"
VERSION=$(node -p "require('./package.json').version")
VSIX="claude-workbench-${VERSION}.vsix"

npm run package
command -v code >/dev/null || { echo "VSCode-CLI 'code' nicht im PATH."; exit 1; }
# Remote-SSH ist extensionDependency (Peer-Rechner-Sessions) — .vsix-Installs ziehen
# Abhaengigkeiten nicht automatisch, daher explizit zuerst installieren.
code --profile "$PROFILE" --install-extension ms-vscode-remote.remote-ssh --force
code --profile "$PROFILE" --install-extension "$VSIX" --force
echo "Installiert: $VSIX (+ Remote-SSH) im Profil \"$PROFILE\"."
