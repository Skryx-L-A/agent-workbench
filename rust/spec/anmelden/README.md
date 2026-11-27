# Vertrag 9: Anmeldung der Copilot CLI (`werkbank-anmelden`)

Stand 27.11.2026 (Nachtrag System-Truststore, Schritt 6). Befehl `werkbank-anmelden copilot [--uebernehmen] [--neu]`. Crate `rust/crates/anmelden/`, Programm `werkbank-anmelden`.
Grundlage: Vertrag 1 (`rust/spec/start/`, Harness `copilot`, Schlüsselweg Option (a)), Vertrag 8
(`rust/spec/modellweg/`, Abschnitt 5) und der Halter-Vertrag (`rust/spec/halter/README.md`, Abschnitt 2.3, Nachtrag B05).

**Nachtrag Halter (23.11.2026, Schritt I, Linux):** Das Ziel der Anmeldung ist das Geheimnis `anmeldung.copilot` in der
Ablage des Halters, nicht mehr die Datei `/var/lib/werkbank/<nutzer>/copilot.schluessel`. Grund: Ab Schritt I liegt der
Anmeldeschlüssel der Cloud-Route nur beim Halter (Weg A, Befund B05); unter Linux Schritt I schreibt `werkbank-anmelden` ihn
direkt in dessen Ablage (Projektentscheid, 23.11.2026, F7 b). Was sich ändert, steht an der Stelle mit „alt → neu“. Ablauf,
Codes, Exitcodes und feste Sätze bleiben; die Schlüsseldatei heißt in diesem Vertrag weiter so und meint jetzt diese Datei.

## 1. Zweck und Grenze

Nach `copilot login` liegt das Token der Organisation sonst im Heim des Nutzers (`~/.copilot/config.json` oder im
Schlüsselbund unter `copilot-cli`). Im Produkt soll es nur in der Ablage des Halters liegen, als Datei `anmeldung.copilot`
(Abschnitt 2). Diese Datei liest nur der Halter, außerhalb des Einschlusses; der Modellweg erhält das Token nie, und im
Einschluss steht nur der feste Platzhalter. alt: „nur in `/var/lib/werkbank/<nutzer>/copilot.schluessel`; diese Datei liest
der Modellweg“.

Der Befehl meldet die CLI in einem Wegwerf-Heim an und legt das Token in die Schlüsseldatei. Er startet keine Sitzung
und kein Modell. Er prüft nie, ob das Token beim Dienst gilt.

Mit `--uebernehmen` (Abschnitt 3b) übernimmt er eine Anmeldung, die die CLI schon im Heim des Nutzers hält: Das Token
wandert aus `config.json` in die Schlüsseldatei, ohne neue Anmeldung und ohne Browser. Danach steht es nur noch in der
Schlüsseldatei. Der Gerätefluss bleibt der Weg für neue Nutzer; einen Weg über stdin gibt es nicht.

## 2. Aufruf

- `werkbank-anmelden copilot` legt die Datei an. Ist sie da, endet der Befehl mit `anmelden.datei_da`, bevor die CLI startet.
- `werkbank-anmelden copilot --neu` ersetzt eine vorhandene Datei atomar (rename).
- `werkbank-anmelden copilot --uebernehmen` und `werkbank-anmelden copilot --uebernehmen --neu` übernehmen die Anmeldung
  der CLI (Abschnitt 3b); `--neu` ersetzt dort eine Schlüsseldatei mit anderem Inhalt.
- Jede andere Form (kein Argument, anderes Harness, unbekannte Option, `--neu` vor `copilot`, `--neu` doppelt,
  `--neu` vor `--uebernehmen`, `--uebernehmen` doppelt oder vor `copilot`) endet mit `anmelden.aufruf`. Der Befehl liest
  nichts von stdin, nimmt kein Argument mit einem Geheimnis und kennt keine weitere Option.

**Ziel (Nachtrag Halter).** Der Zielordner ist die Ablage des Halters `<heim>/.local/state/agent-workbench/halter/geheim/`,
die Schlüsseldatei darin heißt `anmeldung.copilot` (Name des Geheimnisses nach dem Halter-Vertrag, Abschnitt 4). `<heim>` ist das
Feld 6 des Eintrags der eigenen Nutzerkennung in `/etc/passwd`; es muss ein absoluter Pfad sein. `HOME` und `XDG_STATE_HOME`
liest der Befehl nicht. Grund: Der Halter läuft als Benutzereinheit (`wb-halter.service`) mit der Umgebung des Nutzer-Managers,
der Befehl mit der Umgebung des Terminals und `postrm` als root; nur der Pfad aus `passwd` ist für alle drei derselbe (Halter-Vertrag
2.3). alt: „Der Zielordner ist `/var/lib/werkbank/<nutzer>`. Der Name des Nutzers kommt aus `/etc/passwd` nach der eigenen
Nutzerkennung und muss ein einfacher Kontoname sein (`[a-z_][a-z0-9_-]*`, höchstens 32 Zeichen). Den Ordner legt das Paket an
(Eigentümer der Nutzer, 0700); der Befehl legt ihn nie an.“

Regeln für die Ablage (Schritt 3 und 3b Schritt 6):

- Die Ebenen unter dem Heim sind `.local`, `.local/state`, `agent-workbench`, `halter` und `geheim`. Der Befehl öffnet das Heim
  (`O_DIRECTORY`) und jede Ebene relativ zur vorigen mit `O_NOFOLLOW`; er setzt nie einen Pfad zusammen.
- Das Heim und jede vorhandene Ebene ist ein Ordner, kein Verweis, gehört der eigenen Nutzerkennung und ist für Gruppe und andere
  nicht schreibbar. `geheim` hat genau den Modus 0700.
- Fehlt eine Ebene, legt der Befehl sie an (`mkdirat`, Modus 0700, unter `umask` 077) und prüft sie danach wie eine vorhandene.
  Legt ein anderer Prozess sie zugleich an (`EEXIST`), gilt die Prüfung der vorhandenen Ebene. alt: Der Befehl legte den Zielordner
  nie an.
- Verletzt eine Ebene eine Regel, fehlt das Heim oder hat `passwd` keinen Eintrag mit absolutem Heim: `ziel_nicht_schreibbar`. Der
  Befehl ändert dann keine Ebene, keinen Modus und keinen Eigentümer.
- Die Ablage gehört derselben Nutzerkennung wie der Halter unter Linux Schritt I (Halter-Vertrag 2.3). Der Halter beachtet nur
  Dateien, deren Name die Form eines Geheimnisses hat; Temp-Dateien mit einem Punkt am Anfang sieht er nie als Geheimnis.
- Die Datei `/var/lib/werkbank/<nutzer>/copilot.schluessel` des früheren Ziels liest, ändert und löscht der Befehl nie. Sie
  wird nicht mehr gelesen; `werkbank-pruefen` meldet sie (Paketvertrag, Prüfung `copilot`), `apt purge` löscht sie.

## 3. Ablauf

Der Befehl führt diese Schritte in dieser Reihenfolge aus und bricht beim ersten Fehler ab.

1. Aufruf prüfen (`aufruf`).
1a. Reste früherer Läufe räumen (Abschnitt 3a, `rest_nicht_loeschbar`). Das geschieht vor jedem weiteren Schritt, auch wenn danach `datei_da` folgt.
2. Ist die Datei da und fehlt `--neu`: `datei_da`. Ein Link an dieser Stelle zählt als Datei.
3. Ablage prüfen und fehlende Ebenen anlegen (Abschnitt 2, „Regeln für die Ablage“; `ziel_nicht_schreibbar`). alt:
   „Zielordner prüfen: ein Ordner, die eigene Kennung als Eigentümer, Schreibrecht des Eigentümers“.
4. `copilot` im PATH des Aufrufers suchen (`cli_fehlt`).
5. Wegwerf-Heim anlegen (`heim_nicht_anlegbar`): Ordner 0700 im Ordner des Halters `<heim>/.local/state/agent-workbench/halter`
   (die Ebene über `geheim`, Schritt 3 hat sie angelegt und geprüft) mit dem Namen `wb-anmelden-<pid>-<nanos>`, angelegt relativ
   zum ohne Verweis geöffneten Ordner, darin `heim/` und `heim/.copilot/` (beide 0700) und `heim/.copilot/settings.json` (0600)
   mit genau `{"storeTokenPlaintext":true}`. alt: unter `/var/tmp`. Grund: `/var/tmp` darf ein Agent als Projektwurzel einbinden
   und sah dort während des Logins und nach einem harten Abbruch `heim/.copilot/config.json` mit dem Token im Klartext. Den Ordner
   des Halters bindet der Einschluss für keinen Harness ein (`ANMELDE_ORDNER` in `rust/crates/einschluss/src/verbote.rs`).
   Verworfen: `$XDG_RUNTIME_DIR`, weil der Ort aus der Umgebung des Aufrufers käme und ein kleines tmpfs ist; das Heim kommt
   wie die Ablage allein aus `passwd`.
6. `copilot login --device-code` mit leerer Umgebung außer:
   - `HOME` = `heim/`, `COPILOT_HOME` = `heim/.copilot/`;
   - `NODE_USE_SYSTEM_CA` = `1`, fest gesetzt; ein Wert des Aufrufers gilt nie. Herkunft „Projektentscheid, 27.11.2026“: Die CLI
     1.0.88 nutzt für Login und Anfragen rustls über ihre native Laufzeit mit dem System-Truststore. Die Variable schadet nicht und
     hält, falls eine spätere CLI auf TLS von Node wechselt;
   - `PATH` des Aufrufers ohne jeden Ordner, der ein ausführbares `gh` enthält;
   - aus der Umgebung des Aufrufers nur `HTTPS_PROXY`, `HTTP_PROXY`, `ALL_PROXY`, `NO_PROXY` (je groß und klein),
     `SSL_CERT_FILE`, `SSL_CERT_DIR`, `NODE_EXTRA_CA_CERTS`, `TERM`, `LANG`, `LC_ALL`.
   Es gibt also keine D-Bus-Adresse, keinen Schlüsselbund, kein `gh` und kein Token der Umgebung (`COPILOT_GITHUB_TOKEN`,
   `GH_TOKEN`, `GITHUB_TOKEN`). stdin, stdout und stderr der CLI sind die des Befehls: Der Nutzer sieht Adresse und Code
   des Geräteflusses. Endet die CLI mit einem Exit ungleich 0 oder durch ein Signal: `login_abgebrochen`.
7. `heim/.copilot/config.json` lesen (höchstens 1 MiB). Fehlt sie, ist sie kein JSON (Lesart nach M1), hat sie kein Token oder
   mehr als ein Token im Feld: `kein_token`.
8. Form prüfen (Annahme M2, `form_falsch`): Präfix `gho_`, `ghu_` oder `github_pat_`, danach nur `[A-Za-z0-9_]`, im Ganzen 20 bis 255
   Zeichen, und nicht der feste Platzhalter von Vertrag 1 (`gho_` und 36 Zeichen `0`). Ein klassisches Token `ghp_` fällt hier durch.
9. Schlüsseldatei schreiben: Temp-Datei `.anmeldung.copilot.<pid>.tmp` im Zielordner mit `O_EXCL` und 0600, Inhalt Token und
   ein LF (Form einer Datei der Ablage nach dem Halter-Vertrag, Abschnitt 2.3), fsync. Dann ohne `--neu` ein Hardlink auf den
   Zielnamen (ist die Datei inzwischen da: `datei_da`), mit `--neu` ein rename; danach fsync des Zielordners. Die Temp-Datei wird
   in jedem Fall gelöscht. Eigentümer ist der aufrufende Nutzer. Kein Schritt davor und danach gibt das Token aus. alt: Temp-Datei
   `.copilot.schluessel.<pid>.tmp`, Form nach Vertrag 8 Abschnitt 5, kein fsync des Ordners genannt.
10. Wegwerf-Heim löschen.
11. Echtes Heim prüfen (Abschnitt 4). Nur wenn alles stimmt: stdout `Anmeldung gespeichert.`, Exit 0.

Das Wegwerf-Heim wird auf jedem Weg gelöscht: Erfolg, jeder Fehler, SIGINT, SIGTERM, SIGHUP und Panik. Bei SIGKILL oder
Stromausfall bleibt es liegen; der nächste Start räumt es (Abschnitt 3a).

## 3a. Reste früherer Läufe

Wird `werkbank-anmelden` hart beendet (SIGKILL, Stromausfall), bleibt das Wegwerf-Heim liegen und kann ein Token enthalten.
Der Befehl sucht deshalb beim Start (Schritt 1a) in der Wurzel der Wegwerf-Heime (dem Ordner des Halters, Schritt 5; alt:
`/var/tmp`; nicht flach durchsucht: nur die direkten Einträge) nach Resten und löscht sie vor dem Login. Ein Eintrag ist ein Rest, wenn alle Bedingungen gelten:

- Der Name hat genau die Form `wb-anmelden-<pid>-<nanos>`: `<pid>` und `<nanos>` sind nichtleere Ziffernfolgen, die Zahl `<pid>` ist größer als 0.
- Der Eintrag ist ein regulärer Ordner. Ein Verweis (auch auf einen Ordner) ist nie ein Rest, wird nie verfolgt und nie gelöscht.
- Der Eigentümer des Ordners ist die eigene Nutzerkennung. Ordner anderer Nutzer bleiben.
- Der Prozess `<pid>` lebt nicht mehr (`kill(pid, 0)` meldet ESRCH) oder gehört dem Nutzer nicht (EPERM). Die eigene pid des Befehls zählt als nicht lebend:
  ein Ordner mit ihr ist beim Start schon alt. Bei einer unklaren Antwort gilt der Prozess als lebend, der Ordner bleibt.

Lebt der Prozess, bleibt der Ordner: Er gehört einem laufenden Lauf (OPEN 6 nennt die Grenze).

Temp-Dateien (Nachtrag 21.11.2026) sind ebenfalls Reste: im Zielordner `.anmeldung.copilot.<pid>.tmp` (beide Wege; sie kann
das Token tragen; alt: `.copilot.schluessel.<pid>.tmp`; Temp-Dateien mit dem alten Namen unter `/var/lib/werkbank/<nutzer>` sucht
der Befehl nicht mehr) und mit `--uebernehmen` im Ordner der CLI `.config.json.wb-anmelden-<pid>.tmp`. Es gelten dieselben
Bedingungen: genau diese Namensform mit einer Ziffernfolge größer 0 als `<pid>`, eine reguläre Datei (ein Verweis ist nie ein
Rest), Eigentümer der Nutzer, Prozess tot. Der Ordner wird ohne Verweis geöffnet; Ansehen und Löschen geschehen relativ zu
ihm (`fstatat` ohne Verweis, `unlinkat`). Fehlt er oder ist er nicht lesbar, gibt es dort keine Reste. Die Suche nach Resten legt
keine Ebene der Ablage an. Sie zählen in dieselbe
Zahl `<n>` des Hinweises. Gelöscht wird nur über Deskriptoren: Die Wurzel wird einmal ohne Verweis geöffnet, jede Ebene des
Rests relativ zur Ebene darüber mit `O_NOFOLLOW`, jeder Eintrag mit `fstatat` ohne Verweis angesehen und mit `unlinkat` relativ zu
seinem Ordner entfernt; ein Verweis im Baum wird als Verweis entfernt, sein Ziel bleibt. Dasselbe gilt für das Löschen des eigenen
Wegwerf-Heims (Schritt 10). alt: „mit dem Löschen eines Verzeichnisbaums, das nie einem Verweis im Baum folgt“. Fehlt die Wurzel oder ist sie nicht lesbar, gibt es keine Reste (kein Fehler;
das Anlegen des Heims meldet dann `heim_nicht_anlegbar`).

Wurde mindestens ein Rest gelöscht, schreibt der Befehl vor dem Login genau eine Zeile auf stderr:
`Hinweis: Reste frueherer Anmeldungen wurden geloescht: <n>` und LF, mit der Zahl `<n>` der gelöschten Reste. Die Zeile nennt nie einen
Pfad, einen Namen oder einen Inhalt. Wurde nichts gelöscht, gibt es keine Zeile. Scheitert das Löschen eines Rests, endet der Befehl mit
`anmelden.rest_nicht_loeschbar` (Exit 18); es gibt dann keinen Hinweis, keinen Login und keine Schlüsseldatei (fail closed). Der Befehl
bricht beim ersten Rest ab, der nicht gelöscht werden kann.

## 3b. Übernahme einer bestehenden Anmeldung (`--uebernehmen`)

Nachtrag vom 21.11.2026. Bedingungen (alle Pflicht):

1. Der Befehl liest weiter kein stdin und kein Argument mit einem Geheimnis. Die Quelle ist nur die `config.json` der CLI
   des aufrufenden Nutzers im Ordner `COPILOT_HOME` (sonst `~/.copilot`), derselbe Ordner wie in Abschnitt 4. Die Datei ist
   regulär, gehört dem Nutzer, ist kein Verweis und hat genau einen Namen. Sie wird über einen Deskriptor geöffnet
   (`O_NOFOLLOW`, relativ zum geöffneten Ordner); der Ordner wird ebenfalls ohne Verweis geöffnet (`O_DIRECTORY`,
   `O_NOFOLLOW` am letzten Teil). Sie ist höchstens 1 MiB groß. Die Auslese folgt M1 (genau ein Token im Feld `authTokens`, gemessene Form), die
   Form Schritt 8 (M2).
2. Die Schlüsseldatei entsteht mit 0600 und atomar wie in Schritt 9: ohne `--neu` als Hardlink (ist die Datei da:
   `datei_da`), mit `--neu` als rename über die alte Datei. Der rename ist ein Tausch (`RENAME_EXCHANGE` unter Linux,
   `RENAME_SWAP` unter macOS), damit die alte Datei bei einem späteren Fehler zurückkommt.
3. Das Token wird nie ausgegeben, nie protokolliert und nie in eine Fehlermeldung gesetzt, auch nicht in Panik- oder
   Debug-Ausgaben. Der Typ des Tokens hat weder `Debug` noch `Display`.
4. Nach Erfolg steht das Token nicht mehr in `config.json`. Die übrigen Bytes der Datei bleiben unverändert: Entfernt werden
   genau Schlüssel, Doppelpunkt und Wert des einen Blatts (des Tokentexts) und ein angrenzendes Komma mit dem Leerraum dazwischen
   (beim einzigen Blatt bleibt sein Elternobjekt als leeres Objekt; leere Elternobjekte bleiben). Die führenden Kommentarzeilen
   und die Reihenfolge aller Schlüssel bleiben Byte für Byte erhalten. Die Datei wird atomar neu geschrieben, mit gleichem Eigentümer,
   gleicher Gruppe und gleichem Modus. Bei jedem Fehler bleibt alles wie vorher (alles oder nichts); für einen Abbruch
   dazwischen gilt der Wiederanlauf unten.
5. Reste wie beim Gerätefluss (Abschnitt 3a, Exit 18): Auch die Temp-Dateien dieses Wegs werden beim nächsten Start geräumt.
6. Fälle und Mutanten stehen in `faelle-uebernehmen/` und im Einschluss (Abschnitt 7).

Ablauf. Der Befehl führt diese Schritte in dieser Reihenfolge aus und bricht beim ersten Fehler ab. Alle Dateien werden
relativ zu einem geöffneten Ordner angelegt, verknüpft, umbenannt und gelöscht (`openat`, `linkat`, `renameat`, `unlinkat`),
nie über einen zusammengesetzten Pfad. Jeder Name besteht aus einem Teil ohne `/` und ist nicht `.` oder `..`.

1. Aufruf prüfen (`aufruf`).
1a. Reste räumen (Abschnitt 3a): Wegwerf-Heime, Temp-Dateien des Schlüssels im Zielordner und Temp-Dateien von
   `config.json` im Ordner der CLI (`rest_nicht_loeschbar`).
2. Ordner der CLI öffnen (ohne Verweis). Fehlt er: `quelle_ohne_token`. Ist er ein Verweis, gehört er nicht dem Nutzer
   oder ist er für Gruppe oder andere schreibbar: `quelle_unsicher`.
3. `config.json` relativ dazu öffnen (`O_NOFOLLOW`, `O_NONBLOCK`) und lesen. Fehlt sie: `quelle_ohne_token`. Ist sie ein
   Verweis, keine reguläre Datei, nicht die des Nutzers, hat sie mehr als einen Namen oder mehr als 1 MiB: `quelle_unsicher`.
4. Auslese nach M1. Kein JSON (nach den führenden Kommentarzeilen), kein oder mehr als ein Token, das Feld nicht genau einmal
   oben, oder bliebe nach dem Entfernen des Blatts eine Anmeldung nach Abschnitt 4 im Text (etwa ein weiterer tokenartiger Text):
   `quelle_ohne_token`.
5. Form nach Schritt 8 (`form_falsch`).
6. Ablage öffnen, prüfen und fehlende Ebenen anlegen wie Schritt 3 (`ziel_nicht_schreibbar`). Dann die Schlüsseldatei ansehen
   (ohne Verweis zu folgen): Fehlt sie, entsteht sie neu. Enthält sie genau dieses Token und LF (reguläre Datei des Nutzers,
   ein Name, 0600), ist es ein Wiederanlauf: Schritt 8 entfällt. Sonst endet der Befehl ohne `--neu` mit `datei_da`.
   Bis hier ist nichts geschrieben außer fehlenden Ebenen der Ablage. Ein Signal (SIGINT, SIGTERM, SIGHUP) bis zu diesem Punkt endet mit `signal`.
7. Neue `config.json` als Temp-Datei `.config.json.wb-anmelden-<pid>.tmp` im Ordner der CLI (`O_EXCL`, Modus und Gruppe der
   alten Datei), fsync. Scheitert das: `quelle_nicht_schreibbar`.
8. Schlüsseldatei: Temp-Datei `.anmeldung.copilot.<pid>.tmp` im Zielordner (`O_EXCL`, 0600, Token und LF), fsync. Dann
   `linkat` auf den Zielnamen (ersetzt nie). Ist der Name da: ohne `--neu` `datei_da`, mit `--neu` Tausch mit der alten Datei.
   Nach dem Hardlink wird der Temp-Name gelöscht, nach dem Tausch hält er die alte Datei bis Schritt 10. fsync des
   Zielordners. Scheitert ein Teil: `ziel_nicht_schreibbar`, die Temp-Dateien werden gelöscht.
9. `config.json` ersetzen: Nur wenn die Datei noch dieselbe ist wie beim Lesen (Gerät, Inode, Größe, Änderungszeit), wird
   die Temp-Datei per `renameat` an ihren Platz gesetzt, dann fsync des Ordners. Scheitert das oder hat sich die Datei
   geändert: Schritt 8 wird zurückgenommen (der neue Name gelöscht, sofern er noch dieselbe Inode ist; ein Tausch wird
   zurückgetauscht), die Temp-Dateien werden gelöscht, `quelle_nicht_schreibbar`.
10. Nach einem Tausch den Temp-Namen mit der alten Schlüsseldatei löschen, fsync des Zielordners.
11. Echtes Heim prüfen (Abschnitt 4). Nur wenn alles stimmt: stdout `Anmeldung gespeichert.`, Exit 0. War es ein
    Wiederanlauf, steht vorher auf stderr genau die Zeile `Hinweis: Eine unterbrochene Uebernahme wurde abgeschlossen.`

Ab Schritt 7 schließt der Befehl die Schritte bei einem Signal ab (sie dauern Millisekunden) und endet mit ihrem Ergebnis.

Wiederanlauf (Entscheidung). Zwei Dateien in zwei Ordnern lassen sich nicht in einem Schritt tauschen. Die Reihenfolge ist
deshalb: erst die Schlüsseldatei, dann `config.json`. So liegt das Token nach jedem Abbruch an mindestens einer dauerhaften
Stelle, nie an keiner. Was der nächste Aufruf mit `--uebernehmen` vorfindet:

- Abbruch vor dem Hardlink oder Tausch (Schritt 7 bis 8): `config.json` ist unverändert, die Schlüsseldatei fehlt oder ist die
  alte. Liegen Temp-Dateien da, räumt Schritt 1a sie (die des Schlüssels kann das Token tragen). Der Aufruf läuft dann wie neu.
- Abbruch nach dem Hardlink oder Tausch und vor dem Ersetzen von `config.json` (Schritt 8 bis 9): Das Token liegt in der
  Schlüsseldatei und noch in `config.json`. Der nächste Aufruf räumt die Temp-Dateien (nach einem Tausch die alte
  Schlüsseldatei) und findet in Schritt 6 eine Schlüsseldatei mit genau diesem Token. Er führt den Lauf vorwärts zu Ende:
  Er schreibt nur `config.json` neu (Schritt 7 und 9) und lässt die Schlüsseldatei unberührt, auch ohne `--neu`.
- Abbruch nach dem Ersetzen von `config.json` (Schritt 9 bis 11): Die Übernahme ist fertig. Ein neuer Aufruf räumt die
  Temp-Datei der alten Schlüsseldatei und endet mit `quelle_ohne_token`, weil das Token nur noch in der Schlüsseldatei liegt.

Begründung für vorwärts statt zurück: Die Schlüsseldatei ist in diesem Zustand vollständig und synchronisiert, und ihr
Inhalt gleicht dem Token in `config.json` Byte für Byte. Das ist die einzige Marke; ein Protokoll des Laufs gibt es nicht,
weil es selbst eine weitere Datei wäre. Vorwärts entfernt nur die doppelte Kopie aus `config.json` und erreicht das Ziel.
Zurück müsste die Schlüsseldatei löschen und wüsste nicht sicher, ob sie von diesem Lauf stammt. Das Token liegt also nur im
Fenster zwischen Schritt 8 und 9 doppelt (Millisekunden) oder nach einem harten Abbruch in diesem Fenster bis zum nächsten
Aufruf. Ein erneuter Aufruf von `werkbank-anmelden copilot --uebernehmen` schließt ihn ab (OPEN 7).

## 3c. Schutz vor einem Kernabbild

Nachtrag vom 21.11.2026, neu gefasst am 22.11.2026 (Vertragsbefunde B02, B03). Der Befehl hält das Token und den Text von
`config.json` im Speicher, das Kind `copilot login` hält das Token. Ein Kernabbild (etwa nach SIGQUIT oder SIGSEGV) legte es
außerhalb der Schlüsseldatei ab. Die Regel steht im Crate `rust/crates/kernabbild` („Kernabbild-Grenze 1/1“); dieser Abschnitt
legt nur fest, wie der Befehl sie anwendet. Sie gilt für beide Wege (Gerätefluss und `--uebernehmen`).

- **Elternprozess.** Als erste Handlung, vor dem Prüfen des Aufrufs und bevor er etwas liest, ruft der Befehl
  `kernabbild::abschirmen`: unter Linux `PR_SET_DUMPABLE` 0, auf jedem System `RLIMIT_CORE` weich 1 und hart 1. Ist die
  geerbte harte Grenze schon 0, entscheidet das Crate nach `core_pattern`: Ein Muster mit `|` am Anfang oder ein nicht lesbares
  Muster bricht ab; ein Dateimuster oder kein Muster (macOS) läuft weiter mit der Zeile `Hinweis: <Text des Crates>` auf stderr,
  und die Grenze bleibt 0/0. Jeder Abbruch endet mit `anmelden.abschirmung` (Exit 5) und tut nichts weiter (fail closed).
  Unter macOS gibt es `PR_SET_DUMPABLE` nicht; dort gilt nur die Grenze.
- **Kind `copilot login` bis zum `exec`.** Das Kind entsteht durch fork (bzw. `clone` mit `CLONE_VM` in `posix_spawn`) aus dem
  abgeschirmten Elternprozess. Es erbt `PR_SET_DUMPABLE` 0 (Kennzeichen des Adressraums) und die Grenze 1/1 bis zum `exec`.
  Unmittelbar vor dem Start prüft der Befehl diesen Stand erneut (`schutz::vor_dem_kind`): nicht abzugsfähig und Grenze 1/1, oder
  0/0 mit einer Entscheidung des Crates ohne Abbruch. Jeder andere Stand endet mit `anmelden.abschirmung` (Exit 5); die CLI startet
  nicht, das Wegwerf-Heim wird gelöscht.
- **Kind nach dem `exec`.** `execve` setzt `PR_SET_DUMPABLE` für ein Programm ohne Rechtewechsel wieder auf 1; `RLIMIT_CORE` bleibt
  erhalten. Beleg am Kernel-Quelltext v7.0: `fs/exec.c`, `begin_new_exec` ruft `set_dumpable(current->mm, SUID_DUMP_USER)`, wenn
  effektive und reale Kennungen gleich sind und die Datei lesbar ist; dieselbe Funktion setzt nur `RLIMIT_STACK` neu. Gemessen
  (Test `the_device_flow_runs_undumpable_and_the_cli_inherits_rlimit_core_1`, Linux): Die Einträge unter `/proc/<pid>` der Attrappe
  gehören dem Nutzer, und ihr `/proc/self/limits` zeigt 1 und 1. Für das fremde Programm nach dem `exec` hält also allein die Grenze
  1/1.
- **Warum die Grenze 1 genügt (Quelltext).** `fs/coredump.c` v7.0, `coredump_pipe`: Bei einem Muster mit Pipe bricht der Kernel ab,
  wenn die weiche Grenze genau 1 ist (`RLIMIT_CORE is set to 1, aborting core`). Bei einem Dateimuster bricht er ab, wenn die Grenze kleiner
  als `min_coredump` ist (ELF: eine Seite). `coredump_skip` überspringt jeden Prozess mit `PR_SET_DUMPABLE` 0 vor beiden Wegen,
  unabhängig von `fs.suid_dumpable`; der Wert 2 von `fs.suid_dumpable` gilt nur für Prozesse, deren `exec` die Kennung gewechselt hat.
- **Gemessen in einer Wegwerf-VM (22.11.2026).** Ubuntu 26.04.1, Kernel 7.0.0-43-generic, apport 2.34.1 aktiv,
  `core_pattern` mit Pipe an apport, `fs.suid_dumpable` 2. Eine Attrappe der CLI (Python, eine feste Marke im Speicher, keine echte
  CLI und kein Token) stirbt an SIGSEGV. Ergebnis je Fall:
  - Ein ungeschützter Prozess (Grenzen der Shell, weich 0) und einer mit 0/0: apport wird aufgerufen (`called for global pid …,
    core limit 0`); ein eigener Pipe-Empfänger bekommt ein Abbild von 6,8 MB mit der Marke. Das ist die Positivkontrolle und
    zugleich die Gegenprobe zur bisherigen Grenze 0/0.
  - Kind des Befehls bis 21.11.2026 (Grenze 0/0): apport und der eigene Empfänger werden aufgerufen, das Abbild trägt die Marke.
  - Kind dieses Befehls (1/1): Weder apport noch der Empfänger wird aufgerufen; der Kernel meldet `coredump: <pid>(copilot):
    RLIMIT_CORE is set to 1, aborting core`. Bei einem Dateimuster entsteht keine Datei (ein ungeschützter Prozess mit
    unbegrenzter Grenze legt dort eine Datei mit der Marke ab).
  - Geerbte harte Grenze 0 mit Pipe-Muster: Exit 5 mit `kernabbild.muster_pipe`, die CLI startet nicht.
  - Der Befehl selbst (alter und neuer Bau): Das Kind schickt ihm SIGABRT. Er endet mit 134, weder apport noch der Empfänger wird
    aufgerufen. Positivkontrolle mit demselben Signal: Eine abzugsfähige Attrappe mit 0/0 erreicht apport (`signal 6`) und den
    Empfänger (Abbild mit der Marke). SIGSEGV per `kill` taugt dafür nicht: Die Laufzeit von Rust fängt es ab, der Befehl lief weiter.
- **Grenze des Schutzes (Entscheid vom 22.11.2026).** Das Kind startet mit der Grenze 1/1. Eine weitergehende
  Härtung des Kindes ist geplant und Pflicht vor Schritt II.
- **Prüfung am laufenden Programm.** Ob die echte Copilot CLI ihre Grenze ändert, prüft das Projekt in Teil B am laufenden Kind:
  `/proc/<pid>/limits`, Zeile `Max core file size`, erwartet weich 1 und hart 1. Dieser Vertrag startet die echte CLI nie.

## 4. Prüfung des echten Heims

Nach dem Ablegen prüft der Befehl, dass keine Anmeldung der CLI im echten Heim liegt. Er liest nichts davon in einen Text
der Ausgabe und zeigt nichts.

- `config.json` im Ordner `COPILOT_HOME` (sonst `~/.copilot`) des Aufrufers: Eine Anmeldung ist da, wenn das Feld `authTokens` (M1) in der Tiefe
  Objekt, Objekt, Text einen nichtleeren Text hat, oder irgendein Text im Dokument das Präfix eines Tokens trägt (länger als 19
  Zeichen), oder die Datei nach der Lesart von M1 kein JSON ist (fail closed; eine andere Form als die gemessene zählt so). Fehlt die Datei, gibt es dort nichts.
- Schlüsselbund (M3): `secret-tool lookup service copilot-cli` (Linux) oder `security find-generic-password -s copilot-cli`
  (macOS), je nachdem, was im PATH des Aufrufers steht. Ausgabe und Fehlerausgabe werden verworfen; allein der Exit zählt
  (0: Eintrag da), mit 10 Sekunden Frist (Fristablauf: nicht prüfbar). Findet ein Werkzeug einen Eintrag: Anmeldung da.
- Steht keines der beiden Werkzeuge im PATH oder antwortet keines: Der Befehl endet mit Exit 0 und schreibt auf stderr den
  festen Hinweis `Hinweis: Der Schluesselbund wurde nicht geprueft, weil secret-tool und security fehlen.`

Ist eine Anmeldung da, endet der Befehl mit `anmelden.echte_anmeldung`. Die Schlüsseldatei ist dann schon geschrieben und bleibt;
der Hinweis sagt, dass die Anmeldung im echten Heim weg muss. Es gibt kein stilles Weiter und keinen Exit 0.

## 5. Codes, Exitcodes und feste Sätze

Jeder Fehler schreibt genau eine Zeile auf stderr: `werkbank-anmelden: <code>: <Satz>` und LF. Der Satz enthält nie einen
Pfad, einen Namen oder einen Teil eines Tokens. Ausnahme: Bricht `kernabbild::abschirmen` ab, folgt auf die Zeile von
`anmelden.abschirmung` eine zweite Zeile `werkbank-anmelden: <Grund>: <Abhilfe>` mit dem festen Grund und der Abhilfe des
Crates (etwa `kernabbild.muster_pipe`); beide ohne Pfad und ohne Wert. stdout bleibt bei einem Fehler, was die CLI geschrieben hat.

| # | Code | Exit | Satz | Ursache |
|---|---|---|---|---|
| 1 | `anmelden.aufruf` | 64 | `Aufruf: werkbank-anmelden copilot [--uebernehmen] [--neu]` | Aufruf nicht in einer Form aus Abschnitt 2 |
| 2 | `anmelden.cli_fehlt` | 10 | `Die Copilot CLI wurde nicht gefunden.` | kein ausführbares `copilot` im PATH |
| 3 | `anmelden.login_abgebrochen` | 11 | `Die Anmeldung wurde nicht abgeschlossen.` | die CLI endet mit Exit ungleich 0 oder startet nicht |
| 4 | `anmelden.kein_token` | 12 | `Die Anmeldung hat kein Token geliefert.` | `config.json` fehlt, ist kein JSON oder hat nicht genau ein Token |
| 5 | `anmelden.form_falsch` | 13 | `Das Token hat nicht die erwartete Form.` | Form nach Schritt 8 verfehlt |
| 6 | `anmelden.ziel_nicht_schreibbar` | 14 | `Der Zielordner ist nicht schreibbar.` | Heim aus `passwd` fehlt; eine Ebene der Ablage ist ein Verweis, kein Ordner, nicht die eigene, für Gruppe oder andere schreibbar oder nicht anlegbar; `geheim` nicht 0700; Temp-Datei oder rename scheitert (alt: Ordner fehlt, nicht der eigene oder nicht schreibbar) |
| 7 | `anmelden.datei_da` | 15 | `Die Schluesseldatei ist schon da. Mit --neu wird sie ersetzt.` | Datei da ohne `--neu` |
| 8 | `anmelden.echte_anmeldung` | 16 | `Im Heim des Nutzers liegt eine Anmeldung der Copilot CLI. Sie muss weg, bevor eine Sitzung startet.` | Abschnitt 4 |
| 9 | `anmelden.heim_nicht_anlegbar` | 17 | `Das Wegwerf-Heim ist nicht anlegbar.` | Wurzel fehlt oder ist nicht beschreibbar |
| 10 | `anmelden.signal` | 128 + n | `Abgebrochen durch ein Signal.` | SIGINT (130), SIGTERM (143), SIGHUP (129) |
| 11 | `anmelden.rest_nicht_loeschbar` | 18 | `Ein Rest eines frueheren Laufs ist nicht loeschbar.` | Abschnitt 3a: ein Rest eines toten Laufs lässt sich nicht löschen |
| 12 | `anmelden.quelle_ohne_token` | 19 | `In der Konfiguration der Copilot CLI liegt nicht genau ein Token.` | Abschnitt 3b, Schritt 2 bis 4: Ordner oder `config.json` fehlt, kein JSON, nicht genau ein Token, sonst bliebe eine Anmeldung |
| 13 | `anmelden.quelle_unsicher` | 20 | `Die Konfiguration der Copilot CLI ist keine eigene regulaere Datei bis 1 MiB.` | Abschnitt 3b, Schritt 2 und 3: Verweis, nicht regulär, fremder Eigentümer, zweiter Name, geteilter Ordner, größer als 1 MiB |
| 14 | `anmelden.quelle_nicht_schreibbar` | 21 | `Die Konfiguration der Copilot CLI ist nicht neu schreibbar.` | Abschnitt 3b, Schritt 7 und 9: Temp-Datei oder rename scheitert, oder die Datei hat sich geändert; Schritt 8 ist zurückgenommen |
| 15 | `anmelden.abschirmung` | 5 | `Der Schutz vor einem Kernabbild ist nicht setzbar.` | Abschnitt 3c: `kernabbild::abschirmen` bricht ab, oder der Stand vor dem Kind ist nicht abgeschirmt; geprüft in der Bibliothek und mit geerbter harter Grenze 0, kein Fall |

Bei einem Signal beendet der Befehl die CLI (SIGTERM, nach 2 Sekunden SIGKILL), wartet auf sie, löscht das Wegwerf-Heim und
schreibt keine Schlüsseldatei. Das Signal gilt bis zum Beginn von Schritt 9. Kommt es später, schließt der Befehl Schritt 9 bis 11
ab (sie dauern Millisekunden) und endet mit dem Ergebnis dieser Schritte.

## 6. Annahmen

- **M1 (gemessen, CLI 1.0.88, 21.11.).** `config.json` der echten CLI nach einer Anmeldung in der angemeldeten VM (inhaltsfreie
  Diagnose): 509 Byte, keine BOM, LF, 23 Zeilen. Die Zeilen 1 und 2 sind ganze Kommentarzeilen, die mit `//` beginnen. Ab Zeile 3
  steht mehrzeiliges, eingerücktes JSON ohne Komma vor `}` oder `]`. Die oberste Ebene hat die Felder `appTipShown`, `authTokens`
  (Objekt), `firstLaunchAt`, `lastLoggedInUser` (Objekt), `loggedInUsers` (Liste) und `reasoningSummariesCleanupDone`; die Reihenfolge ist nicht gemessen.
  `copilot_tokens` fehlt. Genau ein Text hat ein Token-Präfix: unter `authTokens` in der Tiefe Objekt, Objekt, Text (Klasse `gho_`,
  40 Zeichen). Die Schlüsselnamen darunter werden nicht gelesen und nicht ausgegeben. Die Lesart ist genau diese: Nur führende
  ganze Zeilen mit `//` vor dem ersten `{` werden übersprungen; jedes andere Nicht-JSON (Kommentar in der Mitte, Blockkommentar, Komma
  vor `}`) ist `kein_token` bzw. `quelle_ohne_token`. Das Token ist der eine nicht leere Text in dieser Tiefe, und kein weiterer Text der
  Datei hat ein Token-Präfix. **Eine andere Form (etwa die frühere Annahme `copilot_tokens`) endet mit Exit 19 bzw. 12 und ändert
  nichts: das ist ein gewollter Halt, kein Fehler.** Gemessen ist außerdem, dass `config.json` mit Modus 0600 entsteht und dass die
  CLI ohne Schlüsselbund ihr Token dort ablegt. Die Prüfung des echten Heims (Abschnitt 4) liest dieselbe Lesart; ein Text, der nach diesen
  Regeln kein JSON ist, zählt als Anmeldung (fail closed).
- **M2 (nicht gemessen).** Die Form des Tokens nach Schritt 8. Gemessen sind die Arten `gho_`, `ghu_` und `github_pat_`; die Grenzen
  20 und 255 sind gesetzt, nicht gemessen.
- **M3 (nicht gemessen).** Der Schlüsselbund-Eintrag trägt das Attribut `service` mit dem Wert `copilot-cli` (Linux), bzw. den
  Dienstnamen `copilot-cli` (macOS).

## 7. Fälle

`faelle/<name>.json` beschreibt einen Lauf, `erwartet/<name>.json` die Antwort. `bau-faelle.py` erzeugt beide;
`pruefen.py` prüft Paarung, Felder und die Übereinstimmung mit Abschnitt 5. Die Tests (`rust/crates/anmelden/tests/faelle.rs`)
bauen die Attrappe der CLI, das Wegwerf-Heim und das Zielverzeichnis aus dem Fall und vergleichen stdout, stderr und Exit
byte für byte.

**Fälle des Nachtrags Halter.** Seit der Code des Crates `anmelden` in die Ablage des Halters schreibt, stehen die Fälle des Nachtrags in
`faelle/`, `erwartet/`, `faelle-uebernehmen/` und `erwartet-uebernehmen/`; die Ordner `*-ausstehend/` sind entfernt (alt: dort lagen sie, bis
der Code folgte).

Felder eines Falls: `beschreibung`; `args` (Argumente des Befehls); `attrappe` (`erfolg`: die gemessene Form der CLI 1.0.88, `abbruch`, `ohne-config`, `config-ohne-feld`,
`zwei-tokens`, `config-defekt`, `alte-form`: die frühere Annahme `copilot_tokens`, `haengt`, `fehlt`); `token` (`praefix`, `fuell`, `laenge`: Text = Präfix und das Füllzeichen, aufgefüllt auf
die Länge; kein Token steht als Text in einer Datei); `ziel` (`frei`, `datei-da`, `ordner-fehlt`, `schreibgeschuetzt`, und seit dem
Nachtrag Halter `heim-fehlt`, `ebene-verweis`, `ebene-geteilt`, `alte-datei`; Bedeutung unter „Ziel der Fälle“);
`echtes_heim` (`leer`, `config-mit-token`, `config-defekt`); `bundtool` (`keins`, `leer`, `gefunden`); `wegwerf_wurzel` (`ok`, `fehlt`);
`signal` (`null`, `INT`, `TERM`, `HUP`); `reste` (`keine`, `tot`, `lebend`, `verweis`, `nicht-loeschbar`: Inhalt der Wurzel des Wegwerf-Heims vor dem Lauf,
je ein Ordner mit dem Namen nach Abschnitt 3a: `tot` mit der pid eines beendeten Prozesses, `lebend` mit der pid eines laufenden Prozesses des Nutzers, `verweis` ein
Verweis auf einen Ordner außerhalb mit einer Probedatei, `nicht-loeschbar` ein toter Rest mit einem Unterordner ohne Schreibrecht).

**Ziel der Fälle (Nachtrag Halter).** Der Test baut ein Heim mit eigenem Eintrag für `passwd` (untergeschobene Lesart des
Heims, wie die untergeschobene Nutzerkennung in Abschnitt 8) und darin die Ablage. `frei`: alle Ebenen da, `geheim` 0700, keine
Schlüsseldatei. `datei-da`: dazu `anmeldung.copilot` mit einem anderen Inhalt. `ordner-fehlt`: keine Ebene ab `.local` ist da; der
Befehl legt alle an (alt: der Zielordner fehlt, Ergebnis `ziel_nicht_schreibbar`). `schreibgeschuetzt`: `geheim` hat den Modus 0500.
`heim-fehlt`: der Eintrag in `passwd` nennt ein Heim, das es nicht gibt. `ebene-verweis`: `.local/state` ist ein Verweis auf einen
Ordner des Nutzers außerhalb. `ebene-geteilt`: `agent-workbench` hat den Modus 0770. `alte-datei`: wie `frei`, dazu die Datei des
früheren Ziels `/var/lib/werkbank/<nutzer>/copilot.schluessel` (im Test unter dem Testordner); sie ist danach byte-gleich da.
Nach jedem Fall mit `ziel_nicht_schreibbar` ist keine Ebene neu, kein Modus geändert und das Ziel eines Verweises unverändert.

Fälle der Übernahme (Abschnitt 3b) stehen getrennt in `faelle-uebernehmen/` und `erwartet-uebernehmen/`; die Tests liegen in
`rust/crates/anmelden/tests/uebernehmen.rs`. Felder eines Falls: `beschreibung`; `args`; `quelle` (`erfolg`, `erfolg-letzter`,
`erfolg-allein`: das Blatt in der Mitte, am Ende oder allein, in einer Datei mit weiteren Feldern; `erfolg` hat die gemessene Form
mit zwei Kommentarzeilen; `ohne-config`, `kein-json`, `ohne-feld`, `zwei-tokens`, `weitere-anmeldung`, `token-ausserhalb`,
`token-andere-tiefe`, `kommentar-mitte`, `blockkommentar`, `komma-vor-klammer`, `alte-form`, `verweis`, `hardlink`, `zu-gross`, `fifo`, `ordner`); `quellordner`
(`ok`, `fehlt`, `verweis`, `gruppe-schreibbar`); `token`; `ziel` (`frei`, `datei-da`, `datei-gleich`: die Datei hat schon dieses
Token, `ordner-fehlt`, `schreibgeschuetzt`, `heim-fehlt`, `ebene-verweis`; Bedeutung wie oben unter „Ziel der Fälle“); `bundtool`; `reste` (`keine`, `schluessel-temp`, `config-temp`, `beide`, `lebend`,
`verweis`); `probe` (`null` oder `config-stoeren`: das Ersetzen von `config.json` scheitert). Felder der Antwort: `exit`, `code`,
`stdout`, `stderr`, `ziel_datei` (`token`, `alt`, `keine`), `config` (`ohne-token`: genau der Eintrag fehlt, alles andere ist
byte-gleich; `unveraendert`), `reste_geloescht`, `reste_uebrig` (Temp-Dateien in beiden Ordnern nach dem Lauf).

Die Probe der Tests heißt `WB_ANMELDEN_PROBE` und gilt nur mit `WB_TESTUMGEBUNG=1`: `toeten:<punkt>` beendet den Befehl mit
SIGKILL an einem Punkt zwischen den Schritten (`vor-schreiben`, `config-temp`, `schluessel-temp`, `schluessel`,
`temp-geloescht`, `config`), `warten:<punkt>` wartet dort auf ein Signal (mit der Marke `WB_ANMELDEN_PROBE_MARKE`),
`config-stoeren` lässt Schritt 9 scheitern, `grenze-weich-0` senkt nach der Abschirmung die weiche Grenze für Kernabbilder auf 0
(harte 1), damit die Prüfung vor dem Kind (Abschnitt 3c) abbricht. Ohne `WB_TESTUMGEBUNG=1` gibt es keine Probe.

Felder der Antwort: `exit`; `code` (oder `null`); `stdout`; `stderr`; `ziel_datei` (`neu`, `ersetzt`, `unveraendert`, `nicht-angelegt`);
`login_gestartet`; `reste_geloescht` (Zahl der gelöschten Reste, gleich der Zahl im Hinweis); `reste_uebrig` (Zahl der Einträge in der Wurzel des Wegwerf-Heims nach dem Lauf).
Das Wegwerf-Heim des Laufs selbst liegt nach jedem Fall nicht mehr dort; bei `verweis` ist das Ziel des Verweises unverändert.

Bei jedem gestarteten Login prüfen die Tests, dass die CLI genau `NODE_USE_SYSTEM_CA=1` sieht, obwohl der Aufrufer `NODE_USE_SYSTEM_CA=0`
setzt.

Die Attrappe der CLI gibt die gemessenen Zeilen des Geräteflusses aus (`To authenticate, visit https://github.com/login/device and enter
code ABCD-1234`, `Waiting for authorization...`), prüft in der Umgebung ihres Starts (HOME und COPILOT_HOME im Wegwerf-Heim, Modi 0700,
kein `gh` im PATH, keine D-Bus-Adresse, keine Token-Variable) und schreibt `config.json` in ihr `COPILOT_HOME` mit dem Token.

## 8. Weitere Prüfungen der Tests

Außer den Fällen prüfen die Tests: Das Token steht in keinem Byte von stdout, stderr, den Argumenten und der Umgebung des Befehls und der
CLI, in keiner Datei außer der Schlüsseldatei (und dem Skript der Attrappe). Die Schlüsseldatei ist 0600, gehört dem Nutzer und
enthält Token und LF. Nach Erfolg, jedem Fehler und jedem der drei Signale ist das Wegwerf-Heim weg und die Kind-Prozesse sind beendet.
Das Heim des Aufrufers bleibt unberührt. Die Fälle für den Fremdbesitz und für den Prozess eines anderen Nutzers (Rest bleibt bzw. wird gelöscht)
prüft die Bibliothek mit einer untergeschobenen Nutzerkennung, weil ein Test ohne Root keinen fremden Ordner anlegen kann.

Für die Übernahme prüfen die Tests außerdem: Nach einem SIGKILL an jedem Punkt zwischen den Schritten liegt das Token an
mindestens einer Stelle, und der nächste Aufruf führt zum Ziel (Schlüsseldatei mit Token, `config.json` ohne Token, keine
Temp-Datei); SIGINT, SIGTERM und SIGHUP vor Schritt 7 ändern nichts, danach endet der Lauf mit Erfolg. In keinem Lauf steht das
Token in stdout, stderr oder einer Datei außer der Schlüsseldatei (und `config.json`, solange die Übernahme nicht fertig ist).
Der fremde Eigentümer von Ordner, `config.json` und Zielordner wird mit einer untergeschobenen Nutzerkennung geprüft.

## 9. OPEN

1. (erledigt, 21.11.2026) Feld und Form des Tokens in `config.json` (M1) sind mit der CLI 1.0.88 gemessen (Abschnitt 6). Offen bleibt
   nur, ob `copilot login --device-code` mit `storeTokenPlaintext` ohne Rückfrage speichert und ob die CLI im Gerätefluss einen
   Browser öffnen will (Punkt 5). Ändert eine neuere CLI die Form, endet der Lauf mit Exit 19 (gewollter Halt) und ist neu zu messen.
2. Form und Grenzen des Tokens (M2) und der Eintrag im Schlüsselbund (M3) sind nicht gemessen.
3. (geändert, Nachtrag Halter) Der Paketeintrag `/usr/bin/werkbank-anmelden` liegt im Paket (`rust/spec/paket/README.md`). Die
   Ablage legt der Befehl selbst an (Abschnitt 2); das Paket legt im Heim nichts an. alt: „das Anlegen von
   `/var/lib/werkbank/<nutzer>` (0700) liegt im Paket; ohne diesen Ordner endet der Befehl mit `ziel_nicht_schreibbar`“.
4. (erledigt) Das Aufräumen von Resten nach SIGKILL oder Stromausfall steht in Abschnitt 3a.
5. Reicht der gefilterte PATH nicht für die CLI (sie braucht ein Werkzeug aus einem Ordner mit `gh`), scheitert der Login sichtbar
   mit `login_abgebrochen`. Eine CLI, die einen Browser öffnen will, ist nicht gemessen.
6. Grenzen des Aufräumens (Abschnitt 3a): Ein Rest, dessen pid ein anderer laufender Prozess des Nutzers übernommen hat, bleibt liegen, bis dieser
   Prozess endet. Zwei Läufe zugleich sind nicht ausgeschlossen; ein Lauf löscht nie den Ordner eines lebenden. Ein Rest unter einer anderen Wurzel
   wird nicht gesehen, auch kein Rest eines älteren Laufs unter `/var/tmp` (alte Wurzel, Schritt 5); ihn räumt der Neustart
   des Systems oder die Person. alt: „(anderes `/var/tmp`-Mapping, `PrivateTmp` eines Dienstes)“. Die Namensform ist die einzige Marke: ein Ordner des Nutzers mit
   gleichem Namen, der nicht von diesem Befehl stammt, wird ebenfalls gelöscht.
7. Übernahme (Abschnitt 3b): Die Form der Datei (M1) ist für CLI 1.0.88 gemessen; eine andere Form endet mit
   `quelle_ohne_token` und ändert nichts. Ein Stromausfall ist nicht getestet: Die Reihenfolge der fsync-Aufrufe ist
   hergeleitet, nicht gemessen. Schreibt die CLI selbst gerade `config.json`, erkennt Schritt 9 das nur über Inode, Größe und
   Änderungszeit; ein Schreiben in derselben Zeitmarke mit gleicher Größe bliebe unerkannt. Der Tausch mit `--neu` braucht ein
   Dateisystem mit `RENAME_EXCHANGE` bzw. `RENAME_SWAP`; fehlt es, endet der Aufruf mit `ziel_nicht_schreibbar` und ändert
   nichts. Ein Abbruch nach Schritt 9 lässt einen neuen Aufruf mit `quelle_ohne_token` enden, obwohl die Übernahme fertig ist.
   Scheitert bei einem Fehler nach dem Tausch der Rücktausch selbst, löscht das Aufräumen der Temp-Datei die alte
   Schlüsseldatei; der Zustand gleicht dann dem harten Abbruch zwischen Schritt 8 und 9 mit `--neu`, und der nächste Aufruf
   schließt ihn ab. Ersetzt die CLI `config.json` genau zwischen der Prüfung auf dieselbe Datei und dem `renameat` in Schritt 9,
   überschreibt Schritt 9 ihre neue Fassung: Deren Änderung geht verloren, ein Token wird dabei nicht offengelegt.
   Liegt nach einem harten Abbruch zwischen Schritt 8 und 9 das Token doppelt, meldet das nur ein neuer Aufruf mit
   `--uebernehmen` (er schließt den Zustand ab); eine Prüfung der `config.json` des Heims außerhalb dieses Befehls gibt es nicht.
8. Kernabbild (Abschnitt 3c, neu gefasst am 22.11.2026): Das Kind startet mit der Grenze 1/1. Eine weitergehende
   Härtung des Kindes ist geplant, Pflicht vor Schritt II und ein eigener Schritt. Ob die echte CLI ihre Grenze ändert, prüft das Projekt in Teil B am laufenden Kind.
9. **Nachtrag Halter: Folgen außerhalb dieses Vertrags.** (a) (erledigt) Der Code (`rust/crates/anmelden/`, Tests `faelle.rs` und
   `uebernehmen.rs`) folgt diesem Stand; das Programm schreibt in die Ablage des Halters, die Fälle stehen in den gelesenen Ordnern. (b) Der Modellweg (Vertrag 8) und `lauf.json`
   (`schluessel.copilot`, Paketvertrag) lesen bis zu ihrem Zug den alten Ort. Zwischen dem Bau dieses Stands und deren Zug findet
   die Cloud-Route also kein Token; sie scheitert dann sichtbar, nie still. (c) Der Läufer liest „angemeldet“ heute aus
   der alten Datei; er braucht den neuen Ort oder `schluessel.vorhanden` des Halters. (d) Unter Linux Schritt II gilt dieser Weg
   nicht: Dann schreibt `werkbank-anmelden` nicht mehr in die Ablage, sondern übergibt über `schluessel.setzen` an den Halter
   unter eigenem Konto (Halter-Vertrag, OPEN 18).
10. **Fremder Eigentümer einer Ebene der Ablage** lässt sich ohne root nicht als Fall bauen; die Bibliothek prüft ihn mit der
    untergeschobenen Nutzerkennung (Abschnitt 8). Nicht gemessen: Verhalten auf einem Heim unter NFS oder mit `root_squash`.

<!-- vom Bauvorgang angefuegt -->
> Hinweis: Dieser Text nennt Werkzeuge, die in diesem Klon nicht liegen: `agent-workbench`.
> Sie gehoeren zum Setup der Maschine, aus der dieser Baum stammt; hier fehlen
> sie mit Absicht. Wer sie braucht, schreibt sich seine eigenen.
> *Named here, not shipped here.*
