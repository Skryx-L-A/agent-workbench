# Vertrag `zustandskern`

Stand 20.11.2026 (Nachtrag 20.11.2026, Projektentscheid, 20.11.2026: kleinste Größe 2 Spalten in R3, `anfang`, `Abo`, `Groesse`; der Kern übernimmt nach einer Größenänderung den Cursor des Trägers, R15 Punkt 8; jede Änderung trägt „(Nachtrag 20.11.2026)“);
Stand davor 19.11.2026 (Nachtrag Auflagen, 18.11.2026: R13 inverser Bildschirm, Obergrenze der Bildrate in R5; Nachtrag 19.11.2026: R14 Rückstaugrenze;
Nachtrag 19.11.2026 (Projektentscheid, Cursor-Nachtrag): R15 Cursor, mit der Auflage, 19.11.2026, dass reine Cursorschritte den Deckel `bildrate_max` nicht umgehen).
Ausführbarer Vertrag der Logik des Zustandskerns der neuen Oberfläche.
Grundlage ist der angenommene Entwurf des Zustandskerns mit den acht Projektentscheiden (18.11.2026, Abschnitt 7).
Gebaut ist nur die Python-Referenz. Ein Rust-Bau gilt als vertragstreu, wenn er beide Fallsätze grün besteht.

Quellen stehen als `datei:zeile`. Ohne Pfad liegen sie außerhalb des Repos:
`E` = der Entwurf des Zustandskerns, `A` = die Anforderungsliste, `M` = der Messplan des Terminalträgers,
`I` = die Inventur der bisherigen Oberfläche, `P` = der Gesamtplan. Zeilennummern gelten für den Stand vom 18.11.2026.
`RA` = Messung des Rasterauszugs (außerhalb des Repos, Rohdaten und Läufer unter
`spec/rasterauszug/`) und die Auflagen dazu (außerhalb des Repos).
`RS` = Messung der Rückstaugrenze (außerhalb des Repos, Läufer und Rohdaten unter `spec/rasterauszug/`, Abschnitt „Rückstaugrenze 19.11.2026“).
`T` = Messung der Tasten, Modi, Dialogantworten und Einfügungen (Nachtrag 18.11.2026):
außerhalb des Repos, (Abschnitte 1 bis 4) mit den Rohdaten unter
`the previous implementation` im Zweig (Stand vom 18.11.2026; Claude Code 2.1.327, codex 0.169.0, tmux 3.8).
„(H)“ heißt hergeleitet; „(V)“ heißt im Vertrag festgelegt, siehe Abschnitt „Festgelegt im Vertrag“.

## 1. Schnitt

Der Vertrag beschreibt nur die Logik: Ereignisse hinein, Wirkungen hinaus (`E:183`). Er beschreibt weder Transport noch
Zeichnen noch PTY noch Bildschirmspeicher. Das Raster liefert der Träger als fertige Zeilen; der Kern parst keine
Steuerfolgen (`E:116`). Es gibt keine Systemzeit und keinen Zufall. Die einzige Uhr ist das Feld `t` jedes Ereignisses.

## 2. Aufruf

- `zk <fall.json>` oder `zk < fall.json` (auch `zk -`). Das Programm liest genau einen Fall.
- Exitcode 0: genau ein JSON-Objekt auf stdout, UTF-8. Schlüsselreihenfolge und Leerraum sind frei; Listenreihenfolge zählt.
- Exitcode 2: Der Fall ist ungültig (kein JSON, kein Objekt, `ereignisse` fehlt, `t` fehlt, ist keine ganze Zahl oder fällt).
  stdout bleibt leer; eine Meldung geht nach stderr.
- Grenzen des Programms `zk` (Projektentscheid, 18.11.2026): höchstens 4 194 304 Byte Eingabe, Verschachtelung höchstens 128 Ebenen
  (Grenze von `serde_json`). Mehr ist ein ungültiger Fall mit Exitcode 2. Keine Eingabe löst eine Panik aus. Die Referenz
  prüft diese Grenzen nicht; kein Fall der beiden Sätze kommt in ihre Nähe.
- Ausgabegrenze von `zk` (19.11.): Die Nachrichten an Betrachter haben zusammen höchstens 33 554 432 Byte
  (serialisiert); darüber endet der Fall mit Exitcode 2. Grund: Eine Eingabe von 4 MiB könnte mit vielen Betrachtern sonst
  Gigabyte Ausgabe erzeugen. Die Referenz prüft diese Grenze nicht.
- Ganze Zahlen liegen im Bereich von `i64`. Eine größere Zahl gilt nicht als ganze Zahl (für `t`: Exitcode 2). `NaN`,
  `Infinity` und Zahlen außerhalb von `f64` sind kein gültiges JSON (Exitcode 2). Fehlerhafte Teile in `anfang` (etwa
  Nicht-Text in `zeilen`, Größe außerhalb von Spalten 2 bis 1000 und Zeilen 1 bis 1000 (Nachtrag 20.11.2026; vorher 0 bis 1000, die Referenz prüfte es nicht), Regel mit falscher Art) enden mit Exitcode 2.
- Der Läufer `test_aussen.py` schickt den Fall auf stdin und vergleicht Exitcode und geparstes JSON mit `erwartet/<fall>.json`.

## 3. Fall

```json
{"name": "…", "regeln": ["R2"], "beschreibung": "…",
 "anfang": {
   "personen":  {"<person>": {"rolle": "admin|nutzer", "art": "mensch|agent"}},
   "nachweise": {"<nachweis>": {"person": "<person>"}},
   "routen":    [{"name": "…", "status": "frei|gesperrt|evaluierung", "grund": "…", "zone": "…"}],
   "sitzungen": [{"id": "…", "titel": "…", "route": "…", "kundenraum": "…", "person": "<besitzer>",
                  "groesse": [spalten, zeilen], "bracketed_paste": false, "zeilen": ["…"], "stand": 0}],
   "worker":    [{"id": "…", "sitzung": "…"}],
   "regeln":    {"<schluessel>": {"art": "stufen|zahl", "stufen": ["locker", "…", "streng"], "vorgabe": …,
                                  "nutzer_darf": true, "firma": null, "gruppe": null, "nutzer": {"<person>": …}}}},
 "ereignisse": [{"t": 0, "art": "Verbinden", "…": "…"}]}
```

- `name`, `regeln` und `beschreibung` sind Kommentar; das Programm wertet sie nicht aus.
- Die `groesse` der Sitzung ist eine Liste aus zwei ganzen Zahlen: Spalten 2 bis 1000, Zeilen 1 bis 1000; sonst ist der Fall ungültig (Exitcode 2; Nachtrag 20.11.2026, Projektentscheid, 20.11.2026: JA, der Kern prüft die Größe in `anfang`; eine Spalte ist ungültig).
- Fehlende Felder der Sitzung: `titel` "", `groesse` [80, 24], `bracketed_paste` false, `zeilen` leer, `stand` 0.
  Die Zeilenliste wird auf die Zeilenzahl der Größe mit "" aufgefüllt oder gekürzt.
- Eingebaute Regeln (V): `osc52_schreiben` (art `stufen`, Stufen `["an", "aus"]`, Vorgabe `"an"`, `nutzer_darf` true) und
  `osc52_grenze` (art `zahl`, Vorgabe 100000, `nutzer_darf` true). Ein Eintrag in `anfang.regeln` überschreibt einzelne Felder.
  Grundlage: `A:223` (Vorgabe „an mit Hinweis“, Grenze 100 000 Byte nach dem Dekodieren).
- Eingebaute Regel `bildrate_max` (V, Nachtrag 19.11.2026): art `zahl`, ganze Zahl ab 1, Bilder je Sekunde je Sitzung, Vorgabe **63**,
  `nutzer_darf` true. Beschreibung in R5; Wahl der Vorgabe in Abschnitt 7, Punkt 18.
- Eingebaute Regel `rueckstau_max` (V, Nachtrag 19.11.2026): art `zahl`, ganze Zahl ab 1, Byte je Betrachter, Vorgabe **16384**, `nutzer_darf` true.
  Beschreibung in R14; Wahl der Vorgabe in Abschnitt 7, Punkt 22.
- Das Feld `invers` der Sitzung (bool, Vorgabe false) ist der Stand von DECSCNM (R13).
- Das Feld `cursor` der Sitzung (Nachtrag 19.11.2026, R15) ist optional: `[zeile, spalte, sichtbar]` mit ganzen Zahlen und einem Wahrheitswert, `zeile` kleiner als die Zeilenzahl und `spalte` kleiner als die
  Spaltenzahl der `groesse` der Sitzung. Ohne das Feld kennt der Kern keinen Cursor. Ein Feld in falscher Form oder außerhalb der Größe (auch `null`) ist ein ungültiger Fall (Exitcode 2). Zweck: Der Dienst
  baut den Kern nach einem Fehler neu (Punkt 27) und gibt den zuletzt bekannten Cursor aus `zustand` mit.

## 4. Ereignisse hinein

Jedes Ereignis hat `t` (ganze Millisekunden, nicht fallend) und `art`. Vor jedem Ereignis laufen alle Zeitgeber mit Zeit
höchstens `t` ab, in Reihenfolge ihrer Zeit, bei gleicher Zeit in Reihenfolge ihres Anlegens (V).

| Von | `art` | Felder |
|---|---|---|
| Ansicht | `Verbinden` | `betrachter`, `person`, `raum` (Kundenraum), `ansicht` (`fenster`, `browser`) |
| Ansicht | `Trennen` | `betrachter` |
| Ansicht | `RaumWechsel` | `betrachter`, `raum` |
| Ansicht | `Abo` | `betrachter`, `sitzung`, `an` (Vorgabe true), bei `an` auch `spalten` (2 bis 1000), `zeilen` (1 bis 1000) (gemeldete Größe; Nachtrag 20.11.2026) |
| Ansicht | `Groesse` | `betrachter`, `sitzung`, `spalten` (2 bis 1000), `zeilen` (1 bis 1000) (neue gemeldete Größe; Nachtrag 20.11.2026) |
| Ansicht | `Text` | `betrachter`, `sitzung`, `text` (nach Komposition und toten Tasten) |
| Ansicht | `Taste` | `betrachter`, `sitzung`, `taste`, `umschalter` (Liste aus `umschalt`, `strg`, `alt`) |
| Ansicht | `Einfuegen` | `betrachter`, `sitzung`, `text` |
| Ansicht | `Antwort` | `betrachter`, `sitzung`, `dialog` (ID), `wahl` (1 bis 9) |
| Ansicht | `Freigabe` | `betrachter`, `anfragen` (Liste von IDs), `nachweis` |
| Ansicht | `RouteWaehlen` | `betrachter`, `sitzung`, `route` |
| Ansicht | `EinstellungSetzen` | `betrachter`, `schluessel`, `ebene` (`firma`, `gruppe`, `nutzer`), `wert` |
| Träger | `Ausgabe` | `sitzung`, `stand` (ganze Zahl, steigend), `zeilen` (Objekt Zeilenindex → Text), optional `cursor` (`[zeile, spalte, sichtbar]`, R15) |
| Träger | `SyncBeginn`, `SyncEnde` | `sitzung` (DEC 2026 begonnen oder beendet) |
| Träger | `Modus` | `sitzung`, `bracketed_paste` (bool) und/oder `invers` (bool, dann auch `stand`); keines von beiden: `ereignis_ungueltig` (R13) |
| Träger | `Socket` | `betrachter`, `ausstehend` (ganze Zahl ab 0: nicht abgenommene Bytes dieses Betrachters, R14) |
| Träger | `Titel` | `sitzung`, `text` (aus OSC 0 oder 2) |
| Träger | `Kindende` | `sitzung`, `code` |
| Träger | `GroessenFolge` | `sitzung`, `spalten`, `zeilen` (eine `ESC[8;…t` aus dem Ausgabestrom) |
| Träger | `Zwischenablage` | `sitzung`, `daten_b64` (OSC 52 mit Daten) oder `lesen: true` (OSC 52 mit „?“) |
| Kern (Erkennung) | `Dialog` | `sitzung`, `id`, `dialog_art`, `optionen` (Liste) |
| Kern (Erkennung) | `DialogEnde` | `sitzung` |
| Kern (Abfrage) | `AgentAbfrage` | `worker`, `zustand` (`laeuft`, `wartet`, `fertig`, `unbekannt`), `quelle` |
| Kern (Inbox) | `Nachricht` | `sitzung` (Ziel), `text`, `herkunft` (`{"art": "sitzung", "sitzung": …}` oder `{"art": "mensch", "betrachter": …}`) |
| Stelle | `Anfrage` | `id`, `sitzung`, `werkzeug`, `vorschau`, `frist` (absolute Zeit in ms) |
| Gateway | `Nachweis` | `nachweis`, `person` (ein neuer Menschen-Nachweis) |
| Uhr | `Takt` | keine (lässt nur Zeit vergehen) |

Grundlage der Liste: `E:61` bis `E:75`. `Dialog` kommt aus der Erkennung im Kern (`E:130`), `Takt` und `Nachweis` sind (V).
Ein Ereignis mit unbekannter `art` wird mit `ereignis_unbekannt` abgelehnt. Ein Feld mit falschem Typ ergibt `ereignis_ungueltig`.

## 5. Wirkungen hinaus

```json
{"nachrichten": {"<betrachter>": [{"t": 0, "art": "…", "…": "…"}]},
 "pty":        [{"t": 0, "sitzung": "…", "hex": "1b0d"}],
 "groessen":   [{"t": 0, "sitzung": "…", "spalten": 0, "zeilen": 0}],
 "ablehnungen":[{"t": 0, "ereignis": 3, "grund": "pane_busy"}],
 "protokoll":  [{"t": 0, "person": "…", "schluessel": "…", "ebene": "…", "alt": null, "neu": "…"}],
 "an_stelle":  [{"t": 0, "anfrage": "…", "person": "…", "nachweis": "…"}],
 "abfragen":   [{"t": 0, "worker": "…"}],
 "zustand":    {"sitzungen": […], "betrachter": {…}, "anfragen": {…}, "agenten": {…}, "regeln": {…}}}
```

- `nachrichten` sind die Nachrichten an jeden Betrachter in Sendefolge. Ein Betrachter, der nie etwas bekam, fehlt.
- `pty` sind die Bytes an die PTY als Kleinbuchstaben-Hex. `groessen` sind die Größenbefehle des Kerns an den Träger.
- `ablehnungen` nennen den Index des Ereignisses (ab 0) und den Grund. Eine Ablehnung ändert keinen Zustand,
  außer wo eine Regel es nennt (Hinweis bei Routen).
- `protokoll` ist die Protokollzeile jeder Regeländerung (`A:68`). `an_stelle` ist die Freigabe an die Entscheidungsstelle.
  `abfragen` sind sofortige neue Abfragen eines Workers (`A:44`).
- `zustand` ist der Endstand: je Sitzung `id`, `titel`, `route`, `kundenraum`, `groesse`, `zustand`, `code`, `stand`,
  `letzte_eingabe`, `dialog`, `bracketed_paste`, `invers`, dazu `cursor`, sobald der Kern einen kennt (R15); je verbundenem Betrachter `person`, `raum`, `abos`, `ausstehend` (letzte Meldung des Trägers), `verfallen` (Zahl der verfallenen Bilder, R14) und
  `rueckstand` (je Sitzung, in der er hinter dem Stand liegt, die Zeilen als sortierte Liste); je Anfrage der Zustand;
  je Worker der letzte gesehene Stand oder null; je Regel `firma`, `gruppe`, `nutzer`.

Nachrichten an Betrachter (alle mit `t` und `art`):

| `art` | Felder | Wann |
|---|---|---|
| `Einstellungen` | `werte`: je Schlüssel `wert`, `vorgabe`, `herkunft` (`vorgabe`, `firma`, `gruppe`, `nutzer`) | beim Verbinden; nach jeder Regeländerung an betroffene Personen |
| `Routen` | `routen`: je Route `name`, `status`, `grund`, `zone`, `waehlbar` | beim Verbinden |
| `Sitzungsliste` | `sitzungen`: je Sitzung des Raums `id`, `titel`, `route`, `groesse`, `zustand`, `code` | beim Verbinden, nach Raumwechsel, je Änderung |
| `Anfragen` | `anfragen`: offene Anfragen des Raums | beim Verbinden, nach Raumwechsel |
| `Agenten` | `agenten`: gesehene Worker des Raums | beim Verbinden, nach Raumwechsel |
| `Leeren` | `raum` | beim Raumwechsel, vor dem neuen Raumbild |
| `Abzug` | `sitzung`, `stand`, `groesse`, `invers`, `zeilen` (alle Zeilen), `cursor` (nur wenn der Kern einen kennt, R15) | beim Abo und nach jeder Größenänderung |
| `Rasteraenderung` | `sitzung`, `von`, `stand`, `invers`, `zeilen` (Index → Text), `cursor` (nur wenn der Kern einen kennt, R15) | gebündelt, siehe R5 |
| `Dialog`, `DialogEnde` | `sitzung`, `id` (dazu `dialog_art`, `optionen`) | an Abonnenten der Sitzung |
| `Anfrage` | `id`, `sitzung`, `werkzeug`, `vorschau`, `frist`, `zustand` | je Änderung, an den Raum der Sitzung |
| `Agentenzustand` | `worker`, `zustand`, `gesehen_um`, `quelle` | je bekanntem neuem Stand, an den Raum |
| `Nachricht` | `sitzung`, `absender`, `text`, `zeit` | an den Raum der Zielsitzung |
| `Zwischenablage` | `sitzung`, `daten_b64` | OSC 52, nur an einen Betrachter |
| `Hinweis` | `hinweis` (`osc52_geschrieben`, `route_gesperrt`, `route_ohne_klasse`), dazu `sitzung`/`laenge` oder `route`/`grund` | je Fall |

Hinweise sind Nachrichten der Art `Hinweis`. Sie halten nichts an und verlangen keine Bestätigung (`A:223`, `E:48`).

## 6. Regeln

### R1 Tastenübersetzung

Die Ansicht schickt nie Bytes; der Kern übersetzt `Text`, `Taste` und `Einfuegen` (`E:90`, `E:140`).
- `Text`: die Zeichenkette als UTF-8. Ein Zeichen unter 0x20 oder von 0x7f bis 0x9f lehnt der Kern mit `steuerzeichen_im_text` ab (V).
  Leerer Text hat keine Wirkung.
- `Taste`: `Enter` wird `0d`, `Enter` mit `umschalt` wird `1b 0d` (`E:126`; gemessen `M:554`, `M:555`; Windows `M:507`).
  Weitere Tasten in der xterm-Normalform ohne Anwendungsmodus, gemessen in Claude Code 2.1.327 und codex 0.169.0 (Beleg `T`):
  `Tab` `09`, Umschalt+`Tab` `1b 5b 5a`, `Escape` `1b`, `Ruecktaste` `7f`, Alt+`Ruecktaste` `1b 7f`, `Entf` `1b 5b 33 7e`,
  `Hoch`/`Runter`/`Rechts`/`Links` `1b 5b 41`/`42`/`43`/`44`, `Pos1` `1b 5b 48`, `Ende` `1b 5b 46`, `BildAuf` `1b 5b 35 7e`,
  `BildAb` `1b 5b 36 7e`; neu in der Tabelle, ebenfalls gemessen (`T`): Strg+`Links` `1b 5b 31 3b 35 44`, Strg+`Rechts`
  `1b 5b 31 3b 35 43`, Alt+`Links` `1b 5b 31 3b 33 44`, Alt+`Rechts` `1b 5b 31 3b 33 43` (beide Programme springen ein Wort).
  Buchstabe `a` bis `z` mit `strg` wird 0x01 bis 0x1a, mit `alt` `1b` plus Buchstabe, mit beiden `1b` plus Steuerbyte (V; gemessen
  `T` sind nur a, b, d, e, f, h, k, u, w, y mit `strg` und b, d, f mit `alt`; `1b 01` blieb in beiden Programmen ohne Wirkung und
  ohne Streuzeichen, es ist also eine bestätigte Nicht-Wirkung, keine Wirkung). Jede andere Taste oder Kombination, auch
  Umschalt+`Links`, Strg+`Hoch` und Strg+Alt+`Links` (nicht gemessen), ein unbekannter oder doppelter Umschalter ergibt `taste_unbekannt`.
- Normalform: Der Kern sendet immer die Normalform, nie Kitty- oder modifyOtherKeys-Folgen (`T`: die Normalform wirkt in beiden
  Programmen auch bei eingeschaltetem Kitty; codex versteht modifyOtherKeys nicht, 18 von 18 Formen ohne Wirkung). Für diese
  beiden Programme braucht es kein Modus-Ereignis für DECCKM: keines schaltet `ESC[?1h` ein (`T`, 10 von 10 Aufbauten), und
  `ESC[x` wie `ESC O x` wirken beide. Programme, die DECCKM einschalten (Shell, Editor), sind nicht gemessen (H: sie brauchten die
  Anwendungsform).
- `Einfuegen`: Die Folgen `ESC[200~` und `ESC[201~` im Text entfernt der Kern (V). Dann hängt die Umwandlung der Zeilenumbrüche vom
  Zustand `bracketed_paste` der Sitzung ab (V, Abweichung von „wie ein Terminal“, Begründung unten):
  - Bracketed Paste an: LF und CRLF werden CR (wie Terminal, `M:541`), CR bleibt CR. Der Kern setzt `ESC[200~` davor und
    `ESC[201~` dahinter, zusammen 12 Byte (`E:128`, gemessen `M:541`). Die Regel bleibt, wie sie war. Gemessen (`T`): Mit Klammern
    ergeben LF, CR und CRLF in beiden Programmen einen Zeilenumbruch, die Wandlung schadet nicht.
  - Bracketed Paste aus: CRLF und CR werden LF, LF bleibt LF; der Kern sendet beim Einfügen nie CR. Keine Klammermarken.
    Grund: Ohne Klammern liest Claude Code ein CR als Enter und sendet sofort ab. Gemessen (`T`, Claude Code 2.1.327, codex 0.169.0,
    je 2 von 2 Wiederholungen): `eins` CR `zwei` ohne Klammern sendet `eins` sofort ab und lässt `zwei` im Eingabefeld; bei CRLF
    ebenso. LF bleibt in beiden Programmen ein Umbruch (`eins\nzwei`). codex macht aus CR einen und aus CRLF zwei Umbrüche
    (`eins\n\nzwei`). Mit der Regel „Einfügen sendet nie ab“ ist LF die einzige Form, die in beiden Programmen gleich wirkt.
    Ein Terminal sendet beim Einfügen ohne Klammern dagegen CR; das ist der Unterschied zu „wie Terminal“.
  Bleibt nach dem Entfernen und Umwandeln nichts übrig, geschieht nichts.
- Eingaben brauchen ein Abo der Sitzung (`kein_abo`) und eine laufende Sitzung (`sitzung_beendet` nach `Kindende`) (V).

### R2 OSC 52

Quelle: `A:223`, `E:121` bis `E:125`, `E:167`, `E:169`, Berichtigung `M:496`.
- Die Grenze ist die wirksame Regel `osc52_grenze` des Sitzungsbesitzers, Vorgabe 100 000 Byte **nach dem Dekodieren**.
  Genau so viele Byte wie die Grenze gehen noch; abgelehnt wird erst ein Inhalt, der größer ist. Begründung: `A:223` beschreibt den
  Prüffall als Schreibfolge, „deren Inhalt nach dem Dekodieren größer ist als die Grenze“; die Grenze nennt also die größte
  erlaubte Menge. T8 hat 102 480 Byte fehlerfrei getragen (`M:541`), also auch genau 100 000.
- Über der Grenze: Ablehnung `osc52_zu_gross`; kein Inhalt, kein Hinweis, keine Kürzung (`E:122`).
- Einstellung `osc52_schreiben` = `aus`: weder Inhalt noch Hinweis, auch keine Ablehnung („geschieht nichts“, `E:123`).
- Sonst geht `Zwischenablage` nur an den Betrachter mit der letzten Eingabe in dieser Sitzung (`E:167`). „Betrachter der Sitzung“
  sind die Abonnenten (V); alle bekommen den `Hinweis` `osc52_geschrieben` mit `laenge`.
- Hat niemand mit Abo zuletzt eingegeben, gibt es weder Inhalt noch Hinweis: Ablehnung `osc52_kein_empfaenger` (V).
- Ungültiges Base64: `osc52_ungueltig` (V). Base64 gilt streng nach RFC 4648: nur die 64 Zeichen des Alphabets, Auffüllung
  mit `=` genau passend (19.11.; Python 3.15 liest so, Python 3.9 nimmt zu viel Auffüllung an). Eine Leseanfrage (`lesen: true`) beantwortet niemand: keine Bytes an die PTY,
  keine Nachricht; Ablehnung `osc52_lesen` (`E:124`).

### R3 Größe

Nur der Kern setzt die Größe (`A:222`, `E:129`). Es gilt die gemeldete Größe des Betrachters mit der letzten Eingabe (`E:171`).
- Jeder Betrachter meldet je Abo eine Größe (`Abo`, `Groesse`). Nach einer angenommenen Eingabe (auch `Antwort`) setzt der Kern
  die gemeldete Größe dieses Betrachters, wenn sie abweicht. Eine `Groesse` wirkt sofort nur vom Betrachter mit der letzten Eingabe.
- Eine Änderung schreibt `groessen`, kürzt oder füllt die Zeilen, schickt jedem Abonnenten einen neuen `Abzug` und eine `Sitzungsliste` (V).
- `GroessenFolge` aus dem Ausgabestrom wird nie ausgeführt: Ablehnung `groesse_nur_kern` (`A:222`).
- **Kleinste Größe (Nachtrag 20.11.2026, Projektentscheid, 20.11.2026).** Spalten sind ganze Zahlen von **2** bis 1000, Zeilen ganze Zahlen von 1 bis 1000, sonst `groesse_ungueltig` (V); eine Spalte wird wie 0 abgelehnt, in `Abo` und in `Groesse`, und die Größe der Sitzung bleibt.
  Dasselbe gilt für die `groesse` einer Sitzung in `anfang` (ungültiger Fall, Exitcode 2; Projektentscheid, 20.11.2026). Die Größenfolge des Trägers (`GroessenFolge`) wird nie ausgeführt (`groesse_nur_kern`), gleich welche Zahl sie trägt; sie prüft die Zahl nicht. Grund der Grenze: `alacritty_terminal` 0.26.0 stürzt bei einer Spalte ab oder hängt (`traeger` Abschnitt 7 Punkt 14, Issue 9112).

### R4 Abo und Abzug

- `Abo` an: sofort ein voller `Abzug` des freigegebenen Rasters (`E:81`, `E:173`). Danach gehen `Rasteraenderung`en nur an Betrachter
  mit Abo dieser Sitzung (`E:82`). Ein Abo auf eine Sitzung eines anderen Kundenraums: `fremder_kundenraum`.
- Lückenlos: Jede `Rasteraenderung` an einen Betrachter trägt in `von` den Stand, den dieser Betrachter zuletzt bekam, und in `stand`
  den neuen. Ein Betrachter, der den Stand schon hat, bekommt keine Änderung (V).
- Das Abo lebt mit der Verbindung (`E:51`). Nach `Trennen` und neuem `Verbinden` braucht der Betrachter ein neues Abo und bekommt
  wieder einen vollen Abzug, nie Änderungen seit einem alten Stand (`E:173`, Frage 7).
- `Ausgabe` und `Modus` mit `invers` mit `stand` nicht größer als der höchste bekannte Stand: `stand_alt`. Ein Zeilenindex, der keine Dezimalzahl ohne
  führende Null oder nicht kleiner als die Zeilenzahl ist: `zeile_ausserhalb`. Beide Ablehnungen verwerfen das ganze Ereignis (V).

### R5 Bündelung

- Höchstens eine `Rasteraenderung` je Abstand je Sitzung (`E:82`). Der Abstand ist 16 ms oder, wenn größer, der **aufgerundete** Kehrwert der
  wirksamen Regel `bildrate_max` des Sitzungsbesitzers in ganzen ms: `max(16, aufrunden(1000 / bildrate_max))` (V, Nachtrag 19.11.2026).
  Mit der Vorgabe 63 ist er 16 ms; 30 ergibt 34 ms, 10 ergibt 100 ms, 7 ergibt 143 ms. Eine Obergrenze ist keine Bündelung unter 16 ms: ein
  Wert über 63 verkürzt den Abstand nicht. Der Abstand gilt, wie er beim Eintreffen der Änderung ist; ein später geänderter Wert wirkt auf die nächste.
- Die erste Änderung geht sofort; liegt der letzte Versand weniger als einen Abstand zurück, geht die nächste genau einen Abstand nach dem
  letzten Versand, mit allen Zeilen, die seit dem letzten Versand geändert sind (V). Ein Abstand von genau einem Abstand geht sofort.
- Auch eine `Ausgabe` ohne Zeilen (ein reiner Cursorschritt, R15) hat diese Frist und diesen Abstand (Nachtrag 19.11.2026).
- Die Obergrenze zählt Rasteränderungen je Sekunde, nicht Bytes: Sie hält die Rate der Bilder fest, nicht ihre Größe (`RA`, Abschnitt 7 Punkt 18).
- Ein Versand ohne Abonnenten zählt als Versand (V). Jede Sitzung hat ihre eigene Frist.
- Synchronisierte Aktualisierung (DEC 2026): Zwischen `SyncBeginn` und `SyncEnde` geht kein Auszug hinaus; die Ausgaben warten
  (`E:91`, `M:280`, `M:579`). Endet sie nicht, gibt der Kern genau 150 ms nach `SyncBeginn` frei. Freigabe heißt: die wartenden
  Zeilen und der Stand werden übernommen und wie eine neue Ausgabe gebündelt.
- Ein `Abzug` während der Synchronisierung zeigt den zuletzt freigegebenen Stand (V). Ein zweites `SyncBeginn` verlängert nichts;
  ein `SyncEnde` ohne laufende Synchronisierung hat keine Wirkung (V).
- Am Ende eines Falls fällige Zeitgeber laufen nicht von selbst; ein Fall schließt mit `Takt`.

### R14 Rückstaugrenze

Quelle: Projektentscheid, 19.11.2026, Messung `RS`. Die Grenze schützt die Leitung eines Betrachters vor einem Rückstau. Sie zählt Byte; die Obergrenze der Bildrate (R5) zählt Bilder und
kann L4 (große Nachrichten) nicht von L1 und L2 (kleine) trennen (`RA`).

**Meldung des Trägers (Form).** Der Träger meldet den Stand des Sockets je Betrachter als Ereignis `Socket` mit `ausstehend`: die Zahl der Byte, die der Träger für diesen Betrachter
angenommen hat und die der Socket noch nicht abgenommen hat (Sendepuffer der Anwendung plus Sendepuffer des Kernels). Der Träger meldet nach jeder Nachricht, die er aus `nachrichten` an den
Betrachter schreibt, und bei jeder Änderung, solange der Betrachter hinter dem Stand liegt. Bis zur ersten Meldung gilt 0. Begründung:
- Der Kern kennt keine Byte. Kodierung und Kompression gehören dem Träger, eine Summe im Kern bräuchte deren Größen. Der Socket kennt den Wert (im Läufer: `SO_NWRITE` plus Puffer).
- Ein Messwert statt einer Summe: Eine späte oder doppelte Meldung wird von der nächsten überschrieben. Es gibt keinen Zähler, der abdriften kann, und der Kern führt keine Buchführung über Byte.
- Die Regel braucht nur den Vergleich mit der Grenze. Verworfen sind eine Summe der abgenommenen Byte (der Kern müsste die gesendeten Byte selbst summieren) und eine Meldung nur beim Überschreiten
  (der Kern erführe nicht, wann der Wert wieder unter die Grenze fällt).

Die Regeln:
1. **Grenze je Betrachter in Byte, einstellbar.** Die Einstellung `rueckstau_max` (R12) hat die Vorgabe **16384** Byte (Messung `RS`, Abschnitt 7, Punkt 22), den Mindestwert 1 (0 ergibt `wert_ungueltig`),
   und der Nutzer darf sie nur senken. Sie gilt für die Person des Betrachters, nicht für den Besitzer der Sitzung (anders als `bildrate_max`).
2. **Verfall.** Ist ein Bild fällig (der Versand der Sitzung nach R5) und liegt `ausstehend` eines Betrachters über der Grenze (genau die Grenze geht noch), verfällt es für diesen Betrachter:
   keine Nachricht, `von` und Stand des Betrachters bleiben, `verfallen` steigt um 1, die Zeilen des Bildes gehen in seinen Rückstand. Nur `Rasteraenderung` verfällt; jede andere Nachricht geht hinaus.
3. **Endzustand.** Eine Meldung `Socket` mit `ausstehend` bis zur Grenze für einen Betrachter, der hinter dem Stand der Sitzung liegt, löst sofort genau eine `Rasteraenderung` aus (Nachholen):
   `von` ist der Stand, den er zuletzt bekam, `stand` der jetzige, `zeilen` sind alle Zeilen seines Rückstands und die noch nicht versendeten Zeilen der Sitzung, `invers` ist der jetzige Wert.
   Es braucht keine neue Ausgabe. Das gilt, wenn der Stoß endet, bevor der fällige Versand der Sitzung lief, und nach einer synchronisierten Folge (der freigegebene Stand zählt).
4. **Beschränkter Speicher.** Der Kern hält je Betrachter und Sitzung nur eine Zeilenmenge, also höchstens die Zahl der Zeilen der Sitzung, gleich wie viele Bilder verfielen. Verfallene Bilder
   werden nicht aufbewahrt, das Nachholen ist eine einzige Nachricht. Ein Bild geht nur hinaus, wenn die letzte Meldung höchstens die Grenze zeigt; meldet der Träger nach jeder Nachricht, liegt
   `ausstehend` also nie über Grenze plus einer Nachricht (Messung `RS`). Ein `Abzug` (Abo, Größenänderung) trägt den ganzen Stand: Er geht immer hinaus, gilt nie als verfallenes Bild und löscht den Rückstand der Sitzung.
5. **Zählung.** Der Endstand nennt je Betrachter `verfallen` (Summe seit dem Verbinden), `ausstehend` (letzte Meldung) und `rueckstand`. „Abfragbar“ heißt: Teil von `zustand`. Ein Takt ohne zu sendende
   Änderung ist kein Bild.
6. **Ein langsamer Betrachter bremst niemanden.** Kein Versand, keine Frist und kein Zeitgeber einer Sitzung hängt von der Meldung eines Betrachters ab. Die anderen Betrachter bekommen ihre Bilder zu den
   Zeiten nach R5, als gäbe es den langsamen nicht. Das Nachholen ist kein Versand der Sitzung: Es ändert weder die Frist noch die schmutzigen Zeilen der Sitzung. Für den Nachholenden gilt der Abstand (R5) seit
   seiner letzten `Rasteraenderung`; ist er noch nicht um, legt der Kern einen Zeitgeber auf sein Ende (das ist kein Verfall und wird nicht gezählt). Fällt der nächste Versand der Sitzung in diesen Abstand, wartet
   nur der Nachholende.

### R15 Cursor

Quelle: Projektentscheid, 19.11.2026 („Der Kern führt `cursor` je Sitzung und reicht ihn durch“), Plan zur Ansicht, Abschnitt 4, Form nach `RA` R8; Auflage, 19.11.2026 (Deckel). Neu seit 19.11.
Der Cursor ist Teil des Rasters, wie `invers` (R13): ein Zustand der Sitzung, nicht des Betrachters.
1. **Form und Prüfung.** Die `Ausgabe` des Trägers darf `cursor` tragen: `[zeile, spalte, sichtbar]`, `zeile` und `spalte` ganze Zahlen, `zeile` kleiner als die Zeilenzahl und `spalte` kleiner als die Spaltenzahl der Größe
   der Sitzung (nicht der gemeldeten Größe eines Betrachters), `sichtbar` ein Wahrheitswert (DECTCEM). Alles andere ergibt `ereignis_ungueltig` und verwirft das ganze Ereignis, auch seine Zeilen und seinen Stand:
   zu viele oder zu wenige Elemente, ein Text, `null`, eine Zahl mit Bruch, ein Wahrheitswert als Zahl, `1` oder `"ja"` als `sichtbar`, negative Zahlen, Zahlen außerhalb der Größe. Die Prüfung steht vor der Prüfung des Stands:
   Ein Ereignis mit ungültigem Cursor und altem Stand ergibt `ereignis_ungueltig`, nicht `stand_alt`. Ein fehlendes Feld ist gültig und lässt den Cursor des Kerns stehen. `Modus` trägt keinen Cursor.
2. **Führung.** Der Kern hält je Sitzung höchstens einen Cursor. Anfangs kennt er keinen, außer der Fall nennt `anfang.sitzungen[].cursor` (Abschnitt 3). Eine gültige `Ausgabe` mit `cursor` setzt ihn.
3. **Durchreichen.** `Abzug` und `Rasteraenderung` tragen `cursor` mit dem jetzigen Wert der Sitzung, sobald der Kern einen kennt, **in jeder** Nachricht, nicht nur bei einer Änderung (`RA` R8 vergleicht selbst mit dem Vorbild;
   das Nachholen nach R14 und ein verlorenes Bild brauchen keinen Zustand des Betrachters). Kennt der Kern keinen, fehlt das Feld; deshalb ist jeder Fall ohne `cursor` Byte für Byte so wie vor dem Nachtrag. `zustand.sitzungen[]` nennt
   `cursor` unter derselben Bedingung.
4. **Eine Änderung nur am Cursor** (Stelle oder `sichtbar`) ergibt eine `Rasteraenderung` ohne Zeilen (`zeilen` leer) mit `cursor`; `stand` und `von` zählen wie bei jeder Änderung. Wie bisher löst jede gültige `Ausgabe` einen
   Versand aus, auch ohne Zeile; der Träger meldet eine `Ausgabe` nur bei einem Unterschied (TR2).
5. **Bündelung (R5): der jüngste Cursor gilt.** Mehrere Ausgaben in einem Abstand ergeben eine Nachricht mit dem Cursor der letzten. Ein Bündel nur aus Cursorschritten ist eine Nachricht ohne Zeilen. Begründung: Der Cursor ist
   Zustand, kein Ereignis; die Ansicht braucht nur die Stelle zur Zeit des Bildes.
6. **Deckel der Bildrate (Auflage, R5): reine Cursorschritte umgehen `bildrate_max` nicht.** Eine `Ausgabe` ohne Zeilen geht durch dieselbe Frist wie jede andere: Der Abstand ist `max(16, aufrunden(1000 / bildrate_max))`
   des Sitzungsbesitzers, und es gibt je Abstand und Sitzung höchstens eine `Rasteraenderung` (der Nachholende nach R14 hat seinen eigenen Abstand, Punkt 24). Viele Cursorschritte in kurzer Zeit ergeben also nicht mehr Nachrichten
   als Zeilenänderungen; der jüngste Cursor kommt in der letzten Nachricht an. Hergeleitet: Der Träger meldet bei jedem Stück Bytes mit Cursorbewegung eine `Ausgabe` (TR2); ohne diese Regel könnte ein Programm, das den Cursor
   ständig bewegt, den Deckel unterlaufen. Die Referenz hat keinen eigenen Pfad für Cursorschritte.
7. **Synchronisierung (DEC 2026): der Cursor wartet wie die Zeilen.** Zwischen `SyncBeginn` und `SyncEnde` (oder dem Zeitgeber nach 150 ms) geht kein Cursor hinaus; ein `Abzug` trägt den freigegebenen Cursor. Bei der Freigabe gilt der
   jüngste wartende Cursor. Eine Folge, in der nur der Cursor von dem der Sitzung abweicht, gibt auch dann eine `Rasteraenderung` frei, wenn zugleich ein `Modus` mit gleichem `invers` wartete (sonst wäre die Änderung verloren).
8. **Größenänderung (R3): Der Kern schickt nie einen Cursor außerhalb der Größe.** Ändert sich die Größe der Sitzung, setzt der Kern den Cursor auf `[min(zeile, zeilen − 1), min(spalte, spalten − 1), sichtbar]`, bevor er den `Abzug`
   an die Abonnenten schickt. Grund: `rasterauszug` lehnt einen Cursor außerhalb mit `cursor_ungueltig` ab (`RA` R8). Verworfen sind „den Cursor vergessen“ (der Abzug fiele nach `RA` R6 auf `(0, 0, unsichtbar)`) und
   „unsichtbar machen“. Die Klammer ist nur eine Brücke: Der Träger meldet nach der Größenänderung seinen eigenen Cursor (TR3), und das Raster von `alacritty_terminal` klemmt Spalte und Zeile ebenso. Ein wartender Cursor einer
   synchronisierten Folge wird bei der Freigabe gegen die dann geltende Größe geklemmt. Eine `Ausgabe` mit einem Cursor außerhalb der jetzigen Größe wird nach Punkt 1 abgelehnt.
   **(Nachtrag 20.11.2026, Projektentscheid, 20.11.2026.)** Schrumpfen die Spalten und steht der Cursor rechts vom neuen Rand, bricht der Träger ihn um (`TR3`; gemessen 40 mal 10 auf 20 mal 5: `[7,29]` wird `[4,9]`), der Kern klemmt vorläufig (`[4,19]`).
   Der Kern **übernimmt nach der Größenänderung den Wert des Trägers**: Die `Ausgabe` des Trägers trägt `cursor` immer (Punkt 1), der Kern führt ihn wie jeden Cursor (`Rasteraenderung`, `zustand`), und das Abbild zeigt ihn. So bleibt kein zweiter, abweichender Cursor stehen. Geprüft (20.11.): Das tut die Referenz heute (Fall `r15_groesse_traeger`, Mutant `cursor_nach_groesse_verworfen`); nichts war anzupassen.
9. **Abo und Neuverbindung (R4).** Der `Abzug` beim Abo trägt den Cursor der Sitzung; nach `Trennen`, `RaumWechsel` oder Abo-Ende bleibt der Cursor der Sitzung stehen (er gehört nicht dem Betrachter).
10. **Rückstaugrenze (R14).** Ein verfallenes Bild verliert auch seinen Cursor nicht endgültig: Das Nachholen (und jede spätere Nachricht) trägt den jetzigen Cursor. Ein Betrachter, der nur wegen eines Cursorschritts hinter dem
    Stand liegt, bekommt beim Nachholen eine Rasteränderung ohne Zeilen mit `cursor`; sein `rueckstand` nennt dann eine leere Zeilenliste.
11. **Neubau nach einem Fehler (Punkt 27).** Der Dienst gibt dem neuen Kern den `cursor` aus `zustand` als `anfang.sitzungen[].cursor` mit; jeder Betrachter bekommt danach einen vollen `Abzug` (R4) mit demselben Cursor.
    Gibt der Dienst keinen mit, kennt der Kern keinen, und der Abzug lässt das Feld weg (`RA` R6: Vorbild `(0, 0, unsichtbar)` bis zur nächsten `Ausgabe` mit Cursor).
12. **Schrittweise API.** `Sitzungskern` ergibt dasselbe wie `laufen` (Punkt 27); die Fälle laufen durch `laufen`.

### R13 Inverser Bildschirm (DECSCNM)

Quelle: `RA` (`alacritty_terminal` 0.26.0 kennt Modus 5 nicht; das Raster ändert sich bei `ESC[?5h` und `ESC[?5l` nicht), Auflage, 18.11.2026.
Der Kern führt den Modus selbst. Der Träger meldet jede Folge `ESC[?5h` und `ESC[?5l` aus dem Ausgabestrom als `Modus` mit `invers` true oder false.
- Der Modus gehört zum Raster: `Abzug` und `Rasteraenderung` tragen `invers` (immer, true oder false). Der Anfangswert ist `invers` der Sitzung
  im Fall (Vorgabe false).
- `Modus` mit `invers` braucht ein ganzes `stand` und gehört in die Zählung der Stände wie `Ausgabe`: ein Stand nicht über dem höchsten bekannten
  ergibt `stand_alt`, ein anderer Typ als bool oder ein fehlendes `stand` ergibt `ereignis_ungueltig`. Ein `Modus` ohne `bracketed_paste` und ohne
  `invers` ergibt `ereignis_ungueltig`. Eine Ablehnung ändert nichts, auch nicht `bracketed_paste` im selben Ereignis (V).
- Ändert der Wert `invers`, geht eine `Rasteraenderung` hinaus (gebündelt wie jede andere, R5), auch ohne geänderte Zeile. Ein Wert gleich dem
  jetzigen rückt nur den Stand vor und löst keinen Versand aus (V).
- Während einer synchronisierten Folge wartet der Wert wie die Zeilen; `SyncEnde` oder der Zeitgeber geben beides frei. Ein `Abzug` zeigt den
  freigegebenen Wert (R5).

### R6 Dialog

Solange ein Dialog in einer Sitzung steht, lehnt der Kern `Text`, `Taste` und `Einfuegen` dorthin mit `pane_busy` ab
(`E:46`, `I:69`). Die `Antwort` auf genau diesen Dialog geht durch und schreibt die Ziffer der Wahl (`31` für 1) an die PTY (V).
- Eine zweite Antwort auf denselben Dialog: `bereits_beantwortet`. Falsche ID oder kein Dialog: `dialog_unbekannt`.
  Wahl außerhalb von 1 bis Zahl der Optionen (höchstens 9): `wahl_ungueltig` (V).
- Der Dialog steht, bis die Erkennung `DialogEnde` meldet. Andere Sitzungen bleiben frei. Ein neues Abo bekommt nach dem Abzug den offenen `Dialog`.

### R7 Freigaben

Quelle: `A:216` Fall 4 und 6, `A:188`, `E:141`, `E:142`.
- Eine `Freigabe` nennt genau eine Anfrage. Null oder mehrere: `sammelfreigabe`.
- Die Person des Betrachters muss `art: mensch` haben, sonst `agent_kennung`, auch mit gültigem Nachweis.
- Menschen-Nachweis: ohne `nachweis_fehlt`; unbekannt oder einer anderen Person `nachweis_ungueltig`; schon benutzt `nachweis_verbraucht`.
  Ein Nachweis gilt einmal (V; der Beleg gilt je Freigabe, `E:162`).
- Danach: unbekannte Anfrage `anfrage_unbekannt`, Anfrage eines anderen Kundenraums `fremder_kundenraum`,
  abgelaufen `abgelaufen`, schon erteilt `nicht_offen`.
- Prüffolge: Anzahl, Person, Nachweis, Anfrage (V). Erfolg: Nachweis verbraucht, Anfrage `erteilt`, Eintrag in `an_stelle`,
  `Anfrage` an den Raum.
- Die Frist ist absolut. Bei `t` gleich `frist` ist die Anfrage abgelaufen; der Kern meldet `abgelaufen` zu dieser Zeit (V, `A:186`).

### R8 Routen

Gesperrte Routen bleiben sichtbar und sind nicht wählbar (`A:97`, `E:150`).
- `Routen` nennt alle Routen mit `status`, `grund`, `zone` und `waehlbar`. Wählbar ist nur `frei` (V: auch `evaluierung` ist nicht wählbar).
- `RouteWaehlen` auf eine nicht freie Route: `Hinweis` `route_gesperrt` mit Grund an den Wählenden und Ablehnung `route_gesperrt`.
- Eine Route ohne Eintrag: `Hinweis` `route_ohne_klasse` und Ablehnung `route_ohne_klasse` (`A:93`).
- Eine freie Route wird zur Route der Sitzung; der Raum bekommt eine neue `Sitzungsliste`.

### R9 Kundenraum

Nichts geht über eine Kundenraumgrenze (`A:104`, `E:143`).
- Ein Betrachter sieht nur Sitzungen, Anfragen, Agenten und Nachrichten seines Raums.
- `RaumWechsel`: Der Kern löst alle Abos des Betrachters und seine letzten Eingaben, schickt `Leeren` und danach das Bild des
  neuen Raums (`Sitzungsliste`, `Anfragen`, `Agenten`). Danach erreicht den Betrachter nichts aus Raum A, bis er zurückwechselt.
- Abo, Routenwahl und Freigabe über die Grenze: `fremder_kundenraum`.

### R10 Agentenzustand

„Unbekannt“ ist nie ein Endstand (`A:44`, `I:87`, `E:44`).
- Ein bekannter Zustand wird mit `gesehen_um` = `t` und `quelle` gespeichert und an den Raum gemeldet.
- `unbekannt` ändert nichts: Zustand und `gesehen_um` bleiben, keine Nachricht; der Kern fragt sofort erneut (`abfragen`).
- Ein nie gesehener Worker erscheint in keiner Liste. Jeder Worker steht für sich; ein langsamer Aufruf ändert nur diesen Worker.
- Das Alter rechnet die Ansicht aus `gesehen_um` (`E:84`).

### R11 Absender

Der Absender kommt aus den Feldern, nie aus dem Text (`A:38`, `I:75`, `E:45`).
- `herkunft` `sitzung`: Absender `{"art": "sitzung", "sitzung": …}`, die Sitzung muss bestehen, sonst `absender_unbekannt`.
- `herkunft` `mensch`: Der genannte Betrachter muss verbunden sein und eine Person mit `art: mensch` haben, sonst `absender_unbekannt`.
  Absender ist `{"art": "mensch", "person": …}`.
- Fehlt `herkunft` oder ist die Art unbekannt: `absender_fehlt`. Es gibt keine Vorgabe „Mensch“.

### R12 Einstellungen

Quelle: `A:68`, `A:69`, `A:71`, `P:42`, `E:136`, `E:144`.
- Ebenen `firma` und `gruppe` ändert nur ein Admin (`nur_admin`).
- Die Ebene `nutzer` setzt jeder für sich, nur wenn die Regel dem Nutzer gehört (`nutzer_darf`, sonst `regel_nicht_beim_nutzer`)
  und nur strenger als die Grundlage aus Gruppe, Firma oder Vorgabe (`nur_verschaerfen`; gleich ist nicht strenger).
- Strenger: bei `stufen` eine spätere Stufe, bei `zahl` eine kleinere Zahl. Ungültiger Wert: `wert_ungueltig`; unbekannter Schlüssel:
  `schluessel_unbekannt`.
- Wirksam: Gruppe vor Firma vor Vorgabe; die Nutzerregel gilt nur, solange sie strenger ist als diese Grundlage. `herkunft` nennt die Ebene.
- Jede angenommene Änderung schreibt `protokoll` mit altem und neuem Wert und schickt `Einstellungen` an die Betrachter der
  betroffenen Personen (Firma und Gruppe: alle verbundenen; Nutzer: nur die eigenen).

## 7. Festgelegt im Vertrag

Der Entwurf lässt diese Fragen offen; der Vertrag entscheidet klein:

1. Zeit: Zeitgeber laufen vor einem Ereignis mit gleichem oder späterem `t`; am Fallende nicht. Gleiche Zeit: Reihenfolge des Anlegens.
2. OSC 52 genau an der Grenze: erlaubt (Begründung in R2). „Betrachter der Sitzung“ heißt Abonnent.
3. OSC 52 ohne Empfänger: kein Inhalt und kein Hinweis, Ablehnung `osc52_kein_empfaenger`. Bei „aus“ auch keine Ablehnung.
4. Die letzte Eingabe einer Sitzung erlischt mit Abo-Ende, Trennen und Raumwechsel des Betrachters.
5. Eingaben brauchen ein Abo. Nach `Kindende` lehnt der Kern Eingaben ab.
6. `Text` mit Steuerzeichen wird abgelehnt; `Einfuegen` entfernt Klammermarken im Text und wandelt die Zeilenumbrüche je nach
   Bracketed Paste: an: LF und CRLF zu CR; aus: CR und CRLF zu LF (R1). Nachtrag 18.11.2026, Projektentscheid, Messung `T`.
7. Tastentabelle über Enter und Umschalt+Enter hinaus: xterm-Normalform, ohne Anwendungs-Cursormodus; Strg/Alt+Links/Rechts
   gehören dazu (Nachtrag 18.11.2026, Messung `T`). Kein Modus-Ereignis für DECCKM.
8. Dialogantwort: Ziffer der Wahl an die PTY; sie zählt als Eingabe für Größe und OSC 52.
9. Bündelung: Vorderflanke sofort, dann je 16 ms; Versand ohne Abonnenten zählt; `von` je Betrachter.
10. Synchronisierung: Abzug zeigt den freigegebenen Stand; zweites `SyncBeginn` verlängert nicht.
11. Größenänderung: neuer Abzug an alle Abonnenten; Grenzen: Spalten 2 bis 1000, Zeilen 1 bis 1000 (Nachtrag 20.11.2026).
12. Freigabe: Nachweis einmal verwendbar, Prüffolge Anzahl, Person, Nachweis, Anfrage; Frist bei `t` gleich `frist` abgelaufen.
13. Route `evaluierung` ist nicht wählbar.
14. Eine Gruppe gilt für alle Personen eines Falls; Gruppe geht vor Firma; Nutzerwert gleich der Grundlage ist keine Verschärfung.
15. Eingebaute Regeln `osc52_schreiben` und `osc52_grenze` gelten für den Besitzer der Sitzung (Feld `person`).
16. Raumwechsel prüft keine Raumrechte der Person; das gehört zum Gateway (nicht im Schnitt).
17. `Rueckblick` und `Person` aus `E:71`, `E:74` sind nicht im Vertrag: Rücklauf liegt im Träger, die Anmeldung im Gateway.
18. Obergrenze der Bildrate (Auflage, 18.11.2026, Messung `RA`): Eine Einstellung `bildrate_max` mit Vorgabe 63, im Abstand der Bündelung (R5).
    Die Vorgabe ist die kleinste ganze Zahl, bei der der Abstand 16 ms bleibt; sie liegt über der Rate der Bündelung (62,5 je s). Gemessen (Läufer, ruhige Messmaschine,
    19.11.): Mit Grenze 63 sind L2 (Leitung 12,0 kB/s, Verzögerung 99 ms p95 bei N2, Verzögerung im Kern 9,2 / 18,2 ms), L1, L3 und L5b unverändert; L1 bis L3 und L5b
    erreichten höchstens 54,4 Nachrichten je s, die Grenze greift bei ihnen nie. L4 bei N2 bleibt mit 2,5 Nachrichten je s und 239 kB/s unter der Grenze, 0 Lücken,
    die Warteschlange wächst nicht (höchstens 0,8 ms je s). Ein fester Deckel in Bildern je s kann L4 nicht von L1 und L2 trennen: bei N0 senden alle drei 54,3 bis 54,5
    Nachrichten je s; erst die Größe je Nachricht unterscheidet sie (L4 15,5 kB, L2 0,22 kB auf der Leitung). L4 fiele bei N0 nur mit einer Grenze von etwa 2 je s
    unter 250 kB/s (193 kB/s), und diese Grenze gäbe L2 einen Median von 252 ms. Die Zahlen stehen, Teil 2.
    Eine Firma oder Gruppe darf den Wert senken (R12); gesenkt gilt er für die Sitzungen des Besitzers.
19. Obergrenze: Der Abstand in ganzen ms ist aufgerundet, damit die Rate die Grenze nie überschreitet (`bildrate_abrunden` prüft das).
20. DECSCNM (Auflage, 18.11.2026): Der Kern führt den Modus selbst, der Träger meldet ihn als `Modus` mit `invers` und `stand` (R13).
    Eine Zeile ändert sich dabei nicht. Ein gleicher Wert rückt nur den Stand vor.
21. Die Mindestwerte einer `zahl`-Regel: `bildrate_max` ab 1 (0 ergibt `wert_ungueltig`); die anderen `zahl`-Regeln ab 0.
22. Rückstaugrenze (Projektentscheid, 19.11.2026, Messung `RS`): Die Vorgabe von `rueckstau_max` ist **16384** Byte, der kleinste gemessene Wert, der alle Abnahmepunkte hält. Gemessen (Läufer, ruhige Messmaschine, 19.11., 81 Läufe, Binary `ca0f5f9f…`): Bei 16 KiB verfällt in L1 bis L3 und L5b kein Bild, B2, B3 und V1 bleiben; L4 bei N2 hat 0 Lücken und eine nicht wachsende Warteschlange (Steigung der Verzögerung höchstens −0,29 ms je s; ohne Grenze 481 ms je s und 171 Lückenzeilen); mit zwei Betrachtern hält der lesende B3 und V1 (L4 10 / 20 ms und 10,27 / 19,73 ms), der stumme verliert 3249 Bilder (Summe von 3 Wiederholungen); die ausstehenden Byte liegen höchstens bei Grenze plus einer Nachricht (L4 bei N2 112 864 B gegen 113 603 B). Bei N2 steigt die Verzögerung bis Empfang mit der Grenze (p95 886 ms bei 16 KiB, 5016 ms bei 1024 KiB); Werte unter 16 KiB sind nicht gemessen. Zahlen: Teil 1.
23. Meldung des Sockets (R14): ein Messwert `ausstehend` je Betrachter, keine Summe; vor der ersten Meldung 0. `Socket` mit einem Wert, der keine ganze Zahl ab 0 ist, ergibt `ereignis_ungueltig`, mit einem unbekannten Betrachter `betrachter_unbekannt`. Ein Wert über `i64` gilt nach Abschnitt 2 nicht als ganze Zahl und ergibt ebenfalls `ereignis_ungueltig`; die Referenz nimmt solche Werte an, kein Fall berührt das (Fund, 19.11.).
24. Das Nachholen (R14) ist kein Versand der Sitzung (R5) und hat den eigenen Abstand des Betrachters; dadurch hängt kein anderer Betrachter an ihm.
25. `rueckstau_max` gilt je Betrachter nach dessen Person; `bildrate_max` gilt je Sitzung nach deren Besitzer.
26. Grenze der Betrachterzahl (Messung 19.11., Projektentscheid, 19.11.2026): Je Zyklus aus einer Ausgabe, einem `Socket` je Betrachter und einem Takt braucht `zk` (Release, reine Rechenzeit) bei 10 Betrachtern 27,5 µs, bei 100 0,35 ms und bei 1000 10,3 ms; das Wachstum ist schneller als linear (lineare Suche nach dem Betrachter), hergeleitet liegt die Grenze von 15,9 ms je Bild bei etwa 1300 Betrachtern je Sitzung. Kein Umbau, solange die Zielzahl unter rund 1000 liegt; Ausweg ist eine Karte von Name zu Index, die die Reihenfolge des Verbindens behält.
27. Schrittweise API (Projektentscheid, 19.11.2026): Im `Sitzungskern` zählt die Nachrichtengrenze von 33 554 432 Byte je Schritt, nicht über die Lebenszeit; `laufen` und `zk` zählen weiter über den ganzen Fall. Ein Schritt über der Grenze gibt einen Fehler, danach ist der Kern verbraucht. Der Dienst verwirft ihn dann und baut die Sitzung neu; jeder Betrachter bekommt danach einen vollen Abzug (R4), als hätte er neu abonniert.

28. Cursor (Nachtrag 19.11.2026, Projektentscheid): R15. Festgelegt: `cursor` in `Ausgabe` ist optional und wird streng geprüft (ereignis_ungueltig vor stand_alt); `Abzug`, `Rasteraenderung` und `zustand` tragen
    ihn in jeder Nachricht, sobald er bekannt ist; sonst fehlt das Feld (alle 123 alten Fälle bleiben Byte für Byte gleich); der jüngste Cursor gilt in einem Bündel; er wartet in DEC 2026; er wird bei einer Größenänderung geklemmt;
    reine Cursorschritte laufen durch dieselbe Frist wie Zeilen und den Deckel `bildrate_max` (Auflage); `anfang.sitzungen[].cursor` ist neu, damit der Neubau den Cursor behält.

29. Kleinste Größe (Nachtrag 20.11.2026, Projektentscheid, 20.11.2026): Die kleinste Größe ist 2 Spalten für die ganze Sitzung (Kern, PTY, Raster); der Kern lehnt eine Spalte mit `groesse_ungueltig` ab (R3). Fälle `r3_groesse_grenzen`, `r3_groessenfolge_klein` (nur sichtbar), `r3_anfang_kleinste_groesse`, `r3_anfang_groesse_ungueltig` (nur sichtbar: Die Erwartung `exit 2` ist in beiden Sätzen dieselbe Datei, die Hash-Trennung erlaubt sie nur einmal); Mutanten `groesse_grenze_fehlt`, `groesse_grenze_verschoben`, `groesse_zeilen_verschoben`, `anfang_groesse_ungeprueft`. Alle 142 alten Fälle (86 sichtbar, 56 verdeckt) sind Byte für Byte gleich geblieben (gemessen); keiner nannte eine Spalte.
30. Cursor nach schrumpfenden Spalten (Nachtrag 20.11.2026, Projektentscheid, 20.11.2026): R15 Punkt 8, Fall `r15_groesse_traeger`, Mutant `cursor_nach_groesse_verworfen`.

## 8. Gegenproben (Mutanten)

Jeder Mutant schaltet sich in der Referenz mit `REF_GEGENPROBE=<name>` zu. `gegenprobe.sh` prüft, dass diese Liste und
`zk-ref --gegenproben` gleich sind und dass jeder Mutant mindestens einen Fall rot macht.

| Mutant | Regel | Änderung |
|---|---|---|
| `umschalt_enter_cr` | R1 | Umschalt+Enter wird `0d` statt `1b 0d` |
| `paste_ohne_klammern` | R1 | keine Klammermarken bei Bracketed Paste |
| `paste_endmarke_durch` | R1 | `ESC[200~`/`ESC[201~` im eingefügten Text bleiben stehen |
| `paste_lf_bleibt` | R1 | mit Bracketed Paste bleibt LF im eingefügten Text LF und wird nicht CR |
| `paste_cr_ohne_klammern` | R1 | ohne Bracketed Paste bleibt CR im eingefügten Text CR und wird nicht LF |
| `mod_pfeil_fehlt` | R1 | Strg+ und Alt+`Links`/`Rechts` werden mit `taste_unbekannt` abgelehnt |
| `osc52_grenze_exklusiv` | R2 | genau die Grenze wird schon abgelehnt |
| `osc52_kuerzen` | R2 | über der Grenze wird gekürzt statt abgelehnt |
| `osc52_inhalt_an_alle` | R2 | Inhalt an alle Abonnenten statt an den mit der letzten Eingabe |
| `osc52_aus_hinweis` | R2 | bei „aus“ erscheint trotzdem der Hinweis |
| `osc52_lesen_antwort` | R2 | Leseanfrage wird mit einer leeren Antwort an die PTY beantwortet |
| `groesse_jede_meldung` | R3 | jede Meldung und jedes Abo setzt die Größe |
| `groesse_folge_ausfuehren` | R3 | `ESC[8;…t` aus dem Ausgabestrom wird ausgeführt |
| `abzug_nur_erstes_abo` | R4 | nach Neuverbindung kein Abzug, Änderungen ab altem Stand |
| `aenderung_ohne_abo` | R4 | Rasteränderungen an alle Betrachter des Raums |
| `buendel_ohne_frist` | R5 | jede Ausgabe geht sofort hinaus |
| `sync_ignorieren` | R5 | Ausgaben während DEC 2026 gehen hinaus |
| `sync_ohne_zeitgeber` | R5 | ohne `SyncEnde` gibt kein Zeitgeber frei |
| `dialog_sperrt_nicht` | R6 | Eingabe trotz offenem Dialog |
| `dialog_sperrt_antwort` | R6 | auch die Antwort wird mit `pane_busy` abgelehnt |
| `sammelfreigabe` | R7 | mehrere Anfragen mit einem Handgriff |
| `agent_freigabe` | R7 | Person mit `art: agent` darf freigeben |
| `nachweis_mehrfach` | R7 | ein Nachweis gilt mehrfach |
| `route_gesperrt_unsichtbar` | R8 | nicht freie Routen fehlen in `Routen` |
| `route_gesperrt_waehlbar` | R8 | gesperrte Route ist wählbar |
| `raum_ohne_leeren` | R9 | Raumwechsel ohne `Leeren`, Abos bleiben |
| `raum_abo_bleibt` | R9 | `Leeren` kommt, aber Abos und letzte Eingabe aus Raum A bleiben |
| `agent_unbekannt_endstand` | R10 | `unbekannt` wird als Zustand gespeichert und gemeldet |
| `agent_alter_neu` | R10 | `unbekannt` setzt `gesehen_um` neu |
| `absender_aus_text` | R11 | „Von X:“ am Textanfang wird Absender Mensch X |
| `absender_vorgabe_mensch` | R11 | fehlende Herkunft wird Mensch |
| `nutzer_aendert_regel` | R12 | ein Nutzer ändert Firma oder Gruppe |
| `nutzer_lockert` | R12 | ein Nutzer darf lockern |
| `invers_ignorieren` | R13 | `Modus` mit `invers` ändert nichts (so, wie `alacritty_terminal` DECSCNM ignoriert) |
| `invers_nicht_im_abzug` | R13 | der `Abzug` meldet `invers` immer als false |
| `bildrate_ignorieren` | R5 | die Obergrenze der Bildrate wirkt nicht, der Abstand bleibt 16 ms |
| `bildrate_abrunden` | R5 | der Abstand wird abgerundet statt aufgerundet |
| `rueckstau_ignorieren` | R14 | die Grenze wirkt nicht, kein Bild verfällt |
| `rueckstau_grenze_exklusiv` | R14 | schon bei genau der Grenze verfällt das Bild |
| `rueckstau_kein_nachholen` | R14 | eine Meldung unter der Grenze holt nichts nach; erst eine neue Ausgabe liefert wieder |
| `rueckstau_nur_letzte_zeilen` | R14 | das Nachholen trägt nur die noch nicht versendeten Zeilen, nicht den ganzen Rückstand |
| `rueckstau_warteschlange` | R14 | verfallene Bilder werden aufbewahrt und beim Nachholen einzeln geschickt |
| `rueckstau_bremst_alle` | R14 | liegt ein Betrachter über der Grenze, geht an niemanden ein Bild |
| `rueckstau_nicht_gezaehlt` | R14 | verfallene Bilder werden nicht gezählt |
| `rueckstau_grenze_global` | R14 | die Grenze des Sitzungsbesitzers gilt für alle Betrachter |
| `rueckstau_vorgabe_halb` | R14 | die Vorgabe der Grenze gilt nur zur Hälfte |
| `rueckstau_abzug_gesperrt` | R14 | auch ein Abzug verfällt über der Grenze |
| `rueckstau_nachholen_zaehlt_versand` | R14 | das Nachholen setzt die Frist der Sitzung und verschiebt so den Versand an andere |
| `rueckstau_nachholen_ohne_abstand` | R14 | der Nachholende bekommt das nächste Bild ohne den Abstand seit dem Nachholen |
| `cursor_ignorieren` | R15 | der Kern führt den Cursor aus `Ausgabe` nicht (kein Feld in Abzug und Änderung) |
| `cursor_nicht_im_abzug` | R15 | der `Abzug` trägt keinen Cursor |
| `cursor_nicht_in_aenderung` | R15 | die `Rasteraenderung` trägt keinen Cursor |
| `cursor_nur_bei_zeilen` | R15 | eine `Ausgabe` mit Cursor, aber ohne Zeilen löst keine Änderung aus |
| `cursor_aelterer_gilt` | R15 | im Bündel gilt der Cursor der ersten Ausgabe, ein späterer Cursor des Abstands wird verworfen |
| `cursor_sync_ignorieren` | R15 | der Cursor geht in DEC 2026 sofort in den Stand, die Zeilen warten |
| `cursor_sync_freigabe_ohne_versand` | R15 | eine Folge nur mit Cursorschritt (und gleichem `invers`) gibt bei der Freigabe nichts hinaus |
| `cursor_groesse_nicht_klemmen` | R15 | der Cursor bleibt bei einer Größenänderung stehen, auch außerhalb der neuen Größe |
| `cursor_groesse_vergessen` | R15 | eine Größenänderung löscht den Cursor, der Abzug lässt das Feld weg |
| `cursor_ungueltig_angenommen` | R15 | ein Cursor außerhalb der Größe wird angenommen |
| `cursor_ungueltig_ignoriert` | R15 | ein ungültiger Cursor wird ohne Ablehnung übergangen, der Rest der `Ausgabe` gilt |
| `cursor_sichtbar_nicht_geprueft` | R15 | `sichtbar` muss kein Wahrheitswert sein |
| `cursor_nachholen_fehlt` | R15 | das Nachholen nach R14 trägt keinen Cursor |
| `cursor_anfang_ignoriert` | R15 | `anfang.sitzungen[].cursor` wird nicht gelesen |
| `cursor_zustand_fehlt` | R15 | `zustand` nennt den Cursor nicht |
| `cursor_vorgabe_null` | R15 | ohne bekannten Cursor steht `[0, 0, false]` statt eines fehlenden Feldes |
| `cursor_umgeht_deckel` | R15, R5 | ein reiner Cursorschritt geht sofort hinaus, am Abstand und am Deckel `bildrate_max` vorbei |
| `groesse_grenze_fehlt` | R3 | eine Spalte wird in `Abo` und `Groesse` angenommen |
| `groesse_grenze_verschoben` | R3 | die kleinste Größe ist 3 Spalten (2 Spalten werden abgelehnt) |
| `groesse_zeilen_verschoben` | R3 | die kleinste Größe ist 2 Zeilen (eine Zeile wird abgelehnt) |
| `anfang_groesse_ungeprueft` | R3 | `anfang` nimmt eine Sitzung mit einer Spalte an |
| `cursor_nach_groesse_verworfen` | R15 | der Cursor der ersten `Ausgabe` nach einer Größenänderung wird verworfen, der geklemmte Wert des Kerns bleibt |

## 9. Dateien

- `referenz/zk_ref.py`, Starter `referenz/zk-ref` (ruft `python3 -I -B`; anderes Python über `ZK_PYTHON`).
- `faelle_bauen.py`: schreibt beide Sätze; jede Erwartung entsteht aus der Referenz.
- `aussentests/{faelle,erwartet}/` (sichtbar, Datensatz A) und `verdeckt/{faelle,erwartet}/` (Datensatz B: andere Namen,
  Räume, Texte, Größen und eine Zeitverschiebung von 7000 ms).
- `test_aussen.py`: Läufer mit `ZK_BIN` und `ZK_FAELLE`. `gegenprobe.sh`: Grundlauf und Mutanten.

## 10. Pflichten des Baus (Nachtrag 20.11.2026)

- Spalten 2 bis 1000, Zeilen 1 bis 1000 überall, wo der Kern eine Größe annimmt: `anfang` (ungültiger Fall), `Abo`, `Groesse` (`groesse_ungueltig`); die `Groesse` einer `GroessenFolge` bleibt nie wirksam. Der Kern gibt dem Träger und der PTY nie eine Größe mit einer Spalte.
- Der Kern übernimmt den Cursor jeder `Ausgabe` des Trägers, auch den ersten nach einer Größenänderung (R15 Punkt 8).
- Der Bau von `traeger` entfernt `schmal`, `schmal_rest`, `breite_entfernen` und `entbreiten`; `unicode-width` wird erst entfernt, wenn die cargo-Tore ohne sie grün sind, Stückliste und README ziehen nach (Auflage, 20.11.2026, `traeger` Abschnitt 11).
