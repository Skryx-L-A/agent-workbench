# Vertrag `regelebenen`: Firma, Gruppe, Nutzer (Fassung 1, Fassung 2 als Nachtrag 26.11.2026)

Stand 20.11.2026, Meilenstein M3 (Meilensteinplanung, Abschnitt 5.7).
Nachtrag 22.11.2026 (geschrieben, abgestimmt am 21.11.2026): Befunde B08, B09 und B13. Rollen mit Pseudonym statt `admins`, Personen nur als Pseudonym, `protokoll` ohne `einsicht`, die Zeile in der Form von Vertrag 5.
Nachtrag 26.11.2026 (Fassung 2): Einstellungen im Lauf mit `setzen` und `sicht`, ein Katalog des Pilots und die Admins aus der
Kontenliste der Firmenvorgabe (Abschnitt 15, Abschnitt 5.2). Grundlage sind die Entscheide des Betreibers vom 26.11.2026 (E1, E2, E3,
A26, A27, A30, A31; `rust/spec/seite/OBERFLAECHE.md` OF9 und Abschnitt 4, L4 und L10). Fassung 1 bleibt unverändert gültig.
Nachtrag 27.11.2026 (Fassung 2, Entscheid des Betreibers, 27.11.2026): sechs Optionen nach A30 im Katalog (Abschnitt 15.1 und
15.1.1), die Feldart `fest` und der Grund `fest` (Abschnitte 5.2 und 15.5), eine Quelle der Admins (Abschnitt 15.2), „Stufe“ ist die
Datenklasse (OPEN 15). OPEN 10, 14 und 15 sind geschlossen.
Nachtrag 27.11.2026 (Fassung 2, Projektentscheid, 27.11.2026): das Admin-Feld `leseklassen` für die Leseklasse je Person oder
Gruppe (Abschnitte 15.1, 15.2 und 15.11). Die Wirkung legt `rust/spec/datentor/person/VERTRAG.md` fest.
Nachtrag 27.11.2026 (Fassung 2, Entscheid des Betreibers, 27.11.2026): der Schreibweg der Firmenvorgabe nur über den Dienst
(Abschnitt 15.12) und das Konto in der Ansicht der Admins (Abschnitt 15.13). OPEN 22 und 23 sind geschlossen.
Nachtrag 27.11.2026 (Fassung 2, Entscheid des Betreibers, 27.11.2026): der Schreibhelfer mit eigenem Systemnutzer schreibt die
Firmenvorgabe, der Dienst reicht nur weiter (Abschnitt 15.12, S13). OPEN 25 ist geschlossen; neu sind OPEN 26 bis 29.
Nachtrag 27.11.2026 (Fassung 2, Entscheid des Betreibers, 27.11.2026): der Ordner des Helfers gehört dem Helfernutzer, und `postinst`
trägt das installierende Konto als ersten Admin ein, nur wenn noch keine Firmenvorgabe da ist (Abschnitt 15.12.4). OPEN 28 ist
geschlossen; neu ist OPEN 30.
Nachtrag 27.11.2026 (Fassung 2, Entscheid des Betreibers, 27.11.2026): ohne Firmenvorgabe lädt der Lader keine Admins, und `setzen`
lehnt jede Änderung eines Admins ab; die Regeln der Firma gelten weiter (Abschnitt 15.12.4). OPEN 30 ist geschlossen.
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

### 5.2 Neue Feldarten der Einstellungen im Lauf (Fassung 2, Nachtrag 26.11.2026)

Diese Feldarten gelten für die Schlüssel des Katalogs (Abschnitt 15.1). Die Tabelle oben bleibt die Sicherheitsgrenze; das
Lockerungsverbot gilt auch hier. „Oben“ heißt wieder: der wirksame Wert aller Ebenen über dem Platz der Änderung.

| Feldart (`richtung`) | Strenger oder gleich (zulässig) | Lockerung (nie) | Wirkung |
|---|---|---|---|
| Wahl `frei` (Vorlieben: Aussehen, Schrift, Vorgaben für neue Sitzungen) | jeder erlaubte Wert | gibt es nicht; eine Wahl ist keine Regel | die tiefste Ebene mit Wert gilt |
| Liste je Klasse `schnitt` (`harnesses`, `modelle`) | je Klasse eine Teilmenge von oben; eine Klasse, die oben fehlt, mit jeder Liste | ein Element, das oben in derselben Klasse fehlt | je Klasse der Schnitt aller Ebenen mit dieser Klasse; fehlt eine Klasse auf allen Ebenen, ist sie nicht eingeschränkt |
| Schalter `aus_strenger` (Optionen nach A30) | `false`; `true`, wenn oben `true` | `true`, wenn oben `false` | `true` nur, wenn keine Ebene `false` sagt; Vorgabe `false` |
| Grenzen `nur_firma` (`effort.grenzen`, `optionen.lokale_ca`, `optionen.copilot_eingeschraenktes_token`) | nur die Firma setzt sie | jeder Wert unterhalb der Firma (Code `ebenen.ebene_gesperrt`) | der Wert der Firma |
| Fest `fest` (`optionen.herstellerzugang`; Nachtrag 27.11.2026) | nichts: keine Ebene setzt den Schlüssel, auch kein Admin | jeder Wert auf jeder Ebene (Code `ebenen.ebene_gesperrt`) | immer die Vorgabe; ein Wert im Stand wirkt nicht |

- Ein Wert `null` in `setzen` nimmt den Eintrag der Ebene zurück. Das ist nie eine Lockerung: Es gilt danach der Wert von oben.
- Gleichstand ist keine Lockerung. Die Herkunft bleibt dann bei der höheren Ebene (wie Abschnitt 5).
- Eine Wahl `frei` kann trotzdem an eine Grenze der Admins gebunden sein. `sitzung.harness` und `sitzung.modell` müssen in mindestens
  einer Klasse zur Wahl stehen (Code `ebenen.ausser_grenze`). Das ist keine Lockerung, sondern ein Bezug, wie die Stufen 17 bis 19.
- Grund der Feldart `nur_firma`: Die Grenzen des Efforts werten in Vertrag 2 jeden passenden Eintrag als Schnitt. Ein Eintrag
  einer Gruppe für einen Harness ohne Eintrag der Firma würde dort eine Wahl öffnen, die die Firma nie erlaubt hat. Grenzen je Gruppe
  kommen nach dem Pilot (OPEN 12).

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
| `faelle-setzen/<name>.json`, `erwartet-setzen/<name>.json` | Fälle zu `setzen` und `sicht` (Fassung 2, Abschnitt 15.8); die Referenz steht in `bau-faelle.py` |
| `faelle-schreiben/<name>.json`, `erwartet-schreiben/<name>.json` | Fälle zum Schreibweg der Firmenvorgabe und zum Konto in der Ansicht der Admins (Abschnitte 15.12 und 15.13); Pflichten des Dienstes und des Schreibhelfers, die Referenz steht in `bau-faelle.py` |

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
10. **Geschlossen (Entscheid des Betreibers, 27.11.2026): eine Quelle der Admins.** Der Dienst leitet `rollen` aus der Kontenliste
    der Firmenvorgabe ab (E1). Eine zweite Quelle gibt es nicht (Abschnitt 15.2, Fall `admins-eine-quelle`).
11. **Konto ohne Kennungsform.** Ein Kontoname darf mit `_` beginnen (`[a-z_][a-z0-9_-]*`, `rust/spec/anmelden/README.md:44`). Die
    Kennung einer Person in Vertrag 9 Personenbezug darf das nicht (`rust/spec/personenbezug/README.md:47`). Empfehlung: Der Dienst
    nimmt ein Konto mit `_` am Anfang nicht in die Admins auf und meldet es beim Laden der Firmenvorgabe; die IT benennt es um.
12. **Grenzen des Efforts je Gruppe** gibt es im Pilot nicht (Abschnitt 5.2, `nur_firma`). Ein Weg dafür braucht eine Form, in der
    ein Eintrag der Gruppe nur schneidet. Empfehlung: nach dem Pilot als Liste je Gruppe, die nur auf Einträge der Firma wirkt.
13. **Harnesses und Modelle je Klasse im Start.** `harnesses` und `modelle` stehen im Katalog. Dass `Startwahl` und die Prüfung des
    Starts sie anwenden, verlangt einen Block in Vertrag 2 wie `effort` (`rust/spec/einstellungen/README.md`, Abschnitt „Block
    `effort`“). Empfehlung: Block `zulassung` in Vertrag 2 mit demselben Lader; bis dahin wirken die beiden Schlüssel
    nur auf die Vorgaben für neue Sitzungen (`ebenen.ausser_grenze`).
14. **Geschlossen (Entscheid des Betreibers, 27.11.2026): Optionen nach A30 im Pilot.** Es sind `copilot_ohne_rueckfrage`,
    `herabstufen`, `lokale_ca`, `copilot_telemetrie`, `copilot_eingeschraenktes_token` und `herstellerzugang` (Abschnitt 15.1.1).
    Weitere Optionen kommen nur auf Entscheid des Betreibers dazu. Die Abbildung von `copilot_ohne_rueckfrage` auf
    `harness.copilot.werkzeuge_ohne_rueckfrage` bleibt hergeleitet: Bei „aus“ schreibt der Lader die leere Liste.
15. **Geschlossen (Entscheid des Betreibers, 27.11.2026): „Stufe“ ist die Datenklasse,** also die Klasse der Route (`klassen` von
    Vertrag 2). Grundlage ist der Entscheid des Betreibers vom 17.11.2026: Modelle je Stufe und Zone sind eine Betreibereinstellung.
    `harnesses` und `modelle` sind darum Listen je Klasse.
16. **Werte im Speicher.** `alt` und `neu` stehen als Prüfsumme in der Zeile (Abschnitt 15.6). Wo die Werte selbst liegen, ist OPEN 4.
17. **`optionen.herabstufen` und Vertrag 2.** Vertrag 2 hat kein Feld dafür; das Gegenstück ist `herabstufen.an` im Teilvertrag
    Klassen. Feld folgt mit dem Bau. Empfehlung: Der Lader schreibt den wirksamen Wert der Firma nach `herabstufen.an`. Die Liste der
    Arten bleibt im Teilvertrag Klassen.
18. **`optionen.lokale_ca` und `ca_zulassung`.** Vertrag 2 hat kein Feld dafür. `ca_zulassung` ändert sich nach dem Paketvertrag nur
    durch einen Schritt eines Admins mit root. Feld folgt mit dem Bau. Empfehlung: Eine Cloud-Sitzung startet nur, wenn die Option an
    ist und `ca_zulassung` `zugelassen` sagt. Die Option allein lässt die CA nie zu. `abgelehnt` bleibt eine Sache des Pakets.
19. **`optionen.copilot_telemetrie` und das Erststart-Profil.** Die Abbildung auf `routen[].netzziele` ist hergeleitet. Das
    Erststart-Profil (`rust/spec/einstellungen/erststart.json`) trägt das Telemetrie-Ziel heute ohne Schlüssel mit nur
    `POST /telemetry`. Empfehlung: Bei „aus“ lässt der Lader den Eintrag weg. Gemessen ist, dass die CLI ohne erreichbares
    Telemetrie-Ziel weiterläuft (NXDOMAIN, `rust/spec/einstellungen/README.md`, Abschnitt Vorführdatei).
20. **`optionen.copilot_eingeschraenktes_token`.** Vertrag 2 und Vertrag 9 Anmeldung haben kein Feld für ein fein begrenztes Token.
    Feld folgt mit dem Bau. Empfehlung: Vertrag 9 Anmeldung legt den Weg fest; der Lader wählt ihn nur bei „an“.
21. **`optionen.herstellerzugang` und der Block `stelle`.** Vertrag 2 hat den Block `stelle` noch nicht; er ist für Fassung 3
    vorgeschlagen. Feld folgt mit dem Bau. Empfehlung: Der Lader schreibt im Pilot immer `hersteller.fall: kein`, unabhängig vom
    Stand der Ebenen.
22. **Geschlossen (Entscheid des Betreibers, 27.11.2026): Schreibweg der Firmenvorgabe.** Die Firmenvorgabe bleibt für Nutzer nicht
    schreibbar. Eine Änderung eines Admins aus der Ansicht reicht der Dienst an den Schreibhelfer mit eigenem Systemnutzer weiter. Der
    Helfer prüft den Partner selbst über `SO_PEERCRED` und ersetzt die Datei atomar und fail closed (Abschnitt 15.12, Fälle
    `schreiben-*` und `socket-*`; Entscheid des Betreibers, 27.11.2026).
23. **Geschlossen (Entscheid des Betreibers, 27.11.2026): Konto in der Ansicht.** Admins sehen das Konto, ins Protokoll kommt nur das
    Pseudonym. `sicht` bleibt bei Pseudonymen; der Dienst setzt für einen Admin das Konto ein (Abschnitt 15.13, Fälle `konto-*`).
    Der Teil zum Laden ist in OPEN 25 entschieden.
24. **`leseklassen` in Vertrag 2.** Vertrag 2 hat das Feld noch nicht; der Vorschlag steht in `rust/spec/einstellungen/README.md`,
    OPEN 13. Bis zum Bau gibt der Dienst die Liste direkt an `Personen::neu`.
25. **Geschlossen (Entscheid des Betreibers, 27.11.2026): Konto ohne Pseudonym beim Laden.** Kann der Dienst ein Konto in
    `leseklassen` der Firmenvorgabe nicht zuordnen (etwa ein Konto mit `_` am Anfang, OPEN 11), gibt er die Liste nicht weiter. Das ist
    ein Ladefehler der Firmenvorgabe, und es entsteht kein Tor (fail closed, wie datentor Abschnitt 3). Für eine Änderung aus der
    Ansicht gilt Abschnitt 15.12, Stufe 0.
26. **Schreibhelfer unter macOS und Windows.** Teil A3 des Paketvertrags legt den Helfer nur für Linux fest. Unter macOS fehlt ein
    Systemnutzer und eine `launchd`-Einheit mit Socket, unter Windows ein Dienst mit Named Pipe und Prüfung des Tokens. Bis dahin gibt
    es dort keinen Schreibweg aus der Ansicht; der Dienst meldet `firmenvorgabe.helfer_nicht_erreichbar`. Empfehlung: dieselben Stufen
    H1 bis H12, die Kennung aus `getpeereid` und `TokenUser` wie DI-L11 Punkt 4.
27. **Eigentümer nach einer Änderung von Hand.** Ändert die IT die Datei mit `sudo`, gehört sie danach oft root. Der Helfer lehnt dann
    mit `firmenvorgabe.datei_unsicher` ab, bis jemand den Eigentümer zurücksetzt (Fall `schreiben-datei-root-eigen`). Ob der Lader des
    Dienstes eine root-eigene Datei annimmt, legt dieser Vertrag nicht fest. Empfehlung: Der Lader nimmt Eigentümer root oder
    `werkbank-vorgabe` an; das Paket liefert eine Zeile für die Anleitung (`sudo install -o werkbank-vorgabe -g werkbank-vorgabe -m 0644`).
28. **Geschlossen (Entscheid des Betreibers, 27.11.2026): Ordner des Helfers.** `/etc/werkbank/vorgabe/` gehört `werkbank-vorgabe`
    mit Modus `0755`. Alle Eltern ab `/etc/werkbank` gehören root und haben nicht `& 022`. Der Ordner hat kein Schreibrecht für eine
    Gruppe, und der Helfer hat kein `CAP_DAC_OVERRIDE`. Grund: In einem Ordner mit Modus `0555` scheitern `open(O_CREAT|O_EXCL)` und
    `rename` mit errno 13, auch für den Eigentümer der Datei (gemessen, Abschnitt 15.12.4). Ein Helfer ohne root könnte in einem
    root-eigenen Ordner also nicht atomar ersetzen.
29. **Optionen der Firma und `/etc/werkbank/vorgabe.json`.** Die beiden Dateien sind getrennt (Abschnitt 15.12.4). Die Optionen
    der Ebene `firma` (Abschnitt 15.1.1) bildet der Lader auf Felder von Vertrag 2 ab, etwa `routen[].netzziele`. `lauf` prüft aber
    nur gegen `vorgabe.json`, die `postinst` aus dem Erststart-Profil schreibt. Empfehlung: Der Lader wendet die Optionen auf die
    Arbeitskopie an und schreibt nie in `vorgabe.json`; eine Option kann so nur verschärfen, was `vorgabe.json` erlaubt.
30. **Geschlossen (Entscheid des Betreibers, 27.11.2026): Fehlende Firmenvorgabe beim Laden.** Ohne Datei lädt der Lader keine
    Admins und eine leere `leseklassen`. `setzen` lehnt jede Änderung eines Admins mit dem festen Code `ebenen.nicht_befugt` ab. Die
    Regeln der Firma gelten weiter. Für das Schreiben ist das fail closed. Eine vorhandene, aber ungültige Datei bleibt ein
    Ladefehler. Regel: Abschnitt 15.12.4, „Fehlende Firmenvorgabe“; Fall `faelle-setzen/ohne-firmenvorgabe-keine-admins.json`.

## 15. Fassung 2: Einstellungen im Lauf (Nachtrag 26.11.2026)

Grundlage: Entscheide des Betreibers vom 26.11.2026 E1, E2 und E3, dazu A26, A27, A30 und A31 (`rust/spec/seite/OBERFLAECHE.md`
OF9, Abschnitt 4, L4, L10 und L11). Fassung 1 bleibt gültig, auch `aendern` und alle Fälle in `faelle/`. Fassung 2 fügt zwei reine
Funktionen hinzu, ohne Uhr, Zufall, Datei oder Netz:

- `setzen(stand, wer, bereich, ebene, schluessel, wert) -> Pruefung`: prüft eine einzelne Einstellung und nimmt sie an oder lehnt sie
  mit genau einem festen Code ab (Abschnitt 15.4). Jede Prüfung ergibt eine Protokollzeile (Abschnitt 15.6). Eine Ablehnung ändert
  nichts (S3).
- `sicht(stand, wer) -> Einstellungen`: je Schlüssel des Katalogs der wirksame Wert, seine Herkunft, ob `wer` ihn ändern darf, und ein
  Grund für eine Sperre (Abschnitt 15.5).

Der Dienst ruft beide für das Ereignis `EinstellungSetzen` und die Nachricht `Einstellungen` (Abschnitte 15.9 und 15.10). Er reicht
Felder und Codes unverändert durch und erfindet keinen eigenen Code.

### 15.1 Katalog des Pilots

Jeder Schlüssel gehört zu genau einem Bereich: `nutzer` (Seite B72) oder `admin` (Seite B78). „Ebenen“ nennt die Plätze, auf denen
der Schlüssel stehen darf. Aussehen und Effort sind Wahlen, keine Optionen (OF9 Punkt 7). Nur die Optionen haben die Vorgabe „aus“
(A30, A31).

| Schlüssel | Bereich | Ebenen | Typ | Vorgabe | Richtung (Abschnitt 5.2) | Entscheid |
|---|---|---|---|---|---|---|
| `aussehen.thema` | nutzer | firma, gruppe, nutzer | Wahl: `system`, `hell`, `dunkel` | `system` | frei | OF3 Punkt 2 |
| `aussehen.farbsatz` | nutzer | firma, gruppe, nutzer | Wahl: `grau`, `blauschwarz` | `grau` | frei | OF3 Punkt 2 |
| `aussehen.terminal` | nutzer | firma, gruppe, nutzer | Wahl: `dunkel`, `hell`, `thema` | `dunkel` | frei | OF3 Punkt 2 |
| `terminal.schriftgroesse` | nutzer | firma, gruppe, nutzer | Ganzzahl 9 bis 32 (Pixel) | `13` | frei | OF4 Punkt 2 |
| `sitzung.harness` | nutzer | firma, gruppe, nutzer | Wahl: `claude`, `copilot`, `pi` | `null` (keine Vorgabe) | frei, an `harnesses` gebunden | OF9 Punkt 3 |
| `sitzung.modell` | nutzer | firma, gruppe, nutzer | Modellname (`^[A-Za-z0-9._:/@+-]{1,128}$`, Vertrag 1) | `null` | frei, an `modelle` gebunden | OF9 Punkt 3 |
| `sitzung.effort` | nutzer | firma, gruppe, nutzer | Wahl aus allen Effort-Werten der Harnesses (Vertrag 2, Block `effort`) | `null` | frei | E2 |
| `harnesses` | admin | firma, gruppe, nutzer | Objekt: Klasse → Liste von Harnesses, je höchstens 16, ohne Doppel | `{}` (keine Einschränkung) | schnitt | OF9 Punkt 4 |
| `modelle` | admin | firma, gruppe, nutzer | Objekt: Klasse → Liste von Modellnamen, je höchstens 16, ohne Doppel | `{}` | schnitt | OF9 Punkt 4 |
| `effort.grenzen` | admin | firma | Liste der Grenzen nach Vertrag 2, Block `effort` (höchstens 64) | `[]` (kein Effort wählbar) | nur_firma | E2 |
| `optionen.copilot_ohne_rueckfrage` | admin | firma, gruppe, nutzer | Wahrheitswert | `false` („aus“) | aus_strenger | A30, A31 |
| `optionen.herabstufen` | admin | firma, gruppe, nutzer | Wahrheitswert | `false` („aus“) | aus_strenger | A30, A39 |
| `optionen.lokale_ca` | admin | firma | Wahrheitswert | `false` („aus“) | nur_firma | A30, B41 |
| `optionen.copilot_telemetrie` | admin | firma, gruppe, nutzer | Wahrheitswert | `false` („aus“) | aus_strenger | A30; Team-Issue #56, Punkt 4 |
| `optionen.copilot_eingeschraenktes_token` | admin | firma | Wahrheitswert | `false` („aus“) | nur_firma | A30; Team-Issue #56, Punkt 2 |
| `optionen.herstellerzugang` | admin | keine | Wahrheitswert | `false` („aus“), fest | fest | A11, A30; Team-Issue #12 |
| `leseklassen` | admin | firma | Liste von Einträgen `{"ebene", "wer", "klasse"}`, höchstens 4096 (Abschnitt 15.11) | `[]` (niemand liest) | nur_firma | A05, A34; Projektentscheid, 27.11.2026 |

- Die Klassen in `harnesses` und `modelle` sind die `klassen` der Firmengrundregel. Eine andere Klasse ist `ebenen.wert_ungueltig`.
- Einen Schlüssel für Admins, Rollen, Gruppen oder Routen gibt es im Katalog nicht. Ein solcher Schlüssel ist
  `ebenen.einstellung_unbekannt`. So macht keine Einstellung jemanden zum Admin (Abschnitt 15.2).
- `sitzung.effort` prüft nur die Form. Die Grenze gilt erst in `Startwahl` und beim Start (Vertrag 1, Stufe 18): Eine Vorgabe außerhalb
  der Grenze wird dort `null`. Grund: Die Grenze hängt von Harness, Klasse und Modell ab, die Vorgabe nicht.
- Neue Schlüssel kommen über eine neue Zeile dieser Tabelle und eine neue Fassung dieses Abschnitts. Weitere Optionen kommen nur auf
  Entscheid des Betreibers dazu.

#### 15.1.1 Optionen des Pilots (Entscheid des Betreibers, 27.11.2026)

Jede Option hat die Vorgabe „aus“. Nur Admins stellen sie um (Bereich `admin`). Auf der Nutzerebene geht nur „aus“ (`aus_strenger`)
oder nichts (`nur_firma`, `fest`). Grund für `nur_firma` bei `lokale_ca` und `copilot_eingeschraenktes_token`: Beide gelten für den
Rechner oder die Anmeldung, nicht für eine Person. Eine Gruppe oder ein Nutzer hätte dort keinen eigenen Wert (hergeleitet).

| Schlüssel | „aus“ heißt | „an“ heißt | Feld, auf das der Lader abbildet |
|---|---|---|---|
| `optionen.copilot_ohne_rueckfrage` | Die Copilot CLI fragt vor jedem Werkzeug. | Die Liste der Firmendatei gilt. | Vertrag 2 `harness.copilot.werkzeuge_ohne_rueckfrage`: bei „aus“ die leere Liste (hergeleitet) |
| `optionen.herabstufen` | Die Automatik senkt keine Klasse (A39). | Die Automatik darf senken, nur Arten der Liste. | Kein Feld in Vertrag 2. Gegenstück ist `herabstufen.an` im Teilvertrag Klassen (`rust/spec/datentor/klassen/VERTRAG.md`, Abschnitt 2 „Optionen: Feld `tiefe`“). Feld folgt mit dem Bau (OPEN 17) |
| `optionen.lokale_ca` | Die lokale CA gilt als `offen`. Eine Cloud-Sitzung startet ohne Freigabe nicht (B41). | Die Firma lässt die lokale CA zu. | Kein Feld in Vertrag 2. Gegenstück ist `ca_zulassung` in `/etc/werkbank/lauf.json` (`rust/spec/paket/README.md`, Abschnitt „lauf.json“). Feld folgt mit dem Bau (OPEN 18) |
| `optionen.copilot_telemetrie` | Das Telemetrie-Ziel der Copilot CLI ist gesperrt. | Das Telemetrie-Ziel ist zugelassen; die Firma stellt es für sich an. | Vertrag 2 `routen[].netzziele`: bei „aus“ fehlt der Eintrag mit Host `telemetry.individual.githubcopilot.com` (hergeleitet, OPEN 19) |
| `optionen.copilot_eingeschraenktes_token` | Die Copilot CLI meldet sich wie heute über die Geräte-Anmeldung an. | Ein fein begrenztes Token ersetzt die Geräte-Anmeldung. | Kein Feld in Vertrag 2. Feld folgt mit dem Bau (OPEN 20) |
| `optionen.herstellerzugang` | Kein Herstellerzugang. | gibt es im Pilot nicht Kein Feld in Vertrag 2. Gegenstück ist `hersteller.fall` mit Wert `kein` im Block `stelle`, den `rust/spec/stelle/fassung2/README.md` (Abschnitt 2) für Fassung 3 von Vertrag 2 vorschlägt. Feld folgt mit dem Bau (OPEN 21) |

- **`optionen.herstellerzugang` ist im Pilot fest „aus“** (Team-Issue #12). Er steht nur als Wert da. `sicht` zeigt ihn mit
  `aenderbar: false` und dem Grund `fest`, auch für Admins. `setzen` lehnt jeden Wert auf jeder Ebene mit dem vorhandenen Code
  `ebenen.ebene_gesperrt` ab (Stufe 5), auch `false` und `null`. Der Code passt, weil der Schlüssel auf keiner Ebene stehen darf. Ein
  Wert, den ein Stand trotzdem trägt, wirkt nicht (Feldart `fest`, Abschnitt 5.2).
- Die Fälle stehen in `faelle-setzen/option-<name>-nutzer-abgelehnt.json` und `option-<name>-admin-angenommen.json`, für den
  Herstellerzugang in `option-herstellerzugang-fest.json`.

### 15.2 Wer Admin ist (E1; Projektentscheid, 26.11.2026)

- Die **Kontenliste der Admins** steht in der **Firmenvorgabe**. Sie nennt Konten am Rechner (Benutzernamen), damit die IT sie
  pflegen kann. Die Firmenvorgabe ist eine Datei des Systems. Ändern lässt sie sich nur mit Systemrechten (unter Linux root, unter
  Windows ein Administrator, unter macOS ein Administrator mit `sudo`) und, nur für `leseklassen`, über den Schreibhelfer
  (Abschnitt 15.12). Ihr Ort je Betriebssystem ist OPEN 1. Sie ist keine Datei der
  Ebenen und steht nie in `stand.firma`.
- **Das Konto des Betrachters** liest der Dienst aus dem Verbindungspartner des lokalen Sockets oder der Named Pipe (L11; die Form
  legt der Vertrag `dienst` fest). Nie aus einer Angabe der Ansicht, nie aus einer Datei der Ebenen, nie vom Agenten.
- **Die Zuordnung von Konto zu Pseudonym liegt nur beim Dienst.** Er bildet das Pseudonym über die Stelle (Vertrag 9 Personenbezug,
  `person(kennung, …)`, `rust/spec/personenbezug/README.md` Abschnitt 4). Er gibt `setzen` und `sicht` nur das Pseudonym als `wer`.
  Ebenso bildet er aus der Kontenliste die Menge `stand.admins` der Pseudonyme. Kein Konto steht in einer Datei der Ebenen oder in
  einer Protokollzeile. In einer Nachricht an die Ansicht steht ein Konto nur auf einem Weg: in `Einstellungen` an einen Admin, an der
  Stelle des Pseudonyms (Entscheid des Betreibers, 27.11.2026; Abschnitt 15.13). In einem Fall steht kein echtes Konto; der Platzhalter `konto-beispiel` steht
  nur als abgelehnte Eingabe (Abschnitt 15.11).
- **Eine Quelle (Entscheid des Betreibers, 27.11.2026; schließt OPEN 10).** Der Dienst leitet auch `rollen` aus der Kontenliste der
  Firmenvorgabe ab: Die Rolle `admin` in `rollen` der gespeicherten Firmendatei hat genau, wer in `stand.admins` steht. Eine zweite
  Quelle gibt es nicht. `aendern` liest die Admins darum aus `stand.admins`, sobald der Stand sie trägt (im Lauf immer). Eine
  eingereichte Firmendatei, deren Rolle `admin` andere Pseudonyme nennt als `stand.admins`, lehnt `aendern` mit `ebenen.nicht_befugt`
  ab: Die Admins ändern sich nur über die Kontenliste, also mit Systemrechten. Die Rolle `einsicht` bleibt in der Firmendatei (B09).
  Ein Stand ohne `admins` (die Fälle von Fassung 1) liest `rollen` wie bisher. Fall: `faelle/admins-eine-quelle.json`.
- **Admin** für `setzen` und `sicht` ist genau ein `wer` in `stand.admins`. Eine Angabe der Ansicht macht niemanden zum Admin: Das
  Ereignis `EinstellungSetzen` hat kein Feld dafür (Abschnitt 15.9). Eine eingereichte Datei, die jemanden als Admin nennt, macht ihn
  ebenfalls nicht dazu (Fall `befugnis-selbst-admin` für `aendern`; für `setzen` der Fall `kein-admin-durch-angabe`).
- **Konten in `leseklassen` (Projektentscheid, 27.11.2026).** In der Firmenvorgabe nennt ein Eintrag mit `ebene: nutzer` in `wer`
  das Konto, wie die Kontenliste der Admins. Es ist Pflicht des Dienstes, jedes solche Konto dem Pseudonym der Person
  (`^ps_[0-9a-f]{32}$`) zuzuordnen, bevor er die Liste weitergibt: an `setzen` und `sicht` (im Stand und im Wert einer Änderung) und an
  datentor (`Personen::neu`). Dasselbe gilt für eine Liste, die ein Admin in der Ansicht eingibt. Ein Konto erreicht datentor nie und
  steht nie in einer Protokollzeile, auch nicht in den Prüfsummen `alt` und `neu`. `setzen` lehnt ein Konto an der Stelle eines
  Pseudonyms mit `ebenen.wert_ungueltig` ab (Fall `leseklassen-form-ungueltig`). Ein Konto, das der Dienst nicht zuordnen kann,
  gibt er nicht weiter. In einer Änderung aus der Ansicht wird es zu `wer: null`, das `setzen` ablehnt (Abschnitt 15.12, Stufe 0);
  beim Laden ist es ein Ladefehler der Firmenvorgabe, und es entsteht kein Tor (OPEN 25, geschlossen).
- Vertrag 9 Anmeldung (`rust/spec/anmelden/README.md`) steht nicht dagegen. Er regelt nur die Anmeldung der Copilot CLI und kennt
  weder Pseudonyme noch Admins; den Kontonamen liest er aus `/etc/passwd` (Zeile 44, alte Fassung). Offen bleibt die Kennungsform
  (OPEN 11).

### 15.3 Stand

```json
{"klassen": ["oeffentlich", "intern", "kundendaten"],
 "admins": ["ps_00000000000000000000000000000b01"],
 "gruppen": {"g-eins": ["ps_00000000000000000000000000000c01"]},
 "werte": {"firma": {"aussehen.thema": "hell"},
           "gruppen": {"g-eins": {"terminal.schriftgroesse": 15}},
           "nutzer": {"ps_00000000000000000000000000000c01": {"aussehen.farbsatz": "blauschwarz"}}}}
```

- `klassen` und `gruppen` kommen aus der gespeicherten Firmendatei (Abschnitt 3.1), `admins` vom Dienst (Abschnitt 15.2).
- `werte` hält je Platz die gesetzten Schlüssel. Ein fehlender Schlüssel heißt: Auf dieser Ebene steht kein Wert.
- Wo `werte` gespeichert wird, ist OPEN 1. Die Speicherform ist frei; dieser Vertrag legt nur den Stand fest, den beide Funktionen sehen.

### 15.4 `setzen`: Prüfreihenfolge und Codes

`ebene` ist `firma`, `nutzer` oder `gruppe <schluessel>` (ein Leerzeichen, dann der Schlüssel der Gruppe in Kennungsform). `nutzer`
heißt immer: der Platz von `wer` selbst. Einen Weg, die Nutzerebene eines anderen zu setzen, gibt es im Pilot nicht.
`wert` `null` nimmt den Eintrag dieser Ebene zurück.

Genau ein Code, der der ersten verletzten Stufe:

| Stufe | Code | Verletzung |
|---|---|---|
| 1 | `ebenen.einstellung_unbekannt` | `schluessel` ist kein Text oder steht nicht im Katalog |
| 2 | `ebenen.bereich_falsch` | `bereich` ist nicht der Bereich des Schlüssels (auch jeder andere Text als `nutzer` und `admin`) |
| 3 | `ebenen.ebene_unbekannt` | `ebene` hat keine der drei Formen |
| 4 | `ebenen.gruppe_unbekannt` | die Gruppe steht nicht in `gruppen` der Firma |
| 5 | `ebenen.ebene_gesperrt` | der Schlüssel darf auf dieser Ebene nicht stehen (Spalte „Ebenen“) |
| 6 | `ebenen.nicht_befugt` | Ebene `firma` oder `gruppe <schluessel>`, und `wer` steht nicht in `stand.admins` |
| 7 | `ebenen.wert_ungueltig` | `wert` ist nicht `null` und verletzt Typ, Werte oder Grenzen des Katalogs; für `leseklassen` auch `wert` `null` (Abschnitt 15.11) |
| 8 | `ebenen.lockerung` | `wert` ist nach Abschnitt 5.2 lockerer als oben; auf der Ebene `firma` nie |
| 9 | `ebenen.ausser_grenze` | `sitzung.harness` oder `sitzung.modell` steht in keiner Klasse zur Wahl, nach `harnesses` oder `modelle` bis zu dieser Ebene |

- Prüfbezug: Eine Gruppe wird gegen die Firma geprüft, der Nutzer gegen Firma und seine Gruppe, nie gegen seinen alten Wert (wie
  Abschnitt 5). Ein Admin auf der Nutzerebene ist dort Nutzer: Er darf auch nur verschärfen.
- Eine Ablehnung ändert den Stand nicht. Eine Annahme schreibt `wert` an den Platz oder entfernt den Eintrag bei `null`.
- Mehr als ein Code je Prüfung gibt es nicht. Der Dienst reicht den Code als `grund` der `Ablehnung` durch (Abschnitt 15.9).

### 15.5 `sicht`

`sicht(stand, wer)` gibt ein Objekt mit genau einem Eintrag je Schlüssel des Katalogs:

| Feld | Wert |
|---|---|
| `wert` | der wirksame Wert für `wer` über Firma, seine Gruppe und seine Nutzerebene (Abschnitt 5.2); ohne Eintrag die Vorgabe |
| `herkunft` | `firma`, `gruppe <schluessel>` oder `nutzer`: die tiefste Ebene, die den Wert echt geändert hat. Die Vorgabe des Katalogs zählt als `firma` (wie Abschnitt 6.1) |
| `aenderbar` | ob `wer` den Schlüssel ändern darf |
| `grund` | `null`, `nur_strenger`, `nur_admin` oder `fest` |

Für `leseklassen` zeigt `wert` einem Betrachter, der kein Admin ist, nur die Einträge, die ihn nennen: seinen eigenen Eintrag
(`ebene: nutzer`, `wer` gleich `wer`) und den Eintrag seiner Gruppe, in der Reihenfolge der Liste und ohne Rangfolge. Ein Admin sieht
die ganze Liste. Welche Klasse daraus wirkt, sagt `sicht` nicht (Abschnitt 15.11).

Regeln für `aenderbar` und `grund`, in dieser Reihenfolge:

0. Feldart `fest`: `false`, `fest`, auch für einen Admin (Nachtrag 27.11.2026).
1. `wer` ist Admin: `true`, `null`.
2. Bereich `nutzer`: `true`, `null`.
3. Der Schlüssel darf nicht auf der Nutzerebene stehen: `false`, `nur_admin`.
4. Schalter mit wirksamem Wert `false`: `false`, `nur_admin` (strenger geht es nicht).
5. Sonst: `true`, `nur_strenger`.

Die Ansicht zeigt den Grund mit einem festen kurzen Text (OF9 Punkt 6): `nur_admin` „Vorgabe der Firma“, `nur_strenger` „Nur strenger
als die Vorgabe“, `fest` „Im Pilot fest aus“. Die Herkunft zeigt sie als „Firma“, „Gruppe <Anzeigename>“ oder „du“ (E3).

### 15.6 Protokollzeile

Jede Prüfung von `setzen` ergibt eine Zeile in der Form von Abschnitt 7.1 (Vertrag 5, Abschnitt 6.6). Die Felder:

| Feld | Wert |
|---|---|
| `aktionsname`, `ergebnis` | wie Abschnitt 7.1 |
| `ebene` | `firma`, `gruppe`, `nutzer`, oder `null`, wenn `ebene` keine der drei Formen hat |
| `ziel` | Schlüssel der Gruppe, Pseudonym von `wer` bei `nutzer`, sonst `null` |
| `handelnd` | `wer` (Pseudonym; nie ein Konto) |
| `alt` | sha256 hex klein über RFC 8785 von `{"schluessel": <schluessel>, "wert": <alter Wert>}` des Platzes, `null`, wenn dort kein Wert stand oder der Platz unbekannt ist |
| `neu` | sha256 hex klein über RFC 8785 von `{"schluessel": <schluessel>, "wert": <wert>}` |
| `code` | `null` bei Annahme, sonst der Code aus Abschnitt 15.4 |
| `antrag`, `pflicht`, `zweiter` | immer `null` |

So stehen Admin (als Pseudonym), Ebene, alter und neuer Wert (als Prüfsumme) in der Zeile (A27). Die Zeit setzt die Bibliothek aus
Vertrag 5. Der Schlüssel steht nur in der Prüfsumme; ein eigenes Feld braucht Vertrag 5 (Ergebnis an den Vertrag `stelle`).

### 15.7 Sicherheitszusagen

- **S8 Kein Admin aus der Ansicht:** Ob `wer` auf Firma oder Gruppe schreiben darf, entscheidet nur `stand.admins`.
- **S9 Nutzer nur strenger:** Für jede angenommene Änderung auf der Nutzerebene ist der wirksame Wert jedes Schlüssels mit Richtung
  `schnitt` oder `aus_strenger` gleich oder strenger als vorher.
- **S10 Kein Konto:** Kein Ergebnis von `setzen` oder `sicht` enthält ein Konto. Für `leseklassen` nimmt `setzen` bei `ebene: nutzer`
  nur ein Pseudonym an; `pruefen.py` prüft das für jede Annahme und jede Sicht. Erlaubt ist genau ein Weg eines Kontos zur Ansicht:
  Der Dienst setzt es in `Einstellungen` für einen Admin an die Stelle des Pseudonyms (Abschnitt 15.13). Jeder andere Weg bleibt
  verboten, auch in jeder Zeile und in jeder Nachricht an einen Nichtadmin; `pruefen.py` prüft das in `faelle-schreiben/`.
- **S11 Firmenvorgabe nur über den Schreibhelfer:** Außer root schreibt nur der Schreibhelfer die Firmenvorgabe, und nur für einen
  Partner, dessen uid in der gerade gültigen Datei Admin ist (Abschnitt 15.12, H5). Der Dienst reicht nur weiter und schreibt sie nie
  selbst. Die Ansicht, das Konto des Nutzers und ein Agent schreiben sie nie.
- **S12 Kein Teilschreiben:** Jeder Fehler auf dem Schreibweg lässt die Firmenvorgabe byte-gleich und ergibt im Dienst eine Zeile
  `regel.abgelehnt`. Einzige Ausnahme ist `firmenvorgabe.zustand_unklar`; auch dann steht ein ganzer Inhalt, und der Dienst sperrt.
- **S13 Nur `leseklassen`:** Der Schreibhelfer ändert nie etwas außerhalb von `leseklassen`, auch nicht die Kontenliste der Admins
  (H10). Nach dem Ersetzen gehört die Datei `werkbank-vorgabe` und hat Modus `0644`.
- S3 und S7 gelten auch für `setzen` und `sicht`.

### 15.8 Fälle

`faelle-setzen/<name>.json` mit `erwartet-setzen/<name>.json`. Das Crate `rust/crates/regelebenen` setzt `setzen` und `sicht` um und
liest diesen Ordner in seinem Außentest (`tests/faelle.rs`), dazu aus `faelle-schreiben/` die Teile der Bibliothek.

Öffentliche Form von `laden` und der Prüfung H3 (Projektentscheid, 27.11.2026). `vorgabe::laden(bytes)` prüft H4 an den Bytes der
Firmenvorgabe (höchstens 1 048 576 Byte, strenges JSON, Form wie der Helfer) und gibt die Kontenliste der Admins und `leseklassen` mit
Konten zurück, dazu den Inhalt und die sha256 der Bytes; sonst `firmenvorgabe.ladefehler`. `Firmenvorgabe::stand(klassen, gruppen,
werte, pseudonym)` baut daraus den Stand nach Abschnitt 15.3. `pseudonym` ist die Zuordnung Konto zu Pseudonym als Funktion des
Aufrufers. Grund: Die Zuordnung liegt nur beim Dienst (Abschnitt 15.2, DI-L11 Punkt 4), die Prüfung danach soll für jeden Aufrufer
gleich sein. Ein Admin-Konto ohne Pseudonym ist kein Admin; ein Konto in `leseklassen` ohne Pseudonym und eine Liste, die danach
Abschnitt 15.11 verletzt, sind `firmenvorgabe.ladefehler`. `vorgabe::h3(lage)` nimmt je Elternordner, Ordner und Datei Art, Eigentümer
und Modus aus `lstat` oder `fstat`; ein gescheiterter Aufruf steht als fehlende Angabe. Ergebnis ist `firmenvorgabe.datei_unsicher`
oder kein Code. Die Größe prüft `laden` an den gelesenen Bytes, nicht `h3`. Die Systemaufrufe bleiben beim Aufrufer. Der Schreibhelfer
nutzt dieselben Regeln für H3 und H4.

```json
{"name": "…", "anforderungen": ["A26"], "beschreibung": "…", "stand": {…Abschnitt 15.3…},
 "schritte": [{"setzen": {"wer": "ps_…", "bereich": "nutzer", "ebene": "nutzer", "schluessel": "aussehen.farbsatz", "wert": "blauschwarz"}},
              {"sicht": {"wer": "ps_…"}}]}
```

Ergebnis je Schritt: `{"angenommen", "code", "zeile"}` für `setzen`, `{"einstellungen": {…}}` für `sicht`. `bau-faelle.py` rechnet
jede Erwartung mit der Referenz (`setzen`, `sicht`) und prüft sie gegen ausgeschriebene Kernwerte. `pruefen.py` prüft Paarung, Codes,
die Form der Zeile und der Sicht, dass `admins` nur Pseudonyme nennt und dass es je Code mindestens einen Fall gibt. Die beiden Fälle
aus OF9 heißen `of9-nutzer-lockert` und `of9-nichtadmin-schreibt-adminwert`. Die Admins stehen nur in `stand.admins` (eine Quelle,
Abschnitt 15.2). Je Option des Katalogs gibt es einen Fall „Nutzer setzt: abgelehnt“ und einen Fall „Admin setzt: angenommen“,
für den Herstellerzugang den Fall `option-herstellerzugang-fest` (Abschnitt 15.1.1). Die Fälle zu `leseklassen` heißen
`leseklassen-*` (Abschnitt 15.11). Der Fall `ohne-firmenvorgabe-keine-admins` belegt die fehlende Firmenvorgabe (Abschnitt
15.12.4). Die Fälle zum Schreibweg und zum Konto in der Ansicht der Admins stehen in `faelle-schreiben/`
(Abschnitte 15.12 und 15.13).

### 15.9 Ereignis `EinstellungSetzen` (Ansicht an Dienst)

```json
{"art": "EinstellungSetzen", "schluessel": "aussehen.farbsatz", "wert": "blauschwarz", "bereich": "nutzer", "ebene": "nutzer"}
```

| Feld | Typ | Bedeutung |
|---|---|---|
| `schluessel` | Text | Schlüssel des Katalogs (Abschnitt 15.1) |
| `wert` | JSON-Wert oder `null` | neuer Wert; `null` nimmt den Eintrag der Ebene zurück |
| `bereich` | Text | `nutzer` oder `admin` |
| `ebene` | Text | `firma`, `nutzer` oder `gruppe <schluessel>` |

- Genau diese vier Felder neben `art`. Ein weiteres Feld, etwa `admin`, `konto` oder `wer`, lehnt der Dienst mit seinem Code für eine
  falsche Form ab, bevor er `setzen` ruft.
- Der Dienst ruft `setzen(stand, wer, bereich, ebene, schluessel, wert)` mit `wer` aus Abschnitt 15.2.
- Angenommen: Der Dienst schickt jedem Betrachter, dessen `sicht` sich ändert, eine neue Nachricht `Einstellungen`.
- Abgelehnt: Der Dienst schickt dem Absender eine `Ablehnung` mit `grund` gleich dem Code aus Abschnitt 15.4, unverändert.
- Die Kurzfassung für den Vertrag `dienst` steht außerhalb des Repos (`dienst-vorschlag.md`).

### 15.10 Nachricht `Einstellungen` (Dienst an Ansicht)

```json
{"art": "Einstellungen", "fassung": 2, "admin": false,
 "werte": {"aussehen.farbsatz": {"wert": "blauschwarz", "herkunft": "nutzer", "aenderbar": true, "grund": null},
           "effort.grenzen": {"wert": [], "herkunft": "firma", "aenderbar": false, "grund": "nur_admin"}}}
```

| Feld | Typ | Bedeutung |
|---|---|---|
| `fassung` | Ganzzahl `2` | Form dieses Abschnitts |
| `admin` | Wahrheitswert | `wer` steht in `stand.admins`; nur dann zeigt die Ansicht den Bereich B78 |
| `werte` | Objekt | das Ergebnis von `sicht(stand, wer)`, je Schlüssel des Katalogs; für einen Admin mit Konten an der Stelle der Pseudonyme (Abschnitt 15.13) |

- Der Dienst schickt sie beim Verbinden und nach jeder angenommenen Änderung, die den Betrachter betrifft.
- Die Nachricht nennt nie die Liste der Admins. Ein Konto nennt sie nur einem Admin, an der Stelle des Pseudonyms (Entscheid des
  Betreibers, 27.11.2026; Abschnitt 15.13). Einem Betrachter, der kein Admin ist, nennt sie nie ein Konto.
- `admin` ist eine Anzeige. Die Befugnis prüft `setzen` bei jeder Änderung neu.
- Vertrag `zustandskern` nennt für `Einstellungen` heute `wert`, `vorgabe` und `herkunft` mit `vorgabe` als Herkunft
  (`rust/spec/zustandskern/VERTRAG.md:142`). Dieser Abschnitt ersetzt das für Fassung 2. Die Angleichung dort gehört dem Vertrag
  `zustandskern`.

### 15.11 Admin-Feld `leseklassen` (Projektentscheid, 27.11.2026)

Das Feld legt fest, welche Datenklasse eine Person höchstens lesen darf. datentor nutzt es in P4, das Ordner-Blatt in L9. Dieser
Vertrag legt nur Form, Prüfung und Befugnis fest. Die Wirkung rechnet nur datentor (`rust/spec/datentor/person/VERTRAG.md`,
Abschnitte 2 bis 4), weder `setzen` noch `sicht` noch der Dienst.

**Form.** Dieselbe wie in datentor, Abschnitt 3, damit der Lader die Liste 1:1 durchreicht:

```json
"leseklassen": [{"ebene": "gruppe", "wer": "g-eins", "klasse": "intern"},
                {"ebene": "nutzer", "wer": "konto-beispiel", "klasse": "oeffentlich"}]
```

- `ebene: gruppe`: `wer` ist ein Schlüssel aus `gruppen` der Firmengrundregel.
- `ebene: nutzer`: In der Firmenvorgabe ist `wer` das Konto, wie bei der Kontenliste der Admins. Die Firmenvorgabe ist nur mit
  Systemrechten oder über den Schreibhelfer änderbar (Abschnitt 15.12). Der Dienst ordnet das Konto dem Pseudonym zu, bevor er
  die Liste weitergibt (Abschnitt 15.2). Im Stand und in
  `setzen` steht darum das Pseudonym: `{"ebene": "nutzer", "wer": "ps_00000000000000000000000000000c01", "klasse": "oeffentlich"}`.
- `klasse` ist ein Schlüssel aus `klassen` der Firmengrundregel, byte-genau (D14).
- Höchstens 4096 Einträge. Die leere Liste ist gültig.

**Ungültig** ist der Wert, wenn eine dieser Bedingungen gilt. Ein ungültiger Wert ist ein Ladefehler; `setzen` meldet in jedem Fall den
festen Code `ebenen.wert_ungueltig` (Stufe 7):

- Dasselbe Paar aus `ebene` und `wer` steht zweimal, auch mit derselben Klasse.
- `klasse` steht nicht byte-genau in `klassen`.
- Ein Eintrag hat ein unbekanntes Feld (etwa `kunden`, Frage F4 in datentor) oder es fehlt eines der drei Felder.
- `null` steht an irgendeiner Stelle. Für `leseklassen` ist `wert` `null` auch keine Rücknahme: „Niemand liest“ ist die leere Liste.
- Die Liste hat mehr als 4096 Einträge.
- Dazu: `ebene` ist weder `nutzer` noch `gruppe`; eine Gruppe steht nicht in `gruppen`; bei `ebene: nutzer` steht kein Pseudonym.

**Wirkung (nur datentor).** Klassen sind nach ihrer Position in `klassen` geordnet, die niedrigste zuerst. Eine Person liest Klasse c
genau dann, wenn Position(c) <= Position(Leseklasse). Der eigene Eintrag gilt vor den Gruppen, auch nach unten (F2). Sonst gilt die
höchste Klasse der Gruppen (F1). **Fehlt jeder Eintrag, gilt keine Klasse:** Die Person liest nichts (P2 `person-ohne-leseklasse`, F3).
Die Vorgabe `[]` heißt darum: Niemand liest. Soll eine Installation nicht leer starten, legt die Erstausstattung einen Gruppeneintrag
an. Die Fälle zu F1, F2 und P2 stehen in `rust/spec/datentor/person/sichtbar/` und werden hier nicht gedoppelt.

**Befugnis.** Nur Admins setzen das Feld: Bereich `admin`, Ebene `firma`, Feldart `nur_firma`. Auf einer Gruppe oder beim Nutzer ist
es `ebenen.ebene_gesperrt`, auf der Firma für einen Nichtadmin `ebenen.nicht_befugt`. Der Nutzer setzt es nicht und lockert es nicht.
`sicht` zeigt dem Nutzer seinen eigenen wirksamen Eintrag mit `aenderbar: false` und Grund `nur_admin` (Abschnitt 15.5).

**Protokoll.** Jede Änderung ergibt eine Zeile wie in Abschnitt 15.6, auch jede Ablehnung. `alt` und `neu` sind Prüfsummen über die
Liste mit Pseudonymen; ein Konto steht nie in der Zeile.

**Fälle** in `faelle-setzen/`:

| Fall | Inhalt |
|---|---|
| `leseklassen-admin-setzt` | Ein Admin setzt einen gültigen Eintrag je Gruppe und einen für einen Nutzer. |
| `leseklassen-nutzer-setzt` | Ein Nutzer setzt das Feld auf Firma, Gruppe und Nutzerebene; alles abgelehnt. |
| `leseklassen-klasse-unbekannt` | `geheim`, `Intern`, `intern ` |
| `leseklassen-paar-doppelt` | dasselbe Paar mit anderer und mit gleicher Klasse; gleiches `wer` auf zwei Gruppen ist gültig |
| `leseklassen-feld-unbekannt` | Feld `kunden`, fehlende `klasse`, Objekt statt Liste |
| `leseklassen-null` | `null` als Wert, als `klasse`, als `wer` und als Eintrag |
| `leseklassen-4097` | 4096 Einträge angenommen, 4097 abgelehnt |
| `leseklassen-form-ungueltig` | Platzhalter-Konto `konto-beispiel` statt Pseudonym, unbekannte Gruppe, Ebene `rolle` |
| `leseklassen-sicht-nutzer` | Sicht für drei Nutzer und einen Admin |

**Offen** ist OPEN 24 in Abschnitt 14. OPEN 22, 23 und 25 sind geschlossen (Abschnitte 15.12 und 15.13).

### 15.12 Schreibweg der Firmenvorgabe (Entscheid des Betreibers, 27.11.2026; schließt OPEN 22; Schreibhelfer, Entscheid des Betreibers, 27.11.2026)

Die Firmenvorgabe bleibt eine Datei des Systems, die kein Nutzerkonto schreiben kann. Der Dienst läuft unter dem Konto des Nutzers
(Vertrag `dienst`, DI2) und kann sie darum nicht schreiben. Eine Änderung eines Admins aus der Ansicht schreibt deshalb ein kleiner
**Schreibhelfer** mit eigenem Systemnutzer. Der Helfer ist nicht root. Er kann genau eine Sache: die Firmenvorgabe atomar ganz
ersetzen. Die Ansicht schreibt die Datei nie, das Konto des Nutzers schreibt sie nie, und kein Agent schreibt sie. Die IT pflegt die
Datei weiter von Hand mit `sudo`. Der Weg gilt für ein `EinstellungSetzen` mit `ebene` `firma` und `schluessel` `leseklassen`, im Pilot
der einzige Schlüssel der Firmenvorgabe, den die Ansicht ändert. Die Kontenliste der Admins ändert sich nie über die Ansicht und nie
über den Helfer, nur mit Systemrechten von Hand (eine Quelle, Abschnitt 15.2). Konto, Einheiten und Rechte unter Linux legt
`rust/spec/paket/README.md`, Teil A3, fest. Für macOS und Windows gibt es den Helfer noch nicht (OPEN 26).

#### 15.12.1 Rollen

| Rolle | Tut | Tut nie |
|---|---|---|
| Dienst (Konto des Nutzers) | Prüft das Ereignis (Stufen 0 bis 2), baut die ganze neue Datei aus dem zuletzt geladenen Inhalt und dem neuen Wert von `leseklassen` und reicht sie an den Helfer. Gibt den Code der Antwort unverändert als `grund` weiter, schreibt die Zeile und übernimmt den neuen Stand erst nach der Antwort ohne Code. | Schreibt die Firmenvorgabe nie selbst, auch nicht mit einem anderen Werkzeug. |
| Schreibhelfer (Systemnutzer `werkbank-vorgabe`) | Prüft Partner, Datei und Anfrage selbst (Stufen H1 bis H10) und ersetzt die Datei atomar (Stufen H11 und H12). Vertraut dem Dienst nicht: Jeder Prozess am Rechner kann den Socket erreichen. | Liest keine Ebene, keine Stelle und kein Pseudonym. Ändert nie etwas außer `leseklassen`. Startet nichts, hat kein Netz. |
| IT (root) | Legt Konten der Admins an und ändert die Datei von Hand. Setzt danach Eigentümer und Modus zurück (OPEN 27). | |
| `postinst` (root) | Legt die Datei nur an, wenn sie fehlt, mit dem installierenden Konto als einzigem Admin (Abschnitt 15.12.4). | Ändert eine vorhandene Datei nie, auch nicht beim Upgrade und auch keine ungültige. |

#### 15.12.2 Protokoll des Sockets

- Unix-Socket `/run/werkbank-vorgabe/vorgabe.sock` (Typ `SOCK_STREAM`). Er gehört systemd; nur der Socket lauscht, der Helfer
  bindet nie selbst (Paket, Teil A3). Jeder Nutzer kann sich verbinden. Die Grenze ist die Prüfung des Partners, nicht das Recht am
  Socket.
- **Eine Verbindung, eine Anfrage, eine Antwort.** Der Client schickt genau ein JSON-Objekt in UTF-8 und schließt seine Schreibseite
  (`shutdown(SHUT_WR)`). Der Helfer antwortet mit genau einem JSON-Objekt und einem Zeilenende und schließt die Verbindung. Eine
  zweite Anfrage auf derselben Verbindung gibt es nicht. Der Helfer bearbeitet eine Verbindung nach der anderen; Anfragen laufen
  nie gleichzeitig.
- **Grenzen.** Die Anfrage hat höchstens 1048576 Bytes (1 MiB). Grund: 4096 Einträge von `leseklassen` mit langen Kontonamen passen
  hinein (hergeleitet). Der Helfer liest höchstens 1048577 Bytes; eine längere Anfrage ist `firmenvorgabe.anfrage_zu_gross`, und er
  liest nicht weiter.
- **Fristen.** Die Anfrage muss 2 s nach der Annahme der Verbindung vollständig sein (wie `hallo_frist` in `peerkanal`), sonst
  `firmenvorgabe.anfrage_frist`. Der Dienst wartet höchstens 10 s auf die Antwort, vom Verbinden an. Hat er bis dahin keine Antwort
  oder bricht die Verbindung nach dem Senden ab, kennt er den Ausgang nicht: `firmenvorgabe.zustand_unklar` mit Sperre (unten).
  Scheitert schon `connect` (Socket fehlt, `ECONNREFUSED`, `EACCES`), hat er nichts gesendet: `firmenvorgabe.helfer_nicht_erreichbar`.
- **Ablehnung vor dem Lesen.** Lehnt der Helfer in H1 bis H5 ab, schreibt er die Antwort, schließt seine Schreibseite und verwirft
  danach den Rest der Anfrage bis zur Grenze und zur Frist, ohne ihn als Anfrage zu lesen. „Bevor ein Byte gelesen wird“ gilt für die
  Entscheidung, nicht für dieses Verwerfen. Grund (gemessen in einer Wegwerf-VM): Schließt der Helfer mit ungelesenen Bytes, setzt der
  Kernel die Verbindung zurück, und der Client bekommt `ECONNRESET` statt der Antwort.

Anfrage (Dienst oder ein anderer Prozess an den Helfer):

```json
{"art": "Ersetzen", "fassung": 1, "basis": "<sha256 hex klein der Bytes, die der Client zuletzt gelesen hat>",
 "inhalt": {"admins": ["konto-admin"], "leseklassen": [{"ebene": "nutzer", "wer": "konto-nutzer", "klasse": "intern"}]}}
```

Antwort (Helfer an den Client):

```json
{"code": null, "sha256": "<sha256 hex klein der neuen Bytes>"}
{"code": "firmenvorgabe.partner_nicht_admin", "sha256": null}
```

- Genau diese vier Felder der Anfrage und diese zwei Felder der Antwort. `sha256` steht nur bei `code` `null`.
- `inhalt` ist die ganze neue Firmenvorgabe. Der Helfer schreibt sie als JSON mit sortierten Schlüsseln, Einzug 1 und einem
  Zeilenende. Die Form einer Änderung von Hand bleibt darum nicht erhalten.
- Je Anfrage schreibt der Helfer eine Zeile nach stderr (Journal): `werkbank-vorgabe: ersetzt` oder `werkbank-vorgabe: abgelehnt:
  <code>`. Sie nennt kein Konto, keine uid und keinen Inhalt (wie DI-L11 Punkt 5).

#### 15.12.3 Prüfreihenfolge und Codes

Genau ein Code, der der ersten verletzten Stufe. Jeder Fehler endet fail closed: Niemand schreibt etwas, und die Datei bleibt
byte-gleich. Einzige Ausnahme ist `firmenvorgabe.zustand_unklar` (unten).

**Im Dienst.** Jede Stufe, auch eine Ablehnung, ergibt genau eine Zeile in der Form von Abschnitt 15.6 mit `ebene` `firma`, `ziel`
`null` und dem Pseudonym des Partners als `handelnd`. Bei einem Fehler heißt sie `regel.abgelehnt` und trägt den Code.

| Stufe | Code | Schritt und Verletzung |
|---|---|---|
| 0 | (kein Code) | Der Dienst nimmt das Konto aus `wer` des lokalen Wegs (DI-L11 Punkt 4), bildet das Pseudonym (Abschnitt 15.2) und ordnet jedes Konto im Wert zu. Ein Konto ohne Zuordnung und ein Pseudonym im Wert werden zu `wer: null`. Ohne Partner nimmt der Dienst kein Ereignis an (DI-L11 Punkt 6). |
| 0a | `firmenvorgabe.zustand_unklar` | Der Dienst ist nach einem früheren `zustand_unklar` gesperrt und hat die Firmenvorgabe seitdem nicht neu geladen. Er spricht den Helfer nicht an. |
| 1 | `ebenen.nicht_befugt` | Das Pseudonym des Partners steht nicht in `stand.admins`. Der Dienst spricht den Helfer dann nicht an. |
| 2 | Codes aus Abschnitt 15.4 | `setzen(stand, wer, "admin", "firma", "leseklassen", wert)` auf einer Kopie des Stands lehnt ab, etwa mit `ebenen.wert_ungueltig`. |
| 3 | `firmenvorgabe.helfer_nicht_erreichbar` | `connect` an den Socket des Helfers scheitert. Der Dienst hat nichts gesendet. |
| 4 | Code der Antwort | Der Dienst schickt die Anfrage mit `basis` gleich der sha256 der zuletzt geladenen Bytes und gibt den Code der Antwort unverändert weiter. Ohne Antwort in der Frist: `firmenvorgabe.zustand_unklar`. |

**Im Helfer.** Stufen H1 bis H5 laufen, bevor der Helfer ein Byte der Anfrage liest.

| Stufe | Code | Schritt und Verletzung |
|---|---|---|
| H1 | `firmenvorgabe.partner_unbekannt` | Der Partner ist nicht feststellbar: `SO_PEERCRED` scheitert oder liefert keine uid, oder zur uid gibt es kein Konto (`getpwuid_r`). |
| H2 | `firmenvorgabe.partner_eingeschlossen` | Der Partner läuft in der Einheit eines Laufs (Cgroup, wie `aufrufer:einschluss` in DI-L11 Punkt 5). Ein Agent schreibt die Firmenvorgabe so nie, auch nicht unter der uid eines Admins. |
| H3 | `firmenvorgabe.datei_unsicher` | Die Datei ist keine reguläre Datei (etwa ein Verweis; `lstat`, `O_NOFOLLOW`), gehört nicht dem Helfernutzer `werkbank-vorgabe` oder ist für Gruppe oder andere schreibbar (Modus `& 022`). Ihr Ordner ist kein Ordner, gehört nicht dem Helfernutzer oder hat `& 022`. Jeder Elternordner bis `/` ist ein Ordner, gehört root und hat nicht `& 022`. |
| H4 | `firmenvorgabe.ladefehler` | Die gerade gültige Datei hat nicht die Form: kein JSON-Objekt, keine Kontenliste `admins` aus Konten, oder `leseklassen` nicht in der Form von Abschnitt 15.11 (Konto statt Pseudonym). Ohne lesbare Kontenliste ist niemand Admin. |
| H5 | `firmenvorgabe.partner_nicht_admin` | Die uid des Partners ist keine uid eines Kontos der Kontenliste in der **gerade gültigen** Datei (je Konto `getpwnam_r`). Eine eingereichte Datei, die jemanden als Admin nennt, macht ihn nicht befugt. |
| H6 | `firmenvorgabe.anfrage_zu_gross` | Die Anfrage hat mehr als 1048576 Bytes. |
| H7 | `firmenvorgabe.anfrage_frist` | Die Anfrage ist 2 s nach der Annahme nicht vollständig. |
| H8 | `firmenvorgabe.anfrage_ungueltig` | Die Anfrage ist kein JSON-Objekt mit genau `art` `Ersetzen`, `fassung` `1`, `basis` (64 Zeichen hex klein) und `inhalt`, oder `inhalt` hat nicht die Form von H4. Klassen und Gruppen prüft der Helfer nicht; das tut `setzen` im Dienst. |
| H9 | `firmenvorgabe.veraltet` | `basis` ist nicht die sha256 der Bytes der Datei. Root hat sie inzwischen geändert; der Helfer überschreibt das nicht. Der Dienst lädt die Datei neu, im Pilot erst bei seinem Neustart (Vertrag `dienst`, DI-E1 Punkt 6 und DI-E2); bis dahin bekommt jede weitere Änderung wieder H9. |
| H10 | `firmenvorgabe.ausser_leseklassen` | `inhalt` weicht außerhalb von `leseklassen` von der gültigen Datei ab, etwa in der Kontenliste der Admins oder durch einen weiteren Schlüssel. |
| H11 | `firmenvorgabe.temp_fehlgeschlagen` | Die Temp-Datei im selben Ordner lässt sich nicht exklusiv anlegen (`O_CREAT \| O_EXCL \| O_NOFOLLOW`, Modus `0600`) oder nicht ganz schreiben. Danach setzt der Helfer Modus `0644` mit `fchmod` an der offenen Datei. Eigentümer ist er selbst; ein `fchown` braucht er nicht. |
| H11 | `firmenvorgabe.fsync_fehlgeschlagen` | `fsync` der Temp-Datei scheitert. |
| H11 | `firmenvorgabe.rename_fehlgeschlagen` | `rename` der Temp-Datei auf den Namen der Firmenvorgabe scheitert. |
| H12 | `firmenvorgabe.ordner_fsync_fehlgeschlagen` | `fsync` des Ordners nach dem `rename` scheitert. Der Helfer schreibt die alten Bytes auf demselben Weg zurück (H11 und H12); danach ist die Datei byte-gleich. |
| H12 | `firmenvorgabe.zustand_unklar` | H12 scheitert, und der Rückweg scheitert auch. Durch `rename` steht der alte oder der neue Inhalt ganz, nie ein Teil. |

- In H11 entfernt der Helfer die Temp-Datei, bevor er antwortet. Die alte Datei berührt er dabei nicht.
- H3 wiederholt der Helfer an den offenen Griffen (`fstat` von Ordner und Datei, Dateien relativ zum offenen Ordner mit
  `openat`). Ein Austausch zwischen Prüfung und Schreiben greift so nicht.
- Erst nach der Antwort ohne Code übernimmt der Dienst den neuen Stand. Er schreibt die Zeile `regel.geaendert` aus `setzen` und
  schickt jedem betroffenen Betrachter eine neue Nachricht `Einstellungen`. Vorher wirkt die Änderung nirgends, auch nicht nach einem
  Fehler.
- Die Datei trägt Konten: Der Dienst schickt `leseklassen` in der Kontoform, in der der Admin sie eingegeben hat. In der Zeile stehen
  nur Pseudonyme; `alt` und `neu` sind Prüfsummen über die Liste mit Pseudonymen (Abschnitt 15.6).
- Die Codes `firmenvorgabe.*` sind fest. Der Dienst reicht sie als `grund` der `Ablehnung` durch (Abschnitt 15.9).
- **`firmenvorgabe.zustand_unklar` mit Sperre (Entscheid des Betreibers, 27.11.2026).** Er entsteht im Helfer (H12, Rückweg
  gescheitert) oder im Dienst (keine Antwort in der Frist). Die Datei trägt einen ganzen Inhalt und nie einen Teil; das bleibt
  fail closed. Der Dienst sperrt: Jedes weitere `EinstellungSetzen` auf diesem Weg bekommt `firmenvorgabe.zustand_unklar` (Stufe 0a),
  und er baut kein Tor, bis er die Firmenvorgabe neu geladen hat. Nach dem Neuladen gilt der geladene Inhalt als Basis.
  Neu laden heißt im Pilot: Der Dienst liest die Firmenvorgabe bei seinem Neustart (Vertrag `dienst`, DI-E1 Punkt 6 und DI-E2;
  Projektentscheid, 27.11.2026). Der Schritt `neuladen` der Fälle steht für diesen Neustart.
- **Ladefehler (OPEN 25, geschlossen).** Ein Konto in `leseklassen`, das der Dienst beim Laden nicht zuordnen kann, ist ein Ladefehler
  der Firmenvorgabe, und es entsteht kein Tor. Der Helfer prüft Konten nicht auf ein Pseudonym; er kennt die Stelle nicht.
- **Rest, den der Helfer nicht trennt (hergeleitet).** Jeder Prozess eines Admins außerhalb eines Laufs erreicht den Helfer und kann
  `leseklassen` ersetzen, auch ohne Dienst. Er erzeugt dann keine Zeile nach Vertrag 5, nur die Zeile im Journal. Das ist dieselbe
  Grenze wie eine weitergereichte Verbindung in DI-L11 Punkt 7.

#### 15.12.4 Datei, Ordner und Rechte (Linux)

| Pfad | Eigentümer | Modus |
|---|---|---|
| `/etc/werkbank` (und jeder Elternordner bis `/`) | root | `0755`, nie `& 022` |
| `/etc/werkbank/vorgabe/` | `werkbank-vorgabe:werkbank-vorgabe` | `0755` |
| `/etc/werkbank/vorgabe/firmenvorgabe.json` | `werkbank-vorgabe:werkbank-vorgabe` | `0644` |

- **Ordner des Helfers (Entscheid des Betreibers, 27.11.2026; schließt OPEN 28).** Der Ordner `vorgabe/` gehört dem Helfernutzer
  mit Modus `0755`, und die Eltern ab `/etc/werkbank` gehören root ohne `& 022`. Es gibt kein Schreibrecht für eine Gruppe und kein
  `CAP_DAC_OVERRIDE`. Grund, gemessen am 27.11.2026 am Mac: In einem Ordner mit Modus `0555` scheitern `open(O_CREAT|O_EXCL)` und
  `rename` mit errno 13 (`EACCES`), auch für den Eigentümer der Datei. In einem root-eigenen Ordner könnte der Helfer also weder die
  Temp-Datei anlegen noch umbenennen. Der Helfernutzer könnte die Datei ohnehin ersetzen; der eigene Ordner gibt ihm nur das Recht,
  dort weitere Dateien anzulegen. Der Lader liest nur den einen Namen.
- **Erster Admin (Entscheid des Betreibers, 27.11.2026).** `postinst` legt die Firmenvorgabe nur an, wenn sie fehlt. Dann trägt sie
  das installierende Konto (aus `SUDO_USER`, wie beim Halter) als einzigen Eintrag in `admins` und eine leere `leseklassen`. Eine
  vorhandene Firmenvorgabe ändert `postinst` nie, auch nicht beim Upgrade und auch nicht, wenn die IT sie vor der Installation
  abgelegt hat. Grund: Wer `sudo` hat, kann die Datei ohnehin direkt ändern; der Eintrag gibt keine neue Macht. Ohne installierendes
  Konto entsteht keine Datei, und bis die IT sie pflegt, ist keine Änderung durch einen Admin möglich (unten, „Fehlende
  Firmenvorgabe“). Eine vorhandene,
  ungültige Datei überschreibt `postinst` nicht; der Ladefehler bleibt fail closed. Ablauf und Prüfpunkte: Paket, Teil A3,
  „Erster Admin“.
- **Fehlende Firmenvorgabe (Entscheid des Betreibers, 27.11.2026; schließt OPEN 30).** Fehlt die Datei (`lstat` meldet `ENOENT`),
  lädt der Lader keine Admins: `stand.admins` ist leer, und `leseklassen` ist leer, also liest niemand (Abschnitt 15.11). `setzen`
  lehnt darum jede Änderung eines Admins mit dem festen Code `ebenen.nicht_befugt` ab (Abschnitt 15.4, Stufe 6), auch für ein Konto,
  das vorher Admin war. Der Dienst spricht den Helfer dann nicht an; ein Prozess, der den Helfer direkt anspricht, bekommt
  `firmenvorgabe.datei_unsicher` (H3, keine reguläre Datei). Die Regeln der Firma ohne Firmenvorgabe gelten weiter: die Firmendatei
  (Abschnitt 3.1) und die Werte der Ebene `firma` im Stand. Ein Nutzer setzt seine Nutzerebene wie sonst. Für das Schreiben ist das
  fail closed. Eine vorhandene, aber ungültige Datei bleibt ein Ladefehler, und es entsteht kein Tor (Abschnitt 15.12.3). Fall:
  `faelle-setzen/ohne-firmenvorgabe-keine-admins.json`.
- Die IT ändert die Datei mit `sudo` und setzt danach `chown werkbank-vorgabe:werkbank-vorgabe` und `chmod 0644`. Bis dahin lehnt der
  Helfer mit `firmenvorgabe.datei_unsicher` ab (Fall `schreiben-datei-root-eigen`, OPEN 27).
- Unter Windows liegt die Firmenvorgabe in `%ProgramData%\Werkbank\admin\` (Paket, Teil B). Ein Helfer dort fehlt noch (OPEN 26).
- **Zwei Dateien, nicht eine (Projektentscheid, 27.11.2026).** Neben der Firmenvorgabe dieses Abschnitts schreibt das Paket
  `/etc/werkbank/vorgabe.json` (`root:root`, `0644`, Ordner `/etc/werkbank` `root:root` `0755`). Das ist eine Einstellungsdatei nach
  Vertrag 2 mit Routen und Netzzielen, eine Kopie des Erststart-Profils. `postinst` schreibt sie bei jedem `configure` neu, und `lauf`
  prüft die Routentabelle dagegen und verlangt einen root-eigenen Eigentümer (Modul `ziele::vorgabe` von `lauf`, Konstante
  `VORGABE_PFAD`). Die Firmenvorgabe dieses Abschnitts trägt die Kontenliste der Admins und `leseklassen` und liegt in
  `/etc/werkbank/vorgabe/firmenvorgabe.json` (`werkbank-vorgabe`, `0644`). Der Helfer schreibt nur diese Datei. `vorgabe.json` bleibt
  root-eigen; der Helfer öffnet sie nie, und `ReadWritePaths=` seiner Einheit nennt nur `/etc/werkbank/vorgabe` (Paket, Teil A3).
  Gründe: `postinst` würde eine Änderung eines Admins in `vorgabe.json` beim nächsten `configure` überschreiben, und `lauf` lehnte
  eine Datei des Helfernutzers ab. Wie die Optionen der Firma (Abschnitt 15.1.1) auf Felder von `vorgabe.json` wirken, ist OPEN 29.

#### 15.12.5 Fälle

Fälle in `faelle-schreiben/<name>.json` mit `erwartet-schreiben/<name>.json`. Die Referenz steht in `bau-faelle.py`
(`lauf_schreiben` für den Dienst, `helfer` für den Helfer), die Prüfung in `pruefen.py`. Die Crates `vorgabe-helfer` (Teil `helfer`) und
`regelebenen` (H3 und H4 der Bibliothek) lesen diesen Ordner; der Bau des Dienstes übernimmt den Teil `lauf_schreiben`.

```json
{"name": "…", "anforderungen": ["A05"], "beschreibung": "…",
 "firma": {"klassen": ["oeffentlich", "intern", "kundendaten"], "gruppen": {"g-eins": ["ps_…c01"]}},
 "konten": {"konto-admin": 1001, "konto-nutzer": 1003},
 "zuordnung": {"konto-admin": "ps_…b01", "konto-nutzer": "ps_…c01"},
 "firmenvorgabe": {"art": "datei", "eigentuemer": "werkbank-vorgabe", "modus": "0644",
                   "inhalt": {"admins": ["konto-admin"], "leseklassen": [{"ebene": "nutzer", "wer": "konto-nutzer", "klasse": "intern"}]}},
 "ordner": {"art": "ordner", "eigentuemer": "werkbank-vorgabe", "modus": "0755"},
 "eltern": {"art": "ordner", "eigentuemer": "root", "modus": "0755"},
 "schritte": [{"schreiben": {"partner": 1001, "wert": […], "fehler": ["rename"]}},
              {"anfrage": {"partner": 1003, "anfrage": {"art": "Ersetzen", …}, "fehler": []}},
              {"neuladen": {}},
              {"nachricht": {"partner": 1001}}]}
```

- `konten` steht für das System: Konto zu uid (`getpwnam_r`). `zuordnung` steht für die Stelle (Vertrag 9 Personenbezug): Konto zu
  Pseudonym. Konten sind Platzhalter der Form `konto-*`, uids Beispielzahlen.
- `partner` ist die uid aus `SO_PEERCRED`, `null` heißt: nicht feststellbar.
- `firmenvorgabe`, `ordner` und `eltern` sind der Zustand als Eingabe: `art` (`datei`, `ordner` oder `verweis`), Eigentümer und Modus.
  `eltern` steht für jeden Elternordner bis `/`.
- `schreiben` ist der Weg über die Ansicht und den Dienst, `anfrage` eine Anfrage direkt an den Socket, ohne Dienst. `neuladen` lädt
  im Dienst die Firmenvorgabe neu und hebt die Sperre auf.
- `fehler` schleust Fehler ein. Im Helfer: `eingeschlossen` (H2), `zu_gross` (H6), `frist` (H7), `temp`, `fsync`, `rename`,
  `ordner_fsync` (H11 und H12) und `rueckweg` (der Rückweg nach H12 scheitert). Nur im Dienst: `fremd_geaendert` (die geladenen
  Bytes sind veraltet, H9), `helfer_weg` (Stufe 3) und `antwort_fehlt` (der Helfer ersetzt, aber die Antwort kommt nicht an).
- Ergebnis je `schreiben`: `angenommen`, `code`, `zeile`, `datei` (Eigentümer, Modus, sha256 der Bytes und Inhalt danach),
  `unveraendert` (sha256 gleich wie vorher) und `temp_bleibt` (immer `false`). Die Bytes sind JSON mit sortierten Schlüsseln, Einzug 1
  und einem Zeilenende. Ergebnis je `anfrage`: `antwort`, `datei`, `unveraendert`, `temp_bleibt`. Ergebnis je `neuladen`: `geladen`
  (sha256) und `gesperrt`. Ergebnis je `nachricht`: die Nachricht `Einstellungen` an diesen Partner (Abschnitt 15.13).
- `pruefen.py` prüft: Paarung; die Zeile ohne Konto, mit dem Pseudonym des Partners; nach jedem Fehler dieselbe sha256 wie vorher
  (S12); keine ersetzte Datei für einen Partner, der in der gültigen Datei kein Admin ist (S11); keine Änderung außerhalb von
  `leseklassen` und nach dem Ersetzen Eigentümer `werkbank-vorgabe` mit `0644` (S13); die Sperre nach `zustand_unklar`; ein Konto in
  einer Nachricht nur auf dem einen Weg (S10); je Code mindestens einen Fall, je Weg einen Fall und je Art des Betrachters eine
  Nachricht.

| Fall | Inhalt |
|---|---|
| `schreiben-admin-erfolgreich` | Ein Admin schreibt aus der Ansicht; die Datei trägt die neue Liste mit Konten, Eigentümer `werkbank-vorgabe`, Modus `0644`. |
| `schreiben-nichtadmin` | Ein Nichtadmin schreibt aus der Ansicht, auch für sich selbst: `ebenen.nicht_befugt`, der Helfer wird nicht angesprochen. |
| `schreiben-datei-fremder-eigentuemer` | Eigentümer ist ein Nutzerkonto: `firmenvorgabe.datei_unsicher`. |
| `schreiben-datei-root-eigen` | Eigentümer ist root, etwa nach einer Änderung von Hand: `firmenvorgabe.datei_unsicher`. |
| `schreiben-datei-gruppe-schreibbar` | Modus `0664`: `firmenvorgabe.datei_unsicher`. |
| `schreiben-datei-andere-schreibbar` | Modus `0646`: `firmenvorgabe.datei_unsicher`. |
| `schreiben-datei-verweis` | Die Datei ist ein Verweis: `firmenvorgabe.datei_unsicher`. |
| `schreiben-ordner-schreibbar` | Der Ordner hat Modus `0777`: `firmenvorgabe.datei_unsicher`. |
| `schreiben-ordner-fremder-eigentuemer` | Der Ordner gehört einem Nutzerkonto: `firmenvorgabe.datei_unsicher`. |
| `schreiben-ordner-verweis` | Der Ordner ist ein Verweis: `firmenvorgabe.datei_unsicher`. |
| `schreiben-eltern-nicht-root` | Ein Elternordner gehört nicht root: `firmenvorgabe.datei_unsicher`. |
| `schreiben-eltern-schreibbar` | Ein Elternordner hat Modus `0775`: `firmenvorgabe.datei_unsicher`. |
| `schreiben-veraltet` | Root hat die Datei nach dem Laden geändert: `firmenvorgabe.veraltet`. |
| `schreiben-wert-ungueltig` | Ein Konto ohne Zuordnung und ein Pseudonym statt eines Kontos: `ebenen.wert_ungueltig`. |
| `schreiben-helfer-nicht-erreichbar` | `connect` scheitert: `firmenvorgabe.helfer_nicht_erreichbar`. |
| `schreiben-temp-scheitert` | `firmenvorgabe.temp_fehlgeschlagen`, Temp-Datei entfernt. |
| `schreiben-fsync-scheitert` | `firmenvorgabe.fsync_fehlgeschlagen`. |
| `schreiben-rename-scheitert` | `firmenvorgabe.rename_fehlgeschlagen`. |
| `schreiben-ordner-fsync-scheitert` | `firmenvorgabe.ordner_fsync_fehlgeschlagen`; der Rückweg gelingt, die Sicht zeigt die alte Liste. |
| `schreiben-rueckweg-scheitert` | `firmenvorgabe.zustand_unklar`; der neue Inhalt steht ganz, der Dienst sperrt. |
| `schreiben-antwort-fehlt` | Der Helfer ersetzt, die Antwort kommt nicht an: `firmenvorgabe.zustand_unklar`, Sperre. |
| `schreiben-sperre-bis-neuladen` | Nach `zustand_unklar` lehnt der Dienst ab, ohne den Helfer; nach dem Neuladen gelingt die Änderung. |
| `schreiben-nach-fehler-erfolgreich` | Nach einem gescheiterten `rename` gelingt der nächste Versuch eines anderen Admins. |
| `socket-admin-erfolgreich` | Ein Prozess eines Admins ersetzt die Datei direkt über den Socket. |
| `socket-nichtadmin` | Ein Nichtadmin direkt über den Socket: `firmenvorgabe.partner_nicht_admin`. |
| `socket-partner-unbekannt` | Partner nicht feststellbar und uid ohne Konto: `firmenvorgabe.partner_unbekannt`. |
| `socket-admin-nur-in-neuer-datei` | Ein Nutzer reicht eine Datei ein, die ihn als Admin nennt: `firmenvorgabe.partner_nicht_admin`. |
| `socket-admins-geaendert` | Ein Admin ändert die Kontenliste oder fügt einen Schlüssel hinzu: `firmenvorgabe.ausser_leseklassen`. |
| `socket-eingeschlossen` | Ein Prozess eines Admins in einem Lauf: `firmenvorgabe.partner_eingeschlossen`. |
| `socket-anfrage-zu-gross` | `firmenvorgabe.anfrage_zu_gross`. |
| `socket-anfrage-frist` | `firmenvorgabe.anfrage_frist`. |
| `socket-anfrage-ungueltig` | Basis keine Prüfsumme, Inhalt keine Firmenvorgabe, fremdes Feld, falsche Art: `firmenvorgabe.anfrage_ungueltig`. |
| `socket-veraltet` | Die zweite Anfrage nennt die Basis vor der ersten: `firmenvorgabe.veraltet`. |
| `socket-ladefehler` | Die gültige Datei hat keine lesbare Kontenliste: `firmenvorgabe.ladefehler`. |

### 15.13 Konto in der Ansicht der Admins (Entscheid des Betreibers, 27.11.2026; schließt OPEN 23)

Admins sehen in der Ansicht das Konto; ins Protokoll kommt nur das Pseudonym. `sicht` liefert weiter nur Pseudonyme (S10). Die
Ersetzung ist eine Pflicht des Dienstes, kein Ergebnis von `sicht`:

- Der Dienst baut die Nachricht `Einstellungen` aus `sicht(stand, wer)`. Ist `wer` Admin (`stand.admins`), setzt er in `werte` an
  jede Stelle eines Pseudonyms das Konto aus seiner Zuordnung. Im Pilot sind das nur die Einträge von `leseklassen` mit
  `ebene: nutzer`. Ein Pseudonym ohne Konto in der Zuordnung bleibt stehen.
- Für einen Betrachter, der kein Admin ist, ersetzt der Dienst nie etwas. Er sieht seinen eigenen Eintrag mit seinem Pseudonym.
- Die Liste der Admins steht in keiner Nachricht, auch nicht in der an einen Admin.
- Ein Admin gibt in der Ansicht Konten ein. Der Dienst ordnet sie vor `setzen` zu (Abschnitt 15.2, Stufe 0 in Abschnitt 15.12).
- Zeile, Prüfsummen, Stand, `setzen` und `sicht` nennen nie ein Konto.

Beispiel: Die Firmenvorgabe enthält `{"ebene": "nutzer", "wer": "konto-nutzer", "klasse": "oeffentlich"}`, die Zuordnung
`konto-nutzer` zu `ps_00000000000000000000000000000c01`. `sicht` liefert für jeden Betrachter
`{"ebene": "nutzer", "wer": "ps_00000000000000000000000000000c01", "klasse": "oeffentlich"}`. Der Admin `konto-admin` erhält in
`Einstellungen` `"wer": "konto-nutzer"`. Der Nutzer `konto-nutzer` erhält `"wer": "ps_00000000000000000000000000000c01"`.

| Fall in `faelle-schreiben/` | Inhalt |
|---|---|
| `konto-admin-sieht` | Ein Admin erhält in `Einstellungen` das Konto an der Stelle des Pseudonyms, `admin: true`. |
| `konto-nutzer-sieht-keins` | Zwei Nutzer erhalten ihren Eintrag mit Pseudonym oder eine leere Liste, `admin: false`. |
| `konto-zeile-ohne-konto` | Ein Admin schreibt eine Liste mit Konten; `handelnd`, `alt` und `neu` der Zeile nennen nur Pseudonyme. |

<!-- vom Bauvorgang angefuegt -->
> Hinweis: Dieser Text nennt Werkzeuge, die in diesem Klon nicht liegen: `pi`.
> Sie gehoeren zum Setup der Maschine, aus der dieser Baum stammt; hier fehlen
> sie mit Absicht. Wer sie braucht, schreibt sich seine eigenen.
> *Named here, not shipped here.*
