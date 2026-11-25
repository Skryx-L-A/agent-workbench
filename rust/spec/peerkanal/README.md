# Vertrag peerkanal: lokaler Kanal mit Prüfung der Gegenseite (Fassung 1)

**Fassung 2** (Gruppen und Cgroup der Gegenseite unter Linux, 23.11.2026) steht getrennt in `fassung-2/README.md`.
Diese Datei beschreibt Fassung 1; ihre Fälle und ihr Schema bleiben unverändert.

**Nachtrag 25.11.2026 (Projektentscheid, 25.11.2026): sechs Entscheide.** Eigene SID (`eigene_sid`), Lesefrist, Client ohne
Hallo (`verbinden_ohne_hallo`, `Sid::dienst`), Ereignis vor dem Trennen (`bei_abweisung`), Ende vor vier Byte des Hallos
und Flush vor dem Trennen. Jeder Entscheid steht an seiner Stelle im Text und trägt die Kennzeichnung „Projektentscheid,
25.11.2026“. Wo sich ein Satz ändert, stehen „alt:“ und „neu:“. Die ersten fünf beschreiben, was die Bibliothek schon tut; der
sechste (Flush) ist noch nicht gebaut und trägt den Vermerk „OPEN“. 17 neue Fälle gehören dazu (Abschnitt Fälle und Prüfer);
kein bestehender Fall und keine bestehende Erwartung ändert sich.

Stand 20.11.2026. Auftrag: Projektentscheid, 20.11.2026, Nachtrag 1
des Projekts, Nachtrag 2 (Fragen der Such-Pipe unter Windows) und Nachtrag 3 (Bedarf
des Halters, `rust/spec/halter/` Abschnitt 2.1 vom 20.11.2026). Der Vertrag steht vor dem Code. Der Bau folgt als Crate
`rust/crates/peerkanal`. Verdeckte Fälle und Mutanten folgen mit dem Bau.

Anforderungen: A115 (Peer an der Named Pipe über die SID, Server am Besitzer der Instanz), A121 (Modellweg prüft Prozess,
AppContainer-SID und Job), A120 (kein ungeschützter TCP-Dienst), A03 (Einschluss). Grundsatz 8: Die Bibliothek misst und
prüft nur gegen die Liste des Aufrufers; jede weitere Entscheidung trifft der Aufrufer.

## Zweck und Grenze

Eine Bibliothek statt drei Umsetzungen. Ein Server bietet einen lokalen Kanal an. Je Verbindung misst die Bibliothek die
Kennung der Gegenseite und lässt nur Kennungen aus der Liste des Aufrufers durch. Der Client prüft umgekehrt den Server
gegen eine erwartete Kennung, bevor er ein Nutzbyte sendet. Dieselbe Rust-Schnittstelle gilt unter Windows, Linux und macOS.

- **Windows:** Named Pipe `\\.\pipe\<name>`.
- **Linux und macOS:** Unix-Socket (Stream) `<ordner>/<name>.sock`.
- **Keine Bindung an die Sitzung unter Windows.** Die Paar-Erlaubnis bindet an Person und Projekt, nicht an die Sitzung.
  Wer die Sitzung braucht (Such-Pipe), fragt zusätzlich den Plattformdienst `peer` (pid zu Sitzung) mit pid und Startzeit
  aus der Prozessbindung (Abschnitt Peer). `peerkanal` allein bindet unter Windows nie an die Sitzung.
- **Nicht in diesem Vertrag:** TCP. Den Loopback-Peer des Modellwegs (`GetExtendedTcpTable`, A121) misst der Modellweg
  selbst; die Prozessmessung dieser Bibliothek kann er dafür später mitnutzen (Offen 3). Kein Rahmenformat: Nach dem
  Handschlag ist die Verbindung ein Bytestrom, die Zeilen legt der Aufrufer fest (Stelle 13.3, Such-Socket). Die Bibliothek
  startet keine Prozesse und keine Dienste.

**Unix: keine Trennung durch die uid.** Unter bwrap haben Agent und Weiterreicher dieselbe uid; `SO_PEERCRED` trennt sie
nicht (Nachtrag 1, Punkt 4). Die Bibliothek sagt das nicht zu. Wer unter Unix welche Art ist, folgt aus dem Socket selbst:
Nur der richtige Prozess erreicht ihn, weil der Ordner 0700 außerhalb des Einschlusses liegt und der Starter höchstens
diesen einen Socket in den Einschluss bindet. Die uid-Liste schützt nur gegen andere Kennungen (etwa einen Einschluss mit
eigener Kennung bei Modus 0666). Die pidfd liefert die Bibliothek mit; eine Prüfung über Cgroup oder pidfd macht der
Aufrufer (Stelle: Hub gegen Agent).

## Begriffe

- **Kanalname:** Kennung der Form `^[a-z0-9][a-z0-9._-]{0,63}$`, wie in den Verträgen 1 bis 5.
- **SID:** Konten- oder Dienst-SID als Text: `S-1-5-21-a-b-c-rid` (lokal oder Domäne), `S-1-12-1-a-b-c-d` (Entra ID),
  `S-1-5-80-a-b-c-d-e` (virtuelles Dienstkonto `NT SERVICE\…`), `S-1-5-18` (SYSTEM). Gruppen-SIDs sind keine Kennung.
- **Package-SID:** AppContainer-SID `S-1-15-2-` mit genau sieben Unterautoritäten. `S-1-15-2-1` (ALL APPLICATION PACKAGES)
  und `S-1-15-2-2` passen nicht.
- **Erlaubnis:** Windows `{sid, appcontainer_sid}`, das Paar gilt nur genau so: `appcontainer_sid: null` erlaubt nur
  Prozesse außerhalb eines Containers. So gibt der Aufrufer die erwartete Package-SID an. Die Package-SID gilt je Person
  und Projekt, nicht je Sitzung; zwei Läufe im selben Projekt teilen sie (Windows-Einschluss, Frage 11 gemessen). Ein
  Token mit Restricting SIDs passt auf keine Erlaubnis.
  Unix `{uid, gid?}`; ist `gid` angegeben, muss die primäre gid gleich sein.
- **Peer:** das Ergebnis der Messung (Abschnitt Peer).
- **Hallo:** die 4 Byte `pk1` und LF. Der Client sendet sie nach seiner Prüfung des Servers, der Server antwortet mit
  denselben 4 Byte, wenn er annimmt. Sonst schließt er ohne Antwort. Das Hallo ist kein Nutzbyte.
  Nachtrag (Projektentscheid, 25.11.2026): alt: „Der Client sendet sie nach seiner Prüfung des Servers.“ neu: Der Client
  sendet sie nach seiner Prüfung des Servers, außer er verbindet über `verbinden_ohne_hallo` (Abschnitt Client verbinden).
- **Dienst-SID:** die SID des virtuellen Dienstkontos `NT SERVICE\<name>`, mit `Sid::dienst("<name>")` berechnet (Abschnitt
  Auskunft ohne Verbindung).

## Schnittstelle (Rust, Skizze)

```rust
pub struct ServerVorgabe {
    pub name: Kanalname,                 // Kennungsform
    pub erlaubt: Vec<Erlaubnis>,         // 1..=64, ohne Doppel, alle von der Art des Systems
    pub instanzen: u8,                   // 1..=64 gleichzeitige Verbindungen
    pub hallo_frist: Duration,           // 1 ms ..= 60 s, Vorgabe 2 s
    pub prozess_pflicht: bool,           // Vorgabe false
    pub job: Option<JobBezug>,           // nur Windows, optional: Handle des Jobs des Laufs; ohne Job ist im_job null
    pub unix: Option<UnixOrt>,           // Pflicht unter Unix, unter Windows verboten
}
pub struct UnixOrt { pub ordner: PathBuf, pub modus: SocketModus }   // 0600 | 0660 | 0666
pub enum Erlaubnis { Windows { sid: Sid, appcontainer_sid: Option<Sid> }, Unix { uid: u32, gid: Option<u32> } }

impl Server {
    pub fn anlegen(v: ServerVorgabe) -> Result<Server, Fehler>;
    pub fn annehmen(&self) -> Result<Annahme, Fehler>;      // blockiert; aus bis zu `instanzen` Threads zugleich
}
pub enum Annahme { Vorlaeufig(Vorlaeufig), Abgewiesen(Ereignis) }
impl Vorlaeufig {                                           // Haken des Aufrufers, noch kein Byte zurück
    pub fn peer(&self) -> &Peer;
    pub fn prozess(&self) -> Option<&Prozessbindung>;
    pub fn bestaetigen(self) -> Result<Verbindung, Fehler>; // erst jetzt sendet der Server das Hallo
    pub fn ablehnen(self, grund: Aufrufergrund) -> Ereignis; // schließt ohne Antwort
}
pub struct Aufrufergrund(Kennung);                          // wird im Ereignis zu "aufrufer:<kennung>"

pub struct ClientVorgabe { pub name: Kanalname, pub erwartet: Kennung, pub frist: Duration, pub unix_ordner: Option<PathBuf> }
pub fn verbinden(v: &ClientVorgabe) -> Result<Verbindung, Fehler>;

impl Verbindung { pub fn peer(&self) -> &Peer; pub fn prozess(&self) -> Option<&Prozessbindung>; }  // dazu Read und Write
pub struct Prozessbindung { pub pid: u32, pub startzeit: Option<u64> }  // privat dazu: Handle, pidfd oder Audit-Token
#[cfg(target_os = "linux")] impl Verbindung { pub fn pidfd(&self) -> BorrowedFd<'_>; }
pub struct Fehler { pub code: Code, pub os_fehler: Option<i32> }

// Nachtrag 25.11.2026 (Projektentscheid, 25.11.2026), Einzelheiten in den genannten Abschnitten:
impl Server { pub fn bei_abweisung(&mut self, haken: impl Fn(&Ereignis) + Send + Sync + 'static); }  // Annehmen und Messen
impl Verbindung { pub fn lesefrist(&mut self, frist: Option<Duration>); }                          // Lesen nach dem Handschlag
pub fn verbinden_ohne_hallo(v: &ClientVorgabe) -> Result<Verbindung, Fehler>;                      // Client verbinden
pub fn eigene_sid() -> Result<Sid, Fehler>;                                                        // Auskunft ohne Verbindung
impl Sid { pub fn dienst(name: &str) -> Result<Sid, Fehler>; }                                     // Auskunft ohne Verbindung
```

- Die Schnittstelle ist ohne `unsafe` nutzbar; der Aufrufer darf `#![forbid(unsafe_code)]` setzen. Die Bibliothek kapselt
  Win32 über `windows-sys` und die Unix-Aufrufe über `libc`. Beide stehen unter `MIT OR Apache-2.0` (gelesen am 20.11. aus
  `Cargo.toml` von `windows-sys` 0.61.2 und `libc` 0.2.191 im lokalen Cargo-Speicher). Alle Win32-Funktionen dieses
  Vertrags gibt es in `windows-sys` 0.61.2 (gesucht: `CreateNamedPipeW`, `GetNamedPipeClientProcessId`,
  `ImpersonateNamedPipeClient`, `OpenThreadToken`, `GetTokenInformation` mit `TokenAppContainerSid`, `RevertToSelf`,
  `GetSecurityInfo`, `ConvertStringSecurityDescriptorToSecurityDescriptorW`, `GetProcessTimes`, `IsProcessInJob`,
  `PIPE_REJECT_REMOTE_CLIENTS`, `FILE_FLAG_FIRST_PIPE_INSTANCE`). In `libc` 0.2.191 stehen `SO_PEERPIDFD`,
  `LOCAL_PEERPID` und `LOCAL_PEERTOKEN`.
- Die Bibliothek schreibt kein Protokoll und keine Datei außer dem Socket. Sie liest keine Umgebungsvariable. Sie beendet
  den Prozess nie, mit einer Ausnahme: Scheitert `RevertToSelf`, bricht sie ab (`abort`), weil der Thread sonst mit der
  Kennung des Clients weiterliefe.
- Exitcodes gibt es nicht; es ist eine Bibliothek. Ein Programm, das sie nutzt, bildet `pk.name_belegt` und
  `pk.ordner_rechte` auf Exit 74 ab wie der Such-Socket (Vorschlag).

Die Vorgaben und den Peer als JSON beschreibt `peerkanal.schema.json` (`$defs/server_vorgabe`, `client_vorgabe`, `peer`,
`ereignis`). So sehen die Fälle sie, und so darf ein Aufrufer sie in seine Einstellungen oder sein Protokoll übernehmen.

## Server anlegen

Prüfreihenfolge. Die erste Verletzung beendet `anlegen`; was am Namen oder Pfad liegt, bleibt unverändert.

1. `pk.vorgabe_ungueltig`: Vorgabe nicht in der Form des Schemas, oder eine Erlaubnis passt nicht zum System (SID unter
   Unix, uid unter Windows), `unix` fehlt unter Unix oder steht unter Windows, `job` unter Unix.
2. `pk.ordner_rechte` (Unix): Der Ordner fehlt, ist kein Ordner, ist ein Link (`lstat`), gehört nicht der eigenen
   effektiven uid oder hat nicht genau den Modus 0700. Die Bibliothek legt den Ordner nie an und ändert seine Rechte nie.
3. `pk.pfad_zu_lang` (Unix): `<ordner>/<name>.sock` hat mindestens so viele Byte wie `sun_path` fasst (macOS 104,
   Linux 108; das NUL zählt mit). Also höchstens 103 bzw. 107 Byte.
4. `pk.name_belegt`: Windows: `CreateNamedPipeW` mit `FILE_FLAG_FIRST_PIPE_INSTANCE` scheitert mit Fehler 5 oder 231.
   Unix: Am Pfad liegt irgendetwas, auch ein verwaister Socket, eine Datei oder ein Link. Kein Rückfall: kein Löschen,
   kein zweiter Name, keine Instanz ohne das Flag. Einen verwaisten Socket räumt der Aufrufer, der ihn kennt.
5. `pk.anlegen_fehlgeschlagen`: jeder andere Fehler des Systems, mit `os_fehler`.

**Windows.** `CreateNamedPipeW` mit `PIPE_ACCESS_DUPLEX | FILE_FLAG_FIRST_PIPE_INSTANCE | FILE_FLAG_OVERLAPPED`,
`PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_REJECT_REMOTE_CLIENTS`, `nMaxInstances = instanzen`, und dieser
Sicherheitsbeschreibung:

- Besitzer: die eigene Token-User-SID des Servers.
- DACL geschützt (`D:P`, keine Vererbung). Eine ACE `0x0012019F` für die eigene SID (Lesen, Schreiben und
  `FILE_CREATE_PIPE_INSTANCE` 0x4). Für jede andere erlaubte SID und für jede Package-SID aus der Liste eine ACE
  `0x0012019B` (dasselbe ohne 0x4). Für einen AppContainer gilt die Schnittmenge aus der ACE seiner Person und der ACE
  seiner Package-SID, also höchstens `0x0012019B`, auch wenn die Person die eigene SID des Servers ist (Such-Pipe).
- Trägt die Liste eine Package-SID, bekommt die Instanz das Label `S:(ML;;NW;;;LW)`. Ohne Package-SID setzt die Bibliothek
  kein Label. Nachtrag 24.11.2026 (gemessen in der Windows-VM, Server unter einem Admin- und unter einem Standardkonto):
  Ein AppContainer schreibt auch an einer Instanz ohne dieses Label. alt: „ohne dieses Label verweigert das vorgegebene
  Label Medium ihm das Schreiben (hergeleitet)“. Das Label bleibt Vorgabe, weil es nichts öffnet, was die DACL nicht schon
  erlaubt; ein Einheitstest prüft, dass die Bibliothek es setzt.
- Die ganze Beschreibung als SDDL, Beispiel Such-Pipe: `O:<eigene>D:P(A;;0x12019F;;;<eigene>)(A;;0x12019B;;;<package>)S:(ML;;NW;;;LW)`.
- Nie eine ACE für `S-1-1-0` (Everyone), `S-1-5-11` (Authenticated Users), `S-1-5-32-545` (Users), `S-1-5-32-544`,
  `S-1-5-4` (Interactive), `S-1-5-2` (Network), `S-1-5-7` (Anonymous), `S-1-15-2-1`, `S-1-15-2-2`.
- **Der Name bleibt belegt.** Bevor eine verbundene Instanz an den Aufrufer geht, legt die Bibliothek die nächste
  wartende Instanz an, solange `instanzen` es zulässt. Ist die Grenze erreicht, bleibt die verbundene Instanz der Halter des
  Namens. Weitere Instanzen darf nur die eigene SID anlegen (die DACL gibt anderen kein `FILE_CREATE_PIPE_INSTANCE`).
- **Flush vor dem Trennen** (Projektentscheid, 25.11.2026; **OPEN: noch nicht gebaut**). Vor jedem `DisconnectNamedPipe` ruft die
  Bibliothek `FlushFileBuffers` auf dem Handle der Instanz auf, mit einer Frist. Die Frist ist die `hallo_frist` der
  Server-Vorgabe (Vorgabe 2000 ms; sie entspricht der üblichen `frist` des Clients von 2000 ms in den Fällen). Läuft die
  Frist ab, trennt die Bibliothek trotzdem. Wie sie die Frist umsetzt, bleibt ihr überlassen; der Aufrufer wartet höchstens
  diese Frist. Das gilt für jedes Trennen einer Instanz: Fallenlassen der `Verbindung`, Ablehnung nach Schritt 3 bis 6 und
  `ablehnen`. Grund: `DisconnectNamedPipe` verwirft Bytes, die der Client noch nicht gelesen hat. Ein Server, der direkt nach
  `bestaetigen()` die `Verbindung` fallen lässt, nähme dem Client sonst das Hallo `pk1` weg; `FlushFileBuffers` kehrt erst
  zurück, wenn der Client alles gelesen hat. Bei einer Ablehnung liegt kein Byte für den Client bereit, der Flush kehrt sofort
  zurück. Unter Unix gibt es keinen Flush (hergeleitet: `close` liefert bereits gesendete Bytes noch aus). Fall und Gegenprobe:
  `d-flush-vor-trennen-windows` (der Server lässt sofort fallen, der Client bekommt `pk1`; Teil 1 mit einem rohen Client, der
  erst 300 ms nach dem Hallo liest, ist ohne den Flush rot) und `d-flush-frist-windows` (der Client liest nie, der Server
  trennt nach der Frist).

**Zugriffsmaske des Clients.** Der Client öffnet mit `GENERIC_READ | FILE_WRITE_DATA`, nie mit `GENERIC_WRITE`.
Grund: Er fordert nur die Rechte an, die er braucht; `GENERIC_WRITE` enthält `FILE_APPEND_DATA` (0x4, an der Pipe
`FILE_CREATE_PIPE_INSTANCE`), das die DACL anderen als dem Server nicht gibt. Nachtrag 24.11.2026 (gemessen in der
Windows-VM, Server unter einem Admin- und unter einem Standardkonto): Ein Client mit `GENERIC_READ | GENERIC_WRITE` öffnet
die Instanz trotzdem ohne Fehler. alt: „der Aufruf scheiterte dann mit Fehler 5“. Die Sperre gegen fremde Instanzen liegt
beim Anlegen (`CreateNamedPipeW`, S3), nicht beim Öffnen; warum das Öffnen 0x4 nicht prüft, ist nicht untersucht. Fassung 1
kennt nur den Bytemodus. Braucht ein Aufrufer später
den Nachrichtenmodus (`SetNamedPipeHandleState`), kommt `FILE_WRITE_ATTRIBUTES` dazu (in `0x0012019B` enthalten;
hergeleitet, in der VM zu messen).

**Unix.** `socket(AF_UNIX, SOCK_STREAM | SOCK_CLOEXEC)`, `bind`, dann `chmod` auf den Modus der Vorgabe, erst danach
`listen`. Zwischen `bind` und `chmod` erreicht den Socket niemand sonst, denn der Ordner ist 0700. Modus 0600 bei gleicher
Kennung von Server und Client; 0660 nur mit gid in der Erlaubnis; 0666 nur, wenn der Einschluss dem Client eine eigene
Kennung gibt und nur der Socket gebunden ist (wie Such-Socket, Drehbuch E3). Unter Linux setzt die Bibliothek
`SO_PASSCRED` nicht und braucht es nicht. Beim Ende des Servers (`Drop`) löscht sie den Socket, wenn Inode und Gerät noch
die eigenen sind.

## Annehmen und Messen

Je Verbindung in dieser Reihenfolge. Jede Ablehnung schließt die Verbindung und gibt `Annahme::Abgewiesen(Ereignis)`
zurück. Die Bibliothek liest bis dahin genau die 4 Byte des Hallos und sendet nichts. Jede Stufe hat feste Gründe für
das Feld `grund` des Ereignisses, in der genannten Reihenfolge. Von außen sieht der Client nur: Verbindung ohne Antwort
geschlossen (`pk.abgewiesen`).

1. **Verbindungszeitpunkt** festhalten. Windows: Rückkehr von `ConnectNamedPipe`. Unix: Rückkehr von `accept`.
2. **Prozess binden**, sofort. Windows: `GetNamedPipeClientProcessId`, dann `OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION)`
   und `GetProcessTimes`. Linux: `SO_PEERCRED` (pid, uid, gid) und `SO_PEERPIDFD`; Startzeit aus `/proc/<pid>/stat`
   Feld 22, gelesen, während die pidfd gehalten wird. macOS: `LOCAL_PEERCRED` (uid, primäre gid), `LOCAL_PEERPID`,
   `LOCAL_PEERTOKEN` (Audit-Token mit pidversion), Startzeit über `proc_pidinfo`. Im selben Schritt das Programm:
   Windows `QueryFullProcessImageNameW` am Handle; Linux `readlink /proc/<pid>/exe`, danach prüft die Bibliothek an der
   pidfd, dass der Prozess noch lebt; macOS `proc_pidpath` und die Code-Signatur über das Audit-Token
   (`SecCodeCopyGuestWithAttributes` mit `kSecGuestAttributeAudit`, dann Kennung, Team-ID und `SecCodeCheckValidity`)
   sowie das Sandbox-Kennzeichen (`sandbox_check_by_audit_token`). Was sich nicht messen lässt, ist `null`.
3. **`pk.hallo_ungueltig`:** Binnen `hallo_frist` kommen nicht genau die 4 Byte des Hallos. Unter Windows muss der
   Server vor dem Impersonieren lesen (sonst Fehler 1368, gemessen 18.11.); darum steht das Hallo vor der Messung der
   Kennung. Das Ereignis trägt unter Windows `peer: null`, unter Unix den Peer aus Schritt 2. Die empfangenen Bytes stehen
   nie im Ereignis. Gründe: `pk.hallo_fehlt` (Frist oder Ende der Verbindung vor vier passenden Byte), `pk.hallo_falsch`.
   Nachtrag (Projektentscheid, 25.11.2026): alt: `pk.hallo_fehlt` galt für die Frist; was bei einem Ende der Verbindung
   vor vier Byte gilt, stand nicht im Vertrag. neu: Sind die empfangenen Bytes kein Anfang von `pk1` LF, gilt
   `pk.hallo_falsch`, sobald sie da sind (auch bei weniger als vier Byte und auch, wenn die Verbindung danach endet). Endet
   die Verbindung oder die Frist, bevor vier passende Byte da sind, gilt `pk.hallo_fehlt`. Beispiele: leer, `p`, `pk` und
   `pk1` mit Ende oder Frist gelten als `pk.hallo_fehlt`; `x`, `pq`, `pk1` mit einem anderen vierten Byte als LF (etwa
   CR) und `GET /` gelten als `pk.hallo_falsch`. Fälle: `n-hallo-ende-nach-pk`, `n-hallo-ende-ohne-byte`,
   `n-hallo-falscher-anfang`; `n-hallo-fehlt` und `n-hallo-falsch` bleiben unverändert.
4. **`pk.peer_unmessbar`:** Die Kennung lässt sich nicht messen. Windows: `ImpersonateNamedPipeClient`, `OpenThreadToken`,
   `GetTokenInformation` mit `TokenUser` und `TokenAppContainerSid`, danach sofort `RevertToSelf`. Unix: `SO_PEERCRED`
   bzw. `LOCAL_PEERCRED` scheitert. Ebenso, wenn `prozess_pflicht` gilt und Schritt 2 scheiterte, oder wenn `job` gesetzt
   ist und `IsProcessInJob` nicht antwortet. Gründe: `pk.messung_gescheitert`, `pk.prozess_nicht_offen`, `pk.job_unbekannt`.
5. **`pk.prozess_gewechselt`:** Der gebundene Prozess ist nicht der, der verbunden hat. Linux: Die pidfd meldet einen
   beendeten Prozess, oder `/proc/<pid>` gehört nicht mehr zu ihr. macOS: Die pidversion aus dem Audit-Token passt zu
   keinem lebenden Prozess dieser pid. Windows: Die Startzeit des geöffneten Prozesses liegt nach dem
   Verbindungszeitpunkt, oder der Prozess ist schon beendet (`GetExitCodeProcess` nicht `STILL_ACTIVE`). Konnte Schritt 2
   den Prozess nicht öffnen und gilt `prozess_pflicht` nicht, entfällt diese Prüfung, und `startzeit` ist `null`.
   Gründe: `pk.prozess_beendet`, `pk.startzeit_weicht_ab`, `pk.pidversion_weicht_ab`.
6. **`pk.kennung_nicht_erlaubt`:** Die gemessene Kennung steht nicht in `erlaubt`. Windows: zuerst `pk.eingeschraenkt`
   (das Token trägt Restricting SIDs, `IsTokenRestricted`), dann `pk.sid_nicht_erlaubt` (keine Erlaubnis mit dieser SID),
   dann `pk.appcontainer_falsch` (keine Erlaubnis mit diesem Paar aus `sid` und `appcontainer_sid`); ein Prozess im
   Container gilt nie als die Person ohne Container. Unix: `pk.uid_nicht_erlaubt`, dann `pk.gid_falsch`.
7. **Haken des Aufrufers (Nachtrag 5).** `annehmen` gibt `Annahme::Vorlaeufig` zurück, noch ohne ein Byte an den Client.
   Der Aufrufer prüft über `peer()` und `prozess()`, etwa die Sitzung beim Plattformdienst `peer`. Während des Hakens
   hält die Bibliothek die Prozessbindung (Handle, pidfd, Audit-Token). `bestaetigen()` sendet das Hallo zurück und
   übergibt die `Verbindung`. `ablehnen(grund)` schließt ohne Antwort und gibt das Ereignis mit Code
   `pk.aufrufer_abgewiesen` und Grund `aufrufer:<kennung>` zurück (Kennungsform, etwa `aufrufer:sitzung-fremd`). So geht
   er unverändert in `kanal.abgewiesen` (Vertrag 5). Wird `Vorlaeufig` ohne Entscheidung fallen gelassen, schließt die
   Bibliothek ebenfalls ohne Antwort. Es gibt dann kein Ereignis, weil niemand es annimmt. Dauert der Haken länger als
   die `frist` des Clients, meldet der Client `pk.abgewiesen`. Grund für den Haken: Ohne ihn sähe ein Lauf einer
   anderen Sitzung im selben Projekt erst das Hallo und dann das Ende. Er erführe so, dass er bis zur Listenprüfung
   gekommen ist. Jeder Aufrufer geht durch den Haken. Wer nichts weiter prüft, ruft sofort `bestaetigen()`.

Danach ergibt ein Fehler beim Lesen oder Schreiben `pk.io` mit `os_fehler`.

**Reihenfolge von Ereignis und Ende (Projektentscheid, 25.11.2026).** `Server::bei_abweisung(haken)` setzt einen Haken für
Ereignisse; ein späterer Aufruf ersetzt ihn. Bei jeder Ablehnung (Schritt 3 bis 6 und `Vorlaeufig::ablehnen`) baut die
Bibliothek zuerst das Ereignis, ruft den Haken mit ihm, trennt danach die Verbindung (unter Windows mit dem Flush des
Abschnitts Windows) und gibt erst dann dasselbe Ereignis zurück (`Annahme::Abgewiesen` bzw. Rückgabe von `ablehnen`).
alt: „Jede Ablehnung schließt die Verbindung und gibt `Annahme::Abgewiesen(Ereignis)` zurück“, die Reihenfolge von Ende
und Rückgabe stand nicht fest. neu: Ereignis, dann Ende, dann Rückgabe. Grund: Wer seine Protokollzeile im Haken schreibt,
hat sie, bevor der Client das Ende sieht. Der Haken läuft auf dem Faden von `annehmen` bzw. `ablehnen` und soll bald
zurückkehren; solange er läuft, ist die Verbindung noch nicht getrennt. Ohne Haken bleibt es bei der Rückgabe. Wird
`Vorlaeufig` ohne Entscheidung fallen gelassen, gibt es kein Ereignis und keinen Aufruf des Hakens (Schritt 7). Fälle:
`n-ereignis-vor-dem-trennen` (Ablehnung der Bibliothek) und `n-ereignis-vor-dem-trennen-aufrufer` (`ablehnen`).

**Lesefrist (Projektentscheid, 25.11.2026).** `Verbindung::lesefrist(&mut self, Option<Duration>)` begrenzt jedes folgende
Lesen auf der Verbindung, auf der Server- wie auf der Clientseite und auf allen Systemen gleich. Läuft die Frist ab, ohne
dass ein Byte kam, endet das Lesen mit einem `io::Error` der Art `io::ErrorKind::TimedOut`; die Verbindung bleibt offen,
ein späteres Lesen darf weitergehen. `None` liest ohne Frist und ist der Startwert. Die Frist gilt für jedes Lesen einzeln,
nicht für alle zusammen, und gilt bis zum nächsten Aufruf von `lesefrist`. Das Ende der Gegenseite bleibt ein Lesen mit
0 Byte, nicht `TimedOut`. Schreiben begrenzt sie nicht. Alle anderen Fehler beim Lesen bleiben `pk.io` mit `os_fehler`. Fall:
`d-lesefrist`.

**Was der Server nicht sieht.** Lehnt schon das System ab (DACL, `PIPE_REJECT_REMOTE_CLIENTS`, Ordner 0700), kommt keine
Verbindung an, und es gibt kein Ereignis. Die Liste bleibt trotzdem Pflicht: Sie trennt, was die DACL nicht trennt (das
Paar mit Package-SID; Unix bei 0666).

**Messung durch einen Dienst unter `NT SERVICE`.** Ein solcher Dienst darf den Prozess der Person nicht öffnen (gemessen
18.11., win-bfe Teil c). Dann ist `startzeit` `null` und `im_job` `null`. Die Kennung kommt dennoch sicher aus dem
impersonierten Token (100 von 100, gemessen 18.11.). Wer Startzeit oder Job braucht, setzt `prozess_pflicht` und läuft so,
dass er den Prozess öffnen darf (Modellweg: der Dienst, der den Job hält). Der Server braucht `SeImpersonatePrivilege`;
virtuelle Dienstkonten haben es.

## Peer

| Feld | Windows | Linux | macOS |
|---|---|---|---|
| `system` | `windows` | `linux` | `macos` |
| `sid` | Text, `TokenUser` des impersonierten Tokens | `null` | `null` |
| `appcontainer_sid` | Text aus `TokenAppContainerSid`, `null` außerhalb eines Containers | `null` | `null` |
| `uid`, `gid` | `null` | aus `SO_PEERCRED` | aus `LOCAL_PEERCRED` (gid = erste Gruppe) |
| `pid` | `GetNamedPipeClientProcessId` | `SO_PEERCRED` | `LOCAL_PEERPID` |
| `startzeit` | u64, `FILETIME` der Erzeugung, oder `null` | u64, Feld 22 von `/proc/<pid>/stat` (Takte seit Systemstart) | u64, Mikrosekunden seit 1970 aus `proc_pidinfo`, oder `null` |
| `im_job` | bool, `IsProcessInJob` gegen `job`; `null` ohne `job` oder ohne Prozess | `null` | `null` |
| `programm` | Bildpfad aus `QueryFullProcessImageNameW`, oder `null` | `/proc/<pid>/exe`, oder `null` | `proc_pidpath`, oder `null` |
| `signatur` | `null` | `null` | `{kennung, team_id, gueltig}` aus dem Audit-Token, oder `null` ohne Signatur |
| `eingeschraenkt` | bool: Token mit Restricting SIDs | `null` | `null` |
| `sandbox` | `null` | `null` | bool: Prozess unter Sandbox (Seatbelt) |

**Was in die Bibliothek gehört (Nachtrag 3).** Programm, Signatur, Restricting SIDs und Sandbox-Kennzeichen sind
Messungen am gebundenen Prozess oder am Token. Sie gehören darum in den Peer-Wert; nur die Bibliothek hält Handle,
pidfd und Audit-Token zum richtigen Zeitpunkt. Die Entscheidung bleibt beim Aufrufer (Grundsatz 8): Die Bibliothek weist
von sich aus nur Restricting SIDs ab (`pk.eingeschraenkt`), weil dieselbe SID sonst als die Person durchginge. Ein Sandbox-Kennzeichen weist sie
nicht ab; Such-Socket und Stelle erwarten den Agenten unter Seatbelt als Client. Rolle aus Programm oder Signatur, und
„Agent immer abweisen“, entscheidet der Halter.

**Grenzen der Programmmessung (hergeleitet).** Ein Dienst unter `NT SERVICE` öffnet den Prozess der Person nicht (gemessen
18.11.); dann ist `programm` `null`. Unter Linux ist `/proc/<pid>/exe` eines Prozesses mit `PR_SET_DUMPABLE 0` oder einer
fremden uid nur für root lesbar; dann ist `programm` `null` (Fall `n-programm-linux`).

**Rolle des Halters aus der gid (Nachtrag 25.11.2026, Projektentscheid, 25.11.2026).** Unter Linux trägt der Unix-Peer schon in
Fassung 1 `gid` aus `SO_PEERCRED`; das ist die **effektive** gid zum Zeitpunkt des `connect` (unix(7); Fassung 2, Annahme P2-1). Der
Halter nimmt die Rolle eines Peers unter Linux aus dieser effektiven gid (Halter-Vertrag 2.3 und 2.3a: die gid einer Rollengruppe aus
`halter.json`), nicht mehr aus `programm`. alt: „Rolle aus Programm oder Signatur […] entscheidet der Halter“ galt unter Linux für
`programm`. Grund: Ein Rollenprogramm mit `PR_SET_DUMPABLE 0` (der Modellweg) zeigt kein `programm`; die gid bleibt. Die Bibliothek
ändert dafür nichts; `programm` bleibt ein Messwert des Peers.

`startzeit` ist nur auf derselben Maschine im selben Systemlauf vergleichbar; der Aufrufer vergleicht nur auf Gleichheit.
Die PID allein ist kein Identitätsbeweis; sie gilt nur zusammen mit dem sofort gebundenen Prozess und der Startzeit
(Nachtrag 1, Punkt 2). Eine Session-ID gibt es nicht (Nachtrag 1, Punkt 3).

**Ereignis:** `{"ereignis": "abgewiesen", "kanal": <name>, "code": <Code aus Schritt 3 bis 7>, "grund": <fester Grund dieses Codes>, "peer": <Peer oder null>}`.
Die Gründe tragen die Vorsilbe `pk.` und gehen unverändert in die Zeilenart `kanal.abgewiesen` aus Vertrag 5 (Stand vom
20.11.2026). Es gibt eine Liste für alle Nutzer, mit `code` als Gruppe:

| Code | Gründe in Prüfreihenfolge |
|---|---|
| `pk.hallo_ungueltig` | `pk.hallo_fehlt`, `pk.hallo_falsch` |
| `pk.peer_unmessbar` | `pk.messung_gescheitert`, `pk.prozess_nicht_offen`, `pk.job_unbekannt` |
| `pk.prozess_gewechselt` | `pk.prozess_beendet`, `pk.startzeit_weicht_ab`, `pk.pidversion_weicht_ab` |
| `pk.kennung_nicht_erlaubt` | `pk.eingeschraenkt`, `pk.sid_nicht_erlaubt`, `pk.appcontainer_falsch`, `pk.uid_nicht_erlaubt`, `pk.gid_falsch` |
| `pk.aufrufer_abgewiesen` | `aufrufer:<kennung>`, vom Aufrufer gewählt (Schritt 7) |

**Prozessbindung für Dritte.** `Verbindung::prozess()` gibt die Bindung heraus: `pid`, `startzeit` und den gehaltenen
Bezug (Windows das Prozess-Handle, Linux die pidfd, macOS das Audit-Token). Die Bibliothek hält den Bezug, solange die
Verbindung lebt. Fragt der Aufrufer einen anderen Dienst nach dieser pid (Such-Pipe: Plattformdienst `peer`, pid zu
Sitzung), gibt er **pid und startzeit** mit; der Dienst öffnet den Prozess selbst und antwortet nur, wenn dessen Startzeit
gleich ist. Grund: Ein Handle oder eine pidfd lässt sich nicht ohne zusätzliche Rechte in einen anderen Prozess reichen
(`DuplicateHandle` braucht ein Handle auf den Zielprozess, unter Linux ginge `SCM_RIGHTS` nur über einen Unix-Socket).
Die Startzeit ist ein Wert, den der Dienst selbst nachprüft. Solange die Verbindung lebt, hält die Bibliothek unter Windows
das Handle offen; eine pid mit offenem Handle vergibt Windows nicht neu (hergeleitet aus der Win32-Doku, nicht gemessen).
So ist zwischen Messung und Frage keine Wiederverwendung möglich, und eine spätere wird an der Startzeit erkannt. Ist
`startzeit` `null` (Prozess nicht öffenbar), taugt die pid nicht für eine solche Frage; der Aufrufer setzt dann
`prozess_pflicht`.
Der Aufrufer schreibt es in sein Protokoll. Die Bibliothek gibt jede Ablehnung als Ereignis zurück, die nach dem System
ankam; keine bleibt still.

## Client verbinden (Gegenrichtung)

Prüfreihenfolge:

1. `pk.vorgabe_ungueltig`: Form des Schemas, `erwartet` passt nicht zum System, `unix_ordner` fehlt unter Unix.
2. `pk.nicht_erreichbar`: Kein Server unter dem Namen, oder alle Instanzen belegt bis zum Ende von `frist`. Windows:
   Fehler 2, oder 231 nach `WaitNamedPipeW` bis `frist`. Unix: `ENOENT`, `ECONNREFUSED`, oder `EAGAIN` bis `frist`.
3. `pk.zugriff_verweigert`: Das System lehnt ab (Windows Fehler 5 an der DACL oder ein entfernter Client; Unix `EACCES`).
4. `pk.server_unmessbar`: Die Kennung des Servers lässt sich nicht lesen.
5. `pk.falscher_server`: Windows: Der **Besitzer der Pipe-Instanz** (`GetSecurityInfo` auf den eigenen Handle,
   `OWNER_SECURITY_INFORMATION`) ist nicht die erwartete SID. Nicht über `GetNamedPipeServerProcessId` und
   `OpenProcess`; das scheitert für die Person gegen einen Dienst unter `NT SERVICE` mit Fehler 5 (gemessen 18.11.,
   A115). Unix: uid des Servers aus `SO_PEERCRED`/`LOCAL_PEERCRED` ist nicht die erwartete. In beiden Fällen sendet der
   Client kein Byte, auch kein Hallo.
6. `pk.abgewiesen`: Der Server schließt nach dem Hallo ohne Antwort, oder seine Antwort kommt nicht binnen `frist`.

Windows öffnet mit `SECURITY_SQOS_PRESENT | SECURITY_IDENTIFICATION`: Der Server darf die Kennung des Clients lesen, aber
nicht in dessen Namen handeln. Der Peer am Client trägt die Besitzer-SID als `sid`, `pid` aus
`GetNamedPipeServerProcessId`, `startzeit` und `programm` `null`. Die Bibliothek liest `pid` direkt nach der
Besitzerprüfung, vor dem ersten Byte. Grund: Ein Server darf die Verbindung direkt nach seiner Antwort fallen lassen
(Entscheid 6); läse der Client die `pid` erst nach dem Hallo, hätte der Server die Instanz unter Umständen schon verlassen,
und `pid` wäre `null` (gemessen: 1 von 100 Läufen in `d-flush-vor-trennen-windows`, vor der Korrektur).
Unter Unix misst der Client den Server wie Schritt 2
(uid, pid, Startzeit, Programm, unter macOS die Signatur). Weitere Prüfungen (Programm, Signatur) macht der Aufrufer nach
`verbinden` und vor dem ersten Nutzbyte.

**Grenze:** Läuft der Server unter der SID der Person, beweist der Besitzer nur „ein Prozess der Person“. Echte Trennung
gibt es nur gegen einen Server unter eigenem Konto (Dienst, Halter, Stelle). Ob ein Prozess im AppContainer der Person eine
Pipe mit diesem Namen anlegen kann, ist nicht gemessen (Offen 2).

**Client ohne Hallo (Projektentscheid, 25.11.2026).** `verbinden_ohne_hallo(&ClientVorgabe)` ist der Weg für einen Server,
der kein `pk1` spricht (etwa die Steuer-Pipe eines Dienstes). Er prüft den Besitzer der Instanz genau wie `verbinden`: die
Schritte 1 bis 5 sind dieselben, die Codes sind dieselben, und vor der Prüfung geht kein Byte hinaus (0 Byte). Danach sendet
er kein `pk1` und wartet auf keine Antwort: Schritt 6 entfällt, `pk.abgewiesen` kommt nicht vor, und die `Verbindung` ist ein
Bytestrom, dessen erstes Byte das des Aufrufers ist. Die Codes in dieser Reihenfolge: `pk.vorgabe_ungueltig`,
`pk.nicht_erreichbar`, `pk.zugriff_verweigert`, `pk.server_unmessbar`, `pk.falscher_server`. `frist` begrenzt nur das Warten
auf eine freie Instanz (Schritt 2). Grenzen: (1) Es gibt keinen Weg ohne erwarteten Besitzer; `ClientVorgabe.erwartet` bleibt
Pflicht. Der Besitzer ist die einzige Prüfung der Gegenseite, und die Bibliothek prüft den Client nicht, denn auf der anderen
Seite steht kein `Server` dieser Bibliothek. (2) Gegen einen `Server` dieser Bibliothek ist der Weg falsch: Dessen erstes
gelesenes Byte wäre das des Aufrufers, also meist `pk.hallo_falsch` (ohne Byte: `pk.hallo_fehlt`). (3) Für einen Dienst unter `NT SERVICE` ist die erwartete SID
`Sid::dienst("<name>")` (Abschnitt Auskunft ohne Verbindung). Fälle: `c-ohne-hallo-windows` und `c-ohne-hallo-unix` (der
Besitzer stimmt: `ok`, 0 Byte vor der Prüfung, kein `pk1`), dazu je als Gegenprobe `c-ohne-hallo-falscher-besitzer-windows`
und `c-ohne-hallo-falscher-besitzer-unix` (`pk.falscher_server`, 0 Byte beim Hörer). Unter Unix prüft der Weg die uid des
Servers wie `verbinden` (hergeleitet aus „genau wie `verbinden`“; die Unix-Seite der Bibliothek gibt es noch nicht).

## Auskunft ohne Verbindung (Nachtrag 25.11.2026)

Zwei Aufrufe messen nur und schaffen weder eine Verbindung noch eine Erlaubnis. Sie sind reine Auskunft; welche Kennung der
Aufrufer in eine Liste schreibt oder als Besitzer erwartet, entscheidet er (Grundsatz 8).

- **Eigene SID (Projektentscheid, 25.11.2026).** `pub fn eigene_sid() -> Result<Sid, Fehler>` gibt die `TokenUser`-SID des
  Tokens des eigenen Prozesses. Ein Aufrufer darf sie als erwarteten Besitzer eines Servers benennen, den dasselbe Konto
  betreibt. Windows: `pk.io` mit `os_fehler`, wenn der Systemaufruf scheitert; `pk.io` ohne `os_fehler`, wenn die SID nicht
  die Form `Sid` hat (etwa `LOCAL SERVICE`, `S-1-5-19`). Linux und macOS: `pk.nicht_unterstuetzt`, ohne `os_fehler` und ohne
  Systemaufruf. Fälle: `k-eigene-sid-windows`, `k-eigene-sid-andere-form-windows`, `k-eigene-sid-unix`.
- **Dienst-SID (Projektentscheid, 25.11.2026).** `Sid::dienst("<name>") -> Result<Sid, Fehler>` gibt die SID des
  virtuellen Dienstkontos `NT SERVICE\<name>`: `S-1-5-80-` und fünf Zahlen. Sie entstehen aus SHA-1 über den Namen in
  Großbuchstaben als UTF-16LE; die 20 Byte gelten als fünf 32-Bit-Zahlen, je vier Byte little-endian. Der Name hat die
  Kennungsform, sonst `pk.vorgabe_ungueltig` ohne `os_fehler`. Der Wert ist reine Rechnung ohne Systemaufruf, also auf jedem
  System gleich; unter Windows ist er gleich der Ausgabe von `sc.exe showsid <name>`. Fälle: `k-dienst-sid`,
  `k-dienst-sid-form`.

Codes der Seite `auskunft` (Projektentscheid, 25.11.2026): `pk.vorgabe_ungueltig`, `pk.nicht_unterstuetzt`, `pk.io`.
`pk.nicht_unterstuetzt` ist **neu**: Der Aufruf hat auf diesem System keine Bedeutung. Der Code gehört zu keiner anderen
Seite und zu keinem Ereignis; das Schema führt ihn nicht gesondert, weil `erwartet.code` nur die Form `pk.<name>` verlangt.

## Sicherheitszusagen (prüfbar)

| Nr. | Zusage | Fälle |
|---|---|---|
| S1 | Belegter Name oder Pfad: `pk.name_belegt`, kein Rückfall, Bestand unverändert | `a-vorbeleger-*`, `a-zweimal-anlegen` |
| S2 | DACL nur eigene SID plus erlaubte SIDs und Package-SIDs, geschützt, keine Gruppen; Besitzer die eigene SID | `a-dacl-windows`, `v-sid-*`, `v-alle-pakete` |
| S3 | Zwischen zwei Verbindungen kann kein anderer Prozess den Namen übernehmen | `a-name-bleibt-belegt-windows` |
| S4 | Entfernte Clients werden abgewiesen | `n-remote-windows` |
| S5 | Das Paar (SID, Package-SID) gilt genau; Container und Job werden gemeldet | `n-container-*`, `n-anderer-container-windows`, `n-job-falsch-windows` |
| S6 | Fremde Kennung, die das System durchlässt: Ereignis, keine Verbindung | `n-fremd-unix`, `n-gid-falsch-unix`, `n-container-ohne-eintrag-windows` |
| S7 | Wiederverwendete PID oder toter Client: `pk.prozess_gewechselt` | `n-pid-wiederverwendung-*`, `n-client-tot-mac` |
| S8 | Socket-Ordner nicht genau 0700 der eigenen Kennung, Link oder fehlend: `pk.ordner_rechte` | `a-ordner-*` |
| S9 | Socket hat vor `listen` den Modus der Vorgabe | `a-modus-unix` |
| S10 | Falscher Server: `pk.falscher_server`, 0 Byte gesendet | `c-falscher-server-*`, `c-ordner-fremd-unix` |
| S11 | Kein Nutzbyte vor beiden Prüfungen; das Hallo steht nie im Ereignis | `d-daten-erst-nach-pruefung`, `n-hallo-*` |
| S12 | Die Prüfung des Servers braucht kein Recht am Prozess des Dienstes | `c-dienst-am-besitzer-windows` |
| S13 | Ein Token mit Restricting SIDs wird abgewiesen (Grund `pk.eingeschraenkt`) | `n-eingeschraenkt-windows` |
| S14 | Der Client fordert nie `FILE_CREATE_PIPE_INSTANCE` an; die Instanz trägt bei Package-SID das Label LW | `c-zugriffsmaske-windows` (Verbindung); Maske und Label prüft ein Einheitstest der Bibliothek, weil das System beide nicht erzwingt (Nachtrag 24.11.2026) |
| S15 | Lehnt der Haken ab, bekommt der Client kein Byte zurück, auch kein Hallo; die Prozessbindung gilt während des Hakens | `n-haken-lehnt-ab`, `n-haken-bindung-windows` |
| S16 | Auch ohne Hallo prüft der Client den Besitzer der Instanz vor jedem Byte; ein falscher Besitzer ergibt `pk.falscher_server` und 0 Byte (Nachtrag 25.11.2026) | `c-ohne-hallo-*` |
| S17 | Das Ereignis liegt beim Aufrufer, bevor die Verbindung getrennt wird; der Client sieht das Ende erst danach (Nachtrag 25.11.2026) | `n-ereignis-vor-dem-trennen*` |
| S18 | Unter Windows geht ein bestätigtes Hallo nicht durch das Trennen verloren, und das Trennen wartet höchstens die Frist (Nachtrag 25.11.2026; noch nicht gebaut) | `d-flush-vor-trennen-windows`, `d-flush-frist-windows` |

## Fälle und Prüfer

| Datei | Zweck |
|---|---|
| `peerkanal.schema.json` | Form der Vorgaben, des Peers, des Ereignisses und der Fälle (JSON Schema 2020-12) |
| `faelle/<name>.json` | `beschreibung`, `art` (`vorgabe` oder `aussentest`), `ort`, `konten`, `server`, `client`, `aufbau`, `handlung` |
| `erwartet/<name>.json` | `seite` (`vorgabe`, `anlegen`, `annehmen`, `verbinden`, `auskunft`), `ergebnis`, `code`, `ereignisse`, `peer`, `gemessen` |
| `bau-faelle.py` | erzeugt beide Ordner |
| `pruefen.py` | Paarung, JSON, Schema, Codes je Seite, Beispielkennungen |

- 78 Fälle: 18 `v-*` prüfen Vorgaben und sind schon mit dem Schema prüfbar. 60 Außentests: `a-*` Anlegen, `n-*`
  Annehmen, `c-*` Gegenrichtung, `d-*` Datenfluss, `k-*` Auskunft ohne Verbindung. Je Ort: 39 Windows, 34 Linux, 34 macOS
  (`pruefen.py` zählt die Außentests je Ort). Davon sind 17 neu (Nachtrag 25.11.2026): `k-eigene-sid-windows`,
  `k-eigene-sid-andere-form-windows`, `k-eigene-sid-unix`, `k-dienst-sid`, `k-dienst-sid-form`, `c-ohne-hallo-windows`,
  `c-ohne-hallo-unix`, `c-ohne-hallo-falscher-besitzer-windows`, `c-ohne-hallo-falscher-besitzer-unix`,
  `n-hallo-ende-nach-pk`, `n-hallo-ende-ohne-byte`, `n-hallo-falscher-anfang`, `n-ereignis-vor-dem-trennen`,
  `n-ereignis-vor-dem-trennen-aufrufer`, `d-lesefrist`, `d-flush-vor-trennen-windows`, `d-flush-frist-windows`. Die Seite
  `auskunft` der Erwartung ist neu (Aufruf ohne Verbindung); die 61 früheren Fälle und Erwartungen sind unverändert.
- `ort: windows` läuft in einer Windows-11-VM (eigener Schnappschuss, höchstens 30 min je Block, kein Token in die VM).
  `linux` läuft auf einem Linux-Prüfrechner, `mac` an einem Mac. Suiten mit Socket legen den Testordner unter
  `$HOME/.cache` (104-Byte-Grenze) und laufen mit und ohne macOS-TMPDIR.
- Alle SIDs, uids und gids sind Beispielwerte (zehnstellige Zahlen ab 10000). Der Außenläufer ersetzt sie über `konten`
  durch die Werte der Testmaschine. `pid` und `startzeit` stehen als `=client`, `=server` oder `=neuer_prozess`: der Wert,
  den der Läufer an dieser Seite misst. `system: =system` heißt das System des Laufs.
- `pruefen.py`: `<python mit jsonschema> -I rust/spec/peerkanal/pruefen.py`. Er prüft auch Fassung 2 (`fassung-2/`).
  Ohne jsonschema Exit 2 (alt: Exit 1); mit `--ohne-schema` prüft er bewusst ohne Schema und meldet es.
- **Gemessen am 20.11.:** `pruefen.py` grün (61 Fälle, 0 Fehler). Gegenproben siehe Messbericht: eine
  Vorgabe mit Everyone als gültig erwartet, eine echte SID in einem Fall und ein fehlendes Gegenstück machen ihn rot.
  **Gemessen am 25.11.2026 (nach dem Nachtrag):** `pruefen.py` rc 0 mit 78 Fällen und 0 Fehlern in Fassung 1 (Fassung 2: 17
  Fälle, 0 Fehler); `bau-faelle.py` ein zweites Mal ausgeführt ändert keine Datei. Die neuen Außentests sind wie die alten
  erst mit dem Bau prüfbar; was die Bibliothek heute von ihnen erfüllt, ist damit nicht gesagt.
- **Gemessen am 20.11. (Grundlage der Unix-Seite):** An einem `socketpair` liefern am Mac `LOCAL_PEERPID`,
  `LOCAL_PEERCRED` und `LOCAL_PEERTOKEN` (mit pidversion) pid und uid des eigenen Prozesses; unter Linux (Linux-Baurechner, Kernel 7.2)
  liefern `SO_PEERCRED` pid, uid und gid, und `SO_PEERPIDFD` gibt eine pidfd. Die Windows-Seite stützt sich auf die Probe
  vom 18.11. (Probenplan der Windows-Plattform, Frage 1 und 10, A115).

## Gegenproben für den Bau (Mutanten)

Jede dieser Änderungen muss mindestens einen Fall rot machen: ohne `FILE_FLAG_FIRST_PIPE_INSTANCE` (`a-vorbeleger-windows`);
DACL mit `S-1-5-32-545` (`a-dacl-windows`); Vergleich nur der SID ohne Package-SID (`n-container-ohne-eintrag-windows`);
Client ohne Besitzerprüfung (`c-falscher-server-windows`); Prüfung über `GetNamedPipeServerProcessId` und `OpenProcess`
(`c-dienst-am-besitzer-windows`); Startzeit nicht verglichen (`n-pid-wiederverwendung-*`); `chmod` nach `listen` oder
Ordner nicht geprüft (`a-modus-unix`, `a-ordner-*`); Löschen eines vorhandenen Sockets (`a-vorbeleger-unix`); Hallo vor der
Serverprüfung gesendet (`c-falscher-server-*`); verbundene Instanz übergeben, bevor die nächste wartet
(`a-name-bleibt-belegt-windows`); Client mit `GENERIC_WRITE` oder Instanz ohne Label LW (Einheitstest der Bibliothek;
der Außenfall `c-zugriffsmaske-windows` sieht beide nicht, gemessen 24.11.2026);
Restricting SIDs nicht geprüft (`n-eingeschraenkt-windows`); Hallo vor dem Haken gesendet (`n-haken-lehnt-ab`).
Nachtrag 25.11.2026: `verbinden_ohne_hallo` ohne Besitzerprüfung oder mit einem Byte vor der Prüfung
(`c-ohne-hallo-falscher-besitzer-*`); `verbinden_ohne_hallo`, das trotzdem `pk1` sendet (`c-ohne-hallo-windows`,
`c-ohne-hallo-unix`); Trennen vor dem Haken (`n-ereignis-vor-dem-trennen*`); Ende nach `pk` als `pk.hallo_falsch` oder ein
falscher Anfang als `pk.hallo_fehlt` (`n-hallo-ende-nach-pk`, `n-hallo-falscher-anfang`); eine Lesefrist, die nur das erste
Lesen begrenzt (`d-lesefrist`); `Sid::dienst` mit einem anderen Wert als `sc.exe showsid` (`k-dienst-sid`); `eigene_sid` mit
einer anderen SID als `TokenUser` (`k-eigene-sid-windows`); Trennen ohne `FlushFileBuffers` oder ohne Frist
(`d-flush-vor-trennen-windows`, `d-flush-frist-windows`).

## Nutzer und ihre Felder

| Nutzer | Seite | Braucht | Erlaubnis und Erwartung |
|---|---|---|---|
| Such-Pipe (Nachtrag zu Vertrag 4, `rust/spec/such-socket/windows/`) | Server | `sid`, `appcontainer_sid`, `pid`, `startzeit` mit `prozess_pflicht`; kein Job (`im_job` null); Unix nichts außer dem Socket | Windows: `{sid: Person, appcontainer_sid: Package-SID des Projekts}`; die Sitzung bindet erst der Plattformdienst `peer` mit pid und Startzeit im Haken (Schritt 7; Grenze oben), DACL wie im SDDL-Beispiel; Unix 0600 wie heute. `such-mcp` als Client erwartet die SID der Person (Grenze siehe oben). Sichtbar nach außen: `pk.abgewiesen`, Verbindung ohne Antwort geschlossen |
| Entscheidungsstelle (`rust/spec/stelle/` 13.2) | Server, eine Pipe bzw. ein Socket je Art | `sid`, `appcontainer_sid`; Linux `pidfd` für die Cgroup-Prüfung Hub gegen Agent | je Art eine eigene Vorgabe mit der SID dieser Art; ein `Abgewiesen` reicht die Hülle als Art `unbekannt` an den Kern, damit `stelle.aufrufer_nicht_erlaubt` im Protokoll steht |
| Weiterreicher des Modellwegs (`rust/spec/modellweg/`) | Server | `appcontainer_sid`, `pid`, `startzeit`, `im_job`; `prozess_pflicht` und `job` | erlaubt `{sid: Person, appcontainer_sid: Container des Laufs}`; den Lauf-Schein prüft der Modellweg selbst (A121) |
| Halter (Abschnitt 2.1) | Server unter `NT SERVICE\<halter>` bzw. unter der Person (Linux, macOS); Hub, Brücke und Oberfläche als Clients | `sid`/`uid`, unter Linux `gid` als Quelle der Rolle (Nachtrag 25.11.2026; alt: `programm`), Prozessbindung, `programm` (nur Messwert), `signatur`, `appcontainer_sid`, `eingeschraenkt`, `sandbox` | Clients erwarten die Dienst-SID des Halters am Besitzer der Instanz (OPEN 10 des Halters damit aufgelöst); unter Unix prüft der Client nach `verbinden` uid und Programm bzw. Signatur des Halters |

## Offen

1. Die Felder sind mit Vertrag 4, Stelle, Modellweg und dem Halter-Vertrag noch nicht abgestimmt; das macht das Projekt.
   Stelle (Abschnitt 13) und Modellweg (Nachtrag 1) übernehmen die Felder.
2. Nicht gemessen: ob ein Prozess im AppContainer eine Pipe unter einem globalen Namen anlegen und so einen Dienst
   vorbelegen kann; die erzwungene PID-Wiederverwendung unter Windows und macOS; die interaktive Sitzung (`WinSta0`).
3. Die TCP-Peer-Funktion für den Modellweg kommt später in `peerkanal` (Projektentscheid, 20.11.2026), nicht in Fassung 1:
   `GetExtendedTcpTable` (Besitzer-PID der Loopback-Verbindung), sofort `OpenProcess`, zweite Tabellenlesung (die
   Verbindung gehört noch derselben PID), `STILL_ACTIVE`, Startzeit, `TokenAppContainerSid`, `IsProcessInJob`; Ablehnung
   ohne Antwort binnen 500 ms.
4. Asynchrone Schnittstelle (etwa tokio) folgt bei Bedarf; Fassung 1 ist blockierend.
5. Halter unter Windows: Als `NT SERVICE` öffnet er den Prozess der Person nicht; `programm` und `startzeit` sind dann
   `null`, und die Rolle aus dem Bildpfad ist nicht messbar. Ausweg ist eine ACE für das Dienstkonto am Prozess (wie
   `SICHTBAR` in der Probe win-bfe) oder eine Rolle aus der SID. Das entscheidet der Halter-Vertrag.
6. Halter unter Linux: Er setzt `PR_SET_DUMPABLE 0`; dann liest der Client `/proc/<pid>/exe` des Halters nicht, und die
   Prüfung „uid und Programm“ aus Halter 2.1 hat nur die uid. Das ist mit dem Halter-Vertrag abzugleichen.
7. `sandbox_check_by_audit_token` steht nicht im öffentlichen SDK von macOS (hergeleitet); ob es dauerhaft verfügbar ist,
   ist zu prüfen. Fällt es weg, ist `sandbox` `null`.
8. Startet ein Aufrufer Dienste in Session 0, brauchen Fensterstation und Desktop `READ_CONTROL` für den Client (sonst
   `0xC0000142`, gemessen 18.11.). Das gehört zum Starter, nicht zu dieser Bibliothek.
9. **OPEN (Nachtrag 25.11.2026): Flush vor dem Trennen ist noch nicht gebaut.** Die Pflicht steht im Abschnitt Windows; die
   Bibliothek ruft heute `DisconnectNamedPipe` ohne `FlushFileBuffers` auf. `d-flush-vor-trennen-windows` und
   `d-flush-frist-windows` sind bis zum Bau nicht erfüllbar. Der Entscheid nennt die „Hallo-Frist des Clients“ als Frist; der
   Server kennt die `frist` des Clients nicht. Der Vertrag setzt darum die `hallo_frist` der Server-Vorgabe (Vorgabe 2000 ms) ein.
   Soll eine andere Frist gelten, ist dieser Satz zu ändern.
10. Die Fälle `c-ohne-hallo-unix` und `c-ohne-hallo-falscher-besitzer-unix` beschreiben die Unix-Seite von `verbinden_ohne_hallo`
    (uid des Servers wie bei `verbinden`). Die Bibliothek baut die Unix-Seite noch nicht; der Entscheid nennt Unix nicht
    ausdrücklich.
