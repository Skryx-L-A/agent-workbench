# Vertrag 6: Entscheidungsstelle, dünner Kern des Durchstichs (Fassung 1)

Stand 20.11.2026. Teil des Durchstichs Variante B (Meilensteinplanung, Abschnitt 5.3: „Entscheidungsstelle dünn“).
Anforderungen: A89 bis A93 (MCP-Schreibaufruf), A105 (Freigabe nur für genau diesen Aufruf), A102 (Hashkette), A12 und A104 (kein Personenname, kein Inhalt im Protokoll), A42 und A43 (Start über eine Route mit Klasse).
Grundlage der Freigabe ist der Entwurf zur Freigabe von MCP-Servern (am 18.11.2026 angenommen), Abschnitt 3 (Ops) und Abschnitt 4 (Audit-Zeile). Wo dieser Vertrag davon abweicht, steht es in Abschnitt 11.
Die Nummer 6 ist vorläufig.

Der Vertrag legt Entscheidungen, Rückgaben und Protokollbytes fest, bevor Code entsteht. Es gibt noch keinen Rust-Code. Die sichtbaren Fälle stehen in `faelle/`; verdeckte Fälle und der Mutationslauf folgen mit dem Bau.
„(V)“ heißt: in diesem Vertrag festgelegt. Alle Namen, Schlüssel und Zahlen in Fällen sind erfunden.

Die Stelle entscheidet nur. Sie führt nichts aus, öffnet keinen Socket, liest keine Datei und keine Uhr. Ihre Schnittstellen zu Start (Vertrag 1), Einstellungen (Vertrag 2) und Protokollkette (Vertrag 5) sind unten festgelegt.

## 1. Schnitt

Der Kern ist eine Bibliothek ohne Ein- und Ausgabe (künftig Crate `stelle`). Er besteht aus zwei Funktionen:

```rust
pub fn laden(einstellungen: &[u8], laden_ms: i64, pruefer: Box<dyn Nachweispruefer>) -> Result<Zustand, Fehler>;
pub fn entscheide(zustand: &Zustand, ereignis: &Ereignis, jetzt_ms: i64) -> Ergebnis;

pub struct Ereignis { pub aufrufer: Aufrufer, pub vorgang: Vorgang }          // Vorgang trägt die Eingabe, Abschnitt 4
pub struct Ergebnis { pub zustand: Zustand, pub antwort: Antwort, pub zeilen: Vec<Protokollzeile>, pub zeit_ms: i64 }   // zeit_ms: wirksame Zeit, Abschnitt 5
pub enum Aufrufer { Kern, Hub, Mensch, Agent, Takt, Unbekannt }               // Abschnitt 3
pub trait Nachweispruefer { fn pruefe(&self, herausforderung: &[u8], nachweis: &str) -> Result<String, ()>; }   // Abschnitt 6
```

- **Reine Funktion.** Bei gleichem Zustand, gleichem Ereignis und gleicher Zeit liefert `entscheide` immer dasselbe. Sie ändert den übergebenen Zustand nicht, sondern gibt einen neuen zurück. Sie löst bei keiner Eingabe eine Panik aus.
- **Zeit.** Die Zeit kommt als Eingabe: `jetzt_ms` sind Millisekunden seit 1970 (V). Kein Fall hängt von der Uhr oder vom Ausführungstag ab. Jeder Fall legt die Zeit je Schritt fest.
- **Der Zustand wird im Durchstich nicht gesichert (E1, V).** Er lebt im Speicher der Stelle. Er enthält die gestarteten Sitzungen, alle Anfragen mit ihrem Zustand (auch die abgeschlossenen, denn die Einmaligkeit hängt daran), den Zähler der Anfragekennungen, `laden_ms`, die höchste gesehene Zeit, die geladene Vorgabe und den Prüfer. Er enthält nie ein Argument, nur dessen sha256. Die Vorschau steht nur so lange im Zustand, wie die Anfrage `offen` ist (höchstens 65 536 Byte); mit der Entscheidung oder dem Ablauf wird sie gelöscht. Im Protokoll steht sie nie.
  **Ein Neustart der Stelle macht alle Sitzungen und Anfragen ungültig (fail closed):** `mcp.einloesen` ergibt `stelle.anfrage_unbekannt`, `mcp.anfragen` ergibt `stelle.sitzung_unbekannt`, die Sitzung muss neu starten (Fall `neustart-macht-alles-ungueltig`). Damit eine Kennung über Neustarts nie doppelt vorkommt, trägt jede Anfragekennung die Ladezeit (Abschnitt 4, `mcp.anfragen`).
- **Antwort und Zeilen gelten zusammen (E2).** Erst die Zeilen, dann Zustand und Antwort: Die Hülle um den Kern schreibt die Zeilen eines Ereignisses (mit `sync`) in die Kette, bevor sie den neuen Zustand übernimmt und antwortet. Die Regeln der Hülle stehen in Abschnitt 7.6.
- **Transportschicht.** Sie liegt außerhalb des Kerns. Sie misst den Aufrufer (Abschnitt 3), liest die Einstellungsdatei aus ihrer eigenen Konfiguration, liest die Uhr, schreibt Zeilen und Zustand und spricht die Sockets. Der Kern kennt von alledem nichts.
- **Laden.** `laden` prüft die Einstellungsdatei: Vertrag 2 (Stufen 1 bis 17) und den Block `mcp` (Abschnitt 2). Ist sie ungültig, entsteht kein Zustand und die Stelle startet nicht. Sie lehnt dann nicht jede Anfrage ab, sondern läuft gar nicht (A42, fail closed). `laden` schreibt keine Zeile.
  `laden_ms` ist die Ladezeit (Ganzzahl 0 bis 253 402 300 799 999, sonst `stelle.zeit_ungueltig`); `laden` merkt sie im Zustand und setzt die höchste gesehene Zeit auf `laden_ms`. Die Fälle laden mit einem festen `laden_ms` (`laden_ms` im Fall).

## 2. Einstellungen und Zuordnung der Werkzeuge

Die Stelle liest dieselbe Einstellungsdatei wie Start, Suche und Tor (Vertrag 2, A44). Sie hat keine zweite Datei.
Für den MCP-Teil kündigt das Projekt eine **Fassung 2 von Vertrag 2** an (angekündigt, noch nicht geschrieben): Schlüssel `mcp` mit je Server zwei Listen von Werkzeugnamen, `lesen` und `schreiben`. Dieser Vertrag legt die Form auf diese Ankündigung fest. Gilt später etwas anderes, gilt Vertrag 2.

```json
{"fassung": 2, "klassen": ["oeffentlich", "intern", "kundendaten"], "vorgabestufe": "kundendaten",
 "routen": [{"schluessel": "cloud-beispiel", "art": "cloud", "klasse": "intern"},
            {"schluessel": "lokal-beispiel", "art": "lokal", "klasse": "kundendaten"}],
 "kunden": [{"schluessel": "k-beispiel", "erlaubt": ["lokal-beispiel"], "verboten": []}],
 "kundendaten_an_cloud": {"vorgabe": "nie", "freigaben": []},
 "mcp": {"tickets":  {"lesen": ["get"], "schreiben": ["update"], "freigabe": true, "frist_ms": 60000, "einloesefrist_ms": 30000},
         "notizen":  {"lesen": [], "schreiben": ["anlegen"], "freigabe": false},
         "such-mcp": {"lesen": ["brain_suche"], "schreiben": []}}}
```

| Feld in `mcp.<server>` | Form | Bedeutung |
|---|---|---|
| Schlüssel des Servers | Kennung (`^[a-z0-9][a-z0-9._-]{0,63}$`) | nie ein Anzeigename; für den Such-Server von Vault ist es `such-mcp` |
| `lesen`, `schreiben` | Liste von Werkzeugnamen, Pflicht | Werkzeugname: `^[A-Za-z0-9][A-Za-z0-9._-]{0,127}$`; ohne Doppel; kein Name in beiden Listen |
| `freigabe` | Wahrheitswert, Vorgabe `true` | Option „Freigabe je Schreibaufruf“ für diesen Server (A90, Vorgabe an) |
| `frist_ms` | Ganzzahl 1 bis 86 400 000, Vorgabe 60 000 | längste Frist für die Entscheidung eines Menschen (A90, vom Admin) |
| `einloesefrist_ms` | Ganzzahl 1 bis 86 400 000, Vorgabe 300 000 | Frist zum Einlösen nach der Erteilung. Die Vorgabe ist 5 min; ein Admin darf bis 24 h erhöhen (Befund B15, Entscheidung vom 23.11.2026). Der Schein des Halters bildet die eingestellte Einlösefrist ab; die Form legt der Halter-Vertrag fest (`rust/spec/halter/README.md`, Felder `ausgestellt` und `ablauf`). Stellt die Stelle einen MCP-Schein aus, ist `ausgestellt` der Zeitpunkt der Erteilung und `ablauf` genau `einloesen_bis_ms` der Erteilung, also `ausgestellt` plus `einloesefrist_ms` des Servers. Die Stelle ist die einzige Quelle dieser Frist; der Halter liest die Einstellung nicht. alt: höchstens 300 000, Vorgabe 60 000 (Entscheidung vom 22.11.2026) |

- Die Form ist geschlossen. **Fassung 2 von Vertrag 2 (vereinbart am 20.11., geschrieben mit Beginn von B):** Je Server `freigabe` (Wahrheitswert, Vorgabe `true`), `frist_ms` (Ganzzahl 1 bis 86 400 000) und `einloesefrist_ms` (Ganzzahl 1 bis 86 400 000, Vorgabe 300 000, B15). Es gibt keinen Sammelcode für Fehler im Block `mcp`; sie fallen in die vorhandenen Stufen von Vertrag 2, in der Reihenfolge von Vertrag 2 über die ganze Datei:
  `einst.schluessel_unbekannt` (ein Schlüssel außerhalb der Form, auch `mcp` in einer Datei der Fassung 1), `einst.schluessel_fehlt` (`lesen` oder `schreiben` fehlt), `einst.typ_falsch` (falscher Typ, auch eine Frist, die keine Ganzzahl ist), `einst.anzahl` (mehr als 64 Server oder mehr als 256 Werkzeuge in einer Liste; die Grenzen sind ein Vorschlag dieses Vertrags), `einst.kennung_form` (Server oder Werkzeug verletzt die Form), `einst.kennung_doppelt` (ein Name doppelt in einer Liste),
  dazu zwei neue Codes nach den bisherigen Stufen: `einst.frist_bereich` (`frist_ms` außerhalb von 1 bis 86 400 000 oder `einloesefrist_ms` außerhalb von 1 bis 86 400 000) und `einst.mcp_doppelt` (ein Werkzeug in `lesen` und `schreiben` zugleich). Die festen Gründe der beiden neuen Codes (`Frist ausserhalb des Bereichs`, `Werkzeug in lesen und schreiben`) sind ein Vorschlag. Ohne `mcp` gilt jeder Server als unbekannt (Abschnitt 4). Je ein Fall zeigt `einst.mcp_doppelt`, `einst.frist_bereich`, `einst.schluessel_unbekannt`, `einst.typ_falsch` und `einst.anzahl`.
- **Ein Werkzeug ohne Zuordnung gilt als schreibend (A89, V).** Als lesend gilt nur ein Name in `lesen`. Das ist die einzige Regel; sie steht nirgends in der Transportschicht.
- Die Stelle ordnet zu, nicht der Hub. Ein Hub, der „lesend“ behauptet, entscheidet nichts. Der Hub darf dieselbe Zuordnung lesen, um zu wissen, ob er eine Vorschau bauen muss; verbindlich ist die Antwort der Stelle.
- Der Server `such-mcp` mit Werkzeug `brain_suche` steht in `lesen` (Projektentscheid, 20.11.2026). Er geht im Betrieb direkt an den Socket der Sitzung und nicht durch den Hub; der Fall `lesendes-werkzeug-frei` zeigt nur, dass er als lesend gilt und keine Anfrage entsteht.

## 3. Aufrufer

**Wer etwas aufruft, sagt die Transportschicht, nie ein Feld der Anfrage (V).** Sie misst ihn an der Verbindung (Kennung am Unix-Socket, Client am Named Pipe; Plattformschicht, Plan Abschnitt 4) und übergibt ihn dem Kern als `Aufrufer`.
Eine Eingabe mit einem Feld, das die Tabelle in Abschnitt 4 nicht nennt (etwa `aufrufer`), ist nicht geschlossen und wird abgelehnt (Abschnitt 4, Stufe 3; Fall `anfragen-gruende`).

| Art | Wer das ist | darf |
|---|---|---|
| `kern` | der Kern-Dienst der Sitzungen | `sitzung.starten` |
| `hub` | der MCP-Hub der Sitzung (eigener Prozess außerhalb des Einschlusses) | `mcp.anfragen`, `mcp.einloesen`, `mcp.stand` |
| `mensch` | die Brücke der Oberfläche (Kern-Dienst im Browserweg, außerhalb des Einschlusses) für den angemeldeten Menschen | `mcp.entscheiden`, `mcp.offene` |
| `agent` | jeder Prozess im Einschluss eines Laufs, auch ein Worker | nichts |
| `takt` | der Taktgeber der Stelle selbst | `ablauf.pruefen` |
| `unbekannt` | Messung gescheitert oder nicht zuzuordnen | nichts |

- **Jeder Vorgang hat genau eine erlaubte Art (V).** Der Hub kann nie entscheiden, ein Mensch kann nicht einlösen, ein Agent oder Worker kann nie freigeben (A92). Eine Art ist nie eine Obermenge einer anderen.
- Die Art `mensch` allein genügt nicht für eine Freigabe. Dazu kommt der Nachweis mit Signatur (Abschnitt 6). Es sind zwei getrennte Sperren: Gelingt es einem Agenten, als `mensch` zu erscheinen, fehlt ihm der Schlüssel für den Nachweis (Abschnitt 6).
- **Der Aufrufer steht im Protokoll als feste Art** (`vorgang.abgelehnt`, Abschnitt 7), nie als Name.
- Wie die Plattformschicht die Arten misst, gehört nicht in diesen Vertrag (Grundsatz 8: Die Messung liefert sie als getypten Wert, die Entscheidung trifft die Stelle).

## 4. Vorgänge

Es gibt sieben Vorgänge in zwei Gruppen des Durchstichs: (a) Sitzung starten, (b) Freigabe je MCP-Schreibaufruf mit Anfragen, Offene lesen, Stand fragen, Entscheiden, Einlösen und Ablauf.

| Vorgang | Aufrufer | Eingabe (geschlossen) | Antwort bei Erfolg |
|---|---|---|---|
| `sitzung.starten` | `kern` | `{"start": <Bytes der Startdatei>}`, dazu `orchestrator` und `zusage` nach Abschnitt 15.2; im Fall als `{"text": …}` oder `{"b64": …}` | `{"ok":true,"ergebnis":"gestartet","sitzung":…,"route":…,"route_art":"cloud"\|"lokal"}` |
| `mcp.anfragen` | `hub` | `server`, `werkzeug`, `argumente`, `vorschau`, `frist_ms`, `sitzung`, `person` | lesend: `{"ok":true,"ergebnis":"lesend"}`; Option aus: `{"ok":true,"ergebnis":"ohne_freigabe","anfrage":…,"digest":…}`; sonst `{"ok":true,"ergebnis":"offen","anfrage":…,"digest":…,"frist_bis_ms":…,"vorschau_sha256":…}` |
| `mcp.entscheiden` | `mensch` | `anfrage`, `entscheidung` (`erteilen` oder `ablehnen`), `nachweis` | erteilt: `{"ok":true,"ergebnis":"erteilt","anfrage":…,"einloesen_bis_ms":…}`; abgelehnt: `{"ok":true,"ergebnis":"abgelehnt","anfrage":…}` |
| `mcp.einloesen` | `hub` | `anfrage`, `server`, `werkzeug`, `argumente` | `{"ok":true,"ergebnis":"eingeloest","anfrage":…,"digest":…}` |
| `mcp.offene` | `mensch` | `{}` | `{"ok":true,"ergebnis":"offene","offene":[{"anfrage":…,"server":…,"werkzeug":…,"vorschau":…,"vorschau_sha256":…,"digest":…,"frist_bis_ms":…}]}`, nach Kennung (der Zahl n) sortiert; lesend, keine Zeile im Protokoll |
| `mcp.stand` | `hub` | `anfrage` | `{"ok":true,"ergebnis":"stand","anfrage":…,"zustand":"offen"\|"erteilt"\|"abgelehnt"\|"abgelaufen"\|"eingeloest","frist_bis_ms":…}`; `frist_bis_ms` ist das Ende der laufenden Frist, in den Endzuständen `null`; lesend |
| `ablauf.pruefen` | `takt` | `{}` | `{"ok":true,"ergebnis":"geprueft","abgelaufen":<Zahl>}` |

Jede Ablehnung hat die Form `{"ok":false,"code":"<code>","grund":"<fester ASCII-Text>"}`. Die Codes stehen in Abschnitt 8.

### Prüfreihenfolge

Jeder Vorgang durchläuft dieselben Stufen in fester Reihenfolge. Die erste verletzte Stufe nennt den Code.

| Stufe | Prüfung | Code |
|---|---|---|
| 0 | `jetzt_ms` ist eine Ganzzahl von 0 bis 253 402 300 799 999 | `stelle.zeit_ungueltig` (**keine** Zeile, keine Änderung; die einzige Ablehnung ohne Zeile) |
| 1 | Fristen abarbeiten (Abschnitt 5); das ist keine Ablehnung und läuft bei jedem Ereignis | – |
| 2 | Der Aufrufer ist die erlaubte Art des Vorgangs (Abschnitt 3) | `stelle.aufrufer_nicht_erlaubt` |
| 3 | Die Eingabe hat genau die Felder der Tabelle, jedes mit Typ und Form | `stelle.eingabe_ungueltig` |
| 4 | die Stufen des Vorgangs, unten | siehe unten |

Stufe 3 im Einzelnen (V): `server` und `sitzung` sind Kennungen; `werkzeug` ein Werkzeugname; `argumente` ein JSON-Objekt, nach RFC 8785 höchstens 1 048 576 Byte; `vorschau` Text mit höchstens 65 536 Byte (UTF-8) oder `null`;
`frist_ms` Ganzzahl 1 bis 86 400 000; `person` ein Pseudonym `^ps_[0-9a-f]{32}$`; `anfrage` `^mf-[0-9]{1,15}-[1-9][0-9]{0,9}$`; `nachweis` Text mit 1 bis 4096 Zeichen. Eine Zahl wie `1.0` ist keine Ganzzahl.

**`sitzung.starten`, Stufe 4.** Die Stelle prüft die Startdatei mit derselben Bibliothek wie Kern-Dienst und Startprogramm (`start::pruefen`, Vertrag 1, Stufen 1 bis 14) und gibt deren Code unverändert zurück (Projektentscheid, 20.11.2026). Sie hat keine zweite Prüfregel.
Sie bekommt die **Startdatei** mit `sitzung`, die der Kern-Dienst vergeben hat, nicht die Startanfrage. Danach gilt als letzte Stufe: Die Kennung `sitzung` steht noch nicht im Zustand, sonst `stelle.sitzung_vergeben`.
Bei Erfolg steht die Sitzung (Route, Kunde, Harness) im Zustand. Es gibt in Fassung 1 kein Sitzungsende; eine Kennung bleibt belegt.

**`mcp.anfragen`, Stufe 4**, in dieser Reihenfolge:

1. `server` steht nicht in `mcp` der Einstellungen: `stelle.server_unbekannt`.
2. `sitzung` steht nicht unter den gestarteten Sitzungen: `stelle.sitzung_unbekannt`.
3. Das Werkzeug steht in `lesen`: Antwort `lesend`, **keine** Anfrage, keine Zeile, der Zähler bleibt (A89). Die Stufen 4 bis 6 entfallen, auch eine zu große `frist_ms` stört nicht.
4. Das Werkzeug ist schreibend (in `schreiben` oder ohne Zuordnung) und `freigabe` des Servers ist `false`: Die Anfrage bekommt die nächste Kennung, die Zeile `mcp.schreibaufruf` mit Ergebnis `ohne_freigabe` entsteht, die Antwort heißt `ohne_freigabe`. Der Hub darf ausführen (Option aus, O3 des Entwurfs). Es entsteht kein Eintrag, der sich einlösen ließe.
5. `frist_ms` ist größer als `frist_ms` des Servers: `stelle.frist_ungueltig`. Der Hub darf die Frist verkürzen, nie verlängern.
6. Sonst entsteht die Anfrage mit der Kennung `mf-<laden_ms>-<n>` (`laden_ms` aus dem Zustand; n zählt ab 1, nur geschriebene Anfragen zählen) im Zustand `offen`. Frist bis `jetzt_ms + frist_ms`. Der Zustand speichert `digest`, `vorschau_sha256` und die Person; die Stelle berechnet `digest` und `vorschau_sha256` selbst aus der Eingabe.

**Grenze des Zählers (21.11.).** Die Form der Kennung lässt für `n` höchstens zehn Ziffern zu (`[1-9][0-9]{0,9}`, Abschnitt 4 oben), also höchstens 9 999 999 999. Steht der Zähler dort, lehnt die Stelle nach Punkt 3 und vor Punkt 4 jede weitere Anfrage mit `stelle.anfragen_erschoepft` ab: keine Kennung, kein neuer Eintrag, der Zähler bleibt, die Zeile ist `vorgang.abgelehnt` wie bei jeder Ablehnung. Ein lesendes Werkzeug (Punkt 3) braucht keine Kennung und antwortet weiter. Erst ein Neustart mit neuem `laden_ms` gibt wieder Kennungen (E1). Fail closed: Eine Kennung außerhalb ihrer Form wäre für `mcp.entscheiden`, `mcp.einloesen` und `mcp.stand` nie erreichbar. Kein sichtbarer Fall (er bräuchte 10^10 Anfragen); geprüft in der Bibliothek. Den Grund `Kennungen erschoepft, Neustart noetig` kennen seit 23.11.2026 auch beide Vergleichsimplementierungen (Befund B9): Sobald die Grenze erreichbar ist, lässt sich der Fall bauen. Die Grenze selbst bleibt dort außen vor, weil ein Fall 10^10 Anfragen fahren müsste.

**`mcp.entscheiden`, Stufe 4**, in dieser Reihenfolge:

1. `anfrage` steht nicht im Zustand: `stelle.anfrage_unbekannt`.
2. Zustand `abgelaufen`: `stelle.anfrage_abgelaufen`. Zustand `erteilt`, `abgelehnt` oder `eingeloest`: `stelle.bereits_entschieden`.
3. Der Nachweis besteht die Prüfung (Abschnitt 6): sonst `stelle.nachweis_ungueltig`. Die Anfrage bleibt dabei `offen`.
4. `ablehnen`: Zustand `abgelehnt`, Zeile `mcp.schreibaufruf` mit Ergebnis `abgelehnt`. `erteilen`: Zustand `erteilt`, Frist bis `jetzt_ms + einloesefrist_ms` des Servers, keine Zeile. Der Entscheidende ist das Pseudonym aus dem Nachweis.

**`mcp.einloesen`, Stufe 4**, in dieser Reihenfolge:

1. `anfrage` steht nicht im Zustand: `stelle.anfrage_unbekannt`.
2. Zustand `offen`: `stelle.nicht_freigegeben`; `abgelehnt`: `stelle.anfrage_abgelehnt`; `abgelaufen`: `stelle.anfrage_abgelaufen`; `eingeloest`: `stelle.bereits_eingeloest`.
3. Der `digest` aus `server`, `werkzeug` und `argumente` dieses Aufrufs ist nicht der gespeicherte: `stelle.digest_abweichend`. Die Freigabe bleibt gültig und lässt sich mit dem richtigen Aufruf noch einlösen.
4. Sonst Zustand `eingeloest`, Zeile `mcp.schreibaufruf` mit Ergebnis `eingeloest`. Die Antwort gibt dem Hub die Ausführung frei, genau einmal.

**`mcp.offene`, Stufe 4:** nichts weiter. Die Antwort nennt die Anfragen im Zustand `offen`, nach der Fristarbeit von Stufe 1 (eine abgelaufene steht nicht mehr darin; die Zeile `abgelaufen` entsteht dabei wie bei jedem Ereignis). Der Aufruf ändert sonst nichts und schreibt keine Zeile. Die Oberfläche zeigt die Vorschau aus dieser Antwort.

**`mcp.stand`, Stufe 4:** `anfrage` steht nicht im Zustand: `stelle.anfrage_unbekannt`. Sonst nennt die Antwort den Zustand nach der Fristarbeit von Stufe 1. Der Hub muss damit `mcp.einloesen` nicht wiederholen, um zu erfahren, ob eine Freigabe schon da ist. Ein **erfolgreicher** Aufruf schreibt keine Zeile (außer den Zeilen `abgelaufen` der Stufe 1); jede Ablehnung schreibt `vorgang.abgelehnt` wie bei allen Vorgängen.

**`ablauf.pruefen`, Stufe 4:** keine weitere Prüfung. Die Antwort `geprueft` nennt, wie viele Anfragen Stufe 1 dieses Ereignisses ablaufen ließ.

## 5. Zustandsmaschine, Fristen und Zeit

```
offen ──erteilen──> erteilt ──einloesen──> eingeloest
  │                    │
  ├──ablehnen──> abgelehnt
  └──Frist──> abgelaufen <──Frist──┘
```

- Endzustände sind `abgelehnt`, `eingeloest` und `abgelaufen`. Sie ändern sich nie mehr.
- **Frist (V).** Eine Anfrage `offen` hat die Frist `anfrage + frist_ms`, eine `erteilt` die Frist `erteilung + einloesefrist_ms`. Sie gilt, solange `jetzt_ms` **kleiner** als das Fristende ist. Zum Fristende selbst ist sie abgelaufen; eine Millisekunde davor gilt sie noch (Fall `frist-grenzen`).
- **Ablauf beim nächsten Zugriff und durch den Taktgeber (V).** Stufe 1 setzt jede Anfrage, deren Frist verstrichen ist, auf `abgelaufen` und schreibt für jede eine Zeile `mcp.schreibaufruf` mit Ergebnis `abgelaufen`, in aufsteigender Reihenfolge der Kennung (nach der Zahl), vor der Zeile des eigentlichen Vorgangs.
  Das gilt für jedes Ereignis, auch für ein abgelehntes. Der Vorgang `ablauf.pruefen` sorgt dafür, dass die Zeile auch dann entsteht, wenn niemand die Stelle anspricht (A90, A91). Eine Entscheidung oder ein Einlösen nach dem Fristende sieht also eine abgelaufene Anfrage und wird mit `stelle.anfrage_abgelaufen` abgelehnt; die Zeile `abgelaufen` steht davor.
- **Die Zeit geht nie zurück (V).** Der Zustand merkt sich die höchste gesehene Zeit. Ein Ereignis mit kleinerer `jetzt_ms` gilt mit der höchsten. Wird die Uhr zurückgestellt, lebt keine Frist wieder auf, und neue Fristen beginnen nicht in der Vergangenheit (Fall `uhr-geht-zurueck`).
- Die Einlösefrist beginnt mit der Erteilung und ist unabhängig von der Frist der Entscheidung. Der Mensch kann also bis zum Ende der Entscheidungsfrist erteilen und der Hub danach noch `einloesefrist_ms` einlösen.
- Eine Begrenzung der offenen Anfragen und das Löschen alter Endzustände gehören nicht zum Durchstich (Abschnitt 10).

## 6. Bindung an genau diesen Aufruf und Nachweis des Menschen

**`digest` (V, A105).** sha256 über die Bytes (UTF-8) der Form RFC 8785 von `{"argumente": …, "server": …, "werkzeug": …}`, kleine Hexziffern. Die Stelle berechnet ihn bei `mcp.anfragen` und bei `mcp.einloesen` selbst aus den übergebenen Werten; ein `digest` des Hubs zählt nie.
Eine Freigabe gilt für genau diesen `digest`. Ändert sich ein Argument, ein Werkzeug oder der Server, entsteht eine neue Anfrage.

**`vorschau_sha256`.** sha256 der UTF-8-Bytes der Vorschau, oder `null`, wenn der Hub keine Vorschau liefert (der Server hat keinen lesbaren Vorher-Stand, O2 des Entwurfs). Die Stelle rechnet ihn selbst. Sie speichert die Vorschau nur, solange die Anfrage offen ist, damit `mcp.offene` sie der Brücke zeigen kann.

**Herausforderung.** Der Nachweis des Menschen bezieht sich auf diese Bytes: RFC 8785 von
`{"anfrage": <Kennung>, "art": "mcp.entscheidung", "digest": <digest>, "entscheidung": "erteilen"|"ablehnen", "vorschau_sha256": <Hex oder null>}`.
Damit bindet der Nachweis den Aufruf, die Entscheidung und den Stand der Vorschau. Die Brücke rechnet `vorschau_sha256` selbst über genau die Bytes, die sie der Ansicht geschickt hat; die Ansicht ist keine Sicherheitsgrenze. Ein Nachweis für `erteilen` gilt nie für `ablehnen`.

**Schnittstelle des Prüfers.** `pruefe(herausforderung, nachweis) -> Ok(Pseudonym) | Err`. Der Kern ruft ihn genau einmal je `mcp.entscheiden`, nach den Stufen 0 bis 3 und den Zustandsprüfungen. `Ok` liefert den Entscheidenden als Pseudonym `^ps_[0-9a-f]{32}$`; liefert der Prüfer etwas anderes, gilt `Err`. Jedes `Err` ergibt `stelle.nachweis_ungueltig`.
Der Nachweis ist für **Erteilen und Ablehnen** nötig (Annahme, Abschnitt 11). Eine gültige Art `mensch` ohne Nachweis genügt nie. Das Pseudonym kommt aus dem Nachweis, nicht aus der Verbindung. **Ein Nachweis gilt nur einmal (V):** Er bindet die Anfragekennung, und `mcp.entscheiden` geht nur aus `offen`; nach der ersten Entscheidung ist jeder weitere Nachweis für diese Anfrage wirkungslos (`stelle.bereits_entschieden`).

**Prüfer `h1` (E3, echt im Durchstich).** Der Nachweis ist der Text `h1:<Pseudonym>:<hex>`. `hex` sind 64 kleine Hexziffern: HMAC-SHA256 mit dem Schlüssel (32 Byte) über die Bytes `Pseudonym`, LF (0x0A) und die Bytes der Herausforderung (UTF-8).
`pruefe` rechnet den HMAC mit demselben Schlüssel nach und vergleicht in konstanter Zeit. Er gibt das Pseudonym aus dem Text zurück, wenn der Vergleich gelingt, sonst `Err`; jede andere Form des Textes (Großbuchstaben, anderes Präfix, falsche Längen) ist `Err`.

**Schlüssel (V).** 32 Byte, in einer Datei mit Modus 0600 außerhalb des Einschlusses; der Pfad steht in der Installationskonfiguration von Stelle und Brücke. Der Schlüssel geht nie an die Ansicht, nie ins Protokoll und nie über einen Socket.

| Was im Durchstich | Stand |
|---|---|
| Aufbau von Herausforderung und `digest` | echt |
| Prüfer `h1` mit gemeinsamem Schlüssel | **echt** |
| Gerätesignatur (Passkey mit Anwesenheit oder Gerätesignatur nach dem Vorbild des Handy-Kerns, A92, A105) | später; sie ersetzt `h1` hinter derselben Schnittstelle |

**Ehrlich: Im Durchstich vertraut die Stelle der Brücke.** Den Schlüssel kennen nur die Stelle und die Brücke der Oberfläche (der Kern-Dienst im Browserweg, außerhalb des Einschlusses; die Schlüsseldatei ist nur für deren Dienstkonten lesbar). Die Brücke stellt den Nachweis nur nach einem Klick des angemeldeten Menschen in der Ansicht aus.
**Der Nachweis `h1` belegt „Klick in der Ansicht mit gültigem Token der Brücke“, nicht eine bestimmte Person.** Im Durchstich gibt es keine Anmeldung. Die Brücke trägt genau ein Pseudonym aus ihrer eigenen Installationskonfiguration; die Ansicht setzt es nie. Mehrere Menschen je Brücke gibt es erst mit Anmeldung oder Gerätesignatur.
Ein Agent kann nicht freigeben, weil er weder den Schlüssel kennt noch die Brücke erreicht (Einschluss, Netzgrenze). Eine Brücke, die den Nachweis ohne Klick ausstellt, kann jede Freigabe fälschen; das fängt der Durchstich nicht ab.
**Voraussetzung:** Der Agent erreicht 127.0.0.1 des Rechners nicht (Gegenprobe zu S2 in `rust/spec/vorfuehrung/DREHBUCH.md`). Ohne sie könnte er die Brücke selbst ansprechen und klicken. Die Art `mensch` in Abschnitt 3 ist die Verbindung der Brücke.
Der Schlüssel in den Fällen (32 Byte 0x01, im Fall als `pruefer.schluessel_hex`) ist ein Testschlüssel und gehört nie in den Betrieb. Ein Prüfer mit festem Schlüssel im Quelltext ist ein Fehler.

**Wer der Menschen-Nachweis nicht ist.** Ein geerbter Nachweis gilt nicht (A92): Der Nachweis gilt nur für die Herausforderung dieser Anfrage. Der Hub sieht den Token des Menschen nie (A93); das ist Sache von Halter und Hub, nicht der Stelle.

## 7. Protokoll

Die Stelle schreibt in die Kette der Quelle `stelle`: Datei `stelle-<JJJJ-MM>.jsonl`, den Monat bestimmt `zeit` (Vertrag 5, Abschnitt 1). Format, Kette (`vorher` als sha256 der Rohbytes der Vorzeile, 64 Nullen am Anfang), RFC 8785 und Zeilenende LF sind die von Vertrag 5.
Alle Zeilen sind Fassung 2, geschlossen, und tragen `fassung` 2, `vorher` und `zeit` (UTC mit Millisekunden, `jetzt_ms` nach Abschnitt 5). Es gibt vier Zeilenarten.
Jeder Wert ist ein Schlüssel nach `^[a-z0-9][a-z0-9._-]{0,63}$`, ein Werkzeugname, ein Pseudonym, ein sha256 oder eine Zeit; es braucht kein Escape.

**Nie in einer Zeile (A12, A104, V):** ein Argument, eine Vorschau, ein Personenname, ein Pfad, ein Wert aus einer Anfrage, der nicht seine Form besteht. Die Person steht nur als Pseudonym, ein Argument nur als `digest`.
Ein Wert, der die Form nicht besteht oder fehlt, steht als `null` (Fall `start-kennung-schreibt-null`).

### 7.1 `mcp.schreibaufruf`, eine Zeile je Anfrage am Ende (A91)

```
{"aktionsname":"mcp.schreibaufruf","anfrage":"mf-1795179540000-1","digest":"34b94b40…ba4ced","entscheidend":"ps_00000000000000000000000000000a01","ergebnis":"eingeloest","fassung":2,"person":"ps_00000000000000000000000000000001","server":"tickets","sitzung":"s-1","vorher":"e2bbd239…8070e1","werkzeug":"update","zeit":"2026-11-20T13:00:08.000Z","zeit_anfrage":"2026-11-20T13:00:01.000Z","zeit_entscheidung":"2026-11-20T13:00:05.000Z"}
```

| Feld | Wert |
|---|---|
| `anfrage` | Kennung `mf-<laden_ms>-<n>` |
| `digest` | sha256 nach Abschnitt 6 |
| `entscheidend` | Pseudonym des Menschen, der entschieden hat (erteilt oder abgelehnt); `null` bei `ohne_freigabe` und bei `abgelaufen` ohne Entscheidung |
| `ergebnis` | `eingeloest`, `abgelehnt`, `abgelaufen` oder `ohne_freigabe` |
| `person` | Pseudonym der Person, deren Lauf die Anfrage stellte (vom Hub, Abschnitt 11) |
| `server`, `werkzeug`, `sitzung` | Schlüssel, Name, Kennung |
| `zeit_anfrage` | Zeit der Anfrage |
| `zeit_entscheidung` | Zeit der Entscheidung des Menschen; `null`, wenn keine stattfand |
| `zeit` | Zeit der Zeile, also des Endzustands |

Geschlossene Form: genau diese Felder und `aktionsname`, `fassung`, `vorher`, `zeit`. Zusammenhänge, die der Prüfer der Fälle prüft: `entscheidend` und `zeit_entscheidung` sind beide null oder beide gesetzt; `eingeloest` und `abgelehnt` haben einen Entscheidenden; `ohne_freigabe` hat keinen.
Ein `abgelaufen` nach Erteilung nennt den Entscheidenden, einer vor der Entscheidung nicht. Das ist der Unterschied zwischen „der Mensch hat freigegeben, der Hub hat nicht eingelöst“ und „niemand hat entschieden“.
Je Anfrage steht genau eine Zeile. Offen und erteilt schreiben keine (Abschnitt 12, Punkt 1).

### 7.2 `sitzung.gestartet` und `sitzung.abgelehnt`

```
{"aktionsname":"sitzung.gestartet","ergebnis":"gestartet","fassung":2,"grund":null,"harness":"claude","kunde":"k-beispiel","route":"cloud-beispiel","sitzung":"s-1","vorher":"0000…0000","zeit":"2026-11-20T13:00:00.000Z"}
{"aktionsname":"sitzung.abgelehnt","ergebnis":"abgelehnt","fassung":2,"grund":"start.route_unbekannt","harness":"claude","kunde":"k-beispiel","route":"cloud-fremd","sitzung":"s-1","vorher":"0000…0000","zeit":"2026-11-20T13:00:00.000Z"}
```

`grund` ist `null` bei `gestartet`, sonst der Code der Ablehnung (`start.*` aus Vertrag 1 oder ein `stelle.*`-Code, auch bei falschem Aufrufer oder ungültiger Eingabe). `harness`, `kunde`, `route` und `sitzung` stehen mit dem Wert aus der Startdatei, wenn sie lesbar ist und der Wert die Kennungsform besteht; sonst `null`.
Bei falschem Aufrufer, ungültiger Eingabe, zu großer oder nicht lesbarer Startdatei sind alle vier `null`. Der Aufrufer steht bei einer Startablehnung nicht in der Zeile; die Stelle erfährt ihn nur als Code.

### 7.3 `vorgang.abgelehnt`, jede Ablehnung der MCP-Vorgänge und des Ablaufs

```
{"aktionsname":"vorgang.abgelehnt","anfrage":"mf-1795179540000-1","aufrufer":"hub","ergebnis":"abgelehnt","fassung":2,"grund":"stelle.bereits_eingeloest","vorgang":"mcp.einloesen","vorher":"a3874f23…2401","zeit":"2026-11-20T13:00:04.000Z"}
```

`anfrage`: die Kennung aus der Eingabe, wenn sie die Form besteht, sonst `null` (sie wird nicht gegen den Zustand geprüft). `aufrufer`: die Art aus Abschnitt 3. `grund`: der Code. `vorgang`: der Name aus Abschnitt 4.
Dieser Vertrag verlangt (V): **Jede Ablehnung steht im Protokoll** (A92: „Protokollzeilen der Ablehnungen“), ausgenommen die Zeit der Stufe 0. Die Begrenzung bei Flut und das Zusammenfassen mit `anzahl` kommen später (Abschnitt 10).
Eine Ablehnung der Vorgänge `sitzung.starten` heißt immer `sitzung.abgelehnt`, nie `vorgang.abgelehnt`.

### 7.4 Reihenfolge der Zeilen in einem Ereignis

Zuerst die Zeilen `abgelaufen` der Stufe 1 (aufsteigend nach Kennung), dann die Zeile des Vorgangs. Ein Ereignis schreibt höchstens eine Zeile des Vorgangs selbst.

### 7.5 Bibliothek

Die Zeilen entstehen über die Kette aus Vertrag 5 (Crate `formate`, Modul `kette`). Die vier Zeilenarten dieses Abschnitts stehen seit dem 23.11.2026 in Vertrag 5, Abschnitt 6.8 (`rust/spec/protokollzeile/README.md`): Dort steht, was der Prüfer der Kette von ihrer Form verlangt.
`kette pruefe` und `kette pruefe-quelle` prüfen sie darum nicht mehr nur als Kettenglieder der Fassung 2 (Kopf, Kanonik, `vorher`), sondern auch auf ihre geschlossene Form; `anhaengen_zeile` und `anhaengen_zeilen` lehnen eine abweichende Zeile ab. Die sichtbaren Fälle dazu stehen in `rust/spec/protokollzeile/faelle/stellezeilen.json`, die Zeilenprüfer der Fälle dieses Vertrags weiter in `bau-faelle.mjs` (Abschnitt 9).

## 8. Fehlercodes

| Code | Grund (fest, ASCII) | Stufe |
|---|---|---|
| `stelle.zeit_ungueltig` | `Zeit ungueltig` | 0 |
| `stelle.aufrufer_nicht_erlaubt` | `Aufrufer nicht erlaubt` | 2 |
| `stelle.eingabe_ungueltig` | `Eingabe ungueltig` | 3 |
| `start.*` (Vertrag 1, Stufen 1 bis 14, ohne `start.sitzung_nicht_erlaubt`) | Grund aus Vertrag 1 | 4 |
| `stelle.sitzung_vergeben` | `Sitzung schon vergeben` | 4 |
| `stelle.server_unbekannt` | `Server unbekannt` | 4 |
| `stelle.sitzung_unbekannt` | `Sitzung unbekannt` | 4 |
| `stelle.frist_ungueltig` | `Frist ausserhalb der Vorgabe` | 4 |
| `stelle.anfragen_erschoepft` | `Kennungen erschoepft, Neustart noetig` | 4 (nur in der Bibliothek geprüft, kein Fall) |
| `stelle.anfrage_unbekannt` | `Anfrage unbekannt` | 4 |
| `stelle.anfrage_abgelaufen` | `Anfrage abgelaufen` | 4 |
| `stelle.bereits_entschieden` | `Anfrage schon entschieden` | 4 |
| `stelle.nicht_freigegeben` | `Anfrage nicht freigegeben` | 4 |
| `stelle.anfrage_abgelehnt` | `Anfrage abgelehnt` | 4 |
| `stelle.bereits_eingeloest` | `Freigabe schon eingeloest` | 4 |
| `stelle.digest_abweichend` | `Aufruf weicht von der Anfrage ab` | 4 |
| `stelle.nachweis_ungueltig` | `Nachweis ungueltig` | 4 |
| `einst.*` des Blocks `mcp` (nur beim Laden; Abschnitt 2) | Gründe aus Vertrag 2; neu: `einst.frist_bereich`, `einst.mcp_doppelt` | – |
| `stelle.protokoll_fehler` (nur die Hülle, Abschnitt 13) | `Protokoll nicht schreibbar` | – |
| `stelle.person_fehler` (Abschnitt 14) | `Pseudonym nicht bildbar` | 4 (nur die Hülle) |
| `stelle.vorgabe_fehlt` (Abschnitt 14) | `Vorgabe fehlt oder unsicher` | 4 |
| `stelle.harness_nicht_erlaubt` (Abschnitt 14) | `Harness nicht erlaubt` | 4 |
| `stelle.modell_nicht_erlaubt` (Abschnitt 14) | `Modell nicht erlaubt` | 4 |
| `stelle.effort_nicht_erlaubt` (Abschnitt 14) | `Effort nicht erlaubt` | 4 |
| `stelle.worker_hoechstzahl` (Abschnitt 14) | `Hoechstzahl der Worker erreicht` | 4 |
| `stelle.zusage_ungueltig` (Abschnitt 15) | `Zusage fehlt oder passt nicht` | 4 |
| `stelle.orchestrator_ist_worker` (Abschnitt 15) | `Orchestrator ist selbst ein Worker` | 4 |
| `stelle.route_unbekannt` (Abschnitt 15) | `Route unbekannt` | 4 |
| `stelle.worker_klasse_hoeher` (Abschnitt 15) | `Klasse hoeher als beim Orchestrator` | 4 |
| `stelle.worker_fremder_kunde` (Abschnitt 15) | `Kunde weicht vom Orchestrator ab` | 4 |
| `stelle.zusagen_erschoepft` (Abschnitt 15) | `Zusagen erschoepft, Neustart noetig` | 4 (nur in der Bibliothek geprüft, kein Fall) |

Codes und Gründe sind feste Texte; die Fälle prüfen beide. Eine Ablehnung nennt nie einen Wert aus der Eingabe, auch nicht den Namen eines Servers oder Werkzeugs. Ein Agent, der die Stelle nicht erreichen soll, bekommt von ihr nur `stelle.aufrufer_nicht_erlaubt`.

## 9. Sichtbare Fälle

`faelle/` entsteht mit `node rust/spec/stelle/bau-faelle.mjs` (Mac oder Linux-Baurechner, leicht, ohne Abhängigkeit, ohne Netz). Das Skript nimmt die kanonische Form aus `the previous implementation` und `node:crypto`.
Es enthält ein eigenes kleines Referenzmodell der Stelle und einen eigenen Zeilenprüfer. Jeder Fall trägt die Absicht von Hand: die Antwort, die Zeilenarten je Schritt und den Zustand am Ende. Das Modell rechnet jeden Schritt nach;
weicht es von der Absicht ab, bricht das Skript ab. Es prüft außerdem, dass `entscheide` den übergebenen Zustand nicht ändert, dass jede Zeile kanonisch, geschlossen und verkettet ist, dass kein Argument- und kein Vorschaumarker, kein Namensmarker und kein Pfad in einer Zeile steht, und eine von Hand geschriebene Zeile Byte für Byte.
Der zweite Lauf ergibt dieselben Bytes. Das Modell rechnet Vertrag 1 nur für die Stufen nach, die die Fälle brauchen, und Vertrag 2 nur für Routen ohne Klasse und den Block `mcp`; die vollständige Codematrix liegt in den Fällen jener Verträge.
Grenze des Modells: Es liest JSON mit `JSON.parse` und unterscheidet darum `1.0` nicht von `1`; es nennt eine Route, die kein Objekt ist, oder eine Klasse als Zahl `route_ohne_klasse`. Kein sichtbarer Fall berührt das. Maßgeblich ist der Vertragstext: `1.0` ist keine Ganzzahl, und eine solche Route ist `einst.typ_falsch` (Vertrag 2). Der Bau prüft `1.0` in eigenen Tests (rust/crates/stelle/tests/kern.rs, einstellungen.rs); die Route, die kein Objekt ist, und die Klasse als Zahl folgen als verdeckte Fälle.

**Ein Fall** (`faelle/<name>.json`):

| Feld | Inhalt |
|---|---|
| `beschreibung`, `regeln` | Absicht und Anforderungen |
| `einstellungen.text` | die Einstellungsdatei (Abschnitt 2) als Text |
| `laden_ms` | Ladezeit, mit der der Zustand geladen wird (Abschnitt 1) |
| `pruefer` | `{"art":"h1","schluessel_hex":…}` (Abschnitt 6) |
| `erwartet_laden` | nur bei Fällen, in denen `laden` scheitert: `{"fehler", "grund"}`; dann gibt es keine Schritte |
| `schritte[]` | `aufrufer`, `vorgang`, `jetzt_ms`, `eingabe` und `erwartet`: die ganze `antwort` und `zeilen[]` mit `text` (die Zeile ohne LF) und `sha256` (der Hash dieser Zeile, zugleich `vorher` der nächsten) |
| `ende` | `sitzungen` (sortiert), `anfragen` (Kennung → Zustand), `zaehler`, `zeilen` (Zahl), `letzte` (sha256 der letzten Zeile oder `null`) |

Jeder Fall beginnt mit einer neuen Datei ohne Zeile (`vorher` der ersten Zeile sind 64 Nullen) und einem frisch geladenen Zustand. Ein Bau ist vertragstreu, wenn jeder Schritt dieselbe Antwort und dieselben Zeilenbytes liefert und der Zustand am Ende stimmt. Der Zeitpunkt `jetzt_ms` ist Teil des Falls.

| Fall | Zeigt |
|---|---|
| `start-cloud-gueltig`, `start-lokal-gueltig` | gültiger Start (cloud mit Kunde, lokal ohne Kunde) und die Zeile `sitzung.gestartet` |
| `start-route-unbekannt`, `start-ohne-route`, `start-kunde-unbekannt`, `start-harness-passt-nicht` | Code aus Vertrag 1 unverändert, `sitzung.abgelehnt` mit dem Code |
| `start-kennung-schreibt-null`, `start-bytes` | ungültige Werte und Bytes stehen als `null` in der Zeile |
| `start-sitzung-vergeben`, `start-nur-der-kern` | Kennung einmal; Aufrufer vor Eingabe |
| `laden-route-ohne-klasse` | die Stelle lädt nicht (A42) |
| `laden-mcp-doppelt`, `laden-mcp-frist-bereich`, `laden-mcp-unbekannter-schluessel`, `laden-mcp-typ-falsch`, `laden-mcp-anzahl` | Block `mcp` der Fassung 2 von Vertrag 2: neue Codes und vorhandene Stufen (A89, A90) |
| `freigabe-erteilt-und-eingeloest` | der ganze Weg, eine Zeile mit dem Freigebenden |
| `einloesen-zweimal`, `einloesen-nach-frist`, `frist-grenzen`, `einloesen-anderes-argument` | A105 Punkte 1 bis 3 |
| `agent-und-hub-geben-nicht-frei`, `nur-der-hub-loest-ein` | A92: jede falsche Art scheitert, auch mit gültigem Nachweis |
| `ablehnung-durch-mensch`, `einloesen-ohne-freigabe` | Ablehnung, offene Anfrage |
| `nachweis-ungueltig` | Prüfer `h1`: anderer Aufruf, andere Entscheidung (beide Richtungen), falsches Pseudonym im Text, falscher Schlüssel, Großbuchstaben, fremdes Format |
| `offene-liste`, `offene-nur-der-mensch` | `mcp.offene`: leer, drei offen, Erteilung und Ablehnung nehmen heraus, Ablauf nimmt heraus; Vorschau nur solange offen |
| `stand-fragen`, `stand-unbekannt-und-aufrufer` | `mcp.stand` in allen Zuständen; unbekannte Anfrage, falsche Aufrufer |
| `neustart-macht-alles-ungueltig` | E1: Neustart, neue Kennung mit anderem `laden_ms` |
| `werkzeug-ohne-zuordnung`, `lesendes-werkzeug-frei`, `server-ohne-freigabe-option` | A89 (inklusive `such-mcp`/`brain_suche`), Option aus |
| `ablauf-ohne-entscheidung`, `ablauf-erteilt-nicht-eingeloest`, `uhr-geht-zurueck` | Fristen, Takt, Zeit |
| `anfragen-gruende`, `drei-schreibaufrufe-im-protokoll`, `zeit-ungueltig` | Prüfreihenfolge der Anfrage, A91 mit drei Zeilen, Stufe 0 |
| `worker-frei`, `worker-gruende`, `worker-ohne-vorgabe`, `worker-hoechstzahl-grenzen` | `worker.anfordern` (Abschnitt 14.5) |
| `worker-klasse`, `worker-kunde`, `worker-nur-eine-ebene`, `worker-modellname`, `worker-start-mit-zusage`, `worker-start-zusage-gruende`, `worker-zusage-neustart` | Worker startet nur der Orchestrator (Abschnitt 15.5) |

## 10. Was im Durchstich echt ist und was später kommt

| | Im Durchstich | Später |
|---|---|---|
| Kern `entscheide` mit den sieben Vorgängen | echt, Rust | – |
| Messung des Aufrufers | Schnittstelle (Arten); Messung durch die Plattformschicht | Messung unter Linux und Windows (Plan 5.3) |
| Nachweis des Menschen | Herausforderung und Prüfer `h1` mit gemeinsamem Schlüssel echt; Brücke vertraut | Gerätesignatur, Anmeldung, Zuordnung zu mehreren Menschen |
| Einlösen | online bei der Stelle, durch den Hub, genau einmal | Schein mit Signatur für den Halter (Abschnitt 11, Punkt 3); Ausführung und `mcp.ausgefuehrt` |
| Schnittstelle Hub zu Halter (Ausführung mit dem Token, A93) | **ausdrücklich offen**, Halter-Vertrag | Halter-Vertrag |
| Zeilen | drei Arten plus `mcp.schreibaufruf`, über die Kette von Vertrag 5 | Anker, Monatswechsel, `audit.luecke` (Vertrag 5, Abschnitt 5) |

**Der Durchstich garantiert:** Die Stelle lässt einen Schreibaufruf nur einmal, nur innerhalb der Frist, nur für den Aufruf, dessen `digest` sie bei der Anfrage sah, und nur nach der Entscheidung eines Menschen mit Nachweis ein, und sie schreibt jede Entscheidung und jede Ablehnung in die Kette.
**Der Durchstich garantiert nicht:** dass der Hub nach dem Einlösen genau diesen Aufruf ausführt. Dafür braucht der Halter den signierten Schein. Bis dahin ist der Hub ein vertrauter Teil: Er liegt in der Vertrauensgrenze des Produkts (eigener Prozess außerhalb des Einschlusses). Ein Agent im Einschluss erreicht die Stelle nicht und kann nicht einlösen (A92).
**Restvertrauen in den Hub:** Die Stelle prüft nicht, ob die Vorschau zu den Argumenten passt. Der Mensch unterschreibt den Hash der Vorschau, die er sieht; ob sie den Aufruf wahr beschreibt, hängt am Hub.

**Nicht im Durchstich:**

- **Vier Augen für Schreibaufrufe** (Firmenoption, Vorgabe aus, A105): Der Kern trägt `person` in der Anfrage, aber die Prüfung „Entscheidender ist nicht die Person des Laufs“ fehlt. Sie braucht außerdem eine Zuordnung von Sitzung zu Person, die die Stelle selbst kennt, nicht der Hub.
- **Herstellerzugang** (A11), Einsicht ins Protokoll mit Rolle und zweiter Person (A102), Auskunft und Löschen je Person (A12), Pseudonymbildung (die Stelle bekommt Pseudonyme fertig), Notfallzugang, alle weiteren Vorgänge der heutigen Stelle (`wer`, `raum`, `rechte.*`, `person.*`, …).
- **Sitzungsende**, Begrenzung offener Anfragen, Löschen alter Endzustände, Zusammenfassen von Ablehnungen bei Flut (`anzahl`).
- Sicherung des Zustands (E1: Ein Neustart macht alles ungültig, Abschnitt 1).
- Mehrere Menschen je Anfrage, Sammelfreigabe (A105: gibt es nicht).
- Die Fassung 2 von Vertrag 2 in der echten Datei und im Schema.
- **Aufruferart `brain` (Fassung 2 von Vertrag 6, Wunsch, angenommen):** der Such-Prozess der Sitzung. Er darf nur `mcp.anfragen`, `mcp.einloesen` und `mcp.stand`, und nur für den Server `brain`; der Hub darf den Server `brain` nicht anfragen. Im Durchstich gibt es die Art nicht, und `such-mcp` gilt als lesender Server (Abschnitt 2). Kommt `brain` dazu, erweitert die Tabelle in Abschnitt 3 sich um eine Zeile und die Prüfreihenfolge um die Stufe „Server passt zur Art“.

## 11. Annahmen und Abweichungen vom Entwurf

Der Vertrag trifft diese Annahmen. Jede lässt sich ändern, ohne einen Fall abzuschwächen.

1. **Die Stelle ordnet die Werkzeuge zu, nicht der Hub.** Der Entwurf lässt den Hub die Klasse lesen (Abschnitt 1, Punkt 1). Hier fragt der Hub jeden Aufruf und die Stelle antwortet `lesend` oder legt eine Anfrage an. Grund: „Der Hub kann nie entscheiden.“ Ein Hub mit falscher Zuordnung darf keinen Schreibaufruf als lesend durchlassen.
2. **Digest aus `server`, `werkzeug` und `argumente`.** Der Entwurf bindet zusätzlich `person` und `vorschau_sha256` in den Digest. Der Halter kann beides nicht nachrechnen (Entwurf Halter, Abschnitt b: „den `digest` selbst aus dem Aufruf berechnen“). Hier binden die Herausforderung des Menschen (Vorschau-Hash, Entscheidung, Anfragekennung) und der gespeicherte Anfragesatz die beiden Werte; der Digest bleibt für den Halter nachrechenbar. Die Person der Anfrage steht im Zustand und in der Zeile.
3. **Einlösen online** durch den Hub bei der Stelle (Auftrag und Projektentscheid, 20.11.2026, Vorgang 2) statt Prüfung eines signierten Scheins beim Halter. Der Entwurf Halter nennt das als Alternative. Der signierte Schein folgt, wenn der Halter ihn braucht; dann ändert sich die Antwort von `mcp.einloesen` um das Feld `schein`.
4. **Ergebnis `eingeloest` statt `ausgefuehrt`.** Der Entwurf kennt `ausgefuehrt` und `fehlgeschlagen` nach `mcp.ausgefuehrt` vom Halter. Dieser Vertrag hat den Vorgang nicht; die Zeile steht beim Einlösen. Kommt `mcp.ausgefuehrt`, schreibt die Stelle `ausgefuehrt` oder `fehlgeschlagen` an Stelle von `eingeloest` und der Zustand `eingeloest` wird zu einem Zwischenzustand. Das ändert dann Fälle, die Aussage „genau einmal“ bleibt.
5. **Ablehnung braucht ebenfalls den Nachweis.** Der Entwurf verlangt ihn nur für das Freigeben. Gründe: Ohne Nachweis könnte jeder, der die Oberfläche erreicht, alle Anfragen ablehnen (Störung), und die Zeile hätte keinen Entscheidenden. Der Handgriff ist derselbe.
6. **`person` kommt vom Hub** (Pseudonym der Person des Laufs). Vertrag 1 hat keine Person im Start. Der Hub ist als Aufrufer gemessen; ein ehrlicher Hub setzt sie richtig. Für Vier Augen später muss die Stelle sie selbst kennen (Abschnitt 10).
7. **Feldname `entscheidend`** statt `freigebend` des Entwurfs, weil auch eine Ablehnung einen Entscheidenden hat. Weiter: Die Zeile trägt `sitzung` (der Entwurf nennt `lauf`); sie bindet die Anfrage an die Sitzung des Starts. `mcp.anfragen` verlangt darum eine bekannte Sitzung. Die Zeilen `ausgefuehrt`, `ohne_freigabe` und `abgelaufen` des Entwurfs sind als `ergebnis` übernommen.
8. **Kennungen der Anfragen** tragen die Ladezeit (`mf-<laden_ms>-<n>`, E1), kein Zufall (der Kern hat keinen Zufall). Der Entwurf nennt `mf_<32 hex>`. Sie sind nicht geheim; der Schutz liegt am Aufrufer, am Digest und am Nachweis.
9. **Start**: Die Stelle bekommt die Startdatei vom Kern-Dienst nach `start::pruefen` und vor „Lauf starten“ (Projektentscheid, 20.11.2026). Sie prüft mit derselben Bibliothek ein zweites Mal und protokolliert Erfolg und Ablehnung. Das Protokoll des Starts trägt keine Person (Vertrag 1 hat keine).
10. **Die Option `freigabe` und die zwei Fristen** (`frist_ms`, `einloesefrist_ms`) je Server sind vereinbart (20.11.) und werden in Vertrag 2 Fassung 2 mit Beginn von B geschrieben.
11. **Die Stelle speichert die Vorschau, solange die Anfrage offen ist** (Projektentscheid, 20.11.2026, für `mcp.offene`). Im Protokoll steht sie nie.

## 12. Offene Punkte und Fragen

1. **Entschieden (E1, 20.11.): Der Zustand wird im Durchstich nicht gesichert.** Er ist Speicher; ein Neustart macht alles ungültig (Abschnitt 1). Es bleibt der Projektentscheid, 18.11.2026: `offen` und `erteilt` schreiben keine Zeile. Folge: Stirbt die Stelle nach einer Erteilung, bleibt von ihr keine Spur, die Anfrage ist danach unbekannt (fail closed). Mit der Sicherung des Zustands (später) ist die Frage neu zu stellen.
2. **Vertrag 2, Fassung 2:** vereinbart (Abschnitt 2); geschrieben mit Beginn von B. Die Grenzen von `einst.anzahl` (64 Server, 256 Werkzeuge) und die Gründe der neuen Codes sind Vorschlag dieses Vertrags.
3. **Vertrag 5:** `kette pruefe` kennt bisher nur eine geschlossene Zeilenart. Die Aktionsnamen `mcp.schreibaufruf`, `sitzung.gestartet`, `sitzung.abgelehnt` und `vorgang.abgelehnt` sind dort zu ergänzen, und `formate` braucht Funktionen zum Schreiben mehrerer Zeilen unter einer Sperre (Abschnitt 7.5, 13.1). Dieser Auftrag durfte Vertrag 5 nicht ändern.
4. **Entschieden (E2):** Erst die Zeilen, dann Zustand und Antwort; Regeln der Hülle in Abschnitt 13.1. Kennungen über Neustarts sind durch `laden_ms` eindeutig (E1).
5. **Peer-Messung (offen, Plattformschicht):** Wie die Plattformschicht die Aufruferart aus Socket und gemessener Kennung bestimmt (Abschnitt 13.2), besonders `hub` gegen `agent` bei gleicher Kennung (Entwurf Halter, Nachtrag 3: pidfd und Cgroup der Hub-Unit), und die Named Pipes unter Windows. Ohne sie bleibt A92 am Nachweis allein hängen.
   **Restrisiko bis zur Messung (Befund B23, 21.11.; gilt für Schritt I, Pflicht vor Schritt II):** Die Stelle trennt die Aufruferart heute nur über den Socket (Abschnitt 13.2). Ob an einem Socket wirklich der Hub spricht und nicht ein anderer Prozess mit derselben Nutzer-Kennung, misst die Plattformschicht noch nicht. „Der Agent gibt nicht selbst frei“ (A92) hängt bis dahin an diesen drei Punkten, die der Vertrag schon festlegt:
   (a) Freigeben ist nur `mcp.entscheiden` der Art `mensch` und braucht den Nachweis `h1` (Abschnitt 6). Seinen Schlüssel lesen nur die Dienstkonten von Stelle und Brücke, die Datei liegt außerhalb des Einschlusses.
   (b) Die Sockets der Stelle liegen in einem Ordner 0700 außerhalb des Einschlusses (Abschnitt 13.2); `agent` hat keinen Socket.
   (c) Der Agent erreicht 127.0.0.1 des Rechners nicht und damit auch die Brücke nicht (Abschnitt 6, Voraussetzung; Gegenprobe S2 im Drehbuch).
   Was bleibt: Ein Prozess der Person außerhalb des Einschlusses kann sich als `hub` ausgeben. Er kann dann anfragen, einlösen und den Stand lesen, aber nie freigeben. Ein Einlösen gelingt nur für eine Anfrage, die ein Mensch mit gültigem Nachweis erteilt hat, für genau diesen `digest` und nur einmal. Ein Agent im Einschluss erreicht keinen der Sockets.
   Das gilt in der ersten Fassung als tragbar, weil dort auf einem Rechner nur eine Person arbeitet und die Prozesse der Person ohnehin der Person gehören. Vor Schritt II misst die Plattformschicht `hub` gegen `agent`; erst dann trägt A92 auch die Unterscheidung der Prozesse und nicht nur den Nachweis.
6. **Vault:** Die Zeile des Starts und die Sperrzeile der Suche (Vertrag 5) teilen `sitzung` und `route`; so lässt sich eine Sperre dem Start zuordnen. Keine Frage an Vault bleibt offen; die Zuordnung von `such-mcp` ist übernommen.
7. **Weitere Vorgänge** folgen, sobald Halter und Oberfläche sie brauchen (`mcp.ausgefuehrt`).

## 13. Hülle um den Kern

Gebaut wird die Hülle später. Dieser Abschnitt legt ihre Regeln fest; sie sind kein Fall des Kerns und stehen nicht in `faelle/`.

### 13.1 Reihenfolge: erst die Zeilen, dann Zustand und Antwort (E2)

1. Die Hülle liest die Uhr und ruft `entscheide`.
2. Hat das Ergebnis Zeilen, ergänzt die Hülle jede um `fassung` 2, `vorher` und `zeit` (aus `zeit_ms`) und schreibt sie **in einem Schreibaufruf** mit `sync` an die Datei `stelle-<JJJJ-MM>.jsonl` (über die Kette aus Vertrag 5).
3. **Unter der Sperre** prüft sie vor dem Schreiben, dass der sha256 der letzten Zeile der Datei gleich dem Wert ist, den sie sich gemerkt hat (beim Start aus der Datei gelesen, 64 Nullen bei fehlender oder leerer Datei; nach jedem Schreiben der neue Wert). Weicht er ab, schreibt sie nichts. Das ist der Schutz gegen einen zweiten Schreiber.
4. Erst nach dem erfolgreichen Schreiben übernimmt sie den neuen Zustand, merkt sich den neuen Hash und sendet die Antwort.
5. Scheitert Schritt 3 oder das Schreiben, bleibt der alte Zustand, es bleibt nichts Neues in der Datei (Vertrag 5: bei jedem Fehler unverändert), und die Antwort ist `{"ok":false,"code":"stelle.protokoll_fehler","grund":"Protokoll nicht schreibbar"}`. Der Aufrufer bekommt keine Freigabe (fail closed).
6. Ein Ereignis ohne Zeilen (etwa `erteilen`, `lesend`, `mcp.offene` ohne Ablauf) schreibt nichts und prüft nichts.

Folge: Hat jemand anderes die Kette verändert, scheitert jedes Ereignis mit Zeilen mit `stelle.protokoll_fehler`, bis die Stelle neu startet und den Stand liest. Die Stelle stoppt also lieber, als an eine fremd veränderte Kette anzuhängen.

### 13.2 Ein Socket je Aufruferart

- Die Stelle bietet **je Aufruferart einen eigenen Unix-Socket** (`kern`, `hub`, `mensch`) in einem Ordner mit Modus 0700, außerhalb des Einschlusses. `takt` ist intern und hat keinen Socket; `agent` und `unbekannt` haben keinen.
- **Die Art folgt aus dem Socket und aus der gemessenen Kennung des Gegenübers.** Die Kennung muss zu der für diese Art eingetragenen passen (Installationskonfiguration). Passt sie nicht, übergibt die Hülle dem Kern die Art `unbekannt`, und der Kern lehnt ab und protokolliert (`stelle.aufrufer_nicht_erlaubt`). Ein Gegenüber mit der Kennung eines Agenten gilt als `agent`.
- Die Art folgt nie aus einem Feld des Rahmens. Wie die Kennung gemessen wird (pidfd, Cgroup), gehört zur Plattformschicht (Abschnitt 12, Punkt 5).
- **Unter Windows** bietet die Stelle je Art eine Named Pipe mit einer DACL nur für die SID dieser Art. Die Messung des Gegenübers folgt der Plattformschicht.
- **Die Brücke ist Client am Socket `mensch`.** Sie hält den Schlüssel von `h1`, stellt den Nachweis nur nach einem Klick des angemeldeten Menschen aus und ruft `mcp.offene` und `mcp.entscheiden`.

### 13.3 Rahmen

- Eine JSON-Zeile je Anfrage, genau `{"vorgang": <Name>, "eingabe": <Eingabe des Vorgangs>}`; eine JSON-Zeile je Antwort in RFC 8785; Zeilenende LF; höchstens 2 097 152 Byte je Zeile (Anfrage und Antwort).
- Die Hülle reicht `vorgang` und `eingabe` unverändert an den Kern und setzt Aufrufer und Zeit selbst.
- **Die Hülle liest den Rahmen streng (V, Befund B6 vom 23.11.2026).** Streng heißt: genau ein JSON-Objekt, UTF-8 ohne BOM, kein Text vor oder nach dem Objekt, kein Schlüssel zweimal auf irgendeiner Ebene, keine Zahl außerhalb von JSON (`NaN`, `Infinity`, `1e999`), kein abgeschnittenes oder nachlässiges JSON (kein Komma am Ende, keine einfachen Anführungszeichen, kein Kommentar). Das ist dieselbe Lesung, die der Kern für die Startdatei und für die Einstellungsdatei verwendet; die Stelle hat sie schon (`rust/crates/stelle/src/form.rs`, `objekt_lesen` und `schluessel_doppelt`), die Hülle nimmt genau diese Funktion. Grund: Ein Rahmen mit `argumente` zweimal verkürzt ein üblicher Leser auf den letzten Wert, und Oberfläche, Hub und Stelle können dann verschiedene Werte gesehen haben, obwohl der `digest` nur den Wert der Stelle bindet.
- Ein Rahmenfehler (kein JSON, nicht streng gelesen im Sinne des Punktes darüber, mehr als 2 MiB, andere Felder als die zwei, ein Vorgangsname, den der Kern nicht kennt) beantwortet die Hülle mit `stelle.eingabe_ungueltig` und schließt die Verbindung. Sie schreibt dazu **keine** Zeile, denn der Rahmen liefert keinen Wert, der sicher ins Protokoll darf. Das ist die zweite Ausnahme neben Stufe 0 von der Regel „jede Ablehnung steht im Protokoll“; die Hülle meldet sie im Betriebsprotokoll (stderr) mit Art und Anzahl, nie mit Inhalt.
- Eine Verbindung trägt beliebig viele Anfragen nacheinander. Die Hülle verarbeitet Ereignisse eines Zustands streng nacheinander.

## 14. Nachtrag 27.11.2026: Transport und Vorgänge `person.pseudonym` und `worker.anfordern` (Nr. 98)

Grundlage: Abschnitt 1 („Transportschicht“), Abschnitt 3, Abschnitt 13, Vertrag `peerkanal` Fassung 1 (`rust/spec/peerkanal/README.md`), Vertrag `dienst` Nachtrag „Kanal zur Stelle (L6, Nr. 98)“ und die Regeln DI-L6 und DI-L11 des Vertrags `dienst`. Wo dieser Abschnitt nichts sagt, gelten die Abschnitte 1 bis 13 unverändert. Der Kern bleibt rein: Alles mit Socket, Datei, Uhr und Zufall gehört der Hülle (Abschnitt 13).

### 14.1 Kanäle

1. **Je Aufruferart ein Kanal nach `peerkanal`** (Abschnitt 13.2 bleibt). Die Kanalnamen sind `stelle-kern`, `stelle-hub` und `stelle-mensch`. Unter Linux und macOS ist das der Unix-Socket `<ordner>/<name>.sock`, unter Windows die Named Pipe `\\.\pipe\<name>`. `takt` ist intern, `agent` und `unbekannt` haben keinen Kanal.
2. **Installationskonfiguration der Stelle** (eine Datei der Stelle, nicht die Einstellungsdatei von Vertrag 2): `{"fassung": 1, "ordner": "/abs/pfad", "erlaubt": {"kern": [<Erlaubnis>], "hub": [<Erlaubnis>], "mensch": [<Erlaubnis>]}, "einstellungen": "/abs/pfad", "schluessel_h1": "/abs/pfad", "protokoll": "/abs/pfad", "personenbezug": "/abs/pfad", "vorgabe": "/abs/pfad"}`. Eine Erlaubnis hat die Form von `peerkanal` (`{"uid": <Zahl>}` unter Unix ohne `gid`, `{"sid": "<SID>", "appcontainer_sid": null}` unter Windows). `ordner` gibt es nur unter Unix. Die Form ist geschlossen; jede Abweichung, ein fehlender Schlüssel oder eine leere Liste beendet den Start (Exitcode 78). Eine Erlaubnis mit einem AppContainer gibt es für keine Art.
3. **Server.** Je Art `Server::anlegen` mit `erlaubt` dieser Art, `instanzen` 8, `hallo_frist` 2 s, `prozess_pflicht` true, unter Unix Modus 0600 im Ordner aus Punkt 2 (0700, Besitzer das Konto der Stelle, kein Link; `peerkanal`, Abschnitt „Server anlegen“). `pk.name_belegt` und `pk.ordner_rechte` beenden den Start mit Exitcode 74. Ein Kanal, der sich nicht anlegen lässt, lässt die Stelle nicht mit den anderen Kanälen laufen: Sie startet gar nicht.
4. **Die Art misst die Hülle, nie der Rahmen (V).** Die Art einer Verbindung ist die Art des Kanals, an dem sie angenommen wurde, und nur, wenn `peerkanal` die Kennung der Gegenseite gegen `erlaubt` dieser Art geprüft hat. Lässt `peerkanal` die Verbindung nicht durch, gibt es keine Verbindung und keinen Aufruf des Kerns. Die Hülle meldet die Ablehnung im Betriebsprotokoll (stderr) mit Art des Kanals, Code und Grund von `peerkanal`, nie mit einer Kennung, einem Namen oder einer pid. Eine Zeile `kanal.abgewiesen` in der Kette (Vertrag 5) folgt, wenn Vertrag 5 sie für die Quelle `stelle` zulässt (Frage). Im Haken (peerkanal Stufe 7) bestätigt die Stelle sofort; die Unterscheidung `hub` gegen `agent` bei gleicher Kennung bleibt Abschnitt 12 Punkt 5 (Restrisiko B23).
5. **Der Rahmen** ist der von Abschnitt 13.3, unverändert: eine JSON-Zeile `{"vorgang", "eingabe"}`, streng gelesen, höchstens 2 097 152 Byte; eine Antwort je Anfrage in RFC 8785; Zeilenende LF. Eine Verbindung trägt beliebig viele Anfragen nacheinander. Liest die Hülle binnen 30 s kein ganzes LF-Zeilenende, schließt sie die Verbindung ohne Antwort und ohne Zeile.
6. **Reihenfolge.** Die Hülle verarbeitet die Ereignisse aller Kanäle über einen Zustand streng nacheinander (Abschnitt 13.3). Für jedes Ereignis gilt Abschnitt 13.1: erst die Zeilen mit `sync`, dann der Zustand, dann die Antwort. Scheitert das Schreiben, antwortet sie `stelle.protokoll_fehler`, und es gibt keine Freigabe.
7. **Takt.** Die Hülle ruft `ablauf.pruefen` als Art `takt` einmal je Sekunde und ohne Kanal.

### 14.2 Was die Gegenseite prüft

- **Die Stelle prüft den Dienst** über `erlaubt.kern` und `erlaubt.mensch`: die Kennung des Dienstkontos (uid oder SID), gemessen vom Kernel beim `connect`. Ein Pfad, ein Name und ein Feld des Rahmens zählen nie.
- **Der Dienst prüft die Stelle** als Client von `peerkanal` (`verbinden` mit `erwartet`, Vertrag `dienst` Nachtrag „Kanal zur Stelle“). Er sendet kein Nutzbyte, bevor die Kennung des Servers passt.
- Fällt eine der beiden Prüfungen, gibt es keine Verbindung. Kein Vorgang hat einen Weg ohne Prüfung, auch nicht im Prüfbau.

### 14.3 `mcp.offene` nennt die Sitzung

Die Antwort von `mcp.offene` trägt je Anfrage zusätzlich `sitzung` (die Kennung aus `mcp.anfragen`). Grund: Der Dienst gibt eine offene Anfrage nur dem Kern der Sitzung, zu der sie gehört (DI-L6 Punkt 3). Die Fälle `offene-liste` und `offene-nur-der-mensch` ändern damit ihre Antworten (geänderte Erwartung); in Fassung 2 (`fassung2/faelle/`) ebenso `keine-sammelfreigabe`, `vier-augen-schreibaufruf`, `zweite-person-feld-bei-option-aus` und `zweite-person-senken`, nur um das Feld `sitzung`. Das Modell von Fassung 2 spielt im Gleichlauf auch `worker.anfordern` nach.

### 14.4 `person.pseudonym`

| Vorgang | Aufrufer | Eingabe (geschlossen) | Antwort bei Erfolg |
|---|---|---|---|
| `person.pseudonym` | `kern` | `{"kennung": <Kennung einer Person>}` | `{"ok":true,"ergebnis":"pseudonym","pseudonym":"ps_<32 hex>"}` |

- `kennung` hat die Form von Vertrag 9 Abschnitt 3; im Dienst ist sie `uid-<uid>` (Unix) oder `sid-<SID klein>` (Windows), nie ein Kontoname (Vertrag `dienst`, DI-L11 Punkt 4). Sonst `stelle.eingabe_ungueltig`.
- Die Hülle beantwortet den Vorgang aus dem Speicher von Vertrag 9 Abschnitt 4 (`oeffnen` beim Start, `person(kennung, zufall, jetzt_ms)` je Aufruf; der Zufall kommt vom Betriebssystem). Gebaut werden nur `oeffnen` und `person`, mit Ordner 0700, `schluessel.json`, `zuordnung.json`, Sperre und atomarem Ersetzen, jeder Zugriff über Deskriptoren. Text, Auskunft, Löschen, Wechsel und Abgleich fehlen in dieser Fassung. Der Kern sieht die Kennung nie. Ohne geöffneten Speicher startet die Stelle nicht (Vertrag 9).
- Ein Fehler des Speichers ergibt `stelle.person_fehler` und kein Pseudonym. Es gibt keinen Rückfall auf ein Pseudonym ohne Speicher.
- Eine Antwort ohne Fehler schreibt keine Zeile. Eine Ablehnung schreibt `vorgang.abgelehnt` (Abschnitt 7.3) mit `anfrage: null`.

### 14.5 `worker.anfordern`

Abschnitt 15.3 ändert Eingabe, Antwort, die Form von `modell` und die Stufe 4 dieses Vorgangs.

| Vorgang | Aufrufer | Eingabe (geschlossen) | Antwort bei Erfolg |
|---|---|---|---|
| `worker.anfordern` | `kern` | `{"sitzung", "rolle", "harness", "modell", "effort", "laufend"}` | `{"ok":true,"ergebnis":"frei","sitzung":…}` |

- `sitzung` ist die Kennung des Orchestrators (gestartet, Abschnitt 4), `rolle`, `harness` und `modell` sind Kennungen, `effort` ist eine Kennung, `laufend` ist die Zahl der laufenden Worker dieses Orchestrators, die der Dienst zählt (Ganzzahl 0 bis 1000). Die Stelle kennt kein Sitzungsende (Abschnitt 4) und nimmt den Wert darum vom Dienst, der als `kern` gemessen ist. Ein falscher Wert kommt nur vom Dienst des Nutzers, der Worker auch von Hand starten kann: Die Höchstzahl ist eine Betriebsgrenze, keine Sicherheitsgrenze.
- **Die Regeln kommen nur aus der Vorgabe der Admins** (`vorgabe` der Installationskonfiguration): `{"fassung": 1, "worker": {"hoechstzahl": <1..64>, "harness": [<Kennung>…], "modelle": [<Kennung>…], "effort": [<Kennung>…]}}`, geschlossen, jede Liste 1 bis 64 Einträge ohne Doppel, höchstens 65 536 Byte.
- **Prüfung der Vorgabe mit Deskriptoren** wie `lauf` für die Firmenvorgabe: von `/` abwärts jeder Ordner relativ zum vorigen mit `O_NOFOLLOW|O_DIRECTORY` geöffnet und mit `fstat` geprüft (Besitzer root, für Gruppe und andere nicht schreibbar, außer mit Sticky-Bit über dem letzten Ordner), die Datei mit `O_NOFOLLOW` geöffnet und mit `fstat` geprüft (gewöhnliche Datei, Besitzer root, für Gruppe und andere nicht schreibbar), dann gelesen. Kein Schritt prüft einen Pfadtext und öffnet danach. Im Prüfbau darf der Besitzer zusätzlich das eigene Konto sein, nie ein drittes.
- Die Hülle prüft die Vorgabe beim Start und gibt dem Kern die gelesenen Regeln oder „keine“. Fehlt die Vorgabe, ist sie unsicher oder ungültig, startet die Stelle trotzdem, und jedes `worker.anfordern` ergibt `stelle.vorgabe_fehlt`. Die Datei der Person (Vertrag 2) trägt keine Regel für Worker und wird dafür nie gelesen.
- **Stufe 4**, in dieser Reihenfolge: Regeln fehlen (`stelle.vorgabe_fehlt`); `sitzung` nicht gestartet (`stelle.sitzung_unbekannt`); `harness` nicht in `harness` (`stelle.harness_nicht_erlaubt`); `modell` nicht in `modelle` (`stelle.modell_nicht_erlaubt`); `effort` nicht in `effort` (`stelle.effort_nicht_erlaubt`); `laufend` nicht kleiner als `hoechstzahl` (`stelle.worker_hoechstzahl`). Sonst `frei`.
- `frei` schreibt keine Zeile; jede Ablehnung schreibt `vorgang.abgelehnt` mit `anfrage: null`. Was die Regeln nicht ausdrücklich erlauben, startet nicht (fail closed). Das ist Stufe 1.

### 14.6 Neue Codes (stehen in Abschnitt 8)

| Code | Grund (fest, ASCII) | Stufe |
|---|---|---|
| `stelle.person_fehler` | `Pseudonym nicht bildbar` | 4 (nur die Hülle) |
| `stelle.vorgabe_fehlt` | `Vorgabe fehlt oder unsicher` | 4 |
| `stelle.harness_nicht_erlaubt` | `Harness nicht erlaubt` | 4 |
| `stelle.modell_nicht_erlaubt` | `Modell nicht erlaubt` | 4 |
| `stelle.effort_nicht_erlaubt` | `Effort nicht erlaubt` | 4 |
| `stelle.worker_hoechstzahl` | `Hoechstzahl der Worker erreicht` | 4 |

### 14.7 Fälle (stehen in Abschnitt 9)

| Fall | Zeigt |
|---|---|
| `worker-frei`, `worker-gruende` | `worker.anfordern`: frei; jede Stufe der Prüfreihenfolge mit ihrem Code; Aufrufer außer `kern` |
| `worker-ohne-vorgabe` | ohne Regeln immer `stelle.vorgabe_fehlt`, auch für eine gültige Anfrage |
| `worker-hoechstzahl-grenzen` | `laufend` gleich `hoechstzahl` minus 1 ist frei, gleich `hoechstzahl` nicht |
| `offene-liste` (geändert) | `sitzung` je offener Anfrage |

Die Prüfung der Vorgabe, die Kanäle, die Peer-Prüfung und `person.pseudonym` sind Sache der Hülle. Sie stehen in Rust-Tests des Crates `stelle` (Abschnitt 13: kein Fall des Kerns).

### 14.8 Vertrag 5

Die Zeile `vorgang.abgelehnt` (Abschnitt 7.3) nennt in `vorgang` auch `person.pseudonym` und `worker.anfordern`. Vertrag 5 führt beide Namen in Abschnitt 6.8 (`rust/spec/protokollzeile/README.md`), und `kette pruefe` nimmt sie an.

### 14.9 Wer den Nachweis `h1` ausstellt (Nachtrag 27.11.2026)

Die Brücke von Abschnitt 6 ist für `mcp.entscheiden` der Dienst der Oberfläche (Vertrag `dienst`, Nachtrag „Kanal zur Stelle“, Punkt 17). Er stellt `h1` nur auf dem lokalen Weg aus, für das von `peerkanal` gemessene Konto und mit dem Pseudonym aus seiner Installationskonfiguration (`lauf.stelle.nachweis`); über `dienst.sock` gibt es keinen Nachweis. Herausforderung und HMAC stehen einmal in diesem Crate: `stelle::herausforderung` und `stelle::nachweis_h1` bilden, `PrueferH1` prüft. Der Schlüssel des Dienstes liegt in einer Datei mit Modus genau 0600 (`stelle::schluessel_h1`). Die Ansicht sendet keinen Nachweis mehr.

### 14.10 Offen

- Den Schlüssel `lauf.stelle` und die Meldezeile für `worker.anfordern` legt der Vertrag `dienst` fest (Nachtrag „Kanal zur Stelle (L6, Nr. 98)“). Die Seite von `lauf` fehlt noch. Wer `sitzung.starten` ruft, steht in Abschnitt 15.1.
- Stufe 2 von `worker.anfordern` (Projektentscheid, 27.11.2026): eine Rückfrage an einen Menschen im Inspektor, als Anfrage wie bei MCP mit Frist und Nachweis, und eine Regel in der Vorgabe, wann sie gilt. Sie ist nicht gebaut. alt bis 27.11.2026; seit dem Entscheid in Abschnitt 15 gibt es in diesem Weg keine menschliche Freigabe, Stufe 2 entfällt.
- Vom Speicher nach Vertrag 9 fehlen Text, Auskunft, Löschen, Wechsel und Abgleich.

## 15. Nachtrag 27.11.2026: Worker startet nur der Orchestrator

Grundlage sind die Entscheide des Betreibers vom 27.11.2026: Einen Worker startet der Orchestrator, nie ein Mensch. Der Agent im Einschluss fordert über den Meldeweg von `lauf` an; der Dienst fragt die Stelle; die Stelle entscheidet allein nach den Regeln der Admins, ohne menschliche Freigabe. Ein Worker bekommt nie eine Route höherer Klasse als sein Orchestrator und nie einen Kunden, den der Orchestrator nicht hat. Die Schnittstelle ist mit den Eigentümern von Dienst und `lauf` abgestimmt. Dieser Abschnitt ergänzt 14.5 und ändert ihn, wo er es sagt.

### 15.1 Wer `sitzung.starten` ruft

Bis zu diesem Nachtrag rief kein Programm `sitzung.starten`; nur Tests taten es. Die Stelle kannte darum keine Sitzung, und jedes `worker.anfordern` hätte `stelle.sitzung_unbekannt` ergeben.

- Der Dienst ruft `sitzung.starten` bei jedem `Starten` mit `lauf.stelle` über den Kanal `stelle-kern` (Abschnitt 14.1). Er ruft nach seiner eigenen Prüfreihe und vor dem Start von `lauf`, mit genau den Bytes der Startdatei, die er danach schreibt.
- Lehnt die Stelle ab, endet der Start mit `stelle_abgelehnt`. Fällt sie aus oder antwortet sie nicht in der Form, endet er mit `stelle_nicht_erreichbar` oder `stelle_antwort_ungueltig`. Es gibt nie einen Start ohne Antwort der Stelle. Die Gründe stehen im Vertrag `dienst`.
- Ohne `lauf.stelle` ruft der Dienst die Stelle nicht. Dann ergibt jedes `worker.anfordern` beim Dienst `stelle_nicht_erreichbar` (Vertrag `dienst`, Nachtrag „Kanal zur Stelle“, Punkt 10), es entsteht also kein Worker.

### 15.2 `sitzung.starten` mit Orchestrator und Zusage

| Vorgang | Aufrufer | Eingabe (geschlossen) | Antwort bei Erfolg |
|---|---|---|---|
| `sitzung.starten` | `kern` | `{"start", ("orchestrator"), ("zusage")}` | unverändert (Abschnitt 4) |

- `orchestrator` ist die Kennung einer gestarteten Sitzung, `zusage` eine Kennung `^zs-[0-9]{1,15}-[1-9][0-9]{0,9}$`. Beide sind Felder des Vorgangs, nicht der Start-Struktur. Der Vertrag `start` ändert sich nicht, und `lauf` liest keine Zusage.
- `zusage` ohne `orchestrator` ist `stelle.eingabe_ungueltig` (Stufe 3).
- **Stufe 4**, nach `start::pruefen` und `stelle.sitzung_vergeben`, in dieser Reihenfolge, nur wenn `orchestrator` steht:
  1. `zusage` fehlt, steht nicht im Zustand oder ist abgelaufen: `stelle.zusage_ungueltig`.
  2. Die Zusage gehört zu einem anderen Orchestrator, oder `harness`, `modell`, `effort`, `route` oder `kunde` der Startdatei gleicht nicht genau dem Wert der Zusage: `stelle.zusage_ungueltig`. Fehlt `effort` (Start-Struktur Fassung 1), gleicht es nicht.
  3. Sonst startet die Sitzung wie in Abschnitt 4. Der Zustand merkt für sie den Orchestrator, und die Zusage ist verbraucht.
- Eine Ablehnung verbraucht die Zusage nicht; sie gilt bis zu ihrer Frist. Jede Ablehnung ist `sitzung.abgelehnt` mit dem Code (Abschnitt 7.2). Die Zeile ändert sich nicht.
- **Grenze.** Lässt ein fehlerhafter Dienst `orchestrator` und `zusage` weg, ist es für die Stelle eine gewöhnliche Sitzung. Die Stelle bindet den Worker nur, wenn der Dienst ihn als Worker nennt. Die Gegenprobe `worker_ohne_frei` des Dienstes prüft die andere Seite.

### 15.3 `worker.anfordern`, geändert

| Vorgang | Aufrufer | Eingabe (geschlossen) | Antwort bei Erfolg |
|---|---|---|---|
| `worker.anfordern` | `kern` | `{"sitzung", "rolle", "harness", "modell", "effort", "laufend", "route", "kunde"}` | `{"ok":true,"ergebnis":"frei","sitzung":…,"zusage":…,"zusage_bis_ms":…}` |

- `route` und `kunde` sind genau die Werte, die der Dienst danach startet: Er baut die Start-Struktur des Workers aus der des Orchestrators (`route`, `kunde`, `ordner` und `projekt` gleich) und setzt `harness`, `modell` und `effort` aus der Meldezeile. `route` ist eine Kennung, `kunde` eine Kennung oder `null`.
- **`modell` hat die Form von `modell` aus Vertrag 1**: 1 bis 128 Zeichen aus `[A-Za-z0-9._:/@+-]`. Das gilt auch für die Liste `modelle` der Vorgabe. Grund: Echte Modellnamen wie `qwen3:8b` oder `claude-sonnet-5.5` sind keine Kennungen. Die Form von 14.5 (Kennung) gilt für `modell` nicht mehr.
- **Stufe 4**, in dieser Reihenfolge; sie ersetzt die Reihe von 14.5:
  1. Regeln fehlen: `stelle.vorgabe_fehlt`.
  2. `sitzung` nicht gestartet: `stelle.sitzung_unbekannt`.
  3. `sitzung` ist selbst ein Worker (gestartet mit `orchestrator`): `stelle.orchestrator_ist_worker`. Es gibt nur eine Ebene.
  4. `route` steht nicht in `routen` der Einstellungen: `stelle.route_unbekannt`.
  5. Die Klasse von `route` steht in `klassen` hinter der Klasse der Route des Orchestrators: `stelle.worker_klasse_hoeher`. Verglichen werden nur Positionen in `klassen` (Vertrag 2, die niedrigste zuerst). Gleiche oder niedrigere Klasse ist erlaubt.
  6. `kunde` ist nicht genau der Kunde des Orchestrators, auch `null` gegen einen Kunden oder ein Kunde gegen `null`: `stelle.worker_fremder_kunde`.
  7. `harness` nicht in `harness`: `stelle.harness_nicht_erlaubt`.
  8. `modell` nicht in `modelle`: `stelle.modell_nicht_erlaubt`.
  9. `effort` nicht in `effort`: `stelle.effort_nicht_erlaubt`.
  10. `laufend` nicht kleiner als `hoechstzahl`: `stelle.worker_hoechstzahl`.
  11. Der Zähler der Zusagen steht bei 9 999 999 999: `stelle.zusagen_erschoepft` (wie `stelle.anfragen_erschoepft` in Abschnitt 4; nur in der Bibliothek geprüft, kein Fall).
  12. Sonst `frei`. Die Zusage bekommt die Kennung `zs-<laden_ms>-<n>`; n zählt ab 1, nur ausgestellte Zusagen zählen. Sie gilt bis `jetzt_ms + 30 000` (V). Der Zustand speichert Orchestrator, `harness`, `modell`, `effort`, `route` und `kunde`.
- `frei` schreibt weiter keine Zeile; jede Ablehnung schreibt `vorgang.abgelehnt` mit `anfrage: null`.
- **Fristen.** Stufe 1 (Abschnitt 5) entfernt abgelaufene Zusagen, ohne Zeile. Eine Zusage gilt, solange `jetzt_ms` kleiner als ihr Fristende ist. Ein Neustart verwirft alle Zusagen (E1), und ihre Kennungen tragen das neue `laden_ms`.
- Die Prüfung von Klasse und Kunde ist eine Sicherheitsgrenze, anders als die Höchstzahl (14.5). Darum prüft die Stelle sie bei `worker.anfordern` und bindet das Ergebnis mit der Zusage an den Start.

### 15.4 Neue Codes (stehen in Abschnitt 8)

| Code | Grund (fest, ASCII) | Stufe |
|---|---|---|
| `stelle.zusage_ungueltig` | `Zusage fehlt oder passt nicht` | 4 |
| `stelle.orchestrator_ist_worker` | `Orchestrator ist selbst ein Worker` | 4 |
| `stelle.route_unbekannt` | `Route unbekannt` | 4 |
| `stelle.worker_klasse_hoeher` | `Klasse hoeher als beim Orchestrator` | 4 |
| `stelle.worker_fremder_kunde` | `Kunde weicht vom Orchestrator ab` | 4 |
| `stelle.zusagen_erschoepft` | `Zusagen erschoepft, Neustart noetig` | 4 (nur in der Bibliothek geprüft, kein Fall) |

### 15.5 Fälle (stehen in Abschnitt 9)

| Fall | Zeigt |
|---|---|
| `worker-frei`, `worker-gruende` (geändert) | `frei` mit Zusage und Frist; jede Stufe von 15.3 mit ihrem Code |
| `worker-klasse` | gleiche und niedrigere Klasse frei, höhere `stelle.worker_klasse_hoeher`; eine lokale Route höherer Klasse ebenso |
| `worker-kunde` | gleicher Kunde und beide `null` frei; anderer Kunde, `null` gegen Kunde und Kunde gegen `null` abgelehnt |
| `worker-nur-eine-ebene` | ein gestarteter Worker fordert an: `stelle.orchestrator_ist_worker` |
| `worker-modellname` | `qwen3:8b` und `claude-sonnet-5.5` als `modell` sind gültig; ein Leerzeichen ist `stelle.eingabe_ungueltig` |
| `worker-start-mit-zusage` | Start mit passender Zusage, danach ist sie verbraucht (zweiter Start `stelle.zusage_ungueltig`) |
| `worker-start-zusage-gruende` | ohne Zusage, unbekannt, abgelaufen (Frist genau erreicht), fremder Orchestrator, je ein abweichendes Feld, `effort` fehlt, `zusage` ohne `orchestrator` |
| `worker-zusage-neustart` | nach einem Neustart ist eine alte Zusage `stelle.zusage_ungueltig` |

Gegenproben für den Mutationslauf: `klasse_nicht_geprueft`, `kunde_nicht_geprueft`, `zusage_optional` (Start mit `orchestrator` ohne Zusage gelingt), `zusage_zweimal`, `zusage_ohne_frist`, `zusage_feld_ungeprueft` (je Feld). Jede muss einen roten Fall ergeben.

### 15.6 Offen

- Die Zeile `sitzung.gestartet` nennt den Orchestrator nicht. Ob sie es soll, entscheidet Vertrag 5 mit dem Inspektor; bis dahin ändert sich die Zeile nicht.
- Der Dienst baut den Aufruf von 15.1 und die Felder von 15.2 und 15.3 (Vertrag `dienst`). Die Meldezeile muss `modell` in der Form von 15.3 zulassen.
- Befund aus dem Bau (27.11.2026): `effort` von `worker.anfordern` ist eine Kennung, nie `null`, und ein fehlendes `effort` gleicht nach 15.2 nie. Ein Worker startet darum nur mit einer Startdatei der Fassung 2 (Vertrag 1) mit genau diesem `effort`, und nur mit Einstellungen, deren Block `effort` (Vertrag 2) den Wert für Harness, Klasse und Modell erlaubt. Die Fälle von 15.5 haben dafür Einstellungen mit einem Block `effort` und einer dritten Route `cloud-offen` der Klasse `oeffentlich`. Beide Vergleichsmodelle rechnen eine Startdatei der Fassung 2 nur für gültige Starts mit Effort nach (`name` und `fortsetzung_von` `null`); jede andere bricht das Skript ab, statt eine Erwartung zu raten. Die Prüfung im Bau macht das Crate `start`.
- Befund aus dem Bau (27.11.2026): `bau-faelle.mjs` hat jetzt den Schalter `--pruefen` wie `fassung2/bau-faelle.mjs` (im Speicher erzeugen, nichts schreiben, Byte für Byte mit `faelle/` vergleichen).

<!-- vom Bauvorgang angefuegt -->
> Hinweis: Dieser Text nennt Werkzeuge, die in diesem Klon nicht liegen: `brain`.
> Sie gehoeren zum Setup der Maschine, aus der dieser Baum stammt; hier fehlen
> sie mit Absicht. Wer sie braucht, schreibt sich seine eigenen.
> *Named here, not shipped here.*
