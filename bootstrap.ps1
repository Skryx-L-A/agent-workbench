# bootstrap.ps1 — Windows entry point for the Claude orchestrator setup.
#
#   powershell -ExecutionPolicy Bypass -File .\bootstrap.ps1 [-Distro Ubuntu] [-WithLocal]
#
# What it does:
#   1. checks/installs WSL2 + a Linux distro (the setup needs tmux)
#   2. installs VS Code on the WINDOWS side + the WSL and Claude Code extensions
#   3. copies this repo into the distro and runs bootstrap.sh THERE
#
# Without WSL2 you can still use Claude Code natively on Windows, but the
# worker-pane workflow (tmux) does NOT run — see INSTALL.md, "Reduced mode".
#
# STATUS: UNTESTED — written against the official docs, never executed on a
# Windows machine. Read it before you run it; report what breaks.
[CmdletBinding()]
param(
  [string]$Distro = "Ubuntu",
  [switch]$WithLocal,
  [switch]$SkipVSCode
)
$ErrorActionPreference = "Stop"

function Info($m) { Write-Host "   $m" }
function Step($m) { Write-Host "`n== $m" -ForegroundColor Cyan }
function Warn($m) { Write-Host "   WARNUNG: $m" -ForegroundColor Yellow }

Step "WSL2"
$wslOk = $false
try {
  wsl.exe --status | Out-Null
  $wslOk = ($LASTEXITCODE -eq 0)
} catch { $wslOk = $false }

if (-not $wslOk) {
  Warn "WSL ist nicht eingerichtet. Installiere WSL2 + $Distro (Neustart nötig)."
  wsl.exe --install -d $Distro
  Write-Host ""
  Write-Host "Windows jetzt NEU STARTEN, danach dieses Script erneut ausführen." -ForegroundColor Yellow
  exit 0
}

$distros = (wsl.exe --list --quiet) -replace "`0", "" | Where-Object { $_ -match '\S' } | ForEach-Object { $_.Trim() }
if ($distros -notcontains $Distro) {
  Info "Installiere Distro $Distro ..."
  wsl.exe --install -d $Distro
  Info "Beim ersten Start Linux-Benutzer + Passwort anlegen, dann dieses Script erneut ausführen."
  exit 0
}
Info "WSL2 vorhanden, Distro: $Distro"

if (-not $SkipVSCode) {
  Step "VS Code (Windows) + Extensions"
  if (-not (Get-Command code -ErrorAction SilentlyContinue)) {
    Info "Installiere VS Code über winget ..."
    winget install --id Microsoft.VisualStudioCode --accept-package-agreements --accept-source-agreements
    Warn "Nach der Installation eine NEUE PowerShell öffnen (PATH), dann dieses Script erneut ausführen."
    exit 0
  }
  # verifizierte Marketplace-IDs
  code --install-extension ms-vscode-remote.remote-wsl --force
  code --install-extension anthropic.claude-code --force
  Info "VS Code + WSL-Remote + Claude Code installiert"
}

Step "Setup in der WSL-Distro ausführen"
$repoWin = (Get-Location).Path
$repoWsl = (wsl.exe -d $Distro -- wslpath -a "$repoWin").Trim()
Info "Repo im WSL-Pfad: $repoWsl"

# In das Linux-HOME kopieren: über /mnt/c laufende Scripts sind langsam und
# verlieren das Exec-Bit.
$target = "`$HOME/claude-setup-share"
$flags = if ($WithLocal) { "--with-local" } else { "" }
$cmd = "set -e; mkdir -p $target; cp -R '$repoWsl/.' $target/; cd $target; chmod +x bootstrap.sh verify.sh bundle/bin/* bundle/workbench/*.sh; ./bootstrap.sh $flags"
wsl.exe -d $Distro -- bash -lc "$cmd"

Step "Fertig"
Write-Host @"
   Weiter in der WSL-Distro:
     wsl -d $Distro
     claude              # einloggen (eigenes Abo, Team-Seat oder Console/API)
     ~/claude-setup-share/verify.sh

   VS Code: mit dem WSL-Remote-Fenster arbeiten ("WSL: $Distro" unten links).
   Die Workbench-Extension und die tmux-Worker leben IN der Distro.
"@
