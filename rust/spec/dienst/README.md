# Vertrag `dienst`

Der Vertrag `dienst` legt das Verhalten des Kern-Dienstes `wb-dienst` fest (Regeln DI1 bis DI12): Startdatei, Rechte, Sitzungen mit Kind im Pseudo-Terminal, Zustandskern und Rasterauszug je Betrachter, ein Unix-Socket, mehrere Sitzungen (DI12). Der Dienst übersetzt nur; jede Regel des Kerns, des Auszugs und des Trägers bleibt in deren Verträgen.

Der Vertrag steht in `rust/spec/VERTRAEGE.md` neben dieser Datei. Er ist ausführbar gemeint: Ein Bau gilt als vertragstreu, wenn er die Fälle besteht.

## Anforderungen

Die Anforderungsnummern stammen aus der Anforderungsliste des Neubaus (außerhalb des Repos). `rust/spec/VERTRAEGE.md` nennt sie nur zum Teil. Die Zuordnung folgt der Spalte „Baustein“ der Liste und ist hergeleitet, nicht aus dem Vertrag gemessen.

- A64: Agentensitzungen und Terminal laufen unter Windows nativ, ohne tmux. Der eigene Terminalträger ist dort der einzige Weg.
- A118: Beide Ansichten kommen mit Schritt II in der Gestalt nach A157, jede mit derselben Liste der Bedienwege.
- A120: Das Produkt bietet selbst keinen ungeschützten TCP-Dienst auf 127.0.0.1 an, auf allen Systemen.
- A43: Die Route der Sitzung ist Pflichtangabe; eine Anfrage ohne Route wird abgelehnt. Der Dienst prüft dafür die Stufen von Vertrag `start`, darunter 6 `schluessel_fehlt` (`rust/spec/VERTRAEGE.md`, DI12, Absatz „Stufen, die der Dienst prüft“).
- A149: Der Pfad jedes Unix-Sockets ist unter macOS höchstens 103 Zeichen lang. Der Dienst nimmt für seinen Socket höchstens 100 Byte an (`rust/spec/VERTRAEGE.md`, Abschnitt 3 Startdatei, DI1: Grenze von `sun_path` am Mac).

Abgleich mit der Anforderungsliste: 22.11.2026; die Zuordnung von A43 und A149 ist aus dem Vertragstext belegt.

## Sichtbare Fälle im Repo

`rust/crates/dienst/tests/faelle/` (280 Fälle, Namen `di1_*` bis `di12_*` und `l9_*`) mit den Erwartungen in `rust/crates/dienst/tests/erwartet/`; der Läufer ist `rust/crates/dienst/tests/faelle.rs`. Prüfclient, Läufer und Python-Referenz des Betrachters liegen unter `rust/crates/dienst/tests/werkzeug/`. Weitere Prüfungen des Crates: `abschirmung.rs`, `beobachtung.rs`, `dienst.rs`, `groessenwechsel.rs`, `kinder.rs`, `mehrere.rs`.

Nachtrag 26.11.2026 (Oberfläche L1 bis L11, `rust/spec/VERTRAEGE.md` am Ende): `faelle/` hat 278 Fälle; dazu kamen `dil2_zwei_panes_groesse`,
`dil2_zwei_betrachter_zwei_panes` und `dil6_heute`, die der heutige Code besteht (`dil6_heute` misst den Weg der Freigabe vor dem Nachtrag).
`rust/crates/dienst/tests/faelle-ausstehend/` mit `erwartet-ausstehend/` hält 84 Fälle, die erst der Bau des Nachtrags besteht: 25 neue
(`dil*`) und 59 bestehende Fälle mit neuer Erwartung unter demselben Namen (58 mit den neuen Feldern der `Sitzungsliste`, dazu
`di12_lauf_ereignisse` mit `ereignis_ungueltig` für eine `Freigabe` mit fremdem Schlüssel). Der Läufer liest den Ordner nicht. Der Prüfclient
kennt die Angaben des Nachtrags (`bestand`, `weg: "lokal"`, `wer` im Zustand; `rust/spec/VERTRAEGE.md`, Abschnitt „Prüfmittel des Nachtrags“).

## Gegen den Bau fahren

Aus dem Ordner `rust/`: `cargo test -p dienst --test faelle`. Der Lauf braucht `python3` und `lsof`; fehlt eines, ist er rot, nicht übersprungen. Die Fälle laufen in mehreren Läufern gleichzeitig, jeder in einem eigenen Grundordner.

## Nicht im Repo

Der verdeckte Satz, die Referenz der Gegenprobe und der Fallbau liegen außerhalb des Repos.
