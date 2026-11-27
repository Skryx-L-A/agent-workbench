# `datentor person`: Leseklasse der Person

Teilvertrag zu Vertrag 3 (`rust/spec/VERTRAEGE.md`) für A05 und die zweite Gegenprobe von A34: Route und Person müssen eine Einheit beide zulassen; es gilt der strengere Wert.

- `rust/spec/VERTRAEGE.md`: Regeln P1 bis P7, Fallformat, Nahtstellen zu Vertrag 2, 4 und 5, Vorgabe für den Bau, Fragen F1 bis F6, Nachtrag 27.11.2026 (Quelle `leseklassen`, Pseudonym, Lesen ohne Route).
- `faelle_bauen.py`: schreibt `sichtbar/` und prüft jede Erwartung gegen die Referenz; die Gegenprobe ohne Personenregel muss jeden gültigen Fall rot machen.
- `aussen_laeufer.py`: Läufer von Vertrag 3 mit dem Unterbefehl `person`.

```
python3 -I -B faelle_bauen.py
DT_BIN=<programm> DT_FAELLE=sichtbar python3 -I -B aussen_laeufer.py [name ...]
```

Das Programm ist `datentor person` aus dem Crate `datentor`. Alle Daten sind erfunden.
