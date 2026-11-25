# Vertrag: Einschluss eines Agentenlaufs unter macOS (Fassung 1)

> **Stand 22.11.2026.** Messlauf 2 (gültig ist Lauf 2c, 21.11.) und Messlauf 3 (git, 22.11.) sind eingetragen. Die Frist des
> Wächters ist 5 s. Das Profil schützt Umgebung und Prozessauskunft derselben Kennung **nicht** (Abschnitt „Zweck und
> Grenze“); ein Geheimnis darf darum nur der Halter tragen. Was keine Messung gedeckt hat, steht unter OPEN.

Stand 20.11.2026, Anforderungen A142 bis A145 (Vorschlag,
Kennungen unter macOS, Abschnitt 3). Grundlage sind drei eigene Messläufe am Mac (macOS 26.5.1, Build 25F80,
arm64; Rohdaten außerhalb des Repos) mit Wegwerf-Dateien und Hörern nur auf `127.0.0.1` und `[::1]` (Abschnitt „Messproben“), dazu die Messungen der
alten Mac-Tür (w76, w92, `the previous implementation`). „Gemessen“ nennt eine Probe dieser Läufe. „Hergeleitet“ ist nicht
gemessen. Was nicht gemessen ist, steht unter OPEN und ist keine Zusage.

Der Vertrag steht vor dem Code. Alle Fälle sind heute rot, weil es keine Umsetzung gibt. Verdeckte Fälle und Mutanten
folgen mit dem Bau. Die Fälle der Form `mac` laufen nur am Mac mit echtem Seatbelt; sie sind Außentests.

## Dateien

| Datei | Zweck |
|---|---|
| `README.md` | dieser Vertragstext |
| `einschluss.schema.json` | Form als JSON Schema 2020-12: `$defs/konfig` (Einschlussdatei), `$defs/bereit` (Zeile eines Weiterreichers), `$defs/beschreibung` (Einschlussbeschreibung, nur für die Fälle) |
| `faelle/<name>.json` | `beschreibung`, `form` (`konfig`, `bereit`, `profil`, `profiltext`, `aufruf`, `mac`); bei `konfig` und `bereit` die Bytes als `text` oder `b64`; bei `profil` die Beschreibung als `eingabe`; bei `profiltext` die Beschreibung als `eingabe` und ein Zusatz `einfuegen`; bei `aufruf` die Ausgangslage als `lage`; bei `mac` Eigenschaft, Anforderung, Ausgangslage, Handlung und Beleg |
| `erwartet/<name>.json` | `konfig`: `{"gueltig": true}` oder `{"gueltig": false, "code": …}`; `bereit`: `{"gueltig": …}`; `profil`: `enthaelt` und `enthaelt_nicht` oder `fehler`; `profiltext`: `gueltig` (der Zusatz im gebauten Profil wird von der zweiten Prüfung angenommen oder abgelehnt); `aufruf`: Exitcode und stderr-Zeile oder `exec`; `mac`: `erwartet` und `messwerte` |
| `bau-faelle.py` | erzeugt Schema, `faelle/` und `erwartet/` |
| `pruefen.py` | prüft Paarung, Form, JSON-Lesen, Einschlussdatei und Bereitschaftszeile gegen eine Referenz, jeden Fall `profil` gegen den Referenzbau des Profils, jeden Fall `profiltext` gegen die zweite Prüfung am Text, die stderr-Zeilen gegen Vertrag 1 und je Eigenschaft einen Fall und eine Gegenprobe |

## Zweck und Grenze

Unter macOS läuft der Agent einer Sitzung unter einem festen **Seatbelt-Profil**, das der Starter je Lauf aus der
Einschlussbeschreibung baut und mit `/usr/bin/sandbox-exec -f` anwendet. Das Profil beginnt mit `(deny default)`. Es
gibt dem Agenten den Arbeitsordner, sein Sitzungsheim, die Programmordner und das System zum Lesen, die
**Sitzungsports** seiner eigenen Weiterreicher und den Such-Socket seiner Sitzung. Alles andere ist gesperrt. Einen
eigenen Namensraum gibt es unter macOS nicht. Darum hält der Dienst ein **Wächteramt**: Er beendet beim Ende der Sitzung
jeden Prozess mit dem Merkmal des Laufs (Abschnitt „Prozessende“).

Die Schnittstelle nach außen ist dieselbe wie unter Linux und Windows: Der Kern-Dienst startet das Programm „Lauf
starten“ nach Vertrag 1 (`rust/spec/start/README.md`), und die Suche läuft je Sitzung nach Vertrag 4
(`rust/spec/such-socket/README.md`). Dieser Vertrag legt fest, was unter macOS in den Schritten 6 bis 8 von Vertrag 1
geschieht. Entschieden ist `sandbox-exec` für Schritt II (Projektentscheid) und das Wächteramt statt einer VM je Lauf (Entscheid des Betreibers).

**Grenze unter macOS.** Unter Linux endet ein Lauf mit seinem PID-Namensraum und unter Windows mit seinem Job-Objekt;
unter macOS endet er erst, wenn der Wächter des Dienstes ihn beendet, und nur für Prozesse, die das Merkmal des Laufs
tragen. Bis dahin bleiben auch abgelöste Nachfahren eingeschlossen (Dateien und Netz, gemessen), aber sie leben weiter,
und Seatbelt verbirgt die übrigen Prozesse der Person nicht, sondern sperrt nur Signale an sie.

**Grenze: Umgebung und Prozessauskunft derselben Kennung bleiben lesbar (Befund 1, gemessen).** Das Profil kann die Umgebung
eines Prozesses derselben Kennung nicht sperren. Im Lauf bleiben `KERN_PROCARGS2` (Umgebung; Probe P3, H1, H6), `proc_pidinfo`
(P5, H2) und `proc_pidpath` (P6, H3) lesbar, auch mit den Sperrzeilen für `sysctl-read` und `process-info*` (Rohdatei
`messlauf2-ergebnis.json`, Messlauf 2c). Die Sperrzeilen bleiben im Profil, weil sie nicht schaden, aber sie gelten **nicht** als
Schutz. Die Folge ist eine Pflicht vor Schritt II unter macOS: **Ein Geheimnis darf nur der Halter tragen, ein Prozess unter
einer eigenen Kennung (`_wbhalter`), nie ein Prozess der Person**, auch nicht in Umgebung oder argv. Wer auf derselben Kennung
läuft wie der Agent (der Starter, der Such-Prozess, die Weiterreicher), ist für den Agenten lesbar. Der Schein ist deshalb
kein Geheimnis gegen den Agenten, sondern eine Zuordnung je Lauf.
Der Text für das Restrisiko ist offen.

## Begriffe

| Begriff | Bedeutung |
|---|---|
| Person | das angemeldete macOS-Konto, das die Werkbank benutzt; ohne Adminrechte |
| Starter | das Programm „Lauf starten“ (Vertrag 1), als Prozess der Person |
| Profil | der SBPL-Text, den der Starter je Lauf baut; fest bis auf die Pfade und Ports dieses Laufs |
| Sitzungsheim | ein leerer Ordner je Sitzung außerhalb des Arbeitsordners; im Lauf `HOME`. Gegenstück zum Zustand unter Linux |
| Weiterreicher | Prozess je Sitzung außerhalb des Profils, vom Starter gestartet; hört auf genau einem Port auf `127.0.0.1` und `[::1]` und reicht an Modellweg oder Ausgang derselben Sitzung weiter |
| Modellweg | Weg zum Modell der Route (lokal oder Gateway); der Schlüssel bleibt außerhalb (A14, E4) |
| Ausgang | Weg zu den Cloud-Zielen der Sitzung mit Zielliste; prüft den Hostnamen, weil Seatbelt das nicht kann |
| Sitzungsports | die Ports der Weiterreicher einer Sitzung, höchstens 4; vom System vergeben (Port 0), nie fest, nie der Port der Browser-Brücke |
| Such-Socket | der Unix-Socket des Such-Prozesses der Sitzung im 0700-Ordner außerhalb (Vertrag 4) |
| Merkmal | die Datei `kennung` im 0700-Ordner des Dienstes, die nur das Profil dieses Laufs beschreiben darf; siehe „Prozessende“ |
| Schein | Zufallswert je Lauf in der Umgebung des Agenten; die Weiterreicher prüfen ihn je Verbindung (A143) |
| Browser-Brücke | der HTTP-Hörer des Kern-Dienstes für die Seite; nie ein Sitzungsport |

Kennungen haben die Form `^[a-z0-9][a-z0-9._-]{0,63}$` wie in den Verträgen 1 bis 5.

## Gemeinsam mit Linux und Windows

**Schnittstelle.** Aufruf, Schritte 1 bis 5, Exitcodes und die eine stderr-Zeile gelten wie in Vertrag 1. Jeder Fehler
des Einschlusses ergibt Exit 69 und `lauf.einschluss_fehlt: Einschluss nicht verfuegbar`, ein fehlender Arbeitsordner
Exit 65 und `lauf.ordner_fehlt` (wie `einschluss::Fehler::code()` im Linux-Bau). Den genauen Grund schreibt der Starter
in das Protokoll des Dienstes, nie in die PTY.

**Einschlussbeschreibung.** Der Crate `einschluss` hat ein Modul je System hinter `cfg(target_os)`. Die gemeinsame
Schnittstelle ist Beschreibung, Bau (`Profil::bauen`, rein), Vorprüfung (`Profil::vorpruefen`) und Startbefehl
(`Profil::befehl`), Stand vom 20.11.2026. Unter macOS gelten die Felder so:

| Feld (Linux-Bau) | unter macOS |
|---|---|
| `harness` | gilt; Kennung aus der Harness-Tabelle von Vertrag 1 |
| `arbeitsordner` | gilt; lesen und schreiben (`subpath`) |
| `programmordner` | gilt; lesen und ausführen (`subpath`, `file-map-executable`) |
| `zustand` (`host`, `name`) | gilt anders: `host` ist das Sitzungsheim und wird `HOME`; `name` entfällt, weil nichts eingehängt wird |
| `heim` | gilt anders: das echte Heim der Person; nur für Verbote, Anmeldespeicher und Metadaten der Vorfahren, nie freigegeben |
| `such_socket` | gilt anders: der Hostpfad steht als `path-literal` im Profil; den Gastpfad `/run/brain/such.sock` gibt es nicht |
| `modell_socket` | entfällt; an seine Stelle treten `sitzungsports` (`modellweg`, `ausgang`) |
| `umgebung` | gilt mit Zusätzen (Abschnitt „Umgebung“); `home` ist gleich dem Sitzungsheim |
| `geschuetzt` | gilt; dazu als zweite Linie eine Sperrzeile am Ende des Profils |
| `argv` | gilt |
| `system` | entfällt; die Systempfade sind fest (Abschnitt „Profil“) |
| neu: `merkmal` | Pfad der Datei `kennung` des Laufs im 0700-Ordner des Dienstes |

Die Verbotslisten des Linux-Baus (`verbote.rs`: Geheimnisteile, Anmeldeordner, Anmeldedateien, Zugänge der Harnesses)
gelten. macOS ergänzt sie um die Einträge unter `~/Library` (Liste in `pruefen.py`, `ANMELDESPEICHER`). Anders als unter
Linux bleibt unter macOS auch die Anmeldung des eigenen Harness zu: Der Harness meldet sich nur über den Schein am
Modellweg an.

**Fehler des Crates.** Die Varianten des Linux-Baus gelten mit denselben Namen: `NichtVerfuegbar`,
`BeschreibungUngueltig`, `Verboten`, `Ueberlappt`, `ArbeitsordnerFehlt`, `QuelleUnbrauchbar` und `Probelauf`. `BwrapFehlt`
und `BwrapNichtAusfuehrbar` heißen unter macOS `SeatbeltFehlt` und `SeatbeltFremd`. `TiocstiOffen` und
`NamensraumGesperrt` gibt es unter macOS nicht (OPEN 6).

**Gegenproben (gemeinsame Liste vom 20.11.2026).**

| Gegenprobe | Eigenschaft hier | Stand |
|---|---|---|
| Notizdatei lesen scheitert | M1 | gemessen |
| fremde Adresse scheitert | M2 | gemessen, mit Grenze N8 |
| Port der Browser-Brücke unerreichbar | M3 | gemessen |
| eigener Modellweg-Port erreichbar | M4 | gemessen |
| Modellweg-Port einer anderen Sitzung nicht erreichbar | M5 | gemessen |
| Umgebung ändert die Route nicht (E1) | M6 | gemessen |
| Token nicht lesbar | M7 | gemessen an Wegwerf-Dateien |
| Such-Prozess nicht signalisierbar (E3) | M8 | Signal gemessen; Umgebung bleibt lesbar (Grenze, „Zweck und Grenze“) |

## Plattform macOS

### Einschlussdatei (`konfig`)

Die Einschlussdatei gehört zur Installation. Der Starter liest sie aus seiner eigenen Konfiguration, nie aus der
Startdatei und nie vom Agenten. Ein JSON-Objekt, UTF-8 ohne BOM, höchstens 4096 Byte, geschlossene Form, kein doppelter
Schlüssel. Dieselbe Form wie unter Windows, ohne `shell`.

```json
{"fassung": 1,
 "projektwurzeln": ["/Users/beispiel/Projekte"],
 "programmordner": ["/Users/beispiel/.local/share/claude", "/opt/homebrew"]}
```

| Feld | Typ | Grenzen | Bedeutung |
|---|---|---|---|
| `fassung` | Ganzzahl | genau `1` | |
| `projektwurzeln` | Liste von POSIX-Pfaden | 1 bis 16, ohne Doppel | Arbeitsordner liegen echt unter einer davon |
| `programmordner` | Liste von POSIX-Pfaden | 1 bis 16, ohne Doppel | Harness, Node, `such-mcp`; nur lesen und ausführen |

Kein Port, keine SID, keine Regel steht in dieser Datei. Frei wählbare Seatbelt-Regeln gibt es nicht
(`schluessel_unbekannt`).

**POSIX-Pfad:** wie `arbeitsordner` in Vertrag 1 (absolut, normalisiert, ohne Steuerzeichen, höchstens 1024 Byte), dazu
ohne `"` und `\`. Grund: Beide Zeichen haben in einer SBPL-Zeichenkette eine Bedeutung, und das Profil maskiert nicht.
Vergleiche zwischen Pfaden gelten ohne Rücksicht auf Groß- und Kleinschreibung, weil APFS sie in der Vorgabe nicht
unterscheidet (gemessen D12, D13: Seatbelt prüft den Pfad auf der Platte, nicht die Schreibweise des Aufrufs).

| Stufe | Code | Verletzung |
|---|---|---|
| 1 | `einschluss.zu_gross` | mehr als 4096 Byte |
| 2 | `einschluss.json_ungueltig` | kein UTF-8, BOM, Syntaxfehler, doppelter Schlüssel, kein Objekt, Text danach |
| 3 | `einschluss.schluessel_unbekannt` | ein Schlüssel außerhalb der Form, auch `shell`, `modellweg_port`, `regeln` |
| 4 | `einschluss.schluessel_fehlt` | ein Pflichtschlüssel fehlt |
| 5 | `einschluss.typ_falsch` | falscher JSON-Typ |
| 6 | `einschluss.fassung_unbekannt` | `fassung` ist eine Ganzzahl, aber nicht `1` |
| 7 | `einschluss.liste_grenze` | leere Liste, mehr als 16 Einträge, ein Eintrag doppelt |
| 8 | `einschluss.pfad_form` | ein Pfad verletzt die Regeln für POSIX-Pfade |
| 9 | entfällt (unter Windows `shell_unbekannt`) | |
| 10 | `einschluss.pfad_ueberlappt` | ein Programmordner ist gleich einer Projektwurzel, liegt darüber oder darunter |
| 11 | `einschluss.pfad_geschuetzt` | ein Pfad ist ein Systempfad, das Heim oder liegt darüber, oder er berührt einen Anmeldespeicher |
| 12 | `einschluss.pfad_fehlt` | ein Pfad ist kein vorhandener Ordner |
| 13 | `einschluss.pfad_umgelenkt` | der echte Pfad (`realpath`, `F_GETPATH`) weicht ab: Verweis, `/tmp` statt `/private/tmp`, andere Schreibweise |

Die Nummern sind die des Windows-Vertrags; Stufe 9 bleibt frei, damit gleiche Codes gleiche Stufen haben. Die Stufen 1
bis 8 brauchen nur die Datei; 10 bis 13 brauchen Dateisystem und Person und sind erst mit dem Bau prüfbar.

### Profil

Der Starter baut das Profil als reine Funktion aus der Beschreibung. Zeile für Zeile, in dieser Reihenfolge:

```text
(version 1)
(deny default)
(import "system.sb")
(allow process-fork)
(allow process-exec)
(allow signal (target same-sandbox))
(allow process-info* (target same-sandbox))
(allow sysctl-read)
(allow pseudo-tty)
(allow file-ioctl (literal "/dev/tty") (regex #"^/dev/ttys[0-9]+$") (literal "/dev/ptmx"))
(allow file-read* file-write-data (literal "/dev/tty") (regex #"^/dev/ttys[0-9]+$") (literal "/dev/ptmx") (literal "/dev/null") (literal "/dev/zero"))
(allow file-read* file-map-executable <subpath je Systempfad> <subpath je Programmordner>)
(allow file-read-metadata <literal je Vorfahr von Arbeitsordner, Sitzungsheim und Programmordnern>)
(allow file-read* file-write* (subpath "<arbeitsordner>") (subpath "<sitzungsheim>"))
(allow network-outbound (remote ip "localhost:<modellweg>") [(remote ip "localhost:<ausgang>")])
(allow network-outbound (remote unix-socket (path-literal "<such_socket>")))
(deny file-read* file-write* <subpath je geschützter Ordner> <subpath je Anmeldespeicher unter dem Heim>)
(deny sysctl-read (sysctl-name "kern.procargs2") (sysctl-name "kern.procargs") (sysctl-name-prefix "kern.proc."))
(deny process-info* (target others))
(allow file-write-data (literal "<merkmal>"))
```

Systempfade (nur lesen und ausführen): `/usr`, `/bin`, `/sbin`, `/System`, `/Library/Apple`,
`/Library/Developer/CommandLineTools`, `/private/etc`, `/private/var/db/timezone`, `/private/var/db/dyld`,
`/private/var/select`. Nicht dabei sind `/Applications`, `Xcode.app`, `/opt/homebrew` (nur als Programmordner) und
`/private/var/folders`.

**Regel für git (H9, gemessen, Pflicht vor Schritt II).** Messlauf 2c: `git` scheiterte unter dem Profil mit
`xcode-select: error: No developer tools were found …` (H9; ohne Profil gelang es). Die Ursache ist, dass das Profil jenes Laufs
den Ordner `/Library/Developer/CommandLineTools` nicht freigab (gemessen am Profiltext in der Rohdatei); er steht in der
Systempfad-Liste dieses Vertrags und im Referenzbau. Messlauf 3 (22.11., `messlauf3-git-ergebnis-v.json`,
`messlauf3-git-ergebnis-w.json`, ein Wegwerf-Repo im Heim des Testnutzers, `HOME` im Sitzungsheim, ohne `DEVELOPER_DIR`) misst,
was `git --version`, `git status --short` und `git commit --allow-empty` brauchen. Auf diesem Mac zeigt `xcode-select -p` auf
`/Applications/Xcode.app/Contents/Developer`, und `/usr/bin/git` ist die Hülle von `xcrun`. Unter dem Profil kann `xcode-select`
seinen Verweis nicht lesen und fällt auf `/Library/Developer/CommandLineTools` zurück (hergeleitet aus dem Befund, nicht an
`xcode-select` selbst gemessen):

| Freigabe unter dem Profil | `git` |
|---|---|
| keine (V0, W0) | scheitert, Exit 1, `xcode-select: error: No developer tools were found` |
| `/Library/Developer/CommandLineTools` ganz, Lesen und `file-map-executable` (V1, W3) | **gelingt**, Exit 0 für Version, Status und Commit |
| nur `…/CommandLineTools` und nur Lesen (W4) | gelingt ebenso (`file-map-executable` ist für `git` nicht nötig) |
| nur `…/CommandLineTools/usr` (W1) | scheitert, Exit 74: `developer directory '/Library/Developer/CommandLineTools' isn't accessible` |
| nur `usr/bin/git` und `usr/libexec/git-core` (W2) | scheitert, Exit 1 wie ohne Regel |
| `/Applications/Xcode.app/Contents/Developer` ganz (V2), nur `…/usr` (V3, V4) oder nur `git` (V5, V6) | scheitert (Exit 71, 74 oder 1; V2: `couldn't stat Xcode's Info.plist`) |

Die Regel ist also: **der Ordner `/Library/Developer/CommandLineTools` gehört zu den Systempfaden, nur Lesen und
`file-map-executable`, kein Schreiben** (Fall `profil-git-regel`, M17). Kein anderer Pfad ist nötig. Der Schreibversuch scheiterte
in Messlauf 3 mit EPERM, aber an einer Datei unter `Xcode.app`, nicht unter dem Ordner der Command Line Tools; für diesen Ordner
ist „kein Schreiben“ hergeleitet (die Regel nennt nur Lesen), nicht gemessen. `git` meldet dabei auf stderr, dass es die Datei
`xcrun_db` unter `/var/folders` nicht anlegen kann (Exit bleibt 0); `/private/var/folders` bleibt gesperrt. Ein Mac nur mit Xcode
und ohne Command Line Tools ist nicht gemessen und nach V2 bis V6 nicht lauffähig (OPEN 9).

| Zeile | Grund | Beleg | Anforderung |
|---|---|---|---|
| `(deny default)` | Grundlage; was nicht erlaubt ist, ist gesperrt | gemessen: D3 bis D8, D14, N5 bis N10 | A142 |
| `(import "system.sb")` | Apples Grundregeln für dyld, `/dev/null`, Systemdienste; ohne sie startet kaum ein Programm | gemessen w76, w92; Inhalt gelesen (`/System/Library/Sandbox/Profiles/system.sb`, 311 Zeilen) | A142 |
| `process-fork`, `process-exec` | der Harness startet Werkzeuge; ein Kind erbt das Profil | gemessen D15, D16 | A142 |
| `signal`, `process-info*` nur `same-sandbox` | Signale an eigene Kinder ja, an fremde Prozesse nein | gemessen P1, P2, P8, P9 gesperrt; P10 eigenes Kind gelingt | A142 |
| `sysctl-read` mit der Sperre `kern.procargs*`, `kern.proc.*` | sollte die Umgebung sperren; wirkt nicht, bleibt als zweite Linie | gemessen P3 (ohne Sperre lesbar), H1 (mit Sperre **weiter lesbar**; Grenze, kein Schutz) | A142, A143 |
| `(deny process-info* (target others))` | sollte `proc_pidinfo` und `proc_pidpath` fremder Prozesse sperren; wirkt nicht, bleibt als zweite Linie | gemessen P5, P6 (ohne lesbar), H2, H3 (mit Sperre weiter lesbar; Grenze, kein Schutz) | A142 |
| Terminal-Zeilen | der Harness behält die PTY der Sitzung | übernommen aus w92 | A142 |
| Lesen der Programmordner | Harness, Node, `such-mcp` | gemessen D1 mit der Sonde aus einem Programmordner | A142 |
| Metadaten der Vorfahren | `realpath` und `git` brauchen sie (w92: ohne sie meldet `git` „Invalid path“); `git` braucht zusätzlich den Ordner der Command Line Tools (Abschnitt „Regel für git“) | gemessen H9 (Messlauf 2c, Profil ohne den Ordner: `git` scheitert) und Messlauf 3 | A142 |
| Arbeitsordner und Sitzungsheim lesen und schreiben | Arbeit des Agenten; `HOME` | gemessen D1, D2 | A142 |
| Netz nur `localhost:<Sitzungsport>` | siehe „Netz“ | gemessen N1 bis N15 | A145 |
| Such-Socket als `path-literal` | siehe „Such-Socket“ | gemessen U1, U2 | A142 |
| Sperrzeile `geschuetzt` und Anmeldespeicher | zweite Linie: eine spätere Regel schlägt eine frühere | gemessen: Reihenfolge (spätes `deny` sperrt, spätes `allow` öffnet) | A142, A144 |
| `file-write-data` auf das Merkmal, als letzte Zeile | macht den Lauf erkennbar; steht nach der Sperrzeile, weil das Merkmal im geschützten Dienstordner liegt | gemessen: Messlauf 2c, Prozessende (Abschnitt „Prozessende“) | A142 |

**Was das Profil nie enthält.** `(allow default)`, ein offenes `network*`, `network-bind`, `network-inbound`, ein
`file-read*` oder `file-write*` ohne Filter, `mach-lookup` außerhalb von `system.sb`, `(remote ip "*:…")`,
`(remote tcp …)`, `(remote udp …)`, einen Hostnamen oder eine IP-Adresse. `pruefen.py` prüft diese Liste an jedem Fall
`profil` (`NIE` in `bau-faelle.py`).

**Prüfung am Text.** Nach dem Bau prüft der Starter den fertigen Text noch einmal, wie der Linux-Bau seine Argumentliste:
die ersten drei Zeilen wie oben, genau ein `(deny default)`, keine Zeile aus der Liste „nie“, die drittletzte Zeile
ist die `sysctl`-Sperre, die letzte das Merkmal, und kein `subpath` liegt auf dem Heim oder darüber. Sonst
`BeschreibungUngueltig`. Die Vergleiche laufen auf dem **normalisierten** Text: jeder Leerraum (Leerzeichen, Tab,
Zeilenumbruch, mehrere) wird zu einem Leerzeichen, und nach `(` und vor `)` steht keins. Eine Regel `allow` **ohne Filter** wird
abgelehnt, außer `process-fork`, `process-exec`, `sysctl-read` und `pseudo-tty`; das gilt auch, wenn der Bau sie heute nie erzeugt.
Vor dem 22.11. fing die Prüfung `(allow network-outbound)` nur ohne Leerzeichen; die Formen `(allow network-outbound )`, mit Tab,
mehreren Leerzeichen oder Zeilenumbruch gingen durch (Fail-open). Die Fälle `profiltext-*` halten jede Form fest. Der Bau
lehnt Sitzungsports ohne `modellweg` ab (`BeschreibungUngueltig`, Fall `profil-ohne-modellweg`), damit nie eine Netzregel ohne
Filter entsteht.

**Ablehnungen beim Bau** (`Profil::bauen`, Reihenfolge wie im Linux-Bau):

1. `BeschreibungUngueltig`: Harness verletzt die Kennungsform; `argv` leer; ein Pfad ist kein POSIX-Pfad (auch `"` und
   `\`, obwohl Vertrag 1 sie im Arbeitsordner zulässt); ein Sitzungsport außerhalb von 1 bis 65535 oder zwei gleiche;
   der Such-Socket länger als 103 Byte (`sun_path` fasst unter macOS 104 Byte mit Nullbyte); `HOME` ist nicht das
   Sitzungsheim; `TMPDIR` liegt nicht echt im Sitzungsheim.
2. `Verboten`: der Arbeitsordner ist ein Systempfad, das Heim oder liegt darüber oder unter `/System`, `/usr`, `/bin`,
   `/sbin`, `/Library`, `/Applications`, `/private/etc`, `/private/var/db`, `/opt/homebrew`; ein freigegebener Pfad hat
   einen Geheimnisteil oder berührt einen Anmeldespeicher (gleich, darüber, darunter).
3. `Ueberlappt`: Arbeitsordner, Programmordner und Sitzungsheim überlappen einander oder einen geschützten Ordner; der
   Such-Socket oder das Merkmal liegt in einem freigegebenen Ordner.

### Netz (A145)

Seatbelt filtert ausgehende Verbindungen nur nach `localhost` oder `*` und einem Port. Ein Hostname und eine IP-Adresse
im Filter lassen das Profil nicht laden (gemessen: `"example.com:443"` und `"127.0.0.1:443"` ergeben Exit 65 von
`sandbox-exec`; `"localhost:443"`, `"*:443"` und `(remote tcp "localhost:443")` laden). Darum gilt:

1. Erlaubt sind nur die Sitzungsports dieses Laufs als `localhost:<port>`: der Port des Weiterreichers zum Modellweg und,
   bei Cloud-Zielen, der zum Ausgang. Gemessen erreicht der Agent seinen Modellweg über `127.0.0.1`, `[::1]` und den
   Namen `localhost` (N1, N2, N11, N14). Die IPv4-abgebildete Form `[::ffff:127.0.0.1]` ist mit EPERM gesperrt, auch auf dem
   eigenen Modellweg-Port (N21; ohne Profil verbunden): der Agent nimmt `127.0.0.1` oder `localhost`.
2. Gesperrt mit EPERM sind: die Browser-Brücke (N5, N12), der Modellweg-Port einer anderen Sitzung (N6), derselbe Port
   aus dem Profil einer anderen Sitzung (N15), die eigene LAN-Adresse auf einem anderen Port (N7), UDP an eine fremde
   Adresse (N9) und jedes Hören (N10). Messlauf 2c ergänzt, jeweils auf dem Modellweg-Port: `127.0.0.2` EPERM (N16), `224.0.0.1` EPERM (N17),
   `255.255.255.255` EPERM (N18), `[::ffff:127.0.0.1]` auf Modellweg- und Brücken-Port EPERM (N21, N22), UDP an `127.0.0.1` auf den
   Brücken-Port EPERM (N23). **Erlaubt bleibt:** UDP an `127.0.0.1` auf den Modellweg-Port (N19: gesendet; `localhost:<port>`
   gilt auch für UDP, aber nur auf diesem Port) und `0.0.0.0` auf den Modellweg-Port (N20: verbunden, die Verbindung geht
   zum eigenen Rechner). Rohdatei `messlauf2-ergebnis.json`.
3. **Grenze, gemessen:** `localhost:<port>` gilt auch für die eigene LAN-Adresse auf demselben Port (N8: keine Sperre,
   die Verbindung lief in die Zeitgrenze, weil dort niemand hört). Ein Hörer eines anderen Prozesses auf
   `<LAN-Adresse>:<Sitzungsport>` wäre für den Agenten erreichbar. Die Weiterreicher binden darum nur Loopback, und der
   Starter prüft vor dem exec, dass auf keinem Sitzungsport ein fremder Hörer steht (OPEN 2). Messlauf 2c zeigt, dass
   `localhost` sonst keine weitere Loopback-Adresse deckt (N16: `127.0.0.2` gesperrt).
4. Hostnamen der Cloud-Ziele prüft nur der Ausgang (Zielliste), nicht Seatbelt. Namensauflösung im Lauf: Die Mach-Probe (`sandbox_check` für `mach-lookup`
   am Profil des Agenten, Messlauf 2c, `mach_lookup_agent_b`) meldet `com.apple.dnssd.service` als **verboten**; der Namensdienst
   ist also nicht erreichbar (ohne Profil, am Such-Prozess: erlaubt). Gemessen ist die Auskunft von `sandbox_check`, keine echte
   Namensauflösung im Lauf (OPEN 6). Ebenfalls verboten: `com.apple.SecurityServer`, `com.apple.securityd`,
   `com.apple.coreservices.launchservicesd`, `com.apple.coreservices.appleevents`, `com.apple.pasteboard.1` (Zwischenablage),
   `com.apple.windowserver.active`, `com.apple.tccd`. Erlaubt (aus `system.sb`): `com.apple.cfprefsd.agent`,
   `com.apple.trustd.agent`, `com.apple.system.opendirectoryd.libinfo`.

**Weiterreicher und Sitzungsports.** Wie unter Windows: Jeder Weiterreicher bindet Port 0 auf `127.0.0.1` und denselben
Port auf `[::1]` (sonst neu, höchstens 5 Versuche) und schreibt dann genau eine Zeile `{"bereit":true,"port":<n>}` und
LF auf stdout (`$defs/bereit`, Fälle `bereit-*`). Frist 10 s. Der Starter nimmt den Port nicht blind: Er prüft mit libproc,
dass beide Hörer dem Weiterreicher gehören, den er selbst gestartet hat, und dass der Port nicht der der Browser-Brücke
ist. Sonst Exit 69.

**Peer-Prüfung (A143).** Weil der Loopback des Rechners allen Prozessen offensteht, prüfen Modellweg und Ausgang je
Verbindung den Schein. Die Zuordnung ohne root **gelingt** (Messlauf 2c, Abschnitt `peer`, Sonde über libproc): Die Verbindung aus dem Lauf wird einer
PID zugeordnet, die das Merkmal trägt (`merkmal: true`, 0,008 s); eine Verbindung von außerhalb einer PID ohne Merkmal
(`merkmal: false`, 0,007 s). Das beantwortet den Weg, nicht den Schutz: Der Schein steht in der Umgebung und ist für den Agenten
lesbar (Grenze), die Zuordnung ersetzt ihn nicht, sondern ergänzt ihn. Der Weiterreicher selbst ist nicht gebaut (OPEN 1).

### Such-Socket (Vertrag 4 unter macOS)

**Entscheidung: Unix-Socket als `path-literal`, kein Sitzungsport.** Gründe:

1. Gemessen: Der eigene Socket antwortet (U1), der Socket einer anderen Sitzung im selben Ordner ist mit EPERM gesperrt
   (U2). Die Sperrzeile für den 0700-Ordner des Dienstes stört die Verbindung nicht, weil `network-outbound` eine eigene
   Operation ist (U1 lief mit dieser Sperrzeile).
2. Linux (Socket im Einschluss) und Windows (Named Pipe, Stand vom 20.11.2026) nehmen für die Suche keinen TCP-Port. Ein Socket im
   0700-Ordner mit Modus 0600 ist für andere Kennungen zu; ein Loopback-Port wäre für jeden Prozess des Rechners offen
   (A120).
3. `such-mcp` spricht den Socket über stdio des Harness an; ein Port ist dafür nicht nötig.

Das weicht vom Nachtrag ab, der den Such-Kanal unter den Sitzungsports nennt. Frage siehe unten.
`such-mcp` bekommt unter macOS `--socket <hostpfad>`, denn den Gastpfad `/run/brain/such.sock` gibt es nicht (Nachtrag
zu Vertrag 4, wie unter Windows). `--socket-rechte` ist `0600`: Such-Prozess und Agent haben dieselbe Kennung. Der
Hostpfad ist höchstens 103 Byte lang; der Ordner liegt deshalb kurz unter dem Heim, etwa `~/.wbs/<sitzung>/`, Modus
0700. Gemessen ist die Verbindung mit relativem Pfad aus dem Socket-Ordner (U1) und mit dem absoluten Pfad, der als `path-literal` im
Profil steht (U3: verbunden; U4: ein Socket außerhalb des `path-literal`, absolut, EPERM; Messlauf 2c).

### Prozesse (E3)

Der Such-Prozess läuft außerhalb des Profils unter derselben Kennung. Seatbelt verbirgt ihn nicht; es sperrt den Zugriff:

| Versuch des Agenten | ohne Härtung | mit Härtung | ohne Profil |
|---|---|---|---|
| Signal 0, SIGCONT (Sonde, `/bin/kill`) | EPERM (P1, P2, P8) | EPERM (H4) | gelingt |
| Umgebung lesen (`KERN_PROCARGS2`) | **lesbar** (P3) | **lesbar** (H1; Härtung ohne Wirkung) | lesbar |
| `proc_pidinfo`, `proc_pidpath` | **gelingt** (P5, P6) | **gelingt** (H2, H3; Härtung ohne Wirkung) | gelingt |
| `task_for_pid` | scheitert, `kern=5` (P4) | | scheitert auch ohne Profil (P4) |
| `/bin/ps` | startet nicht: `execvp … Operation not permitted` (P7) | | gelingt |
| Signal an den Agenten einer anderen Sitzung | EPERM (P9) | | gelingt |

Die „Härtung“ sind die zwei Sperrzeilen nach der Sperrzeile für geschützte Ordner. **Sie wirkt nicht** (H1 bis H3 gemessen,
Messlauf 2c); gesperrt bleibt nur das Signal (H4), und der eigene Kind-Prozess bleibt erreichbar (H5, H6). Die Zeilen bleiben,
schützen aber nichts. Dazu gilt wie unter Linux: Die Umgebung
des Such-Prozesses ist leer (`env_clear`), und sein argv enthält kein Geheimnis. `PR_SET_DUMPABLE` gibt es unter macOS
nicht; `task_for_pid` scheitert für Prozesse derselben Kennung ohne Debug-Recht schon ohne Profil (gemessen P4). Die
Bindung an die Lebensdauer des Starters geht unter macOS über `kqueue` mit `EVFILT_PROC` und `NOTE_EXIT` auf die PID des
Starters im Such-Prozess (hergeleitet; Vertrag 1 nennt den Weg).

### Prozessende: Wächteramt des Dienstes

Entschieden (Betreiber, 20.11.2026): die schwächere Zusage mit Wächteramt im Dienst, keine VM je Lauf (A142, A03
Ergänzung macOS).

**Festlegung: Wächter und Dienst sind ein Prozess.** Der Kern-Dienst hält das Wächteramt selbst, als launchd-Dienst mit
`KeepAlive` (A147). Gründe: (1) Der Dienst kennt Sitzungen, Startdateien und den 0700-Ordner mit den Merkmalen schon; ein
zweiter Prozess bräuchte einen eigenen Kanal und eigene Rechte auf diesen Ordner. (2) Ein eigener Wächterprozess kann
ebenso hart sterben; er verschiebt die Lücke, schließt sie nicht. (3) Mit `KeepAlive` startet launchd den Dienst nach
einem harten Ende neu, und der nächste Start räumt zuerst auf (M16). Die Lücke „Nachfahren leben bis zum nächsten
Start“ ist damit so kurz wie der Neustart durch launchd (hergeleitet; nicht gemessen).

**Merkmal des Laufs.** Der Dienst legt je Lauf in seinem 0700-Ordner `merkmal/<sitzung>/` zwei leere Dateien an,
`kennung` und `gegen`, beide Modus 0400. Das Profil des Laufs nennt `kennung` als letzte Zeile mit `file-write-data`;
`gegen` nennt es nicht. Ein Prozess gehört zum Lauf genau dann, wenn `sandbox_check(pid, "file-write-data", PATH, kennung)`
„erlaubt“ und dieselbe Frage für `gegen` „verboten“ ergibt. Gründe aus den Messungen:

- Prozessgruppe und Sitzung taugen nicht: Von fünf Prozessen des Laufs endeten die drei der Gruppe mit `killpg`; die zwei gelösten
  (Enkel nach `setsid` mit doppeltem fork, Kind mit `posix_spawn` und `POSIX_SPAWN_SETSID`) lebten weiter
  (`lebt_nach_killpg`, Messlauf 2c).
- Eine Marke in der Umgebung (Sitzungskennung, Schein) taugt nicht: Die Umgebung bleibt für den Agenten lesbar (Grenze), und
  er kann sie in seinen Kindern ändern oder löschen, und für Plattformprogramme zeigt macOS die Umgebung nicht
  (w92).
- Das Merkmal hängt am Profil, das der Prozess nicht ablegen kann (gemessen D17, D18: kein neues Profil im Lauf). Ein
  Profil mit breitem Schreibrecht erlaubt beide Dateien, eines ohne Recht keine (w92: 0 von 217 fremden Prozessen).
  Messlauf 2c: Der Agent der anderen Sitzung (`agent_b_traegt_merkmal: false`) und der Such-Prozess (`such_traegt_merkmal: false`)
  tragen das Merkmal nicht; von 655 geprüften eigenen Prozessen waren genau die zwei gelösten Treffer, fremde Treffer 0.

**Ablauf.** Endet die Sitzung (Harness endet oder der Kern-Dienst beendet sie), sendet der Wächter SIGTERM an die
Prozessgruppe des Laufs und an jeden eigenen Prozess mit dem Merkmal. Nach der **Frist von 5 s** sendet er jedem, der mit
derselben Startzeit noch lebt, SIGKILL. Er signalisiert nie nach Namen oder Muster und nie eine PID ohne Merkmal. Danach
löscht er `merkmal/<sitzung>/`. Gemessen (Messlauf 2c, `messlauf2-ergebnis.json`, Abschnitt `prozessende`): Suche über 655 eigene Prozesse in 2,375 s; 2 von 2
gelösten Prozessen gefunden, 0 fremde Treffer; Ende nach SIGTERM in 0,015 s (beide beendet, `lebt_nach_sigterm` leer). Die Frist
von 5 s lässt gegen die gemessenen 0,015 s Luft für träge Programme (Projektentscheid, 21.11.2026). Die Suche kostet gemessen
etwa 3,6 ms je Prozess (hergeleitet aus 2,375 s durch 655); der Wächter sucht nur beim Ende einer Sitzung und beim Start.

**Wächter stirbt durch SIGKILL.** Stirbt der Dienst hart, bleiben Prozesse des Laufs zurück. Sie bleiben eingeschlossen
(Dateien und Netz, gemessen D16 und w92). Beim nächsten Start sucht der Dienst vor jeder neuen Sitzung alle Ordner unter
`merkmal/`, beendet jeden eigenen Prozess mit einem dieser Merkmale wie oben und löscht die Ordner; erst danach startet er
eine Sitzung (M16). Fehlt `kennung`, trägt niemand das Merkmal mehr (w92); darum löscht der Dienst die Merkmal-Ordner
erst nach dem Aufräumen.

### Selbstprüfung als Tor (fail closed)

Vor jedem Start prüft der Starter das fertige Profil mit `/usr/bin/sandbox-exec -f <profil>` an Kanarien, Frist 5 s je
Kanarie:

| Kanarie | muss |
|---|---|
| eine Datei im Arbeitsordner lesen | gelingen |
| eine Kanarie-Datei im 0700-Ordner des Dienstes lesen | mit EPERM scheitern |
| einen eigenen Hörer des Starters auf einem Loopback-Port außerhalb der Sitzungsports verbinden | mit EPERM scheitern |
| Signal 0 an einen Prozess der Person außerhalb des Profils (ein Kind des Starters) | mit EPERM scheitern (H4) |

Die Umgebung eines Prozesses außerhalb ist **keine** Kanarie: Sie bleibt unter dem Profil lesbar (H1, Grenze), eine Kanarie
könnte nie grün werden.

Ein Fehler ohne EPERM zählt nicht als gesperrt. Lässt sich die Prüfung nicht ausführen, gelingt eine erlaubte Kanarie
nicht, oder ist eine gesperrte offen, startet kein Agent: Exit 69, `lauf.einschluss_fehlt`. Der Start unter dem Profil
kostet gemessen 0,013 bis 0,015 s (Messlauf 1, 10 Starts von `/usr/bin/true`; ohne Profil 0,002 bis 0,003 s).
Vorher prüft der Starter `/usr/bin/sandbox-exec`: fester Pfad, nach `realpath` derselbe, Eigentümer root, für andere nicht
schreibbar (`SeatbeltFremd`); fehlt es, `SeatbeltFehlt`.

### Ablauf in „Lauf starten“ unter macOS

Schritte 1 bis 5 und die stderr-Zeile gelten wie in Vertrag 1. Schritte 6 bis 8:

| Schritt | Handlung | bei Fehler |
|---|---|---|
| 6a | Einschlussdatei lesen und prüfen (Stufen 1 bis 13) | 69, `lauf.einschluss_fehlt` |
| 6b | Arbeitsordner: echter Pfad gleich dem angegebenen, echt unter einer Projektwurzel, ohne `"` und `\` | 69, `lauf.einschluss_fehlt` |
| 6c | `/usr/bin/sandbox-exec` prüfen; Sitzungsheim mit `tmp` anlegen; Merkmal-Ordner anlegen | 69, `lauf.einschluss_fehlt` |
| 7a | Such-Prozess starten und auf `{"bereit":true}` warten, Frist 10 s (Vertrag 1) | 69, wie Vertrag 1 |
| 7b | Modellweg und Ausgang nach ihren Verträgen, dann je einen Weiterreicher starten; Ports sammeln und prüfen, Frist 10 s | 69, `lauf.einschluss_fehlt` |
| 8a | Profil bauen, am Text prüfen und in den 0700-Ordner schreiben (Modus 0600) | 69, `lauf.einschluss_fehlt` |
| 8b | Selbstprüfung an den Kanarien | 69, `lauf.einschluss_fehlt` |
| 8c | PIDs von Such-Prozess und Weiterreichern in `suche.pid` und `weiter.pid` schreiben; Umgebung setzen; `exec /usr/bin/sandbox-exec -f <profil> -- <argv>` | 69, `lauf.harness_fehlt` |

`sandbox-exec` ersetzt sich nach dem Anwenden des Profils per exec durch den Harness. Die PID bleibt also über beide exec
gleich, und das exec-Modell von Vertrag 1 hält (hergeleitet aus dem Verhalten von `sandbox-exec`; die Messung zeigt,
dass der Prozess unter `sandbox-exec` dieselbe PID trägt wie der Starter, Agent B in Messlauf 1). Scheitert ein Schritt
nach 7a, beendet der Starter Such-Prozess und Weiterreicher mit SIGTERM, bevor er endet.

### Umgebung

Leer bis auf diese Liste; der Starter setzt sie selbst:

| Variable | Wert |
|---|---|
| `HOME` | das Sitzungsheim |
| `PATH` | die Programmordner, `/usr/bin`, `/bin`, `/usr/sbin`, `/sbin` |
| `TERM`, `LANG` | wie unter Linux |
| `TMPDIR` | `<sitzungsheim>/tmp` |
| `CLAUDE_CODE_TMPDIR` | wie `TMPDIR`; Claude Code ignoriert `TMPDIR` und endet sonst still mit rc 1 (w76) |
| `ANTHROPIC_BASE_URL` | `http://127.0.0.1:<Port des Weiterreichers zum Modellweg>` |
| `ANTHROPIC_AUTH_TOKEN` | der Schein dieses Laufs |
| `HTTPS_PROXY` | `http://127.0.0.1:<Port des Weiterreichers zum Ausgang>`, nur mit Cloud-Zielen |

`SSH_AUTH_SOCK`, `DEVELOPER_DIR` und jede Variable mit Zugangsdaten außer dem Schein fehlen. Nur die Deskriptoren 0 bis 2
bleiben offen.

## Messproben

Drei Läufe auf einer ruhigen Messmaschine (Messlauf 3 nur für git), je in einem frischen Wegwerf-Ordner, mit einer
eigenen Sonde in C und festen Kindprogrammen (`cat`, `curl`, `kill`, `sh`, `zsh`, `git`). Kein echter Harness, kein Netz
nach außen: Hörer nur auf `127.0.0.1` und `[::1]`, fremde Adresse ist die eigene LAN-Adresse. Jede Probe lief einmal unter
dem Profil und einmal ohne (Gegenprobe der Gegenprobe). Profil wie oben, ohne Härtung in Messlauf 1; Messlauf 2c prüft zusätzlich das gehärtete Profil.

| Probe | Handlung | mit Profil | ohne Profil |
|---|---|---|---|
| D1, D2 | Arbeitsordner lesen, schreiben | gelingt | gelingt |
| D3 | Notiz des Vaults lesen | EPERM | gelingt |
| D4 bis D6 | Wegwerf-Token `~/.copilot/token`, `~/.config/gh/hosts.yml`, `~/.ssh/id_wegwerf` | EPERM | gelingt |
| D7 | Startdatei im 0700-Ordner | EPERM | gelingt |
| D8 | Heim auflisten | EPERM | gelingt |
| D9 | Verweis im Arbeitsordner auf die Notiz | EPERM | gelingt |
| D10 | harten Link auf die Notiz anlegen | EPERM | gelingt |
| D11 | harter Link, der vor dem Lauf im Arbeitsordner lag | **gelingt** | gelingt |
| D12 | Arbeitsordner in anderer Schreibweise | gelingt | gelingt |
| D13 | Notiz in anderer Schreibweise | EPERM | gelingt |
| D14 | Wurzel des Wegwerf-Ordners auflisten | EPERM | gelingt |
| D15, D16 | Enkel und abgelöster Nachfahr lesen die Notiz | EPERM, EPERM (der Nachfahr bleibt eingeschlossen) | gelingt |
| D17 | `sandbox_init` mit `(allow default)` im Lauf | `Operation not permitted` | gelingt |
| D18 | `sandbox-exec` mit `(allow default)` im Lauf | Exit 71, `sandbox_apply: Operation not permitted` | gelingt |
| D19 | Plist außerhalb über `defaults read` | Exit 1, `Domain … does not exist` (nicht lesbar) | gelingt, Wert gelesen |
| D20 | `cat` der Notiz | Fehler | gelingt |
| N1, N2 | eigener Modellweg über `127.0.0.1`, `[::1]` | verbunden | verbunden |
| N3, N4 | weitere eigene Sitzungsports | verbunden | verbunden |
| N5, N12 | Browser-Brücke (Sonde, `curl`) | EPERM, curl Exit 7 | verbunden |
| N6 | Modellweg-Port einer anderen Sitzung | EPERM | verbunden |
| N7 | eigene LAN-Adresse, Port ohne Hörer | EPERM | Zeitgrenze 10 s |
| N8 | eigene LAN-Adresse, Modellweg-Port | **Zeitgrenze 10 s, keine Sperre** | Zeitgrenze 10 s |
| N9 | UDP an die eigene LAN-Adresse | EPERM | gesendet |
| N10 | auf `127.0.0.1` hören | EPERM | gelingt |
| N11, N14 | `curl` an den Modellweg über `127.0.0.1` und `localhost` | Antwort | Antwort |
| N13 | E1: `ANTHROPIC_BASE_URL` zeigt auf die Brücke | curl Exit 7 | Antwort der Brücke |
| N15 | Modellweg A unter dem Profil der Sitzung B | EPERM | verbunden |
| U1, U2 | eigener und fremder Such-Socket | Antwort, EPERM | beide Antwort |
| L1 | `launchctl submit` eines Auftrags, der die Notiz liest | Exit 1, kein Auftrag, keine Marke | |
| Reihenfolge | `allow` dann `deny` auf dieselbe Datei; umgekehrt | EPERM; gelingt | |

**Messlauf 2c** (21.11.; gültig ist Lauf 2c, frühere ungültige Läufe
zählen nicht). Jede Probe lief mit Profil und ohne:

| Probe | Handlung | mit Profil | ohne Profil |
|---|---|---|---|
| P1, P2, P8 | Signal 0, SIGCONT (Sonde, `/bin/kill`) an den Such-Prozess | EPERM | gelingt |
| P3 | Umgebung des Such-Prozesses (`KERN_PROCARGS2`) | **lesbar, Marke gefunden** | lesbar |
| P4 | `task_for_pid` | `kern=5` | `kern=5` |
| P5, P6 | `proc_pidinfo`, `proc_pidpath` | **gelingt** | gelingt |
| P7, P11 | `/bin/ps` | startet nicht (`execvp … Operation not permitted`) | gelingt |
| P9, P10 | Signal an den Agenten der anderen Sitzung; an das eigene Kind | EPERM; gelingt | gelingt; gelingt |
| H1 bis H3 | wie P3, P5, P6 unter dem gehärteten Profil | **weiter lesbar** | lesbar |
| H4 bis H6 | Signal an den Such-Prozess; an das eigene Kind; Umgebung des eigenen Kindes | EPERM; gelingt; lesbar | gelingt |
| H7, H8 | `zsh`, `uname`, `sysctl hw.ncpu`, `date`; Arbeitsordner lesen, `curl` an den Modellweg | gelingt | gelingt |
| H9 | `git` im Arbeitsordner (Profil ohne den Ordner der Command Line Tools) | Exit 1, `xcode-select: error: No developer tools were found` | gelingt |
| N16 | `127.0.0.2` auf dem Modellweg-Port | EPERM | Zeitgrenze |
| N17, N18 | `224.0.0.1`, `255.255.255.255` | EPERM | `errno=47` |
| N19 | UDP an `127.0.0.1` auf dem Modellweg-Port | **gesendet** | gesendet |
| N20 | `0.0.0.0` auf dem Modellweg-Port | **verbunden** | verbunden |
| N21, N22 | `[::ffff:127.0.0.1]` auf Modellweg- und Brücken-Port | EPERM | verbunden |
| N23 | UDP an `127.0.0.1` auf den Brücken-Port | EPERM | gesendet |
| U3, U4 | eigener Such-Socket absolut; Socket außerhalb des `path-literal`, absolut | verbunden; EPERM | verbunden; verbunden |
| Mach | `sandbox_check` `mach-lookup`, Profil des Agenten | `dnssd`, `SecurityServer`, `securityd`, `launchservicesd`, `appleevents`, `pasteboard.1`, `windowserver.active`, `tccd` verboten; `cfprefsd.agent`, `trustd.agent`, `opendirectoryd.libinfo` erlaubt | |
| Peer | Zuordnung der Verbindung zu einer PID ohne root (libproc) | gelingt, mit Merkmal innen, ohne außen | |
| Prozessende | gelöste Nachfahren, Merkmal-Suche, SIGTERM | siehe „Prozessende“ | |
| Start | 10 Starts von `/usr/bin/true` | 0,010 bis 0,012 s | 0,001 bis 0,002 s |
| Reihenfolge, `launchctl` | `allow` dann `deny` und umgekehrt; `launchctl submit` | EPERM, dann gelingt; Exit 1 | |

**Messlauf 3** (22.11., git): siehe „Regel für git“.

## Sicherheitszusagen (Eigenschaften)

Jede Eigenschaft (M1 bis M17) hat einen Außentest `mac-m<n>-fall` und eine Gegenprobe `mac-m<n>-gegenprobe`. Die Gegenprobe baut die
Schwäche absichtlich ein oder läuft ohne Profil und muss rot werden.

| | Eigenschaft | Anforderung | Stand |
|---|---|---|---|
| M1 | Der Agent liest die Notiz nicht: direkt, über Verweis, in anderer Schreibweise, als Enkel, abgelöst | A142 | gemessen |
| M2 | Keine fremde Adresse: TCP und UDP an Nicht-Loopback, kein Namensdienst | A145 | gemessen; Grenze N8 (eigene LAN-Adresse auf dem Sitzungsport, keine Sperre); Namensdienst nach `sandbox_check` verboten, nicht echt aufgelöst |
| M3 | Die Browser-Brücke ist unerreichbar | A145 | gemessen |
| M4 | Der eigene Modellweg ist erreichbar; das Profil einer anderen Sitzung erreicht ihn nicht | A145 | gemessen |
| M5 | Der Modellweg-Port einer anderen Sitzung ist unerreichbar | A145 | gemessen |
| M6 | E1: Eine Umgebungsvariable ändert die Route nicht | A145 | gemessen |
| M7 | Token und Schlüsseldateien, Startdatei und Heim sind nicht lesbar | A144 | gemessen an Wegwerf-Dateien |
| M8 | E3: Der Such-Prozess ist nicht signalisierbar. **Umgebung und Prozessauskunft sind nicht zu** (Grenze), ein Geheimnis darf nur der Halter tragen | A142 | Signal gemessen; Umgebung, `proc_pidinfo`, `proc_pidpath` lesbar (gemessen) |
| M9 | Nur der Such-Socket der eigenen Sitzung ist erreichbar | A142 | gemessen (relativer Pfad) |
| M10 | Der Agent hört auf keinem Port | A145 | gemessen |
| M11 | Kein Prozess im Lauf lockert sein Profil | A142 | gemessen |
| M12 | Der Agent einer anderen Sitzung ist nicht signalisierbar | A142 | gemessen |
| M13 | `launchctl submit` startet nichts außerhalb des Profils | A142 | gemessen (Exit 1) |
| M14 | Selbstprüfung als Tor, fail closed | A142 | Startkosten gemessen; Tor nicht gebaut |
| M15 | Nach Sitzungsende und Frist von 5 s lebt kein Prozess mit dem Merkmal | A142 | gemessen (Messlauf 2c) |
| M16 | Stirbt der Wächter, räumt der nächste Start auf, bevor er startet | A142 | hergeleitet |
| M17 | `git` läuft unter dem Profil (Ordner der Command Line Tools, nur Lesen); eine Funktionszusage, keine Sicherheitszusage | A142 | gemessen (Messlauf 3) |

## Mutanten für den Bau

Je Mutant muss mindestens ein Fall rot werden: Profil ohne `(deny default)` oder mit `(allow default)`; ohne Sperrzeile
`geschuetzt`; Sperrzeile vor den Freigaben statt danach; ohne `sysctl`-Sperre; ohne `process-info`-Sperre; `localhost`
durch `*` ersetzt; ein Sitzungsport zu viel (Port der Brücke); Merkmal-Zeile vor der Sperrzeile; Selbstprüfung
übersprungen; Selbstprüfung grün trotz wirkungslosem Profil; Kanarie ohne EPERM als gesperrt gezählt; Wächter beendet
nur die Prozessgruppe; Wächter signalisiert nach Namen; kein Aufräumen beim nächsten Start; Merkmal-Ordner vor dem
Aufräumen gelöscht; Pfadvergleich mit Groß- und Kleinschreibung; zweite Prüfung am Text ohne Normalisierung des Leerraums oder
mit Regel `allow` ohne Filter; Profil ohne den Ordner der Command Line Tools; ein Geheimnis in Umgebung oder argv eines Prozesses
der Person.

## Prüfbarkeit der Fälle

`pruefen.py` prüft heute: Paarung, Pflichtfelder je Form, Größe und JSON-Lesen; die Einschlussdatei gegen eine Referenz
der Stufen 1 bis 8 und gegen das Schema (ohne `jsonschema` Exit 2; mit `--ohne-schema` bewusst nur das Muster `posixpfad` mit `re`); die
Bereitschaftszeilen gegen eine Referenz; jeden Fall `profil` gegen einen Referenzbau des Profils nach diesem Text; jeden Fall `profiltext` gegen die zweite Prüfung am
Text; die stderr-Zeilen der Fälle `aufruf` gegen Vertrag 1; je Eigenschaft M1 bis M17 genau einen Fall und eine Gegenprobe mit
Anforderung und Beleg. Erst mit dem Bau prüfbar sind die Stufen 10 bis 13 der Einschlussdatei, alle Fälle `aufruf` und
alle Fälle `mac`.

```sh
python3 -I rust/spec/macos-einschluss/bau-faelle.py   # Schema und Fälle neu erzeugen
<python mit jsonschema> -I rust/spec/macos-einschluss/pruefen.py   # Exit 0 bei 0 Fehlern; ohne jsonschema Exit 2 (bewusst ohne Schema: --ohne-schema)
```

## Abstimmung mit anderen Bauteilen

| Bauteil | liest oder liefert | Annahme dieses Vertrags |
|---|---|---|
| Vertrag 1 (Crate `einschluss`) | Modul `macos` hinter `cfg(target_os = "macos")`; Felder wie in der Tabelle oben | Das Projekt nimmt `merkmal` und `sitzungsports` in die gemeinsame Beschreibung auf oder lässt sie macOS-eigen |
| Vertrag 4 | `such-mcp --socket <hostpfad>` unter macOS; Socket-Ordner kurz | Nachtrag zu Vertrag 4 wie unter Windows |
| Kern, Dienst | Wächteramt, Merkmal-Ordner, Aufräumen beim Start; Weiterreicher; Browser-Brücke nie Sitzungsport | Das Projekt baut das Modul `macos`; Pflicht vor Schritt II: Geheimnisse nur beim Halter (`_wbhalter`), nie in einem Prozess der Person |
| Anforderungen | M1 bis M17 als Außentests zu A142 bis A145; der Absatz „Grenze unter macOS“ als Quelle für A03 und A141 | A146 (Paket, unsigniert ohne Developer-Konto) gehört nicht hierher |

## OPEN

1. **Weiterreicher mit Peer-Prüfung.** Die Zuordnung der Verbindung zu einer PID ohne root gelingt in der Sonde (Messlauf 2c). Ein
   Weiterreicher, der sie je Verbindung nutzt, ist nicht gebaut und nicht gemessen.
2. **LAN-Adresse auf dem Sitzungsport.** `localhost:<port>` deckt auch die eigene LAN-Adresse (N8: keine Sperre). Gemessen ist
   außerdem, dass `localhost` `127.0.0.2`, Multicast, Broadcast und `[::ffff:127.0.0.1]` nicht deckt (N16 bis N18, N21), wohl
   aber `0.0.0.0` (N20) und UDP auf dem Sitzungsport (N19). Ein fremder Hörer auf `<LAN-Adresse>:<Sitzungsport>` wäre für den Agenten
   erreichbar; der Starter prüft ihn vor dem exec (Entwurf, nicht gebaut).
3. **Harte Links.** Ein harter Link, der vor dem Lauf im Arbeitsordner lag, ist lesbar (D11), auch wenn sein Ziel geschützt ist.
   Anlegen kann der Agent ihn nicht (D10). Unter Linux gilt dasselbe für eingehängte Ordner (hergeleitet).
4. **Status von `sandbox-exec`.** Apple führt `sandbox-exec` und `sandbox_init` als veraltet; `system.sb` nennt sich „Apple System
   Private Interface … subject to change“. Es wirkt in macOS 26.5.1 (gemessen) und 26.6.2 (w76). Fällt es weg oder wirkt es nicht
   mehr, schließt die Selbstprüfung den Start (M14).
5. **Terminal.** `TIOCSTI` und eine Sperre bestimmter `ioctl`-Befehle sind unter macOS nicht gemessen. Das Profil erlaubt
   `file-ioctl` auf der PTY der Sitzung.
6. **Mach-Dienste.** Gemessen ist die Auskunft von `sandbox_check` für elf Dienste (Abschnitt „Netz“), nicht jeder Dienst aus
   `system.sb` und keine echte Namensauflösung im Lauf. Eine Zusage gegen eigenen Mach-Code gibt Seatbelt nicht.
7. **Nicht gemessen:** der echte Harness unter diesem Profil im Neubau (w92 maß Claude Code unter dem alten Profil mit rc 0);
   x86_64; andere macOS-Versionen; Neustart des Dienstes (M16); `Ueberlappt`, Selbstprüfung und Wächter im Bau.
8. **Halter unter eigener Kennung.** Ob der Halter (`_wbhalter`) ohne Adminrechte der Person angelegt werden kann, ist nicht
   gemessen und gehört in die Entscheidung zu Schritt II.
9. **git auf einem Mac nur mit Xcode.** Gemessen ist `git` nur mit den Command Line Tools unter `/Library/Developer/CommandLineTools`
   (Messlauf 3). Mit nur `Xcode.app` scheiterten alle gemessenen Freigaben (V2 bis V6). Der Schreibschutz des Ordners der Command
   Line Tools ist hergeleitet (die Regel nennt nur Lesen); der Schreibversuch wurde an `Xcode.app` gemessen.

<!-- vom Bauvorgang angefuegt -->
> Hinweis: Dieser Text nennt Werkzeuge, die in diesem Klon nicht liegen: `brain`.
> Sie gehoeren zum Setup der Maschine, aus der dieser Baum stammt; hier fehlen
> sie mit Absicht. Wer sie braucht, schreibt sich seine eigenen.
> *Named here, not shipped here.*
