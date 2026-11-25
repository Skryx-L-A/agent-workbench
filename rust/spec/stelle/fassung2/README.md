# Vertrag 6: Entscheidungsstelle, Fassung 2

Stand 20.11.2026. Antrag für M3 „Tiefe der Sicherheitsbausteine“ (Meilensteinplanung, Abschnitt 5.7, neue Fassung: „Stelle mit den Ops der ersten Auslieferung“). Noch kein Bau.
Anforderungen: A09 (Abschalten einer Pflicht braucht vier Augen), A11 (Herstellerzugang), A102 und A103 (Einsicht ins Protokoll), A105 (Vier Augen für Schreibaufrufe), A38 (Senken durch das Modell nur mit Freigabe), A12 und A104 (kein Name, kein Inhalt im Protokoll), A92 (kein Agent gibt frei).
Eingearbeitet sind die Nachträge 1 bis 10 vom 20.11. (9: Abgleich, Projektentscheide; 10: feste Regel für `brain_senken`): zweite Person je Werkzeug (E6), Antworten (Abholschritt, einheitliche Listen), Vertrag 2 (Fassung 3, Fernwartung, Regeldatei) und Vault (Art `brain_mensch`, E7).

Fassung 2 ergänzt Fassung 1 (`../README.md`, gemergt am 20.11.2026). Alles, was dieser Text nicht ändert, gilt aus Fassung 1 unverändert: Schnitt (reiner Kern), Zeit, Zustand ohne Sicherung (E1), erst die Zeilen und dann die Antwort (E2), Prüfer `h1`, Kette, Hülle.
Fassung 1 bleibt die Grundlage des laufenden Baus. Ihre Fälle in `../faelle/` ändern sich nicht. Die Fälle von Fassung 2 stehen in `faelle/` dieses Ordners.
**Fassung 1 bleibt im Lauf; Fassung 2 ersetzt ihre Fälle nicht (V).** Ein Bau von Fassung 2 muss beide Ordner bestehen. A105 ist nur mit den Fällen von Fassung 1 belegt: Mit den Fällen von Fassung 2 allein blieben vier Fehler im Modell unentdeckt (Digest, Einlösen nur einmal, nur der Hub löst ein, Nachweis der Entscheidung; gemessen).
„(V)“ heißt: in diesem Vertrag festgelegt. „(A)“ heißt: Annahme, siehe Abschnitt 11. Alle Namen, Schlüssel und Zahlen in Fällen sind erfunden.

## 1. Was Fassung 2 hinzufügt

| Teil | Kurz | Abschnitt |
|---|---|---|
| Herstellerzugang | befristet und protokolliert; „kein Zugang“ ist die Vorgabe, „dauerhaft“ der zweite Grenzfall; Frist 0 wird abgelehnt; jeder Zugriff steht in der Kette | 4.1 |
| Vier Augen für Schreibaufrufe | Option der Firma, Vorgabe aus; an: zwei verschiedene Menschen geben frei | 4.2 |
| Zweite Person je Werkzeug | Option `senken_zweite_person`, Vorgabe aus; an: die Person der Anfrage darf nicht selbst erteilen | 4.3 |
| Mensch gegen Modell aus dem Kanal | Art `brain` ist immer Modellweg; Art `brain_mensch` handelt als Mensch mit Nachweis | 4.3, 4.4 |
| Pflichten ändern | Lockern braucht immer zwei Admins (A09); die Stelle schreibt die Regeldatei selbst | 4.5 |
| Einsicht ins Protokoll | nach `einsicht.regel`: niemand, mit Rolle, oder mit Rolle, Zweck und zweiter Person; nur der Antragsteller holt ab | 4.6 |
| Begrenzung bei Flut | laufende Anfragen je Sitzung; Ablehnungszeilen je Aufruferart und Fenster, der Rest als Sammelzeile mit `anzahl` | 6 |

### 1.1 Die Vorgänge von Vertrag 6 (Fassung 1 und 2 zusammen)

„Jeder Vorgang der Tabelle“ in der Anforderung meint diese fünfzehn Vorgänge.

| Nr. | Vorgang | Aufrufer | seit |
|---|---|---|---|
| 1 | `sitzung.starten` | `kern` | Fassung 1 |
| 2 | `mcp.anfragen` | `hub`, `brain`, `brain_mensch` | Fassung 1 (Arten `brain`, `brain_mensch` neu) |
| 3 | `mcp.entscheiden` | `mensch` | Fassung 1 |
| 4 | `mcp.einloesen` | Art der Anfrage | Fassung 1 |
| 5 | `mcp.offene` | `mensch` | Fassung 1 |
| 6 | `mcp.stand` | Art der Anfrage | Fassung 1 |
| 7 | `ablauf.pruefen` | `takt` | Fassung 1 |
| 8 | `einsicht.anfragen` | `mensch` | Fassung 2 |
| 9 | `pflicht.aendern` | `mensch` | Fassung 2 |
| 10 | `antrag.bestaetigen` | `mensch` | Fassung 2 |
| 11 | `antrag.offene` | `mensch` | Fassung 2 |
| 12 | `antrag.ergebnis` | `mensch` | Fassung 2 |
| 13 | `hersteller.gewaehren` | `mensch` | Fassung 2 |
| 14 | `hersteller.beenden` | `mensch` | Fassung 2 |
| 15 | `hersteller.zugriff` | `hersteller` | Fassung 2 |

Jeder hat mindestens einen sichtbaren Fall (Abschnitt 9 und Fassung 1, Abschnitt 9).

Dazu kommt Vorgang 16 `schluessel.bestaetigen` (Aufrufer `mensch`, Abschnitt 5.1, Befund B22, 21.11.). Seine Form steht fest; Bau und sichtbare Fälle folgen mit dem Schritt der Stelle für den Halter. Bis dahin gilt die Zahl fünfzehn oben.

**Gleichlauf (V).** Eine Einstellungsdatei ohne Block `stelle` lädt mit den Vorgabewerten. Damit verhält sich Fassung 2 wie Fassung 1: jede Zeile und jeder Endzustand der 40 Fälle von Fassung 1 sind Byte für Byte gleich.
Die Antworten tragen in Fassung 2 zusätzliche Felder, sonst sind sie gleich:
- `mcp.anfragen` (`ohne_freigabe`, `offen`), `mcp.stand` und `mcp.einloesen`: `entscheidend`, `mitentscheidend`, `werkzeug`, `zweite_person` (Nachtrag 1, 6, 8);
- `mcp.offene`, je Eintrag: `antragsteller`, `art`, `erste_freigabe`, `inhalt`, `inhalt_sha256` (Nachtrag 3, I2).

Der Erzeuger prüft diesen Gleichlauf bei jedem Lauf (Abschnitt 9).

## 2. Einstellungen

Die Stelle liest weiter nur die eine Einstellungsdatei. Bis der Vertrag Regelebenen gebaut ist, ist das die Firmendatei. **Die Blöcke `mcp` und `stelle` kommen in Fassung 3 von Vertrag 2** (Projektentscheid, 20.11.2026; Fassung 2 ist vergeben). Die Form unten ist Vorschlag dieses Vertrags.

**Block `mcp`**, je Server wie in Fassung 1 (`lesen`, `schreiben`, `freigabe`, `frist_ms`, `einloesefrist_ms`), dazu neu `zweite_person` (Liste von Werkzeugnamen, Vorgabe leer; Nachtrag 1). Ein Name in `zweite_person` darf nicht in `lesen` stehen (`einst.mcp_doppelt`).

**Neuer Block `stelle`** (ganz optional; jedes Feld hat einen Vorgabewert):

```json
"stelle": {
  "vier_augen": false,
  "senken_zweite_person": false,
  "halter_bescheinigung": true,
  "antrag_frist_ms": 600000,
  "einsicht": {"regel": "rolle-vier-augen", "zwecke": ["pruefung", "vorfall", "auskunft"]},
  "hersteller": {"fall": "kein", "hoechstfrist_ms": 28800000, "rollen": ["admin"]},
  "flut": {"offene_je_sitzung": 16, "zeilen_je_fenster": 32, "fenster_ms": 60000}
}
```

| Feld | Form | Vorgabe | Bedeutung |
|---|---|---|---|
| `vier_augen` | Wahrheitswert | `false` | Vier Augen für Schreibaufrufe (A105, O3); die Einsicht regelt `einsicht.regel` (Befund B08) |
| `senken_zweite_person` | Wahrheitswert | `false` | schaltet die Pflicht zur zweiten Person für die Listen `zweite_person` ein (Nachtrag 1) |
| `halter_bescheinigung` | Wahrheitswert | `true` | Bescheinigungspflicht des Halters; `false` erlaubt „schwach“ (A09, Abschnitt 4.5) |
| `antrag_frist_ms` | Ganzzahl 1 bis 86 400 000 | 600 000 | Frist eines Antrags und Abholfrist einer Einsicht |
| `einsicht.zwecke` | Liste von Schlüsseln, 1 bis 32, ohne Doppel | `["pruefung","vorfall","auskunft"]` | die erlaubten Zwecke einer Einsicht; nie Freitext (A104) |
| `einsicht.regel` | `niemand`, `rolle` oder `rolle-vier-augen` | `rolle-vier-augen` | wer ein Protokoll einsieht: niemand; eine Person mit Rolle `einsicht`; oder eine solche Person mit Zweck und Bestätigung durch eine zweite (A102, A103, Befund B08, Befund B6 vom 23.11.2026, Abschnitt 4.6) |
| `hersteller.fall` | `kein`, `befristet` oder `dauerhaft` | `kein` | welcher Fall des Herstellerzugangs gilt |
| `hersteller.hoechstfrist_ms` | Ganzzahl 1 bis 2 592 000 000 (30 Tage) | 28 800 000 (8 h) | längste Frist eines Zugangs |
| `hersteller.rollen` | Liste aus `admin`, `einsicht`, 1 bis 2 | `["admin"]` | wer einen Zugang gewähren und beenden darf |
| `flut.offene_je_sitzung` | Ganzzahl 1 bis 1024 | 16 | laufende Anfragen je Sitzung (Abschnitt 6.1) |
| `flut.zeilen_je_fenster` | Ganzzahl 1 bis 10 000 | 32 | Ablehnungszeilen je Aufruferart und Fenster (Abschnitt 6.2) |
| `flut.fenster_ms` | Ganzzahl 1000 bis 86 400 000 | 60 000 | Länge eines Fensters |

- **Jede Vorgabe ist die sichere Seite (V).** Fehlt ein Block oder ein Feld, gilt die Vorgabe (fail closed): kein Herstellerzugang; Einsicht nur mit der Rolle `einsicht`, einem Zweck aus der Liste und der Bestätigung durch eine zweite Person (`rolle-vier-augen`, Befund B6 vom 23.11.2026). Die Optionen der Firma (Vier Augen, zweite Person) sind aus, wie A102, A105 und O3 es verlangen.
- **Die Vorgabe der Einsicht ist `rolle-vier-augen`, nicht `niemand` (V, Befund B6).** `niemand` wäre der strengste Wert, nähme einer Firma ohne Block `stelle` aber jede Einsicht; A102 verlangt, dass eine Einsicht überhaupt möglich ist. Ein Admin senkt die Regel auf `rolle`, wenn die Firma das so will; das ist eine Entscheidung der Firma und keine Vorgabe. Eine Datei der Fassung 2 mit `vier_augen: true` und ohne `einsicht.regel` gilt damit als `rolle-vier-augen` (Befund B7); das ist kein Ladefehler, sondern die Vorgabe.
- **Ein falscher Wert fällt nie auf die Vorgabe (V, L-34).** Ein Feld mit falschem Typ oder außerhalb des Bereichs ist ein Ladefehler; die Stelle startet dann nicht.
- **Ladefehler.** Sie folgen den Stufen von Vertrag 2 über die ganze Datei, Block `mcp` vor Block `stelle` innerhalb jeder Stufe: `einst.schluessel_unbekannt` (auch `mcp` oder `stelle` in einer Datei der Fassung 1), `einst.schluessel_fehlt`, `einst.typ_falsch`, `einst.anzahl`, `einst.kennung_form` (auch ein Zweck in falscher Form), `einst.kennung_doppelt`, `einst.frist_bereich` (auch `antrag_frist_ms`, `hoechstfrist_ms`, `fenster_ms`), `einst.mcp_doppelt` (auch `lesen` mit `zweite_person`).
  Neu ist ein Code danach: `einst.stelle_wert` (`Wert ausserhalb der Vorgabe`) für eine unbekannte Rolle in `hersteller.rollen`, einen unbekannten Wert von `einsicht.regel`, einen unbekannten Fall und eine Zahl der Flutbegrenzung außerhalb ihres Bereichs.
- **Rollen kommen aus der Firmendatei (Befund B09, abgestimmt am 21.11.2026).** Der Block `stelle` hat kein Feld `rollen` mehr; ein solches Feld ist `einst.schluessel_unbekannt`. Die Rollen stehen nur in `rollen` der Firmendatei (Vertrag Regelebenen, Abschnitt 3.1): Pseudonym → Liste aus `admin` und `einsicht` (1 bis 2, ohne Doppel), höchstens 1024 Einträge, dort mindestens ein `admin`.
  Die Hülle liest sie aus der gespeicherten Firmendatei und übergibt sie `laden` neben der Einstellung (in den Fällen das Feld `einstellungen.rollen`). Ohne Rollen gilt `{}`: keine Rolle, also keine Einsicht und keine Pflichtänderung (fail closed).
  Die Stelle prüft die Form noch einmal selbst. Jede Abweichung ist `stelle.rollen_ungueltig` (`Rollen der Firmendatei ungueltig`), nach allen Stufen `einst.*` und vor `stelle.regel_kette_weicht_ab`; die Stelle startet dann nicht. Eine geänderte Rolle gilt ab dem nächsten Start. `wer` und `handelnd` sind das Pseudonym der geprüften Anmeldung (Vertrag 9), nie eine Kontokennung.
- **Frist 0 ist kein Weg zu „kein Zugang“ (V).** `hoechstfrist_ms` 0 ist ein Ladefehler. Kein Zugang heißt `fall: "kein"`.

### 2.1 Richtung „strenger“ je Feld (für Vertrag 2 Fassung 3 und den Vertrag Regelebenen)

Der Vertrag Regelebenen (Abschnitt 5) verlangt je Feld die Richtung „strenger“. Für die Blöcke `mcp` und `stelle` gilt (V, Vorschlag an Vertrag 2):

| Feld | Strenger ist | Zusammenführung mehrerer Ebenen |
|---|---|---|
| `mcp.<server>` (ganzer Eintrag) | kein Eintrag: Der Server ist unbekannt, jede Anfrage scheitert | Server nur, wenn die Firma ihn nennt |
| `mcp.<server>.lesen` | weniger Werkzeuge; ein Werkzeug in `schreiben` statt `lesen` | Schnittmenge |
| `mcp.<server>.schreiben` | ohne Wirkung auf die Strenge: Ein Werkzeug ohne Zuordnung gilt ohnehin als schreibend | Vereinigung |
| `mcp.<server>.freigabe` | `true` | `true`, wenn eine Ebene `true` sagt |
| `mcp.<server>.frist_ms` | kürzer | kleinster Wert |
| `mcp.<server>.einloesefrist_ms` | kürzer | kleinster Wert |
| `mcp.<server>.zweite_person` | mehr Werkzeuge | Vereinigung |
| `stelle.vier_augen` | `true` | `true`, wenn eine Ebene `true` sagt |
| `stelle.senken_zweite_person` | `true` | `true`, wenn eine Ebene `true` sagt |
| `stelle.halter_bescheinigung` | `true` | `true`, wenn eine Ebene `true` sagt |
| `stelle.antrag_frist_ms` | kürzer | kleinster Wert |
| `stelle.einsicht.regel` | `niemand`, dann `rolle-vier-augen`, dann `rolle` | der strengste Wert der Ebenen |
| `stelle.einsicht.zwecke` | weniger Zwecke | Schnittmenge |
| `stelle.hersteller.fall` | `kein` < `befristet` < `dauerhaft` (kleiner ist strenger) | kleinster Wert |
| `stelle.hersteller.hoechstfrist_ms` | kürzer | kleinster Wert |
| `stelle.hersteller.rollen` | weniger Rollen | Schnittmenge |
| `stelle.flut.offene_je_sitzung` | kleiner | kleinster Wert |
| `stelle.flut.zeilen_je_fenster` | kleiner | kleinster Wert |
| `stelle.flut.fenster_ms` | länger: bei gleicher Zeilenzahl weniger Zeilen je Zeit | größter Wert |

Die Rollen sind kein Feld dieser Tabelle: Sie stehen nur in der Firmendatei (B09), Gruppe und Nutzer setzen keine. Eine leere Schnittmenge ist kein Fehler: Ohne Rolle gibt es keine Einsicht, ohne Zweck bei `rolle-vier-augen` auch nicht (fail closed). Der Vertrag Regelebenen, Abschnitt 5.1, beschreibt `lesen` und `schreiben` noch als Wahrheitswerte; hier sind es Listen (Frage W2).

## 3. Aufrufer

Die Tabelle von Fassung 1 wächst um drei Arten. Es gilt weiter: Die Art misst die Transportschicht, nie ein Feld der Anfrage. **Mensch gegen Modell folgt aus dem Kanal, nie aus einem Feld (V, Nachtrag 7).**

| Art | Wer das ist | darf |
|---|---|---|
| `kern` | Kern-Dienst der Sitzungen | `sitzung.starten` |
| `hub` | MCP-Hub der Sitzung | `mcp.anfragen`, `mcp.einloesen`, `mcp.stand`, nur für Server außer `brain` |
| `brain` (neu) | der Such-Prozess der Sitzung (`rangkern sitzung`); immer Modellweg | `mcp.anfragen`, `mcp.einloesen`, `mcp.stand`, nur für den Server `brain`, nie mit Nachweis |
| `brain_mensch` (neu) | das Programm `brain-dienst` (eigener Crate `brain-dienst`, noch nicht gebaut), der Weg des Menschen außerhalb jeder Sitzung | `mcp.anfragen` (nur mit Nachweis), `mcp.einloesen`, `mcp.stand`, nur für den Server `brain` |
| `mensch` | die Brücke der Oberfläche | `mcp.entscheiden`, `mcp.offene`, `einsicht.anfragen`, `pflicht.aendern`, `antrag.bestaetigen`, `antrag.offene`, `antrag.ergebnis`, `hersteller.gewaehren`, `hersteller.beenden` |
| `hersteller` (neu) | das Rollenprogramm der Fernwartung (Abschnitt 3.1) | `hersteller.zugriff` |
| `takt` | Taktgeber der Stelle | `ablauf.pruefen` |
| `agent`, `unbekannt` | jeder Prozess im Einschluss; Messung gescheitert | nichts |

- **Geändert gegenüber Fassung 1 (V):** Drei Vorgänge haben jetzt drei erlaubte Arten, `hub`, `brain` und `brain_mensch`. Sie sind getrennt durch den Server (Abschnitt 4.4) und durch die Art, die eine Anfrage stellte.
- `brain` und `brain_mensch` unterscheidet die Plattformschicht (`peerkanal`) am Programm und an der Signatur, mit je eigenem Socket oder eigener Pipe. Ein Unterbefehl desselben Programms reicht nicht (E7).
- Die Art `mensch` allein genügt weiter nie. Jeder Vorgang von `mensch` oder `brain_mensch`, der etwas ändert, braucht einen Nachweis (Abschnitt 5). Die Rolle bestimmt die Stelle aus dem Pseudonym des Nachweises und den Rollen der Firmendatei (B09), nie aus einem Feld der Eingabe.

### 3.1 Art `hersteller` (Grundlage, hergeleitet; Nachtrag 4)

- Die Werkbank öffnet für den Hersteller keinen Netzzugang. Eine Fernwartung erreicht das Gerät nur über ein Fernwartungswerkzeug. Sie startet dort lokal ein eigenes, signiertes Rollenprogramm (Arbeitsname `werkbank-wartung`). Dieses verbindet sich über `peerkanal` mit der Stelle.
- `peerkanal` misst Konto, Programm (Bildpfad, unter macOS die Code-Signatur mit Team-ID), Einschränkung und Sandbox. Die Hülle ordnet die Art `hersteller` nur zu, wenn Programm und Signatur passen. Sonst ist die Art `unbekannt`.
- Der Kern lässt einen Zugriff nur zu, wenn ein aktiver Zugang nach `stelle.hersteller.fall` besteht (Abschnitt 4.1). Beide Sperren gelten zusammen.
- Die Fernsitzung selbst liegt außerhalb der Werkbank.
- **Offen (Halter-Vertrag):** Unter Windows sieht ein Dienst unter `NT SERVICE` das Programm eines Prozesses der Person nicht. Läuft die Stelle als Dienst, braucht das Rollenprogramm ein eigenes Dienstkonto oder eine Prozess-DACL für die Stelle.

## 4. Vorgänge

Fassung 2 hat fünfzehn Vorgänge, acht davon neu. Jede Ablehnung hat die Form von Fassung 1 und steht im Protokoll (mit der Begrenzung aus Abschnitt 6.2).

| Vorgang | Aufrufer | Eingabe (geschlossen) | Antwort bei Erfolg |
|---|---|---|---|
| `mcp.anfragen` | `hub`, `brain`, `brain_mensch` | wie Fassung 1; `sitzung` darf bei `brain_mensch` `null` sein; wahlweise `zweite_person` (Wahrheitswert); nur `brain_mensch`: `nachweis` und `nonce` (beide zusammen) | wie Fassung 1; bei `ohne_freigabe` und `offen` dazu `entscheidend`, `mitentscheidend`, `werkzeug`, `zweite_person` |
| `mcp.entscheiden` | `mensch` | wie Fassung 1 | wie Fassung 1; neu bei Vier Augen: `{"ok":true,"ergebnis":"erste_freigabe","anfrage":…}` |
| `mcp.einloesen` | Art der Anfrage | wie Fassung 1 | wie Fassung 1 und `entscheidend`, `mitentscheidend`, `werkzeug`, `zweite_person` |
| `mcp.stand` | Art der Anfrage | wie Fassung 1 | wie Fassung 1 und dieselben vier Felder; neuer Zustand `erste_freigabe` |
| `mcp.offene` | `mensch` | `{}` | wie Fassung 1; je Eintrag dazu `antragsteller`, `art` (`mcp.schreibaufruf`), `erste_freigabe` (`{"person":…,"zeit_ms":…}` oder `null`), `inhalt` (`digest`, `server`, `vorschau_sha256`, `werkzeug`), `inhalt_sha256` |
| `einsicht.anfragen` | `mensch` | `quelle`, `monat`, `zweck`, `nonce`, `nachweis` | bei `einsicht.regel` `rolle` `{"ok":true,"ergebnis":"einsicht_erlaubt","antrag":null,"monat":…,"quelle":…}`; bei `rolle-vier-augen` `{"ok":true,"ergebnis":"antrag_offen","antrag":…,"frist_bis_ms":…}` |
| `pflicht.aendern` | `mensch` | `pflicht`, `wert`, `nonce`, `nachweis` | verschärfen `{"ok":true,"ergebnis":"geaendert","antrag":null,"pflicht":…,"wert":true}`; lockern `{"ok":true,"ergebnis":"antrag_offen","antrag":…,"frist_bis_ms":…}` |
| `antrag.bestaetigen` | `mensch` | `antrag`, `nachweis` | Einsicht: `{"ok":true,"ergebnis":"bestaetigt","antrag":…,"abholen_bis_ms":…}`; Pflicht: `{"ok":true,"ergebnis":"geaendert","antrag":…,"pflicht":…,"wert":false}` |
| `antrag.offene` | `mensch` | `{}` | `{"ok":true,"ergebnis":"antraege","antraege":[{"antrag":…,"antragsteller":…,"art":"einsicht"\|"pflicht","erste_freigabe":null,"frist_bis_ms":…,"inhalt":{…},"inhalt_sha256":…}]}`, nach Kennung sortiert; lesend, keine Zeile |
| `antrag.ergebnis` (neu, Nachtrag 3) | `mensch` | `antrag`, `nachweis` | `{"ok":true,"ergebnis":"einsicht_erlaubt","antrag":…,"monat":…,"quelle":…}`; die Hülle hängt die Zeilen an (Abschnitt 13) |
| `hersteller.gewaehren` | `mensch` | `frist_ms` (Ganzzahl 0 bis 2 592 000 000 oder `null`), `nonce`, `nachweis` | `{"ok":true,"ergebnis":"gewaehrt","zugang":…,"bis_ms":<Zahl oder null>}` |
| `hersteller.beenden` | `mensch` | `zugang`, `nachweis` | `{"ok":true,"ergebnis":"beendet","zugang":…}` |
| `hersteller.zugriff` | `hersteller` | `zugang` | `{"ok":true,"ergebnis":"erlaubt","zugang":…,"bis_ms":…}` |

`sitzung.starten` und `ablauf.pruefen` bleiben wie in Fassung 1; `ablauf.pruefen` zählt in `abgelaufen` jetzt auch Anträge und Zugänge.

**Formen der Stufe 3, neu (V).** `nonce`: `^[0-9a-f]{32}$`. `quelle`: `gateway`, `stelle`, `brain` oder `halter` (Vertrag 5). `monat`: `^[0-9]{4}-(0[1-9]|1[0-2])$`. `zweck`: Schlüssel oder `null`. `pflicht`: Schlüssel. `wert`: Wahrheitswert.
`antrag`: `^an-[0-9]{1,15}-[1-9][0-9]{0,9}$`. `zugang`: `^hz-[0-9]{1,15}-[1-9][0-9]{0,9}$`. Beide tragen wie `mf-` die Ladezeit (E1), mit je eigenem Zähler. Ein wahlweises Feld darf fehlen; jedes andere fehlende oder fremde Feld ist `stelle.eingabe_ungueltig`.
Bei `mcp.anfragen` ist außerdem `stelle.eingabe_ungueltig`: `nachweis` ohne `nonce` oder umgekehrt; `nachweis` oder `sitzung: null` von einer anderen Art als `brain_mensch`.

### 4.1 Herstellerzugang (A11)

Gebaut wird der befristete Zugang. Die beiden anderen Fälle sind seine Grenzfälle: „kein“ lässt keinen Zugang zu, „dauerhaft“ erlaubt einen Zugang ohne Frist. **Welcher Fall in der ersten Fassung gilt, entscheidet die Organisation.** Der Vertrag nimmt die Antwort nicht vorweg: Er macht den Fall zur Einstellung `hersteller.fall` mit der Vorgabe `kein`.
Ebenso offen ist, wer in der Organisation einen Zugang gewähren und beenden darf. Das ist die Einstellung `hersteller.rollen` mit der Vorgabe `["admin"]`.

**`hersteller.gewaehren`, Stufe 4**, in dieser Reihenfolge:

1. `hersteller.fall` ist `kein`: `stelle.hersteller_aus`.
2. `frist_ms` ist 0, oder `null` ohne den Fall `dauerhaft`, oder größer als `hoechstfrist_ms`, oder das Ende läge nach 9999: `stelle.frist_ungueltig`. **Frist 0 wird also immer abgelehnt (V).**
3. Der Nachweis über die Herausforderung (Abschnitt 5) besteht nicht: `stelle.nachweis_ungueltig`.
4. Die Nonce war schon da: `stelle.nonce_benutzt`.
5. Das Pseudonym hat keine Rolle aus `hersteller.rollen`: `stelle.rolle_fehlt`.
6. Ein anderer Zugang ist aktiv: `stelle.zugang_aktiv`. Es gibt höchstens einen aktiven Zugang (A).
7. Sonst entsteht der Zugang `hz-<laden_ms>-<n>` im Zustand `aktiv`, Ende `jetzt_ms + frist_ms` oder ohne Ende bei `null`; Zeile `hersteller.zugang` mit `gewaehrt`.

**`hersteller.zugriff`, Stufe 4:** `zugang` unbekannt: `stelle.zugang_unbekannt`; abgelaufen: `stelle.zugang_abgelaufen`; beendet: `stelle.zugang_beendet`. Sonst Zeile `hersteller.zugriff` mit `erlaubt`.
**Jeder Zugriff steht in der Kette (V):** der erlaubte als `hersteller.zugriff`, der abgelehnte als `vorgang.abgelehnt` mit der Kennung des Zugangs. Ein Zugriff ist jede Anfrage des Rollenprogramms an die Stelle.

**`hersteller.beenden`, Stufe 4:** `zugang` unbekannt, abgelaufen oder beendet wie oben; Nachweis; Rolle aus `hersteller.rollen`. Sonst Zustand `beendet`, Zeile `hersteller.zugang` mit `beendet` und dem Pseudonym.

**Frist (V).** Wie in Fassung 1 gilt ein Zugang, solange `jetzt_ms` kleiner als das Ende ist. Zum Ende selbst ist er abgelaufen; Stufe 1 schreibt dann die Zeile `hersteller.zugang` mit `abgelaufen` und ohne Person.
**Neustart.** Nach einem Neustart ist jeder Zugang unbekannt (E1, fail closed). Eine Zeile über dieses Ende entsteht nicht (Abschnitt 12, Punkt L1).

```
{"aktionsname":"hersteller.zugang","bis":"2026-11-20T13:01:01.000Z","ergebnis":"gewaehrt","fall":"befristet","fassung":2,"person":"ps_00000000000000000000000000000b01","vorher":"e2bbd239…ce8070e1","zeit":"2026-11-20T13:00:01.000Z","zugang":"hz-1795179540000-1"}
{"aktionsname":"hersteller.zugriff","ergebnis":"erlaubt","fassung":2,"vorher":"c82def9b…84e37060","zeit":"2026-11-20T13:00:02.000Z","zugang":"hz-1795179540000-1"}
{"aktionsname":"hersteller.zugang","bis":"2026-11-20T13:01:01.000Z","ergebnis":"abgelaufen","fall":"befristet","fassung":2,"person":null,"vorher":"955fc14c…002f8145","zeit":"2026-11-20T13:01:01.000Z","zugang":"hz-1795179540000-1"}
```

### 4.2 Vier Augen für Schreibaufrufe (A105, O3)

Die Option `vier_augen` ist aus. Dann gilt Fassung 1 unverändert.
**Ist sie an, braucht jede Anfrage mit Freigabe die Erteilung zweier verschiedener Menschen (V).** Die Zahl der nötigen Freigaben wird beim Anlegen der Anfrage festgelegt; eine spätere Änderung der Pflicht gilt nur für neue Anfragen.

`mcp.entscheiden` mit `erteilen` geht dann so:

1. Die Stufen 1 bis 3 von Fassung 1 (unbekannt, abgelaufen, schon entschieden, Nachweis). `erste_freigabe` gilt dabei wie `offen` als noch nicht entschieden.
2. Zustand `erste_freigabe` und dasselbe Pseudonym wie bei der ersten Freigabe: `stelle.selbe_person`, die Anfrage bleibt, wie sie ist. Das ist „eine Person allein wird abgelehnt und protokolliert“.
3. Zustand `offen`: Zustand `erste_freigabe`, keine Zeile, Antwort `erste_freigabe`. Die Frist der Entscheidung läuft weiter.
4. Zustand `erste_freigabe` mit einer anderen Person: Zustand `erteilt`, Einlösefrist ab jetzt, wie Fassung 1.

Die Regel „zweite Freigabe von einer anderen Person als der ersten“ prüft die Stelle (I2).
**Keine Sammelfreigabe (V, A105 Punkt 6).** `mcp.entscheiden` gilt für genau eine Anfrage. Eine Liste von Kennungen in `anfrage` oder ein Zusatzfeld ergibt `stelle.eingabe_ungueltig` mit Zeile (Fall `keine-sammelfreigabe`; über den Gleichlauf gilt er für beide Fassungen). Die Brücke hat genau ein Pseudonym und weiß nichts von anderen Brücken.
`mcp.einloesen` im Zustand `erste_freigabe` ergibt `stelle.zweite_person_fehlt`. **Ablehnen darf eine Person allein (V)**, auch nach der ersten Freigabe einer anderen: Die Ablehnung ist die sichere Richtung.

Die Zeile `mcp.schreibaufruf` einer Anfrage mit Vier Augen trägt zwei Felder mehr: `mitentscheidend` (die erste freigebende Person oder `null`) und `zeit_mitentscheidung`. `entscheidend` bleibt die Person der letzten Entscheidung. Der Zeilenprüfer verlangt bei `eingeloest` zwei verschiedene Pseudonyme.

```
{"aktionsname":"mcp.schreibaufruf","anfrage":"mf-1795179540000-1","digest":"34b94b40…ba4ced","entscheidend":"ps_00000000000000000000000000000a02","ergebnis":"eingeloest","fassung":2,"mitentscheidend":"ps_00000000000000000000000000000a01","person":"ps_00000000000000000000000000000001","server":"tickets","sitzung":"s-1","vorher":"5618102d…db3a6052","werkzeug":"update","zeit":"2026-11-20T13:00:05.000Z","zeit_anfrage":"2026-11-20T13:00:01.000Z","zeit_entscheidung":"2026-11-20T13:00:04.000Z","zeit_mitentscheidung":"2026-11-20T13:00:02.000Z"}
```

**Die zwei Menschen unterscheidet die Stelle nur am Pseudonym aus dem Nachweis.** Mit dem Prüfer `h1` stellt die Brücke beide Nachweise aus. Vier Augen sind im Durchstich also nur so stark wie die Anmeldung an der Brücke; echte Trennung bringt erst die Gerätesignatur je Person (Abschnitt 10).

### 4.3 Zweite Person je Werkzeug, Mensch und Modell (Nachtrag 1, 2, 6, 7; Vault E6, E7)

Die erste Anwendung ist das Senken einer Klasse im Schreibtor von Vault. Das Senken hat einen eigenen Werkzeugnamen, damit die Stelle es ohne Blick in die Argumente erkennt: Server `brain`, Werkzeug `brain_senken`. Das Tor nennt diesen Namen, sobald es senkt; der Agent wählt ihn nie.
Die Einstellungen der Fälle: `mcp.brain` mit `lesen: ["brain_suche"]`, `schreiben: ["brain_schreiben", "brain_senken"]`, `zweite_person: ["brain_senken"]`.

**Pflicht zur zweiten Person (V).** Sie gilt für eine Anfrage, wenn `senken_zweite_person` an ist und das Werkzeug in `zweite_person` seines Servers steht, oder wenn die Anfrage `zweite_person: true` trägt. Das Feld kann die Pflicht nur einschalten, nie ausschalten: `false` und ein fehlendes Feld ändern nichts.
Gilt die Pflicht, ergibt `mcp.entscheiden` mit `erteilen` durch das Pseudonym in `person` der Anfrage den Code `stelle.selbe_person`; die Anfrage bleibt offen. Ablehnen darf dieselbe Person. Mit Vier Augen zusammen darf keine der beiden Freigaben von der Person der Anfrage kommen.

**Wann eine Anfrage eine Freigabe braucht (V):**

| Art | Pflicht zur zweiten Person | `brain_senken` auf Server `brain`, Pflicht aus | sonst |
|---|---|---|---|
| `brain` (Modellweg, nie mit Nachweis) | offen | **immer offen** (feste Regel, A38) | folgt `freigabe` des Servers |
| `hub` (Modellweg) | offen | – (der Hub fragt den Server `brain` nie) | folgt `freigabe` des Servers |
| `brain_mensch` (der Mensch handelt selbst, Nachweis Pflicht) | offen; nur eine **andere** Person erteilt | ohne Freigabe | ohne Freigabe, auch bei `freigabe: true` |

**Feste Regel (V, Nachtrag 10, Befund: fail open über eine Einstellung).** Art `brain` mit Server `brain` und Werkzeug `brain_senken` ergibt immer `offen`. Das hängt weder von `freigabe` des Servers ab noch davon, ob `brain_senken` in `zweite_person` steht; keine Einstellung kann es abschalten.
Die Liste `zweite_person` entscheidet nur noch, ob die erteilende Person eine andere sein muss als die anfragende (zusammen mit `senken_zweite_person`). Der Werkzeugname `brain_senken` ist im Kern fest (Fall `modell-senkt-feste-regel`).

**`brain_mensch` in `mcp.anfragen`, Stufe 4** nach „Server passt zur Art“, Server und Sitzung (eine Sitzung `null` ist erlaubt, Arbeit außerhalb jeder Sitzung): Ohne Nachweis, mit einem Nachweis, der nicht besteht, oder mit einem Pseudonym im Nachweis, das nicht `person` ist: `stelle.nachweis_ungueltig` mit Zeile. Eine benutzte Nonce: `stelle.nonce_benutzt`.
Ohne Freigabe steht der Mensch als `entscheidend` in Antwort und Zeile (`ohne_freigabe` mit `zeit_entscheidung` gleich der Zeit der Anfrage). **`entscheidend` ist am Ende nur im Modellweg ohne Freigabe `null` (V).** Solange eine Anfrage läuft, ist es `null`, bis jemand erteilt.

**Antworten für das Tor (Nachtrag 6, 8).** `mcp.anfragen` (`ohne_freigabe`, `offen`), `mcp.stand` und `mcp.einloesen` nennen `entscheidend`, `mitentscheidend` (erste Person bei Vier Augen, sonst `null`), `werkzeug` und `zweite_person`. Im zweiten Tor-Aufruf trägt `kontext.freigabe` Werkzeug und `zweite_person` aus dieser Antwort; das Tor prüft bei `zweite_person: true` die zwei Personen ein zweites Mal.

**Restvertrauen:** Im Modellweg kommt `person` vom Aufrufer (Fassung 1, Annahme 6). Ein Such-Prozess, der eine fremde Person einträgt, umgeht die Pflicht zur zweiten Person; er liegt außerhalb des Einschlusses. Beim Menschenweg bindet die Stelle `person` an das Pseudonym des Nachweises.

```
{"aktionsname":"mcp.schreibaufruf","anfrage":"mf-1795179540000-1","digest":"e80201ed…65392b5d","entscheidend":"ps_00000000000000000000000000000a01","ergebnis":"ohne_freigabe","fassung":2,"person":"ps_00000000000000000000000000000a01","server":"brain","sitzung":"s-1","vorher":"e2bbd239…ce8070e1","werkzeug":"brain_senken","zeit":"2026-11-20T13:00:01.000Z","zeit_anfrage":"2026-11-20T13:00:01.000Z","zeit_entscheidung":"2026-11-20T13:00:01.000Z"}
```

### 4.4 Arten `brain` und `brain_mensch` (Wunsch, angenommen)

- `mcp.anfragen`: Erste Stufe 4.0 „Server passt zur Art“: `brain` oder `brain_mensch` mit einem anderen Server als `brain`, oder `hub` mit dem Server `brain`, ergibt `stelle.server_nicht_erlaubt`. Danach folgen die Stufen von Fassung 1 und Abschnitt 4.3.
- `mcp.stand` und `mcp.einloesen`: **Eine Anfrage gehört zur Art, die sie stellte (V).** Fragt eine andere Art nach ihr, ist die Antwort `stelle.anfrage_unbekannt`. So verrät die Stelle der anderen Art nicht, ob es die Anfrage gibt, und der Such-Prozess löst nie eine Anfrage des Menschenwegs ein.
- `mcp.entscheiden` und `mcp.offene` sehen Anfragen aller Arten; der Mensch entscheidet sie.
- Der Server `such-mcp` aus Fassung 1 bleibt lesend und unberührt. Der Server `brain` braucht wie jeder Server einen Eintrag im Block `mcp`; ohne ihn ist er unbekannt.

### 4.5 Pflichten ändern und Regeldatei (A09; Nachtrag 5 und 6)

**Die Stelle ist die einzige Quelle der Pflichten (V).** Pflichten sind `vier_augen`, `senken_zweite_person`, `halter_bescheinigung` und je Server `mcp.<server>.freigabe`; die Listen `zweite_person` gehören dazu. Kein Tor, kein Hub und kein Such-Prozess hält eine Kopie. Der Aufrufer fragt für jedes Schreiben `mcp.anfragen`; die Stelle antwortet nach ihren Pflichten im Augenblick. Eine Verschärfung wirkt deshalb ab der nächsten Anfrage (Fall `verschaerfen-wirkt-sofort`).

**Pflicht „schwach“ des Halters (A09, Nachtrag 9).** `halter_bescheinigung` ist die Bescheinigungspflicht des Halters. Abschalten, also „schwach“ erlauben, ist eine Lockerung wie jede andere: Eine Person allein stellt nur einen Antrag, erst ein zweiter Admin macht sie wirksam (Fall `pflicht-schwach-braucht-zwei`).
Die andere Hälfte von A09, „ohne Schalter wird schwach abgelehnt“, gehört dem Halter. Der Halter fragt den Wert bei der Stelle; den Vorgang dafür legt der Halter-Vertrag fest (Frage W5).

**Lockern ist der Wert `false`, Verschärfen der Wert `true` (V).** Verschärfen darf ein Admin allein. **Lockern durch eine Person allein ändert nie etwas (V, A09):** Ihr Versuch wird ein Antrag und steht als Zeile in der Kette; ihre eigene Bestätigung wird abgelehnt und steht ebenfalls in der Kette. Erst ein zweiter Admin macht die Lockerung wirksam. Das gilt unabhängig von der Option `vier_augen`.

**`pflicht.aendern`, Stufe 4:** 1. Pflicht unbekannt: `stelle.pflicht_unbekannt`. 2. Nachweis: `stelle.nachweis_ungueltig`. 3. Nonce: `stelle.nonce_benutzt`. 4. Rolle `admin`: `stelle.rolle_fehlt`. 5. Wert schon gesetzt: `stelle.pflicht_unveraendert`. 6. Ein Antrag für dieselbe Pflicht ist offen: `stelle.antrag_laeuft`.
7. Verschärfen: Wert gilt sofort, Zeile `regel.geaendert`, Datei geschrieben (unten). Lockern: Antrag `an-<laden_ms>-<n>` im Zustand `offen`, Frist `antrag_frist_ms`, Zeile `pflicht.aenderung` mit `antrag`; keine Datei.

**`antrag.bestaetigen`, Stufe 4** (für Pflicht- und Einsichtsanträge): 1. unbekannt: `stelle.antrag_unbekannt`. 2. abgelaufen: `stelle.antrag_abgelaufen`; schon bestätigt: `stelle.bereits_entschieden`. 3. Nachweis. 4. Dasselbe Pseudonym wie der Antragsteller: `stelle.selbe_person`. 5. Rolle: bei Pflicht `admin`, bei Einsicht `einsicht` oder `admin` (A): `stelle.rolle_fehlt`.
6. Pflicht: Der Wert gilt, Zeile `regel.geaendert` mit beiden Personen, Datei geschrieben. Einsicht: Abschnitt 4.6. Ein Antrag, den niemand bestätigt, verfällt mit der Zeile `pflicht.aenderung` mit `abgelaufen`.

**Die Stelle schreibt die Regeldatei selbst (V, I3).** Sie ist der einzige Schreiber der Regeln. Die Konsole schreibt nie in eine Datei; sie stellt nur Vorgänge.
- Geschrieben wird die Datei der Ebene (Firma oder Gruppe). Bis der Vertrag Regelebenen gebaut ist, ist die Firmendatei selbst die Einstellungsdatei; Fassung 2 schreibt nur die Ebene `firma`. Mit Regelebenen läuft das Schreiben über dessen Prüfung `aendern` (Form, Befugnis, Bezug, Lockerung).
- Der Kern führt die Datei im Zustand. Eine wirksame Änderung ergibt die Zeile `regel.geaendert` und im `Ergebnis` das Feld `datei` mit `ebene` und dem neuen Inhalt als RFC 8785. Der Kern schreibt keine Datei; das tut die Hülle (Abschnitt 13).
- **Erst die Zeile, dann die Datei (V).** `alt` und `neu` der Zeile sind sha256 über RFC 8785 der ganzen Datei vor und nach der Änderung. Die Zeile nennt Handelnden und zweite Person als Pseudonyme und den Namen der Pflicht, keinen Wert eines Feldes.
- **Abweichung beim Start (V).** Die Hülle übergibt `laden` den Wert `neu` der letzten Zeile `regel.geaendert` dieser Ebene. Ist der sha256 der gelesenen Datei ein anderer, startet die Stelle nicht (`stelle.regel_kette_weicht_ab`). So erkennt der nächste Start einen Absturz nach der Zeile (Fall `laden-regel-absturz-nach-zeile`). Lauf starten prüft dasselbe mit `lauf.regel_kette_weicht_ab` (Vertrag 1).
- Ein Neustart mit der geschriebenen Datei behält die Änderung (Fall `pflicht-ueberlebt-neustart`).

```
{"aktionsname":"pflicht.aenderung","antrag":"an-1795179540000-1","ergebnis":"antrag","fassung":2,"person":"ps_00000000000000000000000000000b01","pflicht":"mcp.tickets.freigabe","vorher":"e2bbd239…ce8070e1","wert":false,"zeit":"2026-11-20T13:00:01.000Z","zweiter":null}
{"aktionsname":"regel.geaendert","alt":"279875cf…f2e5beba","antrag":null,"ebene":"firma","ergebnis":"geaendert","fassung":2,"handelnd":"ps_00000000000000000000000000000b01","neu":"c9de5a5b…c9ce57ea","pflicht":"mcp.notizen.freigabe","vorher":"a8bc0665…7dc7dc69","zeit":"2026-11-20T13:00:02.000Z","zweiter":null}
```

### 4.6 Einsicht ins Protokoll (A102, A103; Nachtrag 3)

Die Stelle liest keine Datei. Sie entscheidet, ob eine Person einen Monat einer Quelle einsehen darf, und schreibt die Einsicht in die Kette. Die Zeilen liefert die Hülle (Abschnitt 13).

**`einsicht.anfragen`, Stufe 4:** 0. `einsicht.regel` ist `niemand`: `stelle.einsicht_aus`, vor dem Nachweis und ohne die Nonce zu verbrauchen (wie `stelle.hersteller_aus`; B08). 1. Nachweis. 2. Nonce. 3. Rolle `einsicht`: `stelle.rolle_fehlt`. Die Rolle `admin` allein genügt nicht (V).
4. Zweck: Bei `rolle-vier-augen` muss `zweck` gesetzt sein; ein gesetzter Zweck muss in `einsicht.zwecke` stehen. Sonst `stelle.zweck_ungueltig`.
5. Bei `rolle`: Die Einsicht ist erlaubt, Zeile `protokoll.einsicht` mit `erlaubt`; die Antwort geht an die Brücke des Antragstellers selbst, die Hülle hängt die Zeilen an. Bei `rolle` fragt die Oberfläche keinen Zweck ab (keine Abfrage ohne Not). Die Option `vier_augen` ändert daran nichts; sie gilt nur für Schreibaufrufe (B08).
   Bei `rolle-vier-augen`: Antrag `an-<laden_ms>-<n>` ohne Zeile.

**Mit `rolle-vier-augen` in drei Schritten (V, I1).** Die Brücke ist am Socket `mensch` nur Client; die Stelle kann ihr nichts zuschicken.
1. Der Antragsteller stellt den Antrag (`einsicht.anfragen`).
2. Eine zweite Person mit Rolle bestätigt ihn (`antrag.bestaetigen`). Sie sieht in `antrag.offene` nur, was eingesehen werden soll (Art, Quelle, Monat, Zweck), nie die Zeilen. Die Bestätigung schreibt `protokoll.einsicht` mit `erlaubt`, beiden Personen und dem Zweck; die Antwort `bestaetigt` trägt keine Zeilen. Ab jetzt läuft die Abholfrist (`antrag_frist_ms`).
3. Die Brücke des Antragstellers holt das Ergebnis ab (`antrag.ergebnis`). **Nur das Pseudonym des Antragstellers, nur einmal (V).** Die Zeile `protokoll.einsicht` mit `abgeholt` steht vor der Auslieferung in der Kette. Danach ist der Antrag `abgeholt`; die Erlaubnis ist aus dem Zustand verbraucht.

**`antrag.ergebnis`, Stufe 4:** 1. Antrag unbekannt oder kein Einsichtsantrag: `stelle.antrag_unbekannt`. 2. Nachweis über `{"antrag":…,"art":"antrag.ergebnis"}`. 3. Nicht der Antragsteller: `stelle.nicht_antragsteller`. 4. Zustand `offen`: `stelle.antrag_nicht_bestaetigt`; `abgelaufen` oder `verfallen`: `stelle.antrag_abgelaufen`; `abgeholt`: `stelle.bereits_abgeholt`.
Holt niemand ab, verfällt die Einsicht zur Abholfrist mit der Zeile `protokoll.einsicht` mit `verfallen`.

```
{"aktionsname":"protokoll.einsicht","antrag":"an-1795179540000-1","ergebnis":"abgeholt","fassung":2,"monat":"2026-11","person":"ps_00000000000000000000000000000e01","quelle":"stelle","vorher":"ed438e24…4fa1f347","zeit":"2026-11-20T13:00:04.000Z","zweck":"pruefung","zweiter":"ps_00000000000000000000000000000b01"}
```

**Einheitliche Listen (V, I2).** Jeder Eintrag in `antrag.offene` und `mcp.offene` trägt: die Kennung (`antrag` beziehungsweise `anfrage`, wie das Eingabefeld des Vorgangs, der sie bestätigt), `art`, `antragsteller`, `inhalt`, `inhalt_sha256`, `erste_freigabe` (`{"person":…,"zeit_ms":…}` oder `null`) und `frist_bis_ms`. Bei `mcp.offene` bleiben die Felder von Fassung 1 daneben stehen (Gleichlauf).

## 5. Nachweise und Herausforderungen

Der Prüfer `h1` und seine Schnittstelle bleiben wie in Fassung 1. Neu sind diese Herausforderungen, je RFC 8785 als UTF-8:

| Vorgang | Herausforderung |
|---|---|
| `mcp.anfragen` von `brain_mensch` | `{"art":"mcp.handeln","digest":…,"nonce":…,"sitzung":<Kennung oder null>}`; `digest` rechnet die Stelle aus dem Aufruf (Fassung 1, Abschnitt 6) |
| `einsicht.anfragen` | `{"art":"einsicht.anfragen","monat":…,"nonce":…,"quelle":…,"zweck":…}` |
| `pflicht.aendern` | `{"art":"pflicht.aendern","nonce":…,"pflicht":…,"wert":…}` |
| `antrag.bestaetigen` | `{"antrag":…,"art":"antrag.bestaetigen","inhalt_sha256":…}`; `inhalt_sha256` ist sha256 über RFC 8785 des Inhalts aus `antrag.offene` |
| `antrag.ergebnis` | `{"antrag":…,"art":"antrag.ergebnis"}` |
| `hersteller.gewaehren` | `{"art":"hersteller.gewaehren","frist_ms":…,"nonce":…}` |
| `hersteller.beenden` | `{"art":"hersteller.beenden","zugang":…}` |

- **Nonce (V).** Vorgänge ohne Kennung, an die sich ein Nachweis binden ließe, tragen eine Nonce der Brücke oder des `brain-dienst`. Die Stelle merkt sich jede Nonce, sobald ein Nachweis mit ihr gültig war, auch wenn eine spätere Stufe ablehnt. Eine zweite Verwendung ergibt `stelle.nonce_benutzt`. Damit gilt auch hier: Ein Nachweis gilt nur einmal.
  Die gemerkten Nonces gehen mit einem Neustart verloren (E1). Die Brücke zieht für jede Handlung eine neue Nonce (A).
- Ein Nachweis für einen Vorgang gilt nie für einen anderen, weil `art` in jeder Herausforderung steht. Ein Nachweis für `mcp.handeln` bindet den `digest`: Er gilt für genau diesen Aufruf.

### 5.1 Schlüssel-Schein für den Halter (`schluessel.setzen`, `schluessel.loeschen`; Befund B22, Halter OPEN 1)

Der Halter verlangt für `schluessel.setzen` und `schluessel.loeschen` einen frischen Menschen-Nachweis der Stelle (Halter-Vertrag, Abschnitt 4, Stufe 27 `halter.nachweis`). Der Halter darf den Schlüssel von `h1` nie kennen; sonst hielte ein drittes Programm ihn. Darum prüft die Stelle `h1` und gibt dem Halter einen eigenen signierten Schein, nach dem Vorbild des MCP-Scheins (Halter-Vertrag, Abschnitt 6).

**Vorgang 16 `schluessel.bestaetigen` (Aufrufer `mensch`).** Eingabe: `op` (`setzen` oder `loeschen`), `name` (Name eines Geheimnisses im Bereich `person`, Form wie im Halter-Vertrag), `wert_sha256` (bei `setzen` 64 kleine Hexziffern, sha256 über die dekodierten Bytes von `wert_b64`; bei `loeschen` `null`), `nonce` und `nachweis`.
- Herausforderung: RFC 8785 von `{"art":"schluessel.bestaetigen","name":…,"nonce":…,"op":…,"wert_sha256":…}`. Die Brücke rechnet `wert_sha256` selbst über den Wert, den der Mensch in der Ansicht eingegeben hat; der Wert selbst geht nie an die Stelle.
- Prüfung wie bei jedem Nachweis (Abschnitt 5): `h1` ungültig ergibt `stelle.nachweis_ungueltig`, eine schon benutzte Nonce `stelle.nonce_benutzt`, ein Feld außerhalb seiner Form `stelle.eingabe_ungueltig`.
- Antwort bei Erfolg: `{"ok":true,"ergebnis":"schein","schein":<Text>}`. `schein` ist Base64 mit Auffüllung über RFC 8785 von `{"daten":…,"signatur":…}`. Das ist druckbares ASCII ohne Leerzeichen und passt in das Feld `nachweis` des Halters (1 bis 8192 Zeichen).

Die Felder von `daten` (geschlossene Form):

| Feld | Wert |
|---|---|
| `art` | genau `schluessel-schein` |
| `fassung` | `1` |
| `kennung` | `ks-<laden_ms>-<n>` mit eigenem Zähler; Grenze wie bei `mf-` (Fassung 1, Abschnitt 4) |
| `kid` | Kennung des Schlüsselpaars der Stelle nur für Schlüssel-Scheine, nie dasselbe Paar wie für MCP-Scheine; so kann ein MCP-Schein nie einen Schlüssel ändern |
| `op` | `setzen` oder `loeschen` |
| `name` | wie in der Eingabe |
| `wert_sha256` | wie in der Eingabe (`null` bei `loeschen`) |
| `person` | das Pseudonym aus dem Nachweis |
| `ausgestellt_ms`, `bis_ms` | Zeiten in ms; `bis_ms − ausgestellt_ms` ist 120 000, höchstens 300 000 |

`signatur` ist Ed25519 (RFC 8032) über die RFC-8785-Bytes von `daten`, Base64 mit Auffüllung (88 Zeichen).

Was der Halter prüft, in dieser Reihenfolge (Vorschlag für den Halter-Vertrag); jede Abweichung ist `halter.nachweis`:
1. Base64 und JSON sind lesbar, es stehen genau die Felder oben da, `art` und `fassung` stimmen.
2. `kid` steht in `halter.json` als Schlüssel für Schlüssel-Scheine, und die Signatur ist gültig.
3. `op` ist der aufgerufene Vorgang, `name` ist das Feld `name` des Aufrufs.
4. Bei `setzen` ist sha256 über die dekodierten Bytes von `wert_b64` gleich `wert_sha256`; bei `loeschen` ist `wert_sha256` `null`.
5. Die Zeit des Halters liegt in `[ausgestellt_ms, bis_ms]`, und `bis_ms − ausgestellt_ms` ist höchstens 300 000.
6. `kennung` wurde noch nicht eingelöst. Der Halter merkt sich eingelöste Kennungen mindestens bis `bis_ms`; nach einem Neustart des Halters begrenzt die kurze Frist eine Wiederholung.

Danach legt der Halter das Geheimnis unter dem Konto des Aufrufers mit dem Pseudonym `person` ab, wie im Halter-Vertrag.

Protokoll: neue Zeilenart `schluessel.schein` mit `ergebnis` (`ausgestellt`), `kennung`, `op` und `person`. `name` und `wert_sha256` stehen nie in der Zeile; die Prüfsumme eines Geheimnisses gehört in kein dauerhaftes Protokoll. Die geschlossene Form kommt mit dem Bau in Vertrag 5.

Grenze: Wie bei `h1` belegt der Schein einen Klick in der Ansicht mit gültigem Token der Brücke, keine bestimmte Person (Fassung 1, Abschnitt 6). Mit der Gerätesignatur (Abschnitt 10) ändert sich nur der Prüfer, nicht der Schein.

## 6. Begrenzung bei Flut

Ein Agent soll das Protokoll nicht fluten können. Fassung 2 hat zwei Grenzen.

### 6.1 Laufende Anfragen je Sitzung

Eine Anfrage läuft, solange sie `offen`, `erste_freigabe` oder `erteilt` ist. Hat die Sitzung schon `offene_je_sitzung` laufende Anfragen, ergibt `mcp.anfragen` den Code `stelle.zu_viele_offen`. Anfragen ohne Sitzung (`brain_mensch`) zählen zusammen wie eine Sitzung.
Die Stufe steht nach `ohne_freigabe` und vor `stelle.frist_ungueltig`. Lesende Aufrufe und Aufrufe ohne Freigabe zählen nicht; sie legen nichts an, und jeder ausgeführte Schreibaufruf muss im Protokoll stehen (A91).

### 6.2 Ablehnungszeilen je Fenster

- Die Zeit seit `laden_ms` ist in Fenster der Länge `fenster_ms` geteilt. Je Aufruferart schreibt die Stelle in einem Fenster höchstens `zeilen_je_fenster` Ablehnungszeilen (`vorgang.abgelehnt` und `sitzung.abgelehnt`).
- **Jede weitere Ablehnung bekommt dieselbe Antwort, aber keine eigene Zeile.** Die Stelle zählt sie je Gruppe aus Aufruferart, Vorgang und Code.
- **Zusammenfassen (V).** Beim ersten Ereignis nach dem Fenster schreibt Stufe 1 je Gruppe eine Zeile `vorgang.gedrosselt` mit `anzahl`, sortiert nach Aufruferart, Vorgang und Code, vor allen anderen Zeilen des Ereignisses. Der Taktgeber sorgt dafür, dass das auch ohne weitere Anfragen geschieht.
- So steht jede Ablehnung im Protokoll, einzeln oder gezählt (A92). Die Grenze gilt je Art: Ein flutender Agent nimmt dem Hub und dem Menschen keine Zeile weg. Erfolgszeilen werden nie begrenzt.

```
{"aktionsname":"vorgang.gedrosselt","anzahl":2,"aufrufer":"agent","ergebnis":"abgelehnt","fassung":2,"fenster_bis":"2026-11-20T13:00:10.000Z","fenster_von":"2026-11-20T13:00:00.000Z","grund":"stelle.aufrufer_nicht_erlaubt","vorgang":"mcp.entscheiden","vorher":"09988777…21e1a87b","zeit":"2026-11-20T13:00:10.000Z"}
```

## 7. Protokoll

Kette, Datei, Kopf und Bytes bleiben wie in Fassung 1 (Vertrag 5). Fassung 2 hat sechs neue Zeilenarten und erweitert zwei.

| Zeilenart | Felder außer `aktionsname`, `fassung`, `vorher`, `zeit` |
|---|---|
| `mcp.schreibaufruf` | wie Fassung 1; `sitzung` darf `null` sein (`brain_mensch`); `ohne_freigabe` nennt beim Menschenweg den Menschen als `entscheidend`; bei Vier Augen dazu `mitentscheidend` (Pseudonym oder null) und `zeit_mitentscheidung` (Zeit oder null), beide zusammen null oder gesetzt |
| `vorgang.abgelehnt` | wie Fassung 1; `anfrage` nimmt jede Kennung der Form `mf-`, `an-` oder `hz-` aus der Eingabe auf; `aufrufer` kennt die neuen Arten |
| `hersteller.zugang` | `bis` (Zeit oder null), `ergebnis` (`gewaehrt`, `beendet`, `abgelaufen`), `fall` (`befristet`, `dauerhaft`), `person` (Pseudonym; null genau bei `abgelaufen`), `zugang` |
| `hersteller.zugriff` | `ergebnis` (`erlaubt`), `zugang` |
| `protokoll.einsicht` | `antrag` (oder null), `ergebnis` (`erlaubt`, `abgeholt`, `abgelaufen`, `verfallen`), `monat`, `person`, `quelle`, `zweck` (oder null), `zweiter` (oder null) |
| `pflicht.aenderung` | `antrag` (oder null), `ergebnis` (`antrag`, `abgelaufen`), `person`, `pflicht`, `wert`, `zweiter` (oder null) |
| `regel.geaendert` | Form aus Vertrag 5, Abschnitt 6.6 (B13): `alt`, `neu` (sha256 über RFC 8785 der ganzen Ebenen-Datei), `antrag` (oder null), `code` (hier immer null), `ebene` (hier immer `firma`), `ergebnis` (`geaendert`), `handelnd`, `pflicht`, `ziel` (hier immer null), `zweiter` (oder null) |
| `vorgang.gedrosselt` | `anzahl` (1 bis 4 294 967 295), `aufrufer`, `ergebnis` (`abgelehnt`), `fenster_bis`, `fenster_von`, `grund`, `vorgang` |

- Zusammenhänge, die der Zeilenprüfer der Fälle prüft: Eine eingelöste Anfrage mit Vier Augen nennt zwei verschiedene Personen. Eine Einsicht mit Antrag (`erlaubt`, `abgeholt`, `verfallen`) nennt eine zweite, andere Person. `regel.geaendert` hat `antrag` und `zweiter` zusammen, und `zweiter` ist nie `handelnd`.
- **Nie in einer Zeile (A12, A104, V):** wie Fassung 1. Zweck und Pflicht sind Schlüssel aus fester Liste, nie Freitext. Rollen und Werte der Regeldatei stehen nicht in der Zeile.
- **Reihenfolge in einem Ereignis (V):** zuerst die Sammelzeilen der Flut, dann die Zeilen `abgelaufen` der Anfragen, dann der Anträge (`abgelaufen`, `verfallen`), dann der Zugänge, je aufsteigend nach der Zahl der Kennung, zuletzt die Zeile des Vorgangs.
- Die Form von `regel.geaendert` und `regel.abgelehnt` steht in Vertrag 5, Abschnitt 6.6 (B13, Frage W3 beantwortet); Regelebenen 7.1 verweist darauf. Die Stelle schreibt nur `regel.geaendert`. Vertrag 5 ist noch um die übrigen fünf Aktionsnamen dieser Tabelle zu erweitern.

## 8. Fehlercodes

Alle Codes von Fassung 1 gelten weiter. Neu:

| Code | Grund (fest, ASCII) | Vorgang |
|---|---|---|
| `stelle.server_nicht_erlaubt` | `Server fuer diesen Aufrufer nicht erlaubt` | `mcp.anfragen` |
| `stelle.zu_viele_offen` | `Zu viele offene Anfragen` | `mcp.anfragen` |
| `stelle.selbe_person` | `Dieselbe Person darf hier nicht entscheiden` | `mcp.entscheiden`, `antrag.bestaetigen` |
| `stelle.zweite_person_fehlt` | `Zweite Person fehlt` | `mcp.einloesen` |
| `stelle.rolle_fehlt` | `Rolle fehlt` | Einsicht, Pflicht, Antrag, Hersteller |
| `stelle.zweck_ungueltig` | `Zweck fehlt oder unbekannt` | `einsicht.anfragen` |
| `stelle.nonce_benutzt` | `Nachweis schon benutzt` | Vorgänge mit Nonce |
| `stelle.pflicht_unbekannt` | `Pflicht unbekannt` | `pflicht.aendern` |
| `stelle.pflicht_unveraendert` | `Wert schon gesetzt` | `pflicht.aendern` |
| `stelle.antrag_laeuft` | `Antrag laeuft schon` | `pflicht.aendern` |
| `stelle.antrag_unbekannt` | `Antrag unbekannt` | `antrag.bestaetigen`, `antrag.ergebnis` |
| `stelle.antrag_abgelaufen` | `Antrag abgelaufen` | dieselben |
| `stelle.antrag_nicht_bestaetigt` | `Antrag nicht bestaetigt` | `antrag.ergebnis` |
| `stelle.nicht_antragsteller` | `Nur der Antragsteller holt ab` | `antrag.ergebnis` |
| `stelle.bereits_abgeholt` | `Ergebnis schon abgeholt` | `antrag.ergebnis` |
| `stelle.hersteller_aus` | `Herstellerzugang aus` | `hersteller.gewaehren` |
| `stelle.zugang_aktiv` | `Zugang schon aktiv` | `hersteller.gewaehren` |
| `stelle.zugang_unbekannt` | `Zugang unbekannt` | `hersteller.zugriff`, `hersteller.beenden` |
| `stelle.zugang_abgelaufen` | `Zugang abgelaufen` | dieselben |
| `stelle.zugang_beendet` | `Zugang beendet` | dieselben |
| `stelle.regel_kette_weicht_ab` | `Regeldatei weicht von der Kette ab` | Laden |
| `stelle.rollen_ungueltig` | `Rollen der Firmendatei ungueltig` | Laden (B09) |
| `stelle.einsicht_aus` | `Einsicht aus` | `einsicht.anfragen` bei `einsicht.regel` `niemand` (B08) |
| `einst.stelle_wert` | `Wert ausserhalb der Vorgabe` | Laden |

`stelle.frist_ungueltig`, `stelle.nachweis_ungueltig` und `stelle.eingabe_ungueltig` aus Fassung 1 gelten auch für die neuen Vorgänge. Die Gründe sind Vorschlag; die Fälle prüfen Code und Grund.

## 9. Sichtbare Fälle

`faelle/` entsteht mit `node rust/spec/stelle/fassung2/bau-faelle.mjs` (Mac oder Linux-Baurechner, leicht, ohne Abhängigkeit, ohne Netz, unter 1 s). Form eines Falls wie Fassung 1, Abschnitt 9. Neu:
- `ende` trägt zusätzlich `antraege`, `zugaenge` und `pflichten` (die Werte der Pflichten am Ende).
- Ein Schritt mit wirksamer Pflichtänderung trägt in `erwartet` das Feld `datei` (`ebene`, `text`).
- Ein Neustart mit geschriebener Datei trägt `einstellungen.text` und `regel_neu`; ein Ladefall kann `regel_neu` tragen.

Das Referenzmodell baut auf dem Modell von Fassung 1 auf: Laden, Start, Form, Fristen und Zeilenprüfer sind von dort übernommen und erweitert. Das Skript prüft bei jedem Lauf zwei Dinge:

1. **Gleichlauf:** Alle 40 Fälle von Fassung 1 (`../faelle/`) laufen durch das Modell von Fassung 2. Zeilenbytes und Endzustand müssen gleich sein, Antworten bis auf die neuen Felder aus Abschnitt 1.
2. **Absicht:** Jeder Fall von Fassung 2 trägt die Absicht von Hand (Antwort, Zeilenarten je Schritt, Endzustand). Dazu prüft das Skript: Unveränderlichkeit des alten Zustands; Kanonik, geschlossene Form und Kette jeder Zeile; die Inhaltssperre (kein Argument-, Vorschau- oder Namensmarker, kein Pfad); Datei genau dann, wenn `regel.geaendert` entsteht, mit `alt` gleich dem Hash der vorigen und `neu` gleich dem Hash der neuen Datei, die gültig lädt und dieselben Pflichten wie der Zustand trägt; die Felder von `antrag.offene` und dass `antrag.bestaetigen` keine Zeilen zeigt; drei Zeilen gezielt und eine Sammelzeile Byte für Byte.

| Fall | Zeigt |
|---|---|
| `hersteller-kein-zugang` | Vorgabe `kein`: Gewähren und Zugriff abgelehnt, beides in der Kette (A11) |
| `hersteller-frist-grenzen` | Frist 0, über der Höchstfrist, `null` ohne `dauerhaft`: abgelehnt; die Höchstfrist selbst gilt |
| `hersteller-befristet-endet-zur-frist` | Zugriffe bis 1 ms vor dem Ende, jeder Zugriff als Zeile; zum Ende `abgelaufen`; Takt beendet einen zweiten Zugang |
| `hersteller-dauerhaft-und-beenden` | Grenzfall `dauerhaft`: Zugriff nach zehn Tagen, Beenden durch einen Admin, danach abgelehnt |
| `hersteller-gruende` | falsche Aufrufer, Rolle, Nachweis, Nonce, zweiter Zugang, Beenden ohne Rolle |
| `vier-augen-schreibaufruf` | erste Freigabe allein reicht nicht; dieselbe Person zweimal abgelehnt; zweite Person gibt frei; Zeile und Antwort nennen beide |
| `vier-augen-ablehnen-und-ablauf` | Ablehnen durch eine Person; Verfall nach nur einer Freigabe |
| `zweite-person-senken` | `brain_senken` mit Pflicht: dieselbe Person erteilt nicht, bleibt offen; eine zweite erteilt; `stand` und `einloesen` nennen `entscheidend` |
| `zweite-person-ablehnen-selbst` | dieselbe Person darf ablehnen |
| `zweite-person-aus-einstellungen` | Pflicht ohne Feld; Feld `false` schaltet nicht ab; `brain_schreiben` ohne Pflicht |
| `zweite-person-feld-bei-option-aus` | Option aus: keine Pflicht; Feld `true` schaltet ein, auch auf einem Server mit Freigabe aus |
| `mensch-senkt-pflicht-aus` | `brain_mensch` mit Nachweis, Pflicht aus: ohne Freigabe mit `entscheidend`, auch ohne Sitzung |
| `mensch-senkt-pflicht-an` | Pflicht an: offen, selbe Person abgelehnt, andere erteilt; `brain` sieht die Anfrage nicht |
| `modell-senkt-feste-regel` | Nachtrag 10: `brain_senken` ohne Eintrag in `zweite_person` und bei Freigabe aus ist trotzdem offen |
| `modell-senkt-immer-offen` | Art `brain`: `brain_senken` offen trotz Freigabe aus (A38); `brain_schreiben` ohne Freigabe, `entscheidend` null |
| `mensch-und-modell-gruende` | `brain` mit Nachweis; `brain_mensch` ohne, mit falschem, mit fremdem Pseudonym, mit benutzter Nonce, für Server `tickets`, beim Entscheiden |
| `verschaerfen-wirkt-sofort` | Admin schaltet `senken_zweite_person` ein, die Stelle schreibt die Datei, die nächste Anfrage folgt sofort |
| `pflicht-schwach-braucht-zwei` | A09: „schwach“ erlauben braucht zwei Admins; Einschalten darf einer |
| `keine-sammelfreigabe` | A105 (6): Liste oder Zusatzfeld in `mcp.entscheiden` ist ungültig, einzeln gelingt es |
| `pflicht-lockern-braucht-zwei` | A09: Antrag, eigene Bestätigung abgelehnt, zweiter Admin; Zeile `regel.geaendert` und Datei; wirkt nur auf neue Anfragen |
| `pflicht-freigabe-je-server` | Lockern einer Serverfreigabe verfällt ohne zweite Person; Verschärfen durch einen Admin schreibt die Datei |
| `pflicht-gruende` | Prüfreihenfolge von `pflicht.aendern` und `antrag.bestaetigen`, falsche Aufrufer; eine Person allein schreibt nichts |
| `pflicht-ueberlebt-neustart` | Neustart mit der geschriebenen Datei und `regel_neu`: die Lockerung gilt weiter |
| `einsicht-mit-rolle` | A102 mit ausdrücklichem `rolle`: mit Rolle ohne Zweck; ohne Rolle abgelehnt; Zweck nur aus der Liste; Nonce; falsche Aufrufer |
| `einsicht-vier-augen` | A102 mit `rolle-vier-augen`: ohne Zweck abgelehnt; zweite Person mit Rolle; Zweck in der Zeile |
| `einsicht-abholen` | Nachtrag 3: Abholen vor der Bestätigung, durch die zweite Person, zweimal, nach der Abholfrist abgelehnt; einmal durch den Antragsteller |
| `einsicht-antrag-verfaellt` | Antrag ohne zweite Person verfällt mit Zeile |
| `einsicht-niemand` | B08: `niemand` lehnt jede Einsicht vor dem Nachweis ab, auch mit Rolle, Zweck oder als Admin |
| `einsicht-vier-augen-nur-schreiben` | B08: `vier_augen` an, `einsicht.regel` ausdrücklich `rolle`: Einsicht ohne zweite Person und ohne Zweck |
| `einsicht-vorgabe-rolle-vier-augen` | B6, B7: `vier_augen` an, kein Feld `einsicht.regel`: die Einsicht braucht Zweck und zweite Person |
| `brain-nur-server-brain`, `brain-sonst-nichts` | Art `brain` |
| `flut-offene-je-sitzung`, `flut-ablehnungen-zusammengefasst` | Abschnitt 6 |
| `laden-stelle-*` (12, davon vier zu B08 und B09), `laden-mcp-zweite-person-lesend`, `laden-regel-absturz-nach-zeile` | Ladefehler des Blocks `stelle`, von `zweite_person` und Abweichung von der Kette |

Gemessen am 22.11.2026 (Mac, node 26.13.0, nach B08, B09 und B13): `ok: Gleichlauf 40 Faelle von Fassung 1; 47 Faelle von Fassung 2, 261 Schritte, 197 Zeilen`; `--pruefen` danach rc 0. Gegenprobe am 22.11.2026: sechs Mutanten der neuen Regeln (Einsicht folgt `vier_augen`, `niemand` ohne Wirkung, Rollen ungeprüft, `regel.geaendert` ohne `code` und `ziel`, `rollen` im Block `stelle` erlaubt, `einsicht.regel` ungeprüft) machen `--pruefen` alle rot. Am 20.11.2026 (41 Fälle) machten 55 absichtliche Fehler im Modell machen den Lauf alle rot, die letzte Runde 35 von 35 (Ergebnisdatei des Auftrags). Der gezielte Fehler „`brain_senken` folgt doch der Einstellung `zweite_person`“ wird im Fall `modell-senkt-feste-regel` Schritt 2 rot.
Gemessen am 23.11.2026 (Mac, node 26.13.0, nach Befund B6 und B7): `gleich: 48 Faelle Byte fuer Byte wie in faelle/`; `ok: Gleichlauf 40 Faelle von Fassung 1; 48 Faelle von Fassung 2, 266 Schritte, 201 Zeilen`, `--pruefen` rc 0. Gegenprobe am 23.11.2026: Die Vorgabe `rolle` statt `rolle-vier-augen` in der Vorgabetabelle und dieselbe Vorgabe in der Zuweisung des Modells machen beide den Fall `einsicht-vorgabe-rolle-vier-augen` in Schritt 2 rot (`rc 1`).

**Prüfschalter (V, Regel für jeden Antrag mit Fällen).** `node rust/spec/stelle/fassung2/bau-faelle.mjs --pruefen` erzeugt alle Fälle im Speicher neu, schreibt nichts und vergleicht Byte für Byte mit `faelle/` (gleiche Namen, gleiche Bytes). Exit 0 nur bei Gleichheit; Exit 1 nennt die abweichenden oder überzähligen Dateien; ein unbekanntes Argument ergibt Exit 64. Er läuft auf dem Merge-Ergebnis und schließt den Gleichlauf mit Fassung 1 ein.

## 10. Später

- **Gerätesignatur je Person.** Erst sie trennt zwei Menschen technisch. Bis dahin sind Vier Augen, zweite Person und Rollen so stark wie die Anmeldung an der Brücke.
- **Zuordnung von Sitzung zu Person durch die Stelle** für den Modellweg; ebenso die Bindung des Such-Prozesses an genau seine Sitzung.
- **Sicherung des Zustands.** Mit ihr gehen offene Anträge, Zugänge und Nonces nicht mehr mit einem Neustart verloren; dann braucht ein Neustart je laufendem Zugang eine Zeile über sein Ende.
- **Regelebenen:** Schreiben der Gruppendatei über `aendern`; Fassung 2 schreibt nur die Firmendatei.
- **Bereiche des Herstellerzugangs** (etwa nur Diagnose), eine Grenze für erlaubte Zugriffszeilen und Vier Augen beim Gewähren (Frage F1).
- **Ablehnen eines Antrags** durch die zweite Person (heute verfällt er); Auskunft und Löschen je Person (A12).
- **Eigentümer nach Projektentscheid:**
  - Export mit Zweck (`einsicht.exportieren`, A102, A103): Stelle, Vertrag 6, in Schritt II, nicht M1.
  - `audit.luecke` (A102): Vertrag 5, nach der Vorführung.
  - Aufbewahrung (A103): Feld `aufbewahrung_tage` im Vertrag Regelebenen, Block `protokoll` (nicht Vertrag 2, nicht die Stelle; Befund B10); die Pflege nach Frist gehört Vertrag 5.
  - Von A103 behält die Stelle nur die Einsichtsregel (Abschnitt 4.6).
- **Grenze der gemerkten Nonces und der Endzustände** im Speicher.
- Notfallzugang, Personen- und Raumvorgänge der heutigen Stelle.

## 11. Annahmen

1. **Rollen in der Firmendatei von Regelebenen** (B09) als Pseudonym → Rollen. Zwei feste Rollen genügen für Fassung 2: `admin` und `einsicht`. Der Personendienst ersetzt die Tabelle später.
2. **Bestätigen einer Einsicht** darf eine Person mit `einsicht` oder `admin`. Das Lockern einer Pflicht bestätigt nur `admin`.
3. **Vier Augen (`vier_augen`) gelten für Schreibaufrufe; die Einsicht regelt `einsicht.regel` (B08).** Beides gilt nicht für das Gewähren eines Herstellerzugangs. Das Lockern einer Pflicht braucht immer zwei Personen (A09).
4. **Zwei verschiedene Menschen** heißt zwei verschiedene Pseudonyme. Eine der beiden darf die Person der Anfrage sein, außer die Pflicht zur zweiten Person gilt.
5. **Ablehnen ist immer allein erlaubt**, mit Vier Augen und mit Pflicht zur zweiten Person.
6. **Höchstens ein aktiver Herstellerzugang**; ein Zugriff ist eine Anfrage des Rollenprogramms an die Stelle.
7. **Die Grenzen der Flut** (16 laufende Anfragen, 32 Zeilen je Art und Minute) und die Vorgaben 8 h Höchstfrist und 10 min Antrags- und Abholfrist sind Vorschlag. Alle sind Einstellungen.
8. **Zwecke** sind eine feste Liste von Schlüsseln, nie Freitext; die drei Vorgaben sind Vorschlag.
9. **Die Nonce** wählt die Brücke oder der `brain-dienst` frisch je Handlung.
10. **Beim Menschenweg** muss `person` gleich dem Pseudonym des Nachweises sein; die Herausforderung `mcp.handeln` trägt eine Nonce gegen Wiederholung.
11. **Ersetzt durch Nachtrag 10:** Nicht die Liste `zweite_person`, sondern die feste Regel für `brain_senken` (Art `brain`, Server `brain`) macht eine Anfrage im Modellweg immer offen. Andere Werkzeuge in `zweite_person` folgen im Modellweg der Freigabe des Servers, außer die Pflicht zur zweiten Person gilt.

## 12. Offene Fragen und Lücken

- **W1 (beantwortet):** Die Blöcke `mcp` und `stelle` kommen in Vertrag 2 Fassung 3; die Tabelle in Abschnitt 2.1 geht an Vertrag 2.
- **W2:** Der Vertrag Regelebenen, Abschnitt 5.1, beschreibt `mcp.<server>.lesen` und `schreiben` als Wahrheitswerte; Vertrag 6 hat Listen von Werkzeugnamen. Welche Form gilt in Fassung 3?
- **W3 (beantwortet 21.11.2026):** Eine Form für `regel.geaendert` und `regel.abgelehnt` in Vertrag 5, Abschnitt 6.6, mit `handelnd` statt `wer` (B13).
- **W4 (Halter-Vertrag):** Windows-Vorbehalt der Art `hersteller` (Abschnitt 3.1).
- **I1, I2 (beantwortet):** Abholschritt `antrag.ergebnis`; einheitliche Felder der Listen.
- **I3 (beantwortet):** Die Stelle schreibt die Regeldatei selbst (Abschnitt 4.5).
- **Q1 (beantwortet):** Die Fälle genügen für A09, A11 und die Einsicht von A102; A105 nur mit den Fällen von Fassung 1. Nachtrag 9 hat die Lücken „schwach“ und Sammelfreigabe geschlossen.
- **W5 (Halter-Vertrag):** Mit welchem Vorgang fragt der Halter `halter_bescheinigung` bei der Stelle ab, und wie lehnt er „schwach“ ohne Schalter ab?
- **F1 (Organisation):** welcher Fall gilt, wer gewährt, wer beendet; ob das Gewähren vier Augen braucht.
- **V1:** Bleibt `such-mcp` neben dem Server `brain` bestehen?
- **L1 (Lücke):** Ein Neustart beendet Zugänge und Anträge ohne Zeile (wie Anfragen in Fassung 1).
- **L2 (Lücke):** Scheitert das Schreiben der Datei nach der Zeile `regel.geaendert`, steht die Änderung in der Kette, aber nicht auf der Platte. Die Hülle behält den alten Zustand und meldet den Fehler. Der nächste Start erkennt die Abweichung und startet nicht, bis ein Admin die Datei richtet.

## 13. Hülle

Es gelten die Regeln von Fassung 1, Abschnitt 13. Neu:

- **Sockets.** Die Hülle bietet drei weitere Sockets: `brain` (Client ist der Such-Prozess der Sitzung), `brain_mensch` (Client ist das Programm `brain-dienst`) und `hersteller` (Client ist das Rollenprogramm der Fernwartung, Abschnitt 3.1). Alle liegen im selben Ordner mit Modus 0700. Die Art folgt aus Socket, Programm und Signatur (`peerkanal`).
- **Einsicht.** Ist die Antwort `einsicht_erlaubt` (bei `einsicht.anfragen` mit `einsicht.regel` `rolle` oder bei `antrag.ergebnis`), hängt die Hülle die Zeilen der Datei `<quelle>-<monat>.jsonl` als Feld `zeilen` an die Antwort (Rohbytes je Zeile als Text). Sie tut das erst, nachdem die Zeile `protokoll.einsicht` geschrieben ist (E2). Bei `bestaetigt` hängt sie nichts an.
- **Regeldatei (Nachtrag 5).** Trägt das Ergebnis das Feld `datei`, schreibt die Hülle zuerst die Zeilen (E2) und dann die Datei: Wegwerfdatei im selben Ordner, `fsync`, `rename`, `fsync` des Ordners. Unter Unix hat die Datei Modus 0600 im Ordner 0700 der Installation, Eigentümer ist die Kennung der Stelle. Unter Windows darf nur das Dienstkonto der Stelle schreiben (DACL), die Personen dürfen lesen. Scheitert das Schreiben, behält die Hülle den alten Zustand, antwortet mit `stelle.protokoll_fehler` und meldet es im Betriebsprotokoll (Lücke L2).
- **Laden.** Beim Start liest die Hülle die letzte Zeile `regel.geaendert` der Ebene aus der Kette und übergibt deren `neu` an `laden`. Ohne solche Zeile übergibt sie nichts.
- Die Hülle schreibt die Zeile `vorgang.gedrosselt` wie jede andere Zeile; die Zählung macht der Kern.

**Pflicht für den Bau der Hülle (Tests der Stufe 2, keine Kernfälle; Nachtrag 9).** Der Kern-Erzeuger kann das nicht zeigen, weil der Kern keine Ein- und Ausgabe hat. Der Bau der Hülle bringt eigene Tests mit, mindestens:
1. Die Zeilen einer Einsicht stehen erst in der Antwort, nachdem die Zeile `protokoll.einsicht` geschrieben ist; scheitert diese Zeile, enthält die Antwort keine Zeilen.
2. Ein Schreibfehler der Kette ergibt `stelle.protokoll_fehler` und lässt den alten Zustand stehen (Fassung 1, Abschnitt 13.1).
3. Ein Schreibfehler der Regeldatei nach der Zeile `regel.geaendert` lässt den alten Zustand stehen, und der nächste Start lehnt mit `stelle.regel_kette_weicht_ab` ab.
4. Die Art folgt aus Socket, Programm und Signatur; ein falsches Programm am Socket `hersteller` oder `brain_mensch` wird `unbekannt`.

<!-- vom Bauvorgang angefuegt -->
> Hinweis: Dieser Text nennt Werkzeuge, die in diesem Klon nicht liegen: `brain`.
> Sie gehoeren zum Setup der Maschine, aus der dieser Baum stammt; hier fehlen
> sie mit Absicht. Wer sie braucht, schreibt sich seine eigenen.
> *Named here, not shipped here.*
