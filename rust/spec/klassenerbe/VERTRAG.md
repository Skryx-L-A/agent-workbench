# Vertrag `klassenerbe`: Ein Ergebnis erbt die Klasse seiner Quellen (Entwurf für M3)

Stand 20.11.2026. **Entwurf für M3** (Meilensteinplanung, Abschnitt 5.7:
„Tore voll“). Gebaut wird nicht vor dem Durchstich. „(V)“ heißt: in diesem Vertrag festgelegt; „(H)“: aus den Anforderungen
hergeleitet; „(A)“: mit dem genannten Baustein am 20.11. abgestimmt.

Der Vertrag regelt nur das Erbe der Quellen auf eine Einheit, die ein Modell schreibt. Ort und Projekt (A32), Aufteilen (A33)
und Herabstufen (A39 bis A41) regelt `rust/spec/datentor/klassen/` (entsteht; Grenze abgestimmt 20.11.). Das
Schreibtor (`rust/spec/schreibtor/`) nimmt das Erbe als Eingabe `lage.erbe`.

## 1. Begründung aus den Anforderungen

Was ein Agent aus einer Notiz ableitet und zurückschreibt, trägt mindestens deren Klasse. Das folgt aus vier Anforderungen:

1. **A34: Ein Modell bekommt Unzulässiges nie zu sehen**, weder als Treffer noch als Zitat, Titel, Pfad oder Verweis. Eine
   Zusammenfassung ist ein Zitat in anderen Worten. Schreibt ein Agent auf der lokalen Route eine Zusammenfassung einer
   Kundennotiz mit der Klasse des Orts, etwa `intern`, liefert die Suche sie danach an die Cloud-Route. Die Cloud-Route sähe
   dann den Inhalt, den ihr das Tor vorher gesperrt hat. Ohne Erbe ist A34 über einen Umweg verletzt.
2. **A50: Kundendaten gehen nie an ein Cloud-Modell**, solange kein Admin es freigibt. Derselbe Umweg trüge Kundendaten an
   die Cloud, ohne dass ein Admin schaltet.
3. **A40: Die Automatik senkt nie Kundendaten, Personenbezug, Geheimnisse** und alles, dessen Art sie nicht sicher erkennt.
   Das Zuordnen einer Klasse beim Schreiben eines Modells ist Automatik (A36: „vollautomatisch nach Regeln … keine
   Rückfrage“). Bekäme das Ergebnis eine Klasse unter der seiner Quelle, hätte die Automatik gesenkt, ohne die Art zu kennen.
4. **A39: Senken gibt es nur eng begrenzt**, um eine Stufe, nur für unkritische Arten, mit Protokoll und Rückholbefehl. Für
   ein abgeleitetes Ergebnis gibt es davon nichts: keine Art, keine Zeile, keinen Rückweg. Also darf es nicht tiefer fallen.

Daraus folgt: Die Klasse des Ergebnisses ist mindestens das Maximum der wirksamen Klassen seiner Quellen. Will die Firma ein
Ergebnis tiefer haben, gibt es zwei Wege, die beide schon gefordert sind: die Herabstufung (A39) für unkritische Arten und die
Wahl eines Menschen (A37, A38).

## 2. Woran die Quelle erkannt wird

**Quelle ist jede Einheit, die der Such-Prozess dieser Sitzung an das Modell ausgeliefert hat (V).** Der Such-Prozess sieht
jeden Treffer, den er ausliefert (Vertrag 4), und ist zugleich der Aufrufer des Schreibtors. Er führt je Sitzung eine Liste
der ausgelieferten Einheiten: Pseudonym der Einheit, wirksame Klasse und Kunde im Augenblick der Auslieferung.

- Es zählt jeder ausgelieferte Treffer, auch wenn das Modell ihn nicht verwendet. Ob ein Modell einen Treffer genutzt hat,
  lässt sich nicht messen (H). Ein gesperrter Treffer zählt nicht; aus ihm kam nichts beim Modell an (Vertrag 4, Punkt 6).
- Die Liste gilt für die ganze Sitzung bis zu ihrem Ende. Sie leert sich nicht mit der Zeit und nicht durch das Modell.
- Beim Schreiben gibt der Prozess je Quelle zwei Einträge an `erbe`: den Stand bei der Auslieferung und den Stand jetzt, wenn
  die Einheit noch besteht. So zählt eine Einheit, die nach der Auslieferung höher eingestuft wurde, mit der höheren Klasse.
  Eine gesenkte Einheit zählt mit der Klasse, die das Modell gesehen hat (V).
- Weitere lesende Wege des Brain (später etwa `brain_lesen` für eine ganze Notiz) tragen in dieselbe Liste ein.

## 3. Eingabe und Antwort

Eine Anfrage je Schreiben: `{"route": <Text>|null, "sitzungskunde": <Kennung>|null, "gelesen": [{"klasse": …, "kunde": …}]}`.

- `route`: die Route der Sitzung, aus dem Socket.
- `sitzungskunde`: der Kunde der Sitzung aus dem Start (Vertrag 1, Feld `kunde`). Er ist eine Angabe des Menschen, die nur
  gegen die Einstellungen geprüft ist; er darf deshalb nur heben (A). Der Such-Prozess bekommt ihn erst mit
  einer künftigen Fassung von Vertrag 1 und 4 (`--kunde`).
- `gelesen`: die Quellen, `klasse` und `kunde` roh wie im Frontmatter; ein fehlendes Feld gilt als `null`. Ein Pfad oder
  Titel gehört nicht hinein; jedes andere Feld macht den Fall ungültig.

Antwort: `{"ergebnis": "ok", "klasse": <Schlüssel>|null, "kunden": [<Kennung>, …], "grund": null}` oder
`{"ergebnis": "unklar", "klasse": null, "kunden": [], "grund": <Code>}`. `kunden` ist sortiert und ohne Doppel.
An das Schreibtor geht bei `ok` `{"klasse", "kunden"}`, bei `unklar` der Text `"unklar"` (S12 lehnt dann ab).

**Vorgabe:** die Vorgabe von Vertrag 3 mit dem optionalen Block `{"erbe": {"boden": "route"|"gelesen"}}`, Vorgabe `route`.

## 4. Regeln

In dieser Reihenfolge; die erste zutreffende Ablehnung bestimmt den Grund (K0).

| Regel | Wann | Ergebnis | Herkunft |
|---|---|---|---|
| K1 | Die Route fehlt, ist leer, unbekannt oder ohne Klasse | `unklar`, `route-ohne-klasse` | A42, A43 |
| K2 | Eine Quelle hat eine Klasse, die kein Text ist oder nicht in `klassen` steht; oder einen Kunden, der kein Schlüssel ist | `unklar`, `quelle-ohne-klasse` bzw. `quelle-kunde-ungueltig` | (A), fail closed |
| K3 | Boden: bei `route` die Klasse der Route, bei `gelesen` keiner | | Abschnitt 5, Offen E1 |
| K4 | Je Quelle die wirksame Klasse nach D5: eigene Klasse; ohne Klasse die Vorgabestufe; mit Kunde die höchste | | A35, D5 |
| K5 | `klasse` ist das Maximum nach Position aus Boden und K4; ohne Boden und ohne Quelle `null` | `ok` | A34, A39, A40 |
| K6 | `kunden` sind die Kunden der Quellen und der Kunde der Sitzung; ist es mindestens einer, ist `klasse` die höchste | `ok` | A50, A53, D5 |
| K7 | Form des Falls verletzt (Abschnitt 6) | Exit 2 | (V) |

- Eine Quelle mit ungültiger Klasse zählt nicht als niedrigste Klasse (A). Sie kann nur über die Route gekommen
  sein, wenn sie nachträglich falsch umklassiert wurde; dann ist die Lage unklar, und es wird nicht geschrieben.
- **Mehrere Kunden** entscheidet das Schreibtor, nicht das Erbe: Es lehnt mit `kunden-gemischt` ab (S13). Grund: Eine
  Einheit hat in `darf` genau einen Kunden. Gäbe man ihr einen der Kunden, sähen die Routen dieses Kunden die Daten des
  anderen; das wäre ein Übertrag zwischen Kundenräumen (A53). Hat der Bestand am Ziel einen anderen Kunden als eine Quelle,
  lehnt das Schreibtor ebenso ab (A).

## 5. Zusammenspiel mit dem Schreibtor, A39 und A40

- **Schreibtor:** Das Erbe ist einer von fünf Böden der automatischen Klasse (Schreibtor, Abschnitt 5: Ort, Regeln,
  Erkennung, Bestand, Erbe). Das Erbe gilt nur für `modell`. Ein Mensch, der in der Oberfläche einträgt, liest nicht über
  die Sitzung; seine Wahl gilt (A37).
- **A38:** Eine Umklassierung, die ein Mensch im Chat anweist, ist eine ausdrückliche Wahl. Das Erbe gilt für sie nicht; sie
  wirkt erst nach der Freigabe des Menschen. So bleibt Senken eine menschliche Handlung.
- **A39:** Die Herabstufung läuft nach dem Schreiben über den Bestand, nicht beim Schreiben. Ein Ergebnis mit geerbter Klasse
  kann danach um eine Stufe gesenkt werden, wenn seine Art als unkritisch gilt; es hat dann Protokoll und Rückweg. Ein Ergebnis
  einer Kundennotiz wird nie gesenkt (A40: Kundendaten und Klasse CUSTOMER).
- **A40, „was sie nicht sicher erkennt“:** Das Erbe kennt keine Art. Es hebt nur. Darum braucht es keine Art-Erkennung.
- **Erbe nach einer Herabstufung:** Hat die Automatik eine Quelle gesenkt und das Modell sie danach gelesen, erbt das Ergebnis
  die gesenkte Klasse. Wird die Herabstufung zurückgeholt, steigt das Ergebnis nicht mit (Grenze 4, Offen E4).
- **Boden der Route (K3, Vorgabe):** Er deckt, was das Erbe nicht sieht (Abschnitt 7). Er ist auch die Regel für Durchstich B
  ohne Klassenerbe: Der Such-Prozess setzt dort `erbe` = `{"klasse": <Klasse der Route>, "kunden": [<Kunde der Sitzung>]}`.

## 6. Aufruf des Außentestprogramms `klassenerbe`

`klassenerbe < fall.json`, höchstens 1 048 576 Byte. Exit 0 mit `{"antworten": [...]}`, eine Antwort je Anfrage. Exit 2 mit
leerem stdout und genau einer Zeile auf stderr bei: kein JSON, kein Objekt, zu groß, doppelter Schlüssel in irgendeinem
Objekt, unbekanntes Feld im Fall, ungültige Vorgabe (Vertrag 3, Abschnitt 2; `erbe.boden` nur `route` oder `gelesen`), eine
Anfrage nicht mit genau den drei Feldern, `route` weder Text noch `null`, `sitzungskunde` weder Kennung noch `null`,
`gelesen` keine Liste, eine Quelle mit einem anderen Feld als `klasse` und `kunde`.

| Pfad | Zweck |
|---|---|
| `faelle_bauen.py` | schreibt `sichtbar/` aus Tabellen mit ausgeschriebenen Erwartungen und prüft jede gegen eine kleine Referenz |
| `aussen_laeufer.py` | `KE_BIN=<programm> KE_FAELLE=sichtbar python3 -I -B aussen_laeufer.py [name ...]` |
| `sichtbar/` | 20 Fälle: 13 gültige mit 34 Anfragen, 7 ungültige |

Gemessen am 20.11. (Mac, Attrappe außerhalb des Repos mit der Referenz als Programm): 20 von 20 grün; ein Programm, das sofort
endet: 0 von 20. Abgewandelte Attrappen: ungültige Klasse gilt als niedrigste 2 rot; Kunde der Sitzung übergangen 3 rot;
Boden immer `gelesen` 2 rot; ein Kunde hebt die Klasse nicht 3 rot.

## 7. Grenzen

Das Erbe sieht nur, was über den Such-Prozess ausgeliefert wurde. Es sieht nicht:

1. **Wissen ohne Lesen über die Suche:** Dateien im Arbeitsordner, Eingaben des Menschen im Chat, Antworten anderer
   MCP-Server (etwa Tickets), Webseiten und das Wissen des Modells selbst. Der Boden der Route deckt davon nur den Teil, der
   aus dem Brain stammen kann. Den Arbeitsordner trennt der Einschluss (A95, A96), nicht das Erbe. Ein Kunde im Arbeitsordner
   kommt nur über den Kunden der Sitzung hinein; ohne ihn trägt das Ergebnis keinen Kunden.
2. **Umwege außerhalb des Vaults:** Ein Agent liest auf der lokalen Route eine Kundennotiz und schreibt sie in eine Datei im
   Arbeitsordner; eine Cloud-Sitzung im selben Ordner liest die Datei. Das Erbe greift nicht, weil kein Schreiben in den Vault
   stattfand. Das muss der Einschluss verhindern (eigener Arbeitsordner je Sitzung oder je Kundenraum, A53).
3. **Mehrere Sitzungen:** Das Erbe gilt je Sitzung. Was ein Mensch aus einer Sitzung in eine andere kopiert, ist seine
   Handlung.
4. **Rückholen einer Herabstufung** hebt Ergebnisse nicht mit, die aus der gesenkten Einheit abgeleitet wurden (Offen E4).
5. **Neustart des Such-Prozesses:** Die Liste der Quellen lebt im Speicher des Prozesses. Dieser Vertrag verlangt (V): Stirbt
   der Prozess, startet der Starter für dieselbe Sitzung keinen zweiten; eine neue Sitzung beginnt mit einer neuen Kennung.
   Sonst schriebe ein neuer Prozess mit leerer Liste unter der alten Sitzung. Das ist mit Vertrag 1 abzugleichen.
6. **Teile einer Notiz:** Nach dem Aufteilen (A33) ist die Quelle der ausgelieferte Teil mit seiner Klasse, nicht die ganze
   Notiz. Das Erbe rechnet mit der Klasse, die ausgeliefert wurde.

## 8. Offen

Die Entscheidungen für den Menschen (E1 bis E6) sind offen. Technisch offen: die Liste der Quellen im Such-Prozess (Fassung 2 von Vertrag 4),
`--kunde` am Such-Prozess (Vertrag 1 und 4) und die Ablage der Abstammung für E4.
