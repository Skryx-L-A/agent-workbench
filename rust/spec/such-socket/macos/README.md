# Nachtrag zu Vertrag 4: Such-Socket unter macOS

Stand 21.11.2026. Auftrag: Projektentscheid, 21.11.2026 (Meilensteinplanung 5.7, M2m), abgestimmt
(macOS-Einschluss, 21.11.2026) und Windows-Einschluss. Dieser Nachtrag ergänzt Vertrag 4 (`../README.md`) für
macOS. Wo hier nichts anderes steht, gilt Vertrag 4 unverändert, auch alle Fälle von Vertrag 4.

Er setzt zwei Verträge voraus:

| Vertrag | Ablage | Was dieser Nachtrag daraus nimmt |
|---|---|---|
| macOS-Einschluss | `rust/spec/macos-einschluss/README.md` (Fassung 1, auf `development` seit dem 22.11.2026) | dieselbe uid für Agent und Such-Prozess (Abschnitt „Prozesse (E3)“), Ordner und Pfad des Sockets (Abschnitt „Such-Socket“), Regel `path-literal` im Profil (Abschnitt „Profil“), Gegenproben U1 bis U4 (Abschnitt „Messproben“) |
| 5 Protokollzeile | `rust/spec/protokollzeile/` | die gemeinsame Zeilenart `kanal.abgewiesen` wie unter Windows, mit zwei Gründen `such.*` dieses Nachtrags (Abschnitt 4) |

Anforderungen: A34 (gefiltert wird vor der Ausgabe), A61 (dieselbe Rangliste wie unter Linux und Windows), A03
(Einschluss).

„Gemessen“ verweist auf die Messungen im Entwurf des macOS-Einschlusses (Messlauf 1 am 20.11., Messlauf 2 am 21.11.)
und auf die Läufe gegen das Programm am Mac (Abschnitt 7). Alles andere ist hergeleitet und steht als Fall da, der am Mac zu
messen ist.

## 1. Was gleich bleibt

- **Die Route kommt nie aus der Anfrage.** Der Starter gibt Route und Sitzung beim Start in argv. Alle Anfrageregeln von
  Vertrag 4 gelten Byte für Byte: Felder, Zeilen, 1 MiB, Antworten, Tor, Protokollzeile „Ausgabe gesperrt“, Probelauf
  mit Marker-Zählung. Ein Feld `route`, `kunde`, `sitzung` oder `klasse` macht die Anfrage ungültig.
- **Transport:** Unix-Socket (Stream) wie unter Linux, ohne Hallo. Nach der Annahme gelten sofort die Zeilen von
  Vertrag 4. `such-mcp` spricht unter macOS dasselbe Protokoll wie unter Linux.
- **Linux bleibt unverändert.** Die Prüfung des Peers in Abschnitt 4 gilt nur unter macOS. Ob Linux sie ebenfalls
  bekommt oder alle drei Systeme auf `peerkanal` mit Hallo wechseln, entscheidet sich nach M1 (OPEN 1).

## 2. Aufruf unter macOS

`rangkern sitzung` nimmt dieselben Optionen wie unter Linux. Abweichend gilt:

- **`--socket <pfad>`** ist ein absoluter Pfad mit höchstens 103 Byte. `sun_path` fasst unter macOS 104 Byte, das NUL
  zählt mit. Ein relativer Pfad oder ein längerer Pfad ist ein Aufruffehler, Exit 64.
- **`--socket-rechte`** nimmt unter macOS nur `0600`. `0666` ist ein Aufruffehler, Exit 64. Grund: Unter macOS laufen Agent
  und Such-Prozess immer unter derselben uid. Den Fall einer eigenen Kennung des Agenten (Drehbuch E3) gibt es
  hier nicht, und 0666 öffnete den Socket jeder anderen Kennung, die den Ordner erreicht.
- **`PR_SET_DUMPABLE`** gibt es unter macOS nicht. Abschnitt 5 nennt, was an seine Stelle tritt.

Die Reihenfolge der Prüfungen beim Start und die Exitcodes 64, 78 und 74 bleiben wie in Vertrag 4. Abschnitt 3 fügt
Exit 74 zwei Fälle hinzu.

## 3. Ordner und Socket

**Wo der Socket liegt.** Unter macOS gibt es keinen festen Pfad wie `/run/brain/such.sock`. Der Starter legt je
Sitzung einen Ordner mit Modus 0700 kurz unter dem Heim an, etwa `~/.wbs/<sitzung>/`, und gibt dem Such-Prozess
`--socket <ordner>/such.sock`. Denselben Pfad bekommt `such-mcp` mit `--socket` (Abschnitt 6) und das Profil des
Einschlusses als einzigen erlaubten Unix-Socket:
`(allow network-outbound (remote unix-socket (path-literal "<such_socket>")))` (macOS-Einschluss-Vertrag, Abschnitt „Profil“).

**Prüfungen vor `bind`, in dieser Reihenfolge; die erste Verletzung endet mit Exit 74, und am Pfad bleibt alles
unverändert:**

1. Der Elternordner des Sockets ist ein Ordner, kein Verweis, gehört der euid des Such-Prozesses und hat genau den
   Modus 0700. Neu gegenüber Linux. Grund: Unter macOS trennt nur dieser Ordner den Socket von anderen Kennungen; unter
   Linux bindet der Starter den Socket zusätzlich in den Einschluss ein.
2. Am Socketpfad liegt nichts, auch kein Verweis ins Leere. Unter macOS folgt `bind` einem Verweis ins Leere und legt
   den Socket am Ziel an (gemessen am 20.11. am Mac); deshalb prüft der Prozess mit `lstat` vor `bind`, wie in
   Vertrag 4.

Danach legt der Prozess den Socket mit `umask 077` an und setzt ihn auf 0600, wie in Vertrag 4.

**Was der Ordner nicht leistet.** Andere Prozesse derselben uid erreichen den Ordner. Sitzungen derselben Person trennt
unter macOS allein die Regel `path-literal` im Profil des Agenten: Der Socket einer anderen Sitzung im selben Ordner ist
aus dem Profil mit EPERM gesperrt, der eigene antwortet (gemessen, macOS-Einschluss U1 und U2). Mit absolutem Pfad gilt dasselbe:
Der eigene Socket unter `<kurzer Ordner unter dem Heim>/a/such.sock` (39 Byte) antwortet, ein anderer absoluter Pfad
außerhalb der Regel ist mit EPERM gesperrt (gemessen, Messlauf 2 am 21.11., U3 und U4). Ein Prozess derselben uid außerhalb
eines Einschlusses kann den Vault ohnehin selbst lesen; vor ihm schützt dieser Nachtrag nicht.

## 4. Prüfung des Peers je Verbindung

**Vor dem ersten Byte.** Nach `accept` und bevor der Prozess ein Byte liest oder schreibt, ruft er `getpeereid` an der
Verbindung auf.

| Befund | `grund` der Zeile |
|---|---|
| `getpeereid` scheitert | `such.kennung_unmessbar` |
| die euid des Peers ist nicht die euid des Such-Prozesses | `such.kennung_fremd` |

**Abweisung.** Kein Byte an die Verbindung, dann das Ende der Verbindung. Gelesen wird vorher nichts. Der Prozess läuft
weiter und nimmt die nächste Verbindung an. Die Grenze von 16 gleichzeitigen Verbindungen aus Vertrag 4 gilt vor der
Prüfung; eine abgewiesene Verbindung zählt nicht weiter. Die Leerlauffrist von 10 s aus Klarstellung 13 von Vertrag 4 (Entscheid vom
24.11.2026) gilt unverändert auch hier.

**Was die Prüfung leistet.** Sie ist die zweite Sperre hinter Ordner 0700 und Socket 0600. Bei richtig gesetzten Rechten
erreicht keine fremde Kennung den Socket, und die Prüfung weist nie etwas ab. Sie greift, wenn die Rechte falsch gesetzt
sind. Agenten anderer Sitzungen derselben uid trennt sie nicht (Abschnitt 3).

**Protokollzeile, in zwei Stufen (Projektentscheid, 21.11.2026).** Die zwei Gründe kommen mit dem Code-Schritt
in Vertrag 5 (Abschnitt 6) und in die geschlossene Liste von `formate` (`kette.rs`). Ohne ihn lehnte
`kette pruefe` eine solche Zeile als `form_falsch` ab. **Es gilt Stufe 2:** Der Code dieses Nachtrags
baut auf und kommt nur mit ihm nach `development`. Stufe 1 bleibt als Herkunft stehen.

- **Stufe 1, nur bis zu diesem Merge (nicht gebaut):** Die Abweisung schreibt keine Zeile. Der Prozess meldet sie auf stderr mit genau
  `such.abgewiesen: <grund>` und LF, ohne uid und ohne gelesene Bytes.
- **Stufe 2, danach:** Statt der Meldung schreibt er je abgewiesener Verbindung genau eine Zeile `kanal.abgewiesen` in
  die Kette `brain-<JJJJ-MM>.jsonl` im Ordner `--protokoll`, in derselben Form wie unter Windows
  (`../windows/README.md`, Abschnitt 4).

```
{"aktionsname":"kanal.abgewiesen","ergebnis":"abgewiesen","fassung":2,"grund":"such.kennung_fremd","kanal":"such","route":"copilot","sitzung":"s-1","vorher":"<64 hex>","zeit":"2026-11-21T16:00:00.000Z"}
```

- Genau diese neun Felder, RFC 8785, geschlossen. `route` und `sitzung` kommen aus dem Start.
- Die Zeile nennt keine uid, keine gid, keine PID und keinen Kontonamen. Die uid eines fremden Kontos benennt eine
  Person (A12, A104).
- Scheitert das Schreiben, bleibt die Abweisung, und der Prozess meldet das auf stderr ohne Inhalt.

## 5. Ende und Schutz des Prozesses

- **Ende:** wie unter Linux. Auf SIGTERM oder SIGINT löscht der Prozess seinen Socket und endet mit Exit 0. Den Ordner
  räumt der Starter auf.
- **Abschirmung gegen Kernabbilder:** Unter macOS setzt der Prozess als Erstes `RLIMIT_CORE` weich 1 und hart 1 (Crate
  `kernabbild`, Regel „Abschirmung über `kernabbild`“ in Vertrag 4); `PR_SET_DUMPABLE` gibt es dort nicht. Ist die harte Grenze
  schon 0, gibt es kein `core_pattern`: Der Prozess läuft mit der Hinweiszeile `kernabbild: the hard limit for core is 0, this
  system has no core pattern` weiter. Scheitert `setrlimit`, endet der Start mit Exit 5.
- **Statt `PR_SET_DUMPABLE`:** Gemessen hat das Projekt am 21.11.2026 im Messlauf 2 am Mac, an einem Stellvertreter des
  Such-Prozesses: ein Prozess derselben uid außerhalb des Profils, mit einer Marke in der Umgebung. Rohdaten stehen in
  außerhalb des Repos, Zeilen P1 bis P11 und H1 bis H6.
  - Aus dem Profil scheitern Signale an den Such-Prozess mit EPERM: Signal 0, SIGCONT und `/bin/kill` (P1, P2, P8, H4).
    Dasselbe gilt für den Agenten einer anderen Sitzung (P9). An das eigene Kind darf der Agent Signale senden (P10).
  - `task_for_pid` auf den Such-Prozess scheitert mit und ohne Profil (P4, `kern=5`). Ein Debugger bekommt den
    Task-Port also nicht. Gemessen auch am 22.11.2026 ohne Profil: `kern=5` für ein eigenes ad-hoc
    signiertes Programm und für die eigene Shell. Den Task-Port sperrt macOS also selbst, nicht das Profil; eine Prüfung im
    Profil wäre blind (Projektentscheid, 22.11.2026). Die Sperre hängt daran, dass das Programm kein Recht `get-task-allow` trägt. Das prüft
    der statische Fall `mac-s-rangkern-ohne-get-task-allow` (Abschnitt 7).
  - **Grenze unter macOS** (Projektentscheid, 21.11.2026): Argumente und Umgebung (`KERN_PROCARGS2`), `proc_pidinfo` und
    `proc_pidpath` des Such-Prozesses sind aus dem Profil lesbar, auch aus dem gehärteten (P3, P5, P6, H1 bis H3). Darum
    gibt der Starter dem Such-Prozess unter macOS kein Geheimnis in Argumenten oder Umgebung mit. Der Such-Prozess liest
    ohnehin keine Umgebungsvariable (Vertrag 4), und seine Argumente sind Pfade, Route und Sitzung. Ein Geheimnis hält
    unter macOS nur der Halter unter eigenem Dienstkonto (A144).
  - `rangkern` wird für macOS mit Hardened Runtime und ohne `get-task-allow` signiert. Das regelt das Paket für macOS
    (M2m), nicht dieser Nachtrag. Gemessen gegen `rangkern` selbst (22.11., Abschnitt 7): Aus dem Profil scheitert SIGTERM an
    den Such-Prozess mit EPERM, ohne Profil erreicht ihn Signal 0, und die Suche antwortet danach weiter
    (`mac-profil-prozess`). Das Programm aus `cargo build` trägt kein Recht (`codesign -d --entitlements`: leer). Den Bau
    des Pakets prüft derselbe Fall, sobald es ihn gibt.
- Vault, Einstellungsdatei und Protokoll liegen außerhalb dessen, was das Profil dem Agenten zu lesen erlaubt.

## 6. `such-mcp` unter macOS

- **Aufruf:** `such-mcp --socket <pfad>`, mit einem absoluten Pfad von höchstens 103 Byte. Unter macOS ist die Option
  Pflicht und kein Test-Weg, denn `/run/brain/such.sock` gibt es nicht. Ohne sie, mit einem relativen oder zu langen Pfad
  oder mit jeder anderen Angabe: Exit 64 ohne Ausgabe.
- **Woher der Pfad kommt:** Der Starter schreibt ihn je Sitzung in die MCP-Einstellung, mit der er den Harness startet.
  Ändert der Agent ihn, verbietet das Profil die Verbindung (Abschnitt 3).
- **Server prüfen:** Nach `connect` und vor dem ersten Byte ruft es `getpeereid` an der Verbindung auf. Die euid des
  Servers muss die eigene euid sein. Die erwartete Kennung kommt nur aus `geteuid`, nie aus argv, Umgebung oder
  Einstellung. Scheitert der Aufruf oder weicht die euid ab, schreibt es nichts an den Socket; das Ergebnis ist „Suche
  nicht erreichbar“ mit `isError: true`.
- Alles andere gilt wie in Vertrag 4: Methoden, Werkzeug `brain_suche`, Prüfung der Argumente, eine Verbindung je Aufruf,
  eine Antwortzeile, Frist 10 s.

## 7. Fälle und Läufer

Die Fälle liegen in `faelle/` (Eingaben) und `erwartet/` (Soll), gebaut von `bau_faelle.py`; der Läufer ist
`pruefe_macos.py` und nutzt nur die Standardbibliothek von Python. Jeder Fall nennt in `orte`, wo er läuft:

- `mac`: am Mac unter der eigenen Kennung, ohne Einschluss.
- `mac-profil`: am Mac im Profil des macOS-Einschlusses. Der Läufer baut das Profil nach dem macOS-Einschluss-Vertrag, Abschnitt
  „Profil“, und fährt eine Sonde darin (siehe „Läufer“).
- `mac-zweites-konto`: am Mac mit einem zweiten lokalen Konto. Das braucht einen Admin und ist offen (OPEN 4).
- `bau`: als Test im Crate, mit einer Naht für die gemessene euid. Er belegt die Prüfung, solange kein zweites Konto da ist.

**Übernommen:** Alle Fälle von Vertrag 4 (`../faelle/`) laufen am Mac unverändert, mit dem Socket in einem eigenen Ordner
0700. Ihr Soll ist das von Linux, außer dass `--socket-rechte 0666` unter macOS Exit 64 ergibt (Fall `mac-a-rechte-0666`
ersetzt jeden Fall, der 0666 nutzt).
Die Fälle `r-*` (Rückfall der Einbettung) übernimmt `bau_faelle.py` nicht; sie laufen unter macOS über `--vertrag4` direkt aus
`../sichtbar`. Ebenso die acht Fälle vom 23.11.2026, die Erweiterungen von `../pruefe.py` brauchen (`nur_v4` in `bau_faelle.py`:
neue Schritte wie `grenze`, `zeilen`, `line_hex`, oder Felder wie `env_extra`, `vorher_rechte`, `ende_signal`, `nachher_fehlt`).

**Neu:**

| Fall | Was | Soll | `orte` |
|---|---|---|---|
| `mac-a-rechte-0666` | Start mit `--socket-rechte 0666` | Exit 64, nichts angelegt | `mac` |
| `mac-a-pfad-relativ` | `--socket such.sock` | Exit 64 | `mac` |
| `mac-a-pfad-104` | Socketpfad mit 104 Byte | Exit 64 | `mac` |
| `mac-a-pfad-103` | Socketpfad mit genau 103 Byte | Bereitschaftszeile, Suche antwortet | `mac` |
| `mac-a-ordner-0755` | Elternordner mit 0755 | Exit 74, Ordner unverändert | `mac` |
| `mac-a-ordner-verweis` | Elternordner ist ein Verweis auf einen Ordner 0700 | Exit 74 | `mac` |
| `mac-a-ordner-fremd` | Elternordner gehört einer anderen Kennung | Exit 74 | `mac-zweites-konto` |
| `mac-a-verweis-ins-leere` | Am Socketpfad liegt ein Verweis ins Leere | Exit 74, Verweis unverändert, am Ziel kein Socket | `mac` |
| `mac-p-gleiche-kennung` | `ping` und eine Suche unter derselben Kennung | Antworten wie unter Linux, keine Zeile `kanal.abgewiesen` | `mac` |
| `mac-p-fremde-kennung` | Ein zweites Konto verbindet (Rechte absichtlich offen) | kein Byte, Ende der Verbindung; Stufe 1: stderr `such.abgewiesen: such.kennung_fremd`, keine Zeile; Stufe 2: eine Zeile mit `such.kennung_fremd` ohne uid; der Prozess nimmt danach die nächste Verbindung an | `mac-zweites-konto`, `bau` |
| `mac-p-unmessbar` | `getpeereid` scheitert | kein Byte; Stufe 1: stderr `such.abgewiesen: such.kennung_unmessbar`; Stufe 2: Zeile mit `such.kennung_unmessbar` | `bau` |
| `mac-p-zeile-scheitert` | nur Stufe 2: Abweisung, Protokoll nicht schreibbar | kein Byte, Meldung auf stderr ohne Inhalt, Prozess läuft weiter | `bau` |
| `mac-m-ohne-socket` | `such-mcp` ohne `--socket` | Exit 64 ohne Ausgabe | `mac` |
| `mac-m-pfad-relativ` | `such-mcp --socket such.sock` | Exit 64 ohne Ausgabe | `mac` |
| `mac-m-server-fremd` | Der Server hat eine andere euid | „Suche nicht erreichbar“, `isError: true`, kein Byte an den Server | `mac-zweites-konto`, `bau` |
| `mac-e-sigterm` | SIGTERM | Exit 0, Socket weg, Ordner bleibt | `mac` |
| `mac-profil-eigener` | Agent im Profil fragt den eigenen Socket | Antwort (macOS-Einschluss U1) | `mac-profil` |
| `mac-profil-fremde-sitzung` | Agent im Profil verbindet den Socket einer anderen Sitzung | EPERM (macOS-Einschluss U2) | `mac-profil` |
| `mac-profil-prozess` | Agent im Profil sendet SIGTERM an den Such-Prozess; Positivkontrolle: ohne Profil erreicht ihn Signal 0 | EPERM, die Suche antwortet weiter. Bis 22.11. stand hier auch `lldb -p`; es entfällt (Projektentscheid), weil `task_for_pid` auch ohne Profil scheitert (Abschnitt 5) | `mac-profil` |
| `mac-s-rangkern-ohne-get-task-allow` | `codesign -d --entitlements - --xml` an `RANGKERN_BIN`; Gegenprobe: eine ad-hoc signierte Kopie mit `get-task-allow` | das Programm trägt kein verbotenes Recht; die Kopie macht dieselbe Prüfung rot (sonst ist sie blind) | `mac` |

**Mutanten beim Bau** (nur im Einschluss nach der Anordnung vom 21.11.2026, weil der Code Pfade schreibt und Rechte
setzt; unter macOS gibt es dafür noch kein gemessenes Werkzeug, also nur im Crate-Test mit eigenem Ordner): die Prüfung
der euid entfernt (`mac-p-fremde-kennung` rot über `bau`), die Prüfung des Elternordners entfernt (`mac-a-ordner-0755`
rot), 0666 zugelassen (`mac-a-rechte-0666` rot), die Serverprüfung in `such-mcp` entfernt (`mac-m-server-fremd` rot).

**Läufer.** `RANGKERN_BIN=<programm> MCP_BIN=<programm> python3 pruefe_macos.py [--vertrag4] [fall …]`. Je Fall legt er einen
Ordner 0700 an und darin den Ordner `s` (0700) mit dem Socket `s/such.sock`; der Pfad hat höchstens 103 Byte, außer in den Fällen
`mac-a-pfad-104` und `mac-m-pfad-104`. Er fährt die Fälle mit `orte` `mac`, dazu die mit `mac-profil`, wenn
`/usr/bin/sandbox-exec` und der Python der Command Line Tools da sind. Alle anderen meldet er als übersprungen mit Grund
(`mac-zweites-konto`, `bau`), nie als grün. Fehlt `RANGKERN_BIN` oder `MCP_BIN`, sind die zugehörigen Fälle übersprungen.
Ist kein Fall rot, aber einer übersprungen, endet er mit Exit 3, außer der Aufrufer erlaubt das mit `--aus-erlaubt` (dann
Exit 0; seit 23.11.2026, B12 des Reviews, bis dahin immer Exit 0). Ein Tor, das nur den Exit liest, sieht Übersprungenes so
nie als grün. Mit
`--vertrag4` fährt er zusätzlich die Läufer von Vertrag 4 (`../pruefe.py`, `../mcp/pruefe_mcp.py`) unverändert gegen
`../sichtbar` und `../mcp/faelle.json`, ohne `v-socket-rechte-0666`. Stand (23.11.2026): 82 Fälle für den Such-Prozess (66 aus Vertrag 4,
16 neue), 31 für `such-mcp` (26 aus Vertrag 4, 5 neue) und 1 an der Signatur (`signatur_faelle.json`); 5 davon laufen nicht am
Mac ohne Zusatz. Bei den Fällen mit abgewiesenem Peer nennt das Soll beide Stufen aus Abschnitt 4; es gilt Stufe 2 (Feld
`geltend`).

**Profil im Läufer.** Je Fall mit `mac-profil` legt der Läufer unter dem Ordner des Falls `agent/arbeit` und `agent/heim` an
(0700) und baut daraus das Profil, Zeile für Zeile nach dem macOS-Einschluss-Vertrag. Der Socket steht als `path-literal`. Gesperrt sind
Vault, Einstellungen, Protokoll und der Socket-Ordner, dazu `~/.ssh` und `~/Library/Keychains`. Das Merkmal liegt im Ordner
des Falls. Die Sonde ist ein Python-Skript im Arbeitsordner; sie läuft mit `/usr/bin/sandbox-exec -f <profil>` und dem Python
der Command Line Tools. Dessen Ordner steht als Programmordner im Profil: Python löst beim Start seinen eigenen Pfad auf und
braucht dafür die Metadaten der Vorfahren (gemessen am 22.11.: ohne sie `realpath: … Operation not permitted`). Jeder
Profil-Schritt hat eine Positivkontrolle ohne Profil. Vor dem ersten Schritt liest die Sonde ohne Profil eine Notiz des Vault
und im Profil nicht (EPERM); sonst wirkt das Profil nicht, und der Fall ist rot. Bei `fremde-sitzung` verbindet sie ohne
Profil zum Socket der anderen Sitzung (ein Hörer des Läufers im selben Ordner). Bei `prozess` erreicht Signal 0 ohne Profil den
Such-Prozess.

**Gegenproben der Profil-Fälle** (22.11., an einer Kopie des Läufers gegen dasselbe Programm): Ohne die Zeile `path-literal`
ist `mac-profil-eigener` rot (EPERM statt Antwort). Mit dem ganzen Socket-Ordner als `subpath` statt des einen Sockets ist
`mac-profil-fremde-sitzung` rot (verbunden statt EPERM). Eine Gegenprobe, die Signale freigibt, lief nicht: Mutanten für
Signal-Code laufen nur im Einschluss, und unter macOS gibt es dafür keinen. Gelänge das Signal, endete der Such-Prozess, und
die folgende Anfrage desselben Falls wäre rot.

**Gemessen gegen das Programm (21.11.).** Am Mac lief `pruefe_macos.py --vertrag4` gegen `rangkern` und
`such-mcp` aus dem Stand vom 21.11.2026: 131 grün, 0 rot, 9 übersprungen von 140. Übersprungen sind die 8 Fälle ohne Zusatz und
`v-socket-rechte-0666`, jeder mit Grund. Am selben Stand lief `cargo test` am Mac für `rangkern` mit 382 bestanden und für
`such-mcp` mit 87 bestanden, jeweils 0 fehlgeschlagen.

**Gemessen mit Profil (22.11.).** Am Mac lief `pruefe_macos.py --vertrag4` gegen `rangkern` und `such-mcp`
aus `development` vom 22.11.2026 (Läufer vom 22.11.2026): 135 grün, 0 rot, 6 übersprungen von 141. Grün sind auch die drei Fälle
`mac-profil-*` und `mac-s-rangkern-ohne-get-task-allow`. Übersprungen sind die 5 Fälle ohne Zusatz (`mac-zweites-konto`,
`bau`) und `v-socket-rechte-0666`. Ausgabe:
außerhalb des Repos.

## 8. Offen

1. **Peer-Prüfung unter Linux und `peerkanal` unter Unix (nach M1).** Linux prüft den Peer nicht (Vertrag 4,
   „Nicht in diesem Vertrag“). Wechseln alle drei Systeme auf `peerkanal` mit Hallo, ändern sich Vertrag 4, dieser
   Nachtrag und `such-mcp` zugleich (Windows-Nachtrag, OPEN 3 und 5).
2. **Gründe in Vertrag 5: erledigt (Projektentscheid, 21.11.2026).** `such.kennung_fremd` und
   `such.kennung_unmessbar`; der Code baut darauf auf, es gilt Stufe 2.
3. **Schutz des Prozesses: am Stellvertreter gemessen (Messlauf 2 am 21.11.).** Signale sind gesperrt, und
   `task_for_pid` scheitert. Argumente und Umgebung bleiben lesbar; das ist eine Grenze (Abschnitt 5). Der absolute
   Socketpfad im Profil trägt (U3, U4). **Erledigt am 22.11.:** `mac-profil-prozess` lief gegen `rangkern` selbst grün, im
   Profil nach dem macOS-Einschluss-Vertrag auf `development` (Abschnitt 7). Offen bleibt der Bau des Pakets für macOS (M2m): Er muss
   `mac-s-rangkern-ohne-get-task-allow` bestehen.
4. **Zweites Konto am Mac (Betreiber).** Die Fälle mit `mac-zweites-konto` brauchen ein lokales Testkonto. Bis dahin
   belegen die Fälle mit `bau` die Prüfung.

<!-- vom Bauvorgang angefuegt -->
> Hinweis: Dieser Text nennt Werkzeuge, die in diesem Klon nicht liegen: `brain`.
> Sie gehoeren zum Setup der Maschine, aus der dieser Baum stammt; hier fehlen
> sie mit Absicht. Wer sie braucht, schreibt sich seine eigenen.
> *Named here, not shipped here.*
