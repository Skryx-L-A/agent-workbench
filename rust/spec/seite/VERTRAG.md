# Vertrag `seite`

Stand 20.11.2026 (Auftrag, 20.11.2026, Wort, 20.11.2026: die Messpunkte der Oberfläche werden Fälle für die
HTML-Seite des Durchstichs, parallel zum Bau). Ausführbarer Vertrag der Seite, die die Brücke unter `GET /` und `GET /seite.js` ausliefert
(Vertrag `bruecke` Abschnitt 5). Nachtrag 20.11.2026 (abgestimmt, dessen Drehbuch-Läufer die Seite Ende zu Ende bedient): feste
Selektoren `data-testid` als Pflicht (SE7), Token im Fragment als `#t=<token>`, Fehlerfeld mit Code und Text; der Rasterdekoder liegt künftig unter `rust/seite/`.
Gebaut sind eine **Attrappe** der Seite und ein **Läufer**. Die echte Seite gilt als vertragstreu, wenn sie beide
Fallsätze (sichtbar und verdeckt) im kopflosen Browser grün besteht.
„(V)“ heißt im Vertrag festgelegt (Abschnitt 5); „(H)“ heißt hergeleitet, ohne eigene Messung.
**Nachtrag 26.11.2026 (Entscheide des Betreibers, `rust/spec/VERTRAEGE.md` Fassung 2):** Das Startformular liegt im Fenster „Sitzungen“ hinter „+“. Die
Eingabezeile ruht bis nach dem Pilot. F6 wechselt zwischen Bereichen. Die Regeln gelten auch für das App-Fenster (`rust/spec/VERTRAEGE.md` OF14). Geänderte
Stellen tragen den Vermerk „(26.11.)“, die betroffenen Fälle stehen im Abschnitt „Nachtrag 26.11.2026“ am Ende.

Quellen: `BR` = `rust/spec/bruecke/VERTRAG.md`, `AB` = Vertrag `abbild` (außerhalb des Repos),
`VT` = `rust/spec/zustandskern/VERTRAG.md`, `RA` = `rust/spec/rasterauszug/VERTRAG.md`,
`DI` = `rust/spec/dienst/VERTRAG.md`, `EN` = der Entwurf der Ansicht (außerhalb des Repos),
`AN` = die Anforderungsliste (außerhalb des Repos) (A28, A46, A114, A118, A129).
Messpunkt der Stufe (Anforderung „Oberfläche“): „Prüfkatalog am Auge, mit Tastatur (deutsche Belegung mit Umlauten und toten Tasten), mit Vorleser und
im Handy-Browser. Der Text der Anzeige ist im Fenster und im Browser gleich.“

## 1. Schnitt

Die Seite ist eine HTML-Datei und eine Skriptdatei (`BR` Abschnitt 5). Sie liest Rahmen aus `GET /strom`, zeigt Sitzungsliste, Terminal,
Eingabezeile und Startformular und schickt Ereignisse der Ansicht an `POST /eingabe`. Dieser Vertrag legt fest, **was die Seite tut und
was ein Vorleser von ihr sieht**, nicht wie sie aussieht.

- **Ereignisse hinaus** sind die Ereignisse der Ansicht nach `VT` Abschnitt 4 ohne `betrachter` und `t` (wie `AB` AB6): `Abo`, `Groesse`, `Text`,
  `Taste`, `Einfuegen`. Die Seite übersetzt nie in Bytes (`VT` R1). Sie schickt jedes Ereignis als eigenes `POST /eingabe` mit dem Körper als
  JSON-Objekt, den Köpfen `Authorization: Bearer <token>`, `Content-Type: application/json` und `X-Verbindung: <kennung des Stroms>`.
- **Prüfpunkte im DOM** sind die festen Selektoren von SE7 (`data-testid`, Nachtrag). Das Terminal ist `terminal-raster`, seine Zeilen sind
  `terminal-zeile-<n>` (n ab 0, genau so viele wie das Raster Zeilen hat). Alles andere sucht der Läufer über Rollen und Namen des Barrierebaums.
- **Nicht Teil:** Aussehen, Farben, Schrift, Cursor-Darstellung, Markieren und Kopieren mit der Maus, Deflate (Abschnitt 8, Frage 1),
  mehrere sichtbare Terminals, Dialoge (`VT` R6), Routenanzeige (A46) und Zonenanzeige (A129). Diese kommen mit eigenen Fällen, sobald die Nachrichten
  im Strom des Durchstichs ankommen.
  (26.11.) Die Ansicht „Worker“ braucht mehrere sichtbare Terminals (`rust/spec/VERTRAEGE.md` B27, B30, Lücke L2). Ihre Fälle kommen, sobald die Verträge
  `zustandskern` und `dienst` mehrere Abos je Betrachter tragen. Bis dahin gilt dieser Vertrag für ein Terminal.

## 2. Aufruf des Läufers

```
node laeufer/se_lauf.mjs --seite <ordner mit index.html und seite.js> --faelle <ordner> [--fall <name> | --muster <präfix>] [--abbruch] [--laut] [--bericht <datei.json>]
```
- Umgebung: `SE_PLAYWRIGHT` (Ordner eines Pakets `playwright-core`; sonst sucht der Läufer unter `~/.npm/_npx/*/node_modules/`), `SE_CHROMIUM`
  (Programm `chrome-headless-shell`; sonst das neueste unter `~/Library/Caches/ms-playwright` am Mac, `~/.cache/ms-playwright` unter Linux).
- Der Läufer startet **einen** kopflosen Chromium (`headless: true`, kein Fenster) mit `--host-resolver-rules=MAP * ~NOTFOUND , EXCLUDE 127.0.0.1`
  (kein Name außer 127.0.0.1 löst auf) und je Fall einen neuen Browser-Kontext mit Fenstergröße, `isMobile`, `hasTouch` und `locale de-DE`.
- Je Fall startet er eine **Attrappe der Brücke** auf `127.0.0.1` an einem freien Port. Sie liefert die Seite mit dem `Content-Security-Policy` der
  Referenz von `BR` aus, öffnet den Strom (`X-Verbindung` 32 Hex, `X-Deflate: 0`, Abschnitt 8 Frage 1), schickt die Rahmen des Falls und schreibt jede
  Anfrage auf. Das Token ist je Fall neu (64 Hex aus dem Zufall), steht nur im Fragment der Adresse und nie im Fall.
- Die Seitenuhr ist eine **feste, falsche Uhr** (`page.clock.install`; Zeitgeber, `performance.now`, `requestAnimationFrame`). Sie läuft nur in den
  Schritten `Uhr` und beim Warten (je Durchgang 16 ms). Damit hängt kein Ergebnis von der Last der Maschine ab.
- Tasten gehen über CDP an die Seite (`Input.dispatchKeyEvent`, `Input.imeSetComposition`, `Input.insertText`), nie über das System. Einfügen ist
  ein künstliches `paste`-Ereignis mit eigenem `DataTransfer`; die echte Zwischenablage wird nie berührt.
- Exit 0: alle Fälle grün; 1: mindestens ein Fall rot (je Fall eine Zeile `ROT <fall>: <grund>`); 2: Aufbau gescheitert (kein Paket, kein Browser).

## 3. Fall

```json
{"name": "…", "regeln": ["SE1"], "datensatz": "A", "beschreibung": "…",
 "fenster": {"breite": 1024, "hoehe": 768, "mobil": false, "beruehrung": false},
 "starten": [{"status": 200, "antwort": {"sitzung": "…"}}],
 "schritte": [ … ]}
```
`fenster` (Vorgabe 1024 mal 768, ohne Handy) und `starten` (Antworten der Attrappe auf `POST /starten` der Reihe nach; danach 422 `start_form`) sind
freiwillig. Werte der Form `{"$zahl": [min, max]}` passen auf jede ganze Zahl in diesem Bereich (für Größen, die von der Schrift abhängen).
`ruht` (Nachtrag 26.11.2026) ist freiwillig. Am Fall heißt es `{"bedienwege": […], "grund": …}`: Der Läufer überspringt den Fall und zählt ihn als ruhend.
An einem Schritt nennt es den Bedienweg (etwa `"B37"`): Der Läufer überspringt nur diesen Schritt. Beides läuft wieder, wenn der Bedienweg kommt.

| `art` | Felder | Wirkung oder Prüfung |
|---|---|---|
| `Laden` | `ohne_token` (Vorgabe false) | öffnet `http://127.0.0.1:<port>/#t=<token>` (ohne Token ohne Fragment) |
| `Nachricht` | `nachricht` | Rahmen 0x06 mit dem JSON im Strom (`DI` Abschnitt 5) |
| `Ablehnung` | `grund`, `text` (freiwillig) | Rahmen 0x07 `{"grund": …}` (mit `text`, wie `DI` DI12 Punkt 4) im Strom |
| `Raster` | `hex` | Rahmen 0x05 mit diesen Bytes (Rasterrahmen nach `RA` RA9, ohne Deflate; die Sitzungsnummer steht im Rahmen) |
| `StromEnde` | – | die Attrappe beendet den Strom |
| `Taste` | `key`, `code`, `umschalter` (`umschalt`, `strg`, `alt`, `meta`), `vk`, `text` | `keyDown` (mit Text bei druckbarem `key`, sonst `rawKeyDown`) und `keyUp` über CDP |
| `Komposition` | `text` | `Input.imeSetComposition` (laufende Komposition, zum Beispiel nach einer toten Taste) |
| `Einsetzen` | `text` | `Input.insertText` (schließt eine laufende Komposition ab; sonst Text ohne Taste, wie eine Bildschirmtastatur) |
| `Einfuegen` | `text` | künstliches `paste` am fokussierten Element |
| `Fenster` | `breite`, `hoehe` | neue Fenstergröße |
| `Klick` | `ziel` `{rolle, name}` oder `{testid}` | Klick (mit `beruehrung` Tippen) auf genau ein Element dieser Rolle und dieses Namens oder dieses `data-testid` |
| `Waehle` | `testid`, `wert` | wählt im Auswahlfeld mit diesem `data-testid` die Option mit dieser Beschriftung |
| `Uhr` | `ms` | die Seitenuhr läuft `ms` weiter |
| `Warte` | `bis` `{"zeile": [i, text]}` | wartet (höchstens 4 s), bis Zeile i des Terminals diesen Text hat |
| `Ereignisse` | `erwartet`, `ausser` (Nachtrag 26.11.2026) | die Ereignisse an `/eingabe` seit der letzten Prüfung, genau diese in dieser Reihenfolge; Ereignisse mit einer `art` aus `ausser` (etwa `Groesse`) zählen nicht |
| `Starts` | `erwartet` | die Körper an `/starten` seit der letzten Prüfung |
| `Fokus` | `erwartet` `{rolle, name, name_beginnt, name_enthaelt, im_terminal, rahmen}` | Rolle und Name des fokussierten Elements im Barrierebaum; ob es im Terminal liegt; sichtbarer Rahmen (Vorgabe: Pflicht) |
| `Zeilen` | `erwartet` | Text jeder Zeile im DOM (`textContent`) **und** im Barrierebaum (Text der `StaticText`-Knoten unter der Zeile), genau diese Liste |
| `Ansagen` | `erwartet` oder `hoechstens`, `mindestens`, `letzte_enthaelt` | die Ansagen der Live-Regionen seit der letzten Prüfung |
| `Baum` | `erwartet` (Abschnitt 4, SE3), darin seit dem Nachtrag 26.11.2026 `dialog` `{name, modal, listbox, optionen, option_enthaelt}`, `knoepfe`, `fehlt` `[[rolle, name]]`, `formular.im_dialog` | Rollen, Namen, Live-Regionen; genau ein Dialog dieses Namens mit Liste und Optionen; jeder Knopf aus `knoepfe` genau einmal; kein Element aus `fehlt`; das Formular liegt im Dialog |
| `Layout` | `kein_waagerecht`, `ziele_mindestens` | Breite der Seite und Größe der Bedienziele |
| `Attribut` | `ziel`, `attribut`, `wert` | ein Attribut eines Elements |
| `Testid` | `testid`, `anzahl` (Vorgabe 1), `text`, `sichtbar`, `attribute` `{name: wert oder null}`, `optionen_enthaelt`, `wert` (Nachtrag 26.11.2026) | genau `anzahl` Elemente mit diesem `data-testid`; beim ersten Text (ohne Leerraum am Rand), Sichtbarkeit, Attribute (`null` heißt: fehlt), Optionen, Wert des Feldes; wartet höchstens 3 s |
| `Bedienwege` | `fehlen`, `kein_text` (Nachtrag 26.11.2026, OF7, OF8) | kein Element mit `data-bw` aus `fehlen`; der Text `kein_text` steht weder im Text der Seite noch in `title`, `aria-label` oder `placeholder` |

**Nach jedem Fall** prüft der Läufer immer (Netzregel, SE6): nur die Anfragen `GET /`, `GET /seite.js`, `GET /strom`, `POST /eingabe`, `POST /starten`,
ohne Abfrage im Ziel, keine Anfrage an eine andere Adresse, das Token nur im Kopf; genau ein Strom je `Laden` mit Token; jeder POST mit `Origin` und
`Content-Type: application/json`; jedes `/eingabe` mit der Kennung des Stroms und einem JSON-Objekt; keine Verletzung der CSP und kein Fehler im Skript der Seite.

Eine **Ansage** ist, was der Läufer in einer Live-Region (`aria-live` ungleich `off`, `role` `log`, `status` oder `alert`) ankommen sieht: jeder
hinzugefügte Knoten mit Text und jede Textänderung (Text ohne Leerraum am Rand). Gemessen wird damit, was die Seite einem Vorleser zum Ansagen gibt,
nicht, was ein echter Vorleser spricht (Abschnitt 7).

## 4. Regeln

### SE1 Tastatur (deutsche Belegung)

Gilt, solange der Fokus im Terminal steht und eine Sitzung abonniert ist. `s` ist die abonnierte Sitzung.
1. **Druckbare Zeichen** (ein `keydown`, dessen `key` genau ein Zeichen ist) gehen als `{"art": "Text", "sitzung": s, "text": key}`, ein Ereignis je Taste:
   Buchstaben, Ziffern, Leerzeichen, Umlaute und ß direkt (`ä` auf `Quote`, `ö` `Semicolon`, `ü` `BracketLeft`, `ß` `Minus`, groß mit Umschalt).
2. **AltGr- und Wahltasten-Zeichen** sind Text (V): Ein druckbarer `key` mit `alt` (Mac, Wahltaste: `@` L, `€` E, `{` 8, `[` 5, `]` 6, `}` 9,
   `\` Umschalt+7, `~` N, `|` 7) oder mit `strg` und `alt` zusammen (Windows und Linux, AltGr: `@` Q, `€` E, `{` 7, `[` 8, `]` 9, `}` 0, `\` ß-Taste,
   `~` +-Taste, `|` <-Taste) geht als `Text` mit diesem Zeichen, nie als `Taste`.
3. **Tote Tasten und Komposition:** Ein `keydown` mit `key` `Dead` oder `Process`, mit `isComposing` oder mit `keyCode` 229 schickt nichts. Eine laufende
   Komposition (`compositionstart`, `compositionupdate`, `input` mit `isComposing`) schickt **nichts**. Erst das Ergebnis (`compositionend` oder das
   abschließende `insertText`) geht als **ein** `Text`: `^` plus `e` gibt `ê`, `´` plus `e` gibt `é`, `` ` `` plus `e` gibt `è`; tote Taste plus Leerzeichen gibt
   das Zeichen selbst (`^`, `´`, `` ` ``); tote Taste plus Konsonant gibt beide Zeichen in einem Text (`^x`). Ein Ergebnis geht genau einmal hinaus.
4. **Eingesetzter Text ohne Taste** (`input` mit `inputType` `insertText`, etwa von einer Bildschirmtastatur oder einer Diktierfunktion) geht als ein `Text`.
5. **Benannte Tasten** gehen als `{"art": "Taste", "sitzung": s, "taste": t, "umschalter": [...]}` mit den Namen von `VT` R1: `Enter`, `Tab`, `Escape`,
   `Ruecktaste`, `Entf`, `Hoch`, `Runter`, `Links`, `Rechts`, `Pos1`, `Ende`, `BildAuf`, `BildAb`. **Umschalt+Enter** ist `Taste Enter` mit `["umschalt"]`
   (der Kern macht daraus `ESC CR`, A114); Umschalt+Tab ist `Taste Tab` mit `["umschalt"]`.
6. **Strg und Alt mit Buchstaben:** Strg plus Buchstabe ist `Taste <buchstabe klein>` mit `["strg"]` (Strg+C, Strg+D, Strg+L); Alt plus ein Buchstabe a bis z,
   den die Belegung unverändert liefert (Linux, Windows), ist `Taste <buchstabe>` mit `["alt"]`; Strg plus Alt plus Buchstabe ist `["strg", "alt"]`.
   Benannte Tasten mit Umschaltern gehen ebenso (Strg+Links, Alt+Rechts, Alt+Rücktaste). Was der Kern davon nicht kennt, lehnt er ab (`taste_unbekannt`);
   die Seite filtert nicht.
7. **Reihenfolge der Umschalter** (V): immer `umschalt`, `strg`, `alt`, jeder höchstens einmal.
8. **Dem Browser bleiben** (V): jede Kombination mit `meta` (Befehlstaste), Strg+V und jede Kombination aus Strg und Umschalt mit Buchstaben (Kopieren und
   Einfügen im Terminal unter Linux). Die Seite schickt dafür nichts und verhindert nichts.
9. Jede Taste, für die die Seite ein Ereignis schickt, verhindert die Standardwirkung (`preventDefault`): kein Zeichen landet zusätzlich im
   Eingabeelement, Tab bewegt den Fokus nicht, Bild auf und ab rollen die Seite nicht.
10. **Einfügen** (`paste` im Terminal) geht als `{"art": "Einfuegen", "sitzung": s, "text": <text/plain>}` mit dem Text **unverändert**; Zeilenumbrüche,
    Klammermarken und Bracketed Paste macht der Kern (`VT` R1). Leeres Einfügen schickt nichts. Einfügen geht nie als `Text`.
11. Die Seite schickt die Ereignisse **in der Reihenfolge der Eingabe, eins nach dem anderen** (V): das nächste `POST /eingabe` erst nach der Antwort auf das vorige.
12. Steht der Fokus nicht im Terminal, schicken Tasten keine Ereignisse. Felder sammeln ihren Text selbst. (26.11.) Die Eingabezeile (SE5 Punkt 3) ruht
    bis nach dem Pilot.

### SE2 Fokus

1. **Nach dem Laden** (V): Nennt die erste `Sitzungsliste` eine laufende Sitzung, abonniert die Seite die erste laufende und setzt den Fokus ins
   Terminal. (26.11.) Nennt sie keine, steht der Fokus auf dem Knopf „+“ (`neue-sitzung`, Name `Neue Sitzung`). Danach bewegt die Seite den Fokus
   nur auf eine Handlung hin.
2. **Tab-Stopps** außerhalb des Terminals, in dieser Reihenfolge (V, 26.11.):
   - die Knöpfe der Titelleiste von links nach rechts;
   - der Knopf „+“, dann je Sitzung ein Knopf der Sitzungsliste (Listenfolge), dann das Zahnrad im Fuß der Leiste;
   - das Terminal (in der Ansicht „Worker“ jedes sichtbare Pane in Kachelfolge);
   - bei offenem Inspektor seine Reiter und danach sein Inhalt.
   Umschalt+Tab geht rückwärts. In das Terminal kommt man mit Tab und Umschalt+Tab wie in jeden anderen Halt.
   **Fenster „Sitzungen“** (26.11., SE2 gilt ab dem Öffnen): Ist es offen, bleibt der Fokus in ihm. Tab und Umschalt+Tab laufen nur durch seine Halte,
   in dieser Reihenfolge: die Liste der Sitzungen, `Name` (ab L10), `Arbeitsordner`, `Harness`, `Modell`, `Effort` (ab L10), `Route`, `Kunde`, der Knopf `Starten`,
   die Knöpfe `Beenden` und `Fortsetzen` der gewählten Sitzung und der Knopf `Schließen`. Beim Öffnen über „+“ steht der Fokus im ersten Feld des Formulars (`Name`, vor L10 `Arbeitsordner`).
   Escape und `Schließen` schließen das Fenster, und der Fokus geht zurück auf „+“.
3. **Im Terminal gehören Tab und Umschalt+Tab dem Programm** (SE1 Punkt 5); der Fokus bleibt.
4. **Bereiche wechseln** (V, 26.11.): Die Bereiche sind Titelleiste, Leiste, Terminal und Inspektor (wenn offen), in dieser Folge, im Kreis.
   **F6** setzt den Fokus auf den ersten Halt des nächsten Bereichs, **Umschalt+F6** auf den des vorigen.
   - Vom Terminal aus führt F6 in den Inspektor oder, wenn er zu ist, in die Titelleiste. Umschalt+F6 führt auf den Knopf der gewählten Sitzung.
   - F6 geht nie an das Programm. Das Terminal nennt das in seiner Beschreibung (`aria-describedby`).
   - Maus und Tippen gehen wie immer.
5. **Wahl einer Sitzung** (Knopf der Liste, auch mit Enter): Die Seite schickt `{"art": "Abo", "sitzung": alt, "an": false}` für die bisherige,
   dann `Abo` an für die neue (mit Größe, SE5), zeigt deren Raster (Sitzungsnummer nach `DI` DI12 Punkt 5: das Feld `nr` ihres Eintrags der letzten `Sitzungsliste`; Nachtrag P25 23.11.2026: nie die Ziffern der Kennung. Bei `nr: null` gehört kein Rahmen zu ihr; bekommt eine Nummer eine andere Sitzung, vergisst die Seite das Bild darunter)
   und setzt den Fokus ins Terminal. Eingaben gehen danach an die neue Sitzung.
6. **Sichtbarer Fokusrahmen** (V): Das fokussierte Element hat einen Umriss (`outline-style` nicht `none`, Breite mindestens 2 px) oder einen Schatten;
   liegt der Fokus im Terminal, trägt das Element `terminal-raster` den Rahmen. Geprüft an jedem Halt der Tab-Folge.
7. **Welche Sitzung die Seite zeigt (Nachtrag 22.11.2026, Wunsch; drei Regeln, die Läufer und Rekorder kennen müssen):**
   - **Beim Laden** wählt die Seite selbst die erste Sitzung mit `zustand` `laeuft` aus der ersten `Sitzungsliste` und schickt dafür `Abo` mit ihrer gemessenen Größe (Punkt 1).
     Nennt die erste Liste keine laufende Sitzung, wählt sie keine.
   - **In eine danach neu gestartete Sitzung wechselt sie nicht**, auch nicht nach `Gestartet` (Abschnitt „Projektentscheide nach dem Bau der Seite“, Punkt 4). Wer die neue Sitzung sehen will, wählt sie
     mit dem Knopf der Liste (Punkt 5).
   - **Ein Klick auf die schon gewählte Sitzung sendet nichts**; er setzt nur den Fokus ins Terminal.
   - Abgrenzung (Kern, Vertrag `zustandskern`): Ein `Abo` ändert nur den Stand des Betrachters, nicht die Größe der PTY. Die PTY folgt der gemeldeten Größe eines Betrachters,
     sobald dieser Betrachter eine Eingabe macht, oder sofort bei `Groesse`, wenn er die letzte Eingabe gab.
   - Gelesen im Code von `development`: `rust/crates/ansicht/src/kern.rs:452-462` (erste Liste) und `:468-470` (gewählte Sitzung),
     `rust/crates/zustandskern/src/ereignis.rs:182-205` (`Abo`), `:233-235` (`Groesse`) und `:312-318` (Eingabe).

### SE3 Vorleser

1. **Rollen und Namen** (gemessen im Barrierebaum von Chromium, `Accessibility.getFullAXTree`):
   - Sitzungsliste: Rolle `list` mit Namen `Sitzungen`, je Sitzung ein `listitem` mit einem `button`, dessen Name die Kennung und den Zustand nennt
     (`läuft` oder `beendet, Code <n>`).
   - (26.11.) Fenster „Sitzungen“: Rolle `dialog` mit Namen `Sitzungen`, modal. Darin die Liste aller bekannten Sitzungen (Rolle `listbox`, Name
     `Bekannte Sitzungen`) und das Startformular: Rolle `form` mit Namen `Sitzung starten`. `Name` (ab L10) und `Arbeitsordner` sind `textbox`, `Harness`,
     `Modell`, `Effort` (ab L10), `Route` und `Kunde` sind Auswahlfelder (Rolle `combobox`); der Knopf heißt `Starten`. Der Knopf „+“ heißt `Neue Sitzung`.
   - Terminal: genau ein fokussierbares Element der Rolle `textbox`, dessen Name mit `Terminal` beginnt und die Kennung der Sitzung enthält (Empfehlung:
     ein `textarea` mit `aria-roledescription="Terminal"`, Abschnitt 8 Frage 3). Die Zeilen liegen als Text im Barrierebaum (nicht `aria-hidden`).
   - Eingabezeile: `textbox` `Eingabe` und `button` `Senden`. (26.11.) Ruht bis nach dem Pilot (SE5 Punkt 3).
   - Kein Bedienelement (Rollen `button`, `textbox`, `link`, `checkbox`, `combobox`, `listbox`, `option`, `radio`, `slider`, `spinbutton`, `switch`, `tab`,
     `menuitem`, `searchbox`) ohne Namen.
2. **Live-Regionen:** eine oder zwei (V: eine höfliche für Ausgabe und Zustand, wahlweise eine dringliche für Ablehnungen). Das Terminal und seine Zeilen
   liegen **in keiner** Live-Region.
3. **Was angesagt wird** (V, je genau ein Text):
   - **Neue Zeilen der Ausgabe:** Nach einem Rasterrahmen ohne Marke Groesse (`RA` RA9) sammelt die Seite die Zeilen, die der Rahmen setzt, deren Text
     nicht leer ist und die nicht die Zeile des Cursors (nach dem Rahmen) sind. Ein **Vollbild sagt nichts an.** Ist seit der letzten Ausgabe-Ansage
     mindestens 1000 ms (Seitenuhr) vergangen, sagt die Seite die Sammlung sofort an; sonst am Ende der Sperrzeit. Der Text sind die Zeilen der Sammlung
     nach Index, höchstens die letzten 5, getrennt durch `\n`. Also: eine neue Zeile ja, nicht jedes Bild; das Echo in der Zeile des Cursors nicht.
   - **Ablehnung** (Rahmen 0x07 oder Antwort 422 auf `/starten`): sofort `Abgelehnt: <text>`. Trägt die Ablehnung ein Feld `text` (fester Text der
     Prüfstelle, `start.*`, `DI` DI12 Punkt 4), ist das der Text. Sonst: `taste_unbekannt` „Taste nicht belegt“, `sitzung_beendet` „Sitzung beendet“,
     `kein_abo` „kein Abo“, `pane_busy` „Dialog offen“, `steuerzeichen_im_text` „Steuerzeichen im Text“; jeder andere Grund mit seinem Code.
     Derselbe Code und Text stehen im Fehlerfeld (SE7).
   - **Sitzung beendet:** beim Übergang einer Sitzung von `laeuft` auf `beendet` in der `Sitzungsliste`, einmal je Übergang: `Sitzung <id> beendet, Code <code>`.
     Die erste Liste nach dem Laden sagt nichts an.
   - **Verbindung getrennt:** wenn der Strom endet.
   - **Start:** Antwort 200 auf `/starten`: `Sitzung <id> gestartet`.
   - **Zwischenablage:** Nachricht `Hinweis` mit `hinweis` `osc52_geschrieben`: genau einmal `Das Programm hat in die Zwischenablage geschrieben`. Andere
     Hinweise (Titel) sagen nichts an.
   - **Zu groß (Nachtrag 23.11.2026):** Der Dienst lässt eine zu große Nachricht nicht hinaus und schickt statt ihrer einen `Hinweis`
     mit festem Grund (`DI` DI3, Abschnitt 8 Punkt 19). Die Seite sagt genau einmal an: `zwischenablage_zu_gross` „Die Zwischenablage des Programms
     war zu groß und wurde nicht übertragen“, `nachricht_zu_gross` „Eine Nachricht war zu groß und wurde nicht übertragen“. Wie bei `osc52_geschrieben`
     gilt das nur für einen `Hinweis` ohne `sitzung` oder mit der Kennung der gezeigten Sitzung; die Kurzmeldung (`rust/spec/VERTRAEGE.md` B67) zeigt denselben Text.
     Ein unbekannter Grund sagt nichts an.
   - Sonst nichts (keine Ansage für Abo, Größe, Vollbild, Fokuswechsel).
4. Ohne Flut: Unter Dauerausgabe gibt es höchstens eine Ausgabe-Ansage je 1000 ms; der Fall `se3_dauerausgabe` erlaubt höchstens `dauer/10 + 2` Ansagen
   bei `dauer` Bildern im Abstand von 100 ms, mindestens 2, die letzte nennt die letzte Zeile.

### SE4 Gleicher Text

1. Für jede Zeile i des Rasters der gezeigten Sitzung ist der Text des Elements `terminal-zeile-<i>` (DOM `textContent`) **gleich** dem Text der
   Zeile des Abbilds: die Texte ihrer Läufe aneinandergehängt (Zeilenform ohne Stile, `RA` Abschnitt 4). Dasselbe gilt für den Text im Barrierebaum.
2. Die Seite zeigt **genau so viele** Zeilen wie das Raster, leere eingeschlossen (leerer Text).
3. Keine Umwandlung (V): keine Normalisierung (NFD bleibt NFD, das kombinierende Zeichen bleibt hinter seinem Grundzeichen), kein Ersatz von U+00A0,
   Leerzeichen innen und vorn bleiben (die Zeilenform hat am Ende im Vorgabestil keine). Beispiel `umlaute` aus `AB` AB7 ist ein Fall.
4. Rahmen mit Verschiebung (M2, `RA` RA7) ergeben dieselben Zeilen wie das Abbild nach `RA` RA10.

### SE5 Handy-Browser

1. **Keine waagerechte Seitenleiste:** Die Seite ist nie breiter als das Fenster, auch bei 360 und 375 px und mit einer Zeile, die breiter ist als das
   Fenster (das Terminal rollt dann in sich).
2. **Klemme** (V, wie `AB` AB6): Die Seite misst ihre Größe aus der Fläche des Terminals und meldet nie weniger als **2 Spalten** und nie weniger als
   1 Zeile (höchstens 1000 und 1000), im `Abo` und in jedem `Groesse`. Sie meldet eine Größe nur, wenn sie sich gegenüber der zuletzt für diese Sitzung
   gemeldeten ändert. Die Felder von `Groesse` (`sitzung`, `spalten`, `zeilen`, alle Pflicht) legt der Vertrag `dienst` fest, Abschnitt DI3, Punkt „Felder von
   `Groesse`“ (Nachtrag 22.11.2026).
3. **Bildschirmtastatur:** Die Eingabezeile (`textbox` `Eingabe`, Knopf `Senden`) sammelt Text samt Komposition und schickt beim Absenden (Enter im Feld
   oder `Senden`) `Text` mit dem Inhalt (wenn nicht leer) und danach `Taste Enter`; dann ist sie leer. Vorher schickt sie nichts. Sie ist auf jeder Breite da.
   (26.11.) **Ruht bis nach dem Pilot.** Die Eingabezeile kommt mit der Chat-Ansicht (`rust/spec/VERTRAEGE.md` B37, B79, Entscheid F3). Dann gilt dieser Punkt
   wieder unverändert.
4. Eingabezeile und Terminal tragen `autocapitalize="off"` und `spellcheck="false"`; die Eingabezeile `enterkeyhint="send"`. (26.11.) Für das Terminal
   gilt das weiter, für die Eingabezeile ab ihrer Rückkehr.
5. **Bedienziele** außerhalb des Terminals sind bei schmaler Breite mindestens 44 mal 44 px (H: Richtwert der Plattformen für Tippziele).

### SE6 Netz und Start

1. Die Seite liest das Token aus dem Fragment `#t=<token>` (64 Kleinbuchstaben-Hex) und ruft nur `GET /strom`, `POST /eingabe` und `POST /starten`
   desselben Ursprungs, ohne Abfrage im Ziel, mit dem Token nur im Kopf `Authorization: Bearer <token>` (`BR` BR2, BR4). Sie ruft kein Symbol (`/favicon.ico`) ab (V: `<link rel="icon" href="data:,">`), keine fremde Adresse
   und keine Schrift von außen. Sie verletzt die CSP der Brücke nicht.
2. Ohne Token im Fragment ruft sie nichts außer sich selbst.
3. **Startformular:** Absenden schickt `POST /starten` mit `{"fassung": 1, "harness", "modell", "route", "kunde", "arbeitsordner"}`: die gewählten Werte,
   der Text des Arbeitsordners, beim Eintrag „ohne Kunde“ `"kunde": null` (Form der Start-Struktur nach `DI` DI12 Punkt 3). Antwort und Ansage in SE3,
   Fehlerfeld in SE7. Ein Erfolg blendet das Fehlerfeld aus.
   (26.11.) Sobald der Vertrag `dienst` die Felder `name` und `effort` trägt (`rust/spec/VERTRAEGE.md` Abschnitt 4, L10), schickt die Seite `"fassung": 2` mit
   `name` (Text, leer erlaubt) und `effort` (der gewählte Eintrag von `effort-wahl`). Bis dahin bleibt es bei Fassung 1, und das Feld `Effort` fehlt.
4. **`Startwahl` und Modell** (Nachtrag 20.11.2026): Die Nachricht `Startwahl` (Rahmen 0x06) trägt je Route `{"schluessel", "art", "klasse", "modell"}`;
   `modell` ist der Text der Route oder `null` (fehlt es oder ist es kein nicht leerer Text, gilt es wie `null`). Mit der ersten `Startwahl` füllt die Seite
   `harness-wahl` (die Namen der Harness), `route-wahl` (die Schlüssel), `kunde-wahl` (die Kunden, dazu **zuletzt** „ohne Kunde“ mit dem Wert `""`, auch wenn
   `null` in der Liste weiter vorn steht oder fehlt) und `modell-wahl`. Eine Wahl bleibt, wenn es ihren Eintrag nach der Nachricht noch gibt; sonst gilt der erste
   Eintrag, bei `kunde-wahl` „ohne Kunde“. `modell-wahl` bleibt ein Auswahlfeld mit **genau einem** Eintrag: dem Modell der gewählten Route, bei `null` dem Eintrag
   `auto`; er ist gewählt. Ein Wechsel der Route wechselt den Eintrag; eine spätere `Startwahl` ersetzt alle Einträge. `POST /starten` trägt als `modell` genau den
   gewählten Eintrag. Die festen Beispiel-Einträge von `modell-wahl` gelten nur bis zur ersten `Startwahl`; ein Wechsel der Route ändert sie vorher nicht.
5. **Deflate:** Meldet der Strom `X-Deflate: 1`, sind die Nutzlasten der Rasterrahmen (Art 5) gepackt, mit einem Deflate-Kontext je Verbindung (`RA` RA11).
   Nach `inflate_fehler` oder `rahmen_zu_gross` zeigt die Seite den Code im Fehlerfeld (`fehler-code`) mit dem Text „Bild der Verbindung nicht lesbar“, bricht den Strom
   ab und sagt „Verbindung getrennt“ an.

### SE7 Feste Selektoren (Nachtrag 20.11.2026, Pflicht)

Jedes `data-testid` steht **genau einmal** auf der Seite, ist unabhängig von Sprache und sichtbarem Text und ändert sich nicht mit der Gestaltung.
1. `sitzung-liste` (die Liste); darin je Sitzung ein Eintrag `sitzung-<kennung>` (der bedienbare Knopf) mit `data-zustand="laeuft"` oder `"beendet"`,
   bei `beendet` dazu `data-code="<code>"` (bei `laeuft` fehlt `data-code`). Die Attribute folgen jeder neuen `Sitzungsliste`.
2. Startformular: `harness-wahl`, `modell-wahl`, `route-wahl`, `kunde-wahl` (Auswahlfelder; `kunde-wahl` hat den Eintrag „ohne Kunde“, der `null` schickt),
   `arbeitsordner-eingabe`, `sitzung-start` (Knopf).
   (26.11.) Dazu kommen `neue-sitzung` (Knopf „+“), `sitzungen-fenster` (das Fenster), `name-eingabe` und `effort-wahl` (beide sobald L10 gilt; vorher fehlen sie nach `rust/spec/VERTRAEGE.md` OF8),
   `sitzung-beenden` und `sitzung-fortsetzen` (Knöpfe für die gewählte Sitzung) und `sitzungen-schliessen`. Kennungen in einem Fenster oder einer Blase
   stehen genau einmal, solange es offen ist, und fehlen, solange es zu ist.
   Weitere neue Kennungen (26.11.): `inspektor` (der Bereich, nur wenn offen), `inspektor-umschalter`, `ansicht-umschalter` (Orchestrator/Worker),
   `worker-liste` (Blase), `freigaben-blatt`, `ordner-blatt`, `einstellungen` und `admin-einstellungen` (nur für Admins).
3. `terminal-raster`: das Raster als Text im DOM (kein Canvas), mit `data-cursor="<zeile>,<spalte>,<sichtbar>"` (V: `sichtbar` als `true` oder `false`;
   ohne Cursor im Abbild `0,0,false`), darin je Zeile `terminal-zeile-<n>`, n ab 0 in Zeilenfolge, genau so viele wie das Raster Zeilen hat.
4. `eingabe` (Feld der Eingabezeile) und `eingabe-senden` (Knopf). (26.11.) Ruhen mit der Eingabezeile bis nach dem Pilot.
5. `fehler` mit den Kindern `fehler-code` (Code der Ablehnung, unverändert) und `fehler-text` (Text nach SE3 Punkt 3). Der Rahmen 0x08 `Fehler` `{"fehler": "<code>"}` setzt das Fehlerfeld ebenso: Code unverändert, Text
   „Verbindung mit Fehler beendet“. Vor der ersten Ablehnung ist
   `fehler` da, aber nicht sichtbar; nach einer Ablehnung sichtbar mit Code und Text der letzten.

## 5. Festgelegt im Vertrag

1. Prüfpunkte sind die festen Selektoren von SE7; sonst nur Rollen und Namen.
2. AltGr- und Wahltasten-Zeichen sind Text; Alt plus unveränderter Buchstabe ist eine Taste. Auf dem Mac sind damit Wortsprünge über Alt+Buchstabe
   nicht erreichbar (die Wahltaste liefert ein Zeichen); Alt+Links und Alt+Rechts gehen.
3. Reihenfolge der Umschalter `umschalt`, `strg`, `alt`.
4. Meta, Strg+V und Strg+Umschalt+Buchstabe gehören dem Browser.
5. Ereignisse der Reihe nach, eins nach dem anderen.
6. Fokus nach dem Laden ins Terminal der ersten laufenden Sitzung, sonst auf „+“ (26.11.).
7. Tab-Folge wie SE2 Punkt 2, im Fenster „Sitzungen“ nur durch seine Halte; F6 und Umschalt+F6 wechseln die Bereiche (26.11.).
8. Wechsel der Sitzung mit `Abo` aus (`an` false), dann `Abo` an.
9. Ansagen wie SE3 Punkt 3; Sperrzeit 1000 ms; höchstens 5 Zeilen je Ansage; Texte der Gründe.
10. Keine Normalisierung des Textes.
11. Klemme 2 Spalten, keine doppelte Größe.
12. Eingabezeile schickt `Text` und `Taste Enter` erst beim Absenden (ruht bis nach dem Pilot, 26.11.).
13. Kein Abruf eines Symbols; nur drei Routen mit Token.
14. Feste Selektoren SE7; `data-cursor` mit `true`/`false`; Fehlerfeld zeigt die letzte Ablehnung; „ohne Kunde“ schickt `null`.

## 6. Gegenproben (Mutanten)

Jeder Mutant schaltet sich in einer Kopie der Attrappe mit der Zeile `const MUTANT = '<name>';` zu. `gegenprobe.sh` prüft, dass seine Liste gleich der
Liste in `attrappe/seite.js` ist, dass die Attrappe beide Sätze grün besteht und dass jeder Mutant mindestens einen Fall rot macht.

| Mutant | Regel | Änderung |
|---|---|---|
| `komposition_zwischenstand` | SE1.3 | jeder Zwischenstand der Komposition geht als Text |
| `komposition_doppelt` | SE1.3 | das Ergebnis der Komposition geht zweimal |
| `umschalt_enter_cr` | SE1.5 | Umschalt+Enter geht als Enter ohne `umschalt` |
| `altgr_als_taste` | SE1.2 | AltGr- und Wahltasten-Zeichen gehen als Taste mit Umschaltern |
| `umlaut_als_taste` | SE1.1 | Umlaute gehen als Taste |
| `strg_c_als_text` | SE1.6 | Strg+Buchstabe geht als Text |
| `umschalter_sortiert` | SE1.7 | die Umschalter sind alphabetisch sortiert |
| `einfuegen_als_text` | SE1.10 | Einfügen geht als Text |
| `fokus_tab_verlaesst` | SE2.3 | Tab im Terminal bewegt den Fokus |
| `f6_fehlt` | SE2.4 | F6 verlässt das Terminal nicht |
| `fokus_start_dokument` | SE2.1 | nach dem Laden kein Fokus im Terminal |
| `fokus_rahmen_fehlt` | SE2.6 | kein sichtbarer Fokusrahmen |
| `liste_ohne_namen` | SE3.1 | die Sitzungsliste hat keinen Namen |
| `zustand_im_namen_fehlt` | SE3.1 | der Knopf einer Sitzung nennt ihren Zustand nicht |
| `live_terminal` | SE3.2 | die Zeilen des Terminals liegen in einer Live-Region |
| `ansage_jedes_bild` | SE3.3 | jedes Bild sagt alle Zeilen an |
| `ansage_ablehnung_fehlt` | SE3.3 | eine Ablehnung wird nicht angesagt |
| `ansage_beendet_fehlt` | SE3.3 | das Ende einer Sitzung wird nicht angesagt |
| `ansage_beendet_jede_liste` | SE3.3 | jede Liste mit beendeter Sitzung sagt das Ende an |
| `umlaut_nfc` | SE4.3 | die Seite normalisiert den Text nach NFC |
| `zeilen_ohne_leer` | SE4.2 | leere Zeilen fehlen |
| `waagerecht` | SE5.1 | die Seite ist mindestens 900 px breit |
| `spalten_nicht_geklemmt` | SE5.2 | die Seite meldet auch weniger als 2 Spalten |
| `groesse_doppelt` | SE5.2 | die Seite meldet dieselbe Größe noch einmal |
| `eingabezeile_sofort` | SE5.3 | die Eingabezeile schickt jedes Zeichen sofort |
| `autocapitalize_an` | SE5.4 | die Eingabezeile schreibt groß |
| `ziele_klein` | SE5.5 | Knöpfe und Felder sind 20 px hoch |
| `netz_fremde_route` | SE6.1 | die Seite ruft `/favicon.ico` ab |
| `token_in_url` | SE6.1 | das Token steht auch in der Abfrage von `/strom` |
| `token_ganzes_fragment` | SE6.1 | die Seite nimmt das ganze Fragment (mit `t=`) als Token |
| `zeile_testid_ab_eins` | SE7.3 | die Zeilen sind ab 1 statt ab 0 gezählt |
| `cursor_attribut_fehlt` | SE7.3 | `data-cursor` folgt dem Cursor nicht |
| `zustand_attribut_falsch` | SE7.1 | `data-zustand` sagt immer `laeuft`, `data-code` fehlt |
| `kunde_ohne_null` | SE6.3 | „ohne Kunde“ schickt `""` statt `null` |
| `fehler_code_fehlt` | SE7.5 | das Fehlerfeld zeigt den Code nicht |
| `startwahl_ignoriert` | SE6.4 | die Seite liest `Startwahl` nicht |
| `harness_nicht_gefuellt` | SE6.4 | `harness-wahl` bleibt unverändert |
| `route_nicht_gefuellt` | SE6.4 | `route-wahl` bleibt unverändert |
| `kunde_nicht_gefuellt` | SE6.4 | `kunde-wahl` bleibt unverändert |
| `kunde_ohne_kunde_zuerst` | SE6.4 | „ohne Kunde“ steht vor den Kunden |
| `modell_folgt_route_nicht` | SE6.4 | das Modell ist immer das der ersten Route |
| `modell_ohne_auto` | SE6.4 | eine Route ohne Modell ergibt einen leeren Eintrag statt `auto` |
| `modell_beispiele_bleiben` | SE6.4 | die Beispiel-Einträge bleiben nach der `Startwahl` |
| `modell_im_start_falsch` | SE6.4 | `/starten` trägt das Modell der ersten Route statt des gewählten |
| `hinweis_ohne_ansage` | SE3.3 | `osc52_geschrieben` wird nicht angesagt |
| `hinweis_doppelt` | SE3.3 | `osc52_geschrieben` wird zweimal angesagt |
| `hinweis_titel_angesagt` | SE3.3 | ein Hinweis zum Titel wird angesagt |
| `zu_gross_still` | SE3.3 | `zwischenablage_zu_gross` und `nachricht_zu_gross` werden nicht angesagt (Nachtrag 23.11.2026) |
| `zu_gross_fremde_sitzung` | SE3.3 | ein Hinweis zu groß für eine andere Sitzung wird angesagt (Nachtrag 23.11.2026) |
| `hinweis_unbekannt_angesagt` | SE3.3 | ein unbekannter Hinweis wird angesagt (Nachtrag 23.11.2026) |
| `fehler_rahmen_ignoriert` | SE7.5 | der Rahmen `Fehler` setzt das Fehlerfeld nicht |
| `fehler_rahmen_ohne_text` | SE7.5 | der Rahmen `Fehler` setzt den Text nicht |
| `deflate_ignoriert` | SE6.5 | gepackte Rahmen werden als ungepackt gelesen |
| `deflate_kontext_je_rahmen` | SE6.5 | der Deflate-Kontext beginnt mit jedem Rahmen neu |
| `deflate_fehler_still` | SE6.5 | nach `inflate_fehler` bleiben Fehlerfeld und Abbruch aus |
| `fokusfalle_fehlt` | SE2 | Tab verlässt das offene Fenster „Sitzungen“ (Nachtrag 26.11.2026) |
| `f6_ueberspringt_inspektor` | SE2.4 | F6 vom Terminal springt am offenen Inspektor vorbei (Nachtrag 26.11.2026) |
| `platzhalter_sichtbar` | OF8 | ein Bedienweg mit Umfang N steht im Baum (Nachtrag 26.11.2026) |

`eingabezeile_sofort` ruht mit der Eingabezeile (Nachtrag 26.11.2026); `gegenprobe.sh` führt ihn in der Liste `RUHEND` und zählt ihn nicht als überlebt.

## 7. Prüfliste für die Abnahme am Auge (kein Lauf)

Am Mac mit VoiceOver (Safari und Chrome), am Handy mit TalkBack (Chrome, Android) und VoiceOver (Safari, iOS). Je Punkt: gehört, gesehen, Abweichung.
1. Laden mit einer laufenden Sitzung: VoiceOver nennt „Terminal <id>“ und die Beschreibung mit F6. Der Fokusrahmen ist sichtbar.
2. Sitzungsliste mit dem Rotor (Mac) oder Wischen (Handy): „Sitzungen, Liste, 2 Objekte“; jeder Knopf nennt Kennung und „läuft“ oder „beendet, Code …“.
3. Startformular (26.11.: im Fenster „Sitzungen“ hinter „+“): Das Fenster wird als Dialog „Sitzungen“ angesagt, jedes Feld mit seiner Beschriftung; „Starten“ ist ein Knopf; Escape schließt es, und der Fokus steht wieder auf „+“.
4. Im Terminal tippen: Umlaute, `^` plus `e`, `´` plus `e`, AltGr/Wahltaste `@ € { } [ ] \ ~ |` kommen im Programm richtig an; VoiceOver liest das Echo
   nicht doppelt vor.
5. Tab im Terminal bewegt den Fokus nicht; F6 führt in den nächsten Bereich (Inspektor oder Titelleiste), Umschalt+F6 auf den Knopf der gewählten Sitzung (26.11.). Prüfen, dass der Browser F6 an die Seite gibt (Safari, Chrome, Firefox) und dass das App-Fenster F6 nie an das Programm gibt.
6. Eine Ausgabe von 3 Zeilen wird einmal angesagt; eine Dauerausgabe von 30 s flutet nicht (höchstens eine Ansage je Sekunde) und unterbricht das Tippen nicht.
7. Eine Ablehnung (unbekannte Taste) und „Sitzung … beendet“ werden angesagt.
8. Handy: Die Eingabezeile öffnet die Bildschirmtastatur ohne Großschreibung; die Taste „Senden“ der Tastatur schickt; Diktat landet in der Eingabezeile;
   keine waagerechte Leiste bei Hochformat; Tippziele lassen sich treffen. (26.11.) Ruht bis nach dem Pilot, außer „keine waagerechte Leiste“ und „Tippziele“.
9. Der Text im Terminal ist im egui-Fenster und im Browser gleich (Beispiel `umlaute`, mit Auge verglichen).
10. Vergrößern auf 200 % (Mac) und große Schrift (Handy): nichts wird abgeschnitten, keine waagerechte Leiste.

## 8. Fragen und offene Punkte

1. **Deflate.** Die Seite erfährt heute nicht, ob der Dienst Deflate nutzt (`DI` Startdatei `deflate`, Vorgabe true; `BR` reicht die Bytes nur durch).
   Vorschlag: Die Brücke liest `deflate` aus der Startdatei des Dienstes und setzt den Kopf `X-Deflate: 0|1` am Strom; die Attrappe der Brücke im Läufer
   sendet `X-Deflate: 0`. Gegenvorschlag: eine Nachricht des Dienstes am Anfang des Stroms. Alle Fälle laufen heute ohne Deflate; die Attrappe kann kein Deflate.
2. **F6 zum Verlassen des Terminals.** Empfehlung F6/Umschalt+F6 (übliche Taste für den Wechsel zwischen Bereichen, für das Programm ohne Bedeutung im Kern).
   Am Auge zu prüfen, ob jeder Browser F6 an die Seite gibt (Prüfliste Punkt 5). Gegenvorschlag: Strg+F6 oder Esc zweimal schnell (stört vim).
3. **Rolle des Terminals.** Empfehlung `textbox` (ein `textarea` mit `aria-roledescription="Terminal"`): nimmt Komposition und Bildschirmtastatur an und wird
   von VoiceOver und TalkBack als Eingabefeld angesagt. Gegenvorschlag `application` (Tasten gehen am Vorleser vorbei, auf dem Handy schwach unterstützt).
4. **Wahltaste auf dem Mac.** Die Wahltaste liefert Zeichen (Punkt 5.2); „Wahltaste als Meta“ wie im Terminal des Mac fehlt. Soll es eine Einstellung geben?
5. **Hinweise des Kerns** (`osc52_geschrieben`, Titel) haben keine Ansage. Ansagen oder still?
6. **Ansage-Regel** (1000 ms, 5 Zeilen, Vollbild still, Cursorzeile still) ist ein Vorschlag ohne Messung mit Vorleser; die Prüfliste klärt sie.
7. **Text der Ablehnung beim Start.** `BR` antwortet auf `/starten` mit 422 `{"grund"}` ohne `text`; die Prüfstelle `start` liefert aber einen festen Text
   (`DI` DI12 Punkt 4), den das Fehlerfeld zeigen soll. Vorschlag: `BR` reicht `text` in der 422 unverändert mit. Der Fall `se6_starten` hat eine 422 mit und eine ohne `text`.
8. **Woher kommen die Einträge der Auswahlfelder** (Harness, Modell, Route, Kunde)? Vorschlag: aus den Nachrichten `Routen` und `Einstellungen` des Raumkerns.
   Die Attrappe hat feste Einträge; die Fälle wählen nur Werte, die dort stehen.
9. **Einfügen „als Text“.** Der Auftrag nennt Einfügen „als `Text`, Bracketed Paste wie im Kern“. Der Vertrag schickt `Einfuegen` (wie `AB` AB6 und `VT` R1),
   weil nur `Einfuegen` die Klammermarken und die Wandlung der Zeilenumbrüche im Kern bekommt und `Text` Zeilenumbrüche als `steuerzeichen_im_text` ablehnt.
10. **Format von `data-cursor`** (`true`/`false` statt `1`/`0`) ist eine Wahl dieses Vertrags; die Vorführung bestätigt sie für den Drehbuch-Läufer.

## 9. Dateien

- `rust/spec/VERTRAEGE.md` (dieser), `faelle_bauen.py` (außerhalb des Repos; baut beide Sätze; die Rasterrahmen mit `referenz/ra_ref.py`, unveränderte Kopie aus
  der Referenz des Vertrags `rasterauszug` (außerhalb des Repos), SHA-256 `44ce41badb6f6732b503f7bc1355f32202d9a360602857386c2317ca2cd37065`).
- `rust/spec/seite/faelle/` (Datensatz A, sichtbar), `verdeckt/faelle/` (außerhalb des Repos; Datensatz B: andere Sitzungen, Harness, Größen, Texte, Reihenfolgen, Fenster; der
  der Bau sieht ihn nicht).
- Außerhalb des Repos: `laeufer/se_lauf.mjs` (Läufer mit Attrappe der Brücke), `attrappe/index.html` und `attrappe/seite.js` (Attrappe der Seite), `gegenprobe.sh`, `protokoll/` (Läufe).

## Projektentscheide zu Abschnitt 8 (20.11.2026, nach Abnahme des Vertrags)

1. Deflate: Die Brücke setzt am Ereignisstrom `X-Deflate: 0|1` (an den Vertrag `bruecke` gegeben). Fälle mit Deflate folgen, sobald die Brücke es trägt.
2. F6 und Umschalt+F6 verlassen das Terminal: angenommen. Ob jeder Browser F6 an die Seite gibt, prüft die Abnahme am Auge.
3. Rolle des Terminals: `textbox` (`textarea` mit `aria-roledescription`), nicht `application`: angenommen.
4. Wahltaste am Mac: Vorgabe bleibt „liefert Zeichen“ (deutsche Belegung braucht sie für `@`, `[`, `{`). „Wahltaste als Meta“ ist später eine
   Einstellung, nicht im Durchstich.
5. Hinweise des Kerns: Ein Schreiben in die Zwischenablage aus dem Programm (`osc52_geschrieben`) wird einmal höflich angesagt, weil der Mensch es
   wissen soll; Titeländerungen bleiben still.
6. Ansage-Regel (1000 ms, 5 Zeilen): bleibt Vorschlag; die Abnahme am Auge mit VoiceOver misst sie.
7. Ablehnung beim Start: Die Brücke reicht `code` und `text` (an den Vertrag `bruecke` gegeben); `fehler-code` und `fehler-text` zeigen beides.
8. Startwahl: Der Dienst schickt die Nachricht `Startwahl` (Harness-Tabelle, Routen, Kunden aus der Einstellungsdatei; an den Vertrag `dienst` gegeben).
9. Einfügen als `Einfuegen` nach `AB` AB6 und `VT` R1: angenommen.
10. `data-cursor` mit `true`/`false`: mit der Vorführung bestätigt.

## Nachtrag 20.11.2026

Sechs neue Fälle je Satz (Fälle 35 bis 40, beide Sätze je 40): `se6_startwahl`, `se6_modell_auto`, `se3_hinweis_osc52`, `se3_fehler_rahmen`, `se4_deflate`,
`se4_deflate_fehler`. Der Läufer kennt dazu den Fallschlüssel `deflate` (der Strom meldet `X-Deflate: 1`), den Schritt `Fehler` (Rahmen 0x08) und in `Testid`
die Felder `optionen` (genau diese Liste) und `gewaehlt`. Die Attrappe der Seite liest `Startwahl`, `Hinweis`, `Fehler` und gepackte Rahmen (eigener kleiner
Inflater mit Kontext). Die Fälle mit Deflate packen mit `ra_ref.Verbindung(True).verpacken`. Alle 34 früheren Fallateien blieben Byte für Byte gleich.

## Projektentscheide nach dem Bau der Seite (20.11.2026)

Die gebaute Seite (Zweig, Stand vom 20.11.2026) besteht beide Fallsätze im kopflosen Browser am Mac ohne Wiederholung: 34 von 34 sichtbar
und 34 von 34 verdeckt. Zu ihren offenen Punkten:

1. **Modelle im Startformular:** `Startwahl` trägt heute keine Modelle, und die Einstellungsdatei der Fassung 1 kennt keine. Das Feld `modell-wahl`
   bleibt ein Auswahlfeld mit Beispiel-Einträgen. Mit Fassung 2 der Verträge `start` und `einstellungen` (Modell je Route) bekommt `Startwahl` die Modelle je
   Route. Dann füllt die Seite `modell-wahl` daraus. Das kommt als ein Nachtrag zusammen mit dem Nachtrag der `start.*`-Codes.
2. **Feste Beispiel-Einträge bis zur ersten `Startwahl`:** Sie bleiben, solange die Fälle keine `Startwahl` schicken. Der Nachtrag aus Punkt 1 ergänzt
   Fälle mit `Startwahl`. Danach fallen die festen Einträge weg.
3. **Kein Fall prüft bisher** `Startwahl`, `Hinweis` (`osc52_geschrieben`), `X-Deflate: 1` und den Rahmen `Fehler`. Fälle dafür kommen mit demselben
   Nachtrag; `X-Deflate: 1` braucht dazu Deflate in der Attrappe des Läufers.
4. **Kein selbsttätiges Neuverbinden** und **kein selbsttätiger Wechsel** in eine neu gestartete Sitzung: Beides gilt so. Der Fokus folgt nur einer
   Handlung (SE2.1). Ein Knopf „Neu verbinden“ ist nicht Teil des Durchstichs.

## Nachtrag 26.11.2026: Folgen der Entscheide für Fälle und Mutanten

Grundlage: `rust/spec/VERTRAEGE.md` Fassung 2 (Entscheide G1 bis G10, F1 bis F4, E1 bis E8). Kein Fall und kein Mutant wird gelöscht. Was ruht, bleibt
im Fallsatz und läuft wieder, wenn sein Bedienweg kommt. Fallbau, Läufer und Attrappe der Seite (außerhalb des Repos) werden nachgezogen,
bevor die umgebaute Browser-Ansicht gegen die Fälle läuft.

**Fälle, die sich ändern** (sichtbarer Satz, der verdeckte entsprechend):

| Fall | Änderung | Regel |
|---|---|---|
| `se2_start_ohne_sitzung` | Fokus nach dem Laden auf „+“ (`neue-sitzung`) statt auf `Harness` | SE2.1 |
| `se2_tab_folge` | neue Tab-Folge ohne Eingabezeile und ohne festes Startformular; ein zweiter Teil prüft die Halte im Fenster „Sitzungen“ und die Rückkehr auf „+“ | SE2.2 |
| `se2_wechsel` | F6 und Umschalt+F6 nach der Bereichsfolge, wo der Fall sie nutzt | SE2.4 |
| `se3_baum` | Dialog `Sitzungen` mit `listbox` und `form`; keine Eingabezeile; Knopf `Neue Sitzung` | SE3.1 |
| `se6_starten`, `se6_startwahl`, `se6_modell_auto` | vor dem Absenden „+“ klicken; Felder im Fenster; die Start-Struktur bleibt Fassung 1, bis L10 gilt | SE6.3, SE6.4 |
| `se7_selektoren` | neue Kennungen aus SE7 Punkt 2; `eingabe` und `eingabe-senden` fehlen im Pilot | SE7.2, SE7.4 |

**Fälle, die ruhen** (bis die Eingabezeile mit der Chat-Ansicht zurückkommt, B37 und B79): `se5_eingabezeile`. Von `se5_tastatur_attribute` ruht nur
der Teil der Eingabezeile, der Teil des Terminals läuft weiter.

**Mutanten, die ruhen**, mit ihren Fällen: `eingabezeile_sofort` (SE5.3) und der Teil von `autocapitalize_an` für die Eingabezeile (SE5.4).
`f6_fehlt` (SE2.4) und `fokus_start_dokument` (SE2.1) bleiben und prüfen die neue Form.

**Neue Fälle** (vor dem Bau als ausstehend geführt, erst nach dem Umbau grün):
- `se2_sitzungen_fenster`: Öffnen über „+“, Fokus im ersten Feld des Formulars, Fokusfalle im Fenster, Escape schließt und der Fokus steht auf „+“.
- `se2_bereiche`: F6 und Umschalt+F6 durch Titelleiste, Leiste, Terminal und Inspektor, mit offenem und geschlossenem Inspektor.
- `se7_nur_angebunden`: keine Kennung eines Bedienwegs mit Umfang N im Baum (`rust/spec/VERTRAEGE.md` OF7, OF8).
- Dazu die neuen Mutanten `fokusfalle_fehlt` (Tab verlässt das offene Fenster), `f6_ueberspringt_inspektor` (F6 vom Terminal springt am offenen Inspektor
  vorbei) und `platzhalter_sichtbar` (ein Bedienweg mit Umfang N steht im Baum).

Fälle für die Lücken L1 bis L11 (`rust/spec/VERTRAEGE.md` Abschnitt 4) kommen mit den Nachträgen der Verträge `zustandskern` und `dienst`.
