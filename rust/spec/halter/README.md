# Vertrag Halter: Schlüssel außerhalb der Sitzung, Windows, Linux und macOS (Fassung 1)

Stand 20.11.2026, Variante B (M3/M4 der Meilensteinplanung); erweitert um Linux
und macOS (Projektentscheid, 20.11.2026). Windows ist das Hauptsystem und hat den vollen Umfang. Die Schnittstelle ist auf
allen drei Systemen gleich; Unterschiede stehen nur in Abschnitt 2.
Grundlage: der Halterentwurf (mit Nachträgen Projektentscheid, 18.11.2026), der Entwurf zur Freigabe von MCP-Servern,
der Probenplan der Windows-Plattform (Frage 1 Named Pipe, Frage 3 Geheimnisort) und die Anforderungen A14, A93, A105, A106,
A115, A116. Vorlage für die Zusagen waren die Halter des alten Stacks (`the previous implementation`, Abschnitte 7 und 11;
`the previous implementation`).

**Nachtrag B05 (23.11.2026, Projektentscheid, 23.11.2026):** Der Token der Copilot CLI liegt ab Schritt I nur
beim Halter (Befund B05, A93). Dafür gilt: eigener Halter-Prozess auch unter Linux (Abschnitt 2.3), neue Art
`anmeldung.*` im Bereich `person` (Abschnitt 4), Feld `abfrage` und engere Körpergrenze in `modell.aufruf`, die neuen Ops
`modell.strom` (WebSocket) und `modell.suche`, Pfadprüfung im Halter, Antwortköpfe nach Sperrliste, `identity` an jedem
Modellziel und Fristen zum Ziel. Den Gegenpart regelt Vertrag 8 (`rust/spec/modellweg/`, Abschnitt 5).
Was sich gegenüber dem Stand vom 22.11. ändert, steht an der Stelle mit „alt → neu“.

**Nachtrag Anmelden und Paket (23.11.2026):** Anmelde- und Paketvertrag tragen die Pfade nach
(`rust/spec/anmelden/README.md`, Nachtrag Halter; `rust/spec/paket/README.md`, Nachtrag Halter). Hier ändern sich der Ort der
Ablage unter Linux (fester Pfad aus `passwd` statt `$XDG_STATE_HOME`), die Regeln für ihre Ebenen, die Form einer Datei der
Ablage, Abschnitt 13 und die OPEN 18 und 21.

**Nachtrag Rolle aus der Gruppe (25.11.2026, Projektentscheid, 25.11.2026):** Unter Linux folgt die Rolle eines Peers aus
seiner effektiven gid (`SO_PEERCRED`), nicht mehr aus `/proc/<pid>/exe`. Das Rollenprogramm der Brücke trägt dafür setgid mit
der Systemgruppe `wb-bruecke`; `halter.json` ordnet jeder Rolle eine Gruppe zu (Abschnitt 2.3a, Schema `$defs/konfig`). Kein Konto
aus `konten` darf Mitglied einer Rollengruppe sein. Anlass (gemessen am Bau, Ergebnis des Halter-Baus vom 25.11.2026): Der Modellweg
setzt `PR_SET_DUMPABLE 0`; danach liest der Halter `/proc/<pid>/exe` nicht, und die Brücke bekam nie ihre Rolle. Es ändern sich
die Abschnitte 2.1, 2.3, die Tabelle am Ende von 2.4, 9, 11 (L3, neu L10), 12, 13 und die OPEN 3, 18 und 21; neu sind Abschnitt 2.3a
und die Fälle der Form `konfig`.

**Nachtrag Socket, vorhanden nach Ziel und Kette (25.11.2026, Entscheide des Vertragsinhabers, 25.11.2026):** Unter Linux liegt
der Socket immer unter `/run/user/<uid>`, nie unter `$XDG_RUNTIME_DIR` (Abschnitt 2.3, Fund F17). `schluessel.vorhanden` nimmt
genau eines von `name` oder `ziel`; nach `ziel` fragt nur die Rolle `bruecke` (Abschnitte 2.1, 4 und 7). `kopf` ist ein Objekt;
der Vertrag nennt das als Zusage (Abschnitt 4). Ein Treffer der Leck-Sperre schreibt die Kettenzeile `halter.leck_gesperrt`
(Abschnitt 9, Fund F4). Abschnitt 13 nennt 31 verbotene Kopfnamen wie Abschnitt 4. Es ändern sich die Abschnitte 2.1, 2.3, 4,
7, 9, 10, 12, 13 und OPEN 2; neu sind neun Fälle zu `schluessel.vorhanden`.

**Nachtrag N1 (25.11.2026, Entscheid der Leitung nach der zweiten Sicherheitsprüfung):** Am Modell-Host stehen nur genaue
Pfade (Abschnitt 4); jeder abgelehnte Pfad schreibt eine Kettenzeile `halter.pfad_verboten` mit Methode und Pfad (Abschnitt 9).
Es ändern sich die Abschnitte 4, 9, 12, 13 und OPEN 2; neu sind vier Fälle.

**Nachtrag Erststart (25.11.2026, Entscheid der Leitung zum ersten echten Start):** Am Modell-Host gelten beim ersten Start nur
`GET /models` und `POST /chat/completions` (Erststart-Profil, `rust/spec/einstellungen/erststart.json`). Für jeden weiteren Pfad
misst der Halter vor der Freigabe die Form der Antwort: Hinter dem Schalter `erststart_antwortform` eines Ziels schreibt er für genau
die genannten Regeln die Feldnamen der JSON-Antwort, nie Werte, in eine Journalzeile (Abschnitt 4, „Antwortform im Erststart“). Es
ändern sich die Abschnitte 2.3a, 4, 8, 9, 12 und 14 (neu OPEN 24 und 25); neu sind sieben Anfragen und sieben Fälle der Form
`konfig`. Kein bestehender Fall und keine Erwartung ändern sich.

Der Vertrag steht vor dem Code. Dieser Stand enthält die sichtbaren Fälle. Verdeckte Fälle und Mutanten folgen mit dem
Bau. Was nur auf einem System prüfbar ist, steht als Außentest in Abschnitt 11: W für die Windows-VM, L für den Linux-Baurechner, M für
den Mac.
„Gemessen“ heißt hier: in einer genannten Messung belegt (Windows-Probe 18.11., Mac 14.11., Tür-Dienst-Lauf 2). Alles
andere ist festgelegt oder hergeleitet.

## Dateien

| Datei | Zweck |
|---|---|
| `README.md` | dieser Vertragstext |
| `halter.schema.json` | Form als JSON Schema 2020-12: `$defs/anfrage`, `$defs/antwort`, `$defs/schein`, `$defs/konfig` (`halter.json` unter Linux, Nachtrag Rolle aus der Gruppe) |
| `faelle/<name>.json` | `beschreibung`, `form` (`anfrage`, `antwort` oder `konfig`), die Zeile als `text` (UTF-8), `b64` (rohe Bytes) oder `erzeugt` (lange Zeile aus Füllbytes); bei `anfrage` die Lage des Halters als `lage`; bei `konfig` der Text von `halter.json` als `text` und die `umgebung` des Starts (`uid`, `group`, `passwd`, Abschnitt 2.3a) |
| `erwartet/<name>.json` | `{"gueltig": true}` oder `{"gueltig": false, "code": "halter.<code>"}`; bei Antworten `antwort.form` für eine Zeile, die der Halter nie senden darf; bei `konfig` nur `halter.konfig` (Start Schritt 2, Exit 78) |
| `bau-faelle.py` | erzeugt `faelle/` und `erwartet/` |
| `pruefen.py` | prüft die Fälle (Abschnitt 12) |
| `werkzeug.py` | strenges JSON-Lesen, RFC 8785, Ed25519 nach RFC 8032 (nur Standardbibliothek) |

**Kennung** heißt wie in den Verträgen 1 bis 5: Text der Form `^[a-z0-9][a-z0-9._-]{0,63}$`.

## 1. Zweck und Grenze

Der Halter hält Schlüssel und Tokens außerhalb jeder Agentensitzung. Er gibt nie einen Wert heraus. Er führt nur aus:
einen Modellaufruf mit dem Schlüssel des Ziels, einen MCP-Aufruf mit dem Token des Menschen. Ein Agent erreicht ihn nie.

Begriffe:

- **Halter:** der Prozess `wb-halter`. Unter Windows ein Dienst je Maschine unter eigenem Dienstkonto (Projektentscheid
  18.11.2026: ein festes Konto je Maschine; die Trennung der Läufe macht die Prozessgrenze, nicht ein Konto je Lauf).
  Unter Linux und macOS in Fassung 1 ein eigener Prozess der Person außerhalb des Einschlusses (Abschnitt 2), nie ein
  Teil des Modellwegs.
- **Hub:** der MCP-Hub, ein eigener Prozess je Sitzung. Er ist **nicht** Teil des Halters (Festlegung;
  Projektentscheid, 20.11.2026). Er spricht mit dem Halter nur über die Op `mcp.aufruf` und `schluessel.vorhanden`.
- **Brücke:** der Modellweg, der die Anfrage des Agenten mit Lauf-Schein annimmt (A121), sie prüft und mit `modell.aufruf`
  oder, für eine Umstellung auf WebSocket, mit `modell.strom` an den Halter gibt. Den Schlüssel setzt der Halter ein; die
  Brücke kennt ihn nie. Nach einem Lauf ruft sie `modell.suche` (Vertrag 8, Abschnitt 9a).
- **Oberfläche:** das Programm, in dem der Mensch einen Token eingibt, wechselt oder löscht (`schluessel.setzen`,
  `schluessel.loeschen`).
- **Stelle:** die Entscheidungsstelle. Sie stellt den Einmal-Schein aus und den Menschen-Nachweis.
- **Geheimnis:** ein Schlüssel, Token oder Passwort. Es hat einen **Namen** (Abschnitt 4) und einen **Bereich**: `maschine`
  für `modell.*`, `person` für `anmeldung.*`, `mcp.*` und `mail.*`. Ein Geheimnis im Bereich `person` gehört genau einem
  Konto. Über `schluessel.setzen` gesetzt, trägt es das Pseudonym der Person (`ps_<32 hex>`).
- **Anmeldung:** ein Geheimnis `anmeldung.<ziel>`, der Token einer Person für ein Modellziel, etwa der Token der Copilot
  CLI aus dem Gerätefluss (Nachtrag B05). alt: Modellziele hatten nur `modell.*` im Bereich `maschine`. neu: Ein Modellziel
  nennt in `halter.json` entweder ein `modell.*` oder ein `anmeldung.*`. Grund: Der Token gehört einer Person; als
  `maschine` könnte unter Windows jede Person der Maschine mit ihm arbeiten, und `schluessel.setzen` wäre nach Stufe 13 nie
  möglich.
- **Konto:** das gemessene Konto des Aufrufers, unter Windows die SID, unter Linux und macOS die uid.

Nicht in diesem Vertrag: die Ops der Stelle (`mcp.anfrage`, `mcp.freigeben`, `mcp.ausgefuehrt`), die Form des
Menschen-Nachweises, der Weg Agent → Brücke (A121), Einschluss und WFP (A03, A122), die Ansicht der Freigaben (A105 Punkt 6).

## 2. Plattform

Anfragen, Antworten, Codes, Prüfreihenfolge und die Zusagen aus Abschnitt 8 sind auf Windows, Linux und macOS gleich.
Dieser Abschnitt nennt nur, was je System anders ist: Konto, Ablage, Transport, Peer-Messung und Vorbelegung. Windows ist
das Hauptsystem und hat den vollen Umfang.

### 2.1 Gemeinsam

**Kanal aus `peerkanal`.** Endpunkt, Vorbelegungssperre, Peer-Messung und die Prüfung des Halters durch den Client kommen
aus der gemeinsamen Bibliothek `peerkanal` (Vertrag `rust/spec/peerkanal/`; Hinweis
20.11.2026). Der Halter hat keine eigene Pipe- oder Socket-Prüfung. Er übergibt nur Parameter (Kanalname, zugelassene
Konten, Zahl der Instanzen, Rechte) und entscheidet über die Zulassung aus dem Peer-Wert. Die Mechanismen in den Abschnitten
2.2 bis 2.4 sind darum Anforderungen an `peerkanal`, kein Code des Halters. Der Halter liest diese Felder des Peer-Werts
(Feldnamen wie in `rust/spec/peerkanal/README.md`, Abschnitt „Peer“, Stand vom 20.11.2026):

| Feld | Windows | Linux | macOS | wofür im Halter |
|---|---|---|---|---|
| `sid` | Konto (SID aus `TokenUser` des impersonierten Tokens) | `null` | `null` | Konto und Rolle |
| `uid` | `null` | Konto (`SO_PEERCRED`) | Konto (`LOCAL_PEERCRED`) | Konto |
| `gid` | `null` | Rolle: die effektive gid aus `SO_PEERCRED` ist die gid einer Rollengruppe (2.3) | nicht benutzt | Rolle unter Linux |
| `appcontainer_sid` | muss `null` sein | — | — | Agent abweisen |
| `eingeschraenkt` | muss `false` sein (die Bibliothek weist Restricting SIDs schon selbst ab) | — | — | Agent abweisen |
| `programm` | nicht benutzt (für den Halter `null`, siehe 2.2) | Messwert für das Protokoll, keine Rolle | nicht benutzt | nur unter Linux, nur als Messwert |
| `signatur` | — | — | Rolle: `{kennung, team_id, gueltig}` eines Rollenprogramms, `gueltig` `true` | Rolle unter macOS |
| `sandbox` | — | — | muss `false` sein | Agent abweisen |
| `pid`, `startzeit` | nicht benutzt | Lebensprüfung (pidfd) und Protokoll, keine Rolle | nicht benutzt | — |
| `im_job` | nicht benutzt | nicht benutzt | nicht benutzt | — |

alt (Linux): `gid` nicht benutzt; `programm` gab die Rolle („genau ein Rollenprogramm“); `pid` und `startzeit` nicht benutzt. neu: Die
Rolle folgt aus `gid`; `programm` ist nur noch ein Messwert; pid und Startzeit dienen der Lebensprüfung. Grund (Projektentscheid,
25.11.2026): Ein Rollenprogramm mit `PR_SET_DUMPABLE 0` (der Modellweg schirmt sich so ab) zeigt dem Halter kein `programm`; die gid
übersteht das Abschirmen, weil `SO_PEERCRED` sie beim `connect` festhält.

Die Prozessbindung (pidfd, Audit-Token) leistet `peerkanal` selbst; der Halter verlangt sie, liest aber kein Feld dazu. Die
Server-Prüfung im Client liefert `peerkanal` unter Windows am Besitzer der Pipe-Instanz (gleich der SID von
`NT SERVICE\wb-halter`), nicht über `GetNamedPipeServerProcessId` (A115). Das ist in `peerkanal` vom 20.11.2026 so festgelegt;
OPEN 10 ist damit aufgelöst. Fehlt ein Feld, das der Halter für die Rolle braucht, antwortet er `halter.peer`.

**Rolle.** Aus dem Peer-Wert folgt ein **Konto** und eine **Rolle**. Unter Windows folgt die Rolle aus der SID (2.2),
unter Linux aus der effektiven `gid` (2.3; alt: aus `programm`), unter macOS aus `signatur`. Einen Rückfall auf eine andere
Quelle gibt es nicht. Scheitert die Messung oder passt keine Rolle, antwortet der Halter
`halter.peer` und schließt die Verbindung. Die Peer-Messung ist die zweite Sperre; die erste ist für Schreibaufrufe der
Schein und für `setzen` und `loeschen` der Menschen-Nachweis.

| Rolle | erlaubte Ops |
|---|---|
| `hub` | `schluessel.vorhanden`, `mcp.aufruf` |
| `bruecke` | `schluessel.vorhanden`, `modell.aufruf`, `modell.strom`, `strom.teil`, `strom.ende`, `modell.suche` |
| `oberflaeche` | `schluessel.vorhanden`, `schluessel.setzen`, `schluessel.loeschen` |

alt: `bruecke` durfte `schluessel.vorhanden` und `modell.aufruf`. neu: dazu `modell.strom` mit `strom.teil` und
`strom.ende` sowie `modell.suche` (Nachtrag B05).

`schluessel.vorhanden` mit dem Feld `ziel` ist nur der Rolle `bruecke` erlaubt; `hub` und `oberflaeche` fragen weiter mit
`name`. Eine andere Rolle mit `ziel` bekommt `halter.rolle` (Stufe 7, Nachtrag Socket, vorhanden nach Ziel und Kette).

Ein Geheimnis im Bereich `person` trägt das Konto und das Pseudonym der Person, die es gesetzt hat. `mcp.aufruf` sucht es
nach Name und `person` (Stufe 16). Ein Aufrufer erreicht nur Geheimnisse seines eigenen Kontos (Stufe 17). Ausnahme ist ein
**Vertreter**: unter Windows der Hub unter eigener Dienst-SID (2.2), der für die Person seiner Sitzung handelt. Ein
Geheimnis im Bereich `maschine` setzt der Halter nur bei `modell.aufruf` und `modell.strom` ein, und nur für das Ziel, dem
es in `halter.json` zugeordnet ist. Ein Geheimnis `anmeldung.*` setzt er nur dort ein, und nur wenn es dem Konto des
Aufrufers gehört; er sucht es nach Name und Konto, ohne Pseudonym (Stufen 16 und 17). Eine Brücke als Vertreter gibt es in
Fassung 1 nicht (OPEN 19).

**Gleiche Kennung.** Läuft der Halter unter der Kennung der Person (Linux und macOS in Fassung 1), gilt zusätzlich:
Er startet keinen lokalen MCP-Server mit Token in der Umgebung, weil Prozesse derselben Kennung diese Umgebung lesen
(gemessen am Mac 14.11.: `ps eww` zeigt sie bei jedem Prozess, der kein Plattform-Binary ist, auch nach `unsetenv`;
gemessen unter Linux im Tür-Dienst-Lauf 2: `/proc/<pid>/environ` ist für denselben Nutzer lesbar). `halter.json` darf
dort keinen solchen Server führen; sonst endet der Start mit Exit 78. Remote-Server mit Token, den der Halter beim
Aufruf einsetzt, gehen überall. Der Prüfbefehl des Halters nennt den Zustand „Schlüssel für Prozesse der Person außerhalb des Einschlusses erreichbar“ ausdrücklich. Eine Warnmaske gibt es nicht.

**Start und Exitcodes.** Beim Start prüft der Halter in dieser Reihenfolge. Die erste Verletzung beendet ihn. Er schreibt
genau eine Zeile `<code>: <grund>` (ASCII, höchstens 200 Byte) ins Ereignisprotokoll (Windows), ins Journal (Linux) oder
ins Unified Log (macOS), ohne Pfad, Inhalt oder Geheimnis. Unter Windows ist der Exitcode der dienstspezifische Exitcode
(`dwServiceSpecificExitCode`), sonst der Exitcode des Prozesses.

| Schritt | Prüfung | Exit, Code, Grund |
|---|---|---|
| 0 | nur Linux und macOS: Abschirmung gegen ein Kernabbild (`kernabbild::abschirmen`, siehe unten) | 5, `halter.kernabbild`, `Kernabbild nicht abschirmbar` |
| 1 | Aufruf ohne Argumente außer dem Dienstnamen | 64, `halter.aufruf`, `falscher Aufruf` |
| 2 | `halter.json` lesbar, gültig, geschlossene Form, auf Linux und macOS ohne lokalen Server mit Token, auf Linux root-eigen und für die Person nicht schreibbar (2.3), auf Linux jede Rollengruppe aufgelöst und ohne Konto aus `konten` (2.3a; Nachtrag Rolle aus der Gruppe) | 78, `halter.konfig`, `Konfiguration ungueltig` |
| 3 | Ablage mit den Rechten der Plattform | 78, `halter.ablage_rechte`, `Ablage hat falsche Rechte` |
| 4 | Endpunkt exklusiv angelegt (Pipe oder Socket) | 74, `halter.pipe_belegt`, `Endpunkt schon belegt` |
| jeder | unerwarteter Fehler | 70, `halter.intern`, `interner Fehler` |

Erst nach Schritt 4 ist der Halter bereit. Der Startzeitpunkt (`start_ms`) gilt für Abschnitt 6. Unter Linux und macOS
schirmt sich der Halter als Schritt 0, vor Schritt 1 und vor jedem Lesen, über das Crate `rust/crates/kernabbild` ab (Regel
„Kernabbild-Grenze 1/1“, Vertragsbefund B03 vom 21.11.2026): unter Linux `prctl(PR_SET_DUMPABLE, 0)`, auf beiden Systemen
`RLIMIT_CORE` weich 1 und hart 1; ist die geerbte harte Grenze schon 0, entscheidet das Crate nach `core_pattern`. Bricht das Crate
ab, endet der Halter fail closed mit Exit 5 (`halter.kernabbild`); 5 ist unter Linux und macOS sonst nicht belegt. Läuft er nach
einer Entscheidung des Crates mit Hinweis weiter (harte Grenze 0, Dateimuster oder kein Muster), schreibt er dessen Text als eigene
Zeile in dasselbe Protokoll. Unter macOS setzt er zusätzlich `ptrace(PT_DENY_ATTACH)` und läuft mit Hardened Runtime ohne
`get-task-allow`. Unter Windows gilt Schritt 0 nicht; was dort gegen ein Speicherabbild gilt, entscheidet der Windows-Plan. Bis
22.11.2026 stand hier nur `PR_SET_DUMPABLE` 0 ohne Fehlercode. Einen Halter-Prozess gibt es im Code noch nicht (Stand development
22.11.2026); die Regel gilt für den Bau.

**Prüfung des Halters durch den Client.** Vor der ersten Zeile prüft jeder Client die Gegenseite (Tabelle je Plattform).
Scheitert das, sendet er nichts und meldet `halter.dienst_unecht`.

### 2.2 Windows (voller Umfang, A115, A116)

**Konto und Ablage.**

- Dienstkonto: virtuelles Dienstkonto `NT SERVICE\wb-halter`, ein Dienst je Maschine (Projektentscheid, 18.11.2026). Es
  braucht `SeImpersonatePrivilege` (gemessen, Frage 1: ohne dieses Recht scheitert die Peer-Messung). Es hat keine
  Adminrechte.
- Ablage: `%ProgramData%\agent-workbench\halter\geheim\`, je Geheimnis eine Datei. Besitzer ist die SID des Dienstkontos.
  Die DACL hat genau zwei ACEs, beide nicht vererbt: Dienstkonto Vollzugriff, `SYSTEM` Vollzugriff. Die Vererbung vom
  Elternordner ist abgeschaltet.
- Kein Geheimnis der Werkbank liegt in DPAPI-CurrentUser der Person (gemessen, Frage 3: jeder Prozess der Person
  entschlüsselt es, auch im AppContainer). Gemessen ist ebenso: Eine Datei mit dieser DACL liest die Person nicht (Fehler 5),
  auch ein Administrator ohne Besitzübernahme nicht (Fehler 5); der Dienst liest sie.
- Konfiguration: `%ProgramData%\agent-workbench\halter\halter.json`, Schreibrecht nur Administratoren, Leserecht Dienstkonto.
  Sie nennt Ziele, MCP-Server, Konten der Personen, Rollenprogramme und die öffentlichen Schein-Schlüssel der Stelle. Sie
  enthält kein Geheimnis.
- Die Geheimnisse im Bereich `maschine` legt der Installer (Administrator) als Datei in die Ablage. Über die Pipe lässt sich
  kein `modell.*` setzen oder löschen (`halter.bereich`). Ein lokaler MCP-Server mit Token läuft als Kind des Halters unter
  dem Dienstkonto; der Token steht nur in der Umgebung dieses Kindes, nie in argv (Entwurf, Abschnitt a; A93).

**Pipe (Parameter und Anforderungen an `peerkanal`).**

- Name `\\.\pipe\wb-halter`, Byte-Modus, `PIPE_REJECT_REMOTE_CLIENTS`, höchstens 16 Instanzen. Eine 17. Verbindung bekommt
  vom System `ERROR_PIPE_BUSY` (231).
- Die erste Instanz entsteht mit `FILE_FLAG_FIRST_PIPE_INSTANCE`. Scheitert das, startet der Dienst nicht (Exit 74). Gemessen,
  Frage 1: Ein Vorbeleger lässt den zweiten Server mit Fehler 5 scheitern.
- Sicherheitsbeschreibung jeder Instanz: Besitzer ist die SID des Dienstkontos. DACL: `SYSTEM` und Dienstkonto Vollzugriff;
  je berechtigtes Konto der Personen (aus `halter.json`) Lesen und Schreiben **ohne** `FILE_CREATE_PIPE_INSTANCE`. Keine ACE
  für `Everyone`, `Authenticated Users`, `ALL APPLICATION PACKAGES` oder eine AppContainer-SID. Gemessen, Frage 2: Ein Agent
  im AppContainer öffnet die Halter-Pipe nicht (Fehler 5).
- Client-Prüfung: Besitzer der Pipe-Instanz (`GetSecurityInfo`, `OWNER_SECURITY_INFORMATION`) gleich der SID von
  `NT SERVICE\wb-halter` (sie folgt aus dem Namen: `S-1-5-80-…`). Die Prüfung über `GetNamedPipeServerProcessId` ist
  verworfen (A115: scheitert für die Person mit Fehler 5).

**Peer-Messung (in `peerkanal`).** Die SID kommt aus dem impersonierten Token (gemessen, Frage 1: 100 von 100 richtig).
Einen Bildpfad hat der Halter unter Windows nicht: Als `NT SERVICE\wb-halter` öffnet er den Prozess der Person nicht
(gemessen, win-bfe Teil c, Fehler 5). `programm` und `startzeit` sind für ihn darum `null`.

**Rolle aus der SID (Entscheidung 20.11., peerkanal OPEN 5).** Die Rolle folgt unter Windows allein aus der SID:

| Rolle | Konto des Aufrufers | Bedingung |
|---|---|---|
| `hub` | eigenes virtuelles Dienstkonto `NT SERVICE\wb-hub` (`S-1-5-80-…`) | Der Hub-Dienst startet je Sitzung einen Hub-Prozess unter seinem Konto. Er ist **Vertreter** und nennt in `mcp.aufruf` die Person seiner Sitzung. |
| `bruecke` | Dienst-SID des Modellwegs (`rust/spec/modellweg/`) | Nur Geheimnisse im Bereich `maschine`; ein `anmeldung.*` erst mit der Vertreter-Regel (OPEN 19). |
| `oberflaeche` | SID einer Person aus `halter.json` | `appcontainer_sid` `null`, `eingeschraenkt` `false`. |

Die Erlaubnisliste an `peerkanal` enthält genau diese SIDs, je mit `appcontainer_sid: null`. Ein Agent im AppContainer
derselben Person trägt eine Package-SID und fällt damit schon in `peerkanal` durch (Paarregel).

Gründe:

- Eine Dienst-SID kann ein Prozess der Person nicht annehmen. Ohne Adminrechte startet die Person keinen Prozess unter
  `NT SERVICE\wb-hub` (hergeleitet; Außentest W9 misst es). Die Rolle `hub` ist damit gegen Prozesse der Person getrennt,
  was der Bildpfad nie leistete.
- Variante (b), eine Prozess-DACL für die Halter-SID am Rollenprogramm, ist verworfen. Ein Prozess der Person öffnet jeden
  anderen Prozess derselben Person mit vollen Rechten (Standard-DACL des Nutzers) und kann in das echte Programm Code
  einschleusen. Der Bildpfad bewiese dann nichts gegen Prozesse der Person (hergeleitet).
- Die **Oberfläche** ist ein Prozess der Person. Unter Windows erkennt der Halter sie nur als „Prozess der Person außerhalb
  eines Containers“. Jeder solche Prozess bekommt die Rolle `oberflaeche` und damit nur `schluessel.vorhanden`,
  `schluessel.setzen` und `schluessel.loeschen`. Die Sperre für `setzen` und `loeschen` ist der frische Menschen-Nachweis
  der Stelle (Stufe 27), nicht das Programm. `vorhanden` gibt nur `true` oder `false` für Geheimnisse des eigenen Kontos
  heraus.
- Grenze: Der Hub-Dienst ist Vertreter. Ein fehlerhafter Hub könnte lesende Aufrufe mit dem Token einer anderen Person
  derselben Maschine ausführen. Schreibaufrufe bindet der Schein über den `digest` an die Person, die die Stelle dem Lauf
  zuordnet. Auf einer Maschine mit einer Person (Normalfall des Teams) gibt es keine andere Person.

Was die Windows-VM messen muss: W1 (Rollen aus der SID), W9 (Person kann die Hub-SID nicht annehmen) und W10 (Oberfläche
ohne Nachweis setzt nichts), Abschnitt 11.

### 2.3 Linux (Fassung 1, Projektentscheid, 20.11.2026)

- **Konto:** die Kennung der Person. Ein eigener Systemnutzer `wb-halter` folgt in einer späteren Fassung (OPEN 9; dann
  Socket 0660 mit der Gruppe der Rollenprogramme und die Cgroup der Hub-Unit als weitere Messung). Das Hindernis war die
  Peer-Messung: Ein Halter unter fremder uid liest `/proc/<pid>/exe` der Person nicht, weil proc(5) dafür Zugriff nach
  `PTRACE_MODE_READ` verlangt (hergeleitet). Seit dem Nachtrag Rolle aus der Gruppe hängt die Rolle nicht mehr an
  `/proc/<pid>/exe`; die gid aus `SO_PEERCRED` liest auch ein Halter unter fremder uid. Für Schritt II bleiben Ablage, Übergabe
  der Anmeldung und Suche offen (OPEN 18).
- **Eigener Prozess (Nachtrag B05).** alt: „Halter und Modellweg sind ein Prozess. Der Modellweg liest die Schlüsseldatei
  selbst; `modell.aufruf` ist dort ein Aufruf im Prozess.“ neu: Der Halter ist ein eigener Prozess. Der Modellweg spricht
  ihn als Rolle `bruecke` über den Socket an, wie Hub und Oberfläche. Grund: Der Modellweg parst TLS, HTTP und
  WebSocket-Rahmen aus Bytes des Agenten. Hält er den Token nicht, kann ein Fehler in diesem Teil den Token nicht an den
  Agenten geben (A93, Befund B05). Den Halter startet eine systemd-Nutzer-Unit `wb-halter.service` aus dem deb; Programm
  `/usr/lib/werkbank/werkbank-halter`, Unit und `halter.json` stehen seit dem 23.11. im Paketvertrag (Abschnitte „Dienste“ und
  „halter.json“; alt: „den Pfad und die Unit trägt der Paketvertrag nach“). Betriebsart 1 des Modellwegs (lokale Route) liest ihre Schlüsseldatei bis zu
  einem eigenen Schritt selbst (OPEN 16).
- **Konfiguration:** `/etc/werkbank/halter.json`, `root:root`, 0644, aus dem deb. Liegt sie anders, ist sie für die Person
  schreibbar oder gehört sie nicht root, endet der Start mit Exit 78 (`halter.konfig`). alt: Der Vertrag nannte für Linux
  keinen Ort. Grund: Läge sie bei der Person, lenkte jeder Prozess der Person einen Schlüssel an eine andere Adresse.
- **Ablage:** `<heim>/.local/state/agent-workbench/halter/geheim/`, Ordner 0700, je Geheimnis eine Datei 0600 mit dem Namen
  des Geheimnisses, außerhalb des Einschlusses; nie in bwrap eingebunden. `<heim>` ist das Feld 6 des Eintrags der eigenen
  Kennung in `/etc/passwd`; `HOME` und `XDG_STATE_HOME` gelten nicht. alt (Nachtrag B05): `$XDG_STATE_HOME/agent-workbench/halter/geheim/`
  (Vorgabe `~/.local/state/…`). Grund (Nachtrag Anmelden und Paket): Der Halter läuft als Benutzereinheit mit der Umgebung des
  Nutzer-Managers, `werkbank-anmelden` mit der des Terminals und `postrm` als root; nur der Pfad aus `passwd` ist für alle drei
  derselbe. Ein gesetztes `XDG_STATE_HOME` in der Shell lenkte sonst die Anmeldung an einen Ort, den der Halter nicht liest.
- **Ebenen der Ablage (Schritt 3):** Der Halter öffnet das Heim und jede Ebene (`.local`, `.local/state`, `agent-workbench`,
  `halter`, `geheim`) relativ zur vorigen mit `O_NOFOLLOW`. Jede Ebene ist ein Ordner der eigenen Kennung, für Gruppe und andere
  nicht schreibbar; `geheim` hat genau 0700. Fehlt eine Ebene, legt er sie mit 0700 an, wie `werkbank-anmelden` (Anmelde-Vertrag,
  Abschnitt 2). Verletzt eine Ebene die Regel, endet der Start mit Exit 78 (`halter.ablage_rechte`). alt: „Der Halter prüft
  Eigentümer und Modus jeder Ebene ab dem Heimordner (Schritt 3) und öffnet mit `O_NOFOLLOW`“, ohne Regel für eine fehlende Ebene.
- **Datei der Ablage (Nachtrag Anmelden und Paket):** Inhalt ist der Wert (1 bis 12288 Zeichen) und genau ein LF; die Datei hat höchstens
  12289 Byte. Diese Form gilt für alle Programme der Werkbank, auch für `werkbank-pruefen` (Fund F11; Paketvertrag, Prüfung `copilot`). Der Halter nimmt den Wert ohne das LF;
  eine Datei ohne LF am Ende, mit einem zweiten LF oder mit einem Zeichen außerhalb von `\x21` bis `\x7e` gilt als kein
  Geheimnis (`halter.kein_schluessel`). Er beachtet nur Dateien, deren Name die Form eines Geheimnisses hat (Abschnitt 4); Namen
  mit einem Punkt am Anfang (Temp-Dateien von `werkbank-anmelden`) sind nie ein Geheimnis. Er liest die Datei bei jedem Einsatz
  neu (`O_NOFOLLOW`, regulär, eigene Kennung, 0600), damit ein Tausch mit `werkbank-anmelden copilot --neu` ohne Neustart gilt.
- **Wer die Anmeldung hineinlegt (Nachtrag B05, nur Fassung 1 unter Linux):** `werkbank-anmelden copilot` schreibt den
  Token direkt als Datei `anmeldung.copilot` in die Ablage, mit den Schritten seines Vertrags (Temp-Datei mit `O_EXCL` und
  0600, fsync, Tausch; `rust/spec/anmelden/README.md`, Abschnitte 2, 3 und 3b). Der Anmelde-Vertrag führt den Pfad seit dem
  Nachtrag Halter vom 23.11. (alt: „Den Pfad trägt der Anmelde-Vertrag nach“). Grund:
  Die Ablage gehört derselben uid wie `werkbank-anmelden`; der Weg über `schluessel.setzen` gäbe hier keinen Schutz, den
  die Datei nicht schon hat. Ab dem Systemnutzer gilt `schluessel.setzen` mit Menschen-Nachweis (OPEN 9).
- **Grenze der Fassung 1:** Jeder Prozess der Person außerhalb des Einschlusses liest die Ablage. Den Speicher des
  Halters liest er nicht (Schritt 0, `PR_SET_DUMPABLE` 0; Außentest L5). Weil die Ablage im Heim liegt, muss der Einschluss
  sie sperren, auch wenn ein Pfad einer Sitzung (Projektwurzel, Programmordner) darüber liegt (Abschnitt 10). Gegenüber dem Stand vor B05 (Datei 0600
  `/var/lib/werkbank/<nutzer>/copilot.schluessel`, gelesen von drei Programmen) gewinnt Fassung 1 die Trennung vom
  Modellweg, nicht den Schutz gegen Prozesse der Person. Der Prüfbefehl des Halters nennt diesen Zustand wie in 2.1.
- **Socket (über `peerkanal`):** Unix-Socket (Stream) `/run/user/<uid>/wb-halter/halter.sock` (`<uid>` die eigene, aus `getuid`),
  Ordner 0700, Socket 0600. Der Pfad ist fest: Der Halter liest `XDG_RUNTIME_DIR` nicht, und eine gesetzte Umgebung verlegt den
  Socket nie (Nachtrag Socket, vorhanden nach Ziel und Kette; Fund F17). alt: `$XDG_RUNTIME_DIR/wb-halter/halter.sock`, ohne
  absoluten Wert `/run/user/<uid>`. Grund: Eine fremd gesetzte Umgebung der Einheit oder des Starts lenkte sonst den Halter an
  einen Ort, den der Client nicht prüft. Unter der Benutzereinheit ist `XDG_RUNTIME_DIR` ohnehin `/run/user/<uid>` (hergeleitet aus
  systemd, `pam_systemd`); die Einheit braucht darum keine Änderung. Nur der Prüfbau kennt die Naht `WB_HALTER_LAUFZEIT` hinter
  `WB_TESTUMGEBUNG=1`. Test im Bau: `start::tests::xdg_runtime_dir_verlegt_den_socket_nie` (das Testprogramm startet sich mit
  `XDG_RUNTIME_DIR` auf einem anderen Ordner und erwartet `/run/user/<uid>`). Liegt am Pfad schon
  etwas, endet der Halter mit Exit 74 und verändert nichts. **Ausnahme toter Socket (Fund F2, Projektentscheid, 25.11.2026):** Ist der
  Eintrag ein Socket der eigenen uid im eigenen Ordner 0700 und weist `connect` ihn mit `ECONNREFUSED` ab (Rest eines Halters, den
  `SIGKILL` oder der OOM-Killer beendet hat), entfernt der Halter ihn mit `unlinkat` relativ zum Ordner, nur solange Gerät und Inode
  noch die angesehenen sind, und startet weiter. Ein lebender Halter (die Verbindung gelingt), eine Datei, ein Verweis und ein Socket
  einer anderen uid bleiben unverändert und ergeben 74. alt: „Liegt am Pfad schon etwas, endet der Halter mit Exit 74 und verändert
  nichts“ ohne Ausnahme. Grund: Nach einem harten Ende blieb der Socket liegen; der Neustart endete mit 74, und die Einheit startete
  wegen `RestartPreventExitStatus=… 74 …` nie wieder. Die Einheit bleibt unverändert (74 startet weiter nicht im Kreis neu).
- **Peer (über `peerkanal`; Nachtrag Rolle aus der Gruppe):** `uid` gleich der eigenen und in `konten`; die Rolle folgt aus der
  **effektiven gid** des Peers beim `connect` (`SO_PEERCRED`, Feld `gid`): Ist sie die gid einer Rollengruppe aus `halter.json`
  (2.3a), hat der Peer deren Rolle; sonst antwortet der Halter `halter.peer` und schließt. `bruecke` ist die Gruppe `wb-bruecke`;
  ihr Rollenprogramm ist `/usr/lib/werkbank/werkbank-modellweg` mit `root:wb-bruecke` und Modus 2755 (setgid, Paketvertrag,
  Nachtrag Rolle aus der Gruppe). `programm` (`/proc/<pid>/exe`) misst der Halter weiter, aber nur als Messwert für ein Protokoll; es
  entscheidet keine Rolle. Einen Rückfall auf `programm`, die pid oder eine supplementäre Gruppe gibt es nicht (R-LAUF-HALTER).
  alt: „die Rolle folgt aus `programm` (`/proc/<pid>/exe` über die pidfd): genau ein Rollenprogramm unter `/usr` oder `/opt` (root-eigen,
  für die Person nicht schreibbar); […] Ist `programm` `null`, antwortet der Halter `halter.peer`.“ Grund: Der Modellweg setzt
  `PR_SET_DUMPABLE 0` (Vertrag 8, Schritt 1); dann ist `programm` `null`, und die Brücke bekam nie die Rolle (gemessen am Bau vom
  25.11.2026: derselbe Peer bekam mit Dumpable 1 die Rolle, mit Dumpable 0 `halter.peer`). `PR_SET_DUMPABLE 0` bleibt im Modellweg.
  Warum die gid trägt (hergeleitet aus credentials(7), execve(2) und unix(7)): Eine effektive gid außerhalb der eigenen Gruppen
  bekommt ein Prozess ohne `CAP_SETGID` nur durch `execve` eines setgid-Programms dieser Gruppe; eine Datei mit dieser Gruppe und
  setgid kann nur root oder ein Mitglied der Gruppe anlegen. Darum darf kein Konto aus `konten` Mitglied sein (2.3a). Ein
  setgid-Programm läuft mit `AT_SECURE`, ist ohne Dumpable und für Prozesse derselben uid nicht über `ptrace` erreichbar, weil
  die gid abweicht (hergeleitet aus ptrace(2), „Ptrace access mode checking“). Die Rolle trennt damit das Rollenprogramm von
  anderen Prozessen der Person, solange das Programm selbst keinem Aufrufer folgt: **Das Rollenprogramm muss im sicheren Modus
  laufen** und nichts aus Umgebung, Argumenten oder Dateien des Aufrufers als vertraut nehmen, das seine Anfragen an den Halter
  lenkt (Haltung von `secure_getenv`; die Liste der Stellen für den Modellweg gehört Vertrag 8). Ein Kind des Rollenprogramms erbt
  die gid; es darf darum kein Programm des Aufrufers mit dieser gid starten.
- **Fenster der pid (Nachtrag Rolle aus der Gruppe).** Die pidfd entsteht mit `pidfd_open` auf die pid aus `SO_PEERCRED`
  (`SO_PEERPIDFD` fehlt in der Abhängigkeit, OPEN 3). Zwischen `connect` und `pidfd_open` kann der Peer enden und seine pid neu
  vergeben werden. Weil die Rolle aus der gid kommt, die der Kernel beim `connect` festhält, betrifft das Fenster nur die
  Lebensprüfung (`Peer::lebt`) und die Messwerte `programm`, `pid` und `startzeit` im Protokoll, nie die Rolle (hergeleitet).
- **Client-Prüfung (peerkanal OPEN 6):** Der Halter setzt `PR_SET_DUMPABLE 0`; dann liest der Client `/proc/<pid>/exe` des
  Halters nicht. Der Client erkennt den Halter darum nur am Socket im Ordner 0700 außerhalb des Einschlusses und an der uid
  (`uid` des Servers gleich der eigenen, Ordner und Socket der eigenen uid mit 0700 und 0600). Eine Trennung durch die uid
  sagt der Vertrag unter Linux nicht zu, wie `peerkanal` unter Unix. Die Sperre gegen den Agenten ist allein, dass der
  Einschluss den Socket-Ordner nie einbindet (Abschnitt 10, Außentest L1 mit Gegenprobe).
- **Kette (Nachtrag Rolle aus der Gruppe):** Die Kette des Halters (Abschnitt 9) liegt in Schritt I in
  `<heim>/.local/state/agent-workbench/halter/protokoll/`, Ordner 0700 der eigenen Kennung, neben `geheim/` und unter dem Ordner,
  den der Einschluss sperrt (Abschnitt 10). `<heim>` ist das Heim aus `passwd` wie bei der Ablage. Fehlt der Ordner, legt der Halter
  ihn mit 0700 an; hat er eine andere Art, einen anderen Eigentümer oder einen anderen Modus, schreibt der Halter keine Kettenzeile
  und startet trotzdem. Lässt sich eine Zeile nicht anhängen (Fund F16), schreibt der Halter beim ersten Fehler des Laufs genau eine
  feste Zeile ins Journal, ohne Pfad und Inhalt: `halter.kette_anker: Anker der Kette beschaedigt` bei einem unlesbaren oder nicht
  passenden Anker (`kette.anker_falsch` aus Vertrag 5), sonst `halter.kette: Kette nicht fortgeschrieben`; die Anfrage beantwortet er
  unverändert. alt: Ein Fehler der Kette blieb still. `apt purge` löscht `protokoll/` mit der Ablage (Paketvertrag, Abschnitt
  „Entfernen“). In Schritt II liegt die Kette unter `/var/lib/wb-halter/` (Paketvertrag, Teil A2). alt: Der Vertrag nannte
  für Linux keinen Ort. Grund: Der Bau brauchte einen Ort; die Leitung hat den gebauten bestätigt (Projektentscheid, 25.11.2026).

### 2.3a Form von `halter.json` und Rollengruppen unter Linux (Nachtrag Rolle aus der Gruppe)

Das Schema ist `$defs/konfig` in `halter.schema.json`; die Fälle der Form `konfig` prüfen es samt den Regeln dieses Abschnitts,
die kein Schema ausdrücken kann. Jeder Verstoß endet den Start in Schritt 2 mit Exit 78 und der Zeile
`halter.konfig: Konfiguration ungueltig`. Alles hier gilt für Linux in Fassung 1; Windows und macOS bekommen ihre Form mit ihrem Bau.
alt: Die Form hatte kein Schema (OPEN 21 b). neu: die Form, die der Bau vom 25.11.2026 liest, mit `rollen` als Gruppen statt Pfaden.

**Datei und Ordner:** `/etc/werkbank/halter.json`, mit `O_NOFOLLOW` geöffnet, reguläre Datei, Eigentümer uid 0, für Gruppe und andere
nicht schreibbar (0644 oder enger); der Ordner `/etc/werkbank` ist kein Verweis, gehört uid 0 und ist für Gruppe und andere nicht
schreibbar. Höchstens 1048576 Byte. Strenges JSON: UTF-8 ohne BOM, kein doppelter Schlüssel auf keiner Ebene, kein Rest nach dem
Wert. Die Datei muss nicht kanonisch sein.

| Feld | Typ | Grenzen |
|---|---|---|
| `fassung` | Ganzzahl | genau `1` |
| `konten` | Objekt | Schlüssel: uid als Dezimaltext (1 bis 10 Ziffern ohne führende Null außer `0`, Wert höchstens 4294967295); Wert: Objekt mit genau `person`. Die eigene uid des Halters muss darin stehen. Nur Peers mit einer uid aus `konten` bekommen eine Rolle. |
| `konten.<uid>.person` | Text oder `null` | `^ps_[0-9a-f]{32}$`; `null`, wenn kein Pseudonym bekannt ist (Schritt I) |
| `rollen` | Objekt | Schlüssel nur `hub`, `bruecke`, `oberflaeche`; Wert: der Name einer Gruppe, `^[a-z_][a-z0-9_-]{0,31}$`. Zwei Rollen haben nie dieselbe Gruppe. Leeres Objekt erlaubt (dann bekommt niemand eine Rolle). alt: Wert war eine Liste von Pfaden der Rollenprogramme. |
| `ziele` | Objekt | Schlüssel: Kennung; Wert: Ziel (unten). Leeres Objekt erlaubt. |
| `server` | Objekt | Schlüssel: Kennung; Wert: Server (unten). Leeres Objekt erlaubt; in Schritt I leer. |
| `kids` | Objekt | Schlüssel: Kennung; Wert: Base64 mit Auffüllung von genau 32 Byte (öffentlicher Ed25519-Schlüssel der Stelle). In Schritt I leer. |

**Ziel** (`ziele.<kennung>`), acht Pflichtfelder und ein optionales neuntes (`erststart_antwortform`, unten): `host` (1 bis 253 Zeichen aus `[A-Za-z0-9.:-]`), `port` (1 bis 65535),
`tls` (`false` nur für `127.0.0.1`, `::1` oder `localhost`), `praefix` (`""` oder ein Pfad nach `^/[A-Za-z0-9._~/-]{0,511}$` ohne `/`
am Ende und ohne die unsicheren Teile der Pfadprüfung aus Abschnitt 4), `kopf` (`^[a-z0-9-]{1,64}$`; `authorization`, `x-api-key`,
`api-key` oder ein Name, der nicht auf der Liste der verbotenen Kopfnamen steht), `vorsatz` (höchstens 64 Zeichen `\x20` bis `\x7e`),
`schluessel` (Name der Art `modell` oder `anmeldung`) und `pfade` (1 bis 32 Regeln nach Abschnitt 4, je höchstens 512 Zeichen nach der
Methode). Optional `erststart_antwortform`, der Schalter des Erststarts (Abschnitt 4, „Antwortform im Erststart“): 1 bis 8 Regeln
`<METHODE> <pfad>` mit genauem Pfad (kein `*`), ohne Doppel, und jede muss von einer Regel aus `pfade` desselben Ziels gedeckt sein.
Sonst Exit 78. Der Schalter öffnet nie einen Pfad. Eine leere Liste ist ungültig; ohne Schalter fehlt das Feld. Am Server gibt es das
Feld nicht. alt: genau acht Felder (Nachtrag Erststart).

**Server** (`server.<kennung>`), genau zehn Felder, alle Pflicht: `host`, `port`, `tls`, `kopf`, `vorsatz` wie beim Ziel; `pfad` (Form
wie `praefix`, nicht leer), `schluessel` (Name der Art `mcp`), `freigabe` (bool), `werkzeuge` (Werkzeugname auf `lesend` oder
`schreibend`) und `lokal` (bool). `lokal: true` ist unter Linux immer Exit 78 (Abschnitt 2.1, Außentest L4).

**Rollengruppen beim Start (Schritt 2).** Der Halter löst jede Gruppe aus `rollen` in `/etc/group` auf (nie über NSS) und hält danach
nur die Zuordnung gid → Rolle. Exit 78, wenn für eine Rolle gilt:

1. Die Gruppe steht nicht genau einmal in `/etc/group`, oder ihre gid ist keine Zahl oder ist 0.
2. Zwei Rollen lösen auf dieselbe gid auf.
3. Die Gruppe hat Mitglieder (viertes Feld nicht leer).
4. Das Passwortfeld ist nicht `x`, `!`, `*` oder leer (ein Gruppenpasswort ließe `newgrp` ohne Mitgliedschaft zu; `/etc/gshadow`
   liest der Halter nicht, das prüft `werkbank-pruefen`).
5. Die gid ist die Hauptgruppe (Feld 4 in `/etc/passwd`) eines Kontos aus `konten`.

alt: Die Rollenprogramme wurden zur Laufzeit an Pfad, Eigentümer und Modus geprüft (`halter.peer`, nicht 78). neu: Eine falsche
Rollengruppe ist ein Fehler der Konfiguration (78), denn mit ihr hätte jeder Prozess eines Kontos die Rolle. Ein Konto aus `konten` als
Mitglied einer Rollengruppe ist verboten (Projektentscheid, 25.11.2026); `werkbank-pruefen` meldet es als Befund (Paketvertrag).

**Beispiel Schritt I** (uid 1000; Host und Pfadregeln kommen im Paket aus den `netzziele` der Vorlage, und der Name eines Ziels
ist im Paket sein Host, siehe Paketvertrag, `halter_ziel`):

```json
{"fassung": 1,
 "konten": {"1000": {"person": null}},
 "rollen": {"bruecke": "wb-bruecke"},
 "ziele": {"api.githubcopilot.example": {"host": "api.githubcopilot.example", "port": 443, "tls": true, "praefix": "",
                                         "kopf": "authorization", "vorsatz": "Bearer ", "schluessel": "anmeldung.copilot",
                                         "pfade": ["GET /copilot_internal/*", "POST /chat/completions"]}},
 "server": {},
 "kids": {}}
```

### 2.4 macOS (Fassung 1)

Vorlage für die Zusagen ist `the previous implementation` des alten Stacks (Schlüssel nur im eigenen Prozess, nie in Umgebung,
argv, Datei, Log oder Fehlersatz; Peer-Prüfung über libproc). Neu ist der Unix-Socket statt Loopback-TCP und der
Schlüsselbund-Eintrag mit Zugriffsliste nur für den Halter.

- **Dienst:** launchd-Agent der Person (`~/Library/LaunchAgents/…`, Sitzung `Aqua`), weil der Anmeldeschlüsselbund nur in der
  Anmeldesitzung der Person offen ist. Ein launchd-Daemon unter eigenem Konto `_wbhalter` mit eigenem Schlüsselbund braucht
  Adminrechte beim Einrichten und folgt in einer späteren Fassung (OPEN 9). Das Halterprogramm ist mit Developer ID
  signiert, notarisiert, mit Hardened Runtime und ohne `get-task-allow`.
- **Ablage:** je Geheimnis ein Eintrag `kSecClassGenericPassword` im Anmeldeschlüsselbund, Dienst `agent-workbench.halter`,
  Konto gleich dem Namen des Geheimnisses. Der Halter legt den Eintrag selbst an (`SecItemAdd` aus seinem Prozess), nie über
  `/usr/bin/security`. Die Zugriffsliste nennt als vertrauenswürdige Anwendung nur das Halterprogramm; die Partitionsliste
  enthält nur `teamid:<Team-ID des Halters>`, nicht `apple-tool:` und nicht `apple:`. Grund: Gemessen am 14.11. liest ein
  Prozess derselben Kennung einen mit `security` angelegten Eintrag per `security find-generic-password -w` ohne Rückfrage.
  Dass die Zugriffsliste das sperrt, ist hergeleitet und nicht gemessen (Außentest M1).
- **Socket (über `peerkanal`):** Unix-Socket (Stream) `~/Library/Application Support/agent-workbench/halter/halter.sock`, Ordner 0700, Socket
  0600. Ist der Pfad länger als 103 Byte oder liegt dort schon etwas, endet der Halter mit Exit 74 und verändert nichts.
- **Peer (über `peerkanal`):** `uid` gleich der eigenen (`LOCAL_PEERCRED`); die Rolle folgt aus `signatur`
  (`kennung` und `team_id` eines Rollenprogramms, `gueltig` `true`; gemessen am Audit-Token, also an genau diesem Prozess).
  `sandbox` `true` ist immer `halter.peer`, auch mit gültiger Signatur. Hardened Runtime ohne `get-task-allow` verhindert,
  dass ein anderer Prozess derselben uid in ein Rollenprogramm Code einschleust (hergeleitet; Außentest M3).
- **Client-Prüfung:** Ordner und Socket gehören der eigenen uid mit den Modi 0700 und 0600; `LOCAL_PEERTOKEN` des Halters
  besteht die Anforderung des Halterprogramms.
- **Grenze der Fassung 1:** Der Agent im Einschluss erreicht weder Socket noch Schlüsselbund (Annahmen in Abschnitt 10). Ein
  Prozess der Person außerhalb des Einschlusses kann eine Rückfrage des Schlüsselbunds auslösen, die ein Mensch erlauben
  könnte. Erst der Daemon unter eigenem Konto schließt diese Lücke.
- **Anmeldung unter macOS (Nachtrag B05):** Ein Geheimnis `anmeldung.*` hält der Halter unter macOS erst als Daemon unter
  eigenem Konto (OPEN 9). Grund: Grenze M1 vor Schritt II (Projektentscheid, 21.11.2026) verlangt, dass nur ein Halter unter eigenem
  Dienstkonto ein Geheimnis hält (A144); Messlauf 2 macOS zeigt Umgebung und Prozessdaten derselben uid aus der Sandbox
  lesbar. Bis dahin arbeitet keine Person mit macOS mit einem solchen Token vor Fassung 2.

| | Windows | Linux Fassung 1 | macOS Fassung 1 |
|---|---|---|---|
| Konto | `NT SERVICE\wb-halter` | Person | Person (launchd-Agent) |
| Ablage | Datei, DACL Dienstkonto und `SYSTEM` | Datei 0600 im Ordner 0700, außerhalb des Einschlusses | Schlüsselbund, Zugriffsliste nur Halter, Partition nur Team-ID |
| Endpunkt | `\\.\pipe\wb-halter` | Unix-Socket 0600 im Ordner 0700 | Unix-Socket 0600 im Ordner 0700 |
| Konto des Aufrufers | `sid` | `uid` | `uid` |
| Rolle | SID: Hub-Dienst, Modellweg-Dienst, Person | effektive `gid` einer Rollengruppe (alt: `programm`) | `signatur` |
| Client erkennt den Halter | Besitzer der Pipe-Instanz | Socket im Ordner 0700 und `uid` | Socket im Ordner 0700, `uid` und Signatur |
| Agent ausgeschlossen durch | AppContainer ohne ACE an der Pipe | Socket nicht im Einschluss | Seatbelt-Profil (Abschnitt 10), Sandbox-Prüfung |
| Vorbelegung | `FILE_FLAG_FIRST_PIPE_INSTANCE` | Pfad belegt: Exit 74 | Pfad belegt: Exit 74 |
| lokaler MCP-Server mit Token | ja, unter dem Dienstkonto | nein | nein |

## 3. Transport

- Endpunkt je Plattform: Abschnitt 2. Die Zeilen sind auf Pipe und Socket gleich.
- Je Anfrage eine Zeile UTF-8 ohne BOM, abgeschlossen mit LF. Je Antwort eine Zeile, abgeschlossen mit LF. Das LF gehört
  nicht zur Zeile.
- Eine Zeile hat höchstens 33554432 Byte (32 MiB) ohne LF. Eine längere Zeile beantwortet der Halter mit
  `halter.zu_gross`, liest den Rest nicht und schließt die Verbindung.
- **Bytes in RFC 8785 in beide Richtungen.** Eine Anfrage, deren Bytes nicht genau die kanonische Form ihres Werts sind,
  ist ungültig (`halter.nicht_kanonisch`). Damit sind auch Leerraum, Schlüsselordnung, unnötige Escapes und `1.0` statt `1`
  ungültig. Der Halter schreibt jede Antwort kanonisch. Kanonisch schreibt die Bibliothek `formate` (`kette::kanonisch`,
  Vertrag 5).
- Auf einer Verbindung bearbeitet der Halter eine Anfrage nach der anderen; die Antworten kommen in dieser Reihenfolge.
  Mehrere Verbindungen laufen nebeneinander. Ausnahme ist ein offener Strom (`modell.strom` nach `beginn` mit Status 101):
  Dann gehört die Verbindung bis zu seinem Ende nur diesem Strom (Abschnitt 4).
- Der Halter nimmt mindestens 32 gleichzeitige Verbindungen der Rolle `bruecke` an, zusätzlich zu Hub und Oberfläche
  (Nachtrag B05; Vertrag 8 lässt in Betriebsart 1 8 gleichzeitige Anfragen und in Betriebsart 2 32 offene Tunnel zu,
  Projektentscheid 24.11.2026). Grund: Jeder offene Tunnel eines gebundenen Ziels kann eine Verbindung zum Halter halten. Unter Windows legt die Zahl der
  Pipe-Instanzen der Windows-Schritt fest (OPEN 19).
- Fristen: Die erste vollständige Zeile muss binnen 10 s nach dem Verbinden ankommen, jede weitere binnen 300 s nach der
  letzten Antwort. Sonst schließt der Halter die Verbindung ohne Antwort. Solange ein Aufruf läuft oder ein Strom offen ist,
  gilt die 300-s-Frist nicht.
- **Fristen zum Ziel (Nachtrag B05).** Verbindung und TLS binnen 10 s, der Kopf der Antwort binnen 600 s nach dem Senden,
  danach keine Frist, solange Bytes fließen. Gleiche Werte wie Vertrag 8. Wird eine Frist überschritten, gilt
  `halter.ziel_fehler`. alt: Stufe 28 nannte „Frist verstrichen“ ohne Zahl.
- Es gibt keine Op, die einen Wert eines Geheimnisses liefert. Ein unbekanntes `op` ist `halter.op_unbekannt`.

## 4. Anfragen

Jede Anfrage hat `fassung` (genau `1`), `op` und `id`. `id` ist eine Kennung oder eine ganze Zahl von 0 bis
9007199254740991; sie kommt unverändert in jeder Antwortzeile zurück und geht nie ins Protokoll. Jede Form ist geschlossen:
Ein unbekannter Schlüssel macht die Anfrage ungültig, auf jeder Ebene außer in `argumente` und in `kopf`.

**Name eines Geheimnisses:** höchstens 64 Zeichen, Form `^(modell|anmeldung|mcp|mail)\.[a-z0-9][a-z0-9._-]*$`. Der Teil vor
dem ersten Punkt ist die Art. `modell.<ziel>` gehört zum Bereich `maschine`, `anmeldung.<ziel>`, `mcp.<server>` und
`mail.<konto>` zum Bereich `person`. alt: Art `anmeldung` gab es nicht (Nachtrag B05).

**Ziel in `halter.json` (Modellziele, Nachtrag B05):** je Ziel `schluessel` (ein Name der Art `modell` oder `anmeldung`)
und `pfade`, eine Liste von 1 bis 32 Regeln `<METHODE> <pfad>`. Die Methode ist `GET` oder `POST`. Der Pfad ist ein genauer
Pfad, ein Teilbaum mit `/*` am Ende oder `/*` (jeder Pfad); Segmente aus `[A-Za-z0-9._~-]`, kein Segment `.` oder `..`,
`*` nur als letztes Segment. Eine Regel passt wie in Vertrag 8 (Abschnitt 13.2): Sie ist `/*`, oder sie endet auf `/*` und
der Pfad beginnt mit dem Teil davor samt `/`, oder sie ist genau der Pfad. Ein genauer Pfad deckt keinen Unterpfad. alt: nur
genaue Pfade. Grund: Die Route der Copilot CLI nutzt `/*` an `api.individual.githubcopilot.com`. Beide Listen, `pfade` hier und
`regeln` in der Zielliste von Vertrag 8, entstehen aus den `netzziele` der Einstellungsdatei; ein Unterschied ist ein Befund
von `werkbank-pruefen`. Der Modellweg prüft zuerst, der Halter als letzte Sperre noch einmal.

**Abgeleitete Tokens (A93, Entscheid der Leitung, 25.11.2026).** Ein abgeleitetes Token, das ein Ziel gegen den gehaltenen
Schlüssel ausgibt, fällt unter A93 wie der Schlüssel selbst. Ein Pfad, der ein Token ausgibt, steht darum nie in einer Regel
eines gebundenen Ziels, weder genau noch in einem Teilbaum. Für `api.github.com` gelten nur die genauen Pfade
`GET /copilot_internal/user` und `GET /copilot/mcp_registry`, kein Teilbaum; `GET /copilot_internal/v2/token` endet so mit
`halter.pfad_verboten` (Stufe 15), bevor etwas das Ziel erreicht (fail closed). Grund: Die Leck-Sperre (Abschnitt 8) kennt nur
die gehaltenen Werte, nicht ein Token, das erst das Ziel erzeugt. alt: `GET /copilot_internal/*` an `api.github.com`. Fälle:
`s15-abgeleitetes-token` (`GET /copilot_internal/v2/token`, `halter.pfad_verboten`), Gegenproben
`a-modell-genauer-pfad-user` und `a-modell-genauer-pfad-mcp-registry` (gültig).

**Modell-Host nur mit genauen Pfaden (Nachtrag N1, Entscheid der Leitung, 25.11.2026).** Dieselbe Zusage gilt für den Wert von
`endpoints.api` (`api.individual.githubcopilot.com`): Dort stehen nur die gemessenen genauen Pfade `GET /models`, `GET /responses`,
`POST /auto`, `POST /models/session`, `POST /models/session/intent`, `POST /chat/completions` und `POST /agents/sessions`
(Liste mit Herkunft im Einstellungsvertrag, `rust/spec/einstellungen/README.md`, Nachtrag N1). alt: `GET /*` und `POST /*`; ein
Teilbaum kann die Zusage nicht halten, weil die Endpunkte des Hosts nicht gemessen sind. Jeden anderen Pfad lehnt der Halter mit
`halter.pfad_verboten` ab (Stufe 15) und schreibt eine Kettenzeile mit Methode und Pfad, nie Inhalt (Abschnitt 9). **Die Leitung
entscheidet beim ersten Start jede fehlende Regel einzeln anhand der Kette.** Fälle: `s15-modell-host-ausserhalb`
(`POST /v1/fremd`, `halter.pfad_verboten`), `s15-modell-host-unterpfad` (`GET /models/x`, `halter.pfad_verboten`), Gegenproben
`a-modell-host-chat` (`POST /chat/completions`) und `a-modell-host-modelle` (`GET /models`), beide gültig.

**Erststart-Profil (Nachtrag Erststart, Entscheid der Leitung, 25.11.2026).** Beim ersten echten Start mit Token gelten am
Modell-Host zuerst nur `GET /models` und `POST /chat/completions`, mit fest gewähltem Modell (kein `auto`;
`rust/spec/einstellungen/README.md`, Abschnitt „Erststart-Profil“). Grund: „gemessen“ heißt bei den sieben Pfaden oben, dass die
CLI sie ruft, nicht, dass ihre Antwort kein Token trägt. Die übrigen fünf (`GET /responses`, `POST /auto`, `POST /models/session`,
`POST /models/session/intent`, `POST /agents/sessions`) sind beim ersten Start nicht freigeschaltet; die Anfragen der CLI dorthin
enden mit `halter.pfad_verboten` (Stufe 15) und stehen mit Methode und Pfad in der Kette (Abschnitt 9). Fälle:
`s15-erststart-responses` (`modell.strom`), `s15-erststart-auto`, `s15-erststart-models-session`,
`s15-erststart-models-session-intent` und `s15-erststart-agents-sessions` (je `halter.pfad_verboten`), Gegenproben
`a-erststart-modelle` und `a-erststart-chat` (gültig). Test im Bau: `tests/dienst.rs`
`erststart_sperrt_die_fuenf_pfade_mit_kettenzeile` (fünf Kettenzeilen mit Op, Methode und Pfad, die Attrappe bekommt nichts, die
Kette ist ganz; die zwei freien Pfade erreichen das Ziel).

**Antwortform im Erststart (Nachtrag Erststart, Entscheid der Leitung, 25.11.2026).** Für jeden weiteren Pfad misst der Halter
vor der Freigabe die Form der Antwort. Die Diagnose gibt es nur hinter dem ausdrücklichen Schalter des Erststarts, nie im
Normalbetrieb: dem Feld `erststart_antwortform` eines Ziels in `halter.json` (Abschnitt 2.3a). Fehlt das Feld, schreibt der Halter
keine Diagnose, im ausgelieferten Bau wie im Prüfbau; der Prüfbau hat dafür keine eigene Naht. Wer den Schalter setzt, braucht
Schreibrecht an `/etc/werkbank/halter.json` (uid 0); die Sitzung des Agenten erreicht die Datei nicht. Für den Normalbetrieb fehlt
das Feld; `werkbank-pruefen` soll ein gesetztes Feld als Befund melden (OPEN 24).

- **Wann.** Nur bei `modell.aufruf`, wenn `<methode> <pfad>` der Anfrage genau gleich einer Regel des Schalters ist, und nur
  nach einem vollständigen Körper (`ende` ohne Code). Ein Leck-Treffer, ein Fehler des Ziels oder ein abgebrochener Körper geben
  keine Diagnose. `modell.strom` hat keine Diagnose (OPEN 25). Die Pfadprüfung (Stufe 15) läuft unverändert vorher: Der Schalter
  macht keinen Pfad frei, die Regel muss in `pfade` stehen.
- **Die Antwort geht weiter wie bisher.** Der Halter liest nur eine Kopie des Körpers; die Antwort selbst geht durch die
  Leck-Sperre (Abschnitt 8) an die Brücke wie ohne Schalter. Ein abgeleitetes Token, das die Leck-Sperre nicht kennt, erreicht
  damit im Messlauf die Brücke (OPEN 24).
- **Namen.** Der Halter liest die Kopie als JSON und nimmt jeden Schlüssel eines Objekts, rekursiv. Pfadform: ein Schlüssel `k`
  auf der obersten Ebene heißt `k`, unter einem Namen `p` heißt er `p.k`; die Elemente einer Liste unter `p` stehen unter `p[]`
  (eine Liste auf der obersten Ebene unter `[]`). Beispiel: `{"data":[{"id":"x"}],"access_token":"y"}` ergibt `access_token`,
  `data` und `data[].id`. Die Liste ist nach Bytes sortiert, jeder Name einmal. Nie ein Wert, keine Länge, kein Typ.
- **Grenzen.** Höchstens 1048576 Byte Körper (darüber: Form `zu_gross`, keine Namen), höchstens 256 Namen, je Name höchstens
  256 Byte, höchstens 32 Ebenen. Ein längerer Name bleibt weg; eine Überschreitung setzt `abgeschnitten`.
- **Regel der Namen.** Enthält ein Name `token`, `secret`, `key`, `session` oder `credential` (ohne Groß- und Kleinschreibung,
  auch als Teilwort, etwa `access_token`, `apiKey`, `monkey`), markiert die Zeile den Pfad mit `"zu":true`: Er bleibt zu, bis die
  Leitung entscheidet. Ebenso `"zu":true` bei jeder Form außer `json`, bei `abgeschnitten` und bei einem gehaltenen Geheimnis in
  einem Namen. Der Halter ändert dabei keine Regel; die Markierung ist ein Befund für die Leitung.
- **Gehaltenes Geheimnis als Name.** Steht ein gehaltenes Geheimnis in einer der Formen der Leck-Sperre in einem Namen, nennt die
  Zeile keine Namen und trägt die Form `geheimnis`.
- **Zeile.** Genau eine Zeile im Journal (stderr der Einheit): `halter.antwortform: ` und das kanonische JSON-Objekt (RFC 8785)
  mit genau `abgeschnitten` (bool), `form` (`json`, `kein_json`, `zu_gross` oder `geheimnis`), `methode`, `namen` (Liste),
  `pfad` (das Feld `pfad` der Anfrage), `status` (HTTP-Status des Ziels), `ziel` (Name aus `halter.json`) und `zu` (bool). Nie
  `abfrage`, ein Kopf, ein Wert, `id`, `lauf` oder eine Person. Ein Zeilenumbruch in einem Namen ist im JSON maskiert; die Zeile
  bleibt eine Zeile. Die Zeile steht nicht in der Kette; es gibt keine neue Art im Register von Vertrag 5.

```
halter.antwortform: {"abgeschnitten":false,"form":"json","methode":"POST","namen":["access_token","data","data[].id"],"pfad":"/auto","status":200,"ziel":"api.individual.githubcopilot.com","zu":true}
```

Fälle der Form `konfig`: `k-gut-erststart-antwortform` (gültig, auch ein Pfad im Teilbaum von `pfade`), Gegenproben
`k-erststart-antwortform-ungedeckt` (`POST /auto` ohne Regel in `pfade`), `-teilbaum`, `-leer`, `-doppelt`, `-neun` und
`-server` (je `halter.konfig`). Tests im Bau: `antwortform::tests` (Namen ohne Werte mit Markenwert als Gegenprobe, jedes Wort
in jeder Schreibweise, Grenzen genau und um eins darüber, gehaltenes Geheimnis als Name, Zeilenumbruch im Namen),
`konfig::tests::erststart_antwortform_nur_genau_und_gedeckt`, `tests/dienst.rs` `antwortform_nennt_die_namen_ohne_werte` (die Antwort
kommt unverändert an, die Zeile nennt `access_token`, `data`, `data[].id` und `"zu":true`, der Markenwert steht weder in der
Zeile noch in der Kette) und `ohne_schalter_keine_antwortform` (dieselbe Antwort ohne Schalter: keine Zeile). Das Fallformat kennt
keine Antwort des Ziels; die Zeile prüft darum nur der Bau.

### `schluessel.vorhanden`

| Feld | Typ | Grenzen |
|---|---|---|
| `name` | Text | Name eines Geheimnisses |
| `ziel` | Kennung | ein Ziel aus `halter.json`; nur Rolle `bruecke` |

Die Anfrage trägt **genau eines** von `name` und `ziel` (Nachtrag Socket, vorhanden nach Ziel und Kette; Entscheid des
Vertragsinhabers, 25.11.2026). Beide Felder zugleich ergeben `halter.schluessel_unbekannt` (Stufe 8), keines von beiden
`halter.schluessel_fehlt` (Stufe 9). Mit `ziel` löst der Halter den Namen des Geheimnisses selbst über `schluessel` des Ziels in
`halter.json` auf; die Brücke kennt den Namen dann nicht. Ein Ziel, das nicht in `halter.json` steht, ergibt `halter.ziel_unbekannt`
(Stufe 14), nicht `false`: So bleibt ein Fehler der Konfiguration vom fehlenden Schlüssel unterscheidbar. Ein bekanntes Ziel ohne
Geheimnis ergibt `{"vorhanden": false}`. `ziel` darf nur die Rolle `bruecke` senden; jede andere Rolle bekommt `halter.rolle`
(Stufe 7). alt: nur `name`. Fälle: `a-vorhanden-ziel-bruecke`, `a-vorhanden-ziel-anmeldung`, `a-vorhanden-ziel-ohne-geheimnis`
(gültig), Gegenproben `s07-hub-vorhanden-ziel` und `s07-oberflaeche-vorhanden-ziel` (`halter.rolle`), `s08-vorhanden-name-und-ziel`,
`s09-vorhanden-ohne-name-und-ziel`, `s11-vorhanden-ziel-gross` und `s14-vorhanden-ziel-unbekannt`. Dass die Antwort für ein Ziel ohne
Geheimnis `false` ist, prüft der Bau (`tests/dienst.rs`), weil das Fallformat keine Antwort einer Anfrage kennt.

Antwort: `vorhanden` (`true` oder `false`). Im Bereich `person` zählt nur ein Geheimnis des Kontos des Aufrufers; für
einen Vertreter ist die Antwort im Bereich `person` immer `false`. Die Antwort
nennt keinen Wert, keine Länge, keinen Zeitpunkt und keinen Hash.

### `modell.aufruf` (nur Rolle `bruecke`)

| Feld | Typ | Grenzen |
|---|---|---|
| `ziel` | Kennung | muss in `halter.json` stehen |
| `methode` | Text | `GET` oder `POST` |
| `pfad` | Text | `^/[A-Za-z0-9._~/-]{0,511}$`, ohne Abfrageteil; `methode` und `pfad` zusammen müssen zu einer Regel in `pfade` des Ziels passen |
| `abfrage` | Text oder `null` | der Abfrageteil ohne das `?`: `^[A-Za-z0-9._~!$&'()*+,;=:@/?%-]{1,1024}$`, nie `#`; `null` ohne Abfrageteil |
| `kopf` | Objekt | höchstens 64 Einträge, Name `^[a-z0-9-]{1,64}$`, Wert druckbares ASCII ohne CR und LF, höchstens 8192 Zeichen |
| `koerper_b64` | Text | Base64 mit Auffüllung, höchstens 32156332 Zeichen (Rohkörper höchstens 24117248 Byte); leer erlaubt |

- **Zusage zu `kopf` (Nachtrag Socket, vorhanden nach Ziel und Kette; Entscheid des Vertragsinhabers, 25.11.2026).** `kopf` ist ein
  JSON-Objekt, auch bei `modell.strom` und in der Zeile `beginn`. Daraus folgt: Die Reihenfolge der Köpfe geht nicht über den
  Halter, und ein doppelter Name ist nicht darstellbar (ein doppelter Schlüssel ist schon `halter.json_ungueltig`, Stufe 3). Nicht
  gemessen ist, dass die Copilot CLI einen Kopfnamen doppelt sendet. Das Verhalten ändert sich nicht; der Satz macht die Grenze
  für Vertrag 8 sichtbar.

- **Abfrageteil (Nachtrag B05).** alt: Ein Abfrageteil war nicht vorgesehen. neu: eigenes Pflichtfeld `abfrage`. Der Halter
  sendet `<pfad>?<abfrage>`. Ein `?` im Feld `pfad` bleibt `halter.wert_form` (Fall `s11-pfad-query` unverändert). Grund für
  ein eigenes Feld: Die Pfadprüfung (Stufe 15) läuft nur auf `pfad` und muss nie trennen. Dass die Copilot CLI einen
  Abfrageteil sendet, ist nicht gemessen (OPEN 14); ohne das Feld lehnte der Halter aber ab, was Vertrag 8 erlaubt.
- **Körpergrenze (Nachtrag B05).** alt: `koerper_b64` bis 33554432 Zeichen; das passte nie in eine Zeile von 33554432 Byte.
  neu: bis 32156332 Zeichen, das ist Base64 von 24117248 Byte. Mit höchstens 64 Köpfen bleibt die Zeile unter 33554432 Byte
  (gerechnet: 32156332 + 528768 Zeichen Köpfe). Vertrag 8 senkt seine Grenze für gebundene Ziele auf 24117248 Byte.
- **Pfadprüfung (Nachtrag B05).** Vor dem Abgleich mit `pfade` lehnt der Halter einen unsicheren Pfad mit
  `halter.pfad_verboten` ab: `//`, ein Segment `.` oder `..`, `%2e`, `%2f`, `%5c` in jeder Schreibweise (wie Vertrag 8,
  Abschnitt 13.2, Stufe 3; `\` und `%` lässt schon die Form von `pfad` nicht zu). Grund: Mit Teilbäumen ginge sonst
  `GET /copilot_internal/../user/repos` mit dem vollen Konto an `api.github.com`.
- Adresse, Host und Schlüssel des Ziels stehen nur in `halter.json`. Die Anfrage nennt keine Adresse. So kann kein
  Aufrufer den Schlüssel an ein anderes Ziel lenken.
- Verbotene Kopfnamen (`halter.kopf_verboten`): `authorization`, `proxy-authorization`, `x-api-key`, `api-key`, `cookie`,
  `set-cookie`, `host`, `content-length`, `transfer-encoding`, `connection`, `upgrade`, `te`, `trailer`, `keep-alive`,
  `proxy-connection`, `forwarded`, `x-forwarded-for`, `x-forwarded-host`, `accept-encoding`, `sec-websocket-extensions`,
  `x-http-method-override`, `x-http-method`, `x-method-override`, `x-original-method`, `x-original-url`, `x-original-uri`,
  `x-rewrite-url`, `x-forwarded-uri`, `x-forwarded-path`, `x-forwarded-prefix`, `x-envoy-original-path` (31 Namen).
  Den Kopf mit dem Schlüssel setzt der Halter selbst, mit dem Namen aus `halter.json`. alt: ohne `accept-encoding` und
  `sec-websocket-extensions`; beide setzt oder verweigert jetzt der Halter (unten). alt (bis 25.11.2026): 20 Namen, ohne die
  elf Köpfe ab `x-http-method-override`. Grund: Mit ihnen ersetzt ein Proxy vor dem Ziel, der sie beachtet, Methode oder Pfad
  und umginge so die Pfad- und Methodenprüfung des Halters (Stufe 15) als letzte Sperre. Fälle: je Kopf
  `s12-kopf-<name>` (`halter.kopf_verboten`), Gegenprobe `a-modell-kopf-gewoehnlich` (gültig).
- **Keine Kompression (Nachtrag B05).** An jedes Modellziel hängt der Halter `accept-encoding: identity` an. Trägt die
  Antwort `content-encoding` mit einem anderen Wert als `identity`, sendet er nichts davon weiter und endet mit
  `halter.ziel_komprimiert`. Grund: Die Leck-Sperre (Abschnitt 8) ist an komprimierten Bytes blind.
- Antwort als Strom: eine Zeile `beginn` (Status und Kopf), null oder mehr Zeilen `teil` (je höchstens 65536 Byte
  Nutzdaten), zum Schluss genau eine Zeile `ende`.
- **Antwortköpfe (Nachtrag B05).** alt: nur die Namen aus `halter.json`, Vorgabe nur `content-type`. neu: alle Köpfe des
  Ziels außer `connection`, `keep-alive`, `proxy-connection`, `te`, `trailer`, `upgrade`, `transfer-encoding`,
  `content-length`, `set-cookie`, `proxy-authenticate`, `authorization`, `proxy-authorization`, `x-api-key`, `api-key` und
  `cookie`, höchstens 64, je Wert höchstens 8192 Zeichen; ein Ziel mit mehr oder längeren Köpfen endet mit
  `halter.ziel_fehler`. Diese Sperrliste gilt für jedes gebundene Ziel und steht nur hier; sie ist um fünf Namen länger als die in
  Vertrag 8, Abschnitt 13.3 (`authorization`, `proxy-authorization`, `x-api-key`, `api-key`, `cookie`). Vertrag 8 nennt beide Listen
  und sagt, dass für ein gebundenes Ziel diese gilt. Die Leck-Sperre prüft jeden Kopf (Abschnitt 8).
  Grund: Welche Köpfe die CLI braucht, ist nicht gemessen; eine Positivliste bräche sie still. Das gilt nur für Modellziele;
  `mcp.aufruf` reicht keine Köpfe weiter.

### `modell.strom` (nur Rolle `bruecke`, Nachtrag B05)

Für eine Anfrage mit Upgrade auf WebSocket (RFC 6455). Gemessen ist der Modellaufruf der Copilot CLI 1.0.88 in einem Lauf
als `GET /responses` mit 101 und ohne `permessage-deflate`; im selben Lauf ging auch ein `POST /chat/completions` hinaus
(Vertrag 8, Abschnitt 12; OPEN 15). Der Bau braucht darum `modell.aufruf` und `modell.strom`.

| Feld | Typ | Grenzen |
|---|---|---|
| `ziel`, `pfad`, `abfrage`, `kopf` | wie `modell.aufruf` | `kopf` darf `sec-websocket-key`, `sec-websocket-version` und `sec-websocket-protocol` tragen |

- Die Methode ist immer `GET`; `methode` und `pfad` müssen wie bei `modell.aufruf` zu einer Regel in `pfade` passen
  (`GET <pfad>`). Einen Körper gibt es nicht.
- Der Halter setzt `upgrade: websocket`, `connection: upgrade`, `accept-encoding: identity` und den Kopf mit dem Schlüssel
  selbst. Einen Erweiterungskopf sendet er nie.
- Antwortet das Ziel nicht mit 101, läuft die Antwort wie bei `modell.aufruf` (`beginn`, `teil`, `ende`).
- Antwortet es mit 101 und trägt die Antwort `sec-websocket-extensions` (jede Schreibweise, jeder Wert), sendet der Halter
  keine 101, schließt zum Ziel und endet mit `halter.ziel_komprimiert`.
- Sonst sendet er `beginn` mit Status 101 und den Köpfen nach der Sperrliste oben, also ohne `upgrade` und `connection`;
  diese beiden setzt die Brücke zum Harness selbst. Ab dann ist der Strom offen. Die Brücke sendet die Bytes des Harness als Zeilen
  `{"daten_b64":…,"fassung":1,"id":<id des Stroms>,"op":"strom.teil"}` (je höchstens 65536 Byte), der Halter die Bytes des
  Ziels als Zeilen `teil`. Die Bytes gehen in beiden Richtungen unverändert weiter; Rahmen vom Harness sind maskiert.
- Der Halter liest die Rahmen des Ziels nach RFC 6455 Abschnitt 5.2. Ein gesetztes RSV-Bit ist `halter.ziel_komprimiert`;
  ein gesetztes Maskenbit, ein Opcode außer `0x0`, `0x1`, `0x2`, `0x8`, `0x9`, `0xA` oder eine Länge über 33554432 Byte ist
  `halter.ziel_fehler`. Die Prüfung der RSV-Bits kommt zuerst.
- **Rahmenregeln von RFC 6455 Abschnitt 5 (Nachtrag Rahmen).** Der Modellweg prüft die Rahmen eines gebundenen Ziels nicht noch
  einmal (Vertrag 8, Abschnitt 13.3); darum prüft sie der Halter, fail closed, am Rahmenkopf, sobald der Verstoß feststeht:
  1. ein Steuerrahmen (`0x8`, `0x9`, `0xA`) mit FIN=0 oder mit einer Länge über 125;
  2. eine Fortsetzung (`0x0`), während keine geteilte Nachricht offen ist;
  3. ein neuer Datenrahmen (`0x1` oder `0x2`), während eine geteilte Nachricht offen ist (ein Rahmen mit FIN=0 kam, die
     abschließende Fortsetzung fehlt).

  Eine Nachricht ist offen ab einem Datenrahmen mit FIN=0 und endet mit der Fortsetzung mit FIN=1. Steuerrahmen mit FIN=1 und
  Länge bis 125 dürfen mitten in einer offenen Nachricht stehen und ändern sie nicht. Jeder Verstoß ist ein **Abriss**: Der Halter
  sendet kein weiteres `teil`, schließt zum Ziel und sendet `ende` mit `halter.ziel_fehler`; zurückgehaltene Bytes der Leck-Sperre
  gehen nicht hinaus. Die Rahmen der Richtung Harness zum Ziel prüft der Halter nicht; er gibt sie unverändert weiter.
- Die Leck-Sperre läuft auf den Nutzdaten, wie Vertrag 8, Abschnitt 13.3 (W3): über die Nutzdaten aller Datenrahmen in
  Reihenfolge, über Rahmen- und Nachrichtengrenzen; Steuerrahmen je für sich.
- Ende: Die Brücke sendet `{"fassung":1,"id":<id des Stroms>,"op":"strom.ende"}`, oder das Ziel schließt. Der Halter
  sendet dann zuerst die zurückgehaltenen Bytes der Leck-Sperre (Abschnitt 8) als `teil`, dann `ende`, und schließt zum Ziel.
  Das gilt auch, wenn das Ziel mitten in einem Rahmen schließt: Es geht alles hinaus, was die Sperre gesehen hat, auch ein
  unvollständiger Rahmenkopf; ein abgeschnittener Rahmen ist kein Abriss. Beim Abriss (Rahmenregeln oben), bei einem Treffer
  der Leck-Sperre und bei `halter.ziel_komprimiert` geht Zurückgehaltenes nicht hinaus. Nach `ende` bearbeitet die Verbindung wieder Anfragen. Schließt die Brücke die
  Verbindung, schließt der Halter zum Ziel.
- `strom.teil` und `strom.ende` gelten nur auf einer Verbindung mit offenem Strom und mit dessen `id`. Sonst ist es
  `halter.strom` (Stufe 7a). Während der Strom offen ist, ist jede andere Op auf dieser Verbindung `halter.strom`.

### `modell.suche` (nur Rolle `bruecke`, Nachtrag B05)

Suche nach dem Schlüssel nach einem Lauf (Vertrag 8, Abschnitt 9a, E4). Der Wert verlässt den Halter nicht.

| Feld | Typ | Grenzen |
|---|---|---|
| `ziel` | Kennung | muss in `halter.json` stehen; gesucht wird das Geheimnis dieses Ziels (bei `anmeldung.*` das des Kontos des Aufrufers) |
| `pfade` | Liste | 1 bis 16 absolute Pfade, je 1 bis 4096 Zeichen, Form `^/[^\x00-\x1f]*$`, kein Segment `.` oder `..` |

- Was durchsucht und wie gezählt wird, folgt Vertrag 8, Abschnitt 9a, S3 und S4: Verknüpfungen nie verfolgt
  (`O_NOFOLLOW`), Ordner rekursiv, nur genaue Vorkommen der Bytes, auch über Lesegrenzen.
- Antwort: `antwort` mit `ok: true` und `suche` = `{"dateien":<n>,"nicht_lesbar":<n>,"treffer":<n>,"uebersprungen":<n>}`,
  je eine ganze Zahl von 0 bis 9007199254740991. Keine Pfade, keine Stellen, keine Teiltreffer.
- Grenze der Fassung 1: Der Halter liest mit der uid der Person, also dieselben Dateien wie die Brücke. Unter einem
  Systemnutzer muss die Suche mit den Rechten des Aufrufers laufen (OPEN 9).

### `mcp.aufruf` (nur Rolle `hub`)

| Feld | Typ | Grenzen |
|---|---|---|
| `lauf` | Kennung | Lauf, für den der Hub arbeitet |
| `server` | Kennung | muss in `halter.json` stehen |
| `werkzeug` | Text | `^[A-Za-z0-9_.-]{1,128}$` |
| `argumente` | Objekt | frei; geht unverändert an den Server |
| `person` | Text | `^ps_[0-9a-f]{32}$`, Pseudonym der Person |
| `vorschau_sha256` | Text oder `null` | 64 hex klein; `null` bei einem Aufruf ohne Vorschau |
| `schein` | Objekt | nur wenn der Aufruf einen Schein braucht (Abschnitt 6) |

- Die Klasse des Werkzeugs (`lesend` oder `schreibend`) liest der Halter aus seiner eigenen Kopie der Firmenvorgabe in
  `halter.json`, nie aus der Anfrage. Ohne Klasse gilt ein Werkzeug als schreibend (A89).
- Einen Schein braucht genau ein schreibender Aufruf auf einem Server mit Option „Freigabe je Schreibaufruf“ an (Vorgabe an,
  A90). Fehlt er, gilt `halter.schein_fehlt`. Liegt einer bei, wo keiner gebraucht wird, gilt `halter.schein_unerwartet`.
- Die Anfrage trägt **keinen** `digest`. Den `digest` rechnet der Halter selbst aus dem Aufruf, den er gleich sendet.
- Remote-Server: Der Halter setzt den Token beim Aufruf ein. Lokaler Server mit Token: nur unter Windows, als Kind des
  Halters unter dessen Dienstkonto (Abschnitt 2.2); unter Linux und macOS Fassung 1 nicht (Abschnitt 2.1).
- Antwort: `ergebnis`, das Ergebnisobjekt des MCP-Servers. Meldet der Server einen JSON-RPC-Fehler, antwortet der Halter
  `halter.mcp_fehler` mit `mcp_code` (der Zahl), ohne den Text des Servers.

### `schluessel.setzen` und `schluessel.loeschen` (nur Rolle `oberflaeche`)

| Feld | Typ | Grenzen |
|---|---|---|
| `name` | Text | Name eines Geheimnisses im Bereich `person` |
| `wert_b64` | Text | nur bei `setzen`: Base64, 4 bis 16384 Zeichen |
| `nachweis` | Text | 1 bis 8192 Zeichen druckbares ASCII ohne Leerzeichen; frischer Menschen-Nachweis der Stelle |

Der Halter prüft den Nachweis bei der Stelle (Form OPEN 1). Er legt das Geheimnis unter dem Konto des Aufrufers mit dem
Pseudonym aus dem Nachweis ab. `setzen` ersetzt ein vorhandenes Geheimnis desselben Namens. `loeschen` überschreibt die
Datei vor dem Entfernen nicht (hergeleitet: auf SSD ohne Wirkung) und meldet auch bei fehlendem Geheimnis `ok`.

## 5. Antworten

Jede Zeile hat `art`, `fassung` (`1`) und `id`. Die Form ist geschlossen; das Schema `$defs/antwort` ist die vollständige
Liste dessen, was herausgehen darf.

| `art` | Inhalt |
|---|---|
| `antwort`, gelungen | `ok: true`, dazu `vorhanden` (bei `schluessel.vorhanden`), `ergebnis` (bei `mcp.aufruf`), `suche` (bei `modell.suche`) oder nichts |
| `antwort` oder `ende`, Fehler | `ok: false`, `code`, `grund` (der feste Text aus Abschnitt 7), bei `halter.mcp_fehler` dazu `mcp_code`; `id` ist `null`, wenn die Zeile kein Objekt mit gültigem `id` war |
| `beginn` | `status` (100 bis 599, 101 nur bei `modell.strom`), `kopf` (höchstens 64 Einträge, je Wert höchstens 8192 Zeichen; alt: 16 und 1024) |
| `teil` | `daten_b64` |
| `ende`, gelungen | `ok: true` |

Kein Feld trägt einen Schlüssel, einen Token, eine Länge eines Geheimnisses, einen Fehlertext eines Ziels oder einen Pfad.

## 6. Schein und digest

`digest` = sha256 hex klein über die RFC-8785-Bytes von
`{"argumente", "person", "server", "vorschau_sha256", "werkzeug"}`.

Der Schein (`$defs/schein`) besteht aus `daten` und `signatur`:

| Feld in `daten` | Typ | Bedeutung |
|---|---|---|
| `art` | Kennung | genau `mcp-schein` (nicht der Ticket-Schlüssel der Stelle; Nachtrag 18.11.2026) |
| `fassung` | Ganzzahl | genau `1` |
| `kid` | Kennung | Schlüsselkennung; der Halter kennt nur die `kid` aus `halter.json` |
| `anfrage` | Text | `mf_<32 hex>`, Anfrage bei der Stelle |
| `digest` | 64 hex | der freigegebene Aufruf |
| `lauf` | Kennung | Lauf, für den freigegeben wurde |
| `ausgestellt`, `ablauf` | Ganzzahl | Millisekunden seit 1970, UTC |
| `einmal` | 32 hex | 128 Bit Zufall der Stelle |

`signatur` ist Ed25519 (RFC 8032) über die RFC-8785-Bytes von `daten`, Base64 mit Auffüllung (88 Zeichen). Der öffentliche
Schlüssel je `kid` steht in `halter.json`; er gehört dem eigenen Schlüsselpaar der Stelle nur für MCP-Scheine.

Prüfung in fester Reihenfolge (Stufen 19 bis 26 in Abschnitt 7): `kid` bekannt; Signatur gültig; `art` und `fassung`;
`lauf` gleich `lauf` der Anfrage; `digest` gleich dem selbst gerechneten; Zeitform (`ablauf − ausgestellt` von 1 bis 86400000,
`ausgestellt` höchstens 5000 ms nach der eigenen Uhr); Frist (`ausgestellt` nicht vor dem eigenen Start, eigene Uhr vor
`ablauf`); `einmal` noch nicht eingelöst.

**Einlösefrist und Schein (B15, aufgelöst, Entscheidung vom 23.11.2026):** Der Schein trägt die eingestellte
Einlösefrist als `ablauf − ausgestellt`; die Stelle setzt `ablauf` auf `ausgestellt` plus `einloesefrist_ms` aus der Einstellung
(Vorgabe 300000, Bereich 1 bis 86400000 in `rust/spec/einstellungen/README.md` und `rust/spec/stelle/README.md`). Der Halter
liest die Einstellung nicht und prüft nur denselben Bereich 1 bis 86400000; die Stelle bleibt die einzige Quelle der Frist.
alt: Schein höchstens 300000 ms (Widerspruch OPEN 13).

**Einmal-Einlösung (A105).** Nach bestandener Prüfung trägt der Halter `einmal` unter einer einzigen Sperre für alle
Verbindungen als eingelöst ein, **bevor** er den Aufruf an den Server sendet. Zwei gleichzeitige Anfragen mit demselben
Schein führen darum genau einmal aus. Scheitert der Aufruf danach, bleibt der Schein verbraucht; ein neuer Versuch braucht
eine neue Freigabe. Eingelöste Kennungen hält der Halter im Speicher bis zu ihrem `ablauf`. Nach einem Neustart lehnt er
jeden Schein ab, der vor dem eigenen Start ausgestellt wurde; so braucht er keine Datei. Den Ausgang meldet der Halter mit
`mcp.ausgefuehrt` an die Stelle (Op); die Stelle nimmt je Anfrage nur eine Meldung an (zweite Sperre).

## 7. Fehlercodes und Prüfreihenfolge

Die Prüfung meldet genau einen Code: den der ersten verletzten Stufe. Code und Grund sind feste ASCII-Texte. Der Hub und die
Brücke reichen nur den Code weiter.

| Stufe | Code | Verletzung | Grund |
|---|---|---|---|
| 1 | `halter.zu_gross` | Zeile über 33554432 Byte | `Zeile zu gross` |
| 2 | `halter.peer` | Peer-Messung gescheitert oder keine Rolle (Abschnitt 2) | `Aufrufer nicht zugelassen` |
| 3 | `halter.json_ungueltig` | kein UTF-8, BOM, Syntaxfehler, doppelter Schlüssel, kein Objekt, `NaN` | `kein gueltiges JSON` |
| 4 | `halter.nicht_kanonisch` | Bytes sind nicht RFC 8785 | `nicht kanonisch` |
| 5 | `halter.fassung_unbekannt` | `fassung` fehlt oder ist nicht die Ganzzahl `1` | `Fassung unbekannt` |
| 6 | `halter.op_unbekannt` | `op` fehlt, ist kein Text oder nicht in der Tabelle | `Op unbekannt` |
| 7 | `halter.rolle` | Op nicht erlaubt für die Rolle, oder `schluessel.vorhanden` mit `ziel` ohne Rolle `bruecke` | `Op fuer Rolle nicht erlaubt` |
| 7a | `halter.strom` | `strom.teil` oder `strom.ende` ohne offenen Strom dieser `id`, oder eine andere Op bei offenem Strom (Nachtrag B05) | `kein passender Strom` |
| 8 | `halter.schluessel_unbekannt` | ein Schlüssel außerhalb der Form, auch im Schein | `unbekannter Schluessel` |
| 9 | `halter.schluessel_fehlt` | ein Pflichtschlüssel fehlt | `Pflichtfeld fehlt` |
| 10 | `halter.typ_falsch` | falscher JSON-Typ | `falscher Typ` |
| 11 | `halter.wert_form` | Muster, Länge, Aufzählung oder Base64 verletzt | `Wert hat falsche Form` |
| 12 | `halter.kopf_verboten` | verbotener Kopfname in `kopf` | `Kopf nicht erlaubt` |
| 13 | `halter.bereich` | `setzen` oder `loeschen` für einen Namen im Bereich `maschine` | `Bereich nicht erlaubt` |
| 14 | `halter.ziel_unbekannt` | `ziel` oder `server` nicht in `halter.json` (auch `ziel` bei `schluessel.vorhanden`) | `Ziel unbekannt` |
| 15 | `halter.pfad_verboten` | `pfad` unsicher (Abschnitt 4, Pfadprüfung), oder `methode` und `pfad` passen zu keiner Regel des Ziels | `Pfad nicht erlaubt` |
| 16 | `halter.kein_schluessel` | kein Geheimnis für Ziel oder Server (im Bereich `person`: keines mit diesem Namen und dieser `person`; bei `anmeldung.*` keines mit diesem Namen und dem Konto des Aufrufers) | `kein Schluessel hinterlegt` |
| 17 | `halter.person` | das Geheimnis gehört einem anderen Konto als dem des Aufrufers, und der Aufrufer ist kein Vertreter | `Person passt nicht` |
| 18 | `halter.schein_fehlt` oder `halter.schein_unerwartet` | Schein fehlt, obwohl gebraucht, oder liegt bei, obwohl nicht gebraucht | `Schein fehlt` oder `Schein nicht erwartet` |
| 19 | `halter.schein_kid` | `kid` unbekannt | `Schluesselkennung unbekannt` |
| 20 | `halter.schein_signatur` | Signatur ungültig | `Signatur ungueltig` |
| 21 | `halter.schein_art` | `art` nicht `mcp-schein` oder `fassung` nicht `1` | `falsche Art des Scheins` |
| 22 | `halter.schein_lauf` | `lauf` des Scheins nicht der der Anfrage | `Schein fuer anderen Lauf` |
| 23 | `halter.schein_digest` | `digest` passt nicht zum Aufruf | `Schein passt nicht zum Aufruf` |
| 24 | `halter.schein_zeit` | Zeitform verletzt | `Zeiten des Scheins ungueltig` |
| 25 | `halter.schein_abgelaufen` | vor dem Start ausgestellt oder abgelaufen | `Schein abgelaufen` |
| 26 | `halter.schein_benutzt` | `einmal` schon eingelöst | `Schein schon benutzt` |
| 27 | `halter.nachweis` | Menschen-Nachweis ungültig oder nicht frisch | `Nachweis ungueltig` |
| 28 | `halter.ziel_fehler` | Ziel oder Server nicht erreichbar, Frist zum Ziel verstrichen (Abschnitt 3), Rahmen des Ziels nicht lesbar oder gegen RFC 6455 Abschnitt 5 (Abschnitt 4, Rahmenregeln), zu viele oder zu lange Antwortköpfe | `Ziel nicht erreichbar` |
| 28a | `halter.ziel_komprimiert` | Antwort mit `content-encoding` außer `identity`, 101 mit `sec-websocket-extensions` oder Rahmen mit RSV-Bit (Nachtrag B05) | `Antwort komprimiert` |
| 29 | `halter.mcp_fehler` | MCP-Server meldet einen JSON-RPC-Fehler | `MCP-Server meldet Fehler` |
| 30 | `halter.leck_gesperrt` | eine ausgehende Zeile enthielte ein Geheimnis (Abschnitt 8) | `Antwort enthielt einen Schluessel` |
| 31 | `halter.intern` | unerwarteter Fehler | `interner Fehler` |

Stufen 1 bis 27 laufen, bevor irgendetwas das Ziel erreicht. Ab Stufe 28 ist der Aufruf gesendet; bei `mcp.aufruf` mit
Schein ist der Schein dann verbraucht. Client-seitig gibt es dazu `halter.dienst_unecht` (Abschnitt 2.1).

## 8. Was nie herausgeht (A14, A93, A106)

- Kein Wert eines Geheimnisses in einer Antwort, einem Fehlertext, einer Protokollzeile, dem Ereignisprotokoll, argv oder
  der Umgebung eines Prozesses außerhalb des Halters und seiner Kinder. Das gilt auch für ein abgeleitetes Token, das ein
  Ziel gegen ein Geheimnis ausgibt (A93; Abschnitt 4, „Abgeleitete Tokens“).
- **Leck-Sperre.** Vor dem Senden jeder Zeile prüft der Halter die Nutzdaten (`daten_b64` dekodiert, `kopf`, `ergebnis` als
  kanonische Bytes) auf jedes Geheimnis, das er hält, in jeder dieser Formen: die Bytes selbst; Base64 und Base64url, mit und ohne
  Auffüllung, in jedem der drei Versätze (gesucht werden die Zeichen, die allein Bytes des Geheimnisses bestimmen; die Zeichen an
  beiden Enden hängen von den Nachbarbytes ab); und Prozentkodierung mit großen oder kleinen Hexziffern, auch gemischt mit rohen
  Bytes (gesucht wird in den Nutzdaten und in den Nutzdaten nach Auflösen jedes `%XX`, dort auch nach den Base64-Formen). Beim
  Strom hält er die letzten (3 × längste Form − 1) Byte zurück, bis der nächste Teil sie abdeckt; die längste Form ist die
  längere von Geheimnis und Base64-Form, der Faktor 3 deckt eine Form, deren Bytes alle prozentkodiert sind. alt: nur die
  rohen Bytes, Rückhalt (längste Geheimnislänge − 1) Byte. **Am Ende des Stroms** (Ende des Ziels, `strom.ende`, Ende einer Antwort
  mit Körper) gehen die zurückgehaltenen Bytes als letzter Teil vor `ende` hinaus, sonst kürzte der Halter jede Antwort um bis
  zu (längste Geheimnislänge − 1) Byte; sie können keine ganze Form eines Geheimnisses enthalten. Bei einem Treffer, einem Abriss
  (`halter.ziel_fehler`) und `halter.ziel_komprimiert` gehen sie nicht hinaus. Ein Treffer ersetzt die Zeile durch `ende` beziehungsweise `antwort` mit
  `halter.leck_gesperrt`; danach sendet er für diese Anfrage nichts mehr. Jeder Treffer hinterlässt eine Journalzeile ohne
  Inhalt (Abschnitt 9). Nicht gesucht werden andere Kodierungen (etwa JSON-Unicode-Escape einzelner Zeichen, Hex, doppelte
  Prozentkodierung) und abgeleitete Tokens; diese sperrt Abschnitt 4 über die Pfadregeln (OPEN 5). Prüfung im Bau: Tests der
  Leck-Sperre je Form und Versatz, über Teilgrenzen, mit Gegenprobe (harmloser Text geht unverändert durch), und W3 mit
  kodierter Rückgabe der Attrappe.
- **Leck-Sperre im Strom (Nachtrag B05).** Bei `modell.strom` nach der 101 prüft der Halter nicht die rohen Bytes, sondern
  die Nutzdaten der Rahmen des Ziels (Abschnitt 4). Ein Schlüssel über zwei Rahmen mit Rahmenköpfen dazwischen wird so
  erkannt. Ein Treffer schließt beide Richtungen; die Zeile `ende` trägt `halter.leck_gesperrt`.
- **Nur unkomprimiert (Nachtrag B05).** An Modellziele sendet der Halter `accept-encoding: identity` und nie einen
  Erweiterungskopf; eine komprimierte Antwort endet mit `halter.ziel_komprimiert` (Stufe 28a), bevor ein Byte davon
  hinausgeht.
- Der Halter schreibt keine Argumente, Vorschauen, Körper oder Antworten ins Protokoll. Die Diagnose des Erststarts
  (Abschnitt 4, „Antwortform im Erststart“) schreibt nur Feldnamen ins Journal, nie Werte, und nur hinter ihrem Schalter.
- Der Halter setzt nie eine Umgebungsvariable für einen Prozess der Person.

## 9. Protokoll des Halters

Je abgelehntem Schein und je Leck-Sperre schreibt der Halter eine Zeile in seine eigene Kette (Quelle `halter`,
`halter-<JJJJ-MM>.jsonl`, Anker `halter.anker.jsonl`; Nachtrag 18.11.2026, Punkt 2). Vorschlag der
Zeile (Fassung 2, RFC 8785, Kettenregeln aus Vertrag 5):

```
{"aktionsname":"mcp.schein_abgelehnt","digest":"<64 hex>","ergebnis":"abgelehnt","fassung":2,"grund":"benutzt","lauf":"l-beispiel","server":"tickets","vorher":"<64 hex>","werkzeug":"update","zeit":"2026-11-20T13:00:00.000Z"}
```

`grund` aus der festen Liste `signatur`, `digest`, `abgelaufen`, `benutzt`, `peer`, `leck`. Zuordnung: Stufen 19 bis 21 →
`signatur`, 22 → `peer`, 23 → `digest`, 24 und 25 → `abgelaufen`, 26 → `benutzt`, 30 → `leck`. Für Stufe 18 (`schein_fehlt`)
fehlt ein Grund in der Liste (OPEN 2). Der `digest` ist der selbst gerechnete. Keine Argumente, keine Person, kein `id`.

**Leck-Sperre (Nachtrag 25.11.2026, OPEN 2).** Jeder Treffer der Leck-Sperre (Stufe 30, bei `modell.aufruf`, `modell.strom`
und `mcp.aufruf`) schreibt ins Journal (stderr der Einheit, unter Windows das Ereignisprotokoll) genau die feste Zeile
`halter.leck_gesperrt: Antwort enthielt einen Schluessel`, ohne Wert, Pfad, Kopf, Körper, `id`, `lauf` oder Argument. Grund: Ein
Treffer ist ein Versuch, ein Geheimnis hinauszuziehen; ohne Spur bliebe er unbemerkt. alt: kein Journal und keine Kette.
Der Grund der Kettenzeile ist `leck` (Stufe 30). **Kettenzeile (Nachtrag Socket, vorhanden nach Ziel und Kette; Fund F4):** Jeder
Treffer schreibt zusätzlich zur Journalzeile genau eine Zeile `halter.leck_gesperrt` in die Kette des Halters. Vertrag 5 führt die
Art im Register (`formate`, `REGISTER_FREMD`, Gruppe „in anderen Verträgen festgelegt“); ihre Form steht nur hier. Felder genau:
`aktionsname`, `ergebnis` (`abgelehnt`), `fassung` (2), `grund` (`leck`), `op` (`modell.aufruf`, `modell.strom` oder
`mcp.aufruf`), `vorher`, `zeit` und `ziel` (der Name des Ziels oder Servers aus `halter.json`). Nie der Wert, nie ein Teil des
Treffers, keine Form, kein Pfad, kein Kopf, kein Körper, kein `id`, kein `lauf`. Kann der Halter die Kette nicht fortschreiben,
gilt die Regel aus Abschnitt 2.3 (eine feste Journalzeile je Lauf, die Antwort bleibt `halter.leck_gesperrt`). alt: „Die
Kettenzeile selbst schreibt der Halter erst, wenn Vertrag 5 ihre Art in sein Register aufnimmt.“ Test im Bau:
`kette::leck_tests::leck_schreibt_die_kettenzeile_mit_den_feldern_des_vertrags` liest die Zeile und prüft die Kette mit
`formate::kette::pruefe_quelle` (Gegenprobe: eine Art mit Tippfehler bricht die Kette). Form der Zeile (eigene Art, weil ein
Treffer bei `modell.*` keinen Schein, keinen `digest` und keinen `lauf` hat):

```
{"aktionsname":"halter.leck_gesperrt","ergebnis":"abgelehnt","fassung":2,"grund":"leck","op":"modell.aufruf","vorher":"<64 hex>","zeit":"2026-11-20T13:00:00.000Z","ziel":"api.individual.githubcopilot.com"}
```

**Abgelehnter Pfad (Nachtrag N1, 25.11.2026).** Jede Ablehnung in Stufe 15 (`halter.pfad_verboten`) schreibt eine Zeile
`halter.pfad_verboten` in die Kette. Felder genau: `aktionsname`, `ergebnis` (`abgelehnt`), `fassung` (2), `grund` (`pfad`),
`methode` (`GET` oder `POST`, bei `modell.strom` `GET`), `op` (`modell.aufruf` oder `modell.strom`), `pfad` (das Feld `pfad`
der Anfrage, ohne Abfrageteil), `vorher`, `zeit` und `ziel`. Nie `abfrage`, ein Kopf, der Körper, `id` oder ein Wert. Grund: Die
Leitung entscheidet beim ersten Start jede fehlende Regel anhand der Kette (Abschnitt 4). Vertrag 5 führt die Art im Register
(`formate`, `REGISTER_FREMD`); die Zeile kommt in die Kette ohne die feste Journalzeile `halter.kette: Kette nicht
fortgeschrieben`, die nur ein Fehler der Kette auslöst (Abschnitt 2.3). Test im Bau: `kette::tests::die_zeile_eines_abgelehnten_pfads_traegt_methode_und_pfad`,
`tests/dienst.rs` `n1_pfad_ausserhalb_der_regeln_geht_in_die_kette` und die Fallprüfung `tests/faelle.rs` (jede Ablehnung in Stufe 15
trägt Methode und Pfad, keine andere).

```
{"aktionsname":"halter.pfad_verboten","ergebnis":"abgelehnt","fassung":2,"grund":"pfad","methode":"POST","op":"modell.aufruf","pfad":"/v1/fremd","vorher":"<64 hex>","zeit":"2026-11-20T13:00:00.000Z","ziel":"api.individual.githubcopilot.com"}
```

Ort der Kette: unter Linux in Schritt I `<heim>/.local/state/agent-workbench/halter/protokoll/` (Abschnitt 2.3, Nachtrag Rolle aus
der Gruppe), in Schritt II `/var/lib/wb-halter/`.

## 10. Was der Halter vom Einschluss annimmt

Der Halter baut keinen Einschluss. Er verlässt sich auf diese Eigenschaften und prüft sie nur als Außentest.

**Windows (Plattformschicht; A03, A117):** Der Agent läuft im AppContainer mit Job. Die Pipe hat keine ACE für
AppContainer (gemessen, Frage 2: Fehler 5). Kein Prozess des Agenten läuft ohne Container.

**Linux (bwrap):** Weder der Socket-Ordner noch die Ablage ist in den Einschluss eingebunden; `/run/user/<uid>`
(alt: `$XDG_RUNTIME_DIR`) und `<heim>/.local/state/agent-workbench/halter` der Person sind im Einschluss nicht sichtbar. Die Ablage steht in der Verbotsliste
des Einschlusses wie ein Anmeldespeicher: Kein Pfad einer Sitzung darf sie decken, in ihr liegen oder sie berühren (alt:
„`$XDG_STATE_HOME` der Person ist im Einschluss nicht sichtbar“; eine Verbotszeile war nicht verlangt). Der Einschluss hat eine eigene PID-Sicht, also kein
`/proc` der Halterprozesse. Kein setgid wirkt im Einschluss: `bwrap` setzt `no_new_privs` und bindet jeden Pfad mit
`nosuid` ein, ein setgid-Programm (etwa ein Rollenprogramm unter `/usr/lib/werkbank`) läuft dort mit der gid der Person
und nicht mit der seiner Datei (gemessen 25.11.2026 auf dem Linux-Bauhost: außen `egid` 992, im Einschluss 1000, `NoNewPrivs: 1`,
Einbindung `ro,nosuid`; Außentest `l1_no_setgid_acts_in_the_enclosure` in `rust/crates/einschluss/tests/aussen.rs`,
mit Gegenprobe außen, schreibgeschützt und schreibbar eingebunden).

**macOS (Einschluss nach dem macOS-Einschluss-Vertrag).** Der Halter nimmt an:

1. Das Seatbelt-Profil des Agenten verbietet `network-outbound` zu Unix-Sockets unter dem Socket-Ordner des Halters und
   jeden Datei-Zugriff auf diesen Ordner.
2. Es verbietet `mach-lookup` auf `com.apple.SecurityServer` und `com.apple.securityd.xpc` sowie den Lese-Zugriff auf
   `~/Library/Keychains/`. Damit erreicht der Agent den Schlüsselbund gar nicht, auch nicht über `/usr/bin/security`.
3. Es verbietet `process-info*` und `mach-priv-task-port` auf fremde Prozesse; der Agent liest weder Umgebung noch argv des
   Halters.
4. Jeder Prozess des Agenten erbt die Sandbox (kein Start außerhalb über `launchctl`, `open` oder einen XPC-Dienst der
   Person). Der Halter erkennt einen Prozess im Einschluss über `sandbox_check` auf dessen Audit-Token und weist ihn ab.
5. Der Einschluss setzt keine Umgebungsvariable mit Schlüssel und reicht dem Agenten keinen Deskriptor des Halters weiter.

Fehlt eine dieser Eigenschaften, gilt die Zusage „Agent erreicht den Halter nicht“ unter macOS nicht.

## 11. Sicherheitszusagen und Außentests

Jede Zusage ist eine prüfbare Eigenschaft. Die Außentests laufen in der VM mit eigenem
Schnappschuss je Lauf. Vor jedem Lauf mit Claude Code gilt der ssh-Stummel. In der VM liegt kein
echter Token; die Tests nutzen Test-Geheimnisse mit einer Marke `hgx<n>` und Attrappen.

| Nr | Zusage | Außentest (Ausgangslage, Handlung, erwartet, gemessen) | Gegenprobe (muss rot werden) |
|---|---|---|---|
| W1 | Fremde SID abgelehnt, Rolle aus der SID (A115) | Konten P1 und P2 in `halter.json`, P3 nicht; Hub-Dienst `NT SERVICE\wb-hub`, Modellweg-Dienst. P3 verbindet sich; ein AppContainer-Prozess von P1 verbindet sich. Erwartet: P3 und AppContainer abgewiesen (Fehler 5 beim Öffnen oder `pk.abgewiesen`). 100 Verbindungen aus Hub-Prozess, Modellweg, P1 und P2 (je 25): Rolle und SID 100 von 100 richtig, `programm` und `startzeit` `null`. Gemessen: Fehlercodes, SID und Rolle je Verbindung | DACL mit `Authenticated Users`: P3 kommt durch |
| W2 | Zweite Pipe-Instanz abgelehnt (A115) | (a) P1 legt `\\.\pipe\wb-halter` vor dem Dienststart an: Der Dienst endet mit Exit 74 und `halter.pipe_belegt`. (b) Der Client von P2 verbindet sich mit dem Vorbeleger: Besitzer ist nicht `NT SERVICE\wb-halter`, er sendet nichts (`halter.dienst_unecht`). (c) Bei laufendem Dienst versucht P1, eine weitere Instanz anzulegen: Fehler 5. Gemessen: Exitcode, Bytes beim Vorbeleger (0), Fehlercode | ohne `FILE_FLAG_FIRST_PIPE_INSTANCE`: (a) startet |
| W3 | Schlüssel in keiner Antwort (A14, A93, A106) | Test-Geheimnisse mit Marke für ein Modellziel, einen MCP-Server und ein Mailkonto. Jede Op wird mit gültigen und den sichtbaren ungültigen Fällen gerufen. Die Attrappe des Ziels gibt den empfangenen Schlüsselkopf im Körper, in einem Kopf und in einer Fehlermeldung zurück, einmal über eine Teilgrenze verteilt. Erwartet: Jede solche Antwort endet mit `halter.leck_gesperrt`. Gemessen: Zahl der Marken in allen Antwortbytes, im Ereignisprotokoll, in der Kette des Halters, in argv und Umgebung aller Prozesse der Person und in allen Dateien, die die Person lesen kann: 0. Die Attrappe hat den Schlüssel bekommen (Zähler 1 je Aufruf) | Leck-Sperre aus: Marke im Strom gefunden |
| W4 | Einmal-Einlösung (A105) | MCP-Attrappe zählt Schreibaufrufe. (a) Derselbe gültige Schein zweimal nacheinander: erst `ok`, dann `halter.schein_benutzt`. (b) Derselbe Schein auf 8 Verbindungen gleichzeitig: genau 1 `ok`, 7 `halter.schein_benutzt`. (c) Halter-Neustart, dann der Schein noch einmal: `halter.schein_abgelaufen`. (d) Argument nach der Freigabe geändert: `halter.schein_digest`. Gemessen: Zähler der Attrappe 1 in (a) und (b), 0 in (c) und (d); Kettenzeilen mit `benutzt`, `abgelaufen`, `digest` | Eintrag erst nach dem Senden: (b) zählt mehr als 1 |
| W5 | Ablage nur für den Dienst (A116) | P1 normal und P1 im AppContainer lesen eine Datei der Ablage; der Halter liest sie; das Profil von P1 wird nach DPAPI-CurrentUser-Daten der Werkbank durchsucht. Erwartet: Fehler 5, Fehler 5, Halter liest, 0 Funde. Gemessen: Fehlercodes, Zähler | Ablage mit vererbter ACE für `Users`: P1 liest |
| W6 | Token nur im Halter (A93) | Lokaler MCP-Server mit Token in der Umgebung als Kind des Halters. P1 liest dessen Umgebung und Speicher (`OpenProcess` mit `PROCESS_VM_READ`). Erwartet: Fehler 5. Gemessen: Fehlercode | Server unter dem Konto von P1: Umgebung lesbar |
| W7 | Rolle trennt die Ops | Der Hub ruft `modell.aufruf` und `schluessel.setzen`, die Brücke `mcp.aufruf`: je `halter.rolle`; die Attrappen zählen 0 | Rollentabelle aus: Aufruf kommt an |
| W8 | Fremder Token unerreichbar | P2 hat einen Token `mcp.wiki`. Ein Prozess von P1 mit Rolle `oberflaeche` ruft `schluessel.vorhanden` für `mcp.wiki`: `false`. Unter Linux und macOS ruft der Hub von P1 `mcp.aufruf` für `wiki` mit der Person von P2: `halter.person` | Schlüssel nur nach Namen: `true` beziehungsweise Aufruf läuft mit dem Token von P2 |
| W9 | Person kann die Hub-SID nicht annehmen | P1 ohne Adminrechte versucht, einen Prozess unter `NT SERVICE\wb-hub` zu starten (`CreateProcessWithLogonW`, `LogonUser` mit dem Dienstkonto, Dienststeuerung des Hub-Dienstes ändern), und ruft danach als P1 `mcp.aufruf`. Erwartet: jeder Startversuch scheitert; der Aufruf als P1 bekommt `halter.rolle`. Gemessen: Fehlercodes, Antwort des Halters | Hub-Dienst mit Dienst-SD, der P1 `SERVICE_CHANGE_CONFIG` gibt: P1 setzt ein eigenes Programm ein und bekommt die Rolle `hub` |
| W10 | Oberfläche setzt nur mit Nachweis | Ein beliebiger Prozess von P1 außerhalb eines Containers ruft `schluessel.setzen` ohne gültigen Nachweis, mit einem Nachweis für P2 und mit einem abgelaufenen Nachweis. Erwartet: je `halter.nachweis`, die Ablage bleibt unverändert. Gemessen: Antworten, Dateiliste der Ablage vor und nach | Nachweis-Prüfung aus: Token wird ersetzt |

Nicht gemessen und nicht Teil der Außentests von Fassung 1: erzwungene Wiederverwendung einer PID (OPEN 3), interaktive
Sitzung `WinSta0`.

**Linux (auf einem Linux-Prüfrechner; Wegwerfordner unter `/var/tmp`).** W3, W4, W7 und W8 gelten unverändert mit
Unix-Socket. Dazu:

| Nr | Zusage | Außentest | Gegenprobe |
|---|---|---|---|
| L1 | Agent erreicht Halter und Ablage nicht | Ein Agent im bwrap-Einschluss sucht den Socket und die Ablage (`stat`, `connect`, `open`) und durchsucht `/proc`: je Fehler, 0 Funde der Marke | Socket-Ordner in den Einschluss gebunden: `connect` gelingt |
| L2 | Vorbelegung | Am Socketpfad liegt eine Datei oder ein fremder Socket (mit Zuhörer, oder einer anderen uid): Exit 74, der Inhalt bleibt Byte für Byte gleich. Dazu (F2): ein toter Socket der eigenen uid (ohne Zuhörer) wird entfernt, und der Start gelingt; ein lebender Halter bleibt, der zweite Start endet mit 74 | Halter löscht den Pfad trotz Zuhörer: Test rot; Halter räumt den toten Socket nicht: Start endet mit 74 |
| L3 | Rolle an der Gruppe (Nachtrag Rolle aus der Gruppe; alt: „Rolle am Programm: Eine Kopie des Hub-Programms unter dem Heimordner verbindet sich: `halter.peer`“) | Derselbe Peer verbindet sich zweimal: (a) mit der effektiven gid einer Rollengruppe (gestartet über das setgid-Rollenprogramm, ohne root im Test über eine Gruppe des Testnutzers): die Rolle; (b) ohne diese gid, etwa als Kopie des Rollenprogramms ohne setgid unter dem Heimordner: `halter.peer`, Verbindung zu. Dazu ein Peer mit `PR_SET_DUMPABLE 0` und der gid: die Rolle. Gemessen: Antwort und Code je Verbindung | Rolle nur über die uid oder über `programm`: (b) kommt durch beziehungsweise der Peer mit Dumpable 0 bekommt `halter.peer` |
| L10 | Kein Konto in einer Rollengruppe (Nachtrag Rolle aus der Gruppe) | `/etc/group` nennt die Person als Mitglied von `wb-bruecke`, oder `wb-bruecke` ist ihre Hauptgruppe, oder die Gruppe hat ein Passwort: Der Halter endet mit Exit 78 (`halter.konfig`), `werkbank-pruefen` meldet `pruef.rollengruppe_mitglied` (rc 1). Gemessen: Exitcode, Zeile, Ausgabe des Prüfbefehls | Prüfung aus: Halter startet, und jeder Prozess der Person hat die Rolle `bruecke` |
| L4 | Kein lokaler Server mit Token | `halter.json` mit lokalem Server mit Token: Exit 78 | Prüfung aus: Halter startet, die Startumgebung des Kindes zeigt die Marke |
| L5 | Speicher des Halters gegen Prozesse derselben uid (Nachtrag B05) | Halter mit Test-Geheimnis `anmeldung.copilot` (Marke) nach einem `modell.aufruf`. Ein Prozess derselben uid ohne root versucht `ptrace(PTRACE_ATTACH)`, liest `/proc/<pid>/mem` und `/proc/<pid>/environ` und schickt SIGQUIT. Erst prüfen, dass `/proc/<pid>` sichtbar ist und den Einträgen root gehört. Erwartet: je Fehler, kein Kernabbild, 0 Marken. Gemessen: Fehlercodes, Eigner der Einträge, Zahl der Marken | Halter ohne Schritt 0 (`PR_SET_DUMPABLE` 1): `/proc/<pid>/mem` lesbar, Marke gefunden |
| L6 | Leck-Sperre im Strom (Nachtrag B05) | Attrappe des Ziels antwortet auf `modell.strom` mit 101 und gibt den empfangenen Schlüsselkopf in einem Textrahmen zurück, einmal in einem Rahmen, einmal über zwei Rahmen einer Nachricht mit einem Ping dazwischen. Erwartet: `ende` mit `halter.leck_gesperrt`, 0 Marken in den Bytes an die Brücke | Sperre auf rohen Bytes statt Nutzdaten: die geteilte Marke kommt durch |
| L7 | Nichts Komprimiertes geht durch (Nachtrag B05) | Attrappe antwortet (a) mit `content-encoding: gzip`, (b) mit 101 und `sec-websocket-extensions: permessage-deflate`, (c) mit einem Rahmen mit RSV1. Erwartet: je `halter.ziel_komprimiert`, 0 Bytes des Körpers oder Rahmens an die Brücke; die Anfrage an die Attrappe trug `accept-encoding: identity` und keinen Erweiterungskopf | Prüfung aus: die komprimierten Bytes kommen an |
| L8 | Rahmenregeln von RFC 6455 Abschnitt 5 (Nachtrag Rahmen) | Attrappe des Ziels antwortet auf `modell.strom` mit 101 und sendet danach je Lauf genau einen Verstoß: (a) Ping mit FIN=0, (b) Pong mit Länge 126, (c) Close mit Länge 126, (d) Fortsetzung ohne offene Nachricht, (e) Textrahmen mit FIN=0, dann ein neuer Textrahmen. Dazu zwei Läufe ohne Verstoß: (f) Textrahmen mit FIN=0, ein Ping mit FIN=1 und Länge 5, die Fortsetzung mit FIN=1; (g) ein Steuerrahmen der Länge 125. Erwartet: (a) bis (e) je `ende` mit `halter.ziel_fehler`, kein `teil` nach dem Verstoß, Verbindung zur Attrappe geschlossen; (f) und (g) laufen durch, alle Bytes kommen an. Gemessen: Code, Zahl der Bytes nach dem Verstoß (0), Zustand der Verbindung zur Attrappe | Rahmenregeln aus: (a) bis (e) laufen durch |
| L9 | Zurückgehaltenes geht am Ende hinaus (Nachtrag Rahmen) | Attrappe sendet auf `modell.strom` nach der 101 einen Textrahmen von 40 Byte Nutzdaten, der nicht mit dem Test-Geheimnis (Marke, 16 Byte) endet, und schließt (a) zwischen zwei Rahmen, (b) mitten im nächsten Rahmen; dazu (c) `modell.aufruf` mit einem Körper von 40 Byte. Erwartet: Die Summe der `teil`-Bytes ist in (a) und (c) die Länge der Nutzdaten (40), in (b) alles, was die Attrappe gesendet hat; danach `ende` ohne Code. Gemessen: Summe der Bytes je Lauf gegen die der Attrappe | Zurückgehaltenes beim Ende verworfen: die Summe liegt um bis zu 15 Byte darunter |

**macOS (an einem Mac, höchstens 15 min je Lauf).** W3, W4, W7 und W8 gelten unverändert mit
Unix-Socket. Dazu:

| Nr | Zusage | Außentest | Gegenprobe |
|---|---|---|---|
| M1 | Schlüsselbund nur für den Halter | Der Halter legt ein Test-Geheimnis an. Ein Prozess der Person ohne Sandbox liest es mit `security find-generic-password -s agent-workbench.halter -w` und mit `SecItemCopyMatching` aus einem eigenen, anders signierten Programm. Erwartet: kein Wert ohne menschliche Bestätigung (Rückfrage, oder ohne Sitzung ein Fehler wie `errSecInteractionNotAllowed`). Gemessen: Ausgabe ohne Marke, Fehlercode | Eintrag über `/usr/bin/security add-generic-password` angelegt: Wert ohne Rückfrage lesbar (Befund 14.11.) |
| M2 | Agent erreicht Halter und Schlüsselbund nicht | Ein Agent im Seatbelt-Einschluss: `connect` auf den Socket, `security find-generic-password`, Lesen von `~/Library/Keychains/`. Erwartet: je Fehler, 0 Marken | Profil ohne Regel 2 aus Abschnitt 10: `security` erreicht den Schlüsselbund |
| M3 | Rolle an der Code-Signatur | Ein unsigniertes oder anders signiertes Programm mit dem Namen des Hubs verbindet sich: `halter.peer`; ein Prozess im Einschluss mit gültig signiertem Hub-Programm: `halter.peer` | Rolle nur über die uid: beide kommen durch |
| M4 | Kein Wert in Umgebung oder argv | `ps eww` und `ps -o args` über alle Prozesse der Person während eines Modell- und eines MCP-Aufrufs: 0 Marken | Halter reicht den Schlüssel als Umgebung an ein Kind: Marke gefunden (Befund 14.11.) |
| M5 | Vorbelegung und Pfadlänge | Am Socketpfad liegt schon etwas, oder der Pfad hat 104 Byte: Exit 74, nichts verändert | Halter löscht den Pfad: Test rot |

## 12. Prüfbarkeit der Fälle

`pruefen.py` prüft heute alle 160 sichtbaren Fälle: Paarung, Größe, strenges JSON-Lesen, RFC 8785 der Bytes, die
Rollentabelle, Stufe 7a gegen den offenen Strom der `lage`, das Schema, die Stufen 13 bis 27 gegen die `lage` des Falls,
mit Pfadprüfung, Teilbaum-Regeln, Anmeldung nach Konto, Ed25519, `digest`, Zeitform, Frist und Einmal-Kennung.
alt: 90 Fälle. Der Nachtrag B05 bringt 35 Anfragen und 5 Antworten dazu. In den bestehenden Fällen mit `modell.aufruf`
steht jetzt `"abfrage":null`, weil das Feld Pflicht ist; ihre Erwartungen sind unverändert. Die Körpergrenze von
32156332 Zeichen prüft nur das Schema (`maxLength`); Fälle genau an der Grenze folgen mit dem Bau, weil die
Referenzprüfung eine Zeile von 32 MiB Zeichen für Zeichen kanonisiert. Die `lage` beschreibt den Halter: Rolle, Konto und Vertreter-Kennzeichen des Aufrufers (Rolle `null` heißt Peer gescheitert), Uhr
und Startzeit, bekannte `kid`, eingelöste Kennungen, Gültigkeit des Nachweises, Ziele und Server aus `halter.json` und die
gehaltenen Geheimnisse (nur Name, Konto, Pseudonym; kein Wert). Die Fälle sind für alle drei Plattformen gleich.

Erst mit dem Bau prüfbar: welcher Code innerhalb der Stufen 8 bis 12 fällt (das Schema trennt sie nicht), die Stufen 28 bis
31 mit 28a, der Ablauf eines Stroms (beide Richtungen, Rahmen, die Rahmenregeln, Zurückgehaltenes am Ende, Leck-Sperre; Außentests L6, L8, L9), alles an Pipe, Socket und Schlüsselbund (Abschnitt 11) und Gleitkommazahlen in `argumente` (RFC 8785 für Zahlen prüft dann `formate`).
Die Schlüssel der Scheine in den Fällen entstehen in `bau-faelle.py` aus zwei festen Beispielsamen; sie sind Testschlüssel.

**Fälle zu `schluessel.vorhanden` nach `ziel` (Nachtrag Socket, vorhanden nach Ziel und Kette).** Neun neue Anfragen (drei gültig,
sechs mit Code, Abschnitt 4); keiner der bestehenden Fälle und keine Erwartung ändert sich. `pruefen.py` prüft Stufe 7 (`ziel` nur
für `bruecke`) und Stufe 14 (`ziel` in `halter.json`); welcher Code bei beiden oder keinem Feld fällt (Stufe 8 oder 9), prüft der
Bau, weil das Schema die Stufen nicht trennt. Dazu kommen vier Fälle des Nachtrags N1 (zwei gültig, zwei mit
`halter.pfad_verboten`, Abschnitt 4). Der Nachtrag Erststart bringt sieben Anfragen dazu (fünf mit `halter.pfad_verboten`, zwei
gültig, Abschnitt 4). Heute sind es 202 Fälle (alt: 188).

**Fälle der Form `konfig` (Nachtrag Rolle aus der Gruppe).** 36 Fälle `k-*` (8 gültig, 28 mit `halter.konfig`; alt: 29, davon 7
gültig; neu sind die sieben Fälle zu `erststart_antwortform`, Nachtrag Erststart) prüfen
`halter.json` mit der `umgebung` des Starts: `uid` des Halters und die Texte von `/etc/group` und `/etc/passwd`. `pruefen.py` prüft
strenges Lesen, Größe, `$defs/konfig`, verschiedene Gruppen je Rolle, die eigene uid in `konten`, die fünf Regeln der
Rollengruppen (Abschnitt 2.3a) und dass jede Regel von `erststart_antwortform` von `pfade` gedeckt ist (Gegenprobe gemessen: ohne
diese Prüfung meldet `pruefen.py` 1 Fehler, `k-erststart-antwortform-ungedeckt`). alt: 131 Fälle der Formen `anfrage` und `antwort` (der Satz oben nannte 130); keiner davon und keine
Erwartung ändert sich. Den Bau prüft `rust/crates/halter/tests/faelle.rs` gegen dieselben Fälle.

```sh
python3 -I rust/spec/halter/bau-faelle.py              # Fälle neu erzeugen
<python mit jsonschema> -I rust/spec/halter/pruefen.py  # Exit 0 bei 0 Fehlern; ohne jsonschema Exit 1
```

## 13. Abstimmung mit anderen Bauteilen

| Bauteil | liest oder liefert | Annahme dieses Vertrags |
|---|---|---|
| Stelle | liefert Schein (`daten`, `signatur`), `kid`, öffentlichen Schlüssel, Menschen-Nachweis; nimmt `mcp.ausgefuehrt` an | Schein mit genau den Feldern aus Abschnitt 6, `art` `mcp-schein`, Ed25519 über RFC 8785 von `daten`, Zeiten in ms, Frist höchstens 300 s |
| Formate, Vertrag 5 | Quelle `halter` steht schon in `abschnitt()`; die Zeile `mcp.schein_abgelehnt` fehlt; die Art `halter.leck_gesperrt` steht im Register (`REGISTER_FREMD`, Nachtrag Socket, vorhanden nach Ziel und Kette); ebenso `halter.pfad_verboten` (Nachtrag N1) | Vorschlag Abschnitt 9; neuer Grund für Stufe 18 (OPEN 2) |
| `peerkanal` | liefert Endpunkt, Vorbelegungssperre, Peer-Wert und Server-Prüfung auf allen drei Systemen | Peer-Wert mit den Feldern aus Abschnitt 2.1 (`sid`, `uid`, `gid`, `appcontainer_sid`, `eingeschraenkt`, `programm`, `signatur`, `sandbox`); unter Linux ist `gid` die effektive gid aus `SO_PEERCRED` und die Quelle der Rolle (Nachtrag Rolle aus der Gruppe; alt: `programm`); Server-Prüfung unter Windows am Besitzer der Pipe-Instanz (im Stand vom 20.11.2026 so festgelegt); unter Linux erkennt der Client den Halter nur an Socket-Ordner und uid |
| Plattformschicht, Modellweg | liefert die Brücke (A121) und unter Windows AppContainer und Job; Vertrag 8 Abschnitt 5 (Halterweg, Nachtrag B05) | Brücke fragt unter Linux `schluessel.vorhanden` nach `ziel` und erreicht den Halter am festen Socket `/run/user/<uid>/wb-halter/halter.sock` (nie über `$XDG_RUNTIME_DIR`; Abschnitt 2.3); Brücke läuft unter Windows als Dienst mit eigener Dienst-SID, die in `halter.json` steht; unter Linux ist sie das Rollenprogramm `/usr/lib/werkbank/werkbank-modellweg`, ein eigener Prozess neben dem Halter (alt: „unter Linux sind Brücke und Halter ein Prozess“), gestartet über dieses setgid-Programm mit der Gruppe `wb-bruecke` und im sicheren Modus (Abschnitt 2.3; der Satz dazu gehört in Vertrag 8); unter macOS ist sie ein signiertes Rollenprogramm. Sie kennt den Token nie, sendet nie einen der verbotenen Kopfnamen an den Halter (die Liste der verbotenen Kopfnamen in Abschnitt 4, 31 Namen, alt: 20, darunter `authorization`, `cookie`, `x-api-key`, `host`, `content-length`, `accept-encoding`, `sec-websocket-extensions`, `x-http-method-override` und `x-original-url`; die Brücke entfernt sie vorher, wie Vertrag 8, Abschnitt 13.2, Punkt 1), trennt den Abfrageteil in `abfrage`, sendet Körper höchstens 24117248 Byte und bildet die Codes des Halters auf eigene ab |
| Anmelden, Paket | `werkbank-anmelden copilot` schreibt `anmeldung.copilot` in die Ablage (2.3); das deb liefert `/usr/lib/werkbank/werkbank-halter`, die Benutzereinheit `wb-halter.service` und `/etc/werkbank/halter.json` (`postinst`, Form nach 2.3a), legt die Systemgruppe `wb-bruecke` an und setzt `werkbank-modellweg` auf `root:wb-bruecke` 2755 (`dpkg-statoverride`); `werkbank-pruefen` prüft Einheit, Anmeldung, Rollengruppe ohne Mitglied und den Modus des Rollenprogramms (Nachtrag Rolle aus der Gruppe) | nachgetragen am 23.11. (Anmelde-Vertrag Abschnitt 2, Paketvertrag „Nachtrag Halter“); `apt remove` lässt die Ablage liegen, `apt purge` löscht `anmeldung.copilot` und die leeren Ebenen ab `geheim` bis `agent-workbench` als die Person. alt: „Pfade tragen Anmelde- und Paketvertrag in einem eigenen Zug nach; bis dahin gilt dort `copilot.schluessel`“ |
| Hub | liefert `mcp.aufruf`, liest Antworten und Codes | Hub kennt `lauf`, `person` und die Vorschau; rechnet keinen eigenen `digest` für den Halter; läuft unter Windows als Dienst `NT SERVICE\wb-hub` (OPEN 11) |
| Oberfläche | liefert `schluessel.setzen` und `schluessel.loeschen` mit Nachweis | Oberfläche ist ein Rollenprogramm; Eingabe des Tokens nur dort |
| Einschluss macOS | schreibt das Seatbelt-Profil des Agenten | die fünf Eigenschaften aus Abschnitt 10 (Socket-Ordner, Schlüsselbund und `securityd`, fremde Prozesse gesperrt; Sandbox vererbt; kein Schlüssel und kein Deskriptor im Einschluss) |
| Einschluss Linux | bwrap-Profil, Verbotslisten in `rust/crates/einschluss/src/verbote.rs` | Socket-Ordner und Ablage nie eingebunden, eigene PID-Sicht; `.local/state/agent-workbench/halter` steht in `ANMELDE_ORDNER` (offen, OPEN 21) |
| Anforderungen | übernimmt W1 bis W10, L1 bis L9 und M1 bis M5 als Außentests zu A14, A93, A105, A106, A115, A116 | keine |

## 14. OPEN

1. **Form des Menschen-Nachweises** für `setzen` und `loeschen`: liefert die Stelle. Bis dahin prüft nur der Bau, dass
   ein ungültiger Nachweis `halter.nachweis` ergibt.
2. **Grund der Kettenzeile** für `schein_fehlt`: Vorschlag `fehlt` als Ergänzung der festen Liste; Projektentscheid. Für die
   Leck-Sperre entschieden am 25.11.2026: Grund `leck` und die feste Journalzeile (Abschnitt 9). Aufgelöst am 25.11.2026: Vertrag 5
   führt die Art `halter.leck_gesperrt` im Register (`formate`, `REGISTER_FREMD`), und der Halter schreibt die Kettenzeile
   (Abschnitt 9). alt: „Offen: Vertrag 5 nimmt die Art in sein Register auf; erst dann schreibt der Halter die Kettenzeile.“
   Aufgelöst am 25.11.2026 (Nachtrag N1): Vertrag 5 führt auch die Art `halter.pfad_verboten` im Register, und der Halter
   schreibt ihre Zeile (Abschnitt 9).
3. **Wiederverwendung der PID:** Der Halter liest unter Windows weder `pid` noch `startzeit`. Die Rolle folgt aus der SID
   im impersonierten Token und hängt nicht an der PID. Unter Linux und macOS bindet `peerkanal` über pidfd und Audit-Token.
   Unter Linux entsteht die pidfd des Halters in Fassung 1 mit `pidfd_open` auf die pid aus `SO_PEERCRED`, weil die benutzte
   Fassung von rustix kein `SO_PEERPIDFD` hat und `unsafe` verboten ist. Seit dem Nachtrag Rolle aus der Gruppe betrifft das Fenster
   nur Lebensprüfung und Protokoll, nicht die Rolle (Abschnitt 2.3). Wege: eine Fassung von rustix mit `SO_PEERPIDFD` (Antrag einer
   neuen Abhängigkeit) oder `getsockopt` in `peerkanal` (`src/sys`) mit einer `unsafe`-Ausnahme der Leitung.
4. **Bindung des Laufs an den Peer unter Windows.** `lauf` kommt in Fassung 1 aus der Anfrage des Hubs und muss zum Schein
   passen. Unter Linux misst der Halter später die Cgroup der Hub-Unit. Unter Windows fehlt ein gemessener Weg (etwa Job
   des Hubs). Der Schein bindet den Aufruf trotzdem über den `digest`.
5. **Kodierte Lecks:** Base64, Base64url und Prozentkodierung sucht die Leck-Sperre seit 25.11.2026 (Abschnitt 8); die Fälle
   sind Tests im Bau, weil das Fallformat keine Antwort des Ziels kennt (wie OPEN 20). Offen bleiben andere Kodierungen
   (JSON-Unicode-Escape einzelner Zeichen, Hex, doppelte Prozentkodierung, Teile in getrennten Köpfen). alt: alle drei offen.
6. **Schutz der Ablage im Ruhezustand:** DPAPI unter dem Dienstkonto selbst (nicht der Person) ist zulässig, aber nicht
   gemessen. Fassung 1 verlangt nur die DACL.
7. **Mail (A106):** Senden und Abrufen über den Halter brauchen eine eigene Op (`mail.senden`, `mail.abrufen`); Fassung 1
   legt nur Name und Bereich `mail.*` fest.
8. **macOS: Zugriffsliste des Schlüsselbunds** ist nicht gemessen (M1). Hält sie nicht, gilt unter macOS dieselbe Grenze
   wie unter Linux Fassung 1, und der Prüfbefehl nennt sie.
9. **macOS: launchd-Daemon unter eigenem Konto** (`_wbhalter`, eigener Schlüsselbund oder Datei 0600) schließt die Lücke
   „Prozess der Person außerhalb des Einschlusses“; er braucht Adminrechte beim Einrichten. Gleiches gilt für den
   Systemnutzer unter Linux (Einzelheiten OPEN 18).
10. **Server-Prüfung in `peerkanal` unter Windows:** aufgelöst. `peerkanal` vom 20.11.2026 prüft am Besitzer der Pipe-Instanz.
11. **Hub als Dienst unter Windows:** Der Hub-Dienst `NT SERVICE\wb-hub` startet je Sitzung einen Hub-Prozess. Wie der
    Agent im AppContainer den Hub erreicht, legt der Hub-Vertrag fest; der Halter nimmt nur die SID an.
12. **Oberfläche unter Windows** ist nur „Prozess der Person außerhalb eines Containers“. Eine stärkere Trennung bräuchte
    ein eigenes Konto für die Oberfläche; die Sperre ist bis dahin der Menschen-Nachweis.
13. **Obergrenze des Scheins und Einlösefrist:** aufgelöst (Entscheidung vom 23.11.2026). Der Schein gilt von 1 bis 86400000 ms und
    bildet die eingestellte Einlösefrist ab (Abschnitt 6). Fälle: `a-mcp-schein-frist-24h` und `s24-frist-zu-lang`.
14. **Abfrageteil nicht gemessen (Nachtrag B05).** Kein Bericht belegt, dass die Copilot CLI einen Abfrageteil sendet: Die
    Protokollzeile des Modellwegs trägt nur `pfadregel`, und das Prüfziel der Läufe vom 22.11. schneidet die Query ab. Das
    Feld `abfrage` steht trotzdem im Vertrag, weil Vertrag 8 den Abfrageteil durchlässt. Messweg im nächsten Lauf mit Konto:
    Der Modellweg zählt je gebundenem Ziel Anfragen mit und ohne Abfrageteil und die Länge des längsten Abfrageteils, ohne
    Inhalt; dazu eine Zeile im Bericht. Ergibt sich 0, kann ein späterer Schritt `abfrage` auf `null` festlegen.
15. **WebSocket nur in einem Lauf gemessen (Nachtrag B05).** `GET /responses` mit 101 ohne `permessage-deflate` steht in
    Vertrag 8 (Abschnitt 12, Projektentscheid, 20.11.2026, zweiter angemeldeter Start, Copilot CLI 1.0.88, Plan Free); die Rohdaten
    liegen in keinem der Berichte vom 22.11. Im selben Lauf ging auch ein `POST /chat/completions` hinaus. Messweg im
    nächsten Lauf mit Konto: Zählung je gebundenem Ziel nach Methode, Pfadregel und Status 101 aus der Protokollkette, dazu
    die Zahl der Rahmen je Richtung und ob eine Erweiterung angeboten wurde; keine Inhalte.
16. **Betriebsart 1 (lokale Route).** Der Schlüssel der Organisation (`modellweg.schluessel`) passt in den Bereich `maschine`
    (`modell.<ziel>`). Bis zu einem eigenen Schritt liest der Modellweg in Betriebsart 1 seine Datei selbst; nur die Cloud-Route
    geht ab Schritt I über den Halter (Projektentscheid, 23.11.2026).
17. **Lauf-Schein (A08).** A08 verlangt Modellschlüssel und Bescheinigung je Lauf beim Halter. `modell.aufruf` und
    `modell.strom` tragen in Fassung 1 keinen Lauf-Schein. Er gehört in Vertrag 7 (Gateway, unterbrochen, Befund B20); dieser
    Vertrag übernimmt ihn als Feld, sobald Vertrag 7 ihn festlegt (Projektentscheid, 23.11.2026).
18. **Systemnutzer unter Linux (zu OPEN 9).** Erst er schließt die Lücke „Prozess der Person liest die Ablage“. Er braucht
    eine Peer-Messung ohne `/proc/<pid>/exe` (seit dem Nachtrag Rolle aus der Gruppe in Schritt I über die gid gebaut; die Cgroup
    bleibt offen), `schluessel.setzen` statt Datei für
    `werkbank-anmelden` und eine Suche mit den Rechten des Aufrufers. Schritt II. **F7 b passt nicht zu Schritt II** (Projektentscheid,
    23.11.2026): Unter eigenem Konto gehört die Ablage nicht mehr der Person; `werkbank-anmelden` kann dann nicht in sie
    schreiben und darf es nicht. Offen für Schritt II: Weg der Übergabe (`schluessel.setzen` mit Menschen-Nachweis aus einem
    Programm ohne Oberfläche oder eine eigene Op), Umzug eines bestehenden `anmeldung.copilot` aus der Ablage der Person und was
    `apt purge` dann löscht. Bis dahin gilt F7 b nur für Linux Schritt I.
19. **Brücke als Vertreter unter Windows.** Der Modellweg läuft dort unter eigener Dienst-SID; ein `anmeldung.*` einer
    Person darf er erst mit einer Vertreter-Regel und Lauf-Schein (A121, OPEN 17) einsetzen. Dazu legt der Windows-Schritt die
    Zahl der Pipe-Instanzen fest: 16 reichen für 32 Verbindungen der Brücke neben Hub und Oberfläche nicht. Schritt II.
20. **Rahmenregeln und Ende des Stroms ohne Fälle (Nachtrag Rahmen).** Das Fallformat kennt Zeilen der Anfrage und der Antwort
    und die `lage` des Halters, aber keine Folge von Rahmen des Ziels und keine Summe von `teil`-Bytes; die drei Rahmenregeln
    (Abschnitt 4) und die Regel „Zurückgehaltenes geht am Ende hinaus“ (Abschnitt 8) sind darum nur als Außentests L8 und L9
    beschrieben, nicht als Fälle. Bestehende Fälle und Erwartungen bleiben. Ob das Format später um die Rahmenfolge des Ziels
    wachsen soll, entscheidet das Projekt mit dem Bau. Nicht gemessen: Ob die Copilot CLI oder der Dienst jemals Steuerrahmen mit
    Länge über 125 oder geteilte Nachrichten mit Steuerrahmen dazwischen sendet; die Regeln sind die aus RFC 6455 und
    Vertrag 8, nicht aus einer Messung.
21. **Nachzug außerhalb dieses Vertrags (Nachtrag Anmelden und Paket).** (a) Die Verbotsliste des Einschlusses unter Linux
    (`rust/crates/einschluss/src/verbote.rs`, `ANMELDE_ORDNER`) führt die Ablage noch nicht; nötig ist der Eintrag
    `.local/state/agent-workbench/halter` (Ordner relativ zum Heim, gesperrt wie ein Anmeldespeicher, auch für den eigenen
    Harness). Ohne ihn lehnt der Einschluss eine Projektwurzel `.local/state`, `.local/state/agent-workbench` oder darunter nicht
    ab; das Heim selbst und `.local` lehnt er schon heute ab, weil sie `.local/share/keyrings` decken (hergeleitet aus dem Test
    in `verbote.rs`, Zeilen 188 bis 210, Stand development vom 23.11.2026; nicht gemessen). (b) **aufgelöst** (Nachtrag Rolle aus der
    Gruppe): `$defs/konfig` in `halter.schema.json` mit den Regeln aus Abschnitt 2.3a und Fällen der Form `konfig`. alt: „Die Form von
    `halter.json` hat noch kein Schema; […] Ein Schema mit Fällen folgt mit dem Bau.“
22. **Effektive gegen reale gid nicht gemessen (Nachtrag Rolle aus der Gruppe).** Dass `SO_PEERCRED` die effektive gid trägt, ist aus
    unix(7) und dem Kernel hergeleitet. Gemessen ist im Bau nur ein Peer, dessen reale und effektive gid zugleich wechseln (`newgrp`
    auf eine Gruppe des Testnutzers). Ein echtes setgid-Programm braucht root beim Einrichten; Messweg: Außentest L3 in der Wegwerf-VM
    mit `chgrp wb-bruecke` und `chmod 2755` auf einer Kopie des Peers.
23. **NSS-Gruppen.** Der Halter und `werkbank-pruefen` lesen nur `/etc/group` (und `werkbank-pruefen` `/etc/gshadow`). Eine Gruppe aus
    LDAP oder SSSD mit demselben Namen oder derselben gid sieht keiner von beiden. Hergeleitet: Eine Mitgliedschaft über NSS gäbe einem
    Prozess die gid als supplementäre Gruppe; `newgrp` könnte sie dann zur effektiven machen. Messweg: Rechner einer Organisation mit
    zentraler Verwaltung (Außentest AL-16 im Paketvertrag).
24. **Antwort im Messlauf und Befund des Prüfbefehls (Nachtrag Erststart).** Die Leitung hat entschieden, dass die Antwort auf einem
    Pfad mit Schalter weiter durch die Leck-Sperre geht wie bisher. Trägt sie ein abgeleitetes Token (etwa `access_token`), kennt die
    Leck-Sperre es nicht, und es erreicht im Messlauf die Brücke und die CLI; die Zeile meldet es nur. Vorschlag: Bei `"zu":true`
    hält der Halter den Körper zurück und endet mit einem eigenen Code. Das braucht einen neuen Code in Abschnitt 7 und die
    Entscheidung der Leitung. Ebenso offen: `werkbank-pruefen` meldet ein gesetztes `erststart_antwortform` als Befund (Paketvertrag).
25. **Antwortform eines Stroms (Nachtrag Erststart).** `GET /responses` ist ein WebSocket (`modell.strom`); die Diagnose liest nur
    Körper von `modell.aufruf`. Wie die Form der Nachrichten eines Stroms gemessen wird (je Textrahmen, Vereinigung der Namen), ist
    nicht festgelegt. Bis dahin bleibt `GET /responses` zu.

<!-- vom Bauvorgang angefuegt -->
> Hinweis: Dieser Text nennt Werkzeuge, die in diesem Klon nicht liegen: `agent-workbench`.
> Sie gehoeren zum Setup der Maschine, aus der dieser Baum stammt; hier fehlen
> sie mit Absicht. Wer sie braucht, schreibt sich seine eigenen.
> *Named here, not shipped here.*
