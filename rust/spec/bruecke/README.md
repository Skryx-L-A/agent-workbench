# Vertrag `bruecke`

Der Vertrag `bruecke` legt das Verhalten des Crates `bruecke` und des Programms `wb-bruecke` fest: Die Brücke hängt am Unix-Socket des Dienstes, liefert die Ansicht für den Browser über HTTP und reicht die Rahmen des Dienstes weiter. Der Vertrag `dienst` (`rust/spec/dienst/VERTRAG.md`) ist die andere Seite jedes Falls.

Der Vertrag steht in `rust/spec/VERTRAEGE.md` neben dieser Datei. Er ist ausführbar gemeint: Ein Bau gilt als vertragstreu, wenn er die Fälle besteht.

## Anforderungen

Die Anforderungsnummern stammen aus der Anforderungsliste des Neubaus (außerhalb des Repos). `rust/spec/VERTRAEGE.md` nennt sie nur zum Teil. Die Zuordnung folgt der Spalte „Baustein“ der Liste und ist hergeleitet, nicht aus dem Vertrag gemessen.

- A118: Beide Ansichten kommen mit Schritt II in der Gestalt nach A157, jede mit derselben Liste der Bedienwege.
- A157: Die Oberfläche ist die heutige Werkbank.

## Sichtbare Fälle im Repo

`rust/crates/bruecke/tests/aussen/aussentests/faelle/` (62 Fälle, Datensatz A) mit den Erwartungen in `rust/crates/bruecke/tests/aussen/aussentests/erwartet/`; der Läufer ist `rust/crates/bruecke/tests/aussen.rs`. Der Läufer des Vertrags (`test_aussen.py`), der Prüfclient (`pruefclient.py`) und die Python-Referenzen der Gegenseite (`referenz/`) liegen unter `rust/crates/bruecke/tests/aussen/`; dessen `README.md` nennt die Prüfsummen der Kopien. Weitere Prüfungen des Crates: `beobachtung.rs`, `messung.rs`, `programm.rs`.

## Gegen den Bau fahren

Aus dem Ordner `rust/`: `cargo test -p bruecke --test aussen`. Jeder Fall läuft in einem eigenen Läufer (`python3 -I -B test_aussen.py <Fall>`), einige gleichzeitig. Der Lauf braucht `python3` und `lsof`.

## Nicht im Repo

Der verdeckte Satz, die Referenz der Gegenprobe und der Fallbau liegen außerhalb des Repos.
