# Vertrag 1: Start-Struktur und Aufruf „Lauf starten“ (Fassung 1, Stand 23.11.2026)

Stand 23.11.2026 (Nachbesserung gegen den Code am Stand vom 23.11.2026), Durchstich Schritt 1. Der Text trägt Nachträge vom 20.11. bis 23.11. Nachtrag 2 vom 20.11. (nach Entscheidungen des Betreibers und der Leitung):
`copilot` fragt vor Werkzeugen weiter zurück, außer für die Werte aus Vertrag 2, Block `harness`; die Ordnermarke trägt eine
Kennung und Lösen und Senken schreiben eine Protokollzeile (Vertrag 5, Abschnitt 6.4, OPEN 13 geschlossen); der
Zustandsordner der Installation ist festgelegt und „Lauf starten“ schreibt je Cloud-Sitzung die Zielliste des Modellwegs.
Der Nachtrag vom 20.11. ergänzt die Harness-Tabelle um `copilot`, legt die
Startform je Harness fest und führt den Modellweg, die Netzgrenze E5, die Eigenschaft E4 und die Ordnermarke E6 ein. Ein
weiterer Nachtrag vom 20.11. legt den Starter des Einschlusses (Variante A) mit Aufruf, Codes und Beschreibung fest und
schließt OPEN 15. Die Start-Struktur selbst bleibt Fassung 1; ihre Form und
ihre Codes ändern sich nicht. Das Produkt legt die Form fest. Der Kern-Dienst liest die Startanfrage der
Ansicht und schreibt die Startdatei. Das Programm „Lauf starten“ liest die Startdatei. Die Wissenssuche bekommt daraus
nur `sitzung` und `route` (Vertrag 4).

Der Vertrag steht vor dem Code. Die Crates `start` und `lauf` bestehen; `rust/crates/start/tests/faelle.rs` und
`rust/crates/lauf/tests/faelle.rs` fahren die sichtbaren Fälle (der zweite nur im Prüfbau unter Linux). Verdeckte Fälle folgen.
Die Mutanten-Fälle werden noch nicht ausgeführt: Der Läufer `rust/crates/lauf/tests/faelle.rs` lässt sie aus (`AUSGELASSEN`). Entscheid des Betreibers, 23.11.2026: Sie werden ausgeführt; den Läufercode bringt ein späterer Code-Antrag (OPEN 18).

## Dateien

| Datei | Zweck |
|---|---|
| `README.md` | dieser Vertragstext |
| `start.schema.json` | Form als JSON Schema 2020-12: `$defs/anfrage` und `$defs/datei` |
| `faelle/<name>.json` | `beschreibung`, `form` (`anfrage`, `datei` oder `aufruf`); bei `anfrage` und `datei` der Start als `text` (UTF-8) oder `b64` (rohe Bytes) und die Einstellungsdatei als `einstellungen`; bei `aufruf` die Ausgangslage als `lage` |
| `erwartet/<name>.json` | `{"gueltig": true, "route_art": "cloud"}` oder `{"gueltig": false, "code": "start.<code>"}`; bei `aufruf` Exitcode und stderr-Zeile oder `exec` |
| `bau-faelle.py` | erzeugt `faelle/` und `erwartet/` |
| `pruefen.py` | prüft Paarung, Größe, JSON-Lesen und Schema gegen die Fälle |

Die eingebettete Einstellungsdatei ist die Basis der Fälle aus Vertrag 2 (`VORFUEHRUNG` in
`rust/spec/einstellungen/bau-faelle.py`, Fassung 1), nicht die Vorführdatei. Die Aufruf-Fälle zur Netzgrenze nehmen
dieselbe Basis in Fassung 2 (`EINSTELLUNGEN_2`: Netzziel `api.cloud.example:443` an der Cloud-Route, Ziel
`http://127.0.0.1:11434/v1` und Modell `qwen3:8b` an der lokalen Route).

## Zwei Formen

**Startanfrage** (Ansicht an Kern-Dienst): enthält kein `sitzung`. Schickt die Ansicht `sitzung` mit, ist die Anfrage
ungültig (`start.sitzung_nicht_erlaubt`). So wählt kein Browser die Kennung einer Sitzung.

**Startdatei** (Kern-Dienst an „Lauf starten“): dieselben Felder und `sitzung`. Der Kern-Dienst setzt `sitzung` ein.
Er vergibt `s-1`, `s-2` und so weiter und verwendet keine Kennung wieder.

Beide Formen: ein JSON-Objekt, UTF-8 ohne BOM, höchstens 4096 Byte. Geschlossene Form: Ein unbekannter Schlüssel macht
den Start ungültig. Ein doppelter Schlüssel im JSON macht ihn ungültig. Die Reihenfolge der Schlüssel ist frei.

**Kennung** heißt: Text der Form `^[a-z0-9][a-z0-9._-]{0,63}$` (1 bis 64 Zeichen), wie in Vertrag 2 und in der
Protokollzeile.

| Feld | Form | Typ | Grenzen | Bedeutung |
|---|---|---|---|---|
| `fassung` | beide | Ganzzahl | genau `1`, als Zahlwort ohne Bruchteil und Exponent: `1.0`, `1e0`, `1.00` sind `start.typ_falsch`, `-0` ist die Ganzzahl 0 und damit `start.fassung_unbekannt` | kommt schon in der Anfrage mit |
| `sitzung` | nur Datei | Kennung | | vom Kern-Dienst vergeben |
| `harness` | beide | Kennung | muss in der Harness-Tabelle stehen | welcher Harness startet |
| `modell` | beide | Text | 1 bis 128 Zeichen aus `[A-Za-z0-9._:/@+-]` | Modellname, den der Harness bekommt; keine Leerzeichen, keine Option |
| `route` | beide | Kennung | muss in `routen` der Einstellungsdatei stehen | Route der Sitzung, Pflicht (A43) |
| `kunde` | beide | Kennung oder `null` | nicht `null`: muss in `kunden` stehen | Kundenbezug der Sitzung; `null` heißt ohne Kundenbezug. Das Feld ist Pflicht, `null` muss ausdrücklich stehen. |
| `arbeitsordner` | beide | Text | siehe unten | Ordner, in dem der Harness arbeitet |
| `marker` | nur Datei, optional | Objekt | siehe „Marker“ | Zählung von Markern im Probelauf; nur der Kern-Dienst setzt es |

**arbeitsordner:** absoluter, normalisierter Pfad, höchstens 1024 Byte UTF-8. Er beginnt mit `/`, enthält kein `//`,
endet nicht mit `/`, hat kein Segment `.` oder `..` und kein Steuerzeichen (U+0000 bis U+001F und U+007F bis U+009F, also auch die C1-Steuerzeichen). Die Wurzel `/`
allein ist ungültig. Namen wie `.versteckt` oder `...x` sind gewöhnliche Segmente.

**Harness-Tabelle, Fassung 1 mit Nachtrag 20.11.2026:**

| harness | Art | erlaubte Routenart | Startbefehl im Einschluss | in der Vorführung |
|---|---|---|---|---|
| `claude` | Cloud-Harness | nur `cloud` | `claude --model <modell>` | nein, bleibt als Eintrag |
| `copilot` | Cloud-Harness | nur `cloud` | `copilot --model <modell>` (bei `modell` gleich `auto`: `copilot --model auto --auto-tier efficiency`) `--disable-builtin-mcps --additional-mcp-config @<heim>/.copilot/wb-mcp.json` und je Wert aus `harness.copilot.werkzeuge_ohne_rueckfrage` (Vertrag 2) `--allow-tool <wert>` | ja, Cloud-Route |
| `pi` | lokaler Harness | nur `lokal` | `pi --provider wb-lokal --model <modell>` | ja, lokale Route |

Ein Cloud-Harness läuft nur mit einer Cloud-Route, ein lokaler Harness nur mit einer lokalen Route. Sonst gilt
`start.harness_route_unvertraeglich`. Grund: Die Route bestimmt die Klasse, die die Sitzung sehen darf. Ein lokaler
Harness auf einer Cloud-Route oder `claude` oder `copilot` auf einer lokalen Route würde Daten an ein Ziel schicken,
dessen Art nicht zur Route passt. Weitere Harnesses kommen über eine neue Zeile in dieser Tabelle dazu.

**Startform.** Der Startbefehl ist der vollständige argv des Harness. `<modell>` ist genau ein argv-Element mit dem
geprüften Wert aus der Startdatei; es gibt keine Shell und keine weiteren Optionen aus der Startdatei. Den Pfad des
Programms nimmt „Lauf starten“ aus seiner eigenen Konfiguration. Der Harness bekommt in seiner Umgebung `TERM`, ein
festes `PATH` und `HOME` = `<heim>`. `<heim>` ist der Pfad des Heims des Nutzers auf dem Wirt, `heim` der eigenen Konfiguration des
Programms. Im Einschluss ist `<heim>` ein leeres, privates tmpfs am selben Pfad. Der Zustandsordner der Sitzung liegt darunter und
heißt je Harness `.pi` (`pi`), `.copilot` (`copilot`) oder `.claude` (`claude`); er ist beim Start leer. Sonst liegt unter `<heim>`
nur, was der Vertrag nennt: bei `pi` die Datei `.pi/agent/models.json`, bei `copilot` die Datei `.copilot/wb-mcp.json`, beide nur lesend
eingebunden. Das Heim des Wirts bleibt außerhalb.

- **pi** (gemessen: `pi --version` ergibt `1.5.2`; `pi --help` nennt `--provider`, `--model` und
  `PI_CODING_AGENT_DIR`, Vorgabe `~/.pi/agent`). Vor dem exec schreibt das Programm die Anbieter-Definition nach
  `<sitzungen>/<sitzung>/konfig/models.json`, Modus 0600; der Einschluss bindet sie nur lesend als
  `<heim>/.pi/agent/models.json` ein (`dateien` der Beschreibung):

  ```json
  {"providers": {"wb-lokal": {"baseUrl": "http://127.0.0.1:11434/v1", "api": "openai-completions",
   "apiKey": "wb-ohne-schluessel", "models": [{"id": "<modell>"}]}}}
  ```

  `baseUrl` ist immer der Gastpunkt des Modellwegs im Einschluss (E5), nie das Ziel der Route. `<modell>` muss
  gleich `modell` der Route sein (Stufe 15). `apiKey` ist ein fester Text und kein Schlüssel. Dazu setzt das
  Programm `PI_OFFLINE=1` und `PI_TELEMETRY=0`. Die Feldnamen stammen aus einer vorhandenen pi-Konfiguration auf dem
  Entwicklungsrechner (gelesen, ohne Schlüssel); ein Start damit ist noch nicht gemessen (OPEN 8).
- **copilot** (Schlüsselweg Option (a), Projektentscheid, 20.11.2026; Messung zum Token der Copilot CLI mit 1.0.88, der
  dort eingesetzten Version). Der Starter setzt in der Umgebung des Harness (aus dem Feld `cloud` der Beschreibung, nur wenn es gesetzt ist;
  `einschluss/src/profil.rs:513-522`):
  - `COPILOT_GITHUB_TOKEN` = der Platzhalter: der feste Text `gho_` und 36 Zeichen `0`. Er ist kein Schlüssel; der
    Modellweg verwirft ihn und setzt an gebundenen Zielen den echten Schlüssel ein (E4). Gemessen: Die CLI sendet einen
    Platzhalter im `gho_`-Format ohne eigene Prüfung (t2, u1).
  - `HTTPS_PROXY=http://127.0.0.1:8402`, der Gastpunkt des Modellwegs (E5). Gemessen: Mit Proxy löst die CLI keinen
    Namen selbst auf (dns=0) und öffnet keine direkte Verbindung.
  - `COPILOT_AUTO_UPDATE=false` (die Update-Abfrage entfällt).
  - `COPILOT_MASCOT_BLINK=0` (ruhiges Bild: die Figur der CLI blinkt nicht; keine Wirkung auf Netz oder Schlüssel).

  Dazu kommen die Optionen `--disable-builtin-mcps` (der eingebaute GitHub-MCP entfällt) und
  `--additional-mcp-config @<heim>/.copilot/wb-mcp.json`. Diese Datei schreibt das Programm vor dem exec, Modus 0600.
  Sie nennt als einzigen MCP-Server die Wissenssuche nach Vertrag 4.
  **Modell `auto` (Abstimmung, 20.11.2026).** Fehlt `modell` an der Cloud-Route (Vertrag 2), wählt die
  Seite `auto`, und der Start trägt `modell` gleich `auto`; Stufe 15 prüft dann nicht, weil die Route kein `modell` trägt.
  Bei `modell` gleich `auto` hängt das Programm für `copilot` genau `--model auto --auto-tier efficiency` an, sonst
  `--model <modell>`. Grund (gemessen mit 1.0.88 am 20.11.): Diese Form wählte `gpt-6-luna` und verbrauchte eine
  Chat-Einheit, kein Premium-Modell. Fälle: `aufruf-copilot-auto` und `aufruf-gueltig-copilot` (festes Modell).
  **Rückfrage vor Werkzeugen (Projektentscheid, 20.11.2026).** Die Copilot CLI fragt vor Werkzeugen zurück; das bleibt die
  Vorgabe. Nur Vertrag 2, Block `harness`, schaltet die Rückfrage ab, und nur für Läufe im Einschluss (Cloud-Route; ein
  Lauf von `copilot` außerhalb des Einschlusses gibt es nicht). Das Programm hängt für jeden Wert der Liste
  `harness.copilot.werkzeuge_ohne_rueckfrage` genau ein Paar `--allow-tool <wert>` an den Startbefehl, in der Reihenfolge der
  Datei, nach `--additional-mcp-config @<heim>/.copilot/wb-mcp.json`, und sonst nichts. Die erlaubten Werte sind `shell`,
  `write` und `such-mcp`; jeder andere Wert ist schon in Schritt 3 ein Ladefehler (`einst.werkzeug_wert`), das Programm
  sieht ihn nie. Das Programm hängt nie eine Option `--allow-all`, `--allow-all-tools`, `--allow-all-paths`,
  `--allow-all-urls` oder `--yolo` an, auch nicht auf Wunsch der Startdatei, der Umgebung oder des Agenten. Fehlt der Block
  oder ist die Liste leer, bleibt der Startbefehl ohne `--allow-tool`. Für `claude` und `pi` ändert der Block nichts.
  Im Einschluss gibt es kein `gh`, keinen
  D-Bus-Socket des Wirts und keine `~/.copilot/config.json` mit einem Schlüssel. Gemessen (t6): Ist `gh` erreichbar,
  holt sich die CLI dessen Schlüssel selbst. Die Anmeldung mit dem Organisationskonto läuft außerhalb des Einschlusses
  (`copilot login --device-code`); der Schlüssel landet in der Schlüsseldatei des Modellwegs, nie im Einschluss.
- **claude** wird nicht vorgeführt. Die Zeile bleibt, damit vorhandene Fälle und Starts gültig bleiben. `claude` hat in
  Fassung 1 keinen Ausgang: Auf einer Cloud-Route schreibt das Programm kein Bundle und setzt kein `cloud` (Code:
  `ablauf.rs:754-765`, `:913-921`); es gibt kein `127.0.0.1:8402`, kein Bundle und keinen Proxy. Der Start gelingt, jede
  Netzanfrage scheitert. Aus dem Code hergeleitet, nicht gemessen. Entwurf E7 (Außentest, unten).

**Zeichenregel von `modell` gegen echte Namen (gemessen 20.11.).** Alle 28 Modellnamen aus `copilot help config`
(etwa `claude-sonnet-5.5`, `gpt-5.6-sol`, `gemini-3.8-flash`) und alle 19 Namen aus `ollama list` auf dem
Entwicklungsrechner passen auf `^[A-Za-z0-9._:/@+-]{1,128}$`. Darunter sind Namen mit `/`, `_`, `:` und
Großbuchstaben (Form `namensraum/qwen3:4b-q4_K_M`). Die Regel bleibt unverändert.

### Die Einträge des Durchstichs

```json
{"fassung": 1, "harness": "claude", "modell": "claude-sonnet-5-5", "route": "cloud-beispiel",
 "kunde": "k-beispiel", "arbeitsordner": "/home/beispiel/projekt"}
{"fassung": 1, "harness": "pi", "modell": "qwen3:8b", "route": "lokal-beispiel",
 "kunde": "k-beispiel", "arbeitsordner": "/home/beispiel/projekt"}
{"fassung": 1, "harness": "copilot", "modell": "claude-sonnet-5.5", "route": "cloud-beispiel",
 "kunde": "k-beispiel", "arbeitsordner": "/home/beispiel/projekt"}
```

Als Startdatei steht zusätzlich `"sitzung": "s-1"` (beziehungsweise `"s-2"`, `"s-3"`). In der Vorführung laufen
`copilot` auf der Route `copilot` und `pi` auf der Route `lokal` der Vorführdatei (Vertrag 2).

### Entscheidung zu `kunde: null`

`null` ist im Durchstich erlaubt. Gründe: Interne Arbeit ohne Kundenbezug ist der Normalfall; ein erfundener Kunde wäre
schlechter als `null`. Das Tor `darf(einheit, route, kunde)` nutzt den Kunden der Einheit (der Notiz), nicht den der
Sitzung (Vertrag 3). Das Feld `kunde` im Start wird im Durchstich von `darf` nicht benutzt. Es bleibt, weil
der Kundenraum je Sitzung (A53) später darauf aufbaut.

### Die Route ist für den Agenten nicht änderbar

Die Route steht in der Startdatei. Die Datei liegt im Ordner des Kern-Dienstes (Modus 0700) mit Modus 0600, also
außerhalb des Einschlusses. Der Agent sieht weder die Datei noch die Einstellungsdatei. Die Suche bekommt die Route
beim Start ihres Prozesses als Argument, nie aus einer Anfrage des Agenten und nie aus einer Umgebungsvariable.

### Marker (Nachtrag 20.11.2026, Probelauf M1)

`marker` ist ein optionales Objekt der Startdatei: `{"liste": [<Marker>, ...], "ordner": <Pfad>}`, beide Felder Pflicht,
geschlossene Form.
- `liste`: 1 bis 16 Marker, je 1 bis 32 Zeichen aus `[A-Z0-9-]`, ohne Doppel. Das Komma ist ausgeschlossen, weil die
  Option `--marker` die Liste mit Komma trennt.
- `ordner`: der Zählordner, ein absoluter Pfad nach denselben Regeln wie `arbeitsordner`.
- Jeder Formfehler in `liste` oder `ordner` ergibt `start.marker_form`. Unbekannte oder fehlende Schlüssel und falsche
  Typen ergeben die allgemeinen Codes der Stufen 5 bis 7.

Nur der Kern-Dienst setzt das Feld, und nur, wenn `werkbank-probelauf` ihn mit Markerliste und Zählordner gestartet
hat. In der Startanfrage der Ansicht ist es verboten (`start.marker_nicht_erlaubt`), damit kein Browser eine Zählung in
einen Ordner seiner Wahl lenkt. Gezählt wird nur, nie werden Bytes mitgeschnitten (Projektentscheid, 20.11.2026). Ohne das
Objekt bekommt kein Prozess eine Marker-Option.

**Der Lauf ist als Probelauf gekennzeichnet** (Projektentscheid, 20.11.2026). Die Zählung gibt es nur in der
Betriebsart `probelauf` von Suche und Modellweg; sie steht im Protokoll des Laufs, und jede Protokollzeile dieser
Sitzung trägt die Kennzeichnung als Probelauf. Deshalb übergibt „Lauf starten“ beiden Prozessen genau dann
`--probelauf --marker <a,b,...> --marker-zaehlung <datei>`, wenn die Startdatei `marker` trägt, und sonst keine der
drei Optionen. Fehlt bei Suche oder Modellweg eine der drei, endet ihr Aufruf mit einem Fehler (Suche Exit 64,
Modellweg Exit 64; bis 21.11.2026 endete die Suche mit 2); das ergibt hier `lauf.suche_nicht_bereit` bzw. `lauf.modellweg_nicht_bereit`.

## Fehlercodes und Prüfreihenfolge

Die Prüfung meldet genau einen Code: den der ersten verletzten Stufe. Innerhalb einer Stufe ist der Code derselbe,
gleich welche Stelle verletzt ist. Die Codes und Gründe sind feste Texte. Der Kern-Dienst reicht sie unverändert an die
Ansicht durch.

| Stufe | Code | Verletzung | Grund (fest, ASCII) |
|---|---|---|---|
| 1 | `start.zu_gross` | mehr als 4096 Byte | `Start zu gross` |
| 2 | `start.json_ungueltig` | kein UTF-8, BOM, Syntaxfehler, doppelter Schlüssel, oberste Ebene kein Objekt, Text nach dem Objekt, mehr als 127 Verschachtelungsebenen | `kein gueltiges JSON` |
| 3 | `start.sitzung_nicht_erlaubt` | nur Anfrage: Schlüssel `sitzung` vorhanden | `Sitzung vergibt der Dienst` |
| 4 | `start.marker_nicht_erlaubt` | nur Anfrage: Schlüssel `marker` vorhanden | `Marker setzt der Dienst` |
| 5 | `start.schluessel_unbekannt` | ein Schlüssel außerhalb der Form (auch in `marker`) | `unbekannter Schluessel` |
| 6 | `start.schluessel_fehlt` | ein Pflichtschlüssel fehlt (in der Datei auch `sitzung`; in `marker` `liste` oder `ordner`) | `Pflichtfeld fehlt` |
| 7 | `start.typ_falsch` | falscher JSON-Typ (etwa `route: null`, `fassung` als Text oder `1.0`, `marker` kein Objekt) | `falscher Typ` |
| 8 | `start.fassung_unbekannt` | `fassung` ist eine Ganzzahl, aber nicht `1` | `Fassung unbekannt` |
| 9 | `start.kennung_form` | `sitzung`, `harness`, `route` oder `kunde` verletzt die Kennungsform | `Kennung hat falsche Form` |
| 10 | `start.modell_form` | `modell` verletzt Länge oder Zeichenvorrat | `Modell hat falsche Form` |
| 11 | `start.ordner_ungueltig` | `arbeitsordner` verletzt die Pfadregeln | `Arbeitsordner ungueltig` |
| 12 | `start.marker_form` | `marker.liste` (Anzahl, Länge, Zeichen, Doppel) oder `marker.ordner` (Pfadregeln) verletzt die Form | `Marker haben falsche Form` |
| 13 | `start.harness_unbekannt` | `harness` steht nicht in der Harness-Tabelle | `Harness unbekannt` |
| 14 | `start.route_unbekannt` | `route` steht nicht in der Einstellungsdatei | `Route unbekannt` |
| 15 | `start.modell_route` | die Route trägt ein `modell` (Einstellungen der Fassung 2: jede lokale Route, eine Cloud-Route nur, wenn das Feld steht) und `modell` des Starts ist nicht genau gleich | `Modell passt nicht zur Route` |
| 16 | `start.kunde_unbekannt` | `kunde` ist nicht `null` und steht nicht in der Einstellungsdatei | `Kunde unbekannt` |
| 17 | `start.harness_route_unvertraeglich` | Art der Route passt nicht zum Harness | `Harness passt nicht zur Route` |

Die Stufen 1 bis 13 brauchen nur den Start. Die Stufen 14 bis 17 brauchen eine gültige Einstellungsdatei nach Vertrag 2.
Der Nachtrag fügt die Stufen 4, 12 und 15 ein (Marker: Projektentscheid, 20.11.2026; Modell der Route: Projekt mit
Projektentscheid, 20.11.2026). Die übrigen Stufen behalten ihre Codes und ihre Reihenfolge; nur ihre Nummern rücken. Ohne
`marker` und mit Einstellungen der Fassung 1 ergibt jeder alte Fall denselben Code wie vorher. Mit Einstellungen der
Fassung 1 greift Stufe 15 nie,
denn dort trägt keine Route ein Modell. Grund: Der Modellweg lässt nur das Modell der Route durch und prüft selbst nur
gegen `--modell`. Ein anderes Modell im Start ergäbe eine Sitzung, deren
Harness jede Anfrage abgewiesen bekäme.

## Schnittstelle der Prüfstelle

Eine Prüfstelle, zwei Aufrufer. Eine Bibliothek (Crate `start` unter `rust/crates/start`) liest und prüft. Der
Kern-Dienst ruft sie vor dem Start des Kindes und lehnt mit dem Code an die Ansicht ab. Das Programm „Lauf starten“
prüft mit derselben Bibliothek noch einmal (zweite Linie). Dieser Vertrag beschreibt die Schnittstelle; sie besteht in `rust/crates/start/src` (`lesen.rs`, `pruefen.rs`, `form.rs`,
`schreiben.rs`).

- `lesen_anfrage(bytes: &[u8]) -> Result<Startanfrage, Fehler>`: Stufen 1 bis 13 für die Anfrage.
- `lesen_datei(bytes: &[u8]) -> Result<Startdatei, Fehler>`: Stufen 1 bis 13 für die Datei (ohne Stufen 3 und 4).
- `pruefen(start: &Startdatei oder &Startanfrage, einstellungen: &Einstellungen) -> Result<Routenart, Fehler>`:
  Stufen 14 bis 17; `Einstellungen` kommt aus dem Crate `einstellungen` (Vertrag 2).
- `Startanfrage::mit_sitzung(self, sitzung, marker: Option<Marker>) -> Result<Startdatei, Fehler>`: der Kern-Dienst
  setzt die Kennung und, nur im Probelauf, die Marker ein.
- `schreiben(&Startdatei) -> Vec<u8>`: Bytes der Startdatei, die `lesen_datei` wieder annimmt.
- `Fehler::code() -> &'static str` und `Fehler::grund() -> &'static str` geben Code und Grund aus der Tabelle.

## Aufruf „Lauf starten“

Der Kern-Dienst startet ein Programm als Kind in seiner PTY mit `argv = [<programm>, <pfad zur startdatei>]` oder,
mit Meldung (Abschnitt „Meldung an den Dienst“), `argv = [<programm>, --meldung, <pfad des sockets>, <pfad zur startdatei>]`. Die Datei
liegt im 0700-Ordner des Dienstes, Modus 0600. Die Umgebung vom Dienst ist leer bis auf `TERM` (env_clear plus TERM);
alles Weitere setzt das Programm selbst. stdin, stdout und stderr des Programms sind die PTY. Die Umsetzung gibt es für
Linux; zu anderen Systemen siehe „Plattformen in Fassung 1“.

Den Pfad der Einstellungsdatei, des Vaults, des Protokollordners und den Ort des Socket-Ordners nimmt das Programm aus
seiner **eigenen** Konfiguration, nie aus der Startdatei, nie aus der Umgebung und nie vom Agenten.

### Plattformen in Fassung 1

Fassung 1 läuft nur auf Linux mit Variante A. Unter macOS und jedem anderen System laufen die Schritte 0 bis 5c; Schritt 6
endet immer mit 69, `lauf.einschluss_fehlt` (`ablauf.rs:430`). Schritt 5b hat dann die Ordnermarke schon geschrieben, und sie
bleibt (E6). Es gibt dort weder Wächter (`kind.rs:129-135`) noch Subreaper noch Trampolin noch die Meldung über Deskriptor 4
(`uebergabe.rs`, `kind.rs`: Linux). Das Modul `ca` gibt es nur unter Linux (`lib.rs:16`). Die Eigenschaften E3 und E5, `bwrap`,
AppArmor und `pidfd` sind Linux-Merkmale. Die Aufruf-Fälle und `tests/meldung.rs` laufen nur unter Linux
(`rust/crates/lauf/tests/faelle.rs:12`). Unter macOS gilt für `sun_path` 104 Byte; der Code nutzt höchstens 103 (`ablauf.rs:18`).
Entwurf E8 (Außentest, unten).

### Ablauf vor dem exec

| Schritt | Prüfung oder Handlung | bei Fehler: Exit, Code |
|---|---|---|
| 0 | eigene Umgebung: keine der Variablen `SSL_CERT_FILE`, `SSL_CERT_DIR`, `OPENSSL_CONF` ist gesetzt, auch nicht leer; vor allem anderen | 78, `lauf.tls_umgebung` |
| 1 | genau ein Argument, oder `--meldung <pfad>` mit absolutem `<pfad>` und danach genau ein Argument; danach mit `--meldung` die Verbindung zum Socket des Dienstes (siehe „Meldung an den Dienst“) | 64, `lauf.aufruf`; 69, `lauf.meldung_fehlt` |
| 2 | Startdatei mit `O_NOFOLLOW` öffnen: reguläre Datei, Eigentümer ist die eigene Kennung, Modus genau 0600 | 66, `lauf.datei_modus`; nicht lesbar: 66, `lauf.datei_unlesbar` |
| 3 | zuerst die eigene Konfiguration lesen (siehe „Eigene Konfiguration“), dann die Einstellungsdatei lesen und nach Vertrag 2 prüfen | 78, `lauf.konfig_fehlt` oder `lauf.konfig_kaputt`; dann 78, `lauf.einst_unlesbar` oder der `einst.*`-Code |
| 4 | Startdatei mit `lesen_datei` und `pruefen` prüfen (zweite Linie) | 65, der `start.*`-Code |
| 4a | lokale Route: Die Route hat `ziel` und `modell` (fehlen bei Einstellungen der Fassung 1) | 78, `lauf.ziel_fehlt` |
| 5 | Arbeitsordner existiert und ist ein Ordner | 65, `lauf.ordner_fehlt` |
| 5a | nur mit `marker`: Zählordner mit `O_NOFOLLOW` öffnen; er existiert, ist ein Ordner, gehört der eigenen Kennung und hat Modus genau 0700 | 73, `lauf.marker_ordner` |
| 5b | Ordnermarke prüfen und schreiben (siehe „Ordnermarke“) | 78, `lauf.ordner_marke_kaputt` (auch die weiteren Wege unter „Prüfung in Schritt 5b“); 78, `lauf.ordner_marke_kennung`; 77, `lauf.ordner_anderer_kunde`; 77, `lauf.ordner_hoeher_markiert` |
| 5c | neben der Startdatei liegt weder `suche.pid` noch `modellweg.pid` (auch kein Verweis dieses Namens): Das Programm schreibt nie über eine Datei, und ein Ordner, den mehrere Sitzungen teilen (etwa das Heim), kann noch Dateien eines früheren Laufs tragen; kein Kind ist gestartet, die Datei bleibt | 73, `lauf.startordner_belegt` |
| 6 | Einschluss ist verfügbar (Variante A oder B). Der Code (`ablauf.rs:424-460`) prüft fünf Bedingungen: das System ist Linux; der Starter (im Prüfbau `bwrap`) ist ausführbar; Deskriptor 3 ist reservierbar; kein Deskriptor außer den eigenen der Meldung ist geerbt; bei `copilot` auf einer Cloud-Route ist die Verteilung bekannt, also der Ort des System-Bundles | 69, `lauf.einschluss_fehlt` |
| 6a | nur Harness `copilot`: Paket der CLI bereitstellen (siehe „Paket der Copilot CLI“): Form lesen, Cache-Ordner prüfen, unter Sperre prüfen und nötigenfalls im kleinen Einschluss vorentpacken; vor jedem Kind. Ist `harness.copilot` kein ausführbares Programm, überspringt der Schritt und Schritt 8 meldet `lauf.harness_fehlt` wie bisher (Quelle 1, siehe Schritt 8) | 78, `lauf.paket_form`; 73, `lauf.paket_cache`; 69, `lauf.paket_entpacken` |
| 7 | Socket-Ordner 0700 anlegen, Such-Prozess starten (siehe unten) und auf `{"bereit":true}` warten, Frist 10 s | 69, `lauf.suche_fehlt` (Start scheitert) oder `lauf.suche_nicht_bereit` (Frist verstrichen) |
| 7a0 | nur Cloud-Route: Zielliste des Modellwegs schreiben (siehe „Zustandsordner der Installation“), vor dem Start des Modellwegs; hat die Route kein Netzziel, das die Liste tragen kann, oder mehr als 16 (Vertrag 8, Abschnitt 12: `ziele[]` hat 1 bis 16 Einträge), oder hat sie ein Netzziel mit `zugriff` gleich `kopf-einsetzen` in der Unterdomänen-Form `*.name` (die Liste kennt kein `*`), endet der Schritt mit demselben Fehler, bevor eine Zielliste oder eine Wegwerf-CA entsteht; das Programm startet nie still ohne ein solches Ziel | 73, `lauf.zielliste` |
| 7a | Modellweg starten (siehe unten): bei lokaler Route Betriebsart `openai-lokal`, bei Cloud-Route `kopf-einsetzen` mit den Netzzielen; auf `{"bereit":true}` warten, Frist 10 s. Fehlt der Eintrag der Route in `schluessel` der eigenen Konfiguration (Schlüsseldatei), endet der Schritt bei einer lokalen Route immer, bei einer Cloud-Route nur mit gebundenem Ziel (`schluessel: true`) mit `lauf.modellweg_fehlt`, bei der Cloud-Route vor Schritt 7a0 (also ohne Zielliste und ohne CA); ohne gebundenes Ziel startet der Lauf mit leerem Schlüsselnamen (`ablauf.rs:670-675`, `:724-730`) | 69, `lauf.modellweg_fehlt` (Start scheitert) oder `lauf.modellweg_nicht_bereit` (Frist verstrichen) |
| 8 | Einschluss bauen, Zustandsordner und bei `pi` die Anbieter-Definition schreiben, Umgebung des Harness setzen, danach den Wächter starten (siehe „Lebensdauer“), den Startbefehl (Starter, im Prüfbau `bwrap`) als Kind über das Trampolin starten (siehe „Elternprozess des Starters“) | 69, `lauf.harness_fehlt`, aus zwei Quellen: (1) `lauf` selbst, wenn das Programm des Harness in `lauf.json` fehlt oder nicht ausführbar ist (`beschreibung_bauen`, `ablauf.rs:873-876`; nach Such-Prozess, Modellweg und PID-Dateien, vor dem Trampolin; die Kinder werden beendet), (2) das exec des Trampolins in den Starter scheitert (die Zeile schreibt das Trampolin); 73, `lauf.sitzung_belegt` (der Sitzungsordner `sitzungen/<sitzung>` besteht schon: ein Sitzungsname gilt einmal) |
| jeder | unerwarteter Fehler im Programm | 70, `lauf.intern` |

Scheitert ein Schritt nach Schritt 7, beendet das Programm den Such-Prozess (und ab Schritt 7a den Modellweg; eine schon
geschriebene Zielliste bleibt liegen) mit
SIGTERM und löscht danach die Dateien `ca.key`, `ca.crt`, `ca-bundle.pem`, `suche.pid` und `modellweg.pid` neben der Startdatei, den Socket-Ordner und den Sitzungsordner `sitzungen/<sitzung>` (nur wenn dieser Lauf ihn angelegt hat; ein Sitzungsordner, der schon bestand, ist `lauf.sitzung_belegt` und bleibt unberührt), alles bestmöglich: ein Fehler beim Löschen ändert den Code nicht. Dann endet es. Der Modellweg wird wie der Such-Prozess an den Harness gebunden
(Abschnitt „Lebensdauer“). Nach dem Start des Starters
gibt es keinen eigenen Exitcode mehr: Der Exit des Aufrufs ist der Code des Starters (nach dessen exec auf bwrap der des
Harness), und endet der Starter durch ein Signal, endet das Programm durch dasselbe Signal (Abschnitt „Elternprozess des
Starters“, Punkt 5). Die Exitcodes 64 bis 78 gelten bis zum Start des Starters; 69 `lauf.harness_fehlt` kommt aus Quelle 1 von `lauf` selbst vor dem
Trampolin, aus Quelle 2 danach vom Trampolin, wenn dessen exec in den Starter scheitert.

Die Exitcodes folgen `sysexits.h`: 64 `EX_USAGE`, 65 `EX_DATAERR`, 66 `EX_NOINPUT`, 69 `EX_UNAVAILABLE`, 70
`EX_SOFTWARE`, 73 `EX_CANTCREAT` (die Zähldateien oder die Zielliste lassen sich nicht anlegen, oder eine Datei oder ein Ordner der Sitzung besteht schon), 77 `EX_NOPERM` (die Ordnermarke
verbietet den Start), 78 `EX_CONFIG` (auch die eigene Konfiguration fehlt oder ist kaputt).

| Code | Grund (fest, ASCII) |
|---|---|
| `lauf.tls_umgebung` | `TLS-Umgebung gesetzt` |
| `lauf.aufruf` | `genau ein Pfad erwartet` |
| `lauf.datei_unlesbar` | `Startdatei nicht lesbar` |
| `lauf.datei_modus` | `Startdatei nicht eigen, nicht 0600 oder keine Datei` |
| `lauf.konfig_fehlt` | `Konfigurationsdatei fehlt` |
| `lauf.konfig_kaputt` | `Konfigurationsdatei nicht lesbar oder ungueltig` |
| `lauf.einst_unlesbar` | `Einstellungsdatei nicht lesbar` |
| `lauf.ziel_fehlt` | `Route ohne Ziel` |
| `lauf.ordner_fehlt` | `Arbeitsordner fehlt oder ist kein Ordner` |
| `lauf.marker_ordner` | `Zaehlordner fehlt, ist fremd, nicht 0700 oder ein Verweis` |
| `lauf.ordner_marke_kaputt` | `Ordnermarke unlesbar oder beschaedigt` |
| `lauf.ordner_marke_kennung` | `Ordnermarke mit ungueltiger oder doppelter Kennung` |
| `lauf.ordner_anderer_kunde` | `Ordner gehoert einem anderen Kunden` |
| `lauf.ordner_hoeher_markiert` | `Ordner hoeher markiert als die Route` |
| `lauf.startordner_belegt` | `Startordner enthaelt Dateien einer fruehen Sitzung` |
| `lauf.sitzung_belegt` | `Sitzung wurde schon gestartet` |
| `lauf.einschluss_fehlt` | `Einschluss nicht verfuegbar` |
| `lauf.suche_fehlt` | `Suche nicht startbar` |
| `lauf.suche_nicht_bereit` | `Suche nicht bereit` |
| `lauf.zielliste` | `Zielliste nicht schreibbar` |
| `lauf.modellweg_fehlt` | `Modellweg nicht startbar` |
| `lauf.modellweg_nicht_bereit` | `Modellweg nicht bereit` |
| `lauf.paket_form` | `Paket des Harness unbekannt oder nicht lesbar` |
| `lauf.paket_cache` | `Paketordner des Harness unbrauchbar` |
| `lauf.paket_entpacken` | `Paket des Harness nicht entpackt` |
| `lauf.harness_fehlt` | `Harness nicht startbar` |
| `lauf.meldung_fehlt` | `Meldung an den Dienst nicht moeglich` |
| `lauf.intern` | `interner Fehler` |

### Eigene Konfiguration (Schritt 3)

Das Programm liest seine eigene Konfiguration aus `/etc/werkbank/lauf.json` (der Prüfbau liest `lauf.json` neben dem Programm),
vor der Einstellungsdatei, weil deren Pfad darin steht. Das Paket schreibt die Datei beim Einrichten. Zwei feste Gründe,
beide Exit 78:

- `lauf.konfig_fehlt`: Die Datei gibt es nicht (auch nicht, weil ein Ordner davor fehlt).
- `lauf.konfig_kaputt`: Es gibt etwas an dem Pfad, aber es taugt nicht. Das gilt für jede Datei, die sich nicht öffnen oder
  lesen lässt, jede, die keine reguläre Datei ist, jede mit einem Eigentümer, der weder root noch die eigene Kennung ist, jede,
  die für Gruppe oder andere schreibbar ist, jede über 65536 Byte und jede, deren Inhalt die Form verletzt (kein JSON, ein
  unbekannter oder fehlender Schlüssel, ein falscher Typ, ein Pfad, der nicht absolut und normal ist, ein Harness außerhalb der
  Tabelle).

Beide Zeilen tragen weder Pfad noch Inhalt. Vorher endete jeder dieser Fälle als `lauf.intern` (70); `lauf.intern` bleibt für
unerwartete Fehler. Ein Fehler beim Einrichten des Pakets zeigt sich damit im Probelauf als einer der beiden Gründe.

### Schritt 0: die eigene Umgebung (Nachtrag 20.11.2026)

Der Weiterreicher des Modellwegs prüft Zertifikate gegen den System-Truststore. OpenSSL folgt aber `SSL_CERT_FILE`,
`SSL_CERT_DIR` und `OPENSSL_CONF` aus der Umgebung und würde die Prüfung auf eine andere Vertrauensbasis umlenken. Der
Kern-Dienst startet das Programm mit leerer Umgebung (nur `TERM`); die Ablehnung gilt für jeden anderen Aufrufer und ist
die zweite Linie.

- Vor Schritt 1 liest das Programm seine **eigene** Umgebung. Ist mindestens eine der drei Variablen gesetzt, endet es
  mit Exit 78, Code `lauf.tls_umgebung` und der festen stderr-Zeile. Gesetzt heißt vorhanden, auch mit leerem Wert.
- Die Zeile nennt weder den Namen noch den Wert der Variable. Ein Wert steht nie in einer Ausgabe oder einem Protokoll.
- Andere Namen ändern nichts (`SSL_CERT_FILES`, `OPENSSL_CONFX`, `ssl_cert_file`): der Start geht weiter. Namen sind
  genau, Groß- und Kleinschreibung zählt.
- Schritt 0 geht allen anderen Schritten vor, auch einem falschen Argument (Schritt 1) und einer fehlenden Startdatei.
  Es startet weder Suche noch Modellweg.
- Der Modellweg hat dieselbe Stufe 0 für sich selbst (Vertrag 8, Exit 78, `mw.umgebung_unsicher`). „Lauf starten“ und
  Modellweg prüfen unabhängig; das Entfernen der Variablen durch das Startprogramm ersetzt die Ablehnung nicht.
- Fälle: `aufruf-tls-umgebung-*` (jede Variable allein, leer, zwei zugleich, vor Schritt 1 und 2),
  `aufruf-gueltig-umgebung-aehnliche-namen`. In einem Fall nennt `lage.umgebung` die gesetzten Variablen.
- Mutanten (`aufruf-tls-umgebung-mutant-*`, je ein Fehler; jeder muss die Prüfung rot machen): `ohne-pruefung`,
  `nur-ssl-cert-file` (die Liste ist unvollständig), `leer-gilt-nicht` (leer zählt als nicht gesetzt),
  `wert-in-zeile` (die Zeile nennt den Wert). Die Mutanten-Fälle werden noch nicht ausgeführt (Läufer lässt sie aus, `AUSGELASSEN`); Entscheid des Betreibers, 23.11.2026: Sie werden ausgeführt, Läufercode folgt (OPEN 18).

### Verschachtelungstiefe (Nachtrag 20.11.2026)

Eine Startanfrage oder Startdatei mit mehr als 127 Verschachtelungsebenen ist `start.json_ungueltig` (Stufe 2, Stackschutz,
fail closed; die Grenze ist die von `serde_json` in der Voreinstellung, die höchstens 127 Ebenen liest). Gezählt wird, wie viele Objekte und Listen zugleich offen sind; das Objekt der obersten Ebene ist Ebene 1.
Klammern in Texten zählen nicht. Die Prüfung läuft, ohne zu rekursieren, vor dem Lesen des Wertes: Das Lesen darf nicht
an der Stapeltiefe scheitern. Die Form kennt keinen verschachtelten Wert; darum sind Fälle an der Grenze nur unter einem
unbekannten Schlüssel möglich. Tiefe 127 passiert Stufe 2 und scheitert erst an Stufe 5 (`start.schluessel_unbekannt`);
Tiefe 128 ist `start.json_ungueltig`. Fälle: `*-json-tiefe-*` (Listen und Objekte, 1500 Ebenen, vor Stufe 3, in der
Datei, Klammern im Text).

### stderr-Zeile und Kindende

Vor dem exec schreibt das Programm bei jedem Fehler genau eine Zeile `<code>: <grund>` mit Zeilenende `\n` auf stderr,
ASCII, höchstens 200 Byte. Der Grund ist der feste Text aus den Tabellen. Die Zeile enthält keinen Pfad, keinen
Dateiinhalt und kein Geheimnis. Sie erscheint im Terminal der Sitzung, weil stderr die PTY ist. Der Exitcode erscheint
als Kindende in der Sitzungsliste. Bei Erfolg schreibt das Programm nichts.

### Meldung an den Dienst (Nachtrag 22.11.2026)

Der Kern-Dienst kann den Exit vor dem Start des Starters (64 bis 78) nicht vom Exit des Harness unterscheiden; beide
können dieselben Werte tragen. Darum meldet das Programm ein Scheitern vor dem Start über eine eigene Verbindung. Den
Dienst beschreibt der Vertrag `dienst`, Abschnitt 18 „Meldung von lauf“.

- **Aufruf.** `--meldung <pfad>` nennt einen Unix-Stream-Socket, den der Dienst in seinem Ordner der Sitzung anlegt
  (0600). Die Option steht vor der Startdatei, `<pfad>` ist absolut; jede andere Form (die Option nach der Startdatei,
  ein relativer Pfad, die Option ohne Pfad, auch allein) ist 64 `lauf.aufruf`. Fehlt die Option, verhält sich das Programm wie ohne diesen Abschnitt. Schritt 0 und die Prüfung der Argumente
  (Schritt 1) kommen vor der Verbindung und melden nichts über sie.
- **Verbinden.** Vor dem ersten anderen Schritt nach den Argumenten hält das Programm die Deskriptoren 3 und 4 frei
  (je `/dev/null` mit close-on-exec) und verbindet sich dann. Scheitert das `connect`, auch weil der Pfad für `sun_path`
  zu lang ist, endet es mit 69 und `lauf.meldung_fehlt` und schreibt nur die stderr-Zeile. Ist Deskriptor 4 beim Start
  nicht frei, endet es mit derselben Zeile auf der Verbindung.
- **Kein Kind erbt sie.** Die Verbindung trägt close-on-exec, solange das Programm Paket-Entpacken, Such-Prozess,
  Modellweg und Wächter startet. Schritt 6 zählt sie und den freigehaltenen Deskriptor 4 nicht als geerbte Deskriptoren.
- **Zeile bei einem Scheitern.** Scheitert das Programm vor dem Start des Starters, schreibt es genau eine Zeile
  `<code> <grund>\n` und endet mit `code`. Form: höchstens 128 Byte; `code` ist eine ganze Zahl von 1 bis 255 ohne
  führende Null; `grund` ist der Schlüssel des Fehlers (etwa `lauf.harness_fehlt`), nur `[a-z0-9_.]`, 1 bis 64 Zeichen,
  **nicht** der Klartext der stderr-Zeile; nach der Zeile folgt nichts. Die stderr-Zeile bleibt unverändert. Alles
  andere wertet der Dienst als `lauf.meldung_ungueltig` (fail closed, gescheiterter Start mit dem Code des Kindendes).
  Der Dienst liest den Socket beim Kindende zuerst leer.
- **Übergabe an das Trampolin.** Unmittelbar vor dem Start des Trampolins legt das Programm die Verbindung mit `dup2`
  auf den freigehaltenen Deskriptor 4 ohne close-on-exec, startet das Trampolin mit `--halten-melden` statt `--halten`
  und schließt danach beide eigenen Kopien. Das Trampolin setzt close-on-exec auf 4 und ersetzt sich durch den Starter;
  scheitert dieses exec, schreibt es `69 lauf.harness_fehlt`. Scheitert vorher seine Bindung an den Elternprozess oder
  das Setzen von close-on-exec, schreibt es `70 lauf.intern` und startet den Starter nicht. Ende der Verbindung ohne
  Daten heißt für den Dienst: Der Starter läuft. Die Verbindung kommt nie in den Einschluss.
- **Regel.** Zwischen dem `dup2` auf 4 und dem Schließen der eigenen Kopie startet das Programm kein anderes Kind, auch
  nicht aus einem anderen Faden. Zu diesem Zeitpunkt läuft kein weiterer Faden: Der Faden für die Signale startet erst
  nach dem Start des Starters (`rust/crates/lauf/src/uebergabe.rs`, `halten`).
- **Grenze.** Endet lauf nach dem connect und vor dem Trampolin ohne Zeile durch ein Signal oder einen Abbruch, sieht der
  Dienst ebenfalls EOF ohne Daten; das Kindende trägt dann das Signal. Das betrifft SIGKILL, SIGABRT (das Release-Profil
  hat `panic = "abort"`, ein Panic endet so), SIGHUP und SIGTERM (vor `halten` hat lauf keinen Signal-Handler), SIGSEGV und
  den OOM-Killer. Der Dienst ordnet das dem Starter zu; ein falscher Erfolg entsteht nicht (Code des Signals, nicht 0).

Fälle (`rust/crates/lauf/tests/meldung.rs`, Prüfbau, Linux; der Test spielt den Dienst): eine Zeile mit Code und Grund
und Exit gleich Code (66 `lauf.datei_modus`, 69 `lauf.modellweg_nicht_bereit`, 69 `lauf.harness_fehlt` aus Schritt 8);
gelungener Start mit Ende ohne Daten, solange Starter, Such-Prozess, Modellweg und Wächter laufen, danach der Exit des
Starters (Signal 9, Code 42); gescheitertes exec des Starters mit `69 lauf.harness_fehlt`; der Such-Prozess hält die
Verbindung nicht, während das Programm sie hält (`/proc/<pid>/fd`, Gegenstelle über `ss -x`, mit Positivkontrolle);
ohne Option keine Verbindung; gescheitertes `connect` (kein Socket, eine Datei, Pfad über `sun_path`); ein geerbter
Deskriptor 4; das Trampolin ohne Deskriptor 4. `src/meldung/tests.rs` prüft jeden Schlüssel aus `lauf/src/fehler.rs`
und die weitergereichten Schlüssel `start.*` und `einst.*` gegen die Form.

### Such-Prozess je Sitzung

Vor dem Harness startet das Programm je Sitzung einen Such-Prozess **außerhalb** des Einschlusses:

```
rangkern sitzung <vault> --socket <pfad> --route <route> --sitzung <sitzung> --einstellungen <datei> --protokoll <ordner> --socket-rechte 0600|0666
```

Mit `marker` hängt das Programm `--probelauf --marker <a,b,...> --marker-zaehlung <ordner>/<sitzung>.suche.json` an;
ohne `marker` keine der drei Optionen (siehe „Marker“). Die Form des Aufrufs legt Vertrag 4 fest (`rust/spec/such-socket/README.md`, Abschnitt „Aufruf“, Stand vom 20.11.2026).
Das Programm übergibt `--socket-rechte` immer ausdrücklich (siehe „Socket“).
`<route>` und `<sitzung>` kommen aus der geprüften Startdatei. `<vault>`, `<datei>`, `<ordner>` und der Socket-Pfad
kommen aus der eigenen Konfiguration des Programms. Vault und Protokoll liegen außerhalb des Einschlusses. In den
Einschluss kommt nur der Socket. stdin des Such-Prozesses ist `/dev/null`; sein stderr geht nicht in die PTY. Seine
Umgebung ist leer (`env_clear`); alles, was er braucht, steht in seinen Argumenten.

**Bereitschaft.** Der Such-Prozess meldet Bereitschaft mit genau der Zeile `{"bereit":true}` und LF auf stdout
(Vertrag 4, `rust/spec/such-socket/README.md`, Stand vom 20.11.2026). Das Programm wartet darauf höchstens 10 s. Es urteilt allein über die erste vollständige Zeile auf stdout (mit LF, höchstens so lang wie die Bereitschaftszeile): Sie muss genau die Bereitschaftszeile sein. Was danach folgt, wertet es nicht und liest es nicht mehr, gleich ob es im selben Lesezug ankommt oder später. Ausbleiben,
vorzeitiges Ende oder jede andere Zeile ergibt `lauf.suche_nicht_bereit`; der Harness startet dann nicht. Endet der
Such-Prozess vor der Zeile mit einem eigenen Exitcode (Vertrag 4: 5, 64, 78 oder 74), gilt dasselbe.

**Socket.** Das Programm legt je Sitzung einen Ordner mit Modus 0700 **außerhalb** des Einschlusses an; der Socket
liegt darin. Der Ordner wird nie in den Einschluss eingebunden. In den Einschluss kommt nur der Socket selbst, per
Bind am festen Gastpfad **`/run/brain/such.sock`** (Vertrag 4). Die Rechte des Sockets wählt das Programm und übergibt
sie mit `--socket-rechte`; der Such-Prozess setzt sie (Vertrag 4, ohne die Option 0600):

| Fall | `--socket-rechte` |
|---|---|
| Such-Prozess und Agent haben dieselbe Kennung (Fassung 1, Linux ohne Vorrechte: bwrap bildet beide auf dieselbe uid ab) | `0600` |
| der Einschluss gibt dem Agenten eine eigene Kennung (später, mit Dienstkonto) | `0666`; den Zugang regelt dann allein der Ordner 0700 |

Ein Bind-Mount umgeht die Wegprüfung des Ordners, aber nicht die Rechte am Socket. 0666 bei gleicher Kennung würde den
Socket für jede andere Kennung öffnen, die den Pfad erreicht.

Der Hostpfad des Sockets bleibt kurz: `sun_path` fasst unter Linux 108 Byte, unter macOS 104 Byte, jeweils mit dem
abschließenden Nullbyte. Der Ordner liegt deshalb nicht tief, etwa unter `$XDG_RUNTIME_DIR` des Dienstes und nicht unter
einem langen `TMPDIR`. Ist der Hostpfad zu lang, meldet das Programm `lauf.suche_fehlt`.

### Der Such-Prozess ist für den Agenten unerreichbar (E3)

Die Anforderung ist eine **Eigenschaft**, keine Kennung:

> Der Agent kann dem Such-Prozess kein Signal senden, seine Umgebung und seinen Speicher nicht lesen und ihn nicht
> anhängen (`ptrace`). Er erreicht die Suche nur über den Socket seiner Sitzung.

Wie Variante A (Linux, bwrap ohne Vorrechte) sie erfüllt: Der Einschluss hat einen eigenen PID-Raum. Die PID des
Such-Prozesses ist darin nicht adressierbar; `kill`, `/proc/<pid>/environ`, `/proc/<pid>/mem` und `ptrace` scheitern.
Such-Prozess und Agent haben dabei **dieselbe Kennung**. Eine eigene Kennung ist keine Anforderung der Fassung 1, denn
sie braucht ein Vorrecht (eine Systemunit mit eigenem Dienstkonto). Sie kommt später mit einem Dienstkonto und darf den
Vertrag dann verschärfen. In der Vorführung heißt es deshalb „gleiche Kennung, getrennt durch Namensräume“, nicht
„andere Kennung“.

Zwei Härtungen gelten zusätzlich:

- Der Such-Prozess setzt unter Linux als Erstes **`prctl(PR_SET_DUMPABLE, 0)`** selbst (Vertrag 4). Ein Wert, den das
  Programm vor dem exec des Such-Prozesses setzte, gälte nicht, denn exec setzt ihn zurück. Damit bleiben Umgebung, Speicher und `ptrace` auch für Prozesse derselben Kennung zu.
- Seine Umgebung ist leer (siehe oben).

**Gegenprobe:** Derselbe Versuch (Signal senden, Umgebung lesen, `ptrace` anhängen) gegen einen Such-Prozess **ohne**
eigenen PID-Raum des Einschlusses muss rot werden: Das Signal gelingt dort. Damit zeigt die Probe, dass der PID-Raum
die Grenze trägt und nicht ein Zufall des Aufbaus.

### Zustandsordner der Installation (Nachtrag 2, Projektentscheid, 20.11.2026)

Der **Zustandsordner** der Installation liegt im Paket unter `/var/lib/werkbank/<nutzer>/` (`<nutzer>` ist der Nutzer, dem
die Sitzungen gehören). Den Pfad nimmt „Lauf starten“ aus seiner eigenen Konfiguration; das Paket setzt dort diesen Wert, nie
die Startdatei, die Umgebung oder der Agent. Der Ordner hat Modus 0700 und liegt außerhalb jedes Arbeitsordners und jedes
Einschlusses. Er enthält:

| Pfad | Modus | Inhalt |
|---|---|---|
| `ordnermarken.json`, `ordnermarken.lock` | 0600 | die Ordnermarken (siehe „Ordnermarke“) |
| `sitzungen/` | 0700 | je Cloud-Sitzung die Zielliste des Modellwegs |
| `sitzungen/<sitzung>.zielliste.json` | 0600 | Zielliste der Sitzung `<sitzung>` |

Den Ordner `sitzungen/` legt die Installation an. Fehlt er oder gehört er nicht der eigenen Kennung oder hat er nicht den Modus
0700, ist das ein Fehler von Schritt 7a0.

**Zielliste je Cloud-Sitzung.** In Schritt 7a0, nur bei einer Cloud-Route, schreibt „Lauf starten“ die Zielliste des
Modellwegs nach `<zustandsordner>/sitzungen/<sitzung>.zielliste.json`. Ihren Inhalt legt Vertrag 8, Abschnitt 12, fest; sie
entsteht aus `netzziele` der Route (Vertrag 2). Die Regeln:
- Die Datei wird mit `O_CREAT | O_EXCL | O_NOFOLLOW` und Modus 0600 angelegt, **vor** dem Start des Modellwegs. Der Modellweg
  bekommt sie als `--zielliste <pfad>` und nie einen anderen Weg. Existiert die Datei schon (der Kern-Dienst vergibt keine
  Sitzungskennung zweimal), scheitert der Schritt: 73, `lauf.zielliste`. Dasselbe gilt für jeden Schreibfehler.
- Sie bleibt nach dem Ende der Sitzung **liegen**. „Lauf starten“ und der Kern-Dienst löschen sie nicht; sie dient
  `werkbank-modellweg suche` (Vertrag 8, Abschnitt zum Befehl `suche`, `--zielliste <datei>`).
- Sie enthält keinen Schlüssel, nur Namen der Schlüsseldateien (Vertrag 8). Sie liegt außerhalb des Einschlusses.
- Bei einer lokalen Route gibt es keine Zielliste.

**Prüfung (Fälle `aufruf-zielliste-*`):** Cloud-Route (Pfad, Modus, `O_EXCL`, vor dem Modellweg, bleibt liegen), lokale Route
(keine Zielliste), Datei existiert schon, Ordner `sitzungen` fehlt. Mutanten (Fälle `aufruf-zielliste-mutant-*`): die Datei
liegt im Arbeitsordner; sie wird nach dem Start des Modellwegs geschrieben; sie wird am Sitzungsende gelöscht; sie wird ohne
`O_EXCL` geschrieben und überschreibt eine vorhandene. Die Mutanten-Fälle werden noch nicht ausgeführt (Läufer lässt sie aus, `AUSGELASSEN`); Entscheid des Betreibers, 23.11.2026: Sie werden ausgeführt, Läufercode folgt (OPEN 18).

### Modellweg je Sitzung (Nachtrag 20.11.2026)

Vor dem Harness startet das Programm je Sitzung einen zweiten Prozess **außerhalb** des Einschlusses: den Modellweg,
einen dünnen Weiterreicher (Vertrag 8 „Modellweg“, `rust/spec/modellweg/`). Eine Sitzung hat genau eine
Route und damit genau einen Modellweg. Er ist für die lokale Route der Weg zum Modell und für die Cloud-Route der
Ausgang; einen zweiten Baustein gibt es nicht. Der Schlüssel des Ziels kommt nie in den Einschluss, nie in die
Startdatei und nie in die Einstellungsdatei.

| Route | Betriebsart (Vertrag 8) | Gastpunkt im Einschluss | Harness spricht ihn über |
|---|---|---|---|
| lokal | `openai-lokal`: genau `<ziel>/chat/completions`, genau `modell` der Route, Schlüssel als `Authorization: Bearer` | `127.0.0.1:11434` | `baseUrl` `http://127.0.0.1:11434/v1` (pi) |
| cloud | `kopf-einsetzen`: HTTP-Proxy mit den Netzzielen der Route und ihrer Zugriffsart (Vertrag 2) | `127.0.0.1:8402` | `HTTPS_PROXY` (copilot) |

**Aufruf** (Vertrag 8, Abschnitt 4):

```
<modellweg> --betriebsart openai-lokal --socket <pfad> --socket-rechte 0600 --sitzung <sitzung> --route <route> --protokoll <ordner> --ziel <ziel> --modell <modell> --schluessel-datei <pfad>
<modellweg> --betriebsart kopf-einsetzen --socket <pfad> --socket-rechte 0600 --sitzung <sitzung> --route <route> --protokoll <ordner> <Optionen der Betriebsart>
```

- `<ziel>` und `<modell>` sind `ziel` und `modell` der Route (Vertrag 2, Fassung 2). `<sitzung>` und `<route>`
  kommen aus der geprüften Startdatei. Socket-Pfad, Schlüsseldatei und Protokollordner nimmt das Programm aus seiner
  eigenen Konfiguration.
- Die Optionen von `kopf-einsetzen` legt Vertrag 8 Teil C fest; dort ist die Betriebsart beschrieben und unter Linux gebaut
  (`rust/crates/modellweg/src/betriebsart.rs`). Das Programm übergibt `--zielliste`, `--ca-schluessel` und `--ca-zertifikat` (`ablauf.rs:772-779`). Inhaltlich übergibt das Programm die Netzziele der Route, die Schlüsseldatei der
  Route und den Schlüssel der Wegwerf-CA.
- Socket-Ordner, `--socket-rechte`, leere Umgebung, stdin `/dev/null`, stderr nicht in die PTY: wie beim Such-Prozess.
- **Bereitschaft:** genau die Zeile `{"bereit":true}` mit LF auf stdout, Frist 10 s. Vorher schreibt er bei einem
  Fehler eine stderr-Zeile und endet mit einem eigenen Exitcode (Vertrag 8). Ausbleiben, vorzeitiges Ende oder jede
  andere Zeile ergibt `lauf.modellweg_nicht_bereit`; scheitert schon der Start, `lauf.modellweg_fehlt`.
- **Marker:** Mit `marker` hängt das Programm `--probelauf --marker <a,b,...> --marker-zaehlung
  <ordner>/<sitzung>.modellweg.json` an, in beiden Betriebsarten; ohne `marker` keine der drei Optionen. `<a,b,...>` ist `marker.liste` in ihrer
  Reihenfolge, mit Komma verbunden.
- **Lebensdauer:** wie beim Such-Prozess (`PR_SET_PDEATHSIG`, `setsid`, zweite Linie über `modellweg.pid` neben der
  Startdatei, siehe „Lebensdauer“).
- **Im Einschluss:** Der Socket kommt per Bind hinein. Am festen Gastpunkt auf dem eigenen Loopback des Namensraums
  nimmt ein kleiner Teil Verbindungen an und reicht sie über diesen Socket weiter.

**Wegwerf-CA (nur Cloud-Route).** Für `kopf-einsetzen` beendet der Modellweg TLS für die Hosts der Zielliste. Dafür
erzeugt das Programm je Sitzung eine Wegwerf-CA außerhalb des Einschlusses.
- Ihr Schlüssel bleibt draußen; nur der Modellweg bekommt ihn, als Datei 0600 neben der Startdatei.
- Ihr Zertifikat kommt in das System-Bundle **des Einschlusses**: Das Programm bindet eine Kopie des Bundles des Wirts
  mit dem angehängten Zertifikat an den Pfad des System-Bundles im Einschluss (unter Ubuntu
  `/etc/ssl/certs/ca-certificates.crt`). Das Bundle des Wirts ändert sich nicht.
- Nie über `SSL_CERT_FILE` oder `NODE_EXTRA_CA_CERTS`: Gemessen (t2) schreibt die CLI dann die Warnung
  `forceLoginOrgs: an ad-hoc TLS trust root is configured` ins Log. Ebenfalls gemessen (t4): Die CLI nutzt das
  System-Bundle.
- Nach außen prüft der Modellweg TLS mit dem Truststore des Wirts (Vorgabe der Organisation, Vertrag 8 Abschnitt 6).
- **Einschränkung der CA** (Projektentscheid, 20.11.2026, gemessen im Bau des Weiterreichers): Name Constraints mit
  `permitted dNSName` für **alle** Hosts, für die der Modellweg TLS beendet, also jedes Netzziel mit `zugriff:
  kopf-einsetzen`, mit und ohne `schluessel` (etwa `exp.…` und `telemetry.…`). Sonst lehnt ein prüfender Client das
  Zertifikat eines ungebundenen Ziels ab (permitted subtree violation). Dazu `excluded iPAddress` `0.0.0.0/0` und
  `::/0`, `pathlen 0` und eine Gültigkeit von höchstens 24 h. Ein Ziel in Unterdomänen-Form `*.d` mit
  `kopf-einsetzen` kann die Zielliste nicht tragen; es endet schon Schritt 7a0 mit `lauf.zielliste`, und es entsteht keine CA.
  Ziele mit `verbinden` oder `http-lesen` stehen weder in der Zielliste noch in der CA. „Lauf starten“ übergibt dem Modellweg
  nur die Zielliste; diese Ziele sind im Einschluss darum nicht erreichbar (fail closed), und der Start geht trotzdem weiter.
- Die CA gilt nur für diese Sitzung; der Kern-Dienst löscht Schlüssel und Kopie am Sitzungsende.

**Kopf-Tausch und Durchsetzung (Cloud-Route, Vertrag 2 „Netzziel“).**
- An einem Ziel mit `schluessel: true` ersetzt der Modellweg `Authorization: Bearer <Platzhalter>` durch den echten
  Schlüssel.
- Ein erlaubtes Ziel mit `schluessel: false` beantwortet eine Anfrage mit einem Kopf `Authorization` mit 403 und
  schreibt eine Protokollzeile.
- Methode und Pfad setzt er an jedem Ziel mit `kopf-einsetzen` durch. Ein Verstoß ergibt 403 und eine Protokollzeile,
  die sich von einer erlaubten Anfrage unterscheidet (Feld und Werte legt Vertrag 8 fest).

### Die Netzgrenze des Einschlusses (E5, Nachtrag 20.11.2026)

E5 „Netzgrenze“ heißt wie im Drehbuch (Projektentscheid, 20.11.2026); früher hieß sie in diesem Vertrag N1.

Die Anforderung ist eine **Eigenschaft**:

> Der Agent läuft in einem eigenen Netz-Namensraum. Der Loopback des Rechners ist aus dem Einschluss **nie**
> erreichbar. Der Agent erreicht genau eines: den Modellweg seiner Sitzung am festen Gastpunkt, `127.0.0.1:11434` bei
> einer lokalen Route, `127.0.0.1:8402` bei einer Cloud-Route. Über den Modellweg erreicht er nur das Ziel der lokalen
> Route oder die Netzziele der Cloud-Route mit ihrer Zugriffsart (A141). Alles andere ist zu: die Brücke des
> Kern-Dienstes, die Stelle, jeder andere Port des Rechners und das Ziel ohne den Modellweg.

Aufbau:

- **Eigener Netz-Namensraum.** Darin gibt es nur das eigene Loopback-Gerät. Es gibt kein weiteres Netzgerät, keine
  Route nach außen und keinen DNS-Server; `/etc/resolv.conf` im Einschluss nennt keinen erreichbaren Server.
- **Modellweg als Ausgang (Cloud-Route), HTTP-Proxy am Gastpunkt `127.0.0.1:8402`:**
  - `kopf-einsetzen`: `CONNECT host:port`, dann TLS-Abschluss mit der Wegwerf-CA, Kopf-Tausch und Durchsetzung wie
    oben; nach außen eine eigene TLS-Verbindung.
  - `verbinden`: nur `CONNECT host:port`, danach ein reiner TCP-Tunnel. Auf einer Route mit Schlüssel gibt es kein
    solches Ziel (Vertrag 2, `einst.verbinden_mit_schluessel`).
  - `http-lesen`: nur `GET` und `HEAD` mit absoluter `http://host:port/...`-Adresse.
  - Er verbindet nur, wenn `host` ein Netzziel der Route genau trifft (oder über die Unterdomänen-Form `*.`) und
    `port` gleich ist. Er nimmt keine IP-Adresse als `host`. Er löst den Namen selbst auf und verbindet nie zu einer
    Loopback-, Link-local- oder unbestimmten Adresse (`127.0.0.0/8`, `169.254.0.0/16`, `0.0.0.0`, `::1`), auch wenn
    der Name dorthin auflöst. Bei einer Route ohne Netzziele (auch Fassung 1) lehnt er jede Anfrage ab.
  Stand der Umsetzung: Erreichbar sind nur die Ziele der Zielliste, also `kopf-einsetzen` in der einfachen Host-Form; `verbinden`
  und `http-lesen` nicht (siehe „Wegwerf-CA“).
- Der Gastpunkt der jeweils anderen Route gibt es nicht.

**Prüfung (Fälle `aufruf-gueltig-copilot`, `aufruf-gueltig-lokal`):** Aus dem Einschluss heraus:

| Versuch | lokale Route | Cloud-Route |
|---|---|---|
| ein beliebiger anderer Loopback-Port des Rechners, etwa der Port der Brücke | scheitert | scheitert |
| das Ziel der Route direkt (ohne Modellweg) | scheitert | scheitert |
| Gastpunkt `127.0.0.1:11434` | erreicht den Modellweg | scheitert (nicht vorhanden) |
| Gastpunkt `127.0.0.1:8402` mit `CONNECT` auf ein Netzziel | scheitert (nicht vorhanden) | gelingt, TLS endet am Modellweg |
| `CONNECT` auf ein anderes Ziel, anderen Port oder eine IP-Adresse | scheitert | scheitert |
| ein Pfad außerhalb von `pfade`, etwa `GET /user/repos` an `api.github.com` | scheitert | 403 mit Protokollzeile |
| eine Anfrage mit `Authorization` an ein Ziel mit `schluessel: false` | scheitert | 403 mit Protokollzeile |
| `POST` an ein Netzziel mit `http-lesen` | scheitert | scheitert |

**Gegenprobe:** Derselbe Versuch „anderer Loopback-Port des Rechners“ mit einem Profil **ohne** eigenen
Netz-Namensraum muss gelingen und die Prüfung damit rot machen. Der Testaufbau öffnet dafür einen eigenen Port auf dem
Loopback des Rechners (Ersatz für die Brücke) und kein echtes Ziel. Damit zeigt die Probe, dass der Namensraum die
Grenze trägt und nicht ein Zufall des Aufbaus. Eine zweite Gegenprobe gilt dem Ausgang: Ein Modellweg, der jedes
`CONNECT` annimmt, muss die Zeile „anderes Ziel“ rot machen.

Variante A (Linux, bwrap ohne Vorrechte) erfüllt den Namensraum mit `--unshare-net`. Der Modellweg überbrückt ihn über
einen Unix-Socket, der per Bind in den Einschluss kommt; im Einschluss lauscht ein kleiner Teil am festen Gastpunkt.
Wie dieser Teil aussieht, gehört in den Bau.

### Der Schlüssel bleibt draußen (E4, beide Routen, Nachtrag 20.11.2026)

E4 des Drehbuchs („Der Schlüssel des Gateways bleibt draußen“) gilt für beide Routen (Projektentscheid, 20.11.2026): für den
Schlüssel des Modell-Gateways in der Sitzung aus S5 und für den Schlüssel des Copilot-Kontos in der Sitzung aus S1. Der
direkte Aufruf des Ziels ohne den Modellweg gehört zu E4 und muss scheitern; E5 prüft die Netzgrenze allgemein. Die Anforderung ist eine **Eigenschaft**:

> Im Einschluss gibt es keinen Weg zum Schlüssel des Ziels: nicht in Umgebung, argv, Dateien, Prozessen, Speicher,
> Schlüsselbund oder über ein Hilfsprogramm. Der Schlüssel erreicht das Ziel trotzdem, weil der Modellweg ihn außerhalb
> einsetzt. Über den Schlüssel erreicht der Agent nur die Pfade aus `pfade`.

**Prüfung (Fälle `aufruf-e4-cloud` und `aufruf-gueltig-lokal`).**
- Der echte Schlüssel ist eine Marke, die nur die Schlüsseldatei des Modellwegs kennt. Upstream ist eine Attrappe mit
  Zähler, nicht GitHub und nicht das Gateway.
- Ein Probe-Skript als Kind des Harness sucht die Marke:
  - in `env`, `/proc/*/environ` und `/proc/*/cmdline`;
  - per `grep -r` über `/`, das Heim, `/tmp` und `/run`;
  - mit `secret-tool lookup service copilot-cli` und `gh auth token`;
  - in `/proc/<pid>/mem` der CLI (EPERM erwartet).
- Erwartung: 0 Treffer, und der Zähler am Upstream steigt mit der Marke. Der direkte Aufruf des Ziels ohne Modellweg
  scheitert.
- **Gegenprobe ohne Einschluss:** Mit der Marke direkt in der Umgebung findet die Probe 1 Treffer.
- **Mutanten** (Fälle `aufruf-e4-mutant-*`, je ein Aufbaufehler). Jeder muss die Prüfung rot machen:
  1. Der echte Schlüssel steht in der Umgebung des Einschlusses.
  2. `config.json` mit der Marke ist eingebunden.
  3. Der D-Bus-Socket des Wirts ist eingebunden.
  4. Ein `gh` mit Anmeldung ist erreichbar.
  5. Die Pfadliste für `api.github.com` fehlt; dann gelingt `GET /user/repos`.

Die Mutanten-Fälle werden noch nicht ausgeführt (Läufer lässt sie aus, `AUSGELASSEN`); Entscheid des Betreibers, 23.11.2026: Sie werden ausgeführt, Läufercode folgt (OPEN 18).

Die Prüfung beruht auf der Gegenprobe der Messung zum Token der Copilot CLI (Abschnitt 3); die Marke beginnt dort mit `gho_MARKE`. In
den Fällen dieses Vertrags steht sie nur als `<marke>`.

### Ordnermarke (E6, Nachtrag 20.11.2026, Pflicht für M1)

Die Anforderung ist eine **Eigenschaft** (im Drehbuch E6):

> Ein Arbeitsordner, in dem eine Sitzung mit einer Route gearbeitet hat, nimmt danach keine Sitzung mit niedrigerer
> Klasse und keine mit anderem Kunden auf. Das gilt auch für seine Vorfahren und Nachfahren und über jede Schreibweise
> des Pfads. Der Agent kann die Marke weder sehen noch ändern.

**Ort und Form.** Die Marken stehen in einer Datei `ordnermarken.json` im Zustandsordner der Installation (siehe oben:
im Paket `/var/lib/werkbank/<nutzer>/`, Ordner 0700, Datei 0600). Den Pfad nimmt „Lauf starten“ aus seiner eigenen
Konfiguration. Beide liegen außerhalb jedes Arbeitsordners und jedes Einschlusses. Nur „Lauf starten“ und der Befehl `werkbank-ordnermarke`
schreiben die Datei. Sie ist ein geschlossenes JSON-Objekt, UTF-8 ohne BOM, höchstens 1 MiB:

```json
{"fassung": 1, "marken": [{"ordner": "/home/beispiel/projekt", "kennung": "a1b2c3d4e5f60718293a4b5c6d7e8f90", "kunde": "k-beispiel", "klasse_hoch": "kundendaten"}]}
```

- `fassung` ist genau `1`. `marken` ist eine Liste mit höchstens 4096 Einträgen, ohne zwei Einträge mit demselben
  `ordner`.
- `ordner` ist ein Pfad nach den Regeln von `arbeitsordner`, und zwar schon aufgelöst (`realpath`).
- `kennung` ist Pflicht und hat die Form `^[0-9a-f]{32}$` (128 Bit, klein hex). „Lauf starten“ zieht sie aus dem Zufall des
  Systems, wenn es eine neue Marke setzt, und zieht neu, bis sie keiner Marke der Datei gleicht. Eine bestehende Marke
  behält ihre Kennung, auch wenn ihre Klasse steigt; Senken ändert sie nicht. Zwei Einträge mit derselben Kennung, eine
  fehlende oder eine Kennung falscher Form sind ein Ladefehler (`lauf.ordner_marke_kennung`, fail closed wie unten). Die
  Kennung steht in der Protokollzeile (Vertrag 5, Abschnitt 6.4); sie sagt nichts über den Pfad.
- `kunde` ist eine Kennung oder `null`. `null` heißt „kein Kunde“ und ist ein eigener Wert, kein Platzhalter.
- `klasse_hoch` ist der Name einer Klasse aus der Einstellungsdatei (Vertrag 2). Verglichen wird ihre Position in
  `klassen`.
- **Atomar geschrieben:** in eine neue Datei im selben Ordner, `fsync`, dann `rename` über die alte. Lesen, Prüfen und
  Schreiben laufen unter einer exklusiven Sperre (`flock`) auf `ordnermarken.lock` im selben Ordner. So überholen sich
  zwei gleichzeitige Starts nicht.
- Fehlt die Datei (`ENOENT`), gibt es noch keine Marke. Jeder andere Lesefehler, jede Abweichung von der Form und ein
  `klasse_hoch`, das die Einstellungsdatei nicht kennt, ergeben `lauf.ordner_marke_kaputt` (**fail closed**: kein
  Start in diesem Ordner und keinem anderen, bis ein Mensch die Datei repariert). Hat die Datei sonst die richtige Form,
  aber eine Marke ohne Kennung, mit einer Kennung falscher Form oder mit einer Kennung, die ein anderer Eintrag trägt,
  ergibt das `lauf.ordner_marke_kennung`, ebenfalls fail closed.

**Prüfung in Schritt 5b**, vor jedem Kind:
1. `P` = `realpath(arbeitsordner)`. Ein Verweis und eine andere Schreibweise führen so zum selben Pfad.
2. Die verwandten Marken sind die Einträge, deren `ordner` gleich `P` ist, ein Vorfahr von `P` oder ein Nachfahr von
   `P`. Verglichen wird ganzteilig an `/`: `/a/b` ist Vorfahr von `/a/b/c`, nicht von `/a/bc`.
3. Ist die Datei kaputt (Form, unbekannte Klasse): `lauf.ordner_marke_kaputt` (78). Danach prüft das Programm die Kennungen
   **aller** Marken der Datei, nicht nur der verwandten: fehlt eine, hat eine die falsche Form oder trägt sie ein anderer
   Eintrag auch: `lauf.ordner_marke_kennung` (78). Die Kennung geht also vor dem Kunden und vor der Klasse.
4. Hat eine verwandte Marke einen anderen `kunde` als der Start (`null` zählt als Wert): `lauf.ordner_anderer_kunde`
   (77).
5. Liegt die Klasse der Route unter dem höchsten `klasse_hoch` der verwandten Marken: `lauf.ordner_hoeher_markiert`
   (77). Es gilt also die strengste Marke.
6. Sonst schreibt das Programm die Marke für `P`, bevor es ein Kind startet. Fehlt sie, gilt `{kunde des Starts,
   Klasse der Route}` mit einer neuen Kennung (siehe oben); eine bestehende Marke behält ihre Kennung. Sonst wird `klasse_hoch` zum Höheren aus alter Marke und Klasse der Route. **Die Marke steigt
   nur.** Scheitert ein späterer Schritt, bleibt die Marke stehen; das ist die vorsichtige Seite.

Beispiel: Die lokale Sitzung (Klasse `kundendaten`) markiert den Ordner. Danach wird copilot (Route mit `intern`) dort
abgewiesen. Umgekehrt ist eine lokale Sitzung nach copilot erlaubt und hebt die Marke auf `kundendaten`.

**Lösen und Senken nur durch einen Menschen** außerhalb des Einschlusses, für M1 mit dem Befehl:

```
werkbank-ordnermarke zeigen <ordner>
werkbank-ordnermarke loesen <ordner>
werkbank-ordnermarke senken <ordner> --klasse <klasse>
```

- Der Befehl löst `<ordner>` mit `realpath` auf und ändert nur den Eintrag mit genau diesem Pfad. Er nutzt dieselbe
  Sperre und denselben atomaren Weg.
- Er schreibt je Änderung eine Protokollzeile (Projektentscheid, Vertrag 5, Abschnitt 6.4, Fassung 2 der
  Zeilen): `ordnermarke.geloest` für `loesen`, `ordnermarke.gesenkt` für `senken`. Die Zeile steht in
  `<protokollordner>/ordnermarke-<JJJJ-MM>.jsonl` (Quelle `ordnermarke`, Kette nach Vertrag 5, Abschnitt 5). Den
  Protokollordner nimmt der Befehl aus seiner eigenen Konfiguration. Sie trägt die Kennung der Marke, Kunde und Klasse vor
  und nach der Änderung, aber **keinen Pfad und keinen Hash des Pfads**. `zeigen` ändert nichts und schreibt keine Zeile.
- **Auflösung und Ausgabe (Bau, 21.11.).** Gibt es den Ordner nicht mehr, löst der Befehl den vorhandenen Teil des Pfads mit
  `realpath` auf und hängt den Rest an; so lässt sich auch die Marke eines gelöschten Ordners lösen. Ist das Ergebnis kein
  Pfad nach den Regeln von `arbeitsordner` (etwa `/`), endet der Befehl mit `ordnermarke.aufruf`. `zeigen` nimmt keine
  Sperre und legt keine Datei an (die Liste wird nur ganz ersetzt) und schreibt eine Zeile auf die Standardausgabe:
  `{"kennung":"<hex>","kunde":<kennung oder null>,"klasse_hoch":"<klasse>"}`, nie den Pfad. `loesen` und `senken` schreiben
  nichts auf die Standardausgabe. Die Argumente prüft der Befehl zuerst (64), dann die Konfiguration (78), bei `senken` die
  Klasse gegen die Einstellungsdatei (64), dann Sperre, Lesen, Eintrag suchen (66), bei `senken` die Klasse gegen die
  Marke (64). Ein Fehler beim Anlegen oder Schreiben der neuen Datei vor der Zeile ist `ordnermarke.marke_kaputt`, ein
  Fehler beim `fsync` des Ordners nach dem `rename` ist `ordnermarke.protokoll` (die Zeile steht, die Marke ist geändert).
- **Fail closed.** Scheitert das Schreiben der Zeile, scheitert der Befehl, und die Marke bleibt, wie sie war. Dafür gilt
  die Reihenfolge: Sperre nehmen, Marken lesen und prüfen, die neue Datei neben die alte schreiben und `fsync`en, die Zeile
  anhängen, erst dann die neue Datei über die alte `rename`n. Scheitert ein Schritt vor dem `rename`, löscht der Befehl die
  neue Datei und ändert nichts; endet er mit 74 und `ordnermarke.protokoll`, wenn es die Zeile war. Scheitert der `rename`
  nach der Zeile, bleibt die Marke und der Befehl endet mit 74; dann steht eine Zeile ohne Wirkung im Protokoll. Das ist
  die vorsichtige Seite: Zu viel Protokoll ist besser als eine Änderung ohne Zeile. Den `fsync` der neuen Datei hält die
  Sperre nicht: Der Befehl schreibt die neue Datei nach der ersten Sperre, hängt die Zeile an und `rename`t erst unter
  einer zweiten, wenn die Liste noch Byte für Byte die gelesene ist; sonst löscht er die neue Datei und entscheidet unter
  der Sperre neu, wie der Start (ein langsamer `fsync` ließ sonst einen zweiten Befehl nach 5 s scheitern).
- **Fehler des Befehls.** `loesen` und `senken` ändern nur den Eintrag mit genau dem aufgelösten Pfad:

| Code | Exit | Grund (fest, ASCII) | Bedingung |
|---|---|---|---|
| `ordnermarke.aufruf` | 64 | `Aufruf ungueltig` | falsche Argumente; bei `senken` eine Klasse, die die Einstellungsdatei nicht kennt, oder eine Klasse, die nicht unter `klasse_hoch` der Marke liegt |
| `ordnermarke.keine_marke` | 66 | `Keine Marke fuer diesen Ordner` | kein Eintrag mit genau diesem Pfad |
| `ordnermarke.marke_kaputt` | 78 | `Ordnermarken unlesbar oder beschaedigt` | Bedingungen von `lauf.ordner_marke_kaputt` oder `lauf.ordner_marke_kennung` |
| `ordnermarke.protokoll` | 74 | `Protokollzeile nicht schreibbar` | die Zeile oder der `rename` scheitert; die Marke bleibt |
| `ordnermarke.konfig_fehlt` | 78 | `Konfigurationsdatei fehlt` | die eigene Konfiguration des Befehls fehlt (Bedingung von `lauf.konfig_fehlt`) |
| `ordnermarke.konfig_kaputt` | 78 | `Konfigurationsdatei nicht lesbar oder ungueltig` | Bedingungen von `lauf.konfig_kaputt`, dazu eine Einstellungsdatei, die der Befehl nicht lesen oder nicht als Vertrag 2 prüfen kann |

  Bei jedem dieser Fehler ändert der Befehl nichts. Nur bei `ordnermarke.protokoll` nach gelungener Zeile (siehe oben) steht
  eine Zeile ohne Wirkung.

- Er läuft nur in einer Sitzung des Menschen am Rechner. Im Einschluss gibt es ihn nicht, und die Markendatei ist dort
  weder sichtbar noch erreichbar. Das tragen im Bau zwei Dinge, die der Befehl selbst mitbringt: Er liest seine
  Konfiguration von `/etc/werkbank/lauf.json` (der Prüfbau neben dem Programm), und der Einschluss bindet von `/etc` nur fünf
  Dateien (`ETC_DATEIEN`, `rust/crates/einschluss/src/beschreibung.rs`), nicht diese. Der Zustandsordner mit
  `ordnermarken.json` steht im Profil nirgends (`rust/crates/einschluss/src/profil.rs`, `kopf_bauen`: nur `/usr`, die fünf
  Dateien, `/proc`, `/dev`, `/tmp`, `/run`, das Heim, die Programmordner, der Zustandsordner der Sitzung und der
  Arbeitsordner). Im Einschluss endet der Befehl darum mit einem festen Fehler (`ordnermarke.konfig_fehlt` oder
  `ordnermarke.konfig_kaputt`, Exit 78), schreibt keine Zeile und ändert keine Datei. Belegt im Prüfbau:
  `rust/crates/lauf/tests/einschluss.rs`, Test `the_command_changes_nothing_inside_the_enclosure` (Aufruf im echten
  `bwrap`: Exit 78, die Markendatei bleibt byteweise gleich und ist von innen `ENOENT`, kein Protokoll). Der Befehl braucht
  außerhalb des Einschlusses nur seine Konfiguration und den Zugriff auf den Zustands- und den Protokollordner; er liest
  keine Umgebungsvariable und keine Datei, die der Agent schreiben kann.

  **Zwei Linien gegen einen Aufruf aus dem Einschluss (Projektentscheid, Nachtrag 2 vom 21.11.).** Das Programm liegt im Paket
  als `/usr/bin/werkbank-ordnermarke` (root, 0755, dynamisch gegen das OpenSSL der Distribution gelinkt wie `werkbank-lauf`; es
  läuft nicht über den Kern-Dienst). Linie 1 ist die Markendatei: Der Zustandsordner und die Konfiguration sind im Einschluss
  nicht erreichbar, der Befehl endet dort mit einem festen Fehler (siehe oben). Linie 2 ist das Programm selbst: `/usr` ist im
  Einschluss sichtbar (`--ro-bind /usr /usr`), darum legt der Einschluss danach, nur wenn die Datei auf dem Wirt existiert,
  `--ro-bind /dev/null /usr/bin/werkbank-ordnermarke` darüber (`System::vom_host_mit`, `ORDNERMARKE_PFAD` in
  `rust/crates/einschluss/src/beschreibung.rs`). Ohne die Datei gibt es den Bind nicht, weil `bwrap` an einem fehlenden Ziel
  im nur lesbaren `/usr` scheitert. Die Textprüfung des Profils (`kopf_pruefen` in `profil.rs`) lässt genau dieses Paar zu
  (`--ro-bind`, Quelle `/dev/null`, Ziel `/usr/bin/werkbank-ordnermarke`); jede andere Bindung mit der Quelle `/dev/null` weist
  sie ab. Im Einschluss ist das Programm damit das Gerät `/dev/null`: ein Aufruf lässt sich nicht starten und erreicht den Code
  des Befehls nicht (Test `a_bind_of_dev_null_over_a_file_of_usr_makes_it_empty_inside_a_real_enclosure`). Ein Aufruf aus dem
  Einschluss endet darum nie mit einem Code des Befehls (etwa 78), sondern mit dem Fehler beim Start: Eine Shell meldet
  `Permission denied` und Exit 126 (gemessen in an earlier test run am 21.11.2026, Projektentscheid, 21.11.2026). Wer den Exit prüft,
  erwartet im Einschluss 126. Fällt Linie 2 aus, hält Linie 1. Fällt Linie 1 aus (etwa durch eine falsche
  Bindung), hält Linie 2.

**Weitere fail-closed-Wege in Schritt 5b**, alle `lauf.ordner_marke_kaputt` (78), aus dem Code gelesen:
- `realpath(arbeitsordner)` ist kein Pfad nach den Regeln von `arbeitsordner` (Verweis auf ein Ziel mit Steuerzeichen, über 1024 Byte oder nicht UTF-8; `ablauf.rs:224-227`). Fall `aufruf-marke-realpath-ausser-form`.
- Die Sperre auf `ordnermarken.lock` kommt nicht binnen 5 s (`marke.rs:25`, `:296-308`); die Sperre hat also eine Frist. Test `marke/tests.rs` `a_held_lock_ends_the_start_after_the_time_limit`; Entwurf E5.
- Die Liste hat schon 4096 Einträge und ein neuer Ordner käme dazu (`marke.rs:233-234`); ein schon markierter Ordner geht weiter. Entwurf E6.
- Der Zustandsordner ist kein eigener Ordner mit Modus 0700 oder ein Verweis (`marke.rs:254-266`).

**Ehrliche Grenze.** Kopiert ein Mensch Dateien von Hand in einen anderen Ordner, greift die Marke nicht. Das fängt
erst das Klassenerbe in Schritt II ab.

**Prüfung (Fälle `aufruf-befehl-*`, Aufruf `werkbank-ordnermarke` in `lage.argv`):** `loesen` und `senken` mit der Zeile
ohne Pfad, `zeigen` ohne Zeile, jeder Fehlercode der Tabelle, der Ausfall des Protokolls (die Marke bleibt) und das Senken auf
dieselbe oder eine höhere Klasse.

**Prüfung (Fälle `aufruf-marke-*`).** Abgedeckt sind: copilot im Ordner der lokalen Sitzung; ein zweiter Kunde im
Ordner des ersten; ein Ordner ohne Kunde gegen einen Lauf mit Kunde; Unterordner; Elternordner; ein Verweis auf einen
markierten Ordner; eine kaputte Marke; eine unbekannte Klasse. Dazu kommen die Reihenfolge der Codes, die steigende
Marke und eine Marke, die nie sinkt, die Kennung (fehlt, falsche Form, doppelt, neu gezogen und nie gleich einer vorhandenen,
bleibt bei steigender Marke) und ihre Reihenfolge vor Kunde und Klasse. In den Fällen steht die Kennung einer neuen Marke
als `"<neu>"`: irgendeine Kennung der Form `^[0-9a-f]{32}$`, die keine andere Marke der Datei trägt. `lage.zufall` ersetzt
den Zufall des Systems durch eine Liste von Kennungen, die der Reihe nach gezogen werden.

**Mutanten** (Fälle `aufruf-e6-mutant-*`), jeder macht die Prüfung rot:
1. Die Marke liegt im Arbeitsordner und ist für den Agenten schreibbar.
2. Nur der exakte Pfad wird verglichen, nicht Vorfahren und Nachfahren.
3. Der Vergleich läuft ohne `realpath`.
4. Ein Start mit niedrigerer Klasse senkt die Marke ohne Menschen.
5. Der Befehl ändert die Marke und schreibt die Zeile nicht, wenn das Schreiben der Zeile scheitert.
6. Die Zeile trägt den Pfad des Ordners.

Die Mutanten-Fälle werden noch nicht ausgeführt (Läufer lässt sie aus, `AUSGELASSEN`); Entscheid des Betreibers, 23.11.2026: Sie werden ausgeführt, Läufercode folgt (OPEN 18).

### Lebensdauer des Such-Prozesses: gewählter Weg

Das Programm startet den Starter als Kind und bleibt dessen Elternprozess (seit 21.11.2026, Abschnitt „Elternprozess des
Starters“; vorher ersetzte es sich per exec). Der Such-Prozess wird so gebunden:

1. **`PR_SET_PDEATHSIG = SIGTERM`** setzt der Such-Prozess gleich nach dem fork, vor seinem eigenen exec. Danach
   vergleicht er `getppid()` mit der PID des Programms; weicht sie ab, ist der Elternprozess schon tot und er endet
   sofort. Der Elternprozess ist das Programm, das bis zum Ende des Starters lebt. Stirbt es, auch durch SIGKILL, kommt
   SIGTERM.
2. **Eigene Sitzung (`setsid`)** für den Such-Prozess. Er hat kein steuerndes Terminal; Strg-C und SIGHUP der PTY
   treffen ihn nicht.
3. **Der Wächter** (Messung 21.11.2026, Ubuntu 26.04, Kernel 7.0.0-43). Das Todessignal allein genügt nicht, wenn der
   Starter den Lauf ablehnt: Das war gemessen, als das Programm per exec der Starter wurde, dessen AppArmor-Profil `werkbank-einschluss`
   keine `signal`-Regel hat. Endet der Starter mit dem Exit 5, verweigert AppArmor das Senden des Todessignals an die
   Kinder ohne Profil (Audit `operation="signal" ... denied_mask="send" signal=term peer="unconfined"`); `rangkern` und
   `werkbank-modellweg` blieben mit Elternprozess 1 stehen. Mit `signal,` im Profil starben sie (Gegenprobe). Vor dem
   exec startet das Programm darum, als letzten Schritt vor dem exec, einen Wächter: `werkbank-lauf-kind --wache <PID des Programms> <Socket-Ordner> <Sitzungsordner>`, mit
   den pidfds beider Kinder auf stdin und stdout, in eigener Sitzung (`setsid`), außerhalb jedes Profils. Er wartet auf
   das Ende des Prozesses mit der PID des Programms (also des Starters oder des Harness), sendet beiden Kindern
   SIGTERM und nach 2 s SIGKILL und löscht danach den Sitzungsordner `sitzungen/<sitzung>` und zuletzt den Socket-Ordner
   der Sitzung. Die Dateien neben der Startdatei lässt er liegen. Scheitert der Wächter beim Start, endet der Start mit `lauf.intern` und beendet die Kinder. Nach jeder
   Ablehnung durch den Starter leben darum keine Kinder mehr, und Socket-Ordner und Sitzungsordner sind weg. Scheitert
   ein Schritt vor dem exec, löscht das Programm beide Ordner selbst. **`PR_SET_PDEATHSIG` wirkt unter dem Starter-Profil nicht
   (AppArmor verweigert das Signal); die Kinder beendet der Wächter.** Niemand verlässt sich darum nach dem exec auf das
   Todessignal. Das Profil bekommt für M1 keine `signal`-Regel und bleibt so eng wie gemessen (Projektentscheid
   21.11.2026); neu bewertet wird nach M1, falls ein Fall auftaucht, den weder Wächter noch Kern-Dienst decken (etwa ein
   per SIGKILL beendeter Wächter ohne laufenden Kern-Dienst).
4. **Zweite Linie: der Kern-Dienst räumt am Sitzungsende auf.** Das Programm schreibt die PID des Such-Prozesses vor dem
   exec als Dezimalzahl mit `\n` in `suche.pid` neben die Startdatei (Modus 0600), ebenso die PID des Modellwegs in
   `modellweg.pid`. Am Ende des Kindes sendet der Kern-Dienst SIGTERM an die Prozessgruppe jeder dieser PIDs, wenn sie
   noch lebt. Danach löscht er die Dateien der Sitzung (siehe unten).

### Elternprozess des Starters (Nachtrag 21.11.2026)

**Befund (gemessen 21.11.2026, Linux-Baurechner, bubblewrap 0.12.0).** Ein SIGKILL des Starters in seinen ersten etwa 4 ms ließ den
inneren `bwrap` (das erste Programm des neuen PID-Namensraums) mit Elternprozess 1 weiterlaufen; er startete
Weiterreicher und Harness. Über den Produktweg 2 von 60 Runden, mit reinem `bwrap` 19 von 300 Runden, unter Last 2073
von 2400. Ursache am Code von bubblewrap 0.12.0: `raw_clone` (`bubblewrap.c:3075`) erzeugt den inneren Prozess; sein
`PR_SET_PDEATHSIG` setzt er erst in `do_init` (`bubblewrap.c:610`), nach Warten, Mount-Aufbau und fork, und er prüft
danach `getppid()` nicht. `--json-status-fd` hilft nicht: Das Rennen liegt vor dem Schreiben der PID.

**Aufbau.** Das Programm ersetzt sich nicht mehr durch den Starter (Prüfbau: durch `bwrap`), sondern bleibt dessen
Elternprozess:
1. Es setzt `PR_SET_CHILD_SUBREAPER` und nimmt SIGHUP, SIGTERM, SIGINT und SIGQUIT an, bevor der Starter startet.
2. Es startet den Starter über das Trampolin: `werkbank-lauf-kind <PID des Programms> --halten <Starter> <Argumente>`.
   Das Trampolin setzt `PR_SET_PDEATHSIG = SIGKILL`, vergleicht `getppid()` mit der PID des Programms, ruft **kein**
   `setsid` (der Starter behält Steuerterminal und Vordergrundgruppe) und ersetzt sich durch den Starter. Deskriptor 3
   mit der Beschreibung bleibt offen. Scheitert dieses exec, schreibt das Trampolin `lauf.harness_fehlt: Harness nicht
   startbar` und endet mit 69, wie vorher das exec des Programms.
3. SIGHUP und SIGTERM reicht es über einen pidfd an den Starter weiter. SIGINT und SIGQUIT nimmt es an und reicht sie
   nicht weiter: Das Terminal sendet sie der ganzen Vordergrundgruppe, der Starter hat sie also schon; weitergereicht
   bekäme der Harness jedes doppelt.
4. Es wartet auf das Ende des Starters. Danach beendet es jedes andere eigene Kind (aus
   `/proc/self/task/*/children`) mit SIGKILL über einen pidfd und wartet es ab, höchstens 5 s. Das sind Prozesse, die
   es unter dem Starter adoptiert hat, etwa ein innerer `bwrap`, dessen äußerer `bwrap` zu früh starb. Such-Prozess,
   Modellweg und Wächter beendet es **nicht**: Diese beendet wie bisher der Wächter, sobald das Programm endet, und
   danach räumt er Sitzungsordner und Socket-Ordner.
5. Es endet mit dem Exitcode des Starters. Endete der Starter durch ein Signal, endet das Programm durch dasselbe
   Signal (Standardaktion), sodass der Träger wie bisher `128 + n` zählt. Für SIGILL, SIGFPE und SIGSEGV endet es mit
   dem Code `128 + n`.

Gemessen in der Nachstellung (Linux-Baurechner, Python, reiner `bwrap` mit den Schaltern des Einschlusses, SIGKILL an das Kind
d µs nach fork): mit Subreaper 0 Reste in 300 Runden (d bis 40 ms) und in 2400 Runden mit 40 gleichzeitig (d bis 4 ms);
ohne Subreaper 19 von 300 und 1212 von 2400.

**Restlücke (offen, Folgeschritt „Kern als Subreaper“).** Trifft ein SIGKILL das Programm selbst in den ersten
Millisekunden des Starters, fällt der innere `bwrap` wieder an den nächsten Subreaper oder PID 1: In der Nachstellung
blieben 128 von 300 Runden mit Resten (Elternteil getötet, Kind mit `PR_SET_PDEATHSIG` und Prüfung von `getppid()`).
Der Kern-Dienst sendet SIGKILL erst 1000 ms nach SIGHUP; ein SIGKILL von außen (OOM, `kill -9`) bleibt offen. Den Fall
schließt der Kern-Dienst als Subreaper (Projektentscheid, 21.11.2026: eigener Schritt, Pflicht vor der ersten
Auslieferung). Ungemessen im Lieferbau: ob `PR_SET_PDEATHSIG` des Trampolins den Profilwechsel in den Starter übersteht und ob
die Signale des Programms den Starter unter seinem AppArmor-Profil erreichen (Messung in der Wegwerf-VM).

**Geltung für andere Abschnitte.** Schritt 8, der Absatz zum Exitcode darunter und „Starter des Einschlusses“ tragen
diesen Aufbau im Wortlaut. Die Überschrift „Ablauf vor dem exec“ meint den Ablauf vor dem Start des Starters.

Fälle (`rust/crates/lauf/tests/`): `einschluss.rs`
`g1_the_enclosure_ends_also_when_the_starter_is_killed_in_its_first_milliseconds` (60 Runden, 0 Reste);
`kinder.rs` `the_program_passes_sighup_and_sigterm_on_to_the_starter`, `the_program_ends_by_the_signal_of_the_starter`,
`the_program_takes_sigint_and_sigquit_and_does_not_pass_them_on`, `the_program_ends_with_the_code_of_the_starter`,
`the_watcher_and_not_the_parent_ends_the_children_and_then_removes_the_folders` und
`g1b_a_refusal_after_the_exec_leaves_no_child_and_no_folder` (Ablehnung: Code des Starters, nichts bleibt). In den
Aufruf-Fällen steht `eltern_ist_programm` statt `gleiche_pid`.

**Sitzungsordner des Harness (Messung 21.11.2026, an earlier test run).** `sitzungen/<sitzung>` aus der eigenen Konfiguration
(Zustandsordner und Anbieter-Datei des Harness, Schritt 8) gehört der Sitzung und wird an ihrem Ende von seinem Eigner
entfernt, dem Wächter, und bei jedem Fehler vor dem exec vom Programm selbst. Das gilt nach dem Ende des Harness, nach der
Ablehnung durch den Starter, nach SIGKILL des Harness und nach einem Fehler vor dem exec. Gemessen vorher: Der Ordner blieb
liegen, nach einem Neustart des Kern-Dienstes begann die Zählung wieder bei `s-1`, und der Start endete mit
`lauf.sitzung_belegt`. Das Löschen geht nur über den eigenen Sitzungsnamen und folgt nie einem Verweis: Der Elternordner
wird geöffnet, der Name mit `AT_SYMLINK_NOFOLLOW` angesehen und mit `O_NOFOLLOW | O_DIRECTORY` geöffnet, jeder Eintrag
darunter ebenso, rekursiv über Deskriptoren und nie über Pfade (höchstens 32 Ebenen). Ein Verweis oder eine Datei an der
Stelle des Ordners bleibt unberührt; ein Verweis im Ordner wird als Verweis entfernt, sein Ziel nicht angefasst. Ein
Ordner, den dieser Lauf nicht angelegt hat, wird nie gelöscht. Die Marken gescheiterter Starts (Ordnermarke, Schritt 5b)
bleiben bewusst. Die Zielliste `sitzungen/<sitzung>.zielliste.json` im Zustandsordner der Installation bleibt (siehe dort).
Das Räumen toter Sitzungsordner beim Start des Kern-Dienstes und die Prüfung, dass keine Kennung mit bestehendem Ordner
vergeben wird, gehören dem Kern-Dienst (Projektentscheid, 21.11.2026).

**Dateien der Sitzung** im 0700-Ordner des Kern-Dienstes, je Modus 0600: die Startdatei, `suche.pid`, `modellweg.pid`
(der Name gilt für beide Betriebsarten) und bei einer Cloud-Route der Schlüssel der Wegwerf-CA mit der Kopie des
Bundles. Der Kern-Dienst räumt sie am Sitzungsende ab (Frage, 20.11.2026).

### Dateien je Sitzung ausserhalb des Sitzungsordners (Nachtrag 21.11.2026)

Diese Liste nennt jede Datei, jeden Ordner und jeden Socket, den Kern, Brücke, „Lauf starten“, Such-Prozess, Modellweg und
Wächter je Sitzung anlegen, mit Ort, Modus, Anleger und Entferner. **Stand: aus dem Code von `development` vom 21.11.2026
gelesen, nichts davon gemessen. Läufer: `development` vom 21.11.2026.** Die Spalte „Zeile“ nennt Datei und Zeile jenes Standes; sie veralten mit dem Code, die
Aussage bleibt. Es gelten diese Abkürzungen: `<run>` ist `/run/user/<uid>`, `<zustand>` ist `/var/lib/werkbank/<nutzer>`,
`<sitzungen>` ist `/var/lib/werkbank/s/<uid>`, `<laufzeit>` ist `/var/lib/werkbank/l/<uid>`. „Kern“ ist `werkbank-kern`
(Crate `dienst`), „Lauf“ ist `werkbank-lauf` (Crate `lauf`), „Wächter“ ist `werkbank-lauf-kind --wache`, „Suche“ ist der
Such-Prozess `rangkern sitzung`, „Modellweg“ ist `werkbank-modellweg`. Die Spalte „Stand“ trägt `ok`, wenn ein Entferner
zu jedem Ende der Sitzung greift, sonst `offen` mit der Nummer des Befunds unter der Tabelle.

**Kern und Brücke (Liste).**

| Datei | Ort, Modus | Anleger, Zeitpunkt (Zeile) | Entferner, Zeitpunkt (Zeile) | Stand |
|---|---|---|---|---|
| Ordner `werkbank/` | `<run>/werkbank`, 0700 | systemd (`RuntimeDirectory=werkbank` der Einheit `werkbank-kern.service`), Start der Einheit | systemd, Ende der Einheit, auch nach einem Absturz des Kerns | ok |
| `dienst.sock` | `<run>/werkbank/dienst.sock`, 0600 | Kern beim Start (`rechte.rs:137`); ein vorhandener Eintrag ist ein Startfehler (`rechte.rs:121`) | Kern über den Deskriptor des Ordners (`loeschen::datei_entfernen`): am Dienstende (`dienst.rs:1425`) und bei jedem Startfehler nach dem Binden (`dienst.rs:328`, `:341`, `:353`); sonst systemd mit dem Ordner | ok |
| Startordner `s-<n>/` mit `start.json` | `<run>/werkbank/s-<n>/`, 0700; Datei 0600 | Kern bei „Lauf starten“ (`starten.rs:178-206`) | Kern über Deskriptor (`loeschen::sitzung_entfernen`): an jedem Sitzungsende (Kind endet), bei gescheitertem Start (`dienst.rs:1193`, `:1211`), bei `Beenden` und am Dienstende (`aufraeumen.rs:178`); sonst systemd mit dem Ordner | ok |
| `bruecke.token`, `bruecke.port` | `<run>/werkbank/`, 0600 | Brücke beim Start, exklusiv (`rechte.rs:101`); ein vorhandener Eintrag ist ein Startfehler | Brücke am Ende über den Deskriptor des Ordners mit Inode-Vergleich (`rechte.rs:57-66`, `server.rs:200`; bei Startfehler `server.rs:90`); sonst systemd mit dem Ordner des Kerns | ok |

Kern und Brücke legen je Sitzung nichts ausserhalb von `<run>/werkbank` an und schreiben keine Protokolldatei (nur
Standardfehler, also das Journal der Einheit). Sie lesen nur: `/etc/werkbank/kern.json`, `lauf.einstellungen`,
`lauf.sitzungen/s-<n>` (nur `lstat`) und `lauf.marker_datei` (einmal beim Start). Die Einheit der Brücke hat keinen eigenen
`RuntimeDirectory` und hängt mit `BindsTo=` am Kern.

**„Lauf starten“ (`werkbank-lauf`), ausserhalb des Sitzungsordners.**

| Datei | Ort, Modus | Anleger, Zeitpunkt (Zeile) | Entferner, Zeitpunkt (Zeile) | Stand |
|---|---|---|---|---|
| `suche.pid`, `modellweg.pid` | im Startordner (neben `start.json`), 0600 | Lauf, Schritt 8, nach beiden Kindern, `O_EXCL` (`ablauf.rs:614-624`, `:67-71`) | Lauf nur bei einem Fehler vor dem exec (`ablauf.rs:74-78`, `:123`); sonst Kern mit dem Startordner, vorher SIGTERM an die Prozessgruppen aus den Dateien, nur wenn `pid_pruefen` hält (`aufraeumen.rs:1-14`). Ohne laufenden Kern niemand | ok mit Kern; ohne Kern offen (Befund 6) |
| `ca.key`, `ca.crt` | im Startordner, 0600 | Lauf, Schritt 7a, nur Cloud-Route, `O_EXCL` (`ablauf.rs:553-560`) | wie `suche.pid` | ok mit Kern; ohne Kern offen (Befund 6) |
| `ca-bundle.pem` | im Startordner, 0600 | Lauf, Schritt 7a, nur Cloud-Route mit Harness `copilot` (`ablauf.rs:561-570`) | wie `suche.pid` | ok mit Kern; ohne Kern offen (Befund 6) |
| `sitzungen/<sitzung>.zielliste.json` | `<zustand>/sitzungen/`, Datei 0600, Ordner 0700 (von `postinst`) | Lauf, Schritt 7a0, nur Cloud-Route, `O_EXCL`, vor dem Modellweg (`ablauf.rs:534-544`); sie bleibt mit Absicht (Kommentar `ablauf.rs:534`) | Läufer nach seiner Suche im echten Lauf (`--ziel echt`), direkt nach der Suche, über einen Deskriptor des Elternordners (`rust/spec/vorfuehrung/laeufer/drehbuchlib/messen.py`, `zielliste_entfernen`). Sonst entfernt sie der Kern beim Vergeben derselben Nummer nach einem Neustart, über den Deskriptor des Ordners `lauf.ziellisten`, nur als eigene reguläre Datei ohne anderen Eintrag der Nummer (Vertrag `dienst`, DI12 Punkt 15a; `dienst` `starten.rs`, `alte_zielliste_raeumen`). `purge` entfernt den Rest. Im Code von „Lauf starten“ entfernt sie niemand | ok mit Kern (Befund 1); ohne Kern offen |
| Socket-Ordner `<sitzung>/` | `<laufzeit>/<sitzung>/`, 0700 | Lauf, Schritt 7 (`ablauf.rs:352-360`) | Wächter zuletzt (`kind.rs:379`); Lauf bei einem Fehler vor dem exec (`ablauf.rs:124-127`). Beide mit `remove_dir_all` über den Pfad. Niemand, wenn Wächter und Lauf beide ohne Aufräumen enden (SIGKILL) | offen (Befund 4, 5) |
| Sitzungsordner (zur Vollständigkeit) | `<sitzungen>/<sitzung>/`, 0700, mit `zustand/` und `konfig/` | Lauf, Schritt 8 (`ablauf.rs:684-690`, `:696`) | Wächter am Ende (`kind.rs:378`); Lauf bei einem Fehler vor dem exec (`ablauf.rs:128-131`); beide über Deskriptoren (`dateien.rs:156-215`). Räumen toter Ordner beim Start des Kerns gehört dem Kern (Projektentscheid, 21.11.2026) | ok |

**Suche und Modellweg, je Sitzung.**

| Datei | Ort, Modus | Anleger, Zeitpunkt (Zeile) | Entferner, Zeitpunkt (Zeile) | Stand |
|---|---|---|---|---|
| `such.sock` | im Socket-Ordner, 0600 | Suche beim Start, nur wenn der Pfad frei ist (`rangkern` `sitzung/lauf.rs:97-114`) | Suche bei SIGTERM und SIGINT (`sitzung/lauf.rs:75-79`, `:87-93`); sonst der Wächter mit dem Ordner | ok (bei SIGKILL des Kindes räumt der Wächter) |
| `modell.sock` | im Socket-Ordner, 0600 | Modellweg beim Start, nur wenn der Pfad frei ist (`start.rs:336-343`) | Modellweg bei SIGTERM und SIGINT (`start.rs:457-462`), bei Startfehlern (`start.rs:374`, `:386`, `:436`); sonst der Wächter mit dem Ordner | ok (wie `such.sock`) |
| `<sitzung>.suche.json` | im Zählordner, 0600; nur bei Probelauf (`marker`) | Suche, beim Start und je Zählung, über eine Nebendatei und `rename` (`sitzung/probelauf.rs:59-71`, `:121-138`) | niemand im Code; der Läufer liest sie. Das Löschen steht in keinem Vertrag | offen (Befund 2) |
| `<sitzung>.modellweg.json` | im Zählordner, 0600; nur bei Probelauf | Modellweg, beim Start (Schritt 5) und je Zählung, über eine Nebendatei und `rename` (`probelauf.rs:44-46`, `:157-175`; `start.rs:203`) | niemand im Code; der Läufer liest sie | offen (Befund 2) |
| Nebendatei `<zaehldatei>.<pid>.neu` | im Zählordner, 0600 | Suche und Modellweg, kurz vor dem `rename` (`sitzung/probelauf.rs:145-148`, `probelauf.rs:157-163`) | der eigene Prozess nach Fehler (`sitzung/probelauf.rs:138`, `probelauf.rs:174`); nach SIGKILL zwischen Anlegen und `rename` niemand | offen (Befund 3) |
| Zeilen in `gateway-<JJJJ-MM>.jsonl`, `brain-<JJJJ-MM>.jsonl` und den Ankerdateien `<quelle>.anker.jsonl` | `<zustand>/protokoll/`, 0600 | Modellweg (`protokoll.rs:135`, `:147`, `:291`) und Suche (`sitzung/lauf.rs:228-235`) über `formate`: Monatsdatei mit `create_new` (`kette/monate.rs:209-212`), Anker mit `create` (`:227-232`, `:361`) | niemand mit Absicht: die Kette wächst über Monate und ist kein Sitzungszustand. Nur eine Datei, die ein fehlgeschlagener Schreibvorgang neu gemacht hat, wird sofort entfernt (`kette/monate.rs:250-256`) | ok (kein Sitzungszustand) |
| `ordnermarken.json`, `ordnermarken.lock` | `<zustand>/`, 0600 | Lauf, Schritt 5b, je Start unter `flock` (`marke.rs:279-286`, `:363-387`) | niemand mit Absicht: die Marke gilt über die Sitzung hinaus (E6) | ok (kein Sitzungszustand) |
| Nebendatei `ordnermarken.json.<pid>.neu` | `<zustand>/`, 0600 | Lauf, kurz vor dem `rename` (`marke.rs:364-372`) | der Lauf nach Fehler (`marke.rs:384`); nach SIGKILL zwischen Anlegen und `rename` niemand | offen (Befund 3) |

Der Wächter legt nichts an. Der Such-Server für den Agenten (`such-mcp`) und der Befehl `werkbank-modellweg suche` legen
nichts an; die Suche steht dazu im Kopfkommentar von `suche.rs` („writes no file and no protocol line, makes no socket“),
und die Dateioperationen darin stehen nur im Testteil (`suche.rs:467` und danach). Der Ordner des Harness in `konfig/` und
`zustand/` liegt im Sitzungsordner und fällt mit ihm.

**Befunde.** Jede Zeile mit „offen“ in der Tabelle ist hier belegt. Im Code änderte dieser Nachtrag nichts.

1. **Die Zielliste räumt „Lauf starten“ nie; der Läufer nur im echten Lauf; mit Kern räumt sie seit 23.11. der Kern (DI12 Punkt 15a).** `ablauf.rs:539-544` legt sie an,
   `ablauf.rs:124-131` räumt sie auch bei einem Fehler vor dem exec nicht (`Spuren` führt sie nicht). Der Läufer
   (`development` vom 21.11.2026) entfernt die Zielliste seiner ersten Cloud-Sitzung im echten Lauf direkt nach der Suche,
   über einen Deskriptor des Elternordners (`O_DIRECTORY | O_NOFOLLOW`, `os.stat` ohne Verweis, nur eine reguläre Datei
   der eigenen uid, Kennung `^s-[0-9]{1,9}$`; `messen.py` `zielliste_entfernen`). Lief die Suche nicht oder bricht der Lauf
   vorher ab, bleibt sie liegen (Läufer-Vertrag, Abschnitt „Zielliste nach der Suche“). **Offen blieben bis zum 23.11.** der Abbruch vor
   der Suche, ein Start ohne Läufer und ein Fehler vor dem exec. Folge dort: Je Cloud-Sitzung bleibt eine Datei; nach
   einem Neustart des Kerns (Zählung ab `s-1`) scheitert der zweite Lauf mit 73, `lauf.zielliste`, weil `O_EXCL` die Datei
   ablehnt. Liegen für alle Nummern, die ein Lauf des Kerns vergeben darf, solche Listen, lehnt der Kern jeden Start
   mit `nummern_erschoepft` ab. Die Grenze je Lauf des Kerns regelt der Vertrag dienst (DI12 Punkt 4); dieser
   Vertrag nennt keine eigene Zahl. Entschieden ist: Die Zielliste bleibt nach der
   Sitzung liegen, weil der Läufer danach `werkbank-modellweg suche --zielliste` ruft (Mutante `loeschen-am-ende`). Gebaut:
   Der Kern vergibt keine Nummer, deren Zielliste liegt (Schlüssel `lauf.ziellisten` in
   `kern.json`, DI12 Punkt 15). **Nachtrag 23.11.2026:** Die alten Listen ohne
   Läufer räumt der Kern beim Vergeben. Ergibt die Prüfung nach Punkt 15, dass die Nummer `n` nur durch ihre eigene
   reguläre Zielliste belegt ist, entfernt er diese über den Deskriptor des Ordners `lauf.ziellisten` (`openat` mit
   `O_DIRECTORY | O_NOFOLLOW`, `fstatat` ohne Folgen, `unlinkat`) und vergibt `n` (DI12 Punkt 15a). Der Kern fragt nur nach
   Nummern über allen Nummern seines Laufs. Die Listen der Sitzungen dieses Laufs bleiben also für den Läufer liegen, und
   der Leser des Läufers ändert sich nicht. Ein Verweis, ein Ordner, eine fremde Datei oder eine Liste neben einem anderen
   Eintrag der Nummer belegt weiter. Der Fall dazu: `rust/crates/lauf/tests/kreuz.rs`,
   `zweihundertsechsundfuenfzig_cloud_sitzungen_ohne_laeufer`. Er startet `NUMMERN` Cloud-Sitzungen ohne Läufer über den
   echten Kern und den echten `werkbank-lauf` (Konstante im Fall). Dann startet er den Kern neu und verlangt `s-1` für
   den nächsten Start; ohne 15a lautet die Antwort `nummern_erschoepft`. Der Vorschlag, die Liste bei einem Fehler vor
   dem exec in „Lauf starten“ zu räumen, ist nicht gebaut (Projektentscheid, 23.11.2026). Solche Listen räumt der Kern ebenso beim Vergeben.
   **Offen bleibt** nur der Betrieb ohne Kern: Dort räumt niemand.
2. **Die Zähldateien `<sitzung>.suche.json` und `<sitzung>.modellweg.json` räumt niemand.** Der Zählordner gehört dem
   Läufer (Schritt 5a prüft ihn nur). Folge: ein Lauf mit gleicher Kennung überschreibt die alte Datei still über
   `rename`; bei einem Läufer, der den Ordner nicht räumt, wächst er mit jeder Sitzung. Vorschlag: Der Läufer entfernt beide
   Dateien nach dem Lesen der Zählung; der Vertrag nennt ihn als Entferner.
3. **Nebendateien `*.<pid>.neu`.** Sie bleiben nach SIGKILL zwischen Anlegen und `rename` liegen (Zählordner und
   `<zustand>`). Der nächste Prozess mit gleicher PID entfernt seine eigene Nebendatei zuerst (`marke.rs:365`,
   `probelauf.rs:162`), jeder andere nie. Folge: harmlose Reste (0600, Ordner 0700), aber auch hier „niemand“. Vorschlag: Die
   Räumung nimmt alle `*.neu` im Ordner, deren PID nicht lebt.
4. **Socket-Ordner mit `remove_dir_all` über den Pfad** (`ablauf.rs:126`, `kind.rs:379`). Anders als der Sitzungsordner
   (`dateien.rs:156-215`) hält dieser Weg keinen Deskriptor des Elternordners. Hergeleitet aus dem Verhalten von
   `std::fs::remove_dir_all` (der letzte Namensteil als Verweis wird als Verweis entfernt, Einträge darunter werden über
   Deskriptoren geräumt), nicht gemessen: Ein Verweis anstelle eines Elternordners (`<laufzeit>`) würde den Pfad umlenken.
   Der Elternordner gehört dem Nutzer (0700, von `postinst`), darum ist das nur mit der Kennung des Nutzers selbst
   erreichbar. Vorschlag: `dateien::ordner_entfernen` auch für den Socket-Ordner (ein Satz im Vertrag, eine Änderung
   im Code).
5. **Ein Socket-Ordner, den beim Ende niemand räumt, blockiert die Kennung.** Endet der Wächter ohne Aufräumen (SIGKILL
   von aussen) und lebt der Lauf nicht mehr, bleibt `<laufzeit>/<sitzung>/` liegen. `ordner_neu` scheitert dann
   (`ablauf.rs:358`, `AlreadyExists` wird zu `lauf.suche_fehlt`). Der Kern prüft heute nur `lauf.sitzungen` (DI12 Punkt 13),
   nicht `<laufzeit>`; nach dem Neustart des Kerns (Zählung ab `s-1`) trifft ein neuer Start den alten Ordner. Vorschlag:
   Der Kern prüft `<laufzeit>/<sitzung>` wie den Sitzungsordner (neuer Schlüssel neben `lauf.ziellisten`).
6. **Dateien neben `start.json` ohne laufenden Kern** (`suche.pid`, `modellweg.pid`, `ca.key`, `ca.crt`, `ca-bundle.pem`).
   Es räumt nur der Kern (siehe „Ehrliche Grenze“ oben); `ablauf.rs:74-78` räumt nur bei einem Fehler vor dem exec und über
   den Pfad (`remove_file`), nicht über den Deskriptor des Ordners. Folge: Nach einer Ablehnung durch den Starter ohne
   Kern bleibt `ca.key` mit höchstens 23 h Gültigkeit liegen. Offen für Schritt II.

Zu 4 und 6: Ein Verweis im Pfad wird beim Entfernen nie verfolgt, wenn das Ziel selbst ein Verweis ist (`remove_file` und
`remove_dir_all` entfernen den Verweis); nur ein umgelenkter **Elternordner** ist die Lücke. Alle Aussagen dieses
Abschnitts sind aus dem Code gelesen. Vor M1 braucht jede Zeile mit „offen“ einen Fall in `faelle/` oder eine
Entscheidung, dass sie offen bleibt. Für die Befunde 2 bis 5 gibt es beides noch nicht (OPEN 19).

**Heutige Stellen am Stand vom 23.11.2026 (Code seit dem 23.11.2026 unverändert).** Die Tabellen oben tragen den Stand vom 21.11.2026; die Aussagen gelten weiter, die Zeilen sind verschoben: Zielliste `ablauf.rs:731-741` (statt `:534-544`, `:539-544`); Aufräumen nach Fehler `ablauf.rs:178-189` (statt `:124-131`); `remove_dir_all` des Socket-Ordners `ablauf.rs:184` und `kind.rs:457` (statt `ablauf.rs:126`, `kind.rs:379`); Sitzungsordner durch den Wächter `kind.rs:456` (statt `:378`); `Spuren::loeschen` mit `remove_file` `ablauf.rs:78-82` (statt `:74-78`); PID-Dateien `ablauf.rs:811-824` (statt `:614-624`); Socket-Ordner anlegen `ablauf.rs:549-557` (statt `:352-360`).

**Startdatei im Heim oder in einem geteilten Ordner (Messung 21.11.2026).** Das Programm verbietet den Ort nicht; die
Startdatei darf in jedem Ordner liegen. Gemessen (Wegwerf-VM, `strace`): Lagen `start-1.json`, `start-2.json` und
`start-3.json` im selben Ordner `/home/wb`, endete schon die zweite Sitzung mit 70, `lauf.intern`: Der erste Lauf hatte
`suche.pid` und `modellweg.pid` hinterlassen, das Programm schreibt nie über eine Datei, und der Fehler beim Schreiben
war nicht benannt. Das ist jetzt Schritt 5c: ein fester Code (73, `lauf.startordner_belegt`) vor dem Start des ersten
Kindes; die Datei bleibt unberührt. Nur wenn eine solche Datei erst nach Schritt 5c entsteht (Wettlauf), endet der
Start noch mit `lauf.intern`, nachdem er die Kinder beendet hat. Wirklich leer wird der
Ordner erst, wenn der Kern-Dienst die Dateien am Sitzungsende räumt; ein Aufrufer von Hand legt darum für jede
Sitzung einen eigenen privaten Ordner an (0700), wie der Kern-Dienst es tut.

**Aufräumen nach `lauf.startordner_belegt` (Anleitung für Aufrufer von Hand).** Endet ein Start mit 73,
`lauf.startordner_belegt`, liegen Dateien einer früheren Sitzung im Ordner der Startdatei. Zuerst prüfen, dass aus diesem
Ordner keine Sitzung mehr läuft: Die PIDs stehen in `suche.pid` und `modellweg.pid`; lebt einer der Prozesse noch, gehört
der Ordner einer laufenden Sitzung und bleibt unberührt. Läuft keine, die Dateien `suche.pid`, `modellweg.pid`, `ca.key`,
`ca.crt` und `ca-bundle.pem` in diesem Ordner löschen, oder für den neuen Start einen neuen privaten Ordner (0700)
anlegen. Mit laufendem Kern-Dienst ist das nicht nötig; er räumt am Sitzungsende.

**Ehrliche Grenze (offen für Schritt II).** Ein Aufruf von Hand ohne laufenden Kern-Dienst lässt nach einer Ablehnung
durch den Starter das befristete Zertifikat samt Schlüssel im Ordner der Startdatei liegen (Ordner 0700, höchstens 23 h
gültig, nur für die Ziele der Zielliste). Der Wächter räumt Kinder, Sitzungsordner und Socket-Ordner; einen zweiten Räumer für diese
Dateien gibt es in M1 bewusst nicht (Projektentscheid, 21.11.2026: der Kern-Dienst räumt).

Liegt der Arbeitsordner unter dem Ordner
der Startdatei (die Startdatei im Heim, der Arbeitsordner darunter), überlappt er einen geschützten Ordner, und der
Starter lehnt ab: 69, `lauf.einschluss_fehlt` (Fall des Starters, aus dem Code hergeleitet:
`rust/crates/einschluss/src/profil.rs`, Prüfung „mount overlaps a protected folder“, nicht in der VM gemessen).

Bedingung für Schritt 1: `PR_SET_PDEATHSIG` hängt am **Thread**, der geforkt hat. Das Programm forkt deshalb aus dem
Hauptthread und ist beim fork einthreadig. Sonst feuert das Signal schon beim exec, weil exec die übrigen Threads
beendet.

Gründe für den früheren Weg (exec, bis 21.11.2026): SIGWINCH, Vordergrundgruppe und Exitcode blieben ohne
Weiterleitung beim Harness. Er ist ersetzt, weil das Todessignal des inneren `bwrap` zu spät kommt (Abschnitt
„Elternprozess des Starters“). Vordergrundgruppe und SIGWINCH bleiben auch jetzt ohne Weiterleitung beim Harness: Der
Starter erbt Terminal und Prozessgruppe. Endet der Such-Prozess vor dem Harness, bleibt er bis zum Ende des Programms
als Zombie stehen, weil das Programm ihn nicht abholt. Das ist harmlos.

Variante A (Linux): Das Programm startet den Starter des Einschlusses als Kind (Abschnitt „Starter des Einschlusses“)
und lebt, bis der Starter endet; die Bindung gilt damit unverändert. Variante B
(Rust-Einschluss): Das Programm baut den Einschluss selbst und macht exec direkt in den Harness. Der Vertrag trägt beide
Varianten, denn Aufruf, Codes, Bindung und die Eigenschaft E3 sind gleich.

Windows bindet später über das Job-Objekt; macOS kennt kein `PR_SET_PDEATHSIG` (dort wäre es `kqueue` mit `NOTE_EXIT`
im Such-Prozess). Fassung 1 startet unter macOS nie (siehe „Plattformen in Fassung 1“).

### Paket der Copilot CLI (Schritt 6a, Nachtrag 21.11.2026)

**Anlass (gemessen 21.11. mit der CLI 1.0.88).** Die CLI entpackt bei jedem Start vor jeder Oberfläche ihr eingebettetes
Paket (166 MB, 217 Dateien) nach `($XDG_CACHE_HOME oder $HOME/.cache)/copilot/pkg/<plattform>/<fassung>`, wenn dort nicht
die vier Pfade `index.js`, `app.js`, `prebuilds/<plattform>/runtime.node` und `.extraction-complete` stehen. Im Einschluss
ist das Heim ein tmpfs mit 16 MiB; der Start scheitert mit `ENOSPC`. `--version` entpackt, `--binary-version` nicht. Fehlt
einer der vier Pfade, entpackt die CLI erneut. `bwrap` löst Verweise auf; eine Prüfung auf Verweise geht nur auf dem Host.

**Wer und wann.** „Lauf starten“ selbst, in Schritt 6a vor dem ersten Kind, nur für den Harness `copilot`. Die CLI läuft
dafür nie außerhalb eines Einschlusses.

1. **Form lesen, ohne ein Programm auszuführen.** `A` ist der Ordner des echten Pfads von `harness.copilot`.
   `A/package.json`: `name` ist `@github/copilot`, `version` hat die Form `<zahl>.<zahl>.<zahl>`, `bin.copilot` ist der
   Dateiname von `harness.copilot`. Plattform ist `linux-` und die Architektur des Rechners (`x86_64` → `x64`,
   `aarch64` → `arm64`); ein Bau ohne glibc kennt keine. Das Plattformpaket liegt am ersten vorhandenen Ort in der
   Reihenfolge von Node: (1) `A/node_modules/@github/copilot-<plattform>`, (2) `<scope>/copilot-<plattform>`, nur wenn
   `A` die Form `…/node_modules/@github/<name>` hat. Dort: `name` ist `@github/copilot-<plattform>`, `version` gleich
   der des Hauptpakets, `os` enthält `linux`, `cpu` die Architektur, `bin["copilot-<plattform>"]` ist `copilot`, und die
   Datei `copilot` daneben ist eine gewöhnliche Datei ohne Verweis. Beide `package.json` sind gewöhnliche Dateien (kein
   Verweis, höchstens 1 MiB). Jede andere Form: 78, `lauf.paket_form`, kein Kind. Die Fassung kommt immer aus dem Paket.
2. **Cache-Ordner.** `<cache>` ist `<xdg>/werkbank/harness/copilot`, wenn `XDG_CACHE_HOME` des Programms ein absoluter,
   normaler Pfad echt unter `heim` (aus der eigenen Konfiguration) ist, sonst `<heim>/.cache/werkbank/harness/copilot`;
   nie unter `/var/tmp`. Fehlende Ordner entstehen mit 0700. `<cache>` muss danach ein echter Pfad ohne Verweis sein, ein
   Ordner der eigenen Kennung mit Modus genau 0700, ohne Geheimnis-Segment und ohne Berührung eines Anmeldespeichers
   (dieselben Listen wie für die Programmordner); sonst 73, `lauf.paket_cache`.
3. **Sperre.** `flock` exklusiv auf `<cache>/.werkbank.lock` (0600, `O_NOFOLLOW`), Frist 180 s. Zwei Starts zugleich:
   einer entpackt, der andere wartet und findet danach den vollständigen Ordner.
4. **Prüfung auf dem Host** von `<cache>/pkg/<plattform>/<fassung>`: Der Ordner und jeder der vier Pfade hat einen
   echten Pfad gleich seinem Pfad (kein Verweis, auch nicht darüber), gehört der eigenen Kennung, und die vier Pfade sind
   gewöhnliche Dateien. Vollständig: kein Entpacken. Verweis oder fremder Eigner: 73, `lauf.paket_cache`, nichts wird
   gebunden und nichts verändert.
5. **Unvollständig** (einer der vier Pfade fehlt oder ist keine Datei): Der Ordner wird nicht gelöscht, sondern über
   Ordnerdeskriptoren (`O_NOFOLLOW`, nur eigene Ordner, `renameat`) nach `<cache>/.werkbank-beiseite/<plattform>-<fassung>-<ns>`
   umbenannt; dann entpackt der kleine Einschluss neu. Fehlt der Ordner, entpackt er direkt.
6. **Kleiner Einschluss.** Derselbe Satz Pflichtschalter wie das Profil der Sitzung (eigene Namensräume, kein Netz,
   `--disable-userns`, `--cap-drop ALL`, `--clearenv`), `/usr` und der Systemteil vom Host nur lesend, die Programmordner
   nur lesend, `/tmp`, `/run` und das Heim als tmpfs mit je 16 MiB, und eine einzige schreibbare Bindung: `<cache>` an
   `<heim>/.cache/copilot`. Umgebung `HOME`, `PATH`, `COPILOT_AUTO_UPDATE=false`; Aufruf `<harness.copilot> --version`,
   als Datei gestartet wie der Harness selbst (Shebang), nie als `node <pfad>`. Kein Netz, kein Token. Frist 120 s
   (gemessen 3,93 s). Im Lieferbau läuft der kleine Einschluss über den Starter (`werkbank-einschluss vorentpacken
   --beschreibung-fd 3`, Kind des Programms), im Prüfbau über das `bwrap` der Konfiguration mit derselben Argumentliste.
   Danach dieselbe Prüfung wie Punkt 4. Endet das Entpacken nicht mit 0 oder fehlen danach Pfade: 69,
   `lauf.paket_entpacken`, kein Kind.
7. **Einschluss der Sitzung.** Das Feld `paket` der Beschreibung (unten) bindet den Ordner nur lesend genau an
   `<heim>/.cache/copilot/pkg/<plattform>/<fassung>`. Der Starter prüft den Ordner vor dem `exec` noch einmal wie Punkt 4.

**Zweite Zeile.** `lauf.paket_cache` und `lauf.paket_entpacken` schreiben nach der Zeile `<code>: <grund>` eine zweite feste
Zeile `lauf.paket_teil: <exit des Entpackens oder -> <wort>`. Wörter bei `lauf.paket_cache`: `verweis`, `fremd`, `modus`,
`anmeldespeicher`, `anlegen`, `sperre`, `beiseite`; bei `lauf.paket_entpacken`: `rc` (Exit ungleich 0), `kein-platz`
(die Ausgabe nennt `ENOSPC`), `starter` (der Starter lehnte ab), `unvollstaendig` (Exit 0, Pfade fehlen), `frist`,
`start`, `sperre-frist`. Die Ausgabe des Entpackens selbst erscheint nirgends; nur das Wort wird aus ihr abgeleitet.

**Offen (nicht Pflicht für M1).** Aufräumen alter Fassungen und der Ordner in `.werkbank-beiseite`; eine zweite Fassung
neben der ersten (der Cache trägt beide, gebunden wird nur die des Pakets); Schreibzugriffe der CLI unter
`~/.cache/copilot` außerhalb von `pkg/` im Betrieb (gebunden ist nur der Paketordner; der Rest des Heims bleibt tmpfs).

### Starter des Einschlusses (Variante A, Linux; Nachtrag 20.11.2026)

Auf Linux mit aktiver Sperre (`kernel.apparmor_restrict_unprivileged_userns = 1`) ruft „Lauf starten“ als letzten
Schritt nie `/usr/bin/bwrap`. Es startet als Kind über das Trampolin und bleibt dessen Elternprozess (Abschnitt
„Elternprozess des Starters“):

```
/usr/lib/werkbank/werkbank-einschluss start --beschreibung-fd 3
```

- „Lauf starten“ (`werkbank-lauf`) trägt kein AppArmor-Profil. Das Profil trägt allein der Starter am festen Pfad
  `/usr/lib/werkbank/werkbank-einschluss`; liegt er woanders, ist der Aufruf nicht erlaubt (Exit 7).
- Der Starter hat keinen Suchpfad: bwrap ist die Konstante `/usr/bin/bwrap`. Eingabe über Umgebungsvariable, Datei oder
  freies Argument gibt es nicht.
- Auf fd 3 liegt das Lesende einer Pipe, die „Lauf starten“ selbst anlegt und beschreibt. Die Schreibenden sind vor dem
  exec geschlossen. Eine Datei, ein `memfd` oder eine Pipe, die ein anderer Nutzer besitzt, lehnt der Starter ab (Exit 3).
  Die Beschreibung hat höchstens 65536 Byte.
- Alle Argumente außer den drei Aufrufen (unten) sind Exit 2. Der Starter schreibt bei jedem Fehler die Zeile
  `<code>: <grund>` auf stderr; bei `lauf.einschluss_fehlt` folgt eine zweite, feste Zeile mit dem Teilcode (siehe unten).
  Nie ein Pfad und nie ein Dateiinhalt.

**Aufrufe des Starters.**

| Aufruf | Zweck |
|---|---|
| `werkbank-einschluss start --beschreibung-fd 3` | Beschreibung lesen, prüfen, bwrap bauen, `exec` auf bwrap; kehrt nie zurück |
| `werkbank-einschluss selbsttest` | Paket (postinst-Probe, `werkbank-pruefen`): als Nutzer ohne Vorrechte; Exit 0 heißt, der Einschluss ist baubar |
| `werkbank-einschluss vorentpacken --beschreibung-fd 3` | Schritt 6a: kleinen Einschluss lesen (`{"heim", "cache", "programmordner", "programm", "path"}`, geschlossen, sonst Exit 3), prüfen (Cache-Ordner endet auf `werkbank/harness/copilot` echt unter `heim`, eigener Ordner ohne Verweis; sonst Exit 5), `exec` auf bwrap um `<programm> --version`; der Exit ist dann der des Programms |

**Exitcodes und Zeilen des Starters.** Außer bei Exit 2 ist die erste Zeile `<code>: <grund>` aus der Tabelle der Codes
oben; die Spalte rechts nennt sie wörtlich. Bei `lauf.einschluss_fehlt` folgt eine zweite Zeile (Abschnitt „Teilcode“ unter der Tabelle).

| Exit | Bedeutung | stderr |
|---|---|---|
| 0 | `selbsttest` gelaufen (bei `start` kehrt der Aufruf nie zurück) | keine |
| 2 | Aufruf ist keiner der drei | `werkbank-einschluss: Aufruf ungueltig` |
| 3 | Eingabe abgelehnt: fd 3 zu, keine Pipe, nicht lesbar oder Frist, über 65536 Byte, kein JSON, unbekannter Schlüssel, fehlendes Feld, falscher Typ | `lauf.einschluss_fehlt: Einschluss nicht verfuegbar`, dazu die zweite Zeile `lauf.einschluss_teil: 3 <Wort>` (Abschnitt „Teilcode“) |
| 4 | Arbeitsordner fehlt oder ist kein Ordner | `lauf.ordner_fehlt: Arbeitsordner fehlt oder ist kein Ordner` |
| 5 | Beschreibung ungültig, verboten, überlappend oder Quelle unbrauchbar | `lauf.einschluss_fehlt: Einschluss nicht verfuegbar`, dazu die zweite Zeile `lauf.einschluss_teil: 5 <Wort>` (Abschnitt „Teilcode“) |
| 6 | Host kann nicht: keine Plattform, bwrap fehlt oder nicht ausführbar, TIOCSTI offen, `/proc/sys/...` nicht lesbar | `lauf.einschluss_fehlt: Einschluss nicht verfuegbar`, dazu die zweite Zeile `lauf.einschluss_teil: 6 <Wort>` (Abschnitt „Teilcode“) |
| 7 | System verweigert den Namensraum: Profil des Starters nicht geladen, Starter an anderem Pfad, Schalter aus | `lauf.einschluss_fehlt: Einschluss nicht verfuegbar`, dazu die zweite Zeile `lauf.einschluss_teil: 7 <Wort>` (Abschnitt „Teilcode“) |
| 8 | Probelauf oder Selbsttest sonst fehlgeschlagen | `lauf.einschluss_fehlt: Einschluss nicht verfuegbar`, dazu die zweite Zeile `lauf.einschluss_teil: 8 <Wort>` (Abschnitt „Teilcode“) |
| 9 | `exec` auf bwrap fehlgeschlagen | `lauf.einschluss_fehlt: Einschluss nicht verfuegbar`, dazu die zweite Zeile `lauf.einschluss_teil: 9 <Wort>` (Abschnitt „Teilcode“) |

**Teilcode (Projektentscheid, 21.11.2026).** `lauf.einschluss_fehlt` bleibt der feste Code; dieselbe Zeile für alle Gründe
machte die Ursache nur mit `strace` auffindbar (Messung 21.11.2026, an earlier test run). Der Starter schreibt darum nach der
Zeile `lauf.einschluss_fehlt: Einschluss nicht verfuegbar` bei jedem Exit 3 und 5 bis 9 eine **zweite Zeile** in fester Form:

    lauf.einschluss_teil: <Exitcode> <Wort>

`<Wort>` besteht aus Kleinbuchstaben und Bindestrichen und kommt aus dieser Tabelle; es nennt nie einen Pfad und nie einen
Inhalt (die Variante des Fehlers entscheidet, nicht sein Detailtext). Der Starter schreibt sie selbst: Das Programm sieht
als sein Elternprozess nur seinen Exitcode und endet mit ihm; der Wächter ist nicht der Elternprozess und sieht den Exit nicht. Exit
2 und Exit 4 (eigener Code `lauf.ordner_fehlt`) tragen keine zweite Zeile.

| Exit | Wörter |
|---|---|
| 3 | `eingabe-nicht-lesbar`, `eingabe-kein-rohr`, `eingabe-zu-gross`, `eingabe-ungueltig` |
| 5 | `beschreibung-ungueltig`, `verboten`, `ueberlappt`, `quelle-unbrauchbar` |
| 6 | `keine-plattform`, `bwrap-fehlt`, `bwrap-nicht-ausfuehrbar`, `tiocsti-offen` |
| 7 | `namensraum-gesperrt` |
| 8 | `probelauf` |
| 9 | `exec-gescheitert` |

Der Starter läuft als Kind von „Lauf starten“, das mit seinem Code endet; sein Exit 2 bis 9 ist damit der Exit des
Aufrufs (Abschnitt „Elternprozess des Starters“, Punkt 5). Die Exitcodes 64 bis 78 der Tabelle „Ablauf vor dem exec“
gelten bis zum Start des Starters; scheitert das exec des Trampolins in den Starter, ist der Exit 69 mit
`lauf.harness_fehlt`.

**Beschreibung** (JSON, UTF-8, höchstens 65536 Byte, geschlossenes Objekt, jedes Feld Pflicht, jeder Pfad absolut und
normal, also ohne `.`, `..`, doppelten Schrägstrich und Schrägstrich am Ende):

```json
{"harness": "pi", "arbeitsordner": "/srv/beispiel/projekt", "programmordner": ["/usr/lib/werkbank"],
 "zustand": {"host": "/var/lib/werkbank/beispiel/sitzungen/s-1", "name": "zustand"},
 "heim": "/srv/beispiel/heim", "such_socket": "/run/werkbank/beispiel/s-1/such.sock",
 "modell_socket": "/run/werkbank/beispiel/s-1/modell.sock", "lokal_weiter": "/usr/lib/werkbank/einschluss-weiter",
 "cloud": null, "paket": null,
 "umgebung": {"home": "/srv/beispiel/heim", "path": "/usr/bin:/bin", "term": "xterm-256color", "lang": "C.UTF-8", "tmpdir": "/tmp"},
 "geschuetzt": ["/var/lib/werkbank"],
 "dateien": [{"quelle": "/var/lib/werkbank/beispiel/sitzungen/s-1/models.json", "ziel": "/srv/beispiel/heim/.pi/agent/models.json"}],
 "argv": ["/usr/bin/pi", "--provider", "wb-lokal", "--model", "qwen3:8b"]}
```

Die Felder: `harness` (Name aus Vertrag 2), `arbeitsordner`, `programmordner` (Liste), `zustand` (`host` und `name` des
Zustandsordners der Sitzung), `heim`, `such_socket`, `modell_socket` (Text oder `null`), `lokal_weiter` (Text oder
`null`), `cloud` (`null` oder ein Objekt mit `weiterreicher`, `weiter_programm`, `bundle`, `verteilung` = `ubuntu` oder
`arch`), `paket` (`null` oder ein Objekt mit `quelle`, `plattform`, `fassung`; nur Harness `copilot`), `umgebung` (`home`, `path`, `term`, `lang`, `tmpdir`), `geschuetzt` (Liste), `dateien` (Liste) und `argv`
(`argv[0]` ist ein absoluter, normaler Pfad). Es gibt kein Feld `system`. Den Text erzeugt „Lauf starten“ mit der
Funktion `beschreibung_schreiben` des Einschlusses; der Starter liest ihn nach denselben JSON-Regeln wie dieser Vertrag
(doppelter Schlüssel, BOM und Rest nach dem Wert sind Exit 3).

Die neuen Felder dieser Fassung:

- **`lokal_weiter`** ist das Programm `einschluss-weiter` (Text) oder `null`. Es steht nur zusammen mit
  `modell_socket`: Ist `modell_socket` `null`, ist `lokal_weiter` `null`. Der Harness läuft dann hinter dem
  Weiterleiter und spricht den Modellweg auf `127.0.0.1:11434` an (Abschnitt „Modellweg je Sitzung“, lokale Route).
- **`dateien`** ist eine Liste von Objekten `{"quelle": <text>, "ziel": <text>}`. Sie ist Pflicht und darf leer sein.
  Die Ziele sind fest je Harness: bei `pi` `<heim>/.pi/agent/models.json`, bei `copilot` `<heim>/.copilot/wb-mcp.json`;
  jedes andere Ziel ist Exit 5. Die Dateien sind **nur lesend** eingebunden. Die Quelle ist eine eigene reguläre Datei
  mit Modus 0600 und ohne Verweis, der außerhalb von Arbeitsordner und Zustandsordner endet; sonst ist es Exit 5.
- **Umgebung für `pi`:** `PI_OFFLINE=1` und `PI_TELEMETRY=0` setzt der Starter fest und nur für `pi`; für `copilot` und
  `claude` setzt er sie nicht, und kein Feld der Beschreibung ändert sie.
- **Umgebung für `copilot`:** Der Starter schreibt `--setenv` für `COPILOT_GITHUB_TOKEN` (Platzhalter), `HTTPS_PROXY`, `COPILOT_AUTO_UPDATE` und `COPILOT_MASCOT_BLINK`, und nur, wenn `cloud` gesetzt ist (`einschluss/src/profil.rs:513-522`).
- **`paket`** (Nachtrag 21.11.2026) ist `null` oder `{"quelle": <text>, "plattform": "linux-x64"|"linux-arm64", "fassung":
  "<zahl>.<zahl>.<zahl>"}`, nur für den Harness `copilot` (sonst Exit 5). Die Quelle ist der Paketordner im Cache-Ordner
  (Abschnitt „Paket der Copilot CLI“); sie liegt außerhalb von Arbeitsordner, Zustandsordner, Programmordnern und
  geschützten Ordnern, ohne Geheimnis-Segment und Anmeldespeicher (sonst Exit 5). Der Starter bindet sie **nur lesend** an
  `<heim>/.cache/copilot/pkg/<plattform>/<fassung>`, nach dem tmpfs des Heims, und prüft vorher auf dem Host: echter Pfad
  ohne Verweis, eigene Kennung, die vier Pfade als Dateien (sonst Exit 5, `quelle-unbrauchbar`).
- **Der Zustandsordner bleibt beim Start leer.** Der Starter legt in ihn nichts; was der Harness braucht, kommt über
  `dateien` (nur lesend) oder entsteht im Harness.

Prüfbarkeit: Die Fälle `aufruf-starter-*` fixieren die Beschreibung, die „Lauf starten“ dem Starter gibt (Felder, `dateien`
je Harness, `lokal_weiter` nur bei lokaler Route, Umgebung von `pi`). Das Verhalten des Starters selbst (Tabelle oben)
prüfen die Programmtests des Crates `einschluss`. Auch die Fälle `aufruf-starter-*` führt der Läufer nicht aus (`AUSGELASSEN`): Sie sind nur in der Wegwerf-VM gemessen, im Prüfbau nicht.

## Prüfbarkeit der Fälle

`pruefen.py` prüft heute: Jede Datei in `faelle/` hat ein Gegenstück in `erwartet/`; Größe und JSON-Lesen (BOM,
doppelter Schlüssel, UTF-8) stimmen mit dem erwarteten Code; das Schema nimmt jeden gültigen Fall in seiner Form an
und lehnt jeden Fall der Stufen 3 bis 12 ab. Nicht von `pruefen.py`, sondern von den Läufern der Crates `start` (`tests/faelle.rs`) und `lauf` (`tests/faelle.rs`, nur Prüfbau unter Linux) geprüft werden: Stufen 13 bis 17 (Harness-Tabelle und
Einstellungsdatei), die Reihenfolge der Stufen, `1.0` gegen `1`, die Pfadgrenze in Byte statt Zeichen und alle
`aufruf`-Fälle, darunter die Startform je Harness, der Modellweg, die Netzgrenze E5, E4 und die Ordnermarke E6 mit
Gegenproben und
Mutanten (die Mutanten-Fälle führt der Läufer noch nicht aus, OPEN 18). In einem `aufruf`-Fall fehlt `lage.modellweg`, wenn der Modellweg bereit meldet (`"stumm"`: keine Zeile binnen
10 s, `"fehlt"`: der Start scheitert), und `lage.mutant`, wenn der Aufbau richtig ist.

```sh
python3 -I rust/spec/start/bau-faelle.py            # Fälle neu erzeugen
<python mit jsonschema> -I rust/spec/start/pruefen.py  # Exit 0 bei 0 Fehlern; ohne jsonschema Exit 2 (bewusst ohne Schema: --ohne-schema)
```

## OPEN
2. **Eigentümer des Socket-Ordners.** Vertrag 4 (Stand vom 20.11.2026) sagt, der Ordner „gehört nicht der Kennung des
   Agenten“. In Fassung 1 haben Programm, Such-Prozess und Agent dieselbe Kennung; der Ordner gehört ihr zwangsläufig.
   Ihn schützt dann allein, dass er außerhalb des Einschlusses liegt und nie eingebunden wird. Der Satz in Vertrag 4
   gilt erst mit eigener Kennung. Vault sollte ihn entsprechend einschränken.
4. **Aufräumen durch den Kern-Dienst.** Die Prüfung gegen eine wiederverwendete PID (etwa über die Startzeit des
   Prozesses) legt das Projekt mit der Umsetzung fest.
5. **Pfade unter Windows** (Variante B Windows) folgen in einer späteren Fassung; Fassung 1 kennt nur absolute
   POSIX-Pfade.
6. **Grenzen.** 4096 Byte sind gesetzt. Die Zeichenregel von `modell` ist gegen 47 echte Namen gemessen (siehe
   „Startform“); die Länge 128 ist nicht ausgereizt, der längste gemessene Name hat 34 Zeichen.
7. **Startform von copilot.** Option (a) ist entschieden. Die Optionen und die Umgebung folgen der Messung zum
   Token der Copilot CLI mit 1.0.88. Ein echter Lauf mit Konto fehlt noch; er zeigt, ob die CLI die Sperre der Telemetrie
   verträgt und ob WebSocket nötig ist. `--disable-client-telemetry` nennt `copilot --help` in 1.0.113 auf dem
   Entwicklungsrechner; ob 1.0.88 die Option kennt, ist nicht gemessen.
8. **Anbieter-Definition von pi.** Form aus einer vorhandenen Konfiguration gelesen, Start damit nicht gemessen. Ob pi
   ohne Schlüssel startet und `PI_OFFLINE=1` jeden Netzversuch beim Start unterlässt, misst der Bau.
9. **Vertrag 8 „Modellweg“: überholt.** Die Betriebsart `kopf-einsetzen` ist unter Linux gebaut (`rust/crates/modellweg/src/betriebsart.rs`), das Programm übergibt Zielliste und CA (`ablauf.rs:772-779`). Weicht Vertrag 8 ab, gilt er.
10. **Gastpunkte 11434 und 8402.** `11434` (lokal) hat das Projekt vorgegeben; `8402` (Cloud) legt dieser Vertrag
    fest. Ändert das Projekt sie im Bau, ändern sich nur die Absätze und die Fälle.
11. **Proxy der Organisation.** Verlangt das Firmennetz einen Proxy, schaltet ihn der Modellweg außerhalb hinter sich;
    der Agent sieht ihn nicht.
14. **Senken auf eine nicht niedrigere Klasse.** Der Vertrag 5 verlangt in `ordnermarke.gesenkt` eine Klasse, die nicht
    `null` und ungleich der alten ist. Dieser Vertrag lehnt darum `senken` auf dieselbe oder eine höhere Klasse mit
    `ordnermarke.aufruf` (64) ab und schreibt keine Zeile. Das ist eine Entscheidung des Baus dieses Vertrags, nicht
    des Betreibers; sie ist leicht zu ändern, falls ein Anheben gewollt ist.
16. **Rückfall `128 + n` ohne Fall.** Für SIGILL, SIGFPE und SIGSEGV endet das Programm mit dem Code `128 + n`
    (`rust/crates/lauf/src/uebergabe.rs:376`, `pub fn enden`; der Ausgang `128 + s` steht auf `:383`); kein Fall prüft diesen Weg, zwei Mutanten dort (`-`, `*`) bleiben
    unentdeckt.
17. **Weiterreichen unter dem Profil.** Dass SIGHUP und SIGTERM des Programms den Starter unter seinem AppArmor-Profil
    erreichen, belegt im Lieferbau nur die Positivkontrolle der Messung vom 22.11.2026 (SIGTERM an `werkbank-lauf`,
    Ende durch Signal 15, keine Reste); einzeln nachgestellt ist es nicht (Abschnitt „Elternprozess des Starters“).

18. **Mutanten-Fälle werden noch nicht ausgeführt (Entscheid des Betreibers, 23.11.2026, B10).** `rust/crates/lauf/tests/faelle.rs` (`AUSGELASSEN`) lässt alle 26 Fälle `*-mutant-*` aus, ebenso `aufruf-starter-*`, `aufruf-marker-ordner-fremd` und `aufruf-konfig-kaputt-eigentuemer-fremd`. Entscheid: Die Mutanten-Fälle werden ausgeführt; den Läufercode bringt ein späterer Code-Antrag. Bis dahin gilt der Satz „jeder muss die Prüfung rot machen“ als noch nicht geprüft. Für `aufruf-starter-*` gilt: nur in der Wegwerf-VM gemessen, im Prüfbau nicht.
19. **Offene Zeilen der Dateitabelle (B15).** Befunde 2 bis 5 haben weder Fall noch Entscheidungssatz (Regel: „Vor M1 …“). Der Entscheid des Betreibers, 23.11.2026, nennt nur die Mutanten-Fälle; ob er Befunde 2 bis 5 deckt, ist nicht gesagt. Eine Entscheidung je Befund fehlt.
20. **Außentest-Entwürfe, nicht gebaut.** Sie brauchen neue `lage`-Schlüssel und Läufercode in `rust/crates/lauf/tests/faelle.rs`; ein Fall dafür liegt nicht in `faelle/`. Die Fälle sind aus dem Code hergeleitet, nicht gemessen. `<…>` steht für Platzhalter wie in den bestehenden Fällen, Pfade in der Form `/srv/beispiel/…`.
    - E1b (zu `aufruf-einst-unlesbar`): Die Einstellungsdatei ist ein Ordner (neu `einstellungen_art: ordner`); erwartet wie `aufruf-einst-unlesbar`.
    - E2 `aufruf-lokal-ohne-schluesseldatei`: lokale Route, Harness `pi`, neu `konfig_ohne_schluessel: ["lokal-beispiel"]`; erwartet Exit 69, `lauf.modellweg_fehlt`, Suche gestartet und beendet, Modellweg nicht gestartet.
    - E3b `aufruf-cloud-ohne-schluesseldatei-gebunden`: Cloud-Route mit `kopf-einsetzen` und `schluessel: true`, Harness `copilot`, neu `konfig_ohne_schluessel: ["cloud-beispiel"]`; erwartet Exit 69, `lauf.modellweg_fehlt`, Suche gestartet und beendet, keine Zielliste, weder `ca.key` noch `ca.crt`.
    - E3 `aufruf-einschluss-verteilung-unbekannt`: wie `aufruf-gueltig-copilot`, neu `verteilung: unbekannt`; erwartet Exit 69, `lauf.einschluss_fehlt`, Suche nicht gestartet. Variante mit neu `geerbter_deskriptor: 7` und gleicher Erwartung (als Rust-Test `an_inherited_descriptor_ends_the_start_at_step_6` schon belegt).
    - E5 `aufruf-marke-sperre-frist`: neu `sperre_gehalten_s: 6` (ein fremder Prozess hält `flock` auf `ordnermarken.lock` länger als 5 s); erwartet Exit 78, `lauf.ordner_marke_kaputt`, Suche nicht gestartet, Marken unverändert. Der Fall dauert über 5 s.
    - E6 `aufruf-marke-liste-voll`: 4096 Marken (`/srv/beispiel/m-0000` bis `m-4095`, je eigene Kennung), Start im unverwandten Ordner; erwartet Exit 78, `lauf.ordner_marke_kaputt`, Marken unverändert. Gegenstück `aufruf-marke-liste-voll-bestehender-ordner`: Start in `/srv/beispiel/m-0001`; erwartet gültiger Start, Liste unverändert (`marke.rs:217-231`). Als Datei wäre ein Fall mit 4096 Einträgen über 1 MB groß; darum ein Schlüssel, der die Liste im Läufer erzeugt.
    - E7 `aufruf-claude-cloud-ohne-ausgang`: wie `aufruf-gueltig-cloud`; erwartet `exec: claude`, `cloud` und `lokal_weiter` der Beschreibung `null`, weder `HTTPS_PROXY` noch `COPILOT_GITHUB_TOKEN` in der Umgebung, kein `ca-bundle.pem`.
    - E8 `aufruf-macos-einschluss-fehlt`: gültiger lokaler Start, neu `plattform: macos`; erwartet Exit 69, `lauf.einschluss_fehlt`, Suche nicht gestartet, Marke des Ordners danach geschrieben. Braucht einen Läufer unter macOS.
21. **Kommentare im Code, nicht Verhalten.** `rust/crates/lauf/src/fehler.rs:9` („Step 1: not exactly one argument“, seit `--meldung` unvollständig), `:45` („Step 6 and 8“; der Fehler kommt auch aus Schritt 7a und unter einem fremden System aus `halten`), `:72` („Step 8: the `exec` failed“; auch Quelle 1 in `beschreibung_bauen`), `ablauf.rs:793-795` („becomes the starter and then the harness“, seit 21.11. veraltet). Dieser Schritt ändert keinen Code.
22. **Gleiche Grenze in Vertrag `dienst`.** `rust/spec/dienst/VERTRAG.md:832` (Punkt 6a) nennt wie dieser Vertrag nur SIGKILL (Review B2 vom 23.11.). Der Text gehört zum Vertrag `dienst`; dieser Vertrag ändert ihn nicht.
23. **Vorhandene Zähldatei (B11, Rest).** Ein Lauf mit gleicher Kennung überschreibt die alte Zähldatei still (Befund 2 der Dateitabelle); niemand räumt sie. Ob eine vorhandene Zähldatei den Start scheitern lassen soll, ist nicht entschieden und bleibt offen.
24. **Anker unter bubblewrap 0.11.1 (Ubuntu 26.04).** Der Anker (`ANKER_SKRIPT`, `rust/crates/einschluss/src/profil.rs`) wartet, bis PID 1 im Einschluss in `do_wait` schläft, weil `bwrap` 0.12.0 das Todessignal von PID 1 kurz davor setzt. Auf Ubuntu 26.04 ist `bwrap` 0.11.1 geliefert. Dort sind die Prüfung der Lebensleine und ihre Fälle gemessen (25.11.2026), der Fall des Aufbaufensters (Starter stirbt vor dem Todessignal von PID 1) nur auf dem Linux-Baurechner mit 0.12.0. Ob 0.11.1 das Todessignal ebenfalls vor `do_wait` setzt, wird vor Schritt II auf Ubuntu 26.04 gemessen (Projektentscheid, 25.11.2026).
25. **Fänger für `bwrap_halten`.** Der Mutant `crates/lauf/src/uebergabe.rs: replace bwrap_halten -> Fehler with Default::default()` überlebt den Lauf ohne Merkmal (kein Test erreicht die Funktion ohne echten `bwrap`) und ist mit `pruefbau` nicht baubar. Er steht bis dahin in der Liste `erster` (`rust/mutants/lauf.liste`). Ein Fänger über eine Naht ist Pflicht vor Schritt II (Projektentscheid, 25.11.2026).

### Erledigt

1. **Name des lokalen Harness: entschieden.** `pi` (pi 1.5.2, eigener Anbieter über `--provider`/`--model`), abgestimmt
   am 20.11.2026.
3. **Exit vor dem exec gegen Exit des Harness: erledigt (22.11.2026).** Der Dienst unterscheidet beide über die
   Verbindung aus dem Abschnitt „Meldung an den Dienst“ (Form abgestimmt).
13. **Ordnermarke: geschlossen (Nachtrag 2, 20.11.; Ort und Linien 21.11.).** Die Art der Protokollzeile legt Vertrag 5 fest
    (Abschnitt 6.4, Stand vom 20.11.2026); die Marke trägt dafür ihre Kennung. Der Befehl `werkbank-ordnermarke` liegt im Paket
    als `/usr/bin/werkbank-ordnermarke` (root, 0755, dynamisch gegen das OpenSSL der Distribution gelinkt wie `werkbank-lauf`,
    nicht über den Kern-Dienst; die Positivliste des Pakets führt der Paketbau). Gegen einen Aufruf aus dem Einschluss
    schützen ihn zwei Linien, beide Pflicht: die Markendatei und die Konfiguration sind im Einschluss nicht erreichbar, und der
    Einschluss blendet das Programm durch einen Bind von `/dev/null` aus (Abschnitt „Ordnermarke“). Die Ordnermarke prüft der
    Kern-Dienst noch nicht vorab; die Abweisung kommt erst von „Lauf starten“.
15. **Pfad des Protokollordners: geschlossen (20.11.).** Im Paket ist der Protokollordner
    `/var/lib/werkbank/<nutzer>/protokoll/` (Paketvertrag, Tabelle der Nutzerdateien); „Lauf starten“ und der Befehl
    `werkbank-ordnermarke` nehmen ihn aus ihrer eigenen Konfiguration, die das Paket so setzt. Eigentümer des
    Zustandsordners: Der Zustandsordner `/var/lib/werkbank/<nutzer>/` gehört dem Nutzer der Sitzungen. Dort sind Programm, Such-Prozess und Agent derselben
    Kennung (OPEN 2); die Datei schützt allein, dass sie außerhalb des Einschlusses liegt.
12. **Marker-Optionen bei Suche und Modellweg: geschlossen (23.11.2026).** `--probelauf`, `--marker` und `--marker-zaehlung` tragen
    Vertrag 4 (`rust/spec/such-socket/README.md`, Abschnitt „Aufruf“) und Vertrag 8
    (`rust/spec/modellweg/README.md`, Optionen für jede Betriebsart) selbst; dort verlangt jede der drei die beiden anderen.
    Beide Prozesse schreiben die Zähldatei atomar: Nebendatei neben dem Ziel, neu angelegt (`O_EXCL`, 0600, kein Verweis),
    schreiben, `fsync`, `rename` über das Ziel. Eine vorhandene Zähldatei wird ersetzt; ein fremder Rest unter dem Namen der
    Nebendatei lässt das Schreiben scheitern. Sie zählen nur Treffer je Marker und schneiden keine Bytes mit. Belege:
    Suche `rust/crates/rangkern/src/sitzung/probelauf.rs:132-148` (`<datei>.<pid>.neu`, `write_all`, `sync_data`, `rename`)
    mit `loeschen.rs:110-117` (`CREATE|EXCL|NOFOLLOW`); Modellweg `rust/crates/modellweg/src/probelauf.rs:156-176`
    (`schreibe_atomar`, `sync_all`). Scheitert das Schreiben beim Start, gilt je Vertrag ein eigener Code (Vertrag 4: Exit 78;
    Vertrag 8: Exit 74, `mw.zaehlung_fehler`); dieser Vertrag nennt keinen gemeinsamen. Projektentscheid,
    23.11.2026. Die Dateitabelle („Suche und Modellweg, je Sitzung“, Befund 2) sagt dasselbe.

<!-- vom Bauvorgang angefuegt -->
> Hinweis: Dieser Text nennt Werkzeuge, die in diesem Klon nicht liegen: `brain` `pi`.
> Sie gehoeren zum Setup der Maschine, aus der dieser Baum stammt; hier fehlen
> sie mit Absicht. Wer sie braucht, schreibt sich seine eigenen.
> *Named here, not shipped here.*
