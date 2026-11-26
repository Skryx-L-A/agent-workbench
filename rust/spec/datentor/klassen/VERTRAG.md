# Teilvertrag `datentor klassen`: Klassen-Tiefe

Stand 26.11.2026 (Nachtrag 26.11.2026 in Abschnitt 8a). Teilvertrag zu Vertrag 3 (`rust/spec/VERTRAEGE.md`) für die Vererbung nach Ort oder Projekt (A32), das Aufteilen (A33) und das Herabstufen durch die Automatik (A39, A40, A41), mit den Protokollangaben einer Senkung (A39, A104).
„(V)“ heißt: in diesem Vertrag festgelegt. „(H)“ heißt: aus den Anforderungen hergeleitet. Die Fragen F1 bis F8 sind entschieden (Abschnitt 8, Nachtrag 20.11.2026).

Dieser Stand enthält nur den Vertrag und die sichtbaren Fälle. Es gibt noch kein Programm; ohne Programm sind alle Fälle rot. Verdeckte Fälle und Mutanten folgen mit dem Bau.

## 1. Schnitt

- Neu ist eine reine Funktion der Bibliothek `datentor`, `wirksame_klasse` (Abschnitt 10). Sie rechnet aus eigener Klasse, geerbter Klasse, Art der Einheit, Befunden des Filterkerns und den Optionen die Klasse, die danach an `darf` geht.
- `darf`, `wirksam`, `Vorgabe::neu` und alle Fälle von Vertrag 3 bleiben unverändert. `datentor` ohne Unterbefehl bleibt unverändert.
- Das Programm bekommt den Unterbefehl `datentor klassen` mit eigenem Fallformat (Abschnitt 6).
- Nicht in diesem Vertrag: das Erbe einer geschriebenen, abgeleiteten Einheit von ihren Quellen (`rust/spec/klassenerbe/`), die Leseklasse der Person (A05), Rückholen, Kette und Schlüssel des Protokolls (A104, Vertrag 5), das Erkennen der Art aus Typ und Ordner.
- Das Tor bleibt ohne Ein- und Ausgabe, ohne Uhr, ohne Zufall und ohne Schlüssel. Zeit, Lauf, Pseudonym der Einheit und `inhalt_mac` der Protokollzeile setzt der Aufrufer (Abschnitt 5).

## 2. Optionen: Feld `tiefe`

Die Optionen stehen im Fall neben der `vorgabe` von Vertrag 3 im Feld `tiefe`. Im Betrieb kommen sie aus der Einstellungsdatei (Vertrag 2, spätere Fassung mit Regelebenen).

```json
{"aufteilen": true,
 "herabstufen": {"an": false,
                 "arten": ["regel", "bedienwissen", "oeffentliche-quelle", "leere-vorlage"],
                 "gruppen": [{"schluessel": "g-a", "streicht": ["regel"]}]}}
```

| Feld | Typ | Fehlt | Bedeutung | Herkunft |
|---|---|---|---|---|
| `aufteilen` | Wahrheitswert | `true` | Option „Aufteilen“: Eine Einheit mit Teilen bekommt je Teil eine Klasse. | A33 (F2) |
| `herabstufen.an` | Wahrheitswert | `false` | Schalter „Herabstufen“: Die Automatik darf senken. | A39 (F1) |
| `herabstufen.arten` | Liste von Arten | die vier Arten aus A39 | Liste „Herabstufbare Arten“ der Firma; sie gilt erst, wenn der Schalter an ist. | A39 (F1, F3) |
| `herabstufen.gruppen` | Liste | leer | je Gruppe `schluessel` und `streicht` (Liste von Arten, fehlt = leer) | A41 |

Nachtrag 20.11.2026 (F1, F2, F3): Schalter und Liste sind zwei Optionen. Die Admins dürfen eigene Arten hinzufügen, nie aber eine Art der Nie-Liste.

- **Arten** sind Schlüssel in der Schlüsselform von Vertrag 3. Die Vorgabe der Liste sind die vier Arten aus A39: `regel` (Regeln, Rollen, Skills und Arbeitsanweisungen), `bedienwissen` (Bedien- und Fehlerwissen zu allgemein verfügbarer Software), `oeffentliche-quelle` (Auszüge aus öffentlichen Quellen mit Quellenangabe) und `leere-vorlage` (leere Vorlagen). Eine Art, die nicht in der wirksamen Liste steht, sinkt nie.
- **Nie-Liste (A40), fest und nicht einstellbar:** `personenbezug`, `kundendaten`, `geheimnis`, `zugangsdaten`, `vertrag`, `preis`, `finanzen`, `netz-und-system`, `gespraechsprotokoll`, `sitzungsprotokoll`, `entscheidung`, `plan`, `organisation`, `quelltext`, `entwurf`. Dazu sinkt nie eine Art, die nicht sicher erkannt ist (`art: null`, H5).
- **Nie-Liste, Nachtrag 26.11.2026:** `angebot`, `schaetzung` und `kalkulation` (Angebote, Aufwandsschätzungen und Kalkulationen, auch ohne Preise), `ticket`, `testfall` und `fehlerbericht` (aus Kundenprojekten, auch mit reinem Bedienwissen), `sicherheitsbefund` (Schwachstellen, Prüfberichte, Hinweise auf den Ort eines Schlüssels oder Zugangs), `konfiguration` und `lizenz` (Konfigurationen und Lizenzangaben von Firmen- und Kundensystemen; Zugangsangaben sind `zugangsdaten`), `kundenbezug` (indirekter Kundenbezug: Projektkürzel, Produktnamen eines Kunden, fachliche Abläufe eines Kunden) und `vertragsregel` (Inhalte, für die ein Kundenvertrag eigene Regeln hat; sie folgen nur der Kundenachse, nie der Automatik). Für sie gilt T1 wie für die übrigen Nie-Arten.
- **T1, Ladefehler (fail closed, wie A47):** Nennt `arten` oder `streicht` eine Art der Nie-Liste, ist `tiefe` ungültig und der Fall endet mit Exit 2. Das gilt auch bei Schalter aus. Ebenso ungültig ist eine Art, die nicht die Schlüsselform hat, oder eine Art doppelt in `arten`.
- **Vergleich mit der Nie-Liste byte-genau wie D14** (Nachtrag 20.11.2026, 2): T1 vergleicht jede Art nach dem Dekodieren von JSON byte-genau mit den Schlüsseln der Nie-Liste. Abwandlungen wie `vertraege`, `vertrag.` oder `kunden-daten` sind andere Arten und bestehen T1. Die Einordnung (Typ und Ordner, Abschnitt 7) vergibt nur Arten aus ihrem eigenen Wortschatz; eine eigene Art der Admins wirkt also nur auf Einheiten, die die Einordnung mit genau diesem Schlüssel benennt. `\u0076ertrag` ist nach dem Dekodieren `vertrag` und ein Ladefehler.
- `arten` darf leer sein; dann sinkt nichts.
- Eine Gruppe kann nur streichen (A41). Ein Feld zum Hinzufügen oder eine eigene Liste `arten` der Gruppe gibt es nicht; ein solches Feld ist unbekannt und macht den Fall ungültig. In `streicht` ändert eine Art doppelt nichts. Streicht eine Gruppe eine Art, die die Firma nicht erlaubt, fügt das nichts hinzu.
- Gruppenschlüssel haben die Schlüsselform von Vertrag 3 und kommen nicht doppelt vor.
- `null` ist für kein Feld von `tiefe` ein gültiger Wert. Fehlt `tiefe`, gelten alle Vorgaben der Tabelle.

## 3. Einheit

Eine Einheit ohne Teile:

```json
{"klasse": null, "geerbt": "intern", "kunde": null, "art": "regel",
 "befund": {"geheimnis": false, "personenbezug": false, "interner_name": false,
            "host": false, "pfad": false, "kundenkennung": false, "konfiguration": false, "zweifel": false},
 "modell": "frei", "gruppe": "g-a", "senkung_nutzer": "folgt", "bestaetigt": false}
```

Die Felder teilen sich in die **Lage** der ganzen Einheit und den **Inhalt**. Mit Teilen trägt die Einheit nur die Lage und `teile`; jeder Teil trägt nur Inhalt.

| Feld | Teil von | Werte | Fehlt | Bedeutung |
|---|---|---|---|---|
| `geerbt` | Lage | beliebiger JSON-Wert | `null` | geerbte Klasse von Ort oder Projekt (A32) |
| `gruppe` | Lage | Text oder `null` | `null` | Gruppe des Besitzers (A41) |
| `senkung_nutzer` | Lage | `folgt`, `aus`, `bestaetigung` | `folgt` | Einstellung des Nutzers für seine Einheiten (A41) |
| `bestaetigt` | Lage | Wahrheitswert | `false` | Der Nutzer hat diese Senkung bestätigt. |
| `teile` | Lage | Liste von 1 bis 64 Teilen | keine Teile | Aufteilen (A33) |
| `klasse` | Inhalt | beliebiger JSON-Wert | `null` | eigene Klasse, von einem Menschen oder einer Admin-Regel gesetzt |
| `kunde` | Inhalt | Text oder `null` | `null` | Kunde wie in Vertrag 3 |
| `art` | Inhalt | Text oder `null` | `null` | Art nach Abschnitt 2; `null` heißt: nicht sicher erkannt |
| `befund` | Inhalt | Objekt mit den drei Pflichtwerten `geheimnis`, `personenbezug`, `interner_name` und den fünf Werten `host`, `pfad`, `kundenkennung`, `konfiguration`, `zweifel` (Nachtrag 26.11.2026), alle Wahrheitswerte; oder `null` | `null` | Befunde des Filterkerns (Abschnitt 7); `null` heißt: kein Befund vorhanden. Fehlt einer der fünf Werte, gilt er als `true`: eine Prüfung, die nicht lief, ist ein Treffer. `null` als Wert ist ungültig. |
| `modell` | Inhalt | `frei`, `verhindert`, `senken` oder `null` | `null` | Urteil eines lokalen Modells; `null` heißt: kein Modell gelaufen |

- `klasse` und `geerbt` dürfen jeden JSON-Wert haben. Ein Wert außer `null`, der kein Text in `klassen` ist, macht den Fall nicht ungültig; er ergibt die Herkunft `ungueltig` (K1), wie D4 in Vertrag 3.
- Jeder andere falsche Typ oder Wert, ein unbekanntes Feld, ein Teil mit einem Feld der Lage oder mit `teile`, und eine Einheit mit `teile` und einem Feld des Inhalts machen den Fall ungültig.
- Vergleiche sind byte-genau wie D14: `Regel` ist nicht `regel`, `G-A` ist nicht `g-a`.

## 4. Regeln

Je Einheit (oder je Teil) laufen K1 bis K3 und K5, dann K2 und zuletzt `darf` (K6). K4 regelt Einheiten mit Teilen.

| Regel | Inhalt | Herkunft |
|---|---|---|
| K1 | **Grundklasse.** Sind `klasse` oder `geerbt` angegeben, aber kein Text in `klassen`, ist die Einheit ungültig: Klasse `null`, Herkunft `ungueltig`, Senkung `klasse-ungueltig`. Das gilt auch, wenn der andere Wert gültig ist (fail closed). Sonst gilt die eigene Klasse (Herkunft `eigen`), sonst die geerbte (`geerbt`), sonst die Vorgabestufe (`vorgabestufe`). Eine eigene Klasse geht der geerbten vor, auch wenn sie tiefer liegt. | A32, A35 |
| K2 | **Kunde hebt.** Hat der Inhalt einen Kunden, ist die Klasse die höchste und die Herkunft `kunde`. Das gilt nach jeder Senkung; ein leerer Text ist ein Kunde. | D5 |
| K3 | **Herabstufen.** Die Regeln H1 bis H11 laufen in der Reihenfolge der Tabelle unten (H9b direkt nach H9); die erste zutreffende bestimmt den Grund der Senkung. Trifft keine zu, gilt H0: Die Klasse sinkt um genau eine Stufe unter die geerbte, Herkunft `gesenkt`. | A39, A40, A41 |
| K4 | **Aufteilen.** Siehe unten. | A33 |
| K5 | **Protokollangaben** nur bei H0, siehe Abschnitt 5. | A39, A104 |
| K6 | **Übergabe an `darf`.** Die Klasse aus K1 bis K4 und der Kunde gehen an `darf` aus Vertrag 3, mit der Route der Anfrage. Ist die Klasse `null`, geht ein leerer Text als Klasse an `darf`; `darf` sperrt dann nach seiner Reihenfolge, spätestens mit `einheit-ohne-klasse`. | Vertrag 3 |

| Regel | Code | Wann | Herkunft |
|---|---|---|---|
| H1 | `aus` | Der Schalter `herabstufen.an` ist `false`; die Liste wirkt dann nicht. | A39 (F1) |
| H2 | `nicht-geerbt` | Die Herkunft nach K1 ist nicht `geerbt`. Die Automatik senkt nur unter die geerbte Klasse; eine eigene Klasse und die Vorgabestufe bleiben. | A39 |
| H3 | `kundendaten` | Der Inhalt hat einen Kunden, oder die geerbte Klasse ist die höchste. | A40 |
| H4 | `niedrigste` | Die geerbte Klasse ist die niedrigste. | (V) |
| H5 | `art-unbekannt` | `art` ist `null`. | A40, „nicht sicher erkannt“ |
| H6 | `gruppe-ohne-eintrag` | `gruppe` ist ein Text, der in `gruppen` nicht steht (fail closed; bestätigt, F7). | A41 |
| H7 | `art-nicht-herabstufbar` | `art` steht nicht in `arten`, oder die Gruppe streicht sie. | A39, A40, A41 |
| H8 | `befund` | `befund` ist `null`, oder einer der acht Werte ist `true` oder fehlt (Nachtrag 26.11.2026). | A40 |
| H9 | `modell-verhindert` | `modell` ist `verhindert`. Die Werte `frei` und `senken` lösen nichts aus. | A40 |
| H9b | `modell-fehlt` | `art` ist `oeffentliche-quelle` oder `leere-vorlage`, und `modell` ist `null` (Nachtrag 26.11.2026). | A40, „bei Zweifel keine Senkung“ |
| H10 | `nutzer-aus` | `senkung_nutzer` ist `aus`. | A41 |
| H11 | `bestaetigung-fehlt` | `senkung_nutzer` ist `bestaetigung` und `bestaetigt` ist nicht `true`. Die Antwort nennt die Klasse nach der Senkung als `vorschlag`. | A41 |
| H0 | `gesenkt` | sonst | A39 |

- **Genau eine Stufe:** H0 rechnet immer von der geerbten Klasse aus. Die Funktion kennt keinen früheren Lauf. Derselbe Aufruf gibt immer dieselbe Klasse; tiefer als die geerbte minus eins geht es nie (A40, „je Lauf höchstens eine Stufe“). Speichert der Aufrufer das Ergebnis, speichert er es nie als eigene Klasse (Abschnitt 7).
- **Klasse `ungueltig`** geht jeder Regel H vor, auch H1.

**K4, Aufteilen.** Eine Einheit mit `teile` wird so gerechnet:

- `aufteilen` ist `true`: Jeder Teil wird wie eine Einheit gerechnet, mit der Lage der ganzen Einheit und dem eigenen Inhalt. Die Antwort trägt je Teil eine Antwort, dazu `ausgeliefert` (die Nummern der Teile mit `darf: true`, ab 0, aufsteigend) und `gesperrt` (die Zahl der übrigen Teile). Die Suche liefert genau die Teile aus `ausgeliefert` aus, in dieser Reihenfolge.
- `aufteilen` ist `false`: Die Einheit bleibt ganz. Ist ein Teil nach K1 ungültig, ist die ganze Einheit ungültig. Sonst ist ihre Klasse die höchste Klasse ihrer Teile nach K1 und K2; Herkunft `teile`, Senkung `nicht-aufgeteilt`. Es wird nichts gesenkt, weil eine gemischte Einheit erst nach dem Aufteilen eingestuft wird (Plan, Entscheidung 7). Der Kunde der ganzen Einheit ist der Kunde der Teile, wenn alle Teile mit Kunde denselben nennen; nennen sie verschiedene, ist er ein leerer Text. Damit sperrt `darf` sie auf jeder Route (`kunde-ohne-eintrag`).
- Eine Einheit ohne `teile` wird in beiden Fällen gleich gerechnet.

## 5. Antwort

Eine Einheit ohne Teile, oder mit Teilen und `aufteilen: false`:

```json
{"klasse": "oeffentlich", "herkunft": "gesenkt",
 "senkung": {"grund": "gesenkt", "vorschlag": null},
 "protokoll": {"art": "regel", "erkannt_durch": ["typ:regel", "filter:frei", "modell:frei"],
               "regel_ebene": "firma", "klasse_alt": "intern", "klasse_neu": "oeffentlich"},
 "darf": {"darf": true, "grund": "frei", "klasse": "oeffentlich", "meldung": "frei"}}
```

Mit Teilen und `aufteilen: true`: `{"teile": [<Antwort je Teil>], "ausgeliefert": [0], "gesperrt": 1}`.

- `klasse`: die Klasse, die an `darf` geht, als Schlüssel, oder `null` bei Herkunft `ungueltig`.
- `herkunft`: `eigen`, `geerbt`, `vorgabestufe`, `gesenkt`, `kunde`, `teile` oder `ungueltig`. A32 verlangt die Herkunft der Klasse in der Abfrage.
- `senkung.grund`: ein Code aus H0 bis H11 und H9b, `klasse-ungueltig` oder `nicht-aufgeteilt`. Er steht auch, wenn K2 die Klasse danach hebt.
- `senkung.vorschlag`: nur bei `bestaetigung-fehlt` die Klasse eine Stufe unter der geerbten, sonst `null`.
- `protokoll`: nur bei `gesenkt`, sonst `null`. Felder (A39, A104):
  - `art`: die Art der Einheit;
  - `erkannt_durch`: `typ:<art>` und `filter:frei`, dazu `modell:frei`, wenn `modell` `frei` oder `senken` ist; feste Reihenfolge;
  - `regel_ebene`: `gruppe`, wenn die Einheit eine Gruppe hat, sonst `firma`;
  - `klasse_alt` und `klasse_neu`: die geerbte und die gesenkte Klasse.
  Die Angaben nennen keinen Pfad, keinen Titel, keinen Namen und keinen Kunden. Vertrag 5 ergänzt `zeit`, `lauf`, `einheit` (Pseudonym), `inhalt_mac`, `regel`, `regel_stand` und `ordner:ok` in `erkannt_durch`. Das Tor kennt dafür weder Uhr noch Schlüssel noch Ordner.
- `darf`: die Antwort von `darf` nach Vertrag 3, Abschnitt 3, unverändert in Form und Meldung.
- Der Aufrufer gibt dem Modell auch hier nur die Zahl der gesperrten Einheiten oder Teile weiter, nie `herkunft`, `senkung`, `protokoll` oder Felder aus `darf` (A49).

## 6. Aufruf `datentor klassen`

- `datentor klassen < fall.json` (oder `datentor klassen -`). Grenzen wie Vertrag 3, Abschnitt 5: höchstens 1 048 576 Byte, Verschachtelung höchstens 64 Ebenen, das oberste Objekt ist Ebene 1.
- Exitcode 0: auf stdout genau ein JSON-Objekt `{"antworten": [...]}`, eine Antwort je Anfrage in derselben Reihenfolge.
- Exitcode 2: Der Fall ist ungültig. stdout bleibt leer, auf stderr steht genau eine Zeile.
- Ungültig ist ein Fall bei jedem Grund aus Vertrag 3, Abschnitt 5, für Eingabe, Größe, Tiefe und `vorgabe`. Ein doppelter Schlüssel in einem JSON-Objekt auf irgendeiner Ebene macht ihn ungültig; Doppel zählen nach dem Dekodieren, `"an"` und `"an"` sind derselbe Schlüssel (A139).
- Ungültig ist er auch bei einem Verstoß gegen Abschnitt 2 oder 3, bei einem Feld im Fall außer `name`, `regeln`, `beschreibung`, `vorgabe`, `tiefe` und `anfragen`, und bei einer Anfrage mit einem Feld außer `einheit` und `route`. Der Kunde steht in der Einheit, nicht in der Anfrage.
- `route` ist Text oder `null`; fehlt sie, gilt `null`.
- Keine Eingabe löst eine Panik aus.

```json
{"name": "…", "regeln": ["K3"], "beschreibung": "…",
 "vorgabe": {…}, "tiefe": {"herabstufen": {"an": true}},
 "anfragen": [{"einheit": {"geerbt": "intern", "art": "regel", "befund": {…}}, "route": "r-oeff"}]}
```

## 7. Nahtstelle Vertrag 4

Der Einbettungslauf ruft `wirksame_klasse`; die Suche ruft nur `darf` mit der gespeicherten Klasse (Nachtrag 20.11.2026, F4). Der Lauf speichert Klasse und Herkunft getrennt von `datenklasse` und schreibt bei `gesenkt` die Protokollzeile der Senkung. Ist die Notiz seit dem Lauf geändert, gilt die Klasse als unklar, und die Notiz ist bis zur Neubewertung gesperrt (Vertrag 4).

Was der Einbettungslauf je Einheit liefern muss, damit `wirksame_klasse` rechnen kann:

| Feld | Quelle | Wenn nicht lesbar | Heute in Vertrag 4 |
|---|---|---|---|
| `klasse` | `datenklasse` im Frontmatter, nur von Mensch oder Admin-Regel gesetzt, nie vom Ergebnis einer Senkung | Wert, der kein Text ist, durchreichen (ergibt `ungueltig`) | ja |
| `geerbt` | Klasse des Ordners oder Projekts (A32) | ebenso | nein |
| `kunde` | `kunde` im Frontmatter | kein Text → leerer Text, wie Vertrag 4 heute | ja |
| `art` | Art aus Typ und Ordner, nur wenn beide zusammenpassen; Typ nur vom Anlegebefehl (Plan, Entscheidung 7) | `null` | nein |
| `befund` | Filterkern beim Einbetten, je Teil; die acht Werte nach Abschnitt 8a | `null` | nein |
| `modell` | Urteil des lokalen Modells, falls gelaufen | `null` | nein |
| `gruppe` | Gruppe des Besitzers aus den Regelebenen (Vertrag 2, spätere Fassung) | Text, der in `gruppen` nicht steht | nein |
| `senkung_nutzer`, `bestaetigt` | Einstellung des Besitzers und gespeicherte Bestätigung | `bestaetigung` und `false` | nein |
| `teile` | Grenzen der Teile beim Einbetten, je Teil der Inhalt | keine Teile, Einheit bleibt ganz | nein |

Für die Ausgabe gilt: Mit `aufteilen: true` liefert die Suche je Einheit nur die Teile aus `ausgeliefert`, in dieser Reihenfolge. Ein gesperrter Teil liefert nichts, auch keinen Titel und keinen Pfad der Einheit. `gesperrt` in Vertrag 4 zählt Teile, in derselben Einheit wie die Treffer. Die Protokollzeile „Ausgabe gesperrt“ zählt Teile je Gruppe aus Klasse, Kunde und Grund (Nachtrag 20.11.2026, F5).

## 8. Entschiedene Fragen (Nachtrag 20.11.2026)

Die Fragen F1 bis F3 und F7 entschied das Projekt am 20.11.2026, die Fragen F4 bis F6 und F8 beantwortete das Projekt am 20.11.2026.

- **F1, Vorgabe Herabstufen:** Es gibt zwei Optionen. Der Schalter „Herabstufen“ hat die Vorgabe aus. Die Liste „Herabstufbare Arten“ hat als Vorgabe die vier Arten aus A39 und gilt erst, wenn der Schalter an ist. A39 ist entsprechend ergänzt. Fälle: `k3-vorgabe-aus`, `k3-explizit-aus`, `k3-schalter-aus-liste`, `k3-gesenkt`.
- **F2, Vorgabe Aufteilen:** Die Vorgabe ist an (A33, bestätigt 18.11.). Die Regel vom 19.11. „eine Vorgabe ist nie an“ gilt nur für neue Optionen. Fälle: `k4-aufteilen` (Vorgabe), `k4-aufteilen-ausdruecklich-an`, `k4-aufteilen-aus`.
- **F3, Arten erweitern:** Admins dürfen eigene herabstufbare Arten hinzufügen, nie aber eine Art aus A40. Die Nie-Liste steht fest in Abschnitt 2 und ist nicht einstellbar. Eine Nie-Art in der Liste der Admins ist ein Ladefehler (T1, Exit 2). Gruppen dürfen weiter nur streichen. Fälle: `k3-eigene-art`, `u-k-arten-nie-<art>` je Nie-Art, `u-k-arten-nie-schalter-aus`, `u-k-streicht-nie`, `u-k-gruppe-fuegt-hinzu`, `u-k-gruppe-arten`.
- **F4, Wer ruft wann:** Der Einbettungslauf ruft `wirksame_klasse`, speichert Klasse und Herkunft getrennt von `datenklasse` und schreibt die Protokollzeile der Senkung. Die Suche ruft nur `darf`. Ist die Notiz seit dem Lauf geändert, gilt die Klasse als unklar; die Notiz ist bis zur Neubewertung gesperrt (Vertrag 4). Siehe Abschnitt 7.
- **F5, Zählen von Teilen:** `gesperrt` zählt Teile, in derselben Einheit wie die Treffer. Die Protokollzeile zählt Teile je Gruppe aus Klasse, Kunde und Grund. Siehe Abschnitt 7.
- **F6, Abgeleitete Einheiten:** Beim Schreibtor gilt das Maximum der Quellen als `geerbt` im Sinn von H2. Das Schreibtor übergibt es in `Lage::geerbt`.
- **F7, Gruppe ohne Eintrag:** H6 bleibt. Eine Gruppe ohne Eintrag sperrt die Senkung (fail closed).
- **F8, Mehrere Kunden ohne Aufteilen:** Ja. Ohne Aufteilen ergeben zwei Kunden einen leeren Kunden, und die Einheit ist überall gesperrt (K4). Fall: `k4-aufteilen-aus`.

## 8a. Nachtrag 26.11.2026: Art und Inhalt müssen passen

Die Firma beantwortete Plan 10.7 am 26.11.2026. Die vier Arten allein reichen nicht; es kommt auf den Inhalt an. Der Nachtrag verschärft nur. Er macht keine Senkung möglich, die vorher nicht möglich war.

- **Regel:** Eine Einheit sinkt nur, wenn ihre Art herabstufbar ist (H7) **und** der Filterkern ihren Inhalt geprüft hat und nichts findet (H8). Bei Treffer oder Zweifel sinkt sie nicht. Eine unklare Einstufung bleibt gesperrt (H5, H6). Gruppen und Nutzer dürfen nur verschärfen (A41, unverändert).
- **Nie-Liste:** elf Arten mehr, Abschnitt 2.
- **Befund des Filterkerns:** fünf Werte mehr (Abschnitt 3). Der Filterkern setzt sie je Teil:
  - `host`: ein Rechnername, eine Adresse oder ein Dienstname eines Firmen- oder Kundensystems;
  - `pfad`: ein Pfad eines Firmen- oder Kundensystems;
  - `kundenkennung`: ein Projektkürzel, eine Kundennummer oder ein Produktname eines Kunden, aus der Admin-Liste wie die internen Namen (A37);
  - `konfiguration`: ein Konfigurationswert, eine Version, eine Lizenz- oder eine Zugangsangabe eines Systems;
  - `zweifel`: Der Filterkern konnte den Inhalt nicht ganz prüfen, etwa wegen eines unbekannten Formats oder einer Grenze.
  Ein Filterkern, der nur die drei Werte vom 20.11. liefert, lässt nichts mehr sinken (Fall `k3-befund-inhalt`).
- **H9b:** Für `oeffentliche-quelle` und `leere-vorlage` braucht die Senkung zusätzlich das Urteil eines lokalen Modells (`frei` oder `senken`). Der Filterkern sieht nicht, ob eine Zusammenfassung öffentlicher Quellen eigene Bewertungen, Pläne oder Entscheidungen enthält, oder ob die Felder einer Vorlage interne Abläufe zeigen. Ohne Modell bleibt dieser Zweifel; dann sinkt nichts. Das Modell darf weiter nur verhindern, nie auslösen (H9).
- **Die Beispiele der Firma und die Regel, die sie sperrt:**

| Beispiel der Firma | sperrt |
|---|---|
| Regel, Rolle oder Skill, der interne Systeme beschreibt (Projektschlüssel, Hostnamen) | H8 über `kundenkennung`, `host`, `interner_name` |
| Projektregeln eines Kundenprojekts | H3 (Kunde), sonst H8 über `kundenkennung` |
| Bedien- und Fehlerwissen mit eigenen Pfaden, Hosts, Versionen oder Konfigurationen | H8 über `pfad`, `host`, `konfiguration` |
| leere Vorlage eines Kunden | H3 (Kunde), sonst H9b |
| leere Vorlage, deren Felder interne Abläufe zeigen | H9b, das Modell entscheidet (`verhindert`) |
| Zusammenfassung öffentlicher Quellen mit eigenen Bewertungen, Plänen oder Entscheidungen | H9b; mit Aufteilen sind Plan und Entscheidung eigene Teile der Nie-Liste |

- **Grenze:** Feldnamen eines internen Ticketsystems erkennt der Filterkern nur, wenn sie in der Admin-Liste stehen. Ohne Eintrag sperrt sie nur ein Modell mit `verhindert`.
- **Für die Anleitung:** Der Rückholbefehl nimmt die Einstufung zurück; schon ausgelieferte Inhalte holt er nicht zurück.
- Fälle: `k3-nie-arten-firma`, `k3-befund-inhalt`, `k3-modell-pflicht`, `u-k-arten-nie-<art>` je neuer Nie-Art, `u-k-befund-neu-text`, `u-k-befund-neu-null`, `u-k-befund-nur-neu`. Geändert: In `k3-gesenkt` sinken `oeffentliche-quelle` und `leere-vorlage` nur noch mit `modell: frei`.

## 9. Fälle und Läufer

| Pfad | Zweck |
|---|---|
| `faelle_bauen.py` | schreibt `sichtbar/faelle/` und `sichtbar/erwartet/` aus Tabellen mit ausgeschriebenen Erwartungen (Klasse, Herkunft, Grund der Senkung, Grund von `darf`); prüft jede ganze Antwort gegen eine kleine Referenz dieses Vertrags und jeden Fall gegen die Gültigkeitsregeln; die Regeln von Vertrag 3 nimmt es aus `../faelle_bauen.py` |
| `aufruf.sh` | Hülle für den Läufer: ruft `datentor klassen`; `DATENTOR` nennt das Programm |
| `sichtbar/` | die sichtbaren Fälle |

Stand nach dem Nachtrag 26.11.2026: 118 Fälle, davon 33 gültige mit 178 Anfragen und 85 ungültige.

```
python3 -I -B faelle_bauen.py
DT_BIN=klassen/aufruf.sh DATENTOR=<programm> DT_FAELLE=klassen/sichtbar python3 -I -B aussen_laeufer.py [name ...]
```

Der Läufer ist `../aussen_laeufer.py`; die zweite Zeile läuft im Ordner `rust/spec/datentor/`.

## 10. Vorgabe für den Bau (nicht durch Außentests geprüft)

Bestehend und unverändert (abgestimmt): `Vorgabe::neu`, `darf`, `wirksam(&Vorgabe, &Einheit, kunde: Option<&str>) -> Result<&str, Grund>` (D5; einziger Fehler `Grund::EinheitOhneKlasse`) und `Vorgabe::rang(&str) -> Option<usize>`.

```rust
pub struct Tiefe { /* privat */ }
impl Tiefe {
    pub fn neu(aufteilen: bool, herabstufen: bool, arten: Vec<String>, gruppen: Vec<Gruppe>)
        -> Result<Tiefe, TiefeFehler>;              // Abschnitt 2 mit T1; Fehler nennt die erste Verletzung
}
impl Default for Tiefe { /* aufteilen an, herabstufen aus, alle vier Arten, keine Gruppe */ }
pub struct Gruppe { pub schluessel: String, pub streicht: Vec<String> }
pub enum Angabe<'a> { Keine, Text(&'a str), KeinText }
pub struct Befund { pub geheimnis: bool, pub personenbezug: bool, pub interner_name: bool,
                    pub host: bool, pub pfad: bool, pub kundenkennung: bool, pub konfiguration: bool, pub zweifel: bool }
impl Befund { pub fn frei(&self) -> bool; }   // alle acht false; eine nicht gelaufene Prüfung setzt der Aufrufer auf true
pub const NUR_MIT_MODELL: [&str; 2];           // H9b: oeffentliche-quelle, leere-vorlage
pub enum Modell { Frei, Verhindert, Senken }
pub enum NutzerSenkung { Folgt, Aus, Bestaetigung }
pub struct Lage<'a> { pub geerbt: Angabe<'a>, pub gruppe: Option<&'a str>, pub nutzer: NutzerSenkung, pub bestaetigt: bool }
pub struct Inhalt<'a> { pub klasse: Angabe<'a>, pub kunde: Option<&'a str>, pub art: Option<&'a str>,
                        pub befund: Option<Befund>, pub modell: Option<Modell> }

pub fn wirksame_klasse(v: &Vorgabe, t: &Tiefe, lage: &Lage<'_>, inhalt: &Inhalt<'_>) -> Klassenergebnis;  // K1 bis K3, K5
pub fn ganze_klasse(v: &Vorgabe, lage: &Lage<'_>, teile: &[Inhalt<'_>]) -> Klassenergebnis;              // K4, aufteilen aus

pub struct Klassenergebnis { pub klasse: Option<String>, pub kunde: Option<String>, pub herkunft: Herkunft,
                             pub senkung: Senkung, pub vorschlag: Option<String>, pub protokoll: Option<Senkungsangaben> }
impl Klassenergebnis { pub fn einheit(&self) -> Einheit<'_>; }   // K6: klasse, bei None Some("")
pub enum Herkunft { Eigen, Geerbt, Vorgabestufe, Gesenkt, Kunde, Teile, Ungueltig }
pub enum Senkung { Gesenkt, Aus, NichtGeerbt, Kundendaten, Niedrigste, ArtUnbekannt, GruppeOhneEintrag,
                   ArtNichtHerabstufbar, Befund, ModellVerhindert, ModellFehlt, NutzerAus, BestaetigungFehlt,
                   KlasseUngueltig, NichtAufgeteilt }
pub enum Ebene { Firma, Gruppe }
pub struct Senkungsangaben { pub art: String, pub erkannt_durch: Vec<String>, pub regel_ebene: Ebene,
                             pub klasse_alt: String, pub klasse_neu: String }
impl Herkunft { pub fn code(&self) -> &'static str; }
impl Senkung { pub fn code(&self) -> &'static str; }
```

- `wirksame_klasse` baut auf `Vorgabe::rang` und `wirksam` auf: K1 und K3 rechnen über `rang`, K2 ist der Aufruf von `wirksam` mit der Klasse nach K3 und dem Kunden. Ein Fehler von `wirksam` ergibt Herkunft `Ungueltig`.
- `TiefeFehler` hat eine eigene Variante für eine Nie-Art in `arten` oder `streicht` (T1, Nachtrag 20.11.2026). Die Nie-Liste ist eine Konstante der Bibliothek, kein Parameter.
- Danach ruft der Aufrufer `darf(v, &ergebnis.einheit(), route, ergebnis.kunde.as_deref())`.
- Beide Funktionen sind rein und lösen bei keiner Eingabe eine Panik aus.
- Prüfung des Baus: sichtbare und verdeckte Fälle grün; voller Mutationslauf ohne überlebende Mutanten in der Bibliothek, wie Vertrag 3.
