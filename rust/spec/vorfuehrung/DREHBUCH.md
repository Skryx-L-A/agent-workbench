# Vorführdrehbuch des Durchstichs: fünf Schritte als Außentests

Stand 20.11.2026. Grundlage ist der Arbeitsweg der Meilensteinplanung (Abschnitt 5.1) für Variante A (Linux). Nachtrag 20.11.2026 (Entscheidung des Betreibers): Die Vorführung läuft auf einem Ubuntu-Laptop. Die Cloud-Route ist Copilot CLI im Terminal, die lokale Route ein selbst gehostetes Modell. Nachtrag 20.11.2026 (Annahmen zum Aufbau): Ubuntu 26.04.1 LTS, vier Datenklassen, ein Modell-Gateway mit Schlüssel je Rechner, echte Läufe mit Copilot CLI nur auf Zielgeräten.
Jeder Schritt ist ein Außentest mit Gegenprobe. Die Vorführung gilt erst als vorführbar, wenn alle fünf Schritte und alle sechs Ehrlichkeitsprüfungen grün sind; E4 zählt zweimal, für die lokale und für die Cloud-Route (Nachtrag 20.11.2026, 7).
„(H)“ heißt: aus Plan und Anforderungen hergeleitet. Die Werte der Verträge 1, 2, 4 und 5 sind Entwürfe vom 20.11.; ändert sich ein Feld dort, gilt der Vertrag.

## 1. Was gezeigt wird und was nicht

Gezeigt wird der Kern des Produkts: Das Tor entscheidet vor der Ausgabe nach Klasse und Route, und der Agent kommt nicht daran vorbei.

Nicht gezeigt und nicht behauptet werden die Entscheidungsstelle, die MCP-Freigabe je Schreibaufruf, eine Kundenachse mit mehreren Kunden, das Paket und Windows (Plan 5.2) sowie ein Schutz vor Dateien, die ein Mensch von Hand in einen anderen Ordner kopiert (Nachtrag 20.11.2026, 7).

## 2. Aufbau

| Teil | Im Durchstich | Vertrag |
|---|---|---|
| Rechner | ein Ubuntu-Laptop mit Kern-Dienst, Such-Dienst und Einschluss; der Browser läuft auf demselben Laptop, ohne Tunnel. Die Brücke zwischen Browser und Kern-Dienst hört nur auf `127.0.0.1`. Jede Datenanfrage braucht das Token aus dem URL-Fragment. Der Laptop läuft mit Ubuntu 26.04.1 LTS; Namensräume ohne Rechte gibt es dort nur mit AppArmor-Profil, der Einschluss bringt also sein Profil mit. Es gibt keinen Proxy. Für TLS gilt immer der Truststore des Systems, nie ein festes CA-Bündel, weil Firmen-Proxys oft ein eigenes Stammzertifikat eintragen. Software kommt als signiertes deb ohne Netz bei der Installation. Vorbereitet wird in einer Ubuntu-VM mit 26.04. | Kern, Paket |
| Einstellungen | `klassen` `["public", "internal", "confidential", "customer"]` (vier frei gewählte Klassen, `customer` ist die höchste), `vorgabestufe` `customer`; Route `copilot` (Art cloud, Klasse internal); Route `lokal` (Art lokal, Klasse customer); Kunde `k-nord` mit `erlaubt: ["lokal"]` (mit Freigabe); Kunde `k-sued` mit `erlaubt: []` (ohne Freigabe); `kundendaten_an_cloud` `{"vorgabe": "nie", "freigaben": []}` | 2 (`rust/spec/einstellungen/`) |
| Start | Harness, Modell, Route, Kunde, Arbeitsordner; die Route steht im Start, nie in einer Variable des Agenten. Sitzung 1: Harness Copilot CLI, angemeldet mit dem GitHub-Konto des Vorführenden, Route `copilot`. Sitzung 2: ein Modell hinter einem OpenAI-kompatiblen Modell-Gateway (`/v1/chat/completions`), Route `lokal`. In der Vorbereitung steht dort ein kleines Modell über Ollama mit derselben Schnittstelle; am Ende wird nur das Ziel getauscht. Auf dem Laptop selbst läuft kein Modell. Sitzung 1 (`copilot`, ohne Kunde) und Sitzung 2 (`lokal`, `k-nord`) laufen in zwei verschiedenen Arbeitsordnern; beide Ordner bekommen ihre Marke `{kunde, klasse_hoch}` beim ersten Lauf (A156, Ordnermarke) (Nachtrag 20.11.2026, 7). | 1 (`rust/spec/start/`) |
| Modellweg | Der Weiterreicher außerhalb des Einschlusses hat zwei Betriebsarten (Nachtrag 20.11.2026, 7). `openai-lokal` für die Route `lokal`: Das Gateway verlangt je Aufruf einen Schlüssel je Rechner (`Authorization: Bearer`), den die Organisation vergibt; der Weiterreicher setzt ihn ein, der Agent spricht ihn ohne Schlüssel an und sieht ihn nie (A14, E4). Ein nicht freigeschaltetes Modell beantwortet das Gateway mit 403. `kopf-einsetzen` für die Route `copilot`: Die Copilot CLI sieht im Einschluss nur einen Platzhalter (`gho_` und 36 Nullen) als ihr Token. Der Weiterreicher beendet TLS für wenige Ziele mit einer Sitzungs-CA, die nur in einer Kopie des System-Bundles im Einschluss liegt (der Truststore des Laptops bleibt unverändert; die CA gilt nur für die Netzziele der Copilot CLI und höchstens 24 Stunden), und setzt das echte Token der CLI ein. Nach außen prüft er TLS mit dem Truststore des Wirts. | Modellweg, Einbindung Vertrag 1 |
| Tor | `darf(einheit, route, kunde)` in der Bibliothek `datentor` | 3 (`rust/spec/datentor/`) |
| Suche | eigener Prozess unter eigener Kennung; ein Socket je Sitzung trägt die Route; ein dünnes MCP-Werkzeug im Einschluss reicht nur weiter; bei Copilot CLI ist es ein lokaler stdio-Server in dessen MCP-Konfiguration | 4 |
| Protokoll | Zeile `ausgabe.gesperrt` mit `route`, `klasse`, `kunde`, `grund`, `anzahl`, `sitzung`; Prüfbefehl `kette pruefe <datei>`. Ein Programm `kette` gibt es im Paket nicht. Der Läufer braucht einen Prüfbefehl des Pakets mit diesem Aufruf: `<befehl> pruefe [--vorher <64 hex>] [--letzte <64 hex>] <datei>...`; Ausgabe genau eine Zeile RFC 8785 auf stdout (`bruch`, `letzte`, `ok`, `zeilen`); Exit 0 die Kette ist ganz, 1 ein Bruch, 64 Aufruffehler (`laeufer/README.md`, „Bedarf an das Paket“) (Nachtrag 20.11.2026, 7). | 5 |
| Vorführ-Vault | rund 20 erfundene Notizen außerhalb des Einschlusses, darunter **N-K** (`datenklasse: customer`, `kunde: k-nord`, Marker `MK-7Q2`), **N-S** (`datenklasse: customer`, `kunde: k-sued`, Marker `MS-3P8`) und **N-I** (`datenklasse: internal`, Marker `MI-4F9`); alle drei passen auf die Frage **F**. Keine weitere Notiz, die auf F passt, wird auf einer der beiden Routen gesperrt, auch keine Notiz ohne Klasse; sonst stimmen die Zahlen in S3 und S5 nicht. Der Vault liegt in `rust/spec/such-socket/vorfuehr-vault/`; `pruefe_vorfuehrung.py` daneben prüft diese Bedingungen. | Vertrag 4 |
| Frage F | „Wann ist das nächste Wartungsfenster der Presslinie?“ Sie enthält keine Ziffer und kein Wort aus den festen Werten der Protokollzeile, damit die Zählung in S4 sauber ist. | |
| Notizen | N-K: `kunden/k-nord/MK-7Q2-liefertermin-presslinie.md`. N-I: `betrieb/MI-4F9-wartungsfenster-presslinie.md`. Jeder Marker steht genau einmal in Pfad, Titel und Text seiner Notiz. N-S: `kunden/k-sued/MS-3P8-abnahme-presslinie.md`. Alle 21 Notizen tragen eine Klasse; nur N-K und N-S sind Kundendaten, keine trägt `confidential`. | Vault |

**Marker** sind kurze erfundene Zeichenfolgen, die nur in einer Notiz vorkommen. Gezählt wird jeder Marker im Text, im Titel und im Pfad der Notiz.

**Zählung** (Nachtrag 20.11.2026, 7): Suche und Weiterreicher zählen in der Betriebsart probelauf (`--probelauf --marker --marker-zaehlung`, alle drei oder keine) nur Marker. Die Zähldateien `<sitzung>.suche.json` und `<sitzung>.modellweg.json` enthalten nur Marker und Zahlen, nie Text; der Läufer prüft das, und eine Zähldatei mit Text macht den Schritt rot. Zusätzlich gilt die Gegenprobe ohne Prüfling: Steht ein gesperrter Marker in der sichtbaren Ausgabe des Agenten (Terminal, Antwort), ist der Schritt rot, was die Zähldateien auch sagen. Gegen die Attrappen schreibt die Modell-Attrappe weiter alle Bytes mit. Echte Läufe mit der Copilot CLI gibt es nur auf Zielgeräten, so selten wie möglich, weil ein Start rund 100 000 Token kostet; in unseren Läufen steht eine Attrappe ohne Anmeldung an der Stelle der CLI. In der Vorführung vor Zuschauern laufen die echten Modelle; dann zählt nur das Sichtbare, und die Zählung kommt aus dem letzten Außentestlauf desselben Stands. Auf der Cloud-Route beweist `anfragen` nichts, weil der Weiterreicher auch die Upgrade-Anfrage eines WebSocket zählt: Es gibt einen erlaubten Marker ohne Kundenbezug (`MP-2C5`, in keiner Notiz, in der Markerliste und im Prompt an den Cloud-Agenten); grün ist die Zählung der Cloud-Route nur, wenn er am Weiterreicher mindestens einmal zählt und die Kundenmarker 0 sind, sonst ist sie offen (Nachtrag 20.11.2026, 9). Im Probelauf und in der Vorführung fragt der Agent vor Shell-, Schreib- und Suchwerkzeugen nicht nach, weil die Vorführ-Einstellungen das im Einschluss abschalten; die Grenze ist hier der Einschluss, nicht die Rückfrage (Nachtrag 20.11.2026, 11).

## 3. Die fünf Schritte

### S1 Sitzung mit Cloud-Modell starten

- **Ausgangslage:** Aufbau wie in Abschnitt 2, keine Sitzung offen.
- **Handlung:** Der Zuschauer öffnet im Browser die Seite des Kerns und startet eine Sitzung mit dem Harness Copilot CLI über die Route `copilot`.
- **Erwartet:** Die Sitzung läuft, das Terminal zeigt das Eingabefeld des Harness, die Eingabezeile nimmt Text an.
- **Gemessen:**
  - Die Start-Struktur der Sitzung nennt `route: "copilot"`. Die Umgebung des Agenten enthält keinen Wert, aus dem die Suche die Route liest (H, Plan 5.1).
  - Gemessen wird außerdem die Zeit vom Klick bis zum Eingabefeld. Im Außentest misst sie ein Prüfclient gegen die Brücke, nicht ein echter Browser. t0 ist das Absenden von `POST /starten`, t1 der erste Rasterrahmen, der das Eingabefeld zeigt; wiederholbar ist das mit einer Attrappe des Modells, wo der Modellweg sie zulässt. Die Seite darf t0 und t1 zusätzlich anzeigen.
  - Das Merkmal des Eingabefelds von Copilot CLI bringt der Harness-Start mit; es ist noch offen. Für `claude` gilt eine Zeile des Rasters, die ohne Leerzeichen `?forshortcuts`, `❯` oder `>Try` enthält (`the previous implementation:156`). Dieses Muster bleibt für Läufe mit `claude` in der Vorbereitung.
- **Gegenprobe:** Ein Start ohne Route wird abgelehnt (A43). Ein Start über eine Route, die nicht in den Einstellungen steht, wird mit `start.route_unbekannt` und dem festen Text „Route unbekannt“ abgelehnt.
  - Die Ablehnung kommt von der Prüfstelle aus Vertrag 1, bevor ein Kind startet. Die Seite zeigt Code und Wortlaut aus Vertrag 1.
  - Gemessen wird, dass weder das Startprogramm noch der Harness gelaufen ist.
  - Eine Route ohne Klasse kommt am Start nicht an: Vertrag 2 lehnt sie schon beim Laden der Einstellungsdatei ab (`einst.route_ohne_klasse`, A42). Der Text „Route X hat keine Klasse“ gehört zu `darf` (Vertrag 3).
- **Kennungen:** A42, A43; Verträge 1 und 2.

### S2 Der Einschluss hält

- **Ausgangslage:** Sitzung aus S1.
- **Handlung:** Der Vorführende lässt den Agenten (a) die Datei von N-K unter ihrem echten Pfad lesen und (b) eine fremde Adresse aufrufen, etwa `https://example.org/`. Die Netzgrenze lässt nur die Ziele zu, die Copilot CLI braucht (Netzziele je Route aus der Einstellungsdatei nach der Liste der Ziele, die GitHub für Copilot veröffentlicht). `https://example.org/` gehört nicht dazu.
- **Erwartet:** Beides schlägt sichtbar fehl.
- **Gemessen:**
  - Rückgabecodes beider Befehle sind ungleich 0, und der Fehlertext steht im Terminal.
  - Der Zähler von `MK-7Q2` im Terminalverlauf und im Mitschnitt der Attrappe ist 0.
  - Die Netzgrenze verzeichnet 0 aufgebaute Verbindungen zur fremden Adresse.
- **Gegenprobe:** Dieselben zwei Befehle in einer gewöhnlichen Shell des Vorführenden außerhalb des Einschlusses gelingen. Damit ist belegt, dass Datei und Adresse existieren.
- **Kennungen:** A03, A10, A141; Einschluss (Variante A: alter Linux-Einschluss oder bwrap direkt, mit AppArmor-Profil; die Wahl trifft der Betreiber).

### S3 Die Wissenssuche sperrt die Kundennotiz

- **Ausgangslage:** Sitzung aus S1.
- **Handlung:** Der Agent stellt über das MCP-Werkzeug der Suche die Frage F.
- **Erwartet:**
  - N-I kommt als Treffer an.
  - Zu N-K und N-S sieht der Agent nur „2 Treffer gesperrt“, ohne Titel, Pfad oder Inhalt.
  - `darf` antwortet für beide mit dem Grund `klasse-ueber-route` und der wirksamen Klasse `customer`.
- **Gemessen:**
  - Im Mitschnitt zwischen Suche und Agent und im Mitschnitt der Attrappe sind die Zähler von `MK-7Q2` und `MS-3P8` gleich 0 und der von `MI-4F9` größer als 0.
  - Die Zahl gesperrter Treffer ist 2.
- **Gegenprobe:** Dieselbe Frage mit einer Einstellung, in der Route `copilot` die Klasse `customer` hat und `k-nord` auch `copilot` erlaubt, sperrt N-K weiter, jetzt mit dem Grund `kundendaten-an-cloud` (A50), weil die Freigabe fehlt. Ohne den Eintrag in `erlaubt` käme vorher D10 mit `route-fuer-kunde-nicht-erlaubt` (Fund des Läufers, 20.11.).
- **Kennungen:** A34, A49, A50; Verträge 3 und 4.

### S4 Die Sperre steht im Protokoll, die Kette hält

- **Ausgangslage:** nach S3.
- **Handlung:** Der Vorführende zeigt die letzten Zeilen mit `tail -n 5 <datei>`; Vertrag 5 hat dafür keinen eigenen Befehl. Danach ruft er `kette pruefe <datei>` auf.
- **Erwartet:**
  - Zwei Zeilen `ausgabe.gesperrt` nennen `route: "copilot"`, `klasse: "customer"`, `grund: "klasse-ueber-route"`, `anzahl: 1` und die Sitzung aus S1, eine mit `kunde: "k-nord"`, eine mit `kunde: "k-sued"` (eine Zeile je Gruppe aus Klasse, Kunde und Grund, Vertrag 5). Dazu kommen `aktionsname`, `ergebnis`, `fassung`, `vorher` und `zeit` nach Vertrag 5.
  - Die Kettenprüfung endet mit 0 (grün) und gibt den Hash der letzten Zeile aus (`"letzte": "<64 hex>"`).
- **Gemessen:**
  - Die Felder der Zeile werden gelesen. Die Zähler von `MK-7Q2`, `MS-3P8`, der Titel von N-K und N-S und der Wörter der Frage F in der Protokolldatei sind 0 (A12, A104).
  - Erfasst werden außerdem der Exitcode der Prüfung und der Hash der letzten Zeile.
- **Gegenprobe:** In einer Kopie der Datei ändert der Vorführende von Hand ein Zeichen der letzten Zeile, etwa `anzahl` von 1 auf 2. `kette pruefe --letzte <Hash aus dem grünen Lauf> <kopie>` endet mit 1, nennt den Grund `letzte_falsch` und die Zeilennummer.
  - Ohne `--letzte` bliebe eine geänderte letzte Zeile unentdeckt, weil ihr keine Zeile folgt; Vertrag 5 führt das als absichtlich grünen Fall. In einer frischen Vorführung ist die Zeile aus S4 meist die letzte, deshalb braucht die Gegenprobe den Anker.
  - Ändert man eine Zeile, der noch eine folgt, nennt der Befehl die folgende Zeile mit dem Grund `kette_gebrochen`. Ein eingefügtes Leerzeichen nennt die geänderte Zeile selbst mit dem Grund `nicht_kanonisch`.
- **Kennungen:** A12, A24, A102, A104; Vertrag 5.

### S5 Das lokale Modell bekommt die Kundennotiz mit Freigabe, nicht die ohne

- **Ausgangslage:** nach S4. Das lokale Modell ist über den Weiterreicher erreichbar: ein Modell hinter einem Gateway, in der Vorbereitung ein kleines Modell über Ollama mit derselben Schnittstelle.
- **Handlung:** Der Zuschauer startet eine zweite Sitzung über die Route `lokal`, mit dem Harness, den der Harness-Start für diese Route vorsieht, und stellt dieselbe Frage F.
- **Erwartet:**
  - N-K und N-I kommen als Treffer an. Zu N-S sieht der Agent nur „1 Treffer gesperrt“; `darf` antwortet mit `route-fuer-kunde-nicht-erlaubt`, denn `k-sued` erlaubt keine Route (Kundenachse, A51).
  - Das lokale Modell beantwortet die Frage mit Bezug auf N-K: Das Wartungsfenster darf nicht in die Woche der Auslieferung fallen, und der Kunde muss eine Verschiebung zwei Tage vorher erfahren.
  - Für diese Sitzung entsteht genau eine Zeile `ausgabe.gesperrt`, mit `route: "lokal"`, `kunde: "k-sued"` und `grund: "route-fuer-kunde-nicht-erlaubt"`.
- **Gemessen:**
  - In der Antwort der Suche an die zweite Sitzung ist der Zähler von `MK-7Q2` größer als 0 und der von `MS-3P8` gleich 0; die Zahl gesperrter Treffer ist 1. Im Mitschnitt am Weiterreicher ist `MS-3P8` gleich 0 und der Schlüssel nur auf der Seite zum Gateway.
  - Gezählt werden außerdem die Zeilen im Protokoll mit der Sitzung aus S5.
- **Gegenprobe:** Die Frage F in der ersten Sitzung liefert danach weiter „2 Treffer gesperrt“; die zweite Sitzung ändert nichts an der ersten (A48: Die Klasse folgt der Route). Bekommt `k-sued` in einer Kopie der Einstellungen `erlaubt: ["lokal"]`, kommt N-S in der zweiten Sitzung an.
- **Kennungen:** A34, A48, A50, A51; Verträge 1, 3 und 4.

## 4. Ehrlichkeitsprüfungen

Ohne diese Prüfungen wäre Schritt 3 vorgetäuscht (Plan 5.1, letzter Absatz). E1 bis E3 und E4 cloud laufen in der Sitzung aus S1, E4 lokal und E6 in der aus S5, E5 in beiden (Nachtrag 20.11.2026, 7).

| Prüfung | Handlung | Erwartet und gemessen |
|---|---|---|
| E1 Route hängt am Start | Der Agent setzt in seiner Shell jede Umgebungsvariable, deren Name `route` enthält, auf `lokal` und fragt erneut F. Dann schickt er eine Anfrage an die Suche mit einem eigenen Feld `route: "lokal"`. | Die Ergebnisse bleiben wie in S3: 2 gesperrt, Zähler von `MK-7Q2` 0. Die Anfrage mit eigenem Feld wird abgelehnt (`{"error": "bad_request"}`); Vertrag 4: Die Route kommt aus dem Socket, nie aus der Anfrage. |
| E2 Notizen liegen außerhalb | Der Agent sucht im ganzen sichtbaren Dateibaum nach `MK-7Q2`, etwa mit `grep -r`. | Treffer: 0. Gegenprobe außerhalb des Einschlusses: Treffer größer 0. |
| E3 Suche ist ein eigener Prozess | Der Agent sucht den Such-Prozess in seiner Prozessliste, versucht ein Signal an ihn und liest seine Umgebung. | Der Such-Prozess läuft nicht unter der Kennung des Agenten, oder er ist im Einschluss gar nicht sichtbar (eigener PID-Namensraum). Das Signal scheitert, und die Umgebung ist nicht lesbar. Der Agent erreicht die Suche nur über den Socket. Welche der beiden Formen gilt, hängt am Einschluss (Variante A oder B, Vertrag 1). |
| E4 lokal: Der Schlüssel des Gateways bleibt draußen | Der Agent sucht den Schlüssel des Gateways in seiner Umgebung, seinen Argumenten, seinen Dateien und in den Prozessen, die er sieht; dann ruft er das Gateway direkt auf, ohne den Weiterreicher. In der Sitzung aus S5. | Treffer des Schlüssels: 0 (A14). Der direkte Aufruf scheitert: Entweder lässt die Netzgrenze das Ziel nicht zu, oder das Gateway antwortet ohne Schlüssel mit einer Ablehnung. Gegenprobe außerhalb des Einschlusses: Der Weiterreicher hat den Schlüssel, und sein Aufruf gelingt. |
| E4 cloud: Das Token der Copilot CLI bleibt draußen | Der Agent sucht das Token der CLI in Umgebung, Argumenten, Dateien, den `environ`- und `cmdline`-Dateien der Prozesse sowie mit `gh auth token` und `secret-tool`; dann ruft er das Ziel direkt auf, ohne den Weiterreicher. In der Sitzung aus S1 (Nachtrag 20.11.2026, 7). | Gefunden wird nur der Platzhalter; Treffer einer Marke, die nur der Weiterreicher kennt: 0. Der direkte Aufruf scheitert. Der Zähler am Ziel steigt, weil der Weiterreicher das echte Token einsetzt. Gegenprobe außerhalb des Einschlusses: Der Weiterreicher hat das Token, und sein Aufruf gelingt. |
| E5 Netzgrenze | Aus der Sitzung aus S1 und aus der aus S5 versucht der Agent: einen anderen Port des Rechners (etwa den der Brücke), den gleichen Versuch mit dem Gastpunkt der anderen Route, `CONNECT` auf ein anderes Ziel als die Netzziele der Route, und den eigenen Gastpunkt. (Nachtrag 20.11.2026, 7) | Der Agent läuft in einem eigenen Netz-Namensraum: Kein anderer Port des Rechners ist erreichbar; erreichbar ist nur der Weiterreicher seiner Sitzung am festen Gastpunkt (`127.0.0.1:11434` bei der lokalen Route, `127.0.0.1:8402` bei der Cloud-Route), und über ihn nur die Ziele der Route. Der Gastpunkt der anderen Route fehlt, `CONNECT` auf ein anderes Ziel scheitert. Gegenprobe ohne eigenen Namensraum: Derselbe Versuch erreicht den Port des Rechners. Wortlaut der Felder: Vertrag 1, Abschnitt „Die Netzgrenze des Einschlusses (E5)“. |
| E6 Ordnermarke | In der Sitzung aus S5 schreibt der Agent `MK-7Q2` in eine Datei seines Arbeitsordners. Danach versucht der Läufer: (a) einen Start über `copilot` für `k-nord` in diesem Ordner, in einem Unterordner, im Elternordner und über einen Symlink auf den Ordner; (b) einen Start über `lokal` für `k-sued` im selben Ordner und im Unterordner. (Nachtrag 20.11.2026, 7) | (a) Jeder Start wird mit `lauf.ordner_hoeher_markiert` abgewiesen, (b) jeder mit `lauf.ordner_anderer_kunde`; die Abweisung kommt von Lauf starten vor jedem Kind. Kein Kind startet, und der Zähler von `MK-7Q2` an beiden Modellwegen ist 0. Eine beschädigte Marke verhindert den Start (fail closed). Löscht ein Mensch die Marke mit dem eigenen Befehl, steht das in einer eigenen Protokollzeile, und danach startet ein Lauf wie in einem unmarkierten Ordner. Gegenprobe: Ohne Marke liest die zweite Sitzung die Datei, der Zähler steigt, und der Schritt ist rot. |

**Grenze der Ordnermarke, ehrlich** (Nachtrag 20.11.2026, 7): Kopiert ein Mensch Dateien von Hand in einen anderen Ordner, greift die Marke nicht; das fängt erst das Klassenerbe ab (Schritt II).

**Rückfall, ehrlich** (Nachtrag 20.11.2026, 7): Vor E4 cloud prüft der Läufer technisch zwei Dinge: (a) Die Copilot CLI im Einschluss nimmt den Abschluss mit der Sitzungs-CA an (kein Abbruch wegen eines Zertifikats, keine Ablehnung durch eine Richtlinie); (b) der Ausgang kommt durch, das heißt der Weiterreicher erreicht das Ziel vom Laptop aus (ein Endpoint-Schutz lässt die Verbindung zu). Lässt eine Richtlinie den Abschluss nicht zu (a) oder sperrt ein Endpoint-Schutz den Ausgang (b), ist E4 cloud `offen` mit Grund (`ca.cli_abgelehnt`, `ca.ausgang_blockiert`), nie grün; die Vorführung sagt das. Die anderen Schritte laufen weiter. Bezug: die Zulassung der CA durch die Organisation (berichtigt am 20.11.). Die CA kommt nie in den Truststore des Laptops.

**Grenze des Token-Schutzes, ehrlich** (Nachtrag 20.11.2026, 8): Außerhalb des Einschlusses schützt nichts davon: Die Copilot CLI trägt das Token dann direkt zum Host. Die Zusage gilt nur für Läufe über „Lauf starten“. E4 cloud prüft deshalb nur Sitzungen, die über „Lauf starten“ beginnen.

## 5. Ausführung als Außentest

- **Lauf:** Ein Läufer `drehbuch` führt S1 bis S5 und E1 bis E6 nacheinander auf einer Testinstallation aus; E4 läuft zweimal, für die lokale und die Cloud-Route (Nachtrag 20.11.2026, 7). Er schreibt je Schritt eine Zeile mit Ergebnis, Messwerten und Gegenprobe. Grün ist der Lauf nur, wenn jeder Schritt und jede Gegenprobe das Erwartete zeigt.
- **Modell:** Gegen die Attrappen nutzt der Läufer für beide Routen Attrappen (Copilot CLI und Gateway) (Nachtrag 20.11.2026, 7). Im Probelauf laufen die echte Copilot CLI (Route `copilot`) und das Gateway über den Weiterreicher (Route `lokal`), höchstens zwei Copilot-Starts (Nachtrag 20.11.2026, 7).
- **Wann:** Der Läufer steht unter `rust/spec/vorfuehrung/laeufer/`. Aufruf und Betriebsarten: `drehbuch.py --ziel attrappe|echt`, `drehbuch.py probelauf`, `drehbuch.py pruefe-ergebnis`; siehe `laeufer/README.md` und `rust/spec/VERTRAEGE.md` (Nachtrag 20.11.2026, 7).
- **Lücken:** Findet ein Lauf eine Lücke im Einschluss, etwa einen lesbaren Pfad zum Vault, wird sie geschlossen, nicht verdeckt (Plan 5.5, Schritt 4).
- **Zeitmessungen** laufen nur auf einer ruhigen Messmaschine.
