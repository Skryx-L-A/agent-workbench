# Vertrag: Einschluss eines Agentenlaufs unter Windows (Fassung 1)

Stand 20.11.2026, Variante B (Meilensteinplanung, Abschnitte 5.3 und 5.7). Grundlage sind die Messungen der Windows-Probe vom 18.11. (Fragen 1 bis 11
und „Endstand Stufe 0“), der Schnitt der Windows-Plattform und die Anforderungen A03, A115 bis A117 und A120
bis A123. „Gemessen“ verweist auf eine Frage der Probe. „Hergeleitet“ ist nicht gemessen. Was nicht gemessen ist, steht
unter OPEN und ist keine Zusage.

Der Vertrag steht vor dem Code. Alle Fälle sind heute rot, weil es keine Umsetzung gibt. Verdeckte Fälle und Mutanten
folgen mit dem Bau. Die Fälle der Form `vm` laufen nur in der Windows-VM; sie sind Außentests.

## Dateien

| Datei | Zweck |
|---|---|
| `README.md` | dieser Vertragstext |
| `einschluss.schema.json` | Form als JSON Schema 2020-12: `$defs/konfig` (Einschlussdatei) und `$defs/steuer` (Anfrage an den Plattformdienst) |
| `faelle/<name>.json` | `beschreibung`, `form` (`konfig`, `steuer`, `helfer`, `aufruf`, `vm`); bei `konfig`, `steuer` und `helfer` die Bytes als `text` oder `b64`, wo nötig die Ausgangslage als `lage` |
| `erwartet/<name>.json` | `konfig` und `steuer`: `{"gueltig": true}` oder `{"gueltig": false, "code": …}`; `helfer`: `{"antwort": …}`; `aufruf`: Exitcode, stderr-Zeile und Zustand; `vm`: `erwartet` und `messwerte` |
| `bau-faelle.py` | erzeugt Schema, `faelle/` und `erwartet/` |
| `pruefen.py` | prüft Paarung, Form, JSON-Lesen, Schema, Helferzeilen und die Abdeckung der Eigenschaften |

## Zweck und Grenze

Unter Windows läuft der Agent einer Sitzung in einem **AppContainer** mit einem **Job-Objekt**. Ein **WFP-Filterpaar je
AppContainer-SID** begrenzt sein Netz auf die Sitzungsports, also die Ports der Weiterleiter seiner eigenen Sitzung.
Jeder andere Hörer auf dem Loopback des Rechners bleibt gesperrt, ausdrücklich auch die Browser-Brücke des Kern-Dienstes.
**Freigaben je SID** geben ihm den Arbeitsordner und die
Programmordner. Sein Shell-Werkzeug ist das Werkzeug PowerShell von Claude Code (A117).

Die Schnittstelle nach außen ist dieselbe wie unter Linux: Der Kern-Dienst startet das Programm „Lauf starten“ nach
Vertrag 1 (`rust/spec/start/README.md`), und die Suche läuft je Sitzung nach Vertrag 4 (`rust/spec/such-socket/README.md`).
Dieser Vertrag legt fest, was unter Windows in den Schritten 6 bis 8 von Vertrag 1 geschieht.

## Begriffe

| Begriff | Bedeutung |
|---|---|
| Person | das angemeldete Windows-Konto, das die Werkbank benutzt; ohne Adminrechte |
| Projekt | ein kanonischer Arbeitsordner einer Person. Zwei Sitzungen auf demselben Ordner gehören zum selben Projekt |
| Ordner-Schlüssel | 64 kleine Hex-Zeichen: SHA-256 über den kanonischen Arbeitsordner, klein geschrieben, als UTF-16LE |
| AppContainer-Name | `wb.` und die ersten 32 Hex-Zeichen von SHA-256 über `<Nutzer-SID der Person>` LF `<Ordner-Schlüssel>`, als UTF-8; 35 Zeichen |
| AppContainer-SID | die SID zu diesem Namen (`DeriveAppContainerSidFromAppContainerName`), Form `S-1-15-2-` und sieben Zahlen |
| Plattformdienst | Windows-Dienst unter `NT SERVICE\wb-plattform`, kein Administrator. Er hält Jobs und Filter und misst Peers. Er entscheidet nichts (Grundsatz 8) |
| Admin-Helfer | Windows-Dienst unter LocalSystem. Er setzt und entfernt nur Loopback-Ausnahmen |
| Starter | das Programm „Lauf starten“ (Vertrag 1), unter Windows als Prozess der Person |
| Modellweg | eigener Prozess je Sitzung (Vertrag `rust/spec/modellweg/`); prüft je Anfrage den Schein |
| Ausgang | eigener Prozess je Sitzung für die Cloud-Ziele der Sitzung mit Zielliste |
| Weiterleiter | Prozess je Sitzung, vom Starter gestartet; hört auf genau einem Port auf `127.0.0.1` und `[::1]` und reicht an Modellweg oder Ausgang derselben Sitzung weiter |
| Sitzungsports | die Ports der Weiterleiter einer Sitzung, höchstens 4; der Starter lässt sie vom System vergeben, nie fest |
| Browser-Brücke | der HTTP-Hörer des Kern-Dienstes für die Seite. Er ist nie ein Sitzungsport. Der Agent erreicht ihn nie |
| Lauf | ein Agentenprozess mit seinem Job; er trägt die Kennung `sitzung` aus der Startdatei |
| Schein | Zufallswert je Lauf, nur in der Umgebung des Agenten; der Modellweg prüft ihn je Verbindung |

Kennungen haben die Form `^[a-z0-9][a-z0-9._-]{0,63}$` wie in den Verträgen 1 bis 5.

### Entscheidung: SID je Person und Projekt, aus dem Namen hergeleitet

Gemessen und entschieden ist die SID je Person und Projekt (Frage 11, Form A). Neu legt dieser Vertrag fest, wie der Name
entsteht. Der Name mischt die Nutzer-SID ein. Grund (hergeleitet): Die AppContainer-SID hängt nur am Namen. Ohne Anteil
der Person bekämen zwei Personen mit gleichem Pfad dieselbe SID, und die Freigabe der einen gälte für den Agenten der
anderen (W11). Der Ordner geht nur als Hash in den Namen, damit kein Pfad in Namen, Listen oder Protokollen steht.

Die SID schickt nie der Starter. Der Plattformdienst bildet sie selbst aus dem Nutzer-SID des Aufrufers (Impersonation
an der Pipe, A115) und dem Ordner-Schlüssel. So kann eine Person weder Filter noch Ausnahmen für ein fremdes Projekt
anfordern.

## Bausteine und Rechte

| Gegenstand | hält oder darf | darf nicht |
|---|---|---|
| Job-Handle | nur der Plattformdienst (`KILL_ON_JOB_CLOSE`, ohne `BREAKAWAY_OK`) | Starter, Agent, Modellweg |
| Recht auf den Agentenprozess | Dienst: nur `0x101` (`PROCESS_SET_QUOTA`, `PROCESS_TERMINATE`) auf den angehaltenen Agenten | Dienst: nichts auf den Starter |
| Filter-Engine | Dienst über fünf nicht vererbbare ACEs des Installers: Engine `0xC`, `sublayers` und `filters` je `0x21`, `ALE_AUTH_CONNECT_V4/V6` je `0x2` | Agent: nicht öffnen; Person: nichts lesen, löschen, ergänzen |
| Loopback-Ausnahme | nur der Admin-Helfer | Dienstkonto, Starter, Agent, Person |
| Freigaben, AppContainer-Profil | der Starter als Person und Besitzer | Dienst, Agent |
| Schein | Starter erzeugt ihn, Modellweg kennt ihn, Agent hat ihn in seiner Umgebung | nie in einer Datei |
| Geheimnisse | nur der Halter unter eigenem Dienstkonto (A116) | Agent, Person; nie DPAPI-CurrentUser |

Alle Zeilen außer der Herleitung des Namens sind gemessen (Fragen 3, 9, 10a, 11; Entwurf der Windows-Plattform).

## Einschlussdatei (`konfig`)

Die Einschlussdatei gehört zur Installation. Der Starter liest sie aus seiner eigenen Konfiguration, nie aus der
Startdatei und nie vom Agenten. Ein JSON-Objekt, UTF-8 ohne BOM, höchstens 4096 Byte, geschlossene Form, kein doppelter
Schlüssel.

```json
{"fassung": 1,
 "projektwurzeln": ["C:\\Users\\beispiel\\Projekte"],
 "programmordner": ["C:\\Users\\beispiel\\AppData\\Local\\Programs\\werkbank-harness"],
 "shell": "powershell"}
```

| Feld | Typ | Grenzen | Bedeutung |
|---|---|---|---|
| `fassung` | Ganzzahl | genau `1` | |
| `projektwurzeln` | Liste von Windows-Pfaden | 1 bis 16, ohne Doppel | Arbeitsordner liegen echt unter einer davon |
| `programmordner` | Liste von Windows-Pfaden | 1 bis 16, ohne Doppel | bekommen RX für die SID: Harness, Node, Such-MCP |
| `shell` | Text | genau `powershell` | A117; Git Bash ist im Container nicht erlaubt |

Kein Port steht in dieser Datei. Die Sitzungsports entstehen je Sitzung neu (siehe „Weiterleiter und Sitzungsports“).

**Windows-Pfad:** Laufwerksbuchstabe, Doppelpunkt und mindestens ein Segment, getrennt durch `\`, höchstens 1024 Byte
UTF-8. Ein Segment ist nicht leer, enthält keines von `< > : " / \ | ? *`, kein Steuerzeichen (U+0000 bis U+001F, U+007F)
und endet nicht auf Punkt oder Leerzeichen; damit sind `.` und `..` ausgeschlossen. Ein Segment ist kein Gerätename
(`CON`, `PRN`, `AUX`, `NUL`, `COM0` bis `COM9`, `LPT0` bis `LPT9`, gleich welche Schreibweise, auch mit Endung). Nicht
erlaubt sind die Laufwerkswurzel allein, UNC-Pfade und Gerätepfade (`\\?\`, `\\.\`). Vergleiche zwischen Pfaden
gelten ohne Rücksicht auf Groß- und Kleinschreibung.

### Fehlercodes der Einschlussdatei

Genau ein Code, der der ersten verletzten Stufe. Die Stufen 1 bis 9 brauchen nur die Datei; die Stufen 10 bis 13
brauchen Dateisystem und Person und sind erst mit dem Bau prüfbar.

| Stufe | Code | Verletzung |
|---|---|---|
| 1 | `einschluss.zu_gross` | mehr als 4096 Byte |
| 2 | `einschluss.json_ungueltig` | kein UTF-8, BOM, Syntaxfehler, doppelter Schlüssel, kein Objekt, Text danach |
| 3 | `einschluss.schluessel_unbekannt` | ein Schlüssel außerhalb der Form, auch `modellweg_port` oder `sid` |
| 4 | `einschluss.schluessel_fehlt` | ein Pflichtschlüssel fehlt |
| 5 | `einschluss.typ_falsch` | falscher JSON-Typ |
| 6 | `einschluss.fassung_unbekannt` | `fassung` ist eine Ganzzahl, aber nicht `1` |
| 7 | `einschluss.liste_grenze` | leere Liste, mehr als 16 Einträge, ein Eintrag doppelt |
| 8 | `einschluss.pfad_form` | ein Pfad verletzt die Regeln für Windows-Pfade |
| 9 | `einschluss.shell_unbekannt` | `shell` ist nicht `powershell` |
| 10 | `einschluss.pfad_ueberlappt` | ein Programmordner ist gleich einer Projektwurzel, liegt darüber oder darunter |
| 11 | `einschluss.pfad_geschuetzt` | ein Pfad ist gleich dem Profilordner der Person, dem Windows-Ordner, `Program Files` oder liegt darüber |
| 12 | `einschluss.pfad_fehlt` | ein Pfad ist kein vorhandener Ordner |
| 13 | `einschluss.pfad_umgelenkt` | der kanonische Pfad (`GetFinalPathNameByHandle`) weicht ab: Verbindungspunkt, Link oder 8.3-Kurzname |

Grund für Stufe 10: Läge ein Programmordner in einem Arbeitsordner, könnte der Agent dieses Projekts Programme ändern,
die Agenten anderer Projekte ausführen. Grund für Stufe 11: Die Freigabe des Arbeitsordners vererbt sich; über dem Profil
öffnete sie das ganze Profil.

## Windows-Pfad in der Startdatei

Vertrag 1 Fassung 1 kennt nur POSIX-Pfade (OPEN 5 dort). Unter Windows gilt für `arbeitsordner` die Regel für
Windows-Pfade oben. Ein POSIX-Pfad unter Windows und ein Windows-Pfad unter Linux ergeben `start.ordner_ungueltig`. Das ist
ein Nachtrag zu Vertrag 1; das Projekt trägt ihn dort mit dem Bau ein (Abstimmung unten).

## Ablauf in „Lauf starten“ unter Windows

Schritte 1 bis 5 und die stderr-Zeile gelten wie in Vertrag 1. Schritt 2 heißt unter Windows: Die Startdatei ist eine
reguläre Datei ohne Umlenkung, ihr Besitzer ist die Person, die DACL ist geschützt und nennt nur die Person und SYSTEM.
Sonst Exit 66, `lauf.datei_modus`. Schritte 6 bis 8 sind unter Windows:

| Schritt | Handlung | bei Fehler |
|---|---|---|
| 6a | Einschlussdatei lesen und prüfen (Stufen 1 bis 13) | 69, `lauf.einschluss_fehlt` |
| 6b | Arbeitsordner kanonisch prüfen: vorhanden, nicht umgelenkt, echt unter einer Projektwurzel | 69, `lauf.einschluss_fehlt` |
| 6c | Steuer-Pipe `\\.\pipe\wb-plattform` öffnen; Besitzer der Pipe-Instanz ist die Dienst-SID (A115) | 69, `lauf.einschluss_fehlt` |
| 6d | `projekt_neu` an den Dienst (wirkungsgleich, wenn das Projekt besteht); AppContainer-Profil anlegen oder öffnen; Freigaben setzen | 69, `lauf.einschluss_fehlt` |
| 7a | Such-Prozess starten (siehe „Such-Pipe“) und auf `{"bereit":true}` warten, Frist 10 s | 69, wie Vertrag 1 |
| 7b | Modellweg und Ausgang nach ihren Verträgen starten, dann je einen Weiterleiter; jeder Weiterleiter meldet seinen Port (siehe „Weiterleiter und Sitzungsports“), Frist 10 s | 69, `lauf.einschluss_fehlt` |
| 8a | Agent angehalten erzeugen (`CREATE_SUSPENDED`, ConPTY, `SECURITY_CAPABILITIES` ohne Fähigkeiten, Sicherheitsbeschreibung mit `0x101` für die Dienst-SID) | 69, `lauf.harness_fehlt` |
| 8b | `lauf_start` mit den Sitzungsports an den Dienst; erst nach `{"ok":true}` `ResumeThread` | sonst Agent beenden, 69, `lauf.einschluss_fehlt` |
| 8c | auf das Ende des Agenten warten, dann `lauf_ende`; Exitcode ist der des Agenten | |

Windows kennt kein exec. Der Starter bleibt darum als Elternprozess bestehen, bis der Agent endet, und gibt dessen
Exitcode zurück. Die Prüfreihenfolge der Schritte ist fest. Jeder Fehler in 6a bis 8b ergibt die eine stderr-Zeile
`lauf.einschluss_fehlt: Einschluss nicht verfuegbar` nach Vertrag 1. Den genauen Code (`einschluss.*`, `dienst.*`)
schreibt der Plattformdienst in sein Ereignisprotokoll, nicht in die PTY. Grund: Die Zeile in der PTY bleibt so fest wie
in Vertrag 1, und ein Pfad oder eine SID erreicht nie das Terminal.

**Freigaben (gemessen, Frage 6).** Nur für die AppContainer-SID, nie für ALL APPLICATION PACKAGES: Arbeitsordner Modify
mit Vererbung `(OI)(CI)`; Programmordner RX mit Vererbung; jeder Elternordner des Arbeitsordners RX ohne Vererbung;
Fensterstation und Desktop READ_CONTROL (in Session 0 gemessen, in `WinSta0` offen). Hergeleitet: RX auf den
Elternordnern zeigt dem Agenten die Namen der Einträge dort, nicht deren Inhalt (W3, OPEN 4).

**Umgebung des Agenten.** Leer bis auf diese Liste; der Starter setzt sie selbst:

| Variable | Wert |
|---|---|
| `ANTHROPIC_BASE_URL` | `http://127.0.0.1:<Port des Weiterleiters zum Modellweg dieser Sitzung>` |
| `ANTHROPIC_AUTH_TOKEN` | der Schein dieses Laufs |
| `HTTPS_PROXY` | `http://127.0.0.1:<Port des Weiterleiters zum Ausgang dieser Sitzung>`, nur wenn die Sitzung Cloud-Ziele hat; Form nach dem Vertrag des Ausgangs (OPEN 7) |
| `CLAUDE_CODE_USE_POWERSHELL_TOOL` | `1` (A117) |
| `PATH` | die Programmordner, `%SystemRoot%\System32`, `%SystemRoot%\System32\WindowsPowerShell\v1.0`; kein Ordner mit Git Bash oder ssh |
| `SystemRoot`, `TEMP`, `TMP` | `TEMP` und `TMP` zeigen in den Ordner des AppContainers (`GetAppContainerFolderPath`) |
| `NODE_PRESERVE_SYMLINKS` | `1` (gemessen ist die Option `--preserve-symlinks`; die Variable ist hergeleitet, OPEN 6) |

`SSH_AUTH_SOCK` und jede Variable mit Zugangsdaten außer dem Schein fehlen.

## Weiterleiter und Sitzungsports

Unter Linux stellt ein Weiterleiter den Modellweg im eigenen Netz-Namensraum des Einschlusses bereit; dieser Loopback
gehört nur dem Einschluss. Unter Windows gibt es keinen eigenen Namensraum: Der Agent sieht den Loopback des Rechners, auf
dem auch die Browser-Brücke und fremde Dienste hören. Die Loopback-Ausnahme öffnet ihm ohne Filter jeden dieser Hörer
(gemessen, Frage 4). Darum gilt:

1. Die dauerhafte Sperre der SID deckt `127.0.0.0/8` und `::1/128` ganz (gemessen, Fragen 7 und IPv6).
2. Erlaubt werden je Lauf nur die Sitzungsports dieses Laufs, je auf `127.0.0.1` und `[::1]`; die v4-gemappte Form hängt
   am V4-Paar (gemessen). Kein fester Port, kein Port eines anderen Dienstes, nie die Browser-Brücke.
3. Jeder Weiterleiter bindet Port 0 auf `127.0.0.1` und auf `[::1]` mit demselben Port (sonst neu versuchen, höchstens
   5 Mal) und schreibt danach genau eine Zeile `{"bereit":true,"port":<n>}` und LF auf stdout. Der Starter sammelt die
   Ports; höchstens 4 je Sitzung.
4. Der Starter schickt die Ports mit `lauf_start`. Der Dienst nimmt sie nicht blind: Für jeden Port müssen beide Hörer
   (`GetExtendedTcpTable`, Besitzer-PID) einem Prozess gehören, dessen Elternprozess der Aufrufer ist, der nach dem
   Aufrufer gestartet ist, der Nutzer-SID des Aufrufers gehört und in keinem Job eines Laufs liegt. Sonst
   `dienst.port_fremd`, und kein Filter ändert sich. Grund: Der Starter läuft als Person. Ohne diese Prüfung könnte er den
   Port der Browser-Brücke oder eines fremden Dienstes freigeben lassen (W14).
5. Die Erlaubnis gilt je Lauf. `lauf_ende` und das Jobende entfernen genau die Erlaubnisse dieses Laufs; die Sperre bleibt.

**Entscheidung: TCP für Modellweg und Ausgang, Named Pipe für die Suche.** Claude Code erreicht sein Modell nur über eine
HTTP-Basisadresse (`ANTHROPIC_BASE_URL`); gemessen ist nur dieser Weg über Loopback mit Ausnahme (Nachmessung, Frage 8).
Eine Named Pipe kann er als Basisadresse nicht nutzen; dasselbe gilt für den Proxy des Ausgangs (hergeleitet). Die Suche geht
über `such-mcp` per stdio und damit über eine Pipe ohne Port (siehe unten).

**Was WFP trennt und was nicht (gemessen, Frage 11).** WFP filtert je SID, nicht je Lauf. Ein Sitzungsport einer Sitzung
mit **anderer SID** ist gesperrt. Ein Sitzungsport einer anderen Sitzung im **selben Projekt** ist auf TCP erreichbar,
solange beide Läufe leben. Dort trennt allein der Hörer: Modellweg und Ausgang prüfen je Verbindung `peer` mit `tcp` und
nehmen nur den Peer im Job ihrer eigenen Sitzung an (W10). Die Weiterleiter selbst prüfen nichts; sie reichen an Prozesse
weiter, die prüfen.

## Such-Pipe (Vertrag 4 unter Windows)

**Entscheidung: Named Pipe, nicht AF_UNIX und nicht TCP.** Gründe:

1. Nur für die Named Pipe ist die Peer-Prüfung gemessen: SID über `ImpersonateNamedPipeClient` 100 von 100,
   Vorbelegung scheitert mit `FILE_FLAG_FIRST_PIPE_INSTANCE` (Frage 1, A115).
2. TCP auf Loopback scheidet für die Suche aus: Der Weg geht über `such-mcp` per stdio, braucht also keinen Port. Jeder
   weitere Sitzungsport vergrößert die Fläche des Filterpaars; A120 verbietet zudem einen ungeschützten TCP-Dienst.
3. AF_UNIX unter Windows ist für AppContainer nicht gemessen, und Windows liefert dort keine Peer-SID.

**Name:** `\\.\pipe\wb-such-<32 Hex-Zeichen Zufall>`, je Sitzung neu. Der Starter übergibt den Namen dem Such-Prozess mit
`--socket` und dem MCP-Werkzeug im Container über dessen Argument. Der feste Gastpfad `/run/brain/such.sock` aus Vertrag 4
gilt unter Windows nicht; `such-mcp` bekommt unter Windows `--socket <pipe>` (Nachtrag zu Vertrag 4, Abstimmung unten).

**DACL:** geschützt; Zugriff Lesen und Schreiben für die Nutzer-SID der Person und für die AppContainer-SID des Projekts,
sonst niemand. Hergeleitet: Für einen Prozess im AppContainer prüft Windows sowohl die Nutzer-SID als auch die Paket-SID;
darum braucht die DACL beide. `--socket-rechte` aus Vertrag 4 hat unter Windows keine Wirkung; der Such-Prozess setzt die
DACL selbst, und der Starter übergibt dafür die AppContainer-SID mit `--paket-sid <sid>` (Nachtrag zu Vertrag 4).

**Peer-Prüfung je Verbindung.** Zwei Läufe im selben Projekt teilen die SID (gemessen, Frage 11). Die DACL trennt sie also
nicht. Der Such-Prozess prüft deshalb je Verbindung: Er liest die Client-PID (`GetNamedPipeClientProcessId`), fragt den
Plattformdienst mit `peer` und `pid`, und nimmt die Verbindung nur an, wenn `sitzung` gleich seiner eigenen ist. Sonst
schließt er sie ohne Antwort. Antwortet der Dienst nicht binnen 500 ms, schließt er sie ebenso. Grund: Hat Lauf 1 die
Route `lokal-beispiel` und Lauf 2 die Route `cloud-beispiel`, bekäme der Agent von Lauf 2 sonst Treffer, die seine Route
nicht sehen darf (W2).

**Lebensdauer.** Der Starter legt den Such-Prozess in einen eigenen Job mit `KILL_ON_JOB_CLOSE`, dessen Handle nur er
hält. Endet der Starter, auch hart, endet der Such-Prozess (hergeleitet aus Frage 9, V2). Das ersetzt
`PR_SET_PDEATHSIG` aus Vertrag 1. Der Such-Prozess liegt nicht im Job des Agenten.

## Steuer-Pipe des Plattformdienstes (`steuer`)

`\\.\pipe\wb-plattform`, angelegt mit `FILE_FLAG_FIRST_PIPE_INSTANCE`. DACL: SYSTEM, die Dienst-SID und interaktive
Nutzer; keine Paket-SID, also nicht für Agenten (gemessen: fremde Pipe Fehler 5, Fragen 2 und 5). Je Anfrage eine Zeile:
ein JSON-Objekt, UTF-8 ohne BOM, mit LF, höchstens 4096 Byte. Je Antwort eine Zeile JSON mit LF. Lesefrist 3 s je Zeile;
danach schließt der Dienst die Verbindung ohne Antwort. Der Dienst bedient mehrere Verbindungen zugleich (OPEN 3).

| op | Felder | Aufrufer | Wirkung | Antwort bei Erfolg |
|---|---|---|---|---|
| `projekt_neu` | `ordner` | Starter | SID bilden; dauerhafte Sperre V4 und V6 im eigenen Sublayer; dann Helfer `SETZE` | `{"ok":true,"sid":"S-1-15-2-…"}` |
| `lauf_start` | `ordner`, `sitzung`, `pid`, `ports` | Starter | Besitzer der Ports prüfen; Job anlegen, angehaltenen Prozess zuweisen, dynamische Erlaubnis je Port auf `127.0.0.1` und `[::1]` für diesen Lauf | `{"ok":true}` |
| `lauf_ende` | `sitzung` | Starter | Job schließen (der Baum endet); die Erlaubnisse dieses Laufs entfernen; die Sperre bleibt | `{"ok":true}` |
| `projekt_loeschen` | `ordner` | `einschluss-loeschen` | nur ohne lebenden Lauf: Erlaubnis weg, Helfer `ENTFERNE`, aus der Liste nachweisen, dann Sperre und Sublayer löschen | `{"ok":true}` |
| `peer` | genau eines von `tcp` (`lokal`, `fern`) und `pid` | Modellweg, Such-Prozess | Prozess sofort öffnen, Startzeit, AppContainer-SID und Job lesen | `{"ok":true,"pid":…,"start":…,"sid":…,"sitzung":…}` |

`ordner` ist der Ordner-Schlüssel, `sitzung` eine Kennung, `pid` eine Ganzzahl von 1 bis 4 294 967 295, `lokal` und
`fern` Ganzzahlen von 1 bis 65 535. `ports` ist eine Liste von 1 bis 4 verschiedenen Ganzzahlen von 1 bis 65 535: die
Sitzungsports dieses Laufs. `start` ist die Startzeit als FILETIME-Ganzzahl. `peer` beantwortet nur Prozesse, deren
Nutzer-SID gleich der des Aufrufers ist. Ein Fehler kommt als `{"ok":false,"code":"dienst.<code>"}`. `lauf_ende` sendet
der Starter ausdrücklich; leert sich ein Job ohne `lauf_ende`, räumt der Dienst ihn ebenso ab.

`lauf_start` prüft den Prozess: Er ist angehalten, trägt die AppContainer-SID des Projekts, gehört der Nutzer-SID des
Aufrufers, und sein Elternprozess ist der Aufrufer. Sonst `dienst.prozess_falsch`; der Dienst weist dann nichts zu, und der
Starter beendet den Agenten, bevor er je läuft.

### Fehlercodes der Steuer-Pipe

| Stufe | Code | Verletzung |
|---|---|---|
| 1 | `dienst.zu_gross` | mehr als 4096 Byte vor dem LF |
| 2 | `dienst.json_ungueltig` | kein UTF-8, BOM, Syntaxfehler, doppelter Schlüssel, kein Objekt, Text nach dem Objekt |
| 3 | `dienst.op_unbekannt` | `op` fehlt, ist kein Text oder steht nicht in der Tabelle |
| 4 | `dienst.schluessel_unbekannt` | ein Schlüssel außerhalb der Form der op, ausdrücklich auch `port`, `sid`, `arbeitsordner` |
| 5 | `dienst.schluessel_fehlt` | ein Pflichtfeld fehlt; bei `peer` nicht genau eines von `tcp` und `pid` |
| 6 | `dienst.typ_falsch` | falscher JSON-Typ, auch `4242.0` statt `4242` |
| 7 | `dienst.form` | `sitzung` oder `ordner` verletzt seine Form |
| 8 | `dienst.grenze` | `pid`, `lokal`, `fern` oder ein Port außerhalb der Grenzen; `ports` leer, mit mehr als 4 Einträgen oder mit einem Port doppelt |
| 9 | `dienst.projekt_unbekannt` | kein `projekt_neu` für diesen Ordner und diese Person |
| 10 | `dienst.sitzung_besteht` | `lauf_start` für eine Sitzung, die schon läuft |
| 11 | `dienst.sitzung_unbekannt` | `lauf_ende` für eine Sitzung ohne Lauf |
| 12 | `dienst.prozess_falsch` | der Prozess erfüllt die Prüfung von `lauf_start` nicht |
| 13 | `dienst.port_fremd` | ein Port hat nicht auf `127.0.0.1` und `[::1]` je einen Hörer eines Weiterleiters des Aufrufers (Abschnitt „Weiterleiter und Sitzungsports“, Regel 4), oder er ist schon für einen anderen Lauf erlaubt |
| 14 | `dienst.lauf_lebt` | `projekt_loeschen`, solange ein Job der SID einen Prozess hat |
| 15 | `dienst.peer_unbekannt` | kein Prozess zuzuordnen, fremde Person, oder kein Job eines Laufs |
| 16 | `dienst.helfer_fehlt` | `projekt_neu`: der Helfer antwortet nicht; die Sperre steht, die Ausnahme nicht |
| 17 | `dienst.ausnahme_bleibt` | `projekt_loeschen`: der Helfer hat nicht entfernt; Sperre und Ausnahme bleiben |
| 18 | `dienst.intern` | jeder andere Fehler |

Die Stufen 1 bis 8 brauchen nur die Zeile und sind heute gegen das Schema prüfbar (außer `4242.0`). Die Stufen 9 bis 18
brauchen den Dienst.

## Admin-Helfer (`helfer`)

`\\.\pipe\wb-helfer`, `FILE_FLAG_FIRST_PIPE_INSTANCE`, DACL nur SYSTEM und die Dienst-SID des Plattformdienstes. Der
Helfer prüft den Aufrufer doppelt: Token des impersonierten Clients und Prozess des Clients müssen die Dienst-SID tragen.
Die Form bleibt die gemessene Textzeile (Frage 11). Grund: Unter LocalSystem soll der kleinste mögliche Leser stehen; ein
JSON-Leser wäre dort mehr Code mit vollen Rechten.

Anfrage: `SETZE <sid>` LF oder `ENTFERNE <sid>` LF, höchstens 512 Byte mit dem LF. Zeichensatz: `A` bis `Z`, `0` bis `9`,
`-` und Leerzeichen. `<sid>` ist `S-1-15-2-` und genau sieben Dezimalzahlen ohne führende Null, jede höchstens
4 294 967 295. Antworten, je eine Zeile mit LF, in dieser Prüfreihenfolge:

| Stufe | Antwort | Wann |
|---|---|---|
| 1 | Fehler 5 an der Pipe oder `ABGELEHNT aufrufer` | Aufrufer ist nicht der Plattformdienst |
| 2 | `ABGELEHNT zu_lang` | kein LF in den ersten 512 Byte, und mindestens 512 Byte gelesen |
| 2 | keine Antwort, Verbindung zu | kein LF binnen 3 s |
| 3 | `ABGELEHNT anfrage_form` | ein Byte außerhalb des Zeichensatzes vor dem LF, oder Bytes nach dem LF |
| 4 | `ABGELEHNT anfrage_unbekannt` | die Zeile beginnt nicht mit `SETZE ` oder `ENTFERNE ` |
| 5 | `ABGELEHNT sid_form` | der Rest ist keine AppContainer-SID in dieser Form |
| 6 | `ABGELEHNT ohne_sperre` | `SETZE` für eine SID ohne dauerhafte Sperre im Sublayer des Plattformdienstes |
| 7 | `ABGELEHNT intern` | jeder andere Fehler |
| – | `OK gesetzt vorher=0\|1`, `OK entfernt vorher=0\|1` | Erfolg; `vorher` sagt, ob die SID schon in der Liste stand |

Der Helfer liest vor jeder Änderung die ganze Liste (`NetworkIsolationGetAppContainerConfig`) und schreibt sie mit nur
dieser einen Änderung zurück; jede andere SID bleibt. Bei jeder Ablehnung bleibt die Liste gleich. Stufe 6 ist die
Härtung vom 18.11.2026: Die Seite mit den vollen Rechten prüft selbst, dass es nie eine Ausnahme ohne Sperre gibt.

## Programm `einschluss-loeschen`

`argv = [einschluss-loeschen, <arbeitsordner>]`, als Person. Der Arbeitsordner und sein Inhalt bleiben; gelöscht wird nur
der Einschluss des Projekts. Ablauf: `projekt_loeschen`; nur bei `{"ok":true}` Freigaben entfernen und
`DeleteAppContainerProfile` als Person. Wiederholbar: Fehlt das Projekt schon, ist das Erfolg.

| Exit | Code | Grund (fest, ASCII) |
|---|---|---|
| 0 | | Einschluss gelöscht oder nicht vorhanden |
| 64 | `loeschen.aufruf` | `genau ein Pfad erwartet` |
| 65 | `loeschen.ordner_form` | `Arbeitsordner ungueltig` |
| 69 | `loeschen.dienst_fehlt` | `Plattformdienst nicht erreichbar` |
| 75 | `loeschen.lauf_lebt` | `Lauf lebt noch` |
| 75 | `loeschen.ausnahme_bleibt` | `Ausnahme nicht entfernt, Sperre bleibt` |
| 70 | `loeschen.intern` | `interner Fehler` |

stderr wie in Vertrag 1: eine Zeile `<code>: <grund>`. Abräumen gibt es nur hier, nie beim Dienststart (gemessen, Frage 11:
Abräumen beim Start entfernte Ausnahme und Sperre eines ruhenden Projekts).

## Sicherheitszusagen (Eigenschaften)

Jede Eigenschaft hat einen Außentest `vm-w<n>-fall` und eine Gegenprobe `vm-w<n>-gegenprobe`. Die Gegenprobe baut die
Schwäche absichtlich ein und muss rot werden; so zeigt sie, dass die genannte Maßnahme die Grenze trägt.

| | Eigenschaft | Stand |
|---|---|---|
| W1 | Der Agent erreicht Halter und Such-Prozess nur über ihren Kanal. Er öffnet die Halter-Pipe nicht und öffnet weder Halter, Such-Prozess noch Starter als Prozess | Halter-Pipe gemessen; OpenProcess nicht gemessen |
| W2 | Der Agent erreicht nur die Suche seiner eigenen Sitzung, auch neben einem zweiten Lauf derselben SID | nicht gemessen |
| W3 | Der Agent liest keine Daten der Person außerhalb der Freigaben: weder Profil noch Ordner anderer Projekte; die Werkbank legt nichts in DPAPI-CurrentUser | gemessen (Fragen 2, 5, 6); DPAPI-CurrentUser selbst entschlüsselt im Container (gemessen), darum die Regel A116 |
| W4 | Der Agent hat kein Netz außer den Sitzungsports seines Laufs auf `127.0.0.1` und `[::1]` (auch v4-gemappt): kein anderer Hörer auf dem Loopback des Rechners, ausdrücklich nicht die Browser-Brücke, kein LAN, kein Internet | Sperre und ein erlaubter Port gemessen (Fragen 7, IPv6); Browser-Brücke, LAN und Internet nicht gemessen |
| W5 | Stirbt der Plattformdienst, endet der Job, oder der Agent erreicht nichts; nie Ausnahme ohne Sperre | gemessen (Fragen 9, 10a) |
| W6 | Kein Prozess des Laufs verlässt den Job; nach dem Jobende lebt keiner und keine Erlaubnis der SID | gemessen (Fragen 2, 5) |
| W7 | Nach einem Windows-Neustart stehen Sperre und Ausnahme, die Erlaubnis nicht; ein Prozess der SID erreicht nichts bis zum nächsten Lauf | gemessen (Frage 10b2) |
| W8 | Im Betrieb hat keiner Adminrechte: Dienst unter `NT SERVICE` mit fünf ACEs; Agent und Person ändern keine Filter und keine Ausnahme | gemessen (A122, Frage 10a) |
| W9 | Der Helfer setzt und entfernt nur auf `SETZE` und `ENTFERNE` des Dienstes und nur für SIDs mit Sperre | Gegenversuche gemessen (Frage 11); `ohne_sperre` nicht gemessen |
| W10 | Zwei Läufe derselben SID trennen Modellweg und Ausgang: Der Schein des einen Laufs wirkt nicht aus dem anderen, und ein Peer aus dem Job des anderen Laufs wird abgelehnt | Modellweg gemessen (Frage 11, je mindestens 42 von 42); Ausgang nicht gemessen |
| W11 | Zwei Personen mit gleichem Pfad haben verschiedene SIDs und sehen ihre Ordner gegenseitig nicht | nicht gemessen (Herleitung des Namens) |
| W12 | Ein Projekt mit lebendem Lauf wird nicht gelöscht; fällt der Helfer aus, bleibt die Sperre; Löschen ist wiederholbar | gemessen (Frage 11); Wiederholung nicht gemessen |
| W13 | Das Shell-Werkzeug ist PowerShell im Container; seine Kinder halten W1, W3, W4 und W6; Git Bash ist nicht erreichbar | PowerShell gemessen (Frage 5); bash-Aufruf aus dem Werkzeug nicht gemessen |
| W14 | Freigegeben werden nur Sitzungsports dieses Laufs: Fordert der Starter einen Port, den kein eigener Weiterleiter hält (etwa die Browser-Brücke), lehnt der Dienst ab, und kein Filter ändert sich. Der Sitzungsport einer Sitzung mit anderer SID ist gesperrt | nicht gemessen |

Jeder Außentest, der Claude Code startet, legt einen ssh-Stummel vorn in den `PATH` und leert `SSH_AUTH_SOCK`
(Anforderungsliste, Testaufbau vom 18.11.2026). Jeder VM-Lauf folgt den Regeln der geteilten VM (eigener Schnappschuss,
Rückkehr, nur `frisch-installiert` bleibt).

## Prüfbarkeit der Fälle

`pruefen.py` prüft heute: Paarung, Pflichtfelder je Form, Größe und JSON-Lesen, das Schema gegen jeden Fall der Formen
`konfig` und `steuer` (gültige nimmt es an, Stufen der Schemacodes lehnt es ab, spätere Stufen nimmt es an), die
Helferzeilen gegen eine Referenz der Stufen 2 bis 5 und je Eigenschaft W1 bis W14 genau einen Fall und eine Gegenprobe. Erst mit dem
Bau prüfbar sind die Stufen 10 bis 13 der Einschlussdatei, 9 bis 18 der Steuer-Pipe, die Stufen 1 und 6 des Helfers, die
Reihenfolge der Stufen, alle `aufruf`-Fälle und alle `vm`-Fälle.

```sh
python3 -I rust/spec/windows-einschluss/bau-faelle.py            # Schema und Fälle neu erzeugen
<python mit jsonschema> -I rust/spec/windows-einschluss/pruefen.py  # Exit 0 bei 0 Fehlern; ohne jsonschema Exit 2 (bewusst ohne Schema: --ohne-schema)
```

## Abstimmung mit anderen Bauteilen

| Bauteil | liest oder liefert | Annahme dieses Vertrags |
|---|---|---|
| Vertrag 1 | `arbeitsordner` als Windows-Pfad unter Windows; Schritte 6 bis 8 | Nachtrag zu Vertrag 1, OPEN 5 dort; Codes und stderr-Zeile bleiben |
| Vertrag 4 | Such-Prozess mit `--socket <pipe>` und `--paket-sid`; `such-mcp --socket <pipe>` im Container; Peer-Prüfung über `peer` | Vertrag 4 übernimmt Named Pipe, DACL und Peer-Prüfung |
| Kern, Träger | ConPTY des Kern-Dienstes; Browser-Brücke auf dem Loopback des Rechners | die Browser-Brücke ist nie ein Sitzungsport; sie bleibt hinter der Sperre der SID (W4, W14). Ihr Token ist eine zweite Linie, nicht die Grenze |
| Modellweg, Vertrag `rust/spec/modellweg/` | Modellweg je Sitzung; unter Windows ein Weiterleiter auf einem Sitzungsport | Modellweg und Ausgang prüfen unter Windows je Verbindung `peer` mit `tcp` (500 ms Frist, bei Ausbleiben ablehnen; gemessen, Frage 8). Die Anbieterangabe des Harness nennt unter Windows den Sitzungsport statt `127.0.0.1:11434`. Der Weiterleiter meldet `{"bereit":true,"port":<n>}` |
| Stelle | Projektliste für die Erkennung von Waisen | später; dieser Vertrag braucht sie nicht |
| Anforderungen | Außentests W1 bis W14 als Fälle zu A03, A115 bis A117, A120 bis A123 | W2, W11 und die Teile „nicht gemessen“ als neue Außentests |

## OPEN

1. **Interaktive Sitzung (`WinSta0`).** Alle Messungen liefen in Session 0. Ob die Freigabe auf Fensterstation und Desktop
   dort nötig, wirkungslos oder schädlich ist, ist nicht gemessen.
2. **Agent öffnet eine Pipe mit seiner Paket-SID in der DACL.** Gemessen ist nur, dass fremde Pipes scheitern. Dass der
   Agent die Such-Pipe mit der DACL oben öffnen kann, ist hergeleitet. Scheitert es, braucht die Such-Pipe einen anderen
   Weg (etwa eine Fähigkeits-SID je Lauf); W1 und W2 bleiben dann unverändert.
3. **Steuer-Pipe mit Lesefrist und gleichzeitigen Anfragen** ist verlangt, aber nicht gemessen. Die Probe bediente eine
   Anfrage nach der anderen; 5 Ablehnungen am Modellweg kamen daher.
4. **Namen in den Elternordnern.** RX ohne Vererbung zeigt dem Agenten die Namen dort. Ob ein engeres Recht
   (nur Durchqueren) für PowerShell reicht, ist nicht gemessen.
5. **PID-Wiederverwendung** im Peer-Weg ist nur durch das sofortige Öffnen und die Startzeit abgefangen, nicht erzwungen
   gemessen.
6. **`NODE_PRESERVE_SYMLINKS`** statt der gemessenen Option `--preserve-symlinks`; und ob der native Harness sie überhaupt
   braucht.
7. **Ausgang unter Windows.** Ob der Ausgang als Proxy über `HTTPS_PROXY` angesprochen wird, legt sein Vertrag fest; dieser
   Vertrag nimmt das nur an. Die Prüfung der Port-Besitzer (Regel 4) ist hergeleitet und nicht gemessen, ebenso das Binden
   desselben Ports auf `127.0.0.1` und `[::1]`.
8. **Internet und LAN ohne Fähigkeit** (W4) und der Aufruf von `bash.exe` aus dem Werkzeug (W13) sind nicht gemessen.
9. **Harness `pi`** im AppContainer ist nicht gemessen; gemessen ist nur Claude Code 2.1.327. PowerShell 7 ist nicht
   gemessen. CPython mit ctypes startete im Container nicht.
10. **Waisen**, also Projekte, die gelöscht wurden, während Dienst oder Rechner aus waren: Erkennung über die Projektliste
    der Stelle ist entschieden, aber nicht gebaut.
11. **Herleitung der SID nur aus dem Namen** (W11) ist aus der Dokumentation hergeleitet, nicht gemessen.
12. **Scheine beendeter Läufe** entzieht der Modellweg; heute wirken sie nur nicht, weil der Job leer ist.

<!-- vom Bauvorgang angefuegt -->
> Hinweis: Dieser Text nennt Werkzeuge, die in diesem Klon nicht liegen: `brain` `pi`.
> Sie gehoeren zum Setup der Maschine, aus der dieser Baum stammt; hier fehlen
> sie mit Absicht. Wer sie braucht, schreibt sich seine eigenen.
> *Named here, not shipped here.*
