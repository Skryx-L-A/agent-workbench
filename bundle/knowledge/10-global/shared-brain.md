---
permalink: main/10-global/shared-brain
---

---
title: Shared Brain — Multi-User Vault Rules
type: note
branch: 10-global
tags: [vault, multi-user, git]
created: 2026-07-13
review-after: 2026-10
---

Für künftige Sessions: IMMER lesen, sobald mehr als eine Person am Vault schreibt, oder wenn
unklar ist wer gerade arbeitet, ob etwas geteilt oder privat ist, oder wie Git-Konflikte im
Vault zu lösen sind.

## Ausgangslage

`~/Knowledge` ist ab 2026-07-13 ein geteiltes Brain zweier Personen: [[person-1]] (Owner,
GitHub `<your-github-user>`) und [[person-2]] (Platzhalter `{{PERSON_2}}` / `{{PERSON_2_GITHUB}}`,
Details noch offen — siehe TODO in der Person-Note). Ein Vault, ein git-Remote
(`<your-github-user>/knowledge-vault`, privates Repo). Person 2 wird als Collaborator eingeladen.

## Git-Hygiene (Kern der Regel)

- **Vor jedem Schreiben und vor jedem Push:** `git pull --rebase`. Nie ungeprüft in einen
  veralteten Stand schreiben.
- **Nie force-push.** Force überschreibt die Arbeit der anderen Person unsichtbar.
- **Konflikte:** in Markdown lösen (beide Versionen lesen, Inhalt zusammenführen oder die
  neuere/vollständigere Fassung behalten), nie eine Seite blind verwerfen (`git checkout
  --ours`/`--theirs` ohne Lesen ist verboten).
- **Wenn beide gleichzeitig arbeiten:** kleine, häufige Commits statt große Sessions ungepusht
  liegen lassen — reduziert Konfliktfläche und Verlustrisiko.
- Push bleibt Orchestrator-Sache (siehe [[CRITICAL-FACTS]]) — aber `git pull --rebase` vor dem
  Arbeiten darf und soll jede Session/jeder Agent selbst tun, das ist kein Push.

## Secrets-Trennung

- `90-secrets/` ist gitignored und bleibt es — für beide Personen. Wird nie committed, nie
  gepusht, nie zwischen Maschinen synced.
- Nichts Sensibles verlässt je die jeweilige Maschine: keine Keys, Tokens, Passwörter,
  Kundendaten, keine `.env`-Inhalte — auch nicht "kurz zum Zeigen" in eine normale Note.
- Wer eine sensible Information referenzieren muss, schreibt einen **Pointer** (welcher Key,
  wo er liegt, z.B. "Stripe-Secret liegt in `90-secrets/stripe.md` auf Maschine des Nutzers"),
  nie den Wert selbst.

## Persönlich vs. geteilt

- **Geteilt (git-versioniert, beide lesen/schreiben):** `10-global/`, `30-topics/`,
  `templates/`, `tools/`.
- **Geteilt, aber projekt-attribuiert:** `20-projects/<projekt>/` — jedes Projekt markiert im
  MOC/Frontmatter, wessen Projekt es primär ist (Owner-Feld), damit unklar ist, wer die
  Kontext-Hoheit hat, nicht ob man mitlesen darf.
- **Nicht geteilt:** rein persönliches Zeug (Gesundheit, Finanzen, Privates ohne Projektbezug)
  gehört NICHT in den geteilten Vault. Jede Person führt das außerhalb, auf eigener Maschine.

## Wer arbeitet gerade — Identitäts-Mechanik

Siehe `IDENTITY.md` (gitignored, pro Maschine lokal) — löst, dass der Vault früher hart auf den Nutzer als einzigen User kodiert war. Details und Rationale: [[CRITICAL-FACTS]] und die
Kommentare in `tools/session-context.sh`. Kurzfassung: die lokale, ungeteilte `IDENTITY.md`
jeder Maschine sagt einem Agenten, welche Person gerade sitzt; Personendetails selbst leben in
`10-global/people/*.md` (geteilt, versioniert).

Fehlt `IDENTITY.md` auf einer frisch geklonten Maschine (z.B. Person 2 nach `git clone` —
`IDENTITY.md` ist gitignored, kommt also NICHT mit dem Clone mit), meldet der SessionStart-Hook
(`tools/session-context.sh`) das sichtbar mit einer Zeile "IDENTITY.md fehlt — lege sie aus
IDENTITY.md.example an". Vorlage dafür ist `IDENTITY.md.example` (versioniert, Vault-Root,
ausgefülltes Dummy-Beispiel) — kopieren nach `IDENTITY.md`, eigene Werte eintragen, nie
committen. Ein Agent, der diese Warnzeile sieht, soll das selbstständig beheben (Datei
kopieren + Person nach ihren Werten fragen), nicht nur melden.

<!-- relates-to [[person-1]] | relates-to [[person-2]] | relates-to [[CRITICAL-FACTS]] -->
