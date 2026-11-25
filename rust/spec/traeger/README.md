# Vertrag `traeger`

Der Vertrag `traeger` legt das Verhalten des eigenen Terminalträgers fest (Regeln TR1 bis TR16): Pseudo-Terminal des Kindes, Bildschirmspeicher, Antworten auf Terminalabfragen, Titel, Größe, Zwischenablage, Rücklauf, Ende des Kindes. Das Programm `tr` führt die Fälle des Vertrags aus.

Der Vertrag steht in `rust/spec/VERTRAEGE.md` neben dieser Datei. Er ist ausführbar gemeint: Ein Bau gilt als vertragstreu, wenn er die Fälle besteht.

## Anforderungen

Die Anforderungsnummern stammen aus der Anforderungsliste des Neubaus (außerhalb des Repos). `rust/spec/VERTRAEGE.md` nennt sie nur zum Teil. Die Zuordnung folgt der Spalte „Baustein“ der Liste und ist hergeleitet, nicht aus dem Vertrag gemessen.

- A64: Agentensitzungen und Terminal laufen unter Windows nativ, ohne tmux. Der eigene Terminalträger ist dort der einzige Weg.
- A107: Der Ankunftsbeleg eines Auftrags an eine Sitzung hängt nie vom Bildschirm-Rücklauf ab.
- A109: Der eigene Terminalträger beantwortet die Cursorabfrage `ESC[6n` selbst, auf allen Plattformen und unabhängig vom Bildschirmspeicher.
- A110: Den Titel einer Sitzung setzt nur das Programm in der Sitzung, mit OSC 0 oder OSC 2.
- A111: Die Größe einer Sitzung setzt nur ihr Besitzer (der Kern).
- A112: Der Träger und jede Oberfläche des Neubaus beantworten eine Leseanfrage der Zwischenablage aus einem Programm (OSC 52 mit „?“) nie.
- A113: Unter Windows trägt der Träger höchstens 0,1 ms zur Zeit von Taste bis Echo bei, gemessen gegen den leeren Durchreicher.
- A114: Umschalt+Enter sendet der Träger auf allen Systemen als `ESC CR`.
- A119: Der eigene Terminalträger nutzt `portable-pty` für das Pseudo-Terminal und `alacritty_terminal` als Bildschirmspeicher.

## Sichtbare Fälle im Repo

`rust/crates/traeger/tests/faelle/` (134 Fälle, Namen `tr1_*` bis `tr15_*`) mit den Erwartungen in `rust/crates/traeger/tests/erwartet/`; der Läufer ist `rust/crates/traeger/tests/faelle.rs`. Weitere Prüfungen des Crates: `grenzen.rs`, `kinder.rs`.

## Gegen den Bau fahren

Aus dem Ordner `rust/`: `cargo test -p traeger --test faelle`. Ein Fall mit dem Feld `last` läuft nur mit `TR_LAST=1`; ohne dieses ist er „nicht anwendbar“ (weder grün noch rot). Ein Fall mit dem Feld `system` läuft nur auf den genannten Systemen.

## Nicht im Repo

Der verdeckte Satz, die Referenz der Gegenprobe und der Fallbau liegen außerhalb des Repos.
