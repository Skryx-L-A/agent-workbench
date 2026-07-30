#!/usr/bin/env bash
# make-app.sh — (re)build ~/Applications/Claude Workbench.app.  macOS ONLY.
# The bundle just launches VSCode with the "Claude Workbench" profile, so the
# workbench gets its own Dock icon. On Linux/WSL there is no equivalent step:
# start the workbench with `code --profile "Claude Workbench" --new-window`
# (or create a .desktop entry that runs exactly that).
set -euo pipefail

[ "$(uname -s)" = "Darwin" ] || { echo "make-app.sh ist macOS-only (übersprungen)."; exit 0; }

HERE="$(cd "$(dirname "$0")" && pwd)"
APP="$HOME/Applications/Claude Workbench.app"

rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"

cat > "$APP/Contents/MacOS/ClaudeWorkbench" <<'EOF'
#!/bin/bash
# Launch VSCode with the dedicated "Claude Workbench" profile in a new window.
exec open -na "Visual Studio Code" --args --profile "Claude Workbench" --new-window --disable-workspace-trust
EOF
chmod +x "$APP/Contents/MacOS/ClaudeWorkbench"

cat > "$APP/Contents/Info.plist" <<'EOF'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>CFBundleName</key><string>Claude Workbench</string>
	<key>CFBundleDisplayName</key><string>Claude Workbench</string>
	<key>CFBundleIdentifier</key><string>local.claude-workbench</string>
	<key>CFBundleVersion</key><string>1.0</string>
	<key>CFBundleShortVersionString</key><string>1.0</string>
	<key>CFBundleExecutable</key><string>ClaudeWorkbench</string>
	<key>CFBundleIconFile</key><string>AppIcon</string>
	<key>CFBundlePackageType</key><string>APPL</string>
	<key>LSMinimumSystemVersion</key><string>13.0</string>
	<key>NSHighResolutionCapable</key><true/>
	<key>LSUIElement</key><false/>
</dict>
</plist>
EOF
printf 'APPL????' > "$APP/Contents/PkgInfo"
cp "$HERE/assets/AppIcon.icns" "$APP/Contents/Resources/AppIcon.icns"
touch "$APP"
echo "Gebaut: $APP"
