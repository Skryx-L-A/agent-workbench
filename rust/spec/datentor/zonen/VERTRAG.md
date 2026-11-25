# Teilvertrag `datentor zonen`: Modellzonen im Tor

Stand 20.11.2026, Teilvertrag zu Vertrag 3 (`rust/spec/VERTRAEGE.md`).
Grundlage sind A127 und A128, dazu A26, A29, A31, A34, A42, A45, A48, A50 und A139 der Anforderungsliste.
„(V)“ heißt: in diesem Vertrag festgelegt. „(H)“ heißt: aus den Anforderungen hergeleitet.

Dieser Stand enthält nur den Vertrag und die sichtbaren Fälle, keinen Code. Ohne Programm sind alle Fälle rot.
Verdeckte Fälle und Mutanten folgen mit dem Bau. Ein Bau gilt als vertragstreu, wenn er beide Fallsätze grün besteht und Vertrag 3 unverändert grün bleibt.

## 1. Schnitt

Die Modellzonen ergänzen `darf` aus Vertrag 3 um eine zweite Prüfung je Route. Das Tor bleibt eine reine Bibliothek ohne Ein- und Ausgabe, ohne Uhr und ohne Zufall.

- Eine **Zone** ist ein Eintrag der Admins mit einem Schlüssel, einer Klassenregel, einer Bedingung und einer Liste von Routen. Jede Route gehört zu höchstens einer Zone (A127). Die Zone hängt an der Route, nicht am Modellnamen (A48).
- Das Tor kennt keinen Zonennamen, kein Präfix und keinen Klassennamen (A29). Es vergleicht Schlüssel byte-genau und Klassen nur nach ihrer Position. Name, Präfix, Farbe und Beschreibung einer Zone sind Anzeigedaten; sie gehören nicht zur Sicht des Tors.
- Die Funktion `darf` aus Vertrag 3 bleibt unverändert, mit derselben Signatur. Die Zonen prüft eine neue reine Funktion `darf_zonen` (Abschnitt 8). Sie ruft bei ausgeschalteter Option nur `darf` auf.
- Eine Anfrage darf ohne Einheit kommen (`einheit: null`). Das ist die **Laufprüfung**: Darf auf dieser Route überhaupt ein Lauf starten? Sie prüft nur die Regeln ohne Einheit. So läuft eine Sitzung mit Ausnahme zur Evaluierung, bekommt aber keine Einheit (A128, Handlung 4).

Nicht in diesem Teilvertrag: Anzeige und Bedienung in der Konsole, das Schreiben des Protokolls, die Anzeige der wirksamen Regel (A28), die Firmenregel für einen einzelnen Nutzer (A26) als eigene Ebene, Status je Route (A45, A46).

## 2. Option „Modellzonen“

- Die Option heißt in der Sicht des Tors `modellzonen.an` und ist ein Wahrheitswert. Die Vorgabe ist aus (A127, Projektentscheid, 19.11.2026). Den Wert `false` setzt der Lader von Vertrag 2 ein, wenn die Einstellungsdatei nichts sagt. Im Fall des Tors ist das Feld Pflicht, damit es keine zweite Vorgabe gibt.
- **Z0, Option aus:** `darf_zonen` gibt für jede Anfrage mit Einheit genau `darf` aus Vertrag 3 zurück, mit demselben Grund, derselben Klasse und derselben Meldung. `projekt` und `kontext` zählen nicht. Die Laufprüfung prüft nur D2 und D3 und ist sonst `frei`. `protokoll` ist `null`.
- Auch bei ausgeschalteter Option muss `modellzonen` gültig sein (Abschnitt 3). Eine fehlerhafte Zone macht den Fall ungültig, gleich ob die Option an ist (fail closed, V).

## 3. Sicht des Tors auf die Zonen

```json
{"an": true,
 "zonen": [{"schluessel": "z-a", "klassen": "alle", "bedingung": "immer", "routen": ["lokal-a"]},
           {"schluessel": "z-b", "klassen": "alle", "bedingung": "nachrangig", "routen": ["cloud-b"]},
           {"schluessel": "z-c", "klassen": "alle", "bedingung": "freigabe", "routen": ["cloud-c"]},
           {"schluessel": "z-d", "klassen": "niedrigste", "bedingung": "immer", "routen": ["cloud-d"]},
           {"schluessel": "z-e", "klassen": "keine", "bedingung": "gesperrt", "routen": ["cloud-e"]}],
 "freigaben": [{"route": "cloud-c", "kunde": "k-a", "projekt": "p-1"}],
 "ausnahmen": [{"zone": "z-e", "nutzer": ["n-a"], "gruppen": [], "gesetzt_von": "admin-1",
                "zweck": "Evaluierung", "bis": 1800000000}],
 "regeln": [{"ebene": "gruppe", "wer": "g-a", "zone": "z-d", "klassen": "keine"},
            {"ebene": "nutzer", "wer": "n-a", "zone": "z-b", "gesperrt": true}]}
```

- `zonen`: die Zonen **in Vorrangfolge**, die vorrangige zuerst (A128). Je Zone:
  - `schluessel`: Schlüssel der Zone.
  - `klassen`: der Klassensatz. `alle` heißt bis zur höchsten Klasse, `niedrigste` heißt nur die erste Klasse in `klassen`, `keine` heißt keine Klasse. Die Vorlage nennt den zweiten Wert „nur öffentliche“; das Tor kennt den Namen nicht und nimmt die Position (V, A29).
  - `bedingung`: `immer`; `nachrangig` (nur wenn eine vorrangige Zone nicht ausreicht); `freigabe` (Kundendaten nur mit Freigabe je Kunde und Projekt); `gesperrt` (gesperrt, Ausnahme zur Evaluierung).
  - `routen`: Liste von Routenschlüsseln. Fehlt sie, gilt sie als leer.
- `freigaben`: Paare aus Route, Kunde und Projekt. Das ist der Schalter der Admins aus A50, ergänzt um das Projekt (A128). Eine Freigabe darf mehrfach stehen; das ändert nichts (wie Vertrag 3). Fehlt das Feld, gilt es als leer.
- `ausnahmen`: Ausnahmen zur Evaluierung. Je Ausnahme `zone`, `nutzer` und `gruppen` (je eine Liste, fehlt eine, gilt sie als leer), `gesetzt_von` (Schlüssel des Admins), `zweck` (Text) und `bis` (Frist, Abschnitt 6). Fehlt das Feld, gilt es als leer.
- `regeln`: Regeln der Gruppen und Nutzer. Je Regel `ebene` (`gruppe` oder `nutzer`), `wer` (Schlüssel der Gruppe oder des Nutzers), `zone`, dazu `klassen` (ein Klassensatz) oder `gesperrt` (nur `true`) oder beides. Fehlt das Feld, gilt es als leer.

**Schlüssel** haben die Form `^[a-z0-9][a-z0-9._-]{0,63}$` wie in Vertrag 3. Das gilt für Zonen, Projekte, Nutzer und Gruppen in `modellzonen` und für `gesetzt_von`.

**Pflichtfelder** sind `an` und `zonen`, je Zone `schluessel`, `klassen` und `bedingung`, je Freigabe alle drei Felder, je Ausnahme `zone`, `gesetzt_von`, `zweck` und `bis`, je Regel `ebene`, `wer` und `zone`.

**Ungültig** ist `modellzonen` (Z10), wenn eine dieser Bedingungen gilt:

- Ein Feld fehlt, hat den falschen Typ oder ist unbekannt. Die Form ist geschlossen, auf jeder Ebene. `an` ist nur `true` oder `false`.
- Ein Schlüssel hat nicht die Form, oder ein Zonenschlüssel kommt doppelt vor.
- `klassen` oder `bedingung` hat einen anderen als die genannten Werte. Ein Klassenname ist kein Klassensatz.
- Eine Zone nennt eine Route, die es in der Vorgabe nicht gibt. Eine Route steht in zwei Zonen oder zweimal in derselben Zone.
- Die erste Zone hat die Bedingung `nachrangig`; dann gibt es keine vorrangige Zone (V).
- Eine Freigabe nennt eine Route, deren Zone nicht die Bedingung `freigabe` hat, eine Route ohne Zone oder einen unbekannten Kunden. Ein Wert in `route`, `kunde` oder `projekt`, der kein Text ist (etwa eine Liste), macht `modellzonen` ebenso ungültig (Nachtrag 20.11.2026, 2).
- Eine Ausnahme nennt eine Zone, die es nicht gibt oder die nicht die Bedingung `gesperrt` hat. Sie nennt weder Nutzer noch Gruppen. `zweck` hat kein Zeichen außer Leerraum. `bis` ist keine ganze Zahl von 0 bis 2^64 - 1 in der Zahlform aus Abschnitt 6 (Nachtrag 20.11.2026, 2).
- Eine Regel nennt eine unbekannte Zone. Sie hat weder `klassen` noch `gesperrt`, oder `gesperrt` ist nicht `true`.
- **Eine Regel lockert:** Ihr Klassensatz lässt mehr Klassen zu als der Klassensatz der Zone (A128: Gruppen- und Nutzerregeln dürfen nur verschärfen). Gleich ist keine Lockerung. Eine Regel kann die Bedingung einer Zone nicht ändern; das Feld gibt es nicht.

Mit einem ungültigen `modellzonen` entsteht kein Tor (V). Die Ablehnung der Lockerung beim Schreiben in die Einstellungsdatei, mit Rückgabecode ungleich 0, ist Sache von Vertrag 2 und der Konsole. Das Tor lehnt sie zusätzlich ab, wenn sie trotzdem ankommt.

**Leerraum** heißt in diesem Vertrag nur Leerzeichen, Tabulator, LF und CR. Andere Zeichen, auch andere Leerzeichen aus Unicode, zählen als Zeichen (V).

## 4. Antwort

```json
{"darf": true, "grund": "frei", "klasse": "intern", "meldung": "frei",
 "protokoll": {"zone": "z-b", "grund": "z-a reicht nicht", "ausnahme": null}}
```

- `darf`, `grund`, `klasse` und `meldung` haben die Bedeutung aus Vertrag 3, Abschnitt 3. Bei der Laufprüfung ist `klasse` immer `null`.
- `protokoll` ist bei ausgeschalteter Option `null`. Bei eingeschalteter Option ist es ein Objekt mit drei Feldern (Z8):
  - `zone`: der Schlüssel der Zone der Route, oder `null`, wenn die Route fehlt, unbekannt ist oder keine Zone hat.
  - `grund`: der Grund der nachrangigen Wahl, genau wie übergeben. Er steht, wenn die Zone die Bedingung `nachrangig` hat und der Grund gültig ist (Abschnitt 6). Sonst `null`.
  - `ausnahme`: die geltende Ausnahme zur Evaluierung mit allen sechs Feldern (fehlende Listen als `[]`). Sie steht, wenn die Zone die Bedingung `gesperrt` hat und eine Ausnahme gilt (Abschnitt 6). Sonst `null`.
- `protokoll` hängt nicht vom Ergebnis ab. Es steht auch bei `darf: false`, damit die Protokollzeile die Angaben nennen kann (A128: Grund und Ausnahme stehen im Protokoll).
- Der Aufrufer gibt dem Modell bei `darf: false` weiter nur die Zahl der gesperrten Einheiten, nie `grund`, `klasse`, `meldung` oder `protokoll` (A49, wie Vertrag 3).

## 5. Regeln bei eingeschalteter Option

Die Prüfungen laufen in dieser Reihenfolge. Die erste zutreffende bestimmt den Grund (Z9, wie D12). Die Laufprüfung (`einheit: null`) prüft nur die Zeilen mit „ja“ in der Spalte „Lauf“; trifft keine zu, ist sie `frei`.

| Regel | Code | Wann | Meldung | Lauf | Herkunft |
|---|---|---|---|---|---|
| D2 | `route-fehlt` | Die Route fehlt oder ist leer. | `Anfrage ohne Route` | ja | A43 |
| Z1 | `nutzer-fehlt` | `kontext.nutzer` fehlt, ist `null` oder leer. | `Anfrage ohne Nutzer` | ja | (V), fail closed: ohne Nutzer gilt keine Nutzerregel |
| Z2 | `route-ohne-zone` | Die Route steht nicht in der Vorgabe oder in keiner Zone. | `Route {route} hat keine Zone` | ja | A127 |
| D4 | `einheit-ohne-klasse` | wie Vertrag 3 | `Einheit ohne gültige Klasse` | nein | (V) |
| Z3 | `zone-durch-regel-gesperrt` | Eine Regel des Nutzers oder einer seiner Gruppen sperrt die Zone. Eine Ausnahme hebt das nicht auf. | `Zone der Route {route} ist durch eine Regel gesperrt` | ja | A128, A26 |
| Z4 | `zone-gesperrt` | Die Zone hat die Bedingung `gesperrt`, und keine Ausnahme gilt. | `Zone der Route {route} ist gesperrt` | ja | A128 |
| Z5 | `grund-fehlt` | Die Zone hat die Bedingung `nachrangig`, und der Grund ist nicht gültig. | `Route {route} braucht einen Grund für die nachrangige Zone` | ja | A128 |
| Z6 | `klasse-ueber-zone` | Die wirksame Klasse liegt über der **Grenze der Zone**. | `Klasse der Einheit liegt über der Zone der Route {route}` | nein | A128 |
| D6 | `klasse-ueber-route` | Die Route hat eine eigene Klasse, und die wirksame Klasse liegt darüber. | `Klasse der Einheit liegt über Route {route}` | nein | A128, A34 |
| D7 | `kunde-fehlt` | wie Vertrag 3 | `Kundendaten ohne Kunde` | nein | (V) |
| D8 | `kunde-ohne-eintrag` | wie Vertrag 3 | `Kunde ohne Eintrag in der Kundenvorgabe` | nein | A51 |
| D9 | `route-fuer-kunde-verboten` | wie Vertrag 3 | `Route {route} ist für diesen Kunden verboten` | nein | A51 |
| D10 | `route-fuer-kunde-nicht-erlaubt` | wie Vertrag 3 | `Route {route} ist für diesen Kunden nicht erlaubt` | nein | A51 |
| Z7 | `zone-ohne-freigabe` | Die Zone hat die Bedingung `freigabe`, und es gibt keine Freigabe für genau Route, Kunde und Projekt der Einheit. | `Kundendaten an Route {route} ohne Freigabe für Kunde und Projekt` | nein | A128, A50 |
| D11 | `kundendaten-an-cloud` | Die Zone hat nicht die Bedingung `freigabe`, die Route hat die Art `cloud`, und es gibt keine Freigabe nach Vertrag 3 für Route und Kunde. | `Kundendaten an Cloud-Route {route} ohne Freigabe` | nein | A50, A52 |
| D1 | `frei` | sonst | `frei` | ja | |

D7 bis D10, Z7 und D11 gelten nur für Kundendaten, wie in Vertrag 3. D3 (`route-ohne-klasse`) gilt bei eingeschalteter Option nicht: Die Route bekommt ihre Klassen aus der Zone (A128).

- **D5, wirksame Klasse:** unverändert aus Vertrag 3. Eine Einheit mit Kunde hat die höchste Klasse.
- **Grenze der Zone:** die niedrigste Position aus dem Klassensatz der Zone und den Klassensätzen aller Regeln, die für den Nutzer gelten. `alle` ist die Position der höchsten Klasse, `niedrigste` die Position 0, `keine` liegt unter jeder Klasse. Bei `keine` ist jede Einheit gesperrt.
- **Der strengere Wert gilt:** Trägt die Route eine eigene Klasse, muss die Einheit auch darunter bleiben (A128, Handlung 6). Das gilt in beide Richtungen: Eine Route mit eigener Klasse `kundendaten` in einer Zone mit `niedrigste` bekommt nur die niedrigste Klasse.
- **Geltende Regeln:** Eine Regel gilt, wenn sie die Zone der Route nennt und `ebene: nutzer` mit `wer` gleich `kontext.nutzer` oder `ebene: gruppe` mit `wer` in `kontext.gruppen` hat. Byte-genau (D14).
- **Z7 und D11 sind ein Schalter:** In einer Zone mit `freigabe` ersetzt die Freigabe mit Projekt die Freigabe nach A50. Eine Freigabe nach Vertrag 3 ohne Projekt reicht dort nicht. Eine Einheit ohne Projekt (`null`) bekommt dort keine Kundendaten (V, A128: „derselbe Schalter wie in A50, ergänzt um das Projekt“).
- **Bedingung und Klassen sind getrennt:** Eine Ausnahme hebt nur die Sperre (Z4) auf, nicht die Klassenregel (Z6). Ein Grund hebt nur Z5 auf. Eine Freigabe hebt nur Z7 auf.

## 6. Kontext, Zeit, Grund und Ausnahme

Jede Anfrage trägt einen `kontext`. Er kommt aus dem Start der Sitzung, nie aus der Anfrage des Agenten (wie die Route in Vertrag 3).

| Feld | Typ | fehlt | Bedeutung |
|---|---|---|---|
| `nutzer` | Text oder `null` | `null` | Schlüssel des Nutzers der Sitzung |
| `gruppen` | Liste von Texten | `[]` | Gruppen des Nutzers; `null` ist ungültig |
| `jetzt` | ganze Zahl 0 bis 2^64 - 1, oder `null` | `null` | Zeitpunkt der Prüfung in Sekunden seit 1970-01-01 00:00:00 UTC |
| `grund` | Text oder `null` | `null` | Grund der Wahl einer nachrangigen Zone |

- **Das Tor kennt keine Uhr.** Die Zeit für die Frist ist eine Eingabe. Bei gleicher Eingabe ist die Antwort gleich (D15).
- **Ein Grund ist gültig,** wenn er ein Text mit mindestens einem Zeichen außer Leerraum ist. Das Tor prüft nicht, ob eine vorrangige Zone tatsächlich ausgereicht hätte; das kann es nicht wissen (H). Es verlangt den Grund und reicht ihn ins Protokoll.
- **Eine Ausnahme gilt,** wenn alle Bedingungen zutreffen: Sie nennt die Zone der Route. `kontext.nutzer` steht in ihren `nutzer`, oder eine Gruppe aus `kontext.gruppen` steht in ihren `gruppen`. `jetzt` ist nicht `null` und liegt **vor** `bis`. Bei `jetzt` gleich `bis` ist die Frist abgelaufen (V, fail closed). Gelten mehrere, gilt die erste in der Liste.
- **Ausnahme ohne Nutzer** (Nachtrag 20.11.2026, 2): Ob eine Ausnahme gilt, hängt nicht vom Nutzer allein ab. Fehlt `kontext.nutzer` oder ist er `null` oder leer, kann sie über eine Gruppe gelten. Die Antwort ist dann trotzdem `nutzer-fehlt` (Z1, fail closed). `protokoll.ausnahme` nennt die Ausnahme, die über die Gruppe passt, weil das Protokoll nicht vom Ergebnis abhängt (Abschnitt 4).
- **Zahlform von `jetzt` und `bis`** (Nachtrag 20.11.2026, 2): Gültig ist nur eine ganze Zahl ohne Vorzeichen in Ziffernform, also eine JSON-Zahl aus Ziffern ohne `-`, ohne Dezimalpunkt und ohne Exponent, höchstens 2^64 - 1. `-0`, `1e9`, `1.0`, `+1` und Zahlen über 2^64 - 1 ergeben Exit 2; `+1` ist schon kein JSON. Führende Nullen sind in JSON nicht erlaubt.
- `projekt` steht in der Anfrage neben `kunde`: Text oder `null`, fehlt es, gilt `null`. Es ist das Projekt der Einheit, so wie `kunde` der Kunde der Einheit ist. Vergleich byte-genau.

## 7. Aufruf des Außentestprogramms

- `datentor zonen < fall.json` (oder `datentor zonen -`): wie `datentor` in Vertrag 3, Abschnitt 5, mit eigenem Fallformat. Grenzen, Exitcodes, Tiefe, doppelte Schlüssel (A139) und die Form von stdout und stderr gelten unverändert.
- `datentor` ohne Unterbefehl bleibt unverändert. Kein Fall aus Vertrag 3 ändert sich.

Fall:

```json
{"name": "…", "regeln": ["Z6"], "beschreibung": "…",
 "vorgabe": {…},
 "modellzonen": {"an": true, "zonen": [ … ]},
 "anfragen": [{"einheit": {"klasse": "intern"}, "route": "cloud-b", "kunde": null, "projekt": null,
               "kontext": {"nutzer": "n-a", "gruppen": ["g-a"], "jetzt": 1799999900, "grund": "…"}}]}
```

- `vorgabe` ist die Vorgabe aus Vertrag 3, Abschnitt 2, unverändert und mit denselben Regeln für Gültigkeit.
- Pflicht sind `vorgabe`, `modellzonen` und `anfragen`. `name`, `regeln` und `beschreibung` wertet das Programm nicht aus.
- Ungültig (Exit 2) ist der Fall zusätzlich zu Vertrag 3, Abschnitt 5, wenn `modellzonen` ungültig ist (Abschnitt 3), eine Anfrage ein anderes Feld als `einheit`, `route`, `kunde`, `projekt` und `kontext` hat, `einheit` fehlt oder weder Objekt noch `null` ist, `projekt` weder Text noch `null` ist oder `kontext` nicht die Form aus Abschnitt 6 hat.
- Eine Klasse der Einheit, die kein Text ist, ergibt wie in Vertrag 3 den Grund `einheit-ohne-klasse`.

| Pfad | Zweck |
|---|---|
| `faelle_bauen.py` | schreibt `sichtbar/faelle/` und `sichtbar/erwartet/` aus Tabellen mit ausgeschriebenen Erwartungen; prüft jede gegen eine kleine Referenz der Abschnitte 3, 5 und 6. Vorgabe und Option aus prüft es mit der Referenz von Vertrag 3 (`../faelle_bauen.py`, geladen, nicht kopiert). |
| `aussen_laeufer.py` | der Läufer von Vertrag 3 (`../aussen_laeufer.py`, geladen), ruft `DT_BIN zonen` |
| `sichtbar/` | die sichtbaren Fälle |

```
python3 -I -B faelle_bauen.py
DT_BIN=<programm> DT_FAELLE=sichtbar python3 -I -B aussen_laeufer.py [name ...]
```

Die Fälle `z0-aus-*` sind alle gültigen Fälle von Vertrag 3 mit ausgeschalteter Option und ihren Erwartungen von dort (Gegenprobe A127). Ausgenommen ist `d13-tiefe-64`, weil seine Tiefe erst der Fallbauer von Vertrag 3 beim Schreiben erzeugt.
Die Zonen `local`, `conf`, `appr`, `public` und `unsafe` in den Fällen sind Testdaten nach der Vorlage, keine Vorgabe des Produkts.

## 8. Vorgabe für den Bau (nicht durch Außentests geprüft)

```rust
pub enum Klassensatz { Alle, Niedrigste, Keine }
pub enum Bedingung { Immer, Nachrangig, Freigabe, Gesperrt }
pub struct Zone { pub schluessel: String, pub klassen: Klassensatz, pub bedingung: Bedingung, pub routen: Vec<String> }
pub struct ProjektFreigabe { pub route: String, pub kunde: String, pub projekt: String }
pub struct Ausnahme { pub zone: String, pub nutzer: Vec<String>, pub gruppen: Vec<String>,
                      pub gesetzt_von: String, pub zweck: String, pub bis: u64 }
pub enum Ebene { Gruppe, Nutzer }
pub struct Regel { pub ebene: Ebene, pub wer: String, pub zone: String,
                   pub klassen: Option<Klassensatz>, pub gesperrt: bool }
pub struct Zonen { /* privat */ }
impl Zonen {
    pub fn neu(v: &Vorgabe, an: bool, zonen: Vec<Zone>, freigaben: Vec<ProjektFreigabe>,
               ausnahmen: Vec<Ausnahme>, regeln: Vec<Regel>) -> Result<Zonen, ZonenFehler>;  // Abschnitt 3
}
pub struct Kontext<'a> { pub nutzer: Option<&'a str>, pub gruppen: &'a [&'a str],
                         pub jetzt: Option<u64>, pub grund: Option<&'a str> }
pub fn darf_zonen(v: &Vorgabe, z: &Zonen, einheit: Option<&Einheit<'_>>, route: Option<&str>,
                  kunde: Option<&str>, projekt: Option<&str>, k: &Kontext<'_>) -> ZonenAntwort;
pub enum ZonenGrund { Tor(Grund), NutzerFehlt, RouteOhneZone, ZoneDurchRegelGesperrt, ZoneGesperrt,
                      GrundFehlt, KlasseUeberZone, ZoneOhneFreigabe }
pub struct Protokoll { pub zone: Option<String>, pub grund: Option<String>, pub ausnahme: Option<Ausnahme> }
pub struct ZonenAntwort { pub grund: ZonenGrund, pub klasse: Option<String>, pub protokoll: Option<Protokoll> }
impl ZonenAntwort { pub fn darf(&self) -> bool; pub fn meldung(&self, route: Option<&str>) -> String; }
impl ZonenGrund { pub fn code(&self) -> &'static str; }
```

- `Vorgabe`, `darf`, `Antwort` und `Grund` aus Vertrag 3 bleiben mit ihren Signaturen. `Grund` bekommt keine neue Variante; die neuen Gründe stehen in `ZonenGrund`.
- `gesperrt: false` in `Regel` heißt „sperrt nicht“; im Fallformat ist `gesperrt: false` ungültig, damit eine Regel nie nach Entsperren aussieht.
- `darf_zonen` ist rein und löst bei keiner Eingabe eine Panik aus (D15). Bei `an == false` ruft es mit Einheit genau `darf` auf.
- Prüfung des Baus: sichtbare und verdeckte Fälle grün, Vertrag 3 weiter grün, voller Mutationslauf ohne überlebende Mutanten in der Bibliothek (wie Vertrag 3, Abschnitt 7).
- Im Programm steht kein Zonenschlüssel, kein Präfix und kein Klassenname (A29). Prüfbar: Die Zonenschlüssel der Testdaten (`local`, `conf`, `appr`, `public`, `unsafe`) und die Präfixe der Vorlage stehen im Programmcode nicht als Literal.

## 9. Nahtstellen

| Was das Tor braucht | Liefert | Form heute | Was fehlt |
|---|---|---|---|
| Option `modellzonen.an`, Vorgabe `false` auf frischer Installation | Vertrag 2 (Einstellungsdatei) | fehlt in Fassung 1 | neues Feld in einer neuen `fassung`; der Lader setzt `false`, wenn es fehlt |
| Zonen in Vorrangfolge mit Schlüssel, Klassensatz, Bedingung und Routenliste | Vertrag 2 | fehlt (README Vertrag 2, OPEN 4) | Liste `zonen`; Name, Präfix, Farbe und Beschreibung dazu als Anzeigedaten, die das Tor nicht liest |
| Vorlage mit fünf Zonen als Datendatei, ohne Modellnamen | Vertrag 2 (Vorlage, A127) | fehlt | Datei mit Daten; nur der Lader liest sie |
| Freigaben je Route, Kunde und Projekt | Vertrag 2 | `kundendaten_an_cloud.freigaben` ohne Projekt | eigene Liste oder Feld `projekt`; die Projektschlüssel brauchen eine Liste in der Datei oder freie Form |
| Ausnahmen zur Evaluierung mit Nutzern, Gruppen, Admin, Zweck und Frist | Vertrag 2 | fehlt | Liste `ausnahmen`; Frist in der Datei als RFC 3339, der Lader rechnet in Sekunden UTC um |
| Regeln der Gruppen und Nutzer | Vertrag 2 (Regelebenen, A26) | fehlt | Liste `regeln`; die Konsole lehnt eine Lockerung beim Schreiben ab, mit Rückgabecode ungleich 0 |
| Nutzer der Sitzung | Vertrag 1 (Start) | fehlt; Start kennt `route` und `kunde` | Feld `nutzer` in Startanfrage oder Startdatei, vom Kern-Dienst aus der Anmeldung, nie vom Browser |
| Gruppen des Nutzers | Vertrag 1 oder Kern-Dienst | fehlt | Gruppen zum Zeitpunkt des Starts; Herkunft (Einstellungsdatei oder Anmeldung) offen |
| Grund der nachrangigen Wahl | Vertrag 1 (Startanfrage) | fehlt | Feld `grund`, Text oder `null`, aus der Routenwahl der Oberfläche |
| Zeit `jetzt` | Kern-Dienst (Aufrufer des Tors) | fehlt | Uhrzeit beim Start und bei jeder Prüfung, in Sekunden UTC |
| Projekt der Einheit | Vertrag 4 (Suche, Notiz) | fehlt; die Suche reicht nur Klasse und Kunde | Feld `projekt` der Notiz; ein Feld, das kein Text ist, wird `null` (dann keine Kundendaten in Zonen mit `freigabe`) |
| Protokoll mit Zone, Grund und Ausnahme | Vertrag 5 (Protokollzeile) | Grund-Codes in Schlüsselform | Felder für `protokoll`; Text von `grund` und `zweck` sind frei und brauchen eine Längengrenze |

## 10. Offene Fragen

1. **Lockerung im Tor:** Dieser Vertrag macht einen Fall mit lockernder Regel ungültig (fail closed, kein Tor). Die andere Lesart wäre: Das Tor nimmt den strengeren Wert und ignoriert die Lockerung still. Empfehlung: ungültig lassen, weil Vertrag 2 die Lockerung schon beim Schreiben ablehnt und eine still ignorierte Regel die Anzeige der wirksamen Regel (A28) verfälscht.
2. **Ein Schalter für Z7 und D11:** In einer Zone mit `freigabe` ersetzt die Freigabe mit Projekt die Freigabe nach A50. Die strengere Lesart verlangt beide. Empfehlung: ersetzen, wie A128 sagt („derselbe Schalter, ergänzt um das Projekt“); eine Freigabe mit Projekt ist enger als eine ohne.
3. **Nachrangig ohne Prüfung der vorrangigen Zone:** Das Tor verlangt nur einen Grund. Soll es zusätzlich prüfen, ob eine vorrangige Zone die Einheit überhaupt hätte sehen dürfen? Empfehlung: nein, weil „reicht nicht aus“ meist Leistung oder Verfügbarkeit meint, die das Tor nicht kennt.
4. **Nutzer Pflicht:** Mit eingeschalteter Option ist jede Anfrage ohne Nutzer gesperrt (Z1), auch ohne Regeln. Empfehlung: so lassen, weil sonst eine Nutzerregel durch einen fehlenden Nutzer still entfällt.
5. **Zeitformat:** Sekunden seit 1970 UTC als ganze Zahl im Tor. Empfehlung: so lassen; die Einstellungsdatei kann RFC 3339 tragen, der Lader rechnet um.
6. **Firmenregel für einen Nutzer (A26):** Sie bleibt eine Regel der Firma und darf also auch lockern. Dieser Vertrag kennt sie nicht. Soll sie als Zone je Nutzer oder als eigene Ebene kommen?
7. **Klassensatz:** Nur drei Werte (`alle`, `niedrigste`, `keine`). Soll ein Admin auch „bis Klasse X“ setzen können? Dann käme ein vierter Wert mit Klassenschlüssel dazu, ohne bestehenden Fall zu ändern.
