# Vertrag `regelebenen`: Firma, Gruppe, Nutzer (Fassung 1)

Stand 20.11.2026, Meilenstein M3 (Meilensteinplanung, Abschnitt 5.7).
Nachtrag 22.11.2026 (geschrieben, abgestimmt am 21.11.2026): Befunde B08, B09 und B13. Rollen mit Pseudonym statt `admins`, Personen nur als Pseudonym, `protokoll` ohne `einsicht`, die Zeile in der Form von Vertrag 5.
Grundlage sind A26, A27, A28, A41, A45, A103 sowie A30, A35, A39, A102, A128, A134, A135 und A139 der
Anforderungsliste. „(V)“ heißt: in diesem Vertrag festgelegt. „(H)“ heißt: aus den Anforderungen hergeleitet.

Der Vertrag steht vor dem Code. Alle Fälle sind heute rot, weil es noch keine Umsetzung gibt. Verdeckte Fälle,
Bytefälle der Lader und Mutanten folgen mit dem Bau. Die Lockerungssperre ist eine Sicherheitsgrenze; der Bau
braucht darum einen vollen Mutationslauf ohne überlebende Mutanten in der Zusammenführung und in der Änderungsprüfung.

## 1. Zweck und Schnitt

Regeln stehen auf drei Ebenen: Firma, Gruppe, Nutzer, je eine Datei (A26). Die **Zusammenführung** macht aus den
Dateien eines Nutzers seine **wirksame Regel**. Ihr Kern ist eine Einstellung nach Vertrag 2 Fassung 1, dieselbe
getypte Sicht wie heute. Das Tor (Vertrag 3) bleibt darum unverändert: Der Lader ruft wie bisher `Vorgabe::neu` mit
den Feldern dieser Einstellung auf.

Vertrag 2 Fassung 1 bleibt gültig. Er ist der Fall „eine Ebene“: Eine Firmendatei ohne Gruppen, ohne Nutzerregeln
und ohne Nutzerdatei ergibt genau ihre eingebettete Einstellung (Fall `eine-ebene`).

Zwei Operationen, beide reine Funktionen ohne Uhr, Zufall, Datei oder Netz:

- `zusammenfuehren(stand, nutzer) -> Result<Wirksam, Fehler>`: liest die Firmendatei, die Datei der Gruppe des
  Nutzers und seine Nutzerdatei. Sie nimmt je Feld den strengeren Wert. Sie scheitert nie an einer Lockerung.
- `aendern(stand, wer, datei) -> Pruefung`: prüft eine neue Datei einer Ebene. Sie nimmt sie an oder lehnt sie mit
  genau einem festen Code ab. Eine abgelehnte Änderung ändert nichts. Jede Prüfung ergibt eine Protokollzeile (A27).

Schreiben, Ort der Dateien und Anmeldung gehören nicht zu diesem Vertrag. Das Pseudonym `wer` liefert der Aufrufer
(Konsole, Kern-Dienst) aus der geprüften Anmeldung (Vertrag 9), nie aus einer Datei und nie vom Agenten (V, B09).

## 2. Begriffe

- **Kennung**: Text der Form `^[a-z0-9][a-z0-9._-]{0,63}$`, wie in den Verträgen 1 bis 5.
- **Ebene**: `firma`, `gruppe` oder `nutzer`. Die Reihenfolge ist fest: Firma, Gruppe, Nutzer.
- **Pseudonym**: Text der Form `^ps_[0-9a-f]{32}$` (Vertrag 9). Jede Person steht in diesem Vertrag nur so da (A12):
  in `rollen`, in `mitglieder`, als `nutzer` und als `wer`. Eine Kontokennung steht nie in einer Datei der Ebenen.
- **Admin**: ein Pseudonym mit der Rolle `admin` in `rollen` der gespeicherten Firmendatei (A27). Es gibt mindestens
  einen; `rollen` hat höchstens 1024 Einträge (B09).
- **Nutzerregel der Firma**: ein Regelsatz der Firma für einen einzelnen Nutzer (A26). Sie bleibt eine Regel der
  Firma, auch in der Herkunft (A128).
- **Regelsatz**: die Verschärfungen einer Ebene (Abschnitt 4).
- **Strenger**: weniger Daten an weniger Routen. Abschnitt 5 legt das je Feldart fest.
- **Lockerung**: ein Wert, der nicht strenger und nicht gleich dem Wert der Ebenen darüber ist.

## 3. Dateien der Ebenen

Jede Datei ist ein JSON-Objekt, UTF-8 ohne BOM, höchstens 1 048 576 Byte. Die Form ist geschlossen, auf jeder Ebene.
Ein doppelter Schlüssel in irgendeinem Objekt macht die Datei ungültig (A139). Listen ohne Doppel.

### 3.1 Firmendatei

| Feld | Typ | Grenzen | Bedeutung |
|---|---|---|---|
| `fassung` | Ganzzahl | genau `1` | Fassung dieses Vertrags |
| `ebene` | Text | genau `firma` | |
| `einstellung` | Objekt | Vertrag 2 Fassung 1, unverändert geprüft | Grundregel für alle (Klassen, Routen, Kunden, Freigaben) |
| `rollen` | Objekt: Pseudonym → Liste aus `admin`, `einsicht` | 1 bis 1024 Einträge, je 1 bis 2 Rollen ohne Doppel, mindestens ein `admin` | `admin` ändert Firma- und Gruppenebene (A27), `einsicht` sieht das Protokoll ein (Vertrag 6 Fassung 2); die einzige Quelle der Rollen (B09) |
| `gruppen` | Liste von `{"schluessel", "mitglieder"}` | 0 bis 256; `mitglieder` 0 bis 4096 Pseudonyme | ein Nutzer steht in höchstens einer Gruppe |
| `nutzerregeln` | Liste von `{"nutzer", "regeln"}` | 0 bis 4096, `nutzer` (Pseudonym) ohne Doppel | Regelsatz der Firma für einen Nutzer (A26) |
| `routen_status` | Liste, optional | 0 bis 64, `route` ohne Doppel | A45, siehe unten |
| `senkung` | Objekt, optional | siehe unten | Senkung durch die Automatik (A39, A41) |
| `protokoll` | Objekt, optional | siehe unten | Aufbewahrung (A103) |

- **`routen_status`**: je Eintrag `route` (Route aus `einstellung`), `status` (`frei`, `gesperrt` oder `evaluierung`),
  `grund` (Text, 1 bis 200 Zeichen, ohne Steuerzeichen), `seit` (Datum `JJJJ-MM-TT`), `freigabe_durch` (Kennung einer
  Rolle der Firma nach A45, etwa der freigebenden Stelle; sie nennt nie eine Person, auch nicht als Pseudonym, A12) und optional `zone` (Kennung). Alle außer `zone` sind Pflicht. `gesperrt` und `evaluierung` sperren die Route
  für alle; eine Ausnahme zur Evaluierung (A128) steht im Teilvertrag Zonen (`rust/spec/datentor/zonen/VERTRAG.md`).
  Eine Route ohne Eintrag gilt als `frei`.
- **`senkung`**: `aus` (Wahrheitswert), `bestaetigung` (Wahrheitswert) und `arten` (Liste von Kennungen, 0 bis 64), alle
  Pflicht. Fehlt das Objekt, gilt `{"aus": true, "bestaetigung": false, "arten": []}`: Herabstufen ist in der Vorgabe
  aus (Plan 5.7, M3). Die Arten aus A39 bringt die Vorlage als Daten mit, nicht das Programm (A29).
- **`protokoll`**: `aufbewahrung_tage` (Ganzzahl 0 bis 36500; `0` heißt: nichts löschen), Pflicht. Fehlt das Objekt,
  gilt `{"aufbewahrung_tage": 365}` (A103; Projektentscheid, 20.11.2026, Vertrag 9: Speicherbegrenzung als sichere Vorgabe; der
  Erststart fragt nicht, er nennt diesen Wert). Nur die Firma setzt ihn. Wer das Protokoll einsehen darf, steht nicht
  hier, sondern in `einsicht.regel` im Block `stelle` (Vertrag 6 Fassung 2, Vorgabe `rolle`; Befund B08). Ein Feld
  `einsicht` in `protokoll` ist ein unbekannter Schlüssel.

### 3.2 Gruppendatei

`{"fassung": 1, "ebene": "gruppe", "gruppe": <Kennung>, "regeln": <Regelsatz der Gruppe>}`. Die Gruppe muss in
`gruppen` der Firmendatei stehen. Fehlt die Datei einer Gruppe, hat die Gruppe keine eigenen Regeln.

### 3.3 Nutzerdatei

`{"fassung": 1, "ebene": "nutzer", "nutzer": <Pseudonym>, "regeln": <Regelsatz des Nutzers>}`.

## 4. Regelsatz

| Feld | Typ | Firma je Nutzer | Gruppe | Nutzer |
|---|---|---|---|---|
| `routen` | Liste von `{"route", "klasse"?, "gesperrt"?}`, 0 bis 64, `route` ohne Doppel | ja | ja | ja |
| `kunden` | Liste von `{"kunde", "erlaubt"?, "verboten"?}`, 0 bis 256, `kunde` ohne Doppel; Listen je 0 bis 64 Routen | ja | ja | ja |
| `freigaben_entfernt` | Liste von `{"route", "kunde"}`, 0 bis 1024, ohne Doppel | ja | ja | ja |
| `senkung` | Objekt mit `arten`?, `aus`?, `bestaetigung`? | alle drei | alle drei | nur `aus` und `bestaetigung` (A41) |

Alle Felder sind optional. Ein leerer Regelsatz `{}` ändert nichts. Einen Weg, eine Freigabe hinzuzufügen, gibt es
unterhalb der Firmengrundregel nicht: Ein Feld `freigaben` ist ein unbekannter Schlüssel. Ebenso gibt es unterhalb
der Firma kein Feld für Klassen, Vorgabestufe, Routenstatus, Rollen, Gruppen oder `protokoll`.

## 5. Verschärfen je Feldart

Diese Tabelle ist die Sicherheitsgrenze. „Oben“ heißt: der wirksame Wert aller Ebenen darüber.

| Feldart | Strenger oder gleich (zulässig) | Lockerung (nie) | Wirkung beim Laden |
|---|---|---|---|
| Klasse einer Route | Position in `klassen` kleiner oder gleich oben | Position größer | kleinere Position |
| Route `gesperrt` | `true`; `false`, wenn oben frei | `false`, wenn oben gesperrt | gesperrt, wenn eine Ebene `true` sagt |
| Kunde `erlaubt` | Teilmenge von oben | eine Route, die oben nicht erlaubt ist | genau die Liste der Ebene, geschnitten mit oben |
| Kunde `verboten` | jede Liste bekannter Routen | gibt es nicht | Vereinigung mit oben |
| Freigabe | `freigaben_entfernt` nimmt Paare weg | Hinzufügen (kein Feld) | Differenz |
| Senkung `arten` | Teilmenge von oben (Streichen) | eine Art, die oben fehlt (Hinzufügen, A41) | genau die Liste der Ebene, geschnitten mit oben |
| Senkung `aus` | `true` (Abschalten); `false`, wenn oben `false` | `false`, wenn oben `true` | aus, wenn eine Ebene `true` sagt |
| Senkung `bestaetigung` | `true` (Bestätigung verlangen); `false`, wenn oben `false` | `false`, wenn oben `true` | verlangt, wenn eine Ebene `true` sagt |

Weitere Regeln (V):

- Eine **gesperrte Route** fehlt in der wirksamen Einstellung. Sie fällt auch aus `erlaubt`, `verboten` und den
  Freigaben, damit die Einstellung nach Vertrag 2 gültig bleibt. Das Tor meldet für sie `route-ohne-klasse` (D3) und
  gibt ihr nichts. Angaben unterer Ebenen zu einer oben gesperrten Route wirken nicht; sie sind weder Lockerung noch
  Hinweis. Ausnahme: `gesperrt: false` für eine oben gesperrte Route ist eine Lockerung. In `erlaubt` und `verboten` zählt eine oben gesperrte Route als nicht vorhanden.
- **Gleichstand** ist keine Lockerung und keine Änderung. Die Herkunft bleibt dann bei der höheren Ebene.
- **Firma je Nutzer** verschärft in Fassung 1 nur, wie Gruppe und Nutzer. Sie wird vor der Gruppe angewendet.
- **Prüfbezug einer Änderung**: Eine Gruppendatei wird gegen die Firmengrundregel geprüft (mit `routen_status`, ohne
  Nutzerregeln). Eine Nutzerdatei wird gegen die wirksame Regel aus Firma, Nutzerregel der Firma und Gruppe geprüft,
  nicht gegen ihren eigenen alten Stand. Der Nutzer darf eigene Verschärfungen also zurücknehmen, nie unter die
  Ebenen darüber. Eine Nutzerregel der Firma wird gegen die Firmengrundregel derselben Datei geprüft.

### 5.1 Künftige Blöcke (Annahme, noch nicht in Vertrag 2)

Der Schreibtor entwirft einen Block `schreiben`, die Stelle einen Block `mcp`. Kommen sie in eine neue
Fassung von Vertrag 2, gilt für sie diese Verschärfung (Vorgabe, 20.11.2026). Fälle folgen mit den
Blöcken.

| Feld | Strenger oder gleich | Lockerung |
|---|---|---|
| `schreiben.freigabe_modell` (Vorgabe `true`) | `true` | `false` |
| `schreiben.regeln` (eine Regel hebt nur) | Regeln ergänzen | Regeln entfernen oder ändern |
| `schreiben.vorbelegung_mensch` | nur Firma | jeder Wert unterhalb der Firma (OPEN 7) |
| `mcp` je Server `lesen`, `schreiben` (Listen von Werkzeugnamen, wie `rust/spec/einstellungen`, Abschnitt `mcp`) | Werkzeuge entfernen (Teilmenge der Ebene darüber) | Werkzeuge ergänzen |
| `mcp` je Server `freigabe` (Vorgabe `true`) | `true` | `false` |
| `mcp` je Server `frist_ms`, `einloesefrist_ms` | kürzer oder gleich | länger |

## 6. Zusammenführung

Reihenfolge: Firmengrundregel, `routen_status`, Nutzerregel der Firma für diesen Nutzer, Gruppe, Nutzer. Je Ebene
gilt Abschnitt 5. Die Zusammenführung prüft die Form jeder gelesenen Datei streng (Abschnitt 8, Stufen 3 bis 14).
Ein Formfehler in einer dieser Dateien ergibt einen Fehler und keine Einstellung (fail closed). Grund: Eine still
übergangene Gruppen- oder Nutzerdatei wäre eine Lockerung.

Inhaltlich veraltete Angaben brechen das Laden nicht. Sie entstehen, wenn eine höhere Ebene sich nach der Änderung
einer tieferen geändert hat. Die Zusammenführung behält dann den strengeren Wert und meldet einen Hinweis:

| Hinweis | Wann | Wirkung |
|---|---|---|
| `ebenen.lockerung_ignoriert` | ein Wert ist lockerer als oben | der Wert von oben gilt; ein Element in `erlaubt` oder `arten` fällt weg |
| `ebenen.route_unbekannt_ignoriert` | eine Route steht nicht in der Firmengrundregel | die Angabe wirkt nicht |
| `ebenen.kunde_unbekannt_ignoriert` | ein Kunde steht nicht in der Firmengrundregel | die Angabe wirkt nicht |
| `ebenen.klasse_unbekannt_gesperrt` | die Klasse einer Route steht nicht in `klassen` | die Route ist für diesen Nutzer gesperrt (strengster Wert) |

Bleibt keine Route übrig, endet die Zusammenführung mit `ebenen.keine_route` und ohne Einstellung. Der Aufrufer
startet dann nichts; das entspricht „alles gesperrt“.

### 6.1 Ergebnis `Wirksam`

```json
{"einstellung": {"fassung": 1, "klassen": ["…"], "vorgabestufe": "…", "routen": ["…"], "kunden": ["…"],
                 "kundendaten_an_cloud": {"vorgabe": "nie", "freigaben": ["…"]}},
 "senkung": {"aus": true, "bestaetigung": false, "arten": []},
 "protokoll": {"aufbewahrung_tage": 365},
 "routen_status": [],
 "herkunft": [{"pfad": ["routen", "cloud-r", "klasse"], "wert": "intern", "ebene": "firma"}],
 "hinweise": [{"code": "ebenen.lockerung_ignoriert", "ebene": "nutzer", "pfad": ["routen", "cloud-r", "klasse"]}]}
```

- `einstellung` ist gültig nach Vertrag 2 Fassung 1. `vorgabestufe` steht immer da: der Wert der Firma, sonst die
  höchste Klasse. Routen und Kunden stehen in der Reihenfolge der Firmendatei. Jeder Kunde trägt `erlaubt` und
  `verboten`. `erlaubt`, `verboten`, `freigaben` und `senkung.arten` sind byte-weise aufsteigend sortiert.
- `routen_status` gibt die Liste der Firma unverändert wieder (Anzeige der fünf Felder, A45).
- `herkunft` (A28) hat je Feld, das eine untere Ebene ändern kann, eine Zeile: je verbliebener Route `klasse`, je
  gesperrter Route `gesperrt` mit Wert `true`, je Kunde `erlaubt` und `verboten`, dazu
  `["kundendaten_an_cloud", "freigaben"]`, `["senkung", "aus"]`, `["senkung", "bestaetigung"]` und
  `["senkung", "arten"]`. `ebene` ist die tiefste Ebene, die den Wert echt geändert hat, sonst `firma`. Eine Liste
  gilt als geändert, wenn sie nach der Ebene anders ist als davor, auch durch das Entfernen einer gesperrten Route.
  Alle übrigen Felder kommen von der Firma.
- Die Reihenfolge von `herkunft` und `hinweise` ist frei; der Läufer vergleicht sie als Mengen.

## 7. Änderung

`aendern(stand, wer, datei)` bestimmt Ebene und Ziel aus `datei` (`ebene`; `gruppe` oder `nutzer`). Befugt sind:

| Ebene | befugt |
|---|---|
| `firma` | ein Admin der **gespeicherten** Firmendatei. Eine eingereichte Datei, die `wer` als Admin nennt, macht ihn nicht befugt. |
| `gruppe` | ein Admin |
| `nutzer` | der Nutzer selbst (`wer` gleich `nutzer`) oder ein Admin; beide nur verschärfend |

Eine angenommene Datei ersetzt die gespeicherte Datei derselben Ebene und desselben Ziels. Eine abgelehnte ändert
nichts; eine folgende Zusammenführung liefert denselben Stand wie davor (A26, A27).

### 7.1 Protokollzeile

Jede Prüfung ergibt eine Zeile, angenommen wie abgelehnt (A27, A135). Ihre Form steht in Vertrag 5, Abschnitt 6.6
(`regel.geaendert` und `regel.abgelehnt`, Befund B13); Kette, `fassung`, `zeit` und `vorher` setzt die Bibliothek
aus Vertrag 5. Dieser Vertrag füllt die Felder so:

| Feld | Wert |
|---|---|
| `aktionsname`, `ergebnis` | `regel.geaendert` mit `geaendert` bei Annahme, sonst `regel.abgelehnt` mit `abgelehnt` |
| `ebene` | `firma`, `gruppe`, `nutzer`, oder `null`, wenn `datei.ebene` keiner dieser Texte ist |
| `ziel` | Schlüssel der Gruppe oder Pseudonym des Nutzers; `null` bei der Firma oder wenn das Ziel fehlt oder falsch ist |
| `handelnd` | `wer`, das Pseudonym aus der geprüften Anmeldung (früher hieß das Feld hier `wer`) |
| `alt` | sha256 hex klein über RFC 8785 der gespeicherten Datei derselben Ebene und desselben Ziels, sonst `null` |
| `neu` | sha256 hex klein über RFC 8785 der eingereichten Datei |
| `code` | `null` bei Annahme, sonst der Code aus Abschnitt 8 |
| `antrag`, `pflicht`, `zweiter` | immer `null`: Fassung 1 kennt keine zweite Person und keine einzelne Pflicht |

Alter und neuer Wert (A27) stehen damit als Prüfsumme in der Zeile. Die Werte selbst liegen als Fassungen der Datei
beim Speicher der Konsole (OPEN 4).

## 8. Fehlercodes und Prüfreihenfolge

Genau ein Code, der der ersten verletzten Stufe. Innerhalb einer Stufe ist gleich, welche Stelle verletzt ist.

| Stufe | Code | Verletzung | Laden | Änderung |
|---|---|---|---|---|
| 1 | `ebenen.zu_gross` | Datei über 1 048 576 Byte | ja | ja |
| 2 | `ebenen.json_ungueltig` | kein UTF-8, BOM, Syntaxfehler, doppelter Schlüssel, kein Objekt, Text danach | ja | ja |
| 3 | `ebenen.schluessel_unbekannt` | ein Schlüssel außerhalb der Form der Ebene (außer in `einstellung`) | ja | ja |
| 4 | `ebenen.schluessel_fehlt` | ein Pflichtfeld fehlt | ja | ja |
| 5 | `ebenen.typ_falsch` | falscher JSON-Typ | ja | ja |
| 6 | `ebenen.fassung_unbekannt` | `fassung` ist eine Ganzzahl, aber nicht `1` | ja | ja |
| 7 | `ebenen.ebene_falsch` | `ebene` passt nicht zum Platz der Datei, oder `gruppe`/`nutzer` nicht zum Ziel | ja | nein |
| 8 | `ebenen.anzahl` | eine Liste außerhalb ihrer Grenzen, etwa kein Admin | ja | ja |
| 9 | `ebenen.kennung_form` | eine Kennung oder ein Pseudonym verletzt die Form | ja | ja |
| 10 | `ebenen.kennung_doppelt` | ein Doppel in einer Liste (auch in den Rollen eines Pseudonyms) oder ein Nutzer zweimal in `nutzerregeln` | ja | ja |
| 11 | `ebenen.wert_ungueltig` | `ebene`, `status` oder eine Rolle außerhalb ihrer Werte, `seit` kein Datum, `grund` außerhalb der Form, `aufbewahrung_tage` außerhalb der Grenzen | ja | ja |
| 12 | `einst.*` | `einstellung` verletzt Vertrag 2 (dessen Stufen 3 bis 17, dessen Code unverändert) | ja | ja |
| 13 | `ebenen.route_unbekannt` | `routen_status` nennt eine Route, die nicht in `einstellung` steht | ja | ja |
| 14 | `ebenen.nutzer_mehrfach` | ein Nutzer steht in zwei Gruppen | ja | ja |
| 15 | `ebenen.nicht_befugt` | Abschnitt 7 | nein | ja |
| 16 | `ebenen.gruppe_unbekannt` | die Gruppe steht nicht in `gruppen` der Firma | nein | ja |
| 17 | `ebenen.route_unbekannt` | ein Regelsatz nennt eine Route, die nicht in der Firmengrundregel steht | Hinweis | ja |
| 18 | `ebenen.kunde_unbekannt` | ein Regelsatz nennt einen Kunden, der nicht in der Firmengrundregel steht | Hinweis | ja |
| 19 | `ebenen.klasse_unbekannt` | ein Regelsatz nennt eine Klasse, die nicht in `klassen` steht | Hinweis, Route gesperrt | ja |
| 20 | `ebenen.lockerung` | ein Wert ist eine Lockerung nach Abschnitt 5 | Hinweis | ja |
| 21 | `ebenen.keine_route` | nach der Zusammenführung bleibt keine Route | ja | nein |

Beim Laden werden die Dateien in der Reihenfolge Firma, Gruppe, Nutzer geprüft; es gilt der erste Fehler. Bei einer
Änderung der Nutzerebene gilt ein Fehler der Ebenen darüber als Code der Änderung. Die Stufen 17 bis 20 prüft die
Änderung über den ganzen Regelsatz: erst alle Bezüge, dann die Lockerung.

Ein künftiges Programm `wb-regeln` (Konsole und Kommandozeile) endet bei einer Ablehnung mit Exitcode 3 und dem Code
auf stderr, bei einem Lesefehler mit 2, sonst mit 0 (V; A26: „Ablehnung mit Rückgabecode ungleich 0“).

## 9. Sicherheitszusagen als prüfbare Eigenschaften

- **S1 Keine Lockerung nach unten:** Für jeden Stand und jeden Nutzer ist jede Route der wirksamen Einstellung auch
  in der Firmengrundregel, mit gleicher oder niedrigerer Klasse; jede wirksame Freigabe und jedes erlaubte Paar steht
  auch in der Firmengrundregel; `verboten` ist eine Obermenge (ohne gesperrte Routen). Prüfbar mit erzeugten Ständen.
- **S2 Monotonie:** Eine Verschärfung in irgendeiner Datei macht die wirksame Regel jedes Nutzers nie lockerer.
- **S3 Ablehnung ändert nichts:** Nach einer abgelehnten Änderung ist der Stand Byte für Byte gleich.
- **S4 Befugnis aus dem gespeicherten Stand:** Ob `wer` Admin ist, entscheidet nur die gespeicherte Firmendatei.
- **S5 Fail closed:** Eine ungültige Datei, die der Nutzer laden müsste, ergibt keine Einstellung, nie eine
  Einstellung ohne diese Datei.
- **S6 Tor unverändert:** Jede wirksame Einstellung besteht Vertrag 2 Fassung 1; das Tor bekommt sie über
  `Vorgabe::neu` mit unveränderter Signatur.
- **S7 Rein:** Gleicher Stand und gleiche Eingabe ergeben dieselbe Antwort; keine Eingabe löst eine Panik aus.

## 10. Außentestprogramm `regelebenen`

`regelebenen < fall.json`: genau ein Fall auf stdin, höchstens 4 194 304 Byte, Verschachtelung höchstens 64 Ebenen.

```json
{"name": "…", "anforderungen": ["A26"], "beschreibung": "…",
 "stand": {"firma": {…}, "gruppen": {"g-eins": {…}}, "nutzer": {"ps_00000000000000000000000000000c01": {…}}},
 "schritte": [{"zeige": "ps_00000000000000000000000000000c01"},
              {"aendern": {"wer": "ps_00000000000000000000000000000c01", "datei": {…}}}]}
```

- `stand.gruppen` und `stand.nutzer` bilden den Platz der Datei (später der Pfad) auf ihren Inhalt ab. Die Dateien
  sind JSON-Werte; die Stufen 1 und 2 prüfen erst die Bytefälle des Laders.
- Schritte laufen nacheinander auf einem gemeinsamen Stand. Eine angenommene Änderung schreibt ihre Datei auf den
  Platz `firma`, `gruppen.<gruppe>` oder `nutzer.<nutzer>`.
- Exitcode 0: stdout ist `{"schritte": [...]}`, ein Ergebnis je Schritt:
  `{"wirksam": {…}}` oder `{"fehler": "<code>"}` für `zeige`;
  `{"angenommen": true|false, "code": null|"<code>", "zeile": {…}}` für `aendern`.
- Exitcode 2: Der Fall ist ungültig (kein JSON, doppelter Schlüssel irgendwo, unbekanntes Feld im Fall oder im
  Schritt, `wer` oder `zeige` kein Pseudonym, ein Schritt mit mehr oder weniger als einem der Felder `zeige` und
  `aendern`). stdout bleibt leer, auf stderr steht genau eine Zeile.

## 11. Dateien und Prüfung

| Pfad | Zweck |
|---|---|
| `README.md` | dieser Vertrag |
| `regelebenen.schema.json` | Form der drei Dateien (JSON Schema 2020-12); `einstellung` prüft das Schema von Vertrag 2 |
| `bau-faelle.py` | schreibt `faelle/` und `erwartet/`; rechnet jede Erwartung mit einer kleinen Referenz und prüft sie gegen die ausgeschriebenen Kernwerte der Tabelle |
| `pruefen.py` | prüft Paarung, Codes, Schema der Dateien und jede erwartete `einstellung` gegen das Schema von Vertrag 2 |
| `faelle/<name>.json`, `erwartet/<name>.json` | sichtbare Fälle |

```sh
python3 -I rust/spec/regelebenen/bau-faelle.py               # Fälle neu erzeugen und Erwartungen prüfen
<python mit jsonschema> -I rust/spec/regelebenen/pruefen.py   # Exit 0 bei 0 Fehlern; ohne jsonschema Exit 2 (bewusst ohne Schema: --ohne-schema)
```

Erst mit dem Bau prüfbar: die Querverweise und Lockerungen (Stufen 13 bis 21), die Reihenfolge der Stufen, `1.0`
gegen `1`, die Bytestufen 1 und 2, S1, S2 und S7 mit erzeugten Ständen. Windows-eigene Fälle hat dieser Vertrag nicht;
der Ort der Dateien je Betriebssystem (OPEN 1) wird mit seinem Lader zum Außentest für die Windows-VM.

## 12. Entschieden (mit Grund)

- **Laden nimmt den strengeren Wert, Ändern lehnt ab.** Eine höhere Ebene kann sich nach einer tieferen ändern.
  Würde das Laden dann scheitern, sperrte eine Firmenänderung Nutzer aus; würde es die tiefere Datei verwerfen,
  fiele deren Verschärfung weg. Der strengere Wert ist in beiden Fällen sicher.
- **Firma je Nutzer verschärft nur** (Fassung 1; Projektentscheid, 20.11.2026, vorher OPEN 2). So bleibt die
  Zusammenführung monoton, und ein Fehler in der Konsole öffnet einem einzelnen Nutzer keine Route unbemerkt.
  A26 ist in Fassung 1 nur zum Teil gedeckt: abweichende Regeln je Nutzer ja, aber nur strenger. Lockern je Nutzer
  kommt mit den Ausnahmen zur Evaluierung (A128), als befristete, protokollierte Ausnahme und nicht als stille Regel.
- **Der Nutzer prüft gegen die Ebenen darüber**, nicht gegen seinen alten Stand. Die Grenze der Admins bleibt fest;
  der Nutzer kann eigene Vorsicht zurücknehmen.
- **Eine gesperrte Route fällt aus der Einstellung.** Vertrag 2 hat kein Sperrfeld; das Fehlen sperrt im Tor (D3).
- **Rollen stehen nur in der Firmendatei (B09).** Sonst könnte ein Aufrufer sich mit einem Merkmal selbst zum Admin machen.
  Die Stelle liest dieselben `rollen` (Vertrag 6 Fassung 2); eine zweite Liste gibt es nicht.
- **Aufbewahrung 365 Tage, wenn `protokoll` fehlt** (Projektentscheid, 20.11.2026, Vertrag 9). Eine Speicherbegrenzung ist die
  sichere Vorgabe, und A103 verlangt eine Vorgabe ohne Frage beim Erststart. `0` bleibt erlaubt und heißt weiter:
  nichts löschen. Der Admin ändert den Wert; Gruppe und Nutzer setzen ihn nicht.

## 13. Abstimmung mit anderen Bauteilen

| Bauteil | liest oder liefert | Annahme |
|---|---|---|
| Tor, Vertrag 3 | liest `wirksam.einstellung` über den Lader und `Vorgabe::neu` | Signatur und Vertrag 2 Fassung 1 bleiben; D3 sperrt eine fehlende Route |
| Vertrag 5 (Stelle) | liefert Kette und Zeilenart; liest `protokoll` und `rollen` | Form der Zeile in Vertrag 5, Abschnitt 6.6 (B13); Vorgabe `365`; `einsicht.regel` mit Vorgabe `rolle` liegt bei der Stelle (B08) |
| Konsole | liefert `wer` als Pseudonym aus der geprüften Anmeldung (Vertrag 9) und schreibt angenommene Dateien; zeigt `herkunft`, `routen_status`, `hinweise` | speichert Fassungen der Dateien für alten und neuen Wert |
| Vault (Schreibtor, Suche) | Block `schreiben` folgt; liest `senkung` beim Herabstufen | Verschärfung nach Abschnitt 5.1 |
| Vertrag 2 | Lader der Einstellung | eine neue Fassung von Vertrag 2 nimmt die Blöcke `schreiben` und `mcp` auf |

## 14. OPEN

1. **Ort der Dateien je Betriebssystem.** Firma und Gruppen im Admin-Pfad, Nutzer im Profil (A73). Pfade unter
   Windows, macOS und Linux stehen noch nicht fest; ebenso, wer den Pfad bestimmt (eigene Konfiguration des Laders).
2. **Entschieden (Projektentscheid, 20.11.2026):** Die Firma lockert für einen Nutzer in Fassung 1 nicht; siehe Abschnitt 12.
3. **Mehrere Gruppen je Nutzer.** Fassung 1 erlaubt eine. Die Zusammenführung könnte mehrere streng schneiden; dann ist
   die Herkunft mehrdeutig.
4. **Speicher der Fassungen** für alten und neuen Wert (A27): Konsole oder Stelle.
5. **Gruppenadmins** gibt es in Fassung 1 nicht.
6. **Optionen der Admins (A30)** als allgemeiner Block mit Vorgabewert und aktuellem Wert folgen mit der Konsole.
7. **`schreiben.vorbelegung_mensch`**: Welcher der Werte `auto` und `keine` strenger ist, ist offen; bis dahin nur Firma.
8. **Zonen (A127, A128)** und die Ausnahme zur Evaluierung stehen im Teilvertrag `rust/spec/datentor/zonen/VERTRAG.md`; die
   Vorgabe `vorgabestufe` steht in Vertrag 2 (`rust/spec/einstellungen/README.md`, Feldtabelle). Die Aufnahme in eine
   Fassung von Vertrag 2 folgt; `evaluierung` sperrt bis dahin.
9. **Gesperrte Zweige je Produkt** (Projektentscheid, 21.11.2026). Das Push-Tor lehnt einen Push in einen Basis-Zweig ab
   und ist Pflicht ab Schritt II (A155, A160). Die Prüfung im Produkt kommt ergänzend nach Schritt II. Ein Admin wählt
   in der Konsole je Produkt Zweige, in die nur mit der Freigabe eines Menschen geschrieben werden darf (A160). Ohne
   Eintrag ist der Hauptzweig gesperrt. Ob Muster erlaubt sind, wird beim Entwurf entschieden. Verschärfung wie in
   Abschnitt 5: Tiefere Ebenen dürfen Zweige ergänzen, nie entfernen. Die Prüfung liegt im Endprodukt auf dem Server,
   nicht allein auf dem Rechner des Nutzers. Kommt als Feld in eine neue Fassung von Vertrag 2, mit Fällen.
