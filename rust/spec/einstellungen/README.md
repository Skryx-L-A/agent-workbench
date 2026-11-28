# Vertrag 2: Einstellungsdatei (Fassung 1 und Fassung 2)

Nachtrag 26.11.2026: Block `effort` in Fassung 2 mit den Grenzen der Admins und den Effort-Werten je Harness (Abschnitt „Fassung 2:
Block `effort`“; Entscheid des Betreibers E2).

Stand 20.11.2026, Durchstich Schritt 1; Fassung 2 als Nachtrag vom 20.11. (Abschnitt „Fassung 2: Ziel, Modell
und Netzziele“). Das Produkt schreibt die Datei. Der Kern-Dienst, das Programm „Lauf starten“,
die Wissenssuche und das Tor (`darf`) lesen sie. Es gibt nur diese eine Datei; das Tor der Suche und der Start lesen
denselben Stand (A44).

Der Vertrag steht vor dem Code. Alle Fälle sind heute rot, weil es noch keine Umsetzung gibt. Verdeckte Fälle und
Mutanten folgen mit dem Bau.

## Dateien

| Datei | Zweck |
|---|---|
| `README.md` | dieser Vertragstext |
| `einstellungen.schema.json` | Form als JSON Schema 2020-12 |
| `vorfuehrung.json` | die Vorführdatei des Durchstichs (Fassung 2) |
| `erststart.json` | das Erststart-Profil für den ersten echten Start mit Token (Fassung 2, Abschnitt „Erststart-Profil“) |
| `faelle/<name>.json` | Eingabe: `beschreibung` und die Datei als `text` (UTF-8) oder `b64` (rohe Bytes) |
| `erwartet/<name>.json` | `{"gueltig": true}` oder `{"gueltig": false, "code": "einst.<code>"}`; ein gültiger Fall zum Block `effort` trägt dazu `efforts`, eine Liste von `{"harness", "route", "modell", "erlaubt"}` mit den erlaubten Efforts nach der Regel des Blocks |
| `bau-faelle.py` | erzeugt `faelle/`, `erwartet/`, `vorfuehrung.json` und `erststart.json`. Die Fälle zum Block `effort` (Nachtrag 26.11.2026) standen bis zum Bau in `faelle-ausstehend/` und `erwartet-ausstehend/`; seit dem 27.11.2026 stehen sie in `faelle/` und `erwartet/` |
| `pruefen.py` | prüft Paarung, Größe, JSON-Lesen und Schema gegen die Fälle |

## Form

Ein JSON-Objekt, UTF-8 ohne BOM, höchstens 65536 Byte. Geschlossene Form: Ein unbekannter Schlüssel macht die Datei
ungültig, auf jeder Ebene. Ein doppelter Schlüssel im selben JSON-Objekt macht sie ungültig. Eine Ebene ohne
Regelebenen und ohne Konsole; beides folgt in einer späteren Fassung.

**Kennung** heißt überall: Text der Form `^[a-z0-9][a-z0-9._-]{0,63}$` (1 bis 64 Zeichen). Es ist dieselbe Form wie
in der Start-Struktur und in der Protokollzeile.

| Feld | Typ | Grenzen | Bedeutung |
|---|---|---|---|
| `fassung` | Ganzzahl | `1` oder `2` | Fassung des Vertrags; nur das Zahlwort `1` oder `2` ohne Bruchteil und Exponent: `1.0`, `1e0`, `2.0` sind `einst.typ_falsch`, `-0` ist die Ganzzahl 0 und damit `einst.fassung_unbekannt` |
| `klassen` | Liste von Kennungen | 1 bis 16, ohne Doppel | Datenklassen, **geordnet, die niedrigste zuerst**. Die höchste gilt als Kundendaten. Das Tor vergleicht nur Positionen und kennt keinen Klassennamen (A29). Die Texte sind dieselben wie im Notiz-Frontmatter der Suche (`datenklasse: intern`). |
| `vorgabestufe` | Kennung, optional | muss in `klassen` stehen | Klasse für eine Notiz ohne Klasse (A35). Fehlt das Feld, gilt die höchste Klasse; `null` ist kein gültiger Wert (`einst.typ_falsch`). Leser ist das Tor: `darf` setzt die Vorgabestufe selbst ein, schon im Durchstich (Zusage, 20.11.2026). In der Vorführdatei `kundendaten`. |
| `routen` | Liste von Routen | 1 bis 64, Schlüssel ohne Doppel | siehe unten |
| `kunden` | Liste von Kunden | 0 bis 256, Schlüssel ohne Doppel | siehe unten |
| `kundendaten_an_cloud` | Objekt, Pflicht in jeder Fassung | siehe unten | Regel „Kundendaten nie an Cloud“ (A50, A52). Fehlt das Objekt, ist die Datei `einst.schluessel_fehlt`; es gibt keine stille Vorgabe. |

**Route:** `schluessel` (Kennung), `art` (`cloud` oder `lokal`), `klasse` (Kennung aus `klassen`). Alle drei sind
Pflicht. `klasse` ist die **höchste Datenklasse, die die Route sehen darf**. `art` entscheidet nur über die Regel
„Kundendaten nie an Cloud“. Eine Route ohne `klasse` oder mit `klasse: null` ist ungültig; das Tor sperrt sie (A42).

**Kunde:** `schluessel` (Kennung, Pflicht), `erlaubt` und `verboten` (je optional, Liste von Routenschlüsseln, höchstens
64, ohne Doppel, jede Route muss in `routen` stehen). Fehlt `erlaubt`, bekommt der Kunde keine Route für seine Daten
(A51). `verboten` geht vor `erlaubt`. Die erlaubten Werkzeuge je Kunde aus A51 folgen in einer späteren Fassung.

**kundendaten_an_cloud:** `{"vorgabe": "nie", "freigaben": [...]}`. Beide Felder sind Pflicht; fehlt eines oder das ganze Objekt, ist die Datei `einst.schluessel_fehlt` (Fassung 1 und 2, Nachtrag 21.11.2026). `vorgabe` nimmt in
Fassung 1 nur den Text `nie` an. `freigaben` ist eine Liste von `{"route": <Kennung>, "kunde": <Kennung>}`, höchstens
1024, ohne Doppel; `route` muss eine Route der Art `cloud` sein, `kunde` ein eingetragener Kunde. Das ist der spätere
Admin-Schalter je Route und Kunde (A50). Im Durchstich ist die Liste leer. Die Form steht schon fest, damit der
Schalter keine neue Fassung braucht. Die Bedienung durch Admins (Konsole, Protokoll) gehört nicht zu diesem Vertrag.

### Basis der Fälle (Fassung 1)

```json
{
 "fassung": 1,
 "klassen": ["oeffentlich", "intern", "kundendaten"],
 "vorgabestufe": "kundendaten",
 "routen": [
  {"schluessel": "cloud-beispiel", "art": "cloud", "klasse": "intern"},
  {"schluessel": "lokal-beispiel", "art": "lokal", "klasse": "kundendaten"}
 ],
 "kunden": [{"schluessel": "k-beispiel", "erlaubt": ["lokal-beispiel"], "verboten": []}],
 "kundendaten_an_cloud": {"vorgabe": "nie", "freigaben": []}
}
```

Das ist `VORFUEHRUNG` in `bau-faelle.py`, die Basis aller abgeleiteten Fälle (Fall `gueltig-fassung-1`). Vertrag 1
bettet diese Basis ein. Damit gilt: Die Cloud-Route sieht `intern`, aber keine Kundennotiz. Die lokale Route sieht die
Notiz von `k-beispiel`, weil sie dort erlaubt ist.

### Vorführdatei (Fassung 2)

`vorfuehrung.json` trägt die Werte der Vorführung und ist ein eigener Datensatz (`VORFUEHRDATEI_MIT_WERKZEUGEN`, Fall
`gueltig-vorfuehrung-mit-werkzeugen`; der Fall `gueltig-vorfuehrung` behält den Stand ohne `harness`):

```json
{
 "fassung": 2,
 "klassen": ["public", "internal", "confidential", "customer"],
 "vorgabestufe": "customer",
 "routen": [
  {"schluessel": "copilot", "art": "cloud", "klasse": "internal", "netzziele": [
   {"host": "api.github.com", "port": 443, "zugriff": "kopf-einsetzen", "schluessel": true,
    "pfade": [{"methode": "GET", "pfad": "/copilot_internal/user"}, {"methode": "GET", "pfad": "/copilot/mcp_registry"}]},
   {"host": "api.githubcopilot.com", "port": 443, "zugriff": "kopf-einsetzen", "schluessel": true,
    "pfade": [{"methode": "GET", "pfad": "/agents/swe/internal/memory/v0/enabled"}]},
   {"host": "api.individual.githubcopilot.com", "port": 443, "zugriff": "kopf-einsetzen", "schluessel": true,
    "pfade": [{"methode": "GET", "pfad": "/models"}, {"methode": "GET", "pfad": "/responses"}, {"methode": "POST", "pfad": "/auto"},
     {"methode": "POST", "pfad": "/models/session"}, {"methode": "POST", "pfad": "/models/session/intent"},
     {"methode": "POST", "pfad": "/chat/completions"}, {"methode": "POST", "pfad": "/agents/sessions"}]},
   {"host": "telemetry.individual.githubcopilot.com", "port": 443, "zugriff": "kopf-einsetzen", "schluessel": false,
    "pfade": [{"methode": "POST", "pfad": "/telemetry"}]},
   {"host": "exp.individual.githubcopilot.com", "port": 443, "zugriff": "kopf-einsetzen", "schluessel": false,
    "pfade": [{"methode": "POST", "pfad": "/api/v1/assignments"}]},
   {"host": "api.business.githubcopilot.com", "port": 443, "zugriff": "kopf-einsetzen", "schluessel": true,
    "pfade": ["... dieselben sieben Pfade wie an api.individual.githubcopilot.com ..."]},
   {"host": "telemetry.business.githubcopilot.com", "port": 443, "zugriff": "kopf-einsetzen", "schluessel": false,
    "pfade": [{"methode": "POST", "pfad": "/telemetry"}]},
   {"host": "exp.business.githubcopilot.com", "port": 443, "zugriff": "kopf-einsetzen", "schluessel": false,
    "pfade": [{"methode": "POST", "pfad": "/api/v1/assignments"}]}]},
  {"schluessel": "lokal", "art": "lokal", "klasse": "customer",
   "ziel": "http://127.0.0.1:11434/v1", "modell": "qwen3:8b", "netzziele": []}
 ],
 "kunden": [
  {"schluessel": "k-nord", "erlaubt": ["lokal"], "verboten": []},
  {"schluessel": "k-sued", "erlaubt": [], "verboten": []}
 ],
 "kundendaten_an_cloud": {"vorgabe": "nie", "freigaben": []},
 "harness": {"copilot": {"werkzeuge_ohne_rueckfrage": ["shell", "write", "such-mcp"]}}
}
```

`k-nord` darf die lokale Route nutzen. `k-sued` ist eingetragen, hat aber keine Freigabe; `darf` ergibt für ihn
„Route für Kunde nicht erlaubt“. So zeigt die Datei die Beispielregel „lokale Route nur für Kunden mit Freigabe“
(Abgleich mit dem Drehbuch, Projektentscheid, 20.11.2026). Die Klassen sind vier frei gewählte Beispiel-Datenklassen in Kennungsform, geordnet; `customer` ist die höchste und gilt als
Kundendaten. Die Netzziele der Route `copilot` sind gemessen (Messung zum Token der Copilot CLI, Copilot CLI 1.0.88, ohne echtes
Konto): `api.github.com` nur mit der Pfadliste, sonst hätte der Agent über den
Schlüssel das ganze Konto. Dort stehen nur genaue Pfade, kein Teilbaum: `GET /copilot_internal/user` und
`GET /copilot/mcp_registry` (Entscheid der Leitung, 25.11.2026). Damit ist `GET /copilot_internal/v2/token` gesperrt; der Pfad
gäbe ein abgeleitetes Token aus, das die Leck-Sperre des Halters nicht kennt (Halter-Vertrag, Abschnitt 8, A93). Braucht die CLI
den Pfad, scheitert ihr Aufruf sichtbar (Halter `halter.pfad_verboten`, fail closed). alt: `GET /copilot_internal/*`; `api.githubcopilot.com` mit dem einen gemessenen Pfad; der Wert von `endpoints.api` mit
allen Pfaden, bis der echte Lauf die Pfade zeigt; der Wert von `endpoints.telemetry` ohne Schlüssel, nur
`POST /telemetry`; der Wert von `exp` ohne Schlüssel, nur `POST /api/v1/assignments`. `github.com` fehlt bewusst:
Die Anmeldung läuft außerhalb des Einschlusses.

Der echte Lauf (Copilot CLI 1.0.88, the test account, 20.11.2026) hat bestätigt:
- `endpoints.api` ist `https://api.individual.githubcopilot.com`, `endpoints.telemetry` ist
  `https://telemetry.individual.githubcopilot.com`. Der Plan `individual` ist damit gemessen.
- `exp.individual.githubcopilot.com` kam neu hinzu, mit `POST /api/v1/assignments` ohne `Authorization`.
- `Authorization` ging nur an `api.individual.githubcopilot.com`. Es gab keinen Cookie-Kopf und kein WebSocket; ein
  Modellaufruf kam in diesem Lauf nicht vor.
- `telemetry.individual.githubcopilot.com` lieferte NXDOMAIN, und die CLI lief weiter. Der Eintrag bleibt trotzdem
  (Projektentscheid), damit ein Netz, in dem der Name auflöst, die Telemetrie nur über die Pfadliste erreicht.
- `api.github.com` bekam in diesem Start 0 Verbindungen, weil die CLI Daten aus ihrem Cache nahm. Der Eintrag bleibt
  für einen Start ohne Cache.

Ein Lauf mit Modellaufrufen (1.0.88, Messung zum Token der CLI) hat an `api.individual.githubcopilot.com` die Pfade
`POST /auto`, `POST /models/session`, `POST /models/session/intent`, `POST /chat/completions` und
`POST /agents/sessions` gemessen. Alle Aufrufe kamen per `CONNECT` am Proxy an (Nachmessung). Die Pfade stehen
ausdrücklich in `pfade` (Projektentscheid).

**Nur genaue Pfade am Modell-Host (Nachtrag N1, Entscheid der Leitung, 25.11.2026).** An `api.individual.githubcopilot.com`
stehen nur noch genaue Pfade, kein Teilbaum. Grund: Ein Pfad, der ein Token ausgibt, steht nie in einer Regel eines gebundenen
Ziels (Halter-Vertrag, Abschnitt 4, „Abgeleitete Tokens“); ein Teilbaum `/*` kann das nicht halten. alt: `GET /*` und `POST /*`
dazu. Die Liste und die Herkunft jedes Pfads, alle gemessen mit der Copilot CLI 1.0.88 am Wert von `endpoints.api`:

| Methode und Pfad | Zweck | Herkunft |
|---|---|---|
| `GET /models` | Modellliste | gemessen (Vertrag 8, Abschnitt zu den Copilot-Zielen) |
| `GET /responses` | Modellaufruf als WebSocket (101) | gemessen im zweiten angemeldeten Start (Projektentscheid, 20.11.2026) |
| `POST /auto`, `POST /models/session`, `POST /models/session/intent` | Modellwahl und Sitzung | gemessen (Messung zum Token der CLI, per `CONNECT` am Proxy) |
| `POST /chat/completions` | Chat | gemessen (ebenda und im zweiten Start) |
| `POST /agents/sessions` | Sitzung des Agenten | gemessen (Messung zum Token der CLI) |

Nicht aufgenommen, weil die Methode nicht gemessen oder nur ein Teilbaum bekannt ist: `/mcp/readonly` (entfällt mit
`--disable-builtin-mcps`) und `/.well-known/*` (Vertrag 8). Ein öffentlicher Endpunkt kam nicht dazu: Keiner ist für diesen
Host belegt, der über die gemessenen hinausgeht. Einen Pfad außerhalb der Liste lehnt der Halter mit `halter.pfad_verboten` ab
und schreibt eine Kettenzeile mit Methode und Pfad, nie Inhalt (Halter-Vertrag, Abschnitt 9). Die Leitung entscheidet beim
ersten Start jede fehlende Regel einzeln anhand der Kette. `pruefen.py` prüft: kein Teilbaum an einem Ziel mit Schlüssel,
jeder gemessene Pfad geht durch, und Pfade außerhalb der Liste (`GET /copilot_internal/v2/token` an beiden Hosts,
`POST /v1/fremd`, der Unterpfad `GET /models/x`, die falsche Methode `POST /models` und `GET /chat/completions`) gehen nicht
durch. Gegenprobe gemessen: Mit der alten Liste meldet `pruefen.py` 7 Fehler.

### Erststart-Profil (Entscheid der Leitung, 25.11.2026)

**Die Leitung hat für den ersten echten Start mit Token entschieden: Am gebundenen Ziel `api.individual.githubcopilot.com`
gelten zuerst nur `GET /models` und `POST /chat/completions`, und das Modell ist fest gewählt, nie `auto`.** Grund: „Gemessen“
heißt bei den sieben Pfaden des Nachtrags N1 nur, dass die CLI sie ruft, nicht, dass ihre Antwort kein Token trägt. Die übrigen
fünf Pfade (`GET /responses`, `POST /auto`, `POST /models/session`, `POST /models/session/intent`, `POST /agents/sessions`) sind
beim ersten Start nicht freigeschaltet. Eine Anfrage der CLI dorthin endet beim Halter mit `halter.pfad_verboten` und steht mit
Methode und Pfad in der Kette (Halter-Vertrag, Abschnitte 4 und 9). Für jeden weiteren Pfad misst der Halter vor der Freigabe
die Form der Antwort (Halter-Vertrag, Abschnitt 4, „Antwortform im Erststart“); enthält ein Feldname `token`, `secret`, `key`,
`session` oder `credential`, bleibt der Pfad zu, bis die Leitung entscheidet.

**Wer das Profil einsetzt (Entscheid der Leitung, 25.11.2026): das Paket.** Das deb liefert `erststart.json` als
`/usr/share/werkbank/probelauf/erststart.json` neben der Vorführ-Vorlage. `postinst` baut daraus `/etc/werkbank/halter.json`
(Schritt 2c: zwei Pfade am Modell-Host und der Schalter `erststart_antwortform` des Halters für genau diese zwei) und kopiert daraus
die Arbeitskopie der Einstellungen, aus der „Lauf starten“ Route und Modell nimmt (Paketvertrag `rust/spec/paket/README.md`,
Nachtrag Erststart und Abschnitt „halter.json“). Die Vorführdatei bleibt unverändert. Der Läufer setzt nichts ein.

Das Profil ist die eigene Datei `erststart.json` neben `vorfuehrung.json`, ein Datensatz der Fassung 2 ohne neues Feld. Sie
entsteht in `bau-faelle.py` (`ERSTSTARTDATEI`) aus der Vorführdatei und ändert genau zwei Stellen an der Route `copilot` und
fügt den Block `effort` hinzu:

| Stelle | Vorführdatei | Erststart-Profil |
|---|---|---|
| `pfade` am Ziel `api.individual.githubcopilot.com` | die sieben Pfade des Nachtrags N1 | nur `GET /models` und `POST /chat/completions` |
| `modell` der Route | fehlt (der Harness wählt, im Start `auto`) | `gpt-6-luna` |
| Block `effort` | fehlt (kein Effort wählbar) | eine Grenze: `copilot`, Klasse `internal`, Modell `gpt-6-luna`, erlaubt `none` und `low` |

Herkunft des Blocks `effort` (Entscheid des Betreibers, 28.11.2026): Ohne Option sendet die Copilot CLI 1.0.88 mit
`gpt-6-luna` das Feld `reasoning_effort` mit `medium`; mit `--reasoning-effort none` fehlt das Feld (gemessen am 27.11.2026
ohne Konto, über den Halter). Der Block hat keine Vorgabe: Ein Start ohne `effort` bleibt gültig, und „Lauf starten“ setzt dann
keine Option. Ein Start mit `effort` `none` oder `low` geht durch; jeder andere Wert endet mit `start.effort_grenze` (Vertrag 1).

Herkunft des Modellnamens: Die Copilot CLI 1.0.88 wählte mit `--model auto --auto-tier efficiency` das Modell `gpt-6-luna`
und verbrauchte eine Chat-Einheit, kein Premium-Modell (gemessen am 20.11.2026, Vertrag 1, `rust/spec/start/README.md`,
Abschnitt zu `copilot`; ebenso der Läufer der Vorführung, `rust/spec/vorfuehrung/laeufer/README.md`). Mit `modell` an der
Route legt Vertrag 1 das Modell fest: Der Start muss `modell` gleich `gpt-6-luna` tragen (`start.modell_route`), und „Lauf
starten“ hängt `--model gpt-6-luna` an, nie `--model auto`. Dass der Plan des Kontos beim ersten Start dieses Modell
anbietet, ist nicht gemessen; fehlt es, scheitert der Modellaufruf sichtbar am Ziel. Alle anderen Ziele (`api.github.com` mit
den zwei genauen Pfaden, `api.githubcopilot.com`, Telemetrie, `exp`), Klassen, Kunden und der Block `harness` sind wie in der
Vorführdatei.

Fall: `gueltig-erststart` (gültig). `pruefen.py` prüft zusätzlich (`erststart_pruefen`): Schema, genau die zwei Pfade am
Modell-Host, jeder der fünf übrigen Pfade geht nicht durch, `modell` ist ein Text und nicht `auto`, der Block `effort` ist genau
der oben genannte, und außer diesen drei Stellen gleicht die Datei `vorfuehrung.json`. Die Erwartung des Falls nennt unter
`efforts` für `copilot` mit `gpt-6-luna` an der Route `copilot` genau `none` und `low`, für `auto` und für `pi` keinen Wert. Gegenprobe gemessen: Mit `modell: auto`, einem zusätzlichen `POST /auto` und
einer geänderten Kundenfreigabe meldet `pruefen.py` 4 Fehler. Dass jeder der fünf Pfade im Halter `halter.pfad_verboten` mit
Kettenzeile ergibt, prüfen die Halter-Fälle `s15-erststart-*` und der Test
`tests/dienst.rs` `erststart_sperrt_die_fuenf_pfade_mit_kettenzeile` im Crate `halter`.

Der echte Lauf hat an `api.individual.githubcopilot.com` zusätzlich `GET /responses` gemessen, ein
WebSocket-Upgrade. Der Pfad steht seit dem Nachtrag N1 als genauer Pfad in der Liste (alt: `GET /*` traf ihn), und
`pruefen.py` prüft, dass die Datei ihn zulässt. Ziel und Modell der Route `lokal` zeigen
bis zum Bau des Gateway-Wegs auf ein kleines Modell über Ollama auf dem Vorführrechner. Am Ende werden nur diese zwei Werte
getauscht: die Basis-URL eines OpenAI-kompatiblen Modell-Gateways (`https`, im Firmennetz) und sein Modellname (Form
`namensraum/modell:groesse`). Der Schlüssel des Gateways steht nie in dieser Datei (OPEN 8).

### Plan Business (Nachtrag Business, 26.11.2026)

**Die Route `copilot` trägt die Ziele der Pläne individual und Business nebeneinander.** Die Ziele des Plans individual bleiben
unverändert (eigene Tests); die Ziele des Plans Business kommen hinzu, in `vorfuehrung.json` und in `erststart.json`, und damit in die
Vorgabe der Firma (Abschnitt „Vorgabe der Firma für die Netzziele“).

Quelle ist die GitHub-Doku „Copilot allowlist reference“
(`https://docs.github.com/en/copilot/reference/copilot-allowlist-reference`, gelesen am 26.11.2026). Für den Plan Business nennt sie genau
einen Eintrag, `https://*.business.githubcopilot.com` („Allows access to authorized users via a the test account plan“), und keinen
einzelnen Host. Ebenso nennt sie für individual nur `https://*.individual.githubcopilot.com`.

**Form: einzelne Hosts unter der Unterdomänen-Form, nicht die Form selbst.** Die Form der Doku träfe jeden Namen unter
`business.githubcopilot.com`, auch einen, den erst der Agent wählt. Die einzelnen Hosts liegen alle unter ihr, also trägt die Doku sie;
sie sind die engere Form. Der Schlüssel steht nie an einer Unterdomänen-Form (Stufe 31, Fälle `schluessel-unterdomaene-business*`).

| Ziel | Schlüssel | Pfade | Herkunft |
|---|---|---|---|
| `api.business.githubcopilot.com` | ja | dieselben genauen Pfade wie am Modell-Host `api.individual.githubcopilot.com` (Vorführdatei: sieben, Erststart-Profil: zwei) | hergeleitet: der Wert von `endpoints.api` nach dem gemessenen Muster des Plans individual; mit Business nicht gemessen |
| `telemetry.business.githubcopilot.com` | nein | `POST /telemetry` | aus dem Betrieb mit dem Plan Business gemeldet, nicht selbst gemessen; Pfad wie bei individual |
| `exp.business.githubcopilot.com` | nein | `POST /api/v1/assignments` | hergeleitet aus `exp.individual.githubcopilot.com` (gemessen), mit Business nicht gemessen |

`api.github.com` (zwei genaue Pfade) und `api.githubcopilot.com` (ein Pfad) gelten für beide Pläne und bleiben, wie sie sind. Ruft die
CLI mit Business einen Host oder Pfad, der hier fehlt, scheitert der Aufruf sichtbar (Halter `halter.pfad_verboten` oder 403 des
Modellwegs, fail closed); die Leitung entscheidet die Regel dann anhand der Kette.

`pruefen.py` prüft an beiden Dateien (`plaene_pruefen`): Der Schlüssel steht genau an `api.github.com`, `api.githubcopilot.com` und den
beiden Modell-Hosts, nie an einer Unterdomänen-Form. Beide Modell-Hosts tragen dieselben Pfade. Beide Telemetrie-Ziele stehen ohne
Schlüssel mit genau `POST /telemetry`. Die gemessenen Pfade und die Pfade außerhalb der Liste prüft es an beiden Modell-Hosts; im
Erststart-Profil gelten an beiden genau die zwei Pfade.

## Fassung 2: Ziel, Modell und Netzziele (Nachtrag 20.11.2026)

**Entscheidung: neue Fassung statt Erweiterung der Fassung 1.** Fassung 1 bleibt unverändert gültig; eine Datei mit
`fassung: 1` kennt `ziel`, `modell` und `netzziele` an einer Route nicht (`einst.schluessel_unbekannt`). Fassung 2 macht
`netzziele` an jeder Route zur Pflicht und `ziel` und `modell` an jeder lokalen Route. Gründe: Die Crates `einstellungen` und `start` werden gerade
nach Fassung 1 gebaut; ihre Fälle und Codes bleiben gleich, Fassung 2 kommt als Zusatz dazu. Optionale Felder in
Fassung 1 hätten eine fehlende Liste mehrdeutig gemacht („vergessen“ oder „nichts erlaubt“). In Fassung 2 steht die
Netzgrenze immer ausdrücklich in der Datei. Eine Leserin der Fassung 1 lehnt eine Datei der Fassung 2 mit
`einst.fassung_unbekannt` ab, nie still mit falscher Netzgrenze.

**Fassung 1 im Betrieb.** Eine Route der Fassung 1 hat keine Netzziele: Der Einschluss lässt nichts hinaus. Eine
lokale Route der Fassung 1 hat kein Ziel: „Lauf starten“ startet darauf keinen Harness (`lauf.ziel_fehlt`,
Vertrag 1).

**Route in Fassung 2:** zusätzlich zu `schluessel`, `art` und `klasse`:

| Feld | Typ | Grenzen | Bedeutung |
|---|---|---|---|
| `ziel` | Text | nur bei `art: lokal`, dort Pflicht; bei `art: cloud` verboten | Basisadresse des Modells im OpenAI-Sinn, mit `/v1` am Ende; der Modellweg (Vertrag 1, `rust/spec/modellweg/`) ruft `<ziel>/chat/completions` auf. Siehe unten |
| `modell` | Text | bei `art: lokal` Pflicht, bei `art: cloud` optional; 1 bis 128 Zeichen aus `[A-Za-z0-9._:/@+-]` wie `modell` in Vertrag 1 | lokal: das einzige Modell, das der Modellweg durchlässt. Cloud: fehlt das Feld, wählt der Harness das Modell (Copilot); steht es, legt die Route das Modell fest. In beiden Fällen muss `modell` im Start gleich sein (Vertrag 1, `start.modell_route`) |
| `netzziele` | Liste von Netzzielen | Pflicht, 0 bis 64, ohne Doppel; bei `art: lokal` genau 0 | Ziele, die der Agent der Route außerhalb des Rechners erreichen darf |
| `eingeschraenktes_token` | Wahrheitswert, optional, Vorgabe `false` | `true` nur an einer Route mit mindestens einem Ziel mit `schluessel: true` (Stufe 37) | Admin-Schalter (Nachtrag Business, Abschnitt „Admin-Schalter `eingeschraenktes_token`“) |

**Netzziel:** `{"host": <Text>, "port": <Ganzzahl>, "zugriff": <Text>, "schluessel": <bool>, "pfade": [...]}`.
`host`, `port` und `zugriff` sind Pflicht; `schluessel` und `pfade` stehen genau bei `zugriff: kopf-einsetzen` und sind
dort Pflicht. Geschlossene Form. Doppelt heißt: dasselbe Paar `host`/`port` zweimal, gleich mit welcher Zugriffsart.

- `zugriff` ist die Zugriffsart nach A141 und **Pflicht**. Eine fehlende Angabe ist ein Ladefehler
  (`einst.schluessel_fehlt`); es gibt keine Vorgabe, damit nichts still offen steht.
  - `verbinden`: nur der Aufbau einer TCP-Verbindung, für TLS als Tunnel ohne Prüfung des Inhalts.
  - `http-lesen`: nur `GET` und `HEAD` über unverschlüsseltes HTTP.
  - `kopf-einsetzen`: Der Modellweg (Vertrag 1; Vertrag 8 `rust/spec/modellweg/`, Betriebsart `kopf-einsetzen`)
    beendet TLS für diesen Host mit der Wegwerf-CA der Sitzung und lässt nur die Paare aus `pfade` durch. Bei
    `schluessel: true` ersetzt er `Authorization: Bearer <Platzhalter>` durch den Schlüssel der Route. Bei
    `schluessel: false` beantwortet er jede Anfrage mit einem Kopf `Authorization` mit 403 und schreibt eine
    Protokollzeile. Nach außen prüft er TLS mit dem Truststore des Wirts.
- `schluessel` (Wahrheitswert) sagt, ob der Schlüssel der Route an dieses Ziel gebunden ist. Die Schlüsseldatei selbst
  nennt nie diese Datei; ihren Pfad nimmt das Startprogramm aus seiner Konfiguration.
- `pfade`: 1 bis 32 Einträge `{"methode": <Text>, "pfad": <Text>}`, ohne Doppel. `methode` ist `GET`, `HEAD`, `POST`,
  `PUT`, `PATCH` oder `DELETE`, groß geschrieben. `pfad` ist eine Folge von `/<teil>` mit Teilen aus
  `[A-Za-z0-9._~-]`, ohne `.` und `..`, höchstens 256 Zeichen. Er trifft genau diesen Pfad. Endet er auf `/*`,
  trifft er jeden Pfad darunter (`/copilot_internal/*` trifft `/copilot_internal/user`, nicht
  `/copilot_internal`). `/*` allein trifft jeden Pfad. Andere Platzhalter, `/` allein, `/` am Ende,
  `?` und `%` gibt es nicht.
- Der Modellweg vergleicht den Pfad der Anfrage ohne den Anfrageteil nach `?`. Eine Anfrage mit `.` oder `..` als
  Teil, mit `//` oder mit `%` im Pfad lehnt er ab, bevor er vergleicht. Methode und Pfad setzt er an jedem Ziel mit
  `kopf-einsetzen` durch. Ein Verstoß bekommt 403 und eine Protokollzeile, die ihn von einer erlaubten Anfrage
  unterscheidet (Vertrag 8, Protokollzeile).

**Zwei Regeln für eine Cloud-Route mit Schlüssel** (mindestens ein Ziel mit `schluessel: true`):

- **Kein Ziel mit `verbinden`** (`einst.verbinden_mit_schluessel`). Grund: Ein Tunnel ist ungeprüft. Durch ihn kann der
  Agent unter einem erlaubten Namen einen anderen `Host`-Kopf schicken. Ein CDN leitet die Anfrage dann an einen
  fremden Dienst weiter (Domain Fronting). So entstünde neben dem geprüften Weg ein Ausweg, den weder Pfadliste noch
  Protokoll sehen. `http-lesen` bleibt erlaubt, denn dort sieht der Modellweg jede Anfrage.
- **Kein Schlüssel an einem Ziel in Unterdomänen-Form** (`einst.schluessel_unterdomaene`). Grund: Der Schlüssel ginge
  an jeden Namen unter der Domäne, auch an einen, den erst der Agent wählt. Ein Ziel ohne Schlüssel darf die Form
  tragen.

**Form von `host` und `port`:**

- `host` ist ein Name aus mindestens zwei Teilen, getrennt durch `.`. Jeder Teil hat 1 bis 63 Zeichen aus `[a-z0-9-]`
  und beginnt und endet nicht mit `-`. Der letzte Teil beginnt mit einem Buchstaben, damit keine IPv4-Adresse als Name
  durchgeht. Höchstens 253 Zeichen. Kein Großbuchstabe, kein Schema, kein Port im Text, kein Punkt am Ende.
- Verboten sind `localhost` und jeder Name, der auf `.localhost` endet.
- **Unterdomänen-Form:** `*.` vor einem Namen aus mindestens zwei Teilen, etwa `*.githubcopilot.com`. Sie trifft jeden
  Namen, der auf `.githubcopilot.com` endet und davor mindestens einen Teil hat (`api.githubcopilot.com`,
  `api.business.githubcopilot.com`), aber nicht `githubcopilot.com` selbst. Andere Platzhalter gibt es nicht: kein `*`
  allein, kein `*` in der Mitte, kein `*.com`.
- `port` ist eine Ganzzahl von 1 bis 65535. Ein Netzziel trifft nur genau diesen Port.

Grund für „0 Netzziele bei lokaler Route“: Eine lokale Route darf Kundendaten sehen. Jedes weitere Ziel wäre ein Weg,
auf dem Kundendaten den Rechner verlassen. Der Einschluss kennt für eine lokale Sitzung kein Ziel außer dem
Modellweg aus Vertrag 1; nur dieser verbindet zum Ziel. Ein weiteres Ziel im Firmennetz braucht eine spätere Fassung (OPEN 6).

**Ziel:** ein Text, höchstens 256 Byte, ASCII, in der Form `<schema>://<host>:<port><pfad>`.

- `schema` ist der Text vor dem ersten `:`; erlaubt sind nur `http` und `https`, klein geschrieben.
- `host` ist ein Name nach den Regeln oben (auch ein einzelner Teil ist hier der Form nach erlaubt) oder eine IPv4-Adresse
  aus vier Dezimalteilen von 0 bis 255 ohne führende Null. Besteht ein Host nur aus Ziffern und Punkten, muss er eine
  solche IPv4-Adresse sein. IPv6, Nutzerteil (`nutzer@`), Anfrage (`?`) und Anker (`#`) gibt es nicht.
- `port` ist Pflicht: Dezimalziffern ohne führende Null, höchstens fünf Stellen, Wert 1 bis 65535.
- `pfad` ist eine Folge von `/<teil>` mit Teilen aus `[A-Za-z0-9._~-]`, ohne `.` und `..` als Teil. Der letzte Teil ist
  genau `v1`, ohne `/` danach. Beispiele: `/v1`, `/api/v1`.
- **Hostregel:** Eine IPv4-Adresse muss Loopback (`127.0.0.0/8`) oder privat (`10.0.0.0/8`, `172.16.0.0/12`,
  `192.168.0.0/16`) sein. `http` gilt nur für genau `127.0.0.1`; jedes andere Ziel braucht `https`, auch `localhost`
  und eine private Adresse. Das folgt Vertrag 8 (Abschnitt 6: `http` nur für `127.0.0.1` und `[::1]`; IPv6 kennt
  diese Fassung nicht). Klartext bleibt so auf dem Rechner, und ein öffentliches Ziel kann sich nicht als lokale Route
  ausgeben. Ob ein Name im Firmennetz auflöst, prüft die Datei nicht (OPEN 7). Für `https` nutzt der
Modellweg immer den Truststore des Systems; die Datei kennt kein eigenes CA-Bündel (Vorgabe der Organisation).

**Bezugsfassung für die Stufen 3 und 5.** Die Stufen 3 und 5 laufen vor der Typprüfung der Fassung. Sie nehmen die Form
der Fassung 2, wenn `fassung` eine JSON-Zahl mit dem Wert 2 ist (auch `2.0`); sonst die Form der Fassung 1. So bleibt
der Code jeder Datei der Fassung 1 gleich, und `2.0` mit den Feldern der Fassung 2 ergibt `einst.typ_falsch`.

## Fassung 2: Block `mcp` (Nachtrag 20.11.2026)

Der Block ist **optional** und steht nur in Fassung 2 (in Fassung 1 ist `mcp` ein unbekannter Schlüssel). Er gibt der
Entscheidungsstelle (Vertrag 6) die Zuordnung der Werkzeuge je MCP-Server. Die Form ist wortgleich aus Vertrag 6,
Abschnitt 2 übernommen; ändert sich dort etwas, ändert sich diese Stelle mit.

**Fehlt der Block, ist kein MCP-Server freigegeben (fail closed).** Jede bisherige Datei der Fassung 2 bleibt gültig;
keine Erwartung eines bestehenden Falls ändert sich.

```json
"mcp": {"tickets":  {"lesen": ["get"], "schreiben": ["update"], "freigabe": true, "frist_ms": 60000, "einloesefrist_ms": 30000},
        "notizen":  {"lesen": [], "schreiben": ["anlegen"], "freigabe": false},
        "such-mcp": {"lesen": ["brain_suche"], "schreiben": []}}
```

| Feld in `mcp.<server>` | Form | Bedeutung |
|---|---|---|
| Schlüssel des Servers | Kennung, höchstens 64 Server | nie ein Anzeigename; für den Such-Server von Vault ist es `such-mcp` |
| `lesen`, `schreiben` | Liste von Werkzeugnamen, Pflicht, je höchstens 256, ohne Doppel | Werkzeugname: `^[A-Za-z0-9][A-Za-z0-9._-]{0,127}$`; kein Name in beiden Listen |
| `freigabe` | Wahrheitswert, Vorgabe `true` | Freigabe je Schreibaufruf für diesen Server |
| `frist_ms` | Ganzzahl 1 bis 86 400 000, Vorgabe 60 000 | längste Frist für die Entscheidung eines Menschen |
| `einloesefrist_ms` | Ganzzahl 1 bis 86 400 000, Vorgabe 300 000 | Frist zum Einlösen nach der Erteilung. Die Vorgabe ist 5 min, ein Admin darf bis 24 h erhöhen (Entscheidung vom 23.11.2026). alt: Vorgabe 60 000 |

- Die Form ist geschlossen: ein unbekannter Schlüssel im Block oder in einem Server ist `einst.schluessel_unbekannt`.
- Es gibt keinen Sammelcode für den Block. Die Fehler fallen in die vorhandenen Stufen, in deren Reihenfolge über die
  ganze Datei: `einst.schluessel_unbekannt`, `einst.schluessel_fehlt` (`lesen` oder `schreiben` fehlt), `einst.typ_falsch`
  (auch `mcp` als Liste und eine Frist, die keine Ganzzahl ist), `einst.anzahl` (mehr als 64 Server, mehr als 256
  Werkzeuge in einer Liste), `einst.kennung_form` (Server oder Werkzeug), `einst.kennung_doppelt` (derselbe Name zweimal
  in einer Liste).
- Zwei Codes sind neu und stehen nach allen bisherigen Stufen: `einst.frist_bereich` (Stufe 32: eine Frist außerhalb von
  1 bis 86 400 000) und `einst.mcp_doppelt` (Stufe 33: ein Werkzeug steht in `lesen` und in `schreiben` zugleich). Ein
  Server, der zweimal im Objekt `mcp` steht, ist ein doppelter JSON-Schlüssel und damit `einst.json_ungueltig`
  (Stufe 2); ein Wert für `einst.mcp_doppelt` entsteht dort nicht.
- Dieser Vertrag legt nur die Form fest. Was die Stelle mit der Zuordnung tut (ein Werkzeug ohne Zuordnung gilt als
  schreibend), steht in Vertrag 6. `zweite_person` und `senken_zweite_person` gehören nicht zu Fassung 2.
- Fälle: `gueltig-mcp*`, `gueltig-ohne-mcp`, `mcp-*`.
- Typisierte Sicht (Crate `einstellungen`): `Einstellungen::mcp()` liefert die Server in der Reihenfolge der Datei,
  `Einstellungen::mcp_server(name)` den Server mit genau diesem Schlüssel. Der Typ `McpServer` hat die Methoden `name`,
  `lesen`, `schreiben`, `freigabe`, `frist_ms` und `einloesefrist_ms`; fehlende Felder haben die Vorgaben der Tabelle
  (`freigabe` wahr, `frist_ms` 60 000, `einloesefrist_ms` 300 000). Fehlt der Block (oder ist die Datei der Fassung 1), ist `mcp()` leer und
  `mcp_server` liefert nie einen Server: fail closed. Tests: `crates/einstellungen/tests/mcp.rs`.

## Fassung 2: Block `harness` (Nachtrag 20.11.2026, Projektentscheid)

Der Block ist **optional** und steht nur in Fassung 2 (in Fassung 1 ist `harness` ein unbekannter Schlüssel). Er trägt je
Harness Einstellungen für Läufe im Einschluss. Heute kennt er nur `copilot`. Die Copilot CLI fragt vor jedem Werkzeug
nach; diese Rückfrage bleibt die Vorgabe. Der Block darf sie für eine feste Liste von Werkzeugarten abschalten, und
nur für Läufe im Einschluss.

**Fehlt der Block, bleibt die Rückfrage (fail closed).** Jede bisherige Datei der Fassung 2 bleibt gültig; keine
Erwartung eines bestehenden Falls ändert sich.

```json
"harness": {"copilot": {"werkzeuge_ohne_rueckfrage": ["shell", "write", "such-mcp"]}}
```

| Feld | Form | Bedeutung |
|---|---|---|
| `harness` | Objekt, geschlossen, heute nur der Schlüssel `copilot` | Einstellungen je Harness |
| `harness.copilot.werkzeuge_ohne_rueckfrage` | Liste, Pflicht im Eintrag `copilot`, ohne Doppel | die Werkzeugarten, für die die Copilot CLI nicht zurückfragt |

- **Erlaubte Werte, genau drei:** `shell`, `write`, `such-mcp`. Der Vergleich ist wörtlich: `Shell`, ein leerer Text,
  ein Muster wie `shell(git:*)` und jede Option sind keine erlaubten Werte. Das gilt besonders für `allow-all-tools`,
  `allow-all-paths`, `allow-all-urls`, `allow-all` und `yolo`: Sie sind ein Ladefehler, weil sie der Einschluss nie
  freigibt. Ein einziger falscher Wert macht die ganze Datei ungültig.
- Die leere Liste ist gültig und heißt: Die Rückfrage bleibt für alles. Die Reihenfolge der Werte ist die der Datei;
  Vertrag 1 hängt sie in dieser Reihenfolge an den Startbefehl.
- Die Form ist geschlossen: ein unbekannter Schlüssel in `harness` (auch `claude` oder `pi`) oder in `copilot` ist
  `einst.schluessel_unbekannt`. Es gibt keinen Sammelcode: `einst.schluessel_fehlt` (`werkzeuge_ohne_rueckfrage` fehlt),
  `einst.typ_falsch` (`harness` oder `copilot` kein Objekt, die Werte keine Liste, ein Wert kein Text) und
  `einst.kennung_doppelt` (derselbe Wert zweimal) fallen in die vorhandenen Stufen. Der Code `einst.werkzeug_wert`
  ist neu und steht als Stufe 34 nach allen anderen: ein Text, der keiner der drei Werte ist.
- Vertrag 1 legt fest, was „Lauf starten“ damit tut: für jeden Wert genau `--allow-tool <wert>` am Startbefehl von
  `copilot`, sonst nichts, nie eine Option `--allow-all*`.
- Die Vorführdatei (`vorfuehrung.json`) setzt alle drei Werte.
- Fälle: `gueltig-harness*`, `gueltig-vorfuehrung-mit-werkzeugen`, `harness-*`.

## Fassung 2: Admin-Schalter `eingeschraenktes_token` (Nachtrag Business, 26.11.2026)

Das Feld ist **optional** und steht nur an einer Route der Fassung 2 (in Fassung 1 ist es ein unbekannter Schlüssel). Es bestimmt nur,
welches Token der Halter an den Zielen der Route mit `schluessel: true` einsetzt. Sonst ändert es nichts: nicht die Ziele, nicht die Pfade,
nicht den Start.

- **Fehlt das Feld oder ist es `false` (Vorgabe aus), ändert sich nichts.** Der Halter setzt das Token der Geräte-Anmeldung ein, das
  Geheimnis `anmeldung.<route>` (Halter-Vertrag, Abschnitt 1). Jede bisherige Datei der Fassung 2 bleibt gültig; keine Erwartung eines
  bestehenden Falls ändert sich.
- **`true`:** Der Halter setzt das eingeschränkte Token der Admins ein, ein fine-grained Token mit der Berechtigung „Copilot Requests“.
  Es ist das Geheimnis `anmeldung.<route>.eingeschraenkt`. `postinst` schreibt diesen Namen in `halter.json` (Paketvertrag, Abschnitt
  „halter.json“); kein Ziel nennt dann `anmeldung.<route>`.
- **Gesetzt ohne vorhandenes Token: fail closed.** Fehlt das Geheimnis, meldet der Halter `halter.kein_schluessel`, und der Modellweg
  startet nicht (`mw.halter_kein_schluessel`, Exit 78, Vertrag 8). Es gibt keinen Rückfall auf das Token der Geräte-Anmeldung, denn
  `halter.json` nennt je Ziel genau ein Geheimnis.
- Ein Wert, der kein Wahrheitswert ist, ist `einst.typ_falsch` (Stufe 6), auch `1`, `"ja"` und `null`. `true` an einer Route ohne Ziel mit
  `schluessel: true` (auch an jeder lokalen Route) ist der neue Code `einst.token_ohne_schluessel` (Stufe 37, nach allen anderen): Der
  Schalter liefe ins Leere.
- Wie das eingeschränkte Token in die Ablage des Halters kommt, legt dieser Vertrag nicht fest (OPEN 14). Die Vorführdatei und das
  Erststart-Profil setzen den Schalter nicht.
- Typisierte Sicht: `Route::eingeschraenktes_token()` liefert `true` nur, wenn die Datei `true` sagt. Tests:
  `crates/einstellungen/tests/token.rs`.
- Fälle: `gueltig-eingeschraenktes-token*`, `gueltig-erststart-eingeschraenktes-token`, `eingeschraenktes-token-*`.

## Fassung 2: Block `effort` (Nachtrag 26.11.2026, Entscheid des Betreibers E2)

Der Block trägt die **Grenzen des Efforts**, die die Admins setzen (E2, `rust/spec/seite/OBERFLAECHE.md` OF9 Punkt 4 und L10). Er ist
**optional** und steht nur in Fassung 2. In Fassung 1 ist `effort` ein unbekannter Schlüssel. Den Effort einer Sitzung wählt der
Nutzer im Start (Vertrag 1, Feld `effort`); seine Vorgabe ist eine Einstellung des Nutzers (`rust/spec/regelebenen/README.md`,
Abschnitt 15.1, `sitzung.effort`) und steht nicht in dieser Datei.

```json
"effort": {"grenzen": [{"harness": "copilot", "erlaubt": ["low", "medium", "high"]},
                       {"harness": "copilot", "klasse": "intern", "modell": "gpt-6-luna", "erlaubt": ["low", "medium"]},
                       {"harness": "pi", "klasse": "kundendaten", "erlaubt": ["off", "low"]}]}
```

| Feld | Form | Bedeutung |
|---|---|---|
| `effort` | Objekt, geschlossen, nur der Schlüssel `grenzen` | Grenzen der Admins |
| `effort.grenzen` | Liste, Pflicht im Block, 0 bis 64 Einträge | je Eintrag eine Grenze |
| Eintrag `harness` | Kennung, Pflicht, aus der Harness-Tabelle von Vertrag 1 (`claude`, `copilot`, `pi`) | für welchen Harness |
| Eintrag `klasse` | Kennung, optional, aus `klassen` | gilt nur für Routen dieser Klasse; fehlt sie, für jede Klasse |
| Eintrag `modell` | Text nach der Regel von `modell` (Vertrag 1), optional | gilt nur für dieses Modell im Start; fehlt es, für jedes |
| Eintrag `erlaubt` | Liste, Pflicht, 1 bis 7 Werte ohne Doppel | die erlaubten Werte; jeder muss ein Wert des Harness sein |

**Werte je Harness** (gemessen am 26.11.2026 auf dem Entwicklungsrechner mit `<programm> --help`; die Versionen sind neuer als die
in Vertrag 1 genannten, siehe OPEN 10):

| Harness | Option im Startbefehl | Werte in dieser Reihenfolge | Quelle |
|---|---|---|---|
| `claude` | `--effort <wert>` | `low`, `medium`, `high`, `xhigh`, `max` | `claude --help`, Version 2.1.332 |
| `copilot` | `--reasoning-effort <wert>` | `none`, `minimal`, `low`, `medium`, `high`, `xhigh`, `max` | `copilot --help`, Version 1.0.116; „supported levels depend on the model“ |
| `pi` | `--thinking <wert>` | `off`, `minimal`, `low`, `medium`, `high`, `xhigh`, `max` | `pi --help`, Version 1.6.1 |
| `opencode` | kein argv-Paar; Eintrag `options.reasoningEffort` am Modell der Konfigurationsdatei (Vertrag 1) | `none`, `minimal`, `low`, `medium`, `high`, `xhigh`, `max` | gemessen am 27.11.2026 mit Version 1.19.5: jeder Wert erscheint als `reasoning_effort` im Anfragekörper (Entscheid des Betreibers, 27.11.2026) |

Die Werte werden wörtlich verglichen. Es gibt keine gemeinsame Liste: `none` gilt nur für `copilot` und `opencode`, `off` nur für `pi`.

**Erlaubte Efforts eines Starts.** Für Harness `h`, Route `r` mit Klasse `k` und Modell `m` des Starts gilt:
- Passend ist jeder Eintrag mit `harness` gleich `h`, ohne `klasse` oder mit `klasse` gleich `k`, ohne `modell` oder mit `modell`
  gleich `m`. Bei `modell` gleich `auto` passt nur ein Eintrag ohne `modell` oder mit `modell` `auto`.
- Gibt es keinen passenden Eintrag, ist **kein Effort wählbar**. Fehlt der Block, gilt das für jeden Harness (fail closed). Der Start
  bleibt ohne Effort gültig; der Harness nimmt dann seine eigene Vorgabe (Vertrag 1).
- Sonst sind die Werte des Harness erlaubt, die in **jedem** passenden Eintrag stehen, in der Reihenfolge der Tabelle oben.
  Jeder weitere Eintrag kann die Wahl also nur verengen.

Grund für fail closed: Ohne Entscheid der Admins wählt niemand einen teuren Effort. Jede bisherige Datei bleibt gültig, und jeder
bisherige Start (ohne `effort`) ebenso.

- Codes in den vorhandenen Stufen: `einst.schluessel_unbekannt` (3), `einst.schluessel_fehlt` (5, `grenzen`, `harness` oder
  `erlaubt` fehlt), `einst.typ_falsch` (6), `einst.anzahl` (8, `erlaubt` leer oder mit mehr als 7 Werten, mehr als 64 Einträge),
  `einst.kennung_form` (9, `harness` oder `klasse`), `einst.kennung_doppelt` (10, ein Wert zweimal, oder zwei Einträge mit demselben
  `harness`, derselben `klasse` und demselben `modell`), `einst.klasse_unbekannt` (12) und `einst.modell_form` (23).
- Neu stehen nach allen anderen Stufen: 35 `einst.effort_harness` (ein `harness` steht nicht in der Harness-Tabelle) und 36
  `einst.effort_wert` (ein Wert in `erlaubt` ist kein Wert dieses Harness).
- Wer den Block schreibt: Die Admins setzen `effort.grenzen` als Einstellung im Lauf auf der Ebene der Firma
  (`rust/spec/regelebenen/README.md`, Abschnitt 15). Der Lader der Ebenen übernimmt den Wert unverändert in diese Datei. Bis er gebaut
  ist, trägt die Datei der Firma den Block selbst (OPEN 11).
- Typisierte Sicht (Crate `einstellungen`, gebaut 27.11.2026): `Einstellungen::efforts(harness, route, modell) -> Vec<&str>` liefert die
  erlaubten Werte nach der Regel oben; `route` ist der Schlüssel der Route, eine unbekannte Route oder ein unbekannter Harness ergibt eine
  leere Liste. `Einstellungen::effort_grenzen()` gibt die Grenzen in der Reihenfolge der Datei, `effort_werte(harness)` die Werte je
  Harness (Tabelle oben; `None` außerhalb der Harness-Tabelle).
- Fälle: `faelle/` (`gueltig-effort*`, `gueltig-ohne-effort`, `effort-*`); die gültigen Fälle nennen in `erwartet/` unter `efforts` die
  erlaubten Werte für einige Starts, und der Falltest des Crates prüft sie.


## Vorgabe der Firma für die Netzziele (Nachtrag, 26.11.2026)

**Die Routentabelle mit ihren Netzzielen kommt nur aus der Vorgabe der Firma; die Datei des Nutzers darf Routen, Ziele und Pfade
nur streichen.** Grund: A45, die Routentabelle ist Vorgabe der Firma, und ein Nutzer ändert sie nicht. Die Einstellungsdatei, die
„Lauf starten“ liest, gehört dem Nutzer (Arbeitskopie, 0600). Bisher folgten aus ihr die Zielliste des Modellwegs und die Namen der
Wegwerf-CA; nur die Ziele mit Schlüssel waren über die root-eigene `halter.json` gebunden. Ein Nutzer konnte so ein Ziel ohne Schlüssel
ergänzen, eine Route in eine höhere Klasse heben oder das Ziel einer lokalen Route ändern.

- **Vorgabe:** eine Einstellungsdatei dieses Vertrags (Fassung 2) unter `/etc/werkbank/vorgabe.json`, Eigentümer root, Modus 0644, im
  Ordner `/etc/werkbank` (root, 0755). Der Pfad steht an einer Stelle: `VORGABE_PFAD` in `rust/crates/lauf/src/ziele/vorgabe.rs`. Das
  Paket legt sie als Kopie des Erststart-Profils an (Paketvertrag, Abschnitt „vorgabe.json“). Die Ziele beider Pläne, auch des Plans
  Business, stehen damit in der Vorgabe.
- **Schritt 4b von „Lauf starten“ (Vertrag 1), bei jedem Start, auch einem lokalen:** Jede Route in der Datei des Nutzers wird über
  ihren `schluessel` in der Vorgabe gesucht.
  - Fehlt sie dort, endet der Start mit `lauf.vorgabe_route_fremd` (`Route nicht in der Vorgabe der Firma`). Das gilt auch für
    dieselbe Route unter einem neuen Schlüssel.
  - `art`, `klasse`, die Stelle der Klasse in `klassen` und `ziel` müssen gleich der Vorgabe sein, auch bei einer lokalen Route. Sonst
    endet der Start mit `lauf.vorgabe_route_abweichend` (`Route weicht von der Vorgabe der Firma ab`). Die Stelle zählt mit, weil ein
    Umordnen von `klassen` eine Route sonst höher stellt, obwohl ihr Klassenname gleich bleibt. Streicht ein Nutzer eine ungenutzte
    Klasse vor einer genutzten, ist das darum auch eine Ablehnung (fail closed).
  - `modell`: Legt die Vorgabe ein Modell fest, muss die Route genau dieses tragen; ein anderes oder keines ist
    `lauf.vorgabe_route_abweichend`. Legt die Vorgabe keines fest, wählt der Nutzer frei. Eine lokale Route hat nach Stufe 18 immer ein
    Modell; dort ist es also immer fest.
  - `eingeschraenktes_token` muss gleich der Vorgabe sein, in beiden Richtungen; ein fehlendes Feld gilt als `false`. Sonst endet der
    Start mit `lauf.vorgabe_schalter` (`Schalter eingeschraenktes_token weicht von der Vorgabe der Firma ab`). Grund: Der Halter liest
    den Schalter aus der root-eigenen `halter.json`, nie aus dieser Datei; ein abweichender Wert würde sonst still übergangen.
  - Jedes Netzziel der Route steht in der Vorgabe mit gleichem `host` (auch gleicher Unterdomänen-Form) und `port`. Es hat dieselbe
    Zugriffsart und `schluessel: true` nur, wo die Vorgabe den Schlüssel bindet. Jedes Paar aus Methode und Pfad steht wörtlich an
    diesem Ziel der Vorgabe. Ein Teilbaum `/x/*` deckt dabei keinen Pfad, den die Vorgabe nicht wörtlich nennt. Sonst endet der Start
    mit `lauf.vorgabe_erweitert` (`Netzziel nicht in der Vorgabe der Firma`).
- **Streichen gilt:** eine Route, ein Ziel, einen Pfad oder die Schlüsselbindung weglassen. Zielliste und Namen der CA entstehen dann
  ohne das Gestrichene, wie bisher aus der Route des Nutzers.
- **Ergänzen oder Ändern ist eine Ablehnung, nie stilles Ignorieren.** Alle vier Codes enden mit Exit 78.
- **Fail closed:** Fehlt die Vorgabe oder ein Ordner darüber, endet der Start mit `lauf.vorgabe_fehlt` (`Vorgabe der Firma fehlt`), auch
  ein lokaler. Ist sie nicht root-eigen, für Gruppe oder andere schreibbar, ein Verweis oder keine reguläre Datei, liegt sie in einem
  Ordner, der nicht root gehört oder für Gruppe oder andere schreibbar ist, ist sie größer als 65536 Byte oder nicht lesbar, endet er
  mit `lauf.vorgabe_unsicher`. Verletzt sie diesen Vertrag, endet er mit `lauf.vorgabe_kaputt`. Alle Exit 78. Es gibt keinen Rückfall
  auf die Datei des Nutzers.
- **Prüfung über Deskriptoren:** Von `/` an öffnet „Lauf starten“ jeden Ordner mit `O_NOFOLLOW` und `O_DIRECTORY` relativ zum vorigen und
  prüft ihn mit `fstat`. Die Datei öffnet es mit `O_NOFOLLOW` und prüft sie mit `fstat`, bevor es ein Byte liest. Ein Ordner über dem
  letzten darf schreibbar sein, wenn er das Sticky-Bit trägt (wie `/tmp`); der Ordner der Datei nie.
- **Nicht verglichen** werden heute `klassen` über die Stellen der Routen hinaus, `vorgabestufe`, `kunden`, `kundendaten_an_cloud`,
  `harness`, `mcp` und der Block `effort`. Keines davon öffnet ein Netzziel oder ändert eine Route. Der Block `effort` trägt aber
  Grenzen der Admins; liest das Crate ihn, muss er ebenfalls aus der Vorgabe kommen (OPEN 15).
- Der Prüfbau (Feature `pruefbau`) liest `vorgabe.json` neben dem Programm und nimmt neben root auch den eigenen Nutzer als Eigentümer;
  der ausgelieferte Bau nimmt nur root.
- Tests: `crates/lauf/src/ziele/vorgabe/tests.rs` (Regeln, Lesen, Routentabelle, Schnittmenge), `crates/lauf/src/ablauf/tests.rs`
  (Schritt 4b).


## Fehlercodes und Prüfreihenfolge

Die Prüfung meldet genau einen Code: den der ersten verletzten Stufe in dieser Reihenfolge. Innerhalb einer Stufe ist
es gleich, welche Stelle verletzt ist, denn der Code ist derselbe. Die Codes sind feste Texte. Der Kern-Dienst reicht
sie unverändert an die Ansicht durch.

| Stufe | Code | Verletzung | Grund (fest, ASCII) |
|---|---|---|---|
| 1 | `einst.zu_gross` | mehr als 65536 Byte | `Einstellungsdatei zu gross` |
| 2 | `einst.json_ungueltig` | kein UTF-8, BOM, Syntaxfehler, doppelter Schlüssel in einem Objekt, oberste Ebene kein Objekt, Text nach dem Objekt, mehr als 127 Verschachtelungsebenen | `kein gueltiges JSON` |
| 3 | `einst.schluessel_unbekannt` | ein Schlüssel außerhalb der Form der Bezugsfassung, auf jeder Ebene (in Fassung 1 auch `ziel`, `modell` und `netzziele` einer Route) | `unbekannter Schluessel` |
| 4 | `einst.route_ohne_klasse` | eine Route ohne `klasse` oder mit `klasse: null` | `Route ohne Klasse` |
| 5 | `einst.schluessel_fehlt` | ein Pflichtschlüssel der Bezugsfassung fehlt (in beiden Fassungen `fassung`, `klassen`, `routen`, `kunden`, `kundendaten_an_cloud`, dazu `vorgabe` und `freigaben` darin, `route` und `kunde` einer Freigabe, `schluessel` und `art` einer Route, `schluessel` eines Kunden; in Fassung 2 `netzziele` einer Route, `host`, `port` oder `zugriff` eines Netzziels, `methode` oder `pfad` eines Pfadeintrags; `ziel` und `modell` der lokalen Route siehe Stufe 18) | `Pflichtfeld fehlt` |
| 6 | `einst.typ_falsch` | falscher JSON-Typ (etwa `fassung` als Text oder `1.0`, `kundendaten_an_cloud` als Text, `ziel` oder `modell` kein Text, `schluessel` kein Wahrheitswert, `port` kein Zahlwort einer Ganzzahl) | `falscher Typ` |
| 7 | `einst.fassung_unbekannt` | `fassung` ist eine Ganzzahl, aber weder `1` noch `2` | `Fassung unbekannt` |
| 8 | `einst.anzahl` | eine Liste außerhalb ihrer Grenzen (etwa keine Route, keine Klasse, mehr als 64 Netzziele, `pfade` leer oder mit mehr als 32 Einträgen) | `Anzahl ausserhalb der Grenzen` |
| 9 | `einst.kennung_form` | eine Kennung verletzt die Form | `Kennung hat falsche Form` |
| 10 | `einst.kennung_doppelt` | doppelt in `klassen`, in Routen- oder Kundenschlüsseln, in `erlaubt`, `verboten` oder `freigaben` (dort dasselbe Paar `route`/`kunde` zweimal) oder in `netzziele` einer Route (dasselbe Paar `host`/`port` zweimal) oder in `pfade` (dasselbe Paar `methode`/`pfad` zweimal). Routen- und Kundenschlüssel sind getrennte Namensräume: eine Route und ein Kunde dürfen denselben Schlüssel tragen | `Kennung doppelt` |
| 11 | `einst.art_unbekannt` | `art` ist weder `cloud` noch `lokal` | `Routenart unbekannt` |
| 12 | `einst.klasse_unbekannt` | `klasse` einer Route steht nicht in `klassen` | `Klasse unbekannt` |
| 13 | `einst.vorgabestufe_unbekannt` | `vorgabestufe` steht nicht in `klassen` | `Vorgabestufe unbekannt` |
| 14 | `einst.route_unbekannt` | `erlaubt`, `verboten` oder eine Freigabe nennt eine Route, die nicht in `routen` steht | `Route unbekannt` |
| 15 | `einst.kunde_unbekannt` | eine Freigabe nennt einen Kunden, der nicht in `kunden` steht | `Kunde unbekannt` |
| 16 | `einst.vorgabe_wert` | `kundendaten_an_cloud.vorgabe` ist ein Text, aber nicht `nie` | `Vorgabe nicht nie` |
| 17 | `einst.freigabe_route_lokal` | eine Freigabe nennt eine Route der Art `lokal` | `Freigabe fuer lokale Route` |
| 18 | `einst.ziel_art` | Fassung 2: eine lokale Route ohne `ziel` oder ohne `modell`, oder eine Cloud-Route mit `ziel` | `Ziel oder Modell passt nicht zur Routenart` |
| 19 | `einst.ziel_schema` | das Schema des Ziels ist nicht `http` oder `https` (auch `HTTP`, auch ohne `://`) | `Ziel mit falschem Schema` |
| 20 | `einst.ziel_form` | das Ziel verletzt die Form (Host, Port fehlt oder mit führender Null, Pfad, `/v1` fehlt am Ende, Nutzerteil, Anfrage, Länge) | `Ziel hat falsche Form` |
| 21 | `einst.ziel_port` | der Port des Ziels ist 0 oder größer als 65535 | `Port des Ziels ausserhalb 1 bis 65535` |
| 22 | `einst.ziel_host` | die Hostregel ist verletzt (öffentliche IPv4, `0.0.0.0`, `http` zu einem anderen Host als `127.0.0.1`) | `Ziel nicht lokal` |
| 23 | `einst.modell_form` | `modell` einer Route verletzt Länge oder Zeichenvorrat | `Modell hat falsche Form` |
| 24 | `einst.netzziel_lokal` | eine lokale Route hat mindestens ein Netzziel | `Netzziel an lokaler Route` |
| 25 | `einst.netzziel_form` | ein `host` eines Netzziels verletzt die Form (IP, `localhost`, ein Teil, Platzhalter außer `*.`) | `Netzziel hat falsche Form` |
| 26 | `einst.netzziel_port` | ein `port` eines Netzziels liegt außerhalb 1 bis 65535 | `Port des Netzziels ausserhalb 1 bis 65535` |
| 27 | `einst.zugriff_unbekannt` | `zugriff` eines Netzziels ist nicht `verbinden`, `http-lesen` oder `kopf-einsetzen` | `Zugriffsart unbekannt` |
| 28 | `einst.zugriff_felder` | `schluessel` oder `pfade` fehlt bei `kopf-einsetzen`, oder eines von beiden steht bei einer anderen Zugriffsart | `Felder passen nicht zur Zugriffsart` |
| 29 | `einst.pfad_form` | `methode` oder `pfad` eines Pfadeintrags verletzt die Form | `Pfad hat falsche Form` |
| 30 | `einst.verbinden_mit_schluessel` | eine Route mit gebundenem Schlüssel hat ein Ziel mit `verbinden` | `Tunnel neben Schluessel` |
| 31 | `einst.schluessel_unterdomaene` | ein Ziel mit `schluessel: true` steht in Unterdomänen-Form | `Schluessel an Unterdomaene` |
| 32 | `einst.frist_bereich` | Fassung 2: `frist_ms` oder `einloesefrist_ms` eines Servers im Block `mcp` ist eine Ganzzahl außerhalb von 1 bis 86400000 | `Frist ausserhalb des Bereichs` |
| 33 | `einst.mcp_doppelt` | Fassung 2: ein Werkzeugname steht in `lesen` und in `schreiben` desselben Servers | `Werkzeug in lesen und schreiben` |
| 34 | `einst.werkzeug_wert` | Fassung 2: ein Wert in `harness.copilot.werkzeuge_ohne_rueckfrage` ist ein Text, aber nicht `shell`, `write` oder `such-mcp` | `Werkzeugwert nicht erlaubt` |
| 35 | `einst.effort_harness` | Fassung 2 (Nachtrag 26.11.2026): `harness` einer Grenze in `effort.grenzen` steht nicht in der Harness-Tabelle von Vertrag 1 | `Harness der Effort-Grenze unbekannt` |
| 36 | `einst.effort_wert` | Fassung 2 (Nachtrag 26.11.2026): ein Wert in `erlaubt` einer Grenze ist kein Effort-Wert dieses Harness | `Effort-Wert nicht erlaubt` |
| 37 | `einst.token_ohne_schluessel` | Fassung 2: `eingeschraenktes_token: true` an einer Route ohne Ziel mit `schluessel: true` | `eingeschraenktes Token ohne Ziel mit Schluessel` |

Dazu kommt ein Code, den nur das Programm „Lauf starten“ meldet, weil er den Dateizugriff betrifft:
`lauf.einst_unlesbar` (Datei fehlt oder ist nicht lesbar). Siehe Vertrag 1, Abschnitt „Aufruf“.

## Schnittstelle der Prüfstelle

Eine Bibliothek (künftig Crate `einstellungen` unter `rust/crates/einstellungen`) liest und prüft die Datei. Dieser
Vertrag beschreibt nur die Schnittstelle; gebaut wird sie in einer späteren Fassung.

- `lesen(bytes) -> Result<Einstellungen, Fehler>`: führt die Stufen 1 bis 34 und 37 aus (mit dem Block `effort` 1 bis 37). Eine Leserin, die nur Fassung 1
  kennt, führt die Stufen 1 bis 17 aus und meldet für `fassung: 2` den Code `einst.fassung_unbekannt`.
- `Fehler::code() -> &'static str` gibt den Code aus der Tabelle; `Fehler::grund() -> &'static str` den festen
  Grund aus der Tabelle (ASCII, ohne Pfad und ohne Inhalt der Datei).
- `Einstellungen` gibt Routen (Schlüssel, Art, Position der Klasse, Ziel als Schema, Host, Port und Pfad, Modell, Netzziele mit Zugriffsart, Schlüsselbindung und Pfaden),
  Kunden, Vorgabeklasse und Freigaben getypt heraus. Bei Fassung 1 sind die Netzziele leer; Ziel und Modell fehlen.

Aufrufer: der Kern-Dienst (vor dem Start einer Sitzung), das Programm „Lauf starten“, die Wissenssuche und das Tor.

## Verschachtelungstiefe (Nachtrag 20.11.2026)

Eine Einstellungsdatei mit mehr als 127 Verschachtelungsebenen ist `einst.json_ungueltig` (Stufe 2, Stackschutz, fail
closed; die Grenze ist die von `serde_json` in der Voreinstellung, die höchstens 127 Ebenen liest). Gezählt wird, wie viele Objekte und Listen zugleich offen sind; das Objekt der obersten Ebene ist Ebene 1.
Die tiefste Form einer gültigen Datei hat 7 Ebenen (Objekt, `routen`, Route, `netzziele`, Netzziel, `pfade`,
Pfadeintrag); die Grenze liegt weit darüber. Klammern in Texten zählen nicht. Die Prüfung läuft ohne Rekursion vor dem Lesen des Wertes. Die
Form kennt keinen tief verschachtelten Wert; Fälle an der Grenze stehen darum unter einem unbekannten Schlüssel: Tiefe
127 scheitert erst an Stufe 3 (`einst.schluessel_unbekannt`), Tiefe 128 ist `einst.json_ungueltig`. Fälle:
`json-tiefe-*` (Listen und Objekte, 20000 Ebenen, Stufe 2 vor Stufe 3).

## Prüfbarkeit der Fälle

`pruefen.py` prüft heute: Jede Datei in `faelle/` hat ein Gegenstück in `erwartet/`; Größe und JSON-Lesen (BOM,
doppelter Schlüssel, UTF-8) stimmen mit dem erwarteten Code; das Schema nimmt jeden gültigen Fall an und lehnt jeden
Fall der Stufen 3 bis 9, 11, 16, 18 bis 20, 23 bis 32, 34 und 37 ab. Erst mit dem Bau prüfbar sind: Stufe 33 (Querverweis zwischen `lesen` und `schreiben`) und das Doppel in `werkzeuge_ohne_rueckfrage` (Stufe 10, wie bei den Listen von `mcp`), Stufe 10 für Routen und Kunden (JSON Schema
vergleicht dort ganze Objekte), Stufen 12 bis 15 und 17 (Querverweise), Stufen 21 und 22 (Portwert und Hostregel des
Ziels), IPv4-Teile über 255 (Fall `ziel-form-ipv4-256`), die Reihenfolge der Stufen und `1.0` gegen `1` (JSON Schema
hält beide für gleich).

```sh
python3 -I rust/spec/einstellungen/bau-faelle.py            # Fälle neu erzeugen
<python mit jsonschema> -I rust/spec/einstellungen/pruefen.py  # Exit 0 bei 0 Fehlern; ohne jsonschema Exit 2 (bewusst ohne Schema: --ohne-schema)
```

## Entschieden

- Die Vorgabestufe wendet das Tor an (`darf`); die Suche reicht die rohe Klasse oder `null` durch (Vorschlag,
  Zusage, 20.11.2026).

## OPEN

1. **Bedeutung der Klasse** ist abgestimmt (höchste erlaubte Klasse, geordnete Liste, Art eigenes Feld).
   Die endgültige Prüfung erfolgt gegen die Tabellenfälle von Vertrag 3 unter `rust/spec/datentor/`.
2. **Kunde der Sitzung gegen Kunde der Einheit.** `darf` nutzt den Kunden der Einheit. Ein Kundenraum je Sitzung (A53)
   folgt später; dieser Vertrag braucht dafür kein Feld.
3. **Ort der Datei.** Das Programm „Lauf starten“ und die Suche nehmen den Pfad aus ihrer eigenen Konfiguration, nie aus
   der Start-Struktur und nie vom Agenten. Der Ort je Betriebssystem steht noch nicht fest.
4. **Regelebenen, Konsole, Zonen (A26 bis A30, A45, A127)** folgen in einer späteren Fassung mit neuer `fassung`. Die
   Zonen stehen bis dahin im Teilvertrag `rust/spec/datentor/zonen/VERTRAG.md`, die Regelebenen in
   `rust/spec/regelebenen/README.md`; `vorgabestufe` ist schon Feld dieser Fassung (Tabelle der Felder).
5. **Copilot-Ziele.** Entschieden (Nachtrag N1, 25.11.2026): Am Modell-Host stehen nur die gemessenen genauen Pfade;
   jeder andere Pfad endet beim Halter mit `halter.pfad_verboten` und einer Kettenzeile. Offen: ob die CLI im Betrieb
   weitere Pfade braucht; die Leitung entscheidet jede fehlende Regel einzeln anhand der Kette. alt: „`GET /*` und `POST /*`
   bleiben, bis jemand entscheidet“. Für einen anderen Plan ist der Plan im Namen nicht gemessen. Für den ersten echten
   Start gilt das Erststart-Profil (`erststart.json`, Entscheid der Leitung, 25.11.2026): nur `GET /models` und
   `POST /chat/completions`, Modell `gpt-6-luna`. Offen: welche der fünf übrigen Pfade nach der Messung ihrer Antwortform
   freigeschaltet werden; das entscheidet die Leitung je Pfad. Entschieden (Leitung, 25.11.2026): Das Paket setzt
   `erststart.json` ein (Abschnitt „Erststart-Profil“). alt: „Offen ist auch, wer `erststart.json` beim ersten Start statt
   `vorfuehrung.json` einsetzt (Paket oder Läufer); dieser Vertrag legt nur die Datei fest.“
6. **Weitere Ziele einer lokalen Route.** Fassung 2 erlaubt einer lokalen Route nur ihr Ziel. Braucht die Organisation
   mehr (etwa einen Paketspiegel), folgt das mit einer neuen Fassung.
7. **Firmennetz-Host.** Die Hostregel prüft nur die Form: Ein Name mit `https` gilt als Firmennetz-Host. Ob er
   wirklich im Firmennetz auflöst, prüft Fassung 2 nicht. Vorschlag: Der Modellweg verbindet nur, wenn der Name auf
   eine private Adresse auflöst. Das entscheidet die Organisation (Art der Erreichbarkeit).
8. **Schlüssel des Modell-Gateways.** Ein Modell-Gateway verlangt oft je Rechner einen virtuellen Schlüssel
   (`Authorization: Bearer`). Fassung 2 hat bewusst kein Feld dafür: Der Schlüssel steht nie in dieser Datei und nie im
   Einschluss. Der Modellweg liest ihn aus einer Datei 0600 außerhalb; den Pfad nimmt das Startprogramm aus seiner
   eigenen Konfiguration (Vertrag 1, `rust/spec/modellweg/`).
9. **Zugriffsart ohne Angabe: entschieden.** `zugriff` ist Pflicht (A141, fail closed; Projektentscheid, 20.11.2026).
   Die frühere Vorgabe `verbinden` entfällt.
10. **Neuere Versionen der Harnesses (Nachtrag 26.11.2026).** Die Werte des Blocks `effort` sind mit claude 2.1.332, copilot 1.0.116 und
    pi 1.6.1 gemessen. Vertrag 1 nennt copilot 1.0.88 und pi 1.5.2. Ob die älteren Versionen dieselben Optionen kennen, ist nicht
    gemessen. Empfehlung: das Paket legt die Versionen fest, und der Bau misst die Optionen dort neu.
11. **Lader der Ebenen und Block `effort`.** Wer den Wert aus `regelebenen` in diese Datei schreibt, legt der Bau des Laders fest.
    Empfehlung: der Lader, mit demselben Lesen wie heute; der Dienst liest weiter nur `lauf.einstellungen`.
12. **Optionen des Pilots (Nachtrag 27.11.2026, Entscheid des Betreibers).** Der Katalog von `regelebenen` (Abschnitt 15.1.1) nennt
    sechs Optionen mit Vorgabe „aus“. In diese Datei wirken heute nur zwei, beide hergeleitet: `optionen.copilot_ohne_rueckfrage` auf
    `harness.copilot.werkzeuge_ohne_rueckfrage` und `optionen.copilot_telemetrie` auf den Eintrag mit Host
    `telemetry.individual.githubcopilot.com` in `routen[].netzziele`. Für die übrigen vier hat Vertrag 2 kein Feld; es folgt mit dem
    Bau (`regelebenen` OPEN 17, 18, 20 und 21). Empfehlung: dieselbe Lösung wie OPEN 11.
13. **Feld `leseklassen` (Nachtrag 27.11.2026, Projektentscheid, 27.11.2026).** Die Admins setzen die Leseklasse je Person oder Gruppe
    als Einstellung im Lauf (`rust/spec/regelebenen/README.md`, Abschnitt 15.11). Diese Datei hat das Feld noch nicht. Vorschlag für
    eine neue Fassung: ein optionales Feld `leseklassen` in der Form von `rust/spec/datentor/person/VERTRAG.md`, Abschnitt 3, mit
    höchstens 4096 Einträgen. Bei `ebene: nutzer` steht in `wer` das Pseudonym, nie ein Konto; die Zuordnung macht der Dienst vorher.
    Ladefehler in den vorhandenen Stufen: `einst.schluessel_unbekannt` (3, unbekanntes Feld eines Eintrags), `einst.schluessel_fehlt`
    (5), `einst.typ_falsch` (6, auch `null`), `einst.anzahl` (8, mehr als 4096), `einst.kennung_form` (9, `wer` einer Gruppe),
    `einst.kennung_doppelt` (10, dasselbe Paar aus `ebene` und `wer`) und `einst.klasse_unbekannt` (12, byte-genau). Neu wären ein
    Code für eine Ebene außer `nutzer` und `gruppe` und einer für ein `wer` ohne Pseudonymform. Fehlt das Feld, liest niemand
    (datentor P2). Fälle folgen mit der Fassung, wie beim Block `effort` bis zu seinem Bau in `faelle-ausstehend/`.
14. **Plan Business und eingeschränktes Token (Nachtrag Business, 26.11.2026).** Offen: welche Hosts und Pfade die CLI mit dem Plan
    Business wirklich ruft; die Ziele `api.business.githubcopilot.com` und `exp.business.githubcopilot.com` sind hergeleitet, nicht
    gemessen. Offen auch: wie ein Admin das eingeschränkte Token `anmeldung.<route>.eingeschraenkt` in die Ablage des Halters bringt
    (Anmelde- oder Halter-Vertrag) und wie ein Admin den Schalter im Betrieb setzt; heute liest `postinst` ihn aus dem Erststart-Profil
    des Pakets, und das Profil setzt ihn nicht.
15. **Felder der Admins außerhalb der Routentabelle (Nachtrag, 26.11.2026).** Schritt 4b vergleicht heute nur die Routentabelle mit
    der Vorgabe der Firma (Abschnitt „Vorgabe der Firma für die Netzziele“). Der Block `effort` (OPEN 11) und das Feld `leseklassen`
    (OPEN 13) sind Werte der Admins. Sie stehen in der Datei des Nutzers und werden nicht verglichen. Das Crate liest den Block
    `effort` seit dem 27.11.2026; Schritt 4b vergleicht ihn trotzdem nicht mit der Vorgabe. Das Feld `leseklassen` liest das Crate
    heute nicht.
    Vorschlag: Sobald es sie liest, gilt für sie dieselbe Regel wie für die Routentabelle, also nur streichen oder verengen, nie
    erweitern.

<!-- vom Bauvorgang angefuegt -->
> Hinweis: Dieser Text nennt Werkzeuge, die in diesem Klon nicht liegen: `pi`.
> Sie gehoeren zum Setup der Maschine, aus der dieser Baum stammt; hier fehlen
> sie mit Absicht. Wer sie braucht, schreibt sich seine eigenen.
> *Named here, not shipped here.*
