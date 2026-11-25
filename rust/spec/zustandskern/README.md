# Vertrag `zustandskern`

Der Vertrag `zustandskern` legt die Logik des Zustandskerns der neuen Oberfläche fest (Regeln R1 bis R15): Ereignisse und Wirkungen, Räume, Sitzungen, Rechte, Rückstau, Cursor.

Der Vertrag steht in `rust/spec/VERTRAEGE.md` neben dieser Datei. Er ist ausführbar gemeint: Ein Bau gilt als vertragstreu, wenn er die Fälle besteht.

## Anforderungen

Die Anforderungsnummern stammen aus der Anforderungsliste des Neubaus (außerhalb des Repos). `rust/spec/VERTRAEGE.md` nennt sie nur zum Teil. Die Zuordnung folgt der Spalte „Baustein“ der Liste und ist hergeleitet, nicht aus dem Vertrag gemessen.

- A16: Nachrichten anderer Sitzungen tragen ihren Absender und erscheinen nie als Text des Menschen.
- A21: Jeder Dateibeobachter wird zuerst eingehängt und prüft danach einmal den Zustand.
- A22: „Unbekannt“ (nicht nachgesehen) ist nie ein Endzustand der Anzeige.
- A03: Agent und Mensch sind getrennt. Ein Betrachter, dessen Person nicht `art: mensch` hat, wird mit `agent_kennung` abgelehnt, auch mit gültigem Nachweis (`rust/spec/VERTRAEGE.md`, R7 Freigaben).
- A53: Kein Übertrag zwischen Kundenräumen. Ein Abo auf eine Sitzung eines anderen Kundenraums wird mit `fremder_kundenraum` abgelehnt (`rust/spec/VERTRAEGE.md`, R4 Abo und Abzug).

Abgleich mit der Anforderungsliste: 22.11.2026; die Zuordnung von A03 und A53 ist aus dem Vertragstext belegt.

## Sichtbare Fälle im Repo

`rust/crates/zustandskern/tests/faelle/` (534 Fälle; `r*` und `x_*` sind die sichtbaren Fälle des Vertrags, `g_*` sind erzeugte Fälle mit Erwartung aus der Python-Referenz, `h_*` sind von Hand zu den Anmerkungen des Vertrags geschrieben) mit den Erwartungen in `rust/crates/zustandskern/tests/erwartet/`; der Läufer ist `rust/crates/zustandskern/tests/faelle.rs`. Der Erzeuger der `g_*`-Fälle ist `rust/crates/zustandskern/tests/werkzeug/erzeuge_faelle.py`, die Referenz `ra_ref.py` liegt daneben. Weitere Prüfungen des Crates: `grenzen.rs`, `nebenwirkung.rs`, `schritt.rs`.

## Gegen den Bau fahren

Aus dem Ordner `rust/`: `cargo test -p zustandskern --test faelle`.

## Nicht im Repo

Der verdeckte Satz, die Referenz der Gegenprobe und der Fallbau liegen außerhalb des Repos.
