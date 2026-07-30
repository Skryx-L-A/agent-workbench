---
title: Brain 4.0 migration runbook
type: reference
branch: _meta/tools/migrate
permalink: main/tools/migrate/runbook-1-1
---

Ausführungs-Checkliste für den Brain-4.0-Umzug. Ohne Rückfragen abarbeitbar, in
dieser Reihenfolge. Jeder Befehl ist zum Kopieren. Was in `$VAULT` steht, ist
`~/Knowledge`. Kontrakt: [[BRAIN4-PLAN]]. Werkzeug: [[brain4 migration tool]].

Läuft alles durch, dauert es ~15 Minuten. Der einzige Schritt, der nicht
umkehrbar ist, ist Schritt 12 (push) — bis dahin bringt Schritt 0 alles zurück.

```sh
export VAULT="$HOME/Knowledge"
export SNAP="$HOME/.local/trash-snapshots/2026-07-28-brain4"
```

---

## 0. Snapshot und Vorbedingungen

- [ ] **Keine anderen Worker im Vault.** Prüfen, dass niemand gerade in
      `tools/gardener/`, `tools/braincli/`, `tools/eval/`, `templates/`,
      `AGENTS.md` oder `STATUS.md` schreibt. Läuft dort noch etwas, hier
      abbrechen — die Migration fasst genau diese Dateien an.
      ```sh
      tmux ls 2>/dev/null; ls ~/.pi-workers/results/
      ```
- [ ] **Zweite Person.** In [[shared-projects]] nachsehen, ob ein Mitnutzer gerade am
      Vault arbeitet. Wenn ja: erst abstimmen.
- [ ] **Arbeitskopie als Rückweg** (90-secrets bleibt draußen, wird nie kopiert).
      `apply` legt diesen Snapshot inzwischen selbst an und bricht ab, wenn der
      Arbeitsbaum nicht sauber ist — dieser Schritt ist damit die Gegenprobe,
      nicht mehr die einzige Absicherung:
      ```sh
      mkdir -p "$SNAP" && rsync -a --exclude 90-secrets "$VAULT/" "$SNAP/"
      du -sh "$SNAP"
      ```
- [ ] **Git-Stand sauber und aktuell:**
      ```sh
      cd "$VAULT" && git pull --rebase && git status --porcelain
      ```
      Ausgabe muss leer sein. Ist sie es nicht: die offenen Änderungen erst
      committen, sonst landen sie im Migrations-Commit.
- [ ] **Rückweg-Marke setzen:**
      ```sh
      cd "$VAULT" && git rev-parse HEAD | tee "$SNAP/HEAD-before-brain4.txt"
      ```

## 1. Trockenlauf gegen eine frische Kopie

Die Zahlen aus dem Worker-Report sind ein Schnappschuss. Erst gegen eine frische
Kopie laufen lassen, dann erst gegen den echten Vault.

- [ ] ```sh
      rm -rf /tmp/brain4-check
      rsync -a --exclude 90-secrets "$VAULT/" /tmp/brain4-check/
      rm -f /tmp/brain4-check/.brain4-manifest.json
      cd /tmp/brain4-check && git config user.email you@local && git config user.name you \
        && git add -A && git commit -qm base
      cp "$VAULT/tools/migrate/brain4.py" /tmp/brain4.py
      python3 /tmp/brain4.py plan    --vault /tmp/brain4-check
      python3 /tmp/brain4.py apply   --vault /tmp/brain4-check
      python3 /tmp/brain4.py rewrite --vault /tmp/brain4-check
      python3 /tmp/brain4.py verify  --vault /tmp/brain4-check; echo "verify exit=$?"
      ```
- [ ] `verify exit=0` und in der Ausgabe: `notes before N after N`,
      `permalinks … 0 missing, 0 inconsistent`, `ids … 0 missing/duplicate,
      0 malformed`, `90-secrets … unchanged: True`, und die Zahl der
      unaufgelösten Wikilinks nach dem Lauf **nicht größer** als davor.
      Stimmt eines davon nicht: **hier abbrechen** und den Worker fragen.
- [ ] Aufräumen: `rm -rf /tmp/brain4-check` (`/tmp/brain4.py` bleibt, Schritt 2 braucht es)

## 2. Migration auf dem echten Vault

Das Werkzeug liegt selbst im Vault und zieht bei `apply` mit nach
`_meta/tools/migrate/`. Deshalb wird es VORHER nach `/tmp` kopiert — sonst
findet der `rewrite`-Aufruf seine eigene Datei nicht mehr.

- [ ] ```sh
      cd "$VAULT"
      cp "$VAULT/tools/migrate/brain4.py" /tmp/brain4.py
      python3 /tmp/brain4.py plan    --vault "$VAULT"
      python3 /tmp/brain4.py apply   --vault "$VAULT" --allow-real-vault
      python3 /tmp/brain4.py rewrite --vault "$VAULT" --allow-real-vault
      python3 /tmp/brain4.py verify  --vault "$VAULT"; echo "verify exit=$?"
      ```
      `--allow-real-vault` ist Absicht: ohne das Flag verweigert das Werkzeug den
      Schreibzugriff auf `~/Knowledge`. `plan` und `verify` brauchen es nicht.
- [ ] `verify exit=0`. Sonst: Schritt 13 (Rückweg).

## 3. Gardener-, eval- und ingest-Patch

- [ ] ```sh
      cd "$VAULT"
      git apply --stat _meta/tools/migrate/gardener-brain4-taxonomy.patch
      git apply        _meta/tools/migrate/gardener-brain4-taxonomy.patch && echo "Patch sauber angewendet"
      ```
      `--stat` zeigt, was der Patch selbst ändert — `git diff` hilft hier nicht,
      weil die Migration aus Schritt 2 noch nicht committet ist und ihre
      Änderungen mitzählen würde.
      Erwartet: 7 Dateien, 12 Zeilen geändert. Enthält die drei echten
      Tiefenbugs (`parents[3]`->`parents[4]` in eval, `parents[2]`->`parents[3]`
      in ingest, `VAULT_ROOT / "_meta" / "tools" / "braincli"`),
      `MINED_DIR = "00-sources/mined"`, `EXCLUDE_DIRS = {"90-secrets",
      ".obsidian", ".git", "_meta"}` und die drei Test-Fixtures.

## 4. Hook-Pfade in `~/.claude/settings.json`

Drei Hooks sind mit absolutem Pfad registriert und zeigen nach dem Umzug ins
Leere. Sie scheitern still (`2>/dev/null`), deshalb hier von Hand.

- [ ] ```sh
      cp ~/.claude/settings.json "$SNAP/settings.json.bak"
      python3 - <<'PY'
import json, pathlib
p = pathlib.Path.home() / ".claude" / "settings.json"
t = p.read_text()
for old, new in (("Knowledge/tools/session-context.sh", "Knowledge/_meta/tools/session-context.sh"),
                 ("Knowledge/tools/hooks/",            "Knowledge/_meta/tools/hooks/")):
    t = t.replace(old, new)
json.loads(t)          # syntaktisch heil, bevor es geschrieben wird
p.write_text(t)
print("settings.json aktualisiert")
PY
      grep -n 'Knowledge/_meta/tools' ~/.claude/settings.json
      ```
      Erwartet: drei Treffer (`session-context.sh`, `hooks/auto-recall.sh`,
      `hooks/read-tracking.sh`).

## 5. Skripte in `~/.local/bin`

- [ ] ```sh
      cp ~/.local/bin/brain ~/.local/bin/ai-scout ~/.local/bin/check-ollama-kv-ssd "$SNAP/"
      sed -i '' 's#Knowledge/tools/braincli#Knowledge/_meta/tools/braincli#g' ~/.local/bin/brain
      sed -i '' 's#Knowledge/00-inbox#Knowledge/00-sources#g' ~/.local/bin/ai-scout ~/.local/bin/check-ollama-kv-ssd
      grep -n 'Knowledge/' ~/.local/bin/brain ~/.local/bin/ai-scout ~/.local/bin/check-ollama-kv-ssd
      ```
      In keiner Zeile darf noch `Knowledge/tools/` oder `Knowledge/00-inbox`
      stehen.
- [ ] Gegenprobe über den ganzen Ordner:
      ```sh
      grep -rn --exclude-dir=.git -e 'Knowledge/tools/' -e 'Knowledge/00-inbox' -e 'Knowledge/templates' ~/.local/bin || echo "sauber"
      ```

## 6. Venvs neu bauen (16 kaputte Shebangs)

Jedes Skript in den venvs trägt den alten absoluten Pfad im Shebang. Ein Umzug
schreibt den nicht um — die venvs müssen neu erzeugt werden.

- [ ] ```sh
      for t in braincli gardener eval; do
        rm -rf "$VAULT/_meta/tools/$t/.venv"
        (cd "$VAULT/_meta/tools/$t" && uv sync)
      done
      head -1 "$VAULT/_meta/tools/braincli/.venv/bin/brain"
      ```
      Die letzte Zeile muss `_meta` enthalten. Steht dort noch
      `Knowledge/tools/braincli`, hat `uv sync` nicht gegriffen.
- [ ] Kein Rest mit altem Shebang:
      ```sh
      grep -rl '^#!$HOME/Knowledge/tools/' "$VAULT/_meta/tools"/*/.venv/bin/ 2>/dev/null || echo "keine alten Shebangs"
      ```

## 7. Git-Hook neu installieren

- [ ] ```sh
      cp "$VAULT/_meta/tools/git-hooks/pre-push" "$VAULT/.git/hooks/pre-push"
      chmod +x "$VAULT/.git/hooks/pre-push"
      grep -c '_meta/tools/git-hooks' "$VAULT/.git/hooks/pre-push"
      ```
      Erwartet: 10 (die Exclude-Pfade im Secrets-Scan, zwei je Muster).

## 8. Verifikation: die fuenf Testsuiten

- [ ] ```sh
      cd "$VAULT/_meta/tools/gardener" && uv run --with pytest python -m pytest tests -q
      cd "$VAULT/_meta/tools/braincli" && uv run --with pytest python -m pytest tests -q
      cd "$VAULT/_meta/tools/eval"     && uv run --with pytest python -m pytest tests -q
      cd "$VAULT/_meta/tools/migrate"  && uv run --with pytest python -m pytest tests -q
      # (entfaellt seit 2026-07-29: _meta/tools/ingest wurde durch `brain ingest`
      #  abgeloest und geloescht)
      ```
      Erwartet (Stand des Trockenlaufs): gardener 152, braincli 27, eval 15,
      migrate 39, ingest 5 — alle grün. Ein rotes Ergebnis ist ein Stopp.

## 9. Verifikation: die Werkzeuge laufen wirklich

- [ ] `brain` findet Notes:
      ```sh
      brain search "shared brain" -k 3
      brain stats
      ```
      Beides muss Treffer bzw. Zahlen liefern, nicht `command not found` und
      keinen Traceback.
- [ ] Gardener sieht den richtigen Korpus (nichts aus `_meta/`, nichts aus
      `90-secrets/`):
      ```sh
      cd "$VAULT/_meta/tools/gardener" && uv run --project . python -c "
from pathlib import Path
from gardener.vault import load_notes
rels = sorted(n.rel for n in load_notes(Path('$VAULT')))
print('Notes im Korpus:', len(rels))
print('aus _meta:', [r for r in rels if r.startswith('_meta')])
print('aus 90-secrets:', [r for r in rels if r.startswith('90-secrets')])
"
      ```
      Beide Listen müssen leer sein.

## 10. Verifikation: die Hooks feuern wirklich

- [ ] SessionStart-Hook direkt aufrufen (er darf nicht leer bleiben):
      ```sh
      bash "$VAULT/_meta/tools/session-context.sh" | head -20
      ```
- [ ] Read-Tracking schreibt in die neue Log-Datei:
      ```sh
      wc -l "$VAULT/_meta/tools/state/read-heat.log"
      ```
      Danach eine **neue Claude-Session** öffnen und dort prüfen: der
      Brain-Recall-Block und `INDEX.md`/`CRITICAL-FACTS.md` erscheinen wie
      gewohnt am Sessionanfang. Danach noch einmal `wc -l` — die Zeilenzahl muss
      gewachsen sein. Wächst sie nicht, greift Schritt 4 nicht.

## 11. Aufräumen und Doku

- [ ] Manifest löschen (es ist Werkzeug-Ausgabe, kein Vault-Inhalt):
      ```sh
      rm -f "$VAULT/.brain4-manifest.json"
      ```
- [ ] `LOG.md` und `STATUS.md` um einen Eintrag ergänzen: Datum, was umgezogen
      ist, dass jede Note jetzt eine unveränderliche `id` hat.
- [ ] Kurz-Notiz für die zweite Person: der einzige Befehl, den sie braucht, ist
      `git pull --rebase` — Wikilinks überleben den Umzug, weil kein Dateiname
      sich ändert. Wer lokale Änderungen hat, committet sie vorher.

## 12. Ein einziger Commit, sofort gepusht

Geteilter Vault: kein langlebiger Branch, kein force-push.

- [ ] ```sh
      cd "$VAULT"
      git add -A
      git status --porcelain | head -30       # Umbenennungen als R sichtbar?
      git commit -m "Brain 4.0: vault re-layout (00-sources, 40-people, _meta, sessions/) + frontmatter schema v4"
      git pull --rebase
      git push
      ```
- [ ] Nach dem Push: `git log --oneline -1` und `git status` (muss sauber sein).

## 13. Rückweg, falls irgendein Schritt scheitert

Vor dem Push (Schritte 2–11):

```sh
cd "$VAULT" && git reset --hard "$(cat "$SNAP/HEAD-before-brain4.txt")" && git clean -fd
cp "$SNAP/settings.json.bak" ~/.claude/settings.json
cp "$SNAP/brain" "$SNAP/ai-scout" "$SNAP/check-ollama-kv-ssd" ~/.local/bin/
for t in braincli gardener eval; do rm -rf "$VAULT/tools/$t/.venv"; (cd "$VAULT/tools/$t" && uv sync); done
```

`git clean -fd` entfernt auch nicht getrackte Dateien — deshalb liegt in `$SNAP`
eine vollständige rsync-Kopie. Ist nach dem Reset etwas verschwunden, kommt es
von dort zurück.

Nach dem Push: `git revert` des Migrations-Commits, dann Schritte 4–7 rückwärts
(die Kopien in `$SNAP` zurückspielen) und die zweite Person informieren.