# Nachtrag zu Vertrag 4: Such-Pipe unter Windows

Stand 20.11.2026. Auftrag: Projektentscheid, 20.11.2026. Dieser Nachtrag ergänzt
Vertrag 4 (`../README.md`) für Windows. Wo hier nichts anderes steht, gilt Vertrag 4 unverändert.

Er setzt drei Verträge voraus:

| Vertrag | Ablage | Was dieser Nachtrag daraus nimmt |
|---|---|---|
| Windows-Einschluss | `rust/spec/windows-einschluss/` (Zweig, Stand vom 20.11.2026, noch nicht auf `development`) | AppContainer je Person und Projekt, Job je Lauf, Plattformdienst mit `peer`, Start des Such-Prozesses (Schritt 7a), Name der Pipe |
| `peerkanal` | `rust/spec/peerkanal/`, Crate `rust/crates/peerkanal` (Vertrag in Arbeit) | Pipe anlegen mit erster Instanz, Messung des Peers, Prüfung des Servers |
| 5 Protokollzeile | `rust/spec/protokollzeile/` | die gemeinsame Zeilenart `kanal.abgewiesen` (abgestimmt 20.11.2026, Eintrag folgt mit `anhaengen_abgewiesen`) |

Anforderungen: A34 (gefiltert wird vor der Ausgabe), A61 (dieselbe Rangliste wie unter Linux und macOS), A115 (Peer an
der Named Pipe über die SID, Server über den Besitzer der Instanz), A03 und A123 (AppContainer je Person und Projekt,
Job je Lauf), A120 (kein ungeschützter TCP-Dienst).

„Gemessen“ verweist auf den Probenplan der Windows-Plattform (Fragen 1 bis 11, eine Windows-11-VM, 18.11.). Alles andere ist
hergeleitet und steht als Fall da, der in der VM zu messen ist.

## 1. Was gleich bleibt (Auflage 1)

- **Die Route kommt nie aus der Anfrage.** Der Starter gibt Route und Sitzung beim Start in argv. Der Agent ändert
  weder argv noch Umgebung des Such-Prozesses. Alle Anfrageregeln von Vertrag 4 gelten Byte für Byte: Felder, Zeilen,
  1 MiB, Antworten, Tor, Protokollzeile „Ausgabe gesperrt“. Ein Feld `route`, `kunde`, `sitzung`, `klasse` macht die
  Anfrage auch unter Windows ungültig.
- Bereitschaftszeile `{"bereit":true}` und LF auf stdout, Meldungen nach stderr, stdin wird nicht gelesen.
- Die Exitcodes 64, 78 und 74 behalten ihre Bedeutung und ihre Reihenfolge. Neu sind nur Gründe innerhalb von 64 und 74.
- Die Rangliste ist dieselbe wie unter Linux und macOS (A61). Der Transport ändert an `L` nichts.

## 2. Aufruf unter Windows

`rangkern sitzung <vault> --socket <pipe> --paket-sid <sid> --route <schlüssel> --sitzung <schlüssel> --einstellungen <datei> --protokoll <ordner>`

Dazu die Optionen für Vektoren und Einbettung wie in Vertrag 4. Neu oder anders als unter Linux und macOS:

| Option | Regel unter Windows |
|---|---|
| `--socket` | Pflicht. Genau `\\.\pipe\wb-such-` und 32 Zeichen `0-9a-f`. Ein Dateipfad, ein ferner Name (`\\host\pipe\…`), Großbuchstaben oder eine andere Länge sind Exit 64 |
| `--paket-sid` | Pflicht, neu. Die AppContainer-SID des Projekts, Form `S-1-15-2-` und genau sieben Dezimalzahlen ohne führende Null, jede höchstens 4294967295 (wie beim Admin-Helfer des Einschlusses) |
| `--socket-rechte` | Unter Windows ein Aufruffehler, Exit 64. Grund: Die Rechte setzt allein die DACL aus Abschnitt 3. Eine Option, die still nichts tut, lässt einen Starter glauben, er habe etwas gesetzt |

Unter Linux und macOS ist `--paket-sid` eine unbekannte Option und ergibt Exit 64.

Exit 74 unter Windows: Die erste Instanz lässt sich nicht anlegen, weil der Name belegt ist (gemessen: Fehler 5 mit
`FILE_FLAG_FIRST_PIPE_INSTANCE`, Frage 1), oder eine der weiteren Instanzen beim Start scheitert. Eine fremde Pipe unter
dem Namen bleibt unverändert. Wie unter Unix gilt: kein Byte auf stdout.

Den Namen erzeugt der Starter je Sitzung aus 16 Byte Zufall (`BCryptGenRandom`) und gibt ihn Such-Prozess und
`such-mcp` in argv (Windows-Einschluss, Schritt 7a).

## 3. Die Pipe: Anlage und Sicherheit (Auflagen 4 und 5)

**Entscheidung zu Auflage 4: Der Name ist nicht geheim. Geschützt ist die Pipe allein durch DACL, erste Instanz und die
Prüfung des Peers je Verbindung.** Der Zufall im Namen verhindert nur, dass zwei Sitzungen kollidieren und dass jemand
den Namen einer künftigen Sitzung vorab belegt. Gründe:

1. Der Name steht in argv von `such-mcp` und in der MCP-Einstellung des Harness. Beides liest der Agent. Ein zweiter Lauf
   desselben Projekts teilt die AppContainer-SID (gemessen, Frage 11) und hat dieselben Rechte am Arbeitsordner.
2. Ob ein Prozess im AppContainer die Namen unter `\\.\pipe\` aufzählen kann, ist nicht gemessen. Ein Schutz, der auf
   dieser Unkenntnis beruht, wäre nicht prüfbar.
3. Vorbelegen schadet höchstens der Verfügbarkeit: Der Such-Prozess endet mit 74, der Starter startet den Agenten nicht
   (Schritt 7a). Ein falscher Server erreicht so nie einen Agenten dieser Sitzung.

**Anlage** durch `peerkanal` (Vertrag `rust/spec/peerkanal/`, Stand vom 20.11.2026). Der Such-Prozess ruft
`Server::anlegen` mit dieser Vorgabe:

| Feld | Wert | Grund |
|---|---|---|
| `erlaubt` | genau `[{sid: U, appcontainer_sid: P}]` | `U` ist die TokenUser-SID des Such-Prozesses (die Person), `P` der Wert von `--paket-sid`. Die Person ohne Container passt nicht auf das Paar |
| `instanzen` | 4 | vier Verbindungen zugleich; eine fünfte wartet (`WaitNamedPipe`) oder scheitert mit 231. Die Leerlauffrist aus Klarstellung 13 von Vertrag 4 (Entscheid vom 24.11.2026) gilt auch für die Pipe: kein Byte in 10 s, dann schließt der Prozess ohne Antwort und der Platz wird frei |
| `hallo_frist` | 2 s (Vorgabe von `peerkanal`) | |
| `prozess_pflicht` | `true` | `pid` und `startzeit` braucht der Vergleich mit dem Plattformdienst |
| `job` | keiner | Der Such-Prozess hält kein Job-Handle; `im_job` bleibt `null` |

Sichtbar ist davon: Die erste Instanz trägt `FILE_FLAG_FIRST_PIPE_INSTANCE` (Auflage 5), dazu
`PIPE_REJECT_REMOTE_CLIENTS` und Bytemodus. Der Name bleibt belegt, solange der Prozess lebt: Bevor eine verbundene
Instanz übergeben wird, wartet schon die nächste, oder die verbundene hält den Namen. Weitere Instanzen legt nur die SID
`U` an. Die Person gilt dabei als vertrauenswürdig; sie könnte den Vault ohnehin lesen. Ein Agent legt keine Instanz an
(`w-instanz-agent`).

**Sicherheitsbeschreibung**, wie `peerkanal` sie für dieses `erlaubt` setzt:

```
O:<U>D:P(A;;0x12019f;;;<U>)(A;;0x12019b;;;<P>)S:(ML;;NW;;;LW)
```

- Geschützte DACL ohne vererbte Einträge, genau zwei ACEs. `0x12019f` ist Lesen und Schreiben samt
  `FILE_CREATE_PIPE_INSTANCE` (0x4). `0x12019b` ist dasselbe **ohne** 0x4, also ohne Anlegen einer Instanz, ohne
  `WRITE_DAC` und `WRITE_OWNER`.
- Für einen Prozess im AppContainer prüft Windows die Nutzer-SID und die Paket-SID; er bekommt die Schnittmenge, also
  höchstens `0x12019b` (hergeleitet, `peerkanal` S14).
- Der Besitzer ist ausdrücklich `U`. Grund: Unter einem Token mit Adminrechten wäre der Besitzer sonst
  `BUILTIN\Administrators`, und die Prüfung in `such-mcp` (Abschnitt 6) schlüge fehl.
- Das Label `Low` mit `NO_WRITE_UP` lässt den Agenten (niedrige Integrität) schreiben. Ohne Label gilt die Pipe als
  `Medium`, und der Agent könnte nicht schreiben (hergeleitet; OPEN 2).
- Kein ACE für SYSTEM, Administratoren, `ALL APPLICATION PACKAGES` oder ein anderes Konto.

## 4. Prüfung des Peers je Verbindung (Auflagen 2 und 3)

**Nur der Agent dieser Sitzung erreicht die Suche.** Agent heißt: ein Prozess im Job des Laufs dieser Sitzung, also der
Harness und jedes Kind davon.

**Leitung.** Der Client prüft den Server (Abschnitt 6) und sendet dann das Hallo `pk1` und LF. Erst danach misst der
Server; unter Windows muss er vor dem Impersonieren lesen (sonst Fehler 1368, gemessen). Nimmt er an, sendet er dasselbe
Hallo zurück. Danach gelten die Zeilen von Vertrag 4. Weist er ab, schließt er die Verbindung, ohne ein Byte zu senden,
auch ohne Hallo.

**Prüfung.** Der erste zutreffende Schritt nennt den Grund:

| Schritt | Wer | `grund` der Zeile |
|---|---|---|
| Kein Hallo binnen 2 s, oder andere Bytes als `pk1` und LF | `peerkanal` | `pk.hallo_fehlt`, `pk.hallo_falsch` |
| Kennung nicht messbar, Prozess nicht zu öffnen | `peerkanal` | `pk.messung_gescheitert`, `pk.prozess_nicht_offen` |
| Prozess beendet oder nach dem Verbinden gestartet | `peerkanal` | `pk.prozess_beendet`, `pk.startzeit_weicht_ab` |
| Token mit Restricting SIDs, andere SID, anderes Paar aus SID und Paket-SID | `peerkanal` | `pk.eingeschraenkt`, `pk.sid_nicht_erlaubt`, `pk.appcontainer_falsch` |
| Plattformdienst nicht erreichbar, Besitzer seiner Pipe nicht die SID von `NT SERVICE\wb-plattform`, keine Antwort binnen 500 ms, oder ein Fehler außer `dienst.peer_unbekannt` | Such-Prozess | `such.dienst_stumm` |
| Antwort `dienst.peer_unbekannt`, oder `pid`, `start` oder `sid` der Antwort weichen von der Messung ab | Such-Prozess | `such.lauf_unbekannt` |
| `sitzung` der Antwort ist nicht der Wert von `--sitzung` | Such-Prozess | `such.sitzung_fremd` |

Die Frage an den Plattformdienst ist `{"op":"peer","pid":<pid>}` auf `\\.\pipe\wb-plattform` (Windows-Einschluss,
Steuer-Pipe). Seine Antwort nennt den Lauf, dessen Job den Prozess enthält. Der Such-Prozess hält kein Job-Handle; er
fragt deshalb den Dienst, der den Job allein hält. Das Projekt hat am 20.11. bestätigt, dass diese Prüfung Pflicht ist.

**Die drei letzten Schritte liegen vor dem Hallo des Servers.** `peerkanal` hat dafür den Haken des Aufrufers in
Schritt 7 (`rust/spec/peerkanal/README.md` Zeilen 186 bis 189, Zusage S15; bestätigt am 21.11.2026): `annehmen`
gibt `Annahme::Vorlaeufig` zurück, ohne ein Byte an den Client. Der Such-Prozess fragt dann den Plattformdienst `peer`
mit pid und Startzeit aus der Prozessbindung, die `peerkanal` während des Hakens hält. Erst `bestaetigen()` sendet das
Hallo; bei einer der drei Abweisungen ruft er `ablehnen` und schließt ohne Antwort. Die Pflicht, den Plattformdienst zu
fragen, steht in diesem Nachtrag, nicht in `peerkanal`. Der Grund der Zeile ist der Grund aus der Tabelle oben
(`such.*`), nicht `pk.aufrufer_abgewiesen`.

**Abweisung.** Kein Byte an die Verbindung, dann das Ende der Verbindung. Nichts aus den gelesenen Bytes geht in ein
Protokoll. Der Prozess läuft weiter und nimmt die nächste Verbindung an.

**Protokollzeile (Auflage 3).** Je abgewiesener Verbindung genau eine Zeile in der Kette `brain-<JJJJ-MM>.jsonl` im
Ordner `--protokoll`, über die Bibliothek von Vertrag 5 (`anhaengen_abgewiesen`, gleiche Sperre wie „Ausgabe
gesperrt“). Die Zeilenart ist gemeinsam für Such-Prozess, Stelle und Weiterreicher (Projektentscheid, 20.11.2026):

```
{"aktionsname":"kanal.abgewiesen","ergebnis":"abgewiesen","fassung":2,"grund":"pk.appcontainer_falsch","kanal":"such","route":"cloud-beispiel","sitzung":"s-1","vorher":"<64 hex>","zeit":"2026-11-20T15:00:00.000Z"}
```

- Genau diese neun Felder, RFC 8785, geschlossen. `kanal` ist hier immer `such`; `route` und `sitzung` kommen aus dem
  Start. (Für die Stelle sind beide `null`; der Such-Prozess setzt sie immer.)
- `grund` ist ein fester Grund von `peerkanal` mit der Vorsilbe `pk.` oder ein Grund des Kanals mit der Vorsilbe
  `such.`; die Vorsilbe gleicht dem Feld `kanal`. Für `such` gelten die drei Gründe oben (Projektentscheid, 20.11.2026).
  Einen unbekannten Grund lehnt `kette pruefe` als `form_falsch` ab.
- Die Zeile nennt keine SID, keine PID, keinen Konto- oder Prozessnamen und nichts aus den gelesenen Bytes (A12, A104).
  Die SID eines fremden Kontos benennt eine Person.
- Scheitert das Schreiben, bleibt die Abweisung, und der Prozess meldet das auf stderr ohne Inhalt.

**Fremdes Konto und fremder AppContainer.** Die DACL lässt sie nicht an die Pipe: `CreateFile` scheitert mit Fehler 5
(gemessen für ein fremdes Konto, Frage 1). Der Such-Prozess sieht diese Versuche nicht; `peerkanal` meldet kein Ereignis,
und er schreibt keine Zeile. Das ist eine Entscheidung dieses Nachtrags (OPEN 1): Eine Zeile für jedes fremde Konto
verlangte eine DACL, die alle Konten an die Pipe lässt, und verlagerte den Schutz vom Kern in den Code des Prozesses.
`pk.sid_nicht_erlaubt` bleibt als zweite Sperre für eine falsch gesetzte DACL; sichtbar wird es erst mit einem Mutanten
beim Bau.

**Reicht `appcontainer_sid` und der Job, wenn zwei Sitzungen denselben AppContainer nutzen?** Ja, für den Zugriff auf
die Pipe, mit dem Job als entscheidendem Teil:

- Die SID allein reicht nicht. Zwei Läufe desselben Projekts teilen sie (gemessen, Frage 11). Ohne Job-Prüfung bekäme
  ein Lauf mit Route `cloud` die Treffer eines Laufs mit Route `lokal`.
- Den Job trennt der Kern: Jeder Lauf hat einen eigenen Job, den nur der Plattformdienst hält. Ein Prozess verlässt ihn
  nicht (`KILL_ON_JOB_CLOSE` ohne `BREAKAWAY_OK`, gemessen, Fragen 2 und 10a), und in einen fremden Job kommt er ohne
  dessen Handle nicht. Kinder des Agenten liegen im selben Job und gelten zu Recht als Agent der Sitzung.
- Eine wiederverwendete PID verwechselt die Prüfung nicht, weil `peerkanal` den Prozess sofort öffnet und die
  Startzeit mit der des Dienstes vergleicht. Erzwungen gemessen ist das nicht (OPEN 5).
- Der Socket trennt nur die Suchantworten je Sitzung. Die Arbeitsordner schützt die Ordnermarke von Vertrag 1 (A156). Das gilt unter Windows genauso.

## 5. Ende und Schutz des Prozesses

- Windows kennt kein SIGTERM. Der Such-Prozess endet mit dem Job des Starters (`KILL_ON_JOB_CLOSE`, Windows-Einschluss)
  oder durch `TerminateProcess`. Er muss dafür nichts aufräumen: Die Pipe verschwindet mit dem letzten Handle. Danach
  scheitert `CreateFile` auf den Namen mit Fehler 2. Einen Exitcode legt der Nachtrag unter Windows nicht fest.
- Statt `PR_SET_DUMPABLE` gilt: Der Such-Prozess läuft als Person mit mittlerer Integrität außerhalb des AppContainers.
  Ein Prozess im AppContainer öffnet ihn nicht (`OpenProcess` scheitert mit Fehler 5; hergeleitet, W1).
  Vault, Einstellungsdatei und Protokoll liegen außerhalb der Freigaben des Containers.

## 6. `such-mcp` unter Windows

- **Aufruf:** `such-mcp --socket <pipe>`, mit dem Namen der Form aus Abschnitt 2. Unter Windows ist die Option Pflicht,
  denn es gibt keinen festen Pfad wie `/run/brain/such.sock`. Ohne sie, mit einem anderen Namen oder mit jeder anderen
  Angabe: Exit 64 ohne Ausgabe.
- **Stand des Codes (25.11.2026):** `such-mcp` prüft unter Windows Aufruf und Pipe-Namen nach diesem Abschnitt. Es
  verbindet sich noch nicht, weil `peerkanal` die eigene Kennung des Prozesses nicht nennt, die als erwarteter Besitzer
  der Pipe gilt; bis dahin antwortet es mit „Suche nicht erreichbar“ und `isError: true` (fail closed). Die Fälle `wm-*`
  mit einer Antwort des Servers sind darum heute rot; die übrigen sind grün.
- **Woher der Name kommt:** Der Starter schreibt ihn je Sitzung in die MCP-Einstellung, mit der er den Harness startet
  (argv des Werkzeugs). Ändert der Agent den Namen, erreicht er höchstens eine fremde Pipe; die weist ihn nach DACL oder
  Abschnitt 4 ab.
- **Öffnen:** `CreateFileW` mit `GENERIC_READ | FILE_WRITE_DATA`, nicht `GENERIC_WRITE` (das verlangte 0x4 und
  scheiterte an der DACL), und mit `SECURITY_SQOS_PRESENT | SECURITY_IDENTIFICATION`. So kann ein falscher Server den
  Agenten nur erkennen, nicht in seinem Namen handeln. Bei 231 wartet es mit `WaitNamedPipe`, innerhalb der 10 s.
- Es bleibt im Bytemodus und ruft `SetNamedPipeHandleState` nicht (Hinweis, 20.11.2026).
- **Server prüfen (A115):** Vor dem ersten Byte liest es den Besitzer der Instanz (`GetSecurityInfo`,
  `OWNER_SECURITY_INFORMATION`) und vergleicht ihn mit der TokenUser-SID des eigenen Tokens. Im AppContainer ist das die
  Person. Die erwartete SID kommt nur aus dem eigenen Token, nie aus argv, Umgebung oder Einstellung.
  `GetNamedPipeServerProcessId` nutzt es nicht (Berichtigung, gemessen Frage 10: gegen einen Dienst unter
  `NT SERVICE` Fehler 5).
- Weicht der Besitzer ab, schreibt es nichts an die Pipe; das Ergebnis ist „Suche nicht erreichbar“ mit `isError: true`.
- **Hallo:** Nach der Prüfung des Servers sendet es `pk1` und LF und wartet auf dasselbe Hallo, innerhalb der 10 s.
  Schließt der Server ohne Hallo, ist das Ergebnis „Suche nicht erreichbar“. Erst dann schreibt es die Anfrage.
  Das Hallo gilt auch unter Linux und macOS, sobald der Such-Prozess dort `peerkanal` nutzt (OPEN 5).
- Alles andere gilt wie in Vertrag 4: Methoden, Werkzeug `brain_suche`, Prüfung der Argumente, eine Verbindung je
  Aufruf, eine Antwortzeile, Frist 10 s.

## 7. Fälle und Läufer

| Datei | Inhalt |
|---|---|
| `bau_faelle.py` | Fallbauer. Erzeugt `faelle/`, `erwartet/` und `mcp_faelle.json` aus `../sichtbar/`, `../mcp/faelle.json` und den Windows-Fällen im Skript, und prüft deren Form. `--pruefe` vergleicht den Stand auf der Platte mit dem Bau, ändert nichts |
| `faelle/<name>.json`, `erwartet/<name>.json` | Fälle für den Such-Prozess. `v4-*` sind die Fälle von Vertrag 4 über die Pipe, `w-*` die neuen |
| `mcp_faelle.json` | Fälle für `such-mcp`: alle von Vertrag 4 über die Pipe und die neuen `wm-*` |
| `pruefe_windows.py` | Läufer, nur Standardbibliothek (`_winapi` und `ctypes` unter Windows). `RANGKERN_BIN=<programm> python pruefe_windows.py such [fall …]`, `MCP_BIN=<programm> python pruefe_windows.py mcp [fall …]` |

Der Fallbauer übernimmt jeden Fall aus `../sichtbar/` und `../mcp/faelle.json`, auch Probelauf und Zählung der Marker
(`marker_zaehlung`, `stderr_zeile`). Ausgenommen sind die Fälle `r-*` (Rückfall der Einbettung); sie folgen mit dem
Such-Prozess unter Windows. Ausgenommen sind auch die acht Fälle vom 23.11.2026, die
Erweiterungen von `../pruefe.py` brauchen (`nur_v4` in `bau_faelle.py`). Ändert sich Vertrag 4, läuft `bau_faelle.py` neu; bis dahin ist `--pruefe` rot.
Stand 23.11.2026: 96 Fälle für den Such-Prozess (65 `v4-*`, 31 `w-*`) und 34 für `such-mcp` (26 und 8); am 20.11.2026
waren es 68 (37 und 31) und 29 (21 und 8). Die Rechte
0600 der Zähldatei prüft der Läufer nur unter POSIX; unter Windows regelt sie eine ACL, die noch offen ist.

Jeder Fall nennt in `orte`, wo er läuft:

| Ort | Bedeutung |
|---|---|
| `attrappe` | auf Linux oder macOS gegen eine Attrappe, die die Pipe durch einen Unix-Socket nachbildet; prüft den Läufer, nicht Windows |
| `windows` | unter Windows mit Python als Person, ohne zweites Konto und ohne AppContainer |
| `vm-konto` | braucht ein zweites lokales Konto |
| `vm-container` | braucht einen Client im AppContainer eines Laufs und den Plattformdienst |

**Kennungen der Clients.** Ein Schritt nennt mit `peer`, wer verbindet: `agent` (Vorgabe), `person`, `andere-sitzung`,
`ohne-lauf`, `dienst-stumm`, `zeit-falsch`, `anderer-container`, `fremdes-konto`. Unter Windows verbindet der Läufer als
`person` selbst. Für jede andere Kennung braucht er einen Client-Befehl aus der Umgebung,
`PEER_<KENNUNG>` (Großbuchstaben, `-` als `_`) als JSON-Liste; der Läufer hängt den Pipe-Namen an. Der Befehl verbindet
mit `GENERIC_READ | FILE_WRITE_DATA` und Identifikation, schreibt eine Zeile `{"oeffnen_fehler":null}` oder
`{"oeffnen_fehler":<Windows-Fehler>}` auf stdout und reicht danach stdin an die Pipe und die Pipe an stdout weiter, bis
eine Seite endet. `python pruefe_windows.py relais <pipe>` ist dieser Client für ein Konto ohne AppContainer.
Der Relais-Client reicht nur Bytes weiter; das Hallo `pk1` und LF sendet der Läufer selbst als erste Zeile, außer in
den Fällen, die ein fehlendes oder falsches Hallo prüfen.
Für `w-prozess-gesperrt` und `w-instanz-agent` nennt `PEER_PROBE_AGENT` einen Befehl im AppContainer eines Laufs. Der
Läufer hängt `prozess <pid>` oder `instanz <pipe>` an. Der Befehl öffnet die PID mit
`OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_VM_READ)` oder legt mit `CreateNamedPipeW` eine Instanz unter
dem Namen an und gibt `{"fehler":<Windows-Fehler oder null>}` aus.

**Einrichtung in der VM** (ordnet das Projekt; dieser Vertrag startet keine VM):

1. Python 3.12 oder neuer für die Person. Ohne Fremdpakete.
2. Zweites lokales Konto `wbtest-b` ohne Adminrechte, mit ssh-Zugang auf `localhost` (eine volle Anmeldung; gemessen:
   `Start-Process -Credential` taugt nicht). `PEER_FREMDES_KONTO=["ssh","wbtest-b@localhost","python","<pfad>\\pruefe_windows.py","relais"]`.
   Für `wm-fremder-besitzer` dasselbe mit `standin` statt `relais` als `STANDIN_FREMDES_KONTO`.
3. Für `vm-container`: der Plattformdienst des Einschlusses oder seine Probe (`dienst10.c` aus `win-bfe`), ein Projekt
   mit AppContainer-SID in `PAKET_SID`, zwei Läufe `s-test-1` und `s-test-2` desselben Projekts, ein Prozess im
   AppContainer ohne Lauf, ein zweites Projekt. In jedem läuft ein nativer Relais-Client (CPython mit ctypes startet im
   AppContainer nicht, gemessen). Für `dienst-stumm` ist der Dienst angehalten.

**Was der Läufer prüft:** Exit beim Start; die Bereitschaftszeile; je Anfrage die Antwort wie `../pruefe.py`; bei einer
Abweisung, dass binnen der Frist kein Byte, auch kein Hallo, und dann das Ende der Verbindung kommt; bei einer
Annahme das Hallo des Servers; die verbotenen Marken; alle
Protokollzeilen mit genau ihren Feldern je `aktionsname` und die Kette (`vorher` ist der sha256 der vorigen Zeile, die
erste trägt 64 Nullen); unter Windows Besitzer, DACL und Label der Pipe; das Ende der Pipe nach dem Ende des Prozesses.

**Gegenprobe ohne Windows** (gemessen 20.11., siehe Messbericht): Der Fallbauer prüft die Form. Eine Attrappe
außerhalb des Repos bildet die Pipe auf macOS durch einen Unix-Socket nach, liest das Hallo, misst den Peer aus einer Datei des
Läufers statt aus `peerkanal` und gibt die Suche an die Attrappe von Vertrag 4 weiter. Mutanten der Attrappe zeigen, dass jeder
Windows-Fall rot werden kann.

**Nur unter Windows prüfbar:** Besitzer, DACL und Label (`w-pipe-sicherheit`); erste Instanz und volle Instanzen
(`w-e-pipe-belegt` gegen echte Pipes, `w-instanz-agent`); Fehler 5 für fremdes Konto und fremden Container; die
Impersonation mit Stufe Identifikation (`wm-identifikation`); die Prüfung des Besitzers in `such-mcp`
(`wm-fremder-besitzer`); `OpenProcess` aus dem Container (`w-prozess-gesperrt`); alle Fälle mit dem Peer `agent`, denn
der braucht einen echten Lauf.

## 8. Offen

1. **Zeile für fremde Konten.** Entschieden ist hier: DACL weist ab, keine Zeile. Option B: DACL für alle
   angemeldeten Konten, Abweisung und Zeile durch den Prozess. B zeigt jeden fremden Versuch in der Kette, öffnet die
   Pipe aber jedem lokalen Konto bis zum Hallo und erlaubt ihm, die vier Instanzen zu belegen. Empfehlung: A.
   Entschieden ist A (Projektentscheid, 20.11.2026). **Betrieb:** Fremde Versuche zeigt nur die Objektüberwachung von Windows. Ein Admin
   braucht dafür zwei Dinge: die Überwachungsrichtlinie für Objektzugriff (`auditpol /set /subcategory:<Unterkategorie>
   /failure:enable`) und einen Überwachungseintrag für Fehlschläge, entweder in der SACL der Pipe oder als globale SACL
   (`auditpol /resourceSACL`). Einen Überwachungseintrag in die SACL der Pipe setzt nur ein Prozess mit
   `SeSecurityPrivilege`, also nie der Such-Prozess selbst. Nicht gemessen sind die Unterkategorie (Dateisystem oder
   Kernelobjekt) und ob die globale SACL Named Pipes erfasst. Das misst das Projekt in der VM, bevor der Satz als Anleitung gilt.
2. **Haken in `peerkanal`: erledigt (21.11.).** `Annahme::Vorlaeufig` mit `bestaetigen()` und `ablehnen()` steht im
   Vertrag `peerkanal` (Zeilen 186 bis 189). Offen bleibt nur die Crate `rust/crates/peerkanal`; das Projekt plant sie als
   Windows-Schritt nach dem Fix-deb. Bis dahin baut Vault den Such-Prozess unter Windows nicht.
3. **Gemessen ist** nur der Peer über die SID, die erste Instanz und die Abweisung eines fremden Kontos. Nicht gemessen
   sind der Zugriff des Agenten mit Paket-SID und Label in der DACL, Identifikation statt Impersonation, die Frist des
   Hallos und `OpenProcess` aus dem Container.
4. `anhaengen_abgewiesen`, die Gründe `such.*` (abgestimmt) und die Prüfung der Zeilenart in `kette pruefe`
   baut das Projekt.
5. Unter Linux und macOS spricht der Such-Prozess heute ohne Hallo. Nutzt er dort `peerkanal`, ändern sich Vertrag 4 und
   seine Fälle (Hallo vor der ersten Zeile). Das entscheidet Vault mit dem Bau.

<!-- vom Bauvorgang angefuegt -->
> Hinweis: Dieser Text nennt Werkzeuge, die in diesem Klon nicht liegen: `brain`.
> Sie gehoeren zum Setup der Maschine, aus der dieser Baum stammt; hier fehlen
> sie mit Absicht. Wer sie braucht, schreibt sich seine eigenen.
> *Named here, not shipped here.*
