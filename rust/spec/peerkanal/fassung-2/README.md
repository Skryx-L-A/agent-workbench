# Vertrag peerkanal, Fassung 2: Gruppen und Cgroup der Gegenseite unter Linux

Stand 23.11.2026, Teil des Plans Schritt II (Halter unter dem Systemnutzer `wb-halter`). Entscheide
vom 23.11.: Schritt II mit dem Systemnutzer `wb-halter` und den drei Rollengruppen `wb-hub`, `wb-bruecke` und
`wb-oberflaeche`; das Konto `wb-halter` hat eine eigene Hauptgruppe `wb-halter`, die keine Rollengruppe ist. Ob die
Brücke ihre Gruppe über ein setgid-Programm (O1) oder eine System-Einheit (O5) bekommt, ist noch offen; dieser Vertrag
trägt beide Wege. Der Vertrag steht vor dem Code.

## Verhältnis zu Fassung 1

- **Fassung 1 gilt unverändert.** `../peerkanal.schema.json`, `../bau-faelle.py` und die 61 Fälle in `../faelle/` und
  `../erwartet/` bleiben byte-gleich. `../README.md` bekommt nur einen Verweis auf Fassung 2 und die neue Zeile zum
  Prüfer; der Vertragstext von Fassung 1 ändert sich nicht. Nachtrag 25.11.2026 (Projektentscheid, 25.11.2026): Die
  sechs Entscheide dieses Tages stehen als Nachträge in `../README.md` und gelten auch für Fassung 2; 17 weitere Fälle
  ergänzen die 61 (zusammen 78), und das Schema von Fassung 1 kennt dazu die Seite `auskunft`. Die 61 Fälle bleiben
  byte-gleich. `../pruefen.py` prüft Fassung 1 wie vor Fassung 2 gegen
  das Schema von Fassung 1.
- **Fassung 2 ist Fassung 1 mit den Änderungen dieser Datei.** Wo diese Datei nichts sagt, gilt Fassung 1.
- **Die Fälle von Fassung 1 gelten für einen Bau von Fassung 2 weiter.** Der Läufer vergleicht dort nur die Felder, die
  Fassung 1 kennt; `gruppen` und `cgroup` vergleicht er in diesen Fällen nicht. Keine Erwartung von Fassung 1 ändert
  sich. Kein Fall von Fassung 1 wird geändert oder entfernt.
- **Grundsatz 8 bleibt:** Die Bibliothek misst und prüft nur gegen die Liste des Aufrufers. Ob eine Rollengruppe als
  Ergänzungsgruppe reicht, ob zwei Rollengruppen zu viel sind und was die Cgroup bedeutet, entscheidet der Aufrufer im
  Haken (Stufe 7 von Fassung 1).
- **Was Fassung 2 nicht zusagt:** Fassung 2 trennt keine Prozesse derselben uid über die Cgroup. Die Cgroup ist ein
  gemeldeter Wert, keine Erlaubnis.

Dateien von Fassung 2: diese Datei, `peerkanal-2.schema.json`, `bau-faelle.py`, `faelle/` und `erwartet/`. Jeder Fall
und jede Erwartung trägt `"fassung": 2`.

## Änderungen (alt und neu)

| Stelle | Fassung 1 (alt) | Fassung 2 (neu) |
|---|---|---|
| Erlaubnis Unix | `{uid, gid?}`; ist `gid` angegeben, muss die primäre gid gleich sein | dazu `{uid, gruppe}`, **gilt nur unter Linux**; `gid` und `gruppe` nie zugleich |
| `unix` der Server-Vorgabe | `{ordner, modus}` | dazu optional `ordner_gruppe` (gid); dann Ordner 0750 mit dieser Gruppe und Socket 0666 |
| Peer | 12 Felder | dazu `gruppen` und `cgroup` |
| Stufe 1 Anlegen (`pk.vorgabe_ungueltig`) | Form, Art der Erlaubnis, `unix`, `job` | dazu: `gruppe` oder `ordner_gruppe` unter macOS |
| Stufe 2 Anlegen (`pk.ordner_rechte`) | Ordner genau 0700 der eigenen effektiven uid | mit `ordner_gruppe`: genau 0750, Eigentümer die eigene effektive uid, Gruppe gleich `ordner_gruppe` |
| Stufe 2 Annehmen (Prozess binden) | Linux: `SO_PEERCRED`, `SO_PEERPIDFD`, Startzeit, Programm | dazu `SO_PEERGROUPS` und die Cgroup |
| Stufe 6 Annehmen, Gründe Unix | `pk.uid_nicht_erlaubt`, `pk.gid_falsch` | `pk.uid_nicht_erlaubt`, `pk.gruppen_unlesbar`, `pk.gruppe_fehlt`, `pk.gid_falsch` |
| Socket-Modus 0660 | nur mit gid in der Erlaubnis | unverändert; eine Erlaubnis mit `gruppe` macht 0660 nicht zulässig (Abschnitt Ordner mit Gruppe) |
| Codes je Seite | Tabelle in Fassung 1 | unverändert; kein neuer Code, zwei neue Gründe |

## Erlaubnis `{uid, gruppe}` (nur Linux)

- **Form:** `{uid, gruppe}` mit zwei Zahlen wie in Fassung 1 (0 bis 4294967294). `uid` ist Pflicht. Eine Erlaubnis mit
  `gid` und `gruppe` zugleich ist `pk.vorgabe_ungueltig` (Schema). Eine Liste darf alle drei Formen `{uid}`,
  `{uid, gid}` und `{uid, gruppe}` mischen.
- **Nur Linux:** Unter macOS endet `anlegen` mit einer Erlaubnis mit `gruppe` in Stufe 1 mit `pk.vorgabe_ungueltig`
  (Fall `a-gruppe-unter-mac`). Unter Windows ist jede Unix-Erlaubnis schon in Fassung 1 `pk.vorgabe_ungueltig`.
- **Wann die Gruppe trifft:** Die Erlaubnis trifft, wenn `uid` gleich `peer.uid` ist und `gruppe` gleich `peer.gid` ist
  oder in `peer.gruppen` steht.
- **Wenn `gruppen` `null` ist:** Ist `gruppe` gleich `peer.gid`, trifft die Erlaubnis; `gruppen` wird dafür nicht
  gebraucht (Fall `n-gruppen-null-gid-trifft-linux`). Ist `gruppe` ungleich `peer.gid`, trifft diese Erlaubnis nicht.
  Die Bibliothek nimmt in diesem Fall nie an, dass die Gruppe fehlt oder vorhanden ist; sie weiß es nicht.
- **Fail closed:** Trifft keine Erlaubnis und hätte eine Erlaubnis dieser uid mit `gruppe` ungleich `peer.gid` nur über
  `gruppen` treffen können, während `gruppen` `null` ist, gilt der Peer als nicht erlaubt mit dem eigenen Grund
  `pk.gruppen_unlesbar` (Code `pk.kennung_nicht_erlaubt`, Fall `n-gruppen-null-linux`). Trifft eine andere Erlaubnis,
  ist der Peer erlaubt; die fehlende Messung wird dann nicht gebraucht.
- **Mehrere Treffer:** Treffen mehrere Erlaubnisse, ist der Peer erlaubt. Die Bibliothek wählt keine Rolle. Der
  Aufrufer sieht `gid` und `gruppen` im Peer und entscheidet im Haken (Fall `n-mehrere-rollengruppen`: der Halter lehnt
  mit `aufrufer:rollen-mehrdeutig` ab).
- **Hauptgruppe des Systemkontos:** `wb-halter` ist keine Rollengruppe. Der Aufrufer nimmt sie nie als `gruppe` in die
  Liste. Die Bibliothek kennt keine Gruppennamen; sie vergleicht nur Zahlen.

## Ordner mit Gruppe (`unix.ordner_gruppe`, nur Linux)

Ein Server unter einem eigenen Systemkonto (Halter unter `wb-halter`) ist für Clients unter der uid der Person nur
erreichbar, wenn sie seinen Ordner durchqueren dürfen. Fassung 1 verlangt den Ordner 0700 der eigenen Kennung; das
schließt jede fremde uid aus. Der Paketvertrag Teil A2 legt je Rolle einen Ordner `/run/wb-halter/<rolle>/` mit
`wb-halter:wb-<rolle>` und 0750 an (Annahme A2-2 dort). Fassung 2 bietet dafür:

- `unix.ordner_gruppe` (gid). Ist es gesetzt, prüft Stufe 2 von `anlegen`: Der Ordner ist ein Ordner (kein Link, per
  `lstat`), gehört der eigenen effektiven uid, hat die Gruppe `ordner_gruppe` und genau den Modus 0750. Sonst
  `pk.ordner_rechte` (Fall `a-ordner-gruppe-falsch-linux`). Die Bibliothek legt den Ordner weiter nie an und ändert
  weder Gruppe noch Rechte.
- Mit `ordner_gruppe` ist der Socket-Modus genau 0666 (Schema; Fall `v2-ordner-gruppe-modus`). Grund: Der Socket
  bekommt die Gruppe des Servers (`wb-halter`), nicht die des Ordners; mit 0660 erreichte ihn kein Rollenprogramm. Die
  Sperre trägt der Ordner: Ohne `ordner_gruppe` als gid oder Ergänzungsgruppe endet `connect` mit `EACCES`
  (hergeleitet, Annahme P2-5). Der Client meldet dann `pk.zugriff_verweigert` wie in Fassung 1.
- Zwischen `bind` und `chmod` erreicht den Socket nur ein Prozess mit `ordner_gruppe`. Die Liste prüft auch ihn; ein
  Socket mit dem Modus vor `chmod` ist darum kein Weg an der Liste vorbei.
- Ohne `ordner_gruppe` gilt Fassung 1 unverändert.

## Peer: neue Felder

| Feld | Windows | Linux | macOS |
|---|---|---|---|
| `gid` (Fassung 1) | `null` | aus `SO_PEERCRED`; für ein setgid-Programm die Gruppe des Programms (Annahme P2-1) | aus `LOCAL_PEERCRED` (gid = erste Gruppe), unverändert |
| `gruppen` (neu) | `null` | Ergänzungsgruppen aus `SO_PEERGROUPS`, aufsteigend ohne Doppel; `null`, wenn die Messung scheitert | `null` |
| `cgroup` (neu) | `null` | Pfad der Cgroup v2 aus der Zeile `0::` von `/proc/<pid>/cgroup`; `null`, wenn nicht lesbar | `null` |

- **`gruppen` ist `null`, nicht leer,** wo es die Messung nicht gibt oder sie scheitert. `[]` heißt: gemessen, keine
  Ergänzungsgruppe. Unter macOS bleibt `gruppen` `null`, obwohl `LOCAL_PEERCRED` eine Gruppenliste trägt; Fassung 2
  ändert macOS nicht.
- **Messung `gruppen` (Linux):** `getsockopt(SO_PEERGROUPS)` mit einem Puffer für 64 Gruppen. Meldet der Kernel
  `ERANGE`, steht die nötige Länge im Längenfeld; die Bibliothek fragt genau einmal mit dieser Länge nach. Jeder
  andere Fehler, auch ein zweites `ERANGE`, ergibt `gruppen: null`. Kein Abbruch, kein Ereignis an dieser Stelle.
  Die Werte gelten wie bei `SO_PEERCRED` zum Zeitpunkt des `connect` (Annahme P2-2). `peer.gid` steht nur dann in
  `gruppen`, wenn sie auch eine Ergänzungsgruppe ist; die Bibliothek fügt sie nicht hinzu.
- **Messung `cgroup` (Linux):** Im selben Schritt wie `programm`, während die pidfd gehalten wird: `/proc/<pid>/cgroup`
  lesen, nur die Zeile `0::<pfad>` nehmen, danach an der pidfd prüfen, dass der Prozess noch lebt (sonst
  `pk.prozess_gewechselt` wie in Fassung 1). Ohne Zeile `0::` (reine Cgroup v1), bei einem Lesefehler, einem Pfad
  über 4096 Byte, einem Pfad ohne `/` am Anfang oder mit NUL: `cgroup: null`. Kein Abbruch, kein Ereignis
  (Fall `n-cgroup-unlesbar`).
- **Was `cgroup` bedeutet:** Der Wert gilt zum Zeitpunkt des `accept`, nicht des `connect`. Ein Prozess kann danach
  verschoben werden. Unter einer Benutzereinheit kann die Person eigene Prozesse in jede Cgroup ihres Teilbaums
  schieben (Plan O2, hergeleitet). Der Pfad ist relativ zur Wurzel des Cgroup-Namensraums des Servers. Die Bibliothek
  entscheidet nichts aus der Cgroup.
- `PIDFD_GET_INFO` mit Cgroup-Kennung nutzt Fassung 2 nicht (Messung L-7 fehlt). Folgt sie, kommt die Kennung als
  weiteres Feld dazu; `cgroup` bleibt der Pfad.

## Stufen von `annehmen` in Fassung 2

Die Reihenfolge von Fassung 1 bleibt. Geändert sind drei Stufen:

2. **Prozess binden.** Linux zusätzlich: `SO_PEERGROUPS` (`gruppen`) und die Cgroup (`cgroup`), wie oben. Was sich
   nicht messen lässt, ist `null`.
4. **`pk.peer_unmessbar`:** unverändert. Scheitert nur `SO_PEERGROUPS` oder die Cgroup, ist das **kein**
   `pk.peer_unmessbar`; `uid` und `gid` aus `SO_PEERCRED` bleiben Pflicht.
6. **`pk.kennung_nicht_erlaubt`, Unix.** Sei K die Menge der Erlaubnisse mit `uid` gleich `peer.uid`. Trifft eine
   Erlaubnis aus K (Abschnitt Erlaubnis), geht es weiter zu Stufe 7. Sonst gilt der erste zutreffende Grund:
   1. `pk.uid_nicht_erlaubt`: K ist leer.
   2. `pk.gruppen_unlesbar`: K enthält eine Erlaubnis mit `gruppe` ungleich `peer.gid`, und `gruppen` ist `null`.
   3. `pk.gruppe_fehlt`: K enthält eine Erlaubnis mit `gruppe`.
   4. `pk.gid_falsch`: sonst (nur Erlaubnisse mit `gid`, wie in Fassung 1).

   Für eine Liste ohne `gruppe` ergibt das genau die Gründe von Fassung 1.

## Gründe

| Code | Gründe in Prüfreihenfolge |
|---|---|
| `pk.hallo_ungueltig` | unverändert |
| `pk.peer_unmessbar` | unverändert |
| `pk.prozess_gewechselt` | unverändert |
| `pk.kennung_nicht_erlaubt` | `pk.eingeschraenkt`, `pk.sid_nicht_erlaubt`, `pk.appcontainer_falsch`, `pk.uid_nicht_erlaubt`, **`pk.gruppen_unlesbar`**, **`pk.gruppe_fehlt`**, `pk.gid_falsch` |
| `pk.aufrufer_abgewiesen` | unverändert |

`pk.gruppen_unlesbar` und `pk.gruppe_fehlt` gibt es nur unter Linux. Beide gehen wie alle Gründe unverändert in die
Zeilenart `kanal.abgewiesen` aus Vertrag 5. Der Client sieht von außen weiter nur `pk.abgewiesen`.

## Client (Gegenrichtung)

Unverändert bis auf den Peer: Unter Linux misst der Client den Server wie Stufe 2, also mit `gruppen` und `cgroup`.
`erwartet` bleibt `{uid}`. Gegen einen Halter unter `wb-halter` ist die erwartete uid die des Systemkontos; die Prüfung
der Gruppe des Servers ist nicht Teil der Bibliothek.

## macOS und Windows

- **macOS:** Die Kennung bleibt `uid` und `gid` aus `LOCAL_PEERCRED`, die Prozessbindung das Audit-Token
  (`LOCAL_PEERTOKEN` mit pidversion), die Rolle hängt beim Aufrufer an der Code-Signatur über das Audit-Token. `gruppen`
  und `cgroup` sind immer `null` (Fall `n-erlaubt-mac-fassung2`). Die Erlaubnis `{uid, gruppe}` und `ordner_gruppe`
  gelten nur unter Linux; unter macOS sind sie `pk.vorgabe_ungueltig`.
- **Windows:** Die Kennung bleibt die SID aus dem impersonierten Token (`TokenUser`, `TokenAppContainerSid`), der Server
  prüft das Paar aus SID und Package-SID; Dienste unter `NT SERVICE\…` trennen über ihre Dienst-SID. `gruppen` und
  `cgroup` sind immer `null`. Sonst ändert sich nichts.

## Annahmen mit Messbezug

Gemessen ist in Fassung 2 nichts am Kernel. Jede Aussage unten ist hergeleitet und gilt bis zur genannten Messung als
Annahme, nicht als Zusage.

| Nr. | Annahme | Messung |
|---|---|---|
| P2-1 | `SO_PEERCRED` liefert als gid die effektive gid; für ein setgid-Rollenprogramm also die Rollengruppe (gleich Paket A2-4) | L-1 |
| P2-2 | `SO_PEERGROUPS` liefert die Ergänzungsgruppen des Clients zum Zeitpunkt des `connect`, ohne die effektive gid hinzuzufügen | L-1 |
| P2-3 | `SO_PEERGROUPS` gibt es auf Ubuntu 26.04 und dem Linux-Baurechner; der Protokollbeleg dafür ist nicht geladen (Plan, Abschnitt 1) | L-1 |
| P2-4 | `/proc/<pid>/cgroup` eines Prozesses der Person ist für einen Server unter `wb-halter` lesbar; sonst ist `cgroup` dort immer `null` | L-5 |
| P2-5 | `connect` ohne `ordner_gruppe` als gid oder Ergänzungsgruppe scheitert am Ordner 0750 mit `EACCES` (gleich Paket A2-2) | L-2 |
| P2-6 | Ein Prozess mit `no_new_privs` (Agent im Einschluss) bekommt die Rollengruppe auch über ein setgid-Programm nicht | L-3 |
| P2-7 | Ein Testkonto nach `initgroups` trägt seine Hauptgruppe auch als Ergänzungsgruppe; die Fälle setzen die Gruppen des Testkontos ausdrücklich im Aufbau | Aufbau des Läufers |
| P2-8 | Die Cgroup-Kennung aus `PIDFD_GET_INFO` ist auf den Zielkernen verfügbar; Fassung 2 nutzt sie nicht | L-7 |

## Sicherheitszusagen (neu, prüfbar)

| Nr. | Zusage | Fälle |
|---|---|---|
| S16 | Eine Erlaubnis mit `gruppe` trifft genau, wenn `gruppe` die gid ist oder in `gruppen` steht | `a-rollengruppe-linux`, `n-rollengruppe-nur-ergaenzend-linux`, `n-gid-fremd-unix` |
| S17 | Ist `gruppen` `null` und wird es für die Entscheidung gebraucht, wird der Peer abgewiesen (`pk.gruppen_unlesbar`) | `n-gruppen-null-linux`, `n-gruppen-null-gid-trifft-linux` |
| S18 | Mehrere Treffer: die Bibliothek nimmt an und meldet beide Gruppen; der Haken entscheidet | `n-mehrere-rollengruppen` |
| S19 | `cgroup` `null` bricht nichts ab und gehört zu keiner Erlaubnis | `n-cgroup-unlesbar`, `n-cgroup-linux` |
| S20 | Ordner mit Gruppe: genau 0750, eigene uid, Gruppe gleich `ordner_gruppe`; Socket 0666 | `a-ordner-gruppe-*`, `v2-ordner-gruppe-modus`, `v2-gueltig-ordner-gruppe` |
| S21 | `gruppe` und `ordner_gruppe` gelten nur unter Linux | `a-gruppe-unter-mac`, `n-erlaubt-mac-fassung2` |

## Fälle

17 Fälle: 5 `v2-*` prüfen Vorgaben und sind schon mit dem Schema prüfbar; 12 Außentests (10 Linux, 2 macOS).

| Fall | Seite | Erwartung |
|---|---|---|
| `v2-gueltig-gruppe` | vorgabe | gültig |
| `v2-gid-und-gruppe` | vorgabe | `pk.vorgabe_ungueltig` |
| `v2-gruppe-ohne-uid` | vorgabe | `pk.vorgabe_ungueltig` |
| `v2-ordner-gruppe-modus` | vorgabe | `pk.vorgabe_ungueltig` (Modus 0600 mit `ordner_gruppe`) |
| `v2-gueltig-ordner-gruppe` | vorgabe | gültig |
| `a-gruppe-unter-mac` | anlegen | `pk.vorgabe_ungueltig` |
| `a-ordner-gruppe-linux` | anlegen | ok; `connect` ohne Gruppe `EACCES` |
| `a-ordner-gruppe-falsch-linux` | anlegen | `pk.ordner_rechte` |
| `a-rollengruppe-linux` | annehmen | ok, `gid` = Rollengruppe |
| `n-gid-fremd-unix` | annehmen | `pk.gruppe_fehlt` |
| `n-rollengruppe-nur-ergaenzend-linux` | annehmen | ok, Rollengruppe nur in `gruppen` |
| `n-mehrere-rollengruppen` | annehmen | Liste trifft, Haken lehnt ab: `aufrufer:rollen-mehrdeutig` |
| `n-gruppen-null-linux` | annehmen | `pk.gruppen_unlesbar` |
| `n-gruppen-null-gid-trifft-linux` | annehmen | ok, `gruppen` `null` |
| `n-cgroup-unlesbar` | annehmen | ok, `cgroup` `null` |
| `n-cgroup-linux` | annehmen | ok, `cgroup` gemessen |
| `n-erlaubt-mac-fassung2` | annehmen | ok, `gruppen` und `cgroup` `null` |

- `a-rollengruppe-linux` trägt den Namen aus dem Plan. Anders als die `a-*`-Fälle von Fassung 1 ist er ein Fall der
  Seite `annehmen`. `n-gid-fremd-unix` läuft trotz des Namens nur unter Linux, weil `gruppe` nur dort gilt.
- Alle uids und gids sind Beispielwerte wie in Fassung 1. Rollengruppen: `g_hub` 1000000211, `g_bruecke` 1000000212.
  Der Server unter eigenem Konto hat `uid_halter` 1000000103.
- Die Fälle `n-gruppen-null-*` und `n-cgroup-unlesbar` brauchen eine Testnaht des Läufers, die `getsockopt` bzw. das
  Lesen von `/proc/<pid>/cgroup` scheitern lässt. Die setgid-Fälle brauchen ein Dateisystem ohne `nosuid`.
- Linux-Fälle laufen auf einem Linux-Prüfrechner oder in einer Wegwerf-VM; macOS-Fälle an einem Mac.
- Prüfer: `<python mit jsonschema> -I rust/spec/peerkanal/pruefen.py` prüft Fassung 1 und Fassung 2, jede gegen ihr
  Schema. Zusätzlich prüft er für Fassung 2: `gruppen` aufsteigend ohne Doppel; die zwei neuen Gründe nur in Fällen mit
  `ort` genau `linux`; eine Erlaubnis mit `gruppe` außerhalb Linux erwartet `pk.vorgabe_ungueltig`.

## Gegenproben für den Bau (Mutanten)

Jede dieser Änderungen muss mindestens einen Fall rot machen: nur `gid` statt `gid` oder `gruppen` verglichen
(`n-rollengruppe-nur-ergaenzend-linux`); nur `gruppen` ohne `gid` verglichen (`a-rollengruppe-linux`); `gruppen` `null`
als leer behandelt oder als Treffer gewertet (`n-gruppen-null-linux`); `gruppen` immer gebraucht, auch wenn `gruppe`
die gid ist (`n-gruppen-null-gid-trifft-linux`); Abbruch, wenn die Cgroup nicht lesbar ist (`n-cgroup-unlesbar`); die
Bibliothek weist mehrere Treffer selbst ab (`n-mehrere-rollengruppen`); Ordnergruppe nicht geprüft
(`a-ordner-gruppe-falsch-linux`); `gruppe` unter macOS angenommen (`a-gruppe-unter-mac`).

## Nutzer

| Nutzer | Braucht aus Fassung 2 |
|---|---|
| Halter, Schritt II (Halter 2.3 Fassung 2) | Server unter `wb-halter`, je Rolle ein Socket in `/run/wb-halter/<rolle>/` mit `ordner_gruppe` gleich der Rollengruppe; Liste `{uid: Person, gruppe: Rollengruppe}`; im Haken genau eine Rollengruppe in `gid` und `gruppen`, sonst `halter.peer` |
| Entscheidungsstelle, B23 | `gruppen` für die Trennung Hub gegen Agent; `cgroup` als zweite Messung, nicht als Rolle gegen die Person |

## Offen

1. Die Messungen L-1 bis L-7 fehlen. Bis dahin sind P2-1 bis P2-8 Annahmen. Zeigt L-1, dass `SO_PEERCRED` die reale
   gid liefert, bekommt ein setgid-Rollenprogramm seine Gruppe nicht als `gid`; dann trifft die Erlaubnis nur noch,
   wenn die Rolle über eine System-Einheit (O5) als Ergänzungsgruppe kommt, und die Fälle `a-rollengruppe-linux`,
   `n-gruppen-null-gid-trifft-linux` und `n-mehrere-rollengruppen` ändern ihren Aufbau.
2. Ob die Rolle allein aus `gid` kommen muss (Plan O1: „genau eine Rollengruppe ist die gid“) oder auch aus `gruppen`
   (O5), entscheidet der Halter nach L-3. Die Bibliothek trägt beides.
3. Die Form der Sockets je Rolle legt der Halter fest. Wählt er einen anderen Ordner als 0750 mit Rollengruppe, etwa einen
   Ordner mit setgid-Bit und Socket 0660, braucht Fassung 2 einen Nachtrag zu `ordner_gruppe`.
4. Die Belege für `SO_PEERGROUPS`, `PIDFD_GET_INFO` und die Lesbarkeit von `/proc/<pid>/cgroup` unter fremder uid sind
   nicht geladen (Plan OPEN 7).
5. Abstimmung der Felder mit Halter, Stelle und Paket macht das Projekt.

<!-- vom Bauvorgang angefuegt -->
> Hinweis: Dieser Text nennt Werkzeuge, die in diesem Klon nicht liegen: `wb-hub`.
> Sie gehoeren zum Setup der Maschine, aus der dieser Baum stammt; hier fehlen
> sie mit Absicht. Wer sie braucht, schreibt sich seine eigenen.
> *Named here, not shipped here.*
