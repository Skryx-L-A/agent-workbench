# Vertrag `dienst`

Stand 19.11.2026 (Teil B); **Nachtrag 20.11.2026**; **Nachtrag 20.11.2026 (Durchstich)**: ein Dienst, mehrere Sitzungen, DI12 (alle Änderungen tragen „DI12“ oder „Nachtrag 20.11.2026“); **Nachtrag 20.11.2026**: Der Dienst prüft die Start-Struktur mit dem Crate `start` (Stufen 1 bis 17) und liest die Einstellungen mit dem Crate `einstellungen`; `Startwahl` trägt `modell` je Route und die Harness-Tabelle mit `copilot` (DI12 Punkte 3, 4 und 10). **Nachtrag 21.11.2026 (Zeitannahmen)**: Fälle und Kindprogramme ohne Zeitannahme (Abschnitt 15). **Nachtrag 21.11.2026**: Das Sitzungsende räumt auf (DI12 Punkt 12, Abschnitt „Projektentscheide zum Sitzungsende“); neuer Fall `di12_starten_groesste` („Weggefallene Prüfung“). **Nachtrag 22.11.2026**: Der Dienst ist unter Linux der Subreaper seiner Sitzungen und beendet die Waisen einer beendeten Sitzung (Abschnitt 17). **Nachtrag 22.11.2026**: `lauf` meldet seinen Code und Grund über den Socket `meldung.sock` an den Dienst, der Aufruf von `lauf` trägt `--meldung <pfad>` (Abschnitt 18; DI12 Punkte 3, 5 und 9). Jede Änderung des Nachtrags 20.11. trägt „Nachtrag 20.11.2026“ im Text und ihre Herkunft:
„Projektentscheid, 19.11.2026 F2 bis F5“ (Entscheidungen nach dem Cursor-Nachtrag und zu Teil B, Entwurf Abschnitt 11), „Cursor-Nachtrag“ (Cursor in `Ausgabe`, Verträge `traeger` TR2/TR3 und `zustandskern` R15) und „Projektentscheid, 20.11.2026“ (kleinste Größe 2 Spalten). Ausführbarer Vertrag des neuen Crates `dienst` und des Programms `wb-dienst`
(Teil des Plans zur Ansicht (außerhalb des Repos), Abschnitt 5). Grundlage sind die Projektentscheide (Entwurf
Abschnitt 7 und 8), die Verträge `zustandskern` (`VT`), `rasterauszug` (`RA`) und `traeger` (`TR`) und die Schrittweise API des Kerns
(`Sitzungskern`, `rust/crates/zustandskern/src/schritt.rs`).
Gebaut ist nur die Python-Referenz. Ein Rust-Bau gilt als vertragstreu, wenn er beide Fallsätze grün besteht (sichtbar und verdeckt).
„(H)“ heißt hergeleitet; „(V)“ heißt im Vertrag festgelegt, siehe Abschnitt 8.

Quellen: `VT` = `rust/spec/zustandskern/VERTRAG.md`, `RA` = `rust/spec/rasterauszug/VERTRAG.md`,
`TR` = `rust/spec/traeger/VERTRAG.md`, `EN` = der Entwurf oben.

## 1. Schnitt

Der Dienst hält **eine Sitzung** (mit `lauf` in der Startdatei beliebig viele, bis zu 8 gleichzeitig: DI12): ein Kind in einer PTY (über `traeger`), den `Sitzungskern`, je Betrachter einen Auszug (über `rasterauszug`),
eine Uhr und einen Unix-Socket. Er **übersetzt nur**: Jede Regel des Kerns bleibt im Kern, jede Regel des Rasterauszugs im Rasterauszug, jede des Trägers
im Träger. Der Dienst entscheidet nichts über Eingaben, Größen, OSC 52, Bündelung oder Rückstau; er reicht Ereignisse an den Kern, führt dessen Wirkungen
aus und macht aus seinen Nachrichten Rahmen auf dem Socket.

**`wb-dienst` ist ein Programm des Durchstichs** (Projektentscheid 6, `EN` Abschnitt 7; Nachtrag **Projektentscheid, 20.11.2026**: gilt „wird nicht ausgerollt“ für den Probelauf nicht mehr): Es wird nicht im Alltag des Betreibers gestartet, **läuft aber im Probelauf-Paket als Dienst des Benutzers** (`systemd --user`), nie als root-Dienst: ohne Root, ohne Fähigkeiten, ohne `unsafe`, ohne Netzwerk-Socket (der Dienst bleibt ohne TCP, die Brücke ist ein eigener Prozess).
Sein 0700-Ordner liegt unter `$XDG_RUNTIME_DIR/werkbank/` (je Benutzer, 0700, von systemd angelegt; Namen im Vertrag `bruecke`, Abschnitt 2). Die Person
steht in der Startdatei, der Socket hat nur Ordner- und Dateirechte (keine Prüfung des Gegenübers; die Peer-Prüfung kommt aus der Plattformschicht, `EN`
Abschnitt 6 Punkt 6). **Die fehlende Peer-Prüfung bleibt benannt: Im Probelauf ist der Ordner 0700 unter `$XDG_RUNTIME_DIR` die Grenze** (Projektentscheid, 20.11.2026). Das README des Crates sagt das in den ersten Zeilen.

Öffentliche API (Vorgabe für den Bau; die Außentests sehen nur das Programm und den Socket; Namen dürfen abweichen, das Verhalten nicht):

```
Startdatei::lesen(&Path) -> Result<Startdatei, StartFehler>     // DI1, Exitcode 2
rechte_pruefen(&Startdatei) -> Result<(), StartFehler>           // DI2, Exitcode 3
Dienst::neu(Startdatei) -> Result<Dienst, StartFehler>            // bindet den Socket, startet das Kind (Exitcode 1 oder 4)
Dienst::laufen(&mut self) -> i32                                  // Ereignisschleife bis SIGTERM oder SIGINT; Exitcode 0
```

Der Dienst nutzt Fäden und `std` (kein `tokio`, kein `axum`, kein `tungstenite`; `EN` Abschnitt 2). Kein TCP-Hörer (A120). Kein `unsafe`.
Die Referenz ist ein einfädiges Python-Programm; ein Bau darf Fäden benutzen, wenn die beobachtbare Folge nach DI4 bis DI8 dieselbe bleibt.

## 2. Aufruf

- `wb-dienst <startdatei.json>`: läuft, bis SIGTERM oder SIGINT eintrifft. Dann: Betrachter schließen, Socket-Datei entfernen, Kind beenden
  (SIGHUP, bis zu 1 s warten, dann SIGKILL, dann `waitpid`), Exitcode 0. Das Kindende allein beendet den Dienst nicht (DI8).
- `wb-dienst --kind <name> [argumente]`: führt ein **festes Kindprogramm** aus (Abschnitt 7, DI8). Es ist Teil des Programms (wie `tr --kind`); der Prüfclient setzt
  den Platzhalter `@dienst` in `kind.argv` auf den Pfad des Programms.
- Exitcodes beim Start (Reihenfolge der Prüfung: 2, dann 3, dann Kind, dann Bind):
  - **2** Aufruf oder Startdatei ungültig (DI1),
  - **3** Rechte am Ordner oder am Socket-Pfad (DI2),
  - **4** das Kind lässt sich nicht starten (Programm fehlt oder nicht ausführbar, Arbeitsverzeichnis fehlt; die Referenz meldet das über eine Röhre mit `CLOEXEC`),
  - **1** der Socket lässt sich aus einem anderen Grund nicht binden (kein Fall; das schon gestartete Kind wird dann beendet).
  Bei jedem Exitcode ungleich 0 beim Start steht **genau eine** nicht leere Zeile auf stderr, die mit `wb-dienst: ` beginnt; stdout bleibt leer; es bleibt keine Socket-Datei zurück.
  Nach einem Startfehler läuft kein Kind und kein Hörer.
- Keine Eingabe löst eine Panik aus.

## 3. Startdatei (DI1)

```json
{"socket":     {"ordner": "/abs/pfad", "name": "dienst.sock"},
 "betrachter": {"person": "anna", "raum": "raum-nord", "ansicht": "fenster"},
 "kind":       {"argv": ["/abs/programm", "arg"], "env": {"TERM": "xterm-256color"}, "cwd": "/"},
 "groesse":    [40, 10],
 "ruecklauf":  2000,
 "deflate":    true,
 "pruefmodus": false,
 "warteschlange_max": 33554432,
 "anfang":     {"personen": {…}, "sitzungen": [{"id": "s-alpha", …}], "regeln": {…}}}
```

- Pflicht: `socket`, `betrachter`, `kind`, `groesse`, `anfang` (mit dem neuen Schlüssel `lauf` sind `kind` und `groesse` nur zusammen oder gar nicht da, DI12). Vorgaben: `ruecklauf` 2000, `deflate` true, `pruefmodus` false, `socket.name` `dienst.sock`, `kind.cwd` `/`, `warteschlange_max` 33 554 432 (Nachtrag 20.11.2026, Projektentscheid, 19.11.2026 F4).
  **Jeder andere Schlüssel** auf der obersten Ebene, in `socket`, `betrachter` und `kind` ist ungültig (Tippfehler sollen auffallen).
- `socket.ordner`: absoluter Pfad, nicht `/`, ohne NUL. `socket.name`: ein einfacher Dateiname, nicht leer, nicht `.` oder `..`, ohne `/`, ohne Anfang `..`, höchstens 64 Byte.
  Der Pfad `ordner/name` hat höchstens **100 Byte** (Grenze von `sun_path` am Mac); mehr ist ungültig (Exitcode 2, nicht 3).
- `betrachter`: genau `person`, `raum`, `ansicht` (alle Text; `ansicht` ist `fenster` oder `browser`). `person` muss in `anfang.personen` stehen. Das ist die einzige Identität: Jeder
  Betrachter, der sich verbindet, ist diese Person in diesem Raum (`EN` Abschnitt 1: keine Anmeldung).
- `kind`: `argv` (1 bis 256 Texte, je 1 bis 4096 Byte, ohne NUL, `argv[0]` ein **absoluter Pfad**), `env` (höchstens 256 Einträge, Name nicht leer und ohne `=`, Werte Texte), `cwd` (absolut).
  Die Umgebung des Kindes ist **genau** `env` (`env_clear`), nichts sonst.
- `groesse`: `[spalten, zeilen]`, ganze Zahlen; **Spalten 2 bis 1000, Zeilen 1 bis 1000** (ein Wahrheitswert ist keine Zahl). Eine Größe mit **einer Spalte ist ungültig** (Exitcode 2; Nachtrag 20.11.2026, Projektentscheid, 20.11.2026: kleinste Größe 2 Spalten für die ganze Sitzung, Kern, PTY und Raster). Die kleinste Spaltenzahl steht in der Referenz an **einer** Stelle (`KLEINSTE_SPALTEN`; Startdatei, Ereignisse und Träger-Modell lesen dort). Die Verträge `zustandskern`, `traeger` und `rasterauszug` nennen noch „1 bis 1000“; ein eigener Nachtrag zieht sie nach. Bis dahin lehnt der Dienst Größen mit einer Spalte selbst ab (DI4). `ruecklauf`: ganze Zahl 0 bis 100 000 (Einstellung des Trägers, `TR` TR4).
  `deflate` und `pruefmodus`: Wahrheitswerte. `warteschlange_max` (Nachtrag 20.11.2026, nur zum Prüfen): ganze Zahl von 1 bis 33 554 432, **nur mit `pruefmodus` true**, sonst und bei jedem anderen Wert Exitcode 2. Sie senkt die Obergrenze der Warteschlange (DI5) für Fälle, die die Grenze Byte für Byte erreichen; ohne den Schlüssel gilt im Betrieb und im Prüfmodus 33 554 432.
- `anfang`: ein Objekt nach `VT` Abschnitt 3 mit **genau einer** Sitzung (`sitzungen` hat einen Eintrag mit Text `id`). Der Eintrag darf `zeilen`, `stand`, `bracketed_paste`, `invers` und `cursor` **nicht** nennen (sie gehören
  dem Träger; der Dienst setzt sie beim Neubau, DI6; `cursor` seit Nachtrag 20.11.2026, Projektentscheid, 19.11.2026 F2: den Anfangscursor setzt der Dienst, DI11). Nennt er `groesse`, muss sie gleich `groesse` oben sein. `anfang` muss `Sitzungskern::neu` annehmen.
- Die Datei hat höchstens **1 048 576 Byte**, ist UTF-8, ein JSON-Objekt, höchstens **128 Ebenen** tief, ohne `NaN` und `Infinity`. Eine Datei, die fehlt oder sich nicht lesen lässt: Exitcode 2.
- Alle Fehler dieses Abschnitts: Exitcode 2 mit einer Zeile auf stderr.

## 4. Rechte und Socket (DI2) — Sicherheitsstelle, das Projekt liest sie selbst

1. Der **Ordner** `socket.ordner` muss mit `lstat` ein Ordner sein: Fehlt er, ist er ein symbolischer Link (auch auf einen guten Ordner) oder ist er kein Ordner: Exitcode 3. Der Dienst legt den Ordner nie an.
2. Der Besitzer des Ordners muss der **effektive Benutzer** des Dienstes sein, sonst Exitcode 3. (Im Test herstellbar: ein Ordner eines anderen Benutzers mit Modus 0700, am Mac `/var/audit`, auf Linux `/var/lib/private` oder `/etc/ssl/private`; ein Lauf als `root` kann den Fall nicht herstellen.)
3. Der Modus des Ordners (`st_mode & 07777`) muss **genau 0700** sein. Jeder andere Wert ist Exitcode 3: 0750, 0755, 0777, 0701, 0770, 0600, 0500, 2700, 1700.
4. Der Socket-Pfad darf **nicht bestehen**: Jeder Eintrag mit diesem Namen (Datei, Socket, Ordner, auch ein hängender Link) ist Exitcode 3. Der Dienst ersetzt und löscht nie etwas, das schon da ist.
5. Der Socket hat den Modus **genau 0600**. Der Ordner mit 0700 macht die Zeit zwischen `bind` und `chmod` ungefährlich; wer `umask` nicht setzen kann, setzt den Modus gleich nach dem Binden.
6. **Kein TCP-Hörer, kein UDP, kein Netz-Socket irgendeiner Art.** Der Prüfclient zählt die Netz-Sockets des Dienstes mit `lsof -nP -a -p <pid> -i` und erwartet 0.
7. Nach SIGTERM oder SIGINT: die Socket-Datei ist weg, das Kind ist beendet, Exitcode 0. Bei einem Absturz bleibt die Datei stehen; der nächste Start lehnt sie nach Punkt 4 ab und der Betreiber räumt sie auf.
8. Der Dienst prüft das Gegenüber eines Betrügers nicht (keine Peer-Prüfung). Wer den Ordner betreten darf, darf sprechen.

## 5. Protokoll über den Socket (DI3 bis DI5)

Ein **Rahmen** ist `[n: u32 big endian][art: u8][nutzlast: n − 1 Byte]`. `n` zählt die Art und die Nutzlast, ist also mindestens 1.

| Richtung | Art | Name | Nutzlast |
|---|---|---|---|
| hinein | `0x01` | `Ereignis` | JSON-Objekt, ein Ereignis der Ansicht ohne `t` und ohne `betrachter` (unten) |
| hinein | `0x10` | `Uhr` | JSON `{"t": ganze Zahl ≥ 0}` (nur im Prüfmodus, DI9) |
| hinein | `0x11` | `Stumm` | JSON `{"an": bool}` (nur im Prüfmodus) |
| hinein | `0x12` | `Zustand` | leer (nur im Prüfmodus) |
| hinein | `0x13` | `Kernfehler` | leer (nur im Prüfmodus) |
| hinaus | `0x05` | `Raster` | die Bytes eines Rasterrahmens auf der Leitung nach `RA` (mit Deflate, wenn `deflate`), Sitzungsnummer **0** |
| hinaus | `0x06` | `Nachricht` | JSON-Objekt, jede Nachricht des Kerns außer `Abzug` und `Rasteraenderung` |
| hinaus | `0x07` | `Ablehnung` | JSON `{"grund": "<grund des Kerns>"}` |
| hinaus | `0x08` | `Fehler` | JSON `{"fehler": "<code>"}`, danach endet die Verbindung |
| hinaus | `0x11` | `Stumm` | leer, die Quittung auf `Stumm` (DI9) |
| hinaus | `0x12` | `Zustand` | JSON, Antwort auf `Zustand` (DI9) |

Die Nutzlast von `Raster` ist dieselbe, die der Browser später über WebSocket bekommt (`EN` Abschnitt 2). Der Rahmen nach `RA` beginnt in seinen entpackten Bytes selbst mit der Art 5; das ist die Art des Rasterrahmens, nicht die des Rahmens auf dem Socket.

### DI3 Rahmung und Fehler

- Die Rahmen dürfen in beliebigen Stücken ankommen (ein Rahmen in mehreren Lesevorgängen, mehrere Rahmen in einem); der Dienst setzt sie zusammen.
- Höchstens `n = 1 048 576` hinein. Ist `n` größer, endet die Verbindung mit `rahmen_zu_gross`, **sobald die vier Byte des Kopfes da sind** (der Dienst wartet die Nutzlast nicht ab). `n = 0`: `laenge_ungueltig`.
- **Grenze hinaus (Nachtrag 23.11.2026, Projektentscheid, 23.11.2026):** Ein Rahmen `Raster` hat höchstens `n = 16 777 217`: die Art und höchstens `RAHMEN_MAX` = 16 777 216 Byte des Rahmens nach `RA` (`rust/crates/rasterauszug/src/rahmen.rs:34`). Die Grenze gilt für den Rahmen vor Deflate, denn der Empfänger entpackt höchstens so viel (`RA` RA11, „Entpackt ein Rahmen zu mehr als 16 777 216 Byte“), und für die Bytes auf der Leitung (`RA` RA11, „Ohne Deflate gilt dieselbe Obergrenze“; mit Deflate hält sie `n` auf dem Socket in der Grenze). Das Crate `rasterauszug` begrenzt nur den Empfang; sein Kodierer schreibt einen Rahmen jeder Länge. Vor diesem Nachtrag hielt der Dienst die Grenze nicht: Er reihte jeden Rahmen des Kodierers ein (`rust/crates/dienst/src/dienst.rs:921`), und der Empfänger brach mit `rahmen_zu_gross` ab (mit Deflate ist sein Empfänger danach unbrauchbar). Ein `Abzug` von 1000 × 1000 Zellen mit eigenem Truecolor-Stil je Zelle und einem Zeichen aus vier Byte ist länger als `RAHMEN_MAX` (gemessen am Mac 23.11.2026: 16 987 373 Byte vor Deflate, also `n` = 16 987 374 ohne Deflate; mit Deflate 6 851 251 Byte auf der Leitung, der Empfänger entpackt aber zu mehr als `RAHMEN_MAX` und bricht ab). **Der Dienst prüft darum vor dem Einreihen** (`rust/crates/dienst/src/betrachter.rs`, `Raster::nimm`): Ist der Rahmen vor Deflate oder sind die Bytes auf der Leitung länger als `RAHMEN_MAX`, ist das der Fehler `raster_rahmen_zu_gross` nach DI3, wie jeder Fehler des Auszugs (`dienst.rs:923`): ein Rahmen `Fehler`, die Verbindung endet, andere Betrachter und der Kern bleiben. Der Rahmen vor Deflate wird zuerst geprüft, ein zu langer Rahmen geht nie durch Deflate. Fälle: `ein_rasterrahmen_ueber_rahmen_max_ist_rahmen_zu_gross` und `ein_kleiner_rasterrahmen_bleibt_wie_im_crate` in `betrachter.rs`. **Grenze hinaus für `Nachricht` (Nachtrag 23.11.2026, Projektentscheid, 23.11.2026, Option b):** Ein Rahmen `Nachricht` hat auf dem Socket **als Ganzes höchstens `RAHMEN_MAX` = 16 777 216 Byte**: Kopf (4 Byte), Art (1 Byte) und der JSON-Text des Dienstes mit allen Escapes (`json.rs`), also höchstens 16 777 211 Byte JSON. Gezählt werden die Bytes, die der Dienst wirklich schreibt, keine Schätzung. Der einzige Weg einer Nachricht des Kerns hinaus ist `nachricht_an` (`rust/crates/dienst/src/dienst.rs:928`), für Nachrichten des Raumkerns wie der Sitzungskerne, auch `Zwischenablage`. Die Grenze ist strenger als bei `Raster` (dort `n` bis `RAHMEN_MAX` + 1), weil der Auftrag den ganzen Rahmen nennt; jeder Empfänger mit der Grenze `RAHMEN_MAX` für `n` oder für die Nutzlast liest ihn. **Eine längere Nachricht geht nicht hinaus** (`rust/crates/dienst/src/betrachter.rs`, `json_rahmen`, seit dem Nachtrag zu allen JSON-Rahmen unten; vorher `nachricht_rahmen`). Der Betrachter bekommt stattdessen einen Rahmen `Nachricht` `{"art": "Hinweis", "hinweis": "<grund>", "sitzung": …, "t": …}`; `sitzung` und `t` stammen aus der Nachricht, wenn sie Text oder Zahl sind, sonst fehlen sie. Der Hinweis enthält kein Byte des Inhalts. Die Gründe stehen in Abschnitt 8 Punkt 19. **Die Verbindung bleibt**: Der Betrachter hat keinen Fehler gemacht, darum ist es kein Fehler dieses Abschnitts; der Kern erfährt nichts, die Meldung `Socket` folgt wie sonst. Die Grenze des Kerns `osc52_grenze` (Vorgabe 100 000 Byte roh, ohne Höchstwert, `rust/crates/zustandskern/src/regel.rs:32`) kann eine einzelne Zwischenablage über diese Grenze bringen (Base64: 4/3 der rohen Byte); die 35 MB von `di5_grenze_betrieb` sind die Summe vieler Nachrichten, keine einzelne. Fälle: `eine_ablage_an_der_grenze_geht_hinaus` (genau 16 777 216 Byte, geht unverändert hinaus), `eine_ablage_knapp_ueber_der_grenze_gibt_den_grund` (16 777 217 Byte, ohne und mit Escape: der feste Grund) und `eine_andere_nachricht_ueber_der_grenze_gibt_den_grund` in `betrachter.rs`. **Geänderte Erwartung:** alt: eine Nachricht jeder Länge ging als Rahmen `Nachricht` hinaus; neu: über 16 777 216 Byte Rahmen kommt der `Hinweis` mit festem Grund, die Verbindung bleibt; Grund: Projektentscheid, 23.11.2026 (kein Rahmen über `RAHMEN_MAX`, kein stiller Verlust). Kein Außenfall ändert sich (hergeleitet: keiner erzeugt eine einzelne Nachricht dieser Größe). **Grenze hinaus für alle JSON-Rahmen (Nachtrag 23.11.2026, Projektentscheid, 23.11.2026):** Dieselbe Grenze und dieselbe Zählweise gelten für **jeden** Rahmen hinaus mit JSON: `Nachricht`, `Ablehnung`, `Fehler` und `Zustand`. Ein Rahmen hat als Ganzes höchstens `RAHMEN_MAX` = 16 777 216 Byte (Kopf, Art, JSON-Text mit Escapes). Alle vier baut eine einzige Stelle, `json_rahmen` (`rust/crates/dienst/src/betrachter.rs`); `dienst.rs` ruft sie für `Ablehnung` (`ablehnen`), `Fehler` (`fehler`), `Nachricht` (`nachricht_an`) und `Zustand` (DI9) und bindet `json.rs` selbst nicht mehr ein. Der Fall `nur_hier_wird_json_gerahmt` prüft, dass keine andere Datei des Crates `json::bytes` aufruft, also kein JSON-Rahmen an der Grenze vorbei hinausgeht. `Raster` begrenzt `Raster::nimm` (oben), `Stumm` ist leer. Ist ein JSON-Rahmen länger, geht er nicht hinaus; der Betrachter bekommt stattdessen einen kurzen Rahmen derselben Art mit festem Grund und ohne ein Byte des Inhalts, und **die Verbindung bleibt**: bei `Nachricht` der `Hinweis` oben; bei `Zustand` `{"art": "Hinweis", "hinweis": "zustand_zu_gross"}` in einem Rahmen `Zustand` (dieselbe Form); bei `Ablehnung` `{"grund": "ablehnung_zu_gross"}`; bei `Fehler` `{"fehler": "fehler_zu_gross"}` (danach endet die Verbindung wie bei jedem `Fehler`). Die Gründe stehen in Abschnitt 8 Punkt 20. `Zustand` kann die Grenze erreichen: Er trägt `traeger_zeilen` (DI9), bei 1000 × 1000 Zellen mit Zeichen aus vier Byte schon ohne Stil mehr als 4 000 000 Byte, mit Stil je Zelle mehr als `RAHMEN_MAX` (hergeleitet aus dem gemessenen `Abzug` oben). **`Ablehnung` und `Fehler` erreichen die Grenze nie** (hergeleitet aus dem Code, Stand 23.11.2026): Eine `Ablehnung` trägt nur feste Wörter, entweder den Grund des Kerns, der `&'static str` ist (`rust/crates/zustandskern/src/kern.rs:26`, eingetragen `kern.rs:364`, weitergereicht `dienst.rs:655-656`), oder Wörter des Dienstes (`dienst.rs:1088`, `:1093`, `:1101`), oder die Ablehnung von `Starten` mit `grund`, `code` und `text` (`rust/crates/dienst/src/starten.rs:58-59`), deren Werte feste Texte sind (`starten.rs:40-46` mit `text_zu` `:64`, `starten.rs:49-55` mit `rust/crates/start/src/fehler.rs:46` und `:70`, beide `&'static str`). Ein `Fehler` trägt nur `{"fehler": <code>}` (`dienst.rs:905`); der Code ist ein Wort im Quelltext, ein Code von `KopfFehler` (`rahmen.rs`, `code`) oder `raster_` mit einem Code des Auszugs (`dienst.rs:923`, `rust/crates/rasterauszug/src/fehler.rs:102`, `&'static str`). Die gemeinsame Prüfung gilt für beide trotzdem. Fälle in `betrachter.rs`: `ein_zustand_an_und_knapp_ueber_der_grenze` (genau 16 777 216 Byte gehen unverändert hinaus, 16 777 217 Byte ergeben `zustand_zu_gross`, je ohne und mit Escape; die Daten entstehen zur Laufzeit), `ablehnung_und_fehler_ueber_der_grenze_geben_den_grund` und `nur_hier_wird_json_gerahmt`. Gegenprobe im Rust-Bau: Die Prüfung in `json_rahmen` aus, dann sind die Fälle über der Grenze rot; `ablehnen` baut den Rahmen wieder selbst mit `json::bytes`, dann ist `nur_hier_wird_json_gerahmt` rot (gemessen am 23.11.2026 mit je einer der beiden Änderungen). **Geänderte Erwartung:** alt: `Ablehnung`, `Fehler` und `Zustand` gingen in jeder Länge hinaus; neu: über 16 777 216 Byte Rahmen kommt derselbe Rahmen mit festem Grund; Grund: Projektentscheid, 23.11.2026, alle JSON-Rahmen zugleich zu begrenzen, kein Rahmen hinaus über `RAHMEN_MAX`. Kein Außenfall ändert sich (hergeleitet: keiner erzeugt einen dieser Rahmen in dieser Größe).
- Ein vollständiger Rahmen mit unbekannter Art: `art_unbekannt`. Eine Art des Prüfmodus ohne `pruefmodus`: `art_nicht_erlaubt`.
- Die Nutzlast eines `Ereignis` muss UTF-8 sein und ein JSON-Objekt (`NaN` und `Infinity` sind kein JSON): sonst `json_ungueltig`. Ist sie JSON, aber kein Objekt oder ohne Text `art`: `ereignis_ungueltig`.
  **Nachtrag 25.11.2026 (Tiefe):** Eine Nutzlast hat höchstens 127 Ebenen, sonst `json_ungueltig`. Einzige Ausnahme ist das Ereignis `Starten`: Es darf 128 Ebenen tief sein, weil die Brücke einen Körper bis 127 Ebenen in der Hülle `{"art": "Starten", "start": …}` weiterreicht (Vertrag `bruecke`, BR4). Jedes Glied für sich hat wieder höchstens 127 Ebenen; die Prüfung der Start-Struktur folgt danach wie sonst.
- Erlaubte Arten der Ansicht: `Abo`, `Groesse`, `Text`, `Taste`, `Einfuegen`, `Antwort`, `Freigabe`, `RouteWaehlen`, `EinstellungSetzen`, `RaumWechsel` (`VT` Abschnitt 4, Zeilen „Ansicht“ ohne `Verbinden` und `Trennen`).
  **Jede andere Art** (Träger, Kern, Stelle, Gateway, Uhr, auch `Verbinden`, `Trennen`, `Takt`, `Socket`, `Kindende`, `Ausgabe`): `art_nicht_erlaubt`. Ein Client darf nie das Ereignis eines anderen Absenders erfinden.
- Nennt das Ereignis `t` oder `betrachter`: `feld_verboten`. Der Dienst setzt beides (DI4, DI7).
- Prüffolge für einen vollständigen Rahmen: Art bekannt, Prüfart erlaubt, JSON, Objekt mit `art`, Art erlaubt, kein `t`, kein `betrachter`.
- **Jeder Fehler dieses Abschnitts beendet die Verbindung** (V): Der Dienst meldet `Trennen` an den Kern (der Betrachter ist sofort weg), hängt **einen** Rahmen `Fehler` hinter die schon wartenden Rahmen, liest nichts mehr von dieser Verbindung und schließt sie, sobald die Warteschlange geschrieben ist.
  Andere Betrachter und der Kern bleiben unberührt.
- Eine Verbindung, die mitten in einem Rahmen endet, ist ein einfaches Trennen (kein `Fehler`-Rahmen, kein Absturz).
- **Ablehnungen des Kerns** sind keine Fehler: Lehnt der Kern ein Ereignis ab (`taste_unbekannt`, `steuerzeichen_im_text`, `sitzung_beendet`, …), schickt der Dienst `Ablehnung` **nur an den Sender**, nach den Nachrichten desselben Schritts, und die Verbindung bleibt.
  Ablehnungen von Ereignissen, die der Dienst selbst gab (Träger, Takt, Socket), erreichen keinen Betrachter.
- **Größe mit einer Spalte (Nachtrag 20.11.2026, Projektentscheid, 20.11.2026):** Trägt ein `Abo` (mit `spalten`) oder `Groesse` eine ganze Zahl `spalten` kleiner als 2, lehnt der Dienst es ab, **bevor** der Kern es sieht: `Ablehnung` mit dem Grund `groesse_ungueltig` nur an den Sender, die Verbindung bleibt, der Kern bekommt nichts (derselbe Grund wie im Kern für Größen außerhalb des Bereichs; bis der Nachtrag zum Kern 2 Spalten selbst verlangt, steht die Prüfung hier). Das Träger-Modell lehnt eine Größe mit einer Spalte selbst ab und klemmt nie still.
- **Felder von `Groesse` (Nachtrag 22.11.2026, Projektentscheid, 22.11.2026):** Die Ansicht schickt `{"art": "Groesse", "sitzung": <Kennung>, "spalten": <ganze Zahl>, "zeilen": <ganze Zahl>}`.
  `sitzung`, `spalten` und `zeilen` sind Pflicht, `zeilen` gehört dazu. Die Felder sind die der Zeile `Groesse` in `VT` Abschnitt 4 ohne `betrachter`; `betrachter` und `t`
  setzt der Dienst (`dienst.rs:1035-1036`, `ereignis_rahmen`). Gelesen im Code von `development` vom 22.11.2026:
  - `spalten` kleiner als 2: `groesse_ungueltig` vom Dienst, bevor der Kern es sieht (Punkt oben, `dienst.rs:1039-1044`, `ereignis_rahmen`).
  - Ohne `Abo` dieses Betrachters auf die Sitzung: `kein_abo`. Fehlt `spalten` oder `zeilen`, oder liegt `spalten` nicht in 2 bis 1000 oder `zeilen` nicht in 1 bis 1000:
    `groesse_ungueltig` vom Kern (`zustandskern/src/ereignis.rs:222-237`, `:87-100`, Grenzen `kern.rs:15-20`).
  - Im Modus `lauf` ohne bekannte `sitzung`: `sitzung_unbekannt` (`dienst.rs:1048-1056`).
  - Wirkung: Der Kern merkt sich die gemeldete Größe des Betrachters und setzt sie nur, wenn dieser Betrachter die letzte Eingabe gab (`ereignis.rs:233-235`). Der Dienst
    merkt sie sich für DI6 nur bei bestehendem `Abo` (`dienst.rs:185-190`, `abo_nachfuehren`).
- Die Wirkungen `protokoll`, `an_stelle` und `abfragen` des Kerns gibt der Dienst in dieser Stufe nicht weiter (V; kein Gateway, keine Stelle, keine Abfrage).

## 6. Fall und Ergebnis des Prüfclients (DI10)

Der **Prüfclient** (`pruefclient.py`, Python 3 mit Standardbibliothek; Dekoder ist `referenz/ra_ref.py`, die Referenz des anderen Vertrags, nie ein Mutant) liest einen Fall, schreibt die Startdatei, startet `DI_BIN`, spricht als ein oder mehrere Betrachter über den Socket und schreibt **eine** JSON-Zeile.
Umgebung: `DI_BIN` (Pflicht), `DI_TMP` (Ordner für die Wegwerf-Ordner, Vorgabe `/tmp`; der Socket-Pfad bleibt unter 100 Byte), `DI_REF_DIR` (Ordner mit `ra_ref.py`). Er gibt `REF_GEGENPROBE` nur an den Dienst weiter. Exit 0: Ergebnis geschrieben; 2: Fall ungültig; 3: Umgebung falsch (kein `DI_BIN`, kein `lsof`).

```json
{"name": "…", "regeln": ["DI3"], "datensatz": "A", "beschreibung": "…",
 "ordner": {"art": "verzeichnis|fehlt|link|datei|absolut", "modus": "0700", "absolut": ["/var/audit", "/var/lib/private"], "socket_vorher": "datei|link"},
 "start": { …Startdatei… }, "start_roh": "…", "start_auffuellen": 0, "kind_pruefen": false,
 "schritte": [ … ]}
```

- Jeder Fall hat einen Wegwerf-Ordner `/tmp/di-XXXXXXXX/` (Modus 0700) und darin den Socket-Ordner `o` (Modus nach `ordner.modus`, `fehlt`: nicht angelegt; `link`: Link auf einen guten Ordner; `datei`: eine Datei; `absolut`: der erste Ordner der Liste, der einem anderen Benutzer gehört und den Modus 0700 hat, nicht angelegt).
  `socket_vorher` legt vor dem Start eine Datei oder einen hängenden Link am Socket-Pfad an. Am Ende wird alles gelöscht.
- In `start` und `start_roh` ersetzt der Prüfclient `@ordner` (Pfad des Socket-Ordners) und `@dienst` (absoluter Pfad von `DI_BIN`). `start_roh` ist der Text der Startdatei (für ungültige Dateien); `start_auffuellen` hängt so viele Leerzeichen an (gültiges JSON, aber eine größere Datei).
- `kind_pruefen`: Der Prüfclient merkt sich die letzte Zeile `PID=<n>` im Abbild und prüft nach dem Ende, ob dieser Prozess weg ist.
- `limit` (Nachtrag 21.11.2026): die Obergrenze in Sekunden für den ganzen Fall, wenn er so viel feste CPU-Arbeit hat, dass eine ausgelastete Maschine ihn weit dehnt (`di5_grenze_betrieb`: 35 MB Zwischenablage durch Kern und Träger, 19 s bei 16 Kernen unter Last 50, über 60 s bei Last 110). Der Läufer nimmt das Größere aus `DI_ZEIT` und `limit`. Sie beendet nur das Warten auf einen Zustand, der nicht kommt; ein Fall wird durch sie nie grün.

**Schritte** (je Schritt ein Ergebnis, in der Reihenfolge):

| `art` | Felder | Wirkung |
|---|---|---|
| `Verbinden` | `betrachter` (Name im Fall, nicht der des Dienstes), `deflate` (Vorgabe wie die Startdatei), `lesen` (Vorgabe true; Nachtrag 20.11.2026) | verbindet (Wiederholung bis 5 s), wartet auf das erste Rahmen oder das Ende. Mit `lesen` false liest der Prüfclient von diesem Betrachter nichts und wartet nicht (DI5: ein Betrachter, der nichts abnimmt). Ergebnis `{"ok": true}` |
| `Lesen` | `betrachter`, `an` (Vorgabe true) | beginnt oder beendet das Lesen eines Betrachters (Nachtrag 20.11.2026). Ergebnis `{"ok": true}` |
| `Ereignis` | `betrachter`, `ereignis` | sendet einen Rahmen `Ereignis` |
| `Rahmen` | `betrachter`, `art_rahmen` (Name oder Zahl), `json` oder `hex`, `auffuellen_bis` | sendet einen beliebigen Rahmen; `auffuellen_bis` füllt die Nutzlast mit Leerzeichen bis zur Rahmenlänge `n` |
| `Roh` | `betrachter`, `hex` | schreibt rohe Byte (für Kopf und Teilrahmen) |
| `Uhr`, `Stumm`, `Kernfehler` | `betrachter` (Vorgabe: der erste), `t` / `an` | sendet den Rahmen des Prüfmodus |
| `Trennen` | `betrachter` | schließt die Verbindung von der Seite des Clients |
| `Pause` | `sekunden` | liest nebenbei; nur als Atempause zwischen Teilen eines Rahmens |
| `Warte` | `betrachter`, `bis`, `frist` (Vorgabe 20 s), `takt` | wartet, bis die Bedingung gilt: `{"text": s}` (eine Zeile des Abbilds enthält s, Klartext ohne Stil), `{"kindende": true}` (letzte `Sitzungsliste` zeigt einen Zustand ungleich `laeuft`), `{"titel": s}`, `{"nachricht": {Feld: Wert}}` (Teilvergleich einer Nachricht seit dem letzten `Zustand`), `{"ablehnung": grund}`, `{"fehler": code}`, `{"getrennt": true}`, `{"cursor": [zeile, spalte, sichtbar]}` (das Abbild zeigt diesen Cursor; Nachtrag 20.11.2026). Fehler: `{"ok": false, "fehler": "zeit"}` oder `"getrennt"`. `takt` > 0 im Prüfmodus: bei 20 ms ohne Rahmen schickt der Prüfclient eine `Uhr` um `takt` ms weiter |
| `Zustand` | `betrachter`, `mit_t`, `ohne_nachrichten`, `nachrichten_aus`, `voll`, `grenze` | im Prüfmodus: sendet `Zustand` und liest bis zur Antwort (eine **Schranke**: alles, was der Dienst vorher an diesen Betrachter geschickt hat, ist gelesen). Sonst liest er bis 150 ms Ruhe |

**Ergebnis** insgesamt: `{"start": {…}, "ergebnisse": […], "ende": {…}}`.
- `start`: ist der Start gescheitert: `{"ok": false, "exitcode": n, "stderr_zeilen": z, "stderr_beginn": bool, "socket_da": bool}` (`stderr_beginn`: erste Zeile beginnt mit `wb-dienst: `). Sonst `{"ok": true, "ordner_modus": "0700", "socket_modus": "0600", "netz_sockets": 0}`.
- `ergebnisse`: bei gescheitertem Start leer.
- `ende`: `{"exitcode": …, "socket_entfernt": bool, "kind_beendet": bool|null}` nach SIGTERM (10 s Frist, sonst `"kill"`), und, wenn der Fall `"protokoll": true` trägt, `"protokoll": [{"betrachter": "b2", "grund": "rueckstau_gerissen"}, …]`: die Zeilen des Protokolls des Dienstes (stderr, DI5) der Form `wb-dienst: betrachter <name> getrennt: <grund> (…)`, in der Reihenfolge, ohne die Zahlen (Nachtrag 20.11.2026). Trägt der Fall `"geheim": [Wort, …]`, steht dazu `"protokoll_ohne_inhalt": bool`: wahr, wenn keines der Wörter (auch nicht in Base64) in einer Zeile von stderr steht (Projektentscheid, 20.11.2026).
- Ergebnis von `Zustand`: `{"ok": true, "abbild": {"nr", "groesse", "invers", "cursor": [zeile, spalte, sichtbar], "zeilen": ["<Zeilenform>", …]} | null, "nachrichten": […] oder eine Zahl, "ablehnungen": […], "fehler": […], "abbild_fehler": […], "getrennt": bool, "antwort": <Antwort des Dienstes> | null}`.
  `nachrichten_aus` setzt `nachrichten` auf `null` (die Zahl hängt im Betrieb vom Kernelpuffer ab, DI5). `nachrichten` sind die Nachrichten (Art 6) **seit dem letzten `Zustand` dieses Betrachters**, ohne das Feld `t`, außer bei `mit_t`; danach sind die Listen leer. `abbild_fehler` nennt den Code, wenn der Dekoder einen Rahmen abgelehnt hat.
  **Nicht verglichen** wird, was von der Stückelung der PTY abhängt (`TR` TR12): kein Rahmenbyte, kein `stand`, keine Zahl der Rahmen. **Der Cursor wird verglichen** (Nachtrag 20.11.2026, Cursor-Nachtrag: der Träger meldet ihn in `Ausgabe`, DI11): `abbild.cursor` und `kern.sitzungen[].cursor`; Letzterer hängt wie `zustand` und `code` davon ab, ob die Ausgabe schon angekommen ist, und wird darum ohne `voll` entfernt. Die Zahl der Byte in der Warteschlange (`dienst.betrachter.<n>.warteschlange`, `ausstehend`) hängt von den Rahmenbyte ab; der Rahmen `Rasteraenderung` trägt seit dem Cursor `cursor` in jedem Rahmen, in dem er sich ändert (`RA`), darum sind die Zahlen der Fälle `di5_stumm_*` um 3 bis 6 Byte je Bild größer als vor dem Nachtrag.
  `dienst.warteschlange_max` (die Grenze, DI5) steht in der Antwort **nur**, wenn der Schritt `grenze` trägt (sonst entfernt der Prüfclient es).
- Verglichen wird das **Abbild nach Ruhe**, die Folge der übrigen Nachrichten, Ablehnungen, Fehler, Exitcodes, Rechte und der Zustand des Dienstes (DI9).
- Jeder Fall wartet vor einem Vergleich auf ein Ende, das von der Zeit nicht abhängt: Das Kind startet **nie von selbst** (DI8: erst nach einem Startbyte des Clients), und jede Ausgabe, die der Fall prüft, hat eine Marke, auf die er wartet.

## 7. Regeln

### DI4 Betrachter

- Jede Verbindung, die der Dienst annimmt, ist ein Betrachter mit dem Namen `b<n>`; `n` zählt ab 1 in der Reihenfolge des Annehmens und wird **nie** wieder vergeben.
- Beim Annehmen gibt der Dienst dem Kern `Verbinden` mit `betrachter`, `person`, `raum`, `ansicht` aus der Startdatei. Dann gehen die Nachrichten des Kerns hinaus (`Einstellungen`, `Routen`, `Sitzungsliste`, `Anfragen`, `Agenten`). Ein Raster kommt erst nach einem `Abo` (`VT` R4).
- Jedes Ereignis der Ansicht geht mit `betrachter` gleich dem Namen der Verbindung und `t` gleich der Uhr (DI7) an den Kern. Der Dienst merkt sich das letzte angenommene `Abo` (und `Groesse`) mit der gemeldeten Größe des Betrachters (für DI6).
- **Nachrichten aus dem Kern** (in Reihenfolge des Kerns je Betrachter): `Abzug` und `Rasteraenderung` gehen durch `Verbindung::nimm(0, …)` des Betrachters (`RA` RA5 bis RA13, Sitzungsnummer 0, ein Kontext je Verbindung, `deflate` nach der Startdatei) und werden, wenn ein Rahmen entsteht, als Art 5 geschickt; entsteht keiner, wird nichts geschickt. Alle anderen gehen als Art 6. Lehnt `nimm` eine Nachricht ab, ist das ein Fehler des Dienstes: `Fehler` mit dem Code `raster_<code>` und Ende der Verbindung.
- Reihenfolge der Wirkungen eines Schritts: erst `groessen`, dann `pty` (DI8), dann die Nachrichten je Betrachter nach Nummer des Betrachters, dann die `Ablehnung` an den Sender.
- **Größenwechsel mit mehreren Betrachtern (Nachtrag 21.11.2026):** Die Ereignisse, die der Träger für eine neue Größe meldet (abgeschnittene oder umbrochene Zeilen, Cursor), gibt der Dienst erst **nach** den Nachrichten und der `Ablehnung` desselben Schritts an den Kern (sie sind der nächste Schritt). So bekommt jeder Betrachter zuerst den `Abzug` für die neue Größe und danach die `Rasteraenderung` gegen diesen. Ein Größenwechsel beendet keine Verbindung: kein `Fehler` `raster_luecke`, `raster_cursor_ungueltig` oder `raster_kein_abzug` für einen Größenwechsel, auch nicht bei zwei `Groesse` hintereinander oder bei Betrachtern mit verschiedener gemeldeter Größe. Ein echter Fehler im Strom bleibt ein Fehler (DI4, `RA` RA12).
- **Trennen** (Ende der Verbindung, Lesefehler, Schreibfehler, nach einem Fehler nach DI3): `Trennen` an den Kern, höchstens einmal je Betrachter. Der Auszug des Betrachters endet mit der Verbindung. Ein neuer Betrachter braucht ein neues `Abo` und bekommt einen vollen Abzug (`VT` R4, `RA` RA13).
- Die Sitzung überlebt jeden Betrachter. Keine Verbindung und keine Trennung ändert die Sitzung oder das Kind.

### DI5 Rückstau (`VT` R14)

- Jeder Betrachter hat eine **Warteschlange** der Rahmen, die der Dienst für ihn angenommen und der Socket noch nicht abgenommen hat. Geschrieben wird nicht blockierend, ein Rahmen darf teilweise geschrieben werden, der Rest bleibt vorn in der Schlange, die Reihenfolge bleibt. Der Dienst blockiert nie an einem Betrachter.
- **`ausstehend`** eines Betrachters ist die Zahl der Byte der Warteschlange: Summe der **ganzen Rahmen auf dem Socket** (Kopf von 5 Byte plus Nutzlast) für alle nicht oder nur teilweise geschriebenen Rahmen (bei einem teilweise geschriebenen Rahmen der Rest). Die Byte im Kernelpuffer des Sockets zählen **nicht** (offener Messpunkt, Abschnitt 10). Rahmen `Ablehnung`, `Fehler` und `Zustand` zählen mit, lösen aber keine Meldung aus.
- **Meldung `Socket`:** Nach **jeder Nachricht des Kerns**, die der Dienst für einen Betrachter behandelt (auch wenn kein Rahmen entstand), und nach **jedem Schreiben, das die Schlange verkürzt hat**, wenn sie davor nicht leer war, gibt der Dienst dem Kern ein Ereignis `Socket` mit `betrachter` und `ausstehend` (Wert in diesem Augenblick, nach dem Versuch zu schreiben) (V).
  Die Meldungen gehen **breitenweise**: Sie warten, bis alle Nachrichten des laufenden Schritts behandelt sind, und werden dann der Reihe nach gegeben; deren Nachrichten kommen hinter die schon geplanten (so bleibt die Folge `von`/`stand` des Kerns je Betrachter unverändert). Eine Meldung für einen schon getrennten Betrachter entfällt.
- Der Dienst verwirft nie ein Rahmen der Schlange. Ob ein Bild verfällt, entscheidet allein der Kern nach `ausstehend` (`VT` R14 Punkt 2).
- Ein langsamer Betrachter bremst keinen anderen (V): Solange ein Betrachter nichts abnimmt, laufen Kern, Träger und alle anderen Betrachter weiter.
- **Prüfmodus `Stumm`:** Ein stummer Betrachter bekommt nie ein Byte geschrieben (der Kernelpuffer gilt als 0 Byte groß). Alles bleibt in seiner Schlange, `ausstehend` wächst, der Kern lässt Bilder verfallen. Mit `Stumm` aus schreibt der Dienst die Schlange, meldet und der Kern holt nach (`VT` R14 Punkt 3): Das Abbild des Betrachters wird vollständig, ohne Lücke.
  Der Rahmen `Zustand` geht wie jeder Rahmen durch die Schlange; ein stummer Betrachter bekommt die Antwort erst nach `Stumm` aus (der Prüfclient nimmt für die Schranke einen anderen Betrachter).

- **Harte Obergrenze (Nachtrag 20.11.2026, Projektentscheid, 19.11.2026 F4):** Je Betrachter gilt die Obergrenze **33 554 432 Byte** für `ausstehend` (die Byte der Warteschlange, gezählt wie oben; der Kernelpuffer zählt nicht). Sie ist ein Sicherheitsnetz neben R14, nicht dessen Ersatz.
  - **Wann:** Nach jedem Rahmen, den der Dienst in die Schlange gelegt und für den er das Schreiben versucht hat, ist die Schlange **größer** als die Grenze (`ausstehend` > Grenze). Genau die Grenze gilt noch; ein Byte darüber trennt.
  - **Was:** Der Dienst verwirft die Schlange, meldet `Trennen` an den Kern (der Betrachter ist sofort weg), schließt den Socket (kein `Fehler`-Rahmen: die Schlange ist voll und wird nicht mehr geschrieben) und schreibt **eine Zeile** in sein Protokoll, das ist stderr: `wb-dienst: betrachter <name> getrennt: rueckstau_gerissen (ausstehend <n> Byte, Grenze <m> Byte)`. Der Prüfclient wertet nur Name und Grund aus (`ende.protokoll`); die Zahlen hängen im Betrieb vom Kernelpuffer ab. Andere Betrachter, der Kern und das Kind bleiben unberührt. Beides ist bestätigt (Projektentscheid, 20.11.2026): Trennen **ohne** `Fehler`-Rahmen, Protokoll des Dienstes ist stderr.
  - **Die Protokollzeile enthält keine Inhalte der Sitzung** (Projektentscheid, 20.11.2026, Auflage): nur den Namen des Betrachters, den Grund und die Zahlen; keine Byte aus Zeilen, Titel, Zwischenablage, Eingaben oder der Startdatei. Der Prüfclient prüft das mit dem Fall-Feld `geheim` (Wörter, die in der Sitzung vorkommen, auch in Base64): `ende.protokoll_ohne_inhalt` muss wahr sein (Fälle `di5_grenze_drueber` mit Titel und Zeilen, `di5_grenze_betrieb` mit Zwischenablage; Mutant `stau_grenze_protokoll_mit_inhalt`).
  - **Neuverbinden:** Der getrennte Betrachter verbindet sich neu, wie jeder andere, mit neuem Namen (`b<n>` wird nie wieder vergeben, DI4); nach dem `Abo` bekommt er einen **vollen Abzug** (`VT` R4, `RA` RA13), das Abbild gleicht dem des Trägers.
  - **Wie ein Fall die Grenze erreicht, ohne die Maschine zu fragen:** (1) Im Prüfmodus senkt die Startdatei `warteschlange_max` die Grenze; ein `Stumm` gestellter Betrachter nimmt nichts ab (Kernelpuffer 0 Byte), die Rahmen der Bilder, die der Fall der Reihe nach auslöst, sind Rasterrahmen ohne Deflate (feste Byte nach `RA`, nicht JSON). Die Fälle `di5_grenze_genau` (Schlange genau gleich der Grenze: bleibt) und `di5_grenze_drueber` (Grenze um ein Byte kleiner: trennt) stehen auf der Länge, die die Referenz misst; ein Bau mit anderen Rahmenbytes fällt dort rot. (2) **Der Wert im Betrieb steht fest:** Ohne `warteschlange_max` gilt 33 554 432 (der Fall `di5_grenze_vorgabe` liest ihn mit `Zustand` und `grenze` aus der Antwort `dienst.warteschlange_max`); der Fall `di5_grenze_betrieb` läuft **ohne Prüfmodus** und mit der echten Grenze: Ein Betrachter, der nie liest, bekommt Inhalte der Zwischenablage (OSC 52, je 90 000 Byte, 292 Stück, zusammen mehr als die Grenze plus jeder Kernelpuffer) und wird getrennt, ein lesender Betrachter bleibt, der neue Betrachter bekommt das volle Abbild. Die Zahl der Rahmen bis zum Trennen hängt dort vom Kernelpuffer ab und wird nicht verglichen.

### DI6 Fehler im Kern (`VT` Abschnitt 7 Punkt 27)

Gibt ein Schritt des Sitzungskerns einen Fehler (`Err`, etwa über der Grenze von 33 554 432 Byte Nachrichten je Schritt), ist der Kern verbraucht. Der Dienst **baut ihn neu** und geht so vor (V):
1. `Sitzungskern::neu` mit `anfang` der Startdatei, dabei in der Sitzung `groesse` = die jetzige Größe des Trägers, `zeilen` = die Zeilen, die der Träger dem Kern zuletzt gemeldet hat, `stand` = der letzte `stand` des Trägers, `bracketed_paste` und `invers` = die Modi des Trägers.
2. Ist das Kind schon zu Ende: `Kindende` mit dem Code.
3. Für jeden Betrachter in der Reihenfolge der Nummern: `Verbinden` (gleicher Name, gleiche Person), und hatte er ein Abo: `Abo` mit seiner zuletzt gemeldeten Größe.
4. Die Wirkungen des Fehlerschritts gehen **verloren** (der Fehler hat sie nie ausgegeben); die letzte Eingabe geht verloren.
1a. (Nachtrag 20.11.2026, Projektentscheid, 19.11.2026 F2, **berichtigt am 20.11.2026**) In `anfang.sitzungen[0]` steht außerdem `cursor`: **der Cursor, den der Träger dem Kern zuletzt gemeldet hat** (anfangs `[0, 0, true]`, DI11), dieselbe Quelle und derselbe Zeitpunkt wie `zeilen` und `stand`. Der Wortlaut „aus `zustand`“ am 19.11.2026 F2 gilt nicht mehr: Der `cursor` aus `zustand` des alten Kerns weicht ab, wenn eine Synchronisierung (DEC 2026) wartet, denn der Kern hält Zeilen **und** Cursor bis zur Freigabe zurück; der neue Kern bekommt dann Zeilen und Cursor des Trägers, beide vom selben Zeitpunkt (Fall `di6_kernfehler_sync`, Mutant `kernfehler_cursor_aus_zustand`). Ohne den Cursor zeigte jeder Betrachter bis zur nächsten `Ausgabe` einen unsichtbaren Cursor (`RA` R6).
Jeder Betrachter bekommt dann die Nachrichten des Verbindens (`Einstellungen`, `Routen`, `Sitzungsliste` mit dem jetzigen Zustand, `Anfragen`, `Agenten`) und einen **vollen Abzug**, als hätte er neu abonniert; sein Auszug (`RA`) nimmt den Abzug an, gleich welchen Stand er hatte. Sein Abbild gleicht danach dem des Trägers, **auch im Cursor**. Die Verbindungen bleiben.

### DI7 Uhr (`VT` Abschnitt 1)

- **Betrieb (`pruefmodus` false):** Die Uhr ist monoton, in ganzen ms seit dem Start, nie fallend. Jedes Ereignis trägt die Uhr. Der Dienst ruft den Kern an `naechste_frist()`: Ist sie fällig, gibt er ein `Takt` mit der jetzigen Zeit. Die Schleife wartet höchstens bis zur nächsten Frist. Ein gebündeltes Bild (R5) kommt also ohne Zutun eines Clients an.
- **Prüfmodus:** Die Uhr beginnt bei 0 und ändert sich **nur** durch einen Rahmen `Uhr` (jeder Betrachter darf ihn senden). `t` ist eine ganze Zahl ab 0 und nicht kleiner als die jetzige Uhr, sonst `uhr_faellt` (Fehler nach DI3, die Uhr bleibt). Der Dienst setzt die Uhr, gibt dem Kern ein `Takt` mit `t` und führt die Wirkungen aus. Der Dienst weckt sich im Prüfmodus nie selbst für Zeitgeber.
  Nutzlast ungleich `{"t": n}` (Zahl mit Bruch, Text, negativ, weitere Schlüssel, kein Objekt): `uhr_ungueltig`.
- Das Feld `t` aller Ereignisse des Dienstes (Verbinden, Trennen, Ereignisse der Ansicht, des Trägers, `Socket`, `Takt`) ist die Uhr in diesem Augenblick. Die Uhr geht nie zurück.

### DI8 Kind

- Der Dienst startet das Kind **nach** Prüfung der Startdatei und der Rechte und **vor** dem Binden des Sockets (so gibt es nie einen Socket, solange ein Startfehler des Kindes noch möglich ist; ein Prüfclient, der auf den Socket wartet, sieht nie einen Dienst, der gleich mit Exitcode 4 endet). Das Kind liest nichts, bevor der Dienst die PTY liest; es schreibt in den PTY-Puffer. Es läuft in einer PTY von der Größe `groesse`, in einer eigenen Sitzung mit der PTY als Steuerterminal, mit `argv`, **genau** der Umgebung `env` und dem Arbeitsverzeichnis `cwd`. Es bekommt keine weitere Variable.
- Die Ausgabe der PTY geht stückweise an den Träger (`TR` TR2). Dessen Ereignisse gehen mit `sitzung` und `t` an den Kern. Das Ende des Kindes meldet er **nach** der letzten Ausgabe als `Kindende` mit `code`: dem Exitcode (0 bis 255) oder, endet das Kind durch ein Signal, **128 plus die Signalnummer** (`TR` Abschnitt 7 Punkt 11). Danach bleibt die Sitzung lesbar; der Kern lehnt Eingaben mit `sitzung_beendet` ab; der Dienst läuft weiter.
- Die Wirkung `pty` des Kerns schreibt die Bytes **unverändert** an den Master der PTY (kein Umwandeln, kein Ergänzen), nicht blockierend (ein volles Eingabepuffer lässt den Rest in einer Schlange warten). Eingaben sind PTY-Byte nach `VT` R1; der Dienst kennt keine Taste.
- Die Wirkung `groessen` setzt die Größe der PTY (`TIOCSWINSZ`) **und** gibt sie dem Träger. Das geschieht **vor** dem Schreiben der `pty`-Bytes desselben Schritts, so dass das Kind nach einer Eingabe schon die neue Größe liest (V; `VT` R3: nur der Kern setzt die Größe). Die Ereignisse des Trägers nach der Größenänderung (`TR` TR3) gehen an den Kern.
- Der Dienst führt kein `GroessenFolge` aus (der Kern lehnt es ab, `VT` R3).
- **Feste Kindprogramme** (`wb-dienst --kind <name> …`; Standardbibliothek, ohne Hilfsprogramme; **Nachtrag 24.11.2026**: nur im Prüfbau, also mit dem Merkmal `pruefbau`; der Lieferbau kennt `--kind` nicht, behandelt es wie jedes unbekannte Argument und enthält keinen Code der Kindprogramme): Jedes setzt das Terminal auf roh ohne Echo, schreibt `BEREIT` mit `\r\n`, liest **ein Startbyte** und handelt dann. So läuft keine Ausgabe vor dem Betrachter und keine Folge hängt vom Zeitverhalten ab. Der Fall wartet auf `BEREIT` und schickt dann das Startbyte (ein `Text` `g`).
  - `ausgabe <hex>`: schreibt die Byte, endet mit 0.
  - `geteilt <hex1> <hex2> [s]`: schreibt die ersten Byte, wartet 3 ms, schreibt die zweiten (zwei getrennte Lesevorgänge der PTY; die zweite Ausgabe liegt in der Bündelungsfrist der ersten); mit `s` wartet es danach so lange, sonst endet es.
  - `ausgabewarte <hex> <s>`: schreibt die Byte und wartet s Sekunden (das Kind endet nicht von selbst).
  - `ende <code>`: endet mit dem Code. `signal <nr>`: sendet sich selbst das Signal.
  - `zeilen <n>`: schreibt `Z000001` bis `Z<n>` (6 Ziffern), je mit `\r\n`, endet mit 0.
  - `eingabe <n>`: liest `n` weitere Byte, schreibt `EINGABE <hex>`.
  - `klammer <n>`: schreibt `ESC[?2004h` und `KLAMMER`, liest `n` Byte, schreibt `EINGABE <hex>`.
  - `bilder <n>`: je weiteres Byte ein Bild (`ESC[H`, vier Zeilen `Bild kkkk Zeile j ` plus 40 mal `x`), nach einem weiteren Byte `ENDE`; mit einem zweiten Argument wartet es danach so viele Sekunden.
  - `groesse`: schreibt `GROESSE <zeilen> <spalten>` (aus der PTY gelesen).
  - `umgebung <NAME…>`: schreibt je Name `NAME=<Wert>` oder `NAME=-`, dann `CWD=<Pfad>` und `ANZAHL=<Zahl der Variablen>` ohne `CPATH`, `LIBRARY_PATH`, `MANPATH`, `SDKROOT` und `__CF_USER_TEXT_ENCODING` (das Python des Xcode-Wrappers am Mac fügt sie ein; ein Rust-Kind kennt sie nicht).
  - Nachtrag 20.11.2026: `still <s>`: schreibt **nichts** (auch kein `BEREIT`) und liest nichts, wartet s Sekunden (der Anfangscursor vor jeder Ausgabe, DI11). `stufen <hex1> [<hex2> …]`: wie jedes Kind nach `BEREIT` und dem Startbyte schreibt es die Byte `hex1`; jedes weitere Stück schreibt es erst nach **einem weiteren Byte** vom Client (so trifft jedes Stück einzeln beim Träger ein, ohne Zeitverhalten); nach dem letzten endet es mit 0 (Cursorschritte, DECTCEM, Umbruch, DI11). `fuellen [@<hex>] <n1> [<n2> …]`: mit `@<hex>` schreibt es **nach dem Startbyte und vor dem ersten Bild** diese Byte (ein Titel für die Prüfung der Protokollzeile; Nachtrag 21.11.2026: früher vor `BEREIT` nach einer Wartezeit von 300 ms, jetzt ohne Zeitannahme, denn der Betrachter hat `BEREIT` gesehen); je Bild (das erste nach dem Startbyte, jedes weitere nach einem Byte) schreibt es `ESC[H` und `n_k` mal einen Buchstaben (`a`, `b`, …, der Buchstabe wechselt je Bild), nach einem letzten Byte `ESC[2;1H` und `ENDE` (Bilder von bekannter Größe für die Obergrenze, DI5). `osc52viele <n> <b> [wort]`: schreibt `n` Folgen `ESC]52;c;<base64 von b Byte>BEL` (je 120 000 Zeichen bei b = 90 000; die Byte beginnen mit `wort`, sonst `k`), dann `ENDE` (der Betrieb mit der echten Obergrenze, DI5).
  - `titel <text>`: schreibt OSC 2 und `fertig`. `warte <s>`: ignoriert SIGHUP, schreibt `PID=<n>`, wartet s Sekunden (nur SIGKILL beendet es: der Dienst muss es nach SIGHUP und 1 s Wartezeit mit SIGKILL beenden).
- Die Fälle benutzen nur Ausgaben, die der Träger ohne Umbruch und ohne breite Zeichen darstellt (einfacher Text, `\r\n`, SGR, `ESC[H`, `ESC[2J`, die Modi 5, 2004 und 2026, OSC 0, 2 und 52). Die Referenz ersetzt den Träger durch ein kleines Modell dafür (Abschnitt 9, Stichprobe). Das Modell wurde im Nachtrag 20.11.2026 an `tr-ref` nachgemessen und an zwei Stellen berichtigt (beide fielen erst bei 2 und 3 Spalten auf): Ein Zeilenvorschub (`LF`) lässt den ausstehenden Umbruch stehen, und „Zeile ab Cursor löschen“ (`ESC[K`) löscht bei ausstehendem Umbruch nichts. Das Modell lehnt eine Größe mit weniger als 2 Spalten ab (`ValueError`, kein stilles Klemmen; Nachtrag 20.11.2026, Projektentscheid, 20.11.2026) und liest eine OSC-Zeichenfolge in einem Stück (sie darf 120 000 Zeichen haben).

### DI9 Prüfmodus

`pruefmodus` schaltet die Rahmen `Uhr`, `Stumm`, `Zustand` und `Kernfehler` und die eingespeiste Uhr (DI7) frei. Ohne ihn sind sie `art_nicht_erlaubt`. Er ist für Prüfläufe; im Alltag ist er aus.
- `Stumm` (`{"an": bool}`; sonst `stumm_ungueltig`): DI5. Der Dienst quittiert mit einem leeren Rahmen `0x11`: bei `an` **vor** dem Stummschalten (er erreicht den Betrachter noch), bei `aus` **hinter** der geschriebenen Schlange. Der Prüfclient wartet im Schritt `Stumm` auf die Quittung (5 s); so ist die Reihenfolge gegenüber den Rahmen anderer Betrachter festgelegt. `Kernfehler`: der Dienst verhält sich, als hätte der nächste Schritt des Kerns einen Fehler gegeben, und baut sofort neu (DI6).
- `Zustand` antwortet mit `{"kern": {"sitzungen": [{id, titel, zustand, code, groesse, letzte_eingabe, dialog, bracketed_paste, invers, cursor}], "betrachter": {Name: {person, raum, abos, ausstehend, verfallen, rueckstand}}},
  "dienst": {"betrachter": {Name: {"warteschlange": Byte, "stumm": bool}}, "kind": {"ende": bool, "code": null|n}, "kernbau": Zahl der Kerne bisher, "traeger_zeilen": [Zeilenform…], "warteschlange_max": Byte}}`.
  Das ist der Zustand des Kerns (`Sitzungskern::zustand`) ohne `stand` und der Zustand des Dienstes. `kernbau` ist 1 nach dem Start.
  Der Prüfclient **entfernt** aus der Antwort `dienst.traeger_zeilen`, `dienst.kind` und je Sitzung `zustand`, `code` und `cursor`, es sei denn, der Schritt `Zustand` trägt `voll`: Diese Werte hängen davon ab, ob Ausgabe und Kindende schon angekommen sind. Ein Fall mit `voll` wartet vorher darauf.

### DI11 Cursor (Nachtrag 20.11.2026, Projektentscheid, 19.11.2026 F2, Cursor-Nachtrag)

Quelle: `VT` R15 (der Kern führt den Cursor je Sitzung und reicht ihn durch), `TR` TR2 und TR3 (`Ausgabe` trägt `cursor` immer), Projektentscheid, 19.11.2026.
1. **Anfangscursor.** Der Dienst baut den Kern beim Start mit `anfang.sitzungen[0].cursor` gleich **`[0, 0, true]`**, immer, auch wenn das Kind noch nichts geschrieben hat. Der erste `Abzug` eines Betrachters trägt ihn; der Auszug (`RA`) zeigt `(0, 0, sichtbar)` statt `(0, 0, unsichtbar)`. Die Startdatei darf `cursor` nicht nennen (DI1).
2. **Meldung des Trägers.** Der Träger meldet `Ausgabe` **immer mit** `cursor` (`[zeile, spalte, sichtbar]`), auch bei einem reinen Cursorschritt (keine Zeile ändert sich). Eine `Ausgabe` entsteht, wenn eine Zeile **oder der Cursor** von der letzten Meldung abweicht; `[0, 0, true]` gilt als gemeldet. Das Träger-Modell der Referenz setzt das für die festen Kindprogramme um: Stelle (`zeile`, `spalte`; ausstehender Umbruch bleibt auf der letzten Spalte), `sichtbar` nach DECTCEM (`ESC[?25l` und `ESC[?25h`).
3. **Größenänderung.** Der Träger klemmt den zuletzt gemeldeten Cursor auf die neue Größe (`[min(zeile, zeilen − 1), min(spalte, spalten − 1), sichtbar]`, wie der Kern, `VT` R15 Punkt 8), bevor er vergleicht (`TR` TR3). **Bekannte Grenze des Träger-Modells (gemessen gegen `tr-ref`, Nachtrag 20.11.2026; Projektentscheid, 20.11.2026: das Festschreiben des Verhaltens von `alacritty_terminal` macht der Nachtrag zu `traeger` und `zustandskern`, im Vertrag `dienst` steht nur der Verweis, dass das Modell hier vereinfacht):** Schrumpfen die Spalten und steht der Cursor rechts vom neuen Rand, bricht `alacritty_terminal` ihn um (Spalte minus neue Breite, die Zeilen wandern mit); das Modell klemmt nur. Davon hängt **kein Fall** ab: Die Fälle der Größenänderung halten den Cursor links vom neuen Rand (`di11_cursor_groesse`). Ein Bau mit dem echten Träger zeigt in diesem Randfall den Cursor von `alacritty_terminal` (Vertrag `traeger` TR3).
4. **Neubau.** Siehe DI6 (1a): der Cursor des Trägers geht als `anfang.sitzungen[0].cursor` mit (Projektentscheid, 20.11.2026); jeder Betrachter sieht ihn im vollen Abzug, ob sichtbar oder nicht, auch bei wartender Synchronisierung (`di6_kernfehler_sync`: das Abbild vor dem Fehler zeigt noch den alten Stand, danach Zeilen und Cursor des Trägers).
5. **Was der Prüfclient vergleicht.** `abbild.cursor` (`[zeile, spalte, sichtbar]`) und `kern.sitzungen[].cursor` in der Antwort `Zustand`. Der Schritt `Warte` kennt `{"cursor": [z, s, sichtbar]}` und wartet, bis das Abbild den Cursor zeigt; damit sind reine Cursorschritte ohne Textmarke prüfbar.
6. **Abhängigkeit von der kleinsten Spaltenzahl (Projektentscheid, 20.11.2026).** Von der kleinsten Spaltenzahl 2 hängen ab: `di1_start_eine_spalte` (Exitcode 2), `di1_start_zwei_spalten` (2 Spalten gelten, das Abbild bricht nach jedem Zeichenpaar um), `di4_groesse_eine_spalte` (Ablehnung `groesse_ungueltig`). Alle Fälle des Cursors (`di11_*`) benutzen mindestens 20 Spalten.

### DI12 Ein Dienst, mehrere Sitzungen (Nachtrag 20.11.2026)

Herkunft: „Projektentscheid, 20.11.2026 (a)“ (Pflicht), Nachträge 20.11.2026 (Start-Struktur höchstens 4096 Byte, `sitzung` vergibt der Dienst), (Prüfstelle `start`) und (the maintainers Vertrag 1, `lauf.einstellungen`).
**Alle Regeln DI1 bis DI11 gelten unverändert, wenn die Startdatei kein `lauf` hat (Einzelbetrieb): 97 sichtbare und 75 verdeckte alte Fälle bleiben Byte für Byte gleich und grün.**
Die Brücke (`rust/spec/bruecke/`) ist der Gegenstand des Vertrags `bruecke`; sie nutzt `Starten`, `Gestartet`, `Ablehnung` und die Rahmen dieses Vertrags.

1. **Startdatei.** Neuer optionaler Schlüssel `lauf`: `{"programm": "/abs/pfad", "env": {…}, "groesse": [spalten, zeilen], "einstellungen": "/abs/pfad"}`. Genau diese vier Schlüssel (dazu optional `sitzungen`, Punkt 13, `ziellisten` und `laufzeit`, Punkt 15, `marker_datei` und `marker_ordner`, Punkt 14, und `nummer_max`, nur mit `pruefmodus`, siehe unten);
   `programm` und `einstellungen` absolute Pfade ohne NUL (höchstens 4096 Byte), `env` wie `kind.env`, `groesse` wie `groesse` (Spalten 2 bis 1000, Zeilen 1 bis 1000).
   **`lauf.einstellungen`** (Nachtrag) ist der Pfad der Einstellungsdatei (Vertrag 2). Der Dienst liest ihn **nur aus der Startdatei**, nie aus der Anfrage der Ansicht, nie über `argv`.
   - Mit `lauf` sind `kind` und `groesse` **optional, aber nur zusammen**: beide oder keines (sonst Exitcode 2). Ohne beide startet der Dienst mit **null** Sitzungen; `anfang.sitzungen` fehlt dann oder ist leer (sonst Exitcode 2).
     Mit beiden startet er eine erste Sitzung aus `kind` wie bisher (Kennung aus `anfang.sitzungen[0].id`), die mit `lauf` **nicht** die Form `s-<n>` haben darf (sonst Exitcode 2; die Kennungen von `Starten` gehören dem Dienst).
   - `lauf.nummer_max` (ganze Zahl 1 bis 999 999 999, **nur mit `pruefmodus`**, sonst Exitcode 2) senkt die Zahl der Kennungen, die `Starten` insgesamt vergibt (Vorgabe 999 999 999; bis zum Nachtrag P25 1 bis 255, Vorgabe 255); nur zum Prüfen, wie `warteschlange_max`.
   - `lauf.rahmen_max` (ganze Zahl 1 bis 255, **nur mit `pruefmodus`**, sonst Exitcode 2; Nachtrag P25, Abschnitt 19) senkt die Zahl der Rasternummern der Sitzungen von `Starten` (Vorgabe 255); nur zum Prüfen.
   - Ohne `lauf` ist `Starten` (und `Beenden`) keine erlaubte Art: Fehler `art_nicht_erlaubt` (DI3), die Verbindung endet.
2. **Je Sitzung ein eigener Satz** aus Träger, `Sitzungskern` (mit genau dieser einen Sitzung, `VT` unverändert), Kind und PTY. Der Auszug je Betrachter ist **ein** `Verbindung` je Betrachter (ein Deflate-Kontext, `RA`), der den Zustand je Sitzungsnummer
   hält; Betrachter mit Abo auf zwei Sitzungen haben zwei Bilder in derselben Verbindung. Dazu gibt es einen **Raumkern** (ein Kern ohne Sitzung), der die Nachrichten `Einstellungen`, `Routen`, `Anfragen`, `Agenten` liefert.
   - `Verbinden`, `Trennen` und `Takt` gehen an **alle** Kerne (Raumkern zuerst, dann die Sitzungen in der Reihenfolge ihres Starts); ein Betrachter, der sich verbindet, ist in jedem Kern verbunden, und `Socket` (DI5) geht mit derselben Zahl `ausstehend` an alle Kerne.
   - Von den Nachrichten der Kerne gehen `Einstellungen`, `Routen`, `Anfragen`, `Agenten` und `Leeren` **nur vom Raumkern** an die Betrachter; die `Sitzungsliste` der Kerne wird nie weitergegeben, der Dienst schickt seine eigene (Punkt 5). Alle anderen
     Nachrichten (`Abzug`, `Rasteraenderung`, `Nachricht`, `Zwischenablage`, `Hinweis`, `Dialog`, …) kommen aus dem Kern der Sitzung, auf die sie sich beziehen.
   - **Sitzungsnummer** (`RA`, ein Byte, „Rasternummer“): die Sitzung aus `kind` hat die Nummer **0**, eine Sitzung von `Starten` eine Nummer von 1 bis 255. Die Rasterrahmen tragen sie. **Nachtrag P25 (Abschnitt 19):** Die Rasternummer ist von der Kennung getrennt. Die Sitzung `s-<n>` hat die Nummer n nur, solange n höchstens 255 ist und n in diesem Lauf noch nie vergeben war; welche Nummer eine Sitzung trägt, sagt allein das Feld `nr` der `Sitzungsliste` (Punkt 5). Wer die Nummer aus der Kennung ableitet, liest falsche Rahmen.
3. **Route `Starten`** (Ereignis der Ansicht `{"art": "Starten", "start": {…}}`; keine anderen Schlüssel, sonst `Ablehnung start_form`). Der Dienst entscheidet selbst; der Kern sieht das Ereignis nie. Prüffolge (die erste Verletzung antwortet, nichts startet):
   1. **Prüfung der Start-Struktur, Stufen 1 bis 13** an **einer** Stelle (`start_form`, Punkt 4; Nachtrag): Die Struktur kommt als JSON-Wert; der Dienst gibt dem Crate `start` ihre **kanonische Form** (kompakt, Schlüssel sortiert, UTF-8, nicht-ASCII ohne `\u`-Ersatz) und ruft `start::lesen_anfrage`. Die Stufen, ihre Codes und festen Texte stehen in the maintainers Vertrag 1
      (`rust/spec/start/README.md`); die niedrigste verletzte Stufe antwortet. Verletzung: `Ablehnung` mit `grund` **`start_pruefung`**, `code` `start.<code>` und dem festen Text der Stufe (unten und Punkt 4).
   2. **Einstellungsdatei lesbar:** `lauf.einstellungen` ist eine reguläre Datei von höchstens **65 536 Byte**, die das Crate `einstellungen` nach Vertrag 2 liest (vorher: ein JSON-Objekt bis 1 048 576 Byte). Sonst `Ablehnung einst_unlesbar` (der Dienst gibt den `einst.*`-Code nicht weiter). **Die Referenz** liest die Datei nur so weit, wie sie die Felder braucht (`fassung` 1 oder 2, `routen` mit den Texten `schluessel`, `art`, `klasse` und in Fassung 2 dem Text `modell`, `kunden` mit dem Text `schluessel`); die Fälle benutzen nur Dateien, die das Crate liest, oder offensichtlich kaputte (kein Objekt, `routen` keine Liste).
      **Prüfung gegen die Einstellungen, Stufen 14 bis 17** (`start::pruefen`): gleich danach (nach der `Startwahl`, Punkt 10; vor Zahl, Nummer, Ordner und Kind). Verletzung wie bei 1. Die Form (Stufen 1 bis 13) wird **vor** der Einstellungsdatei geprüft: bei falscher Form und unlesbarer Datei antwortet die Stufe.
   3. **Zahl:** höchstens **8 Sitzungen laufen gleichzeitig** (`zustand` `laeuft`); beendete zählen nicht. Sonst `Ablehnung zu_viele_sitzungen`.
   4. **Nummer:** `n = (bisher vergebene Nummern) + 1`; `n` größer als `nummer_max` (999 999 999; bis zum Nachtrag P25: 255): `Ablehnung nummern_erschoepft`. Kennungen sind `s-1`, `s-2`, … **deterministisch und nie wiederverwendet** (auch nicht nach dem Ende); eine abgelehnte oder gescheiterte `Starten` verbraucht keine Nummer. **Mit `lauf.sitzungen` überspringt der Dienst Nummern, deren Eintrag besteht (Punkt 13).**
      **Rasternummer (Nachtrag P25, Abschnitt 19):** Danach wählt der Dienst die Rasternummer der neuen Sitzung: zuerst eine in diesem Lauf nie vergebene (n selbst, wenn n höchstens `rahmen_max` ist, sonst die kleinste), dann von den freien die der Sitzung, die **am längsten beendet** ist. Frei ist eine Nummer, wenn ihre Sitzung beendet ist und kein Betrachter sie abonniert. Ist keine Nummer zu haben: `Ablehnung nummern_erschoepft`, und keine Kennung wird verbraucht.
   5. **Ordner und Datei:** im Ordner des Dienstes (`socket.ordner`, 0700) ein Unterordner **`<kennung>`** mit Modus **genau 0700** (der Dienst legt ihn an und räumt ihn auf; besteht der Name schon, ersetzt oder löscht der Dienst nichts: `Ablehnung start_ordner`),
      darin die Datei **`start.json`** mit Modus **genau 0600** (exklusiv angelegt). **Inhalt:** die Start-Struktur der Anfrage **mit eingesetztem `sitzung`** (= die Kennung), alle anderen Felder **unverändert**. Die Referenz schreibt die kanonische Form;
      der Bau schreibt, was `start::schreiben` ausgibt, das `lesen_datei` wieder annimmt; verglichen wird der **gelesene Inhalt** (das feste Kind gibt ihn kanonisch aus), nicht die Bytes.
   6. **Kind:** in einer neuen PTY der Größe `lauf.groesse`, eigene Sitzung, die PTY als Steuerterminal, mit **`argv = [lauf.programm, "--meldung", <pfad von meldung.sock>, <pfad der start.json>]`** (genau vier Einträge, beide Pfade absolut; Abschnitt 18; Nachtrag 22.11.2026, vorher `[lauf.programm, <pfad der start.json>]`, genau zwei Einträge), der Umgebung **genau `lauf.env`** (nichts sonst), dem Arbeitsverzeichnis **`/`**. Lässt es sich nicht starten
      (Programm fehlt, nicht ausführbar): `Ablehnung programm_nicht_startbar`, der Unterordner wird entfernt, keine Nummer verbraucht.
   7. **Antwort:** Alle Betrachter bekommen die neue `Sitzungsliste` (Punkt 5); **danach** bekommt nur der Absender die Nachricht `Gestartet {"sitzung": "<kennung>"}` (Rahmen Art 6). Für jeden schon verbundenen Betrachter hat der Dienst vorher `Verbinden` an den neuen Kern gegeben.
   Die Gründe sind **feste Wörter**: `start_pruefung` (Stufen 1 bis 17 der Prüfstelle `start`, Punkt 4), `start_form` (nur die Form des **Ereignisses** `Starten`: kein `start`, ein fremder Schlüssel, ohne `lauf`), `einst_unlesbar`, `zu_viele_sitzungen`, `nummern_erschoepft`, `start_ordner`, `programm_nicht_startbar`, `meldung_kanal` (nur der Bau, Abschnitt 18). `Ablehnung` ist der Rahmen 0x07, nur an den Sender (DI3). **Jede Ablehnung von `Starten` trägt (Nachtrag 20.11.2026) `grund`, `code` und `text`:**
   `{"grund": "<wort>", "code": "<wort>", "text": "<fester Text>"}`. In der Referenz sind `code` und `grund` für die Gründe des Dienstes dasselbe Wort; bei `start_pruefung` ist `code` der Code der Stufe (`start.<code>`) und `text` der feste Text der Stufe aus the maintainers Vertrag 1, **unverändert** (Nachtrag; vorher stand der Code auch in `grund`). Die Texte der Referenz: `start_form` „Start hat falsche Form“, `einst_unlesbar` „Einstellungsdatei nicht lesbar“,
   `zu_viele_sitzungen` „Zu viele Sitzungen“, `nummern_erschoepft` „Keine Sitzungsnummer frei“, `start_ordner` „Ordner der Sitzung nicht anlegbar“, `programm_nicht_startbar` „Programm nicht startbar“, `meldung_kanal` „Meldekanal der Sitzung nicht anlegbar“ (nur der Bau). Die Brücke gibt `code` und `text` unverändert an die Seite (Vertrag `bruecke`). Die Ablehnungen des Kerns und die übrigen des Dienstes (`Beenden`, Routing) tragen nur `grund`.
4. **Prüfstelle `start` (Nachträge; gebaut im Nachtrag).** Der Dienst ruft **vor dem Start des Kindes** an der einen Stelle (`start_form`, Punkt 3.1) `start::lesen_anfrage` und nach dem Lesen der Einstellungen (Punkt 3.2) `start::pruefen(start, einstellungen)`; die Einstellungen liest er mit `einstellungen::lesen` aus `lauf.einstellungen`.
   Bei einem Fehler schickt er `Ablehnung {"grund": "start_pruefung", "code": "start.<code>", "text": "<fester Text>"}`: Code und Text sind die der Prüfstelle, **unverändert**, der Dienst führt keine eigene Liste; dann startet kein Kind. Die Datei `start.json` schreibt er mit `start::schreiben` (Startdatei = Anfrage mit eingesetztem `sitzung`, in der Form von Vertrag 1); das Startprogramm prüft danach noch einmal (zweite Linie).
   Der Pfad der Einstellungsdatei und der Pfad der Startdatei kommen **nie** aus der Anfrage der Ansicht (`start.schluessel_unbekannt` ist die Antwort der Prüfstelle).
   **Stufen, die der Dienst prüft:** alle Stufen von Vertrag 1, die eine Anfrage erreichen kann: 1 `zu_gross`, 2 `json_ungueltig` (hier nur: die Struktur ist kein Objekt; Byte-Fehler wie BOM, doppelter Schlüssel oder Syntaxfehler gibt es nicht, weil der Rahmen schon geparst ist), 3 `sitzung_nicht_erlaubt`, 4 `marker_nicht_erlaubt`, 5 `schluessel_unbekannt`, 6 `schluessel_fehlt`, 7 `typ_falsch`, 8 `fassung_unbekannt`, 9 `kennung_form`, 10 `modell_form`, 11 `ordner_ungueltig`, 13 `harness_unbekannt`, 14 `route_unbekannt`, 15 `modell_route`, 16 `kunde_unbekannt`, 17 `harness_route_unvertraeglich`. Stufe 12 `marker_form` erreicht eine Anfrage nie (`marker` endet an Stufe 4).
   **Stufen, die „Lauf starten“ prüft** (Schritte des Aufrufs in Vertrag 1, nicht der Dienst, weil sie Dateisystem, Marker, Einschluss oder Prozesse brauchen): Schritt 0 (TLS-Umgebung), 1 (Argumente), 2 (Startdatei: Eigentümer, Modus 0600), 3 (Einstellungsdatei nach Vertrag 2 mit allen `einst.*`-Codes), 4 (zweite Prüfung der Datei), 4a (lokale Route ohne `ziel` oder `modell`), 5 (Arbeitsordner existiert), 5a (Zählordner der Marker), 5b (Ordnermarke), 6 (Einschluss), 7, 7a0 und 7a (Suche, Zielliste, Modellweg), 8 (exec des Harness). Dem Dienst fehlt für sie die Grundlage: Er prüft nur die Start-Struktur und die Einstellungsdatei, nicht den Rechner.
   **Orakel (`orakel_start.py`):** Die Referenz (`start_lesen_roh`, `stufen_pruefen`) läuft gegen alle 64 Fälle der Form `anfrage` in `rust/spec/start/faelle` mit 100 % Übereinstimmung von Gültigkeit und Code (64 von 64); die Tabelle der 17 Codes und festen Texte der Referenz gleicht der des Vertrags 1. Die Fälle der Formen `datei` (24) und `aufruf` (112) gehören nicht dazu: der Dienst liest nur Anfragen, und der Aufruf ist ein Programm für sich.
   **Gegen das Crate:** Jeder Fall des Dienstes mit einer Ablehnung `start_pruefung` läuft in Rust gegen das echte Crate; dessen Antwort muss die Erwartung des Falls treffen. Der Dienst gibt dem Crate die **kanonische Form** der Struktur; Zahlen außerhalb von `i64` und `u64` sind für den Rust-Leser Gleitkommazahlen (die Referenz bildet das nach, `zahlen_klein`).
5. **`Sitzungsliste`** (Nachricht an **alle** Betrachter): `{"art": "Sitzungsliste", "sitzungen": [ {"id", "harness", "modell", "route", "kunde", "zustand", "nr", ("code"), ("grund")} … ]}` in der Reihenfolge des Starts, die Sitzung aus `kind` zuerst. **`nr`** (Nachtrag P25, Abschnitt 19) ist die Rasternummer der Sitzung (0 bis 255, die Sitzung aus `kind` 0) oder `null`, sobald eine spätere Sitzung ihre Nummer bekommen hat. `harness`, `modell`, `route`, `kunde` sind die Texte der Start-Struktur;
   ist ein Feld kein Text (`kunde: null`, fehlt, Zahl), steht `""`. Für die Sitzung aus `kind` stehen alle vier als `""`. `zustand` ist `laeuft` oder `beendet`; bei `beendet` steht `code` (der `code` aus `Kindende`, DI8, mit Abschnitt 18 der Code der Zeile von `lauf`, wenn sie gültig war), sonst fehlt der Schlüssel. `grund` (Text) steht nur bei `beendet` und nur, wenn `lauf` eine gültige Zeile gemeldet hat (dann der `grund` der Zeile) oder die Meldung ungültig war (`lauf.meldung_ungueltig`); sonst fehlt der Schlüssel (Abschnitt 18; Nachtrag 22.11.2026). Die Liste kommt
   **beim Verbinden** (an der Stelle, an der der Kern seine Liste schickt: zwischen `Routen` und `Anfragen`) und **nach jeder Änderung** (Start einer Sitzung, `Kindende`). Der Raumkern gibt nie eine eigene Liste weiter. Beendete Sitzungen bleiben in der Liste.
6. **`Beenden`** (Ereignis der Ansicht `{"art": "Beenden", "sitzung": "<id>"}`; andere Schlüssel oder eine nicht-Text `sitzung`: `Ablehnung beenden_form`): Unbekannte Sitzung: `Ablehnung sitzung_unbekannt`; schon beendete: `Ablehnung sitzung_beendet` (derselbe Grund wie der Kern für eine Eingabe).
   Sonst beendet der Dienst das Kind **wie beim Dienstende**: SIGHUP, bis zu 1 s warten, dann SIGKILL, `waitpid`. Der Dienst blockiert dabei nicht (die Frist läuft in der Schleife). Die Sitzung **bleibt** mit `beendet` in der Liste; `code` ist der Code des Kindes
   (129 bei SIGHUP, 137 bei SIGKILL, DI8). Ein Kindende von selbst setzt die Sitzung ebenso auf `beendet`. Die anderen Sitzungen bleiben unberührt. `Beenden` hat keine eigene Antwort (die Liste ist die Antwort).
7. **Abo, Eingabe und alle anderen Ereignisse einer Sitzung** tragen `sitzung` und gehen an den Kern **dieser** Sitzung (`Abo`, `Groesse`, `Text`, `Taste`, `Einfuegen`, `Antwort`, `Freigabe`, `RouteWaehlen`); ein Betrachter kann mehrere Sitzungen abonnieren.
   Fehlt `sitzung`, ist sie kein Text oder unbekannt: `Ablehnung sitzung_unbekannt` (vom Dienst, der Kern sieht nichts). Die Wirkungen `pty` und `groessen` eines Kerns gelten nur für die PTY und den Träger **seiner** Sitzung; eine Eingabe an eine Sitzung erreicht nie das Kind einer anderen.
   **Im Lauf lehnt der Dienst `RaumWechsel` und `EinstellungSetzen` selbst ab** (`Ablehnung nicht_im_lauf`; ein Betrachter bleibt in seinem Raum, die Einstellungen sind die der Startdatei), und der Rahmen `Kernfehler` des Prüfmodus ist `art_nicht_erlaubt`. Der Neubau eines Kerns je Sitzung nach DI6 ist
   Pflicht des Baus (Abschnitt 11), ohne Außenfall. `Socket`, `Takt`, `Uhr`, `Stumm` und der Rückstau (DI5, DI7, DI9) gelten für den Dienst als Ganzes; die Obergrenze der Warteschlange gilt je Betrachter über alle Sitzungen.
   **`Zustand` im Lauf** (DI9): `kern.sitzungen` führt die Sitzungen in der Reihenfolge des Starts; `kern.betrachter.<n>` hat die `abos` aller Kerne (in der Reihenfolge der Sitzungen), `verfallen` als Summe, `ausstehend` vom Raumkern; `dienst.kind` und `dienst.traeger_zeilen` sind je Sitzung ein Objekt (der Prüfclient entfernt sie ohne `voll`);
   neu: `dienst.nummern` (Kennung → Nummer), `dienst.gestartet` (Zahl der vergebenen Nummern); `dienst.kernbau` ist die Zahl aller Kerne (Raumkern und Sitzungen).
8. **Dienstende** (DI2 Punkt 7): beendet **alle** Kinder (SIGHUP an alle, bis zu 1 s gemeinsam, dann SIGKILL, `waitpid`) und entfernt **alle Unterordner**, die der Dienst für `Starten` angelegt hat (mit `start.json`). Ein Ordner, der schon vor dem Start da war, bleibt.
9. **Feste Kindprogramme für `lauf`** (Prüfmittel; Teil des Programms wie `--kind`): `wb-dienst --kind lauf <argv des Kindes …>`. Weil `lauf.programm` ein einzelner Pfad mit `argv = [programm, start.json]` ist, schreibt der Prüfclient für `@laufkind` einen kleinen
   Wrapper (`#!/usr/bin/python3 -I`, führt `<DI_BIN> --kind lauf <seine argv>` aus; die Umgebung bleibt, wie der Dienst sie gab; Nachtrag 22.11.2026: Mit Abschnitt 18 steht vor der Startdatei das Paar `--meldung <pfad>`, das das feste Kindprogramm weglässt, bevor es die Startdatei liest und `ARGV=<Zahl>` zählt, so dass die Fälle unverändert bleiben). Das Kind tut nach `BEREIT` und dem Startbyte, was der Plan der **Umgebung** (`KIND_PLAN`, ein JSON-Objekt Kennung → Eintrag, `"*"` für alle) für die Kennung
   sagt, die in der `start.json` steht: `bericht` (schreibt `START=<kanonischer Inhalt>`, `MODI=<Modus der Datei>/<Modus des Ordners>`, `NAME=<ordnername>/<dateiname>`, `ARGV=<Zahl>/<Name von argv[0]>`, `ENV=<kanonische Umgebung ohne die Variablen des Dienstes von DI8 und ohne KIND_PLAN>`, `CWD=<Pfad>`, `ENDE`),
   `marke` (`MARKE <text>`, wartet `sek`), `eingabe` (`MARKE <text>`, liest `n` Byte, schreibt `EINGABE <hex>`), `ende` (endet mit `code`), `warte` (ignoriert SIGHUP, schreibt `PID=<n>`, wartet `sek`), `groesse` (schreibt `GROESSE <zeilen> <spalten>`). Der Prüfclient meldet `abbilder`
   (je Sitzungsnummer), `{"liste": …}` und `{"text": …, "nr": n}` als Bedingungen für `Warte`, `ordner_pruefen` und `kinder_pruefen` am Ende (Unterordner weg, alle gemeldeten PIDs beendet) und ersetzt `@einstellungen` durch den Pfad der Kopie der Vorführdatei.
10. **Nachricht `Startwahl`** (Nachtrag 20.11.2026; nur im Lauf): Der Dienst schickt jedem Betrachter **beim Verbinden als letzte Nachricht** des Verbindens (nach `Einstellungen`, `Routen`, `Sitzungsliste`, `Anfragen`, `Agenten`) und **nach jeder Änderung** (Rahmen Art 6) die Daten, die die Seite für das Startformular braucht:
    `{"art": "Startwahl", "fassung": 1, "harness": [{"name", "art", "routenart"}…], "routen": [{"schluessel", "art", "klasse", "modell"}…], "kunden": ["<schluessel>"…, null]}`.
    - `harness`: die Harness-Tabelle von the maintainers Vertrag 1 (Fassung 1 mit Nachtrag 20.11.2026), in dieser Reihenfolge: `{"name": "claude", "art": "cloud", "routenart": "cloud"}`, `{"name": "copilot", "art": "cloud", "routenart": "cloud"}`, `{"name": "pi", "art": "lokal", "routenart": "lokal"}` (Nachtrag: `copilot` neu; `art`: Cloud- oder lokaler Harness; `routenart`: die erlaubte Routenart). Das Crate `start` gibt die Tabelle nicht heraus; der Dienst hält sie als Konstante, und ein Test prüft jede Zeile gegen das Crate.
    - `routen`: die Routen der Einstellungsdatei **in der Reihenfolge der Datei**, je **nur** `schluessel`, `art`, `klasse` und **`modell`** (Nachtrag, Projektentscheid, 20.11.2026: der Text der Route oder `null`; bei Einstellungen der Fassung 1 und bei einer Cloud-Route ohne `modell` ist er `null`). **Kein Ziel, kein Netzziel, keine Adresse, kein Schlüssel, kein Pfad**: alles andere im Eintrag bleibt weg. `kunden`: die Schlüssel der Kunden in der Reihenfolge der Datei, **zuletzt `null`** („ohne Kunde“).
    - **Quelle ist nur `lauf.einstellungen`** aus der Startdatei. Der Dienst liest die Datei beim **Verbinden** eines Betrachters und bei **jedem `Starten`**. Eine Datei, die er nicht lesen kann (keine reguläre Datei, mehr als 65 536 Byte, vom Crate `einstellungen` abgelehnt), ergibt **keine** `Startwahl`
      (und bei `Starten` `einst_unlesbar`). **Wann sie kommt:** Beim Verbinden bekommt der **neue** Betrachter sie; unterscheidet sich der Inhalt vom zuletzt gesendeten (oder wurde noch keiner gesendet), bekommen sie **alle** Betrachter. Bei jedem `Starten` (nach der Form, vor der Liste) bekommen alle Betrachter eine neue, **wenn sich der Inhalt geändert hat**; bei gleichem Inhalt kommt keine.
    - Die Brücke reicht die Nachricht unverändert durch (Rahmen im Strom, `bruecke` Abschnitt 5). Die Prüfung der Einstellungsdatei nach Vertrag 2 (`einst.*`) gehört dem Crate `einstellungen`; die Referenz liest nur die genannten Felder. Ändert sich nur das `modell` einer Route, ist der Inhalt ein anderer: alle Betrachter bekommen beim nächsten `Starten` eine neue `Startwahl`.
11. **Offene Fragen:** (1) die Prüfstelle (Punkt 4); (2) Nummer 0 für die Sitzung aus `kind` und `n` für `s-<n>`; höchstens 255 Nummern je Lauf des Dienstes; (3) ein eigener Raumkern und die Regel „nur der Raumkern liefert `Einstellungen`, `Routen`, `Anfragen`, `Agenten`“;
    (4) `RaumWechsel` und `EinstellungSetzen` sind im Lauf abgelehnt; (5) die Liste kommt **vor** `Gestartet`; (6) beendete Sitzungen bleiben in der Liste und zählen nicht zu den 8; (7) `lauf.einstellungen` wird bei **jedem** `Starten` gelesen.
12. **Sitzungsende räumt auf** (Nachtrag 21.11.2026; Herkunft: Projektentscheid, 21.11.2026, Vertrag 1 `rust/spec/start/README.md` Zeilen 696 bis 704). Der Dienst räumt die Dateien einer Sitzung an **einer** Stelle auf (eine Funktion „Sitzungsende räumt auf“, Kommentar `SECURITY`), die von drei Wegen gerufen wird:
    dem **Kindende** (normales Ende, gescheiterter Start von „Lauf starten“ mit 64 bis 78, Signal, `Beenden` über das Kindende) **vor** der neuen `Sitzungsliste`, so dass ein Betrachter, der `beendet` sieht, den Ordner nicht mehr findet, und dem **Dienstende** (nach dem Ende der Kinder). Die Sitzung bleibt mit `code` in der Liste. Ein zweiter Aufruf findet nichts. **Zuerst die Signale, dann das Löschen.**
    - **Signal** (nur Linux): SIGTERM an die **Prozessgruppe** der PID aus `suche.pid` und aus `modellweg.pid` (je Datei, in dieser Reihenfolge). Die Datei muss eine **reguläre Datei** im eigenen Sitzungsordner sein (kein Link, per `lstat` und mit gleichem Gerät und gleicher Inode nach dem Öffnen geprüft), höchstens 11 Byte, ihr Inhalt **genau eine Dezimalzahl von 1 bis 10 Ziffern und ein `\n`** (kein Vorzeichen, kein Leerzeichen, kein zweiter Zeilenumbruch), Wert 2 bis 2 147 483 647.
      Der Prozess muss **leben** (kein Zombie), **Sitzungsleiter** sein (Sitzungskennung und Gruppe gleich seiner PID), nicht das Kind selbst und nicht der Dienst, kein eigenes Kind des Dienstes (das Kind einer anderen Sitzung, das sein Träger noch nicht eingesammelt hat), sein **Elternprozess muss der Dienst sein** (Feld 4 von `/proc/<pid>/stat`), und er muss **nach dem Kind der Sitzung gestartet** sein. (Nachtrag 23.11.2026, P23: **Zugehörigkeit über den Elternprozess.** Die Helfer sind Kinder des Kindes der Sitzung. Am Sitzungsende ist dieses Kind beendet, und der Kern hat seine lebenden Kinder dem Dienst als Subreaper übergeben (Abschnitt 17). Ein Prozess, der die PID eines toten Helfers geerbt hat, etwa eine neue Shell der Person oder ein später gestarteter Sitzungsleiter derselben Kennung, hat einen anderen Elternprozess und bekommt kein Signal. Ein adoptiertes Kind kann nur der Dienst einsammeln, und er sammelt nur in seiner eigenen Schleife ein, nie zwischen Prüfung und Signal; der pidfd hält die Identität vom ersten Lesen der PID-Datei bis zum Signal. Der Dienst liest die PID-Datei erst am Sitzungsende: Ein früheres Lesen bräuchte als Beleg das lebende Kind als Elternprozess, und ein Kind, das wie im Fall `di12_sende_signal` Millisekunden nach dem Schreiben endet, käme jedem Takt zuvor; der Beleg am Ende wäre also ohnehin nötig. Ohne diesen Beleg geht kein Signal. Grenze: Eine Waise einer anderen, schon beendeten Sitzung desselben Dienstes hat ebenfalls den Dienst als Elternprozess; sie gehört zum Dienst, nicht zu einem fremden Programm. Schritt II: Zugehörigkeit über eine Gruppe je Sitzung (etwa eine cgroup je Sitzung), damit auch diese Grenze fällt; offen (Projektentscheid, 23.11.2026).) Die Startzeit (Feld 22 von `/proc/<pid>/stat`) zählt in Takten von 10 ms; der Dienst hält Startzeit und PID des Kindes beim Start der Sitzung fest. Der Prozess gilt als später gestartet, wenn seine Startzeit **größer** ist, **oder** wenn sie **gleich** ist und seine PID **größer** als die PID des Kindes (Nachtrag 22.11.2026: Ein Helfer, den das Kind im selben Takt wie den eigenen Start erzeugt, bekam sonst kein Signal. Grund der PID-Regel: Der Kern vergibt PIDs aufsteigend und fängt erst nach dem ganzen PID-Raum von vorn an, was in 10 ms keine Maschine schafft; also ist im selben Takt die größere PID der spätere Start). Eine kleinere oder gleiche Startzeit mit kleinerer PID zählt nicht. **Unter Linux gilt diese Reihenfolge gegen die Wiederverwendung einer PID:** zuerst `pidfd_open` der PID aus der Datei, dann die Prüfung der Felder von `/proc/<pid>/stat`, dann die Frage, ob der Prozess hinter dem pidfd inzwischen beendet ist (ein pidfd ist lesbar, sobald sein Prozess endete; dann kein Signal), erst danach das Signal. Das Signal geht an die **Prozessgruppe** mit der Nummer der geprüften PID. Die Gruppe gehört zur geprüften Identität: Der Prozess ist Gruppenleiter (Gruppe gleich seiner PID), und der Kern gibt eine Gruppennummer nicht an einen anderen Prozess, solange irgendein Prozess der Gruppe lebt; die Nummer kommt erst wieder, wenn der ganze PID-Raum durchlaufen ist. Zwischen Prüfung und Signal benennt die Nummer also keine andere Gruppe. **Im Zweifel kein Signal** (auch wenn Startzeit oder PID des Kindes unbekannt ist oder `pidfd_open` scheitert); die Dateien werden trotzdem gelöscht. Unter macOS gibt es weder `/proc` noch einen pidfd: Dort geht nie ein Signal, das ist die Grenze (siehe den Punkt „macOS“ unten). (Nachtrag 23.11.2026: macOS hat keine gleichwertige Sicherung. `proc_pidinfo` nennt Elternprozess und Startzeit, hält aber keine Identität, und `kqueue` mit `EVFILT_PROC` meldet nur das Ende einer PID; zwischen Prüfung und `killpg` bliebe die Nummer ungebunden. Schritt II braucht dort eine eigene Entscheidung.)
    - **Löschen** folgt keiner Verknüpfung und wirkt nur unter dem eigenen Ordner `s-<n>`: der Ordner wird mit `lstat` geprüft (ein Name, der nicht `s-<Ziffern>` ist, wird nicht angefasst), ein Link oder eine Datei an der Stelle des Ordners wird selbst entfernt, in einem Ordner wird alles entfernt, ein Link darin als Link, **nie sein Ziel**.
    - **macOS:** nur die Dateien werden gelöscht; das Signal folgt in Schritt II (offen, siehe unten). Die Fälle, die das Signal prüfen, tragen das im Prüfclient: ohne `/proc` (nicht Linux) lässt der Prüfclient die Marker `*.term` aus dem Ergebnis des Schritts `Ordner` und aus `ordner_inhalt` heraus, und der Fall prüft nur die Dateien. Die Erwartung ist **eine** (Linux), die Plattform steht im Prüfclient, nicht je Maschine im Fall.
    - **Abgrenzung:** „Lauf starten“ entfernt den CA-Schlüssel bei eigenem Abbruch vor dem `exec` selbst; der Dienst räumt unabhängig davon, keine Seite verlässt sich auf die andere.
    - **Prüfmittel:** der Schritt `Ordner` des Prüfclients (Inhalt des Socket-Ordners jetzt; `warte_auf`, `pause`, `lesen`; Ziffern im gelesenen Inhalt werden zu `#`), das Fallfeld `vorzeuge` (ein Sitzungsleiter, den der Prüfclient **vor** dem Dienst startet, PID in `<base>/vorzeuge.pid`, Marker `vorzeuge.term`), das Fallfeld `ende_warte` (Sekunden vor `ordner_inhalt` am Ende), das feste Kindprogramm `zeuge` (ignoriert SIGHUP, schreibt bei SIGTERM `TERM` in `<ordner>/zeuge.term`, endet nach 6 s selbst) und die Pläne `scheitern` (schreibt `suche.pid`, `modellweg.pid`, `ca.key`, `bundle.pem` und endet mit 78) und `raeumen` (`pid`: `leiter` | `gruppenleiter` | `vor` | `keine`; `form`: `ok` | `ohne_zeilenende` | `leerzeichen` | `zwei_zeilen`; `datei`; `ziel`: `link`; `inhalt`; `links`; `sek`; `code`; schreibt am Ende `FERTIG`) des festen Kindes `lauf`.
13. **Keine Kennung, deren Ordner noch besteht** (Nachtrag 21.11.2026; Herkunft: Projektentscheid, 21.11.2026 nach dem an earlier test run: „Lauf starten“ legt je Sitzung `<sitzungen>/<sitzung>` an und lehnt mit `lauf.sitzung_belegt` ab, wenn er besteht; nach einem Neustart zählte der Dienst wieder ab `s-1`).
    - **Startdatei:** `lauf` erlaubt zusätzlich den optionalen Schlüssel `sitzungen` (in beiden Betriebsarten): Text von 1 bis 4096 Byte, ohne NUL, **absoluter** Pfad, geprüft wie `lauf.einstellungen`. Jeder andere Wert: Exitcode 2 mit der Zeile `lauf.sitzungen muss ein absoluter Pfad sein`. Ohne den Schlüssel bleibt alles wie vorher. Der Dienst liest den Ort **nur** aus seiner Startdatei, nie aus `lauf.json`.
    - **Vergabe bei `Starten`:** Der Dienst nimmt `n = (bisher vergebene Nummern) + 1` wie bisher. Ist `lauf.sitzungen` gesetzt, prüft er `<lauf.sitzungen>/s-<n>` mit `lstat` (folgt keiner Verknüpfung): fehlt der Eintrag (`NotFound`, auch wenn `lauf.sitzungen` selbst fehlt), ist die Kennung frei;
      besteht ein Eintrag **gleich welcher Art** (Ordner, Datei, Verknüpfung, auch eine tote), ist sie belegt und der Dienst prüft `n + 1`; **jeder andere Fehler** (etwa keine Rechte) ist `Ablehnung start_ordner`, nichts wird angelegt und keine Nummer verbraucht.
      `nummer_max` (und die Grenze 255) gilt für **jedes geprüfte** `n`: wird sie überschritten, `Ablehnung nummern_erschoepft`. Die Reihenfolge der übrigen Prüfungen bleibt (Punkt 3: Form, Einstellungen, Stufen, Zahl 8, dann die Nummer). **`naechste` (die Zahl der vergebenen Nummern, `dienst.gestartet`) wird wie bisher erst nach einem gelungenen Start gesetzt**, auf das vergebene `n`; Überspringen setzt nichts.
    - Der eigene Ordner `<socket.ordner>/s-<n>` bleibt wie bisher (Punkt 5): besteht er, `Ablehnung start_ordner` (Fall `di12_starten_ordner_vorhanden`); er wird für die Wahl der Nummer **nicht** gelesen. Der Dienst legt unter `lauf.sitzungen` **nichts an und löscht dort nichts**, schickt dort kein Signal.
    - **Grenze (H):** Ordner, die nach einem Absturz oder Stromausfall unter `lauf.sitzungen` liegen bleiben, belegen ihre Nummer, bis sie jemand entfernt; je Lauf des Dienstes gibt es höchstens 255 Nummern. Das Aufräumen solcher Rückstände beim Dienststart (mit PID-Schutz) ist ein offener Punkt für Schritt II.
    - **Prüfmittel:** das Fallfeld `sitzungen` des Prüfclients: `{"vorher": [{"name": "s-1", "art": "ordner" | "datei" | "link_tot" | "link_ordner"}, …], "fehlt": true (der Ordner wird nicht angelegt), "modus": "0000" (Modus des Ordners nach dem Anlegen der Einträge)}`. Der Prüfclient legt `lauf.sitzungen` als eigenen Wegwerfordner des Falls an (`<base>/sitzungen`), setzt `@sitzungen` in der Startdatei auf seinen Pfad und gibt am Ende `ende.sitzungen_inhalt` (die Namen darin, sortiert) aus. Ein Fall mit Modus `0000` setzt einen Lauf **nicht als Superuser** voraus (der lstat als Superuser scheitert nicht).
14. **Marker im Probelauf** (Nachtrag 21.11.2026, mit den Nachträgen 1 und 2; Herkunft: Vertrag 1 `rust/spec/start/README.md`, Abschnitt „Marker“; an earlier test run, Cloud-Route, Zählung `MP-2C5`; Auflagen, 21.11.2026).
    - **Zwei Schlüssel in `lauf`** (beide optional, in beiden Betriebsarten, je Text von 1 bis 4096 Byte, ohne NUL, absoluter Pfad, geprüft wie `lauf.einstellungen`): **`marker_datei`** (die Datei der Marker) und **`marker_ordner`** (der Zählordner). Jeder andere Wert:
      Exitcode 2 mit der Zeile `lauf.marker_datei muss ein absoluter Pfad sein` bzw. `lauf.marker_ordner muss ein absoluter Pfad sein`. Ohne die Schlüssel gilt alles wie vorher; die alten Fälle bleiben gleich.
    - **Die Datei ist nur für den Probelauf gedacht.** Im Paket schreibt `postinst` den festen Pfad `/run/user/<uid>/werkbank-probelauf/marker.json` in `lauf.marker_datei`; der **Läufer** (`werkbank-probelauf`) **legt die Datei nur im Probelauf an und entfernt sie am Ende**. Ihr Ordner gehört dem Nutzer.
      **Aus der Datei, die der Nutzer schreiben kann, kommt nur die Liste. Der Zählordner kommt aus der Startdatei (`lauf.marker_ordner`, die nur root schreibt); `postinst` schreibt ihn (eigener Ordner 0700 unter dem Zustandsordner des Nutzers).** Der Dienst legt den Zählordner nicht an und prüft ihn nicht
      weiter (das tut „Lauf starten“, Schritt 5a des Vertrags 1).
    - **Lesen: genau einmal beim Start des Dienstes**, nach der Startdatei und der Rechteprüfung, vor dem Socket; danach nie wieder (auch nicht, wenn die Datei sich ändert oder verschwindet; es gilt der Stand vom Start).
      1. **Der Elternordner** wird mit `O_DIRECTORY | O_NOFOLLOW | O_CLOEXEC` geöffnet. **Fehlt er** (`NotFound`), gibt es keine Marker (Normalfall, kein Fehler). Jeder andere Fehler ist ungültig (Grund `ordner`, auch eine Verknüpfung). Der Eigentümer muss die eigene Kennung sein (`ordner_eigentuemer`), der Modus **genau 0700** (`ordner_modus`).
      2. **Die Datei** wird relativ zum Ordner mit `openat(…, O_RDONLY | O_NOFOLLOW | O_CLOEXEC)` geöffnet. **Fehlt sie** (`NotFound`): keine Marker (Normalfall, auch wenn `lauf.marker_ordner` gesetzt ist). Das Öffnen geschieht **mit `O_NONBLOCK`** (bei einer regulären Datei ohne Wirkung): Eine FIFO an der Stelle der Datei würde sonst das Öffnen zum Lesen bis zu einem Schreiber blockieren und den Start des Dienstes anhalten; so öffnet sie sofort und `fstat` lehnt sie mit `datei_art` ab. (Der Elternordner wird mit `O_DIRECTORY` geöffnet: Eine FIFO dort ergibt `ENOTDIR`, also Grund `ordner`, kein Warten.) Eine Verknüpfung: `datei_verknuepfung`; anderer Fehler: `datei_unlesbar`. Per `fstat` am offenen Deskriptor:
         eine reguläre Datei (`datei_art`), eigener Eigentümer (`datei_eigentuemer`), Modus **genau 0600** (`datei_modus`). Gelesen werden **höchstens 4096 Byte**; mehr ist `zu_gross` (4096 Byte gehen, 4097 nicht).
      3. **Inhalt:** UTF-8 (`kein_utf8`), JSON (`kein_json`), ein Objekt mit **genau** dem Schlüssel `liste`, einer Liste von Texten (jede Abweichung, auch ein Schlüssel `ordner` oder ein anderer, ist `form`). Die Form der Liste (1 bis 16 Marker, je 1 bis 32 Zeichen aus `A-Z`, `0-9`, `-`, kein Doppel) prüft allein das Crate `start`
         (`start::Marker::neu`); der Dienst hat keine eigene Formprüfung (die Referenz hat eigenen Code, die Fälle halten beide gleich).
      4. **Zusammensetzen:** Mit gültiger Liste baut der Dienst `start::Marker::neu(liste, lauf.marker_ordner)`. **Fehlt `lauf.marker_ordner`**, oder lehnt `Marker::neu` den Ordner ab (etwa `//` oder `..`): Exitcode 2, der Dienst startet nicht (Zeile `lauf.marker_ordner fehlt, die Marker-Datei braucht ihn` bzw. `lauf.marker_ordner ungueltig`).
         **Fehlt die Datei**, gibt es keine Marker, auch wenn `lauf.marker_ordner` gesetzt ist.
      5. **Jede Abweichung** (Schritte 1 bis 3) ist Exitcode 2 mit einer Zeile `lauf.marker_datei ungueltig: <fester Grund>` (Gründe: `pfad`, `ordner`, `ordner_eigentuemer`, `ordner_modus`, `datei_verknuepfung`, `datei_unlesbar`, `datei_art`, `datei_eigentuemer`, `datei_modus`, `zu_gross`, `kein_utf8`, `kein_json`, `form`); **die Zeile enthält weder den Pfad noch den Inhalt**. Der Dienst startet dann nicht.
    - **Start:** Mit gelesenen Markern schreibt der Dienst bei **jedem** `Starten` `marker` in die `start.json` (`Startanfrage::mit_sitzung(kennung, Some(marker))`), gleich dem Stand vom Start; ohne Marker steht `marker` in keiner `start.json` (`mit_sitzung(kennung, None)`).
      **Die Marker wirken nur auf Zählung und Bericht:** der Dienst nutzt sie für nichts anderes als für `mit_sitzung`; keine Prüfung des Starts, keine Route, keine Klasse, kein Ziel hängt davon ab.
    - **Anfrage der Ansicht:** Eine `Starten`-Anfrage mit `marker` bleibt abgelehnt (`Ablehnung start_pruefung`, `start.marker_nicht_erlaubt`, Stufe 4), **auch** wenn der Dienst Marker trägt; die Anfrage kann den Wert des Dienstes nie ersetzen, und es wird nichts angelegt.
    - **Prüfmittel:** das Fallfeld `marker_datei` des Prüfclients: `{"art": "datei" (Vorgabe) | "fehlt" (Ordner, keine Datei) | "ordner_fehlt" | "link" (die Datei ist eine Verknüpfung) | "fifo" (eine FIFO an der Stelle der Datei) | "ordner_link", "inhalt": <JSON-Wert> | "roh": <Text> | "hex": <Byte als Hex>, "auffuellen": <Byte Leerzeichen>, "modus": "0600", "ordner_modus": "0700"}`;
      der Prüfclient legt `<base>/mk/marker.json` an und setzt `@marker_datei` in der Startdatei auf den Pfad; der Schritt `{"art": "MarkerDatei", "inhalt": …}` schreibt die Datei nach dem Start neu. Das feste Kindprogramm `bericht` gibt den gelesenen Inhalt der `start.json` aus, so dass `marker` im Ergebnis steht oder fehlt.
      Ein fremder Eigentümer ist in den Fällen nicht prüfbar (ohne Superuser); er ist im Bau ein Einheitstest über die Kennungen von Ordner und Datei (`lesen_mit`).
15. **Keine Kennung, deren Zielliste noch liegt** (Nachtrag 21.11.2026; Herkunft: an earlier test run, Projektentscheid, 21.11.2026: `werkbank-lauf` legt je Cloud-Sitzung `<zustand>/sitzungen/s-<n>.zielliste.json` an und lässt sie mit Absicht liegen, weil die Schlüsselsuche sie nach der Sitzung braucht; nach einem Neustart zählt der Kern wieder ab `s-1`, und ein zweiter Lauf scheitert mit Code 73, weil die liegende Datei nicht überschrieben wird. Projektentscheid: Der Kern vergibt keine Kennung, deren Zielliste noch liegt).
    - **Startdatei:** `lauf` erlaubt zusätzlich die optionalen Schlüssel `ziellisten` und `laufzeit` (in beiden Betriebsarten): je Text von 1 bis 4096 Byte, ohne NUL, **absoluter** Pfad, geprüft wie `lauf.sitzungen`. Jeder andere Wert: Exitcode 2 mit der Zeile `lauf.ziellisten muss ein absoluter Pfad sein` bzw.
      `lauf.laufzeit muss ein absoluter Pfad sein`. Das Paket schreibt `lauf.ziellisten` als `$DATEN/<nutzer>/sitzungen` und `lauf.laufzeit` als `$DATEN/l/<uid>` (genau den Wert `laufzeit` aus `lauf.json`; eigene Paketschritte). Ohne die Schlüssel bleibt alles wie vorher.
    - **Vergabe bei `Starten` (Nachtrag 2: der Kern überspringt eine Nummer, wenn IRGENDEINE ihrer Dateien liegt):** Eine Nummer `n` ist **nur frei, wenn `lstat` für JEDEN dieser Einträge `NotFound` ergibt**; jeder andere Erfolg (Datei, Ordner, Verweis, toter Verweis) belegt sie, jeder andere Fehler
      lehnt den Start ab (`Ablehnung start_ordner`, nichts wird angelegt, keine Nummer verbraucht). **Ein Eintrag zählt nur, wenn sein Schlüssel gesetzt ist** (ohne den Schlüssel gibt es den Eintrag nicht; ein fehlender Ordner ergibt `NotFound`, die Nummer ist dann frei):

      | Eintrag | Schlüssel | Anleger |
      |---|---|---|
      | `<lauf.sitzungen>/s-<n>` | `lauf.sitzungen` | `werkbank-lauf`, Sitzungsordner |
      | `<lauf.laufzeit>/s-<n>` | `lauf.laufzeit` | `werkbank-lauf`, Socket-Ordner (Anlegen mit `exklusiv`: eine liegende Datei lässt die nächste Sitzung mit derselben Kennung mit Code 69 scheitern) |
      | `<lauf.ziellisten>/s-<n>.zielliste.json` | `lauf.ziellisten` | `werkbank-lauf`, Zielliste (bleibt mit Absicht liegen; Code 73) |
      | `<lauf.marker_ordner>/s-<n>.suche.json` und `<lauf.marker_ordner>/s-<n>.modellweg.json` | `lauf.marker_ordner` | die Zähldateien von Suche und Modellweg |
      | `<lauf.gespraeche>/s-<n>` | `lauf.gespraeche` | `werkbank-lauf`, Gespräch des Harness (Nachtrag 27.11.2026) |

      Die Namen sind genau so (`s-<n>` ohne den Zusatz oder ein Teil davon belegt in `lauf.ziellisten` oder `lauf.marker_ordner` nichts). **Die Einträge stehen im Bau an genau einer Stelle** (`starten.rs`, die Funktion `eintraege_der_nummer`): Kommt später ein Eintrag dazu, ist das eine Zeile plus ein Fall.
      Die Einträge werden in der Reihenfolge der Tabelle gelesen; der erste Eintrag, der liegt, beantwortet (ein Fehler bei einem späteren Eintrag zählt dann nicht). `nummer_max` (und 255) gilt für jedes geprüfte `n`; `nummern_erschoepft` bleibt die Antwort, wenn keine Nummer frei ist. `naechste` wird wie bisher erst nach einem gelungenen Start gesetzt.
    - **Der Dienst legt in keinem dieser Ordner etwas an und liest dort keine Datei. Er entfernt dort nichts, außer der einen Zielliste nach Punkt 15a** (Regel 4 von DI12 Punkt 13 gilt sonst auch hier).
    - **Gespräche (Nachtrag 27.11.2026).** Herkunft, gemessen: Nach einem Neustart des Dienstes bekam ein Start wieder `s-1` und endete mit 73 `lauf.sitzung_belegt`, weil `gespraeche/s-1` vom ersten Lauf noch lag. Ab Stufe B legt jeder Start von `werkbank-lauf` `gespraeche/<sitzung>/` an, auch mit Fassung 1.
      Regel: `lauf.gespraeche` (optional, DI-E4 Punkt 2, geprüft wie `lauf.bestand`) ist die letzte Zeile der Tabelle. Besteht `<lauf.gespraeche>/s-<n>` in irgendeiner Art, auch als Verweis oder toter Verweis, ist `n` belegt und wird übersprungen wie eine liegende Zielliste. Der Dienst sieht nur mit `lstat` nach und folgt nie einem Verweis.
      Punkt 15a räumt kein Gespräch; eine Zielliste neben einem Gespräch derselben Nummer bleibt liegen. Der Dienst legt unter `lauf.gespraeche` nichts an und liest dort nichts; er entfernt dort nur beim `Loeschen` nach DI-E4.
      Fall `di12_gespraech_belegt` (Gespräche `s-1`, `s-3` und ein fremdes `s-9` liegen; vergeben werden `s-2`, `s-4` bis `s-8` und `s-10`, nichts dort ändert sich), Mutant `gespraeche_vergabe_ungeprueft`. Der neue Schritt `GespraechAnlegen {"sitzung"}` des Prüfclients legt ein Gespräch wie das Fall-Feld `gespraeche` an, aber erst im Lauf.
      Die Fälle `die4_loeschen_gespraech`, `die4_loeschen_gespraeche_unsicher` und `die4_loeschen_gespraech_fortgesetzt` legen die Gespräche ihrer Sitzungen deshalb mit diesem Schritt nach `Gestartet` an, wie `werkbank-lauf` ab Stufe B. Vor dem Start bleibt nur der fremde Eintrag. Ihre Erwartungen ändern sich nur um die Ergebnisse der neuen Schritte.

15a. **Die alte Zielliste der gesuchten Nummer räumt der Kern beim Vergeben** (Nachtrag 23.11.2026). Anlass: Nur der Läufer räumt eine Zielliste. Im Betrieb ohne Läufer bliebe je Cloud-Sitzung eine Liste liegen. Nach einem Neustart des Kerns würden die Listen Nummer um Nummer belegen, bis `nummern_erschoepft` jeden Start ablehnt.
    - **Der Projektentscheid vom 21.11.2026 gilt weiter:** Eine liegende Liste belegt ihre Nummer. Nur die eigene reguläre Liste einer früheren Nummer räumt der Kern beim Vergeben.
    - **Wann:** Bei `Starten` ergibt die Prüfung nach Punkt 15, dass `n` belegt ist. Der Kern fragt nur nach Nummern über `naechste`. Eine Liste dieser Nummer stammt deshalb aus einem früheren Lauf des Kerns oder aus einem Start außerhalb des Kerns. Die Listen der Sitzungen dieses Laufs bleiben liegen, denn der Läufer liest sie nach seinen Sitzungen (`werkbank-modellweg suche --zielliste`).
    - **Bedingungen, alle zugleich:** `lauf.ziellisten` ist gesetzt. Jeder andere Eintrag von `n` aus der Tabelle in Punkt 15 ergibt mit `lstat` `NotFound` (Sitzungsordner, Socket-Ordner, beide Zähldateien). Der Ordner `lauf.ziellisten` öffnet sich mit `openat(AT_FDCWD, …, O_DIRECTORY | O_NOFOLLOW | O_CLOEXEC)` und gehört laut `fstat` dem eigenen Nutzer. `fstatat(<Deskriptor>, "s-<n>.zielliste.json", AT_SYMLINK_NOFOLLOW)` zeigt eine reguläre Datei des eigenen Nutzers.
    - **Wie:** Der Kern entfernt die Liste mit `unlinkat(<Deskriptor>, "s-<n>.zielliste.json", 0)`, nie über einen Pfad. Danach prüft er `n` nach Punkt 15 erneut. Ist `n` jetzt frei, vergibt er `n`; sonst geht die Suche mit `n + 1` weiter wie bisher.
    - **Was weiter belegt:** ein Verweis (auch ein toter), ein Ordner, eine Datei eines fremden Nutzers, eine Liste neben einem anderen Eintrag derselben Nummer und jede Liste unter einem Ordner `lauf.ziellisten`, der ein Verweis ist oder einem anderen Nutzer gehört. In jedem Zweifel entfernt der Kern nichts. Ein Fehler beim Entfernen ändert die Antwort nicht: Die Nummer bleibt belegt.
    - **Kennung, nicht Rahmennummer:** Die Regel arbeitet auf der Zahl der Kennung `s-<n>` und setzt keine Obergrenze voraus. Eine Trennung von Kennung und Rahmennummer ändert an ihr nichts.
    - **Mit der Rasternummer nach P25 (Nachtrag 24.11.2026):** Die Kennung zählt nach einem Neustart wieder ab `s-1`; dort vergibt der Kern eine Kennung einer beendeten Sitzung eines früheren Laufs neu, und nur dort räumt er nach diesem Punkt. Die Neuvergabe einer Rasternummer (Abschnitt 19) berührt keine Datei. Ist keine Rasternummer frei, lehnt der Kern mit `nummern_erschoepft` ab, bevor er eine Kennung sucht: Ein abgelehnter Start räumt keine Liste. Die Erwartungen der vier geänderten Fälle tragen `nr` gleich der Zahl der neuen Kennung. Fall `mehrere.rs` `neustart_vergibt_kennung_mit_alter_zielliste_und_rasternummer_getrennt` prüft beides zugleich.
    - **Bau:** `starten.rs`, Funktionen `alte_zielliste_raeumen` und `nummer_vergebbar`. Die Schleife der Nummer in `dienst.rs` fragt `nummer_vergebbar` statt `kennung_belegt`.
    - **Geänderte Fälle (alt -> neu):** `di12_zielliste_belegt_datei` s-2 -> s-1, die Liste ist danach geräumt. `di12_zielliste_zwei` s-3 -> s-1, die Liste von s-2 bleibt. `di12_zielliste_luecke` zweiter Start s-3 -> s-2. `di12_zielliste_und_sitzungsordner` s-3 -> s-2. `di12_zielliste_nummer_max`: Der Aufbau von s-1 wird von einer Datei zu einem Ordner, damit der Fall weiter `nummern_erschoepft` prüft. Die Erwartung bleibt. Der Test `mehrere.rs` `nach_dem_neustart_keine_kennung_deren_zielliste_liegt` heißt jetzt `nach_dem_neustart_raeumt_der_dienst_die_alte_zielliste_der_nummer`. Nach dem Neustart vergibt der Kern wieder s-1, und das Kind schreibt eine neue Liste ohne Code 73.
    - **Unverändert:** `di12_nummer_alle_fuenf`, `di12_nummer_zielliste_link_tot`, `di12_zielliste_belegt_link`, `di12_zielliste_belegt_link_tot`, `di12_zielliste_belegt_ordner`, `di12_zielliste_ohne_rechte`, `di12_zielliste_falscher_name`, `di12_zielliste_ohne_schluessel` und `di12_zielliste_ordner_fehlt`.
    - **Einheitstest:** `eine_alte_eigene_zielliste_allein_wird_geraeumt` in `starten.rs` prüft Liste allein, Verweis, toten Verweis, Ordner, je einen anderen Eintrag der Nummer, einen fehlenden Schlüssel, einen verlinkten Ordner `lauf.ziellisten` und eine Nummer über 255. Die fremde uid lässt sich ohne Superuser nicht prüfen; das ist offen. Der Prüfclient kennt keinen verlinkten Ordner `lauf.ziellisten`, deshalb gibt es dafür noch keinen Außenfall.
    - **Mutanten:** Der Löschcode wird nur im Einschluss mutiert (Projektentscheid, 22.11.2026).
    - **Prüfmittel:** die Fallfelder `ziellisten`, `laufzeit` und `marker_zaehl` des Prüfclients wie `sitzungen` (`vorher`: Einträge `{name, art}` mit dem ganzen Dateinamen, `fehlt`, `modus`); `@ziellisten`, `@laufzeit` und `@marker_zaehl` in der Startdatei; `ende.<Feld>_inhalt` (die Namen darin, sortiert). Ein Fall mit Modus `0000` setzt einen Lauf nicht als Superuser voraus.

## 8. Festgelegt im Vertrag

Der Plan lässt diese Fragen offen; der Vertrag entscheidet klein:

1. Eine Sitzung, eine Person in der Startdatei für alle Betrachter (`EN` Abschnitt 1). Die Sitzungsnummer des Rasterauszugs ist immer 0.
2. Der Rahmen auf dem Socket hat eine eigene Art; die Nutzlast `Raster` ist der Rahmen nach `RA` (nach Deflate). Alles andere ist JSON.
3. Fehler im Protokoll beenden die Verbindung mit einem `Fehler`-Rahmen; Ablehnungen des Kerns beenden sie nie.
4. Ein Client kann nur Ereignisse der Ansicht geben; `t` und `betrachter` setzt der Dienst, ein Client, der sie nennt, hat einen Fehler gemacht.
5. `ausstehend` zählt die Byte der Schlange des Dienstes mit Kopf von 5 Byte je Rahmen, nicht den Kernelpuffer (Risiko 7, offen).
6. Die Meldung `Socket` je Nachricht und je verkürzendem Schreiben; die Meldungen laufen breitenweise, damit die Folge des Kerns bleibt.
7. Der Dienst startet nie von selbst ein Kind außerhalb der Startdatei und nie ohne die Rechteprüfung; das Kind startet vor dem Binden.
8. Der Neubau des Kerns (DI6) nimmt den Stand des Trägers, nicht den der Startdatei.
9. Die Uhr im Prüfmodus kommt nur von Clients; sie geht nie zurück.
10. Ordner: genau 0700, Besitzer der effektive Benutzer, kein Link; Socket: genau 0600; der Socket-Pfad darf nicht bestehen; der Dienst räumt nur auf, was er selbst angelegt hat.
11. Rechte werden vor dem Kind und vor dem Binden geprüft. Ein Startfehler hinterlässt nichts (kein Kind, kein Socket).
12. Zahl der Betrachter, Größe der Warteschlange und Last sind nicht Teil der Außentests (P4, P5 misst ein späterer Schritt mit dem Läufer).
13. (Nachtrag 20.11.2026) Obergrenze der Warteschlange 33 554 432 Byte je Betrachter, Trennen mit Zeile im Protokoll (stderr), kein Fehlerrahmen; volles Abbild beim Neuverbinden (Projektentscheid, 19.11.2026 F4).
14. (Nachtrag 20.11.2026) Anfangscursor `[0, 0, true]` immer; beim Neubau der Cursor des Trägers (Projektentscheid, 19.11.2026 F2).
15. (Nachtrag 20.11.2026) Kleinste Größe 2 Spalten, an einer Stelle; Größen mit einer Spalte lehnt der Dienst ab (Projektentscheid, 20.11.2026).
16. (Nachtrag 20.11.2026) `geteuid`: siehe Abschnitt 11 (Projektentscheid, 19.11.2026 F5).
17. (Nachtrag 20.11.2026, DI12) Ein Dienst, mehrere Sitzungen: Raumkern und ein Kern je Sitzung, Sitzungsnummer 0 für `kind` und `n` für `s-<n>`, die Liste führt der Dienst, `Starten` und `Beenden` sind Ereignisse des Dienstes, die Prüfstelle `start` ruft der Bau (DI12 Punkt 4).
18. (Nachtrag 22.11.2026) `lauf` meldet Code und Grund über einen Socket im Ordner der Sitzung; der Dienst wertet eine einzige, streng geformte Zeile beim Kindende (Abschnitt 18).
19. (Nachtrag 23.11.2026, Projektentscheid, 23.11.2026) Gründe des Dienstes im `Hinweis` statt einer zu langen Nachricht (DI3, Grenze hinaus für `Nachricht`): `zwischenablage_zu_gross` für eine Nachricht `Zwischenablage`, `nachricht_zu_gross` für jede andere. Es sind feste Wörter; die Verbindung bleibt.
20. (Nachtrag 23.11.2026, Projektentscheid, 23.11.2026) Gründe des Dienstes statt eines zu langen JSON-Rahmens hinaus (DI3, Grenze hinaus für alle JSON-Rahmen): `zustand_zu_gross` im `Hinweis` eines Rahmens `Zustand`, `ablehnung_zu_gross` als `grund` eines Rahmens `Ablehnung`, `fehler_zu_gross` als `fehler` eines Rahmens `Fehler`. Es sind feste Wörter. Nur `zustand_zu_gross` kann vorkommen; die anderen beiden schließen die Lücke der gemeinsamen Prüfung (hergeleitet: `Ablehnung` und `Fehler` erreichen die Grenze nie). Die Verbindung bleibt außer nach `Fehler`.
21. (Nachtrag 26.11.2026, Ordner lesen und durchsuchen, L9) Gründe der Ablehnung von `OrdnerLesen` und `InhaltSuchen`: `form_ungueltig`, `sitzung_unbekannt`, `fremder_kundenraum`, `pfad_ungueltig`, `muster_ungueltig`, `grenze`, `klasse_gesperrt`, `pfad_ausserhalb`, `nicht_lesbar` und `plattform_gesperrt`. Es sind feste Wörter im Rahmen `Ablehnung`; sie nennen keinen Pfad, keinen Namen und kein Muster. Die Bedeutung und die Reihenfolge stehen im Abschnitt „Nachtrag 26.11.2026: Ordner lesen und durchsuchen (L9)“.

## 9. Gegenproben (Mutanten)

Jeder Mutant schaltet sich in der Referenz mit `REF_GEGENPROBE=<name>` zu. `gegenprobe.sh` prüft, dass diese Liste und `dienst_ref.py --gegenproben` gleich sind und dass jeder Mutant auf **mindestens einen** Fall in einem der beiden Sätze rot macht.
Zwei Mutanten sind wegen des Zeitverhaltens zu zwei Läufen gedacht und verlassen sich auf eine Frist (`proto_laenge_ohne_grenze`, `monoton_ohne_takt`); `pty_vor_groesse` schreibt das Byte vor der Größe mit einer Lücke von 150 ms (ein Bau mit falscher Reihenfolge fällt nur mit hoher Wahrscheinlichkeit auf, nicht sicher).

| Mutant | Regel | Änderung |
|---|---|---|
| `start_unbekannter_schluessel_ok` | DI1 | ein unbekannter Schlüssel der Startdatei wird übergangen |
| `start_groesse_1001_ok` | DI1 | die Größe 1001 wird angenommen |
| `start_ruecklauf_ohne_grenze` | DI1 | ruecklauf über 100 000 wird angenommen |
| `start_mehrere_sitzungen_ok` | DI1 | mehrere Sitzungen in `anfang` werden angenommen |
| `start_relativer_pfad_ok` | DI1 | `kind.argv[0]` darf relativ sein |
| `start_datei_ohne_grenze` | DI1 | die Größe der Startdatei wird nicht begrenzt |
| `start_socketname_pfad_ok` | DI1 | `socket.name` darf `/` enthalten |
| `start_traegerfeld_ok` | DI1 | `zeilen`, `stand`, `bracketed_paste`, `invers` in der Sitzung werden angenommen |
| `rechte_modus_nicht_geprueft` | DI2 | der Modus des Ordners wird nicht geprüft |
| `rechte_0750_ok` | DI2 | nur die Rechte für andere werden geprüft (0750 und 0500 gehen) |
| `rechte_ordner_anlegen` | DI2 | ein fehlender Ordner wird mit 0700 angelegt |
| `rechte_link_folgen` | DI2 | ein symbolischer Link als Ordner wird verfolgt |
| `rechte_socket_ersetzen` | DI2 | ein vorhandener Socket-Pfad wird gelöscht und ersetzt |
| `rechte_socket_modus_umask` | DI2 | der Socket hat Modus 0666 statt 0600 |
| `rechte_besitzer_nicht_geprueft` | DI2 | der Besitzer des Ordners wird nicht geprüft |
| `rechte_tcp_hoerer` | DI2 | der Dienst öffnet zusätzlich einen TCP-Hörer auf 127.0.0.1 |
| `rechte_socket_bleibt` | DI2 | nach SIGTERM bleibt die Socket-Datei stehen |
| `proto_laenge_ohne_grenze` | DI3 | die Rahmenlänge ist nicht begrenzt (der Dienst wartet die Nutzlast ab) |
| `proto_laenge_null_ok` | DI3 | Länge 0 wird ohne Fehler übergangen |
| `proto_unbekannte_art_ignoriert` | DI3 | eine unbekannte Rahmenart wird übergangen |
| `proto_json_fehler_bleibt` | DI3 | ungültiges JSON wird übergangen, die Verbindung bleibt |
| `proto_t_vom_client` | DI3 | ein Client darf `t` setzen |
| `proto_betrachter_vom_client` | DI3 | ein Client darf `betrachter` setzen |
| `proto_traegerereignis_ok` | DI3 | Ereignisse des Trägers und des Kerns (`Ausgabe`, `Kindende`, `Socket`, `Modus`, `Titel`, `Zwischenablage`, `Takt`) werden von Clients angenommen |
| `proto_verbinden_ok` | DI3 | `Verbinden` und `Trennen` werden von Clients angenommen |
| `proto_pruef_ohne_modus` | DI3 | Rahmen des Prüfmodus gehen auch ohne `pruefmodus` |
| `proto_fehler_ohne_trennen` | DI3 | nach einem Fehler bleibt die Verbindung offen |
| `proto_ablehnung_verschluckt` | DI3 | Ablehnungen des Kerns gehen an niemanden |
| `proto_ablehnung_an_alle` | DI3 | Ablehnungen gehen an alle Betrachter statt an den Sender |
| `proto_abgeschnitten_absturz` | DI3 | ein abgeschnittener Rahmen am Ende der Verbindung bringt den Dienst zum Absturz |
| `betr_name_wiederverwenden` | DI4 | der kleinste freie Name wird wieder vergeben |
| `betr_trennen_nicht_gemeldet` | DI4 | das Ende einer Verbindung geht nicht als `Trennen` an den Kern |
| `betr_deflate_ignoriert` | DI4 | `deflate` der Startdatei wirkt nicht (die Rahmen gehen ohne Deflate) |
| `betr_nr_eins` | DI4 | die Sitzungsnummer des Rasterauszugs ist 1 statt 0 |
| `stau_nicht_gemeldet` | DI5 | der Dienst gibt dem Kern nie ein `Socket` |
| `stau_ohne_kopf` | DI5 | `ausstehend` zählt den Kopf von 5 Byte je Rahmen nicht |
| `stau_stumm_ignoriert` | DI5 | der Rahmen `Stumm` wirkt nicht |
| `stau_alles_blockiert` | DI5 | solange ein Betrachter stumm ist, geht an keinen etwas hinaus |
| `stau_warteschlange_verwerfen` | DI5 | Rahmen für einen stummen Betrachter werden verworfen statt aufbewahrt |
| `stau_bericht_nur_wachsen` | DI5 | `Socket` kommt nur bei wachsender Schlange, nicht nach dem Schreiben |
| `stau_ohne_leerbericht` | DI5 | nach dem Leeren der Schlange kommt kein `Socket` mit 0 |
| `kernfehler_ohne_neubau` | DI6 | `Kernfehler` hat keine Wirkung (der Kern wird nicht neu gebaut) |
| `kernfehler_kein_abo` | DI6 | nach dem Neubau gibt es kein `Abo`, also keinen Abzug |
| `kernfehler_leerer_schirm` | DI6 | der neue Kern startet mit leerem Schirm statt mit dem Stand des Trägers |
| `kernfehler_betrachter_verloren` | DI6 | nach dem Neubau kennt der Kern keinen Betrachter |
| `uhr_ignoriert` | DI7 | der Rahmen `Uhr` ändert die Uhr nicht |
| `uhr_rueckwaerts_ok` | DI7 | die Uhr darf zurückgehen |
| `uhr_ohne_takt` | DI7 | `Uhr` setzt die Zeit, gibt dem Kern aber kein `Takt` |
| `uhr_echtzeit` | DI7 | im Prüfmodus gilt die echte Uhr statt der eingespeisten |
| `monoton_ohne_takt` | DI7 | im Betrieb gibt der Dienst dem Kern an `naechste_frist` kein `Takt` |
| `kind_code_plus_eins` | DI8 | `Kindende` meldet den Code plus 1 |
| `kind_signal_roh` | DI8 | ein Kind, das durch ein Signal endet, meldet die Signalnummer statt 128 plus Nummer |
| `kind_env_erweitert` | DI8 | das Kind bekommt eine zusätzliche Umgebungsvariable |
| `kind_cwd_nicht_gesetzt` | DI8 | das Arbeitsverzeichnis des Kindes ist das des Dienstes |
| `pty_vor_groesse` | DI8 | die Bytes der Eingabe gehen vor der neuen Größe an die PTY (Lücke von 150 ms) |
| `groesse_pty_nicht` | DI8 | die PTY behält ihre Größe, nur der Träger ändert sie |
| `groesse_ereignis_sofort` | DI4 | die Ereignisse des Trägers für eine neue Größe gehen vor den Nachrichten des Schritts an den Kern (der Betrachter bekommt die Änderung vor dem `Abzug`, danach fehlt ihm der Stand) |
| `eingabe_cr_zu_lf` | DI8 | CR wird auf dem Weg zur PTY zu LF |
| `kind_bleibt_am_leben` | DI8 | nach SIGTERM wird das Kind nicht beendet |
| `start_eine_spalte_ok` | DI1 | eine Größe mit einer Spalte in der Startdatei wird angenommen (Nachtrag 20.11.2026, Projektentscheid, 20.11.2026) |
| `groesse_eine_spalte_durchlassen` | DI4 | `Abo` und `Groesse` mit einer Spalte gehen an den Kern, das Träger-Modell nimmt sie an (Nachtrag 20.11.2026) |
| `start_wmax_ohne_pruefmodus_ok` | DI1 | `warteschlange_max` wird auch ohne `pruefmodus` angenommen (Nachtrag 20.11.2026, Projektentscheid, 19.11.2026 F4) |
| `start_wmax_ueber_grenze_ok` | DI1 | `warteschlange_max` über 33 554 432 wird angenommen |
| `stau_grenze_fehlt` | DI5 | die Obergrenze der Warteschlange fehlt (Nachtrag 20.11.2026) |
| `stau_grenze_ein_byte_frueh` | DI5 | getrennt wird schon bei genau der Grenze (`>=` statt `>`) |
| `stau_grenze_ein_byte_spaet` | DI5 | getrennt wird erst ein Byte nach der Grenze (`> Grenze + 1`) |
| `stau_grenze_nur_pruefmodus` | DI5 | im Betrieb (ohne `pruefmodus`) gilt keine Obergrenze |
| `stau_grenze_vorgabe_klein` | DI5 | die Grenze ohne `warteschlange_max` ist 16 777 216 statt 33 554 432 |
| `stau_grenze_ohne_trennen` | DI5 | der Dienst schließt den Betrachter, meldet dem Kern aber kein `Trennen` |
| `stau_grenze_kein_protokoll` | DI5 | der Dienst schreibt keine Zeile in sein Protokoll |
| `stau_grenze_alle_trennen` | DI5 | die Obergrenze trennt alle Betrachter statt des einen |
| `stau_neuverbinden_ohne_abzug` | DI5 | der erste Betrachter nach dem Trennen bekommt auf sein `Abo` keinen Abzug (der Dienst gibt das `Abo` nicht weiter) |
| `anfang_cursor_fehlt` | DI11 | der Dienst gibt dem Kern keinen Anfangscursor mit (Nachtrag 20.11.2026, Projektentscheid, 19.11.2026 F2) |
| `anfang_cursor_unsichtbar` | DI11 | der Anfangscursor ist `[0, 0, false]` |
| `start_cursorfeld_ok` | DI1, DI11 | `anfang.sitzungen[].cursor` in der Startdatei wird angenommen |
| `kernfehler_cursor_anfang` | DI6, DI11 | der neue Kern bekommt den Anfangscursor statt des Cursors des Trägers |
| `kernfehler_cursor_fehlt` | DI6, DI11 | der neue Kern bekommt keinen Cursor |
| `traeger_cursor_fehlt` | DI11 | die `Ausgabe` des Trägers trägt keinen `cursor` |
| `traeger_cursor_nur_zeilen` | DI11 | ein reiner Cursorschritt (keine Zeile ändert sich) wird nicht gemeldet |
| `traeger_cursor_sichtbar_fehlt` | DI11 | DECTCEM (`?25l`, `?25h`) ändert `sichtbar` nicht |
| `traeger_cursor_umbruch_sofort` | DI11 | nach dem Zeichen in der letzten Spalte steht der Cursor schon in der nächsten Zeile (kein ausstehender Umbruch) |
| `kernfehler_cursor_aus_zustand` | DI6, DI11 | der neue Kern bekommt den `cursor` aus `zustand` des alten Kerns statt des Trägers (bei wartender Synchronisierung der alte Stand; Projektentscheid, 20.11.2026) |
| `stau_grenze_protokoll_mit_inhalt` | DI5 | die Protokollzeile nennt zusätzlich den Titel der Sitzung (Projektentscheid, 20.11.2026) |
| `traeger_groesse_zeilen_oben` | DI11 | beim Verkleinern der Zeilen behält das Träger-Modell die oberen Zeilen statt der unteren (der Cursor wird nur geklemmt, die Zeilen wandern nicht mit) |
| `lauf_starten_ohne_lauf_ablehnung` | DI12 | ohne `lauf` wird `Starten` oder `Beenden` mit `Ablehnung start_form` beantwortet statt mit dem Fehler `art_nicht_erlaubt` (Nachtrag 20.11.2026, Projektentscheid, 20.11.2026 (a)) |
| `lauf_unbekannter_schluessel_ok` | DI1, DI12 | ein unbekannter Schlüssel in `lauf` wird übergangen |
| `lauf_programm_relativ_ok` | DI1, DI12 | `lauf.programm` darf ein relativer Pfad sein |
| `lauf_eine_spalte_ok` | DI1, DI12 | `lauf.groesse` mit einer Spalte wird angenommen |
| `lauf_kind_id_kollision_ok` | DI1, DI12 | die Sitzung aus `kind` darf mit `lauf` eine Kennung der Form `s-<n>` tragen |
| `lauf_groesse_ohne_kind_ok` | DI1, DI12 | `groesse` ohne `kind` wird mit `lauf` angenommen |
| `starten_form_nicht_objekt_ok` | DI12 | eine Start-Struktur, die kein Objekt ist, wird wie ein leeres Objekt behandelt (Stufe 2 fehlt) |
| `starten_gross_ok` | DI12 | die Start-Struktur ist nicht auf 4096 Byte begrenzt (Stufe 1 fehlt) |
| `starten_grenze_frueh` | DI12 | schon 4096 Byte sind zu groß (`>=` statt `>`) |
| `starten_fassung_nicht_geprueft` | DI12 | `fassung` wird nicht auf die Zahl 1 geprüft (Stufe 8 fehlt) |
| `starten_fassung_bool_ok` | DI12 | `fassung` gleich `true` gilt als 1 (Stufe 7) |
| `starten_sitzung_ok` | DI12 | eine mitgeschickte `sitzung` wird angenommen (und überschrieben; Stufe 3 fehlt) |
| `starten_fremdfeld_ok` | DI12 | ein fremder Schlüssel im Ereignis `Starten` wird übergangen |
| `starten_datei_modus_offen` | DI12 | die `start.json` hat den Modus 0644 statt 0600 |
| `starten_ordner_modus_offen` | DI12 | der Unterordner der Sitzung hat den Modus 0755 statt 0700 |
| `starten_argv_zusatz` | DI12 | das Kind bekommt ein drittes Argument |
| `starten_env_erweitert` | DI12 | das Kind bekommt eine zusätzliche Umgebungsvariable |
| `starten_groesse_vom_kind` | DI12 | die PTY hat die Größe von `groesse` (Sitzung aus `kind`) statt `lauf.groesse` |
| `starten_cwd_ordner` | DI12 | das Arbeitsverzeichnis des Kindes ist der Unterordner der Sitzung statt `/` |
| `starten_ordner_ersetzen` | DI12 | ein schon bestehender Unterordner `s-<n>` wird gelöscht und ersetzt |
| `starten_datei_ohne_sitzung` | DI12 | der Dienst setzt `sitzung` nicht in die geschriebene `start.json` |
| `sitzung_id_wieder` | DI12 | die kleinste nicht laufende Kennung wird wieder vergeben |
| `sitzung_neunte_ok` | DI12 | eine neunte gleichzeitige Sitzung wird angenommen |
| `sitzung_achte_abgelehnt` | DI12 | schon die achte gleichzeitige Sitzung wird abgelehnt |
| `sitzung_beendete_zaehlen` | DI12 | beendete Sitzungen zählen zu den höchstens 8 |
| `sitzung_nr_null` | DI12, DI4 | alle Rasterrahmen tragen die Nummer 0 |
| `sitzung_nr_zu_gross_ok` | DI12 | die Grenze der Nummern (`nummer_max`) wird nicht geprüft |
| `starten_fehler_trotzdem_gestartet` | DI12 | nach `programm_nicht_startbar` kommt zusätzlich `Gestartet` |
| `starten_nr_verbraucht` | DI12 | ein gescheitertes `Starten` verbraucht die Nummer |
| `gestartet_an_alle` | DI12 | `Gestartet` geht an alle Betrachter statt nur an den Sender |
| `gestartet_vor_liste` | DI12 | `Gestartet` kommt vor der `Sitzungsliste` |
| `liste_nicht_nach_aenderung` | DI12 | die `Sitzungsliste` kommt nur beim Verbinden, nicht nach Start und Kindende |
| `liste_ohne_code` | DI12 | bei `beendet` fehlt der `code` |
| `liste_felder_leer` | DI12 | `harness`, `modell`, `route`, `kunde` sind in der Liste immer leer |
| `liste_nur_sender` | DI12 | nach `Starten` geht die Liste nur an den Sender |
| `liste_beim_verbinden_fehlt` | DI12 | beim Verbinden fehlt die Sitzungsliste |
| `beenden_ohne_signal` | DI12 | `Beenden` sendet kein SIGHUP |
| `beenden_kein_sigkill` | DI12 | nach `Beenden` folgt auch bei einem Kind, das SIGHUP ignoriert, kein SIGKILL |
| `beenden_entfernt_sitzung` | DI12 | eine beendete Sitzung verschwindet aus der Liste |
| `beenden_unbekannt_still` | DI12 | `Beenden` einer unbekannten Sitzung bleibt ohne Antwort |
| `beenden_beendete_ok` | DI12 | `Beenden` einer schon beendeten Sitzung bleibt ohne Antwort |
| `eingabe_an_alle_sitzungen` | DI12 | Eingaben gehen an die PTY aller Sitzungen |
| `abo_immer_erste_sitzung` | DI12 | ein `Abo` geht immer an die erste Sitzung |
| `lauf_raumwechsel_ok` | DI12 | `RaumWechsel` und `EinstellungSetzen` werden im Lauf an den Kern gegeben statt abgelehnt |
| `ende_kinder_bleiben` | DI12 | das Dienstende beendet die Kinder der Sitzungen von `Starten` nicht |
| `ende_ordner_bleiben` | DI12 | das Dienstende räumt die Unterordner der Sitzungen nicht auf |
| `lauf_kernfehler_ok` | DI12, DI9 | der Rahmen `Kernfehler` wird im Lauf angenommen (ohne Wirkung) |
| `lauf_einstellungen_nicht_pflicht` | DI1, DI12 | `lauf.einstellungen` darf fehlen (Nachtrag 20.11.2026) |
| `lauf_einstellungen_relativ_ok` | DI1, DI12 | `lauf.einstellungen` darf ein relativer Pfad sein |
| `starten_einst_nicht_gelesen` | DI12 | `Starten` liest die Einstellungsdatei nicht |
| `ablehnung_ohne_code` | DI12 | die `Ablehnung` von `Starten` trägt kein Feld `code` (Nachtrag 20.11.2026) |
| `ablehnung_ohne_text` | DI12 | die `Ablehnung` von `Starten` trägt kein Feld `text` |
| `startwahl_fehlt` | DI12 | der Dienst schickt nie eine `Startwahl` |
| `startwahl_nicht_beim_verbinden` | DI12 | die `Startwahl` kommt nicht beim Verbinden |
| `startwahl_nicht_nach_aenderung` | DI12 | eine geänderte Einstellungsdatei führt beim nächsten `Starten` zu keiner neuen `Startwahl` |
| `startwahl_nur_sender` | DI12 | die neue `Startwahl` nach einer Änderung geht nur an den Absender von `Starten` |
| `startwahl_mit_adresse` | DI12 | die Routen der `Startwahl` tragen alle Felder des Eintrags der Einstellungsdatei (Adresse, Schlüsseldatei) |
| `startwahl_ohne_ohne_kunde` | DI12 | in `kunden` fehlt `null` („ohne Kunde“) |
| `startwahl_harness_falsch` | DI12 | die Harness-Tabelle der `Startwahl` nennt für `claude` die Routenart `lokal` |
| `startwahl_trotz_unlesbar` | DI12 | bei nicht lesbarer Einstellungsdatei schickt der Dienst eine leere `Startwahl` |
| `startwahl_routen_sortiert` | DI12 | die Routen der `Startwahl` stehen nach Schlüssel sortiert statt in der Reihenfolge der Datei |
| `stufe_04_marker_ok` | DI12 | ein Schlüssel `marker` in der Anfrage wird angenommen (Stufe 4 fehlt) |
| `stufe_05_unbekannt_ok` | DI12 | ein unbekannter Schlüssel in der Start-Struktur wird angenommen (Stufe 5 fehlt) |
| `stufe_06_fehlt_ok` | DI12 | ein fehlender Pflichtschlüssel wird übergangen (Stufe 6 fehlt) |
| `stufe_07_typ_ok` | DI12 | der JSON-Typ der Werte wird nicht geprüft (Stufe 7 fehlt) |
| `stufe_09_kennung_ok` | DI12 | die Form von `harness`, `route` und `kunde` wird nicht geprüft (Stufe 9 fehlt) |
| `stufe_09_kennung_laenge_ok` | DI12 | eine Kennung darf bis 201 Zeichen lang sein statt 64 |
| `stufe_09_kennung_gross_ok` | DI12 | eine Kennung darf Großbuchstaben tragen |
| `stufe_10_modell_ok` | DI12 | die Form von `modell` wird nicht geprüft (Stufe 10 fehlt) |
| `stufe_10_modell_laenge_ok` | DI12 | `modell` darf länger als 128 Zeichen sein |
| `stufe_10_modell_leer_ok` | DI12 | ein leeres `modell` wird angenommen |
| `stufe_10_modell_zeichen_ok` | DI12 | `modell` darf ein Leerzeichen tragen |
| `stufe_11_ordner_ok` | DI12 | `arbeitsordner` wird nicht geprüft (Stufe 11 fehlt) |
| `stufe_11_ordner_relativ_ok` | DI12 | ein relativer `arbeitsordner` wird angenommen |
| `stufe_11_ordner_laenge_ok` | DI12 | ein `arbeitsordner` bis 2048 Byte wird angenommen statt bis 1024 |
| `stufe_11_ordner_punkt_ok` | DI12 | ein `arbeitsordner` mit Segment `.` oder `..` wird angenommen |
| `stufe_11_ordner_c1_ok` | DI12 | DEL und die C1-Steuerzeichen U+0080 bis U+009F im `arbeitsordner` werden angenommen |
| `stufe_11_ordner_doppel_ok` | DI12 | ein `arbeitsordner` mit `//` wird angenommen |
| `stufe_11_ordner_ende_ok` | DI12 | ein `arbeitsordner` mit `/` am Ende (auch die Wurzel `/`) wird angenommen |
| `stufe_13_harness_ok` | DI12 | ein Harness außerhalb der Tabelle wird angenommen (Stufe 13 fehlt) |
| `stufe_13_copilot_unbekannt` | DI12 | `copilot` steht nicht in der Harness-Tabelle |
| `stufe_14_route_ok` | DI12 | eine Route, die nicht in der Einstellungsdatei steht, wird angenommen (Stufe 14 fehlt) |
| `stufe_15_modell_ok` | DI12 | das `modell` der Route wird nicht gegen das der Anfrage geprüft (Stufe 15 fehlt) |
| `stufe_15_cloud_ignoriert` | DI12 | das `modell` einer Cloud-Route wird ignoriert (nur lokale Routen prüfen Stufe 15) |
| `stufe_16_kunde_ok` | DI12 | ein Kunde, der nicht in der Einstellungsdatei steht, wird angenommen (Stufe 16 fehlt) |
| `stufe_16_null_unbekannt` | DI12 | `kunde` gleich `null` gilt als unbekannter Kunde |
| `stufe_17_art_ok` | DI12 | die Art der Route wird nicht gegen den Harness geprüft (Stufe 17 fehlt) |
| `stufe_17_copilot_lokal` | DI12 | `copilot` gilt als Harness für lokale Routen |
| `stufen_hoechste_zuerst` | DI12 | bei mehreren Verletzungen antwortet die höchste Stufe statt der niedrigsten |
| `stufen_14_bis_17_umgekehrt` | DI12 | die Stufen 14 bis 17 werden in der Reihenfolge 17, 16, 15, 14 geprüft |
| `stufen_14_bis_17_fehlen` | DI12 | die Stufen 14 bis 17 werden nicht geprüft |
| `einst_vor_form` | DI12 | die Einstellungsdatei wird vor der Form der Start-Struktur gelesen (bei beidem falsch antwortet `einst_unlesbar`) |
| `startwahl_ohne_modell` | DI12 | die Routen der `Startwahl` tragen kein Feld `modell` |
| `startwahl_modell_leer` | DI12 | `modell` ist `""` statt `null`, wenn die Route keines trägt |
| `startwahl_modell_ignoriert` | DI12 | `modell` der `Startwahl` ist immer `null` |
| `startwahl_modell_fassung1_erfunden` | DI12 | bei Einstellungen der Fassung 1 steht `modell` `auto` statt `null` |
| `startwahl_ohne_copilot` | DI12 | die Harness-Tabelle der `Startwahl` nennt `copilot` nicht |
| `start_grund_code` | DI12 | `grund` einer Ablehnung der Prüfstelle ist der Code der Stufe statt `start_pruefung` |
| `start_code_ohne_praefix` | DI12 | `code` trägt die Stufe ohne das Präfix `start.` |
| `start_text_form` | DI12 | der Text der Ablehnung der Prüfstelle ist immer der der Form (`Start hat falsche Form`) |
| `ende_nur_bei_normalem_ende` | DI12 | das Sitzungsende räumt nur bei Code 0 auf (ein gescheiterter Start lässt den Ordner stehen) (Nachtrag 21.11.2026) |
| `ende_sitzung_ohne_loeschen` | DI12 | das Kindende löscht den Sitzungsordner nicht (erst das Dienstende) (Nachtrag 21.11.2026) |
| `ende_loeschen_folgt_link` | DI12 | das Löschen folgt einem Link im Sitzungsordner und löscht sein Ziel (Nachtrag 21.11.2026) |
| `ende_signal_ohne_leiter` | DI12 | das Signal geht auch an einen Prozess, der kein Sitzungsleiter ist (Gruppenleiter der Sitzung des Kindes) (Nachtrag 21.11.2026) |
| `ende_signal_ohne_startzeit` | DI12 | das Signal geht auch an einen Prozess, der vor dem Kind gestartet wurde (Nachtrag 21.11.2026) |
| `ende_signal_nach_loeschen` | DI12 | die Reihenfolge ist vertauscht: erst löschen, dann signalisieren (die PID-Dateien sind weg) (Nachtrag 21.11.2026) |
| `ende_signal_link_folgen` | DI12 | eine PID-Datei, die ein Link ist, wird gelesen und ihre PID signalisiert (Nachtrag 21.11.2026) |
| `ende_signal_ohne_zeilenende` | DI12 | die PID-Datei darf Leerzeichen, fehlenden oder zweiten Zeilenumbruch haben (`int(text.strip())`) (Nachtrag 21.11.2026) |
| `ende_dienstende_ohne_signal` | DI12 | das Dienstende löscht die Ordner, schickt aber kein Signal (Nachtrag 21.11.2026) |
| `ende_signal_keine` | DI12 | das Sitzungsende schickt nie ein Signal (nur Löschen) (Nachtrag 21.11.2026) |
| `starten_datei_name` | DI12 | die Datei der Sitzung heißt `start.txt` statt `start.json` (Nachtrag 21.11.2026, Auflage) |
| `sitzungen_pruefung_fehlt` | DI12 | der Dienst prüft `lauf.sitzungen` nicht und vergibt `s-1`, auch wenn der Eintrag besteht (Nachtrag 21.11.2026) |
| `sitzungen_stat_statt_lstat` | DI12 | `stat` statt `lstat`: eine tote Verknüpfung gilt als frei (Nachtrag 21.11.2026) |
| `sitzungen_nur_ordner` | DI12 | nur ein Ordner belegt die Nummer, eine Datei oder Verknüpfung nicht (Nachtrag 21.11.2026) |
| `sitzungen_fehler_gilt_frei` | DI12 | jeder Fehler des `lstat` gilt als frei, auch „keine Rechte“ (statt `start_ordner`) (Nachtrag 21.11.2026) |
| `sitzungen_socket_ordner_geprueft` | DI12 | geprüft wird `socket.ordner` statt `lauf.sitzungen` (Nachtrag 21.11.2026) |
| `sitzungen_nummer_max_nicht_beim_ueberspringen` | DI12 | `nummer_max` gilt nur für die erste geprüfte Nummer, nicht für übersprungene (Nachtrag 21.11.2026) |
| `sitzungen_relativ_ok` | DI12 | ein relativer Pfad in `lauf.sitzungen` wird angenommen (Nachtrag 21.11.2026) |
| `sitzungen_naechste_beim_ueberspringen` | DI12 | schon das Überspringen setzt die Zahl der vergebenen Nummern (`dienst.gestartet`), auch wenn der Start danach scheitert (Nachtrag 21.11.2026) |
| `marker_ordner_ungeprueft` | DI12 | der Elternordner der Marker-Datei wird auf Eigentümer und Modus nicht geprüft (Nachtrag 21.11.2026) |
| `marker_ordner_nofollow_fehlt` | DI12 | der Elternordner wird auch als Verknüpfung geöffnet (ohne `O_NOFOLLOW`) (Nachtrag 21.11.2026) |
| `marker_datei_nofollow_fehlt` | DI12 | die Marker-Datei wird auch als Verknüpfung geöffnet (ohne `O_NOFOLLOW`) (Nachtrag 21.11.2026) |
| `marker_modus_ungeprueft` | DI12 | der Modus der Marker-Datei wird nicht geprüft (Nachtrag 21.11.2026) |
| `marker_groesse_ungeprueft` | DI12 | die Größe der Marker-Datei wird nicht begrenzt (Nachtrag 21.11.2026) |
| `marker_fehlt_ist_fehler` | DI12 | eine fehlende Marker-Datei ist ein Fehler (Exitcode 2) statt keine Marker (Nachtrag 21.11.2026) |
| `marker_je_start_neu_gelesen` | DI12 | die Marker-Datei wird bei jedem `Starten` neu gelesen statt einmal beim Start (Nachtrag 21.11.2026) |
| `marker_nie_gesetzt` | DI12 | der Dienst schreibt `marker` nie in die `start.json`, auch mit Markern (Nachtrag 21.11.2026) |
| `marker_nur_erste_sitzung` | DI12 | nur die erste Sitzung bekommt `marker`, die späteren nicht (Nachtrag 21.11.2026) |
| `marker_ohne_datei` | DI12 | ohne Marker-Datei schreibt der Dienst trotzdem ein `marker` in die `start.json` (Nachtrag 21.11.2026) |
| `marker_anfrage_ersetzt` | DI12 | ein `marker` der Anfrage wird angenommen und ersetzt den des Dienstes (Nachtrag 21.11.2026) |
| `marker_form_uebersprungen` | DI12 | die Form der Marker-Datei und des Zählordners wird nicht geprüft, jedes Objekt gilt (Nachtrag 21.11.2026) |
| `marker_siebzehn_ok` | DI12 | die Liste der Marker darf 17 Einträge haben (Nachtrag 21.11.2026) |
| `marker_komma_ok` | DI12 | ein Marker darf ein Komma enthalten (Nachtrag 21.11.2026) |
| `marker_liste_kein_array_ok` | DI12 | ein `liste`, das kein Array ist (ein Text oder eine Zahl), wird als Liste mit einem Eintrag angenommen (Nachtrag 21.11.2026) |
| `marker_liste_leer_ok` | DI12 | eine leere Liste wird angenommen (Nachtrag 21.11.2026) |
| `ziellisten_pruefung_fehlt` | DI12 | der Dienst prüft `lauf.ziellisten` nicht und vergibt `s-1`, auch wenn die Zielliste liegt (Nachtrag 21.11.2026) |
| `ziellisten_stat_statt_lstat` | DI12 | `stat` statt `lstat` bei der Zielliste: ein toter Verweis gilt als frei (Nachtrag 21.11.2026) |
| `ziellisten_falscher_name` | DI12 | geprüft wird `s-<n>` statt `s-<n>.zielliste.json` (Nachtrag 21.11.2026) |
| `ziellisten_fehler_gilt_frei` | DI12 | jeder Fehler des `lstat` der Zielliste gilt als frei, auch „keine Rechte“ (statt `start_ordner`) (Nachtrag 21.11.2026) |
| `ziellisten_nur_regulaer` | DI12 | nur eine reguläre Datei belegt die Nummer, ein Verweis oder Ordner nicht (Nachtrag 21.11.2026) |
| `ziellisten_ohne_pfadpruefung` | DI12 | `lauf.ziellisten` wird ohne Pfadprüfung angenommen (relativ, leer, kein Text) (Nachtrag 21.11.2026) |
| `laufzeit_nicht_geprueft` | DI12 | der Socket-Ordner unter `lauf.laufzeit` belegt die Nummer nicht (Nachtrag 2, Projektentscheid, 21.11.2026) |
| `zaehldatei_nur_suche` | DI12 | nur `s-<n>.suche.json` belegt die Nummer, `s-<n>.modellweg.json` nicht (Nachtrag 2, Projektentscheid, 21.11.2026) |
| `zaehldatei_ohne_marker_ordner` | DI12 | die Zähldateien werden in einem falschen Ordner gesucht (in `lauf.sitzungen` oder `lauf.ziellisten`) statt in `lauf.marker_ordner` (Nachtrag 2, Projektentscheid, 21.11.2026) |
| `eintrag_fehlt_ist_frei` | DI12 | die Nummer gilt schon als frei, wenn ein Eintrag fehlt (UND und ODER vertauscht) statt erst, wenn alle fehlen (Nachtrag 2, Projektentscheid, 21.11.2026) |
| `gespraeche_vergabe_ungeprueft` | DI12 | der Dienst prüft `lauf.gespraeche` bei der Vergabe nicht und vergibt `s-1`, auch wenn das Gespräch liegt (Nachtrag 27.11.2026) |
| `laufzeit_ohne_pfadpruefung` | DI12 | `lauf.laufzeit` wird ohne Pfadprüfung angenommen (Nachtrag 2, Projektentscheid, 21.11.2026) |
| `marker_ordner_aus_datei` | DI12 | der Zählordner darf aus der Marker-Datei kommen (Schlüssel `ordner` erlaubt und genutzt) statt aus `lauf.marker_ordner` (Nachtrag 21.11.2026, Auflage) |
| `marker_nonblock_fehlt` | DI12 | die Marker-Datei wird ohne `O_NONBLOCK` geöffnet: eine FIFO an ihrer Stelle hält den Start des Dienstes an (der Fall endet rot durch die Zeitgrenze) (Nachtrag 21.11.2026, Nachtrag 3) |
| `marker_ordner_fehlt_uebergangen` | DI12 | ein fehlendes `lauf.marker_ordner` bei gültiger Liste wird übergangen (keine Marker, kein Fehler) (Nachtrag 21.11.2026, Auflage) |

**Gegenprobe der Grenze hinaus (Nachtrag 23.11.2026):** `raster_ohne_grenze_hinaus`, Regel DI3: `Raster::nimm` (`rust/crates/dienst/src/betrachter.rs`) reiht einen Rasterrahmen über `RAHMEN_MAX` ein, vor und nach Deflate. Er steht nicht in der Tabelle oben: `gegenprobe.sh` verlangt für jeden Mutanten der Tabelle einen roten Außenfall, und kein Außenfall erzeugt einen Rasterrahmen dieser Größe (hergeleitet). Die Gegenprobe läuft im Rust-Bau: Beide Prüfungen in `Raster::nimm` aus, dann ist `ein_rasterrahmen_ueber_rahmen_max_ist_rahmen_zu_gross` rot (gemessen am 23.11.2026 mit beiden Prüfungen ausgeschaltet).

**Mutanten der Sicherheitszüge (Nachtrag 25.11.2026):** `cargo mutants` 27.1.0 mit `-p dienst --all-features`, die volle Suite des Crates je Mutant, begrenzt auf die Funktionen der drei Züge vom 23.11.2026. Die Läufe unter Linux liefen in einem Einschluss (eigene PID-Sicht, nur der Wegwerfordner schreibbar), weil Mutanten von Lösch- und Signalcode sonst fremde Prozesse treffen könnten.

- Helfer-Identität (`rust/crates/dienst/src/aufraeumen.rs`, ganze Datei, Linux): 71 Mutanten, 68 gefangen, 3 nicht übersetzbar (`Some(Default::default())` für Typen ohne `Default`), keiner überlebt.
- Startfehler, Subreaper und Socketmodus (`waisen.rs`: `subreaper_werden`, `kinder_lesbar`, `ist_subreaper`, `kinder_verwerfen`, `start_verwerfen`; `dienst.rs` die Prüfung `kinder_lesbar` in `Dienst::neu_mit`; `meldung.rs` `modus_setzen`; `rechte.rs` `socket_modus_setzen`), Linux: 33 Mutanten, im ersten Lauf 21 gefangen und 12 überlebt. Drei Überlebende berührten eine Zusage, für sie gibt es neue Fälle (Gegenprobe je rot): `OFlags::NOFOLLOW | OFlags::CLOEXEC` zu `&` in `modus_setzen` und `socket_modus_setzen` lässt wegen der Rangfolge von `&` vor `|` das `O_NOFOLLOW` weg, das Öffnen folgt einem Link und ändert den Modus eines anderen Sockets (Fälle `ein_link_auf_einen_anderen_socket_aendert_dessen_modus_nicht` in `meldung.rs` und `rechte.rs`); `kinder_lesbar` gibt immer `true` (Fall `tests/kinder_lesbar.rs`: der Dienst als pid 1 eines eigenen Nutzer-, Mount- und PID-Namensraums, ein `tmpfs` über `/proc/1/task` verdeckt die `children`-Datei, der Start endet mit 1 und dem festen Text; ohne `tmpfs` endet derselbe Aufruf mit 4 am fehlenden Kind). Im zweiten Lauf 24 gefangen, 9 überlebt. Die 9 sind gleichwertig oder unter Linux nicht übersetzt: `|` zu `^` zwischen getrennten Bits (`OFlags` in Zeile 54 und 192, `Mode::RUSR | Mode::WUSR`) gibt denselben Wert; `&&` zu `||` in der Prüfung nach dem Setzen (`meldung.rs:80`, `rechte.rs:218`) ändert unter Linux nichts, weil der Deskriptor vorher schon als Socket geprüft und der Modus über ihn gesetzt ist (nur ein Tausch zwischen Setzen und Prüfung könnte es zeigen); `rechte.rs:211` steht im Zweig außerhalb von Linux.
- macOS-Peer (`meldung.rs`, `Meldung::gegenseite_gut`, macOS) und die macOS-Zweige von `modus_setzen` und `socket_modus_setzen`: 17 Mutanten, 15 gefangen, 2 überlebt, beide gleichwertig (`Mode::RUSR | Mode::WUSR` zu `^`). Für den Arm `ENOTCONN` erzeugt `cargo mutants` keinen Mutanten; ihn deckt die Gegenprobe „ENOTCONN streng“ vom 23.11.2026.

**Mutanten mit dem Prüfbau (Nachtrag 25.11.2026):** Der zweite Lauf von `rust/gates.sh` (`cargo mutants -p dienst --features pruefbau --no-shuffle`, `cargo-mutants` 27.1.0, unter Linux im Einschluss) ergab über das ganze Crate 1064 Mutanten, davon 965 gefangen, 75 nicht übersetzbar, 7 im Zeitlimit (jede eine Endlosschleife: `annahme` nimmt jeden Fehler als „nochmal“, `abgenommen` leert die Schlange nie, oder die Jagd in `Waisen::alle_beenden` endet nie; der zugehörige Einheitstest war jeweils schon rot) und 17 überlebt; vier davon in `betrachter.rs` (die Grenze `RAHMEN_MAX` in `Raster::nimm` vor und nach Deflate, `Raster::vergessen`) fangen jetzt die Fälle `ein_rasterrahmen_von_genau_rahmen_max_geht_hinaus` und `nach_vergessen_hat_die_nummer_kein_vorbild`, neun sind die oben genannten gleichwertigen Mutanten von `modus_setzen` und `socket_modus_setzen`, zwei sind gleichwertig, weil die Schlange `arbeit` am Ende jeder Runde leer ist (`entfernte_wegnehmen`) und weil das Ergebnis „keine Nummer frei“ von `rahmennummer` nicht von der Kennung abhängt (`starten`), und zwei stehen im macOS-Zweig von `Meldung::gegenseite_gut` und werden unter Linux nicht übersetzt.

**Mutanten der Änderungen seither (Nachtrag 25.11.2026):** Derselbe zweite Lauf mit den Ausnahmen `gleichwertig` aus `rust/gates.sh`, begrenzt mit `--in-diff` auf die Änderungen in `rust/crates/dienst/src` seit dem Lauf darüber, ergab 127 Mutanten, davon 72 gefangen, 4 nicht übersetzbar, 1 im Zeitlimit (eine Endlosschleife in `leer`, der Einheitstest war schon rot) und 50 überlebt; einer davon (`socket_lesen` weist unter Linux einen `socket.name` mit `\` oder `:` ab, gegen Abschnitt 21 Punkt 10) fängt jetzt der Fall `socketname_mit_backslash_oder_doppelpunkt_nur_unter_windows_ungueltig`, vier in `json.rs` zeigten toten Code (den Wächter auf den Rekursionsfehler in `ereignis_lesen` und den Zweig für das leere Objekt in `objekt_je_glied`: `objekt_je_glied` liest dieselbe Syntax wie `serde_json`, und eine leere Karte ohne `art` ergibt nie ein `Starten`), der jetzt fehlt, mit dem Fall `flacher_fehler_mit_art_starten_und_leeres_objekt`, und 45 stehen in Code hinter `cfg(windows)`, den Linux nicht übersetzt; der Lauf über `startdatei.rs` und `json.rs` danach ergab 216 Mutanten, 208 gefangen, 7 nicht übersetzbar, 1 im Zeitlimit (`leer`) und keinen Überlebenden.

## 10. Offene Punkte

- **Kernelpuffer des Unix-Sockets (Risiko 7).** `ausstehend` zählt den Sendepuffer des Dienstes. Am Mac hat der Kernelpuffer 8192 Byte (`sysctl net.local.stream.sendspace`, `EN`), auf Linux rund 212 992. Wie sich das zur Grenze 16 384 verhält und ob die Meldung reicht, **misst ein späterer Schritt** (P5, Läufer ohne Fenster); der Vertrag prüft den Mechanismus nur mit `Stumm`, das den Kernelpuffer als 0 Byte nimmt.
- **Cursor: entschieden und eingearbeitet** (Nachtrag 20.11.2026, Projektentscheid, 19.11.2026 F2, Cursor-Nachtrag): DI11 und DI6 Punkt 1a. Der Träger meldet den Cursor in `Ausgabe`, der Dienst gibt `[0, 0, true]` als Anfangscursor und beim Neubau den des Trägers mit; die Außentests vergleichen ihn.
- **Stand der Kopie von `zk_ref.py`: erneuert** (Nachtrag 20.11.2026): Die Kopie in `referenz/` ist die Fassung `57ac460e…` (mit dem Mutanten `cursor_umgeht_deckel`), die heutige Referenz des Vertrags `zustandskern`. Bei einer Änderung der Regeln ist die Kopie wieder zu erneuern und `faelle_bauen.py` neu zu fahren.
- **Neubau und Synchronisierung (DEC 2026): entschieden** (Projektentscheid, 20.11.2026): Der Cursor des Neubaus ist der des Trägers, dieselbe Quelle wie `zeilen` und `stand` (DI6 Punkt 1a), Fall `di6_kernfehler_sync`.
- **Kleinste Größe in den Verträgen des Kerns, des Trägers und des Rasterauszugs** steht noch auf 1 Spalte; der Dienst lehnt eine Spalte selbst ab (DI3). Die Doppelprüfung im Dienst **bleibt** (Projektentscheid, 20.11.2026), auch nach dem Nachtrag der drei Verträge (die Ablehnung des Kerns trägt denselben Grund).
- **Peer-Prüfung** (Risiko 6) fehlt: `std` hat keinen Aufruf dafür, `unsafe` ist verboten. `wb-dienst` bleibt ein Prüfprogramm.
- **Effektive Benutzerkennung: entschieden** (Nachtrag 20.11.2026, Projektentscheid, 19.11.2026 F5): `rustix` 1.1.6 direkt mit dem Merkmal `process`, siehe Abschnitt 11.
- **Harte Obergrenze der Warteschlange: entschieden und eingearbeitet** (Nachtrag 20.11.2026, Projektentscheid, 19.11.2026 F4): DI5. Trennen ohne `Fehler`-Rahmen (die Schlange ist voll), Zeile im Protokoll. Offen bleibt die Messung P5 (Kernelpuffer, Risiko 7), sie ändert die Grenze nicht.
- **Fäden.** Ein Bau mit Fäden braucht eine Reihenfolge der Ereignisse (ein Kanal zum Kernfaden); die Fälle bestimmen sie über die Schranke `Zustand`.
- **`protokoll`, `an_stelle`, `abfragen`** des Kerns gibt der Dienst nicht weiter (Gateway, Stelle und Abfrage sind nicht im Umfang).
- **Träger in der Referenz.** Die Referenz benutzt ein kleines Modell statt `alacritty_terminal` (Abschnitt 7 DI8). Die Stichprobe gegen die Rust-Referenz des Trägers steht im Messbericht.
- **Plattform.** Der Fall `fremder Besitzer` braucht einen Ordner mit 0700 eines anderen Benutzers (`/var/audit` am Mac, `/var/lib/private` auf Linux; die Liste steht im Fall) und läuft nicht als `root`. Der Prüfclient braucht `lsof`. Windows ist nicht Teil.

## 11. Pflichten des Baus außerhalb der Außentests

- README des Crates: `wb-dienst` ist ein Programm des Durchstichs (Abschnitt 1); im Probelauf ein Benutzerdienst.
- **Benutzerdienst (Projektentscheid, 20.11.2026):** `wb-dienst` läuft als Dienst des Benutzers (`systemd --user`), nie als root; **kein `unsafe`, kein Root, keine Fähigkeiten (`capabilities`), kein Netzwerk-Socket** (`AF_UNIX` genügt). Die Einheit des Pakets setzt `RuntimeDirectory=werkbank` mit `RuntimeDirectoryMode=0700` und `NoNewPrivileges=yes`.
  Die **Startdatei** ist Installationskonfiguration: Das Paket legt sie an (nicht die Ansicht) und trägt `lauf.programm`, `lauf.einstellungen`, `socket.ordner` ein (die Werte sind Sache des Pakets); der Dienst liest diese Pfade aus keiner anderen Quelle.
- Kein `unsafe`, keine Panik auf Eingaben, `cargo deny` nach `rust/deny.toml`, keine neue Abhängigkeit außer `rustix` für `geteuid`.
- **`geteuid` über `rustix` (Nachtrag 20.11.2026, Projektentscheid, 19.11.2026 F5):** `rustix` **1.1.6** als **direkte** Abhängigkeit des Crates `dienst`, mit dem Merkmal `process` (schon in der `Cargo.lock`; kein `unsafe` im eigenen Code). Der **Grund steht in `Cargo.toml`** (am Eintrag: `std` hat keinen Aufruf für die effektive Benutzerkennung, DI2 Punkt 2 braucht sie, `unsafe` ist verboten); die **Stückliste** des Produkts und die **Vermerke** (Hinweise zu Lizenzen und Abhängigkeiten) werden vom Bau nachgezogen. `deny.toml` bleibt gültig, wenn `cargo deny` mit dem neuen Eintrag grün bleibt. Die Frage (Crate oder andere Prüfung) ist damit entschieden.
- Obergrenze der Warteschlange (DI5, Nachtrag 20.11.2026): die Außenfälle `di5_grenze_*` bestehen; zusätzlich ein Test des Baus, dass ein Betrachter, der nie liest, bei Dauerlast getrennt wird und die Zeile ins Protokoll kommt.
- Test der Grenze der Nachrichten je Schritt (33 554 432 Byte): Der Fehler führt zu DI6 (die Außentests lösen DI6 nur über `Kernfehler` aus).
- Messung P4 (Auszug im Dienst: B2, V1, C1 gegen den Dienst) und P5 (Rückstau mit dem echten Kernelpuffer, zwei Betrachter) als Läufer auf einer ruhigen Messmaschine, nicht in `gates.sh test`.
- Ein Test, dass mehrere Fäden dieselbe Folge geben wie die Referenz (Bau mit Fäden).
- **DI12:** der Aufruf der Prüfstelle `start` (Crate `start`: `lesen_anfrage`, `pruefen`) vor dem Start des Kindes an der Stelle von DI12 Punkt 3.1 und 3.2, mit dem Code und dem Text unverändert in der `Ablehnung`; **der Neubau des Kerns je Sitzung** nach DI6 (ein Fehler eines Kerns baut nur diesen Kern neu und
  gibt jedem Betrachter mit Abo auf diese Sitzung einen vollen Abzug); ein Test, dass `SIGKILL` nach `Beenden` auch einen Kindprozess trifft, der `SIGHUP` ignoriert; ein Test der Nummern und Kennungen (nie wiederverwendet, Grenze `nummer_max`); die Unterordner der Sitzungen werden beim Dienstende
  aufgeräumt, auch nach einem Absturz einer Sitzung; der Dienst bleibt ohne TCP (kein Hörer, auch nicht für die Brücke).

## 12. Dateien

- `referenz/dienst_ref.py` (ausführbar, `#!/usr/bin/python3 -I`; das ist `DI_BIN` der Referenz), `referenz/zk_ref.py` und `referenz/ra_ref.py` (unveränderte Kopien der Referenzen der Verträge `zustandskern` und `rasterauszug`; `sha256` stehen im Bericht).
  Die Referenz führt wirklich aus: Socket, PTY, Kind, Uhr, Rahmung, Deflate, Rechte. Sie übernimmt: die Logik des Kerns (`zk_ref.Kern`), den Auszug und den Dekoder (`ra_ref.Verbindung`), die Zeilenform (`ra_ref`). Sie ersetzt: den Träger durch ein Modell der festen Kindprogramme.
- `pruefclient.py`, `test_aussen.py` (Läufer mit `DI_BIN` und `DI_FAELLE`), `faelle_bauen.py` (alle Erwartungen aus der Referenz; jeder Fall läuft zweimal, die Ergebnisse müssen gleich sein), `gegenprobe.sh`.
- `aussentests/{faelle,erwartet}/` (sichtbar, Datensatz A) und `verdeckt/{faelle,erwartet}/` (Datensatz B: andere Namen, Räume, Größen, Texte, Zahlen, Grenzen). Die Fälle `di12_*` gehören zu DI12 (Nachtrag 20.11.2026).
- `einstellungen/vorfuehrung.json` (Nachtrag 20.11.2026): Kopie der Vorführ-Einstellungsdatei aus the maintainers Vertrag 2; die Fälle mit `lauf` nennen sie über den Platzhalter `@einstellungen`.

## Projektentscheide nach der Abnahme von DI12 (20.11.2026)

Abgenommen nach eigener Prüfung: 136 sichtbare und 111 verdeckte Fälle, 0 gleiche
Hashes, die 194 und 150 alten Dateien Byte für Byte gleich, 145 Mutanten in einem Lauf mit 0 grün geblieben.

1. **Prüfstelle `start`:** Die Referenz prüft nur die Form (`start_form`). Die volle Prüfung kommt aus der Crate `start` im Bau. Das Projekt baut sie
   nicht als Referenz nach. Die Fälle mit `start.*`-Codes kommen als kleiner Nachtrag, sobald das Crate gemergt ist.
2. **Mehrere Sitzungen:** Die Regeln aus DI12 gelten: Nummer 0 für `kind`, n für `s-<n>`, höchstens 255 Nummern je Lauf, ein Raumkern, `RaumWechsel` und
   `EinstellungSetzen` im Lauf abgelehnt, die Liste vor `Gestartet`, beendete Sitzungen in der Liste ohne Zählung zu den 8, die Einstellungsdatei bei
   jedem `Starten` gelesen.
3. **`Startwahl`:** Zeitpunkt und Form gelten (beim Verbinden und bei geändertem Inhalt). Der Vertrag `seite` nimmt `Startwahl` so (dort Abschnitt 8,
   Entscheidung 8). Der Seiten-Bau richtet die Wahlfelder danach aus.
4. **`grund` und `code`:** `code` ist mit dem Crate `start` der Code `start.<code>`, `grund` bleibt das kurze Wort.
5. **Unstabiler alter Fall `di11_cursor_groesse` (verdeckt):** Er bleibt unverändert, bis ein eigener kleiner Nachtrag eine Wartebedingung ergänzt.
   Bei der Abnahme eines Baus gilt bis dahin: Ein einzelnes Rot dieses Falls unter Last wird einmal ohne Last wiederholt und mit beiden Läufen
   gemeldet.

## Projektentscheide zur Prüfung über das Crate `start` (21.11.2026)

1. **`copilot` in der Harness-Tabelle der `Startwahl`:** gilt. Vertrag 1 führt `copilot`, und die Vorführung läuft damit.
2. **`einst_unlesbar`** bleibt der eine feste Grund für jede Einstellungsdatei, die das Crate `einstellungen` ablehnt. Die Seite braucht den Code der
   Stufe nicht.
3. **Obergrenze der Einstellungsdatei** ist 65 536 Byte, wie Stufe 1 von Vertrag 2.
4. **Bekannte Zeitannahmen** in alten Fällen (`di11_cursor_groesse`, `di7_monoton_buendel`, Wartezeit des Kindprogramms `lauf`) werden in einem eigenen
   Schritt zeitunabhängig.

## Projektentscheide zum Sitzungsende (21.11.2026)

1. **Sitzungsende räumt auf:** DI12 Punkt 12. Der Dienst räumt an einer Stelle auf (Kindende vor der Liste, Dienstende nach dem Ende der Kinder): erst SIGTERM an die Gruppe der PID aus `suche.pid` und `modellweg.pid` (nur Linux, nur mit dem PID-Schutz), dann Löschen ohne Verknüpfung zu folgen. Unter macOS nur die Dateien; das Signal ist Schritt II und **offen**.
2. **Plattform der Fälle:** Eine Erwartung (Linux). Ohne `/proc` lässt der Prüfclient die Marker `*.term` aus dem Ergebnis weg; die Fälle prüfen dann nur die Dateien.
3. **Geänderte Erwartungen:** keine Datei der 578 alten Fälle- und Erwartungsdateien (157 und 132 Fälle, je mit Erwartung) hat sich geändert (`cmp`: 578 verglichen, 0 abweichend). Im Rust-Test `tests/mehrere.rs` (`ordner_der_sitzungen_rechte_inhalt_und_aufraeumen_auch_nach_dem_absturz`) stand die Erwartung „der Ordner einer beendeten Sitzung bleibt bis zum Dienstende“; sie lautet jetzt „der Ordner ist nach dem Ende der Sitzung weg, der Ordner der laufenden Sitzung bleibt“. Grund: Punkt 12.

### Weggefallene Prüfung

Zuvor trug der Fall `di12_starten_grenze` (sichtbar und verdeckt) mit seiner Eingabe von 4096 Byte die Zeilen `MODI=0600/0700` und `NAME=s-1/start.json`; seit dem Crate `start` lehnt der Dienst diese Eingabe ab (Stufe 5, `start.schluessel_unbekannt`), so dass die beiden Zeilen nur noch in `di12_starten_bericht` standen. Der neue Fall `di12_starten_groesste` (sichtbar und verdeckt) trägt sie wieder mit der größten Start-Struktur, die das Crate `start` annimmt (Route und Kunde mit 64 Zeichen, `modell` mit 128 Zeichen, `arbeitsordner` mit 1024 Byte, höchstens 4096 Byte kanonisch): die Sitzung startet, das Kind meldet Inhalt, Modus und Name der Datei. Rot machen ihn die Mutanten `starten_datei_modus_offen` und `starten_ordner_modus_offen` (falscher Modus) und `starten_datei_name` (falscher Name).

## 15. Nachtrag Zeitannahmen (21.11.2026): Fälle ohne Zeitannahme

Jeder Fall war unter Last mindestens einmal rot, weil er auf eine Zeit statt auf einen Zustand baute. Regel: Ein Fall wartet auf einen Zustand; eine Frist ist nur die Obergrenze dieses Wartens. Geändert (je Satz sichtbar und verdeckt; die Erwartungen werden von der Referenz neu gebaut):

1. `di11_cursor_groesse`: Der Fall wartet auf das Kindende, bevor er `Groesse` sendet. Vorher kam `Kindende` oder die Antwort auf `Groesse` zuerst, je nach Zeitpunkt (das Kind endet sofort nach seiner Ausgabe). Die Erwartung nennt `Kindende` schon immer vor `Groesse`; sie ändert sich nur um den zusätzlichen Schritt.
2. `di7_monoton_buendel`: echte Uhr bleibt (nur sie fängt den Mutanten `Dienst::bis_frist -> None`). Der Fall setzt die Bildrate auf 1 je Sekunde (Bündelungsabstand 1 s statt 16 ms) und sendet das zweite Stück erst, wenn das erste auf dem Bild steht (Kind `stufen`, ein Byte vom Client je Stück). So liegt das zweite Stück immer in der Bündelungsfrist des ersten und kommt nur durch den Zeitgeber des Dienstes an; vorher trennten 3 ms im Kind beide Stücke, und unter Last lag das zweite hinter der Frist.
3. `di5_grenze_genau`, `di5_grenze_drueber` und die Kalibrierung dazu: `fuellen @<hex>` schreibt den Titel nach dem Startbyte statt nach 300 ms vor `BEREIT`. Die Obergrenze wird aus derselben Folge neu eingemessen (`warteschlange_max` ist jetzt 407 und 406 statt 240 und 239 in A; der Titel steht jetzt auch als Sitzungsliste in der Warteschlange).
4. `di12_sitzungen_acht`, `di12_starten_grenze`, `di12_starten_stufen_gueltig`, `di12_nummern_erschoepft`: Das Kind `lauf` wartet **nicht mehr** 1 s vor `BEREIT`. Die Fälle erwarteten stillschweigend, dass die Kinder noch nichts gezeigt hatten. Jetzt wartet der Fall je Sitzung auf `BEREIT` (Abo, dann `{"text": "BEREIT", "nr": k}`), und die Erwartung zeigt den echten Zustand (Cursor 1, 0 und `BEREIT`).
5. `di5_grenze_betrieb`: Der Fall bleibt in Menge und Inhalt gleich (die Grenze 33 554 432 Byte lässt sich nur mit dieser Menge erreichen). Neu ist `limit` 400 für den Fall und 300, 200 und 60 s als Obergrenzen der drei Wartestellen (vorher 60, 40, 10 s).
6. `di12_startwahl_modell_aenderung` (verdeckt): Das Starten des Satzes B war abgelehnt (das Modell der lokalen Route war geändert), und der Fall wartete zweimal 30 s auf ein `Gestartet`, das nie kam (die Erwartung enthielt zwei `zeit`). Das Starten nennt jetzt das neue Modell und wird angenommen.

Neu im Läufer: das Fallfeld `limit` (Abschnitt 6).

Nachtrag 2 (21.11.2026, Nachtrag 9 zu den Zeitannahmen): Dieselbe Regel gilt für die Fälle der Kennung, der Ziellisten und der Nummern aus der Arbeitskopie.
Jeder Fall `di12_kennung_*`, `di12_zielliste_*` und `di12_nummer_*`, der nach einem Start den Zustand `voll` liest, abonniert die Sitzung und wartet, bis ihr Bild `BEREIT` zeigt (`starte_bereit`), bevor er den Zustand liest. Vorher erwartete er in der ersten Zeile des Trägers `""`; ob das Kind `BEREIT` schon geschrieben hatte, hing von seinem Tempo ab (Torlauf auf dem Linux-Baurechner: `"BEREIT" statt ""` in `di12_kennung_luecke` und `di12_kennung_zwei_belegt`). `voll` bleibt, die Regeln von DI12 (Punkte 13 bis 15) werden wie vorher geprüft; in den Erwartungen steht jetzt `BEREIT` in der Trägerzeile der gestarteten Sitzung. `di12_nummern_im_rahmen` wartet zusätzlich auf das Ende der Sitzung aus `kind` (die Liste mit `code` 0), die er stillschweigend annahm. 30 Fälle und Erwartungen je Satz geändert, keiner kam hinzu oder fiel weg.

## Projektentscheide zur Kennung nach einem Neustart (21.11.2026)

Ordner, die nach einem Absturz oder Stromausfall unter `lauf.sitzungen` liegen bleiben, belegen ihre Nummer, bis sie jemand entfernt; je Lauf des Dienstes gibt es
höchstens 255 Nummern. Das Aufräumen solcher Rückstände beim Dienststart (mit PID-Schutz) ist ein offener Punkt für Schritt II.

1. **Regel:** DI12 Punkt 13. `lauf.sitzungen` ist optional; die Prüfung ist ein `lstat` je Nummer, jeder Eintrag belegt, ein anderer Fehler als „fehlt“ ist `start_ordner`.
2. **Gemessen (Referenz):** Ein fehlender Ordner `lauf.sitzungen` ist frei (der `lstat` meldet `NotFound`), auch wenn der Ordner selbst fehlt. Ein Modus 0000 des Ordners liefert beim `lstat` eines Eintrags `EACCES`, nur für einen Lauf, der nicht Superuser ist.
3. **Zahl der Nummern:** Übersprungene Nummern zählen zur Grenze: Bei `nummer_max` 2 mit `s-1` und `s-2` belegt ist die Antwort `nummern_erschoepft`.
4. **`dienst.gestartet`** (die Zahl der vergebenen Nummern, `naechste`) bleibt beim Überspringen unverändert; erst ein gelungener Start setzt sie.

## Projektentscheide zu den Markern des Probelaufs (21.11.2026)

1. **Regel:** DI12 Punkt 14. Die Marker kommen aus einer Datei (`lauf.marker_datei`), die der Dienst genau einmal beim Start liest; aus dieser Datei, die der Nutzer schreiben kann, kommt nur die Liste. Der Zählordner kommt aus der Startdatei (`lauf.marker_ordner`).
2. **Fehlender Ordner:** Fehlt der Elternordner der Datei, gibt es keine Marker (wie bei einer fehlenden Datei): Der Dienst läuft auch außerhalb des Probelaufs, in dem der Ordner `/run/user/<uid>/werkbank-probelauf` nicht besteht. Das ist festgelegt (V), das Projekt hatte nur „fehlt die Datei“ genannt.
3. **Nur Zählung und Bericht:** Marker wirken auf nichts außer `mit_sitzung` (Punkt 14, „Start“).
4. **Mutanten:** 19 neue (`marker_*`). Der Eigentümer von Ordner und Datei ist in den Fällen nicht prüfbar; er ist ein Einheitstest im Bau (`lesen_mit`).

## Projektentscheide zu den Ziellisten (21.11.2026)

1. **Regel:** DI12 Punkt 15. `lauf.ziellisten` (optional) macht eine Nummer belegt, deren `s-<n>.zielliste.json` noch liegt; geprüft mit `lstat`, jeder Eintrag belegt, nur `NotFound` gibt frei. Der Dienst legt dort nichts an und liest nichts. Seit dem 23.11. entfernt er dort nur die eigene reguläre Liste einer früheren Nummer beim Vergeben (Punkt 15a).
2. **Reihenfolge:** Die Prüfung von `lauf.sitzungen` kommt vor der von `lauf.ziellisten`; ein Fehler beim zweiten Eintrag zählt nur, wenn der erste die Nummer nicht schon belegt hat.
3. **Mutanten:** 6 neue (`ziellisten_*`) und, mit Nachtrag 2, 5 neue (`laufzeit_*`, `zaehldatei_*`, `eintrag_fehlt_ist_frei`).
4. **Nachtrag 2 (Projektentscheid):** Jede Datei der Nummer belegt sie (Tabelle in Punkt 15); `lauf.laufzeit` ist neu, `lauf.marker_ordner` belegt mit den zwei Zähldateien.

## 16. Kernabbild-Grenze 1/1 (21.11.2026; Schritt 2a, Zweig, Stand vom 21.11.2026)

Ein Programm der Werkbank, das ein Geheimnis im Speicher hält, darf kein Kernabbild hinterlassen. Unter Ubuntu gibt der Kern ein Abbild über eine Pipe an `apport` oder `systemd-coredump`; die Grenze 0 allein hält das nicht auf (gemessen im Probelauf 6: ein Prozess, der abgezogen werden konnte, hinterließ einen Bericht mit dem Geheimnis im Speicher, einer, der nicht abgezogen werden konnte, keinen). Der Dienst schirmt sich deshalb selbst ab; die Regeln stehen im Crate `kernabbild` und gelten für `wb-dienst` und `wb-bruecke` gleich.

1. **Erster Schritt des Dienstwegs.** `programm` ruft `abschirmen` auf, nach den Weichen `--exec` und `--kind` und der Prüfung des Aufrufs, vor `signale_anmelden`, vor dem Socket und vor jedem Faden. Es ist die erste Handlung, bevor ein Geheimnis gelesen wird. Das Kind ist nach seinem `exec` wieder abzugsfähig (seine `/proc`-Dateien gehören dem Nutzer); es schützt nur die geerbte Grenze 1/1.
2. **Was `abschirmen` tut.** (a) Unter Linux `prctl(PR_SET_DUMPABLE, 0)`: der Prozess ist nicht abziehbar, seine `/proc/<pid>`-Dateien gehören root. (b) Auf jedem System `RLIMIT_CORE` weich 1 und hart 1. Schlägt `prctl` oder `setrlimit` fehl, endet der Start (fail closed, Gründe in Punkt 4).
3. **Die Werte.** 1 und nicht 0: bei einem Muster mit Pipe hört der Kern an der Grenze 1 auf, bei einem Dateimuster liegt 1 Byte unter der kleinsten Größe eines Abbilds. Eine harte 0 könnte ein Kind ohne root nicht wieder anheben; kein Programm setzt die harte Grenze auf 0. Von `/proc/sys/kernel/core_pattern` werden höchstens 4096 Byte gelesen.
4. **Falltabelle von `entscheiden(harte_grenze, muster)`** (die reine Entscheidung; `lage` und `lage_mit` geben sie ohne Wirkung zurück). Das Muster wird nur gelesen, wenn die harte Grenze 0 ist.

| harte Grenze | Muster | Entscheidung |
|---|---|---|
| nicht 0 (auch unbegrenzt) | wird nicht angesehen | weiter, Grenze 1/1 wird gesetzt |
| 0 | fehlt (macOS) | weiter ohne Setzen, Hinweis `MusterFehlt` |
| 0 | beginnt nicht mit `\|` (Dateimuster) | weiter ohne Setzen, Hinweis `MusterDatei` |
| 0 | beginnt mit `\|` (Pipe) | Abbruch `kernabbild.muster_pipe` |
| 0 | nicht lesbar | Abbruch `kernabbild.muster_nicht_lesbar` |

   Dazu die zwei Abbrüche des Setzens: `kernabbild.abzug_sperren` (`prctl` verweigert) und `kernabbild.grenze_setzen` (`setrlimit` verweigert). Die Gründe sind feste Texte ohne Pfad und ohne Inhalt.
5. **Ausgabe und Exitcode.** Ein Hinweis ist eine Zeile auf der Fehlerausgabe, `wb-dienst: kernabbild: ...`; der Dienst läuft weiter. Ein Abbruch ist eine Zeile `wb-dienst: Kernabbild nicht abschirmbar: <Grund> (<Abhilfe>)` auf der Fehlerausgabe und **Exitcode 5**. Die 5 gehört nur diesem Schritt (die Codes des Starts sind 1 bis 4, 64 gehört der Hülle und dem Kindprogramm); sie wird sonst von nichts genutzt oder ausgewertet.
6. **Beobachtung im Nutzer-Namensraum (Regel für die Prüfstellen).** Ein Prozess mit `PR_SET_DUMPABLE 0` ist für `lsof` desselben Nutzers unsichtbar (gemessen auf Linux: `lsof -i` und `-U` je 0 Zeilen, `/proc/<pid>/fd` gibt `Permission denied`). Eine Prüfung „keine Netz-Sockets“ wäre dann grün, ohne etwas zu sehen. Darum startet der Prüfclient den Dienst unter Linux in `unshare -Ur -- setpriv --bounding-set=-all --inh-caps=-all --` (kein Fork: beide führen per `exec` aus, `proc.pid` bleibt die PID des Dienstes). Der Beobachter bleibt außerhalb, mit demselben Nutzer; er besitzt den Namensraum und sieht den Prozess. **Positivkontrolle zuerst:** Der Prüfclient prüft, ob `lsof` den Prozess sieht (und ob `unshare` und `setpriv` gehen); sieht er ihn nicht, endet der Lauf mit `Umgebung` und einem Grund, also rot, nie grün. Auf anderen Systemen als Linux gibt es kein `PR_SET_DUMPABLE`, also keinen Vorspann. `DI_NS=aus` (ohne Namensraum) ist eine Gegenprobe der Tests, nie eine Einstellung des Produkts: der Dienst ist dann für `lsof` blind, und die Positivkontrolle muss den Lauf beenden.
7. **Der Namensraum ändert an den Fällen nichts** (gemessen in Schritt 2a): Die uid im Namensraum ist 0 und entspricht der echten uid des Nutzers; Eigentümer von Socket-Ordner und Socket, `geteuid`, Modus 0700 und `getpeereid` stimmen weiter überein, DI2 bleibt gleich. Jeder bestehende Fall und jede Erwartung ist unverändert (`cmp`); nur `pruefclient.py` ändert sich (Diffs: Byte für Byte gleich `rust/crates/dienst/tests/werkzeug/pruefclient.py` im Stand vom 21.11.2026).
8. **Kein neuer Fall.** Die Regeln 1 bis 5 werden im Bau durch Rust-Tests und Mutanten gehalten (Einheitstests in `dienst/src/abschirmung.rs` und im Crate, Test `dienst/tests/abschirmung.rs`: stat gehört root, `Max core file size 1 1`, das Kind erbt 1/1; `cargo mutants --in-diff`: 36 Mutanten, 0 verpasst), nicht durch Fälle von außen: Die Entscheidung bei harter Grenze 0 hängt vom `core_pattern` des Wirts ab, das ein Fall nicht festlegen kann, und die Referenz `dienst_ref.py` kennt den Schritt nicht (sie ist ein Python-Modell des Protokolls, kein Programm mit Speicher). Die Regel 6 halten die Tests `dienst/tests/beobachtung.rs` (der Dienst ist sichtbar, 0 Netz-Sockets; ein zusätzlicher TCP-Socket macht die Prüfung rot; ohne Namensraum endet die Positivkontrolle rot). Damit bleibt keine Regel dieses Abschnitts ohne Prüfung; im Vertrag `dienst` fehlt kein Fall.
9. **Verweise.** Die Verweise auf diesen Abschnitt aus den Verträgen `einschluss` und `anmelden` und aus dem Läufer setzen deren Eigentümer; hier sind sie nur genannt.

## 17. Subreaper und Waisen einer Sitzung (22.11.2026; Zweig)

Anlass (gemessen, 21.11.2026): `bwrap` 0.12.0 setzt das Todessignal seines inneren Prozesses erst in `do_init` (`bubblewrap.c:610`). Ein SIGKILL an `werkbank-lauf` in dessen ersten Millisekunden ließ deshalb in 128 von 300 Runden Prozesse zurück, die unter PID 1 weiterliefen. Der Dienst ist darum unter Linux der Subreaper seiner Sitzungen: Jede Waise unter einer Sitzung kommt zu ihm, er sammelt sie ein und beendet am Ende der Sitzung die Waisen dieser Sitzung. Die Regeln stehen im Modul `rust/crates/dienst/src/waisen.rs`.

1. **Stelle im Dienstweg (nur Linux).** `programm` ruft `waisen::subreaper_werden` direkt nach `kernabbild::abschirmen` auf, vor `signale_anmelden`, vor dem Socket und vor dem ersten Kind (`rust/crates/dienst/src/lib.rs:83`). Es setzt `PR_SET_CHILD_SUBREAPER` über `rustix::process::set_child_subreaper` (Merkmal `process`, kein `unsafe`, keine neue Abhängigkeit). Wird der Aufruf verweigert, endet der Start mit **Exitcode 1** und genau einer Zeile `wb-dienst: Subreaper: <Art>` auf der Fehlerausgabe (wie ein Fehler beim Anmelden der Signale; die Codes 2 bis 5 bleiben unverändert). Nachtrag 23.11.2026 (Befund N1): Ist der Dienst Subreaper, prüft `Dienst::neu` vor dem Socket und vor dem ersten Kind einmal, dass `/proc/thread-self/children` (gleich `/proc/self/task/<tid>/children`) lesbar ist; fehlt die Datei (ein Kernel ohne `CONFIG_PROC_CHILDREN`, ein `/proc` mit `hidepid` oder ohne `task`), endet der Start mit **Exitcode 1** und genau der Zeile `wb-dienst: Subreaper: children fehlt` (fail closed: ohne die Datei endete jede Jagd stumm nach dem ersten Blick).
2. **macOS und andere Systeme: alles wie bisher.** Dort gibt es keinen Subreaper und keinen pidfd. Der Dienst setzt nichts, sammelt nichts ein und signalisiert keiner Waise; jede Funktion des Moduls tut dort nichts. Das ist eine Regel, keine Lücke dieses Nachtrags.
3. **Nur adoptierte Prozesse einsammeln.** Die eigenen Kinder des Dienstes sind die Kinder der Sitzungen; ihr Ende liest `traeger::Kind` (`try_wait`, `wait`). Der Dienst ruft nie `waitpid(-1)` und nie `waitid(P_ALL)`. Gewählter Weg: Er liest die Kinder des Prozesses aus `/proc/self/task/<tid>/children` (alle Fäden), lässt jede PID eines eigenen Kindes aus, das sein Träger noch nicht eingesammelt hat, öffnet für jede andere PID einen pidfd, prüft danach den Prozess und sammelt ihn mit `waitid(P_PIDFD, WEXITED | WNOHANG)` ein, wenn er beendet ist. So berührt der Aufruf genau diesen Prozess. Das Ende eines eigenen Kindes sieht der Dienst mit `WNOWAIT` auf dessen pidfd, der direkt nach dem Start geöffnet wird; der Status bleibt dem Träger. Grund der Wahl: `waitid(P_ALL, WNOWAIT)` liefert immer dasselbe erste Kind und sieht dahinter liegende Zombies nicht; die Aufzählung sieht jedes Kind. Ein adoptiertes Kind kann nur der Dienst einsammeln, also bleibt seine PID an den Prozess des pidfd gebunden, solange die Prüfung läuft.
4. **Takt.** Der Dienst sieht höchstens alle 100 ms nach seinen Kindern, während einer Jagd (Punkt 5) alle 5 ms. Nur wenn das Setzen in Punkt 1 gelang, sammelt er ein und jagt; ein Test, der den Dienst in seinem eigenen Prozess laufen lässt, ist kein Subreaper, und das Ende der Kinder anderer Tests geht den Dienst nichts an.
5. **Am Ende einer Sitzung beendet der Dienst nur deren Waisen.** Die Kennung der Sitzung (sid) ist die PID des Kindes der Sitzung (`werkbank-lauf`, Sitzungsleiter in der PTY). Die Jagd beginnt, sobald dieses Kind beendet ist (`WNOWAIT`), spätestens mit `Kindende`, einmal je Sitzung. Grund für den frühen Beginn: Eine Waise, die die PTY offen hält, hielte sonst die Ausgabe offen, und `Kindende` käme nie. `Kindende` kommt deshalb erst, wenn die Waisen, die die PTY halten, beendet sind, mit dem Code des Kindes. Je Prozess gilt diese Reihenfolge gegen die Wiederverwendung einer PID: zuerst `pidfd_open`, dann die Prüfung von `/proc/<pid>/stat` (Feld 4 ist die PID des Dienstes, Feld 3 ist kein Zombie, Feld 6 ist die sid der Sitzung), dann `pidfd_send_signal` über diesen pidfd. Ein Prozess, der die Prüfung nicht besteht, bekommt kein Signal.
6. **Folge wie bei `Beenden` (DI12 Punkt 6):** SIGHUP, bis zu 1 s Wartezeit, dann SIGKILL, wiederholt bis der Prozess beendet und eingesammelt ist. Begründete Abweichung: Ein Prozess, der erst während der Jagd adoptiert wird (ein Enkel, dessen Elternteil gerade endete), bekommt sofort SIGHUP und SIGKILL zum selben Zeitpunkt wie die anderen, also unter Umständen weniger als 1 s nach seinem SIGHUP. Eine Jagd endet, wenn ein Blick keinen Prozess ihrer Sitzung mehr fand (weder lebend noch in diesem Blick eingesammelt), spätestens nach 10 s. Ein Prozess, der danach noch lebt (etwa im Zustand `D`), bleibt bis zum Dienstende.
7. **Eigene Sitzung bleibt stehen.** Wächter, Suche und Modellweg rufen `setsid` und haben eine eigene sid. Der Dienst signalisiert ihnen nie; er sammelt sie nur ein, wenn sie beendet sind. Ihr SIGTERM bleibt bei DI12 Punkt 12 (PID-Dateien) und beim Wächter.
8. **Dienstende.** Nach dem Ende der Kinder (DI8) gilt dieselbe Folge für die Waisen aller Sitzungen, vor dem Entfernen der Ordner (DI12 Punkt 12). Scheitert ein Kern nach dem Start eines Kindes in `Starten`, jagt der Dienst auch die Waisen dieses Kindes. Nachtrag 23.11.2026 (Befund M1): Scheitert `Dienst::neu` nach dem Start des Kindes aus `kind` (Kern, Socket, nicht blockierend, Wecksocket, Bereitschaft), beendet der Dienst das Kind und jagt danach die Waisen dieses Kindes mit der ganzen Folge (Punkt 6, höchstens 10 s), bevor er mit dem Startfehler endet; Exitcode und Zeile des Startfehlers bleiben unverändert. Beide Wege nutzen dieselbe Hilfe (`waisen::kinder_verwerfen`; der Startweg über `waisen::start_verwerfen`), die die PID des Kindes vor seinem Ende nimmt.
9. **Grenzen.** (a) Nur Linux (Punkt 2). (b) Wird die PID einer beendeten Sitzung während ihrer Jagd neu an ein Kind einer laufenden Sitzung vergeben, endet die Jagd sofort; der Linux-Kernel vergibt eine PID erst neu, wenn kein Prozess sie mehr als sid trägt (hergeleitet aus dem Kernel, nicht gemessen). (c) Ein Enkel unter einem lebenden Prozess, der nicht der Dienst ist (etwa unter dem Wächter), ist kein Kind des Dienstes und wird nicht erreicht. (d) Ein SIGKILL an den Dienst selbst lässt seine Waisen an den nächsten Subreaper darüber gehen (hergeleitet: im Paket `systemd --user`, das beim Stoppen der Einheit die Kontrollgruppe beendet); das ist nicht Gegenstand dieses Abschnitts.
10. **Prüfmittel. Kein neuer Fall.** Der Prüfclient kann keine Waise einer Sitzung erzeugen und die Prozesstabelle des Dienstes nicht von außen beurteilen, ohne ein Zeitverhalten anzunehmen; die Referenz `dienst_ref.py` ist ein Modell des Protokolls ohne Prozessbaum, und die Fälle laufen auch am Mac, wo Punkt 2 gilt. Die Regeln halten Rust-Tests und Mutanten:
    - `rust/crates/dienst/tests/waisen.rs` (Linux, das echte `wb-dienst`): `ein_eigenes_kind_und_eine_waise_enden_zugleich_der_code_stimmt_und_die_waise_ist_geerntet` (fünf Runden: `Kindende` mit Code 7, die Waise ist eingesammelt, kein Zombie), `die_waisen_einer_beendeten_sitzung_enden_mit_sighup_und_sigkill_ein_eigenes_setsid_bleibt` (eine Waise außerhalb der Vordergrundgruppe endet durch SIGHUP in unter 800 ms, eine SIGHUP-taube endet frühestens nach 900 ms durch SIGKILL, `Kindende` mit Code 3, der Wächter mit eigenem `setsid` lebt mit dem Dienst als Elternteil weiter und wird nach seinem Ende eingesammelt), `das_dienstende_beendet_die_waisen_jeder_sitzung`.
    - Einheitstests in `rust/crates/dienst/src/waisen.rs` (Felder von `stat`, `children`, Folge SIGHUP und SIGKILL, Höchstdauer, neu vergebene PID, der Blick mit `WNOWAIT` lässt den Status dem Eigentümer, der Testprozess ist nie Subreaper). Nachtrag 23.11.2026: `start_verwerfen_beendet_auch_die_waisen_des_kindes` (Linux; der Test startet das Testprogramm mit nur sich selbst neu, wird dort Subreaper, startet ein SIGHUP-taubes Kind mit einer SIGHUP-tauben Waise und prüft, dass die Hilfe aus Punkt 8 auch die Waise beendet und einsammelt) und `ohne_subreaper_gilt_die_children_datei_als_lesbar`. Der Startfehler selbst ist über das Programm nicht zeitfrei erreichbar (der Kern prüft beim Bau keine Grenze, und das Kind bekommt SIGHUP Mikrosekunden nach seinem Start); die Anbindung der fünf Zweige hält der Text von `Dienst::neu`.
    - `cargo mutants -p dienst --in-diff` im Einschluss und die Nachstellung von `fenster.py` mit dem echten `wb-dienst` (Ergebnis).
11. **Bestehende Fälle und Erwartungen** sind Byte für Byte unverändert (`cmp` aller Dateien unter `aussentests/` und `verdeckt/`, Ergebnis); nur dieser Abschnitt und die Kopfzeile sind neu.

## 18. Meldung von `lauf` an den Dienst (22.11.2026; Zweig)

Anlass (Befund B24, Vertrag `start`, OPEN 3): Der Dienst konnte die Codes 64 bis 78 von `werkbank-lauf` nicht vom Code des Harness unterscheiden. Die Form ist abgestimmt, das Projekt baut die Seite von `lauf`. Ein vererbter Deskriptor geht nicht: Der Träger (`portable-pty` 0.9.0) schließt im Kind vor dem `exec` jeden Deskriptor über 2, und ein eigenes `pre_exec` brauchte `unsafe`. Darum meldet `lauf` über einen Socket, den es über einen Pfad erreicht. Die Regeln stehen im Modul `rust/crates/dienst/src/meldung.rs` und in dessen Anbindung (`sitzung.rs`, `dienst.rs`). Sie gelten nur für Sitzungen aus `Starten` (DI12); die Sitzung aus `kind` hat keinen Meldekanal und ihr Kind bekommt keine Option.

1. **Kanal.** Vor dem Start des Kindes legt der Dienst im Ordner der Sitzung (`<socket.ordner>/<kennung>`, DI12 Punkt 3.5) einen Unix-Stream-Socket **`meldung.sock`** an: gebunden, Modus **genau 0600** (gesetzt zwischen `bind` und `listen`, so dass niemand mit einem anderen Modus verbinden kann), nicht blockierend, `listen`. Nachtrag 23.11.2026 (Befund N3): Der Modus wird am Namen selbst gesetzt, ohne einem Link zu folgen. Unter Linux öffnet der Dienst den Namen mit `O_PATH | O_NOFOLLOW`, verlangt einen Socket und setzt den Modus über `/proc/self/fd/<n>`; anderswo setzt er ihn mit `fchmodat(…, AT_SYMLINK_NOFOLLOW)`. Danach prüft er mit `lstat`, dass der Name ein Socket mit Modus 0600 ist. Sonst lehnt er den Start mit `meldung_kanal` ab; das Ziel eines Links bleibt unverändert. Ist der Pfad länger als **100 Byte** (die Grenze von DI1 für den Socket des Dienstes, die `sun_path` des Mac), oder lässt sich der Socket aus einem anderen Grund nicht anlegen, **lehnt der Dienst den Start ab**: `Ablehnung meldung_kanal`, `code` ebenso, `text` „Meldekanal der Sitzung nicht anlegbar“. Der Unterordner wird entfernt, das Programm nicht gestartet, keine Nummer verbraucht (fail closed). Der Dienst entfernt den Socket am Ende der Sitzung mit deren Ordner, wie bisher durch Deskriptoren (DI12 Punkt 12, `loeschen.rs`), und löscht nichts über einen Pfad.
2. **Aufruf.** Das Kind der Sitzung bekommt `argv = [lauf.programm, "--meldung", <pfad von meldung.sock>, <pfad der start.json>]` (genau vier Einträge, beide Pfade absolut), sonst unverändert: die Umgebung genau `lauf.env`, das Arbeitsverzeichnis `/` (DI12 Punkt 3.6). Vorher war `argv = [lauf.programm, <pfad der start.json>]` (genau zwei Einträge). Ein `lauf`, das die Option nicht bekommt, verhält sich wie bisher (Sache von `lauf`).
3. **Annahme.** Der Dienst nimmt **genau eine** Verbindung an, in jeder Runde seiner Schleife und ohne zu blockieren. Er prüft die Gegenseite: **dieselbe uid wie der Dienst und die pid des eigenen Kindes dieser Sitzung**; die pid ist die des Prozesses, der `connect` aufrief. Unter Linux mit `SO_PEERCRED` (`rustix::net::sockopt::socket_peercred`, Merkmal `net` war schon da, kein `unsafe`). Unter macOS (23.11.2026) mit `nix` 0.28.0, Merkmal `socket`, nur unter macOS: `getsockopt` mit `LocalPeerCred` (`LOCAL_PEERCRED`, die effektive uid) und `LocalPeerPid` (`LOCAL_PEERPID`, die pid); das `unsafe` liegt in `nix`, der Dienst hat keines. Scheitert die uid, schließt der Dienst die Verbindung. Hat die Gegenseite ihr Ende vor dem `accept` schon geschlossen, antwortet macOS bei der pid mit `ENOTCONN` (gemessen, macOS 26.5.1); dann entscheidet allein die uid, damit eine Zeile kurz vor dem Ende von `lauf` nicht verloren geht (Punkt 5). Jeder andere Fehler der pid schließt die Verbindung. **Jede andere Verbindung schließt der Dienst sofort und liest nichts von ihr;** die erste Verbindung, die die Prüfung besteht, wird die eine, danach schließt der Dienst den Hörer, so dass eine weitere Verbindung am Socket scheitert (`ECONNREFUSED`).
4. **Zeile.** Der Dienst liest **höchstens 128 Byte** und blockiert nie. Gültig ist nur `<code> <grund>\n`: `code` eine ganze Zahl von 1 bis 255 in einfacher Dezimalform (nur Ziffern, kein Vorzeichen, keine führende Null), genau ein Leerzeichen, `grund` aus `[a-z0-9_.]` mit 1 bis 64 Zeichen, genau ein `\n`, **nichts danach**. Alles andere ist ungültig: kein Zeilenende in 128 Byte, eine unvollständige Zeile, Daten nach der Zeile, eine andere Form (auch `\r\n`, zwei Leerzeichen, ein Code 0 oder 256, ein `grund` mit Großbuchstaben, Bindestrich oder 65 Zeichen).
5. **Auswertung beim Kindende.** Beim Ende des Kindes (`Kindende`, DI8) **liest der Dienst den Socket zuerst leer** (nimmt an, was wartet, und liest) **und wertet erst dann das Ende aus.** So geht eine Zeile, die `lauf` kurz vor seinem Ende schreibt, nicht verloren. Vier Ausgänge:
   - **Zeile empfangen und gültig:** Die Sitzung endet **mit dem Code der Zeile** (nicht mit dem des Kindes). Dieser Code geht in `Kindende` an den Kern, in die Sitzungsliste (`code`) und in `Zustand` (`dienst.kind.<id>.code`), und die Sitzungsliste trägt das Feld **`grund`** (DI12 Punkt 5).
   - **Zeile empfangen, aber ungültig:** Fail closed, ein gescheiterter Start: Die Sitzung endet mit dem **festen Code 70** (`EX_SOFTWARE`) und dem `grund` **`lauf.meldung_ungueltig`** in der Sitzungsliste, gleich mit welchem Code das Kind endet, auch mit 0 (Projektentscheid, 22.11.2026: ein gescheiterter Start sieht nie wie ein Erfolg aus).
   - **Verbindung ohne Daten (EOF oder stumm):** Der Starter läuft; der Code des Kindendes gehört dem Starter oder dem Harness, es gibt keinen `grund`.
   - **Keine Verbindung bis zum Kindende:** `lauf` scheiterte vor dem `connect`; der Code des Kindendes gehört `lauf`, es gibt keinen `grund`.
   Eine Zeile wird erst beim Ende des Kindes gewertet; bis dahin läuft die Sitzung wie jede andere, der Dienst beendet sie nicht wegen einer Meldung.
6. **Grenzen.** (a) Stirbt `lauf` **nach dem `connect` und vor dem Trampolin an SIGKILL**, sieht der Dienst EOF ohne Daten; das Kindende trägt das Signal, es gibt keinen `grund`. (Nachtrag 23.11.2026, Befund R-B2 eines Reviews:) Diese Grenze gilt für **jedes Ende durch ein Signal** in diesem Fenster, nicht nur für SIGKILL, also auch für SIGABRT (`abort`, ein Fehler beim Belegen von Speicher, eine Panik während einer Panik) und für SIGSEGV, SIGTERM oder SIGHUP. Jedes Mal sieht der Dienst EOF ohne Daten, das Kindende trägt das Signal, und es gibt keinen `grund`. `panic = "abort"` steht in `rust/Cargo.toml:36` nur im Profil `ansicht` (die Browser-Ansicht für `wasm32`); in einem Bau mit diesem Profil endet jede Panik mit SIGABRT. Im Profil `release` wickelt eine Panik ab, und der Prozess endet mit dem Code 101 ohne Signal; das ist dann der Ausgang „Verbindung ohne Daten“ mit dem Code 101 (hergeleitet aus der Profilangabe, nicht gemessen). (b) **Die Prüfung von uid und pid gibt es unter Linux und macOS** (Punkt 3; unter macOS gemessen am 23.11.2026 mit den Tests aus Punkt 8). Unter macOS gilt eine Ausnahme: Hat die Gegenseite vor dem `accept` geschlossen, gibt es keine pid, und es zählt allein die uid. Ein anderer Prozess desselben Nutzers, der sich verbindet, eine Zeile schreibt und schließt, bevor der Dienst annimmt, wird dort angenommen. Auf anderen Systemen gibt es keine gemessene Prüfung der Gegenseite; **dort lässt sich der Dienst nicht bauen** (`compile_error!` in `rust/crates/dienst/src/meldung.rs`, Projektentscheid, 27.11.2026). Vorher nahm er dort die erste Verbindung an (fail open). Der Einheitstest `kein_zweig_fuer_fremde_systeme_laesst_durch` liest alle Quellen des Crates: Kein Zweig `cfg(not(any(…)))` darf eine Prüfung mit `true` oder `Ok(())` bestehen lassen, und der Meldekanal muss die Sperre tragen. Ein Bau für ein fremdes Ziel ist nicht gemessen, weil auf den Bauwirten kein solches Ziel installiert ist. (c) Dieselbe uid genügt nicht: Ein anderer Prozess desselben Nutzers wird unter Linux immer und unter macOS bei offener Verbindung an der pid erkannt. (d) Der Dienst prüft nicht, ob der `grund` zu einem Code von `werkbank-lauf` gehört; die Tabelle gehört dem Vertrag `start`.
7. **Festes Kindprogramm.** `wb-dienst --kind lauf <argv des Kindes …>` (DI12 Punkt 9) lässt das Paar `--meldung <pfad>` hinter dem Programm weg, bevor es die Startdatei liest und `ARGV=<Zahl>` zählt. So bleiben alle Fälle und Erwartungen unverändert; die Referenz `dienst_ref.py` ist ein Protokollmodell ohne Prozesse und startet das Kind ohne das Paar.
8. **Prüfmittel. Kein neuer Außenfall:** Der Prüfclient kann weder einen Socket in den Ordner einer Sitzung legen noch die Gegenseite eines Sockets nach pid beurteilen, und die Referenz kennt den Kanal nicht (wie bei Abschnitt 17). Die Regeln halten Rust-Tests und Mutanten:
   - `rust/crates/dienst/tests/meldung.rs` (Linux und macOS, das echte `wb-dienst`, ein Skript spielt `lauf`): `lauf_bekommt_die_option_meldung_vor_der_startdatei_und_der_socket_ist_0600`, `eine_zeile_gibt_den_code_und_den_grund_der_sitzung` (Zeile `69 lauf.harness_fehlt`, Zeile mit Code 1 bei Programmende 0, Grenzen der Form, der Kern bekommt denselben Code), `eof_ohne_daten_laesst_den_code_beim_starter` (Verbindung über ein `exec` gehalten), `ohne_verbindung_gehoert_der_code_lauf`, `stirbt_lauf_nach_dem_connect_an_sigkill_sieht_der_dienst_eof_und_das_signal`, `jede_andere_zeile_ist_lauf_meldung_ungueltig_mit_dem_festen_code_70` (15 ungültige Formen, darunter 200 Byte ohne Zeilenende und Daten nach der Zeile), `ein_fremder_prozess_der_sich_zuerst_verbindet_wird_geschlossen_und_ein_zweiter_nach_lauf_auch`, `ein_zu_langer_pfad_lehnt_den_start_ab_und_laesst_nichts_zurueck`.
   - Einheitstests in `rust/crates/dienst/src/meldung.rs`: die Form der Zeile mit ihren Grenzen, `binden_legt_den_socket_mit_0600_an_und_ein_langer_pfad_scheitert` (100 Byte binden, 101 nicht), `am_ende_wird_zuerst_gelesen_dann_ausgewertet`, `die_vier_ausgaenge_am_ende`, `nie_mehr_als_128_byte_werden_gelesen`, `eine_fremde_pid_wird_geschlossen_und_die_richtige_danach_angenommen`, seit 23.11.2026 `modus_setzen_folgt_keinem_link` (ein Link an Stelle des Sockets scheitert, sein Ziel behält den Modus 0644; eine Datei und ein fehlender Name scheitern ebenso), `die_verbindung_des_kindes_zaehlt_eine_andere_pid_und_eine_andere_uid_nicht` (ein echter Kindprozess, Linux und macOS); in `sitzung.rs` `kind_von_lauf_hat_vier_argumente_und_wurzel`; in `kindprogramm.rs` `die_meldung_vor_der_startdatei_wird_weggelassen`.
   - `cargo mutants -p dienst --in-diff` im Einschluss (Ergebnis).
9. **Bestehende Fälle und Erwartungen** sind Byte für Byte unverändert (`cmp` aller Dateien unter `aussentests/` und `verdeckt/`, Ergebnis). Geändert sind der Text dieses Abschnitts, die Kopfzeile, DI12 Punkt 3 (Ziffer 6 und die Gründe der Ablehnung), Punkt 5 (Feld `grund`) und Punkt 9 (das feste Kindprogramm) und Abschnitt 8 Punkt 18. Keine Erwartung eines Falls ändert sich, weil das feste Kindprogramm das Paar weglässt (Punkt 7).
10. **Verweise.** Der Vertrag `start` verweist auf diesen Abschnitt (Abschnitt „Meldung von `lauf`“, nicht auf eine Rust-Datei); den Eintrag dort setzt dessen Eigentümer.

## 19. Nachtrag P25: Kennung und Rasternummer getrennt (23.11.2026; Projektentscheid, 23.11.2026)

1. **Anlass.** Ein Lauf des Dienstes vergab höchstens 255 Nummern; der 256. Start endete mit `nummern_erschoepft` (Fall `mehr_als_255_starts_in_einem_lauf`, rot vor diesem Nachtrag). Im Betrieb läuft der Dienst lange.
2. **Kennung.** Die Kennung `s-<n>` kommt aus dem Zähler `naechste` (`dienst.gestartet`) und wird nie wiederverwendet (Punkt 4). Die Grenze ist `KENNUNG_MAX` = 999 999 999: neun Ziffern, die Form `^s-[0-9]{1,9}$`, die die Ziellisten (`messen.py`) und der Vertrag `start` prüfen. Ordner `s-<n>`, Ziellisten, Laufzeit, Markerdateien, Signalziele, Protokoll und Sitzungsliste folgen der Kennung.
3. **Rasternummer.** Die Nummer im Rasterrahmen bleibt ein Byte (`RA`, Rahmenformat unverändert). Die Sitzung aus `kind` hat 0; die Sitzungen von `Starten` teilen sich 1 bis 255 (`NUMMER_MAX`, im Prüfmodus `lauf.rahmen_max`).
4. **Frei** ist eine Nummer, wenn ihre Sitzung beendet ist und kein Betrachter sie abonniert (`Abo` an ohne späteres `Abo` aus und ohne Trennen). Eine abonnierte Nummer wird nie vergeben. Es gibt keine Uhr und keine Frist.
5. **Vergabe nur bei Bedarf** (bei `Starten`, nach der Prüfung der Kennung, vor Ordner und Kind): zuerst eine in diesem Lauf nie vergebene Nummer, dann die freie Nummer der Sitzung, die am längsten beendet ist (Reihenfolge der `Kindende`). Erst dann zeigt der alte Eintrag `nr: null`, und jede Verbindung vergisst die Nummer (`RA` R13); der erste Rahmen der neuen Sitzung ist ein Vollbild. Ist keine Nummer zu haben, endet der Start mit `Ablehnung nummern_erschoepft`, Code und Text wie in Punkt 3 („Keine Sitzungsnummer frei“).
6. **Grenze:** Eine beendete Sitzung bleibt in der Liste (Punkt 5, Mutant `beenden_entfernt_sitzung`). Sie ist **nicht mehr live ansehbar, sobald ihre Nummer neu vergeben ist**: ein `Abo` (an) auf eine Sitzung mit `nr: null` lehnt der Dienst mit `{"grund": "sitzung_ohne_nummer"}` (Rahmen 0x07) ab, und ihr Kern schickt keine Rasterrahmen mehr. Bis dahin zeigt sie ihr letztes Bild.
7. **Zustand:** `dienst.nummern` (Kennung → Rasternummer) zeigt `null` für eine Sitzung ohne Nummer.
8. **Leser der Nummer.** Ansicht (`rust/crates/ansicht/src/kern.rs`, `nr_der`) und alte Seite (`rust/seite/seite-main.js`, `nrIn`) lesen die Nummer aus dem Feld `nr` der letzten Liste, nie aus der Kennung; bekommt eine Nummer eine andere Sitzung, vergisst ihr Empfänger das Bild darunter. Der Läufer (`rust/spec/vorfuehrung/laeufer/drehbuchlib/bruecke.py:31`) folgt im selben Merge-Fenster.
9. **Fälle des Baus** (`rust/crates/dienst/tests/mehrere.rs`): `mehr_als_255_starts_in_einem_lauf` (257 Starts, `s-256` bekommt 1, `s-257` 2), `nummer_bleibt_bis_zur_neuvergabe_in_der_reihenfolge_des_endes` (Rahmengrenze 2: eine früh beendete Sitzung behält Nummer und Bild; die am längsten beendete wird zuerst neu vergeben; `sitzung_ohne_nummer`; Vollbild nach der Neuvergabe), `abonnierte_nummer_wird_nie_vergeben` (Rahmengrenze 1: `nummern_erschoepft`, nach `Abo` aus gelingt der Start). Einheitstest `rahmennummer_nie_vergeben_dann_am_laengsten_beendet` (`starten.rs`) mit der Grenze `KENNUNG_MAX`.
10. **Außentests.** Jede Erwartung mit einer `Sitzungsliste` trägt jetzt `nr`. Keine andere Erwartung ändert sich.

## 20. Nachtrag P26: Träger beim Kindende frei, höchstens 256 beendete Einträge (23.11.2026; Projektentscheid, 23.11.2026)

1. **Anlass.** Jede beendete Sitzung behielt ihren Träger bis zum Dienstende: den PTY-Master (zwei Deskriptoren `/dev/ptmx`) und den schreibenden Faden. Am Mac (systemweit `kern.tty.ptmx_max` 511) lehnte ein Lauf nach etwa 400 bis 450 beendeten Sitzungen jeden weiteren `Starten` mit `programm_nicht_startbar` ab (Messung vom 23.11.2026, Lauf 1).
2. **Träger frei (Teil 1).** Sobald der Dienst `Kindende` einer Sitzung verarbeitet hat (Code, Grund, Aufräumen, Punkt 12), gibt er ihren Träger zurück: Die Schreibschlange schließt (der schreibende Faden endet mit seiner Kopie des Masters), der PTY-Master schließt, der lesende Faden ist eingesammelt, das Kind ist schon eingesammelt (`traeger::Kind::freigeben`). Eintrag, Kern, letztes Bild (`dienst.traeger_zeilen`), Code, Grund und Rasternummer bleiben (Punkt 5, Abschnitt 19).
   - Eine Größe an eine beendete Sitzung ändert weiter das Bild des Trägers und den Kern; ein `TIOCSWINSZ` entfällt, weil keine PTY mehr da ist. Eingaben lehnt der Kern wie bisher mit `sitzung_beendet` ab; Bytes, die dennoch ankämen, verwirft der Träger.
   - `Beenden`, `kill_ab`, Waisenjagd (`blick`, `kind_zeit`) und Dienstende (Punkt 8) sehen die Sitzung unverändert als beendet; das Dienstende sammelt nur noch Träger ohne PTY ein.
   - Keine beobachtbare Erwartung ändert sich durch Teil 1.
3. **Obergrenze (Teil 2).** Die `Sitzungsliste` hält höchstens **256** beendete Sitzungen von `Starten` (`BEENDET_MAX`; im Prüfmodus darf `lauf.beendet_max` von 1 bis 256 sie verkleinern, wie `lauf.rahmen_max`, sonst Exitcode 2 mit `lauf.beendet_max`). Punkt 5 gilt mit dieser Einschränkung.
   - **Beim Ende selbst fällt die eigene Sitzung nie heraus** (Mutant `beenden_entfernt_sitzung` gilt weiter). Hebt ein Ende die Zahl über die Grenze, nimmt der Dienst die Sitzung heraus, die **am längsten beendet** ist (Reihenfolge der `Kindende`) und **die kein Betrachter abonniert** (Abschnitt 19 Punkt 4). Sind alle älteren abonniert, bleibt die Liste länger, bis ein späteres Ende einen freien Eintrag findet; ein `Abo` aus allein nimmt nichts heraus.
   - Die Liste nach diesem Ende zeigt den Eintrag schon nicht mehr. Am Ende der Runde der Schleife verwirft der Dienst die Sitzung ganz: Kern, letztes Bild, Eintrag in `dienst.kind`, `dienst.traeger_zeilen` und `dienst.nummern`. Ihre Kennung wird nie wiederverwendet (Abschnitt 19 Punkt 2); jedes Ereignis mit ihr wird mit `sitzung_unbekannt` abgelehnt.
   - Hatte sie noch eine Rasternummer, gilt die Nummer wieder als **nie vergeben** (Abschnitt 19 Punkt 5), und jede Verbindung vergisst sie (`RA` R13).
   - Ordner, Ziellisten und Marker: Den Ordner `<kennung>` hat das Ende der Sitzung schon entfernt (Punkt 12). Ziellisten und Markerzähler gehören der Kennung und ändern sich durch das Herausnehmen nicht.
   - Waisen: Die Jagd einer Sitzung beginnt bei ihrem Ende (Abschnitt 17). Eine herausgenommene Sitzung fehlt in der Liste der Sitzungsleiter, deren Waisen das Dienstende noch einmal beendet (hergeleitet: nur Waisen, die eine Jagd schon überlebt haben).
4. **Geänderte Erwartungen.** Alt: Eine beendete Sitzung bleibt bis zum Dienstende in Liste und Zustand. Neu: höchstens 256 beendete bleiben, die am längsten beendete freie fällt beim nächsten Ende heraus. Grund: Die Liste wuchs ohne Grenze (145 Byte je Eintrag; ab etwa 7200 Einträgen 502 der Brücke, hergeleitet aus derselben Messung). Keine Erwartungsdatei der Außentests ändert sich. Im Fall des Baus `mehr_als_255_starts_in_einem_lauf` (`mehrere.rs`, Abschnitt 19 Punkt 9) fehlt `s-1` nach 257 Enden in der Liste (alt `nr: null`, neu kein Eintrag).
5. **Fälle des Baus.** `rust/crates/dienst/tests/traeger_frei.rs` (`beendete_sitzungen_geben_pty_und_faeden_zurueck_und_behalten_bild_und_code`: 24 beendete Sitzungen, `/dev/ptmx` und Fäden wachsen nicht, Bild und Code bleiben), `rust/crates/dienst/tests/liste_grenze.rs` (257 beendete mit der Vorgabe; ein abonnierter alter Eintrag bleibt; das eigene Ende fällt nicht heraus), Einheitstests `freigeben_erst_nach_dem_ende_und_das_bild_bleibt` (`traeger`) und `lauf_beendet_max_nur_im_pruefmodus_von_1_bis_256` (`startdatei.rs`).

## 21. Nachtrag 25.11.2026: Plattform Windows (Durchstich B)

Stand des Baus: abgenommen 25.11.2026. `wb-dienst` läuft unter Windows 11 (Build 26300, x64, Ziel `x86_64-pc-windows-gnu`). Unter Linux und macOS ändert sich nichts: Jede Windows-Stelle steht unter `cfg(windows)` oder in einer eigenen Datei (`rechte_windows.rs`, `loeschen_windows.rs`, `sys/windows.rs`). **Dieser Nachtrag weicht an zwei Stellen von Projektentscheiden ab und braucht eine Entscheidung, bevor er gilt:** Punkt 1 (`unsafe` im Modul `sys`, gegen Abschnitt 11 „kein `unsafe`“) und Punkt 7 (keine Kernabbild-Grenze, gegen Abschnitt 16).

1. **`unsafe` nur in `src/sys/windows.rs`.** `std` hat unter Windows weder `AF_UNIX` noch Zugriffslisten, und keine Abhängigkeit des Crates bietet beides ohne `unsafe` (`rustix` 1.1.6 kennt `SocketAddrUnix` nur unter Unix). Das Modul erklärt die nötigen Win32-Aufrufe selbst (`ws2_32`, `advapi32`, `kernel32`, `bcrypt` ist nur in `bruecke`); es kommt kein Paket hinzu. Die Lints des Crates sind die des Workspace mit `unsafe_code = "deny"` statt `forbid`, damit nur dieses Modul es erlauben kann (wie im Crate `peerkanal`). Unter Unix enthält das Crate weiter kein `unsafe`.
2. **Socket: `AF_UNIX`, keine Named Pipe.** Gemessen am 25.11.2026 in der Windows-VM: `bind` legt im Ordner einen Eintrag an (Reparse-Punkt mit dem Tag `IO_REPARSE_TAG_AF_UNIX`), `bind` auf einen bestehenden Pfad (Socket oder Datei) scheitert mit `WSAEADDRINUSE`, der Eintrag bleibt nach `closesocket`, bis er entfernt wird; `WSAPoll` meldet Hörer und Verbindung bereit; `SIO_AF_UNIX_GETPEERPID` nennt den Prozess der Gegenseite. Ein gewöhnlicher zweiter Benutzer bekommt `WSAEACCES`, wenn die Zugriffsliste des Eintrags oder des Ordners ihn nicht nennt, und verbindet sich, wenn sie ihn nennt. Eine Named Pipe lässt sich nicht mit `WSAPoll` warten (`WSAENOTSOCK`) und liegt nicht in einem Ordner; DI2 Punkte 1, 4 und 7 hätten keinen Gegenstand. Die Plattformschicht mit Peer-Prüfung über eine Named Pipe (Crate `peerkanal`) bleibt davon unberührt.
3. **DI2 mit Zugriffslisten.** Der Ordner (Punkte 1 bis 3): ein Ordner, kein Reparse-Punkt (kein Link, keine Junction), Besitzer ist der eigene Benutzer (SID), die Liste ist geschützt (kein geerbter Eintrag), und jeder erlaubende Eintrag nennt den eigenen Benutzer oder `SYSTEM`; ein verbietender Eintrag stört nicht. Das ist die Entsprechung von 0700 (`SYSTEM` entspricht `root`). Die Prüfung läuft am geöffneten Griff; der Dienst hält den Ordner ohne `FILE_SHARE_DELETE` offen bis zum Ende, niemand kann ihn umbenennen, verschieben oder löschen. Der Socket-Pfad (Punkt 4) darf nicht bestehen. Der Eintrag des Sockets (Punkt 5) bekommt den eigenen Benutzer als Besitzer und eine geschützte Liste mit genau einem Eintrag, voller Zugriff für den eigenen Benutzer (Entsprechung von 0600); der Dienst liest sie zurück und beendet sich sonst mit Code 1. Punkt 6 gilt unverändert: kein TCP, kein UDP.
4. **Abweichungen von DI2 unter Windows.** (a) Ein Prozess mit erhöhtem Administrator-Token und `SeBackupPrivilege` oder `SeRestorePrivilege` verbindet sich trotz der Liste (gemessen); das entspricht `root` unter Unix. (b) Ein erhöhter Prozess legt Ordner mit dem Besitzer `Administrators` an; der Dienst lehnt einen solchen Ordner mit Code 3 ab, bis der Besitzer der eigene Benutzer ist. (c) Unter `loeschen_windows.rs` geht das Entfernen über den Pfad unter dem gehaltenen Ordner, nicht über `openat`; `std::fs::remove_dir_all` folgt keinem Link und keiner Junction.
5. **Signale.** SIGTERM und SIGINT sind unter Windows die Konsolenereignisse `CTRL_BREAK_EVENT` und `CTRL_C_EVENT` (über `signal-hook`, sichere Schnittstelle). Der Aufrufer startet den Dienst in einer eigenen Prozessgruppe (`CREATE_NEW_PROCESS_GROUP`) an derselben Konsole und schickt `CTRL_BREAK_EVENT`; das Ende ist dann wie in Abschnitt 2 (Socket weg, Kind beendet, Code 0). `TerminateProcess` lässt sich nicht abfangen und entspricht SIGKILL: Der Eintrag des Sockets bleibt, der nächste Start lehnt ihn nach DI2 Punkt 4 ab.
6. **Kind (DI8).** Das Kind läuft in einer Pseudokonsole (`ConPTY`, über `traeger`). `portable-pty` setzt `SHELL` nur unter Unix, und der Träger leert die Umgebung; darum startet das Programm unter Windows direkt, ohne die Hülle `--exec`, und die Umgebung ist genau `kind.env`. Ein Programm braucht unter Windows kein Ausführungsbit, jede Datei gilt als Programm. Es gibt kein SIGHUP und kein SIGKILL: `Beenden` (DI12 Punkt 6) schickt nichts; das Kind endet, wenn der Dienst die Pseudokonsole schließt (Sitzungsende, Dienstende). Ein Kind, das trotz geschlossener Pseudokonsole weiterläuft, beendet der Dienst nicht (offen, Job-Objekt).
7. **Kernabbild (Abschnitt 16).** `kernabbild` hat unter Windows keine Abschirmung; der Dienst startet ohne sie. Das ist eine Abweichung von Abschnitt 16 (dort Code 5) und braucht eine Entscheidung.
8. **Subreaper und Waisen (Abschnitt 17), Signale ans Ende einer Sitzung (DI12 Punkt 12).** Wie unter macOS: kein Subreaper, kein Signal; die Dateien der Sitzung werden trotzdem entfernt. Eine PID-Datei liest der Dienst unter Windows nicht (ohne Gerät und Inode in `std` gibt es keinen Beleg, dass die geöffnete Datei die geprüfte ist).
9. **`lauf` (DI12, Abschnitte 18 bis 20).** Die Sitzungsordner `<kennung>` und `start.json` entstehen neu mit der geschützten Liste des eigenen Benutzers (0700, 0600). Der Socket `meldung.sock` hat dieselbe Liste wie der Socket des Dienstes; die Gegenseite muss der Prozess des Kindes sein (`SIO_AF_UNIX_GETPEERPID`), den Benutzer prüft die Liste. Die Marker-Datei prüft Ordner und Datei über die Listen mit denselben Gründen wie unter Unix. Eine alte Zielliste (DI12 Punkt 15a) entfernt der Dienst nur, wenn Ordner und Datei dem eigenen Benutzer gehören und ihre Listen geschützt sind.
10. **Pfade.** Die Startdatei behält Pfade mit `/` am Anfang; Windows liest sie auf dem Laufwerk des Arbeitsordners (`/t/o` ist `C:\t\o`). Ein Pfad mit Laufwerksbuchstaben ist weiter ungültig. `socket.name` darf unter Windows zusätzlich weder `\` noch `:` enthalten. Der Pfad des Sockets braucht weniger als 108 Byte (`sun_path` von Windows).
11. **Schreiben (DI5).** Ein nicht blockierender `AF_UNIX`-Socket nimmt unter Windows zwei Aufrufe von `send` ganz an, gleich welcher Größe (gemessen: zweimal 4 000 000 Byte, zweimal 65 536 Byte), danach `WSAEWOULDBLOCK`. Der Dienst gibt darum je Aufruf höchstens 65 536 Byte; im System liegen dann höchstens 128 KiB, die `ausstehend` nicht zählt (wie der Kernelpuffer unter Unix, Abschnitt 10).
12. **Weitere gemessene Abweichungen.** (a) Ein Ordner ohne Rechte des eigenen Benutzers (`lauf.sitzungen`, `lauf.laufzeit`, `lauf.ziellisten` mit Modus 0000) sperrt `lstat` darunter nicht wie unter Unix: Windows meldet für einen fehlenden Namen `NotFound`, der Dienst sah die Nummer als frei (gemessen als erhöhter Administrator). Behoben durch den Nachtrag 25.11.2026 in Abschnitt 21b. (b) ConPTY gibt eine Ausgabe des Kindes in mehreren Stücken weiter, zuerst das Verbergen des Cursors; im Prüfmodus mit stehender Uhr wartet der Rest auf die nächste Uhr (hergeleitet aus dem Abbild mit verborgenem Cursor in `di5_stumm_ein_bild`). Mit laufender Uhr folgt der Rest im nächsten Bild. (c) Ein Kind endet unter Windows nie durch ein Signal; `Kindende` trägt immer einen Exitcode (Fall `di8_kindende_signal`).
13. **Prüfmittel.** Die Außenfälle laufen mit einer Windows-Kopie des Prüfclients (AF_UNIX über `ctypes`, die Modi als Listen, `CTRL_BREAK_EVENT`, `netstat` statt `lsof`). `netstat` zeigt keinen `AF_UNIX`-Socket; die Positivkontrolle von `lsof` entfällt. Die festen Kindprogramme setzen den Rohmodus der Konsole selbst (`SetConsoleMode`, die Entsprechung von `stty raw -echo`); ein Kindprogramm `laufkind.exe` (Kopie von `wb-dienst.exe`, Prüfbau) ersetzt das Skript mit `exec`. Die festen Kindprogramme berichten Pfade und den Programmnamen in der Form der Startdatei (ohne Laufwerk, mit `/`, ohne `.exe`). Der Fall `di8_umgebung_cwd` braucht in der VM den Ordner `C:\usr` (sein `cwd` ist `/usr`). Der Fall `di2_ordner_fremd` nimmt unter Windows `C:\System Volume Information` (Besitzer `SYSTEM`); der Fall `di12_marker_ungueltig_fifo` ist unter Windows nicht anwendbar (kein FIFO). Ergebnis der Außenfälle unter Windows: siehe Punkt 14.
14. **Ergebnis unter Windows (gemessen am 25.11.2026, Windows 11 26H2 in einer VM, Prüfbau, drei Läufer gleichzeitig).** Sichtbarer Satz: 260 von 275 grün; verdeckter Satz: 227 von 244 grün. Die roten Fälle des sichtbaren Satzes: `Beenden` ohne Wirkung (Punkt 6; `di12_beenden`, `di12_beenden_ruhe`, `di12_sende_beenden`, `di12_nummern_erschoepft`, `di12_sitzungen_acht`), Ordner ohne Rechte (Punkt 12 a; `di12_kennung_ohne_rechte`, `di12_nummer_laufzeit_ohne_rechte`, `di12_nummer_zaehl_ohne_rechte`, `di12_zielliste_ohne_rechte`), kein Signal (Punkt 12 c; `di8_kindende_signal`), Stücke der Ausgabe im Prüfmodus (Punkt 12 b; `di5_stumm_nachholen`), nicht anwendbar (`di12_marker_ungueltig_fifo`, `di12_sende_fremd_vor_dem_kind`: kein FIFO, keine eigene Sitzung für den Zeugen vor dem Dienst) und unter Last (`di3_ablehnung_nur_sender`, `di12_sende_pid_link`, `di5_stumm_ein_bild`; allein wiederholt grün). Einmal erreichte der Vorspann-Titel von ConPTY den Kern als Titel einer Sitzung (W7 des Trägers; hergeleitet aus dem Titel, der dem Programmpfad gleicht).
15. **Offen: Windows Error Reporting (Projektentscheid, 25.11.2026).** Vor jeder Auslieferung unter Windows wird Windows Error Reporting für die Programme ausgeschlossen.
16. **Offen: Mutanten der Windows-Stellen (Projektentscheid, 25.11.2026).** Das Tor `mutants` lässt `src/sys/windows.rs` aus (`rust/.cargo/mutants.toml`, `exclude_globs`), weil es hinter `cfg(windows)` steht und unter Linux nie übersetzt wird. Vor jeder Auslieferung unter Windows laufen die Mutanten dieser Stellen in der Windows-Prüfmaschine; überlebt einer, ist die Auslieferung gesperrt.
17. **Windows-Code nur in `src/sys/windows.rs` (Projektentscheid, 25.11.2026).** Aller Windows-Code des Crates `dienst` steht in `src/sys/windows.rs`. Die Zweige `cfg(windows)` der übrigen Module rufen nur eine Funktion von dort auf oder importieren sie, ohne eigene Logik, Vergleiche oder Rechnungen; unter Unix ändert sich kein Code. Grund: Das Tor `mutants` nimmt `crates/*/src/sys/windows.rs` aus (Punkt 16), und `cargo-mutants` wertet `cfg` nicht aus. Ein Mutant hinter `cfg(windows)` in einer anderen Datei überlebt unter Linux und macht das Tor rot. Auch ein Testmodul für Windows zählt dazu: `cargo-mutants` überspringt `cfg(all(test, windows, ..))` nicht und mutiert seine Hilfsfunktionen; darum stehen die Fälle von 21a Punkt 4 in `src/sys/windows.rs`. Erlaubt außerhalb von `src/sys/` sind nur diese Ausnahmen: die beiden ganzen Dateien `rechte_windows.rs` und `loeschen_windows.rs`, die `lib.rs` mit `cfg_attr(windows, path = ..)` einsetzt (gemessen: `cargo mutants --list` führt keinen ihrer Mutanten, weil es `cfg_attr` nicht auswertet und für das Modul die Unix-Datei liest), und der Ausdruck `cfg!(windows)` in `socket_lesen` von `startdatei.rs` (Punkt 10; Linux übersetzt ihn, der Fall `socketname_mit_backslash_oder_doppelpunkt_nur_unter_windows_ungueltig` fängt ihn) samt diesem Fall. **Prüfung:** `rust/crates/dienst/tests/sys_regel.rs` liest die Quellen und meldet jede Stelle außerhalb von `src/sys/`, die nicht eine der festen kurzen Formen hat (ein `use` aus `sys`, ein Aufruf `sys::name(..)` mit Namen als Argumenten, ein Block oder eine Funktion mit genau diesem Aufruf, ein Feld vom Typ aus `sys`; die Regel steht im Kopf der Datei). Gegenprobe: Der Test baut aus den echten Quellen Fassungen mit einer eingesetzten Zeile mit Logik (Vergleich, Verzweigung, Rechnung, zweiter Aufruf, Closure, ein fremdes `use`, ein Testmodul mit Logik) und verlangt je einen Fund; in `starten.rs` eingesetzt macht die Zeile `#[cfg(windows)] const X: bool = 1 > 0;` den Test rot.

### 21a. Nachtrag 25.11.2026: `Beenden`, Kindcode und Job-Objekt unter Windows

Stand: abgenommen 25.11.2026. Dieser Nachtrag ersetzt die Sätze zu `Beenden` und zum Job-Objekt in Punkt 6 und den Punkt 12 (c), soweit sie dem hier Gesagten widersprechen. Unter Linux und macOS ändert sich nichts: `Beenden` schickt SIGHUP, nach bis zu 1 s SIGKILL, das Dienstende ebenso, dann `waitpid`.

1. **`Beenden` (DI12 Punkt 6) unter Windows.** Der Dienst beendet das Kind sofort über den Träger (`Kind::beenden`, `TerminateProcess`; Nachtrag 25.11.2026 im Vertrag `traeger`). Die Frist von 1 s in der Schleife bleibt: Läuft das Kind danach noch, ruft der Dienst `Kind::beenden` noch einmal. Ein Kind kann das Ende unter Windows nicht abfangen oder ignorieren.
2. **Code unter Windows.** Ein so beendetes Kind meldet `Kindende` mit dem Code **129**, wie ein Kind nach SIGHUP unter Unix (128 plus 1). Grund: Die Fälle `di12_beenden_ruhe`, `di12_sende_beenden`, `di12_nummern_erschoepft` und `di12_sitzungen_acht` erwarten nach `Beenden` 129; mit diesem Code gelten sie unter Windows unverändert, und ein Betrachter sieht auf jedem System denselben Code für ein verlangtes Ende. Der Code 137 (SIGKILL nach einem ignorierten SIGHUP) entsteht unter Windows nie.
3. **Dienstende (Abschnitt 2, DI8).** Der Dienst beendet jedes laufende Kind über `Kind::beenden`, wartet wie unter Unix bis zu 1 s auf alle zusammen und sammelt sie dann ein.
4. **Job-Objekt je Sitzung.** Jedes Kind (die Sitzung aus `kind` und jede Sitzung aus `Starten`) kommt direkt nach seinem Start in ein eigenes Job-Objekt mit `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`; jeder Prozess, den das Kind danach startet, ist ebenfalls darin. Der Griff des Jobs ist nicht vererbbar. Der Dienst schließt ihn am Ende der Sitzung (Kindende) und am Dienstende; das System beendet dann jeden Prozess, der noch im Job ist. Endet der Dienst auf andere Weise (Absturz, `TerminateProcess`), schließt das System seine Griffe und beendet die Prozesse der Jobs ebenso. Lässt sich der Job nicht anlegen oder ein laufendes Kind nicht aufnehmen, lehnt der Dienst den Start ab (fail closed) und beendet das schon gestartete Kind: Die Sitzung aus `kind` ist ein Startfehler nach DI8 (Exitcode 4, eine Zeile auf stderr, kein Socket, kein Kind); eine Sitzung aus `Starten` wird mit `programm_nicht_startbar` abgelehnt, ihr Ordner `<kennung>` entfernt, Kennung und Rasternummer bleiben frei. Hat das Kind schon geendet, bevor es in den Job kam, läuft die Sitzung ohne Job weiter und meldet ihr Ende wie sonst; ein Prozess, den es vorher gestartet hat, fällt unter Punkt 5. Fälle des Baus (Windows, Prüfbau, `JOB_VERWEIGERN` in `src/sys/windows.rs` lässt das Anlegen des Jobs scheitern): `ohne_job_startet_der_dienst_nicht_und_das_kind_endet` und `ohne_job_lehnt_der_dienst_starten_ab_und_das_kind_endet` (`src/sys/windows.rs`, Modul `dienst_tests`); beide prüfen denselben Start auch ohne den Fehler.
5. **Wettlauf zwischen Start und Job (offen).** Zwischen dem Start des Kindes durch `portable-pty` und seiner Aufnahme in den Job läuft das Kind schon. Ein Prozess, den es in dieser Zeit startet, ist nicht im Job und kann den Dienst überleben. `portable-pty` 0.9.0 startet das Kind nicht angehalten und nimmt keine Job-Liste beim Start an; schließen ließe sich die Lücke nur mit einem eigenen Start des Kindes. **Projektentscheid, 25.11.2026:** Die Lücke wird vor jeder Auslieferung unter Windows geschlossen, über einen Starter. Der Starter ist `wb-dienst` selbst: Er wartet, bis `IsProcessInJob` ihn im Job der Sitzung zeigt, und startet erst dann das Programm. `Meldung` prüft dann, dass die Gegenseite im Job der Sitzung steht, und `Beenden` beendet den Job. Bis dahin bleibt der Punkt offen; es wird nichts gebaut.
6. **Abweichungen, die bleiben.** (a) `di12_beenden` erwartet für ein Kind, das SIGHUP ignoriert, den Code 137; unter Windows meldet es 129 (Punkt 2). (b) `di8_kindende_signal` erwartet 143 (SIGTERM); unter Windows gibt es kein Signal, das Kind endet mit einem Exitcode. Beide Fälle unter `erwartet/` bleiben unverändert; unter Windows sind sie nicht anwendbar.
7. **Ergebnis unter Windows (gemessen am 25.11.2026, Windows 11 26H2 in einer VM, Prüfbau, drei Läufer gleichzeitig), vorher (development) und nachher.** Sichtbarer Satz: 261 und 265 von 275 grün; verdeckter Satz: 228 und 230 von 244; Brücke: 62 und 62 von 62. Nachher grün: `di12_beenden_ruhe`, `di12_sende_beenden`, `di12_nummern_erschoepft`, `di12_sitzungen_acht`. `di12_beenden` scheitert nur am Schritt, der 137 erwartet (Punkt 6 a). Nach der Runde mit development liefen in der VM fünf Zeugen `laufkind` weiter und nach der Brücke mit development ein weiteres `laufkind`; nach den Runden mit diesem Nachtrag keiner. Unter Last rot, allein wiederholt grün: `di12_sende_pid_form`, `di12_sende_links_im_ordner` (Exitcode des Dienstes `0xC0000142`), `di12_sitzungen_acht` und `di3_ablehnung_nur_sender` im verdeckten Satz (Titel des Vorspanns, Punkt 14), `di5_grenze_genau`. Allein wiederholt weiter rot wie vorher: `di5_stumm_nachholen`, `di5_stumm_ein_bild` (Punkt 12 b).

### 21b. Nachtrag 25.11.2026: Ordner ohne Rechte unter Windows (DI12 Punkte 13 und 15)

Stand: abgenommen 25.11.2026 (Projektentscheid, 25.11.2026). Dieser Nachtrag ersetzt Punkt 12 (a). Gemessen in der Windows-VM mit zwei Konten, einem erhöhten Administrator (über ssh, `SeBackupPrivilege` und `SeRestorePrivilege` aktiv) und einem gewöhnlichen Benutzer ohne Administratorrechte. Der Ordner hat die Liste der Prüfclients für Modus 0000 (geschützt, nur `SYSTEM`, Besitzer der eigene Benutzer).
- Gewöhnlicher Benutzer: `symlink_metadata` eines bestehenden Eintrags scheitert mit `PermissionDenied` (Fehler 5). Für einen fehlenden Namen meldet Windows aber `NotFound` (Fehler 2), weil jeder Benutzer die Prüfung des Durchgangs überspringen darf (`SeChangeNotifyPrivilege`). Das Öffnen des Ordners selbst scheitert mit `PermissionDenied`.
- Erhöhter Administrator: Jedes Öffnen mit `FILE_FLAG_BACKUP_SEMANTICS` gelingt, auch das des Ordners; ein bestehender Eintrag gibt `Ok`, ein fehlender `NotFound`.
- Folge: `lstat` allein zeigt unter Windows nicht, ob eine Nummer frei ist. Die Regel von DI12 Punkt 13 bleibt für Unix richtig; unter Windows lag es nicht nur am Konto der VM.
- Regel (fail-closed): Unter Windows prüft der Dienst vor dem `lstat` jedes Eintrags den Ordner, in dem er liegt (`kennung_belegt`, `ordner_durchsuchbar` in `src/sys/windows.rs`). Er öffnet den Ordner, liest dessen Beschreibung am Griff und prüft mit `AccessCheck` für das Token des Prozesses die Rechte `FILE_TRAVERSE` und `FILE_LIST_DIRECTORY`, die Entsprechung des Suchbits. Gruppen, verbietende Einträge und die Rechte des Besitzers zählen mit, Sicherungsrechte nicht. Fehlt der Ordner, bleibt der Eintrag frei wie unter Unix. Ist er kein Ordner, gewährt seine Liste die Rechte nicht oder scheitert die Prüfung, lehnt der Dienst den Start mit `start_ordner` ab.
- `unsafe`: sieben neue Blöcke in `src/sys/windows.rs` (`GetSecurityInfo`, `LocalFree`, `OpenProcessToken`, `DuplicateToken`, `CloseHandle` zweimal, `AccessCheck`), je mit SAFETY-Kommentar; außerhalb dieser Datei kein `unsafe`.
- Ergebnis der vier Fälle `di12_kennung_ohne_rechte`, `di12_nummer_laufzeit_ohne_rechte`, `di12_nummer_zaehl_ohne_rechte`, `di12_zielliste_ohne_rechte` als erhöhter Administrator: vorher 0 von 4 grün, nachher 4 von 4. Die Fälle unter `erwartet/` sind unverändert. Der Unit-Test `ein_ordner_ohne_eintrag_des_eigenen_nutzers_ist_nicht_durchsuchbar` ist in beiden Konten grün.
- Als gewöhnlicher Benutzer: vorher 0 von 4 grün (die Sitzung startet unter der nächsten Kennung), nachher 4 von 4; zwei Kontrollfälle (`di12_kennung_belegt_datei`, `di1_start_argv_leer`) vorher und nachher grün. Dafür brauchte die Windows-Kopie des Prüfclients eine Umgehung: Jeder Ordner aus `tempfile.mkdtemp` bekommt zusätzlich einen erbenden Vollzugriff des eigenen Benutzers; ohne sie scheiterte der Prüfclient als gewöhnlicher Benutzer beim Schreiben von `start.json` in seinen eigenen Ordner (Ursache nicht getrennt).
- Prüfmittel: Die Fälle laufen unter Windows in beiden Konten. Erst der gewöhnliche Benutzer zeigt, dass kein Sicherungsrecht die Ablehnung verdeckt; das erhöhte Konto zeigt, dass `AccessCheck` ein aktives Sicherungsrecht nicht durchlässt.
- **Offen 1 (Projektentscheid, 25.11.2026):** Ein Elternordner ohne Rechte des eigenen Benutzers meldet unter Windows hergeleitet (aus der Tabelle der Messung, nicht gemessen) ebenfalls `NotFound` beim Öffnen und gilt damit als fehlend; der Eintrag bleibt frei wie unter Unix.
- **Offen 2 (Projektentscheid, 25.11.2026):** Die Windows-Kopie des Prüfclients braucht als gewöhnlicher Benutzer die Umgehung aus dem Punkt „Als gewöhnlicher Benutzer“ (ein erbender Eintrag in den Ordnern aus `tempfile.mkdtemp`). Die Ursache ist nicht getrennt.

### 21c. Nachtrag 25.11.2026: Eigenheiten von ConPTY und Konsolen unter Windows, gemessen

Stand: Zweig, nicht abgenommen. Gemessen am 25.11.2026 in derselben VM (Windows 11 26H2, Prüfbau), allein und unter Last (drei Läufer gleichzeitig), mit einer Messprobe, die das Kind über `portable-pty` 0.9.0 wie der Träger startet und die Bytes der Pseudokonsole roh aufzeichnet. Die Fälle unter `erwartet/` bleiben unverändert.

1. **Scrollen in der letzten Zeile kommt als eigenes Stück.** Reicht ein Bild des Kindes bis in die letzte Zeile, schickt ConPTY den Zeilenvorschub, der scrollt, als eigenes Stück nach dem Bild: erst das Bild bis `ESC[?25h`, dann `\n` allein, ebenso `ENDE\r` und danach `\n` (Fenster 50×8, ein Bild aus vier umbrechenden Zeilen: 3 von 3 Runden). Ohne Scrollen kam jede Antwort des Kindes auf eine Eingabe in einem Stück (Fenster 40×10 und 64×8: 209 von 210). Einmal kam unter Last der Titel, den das Kind vor seinem ersten Bild mit einem eigenen Schreiben schickt, als eigenes Stück. Das Verbergen des Cursors (`ESC[?25l`) kam in keiner der 63 Runden als eigenes Stück. Der Dienst meldet das Scrollen darum als zweite Ausgabe. Im Prüfmodus mit stehender Uhr geht sie erst mit der nächsten Uhr hinaus; ein Fall, der vorher das Abbild prüft, sieht das Bild ohne das Scrollen. So scheitert `di5_stumm_ein_bild`, wenn sein Fenster das Bild bis in die letzte Zeile reichen lässt, unter Windows immer, auch allein. Mit laufender Uhr folgt das Scrollen im nächsten Bild. Dieser Punkt ersetzt die Herleitung in Punkt 12 (b) für diesen Fall.
2. **Vorspann (Punkt 14, W7 des Trägers).** In 63 Runden (33 allein, 30 unter Last) kam der Titel des Vorspanns jedes Mal nach `ESC[6n ESC[?9001h ESC[?1004h ESC[m`, vor jeder Ausgabe des Kindes und genau einmal. Ein Titel gleich dem Programmpfad in einem Fall unter Last ist damit nicht durch die Reihenfolge der Bytes erklärt; die Ursache ist offen. Zweite Messung (25.11.2026, abends): Ändert sich die Größe der Pseudokonsole, bevor ConPTY zum ersten Mal zeichnet, zeichnet ConPTY vor dem Titel neu: `ESC[?25l ESC[m`, dann je Zeile `ESC[K` mit `\r\n` dazwischen, dann `ESC[H`, erst danach der Titel (Größenänderung 0 ms nach dem Start: 16 von 16 Runden, auf dieselbe und auf eine andere Größe; 30 ms nach dem Start: 7 von 16; ohne Größenänderung und 300 ms nach `BEREIT`: 0 von 32). Der Träger meldete den verborgenen Cursor als `Ausgabe` ohne Zeilen und ließ danach den Titel als Ereignis durch. Seitdem schließt nur eine Zeile mit Inhalt oder ein anderes Ereignis den Vorspann; ein Schritt des Cursors allein schließt ihn nicht (`rust/crates/traeger/src/raster.rs`, Fall `das_neuzeichnen_von_conpty_vor_dem_vorspann_laesst_den_titel_verwerfen`). In `di3_ablehnung_nur_sender` und `di12_kennung_luecke` ändert sich die Größe nicht (0 von 18 Mitschriften); beide waren in 18 Läufen grün. Für diese beiden Fälle bleibt die Ursache offen.
3. **Konsolen im nicht interaktiven Desktop.** Ein Prozess, den ein Dienst oder eine ssh-Sitzung (Sitzung 0, `SharedSection=1024,20480,768`) mit einer eigenen Konsole startet, belegt Platz im Desktop-Heap. Ab dem 141. gleichzeitigen Prozess mit eigener Konsole endete jeder sofort mit `0xC0000142` (`STATUS_DLL_INIT_FAILED`), allein und unter Last gleich. 300 gleichzeitige Pseudokonsolen gaben keinen Fehler. Der Dienst startet Kinder nur in Pseudokonsolen; `wb-dienst.exe` importiert kein `user32.dll`. Unter Last lagen höchstens 10 `conhost` gleichzeitig vor. Ein Start des Dienstes, der mit `0xC0000142` endet, ist damit durch diese Last nicht erklärt; er kam in diesen Messungen nicht vor.
4. **Kindende mit `0xC000013A`.** Unter Last endete das Kind eines Falls mit Zeugen in einer eigenen Pseudokonsole (`di12_sende_pid_form`) einmal mit dem Code `0xC000013A` (`STATUS_CONTROL_C_EXIT`, Ende durch ein Konsolenereignis) statt 0, in 47 weiteren Läufen der Fälle `di12_sende_pid_form` und `di12_sende_pid_link` nicht. Welches Ereignis das Kind beendet, ist offen. Gegenprobe (25.11.2026, abends, Prüfbau mit Mitschrift nach Punkt 6): 58 Läufe mit Zeugen in einer verschachtelten Pseudokonsole gegen 58 ohne Zeugen (je 8 allein, je 50 neben drei weiteren Messströmen); jedes Kind endete mit 0, kein Kind meldete `SIGINT` oder `SIGBREAK`. Die volle Last der drei Läufer fehlte in dieser Messung. Der Code ist damit nicht reproduziert.
5. **Neuzeichnen nach einer Größenänderung.** Nach jeder Größenänderung zeichnet ConPTY neu, auch wenn das Kind nichts schreibt: Der Träger meldet `Ausgabe` mit verborgenem Cursor, `GroessenFolge` und `Ausgabe` mit sichtbarem Cursor. Das kam 1,4 bis 3,2 ms nach der Größenänderung (9 von 9 Läufen von `di5_stumm_nachholen`, allein und neben drei Messströmen). In diesem Fall folgen der Größenänderung des Abos `Stumm` und die Eingabe `g` schneller. Das Neuzeichnen trifft darum den schon stummen Betrachter und steht in seinem Rückstand; der Vergleich meldet `rueckstand.s-alpha: zu viel`. Unter Linux zeichnet das Kind `bilder` nach einer Größenänderung nicht neu. Das ist eine Eigenheit von ConPTY; kein Codefix, die Fälle unter `erwartet/` bleiben unverändert. Hergeleitet: Ein Fall mit einer Größenänderung kurz vor `Stumm` bräuchte unter Windows ein Bereit-Signal nach dem Neuzeichnen.
6. **Mitschrift des Prüfbaus.** Für diese Messungen schreibt der Prüfbau unter Windows zwei Mitschriften, nur wenn neben der Programmdatei (Kind) oder im Arbeitsordner des Dienstes ein Ordner `mitschrift` besteht: das Kind seine Konsolenereignisse (`SIGINT`, `SIGBREAK` über die sichere API von `signal-hook`, danach Ende mit `0xC000013A` wie die Vorgabe), der Dienst je Sitzung Größenänderungen, Eingaben und Ereignisse des Trägers. Ohne den Ordner schreibt keiner etwas. Der ausgelieferte Bau enthält beides nicht.

## Nachtrag 26.11.2026: Ordner lesen und durchsuchen (L9)

Grundlage: Vertrag `seite`, `rust/spec/seite/OBERFLAECHE.md` Fassung 2, Abschnitt 4, Lücke L9 mit den Bedienwegen B52 (Ordner-Blatt) und B53 (Inhaltssuche). Die Ansicht liest den Arbeitsordner einer Sitzung und durchsucht seinen Inhalt. **Sicherheitsstelle, das Projekt liest sie selbst.** Gründe in Abschnitt 8 Punkt 21.

1. **Ereignisse.** Zwei neue Arten der Ansicht, in beiden Betriebsarten (mit `kind` und mit `lauf`): `{"art": "OrdnerLesen", "sitzung": <Kennung>, "pfad": <Text>}` und `{"art": "InhaltSuchen", "sitzung": <Kennung>, "muster": <Text>}`. Die Liste der erlaubten Arten aus Abschnitt 5 (DI3) wächst um diese zwei (`ERLAUBT` in `rust/crates/dienst/src/dienst.rs`); die Prüffolge von DI3 bis `feld_verboten` gilt unverändert. Beide Ereignisse gehören dem Dienst: Der Kern sieht sie nie, und sie ändern keinen Zustand.
2. **Antworten.** Nur an den Sender, als Rahmen `Nachricht` (0x06), gebaut über `json_rahmen` (DI3, Grenze `RAHMEN_MAX`):
   - `{"art": "Ordnerinhalt", "sitzung", "pfad", "eintraege": [{"name", "art": "datei" | "ordner" | "anderes", "groesse"}…], "abgeschnitten": bool, "t"}`. `pfad` ist der gesendete Text. `groesse` ist die Größe einer regulären Datei in Byte, bei jeder anderen Art `null`. `anderes` ist alles außer regulärer Datei und Ordner: ein symbolischer Link (auch auf einen Ordner), eine FIFO, ein Socket, ein Gerät. Die Einträge stehen in Bytefolge der Namen, ohne `.` und `..`.
   - `{"art": "Suchtreffer", "sitzung", "muster", "treffer": [{"pfad", "zeile", "auszug"}…], "abgeschnitten": bool, "t"}`. `pfad` ist relativ zum Arbeitsordner, Glieder mit `/` getrennt; `zeile` zählt ab 1; `auszug` ist die Zeile als UTF-8 (eine ungültige Folge wird U+FFFD), ohne `\r` am Ende, auf höchstens 200 Byte am Ende eines Zeichens gekürzt.
   - `t` ist die Uhr des Dienstes (DI7).
3. **Ablehnung.** Rahmen `Ablehnung` (0x07) nur an den Sender, die Verbindung bleibt. Die Prüfungen laufen in dieser Reihenfolge, die erste zutreffende bestimmt den Grund:
   1. `form_ungueltig`: Das Ereignis hat nicht genau die drei Schlüssel `art`, `sitzung` und `pfad` (bzw. `muster`), oder ein Wert ist kein Text.
   2. `sitzung_unbekannt`: Keine Sitzung hat diese Kennung (der Raumkern hat keine).
   3. `fremder_kundenraum`: Der Raum des Betrachters ist nicht der Kundenraum der Sitzung, so wie der Kern der Sitzung beide kennt (dieselbe Prüfung wie beim `Abo`). Kennt der Kern den Betrachter oder die Sitzung nicht, oder hat die Sitzung keinen Kundenraum, gilt das als fremd.
   4. `grenze`: `pfad` länger als 4096 Byte oder mit mehr als 64 Gliedern; `muster` länger als 256 Byte.
   5. `pfad_ungueltig`: `pfad` beginnt mit `/` (absolut), enthält NUL oder `\`, hat ein leeres Glied (auch `a//b` und `/` am Ende), ein Glied `.` oder `..` oder ein Glied, das mit einem Buchstaben und `:` beginnt (Laufwerk unter Windows). Der leere Text ist der Arbeitsordner selbst. `muster_ungueltig`: `muster` ist leer oder enthält NUL, `\n` oder `\r` (die Suche geht zeilenweise).
   6. `klasse_gesperrt`: Der Betrachter darf die Datenklasse der Sitzung nicht lesen, oder es ist nicht belegt, dass er sie lesen darf (Punkt 6).
   7. `nicht_lesbar`: Die Sitzung hat keinen Arbeitsordner, oder der Ordner (die Wurzel oder ein Glied) fehlt, ist kein Ordner oder lässt sich nicht öffnen oder auflisten. Die Wurzel selbst als symbolischer Link ist `nicht_lesbar`.
   8. `pfad_ausserhalb`: Ein Glied von `pfad` ist ein symbolischer Link, gleich wohin er zeigt (unter Windows: ein Reparse-Punkt).
   9. `plattform_gesperrt`: Das System hat keinen sicheren Weg nach Punkt 5 (Windows, Punkt 8).
4. **Arbeitsordner.** Für die Sitzung von `kind` ist es `kind.cwd` der Startdatei, für eine Sitzung von `Starten` das Feld `arbeitsordner` der Start-Struktur (Crate `start`, absolut und normalisiert). Im Bau steht er an einer Stelle: `Sitzung::arbeitsordner` (`rust/crates/dienst/src/sitzung.rs`), gesetzt beim Anlegen der Sitzung in `dienst.rs`.
5. **Sicherheitszusagen (fail closed).**
   1. Nur Pfade unterhalb des Arbeitsordners. Der Pfadtext wird vor jedem Zugriff geprüft (Punkt 3.5), und der Dateiteil prüft jedes Glied noch einmal als einfachen Namen (`loeschen::name_gut`). Der Text ist nicht das einzige Schloss.
   2. Öffnen nur über Deskriptoren: die Wurzel einmal mit `openat(AT_FDCWD, …, O_RDONLY | O_DIRECTORY | O_NOFOLLOW | O_CLOEXEC)`, jedes Glied relativ zum vorher geöffneten Ordner mit denselben Schaltern. Kein Glied ist je ein Link. Wird ein schon geöffneter Ordner gegen einen Link getauscht, bleibt der Weg im geöffneten Ordner (kein Rennen zwischen Prüfung und Öffnen). Es gibt keine Prüfung per Pfadtext gefolgt von einem `open` über den Pfad.
   3. Die Suche öffnet eine Datei relativ zu ihrem Ordner mit `O_RDONLY | O_NOFOLLOW | O_NONBLOCK | O_CLOEXEC` (eine FIFO blockiert das Öffnen nicht) und liest sie nur, wenn `fstat` eine reguläre Datei mit genau einem Link nennt und Gerät und Inode gleich dem `statat` davor sind. Eine Datei mit zwei oder mehr Links wird nicht gelesen (ein harter Link auf eine Datei außerhalb hat dieselbe Inode). Einen Ordner betritt sie nur, wenn `fstat` des geöffneten Ordners Gerät und Inode des `statat` nennt. Links, FIFOs, Sockets und Geräte öffnet die Suche nie und folgt ihnen nie; sie sind kein Inhalt des Ordners und werden ohne Merkmal übergangen.
   4. Kein Fehler wird zu einer leeren Liste ohne Merkmal: Lässt sich die Wurzel oder der genannte Ordner nicht öffnen oder auflisten, ist das eine Ablehnung. Ein Eintrag, dessen `statat` scheitert, dessen Name kein UTF-8 ist (er ließe sich in keinem `pfad` nennen) oder der eine Grenze überschreitet, setzt `abgeschnitten` auf `true`. In der Suche gilt dasselbe für einen Ordner oder eine Datei, die sich nicht öffnen oder lesen lässt, eine Datei mit mehr als einem Link und eine Datei, deren Gerät oder Inode nach dem Öffnen abweicht.
   5. Der Dienst schreibt keinen Pfad, keinen Namen, kein Muster und keinen Treffer ins Protokoll (stderr).
6. **Datenklasse.** Eine Suche oder ein Lesen über eine Datenklasse, die der Betrachter nicht lesen darf, gibt es nicht. Der Dienst kennt heute keinen Weg zu beidem: Die Start-Struktur nennt Route und Kunden, aber nicht die Klasse des Inhalts im Arbeitsordner, und die Leseklasse einer Person (Teilvertrag `datentor person`) baut kein Crate, die Einstellungen tragen keine Leseklassen. **Darum lehnt der Dienst jedes Lesen und jede Suche mit `klasse_gesperrt` ab** (`arbeitsordner::klasse_lesbar`, fest `false`), nach den Prüfungen von Form, Sitzung, Raum und Text und vor jedem Zugriff auf das Dateisystem. Eine Quelle kommt mit einem Projektentscheid und einem Fall, der dann lesbare und gesperrte Sitzungen zeigt.
7. **Grenzen** (an einer Stelle im Bau: `GRENZEN` in `rust/crates/dienst/src/arbeitsordner.rs`):

   | Grenze | Wert | erreicht |
   |---|---|---|
   | Einträge je Ordner (Lesen und Suche) | 1000 | `abgeschnitten` |
   | Treffer je Suche | 200 | `abgeschnitten`, die Suche endet |
   | Tiefe der Suche (der Arbeitsordner ist 0) | 16 | tiefere Ordner bleiben, `abgeschnitten` |
   | Dateien je Suche | 5000 | `abgeschnitten`, die Suche endet |
   | Byte je Datei | 1 048 576 | der Anfang wird durchsucht, `abgeschnitten` |
   | Byte je Suche | 33 554 432 | die letzte Datei nur zum Teil, `abgeschnitten`, die Suche endet |
   | Laufzeit der Suche | 500 ms der Uhr des Dienstes | `abgeschnitten`, die Suche endet |
   | Länge des Auszugs | 200 Byte | gekürzt, kein Merkmal |
   | Platz der Treffer in der Antwort | 8 388 608 Byte (gezählt wie unten) | `abgeschnitten`, die Suche endet |
   | Länge von `muster` | 256 Byte | `grenze` |
   | Länge von `pfad` | 4096 Byte | `grenze` |
   | Glieder von `pfad` | 64 | `grenze` |

   Die Laufzeit misst der Dienst vor jeder Datei mit seiner Uhr (DI7). Im Prüfmodus steht die Uhr während eines Schritts still, darum greift die Laufzeit dort nie; ein Einheitstest prüft sie mit einer Uhr, die je Blick um feste Millisekunden weitergeht (`grenze_der_laufzeit_mit_der_uhr_des_dienstes`), ohne Zeitannahme. Die Antwort bleibt unter `RAHMEN_MAX`: Die Treffer einer Suche dürfen in der Antwort höchstens 8 388 608 Byte belegen, gezählt im schlimmsten Fall (jedes Byte von `pfad` und `auszug` als Escape mit sechs Byte, dazu 64 Byte je Treffer; `treffer_bytes`); ein Treffer, der darüber führt, beendet die Suche mit `abgeschnitten`. Ein `Ordnerinhalt` bleibt ohne eigene Zählung darunter (hergeleitet: 1000 Einträge mit Namen bis 1020 Byte, jedes Byte als Escape, sind weniger als 6,2 MB). Wäre eine Antwort trotzdem länger, macht `json_rahmen` daraus den `Hinweis` `nachricht_zu_gross` (DI3); ein Rahmen über `RAHMEN_MAX` geht nie hinaus.
8. **Windows.** `std` öffnet unter Windows eine Datei nur über ihren ganzen Pfad. Der Weg Glied für Glied relativ zu einem gehaltenen Griff (mit `FILE_FLAG_OPEN_REPARSE_POINT` und der Prüfung auf einen Reparse-Punkt je Glied) ist nicht gebaut. **Darum lehnt der Dienst unter Windows jedes Lesen und jede Suche ab** (`plattform_gesperrt`, `rust/crates/dienst/src/sys/windows.rs`); es gibt keinen Rückfall über den Pfad. Heute greift unter Windows wie überall vorher `klasse_gesperrt` (Punkt 6). Offen: der Weg über Griffe unter Windows, mit eigenem Nachtrag und Fällen.
9. **Fälle.** Außenfälle in `rust/crates/dienst/tests/faelle/` mit den Erwartungen in `tests/erwartet/`: `l9_klasse_gesperrt` (Lesen und Suche einer bekannten Sitzung im eigenen Raum, auch mit gutem `pfad`), `l9_pfad_und_muster` (die Gründe aus Punkt 3.4 und 3.5), `l9_form_und_sitzung` (`form_ungueltig`, `sitzung_unbekannt`), `l9_fremde_sitzung` (`fremder_kundenraum`) und `l9_lauf` (mit `lauf`: der Raumkern hat keine Kennung, eine gestartete Sitzung ist `klasse_gesperrt`). Einheitstests des Dateiteils in `rust/crates/dienst/src/arbeitsordner_fs.rs` (gutes Lesen, gute Suche, Link auf Datei und Ordner außerhalb, Link als Glied in der Mitte, Wurzel als Link, Rennen mit einem getauschten Ordner, harter Link, FIFO, Grenzen, unlesbare Einträge, Namen ohne UTF-8) und der Text- und Klassenprüfung in `arbeitsordner.rs`. Gutes Lesen und gute Suche gibt es nur als Einheitstest, weil Punkt 6 sie über den Socket sperrt.
10. **Geänderte Erwartungen.** Keine: Bisher waren `OrdnerLesen` und `InhaltSuchen` `art_nicht_erlaubt`; kein bestehender Fall nennt sie (hergeleitet: `grep` über `tests/faelle`).

## Nachtrag 26.11.2026: Oberfläche L1, L2, L3, L5, L6, L11

Grundlage: Vertrag `seite`, `rust/spec/seite/OBERFLAECHE.md` Fassung 2, Abschnitt 4 (Lücken L1 bis L11), und der Nachtrag gleichen Namens im
Vertrag `zustandskern` (R-L1, R-L2, R-L6). Die neuen Regeln heißen DI-L1, DI-L2, DI-L3, DI-L5, DI-L6 und DI-L11. Wo dieser Abschnitt nichts
sagt, gelten DI1 bis DI12 und die Abschnitte 15 bis 21c unverändert. L4 und L10 legen die Verträge `regelebenen`, `einstellungen` und `start` fest;
der Dienst reicht ihre Nachrichten nur durch. L9 steht in einem eigenen Nachtrag.

**Ablage der Fälle.** Fälle, für die der Code noch fehlt, liegen in `rust/crates/dienst/tests/faelle-ausstehend/` mit den Erwartungen in
`rust/crates/dienst/tests/erwartet-ausstehend/`. Der Läufer `faelle.rs` liest sie nicht. Der Bau einer Regel verschiebt ihre Fälle nach `faelle/`
und `erwartet/`; ein Fall mit demselben Namen ersetzt dann den alten. Fälle, die der heutige Code besteht, liegen gleich in `faelle/`.

**Ereignisse des Dienstes.** `Umbenennen`, `Loeschen`, `Fortsetzen` und `Wiederbeleben` sind wie `Starten` und `Beenden` Ereignisse des Dienstes
(DI12 Punkt 3): Der Kern sieht sie nie, sie gibt es nur mit `lauf`. Ohne `lauf` ist jedes von ihnen der Fehler `art_nicht_erlaubt` (DI3), die
Verbindung endet. Im Lauf ist auch `Freigabe` ein Ereignis des Dienstes (DI-L6). Die Liste `ERLAUBT` (DI3) bleibt die Liste der Ereignisse, die zum
Kern gehen.

### DI-L1 Worker (L1)

1. **Was ein Worker ist.** Ein Worker ist eine Sitzung von `Starten`, die einen Orchestrator nennt. Das Ereignis `Starten` erlaubt dazu zwei
   Schlüssel neben `start`: `orchestrator` (die Kennung einer Sitzung dieses Dienstes) und `rolle` (eine Kennung, Vorgabe `worker`; nur zusammen mit
   `orchestrator`). Die Start-Struktur ändert sich dadurch nicht. Jeder andere Schlüssel bleibt `start_form`.
2. **Prüfung** direkt nach Punkt 3.1 von DI12 (Form der Start-Struktur) und vor Punkt 3.2, in dieser Reihenfolge, jede Ablehnung mit `grund`, `code`
   und `text` wie bei `Starten`:
   - `orchestrator` ist kein Text in Kennungsform, oder `rolle` steht ohne `orchestrator` oder ist keine Kennung: `start_form`.
   - Die Kennung steht nicht in der `Sitzungsliste`: `orchestrator_unbekannt` („Orchestrator unbekannt“).
   - Die Sitzung ist beendet: `orchestrator_beendet` („Orchestrator ist beendet“).
   - Die Sitzung ist selbst ein Worker: `orchestrator_ist_worker` („Orchestrator ist selbst ein Worker“). Es gibt nur eine Ebene.
   - Der `kunde` der Start-Struktur ist ein anderer als der des Orchestrators (auch `null` gegen einen Kunden): `orchestrator_fremder_kunde`
     („Kunde weicht vom Orchestrator ab“). Grund: kein Übertrag zwischen Kundenräumen (A53).
3. **Nachricht `Agenten` im Lauf.** Der Raumkern hat keine Worker. Der Dienst schickt darum seine eigene Nachricht an der Stelle der des Raumkerns,
   wie bei der `Sitzungsliste` (DI12 Punkt 2):
   `{"art": "Agenten", "agenten": [{"worker", "sitzung", "rolle", "modell", "zustand", "nr", ("code")} …]}` in der Reihenfolge des Starts.
   `worker` ist die Kennung der Sitzung des Workers, `sitzung` die des Orchestrators, `modell` der Text der Start-Struktur, `zustand` `laeuft`
   oder `beendet` (mit `code` wie in der `Sitzungsliste`), `nr` die Rasternummer wie in der `Sitzungsliste`.
4. **Wann.** Beim Verbinden an der Stelle der Nachricht des Raumkerns (nach `Anfragen`, vor `Startwahl`). Danach nur, wenn sich die Liste der Worker
   geändert hat: Start oder Ende eines Workers, Löschen, Fortsetzen, neue Rasternummer. Sie kommt dann direkt nach der `Sitzungsliste` derselben
   Änderung. Ohne Worker ändert sich nichts: Die Liste bleibt `[]` wie die des Raumkerns, kein bestehender Fall ändert sich durch diesen Punkt.
5. `Agentenzustand` gibt es im Lauf nicht. Der Dienst schickt immer die ganze Liste.
6. Ein Worker zählt zu den 8 laufenden Sitzungen (DI12 Punkt 3.3). Das Ende des Orchestrators beendet keinen Worker.
7. **Wie ein Worker entsteht** (Nachtrag 27.11.2026, DI-E3): Im Pilot startet ein Mensch einen Worker im Fenster „Sitzungen“ mit Bezug auf
   einen Orchestrator, also mit `Starten` samt `orchestrator` und `rolle`. Fordert später ein Agent im Einschluss einen Worker über den Meldeweg
   von `lauf` an, ist das nur eine Anfrage an die Stelle; erst nach „frei“ startet der Dienst den Worker auf diesem Weg, und der Agent erreicht
   den Dienst nie direkt (Nachtrag „Kanal zur Stelle (L6, Nr. 98)“).

### DI-L2 Mehrere Abos je Betrachter (L2)

1. Ein `Abo` gilt je `sitzung`, nicht je Rasternummer. Grund: Die Rasternummer wird nach P25 neu vergeben, die Kennung nie. Die Rasterrahmen tragen
   die Nummer; welche Sitzung sie meint, sagt das Feld `nr` der letzten `Sitzungsliste` (Abschnitt 19 Punkt 8).
2. Ein Betrachter hält mehrere Abos zugleich, je sichtbarem Pane eines, jedes mit seiner Größe aus `Abo` und `Groesse`. So war es schon (DI12 Punkt 2
   und 7); dieser Punkt schreibt es fest. Es gelten `zustandskern` R-L2 Punkte 1 bis 6 je Kern der Sitzung.
3. **Die PTY einer Sitzung folgt nur dem Betrachter mit der letzten Eingabe in dieser Sitzung** (Vertrag `seite` SE2 Punkt 7). Eine Eingabe in einem
   Pane setzt die PTY dieses Panes auf die Größe, die der Betrachter für dieses Pane gemeldet hat. Die anderen Panes behalten ihre Größe.
4. **Je Raster:** Abzug, Folge der Stände, Bündelung, Rückstand, Dialog, Zwischenablage und Cursor, im Kern der Sitzung. **Je Betrachter
   gemeinsam:** die eine Verbindung mit einem Deflate-Kontext (`RA`), die eine Warteschlange und ihre Obergrenze (DI5) und `ausstehend`.
5. Eine eigene Obergrenze der Abos gibt es nicht. Sie ist durch die Rasternummern begrenzt: Ein `Abo` an auf eine Sitzung ohne Nummer ergibt
   `sitzung_ohne_nummer` (Abschnitt 19 Punkt 6), also höchstens 256 Abos je Betrachter.
6. `Loeschen` einer Sitzung beendet jedes Abo auf sie (DI-L3 Punkt 5).

### DI-L3 Umbenennen, Fortsetzen, Wiederbeleben, Löschen (L3)

Gemeinsam: Das Ereignis hat genau die genannten Schlüssel, `sitzung` ist Text. Sonst `Ablehnung <name>_form` (`umbenennen_form`, `loeschen_form`,
`fortsetzen_form`, `wiederbeleben_form`). Eine unbekannte Sitzung ergibt `sitzung_unbekannt`. Die Antwort ist die neue `Sitzungsliste` an alle,
dahinter `Agenten`, wenn sich die Liste der Worker geändert hat (DI-L1 Punkt 4). Eine Ablehnung geht nur an den Sender. `Umbenennen` und
`Loeschen` tragen in der Ablehnung nur `grund` (wie `Beenden`), `Fortsetzen` und `Wiederbeleben` `grund`, `code` und `text` (wie `Starten`).

1. **`Umbenennen`** `{"art": "Umbenennen", "sitzung": "<id>", "name": "<name>"}`. `name` ist ein Text mit 1 bis 64 Zeichen (Unicode-Skalarwerte),
   ohne Zeichen unter 0x20 und von 0x7f bis 0x9f, ohne Leerraum am Anfang und am Ende. Sonst, auch bei einem `name`, der kein Text ist,
   `name_ungueltig`. Erlaubt für laufende und beendete Sitzungen, auch für die Sitzung aus `kind`. Wirkung: `name` in der Liste und im Bestand
   (DI-L5). Ein Name muss nicht eindeutig sein.
2. **`Fortsetzen`** `{"art": "Fortsetzen", "sitzung": "<id>"}`. Prüffolge: Form; unbekannt: `sitzung_unbekannt`; die Sitzung läuft:
   `sitzung_laeuft`; sie ist schon fortgesetzt (`fortgesetzt_als` steht): `schon_fortgesetzt`; der Dienst hat keine Start-Struktur für sie (die
   Sitzung aus `kind`): `nicht_fortsetzbar`. Sonst gilt der Weg von `Starten` ab Punkt 3.1 mit der gespeicherten Start-Struktur und mit dem Orchestrator und der Rolle der
   alten Sitzung: dieselben Prüfungen mit den Einstellungen von jetzt, dieselbe Zahl, eine neue Kennung `s-<n>`, ein neuer Ordner, ein neues Kind.
   In `start.json` stehen dazu `sitzung` (die neue Kennung) und **`fortsetzung_von`** (die alte Kennung). Den Harness auf das alte Gespräch zu
   setzen, ist Sache von „Lauf starten“ (Vertrag `start`). Die alte Sitzung bekommt `fortgesetzt_als`, die neue `fortsetzung_von` und den Namen der
   alten. Antwort wie bei `Starten`: die Liste an alle, dann `Gestartet` an den Sender.
3. **`Wiederbeleben`** `{"art": "Wiederbeleben", "sitzung": "<id>"}`. Wie `Fortsetzen`, aber nur für eine abgestürzte Sitzung (`abgestuerzt` true,
   DI-L5 Punkt 2), sonst `nicht_abgestuerzt`. Prüffolge: Form, unbekannt, läuft (`sitzung_laeuft`), nicht abgestürzt, dann wie `Fortsetzen`.
4. **`Loeschen`** `{"art": "Loeschen", "sitzung": "<id>", "bestaetigung": "<id>"}`. Prüffolge: Form (Schlüssel nur `art`, `sitzung` und
   `bestaetigung`, `sitzung` ein Text); unbekannt: `sitzung_unbekannt`; die Sitzung aus `kind`: `nicht_loeschbar`; die Sitzung läuft:
   `sitzung_laeuft`; `bestaetigung` fehlt oder ist nicht genau die Kennung: `bestaetigung_fehlt`; die Sitzung ist Orchestrator eines laufenden
   Workers: `worker_laeuft`.
5. **Was `Loeschen` entfernt:** den Eintrag in der Liste und im Bestand mit Name, Ordner, Projekt, Code und gespeicherter Start-Struktur; den Kern und
   den Träger der Sitzung; jedes Abo auf sie (die Betrachter bekommen keinen Rahmen dieser Nummer mehr, die Nummer ist frei); die Einträge der beendeten
   Worker dieses Orchestrators; das Gespräch des Harness unter `lauf.gespraeche` für jeden dieser Einträge (Nachtrag 27.11.2026, DI-E4).
   **Was `Loeschen` nie entfernt:** den Arbeitsordner und alles darin; die Ordner und Dateien von „Lauf starten“ (`lauf.sitzungen`, `lauf.ziellisten`,
   `lauf.laufzeit`, Marker); die Protokollkette; den Zustand der Stelle und des Halters; Dateien außerhalb des Ordners des Dienstes, außer dem Gespräch
   nach DI-E4. Den Ordner der Sitzung im Ordner des Dienstes gibt es dann schon nicht mehr (DI12 Punkt 12). Die Kennung wird nie wieder vergeben.
   **Löschen ist endgültig:** Danach ergeben `Fortsetzen`, `Wiederbeleben`, `Umbenennen` und `Abo` `sitzung_unbekannt`.
   **Schutz gegen ein Versehen:** nur für beendete Sitzungen; die Bestätigung muss die Kennung wiederholen, die Ansicht schickt sie erst nach ihrer
   Rückfrage (Vertrag `seite`, B16); ein Ereignis löscht genau eine Sitzung, es gibt kein Sammellöschen.

### DI-L5 Sitzungsliste und Bestand (L5)

1. **Neue Felder** jedes Eintrags der `Sitzungsliste` im Lauf (DI12 Punkt 5):
   - `name`: der Name aus `Umbenennen`, sonst der Name aus der Start-Struktur, sobald der Vertrag `start` ihn führt (L10), sonst `""`.
   - `ordner`: `arbeitsordner` der Start-Struktur; `""` für die Sitzung aus `kind`.
   - `projekt`: der Anzeigename, der letzte Bestandteil von `ordner` (`/home/a/kunde-x` ergibt `kunde-x`); `""`, wenn `ordner` leer oder `/` ist.
   - `abgestuerzt` (Wahrheitswert): nur bei `beendet`, Regel in Punkt 2.
   - `fortsetzung_von` und `fortgesetzt_als` (Kennungen): nur, wenn sie bestehen (DI-L3).
   Die Reihenfolge der Einträge bleibt die des Starts; Einträge aus dem Bestand stehen vorn in ihrer gespeicherten Reihenfolge.
2. **`abgestuerzt`** ist wahr, wenn die Sitzung von selbst endete (nicht durch `Beenden`, nicht durch das Dienstende) und ihr Code nicht 0 ist, oder
   wenn sie beim Ende des vorigen Laufs des Dienstes noch lief (Punkt 4). Sonst ist es falsch. **Beim Dienstende** (SIGTERM, SIGINT) setzt der Dienst
   jede laufende Sitzung im Bestand auf `beendet` mit `grund` `dienst.ende` und `abgestuerzt` false und schreibt den Bestand, bevor er die Kinder
   beendet. Nur ein Dienst, der ohne geordnetes Ende stirbt, hinterlässt Einträge mit `laeuft`.
3. **Bestand.** Neuer optionaler Schlüssel `lauf.bestand` der Startdatei: ein absoluter Pfad, geprüft wie `lauf.einstellungen`. Der Elternordner muss
   beim Start nach DI2 Punkte 1 bis 3 geprüft sein (Ordner, kein Link, Besitzer der effektive Benutzer, Modus genau 0700), sonst Exitcode 3. Nach jeder
   Änderung der Liste (Start, Ende, `Umbenennen`, `Fortsetzen`, `Wiederbeleben`, `Loeschen`, Wegfall nach P26) schreibt der Dienst die Datei neu:
   eine neue Datei `<bestand>.neu.<pid>` exklusiv mit Modus 0600, ohne einem Link zu folgen, `fsync`, Umbenennen auf `<bestand>`, `fsync` des
   Ordners. Ein Rest `<bestand>.neu.<pid>` eines abgestürzten Laufs stört nicht; der Dienst löscht ihn nicht.
   Gelingt das nicht, bleibt die alte Datei, und der Dienst schreibt eine Zeile `wb-dienst: bestand nicht geschrieben: <code>` nach stderr, ohne
   Inhalt. Inhalt: `{"fassung": 1, "gestartet": <Zahl der vergebenen Kennungen>, "sitzungen": [{"id", "harness", "modell", "route", "kunde",
   "name", "ordner", "projekt", "zustand", ("code"), ("grund"), ("abgestuerzt"), ("orchestrator"), ("rolle"), ("fortsetzung_von"),
   ("fortgesetzt_als"), "start"} …]}` mit `start` als Start-Struktur der Anfrage. Nie darin: Zeilen, Titel, Eingaben, Zwischenablage, Pfade der
   Startdatei. Höchstens 256 beendete Einträge wie in der Liste (P26) und höchstens 4 194 304 Byte.
4. **Nach einem Neustart** liest der Dienst den Bestand einmal beim Start. Er muss eine reguläre Datei sein, kein Link, mit Besitzer der effektive
   Benutzer und Modus 0600, mit höchstens 4 194 304 Byte und gültiger Form; sonst Exitcode 2 mit einer Zeile (fail closed: der Dienst ersetzt nie eine
   Datei, die er nicht lesen kann). Fehlt die Datei, ist die Liste leer. Jeder Eintrag kommt als `beendet` in die Liste, mit `nr` `null` (das Bild ist
   weg; ein `Abo` ergibt `sitzung_ohne_nummer`). Ein Eintrag, der `laeuft` war, wird `beendet` mit `abgestuerzt` true, ohne `code`, mit `grund`
   `dienst.neustart`. `dienst.gestartet` ist danach das Größere aus `gestartet` und der höchsten Nummer der Kennungen; DI12 Punkte 13 und 15 gelten
   weiter. Eine Start-Struktur aus dem Bestand durchläuft bei `Fortsetzen` alle Prüfungen von `Starten`; der Bestand gibt keinem Inhalt mehr Recht als
   eine neue Anfrage. Der Kern einer Sitzung aus dem Bestand gilt als beendet: Er lehnt Eingaben mit `sitzung_beendet` ab.
   Reihenfolge der Exitcodes: Der Dienst liest den Bestand nach der Prüfung der Rechte (DI2). Ein ungültiger Bestand in einem Ordner mit falschen
   Rechten ergibt also 3, nicht 2.

### DI-L6 Freigabe im Lauf (L6)

1. **Stand vor diesem Nachtrag (Fall `dil6_heute` in `faelle/`):** Im Lauf geht `Freigabe` an den Kern der genannten Sitzung (DI12 Punkt 7). Ohne `sitzung` antwortet
   der Dienst `sitzung_unbekannt`. Der Kern einer Sitzung bekommt nie ein Ereignis `Anfrage`, denn der Dienst hat keine Verbindung zur Stelle, und er
   gibt `an_stelle` nicht weiter (Abschnitt 5, letzter Punkt). Eine Freigabe erreicht also weder die Stelle noch den Hub noch den Halter.
2. **Neuer Schlüssel `lauf.stelle`** der Startdatei: `{"kanal": "<Kanalname>", "ordner": "/abs/pfad"}` (`ordner` nur unter Unix). Der Dienst spricht
   die Stelle als Client von `peerkanal` mit der erwarteten Kennung des eigenen Benutzers an und ist für sie der Aufrufer `mensch` (Vertrag `stelle`,
   Abschnitt 3).
3. **Anfragen holen.** Solange ein Betrachter verbunden ist, fragt der Dienst je Sekunde `mcp.offene`. Jede offene Anfrage, deren `sitzung` eine Sitzung
   dieses Dienstes ist und die der Kern noch nicht kennt, gibt er dem Kern dieser Sitzung als Ereignis `Anfrage` mit `id`, `sitzung`, `werkzeug`,
   `vorschau` und `frist` (die Frist in der Uhr des Dienstes: jetzt plus der Rest bis `frist_bis_ms`).
4. **`Freigabe` im Lauf** `{"art": "Freigabe", "anfragen": [<id>], "nachweis", ("entscheidung"), ("begruendung")}`, ohne `sitzung`; jeder andere
   Schlüssel, auch `sitzung`, und eine Liste `anfragen`, die nicht nur Texte trägt, ergeben `ereignis_ungueltig`. Prüffolge: die
   Form nach `zustandskern` R-L6 Punkt 2 (dieselben Gründe); Anzahl (`sammelfreigabe`); Person mit `art: mensch` (`agent_kennung`); `nachweis` fehlt
   (`nachweis_fehlt`) oder ist kein Text (`nachweis_ungueltig`); die Anfrage kennt kein Kern dieses Dienstes (`anfrage_unbekannt`); anderer
   Kundenraum (`fremder_kundenraum`); nicht offen (`nicht_offen`, `abgelaufen`). Dann `mcp.entscheiden` mit `anfrage`, `entscheidung` und `nachweis`.
   Die Begründung geht nicht an die Stelle; ihre Eingabe ist geschlossen (Vertrag `stelle`, Abschnitt 4).
5. **Antwort der Stelle.** `erteilt` oder `abgelehnt`: Der Dienst gibt dem Kern der Sitzung `AnfrageStand` mit diesem Zustand und der Begründung des
   Ereignisses (`zustandskern` R-L6 Punkt 6); der Raum bekommt die Nachricht `Anfrage`, das ist die Zeile im Guard-Verlauf. Eine Ablehnung der Stelle
   geht als `Ablehnung {"grund": "stelle_abgelehnt", "code": "<code der Stelle>"}` nur an den Sender; der Kern bleibt unverändert. Ist `lauf.stelle`
   nicht gesetzt oder die Stelle nicht erreichbar, ist die Antwort `stelle_nicht_erreichbar` (fail closed: nichts ändert sich).
6. **Ende zu Ende** (Fall `dil6_ende_zu_ende`, ausstehend): Anfrage der Stelle, `Anfragen` an den Betrachter, `Freigabe` mit `erteilen`,
   `mcp.entscheiden` an einer Attrappe der Stelle im Prüfclient, Nachricht `Anfrage` mit `erteilt`; eine zweite Anfrage mit `ablehnen` und
   Begründung, Nachricht `Anfrage` mit `abgelehnt` und Begründung. Der Fall braucht die Form der Zeilen auf dem Kanal der Stelle. Die legt die
   Transportschicht der Stelle fest (Vertrag `stelle`, Abschnitt 1, „Transportschicht“), und sie ist noch nicht geschrieben. Bis dahin prüft der
   Fall `dil6_freigabe_lauf` die Prüffolge bis `anfrage_unbekannt`. Der Weg danach (Hub löst ein, Halter führt aus) gehört den Verträgen `stelle`
   und `halter`.

### DI-L11 Lokaler Weg für Betrachter ohne Brücke (L11)

Sicherheitsstelle: das Projekt liest sie selbst.

1. **Schlüssel `lokal`** der Startdatei, optional, in beiden Betriebsarten: `{"kanal": "<Kanalname>"}`, Kanalname nach `peerkanal`
   (`^[a-z0-9][a-z0-9._-]{0,63}$`). Unter Linux und macOS ist der Weg der Unix-Socket `<socket.ordner>/<kanal>.sock` (höchstens 100 Byte wie DI1,
   nicht gleich `socket.name`), unter Windows die Named Pipe `\\.\pipe\<kanal>`. Ohne `lokal` gibt es keinen lokalen Weg; alles bleibt wie bisher.
2. **Bau auf `peerkanal` Fassung 1** (`rust/spec/peerkanal/README.md`), als Server mit genau einer Erlaubnis: unter Unix `{"uid": <effektive uid des
   Dienstes>}` ohne `gid`, unter Windows `{"sid": <eigene SID>, "appcontainer_sid": null}`. Dazu `instanzen` 8, `hallo_frist` 2 s, `prozess_pflicht`
   true, unter Unix Modus 0600. Nach dem Hallo spricht die Verbindung das Protokoll dieses Vertrags (DI3 bis DI12); jede Verbindung ist ein Betrachter
   `b<n>` mit demselben Zähler wie `dienst.sock`, `ansicht` ist `fenster`.
3. **Rechte.** Unix: der Ordner wie DI2 Punkte 1 bis 3 (0700, Besitzer der effektive Benutzer, kein Link); der Socket mit Modus genau 0600 und dem
   Besitzer des Dienstes; besteht der Pfad schon, endet der Start mit Exitcode 3 (`pk.name_belegt`). Windows: Besitzer der Pipe ist die eigene SID,
   die DACL ist geschützt und hat genau einen Eintrag `(A;;0x12019F;;;<eigene SID>)`; kein Eintrag für `Everyone`, `Authenticated Users`, `Users`,
   `Administrators`, `INTERACTIVE`, `NETWORK`, `ANONYMOUS` oder einen AppContainer; kein Label; `PIPE_REJECT_REMOTE_CLIENTS` und
   `FILE_FLAG_FIRST_PIPE_INSTANCE`. Ist der Name belegt, endet der Start mit Exitcode 3: Ein Prozess, der den Namen vorher anlegt, bekommt nie eine
   Verbindung des Dienstes, und die Ansicht prüft den Server mit der erwarteten Kennung des eigenen Benutzers (peerkanal, Client).
4. **Das Konto kommt allein aus dem Verbindungspartner.** Unix: die uid aus `SO_PEERCRED` (Linux) oder `LOCAL_PEERCRED`, also `getpeereid` (macOS),
   gemessen vom Kernel beim `connect`. Windows: `TokenUser` des Tokens des Clients über `ImpersonateNamedPipeClient` und `OpenThreadToken`.
   Der Dienst bildet daraus einmal je Verbindung, vor dem ersten Rahmen, den Wert **`wer`**: `{"system": "linux"|"macos"|"windows", "uid": <Zahl>
   oder null, "sid": "<SID>" oder null, "name": "<Konto>"}`. `name` ist unter Unix `pw_name` aus `getpwuid_r(uid)`, unter Windows `DOMÄNE\name` aus
   `LookupAccountSidW`. `wer` ändert sich während der Verbindung nie.
   **Vom Konto zum Pseudonym (L4).** Die Prüfung auf Admin und `sicht` (Vertrag `regelebenen`, Abschnitt 15.2) bekommen nie das Konto, sondern dessen
   Pseudonym. Der Dienst bildet aus `wer` die Kennung der Person: unter Unix `uid-<uid>` (Dezimalzahl), unter Windows `sid-<SID in Kleinbuchstaben>`;
   beide haben die Form einer Kennung nach Vertrag 2. Mit dieser Kennung fragt er die Stelle nach dem Pseudonym (`person(kennung, …)`, Vertrag
   `personenbezug` Abschnitt 4) und gibt `setzen` und `sicht` dieses Pseudonym als `wer`. Die Kontenliste der Admins aus der Firmenvorgabe löst er
   beim Laden ebenso auf: Name zu uid oder SID über das System, dann Kennung, dann Pseudonym; die Menge ist `stand.admins`. Ein Name, den das System
   nicht kennt, ergibt kein Pseudonym und keinen Admin (fail closed). Grund für uid und SID statt des Namens: Ein Name lässt sich umbenennen oder
   doppelt vergeben, die Kennung des Systems nicht. `wer` steht in keiner Nachricht an die Ansicht, in keiner Datei der Ebenen und in keiner
   Protokollzeile; nur die Antwort auf `Zustand` im Prüfmodus nennt `wer` (DI9). Konten, die ein Admin eingibt, stehen nur in der Firmenvorgabe und
   in `Einstellungen` an einen Admin (`regelebenen` 15.12 und 15.13; Abschnitt „L4 und L10 im Dienst“ Punkt 6 und 7).
   **Admin hängt allein am Verbindungspartner.** Ob ein Betrachter Admin ist, entscheidet der Dienst nur mit dem Pseudonym aus `wer` des lokalen Wegs.
   Kein Feld eines Ereignisses, keine Startdatei und keine Angabe der Ansicht macht einen Betrachter zum Admin. Ein Betrachter über `dienst.sock`
   hat keinen gemessenen Partner (DI2 Punkt 8, Punkt 7 unten); er ist nie Admin, auch wenn das eigene Konto des Dienstes in der Kontenliste steht.
5. **Verwerfen eines fremden Nutzers.** Das System lässt ihn gar nicht verbinden (Ordner 0700, DACL); dann gibt es kein Ereignis. Erreicht er den
   Server doch (etwa als root, mit `SeBackupPrivilege` oder durch eine spätere Zugriffsliste), verwirft `peerkanal` ihn mit `pk.kennung_nicht_erlaubt`
   und dem Grund `pk.uid_nicht_erlaubt` (Unix) oder `pk.sid_nicht_erlaubt` (Windows); ein Prozess in einem AppContainer mit `pk.appcontainer_falsch`,
   ein Token mit Restricting SIDs mit `pk.eingeschraenkt`. Im Haken des Aufrufers (peerkanal Stufe 7) verwirft der Dienst außerdem mit
   `aufrufer:konto-unbekannt` (kein Name zur uid oder SID), unter Linux mit `aufrufer:einschluss` (die Cgroup des Partners liegt in der Einheit eines
   Laufs, peerkanal Fassung 2) und unter macOS mit `aufrufer:sandbox` (Seatbelt). Je Verwerfen schreibt der Dienst eine Zeile
   `wb-dienst: lokal abgewiesen: <grund>` nach stderr, ohne Namen, uid oder SID.
6. **Fail closed.** Lässt sich der Partner nicht bestimmen (`pk.peer_unmessbar`, `pk.prozess_gewechselt`, ein ungültiges Hallo), gibt es keine
   Verbindung, kein `Verbinden` an den Kern und keine Nachricht. Ohne `wer` erreicht keine Verbindung des lokalen Wegs den Kern.
7. **Wege, auf denen eine Angabe der Ansicht doch zum Konto würde, und ihr Schluss:**
   - Ein Feld im Ereignis: Nennt ein Ereignis `konto`, `wer`, `person`, `uid` oder `sid`, ist das der Fehler `feld_verboten` (DI3, wie `t` und
     `betrachter`), auf beiden Wegen. `bereich` in `EinstellungSetzen` ist ein Wunsch, kein Recht; ob er gilt, entscheidet `regelebenen` mit `wer`.
   - Die Startdatei: `betrachter.person` ist die Person des Kerns, nie das Konto. `wer` kommt nie aus der Startdatei, nie aus der Umgebung (`USER`,
     `LOGNAME`, `USERNAME`) des Dienstes oder der Ansicht, nie aus dem Namen von Socket oder Pipe und nie aus dem Hallo.
   - Eine Verbindung ohne echtes Token: Öffnet ein Client die Pipe mit `SECURITY_ANONYMOUS` oder `SECURITY_IDENTIFICATION` ohne Benutzer, scheitert
     `OpenThreadToken` oder liefert keinen `TokenUser`; das ist `pk.peer_unmessbar`, keine Verbindung.
   - Ein Name statt einer Kennung: Das Pseudonym entsteht aus uid oder SID, nie aus dem Namen; die Kontenliste der Admins wird beim Laden zu uid
     oder SID aufgelöst (Punkt 4). So gibt ein zweiter Eintrag mit demselben Namen kein Recht.
   - Ein Agent mit derselben uid (Linux unter `bwrap`): Der Ordner des Dienstes wird nie in einen Einschluss gebunden (Vertrag `einschluss`), und der
     Haken verwirft einen Partner in der Einheit eines Laufs (Punkt 5). Unter macOS verwirft der Haken einen Partner unter Seatbelt, unter Windows
     verwirft `peerkanal` einen AppContainer.
   - Eine weitergereichte Verbindung: Die Kennung ist die des Prozesses, der `connect` aufrief. Reicht ein Prozess des Kontos seine Verbindung an einen
     anderen Prozess weiter, handelt dieser als das Konto. Das kann nur das Konto selbst; der Dienst kann es nicht verhindern.
   - Die Brücke: `dienst.sock` hat keine Peer-Prüfung (DI2 Punkt 8). Für Betrachter über `dienst.sock` ist `wer` das eigene Konto des Dienstes
     (`geteuid` oder das eigene Token), denn nur dieses Konto erreicht den Socket. Wer über die Brücke kommt, handelt also als dieses Konto; die
     Anmeldung an der Brücke ist dort die Grenze (Vertrag `bruecke`).
8. **Starten über den lokalen Weg** (das Gegenstück zu `POST /starten` der Brücke, Vertrag `bruecke`): Das App-Fenster schickt auf seiner
   Verbindung das Ereignis `Starten` von DI12 Punkt 3 mit der Start-Struktur in der Fassung, die das Crate `start` liest (Fassung 2 mit `effort`
   und `name`, sobald der Vertrag `start` sie führt; Abschnitt „L4 und L10 im Dienst“). Die Antwort geht nur an den Sender:
   - angenommen: erst die neue `Sitzungsliste` an alle, dann `Gestartet {"sitzung": "<id>"}` (Rahmen `Nachricht`); das entspricht 200
     `{"sitzung": "<id>"}` der Brücke;
   - abgelehnt: `Ablehnung {"grund", "code", "text"}` (Rahmen `Ablehnung`, DI12 Punkt 3); das entspricht 422 der Brücke mit denselben Werten.
   Zuordnung ohne neues Feld: Der Dienst verarbeitet die Ereignisse einer Verbindung der Reihe nach, und jedes `Starten` bekommt genau eine Antwort.
   Die Ansicht hält je Verbindung höchstens ein `Starten` offen; die nächste `Gestartet` oder die nächste `Ablehnung` mit `code` ist dann seine
   Antwort. Andere Ablehnungen tragen kein `code` (DI12 Punkt 3). Fall: `dil11_lokal_starten`; Mutant `lokal_ohne_starten`.
9. **`Abo` bekommt kein Feld `nr`** (DI-L2 Punkt 1). Das Fenster abonniert je `sitzung` und ordnet die Rasterrahmen über `nr` der letzten
   `Sitzungsliste` zu. Ein `Abo` mit `nr` ist wie jedes `Abo` zu behandeln: Das Feld bleibt ohne Wirkung, wie jeder unbekannte Schlüssel eines
   Ereignisses der Ansicht, den der Kern nicht liest (hergeleitet aus `VT` Abschnitt 4; kein Fall).
10. Unter Windows steht der Code des Dienstes für den lokalen Weg in `src/sys/windows.rs` (Abschnitt 21 Punkt 17); `peerkanal` kapselt die Aufrufe von
   Win32 selbst. Abschnitt 21 Punkt 2 (`dienst.sock` als `AF_UNIX`) gilt weiter. Grund für die Named Pipe beim lokalen Weg: Nur sie gibt das Token des
   Clients; `SIO_AF_UNIX_GETPEERPID` nennt nur eine pid, und eine pid lässt sich neu vergeben.

### L4 und L10 im Dienst (Verweis)

Die Felder legen `rust/spec/regelebenen/README.md` Abschnitt 15 und `rust/spec/start/README.md` „Fassung 2: `effort` und `name`“ fest (Stand vom
26.11.2026, bis zu ihrem Merge der Zweig der beiden Verträge). Der Dienst reicht sie unverändert durch; hier steht nur, wo er sie berührt.
1. **`EinstellungSetzen` im Lauf** fällt aus `nicht_im_lauf` heraus (DI12 Punkt 7, Punkt 11 (4)) und bleibt in `ERLAUBT` (DI3). Es geht nicht an
   einen Kern: Der Dienst prüft die Form nach `regelebenen` 15.9 und ruft `setzen` mit dem Pseudonym als `wer` (DI-L11 Punkt 4). Eine Ablehnung ist
   `{"grund": <Code aus setzen>}`, unverändert; eine eigene Ablehnung je Ebene gibt es nicht. Ein Feld `konto`, `wer`, `person`, `uid` oder `sid`
   ist vorher schon `feld_verboten` (DI-L11 Punkt 7). `RaumWechsel` bleibt im Lauf `nicht_im_lauf`.
2. **`Einstellungen` im Lauf** ist Fassung 2 nach `regelebenen` 15.10 und kommt vom Dienst, nicht vom Raumkern (DI12 Punkt 2): beim Verbinden je
   Betrachter an der Stelle der Nachricht des Raumkerns, nach einer angenommenen Änderung an jeden Betrachter, dessen `sicht` sich ändert.
3. **`Startwahl`** ist Fassung 2 nach `start` „Nachricht `Startwahl` mit Efforts“ und kommt **je Betrachter**, weil `vorgabe` vom Betrachter
   abhängt. DI12 Punkt 10 gilt so: Jeder Betrachter bekommt beim Verbinden seine `Startwahl` und danach eine neue, sobald sich sein eigener Inhalt
   ändert, nicht mehr eine gemeinsame für alle.
4. **Start-Struktur Fassung 2:** Der Dienst prüft weiter mit dem Crate `start`, dazu Stufe 18 (`start.effort_grenze`). `name` der Start-Struktur ist
   die Vorgabe von `name` in der Liste (DI-L5 Punkt 1).
5. Bestehende Fälle, die das ändert: `di12_lauf_ereignisse` (dort ist `EinstellungSetzen` im Lauf heute `nicht_im_lauf`) und die Fälle mit
   `Einstellungen` oder `Startwahl` im Lauf. Ihre neuen Erwartungen entstehen mit dem Bau von L4 und L10, nicht in diesem Nachtrag.
6. **Schreibweg der Firmenvorgabe** nach `regelebenen` 15.12: ein `EinstellungSetzen` mit `ebene` `firma` und `schluessel` `leseklassen`. Der
   Dienst schreibt die Datei nie selbst; er reicht die Änderung an den Schreibhelfer weiter (Nachtrag 27.11.2026, DI-E1). Die Ansicht schreibt sie
   nie. Stufe 0 nimmt das Konto aus `wer` des lokalen Wegs (DI-L11 Punkt 4),
   Stufe 1 prüft das Pseudonym gegen `stand.admins` vor jedem Zugriff auf die Datei. Ein Betrachter über `dienst.sock` scheitert an Stufe 1 mit
   `ebenen.nicht_befugt` (DI-L11 Punkt 4). Die Ablehnung ist `{"grund": <Code>}` mit genau einem Code der ersten verletzten Stufe, unverändert.
   Zu den Ablehnungsgründen von DI12 Punkt 3 kommen damit die festen Codes `firmenvorgabe.*` aus `regelebenen` 15.12; welcher Code wo entsteht,
   steht in DI-E1 Punkt 6 (Nachtrag 27.11.2026).
   Nach `firmenvorgabe.zustand_unklar` sperrt der Dienst bis zum Neuladen der Firmenvorgabe: Jedes weitere `EinstellungSetzen` bekommt
   `{"grund": "firmenvorgabe.zustand_unklar"}`, und es entsteht kein Tor. Andere Ereignisse bleiben unberührt. Die Sperre bleibt so, und Neuladen
   heißt im Pilot Neustart des Dienstes (Nachtrag 27.11.2026, DI-E2). Die Fälle des Schreibwegs führt der Vertrag `regelebenen` (Abschnitt 15.12,
   Ordner `faelle-schreiben`); der Bau des Dienstes übernimmt sie als Außentest.
7. **Konto in `Einstellungen`** nach `regelebenen` 15.13: Der Dienst baut die Nachricht aus `sicht(stand, wer)`. Nur für einen Admin nach Punkt 6
   setzt er in `werte` das Konto aus der Zuordnung an die Stelle des Pseudonyms; für einen Nichtadmin ersetzt er nie etwas. Die Liste der Admins
   steht in keiner Nachricht. Eine Protokollzeile nennt nie ein Konto, nur Pseudonyme.
8. **Bau des Tors:** Der Dienst übergibt `leseklassen` nach der Zuordnung Konto zu Pseudonym an `Personen::neu` (Vertrag `datentor`). Ein Konto
   erreicht das Tor nie. Scheitert `Personen::neu`, entsteht kein Tor.

### Gegenproben des Nachtrags

| Mutant | Regel | Änderung |
|---|---|---|
| `worker_orchestrator_ungeprueft` | DI-L1 | ein unbekannter Orchestrator wird angenommen |
| `worker_fremder_kunde` | DI-L1 | ein Worker mit anderem Kunden als sein Orchestrator wird angenommen |
| `agenten_vom_raumkern` | DI-L1 | im Lauf geht die leere Liste des Raumkerns hinaus statt der Liste der Worker |
| `umbenennen_ohne_pruefung` | DI-L3 | ein Name mit Steuerzeichen oder mehr als 64 Zeichen wird angenommen |
| `loeschen_laufend` | DI-L3 | eine laufende Sitzung lässt sich löschen |
| `loeschen_ohne_bestaetigung` | DI-L3 | `Loeschen` ohne passende Bestätigung löscht |
| `loeschen_kennung_wieder` | DI-L3 | nach dem Löschen bekommt der nächste Start die gelöschte Kennung |
| `fortsetzen_laufend` | DI-L3 | eine laufende Sitzung lässt sich fortsetzen |
| `fortsetzen_ohne_verweis` | DI-L3 | `start.json` trägt kein `fortsetzung_von` |
| `wiederbeleben_ohne_absturz` | DI-L3 | eine Sitzung mit Code 0 lässt sich wiederbeleben |
| `liste_ohne_ordner` | DI-L5 | die Liste trägt `ordner` und `projekt` nicht |
| `projekt_ganzer_pfad` | DI-L5 | `projekt` ist der ganze Pfad statt seines letzten Bestandteils |
| `bestand_vergessen` | DI-L5 | der Dienst schreibt den Bestand nicht |
| `bestand_laeuft_bleibt` | DI-L5 | ein laufender Eintrag des Bestands bleibt nach dem Neustart `laeuft` |
| `konto_aus_ereignis` | DI-L11 | ein Feld `konto` im Ereignis wird angenommen statt `feld_verboten` |
| `lokal_ohne_peerpruefung` | DI-L11 | der lokale Weg nimmt eine Verbindung an, deren Partner nicht gemessen ist |
| `lokal_ohne_starten` | DI-L11 | `Starten` über den lokalen Weg ist der Fehler `art_nicht_erlaubt` |

DI-L2 hat hier keinen Mutanten: Die Größe je Sitzung entscheidet der Kern, die Gegenproben dazu stehen im Vertrag `zustandskern` (R-L2).
DI-L6 bekommt seinen Mutanten mit dem Fall `dil6_ende_zu_ende`.

### Prüfmittel des Nachtrags

Der Prüfclient kennt drei neue Angaben (Nachtrag 26.11.2026):
- Fall-Feld `bestand` `{"vorher": <JSON>, "modus": "0600", "ordner_modus": "0700"}`: Ordner und Datei von `lauf.bestand` im Wegwerf-Ordner, Platzhalter
  `@bestand`. `ende.bestand` ist die Datei nach dem Ende (Pfade des Wegwerf-Ordners als `@ordner`), dazu `bestand_modus` und `bestand_rest` (andere
  Einträge im Ordner).
- Schritt `Verbinden` mit `weg: "lokal"` und optional `hallo` (Hex, Vorgabe das Hallo von `peerkanal`): verbindet über den lokalen Weg; ohne Antwort
  des Dienstes ist das Ergebnis `{"ok": false, "fehler": "abgewiesen"}`. `ende.lokal_entfernt` und `ende.lokal_protokoll` (die Gründe der Zeilen
  `wb-dienst: lokal abgewiesen: <grund>`).
- In der Antwort auf `Zustand` trägt ein Betrachter des lokalen Wegs `dienst.betrachter.<n>.wer`. Der Prüfclient setzt dort für das Konto des
  Laufs `@uid`, `@konto` und `@system` ein, so dass die Erwartung auf jedem Rechner gilt. Unter Linux läuft der Dienst im Prüfclient in einem
  Nutzer-Namensraum (Abschnitt 16 Punkt 6); dort ist das eigene Konto uid 0 mit dem Namen `root`, und der Prüfclient vergleicht damit (gemessen am
  27.11.2026 auf dem Linux-Baurechner). Betrachter über `dienst.sock` haben kein `wer` im Zustand.

Ein fremder Nutzer am lokalen Weg ist im Fall nicht herstellbar (er bräuchte ein zweites Konto). Diese Ablehnung prüft der Bau mit einem Einheitstest
von `peerkanal` (`pk.uid_nicht_erlaubt`, `pk.sid_nicht_erlaubt`) und einem Test des Dienstes, der den Haken mit einem fremden Wert ruft.

**Geänderte Erwartungen.** 59 sichtbare und 59 verdeckte Fälle bekommen eine neue Erwartung. 58 je Satz unterscheiden sich nur in den neuen
Feldern der `Sitzungsliste` (DI-L5 Punkt 1); `di12_lauf_ereignisse` bekommt für die `Freigabe` mit dem fremden Schlüssel `wahl` jetzt
`ereignis_ungueltig` statt `sitzung_unbekannt` (DI-L6 Punkt 4). Gemessen mit der Referenz: Ohne diese Felder sind alte und neue Erwartung gleich.
Die neuen Erwartungen liegen in `erwartet-ausstehend/`; `faelle/` und `erwartet/` bleiben, bis der Bau sie ersetzt.

## Nachtrag 27.11.2026: Entscheide zu Firmenvorgabe, Sperre, Worker-Anforderung und Löschen

Grundlage: Entscheide des Betreibers, 27.11.2026, zu vier offenen Fragen des Nachtrags „Oberfläche L1, L2, L3, L5, L6, L11“. Die neuen Regeln
heißen DI-E1 bis DI-E4. Wo dieser Abschnitt nichts sagt, gilt jener Nachtrag unverändert. Stellen dort, die ein Entscheid ersetzt, tragen den
Hinweis „(Nachtrag 27.11.2026)“; die Liste steht am Ende dieses Abschnitts. Die Fälle liegen wie dort in `faelle-ausstehend/` und
`erwartet-ausstehend/`.

### DI-E1 Schreibweg der Firmenvorgabe über den Schreibhelfer (Entscheid 1)

1. **Wer schreibt.** Der Dienst bleibt unter dem Konto des Nutzers (DI2) und schreibt die Firmenvorgabe nie, auch nicht mit einem anderen
   Werkzeug. Er reicht die Änderung eines Admins nur an den Schreibhelfer weiter. Der Helfer ist ein eigener kleiner Prozess mit eigenem
   Systemnutzer, nicht root, und wird über einen systemd-Socket gestartet. Er kann genau eine Sache: die Firmenvorgabe atomar ganz ersetzen,
   nachdem er über `SO_PEERCRED` geprüft hat, dass der Partner Admin der gerade gültigen Datei ist. Die Datei gehört dem Helfernutzer, hat
   Modus 0644 und liegt in einem Ordner, dessen Eltern root gehören; die IT pflegt sie weiter mit `sudo`. Helfer, Datei, Rechte und Codes des
   Helfers legen `regelebenen` Abschnitt 15.12 und der Vertrag `paket` fest. Dieser Abschnitt nennt nur den Weg des Dienstes zum Helfer.
2. **Startdatei.** Zwei neue optionale Schlüssel in `lauf`, in beiden Betriebsarten, je ein absoluter Pfad, geprüft wie `lauf.bestand`
   (Text von 1 bis 4096 Byte, ohne NUL; sonst Exitcode 2 mit der Zeile `lauf.<schlüssel> muss ein absoluter Pfad sein`):
   - `firmenvorgabe`: die Datei der Firmenvorgabe (das Paket schreibt unter Linux `/etc/werkbank/vorgabe/firmenvorgabe.json`);
   - `vorgabe_helfer`: der Socket des Helfers (das Paket schreibt unter Linux `/run/werkbank-vorgabe/vorgabe.sock`).
   Der Dienst liest beide nur aus seiner Startdatei. Ohne `firmenvorgabe` gibt es keine Firmenvorgabe und keinen Admin. Ohne `vorgabe_helfer`
   endet jede Änderung mit `firmenvorgabe.helfer_nicht_erreichbar`; das gilt auch unter macOS und Windows, solange es dort keinen Helfer gibt.
3. **Ablauf im Dienst** für ein `EinstellungSetzen` mit `ebene` `firma` und `schluessel` `leseklassen` („L4 und L10 im Dienst“ Punkt 6,
   `regelebenen` 15.12.3). Die erste verletzte Stufe gibt genau einen Code:
   - Stufe 0: Konto aus `wer` des lokalen Wegs (DI-L11 Punkt 4), Pseudonym über die Stelle, Zuordnung jedes Kontos im Wert.
   - Stufe 0a: Der Dienst ist gesperrt (DI-E2 Punkt 4): `firmenvorgabe.zustand_unklar`. Er spricht den Helfer nicht an.
   - Stufe 1: Das Pseudonym steht nicht in `stand.admins`: `ebenen.nicht_befugt`. Er spricht den Helfer nicht an. Ein Betrachter über
     `dienst.sock` endet immer hier (DI-L11 Punkt 4), ebenso jeder Betrachter, wenn die Firmenvorgabe beim Start nicht geladen ist (DI-E2).
   - Stufe 2: `setzen` auf einer Kopie des Stands lehnt ab: dessen Code, etwa `ebenen.wert_ungueltig`.
   - Stufe 3: `connect` an `lauf.vorgabe_helfer` scheitert: `firmenvorgabe.helfer_nicht_erreichbar`. Der Dienst hat nichts gesendet.
   - Stufe 4: Anfrage und Antwort nach Punkt 4 und 5.
4. **Was der Dienst an den Helfer gibt.** Eine Verbindung, ein JSON-Objekt, dann `shutdown(SHUT_WR)`:
   `{"art": "Ersetzen", "fassung": 1, "basis": "<sha256 hex klein der zuletzt geladenen Bytes>", "inhalt": <die ganze neue Firmenvorgabe>}`.
   `inhalt` ist der zuletzt geladene Inhalt, in dem nur `leseklassen` den neuen Wert in der Kontoform trägt, in der der Admin ihn eingegeben hat.
   Der Dienst sendet nie ein Pseudonym, nie `wer` und nie das Konto des Partners: Der Helfer misst seinen Partner selbst und vertraut dem Dienst
   nicht. Partner des Helfers ist der Prozess des Dienstes, also das Konto des Nutzers, das in der Kontenliste der gültigen Datei stehen muss.
5. **Wie der Dienst die Antwort weiterreicht.** Er wartet höchstens 10 s ab dem Verbinden auf genau ein JSON-Objekt mit genau `code` und
   `sha256`.
   - `{"code": null, "sha256": "<64 Zeichen hex klein>"}`: Der Dienst übernimmt den gesendeten Inhalt als neuen Stand und `sha256` als neue
     Basis. Er schreibt die Zeile `regel.geaendert` aus `setzen` und schickt jedem Betrachter, dessen `sicht` sich ändert, eine neue Nachricht
     `Einstellungen` („L4 und L10 im Dienst“ Punkt 2). Erst dann wirkt die Änderung, auch im Tor.
   - `{"code": "<Code>", "sha256": null}` mit einem Code der Form `firmenvorgabe.<name>` (`name` aus `[a-z_]`): `Ablehnung {"grund": <Code>}`
     nur an den Sender, der Code unverändert. Der Stand bleibt; die Zeile heißt `regel.abgelehnt`. Bei `firmenvorgabe.zustand_unklar` sperrt der
     Dienst zusätzlich (DI-E2 Punkt 4).
   - Keine Antwort in der Frist, eine abgebrochene Verbindung nach dem Senden oder eine Antwort in anderer Form: Der Dienst kennt den Ausgang
     nicht. Er antwortet `{"grund": "firmenvorgabe.zustand_unklar"}` und sperrt (DI-E2 Punkt 4).
6. **Wo die Codes entstehen.**

   | Ort | Codes |
   |---|---|
   | Dienst, bei der Änderung | `firmenvorgabe.zustand_unklar` (Stufe 0a, Stufe 4 ohne gültige Antwort), `ebenen.nicht_befugt` (Stufe 1), Codes von `setzen` (Stufe 2), `firmenvorgabe.helfer_nicht_erreichbar` (Stufe 3) |
   | Dienst, beim Start (DI-E2), nur Zeile und Zustand, keine Ablehnung | `firmenvorgabe.datei_unsicher`, `firmenvorgabe.ladefehler` |
   | Helfer, vom Dienst unverändert durchgereicht | `firmenvorgabe.partner_unbekannt`, `firmenvorgabe.partner_eingeschlossen`, `firmenvorgabe.datei_unsicher`, `firmenvorgabe.ladefehler`, `firmenvorgabe.partner_nicht_admin`, `firmenvorgabe.anfrage_zu_gross`, `firmenvorgabe.anfrage_frist`, `firmenvorgabe.anfrage_ungueltig`, `firmenvorgabe.veraltet`, `firmenvorgabe.ausser_leseklassen`, `firmenvorgabe.temp_fehlgeschlagen`, `firmenvorgabe.fsync_fehlgeschlagen`, `firmenvorgabe.rename_fehlgeschlagen`, `firmenvorgabe.ordner_fsync_fehlgeschlagen`, `firmenvorgabe.zustand_unklar` |

   Nach `firmenvorgabe.veraltet` hat die IT die Datei geändert. Der Dienst lädt sie im Pilot nicht neu (DI-E2 Punkt 3): Jede weitere Änderung
   endet mit demselben Code, bis der Dienst neu gestartet ist.
7. **Protokoll.** Jede Stufe, auch eine Ablehnung, ergibt genau eine Zeile nach `regelebenen` 15.6 mit dem Pseudonym des Partners. Keine Zeile
   und keine Nachricht an die Ansicht nennt ein Konto, einen Pfad oder den Inhalt der Datei.
8. **Fälle.** Den Weg prüfen die Fälle `faelle-schreiben/` des Vertrags `regelebenen` (15.12.5); der Bau des Dienstes übernimmt sie als
   Außentest. Eigene Fälle des Dienstes zu DI-E1 gibt es nicht, weil die Referenz des Dienstes `EinstellungSetzen` im Lauf nicht ausführt.

### DI-E2 Firmenvorgabe beim Start, Sperre und Neuladen (Entscheid 2)

1. **Prüfung beim Start.** Einmal beim Start, nach dem Lesen des Bestands (DI-L5 Punkt 4) und vor dem ersten Betrachter, prüft der Dienst die
   Datei `lauf.firmenvorgabe` ganz. Ordner und Datei öffnet er nur über Deskriptoren: den Ordner mit `O_RDONLY | O_DIRECTORY | O_NOFOLLOW |
   O_CLOEXEC`, die Datei relativ dazu mit `O_RDONLY | O_NOFOLLOW | O_NONBLOCK | O_CLOEXEC`, dann `fstat` an beiden Griffen. Er prüft:
   - Datei, Ordner und Eltern wie der Helfer in `regelebenen` 15.12.3 Stufe H3 (reguläre Datei, kein Verweis, Eigentümer, Modus ohne `& 022`,
     Eltern bis `/` root-eigen ohne `& 022`). Eine fehlende Datei ist keine reguläre Datei. Code `firmenvorgabe.datei_unsicher`.
   - Größe höchstens 1 048 576 Byte, Form wie Stufe H4 und die Zuordnung jedes Kontos zu einem Pseudonym (`regelebenen` 15.12.3, Ladefehler).
     Code `firmenvorgabe.ladefehler`.
2. **Ungültig heißt: kein Tor.** Scheitert eine Prüfung, startet der Dienst trotzdem, aber ohne Tor: kein `Personen::neu` („L4 und L10 im
   Dienst“ Punkt 8), niemand ist Admin, jede Änderung der Firmenvorgabe endet an Stufe 1 (DI-E1 Punkt 3), und jede Prüfung, die das Tor braucht,
   lehnt ab (fail closed). Er schreibt eine Zeile `wb-dienst: firmenvorgabe nicht geladen: <code>` nach stderr, ohne Pfad und Inhalt. Sitzungen,
   Liste und Ansicht laufen weiter. Der Exitcode ändert sich nicht.
3. **Neuladen heißt Neustart.** Der Dienst liest die Datei in einem Lauf genau einmal, beim Start. Eine Änderung von Hand mit `sudo` oder über
   einen anderen Weg zum Helfer wirkt erst nach einem Neustart des Dienstes. Einen eigenen Weg zum Neuladen gibt es im Pilot nicht: kein Signal,
   kein Ereignis, kein Beobachten der Datei. Übernimmt der Dienst nach DI-E1 Punkt 5 eine eigene Änderung, ist das kein Neuladen: Er kennt den
   Inhalt, den er gesendet hat, und die Prüfsumme aus der Antwort.
4. **Die Sperre bleibt.** Nach `firmenvorgabe.zustand_unklar` sperrt der Dienst bis zu seinem Ende. Jedes weitere `EinstellungSetzen` mit
   `ebene` `firma` bekommt `{"grund": "firmenvorgabe.zustand_unklar"}`, ohne den Helfer anzusprechen (Stufe 0a). Der Dienst übernimmt keinen
   neuen Stand und baut kein neues Tor; es gilt der Stand vor der gescheiterten Änderung. Andere Ereignisse bleiben unberührt. Die Sperre lebt nur
   im Prozess. Nach dem Neustart prüft der Dienst die Datei nach Punkt 1 neu, ihr Inhalt ist die neue Basis, und die Sperre ist aufgehoben.
5. **Zustand im Prüfmodus.** Mit `lauf.firmenvorgabe` trägt die Antwort auf `Zustand` `dienst.firmenvorgabe {"sha256", "code", "gesperrt"}`:
   die Prüfsumme der geladenen Bytes oder `null`, der Code der Prüfung oder `null`, und ob der Dienst gesperrt ist. Ohne den Schlüssel fehlt das
   Feld; kein bestehender Fall ändert sich.
6. **Fälle.** `die2_vorgabe_start`: Eine gültige Datei ist geladen; nach einer Änderung der Datei im Lauf, einem `Starten` und einem zweiten
   Betrachter nennt der Zustand weiter die Prüfsumme des Starts. `die2_vorgabe_start_unsicher` (Datei für Gruppe oder andere schreibbar),
   `die2_vorgabe_start_verweis`, `die2_vorgabe_start_ordner_offen` und `die2_vorgabe_start_ladefehler`: die Zeile mit dem Code, `code` im Zustand,
   `Starten` geht weiter. Die Referenz prüft Dateiart, Modus von Datei und Ordner, Größe und Form. Eigentümer und Eltern prüft der Bau mit einem
   Einheitstest, denn ein Fall kann keine Datei eines anderen Kontos anlegen; die Zuordnung der Konten kommt mit dem Kanal zur Stelle. Die Sperre
   prüft der Fall `schreiben-sperre-bis-neuladen` in `regelebenen` 15.12.5; dort heißt `neuladen` für den Dienst: Neustart.

### DI-E3 Worker anfordern (Entscheid 3)

Wie ein Worker im Pilot entsteht, steht in DI-L1 Punkt 7. Der Kanal, über den ein Agent einen Worker anfordert, und der Vorgang
`worker.anfordern` stehen im Nachtrag „Kanal zur Stelle (L6, Nr. 98)“. Dieser Abschnitt legt keinen eigenen Kanal fest.

### DI-E4 Löschen mit dem Gespräch (Entscheid 4)

1. **Regel.** „Endgültig löschen“ in der Ansicht (Vertrag `seite`, B16), im Dienst das Ereignis `Loeschen` (DI-L3 Punkt 4), entfernt das Gespräch
   der gelöschten Sitzung mit. Grund: Der Nutzer erwartet von „endgültig“, dass die Unterhaltung weg ist; sonst bliebe sie ohne Ende liegen.
   Arbeitsordner, Protokollkette, Stelle und Halter entfernt `Loeschen` weiter nie.
2. **Ort.** Nach dem Vertrag `start`, Fassung 2, Abschnitt „Fortsetzen in „Lauf starten““, Absatz „Ablage des Gesprächs“, liegt das Gespräch in
   `<zustandsordner>/gespraeche/<sitzung>/` im Zustandsordner der Installation, mit `gespraech.json` und dem Zustandsordner des Harness. Neuer
   optionaler Schlüssel **`lauf.gespraeche`** der Startdatei, in beiden Betriebsarten: der Ordner `gespraeche` dieses Zustandsordners, ein
   absoluter Pfad, geprüft wie `lauf.bestand`. Das Paket schreibt ihn. Ohne den Schlüssel entfernt `Loeschen` kein Gespräch, wie vor diesem
   Nachtrag. Der Dienst baut den Pfad nur aus `lauf.gespraeche` und der Kennung; er liest dafür keine Datei.
3. **Welche Gespräche.** Für die Sitzung und für jeden beendeten Worker, dessen Eintrag mit ihr geht (DI-L3 Punkt 5), den Eintrag
   `<lauf.gespraeche>/<kennung>` mit allem darin. Ausnahme: Eine Sitzung mit `fortgesetzt_als` hat kein eigenes Gespräch mehr. Es gehört der
   Sitzung, die es fortsetzt, und „Lauf starten“ benennt es beim Start dieser Sitzung um; der Dienst entfernt für sie nichts. Sonst verlöre ein
   Start, der noch umbenennt, sein Gespräch. Fehlt `lauf.gespraeche` oder der Eintrag, entfernt der Dienst nichts, und das ist kein Fehler
   (eine Sitzung der Fassung 1, ein Start, der vor dem exec scheiterte, die alte Kennung nach dem Fortsetzen).
4. **Nur über Deskriptoren.** Bei jedem `Loeschen` öffnet der Dienst `lauf.gespraeche` einmal mit `openat(AT_FDCWD, …, O_RDONLY | O_DIRECTORY |
   O_NOFOLLOW | O_CLOEXEC)` und hält den Griff bis zum Ende des Ereignisses. `fstat` muss den effektiven Benutzer als Eigentümer und den Modus
   genau 0700 zeigen; sonst entfernt er nichts. Darunter entfernt er jeden Eintrag wie den Ordner einer Sitzung (DI12 Punkt 12) mit
   `loeschen::sitzung_entfernen` relativ zu diesem Griff: Name nur in der Form `s-<Ziffern>`, `statat` mit `AT_SYMLINK_NOFOLLOW`, ein Ordner nur
   über `openat` mit `O_DIRECTORY | O_NOFOLLOW` und gleichem Gerät und Inode wie bei `statat`, `unlinkat` relativ zum offenen Ordner, ein Verweis
   oder eine Datei als sie selbst und nie ihr Ziel, Tiefe höchstens `TIEFE_MAX`. Danach prüft er mit `statat` und `AT_SYMLINK_NOFOLLOW`, dass der
   Eintrag fehlt. Kein Pfadtext erreicht einen Aufruf außer `lauf.gespraeche` selbst.
5. **Reihenfolge und Fehler.** Der Dienst prüft zuerst DI-L3 Punkt 4 ganz. Dann entfernt er die Gespräche, erst danach Eintrag, Kern, Träger und
   Abos. Scheitert das Öffnen oder die Prüfung von `lauf.gespraeche` oder liegt der Eintrag danach noch da, ist die Antwort
   `Ablehnung {"grund": "gespraech_nicht_geloescht"}` nur an den Sender. Liste, Bestand, Kern und Abos bleiben dann unverändert; ein neues
   `Loeschen` versucht es wieder. Ein Teil des Gesprächs kann dann schon fehlen. Ein `Fortsetzen` endet in „Lauf starten“ dann fail closed oder
   im Harness, nie mit einem stillen Neustart (Vertrag `start`). Der Dienst schreibt eine Zeile `wb-dienst: gespraech nicht geloescht: <grund>`
   nach stderr, ohne Pfad und Kennung, mit `grund` aus `gespraeche_unsicher` (kein Ordner, ein Verweis, fremder Eigentümer, Modus nicht 0700),
   `oeffnen` (ein anderer Fehler beim Öffnen) und `rest` (der Eintrag liegt noch da). `gespraech_nicht_geloescht` ist ein neuer fester Grund von
   DI-L3; `Loeschen` trägt in der Ablehnung weiter nur `grund`.
6. **Was weiter nie entfernt wird:** der Arbeitsordner, die Protokollkette, der Zustand der Stelle und des Halters, die Ordner und Dateien von
   „Lauf starten“ (`lauf.sitzungen`, `lauf.ziellisten`, `lauf.laufzeit`, Marker), die Gespräche anderer Sitzungen und jede Datei außerhalb des
   Ordners des Dienstes außer `<lauf.gespraeche>/<kennung>` nach Punkt 3.
7. **Grenze (hergeleitet).** Eine laufende Sitzung lässt sich nicht löschen (`sitzung_laeuft`); ihr Harness schreibt dann nicht mehr in das
   Gespräch. Zwischen der Prüfung und dem Entfernen kann nur ein Prozess desselben Kontos außerhalb eines Einschlusses den Baum ändern; einen
   Verweis, den er dabei setzt, entfernt der Dienst als Verweis. Der Zustandsordner liegt außerhalb jedes Einschlusses (Vertrag `start`).
8. **Fälle.** `die4_loeschen_gespraech`: Das Gespräch eines Orchestrators und seines beendeten Workers ist danach weg, das einer laufenden
   Sitzung und ein fremder Eintrag bleiben; in jedem Gespräch liegt ein Verweis auf einen Ordner und einer auf eine Datei außerhalb, beide Ziele
   bleiben. `die4_loeschen_gespraech_fortgesetzt`: Löschen einer fortgesetzten Sitzung lässt ihr Gespräch liegen; eine Sitzung ohne Gespräch
   löscht ohne Fehler. `die4_loeschen_gespraeche_unsicher`: `lauf.gespraeche` mit Modus 0755 ergibt `gespraech_nicht_geloescht`, die Zeile
   `gespraeche_unsicher`, und die Sitzung bleibt. Ein Fehler mitten im Entfernen (`rest`) ist im Fall nicht herstellbar, weil der Prüfclient unter
   Linux als root eines Nutzer-Namensraums läuft; ihn prüft der Bau mit einem Einheitstest neben denen von `loeschen.rs`.

### Gegenproben des Nachtrags 27.11.2026

| Mutant | Regel | Änderung |
|---|---|---|
| `loeschen_gespraech_bleibt` | DI-E4 | `Loeschen` entfernt kein Gespräch |
| `loeschen_folgt_verweis` | DI-E4 | das Entfernen folgt einem Verweis im Gespräch und löscht sein Ziel außerhalb |
| `loeschen_gespraeche_ganz` | DI-E4 | `Loeschen` entfernt jeden Eintrag unter `lauf.gespraeche`, auch die Gespräche anderer Sitzungen |
| `loeschen_fortgesetzt_gespraech` | DI-E4 | `Loeschen` einer fortgesetzten Sitzung entfernt das Gespräch unter ihrer Kennung |
| `vorgabe_start_ungeprueft` | DI-E2 | der Dienst nimmt beim Start jede lesbare Datei als Firmenvorgabe an |
| `vorgabe_neu_geladen` | DI-E2 | der Dienst liest die Firmenvorgabe im Lauf neu |

Mutanten des Löschcodes laufen nur in einem Einschluss auf dem Linux-Baurechner.

### Prüfmittel des Nachtrags 27.11.2026

Der Prüfclient kennt drei neue Angaben:
- Fall-Feld `gespraeche` `{"vorher": [<Kennung>…], "modus": "0700"}`: Ordner `<base>/gespraeche` (Platzhalter `@gespraeche`) mit je einem Gespräch
  (`gespraech.json` mit 0600, ein Zustandsordner des Harness mit Unterordnern, ein Verweis auf den Ordner `<base>/aussen` und einer auf eine Datei
  darin). `ende.gespraeche_inhalt` nennt die Einträge danach, `ende.aussen_inhalt` alles unter `<base>/aussen`, `ende.gespraech_protokoll` die
  Gründe der Zeilen `wb-dienst: gespraech nicht geloescht: <grund>`.
- Fall-Feld `firmenvorgabe` `{"inhalt": <JSON>` oder `"text": <Text>, "modus": "0644", "ordner_modus": "0755", "art": "datei" | "verweis"}`:
  `<base>/vorgabe/firmenvorgabe.json` (Platzhalter `@firmenvorgabe`); `verweis` macht den Namen zu einem Verweis auf eine gültige Datei.
  `ende.vorgabe_protokoll` nennt die Codes der Zeilen `wb-dienst: firmenvorgabe nicht geladen: <code>`.
- Schritt `VorgabeAendern {"inhalt": <JSON>}`: ersetzt die Datei im Lauf (neue Datei daneben, Umbenennen).

### Ersetzte Stellen

- DI-L1 Punkt 7: Wie ein Worker im Pilot entsteht (Entscheid 3).
- DI-L3 Punkt 5: `Loeschen` entfernt das Gespräch mit; „das Gespräch des Harness“ fällt aus der Liste dessen, was es nie entfernt (Entscheid 4).
- „L4 und L10 im Dienst“ Punkt 6: Der Dienst schreibt die Datei nicht selbst, sondern über den Helfer (Entscheid 1); die Sperre ist nicht mehr
  vorläufig, und Neuladen heißt Neustart (Entscheid 2).

<!-- vom Bauvorgang angefuegt -->
> Hinweis: Dieser Text nennt Werkzeuge, die in diesem Klon nicht liegen: `pi`.
> Sie gehoeren zum Setup der Maschine, aus der dieser Baum stammt; hier fehlen
> sie mit Absicht. Wer sie braucht, schreibt sich seine eigenen.
> *Named here, not shipped here.*
