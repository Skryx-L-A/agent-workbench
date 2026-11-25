# Vertrag `schreibtor`: jedes Schreiben in den Wissensspeicher geht über ein Tor

Stand 20.11.2026 (Nachtrag E6 vom selben Tag; Nachtrag E7
„Pflichten aus der Stelle“ vom selben Tag). Entwurf vor dem Code; gebaut wird nach dem Durchstich (Abschnitt 10).
Grundlage sind die Anforderungen A15, A32 bis A40, A50 bis A53 und A89 bis A105 der Anforderungsliste, der Entwurf
zur Freigabe von MCP-Servern (am 18.11.2026 angenommen) und die Verträge 1 bis 6 dieser Runde.
„(V)“ heißt: in diesem Vertrag festgelegt. „(H)“ heißt: aus den Anforderungen hergeleitet. „(A)“ heißt: mit dem genannten
Baustein am 20.11. abgestimmt. „(E6)“ heißt: Entscheid des Betreibers, 20.11.2026 (Senken durch einen
Menschen), im Nachtrag eingearbeitet; „(V, E6)“ ist der Vorschlag dazu und noch nicht abgestimmt, wenn der Text es sagt.
„(E7)“ heißt: Entscheidung vom 20.11.: Die Stelle ist die einzige Quelle der Pflichten; das Tor
führt keine Kopie.

Dieser Stand enthält die sichtbaren Fälle, einen Fallbauer mit Referenz und einen Läufer. Verdeckte Fälle und Mutanten folgen
mit dem Bau. Ein Bau gilt als vertragstreu, wenn er beide Fallsätze grün besteht.

| Vertrag | Ablage | Was dieser Vertrag daraus nimmt |
|---|---|---|
| 1 Start-Struktur | `rust/spec/start/` | `sitzung`, `route`, `kunde` der Sitzung |
| 2 Einstellungsdatei | `rust/spec/einstellungen/` | Klassen, Routen, Kunden, Vorgabestufe; künftig den Block `schreiben` (Abschnitt 7) |
| 3 `darf` | `rust/spec/datentor/` | `darf` und die wirksame Klasse D5 (`wirksam`, `rang`) |
| 4 Such-Socket | `rust/spec/such-socket/` | den Such-Prozess der Sitzung als Ort des Tors |
| 5 Protokollzeile | `rust/spec/protokollzeile/` | Kette, RFC 8785, allgemeines Anhängen |
| 6 Entscheidungsstelle | `rust/spec/stelle/`, Fassung 2 unter `rust/spec/stelle/fassung2/` (Zweig) | `mcp.anfragen`, `mcp.stand`, `mcp.einloesen`; alle Pflichten zur Freigabe und zur zweiten Person (Fassung 2, Abschnitt 4.3 und 4.5) |
| Klassenerbe (Entwurf M3) | `rust/spec/klassenerbe/` | das Erbe der Quellen eines Ergebnisses |
| Klassen am Ort (entsteht) | `rust/spec/datentor/klassen/` | die Ortsklasse nach A32 |

## 1. Schnitt

Das Tor ist eine Bibliothek ohne Ein- und Ausgabe, wie `darf`. Es liest keine Datei, schreibt keine, kennt keine Uhr und
keinen Zufall. Es entscheidet über eine Schreibanfrage und sagt, mit welcher Klasse und welchem Kunden geschrieben wird.
Schreiben, Protokoll und die Frage an die Stelle macht der Aufrufer.

- **Aufrufer (E7).** Es gibt genau zwei, beide außerhalb des Einschlusses:
  - Für ein Modell ruft der Such-Prozess `rangkern sitzung` (Vertrag 4) das Tor. Er setzt immer `herkunft: modell`. Gegenüber
    der Stelle ist er die Art `brain`, ohne Nachweis. Nur er kennt Route, Sitzung und die gelesenen Notizen (H).
  - Für einen Menschen ruft das eigene Programm `brain-dienst` das Tor (eigener Crate `brain-dienst`, noch nicht gebaut),
    außerhalb jeder Sitzung. Es setzt immer `herkunft: mensch` und ist gegenüber der Stelle die Art `brain_mensch`. Die Stelle
    misst die Art über `peerkanal`, mit dem Nachweis `h1` der Person (Vertrag 6 Fassung 2, Abschnitt 3 und 4.3). Ein
    Unterbefehl von `rangkern` reicht dafür nicht, weil `peerkanal` das Programm misst.
- **Herkunft** ist `modell` oder `mensch`. `modell` heißt jedes Modell: Agent, Worker, Orchestrator oder ein anderes (A36).
  `mensch` heißt der Weg des Menschen über `brain-dienst` mit gemessener Anmeldung.
- **Wer die Herkunft sagt:** das aufrufende Programm, fest nach seiner Art, nie ein Feld der Anfrage (V). Eine Anfrage mit
  `herkunft`, `route`, `sitzung`, `klasse` beim Anlegen durch ein Modell, `freigabe` oder `werkzeug` ist ungültig oder
  abgelehnt (S1, S5).
- **Pflichten führt nur die Stelle (E7).** Ob ein Schreiben die Freigabe eines Menschen oder eine zweite Person braucht,
  entscheidet die Stelle im Augenblick der Anfrage (Vertrag 6 Fassung 2, Abschnitt 4.5). Das Tor liest keine Option dazu.
  Es sagt nur, welches Werkzeug ein Schreiben ist, und prüft im zweiten Aufruf, was die Stelle geantwortet hat.
- **Ein Tor für alle Wege (V).** Es gibt keinen Schreibweg am Tor vorbei: kein Werkzeug im Einschluss, keine Option, die das
  Tor abschaltet. Der Vault liegt außerhalb des Einschlusses (Vertrag 4); der Agent erreicht ihn nur über den Socket.
- Das Tor senkt nie selbst eine Klasse. Senken gibt es nur auf drei Wegen: die Wahl eines Menschen (A37), die bestätigte
  Umklassierung (A38) und die Herabstufung durch die Automatik (A39, A40; `datentor/klassen`, nicht dieses Tor).
- **Senken durch einen Menschen (E6).** Ein Mensch darf mit eigener Wahl eine Klasse senken, aber nur mit Protokollzeile und
  mit dem Pflichtfeld `grund` (Abschnitt 3.1). Jedes Senken geht als Werkzeug `brain_senken` über die Stelle. Haben die
  Admins dort die Pflicht `senken_zweite_person` eingeschaltet, wirkt ein Senken erst, wenn eine zweite Person es freigibt
  (Abschnitt 2a). Ein Modell
  entscheidet nie, eine Klasse zu senken: Es wählt beim Anlegen und Ändern keine Klasse (S5), und die Automatik senkt nie
  (Abschnitt 5). Eine Umklassierung nach A38 trägt der Chat eines Menschen; sie ist ein Senken des Menschen und hat dieselben
  Pflichten (Abschnitt 2a). Diese Lesart von A38 gilt vorbehaltlich Bestätigung durch den Betreiber (Projektentscheid, 20.11.2026); die
  Alternative „ein Modell senkt nie, auch nicht als Träger“ ändert die Fälle `s6-modell-umklassieren`, `s4-option-aus`,
  `s26-modell-traegt-senken` und `s27-werkzeug` (seit E7 auch die Tabelle der Stelle, Vertrag 6 Fassung 2, Abschnitt 4.3).

## 2. Weg einer Schreibanfrage

Jedes Schreiben eines Modells und jedes Senken geht über die Stelle (E7). Das Tor antwortet dann `freigabe-noetig` und nennt
das Werkzeug. Der Aufrufer fragt immer `mcp.anfragen` mit Server `brain` und diesem Werkzeug. Was die Stelle antwortet,
hängt nur von ihren Pflichten ab (Vertrag 6 Fassung 2, Abschnitt 4.3):

| Weg | Werkzeug | Pflicht der Stelle aus | Pflicht der Stelle an |
|---|---|---|---|
| Modell, Art `brain` | `brain_schreiben` | `ohne_freigabe` (`mcp.brain.freigabe` aus) | `offen` (`mcp.brain.freigabe` an) |
| Modell, Art `brain` | `brain_senken` | immer `offen` (A38) | `offen`, nur eine andere Person als `person` erteilt |
| Mensch, Art `brain_mensch` | `brain_senken` | `ohne_freigabe`, der Mensch selbst ist `entscheidend` | `offen`, eine andere Person erteilt (`senken_zweite_person`) |

**Mensch** (Oberfläche, A37): Die Oberfläche schickt die Anfrage an `brain-dienst`. Der Dienst holt die Lage (Abschnitt 3)
und ruft das Tor. Bei `schreiben` schreibt er die Notiz mit `datenklasse` und `kunde` im Frontmatter; das gilt für jedes
Schreiben, das nicht senkt. Bei `freigabe-noetig` (ein Senken, Werkzeug `brain_senken`) folgt Abschnitt 2a.

**Modell** (Agent im Einschluss, A36, A38, A90):

1. Der Agent ruft über das MCP-Werkzeug im Einschluss den Socket seiner Sitzung, Operation `schreiben` (Fassung 2 von
   Vertrag 4, angekündigt). Route und Sitzung kommen aus dem Socket.
2. Der Such-Prozess holt die Lage und ruft das Tor ohne Freigabe. Bei `abgelehnt` antwortet er dem Agenten mit dem Code und
   schreibt die Zeile `wissen.schreiben.abgelehnt`.
3. Sonst antwortet das Tor immer `freigabe-noetig` (E7). Der Such-Prozess stellt bei der Stelle `mcp.anfragen` für den Server
   `brain` mit dem Werkzeug aus der Antwort (`brain_schreiben` oder, bei einem senkenden Umklassieren, `brain_senken`), als
   Aufrufer der Art `brain`. `argumente` ist die Anfrage des Agenten und das Ergebnis des Tors:
   `{"anfrage": <Anfrage>, "ergebnis": {"klasse": …, "kunde": …}}`. So bindet der `digest` der Stelle auch Klasse und Kunde.
   `vorschau` zeigt Ziel, Art, Vorher und Nachher, Klasse alt und neu sowie den Kunden.
4. Antwortet die Stelle `ohne_freigabe`, geht es mit Schritt 6 weiter, mit `entscheidend: null`. Antwortet sie `offen`, bekommt
   der Agent `{"ergebnis":"offen","anfrage":"mf-…"}`, und der Mensch entscheidet in der Oberfläche mit Nachweis
   (`mcp.entscheiden`, Vertrag 6, Abschnitt 6).
5. Fragt der Agent nach (`schreiben_stand` mit der Kennung), liest der Such-Prozess `mcp.stand` (lesend, ohne Zeile; A).
   Bei `erteilt` geht es mit Schritt 6 weiter.
6. Der Such-Prozess holt die Lage neu und ruft das Tor ein zweites Mal, mit `kontext.freigabe` aus der Antwort der Stelle
   (`ohne_freigabe` oder `mcp.stand`): Klasse und Kunde des gebundenen Ergebnisses, `werkzeug` und `zweite_person`, dazu
   `entscheidend` (Abschnitt 3.2). Bei `schreiben` löst er eine erteilte Anfrage mit `mcp.einloesen` ein, mit denselben
   `argumente`; erst nach `eingeloest` (oder nach `ohne_freigabe`) schreibt er die Notiz.
7. Hat sich die Lage geändert, so dass Klasse oder Kunde abweichen, lehnt das Tor mit `ergebnis-abweichend` ab; ein anderer
   Stand am Ziel ergibt `stand-abweichend`. Der Prozess schreibt dann nicht. Ein neuer Versuch ist eine neue Anfrage.

Die Freigabe ist nur so stark wie Vertrag 6: Sie gilt einmal, nur innerhalb der Frist, nur für diesen `digest` und nur nach
der Entscheidung eines Menschen mit Nachweis (A92, A105). Das Tor selbst prüft keinen Nachweis; es bekommt im Kontext nur, was
die Stelle geantwortet hat. Der Menschenweg geht genauso: `brain-dienst` fragt die Stelle, ruft das Tor ein zweites Mal und
schreibt erst danach.

### 2a. Senken und die zweite Person (E6, E7)

**Senken** heißt: Die Klasse ist ausdrücklich gewählt (ein Mensch mit einer Klasse außer `auto`, oder jedes `umklassieren`),
der Bestand ist vorhanden, `klasse_alt` ist nicht `null`, und `klasse_neu` steht in `klassen` vor `klasse_alt` (Rang wie
`Vorgabe::rang`). `klasse_alt` ist die wirksame Klasse nach D5: Ein Bestand mit Kunde zählt als höchste Klasse, ein Bestand
ohne Klasse als Vorgabestufe. Kein Senken sind: Heben, dieselbe Klasse, Auto, `anlegen` (auch unter der Klasse des Orts, A37)
und ein Bestand mit ungültiger Klasse (S11; die Reparatur nennt eine bekannte Klasse). Ein Wechsel des Kunden allein bei
gleicher Klasse ist kein Senken.

1. **Erster Aufruf.** Ein Senken ohne `grund` lehnt das Tor ab (S17). Sonst antwortet es `freigabe-noetig` mit
   `werkzeug: "brain_senken"`, für Mensch und Modell.
2. **Frage an die Stelle.** Der Aufrufer (`brain-dienst` als Art `brain_mensch` mit Nachweis, oder der Such-Prozess als Art
   `brain`) stellt `mcp.anfragen` für den Server `brain`, Werkzeug `brain_senken`, `argumente` wie in Abschnitt 2 (die Anfrage
   samt `grund`, dazu das Ergebnis; der `digest` bindet so auch den Grund) und `person` = Pseudonym des Schreibenden oder,
   bei einem Modell, des Menschen, der im Chat angewiesen hat. Die `vorschau` zeigt Ziel, Klasse alt und neu und den Grund;
   der Grund steht nie in einer Zeile (Abschnitt 9).
3. **Die Stelle entscheidet nach ihrer Pflicht** (Vertrag 6 Fassung 2, Abschnitt 4.3 und Tabelle in Abschnitt 2):
   - Menschenweg, Pflicht `senken_zweite_person` aus: `ohne_freigabe`; `entscheidend` ist der Mensch selbst, `zweite_person`
     ist `false`.
   - Menschenweg, Pflicht an: `offen`; nur eine andere Person als `person` erteilt (`stelle.selbe_person` sonst), danach
     `zweite_person: true`.
   - Modellweg: `brain_senken` ist immer `offen` (A38); mit Pflicht gilt dieselbe Regel zur anderen Person.
4. **Wer die zweite Person sein darf:** jeder Mensch mit gültigem Nachweis (Vertrag 6, Abschnitt 6), dessen Pseudonym nicht
   die `person` der Anfrage ist. Eine Rolle (etwa nur Admin) verlangt dieser Vertrag nicht; sie wäre eine Einschränkung der
   Stelle (Offen 6). Im Durchstich trägt die Brücke genau ein Pseudonym (Vertrag 6, Abschnitt 6): Dort ist eine zweite
   Person nicht herstellbar, und die Pflicht bleibt aus.
5. **Das Tor prüft ein zweites Mal.** Der Aufrufer ruft es mit `freigabe` aus der Antwort der Stelle und den Kontextfeldern
   `person` und `entscheidend` (Abschnitt 3.2). Weichen Klasse oder Kunde vom gebundenen Ergebnis ab, lehnt es mit
   `ergebnis-abweichend` ab, ein anderes Werkzeug mit `werkzeug-abweichend` (S15). Bei `zweite_person: true` lehnt es eine
   fehlende Person mit `zweite-person-fehlt` und gleiche Personen mit `selbe-person` ab (S18). Bei `zweite_person: false` muss
   im Menschenweg `entscheidend` gleich `person` sein, sonst `person-abweichend` (S18). Eine Ablehnung passiert hier nur,
   wenn Stelle oder Aufrufer versagt haben.
6. **Umklassierung nach A38 (vorbehaltlich Bestätigung durch den Betreiber, Projektentscheid, 20.11.2026).** Ein Modell, das auf Anweisung
   eines Menschen im Chat umklassiert, bleibt ein Träger: Es wirkt
   erst nach der Freigabe eines Menschen (die Stelle antwortet für `brain_senken` im Modellweg immer `offen`), und senkt es,
   braucht es `grund` (S17) und bei eingeschalteter Pflicht eine zweite Person als Entscheidenden. `person` ist dann der
   Mensch, der im Chat angewiesen hat. Ohne diese Regel ließe sich die Pflicht über einen Chat mit einem Modell umgehen.

## 3. Eingabe

Ein Aufruf hat drei Teile. Nur `anfrage` stammt vom Schreibenden; `kontext` und `lage` setzt der Aufrufer.

### 3.1 Anfrage (vom Agenten oder von der Oberfläche)

| Feld | Pflicht | Form | Bedeutung |
|---|---|---|---|
| `art` | ja | `anlegen`, `aendern` oder `umklassieren` | Löschen und Verschieben folgen später |
| `ziel` | ja (S2) | Text, Notizpfad relativ zum Vault | das benannte Ziel (A15) |
| `inhalt` | bei `anlegen`, `aendern`; fehlt bei `umklassieren` | Text, höchstens 1 048 576 Byte UTF-8 | der neue Text ohne Frontmatter der Klasse |
| `vorher_sha256` | bei `aendern`, `umklassieren`; bei `anlegen` fehlt es oder ist `null` | 64 Hexziffern klein | sha256 des Bestands, den der Schreibende gesehen hat |
| `klasse` | bei `umklassieren`; sonst optional, nur Mensch | Text: `auto` oder ein Klassenschlüssel; bei `umklassieren` nie `auto` | die Wahl des Menschen (A37) oder die Zielklasse (A38) |
| `kunde` | bei `umklassieren` (Text oder `null`); sonst nur mit einer Klasse außer `auto` | Kundenschlüssel oder `null` | gewählter Kunde |
| `grund` | beim Senken (Abschnitt 2a, S17); sonst optional; nur zusammen mit einer Klasse außer `auto` oder bei `umklassieren` | Text, 8 bis 500 Zeichen (Unicode-Zeichen, nicht Byte), ohne Steuerzeichen (U+0000 bis U+001F, U+007F), erstes und letztes Zeichen nicht U+0020; nie `null` | die Begründung des Menschen (E6) |

Jedes andere Feld macht die Anfrage ungültig (S1), ausdrücklich `route`, `sitzung`, `herkunft`, `freigabe`, `datenklasse` und
`werkzeug` (E7: den Werkzeugnamen rechnet das Tor aus; der Agent wählt ihn nie).

**Grund (E6, V).** Die Form gilt immer, wenn das Feld da ist, auch wenn nichts gesenkt wird; eine Verletzung ist
`anfrage-ungueltig` (S1), ebenso `grund` bei `anlegen` oder `aendern` ohne Klasse oder mit `auto`. Das Tor liest den Grund
nicht und bewertet ihn nicht; es prüft nur die Form. Fehlt das Feld bei einem Senken, antwortet es `grund-fehlt` (S17). Ein
`grund` ohne Senken ist erlaubt und wird gleich behandelt (Abschnitt 9). Der Text selbst gehört nie in eine Protokollzeile.

**Ziel (A15, V).** Höchstens 512 Byte UTF-8, Segmente mit `/` getrennt, keines leer, keines `.` oder `..`, keines beginnt mit
`.`, kein Steuerzeichen (U+0000 bis U+001F, U+007F), kein `\`, Endung `.md`, nicht nur `.md`. Ein Ziel außerhalb des Vaults
lässt sich so nicht nennen. Groß- und Kleinschreibung bleiben, wie sie sind. Ohne Ziel bricht jedes Schreiben ab, gleich aus
welchem Arbeitsordner (A15).

### 3.2 Kontext (vom Aufrufer, gemessen)

`{"herkunft": "modell"|"mensch", "sitzung": <Kennung>|null, "route": <Text>|null,
"freigabe": null|{"klasse", "kunde", "werkzeug", "zweite_person"}, "person": <Pseudonym>|null, "entscheidend": <Pseudonym>|null}`

- Bei `modell` ist `sitzung` die Kennung aus dem Start, `route` die Route der Sitzung aus dem Socket.
- `freigabe` ist `null` beim ersten Aufruf. Beim zweiten Aufruf (Abschnitt 2, Schritt 6; Abschnitt 2a) ist es ein Objekt mit
  genau vier Feldern (E7): `klasse` und `kunde` des gebundenen Ergebnisses, `werkzeug` (`brain_schreiben` oder `brain_senken`)
  und `zweite_person` (Wahrheitswert). Die Werte übernimmt der Aufrufer aus der Antwort der Stelle (`ohne_freigabe`,
  `mcp.stand` oder `mcp.einloesen`, Vertrag 6 Fassung 2, Abschnitt 4.3), nie aus der Anfrage. Es gilt für `modell` und
  `mensch`.
- Bei `mensch` sind `sitzung` und `route` `null`.
- `person` (E6, V) ist das Pseudonym (`^ps_[0-9a-f]{32}$`) der Person, die schreibt oder, bei `modell`, im Chat angewiesen
  hat; `null`, wenn der Aufrufer sie nicht kennt. `entscheidend` (E6, V) ist das Pseudonym des Menschen, der die Freigabe
  erteilt hat, aus der Antwort der Stelle; `null` ohne `freigabe`. Beide setzt der Aufrufer, nie die Anfrage.
- Mit `freigabe` ist `entscheidend` bei `mensch` nie `null` (E7): Bei `ohne_freigabe` nennt die Stelle den Menschen selbst.
  Bei `modell` darf es `null` sein; das ist `ohne_freigabe` im Modellweg.

### 3.3 Lage (vom Aufrufer, gelesen im Augenblick der Anfrage)

| Feld | Form | Quelle |
|---|---|---|
| `ziel` | `{"vorhanden": false}` oder `{"vorhanden": true, "sha256", "klasse", "kunde"}` | Frontmatter `datenklasse` und `kunde` des Bestands, roh wie in Vertrag 4 |
| `ort` | Klassenschlüssel oder `null` | Ortsklasse des Ziels nach A32 aus `datentor/klassen`; `null`: kein Ort hat eine Klasse |
| `kennzeichen` | Liste von Kennungen | Kennzeichen der neuen Notiz (A36), etwa aus den Tags |
| `erkannt` | `{"klasse": <Schlüssel>|null, "kunden": [<Kennung>]}` | Erkennung über den Inhalt (A37: interne Namen aus der Admin-Liste, Filterkern) |
| `erbe` | `{"klasse", "kunden"}`, `"unklar"` oder `null` | Klassenerbe der Sitzung (Vertrag `klassenerbe`); `null` nur bei `mensch` |

Die Erkennung kann nur heben. Ein lokales Modell darf in ihr eine Klasse vorschlagen, die hebt, nie eine, die senkt (wie A40).

## 4. Regeln

Die Prüfungen laufen in dieser Reihenfolge. Die erste zutreffende bestimmt den Grund (S0).

| Regel | Code | Wann | Herkunft |
|---|---|---|---|
| S1 | `anfrage-ungueltig` | Die Anfrage ist kein Objekt oder verletzt die Form in 3.1. | (V) |
| S2 | `ziel-fehlt` | `ziel` fehlt, ist `null` oder leer. | A15 |
| S3 | `ziel-ungueltig` | `ziel` verletzt die Form in 3.1. | A15 |
| S4 | `route-fehlt`, `route-ohne-klasse` | nur `modell`: Die Route fehlt oder ist leer; oder sie steht nicht in der Vorgabe oder hat keine Klasse. | A42, A43 |
| S5 | `klasse-nur-mensch` | nur `modell`: `anlegen` oder `aendern` mit `klasse` oder `kunde`, auch mit `auto`. | A36 |
| S6 | `klasse-wahl-fehlt` | nur `mensch`: kein Feld `klasse` und Vorbelegung `keine`. | A37 |
| S7 | `klasse-unbekannt`, `kunde-unbekannt` | Die gewählte Klasse (außer `auto`) steht nicht in `klassen`; der gewählte Kunde steht nicht in `kunden`. Byte-genau. | (V) |
| S8 | `ziel-gesperrt` | nur `modell`: Der Bestand ist vorhanden, und `darf(Bestand, Route der Sitzung, Kunde des Bestands)` ist nicht `frei`. | A34 |
| S9 | `ziel-vorhanden`, `ziel-nicht-vorhanden` | `anlegen` über einem Bestand; `aendern` oder `umklassieren` ohne Bestand. | (V) |
| S10 | `stand-abweichend` | `vorher_sha256` ist nicht der sha256 des Bestands. | (V) |
| S11 | `bestand-ohne-klasse` | Die Klasse wird automatisch bestimmt (Abschnitt 5), und der Bestand hat keine gültige Klasse (D4). | (V), fail closed |
| S17 | `grund-fehlt` | Die Anfrage senkt (Abschnitt 2a) und hat kein Feld `grund`. Gilt für Mensch und Modell. | E6 |
| S12 | `erbe-unklar` | nur `modell` mit automatischer Klasse: Das Klassenerbe meldet `unklar`. | (A) |
| S13 | `kunden-gemischt` | Die Kunden aus Bestand, Erkennung und Erbe sind mehr als einer. | (A), A53 |
| S14 | – | Klasse und Kunde nach Abschnitt 5 | A32, A35 bis A37, A39 |
| S15 | `ergebnis-abweichend`, `werkzeug-abweichend` | mit `freigabe` (`modell` oder `mensch`): Klasse oder Kunde aus S14 weichen vom gebundenen Ergebnis ab; sonst: `freigabe.werkzeug` ist nicht das Werkzeug, das das Tor für diese Anfrage ausrechnet (Abschnitt 6). | A105, E7 |
| S18 | `zweite-person-fehlt`, `selbe-person`, `person-abweichend` | mit `freigabe`: Ist `freigabe.zweite_person` `true`, ist `person` oder `entscheidend` `null` (`zweite-person-fehlt`) oder sind beide gleich (`selbe-person`). Ist es `false`, gibt es bei `modell` keine Prüfung; bei `mensch` ist `entscheidend` nicht gleich `person` (`person-abweichend`; der Mensch handelt selbst, Nachweis `h1`). | E6, E7 |

S17 und S18 stehen in der Tabelle an ihrer Stelle der Prüfung (S17 nach S11, S18 nach S15); ihre Nummern folgen der Reihenfolge
der Entstehung. Die Reihenfolge innerhalb der Tabelle gilt.

Danach antwortet das Tor (Abschnitt 6), ohne eine eigene Option zu lesen (E7): ohne `freigabe` mit `freigabe-noetig`, wenn
die Herkunft `modell` ist (jedes `anlegen`, `aendern` und `umklassieren`) oder wenn die Anfrage senkt; sonst mit `schreiben`.
Mit `freigabe`, die S15 und S18 besteht, antwortet es mit `schreiben`. Ob ein Mensch freigeben muss, sagt danach die Stelle.

- **S8 vor S9 und S10 (V).** Ein Modell erfährt über einen Bestand, den seine Route nicht lesen darf, nur `ziel-gesperrt`:
  nicht, ob der Stand passt, nicht seine Klasse. Es erfährt, dass am Pfad etwas liegt; den Pfad hat es selbst genannt.
  Ein Bestand ohne Klasse bekommt in `darf` die Vorgabestufe; ein Bestand mit ungültiger Klasse ist gesperrt.
- **S8 gilt nicht für das Ergebnis (V).** Ein Modell darf in einen Ort schreiben, den seine Route nicht liest (A32). Das ist
  ein Fluss nach oben und kein Leck. Es kann seine eigene Notiz danach nicht mehr finden.
- **S5:** Ein Modell nennt beim Anlegen und Ändern keine Klasse. Das System ordnet sie zu, ohne Rückfrage (A36). Eine Klasse
  nennt ein Modell nur in `umklassieren`, auf Anweisung eines Menschen im Chat. Senkt das, wirkt es erst nach der Freigabe
  eines Menschen (A38, Stelle: `brain_senken` im Modellweg immer `offen`); ein hebendes Umklassieren folgt der Pflicht
  `mcp.brain.freigabe` wie jedes Schreiben eines Modells.

## 5. Klasse und Kunde des Ergebnisses

**Automatisch** (jedes `anlegen` und `aendern` eines Modells; bei einem Menschen ohne `klasse` mit Vorbelegung `auto`, oder
mit `klasse: "auto"`). Die Klasse ist das Maximum nach Position in `klassen` über:

1. den Ort: `ort`, sonst die Vorgabestufe, sonst die höchste Klasse (A32, A35);
2. jede Regel der Admins, deren `ordner` ein Ordner des Ziels ist (ganzes Segment: `kunden/o` trifft `kunden/o/x.md`, nicht
   `kunden/ox/x.md`) und deren `kennzeichen` fehlt oder in `kennzeichen` steht (A36). Eine Regel hebt nur (A);
3. `erkannt.klasse` (A37);
4. bei `aendern` die wirksame Klasse des Bestands nach D5 (A39: Ein Schreiben senkt nie);
5. nur bei `modell`: `erbe.klasse`.

Der Kunde: die Vereinigung von `erkannt.kunden`, dem Kunden des Bestands (bei `aendern`) und, nur bei `modell`, `erbe.kunden`.
Mehr als einer: S13. Genau einer: Er ist der Kunde, und die Klasse ist die höchste (D5). Keiner: `kunde` ist `null`.
Ist die Klasse dann die höchste ohne Kunden, wird die Notiz geschrieben; `darf` sperrt sie auf jeder Route (D7). Das ist
gewollt (fail closed); der Admin sieht sie in der Konsole.

**Ausdrücklich** (ein Mensch mit einer Klasse außer `auto`, und jedes `umklassieren`): Klasse und Kunde sind die gewählten
Werte, auch unter Ort, Regeln, Erkennung und Bestand (A37: „mit gewählter Klasse PUBLIC trägt die zweite PUBLIC“; A38). Ein
Mensch kann so einen Bestand senken, aber nur mit `grund` (S17), über die Stelle mit `brain_senken` und, wenn die Pflicht der
Stelle es verlangt, mit einer zweiten Person (S18, Abschnitt 2a). Das Protokoll nennt `klasse_alt`, `klasse_neu` und den Grund als Hash (Abschnitt 9).

`klasse_alt` ist die wirksame Klasse des Bestands nach D5, oder `null` bei `anlegen` und bei einem Bestand ohne gültige Klasse.

## 6. Antwort

```json
{"ergebnis": "freigabe-noetig", "grund": null, "klasse": "intern", "kunde": null, "klasse_alt": "oeffentlich", "werkzeug": "brain_schreiben"}
{"ergebnis": "freigabe-noetig", "grund": null, "klasse": "oeffentlich", "kunde": null, "klasse_alt": "kundendaten", "werkzeug": "brain_senken"}
{"ergebnis": "schreiben", "grund": null, "klasse": "kundendaten", "kunde": null, "klasse_alt": "intern", "werkzeug": null}
{"ergebnis": "abgelehnt", "grund": "ziel-gesperrt", "klasse": null, "kunde": null, "klasse_alt": null, "werkzeug": null}
```

- `ergebnis`: `schreiben`, `freigabe-noetig` oder `abgelehnt`. `grund` ist bei `abgelehnt` der Code, sonst `null`.
- `werkzeug` (E7, V) ist bei `freigabe-noetig` `"brain_senken"`, wenn die Anfrage senkt (Abschnitt 2a), sonst
  `"brain_schreiben"`; bei `schreiben` und `abgelehnt` ist es `null`. Das Tor rechnet es selbst aus; der Aufrufer stellt mit
  diesem Namen `mcp.anfragen`. Das Feld `zweite_person` aus E6 entfällt: Ob die Pflicht zur zweiten Person gilt, sagt jetzt
  die Stelle. Die Antwort hat weiter sechs Felder. Das Feld `grund` der Antwort ist der Ablehnungscode, nicht der Grund des
  Menschen.
- `klasse` ist der Wert für `datenklasse` im Frontmatter, `kunde` der für `kunde`. Bei `abgelehnt` sind alle drei `null`.
- Der Aufrufer gibt einem Modell `ergebnis`, `grund` und bei `freigabe-noetig` die Kennung der Anfrage weiter. `klasse_alt`
  geht nie an ein Modell, dessen Route den Bestand nicht lesen darf; bei S8 gibt es ohnehin keinen Wert.
- Das Tor ist eine reine Funktion. Gleiche Vorgabe und gleiche Eingabe ergeben dieselbe Antwort. Keine Eingabe löst eine
  Panik aus.

## 7. Vorgabe und Aufruf des Außentestprogramms `schreibtor`

**Vorgabe** ist die Vorgabe von Vertrag 3, Abschnitt 2, mit allen Regeln für Gültigkeit, dazu optional der Block `schreiben`
(A; kommt in eine künftige Fassung von Vertrag 2):

```json
{"schreiben": {"vorbelegung_mensch": "auto",
               "regeln": [{"schluessel": "r-kunden-o", "ordner": "kunden/o", "kennzeichen": "k-markt", "klasse": "kundendaten"}]}}
```

- `vorbelegung_mensch`: `auto` oder `keine`, Vorgabe `auto` (A37).
- **Keine Pflichten im Tor (E7).** Die Felder `freigabe_modell` und `senken_zweite_person` aus dem Stand vor E7 gibt es nicht
  mehr. Eine Vorgabe mit einem der beiden ist ungültig (Exit 2), auch mit dem Wert, der früher die Vorgabe war: Eine Kopie,
  die still ignoriert wird, sähe für einen Admin wie ein wirksamer Schalter aus. Die Pflichten heißen in der Stelle
  `mcp.brain.freigabe` (Freigabe je Schreibaufruf eines Modells, A90) und `senken_zweite_person` mit der Liste
  `mcp.brain.zweite_person` (Vertrag 6 Fassung 2, Abschnitt 2 und 4.5). Die Admins ändern sie dort mit `pflicht.aendern`;
  eine Verschärfung wirkt ab der nächsten Anfrage.
- `regeln`: 0 bis 1024 Objekte mit `schluessel` (Kennung, ohne Doppel), `ordner` (relativer Pfad: kein `/` am Anfang oder
  Ende, kein `//`, kein Segment `..`, kein Steuerzeichen), `kennzeichen` (Kennung, optional), `klasse` (aus `klassen`).
- Geschlossene Form auf jeder Ebene.

**Aufruf.** `schreibtor < fall.json` liest genau einen Fall von stdin, höchstens 4 194 304 Byte.

- Exit 0: Auf stdout steht genau ein JSON-Objekt `{"antworten": [...]}`, eine Antwort je Eintrag in derselben Reihenfolge.
- Exit 2 (S16): Der Fall ist ungültig; stdout bleibt leer, auf stderr steht genau eine Zeile. Ungültig heißt: kein JSON, kein
  Objekt, zu groß, ein doppelter Schlüssel in irgendeinem Objekt (auch in `beschreibung`; Projektentscheid, 20.11.2026), ein unbekanntes
  Feld im Fall, eine ungültige Vorgabe (auch eine mit `freigabe_modell` oder `senken_zweite_person`, E7), oder ein Eintrag verletzt die Form von `kontext` oder `lage` (Abschnitt 3.2, 3.3):
  - `kontext` hat genau die sechs Felder; `herkunft` ist `modell` oder `mensch`; bei `modell` ist `sitzung` eine Kennung;
    bei `mensch` sind `sitzung` und `route` `null`. `freigabe` ist bei beiden `null` oder ein Objekt mit genau `klasse` (aus
    `klassen`), `kunde` (Kennung oder `null`), `werkzeug` (`brain_schreiben` oder `brain_senken`) und `zweite_person`
    (Wahrheitswert; E7). `person` und `entscheidend` sind `null` oder ein Pseudonym `^ps_[0-9a-f]{32}$`; `entscheidend` ist
    nur mit `freigabe` nicht `null`, und bei `mensch` mit `freigabe` nie `null` (E7). `route` ist Text oder `null`.
  - `lage` hat genau die fünf Felder; `ziel` hat eine der zwei Formen, `sha256` mit 64 Hexziffern; `ort`, `erkannt.klasse`
    und `erbe.klasse` stehen in `klassen` oder sind `null`; Kunden und Kennzeichen sind Kennungen; `erbe` ist bei `modell`
    nie `null`.
- Die **Anfrage** macht den Fall nie ungültig. Eine falsche Anfrage ergibt `anfrage-ungueltig` (S1), denn sie stammt vom
  Schreibenden, nicht vom Aufrufer.

Fall:

```json
{"name": "…", "regeln": ["S8"], "beschreibung": "…", "vorgabe": {…},
 "anfragen": [{"kontext": {…}, "anfrage": {…}, "lage": {…}}]}
```

`name`, `regeln` und `beschreibung` wertet das Programm nicht aus. Alle Daten sind erfunden.

## 8. Fälle und Läufer

| Pfad | Zweck |
|---|---|
| `faelle_bauen.py` | schreibt `sichtbar/faelle/` und `sichtbar/erwartet/` aus Tabellen mit ausgeschriebenen Erwartungen; prüft jede gegen eine kleine Referenz der Abschnitte 3 bis 7 und bricht bei einer Abweichung ab |
| `aussen_laeufer.py` | schickt jeden Fall an `ST_BIN`, vergleicht Exitcode und geparstes JSON; Exit 0 alle grün, 1 rote Fälle, 2 Aufruffehler |
| `sichtbar/` | 59 Fälle: 36 gültige mit 206 Anfragen, 23 ungültige (Exit 2) |

```
python3 -I -B faelle_bauen.py
ST_BIN=<programm> ST_FAELLE=sichtbar python3 -I -B aussen_laeufer.py [name ...]
```

Gemessen am 20.11. (Mac, Python 3, Attrappe außerhalb des Repos, die die Referenz aus `faelle_bauen.py` als Programm ausführt):

- Stand vor E6 (41 Fälle): Attrappe 41 von 41 grün; ein Programm, das sofort mit Exit 0 endet: 0 von 41 grün. Abgewandelte
  Attrappen: ohne S8 (`darf` immer frei) 2 Fälle rot; ohne den Boden des Bestands 2 rot; ohne Freigabe (`freigabe-noetig`
  wird `schreiben`) 6 rot; gemischte Kunden werden zu einem Kunden: 1 rot.
- Stand mit E6 (51 Fälle): Attrappe 51 von 51 grün; sofort Exit 0: 0 von 51 grün. Abgewandelte Attrappen, je eine Änderung:
  Option wird übergangen 2 rot; `grund` wird nicht geprüft 5 rot; Form des Grundes wird nicht geprüft 1 rot; selbe Person
  erlaubt 2 rot; fehlende Person erlaubt 1 rot; Modell wählt Klasse 5 rot; Senken auch bei gleicher Klasse 2 rot;
  Grund auch beim Heben verlangt 3 rot; Freigabe des Menschen ignoriert 1 rot; `zweite_person` nicht gemeldet 2 rot; Modell
  ohne Grund 3 rot; Ergebnis der Freigabe nicht verglichen 2 rot; Senken ohne Freigabe für den Menschen 1 rot; `grund: null`
  erlaubt 1 rot.
- Stand mit E7 (59 Fälle): Attrappe 59 von 59 grün; sofort Exit 0: 0 von 59 grün. Abgewandelte Attrappen, je eine Änderung:
  Tor liest wieder eine eigene Option (Modell schreibt direkt) 10 rot; `werkzeug` immer `brain_schreiben` 9 rot; `werkzeug`
  aus der Anfrage übernommen 2 rot; Werkzeug im zweiten Aufruf nicht geprüft 4 rot; S18 bei `zweite_person: true` übergangen
  3 rot; `person-abweichend` übergangen 2 rot; Mensch mit Freigabe und `entscheidend: null` zugelassen 1 rot; alte Optionen
  in der Vorgabe still ignoriert 2 rot.

Die Referenz rechnet `darf` und D5 nach Vertrag 3 selbst nach. Ändert sich Vertrag 3, werden `darf` und `wirksam` im Fallbauer
angepasst und der Satz neu gebaut.

## 9. Protokoll

Die Zeilen schreibt der Aufrufer in die Kette der Quelle `brain` (Vertrag 5; Datei `brain-<JJJJ-MM>.jsonl`), über das
allgemeine Anhängen für geschlossene Zeilenarten, das gerade entsteht (A). Die Zeilenarten legt
dieser Vertrag fest. RFC 8785, Fassung 2, `vorher`, `zeit` wie in Vertrag 5.

```
{"aktionsname":"wissen.geschrieben","anfrage":"mf-1732107600000-1","art":"aendern","einheit":"eh_<32 hex>","ergebnis":"geschrieben","fassung":2,"grund":null,"grund_sha256":null,"herkunft":"modell","klasse_alt":"oeffentlich","klasse_neu":"intern","kunde":null,"route":"cloud-beispiel","sitzung":"s-1","vorher":"<64 hex>","zeit":"…"}
{"aktionsname":"wissen.geschrieben","anfrage":"mf-1732107600000-2","art":"aendern","einheit":"eh_<32 hex>","ergebnis":"geschrieben","fassung":2,"grund":null,"grund_sha256":"<64 hex>","herkunft":"mensch","klasse_alt":"kundendaten","klasse_neu":"oeffentlich","kunde":null,"route":null,"sitzung":null,"vorher":"<64 hex>","zeit":"…"}
{"aktionsname":"wissen.schreiben.abgelehnt","anfrage":null,"art":"anlegen","einheit":null,"ergebnis":"abgelehnt","fassung":2,"grund":"ziel-gesperrt","grund_sha256":null,"herkunft":"modell","klasse_alt":null,"klasse_neu":null,"kunde":null,"route":"cloud-beispiel","sitzung":"s-1","vorher":"<64 hex>","zeit":"…"}
```

- Genau diese 16 Felder (im Stand vor E6 stand hier „16“, gezählt waren 15; mit `grund_sha256` sind es 16). `einheit` ist das Pseudonym `eh_` + HMAC über den Pfad mit dem Schlüssel des Brain-Dienstes
  (Entwurf zur Herabstufung im Audit, Abschnitt 1 und 4); bei einer Ablehnung `null`, damit kein Pfad eines gesperrten
  Bestands eine Spur bekommt. Kein Pfad, kein Titel, kein Inhalt, kein Name (A104, A12).
- **Grund des Menschen (E6, V, nicht abgestimmt).** Der Text steht nie in der Zeile (A104, A12; er kann Namen
  tragen). Die Zeile trägt `grund_sha256`: sha256, 64 kleine Hexziffern, über die UTF-8-Bytes des Grundes genau wie in der
  Anfrage (ohne Kürzen, ohne Normalisieren). Er ist `null`, wenn die Anfrage keinen `grund` trug. Der Text liegt außerhalb der
  Kette in einer Ablage des Brain-Dienstes, adressiert über diesen Hash; ein Prüfer kann Text und Zeile abgleichen, und der
  Text lässt sich auf Anforderung löschen, ohne die Kette zu brechen. Ein Hash eines kurzen Satzes ist nur dem zu erraten, der
  den Satz schon kennt; ein HMAC mit dem Schlüssel des Brain-Dienstes (wie bei `einheit`) wäre dichter, ließe sich aber ohne
  den Schlüssel nicht prüfen (Offen 7). Das Feld `grund` der Zeile bleibt der Ablehnungscode.
- `klasse_alt` und `klasse_neu` stehen bei jedem Schreiben, das eine Klasse setzt; bei einem Senken zeigen sie es
  (`klasse_neu` steht im Rang vor `klasse_alt`). Wer es freigegeben hat, steht in der Zeile der Stelle (`mcp.schreibaufruf`,
  `entscheidend`, `werkzeug`), erreichbar über `anfrage`. Bei `ohne_freigabe` im Menschenweg nennt diese Zeile den Menschen
  selbst als `entscheidend`; ein Senken ist damit immer einer Person zuzuordnen (E7).
- `anfrage` ist die Kennung der Stelle (Form `^mf-[0-9]{1,15}-[1-9][0-9]{0,9}$`, A) oder `null`, wenn das Tor
  ohne die Stelle `schreiben` geantwortet hat. Seit E7 ist sie bei jedem Schreiben eines Modells und bei jedem Senken gesetzt,
  auch für `herkunft: mensch` und auch bei `ohne_freigabe` (die Stelle vergibt die Kennung dann ebenfalls, Vertrag 6
  Fassung 2, Abschnitt 4.3). `null` bleibt sie nur für einen Menschen, der nicht senkt.
- `art`, `herkunft`, `grund` und Klassen sind Schlüssel. `route` und `sitzung` sind `null` bei `mensch`. Ein Wert, der seine
  Form nicht besteht, steht als `null`.
- `freigabe-noetig` schreibt keine Zeile im Brain; die Stelle schreibt `mcp.schreibaufruf` (Vertrag 6, Abschnitt 7.1). Die
  Zeile `wissen.geschrieben` entsteht erst nach dem Schreiben der Datei. Scheitert die Zeile, wird nicht geschrieben (fail
  closed wie Vertrag 4).
- Ein Schreiben eines Menschen, das senkt, ist an `klasse_alt` über `klasse_neu` erkennbar. Rückholbar ist es über die
  Rückhol-Zeile der Herabstufung nicht; dafür gibt es Offen 5.

## 10. Was in A, B und M3 gilt

| | Durchstich A (Linux) | Durchstich B (Windows) | M3 |
|---|---|---|---|
| Schreibweg im Produkt | **keiner.** Der Socket kennt nur `search` und `ping`, `such-mcp` nur `brain_suche` (Vertrag 4); ein Schreiben ist `bad_request` bzw. ein unbekanntes Werkzeug | Empfehlung: keiner; die Freigabe je Schreibaufruf zeigt B an der MCP-Attrappe (Plan 5.3, Vertrag 6) | dieses Tor, gebaut, mit `schreiben` im Socket und `brain_schreiben` im MCP-Werkzeug |
| Was gilt | Kein Modell schreibt in den Vault; der Vault liegt außerhalb des Einschlusses | wie A; optional (Offen 1) das Tor mit Freigabe über die Stelle | volle Regeln S1 bis S18, Protokoll |
| Erbe | – | ohne Klassenerbe: `erbe` = Boden der Route (Vertrag `klassenerbe`, Abschnitt 4) | Klassenerbe nach Vertrag `klassenerbe` |
| Kunde der Sitzung | – | braucht `--kunde` am Such-Prozess (künftige Fassung von Vertrag 1 und 4, A) | ebenso |

In A ist der Vertrag nur Vorbereitung. Seine Fälle laufen gegen ein Programm, das es erst mit dem Bau gibt.

## 11. Annahmen

1. Die Ortsklasse kommt fertig aus `datentor/klassen` (A). Das Tor baut A32 nicht nach.
2. Die wirksame Klasse rechnet `datentor::wirksam(&Vorgabe, &Einheit, kunde) -> Result<&str, Grund>`, das Maximum
   `Vorgabe::rang` (A). Das Tor hat keine eigene Klassenliste.
3. Der Such-Prozess ist für die Stelle der Aufrufer `brain`, `brain-dienst` der Aufrufer `brain_mensch`; beide dürfen nur
   `mcp.anfragen`, `mcp.stand` und `mcp.einloesen` für den Server `brain`, und der Hub darf den Server `brain` nicht anfragen
   (A; Vertrag 6 Fassung 2, Abschnitt 3 und 4.4).
4. Der Kunde der Sitzung darf nur heben (A): Er geht über das Klassenerbe in `erbe.kunden` ein, nie als Senkung.
5. `vorher_sha256` ist der sha256 der ganzen Datei des Bestands, Frontmatter eingeschlossen.
6. (E6) Ein `anlegen` unter einer niedrigeren Klasse als der des Orts ist kein Senken, weil es keinen Bestand gibt (A37, Fall
   `s2-mensch-waehlt`). Wer das als Senken zählen will, ändert S17 und die Fälle `s2`, `s21` und `s24` (Offen 8).
7. (E6) Das Pseudonym der Person kommt vom Aufrufer aus einer gemessenen Anmeldung. Im Durchstich gibt es genau eines je
   Brücke (Vertrag 6, Abschnitt 6); die Pflicht `senken_zweite_person` der Stelle bleibt dort aus.
8. (E7) Die Stelle führt `brain_senken` fest (Projektentscheid, 20.11.2026; Vertrag 6 Fassung 2, Abschnitt 4.3): Für
   die Art `brain` (Modellweg) ist `brain_senken` immer `offen` (A38), unabhängig von jeder Einstellung, auch bei
   `mcp.brain.freigabe` aus und ohne Eintrag in `mcp.brain.zweite_person`. Die Liste `zweite_person` und die Pflicht
   `senken_zweite_person` entscheiden nur, ob die erteilende Person eine andere sein muss. Grund: Eine Sicherheitsregel hängt
   nicht an einem Eintrag, den jemand weglassen kann.

## 12. Offen

Entscheidungen für den Menschen sind offen (E6 ist am 20.11.2026 entschieden, die Folgen stehen in diesem Vertrag).
Technisch offen:

1. Fassung 2 von Vertrag 4: Operationen `schreiben` und `schreiben_stand`, Werkzeug `brain_schreiben`, Option `--kunde`.
2. Fassung 2 von Vertrag 6 mit der Art `brain` und `mcp.stand` (angekündigt).
3. Woher `kennzeichen` stammen (Tags im Frontmatter oder ein eigenes Feld) und wie die Erkennung (A37) gebaut wird.
4. Gleichzeitige Schreiben auf dasselbe Ziel: Der Aufrufer muss zwischen Tor und Schreiben sperren und `sha256` unter der
   Sperre neu prüfen. Das legt der Bau fest.
5. (Verweis aus Abschnitt 9) Rückholen eines Senkens eines Menschen; die Rückhol-Zeile der Herabstufung deckt es nicht.
6. (E6, Stand E7) Fassung 2 von Vertrag 6 (Zweig, Stand vom 20.11.2026) führt inzwischen das Feld
   `zweite_person` bei `mcp.anfragen`, den Code `stelle.selbe_person`, `entscheidend`, `werkzeug` und `zweite_person` in den
   Antworten und die Art `brain_mensch`. Offen bleibt, ob die zweite Person eine Rolle (Admin) haben muss, und der Merge
   dieser Fassung.
7. (E6) `grund_sha256` als einfacher sha256 oder als HMAC; Ablage des Grundtextes außerhalb der Kette (Ort, Aufbewahrung,
   Löschfrist). Ein Feld `person` in der Zeile `wissen.geschrieben` ist seit E7 nicht mehr nötig, damit ein Senken einer
   Person zuzuordnen ist: Jedes Senken hat eine `anfrage`, und deren Zeile `mcp.schreibaufruf` nennt `person` und
   `entscheidend`. Ob die Zeile es trotzdem tragen soll, ist offen.
8. (E6) Zählt ein `anlegen` unter der Ortsklasse als Senken (Annahme 6)?

<!-- vom Bauvorgang angefuegt -->
> Hinweis: Dieser Text nennt Werkzeuge, die in diesem Klon nicht liegen: `brain`.
> Sie gehoeren zum Setup der Maschine, aus der dieser Baum stammt; hier fehlen
> sie mit Absicht. Wer sie braucht, schreibt sich seine eigenen.
> *Named here, not shipped here.*
