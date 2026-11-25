# Vertrag `traeger`

Stand 20.11.2026 (Teil A); Nachtrag TR15 20.11.: die Umgebung des Kindes ist genau die des Falls (auch ohne `SHELL`), ein `start.cwd`, das kein Ordner ist, ist ein Fehler des Starts, und `Tasten` blockiert nie (Schlange mit fester Grenze) (TR12, TR15, Abschnitt 2, 3, 7 Punkt 19, 8); jede Änderung dieses Nachtrags trägt „(TR15)“. Nachtrag A109 20.11.: der Träger beantwortet die Cursorabfrage `ESC[6n` selbst (TR14, TR10, TR12, Abschnitt 7 Punkt 18, Abschnitt 8, Abschnitt 10); jede Änderung dieses Nachtrags trägt „(A109)“. Nachtrag 20.11.2026 (Projektentscheid, 20.11.2026): kleinste Größe 2 Spalten (TR3, TR13, Abschnitt 7 Punkt 14 und 15), Schritt `Groesse` mit `versuch`, Cursor bei schrumpfenden Spalten (TR3, Punkt 17); jede Änderung dieses Nachtrags trägt „(Nachtrag 20.11.2026)“. Nachträge 19.11.2026 (Cursor-Nachtrag): Cursor in `Ausgabe` (TR2, TR3), Fälle für Variantenwahl und
Zeichenverbindung (TR11), Signal (TR12), eine Spalte (Abschnitt 7 Punkt 14; Nachtrag 20.11.2026: ersetzt, eine Spalte ist ungültig), Antworten namentlich (TR10), kleinste Größe (TR13). Jede Änderung dieses Nachtrags trägt „(Nachtrag 19.11.2026)“ oder „Cursor-Nachtrag“ im Text. Ausführbarer Vertrag des neuen Crates `traeger` (Teil des Plans zur Ansicht, Abschnitt 5).
Grundlage: die Messung des Trägers (`spec/traeger/`, Ergebnis), der Zustandskern (`rust/spec/zustandskern/VERTRAG.md`, nachfolgend `VT`),
der Vertrag `rasterauszug` (`rust/spec/rasterauszug/VERTRAG.md`, nachfolgend `RA`, dort Zeilenform R1 bis R4) und die Projektentscheide (Entwurf Abschnitt 7; Messplan Abschnitt 11: Rücklauf als Einstellung, Vorgabe 2000, Emoji-Breite nach Unicode mit dem Sonderfall Hautton).
„(H)“ heißt hergeleitet; „(V)“ heißt im Vertrag festgelegt (Abschnitt 7).

Die **Referenz** ist ein kleines Rust-Programm (`referenz/tr-ref/`), das auf `alacritty_terminal` 0.26.0 und `portable-pty` 0.9.0 aufsetzt, wie die Probe `the previous implementation`. Jede Erwartung der Fälle entsteht aus ihm.
Das ist so gewollt: Der Träger verspricht das Verhalten von `alacritty_terminal` und dazu die Zusätze dieses Vertrags. Ein Rust-Bau gilt als vertragstreu, wenn er beide Fallsätze grün besteht (sichtbar und verdeckt).
Wo `alacritty_terminal` etwas tut, das der Vertrag nicht nennt (zum Beispiel die Antworten auf Geräteabfragen), gilt sein Verhalten mit der gepinnten Fassung `=0.26.0`.

## 1. Schnitt

Der Träger hält für **eine Sitzung** eine PTY (`portable-pty`, ein Kind) und ein Raster (`alacritty_terminal`, Rücklauf als Einstellung). Er macht aus den Bytes des Kindes die Ereignisse des Kerns (`VT` Abschnitt 4, Zeilen „Träger“),
und aus den Wirkungen des Kerns (`pty`-Bytes und `groessen`, `VT` Abschnitt 5) Bytes und Größe für das Kind. Er kennt keine Zeit, keinen Betrachter, keine Einstellung außer dem Rücklauf, keinen Sitzungsnamen und keinen Socket.

Zwei Teile, beide durch das Programm `tr` geprüft:
- **Stromteil** (TR1 bis TR11, TR13): Bytes hinein, Ereignisse und Raster heraus. Kein Kind, kein Prozess; reine Funktion der Bytes (und Größenwechsel).
- **Kindteil** (TR12): ein echtes Kind in einer PTY, feste Kindprogramme.

Öffentliche API (Vorgabe für den Bau; die Außentests sehen nur das Programm `tr`):

```
Raster::neu(spalten, zeilen, ruecklauf) -> Result<Raster, GroesseUngueltig>   // (Nachtrag 20.11.) Spalten 2 bis 1000, Zeilen 1 bis 1000; eine Spalte ist eine Ablehnung, nie ein Raster
Raster::bytes(&mut self, &[u8]) -> Vec<Ereignis>      // ein Stück Bytes des Kindes: Ereignisse in der Reihenfolge von TR2
Raster::groesse(&mut self, spalten, zeilen) -> Result<Vec<Ereignis>, GroesseUngueltig>   // Wirkung `groessen` des Kerns (TR3); (Nachtrag 20.11.) abgelehnt: nichts ändert sich
Raster::antworten(&mut self) -> Vec<u8>                // Bytes, die der Träger an das Kind zurückschreiben muss (TR10)
Raster::zeilen(&self) / ruecklauf_zahl(&self) / ...    // Zustand für Prüfungen
Kind::starten(Start{argv, env, cwd, spalten, zeilen, ruecklauf}) // PTY mit Lesefaden; liefert Ereignisse einschließlich Kindende
Kind::schreibe(&[u8]); Kind::groesse(spalten, zeilen)  // Wirkungen `pty` und `groessen`; (Nachtrag 20.11.) abgelehnte Größe: die PTY behält ihre alte Größe
```

Ereignisse tragen im Programm `tr` **kein** `sitzung` und kein `t`; der Dienst setzt beides ein, bevor er sie dem Kern gibt.

## 2. Aufruf des Außentestprogramms `tr`

- `tr <fall.json>` oder `tr < fall.json` (auch `tr -`). Das Programm liest genau einen Fall.
- Exitcode 0: genau ein JSON-Objekt auf stdout, UTF-8: `{"ergebnisse": [...], "ende": {...}}`. Schlüsselreihenfolge und Leerraum sind frei; Listenreihenfolge zählt.
- Exitcode 2: Der Fall ist ungültig. stdout bleibt leer; eine Meldung geht nach stderr. Ungültig heißt (V): kein JSON, kein Objekt, `art` weder `strom` noch `kind`, `groesse` keine Liste aus zwei ganzen Zahlen (Spalten 2 bis 1000, Zeilen 1 bis 1000; Nachtrag 20.11.2026: eine Spalte ist ungültig),
  `schritte` fehlt oder ist keine Liste, ein Schritt ohne Text `art` oder mit unbekannter `art`, ein Schritt `Groesse` mit Größe außerhalb der Grenzen und ohne `versuch` (Nachtrag 20.11.2026), `versuch` kein Wahrheitswert, `hex` kein Kleinbuchstaben-Hex gerader Länge, ein Pflichtfeld fehlt oder hat den falschen Typ, bei `kind` `start` ohne `kind` oder `env`, (TR15) ein `start.cwd`, das kein Text ist oder kein Ordner nennt, ein Schritt `Tasten` mit `mal` außerhalb von 1 bis 4 194 304 oder mit mehr als 4 194 304 Byte (`hex` mal `mal`).
  Höchstens 4 194 304 Byte Eingabe, Verschachtelung höchstens 128 Ebenen; mehr ist ein ungültiger Fall.
- `tr --kind <name> [argumente]` führt ein **festes Kindprogramm** aus (Abschnitt 6, TR12). Das Programm `tr` startet sich dafür selbst (`std::env::current_exe()`) in der PTY. Der Bau liefert diese Kinder in seinem eigenen `tr` mit.
- Keine Eingabe löst eine Panik aus. Der Läufer `test_aussen.py` schickt den Fall auf stdin und vergleicht Exitcode und geparstes JSON mit `erwartet/<fall>.json`.
- Alle Daten sind erfunden. Der Läufer setzt im Fall nur die Umgebung des Kindes; die Fälle tragen keine Pfade, außer (TR15) dem optionalen `start.cwd` des Kindteils (Vorgabe `/`).

## 3. Fall

```json
{"name": "…", "regeln": ["TR2"], "datensatz": "A", "beschreibung": "…",
 "art": "strom", "groesse": [30, 8], "ruecklauf": 2000,
 "schritte": [ {"art": "Text", "text": "…"}, … ]}
```
```json
{"art": "kind", "groesse": [30, 8], "ruecklauf": 2000,
 "start": {"kind": "ausgabe", "argv": ["<hex>"], "env": {"TERM": "xterm-256color", "PATH": "/usr/bin:/bin"}},
 "schritte": [ {"art": "Warte", "bis": {"kindende": true}}, {"art": "Zustand"} ]}
```
- (TR16) Ebenso wertet das Programm `system` und `last` nicht aus; sie gehören dem Läufer: `"system": ["linux", "macos", "windows"]` (die Systeme, auf denen der Fall gilt; fehlt das Feld, gilt er überall) und `"last": <1 bis 64>` (so viele Lastprozesse hält der Läufer am Laufen, solange der Fall läuft; nur mit `TR_LAST=1`, sonst gilt der Fall als nicht anwendbar).
- `name`, `regeln`, `datensatz`, `beschreibung` wertet das Programm nicht aus. `ruecklauf` (ganze Zahl 0 bis 100 000, Vorgabe **2000**) ist die Einstellung des Rücklaufs (TR4).
- Anfangsstand: ein leeres Raster der Größe, Cursor oben links und sichtbar (`[0, 0, true]`), `stand` 0, der Kern kennt für jede Zeile den Text `""` und den Cursor `[0, 0, true]` (Nachtrag 19.11.2026: Der Dienst gibt dem Kern
  diesen Anfangscursor als `anfang.sitzungen[].cursor` mit, `VT` Abschnitt 3 und R15).

### Schritte des Stromteils

| `art` | Felder | Wirkung |
|---|---|---|
| `Text` | `text` | die UTF-8-Bytes des Textes als ein Stück Bytes des Kindes (TR2) |
| `Bytes` | `hex` | dieselben Bytes als Kleinbuchstaben-Hex (auch halbe UTF-8-Zeichen) |
| `Groesse` | `spalten` (2 bis 1000), `zeilen` (1 bis 1000), `versuch` (Wahrheitswert, Vorgabe false; Nachtrag 20.11.2026) | die Wirkung `groessen` des Kerns (TR3). Außerhalb der Grenzen ist der Fall ungültig (Exitcode 2); mit `versuch: true` ist er gültig und der Schritt wird abgelehnt (siehe unten) |
| `Zustand` | – | gibt zusätzlich den Zustand aus |

### Schritte des Kindteils

| `art` | Felder | Wirkung |
|---|---|---|
| `Warte` | `bis`: `{"text": "…"}`, `{"kindende": true}` oder `{"cursor": [zeile, spalte, sichtbar]}` (Nachtrag 21.11.2026) | liest die Ausgabe des Kindes, bis eine Zeile des Bildes den Text enthält (Klartext ohne Stil), das Kind zu Ende ist (Ende der Ausgabe, Kindende gemeldet) oder der Zustand diesen Cursor zeigt (drei Werte: zwei ganze Zahlen ab 0 und ein Wahrheitswert, sonst ist der Fall ungültig, Exitcode 2); Frist 20 s, dann `ok: false`, `fehler: "zeit"`; endet das Kind, ohne dass das Ziel erreicht ist, `fehler: "nie"`. Die Bedingungen gelten alternativ (die erste, die zutrifft, beendet das Warten). **Wozu `cursor`:** Der Text einer Zeile steht im Bild, bevor der Zeilenvorschub dahinter gelesen ist; ein Fall, der den Cursor der fertigen Zeile `BEREIT` braucht (`[1, 0, true]`), wartet auf den Cursor und nicht allein auf den Text |
| `Tasten` | `hex`, `mal` (ganze Zahl 1 bis 4 194 304, Vorgabe 1; TR15) | schreibt die Bytes (bei `mal` die Bytes von `hex` so oft hintereinander, in **einem** Schreiben) unverändert an die PTY (Wirkung `pty` des Kerns); kehrt sofort zurück (TR15). Ist die Schlange voll: `"ok": false`, `"fehler": "schlange_voll"` |
| `Groesse` | `spalten` (2 bis 1000), `zeilen` (1 bis 1000), `versuch` | setzt die Größe der PTY und des Rasters; Grenzen und `versuch` wie im Stromteil |
| `Zustand` | – | gibt zusätzlich den Zustand aus |

Das Kind startet vor dem ersten Schritt, im Ordner `start.cwd` (optional, ein Text, Vorgabe `/`; TR15). Am Ende des Falls beendet das Programm das Kind, falls es noch läuft.
Die Antworten des Trägers (TR10, TR14) schreibt das Programm nach jedem gelesenen Stück sofort an die PTY, bevor es weiterliest (A109; bis zu diesem Nachtrag schrieb die Referenz im Kindteil keine Antwort, kein älterer Fall des Kindteils fragt etwas ab).

### Ergebnisse

- Stromteil, je Schritt: `{"ereignisse": [...], "pty_antworten": "<hex>"}`, bei `Zustand` dazu `"zustand": {...}`. `ereignisse` sind die Ereignisse dieses Schritts in Reihenfolge; `pty_antworten` sind die Bytes (Hex, leer wenn keine), die der Träger bei diesem Schritt an das Kind zurückschreibt (TR10).
- Kindteil, je Schritt: `{"ok": true|false, "ereignisse": [...]}` (+ `fehler`, + `zustand`). **Ereignisse der Art `Ausgabe` stehen dort nicht**: Ihre Zahl und Stückelung hängt von den Lesepuffern ab (Abschnitt 6, TR12); alle anderen Ereignisse bleiben mit ihrer Reihenfolge.
- **Versuch (Nachtrag 20.11.2026; Projektentscheid, 20.11.2026: JA, das Feld bleibt, optional).** Ein Schritt `Groesse` mit `versuch: true` nimmt für `spalten` und `zeilen` ganze Zahlen von 0 bis 100 000 (sonst ist der Fall ungültig). Liegen beide in den Grenzen (Spalten 2 bis 1000, Zeilen 1 bis 1000), wirkt der Schritt wie ohne `versuch`; sonst wird er
  **abgelehnt**: Raster, Cursor, Größe, Rücklauf, `stand` und im Kindteil auch die Größe der PTY bleiben, es geht kein Ereignis hinaus. Das Ergebnis des Schritts trägt dann `"ok": false` und `"fehler": "groesse_ungueltig"` (der Kindteil trägt `ok` ohnehin; der Stromteil trägt `ok` nur bei Schritten mit `versuch`, bei angenommener Größe `"ok": true`).
  Das ist der einzige Weg, auf dem die Abnahme eine abgelehnte Größe von außen mitten im Fall sieht (der Bau liefert sie als `Err(GroesseUngueltig)`); ohne `versuch` endet der ganze Fall mit Exitcode 2 und kein Zustand entsteht.
- `ende` (immer): der Zustand am Ende.
- **Zustand**: `{"zeilen": ["<Zeilenform>", …] (genau eine je Zeile des Rasters), "cursor": [zeile, spalte, sichtbar], "groesse": [spalten, zeilen], "ruecklauf": {"zahl": n, "erste": <Zeilenform oder null>, "letzte": <…>}, "abbild_gleich": bool, "stand": n}`.
  `ruecklauf.zahl` ist die Zahl der Zeilen im Rücklauf, `erste` die älteste, `letzte` die jüngste. `abbild_gleich` heißt: Wenn man alle Ausgabe-Ereignisse bisher auf `""` und den Cursor `[0, 0, true]` anwendet (nach TR2 und TR3, mit dem Kürzen und Füllen der Zeilen und dem Klemmen des Cursors durch den Kern bei Größenänderung), erhält man genau `zeilen` und `cursor`
  (Nachtrag 19.11.2026: früher nur die Zeilen).
  Im Kindteil fehlt `stand` (er hängt von der Stückelung ab); dort steht `ausgaben_nach_kindende` (Zahl der Ausgabe-Ereignisse nach dem Kindende, erwartet 0).

## 4. Ereignisse hinaus

| `art` | Felder (ohne `sitzung`, `t`) | Nach |
|---|---|---|
| `Ausgabe` | `stand`, `zeilen` (Objekt Zeilenindex als Dezimalzahl zu Zeilenform, kann leer sein), `cursor` (`[zeile, spalte, sichtbar]`, immer; Nachtrag 19.11.2026) | TR2 |
| `Modus` | `invers` (bool) und `stand`; oder `bracketed_paste` (bool) | TR5 |
| `SyncBeginn`, `SyncEnde` | – | TR6 |
| `Zwischenablage` | `daten_b64` (Text) oder `lesen: true` | TR7 |
| `Titel` | `text` | TR8 |
| `GroessenFolge` | `spalten`, `zeilen` | TR9 |
| `Kindende` | `code` | TR12 |

Das ist der Satz der Träger-Ereignisse von `VT` Abschnitt 4 ohne `Socket`.

## 5. Typen

`Zeilenform` ist die kanonische SGR-Schreibweise nach `RA` R3, `Zeile` im Sinn von `RA` Abschnitt 4. Jede Zeile in jedem Ergebnis dieses Vertrags muss `rasterauszug::zeile_lesen` lesen und `zeile_schreiben` unverändert zurückschreiben
(das prüft `faelle_bauen.py` mit der Referenz des anderen Vertrags: `ra-ref` auf alle 223 verschiedenen Zeilen der Erwartungen).

## 6. Regeln

### TR1 Zeilenform des Rasters

Jede Zeile des sichtbaren Rasters (und jede des Rücklaufs) wird zu einer Zeile aus Läufen nach dem Vorbild von `zeile_bauen` in `the previous implementation` und dann in die Zeilenform von `RA` R3 geschrieben:
- Je Zelle von links nach rechts, mit Stil `(fg, bg, fl)`. Aufeinanderfolgende Zellen gleichen Stils sind ein Lauf.
- Farben: benannte Farben 0 bis 15 (also 30 bis 37, 90 bis 97 und die gleichen Indexfarben) sind 1 bis 16; Indexfarbe `i` ist `i + 1`; Truecolor ist `0x0100_0000 | rgb`; die Vorgabefarben (und alle anderen benannten Farben) sind 0. So wird `38;5;9` und `91` dieselbe Farbe 10.
- Attribute `fl`: fett 1, dimm 2, kursiv 4, **jede** Unterstreichung (einfach, doppelt, gewellt, punktiert, gestrichelt) 8, invers 16, verdeckt 32, durchgestrichen 64.
- Die zweite Hälfte eines breiten Zeichens fehlt im Text (auch der Platzhalter am Zeilenende, wenn ein breites Zeichen umbrach). Kombinierende (breitenlose) Zeichen stehen hinter ihrem Grundzeichen im selben Lauf.
- Am Ende der Zeile fallen Leerzeichen im Vorgabestil weg (`RA` R2); Leerzeichen mit Stil, am Anfang und mitten in der Zeile bleiben. Löschen mit gesetzter Hintergrundfarbe (`ESC[44m ESC[2J`) macht Zellen mit Hintergrund: Sie bleiben als Lauf aus Leerzeichen.
- Zeichen von U+0080 bis U+009F stehen nie im Text (sie wären nach `RA` R4 ungültig); ein solches Zeichen im Raster schreibt der Träger als U+FFFD.
- Der Cursor steht nicht in der Zeilenform.

### TR2 Ausgabe-Ereignisse

Der Träger meldet nur die Änderung gegenüber dem, was der Kern schon hat. Seit dem Nachtrag 19.11.2026 (Cursor-Nachtrag, Projektentscheid, 19.11.2026) gehört der Cursor dazu: Der Träger vergleicht jede Zeile **und den Cursor** mit der letzten Meldung.
- Der Kern hat am Anfang für jede Zeile `""`. Nach jedem **Stück Bytes** (jeder Schritt `Text` oder `Bytes`, im Betrieb jeder Lesevorgang an der PTY) schreibt der Träger die Zeilenform aller Zeilen und vergleicht jede mit dem, was er dem Kern zuletzt für diese Zeile gemeldet hat.
  Weicht mindestens eine Zeile **oder der Cursor** ab, folgt **ein** Ereignis `Ausgabe` mit genau den abweichenden Zeilen (Schlüssel: Zeilenindex ab 0 als Dezimalzahl), `stand` und `cursor`. Weicht nichts ab, folgt kein Ereignis.
  Ein Cursorschritt ohne Zeilenänderung ist also eine `Ausgabe` mit leerem `zeilen` und steigendem `stand`.
- `stand` beginnt bei 1 und steigt mit jedem Ereignis, das einen Stand trägt, um genau 1: `Ausgabe` und `Modus` mit `invers` zählen gemeinsam (`VT` R4, R13).
- **Reihenfolge innerhalb eines Stücks:** Der Träger geht die Bytes durch. Vor **jedem** Ereignis, das keine Ausgabe ist (Modus, SyncBeginn, SyncEnde, Zwischenablage, Titel, GroessenFolge, Kindende), meldet er zuerst die bis zum letzten Byte dieser Folge geänderten Zeilen als `Ausgabe`
  (wenn welche abweichen), dann das Ereignis. Am Ende des Stücks folgt die Ausgabe der Zeilen, die danach noch abweichen. Eine Folge wirkt auf das Raster und meldet ihr Ereignis erst mit ihrem letzten Byte; ein Stück, das mitten in einer Folge endet, meldet nichts dazu, die Folge wirkt im nächsten Stück.
  Ein leeres Stück meldet nichts.
- **UTF-8 über Stückgrenzen (Nachtrag 20.11.2026, Projektentscheid, 20.11.2026: im selben Nachtrag).** Ein unvollständiges UTF-8-Zeichen am Ende eines Stücks wird bis zum nächsten Stück zurückgehalten, bei **jeder** Spaltenzahl; Bytes, die nie ein gültiges Zeichen werden können, gehen unverändert weiter.
  Ohne das rechnet `vte` bei weiterem Text im nächsten Stück verschieden (kleinster Fall: Raster 6 mal 2, Stück 1 `65 cc`, Stück 2 `81 65 cc 81` ergibt `éé` mit Cursor auf Spalte 2, nicht `é́`). Fall `tr2_utf8_stueckgrenze`; Mutant `utf8_stueck_nicht_gehalten`.
- Das Ergebnis hängt nicht von der Stückelung ab, außer durch die Zahl der `Ausgabe`-Ereignisse: Dieselben Bytes in einem oder in vielen Stücken geben dasselbe Raster und dieselben anderen Ereignisse in derselben Reihenfolge.
- **Cursor (Nachtrag 19.11.2026).** `cursor` ist `[zeile, spalte, sichtbar]`, immer in **jeder** `Ausgabe`, auch wenn er sich nicht geändert hat (der Kern führt ihn je Sitzung, `VT` R15). Der Wert, gemessen an der Referenz (`alacritty_terminal` 0.26.0, Punkt des Cursors im aktiven Raster):
  - `zeile` und `spalte` zählen ab 0 im sichtbaren Schirm (nie im Rücklauf; laufen Zeilen oben hinaus, bleibt die Zeile des Cursors am Rand des Schirms und ändert sich nicht durch den Rücklauf);
  - **ausstehender Umbruch:** Nach einem Zeichen in der letzten Spalte steht der Cursor auf der letzten Spalte (`spalten − 1`), erst das nächste Zeichen bricht um und setzt ihn in die nächste Zeile; mit DECAWM aus bleibt er am Rand;
  - **breite Zeichen:** Ein Zeichen der Breite 2 schiebt den Cursor um zwei Spalten; am Zeilenende, wo es nicht mehr passt, bricht es um (der Cursor steht danach in der nächsten Zeile auf Spalte 2); füllt es die letzten zwei Spalten, steht der Cursor auf der letzten Spalte (ausstehender Umbruch). Kombinierende und Hauttonzeichen (TR11) schieben ihn nicht;
  - **Alternativschirm** (Modus 1049, 47, 1047): Es gilt der Cursor des gerade angezeigten Schirms; beim Verlassen kehrt der Cursor des Hauptschirms zurück (`alacritty_terminal` setzt beim Betreten den Cursor des Alternativschirms auf den des Hauptschirms);
  - `sichtbar` ist DECTCEM (`ESC[?25h` wahr, `ESC[?25l` falsch, Vorgabe wahr). Die Form des Cursors (DECSCUSR) gehört nicht dazu;
  - **Größenänderung (TR3)** und **Rücklauf:** siehe TR3.
  Der Cursor steht auch in der `Ausgabe`, die vor einem anderen Ereignis geht (Reihenfolge wie oben): Hat sich bis dahin nur er geändert, ist es eine `Ausgabe` mit leerem `zeilen`.
  Anfangs gilt `[0, 0, true]` als gemeldet: Ein Stück ohne Änderung (zum Beispiel nur `ESC[?5h`) meldet keine `Ausgabe`.
- Der Träger rechnet bei jedem Stück nur, was sich geändert hat; die Laufzeit ist kein Teil des Vertrags (H: der Bau muss nicht jede Zeile neu schreiben).

### TR3 Größe

Die Wirkung `groessen` des Kerns (`VT` R3) ändert die Größe des Rasters und der PTY. Der Träger ändert seine Größe nie von selbst (TR9).
- Das Raster ändert sich so, wie `alacritty_terminal::Term::resize` es tut (Zeilen gehen beim Verkleinern in den Rücklauf, umbrochene Zeilen werden verbunden oder neu umbrochen).
- Der Kern kürzt bei einer Größenänderung seine Zeilen oder füllt sie mit `""` auf die neue Zeilenzahl (`VT` R3). Der Träger vergleicht dann das neue Raster mit **diesen** gekürzten oder aufgefüllten Zeilen und meldet die abweichenden als **ein** `Ausgabe` (wie TR2, `stand` steigt). Weicht keine ab, kein Ereignis.
- **Cursor bei einer Größenänderung (Nachtrag 19.11.2026).** Der Kern klemmt seinen Cursor auf die neue Größe (`[min(zeile, zeilen − 1), min(spalte, spalten − 1), sichtbar]`, `VT` R15 Punkt 8). Der Träger tut dasselbe mit dem zuletzt gemeldeten Cursor und vergleicht
  dann den Cursor des neuen Rasters mit **diesem**: Weicht er ab, geht er in der `Ausgabe` der Größenänderung hinaus (mit den abweichenden Zeilen oder allein). Das Raster von `alacritty_terminal` bewegt den Cursor selbst (Zeilen, die beim Verkleinern in den Rücklauf gehen, nehmen ihn mit
  nach oben; beim Vergrößern holt es Zeilen aus dem Rücklauf zurück und der Cursor geht mit nach unten; beim Umbruch der Spalten wandert er mit dem Text). Nach dem Klemmen des Kerns kann das Raster den Cursor also an einer anderen Stelle haben: dann kommt eine `Ausgabe`.
- **Cursor bei schrumpfenden Spalten (Nachtrag 20.11.2026, Projektentscheid, 20.11.2026: gewollt).** Steht der Cursor des Hauptschirms rechts vom neuen Rand, **bricht `alacritty_terminal` ihn um** (auch wenn in der Zeile kein Text steht; gemessen mit `tr-ref`: 40 mal 10 auf 20 mal 5, Cursor `[7,29]` wird `[4,9]`).
  Der Träger **klemmt ihn nicht**: Die `Ausgabe` der Größenänderung trägt den Wert des Rasters (`[4,9]`), der vom geklemmten Wert des Kerns (`[4,19]`, `VT` R15 Punkt 8) abweicht; sie geht also immer hinaus. Der Kern übernimmt den Wert des Trägers mit dieser `Ausgabe` (`VT` R15 Punkt 8, Nachtrag 20.11.2026), so dass kein zweiter, abweichender Cursor stehen bleibt;
  das Abbild der Ansicht zeigt danach den Wert des Trägers. Im **Alternativschirm** bricht die Bibliothek nicht um, sie klemmt (gemessen: `[7,29]` wird `[4,19]`, gleich dem Wert des Kerns, so dass allein dadurch keine `Ausgabe` entsteht); beim Verlassen kehrt der Cursor des Hauptschirms mit seinem umgebrochenen Wert zurück.
  Fälle `tr3_cursor_schrumpf`, `tr3_cursor_schrumpf_text`, `tr3_cursor_schrumpf_alternativ`, `tr3_cursor_schrumpf_zwei`; Mutant `cursor_schrumpf_klemmt`.
- Spalten 2 bis 1000, Zeilen 1 bis 1000 (Nachtrag 20.11.2026; kleinste Größe: Abschnitt 7 Punkt 15). Eine Größe mit einer Spalte (oder 0, oder über 1000) erreicht das Raster nie: beim Start und bei `groesse` wird sie abgelehnt, auf demselben Weg wie 0 (Abschnitt 2), und nichts ändert sich. Im Kindteil setzt der Träger auch die Größe der PTY auf `(spalten, zeilen)`, so dass das Kind sie liest (TIOCGWINSZ, `stty size`); das Kind bekommt sein `SIGWINCH` vom System.

### TR4 Rücklauf

- Der Rücklauf ist eine Einstellung des Trägers (Projektentscheid, Messplan Abschnitt 11). Die Vorgabe ist **2000** Zeilen (nicht 10 000). Der Wert gilt für den Rücklauf des Rasters; der sichtbare Schirm ist davon unabhängig.
- `ruecklauf.zahl` des Zustands ist die Zahl der Zeilen im Rücklauf: Sie wächst mit den Zeilen, die oben aus dem Schirm laufen, bis zur Einstellung und nie darüber (bei 0 bleibt sie 0).
- A119 (Plan): 5000 nummerierte Zeilen ergeben genau 2000 Zeilen im Rücklauf; `erste` und `letzte` sind die nummerierten Zeilen Z`(n - 2000 - Zeilenzahl + 1)` und Z`(n - Zeilenzahl)`; keine fehlt dazwischen.
- Der Kern zeigt den Rücklauf nicht (`VT` Abschnitt 7, Punkt 17); der Zustand dieses Vertrags dient nur der Prüfung.
- Einstellungen über 100 000 sind nicht festgelegt.

### TR5 Modi

Der Träger liest die Bytes **zusätzlich** mit einem eigenen Parser der Steuerfolgen (zum Beispiel `vte::Parser`), nicht mit einer Textsuche. Er meldet:
- `ESC [ ? … h` und `ESC [ ? … l` mit Zwischenzeichen genau `?`: Für jeden Parameter in der Reihenfolge der Folge: `5` ergibt `Modus` mit `invers` (true für `h`, false für `l`) und einem neuen `stand` (TR2); `2004` ergibt `Modus` mit `bracketed_paste`. Jedes Vorkommen meldet, auch bei gleichem Wert wie zuvor.
  Weitere Parameter (zum Beispiel `1`, `25`, `1049`, `2026` mit eigener Regel TR6) erzeugen hier kein Ereignis. Eine Folge ohne `?` (`ESC[5h`), mit Abfrage (`ESC[?5$p`), mit weiterem Zwischenzeichen oder mit `>` statt `?` erzeugt keines.
- Das Raster ändert sich bei `?5` nicht (`alacritty_terminal` kennt DECSCNM nicht; der Kern führt den Modus selbst, `VT` R13).
- Eine Folge in der Zeichenkette einer OSC oder DCS ist keine Folge (der Text `[?5h` in einem Titel ändert nichts). Ein `ESC` beendet die Zeichenkette, die Folge dahinter zählt.
- Folgen dürfen über Stückgrenzen verteilt ankommen und wirken beim letzten Byte; ein abgebrochene Folge (zum Beispiel `ESC[?5` und dann Text) wirkt nicht.

### TR6 Synchronisierte Aktualisierung (DEC 2026)

- `ESC [ ? 2026 h` meldet `SyncBeginn`, `ESC [ ? 2026 l` meldet `SyncEnde`, je Vorkommen und Parameter, auch in einer Folge mit anderen Modi und auch ein Ende ohne Beginn oder ein zweiter Beginn.
- Der Träger **puffert nicht**: Die Bytes zwischen Beginn und Ende wirken sofort auf das Raster, und ihre `Ausgabe` geht nach TR2 zwischen den beiden Ereignissen hinaus. Der Kern hält die Bilder zurück und gibt nach 150 ms frei (`VT` R5). Der Träger hat keinen Zeitgeber dafür.
  (Der Bau darf die Pufferung von `vte::ansi::Processor` nicht benutzen, oder muss ihren Zeitgeber so setzen, dass nie etwas gepuffert wird.)

### TR7 OSC 52

- `ESC ] 52 ; <auswahl> ; <daten> (BEL | ESC \)` meldet `Zwischenablage` mit `daten_b64` gleich dem Text `<daten>` **unverändert** (nicht dekodiert, nicht geprüft; die Grenze von 100 000 Byte nach dem Dekodieren prüft der Kern). Leere Daten sind erlaubt (`daten_b64` ist `""`).
  Mit den Daten `?` ist es eine Leseanfrage: `lesen: true`.
- Die Auswahl (zweiter Teil: `c`, `p`, `s`, leer, `cp`, …) wird nicht ausgewertet. Nur eine Folge mit genau drei Teilen (`52`, Auswahl, Daten) meldet etwas; mit weniger oder mehr Teilen oder einer anderen Nummer als 52 geschieht nichts.
- **Der Träger beantwortet eine Leseanfrage nie**: `pty_antworten` bleibt leer, es geht nichts an die PTY (`VT` R2, A112). Der Inhalt der echten Zwischenablage kommt nie vor.
- Es gibt keine Obergrenze der Länge im Träger; ein Inhalt von 102 480 Byte (136 640 Zeichen) geht vollständig hinaus.

### TR8 Titel

- `ESC ] 0 ; <text> (BEL | ESC \)` und `ESC ] 2 ; <text> …` melden `Titel` mit `text`. Besteht die Zeichenkette aus mehr Teilen (der Text enthält `;`), werden alle Teile ab dem zweiten mit `;` verbunden. Ungültiges UTF-8 wird je Teil zu U+FFFD (verlustbehaftet). Leerer Text ist ein Ereignis mit `""`.
- Ohne Semikolon nach der Nummer (`ESC]2 BEL`) kein Ereignis. OSC 1, 3, 22 und `ESC[22;0t` sind kein Titel.
- Jedes Vorkommen meldet, auch bei gleichem Text. Lange Titel (3000 Zeichen) gehen ungekürzt.

### TR9 Größenfolge

- `ESC [ 8 ; <zeilen> ; <spalten> t` (genau drei Parameter, keine Zwischenzeichen) meldet `GroessenFolge` mit `spalten` und `zeilen` (Achtung: die Folge nennt zuerst die Zeilen). Ein leerer Parameter gilt als 0, eine Zahl über 65535 als 65535 (wie `vte`), 0 geht durch.
- Mit anderer Zahl der Parameter (`ESC[8;5t`, `ESC[8t`, vier Parameter), mit anderer erster Zahl, mit Zwischenzeichen oder Unterparametern (`:`) geschieht nichts.
- Die Folge wird **nie ausgeführt**: Raster und PTY behalten die Größe (der Kern lehnt sie mit `groesse_nur_kern` ab, `VT` R3).

### TR10 Antworten an das Kind

- Auf Geräteabfragen im Strom antwortet `alacritty_terminal` (Nachtrag 19.11.2026, Auflage: namentlich, **gemessen** an der Referenz mit `tr10_antworten_namentlich` und `tr10_antworten_stumm` und **hergeleitet** aus dem Quelltext `term/mod.rs`, jede Antwort an der Stelle `Event::PtyWrite`):
  | Abfrage | Antwort (Hex der Bytes in den Fällen; hier als Text) |
  |---|---|
  | DA1: `ESC[c`, `ESC[0c`, DECID `ESC Z` | `ESC[?6c` |
  | DA2: `ESC[>c`, `ESC[>0c` | `ESC[>0;2600;1c` (die Fassung 0.26.0 als Zahl) |
  | DSR `ESC[5n` | `ESC[0n` |
  | DSR `ESC[6n` | `ESC[<zeile+1>;<spalte+1>R` (Cursorstelle im Raster, Zählung ab 1). (A109) Diese Antwort gibt seit dem Nachtrag A109 **der Träger selbst** (TR14); die gleichlautende Antwort der Bibliothek verwirft er |
  | `ESC[18t` | `ESC[8;<zeilen>;<spalten>t` (Größe des Rasters) |
  | DECRQM privat `ESC[?<n>$p` | `ESC[?<n>;<z>$y` mit z 0 (unbekannt), 1 (gesetzt), 2 (nicht gesetzt) |
  | DECRQM ANSI `ESC[<n>$p` | `ESC[<n>;<z>$y` mit z 0, 1 oder 2 |
  Jede Antwort enthält nur feste Bytes und Zahlen der Abfrage und des Rasters, **nie Text, den das Kind oder ein Dritter vorher gesetzt hat**.
- **Ohne Antwort** bleiben (gemessen, `tr10_antworten_stumm`): DSR mit `?` (`ESC[?6n`, `?15n`, `?25n`, `?996n`), `ESC[14t`, `ESC[16t`, `ESC[19t`, `ESC[15t`, Titelabfrage `ESC[21t` und `ESC[20t`, `ESC[22;0t`, `ESC[23;0t`, DA3 `ESC[=c`, XTVERSION `ESC[>q`, Kitty `ESC[?u` (nur mit eingeschalteter Option der Bibliothek),
  `ESC[x`, DECRQCRA, XTSMGRAPHICS, XTGETTCAP (`DCS + q`), DECRQSS (`DCS $ q … ST`), Farbabfragen OSC 4, 10, 11, 12 mit `?`, OSC 104, OSC 52 (TR7) und ENQ. Grund: `alacritty_terminal` meldet diese als eigene Ereignisse (`ColorRequest`, `TextAreaSizeRequest`, `ClipboardLoad`) oder gar nicht; der Träger
  gibt **nur `PtyWrite`** an das Kind zurück und beantwortet die anderen nie. Ein Bau, der die Farb- oder Größenabfrage der Bibliothek selbst beantworten wollte, brächte Werte, die der Träger nicht kennt.
- Der Inhalt eines gesetzten Titels, einer gesetzten Farbe oder einer DCS-Zeichenkette geht nie in eine Antwort (Fälle `tr10_antwort_titel`, `tr10_antwort_farbe`, `tr10_antwort_decrqss`, `tr10_antwort_kein_text`; Mutanten `antwort_titel_text`, `antwort_farbe_text`, `antwort_decrqss_text`).
  Der Befund der Messung am 19.11.: Keine dieser Abfragen antwortet, und keine Antwort trägt Text; es gab nichts anzuhalten. Die Referenz hat dafür keinen eigenen Filter, sie gibt nur `PtyWrite` weiter.
- Der Träger schreibt **genau diese Bytes** in der Reihenfolge der Abfragen an die PTY zurück Der Träger schreibt **genau diese Bytes** in der Reihenfolge der Abfragen an die PTY zurück
  (im Programm: `pty_antworten`). Ohne diese Antworten läuft ein Kind unter ConPTY nicht an (Messplan Abschnitt 13).
- Die Antwort auf die Cursorabfrage ändert sich mit der Größe und der Cursorstelle des Rasters. (A109) Sie kommt nach TR14 vom Träger, nicht von `alacritty_terminal`; alle übrigen Antworten dieser Regel bleiben die von `PtyWrite`.
- Die Antwort steht im Ergebnis des Schritts, in dem die Abfrage ankam.

### TR11 Sonderfall Hautton

Emoji-Breite nach Unicode mit dem Sonderfall Hautton (Projektentscheid, 18.11.2026, Messplan Abschnitt 11):
- Ein Hauttonzeichen (U+1F3FB bis U+1F3FF) ist **breitenlos** und hängt am Zeichen davor wie ein kombinierendes Zeichen (U+0301). Es belegt keine Zelle, hat keine zweite Hälfte und steht in der Zeilenform direkt hinter dem Grundzeichen (`a👍🏽b` belegt vier Zellen, der Cursor steht danach auf Spalte 4).
  (`alacritty_terminal` macht es sonst zu einem eigenen breiten Zeichen.)
- Das gilt auch am Anfang einer Zeile, am rechten Rand, hinter jedem Zeichen, nach Löschen und wenn die vier Byte des Zeichens über Stückgrenzen verteilt ankommen.
- Andere Emoji-Folgen (Variantenwahl U+FE0F und U+FE0E, Zeichenverbindung U+200D) verhalten sich wie in `alacritty_terminal` 0.26.0; seit dem Cursor-Nachtrag schreiben Fälle dieses Verhalten fest (Abschnitt 7 Punkt 10; `tr11_variante_fe0f`, `tr11_variante_fe0e`, `tr11_zwj_folge`, `tr11_hautton_zwj`,
  `tr11_variante_zeilenende`, `tr11_variante_stueck`, `tr11_variante_ein_stueck`; Mutanten `fe0f_ignoriert`, `fe0e_ignoriert`, `zwj_ignoriert`). Gemessen: Alle drei Zeichen haben die Breite 0 und hängen am Zeichen davor, ändern dessen Breite nicht (`☺️` und `❤️` belegen eine Zelle, nicht zwei) und belegen nie eine Zelle;
  die Zeichen einer ZWJ-Folge belegen ihre eigenen Zellen. Zugesagt ist weiter nur: kein Absturz, und Träger und Ansicht rechnen dieselbe Breite.
- Der Bau darf den Weg frei wählen (zum Beispiel die Bytes vor dem Parser umschreiben). Zeichen U+E0100 bis U+E0104 in der Eingabe sind nicht festgelegt (die Referenz nutzt sie als Platzhalter).

### TR12 PTY und Kind (Kindteil)

- Das Kind läuft in einer PTY von `portable-pty` mit der Größe des Falls. Die Umgebung ist **genau** `start.env`, nichts sonst (`env_clear`; keine zusätzliche Variable, auch `SHELL` nur, wenn `start.env` sie nennt; TR15); das Arbeitsverzeichnis ist `start.cwd` (Vorgabe `/`; TR15). `argv` des Kindes ist das Programm `tr --kind <kind> <argv…>`.
- Der Träger liest die PTY bis zum Ende der Ausgabe und gibt die Bytes stückweise an `Raster::bytes`. Danach wartet er auf das Kind. Erst dann meldet er `Kindende` mit `code` (der Exitcode des Kindes, 0 bis 255). Vor dem `Kindende` geht jede Ausgabe hinaus (TR2: Ausgabe zuerst, dann Kindende; `ausgaben_nach_kindende` ist 0).
  Die Sitzung bleibt danach lesbar (das Raster steht).
- Die Wirkung `pty` des Kerns (Schritt `Tasten`) schreibt die Bytes **unverändert** an die PTY (kein Umwandeln, kein Ergänzen).
- Die Ausgabe-Ereignisse des Kindteils sind nicht Teil des Vergleichs; stattdessen steht im Zustand `abbild_gleich` (alle Ausgabe-Ereignisse zusammen ergeben das Raster).
- Feste Kindprogramme (jedes ruft nur Standardbibliothek und `stty` auf; Ausgabe mit `\r\n`):
  - `ausgabe <hex>`: schreibt die Bytes und endet mit Code 0.
  - `ende <code>`: endet sofort mit dem Code, ohne Ausgabe.
  - `zeilen <n>`: schreibt die Zeilen `Z000001` bis `Z<n>` (6 Ziffern), je mit `\r\n`, Code 0.
  - `groesse`: schreibt `BEREIT`, liest eine Zeile von stdin, ruft `stty size` und schreibt `GROESSE <zeilen> <spalten>`; Code 0.
  - `eingabe <n>`: ruft `stty raw -echo`, schreibt `BEREIT`, liest genau `n` Byte von stdin und schreibt `EINGABE <hex der n Byte>`; Code 0.
  - `umgebung <NAME…>`: schreibt je Name `NAME=<Wert>` oder `NAME=-` (nicht gesetzt); Code 0.
  - (TR15) `umgebung_alle`: schreibt jede Variable des Kindes als `NAME=<Wert>`, nach Name sortiert, je mit `\r\n`; Code 0.
  - (TR15) `ordner`: schreibt `ORDNER <Arbeitsverzeichnis>` (`std::env::current_dir`); Code 0.
  - (TR15) `stumm`: schreibt `BEREIT`, liest nie von stdin und wartet 30 s; Code 0. Der Träger beendet es am Ende des Falls.
  - `titel <text>`: schreibt `ESC ] 2 ; <text> BEL`, dann `fertig`; Code 0.
  - `cursor <hex>` (A109): ruft `stty raw -echo`, schreibt die Bytes (darin die Cursorabfragen), dann `ESC[5n` als Endmarke, liest stdin, bis die Antwort `ESC[0n` auf die Endmarke da ist, höchstens 1 s, und schreibt `\r\nANTWORT <hex der Bytes vor der Endmarke>\r\n` (nach Ablauf der Sekunde `\r\nZEIT <hex des Gelesenen>\r\n`); Code 0.
  - `signal <n> [<hex>]` (Nachtrag 19.11.2026): schreibt die Bytes (wenn angegeben), schickt sich dann selbst das Signal n (`sh -c 'kill -<n> <pid>'`, die Shell ist ohnehin für `stty` nötig) und wartet; die Standardwirkung beendet es. Der Bau darf sich das Signal auch anders schicken.
- **Signal (Nachtrag 19.11.2026, Projektentscheid):** Endet das Kind durch ein Signal, trägt `Kindende` den Code **128 plus die Signalnummer** (1, 2, 9, 15 gemessen: 129, 130, 137, 143). Die Referenz liest die Nummer aus dem Wartestatus (`waitpid`, `WIFSIGNALED`, `WTERMSIG`), nicht aus dem Text von `portable-pty`
  (der ist je System verschieden). Ein Kind, das mit einem Code endet, trägt ihn unverändert. Fälle: `tr12_kind_signal_hup`, `_int`, `_kill`, `_term`; Mutanten `kindende_signal_ohne_128`, `kindende_signal_null`.

### TR13 Grenzen und Exit

- Größen: Spalten 2 bis 1000, Zeilen 1 bis 1000 (Nachtrag 20.11.2026: die kleinste Größe ist **2 Spalten und 1 Zeile**; eine Spalte und 0 sind ungültig, Abschnitt 7 Punkt 14 und 15), 100 000 Zeilen Rücklauf, ein OSC von mehr als 100 000 Byte und ein Titel von 3000 Zeichen müssen funktionieren. Keine Eingabe löst eine Panik aus.
- Exitcode 2 bei ungültigem Fall (Abschnitt 2); der Fall ohne Schritte ist gültig und gibt nur `ende`.

### TR15 Umgebung, Arbeitsordner und Schreiben (Nachtrag 20.11.2026)

Herkunft: Der Dienst-Bau fand zwei Fehler des Trägers (Ergebnis, OPEN 2 und 3), und ein dritter Fund aus dem Dienst-Bau stand schon in TR12 („genau“). `portable-pty` 0.9.0 setzt dem Kind immer `SHELL` (`cmdbuilder.rs` `as_command`) und fällt auf das Heimverzeichnis zurück, wenn `cwd` kein Ordner ist (ebenda, `.filter(|dir| Path::new(dir).is_dir()).unwrap_or(home)`). Zugesagt ist:

- **Umgebung genau.** Das Kind sieht genau die Variablen von `start.env` mit ihren Werten und keine weitere. Fehlt `SHELL` in `start.env`, fehlt sie auch im Kind; nennt `start.env` sie, hat das Kind genau diesen Wert. Fälle `tr15_kind_umgebung_genau`, `tr15_kind_umgebung_shell` (Kindprogramm `umgebung_alle`).
- **`cwd` ist ein Ordner.** `start.cwd` (Text, Vorgabe `/`) ist das Arbeitsverzeichnis des Kindes. Ist es kein Ordner (fehlt, ist eine Datei, ist leer), endet `Kind::starten` mit einem Fehler (`Fehler::Start`), und es startet nichts; im Programm `tr` ist der Fall dann ungültig (Exitcode 2). Es gibt keinen Rückfall in ein anderes Verzeichnis. Fälle `tr15_kind_cwd_ordner`, `tr15_kind_cwd_kein_ordner`, `tr15_kind_cwd_typ` (Kindprogramm `ordner`).
  `tr15_kind_cwd_ordner` nutzt einen Unix-Pfad als Ordner (`/usr/bin`, im verdeckten Satz ein anderer) und trägt deshalb `"system": ["linux", "macos"]` (Nachtrag 21.11.2026, VM-Block A64: unter Windows ist der Pfad kein Ordner, der Fall endete mit Exitcode 2). Unter Windows zählt er als nicht anwendbar (TR16). Ein Fall mit einem Windows-Ordner fehlt noch (OPEN); die zwei Fälle der Ablehnung gelten auf allen Systemen.
- **Schreiben blockiert nie.** `Kind::schreibe` reiht die Bytes in eine Schlange ein und kehrt sofort zurück; ein eigener Faden schreibt sie in Reihenfolge, unverändert, in Stücken von höchstens 4096 Byte an die PTY. Die Schlange hält **höchstens 262 144 Byte** (256 KiB), die Bytes, die der Faden gerade schreibt, mitgezählt. Passt ein Schreiben nicht mehr hinein, wird es als Ganzes abgelehnt (nichts davon wird eingereiht): `Kind::schreibe` liefert `Err` der Art `WouldBlock`, und der Schritt `Tasten` trägt `"ok": false` und `"fehler": "schlange_voll"`. Ein Schreiben von genau 262 144 Byte in eine leere Schlange wird angenommen, eines von 262 145 Byte nie. Ein leeres Schreiben gelingt immer. Ein Schreiben nach dem Ende des Kindes bleibt ohne Meldung (wie bisher); schlägt ein Schreiben an die PTY fehl, nimmt die Schlange nichts mehr an (`BrokenPipe`).
  Begründung der Grenze: Die Eingabe einer Sitzung sind Tasten und Einfügungen von einigen Byte bis einigen KiB; 256 KiB sind 64 mal der Puffer der PTY unter Linux (rund 4 KiB) und weit über jeder großen Einfügung. Sie binden bei einem hängenden Kind höchstens 256 KiB je Sitzung, einen kleinen Teil der 11,4 MiB je Sitzung (P7). Eine größere Grenze verschöbe nur den Zeitpunkt, an dem ein hängendes Kind auffällt.
  Fälle `tr15_kind_schlange_grenze`, `tr15_kind_schlange_voll`, `tr15_kind_schreiben_nie_blockiert` (Kindprogramm `stumm`). Die Antworten des Trägers (TR10, TR14) gehen durch dieselbe Schlange; ist sie voll, gehen sie verloren.
- **Fälle sind unabhängig vom Puffer.** Jedes Schreiben der Fälle ist so gewählt, dass sein Ergebnis nicht davon abhängt, wie viel die PTY schon aufgenommen hat (ein Schreiben bis zur Grenze in eine leere Schlange, eines über der Grenze, mehrere Schreiben mit zusammen höchstens der Grenze).

### TR14 Cursorabfrage (A109)

Anforderung A109 (Projektentscheid, 18.11.2026; Messplan des Terminalträgers, Abschnitt 13 bis 16): Der Träger beantwortet `ESC[6n` **selbst**, auf allen Plattformen und unabhängig davon, ob der Bildschirmspeicher die Abfrage beantwortet.
Grund: `portable-pty` 0.9.0 öffnet ConPTY mit `PSEUDOCONSOLE_INHERIT_CURSOR`; ConPTY schickt dann vor dem ersten Byte des Kindes `ESC[6n` und startet das Kind erst nach der Antwort (gemessen: ohne Antwort in 8 von 8 Läufen kein Eingabefeld von Claude Code nach 60 s).
- **Was eine Cursorabfrage ist (A109).** Genau die CSI-Folgen, die `alacritty_terminal` 0.26.0 als Cursorabfrage nimmt (`vte` 0.15 `('n', [])` mit `device_status(6)`): Endbyte `n`, keine Zwischenzeichen, die Folge nicht verworfen (`ignore`), erster Wert des ersten Parameters 6.
  Also `ESC[6n`, `ESC[06n`, `ESC[6;5n`, `ESC[6:1n`; nicht `ESC[?6n` (Zwischenzeichen `?`, ohne Antwort wie bisher), nicht `ESC[0;6n`, `ESC[n` (erster Parameter nicht 6, ohne Antwort) und nicht `ESC[5;6n` (das ist DSR 5, Antwort `ESC[0n` der Bibliothek). Der Träger findet sie mit dem eigenen Parser der Steuerfolgen (TR5), über Stückgrenzen hinweg.
- **Die Antwort (A109).** `ESC [ <zeile> ; <spalte> R` mit `zeile` und `spalte` **ab 1**, aus dem Cursor des eigenen Rasters **beim letzten Byte der Abfrage**: dem Cursor, den `Ausgabe` meldet (TR2), plus 1 in beiden Werten.
  Also im angezeigten Schirm (im Alternativschirm dessen Cursor), nie im Rücklauf gezählt, mit ausstehendem Umbruch in der letzten Spalte (`spalte` = Spaltenzahl), unabhängig von DECTCEM. Zuerst die Zeile, dann die Spalte.
- **Genau einmal, in Reihenfolge (A109).** Jede Abfrage bekommt genau eine Antwort. Die Antwort steht in `pty_antworten` des Schritts, in dem das letzte Byte der Abfrage ankam, und dort an der Stelle der Abfrage unter den anderen Antworten (TR10): `ESC[c ESC[6n ESC[5n` gibt `ESC[?6c`, die Cursorantwort, `ESC[0n`.
  Eine abgebrochene Abfrage (zum Beispiel `ESC[6` und dann CAN 0x18) bekommt keine.
- **Zusammen mit TR10 (A109).** `alacritty_terminal` antwortet auf dieselben Abfragen über `PtyWrite` mit denselben Bytes. Der Träger **verwirft** diese eine Antwort der Bibliothek (das einzige `PtyWrite` der Form `ESC [ <Ziffern> ; <Ziffern> R`; alle anderen Antworten der Bibliothek enden auf `c`, `n`, `t` oder `y`) und setzt seine eigene an ihre Stelle.
  Weil beide dieselbe Menge von Abfragen erkennen, gibt es keine doppelte und keine verlorene Antwort. Mit `alacritty_terminal` als Bildschirmspeicher sind die Bytes dieselben wie vor dem Nachtrag (alle 183 älteren Fälle byte-gleich); der Unterschied zeigt sich erst bei einem Bildschirmspeicher, der nicht antwortet (unter Windows zum Beispiel `vt100` der Messung).
- **Keine Ausgabe, kein Ereignis (A109).** Die Abfrage ist kein Ereignis nach Abschnitt 4: vor ihr geht keine `Ausgabe` hinaus, sie teilt die Ausgabe eines Stücks nicht (TR2), `stand` ändert sich nicht.
- **Kindteil (A109).** Der Träger schreibt die Antwort sofort an die PTY (Abschnitt 3). Zweiter Fall von A109: das Kindprogramm `cursor` (TR12) schreibt `ESC[6n`, wartet höchstens 1 s und schreibt die erhaltenen Bytes; sie sind die Cursorlage im Raster des Trägers plus 1.
  Unter ConPTY erreicht eine Abfrage des Kindes den Träger nicht (ConPTY beantwortet sie selbst, Messplan Abschnitt 16 W3); dort trifft TR14 die Abfrage aus dem Vorspann von ConPTY. Sie kommt vor jedem Byte des Kindes, der Cursor steht dann bei `[0, 0]`, die Antwort ist `ESC[1;1R`; das passt zu `INHERIT_CURSOR` (ConPTY übernimmt die Lage `[0, 0]` des leeren Rasters).
- Fälle: `tr10_cpr_lage`, `tr10_cpr_mitten`, `tr10_cpr_zwei`, `tr10_cpr_stueck`, `tr10_cpr_alternativ` (Stromteil), `tr12_kind_cpr`, `tr12_kind_cpr_mitten`, `tr12_kind_cpr_zwei`, `tr12_kind_cpr_alternativ` (Kindteil, Linux-PTY), je sichtbar und verdeckt.
  Mutanten: `cpr_keine_antwort`, `cpr_doppelt`, `cpr_vertauscht`, `cpr_nullbasiert`, `cpr_lage_stueckende`, `antworten_kind_verschlucken`.

### TR16 Systeme, Last und Vorspann von ConPTY (Nachtrag 21.11.2026, Projektentscheide)

Herkunft: Das Projekt hat am 21.11.2026 die vier offenen Fragen entschieden (Punkte 1, 2 und 4 betreffen diesen Vertrag; Punkt 3, `Kindende` unter ConPTY, ist ein eigener Bauauftrag A64). Jede Änderung dieses Nachtrags trägt „(TR16)“.

1. **A109 und Last (Entscheidung 1).** Das Kriterium von A109 („Eingabefeld von Claude Code nach höchstens 10 s“) gilt für die ruhige VM wie bei A113. Läufe unter Last sind Messwerte mit Last, weder bestanden noch rot.
   Bedingung, die der Vertrag prüft: **Unter Last darf nur die Zeit wachsen, nie darf eine Cursorantwort fehlen oder doppelt kommen.** Beleg ist der Fall `tr14_cpr_unter_last` (Kindteil, Kindprogramm `cursor`, Feld `last` 16): derselbe Vorgang wie `tr12_kind_cpr`, aber der Läufer hält 16 Lastschleifen am Laufen. Erwartet wird dieselbe Antwort wie ohne Last (aus der Referenz), vier Abfragen, vier Antworten in der Reihenfolge der Abfragen, keine doppelt, keine fehlt.
   Der Fall wartet auf Zustände, nicht auf Zeiten (Nachtrag 21.11.2026 Zeitannahmen); nur das Kindprogramm `cursor` hat eine feste Frist von 1 s, die bei 16 Lastschleifen auf einem Rechner mit mindestens 16 Kernen reicht.
2. **W7: Vorspann-Titel von ConPTY (Entscheidung 2).** Messplan Abschnitt 13 Punkt 4 und Abschnitt 15 W7: ConPTY schreibt vor dem ersten Byte des Kindes `ESC[6n ESC[?9001h ESC[?1004h ESC[m ESC]0;<Pfad der Programmdatei> BEL ESC[?25h`. **Der Träger gibt aus dem Titel dieses Vorspanns kein Ereignis `Titel` aus und ändert den Sitzungstitel nicht.**
   Der Titel einer Sitzung kommt nur aus OSC 0 und 2 des Programms (TR8). Die übrigen Folgen des Vorspanns bleiben wie jede andere Folge (TR5, TR10; die Cursorabfrage beantwortet der Träger nach TR14). Der Bau entscheidet, wie er den Vorspann erkennt; erkennbar ist er an dem Titel mit dem Pfad der gestarteten Programmdatei, der vor jedem anderen Byte des Kindes steht. Ein Titel, den das Programm selbst schreibt, bleibt ein Ereignis, auch wenn er gleich lautet.
   **Gebaut (W7, 21.11.2026, `rust/crates/traeger/src/vorspann.rs`):** Verworfen wird der erste Titel einer Sitzung, wenn er den Pfad der gestarteten Programmdatei nennt (Groß- und Kleinschreibung, Schrägstrich oder Gegenschrägstrich und ein Präfix `\\?\` ändern nichts; bei einem Programm ohne Pfad zählt der Dateiname) und vorher kein anderes Ereignis hinausgegangen ist, auch keine `Ausgabe`. Jeder spätere Titel bleibt ein Ereignis. Der Träger führt den Sitzungstitel nur aus den gemeldeten Titeln (`Raster::titel`).
   Der Vorspann gibt es nur unter ConPTY. Fälle: `tr8_kind_vorspann_titel` (Kind schreibt Text ohne Titel: kein Ereignis `Titel`) und `tr8_kind_vorspann_titel_programm` (Kind schreibt einen Titel: genau ein Ereignis `Titel`, der des Programms), je sichtbar und verdeckt, beide `"system": ["windows"]`. Unter Linux und macOS zählen sie als **nicht anwendbar** (Punkt 4): Die Referenz läuft auf Unix und kennt keinen Vorspann, deshalb hat die Regel keinen Mutanten der Referenz. Belegt wird sie durch den Lauf des Baus unter Windows (VM, Block der Kindteil-Fälle), sobald `Kindende` unter ConPTY gebaut ist (A64).
3. **`Kindende` unter ConPTY (A64).** Unter ConPTY endet die Ausgabe nicht mit dem Kind: die Pipe schließt sich erst mit `ClosePseudoConsole`. Der Träger beobachtet deshalb das Ende des Prozesses selbst (ohne zu warten); ist der Prozess beendet und die Ausgabe noch offen, schließt er die Pseudokonsole genau einmal, liest den Rest der Ausgabe bis zum Ende der Pipe und meldet erst dann `Kindende` mit dem Code des Prozesses (TR2: Ausgabe zuerst, dann `Kindende`; `ausgaben_nach_kindende` bleibt 0). Unter Linux und macOS endet die Ausgabe mit dem Kind und das Verhalten ändert sich nicht. Die Logik (Ende erkannt, schließen, leeren, melden) ist in `rust/crates/traeger/src/abschluss.rs` getrennt und unter Linux mit einer Attrappe getestet; ob ConPTY unter Windows den Rest der Ausgabe nach dem Schließen vollständig liefert, belegt nur der Lauf in der Windows-VM (die Fälle des Kindteils, Block der VM). Keine neuen Fälle.
4. **Systeme (Entscheidung 4).** Kindprogramme mit `stty` oder `sh` (`groesse`, `eingabe`, `signal`, `cursor`) bleiben Fälle für Systeme mit einer Unix-PTY. Jeder Fall mit einem dieser Kindprogramme trägt `"system": ["linux", "macos"]` (macOS hat `stty` und `sh`; die Entscheidung nannte „nur Linux“, der Mac-Lauf soll sie weiter belegen). Unter Windows zählt der Läufer sie als **nicht anwendbar**, nie als grün. Kein `unsafe` im Träger und keine neue Abhängigkeit für Windows-Fassungen dieser Kindprogramme.
   Es kommt nur die Markierung dazu: keine Erwartung und kein anderes Feld eines bestehenden Falls ändert sich (Beleg: `cmp` der alten Dateien, Ergebnisdatei des Laufs).
5. **Läufer.** Der Läufer (`test_aussen.py`, im Bau die Fälle-Tests des Crates) liest `system` und `last`:
   - Gilt ein Fall nicht auf diesem System, führt der Läufer ihn nicht aus und zählt ihn als `nicht anwendbar`: nie als grün, nie als rot. Die Summenzeile nennt die drei Zahlen (`N gruen, M rot, K nicht anwendbar von T`). Gilt kein einziger ausgewählter Fall hier, endet der Läufer mit Exitcode 2 (nichts ist belegt).
   - **Last nur mit Freigabe (Projektentscheid, 21.11.2026):** Ein Fall mit `last` läuft nur, wenn `TR_LAST=1` gesetzt ist. Sonst führt der Läufer ihn nicht aus und zählt ihn als `nicht anwendbar` („Last nicht freigegeben“), nie als grün, nie als rot. Last gibt es nur dort, wo die Maschine sie erlaubt (ein Linux-Prüfrechner), nie im Gesamtlauf am Mac und nie in einem gewöhnlichen Torlauf.
   - Für einen Fall mit `last` und `TR_LAST=1` hält der Läufer so viele Lastprozesse am Laufen, solange der Fall läuft, und beendet sie danach. Die Lastprozesse enden auch von selbst, wenn der Läufer stirbt (auch durch SIGKILL: jede Schleife prüft ihren Elternprozess) und nach einer Höchstdauer (100 s, `TR_LAST_MAX`); jede läuft in einer eigenen Prozessgruppe.
   - `laeufer_selbsttest.py` prüft das mit einem Ersatzprogramm für `tr` und sieben Fällen (dazu einem Fall, der hängt), dazu Mutanten des Läufers (Abschnitt 13): „nicht anwendbar zählt als grün“ macht eine Prüfung rot. Der Selbsttest startet Lastprozesse: nur dort laufen lassen, wo Last erlaubt ist (ein Linux-Prüfrechner).
6. **W1: Bytes unter ConPTY (Nachtrag 21.11.2026, Projektentscheid).** ConPTY schreibt die Zeile eines Kindes selbst neu und legt ein Leerzeichen hinter einem Stilwechsel in den Lauf davor. Gemessen in der VM (Build 26300, `$.ende.zeilen[1]`): Das Kind schreibt `ESC[31m`, `rot`, `ESC[0m`, dann ein Leerzeichen und `Grüße äöü ß` (das Leerzeichen steht hinter dem Stilende); erwartet ist `\u001b[0;31mrot\u001b[0m Grüße äöü ß`, gemessen `\u001b[0;31mrot \u001b[0mGrüße äöü ß` (der verdeckte Satz enthält denselben Fall mit anderem Text). Sichtbar ist das gleich, im Raster weicht die Zelle ab, und die Zeilenform ist auf jedem System die der Referenz.
   Deshalb trägt `tr12_kind_ausgabe` jetzt `"system": ["linux", "macos"]` (nur diese Markierung; die Erwartung ist unverändert). Der neue Fall `tr12_kind_ausgabe_stil` (Regeln TR12 und TR16, kein Feld `system`) schreibt dieselben Bytes mit dem Leerzeichen schon im farbigen Lauf (`ESC[31m`, `rot` und ein Leerzeichen, `ESC[0m`, dann ohne Leerzeichen die Umlaute), ändert ConPTY also nichts; seine Erwartung kommt aus der Referenz. Er gilt für jedes System. Ein Ausgleich im Träger gibt es nicht (er würde unter Linux und macOS die Zeilenform ändern).
7. **W5: Leseanfrage OSC 52 unter ConPTY (Nachtrag 21.11.2026, Projektentscheid).** Unter ConPTY kommt die Leseanfrage `ESC]52;c;?` nie beim Träger an. Gemessen in der VM (Build 26300, Block): Das Ereignis `Zwischenablage` mit `lesen` fehlt, auch in einem Messbau mit `PSEUDOCONSOLE_PASSTHROUGH_MODE` (nie committet). Das Schreiben über OSC 52 kommt an. TR7 Punkt 6 („eine Leseanfrage wird nie beantwortet“) ist damit auch unter Windows erfüllt. Deshalb trägt `tr12_kind_osc52` `"system": ["linux", "macos"]` (nur diese Markierung; die Erwartung ist unverändert). Nebenbefund ohne Regel: Nach `ESC[8;20;50t` steht der Text unter ConPTY im Rücklauf statt in Zeile 0; die Rohbytes dazu sind nicht aufgezeichnet (OPEN).

## 7. Festgelegt im Vertrag

1. Das Verhalten von `alacritty_terminal` 0.26.0 ist die Grundlage (Referenz aus `the previous implementation`); der Vertrag sagt nur, was dazukommt.
2. Zeilenform und Normalform sind die von `RA`; das Programm `tr` gibt jede Zeile so aus.
3. `Ausgabe` je Stück Bytes mit den gegenüber der letzten Meldung abweichenden Zeilen; vor jedem anderen Ereignis eine Ausgabe der bisher geänderten Zeilen (TR2). Der gemeinsame Zähler `stand` für `Ausgabe` und `Modus invers`.
4. Modi, Sync, Titel, OSC 52 und Größenfolge kommen aus einem eigenen Parser der Steuerfolgen, nicht aus Ereignissen von `alacritty_terminal`; jedes Vorkommen meldet.
5. Der Träger puffert DEC 2026 nicht; der Zeitgeber gehört dem Kern.
6. Eine Leseanfrage OSC 52 wird nie beantwortet; Daten werden nie dekodiert.
7. Größenfolgen werden gemeldet, nie ausgeführt; Zahlen über 65535 gelten als 65535.
8. Nach einer Größenänderung vergleicht der Träger mit den vom Kern gekürzten oder aufgefüllten Zeilen.
9. Rücklauf: Einstellung mit Vorgabe 2000 (Projektentscheid).
10. Hauttonzeichen sind breitenlos. Variantenwahl und Zeichenverbindung (U+FE0F, U+FE0E, U+200D) sind eine benannte Abweichung (Projektentscheid, 19.11.2026): Es gilt das Verhalten von `alacritty_terminal` 0.26.0, ohne eigene Tabelle. Zugesagt ist: kein Absturz, und Träger und Ansicht rechnen dieselbe Breite. Fälle schreiben das heutige Verhalten fest (Cursor-Nachtrag, 19.11.: sieben Fälle in TR11, drei Mutanten). P11 lautet: 24 von 24 Aufnahmen gleich oder als benannte Abweichung gedeckt; die Abnahme nennt die Zahl der Abweichungen.
11. Kindende kommt nach der letzten Ausgabe und trägt den Exitcode. Ein Kind, das durch ein Signal endet, trägt 128 plus die Signalnummer (Projektentscheid, 19.11.2026; Fälle im Cursor-Nachtrag, 19.11.: vier Fälle, Kindprogramm `signal`).
12. Die Umgebung des Kindes ist genau die des Falls; die Kindprogramme sind Teil von `tr`.
13. In den Ergebnissen des Kindteils stehen keine Ausgabe-Ereignisse und kein `stand`.
14. Eine Spalte erreicht das Raster nie (Nachtrag 20.11.2026; Projektentscheid, 20.11.2026: **die kleinste Größe ist 2 Spalten für die ganze Sitzung**; Kern, PTY und Raster). Herkunft: `alacritty_terminal` 0.26.0 hat zwei Fehler, die nur bei einem Raster mit einer Spalte auftreten (Fund des Baus 19.11., gemessen mit `tr`; die Meldung an das Projekt ist alacritty/alacritty Issue 9112, Angabe; dieser Nachtrag und der Bau haben nichts nach außen gesendet):
    ein Zeichen der Breite 2 gibt dort eine Panik (`grid/mod.rs:439`), und `Term::resize` auf eine Spalte endet nie, wenn im Raster ein breites Zeichen steht (`grid/resize.rs`; es belegt Speicher, bis die Zuweisung scheitert). Die Bibliothek nennt selbst `term::MIN_COLUMNS = 2` („a minimum of 2 is necessary to hold fullwidth unicode characters“), prüft es aber in `Term::new` und `Term::resize` nicht (die Anwendung Alacritty tut es).
    **Das Minimum ersetzt den Schutz.** Der Schutz des Nachtrags 19.11. (breite Zeichen bei einer Spalte zu U+FFFD, `schmal`, `schmal_rest`, `breite_entfernen`, `entbreiten`) entfällt; eine Größe mit einer Spalte wird beim Start (`Raster::neu`, `Kind::starten`) und bei `groesse` (`Raster::groesse`, `Kind::groesse`) abgelehnt, auf demselben Weg wie 0 (Abschnitt 2): Der Träger klemmt nicht still, und nichts ändert sich
    (Raster, Cursor, Rücklauf, `stand`, und im Kindteil die Größe der PTY). Das gilt für den Hauptschirm und den Alternativschirm. Auch eine Größe über 1000 und mit 0 Zeilen wird so abgelehnt. Die Ansicht klemmt ihren Wunsch auf mindestens 2 Spalten (Vertrag `dienst`/`abbild`, nicht dieser Vertrag).
    Belegt ist es durch Fälle (`tr13_eine_spalte_start`, `tr13_eine_spalte_start_kind`, `tr13_eine_spalte_schritt`, `tr13_eine_spalte_schritt_alternativ`, `tr13_groesse_versuch`, `tr13_groesse_versuch_alternativ`, `tr12_kind_groesse_abgelehnt`) und Mutanten (`grenze_fehlt`, `grenze_start_fehlt`, `grenze_groesse_fehlt`, `grenze_alt_fehlt`, `grenze_verschoben`, `grenze_zeilen_verschoben`, `groesse_pty_vor_pruefung`).
    Sicherheitsnachweis bei 2 Spalten (gemessen 20.11. auf dem Linux-Baurechner mit `werkzeuge/zufallsprobe.py`, Referenz `alacritty_terminal` 0.26.0, kein Schutz): Samen 20261120 mit 3000 Fällen (2889 mit breiten Zeichen) und Samen 7 mit 10 000 Fällen (9603 mit breiten Zeichen), Haupt- und Alternativschirm, Rücklauf 0, 5, 100 und 2000, Größenwechsel zwischen 2 und 14 Spalten, dazu Versuche mit 0, 1, 2 und 3 Spalten:
    ohne Absturz, ohne Endlosschleife (Frist 20 s je Fall), `abbild_gleich` immer wahr, Cursor immer in der Größe, abgelehnte Versuche ändern die Größe nie. Der Prüfer fängt den Fehler bei einer Spalte (mit dem Mutanten `grenze_fehlt`: Panik `index out of bounds` und Endlosschleife von `Term::resize`).
    Zeichen der Breite 2 füllen bei 2 Spalten eine ganze Zeile; ein breites Zeichen in der letzten Spalte bricht um (TR2).
15. Kleinste Größe (Nachtrag 20.11.2026, Projektentscheid, 20.11.2026; vorher Nachtrag 19.11.2026: eine Spalte): **2 Spalten und 1 Zeile**, größte 1000 mal 1000; 0 Spalten, 1 Spalte, 0 Zeilen und mehr als 1000 sind ungültig (Abschnitt 2). Die Fälle des Nachtrags 19.11. mit einer Spalte sind auf 2 Spalten umgestellt oder durch Ablehnungsfälle ersetzt (Ergebnisdatei, Tabelle).
16. Cursor (Nachtrag 19.11.2026, Projektentscheid): `Ausgabe` trägt `cursor` immer; eine `Ausgabe` geht hinaus, wenn eine Zeile oder der Cursor von der letzten Meldung abweicht; der Anfangscursor `[0, 0, true]` gilt als gemeldet (der Dienst gibt ihn dem Kern als `anfang` mit); `abbild_gleich` schließt den Cursor ein; der Träger klemmt den gemeldeten Cursor bei einer Größenänderung wie der Kern (TR3).
    Grund für „immer“: Der Kern reicht den jüngsten Cursor in jeder Nachricht durch (`VT` R15); eine Ausgabe nur mit geänderten Feldern zwänge den Dienst zu Buchführung. Grund für den Anfangscursor: Ohne ihn zeigte ein neu abonnierter Betrachter nach `RA` R6 einen unsichtbaren Cursor, solange das Kind nichts bewegt.
17. Cursor bei schrumpfenden Spalten (Nachtrag 20.11.2026, Projektentscheid, 20.11.2026: JA, festgeschrieben): Der Träger bricht den Cursor des Hauptschirms mit dem Raster um, der Kern klemmt vorläufig (`VT` R15 Punkt 8), die `Ausgabe` des Trägers trägt den Wert des Rasters, der Kern übernimmt ihn, und das Abbild zeigt ihn (TR3). Fälle `tr3_cursor_schrumpf*`, Mutant `cursor_schrumpf_klemmt`.
18. Cursorabfrage (A109, Projektentscheid, 18.11.2026): Der Träger beantwortet `ESC[6n` selbst aus seinem Cursor plus 1, genau einmal je Abfrage, in der Reihenfolge der Abfragen; die gleichlautende Antwort der Bibliothek verwirft er (TR14). Im Kindteil gehen die Antworten sofort an die PTY.

19. (TR15, Nachtrag 20.11.2026, Auftrag) Umgebung genau (auch ohne `SHELL`), `cwd` kein Ordner ist ein Fehler des Starts, `Tasten` blockiert nie, Grenze der Schlange 262 144 Byte (TR15). Der Bau bringt `SHELL` weg, indem er das Kind als `env -u SHELL <Programm>` startet (`env` ersetzt sich durch das Programm: Prozessnummer, Sitzung und PTY bleiben); das ist eine Wahl des Baus, der Vertrag verlangt nur das Ergebnis.

## 8. Gegenproben (Mutanten)

Jeder Mutant schaltet sich in der Referenz mit `REF_GEGENPROBE=<name>` zu. `gegenprobe.sh` baut die Referenz bei Bedarf, prüft, dass diese Liste und `tr-ref-start --gegenproben` gleich sind und dass jeder Mutant mindestens einen Fall rot macht.

| Mutant | Regel | Änderung |
|---|---|---|
| `zeile_leerzeichen_ende_bleiben` | TR1 | Leerzeichen im Vorgabestil am Zeilenende bleiben |
| `zeile_spacer_als_leerzeichen` | TR1 | die zweite Zellhälfte eines breiten Zeichens steht als Leerzeichen im Text |
| `zeile_zerowidth_verloren` | TR1 | kombinierende Zeichen fehlen |
| `zeile_unterstrich_fehlt` | TR1 | das Attribut unterstrichen (Bit 8) fehlt |
| `zeile_benannt_hell_aus` | TR1 | helle benannte Farben werden zur Vorgabe |
| `zeile_indexed_ohne_eins` | TR1 | Indexfarbe `i` wird `i` statt `i + 1` |
| `hautton_getrennt` | TR11 | das Hauttonzeichen ist wie in `alacritty_terminal` ein breites Zeichen |
| `hautton_nicht_zurueck` | TR11 | der Platzhalter des Hauttons erscheint im Text statt des Zeichens |
| `hautton_rest_verloren` | TR11 | ein halbes UTF-8-Zeichen am Ende eines Stücks geht verloren |
| `ausgabe_alle_zeilen` | TR2 | jede Ausgabe nennt alle Zeilen |
| `ausgabe_ohne_aenderung` | TR2 | auch ohne Änderung geht eine Ausgabe hinaus |
| `ausgabe_vor_ereignis_fehlt` | TR2 | vor einem Ereignis keine Ausgabe der bisher geänderten Zeilen |
| `stand_nicht_steigend` | TR2 | `stand` bleibt bei 1 |
| `groesse_ohne_ausgabe` | TR3 | nach einer Größenänderung keine Ausgabe |
| `groesse_nicht_gekuerzt` | TR3 | die Zeilen des Kerns werden bei Verkleinern nicht gekürzt |
| `invers_ignorieren` | TR5 | `?5` meldet nichts |
| `invers_ohne_stand` | TR5 | `Modus` mit `invers` ohne `stand` |
| `klammer_ignorieren` | TR5 | `?2004` meldet nichts |
| `modus_mehrfach_nur_erster` | TR5 | von mehreren Parametern einer Folge zählt nur der erste |
| `modus_text_suche` | TR5 | Modi werden per Textsuche im Stück gefunden |
| `sync_puffert` | TR6 | Bytes zwischen SyncBeginn und SyncEnde wirken erst am Ende |
| `sync_ende_fehlt` | TR6 | `?2026l` meldet nichts |
| `osc52_kuerzen` | TR7 | `daten_b64` wird auf 1024 Zeichen gekürzt |
| `osc52_lesen_antwort` | TR7 | eine Leseanfrage wird an die PTY beantwortet |
| `osc52_selektion_pflicht` | TR7 | nur die Auswahl `c` meldet |
| `titel_nur_osc2` | TR8 | OSC 0 meldet keinen Titel |
| `titel_ohne_join` | TR8 | der Titel endet am ersten Semikolon |
| `groessenfolge_ausfuehren` | TR9 | die Größenfolge ändert die Größe des Rasters |
| `groessenfolge_vertauscht` | TR9 | Spalten und Zeilen vertauscht |
| `ruecklauf_vorgabe_10000` | TR4 | die Vorgabe des Rücklaufs ist 10 000 |
| `ruecklauf_ignoriert` | TR4 | die Einstellung `ruecklauf` wirkt nicht (immer 2000) |
| `ruecklauf_zaehlt_sichtbar` | TR4 | der Rücklauf hält eine Zeile mehr als eingestellt |
| `antworten_verschlucken` | TR10 | Antworten auf Geräteabfragen gehen nicht an das Kind |
| `kindende_code_plus_eins` | TR12 | `Kindende` meldet den Code plus 1 |
| `env_erweitert` | TR12 | das Kind bekommt eine zusätzliche Umgebungsvariable |
| `groesse_pty_nicht_gesetzt` | TR12 | die Größe der PTY ändert sich nicht |
| `tasten_lf` | TR12 | CR wird auf dem Weg zur PTY zu LF |
| `cursor_fehlt` | TR2 | `Ausgabe` trägt kein Feld `cursor` |
| `cursor_ohne_ausgabe` | TR2 | ein Cursorschritt ohne Zeilenänderung löst keine `Ausgabe` aus (der Cursor kommt erst mit der nächsten) |
| `cursor_sichtbar_immer` | TR2 | `sichtbar` ist immer wahr (DECTCEM wirkt nicht) |
| `cursor_spalte_plus_eins` | TR2 | die Spalte wird um 1 zu groß gemeldet |
| `cursor_zeile_mit_ruecklauf` | TR2 | die Zeile zählt den Rücklauf mit |
| `cursor_groesse_nicht_geklemmt` | TR3 | der gemeldete Cursor wird bei einer Größenänderung nicht auf die neue Größe geklemmt |
| `cursor_anfang_unsichtbar` | TR2 | der Anfangscursor gilt als unsichtbar gemeldet |
| `fe0f_ignoriert` | TR11 | U+FE0F wird vor dem Raster verworfen |
| `fe0e_ignoriert` | TR11 | U+FE0E wird vor dem Raster verworfen |
| `zwj_ignoriert` | TR11 | U+200D wird vor dem Raster verworfen |
| `kindende_signal_ohne_128` | TR12 | bei einem Signal meldet `Kindende` nur die Signalnummer |
| `kindende_signal_null` | TR12 | bei einem Signal meldet `Kindende` den Code 0 |
| `grenze_fehlt` | TR13 | eine Spalte wird angenommen und erreicht das Raster (Panik bei einem breiten Zeichen, `Term::resize` endet nicht; die Referenz beendet sich dann nach 1,5 s mit Exitcode 3) |
| `grenze_start_fehlt` | TR13 | nur beim Start (`groesse` des Falls) wird eine Spalte angenommen |
| `grenze_groesse_fehlt` | TR3 | nur bei einem Schritt `Groesse` wird eine Spalte angenommen |
| `grenze_alt_fehlt` | TR3 | nur bei eingeschaltetem Alternativschirm wird eine Spalte angenommen |
| `grenze_verschoben` | TR13 | die kleinste Größe ist 3 Spalten (2 Spalten werden abgelehnt) |
| `grenze_zeilen_verschoben` | TR13 | die kleinste Größe ist 2 Zeilen (eine Zeile wird abgelehnt) |
| `groesse_pty_vor_pruefung` | TR12 | die PTY nimmt eine abgelehnte Größe an, das Raster behält seine alte |
| `cursor_schrumpf_klemmt` | TR3 | beim Schrumpfen der Spalten wird der Cursor des Rasters auf die neue Größe geklemmt statt mit dem Raster umgebrochen |
| `utf8_stueck_nicht_gehalten` | TR2 | ein UTF-8-Zeichen, das an einer Stückgrenze geschnitten ist, wird nicht zurückgehalten, sondern roh weitergegeben |
| `antwort_titel_text` | TR10 | auf `ESC[21t` und `ESC[20t` antwortet der Träger mit dem gesetzten Titel |
| `antwort_farbe_text` | TR10 | auf eine Farbabfrage mit `?` antwortet der Träger mit dem gesetzten Farbtext |
| `antwort_decrqss_text` | TR10 | auf `DCS $ q <text> ST` antwortet der Träger mit dem Text |
| `cpr_keine_antwort` | TR14 | die Cursorabfrage bleibt ohne Antwort (der Träger antwortet nicht, die Antwort der Bibliothek ist verworfen) |
| `cpr_doppelt` | TR14 | die Antwort der Bibliothek wird nicht verworfen: zwei Antworten je Abfrage |
| `cpr_vertauscht` | TR14 | Zeile und Spalte vertauscht |
| `cpr_nullbasiert` | TR14 | Zeile und Spalte ab 0 statt ab 1 |
| `cpr_lage_stueckende` | TR14 | die Lage ist die am Ende des Stücks, nicht beim letzten Byte der Abfrage |
| `antworten_kind_verschlucken` | TR14 | im Kindteil gehen die Antworten nicht an die PTY (Stand der Referenz vor A109) |
| `env_shell` | TR15 | das Kind bekommt `SHELL` zusätzlich (Verhalten von `portable-pty`) |
| `shell_gegeben_verloren` | TR15 | eine `SHELL` im Fall fehlt im Kind |
| `cwd_rueckfall` | TR15 | ein `cwd`, das kein Ordner ist, fällt still auf das Heimverzeichnis zurück |
| `schreiben_blockiert` | TR15 | `Tasten` schreibt sofort und wartet auf die PTY (der Fall hängt; der Läufer meldet das Zeitlimit) |
| `schlange_ohne_grenze` | TR15 | die Schlange hat keine Grenze, ein zu großes Schreiben wird angenommen |
| `schlange_grenze_klein` | TR15 | die Grenze ist 262 143 Byte |
| `schlange_grenze_gross` | TR15 | die Grenze ist 262 145 Byte |
| `schlange_voll_still` | TR15 | ein abgelehntes Schreiben meldet `ok: true` |
| `mal_ignoriert` | TR15 | `mal` zählt nicht, die Bytes gehen nur einmal hinaus |

## 9. Dateien

- `referenz/tr-ref/` (Rust-Projekt mit eigener `Cargo.lock`, nicht Teil von `rust/`), Starter `referenz/tr-ref-start` (baut bei Bedarf mit `cargo build --release --offline`; `TR_REF_BIN` oder `TR_REF_ZIEL` ändern den Ort).
- `faelle_bauen.py`: schreibt beide Sätze; jede Erwartung entsteht aus der Referenz; danach prüft es jede Zeile mit dem anderen Vertrag (`RA_REF`) und die Hashes.
- `aussentests/{faelle,erwartet}/` (sichtbar, Datensatz A) und `verdeckt/{faelle,erwartet}/` (Datensatz B: andere Wörter, Größen, Längen, Argumente).
- `test_aussen.py`: Läufer mit `TR_BIN` und `TR_FAELLE` (liest seit Nachtrag 21.11.2026 `system` und `last`, TR16). `laeufer_selbsttest.py` (TR16): Selbsttest des Läufers mit Mutanten (Abschnitt 13). `gegenprobe.sh`: Selbsttest des Läufers, Grundlauf und Mutanten.
- `werkzeuge/klassen.py`: sortiert die alten Erwartungen (Schnappschuss vor dem Nachtrag) in „gleich“, „nur Cursor“, „Cursor und Zahl der Ausgaben“ und „andere“ (Nachtrag 19.11.2026); `werkzeuge/zufallsprobe.py`: Zufallsprobe mit festem Samen gegen Absturz und Endlosschleife bei 2 Spalten, dazu Versuche mit einer Spalte, die abgelehnt werden müssen (`zufallsprobe.py <tr-ref> <Zahl> <Samen>`; Nachtrag 20.11.2026).
- `werkzeuge/utf8vergleich.py` (Nachtrag 20.11.2026): Vergleich von `tr-ref` mit dem Programm `tr` des Baus an Zufallsfällen mit Stückgrenzen mitten im UTF-8-Zeichen, Größen ab 2 Spalten (`utf8vergleich.py <tr-ref> <tr> <Zahl> <Samen> [--ungueltig]`; erwartet 0 verschiedene Fälle).
- `stichprobe.py`: Vergleich der Referenz mit der Probe; `werkzeuge/probevergleich/` ist das kleine Rust-Programm dazu (`src/schirm.rs` aus `the previous implementation` unverändert hineinkopieren, `cargo build --release`).

## 10. Offene Punkte

- **Variantenwahl und Zeichenverbindung** (U+FE0F, U+FE0E, U+200D): entschieden (Projektentscheid, 19.11.2026): kein eigener Tisch, es gilt `alacritty_terminal` 0.26.0; der Cursor-Nachtrag hat das Verhalten in Fällen festgeschrieben (TR11). `☺️` belegt dort eine Zelle, nicht zwei; P11 (24 von 24) nennt die Aufnahmen, die hier abweichen, als benannte Abweichung.
- **Zeitverhalten und Speicher** sind nicht Teil der Außentests: P7 (höchstens 11,4 MiB je Sitzung bei 2000 Zeilen), Durchsatz (T5) und T2 (10 000 Tastenfolgen unter Dauerausgabe) misst ein späterer Schritt an dem gebauten Crate. Die Fälle mit 5000 und 3000 Zeilen tragen nur A119.
- **Kind beendet durch Signal:** festgelegt als 128 plus Signalnummer (Projektentscheid, 19.11.2026), Fälle seit dem Cursor-Nachtrag (TR12). Offen bleibt, wie ein Bau die Nummer aus `portable-pty` liest: Dessen Text ist je System verschieden (macOS `Killed: 9`, Linux `Killed`).
- **ConPTY:** Der Vertrag und die Referenz laufen auf Unix. Die Antwort `ESC[1;1R` für ConPTY beim Start (Messplan Abschnitt 13) und `portable-pty` unter Windows sind nicht Teil der Fälle. (A109) Die Regel dafür steht jetzt in TR14 und gilt auf allen Plattformen; geprüft wird sie unter Windows mit dem Bau in der VM (A109 erster Fall), nicht mit diesen Fällen.
- **`alacritty_terminal` 0.26.0 ist gepinnt.** Eine andere Fassung kann andere Antworten (TR10) oder Raster (TR1) geben; dann ist die Referenz neu zu erzeugen.
  Gemessen 19.11. (Cursor-Nachtrag, Auflage): Eine neuere Fassung gibt es auf crates.io nicht. Der Index (`index.crates.io/al/ac/alacritty_terminal`) und die Programmierschnittstelle von crates.io führen 0.26.0 (6.4.2026) als neueste, davor `0.26.0-rc1`, `0.25.1`, `0.25.0`. Eine Wegwerf-Probe auf dem Linux-Baurechner
  (`cargo` 1.99.0, Abhängigkeit `alacritty_terminal = "*"`, aufgelöst zu 0.26.0) zeigt beide Fehler an der neuesten Fassung: ein breites Zeichen in einem `Term` mit einer Spalte gibt die Panik `index out of bounds: the len is 1 but the index is 1` bei
  `grid/mod.rs:439:30`; `Term::resize` auf eine Spalte mit einem breiten Zeichen im Raster endet nicht und belegt Speicher, bis die Zuweisung scheitert (`memory allocation of 144 bytes failed` bei 2 GB Grenze, vor Ablauf der 5 s). Der Schutz des Nachtrags 19.11. war also nötig; seit dem Nachtrag 20.11.2026 ersetzt ihn das Minimum von 2 Spalten (Punkt 14), und der Bau reicht keine Größe mit einer Spalte an die Bibliothek. Nichts davon ging nach außen (kein Bericht, kein Issue).
  Die Probe lief ohne die Quelle des Projekts zu ändern. Der Pin bleibt.

## 11. Pflichten des Baus außerhalb der Außentests

- T1 (Plan der Ansicht): `the previous implementation` gegen den Träger auf allen 24 Aufnahmen; Ziel 24 von 24 mit dem Sonderfall (Abschnitt 10 nennt, was offen bleibt).
- T2 und A119 an eigenen Kindprogrammen, P7 an einer Sitzung mit 2000 Zeilen Rücklauf.
- **Pflichten des Baus (Nachtrag 20.11.2026, Auflagen, 20.11.2026):** `schmal`, `schmal_rest`, `breite_entfernen` und `entbreiten` entfallen. Der Träger lehnt eine Größe mit einer Spalte ab (`Raster::neu`, `Raster::groesse`, `Kind::starten`, `Kind::groesse` liefern `Err(GroesseUngueltig)`; Spalten 2 bis 1000, Zeilen 1 bis 1000), klemmt nie still und ändert dabei weder Raster noch PTY.
  Ein unvollständiges UTF-8-Zeichen am Stückende wird bei jeder Spaltenzahl zurückgehalten. Das Programm `tr` setzt `versuch` um (Abschnitt 3). **Auflage (c):** `unicode-width` wird aus `rust/crates/traeger/Cargo.toml` erst entfernt, wenn die cargo-Tore (`gates.sh`) ohne die Abhängigkeit grün sind; die Stückliste (SBOM) und die README ziehen danach nach.
  Danach `cargo deny` (Abschnitt unten) und die beiden Fallsätze (sichtbar und verdeckt) grün; der Lauf gegen den Bau vom 20.11. nennt die Fälle, die der heutige Bau nicht besteht (Ergebnisdatei).
- Kein `unsafe`, keine Panik auf Eingaben, `cargo deny` nach `rust/deny.toml` mit den im Entwurf (Abschnitt 2) genannten benannten Ausnahmen (`Unicode-3.0` für `unicode-ident`, `skip` für drei doppelte Namen, je mit Begründung).

## 12. Projektentscheide zu den Fragen zu ConPTY (20.11.2026, Schritt II)

Die Fragen betreffen ConPTY. Gebaut wird
davon erst nach M1 oder mit dem Wort.

1. **A109 erster Fall, Grenze 10 s:** Das Kriterium gilt in der ruhigen VM (wie A113), nicht nach 11 Minuten Last. Gemessen sind 16 von
   18 Läufen unter 10 s; die zwei Ausreißer (11,6 s kalt, 14,2 s) lagen in Blöcken nach langer Last. Der nächste Windows-Lauf misst fünf Läufe in ruhiger VM.
   Die bekommt das als Hinweis zu A109; widerspricht sie, entscheidet das Projekt nach erneuter Messung.
2. **W7 (Vorspann-Titel von ConPTY):** gehört in den Träger, weil nur er den Vorspann sieht. Regel und Fall kommen als Nachtrag in diesen Vertrag, sobald
   Schritt II beginnt.
3. **`Kindende` unter ConPTY (A64):** ein eigener Bauauftrag vor jedem weiteren Windows-Lauf der Kindteil-Fälle. Vorschlag des Baus: `Kind` prüft unter
   Windows mit `try_wait`, schließt nach dem Ende des Prozesses die Pseudokonsole und meldet dann `Kindende`.
4. **Kindprogramme mit `stty` und `sh`** (`groesse`, `eingabe`, `signal`, `cursor`) bleiben Linux-Fälle. Unter Windows misst der Lauf nur den Rest; kein
   `unsafe` und keine neue Abhängigkeit für Windows-Fassungen.

**Projektentscheid, 21.11.2026 (TR16):** Punkt 1: das Kriterium gilt für die ruhige VM; unter Last darf nur die Zeit wachsen (Fall `tr14_cpr_unter_last`). Punkt 2: W7 steht jetzt als Regel und Fälle im Vertrag (TR16). Punkt 3: `Kindende` unter ConPTY ist der eigene Auftrag A64. Punkt 4: die Kindprogramme mit `stty` und `sh` bleiben Fälle für Unix-Systeme und tragen die Markierung `system` (TR16).

## 13. Gegenproben des Läufers (Nachtrag 21.11.2026, TR16)

`laeufer_selbsttest.py` ändert den Text einer Kopie von `test_aussen.py` (jeder Mutant genau eine Stelle) und fährt dieselben Prüfungen wie am unveränderten Läufer; jeder Mutant muss mindestens eine Prüfung rot machen. `gegenprobe.sh` führt den Selbsttest aus und prüft, dass diese Liste und `laeufer_selbsttest.py --liste` gleich sind.

| Läufer-Mutant | Änderung |
|---|---|
| `na_zaehlt_gruen` | ein nicht anwendbarer Fall zählt als grün |
| `na_wird_gefahren` | ein Fall, der hier nicht gilt, wird trotzdem ausgeführt |
| `na_zaehlt_rot` | ein nicht anwendbarer Fall zählt als rot |
| `system_umgekehrt` | die Prüfung des Systems ist umgekehrt |
| `na_exit_null` | gilt kein ausgewählter Fall hier, endet der Läufer mit 0 statt 2 |
| `na_datei_falsch` | `TR_NA` nennt die roten statt der nicht anwendbaren Fälle |
| `last_ohne_freigabe` | ein Fall mit `last` läuft auch ohne `TR_LAST=1` |
| `last_ignoriert` | das Feld `last` startet keine Lastprozesse |
| `last_bleibt` | die Lastprozesse bleiben nach dem Fall |
| `last_zu_klein` | es läuft ein Lastprozess zu wenig |
| `last_ohne_elternpruefung` | die Lastschleife prüft ihren Elternprozess nicht (nach SIGKILL des Läufers bleiben sie) |
| `last_ohne_hoechstdauer` | die Lastschleife hat keine Höchstdauer |

## Nachtrag 21.11.2026 (Zeitannahmen): Warten auf den Cursor

`tr15_kind_schreiben_nie_blockiert` war unter Last einmal rot (`$.ende.cursor[0]: 0 statt 1`): Der Fall wartete nur auf den Text `BEREIT`, der Zeilenvorschub dahinter war noch nicht gelesen, der Cursor stand auf Zeile 0. Gleich gebaut waren fünf weitere Fälle des Kindteils (`tr12_kind_eingabe`, `tr12_kind_groesse`, `tr12_kind_groesse_abgelehnt`, `tr15_kind_schlange_grenze`, `tr15_kind_schlange_voll`). Alle sechs warten jetzt auf den Text `BEREIT` und danach auf den Cursor `[1, 0, true]`; die Erwartungen ändern sich nur um das Ergebnis des zusätzlichen Schritts. Neu sind vier Fälle der Bedingung `cursor` (`tr15_warte_cursor_zeile`, `tr15_warte_cursor_nie`, `tr15_warte_cursor_form`, `tr15_warte_cursor_typ`): 130 sichtbar, 95 verdeckt. Die Referenz `tr-ref` und das Programm `tr` lesen die Bedingung gleich.

## Nachtrag 25.11.2026 (Windows): `Kind::beenden` und der Code eines beendeten Kindes

Stand: abgenommen 25.11.2026. Unter Windows gibt es kein SIGHUP; der Dienst kann ein Kind nur über den Träger beenden.

1. **`Kind::beenden`** (öffentlich, alle Systeme) beendet ein laufendes Kind über `ChildKiller::kill` von `portable-pty` 0.9.0 und wartet nicht auf das Ende der Ausgabe; `Kindende` folgt wie bei jedem anderen Ende über `lies` (TR2). Ein Kind, das schon geendet hat oder dessen Ausgabe zu Ende ist, bleibt unberührt. Das Crate bleibt ohne `unsafe`.
   - Unix: SIGHUP, bis zu 200 ms Nachfragen, dann SIGKILL (`portable-pty` `lib.rs:340-373`); `Kindende` trägt wie immer 128 plus das Signal (129 oder 137). Der Dienst nutzt unter Unix weiter seine eigene Folge (DI12 Punkt 6).
   - Windows: `TerminateProcess` mit dem Code 1 (`win/mod.rs:41-57`); ein Kind kann es nicht abfangen.
2. **Code unter Windows (TR12).** Ein Kind, das `Kind::beenden` beendet hat und mit dem Code 1 von `TerminateProcess` endet, meldet `Kindende` mit dem Code 129 (128 plus 1, wie das Ende durch SIGHUP unter Unix). So sieht ein verlangtes Ende auf jedem System gleich aus, und die Fälle des Dienstes, die nach `Beenden` 129 erwarten, gelten unverändert. Ein Kind, das im selben Augenblick von selbst mit 1 endet, meldet ebenfalls 129; der Fall wird nicht unterschieden. Ohne `Kind::beenden` bleibt der Code der Exitcode.
3. **`Kind::prozess_griff`** (nur Windows) gibt den Griff des Prozesses für ein Job-Objekt des Dienstes; er gilt, solange der Wert `Kind` lebt.
4. **`Kind::prozess_beendet`** (nur Windows) fragt ohne Warten, ob der Prozess des Kindes geendet hat (`Child::try_wait`, unter Windows `GetExitCodeProcess`); `Kindende` kann noch ausstehen, und der Prozess wird dabei nicht eingesammelt. Der Dienst fragt es, wenn ein Kind nicht in seinen Job kommt (Vertrag `dienst`, Abschnitt 21a Punkt 4).
