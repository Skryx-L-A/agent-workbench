# Vertrag 8: Modellweg (Fassung 1)

Stand 20.11.2026. Teil des Durchstichs (erste Vorführung) und dünne Form des Gateways aus Vertrag 7 (`rust/spec/gateway/`, Entwurf).
Anforderungen: A06 (das eingestellte Modell, kein anderes Ziel), A13 (kein fester Anbieter, kein Rückfall), A14 und A106 (Schlüssel nie beim Agenten), A102 (Kette), A12 und A104 (kein Inhalt, kein Name im Protokoll).
Grundlage sind die Festlegungen am 20.11.2026, die Messungen zur Copilot CLI und die Annahmen zum Modell-Gateway.
Der Vertrag legt Bytes und Rückgaben fest, bevor Code entsteht. Die sichtbaren Fälle stehen in `faelle/`; verdeckte Fälle und der Mutationslauf folgen mit dem Bau. Alle Fälle sind heute rot.

Kennzeichen:
- „(V)“ heißt: in diesem Vertrag festgelegt.
- „(H)“ heißt: hergeleitet.
- „(F)“ heißt: Annahme, nicht von uns gemessen.
- „(G)“ heißt: gemessen mit Copilot CLI 1.0.88 auf dem Linux-Baurechner in bwrap ohne Netz, gegen eine Attrappe und ohne echtes Konto.

**Gliederung.** Teil A gilt für jede Betriebsart. Teil B beschreibt Betriebsart 1 `openai-lokal`; sie wird im Durchstich gebaut. Teil C beschreibt Betriebsart 2 `kopf-einsetzen`.
Sie ist entschieden und gehört zu M1 (Projektentscheid, 20.11.2026): **Bau direkt nach `openai-lokal`.** Eine neue Betriebsart kommt als eigener Teil dazu und ändert Teil A und Teil B nicht.
Teil D enthält Außentests, Fälle, Bau und Ausblick.

Die Beispielnamen in Vertrag, Fällen und Erzeuger sind neutral. Die echten Modellnamen stehen nur in der Vorführdatei der Einstellungen. Die Ziele der Copilot CLI sind technische Ziele des Werkzeugs und stehen deshalb mit ihrem Namen da.

## 1. Zweck und Grenze

Der Modellweg ist ein **Weiterreicher außerhalb des Einschlusses**. Er nimmt Anfragen des Harness aus dem Einschluss an und setzt einen Schlüssel aus einer Datei ein, die der Agent nicht erreicht. Er reicht nur an Ziele aus seiner Konfiguration weiter.

- **Betriebsart 1 `openai-lokal`** (Teil B) dient der lokalen Route. Ein lokaler Harness sendet OpenAI-kompatible Anfragen an ein selbst gehostetes Modell.
  - **Ziel (F):** ein allgemeines OpenAI-kompatibles Modell-Gateway mit `/v1/chat/completions`, in einem internen Netz erreichbar. Ein Hauptmodell kann ein Feld `reasoning` mitliefern; ein Ausweichmodell ist möglich. Hier heißen sie `lokal-beispiel/modell-a` und `lokal-beispiel/modell-b`.
    Jeder Aufruf braucht `Authorization: Bearer <virtueller Schlüssel je Rechner>`. Ein nicht freigeschaltetes Modell beantwortet das Ziel mit 403 `policy_model_blocked`.
  - Bis ein Gateway benannt ist, ist das Ziel ein kleines Modell über Ollama auf dem Rechner (`http://127.0.0.1:11434/v1`).
- **Betriebsart 2 `kopf-einsetzen`** (Teil C) dient der Cloud-Route mit Copilot CLI. Die CLI schickt das GitHub-Token selbst an ihre Dienste (G). Damit das Token nicht im Einschluss liegt, steht dort nur ein Platzhalter.
  Der Weiterreicher beendet TLS für wenige benannte Ziele und prüft Methode und Pfad. **Den Kopf mit dem echten Token setzt ab Schritt I der Halter ein** (Abschnitt 5.1, Vertragsbefund B05 „Halterweg vor Schritt I", Entscheid des Betreibers, 23.11.2026); der Weiterreicher kennt den Token nie.
  **Grenze, ehrlich (Projektentscheid, 20.11.2026, Punkt 3):** Außerhalb des Einschlusses schützt nichts davon; die CLI trägt das Token dann direkt zum Host. Die Zusage gilt nur für Läufe über „Lauf starten“.
- **Kern-Dienst und Ansicht** brauchen im Durchstich nichts vom Modellweg (Projektentscheid, 20.11.2026). Fehler des Ziels sieht die Person im Terminal des Harness.
- Der Modellweg entscheidet nicht über Wissen. Welche Kundendaten der Agent sieht, entscheiden `darf` (Vertrag 3) und der Einschluss.

Nicht in diesem Vertrag: Kosten in Geld, Registry, mehrere Modelle je Route, Lauf-Schein (alles Vertrag 7); ein lesender Stand je Sitzung (später, Abschnitt 18); ein Mitschnitt von Inhalten (zurückgezogen, Abschnitt 9).

---

# Teil A: für jede Betriebsart

## 2. Betriebsarten

| Betriebsart | Stand | Zweck | Teil |
|---|---|---|---|
| `openai-lokal` | Fassung 1, im Durchstich gebaut, Vorgabe | ein OpenAI-kompatibles Ziel, genau ein Pfad, genau ein Modell | B |
| `kopf-einsetzen` | entschieden, Bau direkt nach `openai-lokal` (M1) | CONNECT mit TLS-Abschluss für benannte Ziele, Tausch des Anmeldekopfs, Methode und Pfad je Ziel | C |

Ein Prozess hat genau eine Betriebsart, gewählt mit `--betriebsart` (Abschnitt 4). Wo Betriebsart 2 nicht gebaut ist, endet `--betriebsart kopf-einsetzen` vor der Bereitschaft mit `mw.betriebsart_unbekannt` (Fälle `betriebsart_kopf_einsetzen` und `betriebsart_vor_form` in `faelle/aufruf.json`, Feld `erwartet`). Wo sie gebaut ist, gilt für dieselben Fälle das Feld `erwartet_gebaut`: `mw.aufruf`, weil die Optionen von `openai-lokal` dort fremd sind. Im Durchstich ist sie nur unter Linux gebaut (Bau 20.11.).
Eine Sitzung hat genau eine Route und genau einen Weiterreicher; der Name `modellweg.pid` gilt für beide Betriebsarten (Projektentscheid, 20.11.2026).

## 3. Start und Lebensdauer (H, Festlegung)

„Lauf starten“ (Vertrag 1) startet den Modellweg je Sitzung **vor** dem Harness und außerhalb des Einschlusses, im selben Schritt wie den Such-Prozess.
Vertrag 1 hat dafür den Schritt „Modellweg starten“ mit `lauf.modellweg_fehlt` und `lauf.modellweg_nicht_bereit`, Exit 69 wie die Suche (Projektentscheid, 20.11.2026).

Es gilt dieselbe Bindung wie beim Such-Prozess (Vertrag 1, „Lebensdauer des Such-Prozesses“): `PR_SET_PDEATHSIG = SIGTERM` nach dem fork mit Prüfung von `getppid()` und eine eigene Sitzung (`setsid`).
Die PID steht in `modellweg.pid` neben `suche.pid` (0600; Projektentscheid, 20.11.2026). Am Sitzungsende räumt der Kern-Dienst auf.

**Start unter Linux (Projektentscheid, 25.11.2026, Nachtrag Rolle aus der Gruppe des Paketvertrags).** Unter Linux startet der Modellweg ab Schritt I nur über das setgid-Programm `/usr/lib/werkbank/werkbank-modellweg` (`root:wb-bruecke`, 2755; Paketvertrag, Nachtrag Rolle aus der Gruppe); er läuft dann mit `AT_SECURE` im sicheren Modus und nimmt nichts aus Umgebung, Argumenten oder Dateien des Aufrufers als vertraut, was seine Anfragen an den Halter lenkt (Halter-Vertrag, Abschnitt 2.3).
Grund: Der Halter nimmt die Rolle `bruecke` aus der effektiven gid des Aufrufers (Halter-Vertrag, Abschnitte 2.3 und 2.3a); ohne das setgid-Bit trägt der Modellweg die gid der Person und bekommt beim Halter `halter.peer`. Die Pflichten des sicheren Modus stehen in Abschnitt 3a. Keine Stelle der Kette darf vorher `PR_SET_NO_NEW_PRIVS` setzen, weil setgid bei `execve` dann nicht wirkt (Paketvertrag, Nachtrag Rolle aus der Gruppe). alt: Der Vertrag nannte für den Start unter Linux kein Rollenprogramm.

**Elternprozess.** Gemeint ist der Prozess von „Lauf starten“. Er startet den Modellweg in Schritt 7a über das Trampolin (`werkbank-lauf-kind`, gleiche PID nach dem exec) und bleibt danach dessen Elternprozess, bis der Starter endet; er ersetzt sich nicht mehr durch den Starter (Vertrag 1, „Elternprozess des Starters“, 21.11.2026). Sein `PR_SET_CHILD_SUBREAPER` gilt für adoptierte Enkel und ändert diese Bindung nicht. „Elternprozess“ ist darum der richtige Bezug: Das Todessignal kommt genau dann, wenn der Lauf endet.

**Zwei Regeln von Schritt 1 (Befund 21.11.2026, Züge 315 Waisen und `SigIgn` 0x5000).**
1. **Rennen um `getppid()`.** Stirbt der Elternprozess, bevor `PR_SET_PDEATHSIG` wirkt, sendet der Kern kein Signal mehr. Der Modellweg merkt sich darum `getppid()` in seiner ersten Zeile (vor der Stufe 0; nur das Einlesen von argv läuft davor) und liest es nach dem `prctl` erneut. Weicht der Wert ab, auch nach `None` (kein Elternprozess mehr), endet er sofort mit Exit 69 und `mw.eltern_weg`. Dieselbe Regel steht im Trampolin von Vertrag 1; dort wird gegen die übergebene PID geprüft, hier gegen die eigene erste Ablesung.
2. **Geerbtes `SIG_IGN` für SIGTERM.** Eine Stellung `SIG_IGN` überlebt `execve`. Erbt der Weiterreicher SIGTERM als ignoriert, verwirft der Kern das Todessignal, und `PR_SET_PDEATHSIG = SIGTERM` bleibt wirkungslos, solange kein Handler steht. Der Weiterreicher setzt darum **vor** dem `prctl` die Handler für SIGTERM und SIGINT (`signal_hook::iterator::Signals`, ohne eigenes `unsafe`) und startet sofort den Faden des Endes. Ab dort endet ein Signal den Prozess auch **vor** der Bereitschaft: Er reißt laufende Anfragen ab, wenn es schon einen Dienst gibt, löscht den Socket dieses Prozesses, wenn es schon einen gibt, und endet mit Exit 0. Kein Socket bleibt liegen; der Socket wird im selben Schritt gemerkt, in dem er entsteht (Schritt 7). Ohne Bereitschaftszeile meldet „Lauf starten“ `lauf.modellweg_nicht_bereit`.

Die Suche (Abschnitt 9a) setzt weiter nur das Todessignal mit der Prüfung aus Regel 1 und keine Handler: Sie ist ein kurzer Einzelaufruf, und ein Exit 0 auf ein Signal würde dort „nichts gefunden“ bedeuten (OPEN 29).

**Für den Agenten unerreichbar.** Ohne Vorrechte gibt es keine eigene Kennung. Es gilt die Eigenschaft E3 aus Vertrag 1: Der Agent kann dem Modellweg kein Signal senden,
seine Umgebung und seinen Speicher nicht lesen und ihn nicht anhängen. Variante A trägt das durch den eigenen PID-Raum des Einschlusses.
Der Modellweg führt zuerst die Stufe 0 aus (Umgebung, F1, weiter unten), schirmt sich danach selbst gegen ein Kernabbild ab und beginnt dann mit dem Rest von Schritt 1 (Abschnitt 4). Grund: Vor der Abschirmung ist noch kein Schlüssel gelesen, und die Stufe 0 liest keine Datei. Die Abschirmung ist `kernabbild::abschirmen` (Crate `rust/crates/kernabbild`, Regel „Kernabbild-Grenze 1/1“, Vertragsbefund B03 vom 21.11.2026): unter Linux `PR_SET_DUMPABLE` 0, auf jedem System `RLIMIT_CORE` weich 1 und hart 1; ist die geerbte harte Grenze schon 0, entscheidet das Crate nach `core_pattern`. Bricht sie ab, endet der Modellweg fail closed mit Exit 5 und `mw.kernabbild` (bis 22.11.2026: nur `PR_SET_DUMPABLE` 0 unter Linux, ein Fehler endete mit 70 und `mw.intern`). Danach setzt er die Handler für SIGTERM und SIGINT und erst dann unter Linux `PR_SET_PDEATHSIG` (ein Fehler bleibt 70, `mw.intern`), gefolgt von der Prüfung des Elternprozesses (69, `mw.eltern_weg`). Seine Umgebung ist leer (`env_clear`), stdin ist `/dev/null`, sein stderr geht nicht in die PTY.

**Umgebung des Weiterreichers (F1, Projektentscheid, 20.11.2026, für beide Betriebsarten).** OpenSSL folgt `SSL_CERT_FILE`, `SSL_CERT_DIR` und `OPENSSL_CONF` aus der Umgebung. Der Weiterreicher läuft außerhalb des Einschlusses und verlässt sich nicht darauf, dass der Aufrufer die Umgebung leert.
- Ist beim Start eine der drei Variablen gesetzt, auch leer, endet der Weiterreicher mit Exit 78 und `mw.umgebung_unsicher` (`Umgebung unsicher`). Das ist die **Stufe 0**: Sie läuft vor jedem anderen Schritt, auch vor der Prüfung des Aufrufs, vor dem Lesen jeder Datei und vor der Bereitschaft.
- Die Ausgabe ist die feste Zeile nach Abschnitt 4. Weder der Name noch der Wert der Variablen steht darin; ein Wert steht nie in einer Ausgabe oder einem Protokoll. Andere Namen, auch ähnliche wie `SSL_CERT_FILES` oder `OPENSSL_CONFX`, ändern nichts.
- **Hinweis:** „Lauf starten“ (Vertrag 1) lehnt den Start zusätzlich selbst ab, wenn eine der drei Variablen in der Umgebung steht, die es dem Weiterreicher gibt. Das ist nicht Teil dieses Vertrags; er legt nur die Prüfung im Weiterreicher fest.
- Die Fälle stehen in `faelle/umgebung.json`.

**Socket und Weiterleiter.** Der Socket des Modellwegs liegt in einem Ordner 0700 außerhalb des Einschlusses und wird per Bind an einem festen Pfad in den Einschluss gebracht. Der Ordner wird nie eingebunden.
Ein Weiterleiter des Startprogramms stellt den Socket im **eigenen Netz-Namensraum des Einschlusses** (`--unshare-net`) auf einem Port von `127.0.0.1` bereit.
Der Port ist in Betriebsart 1 `11434` und in Betriebsart 2 `8402` (Projektentscheid, 20.11.2026). Dieser Loopback gehört nur dem Einschluss; ein Dienst im Netz des Rechners kollidiert nicht (Projektentscheid, 20.11.2026).

**Ende.** Auf SIGTERM oder SIGINT löscht der Modellweg seinen Socket und endet mit Exit 0. Laufende Antworten reißen ab; für jede schreibt er vorher eine Zeile, wenn es gelingt.

## 3a. Sicherer Modus unter setgid (Zusagen, Projektentscheid, 25.11.2026)

**Stand des Baus (25.11.2026, Zug sicherer Modus).** Alle fünf Pflichten sind gebaut, jede mit eigenem Code und einem Außentest unter Linux (`rust/crates/modellweg/tests/sicherer_modus.rs`, MW-L1 bis MW-L5); OPEN 30 ist geschlossen. alt: „Dieser Abschnitt ist Vertragstext; gebaut ist davon nichts (OPEN 30, „Zug sicherer Modus")."

**Geltung im Bau (Entscheid des Baus, 25.11.2026).** Unter Linux gelten alle fünf Pflichten **in jedem Start**, nicht erst, wenn `AT_SECURE` gesetzt ist. Grund: Die Außentests müssen ohne root und ohne setgid laufen, und ein zweiter Weg ohne Schutz wäre fail open: Ein Fehler in der Erkennung von `AT_SECURE` schaltete sonst alle fünf Pflichten still ab. Unter macOS und Windows ist Betriebsart 2 nicht gebaut; dort ändert der Zug nichts am Start, und die Pflichten 3, 4 und 5 sind nicht gebaut. Pflicht 1 gilt überall, wo die Zielliste gelesen wird, und Pflicht 2 war schon gebaut.

**Geltung des Vertrags.** Er gilt, sobald der Modellweg unter Linux als setgid-Rollenprogramm läuft (Abschnitt 3), und seine Zusagen sind vor Schritt II Pflicht: Ab dem Systemnutzer des Halters ist die gid der einzige Unterschied zwischen dem Modellweg und jedem anderen Prozess der Person. In Schritt I bringt er noch keinen Gewinn, weil die Person die Ablage des Halters ohnehin liest (Halter-Vertrag, Abschnitt 2.3, „Grenze der Fassung 1").

Unter setgid gilt `AT_SECURE`. glibc entfernt dann `LD_*` und einige Variablen des Resolvers; alles andere bleibt stehen, und Argumente, Dateien und Umgebung kommen weiter vom Aufrufer. Rust und dieses Crate entfernen von sich aus nichts (hergeleitet aus dem Code, nicht gemessen). Die fünf Pflichten schließen das.

| Nr. | Pflicht | Code, Exit | Außentest |
|---|---|---|---|
| 1 | Der Halterweg ist an das Ziel gebunden: `halter_ziel` kommt nicht aus der Zielliste | 78, `mw.ziel_halter_name` | MW-L1 |
| 2 | Der Halter-Socket ist ein fester Pfad, kein Argument | 64, `mw.aufruf`; 78, `mw.halter_fehlt` | MW-L2 |
| 3 | Die Umgebung ist beim Start auf eine Positivliste geleert | 78, `mw.umgebung_rest` | MW-L3 |
| 4 | umask 077; keine Gruppenrechte auf Socket und Dateien | 64, `mw.aufruf` | MW-L4 |
| 5 | Keine Kinder | 70, `mw.kind_verboten`; 70, `mw.intern`, wenn die Dateien der Fäden nicht lesbar sind | MW-L5 |

1. **Zielliste und Halterziel (Pflicht 1).** Gewählte Form: **der Halterweg ist auf ein festes Ziel gebunden.** Der Modellweg nimmt den Namen des Halterziels nicht aus der Zielliste, sondern aus dem `host` des Netzziels (Abschnitt 12, Regel `halter_ziel` gleich `host`). Trägt die Zielliste ein `halter_ziel`, das nicht der `host` dieses Ziels ist, endet der Start mit Exit 78 `mw.ziel_halter_name`. Begründung der Wahl: Die andere Form, eine Zielliste aus einer Quelle, die der Aufrufer nicht schreibt, ist unter Linux nicht zu haben. Die Zielliste ist eine **Sitzungsdatei** von „Lauf starten" je Route (Abschnitt 12); sie gehört der Person, und root schreibt sie nicht. Die Bindung nimmt dem Aufrufer stattdessen die Wahl: Welches Geheimnis und welche Pfade zu einem Host gehören, steht nur in der root-eigenen `halter.json` (Halter-Vertrag, Abschnitt 2.3). Ein eigener Aufrufer mit eigener Zielliste kann damit kein Geheimnis an einen anderen Host lenken; er kann ein gebundenes Ziel nur ungebunden nennen, und dann geht kein Schlüssel mit. Die Pfadregeln prüft der Halter als letzte Sperre ohnehin selbst (H7).
   **Bauform (25.11.2026).** Stufe 12 der Zielliste meldet den Fall mit dem eigenen Code `zl.halter_name` (`src/betriebsart/zielliste.rs`, `HALTER_NAME`); ein weggefallenes Feld bleibt `zl.bindung`. Beide Fälle prüft die Stufe je Ziel in der Reihenfolge der Liste, und das erste Ziel entscheidet. Der Start antwortet auf `zl.halter_name` **immer** mit Exit 78 `mw.ziel_halter_name`, nicht nur unter setgid (`src/start.rs`, `vorbereiten_zwei`). Grund: ein Code für eine Sache; ein Code, der vom setgid-Bit abhinge, prüfte im Test eine andere Sache als im Betrieb. alt: `zl.bindung` und Exit 78 `mw.ziele_ungueltig` für beide Fälle, `mw.ziel_halter_name` nur unter setgid. Die Suche (Abschnitt 9a) antwortet auf `zl.halter_name` mit demselben Code, Exit 78 `mw.ziel_halter_name` (`src/suche.rs`, `halter_ziele`; Fall `zielliste_halter_name` in `faelle/suche.json`). alt (bis zum Nachtrag desselben Tages): „Die Suche bleibt bei `mw.ziele_ungueltig` für jede ungültige Liste" (OPEN 35, erledigt).
2. **Halter-Socket (Pflicht 2).** Der Pfad ist fest: `/run/user/<uid>/wb-halter/halter.sock` mit `<uid>` aus `getuid()`. `XDG_RUNTIME_DIR` liest der Modellweg unter setgid nicht, weil die Variable aus der Umgebung des Aufrufers kommt. Die Option `--halter-socket` gibt es unter setgid nicht; sie ist dort `mw.aufruf` (Exit 64, Abschnitt 13.5). Liegt am festen Pfad kein Socket des Halters, endet der Start mit Exit 78 `mw.halter_fehlt`; einen Rückfall auf eine Angabe des Aufrufers gibt es nicht. Der Halter nimmt denselben festen Pfad und liest `XDG_RUNTIME_DIR` nie (Halter-Vertrag, Abschnitt 2.3, „Socket“, `rust/spec/halter/README.md:309`); ein abweichendes `XDG_RUNTIME_DIR` der Person trennt beide darum nicht mehr (OPEN 31, erledigt). alt: „Folge, benannt: Weicht `XDG_RUNTIME_DIR` der Person von `/run/user/<uid>` ab, findet der Modellweg den Halter nicht (OPEN 31).“ **Bauform (bestätigt 25.11.2026):** Der Pfad entsteht nur in `modellweg::halter::fester_pfad`; der Aufruf kennt keine Option `--halter-socket` (eine unbekannte Option, `mw.aufruf`), und der Modellweg liest keine Umgebungsvariable für den Pfad. Seit Pflicht 3 ist die Umgebung beim Lesen ohnehin leer.
3. **Umgebung (Pflicht 3).** Die Positivliste ist in Fassung 1 **leer**: Nach Stufe 0 (Abschnitt 4) leert der Modellweg seine Umgebung vollständig, bevor er eine Datei liest oder eine Verbindung öffnet. Stufe 0 bleibt davor und lehnt `SSL_CERT_FILE`, `SSL_CERT_DIR` und `OPENSSL_CONF` weiter mit Exit 78 `mw.umgebung_unsicher` ab; so wird ein Versuch sichtbar, statt still geleert zu werden. Steht danach noch ein Name, der nicht in der Positivliste steht, endet der Start mit Exit 78 `mw.umgebung_rest`. Jeder Name, der später in die Liste kommt, braucht eine Zeile in diesem Abschnitt und einen Fall. Grund für eine Positivliste statt einer Verbotsliste: OpenSSL und `getaddrinfo` lesen mehr Variablen, als eine Verbotsliste nennt (Abschnitt 6, `src/ziel.rs`), und unter `AT_SECURE` ignorieren sie nur einen Teil davon (hergeleitet).
   **Bauform (25.11.2026): das Programm führt sich selbst mit leerer Umgebung neu aus.** Nach Stufe 0 prüft der Modellweg, ob seine Umgebung leer ist. Ist sie es nicht, ersetzt er sich per `execve` durch `/proc/self/exe` mit denselben Argumenten und einer leeren Umgebung (`src/os.rs`, `neu_ohne_umgebung`). argv\[0\] des neuen Abbilds ist die Marke `wb-modellweg-geleert:<PID>` mit der ersten Ablesung von `getppid()` (oder `-` ohne Elternprozess); das neue Abbild nimmt diese Ablesung als seine eigene erste, damit die Regel 1 von Abschnitt 3 (Rennen um `getppid()`) über den Wechsel hinweg gilt. Ein Abbild mit Marke, dessen Umgebung nicht leer ist, endet mit Exit 78 `mw.umgebung_rest`; ebenso ein `execve`, das scheitert. Die Marke gibt einem Aufrufer, der sie selbst setzt, nichts: Mit Umgebung endet sein Start mit `mw.umgebung_rest`, ohne Umgebung ist nichts zu leeren, und eine falsche PID endet nur seinen eigenen Start mit `mw.eltern_weg`. Die Reihenfolge ist: erste Ablesung des Elternprozesses, Stufe 0, Pflicht 3, Pflicht 4, dann Schritt 1. Das `execve` liest keine Datei des Aufrufers; der Kern öffnet das laufende Programm über `/proc/self/exe`, also die Datei, die schon läuft, nicht einen Pfad, den jemand tauschen kann.
   Grund der Bauform: Im Crate gilt `unsafe_code = "forbid"`, und `std::env::remove_var` ist in Edition 2024 `unsafe`. Auch ein Leeren im Prozess ließe die Zeichenketten des Starts im Speicher stehen; `/proc/<pid>/environ` zeigt sie, solange das Abbild lebt. Erst ein neues Abbild bekommt vom Kern einen neuen, leeren Block. Das `execve` ist kein Kind: Die PID, der Elternprozess und die Sitzung bleiben (Pflicht 5). Unter setgid wirkt das setgid-Bit beim zweiten `execve` erneut, und `AT_SECURE` gilt im neuen Abbild wieder; `PR_SET_PDEATHSIG` setzt erst das neue Abbild (Schritt 1), weil der Kern es beim `execve` eines setgid-Programms löscht.
4. **Rechte (Pflicht 4).** Vor dem ersten Anlegen setzt der Modellweg `umask 077`. Jede Datei, die er anlegt, hat 0600, jeder Ordner 0700, der Socket genau 0600 (Abschnitt 4, Schritt 7). `--socket-rechte` ist genau `0600`; ein Wert mit einem Gruppen- oder Anderen-Bit ist ein Aufruffehler (Exit 64, `mw.aufruf`), auch `0660`. Grund: Unter setgid gehören neu angelegte Dateien der Gruppe `wb-bruecke`; mit Gruppenrechten stünden Socket und Protokoll jedem Prozess offen, der diese Gruppe trägt. **Bauform (25.11.2026):** Der Start setzt `umask 077` direkt nach Pflicht 3 und vor Schritt 1 (`src/os.rs`, `umask_sicher`); das Binden des Sockets setzt für seinen Moment weiter 0177 (Schritt 7) und stellt danach 077 wieder her. `--socket-rechte` nahm schon vorher nur genau `0600` an (`src/aufruf.rs`); geändert ist daran nichts.
5. **Keine Kinder (Pflicht 5).** Der Modellweg startet keinen Prozess. Im Produktcode gibt es kein `fork` und kein Kind; das einzige `execve` ist das Ersetzen durch sich selbst aus Pflicht 3, und es startet keinen Prozess. alt: „Im Produktcode gibt es kein `fork` und kein `exec` (gemessen: `Command::new` steht nur in einem Test, `rust/crates/modellweg/src/schluessel.rs:160`)." Die Zusage bleibt: Jedes Kind erbte die gid und damit die Rolle `bruecke`.
   **Bauform (25.11.2026): zwei Prüfungen, kein Filter.** (a) Im Bau: Der Einheitstest `no_product_code_starts_a_process` (`src/start.rs`) liest jede Quelldatei des Crates bis zu ihrem Testmodul und lässt genau eine Stelle zu, die einen Prozess startet oder ersetzt: `os::neu_ohne_umgebung`. `spawn(` darf nur für Fäden stehen. (b) Im Betrieb liest das Programm die Dateien `/proc/self/task/<tid>/children` aller Fäden, einmal vor der Bereitschaftszeile und einmal vor jeder angenommenen Verbindung (`src/start.rs`, `ohne_kind`, gerufen in `haupt` über `starten_mit` und in `dienen`). Nennt eine davon eine PID, endet der Prozess mit Exit 70 `mw.kind_verboten`: vor der Bereitschaft mit der Zeile auf stderr und ohne Socket, danach mit dem Ende aus Abschnitt 3 (Anfragen abreißen, Socket löschen), der Zeile auf stderr und Exit 70. Sind die Dateien nicht lesbar, endet er fail closed mit Exit 70 `mw.intern`. Ein Faden, der zwischen dem Lesen des Ordners und seiner Datei endet, zählt nicht als Fehler; seine Kinder gingen an einen anderen Faden desselben Prozesses. Der Einstieg `start::starten` der Bibliothek liest Pflicht 5 nicht, weil er in einem fremden Prozess läuft, etwa einem Test, dessen Kinder nicht die des Modellwegs sind.
   Grund der Wahl: `PR_SET_NO_NEW_PRIVS` verhindert kein Kind, und `RLIMIT_NPROC` zählt alle Prozesse der Person. Ein seccomp-Filter gegen `fork`, `vfork`, `clone` ohne `CLONE_THREAD`, `clone3` und `execve` verhinderte ein Kind, statt es zu bemerken; er bräuchte aber `seccompiler` als neue Abhängigkeit dieses Crates (eine Änderung an `Cargo.toml`, im Zug ohne Freigabe nicht erlaubt; das Paket steht schon in `Cargo.lock`, über `rust/crates/kernsperre`). Der Filter ist nicht gebaut; entschieden ist, dass er in einen eigenen Zug über `rust/crates/kernsperre` gehört und erst nach dem `execve` von Pflicht 3 greift (OPEN 36). **Grenze, benannt:** Die Prüfung im Betrieb erkennt ein Kind erst an der nächsten Verbindung und verhindert es nicht; ein Kind, das zwischen zwei Verbindungen entsteht und endet, sieht sie nicht.

**Außentests (MW-L1 bis MW-L5).** Jeder läuft unter Linux als ganzer Prozess gegen eine Attrappe des Halters, ohne root (gebaut in `rust/crates/modellweg/tests/sicherer_modus.rs`). Wo der Test den festen Pfad des Halters oder Dateien des laufenden Prozesses unter `/proc` braucht, läuft das Programm in einem eigenen Nutzer- und Mount-Namensraum (`unshare -Urm`), den der Test besitzt: Ein Bind über `/run/user`, nur dort sichtbar, setzt die Attrappe an den festen Pfad, ohne den echten Ordner zu berühren, und als Besitzer des Namensraums darf der Test die Dateien des nicht abbildbaren Prozesses lesen. Ohne `unshare` oder Nutzer-Namensräume sind die Tests rot, nicht übersprungen.
MW-L1 Zielliste mit einem `halter_ziel` ungleich dem `host`: Exit 78 `mw.ziel_halter_name`; Gegenprobe mit `halter_ziel` gleich dem `host`: Start läuft.
MW-L2 `--halter-socket <pfad>` im Aufruf: Exit 64 `mw.aufruf`; `XDG_RUNTIME_DIR` zeigt auf eine Attrappe und am festen Pfad liegt keine: Exit 78 `mw.halter_fehlt`; Attrappe am festen Pfad: Start läuft.
MW-L3 Start mit gesetzten Variablen außerhalb der Positivliste, in beiden Betriebsarten: `/proc/<pid>/environ` des laufenden Prozesses ist leer. Gegenprobe: Dieselbe Ablesung sieht die Umgebung eines einfachen Prozesses im selben Aufbau.
MW-L4 `--socket-rechte 0660` und jeder andere Wert außer `0600`: Exit 64 `mw.aufruf`; nach einem gelungenen Start hinter einer umask 000 und einer Anfrage hat der Prozess die umask 077 (`Umask:` in `/proc/<pid>/status`), und Socket, Protokollordner mit allen Einträgen und Zähldatei haben keine Rechte für Gruppe und Andere.
MW-L5 Nach der Bereitschaft und nach einer Anfrage ist `/proc/<pid>/task/<tid>/children` für jeden Faden leer, in beiden Betriebsarten. Gegenprobe: Dieselbe Ablesung sieht das Kind eines einfachen `sh`. Den Code `mw.kind_verboten` vor der Bereitschaft prüft `tests/aussen.rs` (`a_child_before_the_ready_line_ends_the_start`) über die Naht von `starten_mit`, weil kein Aufruf dem Programm ein Kind gibt.
Zusätzlich führt `faelle/umgebung.json` die Fälle der Marke (`argv0`, `erwartet_linux`), und `faelle/kopf-ziele.json` die Fälle von `zl.halter_name` mit `mw.ziel_halter_name`.

## 4. Aufruf

```
modellweg [--betriebsart <art>] --socket <pfad> --socket-rechte 0600 --sitzung <sitzung> --route <route> --protokoll <ordner>
          [--probelauf --marker <liste> --marker-zaehlung <datei>] <Optionen der Betriebsart>
```

**Optionen für jede Betriebsart:**

| Option | Herkunft | Form |
|---|---|---|
| `--betriebsart` | Startprogramm | frei; fehlt sie, gilt `openai-lokal` |
| `--socket` | Konfiguration des Startprogramms | absoluter Pfad ohne Segment `.` oder `..` und ohne Steuerzeichen (U+0000 bis U+001F, U+007F bis U+009F), höchstens 107 Byte (`sun_path`) |
| `--socket-rechte` | Startprogramm | genau `0600` |
| `--sitzung`, `--route` | geprüfte Startdatei (Vertrag 1) | Kennung `^[a-z0-9][a-z0-9._-]{0,63}$` |
| `--protokoll` | Konfiguration des Startprogramms | absoluter Pfad wie oben, höchstens 1024 Byte |
| `--probelauf` | Läufer eines Probelaufs | Schalter ohne Wert; nur zusammen mit `--marker` und `--marker-zaehlung` (Abschnitt 9) |
| `--marker`, `--marker-zaehlung` | Läufer eines Probelaufs | nur zusammen mit `--probelauf`: alle drei oder keiner (Abschnitt 9) |
| `--protokoll-pfad-voll` | Einstellung der Admins | Schalter ohne Wert, Vorgabe aus; nur mit ihr steht der ganze Pfad im Feld `pfad` der Zeile (Abschnitt 8) |

Die Optionen der Betriebsarten stehen in Abschnitt 10 (`--ziel`, `--modell`, `--schluessel-datei`) und Abschnitt 13.5 (`--zielliste`, `--ca-schluessel`, `--ca-zertifikat`). Den Endpunkt des Halters gibt es unter Linux nicht als Option (Abschnitt 5.1).

- Jede Option genau einmal, als zwei Argumente (`--name wert`); nur `--probelauf` und `--protokoll-pfad-voll` sind Schalter ohne Wert. `--name=wert`, eine unbekannte Option oder ein fehlender Wert ist ein Aufruffehler.
- **Prüffolge:**
  1. Alle Optionen lesen; eine unbekannte oder doppelte Option ergibt `mw.aufruf`.
  2. `--betriebsart` prüfen (`mw.betriebsart_unbekannt`).
  3. Die Probelauf-Optionen prüfen (`mw.probelauf_aufruf`).
  4. Vollständigkeit und Form nach der Betriebsart prüfen (`mw.aufruf`).

  So kann eine Betriebsart andere Pflichtoptionen haben, ohne dass sich die Prüfung der anderen ändert.
- **Ein Schlüssel steht nie in argv und nie in der Umgebung**, nur der Pfad seiner Datei. Ziele, Modell und Schlüsseldateien kommen nie aus dem Start und nie vom Agenten.

**Schritte vor der Bereitschaft** (der erste Fehler beendet den Prozess):

| Schritt | Prüfung | Exit, Code |
|---|---|---|
| 0 | Umgebung: keine der Variablen `SSL_CERT_FILE`, `SSL_CERT_DIR`, `OPENSSL_CONF` ist gesetzt, auch nicht leer (Abschnitt 3, F1); vor allem anderen | 78, `mw.umgebung_unsicher` |
| 0a | nur unter Linux: die Umgebung leeren (Abschnitt 3a, Pflicht 3; ein Ersetzen durch sich selbst, danach Stufe 0 erneut), dann `umask 077` (Pflicht 4) | 78, `mw.umgebung_rest` |
| 1 | Abschirmung `kernabbild::abschirmen` (Abschnitt 3); Handler für SIGTERM und SIGINT; Todessignal und danach erneut `getppid()` gegen die erste Ablesung (Abschnitt 3); Aufruf nach der Prüffolge oben | 5, `mw.kernabbild`; 70, `mw.intern` (Handler oder Todessignal abgelehnt); 69, `mw.eltern_weg`; 64, `mw.aufruf`, `mw.betriebsart_unbekannt` oder `mw.probelauf_aufruf` |
| 2 | Konfiguration der Betriebsart (Betriebsart 2: Zielliste, Zulassung der CA und CA, Abschnitt 13.5) | 78, `mw.ziele_ungueltig`, `mw.ziel_halter_name`, `mw.ca_nicht_zugelassen` oder `mw.ca_ungueltig` |
| 3 | Betriebsart 1: die Schlüsseldatei nach Abschnitt 5.2. Betriebsart 2: Verbindung zum Halter, Prüfung der Gegenseite und `schluessel.vorhanden` mit `ziel` für jedes `halter_ziel` (Abschnitte 5.1 und 13.5) | 78, `mw.schluessel_datei`, `mw.schluessel_form`, `mw.halter_fehlt`, `mw.halter_unecht`, `mw.halter_kein_schluessel`, `mw.halter_fehler` oder `mw.halter_frist` |
| 4 | Protokollordner ist ein Ordner | 78, `mw.protokoll_ordner` |
| 5 | nur im Probelauf: Zähldatei mit dem Anfangsstand schreiben, dann die Startzeile `probelauf.zaehlung` an die Kette hängen (Abschnitt 9) | 74, `mw.zaehlung_fehler` oder `mw.protokoll_fehlt` |
| 6 | jedes Ziel einmal auflösen (entfällt bei `127.0.0.1` und `[::1]`); die Adressen gelten für die ganze Sitzung | 69, `mw.ziel_unaufloesbar` |
| 7 | Socket anlegen; liegt am Pfad schon etwas, nichts ändern. Vier Teile in dieser Reihenfolge: **(a)** Den Ordner des Sockets so öffnen, dass **kein** Glied des Pfades über einen Verweis verfolgt wird — unter Linux ein `openat2` mit `RESOLVE_NO_SYMLINKS`, auf jedem anderen System jedes Glied einzeln mit `O_DIRECTORY` und `O_NOFOLLOW`, von `/` aus; ein Glied `..` wird vorab abgewiesen. **(b)** Prüfen, dass der geöffnete Ordner ein Ordner dieses Nutzers ist, in den kein anderer Nutzer schreiben darf (kein `w` für Gruppe und keines für Andere). **(c)** Mit umask 0177 relativ zu diesem Deskriptor binden, nicht erneut über den Pfad der Namen: unter Linux über `/proc/self/fd/<n>/<name>`, auf jedem anderen System mit `fchdir` auf den Deskriptor, `bind` auf den bloßen Namen und `fchdir` zurück. **(d)** Danach über den geöffneten Ordner Art (Socket), Inode und Modus prüfen: Art Socket, genau 0600, und genau diese Inode merken (ein Schritt). Scheitert (a), (b) oder (c), endet der Start mit 74 **vor** dem `bind`; es gibt keinen Rückfall, der öffnet oder bindet. Weicht in (d) etwas ab, den Socket nur über den Deskriptor des Ordners entfernen und nur, wenn der Name noch diese Inode zeigt, und 74. Die Rechte werden nicht per Pfad gesetzt. **Zusage.** (a) und (c) zusammen binden den Socket in genau den Ordner, den (a) geöffnet hat: Ein Verweis über dem Ordner wird nicht verfolgt, und ein Tausch eines Gliedes zwischen (a) und (c) ändert das Ziel des `bind` nicht. Nach (c) liegt der Eintrag immer in diesem Ordner, darum sieht (d) ihn und entfernt ihn im Fehlerfall über den Deskriptor; es bleibt kein Socket an einer Stelle liegen, die der Prozess nicht mehr erreicht. (b) schließt das Fenster zwischen (c) und (d): Wer den Eintrag unter dem Namen tauschen wollte, müsste in den Ordner schreiben dürfen. **Restlücken.** Der Aufrufer gibt einen aufgelösten Pfad: ein Verweis in irgendeinem Glied endet den Start mit 74, auch ein gutartiger (auf macOS sind `/tmp` und `/var` Verweise). Linux: ein Kern ohne `openat2` (vor 5.6), ein fehlendes `/proc` oder ein Name, der mit `/proc/self/fd/<n>/` nicht mehr in `sun_path` passt, endet den Start mit 74. macOS und jedes andere System ohne `openat2`: Für die Dauer von (c) ist das Arbeitsverzeichnis des **ganzen** Prozesses der Ordner des Sockets; in diesem Moment darf kein anderer Faden einen relativen Pfad benutzen (die umask von (c) ist in derselben Weise eine Einstellung des ganzen Prozesses). Zwei Bindungen laufen dort nie nebeneinander: Wechsel, `bind` und Rückweg sind ein Schritt unter einer Sperre des Prozesses, darum findet jeder Wartende das Arbeitsverzeichnis so vor, wie es war. Gelingt der Rückweg nicht, wird der eben gebundene Eintrag über den Deskriptor wieder entfernt und der Start endet mit 74. Beide Systeme: `root` darf trotz (b) in den Ordner schreiben, das Fenster zwischen (c) und (d) bleibt gegen `root` offen; dasselbe Fenster liegt zwischen dem `fstatat` und dem `unlinkat` jedes Löschens, weil POSIX kein Löschen über den Deskriptor eines Eintrags hat | 74, `mw.socket_belegt` |
| 7a | nur unter Linux: kein Faden des Prozesses hat ein Kind (Abschnitt 3a, Pflicht 5; dieselbe Prüfung vor jeder angenommenen Verbindung); scheitert sie, wird der Socket gelöscht | 70, `mw.kind_verboten` oder `mw.intern` |
| 8 | nur im Probelauf: genau die Zeile `mw.probelauf: Marker werden gezählt` und LF auf stderr | |
| 9 | genau die Zeile `{"bereit":true}` und LF auf stdout, danach nichts mehr | |
| jeder | unerwarteter Fehler | 70, `mw.intern` |

**stderr-Zeile.** Vor der Bereitschaft schreibt der Modellweg bei jedem Fehler genau eine Zeile `<code>: <grund>` und LF auf stderr. Sie ist ASCII, höchstens 200 Byte lang und enthält keinen Pfad, keinen Dateiinhalt und keinen Schlüssel. Ausnahme `mw.kernabbild`: Danach folgt die Zeile `mw.kernabbild_grund: <Grund des Crates>` (etwa `kernabbild.muster_pipe`), ebenso ASCII und ohne Pfad. Ausnahme `mw.halter_fehler` und `mw.halter_kein_schluessel` in Schritt 3, wenn der Halter einen Code geantwortet hat: Danach folgt die Zeile `mw.halter_grund: <Code des Halters>` (etwa `halter.rolle`), nur in der Form `^halter\.[a-z_]{1,40}$` und sonst gar nicht; Exitcode und erste Zeile bleiben gleich (Abschnitt 5.1 und 13.5, Schritt 3).
Bei Erfolg schreibt er nichts auf stderr, außer im Probelauf die feste Zeile aus Schritt 8 und, wenn die geerbte harte Grenze für Kernabbilder 0 war und das Muster kein Pipe-Muster ist, die ASCII-Zeile `mw.kernabbild_hinweis: <Text des Crates>` aus Schritt 1. Die Zeile des Probelaufs ist die einzige Zeile mit einem Zeichen außerhalb von ASCII (UTF-8, Text vorgegeben). „Lauf starten“ wertet stderr für die Bereitschaft nicht aus (Vertrag 1); der Hinweis ändert sie nicht. „Lauf starten“ wartet höchstens 10 s auf die Bereitschaft.

| Code | Exit | Grund (fest, ASCII) |
|---|---|---|
| `mw.aufruf` | 64 | `Aufruf ungueltig` |
| `mw.betriebsart_unbekannt` | 64 | `Betriebsart unbekannt oder nicht gebaut` |
| `mw.probelauf_aufruf` | 64 | `Probelauf-Optionen ungueltig` |
| `mw.umgebung_unsicher` | 78 | `Umgebung unsicher` |
| `mw.umgebung_rest` | 78 | `Umgebung nicht geleert` |
| `mw.ziele_ungueltig` | 78 | `Zielliste ungueltig` |
| `mw.ziel_halter_name` | 78 | `Halterziel nicht der Host` |
| `mw.ca_nicht_zugelassen` | 78 | `Lokale CA nicht zugelassen` |
| `mw.ca_ungueltig` | 78 | `CA-Datei ungueltig` |
| `mw.schluessel_datei` | 78 | `Schluesseldatei nicht eigen, nicht 0600 oder keine Datei` |
| `mw.schluessel_form` | 78 | `Schluessel hat falsche Form` |
| `mw.halter_fehlt` | 78 | `Halter nicht erreichbar` |
| `mw.halter_unecht` | 78 | `Halter nicht echt` |
| `mw.halter_kein_schluessel` | 78 | `kein Schluessel beim Halter` |
| `mw.protokoll_ordner` | 78 | `Protokollordner fehlt oder ist kein Ordner` |
| `mw.zaehlung_fehler` | 74 | `Zaehldatei nicht schreibbar` |
| `mw.protokoll_fehlt` | 74 | `Protokoll nicht schreibbar` |
| `mw.ziel_unaufloesbar` | 69 | `Ziel nicht aufloesbar` |
| `mw.eltern_weg` | 69 | `Elternprozess beendet` |
| `mw.socket_belegt` | 74 | `Socket belegt oder nicht anlegbar` |
| `mw.intern` | 70 | `interner Fehler` |
| `mw.kind_verboten` | 70 | `Kindprozess verboten` |
| `mw.kernabbild` | 5 | `Kernabbild nicht abschirmbar` |

## 5. Schlüssel über den Halter und Platzhalter (V)

**Entscheid des Betreibers, 23.11.2026 (Vertragsbefund B05 „Halterweg vor Schritt I").** Der Token der Cloud-Route liegt nicht in einer Datei der Person. Er liegt beim Halter (`rust/spec/halter/README.md`), und der Modellweg kennt seinen Wert nie.
Grund: A93 verlangt den Token nur beim Halter. Bis 22.11.2026 las der Modellweg ihn selbst aus einer Datei 0600 der Person; damit trug ein Prozess der Person den Token, und jeder weitere Prozess derselben Kennung las die Datei (bisher OPEN 5).
Die Grenze der Fassung 1 bleibt benannt: Läuft der Halter unter der Kennung der Person, hält er den Wert nur im eigenen Speicher und schirmt sich gegen Abbild und `ptrace` ab (Halter-Vertrag, Abschnitt 2.3). Das ist eine stärkere Sperre als eine Datei 0600, aber keine Kontengrenze; die schließt erst ein eigener Systemnutzer (Halter-Vertrag, OPEN 9).

**Entschieden im Halter-Vertrag, 23.11.2026 (Nachtrag 22).** Der Halter-Vertrag hat die sieben Fragen aus Abschnitt 5.4 beantwortet. Weg A bleibt. Für Vertrag 8 gilt damit:

| Punkt | Entscheidung |
|---|---|
| Halter unter Linux | Ein **eigener Prozess der Person** mit `PR_SET_DUMPABLE` 0, nicht mehr derselbe Prozess wie der Modellweg. Der Gewinn ist benannt: Der Parser des Modellwegs, der Bytes des Agenten liest, hält den Token nicht mehr. Der Systemnutzer kommt erst in Schritt II. |
| Ablage unter Linux, Fassung 1 | eine Datei 0600 der Person in der Ablage des Halters. **Das bleibt die Grenze** (OPEN 19). |
| `halter.json` | root-eigen unter `/etc/werkbank/`, für die Person nicht schreibbar. Sie führt Adresse, Host, erlaubte Pfade, Kopfnamen und den Namen des Geheimnisses je Modellziel. |
| Art des Geheimnisses (H14) | Der Anmeldetoken der Cloud-Route ist ein Geheimnis **der Person**, Art `anmeldung.<ziel>` im Bereich `person`. `modell.aufruf` setzt es nur ein, wenn es dem Konto des Aufrufers gehört. Der Bereich `maschine` gilt für die Cloud-Route nicht mehr; er bleibt für den Schlüssel der Betriebsart 1 (Abschnitt 5.2). |
| Abfrageteil (H4) | ein eigenes Feld `abfrage`, nie ein `?` im Feld `pfad` (Abschnitt 5.1). |
| Umstellung (H5) | eine eigene Op `modell.strom` (Abschnitt 5.1). |
| Körper (H6) | keine Teile; für ein gebundenes Ziel senkt Vertrag 8 seine Grenze (Abschnitt 13.2, Stufe 7). |
| Pfadliste (H7) | `halter.json` führt sie, der Modellweg prüft zusätzlich; beide Listen entstehen aus `netzziele`, und ein Unterschied ist ein Befund von `werkbank-pruefen`. |
| Suche (H8) | eine Op `modell.suche`; E4 für die Cloud-Route kann damit grün werden (Abschnitt 9a). |
| Übernahme des Tokens | Für Schritt I schreibt `werkbank-anmelden` die Datei `anmeldung.copilot` mit `O_EXCL`, Modus 0600, `fsync` und Tausch direkt in die Ablage des Halters. Ab dem Systemnutzer geht der Weg über `schluessel.setzen` mit dem Schein der Stelle. Das gehört dem Paketvertrag und Vertrag 9, nicht diesem Vertrag. |
| Betriebsart 1 | nicht in diesem Stand (OPEN 17). |
| Lauf-Schein (A08) | gehört zu Vertrag 7; im Halter-Vertrag steht er als offener Punkt. |

### 5.1 Halterweg (Betriebsart 2, Pflicht ab Schritt I)

- **Rolle.** Der Modellweg ist die Rolle `bruecke` des Halters. Ihm sind genau `schluessel.vorhanden`, `modell.aufruf`, `modell.strom`, `strom.teil`, `strom.ende` und `modell.suche` erlaubt (Halter-Vertrag, Rollentabelle `rust/spec/halter/README.md:109`; die vier neuen Ops nach H5 und H8 in Abschnitt 5.4). Unter Windows läuft er dafür als Dienst mit eigener Dienst-SID (`NT SERVICE\wb-modellweg`, Baustein E2 des Windows-Bauplans), unter macOS als signiertes Rollenprogramm.
- **Endpunkt (Abgleich mit dem Halter-Vertrag, 25.11.2026; Festlegung des Baus, 25.11.2026).** Der Halter-Socket ist **genau ein Pfad**: `/run/user/<uid>/wb-halter/halter.sock` mit `<uid>` aus `getuid()`. `XDG_RUNTIME_DIR` liest der Modellweg nicht, in keiner Betriebsart und nicht nur unter setgid, weil die Variable aus der Umgebung des Aufrufers kommt (Abschnitt 3a, Pflicht 2). Der Pfad kommt nie aus der Startanfrage, nie aus einer Option und nie vom Agenten. alt (bis 25.11.2026): „`$XDG_RUNTIME_DIR/wb-halter/halter.sock`, ohne die Variable `/run/user/<uid>/wb-halter/halter.sock`"; alt (bis 22.11.2026): „Der Pfad des Halter-Endpunkts kommt als `--halter-socket <pfad>` aus der Konfiguration des Startprogramms." Grund: Zwei Quellen für einen Pfad wären eine Lücke, nicht eine Nachsicht. **Stand des Halter-Vertrags (nachgezogen 25.11.2026):** Der Halter nimmt unter Linux denselben festen Pfad `/run/user/<uid>/wb-halter/halter.sock` und liest `XDG_RUNTIME_DIR` nie; eine gesetzte Umgebung verlegt seinen Socket nicht (Halter-Vertrag, Abschnitt 2.3, „Socket“, `rust/spec/halter/README.md:309`). Beide Seiten nehmen damit genau einen Pfad aus derselben Quelle, `getuid()`, und OPEN 31 ist auf beiden Seiten erledigt. alt (bis 25.11.2026, berichtigter Stand): „Der Halter-Vertrag nennt heute noch `$XDG_RUNTIME_DIR/wb-halter/halter.sock` (Abschnitt 2.3, „Socket") und `/run/user` nirgends. Er zieht den festen Pfad im angekündigten Zug des Halters nach […]. Bis dahin liegt der Halter nur dann am festen Pfad des Modellwegs, wenn `XDG_RUNTIME_DIR` der Person gleich `/run/user/<uid>` ist (OPEN 31).“ alt (davor): „Grund: Der Halter nimmt denselben festen Pfad und liest `XDG_RUNTIME_DIR` ebenfalls nicht (Halter-Vertrag, Abschnitt 2.3) […]. Damit ist OPEN 31 erledigt."
  Im Bau steht der Pfad an genau einer Stelle (`modellweg::halter::fester_pfad`, `rust/crates/modellweg/src/halter.rs`), und nur der Start ruft sie (`src/start.rs`, Schritt 3). Die **einzige Ausnahme ist eine Naht, die allein in Tests wirkt**: Der Dienst der Betriebsart 2 und die Suche nehmen den Pfad beziehungsweise die Frage an den Halter als Wert entgegen, damit ein Test einen eigenen Halter an einem eigenen Pfad prüfen kann. Das Programm selbst übergibt dort immer den festen Pfad; es gibt keine Option, keine Umgebungsvariable und keinen Rückfall.
  Vor der ersten Zeile prüft der Modellweg die Gegenseite nach dem Halter-Vertrag, Abschnitt 2.3 (`mw.halter_unecht`): Ordner 0700 und Socket 0600 der eigenen uid, und die uid des Servers aus `SO_PEERCRED` gleich der eigenen.
  **Kein Hallo.** Das Vier-Byte-Hallo von `peerkanal` gibt es am Halter nicht; die erste Zeile ist sofort die Anfrage (Halter-Vertrag, Abschnitt 3).
  **Eine Anfrage je Verbindung zur Zeit.** Der Modellweg sendet auf einer Verbindung die nächste Anfrage erst nach der letzten Zeile der vorigen Antwort; mehrere Anfragen zugleich gehen über mehrere Verbindungen (mindestens 8, H12). Ein Strom (`modell.strom` nach der 101) hält seine Verbindung bis `ende`.
  **Festlegung des Baus (25.11.2026): eine Verbindung je Anfrage.** Der Bau öffnet je Anfrage an ein gebundenes Ziel eine eigene Verbindung zum Halter und schließt sie nach der letzten Zeile ihrer Antwort. Damit ist „eine Anfrage je Verbindung zur Zeit" ohne eine Buchführung erfüllt, die Frist von 300 s für eine Verbindung ohne Anfrage wird nie erreicht, und 32 offene Tunnel brauchen höchstens 32 Verbindungen, die der Halter zusagt (Halter-Vertrag, Abschnitt 3: mindestens 32 der Rolle `bruecke`). Der Vertrag lässt eine Wiederverwendung weiter zu; sie ist nur nicht gebaut.
  **Fristen am Halter.** Die erste Zeile erwartet der Halter binnen 10 s nach dem Verbinden, jede weitere binnen 300 s nach der letzten Antwort; sonst schließt er ohne Antwort. Das ist kein Ausfall (siehe unten).
- **Je Anfrage.** Der Modellweg prüft die Anfrage im Tunnel vollständig (Abschnitt 13.2, Stufen 1 bis 8) und gibt sie danach mit `modell.aufruf` an den Halter: `ziel` (der Wert von `halter_ziel`), `methode`, `pfad`, `abfrage`, `kopf` (die Köpfe nach Abschnitt 13.2, ohne jeden Anmeldekopf) und `koerper_b64`. Dazu trägt die Zeile `fassung` `1`, `op` und `id` wie jede Halterzeile (Halter-Vertrag, Abschnitt 4). Den Kopf mit dem Token setzt der Halter selbst ein, mit dem Namen aus seiner `halter.json`.
- **Form der Felder für ein gebundenes Ziel (Abgleich mit dem Halter-Vertrag, 23.11.).** Der Halter nimmt engere Werte an als der Tunnel. Der Modellweg prüft sie darum selbst, bevor er die Anfrage weitergibt; so entsteht für einen Formfehler nie erst beim Halter ein 502.
  - `methode` ist `GET` oder `POST` (`rust/spec/halter/README.md:372`). Eine andere Methode kann der Halter nicht tragen; an einem gebundenen Ziel erlaubt sie darum keine Regel, und die Antwort ist 405 `mw.methode_falsch` (Abschnitt 13.2, Stufe 4).
  - `pfad` hat die Form `^/[A-Za-z0-9._~/-]{0,511}$` (`halter:373`). Ein Pfad mit einem anderen Zeichen, auch mit Prozentkodierung, oder über 512 Zeichen bekommt an einem gebundenen Ziel 403 `mw.pfad_nicht_erlaubt` (Abschnitt 13.2, Stufe 3). Festlegung dieses Vertrags in der Richtung fail closed: Der Halter lehnte ihn ohnehin mit `halter.wert_form` ab.
  - `kopf` hat höchstens 64 Einträge, die Namen die Form `^[a-z0-9-]{1,64}$` und die Werte druckbares ASCII ohne CR und LF mit höchstens 8192 Zeichen (`halter:375`). Der Modellweg schreibt die Namen für ein gebundenes Ziel darum klein. Mehr Einträge, ein längerer Wert oder ein Name außerhalb der Form ergeben 400 `mw.anfrage_unlesbar`.
    **Zwei Folgen der Form, benannt (Befund des Baus, 25.11.2026).** alt: „die Reihenfolge bleibt". Das Feld `kopf` ist
    ein JSON-Objekt, und die Zeile ist kanonisch nach RFC 8785 (Halter-Vertrag, Abschnitt 3): die Namen stehen darin
    **sortiert**, nicht in der Reihenfolge des Harness. Eine Reihenfolge ist auf dem Halterweg darum nicht zusagbar;
    sie bleibt nur auf dem direkten Weg zu einem ungebundenen Ziel (Abschnitt 13.2, Punkt 1). Zweite Folge: Ein Objekt
    trägt einen Namen genau einmal. Sendet der Harness denselben Kopfnamen zweimal, geht an ein gebundenes Ziel nur
    der **letzte** Wert. Beides liegt an der Form des Feldes, nicht am Bau. **Entschieden (Inhaber des Halter-Vertrags, 25.11.2026):**
    Das Feld bleibt ein Objekt. Die Reihenfolge der Köpfe geht über den Halter nicht, und ein doppelter Kopfname ist
    dort nicht darstellbar. Beide Sätze sind damit Zusagen dieses Vertrags, keine offene Frage mehr (Abschnitt 20,
    F2).
  - Die verbotenen Kopfnamen des Halters (Halter-Vertrag, Abschnitt 4, **31 Namen**) entfernt der Modellweg auf dem Halterweg **vollständig**, nicht nur `authorization`, `accept-encoding` und `sec-websocket-extensions`; Abschnitt 13.2, Punkt 1 nennt alle 31 Namen. Sonst antwortete der Halter `halter.kopf_verboten`. `host`, `content-length` und die Rahmung setzt hier der Halter; der Modellweg sendet sie nie mit. Auf dem **direkten** Weg zu einem gebundenen Ziel (vor Schritt I, Abschnitt 5.2) bleiben genau diese drei, weil die Anfrage dort sie braucht (Entscheid des Betreibers, 25.11.2026; Abschnitt 13.2, Punkt 1). alt (bis 25.11.2026): 20 Namen, ohne die elf Köpfe, mit denen ein Proxy vor dem Ziel Methode oder Pfad ersetzen kann; und keine Unterscheidung der zwei Wege.
  - `koerper_b64` hat höchstens 32156332 Zeichen, also einen Rohkörper von höchstens 24117248 Byte (`halter:376`, H6).
  - **Der Körper geht ohne seine Rahmung (Festlegung des Baus, 25.11.2026).** `koerper_b64` trägt die Daten des Körpers, nie eine Rahmung: Kam der Körper des Harness mit `Transfer-Encoding: chunked`, nimmt der Modellweg die Rahmung ab und gibt nur die Daten weiter; die Rahmung nach außen setzt der Halter. Der Kopf `transfer-encoding` fällt dabei ohnehin (einer der 31 Namen). Die Grenze von 24117248 Byte gilt für die **Daten**. Für ein ungebundenes Ziel bleibt es wie bisher: Die Rahmung geht Byte für Byte hinaus (Abschnitt 13.2).
- **Pfad und Abfrage getrennt (H4, entschieden 23.11.).** Der Modellweg trennt das Anfrageziel selbst am ersten `?`. Vor dem `?` steht `pfad`, dahinter `abfrage`; ein `?` steht nie im Feld `pfad`. `abfrage` ist Text oder `null`; die Form ist `^[A-Za-z0-9._~!$&'()*+,;=:@/?%-]{1,1024}$`, und ein `#` kommt darin nie vor. Ein Anfrageziel mit `#` oder eine Abfrage über 1024 Zeichen ist `mw.anfrage_unlesbar` (400, Abschnitt 13.2, Stufe 1). Der Abfrageteil ist **nicht gemessen**: Alle gemessenen Pfade der Copilot CLI sind ohne `?` (OPEN 22).
- **Das Geheimnis gehört der Person (H14, entschieden 23.11.).** Der Token liegt beim Halter als Art `anmeldung.<ziel>` im Bereich `person`. Der Halter sucht es nach Name und Konto des Aufrufers, ohne Pseudonym, und setzt es nur ein, wenn es diesem Konto gehört (`rust/spec/halter/README.md:119` bis `:121`, Stufen 16 und 17). Der Modellweg nennt den Namen des Geheimnisses nicht; er steht je Modellziel in `halter.json`. **Eine Brücke als Vertreter gibt es in Fassung 1 nicht** (`halter:121`, Halter-Vertrag OPEN 19): Unter Linux und macOS trägt der Modellweg dieselbe Kennung wie der Halter, und damit gehört ihm das Geheimnis; unter Windows fährt der Halterweg der Cloud-Route darum erst in Schritt II (OPEN 23).
- **Umstellung auf WebSocket (H5, entschieden 23.11.).** Für eine Upgrade-Anfrage ruft der Modellweg die eigene Op `modell.strom` mit den Feldern `ziel`, `pfad`, `abfrage` und `kopf`; es gibt kein Feld `methode` (die Methode ist immer `GET`) und keinen Körper. `kopf` darf dort `sec-websocket-key`, `sec-websocket-version` und `sec-websocket-protocol` tragen (`rust/spec/halter/README.md:419`). `upgrade`, `connection`, `accept-encoding: identity` und den Kopf mit dem Token setzt der Halter selbst.
  Nach der 101 gehört die Verbindung nur dem Strom. **Die beiden Richtungen haben verschiedene Namen** (`halter:430` bis `:455`): Der Modellweg sendet die Bytes des Harness als Anfragen mit `op` `strom.teil` und `daten_b64`, mit der `id` des Stroms, je höchstens 65536 Byte; der Halter sendet die Bytes des Ziels als Zeilen der `art` `teil`. Beendet wird der Strom mit der Anfrage `op` `strom.ende` derselben `id`, oder das Ziel schließt; danach sendet der Halter `ende`. Solange der Strom offen ist, beantwortet der Halter jede andere Op auf dieser Verbindung mit `halter.strom`; der Modellweg nutzt dafür eine seiner anderen Verbindungen (mindestens 8, H12).
  **Ende, wenn der Weg des Harness scheitert (Projektentscheid, 25.11.2026).** Scheitert der Weg des Harness, solange der Strom offen ist, sendet der Modellweg dem Halter `strom.ende` mit der `id` des Stroms, bevor er seine Verbindung zum Halter schließt. Gescheitert ist der Weg bei einem Lesefehler, der kein Fristablauf ist, bei einem Schreibfehler zum Harness, auch wenn noch Zeilen `teil` des Halters warten, und beim Abriss des Dienstes. Der Fristablauf einer Runde ist kein Scheitern. Ein Abriss, der ein wartendes Lesen beendet, gilt nie als Ende des Harness: Der Modellweg prüft den Abriss vor und nach jedem Lesen. Nach diesem `strom.ende` wartet der Modellweg nicht auf `ende` des Halters, denn den Harness erreicht nichts mehr. Fail closed bleibt: Der Strom endet in jedem Fall. Die Zeile hat dann das Ergebnis `weitergereicht` mit `status` 101; geht `strom.ende` nicht mehr hinaus, gilt der Code dieses Fehlers nach der Tabelle unten. Grund: Der Halter soll das Ende als Ende sehen, nicht als Abriss. Der Halter-Vertrag trägt das: Die Brücke beendet den Strom mit `strom.ende`, und schließt sie danach die Verbindung, schließt er zum Ziel (`rust/spec/halter/README.md:711` bis `:716`). Schließt der Harness seinen Weg ohne Fehler, gilt weiter der Satz davor: `strom.ende`, danach gehen die Zeilen `teil` des Halters bis zu seinem `ende` an den Harness. Bauform: `rust/crates/modellweg/src/tunnel.rs`, `bruecke_halter` und `weg_gescheitert`; die Prüfung des Abrisses in `rust/crates/modellweg/src/betrieb.rs`, `Huelle::read`.
  **Eine Upgrade-Anfrage hat keinen Körper (Festlegung des Baus, 25.11.2026).** `modell.strom` kennt kein Feld für
  einen Körper. Trägt eine durchgereichte Upgrade-Anfrage dennoch einen Körper, geht er nicht mit; der Modellweg
  sendet nur Pfad, Abfrage und Köpfe. Eine Handschlag-Anfrage nach RFC 6455 hat keinen Körper, darum ist das kein
  gemessener Fall; ein Geheimnis ist nicht betroffen, weil der Modellweg keines hält (OPEN 33).
  Die 101 des Halters trägt `upgrade` und `connection` **nicht** (sie stehen auf seiner Sperrliste); diese beiden Köpfe setzt der Modellweg zum Harness selbst (`halter:428`, Abschnitt 13.3).
  Den Kopf `sec-websocket-extensions` entfernt der Halter; eine 101 mit Erweiterung endet bei ihm mit `halter.ziel_komprimiert`, und der Modellweg antwortet dem Harness 502 `mw.antwort_komprimiert` (Abschnitt 13.3). Die Leck-Sperre des Halters läuft über die Rahmengrenzen hinweg. Der Bau braucht **beide** Ops: Im gemessenen Lauf kamen `GET /responses` mit 101 und `POST /chat/completions` vor.
- **Der Weg nach außen gehört dem Halter.** Für ein gebundenes Ziel öffnet der Modellweg keine eigene Verbindung, löst keinen Namen auf und beendet kein TLS nach außen. Adresse, Host, erlaubte Pfade und der Name des Tokens stehen nur in `halter.json`; die Anfrage nennt keine Adresse. Unter Linux liegt sie root-eigen mit 0644 unter `/etc/werkbank/halter.json` (`rust/spec/halter/README.md:240`), unter Windows unter `%ProgramData%\agent-workbench\halter\halter.json` mit Schreibrecht nur für Administratoren (`halter:171`). Der Ort gehört in beiden Fällen dem Halter, nicht diesem Vertrag.
  Die Fristen nach außen gehören dem Halter (H12, entschieden 23.11.): Verbindung und TLS binnen 10 s, der Kopf der Antwort binnen 600 s, und im Strom gilt keine Frist von 300 s. Der Halter hält mindestens 8 Verbindungen der Rolle `bruecke`. Der Modellweg wartet weiter höchstens 600 s auf den Kopf (`mw.halter_frist`).
  **Eine Verbindung ohne Anfrage schließt der Halter nach 300 s** (`halter:330`); die erste Zeile erwartet er binnen 10 s nach dem Verbinden. Das ist kein Ausfall: Der Modellweg verbindet sich dann neu und antwortet nicht `mw.halter_fehlt`. Nur wenn das Verbinden oder die Prüfung der Gegenseite scheitert, gilt der Halter als ausgefallen.
- **Die Pfadregeln gelten doppelt (H7, entschieden 23.11.).** `halter.json` führt die Pfadliste je Ziel, und der Modellweg prüft zusätzlich (Abschnitt 13.2, Stufen 3 und 4). Beide Listen entstehen aus `netzziele` derselben Route; ein Unterschied ist ein Befund von `werkbank-pruefen`, kein stiller Fehler. Der Halter prüft dieselben Regeln wie Vertrag 8: kein `//`, kein `\`, kein Segment `.` oder `..`, kein `%2e`, `%2f` oder `%5c`. Lehnt er ab, antwortet er `halter.pfad_verboten`, und der Modellweg gibt 502 `mw.halter_fehler` an den Harness. Für ein **ungebundenes** Ziel bleibt alles wie bisher: Der Modellweg verbindet sich selbst (Abschnitt 6), und es geht nie ein Schlüssel mit.
- **Antwort.** Der Halter antwortet als Strom: eine Zeile `beginn` (`status`, `kopf`), null oder mehr Zeilen `teil` (`daten_b64`, je höchstens 65536 Byte Nutzdaten), zum Schluss genau eine Zeile `ende`. Daraus setzt der Modellweg die Antwort an den Harness zusammen (Abschnitt 13.3). **Sie geht immer `chunked` hinaus (Festlegung des Baus, 25.11.2026):** `content-length` und `transfer-encoding` stehen auf der Sperrliste des Halters, darum kennt der Modellweg die Länge des Körpers nicht und kann sie nicht nennen. Die Regel „Hatte der Dienst eine `Content-Length`, nennt der Weiterreicher dieselbe" (Abschnitt 13.3) gilt damit nur noch für ein ungebundenes Ziel und für Betriebsart 1.
  **Ein Fehler nach der Zeile `beginn` (Festlegung des Baus, 25.11.2026).** Meldet der Halter einen Code, bevor ein Byte des Körpers beim Harness steht, bekommt der Harness die eigene Antwort des Modellwegs (Status aus der Tabelle unten). Steht schon ein Byte beim Harness, endet die Antwort ohne ihren Abschluss; die Protokollzeile trägt dann den Status, der hinausging, und als `grund` den eigenen Code. Das ist dieselbe Ordnung wie bei einem späten Leck im Antwortstrom (Abschnitt 13.3). Vom Kopf des Ziels reicht der Halter für ein Modellziel alles weiter außer seiner eigenen Sperrliste (H10, entschieden 23.11.). **Diese Liste steht fest im Halter-Vertrag, nicht je Ziel in `halter.json`, und sie ist um fünf Namen länger als die in Abschnitt 13.3** (`rust/spec/halter/README.md:401` bis `:409`): dazu `authorization`, `proxy-authorization`, `x-api-key`, `api-key` und `cookie`. Sie gilt für jedes Modellziel ohne Schalter. Der Kopf der Antwort hat beim Halter höchstens 64 Einträge mit je höchstens 8192 Zeichen; ein Ziel mit mehr oder längeren Köpfen endet bei ihm mit `halter.ziel_fehler`, und der Modellweg antwortet 502 `mw.halter_fehler`. `accept-encoding: identity` hängt der Halter selbst an, und eine Antwort mit einem anderen `content-encoding` endet bei ihm mit `halter.ziel_komprimiert` (H11, entschieden 23.11.); der Modellweg antwortet dem Harness dann 502 `mw.antwort_komprimiert`.
- **Leck-Sperre.** Für ein gebundenes Ziel liegt die Leck-Sperre beim Halter (`halter.leck_gesperrt`, Halter-Vertrag, Abschnitt 8). Das ist Baustein B1 aus E2: Schlüssel und Leck-Sperre liegen nur beim Dienstteil oder beim Halter. Der Modellweg hat für dieses Ziel keinen Wert, gegen den er suchen könnte, und tut nicht, als hätte er einen. Für ein ungebundenes Ziel gibt es weiter keinen Schlüssel und keine Sperre.
- **Kein Rückweg.** Kein Schalter, keine Option und kein Fehlerfall gibt dem Modellweg einen Tokenwert. Erreicht er den Halter beim Start nicht, wird er nicht bereit. Fällt der Halter während der Sitzung aus, endet jede Anfrage an ein gebundenes Ziel mit einem eigenen Fehler. Einen Rückfall auf eine Schlüsseldatei gibt es nicht.

**Fehler des Halterwegs.** Vor der Bereitschaft Exit 78 (Abschnitt 4, Schritt 3); im Betrieb eine eigene Antwort nach Abschnitt 7.

| Code | Exit oder Status | Grund (fest, ASCII) | wann |
|---|---|---|---|
| `mw.halter_fehlt` | 78 | `Halter nicht erreichbar` | Start: der Endpunkt antwortet nicht |
| `mw.halter_unecht` | 78 | `Halter nicht echt` | Start: die Prüfung der Gegenseite scheitert |
| `mw.halter_kein_schluessel` | 78 / 503 | `kein Schluessel beim Halter` | Start: `schluessel.vorhanden` ist `false`, oder der Halter meldet `halter.ziel_unbekannt` oder `halter.kein_schluessel` (Abschnitt 13.5, Schritt 3); im Betrieb: der Halter meldet `halter.kein_schluessel` |
| `mw.halter_fehler` | 78 / 502 | `Halter lehnte ab` | Start: jeder andere Code des Halters auf `schluessel.vorhanden`, eine nicht lesbare Zeile, eine Antwort ohne `vorhanden` oder ein Abriss der Verbindung (Abschnitt 13.5, Schritt 3); im Betrieb: jeder andere Code des Halters, eine nicht lesbare Zeile oder ein Abriss der Verbindung |
| `mw.halter_frist` | 78 / 504 | `Halter antwortete nicht` | Start: keine Antwort auf `schluessel.vorhanden` binnen 5 s für alle gebundenen Ziele zusammen (Abschnitt 13.5, Schritt 3); im Betrieb: keine Zeile `beginn` binnen 600 s |
| `mw.antwort_komprimiert` | 502 | `Antwort komprimiert` | im Betrieb: der Halter meldet `halter.ziel_komprimiert` (komprimierte Antwort oder 101 mit Erweiterung) |

Jeder andere Code des Halters wird `mw.halter_fehler` (502), darunter `halter.pfad_verboten`, `halter.kopf_verboten`, `halter.ziel_unbekannt`, `halter.rolle`, `halter.peer`, `halter.person`, `halter.strom`, `halter.wert_form`, `halter.zu_gross`, `halter.nicht_kanonisch`, `halter.ziel_fehler`, `halter.leck_gesperrt` und `halter.intern` (Codetabelle `rust/spec/halter/README.md:565` bis `:599`).
Beim Start gilt dieselbe Abbildung mit einer Ausnahme: `halter.ziel_unbekannt` auf `schluessel.vorhanden` wird `mw.halter_kein_schluessel`, weil der Halter für dieses Ziel kein Geheimnis einsetzen kann (Abschnitt 13.5, Schritt 3). `halter.ziel_komprimiert` kann auf `schluessel.vorhanden` nicht vorkommen; käme es, wäre es beim Start `mw.halter_fehler`, weil `mw.antwort_komprimiert` ein Code des Betriebs ist.
Der Code des Halters geht **nie** an den Harness: Der Modellweg antwortet mit seinem eigenen Code und schreibt den Code des Halters nur in seine Protokollzeile (Abschnitt 13.4, Feld `grund`).
**Der Code des Halters beim Start (Festlegung des Baus, 25.11.2026).** Endet Schritt 3 mit `mw.halter_fehler` oder `mw.halter_kein_schluessel`, weil der Halter einen Code geantwortet hat, schreibt der Modellweg nach der Zeile `<code>: <grund>` genau eine zweite Zeile `mw.halter_grund: <Code des Halters>` und LF auf stderr, wie bei `mw.kernabbild_grund`. Der Code kommt nur in die Zeile, wenn er die Form `^halter\.[a-z_]{1,40}$` hat. Bei einer Antwort `{"vorhanden": false}`, einer Antwort ohne `vorhanden`, einer nicht lesbaren Zeile, einem Abriss, der Frist und einem Code anderer Form gibt es keine zweite Zeile; nie steht ein anderer Teil der Antwort (Grund, Text, Ziel, Pfad) in ihr. Exitcode und erste Zeile bleiben byte-gleich. Die Zeile steht auf stderr des Modellwegs, nicht beim Harness. Grund: Der Halter schreibt diese Ablehnung nicht ins Journal; ohne den Code ist die Ursache (etwa `halter.rolle`) nicht zu finden.

### 5.2 Schlüsseldatei (nur Betriebsart 1, Fassung 1)

Betriebsart 1 arbeitet gegen ein OpenAI-kompatibles Modell-Gateway mit einem Schlüssel je Rechner (Bereich `maschine`). Sie behält in Fassung 1 die Datei; der Halterweg gilt dort als Folgeschritt mit derselben Op und denselben Codes (Abschnitt 19, OPEN 17).

- **Schlüsseldatei:** mit `O_NOFOLLOW` geöffnet, reguläre Datei, Eigentümer ist die eigene Kennung, Modus genau 0600 (sonst `mw.schluessel_datei`).
  Inhalt höchstens 4097 Byte, genau eine Zeile `^[\x21-\x7e]{1,4096}$` mit oder ohne abschließendes LF (sonst `mw.schluessel_form`).
  Die Datei liegt außerhalb des Einschlusses in einem eigenen Ordner 0700; ihren Pfad nennt die Konfiguration des Startprogramms.
- **Lesen:** Der Modellweg liest den Schlüssel einmal beim Start; ein Wechsel gilt mit der nächsten Sitzung.
- **Der Schlüssel erscheint nie** in einer Antwort an den Harness, in einem Kopf, einem Fehlerkörper, einer Protokollzeile, der Zähldatei, auf stderr, in argv oder in der Umgebung. Die Leck-Sperre des Modellwegs (Abschnitt 7) gilt für diesen Schlüssel.

### 5.3 Platzhalter (jede Betriebsart)

- Im Einschluss steht höchstens ein Platzhalter, etwa in der Anbieter-Datei des Harness oder als `COPILOT_GITHUB_TOKEN`. Jeden Anmeldekopf des Harness (in Betriebsart 2 nur `Authorization`, Abschnitt 13.2) verwirft der Modellweg; den Kopf mit dem Schlüssel setzt in Betriebsart 1 er selbst, in Betriebsart 2 der Halter.
- Ein Platzhalter oder ein anderer Anmeldewert aus dem Einschluss geht nie hinaus. Der Modellweg verlangt den Platzhalter nicht und vergleicht ihn nicht.

### 5.4 Was der Halter-Vertrag liefert (entschieden im Halter-Vertrag, 23.11.2026)

Der Halter-Vertrag wird getrennt von diesem Vertrag geschrieben. Dieser Abschnitt nennt, was Vertrag 8 von ihm annimmt. Bis zum 23.11.2026 war jede Zeile ein Vorschlag; seit der Antwort des Halter-Vertrags ist jede Zeile entschieden. Der Halter-Vertrag ist seit dem Merge nach `development` im Baum; dieser Abschnitt ist gegen ihn abgeglichen (Abgleich vom 23.11.2026). Die Namen sind auf beiden Seiten dieselben: die Ops `modell.aufruf`, `modell.strom`, `strom.teil`, `strom.ende` und `modell.suche`, die Felder `abfrage`, `pfad`, `kopf`, `koerper_b64` und `daten_b64`, die Art `anmeldung.<ziel>` im Bereich `person` und die Codes `halter.pfad_verboten`, `halter.ziel_komprimiert`, `halter.kein_schluessel` und `halter.strom`.

| Nr. | Was | Entscheidung im Halter-Vertrag |
|---|---|---|
| H1 | Rolle `bruecke` mit `schluessel.vorhanden` und `modell.aufruf` | bleibt; dazu kommen `modell.strom`, `strom.teil`, `strom.ende` und `modell.suche` (`rust/spec/halter/README.md:109`) |
| H2 | Unter Linux ein **eigener Halter-Prozess** | ja. Der Satz „Halter und Modellweg sind ein Prozess" fällt. Der Halter ist ein eigener Prozess der Person mit `PR_SET_DUMPABLE` 0; der Systemnutzer kommt erst in Schritt II. |
| H3 | Rolle `bruecke` unter Linux und macOS | Der Modellweg ist Rollenprogramm: unter Linux root-eigen unter `/usr` (im Paket `/usr/lib/werkbank/werkbank-modellweg`), unter macOS signiert, unter Windows ein Dienst mit eigener Dienst-SID (Abschnitt 17a). `halter.json` liegt root-eigen unter `/etc/werkbank/`. |
| H4 | `modell.aufruf` mit **Abfrageteil** | ein **eigenes Feld** `abfrage`, nie ein `?` im Feld `pfad`. Text oder `null`, Form `^[A-Za-z0-9._~!$&'()*+,;=:@/?%-]{1,1024}$`, nie `#`. Der Modellweg trennt selbst und gibt beides getrennt weiter (Abschnitt 5.1). Gemessen ist der Abfrageteil nicht (OPEN 22). |
| H5 | **Upgrade (WebSocket)** | eine eigene Op `modell.strom`. Nach der 101 gehört die Verbindung nur dem Strom, die Bytes gehen als `teil` mit `daten_b64`. Der Halter entfernt `sec-websocket-extensions`; eine 101 mit Erweiterung endet mit `halter.ziel_komprimiert`. Die Leck-Sperre läuft beim Halter über die Rahmengrenzen. Der Bau braucht beide Ops. |
| H6 | **Körper über die Zeilengrenze** | keine Teile. Für ein **gebundenes** Ziel senkt Vertrag 8 die Grenze des Rohkörpers auf 24117248 Byte (23 MiB); darüber 413 `mw.koerper_zu_gross` (Abschnitt 13.2, Stufe 7). Für ein ungebundenes Ziel bleibt die Grenze 33554432 Byte. |
| H7 | **Pfadliste je Ziel** | `halter.json` führt sie. Der Modellweg prüft zusätzlich, beide Listen entstehen aus `netzziele`, und ein Unterschied ist ein Befund von `werkbank-pruefen`. Der Halter prüft dieselben Pfadregeln wie Vertrag 8 und antwortet sonst `halter.pfad_verboten`. |
| H8 | **Suche nach dem Schlüssel** (A93-Prüfung E4) | eine Op `modell.suche` mit `ziel` und `pfade`: 1 bis 16 absolute Pfade, je 1 bis 4096 Zeichen, Form `^/[^\x00-\x1f]*$`, kein Segment `.` oder `..`, nur reguläre Dateien, `O_NOFOLLOW`; die Antwort `suche` enthält nur die vier Zahlen `dateien`, `nicht_lesbar`, `treffer` und `uebersprungen` (Halter-Vertrag, Abschnitt `modell.suche`, Antwort; alt: „`rust/spec/halter/README.md:461` bis `:469`", ein veralteter Zeilenverweis), dieselben wie in Abschnitt 9a, S5. Abschnitt 9a nutzt sie für gebundene Ziele; E4 für die Cloud-Route kann damit grün werden. |
| H9 | **Kopfnamen** | Die Grenzen reichen. `content-type` und `accept` verbietet der Halter nicht. |
| H10 | **Kopf der Antwort** | für Modellziele eine Sperrliste, keine Positivliste. Sie steht **fest im Halter-Vertrag** (`rust/spec/halter/README.md:401` bis `:409`), nicht je Ziel in `halter.json`, und trägt fünf Namen mehr als die in Abschnitt 13.3: `authorization`, `proxy-authorization`, `x-api-key`, `api-key` und `cookie`. Höchstens 64 Köpfe, je höchstens 8192 Zeichen; darüber `halter.ziel_fehler`. |
| H11 | **`accept-encoding: identity`** | Der Halter hängt es an und verwirft eine Antwort mit einem anderen `content-encoding` mit `halter.ziel_komprimiert`. |
| H12 | **Frist und Gleichlauf** | Die Fristen nach außen gehören dem Halter: Verbindung und TLS 10 s, Kopf 600 s, im Strom keine Frist von 300 s. Mindestens 8 Verbindungen der Rolle `bruecke`. Vertrag 8 wartet weiter höchstens 600 s auf den Kopf. |
| H13 | **Zeile des Halters** | nichts; `formate` (Vertrag 5) kann RFC 8785 in beide Richtungen. |
| H14 | **Art des Geheimnisses** (neu im Halter-Vertrag) | Der Anmeldetoken der Cloud-Route ist ein Geheimnis der **Person**, Art `anmeldung.<ziel>` im Bereich `person`. `halter.json` nennt je Modellziel entweder ein `modell.*` oder ein `anmeldung.*` (`rust/spec/halter/README.md:60` bis `:64`, `:348`). `modell.aufruf` setzt es nur ein, wenn es dem Konto des Aufrufers gehört; gesucht wird nach Name und Konto, ohne Pseudonym. |

**Folge von H14 für Windows: vom Halter-Vertrag entschieden, Schritt II.** Unter Windows ist der Aufrufer des Halters ein Dienst unter eigenem Dienstkonto (Abschnitt 17a.2), und die Rollentabelle des Halters erlaubt der Rolle `bruecke` dort nur Geheimnisse im Bereich `maschine` (`rust/spec/halter/README.md:201`). Ein Geheimnis der Person gehört nicht dem Konto dieses Dienstes. Der Halter-Vertrag entscheidet das jetzt: **Eine Brücke als Vertreter gibt es in Fassung 1 nicht** (`halter:121`); die Vertreter-Regel und der Lauf-Schein gehören Schritt II (Halter-Vertrag, OPEN 19, `halter:797` bis `:799`). Unter Windows fährt der Halterweg der Cloud-Route darum in Fassung 1 nicht: Schon beim Start antwortet der Halter auf `schluessel.vorhanden` für ein `anmeldung.*` mit `false`, und Schritt 3 endet mit Exit 78 `mw.halter_kein_schluessel`. Unter Linux und macOS trägt der Modellweg dieselbe Kennung wie der Halter; dort gehört ihm das Geheimnis, und der Weg fährt. Dieselbe OPEN 19 des Halter-Vertrags nennt eine zweite Folge: 16 Pipe-Instanzen reichen für 8 Verbindungen der Brücke neben Hub und Oberfläche nicht sicher, und die Zahl legt der Einschluss-Vertrag Windows fest (`halter:328`). Was unter Windows bis Schritt II gilt, bleibt die Frage 8 in Abschnitt 17a.9 (OPEN 23).

## 6. Weg zum Ziel (V, Festlegung)

- **Dieser Abschnitt gilt für jedes Ziel, das der Modellweg selbst erreicht.** Das sind in Betriebsart 1 das eine Ziel und in Betriebsart 2 die ungebundenen Ziele. Ein gebundenes Ziel der Betriebsart 2 erreicht ab Schritt I nur der Halter (Abschnitt 5.1); Auflösung, TLS und Fristen nach außen gehören dann ihm.
- **Ziele nur aus der Konfiguration.** Der Modellweg verbindet sich nur mit Host und Port der Ziele aus seinem Aufruf oder seiner Zielliste. Eine Anfrage des Harness kann kein anderes Ziel erreichen.
- **Einmal aufgelöst.** Er verbindet sich nur mit den Adressen aus Schritt 6 des Starts und löst den Namen nicht neu auf.
- **Keine Umleitung.** Ein 3xx geht als Antwort an den Harness, ohne `Location`. Der Modellweg folgt ihm nicht.
- **TLS mit dem Truststore des Systems** des Rechners, nie mit einem festen CA-Bündel. Grund: Firmen-Proxys bringen oft eigene Stammzertifikate mit, die im Truststore des Systems stehen. Hostname und Kette prüft TLS gegen den Host des Ziels.
  Weil die Umgebung leer ist, wirken `SSL_CERT_FILE` und `SSL_CERT_DIR` nicht. `http` gilt nur in Betriebsart 1 für `127.0.0.1` und `[::1]`.
- **Fristen:** Verbindung und TLS binnen 10 s; der Kopf der Antwort binnen 600 s nach dem Senden.
  Scheitert die Verbindung, gilt `mw.ziel_nicht_erreichbar` (502). Kommt kein Kopf, gilt `mw.ziel_frist` (504).
- Der Modellweg läuft im Netz des Rechners. Eine Grenze des Betriebssystems für seinen Prozess (etwa nftables je cgroup, root bei der Installation) ist ein Folgeschritt (OPEN 1).

## 7. Antwort an den Harness: Leck-Sperre und eigene Antworten (V)

- **Leck-Sperre.** Der Modellweg sucht die Bytes jedes eigenen Schlüssels in jedem Kopf, den er an den Harness gibt, und im Körper, auch über Teilgrenzen hinweg.
  Für ein gebundenes Ziel der Betriebsart 2 hat er ab Schritt I keinen eigenen Schlüssel; dort sperrt der Halter (Abschnitt 5.1, `halter.leck_gesperrt`), und der Modellweg gibt den Strom des Halters unverändert weiter. Die Sperre des Halters läuft auch über die Rahmengrenzen eines Stroms (H5, entschieden 23.11.).
  Dafür hält er die letzten (Länge des längsten Schlüssels − 1) Bytes zurück, bis der nächste Teil oder das Ende kommt.
  - Fund vor dem ersten Byte an den Harness: Statt der Antwort kommt 502 mit `mw.leck_gesperrt`.
  - Fund später im Strom: Der Modellweg reißt die Verbindung zum Harness ab. Was vorher gesendet war, enthält den Schlüssel nicht.
  - Kodierte Formen (Base64, Prozentkodierung) erkennt Fassung 1 nicht (OPEN 2).
  - In Betriebsart 2 bekommt ein gebundenes Ziel `accept-encoding: identity`; eine komprimierte Antwort von dort wird verworfen (`mw.antwort_komprimiert`, Abschnitt 13.3).
  - In Betriebsart 2 gilt die Sperre nach einer Umstellung auf WebSocket für die Nutzdaten der Rahmen, nicht für die rohen Bytes; eine ausgehandelte Erweiterung gibt es nicht (Abschnitt 13.3).
- **Köpfe** der Antwort legt jede Betriebsart fest (Abschnitte 11 und 13.3). Nie gehen `set-cookie` oder `location` mit.
- **Eigene Antworten** (Ablehnungen und Fehler am Ziel): fester Status, `Content-Type: application/json` und der Körper `{"error":{"code":"mw.<code>","message":"<fester Text>"}}` in RFC 8785.

## 8. Protokollzeile

Je Anfrage entsteht genau eine Zeile in der Kette aus Vertrag 5, auch für Ablehnungen. Ausgenommen ist nur `mw.protokoll_fehlt`.

**Felder für jede Betriebsart:**

| Feld | Wert |
|---|---|
| `aktionsname` | genau `modellweg.aufruf` |
| `betriebsart` | die Betriebsart des Prozesses |
| `ergebnis` | `weitergereicht` (das Ziel hat geantwortet, mit jedem Status), `abgelehnt` (eigene Ablehnung), `ziel_fehler`, `leck_gesperrt` |
| `fassung` | genau `2` |
| `grund` | `null` bei `weitergereicht`, sonst der Code (`mw.…`) passend zum Ergebnis |
| `route`, `sitzung` | aus dem Aufruf |
| `status` | Status, den der Harness bekommen hat (100 bis 599); `null` nur in Betriebsart 2, wenn keine HTTP-Antwort möglich war (`mw.sni_falsch`) |
| `pfad` | der begrenzte Pfad der Anfrage, Form unten; `null`, wenn die Zeile ihn nicht trägt |
| `host` | Hostname des zugeordneten Ziels aus der Zielliste, klein und ohne Port (nur Betriebsart 2, Abschnitt 13.4); in Betriebsart 1 immer `null` |
| `vorher`, `zeit` | wie Vertrag 5 |

Dazu kommen die Felder der Betriebsart: `modell`, `token_ein`, `token_aus` in Betriebsart 1 (Abschnitt 11); `ziel`, `methode`, `pfadregel`, `kopf` in Betriebsart 2 (Abschnitt 13.4).

**Das Feld `pfad` (Projektentscheid, 22.11.2026).** Es steht in beiden Betriebsarten, damit die Form gleich bleibt.

- **Gesetzt** bei jeder Zeile mit dem Ergebnis `abgelehnt` und bei jeder Anfrage mit der Methode `GET`, sonst `null`.
  `null` steht auch dann, wenn keine Anfragezeile gelesen wurde und es also keinen Pfad gibt (etwa `mw.anfrage_unlesbar`, `mw.ausgelastet`, `mw.nur_connect`, `mw.sni_falsch`).
  In Betriebsart 1 trägt nur eine Ablehnung einen Pfad: Ein `GET` kommt nie durch Stufe 4 oder 5 (Abschnitt 10).
- **Wert:** höchstens die ersten zwei Segmente des Pfads, ohne Abfrage und ohne Fragment, mit führendem `/`, etwa `/models/session`.
  Ein Pfad mit einem Segment ergibt `/<seg>`, der Pfad `/` ergibt `/`.
- **Jedes Segment höchstens 32 Zeichen. Fail closed (Festlegung des Baus):** Ein Segment, das länger als 32 Zeichen ist, leer ist oder ein Zeichen außerhalb von `[A-Za-z0-9._~-]` enthält (auch eine Prozentkodierung), steht als `*`.
  Es wird nie gekürzt und nie dekodiert. So kann kein Teil eines Namens hineinrutschen. `/a/` ergibt `/a/*`, `/a//b` ergibt `/a/*`, `/%2e%2e` ergibt `/*`.
- Ein Pfad, der nicht mit `/` beginnt (Form `*` oder absolute URI), ergibt `*`. Beide Betriebsarten weisen eine solche Anfragezeile schon vorher mit `mw.anfrage_unlesbar` ab; die Regel gilt trotzdem.
- **Admin-Schalter `--protokoll-pfad-voll` (Abschnitt 4), Vorgabe aus.** Nur mit ihm steht der ganze Pfad ohne Abfrage und ohne Fragment im Feld.
  Auch dann gilt: Ein Segment mit einem Zeichen außerhalb von `[A-Za-z0-9._~-]` wird zu `*`, und ein Pfad über 512 Zeichen macht das ganze Feld `*`. Die Grenze von 32 Zeichen je Segment entfällt.
  Die Einstellung für Admins (Vertrag 2) und das Weiterreichen durch „Lauf starten“ (Vertrag 1) gehören diesen beiden Verträgen; dieser Vertrag legt nur die Option und das Feld fest (OPEN 26).
- **Prüfbare Form des Feldes.** Im Feld steht `*`, `/`, oder `/` und Segmente, die je `*` oder `[A-Za-z0-9._~-]+` sind, zusammen höchstens 512 Zeichen.
  So kommt in keiner Form eine Abfrage, ein Fragment oder eine Prozentkodierung in die Zeile. Der Prüfer von Vertrag 5 prüft genau diese Form (Vertrag 5, Abschnitt 6.7).
- **Geschlossene Form je Betriebsart.** Die Felder für jede Betriebsart und die Felder der Betriebsart, kein weiteres: 15 Felder in Betriebsart 1, 16 in Betriebsart 2.
- Die Zeile trägt keinen Prompt, keine Antwort, keinen Reasoning-Text, keinen Schlüssel, keinen Platzhalter, keine Adresse und keinen Namen.
  Vom Pfad der Anfrage trägt sie nur das Feld `pfad` in den Grenzen oben: höchstens zwei Segmente, jedes höchstens 32 Zeichen, jedes außerhalb der Zeichenform als `*`, nie gekürzt und nie dekodiert; mit dem Admin-Schalter der ganze Pfad in derselben Zeichenform.
  **Grund für die Grenzen:** Ein abgelehnter Pfad auf einem fremden Dienst kann Namen von Organisation und Repo tragen, das ist Firmeninhalt. Deshalb steht der Pfad begrenzt in der Zeile und der ganze Pfad nur, wenn ein Admin es einschaltet.
  **Bekanntes Restrisiko:** Bei Pfaden der Form `/repos/<org>/…` steht im zweiten Segment der Name der Organisation, wenn er der Zeichenform genügt. Wie das zu behandeln ist, entscheidet die Projektleitung (OPEN 27).
  Sonst stammt kein Wert als freier Text aus der Anfrage des Harness.
- **Bibliothek:** `zeile_modellweg(&Modellweg, vorher)` und `anhaengen_modellweg(ordner, &Modellweg)` mit Sperre und Fail-closed-Regel wie `anhaengen_gesperrt` (Vertrag 5).
  Gebaut sind sie im Crate `modellweg`, Modul `protokoll`, über `formate::kette` (`kanonisch`, `zeit_text`, `anhaengen_zeile`); nur dieses Modul ruft `formate`. Der Prüfer der Form sitzt in `formate` (Vertrag 5, Abschnitt 6.7).
  `Modellweg` ist eine Aufzählung je Betriebsart. Die Fehlerfolge steht je Betriebsart in den Abschnitten 11 und 13.4; sie beginnt immer mit `betriebsart` und endet mit `vorher` und danach `zeit_ungueltig`.
- **Zeitpunkt:** nach dem Ende oder Abriss der Antwort, bei Ablehnungen sofort. Die Zeit liest der Modellweg selbst und übergibt sie in Millisekunden.
- **Scheitert das Anhängen,** antwortet der Modellweg auf jede weitere Anfrage mit `mw.protokoll_fehlt` (503), bis die Sitzung neu startet.

**Quelle `gateway`, Datei `<protokoll>/gateway-<JJJJ-MM>.jsonl`. Begründung:**

1. Der Modellweg ist die dünne Form des Gateways aus Vertrag 7 (Festlegung). Das spätere Gateway übernimmt dieselbe Aufgabe und setzt dieselbe Kette fort.
2. `abschnitt()` aus Vertrag 5 kennt `gateway` schon; es braucht keine neue Quelle in `x-kettenformat`.
3. `halter` passt nicht: Diese Kette gehört dem Halter und seinen eigenen Entscheidungen. `brain` und `stelle` gehören anderen Prozessen.
4. Auf dem Vorführrechner läuft kein altes Gateway, also ist die Regel „altes und neues Gateway nie gleichzeitig“ erfüllt. Mehrere Modellweg-Prozesse (einer je Sitzung) ordnen sich über die Sperre aus Vertrag 5, wie die Such-Prozesse in `brain`.

**Zeilenart `modellweg.aufruf` statt `modell.aufruf`. Begründung:** Vertrag 7 hat für `modell.aufruf` eine geschlossene Form mit Anbieter, Kosten, Währung und Einheit.
Der Modellweg kennt nichts davon. Eine eigene Zeilenart hält jede geschlossene Form am Namen prüfbar und erfindet keine leeren Felder.
Kompatibel heißt: `route`, `sitzung`, `modell`, `token_ein`, `token_aus`, `zeit` und `vorher` haben in beiden Zeilenarten dieselbe Form und Bedeutung. Eine Auswertung je Route, Sitzung und Modell liest beide.
`ergebnis` hat je Zeilenart eigene Werte. Vertrag 7 übernimmt `status` und `grund` in `modell.aufruf`, wenn er weitergeführt wird.

## 9. Probelauf mit Marker-Zählung (V, gleiche Form wie Vertrag 4; Bedingung Projektentscheid, 20.11.2026)

Für den Probelauf M1 (Messpunkt S5 im Drehbuch) zählt der Modellweg Marker, ohne Inhalte zu schreiben. Die Zählung gibt es **nur im Probelauf**.

- **Optionen:** der Schalter `--probelauf`, dazu `--marker <a,b,...>` (1 bis 16 Marker, je 1 bis 32 Zeichen aus `A-Z0-9-`, ohne Doppel) und `--marker-zaehlung <datei>` (absoluter Pfad).
  Alle drei zusammen oder keiner. Fehlen alle drei, zählt und schreibt der Modellweg nichts. Fehlt eine, steht `--probelauf` allein da oder ist ein Marker ungültig, endet er mit Exit 64 und `mw.probelauf_aufruf` und startet nichts.
- **Start im Probelauf:** Vor der Bereitschaft schreibt der Modellweg den Anfangsstand der Zähldatei. Danach hängt er genau eine Zeile der gemeinsamen Zeilenart `probelauf.zaehlung` an seine Kette (Quelle `gateway`, `gateway-<JJJJ-MM>.jsonl`):

  ```
  {"aktionsname":"probelauf.zaehlung","anzahl":2,"ergebnis":"an","fassung":2,"programm":"modellweg","sitzung":"s-2","vorher":"<64 hex>","zeit":"2026-11-20T16:30:00.000Z"}
  ```

  Die Zeile hat genau diese acht Felder. `anzahl` ist die Zahl der Marker (1 bis 16), `programm` eine Kennung (hier `modellweg`). Die Marker selbst stehen nie in der Zeile.
  Fehlerfolge der Bibliothek: `programm`, `sitzung`, `anzahl`, `vorher`; danach `zeit_ungueltig`.
  Lässt sich die Zeile nicht schreiben, startet der Modellweg nicht (Exit 74, `mw.protokoll_fehlt`). Ist die Zähldatei nicht schreibbar, startet er ebenfalls nicht (Exit 74, `mw.zaehlung_fehler`) und schreibt keine Startzeile. Liegt unter dem Zwischennamen `<name>.<pid>.neu` schon etwas, das dieser Aufruf nicht angelegt hat, wird es nicht entfernt; das Schreiben scheitert dann (fail closed, `mw.zaehlung_fehler`).
  Dann folgt die feste stderr-Zeile `mw.probelauf: Marker werden gezählt` und danach die Bereitschaft.
- **Bibliothek:** `anhaengen_probelauf` kommt in `formate`. Bis sie da ist, kapselt der Modellweg den Aufruf in seinem Protokoll-Modul.
- **Zählen:** Je angenommener Anfrage zählt der Modellweg, wie oft jeder Marker Byte für Byte im Körper vorkommt, den er an das Ziel schickt.
  Er nutzt denselben Puffer kurz vor dem Senden und zählt ohne Überlappung von links nach rechts. Köpfe zählen nicht, und abgelehnte Anfragen zählen nicht.
  Nach einer durchgereichten WebSocket-Umstellung zählt er zusätzlich die Rahmen vom Harness zum Ziel (Unterabschnitt „WebSocket im Probelauf“ unten).
- **Datei:** Danach schreibt er die Datei atomar neu (Modus 0600): `{"anfragen":<n>,"marker":{"<marker>":<zahl>,...},"sitzung":"<sitzung>"}` in RFC 8785.
  Den Anfangsstand mit `anfragen` 0 schreibt er vor der Bereitschaft (Schritt 5).
- **Fail closed:** Ist die Datei nicht schreibbar, geht die Anfrage nicht hinaus. Sie bekommt 503 und `mw.zaehlung_fehler`; der Zählstand bleibt beim letzten geschriebenen Wert. Die Protokollzeile trägt `ergebnis` `abgelehnt`.
- Die Protokollzeile bekommt kein Feld für die Zählung; sie bleibt, wie sie ist.
- **WebSocket im Probelauf (Nachtrag 17, Z1 bis Z3; nur Betriebsart 2, nur nach einer durchgereichten 101; fail closed).** Die Copilot CLI ruft das Modell über WebSocket auf (`GET /responses`). Ohne diese Zählung stünde S5 auf der Cloud-Route fälschlich auf 0.
  Die Fälle stehen in `faelle/probelauf-websocket.json`.
  - **Zählen (Z1).** Nach der 101 liest der Weiterreicher im Probelauf auch die Rahmen vom Harness zum Ziel nach RFC 6455 Abschnitt 5.2. Er entmaskiert die Nutzdaten der Datenrahmen (Opcode `0x1`, `0x2` und Fortsetzung `0x0`) **nur für die Zählung**.
    Er zählt jeden Marker Byte für Byte und ohne Überlappung (von links nach rechts) über die Folge aller Datenrahmen-Nutzdaten in der Reihenfolge ihres Eintreffens, über Rahmen- und Nachrichtengrenzen hinweg. Ein Marker, der über zwei Rahmen verteilt ist, zählt einmal.
    Steuerrahmen (`0x8`, `0x9`, `0xA`) zählen nicht und tragen nichts in die Folge; ein Steuerrahmen zwischen zwei Teilen einer Nachricht unterbricht sie nicht. Die Grenzen der gelesenen Teile (TCP-Segmente) haben keine Wirkung.
    Die Bytes an das Ziel bleiben unverändert, also maskiert, wie der Harness sie gesendet hat. W6 bleibt: kein Umschreiben, kein Entmaskieren auf dem Draht.
  - **Zähldatei (Z2).** Nach jedem vollständigen Datenrahmen schreibt der Weiterreicher die Zähldatei atomar neu (Modus 0600, Form unverändert). Ein Steuerrahmen schreibt nichts. `anfragen` zählt weiter nur HTTP-Anfragen, die Upgrade-Anfrage eingeschlossen (Körper: keiner); die Rahmen erhöhen es nicht.
    Es gibt kein zusätzliches Feld: Die Form der Zähldatei, die der Läufer liest, ändert sich nicht. Der Läufer prüft im Probelauf auf der Cloud-Route, dass `anfragen` mindestens 1 ist.
  - **Fail closed (Z3).** Ein Rahmen geht erst an das Ziel, wenn er vollständig gelesen, geprüft und gezählt und die Zähldatei geschrieben ist. Ein Rahmen, der beim Ende der Verbindung unvollständig ist, geht nicht hinaus und wird nicht gezählt; das ist kein Abriss.
    - Ist die Zähldatei nicht schreibbar, geht dieser Rahmen nicht hinaus. Beide Richtungen schließen, und es geht nichts mehr an das Ziel. Der Zählstand bleibt beim letzten geschriebenen Wert. Die Zeile hat das Ergebnis `abgelehnt` und den Grund `mw.zaehlung_fehler`, `status` 101.
    - Ist ein Rahmen vom Harness nicht lesbar, gilt dasselbe mit dem Grund `mw.anfrage_unlesbar`. Nicht lesbar heißt: ein RSV-Bit gesetzt, das Maskenbit fehlt (ein Harness maskiert immer, RFC 6455 Abschnitt 5.1), ein anderer Opcode als `0x0`, `0x1`, `0x2`, `0x8`, `0x9`, `0xA`, eine Länge über 33554432 oder ein Regelverstoß wie in Abschnitt 13.3 (Steuerrahmen mit FIN=0 oder Länge über 125, Fortsetzung ohne offene Nachricht, neuer Datenrahmen bei offener Nachricht).
      Die Prüfung läuft, sobald der Rahmenkopf bis zur Längenangabe da ist, in der Reihenfolge RSV, Maskenbit, Opcode, Länge, Regeln.
    - Ohne Probelauf gilt W6 unverändert: Diese Richtung wird nicht geprüft und nicht gezählt.
  - Mutanten: M-Z0 bis M-Z4 (Abschnitt 14).
- **Grenze:** Das ist eine Messung im Prüfling selbst. Als Gegenprobe zählt der Läufer die Marker zusätzlich in dem, was der Agent sichtbar ausgibt.
- Ohne `--probelauf` gibt es weder Zähldatei noch Startzeile noch die stderr-Zeile.
- **Zurückgezogen (Projektentscheid, 20.11.2026):** Einen Mitschnitt von Anfragekörpern gibt es nicht, auch nicht als Schalter für einen Probelauf. Auf der lokalen Route gehen Kundendaten durch.
  Eine Kopie auf der Platte wäre im Betrieb eine Schwachstelle; was nie geschrieben wird, muss niemand löschen.

## 9a. Suche nach dem Schlüssel (Nachtrag 18)

Nach dem Lauf prüft der Läufer, ob der echte Schlüssel irgendwo im Arbeitsbereich oder in der sichtbaren Ausgabe der Sitzung steht (Prüfung E4). Diese Suche macht der Weiterreicher selbst, **außerhalb des Einschlusses**.
Grund: Er liest die Schlüsseldateien ohnehin (Abschnitt 5). So liest kein zweites Programm Geheimnisse, und der Wert verlässt den Weiterreicher nie. Im Paket heißt das Programm `/usr/lib/werkbank/werkbank-modellweg`.
Die Fälle stehen in `faelle/suche.json`, die Mutanten M-S1 bis M-S4 in Abschnitt 14.

**S1. Aufruf.**

```
werkbank-modellweg suche (--schluessel-datei <datei> | --zielliste <datei>) --in <pfad> [--in <pfad> ...]
```

- Genau eine Schlüsselquelle: `--schluessel-datei` (Route lokal, eine Datei) oder `--zielliste` (Route Copilot; dann gelten alle `schluessel_datei` der gebundenen Ziele, ohne Doppel; ungebundene Ziele tragen keine). Jede der beiden Optionen höchstens einmal.
- **Ab Schritt I trägt ein gebundenes Ziel der Betriebsart 2 keine `schluessel_datei`, sondern `halter_ziel`** (Abschnitte 5.1 und 12). Der Weiterreicher kennt den Wert dann nicht und sucht ihn nicht selbst. Stattdessen ruft er für jedes gebundene Ziel die Op `modell.suche` des Halters mit denselben `--in`-Pfaden (H8, entschieden im Halter-Vertrag, 23.11.2026): höchstens 16 absolute Pfade, nur reguläre Dateien, `O_NOFOLLOW`. Der Halter durchsucht sie nach seinen eigenen Geheimnissen und antwortet nur mit Zahlen; der Wert verlässt ihn nie. Die Regeln S3 bis S6 gelten unverändert, und die Zähler der Antwort gehen in dieselbe Summe wie die eigenen. **E4 kann damit für die Cloud-Route grün werden.** Erreicht der Weiterreicher den Halter nicht, antwortet der Halter mit einem Fehler, oder ist dort kein Geheimnis zum Ziel, endet die Suche mit Exit 78 und `mw.schluessel_fehlt`, und der Läufer meldet E4 als „offen", nie als grün.
- Mindestens ein `--in`, höchstens 16. Alle Pfade absolut nach Abschnitt 4 (ohne Segment `.` oder `..`, ohne Steuerzeichen, höchstens 1024 Byte). Option und Wert sind zwei Argumente; `--name=wert`, eine unbekannte Option oder ein fehlender Wert ist ein Aufruffehler.
- Sonst Exit 64 und `mw.suche_aufruf` (`Suchaufruf ungueltig`).

**S2. Reihenfolge und Schlüssel.**

1. **Stufe 0** (Abschnitt 3, F1) gilt auch hier und läuft vor jedem anderen Schritt: Exit 78, `mw.umgebung_unsicher`.
2. Aufruf nach S1 (64).
3. Bei `--zielliste` die Zielliste nach Abschnitt 12 lesen: eine ungültige ergibt Exit 78 und `mw.ziele_ungueltig`; ein `halter_ziel`, das nicht der `host` seines Ziels ist (`zl.halter_name`), ergibt Exit 78 und `mw.ziel_halter_name`, wie im Start (Abschnitt 3a, Pflicht 1; ein Code für eine Sache, 25.11.2026). alt: auch dieser Fall ergab `mw.ziele_ungueltig`. `ca_zulassung` spielt für die Suche keine Rolle. Eine gültige Liste ohne ein gebundenes Ziel ergibt Exit 78 und `mw.schluessel_fehlt`: Wo nichts zu suchen ist, gibt es kein Grün.
3a. **Präzisierung des Baus (25.11.2026):** Mit `--zielliste` sucht der Weiterreicher ab Schritt I **nichts selbst**. Ein gebundenes Ziel trägt keine `schluessel_datei`, ein ungebundenes trug nie einen Schlüssel; es gibt also keinen eigenen Wert, gegen den er suchen könnte. Seine Zeile ist genau die Summe der Antworten des Halters. Folge, benannt: Teilen zwei gebundene Ziele dasselbe Geheimnis, zählt der Halter dieselben Dateien und dieselben Treffer je Ziel einmal, und die Summe nennt sie mehrfach. Der Ausgang (Exit 0, 1 oder 3) bleibt davon unberührt, denn er hängt nur daran, ob `treffer` und `nicht_lesbar` über 0 liegen. Mit `--schluessel-datei` (Route lokal) bleibt die eigene Suche unverändert.
3b. Trägt die Liste gebundene Ziele, braucht die Suche den Halter; sie findet ihn unter Linux am festen Pfad aus Abschnitt 5.1, nicht über eine Option (Abgleich 25.11.2026; alt: „`--halter-socket` ist dann Pflicht (sonst Exit 64 `mw.suche_aufruf`)"). Der Weiterreicher verbindet sich, prüft die Gegenseite und ruft je `halter_ziel` einmal `modell.suche` mit `ziel` und allen `--in`-Pfaden im Feld `pfade` (1 bis 16 Pfade, je bis 4096 Zeichen; Halter-Vertrag, Abschnitt `modell.suche`, Felder `ziel` und `pfade`; alt: „`rust/spec/halter/README.md:461` bis `:464`", ein veralteter Zeilenverweis). Scheitert das Verbinden, die Prüfung oder ein Aufruf, oder meldet der Halter zu einem Ziel kein Geheimnis, ist das Exit 78 und `mw.schluessel_fehlt`. Die Zähler der Antworten kommen in die Summe von S4; `nicht_lesbar` des Halters zählt wie das eigene.
4. Jede Schlüsseldatei nach Abschnitt 5 lesen (eigene Datei, 0600, `O_NOFOLLOW`, eine Zeile). **Der gesuchte Wert ist genau der Wert, den der Weiterreicher einsetzt:** ohne das abschließende LF, ohne jede Umformung. Verletzt eine Datei Abschnitt 5, in Form oder Rechten, ist das Exit 78 und `mw.schluessel_fehlt` (`Schluessel nicht lesbar`; derselbe Code und Text wie in Abschnitt 13.4, hier mit Exit 78), und es wird nichts gesucht, auch nicht mit den anderen Schlüsseln.
5. Zwei Dateien mit demselben Wert ergeben einen Schlüssel; ein Fund zählt dann einmal.

**S2a. Gegenprobe des Läufers auf der Cloud-Route (Präzisierung 25.11.2026).**

Die Zielliste nennt auf der Cloud-Route keine Schlüsseldatei mehr, darum hat die Gegenprobe von E4 dort kein eigenes
Ziel mehr im Arbeitsbereich. Sie zielt stattdessen auf die Datei des Geheimnisses in der Ablage des Halters
(`<heim>/.local/state/agent-workbench/halter/geheim/`, Name des Geheimnisses zum Ziel aus `/etc/werkbank/halter.json`;
Halter-Vertrag, `rust/spec/halter/README.md:267` und `:264`). Der Läufer legt nur den Verweis an und liest die Datei
nie. Der Verweis liegt wie bisher in einem eigenen Ordner 0700 außerhalb jedes Arbeitsordners, und die Suche bekommt
nur diesen Ordner; folgt sie dem Verweis, ist E4 rot (S3). Das ist kein Pfad einer Sitzung: Die Ablage wird nicht in
den Einschluss eingebunden und bleibt dort unsichtbar (Halter-Vertrag, `rust/spec/halter/README.md:820` bis `:821`),
und die Suche läuft ohnehin außerhalb des Einschlusses (S1). Auf dem Halterweg sucht der Halter selbst, und er
öffnet jede Datei mit `O_NOFOLLOW`; ein Verweis zählt bei ihm wie beim Weiterreicher in `uebersprungen`.
`<heim>` ist in Fassung 1 das Heim aus dem eigenen Eintrag in `/etc/passwd` (Halter-Vertrag, Abschnitt 2.3, „Ablage"). Unter einem späteren Systemnutzer des Halters (Halter-Vertrag, OPEN 9 und 18) wäre es das Heim dieses Nutzers, und die Gegenprobe zielte dann auf einen anderen Ort (OPEN 37).

**S3. Was durchsucht wird.**

- Reguläre Dateien unter jedem `--in`, rekursiv. Eine Datei, die über mehrere `--in` erreichbar ist (verschachtelte oder doppelte `--in`), wird nur einmal gelesen und gezählt (gleiches Gerät, gleicher Inode).
- **Eine symbolische Verknüpfung wird weder gelesen noch verfolgt noch als Fund gezählt.** Grund: Ein Agent könnte im Arbeitsordner einen Verweis auf die Schlüsseldatei anlegen. Sie zählt in `uebersprungen`. Das gilt auch für Verknüpfungen auf Ordner; der Ordner dahinter wird nicht betreten.
- Andere Dateiarten (Socket, FIFO, Gerät) werden nicht geöffnet und zählen in `uebersprungen`, nicht in `nicht_lesbar`.
- Eine Datei oder ein Ordner, der nicht gelesen werden kann, zählt in `nicht_lesbar`; in einen nicht lesbaren Ordner steigt die Suche nicht ab.
- **Ein `--in` selbst** muss ein Ordner oder eine reguläre Datei sein. Ist es eine Verknüpfung, fehlt es, oder ist es eine andere Art, gilt es als nicht gelesen und zählt in `nicht_lesbar` (Exit 3, nie 0): Eine Suchwurzel, die nicht durchsucht wurde, ist kein „kein Fund“. (Festlegung des Vertrags, zu bestätigen: S3 nennt für diesen Fall nur, dass die Verknüpfung nicht verfolgt wird.)
- **Tiefe (Nachtrag 19).** Die Suche steigt höchstens 256 Ordnerebenen unter eine Wurzel; die Wurzel selbst hat die Tiefe 0. Ein tieferer Ordner wird nicht betreten und zählt in `nicht_lesbar` (Exit 3, nie 0). So endet ein Baum, der tiefer ist, als der Prozess Deskriptoren hat, nie grün. (Festlegung des Baus, bestätigt am 21.11.2026)

**S4. Zählung.** Jedes genaue Vorkommen jedes Schlüssels als Bytefolge, von links nach rechts und ohne Überlappung (je Schlüssel für sich), auch über Lesegrenzen hinweg. `treffer` ist die Summe über alle Dateien und alle Schlüssel. Ein Schlüssel, der Teil eines anderen ist, zählt in dessen Vorkommen mit.
Der Bau liest in Blöcken und hält je Schlüssel die letzten (Länge − 1) Bytes zurück; seine Tests laufen mit einem kleinen Puffer, damit die Grenze oft getroffen wird.
**Die Schlüsseldateien selbst werden nicht ausgenommen.** Liegt eine unter einem `--in`, zählt sie als Fund und der Lauf ist rot. Die Ausnahme per Gerät und Inode ist nicht nötig: Der Läufer legt Schlüssel außerhalb jedes `--in` (S8), und eine Ausnahme wäre ein Versteck für einen harten Verweis auf dieselbe Datei.

**S5. Ausgabe.** Auf stdout genau eine Zeile in RFC 8785 und LF: `{"dateien":<n>,"nicht_lesbar":<n>,"treffer":<n>,"uebersprungen":<n>}`.
`dateien` ist die Zahl der gelesenen regulären Dateien. Die Zeile enthält nie einen Wert, nie einen Teil, keine Länge und keinen Dateinamen. stderr trägt bei Exit 0, 1 und 3 nichts; bei Exit 64 und 78 genau die feste Zeile `<code>: <grund>` nach Abschnitt 4 und stdout bleibt leer. Lässt sich die Zeile auf stdout nicht schreiben, endet die Suche mit Exit 70 und der festen Zeile von `mw.intern` auf stderr (Nachtrag 19; wie der unerwartete Fehler in Abschnitt 4). Der Läufer wertet jeden Exit außer 0, 1 und 3 als Fehler der Suche (S8), also nie als grün.

**S6. Exit.**

| Exit | Bedeutung | Bedingung |
|---|---|---|
| 0 | grün | `treffer` 0 und `nicht_lesbar` 0 |
| 1 | rot | `treffer` über 0, auch wenn etwas nicht lesbar war |
| 3 | offen | `treffer` 0 und `nicht_lesbar` über 0 |
| 64 | `mw.suche_aufruf` | S1 |
| 78 | `mw.umgebung_unsicher`, `mw.ziele_ungueltig`, `mw.ziel_halter_name`, `mw.schluessel_fehlt` | S2 |
| 70 | `mw.intern` | die Zeile auf stdout lässt sich nicht schreiben (S5, Nachtrag 19) |

| Code | Exit | Grund (fest, ASCII) |
|---|---|---|
| `mw.suche_aufruf` | 64 | `Suchaufruf ungueltig` |
| `mw.ziele_ungueltig` | 78 | `Zielliste ungueltig` |
| `mw.ziel_halter_name` | 78 | `Halterziel nicht der Host` (wie Abschnitt 4) |
| `mw.schluessel_fehlt` | 78 | `Schluessel nicht lesbar` |
| `mw.intern` | 70 | `interner Fehler` (wie Abschnitt 4) |

**S7. Keine Wirkung nach außen.** Die Suche schreibt keine Datei und keine Protokollzeile, legt keinen Socket an und öffnet keine Verbindung nach außen. Sie fasst die Kette nicht an und liest nur die Schlüsseldateien, die Zielliste und die Dateien unter `--in`.
Eine Ausnahme ist der Halter: Für gebundene Ziele verbindet sich die Suche zu seinem Endpunkt und ruft `modell.suche` (S2, Schritt 3a). Das ist eine lokale Verbindung zu einem Rollenprogramm derselben Maschine, kein Weg nach außen. Der Halter schreibt dafür seine eigene Zeile in seine eigene Kette (Halter-Vertrag); die Kette des Modellwegs bleibt unberührt.

**S8. Für den Läufer (wörtlich zu übernehmen).** Dieselbe Liste `--in` für beide Routen: Arbeitsordner, Protokollordner, Zählordner und die Datei mit der sichtbaren Ausgabe der Sitzung.
Diese Datei schreibt der Läufer 0600 in einen eigenen Ordner 0700 außerhalb jedes Arbeitsordners und löscht sie nach dem Aufruf.
Auswertung: Exit 0 grün; 1 rot; 3 offen `<schritt>.schluessel_suche_unlesbar`; 64 oder jede andere Ausgabe offen `<schritt>.schluessel_suche_fehler`.
Die Mustersuche im Einschluss bleibt eine zweite, schwächere Probe; allein ergibt sie nie grün.

**Grenze, ehrlich.** Die Suche findet den Wert Byte für Byte. Kodierte Formen (Base64, Prozentkodierung) findet sie nicht (OPEN 2). Sie sieht nur, was bis zum Aufruf auf der Platte steht; ein Wert, den der Agent im Speicher hielt und nie ausgab, bleibt unsichtbar.

---

# Teil B: Betriebsart 1 `openai-lokal` (Fassung 1, gebaut im Durchstich)

## 10. Optionen und Anfragen

**Optionen der Betriebsart:**

| Option | Herkunft | Form |
|---|---|---|
| `--ziel` | Feld `ziel` der Route in der Einstellungsdatei (Vertrag 2) | Basisadresse im OpenAI-Sinn: `https://<host>[:<port>][<präfix>]/v1` oder `http://127.0.0.1[:<port>][<präfix>]/v1` beziehungsweise `http://[::1]…/v1`; Port 1 bis 65535 ohne führende Null; Präfix aus Segmenten `/[A-Za-z0-9._~-]+`; kein Benutzerteil, keine Abfrage, kein `/` nach `/v1` |
| `--modell` | Feld `modell` der Route in der Einstellungsdatei | `^[A-Za-z0-9._:/@+-]{1,128}$` (wie `modell` in Vertrag 1) |
| `--schluessel-datei` | Konfiguration des Startprogramms (Installation) | absoluter Pfad wie in Abschnitt 4, höchstens 1024 Byte |

Das Projekt hat bestätigt (20.11.): `ziel` und `modell` sind Pflicht für Routen der Art `lokal`. Die Netzziele der lokalen Sitzung (`netzziele` in Vertrag 2) sind leer; der einzige Weg nach außen ist der Modellweg.
Trägt die Route ein `modell`, muss `modell` im Start gleich sein (`start.modell_route`). Der Modellweg selbst prüft nur gegen `--modell`.
Die Anbieter-Datei des Harness nennt `http://127.0.0.1:11434/v1` und höchstens einen Platzhalter als Schlüssel.

Der Modellweg spricht HTTP/1.1 auf dem Socket und bearbeitet höchstens 8 Anfragen gleichzeitig.

**Prüfreihenfolge (V).** Keiner dieser Schritte berührt das Ziel. Der erste zutreffende Schritt nennt den Code.

| Stufe | Code | Verletzung | Status |
|---|---|---|---|
| 1 | `mw.protokoll_fehlt` | das Anhängen einer Zeile ist vorher gescheitert (Abschnitt 8) | 503 |
| 2 | `mw.anfrage_unlesbar` | kein HTTP/1.1, Kopf ab 65536 Byte, `Content-Length` doppelt, nicht dezimal oder ungleich der Länge des Körpers | 400 |
| 3 | `mw.ausgelastet` | schon 8 Anfragen in Arbeit | 429 |
| 4 | `mw.pfad_unbekannt` | das Anfrageziel ist nicht genau `/v1/chat/completions`; auch ein Abfrageteil oder ein Punkt-Segment macht es falsch | 404 |
| 5 | `mw.methode_falsch` | die Methode ist nicht `POST` | 405 |
| 6 | `mw.chunked_nicht_unterstuetzt` | ein Kopf `Transfer-Encoding` ist vorhanden (vorläufig, bis zur Messung) | 411 |
| 7 | `mw.laenge_fehlt` | kein `Content-Length` | 411 |
| 8 | `mw.koerper_zu_gross` | `Content-Length` über 33554432 | 413 |
| 9 | `mw.json_ungueltig` | kein UTF-8, BOM, Syntaxfehler, doppelter Schlüssel auf jeder Ebene, oberste Ebene kein Objekt | 400 |
| 10 | `mw.modell_nicht_erlaubt` | `model` fehlt, ist kein Text oder nach dem JSON-Lesen nicht genau gleich `--modell` | 403 |
| 11 | `mw.zaehlung_fehler` | nur im Probelauf: die Zähldatei ist nicht schreibbar (Abschnitt 9) | 503 |

- Die Antworten haben die Form aus Abschnitt 7. Die festen Texte stehen in `faelle/anfragen.json`.
- **`Content-Length` ist Pflicht (vorläufig, bis zur Messung).** Eine Anfrage mit `Transfer-Encoding: chunked` bekommt 411 und `mw.chunked_nicht_unterstuetzt`, nie einen Abbruch ohne Grund. Danach schließt der Modellweg die Verbindung, weil er die Rahmung des Körpers nicht liest.
- **Doppelte Schlüssel** lehnt der Modellweg ab, weil ein Leser des Ziels bei zwei `model` oft den letzten nimmt. Sonst ginge ein erlaubtes Modell durch die Prüfung und ein anderes zum Ziel.
- **Ein Modell erlaubt, keine Liste.** Die Freigabe der Modelle am Ziel (403 `policy_model_blocked`) ist die zweite Sperre; die erste ist `--modell`.

**Anfrage an das Ziel (V).** Besteht die Anfrage alle Stufen, sendet der Modellweg genau diese Bytes, den Körper unverändert:

```
POST <präfix>/v1/chat/completions HTTP/1.1
Host: <host>[:<port>]
Authorization: Bearer <schlüssel aus der Datei>
Content-Type: application/json
Accept: <nur wenn der Harness genau application/json oder text/event-stream sendet>
Content-Length: <länge>
Connection: close

<körper, Byte für Byte wie vom Harness>
```

- **Jeder Kopf des Harness außer `Accept` wird verworfen**, ausdrücklich `Authorization` (etwa `Bearer platzhalter`), `X-Api-Key`, `Cookie`, `Host` und `X-Forwarded-*` (Abschnitt 5).
- Der Körper wird nicht umgeformt: kein neues Modell, kein `stream_options`, keine andere Reihenfolge. Gestreamte Anfragen gehen genauso hinaus.

## 11. Antworten, Köpfe und Tokens

**Unverändert durchreichen (V, Projektentscheid, 20.11.2026).** Status und Körper des Ziels gehen Byte für Byte an den Harness. Das gilt auch für Fehler wie 401 und 403 `policy_model_blocked`, für Antworten mit Feld `reasoning` und für SSE-Ströme.
Der Modellweg formt nichts um, sortiert nichts und fügt nichts ein. Einen Strom reicht er in der Reihenfolge des Eintreffens weiter; nur die Leck-Sperre (Abschnitt 7) hält Bytes kurz zurück.

**Köpfe an den Harness:** nur `content-type` des Ziels, wenn er genau einmal kommt und druckbares ASCII mit höchstens 256 Zeichen ist. Dazu kommt die eigene Rahmung: `Connection: close`, und der Körper endet mit dem Schließen.

**Tokens** liest der Modellweg nur aus `usage.prompt_tokens` und `usage.completion_tokens`, falls das Ziel sie liefert, aus dem JSON-Körper oder aus dem letzten SSE-Ereignis mit `usage`. Sonst sind beide `null`.
Er fordert sie nicht an; er ändert dafür keine Anfrage.

**Felder der Zeile in dieser Betriebsart** (15 Felder mit denen aus Abschnitt 8):

| Feld | Wert |
|---|---|
| `modell` | immer `--modell`, nie das angefragte Modell. So kommt kein Text aus der Anfrage in die Zeile |
| `token_ein`, `token_aus` | Ganzzahl 0 bis 4294967295 nur bei `weitergereicht` und nur, wenn das Ziel `usage` liefert; sonst beide `null` |
| `pfad` | der begrenzte Pfad (Abschnitt 8) nur bei `abgelehnt`, sonst `null`; ein `GET` kommt hier nie durch Stufe 4 oder 5 |
| `host` | immer `null`: Diese Betriebsart hat genau ein Ziel aus `--ziel`, und die Form bleibt gleich wie in Betriebsart 2 |

`status` ist in dieser Betriebsart nie `null`. Fehlerfolge: `betriebsart`, `route`, `sitzung`, `modell`, `ergebnis`, `grund`, `status`, `token_ein`, `token_aus`, `pfad`, `host`, `vorher`; danach `zeit_ungueltig`.

```
{"aktionsname":"modellweg.aufruf","betriebsart":"openai-lokal","ergebnis":"weitergereicht","fassung":2,"grund":null,"host":null,"modell":"lokal-beispiel/modell-a","pfad":null,"route":"lokal-beispiel","sitzung":"s-2","status":200,"token_aus":3,"token_ein":9,"vorher":"<64 hex>","zeit":"2026-11-20T15:00:00.000Z"}
```

---

# Teil C: Betriebsart 2 `kopf-einsetzen` (entschieden, Bau direkt nach `openai-lokal`, M1)

**Bau direkt nach `openai-lokal` (M1).** Projektentscheid, 20.11.2026. Grund: Die Person meldet sich mit einem Organisationskonto bei GitHub an. Ein lesbares Token im Einschluss wäre genau die Lücke, die die Vorführung ausschließen soll.
Offene Werte (`endpoints.*`, Pfade der Modellaufrufe, Ablage des Tokens) sind Parameter der Zielliste. Der Bau wartet nicht auf den echten Lauf.
Wo Betriebsart 2 nicht gebaut ist, gilt in `faelle/aufruf.json` das Feld `erwartet` (`mw.betriebsart_unbekannt`). Wo sie gebaut ist, gelten dort das Feld `erwartet_gebaut` und die Fälle in `faelle/kopf-*.json`.

**Köpfe (Projektentscheid, 20.11.2026, ergänzt am 20.11.2026, fail closed):** Der Weiterreicher schreibt nur zwei Köpfe um: `authorization` überall und `accept-encoding` an gebundenen Zielen (durch `identity`). Alle anderen Köpfe gehen unverändert durch; Cookies werden weder gesetzt noch verworfen.
Gemessen im ersten angemeldeten Start (Copilot CLI 1.0.88, Plan Free, 20.11.2026; scheiterte vor dem Modell): die Kopfnamen `accept`, `accept-encoding`, `authorization`, `content-length`, `content-type`, `copilot-harness-id`, `copilot-integration-id`, `editor-version`, `host`, `openai-intent`, `runtime-client-version`, `user-agent`, `x-client-machine-id`, `x-copilot-client-exp-assignment-context`, `x-copilot-exp-flight-names`, `x-github-api-version`, `x-initiator`, `x-interaction-id`; kein `cookie`. `authorization` ging nur an den Wert von `endpoints.api`. WebSocket gab es in diesem Lauf nicht.

## 12. Weg, Zulassung der CA und Zielliste

**Weg (G, Projektentscheid, 20.11.2026).** Eine umgestellte Basisadresse gibt es für den Copilot-Plan nicht: `endpoints.*` kommen aus `/copilot_internal/user`, und `COPILOT_PROVIDER_BASE_URL` ist BYOK und umgeht Copilot.
Die CLI nimmt `HTTPS_PROXY` an, löst dann selbst keinen Namen auf und verbindet sich nicht direkt (dns=0, direkt=0). Sie vertraut dem System-Bundle (rustls-platform-verifier); ohne passende CA lehnt sie TLS ab (`TLSV1_ALERT_UNKNOWN_CA`). Pinning wurde nicht beobachtet.
Daraus folgt der Weg:

1. Im Einschluss stehen `HTTPS_PROXY=http://127.0.0.1:8402` und `COPILOT_GITHUB_TOKEN=<Platzhalter im gho_-Format>`. Die CLI sendet den Platzhalter ohne lokale Prüfung (G).
2. Die CLI öffnet je Ziel `CONNECT <host>:443` beim Weiterreicher.
3. Der Weiterreicher beendet TLS mit einem Blattzertifikat der **Wegwerf-CA** der Sitzung.
4. Er prüft jede Anfrage im Tunnel (Host, Pfad, Methode, Upgrade, Anmeldung) und verwirft `Authorization`.
5. **Gebundenes Ziel:** Er gibt die geprüfte Anfrage mit `modell.aufruf` an den Halter; der setzt das Token ein und öffnet die Verbindung nach außen (Abschnitt 5.1). **Ungebundenes Ziel:** Er öffnet selbst eine TLS-Verbindung mit dem Truststore des Systems des Rechners (Abschnitt 6).

Nachmessung (Projektentscheid, 20.11.2026, Copilot CLI 1.0.88, Attrappe): Auch die Modellaufrufe gehen per `CONNECT` über `HTTPS_PROXY`; ein transparenter Weg hatte 0 Verbindungen. Ein Befund mit dem Gegenteil war ein Messfehler des Mess-Proxys und ist widerrufen. Projektentscheid, 20.11.2026: `CONNECT` bleibt der einzige Eingang in M1; ein Eingang per SNI ist nicht vorgesehen. Die Netzgrenze mit Gegenprobe E5 bleibt Pflicht: Eine CLI, die den Proxy nicht achtet, kommt im Einschluss nirgends hin.

**Wegwerf-CA (Projektentscheid, 20.11.2026, baubar, hergeleitet; Außentest M14 folgt).**
- „Lauf starten“ erzeugt die CA je Sitzung außerhalb des Einschlusses. Schlüssel und Zertifikat sind Sitzungsdateien (0600, Ordner 0700, Vertrag 1; Aufräumen durch den Kern-Dienst).
- Eigenschaften: `basicConstraints` `CA:TRUE` mit `pathlen` 0; Name Constraints mit erlaubten DNS-Namen genau für **alle** Hosts der Zielliste, gebundene und ungebundene; ausgeschlossene IP-Bereiche `0.0.0.0/0` und `::/0`; Laufzeit höchstens 24 h.
  Grund (entschieden 20.11. nach Vertragsfehler F3): Der Weiterreicher beendet TLS für jedes Ziel, um Methode und Pfad durchzusetzen. Fehlt der Host eines ungebundenen Ziels in den Name Constraints, lehnt ein prüfender Client das Blatt ab („permitted subtree violation“, gemessen vom Bau).
  Die erwartete Menge je gültiger Zielliste steht in `faelle/kopf-ziele.json` als `ca_erlaubte_namen`.
  **Grenze (Nachtrag 19, nach der Durchsicht von werkbank-lauf am 21.11.).** Nach RFC 5280 deckt ein erlaubter DNS-Name auch alle seine Unterdomänen. Die CA ließe also auch ein Blatt für eine Unterdomäne eines Ziels zu. Das wirkt nur, wenn jemand außer dem Weiterreicher Blätter ausstellen kann. Der Weiterreicher stellt Blätter nur für die genauen Hosts der Zielliste aus, und der Schlüssel der CA liegt nie im Einschluss (Vertrag 1). Erst ein Leck dieses Schlüssels erlaubte Blätter für Unterdomänen, und auch dann nur gegenüber dem Bundle dieses Einschlusses und höchstens für die Laufzeit der CA. Eine engere Form, etwa ausgeschlossene Unterdomänen je Host, lehnt die Startprüfung ab (Abschnitt 13.5: genau die zwei IP-Bereiche als Ausschluss). Eine leere Zielliste ergäbe eine CA ohne DNS-Grenze; dazu kommt es nicht: Die Zielliste hat 1 bis 16 Ziele, und „Lauf starten“ endet bei einer Route ohne tragfähiges Netzziel vor der CA (Vertrag 1, Schritt 7a0).
- Ins Bundle des Einschlusses kommt nur das Zertifikat. „Lauf starten“ bindet eine Sitzungsdatei (System-Bundle des Rechners plus CA) an die Stelle von `/etc/ssl/certs/ca-certificates.crt`, nur im Einschluss. Der Truststore des Rechners bleibt unverändert.
- Die CA kommt nicht über `SSL_CERT_FILE`; das löst die Log-Warnung `forceLoginOrgs` aus (G, t2).
- Der Weiterreicher bekommt Schlüssel und Zertifikat als `--ca-schluessel` und `--ca-zertifikat` und erzeugt daraus die Blattzertifikate je Ziel im eigenen Speicher.

**Zulassung der CA durch die Organisation (Projektentscheid, 20.11.2026, fail closed).** Die Organisation muss die lokale CA auf dem Rechner zulassen. Die Zielliste trägt dafür das Feld `ca_zulassung`:
- `zugelassen`: Betriebsart 2 darf starten.
- `offen` (keine Antwort) oder `abgelehnt`: Der Weiterreicher startet nicht. Er endet vor der Bereitschaft mit Exit 78 und `mw.ca_nicht_zugelassen` (`Lokale CA nicht zugelassen`).
  „Lauf starten“ meldet dann `lauf.modellweg_nicht_bereit`, und die Cloud-Sitzung startet nicht.
- **Es gibt keinen Rückweg mit Token im Einschluss.** Kein Schalter, keine Option und kein Fehlerfall setzt das echte Token in die Umgebung, in eine Datei oder in einen Schlüsselbund des Einschlusses.
- Im Probelauf meldet der Läufer E4 für die Cloud-Route dann als „offen“ mit dem Grund `mw.ca_nicht_zugelassen`, nie als grün.

**Zielliste (V, Projektentscheid, 20.11.2026).** Eine **Sitzungsdatei**, die „Lauf starten“ aus `netzziele` der Route baut; die Pfade stehen in der Einstellungsdatei (Vertrag 2 Fassung 2: Zugriff `kopf-einsetzen` mit Schlüssel und Pfaden).
Sie liegt außerhalb des Einschlusses und kommt nie aus der Startdatei, der Startanfrage oder vom Agenten. Übergabe als `--zielliste <datei>`. JSON, UTF-8, höchstens 65536 Byte, geschlossene Form auf jeder Ebene.

```json
{"fassung": 1, "ca_zulassung": "zugelassen", "ziele": [
 {"name": "github-api", "host": "api.github.com", "port": 443, "halter_ziel": "api.github.com",
  "regeln": [{"methode": "GET", "pfad": "/copilot_internal/user"}, {"methode": "GET", "pfad": "/copilot/mcp_registry"}], "upgrade": "durchreichen"},
 {"name": "copilot-api", "host": "api.githubcopilot.com", "port": 443, "halter_ziel": "api.githubcopilot.com",
  "regeln": [{"methode": "GET", "pfad": "/agents/swe/internal/memory/v0/enabled"}], "upgrade": "durchreichen"},
 {"name": "copilot-plan", "host": "api.individual.githubcopilot.com", "port": 443, "halter_ziel": "api.individual.githubcopilot.com",
  "regeln": [{"methode": "GET", "pfad": "/models"}, {"methode": "GET", "pfad": "/responses"}, {"methode": "POST", "pfad": "/auto"},
   {"methode": "POST", "pfad": "/models/session"}, {"methode": "POST", "pfad": "/models/session/intent"},
   {"methode": "POST", "pfad": "/chat/completions"}, {"methode": "POST", "pfad": "/agents/sessions"}], "upgrade": "durchreichen"},
 {"name": "copilot-telemetrie", "host": "telemetry.individual.githubcopilot.com", "port": 443, "halter_ziel": null, "regeln": [{"methode": "POST", "pfad": "/telemetry"}], "upgrade": "ablehnen"},
 {"name": "copilot-exp", "host": "exp.individual.githubcopilot.com", "port": 443, "halter_ziel": null, "regeln": [{"methode": "POST", "pfad": "/api/v1/assignments"}], "upgrade": "ablehnen"}]}
```

| Feld | Form | Bedeutung |
|---|---|---|
| `fassung` | Ganzzahl `1` | |
| `ca_zulassung` | `zugelassen`, `offen` oder `abgelehnt` | Antwort der Organisation auf die Zulassung der lokalen CA; nur `zugelassen` erlaubt den Start |
| `ziele[]` | 1 bis 16 | |
| `name` | Kennung, ohne Doppel | Name des Ziels für die Protokollzeile |
| `host` | DNS-Name in Kleinbuchstaben mit mindestens einem Punkt, höchstens 253 Zeichen; keine IP-Adresse, kein `*` | genau ein Host |
| `port` | Ganzzahl 1 bis 65535 | genau ein Port; `host` und `port` zusammen ohne Doppel |
| `halter_ziel` | **genau der `host` dieses Ziels** (gebundenes Ziel) oder `null` (ungebundenes Ziel) | Name des Ziels in `halter.json`, unter dem der Halter Adresse, Kopfnamen, erlaubte Pfade und den **Namen** des Geheimnisses führt; ohne Doppel. Die Zielliste nennt weder Kopfnamen noch Vorsatz noch einen Pfad zu einem Schlüssel und auch nicht den Namen des Geheimnisses: Das steht nur beim Halter (Abschnitt 5.1). `halter.json` liegt unter Linux root-eigen mit 0644 unter `/etc/werkbank/`, für die Person nicht schreibbar (Entscheidung im Halter-Vertrag, 23.11.2026); unter Windows unter `%ProgramData%\agent-workbench\halter\` mit Schreibrecht nur für Administratoren. Bis 22.11.2026 standen hier `schluessel_datei`, `kopf` und `wert_vorsatz`. **Der Name ist der Host (Abgleich 25.11.2026).** `halter.json` nennt jedes Modellziel nach dem Host des Netzziels; `halter_ziel` trägt darum genau den `host` dieses Eintrags. Ein anderer Wert macht die Zielliste ungültig (`zl.halter_name`), und der Start endet darauf in jedem Start mit Exit 78 `mw.ziel_halter_name` (Abschnitt 3a, Pflicht 1). alt (bis 25.11.2026): „(`zl.bindung`, Exit 78 `mw.ziele_ungueltig`), und unter setgid ist er Exit 78 `mw.ziel_halter_name`". Grund: Der Name ist damit bestimmt, und kein Aufrufer mit eigener Zielliste lenkt ein Geheimnis an einen anderen Host. alt: ein freier Name (Beispiel `copilot-github-api`). |
| `regeln[]` | 1 bis 32, je `methode` und `pfad` | `methode`: `GET`, `HEAD`, `POST`, `PUT`, `PATCH`, `DELETE`, `OPTIONS` oder `*` (jede davon); `pfad`: ein genauer Pfad, ein Teilbaum mit `/*` am Ende oder `/*` (jeder Pfad); Segmente aus `[A-Za-z0-9._~-]`, kein `.` und kein `..`, `*` nur als letztes Segment. **Ein gebundenes Ziel trägt nur `GET`, `POST` oder `*`, und `*` bedeutet dort genau `GET` und `POST`** (Abgleich 23.11.2026: `pfade` in `halter.json` kennt nur diese zwei Methoden, `rust/spec/halter/README.md:349`, und `modell.aufruf` nimmt nur sie an, `halter:372`). Eine andere Methode in einer Regel eines gebundenen Ziels macht die Zielliste ungültig (`zl.regel_form`, Exit 78 `mw.ziele_ungueltig`); Festlegung dieses Vertrags in der Richtung fail closed, weil der Halter sie ohnehin nicht tragen kann. Für ein gebundenes Ziel führt `halter.json` dieselbe Liste; beide entstehen aus `netzziele` derselben Route, und ein Unterschied ist ein Befund von `werkbank-pruefen` (H7, Abschnitt 5.4) |
| `upgrade` | `durchreichen` (Vorgabe für gebundene Ziele) oder `ablehnen` | was mit einem Upgrade (WebSocket) geschieht; ein ungebundenes Ziel hat immer `ablehnen` |

**Vorführ-Zielliste (Projektentscheid, 20.11.2026, aus den Messungen):**
- **`api.github.com`:** gebunden; nur `GET /copilot_internal/user` und `GET /copilot/mcp_registry` (Abgleich 25.11.2026; alt: `GET /copilot_internal/*`). Die Pfadregeln selbst gehören der Einstellungsdatei; dieser Vertrag zieht nur sein Beispiel nach. Ohne Pfadliste hätte der Agent über den Weiterreicher das volle Konto, denn ein Token aus dem Gerätefluss hat `repo`, `gist`, `read:org` und `codespace` (G).
- **`api.githubcopilot.com`:** gebunden; nur der gemessene genaue Pfad `GET /agents/swe/internal/memory/v0/enabled`, kein Teilbaum (Abgleich mit Nachtrag N1, 25.11.2026; alt: „Bis eine engere Liste feststeht, gilt `/*`.“). Grund: An einem Ziel mit Schlüssel steht kein Teilbaum (Einstellungsvertrag, Nachtrag N1; Halter-Vertrag, Abschnitt 4).
- **Wert von `endpoints.api`** (`api.<plan>.githubcopilot.com`; hier `individual`, gemessen im Plan Free): gebunden. Gemessen sind `GET /models`, `/mcp/readonly` (entfällt mit `--disable-builtin-mcps`), `/.well-known/*` und (Projektentscheid, 20.11.2026) `POST /auto`, `POST /models/session`, `POST /models/session/intent`, `POST /chat/completions` und `POST /agents/sessions`. Im zweiten angemeldeten Start (Projektentscheid, 20.11.2026, Copilot CLI 1.0.88) kam der Modellaufruf als WebSocket dazu: `GET /responses` mit Upgrade (101), ohne `permessage-deflate`, und einmal `POST /chat/completions`. `accept-encoding: identity` wirkte an allen Aufrufen. **Regeln (Abgleich mit Nachtrag N1, 25.11.2026):** nur die sieben genauen Pfade `GET /models`, `GET /responses`, `POST /auto`, `POST /models/session`, `POST /models/session/intent`, `POST /chat/completions` und `POST /agents/sessions`, kein Teilbaum. `/mcp/readonly` und `/.well-known/*` stehen nicht darin, weil die Methode nicht gemessen oder nur ein Teilbaum bekannt ist. Einen anderen Pfad lehnt der Halter mit `halter.pfad_verboten` ab und schreibt eine Kettenzeile mit Methode und Pfad (Halter-Vertrag, Abschnitt 4, „Modell-Host nur mit genauen Pfaden“, `rust/spec/halter/README.md:520`); der Modellweg lehnt ihn schon vorher mit 403 `mw.pfad_nicht_erlaubt` ab (Abschnitt 13.2, Stufe 3). Die Pfadregeln selbst gehören der Einstellungsdatei (`rust/spec/einstellungen/README.md`, Nachtrag N1); dieser Vertrag nennt nur das Beispiel. alt: „Die Regeln bleiben `/*`; eine engere Liste steht in Abschnitt 19 (OPEN 7).“
- **Wert von `endpoints.telemetry`:** ungebunden; nur `POST /telemetry`, ohne Authorization gemessen. Im Plan Free war der Host nicht auflösbar (NXDOMAIN, gemessen); ein Fehler dort blockiert nichts (Abschnitt 13.1).
- **`exp.individual.githubcopilot.com`:** ungebunden; nur `POST /api/v1/assignments` (genauer Pfad), ohne Authorization gemessen.
- **`github.com`** steht nicht in der Liste. Die Anmeldung läuft außerhalb des Einschlusses.
- **`GET /responses` (WebSocket)** beim Wert von `endpoints.api` ist gemessen (Projektentscheid, 20.11.2026, zweiter Start). Die Zielliste führt dafür die genaue Regel `GET /responses` (Nachtrag N1). alt: „Die Zielliste führt dafür keinen eigenen Eintrag; die Regel `*` `/*` des Ziels deckt ihn.“
- **WebSocket (Projektentscheid, 20.11.2026, Nachtrag 16):** an gebundenen Zielen `durchreichen`; an ungebundenen Zielen `ablehnen`. Wie der Weiterreicher die Upgrade-Anfrage und die Rahmen behandelt, steht in den Abschnitten 13.2 und 13.3.

Die Werte von `endpoints.*` für den eingesetzten Plan und die Ablage des Tokens sind Parameter dieser Datei.

**Prüfreihenfolge des Laders** (Start, Exit 78 `mw.ziele_ungueltig`; die Stufe steht in den Fällen):

| Stufe | Kennung in den Fällen | Verletzung |
|---|---|---|
| 1 | `zl.zu_gross` | mehr als 65536 Byte |
| 2 | `zl.json_ungueltig` | kein UTF-8, BOM, Syntaxfehler, doppelter Schlüssel auf jeder Ebene, oberste Ebene kein Objekt |
| 3 | `zl.schluessel_unbekannt` | ein Schlüssel außerhalb der Form, auf jeder Ebene |
| 4 | `zl.schluessel_fehlt` | ein Pflichtschlüssel fehlt (alle sind Pflicht) |
| 5 | `zl.typ_falsch` | falscher JSON-Typ; eine Ganzzahl nur als Zahlwort ohne Bruchteil und Exponent |
| 6 | `zl.fassung_unbekannt` | `fassung` ist nicht `1` |
| 7 | `zl.anzahl` | `ziele` oder `regeln` außerhalb der Grenzen |
| 8 | `zl.name_form` | `name` verletzt die Kennungsform |
| 9 | `zl.host_form` | `host` verletzt die Form |
| 10 | `zl.port_grenze` | `port` außerhalb von 1 bis 65535 |
| 11 | `zl.regel_form` | `methode` oder `pfad` einer Regel verletzt die Form, oder eine Regel eines gebundenen Ziels (`halter_ziel` nicht `null`) nennt eine Methode außer `GET`, `POST` und `*` (Abgleich mit dem Halter-Vertrag, 23.11.2026) |
| 12 | `zl.bindung`, `zl.halter_name` | **Je Ziel in der Reihenfolge der Liste** (Bau des sicheren Modus, 25.11.2026): erst `zl.bindung`, wenn eines der weggefallenen Felder `schluessel_datei`, `kopf` und `wert_vorsatz` noch dasteht, dann `zl.halter_name`, wenn `halter_ziel` nicht `null` und nicht genau der `host` dieses Ziels ist; das erste Ziel mit einem Fehler entscheidet. Nur auf `zl.halter_name` endet der Start mit `mw.ziel_halter_name` (Abschnitt 3a, Pflicht 1), auf jeden anderen Code der Zielliste mit `mw.ziele_ungueltig`. alt: ein Code `zl.bindung` für beide Fälle: `halter_ziel` ist nicht `null` und nicht genau der `host` dieses Ziels (Abgleich 25.11.2026; bis dahin: verletzt die Kennungsform), oder eines der weggefallenen Felder `schluessel_datei`, `kopf` und `wert_vorsatz` steht noch da. **Präzisierung des Baus (25.11.2026):** Die drei weggefallenen Felder bleiben in Stufe 3 **bekannt**, damit eine Liste, die eines davon neben `halter_ziel` noch trägt, diese Stufe nennt und nicht Stufe 3; `null` als Wert zählt dabei wie ein Wert. Eine Liste ohne `halter_ziel` nennt weiter Stufe 4 (`zl.schluessel_fehlt`), auch wenn sie ein weggefallenes Feld trägt: Ein Pflichtschlüssel fehlt dort. |
| 13 | `zl.upgrade_wert` | `upgrade` ist weder `ablehnen` noch `durchreichen`, oder ein ungebundenes Ziel hat `durchreichen` |
| 14 | `zl.ca_zulassung_wert` | `ca_zulassung` ist keiner der drei Werte |
| 15 | `zl.doppelt` | ein Name doppelt, `host` und `port` doppelt, oder ein `halter_ziel` doppelt. Weil `halter_ziel` genau der `host` ist (Stufe 12), trifft der dritte Fall nur zwei gebundene Ziele mit demselben Host auf verschiedenen Ports |

Eine gültige Liste mit `ca_zulassung` `offen` oder `abgelehnt` ergibt beim Start `mw.ca_nicht_zugelassen` (Abschnitt 13.5).

## 13. Verhalten

### 13.1 Tunnel

Der Weiterreicher spricht auf dem Socket HTTP/1.1 und nimmt nur `CONNECT` an. Prüfreihenfolge (keiner dieser Schritte öffnet eine Verbindung nach außen):

| Stufe | Code | Verletzung | Antwort |
|---|---|---|---|
| 1 | `mw.protokoll_fehlt` | das Anhängen einer Zeile ist vorher gescheitert | 503 |
| 2 | `mw.anfrage_unlesbar` | kein HTTP/1.1, Kopf ab 65536 Byte | 400 |
| 3 | `mw.ausgelastet` | schon 32 Tunnel offen | 429 |
| 4 | `mw.nur_connect` | die Methode ist nicht `CONNECT`, auch ein Proxy-Aufruf wie `GET http://…` | 405 |
| 5 | `mw.ziel_nicht_erlaubt` | das Ziel ist nicht genau `<host>:<port>` eines Eintrags der Zielliste (Groß- und Kleinschreibung zählt, der Port ist Pflicht, keine IP-Adresse) | 403 |
| 6 | `mw.ziel_nicht_erreichbar` | ungebundenes Ziel, das beim Start keine Adresse bekam | 502, Zeile `ziel_fehler` |
| 7 | — | sonst: `HTTP/1.1 200 Connection Established`, dann TLS als Server mit einem Blattzertifikat für `host` | |
| 8 | `mw.sni_falsch` | der TLS-Gruß des Harness nennt nicht genau `host` oder keinen Namen | TLS-Abbruch mit Alert, keine HTTP-Antwort, Zeile mit `status` `null` |

ALPN bietet der Weiterreicher nur `http/1.1` an. Im Tunnel kommen Anfragen nacheinander; jede wird für sich geprüft.

Stufe 3 zählt offene Tunnel, nicht Anfragen: Ein Tunnel zählt ab Stufe 3 bis zum Ende der Verbindung. Die Grenze von 32 liegt unter der Zahl der Verbindungsfäden (64), damit auch der 33. Tunnel einen Faden für seine 429 bekommt (fail closed). Grund der Zahl: Die CLI hält mehrere Verbindungen und einen Strom zugleich offen; am 24.11.2026 gemessen war die Grenze 8 zu klein. Betriebsart 1 bleibt bei 8 Anfragen (Abschnitt 10).

### 13.2 Anfragen im Tunnel

**Prüfreihenfolge (V).** Keiner dieser Schritte berührt das Ziel.

| Stufe | Code | Verletzung | Status |
|---|---|---|---|
| 1 | `mw.anfrage_unlesbar` | kein HTTP/1.1, Anfrageziel nicht in der Form `/…`, ein `#` im Anfrageziel, ein Abfrageteil über 1024 Zeichen oder außerhalb der Form von `abfrage` (Abschnitt 5.1), `Transfer-Encoding` außer genau `chunked`, `Transfer-Encoding` und `Content-Length` zugleich, `Content-Length` doppelt oder nicht dezimal | 400 |
| 2 | `mw.host_falsch` | `Host` fehlt, kommt doppelt oder ist nicht `host` beziehungsweise `host:port` des Tunnels | 400 |
| 3 | `mw.pfad_nicht_erlaubt` | der Pfad (vor `?`) ist unsicher (`//`, `\`, Segment `.` oder `..`, `%2e`, `%2f`, `%5c` in jeder Schreibweise), passt zu keiner Regel des Ziels oder verletzt an einem **gebundenen** Ziel die Form `^/[A-Za-z0-9._~/-]{0,511}$` des Halters (Abschnitt 5.1) | 403 |
| 4 | `mw.methode_falsch` | keine Regel mit passendem Pfad erlaubt die Methode; an einem **gebundenen** Ziel erlaubt keine Regel eine Methode außer `GET` und `POST` (Abschnitt 5.1) | 405 |
| 5 | `mw.upgrade_abgelehnt` | `Upgrade` oder `Connection: upgrade` an einem Ziel mit `upgrade` `ablehnen` | 403 |
| 6 | `mw.anmeldung_an_ungebundenes_ziel` | ein ungebundenes Ziel bekommt einen Kopf `Authorization` | 403 |
| 7 | `mw.koerper_zu_gross` | `Content-Length` über der Grenze des Ziels; bei `chunked` bricht der Weiterreicher beim Weiterreichen ab, sobald der Körper sie überschreitet. Die Grenze ist für ein **gebundenes** Ziel 24117248 Byte (23 MiB) und für ein ungebundenes 33554432 Byte | 413 |
| 8 | `mw.anfrage_unlesbar` | `Content-Length` ungleich der Länge des Körpers | 400 |
| 9 | `mw.halter_kein_schluessel` | gebundenes Ziel: der Halter meldet `halter.kein_schluessel` | 503 |
| 10 | `mw.zaehlung_fehler` | nur im Probelauf: die Zähldatei ist nicht schreibbar | 503 |

- Ein **Verstoß ist im Protokoll von einer erlaubten Anfrage unterscheidbar** (Projektentscheid, 20.11.2026, nach dem OpenShell-Versuch). Stufe 6 schreibt `kopf` `abgewiesen` und den Grund.
- Eine Regel passt, wenn ihr `pfad` `/*` ist, auf `/*` endet und der Pfad mit dem Teil davor samt `/` beginnt, oder genau der Pfad ist. Ein genauer Pfad deckt keinen Unterpfad (`/copilot/mcp_registry/x` passt nicht).
- Der Abfrageteil nach `?` geht unverändert mit; die Regeln prüfen nur den Pfad. Für ein gebundenes Ziel trennt der Weiterreicher ihn ab und gibt ihn als eigenes Feld `abfrage` an den Halter (Abschnitt 5.1, H4); für ein ungebundenes Ziel geht das Anfrageziel Byte für Byte hinaus.
- **Die Grenze von 23 MiB (H6).** Base64 wächst um ein Drittel; 24117248 Byte ergeben 32156332 Zeichen und passen mit Kopf und Rahmen in eine Halterzeile von 33554432 Byte. Der Halter nimmt keinen Körper in Teilen (entschieden im Halter-Vertrag, 23.11.2026), darum senkt Vertrag 8 die Grenze statt sie zu teilen.

**Anfrage an das Ziel (V; Köpfe am 20.11.2026).** Die Anfragezeile und der Körper gehen Byte für Byte unverändert hinaus, auch ein `chunked`-Körper mit seiner Rahmung. Die Köpfe bildet der Weiterreicher so:

1. Er behält die Köpfe des Harness in ihrer Reihenfolge und Schreibweise, an einem **ungebundenen** Ziel ausdrücklich `cookie` und andere Anmeldeköpfe wie `x-api-key`, dort auch `accept-encoding`.
   Er entfernt `authorization`, den Kopf `kopf` des Ziels, die Köpfe einer Verbindung (`connection`, `keep-alive`, `proxy-connection`, `te`, `trailer`, `upgrade`) und an gebundenen Zielen `accept-encoding`.
   **An einem gebundenen Ziel entfernt er zusätzlich die Namen, die der Halter verbietet** (Halter-Vertrag, Abschnitt 4, **31 Namen**): `authorization`, `proxy-authorization`, `x-api-key`, `api-key`, `cookie`, `set-cookie`, `host`, `content-length`, `transfer-encoding`, `connection`, `upgrade`, `te`, `trailer`, `keep-alive`, `proxy-connection`, `forwarded`, `x-forwarded-for`, `x-forwarded-host`, `accept-encoding`, `sec-websocket-extensions`, `x-http-method-override`, `x-http-method`, `x-method-override`, `x-original-method`, `x-original-url`, `x-original-uri`, `x-rewrite-url`, `x-forwarded-uri`, `x-forwarded-path`, `x-forwarded-prefix` und `x-envoy-original-path`. Acht davon fallen schon oben: `authorization`, `accept-encoding` und die sechs Köpfe einer Verbindung.
   **Zwei Wege, zwei Listen (Entscheid des Betreibers, 25.11.2026).**
   - **Halterweg** (`modell.aufruf` und `modell.strom`, ab Schritt I, Abschnitt 5.1): Es entfällt **jeder** der 31 Namen. Dort setzt der Halter `host`, die Rahmung des Körpers und den Kopf mit dem Token selbst; der Körper geht als `koerper_b64` in einem Stück.
   - **Direkter Weg** (vor Schritt I, Verbindung und Schlüssel beim Modellweg selbst, Abschnitte 5.2 und 6): Es entfallen dieselben Namen **außer `host`, `content-length` und `transfer-encoding`**. Diese drei braucht die Anfrage dort: Ohne `Host` fehlt ihr der Pflichtkopf, ohne die anderen zwei die Rahmung ihres Körpers. Der Modellweg behandelt `Host` und die Rahmung dort weiter nach den Regeln dieses Abschnitts (Stufe 2, Stufe 7, Stufe 8 und die Rahmung unten). Damit bleiben für den direkten Weg **20 Namen**.
   Die Namen schreibt er für ein gebundenes Ziel klein und sendet höchstens 64 Einträge (Abschnitt 5.1). Grund der Entfernung: Der Halter antwortete sonst `halter.kopf_verboten`, und die Anfrage endete mit 502 statt mit einer Antwort des Ziels. Die elf Namen ab `x-http-method-override` kamen am 25.11.2026 dazu: Mit ihnen ersetzt ein Proxy vor dem Ziel, der sie beachtet, Methode oder Pfad und umginge so die Pfad- und Methodenprüfung des Halters als letzte Sperre. Das ist enger als an einem ungebundenen Ziel, aber nie weiter.
   alt (23.11.2026 bis 25.11.2026): „zwölf Namen" für jeden Weg, `host`, `content-length` und `transfer-encoding` eingeschlossen; das brach die Anfrage des direkten Weges. alt (bis 23.11.2026): kein Name dieser Liste.
   Ausnahme: An einem Ziel mit `upgrade` `durchreichen` bleiben `upgrade` und `connection` einer Upgrade-Anfrage.
   **WebSocket (Nachtrag 16, W1):** Bei einer Upgrade-Anfrage an einem gebundenen Ziel entfernt er außerdem `sec-websocket-extensions` in jeder Schreibweise und mit jedem Wert, auch mehrfach. Ab Schritt I entfernt der Halter denselben Kopf noch einmal (H5, entschieden 23.11.); die Sperre gilt an beiden Stellen, wie die Pfadregeln nach H7. Grund wie bei `accept-encoding`: Eine ausgehandelte Kompression machte die Leck-Sperre blind. Der Kopf-Tausch (Punkt 4) gilt für die Upgrade-Anfrage wie für jede andere.
2. Nur an einem gebundenen Ziel hängt er `accept-encoding: identity` an. Grund: Die Leck-Sperre sieht in komprimierten Antworten nichts; jeder HTTP-Client nimmt unkomprimierte Antworten an. **Auf dem Halterweg setzt es der Halter** (Abschnitt 5.1): `accept-encoding` ist einer der 31 Namen, die dort entfallen, und der Modellweg sendet den Kopf nicht mit. Dieser Punkt gilt damit nur für den direkten Weg (Betriebsart 1 und, vor Schritt I, ein gebundenes Ziel). Für ein ungebundenes Ziel bleibt der Kopf des Harness unverändert.
3. Er hängt `connection: close` an, außer bei einer durchgereichten Upgrade-Anfrage. Grund: Je Anfrage im Tunnel öffnet er eine eigene Verbindung zum Ziel.
4. **Gebundenes Ziel:** Er hängt keinen Anmeldekopf an. Die Köpfe aus den Punkten 1 bis 3 gehen als `kopf` von `modell.aufruf` an den Halter; der setzt `<kopf>: <wert_vorsatz><Token>` mit den Werten aus seiner `halter.json` selbst ein (Abschnitt 5.1). **Ungebundenes Ziel:** Es bekommt nie einen Schlüssel, und die Anfrage geht wie bisher direkt hinaus.
5. Der Kopf `connection: close` aus Punkt 3 geht nicht an den Halter: Die Verbindung nach außen gehört ihm. Für ein gebundenes Ziel entfällt Punkt 3.

### 13.3 Antworten im Tunnel

- Status und Körper gehen Byte für Byte an den Harness, auch Fehler des Dienstes.
- **Köpfe:** alle Köpfe des Dienstes außer `connection`, `keep-alive`, `proxy-connection`, `te`, `trailer`, `upgrade`, `transfer-encoding`, `content-length`, `set-cookie` und `proxy-authenticate`. Die Reihenfolge und Schreibweise bleiben.
  Bei einer durchgereichten Umstellung (101) bleiben `upgrade` und `connection`. Für ein **gebundenes** Ziel kommen sie ab Schritt I nicht vom Halter: Seine Zeile `beginn` trägt sie nicht, weil sie auf seiner Sperrliste stehen; der Modellweg setzt beide zum Harness selbst (`rust/spec/halter/README.md:428`).
  Die Sperrliste des Halters ist um fünf Namen länger als die hier (`authorization`, `proxy-authorization`, `x-api-key`, `api-key`, `cookie`). Für ein gebundenes Ziel ab Schritt I gilt darum seine Liste; diese hier gilt für ein ungebundenes Ziel und für Betriebsart 1.
- **Rahmung:** Hatte der Dienst eine `Content-Length`, nennt der Weiterreicher dieselbe. Sonst sendet er `transfer-encoding: chunked`. Die Bytes des Körpers bleiben gleich, und der Tunnel bleibt für die nächste Anfrage offen. **Für ein gebundenes Ziel ab Schritt I gilt immer `chunked`** (Abschnitt 5.1): Der Halter sendet weder `content-length` noch `transfer-encoding`, beide stehen auf seiner Sperrliste.
- **Wer die Rahmen eines gebundenen Ziels prüft (ab Schritt I, Nachtrag 22).** Für ein gebundenes Ziel läuft die Umstellung über die Op `modell.strom` des Halters (Abschnitt 5.1, H5). Dann prüft **der Halter** die Rahmen des Dienstes, entfernt die Erweiterung, führt die Leck-Sperre über die Rahmengrenzen und gibt die Nutzdaten als Zeilen `teil` weiter; der Modellweg setzt daraus die Bytes an den Harness zusammen und prüft sie nicht noch einmal. Die Regeln W2 bis W5 unten beschreiben dieselben Entscheidungen; für ein gebundenes Ziel trifft sie der Halter, und der Modellweg bildet seine Codes nach Abschnitt 5.1 ab (`halter.ziel_komprimiert` wird `mw.antwort_komprimiert`, `halter.leck_gesperrt` wird `mw.halter_fehler`). Das gilt auch für die drei Regeln von RFC 6455 Abschnitt 5, die W4 fail closed verlangt (Steuerrahmen mit FIN=0 oder Länge über 125, Fortsetzung ohne offene Nachricht, neuer Datenrahmen bei offener Nachricht): Der Halter prüft sie am Rahmenkopf, und jeder Verstoß ist bei ihm ein Abriss mit `halter.ziel_fehler`, ohne weiteres `teil` und ohne Zurückgehaltenes (`rust/spec/halter/README.md:435` bis `:447`, Nachtrag Rahmen). Der Modellweg antwortet dann nach Abschnitt 5.1 mit `mw.halter_fehler`. Die Rahmen der Richtung Harness zum Ziel prüft der Halter nicht; W2 bis W5 betreffen nur die Rahmen des Dienstes.
  **Folge für die sichtbaren Fälle (Nachführung des Baus, 25.11.2026).** Die Fälle, die eine Entscheidung über die Rahmen des Dienstes oder die Leck-Sperre an einem gebundenen Ziel prüften, gehören damit dem Halter-Vertrag und stehen in dessen Fällen und Außentests (L6 bis L9). In `faelle/kopf-websocket.json` bleiben für den Modellweg die Zusagen, die seine sind: die Bytes jeder Zeile `teil` gehen unverändert und in ihrer Reihenfolge an den Harness, `upgrade` und `connection` setzt er selbst, die Bytes des Harness gehen als `strom.teil` hinaus, und jeder Code des Halters wird nach Abschnitt 5.1 abgebildet. Die Gruppe `antworten` trägt dafür jetzt das Feld `halter` (`beginn`, `teile`, `ende`) statt der Bytes des Dienstes; **kein Fall ist weggefallen, dessen Zusage noch beim Modellweg liegt.** Am Ende des Stroms gehen die zurückgehaltenen Bytes der Leck-Sperre beim Halter als letztes `teil` vor `ende` hinaus, wie W3 es verlangt; bei einem Abriss, einem Treffer und `halter.ziel_komprimiert` gehen sie nicht hinaus (`rust/spec/halter/README.md:448` bis `:453` und `:610` bis `:613`). Für ein **ungebundenes** Ziel gibt es keine Umstellung: Es hat immer `upgrade` `ablehnen` (Abschnitt 12). Die Zählung im Probelauf bleibt beim Modellweg, weil nur er die Bytes des Harness sieht (Abschnitt 9, Z1 bis Z3).
- **WebSocket (Projektentscheid, 20.11.2026, Nachtrag 16 vom 20.11.2026; fail closed).** Antwortet ein gebundenes Ziel mit 101, gilt Folgendes. Die Fälle stehen in `faelle/kopf-websocket.json`.
  - **Keine Erweiterung (W2).** Trägt die 101 einen Kopf `sec-websocket-extensions` (jede Schreibweise, jeder Wert), kommt statt der 101 die Antwort 502 `mw.antwort_komprimiert`. Der Weiterreicher schließt die Verbindung zum Ziel, vom Strom geht nichts an den Harness. Die Zeile hat das Ergebnis `ziel_fehler`. `permessage-deflate` handelt der Weiterreicher nie aus: Die Upgrade-Anfrage trägt keinen Erweiterungskopf (Abschnitt 13.2, W1), und eine Erweiterung des Dienstes bricht ab.
  - **Rahmen lesen (W3).** Nach der 101 liest der Weiterreicher die Rahmen des Dienstes nach RFC 6455 Abschnitt 5.2 (Kopf mit FIN, RSV1 bis RSV3, Opcode, Maskenbit und der Länge in der 7-, 16- oder 64-Bit-Form) und gibt sie unverändert weiter. Die Grenzen der gelesenen Teile (TCP-Segmente) haben keine Wirkung.
  - **Die Sperre läuft auf den Nutzdaten, nicht auf den rohen Bytes (W3).**
    - Datenrahmen (Opcode `0x1`, `0x2` und Fortsetzung `0x0`): eine Sperre über die Nutzdaten aller Datenrahmen in der Reihenfolge, über Rahmen- und Nachrichtengrenzen hinweg. Ein Schlüssel, der über zwei Rahmen verteilt ist, wird so erkannt, auch wenn Rahmenköpfe dazwischen stehen.
    - Steuerrahmen (`0x8` Close, `0x9` Ping, `0xA` Pong): jeder für sich geprüft. Ein Steuerrahmen zwischen zwei Teilen einer Nachricht unterbricht die Sperre der Datenrahmen nicht und trägt nichts in sie hinein.
    - Ein Rahmen geht erst hinaus, wenn die Sperre über seine Nutzdaten gelaufen ist. Wie im Antwortstrom (Abschnitt 7) hält sie die letzten (Länge des längsten Schlüssels − 1) Nutzdaten-Bytes zurück. Weil die Reihenfolge der Bytes bleibt, wartet alles hinter dem ersten zurückgehaltenen Byte mit: der Rest des Rahmens, spätere Rahmenköpfe und Steuerrahmen. Beim Ende der Verbindung geht alles Zurückgehaltene hinaus.
  - **Abriss (W4, W5).** Abriss heißt: beide Richtungen schließen, keine weiteren Bytes an den Harness; Zurückgehaltenes geht nicht hinaus. Ein Abriss entsteht bei:
    - gesetztem RSV1, RSV2 oder RSV3 in einem Rahmen des Dienstes: `mw.antwort_komprimiert` (Ergebnis `ziel_fehler`);
    - gesetztem Maskenbit, einem anderen Opcode als `0x0`, `0x1`, `0x2`, `0x8`, `0x9`, `0xA` oder einer Länge über 33554432 Byte, geprüft schon am Rahmenkopf: `mw.ziel_nicht_erreichbar` (Ergebnis `ziel_fehler`); die Prüfung der RSV-Bits kommt zuerst;
    - einem Rahmen, der RFC 6455 Abschnitt 5 verletzt (Nachtrag 2, fail closed): `mw.ziel_nicht_erreichbar` (Ergebnis `ziel_fehler`), geprüft nach der RSV-Prüfung, sobald der Verstoß feststeht, also am Rahmenkopf. Das sind ein Steuerrahmen (`0x8`, `0x9`, `0xA`) mit FIN=0 oder mit einer Länge über 125, eine Fortsetzung (`0x0`) ohne offene geteilte Nachricht und ein neuer Datenrahmen (`0x1` oder `0x2`), während eine geteilte Nachricht offen ist (FIN=0 kam, die abschließende Fortsetzung fehlt);
    - dem Schlüssel in den Nutzdaten (W3): `mw.leck_gesperrt` (Ergebnis `leck_gesperrt`). Erst kommt die Zeile, dann das Schließen, wie bei einem späten Leck im Antwortstrom.
  - **Ende der Verbindung mitten im Rahmen (Nachtrag 2):** Ein beim Ende der Verbindung abgeschnittener Rahmen ist kein Abriss. Es geht alles hinaus, was die Sperre gesehen hat, auch das Zurückgehaltene und ein unvollständiger Kopf; eine Zeile entsteht wie beim normalen Ende.
  - **Richtung Harness zum Dienst (W6):** Byte für Byte unverändert. Der Harness hatte nie den Schlüssel; der Weiterreicher prüft diese Richtung nicht. **Ausnahme nur im Probelauf (Abschnitt 9, Z1 bis Z3):** Er liest und zählt die Rahmen und verwirft nicht lesbare; die Bytes bleiben auch dort unverändert.
- **Leck-Sperre:** Sie gilt für jeden weitergegebenen Kopf und für den Körper.
- **Komprimierte Antworten (Projektentscheid, 20.11.2026, fail closed):** Trägt die Antwort eines gebundenen Ziels `content-encoding` mit einem anderen Wert als `identity`, kommt statt der Antwort 502 `mw.antwort_komprimiert` (`Antwort komprimiert`). Ab Schritt I verwirft sie schon der Halter mit `halter.ziel_komprimiert` (H11); der Status an den Harness bleibt derselbe.
  Ein ungebundenes Ziel hat nie einen Schlüssel bekommen; seine komprimierte Antwort geht unverändert durch, die Köpfe prüft die Leck-Sperre trotzdem.

### 13.4 Protokollzeile

```
{"aktionsname":"modellweg.aufruf","betriebsart":"kopf-einsetzen","ergebnis":"abgelehnt","fassung":2,"grund":"mw.anmeldung_an_ungebundenes_ziel","host":"telemetry.individual.githubcopilot.com","kopf":"abgewiesen","methode":"POST","pfad":"/telemetry","pfadregel":"/telemetry","route":"cloud-beispiel","sitzung":"s-1","status":403,"vorher":"<64 hex>","zeit":"2026-11-20T16:00:00.000Z","ziel":"copilot-telemetrie"}
```

Je Tunnel-Ablehnung und je Anfrage im Tunnel entsteht eine Zeile. Felder der Betriebsart (16 Felder mit denen aus Abschnitt 8):

| Feld | Wert |
|---|---|
| `ziel` | `name` des Ziels aus der Zielliste; `null`, wenn der Tunnel zu keinem Ziel passt |
| `methode` | Methode der Anfrage, wenn sie eine der sieben bekannten ist; sonst `null` |
| `pfadregel` | `pfad` der Regel, die gepasst hat (etwa `/copilot_internal/user`), nie der Pfad der Anfrage; `null`, wenn keine passte |
| `kopf` | `halter` (gebundenes Ziel, der Halter setzt den Kopf ein), `keiner` (ungebundenes Ziel ohne `Authorization`), `abgewiesen` (ungebundenes Ziel mit `Authorization`, nur mit `mw.anmeldung_an_ungebundenes_ziel`) oder `null` (die Anfrage kam nicht bis zur Bindung). Bis 22.11.2026 hieß der erste Wert `ersetzt`. **Erledigt (25.11.2026):** Das Register einer Zeile der Fassung 2 nimmt `halter` jetzt an (Vertrag 5, Abschnitt 6.7; `rust/crates/formate/src/kette/pruefen.rs:425`), und der Bau trägt ihn an genau einer Stelle (`kopf_einsetzen::KOPF_GEBUNDEN`). Ein Wert außerhalb des Registers bleibt abgewiesen, und eine Zeile außerhalb des Registers wird nicht angehängt. alt: „Der Bau trägt darum weiter `ersetzt`“. |
| `pfad` | der begrenzte Pfad der Anfrage (Abschnitt 8) bei `abgelehnt` und bei `GET`, sonst `null` |
| `host` | `host` des Ziels, dem der Tunnel zugeordnet ist, klein und ohne Port; `null` bei einer Ablehnung vor der Zuordnung. Er steht genau dort, wo `ziel` steht |

- Bei `weitergereicht` ist `kopf` `halter` oder `keiner`.
- Der Code des Halters steht **nie** im Feld `grund`: Dort steht der eigene Code des Modellwegs. Den Code des Halters nennt die Zeile nicht; er enthält keinen Wert, aber er gehört dem Halter und steht in dessen Kette (Halter-Vertrag, Abschnitt 9).
- `status` ist `null` nur bei `mw.sni_falsch`.
- **`host` neben `ziel` (Projektentscheid, 22.11.2026).** `ziel` bleibt der Name aus der Zielliste. `host` steht dazu, damit ein Name wie `ziel-N` nicht aus der Reihenfolge der Liste hergeleitet werden muss.
  Beide Felder sind zugleich gesetzt oder zugleich `null`; der Host kommt aus der geprüften Zielliste (Abschnitt 12), nie aus der Anfrage.
- Fehlerfolge: `betriebsart`, `route`, `sitzung`, `ziel`, `methode`, `pfadregel`, `kopf`, `ergebnis`, `grund`, `status`, `pfad`, `host`, `vorher`; danach `zeit_ungueltig`.
- Gründe je Ergebnis:
  - `abgelehnt`: die Codes der Abschnitte 13.1 und 13.2 außer `mw.protokoll_fehlt` und `mw.halter_kein_schluessel`;
  - `ziel_fehler`: `mw.halter_kein_schluessel`, `mw.halter_fehler`, `mw.halter_frist`, `mw.ziel_nicht_erreichbar`, `mw.ziel_frist`, `mw.antwort_komprimiert`;
  - `leck_gesperrt`: `mw.leck_gesperrt`.
- **WebSocket (Nachtrag 16, W7):** Keine neue Zeilenform, kein neues Feld, kein neuer Code. Die Zeile einer durchgereichten Umstellung entsteht wie bei jeder Antwort erst am Ende: beim Ende der Verbindung (Ergebnis `weitergereicht`, `status` 101) oder beim Abriss (Ergebnisse und Gründe wie in Abschnitt 13.3; `status` 101). Eine 101, die wegen einer Erweiterung durch 502 ersetzt wurde, bekommt ihre Zeile sofort mit `status` 502. Im Probelauf kommen als Gründe eines Abrisses `mw.zaehlung_fehler` und `mw.anfrage_unlesbar` hinzu (beide Ergebnis `abgelehnt`, `status` 101; Abschnitt 9). Eine Nachricht im Strom bekommt keine eigene Zeile.
- Es gibt keine Zählung im Dienst (Projektentscheid, 20.11.2026), außer im Probelauf (Abschnitt 9). Der Zähler für die Gegenprobe sitzt in der Attrappe des Ziels.

### 13.5 Aufruf und Start

```
modellweg --betriebsart kopf-einsetzen --socket <pfad> --socket-rechte 0600 --sitzung <sitzung> --route <route> --protokoll <ordner> \
          --zielliste <datei> --ca-schluessel <datei> --ca-zertifikat <datei>
```

- Alle drei Dateien sind Sitzungsdateien von „Lauf starten“ (0600, eigen, Ordner 0700 außerhalb des Einschlusses). Die Pfade sind absolut.
- **Den Endpunkt des Halters gibt es nicht als Option (Abgleich 25.11.2026).** Er ist der feste Pfad aus Abschnitt 5.1; der Halter lebt länger als die Sitzung und legt seinen Socket selbst an (Halter-Vertrag, Abschnitt 2.3, „Socket“: derselbe feste Pfad, `XDG_RUNTIME_DIR` liest der Halter nie; OPEN 31 erledigt; alt: „dort heute noch unter `$XDG_RUNTIME_DIR`; den festen Pfad zieht der Halter-Vertrag nach, OPEN 31“). `--halter-socket` ist unter Linux in jeder Betriebsart `mw.aufruf` (Exit 64); unter Windows und macOS bleibt die Form des Endpunkts offen (Abschnitt 17a.9, Frage 2). alt: „Die Option ist in Betriebsart 2 Pflicht und in Betriebsart 1 `mw.aufruf`"; Grund der Änderung: Unter setgid lenkte ein Argument des Aufrufers die Anfragen an eine fremde Gegenstelle (Abschnitt 3a, Pflicht 2).
- `--ziel`, `--modell` und `--schluessel-datei` gehören zu Betriebsart 1 und sind hier `mw.aufruf`; ebenso die frühere Form `--ca`.
- **Stufe 0 (Abschnitt 3, F1)** gilt vor Schritt 1 und damit vor allem, was hier steht; sie ist für beide Betriebsarten dieselbe.
- **Schritt 2 des Starts:**
  1. Zielliste nach Abschnitt 12 lesen (`mw.ziele_ungueltig`; für `zl.halter_name` `mw.ziel_halter_name`, Abschnitt 3a, Pflicht 1).
  2. `ca_zulassung` muss `zugelassen` sein (`mw.ca_nicht_zugelassen`, Exit 78). Fail closed, ohne Rückweg.
  3. CA prüfen (`mw.ca_ungueltig`, Exit 78):
     - beide Dateien eigen, 0600, keine Verknüpfung, genau ein Schlüssel und ein Zertifikat, die zueinander passen;
     - `CA:TRUE` mit `pathlen` 0;
     - Name Constraints: die erlaubten DNS-Namen sind genau die Menge aller Hosts der Zielliste, gebunden und ungebunden (`ca_erlaubte_namen`); ein fehlender oder ein zusätzlicher Name ist `mw.ca_ungueltig`; ausgeschlossen sind die IP-Bereiche `0.0.0.0/0` und `::/0`;
     - Laufzeit höchstens 24 h, und sie hat begonnen.
- **Schritt 3 (Halter, Abschnitt 5.1):** für jedes `halter_ziel` der gebundenen Ziele, in der Reihenfolge der Liste: zum Halter verbinden (`mw.halter_fehlt`), die Gegenseite prüfen (`mw.halter_unecht`) und `schluessel.vorhanden` mit dem Feld `ziel` gleich `halter_ziel` rufen, nie mit `name` (Halter-Vertrag, Abschnitt 4, `schluessel.vorhanden`, `rust/spec/halter/README.md:530`). Je Frage eine eigene Verbindung, wie im Betrieb (Abschnitt 5.1). Abbildung der Antwort, jeweils Exit 78:
  - `{"vorhanden": true}`: weiter mit dem nächsten Ziel; nach dem letzten weiter mit Schritt 4.
  - `{"vorhanden": false}` und `halter.ziel_unbekannt`: `mw.halter_kein_schluessel`. Der Halter unterscheidet beide (Konfiguration gegen fehlenden Schlüssel); der Start endet in beiden Fällen gleich, weil kein Geheimnis eingesetzt werden kann. Der Code des Halters steht nur in der zweiten Zeile `mw.halter_grund: <Code des Halters>` (Abschnitt 4, stderr-Zeile; Abschnitt 5.1), und nur in der Form `^halter\.[a-z_]{1,40}$`: für `halter.ziel_unbekannt` ist das `mw.halter_grund: halter.ziel_unbekannt`, für `{"vorhanden": false}` gibt es keine zweite Zeile (alt: „Der Code des Halters steht nirgends in der Ausgabe.“ Grund: Der Halter schreibt diese Ablehnung nicht ins Journal; ohne den Code blieb die Ursache beim Start unbekannt).
  - jeder andere Code nach der Abbildung aus Abschnitt 5.1: `halter.kein_schluessel` wird `mw.halter_kein_schluessel`, jeder andere Code `mw.halter_fehler`, darunter `halter.rolle` (alt: „Eine Antwort `false`, `halter.ziel_unbekannt` oder `halter.rolle` beendet den Start mit `mw.halter_kein_schluessel`“; Grund: `halter.rolle` ist ein Fehler der Rolle, kein fehlender Schlüssel, und Abschnitt 5.1 bildet ihn auf `mw.halter_fehler` ab), `halter.schluessel_fehlt`, `halter.wert_form` (etwa ein `halter_ziel` über 64 Zeichen, das der Halter als Kennung nicht annimmt) und `halter.intern`.
  - eine nicht lesbare Zeile, eine Antwort ohne `vorhanden`, eine Zeile anderer Art oder ein Abriss der Verbindung: `mw.halter_fehler`, ohne zweite Zeile.
  - keine Antwort binnen der Frist: `mw.halter_frist`, ohne zweite Zeile.
  - **Zweite Zeile (Festlegung des Baus, 25.11.2026):** Bei `mw.halter_fehler` und `mw.halter_kein_schluessel` nach einem Code des Halters schreibt der Modellweg nach der Zeile `<code>: <grund>` genau eine Zeile `mw.halter_grund: <Code des Halters>` auf stderr, wie bei `mw.kernabbild_grund`. Der Code kommt nur in die Zeile, wenn er `^halter\.[a-z_]{1,40}$` entspricht; sonst gibt es keine zweite Zeile, und nie steht ein anderer Teil der Antwort darin. Exitcode 78 und die erste Zeile bleiben gleich. Beispiel: `mw.halter_fehler: Halter lehnte ab` und `mw.halter_grund: halter.rolle`.
  **Frist (Festlegung des Baus, 25.11.2026):** 5 s für alle Fragen von Schritt 3 zusammen, gemessen ab dem Beginn des Schritts; ist sie vorbei, geht keine weitere Frage hinaus. Grund: „Lauf starten“ wartet 10 s auf die Bereitschaftszeile (Vertrag 1, Schritt 7a); eine längere Frist endete dort ungesehen mit `lauf.modellweg_nicht_bereit`. Die Frist von 600 s aus Abschnitt 5.1 gilt für den Kopf einer Antwort im Betrieb, nicht für diese Frage.
  Fail closed: Der Start geht nur weiter, wenn der Halter für **jedes** gebundene Ziel `true` antwortet; ohne Antwort startet er nicht. Eine Zielliste ohne gebundenes Ziel überspringt Schritt 3; der Halter wird dann nicht gebraucht.
  **Stand des Baus (25.11.2026): gebaut** (`rust/crates/modellweg/src/start.rs`, `halter_bereit`; `rust/crates/modellweg/src/halter.rs`, `schluessel_da`, `schluessel_da_mit_code` und `FRIST_START`; die zweite Zeile: `start.rs`, `Abbruch` und `abbruch_melden`), geprüft gegen den echten Halter (`halter` als Abhängigkeit der Tests) und gegen eine Attrappe (`tests/halterweg.rs`, Modul `schritt_drei`, und `step_three_asks_the_real_holder_by_target`). alt: „Gebaut sind das Verbinden und die Prüfung der Gegenseite; beide Codes fahren. Die Frage je Ziel ist **nicht** gebaut: `schluessel.vorhanden` des Halters nimmt nur das Feld `name` […]. Der Bau rät darum nicht; die Frage steht in Abschnitt 20. `mw.halter_kein_schluessel` fährt im Betrieb (503), wenn der Halter `halter.kein_schluessel` meldet.“ Der letzte Satz gilt weiter.
- **Schritt 6:** jeden Host der **ungebundenen** Ziele einmal auflösen. Einen gebundenen Host löst der Modellweg nicht auf; dessen Adresse steht nur in `halter.json`, und ein Fehler dort ist `mw.halter_fehler` im Betrieb, nicht `mw.ziel_unaufloesbar` beim Start.
  Ein ungebundenes Ziel ohne Adresse blockiert nichts: Der Weiterreicher startet, und ein CONNECT an dieses Ziel bekommt 502 `mw.ziel_nicht_erreichbar` mit einer Zeile `ziel_fehler` (gemessen: Telemetrie im Plan Free mit NXDOMAIN).
  `mw.ziel_unaufloesbar` (Exit 69) bleibt der Code der Betriebsart 1 und entfällt in Betriebsart 2.
- Die Blattzertifikate erzeugt der Weiterreicher im eigenen Speicher, je Host höchstens für die Laufzeit der CA.

## 14. Gegenprobe E5 und Mutanten

**E5** ist der Name der Gegenprobe im Drehbuch.

**Aufbau (Projektentscheid, 20.11.2026):** Weg aus Abschnitt 12 mit dem echten Weiterreicher und einer Attrappe als Ziel. Das echte Token ist eine Marke, die nur der Weiterreicher kennt (im Außentest im `gho_`-Format; in den Fällen bewusst nicht tokenförmig).
Im Einschluss sucht eine Probe als Kind der CLI nach der Marke:
- in `env`, `/proc/*/environ` und `/proc/*/cmdline`;
- per `grep -r` über `/`, das Heim, `/tmp` und `/run`;
- mit `secret-tool lookup service copilot-cli` und `gh auth token`;
- über `/proc/<cli-pid>/mem` (EPERM erwartet).

**Erwartung:** 0 Treffer, und der Zähler der Attrappe steigt (die Marke kommt dort an). **Gegenprobe ohne Einschluss:** Mit der Marke direkt in der Umgebung findet die Probe 1 Treffer (wie Messung v1).

**Mutanten** (jeder macht E5 rot) und ihre Zuordnung:

| Mutant | Eingriff | rot, weil | Zuordnung |
|---|---|---|---|
| 1 | das echte Token in der Umgebung des Einschlusses | Treffer in `env` und `/proc/*/environ` | Einbindung |
| 2 | `config.json` mit der Marke eingebunden | Treffer per `grep -r` | Einbindung |
| 3 | D-Bus-Socket des Rechners eingebunden | Treffer mit `secret-tool` | Einbindung |
| 4 | ein angemeldetes `gh` erreichbar | Treffer mit `gh auth token` | Einbindung |
| 5 | Pfadliste für `api.github.com` fehlt (Regel `*` `/*`) | `GET /user/repos` gelingt | Weiterreicher |

**Fälle aus dem OpenShell-Versuch (Projektentscheid, 20.11.2026), Zuordnung Weiterreicher:**
- Der Schlüssel geht nur an gebundene Ziele (`kopf-anfragen.json`, Fälle `telemetrie`, `telemetrie_cookie`, `telemetrie_api_key`).
- Ein erlaubtes, ungebundenes Ziel mit Authorization-Kopf bekommt 403 und eine Zeile mit `kopf` `abgewiesen` (`telemetrie_mit_anmeldung`, Zeile `abgewiesen` in `kopf-zeile.json`).
- Methode und Pfad werden durchgesetzt (`user_repos`, `github_post`, `registry_unterpfad`, `punkt_segment`, `kodiert`, `doppelter_strich`, `telemetrie_get`).

**Mutanten des Weiterreichers für WebSocket (Nachtrag 16, M-W7 Nachtrag 2):** Der Erzeuger `bau-faelle.mjs` baut sie mit `BAU_MUTANT=<Name>` und `--pruefen` ein; er schreibt dann nie Dateien. Jeder bricht den Erzeuger mit Exit 1 ab.

| Mutant | Eingriff | rot, weil | Fall |
|---|---|---|---|
| M-W1 | `sec-websocket-extensions` bleibt am gebundenen Ziel (seit 25.11.2026 steht der Name in der Liste der verbotenen Kopfnamen; alt: „bleibt in der Upgrade-Anfrage") | die Bytes an das Ziel tragen den Kopf | `ws_erweiterung_entfernt`, `ws_erweiterung_schreibweisen` |
| M-W2 | eine 101 mit Erweiterung geht durch | der Fall erwartet 502 `mw.antwort_komprimiert` | `ws_erweiterung_101`, `ws_erweiterung_101_schreibweise` |
| M-W3 | Sperre über die rohen Bytes statt über die Nutzdaten | die Rahmenköpfe zwischen den Teilen verdecken die Marke | `ws_marke_geteilt`, `ws_marke_geteilt_ping` |
| M-W4 | ein Steuerrahmen setzt die Sperre der Datenrahmen zurück | der Ping trennt die zwei Teile der Marke | `ws_marke_geteilt_ping` |
| M-W5 | RSV1 wird nicht geprüft | der Rahmen mit RSV1 geht durch | `ws_rsv1` |
| M-W6 | der Kopf-Tausch fehlt in der Upgrade-Anfrage | die Marke steht nicht in den Bytes an das Ziel | `upgrade_durchgereicht`, `ws_erweiterung_entfernt`, `ws_anfrage_tausch` |
| M-W7 | die Regeln für Steuer- und Fortsetzungsrahmen fehlen | der regelwidrige Rahmen geht durch | `ws_ping_fin0`, `ws_ping_laenge_126`, `ws_fortsetzung_ohne_anfang`, `ws_text_in_geteilter` |

**Mutanten der Zählung in WebSocket-Rahmen (Nachtrag 17, Z1 bis Z3; `BAU_MUTANT=<Name>` wie oben):** Jeder bricht den Erzeuger mit Exit 1 ab; die Fälle stehen in `faelle/probelauf-websocket.json`.

| Mutant | Eingriff | rot, weil | Fall |
|---|---|---|---|
| M-Z0 | die Richtung Harness zum Ziel wird nicht gezählt | der Marker im Rahmen bleibt bei 0 | `ws_marker_textrahmen` (erster Fall), auch alle anderen mit Marker |
| M-Z1 | Zählung auf den maskierten Bytes statt auf den entmaskierten | die maskierten Bytes tragen den Marker nicht | `ws_marker_textrahmen` |
| M-Z2 | Zählung je Rahmen statt über die Rahmengrenzen | der geteilte Marker zählt nie | `ws_marker_geteilt_zwei_rahmen`, `ws_marker_ein_und_geteilt`, `ws_marker_ueber_nachrichten`, `ws_ping_zwischen_teilen`, `ws_marker_ohne_ueberlappung` |
| M-Z3 | Steuerrahmen zählen mit | der Marker im Ping oder Close zählt | `ws_marker_im_ping`, `ws_marker_im_close`, `ws_ping_zwischen_teilen` |
| M-Z4 | ein nicht lesbarer Rahmen geht im Probelauf durch | der Rahmen geht hinaus statt des Abrisses | `ws_maske_fehlt`, `ws_rsv1`, `ws_rsv3`, `ws_opcode_3`, `ws_laenge_zu_gross`, `ws_ping_fin0`, `ws_ping_laenge_126`, `ws_fortsetzung_ohne_anfang`, `ws_text_in_geteilter` |

**Mutanten des Feldes `pfad` (23.11., `BAU_MUTANT=<Name>` wie oben):** Jeder bricht den Erzeuger mit Exit 1 ab.
M-P1 kürzt ein zu langes Segment statt `*` (gemessen: `pfad segment_33` rot); M-P2 entfernt die Abfrage nicht (gemessen: `pfad abfrage` rot);
M-P3 setzt das Feld auch bei einer weitergereichten Anfrage ohne `GET` (gemessen: `faelle/antworten.json` verschieden).

**Mutanten der Umgebungsprüfung (F1; `BAU_MUTANT=<Name>` wie oben):** M-F1a (Betriebsart 2 prüft nicht) macht `kopf-einsetzen_ssl_cert_file` rot; M-F1b (eine leere Variable gilt als nicht gesetzt) macht `openai-lokal_leer_ssl_cert_file` rot; M-F1c (`OPENSSL_CONF` fehlt in der Liste) macht `openai-lokal_openssl_conf` rot.

**Mutanten der Suche nach dem Schlüssel (Nachtrag 18; `BAU_MUTANT=<Name>` wie oben):** Jeder bricht den Erzeuger mit Exit 1 ab; die Fälle stehen in `faelle/suche.json`.

| Mutant | Eingriff | rot, weil | Fall |
|---|---|---|---|
| M-S1 | Verknüpfungen werden verfolgt | der Verweis auf die Schlüsseldatei zählt als Fund, und eine Verknüpfung als `--in` wird durchsucht | `nur_verknuepfung`, `in_ist_verknuepfung` |
| M-S2 | nicht lesbare Dateien zählen als ohne Fund | Exit 0 statt 3 | `nicht_lesbar_ohne_fund`, `nicht_lesbarer_ordner`, `in_fehlt`, `in_ist_verknuepfung`, `aufruf_16_in` |
| M-S3 | nur der erste Schlüssel der Zielliste wird gesucht | der Fund des zweiten Schlüssels fehlt | `zielliste_zwei_schluessel_fund_zweiter` |
| M-S4 | ein Fund über eine Lesegrenze zählt nicht | der Block endet mitten im Schlüssel | `fund_lesegrenze_4096`, `fund_lesegrenze_65536` |

`faelle/kopf-gegenprobe.json` hält das Modell von E5: je Aufbau die erwarteten Fundorte, das Ergebnis von `GET /user/repos` und ob E5 rot oder grün sein muss. Der echte Außentest läuft in der VM (Abschnitt 15, M12).

---

# Teil D: Prüfung, Bau, Ausblick

## 15. Gegenproben und Außentests

Die sichtbaren Fälle (Abschnitt 16) prüfen das Verhalten ohne Netz. Gegen das echte Ziel und mit einem echten Schlüssel läuft **kein** Test; jeder Außentest nutzt eine Attrappe des Ziels.
M1 bis M11 gelten für `openai-lokal`, M12 bis M15 für `kopf-einsetzen` (M1, direkt nach dem Bau von `openai-lokal`).

| Kennung | Zusage | Außentest (VM mit Ubuntu 24.04 LTS, Attrappe des Ziels auf dem Rechner) | Gegenprobe |
|---|---|---|---|
| M1 | Der Agent liest die Schlüsseldatei nicht | Im Einschluss: `cat` auf den Pfad der Datei, Suche nach der Marke des Testschlüssels in allen lesbaren Dateien, in `/proc/*/environ` und `/proc/*/cmdline`. Erwartet: 0 Funde, Pfad nicht vorhanden | Datei in den Einschluss gebunden: Fund 1 |
| M2 | Der Agent erreicht den Modellweg-Prozess nicht | Signal, `/proc/<pid>/environ`, `/proc/<pid>/mem`, `ptrace` aus dem Einschluss: alles scheitert | ohne eigenen PID-Raum: das Signal gelingt |
| M3 | Anderes Modell, anderer Pfad, andere Methode abgelehnt | je ein Aufruf; Zähler der Attrappe: 0 | Prüfung von `model` aus: Zähler 1 |
| M4 | Authorization des Harness wird ersetzt | Harness sendet `Bearer platzhalter`; die Attrappe sieht genau den Testschlüssel und nie `platzhalter` | Kopf durchgereicht: Attrappe sieht `platzhalter` |
| M5 | Antwortkopf und Fehlerkörper enthalten keinen Schlüssel | die Attrappe spiegelt den empfangenen Kopf in Körper, Kopf und Fehler, einmal über eine Teilgrenze verteilt; Marke in allen Bytes an den Harness: 0 | Leck-Sperre aus: Marke gefunden |
| M6 | Fehler des Ziels unverändert | 403 `policy_model_blocked`, 401, 500 mit Körpern der Attrappe: Status und Körper an den Harness gleich Byte für Byte | Umformung an: Abweichung |
| M7 | `reasoning` und SSE unverändert | Antwort mit `reasoning`, SSE mit 100 Ereignissen: Bytes gleich | |
| M8 | TLS nur mit dem Truststore des Systems | Attrappe mit Zertifikat einer eigenen Test-CA, die CA im Truststore des Systems: Verbindung gelingt; CA entfernt: `mw.ziel_nicht_erreichbar` | |
| M9 | Ziel nur Host und Port, keine Umleitung, keine neue Auflösung | Attrappe antwortet 302 auf ein zweites Ziel: Zähler des zweiten Ziels 0; Name nach dem Start umgebogen: Verbindungen nur an die erste Adresse | |
| M10 | Zeile und Zähldatei ohne Inhalt | 20 Aufrufe mit Marken in Prompt und Antwort: Marken in der Kette 0; `kette pruefe` grün; die Zähldatei enthält nur Zahlen | |
| M11 | lokale Harnesses | pi und opencode gegen die Attrappe (`reasoning`, `tool_calls`, 401, 403, chunked); Messung | |
| M12 | Token im Einschluss nicht auffindbar (Betriebsart 2) | Gegenprobe E5 nach Abschnitt 14 | Gegenprobe ohne Einschluss und Mutanten 1 bis 5 |
| M13 | Schlüssel nur am gebundenen Ziel, Verstoß sichtbar | Copilot CLI gegen Attrappen aller fünf Ziele; die Attrappen von Telemetrie und exp sehen nie `authorization` und nie die Marke; ein Aufruf mit `authorization` an exp ergibt 403 und eine Zeile `abgewiesen`; ein gebundenes Ziel sieht `accept-encoding: identity`; eine komprimierte Antwort eines gebundenen Ziels ergibt 502 | Bindung an exp: Marke dort gefunden |
| M14 | CA nur im Einschluss, Name Constraints | der Schlüssel der CA ist im Einschluss nicht auffindbar; ein Blattzertifikat für einen Host außerhalb der Liste wird vom Einschluss abgelehnt; die Blätter aller Ziele der Liste, auch der ungebundenen, nimmt die CLI an; das Bundle des Rechners bleibt unverändert | CA ohne Name Constraints: Blatt für fremden Host angenommen |
| M15 | ohne zugelassene CA kein Start und kein Rückweg | Zielliste mit `ca_zulassung` `offen` und `abgelehnt`: Exit 78 `mw.ca_nicht_zugelassen`, die Cloud-Sitzung startet nicht; im Einschluss ist das Token in keinem Fall auffindbar; E4 cloud steht im Probelauf auf „offen“ | `ca_zulassung` nicht geprüft: Start gelingt |

## 16. Sichtbare Fälle

`faelle/` entsteht mit `node rust/spec/modellweg/bau-faelle.mjs` (Mac oder Linux-Baurechner, leicht, ohne Abhängigkeit). Das Skript nimmt die Bytes aus `the previous implementation` und den Vektor `k1` aus `the previous implementation`.
Ein eigenes Referenzmodell im Skript rechnet jeden Fall nach. Weicht es ab, bricht das Skript ab und schreibt nichts.
Die Zeilen rechnet es zweimal, mit `jcs` und mit fester Feldfolge von Hand. `--pruefen` vergleicht die Dateien Byte für Byte mit einem neuen Lauf.
Schlüssel, Marke und Platzhalter in den Fällen sind erfundene Testwerte.

| Datei | Teil | Inhalt |
|---|---|---|
| `faelle/anfragen.json` | B | 27 Fälle (Abschnitt 10): rohe HTTP-Anfrage; erwartet die Bytes an das Ziel oder Status, Körper und Felder der Zeile ohne Berührung des Ziels |
| `faelle/antworten.json` | B | 11 Fälle (Abschnitte 7 und 11): Antwort des Ziels; erwartet die Antwort an den Harness (`antwort`, `ersetzt`, `abgerissen`) und die Felder der Zeile samt Tokens |
| `faelle/aufruf.json` | A, B | 30 Fälle für argv und 13 für die Schlüsseldatei (Abschnitte 4, 5 und 10) |
| `faelle/zeile.json` | A, B | 25 Fälle für die Zeile der Betriebsart 1 und 2 Ketten mit der Ausgabe von `kette pruefe` (Abschnitte 8 und 11) |
| `faelle/probelauf.json` | A | 16 Fälle für die Probelauf-Optionen, eine Folge von 5 Anfragen mit dem Inhalt der Zähldatei danach, 8 Fälle für die Startzeile und 4 Startfolgen mit stdout, stderr und angehängten Bytes (Abschnitt 9) |
| `faelle/probelauf-websocket.json` | C | 27 Fälle für die Zählung der Rahmen vom Harness zum Ziel im Probelauf: gezählte Marker, geteilte Marker, Steuerrahmen, ohne Probelauf, nicht lesbare Rahmen, nicht schreibbare Zähldatei, abgeschnittene Rahmen (Abschnitt 9, Nachtrag 17) |
| `faelle/suche.json` | A | 47 Fälle für die Suche nach dem Schlüssel (Abschnitt 9a, Nachtrag 18): Dateibaum, Aufruf, Zielliste und erwartete Ausgabe samt Exit |
| `faelle/umgebung.json` | A | 34 Fälle, je 17 für beide Betriebsarten, für die Stufe 0 des Starts (Abschnitte 3 und 4, F1) |
| `faelle/kopf-ziele.json` | C | 37 Fälle für die Zielliste samt Zulassung der CA und erwartetem Start (Abschnitt 12) |
| `faelle/kopf-tunnel.json` | C | 15 Fälle für CONNECT und SNI, dazu ein ungebundenes Ziel ohne Adresse (Abschnitt 13.1) |
| `faelle/kopf-anfragen.json` | C | 41 Fälle für Anfragen im Tunnel mit den Bytes an das Ziel, mit den gemessenen Kopfnamen (Abschnitt 13.2) |
| `faelle/kopf-antworten.json` | C | 18 Fälle für Antworten im Tunnel samt WebSocket und Kompression (Abschnitt 13.3) |
| `faelle/kopf-websocket.json` | C | 3 Upgrade-Anfragen, 9 Antworten über die Zeilen des Halters und 2 Fälle für die Richtung vom Harness (Abschnitte 13.2 und 13.3, Nachtrag 16) |
| `faelle/kopf-zeile.json` | C | 22 Fälle für die Zeile der Betriebsart 2 und 27 Fälle für das Feld `pfad` in beiden Formen (Abschnitte 8 und 13.4) |
| `faelle/kopf-aufruf.json` | C | 7 Fälle für argv der Betriebsart 2, gültig ab deren Bau (Abschnitt 13.5) |
| `faelle/kopf-gegenprobe.json` | C | E5: Basis, Gegenprobe und die 5 Mutanten (Abschnitt 14) |

Die Fälle decken ab:
- **Anfragen, Betriebsart 1:** gültige Anfrage; `Bearer platzhalter` ersetzt; fremde Köpfe verworfen; Strom mit `Accept`; Körper Byte für Byte; Ausweichziel Ollama ohne TLS.
- **Modell:** anderes, fehlendes, doppeltes `model`, andere Schreibweise.
- **Pfad und Methode:** `GET /v1/models`, `/v1/completions`, Abfrageteil, Punkt-Segment; GET und PUT.
- **Rahmung:** zu groß; chunked (vorläufig 411); ohne Länge; doppelte oder falsche Länge; HTTP/1.0; kein JSON; Liste; ausgelastet; Protokoll gescheitert.
- **Antworten, Betriebsart 1:** `reasoning` und SSE Byte für Byte; ohne `usage`; 403 `policy_model_blocked` und 401 unverändert; 302 ohne `Location`; Leck im Fehlerkörper, über eine Teilgrenze, spät im Strom, im `content-type`; viele kurze Teile.
- **Aufruf:** Betriebsart als Vorgabe, ausdrücklich, unbekannt, nicht gebaut, doppelt, vor der Vollständigkeit; Optionsfehler; Socket; Ziel; Schlüsseldatei;
  `--protokoll-pfad-voll` allein, vorn, doppelt, mit Wert und in Betriebsart 2.
- **Probelauf:** alle drei Optionen, Schalter vorn, nur `--probelauf`, Marker ohne `--probelauf`, eine fehlt, doppelt, mit Wert, ungültige Marker; Zählung über zwei angenommene Anfragen; eine abgelehnte Anfrage zählt nicht; Zähldatei nicht schreibbar, dann geht nichts hinaus; Startzeile Byte für Byte; Protokoll oder Zähldatei nicht schreibbar, dann kein Start.
- **Zählung in WebSocket-Rahmen (Nachtrag 17):** ein Marker in einem Textrahmen, in einem Binärrahmen, in der 16-Bit-Längenform, zwei Marker in einem Rahmen; ein Marker über zwei Rahmen einer Nachricht, über zwei Nachrichten, byteweise geliefert; ein Marker in einem Rahmen und ein anderer über zwei Rahmen (Pflichtfall); Marker im Ping und im Close zählen nicht, ein Ping zwischen zwei Teilen unterbricht nichts; Zählung ohne Überlappung; ohne Probelauf keine Zählung und unveränderte Bytes auch bei nicht lesbaren Rahmen; Rahmen ohne Maskenbit, mit RSV1, RSV3, Opcode `0x3`, Länge über der Grenze, Ping mit FIN=0 oder Länge 126, Fortsetzung ohne Anfang, Textrahmen in einer geteilten Nachricht: Abriss `mw.anfrage_unlesbar`; Zähldatei nicht schreibbar beim zweiten und beim ersten Datenrahmen: Abriss `mw.zaehlung_fehler`; nur Steuerrahmen schreiben nichts; abgeschnittener Rahmen und abgeschnittener Kopf am Ende der Verbindung.
- **Umgebung beim Start (F1):** jede der drei Variablen allein, leer, zwei zugleich, vor einem ungültigen Aufruf; keine gesetzt und ähnliche Namen (`SSL_CERT_FILES`, `OPENSSL_CONFX`): der Start geht weiter.
- **Betriebsart 2:** Zielliste in 30 Formen, darunter Zulassung `offen` und `abgelehnt` (gültig, Start verweigert), Teilbaum `/*`, 33 Regeln, ungebundenes Ziel mit `durchreichen`; CONNECT an erlaubte und fremde Ziele, ohne Port, Port 80, Großschreibung, IP, Proxy-Form, falsche und fehlende SNI, ungebundenes Ziel ohne Adresse; 31 Tunnel offen (der nächste geht durch) und 32 offen (der nächste bekommt 429).
- **Anfragen im Tunnel:** Platzhalter gegen Marke getauscht, `accept-encoding: identity` am gebundenen Ziel, alle anderen gemessenen Köpfe unverändert; exp-Ziel ohne `authorization` weitergereicht mit unverändertem `accept-encoding`, mit `authorization` abgewiesen, anderer Pfad abgelehnt; genaue Regeln an allen Zielen mit Schlüssel (Nachtrag N1: der Unterpfad `GET /models/x`, der fremde Pfad `POST /v1/fremd` und die falsche Methode `POST /models` am Plan-Host, ein nicht gemessener Pfad an `api.githubcopilot.com`); ein Teilbaum prüft seit N1 nicht mehr ein Fall der Vorführ-Liste, sondern der Test `a_subtree_rule_covers_the_paths_below_it` mit `POST /api/v1/*` an einem ungebundenen Ziel (alt: „genaue und Teilbaum-Regeln“); `GET /user/repos` abgelehnt; Methode durchgesetzt; Unterpfad einer genauen Regel; `..`, `%2e`, `//`; falscher Host; chunked durchgereicht, samt dem Kopf `transfer-encoding` (direkter Weg); WebSocket am gebundenen Ziel durchgereicht, am ungebundenen abgelehnt; Telemetrie ohne Schlüssel, mit Cookie und `X-Api-Key` weitergereicht, mit `Authorization` abgewiesen; zu groß; `X-Http-Method-Override` und `X-Original-Url` am gebundenen Ziel entfernt (`plan_methodenkopf`), derselbe Kopf am ungebundenen Ziel unverändert (`telemetrie_methodenkopf`); am gebundenen Ziel auf dem direkten Weg bleiben `Host` und `Content-Length`, und `Cookie` fällt weg (`plan_rahmung_bleibt`).
- **Antworten im Tunnel:** Köpfe des Dienstes ohne `set-cookie`; chunked neu gerahmt; gzip am gebundenen Ziel verworfen, am ungebundenen durchgereicht, `identity` gelesen; 101 mit `upgrade` und `connection`; Leck in Körper, Kopf, spät im Strom und in einem WebSocket-Rahmen.
- **WebSocket (Nachtrag 16):** Erweiterungskopf der Upgrade-Anfrage entfernt (zwei Schreibweisen), Kopf-Tausch in der Upgrade-Anfrage; 101 mit Erweiterung durch 502 ersetzt; die Marke über zwei Teile einer Nachricht, mit einem Ping dazwischen, in einem Ping und in einem Close; RSV1, RSV2, RSV3, Maskenbit, Opcode `0x3` und eine Länge über der Grenze; die 126- und die 127-Länge ohne Marke; ein Präfix der Marke am Ende (alle Bytes kommen an); ein Ping dazwischen ohne Marke; regelwidrige Rahmen (Ping mit FIN=0, Ping mit Länge 126, Fortsetzung ohne Anfang, Textrahmen in einer geteilten Nachricht); abgeschnittener Rahmen am Ende (alles kommt an); Rahmen vom Harness unverändert.
- **Feld `pfad` (Abschnitt 8):** ein Segment, zwei Segmente, drei und mehr (nur zwei bleiben); Abfrage und Fragment entfernt; Segment mit 32 und mit 33 Zeichen; Prozentkodierung; Zeichen außerhalb; leeres Segment und Schrägstrich am Ende; `*` und absolute URI; die Wurzel `/`;
  `GET` mit Erfolg (Pfad gesetzt), `POST` mit Erfolg (Pfad `null`), Ablehnung mit Pfad; `--protokoll-pfad-voll` mit drei Segmenten, mit einem Zeichen außerhalb und mit einem Pfad über 512 Zeichen; `host` gesetzt und `null`.
- **Zeilen:** Byte für Byte samt Grenzen und gemischter Kette 1g → 2.

**Neue Fälle für den Halterweg (Abschnitt 5.1; beschrieben, noch nicht erzeugt).** Dieser Stand ändert nur den Vertragstext. `node rust/spec/modellweg/bau-faelle.mjs --pruefen` bleibt rc 0, weil `faelle/` unverändert ist; die Fälle der alten Bindung tragen noch `schluessel_datei`, `kopf` und `wert_vorsatz`. Mit dem Entschluss über Abschnitt 5.4 kommen sie in einem eigenen Schritt nach:
- `faelle/kopf-ziele.json`: jedes Ziel der 30 Fälle führt `halter_ziel` statt der drei alten Felder. Neu dazu: `bindung_halter_ziel_form` (`halter_ziel` ist nicht der `host` des Ziels, Stufe 12), `bindung_altes_feld` (ein Ziel trägt noch `schluessel_datei`, Stufe 12 statt Stufe 3), `bindung_ungebunden_mit_halter_ziel` (ungebundenes Ziel mit `durchreichen` und `halter_ziel`, Stufe 13), `doppelt_halter_ziel` (zwei Ziele mit demselben `halter_ziel`, Stufe 15).
- `faelle/kopf-aufruf.json`: `halter_socket_argument` (`--halter-socket` in Betriebsart 2, `mw.aufruf`) und `halter_socket_argument_eins` (dieselbe Option in Betriebsart 1, `mw.aufruf`). alt (bis 25.11.2026): `halter_socket_fehlt`, `halter_socket_in_eins` und `halter_socket_form` zu einer Pflichtoption.
- Nach den Entscheidungen im Halter-Vertrag (Nachtrag 22) kommen dazu: `faelle/kopf-anfragen.json` mit einem Anfrageziel mit Abfrageteil (erwartet `pfad` ohne `?` und das Feld `abfrage`), eines mit `#` (400 `mw.anfrage_unlesbar`), eines mit einer Abfrage über 1024 Zeichen (400) und je ein Körper von 24117248 und 24117249 Byte an einem gebundenen Ziel (weitergereicht beziehungsweise 413 `mw.koerper_zu_gross`) sowie 33554432 Byte an einem ungebundenen Ziel (weitergereicht); `faelle/suche.json` mit einer gültigen Liste mit `halter_ziel` und Antworten von `modell.suche` (Exit 0 bei nur Nullen, Exit 1 bei einem Treffer des Halters, Exit 3 bei `nicht_lesbar` des Halters, Exit 78 `mw.schluessel_fehlt` ohne Halter oder ohne Geheimnis).
- Neue Datei `faelle/kopf-halter.json`: die Zeilen zum Halter und seine Antworten Byte für Byte. Gültige Anfrage mit `kopf`, Feld `abfrage` und Körper; eine Umstellung über `modell.strom` mit Rahmen in beiden Richtungen; eine 101 mit Erweiterung als `halter.ziel_komprimiert` zu 502 `mw.antwort_komprimiert`; `beginn`/`teil`/`ende` zur Antwort an den Harness zusammengesetzt; `halter.kein_schluessel` → 503 `mw.halter_kein_schluessel`; `halter.rolle`, `halter.pfad_verboten` und `halter.leck_gesperrt` → je 502 `mw.halter_fehler` (der Code des Halters geht nicht hinaus); Abriss nach `beginn` → Abriss zum Harness; keine Zeile `beginn` binnen der Frist → 504 `mw.halter_frist`; `schluessel.vorhanden` `false` beim Start → Exit 78 `mw.halter_kein_schluessel`.
- Nach dem Abgleich mit dem gemergten Halter-Vertrag (23.11.2026) kommen dazu: in `faelle/kopf-anfragen.json` eine Anfrage an ein gebundenes Ziel mit `PUT` (405 `mw.methode_falsch`), eine mit Prozentkodierung im Pfad (403 `mw.pfad_nicht_erlaubt`) und eine mit den Köpfen `Cookie` und `X-Api-Key` (erwartet: beide Köpfe fehlen in `kopf` von `modell.aufruf`, die Namen der übrigen klein); in `faelle/kopf-ziele.json` eine Liste mit `PUT` in einer Regel eines gebundenen Ziels (Exit 78 `mw.ziele_ungueltig`, Stufe `zl.regel_form`); in `faelle/kopf-halter.json` eine Umstellung mit Zeilen `strom.teil` und `strom.ende` unter der `id` des Stroms, eine Op bei offenem Strom (`halter.strom` → 502 `mw.halter_fehler`) und eine 101 des Halters ohne `upgrade` und `connection` (erwartet: der Modellweg setzt beide zum Harness selbst).
- `faelle/kopf-zeile.json`: die 12 Fälle tragen `kopf` `halter` statt `ersetzt`; neu `zeile_halter_fehler` und `zeile_halter_frist` mit Ergebnis `ziel_fehler`.
- `faelle/suche.json`: neu `zielliste_nur_halter_ziel` (gültige Liste mit `halter_ziel`, erwartet Exit 78 `mw.schluessel_fehlt`, Abschnitt 9a S1).

**Neue Fälle für Windows und macOS (Abschnitt 17a; beschrieben, noch nicht erzeugt).** Sie kommen mit dem Bau auf diesen Systemen, nicht mit diesem Stand. Die Fälle ohne System (Form der Zeile, Prüfreihenfolge) lassen sich schon auf dem Prüfrechner rechnen; alles an Pipe, Sitzungsport und Dienstkonto braucht die VM.
- Neue Datei `faelle/teile.json`: die Zeilen zwischen Prüfteil und Dienstteil Byte für Byte. Gültige Anfrage mit `kopf` und Körper; die Antwort des Dienstteils als Strom; eine Ablehnung des Dienstteils nach B2 zu 502 `mw.teile_weichen_ab` mit genau einer Zeile; ein Abriss der Pipe zu 502 `mw.dienstteil_fehler`; ein fremder Protokollordner zu `mw.protokoll_fremd`.
- `faelle/kopf-aufruf.json`: `dienstteil_fehlt` (der Prüfteil erreicht die Pipe nicht, Exit 78) und `dienstteil_unecht` (der Besitzer der Pipe-Instanz ist nicht die Dienst-SID, Exit 78).
- Neue Datei `faelle/eingang.json`: Schein und Peer am Eingang (Abschnitt 17a.3): richtiger Schein, falscher Schein (403 `mw.schein_falsch`), fehlender Schein, Peer einer anderen Sitzung (403 `mw.peer_fremde_sitzung`), keine Antwort des Plattformdienstes binnen 500 ms.
- `faelle/kopf-zeile.json`: je eine Zeile des Prüfteils und des Dienstteils für dieselbe Anfrage schließen sich aus; die Zeilenform bleibt unverändert (Abschnitt 17a.5).

## 17. Bau: Vorgabe und Vorschlag

**Vorgabe (`rust/deny.toml`):** Erlaubt sind heute nur MIT und Apache-2.0 und ihre OR-Formen. `ring` braucht ISC und ist damit heute nicht erlaubt; damit fallen auch die üblichen Wege über `rustls` heraus.

**Vorschlag (H, Lizenzen vor dem Bau mit `./gates.sh licenses` zu messen):**

| Teil | Wahl | Lizenz laut Paketangabe (nicht gemessen) |
|---|---|---|
| TLS als Client | `native-tls`: unter Linux System-OpenSSL, unter Windows SChannel, unter macOS Security.framework; jedes nutzt den Truststore des Systems | `native-tls` MIT OR Apache-2.0 |
| Linux darunter | `openssl` (Apache-2.0), `openssl-sys` (MIT), `openssl-probe`, `foreign-types`, `openssl-macros`, `bitflags`, `cfg-if`, `libc`, `once_cell` (je MIT OR Apache-2.0); OpenSSL 3 selbst ist Apache-2.0 und wird dynamisch gelinkt, kein Crate | wie links |
| Windows darunter | `schannel` (MIT), `windows-sys` (MIT OR Apache-2.0) | wie links |
| TLS als Server und Blattzertifikate (Betriebsart 2) | `openssl` direkt (SSL-Server mit SNI-Rückruf, `X509` mit Name Constraints); `native-tls` deckt das nicht ab | wie Linux oben |
| HTTP | eigenes kleines HTTP/1.1 über `std::net` und den TLS-Strom, ohne `hyper` und ohne Laufzeit für asynchronen Code; je Anfrage oder Tunnel ein Thread | keine neue |
| JSON | `serde_json`, schon im Arbeitsbereich | MIT OR Apache-2.0 |
| Namensauflösung | `std::net::ToSocketAddrs` (getaddrinfo des Systems) | keine neue |

Die Liste der erlaubten Lizenzen ändert nur das Projekt. Zeigt der Lauf eine andere Lizenz im Baum, entscheidet das Projekt vor dem Bau.
Hinweis für den Bau: Die Betriebsart ist ein eigener Typ (Aufzählung). Prüfung der Anfrage und Felder der Zeile hängen an ihm, damit Teil C ohne Änderung an Teil B dazukommt.

## 17a. Windows und macOS (Nachtrag 21, 23.11.2026)

**Geltung.** Dieser Abschnitt ist ein Nachtrag zum Halterweg (Abschnitt 5.1). Er beschreibt, was unter Windows und unter macOS anders ist; Teil A bis Teil C gelten unter Linux unverändert.
Gebaut ist unter keinem der beiden Systeme etwas: Fassung 1 des Durchstichs läuft nur unter Linux (Abschnitt 2). Der Abschnitt ist Vertragstext, kein Bauauftrag.
Quelle sind der Baustein E2 des Windows-Bauplans mit den Bausteinen B1 bis B4 und der Pflichtpunkt für macOS (beide Projektentscheid, 21.11.2026). Die Abstimmung mit dem Halter- und Paket-Vertrag steht vor dem Bau des Starters unter Windows noch aus (OPEN 20).

### 17a.1 Die Bausteine, wörtlich

- Unter Windows teilt sich der Modellweg in einen Prüfteil als Person und einen Dienstteil unter `NT SERVICE\wb-modellweg`.
- **B1:** Schlüssel und Leck-Sperre nur im Dienstteil oder beim Halter. Nach Weg A heißt das: beim Halter.
- **B2:** Der Dienstteil prüft alle Regeln von Vertrag 8 selbst noch einmal. Er verlässt sich nie auf den Prüfteil.
- **B3:** Die DACL der Pipe schließt die Package-SID des AppContainers aus. Der Dienstteil liest die SID des Aufrufers aus der Pipe, nie aus einer Angabe.
- **B4:** Die Protokollzeile schreibt der Teil, der entscheidet.
- **macOS:** Der Schlüsselteil liegt beim Halter unter einem eigenen Dienstkonto (`_wbhalter`). Keine Person mit macOS bekommt vor Fassung 2 einen Schlüssel, bevor das gilt.

**Offene Folge der Entscheidung im Halter-Vertrag, 23.11.2026.** Der Anmeldetoken der Cloud-Route ist seit H14 ein Geheimnis der Person (Abschnitt 5.4). Unter Windows ist der Aufrufer des Halters der Dienstteil unter eigenem Dienstkonto; ihm gehört kein Geheimnis der Person. Solange der Halter-Vertrag nicht entschieden hat, ob der Dienstteil als Vertreter der Person gilt, trägt die Teilung nach E2 den Halterweg unter Windows nicht (Frage 8 unten, OPEN 23). Alles Übrige dieses Abschnitts gilt unabhängig davon.

**Warum die Teilung unter Windows nötig ist und unter macOS nicht.** Die Rolle `bruecke` des Halters folgt unter Windows allein aus der SID des Aufrufers (Halter-Vertrag, Abschnitt 2.2). Ein Prozess der Person bekommt dort die Rolle `oberflaeche`, und die darf `modell.aufruf` nicht rufen. Ein Modellweg, der nur als Person läuft, erreicht den Halterweg unter Windows also nicht. Unter macOS folgt die Rolle aus der Signatur des Programms (Halter-Vertrag, Abschnitt 2.4); ein signiertes Rollenprogramm unter der Kennung der Person ist dort als `bruecke` zugelassen. Darum braucht macOS kein zweites Programm des Modellwegs, sondern den Halter unter eigenem Konto.

### 17a.2 Windows: Prozesse und Konten

| Teil | Konto | Zahl | gestartet von | hält |
|---|---|---|---|---|
| Prüfteil | die Person, ohne Adminrechte | einer je Sitzung | „Lauf starten" (Vertrag 1), Schritt „Modellweg starten" | Zielliste, CA-Schlüssel, CA-Zertifikat, Protokollordner, Zähldatei; keinen Schlüssel |
| Dienstteil | virtuelles Dienstkonto `NT SERVICE\wb-modellweg`, ohne Adminrechte | einer je Maschine | die Dienststeuerung beim Systemstart | die Verbindung zum Halter; keinen Schlüssel (B1, Weg A) |
| Halter | `NT SERVICE\wb-halter` | einer je Maschine | die Dienststeuerung | Token, Adresse, Kopfnamen und die Leck-Sperre (Halter-Vertrag, Abschnitt 2.2) |

- **Kein Teil des Modellwegs hält einen Tokenwert.** Nach Weg A setzt der Halter den Kopf selbst ein (Abschnitt 5.1). Der Dienstteil ist nur der Träger der Dienst-SID, die der Halter als Rolle `bruecke` annimmt, und die zweite Sperre nach B2.
- Der Dienstteil lebt länger als eine Sitzung. Er kennt weder Zielliste noch Route noch Sitzung aus seiner eigenen Konfiguration; beides nennt der Prüfteil je Anfrage, und der Dienstteil prüft es gegen die Angaben, die er vom Halter und aus seiner Installationsdatei hat (B2).
- **Zwei Betriebsarten, zwei Prozesse.** Der Einschluss unter Windows nennt den Prozess der Betriebsart 1 „Modellweg" und den der Betriebsart 2 „Ausgang" (`rust/spec/windows-einschluss/README.md`, Begriffe). Beide sind dasselbe Programm mit `--betriebsart`; eine Sitzung mit Cloud-Zielen hat beide. Hergeleitet, mit dem Halter-Vertrag zu bestätigen (OPEN 20).
- **Lebensdauer.** `PR_SET_PDEATHSIG` gibt es unter Windows nicht. An seine Stelle tritt ein eigenes Job-Objekt des Prüfteils mit `KILL_ON_JOB_CLOSE`, dessen Handle nur der Starter hält, wie beim Such-Prozess (`rust/spec/windows-einschluss/README.md`, „Lebensdauer"). Der Prüfteil liegt nie im Job des Agenten. Hergeleitet. Der Dienstteil endet nur mit dem Dienst.
- **Abschirmung gegen ein Speicherabbild (Schritt 1).** `kernabbild::abschirmen` hat unter Windows keine Form: `RLIMIT_CORE` und `PR_SET_DUMPABLE` gibt es dort nicht. Festlegung dieses Nachtrags: Das Crate antwortet unter Windows „nicht anwendbar", der Start läuft weiter, und der Prüfteil schreibt die ASCII-Zeile `mw.kernabbild_hinweis` nach Abschnitt 4. Grund: Nach B1 hält kein Teil des Modellwegs ein Geheimnis, und ein fail closed sperrte hier die ganze Route, ohne einen Wert zu schützen. Was unter Windows gegen ein Speicherabbild des **Halters** gilt, entscheidet der Windows-Plan (Halter-Vertrag, Abschnitt 2.1).

### 17a.3 Windows: Kanal und Peer-Prüfung

Drei Kanäle, alle aus `peerkanal` (`rust/spec/peerkanal/README.md`):

| Kanal | Art | Server | Client | Peer-Prüfung |
|---|---|---|---|---|
| Harness → Prüfteil | TCP auf `127.0.0.1` und `[::1]`, Port des Weiterleiters dieser Sitzung | Prüfteil | der Harness im AppContainer, über den Weiterleiter | nicht `peerkanal`: Schein je Verbindung, dazu `peer` mit `tcp` beim Plattformdienst (17a.4) |
| Prüfteil → Dienstteil | Named Pipe `\\.\pipe\wb-modellweg` | Dienstteil | Prüfteil | `peerkanal`: SID aus dem impersonierten Token; Client prüft den Besitzer der Pipe-Instanz |
| Dienstteil → Halter | Named Pipe `\\.\pipe\wb-halter` | Halter | Dienstteil | `peerkanal`: der Halter liest die Dienst-SID des Modellwegs; der Dienstteil prüft den Besitzer der Pipe-Instanz |

- **Der Eingang des Harness bleibt TCP.** Eine Named Pipe kann der Harness als Basisadresse nicht nutzen, und für den Proxy der Betriebsart 2 gilt dasselbe (`rust/spec/windows-einschluss/README.md`, „Entscheidung: TCP für Modellweg und Ausgang"). `--socket` und `--socket-rechte` aus Abschnitt 4 haben unter Windows keine Wirkung; an ihre Stelle tritt der Sitzungsport des Weiterleiters. Die Form der Option ist offen (OPEN 20).
- **Die Pipe des Dienstteils nach B3.** Sie entsteht mit `FILE_FLAG_FIRST_PIPE_INSTANCE`, Byte-Modus, `PIPE_REJECT_REMOTE_CLIENTS`. Besitzer ist die Dienst-SID. Die DACL ist geschützt und hat genau drei Arten von ACEs: `SYSTEM` und die Dienst-SID mit Vollzugriff, je berechtigtes Konto einer Person Lesen und Schreiben ohne `FILE_CREATE_PIPE_INSTANCE`. **Keine ACE für `Everyone`, `Authenticated Users`, `ALL APPLICATION PACKAGES` oder eine AppContainer-SID.** Gemessen ist für die Pipe des Halters, dass ein Agent im AppContainer sie damit nicht öffnet (Fehler 5, Windows-Probe 18.11.); für diese Pipe ist es hergeleitet (Außentest MW-W2).
- **Die SID kommt aus der Pipe, nie aus einer Angabe (B3).** Der Dienstteil impersoniert den Client und liest `TokenUser` und `TokenAppContainerSid`; ein Token mit Restricting SIDs weist `peerkanal` selbst ab. Eine Anfrage, die ein Konto, eine Sitzung oder eine Route behauptet, ändert daran nichts. Gemessen: Die SID aus dem impersonierten Token war in 100 von 100 Verbindungen richtig (Windows-Probe 18.11.).
- **Was der Dienstteil nicht messen kann.** Ein Dienst unter `NT SERVICE` öffnet den Prozess der Person nicht (gemessen, Windows-Probe 18.11.). `programm`, `startzeit` und `im_job` sind für ihn darum `null`. Er kann einen Lauf nicht an einen Job binden und keine Sitzung feststellen. Die Bindung an die Sitzung bleibt deshalb beim Prüfteil, der als Person läuft und `prozess_pflicht` setzen kann. Das ist der zweite Grund für die Teilung und für B4.
- **Peer-Prüfung am Eingang.** Das Filterpaar der Plattformschicht trennt je AppContainer-SID, nicht je Lauf; ein Sitzungsport einer anderen Sitzung im selben Projekt ist auf TCP erreichbar (gemessen, Windows-Probe, Frage 11). Der Prüfteil nimmt eine Verbindung darum nur an, wenn sie den Schein dieses Laufs trägt und der Plattformdienst für `peer` mit `tcp` die eigene `sitzung` nennt. Antwortet der Dienst nicht binnen 500 ms, lehnt der Prüfteil ab. Gemessen ist diese Trennung im Modellweg mit mindestens 42 von 42 Verbindungen (Windows-Probe, Frage 11, Zusage W10 des Einschlusses).
- **Herkunft des Scheins.** Der Starter erzeugt ihn je Lauf; er steht nie in einer Datei (`rust/spec/windows-einschluss/README.md`, Bausteine). Wie er zum Prüfteil kommt, legt dieser Vertrag nicht fest. Vorschlag: eine Umgebungsvariable, die der Prüfteil in Stufe 0 liest, bevor er seine Umgebung leert; nie argv, nie eine Datei. Entscheidung beim Einschluss-Vertrag Windows, weil „Lauf starten" dort liegt (OPEN 20).

### 17a.4 Windows: wer welchen Schritt ausführt

**Start (Schritte aus Abschnitt 4).** Der Dienstteil startet mit dem System und hat seine eigene Folge; der Prüfteil startet je Sitzung.

| Schritt | Prüfteil (Person) | Dienstteil (`NT SERVICE\wb-modellweg`) |
|---|---|---|
| 0 Umgebung (F1) | ja | ja, bei jedem Dienststart |
| 1 Abschirmung und Aufruf | Aufruf ja; Abschirmung „nicht anwendbar" (17a.2) | wie links; der Dienst hat keine Optionen einer Sitzung |
| 2 Konfiguration der Betriebsart (Zielliste, Zulassung der CA, CA) | ja, vollständig | nein; er sieht die Regeln des Ziels je Anfrage und prüft sie gegen `halter.json` (B2, H7) |
| 3 Halter | nein; er fragt den Dienstteil (`mw.dienstteil_fehlt`, `mw.dienstteil_unecht`, `mw.halter_kein_schluessel`) | ja; er verbindet sich zum Halter, prüft die Gegenseite und ruft `schluessel.vorhanden` je `halter_ziel` |
| 4 Protokollordner | ja | ja, je Sitzung beim ersten Aufruf (17a.5) |
| 5 Probelauf, Zähldatei und Startzeile | ja | nein; die Zählung liegt beim Prüfteil, weil er die Bytes des Harness sieht |
| 6 Auflösung der ungebundenen Ziele | ja | nein; einen gebundenen Host löst nur der Halter auf (Abschnitt 13.5) |
| 7 Eingang | Sitzungsport statt Socket (17a.3) | Pipe nach 17a.3, mit `FILE_FLAG_FIRST_PIPE_INSTANCE`; scheitert sie, startet der Dienst nicht (Exit 74) |
| 8, 9 stderr-Zeile und `{"bereit":true}` | ja, unverändert | nein; ein Dienst meldet Bereitschaft der Dienststeuerung |

**Je Anfrage (Stufen aus den Abschnitten 13.1 und 13.2).**

| Stufe | Prüfteil | Dienstteil |
|---|---|---|
| 13.1, 1 bis 8 (CONNECT, Zahl der Tunnel, Ziel, SNI) | ja, vollständig | nein; er sieht keinen Tunnel |
| Schein und Peer der Verbindung (17a.3) | ja | nein |
| 13.2, 1 bis 8 (Form, Host, Pfad, Methode, Upgrade, Anmeldung, Körpergrenze, Länge) | ja, vollständig | **ja, noch einmal und vollständig** (B2), gegen die Regeln des Ziels aus `halter.json` |
| 13.2, Köpfe nach Punkt 1 bis 5 | ja; er bildet sie | ja; er prüft, dass kein Anmeldekopf und kein `connection: close` mitgeht |
| 13.2, Stufe 9 (`mw.halter_kein_schluessel`) | nein | ja; er hält die Verbindung zum Halter |
| 13.2, Stufe 10 (Zähldatei im Probelauf) | ja | nein |
| `modell.aufruf` an den Halter | nein | ja |
| Leck-Sperre eines gebundenen Ziels | nein (Abschnitt 7) | nein; sie liegt beim Halter (B1) |
| 13.3 Antwort an den Harness zusammensetzen | ja, aus dem Strom des Dienstteils | nein; er gibt `beginn`, `teil` und `ende` unverändert weiter |
| ungebundenes Ziel (Abschnitt 6) | ja, ganz allein; der Dienstteil kommt nicht vor | nein |

- **Ein Unterschied der beiden Prüfungen ist ein Befund, keine stille Abweichung.** Lehnt der Dienstteil eine Anfrage ab, die der Prüfteil erlaubt hat, antwortet der Modellweg 502 `mw.teile_weichen_ab`, und die Zeile trägt diesen Grund. Dieselbe Regel gilt in Abschnitt 5.4 für H7 gegenüber dem Halter.
- **Wer die Zeile schreibt (B4).** Je Anfrage entsteht genau eine Zeile (Abschnitt 8). Sie schreibt der Teil, der das Ergebnis entscheidet:
  - der **Prüfteil** für jede eigene Ablehnung (13.1, 13.2 Stufen 1 bis 8 und 10, Schein und Peer), für jedes ungebundene Ziel und für das Ende oder den Abriss einer Antwort an den Harness;
  - der **Dienstteil** für jede eigene Ablehnung nach B2, für `mw.teile_weichen_ab` und für jeden Code des Halters (`mw.halter_*`).
  Für eine Anfrage, die er übergeben hat und die der Dienstteil entscheidet, schreibt der Prüfteil keine Zeile. So bleibt die Zahl der Zeilen je Anfrage eins.
- **Beide schreiben in dieselbe Kette.** Die Datei ist `<protokoll>/gateway-<JJJJ-MM>.jsonl` der Sitzung (Abschnitt 8). Mehrere Prozesse ordnen sich über die Sperre aus Vertrag 5, wie es Abschnitt 8, Punkt 4 schon für mehrere Modellweg-Prozesse festlegt. Der Protokollordner gehört der Person; der Dienstteil braucht darauf Schreibrecht. **Fail closed:** Er nimmt einen Protokollordner nur an, wenn dessen Besitzer die SID des Aufrufers der Pipe ist, sonst Exit 78 beziehungsweise `mw.protokoll_fremd`. Grund: Sonst schriebe ein Prozess der Person über den Dienst in einen fremden Ordner. Die DACL, die dem Dienstkonto das Schreiben gibt, ist eine Frage an den Halter-Vertrag (OPEN 20).

### 17a.5 Windows: Fehlercodes

Neu sind fünf Codes; `mw.schein_falsch` und `mw.peer_fremde_sitzung` zeigen sich unter Windows und unter macOS, die übrigen nur unter Windows.

| Code | Exit oder Status | Grund (fest, ASCII) | wann |
|---|---|---|---|
| `mw.dienstteil_fehlt` | 78 | `Dienstteil nicht erreichbar` | Start des Prüfteils: die Pipe des Dienstteils antwortet nicht |
| `mw.dienstteil_unecht` | 78 | `Dienstteil nicht echt` | Start des Prüfteils: der Besitzer der Pipe-Instanz ist nicht die Dienst-SID |
| `mw.dienstteil_fehler` | 502 | `Dienstteil lehnte ab` | im Betrieb: ein Fehler des Dienstteils ohne eigenen Code, eine nicht lesbare Zeile oder ein Abriss der Pipe |
| `mw.teile_weichen_ab` | 502 | `Teile weichen ab` | der Dienstteil lehnt ab, was der Prüfteil erlaubt hat (B2) |
| `mw.protokoll_fremd` | 78 / 503 | `Protokollordner fremd` | der Protokollordner gehört nicht der SID des Aufrufers (17a.4) |
| `mw.schein_falsch` | 403 | `Schein falsch` | eine Verbindung am Eingang trägt nicht den Schein dieses Laufs |
| `mw.peer_fremde_sitzung` | 403 | `Peer aus fremder Sitzung` | der Peer der Verbindung gehört nicht zum Lauf dieser Sitzung |

- Der Code des Halters geht nie an den Harness (Abschnitt 5.1). Ebenso geht **kein Code des Dienstteils** an den Harness: Der Prüfteil antwortet mit seinem eigenen Code, und der Code des Dienstteils steht nur in der Zeile, die der Dienstteil selbst schreibt. Grund wie bei den Halter-Codes: fail closed und keine neue Leckfläche über Fremdcodes.
- `mw.dienstteil_frist` gibt es nicht. Die Frist bis zur Zeile `beginn` zählt der Dienstteil gegen den Halter (`mw.halter_frist`, 504); der Prüfteil wartet dieselben 600 s und antwortet danach ebenfalls 504 `mw.halter_frist`. Eine zweite Frist mit eigenem Code trennte nur, was der Harness nicht unterscheiden soll.
- `mw.ziel_unaufloesbar` (Exit 69) bleibt wie in Abschnitt 13.5 der Code der Betriebsart 1. Die Zeilenform aus den Abschnitten 8 und 13.4 ändert sich nicht: kein neues Feld, keine neue Zeilenart. Welcher Teil geschrieben hat, steht nicht in der Zeile; die Zeile nennt nur die Entscheidung.

### 17a.6 macOS

**Prozesse und Konten.**

| Teil | Konto | Zahl | hält |
|---|---|---|---|
| Modellweg | die Person, signiertes Rollenprogramm (Developer ID, notarisiert, Hardened Runtime ohne `get-task-allow`) | einer je Sitzung und Betriebsart | Zielliste, CA, Protokoll; keinen Schlüssel |
| Halter | eigenes Dienstkonto `_wbhalter`, launchd-Daemon | einer je Maschine | Token, Adresse, Kopfnamen, Leck-Sperre |

- **Der Pflichtpunkt.** Der Schlüsselteil liegt beim Halter unter `_wbhalter`. Solange der Halter ein launchd-Agent der Person ist (Halter-Vertrag, Abschnitt 2.4, Fassung 1, OPEN 9), bekommt vor Fassung 2 keine Person mit macOS einen Schlüssel. Das gilt für **beide** Betriebsarten (17a.7).
- **Grund, gemessen.** Unter macOS bleibt die Umgebung eines Prozesses derselben Kennung lesbar: `KERN_PROCARGS2`, `proc_pidinfo` und `proc_pidpath` antworten auch mit den Sperrzeilen des Profils (`rust/spec/macos-einschluss/README.md`, Befund 1, Messlauf 2c). Ein Geheimnis in einem Prozess der Person ist damit nicht gegen andere Prozesse der Person geschützt. Nur ein eigenes Konto trennt.
- **Keine Teilung des Modellwegs.** Die Rolle `bruecke` folgt unter macOS aus der Signatur (Halter-Vertrag, Abschnitt 2.4). Der Modellweg bleibt ein Programm und ein Prozess je Sitzung. B1 bis B4 gelten sinngemäß: Schlüssel und Leck-Sperre liegen beim Halter (B1); der Halter prüft jede Anfrage selbst noch einmal gegen `halter.json` (B2, dort H7); der Kanal ist ein Unix-Socket im Ordner 0700 statt einer Pipe, und die Kennung kommt aus `LOCAL_PEERCRED` und dem Audit-Token, nie aus einer Angabe (B3); die Zeile schreibt der Teil, der entscheidet — hier der Modellweg für seine Ablehnungen, der Halter für seine (B4).

**Kanal und Peer-Prüfung.**

| Kanal | Art | Peer-Prüfung (`peerkanal`) |
|---|---|---|
| Harness → Modellweg | TCP auf `127.0.0.1` und `[::1]`, Sitzungsport des Weiterreichers | nicht `peerkanal`: Schein je Verbindung (A143), dazu die Zuordnung der Verbindung zu einer PID mit dem Merkmal des Laufs |
| Modellweg → Halter | Unix-Socket im Ordner 0700 der Person, Socket 0600 | `uid` aus `LOCAL_PEERCRED`; Rolle aus `signatur` (Kennung, Team-ID, `gueltig`); `sandbox` `true` ist immer `halter.peer` |

- Der Client prüft den Halter an Ordner und Socket (0700, 0600, eigene uid) und an `LOCAL_PEERTOKEN` gegen die Anforderung des Halterprogramms. Scheitert das, sendet er nichts und endet mit `mw.halter_unecht`.
- **Pfadgrenze.** `sun_path` fasst unter macOS 104 Byte, das NUL eingeschlossen. Ein Pfad des Endpunkts ist dort höchstens 103 Byte lang, nicht 107 wie in Abschnitt 13.5. Unter Windows ist der Wert kein Pfad, sondern der Name der Pipe des Halters. Die Option braucht darum eine Form je System (OPEN 20).
- **Zuordnung ohne root gelingt** (gemessen, Messlauf 2c: Verbindung aus dem Lauf einer PID mit Merkmal zugeordnet, 0,008 s; eine Verbindung von außerhalb einer PID ohne Merkmal, 0,007 s). Sie ersetzt den Schein nicht, sondern ergänzt ihn: Der Schein steht in der Umgebung und ist für den Agenten lesbar.

**Schritte und Zeile.** Alle Schritte 0 bis 9 der Tabelle in Abschnitt 4 führt der eine Prozess des Modellwegs aus; der Halter führt keinen davon, er antwortet nur in Schritt 3 auf `schluessel.vorhanden`. Dasselbe gilt für die Stufen der Abschnitte 13.1 und 13.2: Sie laufen alle im Modellweg, und der Halter prüft nach B2 noch einmal, was ihn betrifft (Pfad, Methode, Kopfnamen, Grenzen). Zwei Unterschiede zu Linux: Schritt 7 legt keinen Unix-Socket für den Harness an, sondern nimmt den Sitzungsport des Weiterreichers (wie unter Windows); `PR_SET_PDEATHSIG` entfällt, der Starter beendet den Modellweg mit SIGTERM, und der Wächter des Dienstes beendet, was das Merkmal des Laufs trägt (`rust/spec/macos-einschluss/README.md`). `kernabbild::abschirmen` wirkt unter macOS über `RLIMIT_CORE`; `PR_SET_DUMPABLE` entfällt. Die Zeile schreibt der Modellweg wie unter Linux.

**Fehlercodes.** Neu sind nur `mw.schein_falsch` und `mw.peer_fremde_sitzung` (17a.5). Die Prüfung der Signatur des Halters fällt unter `mw.halter_unecht` (Abschnitt 5.1); ein eigener Code dafür kommt nicht dazu, weil der Harness den Unterschied nicht erfahren soll.

### 17a.7 Betriebsart 1 unter Windows und macOS

Abschnitt 5.2 lässt den Schlüssel der lokalen Route in Fassung 1 in einer Datei 0600 der Person. Unter Linux ist das als Grenze benannt (OPEN 5 und OPEN 17). Unter Windows und macOS trägt diese Form den Pflichtpunkt nicht:

- Unter Windows liest ein Prozess der Person jeden anderen Prozess derselben Person mit vollen Rechten (hergeleitet, Halter-Vertrag, Abschnitt 2.2). Ein Schlüssel im Prüfteil verstieße gegen B1.
- Unter macOS ist die Umgebung und die Prozessauskunft derselben Kennung gemessen lesbar (17a.6).

**Festlegung dieses Nachtrags.** Vor Schritt II gilt auf beiden Systemen: Auch der Schlüssel der Betriebsart 1 kommt vom Halter, mit derselben Op und denselben Codes wie Abschnitt 5.1. Bis dahin startet Betriebsart 1 auf diesen Systemen nicht mit einer Schlüsseldatei. Das ist die Folge von B1 und des Pflichtpunkts, keine neue Entscheidung; die Reihenfolge der Bausteine ist offen (OPEN 17).

### 17a.8 Was gemessen werden muss

Die Außentests M1 bis M15 (Abschnitt 15) gelten unter Linux. Dazu kommen diese; keiner ist gelaufen.

| Nr. | Zusage | Test | Stand |
|---|---|---|---|
| MW-W1 | Der Agent im AppContainer erreicht den Eingang nur über den Weiterleiter seiner Sitzung | Verbindung auf den Sitzungsport der eigenen und einer fremden Sitzung desselben Projekts, je mit und ohne Schein | **gemessen** für den Modellweg, mindestens 42 von 42 (Windows-Probe, Frage 11); für den Schein in dieser Form hergeleitet |
| MW-W2 | Der Agent öffnet die Pipe des Dienstteils nicht | Ein Prozess im AppContainer der Person öffnet `\\.\pipe\wb-modellweg`. Erwartet: Fehler 5 | hergeleitet; gemessen ist dasselbe für die Pipe des Halters (Windows-Probe 18.11., Frage 2) |
| MW-W3 | Die SID des Aufrufers kommt aus der Pipe | 100 Verbindungen aus Prozessen zweier Personen: SID und Zulassung 100 von 100 richtig | **gemessen** für den Mechanismus (Windows-Probe 18.11., Frage 1), nicht für diese Pipe |
| MW-W4 | Die Person erreicht den Speicher des Dienstteils nicht | `OpenProcess` mit `PROCESS_VM_READ` und `PROCESS_QUERY_INFORMATION` auf den Dienstteil aus einem Prozess der Person. Erwartet: Fehler 5 | hergeleitet; gemessen ist, dass ein Dienst unter `NT SERVICE` den Prozess der Person nicht öffnet (Windows-Probe 18.11.), nicht die Gegenrichtung |
| MW-W5 | Kein Teil des Modellwegs trägt den Token | Eine Marke als Token beim Halter. Suche in Umgebung, argv und Speicher beider Teile und in allen Dateien, die die Person liest. Erwartet: 0 Treffer; der Zähler der Attrappe steigt | Annahme, folgt aus Weg A; zu messen in der VM |
| MW-W6 | Ein Unterschied der beiden Prüfungen wird sichtbar | Der Dienstteil bekommt eine Regelliste, die enger ist als die der Zielliste. Erwartet: 502 `mw.teile_weichen_ab` und genau eine Zeile mit diesem Grund | Annahme (Vertragstext) |
| MW-M1 | Ein Geheimnis in einem Prozess der Person ist nicht geschützt | `KERN_PROCARGS2`, `proc_pidinfo` und `proc_pidpath` gegen den Modellweg aus einem Prozess der Person | **gemessen** (Messlauf 2c, Befund 1); daraus folgt der Pflichtpunkt |
| MW-M2 | Der Agent erreicht den Socket des Halters nicht | Agent unter dem Seatbelt-Profil: `connect`, `stat`, `open` auf Ordner und Socket | Annahme des Halter-Vertrags (Abschnitt 10, Außentest M2 dort); das Profil schreibt der Einschluss-Vertrag macOS |
| MW-M3 | Die Person erreicht den Speicher des Halters unter `_wbhalter` nicht | `task_for_pid`, `lldb` und ein Abbild gegen den Daemon mit Testmarke. Erwartet: 0 Treffer | hergeleitet (fremdes Konto, Hardened Runtime ohne `get-task-allow`); nicht gemessen, Halter-Vertrag OPEN 9 |
| MW-M4 | Der Modellweg ist für den Halter ein Rollenprogramm | Eine unsignierte und eine anders signierte Kopie des Modellwegs verbinden sich zum Halter. Erwartet: je `halter.peer`, Antwort an den Harness 502 | Annahme; entspricht Außentest M3 des Halter-Vertrags |

Alles in diesem Abschnitt, was nicht ausdrücklich „gemessen" trägt, ist hergeleitet oder Annahme. Die Zahlen und Fehlercodes der Windows-Probe vom 18.11. und des Messlaufs 2c sind Messungen aus anderen Verträgen, in diesem Vertrag nicht nachgemessen.

### 17a.9 Offene Fragen

**Fragen an Halter- und Paket-Vertrag.**

1. **Dienst-SID im Halter.** Trägt `halter.json` die Dienst-SID `NT SERVICE\wb-modellweg` als Rolle `bruecke` ein, und legt der Installer den Dienst mit diesem virtuellen Dienstkonto an? Der Halter-Vertrag nennt in der Rollentabelle nur „Dienst-SID des Modellwegs" (Abschnitt 2.2). Empfehlung: ja, mit dem Namen aus diesem Abschnitt. Grund: Ohne eingetragene SID bekommt der Dienstteil die Rolle nicht, und der Halterweg fährt unter Windows nicht.
2. **Form des Endpunkts.** Unter Linux ist der Endpunkt seit dem 25.11.2026 ein fester Pfad ohne Option (Abschnitte 3a und 5.1). Unter Windows ist der Endpunkt ein Pipe-Name, unter macOS ein Pfad mit höchstens 103 Byte. Empfehlung: auch dort ein fester Wert je System, geprüft in Schritt 2, keine Option. Grund: Ein Argument des Aufrufers lenkte die Anfragen an eine fremde Gegenstelle. alt: „eine Option mit einer Form je System".
3. **Schreibrecht des Dienstteils am Protokollordner.** Der Ordner gehört der Person, die Zeile schreibt nach B4 auch der Dienst. Empfehlung: Der Starter gibt dem Dienstkonto eine nicht vererbte ACE nur auf den Protokollordner dieser Sitzung, und der Dienstteil prüft den Besitzer (17a.4). Grund: Jede weitere Form legt entweder eine zweite Kette an oder gibt dem Dienst mehr Rechte als eine Sitzung braucht.
4. **Übergabe des Scheins an den Prüfteil** (17a.3). Empfehlung: eine Umgebungsvariable, die der Prüfteil in Stufe 0 liest und danach leert; nie argv, nie eine Datei. Grund: Eine Datei widerspricht der Vorgabe des Einschlusses, und argv bleibt für jeden Prozess der Person lesbar.
5. **Paket unter Windows.** Welche Programme liefert das Paket, unter welchen Pfaden, und wer legt die beiden Dienste an? Der Paketvertrag beschreibt heute nur Linux (`rust/spec/paket/README.md`). Empfehlung: ein Programm mit zwei Betriebsweisen und ein Dienst je Maschine, vom Installer angelegt. Grund: Zwei Programme mit derselben Prüflogik liefen auseinander, und B2 verlangt dieselbe Prüfung an beiden Stellen.
6. **Betriebsart 1 vor Schritt II** (17a.7). Empfehlung: Der Schlüssel der lokalen Route geht auf beiden Systemen zugleich mit der Cloud-Route auf den Halterweg. Grund: Sonst trägt unter Windows und macOS weiter ein Prozess der Person einen Schlüssel, und der Pflichtpunkt hält nicht.
7. **Abbild des Halters unter Windows.** Was schirmt den Halter dort gegen ein Speicherabbild ab? Der Halter-Vertrag verweist auf den Windows-Plan (Abschnitt 2.1). Für den Modellweg ist die Frage nach B1 ohne Wirkung; für den Halter ist sie offen.

8. **Wer ist unter Windows der Aufrufer für ein Geheimnis der Person? Zur Hälfte beantwortet.** Nach H14 (Abschnitt 5.4) gehört der Token der Person, und `modell.aufruf` setzt ihn nur für das Konto des Aufrufers ein. Der Dienstteil hat ein eigenes Dienstkonto. Der Halter-Vertrag hat die Vertreter-Regel für die Brücke auf Schritt II gelegt (`rust/spec/halter/README.md:121` und OPEN 19). Offen bleibt: Was gilt unter Windows bis Schritt II? Empfehlung: Die Cloud-Route bleibt dort bis Schritt II aus, wie unter macOS ohne Daemon (17a.7). Grund: Jeder andere Weg gäbe entweder dem Dienstkonto ein Geheimnis der Person oder dem Prüfteil die Rolle `bruecke`, und beides nimmt eine Sperre weg.
8a. **Zahl der Pipe-Instanzen des Halters unter Windows.** Der Halter-Vertrag nennt 16 Instanzen und sagt selbst, dass sie für 8 Verbindungen der Brücke neben Hub und Oberfläche nicht sicher reichen (`halter:180`, `halter:328`, OPEN 19). Empfehlung: mindestens 8 für die Brücke, dazu Hub und Oberfläche, also mindestens 32. Grund: Eine 17. Verbindung bekommt `ERROR_PIPE_BUSY`, und der Modellweg hat dafür keinen eigenen Code.
9. **Welche Ablage trägt den Token der Person unter Windows?** Die Ablage des Halters gehört dem Dienstkonto (Halter-Vertrag, Abschnitt 2.2), der Token aber der Person. Empfehlung: dieselbe Ablage, mit dem Konto der Person im Namen des Geheimnisses und in der Antwort auf `schluessel.vorhanden`, nie eine Ablage unter dem Profil der Person. Grund: Gemessen entschlüsselt jeder Prozess der Person ein Geheimnis aus DPAPI-CurrentUser, auch im AppContainer (Windows-Probe, Frage 3).

**An den Einschluss-Vertrag macOS.**

10. **Daemon unter `_wbhalter`.** Wann steht der launchd-Daemon unter eigenem Konto, und wer legt das Konto an? Bis dahin bekommt keine Person mit macOS vor Fassung 2 einen Schlüssel. Empfehlung: vor Schritt II, mit dem Einrichten durch den Installer. Grund: Der Pflichtpunkt hängt allein daran.
11. **Profil und Socket des Halters.** Gilt die Sperre des Socket-Ordners im Seatbelt-Profil auch, wenn der Socket unter dem Heimordner von `_wbhalter` liegt statt unter dem der Person? Empfehlung: Das Profil sperrt beide Orte namentlich. Grund: Ein Daemon unter eigenem Konto legt seinen Socket nicht mehr im Ordner der Person an, und eine Regel auf den alten Ort wäre dann leer.
12. **Weiterreicher und Peer-Prüfung.** Der Weiterreicher ist nicht gebaut (`rust/spec/macos-einschluss/README.md`, OPEN 1). Welche Form hat die Zuordnung, die der Modellweg je Verbindung erfragt, und von welchem Prozess? Empfehlung: dieselbe Form wie `peer` des Plattformdienstes unter Windows, mit pid und Startzeit. Grund: Eine gemeinsame Form hält die Prüfung im Modellweg auf beiden Systemen gleich.

## 18. Später (nicht im Durchstich)

- Ein lesender Stand je Sitzung: Route erreichbar oder gesperrt, letzter Fehlercode ohne Inhalt (erste Fassung).
- Eine Grenze des Betriebssystems für den Prozess des Modellwegs (nftables je cgroup, root bei der Installation).
- Leck-Sperre für kodierte Formen des Schlüssels.
- Der Schlüssel der Betriebsart 1 beim Halter statt in einer Datei der Person, mit derselben Op und denselben Codes wie Abschnitt 5.1 (OPEN 17). Für die Cloud-Route ist der Halterweg nicht „später“, sondern Pflicht ab Schritt I (Abschnitt 5.1). Unter Windows und macOS ist er für beide Betriebsarten Pflicht vor Schritt II (Abschnitt 17a.7).
- Der **Bau** unter Windows und macOS; mehrere Modelle je Route; Kosten in Geld (Vertrag 7). Was dort anders gilt, steht seit dem 23.11.2026 als Vertragstext in Abschnitt 17a.

## 19. OPEN

1. **Betriebssystem-Grenze** für den Modellweg-Prozess (siehe „Später“). Bis dahin prüft er sein Ziel nur selbst.
2. **Kodierte Lecks** (Base64, Base64url, Prozentkodierung) erkennt Fassung 1 nicht.
3. **`Transfer-Encoding: chunked` in Betriebsart 1** lehnt Fassung 1 vorläufig mit 411 ab, bis zur Messung (M11). Die Frage bleibt offen.
4. **Vertrag 1 und 2** tragen Schritt, Codes, `modellweg.pid`, `ziel`, `modell` und `netzziele` (bestätigt am 20.11.2026).
5. **Grenze der Schlüsseldatei unter Linux Fassung 1: für die Cloud-Route aufgelöst** (Vertragsbefund B05, Entscheid des Betreibers, 23.11.2026, „Halterweg vor Schritt I“). Der Token der Betriebsart 2 liegt beim Halter (Abschnitt 5.1), nicht in einer Datei der Person. Für die Schlüsseldatei der Betriebsart 1 gilt die Grenze weiter: Ein Prozess der Person außerhalb des Einschlusses liest sie, weil er dieselbe Kennung hat; der Agent im Einschluss erreicht sie nicht (OPEN 17).
6. **Fristen** (10 s, 600 s) und die Grenzen von 8 gleichzeitigen Anfragen (Betriebsart 1) und 32 offenen Tunneln (Betriebsart 2) sind gesetzt, nicht gemessen; gemessen ist nur, dass 8 Tunnel am 24.11.2026 zu wenig waren; eine kurze Antwort dauerte beim Partnerteam rund 6 s (F).
7. **Betriebsart 2, Parameter der Zielliste** (der Bau wartet nicht auf den echten Lauf): die Werte von `endpoints.*` für den eingesetzten Plan; die Ablage des Tokens; die Laufzeit des Tokens aus dem Gerätefluss. **Pfade an Zielen mit Schlüssel: entschieden** (Nachtrag N1 des Einstellungs- und des Halter-Vertrags, 25.11.2026): an `api.individual.githubcopilot.com` nur die sieben gemessenen genauen Pfade, an `api.githubcopilot.com` nur `GET /agents/swe/internal/memory/v0/enabled`, an `api.github.com` nur `GET /copilot_internal/user` und `GET /copilot/mcp_registry`; kein Teilbaum an einem Ziel mit Schlüssel. Ein unbekannter Pfad endet beim Halter mit `halter.pfad_verboten` und einer Kettenzeile; jede fehlende Regel entscheidet die Leitung beim ersten Start einzeln anhand der Kette. Die Regeln gehören der Einstellungsdatei; dieser Vertrag nennt sie nur als Beispiel (Abschnitt 12). alt: „die Pfade der Modellaufrufe und eine engere Liste für `api.githubcopilot.com`“.
8. **Tokenart** (fine-grained PAT mit „Copilot Requests“ oder Token aus dem Gerätefluss) ist eine Betreiberentscheidung; der Vertrag sagt nichts zu.
   Die Übernahme des Tokens in die Schlüsseldatei durch den Kern-Dienst ist ein Vorschlag.
9. **Zulassung der lokalen CA** durch die Organisation. Ohne Zulassung startet Betriebsart 2 nicht, und E4 cloud bleibt im Probelauf „offen“.
10. **Org-Richtlinien** (`managed_settings`, `forceLoginOrgs`): Ob sie einen TLS-beendenden Ausgang erkennen oder sperren, ist nur mit einem Organisationskonto messbar.
11. **Köpfe in Betriebsart 2:** entschieden (Projektentscheid, 20.11.2026, fail closed): `accept-encoding: identity` an gebundenen Zielen, `mw.antwort_komprimiert` für eine komprimierte Antwort von dort.
    Alle anderen Köpfe bleiben unverändert. Ergibt ein weiterer echter Lauf neue Kopfnamen, kommt ein Nachtrag.
12. **Umgebung von OpenSSL (Bau 20.11., Frage F1): entschieden** (Projektentscheid, 20.11.2026). Der Weiterreicher lehnt den Start ab, wenn `SSL_CERT_FILE`, `SSL_CERT_DIR` oder `OPENSSL_CONF` gesetzt ist, mit Exit 78 und `mw.umgebung_unsicher` (Abschnitt 3, Stufe 0 in Abschnitt 4). Das Leeren der Umgebung durch das Startprogramm bleibt zusätzlich; die Ablehnung durch „Lauf starten“ gehört zu Vertrag 1.
13. **Rahmung in Betriebsart 1 (Bau 20.11., Frage F2):** Bei „Ende mit dem Schließen“ kann der Harness einen späten Abriss der Leck-Sperre nicht vom Ende unterscheiden. Betriebsart 2 sendet Antworten ohne Länge schon `chunked`; für `openai-lokal` später ebenso.
14. **`permessage-deflate` mit Sperre (Nachtrag 16, W9):** Entpacken vor der Sperre ist bewusst nicht vorgesehen. Der Weiterreicher handelt keine Erweiterung aus (Abschnitt 13.2) und bricht bei einer Erweiterung ab (Abschnitt 13.3). Ein Dienst, der ohne Kompression nicht arbeitet, bräuchte eine neue Entscheidung.
15. **Zählung der Rahmen im Probelauf (Nachtrag 17):** Die Zähldatei hat kein Feld, das zeigt, ob WebSocket-Rahmen gelesen wurden. `anfragen` zählt nur HTTP-Anfragen; auf der Cloud-Route ist es durch die Upgrade-Anfrage schon vor dem ersten Rahmen mindestens 1. Wer sehen will, dass Rahmen gezählt wurden, liest die Marker-Zähler. Ein eigenes Feld wäre eine Änderung der Form, die der Läufer liest, und bräuchte eine Entscheidung (nicht Teil von Fassung 1).
16. **Suche nach dem Schlüssel (Nachtrag 18):** (a) Ein `--in`, das eine Verknüpfung ist, fehlt oder keine Datei und kein Ordner ist, zählt in `nicht_lesbar` (Exit 3); S3 sagte dazu nur „nicht verfolgt“. Bitte bestätigen. (b) Dieselbe Datei über mehrere `--in` zählt einmal (Gerät und Inode); die Aufgabe nannte das nicht. (c) Verknüpfungen zählen in `uebersprungen`. (d) Eine Schlüsseldatei unter einem `--in` zählt als Fund (keine Ausnahme per Inode, Abschnitt 9a S4). (e) Der Code für eine verletzte Schlüsseldatei ist `mw.schluessel_fehlt` mit Exit 78, wie in der Aufgabe festgelegt; `mw.schluessel_datei` und `mw.schluessel_form` des Starts (Abschnitt 4) gelten für die Suche nicht. Nachtrag 19 (21.11.) ergänzt zwei Festlegungen des Baus, die das Projekt bestätigt hat: (f) Ein Ordner tiefer als 256 Ebenen zählt in `nicht_lesbar` (S3). (g) Lässt sich die Zeile auf stdout nicht schreiben, endet die Suche mit Exit 70 `mw.intern` (S5, S6). Beide wirken fail closed. Fälle dafür gibt es nicht; der Bau deckt sie mit eigenen Tests.
17. **Halterweg, Betriebsart 1 (Nachtrag 20, 23.11.):** Der Schlüssel der lokalen Route bleibt in Fassung 1 eine Datei 0600 der Person (Abschnitt 5.2). Er bleibt ein Geheimnis im Bereich `maschine`; der Token der Cloud-Route ist seit dem 23.11.2026 ein Geheimnis der Person (`anmeldung.<ziel>`, H14 in Abschnitt 5.4). Betriebsart 1 kommt **nicht** in diesem Stand (Entscheidung im Halter-Vertrag, 23.11.2026). Die Reihenfolge ist offen: Sie zieht `/var/lib/werkbank/<nutzer>/modellweg.schluessel` und die achte Prüfung von `werkbank-pruefen` nach (Paketvertrag `rust/spec/paket/README.md:179` und `:342`), und beide gehören zum Halter- und Paket-Vertrag.
18. **Halterweg, Umfang des Halter-Vertrags: entschieden** (Nachtrag 22, 23.11.). Der Halter-Vertrag hat alle vierzehn Punkte aus Abschnitt 5.4 entschieden; die drei Sperren sind aufgelöst. H2: Der Halter ist ein eigener Prozess der Person mit `PR_SET_DUMPABLE` 0; der Satz „Halter und Modellweg sind ein Prozess" steht seit dem Merge nur noch als alter Stand im Halter-Vertrag (`rust/spec/halter/README.md:233`; vor dem Merge war die Zeile `:212`). H4: eigenes Feld `abfrage` statt des `?` im Feld `pfad` (`halter:373` für `pfad` „ohne Abfrageteil", `halter:374` für das neue Feld; vor dem Merge `:312`). H5: eigene Op `modell.strom` (`halter:411`). Der Halter-Vertrag steht für sich; dieser Vertrag nennt nur, was er annimmt. Die Folge von H14 unter Windows ist jetzt entschieden (Abschnitt 5.4, OPEN 23).
19. **Halterweg und die Kontengrenze (Nachtrag 20, ergänzt 23.11. nach der Entscheidung im Halter-Vertrag):** Entschieden ist ein eigener Prozess der Person mit `PR_SET_DUMPABLE` 0; die Ablage bleibt unter Linux in Fassung 1 eine Datei 0600 der Person, und der Systemnutzer kommt erst in Schritt II. Der Gewinn ist benannt und klein, aber echt: Der Parser des Modellwegs, der Bytes des Agenten liest, hält den Token nicht mehr. Die Grenze bleibt: Jeder Prozess der Person liest die Datei 0600 der Ablage. Läuft der Halter unter der Kennung der Person (Halter-Vertrag, Abschnitt 2.3, Fassung 1), hält er den Token im eigenen Speicher und schirmt sich gegen Abbild und `ptrace` ab. Hergeleitet ist: Ein Prozess derselben Kennung ohne root liest diesen Speicher dann nicht, weil `PR_SET_DUMPABLE` 0 auch `ptrace` desselben Nutzers sperrt. Gemessen ist das nicht. Der Außentest steht seit dem Merge im Halter-Vertrag als L5 (`rust/spec/halter/README.md:695`: `ptrace`, `/proc/<pid>/mem`, `/proc/<pid>/environ` und SIGQUIT gegen einen Halter mit Testmarke; erwartet je Fehler, kein Kernabbild, 0 Marken). Er ist noch nicht gelaufen. Erst der eigene Systemnutzer macht daraus eine Kontengrenze (Halter-Vertrag, OPEN 9 und OPEN 18).
20. **Windows und macOS, Abstimmung mit Halter- und Paket-Vertrag (Nachtrag 21, 23.11.):** Abschnitt 17a.9 nennt zehn Fragen an Halter- und Paket-Vertrag (Dienst-SID im Halter, Form des Endpunkts, Schreibrecht am Protokollordner, Übergabe des Scheins, Paket unter Windows, Betriebsart 1 vor Schritt II, Abbild des Halters unter Windows, Aufrufer für ein Geheimnis der Person bis Schritt II, Zahl der Pipe-Instanzen, Ablage des Tokens der Person) und drei an den Einschluss-Vertrag macOS (Daemon unter `_wbhalter`, Profil und Socket des Halters, Form der Peer-Prüfung). Keine ist von diesem Vertrag entscheidbar. Bis zur Antwort steht Abschnitt 17a als Vertragstext, und unter Windows und macOS entsteht kein Code.
21. **Der Schein am Eingang (Nachtrag 21, 23.11.):** Unter Windows und macOS prüft der Modellweg je Verbindung den Schein des Laufs und die Sitzung des Peers (Abschnitt 17a.3, Codes `mw.schein_falsch` und `mw.peer_fremde_sitzung`). Unter Linux gibt es ihn nicht; dort trägt der Socket im Ordner 0700 die Trennung. Der Schein ist kein Geheimnis gegen den Agenten: Unter macOS bleibt die Umgebung für den Agenten lesbar (gemessen, Messlauf 2c), unter Windows steht er in der Umgebung des Agenten. Er ordnet eine Verbindung einem Lauf zu, mehr nicht.
22. **Der Abfrageteil ist nicht gemessen (Nachtrag 22, 23.11.):** Das Feld `abfrage` von `modell.aufruf` steht im Vertrag, weil ein Abfrageteil nach HTTP möglich ist. Alle bisher gemessenen Pfade der Copilot CLI tragen kein `?` (Messung vom 20.11.2026, erster und zweiter angemeldeter Start). Der nächste Lauf mit einem echten Konto muss zeigen, ob und an welchen Pfaden ein Abfrageteil vorkommt, und mit welchen Zeichen. Bis dahin ist die Form von `abfrage` gesetzt, nicht belegt.
23. **Folge von H14 unter Windows: entschieden, Schritt II (Nachtrag 22, 23.11.; nachgezogen nach dem Merge des Halter-Vertrags).** Der Anmeldetoken der Cloud-Route ist ein Geheimnis der Person (`anmeldung.<ziel>`, Bereich `person`), und der Halter setzt es nur für das Konto des Aufrufers ein. Unter Windows ist der Aufrufer der Dienstteil unter eigenem Dienstkonto (Abschnitt 17a.2), dem kein Geheimnis der Person gehört. Der Halter-Vertrag entscheidet: **Eine Brücke als Vertreter gibt es in Fassung 1 nicht** (`rust/spec/halter/README.md:121`); Vertreter-Regel und Lauf-Schein gehören Schritt II (Halter-Vertrag, OPEN 19). Der Halterweg der Cloud-Route fährt unter Windows in Fassung 1 damit nicht: `schluessel.vorhanden` für ein `anmeldung.*` ist aus dem Dienstkonto `false`, und Schritt 3 endet mit Exit 78 `mw.halter_kein_schluessel`. Offen bleibt nur, was unter Windows bis Schritt II gilt (Abschnitt 17a.9, Frage 8), und die Zahl der Pipe-Instanzen (`halter:328`, Halter-Vertrag OPEN 19: 16 reichen für 8 Verbindungen der Brücke neben Hub und Oberfläche nicht sicher).
24. **Rahmenregeln eines gebundenen Ziels: erledigt (im Halter-Vertrag, 23.11.2026).** Abschnitt 13.3, W4 verlangt fail closed einen Abriss, wenn ein Rahmen des Dienstes RFC 6455 Abschnitt 5 verletzt: Steuerrahmen mit FIN=0 oder Länge über 125, Fortsetzung ohne offene Nachricht, neuer Datenrahmen bei offener Nachricht. Für ein gebundenes Ziel prüft der Halter diese drei Regeln jetzt selbst am Rahmenkopf; jeder Verstoß endet bei ihm mit `halter.ziel_fehler` (`rust/spec/halter/README.md:435` bis `:447`, Stufe 28 in `:595`). Damit prüft ab Schritt I der Halter, was W4 verlangt, und der Modellweg prüft die Rahmen weiter nicht noch einmal. Gemessen ist das noch nicht: Das Fallformat des Halters kann keine Rahmenfolgen ausdrücken, die Regeln stehen darum als Außentest L8 im Halter-Vertrag (`:698`). alt: Der Halter-Vertrag prüfte nur RSV-Bit, Maskenbit, Opcode und Länge, und niemand prüfte die drei Regeln.
25. **Zurückgehaltene Bytes am Ende eines Stroms: erledigt (im Halter-Vertrag, 23.11.2026).** Abschnitt 13.3, W3 sagt: „Beim Ende der Verbindung geht alles Zurückgehaltene hinaus." Der Halter-Vertrag sagt jetzt dasselbe: Am Ende des Stroms (Ende des Ziels, `strom.ende`, Ende einer Antwort mit Körper) gehen die zurückgehaltenen Bytes als letztes `teil` vor `ende` hinaus, auch wenn das Ziel mitten in einem Rahmen schließt; bei einem Abriss, einem Treffer und `halter.ziel_komprimiert` gehen sie nicht hinaus (`rust/spec/halter/README.md:448` bis `:453`, `:610` bis `:613`). Das deckt sich mit W3 und mit dem Abriss nach W4 und W5 (Abschnitt 13.3). Gemessen wird es mit dem Außentest L9 des Halters (`:699`). alt: Der Halter-Vertrag nannte nur das Zurückhalten.
26. **Einstellung des Admin-Schalters `--protokoll-pfad-voll` (Vorschlag an Vertrag 2, 23.11.2026).** Dieser Vertrag legt nur die Option des Modellwegs fest.
    Vorschlag: ein Schlüssel `protokoll_pfad_voll` (Wahrheitswert, Vorgabe `false`) in der Einstellung für Admins (Vertrag 2), den „Lauf starten“ (Vertrag 1) als `--protokoll-pfad-voll` weiterreicht, wenn er `true` ist.
    Der Modellweg prüft ihn nicht gegen eine Einstellung; ohne die Option bleibt der Pfad begrenzt. Name und Weg legt Vertrag 2 fest; dieser Vertrag ändert dort nichts.
27. **Organisationsname im zweiten Segment (bekanntes Restrisiko, Abschnitt 8).** Bei Pfaden der Form `/repos/<org>/…` steht der Name der Organisation im zweiten Segment, wenn er `[A-Za-z0-9._~-]{1,32}` genügt; das ist Firmeninhalt.
    Möglich wären: nur das erste Segment, eine Liste erlaubter erster Segmente, oder ein Hash des zweiten Segments. Das entscheidet die Projektleitung.
28. **Alte Ketten ohne `pfad` und `host`.** Der Prüfer kennt für Fassung 2 keine Unterfassungen. Damit eine Kette, die vor dieser Änderung geschrieben wurde, nicht rot wird, nimmt er für `modellweg.aufruf` beide Formen an:
    15 oder 16 Felder mit `pfad` und `host`, und 13 oder 14 Felder ohne beide (Vertrag 5, Abschnitt 6.7). Ob und wann die alte Form wegfällt, entscheidet die Projektleitung; der Schreiber baut nur noch die neue.
29. **Geerbtes `SIG_IGN` in der Suche (Abschnitt 3, Regel 2):** Der Weiterreicher setzt die Handler vor dem `prctl`; `werkbank-modellweg suche` tut es nicht. Erbt die Suche SIGTERM als ignoriert, bleibt ihr Todessignal wirkungslos, und ein Elternprozess, der nach ihrer Prüfung stirbt, lässt sie bis zu ihrem eigenen Ende laufen. Sie ist ein kurzer Einzelaufruf, darum ist das hingenommen. Ein Handler wäre nur mit `signal_hook::low_level::emulate_default_handler` richtig, damit ein Signal nicht wie ein erfolgreiches „nichts gefunden“ aussieht; das braucht eine Entscheidung und eine Regel in Abschnitt 9a.
30. **Zug sicherer Modus (Abschnitt 3a, 25.11.2026): erledigt.** Gebaut sind alle fünf Pflichten mit ihren Codes und die Außentests MW-L1 bis MW-L5 (Abschnitt 3a, „Stand des Baus"), dazu die Geltung unter Linux in jedem Start. alt: „Die fünf Pflichten stehen als Zusagen im Vertrag; gebaut ist keine davon. Gebaut ist seit dem 25.11.2026 der feste Pfad des Halter-Sockets ohne Option (Abschnitt 5.1) und, als Teil der Zielliste, dass `halter_ziel` genau der `host` ist (Stufe 12, `zl.bindung`, Exit 78 `mw.ziele_ungueltig`). Zu bauen bleiben: der eigene Code `mw.ziel_halter_name` unter setgid, das Leeren der Umgebung auf die leere Positivliste (`mw.umgebung_rest`), `umask 077` mit der Abweisung von Gruppenbits in `--socket-rechte` und die Zusage ohne Kinder (`mw.kind_verboten`), dazu die Außentests MW-L1 bis MW-L5." Offen bleibt daraus OPEN 36; OPEN 35 ist erledigt.
31. **`XDG_RUNTIME_DIR` abweichend vom festen Pfad: erledigt auf beiden Seiten (25.11.2026).** Der Modellweg nimmt nur den festen Pfad `/run/user/<uid>/wb-halter/halter.sock` und liest `XDG_RUNTIME_DIR` nicht (Abschnitt 5.1, Abschnitt 3a, Pflicht 2). Der Halter nimmt denselben festen Pfad und liest `XDG_RUNTIME_DIR` nie (Halter-Vertrag, Abschnitt 2.3, „Socket“, `rust/spec/halter/README.md:309`). alt: „**`XDG_RUNTIME_DIR` abweichend vom festen Pfad (berichtigt 25.11.2026).** Auf der Seite des Modellwegs erledigt: Er nimmt nur den festen Pfad `/run/user/<uid>/wb-halter/halter.sock` und liest `XDG_RUNTIME_DIR` nicht (Abschnitt 5.1, Abschnitt 3a, Pflicht 2). Auf der Seite des Halters offen: Der Halter-Vertrag nennt heute noch `$XDG_RUNTIME_DIR/wb-halter/halter.sock` und zieht den festen Pfad im angekündigten Zug des Halters nach (Entscheid des Inhabers, 25.11.2026). Bis dahin findet der Modellweg den Halter nur, wenn `XDG_RUNTIME_DIR` der Person gleich `/run/user/<uid>` ist. alt: „erledigt (Abgleich mit dem Halter-Vertrag, 25.11.2026). Der Halter nimmt denselben festen Pfad `/run/user/<uid>/wb-halter/halter.sock` und liest `XDG_RUNTIME_DIR` nicht (Halter-Vertrag, Abschnitt 2.3). […]"; die Behauptung über den Halter-Vertrag stimmte nicht.“
33. **Körper einer Upgrade-Anfrage an ein gebundenes Ziel (Abschnitt 5.1, 25.11.2026).** `modell.strom` hat kein Feld
    für einen Körper, und der Bau lässt einen Körper einer durchgereichten Upgrade-Anfrage darum fallen, statt die
    Anfrage abzuweisen. Ein Handschlag nach RFC 6455 hat keinen Körper; gemessen ist der Fall nicht. Zu entscheiden
    ist, ob eine solche Anfrage 400 `mw.anfrage_unlesbar` bekommen soll (fail closed) oder ob der Körper wie heute
    fällt. Der Halter ist davon nicht betroffen, und ein Geheimnis ist es auch nicht.
34. **Startschritt 3 fragt je `halter_ziel` mit `ziel`: erledigt (gebaut 25.11.2026).** Der Halter nimmt `schluessel.vorhanden` mit `ziel` an (Halter-Vertrag, Abschnitt 4, `rust/spec/halter/README.md:530`), und Schritt 3 fragt je gebundenes Ziel mit `ziel` (Abschnitt 13.5). Abbildung: `false` und `halter.ziel_unbekannt` ergeben `mw.halter_kein_schluessel` mit Exit 78; jeder andere Fehler folgt Abschnitt 5.1 (`mw.halter_fehler`, Exit 78); ohne Antwort binnen 5 s `mw.halter_frist`, Exit 78.
    alt: „**Startschritt 3 fragt je `halter_ziel` mit `ziel` (nach dem Halter-Zug zu F1, 25.11.2026).** Der Inhaber des Halter-Vertrags hat entschieden: `schluessel.vorhanden` nimmt genau eines von `name` oder `ziel` (Kennung eines Ziels aus `halter.json`); mit `ziel` löst der Halter den Namen des Geheimnisses selbst auf, und nur die Rolle `bruecke` darf nach `ziel` fragen. Ein unbekanntes Ziel ist der Fehler `halter.ziel_unbekannt`, nicht `false`; ein bekanntes Ziel ohne Geheimnis ist `{"vorhanden": false}`; fehlen beide Felder, ist das `halter.schluessel_fehlt` (Stufe 9 des Halter-Vertrags), und stehen beide da, ist es `halter.schluessel_unbekannt` (Stufe 8). Das kommt im nächsten Halter-Zug. Bis dahin ist der Aufruf mit `ziel` **nicht** gebaut, weil der heutige Halter ihn abweist. Danach fragt Schritt 3 je gebundenes Ziel mit `ziel`, und die Abbildung ist: `false` und `halter.ziel_unbekannt` ergeben `mw.halter_kein_schluessel` mit Exit 78.“
32. **Die verbotenen Kopfnamen an einem gebundenen Ziel vor Schritt I: erledigt (Entscheid des Betreibers, 25.11.2026).** Vor Schritt I öffnet der Modellweg die Verbindung zu einem gebundenen Ziel selbst (Abschnitt 6) und setzt den Kopf mit dem Schlüssel aus der Datei ein (Abschnitt 5.2). Entfiele dort `host`, `content-length` oder `transfer-encoding`, fehlten der Anfrage nach außen der Pflichtkopf `Host` und die Rahmung ihres Körpers. Entschieden ist darum: Auf dem direkten Weg bleiben diese drei, auf dem Halterweg entfällt jeder der 31 Namen (Abschnitt 13.2, Punkt 1). Die Liste des Halterwegs steht im Code benannt, auch solange der Weg nicht gebaut ist. alt: „Die Richtung ist entschieden (fail closed: lieber mehr entfernen), die Folge für den Weg vor Schritt I ist benannt und braucht eine Entscheidung der Projektleitung".

35. **Code der Suche für ein fremdes Halterziel (Abschnitt 9a, 25.11.2026): erledigt.** Die Suche antwortet auf `zl.halter_name` wie der Start mit Exit 78 `mw.ziel_halter_name` (Abschnitt 9a, Schritt 3, und Codetabelle). alt: „Die Suche liest dieselbe Zielliste und antwortet auf `zl.halter_name` weiter mit Exit 78 `mw.ziele_ungueltig`, wie auf jede ungültige Liste. Ob die Suche denselben eigenen Code bekommt, ist nicht entschieden."
36. **seccomp-Filter gegen Kinder (Abschnitt 3a, Pflicht 5, 25.11.2026).** Gebaut sind eine Prüfung im Bau und eine im Betrieb, die ein Kind erkennt und den Prozess beendet; ein Filter, der ein Kind verhindert, ist nicht gebaut. **Entschieden (25.11.2026): nicht im Zug sicherer Modus; die Erkennung bleibt.** Bevorzugter Weg ist ein eigener Zug über das Crate `rust/crates/kernsperre` des Arbeitsbereichs, das `seccompiler` schon trägt, also ohne neues fremdes Paket. Jede Änderung an einer `Cargo.toml` braucht dafür eine eigene Freigabe. Der Filter wird erst **nach** dem Ersetzen durch sich selbst aus Pflicht 3 gesetzt, weil Pflicht 3 genau ein `execve` braucht. Vorschlag für seine Form: `fork`, `vfork`, `execve`, `execveat` und `clone` ohne `CLONE_THREAD` mit `EPERM`, `clone3` mit `ENOSYS` (glibc fällt dann für Fäden auf `clone` zurück); `PR_SET_NO_NEW_PRIVS` setzt er erst nach dem setgid-`execve`, also ohne Folge für Abschnitt 3. Gemessen ist davon nichts.

37. **Ziel der Gegenprobe unter einem Systemnutzer des Halters (Abschnitt 9a, S2a, 25.11.2026).** In Fassung 1 ist `<heim>` in der Ablage des Halters das Heim aus dem eigenen Eintrag in `/etc/passwd`, darum findet der Läufer die Datei des Geheimnisses unter dem Heim der Person. Läuft der Halter später als eigener Systemnutzer (Halter-Vertrag, OPEN 9 und 18), ist `<heim>` dessen Heim. Dann zielt die Gegenprobe von S2a auf einen anderen Ort, und ob der Läufer dort einen Verweis anlegen darf, ist nicht festgelegt.

## 20. Frage an den Halter-Vertrag (Bau des Halterwegs, 25.11.2026)

Hier steht, wo dieser Vertrag und der Halter-Vertrag einander widersprachen. Der Bau hat nichts davon selbst entschieden
und nichts am Halter-Vertrag geändert. **Beide Fragen sind seit dem 25.11.2026 entschieden** (Inhaber des
Halter-Vertrags); ihr Text bleibt als Herkunft stehen, die Entscheidung steht je Frage am Ende.

**F1 (entschieden). `schluessel.vorhanden` nahm keinen `ziel`, aber Schritt 3 fragt je Ziel.**

- Dieser Vertrag: „für jedes `halter_ziel` der gebundenen Ziele `schluessel.vorhanden` rufen. Eine Antwort `false`,
  `halter.ziel_unbekannt` oder `halter.rolle` beendet den Start mit `mw.halter_kein_schluessel`"
  (`rust/spec/modellweg/README.md:151` und Abschnitt 13.5, Schritt 3). Dazu: „Der Modellweg nennt den Namen des
  Geheimnisses nicht; er steht je Modellziel in `halter.json`" (Abschnitt 5.1).
- Halter-Vertrag: Die Op hat genau ein Feld, `name`, den Namen eines Geheimnisses (`rust/spec/halter/README.md:499`
  bis `:507`); das Schema schließt jedes weitere Feld aus (`rust/spec/halter/halter.schema.json:184` bis `:204`), und
  `halter.ziel_unbekannt` ist der Code einer Op mit `ziel` (`rust/spec/halter/README.md`, Abschnitt 7, Stufe 14).
- Damit ist Schritt 3 so nicht ausführbar: Ohne den Namen kann der Modellweg die Op nicht rufen, und mit einem
  geratenen Namen bekäme er `false` und würde nie bereit. Der Bau fragt darum nicht und bleibt fail closed: Er
  verbindet sich, prüft die Gegenseite und wird bereit; fehlt das Geheimnis, endet die erste Anfrage an das Ziel mit
  503 `mw.halter_kein_schluessel` (Abschnitt 13.2, Stufe 9).
- Zwei Wege waren denkbar, beide gehörten dem Halter-Vertrag: Die Op bekommt ein Feld `ziel`, oder Schritt 3
  verliert die Frage je Ziel.
- **Entscheidung (25.11.2026): der erste Weg.** Die Op nimmt genau eines von `name` oder `ziel`; mit `ziel` löst der
  Halter den Namen selbst auf, und nur die Rolle `bruecke` fragt nach `ziel`. Unbekanntes Ziel: `halter.ziel_unbekannt`
  (nicht `false`). Bekanntes Ziel ohne Geheimnis: `{"vorhanden": false}`. Keines der beiden Felder:
  `halter.schluessel_fehlt` (Stufe 9). Beide Felder: `halter.schluessel_unbekannt` (Stufe 8). Die Codeliste des
  Halter-Vertrags bleibt geschlossen; dieser Vertrag nennt keinen Code, den sie nicht trägt (Korrektur des Inhabers,
  25.11.2026). Der Halter-Zug baut das; dieser Bau ruft die Op mit `ziel` **nicht**, weil der heutige Halter sie
  abweist. Der Nachzug steht als OPEN 34.
- **Gebaut (25.11.2026).** Nach dem Halter-Zug fragt Schritt 3 je gebundenes Ziel mit `ziel` (Abschnitt 13.5); OPEN 34 ist erledigt.

**F2 (entschieden). Das Feld `kopf` ist ein Objekt: keine Reihenfolge, kein doppelter Name.**

- Halter-Vertrag: `kopf` ist ein Objekt (`rust/spec/halter/README.md:517`, Feld von `modell.aufruf`), und die Bytes
  jeder Zeile sind die kanonische Form ihres Werts nach RFC 8785 (`rust/spec/halter/README.md:452`, Abschnitt 3).
- Dieser Vertrag sagte in Abschnitt 5.1 „die Reihenfolge bleibt". Das geht nicht: Ein kanonisches Objekt hat seine
  Namen sortiert, und einen Namen trägt es genau einmal. Abschnitt 5.1 nennt beide Folgen jetzt.
- **Entscheidung (25.11.2026): Es bleibt so.** `kopf` bleibt ein Objekt; die Reihenfolge der Köpfe geht über den
  Halter nicht, und ein doppelter Kopfname ist dort nicht darstellbar. Beides steht in Abschnitt 5.1 jetzt als Zusage,
  nicht mehr als offene Frage. Gemessen ist kein Fall, in dem die Copilot CLI einen Kopfnamen zweimal sendet oder in
  dem eine Reihenfolge zählt.

<!-- vom Bauvorgang angefuegt -->
> Hinweis: Dieser Text nennt Werkzeuge, die in diesem Klon nicht liegen: `agent-workbench` `brain`.
> Sie gehoeren zum Setup der Maschine, aus der dieser Baum stammt; hier fehlen
> sie mit Absicht. Wer sie braucht, schreibt sich seine eigenen.
> *Named here, not shipped here.*
