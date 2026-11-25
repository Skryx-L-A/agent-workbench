# Vertrag `rasterauszug`

Stand 20.11.2026 (Nachtrag 20.11.2026, Projektentscheid, 20.11.2026: kleinste Größe 2 Spalten in RA5/RA12 und RA10; jede Änderung trägt „(Nachtrag 20.11.2026)“); Stand davor 19.11.2026 (Teil A). Ausführbarer Vertrag des neuen Crates `rasterauszug` (Teil des Plans zur Ansicht,
Abschnitt 5). Grundlage sind die Projektentscheide, 19.11.2026 (Abschnitt 7 des Entwurfs), dort Punkt 2:
Die Zeilenform als SGR-Text steht in diesem Vertrag, und der Rundreise-Test ist Pflicht.
Gebaut ist nur die Python-Referenz. Ein Rust-Bau gilt als vertragstreu, wenn er beide Fallsätze grün besteht (sichtbar und verdeckt).

Quellen: `RA` = `spec/rasterauszug/README.md`, `MOD` = `the previous implementation`, `KOD` = `the previous implementation`,
`VT` = `rust/spec/zustandskern/VERTRAG.md` (Zustandskern, Regeln R4, R5, R13, R14). „(H)“ heißt hergeleitet; „(V)“ heißt im Vertrag festgelegt (Abschnitt 7).
Der Rasterauszug setzt den Gewinner der Messung um: Modell M2, Kodierung A-B, Deflate (`RA`, Abschnitt „Ergebnis“). Die Byte-Form A-B ist die von `KOD` plus drei Felder (R9).

## 1. Schnitt

Der Crate hat vier Aufgaben und keine Ein- oder Ausgabe:

1. **Zeilenform.** Eine Bildschirmzeile ist ein Text mit SGR-Folgen. Der Träger schreibt sie, der Kern reicht sie als Text weiter, der Rasterauszug liest sie in Läufe `[fg, bg, fl, text]` (R1 bis R4).
2. **Auszug.** Je Verbindung (ein Betrachter) und Sitzungsnummer hält der Crate ein Vorbild. Aus den Nachrichten `Abzug` und `Rasteraenderung` des Zustandskerns (`VT` Abschnitt 5) wird ein Rahmen:
   ein Vollbild oder ein Unterschied gegen das Vorbild, mit Verschiebung (M2) (R5 bis R8, R12, R13).
3. **Rahmen.** Der Rahmen hat eine feste Byte-Form (R9). Der Crate liest dieselbe Form wieder und prüft sie streng (R10). Das ist der Dekoder des Betrachters.
4. **Deflate.** Rahmen einer Verbindung laufen durch einen Deflate-Strom mit Kontext je Verbindung (R11).

Nicht im Crate: Sockets, Fäden, Uhren, die Zuordnung Sitzungsname zu Sitzungsnummer (das macht der Dienst), die Entscheidung, wann gesendet wird (das macht der Kern, R5/R14 in `VT`),
Zeichnen. Es gibt keine Systemzeit und keinen Zufall.

Öffentliche API (Vorgabe für den Bau, die Außentests sehen nur das Programm `ra`; Namen der Typen dürfen abweichen, das Verhalten nicht):

```
zeile_lesen(&str) -> Result<Zeile, Fehler>            // R1 bis R4
zeile_schreiben(&Zeile) -> String                     // R3
Verbindung::neu(deflate: bool)
Verbindung::nimm(nr: u8, &serde_json::Value) -> Result<Option<Vec<u8>>, Fehler>   // Kernnachricht -> Rahmen auf der Leitung (R5..R9, R11..R13)
Verbindung::vergessen(nr: u8)                          // R13
Empfang::neu(deflate: bool)
Empfang::nimm(&[u8]) -> Result<(Auszug, &Abbild), Fehler>   // R10, R11
```

## 2. Aufruf des Außentestprogramms `ra`

- `ra <fall.json>` oder `ra < fall.json` (auch `ra -`). Das Programm liest genau einen Fall.
- Exitcode 0: genau ein JSON-Objekt `{"ergebnisse": [...]}` auf stdout, UTF-8, ein Ergebnis je Schritt in der Reihenfolge der Schritte. Schlüsselreihenfolge und Leerraum sind frei; Listenreihenfolge zählt.
- Exitcode 2: Der Fall ist ungültig. stdout bleibt leer; eine Meldung geht nach stderr. Ungültig heißt (V):
  kein JSON, kein Objekt, `schritte` fehlt oder ist keine Liste, ein Schritt ohne Text `art`, eine unbekannte Schritt-`art`, ein Pflichtfeld fehlt oder hat den falschen Typ,
  `nr` keine ganze Zahl von 0 bis 255, `verbindung` unbekannt oder doppelt eingerichtet oder nach `Trennen`, `deflate` kein Wahrheitswert, `hex` kein Kleinbuchstaben-Hex gerader Länge,
  `laeufe` kein Listenmuster `[[ganz, ganz, ganz, text], ...]`, `NaN`, `Infinity` oder Zahlen außerhalb von `f64` (kein gültiges JSON).
  Fehler in der **Nachricht** eines Schritts (falsche Typen darin, Zahlen außerhalb von `i64`) sind dagegen ein Ergebnis `ok: false` (R12), kein Exitcode.
- Grenzen des Programms (wie `zk`): höchstens 4 194 304 Byte Eingabe, Verschachtelung höchstens 128 Ebenen (Grenze von `serde_json`); mehr ist ein ungültiger Fall mit Exitcode 2.
  Keine Eingabe löst eine Panik oder einen Speicherüberlauf aus (R10, R14). Die Referenz prüft die zwei Eingabegrenzen nicht; kein Fall berührt sie.
- Ganze Zahlen liegen im Bereich von `i64`. Eine Zahl mit Bruchteil oder Exponent (auch `3.0`) ist keine ganze Zahl.
- Der Läufer `test_aussen.py` schickt den Fall auf stdin und vergleicht Exitcode und geparstes JSON mit `erwartet/<fall>.json`, mit Ausnahme der Schlüssel `leitung_hex` und `leitung_laenge` (R11).

## 3. Fall

```json
{"name": "…", "regeln": ["RA7"], "datensatz": "A", "beschreibung": "…",
 "schritte": [ {"art": "Verbinden", "verbindung": "v1", "deflate": false}, … ],
 "leitung_erwartung": [{"schritt": 2, "gegen": 1, "hoechstens": 0.25}]}
```

`name`, `regeln`, `datensatz`, `beschreibung` und `leitung_erwartung` wertet das Programm nicht aus (die letzte der Läufer, R11).

### Schritte

| `art` | Felder | Wirkung |
|---|---|---|
| `Zeile` | `sgr` (Text) | Zeilenform lesen und kanonisch schreiben (R1 bis R4) |
| `Laeufe` | `laeufe` (`[[fg, bg, fl, text], …]`) | Läufe prüfen, schreiben, wieder lesen (R3, R4) |
| `Verbinden` | `verbindung` (Text, eindeutig), `deflate` (bool) | richtet eine Verbindung mit eigenem Sender und eigenem Empfänger ein |
| `Nachricht` | `verbindung`, `nr` (0 bis 255), `nachricht` (JSON-Wert, eine Nachricht des Kerns) | Sender erzeugt den Rahmen, Empfänger der Verbindung liest ihn (R5 bis R9, R11 bis R13) |
| `Rahmen` | `verbindung`, `hex` | gibt rohe Leitungsbytes dem Empfänger der Verbindung (R10, R11) |
| `Vergessen` | `verbindung`, `nr` | Sender und Empfänger vergessen die Sitzungsnummer (R13) |
| `Trennen` | `verbindung` | die Verbindung endet; ihr Name wird nicht wieder benutzt, kein Schritt darf sie danach nennen (R13) |

Auf einer Verbindung mit `deflate: true` mischt ein Fall nie `Nachricht` und `Rahmen`: Beide benutzen denselben Kontext des Empfängers (V).

### Ergebnisse (je Schritt ein Objekt)

- Fehler: `{"ok": false, "fehler": "<code>"}`. Die Codes sind genau die der Regeln; die Zustandsänderung ist nach einem Fehler null (R10, R12), abgesehen von Deflate-Kontext (R11).
- `Zeile`: `{"ok": true, "laeufe": [[fg, bg, fl, text], …], "sgr": "<kanonische Schreibweise>", "rundreise": true}`. `rundreise` ist das Ergebnis des Vergleichs `lesen(schreiben(z)) == z`, vom Programm selbst gerechnet.
- `Laeufe`: `{"ok": true, "sgr": "…", "rundreise": true}`.
- `Verbinden`, `Vergessen`, `Trennen`: `{"ok": true}`.
- `Nachricht` ohne Rahmen (nichts zu senden, R6): `{"ok": true, "hex": null, "auszug": null, "abbild": <Abbild des Empfängers der Nummer oder null>, "rundreise": true, "leitung_hex": null, "leitung_laenge": 0}`.
- `Nachricht` mit Rahmen: `{"ok": true, "hex": "<Rahmen vor Deflate>", "auszug": <Auszug>, "abbild": <Abbild>, "rundreise": <bool>, "leitung_hex": "<Bytes auf der Leitung>", "leitung_laenge": <Länge>}`.
  `auszug` ist der Rahmen, vom Empfänger gelesen. `abbild` ist der Zustand des Empfängers danach. `rundreise` heißt: Zeilen, Groesse, invers und Cursor des Abbilds gleichen dem Vorbild des Senders.
- `Rahmen`: `{"ok": true, "hex": "<Bytes nach dem Entpacken>", "auszug": …, "abbild": …}`.
- `Auszug`: `{"nr", "stand", "groesse": [sp, ze] | null, "invers": bool, "schub": [oben, unten, n] | null, "cursor": [zeile, spalte, sichtbar] | null, "zeilen": [[index, "<sgr>"], …]}`.
- `Abbild`: `{"nr", "stand", "groesse": [sp, ze], "invers", "cursor": [zeile, spalte, sichtbar], "zeilen": ["<sgr>", …]}` mit genau `ze` Zeilen in kanonischer Schreibweise.

## 4. Typen

- **Stil** `(fg, bg, fl)`. Farbe: 0 Vorgabe, 1 bis 256 Palette (1 plus Index 0 bis 255; 1 bis 16 sind die benannten Farben, hell ab 9), `0x0100_0000 | rgb` Truecolor. Bits in `fl`: 1 fett, 2 dimm, 4 kursiv, 8 unterstrichen, 16 invers, 32 verdeckt, 64 durchgestrichen (`MOD` Zeilen 14 bis 21).
  Gültig: `fg` und `bg` je 0 bis 256 oder `0x0100_0000` bis `0x01ff_ffff`; `fl` 0 bis 127.
- **Lauf** `(stil, text)`. **Zeile** eine Liste von Läufen. **Kanonische Zeile** (R2): kein Lauf mit leerem Text, keine zwei Nachbarn gleichen Stils, und der letzte Lauf hat nicht den Vorgabestil mit Leerzeichen am Ende.
  Das ist die Form, die `zeile_bauen` in `MOD` erzeugt. Die leere Zeile hat keine Läufe.
- **Erlaubter Text**: Unicode-Skalare ohne C0 (unter U+0020), ohne U+007F und ohne C1 (U+0080 bis U+009F). Die zweite Zellhälfte breiter Zeichen fehlt im Text; kombinierende Zeichen stehen hinter ihrem Grundzeichen im selben Lauf.
  Der Vertrag prüft keine Zellbreiten; eine Zeile darf breiter sein als die Spaltenzahl (V).

## 5. Wirkungen und Fehlercodes

Alle Codes (geschlossene Menge, ein Fall nennt höchstens einen Fehler je Schritt; bei mehreren Fehlern in einem Schritt gilt keine Reihenfolge):

| Gruppe | Codes |
|---|---|
| Zeilenform (R4) | `esc_ungueltig`, `sgr_unvollstaendig`, `sgr_unbekannt`, `sgr_parameter`, `zeichen_ungueltig`, `stil_ungueltig`, `zeile_nicht_kanonisch` |
| Nachricht (R12) | `art_unbekannt`, `nachricht_ungueltig`, `groesse_ungueltig`, `cursor_ungueltig`, `kein_abzug`, `luecke`, `stand_alt`, `zeile_ausserhalb`, `zeile_ungueltig` |
| Rahmen (R10, R11) | `rahmen_abgeschnitten`, `rahmen_rest`, `rahmen_zu_gross`, `art_ungueltig`, `flags_unbekannt`, `rahmen_ungueltig`, `vu_ungueltig`, `utf8_ungueltig`, `stilnr_ungueltig`, `schub_ungueltig`, `zeile_reihenfolge`, `inflate_fehler`, dazu aus R4 `zeichen_ungueltig`, `stil_ungueltig`, `zeile_nicht_kanonisch`, aus R12 `kein_abzug`, `stand_alt`, `zeile_ausserhalb`, `cursor_ungueltig`, `groesse_ungueltig` |

## 6. Regeln

### RA1 Zeilenform lesen

Eine Zeilenform ist Text mit eingestreuten SGR-Folgen `ESC [ Parameter m`. Der Leser hält einen Stil (am Anfang der Vorgabestil) und hängt jedes Textzeichen mit dem jetzigen Stil an.
- Parameter sind durch `;` getrennt. Ein leerer Parameter gilt als 0 (`ESC[m` ist ein Reset, `ESC[;1m` hat zwei Parameter 0 und 1). Ein Parameter hat höchstens drei Ziffern (ASCII), auch führende Nullen (`001` ist 1).
- Die Parameter wirken von links nach rechts auf den Stil: `0` setzt fg, bg und fl zurück. `1`, `2`, `3`, `4`, `7`, `8`, `9` setzen die Bits fett, dimm, kursiv, unterstrichen, invers, verdeckt, durchgestrichen (1, 2, 4, 8, 16, 32, 64).
  `22` löscht fett **und** dimm (Bits 1 und 2), `23`, `24`, `27`, `28`, `29` löschen je ein Bit (kursiv, unterstrichen, invers, verdeckt, durchgestrichen).
- Farben: `30` bis `37` fg gleich 1 plus (Wert minus 30); `40` bis `47` bg ebenso; `90` bis `97` fg gleich 9 plus (Wert minus 90); `100` bis `107` bg ebenso; `39` fg 0; `49` bg 0.
  `38;5;n` und `48;5;n` (n 0 bis 255) setzen fg und bg auf n plus 1; `38;2;r;g;b` und `48;2;r;g;b` (je 0 bis 255) setzen `0x0100_0000 | r<<16 | g<<8 | b`. Danach gilt der nächste Parameter wieder als eigene Anweisung.
- Der Stil gilt über mehrere Folgen hinweg, bis ein `0` oder ein Aus ihn ändert. Eine Folge ohne Text danach hat keine Wirkung auf die Zeile.
- Jeder andere Parameter (zum Beispiel `5`, `6`, `10`, `21`, `53`, `58`, `108`) oder eine fehlende oder zu große Angabe zu `38`/`48` ist `sgr_parameter` (R4).

### RA2 Normalform

Das Ergebnis des Lesers ist immer kanonisch (Abschnitt 4): Nachbarn gleichen Stils sind ein Lauf; Läufe ohne Text entstehen nicht; die Leerzeichen am Ende der Zeile im Vorgabestil entfallen (ein Lauf, der dadurch leer wird, entfällt, und das wiederholt sich
mit dem davor, solange der letzte Lauf den Vorgabestil hat). Leerzeichen mit Stil, Leerzeichen mitten in der Zeile und U+00A0 bleiben. Eine Zeile aus nur Leerzeichen im Vorgabestil, eine leere Zeile und eine Zeile nur aus Folgen sind die leere Zeile.

### RA3 Zeilenform schreiben, Rundreise

Die kanonische Schreibweise einer kanonischen Zeile ist eindeutig:
- Ist der Stil eines Laufs vom Stil davor verschieden (der Stil „davor“ ist am Anfang der Vorgabestil), steht vor dem Text eine Folge: für den Vorgabestil `ESC[0m`, sonst `ESC[` plus die Parameter
  `0`, dann die Attribute in der Reihenfolge 1, 2, 3, 4, 7, 8, 9 (soweit das Bit gesetzt ist), dann die Vordergrundfarbe (nur wenn nicht 0), dann die Hintergrundfarbe (nur wenn nicht 0), mit `;` getrennt, dann `m`.
- Farbe 1 bis 8: `30` plus (Wert minus 1) (bg `40`); 9 bis 16: `90` plus (Wert minus 9) (bg `100`); 17 bis 256: `38;5;(Wert minus 1)` (bg `48;5;…`); Truecolor: `38;2;r;g;b` (bg `48;2;r;g;b`), Zahlen ohne führende Nullen.
- Am Ende der Zeile steht keine Folge. Ein erster Lauf im Vorgabestil hat keine Folge davor.
- **Rundreise (Pflicht):** Für jede kanonische Zeile z gilt `lesen(schreiben(z)) == z`; für jeden gültigen Text s gilt `lesen(schreiben(lesen(s))) == lesen(s)`. Das Programm prüft beides in den Schritten `Zeile` und `Laeufe` und meldet es als `rundreise`.
  Der Bau braucht dazu zusätzlich einen eigenen Test (Abschnitt 11).
- Der Schritt `Laeufe` prüft die Läufe vor dem Schreiben nach R4 (`stil_ungueltig`, `zeichen_ungueltig`, `zeile_nicht_kanonisch`).

### RA4 Fehler der Zeilenform

- `ESC` am Ende des Textes: `sgr_unvollstaendig`. `ESC` mit einem anderen Zeichen als `[` danach: `esc_ungueltig`.
- Nach `ESC[` gelesen werden ASCII-Ziffern und `;`. Endet der Text dort: `sgr_unvollstaendig`. Folgt `m`: die Folge ist zu Ende. Folgt ein anderes Zeichen von `@` bis `~` (zum Beispiel `J`, `H`, `h`): `sgr_unbekannt`.
  Folgt irgendein anderes Zeichen (zum Beispiel `?`, `:`, `+`, Leerzeichen, Nicht-ASCII): `sgr_parameter`. Ein Parameter mit mehr als drei Ziffern oder ein Wert, den RA1 nicht kennt: `sgr_parameter`.
- Text mit einem Zeichen unter U+0020 (außer dem einleitenden `ESC`), U+007F oder U+0080 bis U+009F: `zeichen_ungueltig`. U+00A0 und höher sind erlaubt (auch U+2028, U+200B, U+FEFF).
- Läufe (Schritt `Laeufe`): Ein Stilwert außerhalb von Abschnitt 4: `stil_ungueltig` (auch negative Zahlen). Sonst ein ungültiges Zeichen: `zeichen_ungueltig`. Sonst eine nicht kanonische Zeile: `zeile_nicht_kanonisch`.
- Der Fehler einer Zeile in einer Nachricht des Kerns ist dort immer `zeile_ungueltig`, ohne die Ursache (R12).

### RA5 Abzug

Die Nachricht `Abzug` (`VT` Abschnitt 5: `sitzung`, `stand`, `groesse` [Spalten, Zeilen], `invers`, `zeilen` mit genau so vielen Texten wie Zeilen) macht ein **Vollbild**. Dazu kommt das Feld `cursor` (R8).
- Ablauf: Form prüfen (R12), Groesse prüfen (Spalten 2 bis 1000, Zeilen 1 bis 1000, `groesse_ungueltig`; Nachtrag 20.11.2026: eine Spalte ist ungültig wie 0, Grund in Abschnitt 7 Punkt 13; die Zeilenliste muss die Länge `groesse[1]` haben, sonst `nachricht_ungueltig`), Cursor prüfen (R8), jede Zeile lesen (`zeile_ungueltig`).
- Der Rahmen hat die Marke Groesse (R9), nennt `stand`, `invers`, den Cursor nur wenn die Nachricht einen hat, keinen Schub, und genau die nicht leeren Zeilen in Indexfolge. Leere Zeilen fehlen im Rahmen.
- Das Vorbild der Nummer wird ersetzt (Größe, Zeilen, invers, Cursor, `stand`); ohne Feld `cursor` steht der Cursor auf (0, 0, unsichtbar). Das gilt auch, wenn schon ein Vorbild bestand, gleich welcher Größe und welchen Stands.
- Der Empfänger setzt das Abbild der Nummer auf ein leeres Bild der gemeldeten Größe zurück, wendet dann die Zeilen an. Cursor und invers kommen aus dem Rahmen (fehlt der Cursor im Rahmen: 0, 0, unsichtbar).
- Ein Abzug erzeugt immer einen Rahmen.

### RA6 Änderung und Vorbild

Die Nachricht `Rasteraenderung` (`sitzung`, `von`, `stand`, `invers`, `zeilen`: Objekt Index zu Text, dazu `cursor` nach R8) wird gegen das Vorbild der Nummer gerechnet.
- Das neue Bild ist das Vorbild mit den genannten Zeilen ersetzt. Der Auszug ist der Unterschied nach R7. Zeilen, die dem Vorbild gleichen, kommen nicht in den Rahmen, auch wenn die Nachricht sie nennt.
- Eine Zeile, die leer wird, steht im Rahmen mit 0 Läufen. Ein anderer Stil bei gleichem Text ist eine Änderung.
- Ist nichts zu senden (kein Zeilenunterschied, kein Schub, kein Cursorunterschied nach R8 und `invers` gleich dem Vorbild), entsteht **kein Rahmen** (`hex: null`). Das Vorbild übernimmt dennoch `stand`, damit das nächste `von` passt (V).
- Wird die Nachricht abgelehnt (R12), ändert sich nichts: weder Zeilen, noch Cursor, `invers` oder Stand.
- Der Rahmen einer Änderung hat die Marke Groesse nicht, und sein `stand` ist der der Nachricht.

### RA7 Verschiebung (Modell M2)

Der Unterschied zweier Bilder `alt` (Vorbild) und `neu` mit n Zeilen folgt `diff` in `MOD` (Zeilen 262 bis 335), hier in Worten. Zeilen sind gleich, wenn ihre Läufe gleich sind (die Messung vergleicht einen 64-Bit-Hash; der Vertrag verlangt Gleichheit der Läufe).
- `kosten0` ist die Zahl der Zeilen i mit `alt[i]` ungleich `neu[i]`. `beste` beginnt mit `kosten0`. Es gibt anfangs keine Verschiebung.
- Nur wenn `kosten0` größer als 3 ist, wird gesucht: Für jede Verschiebung `sh` von 1 bis n minus 1 in dieser Reihenfolge: Suche den längsten Lauf aufeinanderfolgender Zeilen i von 0 bis n minus sh minus 1 mit `neu[i]` gleich `alt[i + sh]` und `neu[i]` nicht leer.
  Bei gleich langen Läufen gilt der **erste**. Ist der längste Lauf kürzer als 3, entfällt diese Verschiebung. Sonst sei `a` sein Anfang und `b` sein Ende (ausgeschlossen).
- Die Kosten der Verschiebung sind die Zahl der Zeilen i, bei denen das Modell vom neuen Bild abweicht. Das Modell ist: für i in [a, b) die Zeile `alt[i + sh]`; für i in [b, b + sh) die leere Zeile; sonst `alt[i]`.
- Die Verschiebung wird gewählt, wenn **Kosten plus 2 kleiner als `beste`** ist (mindestens drei Zeilen gespart, strikt). Dann wird `beste` gleich den Kosten gesetzt und die Verschiebung ist `(oben a, unten b + sh, n sh)`. Eine spätere Verschiebung ersetzt die gewählte nur,
  wenn sie dieselbe Bedingung gegen das jetzige `beste` erfüllt (Fund: nicht gegen `kosten0`).
- Danach: Das Modell der gewählten Verschiebung (oder `alt` ohne Verschiebung) wird gegen `neu` gelegt; jede Zeile, die abweicht, kommt in den Rahmen, mit ihrem Index, in Indexfolge.
- Es gibt nur Aufwärtsverschiebung. Abwärtsrollen erkennt M2 nicht; es kommt als geänderte Zeilen.
- Der Empfänger wendet an (R10): Verschiebung, dann die Zeilen. Danach gleicht sein Abbild dem neuen Bild.

### RA8 Cursor und invers

- Das Feld `cursor` der Nachricht ist optional: `[zeile, spalte, sichtbar]` mit ganzen Zahlen und einem Wahrheitswert; `zeile` kleiner als die Zeilenzahl, `spalte` kleiner als die Spaltenzahl der Größe des Vorbilds (bei `Abzug` der neuen Größe),
  sonst `cursor_ungueltig`; ein anderer Aufbau ist `nachricht_ungueltig`. Der Zustandskern führt heute kein Cursorfeld (`VT`); der Crate nimmt es an, sobald der Kern es durchreicht (Abschnitt 10).
- Änderung: Der Rahmen trägt den Cursor, wenn die Nachricht einen nennt und er vom Cursor des Vorbilds abweicht (auch nur `sichtbar`). Ohne Feld oder bei gleichem Wert fehlt er im Rahmen; das Vorbild behält ihn dann.
- `invers`: Jeder Rahmen trägt den Wert der Nachricht (Marke in R9). Ein Wechsel von `invers` allein macht einen Rahmen ohne Zeilen. Ein gleicher Wert allein macht keinen.

### RA9 Rahmen (Byte-Form A-B)

Alle Zahlen sind Varint (`vu`): 7 Bit je Byte, niedrigste Gruppe zuerst, gesetztes Bit 7 heißt „es folgt ein Byte“. Höchstens 10 Byte; der Wert muss in 64 Bit passen; die kürzeste Form ist Pflicht (ein letztes Byte 0 nach einem Fortsetzungsbyte ist ungültig).
`KOD` Zeilen 133 bis 178 ist die Vorlage; neu sind die Marken 4 und 8 und das Feld Groesse.

```
[Art=5] [Sitzung u8] [Flags u8] [Stand vu]
        [wenn Flags & 4: Spalten vu, Zeilen vu]
        [wenn Flags & 1: oben vu, unten vu, n vu]
        [wenn Flags & 2: zeile vu, spalte vu, sichtbar vu (0 oder 1)]
        [Zahl der Stile vu] [je Stil: fg vu, bg vu, fl vu]
        [Zahl der Zeilen vu] [je Zeile: Index vu, Zahl der Läufe vu, je Lauf: Stilnummer vu, Länge in Byte vu, UTF-8-Text]
```
- Flags: 1 Schub, 2 Cursor, 4 Groesse (Vollbild), 8 invers. Die Bits 16 bis 128 müssen 0 sein.
- Die Stiltabelle hält die Stile **in der Reihenfolge des ersten Auftretens** beim Durchgehen der Zeilen und Läufe, ohne Wiederholung. Die Zeilen stehen in aufsteigender Indexfolge.
- Ein Rahmen einer Änderung ohne invers, ohne Groesse und ohne Schub und Cursor gleicht Byte für Byte dem Rahmen des Messläufers (`bin_kodieren`, geprüft an 200 Stichproben, Abschnitt 8).
- Auf der Leitung steht der Rahmen unverändert (ohne Deflate) oder durch Deflate (R11). Länge und Rahmung auf der Leitung sind Sache des Dienstes (nicht Teil des Crates).

### RA10 Dekoder

Der Empfänger liest einen Rahmen streng und ändert sein Abbild nur, wenn der ganze Rahmen gültig ist (alles oder nichts, auch bei einem Fehler am Ende).
- Prüfungen, jede mit eigenem Code: erstes Byte 5 (`art_ungueltig`); Flags ohne Bits 16 bis 128 (`flags_unbekannt`); jedes Varint gültig (`vu_ungueltig`); nicht genug Byte (`rahmen_abgeschnitten`);
  eine Zahl (Stile, Zeilen, Läufe, Textlänge), die größer ist als die Zahl der noch nicht gelesenen Byte (`rahmen_abgeschnitten`, ohne vorher Speicher dafür zu belegen);
  Groesse nur Spalten 2 bis 1000 und Zeilen 1 bis 1000 (`groesse_ungueltig`; Nachtrag 20.11.2026: eine Spalte ist ungültig); Groesse zusammen mit Schub (`rahmen_ungueltig`); Stilwerte nach Abschnitt 4 (`stil_ungueltig`); Stilnummer kleiner als die Zahl der Stile (`stilnr_ungueltig`);
  UTF-8 streng gültig (`utf8_ungueltig`); Text ohne ungültige Zeichen (`zeichen_ungueltig`); jede Zeile kanonisch, also auch kein Lauf mit Länge 0 (`zeile_nicht_kanonisch`); Zeilenindex kleiner als die Zeilenzahl (`zeile_ausserhalb`);
  Zeilenindizes strikt aufsteigend (`zeile_reihenfolge`); Cursor nach R8 und `sichtbar` 0 oder 1 (`cursor_ungueltig`); nach der letzten Zeile keine Byte mehr (`rahmen_rest`).
- Schub gültig, wenn `n` mindestens 1, `oben + n` höchstens `unten` und `unten` höchstens die Zeilenzahl; sonst `schub_ungueltig`. (Der Messläufer übergeht ungültige Schübe still; der Vertrag nicht.)
- Zustand: Ein Rahmen ohne Marke Groesse braucht ein Abbild der Sitzungsnummer (`kein_abzug`) und einen `stand` größer als den des Abbilds (`stand_alt`). Ein Vollbild nimmt jeden Stand an. Jede Nummer hat ihren eigenen Zustand.
- Anwenden: Bei Vollbild ein leeres Abbild der Größe; sonst eine Kopie des Abbilds. Dann der Schub: für i von `oben` bis `unten - n - 1` wird Zeile i zu Zeile i + n, die Zeilen `unten - n` bis `unten - 1` werden leer.
  Dann die Zeilen des Rahmens. Dann Cursor (wenn im Rahmen), `invers` (immer aus dem Rahmen), `stand`.

### RA11 Deflate

Eine Verbindung mit `deflate: true` schickt jeden Rahmen durch einen Deflate-Strom (RFC 7692 Abschnitt 7.2.1, wie in `KOD`):
- Roher Deflate nach RFC 1951 (ohne zlib-Kopf), Fenster höchstens 15 Bit, **ein Kontext je Verbindung** (die Sitzungsnummern einer Verbindung teilen ihn), je Rahmen ein Sync-Flush, die vier Endbytes `00 00 ff ff` entfernt.
- Der Empfänger hängt `00 00 ff ff` an und entpackt mit dem Kontext der Verbindung. Er muss jeden Strom lesen, den zlib in dieser Form erzeugt (Stufen 0 bis 9). Das gilt für Rahmen aus `Rahmen`-Schritten (V).
- Die Bytes dürfen sich zwischen Umsetzungen unterscheiden (zlib, `miniz_oxide`). **Verglichen werden sie nie.** Das Programm schreibt sie als `leitung_hex` auf (Länge `leitung_laenge`); der Läufer prüft sie selbst:
  Ohne Deflate ist `leitung_hex` gleich `hex`. Mit Deflate entpackt der Läufer die Bytes mit zlib (ein Kontext je Verbindung) und muss `hex` erhalten; sie enden nicht auf `00 00 ff ff`.
  Ein Fall kann `leitung_erwartung` tragen: `leitung_laenge(schritt) <= hoechstens * leitung_laenge(gegen)`. Das trägt den Kontext je Verbindung (ein gleicher zweiter Abzug ist höchstens ein Viertel des ersten).
- Entpackt ein Rahmen zu mehr als 16 777 216 Byte, ist das `rahmen_zu_gross` (der Entpacker bricht ab, bevor er mehr Speicher belegt). Ein ungültiger Strom ist `inflate_fehler`. Nach einem dieser beiden Fehler ist der Empfänger der Verbindung unbrauchbar (V).
  Jeder andere Fehler (R10) lässt den Deflate-Kontext des Rahmens verbraucht, das Abbild unverändert.
- Ohne Deflate gilt dieselbe Obergrenze für die Länge eines Rahmens.

### RA12 Prüfung der Nachrichten des Kerns

`Verbindung::nimm` verarbeitet nur `Abzug` und `Rasteraenderung`.
- Kein JSON-Objekt, oder `art` fehlt oder ist kein Text: `nachricht_ungueltig`. Eine andere `art` (Groß- und Kleinschreibung zählt): `art_unbekannt`.
- Pflichtfelder: `Abzug`: `sitzung` (Text), `stand` (ganze Zahl ab 0), `groesse` (Liste aus zwei ganzen Zahlen), `invers` (Wahrheitswert), `zeilen` (Liste von Texten). `Rasteraenderung`: `sitzung` (Text), `von` und `stand` (ganze Zahlen ab 0), `invers`, `zeilen` (Objekt mit Textwerten).
  Fehlt eines oder hat es den falschen Typ (auch Zahlen mit Bruch, Wahrheitswerte als Zahlen, Zahlen außerhalb von `i64`): `nachricht_ungueltig`. Weitere Felder (zum Beispiel `t`) werden übergangen.
  Der Name `sitzung` wird nicht ausgewertet; die Nummer des Aufrufs entscheidet (V).
- Änderung: Ohne Vorbild der Nummer: `kein_abzug`. `von` ungleich dem Stand des Vorbilds: `luecke`. `stand` nicht größer als `von`: `stand_alt`.
  Ein Zeilenschlüssel muss eine Dezimalzahl aus ASCII-Ziffern ohne führende Null (außer `0`) und kleiner als die Zeilenzahl sein, sonst `zeile_ausserhalb` (also auch `-1`, `01`, `1.0`, ` 1`, `+1`, leer, Nicht-ASCII-Ziffern, zu große Zahlen). Ein Fehler beim Lesen einer Zeile: `zeile_ungueltig`.
- Jede Ablehnung ändert nichts (R6).

### RA13 Mehrere Betrachter und Sitzungen

- Das Vorbild gehört dem Betrachter, also der Verbindung, und der Sitzungsnummer. Zwei Verbindungen mit gleicher Nummer teilen nichts. Eine Nummer hat ihren Stand, ihre Größe und ihr Bild. Ein Betrachter, der aufholt (sein `von` liegt weit zurück),
  bekommt einen Unterschied gegen sein eigenes Vorbild, nicht gegen das eines anderen (`VT` R14).
- `Vergessen` löscht Vorbild (Sender) und Abbild (Empfänger) der Nummer; die Nummer 200 ohne Vorbild ist kein Fehler. Danach gilt `kein_abzug` für Änderungen, bis ein Abzug kommt.
- `Trennen`: Die Verbindung und alle ihre Zustände (Vorbilder, Deflate-Kontexte) enden. Eine neue Verbindung beginnt ohne Vorbild; sie braucht einen Abzug, bevor eine Änderung gilt (`VT` R4: nach Trennen nie Änderungen seit einem alten Stand).
- Die Nachrichten zweier Nummern auf einer Verbindung dürfen sich beliebig abwechseln.

### RA14 Grenzen

- Größen: Spalten 2 bis 1000, Zeilen 1 bis 1000 (1000 mal 1000 und 2 mal 1 sind gültig; Nachtrag 20.11.2026: eine Spalte nicht), Zeilen mit vielen Läufen (400 Läufe) und lange Zeilen (20 000 Zeichen) sind gültig. Es gibt keine Grenze der Zeilenbreite gegen die Spaltenzahl.
- Ein Stand reicht bis 2^63 minus 1; im Rahmen bis 2^64 minus 1.
- Keine Eingabe (JSON des Falls, Rahmen, Deflate-Strom) löst eine Panik aus oder belegt Speicher in einer Größe, die sich nur aus einer Zahl in der Eingabe ergibt (R10, R11).
- Exitcodes und Aufbau: Abschnitt 2.

## 7. Festgelegt im Vertrag

Die Messung und der Plan lassen diese Fragen offen; der Vertrag entscheidet klein:

1. Die Zeilenform ist die SGR-Schreibweise von R1 bis R3 mit der Normalform R2. Das war Projektentscheid, 19.11.2026, Punkt 2; der Wortlaut der Schreibweise ist neu.
2. Die Nachricht des Kerns trägt keinen Cursor. Der Rasterauszug nimmt ein optionales Feld `cursor`; bis der Kern es durchreicht, hat der Rahmen keinen Cursor (Abschnitt 8).
3. Der Rahmen A-B des Messläufers wird um drei Felder erweitert: die Marke 4 mit Groesse (macht den Rahmen zum Vollbild), die Marke 8 `invers`. Ein Rahmen ohne diese Marken bleibt byte-gleich dem des Messläufers.
4. Der Auszug trägt kein `von`. Die Lückenfreiheit prüft der Auszug an der Nachricht (`luecke`, `stand_alt`) und der Empfänger am `stand` (`stand_alt`).
5. Ein Abzug erzeugt immer einen Rahmen, ein Vollbild (nicht den Unterschied zum alten Vorbild); er setzt auch das Vorbild einer anderen Größe neu.
6. Ist nichts zu senden, gibt es keinen Rahmen, aber das Vorbild rückt `stand` vor.
7. Der Empfänger ist strenger als der des Messläufers: Ein ungültiger Schub, ein Zeilenindex außerhalb, falsche Reihenfolge und nicht kanonische Zeilen sind Fehler, kein stilles Übergehen.
8. Die Sitzungsnummer (`nr`, ein Byte) vergibt der Dienst; der Crate wertet den Namen der Sitzung nicht aus. **Nachtrag P25 (23.11.2026, Vertrag `dienst` Abschnitt 19):** Der Dienst vergibt eine Nummer neu, wenn ihre Sitzung beendet ist und kein Betrachter sie abonniert. Vorher lässt er jede Verbindung die Nummer vergessen (R13); der erste Rahmen der neuen Sitzung ist deshalb ein Vollbild. Das Rahmenformat ändert sich nicht. Ein Empfänger ordnet die Nummer nach dem Feld `nr` der `Sitzungsliste` zu, nicht nach der Kennung.
9. Ein Betrachter mit mehreren Sitzungen hat einen Deflate-Kontext je Verbindung, nicht je Sitzung.
10. Steuerzeichen im Text und U+0080 bis U+009F sind in der Zeilenform nie erlaubt; die Zeile liefert der Träger bereinigt (Vertrag `traeger`).
11. Deflate-Bytes sind kein Vertragsinhalt; der Läufer prüft sie mit zlib (R11). Die Nutzlast bleibt dieselbe, die der Browser später über WebSocket bekommt.
12. Benannte Abweichungen des Baus von der Referenz (Bau, 19.11., gemessen im Differenztest mit 22 500 eigenen Zufallsfällen; kein Fall des Vertrags ist betroffen):
    - Zwei Fehler in einer Nachricht: Der Bau liest die Zeilenschlüssel von `Rasteraenderung` sortiert, die Referenz in der Reihenfolge der Datei; der gemeldete Code kann deshalb ein anderer der beiden sein (6 von 10 000 Fällen mit absichtlich doppelten Fehlern). Bei mehreren Fehlern in einem Schritt gilt keine Reihenfolge.
    - Mehrfach beschädigte Deflate-Ströme: `miniz_oxide` meldet einen Strom, der für zlib noch „mehr Eingabe braucht“, sofort als `inflate_fehler` statt `rahmen_abgeschnitten` (6 von 7 500). Kürzungen und Einzelbitfehler gültiger Ströme gleichen der Referenz (0 von 7 860 Unterschieden).
    - Zahlen `-0` und `1e999`: Der Bau liest `-0` als Zahl mit Bruchteil (`nachricht_ungueltig`), die Referenz als 0; `1e999` ergibt im Bau Exitcode 2, die Referenz liest `inf`. Fälle mit solchen Zahlen haben keine Erwartung.
    - `miniz_oxide` ist direkte Abhängigkeit des Baus (Entscheidung offen, Antrag 19.11.): Der Strom-Entpacker von `flate2` nimmt einen Rückverweis vor dem Strombeginn an, der Vertrag verlangt `inflate_fehler`.
13. Kleinste Größe (Nachtrag 20.11.2026, Projektentscheid, 20.11.2026): Die kleinste Größe ist 2 Spalten für die ganze Sitzung (Kern, PTY, Raster). Damit keine Stelle der Sitzung eine Spalte annimmt, prüfen **beide** Wege dieselbe Grenze: die Nachricht `Abzug` (RA12/RA5) und der Rahmen mit der Marke Groesse (Flags & 4, RA10);
    eine Größe mit einer Spalte ergibt `groesse_ungueltig`, das Vorbild bleibt (R6). Grund: `alacritty_terminal` 0.26.0 stürzt bei einer Spalte ab oder hängt (`traeger` Abschnitt 7 Punkt 14, Issue 9112); die Ansicht klemmt ihren Wunsch auf mindestens 2 (Vertrag `dienst`/`abbild`).
    Fälle `ra12_groesse` (umgestellt), `ra12_eine_spalte`, `ra12_kleinste_groesse`, `ra10_dek_eine_spalte`; Mutanten `abzug_eine_spalte_ok`, `rahmen_eine_spalte_ok`, `groesse_grenze_verschoben`, `groesse_zeilen_verschoben`.

## 8. Gegenproben (Mutanten)

Jeder Mutant schaltet sich in der Referenz mit `REF_GEGENPROBE=<name>` zu. `gegenprobe.sh` prüft, dass diese Liste und `ra-ref --gegenproben` gleich sind und dass jeder Mutant mindestens einen Fall rot macht.
Stichprobe gegen den Messläufer: `stichprobe.py` (200 Proben, Ergebnis im Bericht).

| Mutant | Regel | Änderung |
|---|---|---|
| `sgr_reset_vergisst_farbe` | RA1 | `0` setzt nur die Attribute zurück, nicht Farben |
| `sgr_aus_22_nur_fett` | RA1 | `22` löscht nur fett, nicht dimm |
| `sgr_truecolor_vertauscht` | RA1 | bei `38;2` und `48;2` sind Rot und Blau vertauscht |
| `sgr_leerer_parameter_ueberspringen` | RA1 | ein `0` in einer Folge mit mehreren Parametern setzt nicht zurück |
| `sgr_unbekannt_still` | RA1 | ein unbekannter Parameter wird übergangen statt `sgr_parameter` |
| `sgr_hell_als_256` | RA3 | Farben 1 bis 16 werden als `38;5;n` geschrieben |
| `sgr_laeufe_nicht_verschmelzen` | RA2 | Nachbarn gleichen Stils bleiben getrennte Läufe |
| `sgr_ende_leerzeichen_bleiben` | RA2 | Leerzeichen im Vorgabestil am Zeilenende bleiben |
| `sgr_ende_leerzeichen_alle` | RA2 | Leerzeichen am Zeilenende fallen auch mit Stil weg |
| `sgr_steuerzeichen_erlaubt` | RA4 | Zeichen unter U+0020 im Text sind erlaubt |
| `sgr_c1_erlaubt` | RA4 | U+0080 bis U+009F im Text sind erlaubt |
| `sgr_schreiben_ohne_reset` | RA3 | die Schreibform lässt das einleitende `0` weg |
| `sgr_ende_reset_anhaengen` | RA3 | am Zeilenende steht ein `ESC[0m` |
| `laeufe_kanon_ungeprueft` | RA4 | `Laeufe` prüft die kanonische Form nicht |
| `stil_ungeprueft` | RA4 | `Laeufe` prüft die Stilwerte nicht |
| `abzug_alle_zeilen` | RA5 | der Abzug listet auch leere Zeilen |
| `abzug_ohne_cursor_reset` | RA5 | ein Abzug ohne Cursor behält den alten Cursor im Vorbild |
| `aenderung_leer_senden` | RA6 | auch ohne Unterschied geht ein Rahmen hinaus |
| `stand_nicht_vorruecken` | RA6 | ohne Rahmen bleibt der Stand des Vorbilds stehen |
| `cursor_immer` | RA8 | der Cursor steht in jedem Rahmen, auch wenn gleich |
| `cursor_aenderung_fehlt` | RA8 | ein geänderter Cursor kommt nie in den Rahmen |
| `invers_nicht_senden` | RA8 | die Marke invers fehlt im Rahmen |
| `invers_allein_kein_rahmen` | RA8 | ein Wechsel von invers allein macht keinen Rahmen |
| `m2_aus` | RA7 | keine Verschiebung wird erkannt |
| `m2_schwelle_kosten0` | RA7 | gesucht wird schon ab drei geänderten Zeilen |
| `m2_lauf_mindestens_zwei` | RA7 | ein Lauf gleicher Zeilen von zwei genügt |
| `m2_gewinn_eins` | RA7 | schon zwei gesparte Zeilen genügen |
| `m2_leere_zeilen_zaehlen` | RA7 | leere Zeilen gelten als gleiche Zeilen eines Laufs |
| `m2_letzter_laengster` | RA7 | bei gleich langen Läufen gilt der letzte |
| `m2_beste_nicht_nachfuehren` | RA7 | `beste` bleibt `kosten0`, eine spätere Verschiebung ersetzt die gewählte |
| `m2_unten_falsch` | RA7 | das Feld unten ist das Ende des Laufs ohne die Verschiebung |
| `m2_gewinn_gleich` | RA7 | Kosten plus 2 gleich `beste` genügt |
| `rahmen_stile_sortiert` | RA9 | die Stiltabelle ist nach Wert sortiert statt nach erstem Auftreten |
| `rahmen_vu_gross_zuerst` | RA9 | Varint mit der höchsten Gruppe zuerst |
| `rahmen_flags_vertauscht` | RA9 | die Marken Schub und Cursor sind vertauscht |
| `rahmen_leere_zeilen_im_abzug` | RA9 | im Vollbild stehen auch leere Zeilen |
| `rahmen_stand_fehlt` | RA9 | der Rahmen enthält den Stand nicht |
| `dek_schub_ungeprueft` | RA10 | ein ungültiger Schub wird angenommen |
| `dek_index_ausserhalb_still` | RA10 | ein Zeilenindex außerhalb wird übergangen |
| `dek_nichtkanonisch_ok` | RA10 | nicht kanonische Zeilen werden angenommen |
| `dek_rest_ok` | RA10 | Byte nach dem Rahmen werden übergangen |
| `dek_nicht_atomar` | RA10 | nach einem Fehler ist ein Teil des Zustands geändert |
| `dek_stand_alt_ok` | RA10 | ein nicht steigender Stand wird angenommen |
| `dek_reset_fehlt` | RA10 | ein Vollbild gleicher Größe lässt die alten Zeilen stehen |
| `dek_vu_nicht_minimal_ok` | RA10 | ein Varint mit unnötigem Endbyte 0 wird angenommen |
| `dek_reihenfolge_ok` | RA10 | Zeilenindizes müssen nicht aufsteigen |
| `dek_cursor_ungeprueft` | RA10 | der Cursor eines Rahmens wird nicht gegen die Größe geprüft |
| `deflate_endung_bleibt` | RA11 | `00 00 ff ff` bleibt am Ende der Leitungsbytes |
| `deflate_voller_flush` | RA11 | Voll-Flush statt Sync-Flush (der Kontext geht verloren) |
| `deflate_neu_je_nachricht` | RA11 | ein neuer Kontext je Nachricht |
| `deflate_grenze_aus` | RA11 | keine Obergrenze beim Entpacken |
| `von_nicht_geprueft` | RA12 | `von` wird nicht gegen den Stand des Vorbilds geprüft |
| `stand_nicht_steigend_ok` | RA12 | `stand` nicht größer als `von` wird angenommen |
| `zeile_fuehrende_null_ok` | RA12 | Zeilenschlüssel mit führender Null werden angenommen |
| `abzug_groesse_1001_ok` | RA12 | die Groesse 1001 wird angenommen |
| `abzug_eine_spalte_ok` | RA12 | ein `Abzug` mit einer Spalte wird angenommen |
| `rahmen_eine_spalte_ok` | RA10 | ein Rahmen mit der Marke Groesse und einer Spalte wird angenommen |
| `groesse_grenze_verschoben` | RA12, RA10 | die kleinste Größe ist 3 Spalten (2 Spalten werden abgelehnt), in `Abzug` und Rahmen |
| `groesse_zeilen_verschoben` | RA12, RA10 | die kleinste Größe ist 2 Zeilen (eine Zeile wird abgelehnt), in `Abzug` und Rahmen |
| `zeilen_laenge_ungeprueft` | RA12 | die Zeilenzahl muss nicht zur Groesse passen |
| `vorbild_geteilt` | RA13 | alle Verbindungen teilen ein Vorbild |
| `vergessen_ignoriert` | RA13 | `Vergessen` tut nichts |
| `trennen_behaelt_vorbild` | RA13 | eine neue Verbindung erbt die Vorbilder der getrennten |

## 9. Dateien

- `referenz/ra_ref.py`, Starter `referenz/ra-ref` (ruft `python3 -I -B`; anderes Python über `RA_PYTHON`).
- `faelle_bauen.py`: schreibt beide Sätze; jede Erwartung entsteht aus der Referenz (die handgebauten Rahmen der Dekoder-Fälle entstehen mit einem eigenen kleinen Zusammensetzer im Skript, die Deflate-Rahmen mit zlib).
- `aussentests/{faelle,erwartet}/` (sichtbar, Datensatz A) und `verdeckt/{faelle,erwartet}/` (Datensatz B: andere Namen, Sitzungsnummern, Texte, Größen und Samen).
- `test_aussen.py`: Läufer mit `RA_BIN` und `RA_FAELLE`. `gegenprobe.sh`: Grundlauf und Mutanten. `stichprobe.py`: Vergleich der Referenz mit dem Code des Messläufers; `werkzeuge/vergleich/` ist das kleine Rust-Programm dazu (zu den Dateien `src/modell.rs` und `src/kodierung.rs` aus `the previous implementation` unverändert hineinkopieren und `cargo build --release` ausführen).

## 10. Offene Punkte

- **Cursor im Kern.** Der Zustandskern (`VT`, Abzug und Rasteraenderung) kennt keinen Cursor; die Naht der Ansicht (`Raster{zeilen, cursor, invers, groesse}`) braucht ihn. Antrag: der Kern führt `cursor` je Sitzung (Ereignis `Ausgabe` mit Feld) und reicht ihn in beiden Nachrichten durch.
- **Obergrenze der Kosten von `diff`.** `diff` ist O(n²) je Rahmen (n Zeilen); bei 1000 Zeilen sind das etwa 10^6 Vergleiche je Verschiebung und 10^9 je Rahmen. Die Messung galt n gleich 60. Der Bau soll die Suche begrenzen dürfen,
  ohne das Ergebnis zu ändern (zum Beispiel über Hashes der Zeilen), sonst ist n gleich 1000 nicht ohne Weiteres bedienbar. Der Vertrag fordert nur das Ergebnis, keine Laufzeit.
- **Laufzeit gegen die Messung (P4).** Der Vertrag misst keine Zeit.

## 11. Pflichten des Baus außerhalb der Außentests

- Ein Rundreise-Test (Zeilenform lesen, schreiben, lesen) auf mindestens 10 000 Zuständen mit festem Samen (Plan zur Ansicht).
- Ein Rundreise-Test Kodierer–Dekoder auf mindestens 10 000 Zuständen mit festem Samen; ein Byte-Vergleich gegen den Messläufer ist nicht Pflicht, die Außentests tragen ihn.
- Kein `unsafe`, keine Panik auf Eingaben, `cargo deny` nach `rust/deny.toml`, `flate2` mit Standardweg `miniz_oxide`.
- **Pflichten des Baus (Nachtrag 20.11.2026, Auflagen, 20.11.2026):** `Abzug` und der Rahmen mit Groesse prüfen Spalten 2 bis 1000 und Zeilen 1 bis 1000 (`groesse_ungueltig`). Der Kodierer baut nie einen Rahmen mit einer Spalte. Die Hinweise zur Entfernung von `unicode-width` und der Auflage (c) stehen im Vertrag `traeger` (Abschnitt 11).
