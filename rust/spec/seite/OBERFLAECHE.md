# Vertrag `seite`, Teil Oberfläche: Gestalt der heutigen Werkbank (A157, A63, A118)

Stand 26.11.2026, Fassung 2. Sie ersetzt die Fassung vom 23.11.2026. Grundlage sind die Entscheide des Betreibers vom 26.11.2026
(`oberflaeche-entscheide-2026-11-26.md` in der Planung, außerhalb des Repos; Kennungen G1 bis G10, F1 bis F4, E1 bis E8). Jede Regel nennt ihren Entscheid.

Dieser Teil legt fest, **wie die Oberfläche aussieht, welche Bedienwege sie hat und was sie dafür vom Kern und vom Dienst braucht**. Das Verhalten
(Tastatur, Fokus, Vorleser, Text, Netz, Selektoren) legt `rust/spec/VERTRAEGE.md` in diesem Ordner fest (SE1 bis SE7). Beide Teile gelten zusammen.

## 0. Gegenstand und Grundsatz

1. **Zwei Ansichten, eine Liste der Bedienwege.** Es gibt zwei Ansichten mit derselben Gestalt, demselben Zustandskern und derselben Liste der Bedienwege
   (Abschnitt 2):
   - **App-Fenster** (egui), mit der Terminalansicht. Es ist die Oberfläche des Pilots (E5). Regeln dafür stehen in OF14.
   - **Browser-Ansicht** (Dioxus-web, Crate `rust/crates/ansicht/`, Ordner `rust/ansicht/`). Sie dient M1, also der Vorführung und Schritt I (E6).
     Nach M1 ruht sie bis nach dem Pilot.
2. **Grundsatz (Betreiber, wörtlich):** „ich finde das was sich von der jetzigen ansicht unterscheidet gefällt mir an der jetzigen ansicht besser“.
   Wo eine Ansicht von der heutigen Mac-App abweicht, gilt die heutige Mac-App. Ausdrückliche Ausnahmen sind die Farbwahl (OF3), die Einstellungen
   (OF9) und später die Verbrauchsansicht (F4).
3. **Vorlage** sind die Bilder der heutigen Mac-App in `the previous implementation/` (`haupt-orchestrator-*.png`, `haupt-worker-*.png`,
   `haupt-workerliste-*.png`, `inspektor-*.png`, `sitzungsfenster-*.png`, `einstellungen-*.png`). Ihr Code wird nicht übernommen, auch nicht der
   von Electron. Beide liefern nur Bedienwege, Maße, Farben und Texte.
4. **Pilot und danach:** Jeder Bedienweg trägt in Abschnitt 2 den Umfang **Pilot**, **nach dem Pilot** oder **entfällt**. Im Pilot ist nur zu sehen,
   was zum Pilot gehört und angebunden ist (OF8).
5. „Gemessen“ heißt: mit einem genannten Lauf belegt. „Hergeleitet“ heißt: aus Code oder Bild gelesen, ohne eigenen Lauf.

## 1. Regeln

### OF1 Aufbau wie die heutige Mac-App (A157; G3, G5, G7, G8)

1. Die Oberfläche hat drei Bereiche: die **Titelleiste** über der ganzen Breite, die **Leiste** links und die **Mitte**. Rechts liegt der **Inspektor**.
   Er ist einklappbar und in der Vorgabe zu (G5).
2. Die **Titelleiste** ist eine Zeile (G3):
   - links der Knopf zum Ein- und Ausblenden der Leiste;
   - dann der Projektname fett, darunter klein „Harness · Modell · Ordner“ der gewählten Sitzung (B02);
   - rechts die Statuspille „N laufen“ (B26), Orchestrator/Worker (B27), der Inspektor-Knopf (B04) und das Zahnrad (B28).
   - Das Freigabe-Abzeichen (B03) erscheint nur bei einer offenen Anfrage (G4).
   - Es gibt keinen eigenen Inhaltskopf und keine Brotkrume.
   - Der Umschalter Code/Agents erscheint erst mit der Agents-Ansicht nach dem Pilot (B01).
3. Die **Leiste** zeigt die Sitzungen nach Projektordner, mit dem Kopf und „+“ (B08), dem Projektnamen als kleiner grauer Überschrift mit Zahl (B09) und
   den Sitzungszeilen (B10). Im Fuß stehen „N Worker laufen“ (B22) und das Zahnrad (B23) (G8).
4. Die **Mitte** zeigt in der Ansicht „Orchestrator“ genau ein Terminal-Pane über die ganze Fläche. In der Ansicht „Worker“ zeigt sie die Panes
   der Worker der gewählten Sitzung gekachelt (B27, B30). Über dem Terminal steht keine Werkzeugzeile:
   - Den Tab-Streifen gibt es nur in der Ansicht „Worker“, und auch dort erst, wenn mehr Worker laufen, als eine Kachelform fasst (B30 bis B33, G7).
   - Die Editorzeile kommt nach dem Pilot (B58 bis B64).
   - **Kacheln** (Projektentscheid, 27.11.2026): jede Kachel 8 px Eckenrundung und 24 px Kopfzeile, zwischen zwei Kacheln 4 px Fuge im Grund der Bühne, die aktive Kachel 2 px Rahmen innen, nur wenn mehr als eine Kachel da ist; in hell und dunkel, gekachelt und gezoomt. Der Grund der Bühne hebt sich im dunklen Thema sichtbar vom Terminal ab: grau dunkel #292a2a (Grund der Leiste der heutigen App, gemessen; deren Fuge #1f2020 hat zum Terminal #1e1e1e nur 1,02:1), blauschwarz #1b1e22 (derselbe Abstand auf #101216), hell #ffffff. Der Rahmen nimmt unter macOS die Akzentfarbe des Systems (`AccentColor`, wo der Browser sie kennt), sonst den Akzent des Themas wie das App-Fenster: hell #3478f6, grau dunkel #5e94f8, blauschwarz #0070e0.
5. Der **Inspektor** hat Symbolreiter wie heute: Schild (Freigaben, B56), Ordner (B52), Uhr (Aktivität) und Blatt mit Lupe (Protokolle). Im Pilot gibt es
   nur Freigaben und Ordner (B51).
6. Das **Fenster „Sitzungen“** öffnet sich in der Oberfläche über „+“ (F1, B68). Es steht nie fest in einer Spalte.
- Prüfmittel: Bilder 1280 px hell und dunkel neben den Vorlagebildern, Prüfung am Auge (OF13).

### OF2 Flächig, Maße wie heute (G1)

1. **Flächig:** Die Leiste ist eine abgesetzte Fläche mit eigenem Grund. Mitte und Inspektor schließen ohne Fuge und ohne Karte an. Es gibt keinen
   dunkleren Grund zwischen den Bereichen.
2. **Maße:** Höhe der Titelleiste, Breite der Leiste, Höhe von Sitzungszeile, Projektzeile und Pane-Kopf, Radien von Knöpfen, Feldern und gewählter
   Zeile, Innenränder und Abstände folgen den Vorlagebildern.
   - Die Werte stehen als Variablen an einer Stelle: in `ansicht.css` für die Browser-Ansicht und in einem Stilmodul für das App-Fenster.
     Beide Ansichten nutzen dieselben Zahlen.
   - Wer sie festlegt, misst sie an den Vorlagebildern. Die Bilder sind Retina, 2 Pixel sind 1 Punkt.
3. **Breiten ziehen:** Leiste und Inspektor lassen sich in der Breite ziehen (B24, B57). Die Grenzen sind: Leiste 180 bis 400 px, Inspektor 260 bis 560 px.
   Die Mitte behält mindestens 480 px. Die Breiten werden je Gerät gemerkt (L8).
- Prüfmittel: `gleichheit.sh` (Browser), Bilder 1280 px neben den Vorlagebildern, Prüfung am Auge.

### OF3 Farben: Vorgabe wie heute, alles wählbar (G2)

1. **Vorgabe wie heute:** Das dunkle Thema ist neutral grau ohne Blaustich, das Terminal ist in beiden Themen dunkel. Die Rollenfarben (`fenster`,
   `grund`, `leiste`, `erhoben`, `linie`, `schrift`, `gedaempft`, Akzent, Zustandsfarben) für grau dunkel und hell werden an den Vorlagebildern gemessen.
   Sie stehen als Variablen an derselben Stelle wie die Maße (OF2 Punkt 2).
2. **Drei Wahlen, frei mischbar** (Einstellungen, Seite Aussehen, B77):
   - Thema: wie das System, hell oder dunkel (Vorgabe: wie das System).
   - Farbsatz des dunklen Themas: grau (Vorgabe) oder blauschwarz.
   - Terminal: dunkel (Vorgabe), hell oder wie das Thema.
3. **Blauschwarz** sind die Rollenfarben der Fassung vom 23.11.2026: `fenster` #0b0d10, `grund` #101216, `leiste` #171a20, `erhoben` #1e232b,
   `linie` #262b34, `schrift` #d8dee9, `gedaempft` #8b93a1. Akzent und Zustandsfarben bleiben wie dort.
4. **Helles Terminal** ist die Palette der Fassung vom 23.11.2026 (`ansicht.css:36-39`, Grund `erhoben`).
5. Jede Kombination hält einen Kontrast von mindestens 4,5:1 zwischen Schrift und Grund und mindestens 3:1 für Bedienelemente.
- Prüfmittel: Bilder aller sechs Kombinationen aus Thema und Terminal bei 1280 px, Kontrastmessung je Rollenfarbe (Liste im Ergebnis), Prüfung am Auge.

### OF4 Schrift

1. Grundschrift ist die Systemschrift in der Größe der Vorlage. Die Stufen sind Projektname in der Titelleiste, Grundtext und kleine Zusatzzeile.
2. Terminal: `ui-monospace, SFMono-Regular, Menlo, monospace` im Browser, im App-Fenster eine mitgelieferte Monospace-Schrift mit Umlauten und
   Rahmenzeichen. Die Größe ist 13 px und in den Einstellungen wählbar.
3. Die Browser-Ansicht lädt keine Schrift von außen (SE6 Punkt 1). Das App-Fenster lädt nichts aus dem Netz.
- Prüfmittel: Läufer des Vertrags `seite` (Netzregel), Bilder, Prüfung am Auge.

### OF5 Breite 1280 px

1. Bei 1280 px stehen Leiste und Mitte nebeneinander, bei offenem Inspektor auch der Inspektor.
2. Kein Text überdeckt einen anderen. Projektname und Herkunft in der Titelleiste enden vor den Knöpfen rechts und kürzen mit „…“.
3. Die Kurzmeldung (B67) steht unten mittig über dem Terminal und verdeckt keine Bedienelemente.
- Prüfmittel: Bilder 1280 px hell und dunkel, Inspektor offen und zu, Prüfung am Auge.

### OF6 Handybreite (Browser)

1. Die Handybreite bekommt bis nach dem Pilot keine neue Arbeit (F3: „handy kommt später, jetzt keine zeit darauf verwenden“).
2. Es gilt nur: Die Seite ist nie breiter als das Fenster, auch bei 360 px und 375 px (SE5 Punkt 1). Bedienziele außerhalb des Terminals sind
   mindestens 44 mal 44 px (SE5 Punkt 5). Unter 900 px steht alles in einer Spalte, und der Inspektor ist zu.
3. Nach dem Pilot kommt das Handy mit der Chat-Ansicht als Vorgabe (F3, B79).
- Prüfmittel: Läufer des Vertrags `seite` (`se5_schmal`, `se5_klemme`).

### OF7 Bedienwege

1. Jeder Bedienweg mit dem Umfang **Pilot** aus Abschnitt 2 hat in jeder Ansicht ein Element mit der Kennung `Bnn`.
   - In der Browser-Ansicht trägt es `data-bw="Bnn"`.
   - Im App-Fenster trägt es die Kennung als Element-ID im Barrierebaum.
2. Ein Bedienweg erscheint, sobald er angebunden ist und sein Anlass vorliegt. Beispiele: B03 bei einer offenen Anfrage, B34 ohne gewählte Sitzung,
   B42 bei offenem Dialog, B05 nach dem Ende des Stroms.
3. Bedienwege mit dem Umfang **nach dem Pilot** haben im Pilot kein Element. Bedienwege mit dem Umfang **entfällt** haben nie eines.
- Prüfmittel: Abgleich der Kennungen gegen Abschnitt 2, Zählung im echten Lauf, Prüfung am Auge.

### OF8 Nur, was funktioniert (G6)

1. Was nicht angebunden ist, steht nicht da. Es gibt keine Marke „noch nicht angebunden“, keinen gesperrten Platzhalter und keine halbtransparenten
   Knöpfe für fehlende Funktionen. Die Fassung vom 23.11.2026 (OF8, `.nna`, `NNA`) ist aufgehoben.
2. Ein Knopf, der nur nach dem Zustand nicht geht, ist sichtbar und gesperrt. Beispiel: „Beenden“ einer beendeten Sitzung. Er trägt einen Hinweis
   mit dem Grund, etwa „Sitzung ist beendet“.
3. Fehlt einer Ansicht eine Nachricht des Kerns, die ein Pilot-Bedienweg braucht (Abschnitt 4), so fehlt der Bedienweg im echten Lauf. Ausgenommen ist
   die Grundform (Spalte „Grundform“ in Abschnitt 2): Sie erscheint, nur ihre Teile aus der Lücke fehlen. Bilder zur
   Abnahme entstehen dann mit der Attrappe der Brücke, die die künftigen Nachrichten schickt. Jedes solche Bild trägt die Beschriftung
   „Attrappe, Zielzustand“ (OF13).
- Prüfmittel: Bilder, Abgleich gegen Abschnitt 2, Prüfung am Auge.

### OF9 Einstellungen: Nutzer und Admin getrennt, besser gestaltet (F2, E1, E2, E3)

1. **Zwei getrennte Bereiche:** die Einstellungen des Nutzers (B72) und die Admin-Einstellungen (B78). Den Admin-Bereich sieht nur, wer Admin ist.
2. **Wer Admin ist (E1):** Admin ist ein Konto aus der Liste in der Firmenvorgabe, also ein Benutzername am Rechner. Die IT legt die Liste an, ändern
   lässt sie sich nur mit Systemrechten. Ob ein Betrachter Admin ist, entscheidet der Dienst, nie die Ansicht (Abschnitt 4, L4).
3. **Nutzer (B72):**
   - Aussehen: die drei Wahlen aus OF3 und die Größe der Terminalschrift.
   - Sitzung: Vorgaben für neue Sitzungen, Harness und Modell aus der Freigabe der Admins, dazu die **Vorgabe des Efforts**.
   - Effort: Den Effort einer Sitzung stellt der Nutzer selbst ein, innerhalb der Grenzen der Admins (E2).
   - Regeln: Der Nutzer sieht jede wirksame Regel. Er darf sie für sich nur verschärfen (A26).
   - Maschinen und Programm zeigen nur Werte.
4. **Admin (B78):** Harnesses und Modelle je Stufe, Grenzen des Efforts je Stufe und Modell, Erlaubnisse, Aufsicht und Wachen, Routen und Datenklassen
   und alle Optionen (A30). Jede Änderung steht im Protokoll, mit Admin, Ebene, altem und neuem Wert und Zeit (A27).
5. **Ebenen:** Regeln gelten auf den Ebenen Firma, Gruppe und Nutzer (A26). Jede Einstellung zeigt ihren **wirksamen Wert und seine Herkunft**, etwa
   „Firma“, „Gruppe Entwicklung“ oder „du“ (E3).
6. **Gestaltung (E3, Betreiber wörtlich):** „arbeite mehr visuell, mit symbolen und kurzen passenden beschreibungen, halte dich an desing standarts
   und an das was du denkst“. Daraus folgt:
   - Je Seite ein Symbol.
   - Gruppen mit Überschrift.
   - Je Einstellung eine Zeile mit Name, Schalter oder Wahl und einer Beschreibung von höchstens einer Zeile.
   - Keine Textabsätze.
   - Was gesperrt ist, nennt den Grund kurz, etwa „Vorgabe der Firma“.
   - Hier weicht die Gestalt ausdrücklich von der heutigen App ab.
7. **Vorgabe „aus“:** Jede Option der Admins hat die Vorgabe „aus“, bis ein Admin sie umstellt (A30, A31). Aussehen und Effort sind Wahlen, keine
   Optionen.
8. Die Ansicht schreibt eine Einstellung nur über das Ereignis `EinstellungSetzen` (Abschnitt 4, L4). Ob es gilt, entscheidet der Dienst.
- Prüfmittel: Bilder der Seiten Aussehen, Sitzung, Harnesses und Modelle (als Nutzer und als Admin), ein Fall je Ablehnung (Nutzer lockert, Nichtadmin
  schreibt Adminwert), Prüfung am Auge.

### OF10 Nichts Rechtliches (A31)

1. Die Oberfläche stellt keine Frage nach Vertrag, Schulung, Einwilligung oder Rechtsgrundlage.
2. Sie zeigt keine Warnung, die den Ablauf anhält.
- Prüfmittel: Prüfung am Auge.

### OF11 Verhalten nach SE1 bis SE7

1. Die Browser-Ansicht besteht beide Fallsätze des Vertrags `seite` (sichtbar und verdeckt) im kopflosen Browser.
2. Die festen Selektoren von SE7 bleiben unverändert. Neue Kennungen sind in SE7 genannt. Keine neue Kennung beginnt mit `sitzung-` oder
   `terminal-zeile-`, außer den dort genannten.
- Prüfmittel: Läufer des Vertrags `seite`.

### OF12 Browser unter Windows und Safari unter macOS (A63)

1. Die Browser-Ansicht läuft für M1 unter Windows in Edge und Chrome und unter macOS in Safari.
2. Die Seite kommt von `wb-bruecke` über `ssh -L`. Sie lädt genau `index.html`, `ansicht.js` und `ansicht_bg.wasm` und verletzt die CSP nicht.
3. Es gelten OF1 bis OF8 und SE1 bis SE7 in jedem dieser Browser, mit der deutschen Belegung des Systems.
- Prüfplan wie in der Fassung vom 23.11.2026 (Windows-VM mit Edge und Chrome, Mac mit Safari, Läufer, echter Lauf je Browser, Tastatur, Bilder).
  Er ist noch nicht gemessen.

### OF13 Abnahme am Auge

1. Der Betreiber nimmt jede Ansicht am Auge ab. Grundlage sind die Bilder dieses Teils und die Prüfliste in `rust/spec/VERTRAEGE.md` Abschnitt 7.
2. Vor der Abnahme der Browser-Ansicht gibt es kein deb an die Firma, keine Aufnahme und keine Vorführung mit ihr. Für das App-Fenster gilt dasselbe
   vor dem Pilot.
3. Die Bilder zur Abnahme zeigen dunkel und hell bei 1280 px:
   - die Ansicht „Orchestrator“ mit Inspektor zu und offen (Reiter Freigaben und Ordner);
   - die Ansicht „Worker“ mit Kacheln und die Worker-Liste;
   - das Fenster „Sitzungen“ mit leerem Formular und mit gewählter beendeter Sitzung;
   - die Einstellungen Aussehen, Sitzung, Harnesses und Modelle, als Nutzer und als Admin.
   Bilder aus der Attrappe tragen die Beschriftung „Attrappe, Zielzustand“ (OF8 Punkt 3).
- Prüfmittel: Prüfung am Auge.

### OF14 App-Fenster (egui; E5)

1. Das App-Fenster ist die Oberfläche des Pilots. Es zeigt die Terminalansicht mit allen Bedienwegen des Umfangs Pilot (Abschnitt 2).
2. Es spricht mit dem Dienst über dieselben Nachrichten und Ereignisse wie die Browser-Ansicht (Vertrag `dienst`). Es nutzt den Weg des Dienstes für
   lokale Betrachter, ohne Brücke. Logik liegt im Zustandskern, nie in der Ansicht (Plan, Abschnitt Oberfläche).
3. Es läuft unter Windows, macOS und Linux. Zuerst kommt das System, auf dem das Pilotteam arbeitet.
4. **Tastatur:** Es schickt dieselben Ereignisse wie SE1, mit deutscher Belegung, toten Tasten, AltGr und Wahltaste. Einfügen geht als `Einfuegen`.
   Kopieren einer Auswahl im Terminal geht in die Zwischenablage des Systems (B38).
5. **Fokus:** Es gilt die Bereichsfolge aus SE2 Punkt 4: F6 und Umschalt+F6 wechseln zwischen Leiste, Terminal, Inspektor und Titelleiste.
6. **Vorleser:** Der Barrierebaum der Plattform (AccessKit) trägt dieselben Rollen und Namen wie SE3 Punkt 1 und dieselben Ansagen wie SE3 Punkt 3.
7. **Gleicher Text:** SE4 gilt. Der Text jeder Zeile ist im App-Fenster und im Browser gleich.
8. Das Fenster merkt sich Größe, Lage, Breiten von Leiste und Inspektor, den Zustand des Inspektors und die Reihenfolge der Sitzungen je Gerät (L8).
- Prüfmittel: ein Läufer für das Fenster, der die Fälle des Vertrags `seite` sinngemäß über den Barrierebaum der Plattform und künstliche Tasten fährt.
  Er ist noch zu bauen. Dazu Bilder wie OF13 und die Prüfliste `rust/spec/VERTRAEGE.md` Abschnitt 7.

## 2. Bedienwege

Umfang: **P** Pilot, **N** nach dem Pilot, **E** entfällt. „Daten“ nennt die Lücke aus Abschnitt 4, die der Bedienweg braucht. „Grundform“ heißt:
Der Bedienweg erscheint schon vor seiner Lücke in der Form, die der Vertrag `seite` heute verlangt (SE2, SE3, SE6, SE7). Nur die Teile, die die Lücke
braucht, fehlen bis dahin, etwa die Worker-Zahl in B10 oder der Ordner in B02. Beide Ansichten lesen diese Spalte (OF8 Punkt 3).

| Nr | Bedienweg | Umfang | Gestalt und Verhalten | Daten | Grundform | Entscheid |
|---|---|---|---|---|---|---|
| B01 | Umschalter Code / Agents | N | erscheint mit der Agents-Ansicht | – | – | G3, Pilot-Tabelle |
| B02 | Titelleiste: Projektname mit Herkunft | P | Projektname fett, darunter „Harness · Modell · Ordner“ der gewählten Sitzung | L5 | ja | G3 |
| B03 | Freigabe-Abzeichen | P | nur bei offener Anfrage, mit Zahl; öffnet den Inspektor im Reiter Freigaben | – | – | G4 |
| B04 | Inspektor ein- und ausklappen | P | Knopf rechts in der Titelleiste; Vorgabe zu; je Gerät gemerkt | L8 | ja | G5 |
| B05 | Warnleiste „Verbindung zum Kern fehlt“ | P | erscheint, wenn der Strom endet oder das Token fehlt | – | – | – |
| B06 | Freigabeleiste | E | geht in B56 auf | – | – | G4 |
| B07 | Meldung „Freigabe erteilt/abgelehnt“ | P | Zeile im Guard-Verlauf des Reiters Freigaben | L6 | – | G4 |
| B08 | Kopf der Leiste mit „+“ | P | öffnet das Fenster „Sitzungen“ mit leerem Formular | – | ja | F1 |
| B09 | Projektzeile | P | Projektname als kleine graue Überschrift, Zahl der Sitzungen rechts | L5 | – | G8 |
| B10 | Sitzungszeile | P | Punkt, Name, darunter „N Worker · Zustand“; gewählte Zeile grau; Klick schickt `Abo` | L1, L5 | ja | G8 |
| B11 | „Fortsetzen“ | P | Kontextmenü und Fenster „Sitzungen“; nur bei beendeter Sitzung | L3 | – | Pilot-Tabelle |
| B12 | „Namen ändern …“ | P | Kontextmenü; Name auch im Formular beim Start | L3 | – | Pilot-Tabelle |
| B14 | „Ordner im Finder zeigen“ | N | – | – | – | Pilot-Tabelle |
| B15 | „Sitzung schließen“ | P | schickt `Beenden`; bei beendeter Sitzung gesperrt mit Grund | – | – | – |
| B16 | „Endgültig löschen …“ | P | nur bei beendeter Sitzung, mit Rückfrage | L3 | – | Pilot-Tabelle |
| B17 | „Wiederbeleben …“ | P | bei abgestürzter Sitzung | L3 | – | Pilot-Tabelle |
| B18 | Gesprächszeile in der Leiste | E | – | – | – | Projektentscheid 23.11., G8 |
| B19 | Umsortieren durch Ziehen | P | Reihenfolge der Sitzungen je Projekt, je Gerät gemerkt | L8 | – | Pilot-Tabelle |
| B20 | Maschinenkarte | N | – | – | – | Pilot-Tabelle |
| B21 | „heute“ im Fuß der Leiste | N | kommt mit der Verbrauchsansicht | – | – | F4, L7 |
| B22 | Zahl laufender Worker | P | „N Worker laufen“ im Fuß, Zahl fett | L1 | – | G8 |
| B23 | Zahnrad im Fuß der Leiste | P | öffnet die Einstellungen (B72) | – | ja | G8 |
| B24 | Breite der Leiste ziehen | P | OF2 Punkt 3 | L8 | – | Pilot-Tabelle |
| B25 | Inhaltskopf | E | geht in B02 auf | – | – | G3 |
| B26 | Statuspille „N laufen“ | P | in der Titelleiste, Punkt und Zahl; Klick öffnet die Worker-Liste (B29) als Blase | L1 | – | G3 |
| B27 | Orchestrator / Worker | P | in der Titelleiste; „Worker“ zeigt die Panes der Worker gekachelt | L1, L2 | – | Pilot-Tabelle |
| B28 | Zahnrad in der Titelleiste | P | öffnet die Einstellungen auf der Seite Sitzung | L4 | – | G3 |
| B29 | Worker-Liste | P | Blase unter der Pille: Worker der gewählten Sitzung mit Rolle, Modell und Zustand | L1 | – | Pilot-Tabelle |
| B30 | Worker-Tabs | P | erst, wenn mehr Worker laufen, als eine Kachelform fasst (6 je Tab) | L2 | – | G7 |
| B31 | Übersicht / Gekachelt / Gezoomt | P | mit B30 | L2 | – | G7 |
| B32 | Ordnen | P | mit B30 | L2 | – | G7 |
| B33 | Hinweis zur Kachelform | P | mit B30 | L2 | – | G7 |
| B34 | Leerzustand | P | Text ohne gewählte Sitzung, in Mitte und Leiste | – | – | – |
| B35 | Terminalbild | P | `terminal-raster`, `terminal-zeile-{n}`, `data-cursor` | – | – | – |
| B36 | Tastatur im Terminal | P | SE1, F6 nach SE2 Punkt 4 | – | – | – |
| B37 | Eingabezeile mit „Senden“ | N | kommt mit der Chat-Ansicht; im Pilot tippt man direkt ins Terminal | – | – | F3 |
| B38 | Kopieren und Einfügen | P | Einfügen schickt `Einfuegen`; Kopieren einer Auswahl im Terminal | – | – | – |
| B39 | Größe an die Fläche anpassen | P | `Abo` und `Groesse` je sichtbarem Pane, Klemme 2 Spalten | L2 | ja | – |
| B40 | Rückblick | N | – | – | – | G9 |
| B41 | Pane-Kopf | P | Punkt, Rolle in Akzentfarbe, Modell, rechts Vergrößern; Sprechblase und „Tokens“ nach dem Pilot | L1 | ja | G9 |
| B42 | Dialoge der Harnesses | P | Leiste „Dialog offen“, Klick schickt `Antwort` | – | – | – |
| B51 | Reiter des Inspektors | P | Symbolreiter; im Pilot Freigaben und Ordner | – | ja | G4, G10 |
| B52 | Ordner-Blatt | P | wie heute: Pfad, darunter der Inhalt des Arbeitsordners | L9 | – | G10 |
| B53 | Inhaltssuche | P | Feld „Im Inhalt suchen“ unter dem Pfad | L9 | – | G10 |
| B54 | Aktivitäts-Blatt | N | – | – | – | Pilot-Tabelle |
| B55 | Protokolle-Blatt | N | – | – | – | Pilot-Tabelle |
| B56 | Freigaben-Blatt | P | wie heute: „Anträge“, „Angehaltene Worker“, „Guard-Verlauf“ mit Zahl; Freigeben, Ablehnen, Begründung, Zum Worker | L6 | ja | G4 |
| B57 | Breite des Inspektors ziehen | P | OF2 Punkt 3 | L8 | – | Pilot-Tabelle |
| B58 | Editor-Tab-Zeile | N | erscheint erst mit offener Datei | – | – | G7 |
| B59 | Datei öffnen | N | – | – | – | Pilot-Tabelle |
| B60 | Datei speichern | N | – | – | – | Pilot-Tabelle |
| B61 | An Orchestrator senden | N | – | – | – | Pilot-Tabelle |
| B62 | Schnellöffner Cmd+P | N | – | – | – | Pilot-Tabelle |
| B63 | Diff-Tab und Auftrag-Tab | N | – | – | – | Pilot-Tabelle |
| B64 | Editor ein- und ausklappen | N | – | – | – | Pilot-Tabelle |
| B66 | Meldung „Ergebnis da“ | N | – | – | – | Pilot-Tabelle |
| B67 | Kurzmeldung | P | unten mittig über dem Terminal, 4 s; Ansage über die Live-Region | – | – | – |
| B68 | Fenster „Sitzungen“ | P | wie heute, als Fenster in der Oberfläche: Liste aller bekannten Sitzungen nach Ordner, auch der beendeten; Formular mit Name und Effort (beide ab L10), Arbeitsordner, Harness, Modell, Route, Kunde und „Starten“; unten „Beenden“ und „Fortsetzen“ für die gewählte Sitzung | L3, L5, L10 | ja | F1 |
| B69 | Modell und Effort beim Start | P | Modelle je Route aus `Startwahl`; Effort innerhalb der Grenzen der Admins, vorbelegt mit der Vorgabe des Nutzers; Kontextfenster nach dem Pilot | L10 | ja | E2 |
| B70 | Frühere Sitzungen | E | geht in B68 auf; „Modell wechseln“ nach dem Pilot | – | – | F1 |
| B71 | Meldung bei abgelehntem Start | P | `fehler`, `fehler-code`, `fehler-text`, Ansage; im Fenster „Sitzungen“ unter „Starten“ | – | ja | – |
| B72 | Einstellungen des Nutzers | P | OF9 Punkt 3, 5, 6 | L4 | ja | F2, E2, E3 |
| B73 | Verbrauchsansicht | N | neu nach F4 und E4: Verlauf als Grafik mit Hochrechnung, je Konto und Grenze, je Sitzung und Worker mit Modell und Effort, Kosten je Modell, sortierbar, über mehr als 30 Tage | – | – | F4, E4 |
| B75 | Anwendungsmenü | P | im App-Fenster das Menü des Systems; in der Browser-Ansicht kein eigener Knopf | – | – | G3 |
| B76 | Escape schließt Fenster und Blase | P | Escape schließt das Fenster „Sitzungen“, die Worker-Liste und das Kontextmenü; im Terminal gehört Escape dem Programm | – | – | – |
| B77 | Farbwahl | P | OF3 Punkt 2, Seite Aussehen | – | – | G2 |
| B78 | Admin-Einstellungen | P | OF9 Punkt 4 bis 7; nur für Admins sichtbar | L4 | – | F2, E1, E2 |
| B79 | Umschalter Chat / Terminal | N | Chat-Ansicht ähnlich Open WebUI; Vorgabe am PC Terminal, am Handy Chat, in den Einstellungen änderbar (A169) | – | – | F3 |
| B80 | Leiste ein- und ausblenden | P | Knopf links in der Titelleiste; je Gerät gemerkt | L8 | – | G3 |

Zählung: **Pilot 47, nach dem Pilot 18, entfällt 4, zusammen 69**, davon 13 mit Grundform. Nicht vergeben: B13 und B43 bis B50 (Gesprächsansicht der alten App, A169),
B65 (Startseite) und B74 (Erststart). Neu seit der Fassung vom 23.11.2026: B78, B79 und B80.

## 3. Prüfmittel

| Prüfmittel | Wo | Regeln |
|---|---|---|
| Läufer des Vertrags `seite` | außerhalb des Repos; Fälle `rust/spec/seite/faelle/` | SE1 bis SE7, OF4, OF6, OF11, OF12 |
| Läufer des App-Fensters | noch zu bauen | OF14 |
| `gleichheit.sh` | `rust/ansicht/test/gleichheit.sh` | OF2 (Browser) |
| Bilder | Abschnitt OF13, aus einem echten Lauf oder aus der Attrappe mit Beschriftung | OF1 bis OF9, OF13 |
| Kontrastmessung | je Rollenfarbe und Kombination | OF3 Punkt 5 |
| Abgleich der Kennungen | `data-bw` und Element-IDs gegen Abschnitt 2 | OF7 |
| Prüfung am Auge | Prüfliste `rust/spec/VERTRAEGE.md` Abschnitt 7 und die Bilder | alle, abschließend OF13 |

## 4. Was die Oberfläche vom Kern und vom Dienst braucht

Die Lücken L1 bis L8 sind am 26.11.2026 im Code von `development` gefunden worden (hergeleitet), L9 bis L11 kamen beim Schreiben dieser Fassung dazu.
Die Namen und Felder unten sind ein **Vorschlag** dieses Vertrags. Festgelegt werden sie in den Verträgen `zustandskern` und `dienst`, und zwar vor
dem Pilot (Projektentscheid, 26.11.2026). Bis dahin zeigt der echte Lauf die betroffenen Bedienwege nicht (OF8 Punkt 3).

| Nr | Was fehlt | Vorschlag | Bedienwege |
|---|---|---|---|
| L1 | `Agenten` und `Agentenzustand` nennen die Sitzung eines Workers nicht. Der Kern kennt sie (`zustandskern/src/kern.rs:226-231`). | Jeder Eintrag trägt `sitzung`, `rolle`, `modell`, `zustand` und `nr` (Nummer seines Rasters). | B10, B22, B26, B27, B29, B41 |
| L2 | Worker-Panes: Es gibt nur ein abonniertes Terminal je Betrachter. | `Abo` gilt je Raster (`nr`). Ein Betrachter hält mehrere Abos zugleich, je sichtbarem Pane eines, mit eigener `Groesse`. Die PTY folgt weiter nur dem Betrachter mit der letzten Eingabe (SE2 Punkt 7). | B27, B30 bis B33, B39 |
| L3 | Kein Ereignis für Fortsetzen, Wiederbeleben, Löschen und Namen ändern (`dienst/src/dienst.rs:53-64`). | Neue Ereignisse `Fortsetzen {sitzung}`, `Wiederbeleben {sitzung}`, `Loeschen {sitzung}` und `Umbenennen {sitzung, name}`. Antwort über die nächste `Sitzungsliste` oder eine `Ablehnung` mit festem Grund. | B11, B12, B16, B17, B68 |
| L4 | `EinstellungSetzen` und `RaumWechsel` werden im Lauf mit `nicht_im_lauf` abgelehnt (`dienst.rs:1242-1243`). Es gibt keine Ebene und keine Rolle Admin. | `EinstellungSetzen {schluessel, wert, bereich: "nutzer" oder "admin", ebene}` ist im Lauf erlaubt. Der Dienst prüft Admin über die Liste der Firmenvorgabe für das Konto des Betrachters (E1), Nutzer nur Verschärfung (A26). Eine Nachricht `Einstellungen` trägt je Schlüssel den wirksamen Wert, die Herkunft und ob der Betrachter ihn ändern darf. | B72, B78 |
| L5 | Die `Sitzungsliste` hat keinen Namen, keinen Ordner und keine beendeten Sitzungen früherer Läufe. | Jeder Eintrag trägt `name`, `ordner` (Arbeitsordner) und `projekt` (Anzeigename). Die Liste nennt alle bekannten Sitzungen, auch beendete. | B02, B09, B10, B68 |
| L6 | `Freigabe` ist erlaubt (`dienst.rs:60`). Ob der Weg bis zum Halter trägt, ist nicht geprüft. | Ein Fall Ende zu Ende: Anfrage, Freigeben, Ablehnen mit Begründung, Zeile im Guard-Verlauf. | B07, B56 |
| L7 | Keine Verbrauchsdaten im Strom. | Im Pilot nicht nötig: B21 und B73 kommen nach dem Pilot (F4). | – |
| L8 | Breiten, Zustand des Inspektors und Reihenfolge der Sitzungen. | Kein Protokoll. Gemerkt wird je Gerät: im Browser in `localStorage`, im App-Fenster in einer Datei des Nutzers. Je Nutzer erst nach dem Pilot. | B04, B19, B24, B57 |
| L9 | Kein Weg, den Arbeitsordner einer Sitzung zu lesen oder zu durchsuchen. | `OrdnerLesen {sitzung, pfad}` und `InhaltSuchen {sitzung, muster}` mit den Antworten `Ordnerinhalt` und `Suchtreffer`. Der Dienst lässt nur Pfade unterhalb des Arbeitsordners zu und lehnt sonst mit festem Grund ab. | B52, B53 |
| L10 | `Startwahl` trägt keinen Effort, und die Start-Struktur hat kein Feld dafür. | `Startwahl` trägt je Route und Modell die erlaubten Efforts (Grenzen der Admins) und die Vorgabe des Nutzers. Die Start-Struktur bekommt `effort` und `name`. | B68, B69 |
| L11 | Das App-Fenster braucht einen Weg zum Dienst ohne Brücke (OF14 Punkt 2). | Ein lokaler Weg für Betrachter auf demselben Rechner (Unix-Socket, unter Windows eine Named Pipe) mit denselben Nachrichten und Ereignissen wie der Strom der Brücke. Das Konto des Betrachters liest der Dienst aus dem Verbindungspartner des Sockets, nie aus einer Angabe der Ansicht; daran hängt die Prüfung auf Admin (L4). | alle im App-Fenster |

## 5. Übergang von der Fassung vom 23.11.2026

1. Die Browser-Ansicht von 2a wird nach dieser Fassung umgebaut. Bis dahin beschreibt die Fassung vom 23.11.2026 den gebauten Stand. Sie liegt
   in der Geschichte des Repos.
2. Die Fälle des Vertrags `seite`, die sich mit dieser Fassung ändern, nennt `rust/spec/VERTRAEGE.md` im Nachtrag vom 26.11.2026. Kein Fall wird gelöscht. Fälle
   für Bedienwege mit Umfang N bleiben erhalten und laufen erst wieder, wenn ihr Bedienweg kommt.
3. Die offenen Entscheidungen der Fassung vom 23.11.2026 (Abschnitt 4 a und b) sind entschieden: F1 und G8.
