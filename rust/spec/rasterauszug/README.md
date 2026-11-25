# Vertrag `rasterauszug`

Der Vertrag `rasterauszug` legt das Verhalten des Crates `rasterauszug` fest (Regeln RA1 bis RA14): den Auszug des Bildschirmrasters je Betrachter, die Zeilenform als SGR-Text, die Byte-Form, die Deflate-Leitung und die Rundreise. Ein Rust-Bau gilt als vertragstreu, wenn er beide Fallsätze (sichtbar und verdeckt) besteht.

Der Vertrag steht in `rust/spec/VERTRAEGE.md` neben dieser Datei. Er ist ausführbar gemeint: Ein Bau gilt als vertragstreu, wenn er die Fälle besteht.

## Anforderungen

Die Anforderungsnummern stammen aus der Anforderungsliste des Neubaus (außerhalb des Repos). `rust/spec/VERTRAEGE.md` nennt sie nur zum Teil. Die Zuordnung folgt der Spalte „Baustein“ der Liste und ist hergeleitet, nicht aus dem Vertrag gemessen.

- A118: Beide Ansichten kommen mit Schritt II in der Gestalt nach A157, jede mit derselben Liste der Bedienwege.
- A157: Die Oberfläche ist die heutige Werkbank.

## Sichtbare Fälle im Repo

`rust/crates/rasterauszug/tests/faelle/` (99 Fälle, Namen `ra1_*` bis `ra14_*`) mit den Erwartungen in `rust/crates/rasterauszug/tests/erwartet/`; der Läufer ist `rust/crates/rasterauszug/tests/faelle.rs`. Weitere Prüfungen des Crates: `grenzen.rs`, `nebenwirkung.rs`, `robust.rs`, `rundreise.rs`.

## Gegen den Bau fahren

Aus dem Ordner `rust/`: `cargo test -p rasterauszug --test faelle`. Der Läufer prüft `leitung_hex` und `leitung_laenge` für sich; die Deflate-Bytes selbst vergleicht er nicht.

## Nicht im Repo

Der verdeckte Satz, die Referenz der Gegenprobe und der Fallbau liegen außerhalb des Repos.
