# Teilvertrag `datentor person`: Leseklasse der Person

Stand 27.11.2026, Teilvertrag zu Vertrag 3 (`rust/spec/VERTRAEGE.md`). Nachtrag 27.11.2026 in Abschnitt 10.
Grundlage sind A05 und A34 (zweite Gegenprobe), dazu A12, A26, A29, A35, A49 und A139 der Anforderungsliste.
„(V)“ heißt: in diesem Vertrag festgelegt. „(H)“ heißt: aus den Anforderungen hergeleitet. Offene Fragen stehen in Abschnitt 9.

Gebaut ist der Vertrag seit 27.11.2026 im Crate `datentor` (`src/person.rs`, `src/programm_person.rs`). Ein Bau gilt als vertragstreu, wenn er beide Fallsätze grün besteht und Vertrag 3 unverändert grün bleibt.

## 1. Schnitt

A05: „Jede Wissensroute prüft die Leseklasse der Person, auch beim Zitat und beim Verweis auf Nachfolger.“ A34: „Route und Person müssen beide zulassen; es gilt der strengere Wert.“

- Die **Leseklasse** einer Person ist die höchste Klasse, die sie lesen darf. Sie kommt aus den Einstellungen, je Person oder über ihre Gruppen (Abschnitt 3).
- Die Personenprüfung kommt **zusätzlich** zu `darf` aus Vertrag 3; sie ersetzt sie nicht. Eine Einheit wird nur ausgegeben, wenn Route und Person sie zulassen. `darf` bleibt unverändert, mit derselben Signatur. Die Prüfung macht eine neue reine Funktion `darf_person` (Abschnitt 8).
- Das Tor bleibt eine reine Bibliothek ohne Ein- und Ausgabe, ohne Uhr und ohne Zufall. Es kennt keinen Klassennamen und vergleicht Klassen nur nach ihrer Position (A29).
- Die Person ist der Nutzer der Sitzung. Nutzer und Gruppen kommen aus dem Start der Sitzung, nie aus der Anfrage des Agenten, in derselben Form wie `kontext` in `../zonen/VERTRAG.md`, Abschnitt 6.

Nicht in diesem Teilvertrag: eine Beschränkung der Person auf Kunden (Frage F4), die Verbindung mit den Modellzonen (Frage F5), das Schreiben des Protokolls, Zitat und Verweis auf Nachfolger selbst (sie gehören der Suche, Abschnitt 7).

## 2. Option

Die Personenprüfung ist eine Grundfunktion (A05), keine Option. Es gibt keinen Schalter und keine Vorgabe-Leseklasse: Eine Person ohne Eintrag hat keine Leseklasse und bekommt nichts (fail closed, V). Das bleibt so (F3, Projektentscheid, 20.11.2026). Den Eintrag einer Person oder einer Gruppe setzt und ändert nur ein Admin, und jede Änderung steht im Protokoll (Projektentscheid, 20.11.2026). Der Agent und die Person selbst ändern keinen Eintrag; die Einstellungen (Vertrag 2) tragen die Einträge.

## 3. Sicht des Tors auf die Personen

```json
{"leseklassen": [{"ebene": "nutzer", "wer": "p-a", "klasse": "intern"},
                 {"ebene": "gruppe", "wer": "g-a", "klasse": "oeffentlich"}]}
```

- `leseklassen`: Liste von Einträgen. Je Eintrag `ebene` (`nutzer` oder `gruppe`), `wer` (Schlüssel des Nutzers oder der Gruppe) und `klasse` (ein Schlüssel aus `klassen` der Vorgabe). Die Form `ebene` und `wer` ist dieselbe wie bei den Regeln der Modellzonen, damit Admins eine Form lernen.
- **Schlüssel** haben die Form `^[a-z0-9][a-z0-9._-]{0,63}$` wie in Vertrag 3.
- **Pflichtfelder** sind `leseklassen` und je Eintrag alle drei Felder. Die Form ist geschlossen, auf jeder Ebene.

**Ungültig** ist `personen` (P0), wenn eine dieser Bedingungen gilt:

- Ein Feld fehlt, hat den falschen Typ oder ist unbekannt. `null` ist für kein Feld gültig.
- `ebene` ist weder `nutzer` noch `gruppe`, oder `wer` hat nicht die Schlüsselform.
- **Ladefehler:** `klasse` ist kein Text in `klassen` der Vorgabe. Eine Einstellung, die einer Person eine Klasse gibt, die es nicht gibt, lässt kein Tor entstehen (fail closed, V). Der Vergleich ist byte-genau (D14): `Intern` ist nicht `intern`.
- Dasselbe Paar aus `ebene` und `wer` steht zweimal, auch mit derselben Klasse (V; zwei Einträge wären mehrdeutig, und ein Leser, der den letzten nimmt, könnte einen strengeren verdecken, wie A139).
- `leseklassen` hat mehr als 4096 Einträge (Nachtrag 27.11.2026, die Grenze des Feldes in den Einstellungen).

Mit einem ungültigen `personen` entsteht kein Tor (V). Der Aufrufer liefert dann nichts aus.

## 4. Regeln

**P4, Leseklasse der Person:**

- Hat der Nutzer einen eigenen Eintrag (`ebene: nutzer`, `wer` gleich `kontext.nutzer`), gilt dessen Klasse, auch wenn eine seiner Gruppen höher liegt (Frage F2).
- Sonst gilt die höchste Klasse aus den Einträgen seiner Gruppen (`ebene: gruppe`, `wer` in `kontext.gruppen`; Frage F1).
- Sonst hat die Person keine Leseklasse.
- Vergleiche sind byte-genau (D14). Ein Gruppenschlüssel zählt nur als Gruppe, ein Nutzerschlüssel nur als Nutzer.

Die Prüfungen laufen in dieser Reihenfolge. Die erste zutreffende bestimmt den Grund (P5, wie D12).

| Regel | Code | Wann | Meldung | Herkunft |
|---|---|---|---|---|
| D2 | `route-fehlt` | wie Vertrag 3 | `Anfrage ohne Route` | A43 |
| P1 | `person-fehlt` | `kontext` fehlt, oder `kontext.nutzer` fehlt, ist `null` oder leer. | `Anfrage ohne Person` | A05, fail closed (V) |
| P2 | `person-ohne-leseklasse` | Die Person hat nach P4 keine Leseklasse; dazu gehört eine Person, die kein Eintrag nennt. | `Person ohne Leseklasse` | A05, fail closed (V) |
| D3 | `route-ohne-klasse` | wie Vertrag 3 | `Route {route} hat keine Klasse` | A42 |
| D4 | `einheit-ohne-klasse` | wie Vertrag 3 | `Einheit ohne gültige Klasse` | (V) |
| D6 | `klasse-ueber-route` | wie Vertrag 3 | `Klasse der Einheit liegt über Route {route}` | A34 |
| P3 | `klasse-ueber-person` | Die wirksame Klasse der Einheit (D5) liegt über der Leseklasse der Person. | `Klasse der Einheit liegt über der Leseklasse der Person` | A05, A34 |
| D7 bis D11 | wie Vertrag 3 | wie Vertrag 3 | wie Vertrag 3 | A50, A51, A52 |
| D1 | `frei` | sonst | `frei` | |

**Begründung der Reihenfolge (V):**

- D2 bleibt zuerst: Ohne Route gibt es keinen Lauf, gleich welche Person fragt. Das hält die Reihenfolge von Vertrag 3 und Modellzonen gleich (dort D2 vor Z1).
- P1 und P2 kommen vor allen Prüfungen der Route und der Einheit: Sie hängen nur an der Sitzung und sperren jede Einheit gleich. So steht der Grund, den der Admin beheben muss, vorn (wie Z1 und Z2 in den Modellzonen).
- D3, D4 und D6 kommen vor P3: Sie sind Gründe von Vertrag 3, die schon ohne Person sperren. Für eine Person mit der höchsten Leseklasse ist die Antwort dann genau `darf` aus Vertrag 3 (P6).
- P3 steht direkt nach D6, vor D7 bis D11: Beide vergleichen die Klasse der Einheit mit einer Grenze. Die Kundenregeln D7 bis D11 gelten erst für eine Einheit, die die Person lesen darf.

**P6, Bezug zu Vertrag 3:** `darf_person` gibt `frei` genau dann, wenn `darf` aus Vertrag 3 `frei` gibt und die Person nach P4 eine Leseklasse hat, die nicht unter der wirksamen Klasse liegt. Hat die Person die höchste Leseklasse, sind Grund, Klasse und Meldung gleich denen von `darf`.

## 5. Antwort

```json
{"darf": false, "grund": "klasse-ueber-person", "klasse": "intern",
 "meldung": "Klasse der Einheit liegt über der Leseklasse der Person", "leseklasse": "oeffentlich"}
```

- `darf`, `grund`, `klasse` und `meldung` haben die Bedeutung aus Vertrag 3, Abschnitt 3. `klasse` ist die wirksame Klasse (D5), auch bei einem Grund P.
- `leseklasse`: die Leseklasse der Person nach P4 als Schlüssel, oder `null`, wenn die Person fehlt oder keine hat.
- Die Antwort nennt nie den Nutzer, eine Gruppe, Inhalt, Titel, Pfad oder Kunden der Einheit. Die Meldungen sind feste Texte.
- Der Aufrufer gibt dem Modell bei `darf: false` nur die Zahl der gesperrten Einheiten weiter, nie `grund`, `klasse`, `meldung` oder `leseklasse` (A49, wie Vertrag 3).

**Protokoll (Nahtstelle Vertrag 5):** Die Zeile `ausgabe.gesperrt` nennt bei einer Sperre über die Person den Grund (P1, P2 oder P3), die wirksame Klasse und die Leseklasse. Die Person steht dort nur als Pseudonym (A12), nie als Name und nie als Konto. Das Pseudonym setzt der Aufrufer (Abschnitt 10).

## 6. Aufruf des Außentestprogramms

- `datentor person < fall.json` (oder `datentor person -`): wie `datentor` in Vertrag 3, Abschnitt 5, mit eigenem Fallformat. Grenzen, Exitcodes, Tiefe, doppelte Schlüssel (A139) und die Form von stdout und stderr gelten unverändert: höchstens 1 048 576 Byte, Verschachtelung höchstens 64 Ebenen, das oberste Objekt ist Ebene 1.
- Zahlform: Dieses Fallformat hat kein Zahlenfeld. Eine Zahl an einer Stelle, die Text, Liste oder Objekt verlangt, ist ein falscher Typ (Exit 2). Die Zahlform aus `../zonen/VERTRAG.md`, Abschnitt 6 (Nachtrag 20.11.2026, 2), gilt für jedes künftige Zahlenfeld.
- `datentor` ohne Unterbefehl bleibt unverändert. Kein Fall aus Vertrag 3 ändert sich.

Fall:

```json
{"name": "…", "regeln": ["P3"], "beschreibung": "…",
 "vorgabe": {…},
 "personen": {"leseklassen": [ … ]},
 "anfragen": [{"einheit": {"klasse": "intern"}, "route": "r-in", "kunde": null,
               "kontext": {"nutzer": "p-a", "gruppen": ["g-a"]}}]}
```

- `vorgabe` ist die Vorgabe aus Vertrag 3, Abschnitt 2, unverändert und mit denselben Regeln für Gültigkeit.
- Pflicht sind `vorgabe`, `personen` und `anfragen`. `name`, `regeln` und `beschreibung` wertet das Programm nicht aus.
- Eine Anfrage hat nur die Felder `einheit`, `route`, `kunde`, `kontext` und `lesen` (Abschnitt 10). `einheit` ist ein Objekt mit höchstens dem Feld `klasse` wie in Vertrag 3; einen Lauf ohne Einheit kennt dieser Vertrag nicht. `route` und `kunde` sind Text oder `null`; fehlen sie, gilt `null`.
- `kontext` ist ein Objekt mit höchstens `nutzer` (Text oder `null`, fehlt = `null`) und `gruppen` (Liste von Texten, fehlt = leer, `null` ist ungültig). Fehlt `kontext`, fehlt die Person (P1). `kontext: null` ist ungültig (Exit 2), wie in den Modellzonen.
- Ungültig (Exit 2) ist der Fall zusätzlich zu Vertrag 3, Abschnitt 5, wenn `personen` ungültig ist (Abschnitt 3) oder eine Anfrage nicht diese Form hat.
- Eine Klasse der Einheit, die kein Text ist, ergibt wie in Vertrag 3 den Grund `einheit-ohne-klasse`, falls P1 und P2 nicht vorher greifen.

| Pfad | Zweck |
|---|---|
| `faelle_bauen.py` | schreibt `sichtbar/faelle/` und `sichtbar/erwartet/` aus Tabellen mit ausgeschriebenen Erwartungen; prüft jede gegen eine kleine Referenz der Abschnitte 3 und 4 und jeden Fall gegen die Gültigkeitsregeln; Vorgabe und `darf` nimmt es aus `../faelle_bauen.py` (geladen, nicht kopiert). Gegenprobe: Jeder gültige Fall hat eine Anfrage, deren Grund ohne Personenregel ein anderer wäre. |
| `aussen_laeufer.py` | der Läufer von Vertrag 3 (`../aussen_laeufer.py`, geladen), ruft `DT_BIN person` |
| `sichtbar/` | die sichtbaren Fälle |

```
python3 -I -B faelle_bauen.py
DT_BIN=<programm> DT_FAELLE=sichtbar python3 -I -B aussen_laeufer.py [name ...]
```

Stand: 63 Fälle, davon 18 gültige mit 96 Anfragen und 45 ungültige. Ohne Personenregel hätten 51 der 96 Anfragen einen anderen Grund. Für ein Lesen ohne Route (P7) heißt „ohne Personenregel“: nur D4 oder `frei`.

## 7. Nahtstelle Vertrag 4: Treffer, Zitat und Nachfolger

Zitat und Verweis auf Nachfolger gehören der Suche (Vertrag 4). Das Tor sagt nur, wie sie es fragt:

- Die Suche fragt das Tor für **jede** Ausgabe einer Einheit mit derselben Route und derselben Person: für den Treffer, für ein Zitat aus ihr, für ihren Titel und Pfad und für den Verweis auf einen Nachfolger.
- Ein Nachfolger ist eine eigene Einheit mit eigener Klasse und eigenem Kunden; die Suche fragt das Tor für ihn gesondert. Ein gesperrter Nachfolger erscheint auch nicht als Verweis.
- Fehlt der Suche die Person (etwa ein Aufruf ohne Sitzung), übergibt sie `nutzer: null`; das Tor sperrt dann mit P1.

| Was das Tor braucht | Liefert | Was fehlt |
|---|---|---|
| Leseklassen je Nutzer und Gruppe | Vertrag 2: Admin-Feld `leseklassen` der Firmenvorgabe (Abschnitt 10) | der Lader reicht die Liste unverändert durch und lehnt eine unbekannte Klasse ab (Ladefehler) |
| Nutzer und Gruppen der Sitzung | Vertrag 1 (Start) oder Kern-Dienst | dieselben Felder wie für die Modellzonen; aus der Anmeldung, nie vom Browser |
| Person bei Zitat und Nachfolger | Vertrag 4 (Suche) | Aufruf von `darf_person` je Ausgabe statt `darf` |
| Pseudonym der Person in der Sperrzeile | Vertrag 5 | Feld für die Leseklasse und das Pseudonym in `ausgabe.gesperrt` |

## 8. Vorgabe für den Bau (nicht durch Außentests geprüft)

```rust
pub enum PersonEbene { Nutzer, Gruppe }
pub struct Leseeintrag { pub ebene: PersonEbene, pub wer: String, pub klasse: String }
pub struct Personen { /* privat */ }
impl Personen {
    pub fn neu(v: &Vorgabe, leseklassen: Vec<Leseeintrag>) -> Result<Personen, PersonenFehler>;  // Abschnitt 3, höchstens LESEKLASSEN_MAX
    pub fn leseklasse(&self, p: &Person<'_>) -> Option<&str>;                                  // P4
}
pub struct Person<'a> { pub nutzer: Option<&'a str>, pub gruppen: &'a [&'a str] }
pub fn darf_person(v: &Vorgabe, p: &Personen, einheit: &Einheit<'_>, route: Option<&str>,
                   kunde: Option<&str>, person: &Person<'_>) -> PersonAntwort;
pub enum PersonGrund { Tor(Grund), PersonFehlt, PersonOhneLeseklasse, KlasseUeberPerson }
pub struct PersonAntwort { pub grund: PersonGrund, pub klasse: Option<String>, pub leseklasse: Option<String> }
impl PersonAntwort { pub fn darf(&self) -> bool; pub fn meldung(&self, route: Option<&str>) -> String; }
impl PersonGrund { pub fn code(&self) -> &'static str; }
pub const LESEKLASSEN_MAX: usize = 4096;                                                       // Nachtrag 27.11.2026
pub enum PersonenFehler { ZuViele(usize), SchluesselForm(String), KlasseUnbekannt { wer: String, klasse: String }, Doppelt(String) }
pub fn lesen_person(v: &Vorgabe, p: &Personen, einheit: &Einheit<'_>, kunde: Option<&str>,
                    person: &Person<'_>) -> PersonAntwort;                                     // P7, Abschnitt 10
```

- `Vorgabe`, `Vorgabe::neu`, `darf`, `Einheit`, `Antwort` und `Grund` aus Vertrag 3 bleiben mit ihren Signaturen. `Grund` bekommt keine neue Variante; die neuen Gründe stehen in `PersonGrund`.
- `darf_person` baut auf `darf` auf: Es ruft `darf` und setzt P1, P2 und P3 nach Abschnitt 4 ein. `PersonEbene` ist ein eigener Typ, damit `Ebene` der Klassen-Tiefe und der Modellzonen unberührt bleibt.
- `darf_person` ist rein und löst bei keiner Eingabe eine Panik aus (D15).
- Prüfung des Baus: sichtbare und verdeckte Fälle grün, Vertrag 3 weiter grün, voller Mutationslauf ohne überlebende Mutanten in der Bibliothek (wie Vertrag 3, Abschnitt 7).

## 9. Fragen

**Entschieden (Projektentscheid, 20.11.2026):** Alle sechs Empfehlungen gelten wie unten. Zu F2 gilt zusätzlich: Den Eintrag einer Person setzt nur ein Admin, und jede Änderung steht im Protokoll (Abschnitt 2). F5 kommt als Nachtrag beider Teilverträge, wenn die zusammengesetzte Funktion gebaut wird.

- **F1, mehrere Gruppen:** Dieser Vertrag nimmt die **höchste** Leseklasse der Gruppen. Empfehlung: so lassen. Eine Leseklasse ist eine Erlaubnis, keine Regel; Erlaubnisse aus Gruppen addieren sich. Mit der niedrigsten würde eine Person, die neu in eine eng gefasste Gruppe kommt, Rechte aus ihrer anderen Gruppe verlieren, und Admins müssten Mitgliedschaften gegeneinander abwägen. Die Sicherheit bleibt, weil die Gruppen aus der Anmeldung kommen, nie vom Agenten, und Route und Kundenregeln weiter gelten.
- **F2, eigener Eintrag gegen Gruppen:** Dieser Vertrag lässt den Eintrag der Person vor ihren Gruppen gelten, auch nach unten. Empfehlung: so lassen. Der Admin kann so eine einzelne Person enger setzen, ohne sie aus einer Gruppe zu nehmen; das entspricht A26 („für einen einzelnen Nutzer kann die Firma andere Regeln setzen“). Die andere Lesart (höchster Wert aus Person und Gruppen) ließe keine Verengung je Person zu.
- **F3, Person ohne Eintrag:** Ohne Eintrag sperrt das Tor alles (P2), es gibt keine Vorgabe-Leseklasse. Empfehlung: so lassen (fail closed). Damit eine Installation nicht leer startet, legt die Erstausstattung (Vertrag 2) einen Gruppeneintrag für alle Personen an; das ist eine Sache der Einstellungen, nicht des Tors.
- **F4, Kundenachse je Person:** A05 nennt nur die Leseklasse. Empfehlung: Eine Beschränkung der Person auf Kunden nicht in diesen Vertrag. Sie gehört zum Kundenraum (A53) und zu den Regeln je Kunde (A51); dort hängt sie an der Sitzung, nicht an der Klasse. Kommt sie, wird dieser Vertrag erweitert, ohne bestehenden Fall abzuschwächen; ein Feld `kunden` am Eintrag ist heute unbekannt und macht den Fall ungültig.
- **F5, Verbindung mit den Modellzonen:** Beide Prüfungen gelten zusammen, der strengere Wert gilt. Empfehlung für die Reihenfolge, wenn beide an sind: D2, Z1, P2, Z2, D4, Z3 bis Z6, D6, P3, D7 bis D11, Z7 und D11 wie in den Modellzonen. Der Grund für die Person steht so bei derselben Art von Grund (Sitzung vor Zone, Klasse nach Klasse). Das gehört in einen Nachtrag beider Teilverträge, nicht in diesen Stand.
- **F6, Leseklasse in der Antwort:** Die Antwort nennt die Leseklasse der Person, damit die Sperrzeile sie ohne zweite Rechnung schreiben kann. Empfehlung: so lassen; sie ist ein Klassenschlüssel und kein Personenbezug. Soll Vertrag 5 sie nicht führen, entfällt nur das Feld.

## 10. Nachtrag 27.11.2026: Quelle, Bindung und Lesen ohne Route

Anlass: Das Ordner-Blatt des Dienstes (Vertrag `dienst`, Nachtrag 26.11.2026, L9) braucht die Entscheidung „darf diese Person Inhalt dieser Klasse lesen“. Bis dahin lehnt der Dienst jedes Lesen ab (`klasse_lesbar`, fest `false`).

**Quelle (V).** Die Einträge stehen im Admin-Feld `leseklassen` der Firmenvorgabe (Vertrag 2). Die Form ist genau die Liste aus Abschnitt 3; der Lader reicht sie unverändert an `Personen::neu` durch. Das Feld setzen nur Admins, auf der Ebene der Firma; Nutzer und Agent setzen und lockern es nicht. Eine Karte Konto auf Klasse ist keine gültige Form, weil sie keine Gruppen kennt (F1, F3).

**Schlüssel (V).** `wer` ist bei `ebene: nutzer` das Pseudonym der Person (`^ps_[0-9a-f]{32}$`, wie `rollen`, `gruppen.mitglieder` und `nutzerregeln` in `rust/spec/regelebenen/README.md`), bei `ebene: gruppe` der Schlüssel einer Gruppe aus `gruppen` derselben Datei. Das Pseudonym hat die Schlüsselform aus Abschnitt 3. Ein Konto erreicht das Tor nie (A12).

**Bindung (V).** Der Aufrufer übergibt je Frage `kontext.nutzer` gleich dem Pseudonym des Betrachters (`betrachter.person` im Dienst, über die Zuordnung des Dienstes) und `kontext.gruppen` gleich den Gruppen, in denen dieses Pseudonym Mitglied ist. Die Leseklasse rechnet nur das Tor nach P4 (`Personen::leseklasse`); der Aufrufer rechnet sie nicht nach. Fehlt der Eintrag, hat die Person keine Leseklasse und bekommt nichts (P2); es gibt keine Vorgabe-Leseklasse.

**P7, Lesen ohne Route (V).** `lesen_person` entscheidet, ob die Person Inhalt selbst lesen darf; nichts geht dabei an ein Modell.

| Reihenfolge | Regel | Code |
|---|---|---|
| 1 | P1 | `person-fehlt` |
| 2 | P2 | `person-ohne-leseklasse` |
| 3 | D4 | `einheit-ohne-klasse` |
| 4 | P3 | `klasse-ueber-person` |
| 5 | D1 | `frei` |

- Die wirksame Klasse ist D5: Inhalt mit Kunde hat die höchste Klasse, Inhalt ohne Klasse die `vorgabestufe`. Kennt der Aufrufer die Klasse eines Inhalts nicht, übergibt er ihn ohne Klasse; dann gilt die `vorgabestufe`, deren Vorgabe die höchste Klasse ist (A35).
- Die Regeln der Route (D2, D3, D6) und der Kunden (D7 bis D11) gelten nicht. Sie sagen, welche Route einen Inhalt sehen darf. Beim Lesen gibt es keine Route. Darum ist Inhalt der höchsten Klasse ohne Kunde für eine Person mit der höchsten Leseklasse `frei`, wo eine Route D7 meldet.
- Unterhalb der höchsten Klasse und ohne Kunde gibt `lesen_person` dieselbe Antwort wie `darf_person` auf einer lokalen Route der höchsten Klasse (Test `p7_lesen_ohne_route`).
- Die Antwort hat die Form aus Abschnitt 5; `meldung` nennt keine Route.

**Fallformat.** Eine Anfrage mit `"lesen": true` ist ein Lesen ohne Route. Sie hat kein Feld `route`, auch nicht `null`. `lesen` hat nur den Wert `true`; `false`, `null` oder ein anderer Typ sind ungültig (Exit 2). Neue Fälle: `pl-lesen`, `pl-reihenfolge`, `pl-vorgabestufe`, `pl-pseudonym`, `p-4096-eintraege`, `u-p-lesen-mit-route`, `u-p-lesen-route-null`, `u-p-lesen-falsch`, `u-p-lesen-text`, `u-p-lesen-null` und `u-p-zu-viele`.

**Offen für den Dienst.** Der Dienst kennt heute weder die Klasse noch den Kunden des Inhalts eines Arbeitsordners. Ohne Klasse gilt die `vorgabestufe`; ohne Kunde gilt kein Kunde. Ob eine Sitzung mit Kunde ihren Kunden mitgibt, entscheidet der Vertrag `dienst`.
