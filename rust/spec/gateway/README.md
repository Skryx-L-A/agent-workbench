# Vertrag 7: Gateway mit Modellweg (Fassung 1)

**Entwurf Fassung 1, wieder aufgenommen 23.11.2026, nicht abgestimmt.** Abgestimmt wird mit den Bausteinen, die Abschnitt 13 nennt (Einstellungen, Start, Halter, Einschluss, Kern-Dienst, Tor, Protokollzeile); bis dahin gelten die Annahmen aus Abschnitt 11 und die Fragen aus Abschnitt 13.

Stand 23.11.2026 (Vertragsbefund B20 vom 21.11.2026: der Entwurf lag seit dem 20.11. unterbrochen; M3 und Schritt II brauchen ihn). Baustein Gateway mit Modellweg (Stufe 2) für die Zeit **nach** der ersten Vorführung.
Anforderungen: A06, A08, A13, A42, A50, A52, A85, A86, A87, A121, A127 bis A129, A133; A12 und A104 für die Kostenzeile; A142 bis A149 für macOS.
Der Vertrag legt Schnitt, Bytes und Rückgaben fest, bevor Code entsteht. Die sichtbaren Fälle stehen in `faelle/`; verdeckte Fälle und der Mutationslauf folgen mit dem Bau.
Alle Fälle sind heute rot, weil es noch keine Umsetzung gibt.

„(V)“ heißt: in diesem Vertrag festgelegt. „(H)“ heißt: aus Anforderungen oder Nachbarverträgen hergeleitet. **„Annahme X“** heißt: Die Festlegung gehört einem anderen Baustein, ist dort noch offen, und dieser Vertrag arbeitet bis zu deren Antwort damit (Abschnitt 11).

Nachbarverträge: Vertrag 1 (Start, `rust/spec/start/`), Vertrag 2 (Einstellungen, `rust/spec/einstellungen/`), Vertrag 3 (`darf`, `rust/spec/datentor/`) mit dem Teilvertrag Zonen (`rust/spec/datentor/zonen/VERTRAG.md`),
Vertrag 4 (Such-Socket, `rust/spec/such-socket/`), Vertrag 5 (Kette, `rust/spec/protokollzeile/`), Vertrag 6 (Stelle, `rust/spec/stelle/`), Vertrag 8 (Modellweg, `rust/spec/modellweg/`), Halter-Vertrag (`rust/spec/halter/`),
Einschluss unter macOS (`rust/spec/macos-einschluss/`), Plattformschicht Windows (Entwurf außerhalb des Repos), Peerkanal (`rust/spec/peerkanal/`).

Die Beispielnamen in Vertrag, Fällen und Erzeuger sind neutral (Routen `cloud-beispiel` und `lokal-beispiel`, Modelle `cloud-beispiel/modell-a` und `lokal-beispiel/modell-a`). Die echten Routen- und Modellnamen stehen nur in der Vorführdatei der Einstellungen (so auch Vertrag 8, Abschnitt 1).

## 1. Schnitt

Das Gateway besteht aus einem **Kern** ohne Ein- und Ausgabe und einer **dünnen Hülle**.

**Kern (V).** Eine Bibliothek (künftig ein eigener Crate `gateway`, noch nicht gebaut). Sie liest keine Datei, öffnet keinen Socket, kennt keine Uhr und keinen Zufall.
Sie entscheidet über einen Modellaufruf und rechnet Tokens und Kosten. Ihre Regeln:

1. **Die Route kommt aus dem Start der Sitzung, nie aus der Anfrage.** Der Kern findet die Sitzung allein über den Lauf-Schein und die Peer-Messung (Abschnitt 4). Route, Kunde und Modell nimmt er aus dem Lauf, den der Kern-Dienst aus der geprüften Startdatei (Vertrag 1) angemeldet hat.
2. **Ohne Registry-Eintrag kein Aufruf.** Hat die Route der Sitzung in der Registry keinen Eintrag für das Modell der Sitzung, bricht der Aufruf mit `gw.registry_fehlt` ab.
3. **Kein fester Anbieter, kein fester Rückfall (A13).** Im Code steht kein Anbieter, keine Adresse und kein Modellname. Der einzige Weg ist der Anbieter des gewählten Registry-Eintrags. Scheitert er, gibt es keinen zweiten Versuch bei einem anderen Anbieter.
4. **Eine Route ohne Klasse ist gesperrt (A42).** Steht die Route der Sitzung nicht in der Einstellungsdatei, endet der Aufruf mit `gw.route_ohne_klasse` und der Meldung „Route X hat keine Klasse“. Es geht kein Byte an ein Modell.
5. **Eine Route ohne Zone ist gesperrt, solange die Option „Modellzonen“ an ist (A127).** Dann gilt auch die Laufprüfung der Zone (Abschnitt 4a).
6. **Die Art passt.** Der Anbieter muss dieselbe Art (`cloud` oder `lokal`) haben wie die Route in der Einstellungsdatei. Sonst `gw.art_unvertraeglich`. Grund: Eine lokale Route darf Kundendaten sehen (Vertrag 2); ein Cloud-Anbieter dahinter brächte sie in eine Cloud.
7. **Der Preis wählt nie eine Stufe tiefer (A87).** Mit der Option „Modellwahl nach Preis“ (Vorgabe aus) wählt der Kern nur unter den Einträgen derselben Route und derselben Stufe.

**Hülle (V).** Ein Teil des Kern-Dienstes (PLAN Abschnitt 1: ein Dienst für Kern, Gateway und Brain-Dienst). Sie
nimmt HTTP auf dem Modellweg an (Abschnitt 4), misst den Peer über die Plattformschicht, ruft den Kern, leitet an genau den Anbieter der Entscheidung weiter
(direkt oder über den Halter, Abschnitt 3), reicht die Antwort zurück und hängt die Kostenzeile an (Abschnitt 6). Die Hülle trifft keine eigene Entscheidung.

### Was die Hülle an den Kern gibt und was nicht

| an den Kern | nicht an den Kern |
|---|---|
| Wert der Kopfzeile `Authorization` (oder ihr Fehlen) | jede andere Kopfzeile, auch eine, die eine Route nennt |
| Peer-Messung als getypter Wert (Abschnitt 4) | Adresse und Port des Clients als Text |
| Pfad der Anfrage | Abfrageteil nach `?` (die Hülle lehnt einen Pfad mit `?` als `gw.pfad_unbekannt` ab) |
| Körper als Bytes | |

### Festlegung: Route in der Anfrage (V)

- **Im Körper: abgelehnt.** Ein Körperfeld `route` steht auf keiner Erlaubnisliste. Die Anfrage endet mit `gw.feld_unbekannt`; es geht kein Byte an ein Modell. Dasselbe gilt für jedes andere Feld außerhalb der Liste.
- **In einer Kopfzeile: wirkungslos.** Die Hülle reicht keine Kopfzeile außer `Authorization` an den Kern. Eine Kopfzeile wie `x-awb-route` ändert nichts; der Aufruf geht an die Route der Sitzung.
- **Im Pfad: abgelehnt.** Es gibt nur die zwei festen Pfade aus Abschnitt 2; jeder andere ist `gw.pfad_unbekannt`.
- **Das Feld `model` zählt nicht.** Der Kern setzt das Modell aus Start und Registry; die Hülle überschreibt `model` im Körper damit. Die Anfrage kann kein anderes Modell erzwingen.

Gründe: Ein Feld im Körper ist eine sichtbare Selbstauskunft und wird laut abgelehnt. Kopfzeilen auszuwerten hieße, eine Liste von Namen zu pflegen; die Hülle liest sie deshalb gar nicht.

## 1a. Schnitt gegen Vertrag 8 (Modellweg)

Vertrag 8 (`rust/spec/modellweg/`) ist die **dünne Form** dieses Vertrags und wird im Durchstich gebaut: ein Weiterreicher je Sitzung, außerhalb des Einschlusses, mit zwei Betriebsarten (`openai-lokal` und `kopf-einsetzen`), ohne Registry, ohne Lauf-Schein und ohne Kosten in Geld.
Dieser Vertrag beschreibt die volle Form. Der Schnitt zwischen beiden (V, nach Vertrag 8, Abschnitte 1, 8 und 18):

| Sache | Vertrag 8 (Modellweg, heute) | Vertrag 7 (Gateway, dieser Vertrag) |
|---|---|---|
| Prozess | ein Prozess je Sitzung, von „Lauf starten“ gestartet | Teil des Kern-Dienstes, ein Prozess je Rechner |
| Ziel | genau ein Ziel und ein Modell aus dem Aufruf (Betriebsart 1) oder eine Zielliste (Betriebsart 2) | Registry mit Anbietern, Routen, Modellen, Stufen und Preisen (Abschnitt 3) |
| Aufrufer | der Harness der eigenen Sitzung, geschützt durch Socket und Netz-Namensraum | Lauf-Schein **und** Peer-Messung je Verbindung (Abschnitt 4) |
| Schlüssel | eine Schlüsseldatei, die der Weiterreicher selbst liest | nie beim Gateway; nur beim Halter (Abschnitt 3) |
| Kosten | keine; nur `token_ein` und `token_aus` in der Zeile | Kostenzeile in Geld (Abschnitt 6, A85) |
| Zeilenart | `modellweg.aufruf` | `modell.aufruf` |
| Kette | Quelle `gateway`, Datei `gateway-<JJJJ-MM>.jsonl` | dieselbe Quelle und dieselbe Kette |

- **Eine Sitzung hat genau einen Weg nach außen (V).** Entweder den Weiterreicher nach Vertrag 8 oder das Gateway nach diesem Vertrag, nie beide. Welcher gilt, entscheidet die Installation, nicht die Anfrage.
- **Betriebsart 1 `openai-lokal` geht in dieses Gateway über (V),** sobald es gebaut ist: Ein Harness mit einstellbarer Basisadresse spricht dann den Modellweg des Gateways an (Abschnitt 4), und die Zeile wird `modell.aufruf` mit Kosten.
- **Betriebsart 2 `kopf-einsetzen` bleibt bei Vertrag 8 (V, H aus Abschnitt 8).** Ein Harness mit eigenem Dienst (Copilot CLI) lässt sich nicht auf das Gateway umlenken; für ihn bleibt der Weiterreicher je Sitzung mit TLS-Abschluss und Kopf-Tausch zuständig. Dieser Vertrag ändert daran nichts.
- **Beide Zeilenarten bleiben nebeneinander (V).** `route`, `sitzung`, `modell`, `token_ein`, `token_aus`, `zeit` und `vorher` haben in beiden dieselbe Form und Bedeutung; eine Auswertung je Route, Sitzung und Modell liest beide. Die Felder `status` und `grund` übernimmt dieser Vertrag aus Vertrag 8 in `modell.aufruf` (Abschnitt 6; so vorgesehen in Vertrag 8, Abschnitt 8).
- **Die Leck-Sperre und die Fristen** aus Vertrag 8, Abschnitte 6 und 7, gelten für die Hülle dieses Gateways sinngemäß weiter; eigene Werte legt der Bau der Hülle fest (OPEN 10).

## 2. Schnittstelle des Kerns

```
entscheide(lage: &Lage, anfrage: &Anfrage) -> Result<Weiterleitung, Fehler>
tokens_aus(protokoll, strom: bool, antwort: &[u8]) -> Option<Tokens>
```

`Lage` hält: das Ergebnis des Laders von Vertrag 2 (`Einstellungen` oder dessen Fehler), das Ergebnis des Registry-Laders (Abschnitt 3), die Sicht auf die Modellzonen (Abschnitt 4a) und die Tabelle der angemeldeten Läufe.
Ein Lauf hat `sitzung`, `route`, `kunde` (Kennung oder `null`), `modell` (alle aus der geprüften Startdatei), `schein_sha256` und `peer` (die erwartete Peer-Messung).
Der Kern-Dienst meldet einen Lauf vor dem Start des Harness an und am Kindende ab (`lauf_anmelden`, `lauf_abmelden`); der Kern hält den Schein nur als sha256.

`Anfrage` hat `authorization` (Text oder fehlt), `verbindung` (Peer-Messung), `pfad` und `koerper` (Bytes).

`Weiterleitung` hat genau diese Felder:

| Feld | Bedeutung |
|---|---|
| `sitzung`, `route`, `kunde` | aus dem Lauf |
| `anbieter` | Schlüssel des Anbieters in der Registry |
| `weg` | `halter` oder `direkt` |
| `halter` | Ziel-Kennung beim Halter, oder `null` beim direkten Weg |
| `ziel` | volle Adresse beim direkten Weg (Ziel des Anbieters plus `pfad`), `null` beim Weg über den Halter |
| `pfad` | `/v1/messages` (Protokoll `messages`) oder `/v1/chat/completions` (Protokoll `chat`) |
| `protokoll` | `messages` oder `chat`, aus dem Pfad der Anfrage |
| `modell` | das Modell, das die Hülle in `model` einsetzt |
| `strom` | `true`, wenn der Körper `"stream": true` trägt; dann setzt die Hülle bei `chat` `stream_options.include_usage` auf `true` |
| `preis_ein`, `preis_aus`, `waehrung`, `einheit` | aus der Registry, für die Kostenzeile |
| `kosten_mitschreiben` | Wert der Option; bei `false` hängt die Hülle keine Zeile an |

**Pfade der Anfrage** (aus `the previous implementation`, Gruppe `gateway-sonderweg`): `POST /gw/v1/modelle/v1/messages` (Protokoll `messages`) und
`POST /gw/v1/modelle/chat/completions` (Protokoll `chat`). `completions` und `models` gehören nicht zu Fassung 1.

### Prüfreihenfolge von `entscheide`

Der erste zutreffende Schritt nennt den Code. Codes sind feste Texte; die Fälle prüfen den Code, keinen Satz.

| Stufe | Code | Verletzung | HTTP | Grund (fest, ASCII) |
|---|---|---|---|---|
| 1 | `gw.einstellungen_ungueltig` | der Lader von Vertrag 2 meldet einen Fehler (etwa `einst.route_ohne_klasse`) | 503 | `Einstellungen ungueltig` |
| 2 | `gw.registry_ungueltig` | der Registry-Lader meldet einen Fehler | 503 | `Registry ungueltig` |
| 3 | `gw.schein_fehlt` | keine Kopfzeile `Authorization`, oder sie beginnt nicht genau mit `Bearer ` | 401 | `Lauf-Schein fehlt` |
| 4 | `gw.schein_unbekannt` | der Schein hat nicht die Form `^wbs_[A-Za-z0-9_-]{43}$`, oder kein angemeldeter Lauf trägt seinen sha256 | 401 | `Lauf-Schein ungueltig` |
| 5 | `gw.peer_unbekannt` | die Peer-Messung ist `unbekannt` | 403 | `Aufrufer nicht messbar` |
| 6 | `gw.peer_falsch` | die Peer-Messung passt nicht zum Lauf des Scheins | 403 | `Aufrufer passt nicht zum Lauf` |
| 7 | `gw.pfad_unbekannt` | der Pfad ist keiner der zwei Pfade | 404 | `Weg unbekannt` |
| 8 | `gw.route_ohne_klasse` | die Route des Laufs steht nicht in der Einstellungsdatei | 403 | `Route <route> hat keine Klasse` |
| 9 | `gw.route_ohne_zone` | Option „Modellzonen“ an, und die Route des Laufs steht in keiner Zone (A127) | 403 | `Route <route> hat keine Zone` |
| 10 | `gw.zone_gesperrt` | Option an, und die Laufprüfung der Zone ist nicht frei (A128) | 403 | `Zone der Route <route> ist gesperrt` |
| 11 | `gw.registry_fehlt` | die Registry hat für die Route keinen Eintrag mit dem Modell des Laufs | 403 | `Route <route> hat keinen Registry-Eintrag` |
| 12 | `gw.art_unvertraeglich` | Art des Anbieters des gewählten Eintrags und Art der Route weichen ab | 403 | `Anbieter passt nicht zur Route` |
| 13 | `gw.protokoll_falsch` | das Protokoll des Pfads ist nicht das des Anbieters | 400 | `falsches Protokoll fuer dieses Modell` |
| 14 | `gw.koerper_zu_gross` | mehr als 2097152 Byte (`chat`) oder 33554432 Byte (`messages`) | 413 | `Koerper zu gross` |
| 15 | `gw.json_ungueltig` | kein UTF-8, BOM, Syntaxfehler, doppelter Schlüssel auf jeder Ebene, oberste Ebene kein Objekt, Text danach | 400 | `kein gueltiges JSON` |
| 16 | `gw.feld_unbekannt` | ein Feld der obersten Ebene steht nicht auf der Erlaubnisliste des Protokolls | 400 | `Feld nicht erlaubt` |

- In den Gründen der Stufen 8 bis 11 steht `<route>` für den Routenschlüssel. Er folgt dem Kennungsmuster und ist nie ein Anzeigename (A42 und A127 verlangen die Nennung der Route).
- Die Antwort an den Client ist `{"fehler":{"code":"<code>","grund":"<grund>"}}` in RFC 8785, mit dem HTTP-Status der Tabelle. Sie nennt keinen Anbieter, kein Ziel, keinen Schein, keine Zone und keinen Wert aus dem Körper.
- Die Stufen 3 bis 6 kommen vor allem anderen, das die Anfrage betrifft. Ein Aufrufer ohne gültigen Schein erfährt also nichts über Route, Zone, Registry oder Körper.
- Die Klasse kommt vor der Zone (Stufe 8 vor Stufe 9). Grund: Ohne gültige Einstellungsdatei und ohne Klasse gibt es keine Route, auf die eine Zone zeigen könnte (Fall `zone_nach_klasse`).

**Erlaubnislisten (V, übernommen aus dem heutigen Gateway, `the previous implementation`, `modell-quellen.ts:198`):**

- `messages`: `model`, `messages`, `system`, `max_tokens`, `stop_sequences`, `stream`, `temperature`, `top_p`, `top_k`, `tools`, `tool_choice`, `thinking`, `metadata`, `output_config`, `context_management`.
- `chat`: `model`, `messages`, `stream`, `stream_options`, `temperature`, `top_p`, `max_tokens`, `max_completion_tokens`, `stop`, `n`, `presence_penalty`, `frequency_penalty`, `logit_bias`, `logprobs`, `top_logprobs`, `tools`, `tool_choice`, `parallel_tool_calls`, `response_format`, `seed`, `user`, `functions`, `function_call`, `reasoning_effort`.

Die Prüfung tiefer Ebenen (etwa `metadata` nur mit `user_id`) und Zusatzfelder je Anbieter übernimmt eine spätere Fassung (OPEN 6). Fassung 1 prüft nur die oberste Ebene und doppelte Schlüssel überall.

### Wahl des Eintrags

1. Der Kern sucht in der Registry die Route des Laufs und darin den Eintrag mit `modell` gleich dem Modell des Laufs. Fehlt er: Stufe 11.
2. Ist `modellwahl_nach_preis` aus (Vorgabe), gilt dieser Eintrag.
3. Ist die Option an, wählt der Kern unter den Einträgen **derselben Route mit derselben `stufe`** den mit der kleinsten Summe `preis_ein + preis_aus`; bei Gleichstand den zuerst gelisteten.
   Ein kostenfreier Eintrag (beide Preise 0) geht damit vor. Ein Eintrag einer anderen Stufe ist nie Kandidat (A87).
4. Die Stufen 12 und 13 prüft der Kern am gewählten Eintrag.

## 3. Registry und Schlüssel beim Halter

### Registry (Annahme R)

Vertrag 2 hat auch in Fassung 2 keine Modell-Registry: Eine lokale Route trägt dort genau ein `ziel` und ein `modell` (`einstellungen/README.md`, Abschnitt „Fassung 2“), mehrere Modelle, Stufen und Preise gibt es nicht.
Bis die Einstellungen die Registry festlegen, liest das Gateway eine **eigene Datei** in der Form unten. Wird die Registry Teil der Einstellungsdatei (eine neue Fassung von Vertrag 2), ändert sich nur der Lader, nicht der Kern. Die Beispielform steht in `registry.beispiel.json`.
Solange beide Dateien ein Ziel nennen, gilt für eine lokale Route (V): Die Registry ist die Quelle des Gateways; weicht `ziel` oder `modell` der Einstellungsdatei davon ab, ist das ein Fehler der Einrichtung, den der Prüfbefehl der Installation meldet (Frage 1, Abschnitt 13).

```json
{"fassung": 1, "waehrung": "EUR", "einheit": 9,
 "optionen": {"kosten_mitschreiben": true, "kostenauswertung": false, "modellwahl_nach_preis": false},
 "anbieter": [
  {"schluessel": "anb-cloud-a", "art": "cloud", "protokoll": "messages", "ziel": null, "halter": "anb-cloud-a"},
  {"schluessel": "anb-lokal", "art": "lokal", "protokoll": "chat", "ziel": "http://127.0.0.1:11434", "halter": null}],
 "routen": [
  {"route": "cloud-beispiel", "modelle": [{"modell": "cloud-beispiel/modell-a", "anbieter": "anb-cloud-a", "stufe": "stufe-1", "preis_ein": 3000, "preis_aus": 15000}]},
  {"route": "lokal-beispiel", "modelle": [{"modell": "lokal-beispiel/modell-a", "anbieter": "anb-lokal", "stufe": "stufe-1", "preis_ein": 0, "preis_aus": 0}]}]}
```

Die Namen sind neutrale Beispiele, keine Produktvorgabe.

| Feld | Typ | Grenzen | Bedeutung |
|---|---|---|---|
| `fassung` | Ganzzahl | genau `1` | |
| `waehrung` | Text | `^[A-Z]{3}$` (ISO 4217) | Währung aller Preise und Kosten |
| `einheit` | Ganzzahl | 0 bis 9 | Preise und Kosten zählen in 10^-`einheit` der Währung; mit 9 also in Milliardsteln |
| `optionen` | Objekt | drei Wahrheitswerte, alle Pflicht | `kosten_mitschreiben` (A85, Vorgabe an), `kostenauswertung` (A86, Vorgabe aus), `modellwahl_nach_preis` (A87, Vorgabe aus) |
| `anbieter[]` | Liste | 1 bis 32 | `schluessel` (Kennung), `art` (`cloud`, `lokal`), `protokoll` (`messages`, `chat`), `ziel` und `halter` (siehe unten) |
| `routen[]` | Liste | 1 bis 64 | `route` (Kennung, Routenschlüssel aus Vertrag 2), `modelle` (1 bis 16) |
| `modelle[]` | Objekt | | `modell` (Form wie `modell` in Vertrag 1), `anbieter` (Kennung eines Anbieters), `stufe` (Kennung), `preis_ein` und `preis_aus` (Ganzzahl 0 bis 1000000, je Token in 10^-`einheit` der Währung) |

- **Genau ein Weg je Anbieter.** Entweder `ziel` (direkter Weg) oder `halter` (Weg über den Halter) ist gesetzt, nie beide, nie keiner.
- **`halter`** ist die Kennung des Ziels in der Datei des Halters. Die Registry nennt für diesen Weg **keine Adresse**; Adresse, Host und Schlüssel kennt nur der Halter (Halter-Vertrag, Abschnitt „`modell.aufruf`“).
- **Ein Cloud-Anbieter geht immer über den Halter.** Er braucht einen Schlüssel, und den hält nur der Halter.
- **`ziel`** (nur direkter Weg) ist `https://<host>[:<port>]` ohne Pfad, Abfrage und Benutzerteil, oder `http://127.0.0.1[:<port>]` beziehungsweise `http://[::1][:<port>]` nur für `art` `lokal`. Der Port liegt zwischen 1 und 65535 ohne führende Null.
  Ein lokaler Anbieter mit Schlüssel (etwa ein selbst gehostetes Modell hinter einem Schlüssel) geht über den Halter.
- **Preisgrenze.** Mit Tokens bis 4294967295 und Preisen bis 1000000 bleiben die Kosten eines Aufrufs unter 2^53. Die Kostenzeile trägt sie deshalb exakt als JSON-Zahl.
- Die Registry enthält nie einen Schlüssel. Ein Feld wie `api_key` ist ein unbekannter Schlüssel.

**Prüfreihenfolge des Laders.** Der erste zutreffende Schritt nennt den Code.

| Stufe | Code | Verletzung |
|---|---|---|
| 1 | `reg.zu_gross` | mehr als 65536 Byte |
| 2 | `reg.json_ungueltig` | kein UTF-8, BOM, Syntaxfehler, doppelter Schlüssel auf jeder Ebene, oberste Ebene kein Objekt |
| 3 | `reg.schluessel_unbekannt` | ein Schlüssel außerhalb der Form, auf jeder Ebene |
| 4 | `reg.schluessel_fehlt` | ein Pflichtschlüssel fehlt (alle Schlüssel sind Pflicht) |
| 5 | `reg.typ_falsch` | falscher JSON-Typ; eine Ganzzahl nur als Zahlwort ohne Bruchteil und Exponent (`1.0`, `1e0` sind falsch) |
| 6 | `reg.fassung_unbekannt` | `fassung` ist nicht `1` |
| 7 | `reg.anzahl` | eine Liste außerhalb ihrer Grenzen |
| 8 | `reg.kennung_form` | `schluessel`, `halter`, `route`, `anbieter` oder `stufe` verletzt die Kennungsform |
| 9 | `reg.modell_form` | `modell` verletzt die Form aus Vertrag 1 |
| 10 | `reg.art_unbekannt` | `art` ist weder `cloud` noch `lokal` |
| 11 | `reg.protokoll_unbekannt` | `protokoll` ist weder `messages` noch `chat` |
| 12 | `reg.weg_form` | `ziel` und `halter` sind beide gesetzt oder beide `null` |
| 13 | `reg.halter_pflicht` | ein Cloud-Anbieter ohne `halter` |
| 14 | `reg.ziel_form` | `ziel` verletzt die Form, oder `http` bei einem Cloud-Anbieter oder an einer fremden Adresse |
| 15 | `reg.waehrung_form` | `waehrung` verletzt die Form |
| 16 | `reg.einheit_grenze` | `einheit` außerhalb von 0 bis 9 |
| 17 | `reg.preis_grenze` | ein Preis außerhalb von 0 bis 1000000 |
| 18 | `reg.kennung_doppelt` | ein Anbieterschlüssel oder eine Route doppelt, oder ein Modell doppelt in einer Route |
| 19 | `reg.anbieter_unbekannt` | ein Modelleintrag nennt einen Anbieter, der nicht in `anbieter` steht |

Ob eine Route der Registry in der Einstellungsdatei steht, ob sie eine Zone hat und ob die Arten passen, prüft der Lader nicht. Die Dateien kann ein Admin getrennt ändern; der Kern prüft alles bei jedem Aufruf (Stufen 8 bis 12).

### Schlüssel nur beim Halter (Annahme H)

Das Gateway hält **keinen** Anbieterschlüssel. Es liest keinen aus einer Datei, der Umgebung oder einem Schlüsselbund. Den Schlüssel setzt allein der Halter ein. Das gilt auf allen drei Systemen (A14, A93, A106, A116, A144).

Die Schnittstelle folgt dem Halter-Vertrag (`rust/spec/halter/README.md`, Stand 20.11., auf `development`):

- Das Gateway ist dort die **Brücke**, ein Rollenprogramm mit den Ops `schluessel.vorhanden` und `modell.aufruf`. Die Rolle folgt unter Windows aus der SID, unter Linux aus dem Programmpfad und unter macOS aus der Code-Signatur (Halter-Vertrag, Abschnitt 2.1).
- Je Aufruf über den Halter sendet die Hülle eine Zeile `modell.aufruf` in RFC 8785 mit `ziel` = `halter` der Weiterleitung, `methode` `POST`, `pfad` = `pfad` der Weiterleitung, `kopf` (nur `content-type` und bei `messages` `anthropic-version` und `anthropic-beta` des Clients) und `koerper_b64` (der Körper mit dem eingesetzten Modell).
  Die verbotenen Kopfnamen des Halters (`authorization`, `x-api-key`, `cookie`, `host`, `content-length` und weitere) sendet die Hülle nie; der Halter lehnte sie mit `halter.kopf_verboten` ab.
- Der Halter kennt Adresse und Schlüssel des Ziels; die Anfrage nennt keine Adresse. So kann das Gateway den Schlüssel an kein anderes Ziel lenken, auch ein fehlerhaftes oder übernommenes Gateway nicht.
- Die Antwort kommt als Strom (`beginn`, `teil`, `ende`). Die Hülle reicht Status, `content-type` und die Teile an den Client weiter.
- Ein Fehler des Halters (`halter.*`) wird zu `502` mit `{"fehler":{"code":"gw.halter","grund":"Halter lehnt ab"}}`; der Code des Halters geht nicht an den Agenten; die Kostenzeile trägt `ergebnis` `anbieter_fehler` und `grund` `gw.halter`.
- Transport: unter Linux und macOS der Unix-Socket des Halters im Ordner 0700, unter Windows `\\.\pipe\wb-halter`. Endpunkt, Peer-Messung und die Prüfung des Halters durch den Client leistet die Bibliothek `peerkanal` (Vertrag `rust/spec/peerkanal/`); das Gateway hat keine eigene Socket-Prüfung.

**Direkter Weg.** Nur für einen Anbieter mit `ziel` (immer `lokal`, ohne Schlüssel). Die Hülle sendet keine Kopfzeile `Authorization`, folgt keiner Umleitung (ein 3xx ist ein Anbieterfehler) und öffnet keine andere Adresse als `ziel`.

**Fehlerantworten des Anbieters.** Bei einem Status außer 2xx gibt die Hülle nur den Status zurück, mit dem Körper `{"fehler":{"code":"gw.anbieter_fehler","grund":"Anbieter lehnt ab"}}`. Den Körper des Anbieters verwirft sie (wie heute, `the previous implementation`).

## 4. Wer ruft: Lauf-Schein und Peer

Den Modellweg erreicht nur der Harness eines angemeldeten Laufs. Zwei Sperren gelten zugleich: der **Lauf-Schein** und die **Peer-Messung**. Keine ersetzt die andere.

**Lauf-Schein (V).** `wbs_` und 43 Zeichen Base64url aus 32 Byte Zufall des Betriebssystems. Der Kern-Dienst erzeugt ihn je Lauf beim Start. Der Kern kennt nur seinen sha256 und vergleicht in konstanter Zeit.
Der Harness bekommt ihn nur in seiner Umgebung (Claude-artige Harnesses: `ANTHROPIC_AUTH_TOKEN`, OpenAI-artige: `OPENAI_API_KEY`), nie in einer Datei im Einschluss, nie in argv.
Der Schein ist kein Anbieterschlüssel. Wer ihn liest, kommt ohne passenden Peer nicht weiter. Am Kindende meldet der Kern-Dienst den Lauf ab; danach ist der Schein `gw.schein_unbekannt`.
**Grenze, ehrlich (H):** Der Schein steht in der Umgebung des Harness und ist für den Agenten lesbar; unter macOS liest ihn auch ein Prozess derselben Kennung außerhalb des Profils (`macos-einschluss/README.md`, Abschnitt „Peer-Prüfung“). Die Peer-Messung ersetzt den Schein nicht, sie ergänzt ihn.
Vertrag 1 kennt den Schein heute nicht: Er setzt bei `pi` einen festen Text (`apiKey` `wb-ohne-schluessel`) und bei `copilot` einen Platzhalter (`start/README.md`, Abschnitt „Startform je Harness“). Wie der Schein von Kern-Dienst zu „Lauf starten“ kommt, legt Vertrag 1 noch nicht fest (Annahme S, Abschnitt 11).

**Linux: Kennung am Socket (V).**

- Das Gateway hört je Sitzung auf einem eigenen Unix-Socket `modell.sock` im Sitzungsordner (Modus 0700, außerhalb des Einschlusses, derselbe Ordner wie der Such-Socket aus Vertrag 1).
  Der Socket hat Modus 0600 und wird per Bind am festen Gastpfad `/run/wb/modell.sock` in den Einschluss gebracht. Der Ordner wird nie eingebunden.
- Die Peer-Messung ist `{"art":"unix","socket":<sitzung des Sockets>,"uid":<SO_PEERCRED uid>}`. Der Lauf erwartet seinen eigenen Socket und die Kennung des Kern-Dienstes (in Fassung 1 dieselbe wie die des Agenten, Vertrag 1).
  Ein Schein, der über den Socket einer anderen Sitzung kommt, ist `gw.peer_falsch`. Damit trennt der Socket die Sitzungen, der Schein die Läufe.
- Harnesses sprechen HTTP über TCP. Im Einschluss hört deshalb ein kleines Relais auf `127.0.0.1:<port>` **im eigenen Netz-Namensraum des Einschlusses** und reicht an `/run/wb/modell.sock` weiter.
  Dieser Loopback gehört nur dem Einschluss; kein Prozess außerhalb erreicht ihn. Das Relais trifft keine Entscheidung; tötet der Agent es, schneidet er sich nur selbst ab. Das Relais baut das Projekt mit dem Einschluss (OPEN 3).
- **Kein ungeschützter TCP-Dienst auf dem Loopback des Rechners.** Das Gateway öffnet unter Linux keinen TCP-Port auf dem Loopback des Rechners für den Modellweg.

**Windows: Lauf-Schein und Peer über die Plattformschicht (H, Entwurf der Windows-Plattform, Ablauf Schritt 5; A121).**

- Unter Windows erreicht ein Agent im AppContainer nur Loopback-TCP. Der Modellweg hört deshalb auf `127.0.0.1:P` und `[::1]:P`; geschützt ist er je Verbindung, nicht durch die Adresse.
- Je Verbindung fragt die Hülle den Plattformdienst nach dem Peer (lokaler und ferner Port hinein; PID, Startzeit, AppContainer-SID, Lauf und „im Job“ heraus). Sie prüft vorher den Besitzer der Pipe-Instanz (OWNER = Dienst-SID).
- Die Peer-Messung ist `{"art":"tcp","sid":…,"job":…,"lauf":…,"im_job":true|false}`. Passt sie nicht zum Lauf des Scheins, oder ist `im_job` nicht `true`, gilt `gw.peer_falsch`.
- Antwortet der Plattformdienst nicht binnen 500 ms, ist der Client schon fort oder hat die Pipe den falschen Besitzer, ist die Messung `{"art":"unbekannt"}` und der Aufruf `gw.peer_unbekannt` (gemessen in der Probe: angehalten und gestoppt abgelehnt).
- Zwei Läufe derselben SID trennt allein der Modellweg über Schein und Job (gemessen, Frage 11: der fremde Schein je mindestens 42 von 42 abgelehnt).

**macOS: Weiterreicher je Sitzung, Peer über libproc und Merkmal (V, nach A142 bis A145, A149 und `rust/spec/macos-einschluss/README.md`).** Pflicht für Schritt II (Projektentscheid, 20.11.2026).

- **Seatbelt filtert keinen Rechnernamen.** Ein Lauf erreicht nach außen nur die **Sitzungsports** seines Profils (A145): `localhost:<modellweg>` und, bei Cloud-Zielen, `localhost:<ausgang>`. Jedes andere Ziel, DNS und jeder andere Unix-Socket sind verweigert. Der Modellweg-Port einer anderen Sitzung ist unerreichbar (gemessen, M5).
- Das Gateway hört darum auf einem **Unix-Socket im 0700-Ordner des Dienstes** (Modus 0600), nicht auf dem Loopback des Rechners. Je Sitzung stellt ein **Weiterreicher** des Starters den Socket auf `127.0.0.1` und `[::1]` bereit: Port 0 binden, denselben Port auf beiden Adressen, dann genau eine Zeile `{"bereit":true,"port":<n>}` und LF auf stdout, Frist 10 s (`macos-einschluss`, Abschnitt „Weiterreicher und Sitzungsports“). Der Starter prüft mit libproc, dass beide Hörer dem Weiterreicher gehören, den er gestartet hat.
- **Jeder Socketpfad ist höchstens 103 Zeichen lang** und liegt in einem kurzen Ordner (A149). Jede Pfadgrenze vergleicht den aufgelösten Pfad (`/var`, `/tmp`, `/etc` sind Verweise nach `/private/…`).
- **Peer-Messung (A143).** Weil der Loopback des Rechners allen Prozessen offensteht, prüft das Gateway je Verbindung: Es ordnet die Verbindung über libproc einer PID zu und liest, ob diese PID das **Merkmal des Laufs** trägt (die Datei `kennung` im 0700-Ordner des Dienstes, die nur das Profil dieses Laufs beschreiben darf).
  Die Messung ist `{"art":"mac","pid":…,"merkmal":true|false,"sitzung":<sitzung des Merkmals>|null,"uid":<LOCAL_PEERCRED uid>}`.
  Der Lauf erwartet `merkmal` `true`, seine eigene `sitzung` und die eigene `uid`. Weicht etwas ab, gilt `gw.peer_falsch`; scheitert die Zuordnung oder ist der Client schon fort, gilt `{"art":"unbekannt"}` und `gw.peer_unbekannt`.
  Gemessen (Messlauf 2c, Sonde über libproc): Eine Verbindung aus dem Lauf wird einer PID mit Merkmal zugeordnet (0,008 s), eine von außerhalb einer PID ohne Merkmal (0,007 s). Ein Weiterreicher, der das je Verbindung nutzt, ist nicht gebaut (`macos-einschluss` OPEN 1).
- **Zwei Läufe derselben Person und desselben Projekts trennt allein der Modellweg** über Schein und Merkmal; das Profil trennt sie nicht (A145).
- **Schlüssel (A144, Annahme M).** Das Gateway hält auch unter macOS keinen Schlüssel. Ein Cloud-Anbieter geht über den Halter, und der Halter läuft unter einem **eigenen, versteckten Dienstkonto** (Arbeitsname `_wbhalter`, ohne Anmeldung, ohne Shell) und hält das Geheimnis in einer Datei: Eigentümer das Dienstkonto, Modus 0600, Ordner 0700.
  Das ist Pflicht vor Schritt II (Pflicht M1 und M2, Stand 21.11.: „Der Schlüsselteil liegt beim Halter unter eigenem Dienstkonto“). Der Halter-Vertrag beschreibt in Fassung 1 dagegen einen launchd-Agenten **der Person** mit Schlüsselbund-Eintrag (Abschnitt 2.4) und nennt den Daemon unter eigenem Konto als spätere Fassung (dort OPEN 9). Dieser Vertrag arbeitet mit dem Dienstkonto; die Entscheidung gehört dem Halter-Vertrag (Annahme M, Abschnitt 11; Frage 2 in Abschnitt 13).
  **Kein Rückweg:** Es gibt keinen Schalter, keine Option und keinen Fehlerfall, der einen Anbieterschlüssel in eine Datei der Person, in die Umgebung eines Prozesses der Person oder in den Schlüsselbund der Person legt. Fehlt der Halter oder lehnt er ab, endet der Aufruf mit `502` und `gw.halter`.
- **Prozessende (A142, schwächere Zusage).** Der Dienst hält das Wächteramt: SIGTERM an Prozessgruppe und Merkmalsträger, nach der **Frist von 5 s** SIGKILL (`macos-einschluss`, Abschnitt „Prozessende“; Ergänzung 22.11. zu A142). Stirbt der Wächter durch SIGKILL, können Nachfahren bis zum nächsten Start des Dienstes leben; der Dienst räumt beim nächsten Start auf. Für das Gateway heißt das: Ein Lauf ohne gültiges Merkmal und ohne angemeldeten Lauf bekommt keine Antwort, auch wenn sein Prozess noch lebt (`gw.schein_unbekannt` nach dem Abmelden).
- **Nicht in Fassung 1:** die Zuordnung über `LOCAL_PEERTOKEN` am Unix-Socket des Dienstes für andere Rollen (Halter, Stelle) — die legt `peerkanal` fest, nicht dieser Vertrag.

## 4a. Modellzonen (A127 bis A129)

Die Zonen entscheidet das Tor, nicht das Gateway (Teilvertrag `rust/spec/datentor/zonen/VERTRAG.md`, auf `development`). Das Gateway ist die zweite Sperre je Aufruf.

- **Option „Modellzonen“** (A127, Vorgabe aus, „eine Vorgabe ist nie an“). Fehlt das Feld in der Einstellungsdatei, setzt der Lader `false` (Teilvertrag, Abschnitt 2).
- **Sicht des Gateways (V).** Die Lage trägt je Route das Ergebnis der **Laufprüfung** des Tors (`darf_zonen` mit `einheit: null`), das der Kern-Dienst beim Start der Sitzung und nach jeder Änderung der Einstellungsdatei berechnet:

  ```json
  {"an": true, "routen": {"cloud-beispiel": {"zone": "z-cloud", "lauf_frei": true, "grund": null},
                          "lokal-beispiel": {"zone": "z-lokal", "lauf_frei": true, "grund": null}}}
  ```

  `zone` ist der Zonenschlüssel oder `null`; `lauf_frei` ist das `darf` der Laufprüfung; `grund` ist der Zonengrund des Tors (`route-ohne-zone`, `zone-gesperrt`, `nutzer-fehlt`, `zone-klasse`, `zone-nachrangig`, `regel-nutzer`, `regel-gruppe`) oder `null`.
- **Zwei Stufen (V).** Ist die Option an und hat die Route keine Zone, gilt `gw.route_ohne_zone` (Stufe 9, A127, wie Z2 im Teilvertrag). Ist die Laufprüfung nicht frei, gilt `gw.zone_gesperrt` (Stufe 10, A128, wie Z4). Beides vor jedem Byte an ein Modell.
- **Ist die Option aus, fragt das Gateway keine Zone** und verhält sich wie in Fassung 1 ohne Zonen (Fall `zonen_aus`).
- **Das Gateway wertet keine Klassen und keine Freigaben aus.** Welche Datenklassen eine Zone sehen darf, welche Ausnahme zur Evaluierung gilt und welcher Grund im Protokoll steht, entscheidet das Tor vor der Ausgabe der Wissenssuche (A128). Das Gateway kennt den Kunden einer Wissenseinheit nicht (Abschnitt 7).
- **Die Ablehnung beim Start einer Sitzung** (A129: eine Sitzung über eine Route, deren Zone die Klasse der Daten nicht sehen darf, startet nicht) gehört Vertrag 1 und dem Tor. Anzeige von Name und Präfix der Zone gehört der Oberfläche. Das Gateway liefert dafür keine Daten.
- **Annahme Z (Abschnitt 11):** Die Zonen stehen in der Einstellungsdatei einer neuen Fassung von Vertrag 2 (dort OPEN 4), und der Kern-Dienst rechnet die Laufprüfung. Eigentümer: Tor, Einstellungen, Kern-Dienst.

## 5. Tokens zählen (V)

`tokens_aus(protokoll, strom, antwort)` liest die Tokens aus der Antwort des Anbieters. Ergebnis: `{token_ein, token_aus}` oder nichts („ohne Angabe“).

- `chat` ohne Strom: `usage.prompt_tokens` und `usage.completion_tokens` des JSON-Objekts.
- `chat` mit Strom: das letzte Ereignis `data: {…}` mit einem Objekt `usage`; `data: [DONE]` zählt nicht.
- `messages` ohne Strom: `usage.input_tokens` plus `usage.cache_creation_input_tokens` plus `usage.cache_read_input_tokens` (fehlende Cache-Felder zählen 0) als Eingabe, `usage.output_tokens` als Ausgabe.
- `messages` mit Strom: `message_start` liefert `message.usage`, jedes `message_delta` überschreibt die Felder, die es trägt. Am Ende gilt dieselbe Rechnung wie ohne Strom.
- Jeder Wert muss eine Ganzzahl von 0 bis 4294967295 sein (Zahlwort ohne Bruchteil und Exponent). Fehlt ein Pflichtwert, ist einer falsch oder liegt eine Summe über 4294967295, gilt „ohne Angabe“.
- Die Cache-Tokens zählen zum Preis der Eingabe (wie heute, `the previous implementation`). Eigene Preise für Cache-Tokens folgen später (OPEN 7).

## 6. Kostenzeile (V)

Je Aufruf, den die Hülle an einen Anbieter gesendet hat, entsteht genau eine Zeile in der Kette der Quelle `gateway` (Vertrag 5: Datei `gateway-<JJJJ-MM>.jsonl`, Fassung 2, RFC 8785, `vorher` über die Rohbytes der vorigen Zeile, Monatswechsel mit Ankerzeile nach Vertrag 5, Abschnitt 5).

```
{"aktionsname":"modell.aufruf","anbieter":"anb-cloud-a","einheit":9,"ergebnis":"ok","fassung":2,"grund":null,"kosten":8100000,"kunde":"k-beispiel","modell":"cloud-beispiel/modell-a","route":"cloud-beispiel","sitzung":"s-1","status":200,"token_aus":300,"token_ein":1200,"vorher":"<64 hex>","waehrung":"EUR","zeit":"<UTC, ISO 8601, Millisekunden>"}
```

| Feld | Wert |
|---|---|
| `aktionsname` | genau `modell.aufruf` |
| `anbieter` | Schlüssel des Anbieters in der Registry, nie ein Geheimnis und nie eine Adresse |
| `einheit` | aus der Registry, 0 bis 9 |
| `ergebnis` | `ok` (2xx und Tokens gelesen), `ohne_angabe` (2xx oder abgebrochener Strom ohne lesbare Tokens), `anbieter_fehler` (Status außer 2xx, Fehler des Halters, Frist) |
| `fassung` | genau `2` |
| `grund` | `null` bei `ok` und `ohne_angabe`; bei `anbieter_fehler` genau einer von `gw.anbieter_fehler`, `gw.halter`, `gw.frist` |
| `kosten` | `token_ein * preis_ein + token_aus * preis_aus` in 10^-`einheit` der Währung; `null` außer bei `ok` |
| `kunde` | Kunde der Sitzung aus dem Start oder `null` |
| `modell` | das gewählte Modell (Form wie in Vertrag 1) |
| `route`, `sitzung` | aus dem Lauf |
| `status` | der HTTP-Status, den der Harness bekommen hat (100 bis 599); `null` nur bei `anbieter_fehler`, wenn keine HTTP-Antwort möglich war (Frist, Abriss vor dem Status) |
| `token_ein`, `token_aus` | Ganzzahl 0 bis 4294967295 bei `ok`, sonst `null` |
| `vorher` | sha256 hex klein der vorigen Zeile; die erste Zeile einer Kette trägt 64 Nullen |
| `waehrung` | aus der Registry |
| `zeit` | UTC mit Millisekunden und `Z`, wie in Vertrag 5 |

- **Geschlossene Form.** Genau diese siebzehn Felder. Die Zeile trägt keinen Prompt, keine Antwort, keinen Pfad, keinen Titel, keinen Schein, keinen Schlüssel, keine Zone und keinen Personennamen (A12, A104).
  `kunde`, `route`, `sitzung` und `anbieter` sind Schlüssel nach `^[a-z0-9][a-z0-9._-]{0,63}$`; `modell` folgt `^[A-Za-z0-9._:/@+-]{1,128}$`; `grund` ist einer der drei festen Codes. Kein Wert braucht deshalb ein Escape.
- **`status` und `grund` sind neu in Fassung 1 dieses Vertrags** (23.11.): Vertrag 8 führt sie in `modellweg.aufruf` und sieht ihre Übernahme in `modell.aufruf` vor (Vertrag 8, Abschnitt 8). Ohne sie bliebe nicht unterscheidbar, ob ein Aufruf am Anbieter, am Halter oder an der Frist scheiterte.
- **Bibliothek.** Crate `formate`, Modul `kette`, in der Form der heutigen Zeilenarten (`zeile_abgewiesen`/`anhaengen_abgewiesen`, `rust/crates/formate/src/kette/monate.rs`):
  `zeile_aufruf(&Aufruf, vorher) -> Result<String>` und `anhaengen_aufruf(ordner, quelle, &Aufruf) -> Result<String>`, mit derselben Sperre, denselben Fehlern und derselben Fail-closed-Regel wie `anhaengen_abgewiesen`. Die Datei folgt aus `quelle` (`gateway`) und `zeit_ms` (`abschnitt`).
  `Aufruf` hat `zeit_ms`, `sitzung`, `route`, `kunde`, `anbieter`, `modell`, `ergebnis`, `grund`, `status`, `token_ein`, `token_aus`, `preis_ein`, `preis_aus`, `waehrung`, `einheit`. Die Bibliothek rechnet `kosten` selbst.
- **Fehler** `feld_ungueltig:<feld>` für den ersten falschen Wert in der Reihenfolge `route`, `sitzung`, `kunde`, `anbieter`, `modell`, `ergebnis`, `grund`, `status`, `token_ein`, `token_aus`, `preis_ein`, `preis_aus`, `waehrung`, `einheit`, `vorher`; danach `zeit_ungueltig`.
  Bei `ok` müssen beide Tokens Zahlen sein, sonst beide `null`.
- **Zeitpunkt.** Die Hülle hängt die Zeile an, wenn die Antwort zu Ende ist oder abbricht. Die Zeit übergibt sie in Millisekunden; die Bibliothek liest keine Uhr.
- **Scheitert das Anhängen,** beantwortet das Gateway jeden weiteren Aufruf aller Sitzungen mit `503` und `gw.protokoll_fehlt` („Protokoll nicht schreibbar“), bis der Kern-Dienst neu startet und `kette pruefe` über den laufenden Abschnitt grün ist. Grund: A85 verlangt Kosten ohne Lücke. Derselbe Fall heißt in Vertrag 8 `mw.protokoll_fehlt` mit Exit 74; die Angleichung der Codes gehört zu Befund B25 (Empfehlung: ein Code).
- **Option „Kosten mitschreiben“** (Vorgabe an, A85): Ist sie aus, entsteht keine Zeile.
- **Kosten je Worker und Projekt (A85) fehlen noch.** Die Zeile trägt `kunde`, aber kein Projekt und keinen Worker, weil der Start sie in Fassung 1 nicht kennt (Annahme K). A85 ist damit nur je Aufruf und je Kunde erfüllt; je Worker und je Projekt bleibt offen (OPEN 11).
- **Auswertung und Budgets** (A86, Option „Kostenauswertung und Budgets“, Vorgabe aus) gehören nicht zu Fassung 1. Ist die Option an, lehnt der Lader nicht ab, aber das Gateway wertet nichts aus und meldet beim Start „Kostenauswertung nicht gebaut“. Abfragen je Worker, Projekt und Kunde rechnen später aus den Zeilen.
- **Ein Schreiber je Quelle.** Altes und neues Gateway laufen nie gleichzeitig (`rust/spec/VERTRAEGE.md`, Quellen der Kette). Ein laufender Monat des alten Gateways (Fassung 1g) wird fortgesetzt; `faelle/kostenzeile.json` zeigt die gemischte Kette. Mehrere Schreiber derselben Quelle ordnen sich über die Sperre aus Vertrag 5; so laufen Zeilen des Weiterreichers (Vertrag 8) und des Gateways in einer Kette.
- Abgelehnte Aufrufe (Stufen 1 bis 16) schreiben in Fassung 1 keine Zeile. Eine Zeilenart dafür folgt (OPEN 8). Eine abgewiesene **Verbindung** schreibt dagegen heute schon eine Zeile: `kanal.abgewiesen` mit `kanal` `weiterreicher` (Vertrag 5, Abschnitt 6.1).

## 7. Kundendaten: was das Gateway garantiert und was nicht

Das Gateway prüft den **Inhalt** eines Aufrufs nicht. Es verlässt sich auf die Tore davor: `darf` und `darf_zonen` (Vertrag 3 mit Teilvertrag Zonen) vor der Ausgabe der Wissenssuche (Vertrag 4) und den Einschluss, der bestimmt, welche Dateien der Agent überhaupt sieht.

**Das Gateway garantiert (V):**

1. Ein Aufruf geht nur an den Anbieter des Registry-Eintrags der Route der Sitzung. Die Route stammt aus dem Start; keine Anfrage ändert sie.
2. Ein Aufruf über eine Route, die nicht in der Einstellungsdatei steht, geht nirgendwohin (`gw.route_ohne_klasse`). Mit eingeschalteten Zonen gilt dasselbe ohne Zone und bei gesperrter Zone (`gw.route_ohne_zone`, `gw.zone_gesperrt`).
3. Ohne Registry-Eintrag geht ein Aufruf nirgendwohin; es gibt keinen Rückfall.
4. Hinter einer lokalen Route steht nie ein Cloud-Anbieter und hinter einer Cloud-Route nie ein lokaler (`gw.art_unvertraeglich`).
5. Ohne gültigen Lauf-Schein und passenden Peer beantwortet der Modellweg keine Anfrage.
6. Das Gateway hält keinen Anbieterschlüssel und gibt keinen heraus, auf keinem der drei Systeme. Die Kostenzeile enthält keinen Inhalt.

**Das Gateway garantiert nicht:**

1. Dass ein Prompt keine Kundendaten enthält. Liest ein Agent auf einer Cloud-Route Kundendaten aus seinem Arbeitsordner, sendet das Gateway sie weiter. Davor schützen die Wahl des Arbeitsordners beim Start und der Einschluss, nicht das Gateway.
2. Dass die Freigabe „Kundendaten an Cloud“ (A50, A52) und die Klassenregel einer Zone (A128) eingehalten werden. Diese Regeln wendet das Tor an, bevor Wissen den Agenten erreicht. Das Gateway kennt den Kunden einer Einheit nicht.
3. Dass ein Harness seinen Dienst nicht selbst anspricht. Harnesses mit eigenem Dienst (Abschnitt 8) erreichen ihr Ziel ohne das Gateway; dort wirkt nur die Netzgrenze.
4. Dass der Anbieter mit den Daten nach den Regeln der Firma umgeht. Das Produkt prüft keinen Vertrag mit einem Anbieter (A50).
5. Dass der Lauf-Schein geheim bleibt. Er steht in der Umgebung des Harness (Abschnitt 4); die Peer-Messung trägt die Trennung.

## 8. Harness mit eigenem Dienst (etwa Copilot CLI)

Manche Harnesses sprechen nur mit ihrem eigenen Dienst und sind an das Konto der Person gebunden; Copilot CLI ist so einer (Entscheid des Betreibers, 20.11.2026: Cloud-Route der ersten Vorführung).
Ein solcher Harness lässt sich nicht auf das Gateway umlenken. Was das Gateway dort kann und was nicht (H):

- **Kann nicht:** den Aufruf vermitteln, den Schlüssel halten (das Konto der Person meldet sich selbst an), Tokens zählen, eine Kostenzeile schreiben, das Modell setzen.
- **Zuständig bleibt Vertrag 8, Betriebsart `kopf-einsetzen`:** ein Weiterreicher je Sitzung mit Zielliste, TLS-Abschluss und Kopf-Tausch. Seine Zeilen heißen `modellweg.aufruf` und laufen in derselben Kette (Abschnitt 1a).
- **Ist beteiligt nur über:** die Netzgrenze des Einschlusses und genau ein Ziel je Route. Die Netzgrenze lässt für die Sitzung nur die Adressen des Dienstes dieser Route durch; jede andere Adresse ist zu.
  Die Route stammt weiter aus dem Start; der Start lässt einen Cloud-Harness nur mit einer Cloud-Route zu (Vertrag 1, `start.harness_route_unvertraeglich`).
- **Folge für A85:** Für eine solche Route gibt es keine Kosten je Aufruf. Kosten kommen höchstens aus der Abrechnung des Dienstes. Das steht in der Anzeige der Route, nicht als stilles Nichts (OPEN 9).
- Die Ziele je Route für die Netzgrenze legen Vertrag 2 (`netzziele`) und Vertrag 8 (Zielliste) fest, nicht dieser Vertrag.

**Lokale Route.** Die lokale Route ist die OpenAI-kompatible Schnittstelle eines selbst gehosteten Modells (`protokoll` `chat`). Bis ein Modell benannt ist, ist es ein kleines Modell über Ollama auf dem Rechner (`http://127.0.0.1:11434`).
Ein Modell auf einem Rechner im internen Netz ist ein Anbieter der Art `lokal` mit `https`-Ziel oder, mit Schlüssel, über den Halter.

## 9. Durchstich: kein Gateway im Pfad

Entscheid des Betreibers, 20.11.2026: Die erste Vorführung läuft auf einem Ubuntu-Laptop (vorbereitet in einer VM mit Ubuntu 24.04 LTS), Browser auf demselben Laptop.
Die Cloud-Route ist Copilot CLI mit dem Organisationskonto der Person, die lokale Route ein selbst gehostetes Modell (bis zur Benennung: Ollama auf dem Rechner). Kein claude, kein eigener API-Schlüssel.

| Teil | in der ersten Vorführung | später (dieser Vertrag) |
|---|---|---|
| Cloud-Route | Copilot CLI spricht mit seinem Dienst; der Weiterreicher nach Vertrag 8 tauscht den Kopf, die Netzgrenze lässt nur dessen Ziele durch. Kein Gateway, kein Halter-Schlüssel, keine Kostenzeile. | Harnesses mit einstellbarer Basisadresse gehen über das Gateway und den Halter; Harnesses mit eigenem Dienst bleiben wie links (Abschnitt 8). |
| Lokale Route | Der Harness spricht über den Weiterreicher nach Vertrag 8 (Betriebsart `openai-lokal`) mit genau einem Ziel und einem Modell. | Über das Gateway: Lauf-Schein, Peer, Registry, Kostenzeile. |
| Kostenfelder | keine (nur Tokens in `modellweg.aufruf`) | Abschnitt 6 |
| Registry | keine | Abschnitt 3 (Annahme R) |
| Zonen | keine (Option aus) | Abschnitt 4a |

Damit hängt der Durchstich an keinem Teil dieses Vertrags. Die Fälle in `faelle/` bleiben rot, bis das Gateway gebaut wird.

**Was nach der Vorführung zuerst echt wird (H):** der Kern mit allen Fällen, der Registry-Lader, die Kostenzeile in `formate`, die Hülle unter Linux mit Socket je Sitzung und Relais, der direkte Weg zum selbst gehosteten Modell.
Danach: der Weg über den Halter, Windows über die Plattformschicht, macOS mit Weiterreicher und Merkmal (Abschnitt 4, Pflicht für Schritt II), Zonen (A127 bis A129), Auswertung (A86).

## 9a. Raum, Bescheinigung und Runner (A06, A08, A133)

Diese drei Kennungen stehen in der Anforderungsliste dieses Vertrags. Was er dazu sagt:

- **A06, Raum am Firmenserver (H).** Ein Lauf in einem Raum erreicht das eingestellte Modell über das Gateway und kein anderes Ziel; das tragen die Garantien 1 bis 5 aus Abschnitt 7 zusammen mit der Netzgrenze des Einschlusses. Das Gateway unterscheidet einen Lauf im Raum nicht von einem Lauf am Arbeitsrechner: Beide brauchen einen angemeldeten Lauf, den Schein und den passenden Peer. Der Raum selbst gehört dem Agenten-Träger, nicht diesem Vertrag.
- **A08, Bescheinigung je Lauf: fehlt in Fassung 1 (offen).** Weder dieser Vertrag noch der Halter-Vertrag kennt eine Bescheinigung (gesucht am 23.11.2026 in `rust/spec/halter/README.md`: kein Treffer). Der Lauf-Schein ist **keine** Bescheinigung: Er ist ein Zufallswert und sagt nichts über die Stärke eines Nachweises oder einen Dienstschlüssel.
  Vorschlag (V für diesen Vertrag, sobald die Form steht): Die `Weiterleitung` bekommt ein Feld `bescheinigung`, und die Hülle legt sie dem Aufruf `modell.aufruf` an den Halter bei; der Halter prüft, dass Modellschlüssel und Bescheinigung zum selben Lauf gehören. Die Form und „stark nur mit Dienstschlüssel“ gehören dem Halter-Vertrag. Bis dahin ist A08 für das Gateway nicht erfüllt (OPEN 13, Frage 15).
- **A133, Runner am Arbeitsrechner (H).** Ein Lauf, den der Server dem Runner vergeben hat, ist für das Gateway ein Lauf wie jeder andere. Ob eine Vergabe gültig ist, prüft der Runner; das Gateway kennt keine Vergabe. Nach einem Abbruch meldet der Kern-Dienst den Lauf ab, und der Schein ist `gw.schein_unbekannt`: Ein Prozess, der den Abbruch überlebt, bekommt keine Antwort mehr. Die Protokollzeile je vergebenem Lauf schreibt der Runner, nicht das Gateway; die Kostenzeile dieses Vertrags ersetzt sie nicht.

## 10. Sichtbare Fälle

`faelle/` und `registry.beispiel.json` entstehen mit `node rust/spec/gateway/bau-faelle.mjs` (Mac oder Linux-Baurechner, leicht, ohne Abhängigkeit).
Das Skript nimmt die Bytes aus `the previous implementation` und den Vektor `k1` aus `the previous implementation`. Die Einstellungs- und Zonenlagen baut es selbst, mit neutralen Namen; es liest die Vorführdatei der Einstellungen **nicht** mehr (sie trägt die echten Namen des Durchstichs und ändert sich mit ihm).
Ein eigenes Referenzmodell im Skript (Kern, Registry-Lader, Zählung, Kostenzeile, ein strenger JSON-Leser) rechnet jeden Fall nach; weicht es von der Absicht im Fall ab, bricht das Skript ab und schreibt nichts.
Die Kostenzeile rechnet es zweimal: mit `jcs` und mit einer festen Feldfolge von Hand. `node rust/spec/gateway/bau-faelle.mjs --pruefen` vergleicht die Dateien auf der Platte Byte für Byte mit einem neuen Lauf (Exit 0 gleich, 1 verschieden).

| Datei | Inhalt |
|---|---|
| `faelle/entscheide.json` | Ausgangslagen (Einstellungen als Text samt erwartetem Code des Laders von Vertrag 2, Registry als Text, Zonensichten, Läufe, Scheine) und 46 Fälle: Anfrage und erwartete Weiterleitung oder Code |
| `faelle/registry.json` | 28 Fälle für den Registry-Lader: Text und `gueltig` oder Code |
| `faelle/zaehlung.json` | 10 Fälle für `tokens_aus`: Protokoll, Strom, Antworttext, erwartete Tokens oder `null` |
| `faelle/kostenzeile.json` | 34 Fälle für `zeile_aufruf` (Zeile und sha256 oder Code) und 2 Ketten für `anhaengen_aufruf` mit erwartetem Inhalt und erwarteter Ausgabe von `kette pruefe` |

Die Fälle decken ab: gültiger Aufruf cloud (über den Halter) und lokal (direkt), Strom, Windows-Peer, macOS-Peer; Route im Körper (abgelehnt), in einer Kopfzeile (wirkungslos), im Pfad (abgelehnt); das Modell der Anfrage zählt nicht;
Route ohne Klasse in der Einstellungsdatei und Einstellungsdatei mit `klasse: null`; ohne Registry-Eintrag; Registry ungültig; lokale Route auf einem Cloud-Anbieter;
**Zonen:** Option aus, Option an mit freier Laufprüfung, Route ohne Zone, gesperrte Zone, Klasse vor Zone, eine gesperrte Cloud-Route ohne Wirkung auf die lokale Sitzung;
**Zielwahl:** der Admin stellt die Cloud-Route auf Anbieter B um, der Aufruf geht an B und nie an A, und ohne Eintrag für das Modell bei B fällt nichts auf A zurück;
Preis aus und an (nie die tiefere Stufe); Kosten aus; ohne Schein, falsche Art, unbekannter Schein, Anbieterschlüssel statt Schein, beendeter Lauf, Schein am fremden Socket, fremde Kennung, fremder Job, nicht im Job, Peer unbekannt;
**macOS:** Verbindung mit Merkmal, ohne Merkmal, mit dem Merkmal einer anderen Sitzung, mit fremder Kennung, ohne Zuordnung (libproc), Schein am Weiterreicher einer anderen Sitzung;
falsches Protokoll, doppelter Schlüssel tief im Körper, Körper kein Objekt, Körper 2 MiB und ein Byte, Feld außerhalb der Liste; die **Kostenzeile Byte für Byte** samt `status` und `grund`, Grenzen, Fehlerfolge und gemischter Kette 1g → 2.

## 11. Annahmen

| Kennung | Annahme | Stand 23.11.2026 | Eigentümer der Entscheidung |
|---|---|---|---|
| R | Die Registry ist eine eigene Datei in der Form aus Abschnitt 3, mit Optionen, Währung und Einheit. | **offen.** Vertrag 2 Fassung 2 trägt `ziel` und `modell` je lokaler Route, aber keine Registry, keine Stufen und keine Preise. | Einstellungen (Modell-Registry, A13), Betreiber für die Form der Preise |
| H | Das Gateway ist die Brücke des Halters und ruft `modell.aufruf` mit `ziel`, `methode`, `pfad`, `kopf`, `koerper_b64`; der Halter kennt Adresse und Schlüssel. | **beantwortet.** Der Halter-Vertrag steht auf `development` und führt die Rolle `bruecke` mit genau diesen Feldern und der Pfadliste je Ziel. | Halter-Vertrag |
| S | Der Kern-Dienst erzeugt den Lauf-Schein und gibt ihn „Lauf starten“ in einer Datei `schein` (0600) neben der Startdatei; „Lauf starten“ liest und löscht sie vor dem exec und setzt den Schein in die Umgebung des Harness. Die Startdatei selbst bleibt ohne Schein. | **offen.** Vertrag 1 kennt keinen Schein; er setzt bei `pi` einen festen Text und bei `copilot` einen Platzhalter. | Start (Vertrag 1), Kern-Dienst |
| P | Unter Linux liegt `modell.sock` im Sitzungsordner neben dem Such-Socket; das Relais im Netz-Namensraum des Einschlusses baut der Einschluss. | **offen.** Vertrag 1 startet heute den Weiterreicher nach Vertrag 8, kein Relais für das Gateway. | Einschluss, Kern-Dienst |
| K | Die Kostenzeile trägt `kunde` aus dem Start. Projekt und Worker fehlen, weil der Start sie in Fassung 1 nicht kennt. | **offen,** damit A85 nur teilweise erfüllt (OPEN 11). | Kern-Dienst, Start (Vertrag 1) |
| Z | Die Zonen stehen in einer neuen Fassung der Einstellungsdatei, und der Kern-Dienst rechnet die Laufprüfung des Tors je Route (Abschnitt 4a). | **offen.** Der Teilvertrag Zonen steht auf `development`; Vertrag 2 hat die Liste `zonen` noch nicht (Teilvertrag, Abschnitt 9). | Tor, Einstellungen, Kern-Dienst |
| M | Unter macOS hält der Halter die Schlüssel unter einem eigenen, versteckten Dienstkonto (`_wbhalter`, Datei 0600 im Ordner 0700), nicht im Schlüsselbund der Person. | **offen, aber Pflicht vor Schritt II** (Pflicht M1 und M2: Schlüsselteil unter eigenem Dienstkonto). Der Halter-Vertrag beschreibt in Fassung 1 den launchd-Agenten der Person mit Schlüsselbund. | Halter-Vertrag, Kern-Dienst |
| W | Unter macOS stellt ein Weiterreicher je Sitzung den Socket des Gateways auf einem Sitzungsport bereit und misst den Peer über libproc und das Merkmal. | **offen.** Die Zuordnung ist gemessen (Messlauf 2c); der Weiterreicher ist nicht gebaut (`macos-einschluss` OPEN 1). | Kern-Dienst, Weiterreicher (Vertrag 8), Plattformschicht |

## 12. OPEN

1. **Prüfer für `modell.aufruf`.** `kette pruefe` (Vertrag 5) prüft die geschlossene Form für `ausgabe.gesperrt` und `kanal.abgewiesen`. Für `modell.aufruf` prüft er heute nur die allgemeinen Schritte der Fassung 2. Vorschlag: dieselbe Prüfung der geschlossenen Form aus Abschnitt 6 (Frage an Vertrag 5).
2. **Startlast (A85).** Die Kosten des ersten Aufrufs einer Sitzung lassen sich aus der Kette ableiten (erste Zeile je `sitzung`). Ein eigenes Feld fehlt bewusst.
3. **Relais im Einschluss** (Annahme P): Bau im Einschluss; Port und Start des Relais sind dort festzulegen.
4. **macOS:** Weg festgelegt (Abschnitt 4), Weiterreicher und Merkmalsprüfung nicht gebaut und nicht gemessen (Annahme W). Die Frist des Wächters ist 5 s (A142, Ergänzung 22.11.).
5. **Zonen (A127 bis A129):** Schnitt festgelegt (Abschnitt 4a). Offen bleiben die Liste `zonen` in Vertrag 2 und die Herkunft von Nutzer, Gruppen und `jetzt` (Teilvertrag Zonen, Abschnitt 9).
6. **Tiefe Prüfung des Körpers** (`metadata`, Zusatzfelder je Anbieter, verbotene Felder in der Tiefe) wie im heutigen Gateway.
7. **Cache-Tokens** mit eigenem Preis.
8. **Zeile für abgelehnte Aufrufe** (`modell.abgelehnt` mit Code, ohne Inhalt).
9. **Anzeige „keine Kosten je Aufruf“** für Routen mit eigenem Dienst (Abschnitt 8).
10. **Weitere Pfade** (`completions`, `models`, `count_tokens`), Fristen (heute 10 Minuten je Aufruf) und die Zahl gleichzeitiger Aufrufe folgen mit dem Bau der Hülle; Vertrag 8 setzt heute 10 s, 600 s und 8 gleichzeitige Anfragen (dort OPEN 6, nicht gemessen).
11. **Kosten je Worker und Projekt (A85).** Fehlen, solange der Start Projekt und Worker nicht kennt (Annahme K). Entweder zwei Felder in der Zeile oder eine Zuordnung über `sitzung` in einer anderen Quelle.
12. **Ein Code für „Protokoll nicht schreibbar“** (Befund B25): `gw.protokoll_fehlt` (503) hier, `mw.protokoll_fehlt` (Exit 74) in Vertrag 8, `ordnermarke.protokoll` (73) in Vertrag 1. Empfehlung: 74 und ein Code.
13. **Bescheinigung je Lauf (A08).** Fehlt in Fassung 1 (Abschnitt 9a). Vorschlag: Feld `bescheinigung` in der Weiterleitung und im Aufruf an den Halter, Form legt der Halter-Vertrag fest.

## 13. Fragen an die Bausteine

Jede Frage nennt den Baustein, der entscheidet. Sie gehören in die Abstimmung, nicht in den Bau.

**Einstellungen, Start und Halter**

1. Registry (Annahme R): eigene Datei wie Abschnitt 3 oder Teil einer neuen Fassung der Einstellungsdatei? Und was gilt, wenn `ziel` und `modell` einer lokalen Route in Vertrag 2 von der Registry abweichen? Empfehlung: Registry in die Einstellungsdatei, ein Ort je Angabe, der Prüfbefehl meldet jede Abweichung.
2. Halter unter macOS (Annahme M): Dienstkonto `_wbhalter` mit Datei 0600 — wie A144 und die Pflichtliste vor Schritt II es verlangen — statt launchd-Agent der Person mit Schlüsselbund (Halter-Vertrag, Abschnitt 2.4, dort OPEN 9). Empfehlung: Dienstkonto, sonst hält kein Schritt-II-Lauf die Zusage „kein Geheimnis bei der Person“.
3. Lauf-Schein (Annahme S): Datei `schein` (0600) neben der Startdatei, von „Lauf starten“ gelesen und gelöscht? Empfehlung: ja, mit einem Fall in Vertrag 1.
5. Zonen in Vertrag 2 (Annahme Z): Liste `zonen` mit Vorrangfolge, Freigaben je Route, Kunde und Projekt, Ausnahmen und Regeln, wie der Teilvertrag sie erwartet.
15. Bescheinigung je Lauf (A08, Abschnitt 9a): Welche Form hat sie, und reicht das Gateway sie dem Halter mit `modell.aufruf` durch? Empfehlung: ein Feld `bescheinigung` in der Weiterleitung, geprüft beim Halter, weil nur dort Schlüssel und Bescheinigung zusammenliegen.

**Kern-Dienst und Einschluss**

4. Relais und Socket unter Linux (Annahme P): Baut der Einschluss das Relais im Netz-Namensraum, oder bekommt das Gateway unter Linux denselben Weiterreicher wie unter macOS? Empfehlung: ein Weg für beide Systeme, der Weiterreicher.
6. Wer rechnet die Laufprüfung der Zonen und gibt sie dem Gateway in die Lage (Abschnitt 4a)? Empfehlung: der Kern-Dienst beim Start der Sitzung und nach jeder Änderung der Einstellungsdatei.
7. Weiterreicher unter macOS (Annahme W): Baut der Kern-Dienst ihn mit der Peer-Messung je Verbindung (libproc, Merkmal), und gilt derselbe Weiterreicher für Suche, Modellweg und Gateway?
8. Nennt der Kern-Dienst in der Anmeldung eines Laufs Projekt und Worker (Annahme K, A85)?
9. Übernimmt der Kern-Dienst die Zeile `kanal.abgewiesen` mit `kanal` `weiterreicher` für abgewiesene Verbindungen am Modellweg, oder braucht das Gateway dafür eine eigene Zeilenart (OPEN 8)?

**Anforderungen und Protokollzeile**

10. A85 verlangt Kosten je Aufruf, Worker und Projekt. Fassung 1 liefert je Aufruf und je Kunde. Reicht das für die Abnahme von Stufe 2, oder muss A85 geteilt werden (OPEN 11)?
11. A121 nennt den Lauf-Schein nur für Windows, A143 für macOS. Unter Linux tragen Socket und Netz-Namensraum die Trennung. Soll eine Kennung den Schein für alle drei Systeme verlangen?
12. Die Codes für „Protokoll nicht schreibbar“ weichen in drei Verträgen ab (Befund B25, OPEN 12). Empfehlung: 74 und ein Code, in A-Nummern und Verträgen nachgezogen.
16. A08 nennt das Gateway als Baustein (Stufe 2), aber die Bescheinigung gibt es in keinem Vertrag (Abschnitt 9a). Gehört A08 ganz zum Halter, oder trägt das Gateway einen Teil? Empfehlung: Halter prüft, Gateway reicht durch; A08 entsprechend schärfen.

**Tor (Zonen) und Protokollzeile**

13. Teilvertrag Zonen: Das Gateway liest nur `zone`, `lauf_frei` und `grund` der Laufprüfung (Abschnitt 4a). Genügt das, oder muss das Gateway auch `protokoll.ausnahme` in eine Zeile schreiben (A128: Grund und Ausnahme stehen im Protokoll)?
14. Vertrag 5: Prüft `kette pruefe` die geschlossene Form von `modell.aufruf` mit (OPEN 1)? Und bleibt das Lückenglied `audit.luecke` (A102) beim Gateway, wie am 20.11. entschieden?

<!-- vom Bauvorgang angefuegt -->
> Hinweis: Dieser Text nennt Werkzeuge, die in diesem Klon nicht liegen: `pi`.
> Sie gehoeren zum Setup der Maschine, aus der dieser Baum stammt; hier fehlen
> sie mit Absicht. Wer sie braucht, schreibt sich seine eigenen.
> *Named here, not shipped here.*
