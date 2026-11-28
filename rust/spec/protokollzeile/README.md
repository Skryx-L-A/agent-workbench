# Vertrag 5: Protokollzeile „Ausgabe gesperrt“ und Prüfbefehl der Kette

Stand 20.11.2026. Teil des Durchstichs (Meilensteinplanung, Abschnitt 5.4, Punkt 5) und Baustein Formate.
Der Abschnitt 5 (Monate, Anker, Wechsel, allgemeines Anhängen, Prüfen eines Ordners) gehört seit dem 20.11.2026 dazu.
Anforderungen: A102 (Hashkette, eine geänderte Zeile macht die Prüfung rot), A104 (kein Pfad, Titel oder Name in der Zeile), A12 (Personenbezug nur als Pseudonym).
Der Vertrag legt Bytes und Rückgaben fest, bevor Code entsteht. Die sichtbaren Fälle stehen in `faelle/`; verdeckte Fälle und der Mutationslauf folgen mit dem Bau.

Grundlage ist das Kettenformat in `the previous implementation` (`x-kettenformat`) und `rust/spec/VERTRAEGE.md`.
Dieser Vertrag ändert daran nichts. Er schreibt nur Fassung 2 und legt eine Zeilenart fest.

## 1. Die Zeile

Eine Zeile entsteht, wenn die Wissenssuche für die Route einer Sitzung Treffer sperrt.
Eine Zeile steht je Antwort und je Gruppe aus Klasse, Kunde und Grund; `anzahl` zählt die gesperrten Einheiten dieser Gruppe.

```
{"aktionsname":"ausgabe.gesperrt","anzahl":1,"ergebnis":"gesperrt","fassung":2,"grund":"kundendaten-an-cloud","klasse":"kundendaten","kunde":"k-beispiel","route":"cloud-beispiel","sitzung":"s-beispiel","vorher":"<64 hex>","zeit":"2026-11-20T13:00:00.000Z"}
```

| Feld | Wert |
|---|---|
| `aktionsname` | genau `ausgabe.gesperrt` |
| `anzahl` | Ganzzahl von 1 bis 4294967295 |
| `ergebnis` | genau `gesperrt` |
| `fassung` | genau `2` |
| `grund` | Code, mit dem `darf` gesperrt hat (Vertrag 3, etwa `kundendaten-an-cloud`) |
| `klasse` | wirksame Klasse der gesperrten Einheiten, mit der `darf` entschieden hat; `null` beim Grund `einheit-ohne-klasse` |
| `kunde` | Schlüssel des Kunden oder `null`, wenn die Einheiten keinem Kunden gehören |
| `route` | Schlüssel der Route der Sitzung |
| `sitzung` | Kennung der Sitzung aus dem Start |
| `vorher` | sha256 hex klein über die Rohbytes der vorigen Zeile ohne Zeilenende; die erste Zeile einer Kette trägt 64 Nullen |
| `zeit` | UTC mit Millisekunden und `Z`, Muster `^[0-9]{4}-(0[1-9]|1[0-2])-(0[1-9]|[12][0-9]|3[01])T([01][0-9]|2[0-3]):[0-5][0-9]:[0-5][0-9]\.[0-9]{3}Z$` |

- **Geschlossene Form.** Genau diese elf Felder, kein weiteres. Die Zeile nennt keinen Pfad, keinen Titel, keinen Anfragetext, keine Meldung von `darf` und keinen Personennamen (A104, A12).
- **Schlüssel.** `grund`, `klasse`, `kunde`, `route` und `sitzung` sind Schlüssel oder Codes, nie Anzeigenamen. Jeder folgt `^[a-z0-9][a-z0-9._-]{0,63}$`; nur `klasse` und `kunde` dürfen `null` sein.
  `klasse`, `kunde` und `route` kommen aus der Einstellungsdatei (Vertrag 2), `sitzung` aus der Start-Struktur (Vertrag 1), `grund` aus der Antwort von `darf` (Vertrag 3).
- **Bytes.** RFC 8785 (Fassung 2), UTF-8, eine Zeile je Glied, Zeilenende LF; das LF gehört nicht zur Zeile. Weil alle Werte dem Schlüsselmuster folgen oder Zahlen sind, braucht die Zeile kein Escape.
- **Datei.** `<quelle>-<JJJJ-MM>.jsonl`, für die Suche `brain-<JJJJ-MM>.jsonl`; den Monat bestimmt `zeit`. Je Quelle gibt es genau eine Kette.
  Mehrere Prozesse derselben Quelle (ein Such-Prozess je Sitzung) schreiben in dieselbe Datei; die Bibliothek ordnet sie mit einer exklusiven Sperre (Abschnitt 2).
- **Zeit.** Der Aufrufer übergibt die Zeit in Millisekunden seit 1970. Weder Schreiber noch Prüfer lesen dafür die Uhr; kein Fall hängt vom Ausführungstag ab.

## 2. Bibliothek (Crate `formate`, Modul `kette`)

| Funktion | Ergebnis | Fehler |
|---|---|---|
| `kanonisch(&Value) -> Result<String>` | Bytes nach RFC 8785 (Schlüssel nach UTF-16-Codeeinheiten, Zahlen wie ECMAScript `Number.prototype.toString`) | `nicht_endlich` für eine Zahl, die kein endlicher Double ist |
| `zeit_text(ms: i64) -> Result<String>` | `JJJJ-MM-TTThh:mm:ss.mmmZ` | `zeit_ungueltig` außerhalb von 1970-01-01 bis 9999-12-31 |
| `abschnitt(quelle, ms) -> Result<String>` | `<quelle>-<JJJJ-MM>.jsonl` | `feld_ungueltig:quelle`, wenn `quelle` nicht `gateway`, `stelle`, `brain`, `halter` oder `ordnermarke` ist; `zeit_ungueltig` |
| `zeile_gesperrt(&Gesperrt, vorher) -> Result<String>` | die Zeile ohne LF | `feld_ungueltig:<feld>` für den ersten falschen Wert in der Reihenfolge `route`, `klasse`, `kunde`, `sitzung`, `grund`, `anzahl`, `vorher`; danach `zeit_ungueltig` |
| `anhaengen_gesperrt(datei, &Gesperrt) -> Result<String>` | hängt Zeile und LF an und gibt den sha256 der Zeile zurück | wie `zeile_gesperrt`; `letzte_zeile_unvollstaendig`; `ein_ausgabe` |

`Gesperrt` hat die Felder `zeit_ms: i64`, `route`, `sitzung`, `grund` (je `String`), `klasse` und `kunde` (je `Option<String>`) und `anzahl: u32`.

`anhaengen_gesperrt` prüft zuerst die Felder. Dann öffnet sie die Datei zum Lesen und Anhängen (eine neue Datei entsteht unter Unix mit den Rechten 0600)
und nimmt eine exklusive Sperre auf genau diesen Handle (`std::fs::File::lock`, seit Rust 1.89: `flock` unter Unix, `LockFileEx` unter Windows; ohne `unsafe` und ohne neue Abhängigkeit).
Erst unter der Sperre liest sie die letzte Zeile. Ist die Datei leer, ist `vorher` die Kette aus 64 Nullen; sonst der sha256 der Rohbytes der letzten Zeile.
Endet eine nicht leere Datei nicht auf LF, schreibt die Funktion nichts und meldet `letzte_zeile_unvollstaendig` (fail closed).
Zeile und LF gehen in einem Schreibaufruf an die Datei, danach `sync_data`; dann gibt sie die Sperre frei.
Bei jedem Fehler bleibt die Datei Byte für Byte unverändert, und der Fehler geht an den Aufrufer (die Suche antwortet dann ohne Treffer).
Scheitert der Schreibaufruf selbst, setzt die Funktion die Datei auf die Länge davor zurück (so weit das Betriebssystem es zulässt). Die Länge der letzten Zeile begrenzt der Schreiber nicht; das tut der Prüfer.
`abschnitt` prüft erst `quelle`, dann die Zeit, wie `zeile_gesperrt` erst die Felder.

Ein Fehler ist ein Wert mit einem festen Code (die Texte oben). Die Fälle prüfen den Code, keinen Satz.

## 3. Prüfbefehl

```
kette pruefe [--vorher <64 hex>] [--letzte <64 hex>] <datei>...
```

Der Befehl prüft die Dateien in der angegebenen Reihenfolge als eine Kette. `--vorher` ist der erwartete Wert `vorher` der ersten Zeile (Vorgabe: 64 Nullen).
`--letzte` ist der erwartete sha256 der letzten Zeile, etwa aus einem Anker; ohne die Option bleibt eine geänderte LETZTE Zeile unentdeckt, weil ihr keine Zeile mehr folgt.
Der Befehl ändert keine Datei und liest keine Uhr.

**Ausgabe.** Genau eine Zeile auf stdout in RFC 8785, dann LF:

```
{"bruch":null,"letzte":"<64 hex>","ok":true,"zeilen":3}
{"bruch":{"datei":"brain-2026-11.jsonl","grund":"kette_gebrochen","zeile":3},"letzte":null,"ok":false,"zeilen":2}
```

- `zeilen` zählt die Zeilen, die vor dem Bruch geprüft und gut waren, über alle Dateien.
- `letzte` ist bei `ok` der sha256 der letzten Zeile, bei null Zeilen `null`; bei einem Bruch `null`.
- `datei` ist nur der Dateiname, ohne Ordner. `zeile` zählt ab 1 innerhalb der Datei; `null` bei `datei_nicht_lesbar` und bei `letzte_falsch` ohne Zeilen.
- `letzte_falsch` nennt Datei und Nummer der letzten guten Zeile; ohne Zeilen die zuletzt genannte Datei und `zeile` `null`.
- Die Prüfung endet beim ersten Bruch.

**Exit.** 0 die Kette ist ganz; 1 ein Bruch; 64 Aufruffehler (kein Unterbefehl, unbekannter Unterbefehl oder Schalter, keine Datei, Wert von `--vorher` oder `--letzte` nicht 64 hex klein, Schalter ohne Wert, ein Schalter doppelt). Bei 64 bleibt stdout leer.
Ein Argument, das mit `--` beginnt, ist ein Schalter; jedes andere ist eine Datei, auch `-x`.

**Prüfschritte je Zeile, in dieser Reihenfolge.** Der erste zutreffende Schritt nennt den Grund.

| Schritt | Grund |
|---|---|
| Datei fehlt oder ist nicht lesbar | `datei_nicht_lesbar` |
| die letzte Zeile einer nicht leeren Datei endet nicht auf LF | `letzte_zeile_unvollstaendig` |
| Zeile leer, länger als 4194304 Bytes, kein UTF-8 (ein BOM zählt als Zeichen), kein JSON-Objekt, oder sie enthält ein einsames Surrogat | `zeile_unlesbar` |
| `fassung` ist da, aber nicht die Zahl 2 (`2.0` besteht diesen Schritt und scheitert an `nicht_kanonisch`) | `form_falsch` |
| `fassung` fehlt, und der Dateiname gehört nicht zu `gateway-<JJJJ-MM>.jsonl` (Fassung 1g) oder `audit.jsonl`, `audit.<JJJJ-MM>[.<N>].jsonl` (Fassung 1s); `MM` ist 01 bis 12, `N` mindestens eine Ziffer, wie die Muster in `x-kettenformat` | `fassung_fehlt` |
| die Zeile trägt `kettenbruch` (Text) | `angekuendigt:<Text>`; ist `kettenbruch` kein Text, `form_falsch` |
| `vorher` fehlt | `vorher_fehlt` |
| `vorher` ist nicht der sha256 der vorigen Zeile (bei der ersten Zeile: der Wert von `--vorher`) | `kette_gebrochen` |
| Fassung 2: die Bytes sind nicht `kanonisch` des gelesenen Werts | `nicht_kanonisch` |
| Fassung 2: `zeit` folgt nicht dem Muster, oder `aktionsname` oder `ergebnis` ist kein Text | `form_falsch` |
| `aktionsname` ist `ausgabe.gesperrt`: Fassung nicht 2, oder die Zeile weicht von der geschlossenen Form in Abschnitt 1 ab | `form_falsch` |
| `aktionsname` ist eine Zeilenart aus Abschnitt 6: Fassung nicht 2, oder die Zeile weicht von deren geschlossener Form ab | `form_falsch` |
| Fassung 2: `aktionsname` steht nicht im Register (unten) | `form_falsch` |
| nach der letzten Zeile: `--letzte` ist gesetzt und nicht der sha256 der letzten Zeile | `letzte_falsch` |

Zeilen der Fassungen 1g und 1s prüft der Befehl nur als Kettenglieder (Lesbarkeit, `kettenbruch`, `vorher`); er kanonisiert sie nicht neu.
**Register der Zeilenarten der Fassung 2.** Dieser Vertrag ist das Register (Entscheid des Betreibers, 23.11.2026). Eine Zeile der Fassung 2, deren `aktionsname` hier nicht steht, ist `form_falsch`; der
Prüfer ist für die Fassung 2 fail closed. So scheitert auch ein Tippfehler in einem bekannten Namen,
etwa `mcp.schreibaufrufe` mit einem Feld `vorschau`. Für die Fassungen 1g und 1s gilt kein Register:
Sie bleiben Kettenglieder, wie der Absatz darüber sagt.
**Eine neue Zeilenart steht zuerst in diesem Vertrag und erst danach im Code.** Wer eine Art schreibt,
die hier fehlt, bekommt `form_falsch`.

| Gruppe | Arten |
|---|---|
| geschlossene Form, hier geprüft | `ausgabe.gesperrt` (Abschnitt 1), `kanal.abgewiesen` (6.1), `probelauf.zaehlung` (6.2), `person.geloescht` und `person.auskunft` (6.3), `ordnermarke.geloest` und `ordnermarke.gesenkt` (6.4), `modellweg.aufruf` (6.7), `mcp.schreibaufruf`, `sitzung.gestartet`, `sitzung.abgelehnt` und `vorgang.abgelehnt` (6.8) |
| angemeldet, Form noch nicht geprüft | `suche.rueckfall` (6.5), `regel.geaendert` und `regel.abgelehnt` (6.6) |
| in anderen Verträgen festgelegt, Form hier nicht geprüft | `hersteller.zugang`, `hersteller.zugriff`, `pflicht.aenderung`, `protokoll.einsicht`, `vorgang.gedrosselt` (Vertrag 6 Fassung 2), `modell.aufruf` und `sitzung.abgemeldet` (Vertrag 7, Gateway), `fall.freitext` (Vertrag 9, Personenbezug), `mcp.schein_abgelehnt` und `halter.leck_gesperrt` (Vertrag Halter; `halter.leck_gesperrt` Nachtrag 25.11.2026, Treffer der Leck-Sperre; `halter.pfad_verboten` Nachtrag 25.11.2026, abgelehnter Pfad außerhalb der Regeln), `wissen.geschrieben` und `wissen.schreiben.abgelehnt` (Vertrag Schreibtor), `klasse.erlaubt` und `lauf.melden` (Audit-Arten der Hülle im Übergang 1s nach 2, `the previous implementation`) |

Die zweite Gruppe bekommt ihre Arme mit dem Bau ihrer Schreiber; bis dahin prüft der Befehl ihre
Zeilen nur als Kettenglieder. Die dritte Gruppe prüft dieser Vertrag nicht auf ihre Form: Ihre Form
steht im genannten Vertrag, und dieser Prüfer weiß nur, dass die Art existiert.
Die Audit-Arten der Hülle sind in der Fassung 1s eine offene Menge aus Vorgang und Ergebnis
(`the previous implementation`). Für die Fassung 2 stehen hier nur die zwei Arten, die der
Übergangsvektor wirklich schreibt; die übrigen kommen mit dem Umstieg der Audit-Kette auf die
Fassung 2 hinzu (Abschnitt 7).

Doppelte Schlüssel in einer Zeile der Fassung 2 ergeben `nicht_kanonisch`, weil `kanonisch` den Schlüssel nur einmal schreibt.
JSON-Zahlen werden als Double gelesen, mit derselben Rundung wie JavaScript (in Rust: `serde_json` mit `float_roundtrip`).

## 4. Sichtbare Fälle

`faelle/` entsteht mit `node rust/spec/protokollzeile/bau-faelle.mjs` (Mac oder Linux-Baurechner, leicht, ohne Abhängigkeit). Das Skript nimmt die Bytes aus `the previous implementation` und `node:crypto`
und die vorhandenen Vektoren aus `the previous implementation`. Für jeden Prüffall rechnet ein kleiner eigener Prüfer im Skript das Ergebnis nach; weicht es von der Absicht im Fall ab, bricht das Skript ab.
Bei gleichem Stand ist die Ausgabe Byte für Byte gleich.

| Datei | Inhalt |
|---|---|
| `faelle/kanonisch.json` | Eingabetext und erwartete Bytes; aus `jcs-vektoren.json` (Feld `rfc8785`) und `fassung-vektoren.json` (Feld `fassung2`) |
| `faelle/zeile.json` | Felder, `vorher` und erwartete Zeile samt sha256, oder der erwartete Fehlercode; dazu `zeit_text` und `abschnitt` |
| `faelle/anhaengen.json` | Folgen von Aufrufen von `anhaengen_gesperrt` auf einen Ordner mit Anfangsinhalt; erwartet je Aufruf Rückgabe oder Fehlercode und am Ende der Inhalt der Datei. Dazu der Fall `zwei_schreiber`: zwei Schreiber mit je eigenem Handle (zwei Prozesse oder zwei Threads mit eigenem `open`) hängen gleichzeitig je 50 Zeilen an; danach ist `kette pruefe` grün mit `zeilen` 100 |
| `faelle/pruefe.json` | Dateien (Name und Inhalt als Text oder `inhalt_b64`; `fehlt` für eine fehlende Datei), der Aufruf und erwartet: Exit und stdout. Im Aufruf steht `@<name>` für den Pfad der Datei in einem eigenen Wegwerfordner |

Die Prüffälle decken ab: eine Kette aus drei gesperrt-Zeilen (grün); eine von Hand geänderte Klasse in Zeile 2 (rot in Zeile 3, `kette_gebrochen`); ein eingefügtes Leerzeichen
(`nicht_kanonisch`); eine gelöschte Zeile; ein zusätzliches Feld `titel` (`form_falsch`); die geänderte letzte Zeile ohne und mit `--letzte`; die fehlende Zeilenende-Markierung;
eine Zeile ohne `fassung` in `brain-…`; die Kette `k1` aus `kette-vektoren.jsonl` (Fassung 1g, grün bis zum angekündigten Bruch); die gemischten Ketten 1g → 2 und 1s → 2;
doppelte Schlüssel; kein UTF-8; Aufruffehler.

## 5. Monate, Anker und Wechsel

Stand 20.11.2026. Dieser Abschnitt erweitert Vertrag 5. Er ändert Abschnitt 1 bis 4 nicht; `anhaengen_gesperrt(datei, …)`, `kette pruefe` und die Fälle dazu bleiben wie sie sind.
Grundlage ist `x-kettenformat` (`kette.abschnitt.2` und `kette.anker.2`, Fassung 2) in `the previous implementation`.

### 5.1 Dateien einer Quelle

Ein Ordner enthält je Quelle die Monatsdateien `<quelle>-<JJJJ-MM>.jsonl` (`MM` 01 bis 12) und die Ankerdatei `<quelle>.anker.jsonl`. Jede andere Datei gehört nicht zur Quelle und zählt nicht
(`brain-2026-11.jsonl.bak`, `stelle-2026-11.jsonl` bei der Quelle `brain`, `brain.jsonl`). Je Quelle gibt es genau eine Kette; die Monatsdateien in aufsteigender Namensfolge sind ihre Glieder.
Ein Monat ohne Zeile hat weder Datei noch Anker (stiller Monat); die Kette läuft über ihn mit `vorher` weiter.

### 5.2 Die Ankerzeile

Beim Wechsel schreibt der Schreiber eine Zeile an `<quelle>.anker.jsonl`: RFC 8785, ein LF, genau diese sieben Felder.

```
{"abschnitt":"brain-2026-11.jsonl","bis":1795180801,"fassung":2,"letzte":"<64 hex>","monat":"2026-11","von":1795179600,"vorher_erste":"<64 hex>"}
```

| Feld | Wert |
|---|---|
| `abschnitt` | Dateiname des geschlossenen Monats, ohne Ordner |
| `monat` | `JJJJ-MM` aus dem Dateinamen |
| `von`, `bis` | `zeit` der ersten und der letzten Zeile in Sekunden seit 1970: die Millisekunden des Textes (Muster aus Abschnitt 1, gültiges Datum ab 1970-01-01) durch 1000 als Double, geschrieben wie jede Zahl nach RFC 8785. `null`, wenn `zeit` kein Text dieses Musters, kein gültiges Datum oder vor 1970 ist oder die Zeile unlesbar ist (länger als 4194304 Bytes, kein JSON-Objekt) |
| `vorher_erste` | das Feld `vorher` der ersten Zeile; `null`, wenn es kein Text ist oder die Zeile unlesbar ist |
| `letzte` | sha256 hex klein über die Rohbytes der letzten Zeile ohne LF |
| `fassung` | genau `2` |

Die Ankerdatei hat die Rechte 0600 (Unix, neue Datei) und wird nur angehängt. Der Prüfer liest auch Ankerzeilen ohne `fassung` (Form 1g des alten Gateways in `gateway.anker.jsonl`, Abschnitt 5.4).

### 5.3 Schreiben über Monate

| Funktion | Ergebnis |
|---|---|
| `anhaengen_gesperrt_in(ordner, quelle, &Gesperrt) -> Result<String>` | wie `anhaengen_gesperrt`, die Datei folgt aus `zeit_ms` und `quelle` (`abschnitt`); gibt den sha256 der Zeile zurück |
| `anhaengen_zeile(ordner, quelle, zeit_ms, &Value) -> Result<String>` | hängt ein geschlossenes JSON-Objekt einer Zeilenart an: der Schreiber setzt `fassung` (2), `vorher` und `zeit` (aus `zeit_ms`), schreibt kanonisch; gibt den sha256 der Zeile zurück. `anhaengen_gesperrt_in` baut das Objekt aus `Gesperrt` und ruft dieselbe Schreibstrecke |
| `anhaengen_zeilen(ordner, quelle, vorher_erwartet, &[String]) -> Result<String>` | hängt schon fertig verkettete Zeilen (je eine Zeile ohne LF, Fassung 2) in einem Schreibaufruf unter einer Sperre an; `vorher_erwartet` ist der sha256, den der Aufrufer als letzte Zeile der Kette erwartet; gibt den sha256 der letzten angehängten Zeile zurück |

**Prüfung vor jedem Zugriff auf Dateien** (der erste Fehler zählt, in dieser Reihenfolge; danach ist nichts geschrieben und nichts angelegt):

- `anhaengen_gesperrt_in`: die Felder von `Gesperrt` wie bei `zeile_gesperrt` (`route`, `klasse`, `kunde`, `sitzung`, `grund`, `anzahl`), dann `feld_ungueltig:quelle`, dann `zeit_ungueltig`.
- `anhaengen_zeile`: `feld_ungueltig:zeile`, wenn der Wert kein Objekt ist; `feld_ungueltig:aktionsname` und `feld_ungueltig:ergebnis`, wenn das Feld kein Text ist (Fassung 2 verlangt beide); `feld_ungueltig:fassung`, `…:vorher`, `…:zeit`, wenn das Objekt das Feld schon trägt (in dieser Reihenfolge); dann `feld_ungueltig:quelle`, dann `zeit_ungueltig`;
  danach baut die Funktion die Zeile mit 64 Nullen als `vorher`: ist `aktionsname` `ausgabe.gesperrt` (Abschnitt 1), `kanal.abgewiesen`, `ordnermarke.geloest` oder `ordnermarke.gesenkt` (Abschnitt 6) und weicht die Zeile von der geschlossenen Form ab, oder ist sie länger als 4194304 Bytes, gilt `feld_ungueltig:zeile`.
- `anhaengen_zeilen`: `feld_ungueltig:zeilen`, wenn die Liste leer ist; `feld_ungueltig:vorher`, wenn `vorher_erwartet` nicht 64 hex klein ist; `feld_ungueltig:quelle`; dann jede Zeile in der Reihenfolge der Liste nach den Prüfschritten aus Abschnitt 3 (so, als stünde sie in `brain-<Monat>.jsonl`; `vorher` der ersten Zeile ist `vorher_erwartet`, `vorher` jeder weiteren der sha256 der vorigen):
  der erste Fehler heißt `zeile_ungueltig:<Grund aus Abschnitt 3>`, etwa `zeile_ungueltig:kette_gebrochen`, `…:nicht_kanonisch`, `…:form_falsch`, `…:fassung_fehlt`, `…:zeile_unlesbar`. Eine Zeile ohne `fassung` ist so immer `fassung_fehlt`.
  Dann den Monat jeder Zeile aus ihrem `zeit` (Millisekunden außerhalb von 1970-01-01 bis 9999-12-31: `zeit_ungueltig`); liegen die Zeilen nicht alle im selben Monat: `feld_ungueltig:zeit`.

**Dann** öffnet der Schreiber `<quelle>.anker.jsonl` zum Lesen und Anhängen (fehlt sie, entsteht sie leer mit 0600) und nimmt eine exklusive Sperre auf diesen Handle (`std::fs::File::lock`). Diese Datei ist die Sperre der Quelle:
Monatsdateien und Anker werden nur unter ihr geschrieben, also deckt sie auch den Wechsel. Ein Fehler lässt die Monatsdateien und den Inhalt der Ankerdatei unverändert; eine leere Ankerdatei kann neu entstehen. Wer `anhaengen_gesperrt(datei, …)` auf eine Monatsdatei richtet, sperrt nur diese Datei und nicht die Quelle; beide Wege nie mischen.
Unter der Sperre, in dieser Reihenfolge; der erste Fehler zählt, `M` ist der Monatsname der Zeit:

1. Die Ankerdatei lesen. Endet sie nicht auf LF: `letzte_zeile_unvollstaendig`. Ist eine Zeile kein JSON-Objekt mit Text `abschnitt`: `anker_falsch`.
2. Die Monatsdateien der Quelle auflisten.
3. **Zeit rückwärts.** Gibt es eine Monatsdatei mit größerem Namen als `M`, oder einen Anker mit `abschnitt` `M`: `zeit_rueckwaerts`. `M` ist dann geschlossen. Innerhalb des laufenden Monats darf die Zeit zurückgehen.
4. `P` ist die größte nicht leere Monatsdatei mit kleinerem Namen als `M` (keine, wenn es keine gibt).
5. **`M` besteht schon und ist nicht leer**: Gibt es `P` und hat `P` keinen Anker, hat jemand die Datei des neuen Monats angelegt, ohne dass der Vormonat geschlossen ist: `abschnitt_vorhanden` (fail closed). Sonst setzt der Schreiber `M` fort; `vorher` ist der sha256 der letzten Zeile von `M` (`letzte_zeile_unvollstaendig`, wenn sie nicht auf LF endet).
6. **`M` fehlt oder ist leer (Wechsel).** Gibt es kein `P`, ist `vorher` 64 Nullen. Sonst: endet `P` nicht auf LF, `letzte_zeile_unvollstaendig`; `vorher` ist der sha256 der letzten Zeile von `P`; gibt es zwei Anker für `P`, oder einen, dessen `letzte` nicht dieser sha256 ist: `anker_falsch`.
7. **Erwartung.** Bei `anhaengen_zeilen` ist `vorher_erwartet` von `vorher` verschieden: `vorher_abweichend`. Die anderen Funktionen haben keine Erwartung.
8. Die Zeile oder Zeilen bauen. Alles Weitere schreibt:
   a. Beim Wechsel mit `P` und ohne Anker für `P`: die Ankerzeile von `P` (Abschnitt 5.2) mit einem Schreibaufruf an die Ankerdatei, `sync_data`. Der Anker entsteht damit genau einmal; steht er schon da (Absturz zwischen Anker und Anlegen), schreibt der Schreiber keinen zweiten.
   b. Fehlt `M`: `M` mit `create_new` und 0600 anlegen. Gibt es sie inzwischen, `abschnitt_vorhanden`; nichts wird geschrieben.
   c. Zeile und LF (bei `anhaengen_zeilen` alle Zeilen, je mit LF) in einem Schreibaufruf an `M`, `sync_data`. Scheitert das, setzt die Funktion `M` auf die Länge davor zurück oder löscht die eben angelegte Datei (so weit das Betriebssystem es zulässt) und meldet `ein_ausgabe`. Der Anker bleibt; er beschreibt den geschlossenen Monat richtig.
9. Sperre freigeben.

Neue Fehlercodes dieses Abschnitts: `zeit_rueckwaerts`, `abschnitt_vorhanden`, `anker_falsch`, `vorher_abweichend`, `zeile_ungueltig:<Grund>`; dazu die bekannten `feld_ungueltig:<feld>` mit den neuen Feldern `zeile`, `aktionsname`, `ergebnis`, `fassung`, `vorher`, `zeit`, `zeilen`.
`ein_ausgabe` bleibt der Code für Öffnen, Sperren, Lesen und Schreiben. Die Suche antwortet bei jedem Fehler ohne Treffer (Abschnitt 2); einzige Ausnahme ist die Zeile `suche.rueckfall`, deren Schreibfehler die Antwort nicht ändert (Abschnitt 6.5).
Zwei Schreiber über eine Monatsgrenze: der erste unter der Sperre wechselt, der zweite findet den Wechsel fertig vor und setzt fort. Es entsteht genau ein Anker.
Eine Zeit hinter der eines schon geschlossenen Monats (ein Schreiber mit rückständiger Uhr) wird mit `zeit_rueckwaerts` abgelehnt, nicht an den laufenden Monat gehängt.

### 5.4 Prüfen über Monate

```
kette pruefe-quelle [--vorher <64 hex>] [--letzte <64 hex>] <ordner> <quelle>
```

Prüft die Monatsdateien der Quelle in aufsteigender Namensfolge als eine Kette und jeden geschlossenen Monat gegen seinen Anker. `--vorher` und `--letzte` bedeuten dasselbe wie bei `kette pruefe`.
Ausgabe, Exit 0 und 1 und `zeilen` wie in Abschnitt 3 (`datei` ist nur ein Dateiname: der Name der Monatsdatei, der Ankerdatei oder, wenn der Ordner nicht lesbar ist, des Ordners).
Exit 64 (stdout leer) bei: anderem Unterbefehl, unbekanntem Schalter, Schalter ohne Wert oder doppelt, Wert nicht 64 hex klein, nicht genau zwei Angaben ohne Schalter (Ordner und Quelle), `quelle` nicht `gateway`, `stelle`, `brain`, `halter` oder `ordnermarke`. Der Befehl ändert nichts und liest keine Uhr.
Als Funktion: `pruefe_quelle(ordner, quelle, vorher, letzte) -> Result<Pruefung, Fehler>` (`feld_ungueltig:quelle`).

**Ein Monat ist geschlossen**, wenn eine spätere Monatsdatei der Quelle besteht (gleich welcher Größe). Eine Monatsdatei ohne Zeile ist kein Glied der Kette und braucht keinen Anker; ein Anker für sie ist `anker_falsch`.

**Schritte, in dieser Reihenfolge; der erste zutreffende nennt den Grund:**

| Schritt | Grund | `datei`, `zeile` |
|---|---|---|
| Der Ordner ist nicht lesbar | `datei_nicht_lesbar` | Ordnername, `null` |
| Die Ankerdatei besteht, ist aber nicht lesbar | `datei_nicht_lesbar` | Ankerdatei, `null` |
| Die letzte Zeile der Ankerdatei endet nicht auf LF | `letzte_zeile_unvollstaendig` | Ankerdatei, Zeilennummer |
| Eine Ankerzeile ist leer, länger als 4194304 Bytes, kein UTF-8, kein JSON-Objekt oder hat ein einsames Surrogat | `zeile_unlesbar` | Ankerdatei, Zeilennummer |
| `abschnitt` einer Ankerzeile ist kein Text oder kein Monatsname dieser Quelle | `anker_falsch` | Ankerdatei, Zeilennummer |
| `abschnitt` kam schon in einer früheren Ankerzeile vor | `anker_doppelt` | Ankerdatei, Zeilennummer der zweiten |
| Die Monatsdatei von `abschnitt` besteht nicht | `anker_ohne_datei` | Ankerdatei, Zeilennummer |
| Jede Monatsdatei in aufsteigender Folge: ihre Zeilen nach Abschnitt 3 (die Kette läuft über die Dateigrenzen; `vorher` der ersten Zeile ist `--vorher`) | die Gründe aus Abschnitt 3 | Monatsdatei, Zeile |
| Gleich nach den Zeilen einer Monatsdatei mit Zeilen, die einen Anker hat: der Anker weicht ab (siehe unten) | `anker_falsch` | Ankerdatei, Zeilennummer |
| Gleich danach, ohne Anker, aber geschlossen | `anker_fehlt` | Monatsdatei, `null` |
| Eine Monatsdatei ohne Zeile hat einen Anker | `anker_falsch` | Ankerdatei, Zeilennummer |
| nach der letzten Zeile: `--letzte` stimmt nicht | `letzte_falsch` | wie Abschnitt 3 |

Der Anker weicht ab, wenn eine dieser Bedingungen gilt: er trägt `fassung` und das ist nicht die Zahl 2; er trägt `fassung` 2 und seine Bytes sind nicht `kanonisch` des gelesenen Werts; er hat andere Felder als die sechs der Tabelle (ohne `fassung`);
ein Wert weicht von dem ab, den die Zeilen der Datei ergeben (`abschnitt`, `monat`, `vorher_erste` = erwarteter `vorher` der ersten Zeile, `letzte` = sha256 der letzten gut geprüften Zeile; `von` und `bis` als Zahl mit gleichem Double oder beide `null`).
Ein Anker ohne `fassung` (Form 1g, `gateway.anker.jsonl` des alten Gateways) wird nur auf Felder und Werte geprüft, nicht auf kanonische Bytes.
Die Monate eines Ordners werden einzeln nach ihrem Anker geprüft, bevor der nächste beginnt. Die Prüfung endet beim ersten Bruch. Hat der Ordner keine Monatsdatei und stimmt `--letzte` nicht, nennt `letzte_falsch` den Namen des Ordners (`zeile` `null`); ihn kennt der Fall nicht vorab, der Bau prüft ihn mit einem Test.
Ein Monat, der fehlt, ohne dass er einen Anker hat, ist ein stiller Monat und kein Bruch. Löscht jemand einen Monat, zeigt es `anker_ohne_datei` (der Anker bleibt) und, ohne Anker, `kette_gebrochen` in der ersten Zeile des nächsten Monats.
Ein Anker beweist nur den geschlossenen Monat. Der laufende Monat hat keinen; eine Änderung seiner letzten Zeile zeigt nur `--letzte`.

### 5.5 Sichtbare Fälle

| Datei | Inhalt |
|---|---|
| `faelle/monate.json` | Aufrufe von `anhaengen_gesperrt_in` (`art` `gesperrt`), `anhaengen_zeile` (`zeile`) und `anhaengen_zeilen` (`zeilen`) auf einen Ordner mit Anfangsinhalt (`anfang`, Name zu Text); erwartet je Aufruf Rückgabe oder Fehlercode, am Ende alle Dateien des Ordners mit Inhalt (`ende`, einschließlich einer leeren Ankerdatei) und das Ergebnis von `kette pruefe-quelle` für die genannten Quellen. Dazu `zwei_schreiber_monatsgrenze` |
| `faelle/pruefe-quelle.json` | Ordnerinhalt (wie `pruefe.json`), Aufruf (`@@` steht für den Ordner, `@<name>` für eine Datei darin) und erwartet Exit und stdout |

Die Fälle decken ab: Wechsel November nach Dezember mit Anker; zwei Wechsel und ein Jahreswechsel; ein stiller Monat; ein leerer Ordner; die neue Monatsdatei besteht schon (mit und ohne Anker des Vormonats); Anker geschrieben, Monat noch nicht angelegt; leere neue Monatsdatei; falscher und doppelter Anker beim Wechsel;
Zeit rückwärts (Vormonat, nie vorhandener Monat, Monat mit Anker ohne Folgedatei) und der erlaubte Rückgang innerhalb des Monats; unvollständige letzte Zeile; zwei Quellen in einem Ordner; Reihenfolge der Feldprüfungen;
das allgemeine Anhängen mit allen Fehlern; mehrere Zeilen mit erwartetem sha256, über zwei Monate, mit falschem sha256 und mit jedem Zeilenfehler;
beim Prüfen: ein Anker geändert (jedes Feld), Monat gelöscht, Zeile im geschlossenen Monat geändert (einfach, dann neu verkettet; auch die letzte Zeile), ein geschlossener Monat ohne Anker, doppelter Anker, Ankerdatei kaputt, fremde Dateinamen, ein Gateway-Ordner mit altem Anker, Aufruffehler.
Der Fall `zwei_schreiber_monatsgrenze` hat 10 Runden mit zwei Schreibern (je eigenem Handle) um den Wechsel November nach Dezember: danach genau ein Anker und `kette pruefe-quelle` grün mit 102 Zeilen.
Die Grenze von 4194304 Bytes im allgemeinen Anhängen steht nicht in den Fällen (eine Zeile dieser Größe machte die Datei sehr groß); der Bau prüft sie mit einem Test.
Das Skript `bau-faelle.mjs` rechnet die Schreibfälle und die Prüffälle mit eigenen Modellen (`schreibeMonat`, `pruefeQuelle`) nach und bricht ab, wenn das Ergebnis von der Absicht im Fall abweicht; zwei Läufe geben dieselben Bytes.

### 5.6 Auslegungen, wo `x-kettenformat` schweigt

- Die Sperre liegt auf der Ankerdatei, nicht auf einer eigenen Sperrdatei: eine Dateiart weniger, und der Anker gehört zur selben Quelle. Preis: die Ankerdatei kann leer entstehen.
- Ein stiller Monat (kein Schreibvorgang) ist kein Bruch. Eine Pflicht „jeder Monat zwischen zwei vorhandenen besteht“ machte jede ruhige Pause zum Dauerfehler und ließe sich nur mit Leerdateien erfüllen. Gelöschte Monate zeigen `anker_ohne_datei` und `kette_gebrochen`.
- Eine Zeit rückwärts innerhalb des laufenden Monats wird nicht abgewiesen (wie bisher).
- Zeilen, die der Prüfer nicht annähme, schreibt keine der Funktionen; die Prüfschritte aus Abschnitt 3 gelten beim Schreiben von `anhaengen_zeilen` und `anhaengen_zeile` unverändert.

### 5.7 Lesen: die letzte Zeile einer Art

Stand 20.11.2026 (Nachtrag 5, Teil D). Wer beim Start wissen will, ob eine gelesene Datei zur letzten Zeile einer Art in der Kette passt (etwa die Regeldatei zur letzten Zeile `regel.geaendert` der Ebene `firma`), liest sie mit:

```
letzte_zeile_der_art(ordner, quelle, vorher, filter: &[(&str, Value)]) -> Result<Option<Gefunden>>
```

`Gefunden` hat `zeile` (die Zeile als JSON-Wert) und `sha256` (sha256 hex klein über die Rohbytes der Zeile ohne LF, wie `vorher` der nächsten Zeile). `Ok(None)` heißt „keine“.

- **Auswahl.** Die letzte Zeile über alle Monatsdateien der Quelle in der Folge der Kette, in der jedes Feld des Filters vorkommt und denselben JSON-Wert hat (Text, Zahl, `true`, `false` oder `null`; Objekte und Listen gleich nach Wert). Ein leerer Filter wählt die letzte Zeile der Kette. Jede Zeile zählt, auch die der Fassungen 1g und 1s.
- **Nur auf einer grünen Kette.** Die Funktion prüft den Ordner wie `kette pruefe-quelle` (Abschnitt 5.4: Zeilen, Kette über die Monate, Anker) in einem Durchgang, in dem sie die Zeilen sieht, und liefert nur dann etwas, wenn die ganze Prüfung grün ist.
  Ein Bruch ist ein Fehler `kette_bruch:<Grund>` mit demselben Grund wie in Abschnitt 3 und 5.4; der Wert trägt Datei und Zeile des Bruchs. Auch ein Bruch hinter der gesuchten Zeile ist ein Fehler: eine kaputte Zeile dahinter könnte eine neuere Zeile der gesuchten Art sein (fail closed).
  Der Auftrag nannte „bis dorthin“; die Funktion prüft mehr, nie weniger.
- **Rückwärts.** Die Kette zu prüfen verlangt, sie von vorn zu lesen; ein Rückwärtslesen der Monate sparte nichts. Das Ergebnis ist dasselbe, als läse man die Monate rückwärts.
- **Fehler in dieser Reihenfolge:** `feld_ungueltig:quelle`, `feld_ungueltig:vorher` (nicht 64 hex klein; wie `--vorher`), dann der Bruch. Ein leerer Ordner ist grün und liefert „keine“. Die Funktion ändert nichts und liest keine Uhr.
- Es gibt dafür keinen Befehl; die Funktion ist eine Bibliotheksfunktion.

Sichtbare Fälle in `faelle/letzte-zeile.json`: `dateien` (wie `pruefe-quelle.json`), `ordner` (`@@` der Ordner, `@weg` ein Ordner, den es nicht gibt), `quelle`, `vorher`, `filter` und `erwartet` (`zeile` mit `sha256`, `keine` oder `fehler` mit `datei` und `zeile` des Bruchs).

## 6. Gemeinsame Zeilenarten

### 6.1 Die Zeile „Kanal abgewiesen“

Stand 20.11.2026, ergänzt am 21.11.2026 um die zwei Gründe der Peer-Prüfung des Such-Sockets unter macOS. Eine gemeinsame Zeilenart der Kanäle zur Wissenssuche, zur Stelle und zum Weiterreicher: Ein Peer, der den Kanal nicht benutzen darf, wird abgewiesen, und die Abweisung steht in der Kette.
Anforderungen wie bei Abschnitt 1: A102 (Kette), A104 (kein Pfad, Titel oder Name in der Zeile), A12 (keine Kennung einer Person oder eines Kontos).

```
{"aktionsname":"kanal.abgewiesen","ergebnis":"abgewiesen","fassung":2,"grund":"pk.sid_nicht_erlaubt","kanal":"such","route":"cloud-beispiel","sitzung":"s-1","vorher":"<64 hex>","zeit":"2026-11-20T13:00:00.000Z"}
```

| Feld | Wert |
|---|---|
| `aktionsname` | genau `kanal.abgewiesen` |
| `ergebnis` | genau `abgewiesen` |
| `fassung` | genau `2` |
| `grund` | entweder einer der 13 festen Gründe der Bibliothek `peerkanal` (Vorsilbe `pk.`, Vertrag `rust/spec/peerkanal/`) oder ein eigener Grund des Kanals, dessen Vorsilbe gleich dem Feld `kanal` ist; die pk-Gründe: `pk.hallo_fehlt`, `pk.hallo_falsch`, `pk.messung_gescheitert`, `pk.prozess_nicht_offen`, `pk.job_unbekannt`, `pk.prozess_beendet`, `pk.startzeit_weicht_ab`, `pk.pidversion_weicht_ab`, `pk.eingeschraenkt`, `pk.sid_nicht_erlaubt`, `pk.appcontainer_falsch`, `pk.uid_nicht_erlaubt`, `pk.gid_falsch` |
| `kanal` | Schlüssel des Kanals, Form wie in Abschnitt 1 (`such`, `stelle`, `weiterreicher`; jeder Schlüssel gilt, eigene Gründe haben heute nur `such`) |
| `route` | Schlüssel der Route oder `null`, wenn die Verbindung keine hatte |
| `sitzung` | Kennung der Sitzung oder `null`, wenn die Verbindung keine hatte |
| `vorher`, `zeit` | wie in Abschnitt 1 |

- **Geschlossene Form.** Genau diese neun Felder, kein weiteres. Keine SID, keine PID, kein Kontoname, kein Pfad (A12, A104): `grund` nennt nur die Art der Abweisung.
- **Eigene Gründe der Kanäle.** Für `kanal` `such` gelten zusätzlich `such.dienst_stumm`, `such.lauf_unbekannt`, `such.sitzung_fremd`, `such.kennung_fremd` und `such.kennung_unmessbar`. Die beiden letzten gehören zur Peer-Prüfung des Such-Sockets unter macOS (Vertrag 4, Nachtrag 21.11.2026): `such.kennung_fremd`, wenn `getpeereid` für die Verbindung eine andere euid meldet als die des Such-Prozesses, `such.kennung_unmessbar`, wenn `getpeereid` scheitert. Die Form der Zeile bleibt gleich (keine uid in der Zeile). Die Vorsilbe muss das Feld `kanal` sein: `such.*` gilt nur bei `kanal` `such`. `stelle` und `weiterreicher` haben heute keine eigenen Gründe, nur die `pk.*`. Jeder andere Grund (unbekannt, falsche Vorsilbe, eine Zeichenfolge der früheren Liste `peer.*`) ist `form_falsch` im Prüfer und `feld_ungueltig:grund` beim Schreiben; die Prüfung des Grundes hängt vom Feld `kanal` ab, nicht umgekehrt.
- **Eine Zeile je abgewiesene Verbindung.** Eine Begrenzung bei einer Flut gleichartiger Abweisungen legt dieser Vertrag nicht fest (Abschnitt 7).
- **Quelle.** Die Zeile steht in der Kette der Quelle, die der Aufrufer nennt (`gateway`, `stelle`, `brain`, `halter` oder `ordnermarke`), über dasselbe Anhängen wie jede andere Zeilenart (Abschnitt 5.3).

**Bibliothek.** `Abgewiesen` hat die Felder `zeit_ms: i64`, `grund`, `kanal` (je `String`) und `route`, `sitzung` (je `Option<String>`).

| Funktion | Ergebnis | Fehler |
|---|---|---|
| `zeile_abgewiesen(&Abgewiesen, vorher) -> Result<String>` | die Zeile ohne LF | `feld_ungueltig:<feld>` für den ersten falschen Wert in der Reihenfolge `grund`, `kanal`, `route`, `sitzung`, `vorher`; danach `zeit_ungueltig` |
| `anhaengen_abgewiesen(ordner, quelle, &Abgewiesen) -> Result<String>` | hängt die Zeile über `anhaengen_zeile` an, gibt den sha256 der Zeile zurück | `feld_ungueltig:<feld>` in der Reihenfolge `grund`, `kanal`, `route`, `sitzung`; dann `feld_ungueltig:quelle`, `zeit_ungueltig`; danach wie Abschnitt 5.3 |

**Prüfen.** `kette pruefe` und `kette pruefe-quelle` prüfen die geschlossene Form nach dem Schritt „`aktionsname` ist `ausgabe.gesperrt`“ aus Abschnitt 3 auch für `kanal.abgewiesen`:
ist `aktionsname` `kanal.abgewiesen` und die Zeile nicht Fassung 2 oder weicht sie von der Form oben ab (ein Feld fehlt oder kommt hinzu, `grund` nicht aus der Liste, ein Schlüssel nicht in der Schlüsselform, `route` oder `sitzung` weder Schlüssel noch `null`, `ergebnis` nicht `abgewiesen`), gilt `form_falsch`.
`anhaengen_zeile` und `anhaengen_zeilen` lehnen eine solche Zeile ab, wie sie `ausgabe.gesperrt` außerhalb der Form ablehnen (`feld_ungueltig:zeile`, `zeile_ungueltig:form_falsch`).

**Sichtbare Fälle** in `faelle/abgewiesen.json` (Erzeuger `bau-faelle.mjs`, eigenes Modell): `zeile` (Felder, `vorher`, erwartete Zeile samt sha256 oder Fehlercode), `pruefe` (wie `faelle/pruefe.json`) und `anhaengen` (wie `faelle/monate.json`, Aufrufart `abgewiesen`).

### 6.2 Die Zeile „Probelauf Zählung“

Stand 20.11.2026 (Nachtrag 3). Der Einsatz der Marker-Zählung im Probelauf steht in der Kette. Die Zeile trägt keinen Marker und keinen Marker-Text, denn ein Marker könnte Inhalte verraten (A104).

```
{"aktionsname":"probelauf.zaehlung","anzahl":3,"ergebnis":"an","fassung":2,"programm":"suche","sitzung":"s-1","vorher":"<64 hex>","zeit":"2026-11-20T13:00:00.000Z"}
```

| Feld | Wert |
|---|---|
| `aktionsname` | genau `probelauf.zaehlung` |
| `anzahl` | Ganzzahl von 1 bis 16: wie viele Marker gezählt wurden |
| `ergebnis` | genau `an` |
| `fassung` | genau `2` |
| `programm` | genau `suche` oder `modellweg` |
| `sitzung` | Schlüssel der Sitzung (Form wie in Abschnitt 1; hier kein `null`) |
| `vorher`, `zeit` | wie in Abschnitt 1 |

Genau diese acht Felder, kein weiteres.

| Funktion | Ergebnis | Fehler |
|---|---|---|
| `zeile_probelauf(&Probelauf, vorher) -> Result<String>` | die Zeile ohne LF | `feld_ungueltig:<feld>` für den ersten falschen Wert in der Reihenfolge `programm`, `sitzung`, `anzahl`, `vorher`; danach `zeit_ungueltig` |
| `anhaengen_probelauf(ordner, quelle, &Probelauf) -> Result<String>` | hängt die Zeile über `anhaengen_zeile` an (Ordner und Quelle wie dort; die Datei folgt aus der Zeit), gibt den sha256 der Zeile zurück | `feld_ungueltig:<feld>` in der Reihenfolge `programm`, `sitzung`, `anzahl`; dann `feld_ungueltig:quelle`, `zeit_ungueltig`; danach wie Abschnitt 5.3 |

`Probelauf` hat die Felder `zeit_ms: i64`, `programm`, `sitzung` (je `String`) und `anzahl: u32`.
Der Prüfer (`kette pruefe`, `kette pruefe-quelle`) prüft die geschlossene Form wie bei `kanal.abgewiesen`: ist `aktionsname` `probelauf.zaehlung` und die Zeile nicht Fassung 2 oder weicht sie von der Form ab, gilt `form_falsch`;
`anhaengen_zeile` und `anhaengen_zeilen` lehnen eine solche Zeile ab (`feld_ungueltig:zeile`, `zeile_ungueltig:form_falsch`).
Sichtbare Fälle in `faelle/probelauf.json` (Abschnitte `zeile`, `pruefe`, `anhaengen` wie bei `faelle/abgewiesen.json`).

### 6.3 Die Zeilen „Person gelöscht“ und „Person Auskunft“ (Vertrag 9)

Stand 20.11.2026 (Nachtrag 5, Teil D2). Vertrag 9 (`rust/spec/personenbezug/README.md`, Abschnitt 8, Zweig, Stand vom 20.11.2026) legt zwei Zeilen der Quelle `stelle` fest und nennt als offen (Frage F3), dass `kette pruefe` ihre geschlossene Form nicht kennt. Dieser Abschnitt trägt die Prüfung nach; gebaut wird hier nur sie, nicht der Schreiber der Stelle.

| Feld | `person.geloescht` | `person.auskunft` |
|---|---|---|
| `aktionsname` | genau `person.geloescht` | genau `person.auskunft` |
| `ergebnis` | `geloescht` oder `unbekannt` | `geliefert` oder `unbekannt` |
| `fassung` | genau `2` | genau `2` |
| `handelnd` | Pseudonym `^ps_[0-9a-f]{32}$` oder `null` (Selbstlöschung) | Pseudonym `^ps_[0-9a-f]{32}$`, nie `null` |
| `loeschmarke` | 64 hex klein bei `geloescht`, `null` bei `unbekannt` | — |
| `zeilen` | — | Ganzzahl 0 bis 4294967295; bei `unbekannt` genau 0 |
| `vorher`, `zeit` | wie in Abschnitt 1 | wie in Abschnitt 1 |

Genau diese sieben Felder je Zeile. `kette pruefe` und `kette pruefe-quelle` melden `form_falsch`, wenn `aktionsname` eine der beiden Arten ist und die Zeile nicht Fassung 2 ist oder von ihrer Form abweicht; `anhaengen_zeile` und `anhaengen_zeilen` lehnen eine solche Zeile ab (`feld_ungueltig:zeile`, `zeile_ungueltig:form_falsch`).
Die Kopplung zwischen `ergebnis` und `loeschmarke` beziehungsweise `zeilen` ist Teil der Form. Die Funktionen `zeile_geloescht` und `zeile_auskunft` aus Vertrag 9 gehören nicht hierher.
Sichtbare Fälle in `faelle/personenzeilen.json` (Abschnitte `pruefe` und `anhaengen`).

### 6.4 Die Zeilen „Ordnermarke gelöst“ und „Ordnermarke gesenkt“

Stand 20.11.2026 (Nachtrag 6, Projektentscheid). Vertrag 1 (`rust/spec/start/README.md`, Abschnitt „Ordnermarke (E6)“) verlangt für `werkbank-ordnermarke loesen` und `werkbank-ordnermarke senken` je Änderung eine Protokollzeile (OPEN 13). Dieser Abschnitt legt sie fest.
Ohne die Zeile darf ein Mensch die Marke nicht lösen oder senken: Der Befehl schreibt die Zeile, bevor er die Markendatei ändert (Vertrag 1). Anforderungen wie bei Abschnitt 1: A102 (Kette), A104 (kein Pfad, kein Hash eines Pfades), A12 (keine Kennung einer Person).

```
{"aktionsname":"ordnermarke.geloest","ergebnis":"ausgefuehrt","fassung":2,"klasse_alt":"kundendaten","klasse_neu":null,"kunde_alt":"k-beispiel","kunde_neu":null,"marke":"a1b2c3d4e5f60718293a4b5c6d7e8f90","vorher":"<64 hex>","zeit":"2026-11-20T13:00:00.000Z"}
{"aktionsname":"ordnermarke.gesenkt","ergebnis":"ausgefuehrt","fassung":2,"klasse_alt":"kundendaten","klasse_neu":"intern","kunde_alt":"k-beispiel","kunde_neu":"k-beispiel","marke":"a1b2c3d4e5f60718293a4b5c6d7e8f90","vorher":"<64 hex>","zeit":"2026-11-20T13:00:00.000Z"}
```

| Feld | Wert |
|---|---|
| `aktionsname` | `ordnermarke.geloest` (Befehl `werkbank-ordnermarke loesen`) oder `ordnermarke.gesenkt` (Befehl `werkbank-ordnermarke senken --klasse`) |
| `ergebnis` | genau `ausgefuehrt`; in dieser Fassung der einzige Wert |
| `fassung` | genau `2` |
| `klasse_alt`, `klasse_neu` | Kennung der Klasse in der Schlüsselform aus Abschnitt 1 oder `null`: `klasse_alt` ist `klasse_hoch` der Marke vor dem Befehl, `klasse_neu` danach |
| `kunde_alt`, `kunde_neu` | Kennung des Kunden in der Schlüsselform aus Abschnitt 1 oder `null` (`null` heißt „kein Kunde“, wie in der Markendatei): `kunde` der Marke vor und nach dem Befehl |
| `marke` | zufällige Kennung der Marke, Muster `^[0-9a-f]{32}$` |
| `vorher`, `zeit` | wie in Abschnitt 1 |

- **Geschlossene Form.** Genau diese zehn Felder, flach, kein weiteres. Die Zeile nennt keinen Pfad, keinen Ordnernamen, keinen Hash eines Pfades, keinen Personennamen und keine Kennung des Menschen, der den Befehl aufgerufen hat (A104, A12).
- **`marke` ist weder Pfad noch Hash eines Pfades.** Ein sha256 über einen Pfad ist erratbar: Wer die Ordner einer Maschine kennt oder rät, bildet den Hash und erkennt, welche Ordner markiert wurden (A104). Darum ist `marke` eine Kennung, die der Befehl zufällig bildet, 128 Bit, 32 Zeichen hex klein, und nicht aus dem Pfad abgeleitet.
  Die Zuordnung von Kennung zu Ordner steht nur in `ordnermarken.json` (Vertrag 1, Zustandsordner, 0700/0600). Wer die Protokollzeile liest, sieht daran keinen Ordner. Wo die Kennung steht und wie lange sie lebt, ist Sache von Vertrag 1; dieser Vertrag prüft nur ihre Form.
- **`ordnermarke.geloest`.** Der Eintrag der Marke ist weg. `kunde_neu` und `klasse_neu` sind `null`; `kunde_alt` und `klasse_alt` sind nicht beide `null` (ein gelöschter Eintrag trug mindestens einen Wert).
- **`ordnermarke.gesenkt`.** `klasse_neu` ist nicht `null` und ungleich `klasse_alt`; `kunde_neu` ist gleich `kunde_alt`, denn Senken ändert den Kunden nicht. `klasse_alt` darf `null` sein; die Zeile hält fest, was der Befehl getan hat.
- **Keine Rangordnung in der Zeile.** Ob „senken“ prüfen muss, dass `klasse_neu` in der Rangfolge der Klassen niedriger liegt als `klasse_alt`, gehört zum Befehl (Vertrag 1, Vertrag 2), nicht zur Zeile. Der Prüfer kennt die Rangfolge nicht und prüft sie nicht.
- **Bytes und Datei.** RFC 8785 (Fassung 2), eine Zeile je Glied, LF; wie bei `ausgabe.gesperrt`.
  Quelle ist `ordnermarke`, die Dateien heißen `ordnermarke-<JJJJ-MM>.jsonl`, Monate und Anker nach Abschnitt 5 (`ordnermarke.anker.jsonl`). `ordnermarke` ist damit eine fünfte Quelle neben `gateway`, `stelle`, `brain` und `halter` (Abschnitt 2, 5.4, 6.1).
- **Fehlercodes beim Schreiben.** Der Schreiber meldet den ersten falschen Wert als `feld_ungueltig:<feld>`: erst die Form je Feld in der Reihenfolge `aktionsname`, `marke`, `klasse_alt`, `klasse_neu`, `kunde_alt`, `kunde_neu`; dann die Kopplung (`geloest`: `klasse_alt` bei beiden `_alt` null, dann `klasse_neu`, dann `kunde_neu`; `gesenkt`: `klasse_neu`, dann `kunde_neu`); dann `vorher`; danach `zeit_ungueltig`.

**Bibliothek.** `Ordnermarke` hat die Felder `zeit_ms: i64`, `aktionsname`, `marke` (je `String`) und `klasse_alt`, `klasse_neu`, `kunde_alt`, `kunde_neu` (je `Option<String>`). `ergebnis` ist fest und kein Feld.

| Funktion | Ergebnis | Fehler |
|---|---|---|
| `zeile_ordnermarke(&Ordnermarke, vorher) -> Result<String>` | die Zeile ohne LF | wie oben, `feld_ungueltig:vorher`, danach `zeit_ungueltig` |
| `anhaengen_ordnermarke(ordner, quelle, &Ordnermarke) -> Result<String>` | hängt die Zeile über `anhaengen_zeile` an, gibt den sha256 der Zeile zurück | `feld_ungueltig:<feld>` wie oben; dann `feld_ungueltig:quelle`, `zeit_ungueltig`; danach wie Abschnitt 5.3 |

**Prüfen.** Bis die geschlossene Form in `formate` gebaut ist, prüft `kette pruefe` diese Zeilen als Kettenglieder der Fassung 2 (Kopf, Kanonik, `vorher`), wie für jede neue Zeilenart. Mit dem Bau gilt dieselbe Regel wie bei `kanal.abgewiesen`:
`kette pruefe` und `kette pruefe-quelle` melden `form_falsch`, wenn `aktionsname` `ordnermarke.geloest` oder `ordnermarke.gesenkt` ist und die Zeile nicht Fassung 2 ist oder von ihrer Form abweicht
(ein Feld fehlt oder kommt hinzu, `marke` nicht 32 hex klein, ein Wert nicht in der Schlüsselform, `ergebnis` nicht `ausgefuehrt`, die Kopplung verletzt). `anhaengen_zeile` und `anhaengen_zeilen` lehnen eine solche Zeile ab (`feld_ungueltig:zeile`, `zeile_ungueltig:form_falsch`).
Jeder andere Aktionsname mit der Vorsilbe `ordnermarke.` hat keine geschlossene Form in diesem Vertrag.

**Sichtbare Fälle** in `faelle/ordnermarke.json` (Erzeuger `bau-faelle.mjs`, eigenes Modell): `zeile` (Felder, `vorher`, erwartete Zeile samt sha256 oder Fehlercode), `pruefe` (wie `faelle/pruefe.json`) und `anhaengen` (wie `faelle/monate.json`, Aufrufart `ordnermarke`, Quelle `ordnermarke`).
Sie decken ab: beide Arten gültig; `marke` mit Großbuchstaben, mit 31 und 33 Zeichen, leer, `null`, als Pfad und als sha256 (64 hex); ein zusätzliches Feld `ordner`, `ordner_hash` oder `titel`; jedes fehlende Feld; `geloest` mit `klasse_neu` oder `kunde_neu`;
`gesenkt` mit `klasse_neu` gleich `klasse_alt`, mit `klasse_neu` `null` und mit anderem `kunde_neu`; `ergebnis` unbekannt; die Reihenfolge der Feldprüfungen; Wechsel in den nächsten Monat mit Anker; das allgemeine Anhängen.
**Mutanten des Modells** (zur Gegenprobe; `ORDNERMARKE_MUTANT=O1|O2|O3 node bau-faelle.mjs` bricht ab, ohne die Variable bleibt die Ausgabe gleich): M-O1 nimmt ein zusätzliches Feld an, M-O2 prüft die Form von `marke` nicht, M-O3 nimmt `geloest` mit `klasse_neu` an.

### 6.5 Die Zeile „Suche Rückfall“

Stand 22.11.2026 (Befund B39, Anforderung A125; Form abgestimmt, Projektentscheid, 22.11.2026). A125 verlangt einen Zähler der Suchantworten mit `fallback: true`, bei denen die Einbettung ausgefallen ist und die Suche lexikalisch antwortet.
Je Sitzung läuft ein eigener Such-Prozess; ein Zähler im Speicher endete mit ihm. Darum schreibt der Such-Prozess je solche Antwort eine Zeile in die Kette `brain`, und die Diagnose zählt die Zeilen. Wann der Such-Prozess schreibt, steht in Vertrag 4 (`rust/spec/such-socket/`).

```
{"aktionsname":"suche.rueckfall","ergebnis":"lexikalisch","fassung":2,"grund":"keine_verbindung","route":"cloud-beispiel","sitzung":"s-1","vorher":"<64 hex>","zeit":"2026-11-22T13:00:00.000Z"}
```

| Feld | Wert |
|---|---|
| `aktionsname` | genau `suche.rueckfall` |
| `ergebnis` | genau `lexikalisch` |
| `fassung` | genau `2` |
| `grund` | genau einer von `keine_verbindung`, `frist`, `antwort_ungueltig`: der Fehler des letzten Versuchs der Einbettung (siehe unten) |
| `route` | Schlüssel der Route der Sitzung (Form wie in Abschnitt 1; hier kein `null`) |
| `sitzung` | Schlüssel der Sitzung (Form wie in Abschnitt 1; hier kein `null`) |
| `vorher`, `zeit` | wie in Abschnitt 1 |

- **Geschlossene Form.** Genau diese acht Felder, kein weiteres. Die Zeile trägt keine Anfrage, keine Notiz, keine Adresse des Einbettungsdienstes und keine Fehlermeldung (A104).
- **Gründe.** Die drei Werte teilen die Fehler eines Versuchs lückenlos auf. `keine_verbindung`: zu keiner Adresse des Endpunkts entsteht eine Verbindung (abgewiesen, oder die Frist des Verbindungsaufbaus ist abgelaufen). `frist`: die Frist des Versuchs läuft nach dem Verbindungsaufbau ab. `antwort_ungueltig`: jeder andere Fehler nach dem Verbindungsaufbau, etwa Status ungleich 200, ein Kopf oder Körper über der Grenze, eine vor dem Ende geschlossene Verbindung, kein JSON, `embeddings` fehlt, ist leer oder kein Vektor. Die Zuordnung im Such-Prozess legt Vertrag 4 fest.
- **Wann eine Zeile entsteht.** Nur wenn der Such-Prozess eine Einbettung eingerichtet hat und sie nach allen Versuchen keinen Vektor liefert; dann eine Zeile je solche Antwort. Ohne eingerichtete Einbettung antwortet die Suche lexikalisch per Einstellung: Die Antwort trägt `fallback: true`, aber es entsteht keine Zeile, denn nichts ist ausgefallen. Eine Anfrage, die ihren Vektor mitbringt, ist nie ein Rückfall.
- **Menge.** Eine Zeile je Rückfall-Antwort, nicht je Wechsel: A125 zählt Antworten, und ein Zähler bis zum nächsten Wechsel ginge mit dem Ende der Sitzung oder des Prozesses verloren. Eine Begrenzung bei Dauerausfall legt dieser Vertrag nicht fest (Abschnitt 7).
- **Schreibfehler.** Anders als bei `ausgabe.gesperrt` (Abschnitt 2 und 5.3) ändert ein Fehler beim Anhängen dieser Zeile die Antwort nicht. Die Zeile dient nur der Diagnose; ein gescheitertes Schreiben lässt die Kette nach Abschnitt 5.3 Schritt 8c ganz, es fehlt nur ein Eintrag des Zählers.
  Dafür gilt: Der Such-Prozess hängt die Zeile mit einem eigenen Aufruf an (`anhaengen_rueckfall` oder `anhaengen_zeile`), nie im selben `anhaengen_zeilen` wie Zeilen `ausgabe.gesperrt`, und erst nach diesen. So entscheidet ihr Fehler nie über die Antwort, der Fehler einer Sperrzeile weiterhin schon.
  Nach einem Fehler gibt es keinen zweiten Versuch für dieselbe Antwort, keinen Puffer im Speicher und keinen Fehlertext in der Antwort; `fallback: true` steht weiter in der Antwort.
- **Quelle.** Die Zeile steht in der Kette `brain` (Vertrag 4). Die Bibliothek nimmt die Quelle als Angabe wie bei den anderen Zeilenarten.

**Bibliothek.** `Rueckfall` hat die Felder `zeit_ms: i64`, `grund`, `route` und `sitzung` (je `String`). `aktionsname` und `ergebnis` sind fest und keine Felder.

| Funktion | Ergebnis | Fehler |
|---|---|---|
| `zeile_rueckfall(&Rueckfall, vorher) -> Result<String>` | die Zeile ohne LF | `feld_ungueltig:<feld>` für den ersten falschen Wert in der Reihenfolge `grund`, `route`, `sitzung`, `vorher`; danach `zeit_ungueltig` |
| `anhaengen_rueckfall(ordner, quelle, &Rueckfall) -> Result<String>` | hängt die Zeile über `anhaengen_zeile` an, gibt den sha256 der Zeile zurück | `feld_ungueltig:<feld>` in der Reihenfolge `grund`, `route`, `sitzung`; dann `feld_ungueltig:quelle`, `zeit_ungueltig`; danach wie Abschnitt 5.3 |

**Prüfen.** Bis die geschlossene Form in `formate` gebaut ist, prüft `kette pruefe` diese Zeile als Kettenglied der Fassung 2 (Kopf, Kanonik, `vorher`), wie jede neue Zeilenart. Mit dem Bau gilt dieselbe Regel wie bei `probelauf.zaehlung`:
`kette pruefe` und `kette pruefe-quelle` melden `form_falsch`, wenn `aktionsname` `suche.rueckfall` ist und die Zeile nicht Fassung 2 ist oder von der Form oben abweicht (ein Feld fehlt oder kommt hinzu, `grund` nicht aus der Liste, `route` oder `sitzung` nicht in der Schlüsselform oder `null`, `ergebnis` nicht `lexikalisch`).
`anhaengen_zeile` und `anhaengen_zeilen` lehnen eine solche Zeile dann ab (`feld_ungueltig:zeile`, `zeile_ungueltig:form_falsch`).

**Sichtbare Fälle** folgen mit dem Bau in `formate` (nach M1) in `faelle/rueckfall.json` (Erzeuger `bau-faelle.mjs`), mit den Abschnitten `zeile`, `pruefe` und `anhaengen` wie bei `faelle/probelauf.json`.
Sie decken mindestens ab: jeder der drei Gründe gültig; ein unbekannter Grund; ein zusätzliches Feld `anfrage`, `adresse` oder `meldung`; jedes fehlende Feld; `route` und `sitzung` als `null`; `ergebnis` unbekannt; die Reihenfolge der Feldprüfungen; Felder vor Quelle vor Zeit; der Wechsel in den nächsten Monat mit Anker; Rückfall- und Sperrzeilen gemischt in einer Kette.

### 6.6 Die Zeilen „Regel geändert“ und „Regel abgelehnt“

Stand 22.11.2026 (Befund B13, abgestimmt am 21.11.2026). Eine Form für beide Schreiber: die Stelle bei einer wirksamen Pflichtänderung (Vertrag 6 Fassung 2, Abschnitt 5) und die Prüfung einer neuen Datei einer Ebene (Vertrag Regelebenen, Abschnitt 7). Vorher standen dort zwei verschiedene Formen; beide Verträge verweisen jetzt hierher.
Anforderungen: A27 (alter und neuer Wert als Prüfsumme), A102 (Kette), A12 (Personen nur als Pseudonym), A104 (kein Name, kein Pfad, kein Wert der Datei in der Zeile).

```
{"aktionsname":"regel.geaendert","alt":"<64 hex>","antrag":null,"code":null,"ebene":"firma","ergebnis":"geaendert","fassung":2,"handelnd":"ps_00000000000000000000000000000b01","neu":"<64 hex>","pflicht":"mcp.notizen.freigabe","vorher":"<64 hex>","zeit":"2026-11-22T13:00:00.000Z","ziel":null,"zweiter":null}
{"aktionsname":"regel.abgelehnt","alt":"<64 hex>","antrag":null,"code":"ebenen.lockerung","ebene":"gruppe","ergebnis":"abgelehnt","fassung":2,"handelnd":"ps_00000000000000000000000000000b01","neu":"<64 hex>","pflicht":null,"vorher":"<64 hex>","zeit":"2026-11-22T13:00:00.000Z","ziel":"g-beispiel","zweiter":null}
```

| Feld | Wert |
|---|---|
| `aktionsname` | genau `regel.geaendert` oder `regel.abgelehnt` |
| `alt` | sha256 hex klein über RFC 8785 der gespeicherten Datei derselben Ebene und desselben Ziels; `null`, wenn es keine gibt |
| `antrag` | Kennung des Antrags `an-<laden_ms>-<n>` (Vertrag 6 Fassung 2), wenn die Änderung eine Bestätigung durch eine zweite Person brauchte; sonst `null` |
| `code` | `null` bei `regel.geaendert`; bei `regel.abgelehnt` der feste Code der Ablehnung (etwa aus Regelebenen Abschnitt 8), Schlüsselform wie in Abschnitt 1 |
| `ebene` | `firma`, `gruppe` oder `nutzer`; `null` nur bei `regel.abgelehnt`, wenn die eingereichte Datei keine dieser Ebenen nennt |
| `ergebnis` | `geaendert` bei `regel.geaendert`, `abgelehnt` bei `regel.abgelehnt` |
| `fassung` | genau `2` |
| `handelnd` | Pseudonym `^ps_[0-9a-f]{32}$` der Person, die die Änderung einreicht oder auslöst; aus der geprüften Anmeldung (Vertrag 9), nie aus einer Datei |
| `neu` | sha256 hex klein über RFC 8785 der neuen Datei: bei `regel.geaendert` der geschriebenen, bei `regel.abgelehnt` der eingereichten; `null` nur bei `regel.abgelehnt`, wenn die eingereichte Datei kein lesbares JSON-Objekt ist (etwa `ebenen.json_ungueltig`, `ebenen.zu_gross`) |
| `pflicht` | Schlüssel der geänderten Pflicht, wenn genau eine Pflicht geändert wurde (Vorgang `pflicht.aendern` der Stelle); `null`, wenn eine ganze Datei eingereicht wurde |
| `vorher`, `zeit` | wie in Abschnitt 1 |
| `ziel` | Kennung der Gruppe bei `gruppe`; Pseudonym des Nutzers bei `nutzer` (nie seine Kontokennung, A12); `null` bei `firma`, und bei `regel.abgelehnt`, wenn die Datei kein gültiges Ziel nennt |
| `zweiter` | Pseudonym der zweiten Person, wenn `antrag` gesetzt ist; sonst `null` |

- **Geschlossene Form.** Genau diese vierzehn Felder, kein weiteres, für beide Aktionsnamen. Die Zeile nennt keinen Wert und keine Rolle aus der Datei, keinen Pfad und keinen Namen (A104); den alten und den neuen Inhalt belegen `alt` und `neu` als Prüfsumme (A27). Die Inhalte selbst liegen als Fassungen der Datei beim Speicher der Konsole (Regelebenen, OPEN 4).
- **Kopplungen.** `ergebnis` folgt aus `aktionsname`. `code` ist genau dann `null`, wenn die Regel geändert wurde. `antrag` und `zweiter` sind beide gesetzt oder beide `null`, und `zweiter` ist nie `handelnd`. `ebene` `null` und `neu` `null` gibt es nur bei `regel.abgelehnt`; bei `ebene` `null` ist auch `ziel` `null`. Bei `firma` ist `ziel` immer `null`.
- **Wer schreibt was.** Die Stelle schreibt nur `regel.geaendert` mit `ebene` `firma` und `ziel` `null`; eine abgelehnte Pflichtänderung ergibt bei ihr `vorgang.abgelehnt` (Vertrag 6 Fassung 2). Die Prüfung von Regelebenen schreibt für jede Prüfung einer Datei eine Zeile, `regel.geaendert` oder `regel.abgelehnt` (A27, A135), mit `pflicht`, `antrag` und `zweiter` `null`, solange Regelebenen keine zweite Person kennt.
- **Lesen beim Start.** Die Hülle der Stelle liest die letzte Zeile `regel.geaendert` mit `ebene` `firma` über Abschnitt 5.7 (Filter `aktionsname` und `ebene`) und vergleicht `neu` mit der gelesenen Datei (Vertrag 6 Fassung 2, Abschnitt 5). Eine Zeile `regel.abgelehnt` zählt dafür nie.

**Bibliothek und Prüfen.** Bis die geschlossene Form in `formate` gebaut ist, prüft `kette pruefe` diese Zeilen als Kettenglieder der Fassung 2 (Kopf, Kanonik, `vorher`), wie jede neue Zeilenart. Mit dem Bau (nach M1, zusammen mit den Zeilenarten der Stelle aus Abschnitt 6.8) melden `kette pruefe` und `kette pruefe-quelle` `form_falsch`, wenn `aktionsname` `regel.geaendert` oder `regel.abgelehnt` ist und die Zeile nicht Fassung 2 ist, von der Form oben abweicht oder eine Kopplung verletzt;
`anhaengen_zeile` und `anhaengen_zeilen` lehnen eine solche Zeile dann ab (`feld_ungueltig:zeile`, `zeile_ungueltig:form_falsch`). Sichtbare Fälle folgen mit dem Bau in `faelle/regel.json`, mit den Abschnitten `zeile`, `pruefe` und `anhaengen` wie bei `faelle/probelauf.json`.

### 6.7 Die Zeile „Modellweg Aufruf“

Stand 23.11.2026. Die Zeile `modellweg.aufruf` gehört Vertrag 8 (`rust/spec/modellweg/`, Abschnitte 8, 11 und 13.4): Er legt ihre Felder und die Kopplung der Werte fest, und der Schreiber steht im Crate `modellweg`.
Dieser Vertrag legt nur fest, **was der Prüfer der Kette von ihr verlangt**, damit eine fremde oder geänderte Zeile dieser Art die Prüfung rot macht (A102, A104).

**Geschlossene Form je Betriebsart.** `betriebsart` nennt die Form: `openai-lokal` hat 15 Felder, `kopf-einsetzen` 16.

| Betriebsart | Felder |
|---|---|
| `openai-lokal` | `aktionsname`, `betriebsart`, `ergebnis`, `fassung`, `grund`, `host`, `modell`, `pfad`, `route`, `sitzung`, `status`, `token_aus`, `token_ein`, `vorher`, `zeit` |
| `kopf-einsetzen` | dieselben ohne `modell`, `token_aus`, `token_ein`, dazu `kopf`, `methode`, `pfadregel`, `ziel` |

- `aktionsname` ist genau `modellweg.aufruf`, `fassung` die Zahl 2, `ergebnis` einer von `weitergereicht`, `abgelehnt`, `ziel_fehler`, `leck_gesperrt`.
- `route` und `sitzung` sind Schlüssel wie in Abschnitt 1; `grund` ist ein Schlüssel oder `null`.
- `status` ist eine Ganzzahl von 100 bis 599, `null` nur in `kopf-einsetzen`.
- `pfad` ist `null` oder ein Wert der Form aus Vertrag 8, Abschnitt 8: `*`, `/`, oder `/` und Segmente, die je `*` oder `[A-Za-z0-9._~-]+` sind, zusammen höchstens 512 Zeichen.
  So kann keine Abfrage, kein Fragment und keine Prozentkodierung in einer Kette stehen.
- `host` ist `null` oder ein Hostname der Zielliste: höchstens 253 Zeichen, mit Punkt, Labels aus `[a-z0-9-]` ohne Bindestrich am Rand, also klein und ohne Port.
  In `openai-lokal` ist er immer `null`: Diese Betriebsart hat genau ein Ziel aus ihrem Aufruf (Vertrag 8, Abschnitt 11).
- `modell` ist ein Text von 1 bis 128 Zeichen; `token_ein` und `token_aus` sind je eine Ganzzahl von 0 bis 4294967295 oder `null`.
- `ziel` ist ein Schlüssel oder `null`; `methode` eine der sieben bekannten oder `null`; `kopf` einer von `ersetzt`, `keiner`, `abgewiesen`, `halter` oder `null`;
  Nachtrag 25.11.2026: `halter` für ein gebundenes Ziel auf dem Halterweg (Vertrag 8, Abschnitt 13.4).
  `pfadregel` ist `null` oder ein Regelpfad (`/*`, oder Segmente `/[A-Za-z0-9._~-]+` ohne `.` und `..` mit einem `/*` am Ende), nie der Pfad einer Anfrage.
- **Ältere Zeilen ohne `pfad` und `host`** (13 Felder in `openai-lokal`, 14 in `kopf-einsetzen`) gelten weiter: Der Prüfer kennt für Fassung 2 keine Unterfassungen, und eine Kette, die vor dem 23.11.2026 geschrieben wurde, darf nicht rot werden.
  Es gelten genau diese zwei Formen: beide Felder da oder beide weg. Ein einzelnes von ihnen ist `form_falsch`. Ob die alte Form wegfällt, entscheidet das Projekt (Vertrag 8, OPEN 19).
- **Die Kopplung der Werte** (welches Ergebnis welchen Grund trägt, wann `pfad` gesetzt ist, dass `host` genau mit `ziel` steht) prüft der Schreiber in Vertrag 8, nicht dieser Prüfer. Er prüft die Form.

**Prüfen.** `kette pruefe` und `kette pruefe-quelle` prüfen diese Form nach dem Schritt „`aktionsname` ist `ausgabe.gesperrt`“ aus Abschnitt 3, wie für `kanal.abgewiesen`:
Ist `aktionsname` `modellweg.aufruf` und die Zeile nicht Fassung 2 oder weicht sie von der Form oben ab, gilt `form_falsch`. `anhaengen_zeile` und `anhaengen_zeilen` lehnen eine solche Zeile ab (`feld_ungueltig:zeile`, `zeile_ungueltig:form_falsch`).

**Sichtbare Fälle** in `faelle/pruefe.json` (Erzeuger `bau-faelle.mjs`): eine Kette aus zwei Zeilen beider Betriebsarten (grün), eine alte Zeile ohne `pfad` und `host` (grün), eine Zeile mit nur einem der zwei Felder, eine mit einem Pfad samt Abfrage und eine mit einem zusätzlichen Feld (je `form_falsch`).

### 6.8 Die Zeilenarten der Entscheidungsstelle

Stand 23.11.2026 (Befund B2 des Reviews von Vertrag und Code). Die vier Zeilenarten `mcp.schreibaufruf`, `sitzung.gestartet`, `sitzung.abgelehnt` und `vorgang.abgelehnt` gehören Vertrag 6 (`rust/spec/stelle/README.md`, Abschnitte 7.1 bis 7.3): Er legt ihre Felder, ihre Werte und die Kopplung der Werte fest, und der Schreiber steht im Crate `stelle` (`src/zeile.rs`).
Dieser Vertrag legt nur fest, **was der Prüfer der Kette von ihnen verlangt**, damit eine fremde oder geänderte Zeile dieser Art die Prüfung rot macht (A102, A104, A12). Weicht Vertrag 6 vom Code ab, gilt Vertrag 6.
Alle vier Arten schreibt die Stelle in die Kette der Quelle `stelle`; jede ist Fassung 2 und trägt `fassung`, `vorher` und `zeit` wie in Abschnitt 1.

```
{"aktionsname":"mcp.schreibaufruf","anfrage":"mf-1795179540000-1","digest":"34b94b40…ba4ced","entscheidend":"ps_00000000000000000000000000000a01","ergebnis":"eingeloest","fassung":2,"person":"ps_00000000000000000000000000000001","server":"tickets","sitzung":"s-1","vorher":"e2bbd239…8070e1","werkzeug":"update","zeit":"2026-11-20T13:00:08.000Z","zeit_anfrage":"2026-11-20T13:00:01.000Z","zeit_entscheidung":"2026-11-20T13:00:05.000Z"}
{"aktionsname":"sitzung.gestartet","ergebnis":"gestartet","fassung":2,"grund":null,"harness":"claude","kunde":"k-beispiel","route":"cloud-beispiel","sitzung":"s-1","vorher":"0000…0000","zeit":"2026-11-20T13:00:00.000Z"}
{"aktionsname":"sitzung.abgelehnt","ergebnis":"abgelehnt","fassung":2,"grund":"start.route_unbekannt","harness":"claude","kunde":"k-beispiel","route":"cloud-fremd","sitzung":"s-1","vorher":"0000…0000","zeit":"2026-11-20T13:00:00.000Z"}
{"aktionsname":"vorgang.abgelehnt","anfrage":"mf-1795179540000-1","aufrufer":"hub","ergebnis":"abgelehnt","fassung":2,"grund":"stelle.bereits_eingeloest","vorgang":"mcp.einloesen","vorher":"a3874f23…2401","zeit":"2026-11-20T13:00:04.000Z"}
```

**`mcp.schreibaufruf`, vierzehn Felder** (Vertrag 6, Abschnitt 7.1): `aktionsname`, `anfrage`, `digest`, `entscheidend`, `ergebnis`, `fassung`, `person`, `server`, `sitzung`, `vorher`, `werkzeug`, `zeit`, `zeit_anfrage`, `zeit_entscheidung`.

| Feld | Wert |
|---|---|
| `aktionsname` | genau `mcp.schreibaufruf` |
| `anfrage` | Anfragekennung `^mf-[0-9]{1,15}-[1-9][0-9]{0,9}$` |
| `digest` | sha256 hex klein, 64 Zeichen |
| `entscheidend` | Pseudonym `^ps_[0-9a-f]{32}$` oder `null` |
| `ergebnis` | `eingeloest`, `abgelehnt`, `abgelaufen` oder `ohne_freigabe` |
| `person` | Pseudonym `^ps_[0-9a-f]{32}$`, nie `null` |
| `server`, `sitzung` | Schlüssel wie in Abschnitt 1 |
| `werkzeug` | Werkzeugname `^[A-Za-z0-9][A-Za-z0-9._-]{0,127}$` |
| `zeit_anfrage` | Zeit im Muster aus Abschnitt 1 |
| `zeit_entscheidung` | Zeit im Muster aus Abschnitt 1 oder `null` |

**`sitzung.gestartet` und `sitzung.abgelehnt`, je zehn Felder** (Vertrag 6, Abschnitt 7.2): `aktionsname`, `ergebnis`, `fassung`, `grund`, `harness`, `kunde`, `route`, `sitzung`, `vorher`, `zeit`.

| Feld | Wert |
|---|---|
| `aktionsname` | genau `sitzung.gestartet` oder `sitzung.abgelehnt` |
| `ergebnis` | `gestartet` bei `sitzung.gestartet`, `abgelehnt` bei `sitzung.abgelehnt` |
| `grund` | `null` bei `sitzung.gestartet`; bei `sitzung.abgelehnt` der Code der Ablehnung in Schlüsselform (`start.*` aus Vertrag 1 oder ein `stelle.*`-Code aus Vertrag 6, Abschnitt 8) |
| `harness`, `kunde`, `route`, `sitzung` | je ein Schlüssel wie in Abschnitt 1 oder `null` |

**`vorgang.abgelehnt`, neun Felder** (Vertrag 6, Abschnitt 7.3): `aktionsname`, `anfrage`, `aufrufer`, `ergebnis`, `fassung`, `grund`, `vorgang`, `vorher`, `zeit`.

| Feld | Wert |
|---|---|
| `aktionsname` | genau `vorgang.abgelehnt` |
| `anfrage` | Anfragekennung wie oben oder `null` |
| `aufrufer` | `kern`, `hub`, `mensch`, `agent`, `takt` oder `unbekannt` (Vertrag 6, Abschnitt 3) |
| `ergebnis` | genau `abgelehnt` |
| `grund` | Code der Ablehnung in Schlüsselform |
| `vorgang` | `mcp.anfragen`, `mcp.entscheiden`, `mcp.einloesen`, `mcp.offene`, `mcp.stand`, `ablauf.pruefen`, `person.pseudonym` oder `worker.anfordern` (Vertrag 6, Abschnitte 4 und 14; die letzten zwei seit dem Nachtrag vom 27.11.2026). Der Name `sitzung.starten` steht hier nie: Eine abgelehnte Startanfrage heißt `sitzung.abgelehnt` |

- **Geschlossene Form.** Genau die genannten Felder je Art, kein weiteres. Ein Zusatzfeld lehnt der Prüfer ab, auch `vorschau`, `argumente`, ein Feld mit einem Personennamen und ein Feld mit einem Pfad (A12, A104). Ein fehlendes Feld lehnt er ebenso ab. Die Zeile nennt kein Argument, keine Vorschau, keinen Namen und keinen Pfad; die Person steht nur als Pseudonym, ein Argument nur als `digest` (Vertrag 6, Abschnitt 7).
- **Kopplungen, die der Prüfer prüft.** `ergebnis` folgt aus `aktionsname` bei den drei Arten mit festem Ergebnis. Bei `mcp.schreibaufruf` sind `entscheidend` und `zeit_entscheidung` beide gesetzt oder beide `null`; `eingeloest` und `abgelehnt` tragen einen Entscheidenden, `ohne_freigabe` trägt keinen, und `abgelaufen` darf beides sein (Vertrag 6, Abschnitt 7.1). Bei den Sitzungszeilen ist `grund` genau dann `null`, wenn die Sitzung gestartet ist.
- **Was der Prüfer nicht prüft.** Dass genau eine Zeile je Anfrage steht, die Reihenfolge der Zeilen eines Ereignisses (Vertrag 6, Abschnitt 7.4), die Zuordnung eines Codes zu einer Stufe und dass alle vier Werte der Sitzungszeile bei einer nicht lesbaren Startdatei `null` sind. Das prüft der Schreiber in Vertrag 6, nicht dieser Prüfer. Er prüft die Form.

**Prüfen.** `kette pruefe` und `kette pruefe-quelle` prüfen diese Formen nach dem Schritt „`aktionsname` ist `ausgabe.gesperrt`“ aus Abschnitt 3, wie für `kanal.abgewiesen`:
Ist `aktionsname` eine der vier Arten und die Zeile nicht Fassung 2 oder weicht sie von der Form oben ab, gilt `form_falsch`. `anhaengen_zeile` und `anhaengen_zeilen` lehnen eine solche Zeile ab (`feld_ungueltig:zeile`, `zeile_ungueltig:form_falsch`).

**Sichtbare Fälle** in `faelle/stellezeilen.json` (Erzeuger `bau-faelle.mjs`), Abschnitte `pruefe` und `anhaengen`: je Zeilenart ein gültiger Fall, ein Fall mit einem Zusatzfeld (`vorschau` oder `argumente`) und ein Fall mit einem fehlenden Feld, dazu die Kopplungen und eine Kette aus Zeilen aller vier Arten.

## 7. Nicht in diesem Vertrag (folgt nach der Vorführung)

- Die Zeilenarten der Entscheidungsstelle stehen seit dem 23.11.2026 in Abschnitt 6.8 und sind nicht mehr offen; die Zeile für das Zeigen einer Marke (`werkbank-ordnermarke zeigen`) ändert nichts und hat keine Zeile; Monate der Fassungen 1g und 1s werden nicht geschrieben.
- Eine Begrenzung bei einer Flut gleichartiger Abweisungen (`kanal.abgewiesen`, Abschnitt 6): offen, braucht eine Entscheidung (etwa eine Zeile je Zeitfenster mit Zähler, was `anzahl` und eine zweite Zeilenform verlangte).
- Eine Begrenzung der Zeilen `suche.rueckfall` bei dauerhaftem Ausfall der Einbettung (Abschnitt 6.5): offen wie bei `kanal.abgewiesen`; heute eine Zeile je Rückfall-Antwort.
- Lückenglied `audit.luecke` (A102): gehört zu Vertrag 5, nach der Vorführung, zusammen mit dem Gateway (Projektentscheid, 20.11.2026).
- Pflege der Aufbewahrung, also das Löschen ganzer Monatsdateien nach `protokoll.aufbewahrung_tage` (Vorgabe 365; Feld des Vertrags Regelebenen, Block `protokoll`, nicht von Vertrag 2; nur die Firma setzt es, Befund B10): gehört zu Vertrag 5, nach der Vorführung (Projektentscheid, 20.11.2026).
- Auskunft und Löschen je Person (A12): Vertrag 9; weitere Zeilenarten.
- Eine Sperre über Rechner hinweg (Netzlaufwerk); die Sperre gilt für Prozesse auf einem Rechner.
- Rechte der neuen Datei unter Windows; neu kanonisieren der Fassungen 1g und 1s; Zeilen der Fassung 1g mit einsamen Surrogaten (das alte Gateway schreibt sie als `\udxxx`).

<!-- vom Bauvorgang angefuegt -->
> Hinweis: Dieser Text nennt Werkzeuge, die in diesem Klon nicht liegen: `brain`.
> Sie gehoeren zum Setup der Maschine, aus der dieser Baum stammt; hier fehlen
> sie mit Absicht. Wer sie braucht, schreibt sich seine eigenen.
> *Named here, not shipped here.*
