# Seite: Rasterdekoder für die Browser-Seite

**Seit 21.11.2026 liefert `wb-bruecke` diese Seite nicht mehr aus** (Nachtrag vom 21.11.2026): Unter `GET /` steht die Ansicht aus
`rust/ansicht/` (Crate `ansicht`, Dioxus-web). Dieser Ordner bleibt im Repo, bis entschieden ist, ob er entfällt; seine Tests laufen weiter.

Dieser Ordner enthält die Browser-Seite des Durchstichs: den Rasterdekoder und die Seite selbst. Es ist nur JavaScript in ES-Modulen.
Der Ordner ist kein Crate und liegt bewusst nicht unter `rust/crates/` (der Cargo-Workspace nimmt dort jeden Ordner als Mitglied); es gibt kein `Cargo.toml`.

**Die Seite lädt keine fremden Skripte und ruft nichts im Netz auf.** Jedes Modul ist eine eigene Datei dieses Ordners, ohne `import` aus fremden Quellen, ohne `fetch`, ohne CDN und ohne npm-Paket. Auch die Tests brauchen nur `node` (Mac und Linux-Baurechner: Version 26).

## Module

| Datei | Aufgabe |
|---|---|
| `raster.js` | `Empfaenger`: der Dekoder der Rasterrahmen nach dem Vertrag `rasterauszug` (RA9 bis RA11, RA13). Ein Empfänger je Verbindung. `nimm(bytes)` liefert `{ auszug, abbild }` oder wirft `RasterFehler` mit demselben Code wie die Referenz `ra_ref.py`. `vergessen(nr)`, `abbild(nr)`, `klon()`. Exportiert auch `zeileZuHtml`. |
| `inflate.js` | `Inflater`: Deflate nach RFC 1951 mit Kontext über die Rahmen einer Verbindung (siehe unten). |
| `html.js` | `zeileZuHtml(zeile, doc)`: Zeilenform (Läufe `[fg, bg, fl, text]`) nach DOM-Knoten. Text wird nur als Textknoten eingefügt, nie als HTML. Stile sind CSS-Klassen. |
| `raster.css` | Die Klassen von `html.js` (Palette 0 bis 255, Attribute, invers). Erzeugt von `erzeuge-css.mjs`; nicht von Hand ändern. |

Gebrauch:

```js
import { Empfaenger, zeileZuHtml, RasterFehler } from './raster.js';
const e = new Empfaenger({ deflate: true });       // eine Verbindung
const { auszug, abbild } = e.nimm(new Uint8Array(rahmenVonDerLeitung));
const zeile0 = zeileZuHtml(abbild.lines[0]);        // DocumentFragment; in ein Zeilenelement mit `white-space: pre` einsetzen
```

Das `abbild` hat `nr`, `stand`, `spalten`, `zeilen`, `lines` (je Zeile eine Liste von Läufen), `invers` und `cursor` (`[zeile, spalte, sichtbar]`). `stand` ist eine Zahl, bei Werten über 2^53 - 1 ein `BigInt`.
Bei einem Fehler bleibt das Abbild unverändert. `inflate_fehler` und `rahmen_zu_gross` machen den Empfänger der Verbindung unbrauchbar (RA11); die Seite muss dann neu verbinden.

Die Klassen: `f-N` und `b-N` (N von 0 bis 255; Farbe c der Zeilenform ist N = c - 1), `fett`, `dimm`, `kursiv`, `unterstrichen`, `verdeckt`, `durchgestrichen`. Invers tauscht Schrift und Grund; die Vorgaben heißen dann `f-grund` und `b-vorn`.
Truecolor hat keine endliche Klassenmenge und wird mit `style.setProperty` gesetzt (CSSOM, kein HTML-Text). Ein Lauf im Vorgabestil ist ein bloßer Textknoten; eine leere Zeile ist ein leeres Fragment (die Zeilenhöhe setzt `.raster-zeile`).

## Die Seite

Die Seite ist das, was die Brücke unter `GET /` und `GET /seite.js` ausliefert (Vertrag `seite`, Vertrag `bruecke` Abschnitt 5). Sie zeigt die Sitzungsliste,
das Terminal einer Sitzung, eine Eingabezeile und das Startformular. Sie liest den Ereignisstrom (`GET /strom`) und schickt die Ereignisse der Ansicht
einzeln und der Reihe nach an `POST /eingabe`; ein Start geht an `POST /starten`. Das Token liest sie aus dem Fragment `#t=<token>` und schickt es nur im Kopf `Authorization`.

| Datei | Aufgabe |
|---|---|
| `seite-main.js` | Einstieg: verbindet die Teile, Sitzungsliste, Wahl der Sitzung (`Abo`), Größe, Ablehnungen, Strom, Tastatur und Formulare. |
| `seite-tasten.js` | `deuteTaste`: ein `keydown` wird zu `Text`, `Taste`, einem Fokuswechsel (F6) oder zu nichts (SE1). Ohne DOM. |
| `seite-netz.js` | Token aus dem Fragment, `Sender` (eine Eingabe nach der anderen), `starten`, `oeffneStrom` (Rahmen in beliebigen Stücken). |
| `seite-ansagen.js` | `Ansager`: die höfliche Live-Region; Ausgabe gesammelt, höchstens eine Ansage je 1000 ms, höchstens 5 Zeilen (SE3). |
| `seite-terminal.js` | `TerminalAnsicht`: Zeilen `terminal-zeile-<n>` mit `zeileZuHtml`, Cursor (`data-cursor` und Markierung), Messung mit Klemme 2 Spalten. |
| `seite-start.js` | Wahlfelder aus der Nachricht `Startwahl`, Start-Struktur der Fassung 1 („ohne Kunde“ ist `null`). |
| `seite.html`, `seite.css` | Vorlage der Seite und ihre Stile; `raster.css` kommt davor. |

Bis eine Nachricht `Startwahl` kommt, tragen die Wahlfelder feste Beispiel-Einträge (die des Vertrags `seite`). `Startwahl` ersetzt Harness, Routen und Kunden
(„ohne Kunde“ steht zuletzt). Jede Route trägt ein `modell` (Text oder `null`): Das Feld `Modell` hat dann genau einen Eintrag, das Modell der gewählten Route,
sonst `auto`, und wechselt mit der Route; `POST /starten` trägt genau diesen Eintrag.

**Bündler.** `node buendeln.mjs` schreibt `ausgabe/index.html` (Vorlage mit `raster.css` und `seite.css` eingebettet) und `ausgabe/seite.js` (alle Module ab
`seite-main.js` in einer Datei, jedes in einer eigenen Funktion, ohne `import` zur Laufzeit). Nur `node`, keine Pakete. Steht in einer Ausgabe die Zeichenfolge
`://`, bricht er mit Exit 1 ab. Die zwei Dateien sind eingecheckt; die Brücke bindet sie mit `include_bytes!` ein. Nach jeder Änderung an einem Modul, an
`seite.html` oder an einer CSS-Datei den Bündler laufen lassen und beide Dateien mit committen; `test/buendel.mjs` wird sonst rot.

**Läufer des Vertrags.** Im Ordner des Vertrags `seite` (kopfloser Chromium, Attrappe der Brücke; `playwright-core` und `chrome-headless-shell` müssen schon da sein):

```sh
node laeufer/se_lauf.mjs --seite <worktree>/rust/seite/ausgabe --faelle aussentests/faelle --bericht <datei.json>
```

Läufe über 20 s nicht neben anderen schweren Läufen starten.

## Deflate

Der Browser könnte mit `DecompressionStream('deflate-raw')` entpacken. Das reicht für den Vertrag nicht:
Der Strom meldet kein Ende der Ausgabe eines Rahmens (gemessen mit Node 26: `write()` kehrt zurück, bevor die Ausgabe sichtbar ist), er hat keine Obergrenze je Rahmen (RA11: 16 MiB), und seine Fehler tragen keinen Code.
`inflate.js` ist deshalb ein eigener Entpacker, synchron, mit Kontext (32 KiB Verlauf) über die Aufrufe einer Verbindung und mit denselben Prüfungen wie zlib (zum Beispiel ein Rückverweis vor den Anfang des Stroms ist `inflate_fehler`, "invalid distance too far back").

## Tests

```sh
sh test/lauf.sh           # html.mjs, inflate.mjs, gleichheit.mjs, buendel.mjs; Exit 0 oder 1
RA_VERTRAG=<Vertragsordner> sh test/lauf.sh --neu    # erzeugt die Referenzdatei zusätzlich frisch, außerhalb des Repos (python3)
```

- `test/gleichheit.mjs`: die Rahmen aus `test/daten/gleichheit.json.gz` gehen durch den JS-Empfänger; Bild, Auszug und Fehlercode müssen die von `ra_ref.Verbindung.empfange` sein. Die Datei erzeugt `test/referenz_erzeugen.py` aus der Referenz: die sichtbaren Fälle des Vertrags (`aussentests`) mit Empfangsseite und eine Zufallsprobe mit festem Samen (Abzug, Änderung, Verschiebung, Cursor, invers, Deflate in den Stufen 0 bis 9, beschädigte Rahmen).
- `test/inflate.mjs`: der Entpacker gegen das zlib von Node (Stufen, Strategien, geteilte Eingabe, Kontext über Rahmen, beschädigte Ströme, Grenzen).
- `test/html.mjs`: `zeileZuHtml` (jede SGR-Nummer aus RA1, Text mit `<`, `>`, `&` und Steuerzeichen bleibt Text) und `raster.css`.
- `test/buendel.mjs`: zwei Läufe des Bündlers ergeben byte-gleiche Ausgaben, gleich den eingecheckten in `ausgabe/`, ohne `://`.
- `test/dom_attrappe.mjs`: eine kleine DOM-Attrappe; `innerHTML` und Verwandte werfen.

Eine Eigenheit von Node 26: Ein Lauf mit `process.exit()` blieb gelegentlich beim Beenden hängen (ein Compilerfaden wartet auf eine Speicherbereinigung, der Hauptfaden wartet auf ihn). Die Tests setzen deshalb `process.exitCode`. Wer sie in einer Schleife startet, gibt jedem Lauf eine Frist.

Die Datei `test/daten/gleichheit.json.gz` enthält nur die sichtbaren Fälle des Vertrags und die Zufallsprobe. Die verdeckten Fälle des Vertrags gehören nie ins Repo; gegen sie läuft der Test nur mit `--neu` und `RA_VERTRAG`, mit der Ausgabe außerhalb des Repos.
