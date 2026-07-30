#!/bin/bash
# install.sh — package the extension and install it into the "Claude Workbench" profile.
set -euo pipefail
cd "$(dirname "$0")/.."

PROFILE="Claude Workbench"
VERSION=$(node -p "require('./package.json').version")
VSIX="claude-workbench-${VERSION}.vsix"

npm run package
command -v code >/dev/null || { echo "VSCode-CLI 'code' nicht im PATH."; exit 1; }
# Design C: die Extension ist extensionKind "ui" und laeuft immer im LOKALEN
# Host — auch in Remote-SSH-Fenstern. Sie wird daher NUR lokal ins Profil
# installiert (kein Remote-Install mehr). Remote-SSH selbst braucht der Nutzer
# weiterhin, um peer-Fenster zu oeffnen, also mitinstallieren.
code --profile "$PROFILE" --install-extension ms-vscode-remote.remote-ssh --force
code --profile "$PROFILE" --install-extension "$VSIX" --force
echo "Installiert: $VSIX (+ Remote-SSH) im Profil \"$PROFILE\"."
