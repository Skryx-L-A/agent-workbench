# Security Policy

## Reporting a vulnerability

Please do not open a public issue. Report it privately through GitHub:
[Report a vulnerability](https://github.com/Skryx-L-A/agent-workbench/security/advisories/new).

Include what is affected, how to reproduce it, and which version or commit you tested. Please
remove secrets and personal data from what you send.

This is a one-person project: reports are answered and fixed on a best-effort basis, and there
is no bug bounty.

## Supported versions

Fixes go to the branches `main` and `development` and to the latest release.

`development` is a work in progress and not released for production use. It is the version for
companies, and some protections that a company deployment needs are not complete yet. The open
points are, by kind:

- access control between users and between machines,
- an audit log that covers every action,
- requests under data protection law (access, correction, deletion),
- installation and hardening of the supporting services.

Do not run `development` on a machine that holds other people's data until these points are
closed. This file is changed when a point is closed.

# Sicherheitsrichtlinie

## Eine Schwachstelle melden

Bitte eröffnen Sie kein öffentliches Issue. Melden Sie sie privat über GitHub:
[Schwachstelle melden](https://github.com/Skryx-L-A/agent-workbench/security/advisories/new).

Nennen Sie, was betroffen ist, wie sich der Fehler nachstellen lässt und welche Version oder
welchen Commit Sie getestet haben. Entfernen Sie bitte Geheimnisse und personenbezogene Daten aus
dem, was Sie senden.

Dies ist ein Ein-Personen-Projekt: Meldungen werden nach bestem Vermögen beantwortet und
behoben, ein Bug-Bounty-Programm gibt es nicht.

## Unterstützte Versionen

Korrekturen gehen in die Zweige `main` und `development` und in die jeweils letzte Version.

`development` ist ein Stand in Arbeit und nicht für den Betrieb freigegeben. Er ist die Version
für Firmen, und einige Schutzfunktionen, die eine Firmeninstallation braucht, sind noch nicht
vollständig. Die offenen Punkte, nach Art:

- Zugriffssteuerung zwischen Nutzern und zwischen Rechnern,
- ein Protokoll, das jede Handlung erfasst,
- Anfragen nach dem Datenschutzrecht (Auskunft, Berichtigung, Löschung),
- Einrichtung und Absicherung der unterstützenden Dienste.

Betreiben Sie `development` nicht auf einem Rechner mit fremden Daten, bis diese Punkte
geschlossen sind. Diese Datei wird geändert, sobald ein Punkt geschlossen ist.
