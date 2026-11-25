# Vertrag `bruecke`

Stand 20.11.2026 (Teil B). Ausführbarer Vertrag des neuen Crates `bruecke` und des Programms `wb-bruecke`
(Durchstich: ein Zuschauer startet im Browser über den neuen Kern eine Agentensitzung). Grundlage sind das Wort, 20.11.2026 (b) und (c) und der
Vertrag `dienst` (`rust/spec/dienst/VERTRAG.md`, im Folgenden `DI`; DI12 ist der Nachtrag für mehrere Sitzungen).
Nachträge: „Projektentscheid, 20.11.2026“ (Benutzerdienst, `port_datei`), „Nachtrag 20.11.2026“ (`X-Deflate`, `code` und `text` der Antwort 422), „Nachtrag 20.11.2026“ (der Dienst prüft mit dem Crate `start`: die 422 trägt `grund` `start_pruefung`, `code` `start.<code>` und den festen Text der Stufe; geändert sind nur die Erwartungen von `br_starten_abgelehnt` und `br_starten_grenze`, `referenz/dienst_ref.py` ist die Kopie der neuen Referenz des Vertrags `dienst`).
Gebaut ist nur die Python-Referenz. Ein Rust-Bau gilt als vertragstreu, wenn er beide Fallsätze grün besteht (sichtbar und verdeckt) und die Pflichten
aus Abschnitt 11 erfüllt. „(H)“ heißt hergeleitet; „(V)“ heißt im Vertrag festgelegt.

## 1. Schnitt

Die Brücke ist ein **eigener Prozess**. Der Dienst `wb-dienst` bleibt ohne TCP (`DI` Abschnitt 4 Punkt 6, Mutant `rechte_tcp_hoerer`). Die Brücke spricht
HTTP/1.1 mit dem Browser (nur `std`, kein Framework, keine neue Crate für HTTP) und reicht Bytes an den Unix-Socket des Dienstes weiter. Sie ist
ein **Programm des Durchstichs** wie der Dienst. **Im Probelauf auf dem Ubuntu-Laptop (Projektentscheid, 20.11.2026)** läuft sie als **Dienst des Benutzers** (`systemd --user`), nie als root-Dienst: ohne Root, ohne Fähigkeiten, ohne Ports außer 127.0.0.1, ohne `unsafe`.
Ihre Laufzeitdateien liegen im 0700-Ordner unter `$XDG_RUNTIME_DIR` (Abschnitt 2, „Namen der Laufzeitdateien“). Eine Peer-Prüfung kennt die Brücke nicht; die Grenze ist der Ordner 0700 und das Token.

Der Zuschauer an seinem Rechner erreicht die Brücke über einen ssh-Tunnel (`ssh -L`) auf die Schleifenadresse. Die Brücke bindet **nur 127.0.0.1**. Es gibt
keinen Schalter für eine andere Adresse. Die Seite (HTML und JS) ist nicht Teil dieses Vertrags; die Brücke liefert sie nur aus (Abschnitt 5).

Die Brücke kennt **kein Ereignis** der Ansicht. Sie prüft Anfragen (Form, `Host`, `Origin`, Token), übersetzt zwei Routen in Rahmen des Dienstes und
reicht alles andere unverändert durch. Jede Regel über Sitzungen, Eingaben und Größen liegt im Dienst.

Die Brücke nutzt Fäden und `std`. Sie liest JSON nur, um zu prüfen, dass ein Koerper JSON ist (`serde_json`, schon im Arbeitsbereich; keine neue Crate).
Kein `unsafe`.

## 2. Aufruf und Startdatei (BR1)

`wb-bruecke <startdatei.json>` läuft, bis SIGTERM oder SIGINT eintrifft. Dann: alle Verbindungen schließen, die Token-Datei entfernen, Exitcode 0.

```json
{"dienst": {"socket": "/abs/ordner/dienst.sock"},
 "port": 0,
 "token_datei": "/abs/ordner/bruecke.token",
 "port_datei": "/abs/ordner/bruecke.port",
 "deflate": true,
 "pruefmodus": false,
 "lese_frist_ms": 10000,
 "verbindungen_max": 64}
```

- Pflicht: `dienst`, `port`, `token_datei`, `deflate`. Optional: `port_datei`, `pruefmodus`, `lese_frist_ms`, `verbindungen_max`. **Jeder andere Schlüssel** ist ungültig, auch `adresse`, `host` und jeder Name, der nach einem Schalter für die
  Bindung aussieht (Projektentscheid, 20.11.2026: „es gibt keinen Schalter dafür“).
- `dienst`: ein Objekt mit genau `socket`, einem absoluten Pfad ohne NUL, 1 bis 100 Byte (Grenze von `sun_path` am Mac). Die Datei muss beim Start nicht bestehen
  (die Brücke verbindet sich erst bei einer Anfrage).
- `port`: ganze Zahl 0 bis 65535 (ein Wahrheitswert ist keine Zahl). 0 heißt: ein freier Port, vom System gewählt.
- `token_datei`: ein absoluter Pfad ohne NUL, nicht `/`, ohne `/` am Ende, höchstens 4096 Byte.
- `port_datei` (Nachtrag 20.11.2026, optional): ein Pfad nach denselben Regeln, **nicht** derselbe wie `token_datei` (sonst Exitcode 2). Mit ihr schreibt die Brücke den gebundenen Port in diese Datei (Inhalt: die Dezimalzahl und `\n`; Modus **genau 0600**; exklusiv angelegt,
  **nach** der Token-Datei und **vor** der Zeile `PORT`). Die Rechte des Ordners und die Regel „darf nicht bestehen“ gelten für sie wie für die Token-Datei (Exitcode 3). Sie wird mit der Token-Datei beim Ende entfernt. Scheitert das Anlegen der Port-Datei, entfernt die Brücke die
  Token-Datei, die sie selbst angelegt hat, und endet mit Exitcode 3.
- `deflate` (Wahrheitswert, **Pflicht**; Nachtrag 20.11.2026): der Wert `deflate` des Dienstes, an dem die Brücke hängt. Er bestimmt den Kopf `X-Deflate` des Stroms (Abschnitt 5). Die Brücke prüft nicht, ob er zum Dienst passt; das ist Sache der Installation.
- `pruefmodus` (Wahrheitswert, Vorgabe false), `lese_frist_ms` (ganze Zahl 100 bis 10000, Vorgabe 10000) und `verbindungen_max` (ganze Zahl 1 bis 64, Vorgabe 64):
  die zwei letzten **nur mit `pruefmodus` true**, sonst Exitcode 2. Sie senken Zeitgrenze und Zahl der Verbindungen für Fälle, die die Grenzen schnell erreichen (wie
  `warteschlange_max` in `DI`); im Betrieb gelten 10 s und 64.
- Die Datei hat höchstens 65 536 Byte, ist UTF-8, ein JSON-Objekt, ohne `NaN` und `Infinity`. Eine Datei, die fehlt oder sich nicht lesen lässt: Exitcode 2.
- Exitcodes beim Start (Reihenfolge der Prüfung: 2, dann 3, dann 1):
  - **2** Aufruf oder Startdatei ungültig,
  - **3** Rechte an der Token-Datei (siehe unten),
  - **1** das Binden des Ports scheitert (zum Beispiel: belegt).
  Bei jedem Exitcode ungleich 0 beim Start steht **genau eine** nicht leere Zeile auf stderr, die mit `wb-bruecke: ` beginnt; stdout bleibt leer; es bleibt keine
  Token-Datei zurück, **die die Brücke angelegt hat** (eine schon bestehende bleibt, wie sie ist).
- **Rechte der Token-Datei (Sicherheitsstelle, das Projekt liest sie selbst).** Der Ordner von `token_datei` muss mit `lstat` ein Ordner sein (kein Link), dem effektiven Benutzer
  gehören und **genau den Modus 0700** haben; sonst Exitcode 3. Die Brücke legt den Ordner nie an. Die Token-Datei darf **nicht bestehen** (Datei, Link, auch ein
  hängender Link, Ordner): Exitcode 3. Die Brücke ersetzt und löscht nie etwas, das schon da ist. Sie legt die Datei **exklusiv** an (`O_EXCL`) mit dem Modus
  **genau 0600**, ohne Fenster zwischen Anlegen und Modus (Umask 0 und Modus beim Anlegen).
- **Reihenfolge beim Start (V):** Startdatei lesen, Rechte prüfen, den Port binden und hören, die Token-Datei schreiben, **dann** auf stdout genau eine Zeile
  `PORT <n>` (der gebundene Port, Dezimalzahl, Zeilenende `\n`) schreiben. stdout trägt sonst nichts: nie das Token, keine zweite Zeile. Wer auf die Zeile
  wartet (oder auf die Port-Datei), findet die Token-Datei schon vor.
- **Bindung:** genau `127.0.0.1`, ein Hörer, TCP. Kein `::1`, kein `0.0.0.0`, kein zweiter Hörer. Der Prüfclient liest die Hörer mit `lsof -nP -a -p <pid> -iTCP -sTCP:LISTEN`
  und erwartet genau `127.0.0.1:<port>`.
- Nach SIGTERM oder SIGINT: Hörer zu, alle Verbindungen zu (auch die zum Dienst), die Token-Datei und die Port-Datei weg, Exitcode 0. Bei einem Absturz bleibt die Datei stehen; der nächste Start lehnt
  sie ab (Exitcode 3), der Betreiber räumt auf.
- Keine Eingabe löst eine Panik aus.

**Namen der Laufzeitdateien und Installationskonfiguration (Projektentscheid, 20.11.2026; das Projekt gibt sie weiter).** Im Probelauf-Paket gilt, wobei `<run>` der Wert von `$XDG_RUNTIME_DIR` ist (je Benutzer, 0700, von systemd angelegt):
- Ordner der Laufzeitdateien: `<run>/werkbank/` (Modus 0700; legt der Benutzerdienst oder `RuntimeDirectory=werkbank` an, nicht die Brücke),
- Socket des Dienstes: `<run>/werkbank/dienst.sock` (`DI`: `socket.ordner` = der Ordner, `socket.name` = `dienst.sock`); Unterordner der Sitzungen `<run>/werkbank/s-<n>/` (`DI12`),
- **Port der Brücke: `<run>/werkbank/bruecke.port`** (0600, Dezimalzahl und Zeilenende), **Token: `<run>/werkbank/bruecke.token`** (0600, 64 Hex und Zeilenende). Der Läufer des Drehbuchs liest beide und öffnet `http://127.0.0.1:<port>/#t=<token>`.
Die **Startdateien** von Dienst und Brücke sind **Installationskonfiguration**: Das Paket legt sie an (zum Beispiel unter `/etc/werkbank/` oder im Konfigurationsordner des Benutzers), nicht die Ansicht und nicht ein Browser. Das Paket trägt die Pfade ein (die Werte sind seine Sache): `token_datei`, `port_datei` und `dienst.socket` in der
Startdatei der Brücke; `lauf.programm` (der Pfad des Programms „Lauf starten“) und `lauf.einstellungen` (der Pfad der Einstellungsdatei) in der Startdatei des Dienstes. Weder `wb-dienst` noch `wb-bruecke` liest diese Pfade aus einer anderen Quelle (nicht aus der Umgebung, nicht aus einer Anfrage, nicht aus `argv`).

## 3. Token (BR2) — Sicherheitsstelle, das Projekt liest sie selbst

1. **Erzeugung:** 32 Byte aus dem Zufall des Systems (`getrandom`/`/dev/urandom`), beim Start, als **64 Kleinbuchstaben-Hex** und ein Zeilenende `\n` in die Token-Datei
   (genau 65 Byte). Das Token wird nie auf stdout, stderr oder in eine Protokollzeile geschrieben, auch nicht gekürzt oder als Base64. Ein Token gilt für die Laufzeit
   des Prozesses; es gibt keinen Widerruf und keine Erneuerung (ein neuer Start gibt ein neues Token).
2. **Form der Anfrage mit Token:** Kopf `Authorization: Bearer <token>`, ein Leerzeichen, genau das Token (64 Zeichen), nichts dahinter. `Bearer` mit großem B. Das Token
   steht nirgends sonst (nicht in der Abfrage des Ziels, nicht in einem Cookie). Die Seite trägt es im URL-Fragment (`#…`, das der Browser nie sendet) und setzt den Kopf selbst.
3. **Vergleich in konstanter Zeit.** Die Brücke vergleicht das vorgelegte Token mit dem eigenen mit einer Funktion, die **jede Stelle bis zur Länge des längeren
   Wertes liest**, nie früher aufhört und den Längenunterschied in das Ergebnis einrechnet:
   ```
   token_gleich(a, b): d = len(a) XOR len(b); für i in 0 .. max(len(a), len(b)): d = d OR (a[i] oder 0) XOR (b[i] oder 0); gib d == 0
   ```
   **Wahl (Frage 1, Abschnitt 10):** Ein Zeitmessfall flattert; darum ist das keine Außenprüfung. Der Vertrag legt die **Eigenschaft** fest und prüft sie mit einem
   Einheitstest: Die Funktion zählt in der Referenz die gelesenen Stellen (`gelesen`); `bruecke_ref.py --selbsttest` prüft an neun Paaren (gleich, erster Byte anders, letzter Byte
   anders, kürzer, länger, leer, nur ein Wert leer, Nullen), dass die Wahrheit stimmt **und** dass genau `max(len(a), len(b))` Stellen gelesen wurden. Der Bau trägt denselben Einheitstest
   (Abschnitt 11). Der Mutant `token_vergleich_frueh` ist nur für diesen Selbsttest sichtbar; `gegenprobe.sh` ruft ihn auf. Außerdem fallen alle falschen Tokens
   (gleiche Länge, andere Länge, nur der Anfang gleich) in den Außenfällen auf 401.
4. **Wann:** Das Token wird geprüft, **bevor die Brücke irgendetwas an den Dienst reicht**. Eine Anfrage ohne gültiges Token öffnet keine Verbindung zum Dienst (der Prüfclient
   zählt die Verbindungen des Dienstes, Abschnitt 8). Die Antwort ist **401 ohne Inhalt** (`Content-Length: 0`, kein `WWW-Authenticate`).
5. Ohne Token sind **nur** `GET /`, `GET /ansicht.js` und `GET /ansicht_bg.wasm` erreichbar (Abschnitt 5; Nachtrag Ansicht 21.11.2026, vorher `GET /seite.js`). Alles andere verlangt das Token, auch jede andere Methode und jedes unbekannte Ziel (zuerst 401, mit
   Token dann 405 oder 404).

## 4. Form der Anfrage und Köpfe (BR3)

Die Prüfungen laufen in **fester Reihenfolge**; die erste, die greift, antwortet, und nichts danach wird geprüft:

1. **Form der Anfrage** (400, Größen: 414 und 431, Zeit: 408, siehe Abschnitt 6),
2. **`Host`** (403),
3. **`Origin`** (403),
4. **Token** (401; entfällt für `GET /`, `GET /ansicht.js` und `GET /ansicht_bg.wasm`),
5. **Methode und Route** (405, 404),
6. nur für POST: Inhaltstyp (415), Länge (411, 413), Lesen (408), JSON (400),
7. erst dann das Weiterreichen an den Dienst.

**Form (400, ohne Inhalt).** Die Anfrage ist **genau** so gebaut, sonst 400:
- Kopfende `\r\n\r\n`; jede Zeile mit `\r\n` beendet (ein einzelnes `\n` oder `\r` in einer Zeile, ein NUL, ein anderes Steuerzeichen als HTAB im Wert: 400);
- Anfragezeile `<Methode> <Ziel> HTTP/1.1` mit genau einem Leerzeichen dazwischen. Die Methode besteht aus 1 bis 16 **Großbuchstaben** A bis Z. Das Ziel beginnt mit `/`
  (kein absolutes `http://…`, kein `*`), hat nur ASCII-Zeichen von `!` bis `~` (kein Leerzeichen, kein Byte ab 0x80). Jede andere Version, auch `HTTP/1.0`: 400;
- Kopfzeilen `Name: Wert`: der Name ein Token (Buchstaben, Ziffern und `!#$%&'*+.^_`|~-`), **kein Leerzeichen vor dem Doppelpunkt**, nicht leer; der Wert ohne führende und
  nachfolgende Leerzeichen und HTAB. **Eine Zeile, die mit Leerzeichen oder HTAB beginnt (Fortsetzung, Faltung), ist 400.** Eine Zeile ohne Doppelpunkt: 400;
- die Köpfe `Host`, `Authorization`, `Origin`, `Content-Length`, `Content-Type` und `X-Verbindung` dürfen **höchstens einmal** vorkommen: 400;
- **`Transfer-Encoding` in jeder Form: 400** (die Brücke liest nur Körper mit `Content-Length`). Ein `Content-Length`, das nicht aus 1 bis 10 Dezimalziffern besteht: 400;
  bei einer Methode außer POST muss `Content-Length` fehlen oder `0` sein: 400;
- **`Host` fehlt: 400** (HTTP/1.1 verlangt ihn).

**`Host` (403 ohne Inhalt).** `Host` muss **genau** `127.0.0.1:<port>` oder `localhost:<port>` sein (`<port>` der gebundene Port, Kleinbuchstaben, Gleichheit Byte für Byte).
Alles andere ist 403: ein fremder Name, ein anderer Port, kein Port, `LOCALHOST`, `[::1]:<port>`, ein Wert mit Vorsatz oder Anhang. Grund: Das hält DNS-Rebinding fern (ein fremder
Name, der auf 127.0.0.1 zeigt, trägt seinen eigenen Namen im Kopf).

**`Origin` (403 ohne Inhalt).** Ist `Origin` da, muss er **genau** `http://127.0.0.1:<port>` oder `http://localhost:<port>` sein (auch `null`, `https://…` und ein anderer Port sind 403;
die Prüfung gilt für jede Methode, auch für die öffentliche Seite). **Ein POST ohne `Origin` ist 403 (V).** Grund: Jeder Browser sendet bei einem POST `Origin`; ein POST ohne ihn kommt nicht
von der Seite, sondern von einem anderen Werkzeug, das die Seite nicht braucht; die Ablehnung kostet nichts und verengt die Fläche. Ein GET ohne `Origin` geht (ein Browser sendet ihn bei einem
GET desselben Ursprungs nicht).

## 5. Routen, Seite, CORS (BR4)

**Keine CORS-Freigabe:** Keine Antwort trägt jemals einen Kopf, dessen Name mit `Access-Control-` beginnt. `OPTIONS` ist wie jede andere Methode (siehe unten). Der Prüfclient liest bei jeder Antwort jedes
Falls, ob ein solcher Kopf da ist (`cors`), und erwartet überall `false`.

**Alle Antworten** tragen `Connection: close`, `Content-Length` (außer dem Strom), `Cache-Control: no-store` und `X-Content-Type-Options: nosniff`, und kein `Set-Cookie`. Die Brücke antwortet
**eine** Anfrage je Verbindung und schließt dann (kein Keep-Alive, kein Pipelining). Eine Ablehnung (400, 401, 403, 404, 405, 408, 411, 413, 414, 415, 431, 502, 503, 504) hat **keinen Inhalt**.

| Methode, Ziel | Token | Wirkung |
|---|---|---|
| `GET /` | nein | 200, `Content-Type: text/html; charset=utf-8`, die Seite; Kopf `Content-Security-Policy` (unten) |
| `GET /ansicht.js` | nein | 200, `Content-Type: text/javascript; charset=utf-8`, das Skript der Ansicht (Klebecode von wasm-bindgen); derselbe Kopf `Content-Security-Policy` |
| `GET /ansicht_bg.wasm` | nein | 200, `Content-Type: application/wasm`, das Modul der Ansicht (Dioxus-web); derselbe Kopf `Content-Security-Policy` |
| `GET /strom` | ja | Ereignisstrom (unten) |
| `POST /starten` | ja | Start einer Sitzung (unten) |
| `POST /eingabe` | ja | ein Ereignis der Ansicht an genau eine Verbindung (unten) |
| jede andere Methode oder falsche Methode auf einem bekannten Ziel (auch `OPTIONS`, `HEAD`, `PUT`) | ja | 405 |
| jedes andere Ziel (auch mit Abfrage; die Brücke vergleicht das Ziel **ganz**, `/?a=1` ist unbekannt) | ja | 404 |

**Die Seite** (`GET /`, `GET /ansicht.js` und `GET /ansicht_bg.wasm`; Projektentscheid, 20.11.2026 (c), Nachtrag Ansicht 21.11.2026): Der Vertrag legt fest, **was die Seite braucht**, nicht wie sie aussieht.
- Sie ist **eine** HTML-Datei, **eine** Skriptdatei (`/ansicht.js`) und **ein** wasm-Modul (`/ansicht_bg.wasm`), ohne fremde Skripte und ohne Netzabrufe nach außen. Die HTML-Datei lädt
  `ansicht.js` relativ, `ansicht.js` lädt `ansicht_bg.wasm` relativ. `GET /seite.js` gibt es nicht mehr (mit Token 404, ohne Token 401). Im Inhalt der drei Dateien steht **keine**
  Zeichenfolge `://` (keine Adresse); ein Namensraum von XML (`http://www.w3.org/2000/svg` und die anderen in `pruefclient.py`, `NAMENSRAEUME`) ist ein Name und zählt nicht. Der Prüfclient
  prüft das (`koerper_ohne_url`).
- `Content-Security-Policy`: eine Policy, die **nur** die Quellen `'self'`, `'none'`, `'unsafe-inline'`, `'wasm-unsafe-eval'` und `data:` nennt (außer in `base-uri` und `form-action`, die `'none'` tragen),
  `default-src` und `frame-ancestors 'none'` enthält; `script-src` nennt `'wasm-unsafe-eval'` (der Prüfclient: `csp_wasm`), sonst kann der Browser das Modul nicht übersetzen. Die Referenz sendet: `default-src 'none'; script-src 'self' 'wasm-unsafe-eval'; style-src 'self' 'unsafe-inline'; connect-src 'self'; img-src 'self' data:; base-uri 'none';
  form-action 'none'; frame-ancestors 'none'`. Der Prüfclient wertet die Policy aus (`csp`); er vergleicht nicht den Wortlaut.
- Die Seite liest das Token aus `location.hash` und sendet es nur im Kopf `Authorization`. Sie ruft **nur** `/strom`, `/starten` und `/eingabe` desselben Ursprungs. Mehr braucht sie nicht:
  - `POST /starten` mit der Start-Struktur, Antwort 200 `{"sitzung": "<id>"}` oder 422 `{"grund": "…", "code": "…", "text": "…"}`;
  - `GET /strom` (mit `fetch` und dem Kopf `Authorization`; ein Binärstrom), die Kennung in `X-Verbindung`;
  - `POST /eingabe` mit `X-Verbindung` und einem Ereignis der Ansicht (`DI` Abschnitt 5).

**`POST /starten`.** Voraussetzungen wie jeder POST: `Content-Type` ist `application/json` (Groß und Klein egal, ein Zusatz `; charset=utf-8` geht; sonst **415**), `Content-Length` ist da (sonst **411**), höchstens
**16 384 Byte** (sonst **413**, bevor die Brücke den Körper liest), der Körper wird innerhalb der Zeitgrenze gelesen (sonst 408) und ist **ein JSON-Wert** (Syntax, Tiefe höchstens 127 Ebenen, ab 128 Ebenen 400; ohne `NaN`
und `Infinity`; sonst **400**). Dann öffnet die Brücke eine **neue, kurze Verbindung** zum Dienst, sendet einen Rahmen `Ereignis` mit `{"art": "Starten", "start": <der Wert>}` (nach dem Parsen
neu geschrieben; der Wert wird **nie als Text in das Ereignis gesetzt**) und liest Rahmen, bis eine der Antworten da ist (Frist 10 s), und schließt die Verbindung:
- `Gestartet` (Nachricht `{"art": "Gestartet", "sitzung": id}`, `DI12`) → **200**, `Content-Type: application/json`, `{"sitzung": "<id>"}`;
- `Ablehnung` (Rahmen 0x07 `{"grund": …, "code": …, "text": …}`, `DI12`) → **422**, `application/json`, `{"code": "<code>", "grund": "<grund>", "text": "<text>"}` mit den Werten des Dienstes **unverändert** (Nachtrag 20.11.2026: die Antwort trägt `code` **und** `text`, bei einem Code `start.*` der feste Text aus the maintainers Vertrag 1, nicht nur `grund`;
  fehlt `code` in der Ablehnung, steht dort der `grund`, fehlt `text`, steht `""`);
- `Fehler` (0x08), Ende der Verbindung → **502** ohne Inhalt; Frist verstrichen → **504** ohne Inhalt; keine Verbindung zum Dienst → **502** ohne Inhalt.
Die Brücke prüft den Inhalt der Start-Struktur **nicht**; das tut der Dienst (`DI12`, später der Crate `start`).

**`GET /strom`.** Ein Ereignisstrom je Browser-Verbindung. Die Brücke öffnet eine Verbindung zum Dienst und antwortet **200** mit `Content-Type: application/octet-stream`, `Cache-Control: no-store`,
`Connection: close`, **ohne** `Content-Length`, mit dem Kopf **`X-Verbindung: <kennung>`** und (Nachtrag 20.11.2026) mit dem Kopf **`X-Deflate: 1`** oder **`X-Deflate: 0`** (der Wert von `deflate` der Startdatei: sind die Rasterrahmen des Stroms gepackt?); dann gibt sie **jedes Byte, das der Dienst sendet, unverändert** weiter (die Rahmen
`[n: u32 big endian][art: u8][nutzlast]` des Dienstes, `DI` Abschnitt 5, **mit ihrer Länge, nicht neu gerahmt, nicht JSON, nicht Base64**). Der Strom endet, wenn der Dienst oder der Browser schließt;
die Brücke schließt dann auch die andere Seite (der Dienst meldet `Trennen`, `DI4`). Die **Kennung** ist **32 Kleinbuchstaben-Hex** aus dem Zufall des Systems (128 Bit, nicht erratbar, nicht
fortlaufend); sie gilt nur für diese Verbindung und endet mit ihr. Kann die Brücke keine Verbindung zum Dienst öffnen: **502** ohne Inhalt.

**`POST /eingabe`.** Der Kopf `X-Verbindung` muss **genau 32 Kleinbuchstaben-Hex** sein (sonst **400**, auch bei fehlendem Kopf); eine wohlgeformte Kennung, die keine offene Verbindung bezeichnet: **404**.
Sonst wie jeder POST (415, 411, 413, 408), Körper höchstens **1 048 575 Byte** (1 MiB minus 1: so überschreitet der Rahmen mit seiner Art die Grenze des Dienstes von `n = 1 048 576` nie; 1 048 576 Byte
sind 413), der Körper ist ein **JSON-Objekt** (sonst 400). Die Brücke sendet den Körper **unverändert** als Nutzlast eines Rahmens `Ereignis` (Art 0x01) an die Verbindung dieser Kennung (nur an diese) und antwortet
**204** ohne Inhalt. Was der Dienst damit tut (Antwort im Strom, `Ablehnung`, `Fehler` mit Ende der Verbindung), steht im Strom. Eine Eingabe löst keine Panik aus, auch wenn der Dienst die Verbindung gerade beendet
(dann 502).

## 6. Grenzen (BR5)

- **Kopf:** Die Anfragezeile und die Köpfe zusammen höchstens **8192 Byte**; ist das Kopfende bis dahin nicht da: **431**. Höchstens **64 Kopfzeilen** (ohne die Anfragezeile): mehr ist **431**.
  Das Ziel hat höchstens **2048 Byte**: mehr ist **414** (die Prüfung steht vor der Prüfung der Version).
- **Körper:** `/eingabe` höchstens 1 048 575 Byte, `/starten` höchstens 16 384 Byte (413, ohne den Körper zu lesen). Andere POST-Ziele lesen nie einen Körper (sie enden vorher mit 401, 403, 404 oder 405).
- **Verbindungen:** höchstens **64** gleichzeitig (jede Anfrage und jeder Strom zählt eine; im Prüfmodus `verbindungen_max`). Die nächste bekommt **503** ohne Inhalt und wird geschlossen; die Brücke startet für sie
  keinen Faden.
- **Zeit beim Lesen:** Kopf und Körper müssen innerhalb von **10 s** (im Prüfmodus `lese_frist_ms`) gelesen sein, gerechnet ab dem Annehmen der Verbindung beziehungsweise ab dem Ende des Kopfes; sonst **408**
  ohne Inhalt, dann Schließen. Der Strom hat nach dem Kopf keine Lesefrist.
- **Eine kaputte Anfrage beendet nur ihre Verbindung:** weder die Brücke noch eine andere Verbindung (auch ein gehaltener Strom) sieht davon etwas. **Keine Eingabe löst eine Panik aus** (Binärdaten, NUL, ungültiges UTF-8,
  riesige Zeilen, abgeschnittene Anfragen).

## 7. Protokoll (BR6)

Das Protokoll der Brücke ist **stderr**. **Eine Zeile je abgelehnter Anfrage** (jede Antwort ab 400, außer dass der Dienst dahinter ablehnt: 422 ist eine Antwort des Dienstes, keine Ablehnung der Brücke):

```
wb-bruecke: abgelehnt <grund>
```

`<grund>` ist **eines** dieser Wörter: `anfrage_form` (400 durch die Form), `kopf_zu_gross`, `kopf_zu_viele`, `ziel_zu_lang`, `zeit`, `host`, `origin`, `token`, `methode`, `route`, `inhalt_typ`, `koerper_laenge` (411),
`koerper_zu_gross`, `json`, `kennung` (400 und 404 bei `/eingabe`), `verbindungen`, `dienst` (502, 504). Die Zeile enthält **nichts sonst**: **kein Token** (weder das richtige noch das vorgelegte), **kein Ziel**,
**keinen Kopfwert**, **keinen Körper**, **keinen Inhalt der Sitzung**. Der Prüfclient prüft das mit dem Fall-Feld `geheim` (Wörter, die in Ziel, Köpfen und Körpern der Anfragen stehen, auch in Base64):
`ende.protokoll_ohne_inhalt` muss wahr sein; und bei **jedem** Fall prüft er, dass das echte Token weder auf stdout noch auf stderr steht (`ende.token_nicht_in_ausgabe`). Erfolgreiche Anfragen schreiben nichts.

## 8. Prüfclient, Fall und Ergebnis

Der **Prüfclient** (`pruefclient.py`, Python 3 mit Standardbibliothek; Dekoder ist `referenz/ra_ref.py`, die Referenz des Vertrags `rasterauszug`, nie ein Mutant) liest einen Fall und
1. startet gegenüber **die Referenz des Dienstes** (`referenz/dienst_ref.py`, eine unveränderte Kopie des Vertrags `dienst` mit DI12; Umgebung `BR_DIENST`) mit `lauf`, dem Wrapper `@laufkind` als Programm (ein festes
   Kind, das nach `BEREIT` und dem Startbyte `MARKE <text>` schreibt) und der Vorführ-Einstellungsdatei (Kopie in `einstellungen/vorfuehrung.json`, `BR_EINST`), im Prüfmodus (Vorgabe) oder im Betrieb (`"dienst": {"pruefmodus": false}`),
2. schreibt die Startdatei der Brücke, startet `BR_BIN`, wartet auf die Zeile `PORT <n>`,
3. spricht HTTP/1.1 zu `127.0.0.1:<n>` über rohe Sockets (auch kaputte Anfragen) und liest Ströme,
4. beendet mit SIGTERM und schreibt **eine** JSON-Zeile.

Umgebung: `BR_BIN` (Pflicht), `BR_DIENST`, `BR_REF_DIR` (Ordner mit `ra_ref.py`), `BR_EINST`, `BR_TMP`. Er gibt `REF_GEGENPROBE` nur an die Brücke weiter. Exit 0: Ergebnis geschrieben; 2: Fall ungültig; 3: Umgebung falsch (kein `lsof`, der Dienst startet nicht).

```json
{"name": "…", "regeln": ["BR3"], "datensatz": "A", "beschreibung": "…",
 "start": { …Startdatei der Bruecke; "@dienst_socket", "@token_datei", "@port_datei", "@port_belegt" werden ersetzt… }, "start_roh": "…",
 "ordner": {"art": "verzeichnis|fehlt|link|datei", "modus": "0700", "token_vorher": "datei|link", "port_vorher": "datei"},
 "dienst": null | {"pruefmodus": true, "deflate": false, "plan": {"*": {"modus": "marke", "text": "X", "sek": 30}}, "groesse": [100, 12]},
 "geheim": ["Wort", …], "schritte": [ … ]}
```

**Schritte:**

| `art` | Felder | Wirkung und Ergebnis |
|---|---|---|
| `Http` | `anfrage` | sendet **eine** Anfrage (Verbindung zu, Antwort bis zum Ende gelesen). `anfrage`: entweder `roh` (der ganze Text, `@port`, `@token`, `@token_falsch` (letztes Zeichen geändert), `@token_anders_erst` (erstes Zeichen geändert) werden ersetzt) oder `methode`, `ziel`, `host` (`"gut"` oder Text oder null), `origin` (`"gut"`, `"gut_localhost"`, Text, null), `token` (`"gut"` oder der Wert des Kopfes `Authorization`, null), `inhalt_typ`, `koerper` (Text oder JSON), `koerper_laenge` (füllt mit Leerzeichen bis zu dieser Länge), `content_laenge` (überschreibt, null lässt den Kopf weg), `verbindung` (Name eines Stroms: setzt `X-Verbindung`), `kennung`, `koepfe` (weitere `[name, wert]`). Ergebnis: `{"status", "cors", "cookie", "close", "nosniff", "no_store", "laenge_stimmt", "typ", "csp" (ob die Policy die Regeln von Abschnitt 5 erfüllt), "koerper_laenge" (ab 400 und bei 204), "json" (bei application/json), "koerper_nicht_leer", "koerper_ohne_url"}` oder `{"status": null, "zu": true}` |
| `Langsam` | `anfrage`, `bytes` oder `nur_kopf`, `frist` | sendet nur den Anfang der Anfrage (oder nur den Kopf) und nichts mehr; wartet, bis die Brücke schließt; Ergebnis wie `Http` und `innerhalb` (die Brücke hat vor der Frist geschlossen) |
| `Strom` | `name`, `anfrage` (Vorgabe `GET /strom` mit Token) | öffnet einen Strom; Ergebnis `{"status", "deflate" (der Wert des Kopfes `X-Deflate`, `"0"` oder `"1"`), "kennung" (32 Hex), "kennung_verschieden" (mindestens 6 verschiedene Zeichen)}`; ab hier liest der Prüfclient die Rahmen (sein Dekoder folgt dem Kopf `X-Deflate`) und setzt sie wie der Prüfclient von `DI` zusammen |
| `Warte` | `strom`, `bis`, `frist` (Vorgabe 20 s) | wartet auf `{"text": s, "nr": n}` (eine Zeile des Abbilds der Sitzungsnummer n enthält s), `{"nachricht": {Feld: Wert}}`, `{"liste": {Feld: Wert}}` (die letzte `Sitzungsliste`), `{"ablehnung": grund}`, `{"fehler": code}`, `{"getrennt": true}`, `{"nachrichten_anzahl": n}`; Ergebnis `{"ok": bool}` |
| `Zustand` | `strom`, `arten`, `ruhe` | liest bis zu `ruhe` Sekunden Ruhe; Ergebnis `{"abbilder": {nr: {groesse, cursor, zeilen}}, "nachrichten": [… ohne t oder nur die Arten], "ablehnungen", "fehler", "abbild_fehler", "getrennt"}` und leert die Listen |
| `Schliesse` | `strom` | schließt den Strom von der Seite des Prüfclients |
| `Dienst` | `warte_betrachter` | **zählt die Verbindungen des Dienstes**: eine frische Verbindung fragt `Zustand` (Prüfmodus). `verbindungen` = die Nummer ihres Namens (`b<n>`, `DI4`: nie wiederverwendet) minus die Zahl der bisherigen Fragen = **Zahl der Verbindungen, die die Brücke geöffnet hat**; `betrachter` = die gerade verbundenen außer der Frage; `sitzungen` = die Kennungen. Mit `warte_betrachter` wartet er (bis 10 s), bis diese Zahl gilt |
| `Halte`, `Frei`, `HalteLesen` | `anzahl`, `name`, `anfrage` | hält leere Verbindungen offen, gibt sie frei, sendet eine Anfrage auf einer gehaltenen |
| `Pause` | `sekunden` | Atempause |

**Ergebnis** insgesamt: `{"start": {…}, "ergebnisse": […], "ende": {…}}`.
- `start`: gescheitert: `{"ok": false, "exitcode", "stderr_zeilen", "stderr_beginn", "stdout_leer", "token_da"}`; sonst `{"ok": true, "port_zeile": true, "stdout_zeilen": 1, "token_datei": {"modus": "0600", "form": bool (64 Hex und `\n`), "verschieden": bool (mindestens 8 verschiedene Zeichen)}, "port_datei" (nur mit `port_datei`): {"modus": "0600", "inhalt_stimmt": bool}, "ordner_modus", "hoerer": ["127.0.0.1"], "stderr_leer"}`.
- `ende`: `{"exitcode", "token_datei_entfernt", "port_datei_entfernt" (nur mit `port_datei`), "protokoll": [<grund>…], "stderr_fremd" (Zeilen, die kein Protokoll sind), "token_nicht_in_ausgabe", "protokoll_ohne_inhalt" (nur mit `geheim`)}` nach SIGTERM (10 s Frist, sonst `"kill"`).
- **Nicht verglichen** wird, was von der Zeit oder vom Betrieb abhängt: keine Portnummer, kein Token, keine Kennung der Verbindung (nur ihre Form), keine Rahmenbyte.
- Jeder Fall wartet vor einem Vergleich auf ein Ende, das nicht von der Zeit abhängt (eine Marke im Bild, eine Ablehnung im Strom). Die Zeitfälle (`br_grenze_zeit`) lesen nur, ob die Brücke **vor** einer Frist von 4 s mit 408 geantwortet hat.
- Kein Fall hat eine Antwort, die sich mit dem Tag der Ausführung ändern kann.

## 9. Gegenproben (Mutanten)

Jeder Mutant schaltet sich in der Referenz mit `REF_GEGENPROBE=<name>` zu. `gegenprobe.sh` prüft, dass diese Liste und `bruecke_ref.py --gegenproben` gleich sind und dass jeder Mutant auf **mindestens einen** Fall in einem der
beiden Sätze rot macht (oder, nur für `token_vergleich_frueh`, im Selbsttest der Referenz). Je Auflage (Auftrag 20.11.2026 (b)) steht mindestens ein Fall und ein Mutant:

| Auflage | Fälle | Mutanten |
|---|---|---|
| Token mit 256 bit, zufällig, Datei 0600 | `br_start_gut`, `br_token_form`, `br_token_datei_*`, `br_token_ordner_*` | `token_kurz`, `token_fest`, `token_nicht_hex`, `token_modus_offen`, `token_auf_stdout`, `start_token_*` |
| ohne gültiges Token 401 ohne Inhalt, **bevor** etwas an den Dienst geht | `br_401_*`, `br_reihenfolge` (Dienst zählt 0 Verbindungen) | `token_nicht_geprueft`, `token_nur_laenge`, `token_nur_praefix`, `token_nach_dienst`, `token_401_mit_inhalt`, `token_in_abfrage_ok`, `token_ohne_bearer_ok`, `token_gross_klein_ok` |
| Vergleich in konstanter Zeit | `bruecke_ref.py --selbsttest`, falsche Tokens in `br_401_*` | `token_vergleich_frueh` |
| `Host` und `Origin` gegen die eigene Adresse | `br_host`, `br_host_form`, `br_origin`, `br_origin_get`, `br_reihenfolge` | `host_*`, `origin_*`, `post_ohne_origin_ok`, `get_ohne_origin_abgelehnt` |
| keine CORS-Freigabe | jede Antwort jedes Falls (`cors`), `br_vorabfrage`, `br_methoden` | `cors_frei`, `cors_vorabfrage_ok` |
| Eingabe und Start nur per POST | `br_methoden` | `methode_put_ok`, `methode_get_starten_ok`, `methode_get_eingabe_ok`, `methode_head_ok` |
| nur 127.0.0.1, kein Schalter | `br_start_gut` (Hörer), `br_start_adresse` | `bind_alle_adressen`, `bind_auch_ipv6`, `start_adresse_ok` |

| Mutant | Regel | Änderung |
|---|---|---|
| `start_adresse_ok` | BR1 | die Startdatei darf einen Schlüssel `adresse` tragen (ein Schalter für die Bindung) |
| `start_port_ohne_grenze` | BR1 | ein Port über 65535 wird angenommen |
| `start_pruefschluessel_ohne_pruefmodus` | BR1 | `lese_frist_ms` und `verbindungen_max` werden auch ohne `pruefmodus` angenommen |
| `start_token_ordner_modus_nicht_geprueft` | BR1 | der Modus des Ordners der Token-Datei wird nicht geprüft |
| `start_token_datei_ersetzen` | BR1 | eine schon bestehende Token-Datei wird gelöscht und ersetzt |
| `start_ordner_link_folgen` | BR1 | ein symbolischer Link als Ordner der Token-Datei wird verfolgt |
| `bind_alle_adressen` | BR1 | die Brücke bindet an 0.0.0.0 |
| `bind_auch_ipv6` | BR1 | die Brücke hört zusätzlich auf `::1` |
| `bind_fehler_token_bleibt` | BR1 | die Token-Datei wird vor dem Binden geschrieben; scheitert das Binden, bleibt sie stehen |
| `token_kurz` | BR1, BR2 | das Token hat 16 Byte statt 32 |
| `token_fest` | BR1, BR2 | das Token ist immer gleich (lauter Nullen) |
| `token_modus_offen` | BR1, BR2 | die Token-Datei hat den Modus 0644 statt 0600 |
| `token_nicht_hex` | BR1, BR2 | das Token wird als Base64 statt als Hex geschrieben |
| `token_auf_stdout` | BR1, BR2 | das Token steht auch auf stdout |
| `token_im_protokoll` | BR6, BR2 | die Protokollzeile bei einem falschen Token nennt das richtige Token |
| `ende_token_bleibt` | BR1 | nach SIGTERM bleibt die Token-Datei stehen |
| `port_zeile_fehlt` | BR1 | die Zeile `PORT <n>` fehlt auf stdout |
| `port_zeile_mit_adresse` | BR1 | die Zeile heißt `PORT 127.0.0.1:<n>` |
| `token_nicht_geprueft` | BR2 | jedes Token wird angenommen |
| `token_nur_laenge` | BR2 | es zählt nur die Länge des Tokens |
| `token_nur_praefix` | BR2 | es zählen nur die ersten 8 Zeichen des Tokens |
| `token_nach_dienst` | BR2 | die Brücke verbindet sich mit dem Dienst, bevor sie das Token prüft |
| `token_vergleich_frueh` | BR2 | der Vergleich hört beim ersten Unterschied auf und liest nicht alle Byte (Fänger: der Selbsttest, Abschnitt 6) |
| `token_ohne_bearer_ok` | BR2 | das Token ohne das Wort `Bearer` wird angenommen |
| `token_gross_klein_ok` | BR2 | `bearer` in Kleinbuchstaben wird angenommen |
| `token_in_abfrage_ok` | BR2 | das Token in der Abfrage des Ziels (`?token=`) wird angenommen |
| `token_401_mit_inhalt` | BR2 | die Antwort 401 hat einen Inhalt |
| `host_nicht_geprueft` | BR3 | der Kopf `Host` wird nicht geprüft |
| `host_ohne_port_ok` | BR3 | `Host` ohne Port wird angenommen |
| `host_ende_ok` | BR3 | `Host` wird nur auf das Ende der erlaubten Werte geprüft (`evil127.0.0.1:<port>` geht) |
| `host_nach_token` | BR3 | das Token wird vor `Host` und `Origin` geprüft |
| `host_fehlt_ok` | BR3 | eine Anfrage ohne `Host` wird angenommen |
| `host_ip6_ok` | BR3 | `Host: [::1]:<port>` wird angenommen |
| `origin_nicht_geprueft` | BR3 | der Kopf `Origin` wird nicht geprüft |
| `origin_null_ok` | BR3 | `Origin: null` wird angenommen |
| `origin_praefix_ok` | BR3 | `Origin` wird nur auf den Anfang `http://127.0.0.1` geprüft |
| `origin_https_ok` | BR3 | `https://127.0.0.1:<port>` wird als Origin angenommen |
| `origin_nach_token` | BR3 | das Token wird vor `Origin` geprüft (aber nach `Host`) |
| `post_ohne_origin_ok` | BR3 | ein POST ohne `Origin` wird angenommen |
| `get_ohne_origin_abgelehnt` | BR3 | auch ein GET ohne `Origin` wird abgelehnt |
| `cors_frei` | BR4 | jede Antwort trägt `Access-Control-Allow-Origin: *` |
| `cors_vorabfrage_ok` | BR4 | `OPTIONS` wird mit 204 und CORS-Köpfen beantwortet |
| `methode_put_ok` | BR4 | `PUT /eingabe` wird wie ein POST behandelt |
| `methode_get_starten_ok` | BR4 | `GET /starten` wird wie ein POST behandelt |
| `methode_get_eingabe_ok` | BR4 | `GET /eingabe` wird wie ein POST behandelt |
| `methode_head_ok` | BR4 | `HEAD /`, `HEAD /ansicht.js` und `HEAD /ansicht_bg.wasm` werden beantwortet |
| `route_unbekannt_200` | BR4 | ein unbekanntes Ziel mit gültigem Token wird mit 200 beantwortet |
| `seite_token_noetig` | BR4 | die Seite verlangt ein Token |
| `seite_ohne_csp` | BR4 | die Seite trägt keinen Kopf `Content-Security-Policy` |
| `csp_fremde_quelle` | BR4 | die Policy nennt eine fremde Quelle |
| `seite_fremdskript` | BR4 | die Seite lädt ein Skript von einer fremden Adresse |
| `seite_ohne_nosniff` | BR4 | die Antworten tragen keinen Kopf `X-Content-Type-Options: nosniff` |
| `seite_js_bleibt` | BR4 | `GET /seite.js` wird weiter ohne Token beantwortet (Nachtrag Ansicht) |
| `wasm_falscher_typ` | BR4 | das Modul kommt als `application/octet-stream` statt `application/wasm` |
| `wasm_token_noetig` | BR4 | `GET /ansicht_bg.wasm` verlangt ein Token |
| `skript_fehlt` | BR4 | `GET /ansicht.js` ist keine Route |
| `csp_ohne_wasm` | BR4 | `script-src` nennt `'wasm-unsafe-eval'` nicht |
| `post_ohne_inhaltstyp_ok` | BR4 | ein POST mit falschem oder fehlendem Inhaltstyp wird angenommen |
| `post_laenge_fehlt_ok` | BR4 | ein POST ohne `Content-Length` wird angenommen (leerer Koerper) |
| `starten_ohne_jsonpruefung` | BR4 | der Koerper von `/starten` wird ungeprüft in das Ereignis eingesetzt |
| `strom_kennung_fehlt` | BR4 | die Antwort des Stroms trägt keinen Kopf `X-Verbindung` |
| `kennung_zaehler` | BR4 | die Kennung der Verbindung ist ein Zähler (erratbar) |
| `strom_rahmen_veraendert` | BR4 | die Rahmen des Dienstes erreichen den Browser verändert |
| `eingabe_an_erste_verbindung` | BR4 | eine Eingabe geht an die erste Verbindung statt an die der Kennung |
| `eingabe_kennung_nicht_geprueft` | BR4 | eine unbekannte Kennung wird auf irgendeine Verbindung abgebildet |
| `eingabe_kein_objekt_ok` | BR4 | ein Koerper von `/eingabe`, der kein JSON-Objekt ist, wird weitergegeben |
| `eingabe_ohne_jsonpruefung` | BR4 | der Koerper von `/eingabe` wird nicht als JSON geprüft |
| `tiefe_nicht_geprueft` | BR4 | die Tiefe des JSON-Körpers eines POST wird nicht geprüft (Nachtrag 21.11.2026) |
| `tiefe_grenze_plus_eins` | BR4 | ein JSON-Körper mit 128 Ebenen wird angenommen (Grenze um eins zu hoch) |
| `tiefe_grenze_minus_eins` | BR4 | ein JSON-Körper mit 127 Ebenen wird abgelehnt (Grenze um eins zu niedrig) |
| `strom_ende_ohne_trennen` | BR4 | endet der Strom, bleibt die Verbindung zum Dienst offen |
| `grenze_kopf_ohne` | BR5 | der Kopf ist nicht auf 8192 Byte begrenzt |
| `grenze_kopfzahl_ohne` | BR5 | die Zahl der Kopfzeilen ist nicht begrenzt |
| `grenze_ziel_ohne` | BR5 | die Länge des Ziels ist nicht begrenzt |
| `grenze_koerper_eingabe_ohne` | BR5 | der Koerper von `/eingabe` ist nicht begrenzt |
| `grenze_koerper_eingabe_ein_byte_frueh` | BR5 | schon 1 048 575 Byte sind für `/eingabe` zu groß |
| `grenze_koerper_starten_ohne` | BR5 | der Koerper von `/starten` ist nicht begrenzt |
| `grenze_zeit_ohne` | BR5 | es gibt keine Zeitgrenze beim Lesen |
| `grenze_verbindungen_ohne` | BR5 | die Zahl der Verbindungen ist nicht begrenzt |
| `form_kaputt_beendet_alle` | BR5 | eine kaputte Anfrage beendet die ganze Brücke |
| `form_http10_ok` | BR3 | `HTTP/1.0` wird angenommen |
| `form_transfer_encoding_ok` | BR3 | `Transfer-Encoding` wird angenommen |
| `form_doppelter_laenge_ok` | BR3 | zwei Köpfe `Content-Length` werden angenommen |
| `form_absolutes_ziel_ok` | BR3 | ein Ziel in absoluter Form (`http://…`) wird angenommen |
| `form_klein_methode_ok` | BR3 | eine kleingeschriebene Methode wird angenommen |
| `form_faltung_ok` | BR3 | eine Fortsetzungszeile (Faltung) wird angenommen |
| `form_nach_host` | BR3 | `Host` wird vor der Form der Anfrage geprüft |
| `protokoll_fehlt` | BR6 | es gibt keine Protokollzeile |
| `protokoll_mit_pfad` | BR6 | die Protokollzeile nennt das Ziel der Anfrage |
| `protokoll_mit_koerper` | BR6 | die Protokollzeile nennt den Anfang des Koerpers |
| `protokoll_mit_kopfwert` | BR6 | die Protokollzeile nennt den Wert von `Host`, `Origin` oder `Authorization` |
| `start_deflate_nicht_pflicht` | BR1 | `deflate` darf in der Startdatei fehlen (Nachtrag 20.11.2026) |
| `x_deflate_fehlt` | BR4 | der Strom trägt keinen Kopf `X-Deflate` |
| `x_deflate_falsch` | BR4 | `X-Deflate` meldet das Gegenteil von `deflate` |
| `starten_ohne_text` | BR4 | die Antwort 422 trägt kein Feld `text` |
| `starten_ohne_code` | BR4 | die Antwort 422 trägt kein Feld `code` |
| `starten_ohne_grund` | BR4 | die Antwort 422 trägt kein Feld `grund` |
| `port_datei_modus_offen` | BR1 | die Port-Datei hat den Modus 0644 statt 0600 (Nachtrag 20.11.2026) |
| `port_datei_bleibt` | BR1 | nach SIGTERM bleibt die Port-Datei stehen |
| `port_datei_ersetzen` | BR1 | eine schon bestehende Port-Datei wird gelöscht und ersetzt |
| `port_datei_ohne_zeilenende` | BR1 | die Port-Datei hat kein Zeilenende |

## 10. Fragen und offene Punkte

Fragen (Entscheidungen, die der Vertrag klein getroffen hat und die das Projekt bestätigt oder ändert):
1. **Token-Vergleich in konstanter Zeit.** Gewählt: eine **Eigenschaft mit Einheitstest** statt eines Zeitmessfalls (Abschnitt 3 Punkt 3). Grund: Ein Zeitmessfall flattert auf einer belasteten Maschine; die Eigenschaft „liest alle Stellen“ ist
   deterministisch zählbar. Folge: Der Mutant `token_vergleich_frueh` fällt nur im Selbsttest der Referenz, nicht in den Außenfällen; der Bau trägt den Einheitstest (Abschnitt 11). Gewünscht (Auftrag, Punkt 2): ja oder ein anderer Weg?
2. **POST ohne `Origin` ist 403.** Grund steht in Abschnitt 4. Folge: Werkzeuge wie `curl` brauchen `-H 'Origin: http://127.0.0.1:<port>'`; die Seite nicht.
3. **`Content-Type: application/json` ist Pflicht** bei beiden POST-Routen (415 sonst). Grund: Eine fremde Seite kann ohne Vorabfrage nur „einfache“ Typen senden; mit dieser Pflicht gäbe es für sie immer eine Vorabfrage, die die Brücke ablehnt (kein CORS). Das ist eine zweite Linie neben Token und `Origin`.
4. **`POST /starten` antwortet synchron** (200 mit Kennung oder 422 mit Grund) über eine **eigene kurze Verbindung** zum Dienst. Der Strom bekommt `Gestartet` nie; er bekommt die `Sitzungsliste` (DI12). So braucht die Seite für den Start keine Zuordnung zwischen Strom und Anfrage.
5. **Grenzen:** 8192 Byte Kopf, 64 Kopfzeilen, 2048 Byte Ziel, 16 384 Byte Körper für `/starten`, 1 048 575 Byte für `/eingabe`, 64 Verbindungen, 10 s Lesefrist. Alle klein gewählt und im Prüfmodus senkbar; ändert das Projekt eine Zahl, ändert sich ein Wert in Abschnitt 6 und in `faelle_bauen.py`.
6. **Nur IPv4.** Der Tunnel des Zuschauers zeigt auf `127.0.0.1:<port>` (`ssh -L <lokal>:127.0.0.1:<port> …`). Ein Hörer auf `::1` ist verboten (Mutant `bind_auch_ipv6`).

Offene Punkte (OPEN):
- **Die Seite ist nicht Teil.** Der Vertrag legt Routen, Formen und die Regeln für Quellen fest (Abschnitt 5); die Referenz liefert Platzhalter. Der Rasterdekoder im Browser ist ein eigener Schritt.
- **Kein TLS, kein Keep-Alive, kein HTTP/2:** nicht nötig hinter dem ssh-Tunnel.
- **Die Ablehnung der Prüfstelle in der Antwort 422** (Nachtrag, gebaut): Der Dienst ruft den Crate `start`; die 422 trägt `grund` `start_pruefung`, `code` `start.<code>` (etwa `start.schluessel_fehlt` für den Körper `{}`, `start.json_ungueltig` für `[1, 2]` und `"text"`) und den festen Text der Stufe. Die Brücke reicht das unverändert durch und hat sich nicht geändert; es ändern sich nur die Erwartungen von `br_starten_abgelehnt` und `br_starten_grenze` (`{}` mit 16384 Byte Körper: `start.schluessel_fehlt`).
- **Mehrere Zuschauer.** Der Vertrag trägt mehrere Ströme mit demselben Token; „je Zuschauer ein Token“ ist nicht Teil des Durchstichs.
- **Mac-Lauf der Fallsätze** folgt gesammelt (Prüfregel des Durchstichs); gemessen ist der Lauf auf dem Linux-Baurechner (siehe Messbericht).
- **Zeitfälle** (`br_grenze_zeit`, `br_grenze_verbindungen`) hängen an Fristen von 0,4 bis 4 s; sie lesen nur, ob die Brücke **vor** der Frist geantwortet hat.
- **`Referrer-Policy`** ist nicht festgelegt (die Seite ruft nichts nach außen, es gibt keine fremden Ziele).

## 11. Pflichten des Baus außerhalb der Außentests

- README des Crates: `wb-bruecke` ist ein Programm des Durchstichs (Abschnitt 1), bindet nur 127.0.0.1 und trägt kein TLS.
- **Benutzerdienst (Projektentscheid, 20.11.2026):** `wb-bruecke` und `wb-dienst` laufen als Dienste des Benutzers (`systemd --user`), **nie als root**; **kein `unsafe`, kein Root, keine Fähigkeiten (`capabilities`), keine Ports außer 127.0.0.1** bei der Brücke. Die Einheiten des Pakets setzen `RuntimeDirectory=werkbank` mit `RuntimeDirectoryMode=0700`
  (der Ordner unter `$XDG_RUNTIME_DIR`) und für beide Dienste `NoNewPrivileges=yes`; die Brücke braucht keine Netzfamilie außer `AF_INET` und `AF_UNIX`. Die Startdateien beider Dienste legt das Paket an (Abschnitt 2, „Namen der Laufzeitdateien“).
- **Einheitstest (kein Außenfall):** Scheitert das Anlegen der Port-Datei nach dem der Token-Datei (zum Beispiel, weil ein Test den Ordner dazwischen sperrt), bleibt keine Token-Datei zurück und der Exitcode ist 3. Der Außenlauf erreicht den Fall nicht, weil die Rechteprüfung vor dem Start jeden Fehler der Port-Datei schon abfängt.
- Die Namen der Laufzeitdateien (`bruecke.port`, `bruecke.token`) stehen in Abschnitt 2; der Bau schreibt sie nur über die Startdatei (`port_datei`, `token_datei`) und kennt keine eingebauten Pfade.
- Kein `unsafe`, keine Panik auf Eingaben, `cargo deny` nach `rust/deny.toml`, **keine neue Abhängigkeit** (HTTP und Zufall nur mit `std`: der Zufall aus `/dev/urandom`, bei einem Lesefehler Exitcode 1; `serde_json` ist schon im Arbeitsbereich).
- Die Adresse ist im Code **fest** `Ipv4Addr::LOCALHOST`; sie ist kein Parameter einer Funktion, die die Startdatei erreicht. Ein Test (nicht Außenfall) belegt das: `cargo test` ruft die Bindung ohne Adresse auf.
- **Einheitstest des Vergleichs:** `token_gleich(a, b)` mit einem Zähler der gelesenen Stellen (zum Beispiel über eine Variante `token_gleich_gezaehlt`) prüft an den neun Paaren des Selbsttests von `referenz/bruecke_ref.py`, dass die Wahrheit stimmt und genau `max(len(a), len(b))` Stellen gelesen werden. Die Funktion nutzt keine Standardvergleiche von Anteilen (`==` auf Bytefolgen).
- **Einheitstest der Form:** ein fester Satz kaputter Anfragen (die Fälle `br_form`) und ein deterministischer Strom von Zufallsbytes (fester Samen) laufen gegen die Prüffunktion der Form, ohne Panik, mit 400 oder einer Größenantwort.
- Die Reihenfolge der Prüfungen steht an **einer** Stelle im Code (Abschnitt 4); ein Test ruft sie mit Anfragen, die zwei Regeln verletzen, und prüft, dass die frühere antwortet.
- Die Seite und das Skript sind in das Programm eingebunden (`include_bytes!`), nicht aus einem Ordner gelesen: Die Brücke liest zur Laufzeit keine Datei außer der Startdatei.
- Ein Test der Grenzen des Körpers an den Grenzwerten (16 384, 16 385, 1 048 575, 1 048 576) und der Zeitgrenze (mit kurzer Frist).
- **Messung** der Antwortzeit und der Zahl der Fäden bei 64 Verbindungen als Läufer auf einer ruhigen Messmaschine, nicht in `gates.sh test`.

## 12. Dateien

- `referenz/bruecke_ref.py` (ausführbar, `#!/usr/bin/python3 -I`; das ist `BR_BIN` der Referenz): Die Referenz führt wirklich aus (TCP, Unix-Socket, Fäden, Zufall, Dateirechte). `--selbsttest` prüft den Vergleich.
- `referenz/dienst_ref.py`, `referenz/ra_ref.py`, `referenz/zk_ref.py`: **unveränderte Kopien** der Referenzen der Verträge `dienst` (mit DI12), `rasterauszug` und `zustandskern` (die Gegenseite der Fälle); `sha256` stehen im Messbericht. Bei einer Änderung dieser Verträge sind die Kopien zu erneuern und `faelle_bauen.py` neu zu fahren.
- `einstellungen/vorfuehrung.json`: Kopie der Vorführ-Einstellungsdatei aus the maintainers Vertrag 2, vom Dienst der Gegenseite gelesen.
- `pruefclient.py`, `test_aussen.py` (Läufer mit `BR_BIN` und `BR_FAELLE`), `faelle_bauen.py` (alle Erwartungen aus der Referenz; jeder Fall läuft zweimal, die Ergebnisse müssen gleich sein), `gegenprobe.sh`.
- `aussentests/{faelle,erwartet}/` (sichtbar, Datensatz A) und `verdeckt/{faelle,erwartet}/` (Datensatz B: andere Texte, Größen, Zahlen, falsche Tokens und Hosts, andere Kanarienwörter).

## 13. Projektentscheide nach der Abnahme (20.11.2026)

Abgenommen nach eigener Prüfung: 60 sichtbare und 58 verdeckte Fälle, 0 gleiche
Hashes zwischen beiden Sätzen, 93 Mutanten in einem Lauf mit 0 grün geblieben.

1. **Token-Vergleich:** Die Eigenschaft „liest alle Stellen“ mit Einheitstest und Selbsttest gilt. Ein Zeitmessfall flattert und prüft nichts
   Zusätzliches. Die Zusage ist damit nicht schwächer als ein Zeitfall.
2. **POST ohne `Origin` ist 403**, und `Content-Type: application/json` ist bei beiden POST-Routen Pflicht. Beides bleibt so.
3. **`POST /starten` antwortet synchron** über eine eigene kurze Verbindung. Die Liste kommt im Strom, `Gestartet` nie.
4. **`deflate` ist Pflicht** in der Startdatei. Ob der Wert zum Dienst passt, ist Sache der Installation.
5. **Grenzen** bleiben wie in Abschnitt 6.
6. **Ablehnung beim Start:** `code` trägt den Code der Prüfstelle (mit dem Crate `start`: `start.<code>`), `grund` bleibt das kurze Wort. Die Brücke
   reicht beides unverändert durch.

## 14. Projektentscheide nach dem Bau (20.11.2026)

Der Bau `wb-bruecke` (Zweig, Stand vom 20.11.2026) besteht beide Fallsätze auf dem Linux-Baurechner ohne Wiederholung: 60 von 60 sichtbar und 58 von 58
verdeckt. Zu seinen offenen Punkten:

1. **Abhängigkeiten:** `signal-hook` und `rustix` (Merkmal `process`) stehen direkt im Crate. Ohne sie gibt es ohne `unsafe` weder ein sauberes Ende
   nach SIGTERM noch die Prüfung des Besitzers. Beide sind schon im Graphen; es kommt kein neues Paket hinzu. Das gilt so.
2. **Tiefe des JSON:** Die Grenze ist die von `serde_json`: 127 Ebenen gehen, ab 128 antwortet die Brücke 400. Die Referenz lässt tiefere Werte zu; das
   war eine Lücke der Referenz, kein Fehler des Baus. **Nachtrag 21.11.2026 (Zeitannahmen):** Die Referenz lehnt ab 128 Ebenen mit 400 ab (`TIEFE_MAX` 127, Tiefe zählt verschachtelte Felder und Objekte, ein Wert ohne Behälter ist 0) und die Fälle `br_starten_tiefe_127` und `br_starten_tiefe_128` stehen in beiden Sätzen; drei Mutanten (`tiefe_nicht_geprueft`, `tiefe_grenze_plus_eins`, `tiefe_grenze_minus_eins`).
3. **Zahlen** wie `1e999` lehnt die Brücke als ungültiges JSON ab. Große ganze Zahlen schreibt sie als Gleitkomma neu. Den Inhalt der Start-Struktur
   prüft der Dienst; das gilt so.
4. **`Content-Type`:** Nach dem Semikolon gelten nur Leerzeichen und Tabulator als Leerraum. Das ist enger als die Referenz und gilt so.

## 15. Nachtrag (21.11.2026)

Der Fall `br_eingabe_dienstfehler` war mit der Referenz unstabil (unter Last etwa jeder fünfte Lauf: `$.ende.protokoll` hatte `["dienst"]`, und der erste `POST /eingabe` antwortete 502 statt 204). Ursache (gemessen, Ausnahme im Lauf aufgezeichnet): Die Referenz setzte nach dem Senden des Rahmens das Zeitlimit des Dienst-Sockets zurück. Hatte der Dienst da schon mit einem Fehlerrahmen geantwortet und die Verbindung beendet, hatte der Stromfaden den Socket schon geschlossen; das Zurücksetzen warf `OSError(9, 'Bad file descriptor')`, und die Referenz machte aus dem gesendeten Rahmen eine 502 mit Protokollzeile `dienst`. Behoben in der Referenz: Das Zurücksetzen steht außerhalb der Fehlerbehandlung des Sendens und ein Fehler dabei wird ignoriert. Der Vertrag ändert sich nicht: Ist der Rahmen gesendet, antwortet die Brücke 204, auch wenn der Dienst die Verbindung gleich danach beendet. Der Bau `wb-bruecke` hat den Wettlauf nicht (`try_clone` teilt den Deskriptor und setzt das Limit vor dem Schreiben, danach nichts mehr). Kein Fall und keine Erwartung ändert sich.

## 16. Kernabbild-Grenze 1/1 (21.11.2026)

Auch `wb-bruecke` schirmt sich als erster Schritt von `main` ab (Crate `kernabbild`, Exitcode 5, Zeile `wb-bruecke: ...`), und ihr Prüfclient startet sie im Nutzer-Namensraum mit Positivkontrolle; die Regeln, Werte und die Falltabelle stehen im Abschnitt 16 „Kernabbild-Grenze 1/1“ von `rust/spec/dienst/VERTRAG.md`. Kein Fall und keine Erwartung der Brücke ändert sich.

## Nachtrag Ansicht (21.11.2026): die Ansicht in drei Dateien

Auftrag (A157 Stufe 2a): Die Brücke liefert statt der Seite des Durchstichs die Ansicht aus dem Crate `ansicht` (Dioxus-web, gebaut für `wasm32-unknown-unknown`,
`wasm-bindgen --target web --out-name ansicht`). Die Ausgabe ist ein flacher Ordner mit genau `index.html`, `ansicht.js` und `ansicht_bg.wasm`, eingebettet in das Programm.

1. **Routen:** `GET /` (die neue `index.html`), `GET /ansicht.js` (`text/javascript; charset=utf-8`) und `GET /ansicht_bg.wasm` (`application/wasm`), alle drei ohne Token und mit
   demselben Kopf `Content-Security-Policy`. Weitere Routen gibt es nicht; `GET /seite.js` ist ein unbekanntes Ziel (mit Token 404, ohne Token 401).
2. **CSP:** `script-src` bekommt nur `'wasm-unsafe-eval'` dazu, sonst ändert sich nichts. Ohne diese Quelle übersetzt der Browser kein wasm-Modul.
3. **Keine Adresse:** In den drei Dateien steht kein `://`, außer in einem Namensraum von XML (der SVG-Namensraum steht als Zeichenkette im Modul, weil die Ansicht Symbole als SVG zeichnet).
4. **Fälle:** `br_seite` holt alle drei Dateien; `br_routen` fragt `/seite.js`, `/ansicht.js/`, `/ansicht.wasm`, `/ansicht_bg.wasm/` und `/ANSICHT.JS` als unbekannte Ziele und
   `/ansicht.js?a=1` und `/ansicht_bg.wasm?a=1` ohne Token; `br_methoden` prüft die falschen Methoden auf beiden neuen Zielen; `br_origin_get` holt beide mit `Origin`.
   Beide Sätze sind mit `faelle_bauen.py` neu gebaut.
5. **Mutanten:** `seite_js_bleibt`, `wasm_falscher_typ`, `wasm_token_noetig`, `skript_fehlt`, `csp_ohne_wasm` (Abschnitt 9).

## 17. Nachtrag 25.11.2026: Plattform Windows (Durchstich B)

Stand des Baus: abgenommen 25.11.2026. `wb-bruecke` läuft unter Windows 11 (Build 26300, x64, Ziel `x86_64-pc-windows-gnu`). Unter Linux und macOS ändert sich nichts. Der Dienst unter Windows steht in Abschnitt 21 des Vertrags `dienst`. **Punkt 1 weicht von „ohne `unsafe`“ (Abschnitt 1) ab und braucht eine Entscheidung, bevor er gilt.**

1. **`unsafe` nur in `src/sys/windows.rs`.** Das Modul erklärt die Win32-Aufrufe selbst (`ws2_32`, `advapi32`, `kernel32`, `bcrypt`); es kommt kein Paket hinzu. Die Lints des Crates sind die des Workspace mit `unsafe_code = "deny"` statt `forbid` (wie im Crate `peerkanal`). Unter Unix enthält das Crate weiter kein `unsafe`.
2. **Socket des Dienstes:** `AF_UNIX` wie unter Unix (Begründung und Messung: Vertrag `dienst`, Abschnitt 21 Punkt 2). Ein Lesen mit Frist, das abläuft (`WSAETIMEDOUT`), gilt wie unter Unix als `WouldBlock`.
3. **Ordner 0700 und Dateien 0600 mit Zugriffslisten.** Der Ordner der Token-Datei (und der Port-Datei) muss ein Ordner sein, kein Reparse-Punkt, dem eigenen Benutzer gehören und eine geschützte Liste haben, die nur den eigenen Benutzer und `SYSTEM` erlaubt; sonst Exitcode 3 mit denselben Gründen wie unter Unix. Die Token-Datei und die Port-Datei entstehen neu (`CREATE_NEW`, nie eine bestehende, kein Reparse-Punkt wird gefolgt), bekommen den eigenen Benutzer als Besitzer und eine geschützte Liste mit genau dem eigenen Benutzer; die Brücke liest sie zurück. **Abweichung:** Zwischen dem Anlegen und dem Setzen der Liste trägt die Datei die geerbte Liste des geprüften Ordners (nur der eigene Benutzer und `SYSTEM`); unter Unix gibt es dieses Fenster nicht (Modus beim Anlegen). Am Ende entfernt die Brücke eine Datei nur, wenn der Pfad noch dieselbe Datei nennt (Volume und Dateinummer; die Entsprechung von Gerät und Inode).
4. **Token (Abschnitt 3 Punkt 1).** Der Zufall kommt unter Windows aus `BCryptGenRandom` mit `BCRYPT_USE_SYSTEM_PREFERRED_RNG`, dem Generator des Systems; `/dev/urandom` gibt es dort nicht.
5. **Signale.** SIGTERM und SIGINT sind die Konsolenereignisse `CTRL_BREAK_EVENT` und `CTRL_C_EVENT` (über `signal-hook`, sichere Schnittstelle); der Aufrufer startet die Brücke in einer eigenen Prozessgruppe (`CREATE_NEW_PROCESS_GROUP`). `TerminateProcess` entspricht SIGKILL.
6. **Kernabbild (Abschnitt 16).** Unter Windows gibt es keine Abschirmung (wie beim Dienst, Abschnitt 21 Punkt 7 dort); das braucht eine Entscheidung.
7. **Pfade.** Die Startdatei behält Pfade mit `/` am Anfang; Windows liest sie auf dem Laufwerk des Arbeitsordners. Einen Dateinamen mit `\` oder `:` legt die Brücke unter Windows nicht an (Exitcode 3).
8. **Schließen nach einer Antwort.** Unter Windows endet ein Socket, der geschlossen wird, während der Client noch sendet (der Rest eines Körpers über der Grenze, die Anfrage einer Verbindung über `verbindungen_max`), mit einem Reset; der Client verliert dann die schon gesendete Antwort (gemessen in der VM: kein Status, nur das Ende der Verbindung, in `br_grenze_verbindungen` und `br_starten_grenze`). Die Brücke schließt darum unter Windows erst ihre Schreibhälfte, liest höchstens 64 KiB und 200 ms lang, was noch kommt, verwirft es und schließt dann.
9. **Prüfmittel.** Die Außenfälle laufen in der Windows-VM mit einer Windows-Kopie des Prüfclients (AF_UNIX über `ctypes`, Modi als Listen, `CTRL_BREAK_EVENT`, `netstat -ano` statt `lsof` für die hörenden Adressen, mit Positivkontrolle). Die Gegenseite ist unter Windows `wb-dienst.exe` (Prüfbau) statt der Python-Referenz, die eine Unix-PTY braucht.
10. **Ergebnis unter Windows (gemessen am 25.11.2026).** 61 von 62 sichtbaren Fällen grün, mit `wb-dienst.exe` als Gegenseite. Rot ist `br_starten_tiefe_127` (502 statt 422); er ist unter Linux mit dem echten `wb-dienst` als Gegenseite ebenso rot und mit der Referenz grün (gemessen). Ursache (hergeleitet, nicht gemessen): Der Dienst bekommt den Körper in einem Ereignis eine Ebene tiefer, und `serde_json` lehnt ab Tiefe 128 ab. Das betrifft das Zusammenspiel von Brücke und Dienst, nicht Windows. **Nachtrag 25.11.2026:** Die Ursache ist gemessen. Die Brücke sendet `{"art": "Starten", "start": <Körper>}`, für den Dienst also 128 Ebenen. `serde_json` liest höchstens 127 Ebenen; der Dienst antwortete mit dem Fehler `json_ungueltig` und trennte, die Brücke antwortete darum 502. Seither liest der Dienst das Ereignis `Starten` Glied für Glied (`ereignis_lesen` in `rust/crates/dienst/src/json.rs`): Jeder Wert eines Glieds hat für sich höchstens 127 Ebenen, das Ereignis also höchstens 128. Die Start-Struktur von 127 Ebenen lehnt die Prüfstelle mit `start.json_ungueltig` ab, die Brücke antwortet 422. Jede andere zu tiefe Nutzlast bleibt `json_ungueltig`. Unter Linux sind mit dem echten `wb-dienst` als Gegenseite 62 von 62 sichtbaren Fällen grün (gemessen).
11. **Offen: Windows Error Reporting (Projektentscheid, 25.11.2026).** Vor jeder Auslieferung unter Windows wird Windows Error Reporting für die Programme ausgeschlossen.
12. **Offen: Mutanten der Windows-Stellen (Projektentscheid, 25.11.2026).** Das Tor `mutants` lässt `src/sys/windows.rs` aus (`rust/.cargo/mutants.toml`, `exclude_globs`), weil es hinter `cfg(windows)` steht und unter Linux nie übersetzt wird. Vor jeder Auslieferung unter Windows laufen die Mutanten dieser Stellen in der Windows-Prüfmaschine; überlebt einer, ist die Auslieferung gesperrt.
