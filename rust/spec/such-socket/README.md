# Vertrag 4: Such-Socket je Sitzung (`rangkern sitzung`)

Stand 20.11.2026. Auftrag: Projektentscheid, 20.11.2026, Durchstich Schritt 1, Meilensteinplanung, Abschnitt 5.4,
Punkt 4. Der Vertrag ergänzt den Vertrag `rangkern serve`; dessen Regeln gelten weiter, soweit hier nichts anderes steht.

Er hängt an vier Verträgen derselben Runde. Die Felder sind mit diesen Verträgen abgestimmt (20.11.2026):

| Vertrag | Ablage | Was dieser Vertrag daraus nimmt |
|---|---|---|
| 1 Start-Struktur | `rust/spec/start/` | `sitzung` und `route`; der Starter startet den Such-Prozess |
| 2 Einstellungsdatei | `rust/spec/einstellungen/` | Routen, Klassen, Kunden, Vorgabestufe |
| 3 `darf(einheit, route, kunde)` | `rust/spec/datentor/` | die Entscheidung je Treffer, Crate `datentor` |
| 5 Protokollzeile „Ausgabe gesperrt“ | `rust/spec/protokollzeile/` | die Zeile und ihr Schreiber |

Anforderungen: A34 (gefiltert wird vor der Ausgabe), A50 bis A52 und A35 über `darf`, A49 (kein Grund an das Modell).

## Zweck und Grenze

Ein Agent im Einschluss fragt die Wissenssuche über einen Socket. Eine Notiz, die seine Route nicht sehen darf, kommt bei
ihm nicht an: kein Titel, kein Pfad, kein Auszug, kein Verweis. Er erfährt nur die Zahl gesperrter Treffer.

**Die Route kommt aus dem Socket, nie aus der Anfrage.** Dafür gilt:

- Je Sitzung läuft ein eigener Such-Prozess. Er bekommt Route und Sitzung beim Start vom Starter (Vertrag 1), nicht vom
  Agenten. Er liest keine Umgebungsvariable.
- Der Such-Prozess läuft außerhalb des Einschlusses. Vault, Einstellungsdatei und Protokoll liegen außerhalb. In den
  Einschluss kommt nur der Socket dieser Sitzung, an einem festen Pfad.
- Eine Anfrage mit einem Feld für Route, Kunde, Sitzung oder Klasse ist ungültig. Sie wird nicht stillschweigend übergangen.
- Ändert der Agent seine Umgebung oder die Einstellung seines MCP-Werkzeugs, erreicht er trotzdem nur den Socket seiner
  Sitzung und damit nur ihre Route.

## Aufruf

`rangkern sitzung <vault> --socket <pfad> --route <schlüssel> --sitzung <schlüssel> --einstellungen <datei> --protokoll <ordner> [--socket-rechte 0600|0666] [--probelauf --marker <a,b,…> --marker-zaehlung <datei>]`

Dazu dürfen die Optionen für Vektoren und Einbettung von `rangkern serve` stehen (`--vectors`, `--index-db`, `--extra-db`,
`--embed`, `--embed-timeout-ms`), mit derselben Bedeutung. Die fünf Optionen vor `--socket-rechte` sind Pflicht. Jede Option
steht höchstens einmal und nur in voller Schreibweise.

**Abschirmung über `kernabbild`** (Projektentscheid, 21.11.2026; Vertrag `dienst`, Abschnitt 16, „Kernabbild-Grenze 1/1“). Als Erstes, vor dem
Lesen der übrigen Argumente, vor dem ersten Faden und vor jedem Socket, ruft der Prozess `kernabbild::abschirmen()` auf, in `main`
nach der Wahl des Unterbefehls. Das gilt für `rangkern sitzung` und `rangkern serve`; `search`, `tokens`, `snippet` und `--licenses`
nehmen den Schritt nicht. Der Schritt setzt auf jedem System `RLIMIT_CORE` weich 1 und hart 1 und unter Linux dazu
`prctl(PR_SET_DUMPABLE, 0)`. Dann kann ein Prozess derselben Kennung ihn weder mit ptrace untersuchen noch seine Einträge unter
`/proc/<pid>` lesen; diese gehören danach root (gemessen auf dem Linux-Baurechner am 20.11.). Unter macOS gibt es `PR_SET_DUMPABLE` nicht; dort gilt
allein die Grenze 1/1 (Nachtrag macOS, Abschnitt 5). Ist die harte Grenze beim Start schon 0, lässt sich 1 nicht erreichen; dann
gilt die Falltabelle von Vertrag `dienst`, Abschnitt 16, Punkt 4:

| harte Grenze | `core_pattern` | Entscheidung |
|---|---|---|
| nicht 0 (auch unbegrenzt) | wird nicht angesehen | weiter, Grenze 1/1 wird gesetzt |
| 0 | fehlt (macOS) | weiter ohne Setzen, Hinweiszeile (`MusterFehlt`) |
| 0 | beginnt nicht mit `\|` (Dateimuster) | weiter ohne Setzen, Hinweiszeile (`MusterDatei`) |
| 0 | beginnt mit `\|` (Pipe) | Abbruch, Grund `kernabbild.muster_pipe` |
| 0 | nicht lesbar | Abbruch, Grund `kernabbild.muster_nicht_lesbar` |

Dazu die zwei Abbrüche des Setzens: `kernabbild.abzug_sperren` (`prctl` verweigert) und `kernabbild.grenze_setzen` (`setrlimit`
verweigert). Jeder der vier Gründe endet mit **Exit 5**, genau einer Zeile `rangkern <befehl>: Kernabbild nicht abschirmbar:
<grund> (<abhilfe>)` auf stderr und nichts auf stdout, bevor ein Socket entsteht. Die Hinweiszeile ist
`rangkern <befehl>: kernabbild: <text>` auf stderr; danach läuft der Prozess normal weiter. Der Schritt kommt vor jeder
Argumentprüfung: Ein Aufruf mit Aufruffehler und nicht abschirmbarem Prozess endet mit 5, nicht mit 64. Kein Test, der
`serve` oder `sitzung` im eigenen Prozess aufruft, schirmt den Testprozess ab; der Schritt liegt in `main`.

Beim Start prüft der Prozess in dieser Reihenfolge. Die erste Verletzung beendet ihn ohne Ausgabe auf stdout:

| Exit | Wann |
|---|---|
| 5 | Die Abschirmung gegen Kernabbilder scheitert (vier Gründe, siehe oben). Das gilt vor jeder der folgenden Prüfungen |
| 64 | Aufruffehler (`EX_USAGE` aus `sysexits.h`, wie Vertrag 1; bis 21.11.2026 war es 2). Seit 23.11.2026 endet jeder Aufruffehler von `rangkern` mit 64, auch ohne Unterbefehl, mit fremdem Unterbefehl und bei `serve`, `search`, `tokens` und `snippet`: unbekannter Unterbefehl oder unbekannte Option, Kurzform, fehlende oder doppelte Option, fehlender Vault. `--route` oder `--sitzung` nicht in der Form `^[a-z0-9][a-z0-9._-]{0,63}$`. `--socket-rechte` nicht genau `0600` oder `0666`. Nicht alle drei oder keine von `--probelauf`, `--marker` und `--marker-zaehlung`, ein Marker außerhalb der Form `^[A-Z0-9-]{1,32}$`, ein leerer Eintrag oder mehr als 16 Marker |
| 78 | Einstellungsdatei fehlt, ist nicht lesbar oder nach Vertrag 2 ungültig. Route nicht in der Einstellungsdatei oder ohne Klasse. Protokollordner fehlt oder ist kein Ordner. Im Probelauf lassen sich Zähldatei oder Zeile `probelauf.zaehlung` beim Start nicht schreiben; die Zeile prüft der Prozess als einzige erst nach dem Socket (Abschnitt „Zählung der Marker“) |
| 74 | Am Socketpfad liegt schon etwas, auch ein Link oder eine fremde Datei; oder der Socket lässt sich nicht anlegen. Was dort liegt, bleibt unverändert |

Danach legt der Prozess den Socket an, lädt Notizen und Index und schreibt genau eine Zeile auf stdout, die
Bereitschaftszeile `{"bereit":true}` mit LF (genau diese 15 Bytes und LF), danach nichts mehr. Erst nach dieser Zeile darf
der Starter den Harness starten. Meldungen für Menschen gehen nach stderr.

**Rechte des Sockets.** Der Prozess setzt den Socket auf die Rechte aus `--socket-rechte`, ohne die Option auf 0600. Den
Wert wählt der Starter (Vertrag 1):

- 0600, wenn Such-Prozess und Agent dieselbe Kennung haben. So ist es im Einschluss ohne Vorrechte: bwrap bildet beide auf
  dieselbe uid ab (Befund, 20.11.2026). Ein Bind-Mount umgeht die Wegprüfung des Ordners, aber nicht die Rechte
  am Socket; 0666 würde ihn für jede andere Kennung öffnen, die den Pfad erreicht.
- 0666 nur, wenn der Einschluss dem Agenten eine eigene Kennung gibt (Drehbuch E3).

Der Ordner des Sockets liegt außerhalb des Einschlusses, hat den Modus 0700 und ist im Einschluss nicht sichtbar. Hat der
Agent eine eigene Kennung, gehört der Ordner nicht ihr. In den Einschluss bindet der Starter nur den Socket.

**Fester Pfad im Einschluss:** `/run/brain/such.sock`. Dort bindet der Starter den Socket ein. Das MCP-Werkzeug im
Einschluss verbindet sich mit genau diesem Pfad; eine Option, die ihn ersetzt, gibt es nur für Tests außerhalb des
Einschlusses.

Die Einstellungsdatei liest der Prozess einmal beim Start. Eine Änderung gilt erst für eine neue Sitzung.

**Ende.** Auf SIGTERM oder SIGINT löscht der Prozess seinen Socket und endet mit Exit 0. Wie der Prozess beim Ende des
Starters endet (etwa `PR_SET_PDEATHSIG` oder Aufräumen durch den Kern-Dienst), legt Vertrag 1 fest. stdin liest der Prozess
nicht, und sein Ende beendet ihn nicht.

## Zählung der Marker (Probelauf)

Projektentscheid, 20.11.2026, für den Probelauf des Drehbuchs (S3, S5). Es wird nur gezählt, es werden keine
Bytes mitgeschrieben. Grund: Auf der lokalen Route liefert die Suche Kundendaten aus, und was nie geschrieben wird, muss
niemand löschen. Ein Schalter, der Inhalte mitschreibt, wäre im Betrieb eine Schwachstelle. Der Weiterreicher des Modellwegs
(Vertrag 8) zählt in derselben Form.

- **Nur in der Betriebsart Probelauf (Projektentscheid, 20.11.2026):** Die Zähloptionen gelten nur zusammen mit dem Schalter
  `--probelauf`. Alle drei oder keine; sonst Exit 64. Ohne sie zählt und schreibt die Suche nichts.
- `--marker <a,b,…>`: 1 bis 16 Marker, je 1 bis 32 Zeichen aus `A-Z0-9-`, durch Komma getrennt. `--marker-zaehlung <datei>`:
  die Zähldatei.
- **Einsatz steht im Protokoll** (Reihenfolge nach der Tabelle oben, Entscheid vom 23.11.2026; bis dahin
  stand hier „nach dem Anlegen des Sockets“ für beides):
  1. Vor dem Anlegen des Sockets schreibt der Prozess die Zähldatei mit Zählerstand 0. Gelingt das nicht, endet er mit
     Exit 78, ohne Socket und ohne Zeile. Das gilt auch, wenn am Socketpfad schon etwas liegt: 78 kommt vor 74.
  2. Nach dem Anlegen des Sockets schreibt er eine Zeile der gemeinsamen Zeilenart `probelauf.zaehlung` aus Vertrag 5 in
     `brain-<JJJJ-MM>.jsonl`: `{"aktionsname":"probelauf.zaehlung","anzahl":<Zahl der Marker>,"ergebnis":"an","fassung":2,
     "programm":"suche","sitzung":"…","vorher":"…","zeit":"…"}`. Die Marker selbst stehen nie in der Zeile. Der
     Weiterreicher schreibt dieselbe Zeilenart mit `programm: modellweg` (Vertrag 8). Gelingt die Zeile nicht, löscht der
     Prozess den Socket wieder und endet mit Exit 78.

  Die Zeile kommt als einzige Prüfung mit 78 erst nach dem Socket, denn sie bleibt in der Kette. So steht für einen Start,
  der vorher scheitert (auch mit 74), keine Zeile in der Kette. Endet ein Start nach dem Schreiben der Zähldatei mit 74
  oder 78, entfernt der Prozess die Zähldatei, die er angelegt hat.
- Vor der Bereitschaftszeile schreibt der Prozess genau eine feste Zeile auf stderr: `such.probelauf: Marker werden gezählt`.
- Gezählt wird nur bei Antworten auf `search`, denn nur sie tragen Inhalt. Für jeden Marker zählt der Prozess, wie oft er
  byte-genau im Puffer der Antwort steht, kurz bevor der Puffer an den Socket geht. `antworten` ist die Zahl dieser Antworten.
- Die Datei hat die Rechte 0600 und wird nach jeder solchen Antwort atomar neu geschrieben: in eine Datei daneben, dann
  Umbenennen. Ihre Form ist RFC 8785, also sortierte Schlüssel ohne Leerraum:
  `{"antworten":n,"marker":{"MI-4F9":3,"MK-7Q2":0},"sitzung":"…"}`.
- Die Zählung kommt vor der Protokollzeile. Lässt sich die Datei nicht schreiben, gibt es keine Protokollzeile, die Zähler
  bleiben, und die Antwort ist `{"error":"protokoll"}`, ohne Treffer.
- Die Zählung ist eine Messung im Prüfling selbst. Gegenprobe ohne Prüfling: Der Läufer des Drehbuchs zählt die Marker
  zusätzlich in dem, was der Agent sichtbar ausgibt. Steht dort ein gesperrter Marker, ist der Schritt rot, egal was die
  Zählung sagt.
- Fälle: `v-marker-cloud`, `v-marker-lokal`, `x-zaehlung-gesperrt`, `e-marker-ohne-datei`, `e-zaehlung-ohne-marker`,
  `e-marker-form`, `e-marker-ohne-probelauf`, `e-probelauf-allein`, `e-zaehlung-ordner-fehlt`. Der Läufer prüft Inhalt,
  Rechte 0600 und die kanonische Form der Datei, die Zeile `probelauf.zaehlung` im Protokoll und die stderr-Zeile.

## Transport und Anfragen

- Unter Linux und macOS ist der Socket ein Unix-Socket (Stream). Unter Windows ist er eine Named Pipe; Aufruf,
  Sicherheit, Prüfung des Peers und `such-mcp` dort regelt der Nachtrag `windows/README.md`, mit eigenen Fällen und Läufer.
  Für macOS regelt der Nachtrag `macos/README.md` Pfad und Ordner des Sockets, die Prüfung des Peers über `getpeereid`
  und `such-mcp` (Stand 21.11.).
- Mehrere Verbindungen nacheinander oder zugleich sind erlaubt. Alle haben dieselbe Route. Auf einer Verbindung kommen die
  Antworten in der Reihenfolge der Anfragen. Das Ende einer Verbindung beendet den Prozess nicht.
- Je Anfrage eine Zeile UTF-8 mit LF, je Antwort genau eine Zeile JSON mit LF. Zeilenregeln, Länge (1 MiB), doppelte
  Schlüssel und Typprüfung wie bei `rangkern serve`.

Felder einer Anfrage. Jedes andere Feld macht die Anfrage ungültig, ausdrücklich auch `route`, `kunde`, `sitzung`, `klasse`,
`datenklasse`, `today`, `query_vector` und `vault`:

| Feld | Pflicht | Typ | Bedeutung |
|---|---|---|---|
| `op` | ja | Text | `search` oder `ping`. `quit` ist ungültig: Die Sitzung beendet der Starter, nicht der Agent |
| `id` | nein | Text oder ganze Zahl | kommt unverändert in der Antwort zurück; nie ins Protokoll |
| `query` | bei `search` | Text | die Frage |
| `k` | nein | ganze Zahl von 1 bis 50 | Zahl der Treffer, Vorgabe 5 |
| `space` | nein | Text oder `null` | wie bei `rangkern serve` |

Das Datum für `valid-until` ist das UTC-Datum beim Lesen der Anfrage. `ping` trägt nur `op` und `id`.

Antworten:

| Anfrage | Antwort |
|---|---|
| `ping` | `{"ok": true}`, dazu `id` |
| `search`, gelungen | `{"fallback": …, "hits": […], "gesperrt": n}`, dazu `id` |
| ungültig | `{"error": "bad_request"}`, dazu `id`, wenn die Zeile ein Objekt mit gültigem `id` ist |
| Vault oder Notiz nicht lesbar | `{"error": "unreadable"}`, dazu `id` |
| Protokollzeile nicht schreibbar | `{"error": "protokoll"}`, dazu `id`; keine Treffer, keine Zahl |

## Suche mit Tor

Sei `L` die ganze Trefferliste, die `rangkern serve` für dieselbe Anfrage im selben Augenblick gäbe, mit `k` so groß, dass
nichts gekürzt wird. Wegen der Präfix-Eigenschaft (Kandidatentiefe 40, seit 20.11.) ist `L[:k]` die heutige Antwort für `k`.

1. Für jeden Treffer in `L` ruft der Prozess `darf(einheit, route, kunde)` aus Vertrag 3 auf, mit der Route aus dem Start.
2. Eingaben aus dem Frontmatter der Notiz, gelesen im Augenblick der Anfrage:
   - `einheit` trägt die Klasse aus `datenklasse`. Fehlt das Feld oder hat es keinen Wert (YAML `null`), ist sie `null`;
     dann setzt `darf` die Vorgabestufe ein. Ein leerer Text oder eine unbekannte Klasse sperrt `darf` mit `einheit-ohne-klasse`. Ein Wert, der kein Text ist, geht so an `darf`, dass es mit `einheit-ohne-klasse` sperrt.
   - `kunde` ist das Feld `kunde`. Fehlt es oder hat es keinen Wert, ist er `null`. Ist es da, aber kein Text, geht ein leerer Text
     an `darf`. Die Notiz gilt dann als Kundendaten eines Kunden ohne Eintrag und ist auf jeder Route gesperrt.
   - Das Feld `class` des Vaults (source, knowledge, derived) hat mit der Datenklasse nichts zu tun.
3. `hits` sind die ersten `k` erlaubten Treffer aus `L`, in der Reihenfolge von `L`. **Das Tor wirkt vor dem Kürzen.** Ein
   gesperrter Treffer belegt keinen Platz. Es gibt höchstens `k` Treffer.
4. `gesperrt` ist die Zahl der gesperrten Treffer unter `L[:k]`. Das sind die Treffer, die ohne Tor in der Antwort gestanden
   hätten.
5. Ein erlaubter Treffer hat die Felder wie bei `rangkern serve`, mit zwei Ausnahmen für Verweise:
   - `abgeloest_durch` nennt nur erlaubte Notizen. Bleibt keine, fehlt das Feld. `status` bleibt.
   - `contradiction` steht nur, wenn mindestens ein offener Widerspruch eine erlaubte Partnernotiz hat.
6. Aus einer gesperrten Notiz kommt nichts in die Antwort: kein `rel`, kein Titel, kein Auszug, kein Verweis, kein `grund`,
   keine `meldung`, keine `klasse` aus `darf` (A34, A49).

Eine Notiz, die zwischen zwei Anfragen umklassiert wird, gilt bei der nächsten Anfrage mit der neuen Klasse. Wie bei
`rangkern serve` sieht jede Anfrage die Dateien so, wie sie beim Lesen ihrer Zeile auf der Platte stehen.

## Protokollzeile

Hat eine Antwort `gesperrt > 0`, schreibt der Prozess vor dem Absenden die Zeilen „Ausgabe gesperrt“ nach Vertrag 5. Er
nutzt dafür dessen Rust-Bibliothek im Ordner `--protokoll` und setzt die Zeit selbst (UTC, Millisekunden).

- Je Antwort eine Zeile je Gruppe aus (`klasse`, `kunde`, `grund`) der gesperrten Treffer in `L[:k]`, mit `anzahl` ab 1. Die
  Summe der `anzahl` ist `gesperrt`.
- Die Zeilen folgen der Reihenfolge, in der eine Gruppe in `L` zum ersten Mal vorkommt.
- `klasse` und `grund` sind die Werte aus der Antwort von `darf`; `klasse` ist `null` bei `einheit-ohne-klasse`.
- `kunde` ist das Feld `kunde` der Notiz, wenn es Text in Schlüsselform ist, sonst `null`.
- `route` und `sitzung` kommen aus dem Start. Die Zeile nennt keinen Pfad, keinen Titel und keinen Anfragetext.
- Schlägt das Schreiben fehl, kommt `{"error": "protokoll"}` und kein Treffer. Eine Sperre bleibt nie unprotokolliert.

Mehrere Sitzungen schreiben dieselbe Datei. Die Ordnung der Kette über Prozesse hinweg sichert die Bibliothek mit einer
exklusiven Dateisperre (Vertrag 5).

### Rückfall der Einbettung (B39, A125; ab dem Code-Schritt nach M1)

Projektentscheid, 22.11.2026 (Weg A), Form der Zeile am 22.11.2026 (Vertrag 5, Abschnitt 6.5). Läuft eine
Suche lexikalisch, weil die eingerichtete Einbettung ausgefallen ist, schreibt der Prozess eine Zeile `suche.rueckfall` in die
Kette `brain` im Ordner `--protokoll`. Die Diagnose zählt diese Zeilen; sie baut das Projekt nach M1. Bis zum Code-Schritt nach M1
schreibt der Prozess die Zeile nicht.

- **Wann.** Genau dann, wenn der Prozess mit `--embed` gestartet ist und `Embedder::embed` nach beiden Versuchen (`RETRIES` 1)
  keinen Vektor liefert. Die Antwort trägt dann `fallback: true`.
- **Wann nicht.** Ohne `--embed` ist die Suche lexikalisch per Einstellung: Die Antwort trägt `fallback: true`, aber es entsteht
  keine Zeile; die Diagnose liest diesen Fall aus der Einstellung. Steht keine Notiz der Vektordatei im Korpus, fragt der Prozess
  Ollama nicht (`rank_embedding` nimmt einen leeren Vektor); das ist kein Rückfall. Ein `ping` und jede Antwort mit `error`
  schreiben keine Zeile, auch nicht `{"error": "protokoll"}` nach einer gescheiterten Zeile „Ausgabe gesperrt“.
- **Eine Zeile je Antwort.** `route` und `sitzung` kommen aus dem Start, `zeit` setzt der Prozess wie bei „Ausgabe gesperrt“.
  Die Zeile nennt keine Anfrage, keine Notiz, keine Adresse und keine Fehlermeldung.
- **Grund** ist der Fehler des letzten Versuchs. Die drei Werte teilen alle Fehler eines Versuchs auf (Stellen in
  `rust/crates/rangkern/src/embed.rs`, Stand vom 22.11.2026, wo heute alle drei derselbe Fehler `Failed` sind):

| `grund` | Fehler des Versuchs |
|---|---|
| `keine_verbindung` | `connect`: Zu keiner Adresse des Endpunkts entsteht eine TCP-Verbindung, weil sie abgewiesen wird oder `CONNECT_MS` (oder der kleinere Rest der Frist) abläuft. |
| `frist` | Nach dem Verbindungsaufbau läuft die Frist des Versuchs ab: Ein Schreib- oder Lesevorgang endet mit `TimedOut` oder `WouldBlock`, oder der Rest ist null und das Setzen der Frist am Socket scheitert (`Deadline::left`). |
| `antwort_ungueltig` | Jeder andere Fehler nach dem Verbindungsaufbau: Status ungleich 200, Kopfzeile oder Körper über der Grenze, Verbindung vor dem Ende geschlossen, ein anderer Ein- oder Ausgabefehler, kein JSON, `embeddings` fehlt, ist leer oder kein Vektor (`first_vector`). |

- **Reihenfolge.** Die Zeile folgt nach der Zählung (Probelauf) und nach den Zeilen „Ausgabe gesperrt“ derselben Antwort, mit
  einem eigenen Aufruf (`anhaengen_rueckfall` oder `anhaengen_zeile`), nie im selben Aufruf wie diese.
- **Ein Schreibfehler ändert die Antwort nicht.** Scheitert das Anhängen, geht die Antwort unverändert hinaus, mit ihren
  Treffern und `fallback: true`. Es gibt keinen zweiten Versuch, keinen Puffer im Speicher, keinen Text auf stderr und kein Feld
  in der Antwort. Die Regel oben („Schlägt das Schreiben fehl, kommt `{"error": "protokoll"}`“) gilt weiter für die Zeilen
  „Ausgabe gesperrt“ und für die Zählung, nicht für diese Zeile. Grund: Die Zeile dient nur der Diagnose; ein gescheitertes
  Schreiben setzt die Monatsdatei auf ihre alte Länge zurück (Vertrag 5, Abschnitt 5.3), die Kette bleibt ganz.
- **Fälle** kommen mit dem Code-Schritt, mit der Attrappe der Einbettung aus dem Einbettungsvertrag von `rangkern serve` und einer
  Prüfung der Kette mit `kette pruefe`: Einbettung an (keine Zeile); aus (eine Zeile je Suche, `keine_verbindung`); Frist
  (`frist`); kaputtes JSON (`antwort_ungueltig`); wieder an (keine weitere Zeile); erster Versuch Frist, zweiter abgewiesen
  (`keine_verbindung`); ohne `--embed` (keine Zeile bei `fallback: true`); keine Notiz der Vektordatei im Korpus (keine Zeile);
  gesperrter Treffer und Rückfall in einer Antwort (erst „Ausgabe gesperrt“, dann `suche.rueckfall`); Protokollordner nach dem
  Start nicht mehr beschreibbar (Antwort gleich wie ohne Fehler, keine Zeile, stderr leer).
- **Gebaut** (23.11.2026): `embed.rs` teilt jeden Fehler eines Versuchs in die drei Gründe;
  `Embedder::einbetten` gibt den Grund des letzten Versuchs, `embed` bleibt mit denselben Versuchen. `rank_embedding` und
  `Server::search_hits_ausfall` geben den Grund weiter; `Server::search_hits` bleibt unverändert. `sitzung` hängt die Zeile mit
  `formate::kette::anhaengen_zeile(<protokoll>, "brain", …)` an und verwirft deren Fehler.
- **Sichtbare Fälle** `r-*` (elf): `r-an`, `r-aus` (mit `ping` und Fehlerantworten), `r-frist`, `r-kaputt`, `r-wieder-an`,
  `r-frist-dann-abgewiesen`, `r-ohne-embed`, `r-relikt` (keine Notiz der Datenbank im Korpus), `r-sperre-und-rueckfall`,
  `r-protokoll-gesperrt`, `r-vorgabe-ohne-lf` (eine vorgegebene Monatsdatei endet ohne LF: Sperre ergibt
  `{"error": "protokoll"}`, die Rückfallzeile fällt ohne Einfluss weg, die Datei bleibt unverändert). `bauen.py` nimmt die hybride Liste aus der Referenz mit `--vectors`, die lexikalische ohne.
  Der Läufer startet je Fall mit dem Feld `einbettung` eine Attrappe auf 127.0.0.1 mit freiem Port (Plan im Kopf von
  `pruefe.py`), baut die Datenbank aus `dbs` und prüft `einbettung_aufrufe` und `stderr_leer`. Jede Protokollzeile prüft er
  als Kette (kanonisch, `vorher`); mit `KETTE_BIN` fährt er zusätzlich `kette pruefe-quelle <protokoll> brain`. Eine
  Monatsdatei, die der Fall unter `files` vorgibt und die der Prozess nicht verändert, prüft er nur auf Gleichheit mit der
  Vorgabe; Kette, Zeilenzahl und `kette pruefe-quelle` gelten nur für Dateien, die der Prozess angelegt oder verändert hat.
  `bauen.py --pruefe` vergleicht den Satz mit der Platte, ohne zu schreiben.

## Nicht in diesem Vertrag

- Keine LLM-Sortierung. Sie kommt später hinter das Tor (A138).
- Kein Schreiben in den Vault. Das Schreibtor folgt nach dem Durchstich.
- Unter Linux keine Prüfung des Peers am Socket. Wer verbinden darf, regeln der Ordner des Sockets und seine Rechte. Eine
  zusätzliche Prüfung der Kennung des Peers (etwa `SO_PEERCRED`) kann später folgen. Unter macOS prüft der Prozess die
  euid des Peers (`macos/README.md`, Abschnitt 4), unter Windows den Peer über `peerkanal` (`windows/README.md`).
- Der Socket trennt nur die Suchantworten je Sitzung. Die Arbeitsordner schützt die Ordnermarke von Vertrag 1 (A156).
- Ein Prozess je Sitzung kostet je Sitzung den Speicher des Index (Messpunkt von `rangkern serve`: höchstens 200 MiB).
- Die Zahl `gesperrt` verrät, dass passende gesperrte Notizen existieren. Das ist gewollt: Die Vorführung zeigt „1 Treffer
  gesperrt“. Die Scores erlaubter Treffer hängen vom ganzen Korpus ab, auch von gesperrten Notizen.

## Klarstellungen (20.11., aus den verdeckten Fällen)

Die verdeckten Fälle und die Abnahme fanden vierzehn Stellen, die der Vertrag offen ließ. Es gilt:

1. **Doppelte Marker** in `--marker` (etwa `MI-1A,MI-1A`) sind ein Aufruffehler, Exit 64. Grund: Die Zähldatei hätte sonst einen
   doppelten Schlüssel.
2. **Leerer Wert** einer Option mit Wert (`--socket ""`, `--einstellungen ""`, `--protokoll ""` und alle anderen) ist ein
   Aufruffehler, Exit 64, vor jeder Prüfung von Datei, Ordner oder Socket.
3. **Zeile über 1 MiB:** Die Antwort ist `{"error":"bad_request"}` ohne `id`, auch wenn die Zeile mit einem gültigen `id`
   beginnt. Die Zeile wird nicht gelesen.
4. **Einsames Surrogat** als Escape (`\ud800` ohne zweite Hälfte) in einer Zeichenkette ist kein gültiges JSON (RFC 8259,
   Abschnitt 8.2). Die Antwort ist `{"error":"bad_request"}` ohne `id`, wie bei jeder Zeile, die kein JSON ist.
5. **Zählung und Protokollfehler:** Gezählt wird die Antwort, bevor die Protokollzeile geschrieben wird. Scheitert danach die
   Zeile, bleibt die Zählung stehen, und gesendet wird `{"error":"protokoll"}`. Die Zählung zeigt damit eher zu viel als zu
   wenig; sie kann ein Leck nie verbergen. Genau: Gezählt wird der Puffer der Inhaltsantwort mit Treffern, `fallback` und
   `gesperrt`, die ohne den Fehler gesendet worden wäre, nicht der Puffer `{"error":"protokoll"}`. `antworten` zählt sie mit.
   Fall: `x-zaehlung-dann-protokoll-gesperrt` (seit 23.11.2026; bis dahin zählte `bauen.py` in diesem Fall 0, W4).
6. **Überlappende Vorkommen** eines Markers zählen nicht doppelt. Gezählt wird von links nach rechts ohne Überlappung, wie
   `str::matches` in Rust und `str.count` in Python.
7. **`such-mcp`, JSON-RPC-Batch** (ein Array auf einer Zeile): Die Antwort ist ein einzelner Fehler `-32600` mit `id: null`.
   Der Socket wird nicht gefragt.
8. **`such-mcp`, `tools/call` ohne `id`** (Benachrichtigung): keine Antwort, kein Aufruf des Sockets.
9. **`such-mcp`, Form der Argumente:** `k` muss eine JSON-Ganzzahl ohne Bruchteil sein; `1.0`, `"5"` und `true` sind ungültig.
   `query` muss Text sein. Ein ungültiges Argument ergibt das Werkzeugergebnis mit `isError: true` wie ein fremdes Argument, ohne
   Aufruf des Sockets. Eine leere Zeile auf stdin wird übergangen, ohne Antwort.
10. **`such-mcp`, `initialize` ohne `protocolVersion`** oder mit einem Wert, der kein Text ist, ergibt `-32602` (Invalid
    params); die Fassung ist im MCP ein Pflichtfeld vom Typ Text. Eine unbekannte Fassung als Text ergibt weiter 2025-11-25.

11. **`--name=wert`** ist dieselbe Option wie `--name wert` (Regeln von argparse, wie `rangkern serve`). „Volle Schreibweise“
    heißt: kein Kürzel wie `--rout`. „Höchstens einmal“ gilt über beide Schreibweisen zusammen: `--route lokal` zusammen mit
    `--route=cloud` ist ein Aufruffehler, Exit 64.
12. **Die Abschirmung ist fail closed** (Projektentscheid, 20.11.2026, ersetzt am 21.11.2026 durch die Regel „Abschirmung über `kernabbild`“ oben):
    Scheitert einer der vier Schritte (`prctl`, `setrlimit`, Muster mit Pipe bei harter Grenze 0, Muster nicht lesbar), endet der
    Start mit Exit 5 und einer Zeile auf stderr, bevor ein Socket entsteht. Bis dahin war es Exit 78 und nur bei
    `PR_SET_DUMPABLE`; 78 gilt jetzt nur noch für Einstellungen, Route und Protokollordner.
13. **Höchstens 16 gleichzeitige Verbindungen** (Projektentscheid, 20.11.2026): Eine 17. gleichzeitige Verbindung wird angenommen und sofort
    ohne ein Byte geschlossen; die bestehenden bleiben bedienbar. Wird eine frei, ist wieder Platz. Grund: Der Agent im
    Einschluss darf den Such-Prozess nicht mit Verbindungen lahmlegen. Die Zahl ist fest, keine Option.
    Entscheid vom 24.11.2026: Kommt auf einer Verbindung 10 s lang kein Byte, schließt der Prozess sie ohne Antwort und ohne
    Protokollzeile; ihr Platz unter den 16 gleichzeitigen Verbindungen wird frei. Die Frist zählt ab dem letzten empfangenen Byte
    (auch mitten in einer Zeile) und gilt nicht, solange der Prozess an einer Antwort arbeitet.
14. **Kein Fenster zwischen `bind` und `chmod`** (Projektentscheid, 20.11.2026): Vor dem `bind` setzt der Prozess `umask 077`; danach setzt
    `chmod` den Wert von `--socket-rechte`. Der Socket ist so in keinem Augenblick für Gruppe oder andere offen.

Grenzen des Läufers, die verdeckt nicht geprüft werden: Monatswechsel während der Sitzung, ein verschwindender Protokollordner,
ein Socket, Ordner oder Link am Socketpfad, ungültiges UTF-8 und halbe Zeilen, ein stummer Socket hinter `such-mcp`. Diese
Punkte prüft die Abnahme am Bau mit eigenen Tests (Fertig-Kriterium der Bauaufträge).

## Fälle und Läufer

- `sichtbar/faelle/<name>.json` und `sichtbar/erwartet/<name>.json`: 87 Fälle (seit 24.11.2026 mit `x-leerlauf-schliesst`; 86 seit 23.11.2026 abends; davor 50, bis 23.11. früh 39). Die 36 neuen kommen aus den
  Entwürfen der Vertragsprüfung; acht davon nutzen Erweiterungen des Läufers (Kopf von `pruefe.py`). `p-*` prüfen die
  Anfrageform, `t-*` das Tor, `z-*` Änderungen zwischen Anfragen, `x-*` das Protokoll, `v-*` mehrere Verbindungen, `e-*` den
  Start, `r-*` den Rückfall der Einbettung.
- `pruefe.py` ist der Läufer, nur mit der Standardbibliothek: `RANGKERN_BIN=<programm> python3 pruefe.py [fall …]`. Der
  Prüfling bekommt nur `PATH`, `LANG=C.UTF-8`, `TZ=UTC` und `HOME=<root>/home`. Für jede Antwort prüft der Läufer die Treffer
  als Liste von `rel`, dazu `fallback`, `gesperrt`, `id` und einzelne Felder (`pruefe_felder`). Jede gesperrte Notiz trägt eine
  Marke (`qkx<n>`) in Pfad, Titel und Text. Steht eine Marke der Liste `verboten` irgendwo in einer Antwortzeile, ist der Fall
  rot. Nach der Bereitschaftszeile prüft er die Rechte des Sockets und unter Linux, dass `/proc/<pid>/stat` root gehört und dass
  die Zeile `Max core file size` von `/proc/<pid>/limits` weich 1 und hart 1 zeigt (Positivkontrolle: Kann er `limits` nicht lesen
  oder fehlt die Zeile, ist der Fall rot, nie grün).
  Die Protokollzeilen prüft er auf genau die Felder ihrer Zeilenart in Vertrag 5 und auf die Werte ohne `zeit` und `vorher`.
  Die Kette prüft `kette pruefe` aus Vertrag 5, sobald es gebaut ist.
  Seit 23.11.2026 (B11 des Reviews, OPEN 4 der Vertragsprüfung) prüft er zusätzlich: Die Bereitschaftszeile ist byte-genau
  `{"bereit":true}` mit LF, und danach kommt bis zum Ende nichts auf stdout. Die Zeile `such.probelauf` auf stderr liegt
  schon in der Pipe, wenn die Bereitschaftszeile gelesen ist. `zeit` jeder Zeile folgt dem Muster von Vertrag 5, passt
  zum Monat der Datei und liegt in der Laufzeit des Prozesses (2 s Spiel). Unter Linux gehört `/proc/<pid>/stat` root und
  nicht der Kennung des Läufers; läuft der Läufer als root, ist der Fall rot. `status` bei `abgeloest_durch` und das
  Bleiben von `contradiction` prüft er über `pruefe_felder`, das `bauen.py` dafür setzt.
- `bauen.py` erzeugt die Erwartungen. Die Liste `L` kommt aus der Referenz des Vertrags `rangkern`: dem Python-Vorbild über
  den sicheren Starter, ohne Vektoren, also über den Rückfallweg. Das Tor rechnet `bauen.py` nach den Gründen von Vertrag 3.
  Ändert sich Vertrag 3, wird `darf` in `bauen.py` angepasst und der Satz neu gebaut. Die Einstellungen der Fälle stehen dort
  ebenfalls. Weicht Vertrag 2 in einem Feldnamen ab, gilt Vertrag 2, und der Satz wird neu gebaut.
- Gemessen am 20.11.:
  - Gegen ein Programm, das sofort endet, sind 0 von 39 Fällen grün (Stand mit Probelauf und Zählung der Marker).
  - Gegen eine Attrappe des Vertrags in Python, außerhalb des Repos, sind 39 von 39 grün (am Mac, ohne die Linux-Prüfung von
    `PR_SET_DUMPABLE`).
  - Dieselbe Attrappe mit abgeschaltetem Tor: 16 rot, jeder mit der Marke der durchgesickerten Notiz.
- Verdeckte Fälle und Mutanten folgen mit dem Bau. Die Aufruffälle mit Exit 64 sind gegen ein Programm ohne `sitzung` schon
  grün; sie zählen nur zusammen mit den übrigen.

## MCP-Werkzeug im Einschluss (`such-mcp`)

Ein eigenes kleines Programm, Crate `rust/crates/such-mcp`. Es reicht Fragen des Agenten an den Socket seiner Sitzung weiter
und gibt die Antwort unverändert zurück. Es kennt weder Vault noch Einstellungen noch Protokoll. Es liest keine
Umgebungsvariable, schreibt keine Datei, hält keinen Zwischenspeicher und baut keine Netzverbindung.

- **Aufruf:** `such-mcp` verbindet sich mit `/run/brain/such.sock`. Nur für Tests außerhalb des Einschlusses gibt es
  `such-mcp --socket <pfad>`. Jede andere Angabe ergibt Exit 64 (`EX_USAGE`) ohne Ausgabe. Ein Lesefehler von stdin oder ein Schreibfehler von stdout ergibt Exit 74 (`EX_IOERR`; bis 21.11.2026 war es 1). Am Ende von stdin endet es mit Exit 0.
- **Transport:** MCP über stdio, je Nachricht eine Zeile JSON-RPC 2.0 in UTF-8 mit LF. stdout trägt nur Protokoll.
- **Methoden:** `initialize` mit den Fassungen 2025-11-25 und 2025-06-18. Die Antwort nennt die Fassung des Clients, wenn sie
  dazugehört, sonst 2025-11-25, dazu `capabilities: {"tools": {}}` und `serverInfo.name` `such-mcp`.
  `notifications/initialized` bekommt keine Antwort, `ping` ein leeres Ergebnis. Dazu kommen `tools/list` und `tools/call`.
  Eine unbekannte Methode ergibt -32601, eine Zeile ohne gültiges JSON -32700 mit `id: null`.
- **Werkzeug:** genau eines, `brain_suche`, mit dem Schema aus `mcp/faelle.json` (`query` Pflicht, `k` von 1 bis 50,
  keine weiteren Felder). Ein anderer Name in `tools/call` ergibt -32602.
- **Aufruf des Werkzeugs:** Jedes Argument außer `query` und `k` macht den Aufruf ungültig, ausdrücklich auch `route`.
  Ebenso ein `k` außerhalb 1 bis 50 oder als Text und ein fehlendes `query`. Dann kommt das Ergebnis mit dem Text
  „Ungültige Anfrage“ und `isError: true`, und der Socket wird nicht gefragt.
  Sonst öffnet das Werkzeug je Aufruf eine Verbindung und schreibt `{"op":"search","query":…}`, mit `k` nur, wenn es
  angegeben ist. Es liest genau eine Antwortzeile. Das Ergebnis ist `content: [{"type":"text","text":<Antwortzeile
  ohne LF>}]`. `isError` ist `true`, wenn die Antwort ein Objekt mit einem Feld `error` ist oder kein JSON-Objekt (kein
  gültiges JSON, eine Liste, eine Zahl, ein Text, `null`, `true`, `false`); nur ein Objekt ohne Feld `error` gibt `false`
  (fail closed: Eine Suche antwortet immer mit einem Objekt, alles andere ist gestört; Entscheid vom
  23.11.2026, bis dahin `false`). Der Text
  bleibt auch dann die Antwortzeile ohne LF. Antwortet der Socket nicht binnen 10 s oder ist
  er nicht erreichbar, lautet der Text „Suche nicht erreichbar“ mit `isError: true`.
- **Fälle:** `mcp/faelle.json` mit 26 Fällen (seit 23.11.2026; bis dahin 21; neu `m-batch`, `m-ruf-ohne-id`,
  `m-argumente-form`, `m-init-ohne-fassung`, `m-antwort-kein-json`) und dem Läufer `mcp/pruefe_mcp.py` (`MCP_BIN=<programm>`). Ein Stand-in hört am
  Socket und schreibt jede empfangene Zeile mit; so prüft der Läufer auch, was das Werkzeug weiterreicht.
- **Gemessen am 20.11.:** Gegen ein Programm, das sofort endet, sind 0 von 21 grün. Gegen eine Attrappe außerhalb des Repos
  sind 21 von 21 grün. Reicht die Attrappe fremde Werkzeuge und Argumente weiter, werden genau 2 Fälle rot
  (`m-unbekanntes-werkzeug`, `m-route-im-argument`).

### Mit der Copilot CLI (Vorführung, Entscheidung 20.11.2026)

Gemessen am 20.11. mit Copilot CLI 1.0.113 am Mac, ohne Anmeldung und ohne Netz (Proxy auf einen toten Port, leeres `HOME`),
interaktiv in einem eigenen Pseudoterminal, gegen einen Stand-in, der jede Zeile mitschreibt. Dazu gemessen am selben Tag mit
einer älteren Version aus dem Release, Prüfsumme gegen `SHA256SUMS` geprüft, auf dem Linux-Baurechner in
`bwrap --unshare-net`, also ganz ohne Netz, gestartet über `node index.js`.

- **Einrichtung:** als lokaler stdio-Server. Dauerhaft in `~/.copilot/mcp-config.json`:
  `{"mcpServers": {"such-mcp": {"type": "local", "command": "<pfad>/such-mcp", "args": [], "tools": ["*"]}}}`, oder
  `copilot mcp add such-mcp -- <pfad>/such-mcp`. Für eine Sitzung genügt `copilot --additional-mcp-config @<datei>`. So lief
  die Messung. Dazu liest die CLI `.mcp.json` oder `.github/mcp.json` im Arbeitsordner. Den Eintrag schreibt der Starter,
  nicht der Agent.
- **Handschlag:** Die CLI fragt zuerst `server/discover` mit der zustandslosen Fassung 2026-07-28 (in `_meta`). `such-mcp`
  antwortet darauf -32601. Die CLI fällt dann auf `initialize` mit 2025-11-25 zurück, schickt `notifications/initialized`
  und `tools/list` mit `params._meta.progressToken`. `such-mcp` nimmt `params` bei `tools/list` an und übergeht `_meta`.
  Fälle: `m-discover-copilot`, `m-liste-meta`, `m-liste-zweimal`. Die ältere Version handelt genauso aus (`server/discover`, dann
  `initialize` 2025-11-25 und `notifications/initialized`). Ohne Anmeldung kam `tools/list` bei ihr in 15 s nicht.
  Angemeldet (ein Lauf mit the test account in der VM, 20.11.2026) kam `tools/list` 0,43 s nach
  `notifications/initialized`, also vor dem Auftrag, und ein zweites Mal direkt nach der Eingabe des Auftrags (17,5 s nach
  dem Start des Servers). Beide trugen `_meta.progressToken`.
- **Umgebung:** Die CLI reicht ihre Umgebung an den Server weiter, dazu `COPILOT_*` und `GIT_CONFIG_*` (beide Versionen). `such-mcp` liest keine
  Umgebungsvariable; ein Token in der Umgebung bliebe dort ungenutzt. Im angemeldeten Lauf hatte der Server 24 Namen in der
  Umgebung und keine Token-Variable; die CLI las das Token aus ihrer `config.json`. Er erbte `HTTP_PROXY`, `HTTPS_PROXY`,
  `NO_PROXY`, `SSL_CERT_FILE`, `NODE_EXTRA_CA_CERTS` und `DBUS_SESSION_BUS_ADDRESS`. Hergeleitet: Ein Token in
  `COPILOT_GITHUB_TOKEN` würde er ebenso erben. Darum gibt `such-mcp` seine Umgebung nie aus und an niemanden weiter.
- **Nicht gemessen ohne Anmeldung:** `tools/call` und der Name, unter dem die CLI das Werkzeug führt und erlaubt
  (`--allow-tool`). Ohne Anmeldung endet `copilot -p` nach 8 s mit „No authentication information found“, noch vor den
  MCP-Servern. Der eine angemeldete Lauf (20.11.2026) scheiterte vor dem Modell, weil der Treiber die Abfragen des
  Terminals nicht beantwortete. Der zweite angemeldete Lauf (20.11.2026, Modell „Auto“, gewählt wurde
  gpt-6-luna) gelang: `tools/list` kam dreimal (0,001 s, 0,09 s und 1,75 s nach dem Start des Servers), dann genau ein
  `tools/call` mit `name: "brain_suche"` (ohne Vorsilbe des Servers), `arguments: {"query": "Wartungsfenster", "k": 10}`
  und `params._meta.progressToken`. `such-mcp` liest bei `tools/call` nur `name` und `arguments` und übergeht `_meta`
  (Fall `m-ruf-meta`). Die Antwort erschien im Terminal. `k` wählte das Modell selbst. Ob der Aufruf eine Bestätigung
  brauchte und wie `--allow-tool` wirkt, steht in den Belegen nicht.
- **Rückfrage vor dem Werkzeug (Projektentscheid, 20.11.2026):** Die Copilot CLI fragt vor jedem Werkzeug nach. Das bleibt
  die Vorgabe. Die Ausnahme regelt Vertrag 2, nicht dieser Vertrag: Die Admin-Einstellung
  `harness.copilot.werkzeuge_ohne_rueckfrage` (Fassung 2, `rust/spec/einstellungen/`) schaltet die Rückfrage für die
  Werkzeugart `such-mcp` ab, und nur für Läufe im Einschluss. Vertrag 1 hängt dafür `--allow-tool such-mcp` an den
  Startbefehl. Fehlt die Einstellung, bleibt die Rückfrage. `rust/spec/einstellungen/vorfuehrung.json` setzt `shell`,
  `write` und `such-mcp`. `such-mcp` kennt die Einstellung nicht und antwortet gleich, ob die CLI zurückgefragt hat oder
  nicht.
- **Später:** Antwortet ein Harness nur noch mit der Fassung 2026-07-28, braucht `such-mcp` `server/discover`. Das kommt dann
  mit einem Nachtrag.

## Vorführ-Vault

`vorfuehr-vault/` hält 21 erfundene Notizen für das Drehbuch (`rust/spec/vorfuehrung/DREHBUCH.md`). Sie enthalten keine echten
Daten und keine Namen. Alle tragen `datenklasse` mit frei gewählten Klassen als Kennungen (Stand 20.11.2026): `public`,
`internal`, `confidential`, `customer`, die niedrigste zuerst. Gesperrt sind nur die zwei Kundennotizen; `confidential` trägt
keine Notiz (siehe unten).

- **Frage F:** „Wann ist das nächste Wartungsfenster der Presslinie?“ Sie enthält keine Ziffer und kein Wort, das in einem
  festen Wert der Protokollzeile steckt (S4).
- **N-I:** `betrieb/MI-4F9-wartungsfenster-presslinie.md`, `datenklasse: internal`, Marker `MI-4F9`.
- **N-K:** `kunden/k-nord/MK-7Q2-liefertermin-presslinie.md`, `datenklasse: customer`, `kunde: k-nord`, Marker `MK-7Q2`. Der
  Kunde `k-nord` hat die Freigabe für die Route `lokal`.
- **N-S:** `kunden/k-sued/MS-3P8-abnahme-presslinie.md`, `datenklasse: customer`, `kunde: k-sued`, Marker `MS-3P8`. Der Kunde
  `k-sued` steht in der Vorführdatei ohne Freigabe (`erlaubt: []`, Entscheidung vom 20.11.2026). An ihm zeigt die
  Vorführung die Kundenachse: Auch die lokale Route sperrt.
- Jeder Marker steht genau einmal, in Pfad, Titel und Text der Notiz.
- Erwartet für F mit `k = 5` (Einstellungen: Route `copilot` Art cloud, Klasse `internal`; Route `lokal` Art lokal, Klasse
  `customer`):
  - `copilot`: N-I kommt an, `gesperrt: 2` (N-K und N-S, je `klasse-ueber-route`), zwei Protokollzeilen.
  - `lokal`: N-I und N-K kommen an, `gesperrt: 1` (N-S, `route-fuer-kunde-nicht-erlaubt`), eine Protokollzeile.
- `pruefe_vorfuehrung.py` prüft das mit der Referenz der Rangliste und dem Tor aus `bauen.py`, dazu die Marker und die Wörter
  von F. Gemessen am 20.11.: grün. Gegenprobe: Eine zusätzliche Notiz ohne Klasse, die auf F passt, macht die Prüfung auf beiden
  Routen rot (6 Fehler).
- **Grenze mit Einbettung (gemessen und hergeleitet):** Über die 4-Gramme passen 19 der 21 Notizen lexikalisch auf F. Eine
  weitere gesperrte Notiz, etwa eine vertrauliche, könnte mit Einbettung unter die ersten 5 rutschen und die Zahlen ändern.
  Deshalb sind nur die zwei Kundennotizen gesperrt, und beide passen gewollt auf F; ohne Vektoren stehen sie auf Platz 2 und 3.
  Ob sie auch mit dem Einbettungsmodell unter den ersten 5 bleiben, misst der Bau.
- `vorfuehr-vault-einrichten.sh <ziel>` legt den Vault außerhalb von `$HOME` an, Ordner 0700, Dateien 0600. Ein vorhandenes
  Ziel ergibt Exit 73, ein Ziel unter `$HOME` Exit 64; beide Male ändert das Skript nichts. Den Ort wählt der Starter.

## Ab M3: gespeicherte Klasse und Aufteilen

Abgestimmt am 20.11.2026 für den Teilvertrag `rust/spec/datentor/klassen/` (F4, F5). Im Durchstich gilt weiter der
rohe Wert aus dem Frontmatter, wie oben.

- **Wirksame Klasse aus dem Einbettungslauf (F4):** Ab M3 berechnet der Einbettungslauf die wirksame Klasse mit Erbe des
  Ortes, Aufteilen und Senken. Er speichert sie mit ihrer Herkunft getrennt von `datenklasse` und schreibt die
  Protokollzeile einer Senkung. Die Suche ruft nur `darf`, mit der gespeicherten Klasse.
- **Nur frische Klasse:** Die Suche nimmt die gespeicherte Klasse nur, wenn sie zum aktuellen Stand der Notiz gehört. Ist die
  Notiz seit dem Einbettungslauf geändert, gilt die Klasse als unklar. Die Notiz ist dann gesperrt, bis der Lauf sie neu
  bewertet (fail closed).
- **Gezählt wird in der Einheit der Treffer (F5):** Liefert die Suche bei aktivem Aufteilen Teile als Treffer, zählt
  `gesperrt` die gesperrten Teile unter den ersten k. Die Protokollzeile zählt die Teile je Gruppe aus Klasse, Kunde und Grund.
  Ohne Aufteilen sind Teil und Einheit dasselbe.

## Offen

Gegenüber Vertrag 1 und 2 ist nichts mehr offen (Stand 21.11.):

- Die Feldnamen `vorgabestufe`, `kunden[].erlaubt` und `kundendaten_an_cloud` stehen in Vertrag 2
  (`rust/spec/einstellungen/README.md`, Zeilen 36, 39 und 45). Die Fälle nutzen sie. Am 21.11. liefen sie am Mac gegen
  `rangkern sitzung` mit dem Leser `einstellungen` grün (macOS-Nachtrag, Abschnitt 7).
- Bereitschaftszeile, Frist 10 s und Ende über SIGTERM stehen gleichlautend in Vertrag 1 (`rust/spec/start/README.md`,
  Abschnitt „Such-Prozess je Sitzung“). Dort steht auch, dass der Such-Prozess mit leerer Umgebung startet.

Restpunkte (22.11.; Projektentscheid, 22.11.2026):

- **Dateimuster nur als Einheit.** Die Zeile „harte Grenze 0, Dateimuster“ der Falltabelle oben (Hinweis `MusterDatei`,
  danach normaler Start) prüft nur die Einheit `src/tests/abschirmung.rs` von `rangkern` und das Crate `kernabbild`. Der
  Programmtest `an_inherited_hard_limit_0_gives_exit_5_with_a_pipe_pattern_and_a_note_with_a_file_pattern` wählt den Zweig nach
  dem `core_pattern` des Wirts. Der Linux-Baurechner hat ein Pipe-Muster (`|/usr/lib/systemd/systemd-coredump …`, gemessen 22.11.), dort lief
  nur der Zweig Exit 5. Am Mac lief der Zweig ohne Muster (`MusterFehlt`) grün. Den Zweig Dateimuster prüft seit dem 23.11.2026
  der Programmtest `tests/kernabbild_muster.rs` von `rangkern` (nur Linux): `serve` und `sitzung` starten mit `unshare -Urm` in einem eigenen
  Nutzer- und Mount-Namensraum, eine Datei des Tests liegt per `mount --bind` über `/proc/sys/kernel/core_pattern`, die harte
  Grenze ist 0. Ein Dateimuster (`core`) muss die Hinweiszeile `MusterDatei` und einen normalen Start geben. Eine Datei mit Modus
  000, gelesen ohne Fähigkeiten (`setpriv`), muss mit Exit 5 und `kernabbild.muster_nicht_lesbar` enden. Ein Ordner kann nicht
  über einer Datei liegen; deshalb kommt der Fall „nicht lesbar“ über die Rechte einer Datei. Verweigert der Kernel das Binden
  im Nutzer-Namensraum, ist der Test rot, nie übersprungen.
- **Windows (Stand 25.11.2026).** `rangkern sitzung` läuft unter Windows über eine Named Pipe des Crates `peerkanal`
  (Nachtrag Windows); `rangkern` bleibt ohne `unsafe`. In einer Windows-11-VM sind 49 von 51 lauffähigen Fällen des Nachtrags
  grün; die Fälle mit dem Peer `agent` brauchen den Windows-Einschluss und laufen erst mit ihm. Den Schritt `kernabbild` gibt es
  unter Windows nicht. Die Leerlauffrist (Klarstellung 13) gilt für die Pipe noch nicht, weil `peerkanal` keine Lesefrist
  anbietet; vier Instanzen begrenzen die Pipe. Die Frage an den Plattformdienst ist gebaut, fragt aber noch nicht
  (fail closed: `such.dienst_stumm`).
- **`such-mcp` unter Windows** prüft Aufruf und Pipe-Namen nach dem Nachtrag, Abschnitt 6 (`--socket <pipe>` Pflicht,
  sonst Exit 64). Es fragt die Suche noch nicht und antwortet mit „Suche nicht erreichbar“ und `isError: true`, bis
  `peerkanal` die eigene Kennung des Prozesses nennt (fail closed).

<!-- vom Bauvorgang angefuegt -->
> Hinweis: Dieser Text nennt Werkzeuge, die in diesem Klon nicht liegen: `brain`.
> Sie gehoeren zum Setup der Maschine, aus der dieser Baum stammt; hier fehlen
> sie mit Absicht. Wer sie braucht, schreibt sich seine eigenen.
> *Named here, not shipped here.*
