# Vertrag `datentor`: `darf(einheit, route, kunde)`

Stand 20.11.2026, Vertrag 3 des Durchstichs (Meilensteinplanung, Abschnitt 5.4).
Grundlage sind die Anforderungen A34, A42, A43 und A50 bis A52 der Anforderungsliste sowie die Abstimmung mit Vertrag 2, Vertrag 4 und Vertrag 5 vom 20.11.
„(V)“ heißt: in diesem Vertrag festgelegt. „(H)“ heißt: aus den Anforderungen hergeleitet.

Dieser Stand enthält die sichtbaren Fälle. Verdeckte Fälle und Mutanten folgen mit dem Bau. Ein Bau gilt als vertragstreu, wenn er beide Fallsätze grün besteht.

## 1. Schnitt

Das Tor ist eine Bibliothek ohne Ein- und Ausgabe. Es liest keine Datei, öffnet keinen Socket, kennt keine Uhr und keinen Zufall.
Es bekommt eine geladene Vorgabe und je Aufruf eine Einheit, eine Route und einen Kunden. Es antwortet mit „darf“ oder „darf nicht“ und einem Grund.

- Die **Einheit** ist ein Stück Wissen, etwa eine Notiz oder ein Abschnitt. Das Tor sieht von ihr nur die Klasse.
- Die **Route** ist der Schlüssel der Route der Sitzung. Sie kommt aus dem Start (Vertrag 1), nie aus der Anfrage des Agenten.
- Der **Kunde** ist der Kunde, dem die Einheit gehört, oder keiner. Die Sitzung trägt in `darf` keinen Kunden; der Kundenraum (A53) folgt nach dem Durchstich.

Nicht im Durchstich und nicht in diesem Vertrag: Leseklasse der Person (A05), Aufteilen (A33), Vererbung, Herabstufen, Modellzonen (A127, A128), Werkzeuge je Kunde (A51, zweiter Teil), Status je Route (A45, A46).
Kommen sie dazu, wird dieser Vertrag erweitert. Kein bestehender Fall wird dabei abgeschwächt.

## 2. Vorgabe

Die Vorgabe ist die Sicht des Tors auf die Einstellungsdatei (Vertrag 2). Der Lader von Vertrag 2 füllt sie; das Tor hat keinen zweiten Stand (A44).

```json
{"klassen": ["oeffentlich", "intern", "kundendaten"],
 "routen": [{"schluessel": "cloud-a", "art": "cloud", "klasse": "intern"},
            {"schluessel": "lokal-a", "art": "lokal", "klasse": "kundendaten"}],
 "kunden": [{"schluessel": "k-a", "erlaubt": ["lokal-a"], "verboten": []}],
 "vorgabestufe": "kundendaten",
 "kundendaten_an_cloud": {"vorgabe": "nie", "freigaben": []}}
```

- `klassen`: die Klassen, **die niedrigste zuerst**, mindestens eine, ohne Doppel. Das Tor vergleicht nur Positionen und kennt keinen Klassennamen (V, wie A29). Die letzte Klasse ist die **höchste Klasse**; sie gilt als Kundendaten (V).
- `vorgabestufe`: die Klasse für eine Einheit ohne Klasse (A35, Option „Vorgabestufe“). Sie muss in `klassen` stehen. Fehlt das Feld, gilt die höchste Klasse (A35: Vorgabe ist die empfindlichste Stufe). `null` ist kein gültiger Wert (Nachtrag 20.11.2026).
- `routen`: je Route `schluessel`, `art` (`cloud` oder `lokal`) und `klasse`. Die Klasse ist die höchste Klasse, die die Route sehen darf (H, A34). Fehlt `klasse` oder ist sie `null`, hat die Route keine Klasse (A42). Die Art entscheidet nur über Abschnitt 4, D11.
- `kunden`: je Kunde `schluessel`, `erlaubt` und `verboten`, je eine Liste von Routenschlüsseln (A51). Fehlt eine Liste, gilt sie als leer. Eine Route darf in einer Liste mehrfach stehen; das ändert nichts (Nachtrag 20.11.2026).
- `kundendaten_an_cloud`: `vorgabe` hat nur den Wert `"nie"` (A50). `freigaben` ist eine Liste von Paaren `{"route", "kunde"}`; jedes Paar ist ein Schalter der Admins (A50, A52). Ein Paar darf mehrfach stehen (Nachtrag 20.11.2026). `route` und `kunde` eines Paars sind Texte; ein anderer Wert, etwa eine Liste, macht die Vorgabe ungültig (Nachtrag 20.11.2026, 3). Fehlt das ganze Objekt, gilt `"nie"` ohne Freigaben.

**Schlüssel** haben die Form `^[a-z0-9][a-z0-9._-]{0,63}$`, wie in Vertrag 1 und Vertrag 5. Das gilt für Klassen, Routen und Kunden in der Vorgabe.

**Pflichtfelder** sind `klassen`, `routen` und `kunden`, je Route `schluessel` und `art`, je Kunde `schluessel`, in `kundendaten_an_cloud` das Feld `vorgabe` und je Freigabe `route` und `kunde`.
Alle anderen Felder dürfen fehlen, mit der oben genannten Bedeutung; `freigaben` gilt dann als leer.

**Ungültig** ist eine Vorgabe (D13), wenn eine dieser Bedingungen gilt:

- Ein Feld fehlt, hat den falschen Typ oder ist unbekannt. Die Form ist geschlossen, auf jeder Ebene.
- Ein Schlüssel hat nicht die Form, oder ein Schlüssel kommt in seiner Liste doppelt vor.
- `art` ist weder `cloud` noch `lokal`, oder die Klasse einer Route steht nicht in `klassen`.
- `erlaubt` oder `verboten` nennt eine Route, die es nicht gibt.
- Eine Freigabe nennt eine unbekannte Route, eine Route der Art `lokal` oder einen unbekannten Kunden.
- `vorgabe` ist nicht `"nie"`, oder `vorgabestufe` steht nicht in `klassen`.

Mit einer ungültigen Vorgabe entsteht kein Tor. Der Aufrufer liefert dann nichts aus (V).

## 3. Antwort

```json
{"darf": false, "grund": "klasse-ueber-route", "klasse": "kundendaten", "meldung": "Klasse der Einheit liegt über Route cloud-a"}
```

- `darf`: `true` genau dann, wenn `grund` gleich `frei` ist.
- `grund`: ein Code aus der Tabelle in Abschnitt 4. Die Codes haben die Schlüsselform; Vertrag 5 kann sie in die Protokollzeile übernehmen.
- `klasse`: die **wirksame Klasse** der Einheit (D5) als Schlüssel, oder `null`, wenn die Einheit keine gültige Klasse hat. Sie steht auch bei jedem anderen Grund, damit die Protokollzeile sie nennen kann.
- `meldung`: fester Text je Grund nach der Tabelle; `{route}` steht für die Route so, wie sie übergeben wurde. Die Meldung nennt nie Inhalt, Titel, Pfad oder Kunden der Einheit.

Der Aufrufer gibt bei `darf: false` dem Modell nur die Zahl der gesperrten Einheiten weiter, nie `grund`, `klasse` oder `meldung` (A49; Vertrag 4).

## 4. Regeln

Die Prüfungen laufen in dieser Reihenfolge. Die erste zutreffende bestimmt den Grund (D12).

| Regel | Code | Wann | Meldung | Herkunft |
|---|---|---|---|---|
| D2 | `route-fehlt` | Die Route fehlt oder ist leer. | `Anfrage ohne Route` | A43 |
| D3 | `route-ohne-klasse` | Die Route steht nicht in der Vorgabe, oder sie hat keine Klasse. | `Route {route} hat keine Klasse` | A42 |
| D4 | `einheit-ohne-klasse` | Die Klasse der Einheit ist angegeben, aber kein Text oder nicht in `klassen`. Eine fehlende Klasse (`null`) ist kein Fall von D4, sie bekommt die Vorgabestufe (D5). | `Einheit ohne gültige Klasse` | (V), fail closed wie A42 |
| D6 | `klasse-ueber-route` | Die wirksame Klasse der Einheit liegt über der Klasse der Route. | `Klasse der Einheit liegt über Route {route}` | A34 |
| D7 | `kunde-fehlt` | Die wirksame Klasse ist die höchste, und die Einheit hat keinen Kunden. | `Kundendaten ohne Kunde` | (V) |
| D8 | `kunde-ohne-eintrag` | Der Kunde steht nicht in `kunden`. | `Kunde ohne Eintrag in der Kundenvorgabe` | A51 |
| D9 | `route-fuer-kunde-verboten` | Die Route steht in `verboten` des Kunden. `verboten` geht vor `erlaubt`. | `Route {route} ist für diesen Kunden verboten` | A51 |
| D10 | `route-fuer-kunde-nicht-erlaubt` | Die Route steht nicht in `erlaubt` des Kunden. | `Route {route} ist für diesen Kunden nicht erlaubt` | A51 |
| D11 | `kundendaten-an-cloud` | Die Route hat die Art `cloud`, und es gibt keine Freigabe für genau dieses Paar aus Route und Kunde. | `Kundendaten an Cloud-Route {route} ohne Freigabe` | A50, A52 |
| D1 | `frei` | sonst | `frei` | |

D7 bis D11 gelten nur für Kundendaten, also für Einheiten mit Kunde oder mit der höchsten Klasse.

- **D5, wirksame Klasse:** Hat die Einheit einen Kunden, ist ihre wirksame Klasse die höchste Klasse (V; wie `kundeHebtAufCustomer` mit Vorgabe an im alten Stack). Sonst ist es ihre eigene Klasse; fehlt sie, die Vorgabestufe (A35). Beispiel: `intern` mit Kunde `k-a` ergibt `kundendaten`, ohne Kunde `intern`.
- **D11, die Freigabe hebt die Klasse nicht:** Eine Cloud-Route bekommt Kundendaten nur, wenn alle drei Bedingungen gelten: Sie hat die höchste Klasse, sie steht beim Kunden in `erlaubt`, und es gibt die Freigabe. Es gilt der strengere Wert (H, A34). Der Schalter verlangt keinen Beleg zu einem Vertrag (A50).
- **D14, Schlüssel byte-genau:** Route, Kunde und Klasse werden byte-genau verglichen, ohne Umwandlung von Groß- und Kleinschreibung, ohne Normalisierung und ohne Kürzen von Leerraum. `K-A` ist nicht `k-a`. Ein leerer Text als Kunde ist ein Kunde und kein Sonderfall (Nachtrag 20.11.2026): Die wirksame Klasse ist die höchste (D5), und weil kein Schlüssel in `kunden` leer ist, folgt höchstens D8 `kunde-ohne-eintrag`. Vertrag 4 reicht für ein Feld `kunde`, das kein Text ist, einen leeren Text weiter; so bleibt die Einheit sicher gesperrt.
- **D15, Bibliothek:** `darf` ist eine reine Funktion. Bei gleicher Vorgabe und gleicher Eingabe gibt sie immer dieselbe Antwort. Sie löst bei keiner Eingabe eine Panik aus.

## 5. Aufruf des Außentestprogramms `datentor`

Die Außentests sehen nur das Programm `datentor`. Der Bau liefert es im Crate `datentor` mit.

- `datentor < fall.json` (oder `datentor -`): Das Programm liest genau einen Fall von stdin, höchstens 1 048 576 Byte, Verschachtelung höchstens 64 Ebenen. Das oberste Objekt ist Ebene 1; genau 64 ist erlaubt, 65 nicht (Nachtrag 20.11.2026).
- Exitcode 0: Auf stdout steht genau ein JSON-Objekt `{"antworten": [...]}` in UTF-8, eine Antwort je Anfrage in derselben Reihenfolge. Schlüsselreihenfolge und Leerraum sind frei.
- Exitcode 2: Der Fall ist ungültig. stdout bleibt leer, auf stderr steht genau eine Zeile.
  Ungültig heißt: kein JSON, kein Objekt, mehr Eingabe als erlaubt, oder ein unbekanntes Feld im Fall. Ebenso: `vorgabe` fehlt oder ist ungültig (Abschnitt 2), oder `anfragen` fehlt oder ist keine Liste.
  Ungültig ist ein Fall auch, wenn ein JSON-Objekt auf irgendeiner Ebene einen Schlüssel doppelt enthält, auch im nicht ausgewerteten Feld `beschreibung` (fail closed; Nachtrag 20.11.2026). Grund: Ein Leser, der den letzten Wert nimmt, kann eine strengere Angabe verdecken. Das gilt für jeden Lader einer Regel- oder Startdatei im Neubau (Projektentscheid, 20.11.2026).
  Ungültig ist auch eine Anfrage mit einem anderen Feld als `einheit`, `route` und `kunde`, oder wenn `einheit` fehlt oder kein Objekt ist. Ebenso, wenn `einheit` ein anderes Feld als `klasse` hat, `route` weder Text noch `null` ist oder `kunde` weder Text noch `null` ist.
- Eine Klasse der Einheit, die kein Text ist (etwa eine Zahl), macht den Fall nicht ungültig. Sie ergibt den Grund `einheit-ohne-klasse` (D4).
- Fehlt `klasse` in der Einheit, gilt sie als `null` (Vorgabestufe, D5). Fehlt `route` in der Anfrage, gilt sie als `null` (D2). Fehlt `kunde`, gilt er als `null`.
- Keine Eingabe löst eine Panik aus.

Fall:

```json
{"name": "…", "regeln": ["D3"], "beschreibung": "…",
 "vorgabe": {…},
 "anfragen": [{"einheit": {"klasse": "intern"}, "route": "cloud-a", "kunde": null}]}
```

`name`, `regeln` und `beschreibung` wertet das Programm nicht aus. Alle Daten sind erfunden.

## 6. Fälle und Läufer

| Pfad | Zweck |
|---|---|
| `faelle_bauen.py` | schreibt `sichtbar/faelle/<name>.json` und `sichtbar/erwartet/<name>.json` aus einer Tabelle mit ausgeschriebenen Erwartungen; prüft jede Erwartung zusätzlich gegen eine kleine Referenz der Regeln aus Abschnitt 4 |
| `aussen_laeufer.py` | schickt jeden Fall auf stdin an `DT_BIN`, vergleicht Exitcode und geparstes JSON mit der Erwartung; Exit 0 alle grün, 1 rote Fälle, 2 Aufruffehler |
| `sichtbar/` | die sichtbaren Fälle |

```
python3 -I -B faelle_bauen.py                      # schreibt sichtbar/ neu und prüft die Erwartungen
DT_BIN=<programm> DT_FAELLE=sichtbar python3 -I -B aussen_laeufer.py [name ...]
```

Der Läufer heißt nicht `test_*.py`, damit ihn kein Gesamtlauf ohne Programm einsammelt.

## 7. Vorgabe für den Bau (nicht durch Außentests geprüft)

```rust
pub struct Vorgabe { /* privat */ }
impl Vorgabe {
    pub fn neu(klassen: Vec<String>, vorgabestufe: Option<String>, routen: Vec<Route>, kunden: Vec<Kunde>,
               freigaben: Vec<Freigabe>)
        -> Result<Vorgabe, VorgabeFehler>;          // Abschnitt 2; Fehler nennt die erste Verletzung
}
pub struct Einheit<'a> { pub klasse: Option<&'a str> }
pub fn darf(v: &Vorgabe, einheit: &Einheit<'_>, route: Option<&str>, kunde: Option<&str>) -> Antwort;
pub struct Antwort { pub grund: Grund, pub klasse: Option<String> }
impl Antwort { pub fn darf(&self) -> bool; pub fn meldung(&self, route: Option<&str>) -> String; }
pub enum Grund { Frei, RouteFehlt, RouteOhneKlasse, EinheitOhneKlasse, KlasseUeberRoute, KundeFehlt,
                 KundeOhneEintrag, RouteFuerKundeVerboten, RouteFuerKundeNichtErlaubt, KundendatenAnCloud }
impl Grund { pub fn code(&self) -> &'static str; }
```

Prüfung des Baus: sichtbare und verdeckte Fälle grün; voller Mutationslauf (`cargo mutants`) ohne überlebende Mutanten in der Bibliothek, weil das Tor die gezeigte Sicherheitsgrenze ist (Plan 5.6, Punkt 4).
