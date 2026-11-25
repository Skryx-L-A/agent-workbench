# Vertrag `seite`, Teil Oberfläche: Code-Ansicht der heutigen Werkbank (A157, A63)

Stand 23.11.2026. Dieser Teil legt fest, **wie die Ansicht aussieht und welche Bedienwege sie zeigt**. Das Verhalten der Seite (Tastatur,
Fokus, Vorleser, Text, Handy, Netz, Selektoren) legt `rust/spec/VERTRAEGE.md` in diesem Ordner fest (SE1 bis SE7). Beide Teile gelten zusammen.

Gegenstand ist die Ansicht der Stufe 2a im Repo: das Crate `rust/crates/ansicht/` (Dioxus-web, Komponenten in `src/oberflaeche.rs`) und der Ordner
`rust/ansicht/` (Stile `ansicht.css`, Bau `bauen.sh`, Prüfung `test/gleichheit.sh`, Ausgabe `ausgabe/`). Zeilenangaben gelten für den Stand von
`development` am 23.11.2026. Electron und die Mac-App sind nur Vorlage für Bedienwege, Maße, Farben und Texte; ihr Code wird nicht übernommen.

„Gemessen“ heißt: mit einem genannten Lauf belegt. „Hergeleitet“ heißt: aus dem Code gelesen, ohne eigenen Lauf.

## 1. Regeln

### OF1 Code-Ansicht der heutigen Werkbank (A157)

1. Die Ansicht ist die Code-Ansicht der heutigen Werkbank mit ihren Funktionen. Sie zeigt Titelleiste, linke Leiste mit Projekten und Sitzungen,
   Fuß der Leiste, Mitte mit Inhaltskopf, Tab-Streifen, Editorzeile, Terminal-Pane und Eingabezeile, rechts das Fenster „Neue Sitzung“ und den Inspektor.
2. Die Agents-Ansicht kommt nicht mit. Der Umschalter „Code / Agents“ zeigt „Code“ gewählt; „Agents“ ist gesperrt (B01).
3. Die Bereiche haben feste Breiten: linke Leiste 232 px, rechte Spalte 360 px, unter 1150 px Fensterbreite 300 px (`ansicht.css:24`, `:322`).
   Verschieben und Größe ändern sind gesperrte Bedienwege (B24, B57).
4. Das Fenster „Neue Sitzung“ steht im Browser als Karte rechts oben und ist immer offen, weil SE2 Punkt 2 seine Felder in der Tab-Folge verlangt.
   Ob es so bleibt oder ein Fenster wird, ist offen (Abschnitt 4, Entscheidung a).
- Prüfmittel: Bilder 1280 px hell und dunkel, Prüfung am Auge (Abschnitt 4).

### OF2 Karten, Ränder, Lücken und Radien

Die Zahlen stehen als Variablen in `rust/ansicht/ansicht.css:20-24` und gelten in beiden Themen.
1. **Karten auf dunklerem Grund:** Der Rahmen `#rahmen` hat 4 px Innenrand und 4 px Lücke (`--fuge: 4px`, `ansicht.css:70-71`). Die Karten
   (linke Leiste, Mitte, rechte Spalte) haben den Radius 10 px (`--r-karte: 10px`). Zwischen den Spalten liegen Fugen von 4 px (`ansicht.css:134`).
2. **Radien:** klein 6 px (`--r-klein`, Knöpfe und Felder), mittel 8 px (`--r-mittel`, Zeilen, Segmente, Tabs), Karte 10 px (`--r-karte`, Karten, Blätter,
   Leisten). Statuspille und Freigabe-Abzeichen 12 px (`ansicht.css:90`, `:204`), Kurzmeldung 14 px (`:318`).
3. **Höhen:** Titelleiste 40 px (`--titelhoehe`), Inhaltskopf mindestens 52 px (`--kopfhoehe`), Tab-Streifen 34 px (`--streifenhoehe`),
   Statuszeile mindestens 28 px (`--statushoehe`), Sitzungszeile 44 px (`--zeilenhoehe`), Projektzeile 32 px (`--projektzeile`), Pane-Kopf 22 px (`:229`).
4. **Abstände der Leisten:** Titelleiste Innenrand 0 6 0 10 px, Abstand 9 px (`:81-82`); Inhaltskopf Innenrand 8 px oben und unten, 12 px links (`:195-196`);
   Tab-Streifen Innenrand 0 12 px, Abstand 6 px (`:209`); Eingabezeile Innenrand 8 12 px, Abstand 8 px (`:256`).
- Prüfmittel: `gleichheit.sh` (die ausgelieferte `index.html` mit eingebettetem `ansicht.css` ist gleich dem frischen Bau), Bilder 1280 px, Prüfung am Auge.

### OF3 Hell und dunkel

1. Die Ansicht folgt dem System (`prefers-color-scheme`, `ansicht.css:27`). Vorgabe ohne Angabe ist dunkel (`:6`).
2. Rollenfarben dunkel und hell (`ansicht.css:8-10`, `:30-32`): `fenster` #0b0d10 und #dfe2e7, `grund` #101216 und #ffffff, `leiste` #171a20 und #f4f5f7,
   `erhoben` #1e232b und #eceef2, `linie` #262b34 und #d8dbe1, `schrift` #d8dee9 und #1b1f27, `gedaempft` #8b93a1 und #5c6472.
3. Akzent dunkel Fläche #0070e0, Text #3398ff; hell Fläche #0070eb, Text #005dc2; Tinte weiß (`:12`, `:33`).
4. Zustandsfarben dunkel: läuft #4ea1ff, wartet #e0a33e, fertig #3fa66a, tot #cd7979; hell #0063d3, #8d6017, #2e7a4e, #b24242 (`:14`, `:34`).
5. Im hellen Thema ist auch das Terminal hell (Palette `:36-39`, Grund `erhoben`). Ein eigenes Thema des Terminals ist später eine Wahl in den
   Einstellungen; heute steht sie gesperrt (B77).
- Prüfmittel: Bilder 1280 px und 375 px je hell und dunkel, Prüfung am Auge.

### OF4 Schrift

1. Grundschrift 13 px mit Zeilenhöhe 1,35, Systemschrift (`ansicht.css:51`). Stufen 17 px (Name im Inhaltskopf), 13 px, 11,5 px, 10,5 px.
2. Terminal: `ui-monospace, SFMono-Regular, Menlo, monospace`, 13 px, Zeilenhöhe 17 px (`:25`, `:242`, `:244`).
3. Die Ansicht lädt keine Schrift von außen (SE6 Punkt 1).
- Prüfmittel: Läufer des Vertrags `seite` (Netzregel), Bilder, Prüfung am Auge.

### OF5 Breite 1280 px

1. Bei 1280 px Breite stehen drei Spalten nebeneinander: linke Leiste, Mitte, rechte Spalte (`ansicht.css:133-135`).
2. Kein Text überdeckt einen anderen. Name und Herkunft im Inhaltskopf enden vor den Knöpfen rechts (`#ik-rechts`).
3. Die Kurzmeldung steht unten mittig über der Eingabezeile, nie auf ihr (`ansicht.css:316-318`).
- Prüfmittel: Bilder 1280 px hell und dunkel, Prüfung am Auge.

### OF6 Handybreite ohne waagerechtes Scrollen

1. Unter 900 px Breite steht alles in einer Spalte (`ansicht.css:325-346`): Leiste, Knöpfe des Inhaltskopfs, Mitte, rechte Spalte, Fuß.
2. Die Seite ist nie breiter als das Fenster, auch bei 360 px und 375 px (SE5 Punkt 1). Das Terminal rollt in sich.
3. Jedes Bedienziel außerhalb des Terminals ist mindestens 44 mal 44 px (`ansicht.css:340`, SE5 Punkt 5).
4. Ausgeblendet sind auf Handybreite Inspektor, Tab-Streifen, Editorzeile, Maschinenkarten, Knöpfe des Pane-Kopfs und die Fugen (`ansicht.css:338`).
   Die Bedienwege darin (B20, B24, B30 bis B33, B51 bis B55, B57 bis B64) sind dort nicht sichtbar.
- Prüfmittel: Läufer des Vertrags `seite` (`se5_schmal`, `se5_klemme`, `se5_eingabezeile`), Bilder 375 px hell und dunkel, Prüfung am Auge.

### OF7 Bedienwege

1. Jeder Bedienweg der Stufe 2a aus Abschnitt 2 hat in der Ansicht ein Element mit `data-bw="Bnn"`. Es sind 66 Bedienwege.
2. Jeder Bedienweg hat genau einen Zustand: `voll` (angebunden), `teilweise` (angebunden, soweit der Neubau die Daten schickt) oder `gesperrt`
   („noch nicht angebunden“, `NNA` in `oberflaeche.rs:31`).
3. Ein Bedienweg erscheint in mindestens einem Zustand der Ansicht. B34 erscheint ohne gewählte Sitzung, B06 bei offenen Anfragen, B42 bei offenem
   Dialog, B05 nach dem Ende des Stroms.
4. Die Gesprächsansicht wird nicht nachgebaut (A169). B13 und B43 bis B50 haben kein Element. B65 und B74 sind nicht in Stufe 2a.
- Prüfmittel: Abgleich der Fundstellen (`grep -n 'data-bw' rust/crates/ansicht/src/oberflaeche.rs`), Zählung von `data-bw` im echten Lauf
  (Skript außerhalb des Repos), Prüfung am Auge.

### OF8 Gesperrt sieht gesperrt aus

1. Ein gesperrter Bedienweg ist sichtbar und nicht bedienbar: Knöpfe und Felder sind `disabled` und nicht fokussierbar.
2. Name (`aria-label`) und Hinweis (`title`) nennen „noch nicht angebunden“. Text ohne Knopf trägt die Klasse `nna-text` (gedämpft, gepunktet unterstrichen).
3. Gesperrte Elemente haben die Deckkraft 0,5 (`.nna`, `ansicht.css:61`). Ein Knopf, der nach dem Zustand gesperrt ist (etwa „Beenden“ einer beendeten
   Sitzung), sieht ebenso aus (`ansicht.css:63`). Gesperrte Knöpfe zeigen den Zeiger `default` (`ansicht.css:53`).
4. Die Marke „noch nicht angebunden“ ist gedämpft, 10,5 px, mit gestricheltem Rand (`.nna-marke`, `ansicht.css:64-65`).
- Prüfmittel: Bilder (Kontextmenü, Einstellungen, beendete Sitzung), Prüfung am Auge.

### OF9 Jede Option hat die Vorgabe „aus“

1. Das Blatt Einstellungen (B72) zeigt sieben Seiten (`SEITEN`, `oberflaeche.rs:1200`) und 17 Optionen (`OPTIONEN`, `oberflaeche.rs:1213`).
2. Jede dieser Optionen steht mit der Vorgabe „aus“: Kästchen `disabled`, nicht gesetzt, Marke „aus“ (`oberflaeche.rs:1291-1297`).
   Das gilt auch für Optionen, die in Electron „an“ sind (zehn Optionen).
3. Thema der Ansicht und Thema des Terminals sind Wahlen, keine Optionen (OF3).
4. Die Ansicht schreibt keine Einstellung (hergeleitet: der Dienst lehnt `EinstellungSetzen` im Lauf ab).
- Prüfmittel: Abgleich der Fundstelle, Bild des Blatts Einstellungen, Prüfung am Auge.

### OF10 Nichts Rechtliches (A31)

1. Die Ansicht stellt keine Frage nach Vertrag, Schulung, Einwilligung oder Rechtsgrundlage.
2. Sie zeigt keine Warnung, die den Ablauf anhält. Die Sicherungen vor jedem Befehl (`guards`) stehen auf „aus“ (OF9).
- Prüfmittel: Prüfung am Auge (hergeleitet aus `oberflaeche.rs`; nicht gemessen).

### OF11 Verhalten nach SE1 bis SE7

1. Die Ansicht besteht beide Fallsätze des Vertrags `seite` (sichtbar und verdeckt) im kopflosen Browser.
2. Die festen Selektoren von SE7 bleiben unverändert. Kein neues `data-testid` beginnt mit `sitzung-` oder `terminal-zeile-`.
- Prüfmittel: Läufer des Vertrags `seite`. Für die Ansicht braucht er drei Änderungen: Er liefert `/ansicht.js` und `/ansicht_bg.wasm` aus, erlaubt sie in
  der Netzregel und sendet `'wasm-unsafe-eval'` in der CSP (Kopie außerhalb des Repos).

### OF12 Browser unter Windows und Safari unter macOS (A63)

1. Die Ansicht läuft unter Windows im Browser (Edge und Chrome). Ergänzung macOS: Sie läuft unter macOS in Safari.
2. Die Seite kommt von `wb-bruecke` über `ssh -L`. Sie lädt genau `index.html`, `ansicht.js` und `ansicht_bg.wasm` und verletzt die CSP nicht.
3. Es gelten OF1 bis OF8 und SE1 bis SE7 in jedem dieser Browser, mit der deutschen Belegung des Systems.

**Prüfplan (noch nicht gemessen):**
1. Windows-VM mit Edge und Chrome, Mac mit Safari. Dienst und Brücke laufen auf dem Kernrechner; der Browser erreicht die Brücke über `ssh -L`.
2. Läufer des Vertrags `seite` mit dem sichtbaren Satz unter Windows (Chromium für Windows). Erwartet: Exit 0, 40 von 40 grün.
3. Echter Lauf je Browser: Seite laden, Sitzung starten, wählen, im Terminal tippen, über die Eingabezeile senden, beenden. Gemessen werden
   Antworten der Brücke, Netzprotokoll (nur die fünf Routen und die zwei Dateien der Ansicht), Fehler der Seite (0) und Verletzungen der CSP (0).
4. Tastatur je System: Umlaute, tote Tasten (`^` plus `e`), AltGr unter Windows (`@ € { } [ ] \ ~ |`, SE1 Punkt 2), Wahltaste in Safari, F6 und
   Umschalt+F6 (Prüfliste Abschnitt 7 von `rust/spec/VERTRAEGE.md`, Punkt 5).
5. Bilder 1280 px hell und dunkel je Browser. Vergleich am Auge mit den Bildern aus Chromium.
6. Ergebnis je Browser: Fassung, System, Exit des Läufers, Zahl der Anfragen, Abweichungen.

### OF13 Abnahme am Auge

1. Die Abnahme am Auge durch den Betreiber steht aus. Sie nimmt die Prüfliste von `rust/spec/VERTRAEGE.md` Abschnitt 7 und die Bilder dieses Teils.
2. Vor der Abnahme gibt es kein deb für die Auslieferung, keine Aufnahme und keine Vorführung mit dieser Ansicht.
3. Zwei Entscheidungen sind vor der Abnahme offen (Abschnitt 4).
- Prüfmittel: Prüfung am Auge.

## 2. Bedienwege

Fundstellen in `rust/crates/ansicht/src/oberflaeche.rs` (Zeile des Elements mit `data-bw`, Funktion). Zählung: **voll 13, teilweise 15, gesperrt 38, zusammen 66.**

| Nr | Bedienweg | Zustand | Umsetzung | Fundstelle |
|---|---|---|---|---|
| B01 | Umschalter Code / Agents | gesperrt | „Code“ gewählt, „Agents“ noch nicht angebunden | `:372` `titelleiste` |
| B02 | Titelleiste: Punkt, Projekt, Sitzung | teilweise | Punkt und Kennung aus `Sitzungsliste`; „Projekt“ ist Platzhalter, die Liste trägt keinen Ordner | `:366` `titelleiste` |
| B03 | Freigabe-Abzeichen mit Zahl | teilweise | Zahl der `Anfragen`, öffnet das Freigaben-Blatt | `:379` `titelleiste` |
| B04 | Inspektor ein- und ausklappen | gesperrt | Knopf rechts, noch nicht angebunden | `:386` `titelleiste` |
| B05 | Warnleiste „Verbindung zum Kern fehlt“ | teilweise | erscheint, wenn der Strom endet oder das Token fehlt | `:402` `kanalwarnung` |
| B06 | Freigabeleiste | teilweise | Anfragen mit Blättern; Freigeben, Ablehnen, Begründung, Zum Worker gesperrt | `:419` `freigabeleiste` |
| B07 | Meldung „Freigabe erteilt/abgelehnt“ | gesperrt | Zeile im Freigaben-Blatt | `:1172` `freigaben_blatt` |
| B08 | Kopf „Projekte“ mit „+“ | voll | bringt „Neue Sitzung“ ins Bild, Fokus auf „Harness“ | `:464` `links` |
| B09 | Projektzeile | gesperrt | Zeile „Projekt“ mit Zahl der Sitzungen | `:477` `links` |
| B10 | Sitzungszeile | voll | Punkt, Kennung, Zusatzzeile; Klick schickt `Abo` | `:484` `links`, Zeile `sitzungszeile` `:499` |
| B11 | Kontextmenü „Fortsetzen“ | gesperrt | Eintrag im Kontextmenü | `:582` `kontextmenue` |
| B12 | Kontextmenü „Namen ändern …“ | gesperrt | ebenda | `:583` `kontextmenue` |
| B14 | „Ordner im Finder zeigen“ | gesperrt | ebenda | `:584` `kontextmenue` |
| B15 | „Sitzung schließen“ | voll | schickt `Beenden`; gesperrt bei beendeter Sitzung | `:585` `kontextmenue` |
| B16 | „Endgültig löschen …“ | gesperrt | ebenda | `:593` `kontextmenue` |
| B17 | „Wiederbeleben …“ | gesperrt | ebenda | `:594` `kontextmenue` |
| B18 | Gesprächszeile in der Leiste | gesperrt | Zeile „Gesprächssitzungen“; ob sie entfällt, ist offen (Entscheidung b) | `:489` `links` |
| B19 | Umsortieren durch Ziehen | gesperrt | Eintrag „Umsortieren (Ziehen)“ | `:595` `kontextmenue` |
| B20 | Maschinenkarten | gesperrt | Karte „Diese Maschine“; auf Handybreite ausgeblendet | `:615` `links_fuss` |
| B21 | Fußzeile Verbrauch | gesperrt | Knopf „Budget: n. v. · Verbrauch“ | `:629` `links_fuss` |
| B22 | Zahl laufender Worker | teilweise | „N Worker laufen“ aus `Agenten` | `:636` `links_fuss` |
| B23 | Zahnrad Einstellungen | teilweise | öffnet das Blatt Einstellungen (B72) | `:641` `links_fuss` |
| B24 | Leistenbreite ziehen | gesperrt | 4-px-Fuge, Breite fest 232 px | `:321` `app` |
| B25 | Inhaltskopf | teilweise | Kennung, Harness, Modell, Route, Kunde; „Ordner“ ist Platzhalter | `:726` `mitte` |
| B26 | Statuspille „N laufen“ | gesperrt | „Worker“ und „noch nicht angebunden“; `Agenten` nennen keine Sitzung; Klick öffnet B29 | `:659` `kopf_rechts` |
| B27 | Orchestrator / Worker | gesperrt | „Orchestrator“ gewählt, „Worker“ gesperrt | `:668` `kopf_rechts` |
| B28 | Zahnrad „Diese Sitzung einstellen“ | gesperrt | Knopf im Inhaltskopf | `:674` `kopf_rechts` |
| B29 | Worker-Liste | teilweise | Blatt mit Worker und Zustand aus `Agenten` | `:1186` `worker_blatt` |
| B30 | Worker-Tabs | gesperrt | „Tab 1“; auf Handybreite ausgeblendet | `:736` `mitte` |
| B31 | Übersicht / Gekachelt / Gezoomt | gesperrt | Segment im Tab-Streifen | `:742` `mitte` |
| B32 | Ordnen | gesperrt | Knopf im Tab-Streifen | `:747` `mitte` |
| B33 | Hinweis zur Kachelform | gesperrt | Text „6 Panes je Tab · 2×3“ | `:740` `mitte` |
| B34 | Leerzustand | voll | Text ohne gewählte Sitzung, in Mitte und Leiste | `:828` `mitte` |
| B35 | Terminalbild | voll | `terminal-raster`, `terminal-zeile-{n}`, `data-cursor` | `:775` `mitte` |
| B36 | Tastatur im Terminal | voll | SE1 mit Komposition und F6 | `:802` `mitte` (`terminal_taste` `:878`, `terminal_eingabe` `:906`) |
| B37 | Eingabezeile mit „Senden“ | voll | `Text` und `Taste Enter` beim Absenden | `:838` `mitte` |
| B38 | Kopieren und Einfügen | teilweise | Einfügen schickt `Einfuegen`; Kopieren bleibt dem Browser | `:851` `mitte` (Marke am Knopf „Senden“), `onpaste` `:813` |
| B39 | Größe an die Fläche anpassen | voll | `Abo` und `Groesse` mit Klemme 2 Spalten | `:776` `mitte` |
| B40 | Rückblick | gesperrt | Marke „Rückblick“ im Pane-Kopf | `:769` `mitte` |
| B41 | Pane-Kopf | teilweise | Punkt, Name, Modell; „Tokens“ mit Hinweis, „Zoom“ gesperrt | `:764` `mitte` |
| B42 | Dialoge der Harnesses | voll | Leiste „Dialog offen“, Klick schickt `Antwort` | `:932` `dialog_teil` |
| B51 | Reiter des Inspektors | gesperrt | „Ordner“, „Aktivität“, „Protokolle“ | `:1089` `rechts` |
| B52 | Ordner-Blatt | gesperrt | Reiter „Ordner“ | `:1090` `rechts` |
| B53 | Inhaltssuche | gesperrt | Suchfeld „Inhalt durchsuchen (rg) …“ | `:1095` `rechts` |
| B54 | Aktivitäts-Blatt | gesperrt | Reiter „Aktivität“ | `:1091` `rechts` |
| B55 | Protokolle-Blatt | gesperrt | Reiter „Protokolle“ | `:1092` `rechts` |
| B56 | Freigaben-Blatt | teilweise | Anfragen mit Sitzung, Werkzeug, Vorschau; Knöpfe gesperrt | `:1157` `freigaben_blatt` |
| B57 | Breite der rechten Spalte ziehen | gesperrt | Fuge, Breite fest 360 px, unter 1150 px 300 px | `:323` `app` |
| B58 | Editor-Tab-Zeile | gesperrt | „Terminal“ gewählt, andere Einträge gesperrt | `:751` `mitte` |
| B59 | Datei öffnen | gesperrt | Eintrag | `:753` `mitte` |
| B60 | Datei speichern | gesperrt | Knopf | `:757` `mitte` |
| B61 | An Orchestrator senden | gesperrt | Knopf; ohne Editor keine Auswahl | `:758` `mitte` |
| B62 | Schnellöffner Cmd+P | gesperrt | Knopf „Schnell öffnen ⌘P“ | `:756` `mitte` |
| B63 | Diff-Tab und Auftrag-Tab | gesperrt | Eintrag „Diff“ | `:754` `mitte` |
| B64 | Editor ein- und ausklappen | gesperrt | Knopf „Einklappen“ | `:759` `mitte` |
| B66 | Meldung „Ergebnis da“ | gesperrt | Glocke in der Fußzeile | `:637` `links_fuss` |
| B67 | Kurzmeldung | voll | Notiz unten mittig für 4 s bei Start, Beenden, `osc52_geschrieben`, `zwischenablage_zu_gross` und `nachricht_zu_gross`; `aria-hidden`, Ansage über die Live-Region | `:1329` `notiz_teil` |
| B68 | Fenster „Neue Sitzung“ | voll | Harness, Modell, Route, Kunde, Arbeitsordner, Starten; Name, Maschine, Pfad, Prüfen gesperrt | `:1017` `rechts` |
| B69 | Modell für diese Sitzung wählen | teilweise | Modelle je Route aus `Startwahl`; Denkstufe und Kontextfenster gesperrt | `:1056` `rechts` |
| B70 | Frühere Sitzungen | teilweise | „Beenden“ bedienbar; „Fortsetzen“ und „Modell wechseln“ gesperrt | `:1080` `rechts` |
| B71 | Meldung bei abgelehntem Start | voll | `fehler`, `fehler-code`, `fehler-text`, Ansage | `:854` `mitte` |
| B72 | Einstellungsfenster | teilweise | sieben Seiten, Werte zum Lesen, Optionen gesperrt mit „aus“ (OF9) | `:1280` `einstellungen_blatt` |
| B73 | Verbrauchsfenster | gesperrt | „Verbrauch“ im Knopf der Fußzeile | `:633` `links_fuss` |
| B75 | Anwendungsmenü | gesperrt | Menüknopf links in der Titelleiste | `:361` `titelleiste` |
| B76 | Escape schließt das Blatt | voll | Escape schließt Blatt und Kontextmenü; im Terminal gehört Escape dem Programm | `:1141` `blatt_rahmen` |
| B77 | Hell, dunkel, wie das System | teilweise | folgt dem System; Wahl und Terminal-Thema gesperrt | `:1301` `einstellungen_blatt` |

Nicht in der Ansicht: B13 und B43 bis B50 entfallen (Gesprächsansicht, A169). B65 (Startseite) und B74 (Erststart) sind nicht in Stufe 2a.

## 3. Prüfmittel

| Prüfmittel | Wo | Regeln |
|---|---|---|
| Läufer des Vertrags `seite` | außerhalb des Repos; Fälle `rust/spec/seite/faelle/` | SE1 bis SE7, OF4, OF6, OF11, OF12 |
| `gleichheit.sh` | `rust/ansicht/test/gleichheit.sh` | OF2 (Ausgabe gleich dem frischen Bau, nur relative Verweise) |
| Bilder | 1280 px und 375 px, hell und dunkel, aus einem echten Lauf | OF1 bis OF8, OF12 |
| Abgleich der Fundstellen | `oberflaeche.rs`, `ansicht.css` | OF2, OF3, OF7, OF8, OF9 |
| Prüfung am Auge | Prüfliste `rust/spec/VERTRAEGE.md` Abschnitt 7 und die Bilder | alle, abschließend OF13 |

## 4. Offene Entscheidungen

a. **Formular „Neue Sitzung“:** offen als Karte rechts oben (heute) oder als Fenster wie in der heutigen Werkbank. Ein Fenster braucht eine Änderung
   von SE2 Punkt 2 (Felder in der Tab-Folge). Entscheidung beim Betreiber.
b. **B18 Gesprächszeile:** Die Gesprächsansicht entfällt (B13, B43 bis B50). Ob die Zeile einer Gesprächssitzung in der Leiste mit entfällt, entscheidet
   der Betreiber. Bis dahin steht sie gesperrt.
