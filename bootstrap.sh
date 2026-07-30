#!/usr/bin/env bash
# bootstrap.sh — install the Claude orchestrator setup on this machine.
#
#   ./bootstrap.sh [--with-local] [--bypass-permissions] [--dry-run] [--skip-deps]
#                  [--non-interactive]
#
# Works on macOS, Linux and WSL2. Idempotent: re-running updates files and skips
# what is already there. Existing files are backed up before they are replaced.
#
#   --with-local          also install the OPTIONAL local-model layer (Ollama/pi workers)
#   --bypass-permissions  write permissions.defaultMode=bypassPermissions into the new
#                         ~/.claude/settings.json (Claude runs shell commands and file
#                         edits WITHOUT asking). Otherwise: acceptEdits.
#                         Same effect: WB_BYPASS=j (or n).
#   --dry-run             print what would happen, change nothing
#   --skip-deps           do not install system packages (assume they exist)
#   --non-interactive     ask nothing, take defaults / WB_* environment variables
#
# It never contains or asks for a Claude login: you log in yourself with `claude`
# (see INSTALL.md, "Three ways to get Claude access").
set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
BUNDLE="$HERE/bundle"
DRY=0; WITH_LOCAL=0; SKIP_DEPS=0; INTERACTIVE=1
# Leer per Vorgabe: ein neuer Nutzer bekommt ein EIGENES, leeres Brain, kein
# fremdes. Wer schon eins hat, setzt WB_VAULT_REMOTE.
VAULT_REMOTE_DEFAULT=""
VSIX="$BUNDLE/workbench/claude-workbench-0.1.0.vsix"

for a in "$@"; do
  case "$a" in
    --with-local) WITH_LOCAL=1 ;;
    --bypass-permissions) WB_BYPASS=j ;;
    --dry-run) DRY=1 ;;
    --skip-deps) SKIP_DEPS=1 ;;
    --non-interactive) INTERACTIVE=0 ;;
    -h|--help) sed -n '2,20p' "$0"; exit 0 ;;
    *) echo "bootstrap: unbekannte Option '$a'" >&2; exit 1 ;;
  esac
done

say()  { printf '\n\033[1m== %s\033[0m\n' "$*"; }
info() { printf '   %s\n' "$*"; }
warn() { printf '   \033[33mWARNUNG: %s\033[0m\n' "$*" >&2; }
die()  { printf '\033[31mFEHLER: %s\033[0m\n' "$*" >&2; exit 1; }
run()  { if [ "$DRY" = 1 ]; then printf '   [dry-run] %s\n' "$*"; else "$@"; fi; }

# ---------------------------------------------------------------- OS detection
OS="$(uname -s)"
IS_WSL=0
case "$OS" in
  Darwin) OS_NAME="macOS"; PKG=brew ;;
  Linux)
    grep -qi microsoft /proc/version 2>/dev/null && { IS_WSL=1; OS_NAME="WSL2 (Linux)"; } || OS_NAME="Linux"
    if   command -v apt-get >/dev/null; then PKG=apt
    elif command -v dnf     >/dev/null; then PKG=dnf
    elif command -v pacman  >/dev/null; then PKG=pacman
    else PKG=none; fi
    ;;
  *) die "nicht unterstütztes OS: $OS (Windows: bootstrap.ps1 benutzen)" ;;
esac
if [ "$PKG" = brew ] && ! command -v brew >/dev/null; then PKG=none; fi

TIMEOUT_CMD=timeout
if [ "$OS" = Darwin ]; then TIMEOUT_CMD=gtimeout; fi

say "Umgebung: $OS_NAME, Paketmanager: $PKG"

# ------------------------------------------------------------------- questions
ask() { # ask <var> <prompt> <default>
  local var="$1" prompt="$2" def="$3" cur ans
  cur="$(eval "printf '%s' \"\${$var:-}\"")"          # WB_* env var wins
  if [ -n "$cur" ]; then eval "$var=\"\$cur\""; return; fi
  if [ "$INTERACTIVE" = 0 ]; then eval "$var=\"\$def\""; return; fi
  printf '   %s [%s]: ' "$prompt" "$def"
  IFS= read -r ans || ans=""
  [ -z "$ans" ] && ans="$def"
  eval "$var=\"\$ans\""
}

detect_ram() {
  if [ "$OS" = Darwin ]; then sysctl -n hw.memsize | awk '{printf "%d", $1/1024/1024/1024}'
  elif [ -r /proc/meminfo ]; then awk '/MemTotal/{printf "%d", $2/1024/1024}' /proc/meminfo
  else echo 16; fi
}
detect_machine() {
  if [ "$OS" = Darwin ]; then echo "Mac ($(uname -m)), macOS $(sw_vers -productVersion 2>/dev/null || echo '?')"
  else echo "$(uname -m), $(. /etc/os-release 2>/dev/null && echo "${PRETTY_NAME:-Linux}")"; fi
}

say "Angaben zur Person (füllen alle Platzhalter automatisch)"
WB_USER_NAME="${WB_USER_NAME:-}";     ask WB_USER_NAME   "Voller Name" "$(id -F 2>/dev/null || echo "$USER")"
WB_GITHUB="${WB_GITHUB:-}";           ask WB_GITHUB      "GitHub-Handle" "$USER"
WB_EMAIL="${WB_EMAIL:-}";             ask WB_EMAIL       "E-Mail (für Git-Commits)" "$(git config --global user.email 2>/dev/null || echo "")"
WB_PROJECTS="${WB_PROJECTS:-}";       ask WB_PROJECTS    "Projekte-Verzeichnis" "$HOME/AI"
WB_MACHINE="${WB_MACHINE:-}";         ask WB_MACHINE     "Maschine (kurz)" "$(detect_machine)"
WB_RAM="${WB_RAM:-}";                 ask WB_RAM         "RAM in GB" "$(detect_ram)"
WB_VAULT_REMOTE="${WB_VAULT_REMOTE:-}"; ask WB_VAULT_REMOTE "Vault-Remote (gemeinsames Brain)" "$VAULT_REMOTE_DEFAULT"
WB_WORKBENCH="${WB_WORKBENCH:-}";     ask WB_WORKBENCH   "VSCode-Workbench installieren? (j/n)" "j"

# Berechtigungen: bewusste, informierte Entscheidung der Person (siehe START.md/ONBOARD.md).
if [ -z "${WB_BYPASS:-}" ] && [ "$INTERACTIVE" = 1 ]; then
  say "Berechtigungen"
  info "bypassPermissions: Claude führt Datei-Änderungen UND Shell-Befehle ohne Rückfrage aus"
  info "  — auch 'rm', auch Netzwerkzugriffe. Es bleibt nur der Schutz des Betriebssystems"
  info "  (Dateirechte, sudo-Passwort). Das Orchestrator-Setup läuft damit flüssig; ohne"
  info "  diesen Modus blockiert es ständig mit Rückfragen."
  info "Alternative 'acceptEdits': Datei-Änderungen ohne Rückfrage, Shell-Befehle mit."
fi
WB_BYPASS="${WB_BYPASS:-}";           ask WB_BYPASS      "Claude ohne Rückfragen laufen lassen (bypassPermissions)? (j/n)" "j"
case "$WB_BYPASS" in j|J|y|Y) DEFAULT_MODE=bypassPermissions ;; *) DEFAULT_MODE=acceptEdits ;; esac

if [ "$WITH_LOCAL" = 0 ]; then
  WB_LOCAL="${WB_LOCAL:-}"
  ask WB_LOCAL "Lokale Modelle (Ollama/pi-Worker) einrichten? (j/n)" "n"
  case "$WB_LOCAL" in j|J|y|Y) WITH_LOCAL=1 ;; esac
fi
case "$WB_WORKBENCH" in j|J|y|Y) WITH_WB=1 ;; *) WITH_WB=0 ;; esac
[ -n "$WB_EMAIL" ] || warn "keine E-Mail angegeben — {{USER_EMAIL}} bleibt leer"

# ------------------------------------------------------------------- packages
pkg_install() { # pkg_install <package...>
  [ "$SKIP_DEPS" = 1 ] && { info "übersprungen (--skip-deps): $*"; return 0; }
  case "$PKG" in
    brew)   run brew install "$@" ;;
    apt)    run sudo apt-get update -qq && run sudo apt-get install -y "$@" ;;
    dnf)    run sudo dnf install -y "$@" ;;
    pacman) run sudo pacman -S --needed --noconfirm "$@" ;;
    none)   warn "kein Paketmanager gefunden — bitte manuell installieren: $*" ;;
  esac
}

say "Basis-Werkzeuge"
need=()
command -v git   >/dev/null || need+=(git)
command -v tmux  >/dev/null || need+=(tmux)
command -v rg    >/dev/null || need+=(ripgrep)
command -v jq    >/dev/null || need+=(jq)
command -v python3 >/dev/null || need+=(python3)
command -v "$TIMEOUT_CMD" >/dev/null || need+=(coreutils)
command -v node  >/dev/null || need+=(nodejs)
command -v git-lfs >/dev/null || need+=(git-lfs)
# Linux-only extras: a clipboard tool + xdg-open (macOS has pbcopy/open natively).
if [ "$OS" = Linux ]; then
  command -v xdg-open >/dev/null || need+=(xdg-utils)
  if ! command -v wl-copy >/dev/null && ! command -v xclip >/dev/null; then
    # Wayland-first (Nobara/GNOME default); xclip is the X11 fallback.
    if [ "${WAYLAND_DISPLAY:-}" != "" ]; then need+=(wl-clipboard); else need+=(xclip); fi
  fi
fi
if [ ${#need[@]} -gt 0 ]; then
  info "installiere: ${need[*]}"
  pkg_install "${need[@]}"
else
  info "alles vorhanden (git, tmux, rg, jq, python3, $TIMEOUT_CMD)"
fi

say "Claude Code CLI"
if command -v claude >/dev/null; then
  info "vorhanden: $(claude --version 2>/dev/null || echo 'claude')"
else
  info "Installation über den offiziellen Installer (https://claude.ai/install.sh)"
  if [ "$DRY" = 1 ]; then
    info "[dry-run] curl -fsSL https://claude.ai/install.sh | bash"
  else
    curl -fsSL https://claude.ai/install.sh | bash
  fi
  info "Alternative (falls Node 22+ vorhanden): npm install -g @anthropic-ai/claude-code"
fi

# ------------------------------------------------------------------- VSCode
say "VSCode + Claude-Code-Extension"
if ! command -v code >/dev/null; then
  case "$PKG" in
    brew) pkg_install --cask visual-studio-code ;;
    *)    warn "VSCode nicht gefunden. Installieren (Linux): https://code.visualstudio.com/docs/setup/linux — danach 'code' in den PATH legen und bootstrap.sh erneut laufen lassen." ;;
  esac
fi
if command -v code >/dev/null; then
  # verifizierte Marketplace-ID (Anthropic, 'Claude Code for VS Code')
  run code --install-extension anthropic.claude-code --force
else
  warn "Extension nicht installiert (kein 'code' im PATH)."
fi

# ------------------------------------------------------------------- ~/.claude
say "~/.claude"
backup() { # backup <file>  -> snapshot before overwriting
  [ -e "$1" ] || return 0
  local snap="$HOME/.local/trash-snapshots/$(date +%Y%m%d)-bootstrap"
  run mkdir -p "$snap"
  run cp -R "$1" "$snap/$(basename "$1").$(date +%H%M%S)"
}

run mkdir -p "$HOME/.claude" "$HOME/.local/bin"
for d in skills roles plugins hooks commands agents; do
  [ -d "$BUNDLE/dot-claude/$d" ] || continue
  run rm -rf "$HOME/.claude/$d.new"
  run cp -R "$BUNDLE/dot-claude/$d" "$HOME/.claude/$d.new"
  backup "$HOME/.claude/$d"
  run rm -rf "$HOME/.claude/$d"
  run mv "$HOME/.claude/$d.new" "$HOME/.claude/$d"
done
if [ -d "$HOME/.claude/hooks" ]; then
  run find "$HOME/.claude/hooks" -type f \( -name '*.sh' -o -name '*.py' \) -exec chmod +x {} +
fi
run cp "$BUNDLE/dot-claude/statusline-command.sh" "$HOME/.claude/statusline-command.sh"
run chmod +x "$HOME/.claude/statusline-command.sh"

if [ -e "$HOME/.claude/settings.json" ]; then
  backup "$HOME/.claude/settings.json"
  warn "~/.claude/settings.json existiert — NICHT überschrieben. Neue Vorlage: $BUNDLE/dot-claude/settings.json (mergen)"
  warn "gewünschter Modus '$DEFAULT_MODE' NICHT gesetzt. Von Hand: jq '.permissions.defaultMode = \"$DEFAULT_MODE\"' ~/.claude/settings.json > ~/.claude/settings.json.new && mv ~/.claude/settings.json.new ~/.claude/settings.json"
elif [ "$DRY" = 1 ]; then
  info "[dry-run] settings.json anlegen mit permissions.defaultMode=$DEFAULT_MODE"
else
  # defaultMode ist die informierte Entscheidung der Person (Frage oben / WB_BYPASS).
  DEFAULT_MODE="$DEFAULT_MODE" python3 - "$BUNDLE/dot-claude/settings.json" "$HOME/.claude/settings.json" <<'PY'
import json, os, sys
src, dst = sys.argv[1], sys.argv[2]
s = json.load(open(src, encoding="utf-8"))
s.setdefault("permissions", {})["defaultMode"] = os.environ["DEFAULT_MODE"]
with open(dst, "w", encoding="utf-8") as f:
    json.dump(s, f, indent=2, ensure_ascii=False)
    f.write("\n")
PY
  info "settings.json angelegt: permissions.defaultMode=$DEFAULT_MODE"
fi

say "Orchestrierungs-Werkzeuge nach ~/.local/bin"
# Alle gebündelten Tools installieren (Orchestrierung, Vault, Worker-Spawner,
# Kontext-Guard, framer-inspo, brain/bm, ai-scout …). Idempotent; überschreibt.
for f in "$BUNDLE/bin/"*; do
  [ -f "$f" ] || continue
  run cp "$f" "$HOME/.local/bin/$(basename "$f")"
  run chmod +x "$HOME/.local/bin/$(basename "$f")"
done
case ":$PATH:" in
  *":$HOME/.local/bin:"*) : ;;
  *) warn "~/.local/bin ist nicht im PATH — in ~/.zshrc bzw. ~/.bashrc ergänzen: export PATH=\"\$HOME/.local/bin:\$PATH\"" ;;
esac

say "Modell-Registry (~/.claude/workbench/models.json)"
run mkdir -p "$HOME/.claude/workbench"
if [ -e "$HOME/.claude/workbench/models.json" ]; then
  info "models.json existiert — unangetastet gelassen (Vorlage: $BUNDLE/workbench/models.default.json)"
else
  run cp "$BUNDLE/workbench/models.default.json" "$HOME/.claude/workbench/models.json"
  info "Registry angelegt. Danach: wb-state models discover --all  (findet, was installiert ist)"
fi

say "tmux-Konfiguration"
if [ -e "$HOME/.tmux.conf" ]; then
  backup "$HOME/.tmux.conf"
  warn "~/.tmux.conf existiert — NICHT ueberschrieben. Vorlage: $BUNDLE/dot-tmux.conf"
  warn "Ohne sie fehlen die Workbench-Tastenkuerzel (u.a. prefix+R zum Wiederbeleben toter Panes)."
else
  run cp "$BUNDLE/dot-tmux.conf" "$HOME/.tmux.conf"
  info "~/.tmux.conf angelegt"
fi

# ------------------------------------------------------------------- vault
say "Knowledge-Vault (~/Knowledge)"
VAULT_SKELETON=0
if [ -d "$HOME/Knowledge/.git" ]; then
  info "existiert bereits — unangetastet gelassen"
elif [ -d "$HOME/Knowledge" ]; then
  warn "~/Knowledge existiert ohne git — unangetastet gelassen"
  VAULT_SKELETON=1
else
  ssh_ok=0
  if [ "$DRY" = 0 ]; then
    ssh -o BatchMode=yes -o StrictHostKeyChecking=accept-new -T git@github.com 2>&1 | grep -q 'successfully authenticated' && ssh_ok=1
  fi
  if [ "$ssh_ok" = 1 ] && [ -n "$WB_VAULT_REMOTE" ]; then
    info "klone das gemeinsame Brain: $WB_VAULT_REMOTE"
    if run git clone "$WB_VAULT_REMOTE" "$HOME/Knowledge"; then :; else
      warn "Klonen fehlgeschlagen (kein Collaborator-Zugriff?) — lege Skelett an"
      VAULT_SKELETON=1
    fi
  else
    warn "kein GitHub-SSH-Zugriff — lege das Vault-Skelett an."
    info "Sobald du als Collaborator im Vault-Repo freigeschaltet bist:"
    info "  mv ~/Knowledge ~/Knowledge.skeleton && git clone $WB_VAULT_REMOTE ~/Knowledge"
    VAULT_SKELETON=1
  fi
  if [ "$VAULT_SKELETON" = 1 ] && [ ! -d "$HOME/Knowledge" ]; then run cp -R "$BUNDLE/knowledge" "$HOME/Knowledge"; fi
fi
# 90-secrets bleibt IMMER lokal und wird nie aus dem Repo befüllt.
run mkdir -p "$HOME/Knowledge/90-secrets"
[ -e "$HOME/Knowledge/90-secrets/.gitignore" ] || run cp "$BUNDLE/knowledge/90-secrets/.gitignore" "$HOME/Knowledge/90-secrets/.gitignore"
if [ -f "$HOME/Knowledge/tools/session-context.sh" ]; then run chmod +x "$HOME/Knowledge/tools/session-context.sh"; fi

# IDENTITY.md: pro Maschine, gitignored, NIE committed. Sagt dem geteilten Brain, wer hier
# am Rechner sitzt. Vorlage: IDENTITY.md.example. Idempotent: nie überschreiben.
if [ -e "$HOME/Knowledge/IDENTITY.md" ]; then
  info "IDENTITY.md existiert — unangetastet gelassen"
elif [ "$DRY" = 1 ]; then
  info "[dry-run] ~/Knowledge/IDENTITY.md aus IDENTITY.md.example anlegen"
else
  cat > "$HOME/Knowledge/IDENTITY.md" <<EOF
---
title: IDENTITY
type: reference
permalink: main/identity
---

Lokale Identität dieser Maschine — gitignored, wird NIE committed (Vorlage:
\`IDENTITY.md.example\`). Regeln des geteilten Brains: \`10-global/shared-brain.md\`.

- User: $WB_USER_NAME, GitHub $WB_GITHUB, $WB_EMAIL.
- Maschine: $WB_MACHINE, User \`$(id -un)\`. Projekte in \`$WB_PROJECTS\`.
EOF
  info "IDENTITY.md angelegt (gitignored — nicht committen)"
fi
if [ -f "$HOME/Knowledge/.gitignore" ] && ! grep -qx 'IDENTITY.md' "$HOME/Knowledge/.gitignore" 2>/dev/null; then
  warn "IDENTITY.md steht NICHT in ~/Knowledge/.gitignore — bitte ergänzen, bevor du committest."
fi

# ------------------------------------------------------------------- optional local
if [ "$WITH_LOCAL" = 1 ]; then
  say "Lokale Modelle (optional)"
  run mkdir -p "$HOME/.pi"
  run cp -R "$BUNDLE/dot-pi/agent" "$HOME/.pi/"
  command -v ollama >/dev/null || pkg_install ollama
  info "Modelle noch selbst ziehen (ollama pull …) und die Aliase in ~/.local/bin/pi-worker anpassen."
fi

# ------------------------------------------------------ optional media layer (Linux/CUDA)
# The media stack (bild/video/tts/stt + local image/video/speech models) is a SEPARATE
# workstream and ships its own installer. Call it ONLY if it is present — this keeps the
# two workstreams decoupled: a bundle without linux/media-install.sh installs fine.
if [ -f "$HERE/linux/media-install.sh" ]; then
  say "Lokale Medien-Modelle (bild/video/tts/stt)"
  if [ "$OS" = Linux ]; then
    run bash "$HERE/linux/media-install.sh"
  else
    info "linux/media-install.sh ist für Linux/CUDA — auf $OS_NAME übersprungen."
  fi
fi

# ------------------------------------------------------------------- placeholders
say "Platzhalter füllen"
fill() { # fill <file>...
  [ "$DRY" = 1 ] && { info "[dry-run] Platzhalter in: $*"; return 0; }
  WB_USER_NAME="$WB_USER_NAME" WB_GITHUB="$WB_GITHUB" WB_EMAIL="$WB_EMAIL" \
  WB_PROJECTS="$WB_PROJECTS" WB_MACHINE="$WB_MACHINE" WB_RAM="$WB_RAM" \
  WB_VAULT_REMOTE="$WB_VAULT_REMOTE" OS_NAME="$OS_NAME" TIMEOUT_CMD="$TIMEOUT_CMD" \
  OS_USERNAME="$(id -un)" WITH_WB="$WITH_WB" WITH_LOCAL="$WITH_LOCAL" \
  python3 - "$@" <<'PY'
import os, re, sys
V = {
    "USER_NAME": os.environ["WB_USER_NAME"],
    "GITHUB_HANDLE": os.environ["WB_GITHUB"],
    "USER_EMAIL": os.environ["WB_EMAIL"],
    "OS_USERNAME": os.environ["OS_USERNAME"],
    "MACHINE_DESCRIPTION": os.environ["WB_MACHINE"],
    "PROJECTS_DIR": os.environ["WB_PROJECTS"],
    "RAM_GB": os.environ["WB_RAM"],
    "VAULT_REMOTE": os.environ["WB_VAULT_REMOTE"] or "kein Remote",
    "OS_NAME": os.environ["OS_NAME"],
    "TIMEOUT_CMD": os.environ["TIMEOUT_CMD"],
    "PROJECT_LIST": "(trage deine Projekte ein, sobald es welche gibt)",
    "PROJECT_KIT_REPO": "privates Repo — Zugriff erfragen oder überspringen",
}
keep = {"WORKBENCH": os.environ["WITH_WB"] == "1", "LOCAL": os.environ["WITH_LOCAL"] == "1"}

def block(text):
    for name, on in keep.items():
        pat = re.compile(r"<!-- BLOCK:%s -->\n(.*?)<!-- /BLOCK:%s -->\n" % (name, name), re.S)
        text = pat.sub((lambda m: m.group(1)) if on else "", text)
    return text

for path in sys.argv[1:]:
    if not os.path.isfile(path):
        continue
    t = open(path, encoding="utf-8").read()
    t = block(t)
    for k, v in V.items():
        t = t.replace("{{%s}}" % k, v)
    open(path, "w", encoding="utf-8").write(t)
PY
}

if [ -e "$HOME/.claude/CLAUDE.md" ] && [ "$DRY" = 0 ]; then
  backup "$HOME/.claude/CLAUDE.md"
fi
run cp "$BUNDLE/dot-claude/CLAUDE.md.template" "$HOME/.claude/CLAUDE.md"
TARGETS=("$HOME/.claude/CLAUDE.md" "$HOME/.claude/roles/orchestrator.md" "$HOME/.claude/roles/agent.md" "$HOME/.claude/plugins/PLUGINS.md")
if [ -f "$HOME/.pi/agent/WORKER.md" ]; then TARGETS+=("$HOME/.pi/agent/WORKER.md"); fi
if [ "$VAULT_SKELETON" = 1 ]; then
  # Nur das SKELETT wird angefasst — ein geklonter echter Vault nie.
  TARGETS+=("$HOME/Knowledge/INDEX.md" "$HOME/Knowledge/CRITICAL-FACTS.md" \
            "$HOME/Knowledge/tools/gardener/SCHEDULING.md" "$HOME/Knowledge/tools/gardener/gardener/config.py")
fi
fill "${TARGETS[@]}"
info "gefüllt: ${#TARGETS[@]} Dateien"

# ------------------------------------------------------------------- workbench
if [ "$WITH_WB" = 1 ]; then
  say "VSCode-Workbench (Profil + Extension)"
  if command -v code >/dev/null; then
    run bash "$BUNDLE/workbench/setup-vscode-profile.sh" "$VSIX"
    if [ "$OS" = Darwin ]; then
      run bash "$BUNDLE/workbench/make-app.sh"
    else
      info "Dock-App übersprungen (macOS-only). Start: code --profile \"Claude Workbench\" --new-window"
    fi
  else
    warn "Workbench übersprungen — 'code' fehlt im PATH."
  fi
fi

# --------------------------------------------------- Anweisungen je Harness
# Jeder Harness liest seine Regeln aus einer ANDEREN Datei (~/.codex/AGENTS.md,
# ~/.config/opencode/AGENTS.md, ~/.antigravity/AGENTS.md ...). wb-instructions
# erzeugt sie alle aus derselben Quelle, damit ein Worker unter codex nicht nach
# anderen Regeln arbeitet als einer unter Claude Code.
if [ -x "$HOME/.local/bin/wb-instructions" ]; then
  say "Anweisungsdateien der uebrigen Harnesses"
  if [ "$DRY" = 1 ]; then
    info "[dry-run] wb-instructions sync"
  else
    "$HOME/.local/bin/wb-instructions" sync 2>&1 | tail -3 || \
      warn "wb-instructions sync meldete einen Fehler — Regeln liegen trotzdem in ~/.claude/CLAUDE.md"
  fi
fi

# ------------------------------------------------------------------- done
say "Fertig"
cat <<EOF
   Berechtigungen: permissions.defaultMode=$DEFAULT_MODE (greift beim nächsten Start von Claude Code)

   Nächste Schritte:
   1) claude            -> im Browser einloggen (eigenes Abo, Team-Seat oder Console/API — INSTALL.md)
   2) ./verify.sh       -> Selbsttest (PASS/FAIL-Liste)
   3) Plugins:  in Claude Code '/plugin' -> siehe ~/.claude/plugins/PLUGINS.md
   4) Workbench starten: $( [ "$OS" = Darwin ] && echo '"Claude Workbench" im Dock' || echo 'code --profile "Claude Workbench" --new-window' )
EOF
