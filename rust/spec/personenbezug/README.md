# Vertrag 9: Personenbezug, Auskunft und Löschen je Person (Entwurf, Fassung 1)

Stand 20.11.2026. Baustein Formate. Die Nummer 9 ist vorläufig; 7 und 8 tragen Gateway und Modellweg.
Anforderungen: A12 (Personenbezug nur als Pseudonym, Freitexte nur als Hash, Auskunft und Löschen je Person), A102 (Hashkette, eine geänderte Zeile macht die Prüfung rot), A103 (Aufbewahrung als Einstellung mit Vorgabewert), A104 (kein Pfad, Titel oder Name in einer Zeile, nie Klartext).
Grundlagen: Vertrag 5 (Kette, Fassung 2, `kette pruefe`), Vertrag 6 (Pseudonym `^ps_[0-9a-f]{32}$` in `person` und `entscheidend`), der Entwurf zur Herabstufung im Audit, Abschnitt 4 (F2: zwei getrennte Schlüssel; die Stelle hält den für Personen) und der heutige Weg P1 der Stelle in Python (`the previous implementation`, Kapitel „Pseudonyme und Freitexte (P1)“).

Der Vertrag legt Bildung, Speicher, Auskunft, Löschen und Bytes fest, bevor Code entsteht. Es gibt noch keinen Rust-Code; gebaut wird nichts in diesem Schritt.
Die sichtbaren Fälle stehen in `faelle/`; verdeckte Fälle und der Mutationslauf folgen mit dem Bau. „(V)“ heißt: in diesem Vertrag festgelegt. „(Annahme)“ heißt: festgelegt, bis das Projekt anders entscheidet.
Alle Kennungen, Schlüssel, Geheimnisse und Texte in den Fällen sind erfunden.

## 1. Die Frage und die Antwort

Die Kette ist nur anhängbar. Eine geänderte Zeile macht sie rot (Vertrag 5). Trotzdem verlangt A12 Auskunft und Löschen je Person: Nach dem Löschen findet die Einsicht keinen Freitext der Person, und die Kette prüft weiter grün.

Die Antwort (V): **Die Kette trägt nie etwas, das ohne einen zweiten, löschbaren Speicher auf eine Person zeigt.**

- Eine Person steht in einer Zeile nur als Pseudonym. Das Pseudonym ist ein HMAC mit dem **Schlüssel der Stelle** über ein **Geheimnis der Person** (32 Zufallsbytes). Beide liegen außerhalb der Kette.
- Ein Freitext steht in einer Zeile nur als Hash. Der Hash ist ein HMAC mit demselben Schlüssel über das Geheimnis der Person und den Text. Der Text selbst liegt beim Geheimnis der Person.
- Der **Speicher der Zuordnung** hält je Person Kennung, Geheimnis und Texte.
- **Löschen** heißt: Geheimnis und Texte der Person unwiderruflich vernichten. Die Zeilen bleiben Byte für Byte, die Kette bleibt grün. Ohne Geheimnis lässt sich das Pseudonym nicht mehr bilden, auch nicht mit dem Schlüssel und der Kennung; ein Freitext-Hash lässt sich auch mit dem Schlüssel und dem geratenen Text nicht mehr bestätigen.

Der Vorschlag des Auftrags („Pseudonym als HMAC mit einem Schlüssel über die Person, Salz je Person nur für Freitexte“) ist darin enthalten, mit einer Änderung: Das Pseudonym hängt am Geheimnis der Person, nicht an ihrer Kennung. Hinge es an der Kennung, könnte jeder mit dem Schlüssel und der Kennung das Pseudonym nach dem Löschen neu rechnen. Das Löschen wirkte dann nicht gegen den Betreiber, der den Schlüssel ohnehin hält. Die Abwägung steht in Abschnitt 12.

## 2. Schnitt

**Bibliothek ohne Ein- und Ausgabe** (künftig Crate `formate`, Modul `personenbezug`; Kanonisierung aus Modul `kette` von Vertrag 5):

```rust
pub fn pseudonym(schluessel: &[u8; 32], geheimnis: &[u8; 32]) -> String;                       // "ps_" + 32 hex
pub fn freitext(schluessel: &[u8; 32], geheimnis: &[u8; 32], text: &str) -> Result<String>;   // "ft_" + 64 hex
pub fn loeschmarke(schluessel: &[u8; 32], geheimnis: &[u8; 32]) -> String;                    // 64 hex
pub fn auszug(zeilen: &[Zeile], eigene: &Eigene) -> Result<Vec<Auszugzeile>>;                  // Abschnitt 5
pub fn aufloesen(zeilen: &[Zeile], karte: &Karte) -> Result<Vec<Einsichtzeile>>;              // Abschnitt 7
pub fn zeile_geloescht(&Geloescht, vorher: &str) -> Result<String>;                            // Abschnitt 8
pub fn zeile_auskunft(&Auskunft, vorher: &str) -> Result<String>;                              // Abschnitt 8
```

Die Bibliothek liest keine Datei, keine Uhr und keinen Zufall. Zeit und Zufall kommen als Eingabe. Bei gleicher Eingabe liefert jede Funktion dasselbe; keine löst eine Panik aus.

**Kleiner Speicher** (Abschnitt 4): ein Typ `Speicher` mit `oeffnen`, `person`, `text`, `auskunft`, `loeschen`, `zuordnen`, `einsicht`, `wechseln` und `abgleich`. Er liest und schreibt nur seinen eigenen Ordner. Die Kette liest er über die Bibliothek von Vertrag 5; Zeilen hängt er nur über `kette` an.

**Eigentümer** ist die Stelle (F2). Kein anderer Dienst hält Schlüssel, Geheimnisse oder Texte. Gateway, Suche und Brain-Dienst fragen das Pseudonym einer Person bei der Stelle; die Vorgänge dafür legt der Vertrag der Stelle fest (heute in Python `person.pseudonym` und `freitext.ablegen`).
Der Schlüssel des Brain-Dienstes für Einheiten und Inhalte (`eh_`, `inhalt_mac`, A104) ist ein anderer Schlüssel und nicht Teil dieses Vertrags.

## 3. Bildung

**Kennung einer Person** (V): eine Kennung nach Vertrag 2 (`^[a-z0-9][a-z0-9._-]{0,63}$`), die nicht mit `ps_` oder `ft_` beginnt. Sonst `feld_ungueltig:kennung`.
Vertrag 2 erlaubt den Unterstrich. Das Projekt verbietet dort die Vorsilben `ps_`, `ft_` und `eh_` (20.11., angekündigt); bis Vertrag 2 das trägt, gilt die Regel hier allein.

**Schlüssel** (V): 32 Byte. **Geheimnis der Person** (V): 32 Byte, beim Anlegen der Person aus dem Zufall des Betriebssystems. Zwei Personen haben nie dasselbe Geheimnis (`geheimnis_doppelt`).
In Dateien und Fällen stehen beide als 64 kleine Hexziffern.

| Wert | Bildung | Form |
|---|---|---|
| Pseudonym | `"ps_"` + die ersten 32 Hexziffern von HMAC-SHA256(Schlüssel, `"wb-person-pseudonym"` LF ‖ Geheimnis) | `^ps_[0-9a-f]{32}$` wie Vertrag 6 |
| Freitext | `"ft_"` + HMAC-SHA256(Schlüssel, `"wb-person-freitext"` LF ‖ Geheimnis ‖ Text in UTF-8), 64 Hexziffern | `^ft_[0-9a-f]{64}$` |
| Löschmarke | HMAC-SHA256(Schlüssel, `"wb-person-loeschung"` LF ‖ Geheimnis), 64 Hexziffern | `^[0-9a-f]{64}$` |

- Die Zweckwörter sind ASCII, LF ist 0x0A, das Geheimnis geht als 32 Rohbytes ein. Weil das Geheimnis eine feste Länge hat, ist die Folge Geheimnis ‖ Text eindeutig.
- Die drei Zweckwörter trennen die Werte. Ein Wert gilt nie für einen anderen Zweck.
- **Text** (V): 1 bis 4096 Unicode-Skalarwerte, unverändert (keine Normalisierung, kein Kürzen). Sonst `text_ungueltig`. Längere Texte kürzt der Aufrufer vorher selbst; P1 kürzt heute stillschweigend auf 4096 Zeichen.
- **Wo die Werte stehen.** Ein Pseudonym steht in Feldern für Personen (Vertrag 6: `person`, `entscheidend`; hier `handelnd`). Ein Freitext steht als `ft_…` in einem Feld, das die jeweilige Zeilenart als Freitextfeld festlegt. Eine Zeile trägt nie eine Kennung, ein Geheimnis oder einen Text.
- **Erkennen an der Form** (V): Auskunft und Einsicht erkennen Pseudonyme und Freitexte an der Form ihres Werts, in jeder Tiefe der Zeile, nicht am Feldnamen. Darum darf kein anderer Wert einer Zeile die Form `ps_…` oder `ft_…` haben. Das sichern Vertrag 2, der die Vorsilben `ps_`, `ft_` und `eh_` für alle Kennungen (Klassen, Routen, Kunden, Freigaben) in jeder Fassung mit eigenem Code verbietet (Projektentscheid, 20.11.2026, angekündigt; etwa `einst.kennung_vorsilbe`), und Vertrag 1, in dem `sitzung` die feste Form `s-N` hat.
- Dieselbe Person hat unter demselben Schlüssel immer dasselbe Pseudonym; verschiedene Personen haben verschiedene; ein anderer Schlüssel ergibt andere (Fall `pseudonym`).

## 4. Speicher der Zuordnung

**Ordner** `<stelle>/personenbezug/`, Modus 0700, Eigentümer das Konto der Stelle. **Der Ordner liegt nie in einem Git-Arbeitsbaum und außerhalb aller Repos** (V, Projektentscheid, 20.11.2026): Repos werden nie umgeschrieben und täglich intern und extern gesichert; eine committete Zuordnung bliebe für immer und wäre durch kein Löschen mehr zu erreichen. Darin genau:

| Datei | Inhalt | Modus |
|---|---|---|
| `schluessel.json` | `{"fassung":1,"schluessel":[{"ab_ms":<ms>,"wert":"<64 hex>"},…]}`; der letzte Eintrag ist der laufende Schlüssel, `ab_ms` steigt streng | 0600 |
| `zuordnung.json` | `{"fassung":1,"personen":{"<kennung>":{"geheimnis":"<64 hex>","seit_ms":<ms>,"texte":{"ft_…":{"text":"…","zuletzt_ms":<ms>}}}}}` | 0600 |
| `zuordnung.sperre` | leer; nur für die Sperre | 0600 |

- Beide JSON-Dateien stehen in RFC 8785 (`kette::kanonisch`) mit einem LF am Ende. Jeder Text unter `texte` ist unter dem Schlüssel abgelegt, der bei seiner letzten Ablage lief; `zuletzt_ms` ist die Zeit dieser Ablage.
- **Getrennte Dateien** (V): Schlüssel und Zuordnung liegen in zwei Dateien. Eine Kopie nur der Zuordnung (etwa in einem Fehlerbericht) ordnet ohne den Schlüssel kein Pseudonym zu; eine Kopie nur des Schlüssels kennt keine Person.
- **Größe** (V): `zuordnung.json` höchstens 33 554 432 Byte. Eine Ablage darüber lehnt mit `speicher_voll` ab.

**Schreiben** (V): Jede Änderung liest und schreibt unter einer exklusiven Sperre auf `zuordnung.sperre` (`std::fs::File::lock`, wie Vertrag 5). Unter der Sperre liest sie den Stand neu, ändert ihn und ersetzt die Datei atomar:
neue Datei `.zuordnung.json.<zufall>` im selben Ordner mit `O_CREAT|O_EXCL|O_NOFOLLOW` und Modus 0600, alles schreiben, `sync_all`, `rename` über die alte Datei, dann `sync_all` auf den Ordner. Bei jedem Fehler bleibt die alte Datei unverändert, und die neue wird entfernt.
`schluessel.json` wird nur von `oeffnen` (Anlegen) und `wechseln` geschrieben, auf dieselbe Weise.

**`oeffnen(ordner, zufall, jetzt_ms)`** prüft in dieser Reihenfolge; der erste Treffer nennt den Fehler (Fall `speicher-oeffnen`):

| Prüfung | Fehler |
|---|---|
| Ordner fehlt, ist kein Ordner, ein Verweis, gehört nicht dem Prozess oder hat Rechte für Gruppe oder andere | `speicher_unsicher` |
| der Ordner oder ein Ordner darüber bis zur Wurzel enthält einen Eintrag `.git` (Ordner, Datei oder Verweis; die Suche geht über den aufgelösten Pfad aufwärts) | `speicher_im_repo` |
| eine der beiden Dateien ist ein Verweis, keine gewöhnliche Datei, gehört nicht dem Prozess oder hat Rechte für Gruppe oder andere | `speicher_unsicher` |
| beide Dateien fehlen | legt beide an: Schlüssel aus `zufall`, `ab_ms` = `jetzt_ms`, keine Person |
| nur `schluessel.json` fehlt | `schluessel_fehlt` |
| nur `zuordnung.json` fehlt | `zuordnung_fehlt` |
| eine Datei ist nicht kanonisch, hat nicht die Form oben, eine Kennung verletzt Abschnitt 3, ein Geheimnis kommt doppelt vor, ein Schlüssel doppelt, oder `ab_ms` steigt nicht | `speicher_ungueltig` |

Fehlt `zuordnung.sperre`, legt `oeffnen` sie an. Fehlt nur eine der beiden JSON-Dateien, legt `oeffnen` nichts neu an (fail closed, Abschnitt 9).
Ohne geöffneten Speicher schreibt die Stelle keine Zeile, die eine Person nennt, und startet nicht (wie P1).

**Vorgänge des Speichers:**

| Vorgang | Wirkung | Fehler |
|---|---|---|
| `person(kennung, zufall, jetzt_ms)` | bekannte Person: ihr Pseudonym unter dem laufenden Schlüssel, `neu: false`, der Zufall bleibt ungenutzt. Neue Person: Eintrag mit dem Zufall als Geheimnis, `seit_ms`, `neu: true` | `feld_ungueltig:kennung`, `geheimnis_doppelt`, `zeit_ungueltig` |
| `text(kennung, text, jetzt_ms)` | legt den Text bei der Person ab und gibt `ft_…` unter dem laufenden Schlüssel zurück; derselbe Text steht einmal, `zuletzt_ms` wird neu gesetzt | `feld_ungueltig:kennung`, `person_unbekannt`, `text_ungueltig`, `speicher_voll` |
| `zuordnen(wert)` | Kennung zu einem Pseudonym (unter jedem Schlüssel) oder einem abgelegten Freitext, sonst `null` | keine |

`zuordnen` ist nur für die Einsicht mit Rolle und für die Prüfung gedacht. Es gibt die Kennung nie an einen Dienst ohne Rolle.

## 4a. Sicherungen (Anforderung an jede Sicherung)

Entschieden (Projektentscheid, 20.11.2026, gemessen auf Mac und dem Linux-Baurechner): **Jede Sicherung von `zuordnung.json` und `schluessel.json` wird nach höchstens 30 Tagen vernichtet.**
Das gilt für jede Sicherung dieser Dateien, gleich wer sie anlegt: die Sicherung der Werkbank, die Sicherung des Betreibers auf eigenen Rechnern (die wir nicht kennen), Kopien von Hand, Archive des Installers und Schnappschüsse des Dateisystems.
Die Unterlagen für Betreiber nennen diese Anforderung; die Stelle kann sie für fremde Sicherungen nicht erzwingen.

- Die Frist sorgt dafür, dass ein Löschen nach A12 spätestens nach 30 Tagen auch in allen Sicherungen wirkt.
- Kommt eine Sicherung zurück, läuft vor der ersten Zeile der Abgleich mit den Löschmarken der Kette (Abschnitt 6). So bleibt eine nach der Sicherung gelöschte Person gelöscht.
- Eine Sicherung gehört wie der Ordner nie in ein Repo (Abschnitt 4).

## 5. Auskunft je Person

`auskunft(kennung, handelnd, dateien, jetzt_ms)` liefert alles, was die Stelle über die Person hält. `dateien` sind alle Kettendateien aller Quellen (Vertrag 5: `gateway`, `stelle`, `brain`, `halter`), auch die Altformen, soweit sie noch aufbewahrt sind.

**Antwort** (V):

```
{"gefunden":true,"pseudonyme":["ps_…",…],"texte":[{"freitext":"ft_…","text":"…","zuletzt":"<zeit>"},…],"zeilen":[{"datei":"stelle-2026-11.jsonl","nr":1,"sha256":"<64 hex>","zeile":{…}},…]}
{"gefunden":false,"pseudonyme":[],"texte":[],"zeilen":[]}
```

- `pseudonyme`: das Pseudonym der Person unter jedem Schlüssel aus `schluessel.json`, in der Reihenfolge der Schlüssel.
- `texte`: alle abgelegten Texte der Person, nach `freitext` sortiert.
- `zeilen`: jede Zeile, in der irgendwo ein Wert eines ihrer Pseudonyme oder einer ihrer Freitexte ist, in der Reihenfolge der Dateien und Zeilen. `datei` ist nur der Dateiname, `nr` zählt ab 1 je Datei, `sha256` ist der Hash der Rohbytes. Mit `sha256` kann die Person oder eine Prüferin die Zeile gegen die Kette prüfen.
- **Ohne Inhalte anderer** (V): In `zeile` ersetzt die Auskunft jeden Wert der Form `ps_…`, der nicht eines ihrer Pseudonyme ist, und jeden Wert der Form `ft_…`, der nicht einer ihrer Freitexte ist, durch den Text `anderer`. Alle übrigen Werte bleiben, auch `vorher`.
- **Vollständig oder gar nicht** (V): Ist eine Datei nicht lesbar oder eine Zeile kein JSON-Objekt, liefert die Auskunft keine Antwort, sondern `zeile_unlesbar:<datei>:<nr>` oder `datei_nicht_lesbar:<datei>`. Eine Teilauskunft sähe aus wie eine vollständige.
- Die Auskunft prüft die Kette nicht. Das ist Sache von `kette pruefe`.
- Eine unbekannte Person, eine gelöschte Person und eine nie angelegte Person ergeben dieselbe Antwort `gefunden: false`.

**Zeile vor Antwort** (V, wie E2 in Vertrag 6): Vor der Antwort hängt der Speicher eine Zeile `person.auskunft` an (Abschnitt 8). Scheitert das, gibt es keine Antwort.
Wer eine Auskunft verlangen darf (die Person selbst, ein Admin), legt der Vertrag der Stelle fest, nicht dieser.

## 6. Löschen je Person

`loeschen(kennung, handelnd, jetzt_ms)`:

1. Die Kennung wird geprüft (`feld_ungueltig:kennung`; dann entsteht keine Zeile).
2. Unter der Sperre: Ist die Person bekannt, berechnet der Speicher die Löschmarke unter dem laufenden Schlüssel. Ist `handelnd` eines ihrer Pseudonyme (sie löscht sich selbst), wird `handelnd` zu `null`.
3. **Erst die Zeile** `person.geloescht` (Abschnitt 8) mit `ergebnis` `geloescht` und der Löschmarke, oder bei einer unbekannten Person `unbekannt` mit `loeschmarke` `null`.
4. **Dann der Speicher:** Der Eintrag der Person mit Geheimnis und allen Texten verschwindet aus `zuordnung.json` (atomar ersetzt).
5. Antwort `{"ergebnis":"geloescht"}` oder `{"ergebnis":"unbekannt"}`.

**Was vernichtet wird:** Kennung, Geheimnis und alle Texte der Person im Speicher. **Was bleibt:** jede Zeile der Kette Byte für Byte, also auch die Pseudonyme und Freitext-Hashes der Person; der Schlüssel.
Danach findet die Auskunft nichts (`gefunden: false`), die Einsicht zeigt `nicht_zuzuordnen`, `zuordnen` gibt `null`, und `kette pruefe` ist grün (Fall `ablauf-auskunft-und-loeschen`).
Eine Person mit derselben Kennung, die später neu angelegt wird, bekommt ein neues Geheimnis, also ein neues Pseudonym. Sie erbt keine alte Zeile (Fall `loeschen-zweimal`).

**Zweimal löschen und unbekannte Person** (V): Der zweite Aufruf findet keinen Eintrag und schreibt `unbekannt` mit `loeschmarke` `null`, genau wie für eine Kennung, die es nie gab. Die Zeile verrät also nicht, ob eine Person je bestand.

**Warum erst die Zeile:** So gilt E2 aus Vertrag 6 (erst die Zeilen, dann der Zustand). Stürzt die Stelle zwischen Zeile und Speicher ab, steht die Löschmarke in der Kette, die Person aber noch im Speicher. Das behebt der **Abgleich**:
`abgleich(dateien)` sammelt die Löschmarken aller Zeilen `person.geloescht` mit `ergebnis` `geloescht` und entfernt jede Person, deren Löschmarke unter irgendeinem Schlüssel darunter ist. Die Stelle ruft ihn nach `oeffnen` auf, bevor sie die erste Zeile schreibt.
Derselbe Abgleich wirkt, wenn eine alte Sicherung des Speichers zurückkommt: Personen, die nach der Sicherung gelöscht wurden, verschwinden wieder (Fall `abgleich-nach-wiederherstellung`). Ein zweiter Abgleich entfernt nichts. **Nach jedem Zurückspielen** einer Sicherung von `zuordnung.json` oder `schluessel.json` läuft der Abgleich, bevor die Stelle eine Zeile schreibt; das gilt auch für ein Zurückspielen von Hand.

Die Löschmarke zeigt ohne das Geheimnis auf kein Pseudonym. Nach dem Löschen ist das Geheimnis fort; auch mit dem Schlüssel lässt sich die Marke keinem Pseudonym der Kette zuordnen.

Wer löschen darf und mit welchem Nachweis, legt der Vertrag der Stelle fest. Das Löschen hebt die Aufbewahrung nicht auf: Die Zeilen bleiben bis zu ihrer Frist (Abschnitt 10).

## 7. Einsicht

`einsicht(dateien)` ist die Ansicht für die Rolle mit Einsicht (A102, A103). Sie gibt jede Zeile zurück und ersetzt jeden Wert der Form `ps_…` durch die Kennung der Person und jeden Wert der Form `ft_…` durch den abgelegten Text, solange der Speicher beides kennt. Sonst steht dort `nicht_zuzuordnen`.
Die Zeile in der Kette bleibt, wie sie ist. Nach dem Löschen findet die Einsicht weder Kennung noch Text der Person (Fall `ablauf-auskunft-und-loeschen`, Schritt 16, ab 0 gezählt).
Rolle, vier Augen, Zweck und die Zeile der Einsicht selbst legt der Vertrag der Stelle fest. Ein Export der Einsicht enthält Kennungen und Texte im Klartext; siehe Grenze 6.

## 8. Zeilen

Beide Zeilen sind Fassung 2 nach Vertrag 5 (RFC 8785, `vorher`, `zeit`, eine Zeile je Glied, LF). Quelle ist `stelle`, Datei `stelle-<JJJJ-MM>.jsonl`.

```
{"aktionsname":"person.geloescht","ergebnis":"geloescht","fassung":2,"handelnd":"ps_…","loeschmarke":"<64 hex>","vorher":"<64 hex>","zeit":"2026-11-20T13:00:30.000Z"}
{"aktionsname":"person.auskunft","ergebnis":"geliefert","fassung":2,"handelnd":"ps_…","vorher":"<64 hex>","zeilen":3,"zeit":"2026-11-20T13:00:20.000Z"}
```

| Feld | `person.geloescht` | `person.auskunft` |
|---|---|---|
| `ergebnis` | `geloescht` oder `unbekannt` | `geliefert` oder `unbekannt` |
| `handelnd` | Pseudonym des Menschen, der löscht; `null` bei Selbstlöschung | Pseudonym des Menschen, der fragt |
| `loeschmarke` | 64 hex bei `geloescht`, `null` bei `unbekannt` | — |
| `zeilen` | — | Zahl der gelieferten Zeilen, 0 bis 4294967295; 0 bei `unbekannt` |

- **Geschlossene Form.** Genau diese Felder. Keine Zeile nennt die Kennung, das Pseudonym oder einen Text der betroffenen Person (A12, A104).
- **Feldfehler** in dieser Reihenfolge: `ergebnis`, `handelnd`, `loeschmarke` bzw. `zeilen`, `vorher`, dann `zeit_ungueltig` (Fall `zeilen`).
- Die Zeile `person.auskunft` nennt nur den Fragenden. Fragt die Person selbst, steht dort ihr Pseudonym; nach ihrem Löschen ist es nicht mehr zuzuordnen.
- Beide Aktionsnamen sind neu. `kette pruefe` prüft sie wie jede Zeile der Fassung 2 und kennt ihre geschlossene Form noch nicht (Frage F3).

## 9. Wechsel und Verlust des Schlüssels

**Wechsel** (`wechseln(wert, jetzt_ms)`, V): Ein neuer Schlüssel kommt als neuer letzter Eintrag nach `schluessel.json`. Er muss neu sein (`schluessel_ungueltig`), und `ab_ms` muss größer sein als beim bisherigen letzten (`zeit_ungueltig`).
Ein Wechsel geschieht nur beim Laden der Stelle. Ein Neustart macht ohnehin alle Sitzungen ungültig (Vertrag 6), also trägt keine laufende Sitzung ein altes Pseudonym weiter.
- Nach dem Wechsel hat jede Person ein neues Pseudonym. Zeilen vor und nach dem Wechsel lassen sich über das Pseudonym nicht mehr verbinden, ohne den Speicher.
- Die Auskunft rechnet das Pseudonym unter jedem Schlüssel und findet beide Teile (Fall `schluessel-wechsel`). Texte bleiben unter dem Hash ihrer letzten Ablage; derselbe Text nach dem Wechsel steht ein zweites Mal unter dem neuen Hash.
- Ein alter Schlüssel bleibt, bis die Aufbewahrung die letzte Zeile unter ihm entfernt hat: Er darf weg, wenn `ab_ms` seines Nachfolgers plus Frist vorbei ist. Das Entfernen gehört zur Pflege (nicht im Durchstich).

**Verlust des Schlüssels.** Fehlt `schluessel.json`, während `zuordnung.json` steht, öffnet der Speicher nicht (`schluessel_fehlt`). Die Stelle startet nicht. Der Schlüssel kommt aus der Sicherung zurück.
Ein neuer Schlüssel von Hand ist ein bewusster Notwechsel: Die alten Pseudonyme lassen sich dann nie mehr rechnen. Die Auskunft fände die alten Zeilen nicht mehr und müsste das als Lücke melden. Den Befehl dafür und die Kennzeichnung der Lücke legt dieser Vertrag noch nicht fest.
**Verlust der Zuordnung.** Fehlt `zuordnung.json`, während der Schlüssel steht, öffnet der Speicher nicht (`zuordnung_fehlt`). Ein stiller Neuanfang würde alle Personen auf einmal löschen und die Auskunft stumm leer machen.

## 10. Aufbewahrung (A103)

- **Einstellung:** Feld `aufbewahrung_tage` im Block `protokoll` der Firmendatei, Vertrag Regelebenen (`rust/spec/regelebenen/README.md`, Abschnitt Felder; development vom 20.11.2026). Ganzzahl 0 bis 36500; 0 heißt „nichts löschen“. Nur die Firma setzt den Wert. **Vorgabe 365 Tage** (Regelebenen: fehlt der Block, gilt `{"aufbewahrung_tage": 365, …}`). Kein Feld von Vertrag 2 und keines der Stelle; einziger Ort ist Regelebenen, Block `protokoll` (Befund B10, 21.11.).
  Der Erststart stellt dazu keine Frage; er nennt den Wert, die Konsole zeigt und ändert ihn (A103, A134). Dieser Vertrag liest den Wert nur; er ist kein Feld von Vertrag 2.
- **Kette:** Gelöscht wird nur eine ganze Monatsdatei, und nur, wenn das Monatsende plus Frist vorbei ist. Danach beginnt die Prüfung an der nächsten Datei mit deren erstem `vorher` aus einem Anker (Vertrag 5, Abschnitt 5).
- **Texte im Speicher:** Ein Text darf weg, wenn `zuletzt_ms` plus Frist vorbei ist. Dann ist auch die letzte Zeile mit seinem Hash fort.
- **Personen im Speicher:** Ein Eintrag bleibt, bis die Person gelöscht wird (Abschnitt 6). Ein Eintrag einer ausgetretenen Person, deren letzte Zeile abgelaufen ist, darf durch die Pflege weg; das braucht die Zeit der letzten Nennung und ist nicht im Durchstich.
- Die Pflege löscht vor Ablauf der Frist nichts. Eine Löschung nach A12 wirkt sofort und unabhängig von der Frist.

## 11. Grenzen

1. **Kontextwissen.** Das Löschen trennt die Zeilen von der Person, nicht die Zeilen voneinander. Alle Zeilen einer Person tragen bis zum nächsten Schlüsselwechsel dasselbe Pseudonym.
   Wer die Person schon kennt und eine ihrer Zeilen erkennt (etwa „sie hat um 13:00 in Sitzung s-1 entschieden“), findet über das Pseudonym alle anderen. Auch `sitzung`, `route`, `kunde` und `zeit` können in einem kleinen Team auf eine Person zeigen. Das Löschen macht die Zeilen nicht anonym, sondern nur ohne Speicher unzuordenbar.
2. **Sicherungen.** Eine Sicherung von `zuordnung.json` aus der Zeit vor dem Löschen ordnet zusammen mit dem Schlüssel wieder zu. Der Abgleich entfernt die Person nach dem Zurückspielen wieder. Solange die alte Sicherung selbst noch liegt, bleibt die Zuordnung darin möglich. Darum gilt Abschnitt 4a: höchstens 30 Tage für jede Sicherung.
3. **Datenträger.** Das atomare Ersetzen entfernt den alten Inhalt aus der Datei, nicht unbedingt vom Datenträger. Alte Blöcke, Schnappschüsse des Dateisystems (etwa APFS, Time Machine, btrfs, snapper, Schnappschüsse vor einem Löschen oder einer Aktualisierung) und Auslagerung halten ihn bis zu ihrer eigenen Frist. Diese Frist steuert nicht die Stelle; sie gehört in dieselbe Grenze von 30 Tagen wie jede Sicherung (Abschnitt 4a). Der Ordner gehört auf einen verschlüsselten Datenträger; ein Löschen auf Blockebene leistet dieser Vertrag nicht.
4. **Wer Schlüssel und Speicher hält,** ordnet bis zum Löschen alles zu. Das ist gewollt (Einsicht mit Rolle). Ein Rateversuch an einem Freitext gelingt diesem Halter vor dem Löschen; nach dem Löschen nicht mehr, weil das Geheimnis fehlt.
5. **Schlüsselverlust** macht alle alten Pseudonyme dauerhaft unzuordenbar und die Auskunft über alte Zeilen unmöglich (Abschnitt 9). Der Schlüssel muss darum in die Sicherung, mit derselben Frist von höchstens 30 Tagen (Abschnitt 4a). Gegen Verlust und gegen Diebstahl zugleich hilft nur eine getrennte, geschützte Sicherung.
6. **Kopien außerhalb.** Ein Export der Einsicht, ein Ausdruck oder ein Fehlerbericht, der vor dem Löschen entstand, enthält Kennungen und Texte im Klartext. Das Löschen erreicht ihn nicht.
7. **Werte ohne Pseudonymform.** Die Auskunft findet nur, was als Pseudonym oder Freitext-Hash in einer Zeile steht. Schreibt eine Zeilenart eine Person anders (etwa eine uid als Zahl oder einen Klartext), findet die Auskunft sie nicht, und das Löschen erreicht sie nicht. Jede neue Zeilenart muss darum ihre Personen- und Freitextfelder hier anmelden.
8. **Rechtliche Wertung.** Ob dieses Löschen rechtlich als Löschung gilt, beurteilt der Vertrag nicht.

## 12. Alternativen und Empfehlung

**A. Wie entsteht das Pseudonym?**

| Weg | Löschen wirkt gegen den Schlüsselhalter | Diebstahl nur der Zuordnung | Bemerkung |
|---|---|---|---|
| A1: HMAC(Schlüssel, Kennung) | nein; er rechnet das Pseudonym aus der Kennung neu | ordnet ohne Schlüssel nichts zu | einfach, aber Löschen je Person ist so nicht möglich |
| A2: Zufall je Person, Zuordnung Kennung → Pseudonym (P1 heute) | ja | ordnet alles zu | kein Schlüssel nötig; „anderer Schlüssel“ gibt es nicht |
| **A3: HMAC(Schlüssel, Geheimnis der Person)** | ja | ordnet ohne Schlüssel nichts zu | **Empfehlung.** Verbindet die Stärken von A1 und A2; Wechsel des Schlüssels möglich |

Empfehlung A3. A1 erfüllt A12 nicht. A2 wäre ebenfalls tragbar und ist heute gebaut; A3 kostet nur einen HMAC mehr und trennt Schlüssel und Zuordnung. Die erste Auslieferung startet ohne Altbestand, also ist kein Übergang von P1 nötig.

**B. Wie lange gilt ein Pseudonym?**

| Weg | Folge |
|---|---|
| **B1: je Person und Schlüssel** | **Empfehlung.** Vier Augen (Vertrag 6, Abschnitt 10) und Filter der Einsicht brauchen ein festes Pseudonym je Person. Grenze 1 gilt voll |
| B2: je Person und Monat (Monat als weiteres Eingabestück des HMAC) | Grenze 1 gilt nur innerhalb eines Monats. Sitzungen über den Monatswechsel tragen zwei Pseudonyme; Auskunft und Vier Augen rechnen je Monat |
| B3: je Zeile | keine Verbindung zwischen Zeilen nach dem Löschen. Vier Augen und Filter gehen nicht mehr über einen Vergleich; die Auskunft muss je Zeile rechnen |

Empfehlung B1 für den Durchstich und die erste Auslieferung. Entschieden (Projektentscheid, 20.11.2026): B1. B2 bleibt ein späterer, kleiner Schritt, falls Grenze 1 im Betrieb beim Betreiber nicht reicht.

**C. Kette über Zusagen statt über Inhalte.** Jede Zeile trüge nur einen Hash eines Datensatzes außerhalb der Kette; Löschen entfernte den Datensatz.
Das würde auch Grenze 1 beheben, verlangt aber einen zweiten Speicher für jede Zeile, ändert Vertrag 5 und macht jede Prüfung von zwei Ablagen abhängig. Nicht empfohlen.

**D. Löschmarke in der Zeile des Löschens.**
Mit Marke (**Empfehlung**) wirkt das Löschen auch nach einem Absturz und nach dem Zurückspielen einer Sicherung (Abschnitt 6). Ohne Marke wäre die Zeile noch ärmer, aber eine zurückgespielte Sicherung brächte gelöschte Personen still zurück.
Die Marke verrät dem Schlüsselhalter nur, dass zu dieser Zeit eine Person gelöscht wurde, nicht welche.

## 13. Sichtbare Fälle

`faelle/` entsteht mit `node rust/spec/personenbezug/bau-faelle.mjs` (Mac oder Linux-Baurechner, leicht, ohne Abhängigkeit). Das Skript nimmt die Bytes aus `the previous implementation` und `node:crypto`.
Ein eigenes Referenzmodell im Skript (Speicher im Arbeitsspeicher, Kette, Prüfer der Fassung 2 nach Vertrag 5) rechnet jeden Schritt nach. Die Absicht jedes Falls steht als Behauptung im Skript; weicht das Modell davon ab, bricht das Skript ab. Bei gleichem Stand ist die Ausgabe Byte für Byte gleich.

| Datei | Inhalt |
|---|---|
| `pseudonym.json` | Vektoren Schlüssel, Geheimnis → Pseudonym und Löschmarke; gleiche Person gleiches Pseudonym, verschiedene Personen verschiedene, anderer Schlüssel andere; Formfehler |
| `freitext.json` | Vektoren Schlüssel, Geheimnis, Text → `ft_…`; Grenze 4096 Skalarwerte (auch mit Zeichen außerhalb der BMP); leerer und zu langer Text |
| `zeilen.json` | `zeile_geloescht` und `zeile_auskunft`: Bytes oder erster Fehlercode |
| `speicher-oeffnen.json` | Ordner mit Anfangsinhalt (Inhalt, Modus, Verweis, `.git` im Ordner oder darüber) → angelegte Dateien oder Fehlercode |
| `speicher-person-und-text.json` | Text ohne Person, bekannte Person behält ihr Geheimnis, Geheimnis doppelt, Kennung in Pseudonymform, gleicher Text einmal mit letzter Zeit |
| `ablauf-auskunft-und-loeschen.json` | Auskunft findet alle Zeilen der Person (als `person`, als `entscheidend`, über ihren Freitext) ohne Inhalte anderer; Löschen; danach Auskunft leer, Einsicht ohne Text und Kennung, Pseudonym und Freitext-Hash nicht zuzuordnen, die andere Person unverändert; `kette pruefe` grün |
| `loeschen-unbekannt.json` | unbekannte Person: Zeile `unbekannt`, Speicher unverändert; Kennung außer Form ohne Zeile |
| `loeschen-zweimal.json` | zweites Löschen gleicht dem einer unbekannten Person; neu angelegte Person mit derselben Kennung erbt nichts |
| `loeschen-selbst.json` | Selbstlöschung: `handelnd` ist `null` |
| `schluessel-wechsel.json` | Wechsel abgelehnt (alter Schlüssel, Zeit steigt nicht); neue Pseudonyme; Auskunft über beide Schlüssel; Löschen trennt beide |
| `abgleich-nach-wiederherstellung.json` | alte Sicherung ordnet wieder zu; Abgleich entfernt die Person genau einmal |

**Ablauffälle** haben `schritte`; jeder Schritt trägt seine Eingabe und `erwartet`. Der Lauf beginnt mit einem leeren Ordner und einer leeren Kettendatei `kette` (`stelle-2026-11.jsonl`):

| Schritt | Eingabe | erwartet |
|---|---|---|
| `oeffnen` | `zufall` (Schlüssel), `jetzt_ms` | `{"ok":true}` |
| `person` | `kennung`, `zufall`, `jetzt_ms` | `{"neu","pseudonym"}` oder `{"fehler"}` |
| `text` | `kennung`, `text`, `jetzt_ms` | `{"freitext"}` oder `{"fehler"}` |
| `zeile` | `felder` (eine Zeile ohne `vorher`) | `{"zeile","sha256"}`: der Lauf setzt `vorher` wie `kette` und hängt die kanonische Zeile an |
| `auskunft` | `kennung`, `handelnd`, `jetzt_ms` | `{"antwort","zeile"}`; die Zeile `person.auskunft` ist angehängt |
| `loeschen` | `kennung`, `handelnd`, `jetzt_ms` | `{"ergebnis","zeile"}` oder `{"fehler"}` |
| `einsicht` | — | `{"zeilen":[{"datei","nr","zeile"}]}` |
| `zuordnen` | `wert` | `{"kennung": <Kennung oder null>}` |
| `wechseln` | `wert`, `jetzt_ms` | `{"ok":true}` oder `{"fehler"}` |
| `abgleich` | — | `{"entfernt": n}` |
| `speicher` | — | `{"schluessel","zuordnung"}`: die Bytes beider Dateien |
| `speicher_setzen` | `zuordnung` (Bytes) | `{"ok":true}`: ersetzt `zuordnung.json`, wie beim Zurückspielen einer Sicherung |
| `pruefe` | — | `{"exit","stdout"}` von `kette pruefe <kette>` nach Vertrag 5 |

Die Zeilenart `fall.freitext` (Felder `person`, `text`) gibt es nur in den Fällen, als Träger eines Freitexts. Die Zeilen `mcp.schreibaufruf` folgen Vertrag 6, Abschnitt 7.
Der Prüfer im Skript deckt nur die Schritte von Vertrag 5, die diese Fälle berühren. Am 20.11. ergab eine unabhängige Nachrechnung mit `/usr/bin/python3 -I` (sha256-Kette, Kanonisierung über `json.dumps` mit sortierten Schlüsseln) für alle sechs Ketten dieselbe Erwartung.

## 14. Nicht in diesem Vertrag oder nicht im Durchstich

- Bau von Bibliothek und Speicher; verdeckte Fälle und Mutationslauf.
- Die Vorgänge der Stelle (Namen, Rechte, Nachweis) für Auskunft, Löschen, Einsicht und Pseudonymabfrage durch Dienste. Dieser Vertrag liefert die Bausteine dafür.
- Pseudonyme für Werte, die keine Person sind (P1: `unbekannt:` mit HMAC, ohne Eintrag). Im Durchstich nennt eine Zeile nur Personen aus dem Speicher.
- Pflege nach Aufbewahrung (Monatsdateien, Texte, alte Schlüssel, ausgetretene Personen); den Wert `protokoll.aufbewahrung_tage` liefert der Vertrag Regelebenen.
- Notwechsel nach Schlüsselverlust und die Kennzeichnung der Lücke in der Auskunft.
- Geschlossene Form von `person.geloescht` und `person.auskunft` in `kette pruefe` (Vertrag 5 prüft heute nur `ausgabe.gesperrt`).
- Rechte des Ordners unter Windows (ACL statt Modus); die Sperre über Rechner hinweg.
- Personenbezug in Inhalten, die keine Zeilen sind (Vault-Notizen, Sitzungsprotokolle, Tickets). Dafür ist jeder Baustein für seine Daten selbst zuständig (Baustein Formate).

<!-- vom Bauvorgang angefuegt -->
> Hinweis: Dieser Text nennt Werkzeuge, die in diesem Klon nicht liegen: `brain`.
> Sie gehoeren zum Setup der Maschine, aus der dieser Baum stammt; hier fehlen
> sie mit Absicht. Wer sie braucht, schreibt sich seine eigenen.
> *Named here, not shipped here.*
