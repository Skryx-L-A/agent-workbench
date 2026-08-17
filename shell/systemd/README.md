# wb-testsuite als systemd-User-Timer (peer)

Gegenstueck zu `../agent-workbench.wb-testsuite.plist` (macOS/launchd). Faehrt denselben
Wrapper (`shell/wb-testsuite-run`), schreibt dieselbe Statusdatei
(`~/.local/state/wb-testsuite-status.txt`) an denselben Ort -- der SessionStart-Hook
`sessionstart-testsuite-status.sh` braucht dafuer keine Anpassung.

Noch nicht installiert. der Nutzer rollt selbst aus.

## Installation

```bash
mkdir -p ~/.config/systemd/user
cp ~/AI/claude-workbench/shell/systemd/wb-testsuite.service ~/.config/systemd/user/
cp ~/AI/claude-workbench/shell/systemd/wb-testsuite.timer ~/.config/systemd/user/
loginctl enable-linger "$USER"   # ueberlebt Logout/Neustart; auf peer laut Check 2026-08-04 schon "yes"
systemctl --user daemon-reload
systemctl --user enable --now wb-testsuite.timer
```

## Pruefen

```bash
systemctl --user list-timers wb-testsuite.timer
systemctl --user status wb-testsuite.timer
journalctl --user -u wb-testsuite.service -n 50   # nur bei Absturz VOR dem eigenen Schreiben relevant
cat ~/.local/state/wb-testsuite-status.txt         # nach dem ersten Lauf
```

Manueller Testlauf ohne auf Sonntag 20:00 zu warten:

```bash
systemctl --user start wb-testsuite.service
```

## Deinstallation

```bash
systemctl --user disable --now wb-testsuite.timer
rm ~/.config/systemd/user/wb-testsuite.service ~/.config/systemd/user/wb-testsuite.timer
systemctl --user daemon-reload
```
